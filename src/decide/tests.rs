use std::collections::{BTreeMap, VecDeque};
use std::sync::Mutex;
use std::time::Duration;

use super::*;
use crate::agent::ProcessOutput;
use crate::engine::BuiltinAdapters;
use crate::ir::{Effort, QuestionId, QuestionKind, TaskId};
use crate::review::PassBinding;
use crate::rundir::scratch_tree::ScratchTree;
use crate::runner::{ExecutionRole, RunFuture, RunnerCall, RunnerRequest};
use crate::topology::events::{
    Answer4, FrozenQuestion, QuestionAnswered4, QuestionRaised4, RunStarted4,
};
use crate::topology::registry::TaskKey;

const RUN: &str = "01DECIDERUN000000000000001";
const RETRY: &str = "retry on frontier";
const PLAN_TEXT: &str = "# The plan\n\nThe widget task makes the widget.\n";
const STAMPED_AT: &str = "2026-11-25T00:00:00Z";

fn question(options: &[&str]) -> Question {
    Question {
        id: QuestionId::from("q-1"),
        kind: QuestionKind::Unblock,
        affected_tasks: vec![TaskId::from("t1")],
        context: "every rung failed".to_owned(),
        options: options.iter().map(|option| (*option).to_owned()).collect(),
    }
}

fn reply(body: &str) -> String {
    format!("here is my proposal\n```json\n{body}\n```\n")
}

const VALID: &str = r#"{"choice": 1, "guidance": "retry", "reasons": ["it flaked"], "confidence": "high", "needs_a_person": false, "citations": ["src/lib.rs:1"]}"#;

fn bad_replies() -> Vec<(String, &'static str)> {
    let outside = VALID.replace("\"choice\": 1", "\"choice\": 9");
    let give_up = VALID.replace("\"choice\": 1", "\"choice\": 2");
    let missing = VALID.replace("\"guidance\": \"retry\", ", "");
    vec![
        (reply(&outside), "not one of the question's options"),
        (reply(&give_up), "gives up"),
        (reply(&missing), "`guidance` is missing"),
        (
            format!("{}{}", reply(VALID), reply(VALID)),
            "2 fenced json blocks",
        ),
        (VALID.to_owned(), "no fenced json block"),
    ]
}

#[test]
fn a_valid_choice_is_accepted() {
    let asked = question(&[RETRY, crate::interaction::GIVE_UP_OPTION]);
    assert!(check_reply(&asked, &reply(VALID)).is_ok());
    let open = question(&[]);
    let null = VALID.replace("\"choice\": 1", "\"choice\": null");
    assert!(check_reply(&open, &reply(&null)).is_ok());
    assert!(
        check_reply(&asked, &reply(&null)).is_err(),
        "null only where there are no options"
    );
}

#[test]
fn a_bad_reply_is_refused_with_its_reason() {
    let asked = question(&[RETRY, crate::interaction::GIVE_UP_OPTION]);
    for (text, needle) in bad_replies() {
        let refused = check_reply(&asked, &text).expect_err("refused");
        assert!(refused.contains(needle), "{needle}: {refused}");
    }
}

#[test]
fn a_give_up_option_in_its_own_words_is_refused() {
    let give_up = VALID.replace("\"choice\": 1", "\"choice\": 2");
    for words in ["abandon", "skip"] {
        let asked = question(&[RETRY, words]);
        assert_eq!(
            crate::interaction::answer_for_option(&asked, 2),
            Some(Answer::Answered {
                text: words.to_owned()
            }),
            "{words}: option 2 reaches the arm that reads its own words"
        );
        let refused = check_reply(&asked, &reply(&give_up)).expect_err("refused");
        assert!(refused.contains("gives up"), "{words}: {refused}");
        assert!(
            check_reply(&asked, &reply(VALID)).is_ok(),
            "{words}: option 1 is accepted"
        );
    }
}

#[test]
fn an_input_is_cut_on_a_character_boundary() {
    let text = "é".repeat(INPUT_BOUND);
    let (kept, cut) = bounded(&text);
    assert!(cut);
    assert!(kept.len() <= INPUT_BOUND);
    assert!(text.is_char_boundary(kept.len()));
    let mut prompt = String::new();
    section(&mut prompt, "The plan file", &text);
    assert!(prompt.contains("[cut here: this input was cut at 65536 bytes"));
}

