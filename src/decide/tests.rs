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

/// A valid record, as a literal, so `concat!` can write whole replies around it as plain text.
macro_rules! valid {
    () => {
        r#"{"choice": 1, "guidance": "retry", "reasons": ["it flaked"], "confidence": "high", "needs_a_person": false, "citations": ["src/lib.rs:1"]}"#
    };
}

const VALID: &str = valid!();

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
            "line 6 of the reply holds ``` after the json block closed at line 4",
        ),
        (VALID.to_owned(), "the reply holds no fenced json block"),
        (
            reply(&with_guidance("skip")),
            "`guidance` gives up on the task",
        ),
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
fn a_fence_mentioned_inside_the_json_block_is_its_text_and_only_a_line_alone_closes_it() {
    let asked = question(&[RETRY, crate::interaction::GIVE_UP_OPTION]);
    for guidance in [
        "close the block with ```",
        "wrap the patch in a ```json fence",
        "```\n```json\n```",
        "or fence it with ~~~",
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
}

/// Replies as a reviewer writes them, in plain text: what each is, the reply, and `None` where it
/// is accepted or the words its refusal holds. Each is read against a question offering a retry
/// and a give-up option.
const FRAMINGS: [(&str, &str, Option<&str>); 26] = [
    (
        "the block alone",
        concat!("```json\n", valid!(), "\n```\n"),
        None,
    ),
    (
        "prose around the block, with no fence in it",
        concat!(
            "Here is my proposal.\n```json\n",
            valid!(),
            "\n```\nA person decides.\n"
        ),
        None,
    ),
    (
        "single and double backticks outside the block are no fence",
        concat!(
            "Run `cargo test`, or ``cargo test -- `x` ``.\n```json\n",
            valid!(),
            "\n```\n"
        ),
        None,
    ),
    (
        "whitespace and carriage returns around each fence",
        concat!("my proposal\r\n  ```json \t\r\n", valid!(), "\r\n```  \r\n"),
        None,
    ),
    (
        "a space between the opening fence and its info string",
        concat!("``` json\n", valid!(), "\n```\n"),
        None,
    ),
    (
        "the closing line ends the reply with no line break after it",
        concat!("```json\n", valid!(), "\n```"),
        None,
    ),
    (
        "backticks and tildes inside the block are its JSON's text",
        concat!(
            "```json\n",
            r#"{"choice": 1, "guidance": "fence it with ``` or ~~~", "reasons": ["```"], "confidence": "high", "needs_a_person": false, "citations": ["~~~"]}"#,
            "\n```\n"
        ),
        None,
    ),
    (
        "inline code of three backticks before the block",
        concat!("```src/lib.rs``` holds it\n```json\n", valid!(), "\n```\n"),
        Some("line 1 of the reply holds ``` before the json block opens"),
    ),
    (
        "three backticks within a line after the block",
        concat!("```json\n", valid!(), "\n```\nsee ```notes``` too\n"),
        Some("line 4 of the reply holds ``` after the json block closed at line 3"),
    ),
    (
        "three tildes within a line before the block",
        concat!("~~~ my proposal ~~~\n```json\n", valid!(), "\n```\n"),
        Some("line 1 of the reply holds ~~~ before the json block opens"),
    ),
    (
        "a line of three tildes after the block",
        concat!("```json\n", valid!(), "\n```\n~~~\n"),
        Some("line 4 of the reply holds ~~~ after the json block closed at line 3"),
    ),
    (
        "a json block fenced with tildes",
        concat!("~~~json\n", valid!(), "\n~~~\n"),
        Some("line 1 of the reply holds ~~~ before the json block opens"),
    ),
    (
        "a second json block",
        concat!(
            "```json\n",
            valid!(),
            "\n```\n```json\n",
            valid!(),
            "\n```\n"
        ),
        Some("line 4 of the reply holds ``` after the json block closed at line 3"),
    ),
    (
        "a closed text block before the proposal",
        concat!("```text\nmy notes\n```\n```json\n", valid!(), "\n```\n"),
        Some("line 1 of the reply holds ``` before the json block opens"),
    ),
    (
        "a block with no info string after the proposal",
        concat!("```json\n", valid!(), "\n```\n```\nsome output\n```\n"),
        Some("line 4 of the reply holds ``` after the json block closed at line 3"),
    ),
    (
        "a block left open after the proposal",
        concat!(
            "```json\n",
            valid!(),
            "\n```\nand then:\n```sh\ncargo test\n"
        ),
        Some("line 5 of the reply holds ``` after the json block closed at line 3"),
    ),
    (
        "a ```json line quoted in an earlier block",
        concat!(
            "an example:\n```text\n```json\n```\n```json\n",
            valid!(),
            "\n```\n"
        ),
        Some("line 2 of the reply holds ``` before the json block opens"),
    ),
    (
        "the whole reply on one line",
        concat!("my proposal: ```json ", valid!(), " ```"),
        Some("line 1 of the reply holds ``` before the json block opens"),
    ),
    (
        "an info string that is not `json`",
        concat!("```JSON\n", valid!(), "\n```\n"),
        Some("line 1 of the reply holds ``` before the json block opens"),
    ),
    (
        "a bare carriage return ends no line",
        concat!("```json\r", valid!(), "\r```\r"),
        Some("line 1 of the reply holds ``` before the json block opens"),
    ),
    (
        "the closing fence with text beside it closes nothing",
        concat!("```json\n", valid!(), "\n``` that is all\n"),
        Some("the json block opened at line 1 is never closed"),
    ),
    (
        "a longer run of backticks closes nothing",
        concat!("```json\n", valid!(), "\n````\n"),
        Some("the json block opened at line 1 is never closed"),
    ),
    (
        "a block never closed",
        concat!("my proposal\n```json\n", valid!(), "\n"),
        Some("the json block opened at line 2 is never closed"),
    ),
    (
        "a ```json line inside the block is its text",
        concat!("```json\n```json\n", valid!(), "\n```\n"),
        Some("the json block does not parse"),
    ),
    (
        "text after the JSON inside the block is its text",
        concat!("```json\n", valid!(), "\nsee ```notes```\n```\n"),
        Some("the json block does not parse"),
    ),
    (
        "no block at all",
        valid!(),
        Some("the reply holds no fenced json block"),
    ),
];

/// Framing: exactly one `json` block, and no line outside it holding three backticks or three
/// tildes, while the block's own closing line is the block's and never outside text.
#[test]
fn a_reply_is_one_json_block_and_no_line_outside_it_holds_a_fence() {
    let asked = question(&[RETRY, crate::interaction::GIVE_UP_OPTION]);
    for (case, text, refusal) in FRAMINGS {
        match (check_reply(&asked, text), refusal) {
            (Ok(record), None) => assert_eq!(record["choice"], 1, "{case}"),
            (Err(reason), Some(needle)) => {
                assert!(reason.contains(needle), "{case}: {reason}\n{text}");
            }
            (verdict, _) => panic!("{case}: {verdict:?}\n{text}"),
        }
    }
}

/// Guidance a person's `upstroke answer --text` would read as declining the task is refused,
/// whatever the choice, while guidance merely mentioning a give-up word is advice.
#[test]
fn guidance_read_as_declining_the_task_is_refused() {
    let offered = question(&[RETRY, crate::interaction::GIVE_UP_OPTION]);
    let open = question(&[]);
    for (guidance, options, declines) in [
        ("skip", &offered, true),
        ("  Decline \n", &offered, true),
        ("FAIL", &offered, true),
        ("abandon", &offered, true),
        ("2", &offered, true),
        ("skip", &open, true),
        ("retry", &offered, false),
        ("1", &offered, false),
        ("skip the flaky fixture and retry", &offered, false),
        ("2", &open, false),
    ] {
        let mut body = with_guidance(guidance);
        if options.options.is_empty() {
            body = body.replace("\"choice\": 1", "\"choice\": null");
        }
        assert_eq!(
            crate::interaction::interpret(options, guidance) == Answer::Declined,
            declines,
            "{guidance:?}: as `upstroke answer --text` reads it"
        );
        match check_reply(options, &reply(&body)) {
            Err(reason) if declines => assert!(
                reason.contains("`guidance` gives up on the task"),
                "{guidance:?}: {reason}"
            ),
            Ok(record) if !declines => assert_eq!(record["guidance"], guidance),
            verdict => panic!("{guidance:?}: {verdict:?}"),
        }
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
        question: HOSTILE.to_owned(),
        refused: true,
    };
    let message = decided.outcome().expect_err("refused").to_string();
    assert!(!message.chars().any(char::is_control), "{message:?}");
    assert!(
        message.contains(&format!("the reply to question {SHOWN} was refused")),
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
    /// The ids of the questions raised, in order.
    raised: Vec<String>,
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

    /// Ends the run parked, as its engine does once the fold says nothing else can run: the
    /// `run_finished` the fold checks against its own derived outcome.
    fn park(&mut self) {
        let parked = self
            .fold
            .open_questions()
            .map_or(0, |open| u32::try_from(open.len()).unwrap_or(u32::MAX));
        self.push(TopologyEventBody::RunFinished {
            data: crate::topology::events::RunFinished4 {
                outcome: crate::events::RunOutcome::Parked,
                halted_at: None,
                merged: 0,
                parked,
            },
        });
    }

    fn raise(&mut self, id: &str, key: u32) {
        self.raised.push(id.to_owned());
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
    /// The question `options` asks about: the first one the run raised.
    question: String,
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
            question_id: self.question.clone(),
            max_cost_usd: DEFAULT_MAX_COST_USD,
            pools_path: Some(self.repo.join("pools.toml")),
            environment: Some(no_variables),
        }
    }

    /// `options`, asking about the question `id`.
    fn asking(&self, id: &str) -> DecideOptions {
        DecideOptions {
            question_id: id.to_owned(),
            ..self.options()
        }
    }

    /// The run's claim directory, in its private directory.
    fn claim(&self) -> PathBuf {
        self.private.join(CLAIM_DIR)
    }

    /// Whether anything stands at the claim's name: whether the run was claimed.
    fn claimed(&self) -> bool {
        std::fs::symlink_metadata(self.claim()).is_ok()
    }

    fn public(&self) -> PathBuf {
        crate::rundir::public_dir(&self.repo, RUN)
    }

    /// The run's one proposal, which must be question `id`'s.
    fn proposal(&self, id: &str) -> Value {
        let path = self.claim().join(PROPOSAL_FILE);
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let saved: Value = serde_json::from_str(&text).expect("a proposal is JSON");
        assert_eq!(saved["question_id"], id, "the run's one proposal: {saved}");
        saved
    }

    /// Whether the run's one proposal is saved and is question `id`'s.
    fn has_proposal(&self, id: &str) -> bool {
        std::fs::read_to_string(self.claim().join(PROPOSAL_FILE))
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
            .is_some_and(|saved| saved["question_id"] == id)
    }

    /// The names of the files in the run's claim, sorted.
    fn claim_files(&self) -> Vec<String> {
        snapshot(&self.claim())
            .keys()
            .filter_map(|path| path.file_name()?.to_str().map(str::to_owned))
            .collect()
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
    // Closed to group and other, as decide requires of it, whatever this process's umask is.
    set_mode(&private, 0o700);
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
    let started = run_started(&plan, &normalized_plan_digest, &private, setup);
    let mut log = Log {
        log,
        fold: crate::topology::fold::TopologyFold::new(crate::topology::fold::FrozenInputs {
            plan,
            normalized_plan_digest,
        }),
        raised: Vec::new(),
    };
    log.push(TopologyEventBody::RunStarted {
        data: Box::new(started),
    });
    write(&mut log);
    // A run whose fold derives it parked is ended parked, as its engine ends it; any other is
    // left with no end, as an engine interrupted mid-run leaves it.
    if log.fold.derived_outcome()
        == crate::topology::events::DerivedOutcome::Ending(crate::events::RunOutcome::Parked)
    {
        log.park();
    }
    let question = log
        .raised
        .first()
        .cloned()
        .unwrap_or_else(|| "q-01A".to_owned());
    drop(log);
    // The run's lock files, as the engine that drove it leaves them: taken once and given back.
    drop(crate::rundir::RunLock::acquire(&public).expect("the run's engine took its lock"));
    Fixture {
        _tree: tree,
        repo,
        private,
        profile,
        question,
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
    assert_eq!(
        saved["raw_output"], RAW_FILE,
        "the raw output is named by its fixed name, beside the proposal"
    );
    assert_eq!(
        std::fs::read_to_string(fixture.claim().join(RAW_FILE)).expect("the raw output"),
        claude_output(&reply(VALID), Some(0.25))
    );
    assert!(report.contains(&format!(
        "saved: {}",
        fixture.claim().join(PROPOSAL_FILE).display()
    )));
    assert!(
        report.contains(&format!(
            "claimed: {}; decide asks a run once",
            fixture.claim().display()
        )),
        "{report}"
    );
    assert!(
        !fixture.repo.join(".upstroke").join("proposals").exists(),
        "nothing is written in the checkout"
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
fn each_bad_reply_is_refused_saved_and_fails_the_command() {
    for (at, (text, reason)) in bad_replies().into_iter().enumerate() {
        let fixture = one_question(&format!("decide-refused-{at}"));
        let runner = Scripted::answering(vec![claude_output(&text, Some(0.1))]);
        let decided = decide(&fixture.options(), &BuiltinAdapters, &runner).expect("decided");
        assert_eq!(runner.seen().len(), 1, "{reason}: one question, one call");
        let saved = fixture.proposal("q-01A");
        assert_eq!(saved["status"], "refused", "{reason}: {saved}");
        assert_eq!(
            saved["settled"], true,
            "{reason}: a clean, priced call is settled at its price whatever its reply"
        );
        assert!(
            saved["reason"]
                .as_str()
                .is_some_and(|text| text.contains(reason)),
            "{reason}: {saved}"
        );
        assert!(
            decided.report.contains("q-01A: decided by"),
            "{reason}: {}",
            decided.report
        );
        assert!(decided.refused && decided.question == "q-01A", "{reason}");
        let error = decided
            .outcome()
            .expect_err("a refused reply makes the command an error")
            .to_string();
        assert!(
            error.contains("q-01A") && error.contains("saved with its reason"),
            "{reason}: {error}"
        );
    }
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
    assert_eq!(settings, &fixture.claim().join(SETTINGS_FILE));
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
        "no line outside the block holds three backticks or three tildes",
        "which never gives up on it",
        "\"needs_a_person\"",
        "Giving up on the task is never an available answer here",
        "Nothing you write will be applied",
        "You may only read",
    ] {
        assert!(prompt.contains(fragment), "{fragment}: {prompt}");
    }
}

#[test]
fn a_binding_question_is_refused_and_no_request_is_made_for_it() {
    let fixture = one_question("decide-binding");
    let observed = observe::observe(&fixture.repo, RUN)
        .expect("readable")
        .expect("schema 4");
    let mut binding = observed.open_questions().remove(0);
    binding.binds_an_agent = true;
    let runner = Scripted::answering(Vec::new());
    let message = propose(
        &fixture.options(),
        &observed,
        &binding,
        &BuiltinAdapters,
        &runner,
    )
    .expect_err("a binding question is left to a person")
    .to_string();
    assert!(runner.seen().is_empty(), "no agent was asked");
    assert!(
        runner.probes().is_empty(),
        "with no question to ask, the reviewer's CLI is not probed"
    );
    assert!(
        message.contains(
            "question q-01A chooses which agent runs a task, and binding questions are left to a \
             person"
        ) && message.contains(NOTHING_DONE),
        "{message}"
    );
    assert!(
        !fixture.claimed(),
        "with no call to make, the run is not claimed and nothing is created"
    );
}

#[test]
fn runs_and_questions_that_cannot_be_decided_are_refused_before_any_request() {
    let refused = |fixture: &Fixture, question_id: &str, run_id: &str| {
        let runner = Scripted::answering(Vec::new());
        let options = DecideOptions {
            run_id: run_id.to_owned(),
            ..fixture.asking(question_id)
        };
        let error = decide(&options, &BuiltinAdapters, &runner).expect_err("refused");
        assert!(runner.seen().is_empty(), "no request: {error}");
        error.to_string()
    };

    let unbound = fixture("decide-unbound", 1, None, |log| log.raise("q-01A", 0));
    let message = refused(&unbound, "q-01A", RUN);
    assert!(
        message.contains("recorded no primary review binding"),
        "{message}"
    );

    let settled = fixture("decide-settled", 1, reviewer(), |log| {
        log.raise("q-01A", 0);
        log.answer("q-01A", 0);
        log.raise("q-01B", 0);
    });
    let message = refused(&settled, "q-01A", RUN);
    assert!(
        message.contains("question q-01A of run 01DECIDERUN000000000000001 is no longer open"),
        "{message}"
    );
    for unknown in ["q-01", "Q-01B", "q-01b", "q-01B "] {
        let message = refused(&settled, unknown, RUN);
        assert!(
            message.contains(&format!("has no question {unknown:?}"))
                && message.contains(NOTHING_DONE),
            "{unknown:?}: no prefix is matched and no case folded: {message}"
        );
    }
    assert!(
        !settled.claimed(),
        "nothing was claimed for a question the run does not have open"
    );

    let message = refused(&settled, "q-01B", "01NOSUCHRUN");
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
    let message = refused(&settled, "q-01B", legacy_run);
    assert!(message.contains("is a schema 1-3 run"), "{message}");
}

/// A decide writes nothing in the run but its claim: every other file of the run's public and
/// private directories is as it was, the claim holds the three fixed names and nothing else, and
/// the checkout holds nothing new.
#[test]
fn a_decide_leaves_every_file_of_the_run_as_it_was_and_writes_no_answer() {
    let fixture = one_question("decide-untouched");
    let public_before = snapshot(&fixture.public());
    let private_before = snapshot(&fixture.private);
    let checkout_before = snapshot(&fixture.repo);
    assert!(!public_before.is_empty() && !private_before.is_empty());

    let runner = Scripted::answering(vec![claude_output(&reply(VALID), Some(0.25))]);
    decide(&fixture.options(), &BuiltinAdapters, &runner).expect("decided");

    assert_eq!(snapshot(&fixture.public()), public_before);
    let mut private_after = snapshot(&fixture.private);
    private_after.retain(|path, _| !path.starts_with(fixture.claim()));
    assert_eq!(private_after, private_before, "only the claim is new");
    assert_eq!(
        fixture.claim_files(),
        [PROPOSAL_FILE, RAW_FILE, SETTINGS_FILE]
    );
    assert_eq!(snapshot(&fixture.repo), checkout_before, "the checkout");
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

/// Spend: the allowance is the run's (a run is asked once), and it is the ceiling of the one call
/// the command makes, whole: nothing is subtracted from it. A settled call counts at its price,
/// above its ceiling included. Another open question of the run is never asked.
#[test]
fn the_one_call_is_capped_at_the_allowance_and_counted_at_its_price() {
    for (tag, cap, cost, ceiling) in [
        ("decide-cap-under", 1.0, 0.5, "1"),
        ("decide-cap-reached", 1.0, 1.0, "1"),
        ("decide-cap-overrun", DEFAULT_MAX_COST_USD, 7.0, "5"),
    ] {
        let capped = two_questions(tag);
        let options = DecideOptions {
            max_cost_usd: cap,
            ..capped.options()
        };
        let runner = Acting::priced(cost);
        let decided = decide(&options, &BuiltinAdapters, &runner).expect("decided");
        assert_eq!(asked_about(&runner.seen()), ["q-b"], "{tag}: one call");
        assert_eq!(
            ceilings(&runner.seen()),
            [Some(ceiling.to_owned())],
            "{tag}"
        );
        let saved = capped.proposal("q-b");
        assert_eq!(saved["cost_usd"], cost, "{tag}: settled at its price");
        assert_eq!(saved["ceiling_usd"], cap, "{tag}");
        assert!(!capped.has_proposal("q-a"), "{tag}: the other question");
        assert!(
            decided.report.contains(&format!("cost: ${cost:.4}")),
            "{tag}: {}",
            decided.report
        );
        assert!(decided.outcome().is_ok(), "{tag}: {}", decided.report);
    }
}

/// `PR435-EXHAUSTED-DECIMAL-ALLOWANCE`: a $0.80 allowance and calls priced at $0.70 once left a
/// float remainder that started a third call. A command now makes one call, capped at the whole
/// allowance, and no remainder is computed: the run's other open questions are not asked, by
/// this command or, the run claimed, by any later one.
#[test]
fn a_decimal_allowance_buys_one_call_and_no_remainder_starts_another() {
    let fixture = fixture("decide-decimal", 3, reviewer(), |log| {
        log.raise("q-c", 0);
        log.raise("q-b", 1);
        log.raise("q-a", 2);
    });
    let options = DecideOptions {
        max_cost_usd: 0.8,
        ..fixture.asking("q-b")
    };
    let runner = Acting::priced(0.7);
    let decided = decide(&options, &BuiltinAdapters, &runner).expect("decided");
    assert!(decided.outcome().is_ok(), "{}", decided.report);
    assert_eq!(asked_about(&runner.seen()), ["q-b"]);
    assert_eq!(ceilings(&runner.seen()), [Some("0.8".to_owned())]);
    assert_eq!(fixture.proposal("q-b")["ceiling_usd"], 0.8);
    assert!(!fixture.has_proposal("q-c") && !fixture.has_proposal("q-a"));

    for id in ["q-a", "q-c"] {
        let later = Acting::priced(0.1);
        let message = decide(
            &DecideOptions {
                max_cost_usd: 0.8,
                ..fixture.asking(id)
            },
            &BuiltinAdapters,
            &later,
        )
        .expect_err("the run was decided once")
        .to_string();
        assert!(later.seen().is_empty(), "{id}: {message}");
        assert!(
            message.contains(&format!("run `{RUN}` was claimed already")),
            "{id}: {message}"
        );
    }
}

/// Lays an earlier claim of the run down in its private directory: the claim directory, holding
/// `files`.
fn earlier_claim(fixture: &Fixture, files: &[(&str, &str)]) -> PathBuf {
    let dir = fixture.claim();
    std::fs::create_dir(&dir).expect("an earlier claim");
    for (name, text) in files {
        std::fs::write(dir.join(name), text).expect("an earlier file");
    }
    dir
}

/// Asserts that `decide` was refused as a run claimed already, with no call and nothing in the
/// run's private directory written or changed, and returns the refusal's words.
fn refused_as_claimed(fixture: &Fixture, case: &str) -> String {
    refused_as_claimed_with(fixture, &fixture.options(), case)
}

/// [`refused_as_claimed`], for a decide run with `options`: from another copy of the checkout,
/// for one.
fn refused_as_claimed_with(fixture: &Fixture, options: &DecideOptions, case: &str) -> String {
    let before = snapshot(&fixture.private);
    let runner = Acting::priced(0.25);
    let message = decide(options, &BuiltinAdapters, &runner)
        .map(|decided| decided.report)
        .expect_err(case)
        .to_string();
    assert!(runner.seen().is_empty(), "{case}: no agent was asked");
    assert_eq!(
        snapshot(&fixture.private),
        before,
        "{case}: nothing was written"
    );
    assert!(
        message.contains(&format!("run `{RUN}` was claimed already"))
            && message.contains(NOTHING_DONE),
        "{case}: {message}"
    );
    message
}

/// Directory: the claim is created exclusively, and whatever already stands at its name (an
/// empty directory, as an interrupted claim leaves it, a partial or a complete earlier claim, a
/// file or a link) means no write and no call.
#[test]
fn a_run_whose_claim_exists_is_not_asked_and_nothing_is_written() {
    // The control: the same run with nothing at the name is claimed and asked once.
    let control = one_question("decide-unclaimed");
    let runner = Acting::priced(0.25);
    decide(&control.options(), &BuiltinAdapters, &runner).expect("decided");
    assert_eq!(runner.seen().len(), 1);
    assert!(control.claim().is_dir());

    let completed = r#"{"question_id": "q-01A", "status": "accepted", "cost_usd": 0.25}"#;
    let cases: [(&str, &[(&str, &str)]); 3] = [
        ("an empty directory", &[]),
        ("a partial claim", &[(SETTINGS_FILE, "{}")]),
        (
            "a complete claim",
            &[
                (SETTINGS_FILE, "{}"),
                (RAW_FILE, "raw"),
                (PROPOSAL_FILE, completed),
            ],
        ),
    ];
    for (at, (case, files)) in cases.into_iter().enumerate() {
        let fixture = one_question(&format!("decide-claimed-{at}"));
        earlier_claim(&fixture, files);
        refused_as_claimed(&fixture, case);
    }

    let filed = one_question("decide-claimed-file");
    std::fs::write(filed.claim(), "not a directory").expect("a file at the name");
    refused_as_claimed(&filed, "a file at the name");

    #[cfg(unix)]
    {
        let linked = one_question("decide-claimed-link");
        let elsewhere = linked.repo.join("elsewhere");
        std::fs::create_dir_all(&elsewhere).expect("a directory elsewhere");
        std::os::unix::fs::symlink(&elsewhere, linked.claim()).expect("a link");
        refused_as_claimed(&linked, "a link at the name");
        assert!(
            snapshot(&elsewhere).is_empty(),
            "nothing was written through the link"
        );
    }
}

/// Directory and interruption: the run is claimed in its private directory immediately before
/// its first call, with the mode a plain `create_dir` gives it and no other, and the claim stays
/// after the call is interrupted, so a later decide asks nothing.
#[test]
fn the_run_is_claimed_in_its_private_directory_before_its_first_call_and_the_claim_outlives_an_interruption()
 {
    let fixture = one_question("decide-claim");
    let claim = fixture.claim();
    let private = fixture.private.clone();
    let runner = Acting::new(move |_| {
        let names: Vec<String> = snapshot(&claim)
            .keys()
            .filter_map(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            names,
            [SETTINGS_FILE],
            "claimed, and holding only the call's settings, while the call is in flight"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt as _;
            // A directory made the plain way beside it, now, under this process's umask.
            let plain = private.with_file_name("plain");
            std::fs::create_dir(&plain).expect("a plain directory");
            let mode = |path: &Path| {
                let metadata = std::fs::symlink_metadata(path).expect("there");
                assert!(metadata.file_type().is_dir(), "{}", path.display());
                metadata.mode() & 0o777
            };
            assert_eq!(
                mode(&claim),
                mode(&plain),
                "the claim keeps the mode create_dir gave it"
            );
            assert_eq!(mode(&private), 0o700, "its parent's mode is unchanged");
            let owner = std::fs::symlink_metadata(&plain).expect("there").uid();
            assert_eq!(
                std::fs::symlink_metadata(&claim).expect("there").uid(),
                owner
            );
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
    assert_eq!(
        interrupted.claim_files(),
        [SETTINGS_FILE],
        "the claim stays"
    );
    refused_as_claimed(&interrupted, "an interrupted run");
}

/// Durability: the private directory is synced once the claim is created in it, so the claim is
/// durable before the call. If it cannot be synced, the run is claimed, the claim stays, and no
/// agent is asked.
#[test]
fn the_private_directory_is_synced_once_the_claim_is_created_in_it() {
    let fixture = one_question("decide-synced");
    let runner = Acting::priced(0.25);
    let message = {
        let _fault = crate::util::fail_barriers_at(&fixture.private);
        decide(&fixture.options(), &BuiltinAdapters, &runner)
            .map(|decided| decided.report)
            .expect_err("the private directory cannot be synced")
            .to_string()
    };
    assert!(runner.seen().is_empty(), "no agent was asked: {message}");
    assert!(
        message.contains(&format!("run `{RUN}` is claimed"))
            && message.contains("could not be synced")
            && message.contains("no agent was asked"),
        "{message}"
    );
    assert!(
        fixture.claimed() && fixture.claim_files().is_empty(),
        "{message}"
    );
    refused_as_claimed(&fixture, "a claim whose parent could not be synced");
}

/// Copies the checkout at `from` to `to`, whole: the run's log, its public directory and the
/// rest, as copying a checkout before its first decision does. The run's private directory is
/// outside the checkout, and the copy's log names the same one.
fn copy_checkout(from: &Path, to: &Path) {
    let mut pending = vec![from.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let relative = dir.strip_prefix(from).expect("under the checkout");
        std::fs::create_dir_all(to.join(relative)).expect("a directory of the copy");
        for entry in std::fs::read_dir(&dir).expect("read a directory").flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let relative = path.strip_prefix(from).expect("under the checkout");
                std::fs::copy(&path, to.join(relative)).expect("a file of the copy");
            }
        }
    }
}

/// `PR435-COPIED-RUN-DOUBLE-SPEND`: a checkout copied before its first decision carries the same
/// run, events, private directory and owner record. Each copy has a run lock of its own, but
/// the claim is in the run's private directory, which both name, so the second decide, from either
/// copy, finds it and is refused: the run's allowance is spent once.
#[test]
fn copied_checkout_shares_the_run_allowance() {
    let fixture = one_question("decide-copied");
    let copy = fixture.repo.with_file_name("copy");
    copy_checkout(&fixture.repo, &copy);
    let from_copy = DecideOptions {
        repo_root: copy.clone(),
        ..fixture.options()
    };

    let runner = Acting::priced(5.0);
    let decided = decide(&fixture.options(), &BuiltinAdapters, &runner).expect("decided");
    assert!(decided.outcome().is_ok(), "{}", decided.report);
    assert_eq!(ceilings(&runner.seen()), [Some("5".to_owned())]);
    refused_as_claimed_with(&fixture, &from_copy, "the copy, after the original");
    refused_as_claimed(&fixture, "the original, again");
    assert!(
        !copy.join(".upstroke").join("proposals").exists()
            && !fixture.repo.join(".upstroke").join("proposals").exists(),
        "neither copy holds a claim of its own"
    );
}

/// The exclusive `create_dir` is the claim, and nothing else is: while the first command's call
/// is in flight, its proposal not yet written and its run lock held in the original's public
/// directory only, a decide from the copy, which takes the copy's own lock, finds the claim and
/// is refused before anything is written or asked. With the claim directory removed, as nothing
/// but a test does, the same decide from the copy is asked.
#[test]
fn the_exclusive_create_dir_is_what_makes_a_second_command_find_the_claim() {
    let fixture = one_question("decide-raced");
    let copy = fixture.repo.with_file_name("copy");
    copy_checkout(&fixture.repo, &copy);
    let from_copy = DecideOptions {
        repo_root: copy.clone(),
        ..fixture.options()
    };
    let second = std::sync::Arc::new(Mutex::new(Vec::new()));
    let runner = Acting::new({
        let second = std::sync::Arc::clone(&second);
        let from_copy = DecideOptions {
            repo_root: copy.clone(),
            ..fixture.options()
        };
        move |_| {
            let inner = Acting::priced(0.25);
            let refused = decide(&from_copy, &BuiltinAdapters, &inner)
                .map(|decided| decided.report)
                .map_err(|error| error.to_string());
            lock(&second).push((refused, inner.seen().len()));
            Ok(exited(claude_output(&reply(VALID), Some(0.25))))
        }
    });
    let decided = decide(&fixture.options(), &BuiltinAdapters, &runner).expect("decided");
    assert!(decided.outcome().is_ok(), "{}", decided.report);
    let attempts = lock(&second).clone();
    let [(Err(message), 0)] = attempts.as_slice() else {
        panic!("one second command, refused with no call: {attempts:?}");
    };
    assert!(
        message.contains(&format!("run `{RUN}` was claimed already"))
            && message.contains(NOTHING_DONE),
        "{message}"
    );

    // The same second command, with nothing at the claim's name, is admitted and asked.
    std::fs::remove_dir_all(fixture.claim()).expect("the claim, removed");
    let runner = Acting::priced(0.25);
    let decided = decide(&from_copy, &BuiltinAdapters, &runner).expect("decided");
    assert!(decided.outcome().is_ok(), "{}", decided.report);
    assert_eq!(runner.seen().len(), 1);
}

/// `PR435-CLAIM-MODE-AT-CREATION`: no directory decide creates is narrowed after it is created,
/// and none is shared between runs or checkouts. A `.upstroke/proposals` an interrupted decide
/// of the old layout left open in the checkout stops nothing: no later decide reads it.
#[cfg(unix)]
#[test]
fn a_directory_left_open_in_the_checkout_stops_no_decide() {
    let fixture = one_question("decide-left-open");
    let left = fixture.repo.join(".upstroke").join("proposals");
    std::fs::create_dir_all(&left).expect("a directory an interrupted decide left");
    set_mode(&left, 0o755);
    let runner = Acting::priced(0.25);
    let decided = decide(&fixture.options(), &BuiltinAdapters, &runner).expect("decided");
    assert!(decided.outcome().is_ok(), "{}", decided.report);
    assert_eq!(runner.seen().len(), 1);
    assert!(
        snapshot(&left).is_empty(),
        "nothing is written in the checkout"
    );
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
    assert!(!fixture.claimed(), "{case}: the run was not claimed");
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
/// other outcome a call returns is saved as refused with what it reported, and fails the
/// command.
#[test]
fn only_a_clean_successful_priced_call_settles_and_any_other_is_saved_refused() {
    // The controls: a clean, priced call settles at its price whatever its reply.
    for (case, stdout, status) in [
        (
            "an accepted reply",
            claude_output(&reply(VALID), Some(0.25)),
            "accepted",
        ),
        (
            "a refused reply",
            claude_output(
                &reply(&VALID.replace("\"choice\": 1", "\"choice\": 9")),
                Some(0.25),
            ),
            "refused",
        ),
    ] {
        let fixture = one_question(&format!("decide-settled-{status}"));
        let runner = Scripted::answering(vec![stdout]);
        let decided = decide(&fixture.options(), &BuiltinAdapters, &runner).expect("decided");
        assert_eq!(ceilings(&runner.seen()), [Some("5".to_owned())], "{case}");
        let saved = fixture.proposal("q-01A");
        assert_eq!(saved["status"], status, "{case}");
        assert_eq!(saved["settled"], true, "{case}");
        assert_eq!(saved["cost_usd"], 0.25, "{case}");
        assert!(decided.report.contains("cost: $0.2500"), "{case}");
        assert_eq!(decided.refused, status == "refused", "{case}");
        assert_eq!(decided.outcome().is_ok(), status == "accepted", "{case}");
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
        let fixture = one_question(&format!("decide-unsettled-{at}"));
        let runner = Acting::new(returns);
        let decided = if case == UNSUCCESSFUL {
            decide(&fixture.options(), &Only(&misread), &runner)
        } else {
            decide(&fixture.options(), &BuiltinAdapters, &runner)
        }
        .expect("decided");
        assert_eq!(runner.seen().len(), 1, "{case}: one call");
        let saved = fixture.proposal("q-01A");
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
            fixture.claim().join(RAW_FILE).is_file(),
            "{case}: the raw output, or the failure, is saved"
        );
        let report = &decided.report;
        assert!(
            report
                .lines()
                .any(|line| line.starts_with("  status: refused — ") && line.contains(reason)),
            "{case}: {report}"
        );
        for line in [
            "  cost: unknown".to_owned(),
            format!("claimed: {}", fixture.claim().display()),
        ] {
            assert!(report.contains(&line), "{case}: {line}: {report}");
        }
        assert!(decided.refused, "{case}");
        let error = decided.outcome().expect_err("nonzero").to_string();
        assert!(
            error.contains("question q-01A was refused"),
            "{case}: {error}"
        );
        refused_as_claimed(&fixture, case);
    }
}

/// Publication: every file is created new and none is replaced. A name another hand took while
/// the call was in flight fails the command with that file as it was and no proposal saved.
#[test]
fn a_taken_name_is_never_replaced_and_fails_the_command() {
    let fixture = one_question("decide-taken");
    let dir = fixture.claim();
    let runner = Acting::new({
        let dir = dir.clone();
        move |_| {
            std::fs::write(dir.join(RAW_FILE), "someone else's bytes").expect("a stray file");
            Ok(exited(claude_output(&reply(VALID), Some(0.25))))
        }
    });
    let message = decide(&fixture.options(), &BuiltinAdapters, &runner)
        .map(|decided| decided.report)
        .expect_err("the raw output's name was taken")
        .to_string();
    assert_eq!(runner.seen().len(), 1, "{message}");
    assert_eq!(
        std::fs::read_to_string(dir.join(RAW_FILE)).expect("the stray file"),
        "someone else's bytes"
    );
    assert!(!fixture.has_proposal("q-01A"));
    assert!(
        message.contains("no proposal for question q-01A was saved")
            && message.contains(RAW_FILE)
            && message.contains("the run stays claimed"),
        "{message}"
    );
    assert!(is_one_line(&message), "{message:?}");

    // The control: the same run, nothing in the way.
    let control = one_question("decide-untaken");
    let runner = Acting::priced(0.25);
    let decided = decide(&control.options(), &BuiltinAdapters, &runner).expect("decided");
    assert_eq!(runner.seen().len(), 1);
    assert!(decided.outcome().is_ok(), "{}", decided.report);
}

#[test]
fn a_run_id_is_a_plain_file_name_component_or_no_path_is_built_from_it() {
    for id in ["q-01A", "Q_1", "a"] {
        assert!(plain_id(id), "{id}");
    }
    let long = "a".repeat(MAX_ID_BYTES + 1);
    for id in [
        "", ".", "..", "a.b", "a/b", r"a\b", "a:b", "a b", "CON", "nul", "com1", "LPT9", &long,
    ] {
        assert!(!plain_id(id), "{id}");
    }
}

/// `PR435-CASE-INSENSITIVE-PROPOSAL-NAMES`: no question id becomes a file name. A run's one claim
/// holds the same three fixed names whichever question it was spent on, so two ids that differ
/// only by case, or an id that is no plain file-name component, name no file at all.
#[test]
fn no_question_id_becomes_a_file_name() {
    let dotted = fixture("decide-dotted", 1, reviewer(), |log| log.raise("q.01A", 0));
    let runner = Acting::priced(0.25);
    let decided = decide(&dotted.options(), &BuiltinAdapters, &runner).expect("decided");
    assert!(decided.outcome().is_ok(), "{}", decided.report);
    assert_eq!(asked_about(&runner.seen()), ["q.01A"]);
    assert_eq!(dotted.proposal("q.01A")["status"], "accepted");
    assert_eq!(
        dotted.claim_files(),
        [PROPOSAL_FILE, RAW_FILE, SETTINGS_FILE]
    );
}

/// The operator's condition on the claim: it is created in the run's private directory with no
/// mode of its own, so decide first checks that directory: a real directory that, on Unix,
/// belongs to the user decide runs as and grants group and other nothing. Each mode below is the
/// fixture's private directory's own; any bit for group or other refuses the run before anything
/// is written, probed or asked.
#[cfg(unix)]
#[test]
fn a_private_directory_open_to_another_user_is_refused_before_anything() {
    for (at, (mode, open)) in [
        (0o700, false),
        (0o750, true),
        (0o710, true),
        (0o705, true),
        (0o701, true),
        (0o755, true),
        (0o777, true),
    ]
    .into_iter()
    .enumerate()
    {
        let fixture = one_question(&format!("decide-private-mode-{at}"));
        set_mode(&fixture.private, mode);
        let before = snapshot(&fixture.private);
        let runner = Acting::priced(0.25);
        let decided = decide(&fixture.options(), &BuiltinAdapters, &runner);
        let case = format!("the private directory at {mode:03o}");
        if !open {
            let decided = decided.unwrap_or_else(|error| panic!("{case}: {error}"));
            assert!(decided.outcome().is_ok(), "{case}: {}", decided.report);
            assert_eq!(runner.seen().len(), 1, "{case}");
            continue;
        }
        let message = decided
            .map(|decided| decided.report)
            .expect_err(&case)
            .to_string();
        assert!(
            message.contains(&format!(
                "run `{RUN}`'s private directory is not one decide keeps its claim in"
            )) && message.contains(&format!(
                "is open to its group or to other users (mode {mode:03o})"
            )) && message.contains(NOTHING_DONE),
            "{case}: {message}"
        );
        assert!(
            runner.seen().is_empty() && runner.probes().is_empty(),
            "{case}: nothing was probed or asked"
        );
        assert!(!fixture.claimed(), "{case}: the run was not claimed");
        assert_eq!(
            snapshot(&fixture.private),
            before,
            "{case}: nothing was written"
        );
    }

    // A link at the private directory's recorded path, to a directory closed to every other user,
    // is no private directory of the run's.
    let linked = one_question("decide-private-link");
    let elsewhere = linked.private.with_file_name("elsewhere");
    std::fs::rename(&linked.private, &elsewhere).expect("the private directory, moved");
    std::os::unix::fs::symlink(&elsewhere, &linked.private).expect("a link in its place");
    let message = refused_before_anything(&linked, &linked.options(), &BuiltinAdapters, "a link");
    assert!(
        message.contains("is not a directory (a link or a file stands at its name)"),
        "{message}"
    );
    assert!(
        !elsewhere.join(CLAIM_DIR).exists(),
        "nothing was created through the link"
    );
}

/// `PR435-PUBLICATION-USES-STALE-CHAIN`: every file is written immediately after the check that
/// admits it, and the raw output's and the proposal's checks are made once the reviewer has
/// returned. A claim replaced by a link, or a private directory opened to its group, while the
/// call is in flight is seen, and nothing is written into it; the run stays claimed.
#[cfg(unix)]
#[test]
fn a_claim_changed_while_the_reviewer_runs_is_not_written_into() {
    type Change = fn(&Path, &Path);
    let cases: [(&str, Change, &str); 2] = [
        (
            "the claim replaced by a link",
            |claim, elsewhere| {
                std::fs::rename(claim, claim.with_file_name("moved")).expect("moved");
                std::os::unix::fs::symlink(elsewhere, claim).expect("a link in its place");
            },
            "is not a directory (a link or a file stands at its name)",
        ),
        (
            "the private directory opened to its group",
            |claim, _| set_mode(claim.parent().expect("the private directory"), 0o750),
            "is open to its group or to other users (mode 750)",
        ),
    ];
    for (at, (case, change, needle)) in cases.into_iter().enumerate() {
        let fixture = one_question(&format!("decide-changed-{at}"));
        let elsewhere = fixture.repo.join("elsewhere");
        std::fs::create_dir_all(&elsewhere).expect("a directory elsewhere");
        let runner = Acting::new({
            let claim = fixture.claim();
            let elsewhere = elsewhere.clone();
            move |_| {
                change(&claim, &elsewhere);
                Ok(exited(claude_output(&reply(VALID), Some(0.25))))
            }
        });
        let message = decide(&fixture.options(), &BuiltinAdapters, &runner)
            .map(|decided| decided.report)
            .expect_err(case)
            .to_string();
        assert_eq!(runner.seen().len(), 1, "{case}: {message}");
        assert!(
            message.contains("no proposal for question q-01A was saved")
                && message.contains(&format!("{RAW_FILE}` was not written"))
                && message.contains(needle)
                && message.contains("the run stays claimed"),
            "{case}: {message}"
        );
        assert!(snapshot(&elsewhere).is_empty(), "{case}: nothing elsewhere");
        let written: Vec<PathBuf> = snapshot(&fixture.private).into_keys().collect();
        assert!(
            written
                .iter()
                .all(|path| !path.ends_with(RAW_FILE) && !path.ends_with(PROPOSAL_FILE)),
            "{case}: {written:?}"
        );
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
    assert!(!fixture.claimed());

    // The control: the same run, once nothing drives it, is asked.
    drop(held);
    let decided = decide(&fixture.options(), &BuiltinAdapters, &runner).expect("decided");
    assert!(decided.outcome().is_ok(), "{}", decided.report);
    assert_eq!(runner.seen().len(), 1, "{}", decided.report);
}

/// Publishes a person's answer to `id` in the run's answers directory, as `upstroke answer` does.
fn answered_by_a_person(fixture: &Fixture, id: &str, answer: Answer) {
    publish_answer(&fixture.public(), id, answer);
}

/// Publishes a person's answer to `id` under the run's public directory `public`, through the
/// production answer writer.
fn publish_answer(public: &Path, id: &str, answer: Answer) {
    let answers = public.join("answers");
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
fn a_question_a_person_has_answered_is_refused_and_never_asked() {
    for (at, answer, described) in [
        (
            0,
            Answer::Answered {
                text: RETRY.to_owned(),
            },
            "(\"retry on frontier\")",
        ),
        (1, Answer::Declined, "(declined)"),
    ] {
        let fixture = two_questions(&format!("decide-answered-{at}"));
        answered_by_a_person(&fixture, "q-b", answer);
        let runner = Acting::priced(0.25);
        let message = decide(&fixture.asking("q-b"), &BuiltinAdapters, &runner)
            .map(|decided| decided.report)
            .expect_err("an answered question is not asked")
            .to_string();
        assert!(
            message.contains(&format!(
                "a person has answered question q-b already {described}, and the run applies \
                 that answer; {NOTHING_DONE}"
            )),
            "{message}"
        );
        assert!(
            runner.seen().is_empty() && runner.probes().is_empty(),
            "nothing to ask, so nothing is probed or asked"
        );
        assert!(!fixture.claimed(), "the run is not claimed");

        // The control: the run's other, unanswered question is asked.
        let decided = decide(&fixture.asking("q-a"), &BuiltinAdapters, &runner).expect("decided");
        assert!(decided.outcome().is_ok(), "{}", decided.report);
        assert_eq!(asked_about(&runner.seen()), ["q-a"]);
        assert!(!fixture.has_proposal("q-b") && fixture.has_proposal("q-a"));
    }
}

/// A Runner that answers the pre-flight's probes as a signed-in Claude Code and, at the first
/// probe, publishes a person's answer to `question` through the production answer writer: an
/// answer that arrives after the command read the answers directory at admission, and before its
/// call. Every other request is a call, recorded and priced.
struct AnsweredDuringPreflight {
    public: PathBuf,
    question: &'static str,
    published: Mutex<bool>,
    seen: Mutex<Vec<RunnerRequest>>,
}

impl Runner for AnsweredDuringPreflight {
    fn run<'a>(&'a self, request: &'a RunnerRequest, _call: RunnerCall<'a>) -> RunFuture<'a> {
        if let Some(probed) = probe_reply(request, SIGNED_IN) {
            let mut published = lock(&self.published);
            if !*published {
                publish_answer(
                    &self.public,
                    self.question,
                    Answer::Answered {
                        text: RETRY.to_owned(),
                    },
                );
                *published = true;
            }
            return Box::pin(async move { Ok(probed) });
        }
        lock(&self.seen).push(request.clone());
        let answer = Ok(exited(claude_output(&reply(VALID), Some(0.25))));
        Box::pin(async move { answer })
    }
}

/// `PR435-ANSWER-AFTER-ADMISSION`: a person answers the question while the command is being
/// admitted (here, during the reviewer's pre-flight). The command reads the answers directory
/// again immediately before it claims the run and starts its call, so the answer is seen and no
/// paid call is made.
#[test]
fn an_answer_published_after_admission_is_seen_before_the_call() {
    let fixture = one_question("decide-answered-late");
    let runner = AnsweredDuringPreflight {
        public: fixture.public(),
        question: "q-01A",
        published: Mutex::new(false),
        seen: Mutex::new(Vec::new()),
    };
    let message = decide(&fixture.options(), &BuiltinAdapters, &runner)
        .map(|decided| decided.report)
        .expect_err("the question was answered before its call")
        .to_string();
    assert!(
        *lock(&runner.published),
        "the answer was published mid-admission"
    );
    assert!(lock(&runner.seen).is_empty(), "no call was made: {message}");
    assert!(
        message.contains("a person has answered question q-01A already"),
        "{message}"
    );
    assert!(
        !fixture.claimed(),
        "the run is not claimed, so it can still be decided: {message}"
    );
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
        assert!(!fixture.claimed(), "{case}: nothing was written");
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

/// An installed Claude Code whose `--version` probe exits with status 1, printing `HOSTILE` on
/// its stderr, which the probe's error quotes.
fn hostile_version(request: &RunnerRequest) -> Option<Result<ProcessOutput, RunnerError>> {
    let versioned = request.command.args.iter().any(|arg| arg == "--version");
    probe_reply(request, SIGNED_IN).map(|probed| {
        Ok(if versioned {
            ProcessOutput {
                code: Some(1),
                stderr: HOSTILE.to_owned(),
                ..probed
            }
        } else {
            probed
        })
    })
}

/// Whether `message` is one line, as `one_line` makes one: no line break, and no control
/// character but escaped.
fn is_one_line(message: &str) -> bool {
    !message.chars().any(char::is_control) && one_line(message.to_owned()) == message
}

/// Refusals: every refusal the command returns is one line, its controls made visible, whatever
/// the reviewer's CLI answered its pre-flight with and whichever module built the refusal.
#[test]
fn every_refusal_is_one_line_whatever_the_reviewer_answered() {
    let statuses: [(&str, Probe, &str); 3] = [
        (
            "its version probe exits 1 printing hostile text",
            hostile_version,
            "--version exited with Some(1): retry q-02: decided by someone/else at max \
             effort\\u{1b}[2J\\u{7}; nothing was written and no agent was asked",
        ),
        (
            "signed out",
            signed_out,
            "is not signed in under pool `claude-max`'s account",
        ),
        (
            "not installed",
            not_installed,
            "did not pass the pre-flight a run makes of it",
        ),
    ];
    for (at, (case, probe, needle)) in statuses.into_iter().enumerate() {
        let fixture = one_question(&format!("decide-one-line-{at}"));
        let runner = Acting::priced(0.25).probing(probe);
        let message = decide(&fixture.options(), &BuiltinAdapters, &runner)
            .expect_err(case)
            .to_string();
        assert!(is_one_line(&message), "{case}: {message:?}");
        assert!(message.contains(needle), "{case}: {message}");
        assert!(runner.seen().is_empty(), "{case}: no agent was asked");
        assert!(!fixture.claimed(), "{case}: nothing was written");
    }

    // A refusal the credential pre-flight builds itself, naming the pools file's profile.
    let fixture = one_question("decide-one-line-profile");
    let profile = fixture.profile.join(HOSTILE);
    std::fs::write(
        fixture.repo.join("pools.toml"),
        format!(
            "[pools.claude-max]\nkind = \"subscription-window\"\nagent = \"{}\"\nprofile = {}\n",
            crate::agent::claude::ADAPTER_ID,
            json!(profile.to_string_lossy())
        ),
    )
    .expect("a pools file naming a hostile profile");
    let message =
        refused_before_anything(&fixture, &fixture.options(), &BuiltinAdapters, "profile");
    assert!(is_one_line(&message), "{message:?}");
    assert!(
        message.contains("pre-flight: pool `claude-max` names the credential profile directory")
            && message.contains("\\u{1b}[2J\\u{7}"),
        "{message}"
    );
}

/// The ids of the questions a recording Runner was asked about, in order, read from each call's
/// prompt.
fn asked_about(seen: &[RunnerRequest]) -> Vec<String> {
    seen.iter()
        .filter_map(|request| {
            String::from_utf8_lossy(&request.command.stdin)
                .lines()
                .find_map(|line| line.strip_prefix("id: ").map(str::to_owned))
        })
        .collect()
}

/// One case of the exit report: the status the faked reviewer's adapter reads every call as
/// (`None`: Claude Code's own reading), whether the reply is one `check_reply` refuses, and
/// whether the command must say its proposal was refused.
struct Exit {
    case: &'static str,
    status: Option<OutcomeStatus>,
    refuse: bool,
    refused: bool,
}

const EXITS: [Exit; 5] = [
    Exit {
        case: "the reply accepted",
        status: None,
        refuse: false,
        refused: false,
    },
    Exit {
        case: "the reply refused",
        status: None,
        refuse: true,
        refused: true,
    },
    Exit {
        case: "an agent error",
        status: Some(OutcomeStatus::AgentError),
        refuse: false,
        refused: true,
    },
    Exit {
        case: "a timeout",
        status: Some(OutcomeStatus::Timeout),
        refuse: false,
        refused: true,
    },
    Exit {
        case: "rate limited",
        status: Some(OutcomeStatus::RateLimited),
        refuse: true,
        refused: true,
    },
];

/// Reports: the exit names a saved reason only for the proposal this command wrote, and the
/// report's `saved` line names exactly the proposal on disk. The reviewer is faked by the status
/// its adapter reads the call as, and the Runner is the recording sink the call reaches: nothing
/// is started, and the call it records is the question asked.
#[test]
fn the_exit_report_names_only_the_record_this_command_wrote() {
    let refused_reply = || {
        claude_output(
            &reply(&VALID.replace("\"choice\": 1", "\"choice\": 9")),
            Some(0.25),
        )
    };
    for (at, exit) in EXITS.into_iter().enumerate() {
        let Exit {
            case,
            status,
            refuse,
            refused,
        } = exit;
        let fixture = fixture(&format!("decide-exit-{at}"), 2, reviewer(), |log| {
            log.raise("q-c", 0);
            log.raise("q-a", 1);
        });
        let runner = if refuse {
            Acting::new(move |_| Ok(exited(refused_reply())))
        } else {
            Acting::priced(0.25)
        };
        let decided = match status {
            Some(status) => decide(&fixture.options(), &Only(&Misread(status)), &runner),
            None => decide(&fixture.options(), &BuiltinAdapters, &runner),
        }
        .expect("decided");
        let report = &decided.report;

        assert_eq!(asked_about(&runner.seen()), ["q-c"], "{case}: one call");
        assert_eq!(decided.question, "q-c", "{case}: {report}");
        assert_eq!(decided.refused, refused, "{case}: {report}");
        assert_eq!(
            fixture.proposal("q-c")["status"],
            if refused { "refused" } else { "accepted" },
            "{case}"
        );
        assert_eq!(
            fixture.claim_files(),
            [PROPOSAL_FILE, RAW_FILE, SETTINGS_FILE],
            "{case}: the one proposal on disk"
        );
        assert!(
            report.lines().any(|line| line
                == format!("  saved: {}", fixture.claim().join(PROPOSAL_FILE).display())),
            "{case}: {report}"
        );

        match decided.outcome() {
            Ok(()) => assert!(!refused, "{case}: {report}"),
            Err(error) => {
                let message = error.to_string();
                assert!(refused, "{case}: {message}");
                assert!(is_one_line(&message), "{case}: {message:?}");
                assert!(
                    message.contains(
                        "the reply to question q-c was refused; it is saved with its reason in \
                         its proposal"
                    ),
                    "{case}: {message}"
                );
            }
        }
    }
}

/// `PR435-RUN-LOCK-AFTER-ADMISSION`: the command holds the run's own lock, the one `resume`
/// takes, from before admission until its proposal is written, so a resume that starts while the
/// reviewer is asked cannot take the run; and once the command returns the lock is free again.
#[test]
fn a_resume_cannot_take_the_run_while_decide_holds_it() {
    let fixture = one_question("decide-locked");
    let public = fixture.public();
    let resumed = std::sync::Arc::new(Mutex::new(Vec::new()));
    let runner = Acting::new({
        let public = public.clone();
        let resumed = std::sync::Arc::clone(&resumed);
        move |_| {
            // What `resume` takes first on the run, attempted while the call is in flight.
            let taken = crate::rundir::RunLock::acquire(&public).map(drop);
            lock(&resumed).push(taken.map_err(|error| error.to_string()));
            Ok(exited(claude_output(&reply(VALID), Some(0.25))))
        }
    });
    let decided = decide(&fixture.options(), &BuiltinAdapters, &runner).expect("decided");
    assert!(decided.outcome().is_ok(), "{}", decided.report);
    let attempts = lock(&resumed).clone();
    let [attempt] = attempts.as_slice() else {
        panic!("one call, one resume attempted during it: {attempts:?}");
    };
    assert!(
        attempt.is_err(),
        "a resume took the run while decide was asking its reviewer"
    );
    assert!(
        !crate::rundir::is_running(&public),
        "decide gives the run's lock back when it returns"
    );
    drop(crate::rundir::RunLock::acquire(&public).expect("a resume can take the run now"));
}

/// `PR435-CASE-FOLDED-QUESTION-SELECTION`: two open questions whose ids differ only by case. The
/// question asked, and the run's one claim spent, is the one named byte for byte. The claim's
/// file names are fixed, so neither id names a file and the two cannot collide on a filesystem
/// that folds case (`PR435-CASE-INSENSITIVE-PROPOSAL-NAMES`); a decide of the other is refused as
/// the run claimed already, with nothing written.
#[test]
fn the_exact_question_is_asked_when_ids_differ_by_case() {
    let fixture = fixture("decide-case", 2, reviewer(), |log| {
        log.raise("q-A", 0);
        log.raise("q-a", 1);
    });
    let runner = Acting::priced(0.25);
    let decided = decide(&fixture.asking("q-a"), &BuiltinAdapters, &runner).expect("decided");
    assert!(decided.outcome().is_ok(), "{}", decided.report);
    assert_eq!(asked_about(&runner.seen()), ["q-a"]);
    assert_eq!(decided.question, "q-a");
    assert_eq!(fixture.proposal("q-a")["question_id"], "q-a");
    assert_eq!(
        fixture.claim_files(),
        [PROPOSAL_FILE, RAW_FILE, SETTINGS_FILE],
        "no file is named after either id"
    );
    refused_as_claimed_with(&fixture, &fixture.asking("q-A"), "the other id");
}

/// `PR435-RUN-ID-VALIDATED-AFTER-LOOKUP`: a run id is checked as one before any path is built
/// from it. An absolute directory, here an archived run's that holds a readable log, or a path
/// that climbs out of the runs directory, is refused with its log unread and nothing done.
#[test]
fn a_run_id_that_is_not_one_is_refused_before_any_path_is_built() {
    let fixture = one_question("decide-run-id");
    let archived = one_question("decide-run-id-archived");
    let elsewhere = archived.public();
    let before = snapshot(&elsewhere);
    for run_id in [
        elsewhere.to_string_lossy().into_owned(),
        format!("../runs/{RUN}"),
        "..".to_owned(),
        String::new(),
    ] {
        let runner = Acting::priced(0.25);
        let options = DecideOptions {
            run_id: run_id.clone(),
            ..fixture.options()
        };
        let message = decide(&options, &BuiltinAdapters, &runner)
            .map(|decided| decided.report)
            .expect_err("not a run id")
            .to_string();
        assert!(
            message.contains("is not a run id") && message.contains(NOTHING_DONE),
            "{run_id}: {message}"
        );
        assert!(
            !message.contains("schema-4") && !message.contains("run_started"),
            "{run_id}: no log was read: {message}"
        );
        assert!(
            runner.seen().is_empty() && runner.probes().is_empty(),
            "{run_id}"
        );
    }
    assert_eq!(
        snapshot(&elsewhere),
        before,
        "the archived run is as it was"
    );
    assert!(!fixture.claimed() && !archived.claimed());
}

/// `PR435-INTERRUPTED-RUN-CLAIM`: a run with an open question and another runnable task, its lock
/// free, is one `status` reports interrupted: the fold derives no end and its log records none.
/// Only a run its log records parked is asked, so this one is refused with nothing spent.
#[test]
fn an_interrupted_run_with_an_open_question_is_refused_before_anything() {
    let fixture = fixture("decide-interrupted-run", 2, reviewer(), |log| {
        log.raise("q-01A", 0);
    });
    let observed = observe::observe(&fixture.repo, RUN)
        .expect("readable")
        .expect("schema 4");
    assert_eq!(observed.open_questions().len(), 1, "an open question");
    assert!(observed.outcome().is_none(), "no end is recorded");
    let status = observed.render().expect("status renders");
    assert!(status.contains("state: interrupted"), "{status}");

    let runner = Acting::priced(0.25);
    let message = decide(&fixture.options(), &BuiltinAdapters, &runner)
        .map(|decided| decided.report)
        .expect_err("an interrupted run is not parked")
        .to_string();
    assert!(
        message.contains(&format!("run `{RUN}` is not parked"))
            && message.contains("interrupted")
            && message.contains(NOTHING_DONE),
        "{message}"
    );
    assert!(runner.seen().is_empty() && runner.probes().is_empty());
    assert!(!fixture.claimed(), "the run's claim is not spent");

    // The control: the same question, its run ended parked, is asked.
    let parked = one_question("decide-parked-run");
    let observed = observe::observe(&parked.repo, RUN)
        .expect("readable")
        .expect("schema 4");
    assert_eq!(observed.outcome(), Some(&crate::events::RunOutcome::Parked));
    let runner = Acting::priced(0.25);
    decide(&parked.options(), &BuiltinAdapters, &runner).expect("a parked run is asked");
    assert_eq!(runner.seen().len(), 1);
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
