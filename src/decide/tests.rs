use std::collections::{BTreeMap, VecDeque};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use super::*;
use crate::agent::Caps;
use crate::engine::BuiltinAdapters;
use crate::ir::{Effort, Outcome, QuestionId, QuestionKind, TaskId};
use crate::review::PassBinding;
use crate::rundir::scratch_tree::ScratchTree;
use crate::runner::{
    ExecutionRole, ProcessFate, RunFuture, RunnerCall, RunnerError, RunnerRequest,
};
use crate::topology::events::{
    Answer4, AttemptFinished4, AttemptNumber, FrozenQuestion, GenerationId, ImageIdentity,
    LeaseDisposition, MergeVerificationUnavailable, QuestionAnswered4, QuestionRaised4,
    RunStarted4, RunnerContract, RunnerPolicy, SequenceId, UnavailableCause, UnavailableOutcome,
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
            "2 fenced blocks",
        ),
        (VALID.to_owned(), "no fenced block"),
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

/// `VALID` with its guidance replaced by `guidance`, written as a JSON string.
fn with_guidance(guidance: &str) -> String {
    VALID.replace(
        "\"guidance\": \"retry\"",
        &format!("\"guidance\": {}", json!(guidance)),
    )
}

#[test]
fn a_fence_mentioned_in_the_guidance_is_text_and_only_a_whole_line_is_a_fence() {
    let asked = question(&[RETRY, crate::interaction::GIVE_UP_OPTION]);
    for guidance in [
        "close the block with ```",
        "wrap the patch in a ```json fence",
        "```\n```json\n```",
    ] {
        let body = with_guidance(guidance);
        let spread: Value = serde_json::from_str(&body).expect("JSON");
        let spread = serde_json::to_string_pretty(&spread).expect("pretty JSON");
        for text in [reply(&body), reply(&spread)] {
            let record = check_reply(&asked, &text)
                .unwrap_or_else(|reason| panic!("{guidance:?} is accepted: {reason}\n{text}"));
            assert_eq!(record["guidance"], guidance, "kept as written: {text}");
        }
    }

    let framed = |text: String| check_reply(&asked, &text);
    assert!(
        framed(format!(
            "my proposal\r\n  ```json \t\r\n{VALID}\r\n```  \r\n"
        ))
        .is_ok(),
        "a fence line may carry whitespace and a carriage return around it"
    );
    assert!(
        framed(format!("```json\n{VALID}\n```")).is_ok(),
        "the last line closes a block with no line break after it"
    );
    assert!(
        framed(format!("```src/lib.rs``` holds it\n{}", reply(VALID))).is_ok(),
        "a line opening with inline code opens no block"
    );
    for (text, needle) in [
        (
            format!("my proposal: ```json {VALID} ```"),
            "no fenced block",
        ),
        (format!("```json\n{VALID}\n"), "never closed"),
        (
            format!("```json\n{VALID}\n``` that is all\n"),
            "never closed",
        ),
        (format!("```JSON\n{VALID}\n```\n"), "not a json block"),
        (
            format!("{}{}", reply(VALID), reply(VALID)),
            "2 fenced blocks",
        ),
    ] {
        let refused = framed(text.clone()).expect_err("refused");
        assert!(refused.contains(needle), "{needle}: {refused}\n{text}");
    }
}

#[test]
fn a_reply_with_a_second_fenced_block_is_refused() {
    let asked = question(&[RETRY, crate::interaction::GIVE_UP_OPTION]);
    let proposal = format!("```json\n{VALID}\n```\n");
    assert!(
        check_reply(&asked, &proposal).is_ok(),
        "the proposal alone is accepted"
    );
    for (text, count, why) in [
        (
            format!("{proposal}```text\nmy notes\n```\n"),
            2,
            "a closed text block after the proposal",
        ),
        (
            format!("```text\nmy notes\n```\n{proposal}"),
            2,
            "a closed text block before the proposal",
        ),
        (
            format!("{proposal}```\nsome output\n```\n"),
            2,
            "a block with no info string",
        ),
        (
            format!("{proposal}```text\nmy notes\n"),
            2,
            "a block left open, which runs to the end of the reply",
        ),
        (
            format!("```text\nmy notes\n```\n{proposal}```\nsome output\n```\n"),
            3,
            "a block before the proposal and one after it",
        ),
        (
            format!("an example:\n```text\n```json\n```\n{}", reply(VALID)),
            2,
            "a ```json line inside another block is that block's text, and the block is counted",
        ),
        (
            format!("{}and then:\n```sh\ncargo test\n", reply(VALID)),
            2,
            "another block left open runs to the end and is counted",
        ),
    ] {
        let refused = check_reply(&asked, &text).expect_err(why);
        assert!(
            refused.contains(&format!("the reply holds {count} fenced blocks")),
            "{why}: {refused}\n{text}"
        );
    }
}

/// A question the coordinator froze for a parked task.
fn frozen(id: &str) -> FrozenQuestion {
    FrozenQuestion {
        id: QuestionId::from(id),
        key: TaskKey(0),
        kind: QuestionKind::Clarify,
        context: format!("{id}: which of two formats to keep"),
        options: vec![RETRY.to_owned()],
    }
}

fn stamped(body: TopologyEventBody) -> TopologyEvent {
    TopologyEvent {
        ts: STAMPED_AT.to_owned(),
        body,
    }
}

/// An attempt that settled parked on a failure of `kind` from `origin`.
fn parked_on(kind: FailureKind, origin: FailureOrigin) -> TopologyEvent {
    stamped(TopologyEventBody::AttemptFinished {
        data: Box::new(AttemptFinished4 {
            key: TaskKey(0),
            generation: GenerationId(0),
            attempt: AttemptNumber(1),
            record: Box::new(crate::events::AttemptRecord {
                attempt: 1,
                tier: "frontier".to_owned(),
                model: "a-model".to_owned(),
                pool: None,
                resumed: false,
                duration: Duration::ZERO,
                cost_usd: None,
                reviews: Vec::new(),
                session_id: None,
                usage: None,
                failure: Some(FailureRecord {
                    kind,
                    origin,
                    reason: "the attempt stopped".to_owned(),
                    detail: None,
                }),
            }),
            settlement: AttemptSettlement::Closed {
                transition: SettlementTransition::Parked {
                    question: frozen("q-parked"),
                },
                lease: LeaseDisposition::PredictedReleased,
            },
        }),
    })
}

#[test]
fn a_question_is_labelled_by_whether_the_worker_or_the_coordinator_raised_it() {
    let spend = stamped(TopologyEventBody::QuestionRaised {
        data: QuestionRaised4 {
            question: FrozenQuestion {
                kind: QuestionKind::ApproveSpend,
                ..frozen("q-spend")
            },
        },
    });
    let verification = stamped(TopologyEventBody::MergeVerificationUnavailable {
        data: MergeVerificationUnavailable {
            sequence: SequenceId(1),
            cause: UnavailableCause::HumanRequired {
                verdict: "a person chooses the format".to_owned(),
            },
            outcome: UnavailableOutcome::Parked {
                question: frozen("q-verify"),
            },
            reviews: Vec::new(),
        },
    });
    let cases = [
        (
            "the worker asked for a person",
            parked_on(FailureKind::NeedsHuman, FailureOrigin::Worker),
            RaisedBy::Worker,
        ),
        (
            "a reviewer asked for a person",
            parked_on(FailureKind::NeedsHuman, FailureOrigin::Reviewer),
            RaisedBy::Coordinator,
        ),
        (
            "every rung failed its gates",
            parked_on(FailureKind::GateFailed, FailureOrigin::Worker),
            RaisedBy::Coordinator,
        ),
        (
            "the review input was too large",
            parked_on(FailureKind::ReviewInputTooLarge, FailureOrigin::Reviewer),
            RaisedBy::Coordinator,
        ),
        (
            "the deferrals ran out",
            parked_on(FailureKind::RateLimited, FailureOrigin::Worker),
            RaisedBy::Coordinator,
        ),
        ("a spend approval", spend, RaisedBy::Coordinator),
        (
            "integration verification",
            verification,
            RaisedBy::Coordinator,
        ),
    ];
    for (case, event, by) in cases {
        assert_eq!(RaisedBy::of(&event), by, "{case}");
    }

    let asked = question(&[RETRY]);
    let worker = question_text(&asked, Some(RaisedBy::Worker));
    assert!(worker.contains("raised by: the worker\n"), "{worker}");
    assert!(!worker.contains("coordinator"), "{worker}");
    let coordinator = question_text(&asked, Some(RaisedBy::Coordinator));
    for line in [
        "raised by: the coordinator\n",
        "The coordinator raised this question itself; the task's worker did not ask it.\n",
    ] {
        assert!(coordinator.contains(line), "{line}: {coordinator}");
    }
    let unknown = question_text(&asked, None);
    assert!(unknown.contains("raised by: unknown"), "{unknown}");
    assert!(!unknown.contains("worker"), "{unknown}");
}

/// Agent text written to take a terminal over: a forged report line, a screen clear, a bell.
const HOSTILE: &str = "retry\nq-02: decided by someone/else at max effort\u{1b}[2J\u{7}";
/// `HOSTILE` as a report line shows it.
const SHOWN: &str = "retry q-02: decided by someone/else at max effort\\u{1b}[2J\\u{7}";

/// Whether `report` holds no control character but the line ends its builder wrote.
fn only_layout_controls(report: &str) -> bool {
    !report.chars().any(|c| c.is_control() && c != '\n')
}

/// A decider for rendering alone: nothing is built or started with it.
fn rendering_decider() -> Decider<'static> {
    Decider {
        adapter: &crate::agent::claude::ClaudeCodeAdapter,
        agent: crate::agent::claude::ADAPTER_ID.to_owned(),
        model: "opus".to_owned(),
        effort: Effort::Medium,
        timeout: Duration::from_secs(1),
        credential: None,
        private_denials: Vec::new(),
    }
}