/// A Runner that records what it was asked to run and answers from a table,
/// in order, so `decide` is driven with no process and no network.
struct Scripted {
    replies: Mutex<VecDeque<String>>,
    seen: Mutex<Vec<RunnerRequest>>,
}

impl Scripted {
    fn answering(replies: Vec<String>) -> Self {
        Self {
            replies: Mutex::new(replies.into()),
            seen: Mutex::new(Vec::new()),
        }
    }

    fn seen(&self) -> Vec<RunnerRequest> {
        self.seen
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

impl Runner for Scripted {
    fn run<'a>(&'a self, request: &'a RunnerRequest, _call: RunnerCall<'a>) -> RunFuture<'a> {
        self.seen
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(request.clone());
        let stdout = self
            .replies
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .pop_front()
            .unwrap_or_default();
        let answer = Ok(ProcessOutput {
            code: Some(0),
            stdout,
            stderr: String::new(),
            duration: Duration::ZERO,
            timed_out: false,
            output_limited: false,
        });
        Box::pin(async move { answer })
    }
}

/// What the Claude adapter's own `parse` reads: the reply's text and its cost.
fn claude_output(text: &str, cost_usd: Option<f64>) -> String {
    let mut output = json!({ "result": text, "session_id": "s-1" });
    if let (Some(cost), Some(object)) = (cost_usd, output.as_object_mut()) {
        object.insert("total_cost_usd".to_owned(), json!(cost));
    }
    output.to_string()
}

/// A scratch tree for one test, guarded by the token that authorises its
/// deletion (as `answer`'s tests acquire theirs). Bind it to a live local.
fn scratch(tag: &str) -> ScratchTree {
    let parent = std::env::temp_dir();
    match crate::rundir::scratch_tree::acquire(&parent, tag) {
        Ok(tree) => tree,
        Err(refusal) => panic!(
            "a scratch tree for `{tag}` under {}: {refusal:?}",
            parent.display()
        ),
    }
}

fn plan(tasks: usize) -> crate::ir::Plan {
    use crate::ir::{Plan, PlanSource, Task, TaskKind};
    let task = |at: usize| {
        let id = format!("t{at}");
        Task {
            id: TaskId::from(id.as_str()),
            kind: TaskKind::Refactor,
            title: format!("{id} title"),
            body: format!("{id} body: make the widget"),
            depends_on: Vec::new(),
            acceptance: vec![format!("{id} passes")],
            path_hints: vec![format!("src/{id}/")],
            suggested_tier: None,
            min_tier: None,
            artifacts_in: Vec::new(),
            artifacts_out: Vec::new(),
        }
    };
    Plan {
        source: PlanSource {
            adapter: "markdown".to_owned(),
            hash: "decide-plan-hash".to_owned(),
        },
        tasks: (0..tasks).map(task).collect(),
        artifacts: Vec::new(),
    }
}

