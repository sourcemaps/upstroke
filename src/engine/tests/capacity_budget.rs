//! Extended notes: `docs/internals/engine/tests/capacity_budget.md`
// Allowlist placement: the funnel section of `effects/allowlist.toml`.
#![allow(clippy::disallowed_methods)]
#![forbid(clippy::disallowed_types, clippy::disallowed_macros)]

use super::*;

const CLAUDE_UNRELATED_RESET: &str = r#"{"type":"result","is_error":true,"result":"MCP cache resets 9am (UTC)","session_id":"s-limit"}"#;

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
fn a_rate_limited_failure_carries_a_limit_from_stderr_without_changing_its_reason() {
    const LIMIT: &str = "You've hit your 5-hour limit · resets 5:30am (UTC)\n";
    let output = |stderr: &str| crate::agent::ProcessOutput {
        stdout: CLAUDE_REQUEST_FAILED.to_owned(),
        stderr: stderr.to_owned(),
        code: Some(1),
        timed_out: false,
        output_limited: false,
        duration: Duration::ZERO,
    };

    let limited = output(LIMIT);
    let outcome = crate::agent::claude::ClaudeCodeAdapter
        .parse(&limited)
        .expect("parses");
    assert_eq!(outcome.status, OutcomeStatus::RateLimited);
    let failure = evaluate_outcome(&outcome, &limited).expect("a rate limit is a failure");
    assert_eq!(failure.kind, FailureKind::RateLimited);
    assert_eq!(failure.reason, "pool rate-limited: request failed");
    assert_eq!(failure.feedback, None);
    assert_eq!(failure.limit_message(), Some(LIMIT));

    let quiet = output("warning: telemetry disabled\n");
    let outcome = fake_outcome(
        OutcomeStatus::RateLimited,
        Some("5-hour limit reached".to_owned()),
        "s0",
        None,
        Duration::ZERO,
    );
    let failure = evaluate_outcome(&outcome, &quiet).expect("a rate limit is a failure");
    assert_eq!(failure.reason, "pool rate-limited: 5-hour limit reached");
    assert_eq!(
        failure.limit_message(),
        None,
        "stderr that did not signal the limit is not carried"
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
    assert!(
        asked[0].question.affected_tasks == [TaskId::from("t1")]
            && asked[0].question.context.starts_with("Task `t1` — One\n"),
        "and it names and describes the task it holds: {:?}",
        asked[0].question
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
fn a_legacy_run_refuses_a_profiled_pool_at_preflight_rather_than_ignore_it() {
    let (_tree, repo) = temp_engine_repo("legacyprofile");
    seed(
        &repo,
        "## One\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        None,
    );
    let mut opts = options(&repo);
    opts.pools_path = Some(pools_file(
        &repo,
        "[pools.claude-max]\nkind = \"subscription-window\"\nagent = \"claude-code\"\n\
         profile = \"/profiles/personal\"\n",
    ));
    let source = fake(Effect::EditFile);

    let err = run_with(&opts, &source).expect_err("a legacy run cannot carry the profile");

    let text = err.to_string();
    assert!(
        matches!(err, UpstrokeError::Refused { .. }),
        "a refusal, not some other failure: {text}"
    );
    assert!(
        text.contains("`claude-max`") && text.contains("/profiles/personal"),
        "the refusal names the pool and the profile it would have ignored: {text}"
    );
    assert!(
        text.contains("execution topology (schema 4)"),
        "and where profiles do apply: {text}"
    );
    let branch = git_in(&repo, &["rev-parse", "--abbrev-ref", "HEAD"]);
    assert_eq!(branch.trim(), "main", "refused before branching");
    assert!(
        !crate::rundir::runs_root(&repo).exists(),
        "and before any run directory was created"
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
    assert_eq!(signals[0].reset_at, None, "{signals:?}");

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
fn a_rate_limit_that_names_its_reset_records_it_on_pool_exhausted() {
    let (_tree, repo) = temp_engine_repo("poolexhaustedreset");
    seed(
        &repo,
        "## One\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let pools = pools_file(&repo, CLAUDE_POOL);
    opts.pools_path = Some(pools.clone());
    let mut source = source(
        vec![Effect::RateLimited, Effect::EditFile],
        vec![ReviewBehavior::Pass],
    );
    source.adapter.worker_rate_limit = "You've hit your 5-hour limit · resets 5:30am (UTC)";
    let before = crate::util::unix_seconds_now();
    let report = run_with(&opts, &source).expect("run");
    let after = crate::util::unix_seconds_now();
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
    assert!(
        signals[0].detail.contains("resets 5:30am (UTC)"),
        "{signals:?}"
    );
    let reset_at = signals[0]
        .reset_at
        .as_deref()
        .expect("the failure text named its reset");
    assert!(reset_at.ends_with("T05:30:00Z"), "{reset_at}");
    assert_eq!(reset_at.len(), "1970-01-01T05:30:00Z".len(), "{reset_at}");
    assert!(
        crate::util::rfc3339_utc(before.saturating_sub(43_200)).as_str() <= reset_at
            && reset_at <= crate::util::rfc3339_utc(after + 43_200).as_str(),
        "{reset_at} is the 05:30 UTC nearest the signal, which fell between \
         {before} and {after}"
    );

    let signal_at = events
        .iter()
        .position(|e| matches!(e.body, EventBody::PoolExhausted { .. }))
        .expect("the signal is in the log");
    let mut warnings = Vec::new();
    let cfg = config::load(None, &repo, Some(&pools), &mut warnings).expect("pools");
    let at_the_signal = capacity::estimate(&cfg.pools, &capacity::observe(&events[..=signal_at]));
    assert_eq!(at_the_signal[0].remaining, capacity::Remaining::Exhausted);
    assert_eq!(
        at_the_signal[0].reset_at.as_deref(),
        Some(reset_at),
        "{}",
        at_the_signal[0].describe()
    );
}

#[test]
fn a_limit_on_stderr_beside_an_unrelated_result_records_its_reset_on_pool_exhausted() {
    let (_tree, repo) = temp_engine_repo("poolexhaustedstderrreset");
    seed(
        &repo,
        "## One\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    opts.pools_path = Some(pools_file(&repo, CLAUDE_POOL));
    let mut source = source(
        vec![Effect::RateLimited, Effect::EditFile],
        vec![ReviewBehavior::Pass],
    );
    // The fake worker echoes this through the native shell (see
    // `echo_operand`), so `-` stands in for the real message's `·`, which
    // `cmd` would write in the console's code page. The parser reads any
    // non-alphanumeric run between the limit and `resets`, and needs the
    // ` (UTC)` that `echo_operand` keeps.
    source.adapter.worker_limit_on_stderr =
        Some("You've hit your 5-hour limit - resets 5:30am (UTC)");
    let report = run_with(&opts, &source).expect("run");
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:?}");

    let events = events_of(&repo, &report.run_id);
    let reasons: Vec<&str> = events
        .iter()
        .filter_map(|e| match &e.body {
            EventBody::AttemptFinished { data, .. } => data
                .failure
                .as_ref()
                .filter(|f| f.kind == FailureKind::RateLimited)
                .map(|f| f.reason.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        reasons,
        ["pool rate-limited: request failed"],
        "stderr classified the attempt and the JSON result is still its reason"
    );

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
    assert_eq!(
        signals[0].detail, "pool rate-limited: request failed",
        "{signals:?}"
    );
    let reset_at = signals[0]
        .reset_at
        .as_deref()
        .expect("the limit on stderr named its reset");
    assert!(reset_at.ends_with("T05:30:00Z"), "{reset_at}");
    assert_eq!(reset_at.len(), "1970-01-01T05:30:00Z".len(), "{reset_at}");
}

#[test]
fn a_limit_on_stderr_outranks_an_unrelated_reset_in_the_result_on_pool_exhausted() {
    const REASON: &str = "pool rate-limited: MCP cache resets 9am (UTC)";
    assert_eq!(
        crate::agent::reset_at_from_limit_message(REASON, crate::util::unix_seconds_now()),
        None,
        "the reason's reset follows no limit, so it is not read even alone"
    );

    let (_tree, repo) = temp_engine_repo("poolexhaustedstderroutranksreason");
    seed(
        &repo,
        "## One\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    opts.pools_path = Some(pools_file(&repo, CLAUDE_POOL));
    let mut source = source(
        vec![Effect::RateLimited, Effect::EditFile],
        vec![ReviewBehavior::Pass],
    );
    source.adapter.worker_limit_result = CLAUDE_UNRELATED_RESET;
    source.adapter.worker_limit_on_stderr =
        Some("You've hit your 5-hour limit - resets 5:30am (UTC)");
    let report = run_with(&opts, &source).expect("run");
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:?}");

    let events = events_of(&repo, &report.run_id);
    let reasons: Vec<&str> = events
        .iter()
        .filter_map(|e| match &e.body {
            EventBody::AttemptFinished { data, .. } => data
                .failure
                .as_ref()
                .filter(|f| f.kind == FailureKind::RateLimited)
                .map(|f| f.reason.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        reasons,
        [REASON],
        "stderr classified the attempt and the JSON result is still its reason"
    );

    let signals: Vec<&events::PoolExhausted> = events
        .iter()
        .filter_map(|e| match &e.body {
            EventBody::PoolExhausted { data, .. } => Some(data),
            _ => None,
        })
        .collect();
    assert_eq!(signals.len(), 1, "{signals:?}");
    assert_eq!(signals[0].pool, "claude-max");
    assert_eq!(signals[0].detail, REASON, "{signals:?}");
    let reset_at = signals[0]
        .reset_at
        .as_deref()
        .expect("the limit on stderr named its reset");
    assert!(
        reset_at.ends_with("T05:30:00Z"),
        "the limit's own reset, not the reason's 09:00: {reset_at}"
    );
    assert_eq!(reset_at.len(), "1970-01-01T05:30:00Z".len(), "{reset_at}");
}

#[test]
fn an_unreadable_reset_on_stderr_is_not_replaced_by_the_results_on_pool_exhausted() {
    const LIMITED_RESULT: &str = r#"{"type":"result","is_error":true,"result":"You've hit your weekly limit - resets 9am (UTC)","session_id":"s-limit"}"#;
    for (name, result, reason) in [
        (
            "poolexhaustedstderrunreadable",
            CLAUDE_UNRELATED_RESET,
            "pool rate-limited: MCP cache resets 9am (UTC)",
        ),
        (
            "poolexhaustedstderrunreadablelimited",
            LIMITED_RESULT,
            "pool rate-limited: You've hit your weekly limit - resets 9am (UTC)",
        ),
    ] {
        let (_tree, repo) = temp_engine_repo(name);
        seed(
            &repo,
            "## One\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
            Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
        );
        let mut opts = options(&repo);
        opts.config_path = Some(repo.join("upstroke.toml"));
        opts.pools_path = Some(pools_file(&repo, CLAUDE_POOL));
        let mut source = source(
            vec![Effect::RateLimited, Effect::EditFile],
            vec![ReviewBehavior::Pass],
        );
        source.adapter.worker_limit_result = result;
        // `-` for the real message's `·`, as in the tests above.
        source.adapter.worker_limit_on_stderr =
            Some("You've hit your 5-hour limit - resets 5:30am (Europe/London)");
        let report = run_with(&opts, &source).expect("run");
        assert_eq!(report.outcome(), RunOutcome::Complete, "{report:?}");

        let events = events_of(&repo, &report.run_id);
        let reasons: Vec<&str> = events
            .iter()
            .filter_map(|e| match &e.body {
                EventBody::AttemptFinished { data, .. } => data
                    .failure
                    .as_ref()
                    .filter(|f| f.kind == FailureKind::RateLimited)
                    .map(|f| f.reason.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            reasons,
            [reason],
            "stderr classified the attempt and the JSON result is still its reason"
        );

        let signals: Vec<&events::PoolExhausted> = events
            .iter()
            .filter_map(|e| match &e.body {
                EventBody::PoolExhausted { data, .. } => Some(data),
                _ => None,
            })
            .collect();
        assert_eq!(signals.len(), 1, "{signals:?}");
        assert_eq!(signals[0].pool, "claude-max");
        assert_eq!(signals[0].detail, reason, "{signals:?}");
        assert_eq!(
            signals[0].reset_at, None,
            "the limit on stderr gave a reset in another zone, and no reset in the \
             result stands in for it: {signals:?}"
        );
    }
}

#[test]
fn a_reviewers_limit_on_stderr_beside_an_unrelated_result_records_its_reset_on_pool_exhausted() {
    let (_tree, repo) = temp_engine_repo("poolexhaustedreviewerstderr");
    seed(
        &repo,
        "## One\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    opts.pools_path = Some(pools_file(&repo, CLAUDE_POOL));
    let mut source = source(
        vec![Effect::EditFile],
        vec![ReviewBehavior::RateLimited, ReviewBehavior::Pass],
    );
    // `-` for the real message's `·`, as in the worker's test above.
    source.adapter.review_limit_on_stderr =
        Some("You've hit your 5-hour limit - resets 5:30am (UTC)");
    let report = run_with(&opts, &source).expect("run");
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:?}");

    let t1 = task(&report, "t1");
    assert_eq!(t1.attempts.len(), 2, "{report:?}");
    let refused = t1.attempts[0]
        .failure
        .as_ref()
        .expect("the first attempt's reviewer was refused");
    assert_eq!(refused.kind, FailureKind::RateLimited);
    assert_eq!(refused.origin, FailureOrigin::Reviewer);
    assert_eq!(
        refused.reason, "reviewer unavailable: request failed",
        "stderr classified the review and the JSON result is still its reason"
    );

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
    assert_eq!(
        signals[0].detail, "reviewer unavailable: request failed",
        "{signals:?}"
    );
    let reset_at = signals[0]
        .reset_at
        .as_deref()
        .expect("the reviewer's limit on stderr named its reset");
    assert!(reset_at.ends_with("T05:30:00Z"), "{reset_at}");
    assert_eq!(reset_at.len(), "1970-01-01T05:30:00Z".len(), "{reset_at}");
}

#[test]
fn a_reviewers_limit_in_the_result_past_the_reasons_head_records_its_reset_on_pool_exhausted() {
    let payload: serde_json::Value =
        serde_json::from_str(LIMIT_PAST_THE_HEAD).expect("the reviewer's result is JSON");
    let result = payload["result"]
        .as_str()
        .expect("the payload has a result");
    let limit = result
        .find("You've hit your 5-hour limit")
        .expect("the result holds the limit");
    assert!(
        limit > 400,
        "the limit starts past the reason's head: {limit}"
    );
    let reason = format!("reviewer unavailable: {}", crate::util::head(result, 400));
    assert_eq!(
        crate::agent::reset_at_from_limit_message(&reason, crate::util::unix_seconds_now()),
        None,
        "the head holds only the unrelated reset, so the reason alone gives none"
    );

    let (_tree, repo) = temp_engine_repo("poolexhaustedreviewerresult");
    seed(
        &repo,
        "## One\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    opts.pools_path = Some(pools_file(&repo, CLAUDE_POOL));
    let mut source = source(
        vec![Effect::EditFile],
        vec![ReviewBehavior::RateLimited, ReviewBehavior::Pass],
    );
    source.adapter.review_limit_result = LIMIT_PAST_THE_HEAD;
    source.adapter.review_limit_on_stderr = Some("request failed");
    let report = run_with(&opts, &source).expect("run");
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:?}");

    let t1 = task(&report, "t1");
    assert_eq!(t1.attempts.len(), 2, "{report:?}");
    let refused = t1.attempts[0]
        .failure
        .as_ref()
        .expect("the first attempt's reviewer was refused");
    assert_eq!(refused.kind, FailureKind::RateLimited);
    assert_eq!(refused.origin, FailureOrigin::Reviewer);
    assert_eq!(
        refused.reason, reason,
        "the displayed reason is still the result's 400-byte head"
    );

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
    assert_eq!(
        signals[0].detail,
        crate::util::head(&reason, 400),
        "{signals:?}"
    );
    let reset_at = signals[0]
        .reset_at
        .as_deref()
        .expect("the reviewer's limit in its result named its reset");
    assert!(reset_at.ends_with("T05:30:00Z"), "{reset_at}");
    assert_eq!(reset_at.len(), "1970-01-01T05:30:00Z".len(), "{reset_at}");
}

#[test]
fn a_reviewers_quota_limit_in_the_result_outranks_a_generic_limit_on_stderr_on_pool_exhausted() {
    const GENERIC_LIMIT: &str = "HTTP 429: rate limit";
    assert!(
        crate::agent::looks_rate_limited(GENERIC_LIMIT),
        "the stderr signals a limit"
    );
    assert!(
        !crate::agent::names_a_quota_limit(GENERIC_LIMIT),
        "the stderr names no quota limit"
    );
    let payload: serde_json::Value =
        serde_json::from_str(LIMIT_PAST_THE_HEAD).expect("the reviewer's result is JSON");
    let result = payload["result"]
        .as_str()
        .expect("the payload has a result");
    let limit = result
        .find("You've hit your 5-hour limit")
        .expect("the result holds the limit");
    assert!(
        limit > 400,
        "the limit starts past the reason's head: {limit}"
    );
    let reason = format!("reviewer unavailable: {}", crate::util::head(result, 400));
    assert!(
        !crate::agent::names_a_quota_limit(&reason),
        "the reason's head names no quota limit, so only the whole result can give the reset"
    );

    let (_tree, repo) = temp_engine_repo("poolexhaustedreviewergeneric");
    seed(
        &repo,
        "## One\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    opts.pools_path = Some(pools_file(&repo, CLAUDE_POOL));
    let mut source = source(
        vec![Effect::EditFile],
        vec![ReviewBehavior::RateLimited, ReviewBehavior::Pass],
    );
    source.adapter.review_limit_result = LIMIT_PAST_THE_HEAD;
    source.adapter.review_limit_on_stderr = Some(GENERIC_LIMIT);
    let report = run_with(&opts, &source).expect("run");
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:?}");

    let t1 = task(&report, "t1");
    assert_eq!(t1.attempts.len(), 2, "{report:?}");
    let refused = t1.attempts[0]
        .failure
        .as_ref()
        .expect("the first attempt's reviewer was refused");
    assert_eq!(refused.kind, FailureKind::RateLimited);
    assert_eq!(refused.origin, FailureOrigin::Reviewer);
    assert_eq!(
        refused.reason, reason,
        "the displayed reason is still the result's 400-byte head"
    );

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
    assert_eq!(
        signals[0].detail,
        crate::util::head(&reason, 400),
        "{signals:?}"
    );
    let reset_at = signals[0]
        .reset_at
        .as_deref()
        .expect("the quota limit in the result, not the generic stderr, named the reset");
    assert!(reset_at.ends_with("T05:30:00Z"), "{reset_at}");
    assert_eq!(reset_at.len(), "1970-01-01T05:30:00Z".len(), "{reset_at}");
}

const MONTH_NAMES: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

fn deferral_rate_limit(now: u64) -> (String, String) {
    let reset = (now + 3 * 3_600) / 60 * 60;
    let (_, month, day) = crate::util::civil_from_days(
        i64::try_from(reset / 86_400).expect("the reset's day fits an i64"),
    );
    let month = MONTH_NAMES[usize::try_from(month - 1).expect("a month runs from 1 to 12")];
    let minute_of_day = reset % 86_400 / 60;
    let (hour, minute) = (minute_of_day / 60, minute_of_day % 60);
    let minutes = if minute == 0 {
        String::new()
    } else {
        format!(":{minute:02}")
    };
    let text = format!(
        "You've hit your weekly limit · resets {month} {day}, {}{minutes}{} (UTC)",
        (hour + 11) % 12 + 1,
        if hour < 12 { "am" } else { "pm" },
    );
    (text, crate::util::rfc3339_utc(reset))
}

#[test]
fn the_deferral_rate_limit_text_names_one_reset_at_any_time_of_day() {
    let at = |year, month, day, second_of_day: i64| {
        u64::try_from(crate::util::days_from_civil(year, month, day) * 86_400 + second_of_day)
            .expect("an instant after 1970")
    };
    assert_eq!(
        deferral_rate_limit(at(2026, 11, 14, 14 * 3_600)),
        (
            "You've hit your weekly limit · resets Nov 14, 5pm (UTC)".to_owned(),
            "2026-11-14T17:00:00Z".to_owned()
        ),
        "built three hours before it, the text the deferral tests used to carry"
    );
    for built in [
        at(2026, 11, 11, 0),
        at(2027, 5, 16, 5 * 3_600 - 1),
        at(2027, 5, 16, 5 * 3_600),
        at(2026, 12, 31, 21 * 3_600 + 30 * 60),
        at(2028, 2, 29, 9 * 3_600),
    ] {
        let (text, reset) = deferral_rate_limit(built);
        for second in 0..86_400 {
            assert_eq!(
                crate::agent::reset_at_from_limit_message(&text, built + second).as_deref(),
                Some(reset.as_str()),
                "{text}, built at {built}, read {second}s later"
            );
        }
    }
}

#[test]
fn a_deferral_rate_limit_keeps_the_reset_pool_exhausted_recorded() {
    let (_tree, repo) = temp_engine_repo("poolexhaustedkeepsreset");
    seed(
        &repo,
        "## One\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let pools = pools_file(&repo, CLAUDE_POOL);
    opts.pools_path = Some(pools.clone());
    let mut source = source(
        vec![Effect::RateLimited, Effect::RateLimited, Effect::EditFile],
        vec![ReviewBehavior::Pass],
    );
    let (limit, _) = deferral_rate_limit(crate::util::unix_seconds_now());
    source.adapter.worker_rate_limit = limit.leak();
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
    assert_eq!(signals.len(), 1, "one outage, one signal: {signals:?}");
    let reset_at = signals[0]
        .reset_at
        .clone()
        .expect("the failure text named its reset");

    let rate_limited: Vec<usize> = events
        .iter()
        .enumerate()
        .filter(|(_, e)| match &e.body {
            EventBody::AttemptFinished { data, .. } => data
                .failure
                .as_ref()
                .is_some_and(|f| f.kind == FailureKind::RateLimited),
            _ => false,
        })
        .map(|(at, _)| at)
        .collect();
    assert_eq!(rate_limited.len(), 2, "two rate-limited attempts");
    let signal_at = events
        .iter()
        .position(|e| matches!(e.body, EventBody::PoolExhausted { .. }))
        .expect("the signal is in the log");
    assert!(
        rate_limited[0] < signal_at && signal_at < rate_limited[1],
        "the second rate limit falls after the signal it must not erase"
    );

    let mut warnings = Vec::new();
    let cfg = config::load(None, &repo, Some(&pools), &mut warnings).expect("pools");
    let after_the_deferral =
        capacity::estimate(&cfg.pools, &capacity::observe(&events[..=rate_limited[1]]));
    assert_eq!(
        after_the_deferral[0].remaining,
        capacity::Remaining::Exhausted
    );
    assert_eq!(
        after_the_deferral[0].reset_at.as_deref(),
        Some(reset_at.as_str()),
        "{}",
        after_the_deferral[0].describe()
    );

    let settled = capacity::estimate(&cfg.pools, &capacity::observe(&events));
    assert_ne!(
        settled[0].remaining,
        capacity::Remaining::Exhausted,
        "{}",
        settled[0].describe()
    );
}

#[test]
fn a_different_later_reset_writes_one_more_pool_exhausted() {
    let (_tree, repo) = temp_engine_repo("poolexhaustedlaterreset");
    seed(
        &repo,
        "## One\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let pools = pools_file(&repo, CLAUDE_POOL);
    opts.pools_path = Some(pools.clone());
    let mut source = source(
        vec![Effect::RateLimited, Effect::RateLimited, Effect::EditFile],
        vec![ReviewBehavior::Pass],
    );
    source.adapter.worker_rate_limits = vec![
        "You've hit your 5-hour limit · resets 5:30am (UTC)",
        "You've hit your weekly limit · resets Nov 14, 5pm (UTC)",
    ];
    let report = run_with(&opts, &source).expect("run");
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:?}");

    let events = events_of(&repo, &report.run_id);
    let signals: Vec<(usize, &events::PoolExhausted)> = events
        .iter()
        .enumerate()
        .filter_map(|(at, e)| match &e.body {
            EventBody::PoolExhausted { data, .. } => Some((at, data)),
            _ => None,
        })
        .collect();
    assert_eq!(
        signals.len(),
        2,
        "a later refusal naming a different reset is one more fact: {signals:?}"
    );
    assert!(
        signals
            .iter()
            .all(|(_, signal)| signal.pool == "claude-max"),
        "{signals:?}"
    );
    let five_hour = signals[0]
        .1
        .reset_at
        .clone()
        .expect("the five-hour text named its reset");
    assert!(five_hour.ends_with("T05:30:00Z"), "{five_hour}");
    let weekly = signals[1]
        .1
        .reset_at
        .clone()
        .expect("the weekly text named its reset");
    assert!(weekly.ends_with("-11-14T17:00:00Z"), "{weekly}");
    assert!(signals[1].1.detail.contains("weekly limit"), "{signals:?}");

    let rate_limited: Vec<usize> = events
        .iter()
        .enumerate()
        .filter(|(_, e)| match &e.body {
            EventBody::AttemptFinished { data, .. } => data
                .failure
                .as_ref()
                .is_some_and(|f| f.kind == FailureKind::RateLimited),
            _ => false,
        })
        .map(|(at, _)| at)
        .collect();
    assert_eq!(rate_limited.len(), 2, "two rate-limited attempts");
    assert!(
        rate_limited[0] < signals[0].0
            && signals[0].0 < rate_limited[1]
            && rate_limited[1] < signals[1].0,
        "each signal follows the refusal it was read from"
    );

    let mut warnings = Vec::new();
    let cfg = config::load(None, &repo, Some(&pools), &mut warnings).expect("pools");
    let through_both = capacity::estimate(&cfg.pools, &capacity::observe(&events[..=signals[1].0]));
    assert_eq!(through_both[0].remaining, capacity::Remaining::Exhausted);
    assert_eq!(
        through_both[0].reset_at.as_deref(),
        Some(weekly.as_str()),
        "the latest signal's reset, not the five-hour one it replaced: {}",
        through_both[0].describe()
    );
}

#[test]
fn a_rate_limit_after_a_success_retired_its_pool_writes_pool_exhausted_again() {
    let (_tree, repo) = temp_engine_repo("poolexhaustedafterretire");
    seed(
        &repo,
        "## One\n<!-- upstroke: id=t1 kind=implement depends= -->\n\n\
             ## Two\n<!-- upstroke: id=t2 kind=implement depends=t1 -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let pools = pools_file(&repo, CLAUDE_POOL);
    opts.pools_path = Some(pools.clone());
    let mut source = source(
        vec![
            Effect::RateLimited,
            Effect::EditFile,
            Effect::RateLimited,
            Effect::EditFile,
        ],
        vec![ReviewBehavior::Pass],
    );
    let (limit, reset) = deferral_rate_limit(crate::util::unix_seconds_now());
    source.adapter.worker_rate_limit = limit.leak();
    let report = run_with(&opts, &source).expect("run");
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:?}");

    let events = events_of(&repo, &report.run_id);
    let signals: Vec<(usize, &events::PoolExhausted)> = events
        .iter()
        .enumerate()
        .filter_map(|(at, e)| match &e.body {
            EventBody::PoolExhausted { data, .. } => Some((at, data)),
            _ => None,
        })
        .collect();
    assert_eq!(
        signals.len(),
        2,
        "the success between the outages retired the first, so the second is news: {signals:?}"
    );
    let first = signals[0]
        .1
        .reset_at
        .clone()
        .expect("the weekly text named its reset");
    assert_eq!(first, reset, "{first}");
    assert_eq!(
        signals[1].1.reset_at.as_deref(),
        Some(first.as_str()),
        "the same reset as before, which the remembered one used to swallow: {signals:?}"
    );
    assert_eq!(signals[0].1.pool, "claude-max");
    assert_eq!(signals[1].1.pool, "claude-max");

    let finished: Vec<(usize, &str, bool)> = events
        .iter()
        .enumerate()
        .filter_map(|(at, e)| match &e.body {
            EventBody::AttemptFinished { task, data, .. } => Some((
                at,
                task.as_str(),
                data.failure
                    .as_ref()
                    .is_some_and(|f| f.kind == FailureKind::RateLimited),
            )),
            _ => None,
        })
        .collect();
    let shape: Vec<(&str, bool)> = finished
        .iter()
        .map(|(_, task, limited)| (*task, *limited))
        .collect();
    assert_eq!(
        shape,
        [("t1", true), ("t1", false), ("t2", true), ("t2", false)],
        "t1 is refused then served, and t2 is refused on the same pool after"
    );
    let retired_at = finished[1].0;
    assert!(
        signals[0].0 < retired_at && retired_at < finished[2].0 && finished[2].0 < signals[1].0,
        "the second signal follows t2's refusal, after t1's success retired the first"
    );

    let mut warnings = Vec::new();
    let cfg = config::load(None, &repo, Some(&pools), &mut warnings).expect("pools");
    let after_success = capacity::estimate(&cfg.pools, &capacity::observe(&events[..=retired_at]));
    assert_ne!(
        after_success[0].remaining,
        capacity::Remaining::Exhausted,
        "{}",
        after_success[0].describe()
    );
    let through_second =
        capacity::estimate(&cfg.pools, &capacity::observe(&events[..=signals[1].0]));
    assert_eq!(through_second[0].remaining, capacity::Remaining::Exhausted);
    assert_eq!(
        through_second[0].reset_at.as_deref(),
        Some(first.as_str()),
        "the fold reports the reset again, not \"unknown\": {}",
        through_second[0].describe()
    );
}

#[test]
fn another_agents_rate_limit_naming_a_reset_records_none_on_pool_exhausted() {
    const COPILOT_RESET: &str = "premium request allowance exhausted; you've hit your weekly limit · resets Dec 1, 12am (UTC)";
    assert!(
        crate::agent::reset_at_from_limit_message(COPILOT_RESET, crate::util::unix_seconds_now())
            .is_some(),
        "the text alone parses, so only the agent can be what keeps its reset out"
    );
    let (_tree, repo) = temp_engine_repo("poolexhaustedotheragent");
    seed(
        &repo,
        "## One\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some(
            "[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n\n\
             [[pins]]\ntier = \"small\"\nagent = \"copilot\"\nmodel = \"gpt-5-mini\"\n",
        ),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    opts.pools_path = Some(pools_file(
        &repo,
        "[pools.copilot-window]\nkind = \"subscription-window\"\nagent = \"copilot\"\n\
         sources = [\"signals\"]\n",
    ));
    let mut source = cross_vendor(
        vec![Effect::EditFile],
        vec![ReviewBehavior::Pass],
        vec![ReviewBehavior::Pass],
    );
    let copilot = source.copilot.as_mut().expect("this source has a copilot");
    copilot.effects = vec![Effect::RateLimited, Effect::EditFile];
    copilot.worker_rate_limit = COPILOT_RESET;
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
    assert_eq!(signals[0].pool, "copilot-window");
    assert_eq!(signals[0].agent, "copilot");
    assert!(
        signals[0].detail.contains("resets Dec 1, 12am (UTC)"),
        "{signals:?}"
    );
    assert_eq!(
        signals[0].reset_at, None,
        "only Claude Code's wording is read for a reset: {signals:?}"
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
        questions_in_log: false,
        unattributed_cost_incomplete: false,
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
        !legacy_status(&seen).running,
        "a run that recorded its finish is not running"
    );
    assert!(legacy_status(&seen).held, "though a resume does hold it");
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