#[test]
fn agent_text_is_rendered_inside_its_own_line_with_its_controls_made_visible() {
    let asked = question(&["retry \u{1b}[31mnow", crate::interaction::GIVE_UP_OPTION]);
    let record = json!({
        "choice": 1,
        "guidance": HOSTILE,
        "reasons": [HOSTILE],
        "confidence": "high",
        "needs_a_person": false,
        "citations": [HOSTILE],
    });
    let decider = rendering_decider();
    let mut out = TerminalLines::default();
    let saved = Path::new("q-1.1.json");
    render_one(&mut out, &decider, &asked, &Ok(record), Some(0.25), saved);
    render_one(
        &mut out,
        &decider,
        &asked,
        &Err(HOSTILE.to_owned()),
        None,
        saved,
    );
    let report = out.into_string();
    assert!(only_layout_controls(&report), "{report:?}");
    assert!(
        !report.lines().any(|line| line.starts_with("q-02")),
        "no line is forged: {report}"
    );
    for line in [
        "  choice: 1) retry \\u{1b}[31mnow".to_owned(),
        format!("  guidance: {SHOWN}"),
        format!("  reason: {SHOWN}"),
        format!("  citation: {SHOWN}"),
        format!("  status: refused — {SHOWN}"),
    ] {
        assert!(report.lines().any(|kept| kept == line), "{line}: {report}");
    }

    let decided = Decided {
        report,
        refused: vec!["q-1".to_owned(), HOSTILE.to_owned()],
        left: vec![HOSTILE.to_owned()],
    };
    let message = decided.outcome().expect_err("refused").to_string();
    assert!(!message.chars().any(char::is_control), "{message:?}");
    assert!(message.contains(&format!("q-1, {SHOWN}")), "{message}");
    assert!(
        message.contains(&format!("left without a proposal ({SHOWN})")),
        "{message}"
    );
}

/// `claude --help` as the probe reads it: every flag the adapter requires, and every effort.
const CLAUDE_HELP: &str = "Usage: claude [options]\n  -p, --print\n  --output-format <format>\n  \
                           --model <model>\n  --settings <file>\n  --setting-sources <sources>\n  \
                           --permission-mode <mode>\n  \
                           --effort <level> (low, medium, high, xhigh, max)\n";
const SIGNED_IN: &str = r#"{"loggedIn":true,"authMethod":"claude.ai","apiProvider":"firstParty"}"#;
const SIGNED_OUT: &str = r#"{"loggedIn":false,"authMethod":"none","apiProvider":"firstParty"}"#;

/// What an installed Claude Code answers a pre-flight probe with, its sign-in reported as
/// `auth_status`: `None` for a request that is no probe. No process is started.
fn probe_reply(request: &RunnerRequest, auth_status: &str) -> Option<ProcessOutput> {
    if !matches!(request.role, ExecutionRole::Probe(_)) {
        return None;
    }
    let args = &request.command.args;
    let stdout = if args.iter().any(|arg| arg == "--version") {
        "2.1.226 (Claude Code)\n".to_owned()
    } else if args.iter().any(|arg| arg == "--help") {
        CLAUDE_HELP.to_owned()
    } else {
        auth_status.to_owned()
    };
    Some(exited(stdout))
}

/// A Runner that records what it was asked to run and answers from a table,
/// in order, so `decide` is driven with no process and no network. It answers
/// the pre-flight's probes as a signed-in Claude Code, apart from the table, and
/// records them apart from the calls.
struct Scripted {
    replies: Mutex<VecDeque<String>>,
    seen: Mutex<Vec<RunnerRequest>>,
    probes: Mutex<Vec<RunnerRequest>>,
}

impl Scripted {
    fn answering(replies: Vec<String>) -> Self {
        Self {
            replies: Mutex::new(replies.into()),
            seen: Mutex::new(Vec::new()),
            probes: Mutex::new(Vec::new()),
        }
    }

    fn seen(&self) -> Vec<RunnerRequest> {
        self.seen
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn probes(&self) -> Vec<RunnerRequest> {
        lock(&self.probes).clone()
    }
}

impl Runner for Scripted {
    fn run<'a>(&'a self, request: &'a RunnerRequest, _call: RunnerCall<'a>) -> RunFuture<'a> {
        if let Some(probed) = probe_reply(request, SIGNED_IN) {
            lock(&self.probes).push(request.clone());
            return Box::pin(async move { Ok(probed) });
        }
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
        // The hash the markdown adapter gives the fixture's plan file, so the run recorded
        // the plan that file holds.
        source: PlanSource {
            adapter: "markdown".to_owned(),
            hash: crate::ir::content_hash(PLAN_TEXT.as_bytes()),
        },
        tasks: (0..tasks).map(task).collect(),
        artifacts: Vec::new(),
    }
}

fn host_runner() -> RunnerPolicy {
    RunnerPolicy {
        kind: RunnerKind::Host,
        policy: RunnerContract::HostV1,
        image: None,
        credential_volumes: None,
        mounts: Vec::new(),
    }
}

fn container_runner() -> RunnerPolicy {
    RunnerPolicy {
        kind: RunnerKind::Container,
        policy: RunnerContract::ContainerV1,
        image: Some(ImageIdentity {
            reference: "ghcr.io/example/upstroke-runner:v1".to_owned(),
            id: "sha256:11223344556677889900aabbccddeeff11223344556677889900aabbccddeeff"
                .to_owned(),
            digest: None,
        }),
        credential_volumes: Some(BTreeMap::from([(
            crate::agent::claude::ADAPTER_ID.to_owned(),
            "upstroke-creds-claude".to_owned(),
        )])),
        mounts: Vec::new(),
    }
}

fn run_started(
    plan: &crate::ir::Plan,
    normalized_plan_digest: &str,
    private: &Path,
    setup: Setup,
) -> RunStarted4 {
    use crate::events::{BindingSummary, ChainSummary, GateSummary};
    use crate::gates::ShellKind;
    use crate::ir::{ResolvedEffortPolicy, Tier};
    use crate::review::ReviewPlan;
    use crate::topology::events::{CommitSha, GitRef, IncarnationId, TopologyLimits};
    use crate::topology::paths::{PathGrammar, PathPolicy, PathPolicyVersion};

    let Setup {
        primary, runner, ..
    } = setup;
    let mut probed_agents = vec![crate::agent::claude::ADAPTER_ID.to_owned()];
    probed_agents.extend(primary.iter().map(|binding| binding.agent.clone()));
    probed_agents.sort_unstable();
    probed_agents.dedup();

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
        runner,
        probed_agents,
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
    profile: PathBuf,
}

/// The coordinator environment of every fixture: no variable is set, so the credential
/// preflight never reads this test process's own.
fn no_variables(_key: &str) -> Option<OsString> {
    None
}

impl Fixture {
    fn options(&self) -> DecideOptions {
        DecideOptions {
            repo_root: self.repo.clone(),
            run_id: RUN.to_owned(),
            question_id: None,
            max_cost_usd: DEFAULT_MAX_COST_USD,
            pools_path: Some(self.repo.join("pools.toml")),
            environment: Some(no_variables),
        }
    }

    fn proposals_written(&self) -> bool {
        proposals_dir(&self.repo, RUN).exists()
    }

    fn public(&self) -> PathBuf {
        crate::rundir::public_dir(&self.repo, RUN)
    }

    fn proposal(&self, id: &str) -> Value {
        let path = proposals_dir(&self.repo, RUN).join(format!("{id}.json"));
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        serde_json::from_str(&text).expect("a proposal is JSON")
    }

    fn has_proposal(&self, id: &str) -> bool {
        proposals_dir(&self.repo, RUN)
            .join(format!("{id}.json"))
            .exists()
    }

    /// `.upstroke/proposals`, created owner-only as an earlier decide of another run, another
    /// caller, would have left it.
    fn proposals_parent(&self) -> PathBuf {
        let parent = self.repo.join(".upstroke").join("proposals");
        std::fs::create_dir_all(&parent).expect("the proposals directory");
        set_mode(&parent, 0o700);
        parent
    }
}

/// Sets `path`'s permission bits on Unix; elsewhere, nothing.
#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).expect("set a mode");
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) {}

/// A pools file whose one pool binds `agent` to the credential profile `profile`.
fn pools(agent: &str, profile: &str) -> String {
    format!(
        "[pools.claude-max]\nkind = \"subscription-window\"\nagent = \"{agent}\"\nprofile = '{profile}'\n"
    )
}

/// What the recorded run looks like: its primary reviewer, its runner, and the name of the
/// scratch directory its private directory is under.
struct Setup {
    primary: Option<PassBinding>,
    runner: RunnerPolicy,
    private_root: &'static str,
}

impl Setup {
    fn host(primary: Option<PassBinding>) -> Self {
        Self {
            primary,
            runner: host_runner(),
            private_root: "private",
        }
    }
}

/// A schema-4 run with `tasks` tasks whose log `write` fills after `run_started`.
fn fixture(
    tag: &str,
    tasks: usize,
    primary: Option<PassBinding>,
    write: impl FnOnce(&mut Log),
) -> Fixture {
    fixture_with(tag, tasks, Setup::host(primary), write)
}