fn run_started(
    plan: &crate::ir::Plan,
    normalized_plan_digest: &str,
    private: &Path,
    primary: Option<PassBinding>,
) -> RunStarted4 {
    use crate::events::{BindingSummary, ChainSummary, GateSummary};
    use crate::gates::ShellKind;
    use crate::ir::{ResolvedEffortPolicy, Tier};
    use crate::review::ReviewPlan;
    use crate::topology::events::{
        CommitSha, GitRef, IncarnationId, RunnerContract, RunnerKind, RunnerPolicy, TopologyLimits,
    };
    use crate::topology::paths::{PathGrammar, PathPolicy, PathPolicyVersion};

    let chain = |task: &str| ChainSummary {
        task: task.to_owned(),
        tiers: vec![Tier::Mid, Tier::Frontier],
        attempts_per: 2,
        bindings: Some(vec![
            BindingSummary {
                tier: Tier::Mid,
                agent: crate::agent::claude::ADAPTER_ID.to_owned(),
                model: format!("{task}-mid-model"),
                pinned: false,
            },
            BindingSummary {
                tier: Tier::Frontier,
                agent: crate::agent::claude::ADAPTER_ID.to_owned(),
                model: format!("{task}-frontier-model"),
                pinned: true,
            },
        ]),
    };
    let unauthenticated = RunStarted4 {
        schema: crate::topology::schema::TOPOLOGY_SCHEMA,
        upstroke_version: "0.2.0-decide".to_owned(),
        run_id: RUN.to_owned(),
        incarnation: IncarnationId("01DECIDEINC000000000000000".to_owned()),
        runner: RunnerPolicy {
            kind: RunnerKind::Host,
            policy: RunnerContract::HostV1,
            image: None,
            credential_volumes: None,
            mounts: Vec::new(),
        },
        probed_agents: vec![crate::agent::claude::ADAPTER_ID.to_owned()],
        shell: ShellKind::Bash,
        branch: format!("upstroke/run-{RUN}"),
        integration_ref: GitRef(format!("refs/heads/upstroke/run-{RUN}")),
        base_sha: CommitSha("a".repeat(40)),
        execution_root: "/var/lib/upstroke/execution".to_owned(),
        private_dir: private.to_string_lossy().into_owned(),
        plan_path: "docs/plan.md".to_owned(),
        config_path: None,
        plan_hash: plan.source.hash.clone(),
        normalized_plan_digest: normalized_plan_digest.to_owned(),
        registry_digest: String::new(),
        path_policy: PathPolicy {
            version: PathPolicyVersion::V2,
            case_fold: true,
            grammar: PathGrammar::Globset,
        },
        limits: TopologyLimits {
            max_parallel: 1,
            max_defers: 2,
            max_merge_repairs: 3,
        },
        gates: vec!["fmt".to_owned()],
        gates_from_config: true,
        gate_cmds: vec![GateSummary {
            name: "fmt".to_owned(),
            cmd: "cargo fmt --check".to_owned(),
            timeout: Duration::from_secs(60),
            shell: ShellKind::Bash,
        }],
        interaction_mode: "never".to_owned(),
        chains: plan.tasks.iter().map(|t| chain(t.id.as_str())).collect(),
        effort_policy: ResolvedEffortPolicy {
            small: Effort::Low,
            mid: Effort::High,
            frontier: Effort::Max,
            review: Effort::Medium,
        },
        reviews: ReviewPlan {
            enabled: Some(primary.is_some()),
            alternative_available: Some(false),
            pass_timeout_secs: Some(900),
            primary,
            alternative: None,
            second_opinion: vec![None; plan.tasks.len()],
        },
    };
    let registry_digest = crate::topology::registry::TaskRegistry::originals_with_agents(
        plan,
        &unauthenticated.registry_record(),
        &unauthenticated.probed_agents,
    )
    .expect("the fixture record derives a registry")
    .digest();
    RunStarted4 {
        registry_digest,
        ..unauthenticated
    }
}

/// A schema-4 run written through the event funnel, each line checked by the
/// fold before it is appended (as `capacity`'s schema-4 fixture writes one).
struct Log {
    log: crate::events::EventLog,
    fold: crate::topology::fold::TopologyFold,
}

impl Log {
    fn push(&mut self, body: TopologyEventBody) {
        let event = TopologyEvent {
            ts: STAMPED_AT.to_owned(),
            body,
        };
        let (line, checked) = crate::events::log::TopologyLine::round_trip(&event)
            .expect("the event survives its own wire format");
        let delta = self.fold.plan_transition(&checked).unwrap_or_else(|error| {
            panic!(
                "the fold refuses this fixture's `{}`: {error}",
                checked.body.kind()
            )
        });
        self.log
            .append_topology(line.site(), &line)
            .expect("append the committed line");
        self.fold.apply_delta(delta);
    }

    fn raise(&mut self, id: &str, key: u32) {
        self.push(TopologyEventBody::QuestionRaised {
            data: QuestionRaised4 {
                question: FrozenQuestion {
                    id: QuestionId::from(id),
                    key: TaskKey(key),
                    kind: QuestionKind::Unblock,
                    context: format!("{id}: every rung failed on the same assertion"),
                    options: vec![
                        RETRY.to_owned(),
                        crate::interaction::GIVE_UP_OPTION.to_owned(),
                    ],
                },
            },
        });
    }

    fn answer(&mut self, id: &str, key: u32) {
        self.push(TopologyEventBody::QuestionAnswered {
            data: QuestionAnswered4 {
                key: TaskKey(key),
                question: QuestionId::from(id),
                answer: Answer4::Answered {
                    option_index: 0,
                    binding_override: None,
                },
                via: "upstroke answer".to_owned(),
                text: Some(RETRY.to_owned()),
            },
        });
    }
}

