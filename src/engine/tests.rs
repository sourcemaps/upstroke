//! Extended notes: `docs/internals/engine/tests.md`
// LEGACY-EFFECT: this module is in the **frozen legacy section** of
// `effects/allowlist.toml`, which carries its justification and the condition

#![allow(clippy::disallowed_methods, clippy::disallowed_types)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use super::attempt::{
    QUESTION_MARKER, artifact_path, evaluate_outcome, materialize_prompt, review_failure,
    worker_question,
};
use super::coordinator::{question_options, run_contained, run_harness_inner, run_harness_on};
use super::preflight::{gates_differ, validate_inputs};
use super::report::{sum_opt, task_report, total_of};
use super::resume::{resume_contained, resume_harness_inner};
use super::*;
use crate::agent::{AgentAdapter, Caps, ProcessOutput, TaskRun};
use crate::capacity;
use crate::config;
use crate::error::UpstrokeError;
use crate::events::{self, EventBody, EventLog, GateSummary, Progress, RunState, TaskState};
use crate::interaction::{self, AnswerSource, QuestionRecord, Sleeper};
use crate::ir::{
    Answer, Effort, Outcome, OutcomeStatus, PermissionMode, Question, QuestionId, QuestionKind,
    ResolvedEffortPolicy, Task, TaskId, TaskKind, Usage, WorkerProfile,
};
use crate::review;
use crate::rundir::scratch_tree::ScratchTree;
use crate::rundir::{self, RunLock, RunPaths, WorktreeLock};
use crate::runner::CommandSpec;
use crate::topology::effects::EventSite;
use crate::workspace::Workspace;

#[derive(Clone, Copy, PartialEq)]
enum Effect {
    EditFile,

    EditTest,

    LargeEdit,

    OpaqueEdit,

    IgnoredGateInput,

    FrozenCandidate,

    JamCleanupAfterReview,

    LargeEditQuestionWriteFailure,

    NoEdit,

    SpawnError,

    Error,

    RateLimited,

    AskQuestion,

    Exit,
}

const CRASH_EXIT_CODE: i32 = 42;

#[derive(Clone, Copy, PartialEq)]
enum ReviewBehavior {
    Pass,
    Fail,

    Unparseable,

    RateLimited,

    SpawnError,

    NeedsHuman,
}

struct FakeAdapter {
    id: &'static str,
    effects: Vec<Effect>,
    reviews: Vec<ReviewBehavior>,

    probe_error: Option<&'static str>,

    reports_cost: bool,
    calls: Mutex<Calls>,
    role_probe: Option<RoleProbe>,
}

#[derive(Default)]
struct Calls {
    worker: usize,
    review: usize,
    review_spawn_failures: usize,
    runs: Vec<RecordedRun>,
    review_snapshots: Vec<(String, String)>,
}

#[derive(Clone)]
struct RecordedRun {
    model: String,
    resume: Option<String>,
    prompt: String,
}

const REVIEW_MARKER: &str = "UPSTROKE-FAKE-REVIEW";

impl FakeAdapter {
    fn new(effects: Vec<Effect>, reviews: Vec<ReviewBehavior>) -> Self {
        Self {
            id: "claude-code",
            effects,
            reviews,
            probe_error: None,
            reports_cost: true,
            calls: Mutex::new(Calls::default()),
            role_probe: None,
        }
    }

    fn copilot(reviews: Vec<ReviewBehavior>) -> Self {
        Self {
            id: "copilot",
            effects: Vec::new(),
            reviews,
            probe_error: None,
            reports_cost: true,
            calls: Mutex::new(Calls::default()),
            role_probe: None,
        }
    }

    fn probing(mut self, probe: RoleProbe) -> Self {
        self.role_probe = Some(probe);
        self
    }

    fn broken(mut self, message: &'static str) -> Self {
        self.probe_error = Some(message);
        self
    }

    fn unpriced(mut self) -> Self {
        self.reports_cost = false;
        self
    }

    fn runs(&self) -> Vec<RecordedRun> {
        self.calls
            .lock()
            .map(|c| c.runs.clone())
            .unwrap_or_default()
    }

    fn reviews_run(&self) -> usize {
        self.calls.lock().map(|c| c.review).unwrap_or_default()
    }

    fn review_spawn_failures(&self) -> usize {
        self.calls
            .lock()
            .map(|c| c.review_spawn_failures)
            .unwrap_or_default()
    }

    fn review_snapshots(&self) -> Vec<(String, String)> {
        self.calls
            .lock()
            .map(|calls| calls.review_snapshots.clone())
            .unwrap_or_default()
    }
}

fn fake(effect: Effect) -> FakeSource {
    source(vec![effect], vec![ReviewBehavior::Pass])
}

fn source(effects: Vec<Effect>, reviews: Vec<ReviewBehavior>) -> FakeSource {
    FakeSource {
        adapter: FakeAdapter::new(effects, reviews),
        copilot: None,
    }
}

fn cross_vendor(
    effects: Vec<Effect>,
    reviews: Vec<ReviewBehavior>,
    second: Vec<ReviewBehavior>,
) -> FakeSource {
    FakeSource {
        adapter: FakeAdapter::new(effects, reviews),
        copilot: Some(FakeAdapter::copilot(second)),
    }
}

fn scripted<T: Copy>(script: &[T], index: usize, fallback: T) -> T {
    script
        .get(index)
        .copied()
        .or_else(|| script.last().copied())
        .unwrap_or(fallback)
}

impl AgentAdapter for FakeAdapter {
    fn id(&self) -> &'static str {
        self.id
    }

    fn probe(&self, _runner: &dyn crate::runner::Runner) -> Result<Caps, UpstrokeError> {
        if let Some(message) = self.probe_error {
            return Err(UpstrokeError::Agent {
                message: message.to_owned(),
            });
        }
        Ok(Caps {
            version: "0.0.0-fake".to_owned(),
            json_output: true,
            session_resume: true,
            cost_reporting: true,
            read_only_mode: true,
            acp: false,
            model_list: false,
        })
    }

    fn build(&self, run: &TaskRun) -> Result<CommandSpec, UpstrokeError> {
        if run.profile.permissions == PermissionMode::ReadOnly {
            let effect = self
                .calls
                .lock()
                .map(|calls| {
                    scripted(
                        &self.effects,
                        calls.worker.saturating_sub(1),
                        Effect::EditFile,
                    )
                })
                .unwrap_or(Effect::EditFile);
            let behavior = {
                let mut calls = self.calls.lock().map_err(|_| UpstrokeError::Agent {
                    message: "fake adapter lock poisoned".to_owned(),
                })?;
                let index = calls.review + calls.review_spawn_failures;
                let behavior = scripted(&self.reviews, index, ReviewBehavior::Pass);
                if behavior == ReviewBehavior::SpawnError {
                    calls.review_spawn_failures += 1;
                }
                behavior
            };
            if behavior == ReviewBehavior::SpawnError {
                return Ok(CommandSpec::new(
                    run.workspace
                        .join("missing-reviewer-executable")
                        .to_string_lossy(),
                ));
            }
            if effect == Effect::FrozenCandidate {
                let tree = Command::new("git")
                    .arg("-C")
                    .arg(&run.workspace)
                    .args(["rev-parse", "HEAD^{tree}"])
                    .output()
                    .map_err(|e| UpstrokeError::Agent {
                        message: format!("fake could not inspect reviewer tree: {e}"),
                    })?;
                if !tree.status.success() {
                    return Err(UpstrokeError::Agent {
                        message: format!(
                            "fake could not inspect reviewer tree: {}",
                            String::from_utf8_lossy(&tree.stderr).trim()
                        ),
                    });
                }
                let contents =
                    fs::read_to_string(run.workspace.join("agent-output.txt")).map_err(|e| {
                        UpstrokeError::Agent {
                            message: format!("fake could not inspect reviewer candidate: {e}"),
                        }
                    })?;
                self.calls
                    .lock()
                    .map_err(|_| UpstrokeError::Agent {
                        message: "fake adapter lock poisoned".to_owned(),
                    })?
                    .review_snapshots
                    .push((
                        String::from_utf8_lossy(&tree.stdout).trim().to_owned(),
                        contents,
                    ));
            }
            if effect == Effect::JamCleanupAfterReview {
                let common = Command::new("git")
                    .arg("-C")
                    .arg(&run.workspace)
                    .args(["rev-parse", "--git-common-dir"])
                    .output()
                    .map_err(|e| UpstrokeError::Agent {
                        message: format!("fake could not inspect git common dir: {e}"),
                    })?;
                if !common.status.success() {
                    return Err(UpstrokeError::Agent {
                        message: format!(
                            "fake could not inspect git common dir: {}",
                            String::from_utf8_lossy(&common.stderr).trim()
                        ),
                    });
                }
                let common = PathBuf::from(String::from_utf8_lossy(&common.stdout).trim());
                let common = if common.is_absolute() {
                    common
                } else {
                    run.workspace.join(common)
                };
                fs::write(common.join("index.lock"), "jam\n").map_err(|e| {
                    UpstrokeError::Agent {
                        message: format!("fake could not jam cleanup: {e}"),
                    }
                })?;
            }

            if let Some(probe) = &self.role_probe {
                return Ok(probe.reviewer.clone());
            }
            return Ok(shell_spec(&format!("echo {REVIEW_MARKER}")));
        }
        let index = {
            let mut calls = self.calls.lock().map_err(|_| UpstrokeError::Agent {
                message: "fake adapter lock poisoned".to_owned(),
            })?;
            let index = calls.worker;
            calls.worker += 1;
            calls.runs.push(RecordedRun {
                model: run.profile.model.clone(),
                resume: run.resume_session.clone(),
                prompt: run.prompt.clone(),
            });
            index
        };
        let edit: Option<(&str, String)> = match scripted(&self.effects, index, Effect::EditFile) {
            Effect::Exit => {
                let _ = fs::write(
                    run.workspace.join("agent-output.txt"),
                    "half-written by an agent that never came back\n",
                );
                std::process::exit(CRASH_EXIT_CODE);
            }
            Effect::EditFile
            | Effect::AskQuestion
            | Effect::JamCleanupAfterReview
            | Effect::FrozenCandidate => {
                let marker = run.workspace.join("agent-output.txt");
                let previous = fs::read_to_string(&marker).unwrap_or_default();
                Some(("agent-output.txt", format!("{previous}edited: {index}\n")))
            }
            Effect::EditTest => Some((
                "widget_test.rs",
                "#[test]\nfn widget_works() {\n    assert!(true);\n}\n".to_owned(),
            )),
            Effect::LargeEdit | Effect::LargeEditQuestionWriteFailure => Some((
                "large-agent-output.txt",
                "x".repeat(review::MAX_DIFF_BYTES + 1),
            )),
            Effect::OpaqueEdit => Some(("opaque-agent-output.bin", "\0hidden bytes".to_owned())),
            Effect::IgnoredGateInput => {
                fs::write(run.workspace.join(".gitignore"), "ignored.flag\n").map_err(|e| {
                    UpstrokeError::Agent {
                        message: format!("fake ignore rule failed: {e}"),
                    }
                })?;
                fs::write(run.workspace.join("ignored.flag"), "gate-only input\n").map_err(
                    |e| UpstrokeError::Agent {
                        message: format!("fake ignored input failed: {e}"),
                    },
                )?;
                Some(("agent-output.txt", "reviewed edit\n".to_owned()))
            }
            Effect::SpawnError => {
                return Ok(CommandSpec::new(
                    run.workspace
                        .join("missing-worker-executable")
                        .to_string_lossy(),
                ));
            }
            Effect::NoEdit | Effect::Error | Effect::RateLimited => None,
        };
        if let Some((name, content)) = edit {
            fs::write(run.workspace.join(name), content).map_err(|e| UpstrokeError::Agent {
                message: format!("fake edit failed: {e}"),
            })?;
        }
        if scripted(&self.effects, index, Effect::EditFile) == Effect::LargeEditQuestionWriteFailure
        {
            let run_id =
                rundir::latest_run(&run.workspace).ok_or_else(|| UpstrokeError::Agent {
                    message: "fake could not find the active run".to_owned(),
                })?;
            let questions = rundir::public_dir(&run.workspace, &run_id).join("questions");
            fs::remove_dir(&questions).map_err(|e| UpstrokeError::Agent {
                message: format!("fake could not remove questions directory: {e}"),
            })?;
            fs::write(&questions, "not a directory\n").map_err(|e| UpstrokeError::Agent {
                message: format!("fake could not block question writes: {e}"),
            })?;
        }
        if let Some(probe) = &self.role_probe {
            probe.read_as_the_operator_does(&run.workspace);
            return Ok(probe.worker.clone());
        }
        Ok(shell_spec("exit 0"))
    }

    fn materialize_permissions(
        &self,
        profile: &WorkerProfile,
        gate_cmds: &[String],
        dir: &Path,
        stem: &str,
    ) -> Result<Option<PathBuf>, UpstrokeError> {
        crate::agent::claude::ClaudeCodeAdapter
            .materialize_permissions(profile, gate_cmds, dir, stem)
    }

    fn parse(&self, out: &ProcessOutput) -> Result<Outcome, UpstrokeError> {
        if out.stdout.contains(REVIEW_MARKER) {
            let index = {
                let mut calls = self.calls.lock().map_err(|_| UpstrokeError::Agent {
                    message: "fake adapter lock poisoned".to_owned(),
                })?;
                let index = calls.review + calls.review_spawn_failures;
                calls.review += 1;
                index
            };
            let behavior = scripted(&self.reviews, index, ReviewBehavior::Pass);
            if behavior == ReviewBehavior::RateLimited {
                return Ok(fake_outcome(
                    OutcomeStatus::RateLimited,
                    Some("5-hour limit reached".to_owned()),
                    "fake-review-session",
                    Some(0.0),
                    out.duration,
                ));
            }
            let answer = match behavior {
                ReviewBehavior::Pass => {
                    "```json\n{\"pass\": true, \"reasons\": [\"meets the acceptance \
                         criteria\"], \"required_changes\": []}\n```"
                }
                ReviewBehavior::Fail => {
                    "```json\n{\"pass\": false, \"reasons\": [\"no error handling for \
                         empty input\"], \"required_changes\": \
                         [\"handle the empty-input case\"]}\n```"
                }
                ReviewBehavior::NeedsHuman => {
                    "```json\n{\"pass\": false, \"reasons\": [\"the acceptance criteria \
                         contradict the API contract\"], \"needs_human\": true}\n```"
                }
                ReviewBehavior::Unparseable => "Looks fine to me, ship it.",
                ReviewBehavior::RateLimited => unreachable!("handled above"),
                ReviewBehavior::SpawnError => unreachable!("handled during command build"),
            };
            return Ok(fake_outcome(
                OutcomeStatus::Completed,
                Some(answer.to_owned()),
                "fake-review-session",
                self.reports_cost.then_some(0.05),
                out.duration,
            ));
        }

        let index = self
            .calls
            .lock()
            .map(|c| c.worker.saturating_sub(1))
            .unwrap_or(0);
        let effect = scripted(&self.effects, index, Effect::EditFile);
        let status = match effect {
            Effect::Error => OutcomeStatus::AgentError,
            Effect::RateLimited => OutcomeStatus::RateLimited,

            Effect::EditFile
            | Effect::EditTest
            | Effect::LargeEdit
            | Effect::OpaqueEdit
            | Effect::IgnoredGateInput
            | Effect::FrozenCandidate
            | Effect::JamCleanupAfterReview
            | Effect::LargeEditQuestionWriteFailure
            | Effect::NoEdit
            | Effect::AskQuestion
            | Effect::SpawnError
            | Effect::Exit => OutcomeStatus::Completed,
        };
        let detail = match effect {
            Effect::Error => Some("fake adapter error detail".to_owned()),
            Effect::RateLimited => Some("5-hour limit reached".to_owned()),
            Effect::AskQuestion => Some(
                "I made a start but stopped.\nUPSTROKE-QUESTION: should cursors be opaque or \
                     signed?"
                    .to_owned(),
            ),
            _ => None,
        };
        let mut outcome = fake_outcome(
            status,
            detail,
            &format!("s{index}"),
            Some(0.01),
            out.duration,
        );
        outcome.usage = Some(Usage::default());
        Ok(outcome)
    }
}

fn fake_outcome(
    status: OutcomeStatus,
    detail: Option<String>,
    session: &str,
    cost_usd: Option<f64>,
    duration: Duration,
) -> Outcome {
    Outcome {
        status,
        diff: String::new(),
        detail,
        session_id: Some(session.to_owned()),
        usage: None,
        cost_usd,
        transcript_path: PathBuf::new(),
        duration,
    }
}

struct FakeSource {
    adapter: FakeAdapter,

    copilot: Option<FakeAdapter>,
}

impl FakeSource {
    fn copilot(&self) -> &FakeAdapter {
        self.copilot.as_ref().expect("this source has a copilot")
    }

    fn probing_with(mut self, probe: RoleProbe) -> Self {
        self.adapter = self.adapter.probing(probe);
        self
    }
}

impl AdapterSource for FakeSource {
    fn get(&self, id: &str) -> Option<&dyn AgentAdapter> {
        if id == self.adapter.id {
            return Some(&self.adapter as &dyn AgentAdapter);
        }
        self.copilot
            .as_ref()
            .filter(|a| a.id == id)
            .map(|a| a as &dyn AgentAdapter)
    }
}

struct ScriptedAnswers {
    answers: Mutex<std::collections::VecDeque<Answer>>,
}

impl ScriptedAnswers {
    fn new(answers: Vec<Answer>) -> Self {
        Self {
            answers: Mutex::new(answers.into()),
        }
    }
}

impl AnswerSource for ScriptedAnswers {
    fn id(&self) -> &'static str {
        "scripted"
    }

    fn resolve(&self, _question: &Question) -> Result<Answer, UpstrokeError> {
        Ok(self
            .answers
            .lock()
            .ok()
            .and_then(|mut a| a.pop_front())
            .unwrap_or(Answer::Unanswered))
    }
}

#[derive(Default)]
struct RecordingSleeper {
    waits: Mutex<Vec<Duration>>,
}

impl Sleeper for RecordingSleeper {
    fn sleep(&self, duration: Duration) {
        if let Ok(mut waits) = self.waits.lock() {
            waits.push(duration);
        }
    }
}

impl RecordingSleeper {
    fn waits(&self) -> Vec<Duration> {
        self.waits.lock().map(|w| w.clone()).unwrap_or_default()
    }
}

fn shell_spec(script: &str) -> CommandSpec {
    crate::gates::ShellKind::native().spec(script)
}

fn git_in(repo: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .expect("run git");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn candidate_mutation_marker(repo: &Path) -> PathBuf {
    let name = repo
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "repo".to_owned());
    repo.with_file_name(format!("{name}-candidate-mutation.txt"))
}

fn mutate_index_after_candidate_capture(
    workspace: &Workspace,
    candidate: &crate::workspace::CapturedCandidate,
) -> Result<(), UpstrokeError> {
    fs::write(
        workspace.root().join("agent-output.txt"),
        "tampered after capture\n",
    )
    .map_err(|error| UpstrokeError::Git {
        message: format!("test could not mutate the captured worktree: {error}"),
    })?;
    let add = Command::new("git")
        .arg("-C")
        .arg(workspace.root())
        .args(["add", "-A"])
        .output()
        .map_err(|error| UpstrokeError::Git {
            message: format!("test could not stage its post-capture mutation: {error}"),
        })?;
    if !add.status.success() {
        return Err(UpstrokeError::Git {
            message: format!(
                "test could not stage its post-capture mutation: {}",
                String::from_utf8_lossy(&add.stderr).trim()
            ),
        });
    }
    let tampered_tree = Command::new("git")
        .arg("-C")
        .arg(workspace.root())
        .arg("write-tree")
        .output()
        .map_err(|error| UpstrokeError::Git {
            message: format!("test could not inspect its post-capture tree: {error}"),
        })?;
    if !tampered_tree.status.success() {
        return Err(UpstrokeError::Git {
            message: format!(
                "test could not inspect its post-capture tree: {}",
                String::from_utf8_lossy(&tampered_tree.stderr).trim()
            ),
        });
    }
    let tampered_tree = String::from_utf8_lossy(&tampered_tree.stdout)
        .trim()
        .to_owned();
    if tampered_tree == candidate.tree_oid {
        return Err(UpstrokeError::Git {
            message: "test post-capture mutation did not change the staged tree".to_owned(),
        });
    }
    fs::write(
        candidate_mutation_marker(workspace.root()),
        format!(
            "{}\n{}\n{tampered_tree}\n",
            candidate.parent_oid, candidate.tree_oid
        ),
    )
    .map_err(|error| UpstrokeError::Git {
        message: format!("test could not record its capture identities: {error}"),
    })
}

/// A scratch tree this test owns, without a repository in it, guarded by the
/// token that authorises its deletion.
///
/// **Bind the guard to a live local**: `let _ = temp_engine_scratch("x")`
/// drops it at the end of that statement and deletes the fixture.
fn temp_engine_scratch(tag: &str) -> ScratchTree {
    let parent = std::env::temp_dir();
    match crate::rundir::scratch_tree::acquire(&parent, tag) {
        Ok(tree) => tree,
        Err(refusal) => panic!(
            "a scratch tree for `{tag}` under {}: {refusal:?}",
            parent.display()
        ),
    }
}

/// A git repository in a scratch tree this test owns, guarded by the token
/// that authorises the tree's deletion.
///
/// The helper here built `temp_dir()/upstroke-engine-<tag>-<pid>` and
/// pre-cleaned it with a discarded `remove_dir_all` before it had any claim on
/// the name, then returned a root nothing reclaimed
/// (`PR64-CLEANUP-003-SCRATCH-PRECLEAN`, `PR7-SCRATCH-FIXTURE-LEAK`).
/// `acquire` refuses an occupied root rather than emptying it, keys on a ULID
/// no recycled pid can supply, and reclaims on drop.
///
/// # The repository is a CHILD of the tree, not the tree itself
///
/// [`private_root_for`] puts a run's private half at
/// `<repo>-home`, a **sibling** of the repository, because the design keeps
/// the two halves apart (`DESIGN.md` §15, and
/// `agent_authored_files_land_outside_the_workspace`). A repository that was
/// the tree root would therefore put its private half beside the tree, where
/// no guard reaches it -- measured at 159 surviving `…-home` directories in
/// one green suite run. As a child, both halves are inside the guarded tree.
///
/// **Bind the guard to a live local**: `let (_tree, repo) = …` keeps the tree
/// for the rest of the scope; dropping the guard deletes the repository.
fn temp_engine_repo(tag: &str) -> (ScratchTree, PathBuf) {
    let parent = std::env::temp_dir();
    let tree = match crate::rundir::scratch_tree::acquire(&parent, tag) {
        Ok(tree) => tree,
        Err(refusal) => panic!(
            "a scratch tree for `{tag}` under {}: {refusal:?}",
            parent.display()
        ),
    };
    let dir = tree.path().join("repo");
    fs::create_dir(&dir).expect("the repository, a child of the guarded tree");
    git_in(&dir, &["init", "-q", "-b", "main"]);
    git_in(&dir, &["config", "user.email", "test@upstroke.local"]);
    git_in(&dir, &["config", "user.name", "upstroke tests"]);
    fs::write(dir.join("README.md"), "seed\n").expect("seed");
    fs::write(
        dir.join("plan.md"),
        "## Implement the widget\n<!-- upstroke: id=t1 depends= -->\nMake it.\n\n\
             ## Document the widget\n<!-- upstroke: id=t2 depends=t1 -->\nWrite it up.\n",
    )
    .expect("plan");
    git_in(&dir, &["add", "-A"]);
    git_in(&dir, &["commit", "-q", "-m", "seed"]);
    (tree, dir)
}

fn seed(repo: &Path, plan: &str, config: Option<&str>) {
    fs::write(repo.join("plan.md"), plan).expect("plan");
    if let Some(config) = config {
        fs::write(repo.join("upstroke.toml"), config).expect("config");
    }
    git_in(repo, &["add", "-A"]);
    git_in(repo, &["commit", "-q", "-m", "fixture"]);
}

fn options(repo: &Path) -> RunOptions {
    let mut opts = RunOptions::new(repo.join("plan.md"), repo.to_path_buf());
    opts.pools_path = Some(no_pools());
    opts.attempt_timeout = Duration::from_secs(60);

    opts.defer_backoff = Duration::ZERO;
    opts.wait_on_block = Some(Duration::ZERO);
    opts.private_root = Some(private_root_for(repo));
    opts
}

fn no_pools() -> PathBuf {
    /// The tree and the file, held together so the tree outlives every
    /// reader. The root was `temp_dir()/upstroke-engine-nopools-<pid>`,
    /// created and never removed (`PR7-SCRATCH-FIXTURE-LEAK`); it is
    /// process-wide because every test here reads it.
    struct Shared {
        _tree: ScratchTree,
        path: PathBuf,
    }

    static SHARED: OnceLock<Shared> = OnceLock::new();
    SHARED
        .get_or_init(|| {
            let tree = temp_engine_scratch("engine-nopools");
            let path = tree.path().join("pools.toml");
            fs::write(
                &path,
                "# no pools
",
            )
            .expect("empty pools file");
            Shared { _tree: tree, path }
        })
        .path
        .clone()
}

/// The run's private half: a **sibling** of the repository, as the design keeps
/// the two apart, and a fixed name rather than `<repo>-home`.
///
/// The repository is a child of its own guarded tree ([`temp_engine_repo`]), so
/// a sibling of it is inside that tree and no second fixture can collide on the
/// name. Four characters rather than nine matter because this segment is in the
/// deepest path the suite builds and Windows refuses a `$GIT_DIR` past 220
/// characters; `rundir::scratch_tree`'s `NAME_ENTROPY` records the whole budget.
fn private_root_for(repo: &Path) -> PathBuf {
    repo.with_file_name("home")
}

fn resume_options(repo: &Path, run_id: &str) -> ResumeOptions {
    let mut opts = ResumeOptions::new(run_id.to_owned(), repo.to_path_buf());
    opts.pools_path = Some(no_pools());
    opts.attempt_timeout = Duration::from_secs(60);
    opts.defer_backoff = Duration::ZERO;
    opts.wait_on_block = Some(Duration::ZERO);
    opts.private_root = Some(private_root_for(repo));
    opts
}

fn paths_of(repo: &Path, run_id: &str) -> RunPaths {
    RunPaths::with_private_root(repo, run_id, &private_root_for(repo))
}

fn committed(report: &RunReport, id: &str) -> bool {
    report
        .tasks
        .iter()
        .any(|t| t.id == id && matches!(t.status, TaskRunStatus::Committed { .. }))
}

fn task<'a>(report: &'a RunReport, id: &str) -> &'a TaskReport {
    report
        .tasks
        .iter()
        .find(|t| t.id == id)
        .unwrap_or_else(|| panic!("no task `{id}` in {report:?}"))
}

#[derive(Default)]
struct FailTheThirdLegacyAppend {
    entered: u32,
}

impl crate::events::log::EventHooks for FailTheThirdLegacyAppend {
    fn point(
        &mut self,
        site: EventSite,
        point: crate::topology::effects::SubEffectPoint,
        mode: crate::topology::effects::InjectionMode,
    ) -> crate::topology::effects::Injection {
        use crate::topology::effects::{Injection, InjectionMode, SubEffectPoint};
        if site != EventSite::LegacyAppend
            || point != SubEffectPoint::Written
            || mode != InjectionMode::ErrorReturn
        {
            return Injection::Proceed;
        }
        self.entered += 1;
        if self.entered == 3 {
            Injection::Error
        } else {
            Injection::Proceed
        }
    }
}

fn fail_the_third_legacy_append() -> Box<dyn crate::events::log::EventHooks> {
    Box::<FailTheThirdLegacyAppend>::default()
}

#[test]
fn a_returned_legacy_append_error_stops_the_run() {
    let (_tree, repo) = temp_engine_repo("legacy-append-error");
    let mut opts = options(&repo);
    opts.log_hooks = Some(fail_the_third_legacy_append);
    let source = fake(Effect::EditFile);

    let error = run_with(&opts, &source)
        .expect_err("a returned append error must reach the caller, not be swallowed");
    let message = error.to_string();
    assert!(
        message.contains("Event.LegacyAppend"),
        "the error must be the append's own, naming its site: {message}"
    );
    assert!(
        message.contains("Written"),
        "…and its point, so an operator can tell which coordinate failed: {message}"
    );
}

#[test]
fn a_returned_legacy_append_error_still_leaves_the_partial_report() {
    let (_tree, repo) = temp_engine_repo("legacy-append-partial");
    let mut opts = options(&repo);
    opts.log_hooks = Some(fail_the_third_legacy_append);
    let source = fake(Effect::EditFile);

    run_with(&opts, &source).expect_err("the append error stops the run");

    let runs = opts.repo_root.join(".upstroke").join("runs");
    let public = fs::read_dir(&runs)
        .expect("the runs root")
        .flatten()
        .map(|entry| entry.path())
        .find(|path| path.join("events.jsonl").is_file())
        .expect("the failed run left its public directory and its log");

    let log = fs::read_to_string(public.join("events.jsonl")).expect("the log");
    let complete = log.lines().filter(|line| line.ends_with('}')).count();
    assert!(
        complete >= 2,
        "only {complete} complete line(s) in the log: the injected failure landed on \
         a startup append, so this test never reached drain_and_report's branch"
    );

    let report = public.join("report.json");
    assert!(
        report.is_file(),
        "no report beside {}: the legacy engine's partial report is a courtesy for \
         whoever opens the directory next, and failing to write it must not be \
         silent (PR5-CONF-011)",
        public.display()
    );
    let parsed: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&report).expect("read the partial report"))
            .expect("the partial report is JSON");
    assert!(
        parsed.get("tasks").is_some(),
        "the partial report is a report, not a stub: {parsed}"
    );
}

#[test]
fn happy_path_commits_one_commit_per_task() {
    let (_tree, repo) = temp_engine_repo("happy");
    let source = fake(Effect::EditFile);
    let report = run_with(&options(&repo), &source).expect("run succeeds");

    assert_eq!(report.outcome(), RunOutcome::Complete);
    assert_eq!(report.tasks.len(), 2);
    assert!(
        report
            .tasks
            .iter()
            .all(|t| matches!(t.status, TaskRunStatus::Committed { .. })),
        "report: {report:?}"
    );

    assert!(
        (report.total_cost_usd - 0.12).abs() < 1e-9,
        "worker and reviewer spend both counted: {}",
        report.total_cost_usd
    );

    let branch = git_in(&repo, &["rev-parse", "--abbrev-ref", "HEAD"]);
    assert!(branch.trim().starts_with("upstroke/run-"), "on run branch");
    let count = git_in(&repo, &["rev-list", "--count", "main..HEAD"]);
    assert_eq!(count.trim(), "2", "one commit per task");
    let log = git_in(&repo, &["log", "--format=%s", "main..HEAD"]);
    assert!(
        log.contains("[upstroke] t1: Implement the widget"),
        "log: {log}"
    );
    assert!(log.contains("[upstroke] t2: Document the widget"));
    assert!(
        git_in(&repo, &["status", "--porcelain"]).trim().is_empty(),
        "clean tree after run"
    );
    assert!(
        repo.join(".upstroke").join("runs").exists(),
        "run dir written"
    );
}

#[test]
fn gates_review_and_commit_use_one_frozen_candidate_tree() {
    let (_tree, repo) = temp_engine_repo("one-frozen-candidate");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 depends= -->\n",
        Some(
            "[interaction]\nmode = \"never\"\n\n\
                 [[gates]]\nname = \"frozen-candidate\"\n\
                 cmd = 'git grep -q \"edited: 0\" -- agent-output.txt'\n",
        ),
    );
    let base = git_in(&repo, &["rev-parse", "HEAD"]).trim().to_owned();
    let marker = candidate_mutation_marker(&repo);
    let _ = fs::remove_file(&marker);
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    opts.after_candidate_capture = Some(mutate_index_after_candidate_capture);
    let source = fake(Effect::FrozenCandidate);

    let report = run_with(&opts, &source).expect("the frozen candidate remains authoritative");
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:?}");
    assert_eq!(report.gates, ["frozen-candidate"]);

    let capture: Vec<_> = fs::read_to_string(&marker)
        .expect("the post-capture mutation hook ran")
        .lines()
        .map(str::to_owned)
        .collect();
    assert_eq!(capture.len(), 3, "capture marker: {capture:?}");
    assert_eq!(capture[0], base, "captured parent is the attempt parent");
    assert_ne!(
        capture[1], capture[2],
        "the mutable index really changed after capture"
    );

    let logged = events_of(&repo, &report.run_id);
    let prepared = logged
        .iter()
        .find_map(|event| match &event.body {
            EventBody::AttemptFinished {
                prepared_commit, ..
            } => prepared_commit.as_deref().cloned(),
            _ => None,
        })
        .expect("successful settlement records its prepared object");
    assert_eq!(
        prepared.branch_ref,
        format!("refs/heads/{}", report.branch),
        "the durable settlement owns the exact run ref"
    );
    assert_eq!(prepared.parent_sha, capture[0]);
    assert_eq!(prepared.tree_sha, capture[1]);
    assert_ne!(prepared.tree_sha, capture[2]);

    let review_snapshots = source.adapter.review_snapshots();
    assert_eq!(review_snapshots.len(), 1, "one reviewer snapshot");
    assert_eq!(review_snapshots[0].0, prepared.tree_sha);
    assert_eq!(
        review_snapshots[0].1.replace("\r\n", "\n"),
        "edited: 0\n",
        "review sees the captured tree, not the later staged mutation"
    );

    let committed = logged
        .iter()
        .find_map(|event| match &event.body {
            EventBody::TaskCommitted { data, .. } => Some(data),
            _ => None,
        })
        .expect("task_committed follows the prepared settlement");
    let head = git_in(&repo, &["rev-parse", "HEAD"]).trim().to_owned();
    let head_tree = git_in(&repo, &["rev-parse", "HEAD^{tree}"])
        .trim()
        .to_owned();
    assert_eq!(head, prepared.commit_sha);
    assert_eq!(committed.sha, prepared.commit_sha);
    assert_eq!(head_tree, prepared.tree_sha);
    assert_eq!(
        git_in(&repo, &["show", "HEAD:agent-output.txt"]),
        "edited: 0\n",
        "the staged post-capture mutation is never published"
    );
}

#[test]
fn an_oversized_review_diff_is_settled_once_before_the_task_parks() {
    let (_tree, repo) = temp_engine_repo("oversizedreviewsettlement");
    seed(
        &repo,
        "## Generate the large fixture\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some(
            "[interaction]\nmode = \"never\"\n\n\
                 [routing]\nimplement = { chain = [\"small\"], attempts_per = 3 }\n",
        ),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = fake(Effect::LargeEdit);
    let report = run_with(&opts, &source).expect("policy failure is a settled run outcome");

    assert_eq!(report.outcome(), RunOutcome::Parked, "{report:?}");
    let task_report = task(&report, "t1");
    assert_eq!(
        task_report.attempts.len(),
        1,
        "the policy boundary is not retried"
    );
    let attempt = &task_report.attempts[0];
    let failure = attempt.failure.as_ref().expect("settled policy failure");
    assert_eq!(failure.kind, FailureKind::ReviewInputTooLarge);
    assert_eq!(failure.origin, FailureOrigin::Reviewer);
    assert_eq!(attempt.cost_usd, Some(0.01), "worker spend is retained");
    assert_eq!(attempt.session_id.as_deref(), Some("s0"));
    assert!(attempt.usage.is_some(), "worker usage is retained");
    assert!(attempt.reviews.is_empty(), "no reviewer was dispatched");
    assert_eq!(source.adapter.reviews_run(), 0);

    let logged = events_of(&repo, &report.run_id);
    assert_eq!(
        logged
            .iter()
            .filter(|event| matches!(event.body, EventBody::AttemptFinished { .. }))
            .count(),
        1,
        "the attempt has a terminal ledger event"
    );
    let parking = logged.iter().find_map(|event| match &event.body {
        EventBody::AttemptFinished { parking, .. } => parking.as_deref(),
        _ => None,
    });
    assert!(
        parking.is_some(),
        "the settlement atomically carries its parking question"
    );
    assert!(
        !logged.iter().any(|event| matches!(
            event.body,
            EventBody::QuestionRaised { .. } | EventBody::TaskParked { .. }
        )),
        "policy parking must not reopen a crash window with follow-up events"
    );
    assert!(
        !logged
            .iter()
            .any(|event| matches!(event.body, EventBody::AttemptInterrupted { .. })),
        "replay must never invent an interruption for the settled refusal"
    );
    assert!(
        git_in(&repo, &["status", "--porcelain"]).trim().is_empty(),
        "parking cleans the unreviewed oversized diff"
    );
    let question = report.questions.first().expect("scope question");
    assert_eq!(question.question.kind, QuestionKind::Unblock);
    assert!(question.question.context.contains("smaller diff"));
    assert!(question.question.context.contains("starting a new run"));
    assert!(
        !question.question.context.contains("chain is spent")
            && !question.question.context.contains("all failed"),
        "policy parking must not pretend the escalation chain was exhausted: {}",
        question.question.context
    );

    let paths = paths_of(&repo, &report.run_id);
    truncate_log_after(&paths, "attempt_finished");
    fs::write(repo.join("crash-residue.txt"), "unreviewed\n").expect("crash residue");
    let retry = fake(Effect::EditFile);
    let resumed =
        resume_with(&resume_options(&repo, &report.run_id), &retry).expect("resume parks");
    assert_eq!(resumed.outcome(), RunOutcome::Parked, "{resumed:?}");
    assert!(
        retry.adapter.runs().is_empty(),
        "resume paid for the oversized attempt again"
    );
    assert_eq!(task(&resumed, "t1").attempts.len(), 1);
    assert_eq!(resumed.questions.len(), 1);
    assert!(
        git_in(&repo, &["status", "--porcelain"]).trim().is_empty(),
        "resume did not discard crash residue"
    );
}

#[test]
fn opaque_review_input_has_distinct_failure_and_remediation() {
    let (_tree, repo) = temp_engine_repo("opaquereviewsettlement");
    seed(
        &repo,
        "## Generate an opaque artifact\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some(
            "[interaction]\nmode = \"never\"\n\n\
                 [routing]\nimplement = { chain = [\"small\"], attempts_per = 3 }\n",
        ),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = fake(Effect::OpaqueEdit);
    let report = run_with(&opts, &source).expect("opaque evidence parks fail-closed");

    assert_eq!(report.outcome(), RunOutcome::Parked, "{report:?}");
    let attempt = &task(&report, "t1").attempts[0];
    assert_eq!(
        attempt.failure.as_ref().map(|failure| failure.kind),
        Some(FailureKind::ReviewInputOpaque)
    );
    assert_eq!(source.adapter.reviews_run(), 0);
    let context = &report.questions[0].question.context;
    assert!(context.contains("hides changed content"), "{context}");
    assert!(!context.contains("smaller diff"), "{context}");
}

#[test]
fn opaque_test_task_parks_before_test_provenance_retry() {
    let (_tree, repo) = temp_engine_repo("opaquetestprovenance");
    seed(
        &repo,
        "## Add the regression\n<!-- upstroke: id=t1 kind=test depends= -->\n",
        Some(
            "[interaction]\nmode = \"never\"\n\n\
                 [routing]\ntest = { chain = [\"small\"], attempts_per = 3 }\n",
        ),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = fake(Effect::OpaqueEdit);
    let report = run_with(&opts, &source).expect("opaque evidence parks fail-closed");

    assert_eq!(report.outcome(), RunOutcome::Parked, "{report:?}");
    let task = task(&report, "t1");
    assert_eq!(task.attempts.len(), 1, "opaque evidence is not retried");
    assert_eq!(
        task.attempts[0]
            .failure
            .as_ref()
            .map(|failure| failure.kind),
        Some(FailureKind::ReviewInputOpaque),
        "the intrinsic evidence failure wins over Test provenance"
    );
    assert_eq!(source.adapter.reviews_run(), 0);
    assert!(
        report.questions[0]
            .question
            .context
            .contains("hides changed content")
    );
}

#[test]
fn failed_parking_payload_still_settles_and_cleans_the_attempt() {
    let (_tree, repo) = temp_engine_repo("oversizedreviewquestionwrite");
    seed(
        &repo,
        "## Generate the large fixture\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some(
            "[interaction]\nmode = \"never\"\n\n\
                 [routing]\nimplement = { chain = [\"small\"], attempts_per = 3 }\n",
        ),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = fake(Effect::LargeEditQuestionWriteFailure);
    let error = run_with(&opts, &source).expect_err("question projection must fail");
    assert!(
        error.to_string().contains("questions"),
        "wrong failure surfaced: {error}"
    );

    let run_id = rundir::latest_run(&repo).expect("failed run remains resumable");
    let logged = events_of(&repo, &run_id);
    let parking = logged.iter().find_map(|event| match &event.body {
        EventBody::AttemptFinished { parking, .. } => parking.as_deref(),
        _ => None,
    });
    assert!(
        parking.is_some(),
        "the event must retain parking even when its JSON projection fails"
    );
    assert_eq!(
        logged
            .iter()
            .filter(|event| matches!(event.body, EventBody::AttemptFinished { .. }))
            .count(),
        1,
        "the paid attempt must settle exactly once"
    );
    assert!(
        git_in(&repo, &["status", "--porcelain"]).trim().is_empty(),
        "a failed question write leaked the oversized unreviewed diff"
    );

    let paths = paths_of(&repo, &run_id);
    fs::remove_file(paths.questions()).expect("remove injected blocker");
    fs::create_dir(paths.questions()).expect("restore questions directory");
    let retry = fake(Effect::EditFile);
    let resumed = resume_with(&resume_options(&repo, &run_id), &retry)
        .expect("resume repairs the projection and remains parked");
    assert_eq!(resumed.outcome(), RunOutcome::Parked, "{resumed:?}");
    assert!(
        retry.adapter.runs().is_empty(),
        "resume paid for an already-settled attempt"
    );
    assert_eq!(task(&resumed, "t1").attempts.len(), 1);
    let question = resumed.questions.first().expect("restored question");
    assert!(
        interaction::answer_path(&paths.questions(), &question.question.id).exists(),
        "resume did not rematerialize the authoritative question"
    );
}

#[test]
fn dirty_tree_is_refused() {
    let (_tree, repo) = temp_engine_repo("dirty");
    fs::write(repo.join("stray.txt"), "uncommitted\n").expect("stray");
    let source = fake(Effect::EditFile);
    let err = run_with(&options(&repo), &source).expect_err("must refuse");
    assert!(err.to_string().contains("not clean"), "got: {err}");
    let branch = git_in(&repo, &["rev-parse", "--abbrev-ref", "HEAD"]);
    assert_eq!(branch.trim(), "main", "no run branch created");
}

#[test]
fn sparse_checkout_preflight_refusal_leaves_worktree_clean() {
    let (_tree, repo) = temp_engine_repo("sparse-worker-preflight");
    git_in(&repo, &["update-index", "--skip-worktree", "README.md"]);
    let source = fake(Effect::EditFile);

    let error = run_with(&options(&repo), &source)
        .expect_err("incomplete materialization must be refused")
        .to_string();
    assert!(error.contains("sparse checkout is active"), "{error}");
    assert!(
        source.adapter.runs().is_empty(),
        "a worker was dispatched before sparse-checkout refusal"
    );
    assert_eq!(
        git_in(&repo, &["rev-parse", "--abbrev-ref", "HEAD"]).trim(),
        "main",
        "preflight refusal must not create or switch a run branch"
    );
    git_in(&repo, &["update-index", "--no-skip-worktree", "README.md"]);
    let worktree_git_dir = Workspace::open(&repo)
        .expect("open worktree")
        .worktree_git_dir()
        .expect("resolve private git dir");
    assert!(
        worktree_git_dir.join("upstroke-worktree.lock").exists(),
        "the regression must exercise acquisition of the private worktree lease"
    );
    assert!(
        !repo.join(".upstroke").exists(),
        "a refused preflight must not create working-tree coordinator state"
    );
    assert!(
        git_in(&repo, &["status", "--porcelain", "--untracked-files=all"])
            .trim()
            .is_empty(),
        "a refused preflight left coordinator state visible to Git"
    );
}

#[test]
fn passing_configured_gates_commit_and_are_reported() {
    let (_tree, repo) = temp_engine_repo("gatepass");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 depends= -->\n",
        Some("[[gates]]\nname = \"version\"\ncmd = \"git --version\"\n"),
    );

    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = fake(Effect::EditFile);
    let report = run_with(&opts, &source).expect("run");
    assert_eq!(report.outcome(), RunOutcome::Complete, "report: {report:?}");
    assert_eq!(report.gates, ["version"]);
    assert!(report.gates_from_config);
    assert!(report.render().contains("gates: version [from config]"));
}

#[test]
fn ignored_worker_input_cannot_make_a_gate_pass() {
    let (_tree, repo) = temp_engine_repo("ignored-gate-input");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 depends= -->\n",
        Some(
            "[interaction]\nmode = \"never\"\n\n\
                 [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n\n\
                 [[gates]]\nname = \"ignored-input\"\ncmd = \"git hash-object ignored.flag\"\n",
        ),
    );

    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = fake(Effect::IgnoredGateInput);
    let report = run_with(&opts, &source).expect("gate failure settles the task");

    assert!(!committed(&report, "t1"), "report: {report:?}");
    assert!(task(&report, "t1").attempts.iter().any(|attempt| {
        attempt
            .failure
            .as_ref()
            .is_some_and(|failure| failure.kind == FailureKind::GateFailed)
    }));
    assert!(
        !repo.join("ignored.flag").exists(),
        "ignored worker-only input was cleaned from the authoritative workspace"
    );
}

#[test]
fn unresolvable_gate_refuses_at_preflight() {
    let (_tree, repo) = temp_engine_repo("gateresolve");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 depends= -->\n",
        Some("[[gates]]\nname = \"ghost\"\ncmd = \"definitely-not-a-real-tool-xyz build\"\n"),
    );

    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = fake(Effect::EditFile);
    let err = run_with(&opts, &source).expect_err("must refuse");
    assert!(err.to_string().contains("not found on PATH"), "got: {err}");
    let branch = git_in(&repo, &["rev-parse", "--abbrev-ref", "HEAD"]);
    assert_eq!(branch.trim(), "main", "refused before branching");
}

#[test]
fn test_task_without_test_code_fails_provenance() {
    let (_tree, repo) = temp_engine_repo("provenance");
    seed(
        &repo,
        "## Test the widget\n<!-- upstroke: id=tt depends= -->\nAdd coverage.\n",
        Some("[routing]\ntest = { chain = [\"small\"], attempts_per = 1 }\n"),
    );

    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = fake(Effect::EditFile);
    let report = run_with(&opts, &source).expect("engine ok");
    let reason = &task(&report, "tt").attempts[0]
        .failure
        .as_ref()
        .expect("provenance should fail")
        .reason;
    assert!(reason.contains("provenance"), "reason: {reason}");
}

#[test]
fn test_task_adding_real_tests_passes_provenance() {
    let (_tree, repo) = temp_engine_repo("provenance-ok");
    seed(
        &repo,
        "## Test the widget\n<!-- upstroke: id=tt depends= -->\n",
        None,
    );

    let source = fake(Effect::EditTest);
    let report = run_with(&options(&repo), &source).expect("engine ok");
    assert_eq!(report.outcome(), RunOutcome::Complete, "report: {report:?}");
    assert!(committed(&report, "tt"));
}

#[test]
fn gate_residue_is_scrubbed_not_committed() {
    let (_tree, repo) = temp_engine_repo("residue");

    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 depends= -->\n",
        Some("[[gates]]\nname = \"leaky\"\ncmd = \"echo residue> residue.txt\"\n"),
    );

    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = fake(Effect::EditFile);
    let report = run_with(&opts, &source).expect("run");
    assert_eq!(report.outcome(), RunOutcome::Complete, "report: {report:?}");
    assert!(!repo.join("residue.txt").exists(), "residue scrubbed");
    assert!(
        git_in(&repo, &["status", "--porcelain"]).trim().is_empty(),
        "clean tree after run"
    );
    let log = git_in(&repo, &["log", "--name-only", "--format=", "main..HEAD"]);
    assert!(!log.contains("residue.txt"), "log: {log}");
}

#[test]
fn the_reviewer_is_read_only_and_bound_to_the_review_tier() {
    let (_tree, repo) = temp_engine_repo("reviewbinding");
    let source = fake(Effect::EditFile);
    let report = run_with(&options(&repo), &source).expect("run");
    let settings = paths_of(&repo, &report.run_id).settings();
    let allow_list = |file: &str| -> Vec<String> {
        let text = fs::read_to_string(settings.join(file)).expect("settings written");
        let value: serde_json::Value = serde_json::from_str(&text).expect("json");
        value["permissions"]["allow"]
            .as_array()
            .expect("allow list")
            .iter()
            .map(|v| v.as_str().unwrap_or_default().to_owned())
            .collect()
    };

    let reviewer = allow_list("00-t1-1-review.json");
    assert_eq!(reviewer, ["Read", "Glob", "Grep"], "read-only, no shell");

    let implementer = allow_list("00-t1-1.json");
    assert!(
        implementer.contains(&"Edit".to_owned()),
        "implementer can edit"
    );

    assert!(
        !settings.starts_with(&repo),
        "settings live outside the workspace: {}",
        settings.display()
    );
}

#[test]
fn review_can_be_switched_off_explicitly() {
    let (_tree, repo) = temp_engine_repo("noreview");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 depends= -->\n",
        Some("[routing]\nreview = { enabled = false }\n"),
    );

    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));

    let source = source(vec![Effect::EditFile], vec![ReviewBehavior::Fail]);
    let report = run_with(&opts, &source).expect("run");
    assert_eq!(report.outcome(), RunOutcome::Complete, "report: {report:?}");
    assert!(report.tasks[0].review_models.is_empty());
    assert!(report.tasks[0].review_cost_usd.is_none());
}

#[test]
fn reviewer_spend_is_attributed_separately() {
    let (_tree, repo) = temp_engine_repo("reviewcost");
    let source = fake(Effect::EditFile);
    let report = run_with(&options(&repo), &source).expect("run");
    let t1 = task(&report, "t1");
    assert_eq!(t1.cost_usd, Some(0.01), "implementer's own spend");
    assert_eq!(t1.review_cost_usd, Some(0.05), "reviewer's, kept apart");
    assert_eq!(t1.review_models, ["claude-opus-5"]);
    assert!((t1.total_cost_usd().expect("both") - 0.06).abs() < 1e-9);
    let rendered = report.render();
    assert!(rendered.contains("+ review claude-opus-5"), "{rendered}");
}

const FRONTIER_AUTH_PLAN: &str = "## Rotate the signing key\n\
         <!-- upstroke: id=t1 kind=implement depends= tier=frontier paths=src/auth/** -->\n\
         Rotate it.\n";

const SECOND_OPINION_CONFIG: &str = "[routing]\n\
         implement = { chain = [\"frontier\"], attempts_per = 1 }\n\n\
         [[routing.overrides]]\n\
         paths = [\"src/auth/**\"]\n\
         second_opinion = \"different-vendor\"\n";

const FRONTIER_ONLY_CONFIG: &str =
    "[routing]\nimplement = { chain = [\"frontier\"], attempts_per = 1 }\n";

fn cross_vendor_opts(repo: &Path) -> RunOptions {
    let mut opts = options(repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    opts
}

#[test]
fn a_second_opinion_runs_a_second_family_and_leaves_the_primary_alone() {
    let (_tree, repo) = temp_engine_repo("secondopinion");
    seed(&repo, FRONTIER_AUTH_PLAN, Some(SECOND_OPINION_CONFIG));
    let source = cross_vendor(
        vec![Effect::EditFile],
        vec![ReviewBehavior::Pass],
        vec![ReviewBehavior::Pass],
    );
    let report = run_with(&cross_vendor_opts(&repo), &source).expect("run");

    assert!(committed(&report, "t1"), "both passed: {report:?}");
    let t1 = task(&report, "t1");
    assert_eq!(
        t1.review_models,
        ["claude-opus-5", "gpt-5.3-codex"],
        "one pass per family, primary first"
    );
    assert_eq!(t1.model, "claude-opus-5", "written by the frontier model");
    assert_eq!(source.adapter.reviews_run(), 1);
    assert_eq!(source.copilot().reviews_run(), 1);

    assert_eq!(t1.review_cost_usd, Some(0.10), "0.05 per pass");
    assert_eq!(t1.cost_usd, Some(0.01), "implementer's own");
    let rendered = report.render();
    assert!(
        rendered.contains("+ review claude-opus-5, gpt-5.3-codex"),
        "{rendered}"
    );
}

#[test]
fn a_second_opinion_that_fails_fails_the_attempt() {
    let (_tree, repo) = temp_engine_repo("secondopinionfail");
    seed(&repo, FRONTIER_AUTH_PLAN, Some(SECOND_OPINION_CONFIG));
    let source = cross_vendor(
        vec![Effect::EditFile],
        vec![ReviewBehavior::Pass],
        vec![ReviewBehavior::Fail],
    );
    let report = run_with(&cross_vendor_opts(&repo), &source).expect("run");

    assert!(!committed(&report, "t1"), "a rejected change cannot commit");
    let t1 = task(&report, "t1");
    let last = t1.attempts.last().expect("an attempt ran");
    assert_eq!(
        last.failure.as_ref().map(|f| f.kind),
        Some(FailureKind::ReviewFailed)
    );
    assert_eq!(
        last.reviews.iter().map(|r| r.outcome).collect::<Vec<_>>(),
        [
            events::ReviewPassOutcome::Passed,
            events::ReviewPassOutcome::Failed
        ],
        "the record says which pass objected, and that it really judged"
    );
    assert_eq!(last.reviews[1].agent, "copilot");
}

#[test]
fn a_failing_first_pass_never_spends_the_second_reviewer() {
    let (_tree, repo) = temp_engine_repo("shortcircuit");
    seed(&repo, FRONTIER_AUTH_PLAN, Some(SECOND_OPINION_CONFIG));
    let source = cross_vendor(
        vec![Effect::EditFile],
        vec![ReviewBehavior::Fail],
        vec![ReviewBehavior::Pass],
    );
    let report = run_with(&cross_vendor_opts(&repo), &source).expect("run");

    assert!(!committed(&report, "t1"));
    assert_eq!(source.adapter.reviews_run(), 1);
    assert_eq!(
        source.copilot().reviews_run(),
        0,
        "the second vendor was never asked"
    );
    let last = task(&report, "t1").attempts.last().expect("attempt");
    assert_eq!(last.reviews.len(), 1, "only what actually ran is recorded");
}

#[test]
fn a_frontier_task_is_not_reviewed_by_the_model_that_wrote_it() {
    let (_tree, repo) = temp_engine_repo("selfreview");
    seed(&repo, FRONTIER_AUTH_PLAN, Some(FRONTIER_ONLY_CONFIG));

    let source = cross_vendor(
        vec![Effect::EditFile],
        vec![ReviewBehavior::Fail],
        vec![ReviewBehavior::Pass],
    );
    let report = run_with(&cross_vendor_opts(&repo), &source).expect("run");

    assert!(committed(&report, "t1"), "{report:?}");
    assert_eq!(task(&report, "t1").review_models, ["gpt-5.3-codex"]);
    assert_eq!(source.adapter.reviews_run(), 0, "never judged its own work");
    assert_eq!(source.copilot().reviews_run(), 1);
}

#[test]
fn a_lower_rung_keeps_the_frontier_reviewer() {
    let (_tree, repo) = temp_engine_repo("noneedtorebind");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"mid\"], attempts_per = 1 }\n"),
    );
    let source = cross_vendor(
        vec![Effect::EditFile],
        vec![ReviewBehavior::Pass],
        vec![ReviewBehavior::Pass],
    );
    let report = run_with(&cross_vendor_opts(&repo), &source).expect("run");

    assert_eq!(task(&report, "t1").model, "claude-sonnet-5");
    assert_eq!(task(&report, "t1").review_models, ["claude-opus-5"]);
    assert_eq!(source.copilot().reviews_run(), 0);
}

#[test]
fn a_configured_second_opinion_with_no_second_family_refuses_before_spending() {
    let (_tree, repo) = temp_engine_repo("nosecondfamily");
    seed(&repo, FRONTIER_AUTH_PLAN, Some(SECOND_OPINION_CONFIG));
    let source = source(vec![Effect::EditFile], vec![ReviewBehavior::Pass]);
    let error = run_with(&cross_vendor_opts(&repo), &source)
        .expect_err("a promised reviewer that cannot exist must stop the run");
    let message = error.to_string();
    assert!(message.contains("t1"), "names the task: {message}");
    assert!(
        message.contains("src/auth/**"),
        "names the override: {message}"
    );
    assert!(
        message.contains("second opinion"),
        "says what is missing: {message}"
    );
    assert_eq!(source.adapter.runs().len(), 0, "nothing was spent");
}

#[test]
fn without_a_second_vendor_self_review_warns_rather_than_refusing() {
    let (_tree, repo) = temp_engine_repo("selfreviewwarn");
    seed(&repo, FRONTIER_AUTH_PLAN, Some(FRONTIER_ONLY_CONFIG));
    let source = source(vec![Effect::EditFile], vec![ReviewBehavior::Pass]);
    let report = run_with(&cross_vendor_opts(&repo), &source).expect("run still works");

    assert!(committed(&report, "t1"));
    assert_eq!(task(&report, "t1").review_models, ["claude-opus-5"]);
    assert!(
        report
            .warnings
            .iter()
            .any(|w| w.contains("t1") && w.contains("also the reviewer")),
        "warnings: {:?}",
        report.warnings
    );
}

#[test]
fn an_unprobeable_cross_family_reviewer_downgrades_instead_of_halting() {
    let (_tree, repo) = temp_engine_repo("brokencopilot");
    seed(&repo, FRONTIER_AUTH_PLAN, Some(FRONTIER_ONLY_CONFIG));
    let source = FakeSource {
        adapter: FakeAdapter::new(vec![Effect::EditFile], vec![ReviewBehavior::Pass]),
        copilot: Some(FakeAdapter::copilot(vec![ReviewBehavior::Pass]).broken("not logged in")),
    };
    let report =
        run_with(&cross_vendor_opts(&repo), &source).expect("a broken upgrade is not a broken run");

    assert!(committed(&report, "t1"));
    assert_eq!(
        task(&report, "t1").review_models,
        ["claude-opus-5"],
        "fell back to same-model review"
    );
    assert!(
        report
            .warnings
            .iter()
            .any(|w| w.contains("not logged in") && w.contains("same-model review")),
        "warnings: {:?}",
        report.warnings
    );

    assert!(
        report
            .warnings
            .iter()
            .any(|w| w.contains("t1") && w.contains("also the reviewer")),
        "warnings: {:?}",
        report.warnings
    );
}

#[test]
fn the_same_broken_reviewer_is_fatal_when_the_config_asked_for_it() {
    let (_tree, repo) = temp_engine_repo("brokenrequired");
    seed(&repo, FRONTIER_AUTH_PLAN, Some(SECOND_OPINION_CONFIG));
    let source = FakeSource {
        adapter: FakeAdapter::new(vec![Effect::EditFile], vec![ReviewBehavior::Pass]),
        copilot: Some(FakeAdapter::copilot(vec![ReviewBehavior::Pass]).broken("not logged in")),
    };
    let error = run_with(&cross_vendor_opts(&repo), &source)
        .expect_err("a required reviewer that cannot run stops the run");
    assert!(error.to_string().contains("not logged in"), "got: {error}");
}

#[test]
fn a_resume_keeps_the_reviewers_the_run_started_with() {
    let (_tree, repo) = temp_engine_repo("resumereviewers");
    seed(
        &repo,
        "## Rotate the signing key\n\
             <!-- upstroke: id=t1 kind=implement depends= tier=frontier -->\n",
        Some(
            "[interaction]\nmode = \"never\"\n\n\
                 [routing]\nimplement = { chain = [\"frontier\"], attempts_per = 1 }\n",
        ),
    );

    let source = source(vec![Effect::NoEdit], vec![ReviewBehavior::Pass]);
    let first = run_with(&cross_vendor_opts(&repo), &source).expect("run");
    assert!(
        matches!(task(&first, "t1").status, TaskRunStatus::Parked { .. }),
        "{first:?}"
    );

    let paths = paths_of(&repo, &first.run_id);
    let recorded = {
        let mut warnings = Vec::new();
        let events = events::read_all(&paths.events(), &mut warnings).expect("log");
        events::started_of(&events, &paths.events())
            .expect("run_started")
            .reviews
            .clone()
    };
    let recorded = recorded.expect("step 9 records who reviews");
    assert_eq!(
        recorded.alternative, None,
        "there was nothing to rebind to when this run started"
    );
    assert_eq!(recorded.pass_timeout_secs, Some(5400));

    fs::write(
        repo.join("upstroke.toml"),
        "[interaction]\nmode = \"never\"\n\n\
             [routing]\nimplement = { chain = [\"frontier\"], attempts_per = 1 }\n\
             review = { timeout_secs = 60 }\n",
    )
    .expect("edit only the future review timeout");

    crate::answer::answer(
        &repo,
        &first.questions[0].question.id.to_string(),
        crate::answer::Reply::Text("put the key in src/auth/keys.rs".to_owned()),
    )
    .expect("answer");

    let later = cross_vendor(
        vec![Effect::EditFile],
        vec![ReviewBehavior::Pass],
        vec![ReviewBehavior::Pass],
    );
    let resumed =
        resume_with(&resume_options(&repo, &first.run_id), &later).expect("resume continues");

    assert!(committed(&resumed, "t1"), "{resumed:?}");
    assert_eq!(
        task(&resumed, "t1").review_models,
        ["claude-opus-5"],
        "the recorded reviewer judged the resumed attempt"
    );
    assert_eq!(
        later.copilot().reviews_run(),
        0,
        "a CLI installed since the run began must not become its judge"
    );
    let warning = resumed
        .warnings
        .iter()
        .find(|warning| warning.contains("review pass timeout"))
        .unwrap_or_else(|| panic!("no timeout-difference warning: {:?}", resumed.warnings));
    assert!(warning.contains("60s"), "{warning}");
    assert!(warning.contains("5400s"), "{warning}");
    assert!(warning.contains("Start a new run"), "{warning}");
}

#[test]
fn resume_runs_with_the_effort_policy_the_run_recorded_not_todays_config() {
    let original = "[interaction]\nmode = \"never\"\n\n\
                        [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n\n\
                        [routing.effort]\nimplementation = \"xhigh\"\nreview = \"max\"\n";
    let (_tree, repo, run_id) = parked_run_with_config("resumeeffort", original);
    fs::write(
        repo.join("upstroke.toml"),
        "[interaction]\nmode = \"never\"\n\n\
             [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n\n\
             [routing.effort]\nimplementation = \"low\"\nreview = \"high\"\n",
    )
    .expect("edit effort only");

    let resumed = resume_answering(&repo, &run_id, Effect::EditFile);
    assert_eq!(resumed.outcome(), RunOutcome::Complete, "{resumed:?}");
    let logged = events_of(&repo, &run_id);
    let resumed_worker = logged
        .iter()
        .filter_map(|event| match &event.body {
            EventBody::AttemptStarted { data, .. } => Some(data),
            _ => None,
        })
        .next_back()
        .expect("resumed worker start");
    assert_eq!(resumed_worker.effort, Some(Effort::XHigh));
    let resumed_reviews = logged
        .iter()
        .filter_map(|event| match &event.body {
            EventBody::AttemptFinished { data, .. } if !data.reviews.is_empty() => {
                Some(&data.reviews)
            }
            _ => None,
        })
        .next_back()
        .expect("resumed review records");
    assert!(
        resumed_reviews
            .iter()
            .all(|review| review.effort == Some(Effort::Max)),
        "every review pass keeps max: {resumed_reviews:?}"
    );
    let warning = resumed
        .warnings
        .iter()
        .find(|warning| warning.contains("today's effort policy"))
        .unwrap_or_else(|| panic!("no effort difference warning: {:?}", resumed.warnings));
    assert!(warning.contains("implementation small=low"), "{warning}");
    assert!(warning.contains("implementation small=xhigh"), "{warning}");
    assert!(warning.contains("review=max"), "{warning}");
    assert!(warning.contains("Start a new run"), "{warning}");
}

#[test]
fn resume_restores_the_recorded_worker_binding_before_preflight() {
    let original = "[interaction]\nmode = \"never\"\n\n\
                        [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n";
    let (_tree, repo, run_id) = parked_run_with_config("resumebinding", original);
    fs::write(
        repo.join("upstroke.toml"),
        "[interaction]\nmode = \"never\"\n\n\
             [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n\n\
             [[pins]]\ntier = \"small\"\nagent = \"copilot\"\nmodel = \"gpt-5-mini\"\n",
    )
    .expect("edit only the binding");

    let resumed = resume_answering(&repo, &run_id, Effect::EditFile);
    assert_eq!(resumed.outcome(), RunOutcome::Complete, "{resumed:?}");
    let logged = events_of(&repo, &run_id);
    let worker = logged
        .iter()
        .filter_map(|event| match &event.body {
            EventBody::AttemptStarted { data, .. } => Some(data),
            _ => None,
        })
        .next_back()
        .expect("resumed worker");
    assert_eq!(worker.agent, "claude-code");
    assert_eq!(worker.model, "claude-haiku-4-5");
    assert_eq!(
        worker.selection_origin,
        Some(events::SelectionOrigin::Auto),
        "the recorded absence of a pin is part of the snapshot too"
    );
    assert!(
        resumed
            .warnings
            .iter()
            .any(|warning| warning.contains("today's worker bindings")
                && warning.contains("gpt-5-mini")
                && warning.contains("claude-haiku-4-5")),
        "binding difference warning: {:?}",
        resumed.warnings
    );
}

#[test]
fn the_resume_that_rederives_an_old_logs_effort_records_it_for_the_next_one() {
    let original = "[interaction]\nmode = \"never\"\n\n\
                        [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n\n\
                        [routing.effort]\nimplementation = \"xhigh\"\nreview = \"max\"\n";
    let (_tree, repo, run_id) = parked_run_with_config("oldlogeffort", original);
    let paths = paths_of(&repo, &run_id);
    rewrite_run_started_as_schema_one(&paths, &["effort_policy"]);

    fs::write(
        repo.join("upstroke.toml"),
        "[interaction]\nmode = \"never\"\n\n\
             [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n\n\
             [routing.effort]\nimplementation = \"high\"\nreview = \"xhigh\"\n",
    )
    .expect("first derived policy");
    let first = resume_answering(&repo, &run_id, Effect::NoEdit);
    assert_eq!(first.outcome(), RunOutcome::Parked, "{first:?}");
    assert!(
        first
            .warnings
            .iter()
            .any(|warning| warning.contains("predates the effort-policy record")),
        "legacy warning: {:?}",
        first.warnings
    );
    let established = ResolvedEffortPolicy {
        small: Effort::High,
        mid: Effort::High,
        frontier: Effort::High,
        review: Effort::XHigh,
    };
    assert_eq!(
        events::recorded_effort_policy(&events_of(&repo, &run_id)),
        Some(established),
        "the first resume writes down what it derived"
    );
    let after_first = events_of(&repo, &run_id);
    assert!(events::recorded_chains(&after_first).is_some());
    assert_eq!(
        after_first
            .iter()
            .filter(|event| matches!(event.body, EventBody::RunSchemaUpgraded { .. }))
            .count(),
        1,
        "the first current-binary resume appends one downgrade barrier"
    );

    fs::write(
        repo.join("upstroke.toml"),
        "[interaction]\nmode = \"never\"\n\n\
             [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n\n\
             [routing.effort]\nimplementation = \"low\"\nreview = \"medium\"\n",
    )
    .expect("later policy");
    let second = resume_answering(&repo, &run_id, Effect::EditFile);
    assert_eq!(second.outcome(), RunOutcome::Complete, "{second:?}");
    let logged = events_of(&repo, &run_id);
    let worker = logged
        .iter()
        .filter_map(|event| match &event.body {
            EventBody::AttemptStarted { data, .. } => Some(data),
            _ => None,
        })
        .next_back()
        .expect("second resumed worker");
    assert_eq!(worker.effort, Some(Effort::High));
    let reviews = logged
        .iter()
        .filter_map(|event| match &event.body {
            EventBody::AttemptFinished { data, .. } if !data.reviews.is_empty() => {
                Some(&data.reviews)
            }
            _ => None,
        })
        .next_back()
        .expect("second resumed reviews");
    assert!(
        reviews
            .iter()
            .all(|review| review.effort == Some(Effort::XHigh)),
        "reviews retain the established legacy policy: {reviews:?}"
    );
    assert!(
        second
            .warnings
            .iter()
            .any(|warning| warning.contains("today's effort policy")),
        "the later edit is reported: {:?}",
        second.warnings
    );
    assert!(
        !second
            .warnings
            .iter()
            .any(|warning| warning.contains("predates the effort-policy record")),
        "the legacy absence was established once: {:?}",
        second.warnings
    );
    assert_eq!(
        events_of(&repo, &run_id)
            .iter()
            .filter(|event| matches!(event.body, EventBody::RunSchemaUpgraded { .. }))
            .count(),
        1,
        "later resumes must not append duplicate schema transitions"
    );
}

#[test]
fn the_resume_that_rederives_an_old_review_plan_records_it_for_the_next_one() {
    let original = "[interaction]\nmode = \"never\"\n\n\
                        [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n";
    let (_tree, repo, run_id) = parked_run_with_config("oldlogreviews", original);
    let paths = paths_of(&repo, &run_id);
    rewrite_run_started_as_schema_two(&paths);
    strip_run_started_field(&paths, "reviews");

    fs::write(
        repo.join("upstroke.toml"),
        "[interaction]\nmode = \"never\"\n\n\
             [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n\
             review = { timeout_secs = 60 }\n",
    )
    .expect("first derived review plan");
    let first = resume_answering(&repo, &run_id, Effect::NoEdit);
    assert_eq!(first.outcome(), RunOutcome::Parked, "{first:?}");
    assert!(
        first
            .warnings
            .iter()
            .any(|warning| warning.contains("predates the review record")),
        "legacy warning: {:?}",
        first.warnings
    );
    let established = events::recorded_reviews(&events_of(&repo, &run_id))
        .cloned()
        .expect("the first resume writes down what it derived");
    assert_eq!(established.pass_timeout_secs, Some(60));

    fs::write(
        repo.join("upstroke.toml"),
        "[interaction]\nmode = \"never\"\n\n\
             [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n\
             review = { timeout_secs = 120 }\n",
    )
    .expect("later review plan");
    let second = resume_answering(&repo, &run_id, Effect::EditFile);
    assert_eq!(second.outcome(), RunOutcome::Complete, "{second:?}");
    assert_eq!(
        events::recorded_reviews(&events_of(&repo, &run_id))
            .expect("record survives")
            .pass_timeout_secs,
        Some(60),
        "a later config edit cannot replace the established plan"
    );
    let warning = second
        .warnings
        .iter()
        .find(|warning| warning.contains("today's review pass timeout"))
        .unwrap_or_else(|| panic!("no timeout drift warning: {:?}", second.warnings));
    assert!(warning.contains("120s"), "{warning}");
    assert!(warning.contains("60s"), "{warning}");
    assert!(
        !second
            .warnings
            .iter()
            .any(|warning| warning.contains("predates the review record")),
        "the legacy absence is established exactly once: {:?}",
        second.warnings
    );
}

#[test]
fn a_schema_two_resume_records_the_complete_review_barrier_before_work() {
    let (_tree, repo, run_id) = parked_run("schema2reviewbarrier");
    let paths = paths_of(&repo, &run_id);
    rewrite_run_started_as_schema_two(&paths);
    fs::write(
        repo.join("upstroke.toml"),
        "[interaction]\nmode = \"never\"\n\n\
             [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n\
             review = { timeout_secs = 47 }\n",
    )
    .expect("first explicit complete-review timeout");

    let resumed = resume_answering(&repo, &run_id, Effect::NoEdit);
    assert_eq!(resumed.outcome(), RunOutcome::Parked, "{resumed:?}");

    let logged = events_of(&repo, &run_id);
    let barrier = logged
        .iter()
        .position(|event| {
            matches!(
                &event.body,
                EventBody::RunSchemaUpgraded { data }
                    if data.from == 2 && data.to == events::SCHEMA_VERSION
            )
        })
        .expect("schema 2 -> 3 downgrade barrier");
    let resumed_attempt = logged
        .iter()
        .enumerate()
        .skip(barrier + 1)
        .find(|(_, event)| matches!(event.body, EventBody::AttemptStarted { .. }))
        .map(|(index, _)| index)
        .expect("resumed attempt after the barrier");
    assert!(
        barrier < resumed_attempt,
        "the old verification contract must be fenced off before work starts"
    );
    let upgraded_reviews = events::recorded_complete_reviews(&logged)
        .expect("schema-3 resume records a complete review plan");
    assert_eq!(upgraded_reviews.pass_timeout_secs, Some(47));
    assert_eq!(upgraded_reviews.enabled, Some(true));
    assert_eq!(
        upgraded_reviews.alternative_available,
        Some(upgraded_reviews.alternative.is_some())
    );
    assert_eq!(upgraded_reviews.second_opinion.len(), 1);

    fs::write(
        repo.join("upstroke.toml"),
        "[interaction]\nmode = \"never\"\n\n\
             [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n\
             review = { timeout_secs = 83 }\n",
    )
    .expect("later configured timeout");
    let second = resume_answering(&repo, &run_id, Effect::EditFile);
    assert_eq!(second.outcome(), RunOutcome::Complete, "{second:?}");
    assert_eq!(
        events::recorded_reviews(&events_of(&repo, &run_id))
            .expect("upgraded review plan survives")
            .pass_timeout_secs,
        Some(47),
        "a later binary/config default cannot reinterpret the upgraded timeout"
    );
    assert!(
        second.warnings.iter().any(|warning| {
            warning.contains("today's review pass timeout")
                && warning.contains("83s")
                && warning.contains("47s")
        }),
        "timeout drift warning: {:?}",
        second.warnings
    );
}

#[test]
fn max_parallel_above_one_refuses_before_the_run_touches_the_workspace() {
    let (_tree, repo) = temp_engine_repo("maxparallelrefusal");
    seed(
        &repo,
        "## Doomed\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some(
            "[engine]\nmax_parallel = 3\n\n\
             [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n",
        ),
    );
    let head_before = git_in(&repo, &["rev-parse", "HEAD"]);
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = fake(Effect::EditFile);

    let error = run_with(&opts, &source).expect_err("a refused ceiling must not start a run");
    assert!(error.to_string().contains("max_parallel = 3"), "{error}");

    assert!(
        source.adapter.runs().is_empty(),
        "nothing may be spawned, let alone paid for"
    );
    assert!(
        rundir::list_runs(&repo).is_empty(),
        "no run directory: {:?}",
        rundir::list_runs(&repo)
    );
    assert_eq!(
        git_in(&repo, &["branch", "--list", "upstroke/run-*"]),
        "",
        "no run branch"
    );
    assert_eq!(git_in(&repo, &["rev-parse", "HEAD"]), head_before);
    assert_eq!(
        git_in(&repo, &["status", "--porcelain"]),
        "",
        "working tree untouched"
    );
}

fn worktree_lock_path(repo: &Path) -> PathBuf {
    Workspace::open(repo)
        .expect("workspace")
        .worktree_git_dir()
        .expect("worktree git dir")
        .join("upstroke-worktree.lock")
}

#[test]
fn a_refused_ceiling_beats_the_lease_rather_than_racing_it() {
    let (_tree, repo) = temp_engine_repo("ceilingbeforelease");
    seed(
        &repo,
        "## Doomed\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some(
            "[engine]\nmax_parallel = 3\n\n\
             [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n",
        ),
    );
    let lease = worktree_lock_path(&repo);
    assert!(
        !lease.exists(),
        "the fixture has never taken the lease, so the file cannot exist yet"
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));

    let source = fake(Effect::EditFile);
    let refused = run_with(&opts, &source)
        .expect_err("a ceiling this engine cannot honour must not start a run")
        .to_string();
    assert!(refused.contains("max_parallel = 3"), "{refused}");
    assert!(
        !lease.exists(),
        "the worktree lock file at {} was created before the config was read",
        lease.display()
    );

    let competitor = WorktreeLock::acquire(&repo).expect("a competing holder takes the lease");
    assert!(
        lease.exists(),
        "the competing holder is what creates the file, which is how (a) means anything"
    );
    let contended = run_with(&opts, &source)
        .expect_err("a refused ceiling still refuses while somebody holds the lease")
        .to_string();
    assert!(
        contended.contains("max_parallel = 3"),
        "the config error must win the race it never needed to enter: {contended}"
    );
    assert!(
        !contended.contains("another upstroke process"),
        "lock contention must not be the diagnosis for a config error: {contended}"
    );
    drop(competitor);

    assert!(
        source.adapter.runs().is_empty(),
        "nothing may be spawned, let alone paid for"
    );
    assert!(
        rundir::list_runs(&repo).is_empty(),
        "and no run directory: {:?}",
        rundir::list_runs(&repo)
    );
}

#[test]
fn a_refused_ceiling_beats_both_locks_on_resume() {
    let (_tree, repo, run_id) = parked_run("resumeceilingbeforelocks");
    let paths = paths_of(&repo, &run_id);
    rewrite_run_started_as_schema_two(&paths);
    fs::write(
        repo.join("upstroke.toml"),
        format!("{PARKED_RUN_CONFIG}\n[engine]\nmax_per_agent = 0\n"),
    )
    .expect("today's config");

    let lease = worktree_lock_path(&repo);
    let run_lock = rundir::lock_file(&paths.public);
    for path in [&lease, &run_lock] {
        fs::remove_file(path).unwrap_or_else(|error| {
            panic!("the fixture run left {}: {error}", path.display());
        });
    }

    let refused = resume_err(&repo, &run_id);
    assert!(refused.contains("max_per_agent"), "{refused}");
    assert!(
        !lease.exists(),
        "the worktree lease was taken before the config was read"
    );
    assert!(
        !run_lock.exists(),
        "the run lock was taken before the config was read"
    );

    let competitor = WorktreeLock::acquire(&repo).expect("a competing holder takes the lease");
    let contended = resume_err(&repo, &run_id);
    assert!(
        contended.contains("max_per_agent"),
        "the config error must win the race it never needed to enter: {contended}"
    );
    assert!(
        !contended.contains("another upstroke process"),
        "lock contention must not be the diagnosis for a config error: {contended}"
    );
    drop(competitor);
    assert!(
        !run_lock.exists(),
        "and the run lock stays untaken either way"
    );
}

fn config_with_repairs(repairs: u32) -> String {
    format!(
        "[engine]\nmax_merge_repairs = {repairs}\n\n\
         [routing]\nimplement = {{ chain = [\"small\"], attempts_per = 1 }}\n"
    )
}

#[test]
fn the_analysis_adopted_under_the_lease_is_the_one_its_own_bytes_were_validated_from() {
    let (_tree, repo) = temp_engine_repo("confirmunderlease");
    let config = repo.join("upstroke.toml");
    let mut opts = options(&repo);
    opts.config_path = Some(config.clone());

    fs::write(&config, config_with_repairs(5)).expect("the config before the lease");
    let validated = validate_inputs(&opts, config::EngineLimits::Fresh).expect("pre-lock check");
    fs::write(&config, config_with_repairs(7)).expect("the config at the lease");
    let analysis = validated
        .confirm_under_lease(&opts, config::EngineLimits::Fresh)
        .expect("a valid config is still valid");
    assert_eq!(
        analysis.config.max_merge_repairs, 7,
        "the adopted analysis must describe the bytes the run is holding"
    );

    fs::write(&config, config_with_repairs(5)).expect("the config before the lease");
    let validated = validate_inputs(&opts, config::EngineLimits::Fresh).expect("pre-lock check");
    fs::write(
        &config,
        "[engine]\nmax_parallel = 3\n\n\
         [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n",
    )
    .expect("a ceiling this engine cannot honour");
    let refused = validated
        .confirm_under_lease(&opts, config::EngineLimits::Fresh)
        .expect_err("an unhonourable ceiling must not be adopted because an older file was fine")
        .to_string();
    assert!(refused.contains("max_parallel = 3"), "{refused}");

    fs::write(&config, config_with_repairs(5)).expect("A");
    let validated = validate_inputs(&opts, config::EngineLimits::Fresh).expect("pre-lock check");
    fs::write(&config, config_with_repairs(9)).expect("B");
    fs::write(&config, config_with_repairs(5)).expect("A again");
    let analysis = validated
        .confirm_under_lease(&opts, config::EngineLimits::Fresh)
        .expect("A is what was captured and A is what is there");
    assert_eq!(
        analysis.config.max_merge_repairs, 5,
        "B was adopted from an excursion neither capture can see"
    );
}

#[test]
fn the_gate_derivation_is_taken_under_the_lease_not_carried_over_it() {
    let (_tree, repo) = temp_engine_repo("gatesunderlease");
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    fs::write(opts.config_path.as_ref().expect("config path"), "").expect("an empty config");

    let validated = validate_inputs(&opts, config::EngineLimits::Fresh).expect("pre-lock check");

    fs::write(repo.join("Cargo.toml"), "[package]\nname = \"x\"\n").expect("a rust repo now");
    let analysis = validated
        .confirm_under_lease(&opts, config::EngineLimits::Fresh)
        .expect("a shape change is not a refusal");
    assert_eq!(
        analysis
            .gates
            .iter()
            .map(|gate| gate.name.clone())
            .collect::<Vec<_>>(),
        vec!["check".to_owned(), "test".to_owned()],
        "the gates a run is held to must be derived from the worktree it holds"
    );
}

fn volatile_strings(repo: &Path, run_id: &str) -> Vec<String> {
    let mut volatile = vec![run_id.to_owned()];

    for path in [private_root_for(repo), repo.to_path_buf()] {
        let text = path.to_string_lossy().into_owned();
        volatile.push(text.replace('\\', "/"));
        volatile.push(text);
    }
    volatile
}

fn replace_exact_runs(
    text: &str,
    len: usize,
    token: &str,
    member: impl Fn(char) -> bool,
) -> String {
    let mut out = String::with_capacity(text.len());
    let mut run = String::new();
    let flush = |run: &mut String, out: &mut String| {
        if run.chars().count() == len {
            out.push_str(token);
        } else {
            out.push_str(run);
        }
        run.clear();
    };
    for ch in text.chars() {
        if member(ch) {
            run.push(ch);
            continue;
        }
        flush(&mut run, &mut out);
        out.push(ch);
    }
    flush(&mut run, &mut out);
    out
}

fn canonicalize_json(value: &mut serde_json::Value, volatile: &[String]) {
    match value {
        serde_json::Value::String(text) => {
            let mut canonical = text.clone();
            for needle in volatile {
                canonical = canonical.replace(needle.as_str(), "<volatile>");
            }
            canonical = replace_exact_runs(&canonical, 40, "<sha>", |ch| {
                ch.is_ascii_digit() || ch.is_ascii_lowercase() && ch.is_ascii_hexdigit()
            });
            *text = replace_exact_runs(&canonical, 26, "<ulid>", |ch| {
                ch.is_ascii_digit() || ch.is_ascii_uppercase()
            });
        }
        serde_json::Value::Array(items) => {
            for item in items {
                canonicalize_json(item, volatile);
            }
        }
        serde_json::Value::Object(fields) => {
            for (key, field) in fields.iter_mut() {
                if matches!(key.as_str(), "ts" | "duration_ms" | "duration") {
                    *field = serde_json::Value::String(format!("<{key}>"));
                    continue;
                }
                canonicalize_json(field, volatile);
            }
        }
        _ => {}
    }
}

fn canonical_trace(events: &[events::Event], repo: &Path, run_id: &str) -> Vec<String> {
    let volatile = volatile_strings(repo, run_id);
    events
        .iter()
        .map(|event| {
            let mut value = serde_json::to_value(event).expect("an event serializes");
            canonicalize_json(&mut value, &volatile);
            value.to_string()
        })
        .collect()
}

fn canonical_projection(report: &RunReport, repo: &Path, run_id: &str) -> String {
    let volatile = volatile_strings(repo, run_id);
    let mut value = serde_json::to_value(report).expect("a report serializes");
    value
        .as_object_mut()
        .expect("a report is an object")
        .remove("warnings")
        .expect("a report records its warnings");
    canonicalize_json(&mut value, &volatile);
    value.to_string()
}

const LEGACY_RESUME_LIMITS: &str = "\n[engine]\nmax_parallel = 2\nmax_merge_repairs = 7\n\
                                    max_per_agent = 4\nmax_per_pool = 5\n";

const LEGACY_RESUME_NO_LIMITS: &str = "\n# no [engine] ceilings in this arm\n";

#[derive(Clone, Copy)]
enum LegacyFixture {
    Parked,

    InterruptedAttempt,
}

struct LegacyArm {
    report: RunReport,

    trace: Vec<String>,

    projection: String,

    tree: String,
    events: Vec<events::Event>,
}

fn legacy_resume_pair(tag: &str, fixture: LegacyFixture) -> Vec<LegacyArm> {
    let mut observed = Vec::new();
    for (arm, extra) in [
        ("control", LEGACY_RESUME_NO_LIMITS),
        ("limits", LEGACY_RESUME_LIMITS),
    ] {
        let (_tree, repo, run_id) = parked_run(&format!("legacylimits-{tag}-{arm}"));
        let paths = paths_of(&repo, &run_id);
        rewrite_run_started_as_schema_two(&paths);
        if matches!(fixture, LegacyFixture::InterruptedAttempt) {
            truncate_log_after(&paths, "attempt_started");
        }
        fs::write(
            repo.join("upstroke.toml"),
            format!("{PARKED_RUN_CONFIG}{extra}"),
        )
        .expect("today's config");

        let report = resume_answering(&repo, &run_id, Effect::EditFile);
        let events = events_of(&repo, &run_id);
        observed.push(LegacyArm {
            trace: canonical_trace(&events, &repo, &run_id),
            projection: canonical_projection(&report, &repo, &run_id),
            tree: git_in(&repo, &["rev-parse", "HEAD^{tree}"]),
            events,
            report,
        });
    }
    observed
}

#[test]
fn a_legacy_resume_is_not_reinterpreted_by_the_new_engine_limits() {
    for (fixture, tag) in [
        (LegacyFixture::Parked, "parked"),
        (LegacyFixture::InterruptedAttempt, "interrupted"),
    ] {
        let observed = legacy_resume_pair(tag, fixture);
        let control = &observed[0];
        let limits = &observed[1];

        assert_eq!(
            control.report.outcome(),
            RunOutcome::Complete,
            "the {tag} control resume continues to the end: {:?}",
            control.report
        );
        assert_eq!(
            limits.report.outcome(),
            control.report.outcome(),
            "the new keys must not change how a legacy run ends ({tag})"
        );
        assert_eq!(
            limits.trace, control.trace,
            "nor what it records, nor with what contents, nor in what order ({tag})"
        );
        assert_eq!(
            limits.projection, control.projection,
            "nor what it reports about each task ({tag})"
        );
        assert_eq!(
            limits.tree, control.tree,
            "nor the tree it committed ({tag})"
        );
        assert!(
            !control.tree.is_empty(),
            "the fixture must actually have committed something for that to mean anything"
        );

        let mut open = 0i32;
        let mut peak = 0i32;
        for event in &limits.events {
            match &event.body {
                EventBody::AttemptStarted { .. } => open += 1,

                EventBody::AttemptFinished { .. } | EventBody::AttemptInterrupted { .. } => {
                    open -= 1;
                }
                _ => continue,
            }
            peak = peak.max(open);
        }
        assert_eq!(peak, 1, "the resume ran one attempt at a time ({tag})");
        assert_eq!(open, 0, "and settled every one of them ({tag})");

        for key in [
            "max_parallel",
            "max_merge_repairs",
            "max_per_agent",
            "max_per_pool",
        ] {
            assert!(
                limits
                    .report
                    .warnings
                    .iter()
                    .any(|warning| warning.contains(key) && warning.contains("not acted on")),
                "`{key}` must be reported as unacted-on ({tag}): {:?}",
                limits.report.warnings
            );
            assert!(
                !control
                    .report
                    .warnings
                    .iter()
                    .any(|warning| warning.contains(key)),
                "and only when it was written ({tag}): {:?}",
                control.report.warnings
            );
        }
        assert!(
            limits.report.warnings.iter().any(|warning| {
                warning.contains("max_parallel = 2") && warning.contains("this resume")
            }),
            "and the refused-for-fresh-runs ceiling says which run it is talking about: {:?}",
            limits.report.warnings
        );
    }
}

#[test]
fn schema_two_review_markers_upgrade_independently_of_timeout() {
    let (_tree, repo, run_id) = parked_run("schema2reviewmarkers");
    let paths = paths_of(&repo, &run_id);
    let recorded_timeout = events::recorded_reviews(&events_of(&repo, &run_id))
        .and_then(|plan| plan.pass_timeout_secs)
        .expect("current run records a timeout");
    rewrite_run_started_as_schema_two_missing_review_fields(
        &paths,
        &["enabled", "alternative_available"],
    );

    let first = resume_answering(&repo, &run_id, Effect::NoEdit);
    assert_eq!(first.outcome(), RunOutcome::Parked, "{first:?}");
    assert!(
        first
            .warnings
            .iter()
            .any(|warning| warning.contains("explicit reviewer-identity markers")),
        "marker-upgrade warning: {:?}",
        first.warnings
    );
    let upgraded = events::recorded_complete_reviews(&events_of(&repo, &run_id))
        .cloned()
        .expect("schema-3 resume records the complete identity");
    assert_eq!(upgraded.pass_timeout_secs, Some(recorded_timeout));
    assert_eq!(upgraded.enabled, Some(upgraded.primary.is_some()));
    assert_eq!(
        upgraded.alternative_available,
        Some(upgraded.alternative.is_some())
    );

    let second = resume_answering(&repo, &run_id, Effect::EditFile);
    assert_eq!(second.outcome(), RunOutcome::Complete, "{second:?}");
    assert_eq!(
        events::recorded_complete_reviews(&events_of(&repo, &run_id)),
        Some(&upgraded),
        "the next replay accepts and preserves the explicit markers"
    );
}

#[test]
fn schema_two_inconsistent_review_identity_is_refused_before_upgrade_and_spend() {
    let (_tree, repo, run_id) = parked_run("schema2badreviewidentity");
    let paths = paths_of(&repo, &run_id);
    let text = fs::read_to_string(paths.events()).expect("log");
    let mut rewritten = false;
    let lines: Vec<String> = text
        .lines()
        .map(|line| {
            let mut value: serde_json::Value = serde_json::from_str(line).expect("event json");
            if value.get("event").and_then(serde_json::Value::as_str) == Some("run_started") {
                let data = value
                    .get_mut("data")
                    .and_then(serde_json::Value::as_object_mut)
                    .expect("run_started data");
                data.insert("schema".to_owned(), serde_json::Value::from(2));
                let reviews = data
                    .get_mut("reviews")
                    .and_then(serde_json::Value::as_object_mut)
                    .expect("review plan");
                reviews.insert("enabled".to_owned(), serde_json::Value::Bool(true));
                reviews.insert("primary".to_owned(), serde_json::Value::Null);
                rewritten = true;
            }
            value.to_string()
        })
        .collect();
    assert!(rewritten);
    fs::write(paths.events(), format!("{}\n", lines.join("\n"))).expect("rewrite");

    let question = events_of(&repo, &run_id)
        .iter()
        .find_map(|event| match &event.body {
            EventBody::QuestionRaised { data, .. } => Some(data.question.id.to_string()),
            EventBody::AttemptFinished {
                parking: Some(parking),
                ..
            } => Some(parking.question.id.to_string()),
            _ => None,
        })
        .expect("parked question");
    crate::answer::answer(
        &repo,
        &question,
        crate::answer::Reply::Text("continue".to_owned()),
    )
    .expect("answer");
    let source = fake(Effect::EditFile);
    let error = resume_harness_inner(
        &resume_options(&repo, &run_id),
        &Harness {
            adapters: &source,
            answers: None,
            sleeper: None,
        },
    )
    .expect_err("an inconsistent inherited review identity must fail closed");
    assert!(
        error
            .to_string()
            .contains("reviews.enabled does not match the recorded primary reviewer"),
        "wrong error: {error}"
    );
    assert!(
        source.adapter.runs().is_empty(),
        "no worker may run under the malformed identity"
    );
    assert!(
        !events_of(&repo, &run_id)
            .iter()
            .any(|event| matches!(event.body, EventBody::RunSchemaUpgraded { .. })),
        "the malformed identity must not be blessed by a schema upgrade"
    );
}

#[test]
fn a_resume_whose_effort_policy_did_not_move_says_nothing_about_it() {
    let config = "[interaction]\nmode = \"never\"\n\n\
                      [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n\n\
                      [routing.effort]\nimplementation = \"xhigh\"\nreview = \"max\"\n";
    let (_tree, repo, run_id) = parked_run_with_config("effortunmoved", config);
    let resumed = resume_answering(&repo, &run_id, Effect::EditFile);
    assert_eq!(resumed.outcome(), RunOutcome::Complete, "{resumed:?}");
    assert!(
        !resumed
            .warnings
            .iter()
            .any(|warning| warning.contains("effort policy") || warning.contains("effort-policy")),
        "an unchanged policy must be silent: {:?}",
        resumed.warnings
    );
}

#[test]
fn a_log_written_before_step_9_still_gets_reviewed_on_resume() {
    let (_tree, repo) = temp_engine_repo("oldlogresume");
    seed(
        &repo,
        "## Rotate the signing key\n\
             <!-- upstroke: id=t1 kind=implement depends= tier=frontier -->\n",
        Some(
            "[interaction]\nmode = \"never\"\n\n\
                 [routing]\nimplement = { chain = [\"frontier\"], attempts_per = 1 }\n",
        ),
    );
    let first_source = source(vec![Effect::NoEdit], vec![ReviewBehavior::Pass]);
    let first = run_with(&cross_vendor_opts(&repo), &first_source).expect("run");

    let paths = paths_of(&repo, &first.run_id);
    rewrite_run_started_as_schema_two(&paths);
    strip_run_started_field(&paths, "reviews");

    crate::answer::answer(
        &repo,
        &first.questions[0].question.id.to_string(),
        crate::answer::Reply::Text("put the key in src/auth/keys.rs".to_owned()),
    )
    .expect("answer");

    let later = source(vec![Effect::EditFile], vec![ReviewBehavior::Fail]);
    let resumed =
        resume_with(&resume_options(&repo, &first.run_id), &later).expect("resume continues");
    assert!(
        !committed(&resumed, "t1"),
        "an older log must not silently switch review off: {resumed:?}"
    );
}

#[test]
fn an_unavailable_reviewer_is_recorded_as_such_not_as_a_rejection() {
    let (_tree, repo) = temp_engine_repo("outagerecord");
    seed(&repo, FRONTIER_AUTH_PLAN, Some(SECOND_OPINION_CONFIG));
    let source = cross_vendor(
        vec![Effect::EditFile],
        vec![ReviewBehavior::Pass],
        vec![ReviewBehavior::RateLimited, ReviewBehavior::Pass],
    );
    let report = run_with(&cross_vendor_opts(&repo), &source).expect("run");

    let first = &task(&report, "t1").attempts[0];
    assert_eq!(
        first.reviews.iter().map(|r| r.outcome).collect::<Vec<_>>(),
        [
            events::ReviewPassOutcome::Passed,
            events::ReviewPassOutcome::Unavailable
        ],
        "the second vendor was down, not unimpressed"
    );

    assert!(committed(&report, "t1"), "{report:?}");
}

#[test]
fn second_reviewer_spawn_failure_settles_worker_and_first_review_evidence() {
    let (_tree, repo) = temp_engine_repo("secondreviewerspawnsettlement");
    seed(&repo, FRONTIER_AUTH_PLAN, Some(SECOND_OPINION_CONFIG));
    let source = cross_vendor(
        vec![Effect::EditFile],
        vec![ReviewBehavior::Pass],
        vec![ReviewBehavior::SpawnError, ReviewBehavior::Pass],
    );
    let report = run_with(&cross_vendor_opts(&repo), &source).expect("settled run");

    assert!(
        committed(&report, "t1"),
        "the deferred retry recovers: {report:?}"
    );
    let task = task(&report, "t1");
    assert_eq!(task.attempts.len(), 2, "one settled outage, one recovery");
    let first = &task.attempts[0];
    let failure = first.failure.as_ref().expect("spawn failure is recorded");
    assert_eq!(failure.kind, FailureKind::ReviewUnavailable);
    assert_eq!(failure.origin, FailureOrigin::Reviewer);
    assert_eq!(first.cost_usd, Some(0.01), "worker spend survives");
    assert_eq!(first.session_id.as_deref(), Some("s0"));
    assert!(first.usage.is_some(), "worker usage survives");
    assert_eq!(
        first.reviews.iter().map(|r| r.outcome).collect::<Vec<_>>(),
        [
            events::ReviewPassOutcome::Passed,
            events::ReviewPassOutcome::Unavailable
        ],
        "the completed first verdict is not discarded"
    );
    assert_eq!(first.reviews[0].cost_usd, Some(0.05));
    assert_eq!(first.reviews[1].cost_usd, None);
    assert_eq!(source.copilot().review_spawn_failures(), 1);

    let logged = events_of(&repo, &report.run_id);
    assert!(logged.iter().any(|event| matches!(
        &event.body,
        EventBody::AttemptFinished {
            task,
            attempt: 1,
            ..
        } if task == "t1"
    )));
    assert!(!logged.iter().any(|event| matches!(
        &event.body,
        EventBody::AttemptInterrupted {
            task,
            attempt: 1,
            ..
        } if task == "t1"
    )));
}

#[test]
fn a_total_missing_an_unreported_reviewer_is_marked_rather_than_implied() {
    let (_tree, repo) = temp_engine_repo("partialcost");
    seed(&repo, FRONTIER_AUTH_PLAN, Some(SECOND_OPINION_CONFIG));
    let source = FakeSource {
        adapter: FakeAdapter::new(vec![Effect::EditFile], vec![ReviewBehavior::Pass]),
        copilot: Some(FakeAdapter::copilot(vec![ReviewBehavior::Pass]).unpriced()),
    };
    let report = run_with(&cross_vendor_opts(&repo), &source).expect("run");

    let t1 = task(&report, "t1");
    assert_eq!(t1.review_cost_usd, Some(0.05), "only what was reported");
    assert!(t1.review_cost_incomplete, "and it is not the whole story");
    assert!(
        report.render().contains("$0.0500?"),
        "the summary marks it: {}",
        report.render()
    );
    let ledger = report.render_ledger();
    assert!(ledger.contains("$0.0500?"), "{ledger}");
    assert!(
        ledger.contains("reports no spend"),
        "legend present: {ledger}"
    );
}

#[test]
fn every_model_that_judged_a_task_is_listed_beside_the_cost_of_all_of_them() {
    let (_tree, repo) = temp_engine_repo("reviewtrail");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some(
            "[interaction]\nmode = \"never\"\n\n\
                 [routing]\nimplement = { chain = [\"mid\", \"frontier\"], attempts_per = 1 }\n",
        ),
    );

    let source = cross_vendor(
        vec![Effect::EditFile],
        vec![ReviewBehavior::Fail, ReviewBehavior::Pass],
        vec![ReviewBehavior::Pass],
    );
    let report = run_with(&cross_vendor_opts(&repo), &source).expect("run");
    let t1 = task(&report, "t1");
    assert_eq!(t1.attempts.len(), 2, "escalated: {t1:?}");
    assert_eq!(
        t1.review_models,
        ["claude-opus-5", "gpt-5.3-codex"],
        "both judges, in the order they judged"
    );
}

#[test]
fn each_pass_writes_its_own_verdict_transcript() {
    let (_tree, repo) = temp_engine_repo("passtranscripts");
    seed(&repo, FRONTIER_AUTH_PLAN, Some(SECOND_OPINION_CONFIG));
    let source = cross_vendor(
        vec![Effect::EditFile],
        vec![ReviewBehavior::Pass],
        vec![ReviewBehavior::Pass],
    );
    let report = run_with(&cross_vendor_opts(&repo), &source).expect("run");
    let reviews = paths_of(&repo, &report.run_id).reviews();
    assert!(reviews.join("00-t1-1-review.json").is_file());
    assert!(
        reviews.join("00-t1-1-second-opinion-review.json").is_file(),
        "the second verdict cannot overwrite the first"
    );
}

#[test]
fn the_run_record_survives_completion() {
    let (_tree, repo) = temp_engine_repo("record");
    let source = fake(Effect::EditFile);
    let report = run_with(&options(&repo), &source).expect("run");
    let report_path = repo
        .join(".upstroke")
        .join("runs")
        .join(&report.run_id)
        .join("report.json");
    let text = fs::read_to_string(&report_path).expect("report.json written");
    let restored: RunReport = serde_json::from_str(&text).expect("report round-trips");
    assert_eq!(restored.tasks.len(), 2);
    assert_eq!(restored.branch, report.branch);
    assert!(matches!(
        restored.tasks[0].status,
        TaskRunStatus::Committed { .. }
    ));
    assert_eq!(
        restored.tasks[0].attempts.len(),
        1,
        "the per-attempt ledger persists too"
    );
}

#[test]
fn forward_dependencies_run_in_topo_order_not_plan_order() {
    let (_tree, repo) = temp_engine_repo("topo");
    seed(
        &repo,
        "## Second by dependency\n<!-- upstroke: id=late depends=early -->\n\n\
             ## First by dependency\n<!-- upstroke: id=early depends= -->\n",
        None,
    );

    let source = fake(Effect::EditFile);
    let report = run_with(&options(&repo), &source).expect("run");
    let ids: Vec<&str> = report.tasks.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, ["early", "late"], "dependency beats document order");
}

#[test]
fn a_contradictory_pass_fails_closed() {
    let failure = review_failure(
        review::ReviewResult::Judged(crate::ir::Verdict {
            pass: true,
            reasons: vec!["looks fine".to_owned()],
            required_changes: vec!["parameterize the SQL".to_owned()],
            needs_human: false,
        }),
        false,
    )
    .expect("a pass that still demands changes cannot commit");
    assert_eq!(failure.kind, FailureKind::ReviewFailed);
    assert!(
        failure.reason.contains("parameterize the SQL"),
        "{}",
        failure.reason
    );

    assert!(
        review_failure(
            review::ReviewResult::Judged(crate::ir::Verdict {
                pass: true,
                reasons: vec!["meets the criteria".to_owned()],
                required_changes: Vec::new(),
                needs_human: false,
            }),
            false
        )
        .is_none()
    );
}

#[test]
fn an_unavailable_reviewer_is_not_a_rejection() {
    let failure = review_failure(
        review::ReviewResult::Unavailable {
            status: OutcomeStatus::RateLimited,
            detail: "5-hour limit reached".to_owned(),
        },
        false,
    )
    .expect("still fails the attempt");
    assert_eq!(failure.kind, FailureKind::RateLimited);
    assert_eq!(failure.origin, FailureOrigin::Reviewer);
    assert!(failure.is_outage(), "defers instead of blaming the worker");
    assert!(failure.reason.contains("reviewer unavailable"));

    let failure = review_failure(
        review::ReviewResult::Unavailable {
            status: OutcomeStatus::Timeout,
            detail: String::new(),
        },
        false,
    )
    .expect("still fails");
    assert_eq!(failure.kind, FailureKind::Timeout);
    assert_eq!(failure.origin, FailureOrigin::Reviewer);

    let failure = review_failure(
        review::ReviewResult::Unavailable {
            status: OutcomeStatus::AgentError,
            detail: "spawn failed".to_owned(),
        },
        false,
    )
    .expect("still fails");
    assert_eq!(failure.kind, FailureKind::ReviewUnavailable);
}

#[test]
fn required_changes_reach_the_retry_as_a_clean_list() {
    let failure = review_failure(
        review::ReviewResult::Judged(crate::ir::Verdict {
            pass: false,
            reasons: vec!["incomplete".to_owned()],
            required_changes: vec![
                "handle the empty-input case".to_owned(),
                "add a round-trip test".to_owned(),
            ],
            needs_human: false,
        }),
        false,
    )
    .expect("fails");
    assert_eq!(
        failure.feedback.as_deref(),
        Some("- handle the empty-input case\n- add a round-trip test"),
        "every item bulleted, including the first"
    );
    assert_eq!(
        failure.origin,
        FailureOrigin::Worker,
        "a rejected diff is the worker's to fix"
    );
}

#[test]
fn prompt_names_the_allowed_gate_commands() {
    let task = Task {
        id: TaskId::from("t1"),
        kind: TaskKind::Implement,
        title: "Do the thing".to_owned(),
        body: String::new(),
        depends_on: Vec::new(),
        acceptance: Vec::new(),
        path_hints: Vec::new(),
        suggested_tier: None,
        min_tier: None,
        artifacts_in: Vec::new(),
        artifacts_out: Vec::new(),
    };
    let tree = temp_engine_scratch("prompt");
    let run_dir = tree.path();
    fs::create_dir_all(run_dir.join("artifacts")).expect("run dir");
    let prompt = materialize_prompt(
        crate::engine::assembly::WorkerSubject::of(&task),
        &["cargo check --all-targets".to_owned()],
        run_dir,
        None,
    );
    assert!(prompt.contains("EXACTLY these commands"));
    assert!(prompt.contains("- cargo check --all-targets"));
    assert!(
        prompt.contains(QUESTION_MARKER),
        "the worker is told how to ask (§12)"
    );
    let bare = materialize_prompt(
        crate::engine::assembly::WorkerSubject::of(&task),
        &[],
        run_dir,
        None,
    );
    assert!(!bare.contains("EXACTLY these commands"));
}

#[test]
fn prompt_wires_artifacts_to_real_files() {
    let tree = temp_engine_scratch("artifact");
    let run_dir = tree.path();
    fs::create_dir_all(run_dir.join("artifacts")).expect("run dir");
    let mut task = Task {
        id: TaskId::from("t1"),
        kind: TaskKind::Implement,
        title: "Build it".to_owned(),
        body: String::new(),
        depends_on: Vec::new(),
        acceptance: Vec::new(),
        path_hints: Vec::new(),
        suggested_tier: None,
        min_tier: None,
        artifacts_in: vec![crate::ir::ArtifactId::from("api-contract")],
        artifacts_out: vec![crate::ir::ArtifactId::from("notes")],
    };

    let prompt = materialize_prompt(
        crate::engine::assembly::WorkerSubject::of(&task),
        &[],
        run_dir,
        None,
    );
    assert!(prompt.contains("did \n     not leave one") || prompt.contains("did not leave one"));
    assert!(
        prompt.contains("write artifact `notes`"),
        "producer told where to write"
    );

    fs::write(
        artifact_path(run_dir, "api-contract"),
        "cursor = base64(offset)",
    )
    .expect("artifact");
    let prompt = materialize_prompt(
        crate::engine::assembly::WorkerSubject::of(&task),
        &[],
        run_dir,
        None,
    );
    assert!(
        prompt.contains("cursor = base64(offset)"),
        "content inlined"
    );

    task.artifacts_in.clear();
    task.artifacts_out.clear();
    let bare = materialize_prompt(
        crate::engine::assembly::WorkerSubject::of(&task),
        &[],
        run_dir,
        None,
    );
    assert!(!bare.contains("artifact"));
}

#[test]
fn a_gate_failure_recovers_on_the_same_rung_via_session_resume() {
    let (_tree, repo) = temp_engine_repo("resume");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some(
            "[routing]\nimplement = { chain = [\"small\"], attempts_per = 2 }\n\n\
                 [[gates]]\nname = \"needs-test\"\ncmd = \"git ls-files --error-unmatch \
                 widget_test.rs\"\n",
        ),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = source(
        vec![Effect::EditFile, Effect::EditTest],
        vec![ReviewBehavior::Pass],
    );
    let report = run_with(&opts, &source).expect("run");

    assert!(committed(&report, "t1"), "report: {report:?}");
    let t1 = task(&report, "t1");
    assert_eq!(t1.attempts.len(), 2, "one retry, not an escalation");
    assert_eq!(t1.attempts[0].tier, "small");
    assert_eq!(t1.attempts[1].tier, "small", "same rung");
    assert!(!t1.attempts[0].resumed);
    assert!(t1.attempts[1].resumed, "§11.4 retries in-session");

    let runs = source.adapter.runs();
    assert_eq!(runs[0].resume, None);
    assert_eq!(
        runs[1].resume.as_deref(),
        Some("s0"),
        "the retry resumed the failed attempt's session"
    );
    assert!(
        runs[1].prompt.contains("gate `needs-test` failed"),
        "the gate's own words go back: {}",
        runs[1].prompt
    );
    assert!(
        !runs[1].prompt.contains("# Task:"),
        "a resumed session already holds the task; the prompt stays terse"
    );

    let files = git_in(&repo, &["show", "--name-only", "--format=", "HEAD"]);
    assert!(files.contains("agent-output.txt"), "files: {files}");
    assert!(files.contains("widget_test.rs"), "files: {files}");
}

#[test]
fn exhausting_a_rung_escalates_with_a_fresh_session_and_the_history() {
    let (_tree, repo) = temp_engine_repo("escalate");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\", \"mid\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = source(
        vec![Effect::NoEdit, Effect::EditFile],
        vec![ReviewBehavior::Pass],
    );
    let report = run_with(&opts, &source).expect("run");

    assert!(committed(&report, "t1"), "report: {report:?}");
    let t1 = task(&report, "t1");
    assert_eq!(t1.attempts.len(), 2);
    assert_eq!(t1.attempts[0].tier, "small");
    assert_eq!(t1.attempts[0].model, "claude-haiku-4-5");
    assert_eq!(t1.attempts[1].tier, "mid", "one rung up");
    assert_eq!(t1.attempts[1].model, "claude-sonnet-5");
    assert!(
        !t1.attempts[1].resumed,
        "§11.4: a new rung is a new session — a different model cannot \
             inherit another's conversation"
    );
    assert_eq!(t1.trail(), "small failed → mid ok");

    let runs = source.adapter.runs();

    assert_eq!(runs[0].model, "claude-haiku-4-5");
    assert_eq!(runs[1].model, "claude-sonnet-5");
    assert_eq!(runs[1].resume, None, "fresh session");
    assert!(
        runs[1].prompt.contains("# Task:"),
        "a fresh worker gets the whole task again"
    );
    assert!(
        runs[1].prompt.contains("diff is empty"),
        "and what the previous rung got wrong: {}",
        runs[1].prompt
    );
}

#[test]
fn a_parked_question_does_not_stop_the_runnable_frontier() {
    let (_tree, repo) = temp_engine_repo("park");
    seed(
        &repo,
        "## Doomed\n<!-- upstroke: id=t1 kind=implement depends= -->\n\n\
             ## Depends on the doomed one\n<!-- upstroke: id=t2 kind=implement depends=t1 -->\n\n\
             ## Independent\n<!-- upstroke: id=t3 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));

    let source = source(
        vec![Effect::NoEdit, Effect::EditFile],
        vec![ReviewBehavior::Pass],
    );
    let report = run_with(&opts, &source).expect("run");

    let TaskRunStatus::Parked { question, .. } = &task(&report, "t1").status else {
        panic!("t1 should park on a question: {report:?}");
    };
    assert!(committed(&report, "t3"), "independent work kept going");
    assert!(
        matches!(&task(&report, "t2").status, TaskRunStatus::Blocked { by } if by == "t1"),
        "a dependent of a parked task is blocked, not failed"
    );
    assert!(report.halted_at.is_none(), "parking never halts a run");
    assert_eq!(report.outcome(), RunOutcome::Parked);
    assert_eq!(report.parked_tasks(), ["t1"]);

    let path = repo
        .join(".upstroke")
        .join("runs")
        .join(&report.run_id)
        .join("questions")
        .join(format!("{question}.json"));
    let record: QuestionRecord =
        serde_json::from_str(&fs::read_to_string(&path).expect("question file")).expect("parses");
    assert_eq!(record.question.kind, QuestionKind::Unblock);
    assert_eq!(record.question.affected_tasks, [TaskId::from("t1")]);
    assert!(record.answer.is_none(), "still open");
    assert!(record.question.context.contains("Doomed"));

    let rendered = report.render();
    assert!(rendered.contains("PARKED"), "{rendered}");
    assert!(rendered.contains("open questions (1)"), "{rendered}");
}

#[test]
fn answering_the_question_retries_the_task_with_the_operators_words() {
    let (_tree, repo) = temp_engine_repo("answered");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = source(
        vec![Effect::NoEdit, Effect::EditFile],
        vec![ReviewBehavior::Pass],
    );
    let answers = ScriptedAnswers::new(vec![Answer::Answered {
        text: "the widget lives in src/widget.rs — write it there".to_owned(),
    }]);
    let report = run_harness(
        &opts,
        &Harness {
            adapters: &source,
            answers: Some(&answers),
            sleeper: None,
        },
    )
    .expect("run");

    assert!(committed(&report, "t1"), "report: {report:?}");
    assert_eq!(report.outcome(), RunOutcome::Complete);
    let t1 = task(&report, "t1");
    assert_eq!(t1.attempts.len(), 2, "the answer bought a fresh allowance");
    assert_eq!(
        t1.attempts[1].tier, "small",
        "an answer does not move the rung — the chain was already spent"
    );

    let runs = source.adapter.runs();
    assert!(
        runs[1].prompt.contains("src/widget.rs"),
        "the operator's answer reaches the agent: {}",
        runs[1].prompt
    );
    assert!(
        runs[1].prompt.contains("instruction from a person"),
        "and is labelled as an instruction, not quoted data"
    );

    let record = report.questions.first().expect("one question");
    assert!(
        matches!(&record.answer, Some(Answer::Answered { text }) if text.contains("widget.rs")),
        "the answer is recorded against the question: {record:?}"
    );
}

fn design_defect_lines(paths: &RunPaths) -> Vec<serde_json::Value> {
    let text = fs::read_to_string(paths.events()).expect("log");
    text.lines()
        .filter(|line| line.contains("\"event\":\"design_defect\""))
        .map(|line| serde_json::from_str(line).expect("the record parses"))
        .collect()
}

fn assert_unclassified_on_disk(line: &serde_json::Value) {
    let data = line
        .get("data")
        .and_then(serde_json::Value::as_object)
        .expect("the record has a payload object");
    let mut keys: Vec<&str> = data.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        ["answer", "context", "question"],
        "the schema-3 writer writes the record it always wrote, with no attribution and no \
         citation key: {line}"
    );
    let event: events::Event = serde_json::from_value(line.clone()).expect("the line reads");
    let EventBody::DesignDefect { data } = &event.body else {
        panic!("not a design_defect line: {line}");
    };
    assert_eq!(
        data.effective_attribution(),
        events::EffectiveAttribution::Unclassified,
        "written before the taxonomy, so unclassified, never a discovery: {line}"
    );
}

#[test]
fn the_legacy_ingest_writes_an_unclassified_design_defect() {
    let (_tree, repo) = temp_engine_repo("legacydefectbytes");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = source(
        vec![Effect::NoEdit, Effect::EditFile],
        vec![ReviewBehavior::Pass],
    );
    let answers = ScriptedAnswers::new(vec![Answer::Answered {
        text: "the widget lives in src/widget.rs — write it there".to_owned(),
    }]);
    let report = run_harness(
        &opts,
        &Harness {
            adapters: &source,
            answers: Some(&answers),
            sleeper: None,
        },
    )
    .expect("run");
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:?}");

    let lines = design_defect_lines(&paths_of(&repo, &report.run_id));
    assert_eq!(lines.len(), 1, "one answered question, one record");
    assert_unclassified_on_disk(lines.first().expect("the one record"));
}

#[test]
fn the_resume_repair_writes_an_unclassified_design_defect() {
    let (_tree, repo) = temp_engine_repo("legacydefectresume");
    seed(
        &repo,
        "## Doomed\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = source(vec![Effect::NoEdit], vec![ReviewBehavior::Pass]);
    let answers = ScriptedAnswers::new(vec![Answer::Declined]);
    let report = run_harness(
        &opts,
        &Harness {
            adapters: &source,
            answers: Some(&answers),
            sleeper: None,
        },
    )
    .expect("build a complete decline sequence");
    let paths = paths_of(&repo, &report.run_id);
    truncate_log_after(&paths, "question_answered");
    assert!(
        design_defect_lines(&paths).is_empty(),
        "the crash prefix ends before the record"
    );

    let resumed_source = fake(Effect::EditFile);
    let resumed = resume_with(&resume_options(&repo, &report.run_id), &resumed_source)
        .expect("resume repairs the incomplete settlement");
    assert_eq!(resumed.outcome(), RunOutcome::Halted, "{resumed:?}");
    assert!(
        resumed_source.adapter.runs().is_empty(),
        "the repair settles the decline before another paid attempt"
    );

    let lines = design_defect_lines(&paths);
    assert_eq!(lines.len(), 1, "the missing record is appended once");
    assert_unclassified_on_disk(lines.first().expect("the one record"));
}

#[test]
fn declining_fails_the_task_and_halt_is_the_default() {
    let (_tree, repo) = temp_engine_repo("declined");
    seed(
        &repo,
        "## Doomed\n<!-- upstroke: id=t1 kind=implement depends= -->\n\n\
             ## Independent\n<!-- upstroke: id=t3 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = source(vec![Effect::NoEdit], vec![ReviewBehavior::Pass]);
    let answers = ScriptedAnswers::new(vec![Answer::Declined]);
    let report = run_harness(
        &opts,
        &Harness {
            adapters: &source,
            answers: Some(&answers),
            sleeper: None,
        },
    )
    .expect("run");

    let TaskRunStatus::Failed { kind, reason } = &task(&report, "t1").status else {
        panic!("a declined question fails its task: {report:?}");
    };
    assert_eq!(*kind, FailureKind::Declined);
    assert!(!reason.is_empty());
    assert_eq!(
        report.halted_at.as_deref(),
        Some("t1"),
        "§17's default on_task_failure is halt"
    );
    assert_eq!(report.outcome(), RunOutcome::Halted);
}

#[test]
fn resume_repairs_every_decline_settlement_crash_prefix() {
    for (tag, last_durable_event) in [
        ("answered", "question_answered"),
        ("defect", "design_defect"),
    ] {
        let (_tree, repo) = temp_engine_repo(&format!("declineprefix-{tag}"));
        seed(
            &repo,
            "## Doomed\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
            Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
        );
        let mut opts = options(&repo);
        opts.config_path = Some(repo.join("upstroke.toml"));
        let initial = source(vec![Effect::NoEdit], vec![ReviewBehavior::Pass]);
        let answers = ScriptedAnswers::new(vec![Answer::Declined]);
        let report = run_harness(
            &opts,
            &Harness {
                adapters: &initial,
                answers: Some(&answers),
                sleeper: None,
            },
        )
        .expect("build a complete decline sequence");
        let paths = paths_of(&repo, &report.run_id);
        truncate_log_after(&paths, last_durable_event);
        fs::write(
            repo.join("upstroke.toml"),
            "[engine]\non_task_failure = \"continue\"\n\n\
                 [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n",
        )
        .expect("change today's policy after the decline was durable");

        let resumed_source = fake(Effect::EditFile);
        let resumed = resume_with(&resume_options(&repo, &report.run_id), &resumed_source)
            .expect("resume repairs the incomplete settlement");
        assert_eq!(
            resumed.outcome(),
            RunOutcome::Halted,
            "prefix {tag}: repair must use the policy recorded with the answer"
        );
        assert!(
            matches!(
                task(&resumed, "t1").status,
                TaskRunStatus::Failed {
                    kind: FailureKind::Declined,
                    ..
                }
            ),
            "prefix {tag}: {resumed:?}"
        );
        assert!(
            resumed_source.adapter.runs().is_empty(),
            "repair must settle the decline before another paid attempt"
        );

        let logged = events_of(&repo, &report.run_id);
        assert_eq!(
            logged
                .iter()
                .filter(|event| matches!(event.body, EventBody::DesignDefect { .. }))
                .count(),
            1,
            "the missing prefix is appended once"
        );
        assert_eq!(
            logged
                .iter()
                .filter(|event| matches!(event.body, EventBody::TaskFailed { .. }))
                .count(),
            1,
            "the declined task is settled once"
        );
    }
}

#[test]
fn schema_two_decline_prefix_preserves_or_refuses_unknown_halt_policy() {
    let (_tree, repo) = temp_engine_repo("legacydeclinepolicy");
    seed(
        &repo,
        "## Doomed\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let initial = source(vec![Effect::NoEdit], vec![ReviewBehavior::Pass]);
    let answers = ScriptedAnswers::new(vec![Answer::Declined]);
    let report = run_harness(
        &opts,
        &Harness {
            adapters: &initial,
            answers: Some(&answers),
            sleeper: None,
        },
    )
    .expect("build a complete decline sequence");
    let paths = paths_of(&repo, &report.run_id);
    truncate_log_after(&paths, "question_answered");
    rewrite_run_started_as_schema_two(&paths);
    strip_event_data_field(&paths, "question_answered", "decline_halts_run");
    fs::write(
        repo.join("upstroke.toml"),
        "[engine]\non_task_failure = \"continue\"\n\n\
             [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n",
    )
    .expect("today's policy differs");

    let error = resume_err(&repo, &report.run_id);
    assert!(
        error.contains("contemporaneous on_task_failure policy"),
        "{error}"
    );
    assert!(
        error.contains("cannot safely decide an old answer"),
        "{error}"
    );
}

#[test]
fn on_task_failure_continue_keeps_independent_work_moving() {
    let (_tree, repo) = temp_engine_repo("continue");
    seed(
        &repo,
        "## Doomed\n<!-- upstroke: id=t1 kind=implement depends= -->\n\n\
             ## Depends on the doomed one\n<!-- upstroke: id=t2 kind=implement depends=t1 -->\n\n\
             ## Independent\n<!-- upstroke: id=t3 kind=implement depends= -->\n",
        Some(
            "[engine]\non_task_failure = \"continue\"\n\n\
                 [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n",
        ),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = source(
        vec![Effect::NoEdit, Effect::EditFile],
        vec![ReviewBehavior::Pass],
    );
    let answers = ScriptedAnswers::new(vec![Answer::Declined]);
    let report = run_harness(
        &opts,
        &Harness {
            adapters: &source,
            answers: Some(&answers),
            sleeper: None,
        },
    )
    .expect("run");

    assert!(report.halted_at.is_none(), "configured to continue");
    assert!(matches!(
        task(&report, "t1").status,
        TaskRunStatus::Failed { .. }
    ));
    assert!(committed(&report, "t3"));
    assert!(
        matches!(&task(&report, "t2").status, TaskRunStatus::Blocked { by } if by == "t1"),
        "§19: dependents of a failed task are blocked"
    );
}

#[test]
fn a_rate_limit_defers_without_spending_an_attempt() {
    let (_tree, repo) = temp_engine_repo("ratelimit");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = source(
        vec![Effect::RateLimited, Effect::EditFile],
        vec![ReviewBehavior::Pass],
    );
    let sleeper = RecordingSleeper::default();
    let report = run_harness(
        &opts,
        &Harness {
            adapters: &source,
            answers: None,
            sleeper: Some(&sleeper),
        },
    )
    .expect("run");

    assert!(
        committed(&report, "t1"),
        "the rate limit cost no attempt: {report:?}"
    );
    let t1 = task(&report, "t1");
    assert_eq!(t1.attempts.len(), 2);
    assert_eq!(
        t1.attempts[0].failure.as_ref().map(|f| f.kind),
        Some(FailureKind::RateLimited)
    );
    assert_eq!(t1.attempts[1].tier, "small", "never escalated for a pool");
    assert_eq!(
        sleeper.waits().len(),
        1,
        "waited once, because deferred work was all that was left"
    );
    assert!(
        git_in(&repo, &["status", "--porcelain"]).trim().is_empty(),
        "a deferred task hands back a clean tree — another task may run next"
    );
}

#[test]
fn a_pool_that_never_returns_ends_at_the_human_rung() {
    let (_tree, repo) = temp_engine_repo("ratelimit-forever");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\", \"mid\"], attempts_per = 2 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    opts.max_defers = 2;
    let source = source(vec![Effect::RateLimited], vec![ReviewBehavior::Pass]);
    let sleeper = RecordingSleeper::default();
    let report = run_harness(
        &opts,
        &Harness {
            adapters: &source,
            answers: None,
            sleeper: Some(&sleeper),
        },
    )
    .expect("run");

    assert!(
        matches!(task(&report, "t1").status, TaskRunStatus::Parked { .. }),
        "an exhausted pool becomes a question, not an infinite retry: {report:?}"
    );
    assert_eq!(
        task(&report, "t1").attempts.len(),
        3,
        "two deferrals, then the attempt that gave up"
    );
    assert!(
        task(&report, "t1")
            .attempts
            .iter()
            .all(|a| a.tier == "small"),
        "a busy pool never pushes the task up-tier"
    );
    assert_eq!(sleeper.waits().len(), 2, "one wait per deferral");
}

#[test]
fn an_unavailable_reviewer_defers_the_task_instead_of_escalating_it() {
    let (_tree, repo) = temp_engine_repo("reviewdown");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\", \"mid\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = source(
        vec![Effect::EditFile],
        vec![ReviewBehavior::RateLimited, ReviewBehavior::Pass],
    );
    let sleeper = RecordingSleeper::default();
    let report = run_harness(
        &opts,
        &Harness {
            adapters: &source,
            answers: None,
            sleeper: Some(&sleeper),
        },
    )
    .expect("run");

    assert!(committed(&report, "t1"), "report: {report:?}");
    let t1 = task(&report, "t1");
    assert_eq!(t1.attempts.len(), 2);
    assert_eq!(
        t1.attempts[0].failure.as_ref().map(|f| f.origin),
        Some(FailureOrigin::Reviewer),
        "the outage is attributed to the judge"
    );
    assert_eq!(
        t1.attempts[1].tier, "small",
        "the implementer was never escalated for the reviewer being down"
    );
}

#[test]
fn a_reviewer_asking_for_a_human_parks_without_spending_the_chain() {
    let (_tree, repo) = temp_engine_repo("needshuman");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\", \"mid\"], attempts_per = 2 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = source(vec![Effect::EditFile], vec![ReviewBehavior::NeedsHuman]);
    let report = run_with(&opts, &source).expect("run");

    let t1 = task(&report, "t1");
    assert!(
        matches!(t1.status, TaskRunStatus::Parked { .. }),
        "report: {report:?}"
    );
    assert_eq!(
        t1.attempts.len(),
        1,
        "the reviewer declined to judge, so nothing was retried or escalated"
    );
    assert_eq!(
        t1.attempts[0].failure.as_ref().map(|f| f.kind),
        Some(FailureKind::NeedsHuman)
    );
    let record = report.questions.first().expect("question raised");
    assert_eq!(record.question.kind, QuestionKind::Clarify);
    assert!(
        record
            .question
            .context
            .contains("contradict the API contract"),
        "the reviewer's reason reaches the person: {}",
        record.question.context
    );
    assert!(
        record.question.context.contains("not instructions to you"),
        "agent-authored text is labelled as data"
    );
}

#[test]
fn a_worker_can_stop_and_ask_rather_than_guess() {
    let (_tree, repo) = temp_engine_repo("workerasks");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n\n\
             ## Independent\n<!-- upstroke: id=t3 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\", \"mid\"], attempts_per = 2 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = source(
        vec![Effect::AskQuestion, Effect::EditFile],
        vec![ReviewBehavior::Pass],
    );
    let answers = ScriptedAnswers::new(vec![Answer::Answered {
        text: "opaque cursors".to_owned(),
    }]);
    let report = run_harness(
        &opts,
        &Harness {
            adapters: &source,
            answers: Some(&answers),
            sleeper: None,
        },
    )
    .expect("run");

    let record = report.questions.first().expect("the worker's question");
    assert_eq!(record.question.kind, QuestionKind::Clarify);
    assert!(
        record.question.context.contains("opaque or signed"),
        "context: {}",
        record.question.context
    );

    assert!(committed(&report, "t3"));
    assert!(committed(&report, "t1"), "report: {report:?}");
    let t1 = task(&report, "t1");
    assert_eq!(
        t1.attempts.len(),
        2,
        "asking cost no attempt — only the retry after the answer"
    );

    assert!(
        !t1.attempts[1].resumed,
        "a parked task never resumes into a tree that was reverted underneath it"
    );

    let runs = source.adapter.runs();
    let retry = &runs[2];
    assert_eq!(retry.resume, None, "fresh session, not --resume");
    assert!(
        retry.prompt.contains("# Task:"),
        "the whole task is re-sent, since the session no longer carries it: {}",
        retry.prompt
    );
    assert!(
        retry.prompt.contains("opaque cursors"),
        "and the operator's answer travels with it: {}",
        retry.prompt
    );
    assert!(
        retry.prompt.contains("instruction from a person"),
        "labelled as an instruction rather than quoted as data"
    );
}

#[test]
fn ci_mode_parks_rather_than_failing_and_says_so() {
    let (_tree, repo) = temp_engine_repo("ci");
    seed(
        &repo,
        "## Doomed\n<!-- upstroke: id=t1 kind=implement depends= -->\n\n\
             ## Depends on the doomed one\n<!-- upstroke: id=t2 kind=implement depends=t1 -->\n\n\
             ## Independent\n<!-- upstroke: id=t3 kind=implement depends= -->\n",
        Some(
            "[interaction]\nmode = \"never\"\n\n\
                 [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n",
        ),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = source(
        vec![Effect::NoEdit, Effect::EditFile],
        vec![ReviewBehavior::Pass],
    );
    let report = run_with(&opts, &source).expect("run");

    assert_eq!(report.outcome(), RunOutcome::Parked);
    assert!(report.halted_at.is_none(), "parked is not halted");
    assert!(matches!(
        task(&report, "t1").status,
        TaskRunStatus::Parked { .. }
    ));
    assert!(committed(&report, "t3"));
    assert!(matches!(
        task(&report, "t2").status,
        TaskRunStatus::Blocked { .. }
    ));
    assert!(
        report.questions.iter().all(QuestionRecord::is_open),
        "nothing answered it, and nothing pretended to"
    );
}

#[test]
fn an_unanswerable_question_is_never_asked_twice() {
    let (_tree, repo) = temp_engine_repo("noloop");
    seed(
        &repo,
        "## Doomed\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = source(vec![Effect::NoEdit], vec![ReviewBehavior::Pass]);
    let answers = CountingAnswers::default();
    let report = run_harness(
        &opts,
        &Harness {
            adapters: &source,
            answers: Some(&answers),
            sleeper: None,
        },
    )
    .expect("run terminates");
    assert_eq!(report.outcome(), RunOutcome::Parked);
    assert_eq!(
        answers.count(),
        1,
        "asked once; an unreachable channel is not retried"
    );
}

#[derive(Default)]
struct CountingAnswers {
    calls: Mutex<usize>,
}

impl CountingAnswers {
    fn count(&self) -> usize {
        self.calls.lock().map(|c| *c).unwrap_or(0)
    }
}

impl AnswerSource for CountingAnswers {
    fn id(&self) -> &'static str {
        "counting"
    }

    fn resolve(&self, _question: &Question) -> Result<Answer, UpstrokeError> {
        if let Ok(mut calls) = self.calls.lock() {
            *calls += 1;
        }
        Ok(Answer::Unanswered)
    }
}

#[test]
fn agent_errors_and_empty_diffs_carry_feedback_the_retry_can_use() {
    let (_tree, repo) = temp_engine_repo("feedback");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 2 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = source(
        vec![Effect::Error, Effect::EditFile],
        vec![ReviewBehavior::Pass],
    );
    let report = run_with(&opts, &source).expect("run");
    assert!(committed(&report, "t1"), "report: {report:?}");
    let runs = source.adapter.runs();
    assert!(
        runs[1].prompt.contains("fake adapter error detail"),
        "the adapter's own diagnosis reaches the retry: {}",
        runs[1].prompt
    );
}

#[test]
fn an_unparseable_reviewer_fails_after_one_reask() {
    let (_tree, repo) = temp_engine_repo("reviewprose");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = source(vec![Effect::EditFile], vec![ReviewBehavior::Unparseable]);
    let report = run_with(&opts, &source).expect("engine ok");

    let failure = task(&report, "t1").attempts[0]
        .failure
        .as_ref()
        .expect("a reviewer that never answers cannot pass a task");
    assert_eq!(failure.kind, FailureKind::ReviewFailed);
    assert!(
        failure.reason.contains("re-ask"),
        "reason: {}",
        failure.reason
    );

    let reviews = paths_of(&repo, &report.run_id).reviews();
    assert!(reviews.join("00-t1-1-review.json").is_file());
    assert!(
        reviews.join("00-t1-1-review-reask.json").is_file(),
        "one re-ask before giving up (§11.2)"
    );
    assert_eq!(
        git_in(&repo, &["rev-list", "--count", "main..HEAD"]).trim(),
        "0",
        "nothing commits without a passing verdict"
    );
}

#[test]
fn gate_logs_are_named_by_the_collision_free_stem() {
    let (_tree, repo) = temp_engine_repo("gatelogs");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some(
            "[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n\n\
                 [[gates]]\nname = \"never\"\ncmd = \"git frobnicate-not-a-command\"\n",
        ),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = fake(Effect::EditFile);
    let report = run_with(&opts, &source).expect("engine ok");

    let failure = task(&report, "t1").attempts[0]
        .failure
        .as_ref()
        .expect("gate should fail");
    assert_eq!(failure.kind, FailureKind::GateFailed);
    let gates_dir = paths_of(&repo, &report.run_id).gates();
    assert!(
        gates_dir.join("00-t1-1-never.log").is_file(),
        "the log stem matches the task's other artifacts, so two ids that \
             sanitize alike cannot overwrite each other"
    );
    assert!(
        git_in(&repo, &["status", "--porcelain"]).trim().is_empty(),
        "rolled back"
    );
}

#[test]
fn the_trail_summarizes_the_ladder() {
    let report = TaskReport {
        id: "t1".to_owned(),
        title: "x".to_owned(),
        model: "m".to_owned(),
        status: TaskRunStatus::Skipped,
        duration: Duration::ZERO,
        cost_usd: None,
        review_models: Vec::new(),
        review_cost_usd: None,
        review_cost_incomplete: false,
        session_id: None,
        attempts: vec![
            attempt_record(1, "small", true),
            attempt_record(2, "small", true),
            attempt_record(3, "mid", false),
        ],
    };
    assert_eq!(report.trail(), "small×2 failed → mid ok");
}

fn attempt_record(attempt: u32, tier: &str, failed: bool) -> AttemptRecord {
    AttemptRecord {
        attempt,
        tier: tier.to_owned(),
        model: "m".to_owned(),
        pool: None,
        resumed: false,
        duration: Duration::ZERO,
        cost_usd: None,
        reviews: Vec::new(),
        session_id: None,
        usage: None,
        failure: failed.then(|| FailureRecord {
            kind: FailureKind::GateFailed,
            origin: FailureOrigin::Worker,
            reason: "no".to_owned(),
            detail: None,
        }),
    }
}

fn raw_object_after(line: &str, key: &str) -> Option<String> {
    let start = line.find(key)? + key.len();
    let bytes: Vec<char> = line[start..].chars().collect();
    if bytes.first() != Some(&'{') {
        return None;
    }
    let (mut depth, mut in_string, mut escaped) = (0usize, false, false);
    for (index, c) in bytes.iter().enumerate() {
        if escaped {
            escaped = false;
            continue;
        }
        match c {
            '\\' if in_string => escaped = true,
            '"' => in_string = !in_string,
            '{' if !in_string => depth += 1,
            '}' if !in_string => {
                depth -= 1;
                if depth == 0 {
                    return Some(bytes[..=index].iter().collect());
                }
            }
            _ => {}
        }
    }
    None
}

#[test]
fn the_legacy_wire_and_report_carry_no_feedback_on_the_attempt_record() {
    let (_tree, repo) = temp_engine_repo("legacy-no-detail");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some(
            "[routing]\nimplement = { chain = [\"small\"], attempts_per = 2 }\n\n\
                 [[gates]]\nname = \"needs-test\"\ncmd = \"git ls-files --error-unmatch \
                 widget_test.rs\"\n",
        ),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = source(
        vec![Effect::EditFile, Effect::EditTest],
        vec![ReviewBehavior::Pass],
    );
    run_with(&opts, &source).expect("run");

    let runs = opts.repo_root.join(".upstroke").join("runs");
    let public = fs::read_dir(&runs)
        .expect("the runs root")
        .flatten()
        .map(|entry| entry.path())
        .find(|path| path.join("events.jsonl").is_file())
        .expect("the run left a public directory");
    let log = fs::read_to_string(public.join("events.jsonl")).expect("the log");

    const PRE_CHANGE_FAILURE: &str = concat!(
        r#"{"kind":"gate_failed","origin":"worker","reason":"gate `needs-test` failed: "#,
        r#"error: pathspec 'widget_test.rs' did not match any file(s) known to git\n"#,
        r#"Did you forget to 'git add'?\n\nexit code: Some(1)"}"#,
    );

    let mut failures = 0;
    let mut carried_tail = false;
    for line in log.lines() {
        let event: serde_json::Value = serde_json::from_str(line).expect("a json line");
        if event.get("event").and_then(serde_json::Value::as_str) != Some("attempt_finished") {
            continue;
        }
        let Some(failure) = event.pointer("/data/failure").filter(|v| !v.is_null()) else {
            continue;
        };
        failures += 1;

        let bytes =
            raw_object_after(line, "\"failure\":").expect("the line carries a failure object");
        let stripped = bytes.replace(",\"detail\":null", "");
        assert_ne!(
            stripped, bytes,
            "the legacy failure record carries no `detail` key at all: {bytes}. This test \
             asserts the *only* difference from `610106b` is that one null, and if the key \
             is absent the assertion below is vacuous"
        );
        assert_eq!(
            bytes.matches(",\"detail\":").count(),
            1,
            "expected exactly one `detail` key in {bytes}"
        );

        assert_eq!(
            stripped, PRE_CHANGE_FAILURE,
            "the legacy failure record is not the bytes `610106b` wrote for this \
             scenario. If a newer git reworded its pathspec error, re-capture the \
             fixture at that commit rather than loosening this comparison"
        );

        let object = failure.as_object().expect("the failure is an object");
        assert_eq!(
            object.get("detail"),
            Some(&serde_json::Value::Null),
            "the legacy attempt record's `detail` is {:?}; it must be present and null — \
             a value would be §11.4's feedback duplicated onto the wire, and an absent \
             key would mean the field stopped serializing, which breaks schema 4's \
             strict door",
            object.get("detail")
        );

        if event
            .pointer("/transition/data/detail")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|d| d.contains("widget_test.rs"))
        {
            carried_tail = true;
        }
    }
    assert!(
        failures >= 1,
        "no failed attempt in the log, so this test asserted nothing"
    );
    assert!(
        carried_tail,
        "the gate tail reached no `ladder_retry` transition either, so the legacy engine \
         lost §11.4's feedback rather than keeping it where it belongs"
    );

    let report: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(public.join("report.json")).expect("report.json is written"),
    )
    .expect("report.json is json");
    let mut checked = 0;
    for task in report["tasks"].as_array().into_iter().flatten() {
        for attempt in task["attempts"].as_array().into_iter().flatten() {
            let Some(failure) = attempt.get("failure").filter(|v| !v.is_null()) else {
                continue;
            };
            checked += 1;
            let object = failure.as_object().expect("the failure is an object");
            assert_eq!(
                object.get("detail"),
                Some(&serde_json::Value::Null),
                "report.json's attempt {} carries `detail` as {:?}; it must be present \
                 and null. `is_null()` was the earlier assertion and it cannot tell an \
                 explicit null from an absent key, so it would have passed had the \
                 field silently stopped serializing",
                attempt["attempt"],
                object.get("detail")
            );
        }
    }
    assert!(
        checked >= 1,
        "no failed attempt reached report.json, so its half of this test asserted nothing"
    );

    let runs = source.adapter.runs();
    assert!(
        runs[1].prompt.contains("gate `needs-test` failed"),
        "the legacy retry lost the gate's words: {}",
        runs[1].prompt
    );
}

#[test]
fn an_explicit_null_detail_survives_the_strict_door() {
    use crate::topology::events::{
        AttemptFinished4, AttemptNumber, AttemptSettlement, GenerationId, LeaseDisposition,
        SettlementTransition, TopologyEvent, TopologyEventBody,
    };

    let mut record = serde_json::to_value(attempt_record(1, "mid", true)).expect("serialize");
    let failure = record
        .get_mut("failure")
        .and_then(serde_json::Value::as_object_mut)
        .expect("a failed record carries a failure object");

    failure.insert("detail".to_owned(), serde_json::Value::Null);

    let mut value = serde_json::to_value(TopologyEvent::now(TopologyEventBody::AttemptFinished {
        data: Box::new(AttemptFinished4 {
            key: crate::topology::registry::TaskKey(0),
            generation: GenerationId(0),
            attempt: AttemptNumber(1),
            record: Box::new(attempt_record(1, "mid", true)),
            settlement: AttemptSettlement::Closed {
                transition: SettlementTransition::Retry,
                lease: LeaseDisposition::PredictedReleased,
            },
        }),
    }))
    .expect("the event serializes");
    value["data"]["record"] = record;

    let parsed: TopologyEvent = serde_json::from_value(value).expect(
        "an explicit null on a known optional field must pass the strict door; if this \
         refuses, the door is reporting a field the record does declare, and every \
         failed attempt's settlement is unreadable",
    );
    let TopologyEventBody::AttemptFinished { data } = parsed.body else {
        unreachable!("built as an attempt_finished")
    };
    assert_eq!(
        data.record
            .failure
            .as_ref()
            .expect("the failure survives")
            .detail,
        None,
        "an explicit null must read back as None"
    );
}

#[test]
fn both_feedback_sources_reach_the_durable_attempt_record() {
    use crate::gates::GateFailure;

    let tail =
        "error[E0308]: mismatched types\n  --> src/alpha.rs:12:9\n   expected `u32`, found `&str`";
    let gate = super::classify::gate_failure(&GateFailure {
        gate: "cargo test".to_owned(),
        summary: "1 failed".to_owned(),
        log_tail: tail.to_owned(),
    });
    assert_eq!(
        durable_detail(&gate).as_deref(),
        Some(tail),
        "§11.1's gate tail did not reach the record, so a resume cannot tell the \
         retry what the gate printed"
    );

    let review = super::attempt::review_failure(
        review::ReviewResult::Judged(crate::ir::Verdict {
            pass: false,
            reasons: vec!["the parser accepts a trailing comma".to_owned()],
            required_changes: vec![
                "reject a trailing comma in `parse_list`".to_owned(),
                "add a case for the empty list".to_owned(),
            ],
            needs_human: false,
        }),
        false,
    )
    .expect("a failed verdict is a failure");
    assert_eq!(
        durable_detail(&review).as_deref(),
        Some("- reject a trailing comma in `parse_list`\n- add a case for the empty list"),
        "§11.2's required_changes did not reach the record verbatim, so an \
         escalation carries the reviewer's summary instead of its instructions"
    );
}

fn durable_detail(failure: &crate::ladder::AttemptFailure) -> Option<String> {
    let outcome = crate::ir::Outcome {
        status: crate::ir::OutcomeStatus::Completed,
        diff: String::new(),
        detail: None,
        session_id: None,
        usage: None,
        cost_usd: None,
        transcript_path: PathBuf::new(),
        duration: Duration::ZERO,
    };
    super::classify::attempt_record(
        1,
        super::classify::AttemptFacts {
            tier: crate::ir::Tier::Mid,
            model: "claude-opus-5",
            pool: None,
            resumed: false,
            outcome: &outcome,
            reviews: &[],
            failure: Some(failure),

            feedback: super::classify::FeedbackCarrier::AttemptRecord,
        },
    )
    .failure
    .expect("a classified failure produces a failure record")
    .detail
}

#[test]
fn a_worker_question_is_read_from_the_marker_onward() {
    assert_eq!(
        worker_question(Some("Did some work.\nUPSTROKE-QUESTION: opaque or signed?")).as_deref(),
        Some("opaque or signed?")
    );

    assert_eq!(
        worker_question(Some("UPSTROKE-QUESTION: which store?\nRedis or Postgres?")).as_deref(),
        Some("which store?\nRedis or Postgres?")
    );
    assert_eq!(worker_question(Some("UPSTROKE-QUESTION:   ")), None);
    assert_eq!(worker_question(Some("no marker here")), None);
    assert_eq!(worker_question(None), None);
}

#[test]
fn an_echoed_marker_does_not_swallow_the_real_question() {
    let reply = "The retry feedback says I can use the UPSTROKE-QUESTION: marker if I am \
                     blocked. I considered whether this needs one.\n\n\
                     UPSTROKE-QUESTION: should cursors be opaque or signed?";
    assert_eq!(
        worker_question(Some(reply)).as_deref(),
        Some("should cursors be opaque or signed?"),
        "last marker wins, matching the prompt and review.rs's verdict rule"
    );
}

#[test]
fn an_outage_is_never_reclassified_as_a_question() {
    let quoting = "I will end with the UPSTROKE-QUESTION: marker if I get stuck.";
    let output = crate::agent::ProcessOutput {
        stdout: String::new(),
        stderr: String::new(),
        code: Some(1),
        timed_out: false,
        output_limited: false,
        duration: Duration::ZERO,
    };
    for (status, expected) in [
        (OutcomeStatus::RateLimited, FailureKind::RateLimited),
        (OutcomeStatus::Timeout, FailureKind::Timeout),
        (OutcomeStatus::AgentError, FailureKind::AgentError),
    ] {
        let outcome = fake_outcome(status, Some(quoting.to_owned()), "s0", None, Duration::ZERO);
        let failure = evaluate_outcome(&outcome, &output).expect("still a failure");
        assert_eq!(failure.kind, expected, "{status:?} must keep its own kind");
    }

    let mut asked = fake_outcome(
        OutcomeStatus::Completed,
        Some("UPSTROKE-QUESTION: opaque or signed?".to_owned()),
        "s0",
        None,
        Duration::ZERO,
    );
    asked.diff = "diff --git a/x b/x\n+x\n".to_owned();
    assert_eq!(
        evaluate_outcome(&asked, &output).expect("parks").kind,
        FailureKind::NeedsHuman
    );
}

#[test]
fn a_halted_run_stops_asking_and_keeps_naming_the_real_cause() {
    let (_tree, repo) = temp_engine_repo("haltpark");
    seed(
        &repo,
        "## Asks a question\n<!-- upstroke: id=t1 kind=implement depends= -->\n\n\
             ## Exhausts its chain\n<!-- upstroke: id=t2 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));

    let source = source(
        vec![Effect::AskQuestion, Effect::NoEdit],
        vec![ReviewBehavior::Pass],
    );

    let answers = ScriptedAnswers::new(vec![
        Answer::Declined,
        Answer::Answered {
            text: "this answer must never be used".to_owned(),
        },
    ]);
    let report = run_harness(
        &opts,
        &Harness {
            adapters: &source,
            answers: Some(&answers),
            sleeper: None,
        },
    )
    .expect("run");

    assert!(
        matches!(
            task(&report, "t1").status,
            TaskRunStatus::Failed {
                kind: FailureKind::Declined,
                ..
            }
        ),
        "the decline is what halts the run: {report:?}"
    );
    assert_eq!(
        report.halted_at.as_deref(),
        Some("t1"),
        "halted_at names the task that actually caused the halt"
    );

    assert!(
        matches!(task(&report, "t2").status, TaskRunStatus::Parked { .. }),
        "t2 is never asked after the halt, so it stays parked rather than \
             silently consuming an answer: {report:?}"
    );
    let t2_question = report
        .questions
        .iter()
        .find(|q| q.question.affected_tasks.iter().any(|t| t.as_str() == "t2"))
        .expect("t2 raised a question");
    assert!(
        t2_question.is_open(),
        "left open on disk for a later resume (§15)"
    );
}

#[test]
fn unreported_cost_stays_unreported_rather_than_zero() {
    assert_eq!(sum_opt([None, None].into_iter()), None);
    assert_eq!(
        sum_opt([Some(0.01), None, Some(0.02)].into_iter()),
        Some(0.03)
    );
}

fn replay_of(repo: &Path, run_id: &str) -> crate::status::RunStatus {
    crate::status::load(repo, Some(run_id)).expect("the run reads back")
}

struct Scenario {
    name: &'static str,
    config: &'static str,

    plan: Option<&'static str>,
    effects: Vec<Effect>,
    reviews: Vec<ReviewBehavior>,

    second_opinion: Option<Vec<ReviewBehavior>>,
    answers: Vec<Answer>,
}

impl Scenario {
    fn new(name: &'static str, config: &'static str, effects: Vec<Effect>) -> Self {
        Self {
            name,
            config,
            plan: None,
            effects,
            reviews: vec![ReviewBehavior::Pass],
            second_opinion: None,
            answers: Vec::new(),
        }
    }

    fn reviewed(mut self, reviews: Vec<ReviewBehavior>) -> Self {
        self.reviews = reviews;
        self
    }

    fn cross_vendor(mut self, plan: &'static str, second: Vec<ReviewBehavior>) -> Self {
        self.plan = Some(plan);
        self.second_opinion = Some(second);
        self
    }

    fn answered(mut self, answers: Vec<Answer>) -> Self {
        self.answers = answers;
        self
    }
}

fn assert_live_equals_replay(repo: &Path, live: &RunState, report: &RunReport) {
    let replayed = replay_of(repo, &report.run_id);
    assert_eq!(
        &replayed.state, live,
        "replaying the log produced different state than the run that wrote it"
    );

    let strip = |report: &RunReport| {
        let mut value = serde_json::to_value(report).expect("serialize");
        if let Some(object) = value.as_object_mut() {
            object.remove("warnings");
        }
        value
    };
    assert_eq!(
        strip(&replayed.report()),
        strip(report),
        "the report derived from the log differs from the one the run wrote"
    );
}

#[test]
fn live_state_equals_replayed_state_across_every_ladder_path() {
    let scenarios = vec![
        Scenario::new(
            "commit",
            "[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n",
            vec![Effect::EditFile],
        ),
        Scenario::new(
            "retry",
            "[routing]\nimplement = { chain = [\"small\"], attempts_per = 2 }\n",
            vec![Effect::NoEdit, Effect::EditFile],
        ),
        Scenario::new(
            "escalate",
            "[routing]\nimplement = { chain = [\"small\", \"mid\"], attempts_per = 1 }\n",
            vec![Effect::NoEdit, Effect::EditFile],
        ),
        Scenario::new(
            "defer",
            "[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n",
            vec![Effect::RateLimited, Effect::EditFile],
        ),
        Scenario::new(
            "park-then-answer",
            "[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n",
            vec![Effect::NoEdit, Effect::EditFile],
        )
        .answered(vec![Answer::Answered {
            text: "the widget lives in src/widget.rs".to_owned(),
        }]),
        Scenario::new(
            "decline-and-halt",
            "[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n",
            vec![Effect::NoEdit],
        )
        .answered(vec![Answer::Declined]),
        Scenario::new(
            "reviewer-asks-for-a-human",
            "[routing]\nimplement = { chain = [\"small\", \"mid\"], attempts_per = 2 }\n",
            vec![Effect::EditFile],
        )
        .reviewed(vec![ReviewBehavior::NeedsHuman]),
        Scenario::new(
            "second-opinion-passes",
            SECOND_OPINION_CONFIG,
            vec![Effect::EditFile],
        )
        .cross_vendor(FRONTIER_AUTH_PLAN, vec![ReviewBehavior::Pass]),
        Scenario::new(
            "second-opinion-rejects",
            SECOND_OPINION_CONFIG,
            vec![Effect::EditFile],
        )
        .cross_vendor(FRONTIER_AUTH_PLAN, vec![ReviewBehavior::Fail])
        .answered(vec![Answer::Declined]),
        Scenario::new(
            "self-review-rebind",
            FRONTIER_ONLY_CONFIG,
            vec![Effect::EditFile],
        )
        .cross_vendor(FRONTIER_AUTH_PLAN, vec![ReviewBehavior::Pass]),
        Scenario::new(
            "budget-stop",
            "[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n\n\
                 [budgets]\nrun_usd = 0.05\n",
            vec![Effect::EditFile],
        ),
        Scenario::new(
            "approve-spend",
            "[routing]\nimplement = { chain = [\"mid\", \"frontier\"], attempts_per = 1 }\n\n\
                 [interaction]\nask_before = { frontier_escalation_over_usd = 0.005 }\n",
            vec![Effect::NoEdit, Effect::EditFile],
        )
        .answered(vec![Answer::Answered {
            text: "approve: run the escalated attempt".to_owned(),
        }]),
    ];

    for Scenario {
        name,
        config,
        plan,
        effects,
        reviews,
        second_opinion,
        answers,
    } in scenarios
    {
        let (_tree, repo) = temp_engine_repo(&format!("replay-{name}"));
        seed(
            &repo,
            plan.unwrap_or(
                "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n\n\
                     ## Independent\n<!-- upstroke: id=t3 kind=implement depends= -->\n",
            ),
            Some(config),
        );
        let mut opts = options(&repo);
        opts.config_path = Some(repo.join("upstroke.toml"));
        let cross_vendor_scenario = second_opinion.is_some();
        let source = match second_opinion {
            Some(second) => cross_vendor(effects, reviews, second),
            None => source(effects, reviews),
        };
        let scripted = ScriptedAnswers::new(answers);
        let (report, live) = run_harness_inner(
            &opts,
            &Harness {
                adapters: &source,
                answers: Some(&scripted),
                sleeper: None,
            },
        )
        .unwrap_or_else(|e| panic!("{name}: {e}"));

        if cross_vendor_scenario {
            let judged: Vec<&str> = report
                .tasks
                .iter()
                .flat_map(|t| &t.attempts)
                .flat_map(|a| &a.reviews)
                .map(|r| r.agent.as_str())
                .collect();
            assert!(
                judged.contains(&"copilot"),
                "{name}: the second vendor never judged anything, so this scenario \
                     exercises nothing new: {judged:?}"
            );
        }
        assert_live_equals_replay(&repo, &live, &report);
    }
}

#[test]
fn an_aborting_error_still_leaves_a_replayable_log() {
    let (_tree, repo) = temp_engine_repo("abortlog");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));

    let source = source(vec![Effect::EditFile], vec![ReviewBehavior::Pass]);
    opts.attempt_timeout = Duration::from_secs(60);
    let report = run_with(&opts, &source).expect("the run itself succeeds");

    let paths = paths_of(&repo, &report.run_id);
    let text = fs::read_to_string(paths.events()).expect("log");
    let lines: Vec<&str> = text.lines().collect();
    let cut = lines
        .iter()
        .position(|line| line.contains("\"attempt_finished\""))
        .expect("the run recorded an attempt");
    fs::write(paths.events(), format!("{}\n", lines[..cut].join("\n"))).expect("truncate");

    let replayed = replay_of(&repo, &report.run_id);
    assert_eq!(replayed.interrupted, 1, "the dangling attempt is settled");
    assert!(
        replayed.interrupted_run(),
        "and the run reads as interrupted rather than finished"
    );
    assert_eq!(replayed.state.states[0], TaskState::Pending);
}

#[test]
fn a_run_that_has_spent_nothing_totals_positive_zero() {
    let nothing: [f64; 0] = [];
    assert!(
        nothing.iter().sum::<f64>().is_sign_negative(),
        "`sum` no longer folds from -0.0, so `total_of` is obsolete"
    );

    assert!(!total_of(&[]).is_sign_negative(), "a spent-nothing total");
    assert_eq!(format!("${:.4}", total_of(&[])), "$0.0000");

    let spent = vec![
        task_report_costing(Some(0.25), Some(1.5)),
        task_report_costing(None, None),
        task_report_costing(Some(0.0), None),
    ];
    assert!((total_of(&spent) - 1.75).abs() < f64::EPSILON);
}

fn task_report_costing(worker: Option<f64>, review: Option<f64>) -> TaskReport {
    TaskReport {
        id: "t".to_owned(),
        title: String::new(),
        model: String::new(),
        status: TaskRunStatus::Skipped,
        duration: Duration::ZERO,
        cost_usd: worker,
        review_models: Vec::new(),
        review_cost_usd: review,
        review_cost_incomplete: false,
        session_id: None,
        attempts: Vec::new(),
    }
}

#[test]
fn a_live_run_reads_as_running_rather_than_halted() {
    let (_tree, repo) = temp_engine_repo("livestatus");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n\n\
             ## Waits on the widget\n<!-- upstroke: id=t2 kind=implement depends=t1 -->\n\n\
             ## Independent\n<!-- upstroke: id=t3 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let report = run_with(&opts, &fake(Effect::EditFile)).expect("run");
    let paths = paths_of(&repo, &report.run_id);

    let text = fs::read_to_string(paths.events()).expect("log");
    let lines: Vec<&str> = text.lines().collect();
    let cut = lines
        .iter()
        .position(|line| line.contains("\"attempt_finished\""))
        .expect("an attempt");
    fs::write(paths.events(), format!("{}\n", lines[..cut].join("\n"))).expect("truncate");

    let stopped = replay_of(&repo, &report.run_id);
    assert!(stopped.interrupted_run());
    let out = crate::status::render(&stopped);
    assert!(out.contains("skipped (run interrupted)"), "{out}");
    assert!(out.contains("t2: blocked by `t1`"), "{out}");

    let lock = RunLock::acquire(&paths.public).expect("simulate a live engine");

    let live = replay_of(&repo, &report.run_id);
    assert!(live.running, "a held lock means an engine is driving this");
    assert_eq!(
        live.interrupted, 0,
        "an attempt in flight has not been interrupted"
    );
    let out = crate::status::render(&live);
    assert!(out.contains("t1: running now"), "{out}");

    assert!(out.contains("t2: queued"), "{out}");
    assert!(out.contains("t3: queued"), "{out}");
    assert!(out.contains("run in progress"), "{out}");
    for lie in [
        "small failed",
        "skipped (run halted)",
        "skipped (run interrupted)",
        "run complete",
        "run interrupted",
        "blocked by",
    ] {
        assert!(!out.contains(lie), "a live run reported `{lie}`:\n{out}");
    }
    drop(lock);
}

#[test]
fn a_truncated_run_resumes_without_spending_the_interrupted_attempt() {
    let (_tree, repo) = temp_engine_repo("resumetrunc");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = fake(Effect::EditFile);
    let report = run_with(&opts, &source).expect("run");
    let run_id = report.run_id.clone();
    let paths = paths_of(&repo, &run_id);

    let text = fs::read_to_string(paths.events()).expect("log");
    let lines: Vec<&str> = text.lines().collect();
    let cut = lines
        .iter()
        .position(|line| line.contains("\"attempt_finished\""))
        .expect("an attempt");
    fs::write(paths.events(), format!("{}\n", lines[..cut].join("\n"))).expect("truncate");
    git_in(&repo, &["reset", "-q", "--hard", "HEAD~1"]);
    fs::write(repo.join("agent-output.txt"), "half-written\n").expect("residue");

    let source = fake(Effect::EditFile);
    let (resumed, state) = resume_harness_inner(
        &resume_options(&repo, &run_id),
        &Harness {
            adapters: &source,
            answers: None,
            sleeper: None,
        },
    )
    .expect("resume");

    assert_eq!(resumed.outcome(), RunOutcome::Complete, "{resumed:?}");
    assert!(committed(&resumed, "t1"));

    let t1 = task(&resumed, "t1");
    assert_eq!(
        t1.attempts.len(),
        2,
        "the interrupted attempt is on the record beside the one that worked"
    );
    assert_eq!(
        t1.attempts[0].failure.as_ref().map(|f| f.kind),
        Some(FailureKind::Interrupted)
    );
    assert_eq!(
        t1.attempts[0].cost_usd, None,
        "unknown spend is reported as unknown, not as free"
    );
    assert_eq!(t1.attempts[1].tier, "small", "still on the same rung");
    assert!(
        !t1.attempts[1].resumed,
        "§14: the tree was discarded, so the session cannot be trusted"
    );

    assert!(
        git_in(&repo, &["status", "--porcelain"]).trim().is_empty(),
        "crash residue discarded"
    );
    assert_eq!(
        git_in(&repo, &["rev-list", "--count", "main..HEAD"]).trim(),
        "1",
        "one commit, not a duplicate of the interrupted attempt's work"
    );
    assert!(
        resumed
            .warnings
            .iter()
            .any(|w| w.contains("discarded") && w.contains("agent-output.txt")),
        "the operator is told what was thrown away: {:?}",
        resumed.warnings
    );
    assert_live_equals_replay(&repo, &state, &resumed);
}

#[test]
fn killing_a_run_mid_attempt_leaves_a_resumable_record() {
    let (_tree, repo) = temp_engine_repo("crashkill");
    seed(
        &repo,
        "## First\n<!-- upstroke: id=t1 kind=implement depends= -->\n\n\
             ## Second\n<!-- upstroke: id=t2 kind=implement depends=t1 -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );

    let exe = std::env::current_exe().expect("test binary");
    let status = Command::new(exe)
        .args([
            "--exact",
            "engine::tests::crash_child_dies_inside_an_attempt",
            "--ignored",
            "--test-threads",
            "1",
        ])
        .env("UPSTROKE_CRASH_REPO", &repo)
        .output()
        .expect("spawn the child run");
    assert_eq!(
        status.status.code(),
        Some(CRASH_EXIT_CODE),
        "the child must die inside the attempt, not finish or panic: {}",
        String::from_utf8_lossy(&status.stderr)
    );

    let run_id = rundir::latest_run(&repo).expect("the child started a run");
    let paths = paths_of(&repo, &run_id);

    assert!(
        !git_in(&repo, &["status", "--porcelain"]).trim().is_empty(),
        "the dead agent's edits are still in the tree"
    );
    let log = fs::read_to_string(paths.events()).expect("log");
    let last = log.lines().last().expect("events");
    assert!(
        last.contains("\"attempt_started\"") && last.contains("\"t2\""),
        "the log ends mid-attempt: {last}"
    );
    assert!(
        !log.contains("\"run_finished\""),
        "a killed run never records an ending"
    );

    let before = replay_of(&repo, &run_id);
    assert!(before.interrupted_run(), "status calls it interrupted");
    assert_eq!(before.interrupted, 1);
    assert!(
        crate::status::render(&before).contains(&format!("upstroke resume {run_id}")),
        "and tells the operator how to continue it"
    );

    assert!(
        !rundir::is_running(&paths.public),
        "the OS released the lock"
    );

    let rendered = crate::status::render(&before);
    assert!(
        rendered.contains("run interrupted: 1 task(s) committed so far"),
        "{rendered}"
    );
    assert!(
        !rendered.contains("run complete"),
        "a killed run claimed it completed:\n{rendered}"
    );

    assert!(rendered.contains("skipped (run interrupted)"), "{rendered}");

    let source = fake(Effect::EditFile);
    let (resumed, state) = resume_harness_inner(
        &resume_options(&repo, &run_id),
        &Harness {
            adapters: &source,
            answers: None,
            sleeper: None,
        },
    )
    .expect("resume the killed run");

    assert_eq!(resumed.outcome(), RunOutcome::Complete, "{resumed:?}");
    assert!(committed(&resumed, "t1"), "the work it did survived");
    assert!(
        committed(&resumed, "t2"),
        "and the work it died in got done"
    );
    assert_eq!(
        git_in(&repo, &["rev-list", "--count", "main..HEAD"]).trim(),
        "2",
        "one commit per task, with nothing duplicated by the resume"
    );
    let t2 = task(&resumed, "t2");
    assert_eq!(
        t2.attempts[0].failure.as_ref().map(|f| f.kind),
        Some(FailureKind::Interrupted),
        "the attempt it died in is on the record: {t2:?}"
    );
    assert_live_equals_replay(&repo, &state, &resumed);
}

#[test]
#[ignore = "spawned by killing_a_run_mid_attempt_leaves_a_resumable_record"]
fn crash_child_dies_inside_an_attempt() {
    let Ok(repo) = std::env::var("UPSTROKE_CRASH_REPO") else {
        return;
    };
    let repo = PathBuf::from(repo);
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));

    let source = source(
        vec![Effect::EditFile, Effect::Exit],
        vec![ReviewBehavior::Pass],
    );
    let _ = run_with(&opts, &source);

    std::process::exit(0);
}

const V1_OBJECT_GRAPH_GATES: &str = "[[gates]]\nname = \"snapshot-bytes\"\n\
     cmd = 'git grep --no-index -q -F recorded -- probe.txt'\n\n\
     [[gates]]\nname = \"object-graph\"\n\
     cmd = 'git grep -q -F recorded HEAD -- probe.txt'\n";

/// The guard comes back with the path because it owns the root the path is
/// under: dropping it here would reclaim the repository before the caller's
/// first assertion.
fn replaced_probe_repo(tag: &str, plan: &str, config: &str) -> (ScratchTree, PathBuf) {
    let (tree, repo) = temp_engine_repo(tag);
    crate::workspace_manager::fixture::pin_replacement_refs_in(&repo);
    fs::write(repo.join("probe.txt"), "recorded\n").expect("the recorded probe");
    seed(&repo, plan, Some(config));
    let recorded = git_in(&repo, &["rev-parse", "HEAD:probe.txt"])
        .trim()
        .to_owned();

    fs::write(repo.join("replacing.txt"), "replacing\n").expect("the replacing probe");
    let replacing = git_in(&repo, &["hash-object", "-w", "replacing.txt"])
        .trim()
        .to_owned();
    fs::remove_file(repo.join("replacing.txt")).expect("the replacing probe's file");
    assert_ne!(recorded, replacing, "two distinct blobs");

    git_in(&repo, &["replace", &recorded, &replacing]);
    assert_eq!(
        git_in(&repo, &["replace", "-l"]).trim(),
        recorded,
        "the replacement is in place"
    );
    assert_eq!(
        git_in(&repo, &["show", "HEAD:probe.txt"]),
        "replacing\n",
        "a Git child that does not refuse the replacement reads the replacing \
         blob in the tree every snapshot of this run starts from; without that \
         neither gate measures anything"
    );
    assert!(
        git_in(&repo, &["status", "--porcelain"]).trim().is_empty(),
        "the fixture left the worktree dirty"
    );
    (tree, repo)
}

#[test]
fn the_v1_conductor_runs_and_resumes_on_the_graph_its_own_workspace_wrote() {
    let status = crate::workspace_manager::fixture::run_replacement_witness_child(
        "engine::tests::v1_object_graph_helper",
    );
    assert!(
        status.success(),
        "the child drives `engine::run_harness` and `engine::resume_harness` over a \
         repository whose committed probe blob carries a replacement, and ended \
         {status:?}"
    );
}

#[test]
#[ignore = "subprocess helper"]
fn v1_object_graph_helper() {
    if std::env::var_os(crate::workspace_manager::fixture::REPLACEMENT_WITNESS).is_none() {
        return;
    }
    crate::workspace_manager::fixture::assert_replacement_controls_pinned("v1-object-graph");

    let (_tree, repo) = replaced_probe_repo(
        "v1graphrun",
        "## Implement the widget\n<!-- upstroke: id=t1 depends= -->\n",
        &format!("[interaction]\nmode = \"never\"\n\n{V1_OBJECT_GRAPH_GATES}"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let report = run_with(&opts, &fake(Effect::EditFile)).expect("the run");
    assert_eq!(
        report.gates,
        ["snapshot-bytes", "object-graph"],
        "{report:?}"
    );
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:?}");
    assert!(committed(&report, "t1"), "{report:?}");

    let (_tree, repo) = replaced_probe_repo(
        "v1graphresume",
        "## Doomed\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        &format!(
            "[interaction]\nmode = \"never\"\n\n\
             [routing]\nimplement = {{ chain = [\"small\"], attempts_per = 1 }}\n\n\
             {V1_OBJECT_GRAPH_GATES}"
        ),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let parked = run_with(
        &opts,
        &source(vec![Effect::NoEdit], vec![ReviewBehavior::Pass]),
    )
    .expect("the first run");
    assert_eq!(parked.outcome(), RunOutcome::Parked, "{parked:?}");
    let question = parked
        .questions
        .first()
        .expect("a question was raised")
        .question
        .id
        .to_string();
    crate::answer::answer(
        &repo,
        &question[..8],
        crate::answer::Reply::Text("the widget lives in src/widget.rs".to_owned()),
    )
    .expect("answer");

    let resumed = resume_with(
        &resume_options(&repo, &parked.run_id),
        &fake(Effect::EditFile),
    )
    .expect("the resume");
    assert_eq!(
        resumed.gates,
        ["snapshot-bytes", "object-graph"],
        "{resumed:?}"
    );
    assert_eq!(resumed.outcome(), RunOutcome::Complete, "{resumed:?}");
    assert!(committed(&resumed, "t1"), "{resumed:?}");
}

#[test]
fn a_checkout_a_replacement_rewrote_is_refused_with_a_check_the_reader_can_run() {
    let status = crate::workspace_manager::fixture::run_replacement_witness_child(
        "engine::tests::replaced_head_refusal_helper",
    );
    assert!(
        status.success(),
        "the child refuses a run, and a resume off the run branch, over a checkout that \
         a replacement of HEAD rewrote, and ended {status:?}"
    );
}

fn replace_head_with_a_sibling(repo: &Path) {
    crate::workspace_manager::fixture::pin_replacement_refs_in(repo);
    let head = git_in(repo, &["rev-parse", "HEAD"]).trim().to_owned();
    let parent = git_in(repo, &["rev-parse", "HEAD^"]).trim().to_owned();
    fs::write(repo.join("README.md"), "replaced\n").expect("the replacing content");
    git_in(repo, &["add", "README.md"]);
    let tree = git_in(repo, &["write-tree"]).trim().to_owned();
    git_in(repo, &["reset", "-q", "--hard", &head]);
    let sibling = git_in(
        repo,
        &["commit-tree", &tree, "-p", &parent, "-m", "replacing"],
    )
    .trim()
    .to_owned();
    git_in(repo, &["replace", &head, &sibling]);
    git_in(repo, &["reset", "-q", "--hard", "HEAD"]);
    assert_eq!(
        fs::read_to_string(repo.join("README.md"))
            .expect("the checked-out README")
            .replace("\r\n", "\n"),
        "replaced\n",
        "the checkout was written through the replacement"
    );
    assert!(
        git_in(repo, &["status", "--porcelain"]).trim().is_empty(),
        "plain `git status` reads the replacement and reports the checkout clean"
    );
    assert_eq!(
        git_in(
            repo,
            &[
                "--no-replace-objects",
                "-c",
                "core.useReplaceRefs=false",
                "status",
                "--porcelain"
            ]
        )
        .trim(),
        "M  README.md",
        "the command the refusal names lists what upstroke finds, and the repository pins \
         `core.useReplaceRefs = true`, which on Git 2.41 outranks the variable alone"
    );
}

#[test]
#[ignore = "subprocess helper"]
fn replaced_head_refusal_helper() {
    if std::env::var_os(crate::workspace_manager::fixture::REPLACEMENT_WITNESS).is_none() {
        return;
    }
    crate::workspace_manager::fixture::assert_replacement_controls_pinned("replaced-head-refusal");

    let (_tree, repo) = temp_engine_repo("replacedheadrun");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 depends= -->\n",
        Some("[interaction]\nmode = \"never\"\n"),
    );
    replace_head_with_a_sibling(&repo);
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let refused = run_with(&opts, &fake(Effect::EditFile))
        .expect_err("a checkout that differs from what HEAD records is refused")
        .to_string();
    for named in [
        "not clean",
        "`git --no-replace-objects -c core.useReplaceRefs=false status`",
        "`git replace -l`",
    ] {
        assert!(refused.contains(named), "`{named}` in: {refused}");
    }
    assert_eq!(
        git_in(&repo, &["rev-parse", "--abbrev-ref", "HEAD"]).trim(),
        "main",
        "no run branch was created"
    );

    let (_tree, repo) = temp_engine_repo("replacedheadresume");
    seed(
        &repo,
        "## Doomed\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some(
            "[interaction]\nmode = \"never\"\n\n\
             [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n",
        ),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let parked = run_with(
        &opts,
        &source(vec![Effect::NoEdit], vec![ReviewBehavior::Pass]),
    )
    .expect("the first run");
    assert_eq!(parked.outcome(), RunOutcome::Parked, "{parked:?}");
    git_in(&repo, &["switch", "-q", "main"]);
    replace_head_with_a_sibling(&repo);
    let refused = resume_with(
        &resume_options(&repo, &parked.run_id),
        &fake(Effect::EditFile),
    )
    .expect_err("a resume off the run branch over that checkout is refused")
    .to_string();
    for named in [
        "uncommitted changes",
        "`git --no-replace-objects -c core.useReplaceRefs=false status`",
        "`git replace -l`",
    ] {
        assert!(refused.contains(named), "`{named}` in: {refused}");
    }
}

#[test]
fn a_parked_run_is_answered_out_of_band_and_resumed() {
    let (_tree, repo) = temp_engine_repo("answerresume");
    seed(
        &repo,
        "## Doomed\n<!-- upstroke: id=t1 kind=implement depends= -->\n\n\
             ## Depends on it\n<!-- upstroke: id=t2 kind=implement depends=t1 -->\n",
        Some(
            "[interaction]\nmode = \"never\"\n\n\
                 [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n",
        ),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = source(vec![Effect::NoEdit], vec![ReviewBehavior::Pass]);
    let report = run_with(&opts, &source).expect("run");

    assert_eq!(report.outcome(), RunOutcome::Parked);
    let run_id = report.run_id.clone();
    let question = report
        .questions
        .first()
        .expect("a question was raised")
        .question
        .id
        .to_string();

    let recorded = crate::answer::answer(
        &repo,
        &question[..8],
        crate::answer::Reply::Text("the widget lives in src/widget.rs".to_owned()),
    )
    .expect("answer by prefix");
    assert_eq!(recorded.run_id, run_id);
    assert!(!recorded.run_is_live);

    let source = fake(Effect::EditFile);
    let (resumed, state) = resume_harness_inner(
        &resume_options(&repo, &run_id),
        &Harness {
            adapters: &source,
            answers: None,
            sleeper: None,
        },
    )
    .expect("resume");

    assert_eq!(resumed.outcome(), RunOutcome::Complete, "{resumed:?}");
    assert!(committed(&resumed, "t1"), "the answer un-parked it");
    assert!(committed(&resumed, "t2"), "and its dependent ran");

    let runs = source.adapter.runs();
    let retry = runs.first().expect("a retry ran");
    assert!(
        retry.prompt.contains("src/widget.rs"),
        "the operator's answer reached the agent: {}",
        retry.prompt
    );
    assert!(
        retry.prompt.contains("instruction from a person"),
        "labelled as an instruction, not quoted as data"
    );
    assert_live_equals_replay(&repo, &state, &resumed);
}

#[test]
fn an_answer_arriving_mid_run_unparks_without_a_hard_block() {
    let (_tree, repo) = temp_engine_repo("midrun");
    seed(
        &repo,
        "## Asks a question\n<!-- upstroke: id=t1 kind=implement depends= -->\n\n\
             ## Independent\n<!-- upstroke: id=t3 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 2 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = source(
        vec![Effect::AskQuestion, Effect::EditFile],
        vec![ReviewBehavior::Pass],
    );

    let report = run_harness(
        &opts,
        &Harness {
            adapters: &source,
            answers: Some(&AnsweringViaFile { repo: repo.clone() }),
            sleeper: None,
        },
    )
    .expect("run");

    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:?}");
    assert!(committed(&report, "t1"), "the file answer released it");
    assert!(committed(&report, "t3"));
    let answered = report.questions.first().expect("one question");
    assert!(
        matches!(&answered.answer, Some(Answer::Answered { text }) if text.contains("opaque")),
        "the answer is recorded against the question: {answered:?}"
    );
}

struct AnsweringViaFile {
    repo: PathBuf,
}

impl AnswerSource for AnsweringViaFile {
    fn id(&self) -> &'static str {
        "test-file-writer"
    }

    fn resolve(&self, question: &Question) -> Result<Answer, UpstrokeError> {
        let _ = crate::answer::answer(
            &self.repo,
            question.id.as_str(),
            crate::answer::Reply::Text("opaque cursors".to_owned()),
        );
        Ok(Answer::Unanswered)
    }
}

#[test]
fn blocking_propagates_transitively_and_against_plan_order() {
    let (_tree, repo) = temp_engine_repo("blocked");
    seed(
        &repo,
        "## Last\n<!-- upstroke: id=late kind=implement depends=mid -->\n\n\
             ## Middle\n<!-- upstroke: id=mid kind=implement depends=first -->\n\n\
             ## First\n<!-- upstroke: id=first kind=implement depends= -->\n",
        Some(
            "[interaction]\nmode = \"never\"\n\n\
                 [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n",
        ),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = source(vec![Effect::NoEdit], vec![ReviewBehavior::Pass]);
    let report = run_with(&opts, &source).expect("run");

    assert!(matches!(
        task(&report, "first").status,
        TaskRunStatus::Parked { .. }
    ));
    assert!(
        matches!(&task(&report, "mid").status, TaskRunStatus::Blocked { by } if by == "first"),
        "the direct dependent is blocked: {report:?}"
    );
    assert!(
        matches!(&task(&report, "late").status, TaskRunStatus::Blocked { by } if by == "mid"),
        "and so is its dependent, naming the nearest blocker: {report:?}"
    );
}

#[test]
fn answering_a_blocker_releases_the_chain_behind_it() {
    let (_tree, repo) = temp_engine_repo("unblock");
    seed(
        &repo,
        "## Last\n<!-- upstroke: id=late kind=implement depends=mid -->\n\n\
             ## Middle\n<!-- upstroke: id=mid kind=implement depends=first -->\n\n\
             ## First\n<!-- upstroke: id=first kind=implement depends= -->\n",
        Some(
            "[interaction]\nmode = \"never\"\n\n\
                 [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n",
        ),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = source(vec![Effect::NoEdit], vec![ReviewBehavior::Pass]);
    let report = run_with(&opts, &source).expect("run");
    let run_id = report.run_id.clone();
    let question = report.questions[0].question.id.to_string();

    crate::answer::answer(
        &repo,
        &question,
        crate::answer::Reply::Text("write src/first.rs".to_owned()),
    )
    .expect("answer");

    let source = fake(Effect::EditFile);
    let resumed = resume_with(&resume_options(&repo, &run_id), &source).expect("resume");
    assert_eq!(resumed.outcome(), RunOutcome::Complete, "{resumed:?}");
    for id in ["first", "mid", "late"] {
        assert!(committed(&resumed, id), "{id} should have run: {resumed:?}");
    }
}

#[test]
fn an_exhausted_pool_and_a_silent_operator_still_terminate() {
    let (_tree, repo) = temp_engine_repo("terminate");
    seed(
        &repo,
        "## One\n<!-- upstroke: id=t1 kind=implement depends= -->\n\n\
             ## Two\n<!-- upstroke: id=t2 kind=implement depends= -->\n\n\
             ## After one\n<!-- upstroke: id=t3 kind=implement depends=t1 -->\n",
        Some("[routing]\nimplement = { chain = [\"small\", \"mid\"], attempts_per = 2 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    opts.max_defers = 2;
    let source = source(vec![Effect::RateLimited], vec![ReviewBehavior::Pass]);
    let answers = CountingAnswers::default();
    let sleeper = RecordingSleeper::default();
    let report = run_harness(
        &opts,
        &Harness {
            adapters: &source,
            answers: Some(&answers),
            sleeper: Some(&sleeper),
        },
    )
    .expect("the run terminates rather than spinning");

    assert_eq!(report.outcome(), RunOutcome::Parked);
    for id in ["t1", "t2"] {
        assert!(
            matches!(task(&report, id).status, TaskRunStatus::Parked { .. }),
            "{id}: {report:?}"
        );
    }
    assert!(matches!(
        task(&report, "t3").status,
        TaskRunStatus::Blocked { .. }
    ));
    assert_eq!(
        answers.count(),
        2,
        "each question is asked exactly once, however many times the loop turns"
    );
    assert!(
        !sleeper.waits().is_empty(),
        "the deferral branch really fired"
    );
    assert!(
        sleeper.waits().len() <= 8,
        "and it was bounded: {:?}",
        sleeper.waits()
    );
}

fn resume_err(repo: &Path, run_id: &str) -> String {
    let source = fake(Effect::EditFile);
    resume_with(&resume_options(repo, run_id), &source)
        .expect_err("resume must refuse")
        .to_string()
}

const PARKED_RUN_CONFIG: &str = "[interaction]\nmode = \"never\"\n\n\
         [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n";

/// The guard comes back with the path because it owns the root the path is
/// under: dropping it here would reclaim the repository before the caller's
/// first assertion.
fn parked_run(tag: &str) -> (ScratchTree, PathBuf, String) {
    parked_run_with_config(tag, PARKED_RUN_CONFIG)
}

fn parked_run_with_config(tag: &str, config: &str) -> (ScratchTree, PathBuf, String) {
    let (tree, repo) = temp_engine_repo(tag);
    seed(
        &repo,
        "## Doomed\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some(config),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = source(vec![Effect::NoEdit], vec![ReviewBehavior::Pass]);
    let report = run_with(&opts, &source).expect("run");
    assert_eq!(report.outcome(), RunOutcome::Parked);
    (tree, repo, report.run_id)
}

#[test]
fn resume_refuses_schema_two_failed_attempt_without_recorded_decision() {
    let (_tree, repo, run_id) = parked_run("legacyfailedprefix");
    let paths = paths_of(&repo, &run_id);
    rewrite_run_started_as_schema_two(&paths);
    strip_event_field(&paths, "attempt_finished", "parking");
    truncate_log_after(&paths, "attempt_finished");
    let before = fs::read(paths.events()).expect("legacy prefix");

    let error = resume_err(&repo, &run_id);
    assert!(error.contains("failed attempt 1"), "{error}");
    assert!(
        error.contains("without its durable ladder or parking decision"),
        "{error}"
    );
    assert_eq!(
        fs::read(paths.events()).expect("refused log"),
        before,
        "refusal must not upgrade or otherwise mutate the ambiguous prefix"
    );
}

#[test]
fn resume_refuses_when_the_branch_moved_under_it() {
    let (_tree, repo, run_id) = parked_run("headmoved");
    fs::write(repo.join("someone-else.txt"), "a hand-made commit\n").expect("file");
    git_in(&repo, &["add", "-A"]);
    git_in(&repo, &["commit", "-q", "-m", "not the engine's work"]);

    let err = resume_err(&repo, &run_id);
    assert!(err.contains("record ends at"), "got: {err}");
    assert!(
        err.contains("Move the branch back"),
        "and says what to do: {err}"
    );
}

#[test]
fn resume_refuses_when_the_frozen_plan_changed() {
    let (_tree, repo, run_id) = parked_run("planmoved");
    fs::write(
        repo.join("plan.md"),
        "## Doomed\n<!-- upstroke: id=t1 kind=implement depends= -->\nNow with a body.\n",
    )
    .expect("edit the plan");

    let err = resume_err(&repo, &run_id);
    assert!(
        err.contains("has changed since this run froze it"),
        "got: {err}"
    );
    assert!(
        err.contains("attribute work to the wrong tasks"),
        "and why it matters: {err}"
    );
}

#[test]
fn status_export_and_resume_refuse_mutated_normalized_plan_bytes() {
    let (_tree, repo, run_id) = parked_run("normalized-plan-tamper");
    let plan_path = paths_of(&repo, &run_id).plan_json();
    let mut plan: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&plan_path).expect("frozen plan"))
            .expect("valid frozen plan");
    plan["tasks"][0]["title"] =
        serde_json::Value::String("tampered but self-hash unchanged".to_owned());
    fs::write(
        &plan_path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&plan).expect("serialize plan")
        ),
    )
    .expect("replace frozen plan");

    let status_error = match crate::status::load(&repo, Some(&run_id)) {
        Ok(_) => panic!("status must authenticate the exact normalized bytes"),
        Err(error) => error.to_string(),
    };
    assert!(
        status_error.contains("normalized plan digest"),
        "{status_error}"
    );

    let export_error = match crate::export::load(&repo, &run_id) {
        Ok(_) => panic!("export must authenticate the exact normalized bytes"),
        Err(error) => error.to_string(),
    };
    assert!(
        export_error.contains("normalized plan digest"),
        "{export_error}"
    );

    let resume_error = resume_err(&repo, &run_id);
    assert!(
        resume_error.contains("exact bytes") && resume_error.contains("normalized-plan digest"),
        "{resume_error}"
    );
}

#[test]
fn resume_refuses_schema_two_spend_question_without_task_parked() {
    let (_tree, repo, run_id) = parked_run("legacyspendprefix");
    let paths = paths_of(&repo, &run_id);
    rewrite_run_started_as_schema_two(&paths);
    strip_event_field(&paths, "attempt_finished", "parking");
    truncate_log_after(&paths, "attempt_finished");
    let mut warnings = Vec::new();
    let mut log = EventLog::open(EventSite::LegacyOpenLog, &paths.events(), &mut warnings)
        .expect("legacy log");
    log.append(
        EventSite::LegacyAppend,
        EventBody::LadderEscalated {
            task: "t1".to_owned(),
            attempt: 1,
            rung: 0,
            data: events::LadderEscalated {
                to_rung: 1,
                tier: "small".to_owned(),
                summary: "escalate".to_owned(),
                detail: None,
            },
        },
    )
    .expect("legacy escalation");
    log.append(
        EventSite::LegacyAppend,
        EventBody::QuestionRaised {
            task: "t1".to_owned(),
            data: Box::new(events::QuestionRaised {
                question: Question {
                    id: QuestionId::from("q-spend-prefix"),
                    kind: QuestionKind::ApproveSpend,
                    affected_tasks: vec![TaskId::from("t1")],
                    context: "approve spend".to_owned(),
                    options: Vec::new(),
                },
            }),
        },
    )
    .expect("legacy question");
    drop(log);
    let before = fs::read(paths.events()).expect("ambiguous prefix");

    let error = resume_err(&repo, &run_id);
    assert!(error.contains("ApproveSpend"), "{error}");
    assert!(error.contains("before durably parking the task"), "{error}");
    assert_eq!(
        fs::read(paths.events()).expect("refused log"),
        before,
        "refusal never upgrades the spend-approval gap"
    );
}

#[test]
fn legacy_status_still_refuses_a_mismatched_self_reported_plan_hash() {
    let (_tree, repo, run_id) = parked_run("legacy-status-plan-hash");
    let paths = paths_of(&repo, &run_id);
    rewrite_run_started_as_schema_two(&paths);
    let plan_path = paths.plan_json();
    let mut plan: serde_json::Value =
        serde_json::from_slice(&fs::read(&plan_path).expect("frozen plan"))
            .expect("valid frozen plan");
    plan["source"]["hash"] = serde_json::Value::String("different-plan".to_owned());
    fs::write(
        &plan_path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&plan).expect("serialize plan")
        ),
    )
    .expect("replace frozen plan");

    let error = match crate::status::load(&repo, Some(&run_id)) {
        Ok(_) => panic!("legacy status retains its source-hash boundary"),
        Err(error) => error.to_string(),
    };
    assert!(error.contains("frozen plan hash"), "{error}");
    assert!(error.contains("different-plan"), "{error}");
}

#[test]
fn legacy_upgrade_never_blesses_a_modified_normalized_snapshot() {
    let (_tree, repo, run_id) = parked_run("legacy-plan-upgrade-tamper");
    let paths = paths_of(&repo, &run_id);
    rewrite_run_started_as_schema_two(&paths);
    let plan_path = paths.plan_json();
    let mut plan: serde_json::Value =
        serde_json::from_slice(&fs::read(&plan_path).expect("frozen plan"))
            .expect("valid frozen plan");
    plan["tasks"][0]["title"] = serde_json::Value::String("modified snapshot".to_owned());
    fs::write(
        &plan_path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&plan).expect("serialize plan")
        ),
    )
    .expect("tamper legacy snapshot");
    let before = fs::read(paths.events()).expect("legacy log");

    let error = resume_err(&repo, &run_id);
    assert!(error.contains("Refusing to bless"), "{error}");
    assert_eq!(
        fs::read(paths.events()).expect("refused log"),
        before,
        "refusal happens before the schema upgrade append"
    );
}

#[test]
fn resume_compares_canonical_source_semantics_to_the_recorded_plan_digest() {
    let (_tree, repo, run_id) = parked_run("source-semantics-digest");
    fs::write(
        repo.join("plan.md"),
        "## Changed semantics\n<!-- upstroke: id=t1 kind=implement depends= -->\nDifferent body.\n",
    )
    .expect("change source plan");
    let new_hash = crate::ir::content_hash(&fs::read(repo.join("plan.md")).expect("plan"));
    let paths = paths_of(&repo, &run_id);
    let text = fs::read_to_string(paths.events()).expect("log");
    let rewritten = text
        .lines()
        .map(|line| {
            let mut value: serde_json::Value = serde_json::from_str(line).expect("event");
            if value["event"] == "run_started" {
                value["data"]["plan_hash"] = serde_json::Value::String(new_hash.clone());
            }
            value.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(paths.events(), format!("{rewritten}\n")).expect("force legacy hash guard equal");

    let error = resume_err(&repo, &run_id);
    assert!(
        error.contains("validated source plan now normalizes to digest"),
        "{error}"
    );
}

#[test]
fn resume_refuses_when_routing_moved_under_a_recorded_rung() {
    let (_tree, repo, run_id) = parked_run("chainmoved");
    fs::write(
        repo.join("upstroke.toml"),
        "[interaction]\nmode = \"never\"\n\n\
             [routing]\nimplement = { chain = [\"mid\", \"frontier\"], attempts_per = 1 }\n",
    )
    .expect("edit config");

    let err = resume_err(&repo, &run_id);
    assert!(err.contains("routing has changed"), "got: {err}");
    assert!(err.contains("`t1` ran on [small]"), "names the task: {err}");
}

fn parked_run_with_gate(tag: &str, cmd: &str) -> (ScratchTree, PathBuf, String) {
    parked_run_with_config(tag, &gate_config(cmd))
}

fn gate_config(cmd: &str) -> String {
    format!("{PARKED_RUN_CONFIG}\n[[gates]]\nname = \"check\"\ncmd = \"{cmd}\"\n")
}

fn resume_answering(repo: &Path, run_id: &str, effect: Effect) -> RunReport {
    let source = source(vec![effect], vec![ReviewBehavior::Pass]);
    let answers = ScriptedAnswers::new(vec![Answer::Answered {
        text: "carry on".to_owned(),
    }]);
    resume_harness(
        &resume_options(repo, run_id),
        &Harness {
            adapters: &source,
            answers: Some(&answers),
            sleeper: None,
        },
    )
    .expect("resume")
}

#[test]
fn resume_runs_the_gates_the_run_recorded_not_todays() {
    let (_tree, repo, run_id) = parked_run_with_gate("gaterecorded", "git --version");

    fs::write(
        repo.join("upstroke.toml"),
        gate_config("git frobnicate-not-a-command"),
    )
    .expect("edit config");

    let resumed = resume_answering(&repo, &run_id, Effect::EditFile);
    assert_eq!(resumed.outcome(), RunOutcome::Complete, "{resumed:?}");
    assert!(
        committed(&resumed, "t1"),
        "the recorded gate ran, not the one in today's config: {resumed:?}"
    );

    let warning = resumed
        .warnings
        .iter()
        .find(|w| w.contains("differ from the ones this run recorded"))
        .unwrap_or_else(|| panic!("no gate-difference warning: {:?}", resumed.warnings));
    assert!(
        warning.contains(
            "`check` runs `git --version` and today's config says `git \
                              frobnicate-not-a-command`"
        ),
        "names the edit: {warning}"
    );
    assert!(
        warning.contains("Start a new run to adopt them"),
        "and what to do about it: {warning}"
    );

    assert_eq!(resumed.gates, ["check"]);
}

#[test]
fn a_resume_with_a_gate_record_is_not_refused_over_todays_gates_section() {
    let (_tree, repo, run_id) = parked_run_with_gate("gate_record_lenient", "git --version");
    fs::write(
        repo.join("upstroke.toml"),
        format!(
            "{PARKED_RUN_CONFIG}\n[[gates]]\nname = \"check\"\ncmd = \"git --version\"\n\
             timeout_secs = 0\ntimeout_sec = 3600\n\
             [[gates]]\nname = \"Check\"\ncmd = \"git frobnicate-not-a-command\"\n"
        ),
    )
    .expect("edit config");

    let resumed = resume_answering(&repo, &run_id, Effect::EditFile);
    assert_eq!(resumed.outcome(), RunOutcome::Complete, "{resumed:?}");
    assert!(
        committed(&resumed, "t1"),
        "the recorded gate ran, not today's zero-timeout or failing one: {resumed:?}"
    );
    assert_eq!(
        resumed.gates,
        ["check"],
        "the report describes the recorded gate"
    );
    for shape in [
        "timeout_secs must be at least 1",
        "has unknown key `timeout_sec`",
        "repeats the name `Check` of entry 1",
    ] {
        assert!(
            resumed
                .warnings
                .iter()
                .any(|w| w.contains(shape) && w.contains("recorded")),
            "each shape is announced, with the record named as what runs (`{shape}`): {:?}",
            resumed.warnings
        );
    }
}

#[test]
fn a_resume_with_no_gate_record_refuses_the_gate_shapes_a_fresh_run_refuses() {
    let (_tree, repo, run_id) = parked_run_with_gate("gate_record_absent", "git --version");
    let paths = paths_of(&repo, &run_id);
    strip_event_data_field(&paths, "run_started", "gate_cmds");
    let before = fs::read_to_string(paths.events()).expect("the log before");
    assert!(
        events::recorded_gates(&events_of(&repo, &run_id)).is_none(),
        "the fixture must have no gate record left"
    );
    fs::write(
        repo.join("upstroke.toml"),
        format!(
            "{PARKED_RUN_CONFIG}\n[[gates]]\nname = \"check\"\ncmd = \"git --version\"\n\
             timeout_sec = 3600\n"
        ),
    )
    .expect("edit config");

    let source = source(vec![Effect::EditFile], vec![ReviewBehavior::Pass]);
    let error = resume_with(&resume_options(&repo, &run_id), &source)
        .expect_err("a resume whose gates come from this file refuses it");
    let message = error.to_string();
    assert!(
        message.contains("[[gates]] entry 1") && message.contains("`timeout_sec`"),
        "names the entry and the key: {message}"
    );

    let after = fs::read_to_string(paths.events()).expect("the log after");
    assert_eq!(after, before, "a refusal before the lock appends nothing");

    fs::write(repo.join("upstroke.toml"), gate_config("git --version")).expect("edit config");
    let resumed = resume_answering(&repo, &run_id, Effect::EditFile);
    assert_eq!(resumed.outcome(), RunOutcome::Complete, "{resumed:?}");
    assert!(committed(&resumed, "t1"), "{resumed:?}");
    assert!(
        events::recorded_gates(&events_of(&repo, &run_id)).is_some(),
        "the resume recorded the gates it settled on"
    );
}

#[test]
fn the_report_labels_gates_from_the_record_not_todays_config() {
    let (_tree, repo, run_id) = parked_run_with_gate("gatelabel", "git --version");

    fs::write(repo.join("upstroke.toml"), PARKED_RUN_CONFIG).expect("edit config");

    let resumed = resume_answering(&repo, &run_id, Effect::EditFile);
    assert_eq!(resumed.gates, ["check"], "the recorded gate ran");
    assert!(
        resumed.gates_from_config,
        "and is labelled as the record has it, not as today's config would"
    );

    let replayed = replay_of(&repo, &run_id).report();
    assert_eq!(replayed.gates, resumed.gates);
    assert_eq!(replayed.gates_from_config, resumed.gates_from_config);
}

#[test]
fn a_resume_whose_gates_did_not_move_says_nothing_about_them() {
    let (_tree, repo, run_id) = parked_run_with_gate("gateunmoved", "git --version");
    let resumed = resume_answering(&repo, &run_id, Effect::EditFile);
    assert_eq!(resumed.outcome(), RunOutcome::Complete, "{resumed:?}");
    assert!(
        !resumed
            .warnings
            .iter()
            .any(|w| w.contains("gates") && w.contains("recorded")),
        "an untouched config must warn about nothing: {:?}",
        resumed.warnings
    );
}

#[test]
fn a_gate_difference_is_described_without_inventing_edits() {
    let gate = |name: &str, cmd: &str| GateSummary {
        name: name.to_owned(),
        cmd: cmd.to_owned(),
        timeout: Duration::from_secs(600),
        shell: crate::gates::ShellKind::Sh,
    };
    let check = gate("check", "cargo test");
    let only_check = std::slice::from_ref(&check);

    assert_eq!(
        gates_differ(only_check, only_check),
        None,
        "identical gates are not a difference"
    );

    let added =
        gates_differ(only_check, &[check.clone(), gate("check", "true")]).expect("a difference");
    assert!(
        added.contains("`check` (`true`) is in today's config and not in the record"),
        "names the added gate: {added}"
    );
    assert!(
        !added.contains("different order"),
        "and does not invent a reorder: {added}"
    );

    let removed = gates_differ(&[check.clone(), gate("check", "cargo clippy")], only_check)
        .expect("a difference");
    assert!(
        removed.contains("`check` (`cargo clippy`) is in the record and not in today's config"),
        "names the removed gate: {removed}"
    );

    let edited = gates_differ(only_check, &[gate("check", "true")]).expect("a difference");
    assert!(
        edited.contains("`check` runs `cargo test` and today's config says `true`"),
        "{edited}"
    );

    let renamed = gates_differ(only_check, &[gate("verify", "cargo test")]).expect("a difference");
    assert!(
        renamed.contains("`check` (`cargo test`) is in the record"),
        "{renamed}"
    );
    assert!(
        renamed.contains("`verify` (`cargo test`) is in today's config"),
        "{renamed}"
    );

    let reshelled = gates_differ(
        only_check,
        &[GateSummary {
            shell: crate::gates::ShellKind::Bash,
            ..check.clone()
        }],
    )
    .expect("a difference");
    assert!(
        reshelled.contains("`check` runs under `sh` and today's config says `bash`"),
        "{reshelled}"
    );

    let other = gate("test", "cargo test");
    let reordered =
        gates_differ(&[check.clone(), other.clone()], &[other, check]).expect("a difference");
    assert!(reordered.contains("in a different order"), "{reordered}");
    assert!(
        !reordered.contains("not in the record"),
        "nothing came or went: {reordered}"
    );
}

#[test]
fn a_log_that_predates_the_gate_record_rederives_and_says_what_it_can() {
    let (_tree, repo, run_id) = parked_run_with_gate("oldgatelog", "git --version");
    strip_run_started_field(&paths_of(&repo, &run_id), "gate_cmds");

    fs::write(
        repo.join("upstroke.toml"),
        format!("{PARKED_RUN_CONFIG}\n[[gates]]\nname = \"renamed\"\ncmd = \"git --version\"\n"),
    )
    .expect("edit config");

    let resumed = resume_answering(&repo, &run_id, Effect::EditFile);
    assert_eq!(resumed.outcome(), RunOutcome::Complete, "{resumed:?}");
    let warning = resumed
        .warnings
        .iter()
        .find(|w| w.contains("predates the gate record"))
        .unwrap_or_else(|| panic!("no warning: {:?}", resumed.warnings));
    assert!(
        warning.contains("the gate names have moved"),
        "an old log still knows this much: {warning}"
    );
    assert!(
        warning.contains("recorded [check]") && warning.contains("resolves [renamed]"),
        "and says which: {warning}"
    );
}

#[test]
fn the_resume_that_rederives_an_old_logs_gates_records_them_for_the_next_one() {
    let (_tree, repo, run_id) = parked_run_with_gate("oldgateestablish", "git --version");
    strip_run_started_field(&paths_of(&repo, &run_id), "gate_cmds");

    let first = resume_answering(&repo, &run_id, Effect::NoEdit);
    assert_eq!(first.outcome(), RunOutcome::Parked, "{first:?}");
    assert!(
        first
            .warnings
            .iter()
            .any(|w| w.contains("predates the gate record")),
        "the first resume re-derived: {:?}",
        first.warnings
    );

    let paths = paths_of(&repo, &run_id);
    let mut log_warnings = Vec::new();
    let logged = events::read_all(&paths.events(), &mut log_warnings).expect("log");
    let established = events::recorded_gates(&logged).expect("the resume recorded its gates");
    assert_eq!(established.len(), 1);
    assert_eq!(established[0].cmd, "git --version");

    fs::write(
        repo.join("upstroke.toml"),
        gate_config("git frobnicate-not-a-command"),
    )
    .expect("edit config");

    let second = resume_answering(&repo, &run_id, Effect::EditFile);
    assert_eq!(second.outcome(), RunOutcome::Complete, "{second:?}");
    assert!(
        committed(&second, "t1"),
        "the established gate ran, not the weakened one: {second:?}"
    );

    assert!(
        second
            .warnings
            .iter()
            .any(|w| w.contains("differ from the ones this run recorded")),
        "{:?}",
        second.warnings
    );
    assert!(
        !second
            .warnings
            .iter()
            .any(|w| w.contains("predates the gate record")),
        "the log is no longer speechless about its gates: {:?}",
        second.warnings
    );
}

#[test]
fn an_old_gateless_log_is_not_warned_at_about_nothing() {
    let (_tree, repo, run_id) = parked_run("oldgatelessslog");
    strip_run_started_field(&paths_of(&repo, &run_id), "gate_cmds");

    let resumed = resume_answering(&repo, &run_id, Effect::EditFile);
    assert!(
        !resumed.warnings.iter().any(|w| w.contains("gate")),
        "nothing to say: {:?}",
        resumed.warnings
    );
}

#[test]
fn resume_refuses_when_the_run_branch_is_gone() {
    let (_tree, repo, run_id) = parked_run("branchgone");
    let branch = git_in(&repo, &["rev-parse", "--abbrev-ref", "HEAD"])
        .trim()
        .to_owned();
    git_in(&repo, &["switch", "-q", "main"]);
    git_in(&repo, &["branch", "-q", "-D", &branch]);

    let err = resume_err(&repo, &run_id);
    assert!(err.contains("no longer exists"), "got: {err}");
}

#[test]
fn resume_refuses_to_switch_over_uncommitted_work() {
    let (_tree, repo, run_id) = parked_run("dirtyelsewhere");
    git_in(&repo, &["switch", "-q", "main"]);
    fs::write(
        repo.join("my-own-work.txt"),
        "not the engine's to discard\n",
    )
    .expect("file");

    let err = resume_err(&repo, &run_id);
    assert!(err.contains("Commit or stash"), "got: {err}");
    assert!(
        repo.join("my-own-work.txt").exists(),
        "a refusal must not destroy the work it refused over"
    );
}

#[test]
fn resume_refuses_a_run_that_already_finished_or_halted() {
    let (_tree, repo) = temp_engine_repo("finished");
    let complete = fake(Effect::EditFile);
    let report = run_with(&options(&repo), &complete).expect("run");
    assert_eq!(report.outcome(), RunOutcome::Complete);
    let err = resume_err(&repo, &report.run_id);
    assert!(err.contains("already completed"), "got: {err}");

    let (_tree, repo) = temp_engine_repo("halted");
    seed(
        &repo,
        "## Doomed\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = source(vec![Effect::NoEdit], vec![ReviewBehavior::Pass]);
    let answers = ScriptedAnswers::new(vec![Answer::Declined]);
    let report = run_harness(
        &opts,
        &Harness {
            adapters: &source,
            answers: Some(&answers),
            sleeper: None,
        },
    )
    .expect("run");
    assert_eq!(report.outcome(), RunOutcome::Halted);
    let err = resume_err(&repo, &report.run_id);
    assert!(err.contains("halted at `t1`"), "got: {err}");
}

#[test]
fn resume_refuses_while_another_process_holds_the_run() {
    let (_tree, repo, run_id) = parked_run("locked");
    let paths = paths_of(&repo, &run_id);
    let _held = RunLock::acquire(&paths.public).expect("hold it");

    let err = resume_err(&repo, &run_id);
    assert!(err.contains("already driving run"), "got: {err}");
}

#[test]
fn an_unknown_run_id_lists_what_is_there() {
    let (_tree, repo, _) = parked_run("unknownid");
    let err = resume_err(&repo, "01NOPE");
    assert!(err.contains("known runs"), "got: {err}");
}

#[test]
fn status_reports_a_live_run_and_the_ledger_reads_from_the_log() {
    let (_tree, repo) = temp_engine_repo("statusledger");
    let source = fake(Effect::EditFile);
    let report = run_with(&options(&repo), &source).expect("run");

    let loaded = replay_of(&repo, &report.run_id);
    assert!(!loaded.running, "nothing holds a finished run");
    assert!(!loaded.interrupted_run());

    let rendered = crate::status::render(&loaded);
    assert!(rendered.contains("ledger:"), "{rendered}");
    assert!(rendered.contains("t1"), "{rendered}");
    assert!(
        rendered.contains("per-pool drain: no pool is connected"),
        "{rendered}"
    );
    assert!(
        rendered.contains(&loaded.paths.private.display().to_string()),
        "and points at where the transcripts actually are"
    );

    assert!(
        (loaded.report().total_cost_usd - report.total_cost_usd).abs() < 1e-9,
        "{} vs {}",
        loaded.report().total_cost_usd,
        report.total_cost_usd
    );

    let paths = paths_of(&repo, &report.run_id);
    let _held = RunLock::acquire(&paths.public).expect("claim the finished run");
    let claimed = replay_of(&repo, &report.run_id);
    assert!(!claimed.running, "a finished run is not running");
    assert!(claimed.held, "but something does hold it");
    let rendered = crate::status::render(&claimed);
    assert!(
        rendered.contains("another process holds this run"),
        "{rendered}"
    );
    assert!(rendered.contains("run complete"), "{rendered}");
    assert!(!rendered.contains("run in progress"), "{rendered}");
}

#[test]
fn following_a_finished_run_replays_it_and_stops() {
    let (_tree, repo) = temp_engine_repo("follow");
    let source = fake(Effect::EditFile);
    let report = run_with(&options(&repo), &source).expect("run");

    let loaded = replay_of(&repo, &report.run_id);
    let sleeper = RecordingSleeper::default();
    let mut out: Vec<u8> = Vec::new();
    crate::status::follow(&loaded, &sleeper, Duration::ZERO, 2, &mut out).expect("follow");
    let text = String::from_utf8(out).expect("utf8");

    assert!(text.contains("run"), "{text}");
    assert!(text.contains("t1: committed"), "{text}");
    assert!(
        text.contains("run finished"),
        "it stops at the ending rather than idling: {text}"
    );
    assert!(
        sleeper.waits().is_empty(),
        "a finished run needs no waiting at all"
    );
    for line in text.lines() {
        assert!(!line.is_empty());
    }
}

#[test]
fn follow_ignores_a_terminal_marker_superseded_by_resume() {
    let (_tree, repo) = temp_engine_repo("followresume");
    let source = fake(Effect::EditFile);
    let report = run_with(&options(&repo), &source).expect("run");
    let paths = paths_of(&repo, &report.run_id);
    let mut warnings = Vec::new();
    let mut log = events::EventLog::open(EventSite::LegacyOpenLog, &paths.events(), &mut warnings)
        .expect("open log");
    log.append(
        EventSite::LegacyAppend,
        EventBody::RunResumed {
            data: events::RunResumed {
                head_sha: "second-epoch".to_owned(),
                interrupted_attempts: 0,
                discarded: Vec::new(),
                gates: None,
                effort_policy: None,
                reviews: None,
                chains: None,
                normalized_plan_digest: None,
            },
        },
    )
    .expect("resume marker");
    log.append(
        EventSite::LegacyAppend,
        EventBody::RunFinished {
            data: events::RunFinished {
                outcome: events::RunOutcome::Complete,
                halted_at: None,
                committed: 1,
                parked: 0,
            },
        },
    )
    .expect("second finish");
    drop(log);

    let loaded = replay_of(&repo, &report.run_id);
    let sleeper = RecordingSleeper::default();
    let mut out = Vec::new();
    crate::status::follow(&loaded, &sleeper, Duration::ZERO, 2, &mut out).expect("follow");
    let text = String::from_utf8(out).expect("utf8");
    assert!(text.contains("resumed at second-epo"), "{text}");
    assert_eq!(
        text.matches("run finished").count(),
        2,
        "the historical finish must not truncate the later epoch: {text}"
    );
}

#[test]
fn follow_waits_at_held_historical_terminal_until_resume_marker() {
    struct ResumeOnSleep {
        events: PathBuf,
        lock: Mutex<Option<RunLock>>,
    }

    impl Sleeper for ResumeOnSleep {
        fn sleep(&self, _: Duration) {
            let Ok(mut lock) = self.lock.lock() else {
                return;
            };
            if lock.is_none() {
                return;
            }
            let mut warnings = Vec::new();
            let mut log =
                events::EventLog::open(EventSite::LegacyOpenLog, &self.events, &mut warnings)
                    .expect("log");
            log.append(
                EventSite::LegacyAppend,
                EventBody::RunResumed {
                    data: events::RunResumed {
                        head_sha: "resumed-head".to_owned(),
                        interrupted_attempts: 0,
                        discarded: Vec::new(),
                        gates: None,
                        effort_policy: None,
                        reviews: None,
                        chains: None,
                        normalized_plan_digest: None,
                    },
                },
            )
            .expect("resume marker");
            log.append(
                EventSite::LegacyAppend,
                EventBody::RunFinished {
                    data: events::RunFinished {
                        outcome: events::RunOutcome::Complete,
                        halted_at: None,
                        committed: 1,
                        parked: 0,
                    },
                },
            )
            .expect("new terminal");
            drop(log);
            drop(lock.take());
        }
    }

    let (_tree, repo) = temp_engine_repo("followheldterminal");
    let report = run_with(&options(&repo), &fake(Effect::EditFile)).expect("run");
    let paths = paths_of(&repo, &report.run_id);
    let loaded = replay_of(&repo, &report.run_id);
    let sleeper = ResumeOnSleep {
        events: paths.events(),
        lock: Mutex::new(Some(
            RunLock::acquire(&paths.public).expect("resume owns lock before marker"),
        )),
    };
    let mut out = Vec::new();
    crate::status::follow(&loaded, &sleeper, Duration::ZERO, 1, &mut out).expect("follow");
    let text = String::from_utf8(out).expect("utf8");
    assert!(text.contains("resumed at resumed-he"), "{text}");
    assert_eq!(text.matches("run finished").count(), 2, "{text}");
}

#[test]
fn transcripts_live_outside_the_workspace_and_survive_a_rollback() {
    let (_tree, repo) = temp_engine_repo("private");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 2 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));

    let adapters = source(
        vec![Effect::NoEdit, Effect::EditFile],
        vec![ReviewBehavior::Pass],
    );
    let report = run_with(&opts, &adapters).expect("run");
    let paths = paths_of(&repo, &report.run_id);

    for private in [paths.transcripts(), paths.reviews(), paths.settings()] {
        assert!(
            !private.starts_with(&repo),
            "{} must be outside the workspace",
            private.display()
        );
        assert!(
            fs::read_dir(&private).into_iter().flatten().count() > 0,
            "{} kept its contents across the rollback",
            private.display()
        );
    }

    assert!(paths.events().starts_with(&repo));
    assert!(paths.questions().starts_with(&repo));

    let in_repo = repo.join(".upstroke").join("runs").join(&report.run_id);
    for leaked in ["transcripts", "reviews", "settings", "gates"] {
        assert!(
            !in_repo.join(leaked).exists(),
            "{leaked}/ must not exist inside the workspace"
        );
    }
}

fn strip_run_started_field(paths: &RunPaths, field: &str) {
    let text = fs::read_to_string(paths.events()).expect("log");
    let mut stripped = false;
    let rewritten: Vec<String> = text
        .lines()
        .map(|line| {
            let mut value: serde_json::Value =
                serde_json::from_str(line).expect("every line is an event");
            if value.get("event").and_then(|e| e.as_str()) == Some("run_started") {
                if let Some(data) = value.get_mut("data").and_then(|d| d.as_object_mut()) {
                    data.remove(field)
                        .unwrap_or_else(|| panic!("the run recorded no `{field}`"));
                    stripped = true;
                }
            }
            value.to_string()
        })
        .collect();
    assert!(
        stripped,
        "the log has no run_started to strip `{field}` from"
    );
    fs::write(paths.events(), format!("{}\n", rewritten.join("\n"))).expect("rewrite");
}

fn rewrite_run_started_as_schema_one(paths: &RunPaths, absent: &[&str]) {
    let text = fs::read_to_string(paths.events()).expect("log");
    let mut rewritten_start = false;
    let rewritten: Vec<String> = text
        .lines()
        .map(|line| {
            let mut value: serde_json::Value =
                serde_json::from_str(line).expect("every line is an event");
            if value.get("event").and_then(|event| event.as_str()) == Some("run_started") {
                let data = value
                    .get_mut("data")
                    .and_then(serde_json::Value::as_object_mut)
                    .expect("run_started data");
                data.insert("schema".to_owned(), serde_json::Value::from(1));
                data.remove("normalized_plan_digest");
                for field in absent {
                    data.remove(*field)
                        .unwrap_or_else(|| panic!("the run recorded no `{field}`"));
                }
                for chain in data
                    .get_mut("chains")
                    .and_then(serde_json::Value::as_array_mut)
                    .expect("run_started chains")
                {
                    chain
                        .as_object_mut()
                        .expect("chain object")
                        .remove("bindings")
                        .expect("a schema-2 run records chain bindings");
                }
                rewritten_start = true;
            }
            value.to_string()
        })
        .collect();
    assert!(rewritten_start, "the log has no run_started event");
    fs::write(paths.events(), format!("{}\n", rewritten.join("\n"))).expect("rewrite");
}

fn rewrite_run_started_as_schema_two(paths: &RunPaths) {
    rewrite_run_started_as_schema_two_missing_review_fields(paths, &["pass_timeout_secs"]);
}

fn rewrite_run_started_as_schema_two_missing_review_fields(
    paths: &RunPaths,
    absent_review_fields: &[&str],
) {
    let text = fs::read_to_string(paths.events()).expect("log");
    let mut rewritten_start = false;
    let rewritten: Vec<String> = text
        .lines()
        .map(|line| {
            let mut value: serde_json::Value =
                serde_json::from_str(line).expect("every line is an event");
            if value.get("event").and_then(|event| event.as_str()) == Some("run_started") {
                let data = value
                    .get_mut("data")
                    .and_then(serde_json::Value::as_object_mut)
                    .expect("run_started data");
                data.insert("schema".to_owned(), serde_json::Value::from(2));
                data.remove("normalized_plan_digest");
                let reviews = data
                    .get_mut("reviews")
                    .and_then(serde_json::Value::as_object_mut)
                    .expect("recorded review plan");
                for field in absent_review_fields {
                    reviews
                        .remove(*field)
                        .unwrap_or_else(|| panic!("current review plan records `{field}`"));
                }
                rewritten_start = true;
            }
            value.to_string()
        })
        .collect();
    assert!(rewritten_start, "the log has no run_started event");
    fs::write(paths.events(), format!("{}\n", rewritten.join("\n"))).expect("rewrite");
}

fn strip_event_field(paths: &RunPaths, event: &str, field: &str) {
    rewrite_event_field(paths, event, field, false);
}

fn strip_event_data_field(paths: &RunPaths, event: &str, field: &str) {
    rewrite_event_field(paths, event, field, true);
}

fn rewrite_event_field(paths: &RunPaths, event: &str, field: &str, nested_in_data: bool) {
    let text = fs::read_to_string(paths.events()).expect("log");
    let mut stripped = false;
    let rewritten: Vec<String> = text
        .lines()
        .map(|line| {
            let mut value: serde_json::Value =
                serde_json::from_str(line).expect("every line is an event");
            if value.get("event").and_then(serde_json::Value::as_str) == Some(event) {
                let object = if nested_in_data {
                    value
                        .get_mut("data")
                        .and_then(serde_json::Value::as_object_mut)
                        .expect("event data")
                } else {
                    value.as_object_mut().expect("event object")
                };
                object
                    .remove(field)
                    .unwrap_or_else(|| panic!("{event} records `{field}`"));
                stripped = true;
            }
            value.to_string()
        })
        .collect();
    assert!(stripped, "the log has no `{event}.{field}` to strip");
    fs::write(paths.events(), format!("{}\n", rewritten.join("\n"))).expect("rewrite");
}

fn truncate_log_before(paths: &RunPaths, event: &str) {
    let text = fs::read_to_string(paths.events()).expect("log");
    let lines: Vec<&str> = text.lines().collect();
    let cut = lines
        .iter()
        .position(|line| line.contains(&format!("\"{event}\"")))
        .unwrap_or_else(|| panic!("the run recorded no {event}"));
    fs::write(paths.events(), format!("{}\n", lines[..cut].join("\n"))).expect("truncate");
}

fn truncate_log_after(paths: &RunPaths, event: &str) {
    let text = fs::read_to_string(paths.events()).expect("log");
    let lines: Vec<&str> = text.lines().collect();
    let cut = lines
        .iter()
        .position(|line| line.contains(&format!("\"{event}\"")))
        .unwrap_or_else(|| panic!("the run recorded no {event}"))
        + 1;
    fs::write(paths.events(), format!("{}\n", lines[..cut].join("\n"))).expect("truncate");
}

fn prepared_commit_of(paths: &RunPaths) -> events::PreparedCommit {
    let mut warnings = Vec::new();
    events::read_all(&paths.events(), &mut warnings)
        .expect("read prepared settlement")
        .into_iter()
        .find_map(|event| match event.body {
            EventBody::AttemptFinished {
                prepared_commit: Some(prepared),
                ..
            } => Some(*prepared),
            _ => None,
        })
        .expect("successful settlement records its prepared commit")
}

fn recreate_prepared_pin(repo: &Path, prepared: &events::PreparedCommit, target: &str) {
    let zero = "0".repeat(target.len());
    git_in(repo, &["update-ref", &prepared.pin_ref, target, &zero]);
}

#[test]
fn resume_adopts_the_commit_it_made_but_never_recorded() {
    let (_tree, repo) = temp_engine_repo("adoptcommit");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let report = run_with(&opts, &fake(Effect::EditFile)).expect("run");
    let run_id = report.run_id.clone();
    let paths = paths_of(&repo, &run_id);
    let sha = git_in(&repo, &["rev-parse", "HEAD"]).trim().to_owned();

    truncate_log_before(&paths, "task_committed");

    let source = fake(Effect::EditFile);
    let (resumed, state) = resume_harness_inner(
        &resume_options(&repo, &run_id),
        &Harness {
            adapters: &source,
            answers: None,
            sleeper: None,
        },
    )
    .expect("resume");

    assert_eq!(resumed.outcome(), RunOutcome::Complete, "{resumed:?}");
    assert!(committed(&resumed, "t1"), "adopted rather than redone");
    assert_eq!(
        git_in(&repo, &["rev-parse", "HEAD"]).trim(),
        sha,
        "and the branch was left exactly where it stood"
    );
    assert_eq!(
        git_in(&repo, &["rev-list", "--count", "main..HEAD"]).trim(),
        "1",
        "one commit, not a second one for the same work"
    );
    assert_eq!(
        task(&resumed, "t1").attempts.len(),
        1,
        "the attempt that passed was not spent again: {resumed:?}"
    );
    assert!(
        resumed
            .warnings
            .iter()
            .any(|w| w.contains("adopted commit")),
        "and the operator is told: {:?}",
        resumed.warnings
    );
    assert_live_equals_replay(&repo, &state, &resumed);
}

#[test]
fn resume_recovers_every_prepared_commit_ref_crash_prefix() {
    for (tag, reset_to_parent, recreate_pin) in [
        ("prepared-same-head", true, true),
        ("prepared-head-with-pin", false, true),
        ("prepared-head-no-pin", false, false),
    ] {
        let (_tree, repo) = temp_engine_repo(tag);
        seed(
            &repo,
            "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
            Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
        );
        let mut opts = options(&repo);
        opts.config_path = Some(repo.join("upstroke.toml"));
        let report = run_with(&opts, &fake(Effect::EditFile)).expect("run");
        let paths = paths_of(&repo, &report.run_id);
        let prepared = prepared_commit_of(&paths);
        truncate_log_before(&paths, "task_committed");
        if reset_to_parent {
            git_in(&repo, &["reset", "-q", "--soft", &prepared.parent_sha]);
        }
        if recreate_pin {
            let target = prepared.commit_sha.clone();
            recreate_prepared_pin(&repo, &prepared, &target);
        }

        let source = fake(Effect::EditFile);
        let resumed = resume_harness(
            &resume_options(&repo, &report.run_id),
            &Harness {
                adapters: &source,
                answers: None,
                sleeper: None,
            },
        )
        .expect("recover exact prepared object");
        assert_eq!(
            resumed.outcome(),
            RunOutcome::Complete,
            "{tag}: {resumed:?}"
        );
        assert_eq!(
            git_in(&repo, &["rev-parse", "HEAD"]).trim(),
            prepared.commit_sha,
            "{tag}: the exact reviewed object is published"
        );
        assert_eq!(task(&resumed, "t1").attempts.len(), 1, "{tag}");
        let workspace = Workspace::open(&repo).expect("workspace");
        assert_eq!(
            workspace
                .prepared_pin_target(&prepared.pin_ref)
                .expect("pin lookup"),
            None,
            "{tag}: recovery cleans the private pin"
        );
    }
}

#[test]
fn resume_removes_a_pin_whose_successful_settlement_never_landed() {
    let (_tree, repo) = temp_engine_repo("prepared-orphan");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 2 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let report = run_with(&opts, &fake(Effect::EditFile)).expect("run");
    let paths = paths_of(&repo, &report.run_id);
    let prepared = prepared_commit_of(&paths);
    truncate_log_before(&paths, "attempt_finished");
    git_in(&repo, &["reset", "-q", "--soft", &prepared.parent_sha]);
    let target = prepared.commit_sha.clone();
    recreate_prepared_pin(&repo, &prepared, &target);

    let source = fake(Effect::EditFile);
    let resumed = resume_harness(
        &resume_options(&repo, &report.run_id),
        &Harness {
            adapters: &source,
            answers: None,
            sleeper: None,
        },
    )
    .expect("orphan pin is not a settlement");
    assert_eq!(resumed.outcome(), RunOutcome::Complete, "{resumed:?}");
    assert_eq!(task(&resumed, "t1").attempts.len(), 2);
    assert!(
        resumed
            .warnings
            .iter()
            .any(|warning| warning.contains("removed orphan prepared commit pin")),
        "{:?}",
        resumed.warnings
    );
    assert_eq!(
        Workspace::open(&repo)
            .expect("workspace")
            .prepared_pin_target(&prepared.pin_ref)
            .expect("pin lookup"),
        None
    );
}

#[test]
fn resume_refuses_a_substituted_prepared_pin_without_deleting_it() {
    let (_tree, repo) = temp_engine_repo("prepared-pin-mismatch");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let report = run_with(&opts, &fake(Effect::EditFile)).expect("run");
    let paths = paths_of(&repo, &report.run_id);
    let prepared = prepared_commit_of(&paths);
    truncate_log_before(&paths, "task_committed");
    git_in(&repo, &["reset", "-q", "--soft", &prepared.parent_sha]);
    recreate_prepared_pin(&repo, &prepared, &prepared.parent_sha);

    let err = resume_err(&repo, &report.run_id);
    assert!(err.contains("not pinned"), "{err}");
    assert_eq!(
        Workspace::open(&repo)
            .expect("workspace")
            .prepared_pin_target(&prepared.pin_ref)
            .expect("pin lookup")
            .as_deref(),
        Some(prepared.parent_sha.as_str()),
        "refusal never deletes the substituted target"
    );
    assert_eq!(
        git_in(&repo, &["rev-parse", "HEAD"]).trim(),
        prepared.parent_sha,
        "HEAD remains at the recorded parent"
    );
}

#[test]
fn resume_refuses_symbolic_run_ref_at_already_published_prepared_prefix() {
    let (_tree, repo) = temp_engine_repo("prepared-symbolic-run-ref");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let report = run_with(&opts, &fake(Effect::EditFile)).expect("run");
    let paths = paths_of(&repo, &report.run_id);
    let prepared = prepared_commit_of(&paths);
    truncate_log_before(&paths, "task_committed");
    recreate_prepared_pin(&repo, &prepared, &prepared.commit_sha);

    git_in(&repo, &["branch", "victim", prepared.commit_sha.as_str()]);
    git_in(
        &repo,
        &[
            "symbolic-ref",
            prepared.branch_ref.as_str(),
            "refs/heads/victim",
        ],
    );
    let events_before = fs::read(paths.events()).expect("event bytes before refusal");
    let victim_before = git_in(&repo, &["rev-parse", "refs/heads/victim"]);

    let error = resume_err(&repo, &report.run_id);
    assert!(error.contains("itself symbolic"), "{error}");
    assert_eq!(
        fs::read(paths.events()).expect("event bytes after refusal"),
        events_before,
        "refusal happens before task_committed or any other repair append"
    );
    assert_eq!(
        Workspace::open(&repo)
            .expect("workspace")
            .prepared_pin_target(&prepared.pin_ref)
            .expect("pin lookup")
            .as_deref(),
        Some(prepared.commit_sha.as_str()),
        "refusal preserves the durable prepared pin"
    );
    assert_eq!(
        git_in(&repo, &["rev-parse", "refs/heads/victim"]),
        victim_before,
        "the symbolic run ref never advances or deletes its victim"
    );
    assert_eq!(
        git_in(
            &repo,
            &["symbolic-ref", "--no-recurse", prepared.branch_ref.as_str(),],
        )
        .trim(),
        "refs/heads/victim",
        "refusal preserves the substituted symbolic run ref for inspection"
    );
}

#[test]
fn recovered_prepared_commit_precedes_unrelated_answer_defect_repair() {
    let (_tree, repo) = temp_engine_repo("prepared-before-repair");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let report = run_with(&opts, &fake(Effect::EditFile)).expect("run");
    let paths = paths_of(&repo, &report.run_id);
    truncate_log_before(&paths, "task_committed");

    let question_id = QuestionId::from("q-before-success");
    let question = Question {
        id: question_id.clone(),
        kind: QuestionKind::Unblock,
        affected_tasks: vec![TaskId::from("t1")],
        context: "an earlier question".to_owned(),
        options: Vec::new(),
    };
    let inserted = [
        events::Event::now(EventBody::QuestionRaised {
            task: "t1".to_owned(),
            data: Box::new(events::QuestionRaised {
                question: question.clone(),
            }),
        }),
        events::Event::now(EventBody::TaskParked {
            task: "t1".to_owned(),
            data: events::TaskParked {
                question: question_id.to_string(),
                refund_attempt: false,
            },
        }),
        events::Event::now(EventBody::QuestionAnswered {
            data: events::QuestionAnswered {
                question: question_id,
                answer: Answer::Answered {
                    text: "continue".to_owned(),
                },
                decline_halts_run: None,
                via: "answer-file".to_owned(),
            },
        }),
    ];
    let mut lines: Vec<String> = fs::read_to_string(paths.events())
        .expect("log")
        .lines()
        .map(str::to_owned)
        .collect();
    let before_attempt = lines
        .iter()
        .position(|line| line.contains("\"attempt_started\""))
        .expect("attempt start");
    lines.splice(
        before_attempt..before_attempt,
        inserted
            .iter()
            .map(|event| serde_json::to_string(event).expect("event json")),
    );
    fs::write(paths.events(), format!("{}\n", lines.join("\n"))).expect("insert prefix");

    let source = fake(Effect::EditFile);
    let resumed = resume_harness(
        &resume_options(&repo, &report.run_id),
        &Harness {
            adapters: &source,
            answers: None,
            sleeper: None,
        },
    )
    .expect("resume closes settlement before repairing older metadata");
    assert_eq!(resumed.outcome(), RunOutcome::Complete, "{resumed:?}");
    let logged = events_of(&repo, &report.run_id);
    let settlement = logged
        .iter()
        .rposition(|event| matches!(event.body, EventBody::AttemptFinished { .. }))
        .expect("settlement");
    assert!(
        matches!(
            logged.get(settlement + 1).map(|event| &event.body),
            Some(EventBody::TaskCommitted { task, .. }) if task == "t1"
        ),
        "task_committed must immediately close the prepared settlement"
    );
    assert!(
        logged
            .iter()
            .skip(settlement + 2)
            .any(|event| matches!(event.body, EventBody::DesignDefect { .. })),
        "the older answer repair still lands after the commit"
    );
    events::replay(logged, vec!["t1".to_owned()], &paths.events())
        .expect("the repaired log remains replayable");
}

#[test]
fn resume_refuses_an_arbitrary_tree_with_the_same_parent_and_subject() {
    let (_tree, repo) = temp_engine_repo("adoptforeign");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let report = run_with(&opts, &fake(Effect::EditFile)).expect("run");
    truncate_log_before(&paths_of(&repo, &report.run_id), "task_committed");
    let subject = git_in(&repo, &["show", "-s", "--format=%s", "HEAD"]);
    fs::write(repo.join("foreign.txt"), "not reviewed\n").expect("foreign tree");
    git_in(&repo, &["add", "foreign.txt"]);
    git_in(&repo, &["commit", "-q", "--amend", "--no-edit"]);
    assert_eq!(
        git_in(&repo, &["show", "-s", "--format=%s", "HEAD"]),
        subject,
        "the substituted commit deliberately has the expected subject"
    );

    let err = resume_err(&repo, &report.run_id);
    assert!(err.contains("record ends at"), "got: {err}");
}

#[test]
fn legacy_success_without_prepared_identity_is_never_adopted_by_subject() {
    let (_tree, repo) = temp_engine_repo("legacy-subject");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let report = run_with(&opts, &fake(Effect::EditFile)).expect("run");
    let paths = paths_of(&repo, &report.run_id);
    truncate_log_before(&paths, "task_committed");
    rewrite_run_started_as_schema_two(&paths);
    strip_event_field(&paths, "attempt_finished", "prepared_commit");

    let err = resume_err(&repo, &report.run_id);
    assert!(err.contains("subject alone"), "{err}");
    assert_eq!(
        git_in(&repo, &["rev-list", "--count", "main..HEAD"]).trim(),
        "1",
        "refusal preserves the plausible legacy commit"
    );
}

#[test]
fn resume_writes_where_the_run_recorded_not_where_defaults_point() {
    let (_tree, repo) = temp_engine_repo("privatedir");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let report = run_with(&opts, &fake(Effect::EditFile)).expect("run");
    let run_id = report.run_id.clone();
    let recorded = paths_of(&repo, &run_id);

    truncate_log_before(&recorded, "attempt_finished");
    git_in(&repo, &["reset", "-q", "--hard", "HEAD~1"]);

    let mut resume = resume_options(&repo, &run_id);
    resume.private_root = None;
    let source = fake(Effect::EditFile);
    let resumed = resume_harness(
        &resume,
        &Harness {
            adapters: &source,
            answers: None,
            sleeper: None,
        },
    )
    .expect("resume");
    assert_eq!(resumed.outcome(), RunOutcome::Complete, "{resumed:?}");
    assert!(
        recorded.transcripts().join("00-t1-2.json").is_file(),
        "the resumed attempt wrote under {}",
        recorded.transcripts().display()
    );
}

#[test]
fn resume_makes_a_stale_question_payload_agree_with_the_log() {
    let (_tree, repo) = temp_engine_repo("stalepayload");
    seed(
        &repo,
        "## Doomed\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some(
            "[interaction]\nmode = \"never\"\n\n\
                 [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n",
        ),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let doomed = source(vec![Effect::NoEdit], vec![ReviewBehavior::Pass]);
    let report = run_with(&opts, &doomed).expect("run");
    let run_id = report.run_id.clone();
    let first = report.questions[0].question.id.to_string();

    crate::answer::answer(
        &repo,
        &first,
        crate::answer::Reply::Text("try again".to_owned()),
    )
    .expect("answer");

    let retry = source(vec![Effect::NoEdit], vec![ReviewBehavior::Pass]);
    let resumed = resume_with(&resume_options(&repo, &run_id), &retry).expect("resume");
    assert_eq!(resumed.outcome(), RunOutcome::Parked, "{resumed:?}");

    let questions = rundir::public_dir(&repo, &run_id).join("questions");
    let path = questions.join(format!("{first}.json"));
    let mut record: QuestionRecord =
        serde_json::from_str(&fs::read_to_string(&path).expect("read")).expect("parse");
    record.answer = None;
    interaction::write_question(&questions, &record).expect("rewrite");
    crate::answer::answer(&repo, &first, crate::answer::Reply::Decline)
        .expect("a stale payload is exactly what makes a second answer look acceptable");

    let source = fake(Effect::EditFile);
    resume_with(&resume_options(&repo, &run_id), &source).expect("second resume");

    let record: QuestionRecord =
        serde_json::from_str(&fs::read_to_string(&path).expect("read")).expect("parse");
    assert!(
        !record.is_open(),
        "the payload agrees with the log again, so nobody answers it twice"
    );
}

#[test]
fn a_private_run_collision_refuses_before_early_error_cleanup() {
    let root =
        rundir::scratch_tree::acquire(&std::env::temp_dir(), "coordinator-private-collision")
            .expect("owned regression fixture");
    let repo = root.path().join("second-repo");
    fs::create_dir(&repo).expect("create the second repository");
    git_in(&repo, &["init", "-q", "-b", "main"]);
    git_in(&repo, &["config", "user.email", "test@upstroke.local"]);
    git_in(&repo, &["config", "user.name", "upstroke tests"]);
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    // If the coordinator mistakenly adopts the occupied private root, this
    // branch conflict sends it through the destructive early-error arm.
    git_in(&repo, &["branch", "upstroke"]);

    const RUN_ID: &str = "01M191Y2PSVX78RNEP31D23K02";
    let private_root = root.path().join("shared-home");
    let first =
        rundir::RunPaths::with_private_root(&root.path().join("first-repo"), RUN_ID, &private_root);
    first
        .create_fresh()
        .expect("the first run reserves its private half");
    let transcript = first.transcripts().join("t1-1.json");
    fs::write(&transcript, b"first run evidence").expect("first run transcript");
    let mut opts = options(&repo);
    opts.private_root = Some(private_root);
    let source = fake(Effect::EditFile);
    let harness = Harness::new(&source);
    let contained = crate::runner::host::contain_write_command(&mut crate::agent::proc::NoHooks)
        .expect("the test coordinator is contained");
    let error = coordinator::run_harness_inner_with_id(
        &opts,
        &harness,
        &crate::runner::host::HostRunner::new(),
        &contained,
        || RUN_ID.to_owned(),
    )
    .expect_err("the second run refuses the occupied private half");
    assert_eq!(
        fs::read(&transcript).expect("early-error cleanup did not delete the first run"),
        b"first run evidence"
    );
    assert!(
        error
            .to_string()
            .contains("reserve fresh private run directory"),
        "the allocation boundary must refuse before branch creation: {error}"
    );
    assert!(
        rundir::scratch_tree::proves_absent(&opts.paths(RUN_ID).public),
        "the refused coordinator leaves no public run directory"
    );
}

#[test]
fn a_run_that_never_started_leaves_no_directory_behind() {
    let (_tree, repo) = temp_engine_repo("husk");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );

    git_in(&repo, &["branch", "upstroke"]);

    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = fake(Effect::EditFile);
    run_with(&opts, &source).expect_err("the run branch cannot be created");

    assert_eq!(
        rundir::latest_run(&repo),
        None,
        "no husk left behind to shadow the next run"
    );
}

struct BacklogAnswers {
    repo: PathBuf,
    used: Mutex<bool>,
}

impl AnswerSource for BacklogAnswers {
    fn id(&self) -> &'static str {
        "backlog"
    }

    fn resolve(&self, question: &Question) -> Result<Answer, UpstrokeError> {
        let Ok(mut used) = self.used.lock() else {
            return Ok(Answer::Unanswered);
        };
        if *used {
            return Ok(Answer::Unanswered);
        }
        *used = true;
        let run = rundir::latest_run(&self.repo).expect("a run");
        let dir = rundir::public_dir(&self.repo, &run).join("questions");
        let other = fs::read_dir(&dir)
            .expect("questions dir")
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                name.strip_suffix(".json").map(str::to_owned)
            })
            .find(|id| id.as_str() != question.id.as_str());
        if let Some(other) = other {
            let _ = crate::answer::answer(
                &self.repo,
                &other,
                crate::answer::Reply::Text("write src/other.rs".to_owned()),
            );
        }
        Ok(Answer::Answered {
            text: "write src/widget.rs".to_owned(),
        })
    }
}

#[test]
fn a_typed_answer_survives_another_question_being_answered_at_the_same_time() {
    let (_tree, repo) = temp_engine_repo("backlog");
    seed(
        &repo,
        "## First\n<!-- upstroke: id=t1 kind=implement depends= -->\n\n\
             ## Second\n<!-- upstroke: id=t2 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));

    let source = source(
        vec![Effect::NoEdit, Effect::NoEdit, Effect::EditFile],
        vec![ReviewBehavior::Pass],
    );
    let answers = BacklogAnswers {
        repo: repo.clone(),
        used: Mutex::new(false),
    };
    let report = run_harness(
        &opts,
        &Harness {
            adapters: &source,
            answers: Some(&answers),
            sleeper: None,
        },
    )
    .expect("run");

    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:?}");
    for id in ["t1", "t2"] {
        assert!(committed(&report, id), "{id} was released: {report:?}");
    }
}

#[test]
fn an_answer_file_that_changes_nothing_does_not_spin_the_scheduler() {
    let (_tree, repo) = temp_engine_repo("nullanswer");
    seed(
        &repo,
        "## Doomed\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some(
            "[interaction]\nmode = \"never\"\n\n\
                 [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n",
        ),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = source(vec![Effect::NoEdit], vec![ReviewBehavior::Pass]);
    let report = run_with(&opts, &source).expect("run");
    assert_eq!(report.outcome(), RunOutcome::Parked);
    let run_id = report.run_id.clone();
    let question = report.questions[0].question.id.clone();

    let answers = rundir::public_dir(&repo, &run_id).join("answers");
    fs::create_dir_all(&answers).expect("answers dir");
    interaction::write_answer(
        &answers,
        &question,
        &interaction::AnswerRecord::unattributed(Answer::Unanswered),
    )
    .expect("write");

    let source = fake(Effect::EditFile);
    let resumed = resume_with(&resume_options(&repo, &run_id), &source).expect("resume");
    assert_eq!(
        resumed.outcome(),
        RunOutcome::Parked,
        "still waiting on a real answer, and the run ended saying so: {resumed:?}"
    );
}

#[test]
fn a_legacy_answer_file_with_a_foreign_column_still_parks_rather_than_erroring() {
    let (_tree, repo) = temp_engine_repo("foreigncolumn");
    seed(
        &repo,
        "## Doomed\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some(
            "[interaction]\nmode = \"never\"\n\n\
                 [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n",
        ),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = source(vec![Effect::NoEdit], vec![ReviewBehavior::Pass]);
    let report = run_with(&opts, &source).expect("run");
    assert_eq!(report.outcome(), RunOutcome::Parked);
    let run_id = report.run_id.clone();
    let question = report.questions[0].question.id.clone();

    let answers = rundir::public_dir(&repo, &run_id).join("answers");
    fs::create_dir_all(&answers).expect("answers dir");
    fs::write(
        interaction::answer_path(&answers, &question),
        r#"{"answer":"unanswered","citation":7}"#,
    )
    .expect("a file the base tolerated: a column the answer does not know, of a foreign type");

    let source = fake(Effect::EditFile);
    let resumed = resume_with(&resume_options(&repo, &run_id), &source)
        .expect("the legacy reader ignores the column, as the base did, and the resume runs");
    assert_eq!(
        resumed.outcome(),
        RunOutcome::Parked,
        "still waiting on a real answer, not erroring on the column: {resumed:?}"
    );
}

struct LockReleasingSleeper {
    waits: Mutex<u32>,
    release_after: u32,
    lock: Mutex<Option<RunLock>>,
}

impl LockReleasingSleeper {
    fn new(lock: RunLock, release_after: u32) -> Self {
        Self {
            waits: Mutex::new(0),
            release_after,
            lock: Mutex::new(Some(lock)),
        }
    }

    fn waits(&self) -> u32 {
        self.waits.lock().map(|w| *w).unwrap_or(0)
    }
}

impl Sleeper for LockReleasingSleeper {
    fn sleep(&self, _: Duration) {
        let Ok(mut waits) = self.waits.lock() else {
            return;
        };
        *waits += 1;
        if *waits == self.release_after {
            if let Ok(mut lock) = self.lock.lock() {
                drop(lock.take());
            }
        }
    }
}

#[test]
fn following_waits_out_a_silent_live_run_and_stops_once_it_dies() {
    let (_tree, repo) = temp_engine_repo("followlive");
    let source = fake(Effect::EditFile);
    let report = run_with(&options(&repo), &source).expect("run");
    let paths = paths_of(&repo, &report.run_id);

    let text = fs::read_to_string(paths.events()).expect("log");
    let kept: Vec<&str> = text
        .lines()
        .filter(|line| !line.contains("\"run_finished\""))
        .collect();
    fs::write(paths.events(), format!("{}\n", kept.join("\n"))).expect("truncate");

    let loaded = replay_of(&repo, &report.run_id);
    let held = RunLock::acquire(&paths.public).expect("simulate a live engine");
    let sleeper = LockReleasingSleeper::new(held, 5);
    let mut out: Vec<u8> = Vec::new();

    crate::status::follow(&loaded, &sleeper, Duration::ZERO, 1, &mut out).expect("follow");

    assert!(
        sleeper.waits() > 5,
        "watched the live run past its idle budget and stopped once the lock went, \
             instead of timing out its silence: {} sleeps",
        sleeper.waits()
    );
}

fn pools_file(repo: &Path, content: &str) -> PathBuf {
    let dir = private_root_for(repo);
    fs::create_dir_all(&dir).expect("pools dir");
    let path = dir.join("pools.toml");
    fs::write(&path, content).expect("pools file");
    path
}

const CLAUDE_POOL: &str = "[pools.claude-max]\nkind = \"subscription-window\"\nagent = \
                               \"claude-code\"\nsources = [\"signals\", \"self\"]\n";

fn events_of(repo: &Path, run_id: &str) -> Vec<events::Event> {
    let mut ignored = Vec::new();
    events::read_all(&paths_of(repo, run_id).events(), &mut ignored).expect("the log reads")
}

fn budget_events(events: &[events::Event]) -> Vec<&events::BudgetExceeded> {
    events
        .iter()
        .filter_map(|event| match &event.body {
            EventBody::BudgetExceeded { data } => Some(data),
            _ => None,
        })
        .collect()
}

#[test]
fn a_run_budget_stops_the_run_exactly_once_and_survives_replay() {
    let (_tree, repo) = temp_engine_repo("budgetstop");
    seed(
        &repo,
        "## One\n<!-- upstroke: id=t1 kind=implement depends= -->\n\n\
             ## Two\n<!-- upstroke: id=t2 kind=implement depends= -->\n\n\
             ## Three\n<!-- upstroke: id=t3 kind=implement depends= -->\n",
        Some(
            "[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n\n\
                 [budgets]\nrun_usd = 0.05\n",
        ),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = fake(Effect::EditFile);
    let (report, live) = run_harness_inner(
        &opts,
        &Harness {
            adapters: &source,
            answers: None,
            sleeper: None,
        },
    )
    .expect("a budget stop is not an engine error");

    assert_eq!(report.outcome(), RunOutcome::BudgetExceeded, "{report:?}");
    assert!(committed(&report, "t1"));
    let stop = report.budget_stop.as_ref().expect("a recorded stop");
    assert_eq!(stop.budget, events::BudgetKind::Run);
    assert_eq!(stop.task, "t2", "names the task that did not start");
    assert!(stop.spent_usd >= 0.05, "spent: {}", stop.spent_usd);

    let events = events_of(&repo, &report.run_id);
    assert_eq!(
        budget_events(&events).len(),
        1,
        "{:?}",
        budget_events(&events)
    );

    assert!(matches!(task(&report, "t2").status, TaskRunStatus::Skipped));
    assert!(task(&report, "t2").attempts.is_empty());
    assert_live_equals_replay(&repo, &live, &report);
}

#[test]
fn a_task_budget_also_ends_the_run_and_says_which_ceiling_it_was() {
    let (_tree, repo) = temp_engine_repo("taskbudget");
    seed(
        &repo,
        "## One\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some(
            "[routing]\nimplement = { chain = [\"small\", \"mid\"], attempts_per = 1 }\n\n\
                 [budgets]\ntask_usd = 0.005\n",
        ),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));

    let source = source(
        vec![Effect::NoEdit, Effect::EditFile],
        vec![ReviewBehavior::Pass],
    );
    let report = run_with(&opts, &source).expect("run");
    assert_eq!(report.outcome(), RunOutcome::BudgetExceeded, "{report:?}");
    let stop = report.budget_stop.as_ref().expect("a recorded stop");
    assert_eq!(stop.budget, events::BudgetKind::Task);
    assert_eq!(stop.task, "t1");
    assert_eq!(
        task(&report, "t1").attempts.len(),
        1,
        "the escalated attempt never spawned"
    );
    let rendered = report.render();
    assert!(rendered.contains("task_usd"), "{rendered}");
    assert!(rendered.contains("upstroke resume"), "{rendered}");
}

#[test]
fn resuming_with_a_higher_ceiling_continues_the_run_the_budget_stopped() {
    let (_tree, repo) = temp_engine_repo("budgetresume");
    seed(
        &repo,
        "## One\n<!-- upstroke: id=t1 kind=implement depends= -->\n\n\
             ## Two\n<!-- upstroke: id=t2 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    opts.budget_usd = Some(0.05);
    let source = fake(Effect::EditFile);
    let stopped = run_with(&opts, &source).expect("run");
    assert_eq!(stopped.outcome(), RunOutcome::BudgetExceeded);
    assert!(!committed(&stopped, "t2"));

    let mut resume_opts = resume_options(&repo, &stopped.run_id);
    resume_opts.budget_usd = Some(10.0);
    let source = fake(Effect::EditFile);
    let resumed = resume_harness(
        &resume_opts,
        &Harness {
            adapters: &source,
            answers: None,
            sleeper: None,
        },
    )
    .expect("a budget stop is exactly what resume is for");
    assert_eq!(resumed.outcome(), RunOutcome::Complete, "{resumed:?}");
    assert!(committed(&resumed, "t2"));
    assert!(
        resumed.budget_stop.is_none(),
        "the stop the resume got past must not still be reported"
    );
}

#[test]
fn a_resume_that_does_not_raise_the_ceiling_stops_again_rather_than_running_past_it() {
    let (_tree, repo) = temp_engine_repo("budgetresumelow");
    seed(
        &repo,
        "## One\n<!-- upstroke: id=t1 kind=implement depends= -->\n\n\
             ## Two\n<!-- upstroke: id=t2 kind=implement depends= -->\n",
        Some(
            "[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n\n\
                 [budgets]\nrun_usd = 0.05\n",
        ),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = fake(Effect::EditFile);
    let stopped = run_with(&opts, &source).expect("run");
    assert_eq!(stopped.outcome(), RunOutcome::BudgetExceeded);

    let source = fake(Effect::EditFile);
    let again = resume_harness(
        &resume_options(&repo, &stopped.run_id),
        &Harness {
            adapters: &source,
            answers: None,
            sleeper: None,
        },
    )
    .expect("resume");
    assert_eq!(again.outcome(), RunOutcome::BudgetExceeded, "{again:?}");
    assert!(!committed(&again, "t2"));
}

#[test]
fn a_frontier_escalation_over_the_threshold_parks_for_approval_then_runs_it() {
    let (_tree, repo) = temp_engine_repo("approvespend");
    seed(
        &repo,
        "## One\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some(
            "[routing]\nimplement = { chain = [\"mid\", \"frontier\"], attempts_per = 1 }\n\n\
                 [interaction]\nask_before = { frontier_escalation_over_usd = 0.005 }\n",
        ),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = source(
        vec![Effect::NoEdit, Effect::EditFile],
        vec![ReviewBehavior::Pass],
    );
    let scripted = ScriptedAnswers::new(vec![Answer::Answered {
        text: "approve: run the escalated attempt".to_owned(),
    }]);
    let (report, live) = run_harness_inner(
        &opts,
        &Harness {
            adapters: &source,
            answers: Some(&scripted),
            sleeper: None,
        },
    )
    .expect("run");

    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:?}");
    assert!(committed(&report, "t1"));
    let asked: Vec<&QuestionRecord> = report
        .questions
        .iter()
        .filter(|q| q.question.kind == QuestionKind::ApproveSpend)
        .collect();
    assert_eq!(asked.len(), 1, "asked once: {:?}", report.questions);
    assert!(
        asked[0].question.context.contains("frontier"),
        "the question names where the money is going: {}",
        asked[0].question.context
    );
    assert!(
        asked[0].question.context.contains("$0.0100"),
        "and quotes reported spend to date: {}",
        asked[0].question.context
    );

    let tiers: Vec<&str> = task(&report, "t1")
        .attempts
        .iter()
        .map(|a| a.tier.as_str())
        .collect();
    assert_eq!(tiers, ["mid", "frontier"], "{tiers:?}");
    assert_live_equals_replay(&repo, &live, &report);
}

#[test]
fn a_declined_spend_approval_fails_the_task_through_the_halt_policy() {
    let (_tree, repo) = temp_engine_repo("declinespend");
    seed(
        &repo,
        "## One\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some(
            "[routing]\nimplement = { chain = [\"mid\", \"frontier\"], attempts_per = 1 }\n\n\
                 [interaction]\nask_before = { frontier_escalation_over_usd = 0.005 }\n",
        ),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = source(
        vec![Effect::NoEdit, Effect::EditFile],
        vec![ReviewBehavior::Pass],
    );
    let scripted = ScriptedAnswers::new(vec![Answer::Declined]);
    let report = run_harness(
        &opts,
        &Harness {
            adapters: &source,
            answers: Some(&scripted),
            sleeper: None,
        },
    )
    .expect("run");

    assert_eq!(report.outcome(), RunOutcome::Halted, "{report:?}");
    assert!(matches!(
        task(&report, "t1").status,
        TaskRunStatus::Failed {
            kind: FailureKind::Declined,
            ..
        }
    ));
    assert_eq!(
        task(&report, "t1").attempts.len(),
        1,
        "declining must not have spent the frontier attempt"
    );
}

#[test]
fn a_chain_that_starts_at_frontier_never_asks_to_approve_spend() {
    let (_tree, repo) = temp_engine_repo("frontierstart");
    seed(
        &repo,
        "## One\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some(
            "[routing]\nimplement = { chain = [\"frontier\"], attempts_per = 2 }\n\n\
                 [interaction]\nask_before = { frontier_escalation_over_usd = 0.0 }\n",
        ),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = source(
        vec![Effect::NoEdit, Effect::EditFile],
        vec![ReviewBehavior::Pass],
    );
    let report = run_with(&opts, &source).expect("run");
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:?}");
    assert!(
        report
            .questions
            .iter()
            .all(|q| q.question.kind != QuestionKind::ApproveSpend),
        "questions: {:?}",
        report.questions
    );
}

#[test]
fn attempts_are_attributed_to_the_pool_that_paid_them() {
    let (_tree, repo) = temp_engine_repo("poolattrib");
    seed(
        &repo,
        "## One\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some(
            "[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n\n\
                 [routing.effort]\nimplementation = \"xhigh\"\nreview = \"max\"\n",
        ),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    opts.pools_path = Some(pools_file(&repo, CLAUDE_POOL));
    let source = fake(Effect::EditFile);
    let report = run_with(&opts, &source).expect("run");

    let attempt = &task(&report, "t1").attempts[0];
    assert_eq!(attempt.pool.as_deref(), Some("claude-max"));
    assert!(
        attempt
            .reviews
            .iter()
            .all(|r| r.pool.as_deref() == Some("claude-max")),
        "the reviewer's own pool is attributed too: {:?}",
        attempt.reviews
    );

    let drain = &report.pool_drain;
    assert_eq!(drain.len(), 1, "{drain:?}");
    assert_eq!(drain[0].pool, "claude-max");
    assert_eq!(drain[0].attempts, 2, "implementer plus its reviewer");
    let ledger = report.render_ledger();
    assert!(ledger.contains("claude-max"), "{ledger}");

    let events = events_of(&repo, &report.run_id);
    let started = events
        .iter()
        .find_map(|event| match &event.body {
            EventBody::AttemptStarted { data, .. } => Some(data),
            _ => None,
        })
        .expect("worker start was emitted");
    assert_eq!(started.adapter.as_deref(), Some("claude-code"));
    assert_eq!(started.preflight_cli_version.as_deref(), Some("0.0.0-fake"));
    assert_eq!(started.effort, Some(Effort::XHigh));
    assert_eq!(
        started.selection_origin,
        Some(events::SelectionOrigin::Auto)
    );

    let review = events
        .iter()
        .find_map(|event| match &event.body {
            EventBody::AttemptFinished { data, .. } => data.reviews.first(),
            _ => None,
        })
        .expect("review pass actually ran");
    assert_eq!(review.adapter.as_deref(), Some("claude-code"));
    assert_eq!(review.preflight_cli_version.as_deref(), Some("0.0.0-fake"));
    assert_eq!(review.effort, Some(Effort::Max));
    let snapshots: Vec<&events::CapacitySnapshot> = events
        .iter()
        .filter_map(|e| match &e.body {
            EventBody::CapacitySnapshot { data } => Some(data),
            _ => None,
        })
        .collect();
    assert_eq!(snapshots.len(), 1, "one snapshot per run start (§14)");
    assert_eq!(snapshots[0].pools.len(), 1);
    assert_eq!(
        snapshots[0].pools[0].remaining, "unknown",
        "never optimistic: an unmeasured pool is unknown, not full"
    );
}

#[test]
fn a_pinned_live_attempt_records_its_selection_origin() {
    let (_tree, repo) = temp_engine_repo("pinorigin");
    seed(
        &repo,
        "## One\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some(
            "[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n\
                 [[pins]]\ntier = \"small\"\nagent = \"claude-code\"\n\
                 model = \"claude-haiku-4-5\"\neffort = \"max\"\n",
        ),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let report = run_with(&opts, &fake(Effect::EditFile)).expect("run");
    let events = events_of(&repo, &report.run_id);
    let started = events
        .iter()
        .find_map(|event| match &event.body {
            EventBody::AttemptStarted { data, .. } => Some(data),
            _ => None,
        })
        .expect("worker start was emitted");
    assert_eq!(started.selection_origin, Some(events::SelectionOrigin::Pin));
    assert_eq!(started.effort, Some(Effort::Max));
}

#[test]
fn a_rate_limit_marks_its_pool_exhausted_and_a_recovery_retires_the_signal() {
    let (_tree, repo) = temp_engine_repo("poolexhausted");
    seed(
        &repo,
        "## One\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let pools = pools_file(&repo, CLAUDE_POOL);
    opts.pools_path = Some(pools.clone());
    let source = source(
        vec![Effect::RateLimited, Effect::EditFile],
        vec![ReviewBehavior::Pass],
    );
    let report = run_with(&opts, &source).expect("run");
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:?}");

    let events = events_of(&repo, &report.run_id);
    let signals: Vec<&events::PoolExhausted> = events
        .iter()
        .filter_map(|e| match &e.body {
            EventBody::PoolExhausted { data, .. } => Some(data),
            _ => None,
        })
        .collect();
    assert_eq!(signals.len(), 1, "{signals:?}");
    assert_eq!(signals[0].pool, "claude-max");
    assert_eq!(signals[0].agent, "claude-code");

    let signal_at = events
        .iter()
        .position(|e| matches!(e.body, EventBody::PoolExhausted { .. }))
        .expect("the signal is in the log");
    let through_signal = events[..=signal_at].to_vec();
    let mut warnings = Vec::new();
    let cfg = config::load(None, &repo, Some(&pools), &mut warnings).expect("pools");
    let at_the_signal = capacity::estimate(&cfg.pools, &capacity::observe(&through_signal));
    assert_eq!(at_the_signal[0].remaining, capacity::Remaining::Exhausted);
    assert_eq!(at_the_signal[0].confidence, capacity::Confidence::Signal);

    let settled = capacity::estimate(&cfg.pools, &capacity::observe(&events));
    assert_ne!(
        settled[0].remaining,
        capacity::Remaining::Exhausted,
        "{}",
        settled[0].describe()
    );
}

#[test]
fn reviewer_rate_limit_retires_recovered_implementer_pool_live() {
    let (_tree, repo) = temp_engine_repo("reviewerlimitretiresworker");
    seed(&repo, FRONTIER_AUTH_PLAN, Some(SECOND_OPINION_CONFIG));
    let pools = pools_file(
        &repo,
        "[pools.claude-max]\nkind = \"subscription-window\"\nagent = \"claude-code\"\n\
             sources = [\"signals\"]\n\n[pools.copilot-window]\nkind = \"subscription-window\"\n\
             agent = \"copilot\"\nsources = [\"signals\"]\n",
    );
    let mut opts = cross_vendor_opts(&repo);
    opts.pools_path = Some(pools);
    let source = cross_vendor(
        vec![
            Effect::RateLimited,
            Effect::EditFile,
            Effect::RateLimited,
            Effect::EditFile,
        ],
        vec![ReviewBehavior::Pass],
        vec![ReviewBehavior::RateLimited, ReviewBehavior::Pass],
    );
    let report = run_with(&opts, &source).expect("outages eventually recover");
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:?}");

    let signals: Vec<String> = events_of(&repo, &report.run_id)
        .iter()
        .filter_map(|event| match &event.body {
            EventBody::PoolExhausted { data, .. } => Some(data.pool.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        signals,
        ["claude-max", "copilot-window", "claude-max"],
        "the reviewer outage must not leave the successfully serving worker pool retired forever"
    );
}

#[test]
fn the_budget_flag_is_validated_like_the_config_key() {
    let (_tree, repo) = temp_engine_repo("budgetflag");
    seed(
        &repo,
        "## One\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    for bad in [0.0, -5.0, f64::NAN] {
        let mut opts = options(&repo);
        opts.config_path = Some(repo.join("upstroke.toml"));
        opts.budget_usd = Some(bad);
        let source = fake(Effect::EditFile);
        let err = run_with(&opts, &source).expect_err("a meaningless ceiling must refuse");
        assert!(
            err.to_string().contains("not a spendable ceiling"),
            "--budget {bad}: {err}"
        );
    }

    let branch = git_in(&repo, &["rev-parse", "--abbrev-ref", "HEAD"]);
    assert_eq!(branch.trim(), "main", "refused before branching");
}

#[test]
fn a_spend_approval_is_not_fed_back_to_the_agent_as_an_instruction() {
    let (_tree, repo) = temp_engine_repo("approvalfeedback");
    seed(
        &repo,
        "## One\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some(
            "[routing]\nimplement = { chain = [\"mid\", \"frontier\"], attempts_per = 1 }\n\n\
                 [interaction]\nask_before = { frontier_escalation_over_usd = 0.005 }\n",
        ),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = source(
        vec![Effect::NoEdit, Effect::EditFile],
        vec![ReviewBehavior::Pass],
    );
    let scripted = ScriptedAnswers::new(vec![Answer::Answered {
        text: "approve: run the escalated attempt".to_owned(),
    }]);
    let report = run_harness(
        &opts,
        &Harness {
            adapters: &source,
            answers: Some(&scripted),
            sleeper: None,
        },
    )
    .expect("run");
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:?}");

    let frontier = &source.adapter.runs()[1].prompt;
    assert!(
        !frontier.contains("approve: run the escalated attempt"),
        "the approval reached the implementer as guidance:
{frontier}"
    );

    assert!(
        !frontier.contains("instruction from a person"),
        "and no human-instruction framing at all:
{frontier}"
    );
}

#[test]
fn picking_an_option_is_an_un_park_and_not_a_decision() {
    let (_tree, repo) = temp_engine_repo("cannedoption");
    seed(
        &repo,
        "## One\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));

    let source = source(
        vec![Effect::AskQuestion, Effect::EditFile],
        vec![ReviewBehavior::Pass],
    );
    let scripted = ScriptedAnswers::new(vec![Answer::Answered {
        text: question_options(QuestionKind::Clarify)[0].clone(),
    }]);
    let report = run_harness(
        &opts,
        &Harness {
            adapters: &source,
            answers: Some(&scripted),
            sleeper: None,
        },
    )
    .expect("run");
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:?}");

    let runs = source.adapter.runs();
    let retry = &runs
        .iter()
        .filter(|r| !r.prompt.contains("DATA UNDER REVIEW"))
        .nth(1)
        .expect("a second implementer attempt")
        .prompt;
    assert!(
        !retry.contains("answer in your own words"),
        "the option label reached the implementer as guidance:\n{retry}"
    );
    assert!(
        !retry.contains("instruction from a person"),
        "and with no human-instruction framing at all:\n{retry}"
    );

    for review in runs
        .iter()
        .filter(|r| r.prompt.contains("DATA UNDER REVIEW"))
    {
        assert!(
            !review.prompt.contains("answer in your own words"),
            "the option label reached the reviewer as a decision:\n{}",
            review.prompt
        );
    }
}

#[test]
fn one_outage_records_one_signal_however_many_deferrals_it_causes() {
    let (_tree, repo) = temp_engine_repo("onesignal");
    seed(
        &repo,
        "## One\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    opts.pools_path = Some(pools_file(&repo, CLAUDE_POOL));

    let source = source(
        vec![
            Effect::RateLimited,
            Effect::RateLimited,
            Effect::RateLimited,
            Effect::EditFile,
        ],
        vec![ReviewBehavior::Pass],
    );
    let report = run_with(&opts, &source).expect("run");
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:?}");
    let signals = events_of(&repo, &report.run_id)
        .iter()
        .filter(|e| matches!(e.body, EventBody::PoolExhausted { .. }))
        .count();
    assert_eq!(
        signals, 1,
        "one outage is one fact; the deferrals are already on `task_deferred`"
    );
}

#[test]
fn a_budget_stop_hands_back_a_clean_tree() {
    let (_tree, repo) = temp_engine_repo("budgetdirty");
    seed(
        &repo,
        "## One\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 2 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));

    opts.budget_usd = Some(0.05);
    let rejected = source(vec![Effect::EditFile], vec![ReviewBehavior::Fail]);
    let stopped = run_with(&opts, &rejected).expect("run");
    assert_eq!(stopped.outcome(), RunOutcome::BudgetExceeded, "{stopped:?}");

    let workspace = Workspace::open(&repo).expect("open");
    let left = workspace.uncommitted_summary().expect("status");
    assert!(
        left.is_empty(),
        "a clean stop left the rejected attempt in the operator's tree: {left:?}"
    );

    let mut resume_opts = resume_options(&repo, &stopped.run_id);
    resume_opts.budget_usd = Some(10.0);
    let accepted = source(vec![Effect::EditFile], vec![ReviewBehavior::Pass]);
    let resumed = resume_harness(
        &resume_opts,
        &Harness {
            adapters: &accepted,
            answers: None,
            sleeper: None,
        },
    )
    .expect("resume");
    assert_eq!(resumed.outcome(), RunOutcome::Complete, "{resumed:?}");
    assert!(committed(&resumed, "t1"));
    assert!(
        !resumed
            .warnings
            .iter()
            .any(|w| w.contains("discarded") && w.contains("uncommitted")),
        "nothing should have been left for the resume to discard: {:?}",
        resumed.warnings
    );
}

fn priced_and_unpriced_attempts() -> TaskReport {
    let attempt = |cost: Option<f64>| AttemptRecord {
        attempt: 1,
        tier: "frontier".to_owned(),
        model: "m".to_owned(),
        pool: None,
        resumed: false,
        duration: Duration::ZERO,
        cost_usd: cost,
        reviews: Vec::new(),
        session_id: None,
        usage: None,
        failure: None,
    };
    let mut task = task_report_costing(Some(0.2020), None);
    task.status = TaskRunStatus::Committed {
        sha: "abc123".to_owned(),
    };
    task.attempts = vec![attempt(None), attempt(Some(0.2020))];
    task
}

fn empty_report() -> RunReport {
    RunReport {
        run_id: "01RUN".to_owned(),
        branch: "b".to_owned(),
        gates: Vec::new(),
        gates_from_config: false,
        warnings: Vec::new(),
        tasks: Vec::new(),
        halted_at: None,
        questions: Vec::new(),
        budget_stop: None,
        total_cost_usd: 0.0,
        pool_drain: Vec::new(),
        running: false,
        interrupted: false,
    }
}

#[test]
fn an_unpriced_worker_reads_as_unreported_rather_than_free() {
    let mut task = task_report_costing(None, None);
    task.id = "t1".to_owned();
    task.model = "gpt-5.6-sol".to_owned();
    task.status = TaskRunStatus::Committed {
        sha: "abc123".to_owned(),
    };

    task.attempts = vec![AttemptRecord {
        attempt: 1,
        tier: "frontier".to_owned(),
        model: "gpt-5.6-sol".to_owned(),
        pool: None,
        resumed: false,
        duration: Duration::from_secs(46),
        cost_usd: None,
        reviews: Vec::new(),
        session_id: None,
        usage: None,
        failure: None,
    }];
    let report = RunReport {
        tasks: vec![task],
        ..empty_report()
    };

    let rendered = report.render();
    assert!(rendered.contains("gpt-5.6-sol $?"), "{rendered}");
    let task_line = rendered
        .lines()
        .find(|l| l.contains("t1: committed"))
        .expect("the task line");
    assert!(
        !task_line.contains("$0.0000"),
        "unreported spend rendered as free: {task_line}"
    );

    assert!(
        report.total_is_floor(),
        "an unpriced worker makes it a floor"
    );
    assert!(rendered.contains("total: $0.0000?"), "{rendered}");
    let ledger = report.render_ledger();
    assert!(ledger.contains("total $0.0000?"), "{ledger}");
    assert!(ledger.contains("a floor, not a total"), "{ledger}");

    let row = ledger
        .lines()
        .find(|l| l.trim_start().starts_with("t1"))
        .expect("the ledger row");
    assert!(row.contains('—'), "{row}");

    let mut mixed = priced_and_unpriced_attempts();
    mixed.id = "t2".to_owned();
    let row = RunReport {
        tasks: vec![mixed],
        ..empty_report()
    }
    .render_ledger();
    let row = row
        .lines()
        .find(|l| l.trim_start().starts_with("t2"))
        .expect("the ledger row");
    assert!(row.contains("$0.2020?"), "a floor must say so: {row}");

    let mut priced = report;
    priced.tasks[0].cost_usd = Some(0.2020);
    assert!(priced.render().contains("$0.2020"), "{}", priced.render());
}

#[test]
fn a_status_from_a_newer_upstroke_does_not_fail_the_whole_report() {
    let text = r#"{
          "run_id": "01RUN", "branch": "b", "gates": [], "gates_from_config": false,
          "warnings": [], "halted_at": null, "questions": [], "total_cost_usd": 0.0,
          "tasks": [
            {"id": "t1", "title": "One", "model": "m",
             "status": {"status": "teleported", "destination": "elsewhere"},
             "duration": {"secs": 0, "nanos": 0}, "cost_usd": null,
             "review_models": [], "review_cost_usd": null,
             "review_cost_incomplete": false, "session_id": null, "attempts": []},
            {"id": "t2", "title": "Two", "model": "m",
             "status": {"status": "committed", "sha": "abc123"},
             "duration": {"secs": 0, "nanos": 0}, "cost_usd": null,
             "review_models": [], "review_cost_usd": null,
             "review_cost_incomplete": false, "session_id": null, "attempts": []}
          ]
        }"#;

    let report: RunReport =
        serde_json::from_str(text).expect("one unknown status must not sink the report");
    assert!(matches!(task(&report, "t1").status, TaskRunStatus::Unknown));

    assert!(
        matches!(&task(&report, "t2").status, TaskRunStatus::Committed { sha } if sha == "abc123")
    );
    let rendered = report.render();
    assert!(rendered.contains("t1: status not recognised"), "{rendered}");
    assert!(rendered.contains("t2: committed abc123"), "{rendered}");
}

#[test]
fn a_report_for_a_dead_run_never_says_a_task_is_running() {
    let task = Task {
        id: TaskId::from("t1"),
        kind: TaskKind::Implement,
        title: "One".to_owned(),
        body: String::new(),
        depends_on: Vec::new(),
        acceptance: Vec::new(),
        path_hints: Vec::new(),
        suggested_tier: None,
        min_tier: None,
        artifacts_in: Vec::new(),
        artifacts_out: Vec::new(),
    };
    let mid_attempt = Progress {
        in_flight: Some(events::InFlight {
            attempt: 2,
            rung: 1,
            tier: "mid".to_owned(),
            model: "claude-sonnet-5".to_owned(),
            profile: "mid-claude-sonnet-5".to_owned(),
            pool: None,
        }),
        ..Progress::default()
    };

    for state in [TaskState::Pending, TaskState::Deferred] {
        let dead = task_report(&task, &state, &mid_attempt, false);
        assert!(
            matches!(dead.status, TaskRunStatus::Skipped),
            "a report for an ended run claimed a live attempt from {state:?}: {:?}",
            dead.status
        );
        let live = task_report(&task, &state, &mid_attempt, true);
        assert!(
            matches!(live.status, TaskRunStatus::Running { .. }),
            "and a live one still reports it: {:?}",
            live.status
        );
    }
}

#[test]
fn a_budget_stop_keeps_its_outcome_while_a_resume_holds_the_lock() {
    let (_tree, repo) = temp_engine_repo("resumewindow");
    seed(
        &repo,
        "## One\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 2 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    opts.budget_usd = Some(0.05);
    let rejected = source(vec![Effect::EditFile], vec![ReviewBehavior::Fail]);
    let stopped = run_with(&opts, &rejected).expect("run");
    assert_eq!(stopped.outcome(), RunOutcome::BudgetExceeded, "{stopped:?}");

    let paths = paths_of(&repo, &stopped.run_id);
    let _held = RunLock::acquire(&paths.public).expect("the resume claims it");

    let seen = replay_of(&repo, &stopped.run_id);
    assert!(
        !seen.running,
        "a run that recorded its finish is not running"
    );
    assert!(seen.held, "though a resume does hold it");
    let out = crate::status::render(&seen);
    assert!(out.contains("run stopped at its budget"), "{out}");
    assert!(out.contains("upstroke resume"), "{out}");
    assert!(out.contains("another process holds this run"), "{out}");
    assert!(!out.contains("run in progress"), "{out}");
}

#[test]
fn a_budget_stop_survives_a_git_that_cannot_clean_the_tree() {
    let (_tree, repo) = temp_engine_repo("budgetjam");
    seed(
        &repo,
        "## One\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 2 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    opts.budget_usd = Some(0.05);
    let rejected = source(
        vec![Effect::JamCleanupAfterReview],
        vec![ReviewBehavior::Fail],
    );
    let stopped = run_with(&opts, &rejected).expect("the ceiling still ends the run");

    assert_eq!(
        stopped.outcome(),
        RunOutcome::BudgetExceeded,
        "a failed cleanup relabelled the stop: {stopped:?}"
    );
    let stop = stopped
        .budget_stop
        .as_ref()
        .expect("the ceiling is on the record even when the cleanup failed");
    assert_eq!(stop.budget, events::BudgetKind::Run);

    assert!(
        stopped
            .warnings
            .iter()
            .any(|w| w.contains("could not be cleaned")),
        "the dirty tree went unmentioned: {:?}",
        stopped.warnings
    );
}

#[test]
fn a_budget_stop_survives_a_stale_decline_file() {
    let (_tree, repo) = temp_engine_repo("budgetdecline");
    seed(
        &repo,
        "## One\n<!-- upstroke: id=t1 kind=implement depends= -->\n\n\
             ## Two\n<!-- upstroke: id=t2 kind=implement depends= -->\n",
        Some(
            "[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n\n\
                 [budgets]\nrun_usd = 0.05\n",
        ),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = fake(Effect::EditFile);
    let report = run_with(&opts, &source).expect("run");
    assert_eq!(report.outcome(), RunOutcome::BudgetExceeded, "{report:?}");
    assert!(report.halted_at.is_none(), "nothing failed: {report:?}");
}

#[derive(Debug, Clone)]
struct RoutedProcess {
    role: crate::runner::ExecutionRole,
    program: String,
    invocation: String,
    workspace: PathBuf,
    agent: Option<String>,
    slotted: bool,

    stdin: String,
}

struct RecordingRunner {
    inner: crate::runner::host::HostRunner,
    seen: Mutex<Vec<RoutedProcess>>,
}

impl RecordingRunner {
    fn new() -> Self {
        Self {
            inner: crate::runner::host::HostRunner::new(),
            seen: Mutex::new(Vec::new()),
        }
    }

    fn seen(&self) -> Vec<RoutedProcess> {
        self.seen.lock().expect("recorder").clone()
    }
}

impl crate::runner::contract::tests::InlineRunner for RecordingRunner {
    fn run_inline(
        &self,
        request: &crate::runner::RunnerRequest,
    ) -> Result<ProcessOutput, crate::runner::RunnerError> {
        self.seen.lock().expect("recorder").push(RoutedProcess {
            role: request.role.clone(),
            program: request.command.program.clone(),
            invocation: request.invocation.render(),
            workspace: request.workspace.clone(),
            agent: request.agent.as_ref().map(|id| id.as_str().to_owned()),
            slotted: request.role.is_slotted(),
            stdin: String::from_utf8_lossy(&request.command.stdin).into_owned(),
        });
        crate::runner::Runner::run_blocking(&self.inner, request)
    }
}

fn program_stem(program: &str) -> String {
    Path::new(program)
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

#[test]
fn the_legacy_engine_routes_every_process_through_the_runner() {
    let (_tree, repo) = temp_engine_repo("routed");
    seed(
        &repo,
        "## One\n<!-- upstroke: id=t1 kind=implement depends= -->\n\n\
             ## Two\n<!-- upstroke: id=t2 kind=implement depends= -->\n",
        Some(
            "[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n\n\
                 [[gates]]\nname = \"first\"\ncmd = \"echo gate-one\"\n\n\
                 [[gates]]\nname = \"second\"\ncmd = \"echo gate-two\"\n",
        ),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = source(vec![Effect::EditFile], vec![ReviewBehavior::Pass]);
    let runner = RecordingRunner::new();
    let report = run_harness_on(
        &opts,
        &Harness {
            adapters: &source,
            answers: None,
            sleeper: None,
        },
        &runner,
    )
    .expect("run");

    assert!(committed(&report, "t1"), "report: {report:?}");
    assert!(committed(&report, "t2"), "report: {report:?}");
    let seen = runner.seen();

    let expected_ids = vec![
        "k0.g0.a1.worker.o0",
        "k0.g0.a1.gate0.o0",
        "k0.g0.a1.gate1.o0",
        "k0.g0.a1.review_pass0.o0",
        "k1.g0.a1.worker.o0",
        "k1.g0.a1.gate0.o0",
        "k1.g0.a1.gate1.o0",
        "k1.g0.a1.review_pass0.o0",
    ];
    let ids: Vec<&str> = seen.iter().map(|p| p.invocation.as_str()).collect();
    assert_eq!(ids, expected_ids, "recorded: {seen:#?}");
    assert_eq!(
        seen.len(),
        2 * (1 + 2 + 1),
        "two tasks x (worker + two gates + one review)"
    );
    assert_eq!(
        ids.iter().collect::<std::collections::BTreeSet<_>>().len(),
        ids.len(),
        "two processes of one run share an identity"
    );
    assert!(
        ids.iter().all(|id| id.contains(".g0.")),
        "the legacy engine assigns legacy-scoped values: generation 0"
    );

    use crate::runner::ExecutionRole;
    let by_role = |role: &ExecutionRole| seen.iter().filter(|p| &p.role == role).count();
    assert_eq!(by_role(&ExecutionRole::Implement), 2);
    assert_eq!(by_role(&ExecutionRole::Gate), 4);
    assert_eq!(by_role(&ExecutionRole::Review), 2);
    for process in &seen {
        match process.role {
            ExecutionRole::Implement | ExecutionRole::Review => {
                assert!(process.slotted, "{process:?}");
                assert_eq!(process.agent.as_deref(), Some("claude-code"), "{process:?}");
            }
            ExecutionRole::Gate => {
                assert!(!process.slotted, "{process:?}");
                assert_eq!(process.agent, None, "{process:?}");
            }
            ExecutionRole::Probe(_) => panic!("the legacy engine probes nothing: {process:?}"),
        }
    }

    let worker_stdin = &seen
        .iter()
        .find(|p| p.role == ExecutionRole::Implement)
        .expect("a worker")
        .stdin;
    assert!(
        worker_stdin.contains("## One") || worker_stdin.contains("One"),
        "the worker prompt is delivered on stdin: {worker_stdin:?}"
    );
    assert!(
        worker_stdin.contains("Acceptance") || worker_stdin.len() > 200,
        "and it is the materialized prompt, not a token: {} bytes",
        worker_stdin.len()
    );
    let review_stdin = &seen
        .iter()
        .find(|p| p.role == ExecutionRole::Review)
        .expect("a review")
        .stdin;
    assert!(
        review_stdin.contains("READ-ONLY"),
        "the review prompt is delivered on stdin: {review_stdin:?}"
    );
    assert_ne!(
        worker_stdin, review_stdin,
        "a worker and a judge are not sent the same prompt"
    );
    for gate in seen.iter().filter(|p| p.role == ExecutionRole::Gate) {
        assert!(gate.stdin.is_empty(), "a gate reads no stdin: {gate:?}");
    }

    let shell_probes = |seen: &[RoutedProcess]| {
        seen.iter()
            .filter(|p| p.role == ExecutionRole::Probe(crate::runner::ProbeTarget::Shell))
            .count()
    };
    assert_eq!(
        shell_probes(&seen),
        0,
        "the legacy engine ran a shell probe"
    );
    crate::runner::host::run_shell_probe(
        &runner,
        crate::gates::ShellKind::native(),
        repo.clone(),
        crate::runner::InvocationId::probe(crate::runner::ProbeTarget::Shell, 0)
            .expect("the shell probe identity"),
    )
    .expect("the recorded shell runs `exit 0`");
    assert_eq!(
        shell_probes(&runner.seen()),
        1,
        "the recorder cannot see a shell probe, so the zero above proved nothing"
    );

    assert!(
        seen.iter().all(|p| program_stem(&p.program) != "git"),
        "a git process went through the Runner: {seen:#?}"
    );
    assert!(
        !git_in(&repo, &["log", "--oneline", &report.branch]).is_empty(),
        "the run's branch has commits, so authoritative Git did run"
    );

    let worker = seen
        .iter()
        .find(|p| p.role == ExecutionRole::Implement)
        .expect("a worker");
    assert!(
        crate::util::same_path(&worker.workspace, &repo),
        "the worker runs in the repo root: {} is not {}",
        worker.workspace.display(),
        repo.display()
    );
    for process in seen.iter().filter(|p| p.role != ExecutionRole::Implement) {
        assert!(
            !crate::util::same_path(&process.workspace, &repo),
            "a gate or reviewer judged the live worktree: {process:?}"
        );
        assert!(process.workspace.is_absolute(), "{process:?}");
    }
}

#[test]
fn a_retried_attempt_with_two_passes_and_a_reask_assigns_every_identity_from_production() {
    let (_tree, repo) = temp_engine_repo("identities");
    seed(
        &repo,
        FRONTIER_AUTH_PLAN,
        Some(
            "[routing]\n\
             implement = { chain = [\"frontier\"], attempts_per = 2 }\n\n\
             [[routing.overrides]]\n\
             paths = [\"src/auth/**\"]\n\
             second_opinion = \"different-vendor\"\n\n\
             [[gates]]\nname = \"first\"\ncmd = \"echo gate-one\"\n\n\
             [[gates]]\nname = \"second\"\ncmd = \"echo gate-two\"\n",
        ),
    );
    let source = cross_vendor(
        vec![Effect::NoEdit, Effect::EditFile],
        vec![ReviewBehavior::Unparseable, ReviewBehavior::Pass],
        vec![ReviewBehavior::Pass],
    );
    let runner = RecordingRunner::new();
    let report = run_harness_on(
        &cross_vendor_opts(&repo),
        &Harness {
            adapters: &source,
            answers: None,
            sleeper: None,
        },
        &runner,
    )
    .expect("run");
    assert!(committed(&report, "t1"), "report: {report:?}");

    let seen = runner.seen();
    let ids: Vec<&str> = seen.iter().map(|p| p.invocation.as_str()).collect();
    assert_eq!(
        ids,
        vec![
            "k0.g0.a1.worker.o0",
            "k0.g0.a2.worker.o0",
            "k0.g0.a2.gate0.o0",
            "k0.g0.a2.gate1.o0",
            "k0.g0.a2.review_pass0.o0",
            "k0.g0.a2.review_reask0.o0",
            "k0.g0.a2.review_pass1.o0",
        ],
        "recorded: {seen:#?}"
    );

    use std::collections::BTreeSet;
    let attempts: BTreeSet<&str> = ids
        .iter()
        .map(|id| id.split('.').nth(2).expect("the attempt field"))
        .collect();
    assert_eq!(
        attempts,
        BTreeSet::from(["a1", "a2"]),
        "two attempt numbers"
    );
    let roles: BTreeSet<&str> = ids
        .iter()
        .map(|id| id.split('.').nth(3).expect("the role field"))
        .collect();
    assert_eq!(
        roles,
        BTreeSet::from([
            "worker",
            "gate0",
            "gate1",
            "review_pass0",
            "review_reask0",
            "review_pass1",
        ]),
        "six distinct role members across the run"
    );
    assert_eq!(
        ids.iter().collect::<BTreeSet<_>>().len(),
        ids.len(),
        "two processes of one run share an identity"
    );

    assert_eq!(
        source.adapter.reviews_run(),
        2,
        "the primary reviewer's verdict and its one re-ask"
    );
    assert_eq!(
        source.copilot().reviews_run(),
        1,
        "the second family answered once, and was not re-asked"
    );
}

#[test]
fn a_worker_that_cannot_be_spawned_returns_an_error_and_settles_nothing() {
    let (_tree, repo) = temp_engine_repo("workerspawn");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 2 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = source(vec![Effect::SpawnError], vec![ReviewBehavior::Pass]);
    let error = run_with(&opts, &source).expect_err(
        "a worker that cannot be spawned is an infrastructure error, not a run that finished",
    );
    let message = error.to_string();
    assert!(
        message.contains("failed to spawn"),
        "the runner's own diagnostic reaches the caller: {message}"
    );
    assert!(
        message.contains("missing-worker-executable"),
        "and it names the program: {message}"
    );

    let run_id = rundir::latest_run(&repo).expect("the run created its directory");
    let log = fs::read_to_string(paths_of(&repo, &run_id).events()).expect("events.jsonl");
    let kinds: Vec<String> = log
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str::<serde_json::Value>(line)
                .expect("an event")
                .get("event")
                .and_then(|kind| kind.as_str())
                .unwrap_or_default()
                .to_owned()
        })
        .collect();
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| *kind == "attempt_started")
            .count(),
        1,
        "exactly one attempt was dispatched: {kinds:?}"
    );
    for settled in [
        "attempt_finished",
        "task_failed",
        "task_completed",
        "run_finished",
    ] {
        assert_eq!(
            kinds.iter().filter(|kind| *kind == settled).count(),
            0,
            "a spawn failure synthesized `{settled}`: {kinds:?}"
        );
    }
    assert_eq!(source.adapter.runs().len(), 1, "the ladder bought no retry");
}

#[test]
fn the_engine_facade_exposes_exactly_the_items_the_packet_enumerates() {
    use std::collections::BTreeSet;

    let raw = include_str!("mod.rs");
    let blanked = crate::effects::production_code(raw);
    let source: &str = &blanked;
    assert!(
        source.len() * 2 > raw.len(),
        "the blanked region of the engine facade is {} of {} bytes, so a census over it says \
         little about the file",
        source.len(),
        raw.len()
    );

    assert_eq!(
        top_level_public_fns(source),
        BTreeSet::new(),
        "the engine facade declares a public function of its own again; its six entry points \
         are defined in the conductor modules they drive and re-exported here"
    );
    let entry_points: BTreeSet<String> = public_facade_entry_points().into_iter().collect();
    assert_eq!(
        entry_points,
        [
            "run",
            "run_with",
            "run_harness",
            "resume",
            "resume_with",
            "resume_harness",
        ]
        .map(str::to_owned)
        .into_iter()
        .collect::<BTreeSet<String>>(),
        "the engine facade's public functions moved away from the packet's list"
    );

    for widening in [
        "pub(crate) fn",
        "pub(crate) use",
        "pub struct",
        "pub enum",
        "pub const",
        "pub mod ",
    ] {
        assert!(
            !source.contains(widening),
            "`{widening}` appeared in the engine facade, which the visibility rule forbids"
        );
    }

    let reexported: BTreeSet<&str> = facade_reexports(source)
        .into_iter()
        .filter(|(from, _)| !CONDUCTOR_MODULES.contains(from))
        .flat_map(|(_, names)| names)
        .collect();
    assert_eq!(
        reexported,
        BTreeSet::from([
            "RunOptions",
            "ResumeOptions",
            "Harness",
            "DEFAULT_ATTEMPT_TIMEOUT",
            "DEFAULT_MAX_DEFERS",
            "RunReport",
            "TaskReport",
            "TaskRunStatus",
            "RunOutcome",
            "PoolDrainRow",
            "topo_order",
            "AdapterSource",
            "BuiltinAdapters",
            "AttemptRecord",
            "FailureRecord",
            "AttemptFailure",
            "FailureKind",
            "FailureOrigin",
        ]),
        "the engine facade's re-exports moved away from the packet's list"
    );
    assert_eq!(reexported.len(), 18, "five groups, eighteen names");
    assert_eq!(
        facade_reexports(source).len(),
        7,
        "the packet's five groups and one per conductor module"
    );

    let conductors = [
        (
            "coordinator",
            include_str!("coordinator.rs"),
            "run_harness_on",
        ),
        ("resume", include_str!("resume.rs"), "resume_harness_on"),
    ];
    assert_eq!(
        conductors.map(|(module, _, _)| module),
        CONDUCTOR_MODULES,
        "a conductor module is re-exported here and not read below"
    );
    for (module, text, seam) in conductors {
        let production = crate::effects::production_code(text);
        let declared = top_level_public_fns(&production);
        let reexported_from_it: BTreeSet<&str> = facade_reexports(source)
            .into_iter()
            .filter(|(from, _)| *from == module)
            .flat_map(|(_, names)| names)
            .collect();
        assert_eq!(
            declared, reexported_from_it,
            "`engine::{module}` declares a `pub fn` the facade does not re-export, or the facade \
             re-exports a name that is not one"
        );
        assert!(
            production.contains(&format!("pub(super) fn {seam}(")),
            "`pub(super) fn {seam}(` is gone from `engine::{module}`"
        );
        for public in [format!("pub fn {seam}"), format!("pub(crate) fn {seam}")] {
            assert!(
                !production.contains(&public),
                "an explicit-Runner entry point is public again: `{public}` in `engine::{module}`"
            );
        }
    }
}

const CONDUCTOR_MODULES: [&str; 2] = ["coordinator", "resume"];

fn top_level_public_fns(production: &str) -> std::collections::BTreeSet<&str> {
    production
        .lines()
        .filter_map(|line| line.strip_prefix("pub fn "))
        .filter_map(|rest| rest.split(['(', '<', ' ']).next())
        .collect()
}

fn facade_reexports(production: &str) -> Vec<(&str, Vec<&str>)> {
    let mut groups = Vec::new();
    let mut rest = production;
    while let Some(start) = rest.find("pub use ") {
        rest = &rest[start + "pub use ".len()..];
        let end = rest.find(';').expect("a `pub use` ends in a semicolon");
        let statement = &rest[..end];
        rest = &rest[end..];
        match (statement.find('{'), statement.find('}')) {
            (Some(open), Some(close)) => {
                let from = statement[..open].trim().trim_end_matches("::");
                let names = statement[open + 1..close]
                    .split(',')
                    .map(str::trim)
                    .filter(|name| !name.is_empty())
                    .collect();
                groups.push((from, names));
            }
            _ => {
                let (from, name) = statement.trim().rsplit_once("::").expect("a path");
                groups.push((from, vec![name.trim()]));
            }
        }
    }
    groups
}

fn public_facade_entry_points() -> Vec<String> {
    let production = crate::effects::production_code(include_str!("mod.rs"));
    let mut names: Vec<String> = facade_reexports(&production)
        .into_iter()
        .filter(|(from, _)| CONDUCTOR_MODULES.contains(from))
        .flat_map(|(_, names)| names)
        .map(str::to_owned)
        .collect();
    names.sort_unstable();
    names.dedup();
    names
}

#[test]
fn every_public_write_coordinator_entry_point_establishes_containment() {
    let (_tree, repo) = temp_engine_repo("containment-facade");
    let mut run_opts = options(&repo);
    run_opts.plan_path = repo.join("absent-plan.md");
    let mut resume_opts = ResumeOptions::new("01ABSENTRUN".to_owned(), repo.clone());
    resume_opts.pools_path = Some(no_pools());
    resume_opts.private_root = Some(private_root_for(&repo));
    let adapters = BuiltinAdapters;

    type Call<'a> = Box<dyn Fn() -> Result<RunReport, UpstrokeError> + 'a>;
    let entry_points: Vec<(&str, Call<'_>)> = vec![
        ("run", Box::new(|| run(&run_opts))),
        ("run_with", Box::new(|| run_with(&run_opts, &adapters))),
        (
            "run_harness",
            Box::new(|| run_harness(&run_opts, &Harness::new(&adapters))),
        ),
        ("resume", Box::new(|| resume(&resume_opts))),
        (
            "resume_with",
            Box::new(|| resume_with(&resume_opts, &adapters)),
        ),
        (
            "resume_harness",
            Box::new(|| resume_harness(&resume_opts, &Harness::new(&adapters))),
        ),
    ];

    let mut driven: Vec<&str> = entry_points.iter().map(|(name, _)| *name).collect();
    driven.sort_unstable();
    assert_eq!(
        driven,
        public_facade_entry_points(),
        "a public engine entry point is not driven here; every one of them makes its caller a \
         write coordinator"
    );
    assert_eq!(driven.len(), 6, "six entry points, and this is the count");

    for (name, call) in &entry_points {
        let before = crate::runner::host::containment_establishments();
        let outcome = call();
        assert!(
            outcome.is_err(),
            "{name}: the fixture relies on this refusing on its own input"
        );
        assert_eq!(
            crate::runner::host::containment_establishments(),
            before + 1,
            "`engine::{name}` entered the write coordinator without establishing containment \
             (INV-18); a kill after CreateProcessW and before private-job assignment would leave \
             a suspended stub alive"
        );

        #[cfg(windows)]
        assert!(
            crate::agent::proc::ambient_job_established(),
            "`engine::{name}` returned without this process joining its ambient Job Object"
        );
    }
}

#[test]
fn no_read_only_public_entry_point_establishes_containment() {
    let (_tree, repo) = temp_engine_repo("containment-readonly");
    let scratch = private_root_for(&repo);
    fs::create_dir_all(&scratch).expect("scratch");
    let absent = repo.join("absent-plan.md");

    type Call<'a> = Box<dyn Fn() + 'a>;
    let read_only: Vec<(&str, Call<'_>)> = vec![
        (
            "validate::run",
            Box::new(|| {
                let _ = crate::validate::run(&crate::validate::ValidateOptions {
                    plan_path: absent.clone(),
                    config_path: None,
                    config_root: repo.clone(),
                    pools_path: Some(no_pools()),
                    engine_limits: config::EngineLimits::Fresh,
                });
            }),
        ),
        (
            "status::load",
            Box::new(|| {
                let _ = crate::status::load(&repo, None);
            }),
        ),
        (
            "export::load",
            Box::new(|| {
                let _ = crate::export::load(&repo, "01ABSENTRUN");
            }),
        ),
        (
            "answer::answer",
            Box::new(|| {
                let _ = crate::answer::answer(&repo, "q1", crate::answer::Reply::Decline);
            }),
        ),
        (
            "capacity::report",
            Box::new(|| {
                let _ = capacity::report(
                    &capacity::CapacityOptions {
                        config_path: Some(absent.clone()),
                        pools_path: Some(no_pools()),
                        repo_root: repo.clone(),
                    },
                    &BuiltinAdapters,
                );
            }),
        ),
        (
            "connect::run_with",
            Box::new(|| {
                let _ = crate::connect::run_with(
                    &crate::connect::ConnectOptions {
                        pools_path: Some(scratch.join("pools.toml")),
                        force: true,
                    },
                    &BuiltinAdapters,
                    std::iter::empty(),
                );
            }),
        ),
    ];
    assert_eq!(
        read_only.len(),
        6,
        "one library entry point per read-only subcommand — the same six \
         `src/main.rs` counts on the dispatch side"
    );

    for (name, call) in &read_only {
        let before = crate::runner::host::containment_establishments();
        call();
        assert_eq!(
            crate::runner::host::containment_establishments(),
            before,
            "`{name}` is not a write coordinator and established containment anyway"
        );
    }
}

#[test]
fn a_facade_run_refuses_before_any_effect_when_containment_fails() {
    let (_tree, repo) = temp_engine_repo("containment-order");
    let mut opts = options(&repo);
    opts.plan_path = repo.join("absent-plan.md");
    let source = source(vec![Effect::EditFile], vec![ReviewBehavior::Pass]);
    let harness = Harness::new(&source);
    let runner = RecordingRunner::new();

    let refused = run_contained(&opts, &harness, &runner, || {
        Err(UpstrokeError::Refused {
            message: "the ambient Job Object could not be established (simulated failure)"
                .to_owned(),
        })
    })
    .expect_err("a run whose ambient job cannot be established must refuse");
    let refused = refused.to_string();
    assert!(
        refused.contains("ambient Job Object"),
        "the refusal must diagnose the ambient job: {refused}"
    );
    assert!(
        !refused.contains("absent-plan"),
        "the coordinator ran before containment: {refused}"
    );
    assert!(
        runner.seen().is_empty(),
        "a run refused at startup spawned a process: {:?}",
        runner.seen()
    );

    let reached = run_contained(&opts, &harness, &runner, || {
        crate::runner::host::contain_write_command(&mut crate::agent::proc::NoHooks)
    })
    .expect_err("the coordinator then fails on its own, on the plan");
    let reached = reached.to_string();
    assert!(
        reached.contains("absent-plan"),
        "with containment established the coordinator must run: {reached}"
    );
    assert!(
        !reached.contains("ambient Job Object"),
        "a successful establishment must not be reported as a refusal: {reached}"
    );
}

#[test]
fn a_facade_resume_refuses_before_any_effect_when_containment_fails() {
    let (_tree, repo) = temp_engine_repo("containment-order-resume");
    let mut opts = ResumeOptions::new("01ABSENTRUN".to_owned(), repo.clone());
    opts.pools_path = Some(no_pools());
    opts.private_root = Some(private_root_for(&repo));
    let source = source(vec![Effect::EditFile], vec![ReviewBehavior::Pass]);
    let harness = Harness::new(&source);
    let runner = RecordingRunner::new();

    let refused = resume_contained(&opts, &harness, &runner, || {
        Err(UpstrokeError::Refused {
            message: "the ambient Job Object could not be established (simulated failure)"
                .to_owned(),
        })
    })
    .expect_err("a resume whose ambient job cannot be established must refuse");

    let looked_in = repo.display().to_string();
    let refused = refused.to_string();
    assert!(
        refused.contains("ambient Job Object"),
        "the refusal must diagnose the ambient job: {refused}"
    );
    assert!(
        !refused.contains(&looked_in),
        "the coordinator ran before containment: {refused}"
    );
    assert!(
        runner.seen().is_empty(),
        "a resume refused at startup spawned a process: {:?}",
        runner.seen()
    );

    let reached = resume_contained(&opts, &harness, &runner, || {
        crate::runner::host::contain_write_command(&mut crate::agent::proc::NoHooks)
    })
    .expect_err("the coordinator then fails on its own, on the run it cannot find");
    let reached = reached.to_string();
    assert!(
        reached.contains(&looked_in),
        "with containment established the coordinator must run: {reached}"
    );
    assert!(
        !reached.contains("ambient Job Object"),
        "a successful establishment must not be reported as a refusal: {reached}"
    );
}

const PARKING_SETTLEMENT_KILL_CHILD: &str = "engine::tests::parking_settlement_kill_child";

const QUESTION_PAYLOAD_KILL_CHILD: &str = "engine::tests::question_payload_kill_child";

const KILL_CHILD_BOUND: Duration = Duration::from_secs(120);

const ASKING_PLAN: &str =
    "## Ask before building\n<!-- upstroke: id=t1 kind=implement depends= -->\n";

const PARKING_CONFIG: &str = "[interaction]\nmode = \"never\"\n\n\
     [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n";

struct KillOnceTheParkingSettlementIsDurable {
    repo: PathBuf,
}

impl crate::events::log::EventHooks for KillOnceTheParkingSettlementIsDurable {
    fn phase(&mut self, site: EventSite, phase: crate::topology::effects::HookPhase) {
        if site != EventSite::LegacyAppend || phase != crate::topology::effects::HookPhase::After {
            return;
        }
        let Some(run_id) = rundir::latest_run(&self.repo) else {
            return;
        };
        let log = fs::read_to_string(paths_of(&self.repo, &run_id).events()).unwrap_or_default();
        if log.lines().last().is_some_and(|line| {
            line.contains("\"attempt_finished\"") && line.contains("\"parking\":{")
        }) {
            std::process::abort();
        }
    }
}

fn kill_once_the_parking_settlement_is_durable() -> Box<dyn crate::events::log::EventHooks> {
    Box::new(KillOnceTheParkingSettlementIsDurable {
        repo: PathBuf::from(
            std::env::var("UPSTROKE_CRASH_REPO").expect("the parent names the repository"),
        ),
    })
}

#[test]
#[ignore = "spawned by the question payload witnesses"]
fn parking_settlement_kill_child() {
    let Ok(repo) = std::env::var("UPSTROKE_CRASH_REPO") else {
        return;
    };
    let repo = PathBuf::from(repo);
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    opts.log_hooks = Some(kill_once_the_parking_settlement_is_durable);
    let source = source(vec![Effect::AskQuestion], vec![ReviewBehavior::Pass]);
    let outcome = run_with(&opts, &source).map(|report| report.outcome());
    panic!("the run went on past its durable parking settlement: {outcome:?}");
}

struct QuestionPayloadKilledAt {
    inner: rundir::HarnessHooks,
    at: crate::topology::effects::HookPhase,
}

impl rundir::RunDirHooks for QuestionPayloadKilledAt {
    fn hook(
        &mut self,
        site: crate::topology::effects::EffectSiteId,
        phase: crate::topology::effects::HookPhase,
    ) -> crate::topology::effects::Injection {
        let answered = self.inner.hook(site, phase);
        if site
            == crate::topology::effects::EffectSiteId::RunDir(
                crate::topology::effects::RunDirSite::WriteQuestionPayload,
            )
            && phase == self.at
        {
            crate::observations::Exported::new(std::sync::Arc::clone(self.inner.harness()))
                .carried(crate::topology::effects::Injection::Kill)
        } else {
            answered
        }
    }

    fn durability_ledger(&self) -> crate::util::DurabilityLedger {
        self.inner.durability_ledger()
    }
}

fn payload_phase_named(name: &str) -> crate::topology::effects::HookPhase {
    match name {
        "Before" => crate::topology::effects::HookPhase::Before,
        "After" => crate::topology::effects::HookPhase::After,
        other => panic!("`{other}` is not a phase of `RunDir.WriteQuestionPayload`"),
    }
}

#[test]
#[ignore = "spawned by the question payload witnesses"]
fn question_payload_kill_child() {
    let Ok(repo) = std::env::var("UPSTROKE_CRASH_REPO") else {
        return;
    };
    let repo = PathBuf::from(repo);
    let at = payload_phase_named(
        &std::env::var("UPSTROKE_TEST_KILL_COORDINATE").expect("the parent names the phase"),
    );
    let run_id = rundir::latest_run(&repo).expect("the parent's run");
    let replayed = replay_of(&repo, &run_id);
    let [record] = replayed.state.questions.as_slice() else {
        panic!(
            "the parking settlement records one question: {:?}",
            replayed.state.questions
        );
    };
    let mut hooks = QuestionPayloadKilledAt {
        inner: rundir::HarnessHooks::new(std::sync::Arc::new(Mutex::new(
            crate::topology::effects::HookHarness::new(),
        ))),
        at,
    };
    let written = rundir::write_question_payload(
        &paths_of(&repo, &run_id).questions(),
        &crate::util::filename_component(record.question.id.as_str()),
        record,
        &mut hooks,
    );
    panic!(
        "the kill at `RunDir.WriteQuestionPayload` ({at}) did not take this process: {written:?}"
    );
}

fn a_run_killed_once_its_parking_settlement_is_durable(
    tag: &str,
) -> (
    rundir::scratch_tree::ScratchTree,
    PathBuf,
    String,
    QuestionRecord,
) {
    // The repository and its sibling private root (`private_root_for`) are made
    // in one tree, handed back for the caller to hold: the guard reclaims both
    // when the witness returns and when it unwinds (#292's review round 6).
    let tree = rundir::scratch_tree::acquire(&std::env::temp_dir(), tag)
        .expect("a scratch tree for the repository and its private root");
    git_in(tree.path(), &["init", "-q", "-b", "main", "repo"]);
    let repo = tree.path().join("repo");
    git_in(&repo, &["config", "user.email", "test@upstroke.local"]);
    git_in(&repo, &["config", "user.name", "upstroke tests"]);
    seed(&repo, ASKING_PLAN, Some(PARKING_CONFIG));
    let temporary = tree.path().as_os_str();
    let Some(killed) = crate::workspace_manager::fixture::run_kill_child_within(
        PARKING_SETTLEMENT_KILL_CHILD,
        &[
            ("UPSTROKE_CRASH_REPO", repo.as_os_str()),
            ("TMPDIR", temporary),
            ("TMP", temporary),
            ("TEMP", temporary),
        ],
        KILL_CHILD_BOUND,
    ) else {
        panic!(
            "{tag}: the parking run did not end within {KILL_CHILD_BOUND:?}, and was killed and \
             reaped"
        );
    };
    assert!(
        crate::workspace_manager::fixture::died_by_abort(&killed),
        "{tag}: the run must die once its parking settlement is durable: {killed:?}"
    );
    let run_id = rundir::latest_run(&repo).expect("the child started a run");
    let paths = paths_of(&repo, &run_id);
    let log = fs::read_to_string(paths.events()).expect("the log");
    let last = log.lines().last().expect("events");
    assert!(
        last.contains("\"attempt_finished\"") && last.contains("\"parking\":{"),
        "{tag}: the log ends at the parking settlement: {last}"
    );
    let replayed = replay_of(&repo, &run_id);
    let [record] = replayed.state.questions.as_slice() else {
        panic!(
            "{tag}: the settlement parks one question: {:?}",
            replayed.state.questions
        );
    };
    assert!(record.is_open(), "{tag}: nothing answered it");
    assert_eq!(
        fs::read_dir(paths.questions()).map_or(0, Iterator::count),
        0,
        "{tag}: the process died before the question's payload was written"
    );
    assert!(
        !rundir::is_running(&paths.public),
        "{tag}: the OS released the run lock"
    );
    (tree, repo, run_id, record.clone())
}

fn question_payload(repo: &Path, run_id: &str, record: &QuestionRecord) -> PathBuf {
    paths_of(repo, run_id).questions().join(format!(
        "{}.json",
        crate::util::filename_component(record.question.id.as_str())
    ))
}

fn resume_options_in(
    tree: &rundir::scratch_tree::ScratchTree,
    repo: &Path,
    run_id: &str,
) -> ResumeOptions {
    let pools = tree.path().join("pools.toml");
    fs::write(&pools, "# no pools\n").expect("the witness's own empty pools file");
    let mut opts = ResumeOptions::new(run_id.to_owned(), repo.to_path_buf());
    opts.pools_path = Some(pools);
    opts.attempt_timeout = Duration::from_secs(60);
    opts.defer_backoff = Duration::ZERO;
    opts.wait_on_block = Some(Duration::ZERO);
    opts.private_root = Some(private_root_for(repo));
    opts
}

fn resume_parked(
    tree: &rundir::scratch_tree::ScratchTree,
    repo: &Path,
    run_id: &str,
    record: &QuestionRecord,
    tag: &str,
) {
    let source = source(vec![Effect::AskQuestion], vec![ReviewBehavior::Pass]);
    let (resumed, state) = resume_harness_inner(
        &resume_options_in(tree, repo, run_id),
        &Harness {
            adapters: &source,
            answers: None,
            sleeper: None,
        },
    )
    .expect("the resume converges");
    assert_eq!(
        resumed.outcome(),
        RunOutcome::Parked,
        "{tag}: the question is still open, so the run parks again: {resumed:?}"
    );
    let payload = question_payload(repo, run_id, record);
    let written: QuestionRecord = serde_json::from_slice(
        &fs::read(&payload).expect("the resume leaves the question's payload"),
    )
    .expect("the payload is a question record");
    assert_eq!(
        &written, record,
        "{tag}: the payload holds the question the parking settlement records"
    );
    assert_eq!(
        resumed.questions,
        vec![record.clone()],
        "{tag}: the report names the one open question"
    );
    assert_live_equals_replay(repo, &state, &resumed);
    assert_live_equals_replay(repo, &state, &resumed);
}

fn a_kill_at_the_question_payload_write_is_recovered_by_the_resume(
    phase: crate::topology::effects::HookPhase,
    tag: &str,
) {
    use crate::topology::effects::{
        EffectSiteId, EntryPhase, HookPhase, ResourceRow, ResumeAction, RunDirSite,
    };

    let site = EffectSiteId::RunDir(RunDirSite::WriteQuestionPayload);
    let semantics = site.semantics(match phase {
        HookPhase::Before => EntryPhase::Before,
        HookPhase::After => EntryPhase::After,
        HookPhase::Point { .. } => panic!("the payload's coordinates are its two phases"),
    });
    // Bound first, so it is reclaimed last: `tree` owns the repository and its
    // private root until this witness returns or unwinds.
    let (tree, repo, run_id, record) = a_run_killed_once_its_parking_settlement_is_durable(tag);
    let payload = question_payload(&repo, &run_id, &record);
    let coordinate = format!("{phase:?}");
    let temporary = tree.path().as_os_str();
    let Some(killed) = crate::workspace_manager::fixture::run_kill_child_within(
        QUESTION_PAYLOAD_KILL_CHILD,
        &[
            ("UPSTROKE_CRASH_REPO", repo.as_os_str()),
            (
                "UPSTROKE_TEST_KILL_COORDINATE",
                std::ffi::OsStr::new(&coordinate),
            ),
            ("TMPDIR", temporary),
            ("TMP", temporary),
            ("TEMP", temporary),
        ],
        KILL_CHILD_BOUND,
    ) else {
        panic!(
            "{tag}: the payload writer armed at the {phase} phase did not end within \
             {KILL_CHILD_BOUND:?}, and was killed and reaped"
        );
    };
    assert!(
        crate::workspace_manager::fixture::died_by_abort(&killed),
        "{tag}: the payload writer must die at the {phase} phase: {killed:?}"
    );
    assert_eq!(
        payload.is_file(),
        semantics.rows.contains(&ResourceRow::R21),
        "{tag}: the payload is left exactly where the authority's rows say R21 holds it ({:?})",
        semantics.rows
    );
    assert_eq!(
        semantics.action,
        match phase {
            HookPhase::Before => ResumeAction::ResumeUnperformed,
            _ => ResumeAction::AdoptPerformed,
        },
        "{tag}: the action the authority tables for `{site}` ({phase})"
    );
    let left = fs::read(&payload).ok();
    let log = fs::read(paths_of(&repo, &run_id).events()).expect("the log");

    resume_parked(&tree, &repo, &run_id, &record, tag);
    if let Some(left) = left {
        assert_eq!(
            fs::read(&payload).expect("the payload"),
            left,
            "{tag}: the resume's rewrite adopted the payload the killed write left, byte for byte"
        );
    }
    assert!(
        fs::read(paths_of(&repo, &run_id).events())
            .expect("the log")
            .starts_with(&log),
        "{tag}: the resume appended after the durable prefix"
    );
}

#[test]
fn a_kill_before_a_parked_questions_payload_is_written_is_recovered_by_the_resume_writing_it() {
    a_kill_at_the_question_payload_write_is_recovered_by_the_resume(
        crate::topology::effects::HookPhase::Before,
        "payload-kill-before",
    );
}

#[test]
fn a_kill_after_a_parked_questions_payload_is_written_is_recovered_by_the_resume_adopting_it() {
    a_kill_at_the_question_payload_write_is_recovered_by_the_resume(
        crate::topology::effects::HookPhase::After,
        "payload-kill-after",
    );
}

#[cfg(windows)]
struct CreationsRecorded {
    inner: crate::runner::HarnessHooks,
    created: Vec<u32>,
}

#[cfg(windows)]
impl crate::agent::proc::SpawnHooks for CreationsRecorded {
    fn point(
        &mut self,
        point: crate::topology::effects::SubEffectPoint,
    ) -> crate::topology::effects::Injection {
        self.inner.point(point)
    }

    fn point_mode(
        &mut self,
        point: crate::topology::effects::SubEffectPoint,
        mode: crate::topology::effects::InjectionMode,
    ) -> crate::topology::effects::Injection {
        self.inner.point_mode(point, mode)
    }

    fn phase(
        &mut self,
        site: crate::topology::effects::ProcessSite,
        phase: crate::topology::effects::HookPhase,
    ) -> crate::topology::effects::Injection {
        self.inner.phase(site, phase)
    }

    fn child_created(&mut self, pid: u32) {
        self.created.push(pid);
        self.inner.child_created(pid);
    }
}

#[cfg(windows)]
#[test]
fn a_resume_whose_ambient_job_join_errs_runs_nothing_and_the_next_resume_converges() {
    use crate::topology::effects::{HookHarness, HookPhase, InjectionMode, SubEffectPoint};

    let tag = "ambient-join-error-resume";
    // `tree` owns the repository and its private root until this witness
    // returns or unwinds.
    let (tree, repo, run_id, record) = a_run_killed_once_its_parking_settlement_is_durable(tag);
    let paths = paths_of(&repo, &run_id);
    let payload = question_payload(&repo, &run_id, &record);
    let log = fs::read(paths.events()).expect("the log");
    let site = crate::runner::SPAWN_SITE;
    let point = SubEffectPoint::AmbientJobJoined;
    let mode = InjectionMode::ErrorReturn;
    let harness = std::sync::Arc::new(Mutex::new(HookHarness::new()));
    harness
        .lock()
        .expect("the harness")
        .arm(site, point, mode)
        .expect("the ambient join supports an error return");
    let mut hooks = CreationsRecorded {
        inner: crate::runner::HarnessHooks::new(std::sync::Arc::clone(&harness)),
        created: Vec::new(),
    };
    let source = source(vec![Effect::AskQuestion], vec![ReviewBehavior::Pass]);
    let runner = RecordingRunner::new();

    let refused = resume_contained(
        &resume_options_in(&tree, &repo, &run_id),
        &Harness::new(&source),
        &runner,
        || crate::runner::host::contain_write_command(&mut hooks),
    )
    .expect_err("a resume whose ambient job join errs refuses");
    let refused = refused.to_string();
    assert!(
        refused.contains("INV-18")
            && refused.contains("(simulated failure). No process was spawned"),
        "{tag}: the refusal is the containment step's, for the injected failure: {refused}"
    );
    assert!(
        harness
            .lock()
            .expect("the harness")
            .observed(site, HookPhase::Point { point, mode }),
        "{tag}: the armed point fired"
    );
    assert!(
        hooks.created.is_empty(),
        "{tag}: the funnel created a process: {:?}",
        hooks.created
    );
    assert!(
        runner.seen().is_empty(),
        "{tag}: the resume went on and ran a process: {:?}",
        runner.seen()
    );
    assert_eq!(
        fs::read(paths.events()).expect("the log"),
        log,
        "{tag}: the resume went on and appended: {refused}"
    );
    assert!(
        !payload.exists(),
        "{tag}: the resume went on and rewrote the question's payload: {refused}"
    );
    assert!(
        !rundir::is_running(&paths.public),
        "{tag}: nothing holds the run lock"
    );
    drop(hooks);

    resume_parked(&tree, &repo, &run_id, &record, tag);
}

const ROLE_PROBE: &str = "UPSTROKE_V1_ROLE_PROBE";

/// The repository a role creates inside its own working directory. Its name is
/// short because Windows' `MAX_PATH` is 260 characters and a gate's working
/// directory is the product's snapshot, nested in the private root at
/// `runs/<ULID>/gate-worktrees/worktrees/upstroke-gates-<pid>-<ULID>`: the
/// longest path Git writes in it, `.git/refs/replace/<40 hex>.lock`, is checked
/// against [`WINDOWS_MAX_PATH`] as the `path-budget` record, so a name that grows
/// fails there, with the length, before Git's "Filename too long".
const INSIDE_FIXTURE: &str = "fx";

/// Windows' `MAX_PATH`, 260 characters with the terminating null.
const WINDOWS_MAX_PATH: usize = 260;

#[derive(Clone)]
struct RoleProbe {
    worker: CommandSpec,
    reviewer: CommandSpec,
    operator_reads: Option<(PathBuf, String)>,
}

impl RoleProbe {
    fn new(overlay: &[(&str, &str)]) -> Self {
        let exe = std::env::current_exe()
            .expect("this test binary")
            .to_string_lossy()
            .into_owned();
        let helper = |test: &str| {
            let mut spec = CommandSpec::new(exe.clone())
                .arg("--exact")
                .arg(test)
                .arg("--ignored")
                .arg("--nocapture");
            for (key, value) in overlay {
                spec = spec.env(*key, *value);
            }
            spec
        };
        Self {
            worker: helper("engine::tests::v1_role_probe_worker"),
            reviewer: helper("engine::tests::v1_role_probe_reviewer"),
            operator_reads: None,
        }
    }

    fn reading_as_the_operator(mut self, record: &Path, blob: &str) -> Self {
        self.operator_reads = Some((record.to_path_buf(), blob.to_owned()));
        self
    }

    fn read_as_the_operator_does(&self, workspace: &Path) {
        if let Some((record, blob)) = &self.operator_reads {
            let read = git_in(workspace, &["cat-file", "-p", blob]);
            fs::write(
                record.join(format!("operator-{}", crate::ulid::ulid())),
                format!("operator blob {}\n", read.trim()),
            )
            .expect("the operator's reading");
        }
    }
}

fn role_probe_gates() -> String {
    format!(
        "[[gates]]\nname = \"role-probe\"\n\
         cmd = '\"{}\" --exact engine::tests::v1_role_probe_gate --ignored --nocapture'\n",
        std::env::current_exe().expect("this test binary").display()
    )
}

fn probe_git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(|error| error.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_owned())
    }
}

fn probe_fixture(parent: &Path, name: &str) -> Result<String, String> {
    let fixture = parent.join(name);
    let read = (|| {
        fs::create_dir_all(&fixture).map_err(|error| error.to_string())?;
        let git = |args: &[&str]| probe_git(&fixture, args).map(|out| out.trim().to_owned());
        git(&["init", "-q"])?;
        fs::write(fixture.join("f.txt"), "recorded\n").map_err(|error| error.to_string())?;
        git(&["add", "f.txt"])?;
        // `maintenance.autoDetach=false` works around
        // PR326-MACOS-A-DAEMONIZING-DESCENDANT-HANGS-UP-THE-ROLE, owner ruling
        // 2026-09-28: a commit whose auto-maintenance detaches forks, exits and
        // calls setsid, and on macOS the role that ran it then dies of SIGHUP.
        // The witnesses avoid that trigger; the defect is filed, not absent.
        git(&[
            "-c",
            "user.name=probe",
            "-c",
            "user.email=probe@upstroke.local",
            "-c",
            "maintenance.autoDetach=false",
            "commit",
            "-q",
            "-m",
            "recorded",
        ])?;
        let recorded = git(&["rev-parse", "HEAD:f.txt"])?;
        fs::write(fixture.join("replacing.txt"), "replacing\n")
            .map_err(|error| error.to_string())?;
        let replacing = git(&["hash-object", "-w", "replacing.txt"])?;
        git(&["replace", &recorded, &replacing])?;
        git(&["cat-file", "-p", &recorded])
    })();
    let _ = fs::remove_dir_all(&fixture);
    read
}

fn v1_role_probe(role: &str) {
    let Some(dir) = std::env::var_os(ROLE_PROBE) else {
        return;
    };
    let dir = PathBuf::from(dir);
    let spec = fs::read_to_string(dir.join("spec")).expect("the spec the witness wrote");
    let here = std::env::current_dir().expect("this role's working directory");
    let mut record: Vec<String> = Vec::new();
    let mut note = |check: &str, ok: bool, detail: String| {
        let verdict = if ok { "ok" } else { "FAIL" };
        record.push(format!("{role} {check} {verdict} {detail}"));
    };
    note(
        "variable",
        std::env::var_os("GIT_NO_REPLACE_OBJECTS").is_none(),
        "GIT_NO_REPLACE_OBJECTS is not in this role's environment".to_owned(),
    );
    if role == "gate" && spec.lines().any(|line| line == "worktree true") {
        let set = probe_git_past_another_registrations_write(
            &here,
            &["config", "--worktree", "core.useReplaceRefs", "true"],
        );
        note("worktree-true", set.is_ok(), format!("{set:?}"));
    }
    let trimmed = |read: Result<String, String>| read.map(|out| out.trim().to_owned());
    for line in spec.lines() {
        let (kind, rest) = line.split_once(' ').unwrap_or((line, ""));
        let (first, second) = rest.split_once(' ').unwrap_or((rest, ""));
        match kind {
            "include" => {
                let origin = probe_git(
                    &here,
                    &[
                        "config",
                        "-z",
                        "--show-origin",
                        "--get",
                        "core.useReplaceRefs",
                    ],
                );
                let expected = format!("file:{}\0false\0", rest.replace('\\', "/"));
                let read = origin.clone().map(|origin| origin.replace('\\', "/"));
                note("include", read == Ok(expected), format!("{origin:?}"));
            }
            "blob" => {
                let read = trimmed(probe_git(&here, &["cat-file", "-p", first]));
                note("blob", read.as_deref() == Ok(second), format!("{read:?}"));
            }
            "tree" => {
                let read = trimmed(probe_git(
                    &here,
                    &["rev-parse", &format!("{first}:inner.txt")],
                ));
                note("tree", read.as_deref() == Ok(second), format!("{read:?}"));
            }
            "commit" => {
                let read = trimmed(probe_git(&here, &["log", "-1", "--format=%s", first]));
                note("commit", read.as_deref() == Ok(second), format!("{read:?}"));
            }
            "candidate" if role == "worker" => {
                fs::write(here.join("candidate.txt"), format!("{first}\n"))
                    .expect("the worker's candidate");
                let replacing = dir.join(format!("replacing-{}", std::process::id()));
                fs::write(&replacing, format!("{second}\n")).expect("the replacing content");
                let recorded = trimmed(probe_git(&here, &["hash-object", "-w", "candidate.txt"]));
                let replacement = trimmed(probe_git(
                    &here,
                    &["hash-object", "-w", &replacing.to_string_lossy()],
                ));
                let installed = match (&recorded, &replacement) {
                    (Ok(recorded), Ok(replacement)) => {
                        probe_git(&here, &["replace", "-f", recorded, replacement])
                    }
                    _ => Err(format!("{recorded:?} {replacement:?}")),
                };
                note(
                    "candidate-replaced",
                    installed.is_ok(),
                    format!("{installed:?}"),
                );
                let read = match &recorded {
                    Ok(recorded) => trimmed(probe_git(&here, &["cat-file", "-p", recorded])),
                    Err(error) => Err(error.clone()),
                };
                note(
                    "candidate",
                    read.as_deref() == Ok(first),
                    format!("{read:?}"),
                );
            }
            "candidate" => {
                let read = trimmed(probe_git(&here, &["show", "HEAD:candidate.txt"]));
                note(
                    "candidate",
                    read.as_deref() == Ok(first),
                    format!("{read:?}"),
                );
                let recorded = trimmed(probe_git(&here, &["hash-object", "candidate.txt"]));
                let listed = trimmed(probe_git(&here, &["replace", "-l"]));
                note(
                    "candidate-replacement-installed",
                    matches!((&recorded, &listed), (Ok(id), Ok(list)) if list.lines().any(|line| line == id)),
                    format!("{recorded:?} in {listed:?}"),
                );
                let diff = probe_git(&here, &["diff", "--quiet", "--exit-code", "HEAD"]);
                note("diff", diff.is_ok(), format!("{diff:?}"));
                let status = trimmed(probe_git(&here, &["status", "--porcelain"]));
                note("status", status.as_deref() == Ok(""), format!("{status:?}"));
            }
            "worktree" => {}
            _ => note(
                "spec",
                false,
                format!("a spec line this probe cannot read: {line:?}"),
            ),
        }
    }
    let outside = format!("fixture-{role}-{}", std::process::id());
    for (place, parent, name) in [
        ("inside", here.clone(), INSIDE_FIXTURE),
        ("outside", dir.join("fixtures"), outside.as_str()),
    ] {
        if place == "inside" {
            let deepest = parent
                .join(name)
                .join(".git")
                .join("refs")
                .join("replace")
                .join(format!("{}.lock", "0".repeat(40)));
            let length = deepest.to_string_lossy().chars().count();
            note(
                "path-budget",
                !cfg!(windows) || length < WINDOWS_MAX_PATH,
                format!("{length} {}", deepest.display()),
            );
        }
        let read = probe_fixture(&parent, name);
        note(
            &format!("fixture-{place}"),
            read.as_deref() == Ok("replacing"),
            format!("{read:?}"),
        );
    }
    let failed = record.iter().any(|line| line.contains(" FAIL "));
    let log = dir.join("log");
    fs::create_dir_all(&log).expect("the record's directory");
    fs::write(
        log.join(format!(
            "{role}-{}-{}",
            std::process::id(),
            crate::ulid::ulid()
        )),
        record.join("\n") + "\n",
    )
    .expect("the record");
    if role == "gate" && failed {
        std::process::exit(1);
    }
}

#[test]
#[ignore = "a v0.1 worker the role witnesses start"]
fn v1_role_probe_worker() {
    v1_role_probe("worker");
}

#[test]
#[ignore = "a v0.1 gate the role witnesses start"]
fn v1_role_probe_gate() {
    v1_role_probe("gate");
}

#[test]
#[ignore = "a v0.1 reviewer the role witnesses start"]
fn v1_role_probe_reviewer() {
    use std::io::Write as _;

    if std::env::var_os(ROLE_PROBE).is_none() {
        return;
    }
    v1_role_probe("reviewer");
    std::io::stdout()
        .write_all(format!("{REVIEW_MARKER}\n").as_bytes())
        .expect("the review marker");
}

fn take_role_records(probe: &Path) -> Vec<String> {
    let log = probe.join("log");
    let mut lines = Vec::new();
    let mut entries: Vec<PathBuf> = fs::read_dir(&log)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .collect()
        })
        .unwrap_or_default();
    entries.sort();
    for entry in entries {
        let text = fs::read_to_string(&entry).expect("a role's record");
        lines.extend(text.lines().map(str::to_owned));
        fs::remove_file(&entry).expect("the record, taken");
    }
    lines
}

fn assert_every_role_read_the_recorded_graph(records: &[String], when: &str) {
    for role in ["worker", "gate", "reviewer"] {
        assert!(
            records
                .iter()
                .any(|line| line.starts_with(&format!("{role} "))),
            "{when}: no {role} ran a probe: {records:#?}"
        );
    }
    let failed: Vec<&String> = records
        .iter()
        .filter(|line| !line.contains(" ok "))
        .collect();
    assert!(failed.is_empty(), "{when}: {failed:#?}");
}

fn take_role_and_operator_records(probe: &Path) -> (Vec<String>, Vec<String>) {
    take_role_records(probe)
        .into_iter()
        .partition(|line| !line.starts_with("operator "))
}

fn challenge_every_scope(truthful: &Path, probe: &Path) -> Vec<(&'static str, std::ffi::OsString)> {
    vec![
        ("GIT_CONFIG_SYSTEM", truthful.as_os_str().to_owned()),
        ("GIT_CONFIG_NOSYSTEM", "0".into()),
        ("GIT_CONFIG_GLOBAL", truthful.as_os_str().to_owned()),
        ("GIT_CONFIG_COUNT", "1".into()),
        ("GIT_CONFIG_KEY_0", "core.useReplaceRefs".into()),
        ("GIT_CONFIG_VALUE_0", "true".into()),
        (
            "GIT_CONFIG_PARAMETERS",
            "'core.useReplaceRefs'='true'".into(),
        ),
        (ROLE_PROBE, probe.as_os_str().to_owned()),
    ]
}

const OVERLAY_SAYS_TRUE: [(&str, &str); 4] = [
    ("GIT_CONFIG_COUNT", "1"),
    ("GIT_CONFIG_KEY_0", "core.useReplaceRefs"),
    ("GIT_CONFIG_VALUE_0", "true"),
    ("GIT_CONFIG_PARAMETERS", "'core.useReplaceRefs'='true'"),
];

fn install_a_replaced_graph(repo: &Path, probe: &Path) -> (String, String) {
    fs::create_dir_all(repo.join("dir")).expect("the replaced tree's directory");
    fs::write(repo.join("blob.txt"), "recorded-blob\n").expect("the replaced blob");
    fs::write(repo.join("dir").join("inner.txt"), "recorded-tree\n").expect("the replaced tree");
    git_in(repo, &["add", "-A"]);
    git_in(repo, &["commit", "-q", "-m", "recorded-commit"]);
    let id = |spec: &str| git_in(repo, &["rev-parse", spec]).trim().to_owned();
    let (commit, blob, tree, inner) = (
        id("HEAD"),
        id("HEAD:blob.txt"),
        id("HEAD:dir"),
        id("HEAD:dir/inner.txt"),
    );
    let written = |name: &str, content: &str| {
        let file = probe.join(name);
        fs::write(&file, content).expect("a replacing object's content");
        git_in(repo, &["hash-object", "-w", &file.to_string_lossy()])
            .trim()
            .to_owned()
    };
    let replacing_blob = written("replacing-blob", "replacing-blob\n");
    let replacing_inner = written("replacing-inner", "replacing-tree\n");
    let index = probe.join("replacing-index");
    let with_index = |args: &[&str]| {
        let out = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .env("GIT_INDEX_FILE", &index)
            .output()
            .expect("run git");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_owned()
    };
    with_index(&[
        "update-index",
        "--add",
        "--cacheinfo",
        &format!("100644,{replacing_inner},inner.txt"),
    ]);
    let replacing_tree = with_index(&["write-tree"]);
    let replacing_commit = git_in(
        repo,
        &[
            "commit-tree",
            &id("HEAD^{tree}"),
            "-p",
            "HEAD^",
            "-m",
            "replacing-commit",
        ],
    )
    .trim()
    .to_owned();
    git_in(repo, &["replace", &blob, &replacing_blob]);
    git_in(repo, &["replace", &tree, &replacing_tree]);
    git_in(repo, &["replace", &commit, &replacing_commit]);
    crate::workspace_manager::fixture::pin_replacement_refs_in(repo);
    git_in(repo, &["config", "extensions.worktreeConfig", "true"]);
    git_in(
        repo,
        &["config", "--worktree", "core.useReplaceRefs", "true"],
    );
    (
        blob.clone(),
        format!(
            "blob {blob} recorded-blob\ntree {tree} {inner}\ncommit {commit} recorded-commit\n\
             candidate recorded-candidate replacing-candidate\nworktree true\n"
        ),
    )
}

fn assert_every_scope_says_true(repo: &Path) {
    let scopes = git_in(
        repo,
        &["config", "--show-scope", "--get-all", "core.useReplaceRefs"],
    );
    for scope in ["system", "global", "local", "worktree"] {
        assert!(
            scopes.lines().any(|line| line == format!("{scope}\ttrue")),
            "the challenge must reach Git at `{scope}` scope: {scopes:?}"
        );
    }
    assert_eq!(
        scopes
            .lines()
            .filter(|line| *line == "command\ttrue")
            .count(),
        2,
        "and at command scope twice, `GIT_CONFIG_COUNT` and `GIT_CONFIG_PARAMETERS`: {scopes:?}"
    );
    assert!(
        !scopes.contains("false"),
        "nothing the operator has says `false`: {scopes:?}"
    );
}

fn configuration_of(repo: &Path, also: &[&Path]) -> Vec<(PathBuf, Vec<u8>)> {
    let common = PathBuf::from(
        git_in(
            repo,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        )
        .trim(),
    );
    let mut files: Vec<PathBuf> = also.iter().map(|file| file.to_path_buf()).collect();
    let mut named: Vec<PathBuf> = fs::read_dir(&common)
        .expect("the common directory")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with("config"))
        })
        .collect();
    named.sort();
    files.extend(named);
    files
        .into_iter()
        .map(|file| {
            let bytes = fs::read(&file).expect("a configuration file");
            (file, bytes)
        })
        .collect()
}

#[test]
fn the_v1_conductor_keeps_every_role_on_the_recorded_graph_of_its_own_repository() {
    let tree = temp_engine_scratch("v1-role-graph");
    let truthful = tree.path().join("true.gitconfig");
    fs::write(&truthful, "[core]\n\tuseReplaceRefs = true\n").expect("a `true` for two scopes");
    let status = crate::workspace_manager::fixture::run_challenged_replacement_witness_child(
        "engine::tests::v1_role_graph_helper",
        &challenge_every_scope(&truthful, tree.path()),
    );
    assert!(
        status.success(),
        "the child runs and resumes a v0.1 run whose worker, gates and reviewers each read \
         a repository with blob, tree and commit replacements, under `core.useReplaceRefs = \
         true` at every scope, and ended {status:?}"
    );
}

#[test]
#[ignore = "subprocess helper"]
fn v1_role_graph_helper() {
    if std::env::var_os(crate::workspace_manager::fixture::REPLACEMENT_WITNESS).is_none() {
        return;
    }
    let probe = PathBuf::from(std::env::var_os(ROLE_PROBE).expect("the probe's directory"));
    let truthful = PathBuf::from(std::env::var_os("GIT_CONFIG_GLOBAL").expect("the challenge"));
    fs::create_dir_all(probe.join("fixtures")).expect("the fixtures' parent");
    fs::create_dir_all(probe.join("log")).expect("the records' directory");

    let (_tree, repo) = temp_engine_repo("v1g");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some(&format!(
            "[interaction]\nmode = \"never\"\n\n\
             [routing]\nimplement = {{ chain = [\"small\"], attempts_per = 1 }}\n\n{}",
            role_probe_gates()
        )),
    );
    let (blob, spec) = install_a_replaced_graph(&repo, &probe);
    let include = private_root_for(&repo)
        .join("git")
        .join("recorded-objects.gitconfig");
    fs::write(
        probe.join("spec"),
        format!("include {}\n{spec}", include.display()),
    )
    .expect("the probe's spec");
    assert_every_scope_says_true(&repo);
    assert_eq!(
        git_in(&repo, &["cat-file", "-p", &blob]).trim(),
        "replacing-blob",
        "the operator's own Git honours the replacements, or nothing below is measured"
    );
    let configuration = configuration_of(&repo, &[&truthful]);

    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let probing =
        RoleProbe::new(&OVERLAY_SAYS_TRUE).reading_as_the_operator(&probe.join("log"), &blob);
    let first = run_with(
        &opts,
        &source(vec![Effect::EditFile], vec![ReviewBehavior::Fail]).probing_with(probing.clone()),
    )
    .expect("the first run");
    assert_eq!(first.outcome(), RunOutcome::Parked, "{first:?}");
    let (records, operator) = take_role_and_operator_records(&probe);
    assert_every_role_read_the_recorded_graph(&records, "the first run");
    assert!(
        !operator.is_empty()
            && operator
                .iter()
                .all(|line| line == "operator blob replacing-blob"),
        "while the run's roles read the recorded graph, the operator's own Git kept reading \
         the replacements: {operator:?}"
    );
    assert_eq!(
        configuration_of(&repo, &[&truthful]),
        configuration,
        "the run changed no Git configuration"
    );
    assert_eq!(
        fs::read(&include).expect("the include"),
        crate::runner::host::RECORDED_OBJECTS_INCLUDE
    );

    fs::remove_file(&include).expect("the include, gone before the resume");
    let question = first
        .questions
        .first()
        .expect("a question was raised")
        .question
        .id
        .to_string();
    crate::answer::answer(
        &repo,
        &question[..8],
        crate::answer::Reply::Text("the widget lives in src/widget.rs".to_owned()),
    )
    .expect("answer");
    let resumed = resume_with(
        &resume_options(&repo, &first.run_id),
        &source(vec![Effect::EditFile], vec![ReviewBehavior::Pass]).probing_with(probing),
    )
    .expect("the resume");
    assert_eq!(resumed.outcome(), RunOutcome::Complete, "{resumed:?}");
    assert!(committed(&resumed, "t1"), "{resumed:?}");
    let (records, operator) = take_role_and_operator_records(&probe);
    assert_every_role_read_the_recorded_graph(&records, "the resume");
    assert!(
        !operator.is_empty()
            && operator
                .iter()
                .all(|line| line == "operator blob replacing-blob"),
        "{operator:?}"
    );
    assert_eq!(
        fs::read(&include).expect("the include the resume wrote again"),
        crate::runner::host::RECORDED_OBJECTS_INCLUDE
    );
    assert_eq!(
        configuration_of(&repo, &[&truthful]),
        configuration,
        "the resume changed no Git configuration"
    );
    assert_eq!(
        git_in(&repo, &["cat-file", "-p", &blob]).trim(),
        "replacing-blob",
        "and afterwards the operator's Git still honours the replacements"
    );
    assert_eq!(
        git_in(
            &repo,
            &[
                "--no-replace-objects",
                "-c",
                "core.useReplaceRefs=false",
                "show",
                "HEAD:candidate.txt"
            ]
        )
        .trim(),
        "recorded-candidate",
        "the task committed the bytes its worker wrote, not the replacement it installed"
    );
}

fn include_for(entry: &Path) -> PathBuf {
    private_root_for(entry)
        .join("git")
        .join("recorded-objects.gitconfig")
}

fn write_probe_spec(probe: &Path, entry: &Path, spec: &str) {
    for directory in ["fixtures", "log"] {
        fs::create_dir_all(probe.join(directory)).expect("the probe's directories");
    }
    fs::write(
        probe.join("spec"),
        format!("include {}\n{spec}", include_for(entry).display()),
    )
    .expect("the probe's spec");
}

#[test]
fn a_gate_that_replaces_objects_in_its_own_fixture_passes_under_the_v1_runner() {
    let tree = temp_engine_scratch("v1-own-fixture");
    let status = crate::workspace_manager::fixture::run_challenged_replacement_witness_child(
        "engine::tests::v1_own_fixture_helper",
        &[(ROLE_PROBE, tree.path().as_os_str().to_owned())],
    );
    assert!(
        status.success(),
        "the child runs a v0.1 task in a repository with nothing under `refs/replace/`, \
         whose worker, gate and reviewer each create a repository of their own, install a \
         replacement in it and read it back, and ended {status:?}"
    );
}

#[test]
#[ignore = "subprocess helper"]
fn v1_own_fixture_helper() {
    if std::env::var_os(crate::workspace_manager::fixture::REPLACEMENT_WITNESS).is_none() {
        return;
    }
    crate::workspace_manager::fixture::assert_replacement_controls_pinned("v1-own-fixture");
    let probe = PathBuf::from(std::env::var_os(ROLE_PROBE).expect("the probe's directory"));
    let (_tree, repo) = temp_engine_repo("v1o");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 depends= -->\n",
        Some(&format!(
            "[interaction]\nmode = \"never\"\n\n{}",
            role_probe_gates()
        )),
    );
    assert!(
        git_in(&repo, &["replace", "-l"]).trim().is_empty(),
        "the managed repository holds no replacement"
    );
    write_probe_spec(&probe, &repo, "");
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let report = run_with(
        &opts,
        &fake(Effect::EditFile).probing_with(RoleProbe::new(&[])),
    )
    .expect("the run");
    let (records, _) = take_role_and_operator_records(&probe);
    assert_eq!(
        report.outcome(),
        RunOutcome::Complete,
        "{report:?}\n{records:#?}"
    );
    assert_every_role_read_the_recorded_graph(&records, "the run");
}

#[test]
fn a_refused_an_interrupted_and_a_resumed_v1_run_leave_git_configuration_as_they_found_it() {
    let tree = temp_engine_scratch("v1-configuration");
    let truthful = tree.path().join("true.gitconfig");
    fs::write(&truthful, "[core]\n\tuseReplaceRefs = true\n").expect("a `true` for two scopes");
    let status = crate::workspace_manager::fixture::run_challenged_replacement_witness_child(
        "engine::tests::v1_configuration_helper",
        &challenge_every_scope(&truthful, tree.path()),
    );
    assert!(
        status.success(),
        "the child refuses a run over a dirty checkout, kills one inside its attempt and \
         resumes it, comparing every Git configuration file after each, and ended {status:?}"
    );
}

#[test]
#[ignore = "subprocess helper"]
fn v1_configuration_helper() {
    if std::env::var_os(crate::workspace_manager::fixture::REPLACEMENT_WITNESS).is_none() {
        return;
    }
    let probe = PathBuf::from(std::env::var_os(ROLE_PROBE).expect("the probe's directory"));
    let truthful = PathBuf::from(std::env::var_os("GIT_CONFIG_GLOBAL").expect("the challenge"));
    let (_tree, repo) = temp_engine_repo("v1c");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some(&format!(
            "[interaction]\nmode = \"never\"\n\n\
             [routing]\nimplement = {{ chain = [\"small\"], attempts_per = 1 }}\n\n{}",
            role_probe_gates()
        )),
    );
    let (blob, spec) = install_a_replaced_graph(&repo, &probe);
    write_probe_spec(&probe, &repo, &spec);
    assert_every_scope_says_true(&repo);
    let configuration = configuration_of(&repo, &[&truthful]);
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));

    fs::write(repo.join("stray.txt"), "stray\n").expect("a stray file");
    let refused = run_with(
        &opts,
        &fake(Effect::EditFile).probing_with(RoleProbe::new(&OVERLAY_SAYS_TRUE)),
    )
    .expect_err("a checkout with a stray file is refused")
    .to_string();
    assert!(refused.contains("not clean"), "{refused}");
    assert_eq!(
        configuration_of(&repo, &[&truthful]),
        configuration,
        "a refused run changed no Git configuration"
    );
    fs::remove_file(repo.join("stray.txt")).expect("the stray file, gone");

    let died = Command::new(std::env::current_exe().expect("this test binary"))
        .args([
            "--exact",
            "engine::tests::v1_interrupted_run_helper",
            "--ignored",
            "--nocapture",
        ])
        .env("UPSTROKE_V1_INTERRUPTED_REPO", &repo)
        .output()
        .expect("spawn the run that dies");
    assert_eq!(
        died.status.code(),
        Some(CRASH_EXIT_CODE),
        "the run must die inside its attempt: {}",
        String::from_utf8_lossy(&died.stderr)
    );
    assert_eq!(
        configuration_of(&repo, &[&truthful]),
        configuration,
        "an interrupted run changed no Git configuration"
    );
    let run_id = rundir::latest_run(&repo).expect("the interrupted run");

    let include = include_for(&repo);
    fs::remove_file(&include).expect("the include both runs wrote, gone before the resume");
    let resumed = resume_with(
        &resume_options(&repo, &run_id),
        &fake(Effect::EditFile).probing_with(
            RoleProbe::new(&OVERLAY_SAYS_TRUE).reading_as_the_operator(&probe.join("log"), &blob),
        ),
    )
    .expect("the resume");
    let (records, operator) = take_role_and_operator_records(&probe);
    assert_eq!(
        resumed.outcome(),
        RunOutcome::Complete,
        "{resumed:?}\n{records:#?}"
    );
    assert_every_role_read_the_recorded_graph(&records, "the resume of an interrupted run");
    assert!(
        !operator.is_empty()
            && operator
                .iter()
                .all(|line| line == "operator blob replacing-blob"),
        "{operator:?}"
    );
    assert_eq!(
        fs::read(&include).expect("the include the resume wrote again"),
        crate::runner::host::RECORDED_OBJECTS_INCLUDE
    );
    assert_eq!(
        configuration_of(&repo, &[&truthful]),
        configuration,
        "the resume changed no Git configuration"
    );
}

#[test]
#[ignore = "spawned by the configuration witness"]
fn v1_interrupted_run_helper() {
    let Some(repo) = std::env::var_os("UPSTROKE_V1_INTERRUPTED_REPO") else {
        return;
    };
    let repo = PathBuf::from(repo);
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let _ = run_with(
        &opts,
        &fake(Effect::Exit).probing_with(RoleProbe::new(&OVERLAY_SAYS_TRUE)),
    );
    std::process::exit(0);
}

#[test]
fn sibling_v1_runs_in_one_repository_share_one_include_and_keep_their_roles_recorded() {
    let tree = temp_engine_scratch("v1-siblings");
    let status = crate::workspace_manager::fixture::run_challenged_replacement_witness_child(
        "engine::tests::v1_siblings_helper",
        &[(ROLE_PROBE, tree.path().as_os_str().to_owned())],
    );
    assert!(
        status.success(),
        "the child starts two v0.1 runs at once, in two linked worktrees of one repository \
         with replacements, over one private root with no include in it yet, and ended \
         {status:?}"
    );
}

#[test]
#[ignore = "subprocess helper"]
fn v1_siblings_helper() {
    if std::env::var_os(crate::workspace_manager::fixture::REPLACEMENT_WITNESS).is_none() {
        return;
    }
    crate::workspace_manager::fixture::assert_replacement_controls_pinned("v1-siblings");
    let probe = PathBuf::from(std::env::var_os(ROLE_PROBE).expect("the probe's directory"));
    let (_tree, repo) = temp_engine_repo("v1s");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 depends= -->\n",
        Some(&format!(
            "[interaction]\nmode = \"never\"\n\n{}",
            role_probe_gates()
        )),
    );
    let (blob, spec) = install_a_replaced_graph(&repo, &probe);
    let home = private_root_for(&repo);
    assert!(!home.exists(), "no run has written an include yet");
    let mut checkouts = Vec::new();
    for sibling in ["a", "b"] {
        let checkout = repo.with_file_name(format!("sibling-{sibling}"));
        git_in(
            &repo,
            &[
                "--no-replace-objects",
                "-c",
                "core.useReplaceRefs=false",
                "worktree",
                "add",
                "-q",
                "-b",
                &format!("sibling-{sibling}"),
                &checkout.to_string_lossy(),
            ],
        );
        git_in(
            &checkout,
            &["config", "--worktree", "core.useReplaceRefs", "true"],
        );
        assert_eq!(
            private_root_for(&checkout),
            home,
            "both siblings' runs keep their private half, and the include, under one root"
        );
        let records = probe.join(format!("sibling-{sibling}"));
        write_probe_spec(
            &records,
            &checkout,
            &spec.replace("-candidate", &format!("-candidate-{sibling}")),
        );
        checkouts.push((sibling, checkout, records));
    }
    let configuration = configuration_of(&repo, &[]);
    let children: Vec<_> = checkouts
        .into_iter()
        .map(|(sibling, checkout, records)| {
            let child = Command::new(std::env::current_exe().expect("this test binary"))
                .args([
                    "--exact",
                    "engine::tests::v1_sibling_run_helper",
                    "--ignored",
                    "--nocapture",
                ])
                .env("UPSTROKE_V1_SIBLING_CHECKOUT", &checkout)
                .env(ROLE_PROBE, &records)
                .spawn()
                .expect("spawn a sibling run");
            (sibling, records, child)
        })
        .collect();
    let ended: Vec<_> = children
        .into_iter()
        .map(|(sibling, records, mut child)| {
            (sibling, records, child.wait().expect("a sibling run"))
        })
        .collect();
    for (sibling, records, status) in ended {
        assert!(status.success(), "sibling {sibling} ended {status:?}");
        let (records, _) = take_role_and_operator_records(&records);
        assert_every_role_read_the_recorded_graph(&records, &format!("sibling {sibling}"));
    }
    assert_eq!(
        fs::read(home.join("git").join("recorded-objects.gitconfig")).expect("the shared include"),
        crate::runner::host::RECORDED_OBJECTS_INCLUDE
    );
    assert_eq!(
        configuration_of(&repo, &[]),
        configuration,
        "two runs at once changed no Git configuration"
    );
    assert_eq!(
        git_in(&repo, &["cat-file", "-p", &blob]).trim(),
        "replacing-blob",
        "and the operator's Git still honours the replacements"
    );
}

#[test]
#[ignore = "spawned by the siblings witness"]
fn v1_sibling_run_helper() {
    let Some(checkout) = std::env::var_os("UPSTROKE_V1_SIBLING_CHECKOUT") else {
        return;
    };
    let checkout = PathBuf::from(checkout);
    let mut opts = options(&checkout);
    opts.config_path = Some(checkout.join("upstroke.toml"));
    let report = run_with(
        &opts,
        &fake(Effect::EditFile).probing_with(RoleProbe::new(&[])),
    )
    .expect("a sibling run");
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:?}");
}

fn probe_git_past_another_registrations_write(dir: &Path, args: &[&str]) -> Result<String, String> {
    let mut command = Command::new("git");
    command.arg("-C").arg(dir).args(args);
    probe_answer(past_another_registrations_write(
        dir,
        std::time::Instant::now() + Duration::from_secs(10),
        &mut || command.output(),
    ))
}

fn past_another_registrations_write(
    dir: &Path,
    deadline: std::time::Instant,
    run: &mut dyn FnMut() -> std::io::Result<std::process::Output>,
) -> std::io::Result<std::process::Output> {
    let mut pause = Duration::from_millis(1);
    loop {
        let output = run()?;
        let torn = anothers_empty_commondir_in(&output, dir);
        let now = std::time::Instant::now();
        if !torn || now >= deadline {
            return Ok(output);
        }
        std::thread::sleep(pause.min(deadline.saturating_duration_since(now)));
        pause = pause.saturating_mul(2).min(Duration::from_millis(50));
    }
}

fn anothers_empty_commondir_in(output: &std::process::Output, dir: &Path) -> bool {
    if output.status.code() != Some(128) {
        return false;
    }
    let Some(line) = output.stderr.strip_suffix(b"\n") else {
        return false;
    };
    let Some(registration) = line
        .strip_prefix(b"fatal: failed to read ")
        .and_then(|rest| rest.strip_suffix(b"/commondir: Success"))
    else {
        return false;
    };
    let own = dir
        .file_name()
        .map(std::ffi::OsStr::as_encoded_bytes)
        .unwrap_or_default();
    let mut components = registration.rsplit(|byte| *byte == b'/');
    let id = components.next().unwrap_or_default();
    !line.contains(&b'\n')
        && components.next() == Some(b"worktrees".as_slice())
        && !id.is_empty()
        && !id.starts_with(own)
}

fn probe_answer(output: std::io::Result<std::process::Output>) -> Result<String, String> {
    let out = output.map_err(|error| error.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_owned())
    }
}

#[test]
fn a_gates_worktree_setting_is_attempted_again_only_past_another_registrations_empty_commondir() {
    let dir = Path::new("/r/store/worktrees/upstroke-gates-1-01AAAAAAAAAAAAAAAAAAAAAAAA");
    let other = "fatal: failed to read /r/.git/worktrees/upstroke-gates-2-01BBBBBBBBBBBBBBBBBBBBBBBB\
                 /commondir: Success\n";
    let own = "fatal: failed to read /r/.git/worktrees/upstroke-gates-1-01AAAAAAAAAAAAAAAAAAAAAAAA\
               /commondir: Success\n";
    let numbered = "fatal: failed to read /r/.git/worktrees/upstroke-gates-1-01AAAAAAAAAAAAAAAAAAAAAAAA1\
                    /commondir: Success\n";
    #[cfg(unix)]
    let exited = |code: i32| {
        <std::process::ExitStatus as std::os::unix::process::ExitStatusExt>::from_raw(code << 8)
    };
    #[cfg(windows)]
    let exited = |code: u32| {
        <std::process::ExitStatus as std::os::windows::process::ExitStatusExt>::from_raw(code)
    };
    #[cfg(unix)]
    let signalled =
        Some(<std::process::ExitStatus as std::os::unix::process::ExitStatusExt>::from_raw(15));
    #[cfg(not(unix))]
    let signalled: Option<std::process::ExitStatus> = None;
    let output = |status, stderr: &str| std::process::Output {
        status,
        stdout: Vec::new(),
        stderr: stderr.as_bytes().to_vec(),
    };
    let far = || std::time::Instant::now() + Duration::from_secs(60);
    let attempt = |deadline: std::time::Instant,
                   script: Vec<std::process::Output>|
     -> (Result<std::process::Output, String>, usize) {
        let mut script = script.into_iter();
        let mut probes = 0;
        let read = past_another_registrations_write(dir, deadline, &mut || {
            probes += 1;
            script
                .next()
                .ok_or_else(|| std::io::Error::other("probed past the script"))
        });
        (read.map_err(|error| error.to_string()), probes)
    };
    let torn = output(exited(128), other);
    let set = output(exited(0), "");

    assert_eq!(
        attempt(far(), vec![torn.clone(), torn.clone(), set.clone()]),
        (Ok(set.clone()), 3),
        "past two tears of another registration"
    );
    let at_once = [
        (
            output(exited(1), other),
            "the same line, with an exit that is not Git's death",
        ),
        (
            output(exited(128), other.trim_end_matches('\n')),
            "the same line, unfinished",
        ),
        (
            output(exited(128), &format!("{other}\n")),
            "the same line and an empty one after it",
        ),
        (
            output(exited(128), &format!("warning: w924\n{other}")),
            "a second line",
        ),
        (
            output(
                exited(128),
                &format!("fatal: failed to read /r/one\n{other}"),
            ),
            "two lines, each Git's death",
        ),
        (output(exited(128), own), "its own registration"),
        (
            output(exited(128), numbered),
            "its own, under the suffix Git gives a taken name",
        ),
        (
            output(
                exited(128),
                "fatal: failed to read /r/.git/worktrees/w/commondir: No such file or directory\n",
            ),
            "a removal's",
        ),
        (
            output(
                exited(128),
                "fatal: failed to read '/r/.git/worktrees/w/locked'\n",
            ),
            "a lock's",
        ),
        (
            output(
                exited(128),
                "fatal: --worktree cannot be used with multiple working trees\n",
            ),
            "another refusal",
        ),
    ]
    .into_iter()
    .chain(signalled.map(|status| {
        (
            output(status, other),
            "the same line from a Git a signal ended",
        )
    }));
    for (answer, why) in at_once {
        assert_eq!(
            attempt(far(), vec![answer.clone(), set.clone()]),
            (Ok(answer), 1),
            "{why}, at once"
        );
    }
    assert_eq!(
        attempt(std::time::Instant::now(), vec![torn.clone(), set.clone()]),
        (Ok(torn), 1),
        "a deadline already past"
    );
    assert_eq!(
        attempt(far(), vec![set.clone()]),
        (Ok(set), 1),
        "a success, at once"
    );
    assert_eq!(
        attempt(far(), Vec::new()),
        (Err("probed past the script".to_owned()), 1),
        "a probe that cannot start, at once"
    );
}

#[test]
fn a_gates_worktree_setting_answers_what_the_probe_answers_when_nothing_tears() {
    let (_tree, repo) = temp_engine_repo("w924-probe-answers");
    let missing = repo.join("w924-no-such-directory");
    for (dir, args) in [
        (repo.as_path(), &["rev-parse", "--is-inside-work-tree"][..]),
        (
            repo.as_path(),
            &["rev-parse", "--verify", "w924-no-such-revision"],
        ),
        (repo.as_path(), &["config", "--w924-no-such-option"]),
        (
            missing.as_path(),
            &["config", "--worktree", "core.useReplaceRefs", "true"],
        ),
    ] {
        assert_eq!(
            probe_git_past_another_registrations_write(dir, args),
            probe_git(dir, args),
            "{args:?} in {}",
            dir.display()
        );
    }
}

#[cfg(unix)]
#[test]
fn the_gate_role_sets_its_worktree_value_again_only_past_another_registrations_empty_commondir() {
    use std::os::unix::fs::PermissionsExt;

    let tree = temp_engine_scratch("w924-gate-probe");
    let other = "fatal: failed to read /r/.git/worktrees/upstroke-gates-2/commondir: Success";
    let own = "fatal: failed to read /r/.git/worktrees/upstroke-gates-1/commondir: Success";
    let refused = format!("Err({other:?})");
    for (case, stderr, ending, probes, recorded) in [
        (
            "another-registration",
            format!("{other}\\n"),
            "exit 128",
            2,
            "ok Ok(\"\")".to_owned(),
        ),
        (
            "exit-1",
            format!("{other}\\n"),
            "exit 1",
            1,
            format!("FAIL {refused}"),
        ),
        (
            "signal",
            format!("{other}\\n"),
            "kill -KILL $$",
            1,
            format!("FAIL {refused}"),
        ),
        (
            "unfinished",
            other.to_owned(),
            "exit 128",
            1,
            format!("FAIL {refused}"),
        ),
        (
            "extra-blank-line",
            format!("{other}\\n\\n"),
            "exit 128",
            1,
            format!("FAIL {refused}"),
        ),
        (
            "own-registration",
            format!("{own}\\n"),
            "exit 128",
            1,
            format!("FAIL Err({own:?})"),
        ),
        (
            "unrelated",
            "fatal: w924 unrelated\\n".to_owned(),
            "exit 128",
            1,
            "FAIL Err(\"fatal: w924 unrelated\")".to_owned(),
        ),
    ] {
        let place = tree.path().join(case);
        let here = place.join("upstroke-gates-1");
        let probe = place.join("probe");
        let bin = place.join("bin");
        for directory in [&here, &probe, &bin] {
            fs::create_dir_all(directory).expect("the case's directories");
        }
        fs::write(probe.join("spec"), "worktree true\n").expect("the gate's spec");
        let calls = place.join("calls");
        let answered = place.join("answered");
        let git = bin.join("git");
        fs::write(
            &git,
            format!(
                "#!/bin/sh\n\
                 if [ \"$3 $4 $5 $6\" = 'config --worktree core.useReplaceRefs true' ]; then\n\
                 \x20 echo probed >> \"$UPSTROKE_W924_CALLS\"\n\
                 \x20 if [ ! -e \"$UPSTROKE_W924_ANSWERED\" ]; then\n\
                 \x20   : > \"$UPSTROKE_W924_ANSWERED\"\n\
                 \x20   printf '{stderr}' >&2\n\
                 \x20   {ending}\n\
                 \x20 fi\n\
                 \x20 exit 0\n\
                 fi\n\
                 exit 1\n"
            ),
        )
        .expect("the case's Git");
        fs::set_permissions(&git, fs::Permissions::from_mode(0o700)).expect("an executable Git");
        Command::new(std::env::current_exe().expect("this test binary"))
            .args([
                "--exact",
                "engine::tests::v1_role_probe_gate",
                "--ignored",
                "--nocapture",
            ])
            .current_dir(&here)
            .env(ROLE_PROBE, &probe)
            .env("UPSTROKE_W924_CALLS", &calls)
            .env("UPSTROKE_W924_ANSWERED", &answered)
            .env(
                "PATH",
                std::env::join_paths([bin.as_path(), Path::new("/usr/bin"), Path::new("/bin")])
                    .expect("a synthetic PATH"),
            )
            .output()
            .expect("the gate role");
        let records: Vec<_> = fs::read_dir(probe.join("log"))
            .expect("the gate's records")
            .map(|entry| fs::read_to_string(entry.expect("a record").path()).expect("its text"))
            .collect();
        let setting: Vec<_> = records
            .iter()
            .flat_map(|record| record.lines())
            .filter_map(|line| line.strip_prefix("gate worktree-true "))
            .collect();
        assert_eq!(setting, [recorded.as_str()], "{case}: {records:#?}");
        assert_eq!(
            fs::read_to_string(&calls)
                .expect("the setting's calls")
                .lines()
                .count(),
            probes,
            "{case}"
        );
    }
}

#[test]
fn the_v1_include_names_the_managed_repository_however_its_path_is_spelled() {
    let tree = temp_engine_scratch("v1-shapes");
    let status = crate::workspace_manager::fixture::run_challenged_replacement_witness_child(
        "engine::tests::v1_shapes_helper",
        &[(ROLE_PROBE, tree.path().as_os_str().to_owned())],
    );
    assert!(
        status.success(),
        "the child runs a v0.1 task in repositories with replacements whose paths carry \
         glob characters, spaces and a quote, that are entered through a symbolic link, \
         keep their Git directory elsewhere or behind a link, are linked worktrees, or are \
         spelled in another case, and ended {status:?}"
    );
}

fn shaped_engine_repo(repo: &Path) {
    git_in(repo, &["config", "user.email", "test@upstroke.local"]);
    git_in(repo, &["config", "user.name", "upstroke tests"]);
    fs::write(repo.join("README.md"), "seed\n").expect("seed");
    seed(
        repo,
        "## Implement the widget\n<!-- upstroke: id=t1 depends= -->\n",
        Some(&format!(
            "[interaction]\nmode = \"never\"\n\n{}",
            role_probe_gates()
        )),
    );
}

#[test]
#[ignore = "subprocess helper"]
fn v1_shapes_helper() {
    if std::env::var_os(crate::workspace_manager::fixture::REPLACEMENT_WITNESS).is_none() {
        return;
    }
    crate::workspace_manager::fixture::assert_replacement_controls_pinned("v1-shapes");
    let probe = PathBuf::from(std::env::var_os(ROLE_PROBE).expect("the probe's directory"));
    let tree = temp_engine_scratch("v1p");
    let root = tree.path();
    let init = |repo: &Path, args: &[&str]| {
        fs::create_dir_all(repo).expect("the repository's directory");
        let mut all = vec!["init", "-q", "-b", "main"];
        all.extend_from_slice(args);
        git_in(repo, &all);
    };
    let mut shapes: Vec<(&str, PathBuf)> = Vec::new();

    let spelled = if cfg!(windows) {
        "a [x] '"
    } else {
        "a [x]*? '"
    };
    let repo = root.join(spelled).join("repo");
    init(&repo, &[]);
    shapes.push(("glob characters, spaces and a quote", repo));

    let store = root.join("s").join("store.git");
    let repo = root.join("s").join("repo");
    init(&repo, &["--separate-git-dir", &store.to_string_lossy()]);
    shapes.push(("a separate Git directory", repo));

    let main = root.join("l").join("main");
    init(&main, &[]);
    shaped_engine_repo(&main);
    let repo = root.join("l").join("repo");
    git_in(
        &main,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "linked",
            &repo.to_string_lossy(),
        ],
    );
    shapes.push(("entered through a linked worktree", repo));

    #[cfg(unix)]
    {
        let real = root.join("e").join("real");
        init(&real, &[]);
        let link = root.join("e").join("repo");
        std::os::unix::fs::symlink(&real, &link).expect("a link to the repository");
        shapes.push(("entered through a symbolic link", link));

        let repo = root.join("g").join("repo");
        init(&repo, &[]);
        let elsewhere = root.join("g").join("elsewhere.git");
        fs::rename(repo.join(".git"), &elsewhere).expect("the Git directory, moved");
        std::os::unix::fs::symlink(&elsewhere, repo.join(".git")).expect("a link to it");
        shapes.push(("a Git directory behind a symbolic link", repo));
    }
    if cfg!(any(windows, target_os = "macos")) {
        let repo = root.join("c").join("repo");
        init(&repo, &[]);
        shapes.push(("spelled in another case", root.join("c").join("REPO")));
    }

    for (shape, entry) in shapes {
        if shape != "entered through a linked worktree" {
            shaped_engine_repo(&entry);
        }
        let (blob, spec) = install_a_replaced_graph(&entry, &probe);
        write_probe_spec(&probe, &entry, &spec);
        let mut opts = options(&entry);
        opts.config_path = Some(entry.join("upstroke.toml"));
        let report = run_with(
            &opts,
            &fake(Effect::EditFile).probing_with(RoleProbe::new(&[])),
        )
        .unwrap_or_else(|error| panic!("{shape}: the run: {error}"));
        let (records, _) = take_role_and_operator_records(&probe);
        assert_eq!(
            report.outcome(),
            RunOutcome::Complete,
            "{shape}: {report:?}\n{records:#?}"
        );
        assert_every_role_read_the_recorded_graph(&records, shape);
        assert_eq!(
            git_in(&entry, &["cat-file", "-p", &blob]).trim(),
            "replacing-blob",
            "{shape}: the operator's Git still honours the replacements"
        );
    }
}

const TORN_REGISTRATION: &str = "foreign-torn";

fn after_capture_failed(error: impl std::fmt::Display) -> UpstrokeError {
    UpstrokeError::Git {
        message: format!("the test's after-capture step: {error}"),
    }
}

fn git_after_capture(root: &Path, args: &[&str]) -> Result<String, UpstrokeError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(after_capture_failed)?;
    if !output.status.success() {
        return Err(after_capture_failed(format!(
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn torn_registration_of(repo: &Path) -> PathBuf {
    repo.join(".git").join("worktrees").join(TORN_REGISTRATION)
}

fn plant_a_torn_registration_in(repo: &Path) -> std::io::Result<PathBuf> {
    let admin = torn_registration_of(repo);
    fs::create_dir_all(&admin)?;
    let checkout = repo.with_file_name("torn-checkout").join(".git");
    let mut gitdir =
        crate::runner::host::GitdirRule::native().spelling(checkout.as_os_str().as_encoded_bytes());
    gitdir.push(b'\n');
    fs::write(admin.join("gitdir"), gitdir)?;
    fs::write(admin.join("commondir"), b"")?;
    Ok(admin)
}

fn repair_the_torn_registration(repo: &Path) {
    fs::remove_dir_all(torn_registration_of(repo)).expect("the operator removes the residue");
}

fn captured_marker(repo: &Path) -> PathBuf {
    repo.with_file_name("captured.txt")
}

fn record_the_capture(
    workspace: &Workspace,
    candidate: &crate::workspace::CapturedCandidate,
) -> Result<(), UpstrokeError> {
    fs::write(
        captured_marker(workspace.root()),
        format!(
            "{} {} {}",
            candidate.branch_ref, candidate.parent_oid, candidate.tree_oid
        ),
    )
    .map_err(after_capture_failed)
}

fn captured(repo: &Path) -> (String, String, String) {
    let text = fs::read_to_string(captured_marker(repo)).expect("the capture was recorded");
    let mut parts = text.split_whitespace().map(str::to_owned);
    let mut next = || parts.next().expect("three captured identities");
    (next(), next(), next())
}

fn tear_after_capture(
    workspace: &Workspace,
    candidate: &crate::workspace::CapturedCandidate,
) -> Result<(), UpstrokeError> {
    record_the_capture(workspace, candidate)?;
    plant_a_torn_registration_in(workspace.root())
        .map(|_| ())
        .map_err(after_capture_failed)
}

fn unstage_and_tear_after_capture(
    workspace: &Workspace,
    candidate: &crate::workspace::CapturedCandidate,
) -> Result<(), UpstrokeError> {
    record_the_capture(workspace, candidate)?;
    git_after_capture(
        workspace.root(),
        &["reset", "-q", "HEAD", "--", "agent-output.txt"],
    )?;
    plant_a_torn_registration_in(workspace.root())
        .map(|_| ())
        .map_err(after_capture_failed)
}

fn move_head_and_tear_after_capture(
    workspace: &Workspace,
    candidate: &crate::workspace::CapturedCandidate,
) -> Result<(), UpstrokeError> {
    record_the_capture(workspace, candidate)?;
    let root = workspace.root();
    let moved = git_after_capture(
        root,
        &[
            "commit-tree",
            "HEAD^{tree}",
            "-p",
            "HEAD",
            "-m",
            "moved after capture",
        ],
    )?;
    git_after_capture(root, &["update-ref", &candidate.branch_ref, &moved])?;
    plant_a_torn_registration_in(root)
        .map(|_| ())
        .map_err(after_capture_failed)
}

fn block_the_kept_pin_and_tear_after_capture(
    workspace: &Workspace,
    candidate: &crate::workspace::CapturedCandidate,
) -> Result<(), UpstrokeError> {
    record_the_capture(workspace, candidate)?;
    let root = workspace.root();
    let run_id =
        rundir::latest_run(root).ok_or_else(|| after_capture_failed("no run has started"))?;
    let below = format!(
        "{}{}/blocker",
        super::coordinator::prepared_pin_ref(&run_id, 0, 1),
        crate::workspace::KEPT_PIN_SUFFIX
    );
    git_after_capture(root, &["update-ref", &below, &candidate.parent_oid])?;
    plant_a_torn_registration_in(root)
        .map(|_| ())
        .map_err(after_capture_failed)
}

fn block_the_snapshot_store_after_capture(
    workspace: &Workspace,
    candidate: &crate::workspace::CapturedCandidate,
) -> Result<(), UpstrokeError> {
    record_the_capture(workspace, candidate)?;
    let run_id = rundir::latest_run(workspace.root())
        .ok_or_else(|| after_capture_failed("no run has started"))?;
    let store = paths_of(workspace.root(), &run_id).gate_worktrees();
    fs::create_dir_all(&store).map_err(after_capture_failed)?;
    let intents = store.join("intents");
    if intents.is_dir() {
        fs::remove_dir_all(&intents).map_err(after_capture_failed)?;
    }
    fs::write(&intents, "not a directory\n").map_err(after_capture_failed)
}

fn fail_after_capture(
    workspace: &Workspace,
    candidate: &crate::workspace::CapturedCandidate,
) -> Result<(), UpstrokeError> {
    record_the_capture(workspace, candidate)?;
    Err(after_capture_failed(
        "an attempt error that is not the registry's",
    ))
}

fn replace_the_new_blob_and_tear_after_capture(
    workspace: &Workspace,
    candidate: &crate::workspace::CapturedCandidate,
) -> Result<(), UpstrokeError> {
    record_the_capture(workspace, candidate)?;
    let root = workspace.root();
    let listing = git_after_capture(
        root,
        &[
            "-c",
            "core.useReplaceRefs=false",
            "ls-tree",
            &candidate.tree_oid,
            "new.txt",
        ],
    )?;
    let original = listing
        .split_whitespace()
        .nth(2)
        .ok_or_else(|| after_capture_failed("no new.txt in the captured tree"))?
        .to_owned();
    let other = root.with_file_name("replacement.txt");
    fs::write(&other, "replacement content\n").map_err(after_capture_failed)?;
    let replacement = git_after_capture(root, &["hash-object", "-w", &other.to_string_lossy()])?;
    git_after_capture(root, &["replace", &original, &replacement])?;
    plant_a_torn_registration_in(root)
        .map(|_| ())
        .map_err(after_capture_failed)
}

fn replace_the_captured_tree_and_tear_after_capture(
    workspace: &Workspace,
    candidate: &crate::workspace::CapturedCandidate,
) -> Result<(), UpstrokeError> {
    record_the_capture(workspace, candidate)?;
    let root = workspace.root();
    let parent_tree = git_after_capture(
        root,
        &[
            "-c",
            "core.useReplaceRefs=false",
            "rev-parse",
            &format!("{}^{{tree}}", candidate.parent_oid),
        ],
    )?;
    git_after_capture(root, &["replace", &candidate.tree_oid, &parent_tree])?;
    plant_a_torn_registration_in(root)
        .map(|_| ())
        .map_err(after_capture_failed)
}

fn one_gated_task(repo: &Path) {
    seed(
        repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some(
            "[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n\n\
             [[gates]]\nname = \"version\"\ncmd = \"git --version\"\n",
        ),
    );
}

fn refusal_options(
    repo: &Path,
    after_capture: super::options::AfterCandidateCapture,
) -> RunOptions {
    let mut opts = options(repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    opts.after_candidate_capture = Some(after_capture);
    opts
}

fn resume_the_run(
    repo: &Path,
    run_id: &str,
    adapters: &dyn AdapterSource,
) -> Result<RunReport, UpstrokeError> {
    resume_harness(
        &resume_options(repo, run_id),
        &Harness {
            adapters,
            answers: None,
            sleeper: None,
        },
    )
}

fn kept_pins(repo: &Path) -> Vec<(String, String)> {
    git_in(
        repo,
        &[
            "for-each-ref",
            "--format=%(refname) %(objectname)",
            "refs/upstroke/prepared/",
        ],
    )
    .lines()
    .filter_map(|line| line.split_once(' '))
    .filter(|(name, _)| name.ends_with("-kept"))
    .map(|(name, oid)| (name.to_owned(), oid.to_owned()))
    .collect()
}

fn registry_refusal_text(result: &Result<RunReport, UpstrokeError>, what: &str) -> String {
    match result {
        Err(UpstrokeError::RegistryRefused { message }) => message.clone(),
        Err(other) => panic!("{what}: a registry refusal, never {other:?}"),
        Ok(report) => panic!("{what}: a registry refusal, not {report:?}"),
    }
}

fn kept_pin_warning(report: &RunReport) -> Option<&String> {
    report
        .warnings
        .iter()
        .find(|warning| warning.contains("-kept"))
}

fn tree_of(repo: &Path, revision: &str) -> String {
    git_in(
        repo,
        &[
            "-c",
            "core.useReplaceRefs=false",
            "rev-parse",
            &format!("{revision}^{{tree}}"),
        ],
    )
    .trim()
    .to_owned()
}

fn status_of(repo: &Path) -> String {
    git_in(
        repo,
        &[
            "--no-replace-objects",
            "-c",
            "core.useReplaceRefs=false",
            "status",
            "--porcelain",
        ],
    )
}

fn advertised_commands(warning: &str, pin: &str) -> Vec<String> {
    warning
        .split('`')
        .skip(1)
        .step_by(2)
        .filter(|span| span.starts_with("git "))
        .map(|span| span.replace("<pin>", pin))
        .collect()
}

fn subcommand_of(command: &str) -> String {
    let mut words = command.split_whitespace().skip(1);
    while let Some(word) = words.next() {
        if word == "-c" || word == "-C" {
            words.next();
            continue;
        }
        if word.starts_with('-') {
            continue;
        }
        return word.to_owned();
    }
    String::new()
}

fn run_the_advertised(repo: &Path, command: &str) -> std::process::ExitStatus {
    let mut words = command.split_whitespace();
    assert_eq!(words.next(), Some("git"), "{command}");
    Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(words)
        .status()
        .expect("run the advertised command")
}

fn follow_the_restore(repo: &Path, warning: &str, pin: &str) -> String {
    let command = advertised_commands(warning, pin)
        .into_iter()
        .find(|command| matches!(subcommand_of(command).as_str(), "restore" | "checkout"))
        .expect("the warning advertises a command for a HEAD still at the pin's parent");
    assert!(run_the_advertised(repo, &command).success(), "{command}");
    command
}

fn follow_the_pick(repo: &Path, warning: &str, pin: &str) -> String {
    let commands = advertised_commands(warning, pin);
    let restore = commands
        .iter()
        .position(|command| matches!(subcommand_of(command).as_str(), "restore" | "checkout"))
        .expect("the warning advertises a restore");
    let command = commands
        .into_iter()
        .skip(restore + 1)
        .find(|command| !matches!(subcommand_of(command).as_str(), "update-ref" | "diff"))
        .expect("the warning advertises a command for a later HEAD");
    assert!(run_the_advertised(repo, &command).success(), "{command}");
    command
}

fn recorded(repo: &Path, args: &[&str]) -> String {
    let mut full = vec!["--no-replace-objects", "-c", "core.useReplaceRefs=false"];
    full.extend_from_slice(args);
    git_in(repo, &full)
}

fn differences_from_the_pin(repo: &Path, pin: &str) -> Vec<String> {
    let mut problems = Vec::new();
    let mut paths = std::collections::BTreeSet::new();
    for entry in recorded(repo, &["ls-tree", "-r", "-z", "--full-tree", pin])
        .split('\0')
        .filter(|entry| !entry.is_empty())
    {
        let (meta, path) = entry.split_once('\t').expect("a tree entry");
        let oid = meta.split_whitespace().nth(2).expect("an object id");
        paths.insert(path.to_owned());
        let bytes = recorded(repo, &["cat-file", "blob", oid]);
        match fs::read_to_string(repo.join(path)) {
            Ok(text) if text == bytes => {}
            Ok(text) => problems.push(format!("{path} holds {text:?}, the pin records {bytes:?}")),
            Err(error) => problems.push(format!("{path} is not in the checkout: {error}")),
        }
    }
    for path in recorded(repo, &["ls-files", "-z"])
        .split('\0')
        .filter(|path| !path.is_empty())
    {
        if !paths.contains(path) {
            problems.push(format!("{path} is in the index, not in the pin"));
        }
    }
    let index = recorded(repo, &["write-tree"]).trim().to_owned();
    if index != tree_of(repo, pin) {
        problems.push(format!("the index's tree is {index}, not the pin's"));
    }
    problems
}

struct ShapedWorker {
    inner: FakeAdapter,
}

impl AgentAdapter for ShapedWorker {
    fn id(&self) -> &'static str {
        self.inner.id()
    }

    fn probe(&self, runner: &dyn crate::runner::Runner) -> Result<Caps, UpstrokeError> {
        self.inner.probe(runner)
    }

    fn build(&self, run: &TaskRun) -> Result<CommandSpec, UpstrokeError> {
        if run.profile.permissions != PermissionMode::ReadOnly {
            let write = |name: &str, text: &str| fs::write(run.workspace.join(name), text);
            write("tracked.txt", "paid edit\n").map_err(after_capture_failed)?;
            write("new.txt", "paid new file\n").map_err(after_capture_failed)?;
            fs::remove_file(run.workspace.join("deleted.txt")).map_err(after_capture_failed)?;
        }
        self.inner.build(run)
    }

    fn parse(&self, out: &ProcessOutput) -> Result<Outcome, UpstrokeError> {
        self.inner.parse(out)
    }

    fn materialize_permissions(
        &self,
        profile: &WorkerProfile,
        gate_cmds: &[String],
        dir: &Path,
        stem: &str,
    ) -> Result<Option<PathBuf>, UpstrokeError> {
        self.inner
            .materialize_permissions(profile, gate_cmds, dir, stem)
    }
}

struct OneAdapter<A: AgentAdapter> {
    adapter: A,
}

impl<A: AgentAdapter> AdapterSource for OneAdapter<A> {
    fn get(&self, id: &str) -> Option<&dyn AgentAdapter> {
        (id == self.adapter.id()).then_some(&self.adapter as &dyn AgentAdapter)
    }
}

fn shaped_worker() -> OneAdapter<ShapedWorker> {
    OneAdapter {
        adapter: ShapedWorker {
            inner: FakeAdapter::new(vec![Effect::EditFile], vec![ReviewBehavior::Pass]),
        },
    }
}

fn shaped_repo(tag: &str) -> (ScratchTree, PathBuf) {
    let (tree, repo) = temp_engine_repo(tag);
    git_in(&repo, &["config", "core.autocrlf", "false"]);
    fs::write(repo.join("tracked.txt"), "base\n").expect("a tracked file");
    fs::write(repo.join("deleted.txt"), "base\n").expect("a file the worker deletes");
    one_gated_task(&repo);
    (tree, repo)
}

#[test]
fn a_registry_refusal_after_capture_keeps_and_pins_the_captured_candidate_across_both_resumes() {
    let (_tree, repo) = temp_engine_repo("kept-static");
    one_gated_task(&repo);
    let run = run_with(
        &refusal_options(&repo, tear_after_capture),
        &fake(Effect::EditFile),
    );
    let refusal = registry_refusal_text(&run, "the run beside a tear that stays");
    let (_, parent, tree) = captured(&repo);
    let status = status_of(&repo);
    let kept = kept_pins(&repo);
    let [(pin, commit)] = kept.as_slice() else {
        panic!("one kept pin: {kept:?}");
    };
    assert!(
        refusal.contains(pin.as_str()) && refusal.contains("is kept in this checkout"),
        "the refusal names the pin: {refusal}"
    );
    assert!(
        status.contains("agent-output.txt"),
        "the checkout still holds the candidate: {status}"
    );
    assert_eq!(
        tree_of(&repo, commit),
        tree,
        "the pin holds the captured tree"
    );
    assert_eq!(
        git_in(&repo, &["rev-parse", &format!("{commit}^")]).trim(),
        parent,
        "on the captured parent"
    );

    let run_id = rundir::latest_run(&repo).expect("the run started");
    let while_torn = resume_the_run(&repo, &run_id, &fake(Effect::EditFile));
    registry_refusal_text(&while_torn, "the resume while the residue stays");
    assert_eq!(
        status_of(&repo),
        status,
        "the refused resume discards nothing"
    );
    assert_eq!(kept_pins(&repo), kept, "and moves no pin");

    repair_the_torn_registration(&repo);
    let report = resume_the_run(&repo, &run_id, &fake(Effect::EditFile))
        .expect("the resume after the repair");
    assert!(committed(&report, "t1"), "{report:?}");
    let warning = kept_pin_warning(&report).expect("the resume names the kept pin");
    assert!(warning.contains(pin.as_str()), "{warning}");
    assert_eq!(kept_pins(&repo), kept, "no resume removes a kept pin");
}

#[test]
fn a_kept_pin_holds_the_captured_tree_when_the_index_changed_after_capture() {
    let (_tree, repo) = temp_engine_repo("kept-index");
    git_in(&repo, &["config", "core.autocrlf", "false"]);
    one_gated_task(&repo);
    let run = run_with(
        &refusal_options(&repo, unstage_and_tear_after_capture),
        &fake(Effect::EditFile),
    );
    registry_refusal_text(&run, "the run beside a tear that stays");
    let (_, _, tree) = captured(&repo);
    let index = git_in(&repo, &["write-tree"]).trim().to_owned();
    assert_ne!(index, tree, "the index changed after the capture");
    let kept = kept_pins(&repo);
    let [(pin, commit)] = kept.as_slice() else {
        panic!("one kept pin: {kept:?}");
    };
    assert_eq!(
        tree_of(&repo, commit),
        tree,
        "the pin holds the captured candidate, never the index as it stands at the refusal"
    );
    let paid = fs::read_to_string(repo.join("agent-output.txt")).expect("the paid output");

    repair_the_torn_registration(&repo);
    let run_id = rundir::latest_run(&repo).expect("the run started");
    let report =
        resume_the_run(&repo, &run_id, &fake(Effect::NoEdit)).expect("the resume after the repair");
    assert!(
        !repo.join("agent-output.txt").exists(),
        "the resume discarded the checkout's copy"
    );
    let warning = kept_pin_warning(&report).expect("the resume names the kept pin");
    follow_the_restore(&repo, warning, pin);
    assert_eq!(
        fs::read_to_string(repo.join("agent-output.txt")).expect("the restored output"),
        paid,
        "following the warning brings the paid output back"
    );
}

#[test]
fn following_the_kept_pin_warning_at_its_parent_restores_deletions_too() {
    let (_tree, repo) = shaped_repo("kept-restore");
    let run = run_with(
        &refusal_options(&repo, tear_after_capture),
        &shaped_worker(),
    );
    registry_refusal_text(&run, "the shaped run beside a tear that stays");
    let kept = kept_pins(&repo);
    let [(pin, commit)] = kept.as_slice() else {
        panic!("one kept pin: {kept:?}");
    };
    let kept_change = git_in(
        &repo,
        &["diff", "--name-status", &format!("{commit}^"), commit],
    );
    assert!(kept_change.contains("D\tdeleted.txt"), "{kept_change}");

    repair_the_torn_registration(&repo);
    let run_id = rundir::latest_run(&repo).expect("the run started");
    let report =
        resume_the_run(&repo, &run_id, &fake(Effect::NoEdit)).expect("the resume after the repair");
    let warning = kept_pin_warning(&report).expect("the resume names the kept pin");
    let command = follow_the_restore(&repo, warning, pin);
    assert_eq!(
        differences_from_the_pin(&repo, pin),
        Vec::<String>::new(),
        "`{command}` restores the kept tree exactly"
    );
    assert!(
        !repo.join("deleted.txt").exists(),
        "the deletion is restored too"
    );
}

#[test]
fn following_the_kept_pin_warning_on_a_later_head_applies_exactly_the_kept_change() {
    let (_tree, repo) = shaped_repo("kept-later");
    let run = run_with(
        &refusal_options(&repo, tear_after_capture),
        &shaped_worker(),
    );
    registry_refusal_text(&run, "the shaped run beside a tear that stays");
    let kept = kept_pins(&repo);
    let [(pin, commit)] = kept.as_slice() else {
        panic!("one kept pin: {kept:?}");
    };

    repair_the_torn_registration(&repo);
    let run_id = rundir::latest_run(&repo).expect("the run started");
    let report = resume_the_run(&repo, &run_id, &fake(Effect::EditFile))
        .expect("the resume after the repair");
    assert!(committed(&report, "t1"), "{report:?}");
    assert_ne!(
        git_in(&repo, &["rev-parse", "HEAD"]).trim(),
        git_in(&repo, &["rev-parse", &format!("{commit}^")]).trim(),
        "HEAD advanced past the pin's parent"
    );
    let warning = kept_pin_warning(&report).expect("the resume names the kept pin");
    let command = follow_the_pick(&repo, warning, pin);
    let mut staged: Vec<String> = git_in(&repo, &["diff", "--cached", "--name-status", "HEAD"])
        .lines()
        .map(str::to_owned)
        .collect();
    staged.sort_unstable();
    assert_eq!(
        staged,
        ["A\tnew.txt", "D\tdeleted.txt", "M\ttracked.txt"],
        "`{command}` stages exactly the kept change"
    );
}

fn a_torn_topology_slot(repo: &Path) -> (crate::workspace_manager::WorkspaceManager, PathBuf) {
    use crate::workspace_manager::{NoHooks, Slot, WorkspaceManager};
    let private = repo.with_file_name("topo");
    fs::create_dir_all(&private).expect("the topology run's private root");
    let manager = WorkspaceManager::derive(
        repo,
        &private,
        crate::workspace_manager::fixture::RUN_ID,
        "inc-1",
    )
    .expect("a topology manager of the same repository");
    manager
        .create_execution_root(&mut NoHooks)
        .expect("its execution root");
    let slot = Slot::Task {
        key: "alpha".to_owned(),
        generation: 1,
    };
    manager
        .write_intent(&mut NoHooks, &slot)
        .expect("its intent");
    let head = git_in(repo, &["rev-parse", "HEAD"]).trim().to_owned();
    let path = manager
        .add_worktree(&mut NoHooks, &slot, &head)
        .expect("a topology slot");
    let admin = crate::workspace_manager::fixture::tear_registration(&manager, &path);
    (manager, admin)
}

fn finish_the_topology_registration(admin: &Path) {
    fs::write(admin.join("commondir"), "../..\n").expect("its writer writes commondir");
    fs::remove_file(admin.join("locked")).expect("and unlocks");
}

#[test]
fn a_topology_slots_torn_registration_refuses_a_legacy_snapshot_and_keeps_its_output() {
    let (_tree, repo) = temp_engine_repo("kept-topo");
    one_gated_task(&repo);
    let (_manager, admin) = a_torn_topology_slot(&repo);
    let run = run_with(
        &refusal_options(&repo, record_the_capture),
        &fake(Effect::EditFile),
    );
    registry_refusal_text(&run, "the run beside a topology writer's residue");
    let (_, _, tree) = captured(&repo);
    assert!(status_of(&repo).contains("agent-output.txt"));
    let kept = kept_pins(&repo);
    let [(pin, commit)] = kept.as_slice() else {
        panic!("one kept pin: {kept:?}");
    };
    assert_eq!(
        tree_of(&repo, commit),
        tree,
        "the pin holds the captured tree"
    );

    finish_the_topology_registration(&admin);
    let run_id = rundir::latest_run(&repo).expect("the run started");
    let report = resume_the_run(&repo, &run_id, &fake(Effect::EditFile))
        .expect("the resume after the topology writer finished");
    assert!(committed(&report, "t1"), "{report:?}");
    let warning = kept_pin_warning(&report).expect("the resume names the kept pin");
    assert!(warning.contains(pin.as_str()), "{warning}");
    assert_eq!(kept_pins(&repo), kept, "no resume removes a kept pin");
}

#[test]
fn an_attempt_error_that_is_not_a_registry_refusal_keeps_the_output_pinned() {
    let (_tree, repo) = temp_engine_repo("kept-not-reg");
    one_gated_task(&repo);
    let run = run_with(
        &refusal_options(&repo, fail_after_capture),
        &fake(Effect::EditFile),
    );
    let Err(UpstrokeError::WithWarnings(warned)) = &run else {
        panic!("the attempt's own error, with the kept note beside it: {run:?}");
    };
    assert!(
        matches!(warned.error.as_ref(), UpstrokeError::Git { message } if message.contains("not the registry's")),
        "the error is the attempt's own, not a registry refusal: {run:?}"
    );
    assert!(
        warned
            .warnings
            .iter()
            .any(|warning| warning.contains("is kept in this checkout and pinned at")),
        "{run:?}"
    );
    assert!(
        status_of(&repo).contains("agent-output.txt"),
        "nothing discarded the attempt's output"
    );
    let kept = kept_pins(&repo);
    let [(_, commit)] = kept.as_slice() else {
        panic!("one kept pin: {kept:?}");
    };
    assert!(
        git_in(&repo, &["ls-tree", "-r", "--name-only", commit]).contains("agent-output.txt"),
        "the pin holds the output"
    );
}

#[test]
fn a_snapshot_failure_that_is_not_the_registrys_keeps_the_output_pinned() {
    let (_tree, repo) = temp_engine_repo("kept-store");
    one_gated_task(&repo);
    let run = run_with(
        &refusal_options(&repo, block_the_snapshot_store_after_capture),
        &fake(Effect::EditFile),
    );
    assert!(
        run.is_err() && !matches!(&run, Err(UpstrokeError::RegistryRefused { .. })),
        "a snapshot store that cannot be made is not the registry's refusal: {run:?}"
    );
    assert!(
        status_of(&repo).contains("agent-output.txt"),
        "nothing discarded the attempt's output"
    );
    assert_eq!(kept_pins(&repo).len(), 1, "the output is pinned");
}

#[test]
fn a_kept_pin_that_cannot_be_written_is_reported_and_nothing_is_discarded() {
    let (_tree, repo) = temp_engine_repo("kept-unpinned");
    one_gated_task(&repo);
    let run = run_with(
        &refusal_options(&repo, block_the_kept_pin_and_tear_after_capture),
        &fake(Effect::EditFile),
    );
    let refusal = registry_refusal_text(&run, "the run whose kept pin's name a ref blocks");
    assert!(
        refusal.contains("is kept in this checkout, and pinning it at")
            && refusal.contains("failed"),
        "the refusal says the pin failed: {refusal}"
    );
    assert!(
        status_of(&repo).contains("agent-output.txt"),
        "nothing is discarded"
    );
    assert_eq!(kept_pins(&repo), Vec::<(String, String)>::new());
}

#[test]
fn a_review_snapshot_refused_after_capture_keeps_and_pins_the_candidate() {
    let (_tree, repo) = temp_engine_repo("kept-review");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 depends= -->\n",
        Some("[interaction]\nmode = \"never\"\n"),
    );
    let run = run_with(
        &refusal_options(&repo, tear_after_capture),
        &fake(Effect::EditFile),
    );
    let refusal = registry_refusal_text(&run, "the review snapshot beside a tear that stays");
    let (_, _, tree) = captured(&repo);
    assert!(status_of(&repo).contains("agent-output.txt"));
    let kept = kept_pins(&repo);
    let [(pin, commit)] = kept.as_slice() else {
        panic!("one kept pin: {kept:?}");
    };
    assert!(refusal.contains(pin.as_str()), "{refusal}");
    assert_eq!(
        tree_of(&repo, commit),
        tree,
        "the pin holds the captured tree"
    );
}

fn canonical_common_git_dir_of(repo: &Path) -> PathBuf {
    let common = git_in(
        repo,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    );
    fs::canonicalize(common.trim()).expect("the canonical common git dir")
}

struct OnceContended {
    done: std::sync::Arc<std::sync::atomic::AtomicBool>,
    writer: std::thread::JoinHandle<Option<usize>>,
}

impl OnceContended {
    fn spawn(repo: &Path, finish: impl FnOnce() + Send + 'static) -> Self {
        let common = canonical_common_git_dir_of(repo);
        let before = crate::workspace_manager::contended_attempts(&common);
        let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stop = std::sync::Arc::clone(&done);
        let writer = std::thread::spawn(move || {
            let watchdog = std::time::Instant::now() + Duration::from_secs(60);
            while std::time::Instant::now() < watchdog
                && !stop.load(std::sync::atomic::Ordering::SeqCst)
            {
                let now = crate::workspace_manager::contended_attempts(&common);
                if now > before {
                    finish();
                    return Some(now - before);
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            None
        });
        Self { done, writer }
    }

    fn join(self) -> Option<usize> {
        self.done.store(true, std::sync::atomic::Ordering::SeqCst);
        self.writer.join().expect("the writer thread")
    }
}

#[test]
fn a_legacy_run_completes_past_a_tear_its_writer_finishes() {
    let (_tree, repo) = temp_engine_repo("tear-legacy");
    one_gated_task(&repo);
    let writer = OnceContended::spawn(&repo, {
        let repo = repo.clone();
        move || repair_the_torn_registration(&repo)
    });
    let run = run_with(
        &refusal_options(&repo, tear_after_capture),
        &fake(Effect::EditFile),
    );
    let contended = writer.join();
    let report = run.expect("the run is attempted past a tear its writer finished");
    assert!(committed(&report, "t1"), "{report:?}");
    assert!(
        contended.is_some(),
        "the snapshot add failed on the tear at least once"
    );
    assert_eq!(
        kept_pins(&repo),
        Vec::<(String, String)>::new(),
        "nothing is kept"
    );
    assert_eq!(status_of(&repo), "", "and nothing is left behind");
}

#[test]
fn a_legacy_run_completes_past_a_topology_slot_its_writer_finishes() {
    let (_tree, repo) = temp_engine_repo("tear-topo");
    one_gated_task(&repo);
    let (_manager, admin) = a_torn_topology_slot(&repo);
    let writer = OnceContended::spawn(&repo, move || finish_the_topology_registration(&admin));
    let run = run_with(
        &refusal_options(&repo, record_the_capture),
        &fake(Effect::EditFile),
    );
    let contended = writer.join();
    let report = run.expect("the run is attempted past the topology writer");
    assert!(committed(&report, "t1"), "{report:?}");
    assert!(
        contended.is_some(),
        "the snapshot add failed on the topology slot's registration at least once"
    );
    assert_eq!(
        kept_pins(&repo),
        Vec::<(String, String)>::new(),
        "nothing is kept"
    );
}

fn restore_at_the_parent_under(tag: &str, after_capture: super::options::AfterCandidateCapture) {
    let (_tree, repo) = shaped_repo(tag);
    let run = run_with(&refusal_options(&repo, after_capture), &shaped_worker());
    registry_refusal_text(&run, tag);
    let (_, _, tree) = captured(&repo);
    let kept = kept_pins(&repo);
    let [(pin, _)] = kept.as_slice() else {
        panic!("{tag}: one kept pin: {kept:?}");
    };
    assert_eq!(
        tree_of(&repo, pin),
        tree,
        "{tag}: the pin holds the captured tree"
    );

    repair_the_torn_registration(&repo);
    let run_id = rundir::latest_run(&repo).expect("the run started");
    let report =
        resume_the_run(&repo, &run_id, &fake(Effect::NoEdit)).expect("the resume after the repair");
    assert!(
        !repo.join("new.txt").exists(),
        "{tag}: the resume discarded the checkout's copy"
    );
    let warning = kept_pin_warning(&report).expect("the resume names the kept pin");
    let command = follow_the_restore(&repo, warning, pin);
    assert_eq!(
        differences_from_the_pin(&repo, pin),
        Vec::<String>::new(),
        "{tag}: `{command}` restores the pin as the repository records it"
    );
}

#[test]
fn the_kept_pin_restore_writes_the_recorded_blob_under_a_blob_replacement() {
    restore_at_the_parent_under("kept-blob", replace_the_new_blob_and_tear_after_capture);
}

#[test]
fn the_kept_pin_restore_writes_the_recorded_tree_under_a_tree_replacement() {
    restore_at_the_parent_under(
        "kept-tree",
        replace_the_captured_tree_and_tear_after_capture,
    );
}

fn pick_on_a_later_head_under(
    tag: &str,
    after_capture: super::options::AfterCandidateCapture,
    graft: bool,
) {
    let (_tree, repo) = shaped_repo(tag);
    let run = run_with(&refusal_options(&repo, after_capture), &shaped_worker());
    registry_refusal_text(&run, tag);
    let kept = kept_pins(&repo);
    let [(pin, commit)] = kept.as_slice() else {
        panic!("{tag}: one kept pin: {kept:?}");
    };

    repair_the_torn_registration(&repo);
    let run_id = rundir::latest_run(&repo).expect("the run started");
    let report = resume_the_run(&repo, &run_id, &fake(Effect::EditFile))
        .expect("the resume after the repair");
    assert!(committed(&report, "t1"), "{tag}: {report:?}");
    if graft {
        let root = git_in(&repo, &["rev-list", "--max-parents=0", "HEAD"])
            .trim()
            .to_owned();
        git_in(&repo, &["replace", "--graft", commit, &root]);
    }
    let warning = kept_pin_warning(&report).expect("the resume names the kept pin");
    let command = follow_the_pick(&repo, warning, pin);
    let mut staged: Vec<String> = recorded(&repo, &["diff", "--cached", "--name-status", "HEAD"])
        .lines()
        .map(str::to_owned)
        .collect();
    staged.sort_unstable();
    assert_eq!(
        staged,
        ["A\tnew.txt", "D\tdeleted.txt", "M\ttracked.txt"],
        "{tag}: `{command}` stages exactly the kept change"
    );
    for path in ["tracked.txt", "new.txt"] {
        assert_eq!(
            fs::read_to_string(repo.join(path)).expect("a picked file"),
            recorded(&repo, &["cat-file", "blob", &format!("{commit}:{path}")]),
            "{tag}: {path} holds what the pin records"
        );
    }
    assert!(
        !repo.join("deleted.txt").exists(),
        "{tag}: the deletion applies"
    );
    assert_eq!(
        fs::read_to_string(repo.join("agent-output.txt")).expect("the later commit's file"),
        recorded(&repo, &["cat-file", "blob", "HEAD:agent-output.txt"]),
        "{tag}: the later commit's file stays"
    );
}

#[test]
fn the_kept_pin_pick_applies_the_recorded_change_under_a_tree_replacement() {
    pick_on_a_later_head_under(
        "pick-tree",
        replace_the_captured_tree_and_tear_after_capture,
        false,
    );
}

#[test]
fn the_kept_pin_pick_takes_the_recorded_parent_under_a_graft() {
    pick_on_a_later_head_under("pick-graft", tear_after_capture, true);
}

struct TearingWorker {
    inner: FakeAdapter,
}

impl AgentAdapter for TearingWorker {
    fn id(&self) -> &'static str {
        self.inner.id()
    }

    fn probe(&self, runner: &dyn crate::runner::Runner) -> Result<Caps, UpstrokeError> {
        self.inner.probe(runner)
    }

    fn build(&self, run: &TaskRun) -> Result<CommandSpec, UpstrokeError> {
        if run.profile.permissions != PermissionMode::ReadOnly {
            plant_a_torn_registration_in(&run.workspace).map_err(after_capture_failed)?;
        }
        self.inner.build(run)
    }

    fn parse(&self, out: &ProcessOutput) -> Result<Outcome, UpstrokeError> {
        self.inner.parse(out)
    }

    fn materialize_permissions(
        &self,
        profile: &WorkerProfile,
        gate_cmds: &[String],
        dir: &Path,
        stem: &str,
    ) -> Result<Option<PathBuf>, UpstrokeError> {
        self.inner
            .materialize_permissions(profile, gate_cmds, dir, stem)
    }
}

#[test]
fn every_kept_pin_is_named_after_a_second_refusal_on_the_next_resume() {
    let (_tree, repo) = temp_engine_repo("kept-twice");
    one_gated_task(&repo);
    let first = run_with(
        &refusal_options(&repo, tear_after_capture),
        &fake(Effect::EditFile),
    );
    registry_refusal_text(&first, "the first refusal");
    assert_eq!(kept_pins(&repo).len(), 1);

    repair_the_torn_registration(&repo);
    let run_id = rundir::latest_run(&repo).expect("the run started");
    let tearing = OneAdapter {
        adapter: TearingWorker {
            inner: FakeAdapter::new(vec![Effect::EditFile], vec![ReviewBehavior::Pass]),
        },
    };
    let second = resume_the_run(&repo, &run_id, &tearing);
    registry_refusal_text(&second, "the resumed attempt's own refusal");
    let both = kept_pins(&repo);
    assert_eq!(
        both.len(),
        2,
        "each refused attempt keeps its own pin: {both:?}"
    );

    repair_the_torn_registration(&repo);
    let report = resume_the_run(&repo, &run_id, &fake(Effect::EditFile))
        .expect("the resume after the second repair");
    assert!(committed(&report, "t1"), "{report:?}");
    let warning = kept_pin_warning(&report).expect("the resume names the kept pins");
    for (pin, _) in &both {
        assert!(
            warning.contains(pin.as_str()),
            "every surviving pin is named; {pin} is not: {warning}"
        );
    }
    assert_eq!(kept_pins(&repo), both, "no resume removes a kept pin");
}

fn named_after_a_failed_resume(tag: &str, retire: bool) {
    let (_tree, repo) = temp_engine_repo(tag);
    one_gated_task(&repo);
    let run = run_with(
        &refusal_options(&repo, tear_after_capture),
        &fake(Effect::EditFile),
    );
    registry_refusal_text(&run, tag);
    let kept = kept_pins(&repo);
    let [(pin, _)] = kept.as_slice() else {
        panic!("{tag}: one kept pin: {kept:?}");
    };

    repair_the_torn_registration(&repo);
    let run_id = rundir::latest_run(&repo).expect("the run started");
    let failed = resume_the_run(&repo, &run_id, &fake(Effect::SpawnError));
    assert!(
        failed.is_err(),
        "{tag}: the resume fails after settling the interrupted attempt"
    );
    let mut warnings = Vec::new();
    let events = events::read_all(
        &rundir::public_dir(&repo, &run_id).join("events.jsonl"),
        &mut warnings,
    )
    .expect("the run's log");
    let mut state = RunState::new(vec!["t1".to_owned()]);
    for event in &events {
        state.apply(event);
    }
    assert!(
        state
            .interrupted_attempts()
            .iter()
            .all(|interrupted| interrupted.flight.attempt != 1),
        "{tag}: attempt 1 is no longer in flight after the failed resume"
    );
    if retire {
        git_in(&repo, &["update-ref", "-d", pin]);
    }

    let report = resume_the_run(&repo, &run_id, &fake(Effect::EditFile))
        .expect("the resume after the failed one");
    let named = report
        .warnings
        .iter()
        .any(|warning| warning.contains(pin.as_str()));
    if retire {
        assert_eq!(kept_pins(&repo), Vec::<(String, String)>::new());
        assert!(
            !named,
            "{tag}: a removed pin is named no more: {:?}",
            report.warnings
        );
    } else {
        assert_eq!(
            kept_pins(&repo),
            kept,
            "{tag}: the pin survives both resumes"
        );
        assert!(
            named,
            "{tag}: the pin is named again: {:?}",
            report.warnings
        );
    }
}

#[test]
fn a_kept_pin_is_named_after_a_resume_that_failed() {
    named_after_a_failed_resume("kept-failed", false);
}

#[test]
fn a_removed_kept_pin_is_named_no_more() {
    named_after_a_failed_resume("kept-retired", true);
}

const ENGINE_CALL_BOUND: Duration = Duration::from_secs(300);

fn bounded<T: Send + 'static>(what: &str, call: impl FnOnce() -> T + Send + 'static) -> T {
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(call());
    });
    match receiver.recv_timeout(ENGINE_CALL_BOUND) {
        Ok(value) => value,
        Err(error) => panic!("{what}: no answer within {ENGINE_CALL_BOUND:?} ({error})"),
    }
}

fn bounded_run(
    what: &str,
    repo: &Path,
    after_capture: Option<super::options::AfterCandidateCapture>,
    adapters: impl AdapterSource + Send + 'static,
) -> Result<RunReport, UpstrokeError> {
    let mut opts = options(repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    opts.after_candidate_capture = after_capture;
    bounded(what, move || run_with(&opts, &adapters))
}

fn bounded_resume(
    what: &str,
    repo: &Path,
    adapters: impl AdapterSource + Send + 'static,
) -> Result<RunReport, UpstrokeError> {
    let repo = repo.to_path_buf();
    let run_id = rundir::latest_run(&repo).expect("a run to resume");
    bounded(what, move || resume_the_run(&repo, &run_id, &adapters))
}

fn bounded_child(
    what: &str,
    mut command: Command,
    log: &Path,
) -> (std::process::ExitStatus, String) {
    let out = fs::File::create(log).expect("the child's log");
    let err = out.try_clone().expect("the child's log, for its stderr");
    let mut child = command
        .stdout(out)
        .stderr(err)
        .spawn()
        .expect("spawn the child");
    let deadline = std::time::Instant::now() + ENGINE_CALL_BOUND;
    loop {
        if let Some(status) = child.try_wait().expect("poll the child") {
            return (status, fs::read_to_string(log).unwrap_or_default());
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "{what}: the child did not end within {ENGINE_CALL_BOUND:?}: {}",
                fs::read_to_string(log).unwrap_or_default()
            );
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn this_test_binary(fixture: &str) -> Command {
    let mut command = Command::new(std::env::current_exe().expect("this test binary"));
    command.args([
        "--exact",
        &format!("engine::tests::{fixture}"),
        "--ignored",
        "--test-threads",
        "1",
    ]);
    command
}

fn kept_pin_of(repo: &Path, attempt: u32) -> String {
    let run_id = rundir::latest_run(repo).expect("a run started");
    format!(
        "{}{}",
        super::coordinator::prepared_pin_ref(&run_id, 0, attempt),
        crate::workspace::KEPT_PIN_SUFFIX
    )
}

fn pin_target(repo: &Path, pin: &str) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "--verify", "--quiet", pin])
        .output()
        .expect("run git");
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

fn blob_at(repo: &Path, revision: &str, path: &str) -> Option<Vec<u8>> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["--no-replace-objects", "cat-file", "blob"])
        .arg(format!("{revision}:{path}"))
        .output()
        .expect("run git");
    out.status.success().then_some(out.stdout)
}

fn kept_copies(repo: &Path) -> Vec<PathBuf> {
    let run_id = rundir::latest_run(repo).expect("a run started");
    let mut copies: Vec<PathBuf> = fs::read_dir(paths_of(repo, &run_id).public)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| {
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.starts_with("kept-") && name.ends_with(".bundle"))
                })
                .collect()
        })
        .unwrap_or_default();
    copies.sort();
    copies
}

fn restore_the_pin_from(repo: &Path, copy: &Path, pin: &str) {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("fetch")
        .arg(copy)
        .arg(format!("{pin}:{pin}"))
        .output()
        .expect("run git fetch");
    assert!(
        out.status.success(),
        "git fetch {} {pin}:{pin}: {}",
        copy.display(),
        String::from_utf8_lossy(&out.stderr)
    );
}

fn forget_every_unreferenced_object(repo: &Path) {
    git_in(repo, &["reflog", "expire", "--expire=now", "--all"]);
    git_in(repo, &["gc", "-q", "--prune=now"]);
}

fn warning_with<'a>(report: &'a RunReport, needle: &str) -> Option<&'a String> {
    report
        .warnings
        .iter()
        .find(|warning| warning.contains(needle))
}

fn resume_refusal(result: &Result<RunReport, UpstrokeError>, what: &str) -> String {
    match result {
        Err(UpstrokeError::Resume { message, .. }) => message.clone(),
        Err(other) => panic!("{what}: a resume refusal, never {other:?}"),
        Ok(report) => panic!("{what}: a resume refusal, not {report:?}"),
    }
}

fn file_bytes(repo: &Path, path: &str) -> Option<Vec<u8>> {
    fs::read(repo.join(path)).ok()
}

fn rd1_repo(tag: &str) -> (ScratchTree, PathBuf) {
    let (tree, repo) = temp_engine_repo(tag);
    git_in(&repo, &["config", "core.autocrlf", "false"]);
    for (name, content) in [
        ("tracked.txt", "base\n"),
        ("deleted.txt", "to be deleted\n"),
        (".gitignore", "ignored.flag\n"),
        ("sub/t.txt", "sub base\n"),
    ] {
        let path = repo.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("a parent");
        }
        fs::write(path, content).expect("a tracked file");
    }
    one_gated_task(&repo);
    (tree, repo)
}

type Shape = fn(&Path) -> std::io::Result<()>;

struct ShapingWorker {
    inner: FakeAdapter,
    shape: Shape,
    then_exit: bool,
}

impl AgentAdapter for ShapingWorker {
    fn id(&self) -> &'static str {
        self.inner.id()
    }

    fn probe(&self, runner: &dyn crate::runner::Runner) -> Result<Caps, UpstrokeError> {
        self.inner.probe(runner)
    }

    fn build(&self, run: &TaskRun) -> Result<CommandSpec, UpstrokeError> {
        if run.profile.permissions != PermissionMode::ReadOnly {
            (self.shape)(&run.workspace).map_err(after_capture_failed)?;
            if self.then_exit {
                std::process::exit(CRASH_EXIT_CODE);
            }
        }
        self.inner.build(run)
    }

    fn parse(&self, out: &ProcessOutput) -> Result<Outcome, UpstrokeError> {
        self.inner.parse(out)
    }

    fn materialize_permissions(
        &self,
        profile: &WorkerProfile,
        gate_cmds: &[String],
        dir: &Path,
        stem: &str,
    ) -> Result<Option<PathBuf>, UpstrokeError> {
        self.inner
            .materialize_permissions(profile, gate_cmds, dir, stem)
    }
}

fn shaping(shape: Shape) -> OneAdapter<ShapingWorker> {
    OneAdapter {
        adapter: ShapingWorker {
            inner: FakeAdapter::new(vec![Effect::NoEdit], vec![ReviewBehavior::Pass]),
            shape,
            then_exit: false,
        },
    }
}

fn write_into(root: &Path, name: &str, content: &[u8]) -> std::io::Result<()> {
    let path = root.join(name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)
}

fn git_from_a_shape(root: &Path, args: &[&str]) -> std::io::Result<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()?;
    if !out.status.success() {
        return Err(std::io::Error::other(format!(
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

fn paid_output(root: &Path) -> std::io::Result<()> {
    write_into(root, "agent-output.txt", b"paid output\n")
}

fn paid_output_in_a_new_directory(root: &Path) -> std::io::Result<()> {
    write_into(root, "agent-output.txt", b"paid output\n")?;
    write_into(root, "newdir/inner.txt", b"paid, in a new directory\n")
}

fn paid_edit_to_a_tracked_file(root: &Path) -> std::io::Result<()> {
    write_into(root, "agent-output.txt", b"paid output\n")?;
    write_into(root, "sub/t.txt", b"paid edit\n")
}

#[cfg(target_os = "linux")]
fn non_utf8_shape() -> Option<(&'static str, Shape)> {
    Some(("x17", a_name_that_is_not_utf8 as Shape))
}

#[cfg(not(target_os = "linux"))]
fn non_utf8_shape() -> Option<(&'static str, Shape)> {
    None
}

fn shape_named(name: &str) -> Option<Shape> {
    let shapes: [(&str, Shape); 13] = [
        ("paid-output", paid_output),
        ("x4", every_kind_of_change),
        ("rd1-4-v1", ignore_rule_of_a_new_directory),
        ("rd1-4-v3", tracked_ignore_rule_edited),
        ("rd1-5", interrupted_text_write),
        ("x10", nested_repository_and_unreadable_file),
        ("x18", output_and_a_per_worktree_ignored_file),
        ("x20", nested_repository_with_a_commit),
        ("rd2-2", ignored_directory_where_head_has_a_file),
        ("rd2-2r", ignored_file_where_head_has_a_directory),
        ("rd2-2u", ignored_directory_with_an_unreadable_file),
        ("rd2-2n", ignored_directory_with_a_nested_repository),
        ("rd2-2b", tracked_edit_and_an_ignored_deletion),
    ];
    shapes
        .into_iter()
        .chain([("crlf", crlf_output as Shape)])
        .chain(non_utf8_shape())
        .find(|(known, _)| *known == name)
        .map(|(_, shape)| shape)
}

fn die_inside_an_attempt(repo: &Path, shape: &str) {
    let mut command = this_test_binary("kept_shape_child_dies_inside_an_attempt");
    command
        .env("UPSTROKE_KEPT_CHILD_REPO", repo)
        .env("UPSTROKE_KEPT_CHILD_SHAPE", shape);
    let (status, log) = bounded_child(
        "the run that dies inside its attempt",
        command,
        &repo.with_file_name("dies-inside.log"),
    );
    assert_eq!(
        status.code(),
        Some(CRASH_EXIT_CODE),
        "the child dies inside the attempt, after its worker wrote: {log}"
    );
}

#[test]
#[ignore = "spawned by the R-D1 tests that kill a run inside its attempt"]
fn kept_shape_child_dies_inside_an_attempt() {
    let (Ok(repo), Ok(shape)) = (
        std::env::var("UPSTROKE_KEPT_CHILD_REPO"),
        std::env::var("UPSTROKE_KEPT_CHILD_SHAPE"),
    ) else {
        return;
    };
    let repo = PathBuf::from(repo);
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = OneAdapter {
        adapter: ShapingWorker {
            inner: FakeAdapter::new(vec![Effect::NoEdit], vec![ReviewBehavior::Pass]),
            shape: shape_named(&shape).expect("a shape the parent names"),
            then_exit: true,
        },
    };
    let _ = run_with(&opts, &source);
    std::process::exit(0);
}

fn resume_and_die_inside_the_attempt(repo: &Path) {
    let mut command = this_test_binary("kept_resume_child_dies_inside_the_attempt");
    command.env("UPSTROKE_KEPT_RESUME_REPO", repo);
    let (status, log) = bounded_child(
        "the resume that dies inside its attempt",
        command,
        &repo.with_file_name("resume-dies.log"),
    );
    assert_eq!(
        status.code(),
        Some(CRASH_EXIT_CODE),
        "the resume reached its attempt, past the guarded discard: {log}"
    );
}

#[test]
#[ignore = "spawned by the R-D1 tests that run two resumes"]
fn kept_resume_child_dies_inside_the_attempt() {
    let Ok(repo) = std::env::var("UPSTROKE_KEPT_RESUME_REPO") else {
        return;
    };
    let repo = PathBuf::from(repo);
    let run_id = rundir::latest_run(&repo).expect("a run to resume");
    let source = source(vec![Effect::Exit], vec![ReviewBehavior::Pass]);
    let _ = resume_the_run(&repo, &run_id, &source);
    std::process::exit(0);
}

struct ChildEngine {
    report: PathBuf,
    log: PathBuf,
}

fn child_engine(repo: &Path, label: &str) -> ChildEngine {
    ChildEngine {
        report: repo.with_file_name(format!("{label}.report")),
        log: repo.with_file_name(format!("{label}.log")),
    }
}

impl ChildEngine {
    fn run(
        &self,
        what: &str,
        repo: &Path,
        fixture: &str,
        env: &[(&str, &std::ffi::OsStr)],
    ) -> String {
        let _ = fs::remove_file(&self.report);
        let mut command = this_test_binary(fixture);
        command
            .env("UPSTROKE_KEPT_ENGINE_REPO", repo)
            .env("UPSTROKE_KEPT_ENGINE_REPORT", &self.report);
        for (name, value) in env {
            command.env(name, value);
        }
        let (status, log) = bounded_child(what, command, &self.log);
        assert!(
            status.success(),
            "{what}: the child ended {status:?}: {log}"
        );
        fs::read_to_string(&self.report).unwrap_or_else(|_| panic!("{what}: no report: {log}"))
    }
}

fn report_of(result: &Result<RunReport, UpstrokeError>) -> String {
    match result {
        Ok(report) => format!("ok\n{}", report.warnings.join("\n")),
        Err(error) => format!("err\n{error}\n{error:?}"),
    }
}

#[test]
#[ignore = "spawned by the R-D1 tests that resume in a child"]
fn kept_engine_child_resumes() {
    let (Ok(repo), Ok(report)) = (
        std::env::var("UPSTROKE_KEPT_ENGINE_REPO"),
        std::env::var("UPSTROKE_KEPT_ENGINE_REPORT"),
    ) else {
        return;
    };
    let repo = PathBuf::from(repo);
    let run_id = rundir::latest_run(&repo).expect("a run to resume");
    let resumed = resume_the_run(&repo, &run_id, &fake(Effect::EditFile));
    fs::write(report, report_of(&resumed)).expect("the child's report");
}

#[test]
#[ignore = "spawned by the R-D1 tests that run in a child"]
fn kept_engine_child_runs_and_fails_after_capture() {
    let (Ok(repo), Ok(report)) = (
        std::env::var("UPSTROKE_KEPT_ENGINE_REPO"),
        std::env::var("UPSTROKE_KEPT_ENGINE_REPORT"),
    ) else {
        return;
    };
    let repo = PathBuf::from(repo);
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    opts.after_candidate_capture = Some(fail_after_capture);
    let ran = run_with(&opts, &fake(Effect::EditFile));
    fs::write(report, report_of(&ran)).expect("the child's report");
}

#[test]
#[ignore = "spawned by the R-D1 tests that run in a child"]
fn kept_engine_child_runs() {
    let (Ok(repo), Ok(report)) = (
        std::env::var("UPSTROKE_KEPT_ENGINE_REPO"),
        std::env::var("UPSTROKE_KEPT_ENGINE_REPORT"),
    ) else {
        return;
    };
    let repo = PathBuf::from(repo);
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let ran = run_with(&opts, &fake(Effect::EditFile));
    fs::write(report, report_of(&ran)).expect("the child's report");
}

#[test]
#[ignore = "spawned by the R-D1 tests that run in a child"]
fn kept_engine_child_runs_and_tears_after_capture() {
    let (Ok(repo), Ok(report)) = (
        std::env::var("UPSTROKE_KEPT_ENGINE_REPO"),
        std::env::var("UPSTROKE_KEPT_ENGINE_REPORT"),
    ) else {
        return;
    };
    let repo = PathBuf::from(repo);
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    opts.after_candidate_capture = Some(tear_after_capture);
    let ran = run_with(&opts, &fake(Effect::EditFile));
    fs::write(report, report_of(&ran)).expect("the child's report");
}

#[cfg(unix)]
struct GitShim {
    dir: PathBuf,
    log: PathBuf,
}

#[cfg(unix)]
struct ShimRule<'a> {
    subcommand: &'a str,
    holding: &'a str,
    after: bool,
    action: String,
}

#[cfg(unix)]
fn real_git() -> String {
    String::from_utf8(
        Command::new("sh")
            .args(["-c", "command -v git"])
            .output()
            .expect("find git")
            .stdout,
    )
    .expect("a UTF-8 path to git")
    .trim()
    .to_owned()
}

#[cfg(unix)]
fn git_shim(beside: &Path, label: &str, rules: &[ShimRule<'_>]) -> GitShim {
    use std::os::unix::fs::PermissionsExt;
    let dir = beside.with_file_name(format!("{label}-shim"));
    fs::create_dir_all(&dir).expect("the shim's directory");
    let log = dir.join("calls.log");
    let real = real_git();
    let mut script = format!(
        "#!/bin/sh\nreal='{real}'\nforeign() {{ env -u GIT_INDEX_FILE -u GIT_NO_REPLACE_OBJECTS \"$real\" \"$@\"; }}\nsub=''\nskip=''\nfor a in \"$@\"; do\n  if [ -n \"$skip\" ]; then skip=''; continue; fi\n  case \"$a\" in\n    -C|-c) skip=1 ;;\n    -*) ;;\n    *) sub=\"$a\"; break ;;\n  esac\ndone\nprintf '%s\\n' \"$*\" >> '{}'\n",
        log.display()
    );
    let mut afters = String::new();
    for (index, rule) in rules.iter().enumerate() {
        let marker = dir.join(format!("rule-{index}.fired"));
        let guard = format!(
            "if [ \"$sub\" = '{}' ] && case \" $* \" in *'{}'*) true;; *) false;; esac && [ ! -e '{}' ]; then\n  : > '{}'\n{}\nfi\n",
            rule.subcommand,
            rule.holding,
            marker.display(),
            marker.display(),
            rule.action
        );
        if rule.after {
            afters.push_str(&guard);
        } else {
            script.push_str(&guard);
        }
    }
    if afters.is_empty() {
        script.push_str("exec \"$real\" \"$@\"\n");
    } else {
        script.push_str("\"$real\" \"$@\"\nstatus=$?\n");
        script.push_str(&afters);
        script.push_str("exit $status\n");
    }
    let shim = dir.join("git");
    fs::write(&shim, script).expect("the shim");
    fs::set_permissions(&shim, fs::Permissions::from_mode(0o755)).expect("an executable shim");
    GitShim { dir, log }
}

#[cfg(unix)]
impl GitShim {
    fn path(&self) -> std::ffi::OsString {
        let mut path = self.dir.clone().into_os_string();
        path.push(":");
        path.push(std::env::var_os("PATH").expect("a PATH"));
        path
    }

    fn fired(&self, rule: usize) -> bool {
        self.dir.join(format!("rule-{rule}.fired")).exists()
    }

    fn calls(&self) -> String {
        fs::read_to_string(&self.log).unwrap_or_default()
    }
}

fn ignore_rule_of_a_new_directory(root: &Path) -> std::io::Result<()> {
    write_into(root, "newdir/.gitignore", b"paid-output.txt\n")?;
    write_into(
        root,
        "newdir/paid-output.txt",
        b"paid, ignored by its own rule\n",
    )
}

fn tracked_ignore_rule_edited(root: &Path) -> std::io::Result<()> {
    write_into(root, ".gitignore", b"ignored.flag\npaid-new.txt\n")?;
    write_into(root, "paid-new.txt", b"paid, covered by the edited rule\n")
}

fn interrupted_text_write(root: &Path) -> std::io::Result<()> {
    write_into(root, "tracked.txt", b"paid text cut mid-character \xe2\x82")
}

fn a_nested_repository_at(root: &Path, at: &str, commit: bool) -> std::io::Result<()> {
    let nested = root.join(at);
    fs::create_dir_all(&nested)?;
    git_from_a_shape(&nested, &["init", "-q"])?;
    write_into(&nested, "n.txt", b"nested\n")?;
    if commit {
        git_from_a_shape(&nested, &["add", "-A"])?;
        git_from_a_shape(
            &nested,
            &[
                "-c",
                "user.name=N",
                "-c",
                "user.email=n@example.invalid",
                "commit",
                "-q",
                "-m",
                "n",
            ],
        )?;
    }
    Ok(())
}

fn unreadable_file_at(root: &Path, at: &str) -> std::io::Result<()> {
    write_into(root, at, b"paid, unreadable\n")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(root.join(at), fs::Permissions::from_mode(0o000))?;
    }
    Ok(())
}

fn nested_repository_and_unreadable_file(root: &Path) -> std::io::Result<()> {
    paid_output(root)?;
    a_nested_repository_at(root, "nested", false)?;
    unreadable_file_at(root, "locked.txt")
}

fn output_and_a_per_worktree_ignored_file(root: &Path) -> std::io::Result<()> {
    paid_output(root)?;
    write_into(root, "cache.bin", b"ignored by this worktree's own rules\n")
}

fn nested_repository_with_a_commit(root: &Path) -> std::io::Result<()> {
    paid_output(root)?;
    a_nested_repository_at(root, "nested", true)
}

fn ignored_directory_where_head_has_a_file(root: &Path) -> std::io::Result<()> {
    fs::remove_file(root.join("tracked.txt"))?;
    write_into(root, ".gitignore", b"ignored.flag\ntracked.txt/\n")?;
    write_into(root, "tracked.txt/paid.txt", b"paid and ignored\n")
}

fn ignored_file_where_head_has_a_directory(root: &Path) -> std::io::Result<()> {
    fs::remove_dir_all(root.join("sub"))?;
    write_into(root, ".gitignore", b"ignored.flag\nsub\n")?;
    write_into(root, "sub", b"paid, a file where a directory was\n")
}

fn ignored_directory_with_an_unreadable_file(root: &Path) -> std::io::Result<()> {
    ignored_directory_where_head_has_a_file(root)?;
    unreadable_file_at(root, "tracked.txt/locked.txt")
}

fn ignored_directory_with_a_nested_repository(root: &Path) -> std::io::Result<()> {
    ignored_directory_where_head_has_a_file(root)?;
    a_nested_repository_at(root, "tracked.txt/nested", true)
}

fn tracked_edit_and_an_ignored_deletion(root: &Path) -> std::io::Result<()> {
    write_into(root, "tracked.txt", b"paid tracked\n")?;
    write_into(root, ".gitignore", b"ignored.flag\ndeleted.txt\n")?;
    fs::remove_file(root.join("deleted.txt"))
}

fn crlf_output(root: &Path) -> std::io::Result<()> {
    write_into(root, "tracked.txt", b"paid output\r\n")
}

fn move_the_branch_then_write(root: &Path) -> std::io::Result<()> {
    let moved = git_from_a_shape(
        root,
        &[
            "commit-tree",
            "HEAD^{tree}",
            "-p",
            "HEAD",
            "-m",
            "moved before capture",
        ],
    )?;
    let branch = git_from_a_shape(root, &["symbolic-ref", "HEAD"])?;
    git_from_a_shape(root, &["update-ref", &branch, &moved])?;
    paid_output(root)
}

fn the_parent_of(root: &Path, commit: &str) -> Result<String, UpstrokeError> {
    git_after_capture(root, &["rev-parse", &format!("{commit}^")])
}

fn move_back_and_tear_after_capture(
    workspace: &Workspace,
    candidate: &crate::workspace::CapturedCandidate,
) -> Result<(), UpstrokeError> {
    record_the_capture(workspace, candidate)?;
    let root = workspace.root();
    let recorded = the_parent_of(root, &candidate.parent_oid)?;
    git_after_capture(root, &["update-ref", &candidate.branch_ref, &recorded])?;
    plant_a_torn_registration_in(root)
        .map(|_| ())
        .map_err(after_capture_failed)
}

fn reset_hard_back_and_tear_after_capture(
    workspace: &Workspace,
    candidate: &crate::workspace::CapturedCandidate,
) -> Result<(), UpstrokeError> {
    record_the_capture(workspace, candidate)?;
    let root = workspace.root();
    let recorded = the_parent_of(root, &candidate.parent_oid)?;
    git_after_capture(root, &["reset", "-q", "--hard", &recorded])?;
    plant_a_torn_registration_in(root)
        .map(|_| ())
        .map_err(after_capture_failed)
}

fn detach_and_tear_after_capture(
    workspace: &Workspace,
    candidate: &crate::workspace::CapturedCandidate,
) -> Result<(), UpstrokeError> {
    record_the_capture(workspace, candidate)?;
    git_after_capture(workspace.root(), &["checkout", "-q", "--detach"])?;
    tear_after_capture(workspace, candidate)
}

fn switch_away_and_tear_after_capture(
    workspace: &Workspace,
    candidate: &crate::workspace::CapturedCandidate,
) -> Result<(), UpstrokeError> {
    record_the_capture(workspace, candidate)?;
    git_after_capture(workspace.root(), &["switch", "-q", "-c", "other"])?;
    tear_after_capture(workspace, candidate)
}

fn delete_the_branch_and_tear_after_capture(
    workspace: &Workspace,
    candidate: &crate::workspace::CapturedCandidate,
) -> Result<(), UpstrokeError> {
    record_the_capture(workspace, candidate)?;
    git_after_capture(
        workspace.root(),
        &["update-ref", "-d", &candidate.branch_ref],
    )?;
    tear_after_capture(workspace, candidate)
}

fn make_the_branch_symbolic_and_tear_after_capture(
    workspace: &Workspace,
    candidate: &crate::workspace::CapturedCandidate,
) -> Result<(), UpstrokeError> {
    record_the_capture(workspace, candidate)?;
    let root = workspace.root();
    git_after_capture(root, &["branch", "-q", "victim", &candidate.parent_oid])?;
    git_after_capture(
        root,
        &["symbolic-ref", &candidate.branch_ref, "refs/heads/victim"],
    )?;
    tear_after_capture(workspace, candidate)
}

#[cfg(unix)]
fn mode_of(path: &Path, mode: u32) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
}

#[cfg(unix)]
fn make_the_store_read_only_after_capture(
    workspace: &Workspace,
    candidate: &crate::workspace::CapturedCandidate,
) -> Result<(), UpstrokeError> {
    record_the_capture(workspace, candidate)?;
    let git = workspace.root().join(".git");
    mode_of(&git.join("refs"), 0o555).map_err(after_capture_failed)?;
    mode_of(&git, 0o555).map_err(after_capture_failed)
}

#[cfg(unix)]
fn make_the_refs_read_only_and_tear_after_capture(
    workspace: &Workspace,
    candidate: &crate::workspace::CapturedCandidate,
) -> Result<(), UpstrokeError> {
    tear_after_capture(workspace, candidate)?;
    mode_of(&workspace.root().join(".git").join("refs"), 0o555).map_err(after_capture_failed)
}

fn block_above_the_kept_pin_and_tear_after_capture(
    workspace: &Workspace,
    candidate: &crate::workspace::CapturedCandidate,
) -> Result<(), UpstrokeError> {
    record_the_capture(workspace, candidate)?;
    let root = workspace.root();
    let run_id =
        rundir::latest_run(root).ok_or_else(|| after_capture_failed("no run has started"))?;
    let above = format!("refs/upstroke/prepared/{run_id}");
    git_after_capture(root, &["update-ref", &above, &candidate.parent_oid])?;
    tear_after_capture(workspace, candidate)
}

fn kept_name_after_capture(workspace: &Workspace) -> Result<String, UpstrokeError> {
    let run_id = rundir::latest_run(workspace.root())
        .ok_or_else(|| after_capture_failed("no run has started"))?;
    Ok(format!(
        "{}{}",
        super::coordinator::prepared_pin_ref(&run_id, 0, 1),
        crate::workspace::KEPT_PIN_SUFFIX
    ))
}

fn make_the_kept_name_symbolic_and_tear_after_capture(
    workspace: &Workspace,
    candidate: &crate::workspace::CapturedCandidate,
) -> Result<(), UpstrokeError> {
    record_the_capture(workspace, candidate)?;
    let kept = kept_name_after_capture(workspace)?;
    git_after_capture(
        workspace.root(),
        &["symbolic-ref", &kept, &candidate.branch_ref],
    )?;
    tear_after_capture(workspace, candidate)
}

fn plant_a_foreign_ref_at_the_kept_name_and_tear_after_capture(
    workspace: &Workspace,
    candidate: &crate::workspace::CapturedCandidate,
) -> Result<(), UpstrokeError> {
    record_the_capture(workspace, candidate)?;
    let kept = kept_name_after_capture(workspace)?;
    git_after_capture(
        workspace.root(),
        &["update-ref", &kept, &candidate.parent_oid],
    )?;
    tear_after_capture(workspace, candidate)
}

fn exit_after_capture(
    _workspace: &Workspace,
    _candidate: &crate::workspace::CapturedCandidate,
) -> Result<(), UpstrokeError> {
    std::process::exit(CRASH_EXIT_CODE)
}

fn advance_the_branch_after_capture(
    workspace: &Workspace,
    candidate: &crate::workspace::CapturedCandidate,
) -> Result<(), UpstrokeError> {
    record_the_capture(workspace, candidate)?;
    let root = workspace.root();
    let moved = git_after_capture(
        root,
        &[
            "commit-tree",
            "HEAD^{tree}",
            "-p",
            "HEAD",
            "-m",
            "moved after capture",
        ],
    )?;
    git_after_capture(root, &["update-ref", &candidate.branch_ref, &moved]).map(|_| ())
}

fn shared_indexes_of(repo: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(repo.join(".git"))
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|entry| entry.file_name().to_str().map(str::to_owned))
                .filter(|name| name.starts_with("sharedindex."))
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

fn shared_indexes_marker(repo: &Path) -> PathBuf {
    repo.with_file_name("shared-indexes-at-the-arm.txt")
}

fn record_shared_indexes_and_fail_after_capture(
    workspace: &Workspace,
    candidate: &crate::workspace::CapturedCandidate,
) -> Result<(), UpstrokeError> {
    record_the_capture(workspace, candidate)?;
    let root = workspace.root();
    fs::write(
        shared_indexes_marker(root),
        shared_indexes_of(root).join("\n"),
    )
    .map_err(after_capture_failed)?;
    Err(after_capture_failed(
        "an attempt error that is not the registry's",
    ))
}

#[test]
#[ignore = "spawned by a_run_that_dies_after_its_capture_loses_nothing_on_resume"]
fn kept_capture_child_dies_after_its_capture() {
    let Ok(repo) = std::env::var("UPSTROKE_KEPT_CHILD_REPO") else {
        return;
    };
    let repo = PathBuf::from(repo);
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    opts.after_candidate_capture = Some(exit_after_capture);
    let _ = run_with(&opts, &fake(Effect::EditFile));
    std::process::exit(0);
}

fn the_restore_brings_the_output_back(repo: &Path, report: &RunReport, pin: &str) {
    let warning = kept_pin_warning(report).expect("the resume names the kept pin");
    assert!(warning.contains(pin), "{warning}");
    follow_the_restore(repo, warning, pin);
    let differences = differences_from_the_pin(repo, pin);
    assert!(
        differences.is_empty(),
        "the restore reproduces the pin: {differences:?}"
    );
}

#[test]
fn a_branch_moved_before_capture_and_back_keeps_the_output_through_the_first_resume() {
    for (shape, worker) in [
        ("a1", move_the_branch_then_write as Shape),
        (
            "a1t",
            move_the_branch_with_a_foreign_change_then_write as Shape,
        ),
    ] {
        let (_tree, repo) = rd1_repo(&format!("rd1-{shape}"));
        let run = bounded_run(
            shape,
            &repo,
            Some(move_back_and_tear_after_capture),
            shaping(worker),
        );
        let refusal = registry_refusal_text(&run, shape);
        assert!(refusal.contains("pinned at"), "{shape}: {refusal}");
        let (_, parent, tree) = captured(&repo);
        let kept = kept_pins(&repo);
        let [(pin, commit)] = kept.as_slice() else {
            panic!("{shape}: one kept pin: {kept:?}");
        };
        assert_eq!(
            tree_of(&repo, commit),
            tree,
            "{shape}: the pin holds the captured tree"
        );
        assert_eq!(
            git_in(&repo, &["rev-parse", &format!("{commit}^")]).trim(),
            parent,
            "{shape}: on the captured parent, the moved commit"
        );

        repair_the_torn_registration(&repo);
        let report = bounded_resume(shape, &repo, fake(Effect::EditFile))
            .unwrap_or_else(|error| panic!("{shape}: the first resume: {error:?}"));
        assert!(committed(&report, "t1"), "{shape}: {report:?}");
        assert!(
            warning_with(&report, "discarded").is_some_and(|w| w.contains("agent-output.txt")),
            "{shape}: the first resume discards the leftovers: {:?}",
            report.warnings
        );
        assert!(
            kept_pin_warning(&report)
                .is_some_and(|w| w.contains("neither HEAD nor one of its ancestors")),
            "{shape}: the warning says what a pin on a moved branch's parent restores: {:?}",
            report.warnings
        );
        the_restore_brings_the_output_back(&repo, &report, pin);
    }
}

fn move_the_branch_with_a_foreign_change_then_write(root: &Path) -> std::io::Result<()> {
    let index = root.with_file_name("foreign-index");
    let foreign = |args: &[&str]| -> std::io::Result<String> {
        let out = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .env("GIT_INDEX_FILE", &index)
            .output()?;
        if !out.status.success() {
            return Err(std::io::Error::other(format!(
                "git {args:?}: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
    };
    foreign(&["read-tree", "HEAD"])?;
    let blob_file = root.with_file_name("foreign-blob.txt");
    fs::write(&blob_file, "a foreign change\n")?;
    let blob = foreign(&["hash-object", "-w", &blob_file.to_string_lossy()])?;
    foreign(&[
        "update-index",
        "--cacheinfo",
        &format!("100644,{blob},tracked.txt"),
    ])?;
    let tree = foreign(&["write-tree"])?;
    let moved = git_from_a_shape(
        root,
        &[
            "commit-tree",
            &tree,
            "-p",
            "HEAD",
            "-m",
            "moved, with a foreign change",
        ],
    )?;
    let branch = git_from_a_shape(root, &["symbolic-ref", "HEAD"])?;
    git_from_a_shape(root, &["update-ref", &branch, &moved])?;
    paid_output(root)
}

#[test]
fn a_branch_moved_before_capture_and_reset_hard_back_keeps_the_output_in_the_pin() {
    let (_tree, repo) = rd1_repo("rd1-a1h");
    let run = bounded_run(
        "the run whose branch went and was reset back",
        &repo,
        Some(reset_hard_back_and_tear_after_capture),
        shaping(move_the_branch_then_write),
    );
    let refusal = registry_refusal_text(&run, "the run whose branch was reset back");
    assert!(refusal.contains("pinned at"), "{refusal}");
    assert_eq!(status_of(&repo), "", "the reset emptied the checkout");
    let (_, _, tree) = captured(&repo);
    let kept = kept_pins(&repo);
    let [(pin, commit)] = kept.as_slice() else {
        panic!("one kept pin, the only copy: {kept:?}");
    };
    assert_eq!(
        tree_of(&repo, commit),
        tree,
        "the pin holds the captured tree"
    );

    repair_the_torn_registration(&repo);
    let report = bounded_resume("the resume", &repo, fake(Effect::EditFile)).expect("the resume");
    assert!(committed(&report, "t1"), "{report:?}");
    the_restore_brings_the_output_back(&repo, &report, pin);
}

#[test]
fn a_branch_moved_after_capture_keeps_the_output_pinned_at_its_captured_identity() {
    let (_tree, repo) = rd1_repo("rd1-a2");
    let run = bounded_run(
        "the run whose branch moved after capture",
        &repo,
        Some(move_head_and_tear_after_capture),
        fake(Effect::EditFile),
    );
    let refusal = registry_refusal_text(&run, "the run whose branch moved after capture");
    assert!(refusal.contains("pinned at"), "{refusal}");
    let (branch_ref, parent, tree) = captured(&repo);
    let moved = git_in(&repo, &["rev-parse", &branch_ref]).trim().to_owned();
    assert_ne!(moved, parent, "the branch moved");
    let kept = kept_pins(&repo);
    let [(pin, commit)] = kept.as_slice() else {
        panic!("one kept pin: {kept:?}");
    };
    assert_eq!(
        git_in(&repo, &["rev-parse", &format!("{commit}^")]).trim(),
        parent,
        "the pin's parent is the captured parent, not HEAD's moved commit"
    );
    assert_eq!(
        tree_of(&repo, commit),
        tree,
        "and its tree the captured tree"
    );

    repair_the_torn_registration(&repo);
    let before = status_of(&repo);
    let first = bounded_resume(
        "the resume on the moved branch",
        &repo,
        fake(Effect::EditFile),
    );
    let refusal = resume_refusal(&first, "the resume on the moved branch");
    assert!(
        refusal.contains(&format!("git update-ref {branch_ref} {parent}")),
        "the refusal names the command that moves the branch back: {refusal}"
    );
    assert_eq!(
        status_of(&repo),
        before,
        "the refused resume discards nothing"
    );

    git_in(&repo, &["reset", "-q", "--hard", &parent]);
    let report = bounded_resume("the resume after the reset", &repo, fake(Effect::EditFile))
        .expect("the resume after the operator's reset");
    assert!(committed(&report, "t1"), "{report:?}");
    the_restore_brings_the_output_back(&repo, &report, pin);
}

#[test]
fn a_kept_pin_is_written_whatever_head_is_when_the_snapshot_is_refused() {
    let shapes: [(&str, super::options::AfterCandidateCapture); 4] = [
        ("detached", detach_and_tear_after_capture),
        ("other-branch", switch_away_and_tear_after_capture),
        ("branch-deleted", delete_the_branch_and_tear_after_capture),
        (
            "branch-symbolic",
            make_the_branch_symbolic_and_tear_after_capture,
        ),
    ];
    for (shape, after_capture) in shapes {
        let (_tree, repo) = rd1_repo(&format!("rd1-a3-{shape}"));
        let run = bounded_run(shape, &repo, Some(after_capture), fake(Effect::EditFile));
        let refusal = registry_refusal_text(&run, shape);
        assert!(refusal.contains("pinned at"), "{shape}: {refusal}");
        let (branch_ref, parent, tree) = captured(&repo);
        let kept = kept_pins(&repo);
        let [(pin, commit)] = kept.as_slice() else {
            panic!("{shape}: one kept pin: {kept:?}");
        };
        assert_eq!(tree_of(&repo, commit), tree, "{shape}: the captured tree");
        assert_eq!(
            git_in(&repo, &["rev-parse", &format!("{commit}^")]).trim(),
            parent,
            "{shape}: on the captured parent"
        );
        let branch = branch_ref.trim_start_matches("refs/heads/");
        repair_the_torn_registration(&repo);
        match shape {
            "detached" | "other-branch" => {
                git_in(&repo, &["switch", "-q", branch]);
            }
            "branch-deleted" => {
                git_in(&repo, &["update-ref", &branch_ref, &parent]);
            }
            _ => {
                git_in(&repo, &["symbolic-ref", "--delete", &branch_ref]);
                git_in(&repo, &["update-ref", &branch_ref, &parent]);
            }
        }
        let report = bounded_resume(shape, &repo, fake(Effect::EditFile))
            .unwrap_or_else(|error| panic!("{shape}: the resume: {error:?}"));
        assert!(committed(&report, "t1"), "{shape}: {report:?}");
        assert!(
            warning_with(&report, "discarded").is_some_and(|w| w.contains("agent-output.txt")),
            "{shape}: the resume discards: {:?}",
            report.warnings
        );
        assert!(
            kept_pin_warning(&report).is_some_and(|w| w.contains(pin.as_str())),
            "{shape}: and names the pin: {:?}",
            report.warnings
        );
    }
}

#[cfg(unix)]
fn mode_bits_bind(directory: &Path) -> bool {
    let probe = directory.join("upstroke-mode-probe");
    match fs::write(&probe, "probe\n") {
        Ok(()) => {
            let _ = fs::remove_file(&probe);
            false
        }
        Err(_) => true,
    }
}

#[cfg(unix)]
#[test]
fn one_storage_fault_that_refuses_the_snapshot_and_the_pin_loses_nothing() {
    let (_tree, repo) = rd1_repo("rd1-b1");
    let run = bounded_run(
        "the run under one storage fault",
        &repo,
        Some(make_the_store_read_only_after_capture),
        fake(Effect::EditFile),
    );
    let git = repo.join(".git");
    let binds = mode_bits_bind(&git) && mode_bits_bind(&git.join("refs"));
    mode_of(&git, 0o755).expect("the fault healed");
    mode_of(&git.join("refs"), 0o755).expect("the fault healed");
    assert!(
        binds,
        "prerequisite not met: the mode bits did not bind (root, or CAP_DAC_OVERRIDE)"
    );
    let refusal = registry_refusal_text(&run, "the run under one storage fault");
    assert!(
        refusal.contains("pinning it at") && refusal.contains("failed"),
        "the pin failed too: {refusal}"
    );
    assert!(
        status_of(&repo).contains("agent-output.txt"),
        "nothing discarded the output"
    );
    assert_eq!(kept_pins(&repo), Vec::<(String, String)>::new());

    let report = bounded_resume(
        "the resume once the fault healed",
        &repo,
        fake(Effect::EditFile),
    )
    .expect("the resume");
    assert!(committed(&report, "t1"), "{report:?}");
    let (_, _, tree) = captured(&repo);
    let pin = kept_pin_of(&repo, 1);
    let commit = pin_target(&repo, &pin).expect("the resume wrote the kept pin");
    assert_eq!(
        tree_of(&repo, &commit),
        tree,
        "the pin holds the captured tree"
    );
    assert!(
        kept_pin_warning(&report).is_some_and(|w| w.contains(&pin)),
        "and names it: {:?}",
        report.warnings
    );
}

#[cfg(unix)]
#[test]
fn a_resume_refuses_while_the_ref_store_cannot_take_the_kept_pin() {
    let (_tree, repo) = rd1_repo("rd1-b2");
    let run = bounded_run(
        "the run whose ref store is read-only",
        &repo,
        Some(make_the_refs_read_only_and_tear_after_capture),
        fake(Effect::EditFile),
    );
    let refs = repo.join(".git").join("refs");
    let binds = mode_bits_bind(&refs);
    if !binds {
        mode_of(&refs, 0o755).expect("writable again");
    }
    assert!(
        binds,
        "prerequisite not met: the mode bit did not bind (root, or CAP_DAC_OVERRIDE)"
    );
    let refusal = registry_refusal_text(&run, "the run whose ref store is read-only");
    assert!(refusal.contains("failed"), "the pin failed: {refusal}");
    repair_the_torn_registration(&repo);
    let before = status_of(&repo);
    let first = bounded_resume(
        "the resume while the refs are read-only",
        &repo,
        fake(Effect::EditFile),
    );
    mode_of(&refs, 0o755).expect("the fault healed");
    let refusal = resume_refusal(&first, "the resume while the refs are read-only");
    assert!(
        refusal.contains("keeping them before the discard stopped"),
        "{refusal}"
    );
    assert_eq!(status_of(&repo), before, "nothing discarded");
    assert_eq!(
        kept_pins(&repo),
        Vec::<(String, String)>::new(),
        "and no pin"
    );

    let report = bounded_resume("the resume once healed", &repo, fake(Effect::EditFile))
        .expect("the resume once healed");
    assert!(committed(&report, "t1"), "{report:?}");
    assert!(
        pin_target(&repo, &kept_pin_of(&repo, 1)).is_some(),
        "the resume pinned the leftovers"
    );
}

#[test]
fn a_resume_writes_the_kept_pin_a_blocked_name_refused_and_refuses_while_it_is_blocked() {
    let shapes: [(&str, super::options::AfterCandidateCapture); 3] = [
        ("below", block_the_kept_pin_and_tear_after_capture),
        ("above", block_above_the_kept_pin_and_tear_after_capture),
        (
            "symbolic",
            make_the_kept_name_symbolic_and_tear_after_capture,
        ),
    ];
    for (shape, after_capture) in shapes {
        let (_tree, repo) = rd1_repo(&format!("rd1-c-{shape}"));
        let run = bounded_run(shape, &repo, Some(after_capture), fake(Effect::EditFile));
        let refusal = registry_refusal_text(&run, shape);
        assert!(
            refusal.contains("pinning it at") && refusal.contains("failed"),
            "{shape}: the pin failed: {refusal}"
        );
        repair_the_torn_registration(&repo);
        let before = status_of(&repo);
        let first = bounded_resume(shape, &repo, fake(Effect::EditFile));
        assert!(
            matches!(&first, Err(UpstrokeError::Resume { .. })),
            "{shape}: the resume refuses while the name is blocked: {first:?}"
        );
        assert_eq!(status_of(&repo), before, "{shape}: nothing discarded");

        let pin = kept_pin_of(&repo, 1);
        let run_id = rundir::latest_run(&repo).expect("the run");
        match shape {
            "below" => {
                git_in(&repo, &["update-ref", "-d", &format!("{pin}/blocker")]);
            }
            "above" => {
                git_in(
                    &repo,
                    &[
                        "update-ref",
                        "-d",
                        &format!("refs/upstroke/prepared/{run_id}"),
                    ],
                );
            }
            _ => {
                git_in(&repo, &["symbolic-ref", "--delete", &pin]);
            }
        }
        let report = bounded_resume(shape, &repo, fake(Effect::EditFile))
            .unwrap_or_else(|error| panic!("{shape}: the resume once unblocked: {error:?}"));
        assert!(committed(&report, "t1"), "{shape}: {report:?}");
        let (_, _, tree) = captured(&repo);
        let commit = pin_target(&repo, &pin)
            .unwrap_or_else(|| panic!("{shape}: the resume wrote the kept pin"));
        assert_eq!(tree_of(&repo, &commit), tree, "{shape}: holding the output");
    }
}

#[test]
fn a_foreign_ref_at_the_kept_name_is_neither_trusted_nor_named_as_the_output() {
    let (_tree, repo) = rd1_repo("rd1-c3");
    let run = bounded_run(
        "the run whose kept name a foreign ref holds",
        &repo,
        Some(plant_a_foreign_ref_at_the_kept_name_and_tear_after_capture),
        fake(Effect::EditFile),
    );
    let refusal = registry_refusal_text(&run, "the run whose kept name a foreign ref holds");
    assert!(refusal.contains("already exists"), "{refusal}");
    repair_the_torn_registration(&repo);
    let pin = kept_pin_of(&repo, 1);
    let foreign = pin_target(&repo, &pin).expect("the foreign ref");
    let before = status_of(&repo);
    let first = bounded_resume(
        "the resume beside the foreign ref",
        &repo,
        fake(Effect::EditFile),
    );
    let refusal = resume_refusal(&first, "the resume beside the foreign ref");
    assert!(
        refusal.contains("is not upstroke's kept commit of this attempt"),
        "{refusal}"
    );
    assert_eq!(status_of(&repo), before, "nothing discarded");

    git_in(&repo, &["update-ref", "-d", &pin]);
    let report = bounded_resume(
        "the resume once the ref is gone",
        &repo,
        fake(Effect::EditFile),
    )
    .expect("the resume");
    assert!(committed(&report, "t1"), "{report:?}");
    let commit = pin_target(&repo, &pin).expect("the resume wrote the kept pin");
    assert_ne!(commit, foreign);
    let (_, _, tree) = captured(&repo);
    assert_eq!(tree_of(&repo, &commit), tree, "the pin holds the output");
}

#[test]
fn a_run_that_dies_after_its_capture_loses_nothing_on_resume() {
    let (_tree, repo) = rd1_repo("rd1-e1");
    let mut command = this_test_binary("kept_capture_child_dies_after_its_capture");
    command.env("UPSTROKE_KEPT_CHILD_REPO", &repo);
    let (status, log) = bounded_child(
        "the run that dies after its capture",
        command,
        &repo.with_file_name("dies-after-capture.log"),
    );
    assert_eq!(status.code(), Some(CRASH_EXIT_CODE), "{log}");
    assert!(
        status_of(&repo).contains("agent-output.txt"),
        "the leftovers"
    );
    assert_eq!(
        kept_pins(&repo),
        Vec::<(String, String)>::new(),
        "no pin yet"
    );
    let leftovers = file_bytes(&repo, "agent-output.txt").expect("the leftovers' bytes");

    let report = bounded_resume("the resume", &repo, fake(Effect::EditFile)).expect("the resume");
    assert!(committed(&report, "t1"), "{report:?}");
    let pin = kept_pin_of(&repo, 1);
    let commit = pin_target(&repo, &pin).expect("attempt 1's kept pin");
    assert_eq!(
        blob_at(&repo, &commit, "agent-output.txt"),
        Some(leftovers),
        "the pin holds the leftovers"
    );
    the_restore_brings_the_output_back(&repo, &report, &pin);
}

#[cfg(unix)]
#[test]
fn a_run_that_dies_between_its_pins_commit_and_its_ref_loses_nothing_on_resume() {
    let (_tree, repo) = rd1_repo("rd1-e2");
    let shim = git_shim(
        &repo,
        "e2",
        &[ShimRule {
            subcommand: "update-ref",
            holding: "-kept",
            after: false,
            action: "  kill -KILL $PPID\n  exit 1".to_owned(),
        }],
    );
    let child = child_engine(&repo, "e2");
    let mut command = this_test_binary("kept_engine_child_runs_and_tears_after_capture");
    command
        .env("UPSTROKE_KEPT_ENGINE_REPO", &repo)
        .env("UPSTROKE_KEPT_ENGINE_REPORT", &child.report)
        .env("PATH", shim.path());
    let (status, log) = bounded_child("the run killed at its pin's ref", command, &child.log);
    assert!(
        shim.fired(0),
        "the shim killed the run at the pin's update-ref: {log}"
    );
    assert!(
        status.code().is_none(),
        "the run died by a signal: {status:?}"
    );
    assert!(
        status_of(&repo).contains("agent-output.txt"),
        "the leftovers"
    );
    assert_eq!(
        kept_pins(&repo),
        Vec::<(String, String)>::new(),
        "a commit, no ref"
    );

    repair_the_torn_registration(&repo);
    let report = bounded_resume("the resume", &repo, fake(Effect::EditFile)).expect("the resume");
    assert!(committed(&report, "t1"), "{report:?}");
    let pin = kept_pin_of(&repo, 1);
    let (_, _, tree) = captured(&repo);
    let commit = pin_target(&repo, &pin).expect("the resume wrote the kept pin");
    assert_eq!(tree_of(&repo, &commit), tree, "holding the output");
}

#[test]
fn a_kept_pin_removed_before_the_resume_is_written_again_from_the_checkout() {
    for how in ["update-ref", "fetch-prune"] {
        let (_tree, repo) = rd1_repo(&format!("rd1-f-{how}"));
        let run = bounded_run(how, &repo, Some(tear_after_capture), fake(Effect::EditFile));
        registry_refusal_text(&run, how);
        let pin = kept_pin_of(&repo, 1);
        let first = pin_target(&repo, &pin).expect("the refusal's pin");
        if how == "update-ref" {
            git_in(&repo, &["update-ref", "-d", &pin]);
        } else {
            let remote = repo.with_file_name("remote.git");
            git_in(
                repo.parent().expect("the tree"),
                &["init", "-q", "--bare", &remote.to_string_lossy()],
            );
            let (branch_ref, _, _) = captured(&repo);
            git_in(
                &repo,
                &[
                    "push",
                    "-q",
                    &remote.to_string_lossy(),
                    &format!("{branch_ref}:{branch_ref}"),
                ],
            );
            git_in(
                &repo,
                &[
                    "-c",
                    "maintenance.auto=false",
                    "-c",
                    "gc.auto=0",
                    "fetch",
                    "-q",
                    "--prune",
                    &remote.to_string_lossy(),
                    "+refs/upstroke/*:refs/upstroke/*",
                ],
            );
        }
        assert_eq!(pin_target(&repo, &pin), None, "{how}: the pin is gone");
        repair_the_torn_registration(&repo);
        let report = bounded_resume(how, &repo, fake(Effect::EditFile))
            .unwrap_or_else(|error| panic!("{how}: the resume: {error:?}"));
        assert!(committed(&report, "t1"), "{how}: {report:?}");
        let again = pin_target(&repo, &pin).expect("the resume wrote the pin again");
        assert_eq!(
            tree_of(&repo, &again),
            tree_of(&repo, &first),
            "{how}: from the checkout, which held the output"
        );
        assert!(
            kept_pin_warning(&report).is_some_and(|w| w.contains(&pin)),
            "{how}: and names it: {:?}",
            report.warnings
        );
    }
}

#[test]
fn a_resume_refuses_while_the_checkout_holds_more_than_its_kept_pin() {
    let (_tree, repo) = rd1_repo("rd1-x3");
    let run = bounded_run(
        "the pinned refusal",
        &repo,
        Some(tear_after_capture),
        fake(Effect::EditFile),
    );
    registry_refusal_text(&run, "the pinned refusal");
    let mut text = fs::read_to_string(repo.join("agent-output.txt")).expect("the output");
    text.push_str("the operator's own line\n");
    fs::write(repo.join("agent-output.txt"), &text).expect("an edit after the pin");
    repair_the_torn_registration(&repo);
    let first = bounded_resume(
        "the resume over the operator's edit",
        &repo,
        fake(Effect::EditFile),
    );
    let refusal = resume_refusal(&first, "the resume over the operator's edit");
    assert!(
        refusal.contains("holds otherwise") && refusal.contains("agent-output.txt"),
        "{refusal}"
    );
    assert_eq!(
        fs::read_to_string(repo.join("agent-output.txt")).expect("the output"),
        text,
        "the operator's line stays"
    );
    git_in(&repo, &["reset", "-q", "--hard"]);
    git_in(&repo, &["clean", "-qfd"]);
    let report = bounded_resume(
        "the resume after the operator's clean",
        &repo,
        fake(Effect::EditFile),
    )
    .expect("the resume");
    assert!(committed(&report, "t1"), "{report:?}");
    let (_, _, tree) = captured(&repo);
    let commit = pin_target(&repo, &kept_pin_of(&repo, 1)).expect("the pin");
    assert_eq!(
        tree_of(&repo, &commit),
        tree,
        "the pin holds the captured tree"
    );
}

#[test]
fn leftovers_with_no_attempt_in_flight_are_discarded_as_before() {
    let (_tree, repo, run_id) = parked_run("rd1-x2");
    fs::write(
        repo.join("operator-file.txt"),
        "left with no attempt in flight\n",
    )
    .expect("a leftover");
    let repo_in = repo.clone();
    let run_in = run_id.clone();
    let resumed = bounded("the answered resume", move || {
        resume_answering(&repo_in, &run_in, Effect::EditFile)
    });
    assert!(
        warning_with(&resumed, "discarded").is_some_and(|w| w.contains("operator-file.txt")),
        "the discard warning names the file: {:?}",
        resumed.warnings
    );
    assert_eq!(
        kept_pins(&repo),
        Vec::<(String, String)>::new(),
        "no kept pin is written"
    );
    assert!(kept_copies(&repo).is_empty(), "and no copy");
}

fn a_pinned_refusal(repo: &Path, adapters: impl AdapterSource + Send + 'static) -> String {
    let run = bounded_run(
        "the pinned refusal",
        repo,
        Some(tear_after_capture),
        adapters,
    );
    let refusal = registry_refusal_text(&run, "the pinned refusal");
    assert!(refusal.contains("pinned at"), "{refusal}");
    repair_the_torn_registration(repo);
    kept_pin_of(repo, 1)
}

fn a_blocked_refusal(repo: &Path, adapters: impl AdapterSource + Send + 'static) -> String {
    let run = bounded_run(
        "the blocked refusal",
        repo,
        Some(block_the_kept_pin_and_tear_after_capture),
        adapters,
    );
    let refusal = registry_refusal_text(&run, "the blocked refusal");
    assert!(refusal.contains("failed"), "{refusal}");
    let pin = kept_pin_of(repo, 1);
    git_in(repo, &["update-ref", "-d", &format!("{pin}/blocker")]);
    repair_the_torn_registration(repo);
    pin
}

#[cfg(unix)]
#[test]
fn a_foreign_index_reset_during_the_resumes_capture_cannot_change_the_kept_tree() {
    let (_tree, repo) = rd1_repo("rd2-rd1-1");
    let pin = a_blocked_refusal(&repo, fake(Effect::EditFile));
    let (_, _, tree) = captured(&repo);
    let shim = git_shim(
        &repo,
        "rd1-1",
        &[ShimRule {
            subcommand: "write-tree",
            holding: "write-tree",
            after: false,
            action: format!("  foreign -C '{}' reset -q HEAD -- .", repo.display()),
        }],
    );
    let child = child_engine(&repo, "rd1-1");
    let report = child.run(
        "the resume whose capture another client's reset meets",
        &repo,
        "kept_engine_child_resumes",
        &[("PATH", &shim.path())],
    );
    assert!(
        shim.fired(0),
        "the foreign reset ran before the capture's write-tree"
    );
    assert!(report.starts_with("ok"), "{report}");
    let commit = pin_target(&repo, &pin).expect("the resume pinned the leftovers");
    assert_eq!(
        tree_of(&repo, &commit),
        tree,
        "the pin holds the captured tree, whatever the foreign reset did to the index"
    );
    assert_eq!(
        blob_at(&repo, &commit, "agent-output.txt"),
        Some(b"edited: 0\n".to_vec()),
        "and the output is restorable from it"
    );
}

#[test]
fn an_assume_unchanged_entry_holding_output_is_kept_before_the_discard() {
    let (_tree, repo) = shaped_repo("rd2-rd1-2");
    let pin = a_blocked_refusal(&repo, shaped_worker());
    git_in(&repo, &["reset", "-q", "HEAD", "--", "tracked.txt"]);
    git_in(
        &repo,
        &["update-index", "--assume-unchanged", "tracked.txt"],
    );
    let report = bounded_resume(
        "the resume over an assume-unchanged entry",
        &repo,
        fake(Effect::EditFile),
    )
    .expect("the resume");
    assert!(committed(&report, "t1"), "{report:?}");
    let commit = pin_target(&repo, &pin).expect("the resume pinned the leftovers");
    assert_eq!(
        blob_at(&repo, &commit, "tracked.txt"),
        Some(b"paid edit\n".to_vec()),
        "the pin's tracked.txt is the paid content"
    );
}

#[cfg(unix)]
#[test]
fn the_output_survives_a_pin_deleted_after_the_resumes_last_pin_check() {
    for (label, rule) in [
        (
            "after-list-heads",
            ShimRule {
                subcommand: "bundle",
                holding: ".partial",
                after: true,
                action: String::new(),
            },
        ),
        (
            "before-read-tree-m",
            ShimRule {
                subcommand: "read-tree",
                holding: " -m ",
                after: false,
                action: String::new(),
            },
        ),
    ] {
        let (_tree, repo) = rd1_repo(&format!("rd2-rd1-3-{label}"));
        let pin = a_pinned_refusal(&repo, fake(Effect::EditFile));
        let (_, _, tree) = captured(&repo);
        let holding = if label == "after-list-heads" {
            "list-heads"
        } else {
            rule.holding
        };
        let rule = ShimRule {
            holding,
            action: format!("  foreign -C '{}' update-ref -d '{pin}'", repo.display()),
            ..rule
        };
        let shim = git_shim(&repo, label, &[rule]);
        let child = child_engine(&repo, label);
        let report = child.run(
            label,
            &repo,
            "kept_engine_child_resumes",
            &[("PATH", &shim.path())],
        );
        assert!(
            shim.fired(0),
            "{label}: the pin was deleted: {}",
            shim.calls()
        );
        assert!(report.starts_with("ok"), "{label}: {report}");
        assert_eq!(pin_target(&repo, &pin), None, "{label}: the pin is gone");
        let copies = kept_copies(&repo);
        let [copy] = copies.as_slice() else {
            panic!("{label}: one copy: {copies:?}");
        };
        forget_every_unreferenced_object(&repo);
        restore_the_pin_from(&repo, copy, &pin);
        let commit = pin_target(&repo, &pin).expect("the pin, restored");
        assert_eq!(
            tree_of(&repo, &commit),
            tree,
            "{label}: the copy holds the output"
        );
    }
}

#[test]
fn an_ignore_rule_that_changes_with_the_discard_exposes_nothing_to_it() {
    for (variant, shape, kept) in [
        ("rd1-4-v1", "rd1-4-v1", "newdir/paid-output.txt"),
        ("rd1-4-v3", "rd1-4-v3", "paid-new.txt"),
    ] {
        let (_tree, repo) = rd1_repo(&format!("rd2-{variant}"));
        let before = file_bytes(&repo, kept);
        assert_eq!(before, None, "{variant}: not there before the worker");
        die_inside_an_attempt(&repo, shape);
        let written = file_bytes(&repo, kept).expect("the worker's ignored output");
        let report = bounded_resume(variant, &repo, fake(Effect::EditFile))
            .unwrap_or_else(|error| panic!("{variant}: the resume: {error:?}"));
        assert!(committed(&report, "t1"), "{variant}: {report:?}");
        assert_eq!(
            file_bytes(&repo, kept),
            Some(written),
            "{variant}: the resume leaves the file the rule covered"
        );
    }
}

#[test]
fn an_interrupted_text_write_does_not_stop_the_resume() {
    let (_tree, repo) = rd1_repo("rd2-rd1-5");
    die_inside_an_attempt(&repo, "rd1-5");
    let report = bounded_resume(
        "the resume over an interrupted text write",
        &repo,
        fake(Effect::EditFile),
    )
    .expect("the resume completes");
    assert!(committed(&report, "t1"), "{report:?}");
    assert_eq!(status_of(&repo), "", "and the checkout is clean");
    let commit = pin_target(&repo, &kept_pin_of(&repo, 1)).expect("the kept pin");
    assert_eq!(
        blob_at(&repo, &commit, "tracked.txt"),
        Some(b"paid text cut mid-character \xe2\x82".to_vec()),
        "the kept pin holds the bytes exactly"
    );
}

struct PartWay {
    #[cfg(unix)]
    directory: PathBuf,
    #[cfg(windows)]
    _held: fs::File,
}

impl PartWay {
    fn hold(repo: &Path, file: &str) -> Self {
        let path = repo.join(file);
        #[cfg(unix)]
        {
            let directory = path.parent().expect("the file's directory").to_path_buf();
            mode_of(&directory, 0o555).expect("a directory nothing can write");
            let binds = mode_bits_bind(&directory);
            if !binds {
                mode_of(&directory, 0o755).expect("writable again");
            }
            assert!(
                binds,
                "prerequisite not met: the mode bit did not bind (root, or CAP_DAC_OVERRIDE)"
            );
            Self { directory }
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            const FILE_SHARE_READ: u32 = 1;
            let held = fs::OpenOptions::new()
                .read(true)
                .share_mode(FILE_SHARE_READ)
                .open(&path)
                .expect("the file, held open");
            Self { _held: held }
        }
    }

    fn heal(self) {
        #[cfg(unix)]
        mode_of(&self.directory, 0o755).expect("writable again");
    }
}

#[cfg(any(unix, windows))]
#[test]
fn a_discard_stopped_part_way_finishes_on_the_next_resume_without_operator_cleanup() {
    for (form, shape, held) in [
        (
            "unlink",
            paid_output_in_a_new_directory as Shape,
            "newdir/inner.txt",
        ),
        ("write", paid_edit_to_a_tracked_file as Shape, "sub/t.txt"),
    ] {
        let (_tree, repo) = rd1_repo(&format!("rd2-greset-{form}"));
        let pin = a_pinned_refusal(&repo, shaping(shape));
        let (_, _, tree) = captured(&repo);
        let fault = PartWay::hold(&repo, held);
        let first = bounded_resume(form, &repo, fake(Effect::EditFile));
        fault.heal();
        assert!(
            first.is_err(),
            "{form}: the fault stops the first resume: {first:?}"
        );
        let report = bounded_resume(form, &repo, fake(Effect::EditFile))
            .unwrap_or_else(|error| panic!("{form}: the second resume: {error:?}"));
        assert!(committed(&report, "t1"), "{form}: {report:?}");
        assert_eq!(status_of(&repo), "", "{form}: the tree is clean");
        let commit = pin_target(&repo, &pin).expect("the pin");
        assert_eq!(
            tree_of(&repo, &commit),
            tree,
            "{form}: the pin holds the output"
        );
    }
}

fn latin1_output(root: &Path) -> std::io::Result<()> {
    write_into(root, "latin1.txt", b"caf\xe9 au lait\n")
}

#[test]
fn an_undecodable_diff_keeps_the_output_pinned() {
    let (_tree, repo) = rd1_repo("rd2-n1a");
    let run = bounded_run(
        "the run whose diff is not UTF-8",
        &repo,
        None,
        shaping(latin1_output),
    );
    let Err(UpstrokeError::WithWarnings(warned)) = &run else {
        panic!("the capture's own refusal, with the kept note: {run:?}");
    };
    assert!(
        warned
            .warnings
            .iter()
            .any(|warning| warning.contains("is kept in this checkout and pinned at")),
        "{run:?}"
    );
    assert_eq!(
        file_bytes(&repo, "latin1.txt"),
        Some(b"caf\xe9 au lait\n".to_vec()),
        "the checkout keeps the file"
    );
    let kept = kept_pins(&repo);
    let [(_, commit)] = kept.as_slice() else {
        panic!("one kept pin: {kept:?}");
    };
    assert_eq!(
        blob_at(&repo, commit, "latin1.txt"),
        Some(b"caf\xe9 au lait\n".to_vec()),
        "the pin holds it"
    );
}

#[cfg(unix)]
#[test]
fn a_branch_moved_during_the_capture_keeps_the_output_pinned() {
    for put_back in ["update-ref", "reset-hard"] {
        let (_tree, repo) = rd1_repo(&format!("rd2-n1c-{put_back}"));
        let recorded = git_in(&repo, &["rev-parse", "HEAD"]).trim().to_owned();
        let move_it = format!(
            "  r='{}'\n  b=$(foreign -C \"$r\" symbolic-ref HEAD)\n  m=$(foreign -C \"$r\" commit-tree HEAD^{{tree}} -p HEAD -m moved-during-capture)\n  foreign -C \"$r\" update-ref \"$b\" \"$m\"",
            repo.display()
        );
        let shim = git_shim(
            &repo,
            put_back,
            &[ShimRule {
                subcommand: "diff",
                holding: "--no-textconv",
                after: false,
                action: move_it,
            }],
        );
        let child = child_engine(&repo, put_back);
        let report = child.run(
            put_back,
            &repo,
            "kept_engine_child_runs",
            &[("PATH", &shim.path())],
        );
        assert!(
            shim.fired(0),
            "{put_back}: the branch moved during the capture"
        );
        assert!(
            report.contains("while capturing the candidate")
                && report.contains("is kept in this checkout and pinned at"),
            "{put_back}: the capture's refusal, with the kept note: {report}"
        );
        let pin = kept_pin_of(&repo, 1);
        let commit = pin_target(&repo, &pin).expect("the kept pin");
        assert_eq!(
            blob_at(&repo, &commit, "agent-output.txt"),
            Some(b"edited: 0\n".to_vec()),
            "{put_back}: the pin holds the output"
        );
        let branch = git_in(&repo, &["symbolic-ref", "HEAD"]).trim().to_owned();
        if put_back == "update-ref" {
            git_in(&repo, &["update-ref", &branch, &recorded]);
            assert!(
                status_of(&repo).contains("agent-output.txt"),
                "the checkout keeps it"
            );
        } else {
            git_in(&repo, &["reset", "-q", "--hard", &recorded]);
            assert_eq!(status_of(&repo), "", "the pin is the only copy");
        }
        let resumed = bounded_resume(put_back, &repo, fake(Effect::EditFile))
            .unwrap_or_else(|error| panic!("{put_back}: the resume: {error:?}"));
        assert!(committed(&resumed, "t1"), "{put_back}: {resumed:?}");
        assert!(
            kept_pin_warning(&resumed).is_some_and(|w| w.contains(&pin)),
            "{put_back}: the resume names the pin: {:?}",
            resumed.warnings
        );
        assert_eq!(
            pin_target(&repo, &pin),
            Some(commit),
            "{put_back}: and keeps it"
        );
    }
}

#[test]
fn a_reviewed_candidate_refused_on_a_moved_head_is_kept_and_pinned() {
    for put_back in ["kept", "reset-hard"] {
        let (_tree, repo) = rd1_repo(&format!("rd2-n2a-{put_back}"));
        let run = bounded_run(
            put_back,
            &repo,
            Some(advance_the_branch_after_capture),
            fake(Effect::EditFile),
        );
        let Err(UpstrokeError::WithWarnings(warned)) = &run else {
            panic!("{put_back}: the publication's refusal, with the kept note: {run:?}");
        };
        assert!(
            warned.error.to_string().contains("HEAD moved"),
            "{put_back}: {run:?}"
        );
        assert!(
            warned.warnings.iter().any(|warning| warning.contains(
                "the reviewed candidate of attempt 1 of `t1` is kept in this checkout and pinned at"
            )),
            "{put_back}: {run:?}"
        );
        let (_, parent, tree) = captured(&repo);
        let pin = kept_pin_of(&repo, 1);
        let commit = pin_target(&repo, &pin).expect("the kept pin");
        assert_eq!(
            tree_of(&repo, &commit),
            tree,
            "{put_back}: the captured tree"
        );
        if put_back == "kept" {
            assert!(
                status_of(&repo).contains("agent-output.txt"),
                "the checkout keeps it"
            );
        } else {
            git_in(&repo, &["reset", "-q", "--hard", &parent]);
            assert_eq!(status_of(&repo), "", "only the pin holds it");
            assert_eq!(
                blob_at(&repo, &commit, "agent-output.txt"),
                Some(b"edited: 0\n".to_vec())
            );
        }
    }
}

fn switch_away_then_write(root: &Path) -> std::io::Result<()> {
    git_from_a_shape(root, &["switch", "-q", "-c", "elsewhere"])?;
    paid_output(root)
}

#[test]
fn a_candidate_captured_on_another_branch_is_kept_and_pinned() {
    let (_tree, repo) = rd1_repo("rd2-n2b");
    let run = bounded_run(
        "the run whose worker switched branches",
        &repo,
        None,
        shaping(switch_away_then_write),
    );
    let Err(UpstrokeError::WithWarnings(warned)) = &run else {
        panic!("the publication's refusal, with the kept note: {run:?}");
    };
    assert!(
        warned.error.to_string().contains("refusing publication"),
        "{run:?}"
    );
    assert!(
        warned
            .warnings
            .iter()
            .any(|warning| warning.contains("is kept in this checkout and pinned at")),
        "{run:?}"
    );
    let commit = pin_target(&repo, &kept_pin_of(&repo, 1)).expect("the kept pin");
    assert_eq!(
        blob_at(&repo, &commit, "agent-output.txt"),
        Some(b"paid output\n".to_vec()),
        "the pin holds the candidate"
    );
    assert_eq!(
        file_bytes(&repo, "agent-output.txt"),
        Some(b"paid output\n".to_vec()),
        "and the checkout keeps it"
    );
}

#[derive(Default)]
struct FailTheLegacyAppendNumbered<const N: u32> {
    entered: u32,
}

impl<const N: u32> crate::events::log::EventHooks for FailTheLegacyAppendNumbered<N> {
    fn point(
        &mut self,
        site: EventSite,
        point: crate::topology::effects::SubEffectPoint,
        mode: crate::topology::effects::InjectionMode,
    ) -> crate::topology::effects::Injection {
        use crate::topology::effects::{Injection, InjectionMode, SubEffectPoint};
        if site != EventSite::LegacyAppend
            || point != SubEffectPoint::Written
            || mode != InjectionMode::ErrorReturn
        {
            return Injection::Proceed;
        }
        self.entered += 1;
        if self.entered == N {
            Injection::Error
        } else {
            Injection::Proceed
        }
    }
}

const PASSING_SETTLEMENT_APPEND: u32 = 4;

fn fail_the_passing_settlement() -> Box<dyn crate::events::log::EventHooks> {
    Box::<FailTheLegacyAppendNumbered<PASSING_SETTLEMENT_APPEND>>::default()
}

#[test]
fn a_reviewed_candidate_whose_settlement_is_not_appended_is_kept() {
    let (_tree, repo) = rd1_repo("rd2-n2c");
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    opts.log_hooks = Some(fail_the_passing_settlement);
    let run = bounded("the run whose settlement fails", move || {
        run_with(&opts, &fake(Effect::EditFile))
    });
    let Err(UpstrokeError::WithWarnings(warned)) = &run else {
        panic!("the append's own error, with the kept note: {run:?}");
    };
    assert!(
        warned.error.to_string().contains("Event.LegacyAppend"),
        "{run:?}"
    );
    let run_id = rundir::latest_run(&repo).expect("the run");
    let log = fs::read_to_string(paths_of(&repo, &run_id).events()).expect("the log");
    let last = log.lines().last().expect("a line");
    assert!(
        last.contains("\"attempt_finished\"") && !last.ends_with('}'),
        "the failed append was the attempt's settlement, cut short: {last}"
    );
    let orphan = super::coordinator::prepared_pin_ref(&run_id, 0, 1);
    assert!(
        pin_target(&repo, &orphan).is_some(),
        "the prepared pin was written"
    );
    let pin = kept_pin_of(&repo, 1);
    let commit = pin_target(&repo, &pin).expect("the kept pin");
    assert!(
        status_of(&repo).contains("agent-output.txt"),
        "the checkout keeps the candidate"
    );
    let resumed = bounded_resume("the resume", &repo, fake(Effect::EditFile)).expect("the resume");
    assert!(
        warning_with(&resumed, "removed orphan prepared commit pin").is_some(),
        "{:?}",
        resumed.warnings
    );
    assert_eq!(pin_target(&repo, &orphan), None, "the orphan is removed");
    assert_eq!(pin_target(&repo, &pin), Some(commit), "the kept pin is not");
    assert!(
        kept_pin_warning(&resumed).is_some_and(|w| w.contains(&pin)),
        "and is named: {:?}",
        resumed.warnings
    );
}

#[cfg(unix)]
#[test]
fn an_unreadable_leftover_and_a_nested_repository_are_left_in_place_and_named() {
    let (_tree, repo) = rd1_repo("rd2-x10");
    die_inside_an_attempt(&repo, "x10");
    let locked = repo.join("locked.txt");
    let binds = fs::read(&locked).is_err();
    if !binds {
        mode_of(&locked, 0o644).expect("readable again");
    }
    assert!(binds, "prerequisite not met: mode 0 did not bind (root)");
    let resumed = bounded_resume("the resume", &repo, fake(Effect::EditFile));
    mode_of(&locked, 0o644).expect("readable again");
    assert!(
        matches!(&resumed, Err(error) if error.to_string().contains("git add -A failed")),
        "the resumed attempt's own capture meets what the discard left: {resumed:?}"
    );
    let run_id = rundir::latest_run(&repo).expect("the run");
    let log = fs::read_to_string(paths_of(&repo, &run_id).events()).expect("the log");
    let resumed_line = log
        .lines()
        .find(|line| line.contains("\"run_resumed\""))
        .expect("the guarded discard completed and the resume was recorded");
    assert!(
        resumed_line.contains("agent-output.txt")
            && !resumed_line.contains("locked.txt")
            && !resumed_line.contains("nested"),
        "the record names what was discarded, not what was left: {resumed_line}"
    );
    assert!(
        repo.join("nested").join(".git").is_dir(),
        "the nested repository stays"
    );
    assert_eq!(
        file_bytes(&repo, "locked.txt"),
        Some(b"paid, unreadable\n".to_vec())
    );
    let commit = pin_target(&repo, &kept_pin_of(&repo, 1)).expect("the kept pin");
    assert_eq!(
        blob_at(&repo, &commit, "agent-output.txt"),
        Some(b"paid output\n".to_vec()),
        "the pin holds the output"
    );
}

#[test]
fn an_operators_new_file_after_a_pinned_refusal_stays_in_the_checkout() {
    let (_tree, repo) = rd1_repo("rd2-x16");
    a_pinned_refusal(&repo, fake(Effect::EditFile));
    fs::write(
        repo.join("operator-notes.txt"),
        "the operator's own notes\n",
    )
    .expect("a new file");
    let report = bounded_resume("the resume", &repo, fake(Effect::EditFile)).expect("the resume");
    assert!(committed(&report, "t1"), "{report:?}");
    assert_eq!(
        file_bytes(&repo, "operator-notes.txt"),
        Some(b"the operator's own notes\n".to_vec()),
        "the file stays"
    );
    assert!(
        warning_with(&report, "remain in this checkout")
            .is_some_and(|w| w.contains("operator-notes.txt")),
        "named: {:?}",
        report.warnings
    );
}

#[test]
fn the_discard_reads_the_checkouts_per_worktree_ignore_rules() {
    let (_tree, repo) = rd1_repo("rd2-x18");
    git_in(&repo, &["config", "extensions.worktreeConfig", "true"]);
    let excludes = repo.with_file_name("worktree-excludes");
    fs::write(&excludes, "cache.bin\n").expect("the worktree's excludes");
    git_in(
        &repo,
        &[
            "config",
            "--worktree",
            "core.excludesFile",
            &excludes.to_string_lossy(),
        ],
    );
    die_inside_an_attempt(&repo, "x18");
    let report = bounded_resume("the resume", &repo, fake(Effect::EditFile)).expect("the resume");
    assert!(committed(&report, "t1"), "{report:?}");
    assert_eq!(
        file_bytes(&repo, "cache.bin"),
        Some(b"ignored by this worktree's own rules\n".to_vec()),
        "cache.bin stays"
    );
    for (pin, commit) in kept_pins(&repo) {
        assert_eq!(
            blob_at(&repo, &commit, "cache.bin"),
            None,
            "and no pin holds it: {pin}"
        );
    }
}

#[cfg(unix)]
#[test]
fn a_branch_moved_during_the_discard_refuses_with_the_output_kept() {
    let (_tree, repo) = rd1_repo("rd2-x19");
    let pin = a_pinned_refusal(&repo, fake(Effect::EditFile));
    let (branch_ref, _, tree) = captured(&repo);
    let shim = git_shim(
        &repo,
        "x19",
        &[ShimRule {
            subcommand: "read-tree",
            holding: " -m ",
            after: false,
            action: format!(
                "  r='{}'\n  m=$(foreign -C \"$r\" commit-tree HEAD^{{tree}} -p HEAD -m moved-during-discard)\n  foreign -C \"$r\" update-ref '{branch_ref}' \"$m\"",
                repo.display()
            ),
        }],
    );
    let child = child_engine(&repo, "x19");
    let report = child.run(
        "the resume",
        &repo,
        "kept_engine_child_resumes",
        &[("PATH", &shim.path())],
    );
    assert!(shim.fired(0), "the branch moved before the revert");
    assert!(
        report.starts_with("err") && report.contains("during the discard"),
        "the resume refuses naming the move: {report}"
    );
    let commit = pin_target(&repo, &pin).expect("the pin");
    assert_eq!(tree_of(&repo, &commit), tree, "the pin holds the output");
    let copies = kept_copies(&repo);
    let [copy] = copies.as_slice() else {
        panic!("one copy: {copies:?}");
    };
    git_in(&repo, &["update-ref", "-d", &pin]);
    forget_every_unreferenced_object(&repo);
    restore_the_pin_from(&repo, copy, &pin);
    assert_eq!(
        tree_of(&repo, &pin_target(&repo, &pin).expect("restored")),
        tree,
        "and so does its copy"
    );
}

#[test]
fn a_nested_repository_with_a_commit_does_not_wedge_the_resume() {
    let (_tree, repo) = rd1_repo("rd2-x20");
    die_inside_an_attempt(&repo, "x20");
    let nested_head = git_in(&repo.join("nested"), &["rev-parse", "HEAD"])
        .trim()
        .to_owned();
    let run_id = rundir::latest_run(&repo).expect("a run to resume");
    let log = paths_of(&repo, &run_id).events();
    let recorded = fs::read(&log).expect("the event log");
    let writable = fs::metadata(&log).expect("the event log").permissions();
    let mut read_only = writable.clone();
    read_only.set_readonly(true);
    fs::set_permissions(&log, read_only).expect("the event log made read-only");
    let first = bounded_resume("the first resume", &repo, fake(Effect::EditFile));
    fs::set_permissions(&log, writable).expect("the event log made writable again");
    match &first {
        Err(UpstrokeError::Resume { message, .. }) => {
            panic!("the first resume does not refuse: {message}")
        }
        Err(_) => {}
        Ok(report) => panic!(
            "prerequisite not met: the read-only event log took a write (root, or \
             CAP_DAC_OVERRIDE): {report:?}"
        ),
    }
    assert_eq!(
        fs::read(&log).expect("the event log"),
        recorded,
        "the first resume recorded nothing, so the second finds the same attempt in flight"
    );
    assert!(
        repo.join("nested").join(".git").is_dir(),
        "the repository stays"
    );
    let report = bounded_resume(
        "the second resume, over the same attempt",
        &repo,
        fake(Effect::EditFile),
    )
    .expect("the second resume does not refuse");
    assert!(
        repo.join("nested").join(".git").is_dir(),
        "the repository stays: {:?}",
        report.warnings
    );
    assert_eq!(
        git_in(&repo.join("nested"), &["rev-parse", "HEAD"]).trim(),
        nested_head,
        "and the nested repository stays as it was"
    );
}

fn sha256_of(path: &Path) -> String {
    use sha2::Digest;
    format!(
        "{:x}",
        sha2::Sha256::digest(fs::read(path).expect("a copy"))
    )
}

fn restored_warning(report: &RunReport) -> Option<&String> {
    warning_with(report, "was put back from its copy")
}

fn a_first_resume_stopped_part_way(repo: &Path) -> PathBuf {
    let fault = PartWay::hold(repo, "newdir/inner.txt");
    let first = bounded_resume("the first resume", repo, fake(Effect::EditFile));
    fault.heal();
    let refusal = resume_refusal(&first, "the first resume");
    assert!(refusal.contains("stopped part-way"), "{refusal}");
    let copies = kept_copies(repo);
    let [copy] = copies.as_slice() else {
        panic!("the first resume wrote one copy: {copies:?}");
    };
    copy.clone()
}

#[cfg(any(unix, windows))]
#[test]
fn a_retry_after_a_deleted_pin_restores_it_from_the_untouched_copy() {
    let (_tree, repo) = rd1_repo("rd3-rd2-1");
    let pin = a_pinned_refusal(&repo, shaping(paid_output_in_a_new_directory));
    let (_, _, tree) = captured(&repo);
    let original = pin_target(&repo, &pin).expect("the refusal's pin");
    let copy = a_first_resume_stopped_part_way(&repo);
    let copied = sha256_of(&copy);
    git_in(&repo, &["update-ref", "-d", &pin]);
    let report = bounded_resume("the second resume", &repo, fake(Effect::EditFile))
        .expect("the second resume completes");
    assert!(committed(&report, "t1"), "{report:?}");
    assert_eq!(sha256_of(&copy), copied, "the first copy is byte-identical");
    assert_eq!(
        pin_target(&repo, &pin),
        Some(original.clone()),
        "the kept ref is the original commit again"
    );
    assert_eq!(tree_of(&repo, &original), tree, "holding the output");
    let restored = restored_warning(&report).expect("the restored warning");
    assert!(
        restored.contains(&copy.display().to_string()),
        "it names the copy the pin came back from: {restored}"
    );
    git_in(&repo, &["update-ref", "-d", &pin]);
    forget_every_unreferenced_object(&repo);
    restore_the_pin_from(&repo, &copy, &pin);
    assert_eq!(
        tree_of(&repo, &pin_target(&repo, &pin).expect("restored")),
        tree,
        "the output still restores after every reflog expired and gc --prune=now"
    );
}

#[cfg(unix)]
#[test]
fn the_output_survives_its_pin_deleted_after_the_restore() {
    let (_tree, repo) = rd1_repo("rd3-rd2-1b");
    let pin = a_pinned_refusal(&repo, shaping(paid_output_in_a_new_directory));
    let (_, _, tree) = captured(&repo);
    a_first_resume_stopped_part_way(&repo);
    git_in(&repo, &["update-ref", "-d", &pin]);
    let shim = git_shim(
        &repo,
        "rd2-1b",
        &[ShimRule {
            subcommand: "read-tree",
            holding: " -m ",
            after: false,
            action: format!("  foreign -C '{}' update-ref -d '{pin}'", repo.display()),
        }],
    );
    let child = child_engine(&repo, "rd2-1b");
    let report = child.run(
        "the second resume",
        &repo,
        "kept_engine_child_resumes",
        &[("PATH", &shim.path())],
    );
    assert!(shim.fired(0), "the pin was deleted again after its restore");
    assert!(report.starts_with("ok"), "{report}");
    assert_eq!(pin_target(&repo, &pin), None);
    forget_every_unreferenced_object(&repo);
    for copy in kept_copies(&repo) {
        restore_the_pin_from(&repo, &copy, &pin);
        assert_eq!(
            tree_of(&repo, &pin_target(&repo, &pin).expect("restored")),
            tree,
            "the copy {} alone holds the output",
            copy.display()
        );
        git_in(&repo, &["update-ref", "-d", &pin]);
    }
}

#[cfg(any(unix, windows))]
#[test]
fn a_kept_pin_moved_from_its_copy_refuses_and_discards_nothing() {
    let (_tree, repo) = rd1_repo("rd3-rd2-1m");
    let pin = a_pinned_refusal(&repo, shaping(paid_output_in_a_new_directory));
    let (branch_ref, _, _) = captured(&repo);
    let copy = a_first_resume_stopped_part_way(&repo);
    let run_id = rundir::latest_run(&repo).expect("the run");
    let other_name = format!(
        "{}{}",
        super::coordinator::prepared_pin_ref(&run_id, 0, 9),
        crate::workspace::KEPT_PIN_SUFFIX
    );
    let head = git_in(&repo, &["rev-parse", "HEAD"]).trim().to_owned();
    let head_tree = tree_of(&repo, &head);
    let other = Workspace::open(&repo)
        .expect("the workspace")
        .prepare_commit_from_candidate(
            &branch_ref,
            &head,
            &head_tree,
            "[upstroke] kept: t1 attempt 1",
            &other_name,
        )
        .expect("another kept-looking commit")
        .commit_sha;
    git_in(&repo, &["update-ref", &pin, &other]);
    git_in(&repo, &["update-ref", "-d", &other_name]);
    let before = status_of(&repo);
    let second = bounded_resume("the second resume", &repo, fake(Effect::EditFile));
    let refusal = resume_refusal(&second, "the second resume");
    assert!(
        refusal.contains(&format!("`{pin}` names {other}, but its copy"))
            && refusal.contains(&copy.display().to_string()),
        "the refusal names the pin and the copy: {refusal}"
    );
    assert_eq!(status_of(&repo), before, "the checkout keeps what it held");
}

fn plant_a_copy(repo: &Path, pin: &str, suffix: &str) -> PathBuf {
    let run_id = rundir::latest_run(repo).expect("the run");
    let head = git_in(repo, &["rev-parse", "HEAD"]).trim().to_owned();
    let planted = paths_of(repo, &run_id)
        .public
        .join(format!("kept-0-1-{}{suffix}", crate::ulid::ulid()));
    git_in(
        repo,
        &[
            "bundle",
            "create",
            &planted.to_string_lossy(),
            pin,
            &format!("^{head}"),
        ],
    );
    planted
}

fn cut_to_half(path: &Path) {
    let bytes = fs::read(path).expect("the file");
    fs::write(path, bytes.get(..bytes.len() / 2).expect("its first half")).expect("cut short");
}

#[test]
fn a_copy_cut_short_before_its_rename_is_never_taken_for_the_copy() {
    let (_tree, repo) = rd1_repo("rd3-rd2-1c");
    let pin = a_pinned_refusal(&repo, fake(Effect::EditFile));
    let (_, _, tree) = captured(&repo);
    let partial = plant_a_copy(&repo, &pin, ".partial");
    cut_to_half(&partial);
    let report = bounded_resume("the resume", &repo, fake(Effect::EditFile)).expect("the resume");
    assert!(committed(&report, "t1"), "{report:?}");
    let copies = kept_copies(&repo);
    let [copy] = copies.as_slice() else {
        panic!("the resume wrote one copy of its own: {copies:?}");
    };
    git_in(&repo, &["update-ref", "-d", &pin]);
    forget_every_unreferenced_object(&repo);
    restore_the_pin_from(&repo, copy, &pin);
    assert_eq!(
        tree_of(&repo, &pin_target(&repo, &pin).expect("restored")),
        tree,
        "the output restores from the new copy"
    );
}

#[cfg(unix)]
#[test]
fn a_copy_cut_short_is_never_restored_from() {
    let (_tree, repo) = rd1_repo("rd4-rd2-1cr");
    let pin = a_pinned_refusal(&repo, fake(Effect::EditFile));
    let (_, _, tree) = captured(&repo);
    let crash = format!(
        "  r='{}'\n  f=''\n  take=''\n  for a in \"$@\"; do\n    if [ -n \"$take\" ]; then f=\"$a\"; break; fi\n    [ \"$a\" = create ] && take=1\n  done\n  size=$(wc -c < \"$r/$f\")\n  head -c $((size / 2)) \"$r/$f\" > \"$r/$f.cut\"\n  mv \"$r/$f.cut\" \"$r/$f\"\n  kill -KILL $PPID\n  exit 1",
        repo.display()
    );
    let shim = git_shim(
        &repo,
        "rd2-1cr",
        &[ShimRule {
            subcommand: "bundle",
            holding: " create ",
            after: true,
            action: crash,
        }],
    );
    let child = child_engine(&repo, "rd2-1cr");
    let mut command = this_test_binary("kept_engine_child_resumes");
    command
        .env("UPSTROKE_KEPT_ENGINE_REPO", &repo)
        .env("UPSTROKE_KEPT_ENGINE_REPORT", &child.report)
        .env("PATH", shim.path());
    let (status, log) = bounded_child(
        "the resume that dies after its bundle create",
        command,
        &child.log,
    );
    assert!(
        shim.fired(0),
        "the resume died right after `bundle create`: {log}"
    );
    assert!(status.code().is_none(), "by a signal: {status:?}");
    assert!(
        kept_copies(&repo).is_empty(),
        "the crash left no final name, only the copy it cut short"
    );
    git_in(&repo, &["update-ref", "-d", &pin]);
    let report = bounded_resume("the next resume", &repo, fake(Effect::EditFile))
        .expect("the next resume pins afresh and completes");
    assert!(committed(&report, "t1"), "{report:?}");
    assert!(
        restored_warning(&report).is_none(),
        "nothing was restored from the cut copy"
    );
    let commit = pin_target(&repo, &pin).expect("pinned afresh");
    assert_eq!(
        tree_of(&repo, &commit),
        tree,
        "from the checkout, which held the output"
    );
    assert_eq!(kept_copies(&repo).len(), 1, "and copied");
}

fn the_newest_copy_holds(repo: &Path, pin: &str, path: &str, bytes: &[u8]) {
    let copy = kept_copies(repo).into_iter().max().expect("a copy");
    let kept = pin_target(repo, pin).expect("the pin");
    git_in(repo, &["update-ref", "-d", pin]);
    forget_every_unreferenced_object(repo);
    restore_the_pin_from(repo, &copy, pin);
    assert_eq!(
        pin_target(repo, pin),
        Some(kept),
        "the copy names the pin's commit"
    );
    assert_eq!(
        blob_at(repo, pin, path),
        Some(bytes.to_vec()),
        "the copy holds {path}"
    );
}

#[test]
fn an_ignored_directory_where_head_has_a_file_is_pinned_before_the_discard() {
    let (_tree, repo) = rd1_repo("rd3-rd2-2");
    die_inside_an_attempt(&repo, "rd2-2");
    let report = bounded_resume("the resume", &repo, fake(Effect::EditFile)).expect("the resume");
    assert!(committed(&report, "t1"), "{report:?}");
    assert_eq!(
        file_bytes(&repo, "tracked.txt"),
        Some(b"base\n".to_vec()),
        "HEAD's file"
    );
    let pin = kept_pin_of(&repo, 1);
    assert_eq!(
        blob_at(&repo, &pin, "tracked.txt/paid.txt"),
        Some(b"paid and ignored\n".to_vec()),
        "the pin holds paid.txt"
    );
    the_newest_copy_holds(&repo, &pin, "tracked.txt/paid.txt", b"paid and ignored\n");
}

#[test]
fn an_ignored_file_where_head_has_a_directory_is_pinned_before_the_discard() {
    let (_tree, repo) = rd1_repo("rd3-rd2-2r");
    die_inside_an_attempt(&repo, "rd2-2r");
    let report = bounded_resume("the resume", &repo, fake(Effect::EditFile)).expect("the resume");
    assert!(committed(&report, "t1"), "{report:?}");
    assert_eq!(
        file_bytes(&repo, "sub/t.txt"),
        Some(b"sub base\n".to_vec()),
        "HEAD's directory"
    );
    let pin = kept_pin_of(&repo, 1);
    assert_eq!(
        blob_at(&repo, &pin, "sub"),
        Some(b"paid, a file where a directory was\n".to_vec()),
        "the pin holds the file"
    );
    the_newest_copy_holds(&repo, &pin, "sub", b"paid, a file where a directory was\n");
}

#[test]
fn an_obstruction_a_refusals_pin_lacks_is_refused_and_kept() {
    let (_tree, repo) = rd1_repo("rd3-rd2-2p");
    a_pinned_refusal(&repo, shaping(ignored_directory_where_head_has_a_file));
    let resumed = bounded_resume("the resume", &repo, fake(Effect::EditFile));
    let refusal = resume_refusal(&resumed, "the resume");
    assert!(
        refusal.contains("`tracked.txt/paid.txt`") && refusal.contains("move them out of the way"),
        "{refusal}"
    );
    assert_eq!(
        file_bytes(&repo, "tracked.txt/paid.txt"),
        Some(b"paid and ignored\n".to_vec()),
        "the checkout keeps it"
    );
}

#[cfg(unix)]
#[test]
fn an_unreadable_file_in_the_discards_way_is_refused_and_kept() {
    let (_tree, repo) = rd1_repo("rd3-rd2-2u");
    die_inside_an_attempt(&repo, "rd2-2u");
    let locked = repo.join("tracked.txt").join("locked.txt");
    let binds = fs::read(&locked).is_err();
    let first = bounded_resume("the first resume", &repo, fake(Effect::EditFile));
    let second = bounded_resume("the second resume", &repo, fake(Effect::EditFile));
    mode_of(&locked, 0o644).expect("readable again");
    assert!(binds, "prerequisite not met: mode 0 did not bind (root)");
    for (which, resumed) in [("first", &first), ("second", &second)] {
        let refusal = resume_refusal(resumed, which);
        assert!(
            refusal.contains("keeping them before the discard stopped"),
            "{which}: {refusal}"
        );
    }
    assert_eq!(
        file_bytes(&repo, "tracked.txt/paid.txt"),
        Some(b"paid and ignored\n".to_vec())
    );
    assert_eq!(
        file_bytes(&repo, "tracked.txt/locked.txt"),
        Some(b"paid, unreadable\n".to_vec())
    );
}

#[test]
fn a_nested_repository_in_the_discards_way_is_refused_and_kept() {
    let (_tree, repo) = rd1_repo("rd3-rd2-2n");
    die_inside_an_attempt(&repo, "rd2-2n");
    let nested = repo.join("tracked.txt").join("nested");
    let nested_head = git_in(&nested, &["rev-parse", "HEAD"]).trim().to_owned();
    for which in ["first", "second"] {
        let resumed = bounded_resume(which, &repo, fake(Effect::EditFile));
        let refusal = resume_refusal(&resumed, which);
        assert!(refusal.contains("stopped part-way"), "{which}: {refusal}");
    }
    let pin = kept_pin_of(&repo, 1);
    assert_eq!(
        blob_at(&repo, &pin, "tracked.txt/paid.txt"),
        Some(b"paid and ignored\n".to_vec()),
        "the pin holds the paid file"
    );
    assert_eq!(
        git_in(&nested, &["rev-parse", "HEAD"]).trim(),
        nested_head,
        "the nested repository stays"
    );
}

#[cfg(unix)]
#[test]
fn a_file_written_after_the_capture_is_never_written_over() {
    let (_tree, repo) = rd1_repo("rd3-rd2-2b");
    die_inside_an_attempt(&repo, "rd2-2b");
    let shim = git_shim(
        &repo,
        "rd2-2b",
        &[ShimRule {
            subcommand: "read-tree",
            holding: " -m ",
            after: false,
            action: format!(
                "  printf 'new uncaptured paid bytes\\n' > '{}'",
                repo.join("deleted.txt").display()
            ),
        }],
    );
    let child = child_engine(&repo, "rd2-2b");
    let report = child.run(
        "the first resume",
        &repo,
        "kept_engine_child_resumes",
        &[("PATH", &shim.path())],
    );
    assert!(shim.fired(0), "the late write ran before the revert");
    assert!(
        report.starts_with("err")
            && report.contains("stopped part-way")
            && report.contains("deleted.txt"),
        "{report}"
    );
    assert_eq!(
        file_bytes(&repo, "deleted.txt"),
        Some(b"new uncaptured paid bytes\n".to_vec()),
        "the bytes written after the capture stay"
    );
    let next = bounded_resume("the next resume", &repo, fake(Effect::EditFile));
    let refusal = resume_refusal(&next, "the next resume");
    assert!(
        refusal.contains("holds otherwise"),
        "X3's refusal: {refusal}"
    );
    assert_eq!(
        file_bytes(&repo, "deleted.txt"),
        Some(b"new uncaptured paid bytes\n".to_vec())
    );
}

fn autocrlf_through(repo: &Path, condition: &str) {
    let settings = repo.with_file_name("autocrlf.inc");
    fs::write(&settings, "[core]\n\tautocrlf = true\n").expect("the included settings");
    let settings = settings.to_string_lossy().replace('\\', "/");
    let git_dir = git_in(repo, &["rev-parse", "--absolute-git-dir"])
        .trim()
        .to_owned();
    match condition {
        "onbranch" => {
            git_in(
                repo,
                &[
                    "config",
                    "--add",
                    "includeIf.onbranch:upstroke/**.path",
                    &settings,
                ],
            );
        }
        "gitdir-exact" => {
            git_in(
                repo,
                &[
                    "config",
                    "--add",
                    &format!("includeIf.gitdir:{git_dir}.path"),
                    &settings,
                ],
            );
        }
        "gitdir-suffix" => {
            git_in(
                repo,
                &[
                    "config",
                    "--add",
                    "includeIf.gitdir:**/.git.path",
                    &settings,
                ],
            );
        }
        "worktree-relative" => {
            git_in(repo, &["config", "extensions.worktreeConfig", "true"]);
            fs::write(
                Path::new(&git_dir).join("config.worktree"),
                "[include]\n\tpath = autocrlf-wt.inc\n",
            )
            .expect("config.worktree");
            fs::write(
                Path::new(&git_dir).join("autocrlf-wt.inc"),
                "[core]\n\tautocrlf = true\n",
            )
            .expect("the relative include");
        }
        "hasconfig" => {
            git_in(
                repo,
                &["remote", "add", "origin", "https://example.invalid/r.git"],
            );
            git_in(
                repo,
                &[
                    "config",
                    "--add",
                    "includeIf.hasconfig:remote.*.url:https://example.invalid/**.path",
                    &settings,
                ],
            );
        }
        _ => {
            let checkout = git_in(repo, &["rev-parse", "--show-toplevel"])
                .trim()
                .to_owned();
            git_in(
                repo,
                &[
                    "config",
                    "--add",
                    &format!("includeIf.gitdir:{checkout}/.upstroke/.path"),
                    &settings,
                ],
            );
        }
    }
}

fn a_condition_chooses_the_configuration(condition: &str, turns_it_on: bool) {
    let (_tree, repo) = rd1_repo(&format!("rd3-rd2-3-{condition}"));
    autocrlf_through(&repo, condition);
    let pin = a_pinned_refusal(&repo, shaping(crlf_output));
    let effective = git_in(&repo, &["config", "--get", "core.autocrlf"])
        .trim()
        .to_owned();
    assert_eq!(
        effective == "true",
        turns_it_on,
        "{condition}: the checkout's autocrlf is {effective}"
    );
    let stored: &[u8] = if turns_it_on {
        b"paid output\n"
    } else {
        b"paid output\r\n"
    };
    assert_eq!(
        blob_at(&repo, &pin, "tracked.txt"),
        Some(stored.to_vec()),
        "{condition}: the coordinator's pin, as the checkout's Git stores it"
    );
    resume_and_die_inside_the_attempt(&repo);
    let report = bounded_resume(condition, &repo, fake(Effect::EditFile))
        .unwrap_or_else(|error| panic!("{condition}: the second resume: {error:?}"));
    assert!(committed(&report, "t1"), "{condition}: {report:?}");
    assert_eq!(
        blob_at(&repo, &pin, "tracked.txt"),
        Some(stored.to_vec()),
        "{condition}: the pin holds the output"
    );
    assert_eq!(status_of(&repo), "", "{condition}: the checkout is clean");
}

#[test]
fn a_branch_conditioned_configuration_is_the_captures_configuration() {
    a_condition_chooses_the_configuration("onbranch", true);
}

#[test]
fn an_exact_gitdir_conditioned_configuration_is_the_captures_configuration() {
    a_condition_chooses_the_configuration("gitdir-exact", true);
}

#[test]
fn a_suffix_gitdir_conditioned_configuration_is_the_captures_configuration() {
    a_condition_chooses_the_configuration("gitdir-suffix", true);
}

#[test]
fn a_relative_include_in_the_worktree_configuration_is_the_captures_configuration() {
    a_condition_chooses_the_configuration("worktree-relative", true);
}

#[test]
fn a_remote_conditioned_configuration_is_read_alike() {
    a_condition_chooses_the_configuration("hasconfig", true);
}

#[test]
fn a_condition_only_a_private_directory_meets_changes_no_byte() {
    let (_tree, repo) = rd1_repo("rd3-rd2-3p");
    autocrlf_through(&repo, "private-only");
    assert_ne!(
        git_in(&repo, &["config", "--get", "core.autocrlf"]).trim(),
        "true",
        "the checkout itself does not meet the condition"
    );
    die_inside_an_attempt(&repo, "crlf");
    let report = bounded_resume("the resume", &repo, fake(Effect::EditFile)).expect("the resume");
    assert!(committed(&report, "t1"), "{report:?}");
    assert_eq!(
        blob_at(&repo, &kept_pin_of(&repo, 1), "tracked.txt"),
        Some(b"paid output\r\n".to_vec()),
        "the resume's pin holds the CRLF bytes exactly"
    );
}

fn status_reads(repo: &Path) -> bool {
    let status = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["status", "--porcelain"])
        .output()
        .expect("run git status");
    let listed = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["ls-files"])
        .output()
        .expect("run git ls-files");
    status.status.success() && listed.status.success()
}

fn split_the_checkouts_index(repo: &Path, through_config: bool) {
    if through_config {
        git_in(repo, &["config", "core.splitIndex", "true"]);
    }
    git_in(repo, &["config", "splitIndex.sharedIndexExpire", "now"]);
    git_in(repo, &["update-index", "--split-index"]);
    assert!(
        !shared_indexes_of(repo).is_empty(),
        "the checkout's index is split"
    );
}

#[test]
fn a_capture_leaves_a_split_index_readable_and_pins() {
    let (_tree, repo) = rd1_repo("rd4-rd3-1");
    split_the_checkouts_index(&repo, true);
    let run = bounded_run(
        "N1's arm on a split index",
        &repo,
        Some(record_shared_indexes_and_fail_after_capture),
        fake(Effect::EditFile),
    );
    let after = shared_indexes_of(&repo).join("\n");
    let at_the_arm = fs::read_to_string(shared_indexes_marker(&repo)).expect("the arm's record");
    let Err(UpstrokeError::WithWarnings(warned)) = &run else {
        panic!("the attempt's error, with the kept note: {run:?}");
    };
    assert!(
        warned
            .warnings
            .iter()
            .any(|warning| warning.contains("is kept in this checkout and pinned at")),
        "{run:?}"
    );
    assert_eq!(
        after, at_the_arm,
        "the arm adds and removes no shared index"
    );
    assert!(
        status_reads(&repo),
        "git ls-files and git status read the checkout's index"
    );
    let commit = pin_target(&repo, &kept_pin_of(&repo, 1)).expect("the kept pin");
    assert_eq!(
        blob_at(&repo, &commit, "agent-output.txt"),
        Some(b"edited: 0\n".to_vec()),
        "the output is pinned"
    );
    let report =
        bounded_resume("the resume", &repo, fake(Effect::EditFile)).expect("a resume completes");
    assert!(committed(&report, "t1"), "{report:?}");
    assert!(status_reads(&repo));
}

#[test]
fn a_resume_capture_leaves_a_split_index_readable() {
    let (_tree, repo) = rd1_repo("rd4-rd3-1g");
    split_the_checkouts_index(&repo, true);
    a_pinned_refusal(&repo, fake(Effect::EditFile));
    let report =
        bounded_resume("the resume", &repo, fake(Effect::EditFile)).expect("the resume completes");
    assert!(committed(&report, "t1"), "{report:?}");
    assert!(status_reads(&repo), "git status reads the checkout's index");
}

#[test]
fn an_inherited_split_index_variable_expires_nothing() {
    let (_tree, repo) = rd1_repo("rd4-rd3-1e");
    split_the_checkouts_index(&repo, false);
    let forced = std::ffi::OsStr::new("1");
    let child = child_engine(&repo, "rd3-1e-run");
    let report = child.run(
        "N1's arm under an inherited GIT_TEST_SPLIT_INDEX",
        &repo,
        "kept_engine_child_runs_and_fails_after_capture",
        &[("GIT_TEST_SPLIT_INDEX", forced)],
    );
    assert!(
        report.contains("is kept in this checkout and pinned at"),
        "{report}"
    );
    assert!(status_reads(&repo), "the checkout's index stays readable");
    let pin = kept_pin_of(&repo, 1);
    assert!(
        blob_at(&repo, &pin, "agent-output.txt").is_some(),
        "and the output pinned"
    );
    let child = child_engine(&repo, "rd3-1e-resume");
    let report = child.run(
        "the resume's capture under an inherited GIT_TEST_SPLIT_INDEX",
        &repo,
        "kept_engine_child_resumes",
        &[("GIT_TEST_SPLIT_INDEX", forced)],
    );
    assert!(report.starts_with("ok"), "{report}");
    assert!(status_reads(&repo), "and after the resume's capture too");
}

#[cfg(unix)]
#[test]
fn the_discard_never_queries_the_checkouts_fsmonitor() {
    use std::os::unix::fs::PermissionsExt;
    let (_tree, repo) = rd1_repo("rd4-rd3-1f");
    let hook = repo.with_file_name("fsmonitor-hook.sh");
    let log = repo.with_file_name("fsmonitor-hook.log");
    fs::write(
        &hook,
        format!(
            "#!/bin/sh\necho \"$(pwd -P) $*\" >> '{}'\nprintf 'upstroke-%s\\0/\\0' \"$(date +%s)\"\n",
            log.display()
        ),
    )
    .expect("the hook");
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).expect("an executable hook");
    git_in(
        &repo,
        &["config", "core.fsmonitor", &hook.to_string_lossy()],
    );
    git_in(&repo, &["config", "core.fsmonitorHookVersion", "2"]);
    git_in(&repo, &["status", "--porcelain"]);
    let checkout = fs::canonicalize(&repo)
        .expect("the checkout")
        .display()
        .to_string();
    let calls_on_the_checkout = || {
        fs::read_to_string(&log)
            .unwrap_or_default()
            .lines()
            .filter(|line| line.split(' ').next() == Some(checkout.as_str()))
            .count()
    };
    let by_the_user = calls_on_the_checkout();
    assert!(by_the_user > 0, "the hook is live for the checkout");
    a_pinned_refusal(&repo, fake(Effect::EditFile));
    let report =
        bounded_resume("the resume", &repo, fake(Effect::EditFile)).expect("the resume completes");
    assert!(committed(&report, "t1"), "{report:?}");
    assert_eq!(
        calls_on_the_checkout(),
        by_the_user,
        "no engine child queried the checkout's fsmonitor"
    );
}

#[cfg(unix)]
fn listing_at_the_first_revert(repo: &Path, label: &str) -> (GitShim, PathBuf) {
    let run_id = rundir::latest_run(repo).expect("the run");
    let public = paths_of(repo, &run_id).public;
    let listing = repo.with_file_name(format!("{label}-listing.txt"));
    let shim = git_shim(
        repo,
        label,
        &[ShimRule {
            subcommand: "read-tree",
            holding: " -m ",
            after: false,
            action: format!("  ls '{}' > '{}'", public.display(), listing.display()),
        }],
    );
    (shim, listing)
}

#[cfg(unix)]
fn copies_listed(listing: &Path) -> (Vec<String>, Vec<String>) {
    let text = fs::read_to_string(listing).expect("the listing at the first read-tree -m");
    let copies = text
        .lines()
        .filter(|name| name.starts_with("kept-0-1-") && name.ends_with(".bundle"))
        .map(str::to_owned)
        .collect();
    let partials = text
        .lines()
        .filter(|name| name.ends_with(".partial"))
        .map(str::to_owned)
        .collect();
    (copies, partials)
}

#[cfg(unix)]
#[test]
fn a_resume_writes_and_syncs_its_own_copy_before_the_revert() {
    let (_tree, repo) = rd1_repo("rd4-rd3-2");
    let pin = a_pinned_refusal(&repo, fake(Effect::EditFile));
    let planted = plant_a_copy(&repo, &pin, ".bundle");
    let planted_bytes = sha256_of(&planted);
    let planted_name = planted
        .file_name()
        .and_then(|name| name.to_str())
        .expect("a name")
        .to_owned();
    let (shim, listing) = listing_at_the_first_revert(&repo, "rd3-2");
    let child = child_engine(&repo, "rd3-2");
    let report = child.run(
        "the resume",
        &repo,
        "kept_engine_child_resumes",
        &[("PATH", &shim.path())],
    );
    assert!(report.starts_with("ok"), "{report}");
    assert!(shim.fired(0), "the revert ran");
    let (copies, partials) = copies_listed(&listing);
    let [first, second] = copies.as_slice() else {
        panic!("at the first read-tree -m, the planted copy and one more: {copies:?}");
    };
    assert_eq!(first, &planted_name, "the planted copy");
    assert!(
        second > first,
        "a second copy, under a newer name: {copies:?}"
    );
    assert!(
        partials.is_empty(),
        "renamed into place before the revert: {partials:?}"
    );
    assert_eq!(
        sha256_of(&planted),
        planted_bytes,
        "the planted copy is byte-identical"
    );
    let fresh = planted.with_file_name(second);
    let heads = git_in(&repo, &["bundle", "list-heads", &fresh.to_string_lossy()]);
    assert!(
        heads.trim().ends_with(&pin),
        "the new copy names the pin: {heads}"
    );
}

#[cfg(unix)]
#[test]
fn a_pin_restored_from_a_copy_is_copied_afresh_before_the_revert() {
    let (_tree, repo) = rd1_repo("rd4-rd3-2r");
    let pin = a_pinned_refusal(&repo, fake(Effect::EditFile));
    let planted = plant_a_copy(&repo, &pin, ".bundle");
    git_in(&repo, &["update-ref", "-d", &pin]);
    let (shim, listing) = listing_at_the_first_revert(&repo, "rd3-2r");
    let child = child_engine(&repo, "rd3-2r");
    let report = child.run(
        "the resume",
        &repo,
        "kept_engine_child_resumes",
        &[("PATH", &shim.path())],
    );
    assert!(report.starts_with("ok"), "{report}");
    let (copies, _) = copies_listed(&listing);
    assert_eq!(
        copies.len(),
        2,
        "the new copy exists at the first read-tree -m: {copies:?}"
    );
    let restored = report
        .lines()
        .find(|line| line.contains("was put back from its copy"))
        .unwrap_or_else(|| panic!("the restored warning: {report}"));
    assert!(
        restored.contains(&planted.display().to_string()),
        "{restored}"
    );
    let fresh = copies.iter().max().expect("the newer copy");
    let copied = report
        .lines()
        .find(|line| line.contains("copied outside every ref"))
        .unwrap_or_else(|| panic!("the copy warning: {report}"));
    assert!(
        copied.contains(fresh.as_str()),
        "the copy warning names the new copy: {copied}"
    );
    assert!(pin_target(&repo, &pin).is_some(), "the pin is back");
}

#[cfg(unix)]
#[test]
fn a_resume_that_cannot_write_its_own_copy_refuses_and_discards_nothing() {
    let (_tree, repo) = rd1_repo("rd4-rd3-2p");
    let pin = a_pinned_refusal(&repo, fake(Effect::EditFile));
    plant_a_copy(&repo, &pin, ".bundle");
    let run_id = rundir::latest_run(&repo).expect("the run");
    let public = paths_of(&repo, &run_id).public;
    let before = status_of(&repo);
    mode_of(&public, 0o555).expect("a run directory that takes no new file");
    let binds = mode_bits_bind(&public);
    let first = if binds {
        Some(bounded_resume("the resume", &repo, fake(Effect::EditFile)))
    } else {
        None
    };
    mode_of(&public, 0o755).expect("writable again");
    let first = first.expect("prerequisite not met: the mode bit did not bind (root)");
    let refusal = resume_refusal(&first, "the resume");
    assert!(
        refusal.contains("keeping them before the discard stopped") && refusal.contains("bundle"),
        "the copy's own failure: {refusal}"
    );
    assert_eq!(status_of(&repo), before, "nothing discarded");
    let report =
        bounded_resume("the next resume", &repo, fake(Effect::EditFile)).expect("it completes");
    assert!(committed(&report, "t1"), "{report:?}");
}

struct UnparsedWorker {
    inner: FakeAdapter,
}

impl AgentAdapter for UnparsedWorker {
    fn id(&self) -> &'static str {
        self.inner.id()
    }

    fn probe(&self, runner: &dyn crate::runner::Runner) -> Result<Caps, UpstrokeError> {
        self.inner.probe(runner)
    }

    fn build(&self, run: &TaskRun) -> Result<CommandSpec, UpstrokeError> {
        if run.profile.permissions != PermissionMode::ReadOnly {
            paid_output(&run.workspace).map_err(after_capture_failed)?;
        }
        self.inner.build(run)
    }

    fn parse(&self, out: &ProcessOutput) -> Result<Outcome, UpstrokeError> {
        if out.stdout.contains(REVIEW_MARKER) {
            return self.inner.parse(out);
        }
        Err(UpstrokeError::Agent {
            message: "the worker's output could not be parsed".to_owned(),
        })
    }

    fn materialize_permissions(
        &self,
        profile: &WorkerProfile,
        gate_cmds: &[String],
        dir: &Path,
        stem: &str,
    ) -> Result<Option<PathBuf>, UpstrokeError> {
        self.inner
            .materialize_permissions(profile, gate_cmds, dir, stem)
    }
}

#[test]
fn an_error_after_the_worker_wrote_and_before_its_capture_keeps_the_output_pinned() {
    let (_tree, repo) = rd1_repo("rd2-n1d");
    let run = bounded_run(
        "the run whose worker's output cannot be parsed",
        &repo,
        None,
        OneAdapter {
            adapter: UnparsedWorker {
                inner: FakeAdapter::new(vec![Effect::NoEdit], vec![ReviewBehavior::Pass]),
            },
        },
    );
    let Err(UpstrokeError::WithWarnings(warned)) = &run else {
        panic!("the parse's own error, with the kept note: {run:?}");
    };
    assert!(
        warned.error.to_string().contains("could not be parsed"),
        "{run:?}"
    );
    assert!(
        warned
            .warnings
            .iter()
            .any(|warning| warning.contains("is kept in this checkout and pinned at")),
        "{run:?}"
    );
    assert_eq!(
        file_bytes(&repo, "agent-output.txt"),
        Some(b"paid output\n".to_vec()),
        "the checkout keeps the output"
    );
    let commit = pin_target(&repo, &kept_pin_of(&repo, 1)).expect("the kept pin");
    assert_eq!(
        blob_at(&repo, &commit, "agent-output.txt"),
        Some(b"paid output\n".to_vec()),
        "and the pin holds it"
    );
}

#[test]
fn an_earlier_attempts_retained_output_is_kept_when_the_next_worker_cannot_start() {
    let (_tree, repo) = temp_engine_repo("rd2-n1e");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 2 }\n"),
    );
    let run = bounded_run(
        "the same-session retry whose worker cannot start",
        &repo,
        None,
        source(
            vec![Effect::EditFile, Effect::SpawnError],
            vec![ReviewBehavior::Fail],
        ),
    );
    let Err(UpstrokeError::WithWarnings(warned)) = &run else {
        panic!("the spawn's own error, with the kept note: {run:?}");
    };
    assert!(
        warned
            .warnings
            .iter()
            .any(|warning| warning
                .contains("attempt 2 of `t1` is kept in this checkout and pinned at")),
        "{run:?}"
    );
    let commit = pin_target(&repo, &kept_pin_of(&repo, 2)).expect("attempt 2's kept pin");
    assert_eq!(
        blob_at(&repo, &commit, "agent-output.txt"),
        Some(b"edited: 0\n".to_vec()),
        "it holds the output attempt 1 left for the retry"
    );
}

#[cfg(unix)]
fn modes_under(directory: &Path, mode: u32) -> std::io::Result<()> {
    mode_of(directory, mode)?;
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_dir() && !path.is_symlink() {
            modes_under(&path, mode)?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn make_the_objects_read_only_and_tear_after_capture(
    workspace: &Workspace,
    candidate: &crate::workspace::CapturedCandidate,
) -> Result<(), UpstrokeError> {
    tear_after_capture(workspace, candidate)?;
    modes_under(&workspace.root().join(".git").join("objects"), 0o555).map_err(after_capture_failed)
}

#[cfg(unix)]
fn make_the_reflogs_read_only_and_tear_after_capture(
    workspace: &Workspace,
    candidate: &crate::workspace::CapturedCandidate,
) -> Result<(), UpstrokeError> {
    tear_after_capture(workspace, candidate)?;
    git_after_capture(
        workspace.root(),
        &["config", "core.logAllRefUpdates", "always"],
    )?;
    let logs = workspace.root().join(".git").join("logs");
    fs::create_dir_all(&logs).map_err(after_capture_failed)?;
    modes_under(&logs, 0o555).map_err(after_capture_failed)
}

#[cfg(unix)]
#[test]
fn a_store_fault_at_the_pins_objects_or_its_reflog_loses_nothing() {
    let faults: [(&str, super::options::AfterCandidateCapture, &str); 2] = [
        (
            "objects",
            make_the_objects_read_only_and_tear_after_capture,
            "objects",
        ),
        (
            "reflog",
            make_the_reflogs_read_only_and_tear_after_capture,
            "logs",
        ),
    ];
    for (fault, after_capture, directory) in faults {
        let (_tree, repo) = rd1_repo(&format!("rd1-b-{fault}"));
        let run = bounded_run(fault, &repo, Some(after_capture), fake(Effect::EditFile));
        let store = repo.join(".git").join(directory);
        let binds = mode_bits_bind(&store);
        modes_under(&store, 0o755).expect("the fault healed");
        assert!(
            binds,
            "{fault}: prerequisite not met: the mode bit did not bind (root, or CAP_DAC_OVERRIDE)"
        );
        let ended = format!("{run:?}");
        assert!(
            run.is_err() && ended.contains("pinning it at") && ended.contains("failed"),
            "{fault}: the run ends with the checkout kept and the pin failed: {ended}"
        );
        assert_eq!(
            kept_pins(&repo),
            Vec::<(String, String)>::new(),
            "{fault}: no pin"
        );
        assert!(
            status_of(&repo).contains("agent-output.txt"),
            "{fault}: nothing discarded"
        );
        repair_the_torn_registration(&repo);
        let report = bounded_resume(fault, &repo, fake(Effect::EditFile))
            .unwrap_or_else(|error| panic!("{fault}: the resume once healed: {error:?}"));
        assert!(committed(&report, "t1"), "{fault}: {report:?}");
        let (_, _, tree) = captured(&repo);
        let commit = pin_target(&repo, &kept_pin_of(&repo, 1))
            .unwrap_or_else(|| panic!("{fault}: the resume wrote the kept pin"));
        assert_eq!(tree_of(&repo, &commit), tree, "{fault}: holding the output");
    }
}

fn every_kind_of_change(root: &Path) -> std::io::Result<()> {
    write_into(root, "tracked.txt", b"paid edit\n")?;
    fs::remove_file(root.join("deleted.txt"))?;
    write_into(root, "new.txt", b"paid new file\n")?;
    write_into(root, "newdir/inner.txt", b"paid, in a new directory\n")?;
    write_into(
        root,
        "ignored.flag",
        b"ignored, never captured, never discarded\n",
    )?;
    #[cfg(unix)]
    {
        mode_of(&root.join("sub").join("t.txt"), 0o755)?;
        std::os::unix::fs::symlink("tracked.txt", root.join("link"))?;
    }
    Ok(())
}

#[test]
fn every_kind_of_change_is_pinned_and_an_ignored_file_survives() {
    let (_tree, repo) = rd1_repo("rd1-x4");
    die_inside_an_attempt(&repo, "x4");
    let report = bounded_resume("the resume", &repo, fake(Effect::EditFile)).expect("the resume");
    assert!(committed(&report, "t1"), "{report:?}");
    assert_eq!(
        file_bytes(&repo, "ignored.flag"),
        Some(b"ignored, never captured, never discarded\n".to_vec()),
        "the ignored file survives the discard"
    );
    let pin = kept_pin_of(&repo, 1);
    for (path, bytes) in [
        ("tracked.txt", &b"paid edit\n"[..]),
        ("new.txt", b"paid new file\n"),
        ("newdir/inner.txt", b"paid, in a new directory\n"),
    ] {
        assert_eq!(blob_at(&repo, &pin, path), Some(bytes.to_vec()), "{path}");
    }
    assert!(
        blob_at(&repo, &pin, "deleted.txt").is_none(),
        "the deletion"
    );
    assert!(
        blob_at(&repo, &pin, "ignored.flag").is_none(),
        "no ignored file"
    );
    #[cfg(unix)]
    {
        let listing = recorded(&repo, &["ls-tree", "-r", "--full-tree", &pin]);
        assert!(
            listing
                .lines()
                .any(|line| line.starts_with("100755 ") && line.ends_with("\tsub/t.txt")),
            "the executable bit: {listing}"
        );
        assert!(
            listing
                .lines()
                .any(|line| line.starts_with("120000 ") && line.ends_with("\tlink")),
            "the symbolic link: {listing}"
        );
    }
    let warning = kept_pin_warning(&report).expect("the resume names the pin");
    follow_the_restore(&repo, warning, &pin);
    let differences = recorded(&repo, &["diff", "--name-status", &pin, "--"]);
    assert_eq!(
        differences, "",
        "the restore reproduces the pin, modes included"
    );
}

#[cfg(target_os = "linux")]
fn a_name_that_is_not_utf8(root: &Path) -> std::io::Result<()> {
    use std::os::unix::ffi::OsStrExt;
    paid_output(root)?;
    fs::write(
        root.join(std::ffi::OsStr::from_bytes(b"caf\xe9.txt")),
        b"paid, under a Latin-1 name\n",
    )
}

#[cfg(target_os = "linux")]
#[test]
fn a_leftover_whose_name_is_not_utf8_is_pinned_then_reverted() {
    use std::os::unix::ffi::OsStrExt;
    let (_tree, repo) = rd1_repo("rd1-x17");
    die_inside_an_attempt(&repo, "x17");
    let report = bounded_resume("the resume", &repo, fake(Effect::EditFile)).expect("the resume");
    assert!(committed(&report, "t1"), "{report:?}");
    assert!(
        !repo
            .join(std::ffi::OsStr::from_bytes(b"caf\xe9.txt"))
            .exists(),
        "reverted"
    );
    let pin = kept_pin_of(&repo, 1);
    let out = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .args(["ls-tree", "-r", "-z", "--name-only", &pin])
        .output()
        .expect("run git ls-tree");
    assert!(
        out.stdout
            .split(|byte| *byte == 0)
            .any(|name| name == b"caf\xe9.txt"),
        "the pin holds the name's own bytes"
    );
}

#[cfg(unix)]
#[test]
fn a_resume_that_dies_before_its_revert_is_finished_by_the_next() {
    let (_tree, repo) = rd1_repo("rd1-x6");
    let pin = a_pinned_refusal(&repo, fake(Effect::EditFile));
    let (_, _, tree) = captured(&repo);
    let shim = git_shim(
        &repo,
        "x6",
        &[ShimRule {
            subcommand: "read-tree",
            holding: " -m ",
            after: false,
            action: "  kill -KILL $PPID\n  exit 1".to_owned(),
        }],
    );
    let child = child_engine(&repo, "x6");
    let mut command = this_test_binary("kept_engine_child_resumes");
    command
        .env("UPSTROKE_KEPT_ENGINE_REPO", &repo)
        .env("UPSTROKE_KEPT_ENGINE_REPORT", &child.report)
        .env("PATH", shim.path());
    let (status, log) = bounded_child(
        "the resume that dies before its revert",
        command,
        &child.log,
    );
    assert!(
        shim.fired(0),
        "the resume died at its revert's first child: {log}"
    );
    assert!(status.code().is_none(), "by a signal: {status:?}");
    assert_eq!(kept_copies(&repo).len(), 1, "after its copy");
    assert!(
        status_of(&repo).contains("agent-output.txt"),
        "nothing reverted"
    );
    let report = bounded_resume("the next resume", &repo, fake(Effect::EditFile))
        .expect("the next resume completes");
    assert!(committed(&report, "t1"), "{report:?}");
    assert_eq!(
        kept_copies(&repo).len(),
        2,
        "it wrote a fresh copy of its own before its revert"
    );
    assert_eq!(
        tree_of(&repo, &pin_target(&repo, &pin).expect("the pin")),
        tree,
        "the pin holds the output"
    );
}

#[cfg(unix)]
#[test]
fn a_hostile_repository_keeps_its_index_readable_and_its_monitor_unqueried() {
    use std::os::unix::fs::PermissionsExt;
    let (_tree, repo) = rd1_repo("rd4-hostile");
    let hook = repo.with_file_name("fsmonitor-hook.sh");
    let log = repo.with_file_name("fsmonitor-hook.log");
    fs::write(
        &hook,
        format!(
            "#!/bin/sh\necho \"$(pwd -P) $*\" >> '{}'\nprintf 'upstroke-%s\\0/\\0' \"$(date +%s)\"\n",
            log.display()
        ),
    )
    .expect("the hook");
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).expect("an executable hook");
    for (key, value) in [
        ("core.untrackedCache", "true"),
        ("core.fsmonitor", &*hook.to_string_lossy()),
        ("core.fsmonitorHookVersion", "2"),
    ] {
        git_in(&repo, &["config", key, value]);
    }
    split_the_checkouts_index(&repo, true);
    git_in(&repo, &["status", "--porcelain"]);
    let checkout = fs::canonicalize(&repo)
        .expect("the checkout")
        .display()
        .to_string();
    let calls_on_the_checkout = || {
        fs::read_to_string(&log)
            .unwrap_or_default()
            .lines()
            .filter(|line| line.split(' ').next() == Some(checkout.as_str()))
            .count()
    };
    let by_the_user = calls_on_the_checkout();
    assert!(by_the_user > 0, "the hook is live for the checkout");
    let run = bounded_run(
        "N1's arm in the hostile repository",
        &repo,
        Some(record_shared_indexes_and_fail_after_capture),
        fake(Effect::EditFile),
    );
    let during_the_run = calls_on_the_checkout() - by_the_user;
    let after = shared_indexes_of(&repo).join("\n");
    let at_the_arm = fs::read_to_string(shared_indexes_marker(&repo)).expect("the arm's record");
    assert!(
        format!("{run:?}").contains("is kept in this checkout and pinned at"),
        "{run:?}"
    );
    assert_eq!(
        after, at_the_arm,
        "the pin adds and removes no shared index"
    );
    assert!(status_reads(&repo), "the index reads after the pin");
    let before_the_resume = calls_on_the_checkout();
    let report =
        bounded_resume("the resume", &repo, fake(Effect::EditFile)).expect("the resume completes");
    let during_the_resume = calls_on_the_checkout() - before_the_resume;
    assert!(committed(&report, "t1"), "{report:?}");
    assert!(status_reads(&repo), "and after the resume");
    assert_eq!(
        (during_the_run, during_the_resume),
        (0, 0),
        "no engine child queried the checkout's fsmonitor"
    );
}