/// [`fixture`], recorded as `setup` says. The pools file binds Claude Code to a credential
/// profile directory that exists in the scratch tree, so the credential preflight admits it.
fn fixture_with(tag: &str, tasks: usize, setup: Setup, write: impl FnOnce(&mut Log)) -> Fixture {
    let tree = scratch(tag);
    let repo = tree.path().join("repo");
    let private = tree.path().join(setup.private_root).join(RUN);
    let profile = tree.path().join("profiles").join("claude-max");
    std::fs::create_dir_all(repo.join("docs")).expect("docs");
    std::fs::write(repo.join("docs").join("plan.md"), PLAN_TEXT).expect("the plan file");
    std::fs::create_dir_all(&profile).expect("the credential profile directory");
    std::fs::write(
        repo.join("pools.toml"),
        pools(crate::agent::claude::ADAPTER_ID, &profile.to_string_lossy()),
    )
    .expect("the pools file");
    std::fs::create_dir_all(&private).expect("the private directory");
    std::fs::write(private.join("transcript.txt"), "a transcript\n").expect("a transcript");

    let plan = plan(tasks);
    let mut plan_bytes = serde_json::to_vec_pretty(&plan).expect("the plan serializes");
    plan_bytes.push(b'\n');
    let normalized_plan_digest = crate::events::normalized_plan_digest(&plan_bytes);
    let public = crate::rundir::public_dir(&repo, RUN);
    let hooks = &mut crate::rundir::NoHooks;
    crate::rundir::create_public_dir(&public, hooks).expect("the run's public directory");
    // `.upstroke` as a run leaves it under the common umask, whatever this process's own is.
    set_mode(&repo.join(".upstroke"), 0o755);
    crate::rundir::write_plan(&public, &plan_bytes, hooks).expect("the frozen plan");
    let path = public.join(crate::rundir::EVENT_LOG);
    let log = crate::events::EventLog::open(
        crate::topology::effects::EventSite::OpenLog,
        &path,
        &mut Vec::new(),
    )
    .expect("open the run's log");
    let started = run_started(&plan, &normalized_plan_digest, &private, setup);
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
        profile,
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

    let saved = fixture.proposal("q-01A");
    assert_eq!(saved["run_id"], RUN);
    assert_eq!(saved["question_id"], "q-01A");
    assert_eq!(saved["agent"], "claude-code");
    assert_eq!(saved["model"], "opus");
    assert_eq!(saved["effort"], "medium");
    assert_eq!(saved["status"], "accepted");
    assert_eq!(saved["cost_usd"], 0.25);
    assert_eq!(saved["settled"], true);
    assert_eq!(saved["ceiling_usd"], 5.0);
    assert_eq!(saved["record"]["choice"], 1);
    for field in FIELDS {
        assert!(saved["record"].get(field).is_some(), "{field}: {saved}");
    }
    let raw = saved["raw_output"].as_str().expect("a reference");
    assert_eq!(
        raw,
        format!(".upstroke/proposals/{RUN}/q-01A.out"),
        "the raw output is named from the repository root, `/`-separated on every platform"
    );
    assert_eq!(
        fixture.repo.join(raw),
        proposals_dir(&fixture.repo, RUN).join("q-01A.out"),
        "the raw output is saved beside the proposal"
    );
    assert_eq!(
        std::fs::read_to_string(fixture.repo.join(raw)).expect("the raw output"),
        claude_output(&reply(VALID), Some(0.25))
    );
    assert!(report.contains(&format!(
        "saved: {}",
        proposals_dir(&fixture.repo, RUN)
            .join("q-01A.json")
            .display()
    )));
    assert!(
        report.contains(&format!(
            "claimed: {}; decide asks a run once",
            proposals_dir(&fixture.repo, RUN).display()
        )),
        "{report}"
    );
}

#[test]
fn a_saved_proposal_names_its_raw_output_wherever_the_repository_is_moved() {
    let fixture = one_question("decide-moved-repo");
    let runner = Scripted::answering(vec![claude_output(&reply(VALID), Some(0.25))]);
    decide(&fixture.options(), &BuiltinAdapters, &runner).expect("decided");
    let moved = fixture.repo.with_file_name("moved-repo");
    std::fs::rename(&fixture.repo, &moved).expect("move the repository");
    let path = proposals_dir(&moved, RUN).join("q-01A.json");
    let saved: Value =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("the proposal")).expect("JSON");
    let raw = saved["raw_output"].as_str().expect("a reference");
    assert!(
        !Path::new(raw).is_absolute() && !raw.contains('\\'),
        "relative and `/`-separated: {raw}"
    );
    assert_eq!(
        std::fs::read_to_string(moved.join(raw)).expect("the reference resolves from the new root"),
        claude_output(&reply(VALID), Some(0.25))
    );
}

#[test]
fn a_decide_report_shows_agent_text_as_data_and_the_proposal_keeps_it_as_written() {
    let fixture = one_question("decide-hostile");
    let body = with_guidance(HOSTILE);
    let runner = Scripted::answering(vec![claude_output(&reply(&body), Some(0.25))]);
    let decided = decide(&fixture.options(), &BuiltinAdapters, &runner).expect("decided");
    let report = &decided.report;
    assert!(decided.outcome().is_ok(), "{report}");
    assert!(only_layout_controls(report), "{report:?}");
    assert!(
        report
            .lines()
            .any(|line| line == format!("  guidance: {SHOWN}")),
        "{report}"
    );
    assert!(
        !report.lines().any(|line| line.starts_with("q-02")),
        "no line is forged: {report}"
    );
    assert_eq!(
        fixture.proposal("q-01A")["record"]["guidance"],
        HOSTILE,
        "the saved record keeps the text as the agent wrote it"
    );
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
        let saved = fixture.proposal(id);
        assert_eq!(saved["status"], "refused", "{id}: {saved}");
        assert_eq!(
            saved["settled"], true,
            "{id}: a clean, priced call is settled at its price whatever its reply"
        );
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
    assert_eq!(fixture.proposal("q-a")["status"], "accepted");
    assert_eq!(decided.refused, ids[..5].to_vec());
    assert!(decided.left.is_empty(), "{}", decided.report);
    assert!(
        decided.report.contains("total cost: $0.6000"),
        "{}",
        decided.report
    );
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
        Some(fixture.profile.clone()),
        "the first pool of the reviewer's agent"
    );

    let settings = request
        .command
        .settings_file
        .as_ref()
        .expect("the read-only profile's settings file");
    assert_eq!(
        settings,
        &proposals_dir(&fixture.repo, RUN).join("q-01A.settings.json")
    );
    let written: Value =
        serde_json::from_str(&std::fs::read_to_string(settings).expect("the settings file"))
            .expect("JSON");
    let profile = crate::review::profile_for("claude-code", "opus", "decide-opus", Effort::Medium);
    assert_eq!(profile.permissions, crate::ir::PermissionMode::ReadOnly);
    let mut expected = crate::agent::claude::permission_settings(&profile, &[]);
    let denials = private_denials(&fixture.private.to_string_lossy()).expect("spellable");
    if let Some(deny) = expected
        .pointer_mut("/permissions/deny")
        .and_then(Value::as_array_mut)
    {
        deny.extend(denials.iter().cloned().map(Value::String));
    }
    assert_eq!(
        written, expected,
        "the read-only profile's settings, and reads of the run's private directory denied"
    );
    let recorded = rule_path(&fixture.private).expect("an absolute fixture path");
    for rule in [format!("Read({recorded})"), format!("Read({recorded}/**)")] {
        assert!(denials.contains(&rule), "{rule}: {denials:?}");
    }

    let args = &request.command.args;
    let at = args
        .iter()
        .position(|arg| arg == "--max-budget-usd")
        .expect("the call's own ceiling");
    assert_eq!(
        args.get(at + 1).map(String::as_str),
        Some("5"),
        "all of --max-cost-usd is left for the first call: {args:?}"
    );
    assert!(
        args.iter().any(|arg| arg == "--strict-mcp-config"),
        "{args:?}"
    );
    assert_eq!(
        (
            flag_value(request, "--model").as_deref(),
            flag_value(request, "--effort").as_deref()
        ),
        (Some("opus"), Some("medium")),
        "the recorded reviewer's model, at the run's recorded review effort: {args:?}"
    );

    let prompt = String::from_utf8_lossy(&request.command.stdin);
    for fragment in [
        "id: q-01A\n",
        "raised by: the coordinator\n",
        "The coordinator raised this question itself; the task's worker did not ask it.",
        "q-01A: every rung failed on the same assertion",
        "  1) retry on frontier",
        "t0 title (t0)",
        "t0 body: make the widget",
        "The widget task makes the widget.",
        "(no settled attempt of this task failed)",
        "exactly one fenced ```json block",
        "Its opening ```json and its closing ``` are each a line of their own",
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
        runner.probes().is_empty(),
        "with no question to ask, the reviewer's CLI is not probed"
    );
    assert!(
        decided
            .report
            .contains("q-01A: skipped — it chooses which agent runs a task, and binding questions are left to a person"),
        "{}",
        decided.report
    );
    assert!(decided.outcome().is_ok());
    assert!(
        !fixture.repo.join(".upstroke").join("proposals").exists(),
        "with no call to make, the run is not claimed and nothing is created"
    );
}