struct Fixture {
    _tree: ScratchTree,
    repo: PathBuf,
    private: PathBuf,
}

impl Fixture {
    fn options(&self) -> DecideOptions {
        DecideOptions {
            repo_root: self.repo.clone(),
            run_id: RUN.to_owned(),
            question_id: None,
            max_cost_usd: DEFAULT_MAX_COST_USD,
            pools_path: Some(self.repo.join("pools.toml")),
        }
    }

    fn public(&self) -> PathBuf {
        crate::rundir::public_dir(&self.repo, RUN)
    }

    fn proposal(&self, id: &str, n: u32) -> Value {
        let path = proposals_dir(&self.repo, RUN).join(format!("{id}.{n}.json"));
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        serde_json::from_str(&text).expect("a proposal is JSON")
    }
}

const POOLS: &str = "[pools.claude-max]\nkind = \"subscription-window\"\nagent = \"claude-code\"\nprofile = \"/profiles/claude-max\"\n";

/// A schema-4 run with `tasks` tasks whose log `write` fills after `run_started`.
fn fixture(
    tag: &str,
    tasks: usize,
    primary: Option<PassBinding>,
    write: impl FnOnce(&mut Log),
) -> Fixture {
    let tree = scratch(tag);
    let repo = tree.path().join("repo");
    let private = tree.path().join("private").join(RUN);
    std::fs::create_dir_all(repo.join("docs")).expect("docs");
    std::fs::write(repo.join("docs").join("plan.md"), PLAN_TEXT).expect("the plan file");
    std::fs::write(repo.join("pools.toml"), POOLS).expect("the pools file");
    std::fs::create_dir_all(&private).expect("the private directory");
    std::fs::write(private.join("transcript.txt"), "a transcript\n").expect("a transcript");

    let plan = plan(tasks);
    let mut plan_bytes = serde_json::to_vec_pretty(&plan).expect("the plan serializes");
    plan_bytes.push(b'\n');
    let normalized_plan_digest = crate::events::normalized_plan_digest(&plan_bytes);
    let public = crate::rundir::public_dir(&repo, RUN);
    let hooks = &mut crate::rundir::NoHooks;
    crate::rundir::create_public_dir(&public, hooks).expect("the run's public directory");
    crate::rundir::write_plan(&public, &plan_bytes, hooks).expect("the frozen plan");
    let path = public.join(crate::rundir::EVENT_LOG);
    let log = crate::events::EventLog::open(
        crate::topology::effects::EventSite::OpenLog,
        &path,
        &mut Vec::new(),
    )
    .expect("open the run's log");
    let started = run_started(&plan, &normalized_plan_digest, &private, primary);
    let mut log = Log {
        log,
        fold: crate::topology::fold::TopologyFold::new(crate::topology::fold::FrozenInputs {
            plan,
            normalized_plan_digest,
        }),
    };
    log.push(TopologyEventBody::RunStarted {
        data: Box::new(started),
    });
    write(&mut log);
    drop(log);
    Fixture {
        _tree: tree,
        repo,
        private,
    }
}

fn reviewer() -> Option<PassBinding> {
    Some(PassBinding::new(crate::agent::claude::ADAPTER_ID, "opus"))
}

fn one_question(tag: &str) -> Fixture {
    fixture(tag, 1, reviewer(), |log| log.raise("q-01A", 0))
}

/// Every file under `dir`, by path, with its bytes.
fn snapshot(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut files = BTreeMap::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(next) = pending.pop() {
        for entry in std::fs::read_dir(&next)
            .expect("read a directory")
            .flatten()
        {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let bytes = std::fs::read(&path).expect("read a file");
                files.insert(path, bytes);
            }
        }
    }
    files
}

