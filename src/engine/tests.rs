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

#[cfg(test)]
mod capacity_budget;
#[cfg(test)]
mod kept_output;
#[cfg(test)]
mod legacy_resume;
#[cfg(test)]
mod questions;
#[cfg(test)]
mod snapshot_registration;
#[cfg(test)]
mod topology_admission;
#[cfg(test)]
mod topology_execution;
#[cfg(test)]
mod verification;

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

    worker_rate_limit: &'static str,

    worker_rate_limits: Vec<&'static str>,

    worker_limit_on_stderr: Option<&'static str>,

    worker_limit_result: &'static str,

    review_limit_on_stderr: Option<&'static str>,

    review_limit_result: &'static str,
    calls: Mutex<Calls>,
    role_probe: Option<RoleProbe>,
}

const FIVE_HOUR_LIMIT_REACHED: &str = "5-hour limit reached";

const CLAUDE_REQUEST_FAILED: &str =
    r#"{"type":"result","is_error":true,"result":"request failed","session_id":"s-limit"}"#;

const CLAUDE_REVIEW_REQUEST_FAILED: &str = r#"{"type":"result","is_error":true,"result":"request failed","session_id":"fake-review-session","fake":"UPSTROKE-FAKE-REVIEW"}"#;

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
            worker_rate_limit: FIVE_HOUR_LIMIT_REACHED,
            worker_rate_limits: Vec::new(),
            worker_limit_on_stderr: None,
            worker_limit_result: CLAUDE_REQUEST_FAILED,
            review_limit_on_stderr: None,
            review_limit_result: CLAUDE_REVIEW_REQUEST_FAILED,
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
            worker_rate_limit: FIVE_HOUR_LIMIT_REACHED,
            worker_rate_limits: Vec::new(),
            worker_limit_on_stderr: None,
            worker_limit_result: CLAUDE_REQUEST_FAILED,
            review_limit_on_stderr: None,
            review_limit_result: CLAUDE_REVIEW_REQUEST_FAILED,
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
            if let (ReviewBehavior::RateLimited, Some(limit)) =
                (behavior, self.review_limit_on_stderr)
            {
                return Ok(limit_on_stderr_spec(self.review_limit_result, limit));
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
        if let (Effect::RateLimited, Some(limit)) = (
            scripted(&self.effects, index, Effect::EditFile),
            self.worker_limit_on_stderr,
        ) {
            return Ok(limit_on_stderr_spec(self.worker_limit_result, limit));
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
            if behavior == ReviewBehavior::RateLimited && self.review_limit_on_stderr.is_some() {
                return crate::agent::claude::ClaudeCodeAdapter.parse(out);
            }
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
        if effect == Effect::RateLimited && self.worker_limit_on_stderr.is_some() {
            return crate::agent::claude::ClaudeCodeAdapter.parse(out);
        }
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
            Effect::RateLimited => {
                Some(scripted(&self.worker_rate_limits, index, self.worker_rate_limit).to_owned())
            }
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

fn limit_on_stderr_spec(result: &str, limit: &str) -> CommandSpec {
    // The JSON result on stdout, the limit on stderr, then a failing
    // exit, spelled with what both shells `shell_spec` picks share:
    // `echo`, `>&2` and `&&`. The `printf '…'; …` spelling this
    // replaces was `sh`'s alone, and under `cmd /C` the missing
    // `printf` made the attempt an agent error that parked the task.
    // There is no blank before `>&2` or `&&`, so `cmd` writes no
    // trailing one; its CRLF line ends change neither the JSON nor
    // the reset the parser reads. `echo_operand` says why each text
    // is still quoted per shell.
    shell_spec(&format!(
        "echo {}&& echo {}>&2&& exit 1",
        echo_operand(result),
        echo_operand(limit)
    ))
}

/// `text` as one operand of `echo` in the shell `shell_spec` picks, so that
/// `cmd /C` and `sh -c` print the same line. `cmd`'s `echo` prints every
/// character it is given, quotes included, and reads none of `sh`'s escapes,
/// so `cmd` gets the text bare. `sh` reads a bare `"` as quoting, `(` and `)`
/// as syntax and `{a,b}` as a brace expansion, so it gets the text in double
/// quotes with each `"` escaped. A JSON result needs its quotes and a limit
/// message its ` (UTC)`, so no one spelling serves both. The refused
/// characters are the ones that would still differ: `cmd` reads `^ & | < > %`
/// even in a bare `echo`, `sh` expands `$` and `` ` `` inside double quotes,
/// its `echo` may read `\` as an escape, and `cmd` writes in the console's
/// code page rather than UTF-8.
fn echo_operand(text: &str) -> String {
    assert!(
        text.is_ascii() && !text.contains(['^', '&', '|', '<', '>', '%', '$', '`', '\\']),
        "{text:?} holds a character `cmd` and `sh` would not echo alike"
    );
    if matches!(
        crate::gates::ShellKind::native(),
        crate::gates::ShellKind::Cmd
    ) {
        text.to_owned()
    } else {
        format!("\"{}\"", text.replace('"', "\\\""))
    }
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
    git_in(&dir, &["config", "maintenance.auto", "false"]);
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
    opts.legacy_writer = true;
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

fn worktree_lock_path(repo: &Path) -> PathBuf {
    Workspace::open(repo)
        .expect("workspace")
        .worktree_git_dir()
        .expect("worktree git dir")
        .join("upstroke-worktree.lock")
}

fn replay_of(repo: &Path, run_id: &str) -> crate::status::Status {
    crate::status::load(repo, Some(run_id)).expect("the run reads back")
}

fn legacy_status(status: &crate::status::Status) -> &crate::status::RunStatus {
    match status {
        crate::status::Status::Legacy(run) => run.as_ref(),
        crate::status::Status::Topology(run) => {
            panic!(
                "run `{}` read back as schema 4, not as a legacy run",
                run.run_id
            )
        }
    }
}

fn assert_live_equals_replay(repo: &Path, live: &RunState, report: &RunReport) {
    let replayed = replay_of(repo, &report.run_id);
    assert_eq!(
        &legacy_status(&replayed).state,
        live,
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
        strip(&legacy_status(&replayed).report()),
        strip(report),
        "the report derived from the log differs from the one the run wrote"
    );
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

fn events_of(repo: &Path, run_id: &str) -> Vec<events::Event> {
    let mut ignored = Vec::new();
    events::read_all(&paths_of(repo, run_id).events(), &mut ignored).expect("the log reads")
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

const PARKING_SETTLEMENT_KILL_CHILD: &str = "engine::tests::parking_settlement_kill_child";

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
    let [record] = legacy_status(&replayed).state.questions.as_slice() else {
        panic!(
            "the parking settlement records one question: {:?}",
            legacy_status(&replayed).state.questions
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
    let [record] = legacy_status(&replayed).state.questions.as_slice() else {
        panic!(
            "{tag}: the settlement parks one question: {:?}",
            legacy_status(&replayed).state.questions
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

#[cfg(unix)]
fn write_the_gate_probe_stand_in(git: &Path, script: &str) {
    use std::os::unix::fs::PermissionsExt;

    crate::runner::container::write_program_in_its_own_process(git, script);
    fs::set_permissions(git, fs::Permissions::from_mode(0o700)).expect("an executable Git");
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "subprocess helper; holds forked descriptors away from other tests"]
fn gate_probe_writer_helper() {
    let tree = temp_engine_scratch("gate-probe-writer");
    crate::runner::host::tests::inherited_writer::writes_leave_no_writer_in_another_threads_fork(
        &tree.path().join("gate probe fifo ' $name"),
        |fifo, marker| {
            write_the_gate_probe_stand_in(
                fifo,
                &crate::runner::host::tests::inherited_writer::shim_script(marker),
            );
        },
    );
}

#[cfg(unix)]
const GATE_PROBE_TREE: &str = "UPSTROKE_W924_GATE_PROBE_TREE";

#[cfg(unix)]
#[test]
#[ignore = "subprocess helper"]
fn the_gate_role_probe_cases_child() {
    let Some(tree) = std::env::var_os(GATE_PROBE_TREE).map(PathBuf::from) else {
        return;
    };
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
        let place = tree.join(case);
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
        write_the_gate_probe_stand_in(
            &git,
            &format!(
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
        );
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
    fs::write(tree.join("ran"), "").expect("the child's record that its cases ran");
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

fn fail_after_capture(
    workspace: &Workspace,
    candidate: &crate::workspace::CapturedCandidate,
) -> Result<(), UpstrokeError> {
    record_the_capture(workspace, candidate)?;
    Err(after_capture_failed(
        "an attempt error that is not the registry's",
    ))
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

fn warning_with<'a>(report: &'a RunReport, needle: &str) -> Option<&'a String> {
    report
        .warnings
        .iter()
        .find(|warning| warning.contains(needle))
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

fn exit_after_capture(
    _workspace: &Workspace,
    _candidate: &crate::workspace::CapturedCandidate,
) -> Result<(), UpstrokeError> {
    std::process::exit(CRASH_EXIT_CODE)
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

#[cfg(target_os = "linux")]
fn a_name_that_is_not_utf8(root: &Path) -> std::io::Result<()> {
    use std::os::unix::ffi::OsStrExt;
    paid_output(root)?;
    fs::write(
        root.join(std::ffi::OsStr::from_bytes(b"caf\xe9.txt")),
        b"paid, under a Latin-1 name\n",
    )
}

#[cfg(unix)]
const INHERITED_RUN_ROOT: &str = "UPSTROKE_INHERITED_RUN_ROOT";
#[cfg(unix)]
const INHERITED_RUN_PRIVATE: &str = "UPSTROKE_INHERITED_RUN_PRIVATE";
// Not `cfg(unix)`: the portable J7 test
// `a_schema_4_resume_runs_the_recorded_gates_and_names_how_todays_differ` seeds it too.
const WORKER_ONLY_PLAN: &str = "## Implement the widget\n<!-- upstroke: id=t1 depends= -->\n";
#[cfg(unix)]
const WORKER_ONLY_CONFIG: &str = "[routing]\nreview = { enabled = false }\n";

#[cfg(unix)]
fn copy_tree_for_a_layout(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("a copy's directory");
    for entry in fs::read_dir(from).expect("read a directory to copy") {
        let entry = entry.expect("a directory entry");
        let target = to.join(entry.file_name());
        let kind = entry.file_type().expect("an entry's type");
        if kind.is_dir() {
            copy_tree_for_a_layout(&entry.path(), &target);
        } else if kind.is_symlink() {
            let link = fs::read_link(entry.path()).expect("a link's target");
            std::os::unix::fs::symlink(link, &target).expect("copy a link");
        } else if let Err(error) = fs::copy(entry.path(), &target) {
            panic!(
                "copy a file: {} to {}, listed as {kind:?}, now {:?}: {error}",
                entry.path().display(),
                target.display(),
                fs::symlink_metadata(entry.path()).map(|metadata| metadata.file_type())
            );
        }
    }
}

#[cfg(unix)]
#[test]
#[ignore = "subprocess helper"]
fn inherited_selection_run_helper() {
    let Some(work) = std::env::var_os(INHERITED_RUN_ROOT).map(PathBuf::from) else {
        return;
    };
    seed(&work, WORKER_ONLY_PLAN, Some(WORKER_ONLY_CONFIG));
    let mut opts = options(&work);
    opts.config_path = Some(work.join("upstroke.toml"));
    let report = run_with(&opts, &fake(Effect::EditFile)).expect("the run");
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:?}");
    assert!(committed(&report, "t1"), "{report:?}");
    assert!(
        report.gates.is_empty(),
        "a worker-only task: {:?}",
        report.gates
    );
    let edited = git_in(
        &work,
        &["show", &format!("{}:agent-output.txt", report.branch)],
    );
    assert!(
        edited.contains("edited"),
        "B.git's run branch holds the worker's edit: {edited}"
    );
}

#[cfg(unix)]
#[test]
#[ignore = "subprocess helper"]
fn absolute_common_dir_run_helper() {
    gated_run_under_an_inherited_common_dir();
}

#[cfg(unix)]
fn gated_run_under_an_inherited_common_dir() {
    let Some(repo) = std::env::var_os(INHERITED_RUN_ROOT).map(PathBuf::from) else {
        return;
    };
    let private = PathBuf::from(std::env::var_os(INHERITED_RUN_PRIVATE).expect("the private root"));
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    opts.private_root = Some(private);
    let report = run_with(&opts, &fake(Effect::EditFile)).expect("the run");
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:?}");
    assert_eq!(report.gates, ["version"], "{report:?}");
    let edited = git_in(
        &repo,
        &["show", &format!("{}:agent-output.txt", report.branch)],
    );
    assert!(
        edited.contains("edited"),
        "the run branch holds the worker's file: {edited}"
    );
}

#[cfg(unix)]
#[test]
#[ignore = "subprocess helper"]
fn relative_common_dir_run_helper() {
    gated_run_under_an_inherited_common_dir();
}

// ===========================================================================
// PR12: a schema-4 run through the public entry points
// ===========================================================================
//
// The stub agent is a real CLI as far as the runner is concerned: every probe,
// worker and reviewer is a native-shell process the production `HostRunner`
// starts in the run's own worktrees. Nothing here replaces the runner, the
// workspace manager, the loop or the event log.

const TOPOLOGY_STUB: &str = "claude-code";
const TOPOLOGY_STUB_REVIEW: &str = "UPSTROKE-TOPOLOGY-STUB-REVIEW";
const TOPOLOGY_STUB_VERDICT: &str = "```json\n{\"pass\": true, \"reasons\": [\"meets the \
                                     acceptance criteria\"], \"required_changes\": []}\n```";
const TOPOLOGY_STUB_QUESTION: &str =
    "UPSTROKE-QUESTION: which of the two widget formats does the spec mean?";
const TOPOLOGY_STUB_ASKS: &str = "UPSTROKE-TOPOLOGY-STUB-ASKS";
/// The worker's edit: one line appended to `agent-output.txt`, spelled so that
/// both shells `shell_spec` picks run it alike, `cmd /C` on Windows and `sh -c`
/// elsewhere (`gates::ShellKind::native`). `echo` is a builtin of both and `>>`
/// appends in both; with no space before `>>`, `cmd` writes no trailing blank.
/// `cmd` ends the line with CRLF and `sh` with LF, and every assertion over the
/// file reads it by content. The `printf '…\n'` spelling this replaces was a
/// `sh` command: `cmd` does not quote with `'`, has no `printf` builtin, and
/// opens a redirection's file before it looks the program up.
const TOPOLOGY_STUB_EDIT: &str = "echo edited by the topology stub>> agent-output.txt";

struct TopologyStub {
    worker_script: String,
    questions_left: Mutex<usize>,
}

impl TopologyStub {
    fn new(worker_script: &str, questions: usize) -> Self {
        Self {
            worker_script: worker_script.to_owned(),
            questions_left: Mutex::new(questions),
        }
    }
}

impl AgentAdapter for TopologyStub {
    fn id(&self) -> &'static str {
        TOPOLOGY_STUB
    }

    fn probe(&self, runner: &dyn crate::runner::Runner) -> Result<Caps, UpstrokeError> {
        let request = crate::agent::probe_request(
            TOPOLOGY_STUB,
            shell_spec("exit 0"),
            0,
            Duration::from_secs(60),
        )?;
        let output = runner.run_blocking(&request)?;
        if output.code != Some(0) {
            return Err(UpstrokeError::Agent {
                message: format!("the topology stub's probe exited {:?}", output.code),
            });
        }
        Ok(Caps {
            version: "0.0.0-topology-stub".to_owned(),
            json_output: true,
            session_resume: false,
            cost_reporting: true,
            read_only_mode: true,
            acp: false,
            model_list: false,
        })
    }

    fn build(&self, run: &TaskRun) -> Result<CommandSpec, UpstrokeError> {
        if run.profile.permissions == PermissionMode::ReadOnly {
            return Ok(shell_spec(&format!("echo {TOPOLOGY_STUB_REVIEW}")));
        }
        Ok(shell_spec(&self.worker_script))
    }

    fn parse(&self, out: &ProcessOutput) -> Result<Outcome, UpstrokeError> {
        // Only a worker that exited cleanly completed, as a real adapter reads
        // its end (`agent::claude`'s `parse_output`): a timeout is a timeout,
        // and any other end is an agent error whose stderr the attempt reports.
        // Read as `Completed`, a failed edit was reported as an empty diff.
        if out.timed_out || out.output_limited || out.code != Some(0) {
            return Ok(Outcome {
                status: if out.timed_out {
                    OutcomeStatus::Timeout
                } else {
                    OutcomeStatus::AgentError
                },
                diff: String::new(),
                detail: None,
                session_id: None,
                usage: None,
                cost_usd: Some(0.01),
                transcript_path: PathBuf::new(),
                duration: out.duration,
            });
        }
        let detail = if out.stdout.contains(TOPOLOGY_STUB_REVIEW) {
            TOPOLOGY_STUB_VERDICT
        } else if out.stdout.contains(TOPOLOGY_STUB_ASKS) {
            TOPOLOGY_STUB_QUESTION
        } else {
            let mut left = self
                .questions_left
                .lock()
                .map_err(|_| UpstrokeError::Agent {
                    message: "topology stub lock poisoned".to_owned(),
                })?;
            if *left > 0 {
                *left -= 1;
                TOPOLOGY_STUB_QUESTION
            } else {
                "done"
            }
        };
        Ok(Outcome {
            status: OutcomeStatus::Completed,
            diff: String::new(),
            detail: Some(detail.to_owned()),
            session_id: None,
            usage: None,
            cost_usd: Some(0.01),
            transcript_path: PathBuf::new(),
            duration: out.duration,
        })
    }
}

/// The stub, shareable across the coordinator's pipeline threads.
struct SharedTopologyStubs(std::sync::Arc<TopologyStub>);

impl AdapterSource for SharedTopologyStubs {
    fn get(&self, id: &str) -> Option<&dyn AgentAdapter> {
        (id == TOPOLOGY_STUB).then_some(&*self.0 as &dyn AgentAdapter)
    }

    fn shared(&self) -> Option<std::sync::Arc<dyn AdapterSource + Send + Sync>> {
        Some(std::sync::Arc::new(Self(std::sync::Arc::clone(&self.0))))
    }
}

/// The same stub behind a source that cannot be shared: the step loop.
struct BorrowedTopologyStub<'a>(&'a TopologyStub);

impl AdapterSource for BorrowedTopologyStub<'_> {
    fn get(&self, id: &str) -> Option<&dyn AgentAdapter> {
        (id == TOPOLOGY_STUB).then_some(self.0 as &dyn AgentAdapter)
    }
}

fn topology_options(repo: &Path) -> RunOptions {
    let mut opts = options(repo);
    opts.legacy_writer = false;
    fs::create_dir_all(private_root_for(repo)).expect("the explicit private root");
    opts
}

/// Waits, bounded, until nothing holds the run: neither its lock nor its cleanup
/// lease. A run's agent-cleanup reapers keep its cleanup lease until every agent
/// process is gone (`rundir::WorktreeLock`'s acquisition says so where it refuses),
/// so a resume issued the instant `run_with` returns can meet a reaper still
/// settling and refuse overlapping ownership, as it should.
fn until_released(repo: &Path, run_id: &str) {
    let public = rundir::public_dir(repo, run_id);
    for _ in 0..1000 {
        if !rundir::is_running(&public) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("run `{run_id}` was still held ten seconds after its command returned");
}

fn first_line_of_the_log(repo: &Path, run_id: &str) -> serde_json::Value {
    let log = fs::read_to_string(rundir::public_dir(repo, run_id).join("events.jsonl"))
        .expect("the run's event log");
    serde_json::from_str(log.lines().next().expect("a committed first line"))
        .expect("the first line is an event")
}

fn assert_merged_on_the_integration_branch(repo: &Path, report: &RunReport) {
    let merged = git_in(
        repo,
        &[
            "show",
            &format!("refs/heads/{}:agent-output.txt", report.branch),
        ],
    );
    assert!(
        merged.contains("edited by the topology stub"),
        "the integration branch holds the stub's edit: {merged}"
    );
}

/// Two tasks whose path hints are disjoint. Without hints each would take the
/// global dispatch lease (design 26: "an absent or repo-wide hint takes a global
/// lease"), and the two would never be admitted together.
#[cfg(unix)]
const TWO_INDEPENDENT_TASKS: &str = "## Implement the widget\n\
                                     <!-- upstroke: id=t1 depends= paths=widget/** -->\n\
                                     Make it.\n\n## Implement the gadget\n\
                                     <!-- upstroke: id=t2 depends= paths=gadget/** -->\n\
                                     Make that.\n";

// ---------------------------------------------------------------------------
// PR12: the remaining public-path witnesses (P12-W1, W2, W3, W5, W14, W16; the
// resume refusals and the recorded-limits warning)
// ---------------------------------------------------------------------------

fn private_dir_of(repo: &Path, run_id: &str) -> PathBuf {
    PathBuf::from(
        first_line_of_the_log(repo, run_id)["data"]["private_dir"]
            .as_str()
            .expect("run_started names its private half"),
    )
}

fn event_kinds(repo: &Path, run_id: &str) -> Vec<String> {
    fs::read_to_string(rundir::public_dir(repo, run_id).join("events.jsonl"))
        .expect("the run's event log")
        .lines()
        .map(|line| {
            serde_json::from_str::<serde_json::Value>(line).expect("an event")["event"]
                .as_str()
                .unwrap_or_default()
                .to_owned()
        })
        .collect()
}

/// The topology stub, also noting at every probe whether the run's owner record
/// was already published under the private root.
struct OwnerWitness {
    stub: TopologyStub,
    private_root: PathBuf,
    owner_at_probe: Mutex<Vec<bool>>,
}

impl AgentAdapter for OwnerWitness {
    fn id(&self) -> &'static str {
        self.stub.id()
    }

    fn probe(&self, runner: &dyn crate::runner::Runner) -> Result<Caps, UpstrokeError> {
        let published = fs::read_dir(self.private_root.join("runs")).is_ok_and(|runs| {
            runs.flatten()
                .any(|run| run.path().join("owner.json").is_file())
        });
        self.owner_at_probe
            .lock()
            .map_err(|_| UpstrokeError::Agent {
                message: "owner witness lock poisoned".to_owned(),
            })?
            .push(published);
        self.stub.probe(runner)
    }

    fn build(&self, run: &TaskRun) -> Result<CommandSpec, UpstrokeError> {
        self.stub.build(run)
    }

    fn parse(&self, out: &ProcessOutput) -> Result<Outcome, UpstrokeError> {
        self.stub.parse(out)
    }
}

struct BorrowedOwnerWitness<'a>(&'a OwnerWitness);

impl AdapterSource for BorrowedOwnerWitness<'_> {
    fn get(&self, id: &str) -> Option<&dyn AgentAdapter> {
        (id == TOPOLOGY_STUB).then_some(self.0 as &dyn AgentAdapter)
    }
}

// ---------------------------------------------------------------------------
// G7: the native HOST acceptance record
// ---------------------------------------------------------------------------
//
// One ignored test. It drives two schema-4 runs on the production `HostRunner` through the public entry points, asserts
// of each what the PR12 witnesses above assert, and only then prints the runs'
// raw records as bounded, numbered lines that a reader of the job log can
// reconstruct and verify. The notes give the framing and what acceptance needs.

/// [`OwnerWitness`] behind a source that can be shared. Production's
/// `BuiltinAdapters` can be, so `engine::run` drives the concurrent driver; the
/// acceptance record takes that driver too and still notes, at every adapter
/// probe made through this source, whether the run's owner record was already
/// published. That is all it observes: not the shell probe, and no probe made
/// any other way.
struct SharedOwnerWitness(std::sync::Arc<OwnerWitness>);

impl AdapterSource for SharedOwnerWitness {
    fn get(&self, id: &str) -> Option<&dyn AgentAdapter> {
        (id == TOPOLOGY_STUB).then_some(&*self.0 as &dyn AgentAdapter)
    }

    fn shared(&self) -> Option<std::sync::Arc<dyn AdapterSource + Send + Sync>> {
        Some(std::sync::Arc::new(Self(std::sync::Arc::clone(&self.0))))
    }
}

/// The first field of every line of the record's framing.
const HOST_ACCEPTANCE_MARKER: &str = "UPSTROKE-HOST-ACCEPTANCE-RECORD";

/// The framing's version, the second field of every line.
const HOST_ACCEPTANCE_FRAMING: &str = "v1";

/// Record bytes per chunk line: 192 hex characters, so no line passes about 250.
const HOST_ACCEPTANCE_CHUNK: usize = 96;

/// The only environment names the record reads. Provenance, never proof: the CI
/// run, attempt, job and runner a reader of the log can check the record against.
const HOST_ACCEPTANCE_CI_VARIABLES: [&str; 15] = [
    "GITHUB_ACTIONS",
    "GITHUB_REPOSITORY",
    "GITHUB_WORKFLOW",
    "GITHUB_EVENT_NAME",
    "GITHUB_SHA",
    "GITHUB_REF",
    "GITHUB_RUN_ID",
    "GITHUB_RUN_ATTEMPT",
    "GITHUB_JOB",
    "RUNNER_NAME",
    "RUNNER_OS",
    "RUNNER_ARCH",
    "RUNNER_ENVIRONMENT",
    "ImageOS",
    "ImageVersion",
];

/// Each run's four records, by name and by the half of the run that holds them.
const HOST_ACCEPTANCE_FILES: [(&str, &str); 4] = [
    ("events.jsonl", "public"),
    ("report.json", "public"),
    ("owner.json", "private"),
    ("committed.json", "private"),
];

/// The names that select another repository's Git context. The harness's own
/// checkout is read without them, so the record names the repository the test
/// binary was built from and not one an inherited variable points at.
const HARNESS_GIT_CONTEXT: [&str; 10] = [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_COMMON_DIR",
    "GIT_IMPLICIT_WORK_TREE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_CONFIG",
    "GIT_CONFIG_COUNT",
    "GIT_CONFIG_PARAMETERS",
];

fn host_acceptance_sha256(bytes: &[u8]) -> String {
    format!("{:x}", <sha2::Sha256 as sha2::Digest>::digest(bytes))
}

fn host_acceptance_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        hex.push(char::from(DIGITS[usize::from(byte >> 4)]));
        hex.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    hex
}

/// Git in the checkout this test binary was compiled from, never a scratch run
/// repository, with [`HARNESS_GIT_CONTEXT`] removed from the child's environment.
fn harness_git(args: &[&str]) -> String {
    let mut git = Command::new("git");
    git.arg("-C").arg(env!("CARGO_MANIFEST_DIR")).args(args);
    for name in HARNESS_GIT_CONTEXT {
        git.env_remove(name);
    }
    let out = git
        .output()
        .expect("git runs in the harness's own checkout");
    assert!(
        out.status.success(),
        "git {args:?} in the harness's own checkout: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).expect("git prints UTF-8")
}

/// The test binary that is running, by its own bytes: streamed, so a large
/// harness is never held in memory, and compared with its length on disk.
fn host_acceptance_current_exe() -> serde_json::Value {
    use std::io::Read as _;

    let exe = std::env::current_exe().expect("the test binary's own path");
    let mut file = fs::File::open(&exe).expect("the test binary opens for reading");
    let mut hasher = <sha2::Sha256 as sha2::Digest>::new();
    let mut buffer = vec![0_u8; 1 << 20];
    let mut bytes: u64 = 0;
    loop {
        let read = file.read(&mut buffer).expect("the test binary reads");
        if read == 0 {
            break;
        }
        sha2::Digest::update(&mut hasher, &buffer[..read]);
        bytes += u64::try_from(read).expect("one read fits in u64");
    }
    assert_eq!(
        bytes,
        fs::metadata(&exe)
            .expect("the test binary's metadata")
            .len(),
        "the whole test binary was hashed"
    );
    serde_json::json!({
        "file_name": exe.file_name().map(|name| name.to_string_lossy().into_owned()),
        "bytes": bytes,
        "sha256": format!("{:x}", sha2::Digest::finalize(hasher)),
    })
}

/// What built and ran the record: the crate, the platform, the test binary's
/// digest and the source checkout's HEAD and tree.
fn host_acceptance_harness() -> serde_json::Value {
    let status = harness_git(&[
        "--no-optional-locks",
        "status",
        "--porcelain",
        "--untracked-files=no",
    ]);
    serde_json::json!({
        "crate": env!("CARGO_PKG_NAME"),
        "crate_version": env!("CARGO_PKG_VERSION"),
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "family": std::env::consts::FAMILY,
        "current_exe": host_acceptance_current_exe(),
        "source": {
            "manifest_dir": env!("CARGO_MANIFEST_DIR"),
            "head": harness_git(&["rev-parse", "HEAD"]).trim(),
            "tree": harness_git(&["rev-parse", "HEAD^{tree}"]).trim(),
            "tracked_changes": !status.trim().is_empty(),
        },
    })
}

/// [`HOST_ACCEPTANCE_CI_VARIABLES`], each a string or null, and whether any is set.
fn host_acceptance_ci() -> serde_json::Value {
    let mut variables = serde_json::Map::new();
    for name in HOST_ACCEPTANCE_CI_VARIABLES {
        let value = std::env::var_os(name).map_or(serde_json::Value::Null, |value| {
            serde_json::Value::String(value.to_string_lossy().into_owned())
        });
        variables.insert(name.to_owned(), value);
    }
    let present = variables.values().any(|value| !value.is_null());
    serde_json::json!({
        "state": if present { "present" } else { "absent" },
        "statement": if present {
            "provenance only: environment text naming the CI run; the native binding checks the \
             checkout and source against the actual job and review head"
        } else {
            "none of the allowlisted CI variables is set: this record was not made by a GitHub \
             Actions job"
        },
        "variables": variables,
    })
}

/// What the parked epoch of the resumed run left, read before it was answered.
struct HostAcceptanceParked {
    log_bytes: usize,
    log_sha256: String,
    owner_sha256: String,
}

/// One released run's records, asserted and returned as its part of the record:
/// the owner published before every adapter probe the owner witness saw (each
/// `AgentAdapter::probe` the run made through its adapter source, the first
/// included); `run_started(4)` on the host runner;
/// the commit record naming the committed first line's digest; one runner in
/// `owner.json`, `run_started`, every `run_resumed` and `report.json`; the report
/// the fresh projection of the log; both tasks merged and the recorded
/// integration ref holding the last merge; the log ending complete.
fn host_acceptance_run(
    case: &str,
    repo: &Path,
    run_id: &str,
    probes: &[bool],
    invocations: serde_json::Value,
    parked: Option<&HostAcceptanceParked>,
) -> serde_json::Value {
    assert!(
        !probes.is_empty() && probes.iter().all(|published| *published),
        "{case}: owner.json was published before every adapter probe the owner witness saw, \
         the first included: {probes:?}"
    );
    let public = rundir::public_dir(repo, run_id);
    let private = private_dir_of(repo, run_id);
    let files: Vec<(&str, &str, Vec<u8>)> = HOST_ACCEPTANCE_FILES
        .iter()
        .map(|(name, half)| {
            let directory = if *half == "public" { &public } else { &private };
            let path = directory.join(name);
            let bytes = fs::read(&path)
                .unwrap_or_else(|error| panic!("{case}: {}: {error}", path.display()));
            (*name, *half, bytes)
        })
        .collect();
    let bytes_of = |wanted: &str| {
        files
            .iter()
            .find(|(name, _, _)| *name == wanted)
            .map(|(_, _, bytes)| bytes.as_slice())
            .unwrap_or_else(|| panic!("{case}: no {wanted}"))
    };
    let parse = |name: &str| -> serde_json::Value {
        serde_json::from_slice(bytes_of(name))
            .unwrap_or_else(|error| panic!("{case}: {name} parses: {error}"))
    };

    let log = bytes_of("events.jsonl");
    let events: Vec<serde_json::Value> = std::str::from_utf8(log)
        .expect("events.jsonl is UTF-8")
        .lines()
        .map(|line| serde_json::from_str(line).expect("an event"))
        .collect();
    let first_line = &log[..log
        .iter()
        .position(|byte| *byte == b'\n')
        .expect("a committed first line")];
    let first_digest = host_acceptance_sha256(first_line);
    let started = events.first().expect("a first event");
    assert_eq!(started["event"], "run_started", "{case}");
    assert_eq!(
        started["data"]["schema"], 4,
        "{case}: the public entry point wrote schema 4"
    );
    assert_eq!(started["data"]["run_id"], run_id, "{case}");
    assert_eq!(started["data"]["branch"], format!("upstroke/run-{run_id}"));
    let runner = &started["data"]["runner"];
    assert_eq!(
        runner["kind"], "host",
        "{case}: the production HostRunner ran it"
    );
    assert!(
        runner["image"].is_null(),
        "{case}: a host runner records no image: {runner}"
    );
    let runner_bytes = serde_json::to_vec(runner).expect("run_started's runner");
    let same_runner =
        |value: &serde_json::Value| serde_json::to_vec(value).expect("a runner") == runner_bytes;

    let committed = parse("committed.json");
    assert_eq!(
        committed["run_started_sha256"],
        format!("sha256:{first_digest}"),
        "{case}: committed.json names the digest of the committed first line"
    );
    assert_eq!(committed["run_id"], run_id, "{case}");
    assert_eq!(
        committed["incarnation"], started["data"]["incarnation"],
        "{case}"
    );

    let owner = parse("owner.json");
    assert!(
        same_runner(&owner["runner"]),
        "{case}: owner.json records the runner run_started(4) records"
    );
    assert_eq!(owner["run_id"], run_id, "{case}");
    assert_eq!(
        owner["incarnation"], started["data"]["incarnation"],
        "{case}"
    );

    let resumed: Vec<&serde_json::Value> = events
        .iter()
        .filter(|event| event["event"] == "run_resumed")
        .collect();
    assert_eq!(
        resumed.is_empty(),
        parked.is_none(),
        "{case}: a resumed run records its resume, a fresh one none"
    );
    assert!(
        resumed
            .iter()
            .all(|event| same_runner(&event["data"]["runner"])),
        "{case}: every epoch records the runner run_started(4) records"
    );

    let report_bytes = bytes_of("report.json");
    let report = parse("report.json");
    assert!(
        same_runner(&report["runner"]),
        "{case}: report.json names the runner run_started(4) records"
    );
    assert_eq!(report["run_id"], run_id, "{case}");
    assert_eq!(report["outcome"], "complete", "{case}");
    let projected = crate::engine::topology::observe::observe(repo, run_id)
        .expect("observes")
        .expect("a schema-4 log")
        .report()
        .expect("projects");
    assert!(
        projected.is_fresh_against(report_bytes),
        "{case}: report.json is the projection of the committed log"
    );

    let merged: Vec<String> = events
        .iter()
        .filter(|event| event["event"] == "task_merged")
        .map(|event| {
            event["data"]["merged_sha"]
                .as_str()
                .expect("a merged sha")
                .to_owned()
        })
        .collect();
    assert_eq!(merged.len(), 2, "{case}: both tasks merged: {merged:?}");
    let integration_ref = started["data"]["integration_ref"]
        .as_str()
        .expect("run_started names its integration ref");
    assert_eq!(integration_ref, format!("refs/heads/upstroke/run-{run_id}"));
    let at_ref = git_in(repo, &["rev-parse", integration_ref])
        .trim()
        .to_owned();
    assert_eq!(
        Some(&at_ref),
        merged.last(),
        "{case}: the recorded integration ref holds the last merge the log records"
    );
    let last = events.last().expect("a last event");
    assert_eq!(last["event"], "run_finished", "{case}");
    assert_eq!(last["data"]["outcome"], "complete", "{case}");

    if let Some(parked) = parked {
        assert!(
            log.len() > parked.log_bytes
                && host_acceptance_sha256(&log[..parked.log_bytes]) == parked.log_sha256,
            "{case}: the resumed log continues the parked log byte for byte"
        );
        assert_eq!(
            host_acceptance_sha256(bytes_of("owner.json")),
            parked.owner_sha256,
            "{case}: owner.json is the record the run was created with, unchanged by the resume"
        );
    }

    let epochs: Vec<serde_json::Value> = std::iter::once(serde_json::json!({
        "event": "run_started",
        "incarnation": started["data"]["incarnation"],
    }))
    .chain(resumed.iter().map(|event| {
        serde_json::json!({
            "event": "run_resumed",
            "incarnation": event["data"]["incarnation"],
        })
    }))
    .collect();
    let mut embedded = serde_json::Map::new();
    for (name, half, bytes) in &files {
        embedded.insert(
            (*name).to_owned(),
            serde_json::json!({
                "half": half,
                "bytes": bytes.len(),
                "sha256": host_acceptance_sha256(bytes),
                "utf8": std::str::from_utf8(bytes)
                    .unwrap_or_else(|error| panic!("{case}: {name} is UTF-8: {error}")),
            }),
        );
    }
    serde_json::json!({
        "case": case,
        "run_id": run_id,
        "invocations": invocations,
        "outcome": "complete",
        "branch": started["data"]["branch"],
        "integration_ref": integration_ref,
        "integration_ref_sha": at_ref,
        "merged_shas": merged,
        "runner": runner,
        "owner_published_at_each_adapter_probe": probes,
        "first_line": {
            "text": std::str::from_utf8(first_line).expect("the first line is UTF-8"),
            "bytes": first_line.len(),
            "sha256": first_digest,
        },
        "committed_run_started_sha256": committed["run_started_sha256"],
        "epochs": epochs,
        "report_fresh": true,
        "parked_log": parked.map(|parked| serde_json::json!({
            "bytes": parked.log_bytes,
            "sha256": parked.log_sha256,
        })),
        "owner_sha256_at_park": parked.map(|parked| parked.owner_sha256.clone()),
        "files": embedded,
    })
}

/// A complete fresh schema-4 run: `engine::run_with` over the two-task plan.
fn host_acceptance_fresh() -> serde_json::Value {
    let (_tree, repo) = temp_engine_repo("host-acceptance-fresh");
    let witness = std::sync::Arc::new(OwnerWitness {
        stub: TopologyStub::new(TOPOLOGY_STUB_EDIT, 0),
        private_root: private_root_for(&repo),
        owner_at_probe: Mutex::new(Vec::new()),
    });
    let source = SharedOwnerWitness(std::sync::Arc::clone(&witness));
    let report =
        run_with(&topology_options(&repo), &source).expect("the fresh schema-4 host run completes");
    until_released(&repo, &report.run_id);
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:#?}");
    assert!(
        committed(&report, "t1") && committed(&report, "t2"),
        "{report:#?}"
    );
    assert_merged_on_the_integration_branch(&repo, &report);
    let probes = witness
        .owner_at_probe
        .lock()
        .expect("owner witness")
        .clone();
    host_acceptance_run(
        "fresh",
        &repo,
        &report.run_id,
        &probes,
        serde_json::json!([
            {"entry": "engine::run_with", "outcome": "complete", "adapter_probes": probes.len()}
        ]),
        None,
    )
}

/// A separate run that parks on its one question, is answered through
/// `answer::answer` and is finished by `engine::resume_with`: two epochs of one
/// run id, each released before the next command and before the records are read.
fn host_acceptance_resumed() -> serde_json::Value {
    let (_tree, repo) = temp_engine_repo("host-acceptance-resumed");
    let witness = std::sync::Arc::new(OwnerWitness {
        stub: TopologyStub::new(TOPOLOGY_STUB_EDIT, 1),
        private_root: private_root_for(&repo),
        owner_at_probe: Mutex::new(Vec::new()),
    });
    let source = SharedOwnerWitness(std::sync::Arc::clone(&witness));
    let parked = run_with(&topology_options(&repo), &source).expect("the run parks");
    until_released(&repo, &parked.run_id);
    assert_eq!(parked.outcome(), RunOutcome::Parked, "{parked:#?}");
    let [asked] = parked.questions.as_slice() else {
        panic!("one question: {parked:#?}");
    };
    let question = asked.question.id.to_string();
    let parked_probes = witness.owner_at_probe.lock().expect("owner witness").len();
    let log_at_park =
        fs::read(rundir::public_dir(&repo, &parked.run_id).join("events.jsonl")).expect("the log");
    let owner_at_park = fs::read(private_dir_of(&repo, &parked.run_id).join("owner.json"))
        .expect("owner.json at the park");

    crate::answer::answer(
        &repo,
        &question,
        crate::answer::Reply::Text("the first".to_owned()),
    )
    .expect("the answer is recorded");
    until_released(&repo, &parked.run_id);
    let mut resume = resume_options(&repo, &parked.run_id);
    resume.private_root = None;
    let report = resume_with(&resume, &source).expect("the resume completes the run");
    until_released(&repo, &report.run_id);
    assert_eq!(report.run_id, parked.run_id, "one run across both epochs");
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:#?}");
    assert!(
        committed(&report, "t1") && committed(&report, "t2"),
        "{report:#?}"
    );
    assert_merged_on_the_integration_branch(&repo, &report);
    let probes = witness
        .owner_at_probe
        .lock()
        .expect("owner witness")
        .clone();
    host_acceptance_run(
        "parked-answered-resumed",
        &repo,
        &report.run_id,
        &probes,
        serde_json::json!([
            {"entry": "engine::run_with", "outcome": "parked", "question": question, "adapter_probes": parked_probes},
            {"entry": "answer::answer", "question": question, "reply": "the first"},
            {"entry": "engine::resume_with", "outcome": "complete", "adapter_probes": probes.len() - parked_probes}
        ]),
        Some(&HostAcceptanceParked {
            log_bytes: log_at_park.len(),
            log_sha256: host_acceptance_sha256(&log_at_park),
            owner_sha256: host_acceptance_sha256(&owner_at_park),
        }),
    )
}

/// The record's bytes as numbered lines: a `BEGIN` summary, one line of
/// [`HOST_ACCEPTANCE_CHUNK`] bytes in lowercase hex per chunk, and an `END`
/// summary repeating the `BEGIN` one. The summary is the tag (the digest's first
/// sixteen characters, which every chunk line repeats), the chunk count, the byte
/// count and the whole record's SHA-256.
fn host_acceptance_frame(record: &[u8]) -> String {
    let digest = host_acceptance_sha256(record);
    let tag = &digest[..16];
    let chunks: Vec<&[u8]> = record.chunks(HOST_ACCEPTANCE_CHUNK).collect();
    let summary = format!("{tag} {} {} {digest}", chunks.len(), record.len());
    let mut lines = vec![format!(
        "{HOST_ACCEPTANCE_MARKER} {HOST_ACCEPTANCE_FRAMING} BEGIN {summary}"
    )];
    lines.extend(chunks.iter().enumerate().map(|(index, chunk)| {
        format!(
            "{HOST_ACCEPTANCE_MARKER} {HOST_ACCEPTANCE_FRAMING} {tag} {} {}",
            index + 1,
            host_acceptance_hex(chunk)
        )
    }));
    lines.push(format!(
        "{HOST_ACCEPTANCE_MARKER} {HOST_ACCEPTANCE_FRAMING} END {summary}"
    ));
    lines.join("\n") + "\n"
}

/// G7's native HOST acceptance record (root direction `087fea89`). Two schema-4
/// runs on the production `HostRunner` through the public entry points, with the
/// deterministic stub adapter: a complete fresh run, and a separate run parked,
/// answered and resumed. Every assertion above holds of both before a byte is
/// printed; then the record -- the harness's provenance, both full run ids and
/// runner identities, and each run's raw `events.jsonl`, `owner.json`,
/// `committed.json` and `report.json` with their lengths and digests -- goes to
/// stdout in [`host_acceptance_frame`]'s lines, written directly because the
/// engine forbids `println!`. Printing is not the verdict: a record is accepted
/// only with this test's pass line, the command's zero exit, and a complete
/// reconstruction whose digests all agree.
#[test]
#[ignore = "native host acceptance record: run only by CI's capture step, which selects it by \
            its exact name after the suite, or after a shard's share of it"]
fn host_acceptance_record() {
    use std::io::Write as _;

    let fresh = host_acceptance_fresh();
    let resumed = host_acceptance_resumed();
    assert_ne!(fresh["run_id"], resumed["run_id"], "two runs, two run ids");
    let mut manifest = Vec::new();
    for run in [&fresh, &resumed] {
        for (name, half) in HOST_ACCEPTANCE_FILES {
            let file = &run["files"][name];
            manifest.push(serde_json::json!({
                "case": run["case"],
                "run_id": run["run_id"],
                "file": name,
                "half": half,
                "bytes": file["bytes"],
                "sha256": file["sha256"],
            }));
        }
    }
    let record = serde_json::json!({
        "format": "upstroke-host-acceptance-record",
        "format_version": 2,
        "test": "engine::tests::host_acceptance_record",
        "harness": host_acceptance_harness(),
        "ci": host_acceptance_ci(),
        "scope": {
            "runner": "host: the production HostRunner started the stub adapter's probe, worker \
                       and reviewer processes; runner.image is null by design and no container \
                       was used",
            "owner_witness": "OwnerWitness notes, at each AgentAdapter::probe the run made \
                              through its adapter source, whether owner.json was already \
                              published under the private root; \
                              owner_published_at_each_adapter_probe is that list and nothing \
                              more. It does not observe the shell probe or any probe made \
                              another way, whose order relative to the owner record is the \
                              engine's composition, not a separate observation here",
            "provider": "the deterministic stub adapter (engine::tests::TopologyStub behind \
                         OwnerWitness); no provider was called",
            "coordinator": "this test process called engine::run_with, answer::answer and \
                            engine::resume_with; it is not the upstroke CLI process",
            "acceptance": "printed only after every assertion passed; accepted only with the \
                           named test's pass line, exit 0, complete reconstruction and matching \
                           digests",
        },
        "runs": [fresh, resumed],
        "manifest": manifest,
    });
    let bytes = serde_json::to_vec(&record).expect("the record serializes");
    let mut stdout = std::io::stdout().lock();
    stdout
        .write_all(host_acceptance_frame(&bytes).as_bytes())
        .expect("the record reaches stdout");
    stdout.flush().expect("the record is flushed");
}

// ---------------------------------------------------------------------------
// G7: the native CONTAINER acceptance record
// ---------------------------------------------------------------------------
//
// One ignored test, selected by its exact name only by the platform capture
// recipes under `.github/g7-platform-environment/`, never by a required job.
// It drives two schema-4 runs whose `[runner]` selects the container boundary
// through the public entry points, on the production `ContainerRunner` over
// the production runtime, which `engine::topology::entry::container_runtime`
// observes while armed. The notes give the framing, the observation and what
// acceptance needs.

/// The first field of every line of the container record's framing.
const CONTAINER_ACCEPTANCE_MARKER: &str = "UPSTROKE-CONTAINER-ACCEPTANCE-RECORD";

/// The container framing's version, the second field of every line.
const CONTAINER_ACCEPTANCE_FRAMING: &str = "v1";

/// The image reference both runs select. Required: never defaulted, pulled or skipped.
const CONTAINER_ACCEPTANCE_IMAGE: &str = "UPSTROKE_CONTAINER_ACCEPTANCE_IMAGE";

const CONTAINER_ACCEPTANCE_DIR_TARGET: &str = "/upstroke-acceptance/dir";
const CONTAINER_ACCEPTANCE_FILE_TARGET: &str = "/upstroke-acceptance/file.txt";
const CONTAINER_ACCEPTANCE_FILE_TEXT: &str = "upstroke-acceptance-configured-file-v1";
const CONTAINER_ACCEPTANCE_PROBE: &str = "UPSTROKE-ACCEPTANCE-PROBE";
const CONTAINER_ACCEPTANCE_WORKER: &str = "UPSTROKE-ACCEPTANCE-WORKER";
const CONTAINER_ACCEPTANCE_REVIEW: &str = "UPSTROKE-ACCEPTANCE-REVIEW";
const CONTAINER_ACCEPTANCE_REVIEW_WRITE: &str = ".upstroke-acceptance-review-write";

fn container_acceptance_write_attempt(path: &str) -> String {
    format!("$(if ( : > {path} ) 2>/dev/null; then echo allowed; else echo refused; fi)")
}

/// The agent probe, in the probe container's scratch directory.
fn container_acceptance_probe_script() -> String {
    format!(
        "echo \"{CONTAINER_ACCEPTANCE_PROBE} uid=$(id -u) gid=$(id -g) system=$(uname -s) \
         release=$(uname -r) machine=$(uname -m) root_write={root} dir_write={dir} \
         file=$(cat {CONTAINER_ACCEPTANCE_FILE_TARGET})\"",
        root = container_acceptance_write_attempt("/upstroke-acceptance-probe-write"),
        dir = container_acceptance_write_attempt(&format!(
            "{CONTAINER_ACCEPTANCE_DIR_TARGET}/probe-write"
        )),
    )
}

/// The worker: the topology stub's edit, then what the container sees of the
/// checkout, the configured file and the role's Git view.
fn container_acceptance_worker_script() -> String {
    format!(
        "{TOPOLOGY_STUB_EDIT} && if [ -f .git ]; then g=$(sed -n 's/^gitdir: //p' .git); \
         else g=.git; fi && echo \"{CONTAINER_ACCEPTANCE_WORKER} uid=$(id -u) gid=$(id -g) \
         readme=$(cat README.md) file=$(cat {CONTAINER_ACCEPTANCE_FILE_TARGET}) \
         head=$(cat \"$g/HEAD\") alternates=$(cat \"$g/objects/info/alternates\")\" \
         && echo worker >> {CONTAINER_ACCEPTANCE_DIR_TARGET}/workers.txt"
    )
}

/// The reviewer: a write to its checkout and to the configured directory, each
/// expected refused, then the stub's passing verdict.
fn container_acceptance_review_script() -> String {
    format!(
        "echo \"{CONTAINER_ACCEPTANCE_REVIEW} workspace_write={workspace} dir_write={dir} \
         file=$(cat {CONTAINER_ACCEPTANCE_FILE_TARGET}) uid=$(id -u) gid=$(id -g)\" \
         && echo {TOPOLOGY_STUB_REVIEW}",
        workspace = container_acceptance_write_attempt(CONTAINER_ACCEPTANCE_REVIEW_WRITE),
        dir = container_acceptance_write_attempt(&format!(
            "{CONTAINER_ACCEPTANCE_DIR_TARGET}/review-write"
        )),
    )
}

/// The one `[[gates]]` command, run by the run's recorded shell in the container.
fn container_acceptance_gate_script() -> String {
    format!(
        "grep -q \"edited by the topology stub\" agent-output.txt \
         && echo gate >> {CONTAINER_ACCEPTANCE_DIR_TARGET}/gates.txt"
    )
}

fn container_acceptance_config(image: &str, directory: &Path, file: &Path) -> String {
    let literal = |text: &str| {
        assert!(
            !text.contains(['\'', '\n', '\r']),
            "`{text}` cannot be written as a TOML literal string"
        );
        format!("'{text}'")
    };
    let spelled = |path: &Path| {
        path.to_str()
            .expect("a fixture path this record can spell in TOML")
            .to_owned()
    };
    format!(
        "[engine]\nshell = \"sh\"\n\n[runner]\nkind = \"container\"\nimage = {image}\nmounts = [\n  \
         {{ source = {directory}, target = \"{CONTAINER_ACCEPTANCE_DIR_TARGET}\", read_only = false }},\n  \
         {{ source = {file}, target = \"{CONTAINER_ACCEPTANCE_FILE_TARGET}\", read_only = true }},\n]\n\n\
         [[gates]]\nname = \"acceptance\"\ncmd = {gate}\n",
        image = literal(image),
        directory = literal(&spelled(directory)),
        file = literal(&spelled(file)),
        gate = literal(&container_acceptance_gate_script()),
    )
}

/// The configured mounts' sources, beside the repository in its guarded tree,
/// and the run's config committed with the plan.
struct ContainerAcceptanceFixture {
    image: String,
    directory: PathBuf,
    file: PathBuf,
}

fn container_acceptance_fixture(
    tree: &Path,
    repo: &Path,
    image: &str,
) -> ContainerAcceptanceFixture {
    let mounts = tree.join("mounts");
    fs::create_dir(&mounts).expect("the configured mounts' parent, inside the guarded tree");
    let directory = mounts.join("configured-dir");
    fs::create_dir(&directory).expect("the configured directory mount's source");
    let file = mounts.join("configured-file.txt");
    fs::write(&file, format!("{CONTAINER_ACCEPTANCE_FILE_TEXT}\n"))
        .expect("the configured file mount's source");
    let plan = fs::read_to_string(repo.join("plan.md")).expect("the plan temp_engine_repo seeded");
    seed(
        repo,
        &plan,
        Some(&container_acceptance_config(image, &directory, &file)),
    );
    ContainerAcceptanceFixture {
        image: image.to_owned(),
        directory,
        file,
    }
}

/// The topology stub's parse and question counter, with every command spelled
/// for the Linux image's `sh` (never the native shell), noting at each adapter
/// probe whether the owner record was published and keeping every output it saw.
struct ContainerWitness {
    stub: TopologyStub,
    private_root: PathBuf,
    owner_at_probe: Mutex<Vec<bool>>,
    outputs: Mutex<Vec<serde_json::Value>>,
}

impl ContainerWitness {
    fn new(questions: usize, private_root: PathBuf) -> Self {
        Self {
            stub: TopologyStub::new(&container_acceptance_worker_script(), questions),
            private_root,
            owner_at_probe: Mutex::new(Vec::new()),
            outputs: Mutex::new(Vec::new()),
        }
    }

    fn witness(&self, kind: &str, out: &ProcessOutput) -> Result<(), UpstrokeError> {
        self.outputs
            .lock()
            .map_err(|_| UpstrokeError::Agent {
                message: "container witness lock poisoned".to_owned(),
            })?
            .push(serde_json::json!({
                "kind": kind,
                "code": out.code,
                "timed_out": out.timed_out,
                "output_limited": out.output_limited,
                "stdout": out.stdout,
                "stderr": out.stderr,
            }));
        Ok(())
    }
}

impl AgentAdapter for ContainerWitness {
    fn id(&self) -> &'static str {
        TOPOLOGY_STUB
    }

    fn probe(&self, runner: &dyn crate::runner::Runner) -> Result<Caps, UpstrokeError> {
        let published = fs::read_dir(self.private_root.join("runs")).is_ok_and(|runs| {
            runs.flatten()
                .any(|run| run.path().join("owner.json").is_file())
        });
        self.owner_at_probe
            .lock()
            .map_err(|_| UpstrokeError::Agent {
                message: "container witness lock poisoned".to_owned(),
            })?
            .push(published);
        let request = crate::agent::probe_request(
            TOPOLOGY_STUB,
            crate::gates::ShellKind::Sh.spec(&container_acceptance_probe_script()),
            0,
            Duration::from_secs(60),
        )?;
        let output = runner.run_blocking(&request)?;
        self.witness("probe", &output)?;
        if output.code != Some(0) {
            return Err(UpstrokeError::Agent {
                message: format!(
                    "the container stub's probe exited {:?}: {}",
                    output.code, output.stderr
                ),
            });
        }
        Ok(Caps {
            version: "0.0.0-container-stub".to_owned(),
            json_output: true,
            session_resume: false,
            cost_reporting: true,
            read_only_mode: true,
            acp: false,
            model_list: false,
        })
    }

    fn build(&self, run: &TaskRun) -> Result<CommandSpec, UpstrokeError> {
        let script = if run.profile.permissions == PermissionMode::ReadOnly {
            container_acceptance_review_script()
        } else {
            container_acceptance_worker_script()
        };
        Ok(crate::gates::ShellKind::Sh.spec(&script))
    }

    fn parse(&self, out: &ProcessOutput) -> Result<Outcome, UpstrokeError> {
        let kind = if out.stdout.contains(CONTAINER_ACCEPTANCE_REVIEW) {
            "review"
        } else {
            "worker"
        };
        self.witness(kind, out)?;
        self.stub.parse(out)
    }
}

/// [`ContainerWitness`] behind a source that can be shared, as production's is.
struct SharedContainerWitness(std::sync::Arc<ContainerWitness>);

impl AdapterSource for SharedContainerWitness {
    fn get(&self, id: &str) -> Option<&dyn AgentAdapter> {
        (id == TOPOLOGY_STUB).then_some(&*self.0 as &dyn AgentAdapter)
    }

    fn shared(&self) -> Option<std::sync::Arc<dyn AdapterSource + Send + Sync>> {
        Some(std::sync::Arc::new(Self(std::sync::Arc::clone(&self.0))))
    }
}

/// What the parked epoch of the resumed container run left, read before it was answered.
struct ContainerAcceptanceParked {
    log_bytes: usize,
    log_sha256: String,
    owner_sha256: String,
}

/// One container case's inputs to [`container_acceptance_run`].
struct ContainerAcceptanceCase<'a> {
    case: &'a str,
    repo: &'a Path,
    run_id: &'a str,
    witness: &'a ContainerWitness,
    journal: &'a crate::runner::container::observed::Journal,
    calls: Vec<serde_json::Value>,
    fixture: &'a ContainerAcceptanceFixture,
    parked: Option<&'a ContainerAcceptanceParked>,
}

fn container_acceptance_creates(
    snapshot: &serde_json::Value,
    from: usize,
    to: usize,
) -> Vec<serde_json::Value> {
    snapshot["operations"]
        .as_array()
        .expect("the journal's operations")
        .iter()
        .filter(|operation| {
            operation["op"] == "create"
                && operation["runtime"]
                    .as_u64()
                    .and_then(|runtime| usize::try_from(runtime).ok())
                    .is_some_and(|runtime| (from..to).contains(&runtime))
        })
        .cloned()
        .collect()
}

/// After one public call: the daemon lists no container labelled with the
/// call's private root, and no container intent remains on disk.
fn container_acceptance_end_state(
    call: &str,
    repo: &Path,
    journal: &crate::runner::container::observed::Journal,
    from: usize,
    to: usize,
) -> serde_json::Value {
    let snapshot = journal.snapshot();
    let labels: std::collections::BTreeSet<String> =
        container_acceptance_creates(&snapshot, from, to)
            .iter()
            .filter_map(|create| {
                create["outcome"]["planned"]["labels"]["upstroke.private_root"]
                    .as_str()
                    .map(str::to_owned)
            })
            .collect();
    assert!(
        !labels.is_empty(),
        "{call}: the call created containers labelled with a private root"
    );
    for label in &labels {
        let remaining = crate::runner::container::observed::labelled(label)
            .unwrap_or_else(|error| panic!("{call}: the runtime lists by label: {error}"));
        assert!(
            remaining.is_empty(),
            "{call}: no container labelled with this private root remains: {remaining:?}"
        );
    }
    let intents = crate::runner::container::observed::intent_directory(&private_root_for(repo));
    let entries: Vec<String> = match fs::read_dir(&intents) {
        Ok(entries) => entries
            .map(|entry| {
                entry
                    .expect("an intent directory entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => panic!("{call}: {}: {error}", intents.display()),
    };
    assert!(
        entries.is_empty(),
        "{call}: every container intent was removed: {entries:?}"
    );
    serde_json::json!({
        "private_root_labels": labels,
        "labelled_containers_remaining": [],
        "intent_directory_entries": entries,
    })
}

fn container_acceptance_fields(
    stdout: &str,
    marker: &str,
) -> std::collections::BTreeMap<String, String> {
    let line = stdout
        .lines()
        .find(|line| line.starts_with(marker))
        .unwrap_or_else(|| panic!("no `{marker}` line in {stdout:?}"));
    line.split_whitespace()
        .skip(1)
        .filter_map(|field| field.split_once('='))
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .collect()
}

fn container_acceptance_role(invocation: &str) -> &'static str {
    if invocation.starts_with("p.shell.") {
        "probe-shell"
    } else if invocation.starts_with("p.agent-") {
        "probe-agent"
    } else if invocation.contains(".worker.") {
        "worker"
    } else if invocation.contains(".gate") {
        "gate"
    } else if invocation.contains(".review_") {
        "review"
    } else {
        panic!("`{invocation}` is no invocation this record knows")
    }
}

#[cfg(unix)]
fn container_acceptance_owner(path: &Path) -> serde_json::Value {
    use std::os::unix::fs::MetadataExt as _;
    let metadata = fs::metadata(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    serde_json::json!({
        "uid": metadata.uid(),
        "gid": metadata.gid(),
        "mode": format!("{:o}", metadata.mode() & 0o7777),
    })
}

#[cfg(not(unix))]
fn container_acceptance_owner(path: &Path) -> serde_json::Value {
    let metadata = fs::metadata(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    serde_json::json!({
        "uid": null,
        "gid": null,
        "readonly": metadata.permissions().readonly(),
    })
}

fn container_acceptance_lines(path: &Path) -> usize {
    match fs::read_to_string(path) {
        Ok(text) => text.lines().count(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
        Err(error) => panic!("{}: {error}", path.display()),
    }
}

/// What every container of one run is checked against: the run's own records
/// and the journal's operations.
struct ContainerAcceptanceRecorded<'a> {
    case: &'a str,
    run_id: &'a str,
    image_id: &'a str,
    policy_digest: &'a str,
    operations: &'a [serde_json::Value],
}

/// One container, validated from the journal: its planned spec, the intent on
/// disk, the daemon's own inspection at creation and before removal, and the
/// start, collection and removal that followed, all in one runtime.
fn container_acceptance_container(
    recorded: &ContainerAcceptanceRecorded<'_>,
    create: &serde_json::Value,
    epoch: usize,
    incarnation: &str,
) -> serde_json::Value {
    let ContainerAcceptanceRecorded {
        case,
        run_id,
        image_id,
        policy_digest,
        operations,
    } = *recorded;
    let name = create["target"].as_str().expect("a container name");
    let runtime = &create["runtime"];
    let outcome = &create["outcome"];
    let planned = &outcome["planned"];
    let at = |what: &str| format!("{case}: `{name}` {what}");
    assert_eq!(planned["name"], name, "{}", at("is the planned name"));
    assert_eq!(
        planned["image_id"],
        image_id,
        "{}",
        at("is planned from the recorded image id")
    );
    assert_eq!(
        outcome["result"]["ok"]["reported_image_id"],
        image_id,
        "{}: {outcome}",
        at("was created and reports the recorded image id")
    );
    assert_eq!(
        outcome["owner_published"],
        true,
        "{}",
        at("was created after owner.json")
    );

    let inspected = &outcome["inspect_at_create"]["parsed"]["value"];
    assert!(
        inspected.is_object(),
        "{}: {}",
        at("has the daemon's inspection at creation"),
        outcome["inspect_at_create"]
    );
    assert_eq!(
        inspected["Image"],
        image_id,
        "{}",
        at("inspects as the recorded image id")
    );
    assert_eq!(
        inspected["Name"],
        format!("/{name}"),
        "{}",
        at("inspects by its name")
    );
    let labels = planned["labels"].as_object().expect("the planned labels");
    for (key, value) in labels {
        assert_eq!(
            &inspected["Labels"][key],
            value,
            "{}",
            at(&format!("carries {key}"))
        );
    }
    assert_eq!(
        planned["read_only_root"],
        true,
        "{}",
        at("plans a read-only root")
    );
    assert_eq!(
        inspected["ReadonlyRootfs"],
        true,
        "{}",
        at("has a read-only root")
    );
    assert_eq!(
        inspected["User"],
        planned["user"].as_str().unwrap_or_default(),
        "{}",
        at("runs as its planned user")
    );
    assert_eq!(
        inspected["WorkingDir"],
        planned["workdir"],
        "{}",
        at("has its workdir")
    );

    let planned_mounts = planned["mounts"].as_array().expect("the planned mounts");
    let daemon_mounts = inspected["Mounts"].as_array().cloned().unwrap_or_default();
    let requested = inspected["HostConfigMounts"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let mut mounts = Vec::new();
    for mount in planned_mounts {
        let target = mount["target"].as_str().expect("a mount target");
        let read_only = mount["read_only"].as_bool().expect("a read-only flag");
        let kind = mount["type"].as_str().expect("a mount type");
        let request = requested
            .iter()
            .find(|request| request["Target"] == target)
            .unwrap_or_else(|| panic!("{}: {requested:?}", at(&format!("was asked for {target}"))));
        assert_eq!(
            request["Type"],
            kind,
            "{}",
            at(&format!("asked for a {kind} at {target}"))
        );
        let held = daemon_mounts
            .iter()
            .find(|held| held["Destination"] == target);
        if kind == "tmpfs" {
            mounts.push(serde_json::json!({
                "target": target,
                "type": kind,
                "requested": request,
                "held": held,
            }));
            continue;
        }
        let held =
            held.unwrap_or_else(|| panic!("{}: {daemon_mounts:?}", at(&format!("holds {target}"))));
        assert_eq!(
            held["Type"],
            kind,
            "{}",
            at(&format!("holds a {kind} at {target}"))
        );
        assert_eq!(
            held["RW"],
            !read_only,
            "{}",
            at(&format!("holds {target} read-only={read_only}"))
        );
        if kind == "bind" && !cfg!(windows) {
            assert_eq!(
                held["Source"],
                mount["source"],
                "{}",
                at(&format!("binds {target} from the planned source"))
            );
        }
        mounts.push(serde_json::json!({
            "target": target,
            "type": kind,
            "read_only": read_only,
            "planned_source": mount["source"],
            "argument": mount["argument"],
            "requested_source": request["Source"],
            "inspect_source": held["Source"],
            "inspect_rw": held["RW"],
        }));
    }
    for held in &daemon_mounts {
        assert!(
            planned_mounts
                .iter()
                .any(|mount| mount["target"] == held["Destination"]),
            "{}: {held}",
            at("holds no mount it was not planned")
        );
    }

    let intent = &outcome["intent"]["parsed"];
    assert!(
        intent["invocation"].is_string(),
        "{}: {}",
        at("has its intent on disk, parsed as the runner's record"),
        outcome["intent"]
    );
    assert_eq!(intent["run_id"], run_id, "{}", at("intends this run"));
    assert_eq!(
        intent["incarnation"],
        incarnation,
        "{}",
        at("intends its epoch's incarnation")
    );
    assert_eq!(
        intent["invocation"].as_str(),
        labels
            .get("upstroke.invocation")
            .and_then(serde_json::Value::as_str),
        "{}",
        at("intends the invocation it is labelled with")
    );
    assert_eq!(
        intent["runner_policy_sha256"],
        policy_digest,
        "{}",
        at("intends the recorded RunnerPolicy's digest")
    );
    assert_eq!(
        labels
            .get("upstroke.incarnation")
            .and_then(serde_json::Value::as_str),
        Some(incarnation),
        "{}",
        at("is labelled with its epoch's incarnation")
    );
    assert_eq!(
        labels
            .get("upstroke.run")
            .and_then(serde_json::Value::as_str),
        Some(run_id),
        "{}",
        at("is labelled with this run")
    );

    let find = |op: &str| {
        let found: Vec<&serde_json::Value> = operations
            .iter()
            .filter(|operation| {
                operation["op"] == op
                    && operation["target"] == name
                    && &operation["runtime"] == runtime
            })
            .collect();
        let [one] = found.as_slice() else {
            panic!(
                "{}: {found:?}",
                at(&format!("has exactly one {op} in its runtime"))
            );
        };
        *one
    };
    let started = find("start");
    assert!(
        started["outcome"]["ok"].is_null() && started["outcome"]["error"].is_null(),
        "{}: {started}",
        at("started")
    );
    let collected = find("collect");
    let exit_code = collected["outcome"]["ok"]["exit_code"]
        .as_i64()
        .unwrap_or_else(|| panic!("{}: {collected}", at("was collected with an exit code")));
    assert_eq!(exit_code, 0, "{}", at("exited 0"));
    let removed = find("remove");
    assert_eq!(
        removed["outcome"]["result"]["ok"],
        "process-gone",
        "{}: {removed}",
        at("was removed with its process gone")
    );
    let before = &removed["outcome"]["inspect_before_remove"]["parsed"]["value"];
    assert!(
        before.is_object(),
        "{}: {removed}",
        at("was inspected before its removal")
    );
    assert_eq!(
        before["Id"],
        inspected["Id"],
        "{}",
        at("is the container that was created")
    );
    assert_eq!(
        before["Image"],
        image_id,
        "{}",
        at("still inspects as the recorded image id")
    );
    assert_eq!(
        before["State"]["Status"],
        "exited",
        "{}",
        at("had exited before its removal")
    );
    assert_eq!(
        before["State"]["ExitCode"],
        exit_code,
        "{}",
        at("exited as collected")
    );
    assert_ne!(
        before["State"]["StartedAt"],
        "0001-01-01T00:00:00Z",
        "{}",
        at("had been started")
    );
    let sequence =
        |operation: &serde_json::Value| operation["sequence"].as_u64().expect("a sequence");
    assert!(
        sequence(create) < sequence(started)
            && sequence(started) < sequence(collected)
            && sequence(collected) < sequence(removed),
        "{}",
        at("was created, started, collected and removed in that order")
    );
    let stops: Vec<&serde_json::Value> = operations
        .iter()
        .filter(|operation| {
            operation["op"] == "stop"
                && operation["target"] == name
                && &operation["runtime"] == runtime
        })
        .collect();
    assert!(
        stops.len() <= 1,
        "{}: {stops:?}",
        at("was stopped at most once")
    );
    for stop in &stops {
        assert!(
            stop["outcome"]["result"]["ok"].is_string(),
            "{}: {stop}",
            at("was stopped and settled")
        );
        assert!(
            sequence(collected) < sequence(stop) && sequence(stop) < sequence(removed),
            "{}",
            at("was stopped by its release, after its collection and before its removal")
        );
    }
    let invocation = intent["invocation"]
        .as_str()
        .expect("the intent's invocation, asserted above")
        .to_owned();
    serde_json::json!({
        "name": name,
        "runtime": runtime,
        "epoch": epoch,
        "incarnation": incarnation,
        "invocation": invocation,
        "role": container_acceptance_role(&invocation),
        "container_id": inspected["Id"],
        "image": inspected["Image"],
        "config_image": inspected["ConfigImage"],
        "user": inspected["User"],
        "readonly_rootfs": inspected["ReadonlyRootfs"],
        "workdir": inspected["WorkingDir"],
        "command": inspected["Cmd"],
        "mounts": mounts,
        "exit_code": exit_code,
        "started_at": before["State"]["StartedAt"],
        "finished_at": before["State"]["FinishedAt"],
        "sequence": [sequence(create), sequence(started), sequence(collected), sequence(removed)],
        "release_stops": stops
            .iter()
            .map(|stop| serde_json::json!({
                "sequence": stop["sequence"],
                "mode": stop["outcome"]["mode"],
                "settled": stop["outcome"]["result"]["ok"],
            }))
            .collect::<Vec<_>>(),
        "inspect_at_create_sha256": outcome["inspect_at_create"]["sha256"],
        "inspect_before_remove_sha256": removed["outcome"]["inspect_before_remove"]["sha256"],
        "intent_sha256": outcome["intent"]["sha256"],
    })
}

/// The product runtime's own trace of one runtime, by the names each effect was asked about.
fn container_acceptance_traced(
    runtime: &serde_json::Value,
    op: &str,
) -> std::collections::BTreeSet<String> {
    let prefix = format!("rt:{op}:");
    runtime["trace"]
        .as_array()
        .expect("a runtime's trace")
        .iter()
        .filter_map(|run| run[0].as_str())
        .filter_map(|entry| entry.strip_prefix(&prefix))
        .map(str::to_owned)
        .collect()
}

fn container_acceptance_journaled(
    operations: &[serde_json::Value],
    runtime: usize,
    op: &str,
) -> std::collections::BTreeSet<String> {
    operations
        .iter()
        .filter(|operation| operation["op"] == op && operation["runtime"] == runtime)
        .filter_map(|operation| operation["target"].as_str().map(str::to_owned))
        .collect()
}

/// One released container run's records and observation, asserted and returned
/// as its part of the record: everything [`host_acceptance_run`] asserts of a
/// host run, with the container runner and its image identity in its place, and
/// every container of every epoch validated by [`container_acceptance_container`].
fn container_acceptance_run(input: &ContainerAcceptanceCase<'_>) -> serde_json::Value {
    let case = input.case;
    let repo = input.repo;
    let run_id = input.run_id;
    let witness = input.witness;
    let journal = input.journal;
    let fixture = input.fixture;
    let parked = input.parked;
    let probes = witness
        .owner_at_probe
        .lock()
        .expect("container witness")
        .clone();
    assert!(
        !probes.is_empty() && probes.iter().all(|published| *published),
        "{case}: owner.json was published before every adapter probe: {probes:?}"
    );
    let public = rundir::public_dir(repo, run_id);
    let private = private_dir_of(repo, run_id);
    let files: Vec<(&str, &str, Vec<u8>)> = HOST_ACCEPTANCE_FILES
        .iter()
        .map(|(name, half)| {
            let directory = if *half == "public" { &public } else { &private };
            let path = directory.join(name);
            let bytes = fs::read(&path)
                .unwrap_or_else(|error| panic!("{case}: {}: {error}", path.display()));
            (*name, *half, bytes)
        })
        .collect();
    let bytes_of = |wanted: &str| {
        files
            .iter()
            .find(|(name, _, _)| *name == wanted)
            .map(|(_, _, bytes)| bytes.as_slice())
            .unwrap_or_else(|| panic!("{case}: no {wanted}"))
    };
    let parse = |name: &str| -> serde_json::Value {
        serde_json::from_slice(bytes_of(name))
            .unwrap_or_else(|error| panic!("{case}: {name} parses: {error}"))
    };

    let log = bytes_of("events.jsonl");
    let events: Vec<serde_json::Value> = std::str::from_utf8(log)
        .expect("events.jsonl is UTF-8")
        .lines()
        .map(|line| serde_json::from_str(line).expect("an event"))
        .collect();
    let first_line = &log[..log
        .iter()
        .position(|byte| *byte == b'\n')
        .expect("a committed first line")];
    let first_digest = host_acceptance_sha256(first_line);
    let started = events.first().expect("a first event");
    assert_eq!(started["event"], "run_started", "{case}");
    assert_eq!(
        started["data"]["schema"], 4,
        "{case}: the public entry point wrote schema 4"
    );
    assert_eq!(started["data"]["run_id"], run_id, "{case}");
    assert_eq!(started["data"]["branch"], format!("upstroke/run-{run_id}"));
    assert_eq!(
        started["data"]["shell"], "sh",
        "{case}: the run recorded the image's shell"
    );
    let runner = &started["data"]["runner"];
    assert_eq!(
        runner["kind"], "container",
        "{case}: the production ContainerRunner ran it"
    );
    assert_eq!(runner["policy"], "container-v1", "{case}: {runner}");
    assert_eq!(
        runner["image"]["reference"], fixture.image,
        "{case}: the configured reference is recorded"
    );
    let image_id = runner["image"]["id"]
        .as_str()
        .filter(|id| !id.is_empty())
        .unwrap_or_else(|| panic!("{case}: run_started records an image id: {runner}"))
        .to_owned();
    assert_eq!(
        runner["credential_volumes"],
        serde_json::json!({}),
        "{case}: {runner}"
    );
    let recorded_mounts = runner["mounts"].as_array().expect("the recorded mounts");
    assert_eq!(
        recorded_mounts
            .iter()
            .map(|mount| (mount["target"].clone(), mount["read_only"].clone()))
            .collect::<Vec<_>>(),
        vec![
            (
                serde_json::json!(CONTAINER_ACCEPTANCE_DIR_TARGET),
                serde_json::json!(false)
            ),
            (
                serde_json::json!(CONTAINER_ACCEPTANCE_FILE_TARGET),
                serde_json::json!(true)
            ),
        ],
        "{case}: both configured mounts are recorded: {runner}"
    );
    let policy: crate::topology::events::RunnerPolicy = serde_json::from_value(runner.clone())
        .unwrap_or_else(|error| panic!("{case}: run_started's runner is a RunnerPolicy: {error}"));
    let policy_digest = crate::runner::policy::runner_policy_sha256(&policy);
    let runner_bytes = serde_json::to_vec(runner).expect("run_started's runner");
    let same_runner =
        |value: &serde_json::Value| serde_json::to_vec(value).expect("a runner") == runner_bytes;

    let committed = parse("committed.json");
    assert_eq!(
        committed["run_started_sha256"],
        format!("sha256:{first_digest}"),
        "{case}: committed.json names the digest of the committed first line"
    );
    assert_eq!(committed["run_id"], run_id, "{case}");
    assert_eq!(
        committed["incarnation"], started["data"]["incarnation"],
        "{case}"
    );

    let owner = parse("owner.json");
    assert!(
        same_runner(&owner["runner"]),
        "{case}: owner.json records the runner run_started(4) records"
    );
    assert_eq!(owner["run_id"], run_id, "{case}");
    assert_eq!(
        owner["incarnation"], started["data"]["incarnation"],
        "{case}"
    );

    let resumed: Vec<&serde_json::Value> = events
        .iter()
        .filter(|event| event["event"] == "run_resumed")
        .collect();
    assert_eq!(
        resumed.is_empty(),
        parked.is_none(),
        "{case}: a resumed run records its resume, a fresh one none"
    );
    assert!(
        resumed
            .iter()
            .all(|event| same_runner(&event["data"]["runner"])),
        "{case}: every epoch records the runner run_started(4) records"
    );

    let report_bytes = bytes_of("report.json");
    let report = parse("report.json");
    assert!(
        same_runner(&report["runner"]),
        "{case}: report.json names the runner run_started(4) records"
    );
    assert_eq!(report["run_id"], run_id, "{case}");
    assert_eq!(report["outcome"], "complete", "{case}");
    let projected = crate::engine::topology::observe::observe(repo, run_id)
        .expect("observes")
        .expect("a schema-4 log")
        .report()
        .expect("projects");
    assert!(
        projected.is_fresh_against(report_bytes),
        "{case}: report.json is the projection of the committed log"
    );

    let merged: Vec<String> = events
        .iter()
        .filter(|event| event["event"] == "task_merged")
        .map(|event| {
            event["data"]["merged_sha"]
                .as_str()
                .expect("a merged sha")
                .to_owned()
        })
        .collect();
    assert_eq!(merged.len(), 2, "{case}: both tasks merged: {merged:?}");
    let integration_ref = started["data"]["integration_ref"]
        .as_str()
        .expect("run_started names its integration ref");
    assert_eq!(integration_ref, format!("refs/heads/upstroke/run-{run_id}"));
    let at_ref = git_in(repo, &["rev-parse", integration_ref])
        .trim()
        .to_owned();
    assert_eq!(
        Some(&at_ref),
        merged.last(),
        "{case}: the recorded integration ref holds the last merge the log records"
    );
    let last = events.last().expect("a last event");
    assert_eq!(last["event"], "run_finished", "{case}");
    assert_eq!(last["data"]["outcome"], "complete", "{case}");

    if let Some(parked) = parked {
        assert!(
            log.len() > parked.log_bytes
                && host_acceptance_sha256(&log[..parked.log_bytes]) == parked.log_sha256,
            "{case}: the resumed log continues the parked log byte for byte"
        );
        assert_eq!(
            host_acceptance_sha256(bytes_of("owner.json")),
            parked.owner_sha256,
            "{case}: owner.json is the record the run was created with, unchanged by the resume"
        );
    }

    let mut epochs: Vec<String> = vec![
        started["data"]["incarnation"]
            .as_str()
            .expect("run_started's incarnation")
            .to_owned(),
    ];
    epochs.extend(resumed.iter().map(|event| {
        event["data"]["incarnation"]
            .as_str()
            .expect("run_resumed's incarnation")
            .to_owned()
    }));
    let mut attempts: Vec<std::collections::BTreeSet<(u64, u64, u64)>> =
        vec![std::collections::BTreeSet::new(); epochs.len()];
    let mut epoch = 0;
    for event in &events {
        if event["event"] == "run_resumed" {
            epoch += 1;
        }
        if event["event"] == "attempt_started" {
            let number = |field: &str| {
                event["data"][field]
                    .as_u64()
                    .unwrap_or_else(|| panic!("{case}: attempt_started's {field}: {event}"))
            };
            attempts[epoch].insert((number("key"), number("generation"), number("attempt")));
        }
    }

    let snapshot = journal.snapshot();
    let runtimes = snapshot["runtimes"]
        .as_array()
        .expect("the journal's runtimes");
    let operations = snapshot["operations"]
        .as_array()
        .expect("the journal's operations");
    let mut runners: Vec<usize> = Vec::new();
    for call in &input.calls {
        let range = call["runtimes"].as_array().expect("a call's runtime range");
        let index = |at: usize| {
            range[at]
                .as_u64()
                .and_then(|index| usize::try_from(index).ok())
                .expect("a runtime index")
        };
        let (from, to) = (index(0), index(1));
        if call["entry"] == "answer::answer" {
            assert_eq!(
                from, to,
                "{case}: answer::answer constructed no container runtime"
            );
            continue;
        }
        assert_eq!(
            to - from,
            3,
            "{case}: {}: the public composition constructed three runtimes: {call}",
            call["entry"]
        );
        for (inspection, runtime) in runtimes.iter().enumerate().skip(from).take(2) {
            assert!(
                runtime["identity"].is_null(),
                "{case}: runtime {inspection} serves inspection, not a run's containers"
            );
        }
        assert_ne!(
            runtimes[from]["site"],
            runtimes[from + 1]["site"],
            "{case}: the two inspection runtimes were constructed at two sites"
        );
        let runner_runtime = &runtimes[from + 2];
        assert_eq!(
            runner_runtime["identity"]["run_id"],
            run_id,
            "{case}: runtime {} is this run's container runner",
            from + 2
        );
        for runtime in &runtimes[from..to] {
            assert!(
                runtime["site"]
                    .as_str()
                    .is_some_and(|site| site.starts_with("src/engine/topology/entry.rs:")),
                "{case}: every runtime was constructed by the schema-4 entry: {runtime}"
            );
        }
        runners.push(from + 2);
    }
    assert_eq!(
        runners.len(),
        epochs.len(),
        "{case}: one container runner per recorded epoch"
    );
    assert_eq!(
        input
            .calls
            .last()
            .and_then(|call| call["runtimes"][1].as_u64())
            .and_then(|to| usize::try_from(to).ok()),
        Some(runtimes.len()),
        "{case}: every runtime the journal holds was constructed by one of the case's calls"
    );

    for (index, runtime) in runtimes.iter().enumerate() {
        let runner_epoch = runners.iter().position(|runner| *runner == index);
        for op in ["create", "start", "stop", "remove"] {
            let traced = container_acceptance_traced(runtime, op);
            let journaled = container_acceptance_journaled(operations, index, op);
            assert_eq!(
                traced, journaled,
                "{case}: runtime {index}'s own trace and the journal name the same {op}s"
            );
            if runner_epoch.is_none() {
                assert!(
                    journaled.is_empty(),
                    "{case}: runtime {index} performed no {op}: {journaled:?}"
                );
            }
        }
    }

    let recorded = ContainerAcceptanceRecorded {
        case,
        run_id,
        image_id: &image_id,
        policy_digest: &policy_digest,
        operations,
    };
    let mut containers: Vec<serde_json::Value> = Vec::new();
    for (epoch, runner) in runners.iter().enumerate() {
        let incarnation = epochs[epoch].as_str();
        assert_eq!(
            runtimes[*runner]["identity"]["incarnation"], incarnation,
            "{case}: the epoch-{epoch} runner serves that epoch's incarnation"
        );
        let creates = container_acceptance_creates(&snapshot, *runner, *runner + 1);
        assert!(
            !creates.is_empty(),
            "{case}: epoch {epoch} created containers"
        );
        let validated: Vec<serde_json::Value> = creates
            .iter()
            .map(|create| container_acceptance_container(&recorded, create, epoch, incarnation))
            .collect();
        let roles = |role: &str| {
            validated
                .iter()
                .filter(|container| container["role"] == role)
                .count()
        };
        assert!(
            roles("probe-shell") >= 1 && roles("probe-agent") >= 1,
            "{case}: epoch {epoch} probed the shell and the agent inside the image"
        );
        let first_work = validated
            .iter()
            .filter(|container| {
                !container["role"]
                    .as_str()
                    .is_some_and(|role| role.starts_with("probe-"))
            })
            .filter_map(|container| container["sequence"][0].as_u64())
            .min();
        let last_probe = validated
            .iter()
            .filter(|container| {
                container["role"]
                    .as_str()
                    .is_some_and(|role| role.starts_with("probe-"))
            })
            .filter_map(|container| container["sequence"][3].as_u64())
            .max();
        assert!(
            first_work.is_none() || last_probe < first_work,
            "{case}: epoch {epoch}'s probes ran and were removed before its first work container"
        );
        let workers: std::collections::BTreeSet<(u64, u64, u64)> = validated
            .iter()
            .filter(|container| container["role"] == "worker")
            .map(|container| {
                let invocation = container["invocation"].as_str().expect("an invocation");
                let number = |prefix: char, part: &str| {
                    part.strip_prefix(prefix)
                        .and_then(|digits| digits.parse::<u64>().ok())
                        .unwrap_or_else(|| panic!("{case}: `{invocation}`'s {prefix} field"))
                };
                let parts: Vec<&str> = invocation.split('.').collect();
                (
                    number('k', parts[0]),
                    number('g', parts[1]),
                    number('a', parts[2]),
                )
            })
            .collect();
        assert_eq!(
            workers, attempts[epoch],
            "{case}: epoch {epoch}'s worker containers are exactly its recorded attempts"
        );
        containers.extend(validated);
    }

    let count = |role: &str| {
        containers
            .iter()
            .filter(|container| container["role"] == role)
            .count()
    };
    let outputs = witness.outputs.lock().expect("container witness").clone();
    let witnessed = |kind: &str| -> Vec<&serde_json::Value> {
        outputs
            .iter()
            .filter(|output| output["kind"] == kind)
            .collect()
    };
    let planned_user = containers
        .first()
        .map(|container| container["user"].clone())
        .expect("a container");
    assert!(
        containers
            .iter()
            .all(|container| container["user"] == planned_user),
        "{case}: every container ran as one user"
    );
    let ids = |fields: &std::collections::BTreeMap<String, String>| {
        format!(
            "{}:{}",
            fields.get("uid").map_or("?", String::as_str),
            fields.get("gid").map_or("?", String::as_str)
        )
    };
    let probe_outputs = witnessed("probe");
    assert_eq!(
        probe_outputs.len(),
        count("probe-agent"),
        "{case}: the adapter saw one output per agent probe container"
    );
    let mut observed_ids = std::collections::BTreeSet::new();
    for output in &probe_outputs {
        let fields = container_acceptance_fields(
            output["stdout"].as_str().unwrap_or_default(),
            CONTAINER_ACCEPTANCE_PROBE,
        );
        assert_eq!(
            fields.get("system").map(String::as_str),
            Some("Linux"),
            "{case}: {output}"
        );
        assert_eq!(
            fields.get("root_write").map(String::as_str),
            Some("refused"),
            "{case}: {output}"
        );
        assert_eq!(
            fields.get("dir_write").map(String::as_str),
            Some("refused"),
            "{case}: {output}"
        );
        assert_eq!(
            fields.get("file").map(String::as_str),
            Some(CONTAINER_ACCEPTANCE_FILE_TEXT),
            "{case}: {output}"
        );
        observed_ids.insert(ids(&fields));
    }
    let worker_outputs = witnessed("worker");
    assert_eq!(
        worker_outputs.len(),
        count("worker"),
        "{case}: the adapter saw one output per worker container"
    );
    let mut heads = Vec::new();
    for output in &worker_outputs {
        let fields = container_acceptance_fields(
            output["stdout"].as_str().unwrap_or_default(),
            CONTAINER_ACCEPTANCE_WORKER,
        );
        assert_eq!(
            fields.get("readme").map(String::as_str),
            Some("seed"),
            "{case}: {output}"
        );
        assert_eq!(
            fields.get("file").map(String::as_str),
            Some(CONTAINER_ACCEPTANCE_FILE_TEXT),
            "{case}: {output}"
        );
        let head = fields.get("head").cloned().unwrap_or_default();
        assert!(
            head.len() == 40 && head.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "{case}: the worker's Git view names a commit: {output}"
        );
        assert_eq!(
            git_in(repo, &["cat-file", "-t", &head]).trim(),
            "commit",
            "{case}: the commit the container's view names is this repository's"
        );
        heads.push(serde_json::json!({
            "head": head,
            "alternates": fields.get("alternates"),
        }));
        observed_ids.insert(ids(&fields));
    }
    let review_outputs = witnessed("review");
    assert_eq!(
        review_outputs.len(),
        count("review"),
        "{case}: the adapter saw one output per review container"
    );
    for output in &review_outputs {
        let fields = container_acceptance_fields(
            output["stdout"].as_str().unwrap_or_default(),
            CONTAINER_ACCEPTANCE_REVIEW,
        );
        assert_eq!(
            fields.get("workspace_write").map(String::as_str),
            Some("refused"),
            "{case}: {output}"
        );
        assert_eq!(
            fields.get("dir_write").map(String::as_str),
            Some("refused"),
            "{case}: {output}"
        );
        assert_eq!(
            fields.get("file").map(String::as_str),
            Some(CONTAINER_ACCEPTANCE_FILE_TEXT),
            "{case}: {output}"
        );
        observed_ids.insert(ids(&fields));
    }
    if let Some(user) = planned_user.as_str().filter(|user| !user.is_empty()) {
        assert_eq!(
            observed_ids.iter().map(String::as_str).collect::<Vec<_>>(),
            vec![user],
            "{case}: every container's processes ran as the planned user"
        );
    }

    let workers_file = fixture.directory.join("workers.txt");
    let gates_file = fixture.directory.join("gates.txt");
    assert_eq!(
        container_acceptance_lines(&workers_file),
        count("worker"),
        "{case}: every worker container wrote once through the configured directory mount"
    );
    assert_eq!(
        container_acceptance_lines(&gates_file),
        count("gate"),
        "{case}: every gate container wrote once through the configured directory mount"
    );
    for refused in ["review-write", "probe-write"] {
        assert!(
            !fixture.directory.join(refused).exists(),
            "{case}: no read-only role wrote `{refused}`"
        );
    }
    assert_eq!(
        fs::read_to_string(&fixture.file).expect("the configured file"),
        format!("{CONTAINER_ACCEPTANCE_FILE_TEXT}\n"),
        "{case}: the read-only file mount is unchanged"
    );
    let tree_files = git_in(repo, &["ls-tree", "-r", "--name-only", integration_ref]);
    assert!(
        tree_files
            .lines()
            .all(|path| !path.ends_with(CONTAINER_ACCEPTANCE_REVIEW_WRITE)),
        "{case}: no reviewer's write reached the integration branch"
    );

    let mut embedded = serde_json::Map::new();
    for (name, half, bytes) in &files {
        embedded.insert(
            (*name).to_owned(),
            serde_json::json!({
                "half": half,
                "bytes": bytes.len(),
                "sha256": host_acceptance_sha256(bytes),
                "utf8": std::str::from_utf8(bytes)
                    .unwrap_or_else(|error| panic!("{case}: {name} is UTF-8: {error}")),
            }),
        );
    }
    serde_json::json!({
        "case": case,
        "run_id": run_id,
        "calls": input.calls,
        "outcome": "complete",
        "branch": started["data"]["branch"],
        "integration_ref": integration_ref,
        "integration_ref_sha": at_ref,
        "merged_shas": merged,
        "base_sha": started["data"]["base_sha"],
        "runner": runner,
        "runner_policy_sha256": policy_digest,
        "owner_published_at_each_adapter_probe": probes,
        "first_line": {
            "text": std::str::from_utf8(first_line).expect("the first line is UTF-8"),
            "bytes": first_line.len(),
            "sha256": first_digest,
        },
        "committed_run_started_sha256": committed["run_started_sha256"],
        "epochs": epochs,
        "report_fresh": true,
        "parked_log": parked.map(|parked| serde_json::json!({
            "bytes": parked.log_bytes,
            "sha256": parked.log_sha256,
        })),
        "owner_sha256_at_park": parked.map(|parked| parked.owner_sha256.clone()),
        "containers": containers,
        "container_counts": {
            "total": containers.len(),
            "probe_shell": count("probe-shell"),
            "probe_agent": count("probe-agent"),
            "worker": count("worker"),
            "gate": count("gate"),
            "review": count("review"),
        },
        "witness": {
            "outputs": outputs,
            "worker_views": heads,
            "process_ids": observed_ids,
        },
        "host_effects": {
            "workers_txt_lines": container_acceptance_lines(&workers_file),
            "gates_txt_lines": container_acceptance_lines(&gates_file),
            "workers_txt_owner": container_acceptance_owner(&workers_file),
            "configured_file_owner": container_acceptance_owner(&fixture.file),
            "read_only_writes_absent": ["review-write", "probe-write"],
        },
        "journal": snapshot,
        "files": embedded,
    })
}

/// A complete fresh schema-4 container run: `engine::run_with` over the two-task plan.
fn container_acceptance_fresh(image: &str) -> serde_json::Value {
    let (tree, repo) = temp_engine_repo("container-acceptance-fresh");
    let fixture = container_acceptance_fixture(tree.path(), &repo, image);
    let witness = std::sync::Arc::new(ContainerWitness::new(0, private_root_for(&repo)));
    let source = SharedContainerWitness(std::sync::Arc::clone(&witness));
    let journal = crate::runner::container::observed::Journal::default();
    let report = {
        let _armed = crate::engine::topology::entry::container_runtime::arm(&journal);
        run_with(&topology_options(&repo), &source)
            .expect("the fresh schema-4 container run completes")
    };
    let constructed = journal.runtimes();
    until_released(&repo, &report.run_id);
    let end_state = container_acceptance_end_state("fresh", &repo, &journal, 0, constructed);
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:#?}");
    assert!(
        committed(&report, "t1") && committed(&report, "t2"),
        "{report:#?}"
    );
    assert_merged_on_the_integration_branch(&repo, &report);
    let probes = witness
        .owner_at_probe
        .lock()
        .expect("container witness")
        .len();
    container_acceptance_run(&ContainerAcceptanceCase {
        case: "fresh",
        repo: &repo,
        run_id: &report.run_id,
        witness: &witness,
        journal: &journal,
        calls: vec![serde_json::json!({
            "entry": "engine::run_with",
            "outcome": "complete",
            "adapter_probes": probes,
            "runtimes": [0, constructed],
            "end_state": end_state,
        })],
        fixture: &fixture,
        parked: None,
    })
}

/// A separate container run that parks on its one question, is answered through
/// `answer::answer` and is finished by `engine::resume_with`: two epochs of one
/// run id, each released before the next command and before the records are read.
fn container_acceptance_resumed(image: &str) -> serde_json::Value {
    let (tree, repo) = temp_engine_repo("container-acceptance-resumed");
    let fixture = container_acceptance_fixture(tree.path(), &repo, image);
    let witness = std::sync::Arc::new(ContainerWitness::new(1, private_root_for(&repo)));
    let source = SharedContainerWitness(std::sync::Arc::clone(&witness));
    let journal = crate::runner::container::observed::Journal::default();
    let parked = {
        let _armed = crate::engine::topology::entry::container_runtime::arm(&journal);
        run_with(&topology_options(&repo), &source).expect("the container run parks")
    };
    let at_park = journal.runtimes();
    until_released(&repo, &parked.run_id);
    let parked_end = container_acceptance_end_state("parked", &repo, &journal, 0, at_park);
    assert_eq!(parked.outcome(), RunOutcome::Parked, "{parked:#?}");
    let [asked] = parked.questions.as_slice() else {
        panic!("one question: {parked:#?}");
    };
    let question = asked.question.id.to_string();
    let parked_probes = witness
        .owner_at_probe
        .lock()
        .expect("container witness")
        .len();
    let log_at_park =
        fs::read(rundir::public_dir(&repo, &parked.run_id).join("events.jsonl")).expect("the log");
    let owner_at_park = fs::read(private_dir_of(&repo, &parked.run_id).join("owner.json"))
        .expect("owner.json at the park");

    crate::answer::answer(
        &repo,
        &question,
        crate::answer::Reply::Text("the first".to_owned()),
    )
    .expect("the answer is recorded");
    until_released(&repo, &parked.run_id);
    let at_answer = journal.runtimes();
    let mut resume = resume_options(&repo, &parked.run_id);
    resume.private_root = None;
    let report = {
        let _armed = crate::engine::topology::entry::container_runtime::arm(&journal);
        resume_with(&resume, &source).expect("the resume completes the container run")
    };
    let at_resume = journal.runtimes();
    until_released(&repo, &report.run_id);
    let resumed_end =
        container_acceptance_end_state("resumed", &repo, &journal, at_answer, at_resume);
    assert_eq!(report.run_id, parked.run_id, "one run across both epochs");
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:#?}");
    assert!(
        committed(&report, "t1") && committed(&report, "t2"),
        "{report:#?}"
    );
    assert_merged_on_the_integration_branch(&repo, &report);
    let probes = witness
        .owner_at_probe
        .lock()
        .expect("container witness")
        .len();
    container_acceptance_run(&ContainerAcceptanceCase {
        case: "parked-answered-resumed",
        repo: &repo,
        run_id: &report.run_id,
        witness: &witness,
        journal: &journal,
        calls: vec![
            serde_json::json!({
                "entry": "engine::run_with",
                "outcome": "parked",
                "question": question,
                "adapter_probes": parked_probes,
                "runtimes": [0, at_park],
                "end_state": parked_end,
            }),
            serde_json::json!({
                "entry": "answer::answer",
                "question": question,
                "reply": "the first",
                "runtimes": [at_park, at_answer],
            }),
            serde_json::json!({
                "entry": "engine::resume_with",
                "outcome": "complete",
                "adapter_probes": probes - parked_probes,
                "runtimes": [at_answer, at_resume],
                "end_state": resumed_end,
            }),
        ],
        fixture: &fixture,
        parked: Some(&ContainerAcceptanceParked {
            log_bytes: log_at_park.len(),
            log_sha256: host_acceptance_sha256(&log_at_park),
            owner_sha256: host_acceptance_sha256(&owner_at_park),
        }),
    })
}

/// The container record's bytes in [`host_acceptance_frame`]'s line shape, under
/// [`CONTAINER_ACCEPTANCE_MARKER`] so that neither record's reader takes the other's lines.
fn container_acceptance_frame(record: &[u8]) -> String {
    let digest = host_acceptance_sha256(record);
    let tag = &digest[..16];
    let chunks: Vec<&[u8]> = record.chunks(HOST_ACCEPTANCE_CHUNK).collect();
    let summary = format!("{tag} {} {} {digest}", chunks.len(), record.len());
    let mut lines = vec![format!(
        "{CONTAINER_ACCEPTANCE_MARKER} {CONTAINER_ACCEPTANCE_FRAMING} BEGIN {summary}"
    )];
    lines.extend(chunks.iter().enumerate().map(|(index, chunk)| {
        format!(
            "{CONTAINER_ACCEPTANCE_MARKER} {CONTAINER_ACCEPTANCE_FRAMING} {tag} {} {}",
            index + 1,
            host_acceptance_hex(chunk)
        )
    }));
    lines.push(format!(
        "{CONTAINER_ACCEPTANCE_MARKER} {CONTAINER_ACCEPTANCE_FRAMING} END {summary}"
    ));
    lines.join("\n") + "\n"
}

/// G7's native CONTAINER acceptance record. Two schema-4 runs whose `[runner]`
/// selects the container boundary, through the public entry points with the
/// deterministic stub adapter under the image's `sh`: a complete fresh run, and
/// a separate run parked, answered and resumed. A reachable runtime holding the
/// image named by [`CONTAINER_ACCEPTANCE_IMAGE`] is required, never skipped.
/// Every assertion holds of both runs before a byte is printed; printing is not
/// the verdict, which needs this test's pass line, the command's zero exit and a
/// complete reconstruction whose digests all agree.
#[test]
#[ignore = "native container acceptance record: run only by the platform capture recipes, which \
            select it by its exact name with a reachable Docker daemon and the image named by \
            UPSTROKE_CONTAINER_ACCEPTANCE_IMAGE"]
fn container_acceptance_record() {
    use std::io::Write as _;

    let image = std::env::var(CONTAINER_ACCEPTANCE_IMAGE).unwrap_or_else(|_| {
        panic!(
            "{CONTAINER_ACCEPTANCE_IMAGE} must name the image reference this record runs from; \
             it is never defaulted, pulled or skipped"
        )
    });
    crate::runner::container::observed::reachable().unwrap_or_else(|reason| {
        panic!("a reachable container runtime is required and was not available: {reason}")
    });
    let daemon = crate::runner::container::observed::daemon();
    let info = &daemon["info"]["parsed"]["value"];
    assert_eq!(
        info["OSType"], "linux",
        "the daemon runs Linux containers: {daemon}"
    );
    let image_answer = crate::runner::container::observed::image(&image);
    let image_id = image_answer["parsed"]["value"]["Id"]
        .as_str()
        .unwrap_or_else(|| {
            panic!("the runtime holds `{image}` already; nothing is pulled: {image_answer}")
        })
        .to_owned();

    let fresh = container_acceptance_fresh(&image);
    let resumed = container_acceptance_resumed(&image);
    assert_ne!(fresh["run_id"], resumed["run_id"], "two runs, two run ids");
    for run in [&fresh, &resumed] {
        assert_eq!(
            run["runner"]["image"]["id"], image_id,
            "{}: the recorded image id is the one the runtime reports for `{image}`",
            run["case"]
        );
    }
    let mut manifest = Vec::new();
    for run in [&fresh, &resumed] {
        for (name, half) in HOST_ACCEPTANCE_FILES {
            let file = &run["files"][name];
            manifest.push(serde_json::json!({
                "case": run["case"],
                "run_id": run["run_id"],
                "file": name,
                "half": half,
                "bytes": file["bytes"],
                "sha256": file["sha256"],
            }));
        }
    }
    let os = std::env::consts::OS;
    let statement = format!(
        "a native {os} coordinator (this test process) drove the {} Docker daemon `{}` ({}, kernel {}, \
         {}); every container ran in that daemon's environment, never natively on {os}",
        info["OSType"].as_str().unwrap_or("?"),
        info["Name"].as_str().unwrap_or("?"),
        info["OperatingSystem"].as_str().unwrap_or("?"),
        info["KernelVersion"].as_str().unwrap_or("?"),
        info["Architecture"].as_str().unwrap_or("?"),
    );
    let record = serde_json::json!({
        "format": "upstroke-container-acceptance-record",
        "format_version": 1,
        "test": "engine::tests::container_acceptance_record",
        "harness": host_acceptance_harness(),
        "ci": host_acceptance_ci(),
        "platform": {
            "coordinator": {
                "os": os,
                "arch": std::env::consts::ARCH,
                "family": std::env::consts::FAMILY,
            },
            "daemon": daemon,
            "statement": statement,
        },
        "image": {
            "variable": CONTAINER_ACCEPTANCE_IMAGE,
            "reference": image,
            "id": image_id,
            "inspect": image_answer,
        },
        "scope": {
            "runner": "container: the production ContainerRunner over the production runtime \
                       created every probe, worker, gate and reviewer container from the recorded \
                       image id; the observer wrapped each runtime the schema-4 entry constructed \
                       and delegated every operation to it",
            "observer": "engine::topology::entry::container_runtime, armed by this test on its own \
                         thread for each public call, journaled every runtime the entry \
                         constructed, every operation, the planned spec and on-disk intent of every \
                         container, and the daemon's inspection at its creation and before its \
                         removal; each runtime's own trace is compared with the journal",
            "provider": "the deterministic stub adapter (engine::tests::ContainerWitness over \
                         TopologyStub's parse) under the image's sh; no provider was called",
            "coordinator": "this test process called engine::run_with, answer::answer and \
                            engine::resume_with; it is not the upstroke CLI process, and no CLI \
                            process was restarted",
            "acceptance": "printed only after every assertion passed; accepted only with the named \
                           test's pass line, exit 0, complete reconstruction and matching digests",
            "not_claimed": "kills, crashes, power loss, reaper or containment behaviour, exhaustive \
                            filesystem or process cleanup, and real providers",
        },
        "runs": [fresh, resumed],
        "manifest": manifest,
    });
    let bytes = serde_json::to_vec(&record).expect("the record serializes");
    let mut stdout = std::io::stdout().lock();
    stdout
        .write_all(container_acceptance_frame(&bytes).as_bytes())
        .expect("the record reaches stdout");
    stdout.flush().expect("the record is flushed");
}

/// The container-runtime seam observes only while armed, only on the arming
/// thread, and only by wrapping what production constructs; unarmed, and on any
/// other thread, it constructs exactly what production does and journals nothing.
#[test]
fn the_container_runtime_seam_observes_only_while_armed_and_only_on_its_own_thread() {
    use crate::engine::topology::entry::container_runtime;

    let journal = crate::runner::container::observed::Journal::default();
    drop(container_runtime::for_inspection());
    assert_eq!(
        journal.runtimes(),
        0,
        "an unarmed construction is not observed"
    );
    {
        let _armed = container_runtime::arm(&journal);
        drop(container_runtime::for_inspection());
        assert_eq!(
            journal.runtimes(),
            1,
            "an armed construction on this thread is observed"
        );
        std::thread::spawn(|| drop(container_runtime::for_inspection()))
            .join()
            .expect("the other thread constructs a runtime");
        assert_eq!(
            journal.runtimes(),
            1,
            "another thread's construction is not"
        );
        let identity = crate::runner::container::exec::RunIdentity {
            private_root: PathBuf::from("private-root"),
            run_id: "run".to_owned(),
            run_dir: PathBuf::from("run-dir"),
            incarnation: "incarnation".to_owned(),
            repo_key: "repo-key".to_owned(),
        };
        drop(container_runtime::for_runner(&identity));
        assert_eq!(
            journal.runtimes(),
            2,
            "a runner construction is observed with its identity"
        );
    }
    drop(container_runtime::for_inspection());
    assert_eq!(
        journal.runtimes(),
        2,
        "nothing is observed once the guard has dropped"
    );
    let snapshot = journal.snapshot();
    let runtimes = snapshot["runtimes"]
        .as_array()
        .expect("the journal's runtimes");
    assert!(
        runtimes.iter().all(|runtime| runtime["site"]
            .as_str()
            .is_some_and(|site| site.starts_with("src/engine/tests.rs:"))),
        "each construction names the call site that asked for it: {runtimes:?}"
    );
    assert!(
        runtimes[0]["identity"].is_null(),
        "an inspection runtime serves no run"
    );
    assert_eq!(runtimes[1]["identity"]["run_id"], "run");
    assert_eq!(runtimes[1]["identity"]["incarnation"], "incarnation");
    assert!(
        snapshot["operations"].as_array().is_some_and(Vec::is_empty),
        "constructing a runtime performs no operation"
    );
}

/// What one path under a private root is: its kind, its permission bits and its bytes or target.
#[derive(Debug, Clone, PartialEq, Eq)]
enum PrivateEntry {
    Directory { mode: u32 },
    File { mode: u32, bytes: Vec<u8> },
    Link { target: PathBuf },
}

/// Everything persistent under `root`, the root itself included, by path relative to it.
fn private_snapshot(root: &Path) -> std::collections::BTreeMap<PathBuf, PrivateEntry> {
    snapshot_under(root, false, "")
}

fn private_snapshot_in(
    root: &Path,
    case: &str,
) -> std::collections::BTreeMap<PathBuf, PrivateEntry> {
    snapshot_under(root, false, case)
}

fn snapshot_under(
    top: &Path,
    outside_git: bool,
    case: &str,
) -> std::collections::BTreeMap<PathBuf, PrivateEntry> {
    fn mode(metadata: &fs::Metadata) -> u32 {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            metadata.permissions().mode()
        }
        #[cfg(not(unix))]
        {
            u32::from(metadata.permissions().readonly())
        }
    }
    let at = if case.is_empty() {
        String::new()
    } else {
        format!("{case}: ")
    };
    let failed = |what: &str, path: &Path, error: &std::io::Error| {
        format!(
            "{at}the snapshot of {}: {what} {}: {error}",
            top.display(),
            path.display()
        )
    };
    let mut entries = std::collections::BTreeMap::new();
    let first = fs::symlink_metadata(top)
        .unwrap_or_else(|error| panic!("{}", failed("the metadata of", top, &error)));
    entries.insert(
        PathBuf::new(),
        PrivateEntry::Directory { mode: mode(&first) },
    );
    let mut pending = vec![top.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let listed = fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("{}", failed("listing", &directory, &error)));
        for entry in listed {
            let path = entry
                .unwrap_or_else(|error| panic!("{}", failed("an entry of", &directory, &error)))
                .path();
            let relative = path
                .strip_prefix(top)
                .expect("under the snapshot's top")
                .to_path_buf();
            if outside_git && relative == Path::new(".git") {
                continue;
            }
            let metadata = fs::symlink_metadata(&path)
                .unwrap_or_else(|error| panic!("{}", failed("the metadata of", &path, &error)));
            let found = if metadata.file_type().is_symlink() {
                PrivateEntry::Link {
                    target: fs::read_link(&path).unwrap_or_else(|error| {
                        panic!("{}", failed("the target of", &path, &error))
                    }),
                }
            } else if metadata.is_dir() {
                pending.push(path.clone());
                PrivateEntry::Directory {
                    mode: mode(&metadata),
                }
            } else {
                PrivateEntry::File {
                    mode: mode(&metadata),
                    bytes: fs::read(&path)
                        .unwrap_or_else(|error| panic!("{}", failed("reading", &path, &error))),
                }
            };
            entries.insert(relative, found);
        }
    }
    entries
}

/// The paths whose entry differs between two snapshots, each with how, and never its bytes.
fn moved_entries(
    before: &std::collections::BTreeMap<PathBuf, PrivateEntry>,
    after: &std::collections::BTreeMap<PathBuf, PrivateEntry>,
) -> Vec<String> {
    let mut moved = Vec::new();
    for (path, entry) in after {
        match before.get(path) {
            None => moved.push(format!("created {}", path.display())),
            Some(earlier) if earlier != entry => moved.push(format!("changed {}", path.display())),
            Some(_) => {}
        }
    }
    for path in before.keys().filter(|path| !after.contains_key(*path)) {
        moved.push(format!("removed {}", path.display()));
    }
    moved
}

fn every_ref(repo: &Path) -> String {
    git_in(repo, &["for-each-ref", "--format=%(refname) %(objectname)"])
}

#[derive(Debug, Clone, Copy)]
enum IncludeState {
    Absent,
    Changed,
}

/// Leaves `root`'s include absent — its whole `git` directory gone — or holding other bytes in
/// a directory a writer would tighten, so that publishing it is visible in every path, byte and
/// mode it touches.
fn put_include(root: &Path, state: IncludeState) {
    let directory = root.join("git");
    match state {
        IncludeState::Absent => {
            if directory.exists() {
                fs::remove_dir_all(&directory).expect("the include's directory, removed");
            }
        }
        IncludeState::Changed => {
            fs::create_dir_all(&directory).expect("the include's directory");
            fs::write(
                directory.join("recorded-objects.gitconfig"),
                "[core]\n\tuseReplaceRefs = true\n",
            )
            .expect("an include holding other bytes");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;
                fs::set_permissions(&directory, fs::Permissions::from_mode(0o755))
                    .expect("a directory the include's writer would tighten");
            }
        }
    }
}

#[cfg(unix)]
#[test]
#[ignore = "spawned by the recorded-shell guards, in a process whose PATH holds a fake shell"]
fn recorded_shell_helper() {
    let Some(repo) = std::env::var_os("UPSTROKE_RECORDED_SHELL_REPO") else {
        return;
    };
    let repo = PathBuf::from(repo);
    let config = PathBuf::from(
        std::env::var_os("UPSTROKE_RECORDED_SHELL_CONFIG").expect("the helper's config"),
    );
    let source = SharedTopologyStubs(std::sync::Arc::new(TopologyStub::new(
        TOPOLOGY_STUB_EDIT,
        1,
    )));
    match std::env::var("UPSTROKE_RECORDED_SHELL_MODE").as_deref() {
        Ok("create") => {
            let mut opts = topology_options(&repo);
            opts.config_path = Some(config);
            let parked = run_with(&opts, &source).expect("the run is created and parks");
            assert_eq!(parked.outcome(), RunOutcome::Parked, "{parked:#?}");
        }
        Ok("resume") => {
            let run_id = rundir::latest_run(&repo).expect("the parked run");
            let resumed =
                |kinds: Vec<String>| kinds.iter().filter(|kind| *kind == "run_resumed").count();
            let before = resumed(event_kinds(&repo, &run_id));
            let mut resume = resume_options(&repo, &run_id);
            resume.config_path = Some(config);
            resume_with(&resume, &source)
                .expect("the recorded shell runs again, so the run resumes");
            assert_eq!(resumed(event_kinds(&repo, &run_id)), before + 1);
        }
        other => panic!("the recorded-shell helper has no mode {other:?}"),
    }
}

/// The file the signal helper's workers write their pids to, beside the repository.
#[cfg(unix)]
fn signalled_workers(repo: &Path) -> PathBuf {
    repo.with_file_name("worker-pids")
}

#[cfg(unix)]
#[test]
#[ignore = "subprocess helper"]
fn topology_signalled_child() {
    let Some(repo) = std::env::var_os("UPSTROKE_TOPOLOGY_SIGNAL_REPO").map(PathBuf::from) else {
        return;
    };
    // Each worker writes its pid where the parent finds it, then sleeps as before.
    let workers = signalled_workers(&repo);
    let stub = TopologyStub::new(&format!("echo $$ >> '{}'; sleep 30", workers.display()), 0);
    let _ = run_with(&topology_options(&repo), &BorrowedTopologyStub(&stub));
}

#[cfg(unix)]
fn announcing_worker(gate: &Path) -> String {
    let gate = gate.display();
    format!(
        "case \"$(basename \"$PWD\")\" in \
         k1-*) d=asks; echo {TOPOLOGY_STUB_ASKS} ;; \
         k2-*) d=waits; : > '{gate}/waits-started'; i=0; \
         while [ ! -e '{gate}/release' ] && [ \"$i\" -lt 4800 ]; do sleep 0.05; i=$((i+1)); done; \
         [ -e '{gate}/release' ] || exit 7 ;; \
         *) d=first ;; \
         esac; mkdir -p \"$d\" && echo edited by the topology stub>> \"$d/out.txt\""
    )
}

#[cfg(unix)]
#[test]
#[ignore = "subprocess helper"]
fn topology_announcing_child() {
    let Some(repo) = std::env::var_os("UPSTROKE_ANNOUNCE_REPO").map(PathBuf::from) else {
        return;
    };
    let gate = std::env::var_os("UPSTROKE_ANNOUNCE_GATE")
        .map(PathBuf::from)
        .expect("the parent names the gate");
    let step = std::env::var("UPSTROKE_ANNOUNCE_DRIVER").is_ok_and(|driver| driver == "step");
    let run_id = std::env::var("UPSTROKE_ANNOUNCE_RUN").unwrap_or_default();
    let stub = TopologyStub::new(&announcing_worker(&gate), 0);
    let shared = SharedTopologyStubs(std::sync::Arc::new(TopologyStub::new(
        &announcing_worker(&gate),
        0,
    )));
    let borrowed = BorrowedTopologyStub(&stub);
    let source: &dyn AdapterSource = if step { &borrowed } else { &shared };
    let report = if run_id.is_empty() {
        let mut opts = topology_options(&repo);
        opts.config_path = Some(repo.join("upstroke.toml"));
        opts.attempt_timeout = Duration::from_secs(600);
        run_with(&opts, source)
    } else {
        let mut resume = resume_options(&repo, &run_id);
        resume.private_root = None;
        resume.config_path = Some(repo.join("upstroke.toml"));
        resume.budget_usd = Some(10.0);
        resume.attempt_timeout = Duration::from_secs(600);
        resume_with(&resume, source)
    }
    .expect("the run ends");
    assert_eq!(report.outcome(), RunOutcome::Parked, "{report:#?}");
    assert_eq!(report.parked_tasks(), ["asks"], "{report:#?}");
    assert!(
        committed(&report, "first") && committed(&report, "waits"),
        "{report:#?}"
    );
}

/// A gate that builds a repository of its own in a temporary directory, installs
/// a blob replacement there, and passes only if Git honours it. Under a
/// process-wide replacement setting (`ObjectGraph::Recorded`, the old schema-4
/// host composition) it fails; under the managed repository's conditional
/// include (`ObjectGraph::RecordedIn`) it passes.
#[cfg(unix)]
const UNRELATED_REPOSITORY_GATE: &str = "\n[[gates]]\nname = \"unrelated-replacement\"\n\
     cmd = 'd=$(mktemp -d) && git -C \"$d\" init -q && printf \"one\\n\" > \"$d/f\" && \
     git -C \"$d\" add f && git -C \"$d\" -c user.name=gate -c user.email=gate@upstroke.local \
     commit -qm one && a=$(git -C \"$d\" rev-parse HEAD:f) && \
     b=$(printf \"replaced\\n\" | git -C \"$d\" hash-object -w --stdin) && \
     git -C \"$d\" replace \"$a\" \"$b\" && test \"$(git -C \"$d\" show HEAD:f)\" = replaced; \
     s=$?; rm -rf \"$d\"; exit $s'\n";

#[cfg(unix)]
#[test]
#[ignore = "subprocess helper"]
fn topology_object_graph_helper() {
    if std::env::var_os(crate::workspace_manager::fixture::REPLACEMENT_WITNESS).is_none() {
        return;
    }
    crate::workspace_manager::fixture::assert_replacement_controls_pinned("topology-object-graph");
    let config = format!(
        "[interaction]\nmode = \"never\"\n\n{V1_OBJECT_GRAPH_GATES}{UNRELATED_REPOSITORY_GATE}"
    );

    let (_tree, repo) = replaced_probe_repo("topographrun", WORKER_ONLY_PLAN, &config);
    let mut opts = topology_options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let stub = TopologyStub::new(TOPOLOGY_STUB_EDIT, 0);
    let report = run_with(&opts, &BorrowedTopologyStub(&stub)).expect("the fresh run");
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:?}");
    assert!(committed(&report, "t1"), "{report:?}");

    let (_tree, repo) = replaced_probe_repo("topographresume", WORKER_ONLY_PLAN, &config);
    let mut opts = topology_options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let stub = TopologyStub::new(TOPOLOGY_STUB_EDIT, 1);
    let parked = run_with(&opts, &BorrowedTopologyStub(&stub)).expect("the first run");
    assert_eq!(parked.outcome(), RunOutcome::Parked, "{parked:?}");
    until_released(&repo, &parked.run_id);
    let question = parked
        .questions
        .first()
        .expect("a question was raised")
        .question
        .id
        .to_string();
    crate::answer::answer(
        &repo,
        &question,
        crate::answer::Reply::Text("the first".to_owned()),
    )
    .expect("the answer");
    let mut resume = resume_options(&repo, &parked.run_id);
    resume.private_root = None;
    resume.config_path = Some(repo.join("upstroke.toml"));
    let resumed = resume_with(&resume, &BorrowedTopologyStub(&stub)).expect("the resume");
    assert_eq!(resumed.outcome(), RunOutcome::Complete, "{resumed:?}");
    assert!(committed(&resumed, "t1"), "{resumed:?}");
}

#[cfg(unix)]
const INHERITED_RUN_ID: &str = "UPSTROKE_INHERITED_RUN_ID";

#[cfg(unix)]
#[test]
#[ignore = "subprocess helper"]
fn relative_common_dir_topology_helper() {
    let Some(repo) = std::env::var_os(INHERITED_RUN_ROOT).map(PathBuf::from) else {
        return;
    };
    let private = PathBuf::from(std::env::var_os(INHERITED_RUN_PRIVATE).expect("the private root"));
    let mut opts = options(&repo);
    opts.legacy_writer = false;
    opts.config_path = Some(repo.join("upstroke.toml"));
    opts.private_root = Some(private);
    let stub = TopologyStub::new(TOPOLOGY_STUB_EDIT, 0);
    let report = run_with(&opts, &BorrowedTopologyStub(&stub)).expect("the run");
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:?}");
    assert!(committed(&report, "t1"), "{report:?}");
    assert_eq!(
        first_line_of_the_log(&repo, &report.run_id)["data"]["schema"],
        4,
        "a schema-4 run"
    );
}

#[cfg(unix)]
#[test]
#[ignore = "subprocess helper"]
fn relative_object_directory_topology_helper() {
    let Some(repo) = std::env::var_os(INHERITED_RUN_ROOT).map(PathBuf::from) else {
        return;
    };
    let run_id = std::env::var_os(INHERITED_RUN_ID)
        .and_then(|id| id.into_string().ok())
        .expect("the parked run's id");
    let names = |repo: &Path| {
        let mut names: Vec<String> = fs::read_dir(rundir::runs_root(repo))
            .expect("the runs root")
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    };
    let runs = names(&repo);
    let log = rundir::public_dir(&repo, &run_id).join("events.jsonl");
    let before = fs::read(&log).expect("the parked run's log");
    let stub = TopologyStub::new(TOPOLOGY_STUB_EDIT, 0);

    let refused = run_with(&topology_options(&repo), &BorrowedTopologyStub(&stub))
        .expect_err("a relative GIT_OBJECT_DIRECTORY refuses a fresh schema-4 run");
    let said = refused.to_string();
    assert!(
        said.contains("GIT_OBJECT_DIRECTORY") && !said.contains("already driving worktree"),
        "{said}"
    );
    assert_eq!(
        names(&repo),
        runs,
        "the refused run created a run directory"
    );

    let mut resume = resume_options(&repo, &run_id);
    resume.private_root = None;
    let refused = resume_with(&resume, &BorrowedTopologyStub(&stub))
        .expect_err("a relative GIT_OBJECT_DIRECTORY refuses the schema-4 resume");
    let said = refused.to_string();
    assert!(
        said.contains("GIT_OBJECT_DIRECTORY") && !said.contains("already driving worktree"),
        "{said}"
    );
    assert_eq!(
        fs::read(&log).expect("the log"),
        before,
        "the refused resume wrote"
    );
}

/// One pool, `claude-max`, for the topology stub's agent, in a file outside the checkout.
fn stub_pool_file(tree: &ScratchTree) -> PathBuf {
    let path = tree.path().join("pools.toml");
    fs::write(
        &path,
        "[pools.claude-max]\nkind = \"subscription-window\"\nagent = \"claude-code\"\n",
    )
    .expect("the pools file");
    path
}

#[derive(Debug, Clone, Copy)]
enum ExclusionState {
    Absent,
    Changed,
}

/// Leaves the checkout's run exclusion absent — `.upstroke` itself gone when nothing else is in
/// it — or holding bytes that exclude nothing, so that making it is visible in every path and
/// byte it touches.
fn put_exclusion(repo: &Path, state: ExclusionState) {
    let namespace = repo.join(".upstroke");
    let exclusion = namespace.join(".gitignore");
    match state {
        ExclusionState::Absent => {
            if exclusion.exists() {
                fs::remove_file(&exclusion).expect("the exclusion, removed");
            }
            if namespace.exists()
                && fs::read_dir(&namespace)
                    .expect("the namespace")
                    .next()
                    .is_none()
            {
                fs::remove_dir(&namespace).expect("the empty namespace, removed");
            }
        }
        ExclusionState::Changed => {
            fs::create_dir_all(&namespace).expect("the namespace");
            fs::write(&exclusion, "# excludes nothing\n").expect("an exclusion of other bytes");
        }
    }
}

/// Everything persistent in the checkout but its Git directory, the checkout itself included,
/// by path relative to it: the operator's files and the engine's `.upstroke` namespace, its
/// exclusion and every run's public half with its log. [`private_snapshot`]'s walk, without
/// `.git`, whose refs [`every_ref`] reads and whose lease file belongs to the lease's holder.
fn public_snapshot(repo: &Path) -> std::collections::BTreeMap<PathBuf, PrivateEntry> {
    snapshot_under(repo, true, "")
}

fn public_snapshot_in(
    repo: &Path,
    case: &str,
) -> std::collections::BTreeMap<PathBuf, PrivateEntry> {
    snapshot_under(repo, true, case)
}

/// With nothing holding the lease, `command` is admitted and makes the run exclusion and the
/// include from their absent states: the exclusion's fixed `*`, the include's fixed bytes, and
/// on Unix the include's directory at `0700`.
fn assert_admitted_and_published(
    repo: &Path,
    command: impl Fn() -> Result<RunReport, UpstrokeError>,
) -> RunReport {
    let home = private_root_for(repo);
    put_exclusion(repo, ExclusionState::Absent);
    put_include(&home, IncludeState::Absent);
    let report = command().expect("with the lease free the command is admitted");
    until_released(repo, &report.run_id);
    assert_eq!(
        fs::read(repo.join(".upstroke").join(".gitignore")).expect("the run exclusion"),
        b"*\n",
        "the admitted command made the run exclusion"
    );
    assert_eq!(
        fs::read(include_for(repo)).expect("the include"),
        crate::runner::host::RECORDED_OBJECTS_INCLUDE,
        "the admitted command published the include"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = fs::metadata(home.join("git"))
            .expect("the include's directory")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o700, "{mode:o}");
    }
    report
}

#[cfg(unix)]
const DEFAULT_ROOT_LEASE_REPO: &str = "UPSTROKE_DEFAULT_ROOT_LEASE_REPO";

#[cfg(unix)]
#[test]
#[ignore = "subprocess helper"]
fn default_root_lease_child() {
    let Some(repo) = std::env::var_os(DEFAULT_ROOT_LEASE_REPO).map(PathBuf::from) else {
        return;
    };
    let home = PathBuf::from(std::env::var_os("HOME").expect("the parent names the home"));
    let root = rundir::default_private_root();
    assert_eq!(
        root,
        home.join(".upstroke"),
        "the premise: the default root hangs from this home"
    );
    assert!(
        fs::symlink_metadata(&root).is_err(),
        "the premise: no default root exists yet"
    );
    let mut opts = options(&repo);
    opts.legacy_writer = false;
    opts.private_root = None;
    let stub = ObservedStub::new(0);
    let source = BorrowedObserved(&stub);

    let lease = WorktreeLock::acquire(&repo).expect("the test's lease");
    let checkout = public_snapshot(&repo);
    let homes = private_snapshot(&home);
    let refs = every_ref(&repo);
    let refused = run_with(&opts, &source).expect_err("the lease is held");
    let checkout_after = public_snapshot(&repo);
    let homes_after = private_snapshot(&home);
    let refs_after = every_ref(&repo);
    drop(lease);
    assert!(
        refused.to_string().contains("is already driving worktree"),
        "{refused}"
    );
    assert!(stub.built().is_empty(), "no worker or reviewer was built");
    let moved = moved_entries(&homes, &homes_after);
    assert!(
        moved.is_empty(),
        "the refused run created under the home: {moved:?}"
    );
    let moved = moved_entries(&checkout, &checkout_after);
    assert!(
        moved.is_empty(),
        "the refused run wrote in the checkout: {moved:?}"
    );
    assert_eq!(refs_after, refs, "the refs moved");

    let report = run_with(&opts, &source).expect("with the lease free the run is admitted");
    until_released(&repo, &report.run_id);
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:#?}");
    assert_eq!(
        fs::read(root.join("git").join("recorded-objects.gitconfig")).expect("the include"),
        crate::runner::host::RECORDED_OBJECTS_INCLUDE,
        "the admitted run created the default root through the include's publication"
    );
    assert_eq!(
        private_dir_of(&repo, &report.run_id),
        fs::canonicalize(&root)
            .expect("the default root, created")
            .join("runs")
            .join(&report.run_id),
        "the run's private half is under the default root it authorized once it existed"
    );
    assert_eq!(
        fs::read(repo.join(".upstroke").join(".gitignore")).expect("the run exclusion"),
        b"*\n"
    );
}

fn with_initial_read_failing<T>(log: &Path, call: impl FnOnce() -> T) -> (T, usize) {
    let fault = crate::engine::topology::entry::initial_log_read_fault::arm(log);
    let result = call();
    (result, fault.hits())
}

fn assert_the_initial_read_refused(refused: Option<UpstrokeError>, log: &Path, case: &str) {
    match refused {
        Some(UpstrokeError::Io { path, source }) => {
            assert_eq!(path, log, "{case}: the refusal names another path");
            assert_eq!(
                source.to_string(),
                crate::engine::topology::entry::initial_log_read_fault::INJECTED,
                "{case}: the refusal is not the failed read's own"
            );
        }
        other => panic!("{case}: not the failed initial read's own refusal: {other:?}"),
    }
}

#[cfg(unix)]
const DEFAULT_ROOT_UNREAD_REPO: &str = "UPSTROKE_DEFAULT_ROOT_UNREAD_REPO";

#[cfg(unix)]
const DEFAULT_ROOT_UNREAD_RUN: &str = "UPSTROKE_DEFAULT_ROOT_UNREAD_RUN";

#[cfg(unix)]
const DEFAULT_ROOT_UNREAD_DONE: &str =
    "default-root unread resume: refused in both default-root states, then admitted";

#[cfg(unix)]
#[test]
#[ignore = "subprocess helper"]
fn default_root_unread_resume_child() {
    let (Some(repo), Some(run_id)) = (
        std::env::var_os(DEFAULT_ROOT_UNREAD_REPO).map(PathBuf::from),
        std::env::var(DEFAULT_ROOT_UNREAD_RUN).ok(),
    ) else {
        return;
    };
    let home = PathBuf::from(std::env::var_os("HOME").expect("the parent names the home"));
    let root = rundir::default_private_root();
    assert_eq!(
        root,
        home.join(".upstroke"),
        "the premise: the default root hangs from this home"
    );
    assert!(
        fs::symlink_metadata(&root).is_err(),
        "the premise: no default root exists yet"
    );
    let log = rundir::public_dir(&repo, &run_id).join(rundir::EVENT_LOG);
    let mut resume = resume_options(&repo, &run_id);
    resume.private_root = None;
    let stub = ObservedStub::new(0);
    let source = BorrowedObserved(&stub);

    for state in ["no default root", "a default root holding another include"] {
        if state != "no default root" {
            put_include(&root, IncludeState::Changed);
        }
        let lease = WorktreeLock::acquire(&repo).expect("the test's lease");
        let homes = private_snapshot(&home);
        let checkout = public_snapshot(&repo);
        let refs = every_ref(&repo);
        let (refused, reads) = with_initial_read_failing(&log, || resume_with(&resume, &source));
        let homes_after = private_snapshot(&home);
        let checkout_after = public_snapshot(&repo);
        let refs_after = every_ref(&repo);
        drop(lease);
        let moved = moved_entries(&homes, &homes_after);
        assert!(
            moved.is_empty(),
            "{state}: the unread resume wrote under the home: {moved:?}"
        );
        let moved = moved_entries(&checkout, &checkout_after);
        assert!(
            moved.is_empty(),
            "{state}: the unread resume wrote in the checkout: {moved:?}"
        );
        assert_eq!(refs_after, refs, "{state}: the refs moved");
        assert!(
            stub.built().is_empty(),
            "{state}: a worker or reviewer was built"
        );
        assert_eq!(reads, 1, "{state}: the router's initial read failed once");
        assert_the_initial_read_refused(refused.err(), &log, state);
    }

    let homes = private_snapshot(&home);
    let report = assert_admitted_and_published(&repo, || resume_with(&resume, &source));
    assert_eq!(report.run_id, run_id);
    let moved = moved_entries(&homes, &private_snapshot(&home));
    assert!(
        moved.is_empty(),
        "read, the schema-4 resume wrote under today's default root, which is not its authority: \
         {moved:?}"
    );
    crate::workspace_manager::fixture::say(DEFAULT_ROOT_UNREAD_DONE);
}

const WORKTREE_LEASE_CONTENDER: &str = "engine::tests::worktree_lease_contender";

const CONTENDER_REPO: &str = "UPSTROKE_TEST_CONTENDER_REPO";

const CONTENDER_GIT_DIR: &str = "UPSTROKE_TEST_CONTENDER_GIT_DIR";

const CONTENDER_TOOK: &str = "worktree lease contender: took the lease and gave it back";

const CONTENDER_REFUSED: &str = "worktree lease contender: refused: ";

#[derive(Debug, PartialEq, Eq)]
enum Contended {
    Took,
    Refused(String),
}

fn contend_for_the_worktree_lease(
    repo: &Path,
    git_dir: &Path,
    scratch: &Path,
    case: &str,
) -> Contended {
    let exe = std::env::current_exe().expect("the test binary");
    let mut command = CommandSpec::new(exe.to_string_lossy().into_owned());
    command.args = [
        "--exact",
        WORKTREE_LEASE_CONTENDER,
        "--ignored",
        "--nocapture",
        "--test-threads",
        "1",
    ]
    .map(str::to_owned)
    .to_vec();
    command.env = vec![
        (
            CONTENDER_REPO.to_owned(),
            repo.to_string_lossy().into_owned(),
        ),
        (
            CONTENDER_GIT_DIR.to_owned(),
            git_dir.to_string_lossy().into_owned(),
        ),
    ];
    let request = crate::runner::gate_request(
        command,
        scratch.to_path_buf(),
        Duration::from_secs(120),
        crate::runner::InvocationId::attempt(
            crate::topology::registry::TaskKey(0),
            crate::topology::events::GenerationId(0),
            crate::topology::events::AttemptNumber(1),
            crate::runner::invocation::AttemptRole::Gate(0),
            0,
        ),
    );
    let output =
        crate::runner::Runner::run_blocking(&crate::runner::host::HostRunner::new(), &request)
            .unwrap_or_else(|error| panic!("{case}: the lease contender did not run: {error}"));
    assert!(
        !output.timed_out && output.code == Some(0) && output.stdout.contains("1 passed"),
        "{case}: the lease contender did not run to its end: {output:?}"
    );
    if output.stderr.contains(CONTENDER_TOOK) {
        return Contended::Took;
    }
    let refused = output
        .stderr
        .lines()
        .find_map(|line| line.strip_prefix(CONTENDER_REFUSED))
        .unwrap_or_else(|| panic!("{case}: the lease contender said neither: {output:?}"));
    Contended::Refused(refused.to_owned())
}

fn assert_the_lease_is(held: bool, repo: &Path, git_dir: &Path, scratch: &Path, case: &str) {
    let contended = contend_for_the_worktree_lease(repo, git_dir, scratch, case);
    if !held {
        assert_eq!(
            contended,
            Contended::Took,
            "{case}: another process could not take the free worktree lease"
        );
        return;
    }
    let Contended::Refused(refusal) = &contended else {
        panic!("{case}: another process took the worktree lease this test holds");
    };
    assert!(
        refusal.contains("is already driving worktree"),
        "{case}: the contender was refused for another reason: {refusal}"
    );
    if cfg!(unix) {
        let holder = format!("(pid {})", std::process::id());
        assert!(
            refusal.contains(&holder),
            "{case}: the lease's holder is not this process {holder}: {refusal}"
        );
    }
}

#[test]
#[ignore = "subprocess helper"]
fn worktree_lease_contender() {
    let (Some(repo), Some(git_dir)) = (
        std::env::var_os(CONTENDER_REPO).map(PathBuf::from),
        std::env::var_os(CONTENDER_GIT_DIR).map(PathBuf::from),
    ) else {
        return;
    };
    match WorktreeLock::acquire_in(&repo, &git_dir) {
        Ok(lease) => {
            drop(lease);
            crate::workspace_manager::fixture::say(CONTENDER_TOOK);
        }
        Err(refused) => {
            crate::workspace_manager::fixture::say(&format!("{CONTENDER_REFUSED}{refused}"));
        }
    }
}

#[cfg(unix)]
const DEFAULT_ROOT_HUSK_REPO: &str = "UPSTROKE_DEFAULT_ROOT_HUSK_REPO";

#[cfg(unix)]
const DEFAULT_ROOT_HUSK_RUN: &str = "UPSTROKE_DEFAULT_ROOT_HUSK_RUN";

#[cfg(unix)]
const DEFAULT_ROOT_HUSK_CREATION_ROOT: &str = "UPSTROKE_DEFAULT_ROOT_HUSK_CREATION_ROOT";

#[cfg(unix)]
const DEFAULT_ROOT_HUSK_DONE: &str =
    "default-root husk resume: refused in both default-root states, the lease free and held";

#[cfg(unix)]
#[test]
#[ignore = "subprocess helper"]
fn default_root_husk_resume_child() {
    let (Some(repo), Some(run_id), Some(creation_root)) = (
        std::env::var_os(DEFAULT_ROOT_HUSK_REPO).map(PathBuf::from),
        std::env::var(DEFAULT_ROOT_HUSK_RUN).ok(),
        std::env::var_os(DEFAULT_ROOT_HUSK_CREATION_ROOT).map(PathBuf::from),
    ) else {
        return;
    };
    let home = PathBuf::from(std::env::var_os("HOME").expect("the parent names the home"));
    let root = rundir::default_private_root();
    assert_eq!(
        root,
        home.join(".upstroke"),
        "the premise: the default root hangs from this home"
    );
    assert!(
        fs::symlink_metadata(&root).is_err(),
        "the premise: no default root exists yet"
    );
    assert!(
        !creation_root.starts_with(&home),
        "the premise: the husk was created under a root outside this home"
    );
    let says = rundir::resolve_run_id(&repo, &run_id)
        .expect_err("the premise: the resolver refuses the husk's exact id")
        .to_string();
    let mut resume = resume_options(&repo, &run_id);
    resume.private_root = None;
    let stub = ObservedStub::new(0);
    let source = BorrowedObserved(&stub);
    let git_dir = repo.join(".git");
    let scratch = repo.parent().expect("the checkout's fixture tree");
    drop(WorktreeLock::acquire(&repo).expect("the lease, taken once so that its file exists"));
    assert!(
        git_dir.join("upstroke-worktree.lock").is_file(),
        "the premise: the lease's own file exists before the first snapshot of .git, so no lease \
         taken below creates anything there"
    );

    for state in ["no default root", "a default root holding another include"] {
        if state != "no default root" {
            put_include(&root, IncludeState::Changed);
        }
        for held in [false, true] {
            let case = format!("{state}, lease held {held}");
            let git = private_snapshot_in(&git_dir, &case);
            let lease = held.then(|| WorktreeLock::acquire(&repo).expect("the test's lease"));
            assert_the_lease_is(
                held,
                &repo,
                &git_dir,
                scratch,
                &format!("{case}, before the resume"),
            );
            let homes = private_snapshot_in(&home, &case);
            let created = private_snapshot_in(&creation_root, &case);
            let checkout = public_snapshot_in(&repo, &case);
            let refused = resume_with(&resume, &source);
            let homes_after = private_snapshot_in(&home, &case);
            let created_after = private_snapshot_in(&creation_root, &case);
            let checkout_after = public_snapshot_in(&repo, &case);
            if held {
                assert_the_lease_is(
                    true,
                    &repo,
                    &git_dir,
                    scratch,
                    &format!("{case}, after the resume and every observation under the lease"),
                );
            }
            drop(lease);
            let git_after = private_snapshot_in(&git_dir, &case);
            for (place, before, after) in [
                ("under the home", &homes, &homes_after),
                ("under the husk's creation root", &created, &created_after),
                ("in the checkout", &checkout, &checkout_after),
                ("in the checkout's Git directory", &git, &git_after),
            ] {
                let moved = moved_entries(before, after);
                assert!(
                    moved.is_empty(),
                    "{case}: the husk's resume wrote {place}: {moved:?}"
                );
            }
            assert!(
                stub.built().is_empty(),
                "{case}: a worker or reviewer was built"
            );
            let refused = refused.expect_err("a husk has no run to open");
            assert_eq!(
                refused.to_string(),
                says,
                "{case}: not the resolver's own refusal"
            );
        }
    }
    crate::workspace_manager::fixture::say(DEFAULT_ROOT_HUSK_DONE);
}

fn recorded_execution_root(repo: &Path, run_id: &str) -> PathBuf {
    PathBuf::from(
        first_line_of_the_log(repo, run_id)["data"]["execution_root"]
            .as_str()
            .expect("the run records its execution root"),
    )
}

fn execution_namespaces(repo: &Path) -> Vec<String> {
    let workspaces = private_root_for(repo).join("workspaces");
    let mut keys: Vec<String> = match fs::read_dir(&workspaces) {
        Ok(listed) => listed
            .map(|entry| {
                entry
                    .expect("an entry of the workspaces directory")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => panic!("list {}: {error}", workspaces.display()),
    };
    keys.sort();
    keys
}

#[cfg(unix)]
fn manager_today(repo: &Path, run_id: &str) -> crate::workspace_manager::WorkspaceManager {
    let root = crate::engine::topology::recover::chain::RootDerived::derive(repo, run_id, None)
        .expect("the run's recorded root");
    let first = first_line_of_the_log(repo, run_id);
    let incarnation = first["data"]["incarnation"]
        .as_str()
        .expect("the run records its incarnation");
    crate::workspace_manager::WorkspaceManager::derive(
        repo,
        root.private_root(),
        run_id,
        incarnation,
    )
    .expect("the run's workspace manager")
}

#[cfg(unix)]
type Places = Vec<(
    &'static str,
    std::collections::BTreeMap<PathBuf, PrivateEntry>,
)>;

#[cfg(unix)]
fn common_dir_fixture(repo: &Path, other: &Path) -> Places {
    vec![
        ("in the checkout", public_snapshot(repo)),
        (
            "in the checkout's Git directory",
            private_snapshot(&repo.join(".git")),
        ),
        (
            "under the private root",
            private_snapshot(&private_root_for(repo)),
        ),
        ("in the other common directory", private_snapshot(other)),
    ]
}

#[cfg(unix)]
fn moved_in(before: &Places, after: &Places) -> Vec<String> {
    before
        .iter()
        .zip(after)
        .flat_map(|((place, earlier), (_, later))| {
            moved_entries(earlier, later)
                .into_iter()
                .map(move |moved| format!("{place}: {moved}"))
        })
        .collect()
}

#[cfg(unix)]
fn assert_admitted_at_the_recorded_roots(repo: &Path, open: &str, finished: &str) {
    for run_id in [open, finished] {
        assert_eq!(
            manager_today(repo, run_id).execution_root(),
            recorded_execution_root(repo, run_id),
            "the premise: this Git environment derives the root `{run_id}` recorded"
        );
    }
    let namespaces = execution_namespaces(repo);
    let stub = ObservedStub::new(0);
    let source = BorrowedObserved(&stub);

    let mut resume = resume_options(repo, open);
    resume.private_root = None;
    let report = assert_admitted_and_published(repo, || resume_with(&resume, &source));
    assert_eq!(report.run_id, open);

    let residue = recorded_execution_root(repo, finished);
    assert!(
        residue.is_dir(),
        "the premise: the finished run's root is retained"
    );
    let mut resume = resume_options(repo, finished);
    resume.private_root = None;
    let refused = resume_with(&resume, &source).expect_err("a finished run does not continue");
    assert!(
        refused.to_string().contains("already finished"),
        "the finished run was finalized: {refused}"
    );
    assert!(
        fs::symlink_metadata(&residue).is_err(),
        "finalization reclaimed the root the run recorded: {refused}"
    );
    assert_eq!(
        execution_namespaces(repo),
        namespaces,
        "the admitted resumes made no other namespace"
    );
}

#[cfg(unix)]
const COMMON_DIR_REPO: &str = "UPSTROKE_COMMON_DIR_REPO";

#[cfg(unix)]
const COMMON_DIR_OPEN_RUN: &str = "UPSTROKE_COMMON_DIR_OPEN_RUN";

#[cfg(unix)]
const COMMON_DIR_FINISHED_RUN: &str = "UPSTROKE_COMMON_DIR_FINISHED_RUN";

#[cfg(unix)]
const COMMON_DIR_REFUSED: &str =
    "common-dir resume: both runs refused under another common directory, nothing changed";

#[cfg(unix)]
const COMMON_DIR_ADMITTED: &str =
    "common-dir resume: admitted and finalized at the roots the runs recorded";

#[cfg(unix)]
#[test]
#[ignore = "subprocess helper"]
fn common_dir_refused_resume_child() {
    let (Some(repo), Some(open), Some(finished), Some(other)) = (
        std::env::var_os(COMMON_DIR_REPO).map(PathBuf::from),
        std::env::var(COMMON_DIR_OPEN_RUN).ok(),
        std::env::var(COMMON_DIR_FINISHED_RUN).ok(),
        std::env::var_os("GIT_COMMON_DIR").map(PathBuf::from),
    ) else {
        return;
    };
    assert!(
        other.is_absolute() && !other.starts_with(&repo),
        "the premise: this process's Git environment names another common directory, absolute \
         and outside the checkout: {}",
        other.display()
    );
    let git_dir = Workspace::open(&repo)
        .expect("the checkout")
        .worktree_git_dir()
        .expect("its Git directory");
    let physical = rundir::RepoKey::for_worktree_git_dir(&git_dir)
        .expect("the checkout's physical repository key");
    assert!(
        recorded_execution_root(&repo, &finished).is_dir(),
        "the premise: the finished run's root is retained"
    );
    let stub = ObservedStub::new(0);
    let source = BorrowedObserved(&stub);

    for (what, run_id) in [("the open run", &open), ("the finished run", &finished)] {
        let owner: serde_json::Value = serde_json::from_slice(
            &fs::read(private_dir_of(&repo, run_id).join(rundir::OWNER_RECORD))
                .expect("the owner record"),
        )
        .expect("an owner record");
        assert_eq!(
            owner["repo_key"].as_str(),
            Some(physical.as_str()),
            "the premise: {what}'s owner record names the checkout's physical key"
        );
        let recorded = recorded_execution_root(&repo, run_id);
        let manager = manager_today(&repo, run_id);
        assert_eq!(
            manager.common_git_dir(),
            other.as_path(),
            "the premise: today's manager of {what} takes the other common directory"
        );
        assert_ne!(
            manager.execution_root(),
            recorded.as_path(),
            "the premise: and derives another execution root than {what} recorded"
        );
        let mut resume = resume_options(&repo, run_id);
        resume.private_root = None;
        for exclusion in [ExclusionState::Absent, ExclusionState::Changed] {
            for include in [IncludeState::Absent, IncludeState::Changed] {
                put_exclusion(&repo, exclusion);
                put_include(&private_root_for(&repo), include);
                let before = common_dir_fixture(&repo, &other);
                let refused = resume_with(&resume, &source);
                let after = common_dir_fixture(&repo, &other);
                let case = format!("{what}, exclusion {exclusion:?}, include {include:?}");
                let moved = moved_in(&before, &after);
                assert!(
                    moved.is_empty(),
                    "{case}: the resume under another common directory wrote {moved:?}"
                );
                assert!(
                    stub.built().is_empty(),
                    "{case}: a worker or reviewer was built"
                );
                let refused = refused
                    .expect_err("a resume never adopts another root")
                    .to_string();
                for named in [
                    format!(
                        "run `{run_id}` records its execution root as {}",
                        recorded.display()
                    ),
                    format!(
                        "derives {} for it today",
                        manager.execution_root().display()
                    ),
                    format!(
                        "canonical private root {}",
                        manager.private_root().display()
                    ),
                    format!(
                        "effective common directory {}",
                        manager.common_git_dir().display()
                    ),
                ] {
                    assert!(
                        refused.contains(&named),
                        "{case}: the refusal names `{named}`: {refused}"
                    );
                }
            }
        }
    }
    crate::workspace_manager::fixture::say(COMMON_DIR_REFUSED);
}

#[cfg(unix)]
#[test]
#[ignore = "subprocess helper"]
fn common_dir_admitted_resume_child() {
    let (Some(repo), Some(open), Some(finished)) = (
        std::env::var_os(COMMON_DIR_REPO).map(PathBuf::from),
        std::env::var(COMMON_DIR_OPEN_RUN).ok(),
        std::env::var(COMMON_DIR_FINISHED_RUN).ok(),
    ) else {
        return;
    };
    assert!(
        std::env::var_os("GIT_COMMON_DIR").is_some(),
        "the premise: this process's Git environment names a common directory"
    );
    assert_admitted_at_the_recorded_roots(&repo, &open, &finished);
    crate::workspace_manager::fixture::say(COMMON_DIR_ADMITTED);
}

/// Two implementation tasks, the second on top of the first, each held to one
/// attempt on one rung.
const ONE_ATTEMPT_PLAN: &str = "## Implement the widget\n\
                                <!-- upstroke: id=t1 kind=implement depends= -->\n\
                                Make it.\n\n## Implement the gadget\n\
                                <!-- upstroke: id=t2 kind=implement depends=t1 -->\n\
                                Make that, on top of the widget.\n";

fn log_events(repo: &Path, run_id: &str) -> Vec<serde_json::Value> {
    fs::read_to_string(rundir::public_dir(repo, run_id).join("events.jsonl"))
        .expect("the run's event log")
        .lines()
        .map(|line| serde_json::from_str(line).expect("a committed event"))
        .collect()
}

/// Where `no_committer_identity_helper` records that it ran.
#[cfg(unix)]
const NO_IDENTITY_MARKER: &str = "UPSTROKE_FINDINGS2_NO_IDENTITY_MARKER";

#[cfg(unix)]
#[test]
#[ignore = "subprocess helper"]
fn no_committer_identity_helper() {
    if std::env::var_os(crate::workspace_manager::fixture::REPLACEMENT_WITNESS).is_none() {
        return;
    }
    let marker = PathBuf::from(std::env::var_os(NO_IDENTITY_MARKER).expect("the marker"));
    fs::write(&marker, b"ran\n").expect("the marker");
    let (_tree, repo) = temp_engine_repo("no-identity");
    seed(
        &repo,
        TWO_INDEPENDENT_TASKS,
        Some("[engine]\nmax_parallel = 2\n\n[routing]\nreview = { enabled = false }\n"),
    );
    git_in(&repo, &["config", "--unset", "user.name"]);
    git_in(&repo, &["config", "--unset", "user.email"]);
    git_in(&repo, &["config", "user.useConfigOnly", "true"]);
    let ident = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .args(["var", "GIT_COMMITTER_IDENT"])
        .output()
        .expect("run git var");
    assert!(
        !ident.status.success(),
        "the premise: Git finds no committer identity in this repository and environment"
    );

    let source = SharedTopologyStubs(std::sync::Arc::new(TopologyStub::new(
        "printf 'edited by the topology stub\\n' > \"out.$$.txt\"",
        0,
    )));
    let report = run_with(&topology_options(&repo), &source)
        .expect("the run integrates without an operator identity");
    until_released(&repo, &report.run_id);
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:#?}");
    assert!(
        committed(&report, "t1") && committed(&report, "t2"),
        "{report:#?}"
    );
    let projected = crate::engine::topology::observe::observe(&repo, &report.run_id)
        .expect("observes")
        .expect("a schema-4 log")
        .report()
        .expect("projects");
    let picked = projected
        .integration_ledger
        .iter()
        .find(|row| row.basis == "stale_clean")
        .and_then(|row| row.proposed_sha.clone())
        .unwrap_or_else(|| {
            panic!(
                "the premise: one merge was a stale candidate's cherry-pick: {:#?}",
                projected.integration_ledger
            )
        });
    assert_eq!(
        git_in(&repo, &["log", "-1", "--format=%cn <%ce>", &picked]).trim(),
        "upstroke <upstroke@upstroke.local>",
        "the cherry-picked proposal carries the engine's committer identity"
    );
}

#[cfg(unix)]
const UNSPELLABLE_HOME_REPO: &str = "UPSTROKE_UNSPELLABLE_HOME_REPO";

#[cfg(unix)]
#[test]
#[ignore = "subprocess helper"]
fn unspellable_default_root_child() {
    let Some(repo) = std::env::var_os(UNSPELLABLE_HOME_REPO).map(PathBuf::from) else {
        return;
    };
    let mut opts = options(&repo);
    opts.legacy_writer = false;
    opts.private_root = None;
    let source = SharedTopologyStubs(std::sync::Arc::new(TopologyStub::new(
        TOPOLOGY_STUB_EDIT,
        0,
    )));
    let refused = run_with(&opts, &source).expect_err("an unspellable default root is refused");
    let said = refused.to_string();
    assert!(
        said.contains("the default private root") && said.contains("is not valid UTF-8"),
        "{said}"
    );
    assert!(said.contains("Nothing was locked or created"), "{said}");
    assert!(
        rundir::run_dir_names(&repo).is_empty(),
        "no run directory exists after the refusal"
    );
}

/// The model the default bindings give the frontier tier (`catalog::example_binding`).
const FRONTIER_MODEL: &str = "claude-opus-5";

/// The mark a task's body carries to make its workers below the frontier end without an
/// edit, so that its ladder must escalate onto the frontier.
const ESCALATES: &str = "UPSTROKE-ESCALATES";

/// What [`ObservedStub`] was asked to build.
#[derive(Debug, Clone)]
struct Built {
    model: String,
    reviewer: bool,
    prompt: String,
}

/// The topology stub, recording every worker and reviewer it is asked to build: the model, whether
/// it reviews, and the prompt it was handed. A worker below the frontier tier whose prompt
/// carries [`ESCALATES`] ends without an edit.
struct ObservedStub {
    stub: TopologyStub,
    built: Mutex<Vec<Built>>,
}

impl ObservedStub {
    fn new(questions: usize) -> Self {
        Self {
            stub: TopologyStub::new(TOPOLOGY_STUB_EDIT, questions),
            built: Mutex::new(Vec::new()),
        }
    }

    fn built(&self) -> Vec<Built> {
        self.built
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// The model each worker ran on, in order.
    fn workers(&self) -> Vec<String> {
        self.built()
            .into_iter()
            .filter(|built| !built.reviewer)
            .map(|built| built.model)
            .collect()
    }
}

impl AgentAdapter for ObservedStub {
    fn id(&self) -> &'static str {
        self.stub.id()
    }

    fn probe(&self, runner: &dyn crate::runner::Runner) -> Result<Caps, UpstrokeError> {
        self.stub.probe(runner)
    }

    fn build(&self, run: &TaskRun) -> Result<CommandSpec, UpstrokeError> {
        let reviewer = run.profile.permissions == PermissionMode::ReadOnly;
        self.built
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(Built {
                model: run.profile.model.clone(),
                reviewer,
                prompt: run.prompt.clone(),
            });
        if !reviewer && run.profile.model != FRONTIER_MODEL && run.prompt.contains(ESCALATES) {
            return Ok(shell_spec("exit 0"));
        }
        self.stub.build(run)
    }

    fn parse(&self, out: &ProcessOutput) -> Result<Outcome, UpstrokeError> {
        self.stub.parse(out)
    }
}

/// [`ObservedStub`] behind a source that cannot be shared: the step loop.
struct BorrowedObserved<'a>(&'a ObservedStub);

impl AdapterSource for BorrowedObserved<'_> {
    fn get(&self, id: &str) -> Option<&dyn AgentAdapter> {
        (id == TOPOLOGY_STUB).then_some(self.0 as &dyn AgentAdapter)
    }
}
#[cfg(unix)]
const INHERITED_COMMON_REFUSALS: &str = "UPSTROKE_INHERITED_COMMON_REFUSALS";

#[cfg(unix)]
#[test]
#[ignore = "subprocess helper"]
fn inherited_common_dir_mount_child() {
    let Some(repo) = std::env::var_os(INHERITED_RUN_ROOT).map(PathBuf::from) else {
        return;
    };
    let run_id = std::env::var_os(INHERITED_RUN_ID)
        .and_then(|id| id.into_string().ok())
        .expect("the recorded run's id");
    let common =
        fs::canonicalize(repo.with_file_name("common")).expect("the relocated common directory");
    let names_the_common_directory = |said: &str| {
        said.contains(&format!("lies inside `{}`", common.display()))
            && said.contains("authoritative Git")
    };
    let source = SharedTopologyStubs(std::sync::Arc::new(TopologyStub::new(
        TOPOLOGY_STUB_EDIT,
        0,
    )));

    let mut opts = topology_options(&repo);
    opts.config_path = Some(repo.with_file_name("container.toml"));
    let refused = run_with(&opts, &source).expect_err("a fresh run refuses the configured mount");
    let said = refused.to_string();
    assert!(
        said.contains("[runner] the mount") && names_the_common_directory(&said),
        "the fresh run: {said}"
    );

    let refused = resume_with(&resume_options(&repo, &run_id), &source)
        .expect_err("a resume refuses the recorded mount");
    let resumed = refused.to_string();
    assert!(
        resumed.contains(&format!("run `{run_id}` records the mount"))
            && names_the_common_directory(&resumed)
            && resumed.contains("nothing was published, locked or written"),
        "the resume: {resumed}"
    );
    let refusals =
        std::env::var_os(INHERITED_COMMON_REFUSALS).expect("the parent names the refusals' file");
    fs::write(refusals, format!("{said}\n{resumed}\n")).expect("the refusals recorded");
}

#[cfg(unix)]
const RECORDED_TARGET_REFUSAL: &str = "UPSTROKE_RECORDED_TARGET_REFUSAL";

#[cfg(unix)]
#[test]
#[ignore = "subprocess helper"]
fn recorded_target_resume_child() {
    let repo = PathBuf::from(
        std::env::var_os(INHERITED_RUN_ROOT).expect("the parent names the recorded repository"),
    );
    let run_id = std::env::var(INHERITED_RUN_ID).expect("the recorded run's UTF-8 id");
    let source = SharedTopologyStubs(std::sync::Arc::new(TopologyStub::new(
        TOPOLOGY_STUB_EDIT,
        0,
    )));
    let refused = resume_with(&resume_options(&repo, &run_id), &source)
        .expect_err("a resume refuses the recorded target");
    let refusal =
        std::env::var_os(RECORDED_TARGET_REFUSAL).expect("the parent names the refusal's file");
    fs::write(refusal, refused.to_string()).expect("the refusal recorded");
}

// `-` for the real message's `·`, as in the tests above.
const LIMIT_PAST_THE_HEAD: &str = concat!(
    r#"{"type":"result","is_error":true,"result":"MCP cache resets 9am (UTC). "#,
    "mcp server docs: tool list refreshed in 812ms, 14 tools registered, 2 skipped as duplicates. ",
    "mcp server docs: tool list refreshed in 815ms, 14 tools registered, 2 skipped as duplicates. ",
    "mcp server docs: tool list refreshed in 818ms, 14 tools registered, 2 skipped as duplicates. ",
    "mcp server docs: tool list refreshed in 821ms, 14 tools registered, 2 skipped as duplicates. ",
    "mcp server docs: tool list refreshed in 824ms, 14 tools registered, 2 skipped as duplicates. ",
    r#"You've hit your 5-hour limit - resets 5:30am (UTC)","session_id":"fake-review-session","fake":"UPSTROKE-FAKE-REVIEW"}"#,
);