/// At most one call per question in a command: a question listed twice is asked once.
#[test]
fn a_question_listed_twice_is_asked_once() {
    let fixture = one_question("decide-twice");
    let observed = observe::observe(&fixture.repo, RUN)
        .expect("readable")
        .expect("schema 4");
    let mut twice = observed.open_questions();
    twice.extend(observed.open_questions());
    assert_eq!(twice.len(), 2, "the run's one question, listed twice");
    let runner = Acting::priced(0.25);
    let decided = propose(
        &fixture.options(),
        &observed,
        &twice,
        &BuiltinAdapters,
        &runner,
    )
    .expect("decided");
    assert_eq!(runner.seen().len(), 1, "{}", decided.report);
    assert!(
        decided.report.contains(
            "q-01A: skipped — it was already asked in this command, and a question gets at most \
             one call"
        ),
        "{}",
        decided.report
    );
    assert!(decided.outcome().is_ok(), "{}", decided.report);
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

/// Two questions, `q-b` raised first.
fn two_questions(tag: &str) -> Fixture {
    fixture(tag, 2, reviewer(), |log| {
        log.raise("q-b", 0);
        log.raise("q-a", 1);
    })
}

/// Spend: the allowance is the command's, so the run's (a run is asked once), and a settled
/// call counts at its price, above its ceiling included. Once nothing is left the next question
/// is not started, and the command names it and fails.
#[test]
fn the_allowance_stops_the_next_question_and_the_command_names_it_and_fails() {
    // The control: the same two questions and $1 cap, the first call priced under it.
    let control = two_questions("decide-cap-control");
    let options = DecideOptions {
        max_cost_usd: 1.0,
        ..control.options()
    };
    let runner = Acting::priced(0.5);
    let decided = decide(&options, &BuiltinAdapters, &runner).expect("decided");
    assert_eq!(
        ceilings(&runner.seen()),
        [Some("1".to_owned()), Some("0.5".to_owned())],
        "each call is given what the earlier one left"
    );
    assert!(decided.outcome().is_ok(), "{}", decided.report);

    for (tag, cap, cost, spent) in [
        ("decide-cap-reached", 1.0, 1.0, "$1.0000"),
        ("decide-cap-overrun", DEFAULT_MAX_COST_USD, 7.0, "$7.0000"),
    ] {
        let capped = two_questions(tag);
        let options = DecideOptions {
            max_cost_usd: cap,
            ..capped.options()
        };
        let runner = Acting::priced(cost);
        let decided = decide(&options, &BuiltinAdapters, &runner).expect("decided");
        assert_eq!(
            runner.seen().len(),
            1,
            "{tag}: the next question is not started"
        );
        assert_eq!(capped.proposal("q-b")["cost_usd"], cost, "{tag}");
        assert!(!capped.has_proposal("q-a"), "{tag}");
        assert!(
            decided.report.contains(&format!(
                "stopped: this command's calls have spent {spent}, which reaches the \
                 --max-cost-usd allowance of ${cap:.4} for the run, so no further call is \
                 started; left without a proposal: q-a"
            )),
            "{tag}: {}",
            decided.report
        );
        assert!(
            decided.report.contains(&format!("total cost: {spent}")),
            "{tag}: settled at its price: {}",
            decided.report
        );
        assert_eq!(decided.left, ["q-a"], "{tag}");
        let error = decided.outcome().expect_err("work was left").to_string();
        assert!(
            error.contains("1 question(s) left without a proposal (q-a)"),
            "{error}"
        );
    }
}

/// Lays an earlier claim of the run down under an owner-only `.upstroke/proposals`: the run's
/// directory, holding `files`.
fn earlier_claim(fixture: &Fixture, files: &[(&str, &str)]) -> PathBuf {
    let dir = proposals_dir(&fixture.repo, RUN);
    fixture.proposals_parent();
    std::fs::create_dir(&dir).expect("an earlier claim");
    set_mode(&dir, 0o700);
    for (name, text) in files {
        std::fs::write(dir.join(name), text).expect("an earlier file");
    }
    dir
}

/// Asserts that `decide` was refused as a run claimed already, with no call and nothing under
/// `.upstroke/proposals` written or changed, and returns the refusal's words.
fn refused_as_claimed(fixture: &Fixture, case: &str) -> String {
    let parent = fixture.repo.join(".upstroke").join("proposals");
    let before = snapshot(&parent);
    let runner = Acting::priced(0.25);
    let message = decide(&fixture.options(), &BuiltinAdapters, &runner)
        .map(|decided| decided.report)
        .expect_err(case)
        .to_string();
    assert!(runner.seen().is_empty(), "{case}: no agent was asked");
    assert_eq!(snapshot(&parent), before, "{case}: nothing was written");
    assert!(
        message.contains(&format!("run `{RUN}` was claimed already"))
            && message.contains(NOTHING_DONE),
        "{case}: {message}"
    );
    message
}

/// Directory: the run's directory is created exclusively, and whatever already stands at its
/// name (an empty directory, an older format's history, a partial or a complete earlier run, a
/// file or a link) means no write and no call.
#[test]
fn a_run_whose_directory_exists_is_not_asked_and_nothing_is_written() {
    // The control: the same run with nothing at the name is claimed and asked once.
    let control = one_question("decide-unclaimed");
    let runner = Acting::priced(0.25);
    decide(&control.options(), &BuiltinAdapters, &runner).expect("decided");
    assert_eq!(runner.seen().len(), 1);
    assert!(proposals_dir(&control.repo, RUN).is_dir());

    let older = r#"{"sequence": 1, "question_id": "q-01A", "slot": 1, "reserved_usd": 5.0}"#;
    let completed = r#"{"question_id": "q-01A", "status": "accepted", "cost_usd": 0.25}"#;
    let cases: [(&str, &[(&str, &str)]); 4] = [
        ("an empty directory", &[]),
        (
            "an older format's history",
            &[
                ("reservation-1.json", older),
                ("q-01A.1-decide.json", "{}"),
                ("q-01A.1.out", "raw"),
                ("q-01A.1.json", completed),
            ],
        ),
        ("a partial run", &[("q-01A.settings.json", "{}")]),
        (
            "a complete run",
            &[
                ("q-01A.settings.json", "{}"),
                ("q-01A.out", "raw"),
                ("q-01A.json", completed),
            ],
        ),
    ];
    for (at, (case, files)) in cases.into_iter().enumerate() {
        let fixture = one_question(&format!("decide-claimed-{at}"));
        earlier_claim(&fixture, files);
        refused_as_claimed(&fixture, case);
    }

    let filed = one_question("decide-claimed-file");
    let parent = filed.proposals_parent();
    std::fs::write(parent.join(RUN), "not a directory").expect("a file at the name");
    refused_as_claimed(&filed, "a file at the name");

    #[cfg(unix)]
    {
        let linked = one_question("decide-claimed-link");
        let parent = linked.proposals_parent();
        let elsewhere = linked.repo.join("elsewhere");
        std::fs::create_dir_all(&elsewhere).expect("a directory elsewhere");
        std::os::unix::fs::symlink(&elsewhere, parent.join(RUN)).expect("a link");
        refused_as_claimed(&linked, "a link at the name");
        assert!(
            snapshot(&elsewhere).is_empty(),
            "nothing was written through the link"
        );
    }
}

/// Directory and interruption: the run is claimed owner-only immediately before its first call,
/// and the claim stays after the call is interrupted, so a later decide asks nothing.
#[test]
fn the_run_is_claimed_owner_only_before_its_first_call_and_the_claim_outlives_an_interruption() {
    let fixture = one_question("decide-claim");
    let repo = fixture.repo.clone();
    let runner = Acting::new(move |_| {
        let dir = proposals_dir(&repo, RUN);
        let names: Vec<String> = snapshot(&dir)
            .keys()
            .filter_map(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            names,
            ["q-01A.settings.json"],
            "claimed, and holding only the call's settings, while the call is in flight"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt as _;
            let owner = std::fs::metadata(&repo).expect("the repository").uid();
            for made in [dir.clone(), repo.join(".upstroke").join("proposals")] {
                let metadata = std::fs::symlink_metadata(&made).expect("created");
                assert!(metadata.file_type().is_dir(), "{}", made.display());
                assert_eq!(metadata.uid(), owner, "{}", made.display());
                assert_eq!(metadata.mode() & 0o777, 0o700, "{}", made.display());
            }
        }
        Ok(exited(claude_output(&reply(VALID), Some(0.25))))
    });
    let decided = decide(&fixture.options(), &BuiltinAdapters, &runner).expect("decided");
    assert_eq!(runner.seen().len(), 1, "{}", decided.report);
    assert!(decided.outcome().is_ok(), "{}", decided.report);
    refused_as_claimed(&fixture, "a run asked once already");

    let interrupted = two_questions("decide-interrupted");
    let runner = Acting::new(|_| panic!("the launcher is killed while its call is in flight"));
    let options = interrupted.options();
    let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        decide(&options, &BuiltinAdapters, &runner)
    }));
    assert!(unwound.is_err(), "the launcher died mid-call");
    let dir = proposals_dir(&interrupted.repo, RUN);
    assert!(dir.join("q-b.settings.json").exists(), "the claim stays");
    assert!(!dir.join("q-b.out").exists() && !interrupted.has_proposal("q-b"));
    refused_as_claimed(&interrupted, "an interrupted run");
}