#[test]
fn a_valid_proposal_is_accepted_printed_and_saved_with_its_cost() {
    let fixture = one_question("decide-accepted");
    let runner = Scripted::answering(vec![claude_output(&reply(VALID), Some(0.25))]);
    let decided = decide(&fixture.options(), &BuiltinAdapters, &runner).expect("decided");
    assert!(decided.outcome().is_ok(), "{decided:?}");
    let report = &decided.report;
    for line in [
        "q-01A: decided by claude-code/opus at medium effort",
        "status: accepted",
        "choice: 1) retry on frontier",
        "guidance: retry",
        "confidence: high",
        "needs a person: false",
        "reason: it flaked",
        "citation: src/lib.rs:1",
        "cost: $0.2500",
        "total cost: $0.2500",
        "nothing was answered",
        "upstroke answer <question-id>",
    ] {
        assert!(report.contains(line), "{line}: {report}");
    }

    let saved = fixture.proposal("q-01A", 1);
    assert_eq!(saved["run_id"], RUN);
    assert_eq!(saved["question_id"], "q-01A");
    assert_eq!(saved["agent"], "claude-code");
    assert_eq!(saved["model"], "opus");
    assert_eq!(saved["effort"], "medium");
    assert_eq!(saved["status"], "accepted");
    assert_eq!(saved["cost_usd"], 0.25);
    assert_eq!(saved["record"]["choice"], 1);
    for field in FIELDS {
        assert!(saved["record"].get(field).is_some(), "{field}: {saved}");
    }
    let raw = PathBuf::from(saved["raw_output"].as_str().expect("a path"));
    assert_eq!(
        raw,
        proposals_dir(&fixture.repo, RUN).join("q-01A.1.out"),
        "the raw output is saved beside the proposal"
    );
    assert_eq!(
        std::fs::read_to_string(&raw).expect("the raw output"),
        claude_output(&reply(VALID), Some(0.25))
    );
    assert!(report.contains(&format!(
        "saved: {}",
        proposals_dir(&fixture.repo, RUN)
            .join("q-01A.1.json")
            .display()
    )));
}

#[test]
fn each_bad_reply_is_refused_saved_and_fails_the_command_after_the_others() {
    let bad = bad_replies();
    let ids = ["q-f", "q-e", "q-d", "q-c", "q-b", "q-a"];
    let fixture = fixture("decide-refused", ids.len(), reviewer(), |log| {
        for (key, id) in ids.iter().enumerate() {
            log.raise(id, u32::try_from(key).expect("a small key"));
        }
    });
    let mut replies: Vec<String> = bad
        .iter()
        .map(|(text, _)| claude_output(text, Some(0.1)))
        .collect();
    replies.push(claude_output(&reply(VALID), Some(0.1)));
    let runner = Scripted::answering(replies);
    let decided = decide(&fixture.options(), &BuiltinAdapters, &runner).expect("decided");

    assert_eq!(runner.seen().len(), ids.len(), "every question was asked");
    let asked_in_order: Vec<String> = runner
        .seen()
        .iter()
        .map(|request| String::from_utf8_lossy(&request.command.stdin).into_owned())
        .collect();
    for (prompt, id) in asked_in_order.iter().zip(ids) {
        assert!(
            prompt.contains(&format!("id: {id}\n")),
            "asked in the order raised"
        );
    }
    for ((_, reason), id) in bad.iter().zip(ids) {
        let saved = fixture.proposal(id, 1);
        assert_eq!(saved["status"], "refused", "{id}: {saved}");
        assert!(
            saved["reason"]
                .as_str()
                .is_some_and(|text| text.contains(reason)),
            "{id}: {saved}"
        );
        assert!(
            decided.report.contains(&format!("{id}: decided by")),
            "{id} is printed"
        );
    }
    assert_eq!(fixture.proposal("q-a", 1)["status"], "accepted");
    assert_eq!(decided.refused, ids[..5].to_vec());
    let error = decided
        .outcome()
        .expect_err("a refused reply makes the command an error");
    assert!(error.to_string().contains("q-f"), "{error}");
}

#[test]
fn the_request_is_a_decide_request_for_the_runs_reviewer_with_no_overlay() {
    let fixture = one_question("decide-request");
    let runner = Scripted::answering(vec![claude_output(&reply(VALID), Some(0.25))]);
    decide(&fixture.options(), &BuiltinAdapters, &runner).expect("decided");
    let seen = runner.seen();
    let [request] = seen.as_slice() else {
        panic!("one request: {seen:?}");
    };
    assert_eq!(request.role, ExecutionRole::Decide);
    assert_eq!(
        request.agent,
        Some(AgentId::new(crate::agent::claude::ADAPTER_ID))
    );
    assert!(request.command.env.is_empty(), "{:?}", request.command.env);
    assert_eq!(request.workspace, fixture.repo);
    assert_eq!(request.timeout, Duration::from_secs(900));
    assert_eq!(request.invocation, InvocationId::Decide { ordinal: 1 });
    assert_eq!(
        request.credential_profile,
        Some(PathBuf::from("/profiles/claude-max")),
        "the first pool of the reviewer's agent"
    );

    let settings = request
        .command
        .settings_file
        .as_ref()
        .expect("the read-only profile's settings file");
    let written: Value =
        serde_json::from_str(&std::fs::read_to_string(settings).expect("the settings file"))
            .expect("JSON");
    let profile = crate::review::profile_for("claude-code", "opus", "decide-opus", Effort::Medium);
    assert_eq!(profile.permissions, crate::ir::PermissionMode::ReadOnly);
    assert_eq!(
        written,
        crate::agent::claude::permission_settings(&profile, &[])
    );

    let prompt = String::from_utf8_lossy(&request.command.stdin);
    for fragment in [
        "id: q-01A\n",
        "raised by: the worker",
        "q-01A: every rung failed on the same assertion",
        "  1) retry on frontier",
        "t0 title (t0)",
        "t0 body: make the widget",
        "The widget task makes the widget.",
        "(no settled attempt of this task failed)",
        "exactly one fenced ```json block",
        "\"needs_a_person\"",
        "Giving up on the task is never an available answer here",
        "Nothing you write will be applied",
        "You may only read",
    ] {
        assert!(prompt.contains(fragment), "{fragment}: {prompt}");
    }
}

#[test]
fn a_binding_question_is_skipped_and_no_request_is_made_for_it() {
    let fixture = one_question("decide-binding");
    let observed = observe::observe(&fixture.repo, RUN)
        .expect("readable")
        .expect("schema 4");
    let mut binding = observed.open_questions().remove(0);
    binding.binds_an_agent = true;
    let runner = Scripted::answering(Vec::new());
    let decided = propose(
        &fixture.options(),
        &observed,
        &[binding],
        &BuiltinAdapters,
        &runner,
    )
    .expect("decided");
    assert!(runner.seen().is_empty(), "no agent was asked");
    assert!(
        decided
            .report
            .contains("q-01A: skipped — it chooses which agent runs a task, and binding questions are left to a person"),
        "{}",
        decided.report
    );
    assert!(decided.outcome().is_ok());
    assert!(
        !proposals_dir(&fixture.repo, RUN)
            .join("q-01A.1.json")
            .exists()
    );
}

#[test]
fn runs_and_questions_that_cannot_be_decided_are_refused_before_any_request() {
    let refused = |fixture: &Fixture, question_id: Option<&str>, run_id: &str| {
        let runner = Scripted::answering(Vec::new());
        let options = DecideOptions {
            run_id: run_id.to_owned(),
            question_id: question_id.map(str::to_owned),
            ..fixture.options()
        };
        let error = decide(&options, &BuiltinAdapters, &runner).expect_err("refused");
        assert!(runner.seen().is_empty(), "no request: {error}");
        error.to_string()
    };

    let unbound = fixture("decide-unbound", 1, None, |log| log.raise("q-01A", 0));
    let message = refused(&unbound, None, RUN);
    assert!(
        message.contains("recorded no primary review binding"),
        "{message}"
    );

    let settled = fixture("decide-settled", 1, reviewer(), |log| {
        log.raise("q-01A", 0);
        log.answer("q-01A", 0);
    });
    let message = refused(&settled, Some("q-01A"), RUN);
    assert!(
        message.contains("question q-01A of run 01DECIDERUN000000000000001 is no longer open"),
        "{message}"
    );

    let message = refused(&settled, None, "01NOSUCHRUN");
    assert!(message.contains("no run `01NOSUCHRUN`"), "{message}");

    let legacy_run = "01DECIDELEGACY";
    let public = crate::rundir::public_dir(&settled.repo, legacy_run);
    std::fs::create_dir_all(&public).expect("legacy run directory");
    std::fs::write(
        public.join(crate::rundir::EVENT_LOG),
        format!(
            "{{\"ts\":\"2026-08-20T00:00:00Z\",\"event\":\"run_started\",\
             \"data\":{{\"schema\":3,\"run_id\":\"{legacy_run}\"}}}}\n"
        ),
    )
    .expect("a schema-3 log");
    let message = refused(&settled, None, legacy_run);
    assert!(message.contains("is a schema 1-3 run"), "{message}");
}