/// Directory durability: each directory's parent is synced whoever created it, the run's own
/// `.upstroke` and another caller's `.upstroke/proposals` included. A parent that cannot be
/// synced ends the command before any call, and the run is claimed only if its own directory
/// was created.
#[test]
fn each_parent_is_synced_whoever_created_it() {
    // The control: another caller's owner-only `.upstroke/proposals`, nothing armed.
    let control = one_question("decide-synced-control");
    control.proposals_parent();
    let runner = Acting::priced(0.25);
    decide(&control.options(), &BuiltinAdapters, &runner).expect("decided");
    assert_eq!(runner.seen().len(), 1);

    for (at, (synced, claimed)) in [
        (
            "the repository root, which holds the run's `.upstroke`",
            false,
        ),
        (
            "`.upstroke`, which holds another caller's `proposals`",
            false,
        ),
        (
            "`.upstroke/proposals`, which holds the run's directory",
            true,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let fixture = one_question(&format!("decide-synced-{at}"));
        let parent = fixture.proposals_parent();
        let target = match at {
            0 => fixture.repo.clone(),
            1 => fixture.repo.join(".upstroke"),
            _ => parent.clone(),
        };
        let runner = Acting::priced(0.25);
        let message = {
            let _fault = crate::util::fail_barriers_at(&target);
            decide(&fixture.options(), &BuiltinAdapters, &runner)
                .map(|decided| decided.report)
                .expect_err(synced)
                .to_string()
        };
        assert!(runner.seen().is_empty(), "{synced}: no agent was asked");
        assert!(
            message.contains("could not be synced") && message.contains("no agent was asked"),
            "{synced}: {message}"
        );
        assert_eq!(
            proposals_dir(&fixture.repo, RUN).exists(),
            claimed,
            "{synced}: {message}"
        );
        if claimed {
            assert!(
                message.contains(&format!("run `{RUN}` is claimed")),
                "{message}"
            );
            refused_as_claimed(&fixture, "a claim whose parent could not be synced");
        } else {
            assert!(message.contains("the run was not claimed"), "{message}");
            let runner = Acting::priced(0.25);
            decide(&fixture.options(), &BuiltinAdapters, &runner)
                .expect("once the parent syncs, the run is claimed");
            assert_eq!(runner.seen().len(), 1, "{synced}");
        }
    }
}

/// The value the request's command gives `flag`, if it carries it with one.
fn flag_value(request: &RunnerRequest, flag: &str) -> Option<String> {
    let args = &request.command.args;
    let at = args.iter().position(|arg| arg == flag)?;
    args.get(at + 1).cloned()
}

/// Asserts that `decide` refused before writing a proposal or making a request, and returns
/// the refusal's words.
fn refused_before_anything(
    fixture: &Fixture,
    options: &DecideOptions,
    adapters: &dyn AdapterSource,
    case: &str,
) -> String {
    let runner = Scripted::answering(vec![claude_output(&reply(VALID), Some(0.25))]);
    let message = decide(options, adapters, &runner)
        .map(|decided| decided.report)
        .expect_err(case)
        .to_string();
    assert!(runner.seen().is_empty(), "{case}: no agent was asked");
    assert!(
        !fixture.proposals_written(),
        "{case}: nothing was written under the proposals directory"
    );
    message
}

#[test]
fn each_built_in_adapter_is_admitted_only_when_it_keeps_every_guarantee() {
    assert_eq!(
        crate::agent::ADAPTERS.len(),
        3,
        "one case per built-in adapter"
    );
    for adapter in crate::agent::ADAPTERS {
        let id = adapter.id();
        let fixture = fixture(
            &format!("decide-adapter-{id}"),
            1,
            Some(PassBinding::new(id, "a-model")),
            |log| log.raise("q-01A", 0),
        );
        if id == crate::agent::claude::ADAPTER_ID {
            assert_eq!(guarantees_of(id), Guarantee::ALL);
            let runner = Scripted::answering(vec![claude_output(&reply(VALID), Some(0.25))]);
            let decided = decide(&fixture.options(), &BuiltinAdapters, &runner)
                .expect("Claude Code is admitted");
            assert!(decided.outcome().is_ok(), "{decided:?}");
            assert_eq!(runner.seen().len(), 1, "{id}: one question, one call");
        } else {
            assert!(guarantees_of(id).is_empty(), "{id} claims nothing");
            let message =
                refused_before_anything(&fixture, &fixture.options(), &BuiltinAdapters, id);
            assert!(
                message.contains(&format!(
                    "primary reviewer `{id}` cannot guarantee cost reporting, a per-call dollar \
                     ceiling, no web access or the run's private data being unreadable"
                )),
                "{id}: {message}"
            );
            assert!(message.contains(NOTHING_DONE), "{id}: {message}");
        }
    }
}

#[test]
fn a_container_run_is_refused_before_anything_is_written_or_asked() {
    let setup = Setup {
        runner: container_runner(),
        ..Setup::host(reviewer())
    };
    let fixture = fixture_with("decide-container", 1, setup, |log| log.raise("q-01A", 0));
    let message =
        refused_before_anything(&fixture, &fixture.options(), &BuiltinAdapters, "container");
    assert!(
        message.contains("was recorded on the container runner"),
        "{message}"
    );
    assert!(message.contains(NOTHING_DONE), "{message}");
}

/// A coordinator environment in which a Claude Code token is set.
fn token_set(key: &str) -> Option<OsString> {
    (key == "CLAUDE_CODE_OAUTH_TOKEN").then(|| OsString::from("a-token"))
}

#[test]
fn a_run_the_credential_preflight_refuses_is_refused_before_anything_is_written_or_asked() {
    let fixture = one_question("decide-credentials");
    let claude = crate::agent::claude::ADAPTER_ID;
    let profile = fixture.profile.to_string_lossy().into_owned();
    let absent = fixture
        .profile
        .with_file_name("no-such-profile")
        .to_string_lossy()
        .into_owned();
    let environment: fn(&str) -> Option<OsString> = no_variables;
    let cases = [
        (
            "absent",
            pools(claude, &absent),
            environment,
            "which does not exist",
        ),
        (
            "relative",
            pools(claude, "profiles/claude-max"),
            environment,
            "which is a relative path",
        ),
        (
            "codex",
            pools(crate::agent::codex::ADAPTER_ID, &profile),
            environment,
            "supports Claude Code's profile only",
        ),
        (
            "token",
            pools(claude, &profile),
            token_set,
            "`CLAUDE_CODE_OAUTH_TOKEN` is set",
        ),
    ];
    for (case, text, environment, needle) in cases {
        let path = fixture.repo.join(format!("pools-{case}.toml"));
        std::fs::write(&path, text).expect("a pools file");
        let options = DecideOptions {
            pools_path: Some(path),
            environment: Some(environment),
            ..fixture.options()
        };
        let message = refused_before_anything(&fixture, &options, &BuiltinAdapters, case);
        assert!(
            message.contains("pre-flight:") && message.contains(needle),
            "{case}: {message}"
        );
    }
    let runner = Scripted::answering(vec![claude_output(&reply(VALID), Some(0.25))]);
    decide(&fixture.options(), &BuiltinAdapters, &runner).expect("the usable profile admits");
    assert_eq!(runner.seen().len(), 1);
}

#[test]
fn a_cap_that_is_not_a_positive_number_of_dollars_is_refused_before_anything() {
    let fixture = one_question("decide-bad-cap");
    for cap in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let options = DecideOptions {
            max_cost_usd: cap,
            ..fixture.options()
        };
        let message = refused_before_anything(&fixture, &options, &BuiltinAdapters, "a bad cap");
        assert!(
            message.contains("is not a positive number of dollars"),
            "{cap}: {message}"
        );
    }
}

#[test]
fn a_private_directory_that_cannot_be_spelled_in_a_rule_is_refused_before_anything() {
    let setup = Setup {
        private_root: "private[old]",
        ..Setup::host(reviewer())
    };
    let fixture = fixture_with("decide-unspellable", 1, setup, |log| log.raise("q-01A", 0));
    let message = refused_before_anything(
        &fixture,
        &fixture.options(),
        &BuiltinAdapters,
        "unspellable",
    );
    assert!(
        message.contains("cannot be written into a permission rule exactly"),
        "{message}"
    );
}

#[test]
fn a_private_directory_is_spelled_as_an_absolute_rule_or_not_at_all() {
    let root = std::env::temp_dir();
    assert!(rule_path(&root.join("runs").join("01X")).is_some());
    assert_eq!(rule_path(Path::new("relative/runs/01X")), None);
    assert_eq!(rule_path(&root.join("..").join("01X")), None);
    for name in ["a*", "a?", "a[1]", "a{b}", "a(b)"] {
        assert_eq!(rule_path(&root.join(name)), None, "{name}");
    }
    #[cfg(unix)]
    assert_eq!(
        rule_path(Path::new("/var/lib/Upstroke/private runs/01X")).as_deref(),
        Some("//var/lib/Upstroke/private runs/01X")
    );
    #[cfg(windows)]
    assert_eq!(
        rule_path(Path::new(r"C:\Users\dev\.upstroke\runs\01X")).as_deref(),
        Some("//c/Users/dev/.upstroke/runs/01X")
    );
    let denials = private_denials(&root.join("runs").join("01X").to_string_lossy())
        .expect("an absolute directory");
    let spelled = rule_path(&root.join("runs").join("01X")).expect("spellable");
    assert_eq!(
        denials.get(..2),
        Some([format!("Read({spelled})"), format!("Read({spelled}/**)")].as_slice()),
        "the directory and everything under it, as recorded first"
    );
}

#[test]
fn each_calls_ceiling_is_what_is_left_of_the_cap() {
    let fixture = fixture("decide-ceiling", 2, reviewer(), |log| {
        log.raise("q-b", 0);
        log.raise("q-a", 1);
    });
    let runner = Scripted::answering(vec![
        claude_output(&reply(VALID), Some(1.25)),
        claude_output(&reply(VALID), Some(1.0)),
    ]);
    decide(&fixture.options(), &BuiltinAdapters, &runner).expect("decided");
    let ceilings: Vec<Option<String>> = runner
        .seen()
        .iter()
        .map(|request| flag_value(request, "--max-budget-usd"))
        .collect();
    assert_eq!(
        ceilings,
        [Some("5".to_owned()), Some("3.75".to_owned())],
        "$5 for the first call, and the $3.75 the first left for the second"
    );
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A process that exited 0 having printed `stdout`.
fn exited(stdout: String) -> ProcessOutput {
    ProcessOutput {
        code: Some(0),
        stdout,
        stderr: String::new(),
        duration: Duration::ZERO,
        timed_out: false,
        output_limited: false,
    }
}

type Act = Box<dyn Fn(&RunnerRequest) -> Result<ProcessOutput, RunnerError> + Send + Sync>;

/// How an inert Runner answers a request: `None` when the request is not a pre-flight probe.
type Probe = fn(&RunnerRequest) -> Option<Result<ProcessOutput, RunnerError>>;

/// An installed Claude Code, signed in.
fn signed_in(request: &RunnerRequest) -> Option<Result<ProcessOutput, RunnerError>> {
    probe_reply(request, SIGNED_IN).map(Ok)
}

/// An installed Claude Code, signed out.
fn signed_out(request: &RunnerRequest) -> Option<Result<ProcessOutput, RunnerError>> {
    probe_reply(request, SIGNED_OUT).map(Ok)
}

/// No Claude Code installed: no probe's process could be started.
fn not_installed(request: &RunnerRequest) -> Option<Result<ProcessOutput, RunnerError>> {
    probe_reply(request, SIGNED_IN).map(|_| {
        Err(RunnerError::never_started(
            &request.invocation,
            UpstrokeError::Refused {
                message: "no `claude` program was found on PATH".to_owned(),
            },
        ))
    })
}

/// An inert Runner that records each request and answers it with what `act` does while the
/// call is in flight: no process is started. It answers the pre-flight's probes as `probe` says,
/// apart from `act` (by default as a signed-in Claude Code), and records them apart.
struct Acting {
    act: Act,
    probe: Probe,
    seen: Mutex<Vec<RunnerRequest>>,
    probes: Mutex<Vec<RunnerRequest>>,
}

impl Acting {
    fn new(
        act: impl Fn(&RunnerRequest) -> Result<ProcessOutput, RunnerError> + Send + Sync + 'static,
    ) -> Self {
        Self {
            act: Box::new(act),
            probe: signed_in,
            seen: Mutex::new(Vec::new()),
            probes: Mutex::new(Vec::new()),
        }
    }

    fn priced(cost_usd: f64) -> Self {
        Self::new(move |_| Ok(exited(claude_output(&reply(VALID), Some(cost_usd)))))
    }

    /// The same Runner, answering the pre-flight's probes as `probe` says.
    fn probing(self, probe: Probe) -> Self {
        Self { probe, ..self }
    }

    fn seen(&self) -> Vec<RunnerRequest> {
        lock(&self.seen).clone()
    }

    fn probes(&self) -> Vec<RunnerRequest> {
        lock(&self.probes).clone()
    }
}

impl Runner for Acting {
    fn run<'a>(&'a self, request: &'a RunnerRequest, _call: RunnerCall<'a>) -> RunFuture<'a> {
        if let Some(probed) = (self.probe)(request) {
            lock(&self.probes).push(request.clone());
            return Box::pin(async move { probed });
        }
        lock(&self.seen).push(request.clone());
        let answer = (self.act)(request);
        Box::pin(async move { answer })
    }
}