#[test]
fn a_decide_leaves_every_file_of_the_run_as_it_was_and_writes_no_answer() {
    let fixture = one_question("decide-untouched");
    let public_before = snapshot(&fixture.public());
    let private_before = snapshot(&fixture.private);
    assert!(!public_before.is_empty() && !private_before.is_empty());

    let runner = Scripted::answering(vec![claude_output(&reply(VALID), Some(0.25))]);
    decide(&fixture.options(), &BuiltinAdapters, &runner).expect("decided");

    assert_eq!(snapshot(&fixture.public()), public_before);
    assert_eq!(snapshot(&fixture.private), private_before);
    let observed = observe::observe(&fixture.repo, RUN)
        .expect("readable")
        .expect("schema 4");
    assert!(!observed.paths().answers().exists(), "no answers directory");
    assert!(!fixture.public().join("answers").exists());
}

#[test]
fn a_third_proposal_is_refused_before_a_request_and_the_dollar_cap_stops_the_next_question() {
    let third = one_question("decide-third");
    for n in 1..=2 {
        let runner = Scripted::answering(vec![claude_output(&reply(VALID), None)]);
        let decided = decide(&third.options(), &BuiltinAdapters, &runner).expect("decided");
        assert_eq!(third.proposal("q-01A", n)["status"], "accepted");
        assert!(
            decided.report.contains("cost: unknown")
                && decided.report.contains("cost unknown for: q-01A"),
            "{}",
            decided.report
        );
    }
    let runner = Scripted::answering(vec![claude_output(&reply(VALID), None)]);
    let decided = decide(&third.options(), &BuiltinAdapters, &runner).expect("decided");
    assert!(runner.seen().is_empty(), "refused before any request");
    assert!(
        decided
            .report
            .contains("q-01A: refused — it already has 2 saved proposals"),
        "{}",
        decided.report
    );
    assert!(decided.outcome().is_err());
    assert!(
        !proposals_dir(&third.repo, RUN)
            .join("q-01A.3.json")
            .exists()
    );

    let capped = fixture("decide-cap", 2, reviewer(), |log| {
        log.raise("q-b", 0);
        log.raise("q-a", 1);
    });
    let runner = Scripted::answering(vec![
        claude_output(&reply(VALID), Some(1.0)),
        claude_output(&reply(VALID), Some(1.0)),
    ]);
    let options = DecideOptions {
        max_cost_usd: 1.0,
        ..capped.options()
    };
    let decided = decide(&options, &BuiltinAdapters, &runner).expect("decided");
    assert_eq!(
        runner.seen().len(),
        1,
        "the cap stops before the next question"
    );
    assert!(
        decided
            .report
            .contains("reaches the --max-cost-usd cap of $1.0000; left without a proposal: q-a"),
        "{}",
        decided.report
    );
}

/// The production region of a source file: everything before its trailing
/// `#[cfg(test)]` module.
fn production(source: &str) -> &str {
    source.split("\n#[cfg(test)]\n").next().unwrap_or(source)
}

#[test]
fn only_the_cli_names_the_decider_and_no_engine_file_reaches_it() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut checked = 0_usize;
    for (path, bytes) in snapshot(&src) {
        let Ok(relative) = path.strip_prefix(&src) else {
            continue;
        };
        let relative = relative.to_string_lossy().replace('\\', "/");
        let is_test_file = relative.ends_with("tests.rs")
            || relative.contains("/tests/")
            || relative.contains("test_support");
        if !relative.ends_with(".rs") || is_test_file {
            continue;
        }
        checked += 1;
        let source = String::from_utf8_lossy(&bytes);
        let code = production(&source);
        if relative != "decide.rs" && relative != "main.rs" {
            assert!(
                !code.contains("decide::"),
                "src/{relative} names `decide::`; only the CLI reaches the decider"
            );
            assert!(
                !code.contains("\"proposals\"") && !code.contains("/proposals"),
                "src/{relative} names the proposals directory"
            );
        }
        if relative.starts_with("engine/") {
            assert!(
                !source.contains("crate::decide"),
                "src/{relative} names `crate::decide`; no engine code calls the decider"
            );
        }
    }
    assert!(checked > 100, "the census read the tree: {checked} files");
}