fn ceilings(seen: &[RunnerRequest]) -> Vec<Option<String>> {
    seen.iter()
        .map(|request| flag_value(request, "--max-budget-usd"))
        .collect()
}

/// A process that printed `stdout` and ended as `code`, `timed_out` and `output_limited` say.
fn ended(
    code: Option<i32>,
    timed_out: bool,
    output_limited: bool,
    stdout: String,
) -> ProcessOutput {
    ProcessOutput {
        code,
        timed_out,
        output_limited,
        ..exited(stdout)
    }
}

/// Claude Code's own adapter, whose reading of every output is given `status`: what an adapter
/// that reads a call as failed returns. It is probed and its sign-in discovered as Claude Code's.
struct Misread(OutcomeStatus);

impl AgentAdapter for Misread {
    fn id(&self) -> &'static str {
        crate::agent::claude::ADAPTER_ID
    }

    fn probe(&self, runner: &dyn Runner) -> Result<Caps, UpstrokeError> {
        crate::agent::claude::ClaudeCodeAdapter.probe(runner)
    }

    fn discover(
        &self,
        runner: &dyn Runner,
        caps: &Caps,
    ) -> Result<crate::agent::Discovery, UpstrokeError> {
        crate::agent::claude::ClaudeCodeAdapter.discover(runner, caps)
    }

    fn build(&self, run: &TaskRun) -> Result<CommandSpec, UpstrokeError> {
        crate::agent::claude::ClaudeCodeAdapter.build(run)
    }

    fn parse(&self, out: &ProcessOutput) -> Result<Outcome, UpstrokeError> {
        crate::agent::claude::ClaudeCodeAdapter
            .parse(out)
            .map(|outcome| Outcome {
                status: self.0,
                ..outcome
            })
    }
}

type Returns = fn(&RunnerRequest) -> Result<ProcessOutput, RunnerError>;

/// Spend and interruption: only a clean, successful, priced call is settled, at its price. Every
/// other outcome a call returns is saved as refused with what it reported, counts nothing, and
/// ends the command, which names the questions it left and fails.
#[test]
fn only_a_clean_successful_priced_call_settles_and_any_other_ends_the_command() {
    let priced = || claude_output(&reply(VALID), Some(0.25));
    // The controls, the same two questions: a clean, priced call settles at its price whatever
    // its reply, and the next question is asked with what is left.
    for (case, stdout, status) in [
        ("an accepted reply", priced(), "accepted"),
        (
            "a refused reply",
            claude_output(
                &reply(&VALID.replace("\"choice\": 1", "\"choice\": 9")),
                Some(0.25),
            ),
            "refused",
        ),
    ] {
        let fixture = two_questions(&format!("decide-settled-{status}"));
        let runner = Scripted::answering(vec![stdout, priced()]);
        let decided = decide(&fixture.options(), &BuiltinAdapters, &runner).expect("decided");
        assert_eq!(
            ceilings(&runner.seen()),
            [Some("5".to_owned()), Some("4.75".to_owned())],
            "{case}: settled at its price"
        );
        let saved = fixture.proposal("q-b");
        assert_eq!(saved["status"], status, "{case}");
        assert_eq!(saved["settled"], true, "{case}");
        assert_eq!(fixture.proposal("q-a")["status"], "accepted", "{case}");
        assert!(decided.left.is_empty(), "{case}: {}", decided.report);
        assert!(decided.report.contains("total cost: $0.5000"), "{case}");
    }

    // The one case read through an adapter that reads every call as an agent error.
    const UNSUCCESSFUL: &str = "an unsuccessful outcome";
    let cases: [(&str, Returns, &str, Option<f64>); 8] = [
        (
            "no cost reported",
            |_| Ok(exited(claude_output(&reply(VALID), None))),
            "the call reported no cost",
            None,
        ),
        (
            "a cost that is no price",
            |_| Ok(exited(claude_output(&reply(VALID), Some(-1.0)))),
            "reported a cost of -1, which is no price",
            Some(-1.0),
        ),
        (
            "a nonzero exit",
            |_| {
                Ok(ended(
                    Some(1),
                    false,
                    false,
                    claude_output(&reply(VALID), Some(0.25)),
                ))
            },
            "the agent exited with status 1",
            Some(0.25),
        ),
        (
            "a signal",
            |_| {
                Ok(ended(
                    None,
                    false,
                    false,
                    claude_output(&reply(VALID), Some(0.25)),
                ))
            },
            "the agent was ended by a signal",
            Some(0.25),
        ),
        (
            "a timeout",
            |_| {
                Ok(ended(
                    Some(0),
                    true,
                    false,
                    claude_output(&reply(VALID), Some(0.25)),
                ))
            },
            "the call timed out",
            Some(0.25),
        ),
        (
            "an output limit",
            |_| {
                Ok(ended(
                    Some(0),
                    false,
                    true,
                    claude_output(&reply(VALID), Some(0.25)),
                ))
            },
            "the call's output exceeded its limit",
            Some(0.25),
        ),
        (
            "a failed process",
            |request| {
                Err(RunnerError::cancelled(
                    &request.invocation,
                    ProcessFate::Gone,
                ))
            },
            "the agent process failed",
            None,
        ),
        (
            UNSUCCESSFUL,
            |_| Ok(exited(claude_output(&reply(VALID), Some(0.25)))),
            "the adapter read the call as an agent error",
            Some(0.25),
        ),
    ];
    let misread = Misread(OutcomeStatus::AgentError);
    for (at, (case, returns, reason, cost)) in cases.into_iter().enumerate() {
        let fixture = two_questions(&format!("decide-unsettled-{at}"));
        let runner = Acting::new(returns);
        let decided = if case == UNSUCCESSFUL {
            decide(&fixture.options(), &Only(&misread), &runner)
        } else {
            decide(&fixture.options(), &BuiltinAdapters, &runner)
        }
        .expect("decided");
        assert_eq!(runner.seen().len(), 1, "{case}: the command ended");
        let saved = fixture.proposal("q-b");
        assert_eq!(saved["status"], "refused", "{case}: {saved}");
        assert_eq!(saved["settled"], false, "{case}: {saved}");
        assert_eq!(
            saved["cost_usd"],
            json!(cost),
            "{case}: what it reported is kept"
        );
        assert!(
            saved["reason"]
                .as_str()
                .is_some_and(|text| text.contains(reason)),
            "{case}: {saved}"
        );
        assert!(
            proposals_dir(&fixture.repo, RUN).join("q-b.out").is_file(),
            "{case}: the raw output, or the failure, is saved"
        );
        assert!(!fixture.has_proposal("q-a"), "{case}");
        let report = &decided.report;
        for line in [
            "stopped: q-b's call did not settle at a price (".to_owned(),
            "so what it spent is unknown and no further call is started; left without a \
             proposal: q-a"
                .to_owned(),
            "total cost: $0.0000".to_owned(),
            "cost unknown for: q-b".to_owned(),
            format!("claimed: {}", proposals_dir(&fixture.repo, RUN).display()),
        ] {
            assert!(report.contains(&line), "{case}: {line}: {report}");
        }
        assert_eq!(decided.refused, ["q-b"], "{case}");
        assert_eq!(decided.left, ["q-a"], "{case}");
        let error = decided.outcome().expect_err("nonzero").to_string();
        assert!(
            error.contains("(q-b)") && error.contains("left without a proposal (q-a)"),
            "{case}: {error}"
        );
        refused_as_claimed(&fixture, case);
    }
}

/// Publication: every file is created new and none is replaced. A name another hand took while
/// the call was in flight ends the command with that file as it was, no proposal saved, and the
/// question named as left.
#[test]
fn a_taken_name_is_never_replaced_and_ends_the_command_naming_the_questions_left() {
    let fixture = two_questions("decide-taken");
    let dir = proposals_dir(&fixture.repo, RUN);
    let runner = Acting::new({
        let dir = dir.clone();
        move |_| {
            std::fs::write(dir.join("q-b.out"), "someone else's bytes").expect("a stray file");
            Ok(exited(claude_output(&reply(VALID), Some(0.25))))
        }
    });
    let decided = decide(&fixture.options(), &BuiltinAdapters, &runner).expect("decided");
    assert_eq!(runner.seen().len(), 1, "{}", decided.report);
    assert_eq!(
        std::fs::read_to_string(dir.join("q-b.out")).expect("the stray file"),
        "someone else's bytes"
    );
    assert!(!fixture.has_proposal("q-b") && !fixture.has_proposal("q-a"));
    assert!(
        decided.report.contains("q-b.out")
            && decided
                .report
                .contains("the run stays claimed and no further call is started; left without a proposal: q-b, q-a"),
        "{}",
        decided.report
    );
    assert_eq!(decided.left, ["q-b", "q-a"]);
    assert!(decided.outcome().is_err());

    // The control: the same two questions, nothing in the way.
    let control = two_questions("decide-untaken");
    let runner = Acting::priced(0.25);
    let decided = decide(&control.options(), &BuiltinAdapters, &runner).expect("decided");
    assert_eq!(runner.seen().len(), 2);
    assert!(decided.outcome().is_ok(), "{}", decided.report);
}

#[test]
fn an_untrusted_proposals_directory_or_a_question_id_that_is_not_plain_is_refused() {
    for id in ["q-01A", "Q_1", "a"] {
        assert!(plain_id(id), "{id}");
    }
    let long = "a".repeat(MAX_ID_BYTES + 1);
    for id in [
        "", ".", "..", "a.b", "a/b", r"a\b", "a:b", "a b", "CON", "nul", "com1", "LPT9", &long,
    ] {
        assert!(!plain_id(id), "{id}");
    }

    let dotted = fixture("decide-dotted", 1, reviewer(), |log| log.raise("q.01A", 0));
    let runner = Acting::priced(0.25);
    let decided = decide(&dotted.options(), &BuiltinAdapters, &runner).expect("decided");
    assert!(runner.seen().is_empty());
    assert!(
        decided
            .report
            .contains("\"q.01A\": refused — its id is not a plain file-name component"),
        "{}",
        decided.report
    );
    assert!(!dotted.proposals_written());

    let filed = one_question("decide-filed");
    let proposals = filed.repo.join(".upstroke").join("proposals");
    std::fs::write(&proposals, "not a directory").expect("a file at the name");
    let message = refused_before_anything(&filed, &filed.options(), &BuiltinAdapters, "a file");
    assert!(
        message.contains("is not one decide can trust") && message.contains("is not a directory"),
        "{message}"
    );
    assert!(message.contains("the run was not claimed"), "{message}");

    #[cfg(unix)]
    {
        let linked = one_question("decide-linked");
        let elsewhere = linked.repo.join("elsewhere");
        std::fs::create_dir_all(&elsewhere).expect("a directory elsewhere");
        set_mode(&elsewhere, 0o700);
        std::os::unix::fs::symlink(&elsewhere, linked.repo.join(".upstroke").join("proposals"))
            .expect("a link");
        let message =
            refused_before_anything(&linked, &linked.options(), &BuiltinAdapters, "a link");
        assert!(message.contains("is not a directory"), "{message}");
        assert!(
            snapshot(&elsewhere).is_empty(),
            "nothing was written through the link"
        );
    }
}

/// Directory: on Unix every directory of the chain belongs to the repository's owner, no other
/// user may write any of them, and the two decide creates are open to no other user at all.
/// Each mode a run or a person could leave is checked against the same fixture.
#[cfg(unix)]
#[test]
fn the_chain_is_the_owners_and_closed_to_every_other_user() {
    let proposals = |fixture: &Fixture| fixture.repo.join(".upstroke").join("proposals");
    let upstroke = |fixture: &Fixture| fixture.repo.join(".upstroke");
    type Chosen = fn(&Fixture) -> PathBuf;
    let cases: [(Chosen, u32, Option<&str>); 7] = [
        (proposals, 0o700, None),
        (
            proposals,
            0o777,
            Some("open to a user other than its owner (mode 777)"),
        ),
        (
            proposals,
            0o770,
            Some("open to a user other than its owner (mode 770)"),
        ),
        (
            proposals,
            0o755,
            Some("open to a user other than its owner (mode 755)"),
        ),
        (upstroke, 0o755, None),
        (
            upstroke,
            0o775,
            Some("writable by a user other than its owner (mode 775)"),
        ),
        (
            upstroke,
            0o757,
            Some("writable by a user other than its owner (mode 757)"),
        ),
    ];
    for (at, (chosen, mode, refusal)) in cases.into_iter().enumerate() {
        let fixture = one_question(&format!("decide-mode-{at}"));
        let dir = chosen(&fixture);
        std::fs::create_dir_all(&dir).expect("the directory");
        set_mode(&dir, mode);
        let case = format!("{} at {mode:03o}", dir.display());
        match refusal {
            None => {
                let runner = Acting::priced(0.25);
                decide(&fixture.options(), &BuiltinAdapters, &runner)
                    .unwrap_or_else(|error| panic!("{case}: {error}"));
                assert_eq!(runner.seen().len(), 1, "{case}");
            }
            Some(needle) => {
                let message =
                    refused_before_anything(&fixture, &fixture.options(), &BuiltinAdapters, &case);
                assert!(
                    message.contains("is not one decide can trust") && message.contains(needle),
                    "{case}: {message}"
                );
            }
        }
    }
}

#[test]
fn a_decide_call_adds_only_its_own_flags_to_an_ordinary_reviewers_command() {
    let fixture = one_question("decide-ordinary");
    let runner = Scripted::answering(vec![claude_output(&reply(VALID), Some(0.25))]);
    decide(&fixture.options(), &BuiltinAdapters, &runner).expect("decided");
    let seen = runner.seen();
    let [request] = seen.as_slice() else {
        panic!("one request: {seen:?}");
    };

    let reviewer = crate::review::profile_for("claude-code", "opus", "decide-opus", Effort::Medium);
    assert!(
        reviewer.extra_args.is_empty(),
        "an ordinary reviewer's profile carries no decide flag"
    );
    let ordinary = TaskRun {
        prompt: String::new(),
        profile: reviewer.clone(),
        workspace: fixture.repo.clone(),
        gate_cmds: Vec::new(),
        resume_session: None,
        settings_path: request.command.settings_file.clone(),
    };
    let mut expected = crate::agent::claude::build_args(&ordinary);
    assert!(
        !expected
            .iter()
            .any(|arg| arg == "--max-budget-usd" || arg == "--strict-mcp-config"),
        "an ordinary reviewer is built with neither: {expected:?}"
    );
    expected.extend(call_args(DEFAULT_MAX_COST_USD));
    assert_eq!(
        request.command.args, expected,
        "the ordinary reviewer's arguments, then the decide call's own"
    );
    assert_eq!(
        call_settings(&reviewer, &[]),
        crate::agent::claude::permission_settings(&reviewer, &[]),
        "a decide call's settings add deny rules only; the reviewer's are its own"
    );
}

/// A mocked adapter: Claude Code's own probe, sign-in discovery, `build` and `parse` under the id
/// `id`, with each built command passed through `doctor`.
struct Doctored {
    id: &'static str,
    doctor: fn(CommandSpec) -> CommandSpec,
}

impl AgentAdapter for Doctored {
    fn id(&self) -> &'static str {
        self.id
    }

    fn probe(&self, runner: &dyn Runner) -> Result<Caps, UpstrokeError> {
        crate::agent::claude::ClaudeCodeAdapter.probe(runner)
    }

    fn discover(
        &self,
        runner: &dyn Runner,
        caps: &Caps,
    ) -> Result<crate::agent::Discovery, UpstrokeError> {
        crate::agent::claude::ClaudeCodeAdapter.discover(runner, caps)
    }

    fn build(&self, run: &TaskRun) -> Result<CommandSpec, UpstrokeError> {
        crate::agent::claude::ClaudeCodeAdapter
            .build(run)
            .map(self.doctor)
    }

    fn parse(&self, out: &ProcessOutput) -> Result<Outcome, UpstrokeError> {
        crate::agent::claude::ClaudeCodeAdapter.parse(out)
    }
}

/// An adapter source that answers every id with one adapter.
struct Only<'a>(&'a dyn AgentAdapter);

impl AdapterSource for Only<'_> {
    fn get(&self, _id: &str) -> Option<&dyn AgentAdapter> {
        Some(self.0)
    }
}

fn set_value(mut command: CommandSpec, flag: &str, value: &str) -> CommandSpec {
    if let Some(at) = command.args.iter().position(|arg| arg == flag) {
        if let Some(slot) = command.args.get_mut(at + 1) {
            value.clone_into(slot);
        }
    }
    command
}

fn drop_flag(mut command: CommandSpec, flag: &str, values: usize) -> CommandSpec {
    if let Some(at) = command.args.iter().position(|arg| arg == flag) {
        let end = (at + 1 + values).min(command.args.len());
        command.args.drain(at..end);
    }
    command
}

#[test]
fn a_mocked_adapter_whose_command_breaks_a_promise_is_refused_before_anything() {
    type Doctor = fn(CommandSpec) -> CommandSpec;
    let fixture = one_question("decide-mocked");
    let cases: [(&str, Doctor, &str); 15] = [
        (
            "another model",
            |command| set_value(command, "--model", "sonnet"),
            "is not the recorded reviewer's, claude-code/opus at medium effort: it carries \
             `--model \"sonnet\"`, not `--model \"opus\"`",
        ),
        (
            "another effort",
            |command| set_value(command, "--effort", "max"),
            "is not the recorded reviewer's, claude-code/opus at medium effort: it carries \
             `--effort \"max\"`, not `--effort \"medium\"`",
        ),
        (
            "no effort",
            |command| drop_flag(command, "--effort", 1),
            "is not the recorded reviewer's, claude-code/opus at medium effort: it does not \
             carry `--effort`",
        ),
        (
            "no ceiling",
            |command| drop_flag(command, "--max-budget-usd", 1),
            "does not keep a per-call dollar ceiling",
        ),
        (
            "a ceiling above what is left",
            |command| set_value(command, "--max-budget-usd", "5.01"),
            "does not keep a per-call dollar ceiling",
        ),
        (
            "text output",
            |command| set_value(command, "--output-format", "text"),
            "does not keep cost reporting",
        ),
        (
            "permissions bypassed",
            |command| set_value(command, "--permission-mode", "bypassPermissions"),
            "does not keep no web access",
        ),
        (
            "user settings loaded",
            |command| set_value(command, "--setting-sources", "user"),
            "does not keep no web access",
        ),
        (
            "every MCP server",
            |command| drop_flag(command, "--strict-mcp-config", 0),
            "does not keep no web access",
        ),
        (
            "another settings file",
            |command| set_value(command, "--settings", "elsewhere.json"),
            "does not name the settings written for it",
        ),
        (
            "a web tool allowed",
            |command| command.arg("--allowedTools").arg("WebFetch"),
            "carries `--allowedTools`, which no decide call is built with",
        ),
        (
            "a second permission mode",
            |command| command.arg("--permission-mode").arg("bypassPermissions"),
            "carries `--permission-mode` twice",
        ),
        (
            "a flag with no value",
            |command| command.arg("--max-turns"),
            "ends with `--max-turns` and no value",
        ),
        (
            "another program",
            |mut command| {
                "sh".clone_into(&mut command.program);
                command
            },
            "starts `sh`, not Claude Code's CLI",
        ),
        (
            "an environment overlay",
            |command| command.env("ANTHROPIC_BASE_URL", "http://127.0.0.1:9"),
            "carries an environment overlay (ANTHROPIC_BASE_URL)",
        ),
    ];
    for (case, doctor, needle) in cases {
        let adapter = Doctored {
            id: crate::agent::claude::ADAPTER_ID,
            doctor,
        };
        let message = refused_before_anything(&fixture, &fixture.options(), &Only(&adapter), case);
        assert!(
            message.contains("the command built for `claude-code`") && message.contains(needle),
            "{case}: {message}"
        );
        assert!(
            message.contains("so it was not started"),
            "{case}: {message}"
        );
    }

    let unclaimed = Doctored {
        id: "mock-agent",
        doctor: |command| command,
    };
    let message = refused_before_anything(
        &fixture,
        &fixture.options(),
        &Only(&unclaimed),
        "an adapter that claims nothing",
    );
    assert!(
        message.contains("`mock-agent` cannot guarantee cost reporting"),
        "{message}"
    );

    let faithful = Doctored {
        id: crate::agent::claude::ADAPTER_ID,
        doctor: |command| command,
    };
    let runner = Scripted::answering(vec![claude_output(&reply(VALID), Some(0.25))]);
    decide(&fixture.options(), &Only(&faithful), &runner)
        .expect("the undoctored command keeps every promise");
    assert_eq!(runner.seen().len(), 1, "and only that one is started");
}

#[test]
fn a_driven_run_is_refused_before_anything_is_written_probed_or_asked() {
    let fixture = one_question("decide-driven");
    let held = crate::rundir::RunLock::acquire(&fixture.public()).expect("an engine's run lock");
    let runner = Acting::priced(0.25);
    let message = decide(&fixture.options(), &BuiltinAdapters, &runner)
        .expect_err("a driven run")
        .to_string();
    assert!(
        message.contains(&format!("run `{RUN}` is being driven")),
        "{message}"
    );
    assert!(message.contains(NOTHING_DONE), "{message}");
    assert!(runner.seen().is_empty() && runner.probes().is_empty());
    assert!(!fixture.proposals_written());

    // The control: the same run, once nothing drives it, is asked.
    drop(held);
    let decided = decide(&fixture.options(), &BuiltinAdapters, &runner).expect("decided");
    assert!(decided.outcome().is_ok(), "{}", decided.report);
    assert_eq!(runner.seen().len(), 1, "{}", decided.report);
}

/// Publishes a person's answer to `id` in the run's answers directory, as `upstroke answer` does.
fn answered_by_a_person(fixture: &Fixture, id: &str, answer: Answer) {
    let answers = fixture.public().join("answers");
    std::fs::create_dir_all(&answers).expect("the answers directory");
    let published = crate::interaction::write_answer(
        &answers,
        &QuestionId::from(id),
        &crate::interaction::AnswerRecord::unattributed(answer),
    )
    .expect("the answer is published");
    assert_eq!(published, crate::interaction::AnswerPublication::Published);
}

#[test]
fn a_question_a_person_has_answered_is_skipped_and_never_asked() {
    let fixture = two_questions("decide-answered");
    answered_by_a_person(
        &fixture,
        "q-b",
        Answer::Answered {
            text: RETRY.to_owned(),
        },
    );
    let runner = Acting::priced(0.25);
    let decided = decide(&fixture.options(), &BuiltinAdapters, &runner).expect("decided");
    assert!(decided.outcome().is_ok(), "{}", decided.report);
    assert!(
        decided.report.contains(
            "q-b: skipped — a person has answered it already (\"retry on frontier\"), and the \
             run applies that answer; no agent was asked"
        ),
        "{}",
        decided.report
    );
    let seen = runner.seen();
    let [request] = seen.as_slice() else {
        panic!("only the unanswered question is asked: {seen:?}");
    };
    let prompt = String::from_utf8_lossy(&request.command.stdin);
    assert!(prompt.contains("id: q-a\n"), "{prompt}");
    assert!(!fixture.has_proposal("q-b") && fixture.has_proposal("q-a"));

    // Every question answered: nothing to ask, so nothing is probed, claimed or asked.
    let fixture = one_question("decide-all-answered");
    answered_by_a_person(&fixture, "q-01A", Answer::Declined);
    let runner = Acting::priced(0.25);
    let decided = decide(&fixture.options(), &BuiltinAdapters, &runner).expect("decided");
    assert!(decided.outcome().is_ok(), "{}", decided.report);
    assert!(
        decided
            .report
            .contains("q-01A: skipped — a person has answered it already (declined)"),
        "{}",
        decided.report
    );
    assert!(runner.seen().is_empty() && runner.probes().is_empty());
    assert!(!fixture.proposals_written(), "the run is not claimed");
}

#[test]
fn the_recorded_reviewer_passes_the_runs_own_preflight_before_anything_is_written_or_asked() {
    // Admitted: its CLI probed and the pools' one account found signed in, each probe under the
    // run's own probe identities, and only then the call.
    let fixture = one_question("decide-preflight-admitted");
    let runner = Acting::priced(0.25);
    let decided = decide(&fixture.options(), &BuiltinAdapters, &runner).expect("decided");
    assert!(decided.outcome().is_ok(), "{}", decided.report);
    let probes: Vec<String> = runner
        .probes()
        .iter()
        .map(|probe| probe.invocation.to_string())
        .collect();
    let claude = crate::agent::claude::ADAPTER_ID;
    let expected: Vec<String> = [0, 1, 2]
        .into_iter()
        .map(|ordinal| {
            InvocationId::probe(
                crate::runner::ProbeTarget::Agent(AgentId::new(claude)),
                ordinal,
            )
            .expect("a probe identity")
            .to_string()
        })
        .collect();
    assert_eq!(probes, expected, "--version, --help, auth status");
    let auth = runner.probes().pop().expect("the sign-in discovery");
    assert_eq!(
        auth.credential_profile,
        Some(fixture.profile.clone()),
        "the account the pools file configures for the reviewer's agent"
    );
    assert_eq!(runner.seen().len(), 1);

    // Refused: signed out, or no CLI to probe. Nothing is written and no agent asked.
    for (case, probe, needle) in [
        (
            "signed out",
            signed_out as Probe,
            format!(
                "agent `{claude}` is not signed in under pool `claude-max`'s account, the \
                 credential profile directory `"
            ),
        ),
        (
            "not installed",
            not_installed,
            format!(
                "run `{RUN}`'s recorded reviewer did not pass the pre-flight a run makes of it: \
                 the `{claude}` CLI:"
            ),
        ),
    ] {
        let fixture = one_question(&format!("decide-preflight-{}", case.replace(' ', "-")));
        let runner = Acting::priced(0.25).probing(probe);
        let message = decide(&fixture.options(), &BuiltinAdapters, &runner)
            .expect_err(case)
            .to_string();
        assert!(message.contains(&needle), "{case}: {message}");
        assert!(
            case != "signed out" || message.contains(&format!("{}`", fixture.profile.display())),
            "{case}: the account checked is the one the pools file configures: {message}"
        );
        assert!(!runner.probes().is_empty(), "{case}: the CLI was probed");
        assert!(runner.seen().is_empty(), "{case}: no agent was asked");
        assert!(!fixture.proposals_written(), "{case}: nothing was written");
    }
}

#[test]
fn the_plan_is_forwarded_only_when_its_bytes_are_the_plan_the_run_recorded() {
    let prompt_of = |fixture: &Fixture| {
        let runner = Scripted::answering(vec![claude_output(&reply(VALID), Some(0.25))]);
        decide(&fixture.options(), &BuiltinAdapters, &runner).expect("decided");
        let seen = runner.seen();
        let [request] = seen.as_slice() else {
            panic!("one request: {seen:?}");
        };
        String::from_utf8_lossy(&request.command.stdin).into_owned()
    };

    let fixture = one_question("decide-plan-recorded");
    let prompt = prompt_of(&fixture);
    assert!(prompt.contains(PLAN_TEXT), "{prompt}");
    assert!(!prompt.contains("is not forwarded"), "{prompt}");

    let fixture = one_question("decide-plan-edited");
    let edited = "# The plan\n\nThe widget task makes a gadget instead.\n";
    std::fs::write(fixture.repo.join("docs").join("plan.md"), edited).expect("edit the plan");
    let prompt = prompt_of(&fixture);
    assert!(!prompt.contains("makes a gadget instead"), "{prompt}");
    assert!(!prompt.contains(PLAN_TEXT), "{prompt}");
    assert!(
        prompt.contains(&format!(
            "is not forwarded: its bytes hash to {}, not to the {} the run recorded",
            crate::ir::content_hash(edited.as_bytes()),
            crate::ir::content_hash(PLAN_TEXT.as_bytes())
        )),
        "{prompt}"
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
