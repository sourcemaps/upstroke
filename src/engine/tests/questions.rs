//! Extended notes: `docs/internals/engine/tests/questions.md`
// Allowlist placement: the funnel section of `effects/allowlist.toml`.
#![allow(clippy::disallowed_methods)]
#![forbid(clippy::disallowed_types, clippy::disallowed_macros)]

use super::*;

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

const QUESTION_PAYLOAD_KILL_CHILD: &str = "engine::tests::question_payload_kill_child";

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

/// P12-W13. A parked schema-4 question has no payload file: `status` names it,
/// `show` renders it from the fold by an id prefix, `answer` records it once and
/// refuses a second, the resume ingests it from the file the run polls, and an
/// answer after that refuses the settled question as no longer open.
#[test]
fn a_parked_schema_4_question_is_shown_answered_once_and_ingested_on_resume() {
    let (_tree, repo) = temp_engine_repo("topology-answer");
    let source = SharedTopologyStubs(std::sync::Arc::new(TopologyStub::new(
        TOPOLOGY_STUB_EDIT,
        1,
    )));
    let parked = run_with(&topology_options(&repo), &source).expect("the run parks");
    assert_eq!(parked.outcome(), RunOutcome::Parked, "{parked:#?}");
    until_released(&repo, &parked.run_id);
    let [asked] = parked.questions.as_slice() else {
        panic!("one question: {parked:#?}");
    };
    let id = asked.question.id.to_string();
    let public = rundir::public_dir(&repo, &parked.run_id);
    assert!(
        !public.join("questions").exists(),
        "a schema-4 run writes no question payload file"
    );
    let summary = parked.render();
    assert!(
        !summary.contains("payloads:"),
        "and its summary names no payload directory: {summary}"
    );
    assert!(
        summary.contains("upstroke answer <question id>"),
        "{summary}"
    );

    let rendered = crate::status::render(
        &crate::status::load(&repo, Some(&parked.run_id)).expect("status reads the parked run"),
    );
    assert!(
        rendered.contains(&format!("upstroke answer {id}")),
        "{rendered}"
    );
    let prefix = &id[..id.len() - 6];
    let shown = crate::answer::show(&repo, prefix).expect("show finds it by prefix");
    assert!(shown.contains("widget formats"), "{shown}");

    let recorded = crate::answer::answer(
        &repo,
        &id,
        crate::answer::Reply::Text("the first".to_owned()),
    )
    .expect("the answer is recorded");
    assert_eq!(recorded.run_id, parked.run_id);
    assert_eq!(recorded.question_id, id);
    assert!(!recorded.run_is_live, "nothing drives the parked run");
    let again = crate::answer::answer(&repo, &id, crate::answer::Reply::Text("again".to_owned()))
        .expect_err("a question is answered once");
    assert!(again.to_string().contains("already answered"), "{again}");

    let mut resume = resume_options(&repo, &parked.run_id);
    resume.private_root = None;
    let report = resume_with(&resume, &source).expect("the resume completes the run");
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:#?}");
    let log = fs::read_to_string(public.join("events.jsonl")).expect("the log");
    assert!(
        log.lines()
            .any(|line| line.contains("\"question_answered\"") && line.contains(&id)),
        "the resume ingested the answer"
    );
    until_released(&repo, &parked.run_id);
    let late = crate::answer::answer(&repo, &id, crate::answer::Reply::Text("late".to_owned()))
        .expect_err("the settled question takes no answer");
    assert!(late.to_string().contains("no longer open"), "{late}");
}

#[test]
fn a_schema_4_run_and_its_resume_leave_the_checkout_and_print_no_way_back_from_it() {
    let checkout = |repo: &Path| {
        (
            git_in(repo, &["symbolic-ref", "HEAD"]).trim().to_owned(),
            git_in(repo, &["rev-parse", "HEAD"]).trim().to_owned(),
        )
    };
    let names_the_branch_alone = |summary: &str, branch: &str, when: &str| {
        assert!(
            summary
                .lines()
                .any(|line| line == format!("branch: {branch}")),
            "{when}: the summary names the run's branch: {summary}"
        );
        assert!(
            !summary.contains("return with") && !summary.contains("git switch"),
            "{when}: nothing switched the checkout, so nothing is returned from: {summary}"
        );
    };

    let (_tree, repo) = temp_engine_repo("topology-no-return");
    let before = checkout(&repo);
    assert_eq!(
        before.0, "refs/heads/main",
        "the premise: the operator is on `main`"
    );
    let source = SharedTopologyStubs(std::sync::Arc::new(TopologyStub::new(
        TOPOLOGY_STUB_EDIT,
        1,
    )));
    let parked = run_with(&topology_options(&repo), &source).expect("the run parks");
    assert_eq!(parked.outcome(), RunOutcome::Parked, "{parked:#?}");
    assert_eq!(
        checkout(&repo),
        before,
        "the fresh run left the checkout where it was"
    );
    names_the_branch_alone(&parked.render(), &parked.branch, "fresh");

    until_released(&repo, &parked.run_id);
    let [asked] = parked.questions.as_slice() else {
        panic!("one question: {parked:#?}");
    };
    crate::answer::answer(
        &repo,
        &asked.question.id.to_string(),
        crate::answer::Reply::Text("the first".to_owned()),
    )
    .expect("the answer is recorded");
    let mut resume = resume_options(&repo, &parked.run_id);
    resume.private_root = None;
    let resumed = resume_with(&resume, &source).expect("the resume completes the run");
    assert_eq!(resumed.outcome(), RunOutcome::Complete, "{resumed:#?}");
    assert_eq!(
        checkout(&repo),
        before,
        "the resume left the checkout where it was"
    );
    names_the_branch_alone(&resumed.render(), &resumed.branch, "resumed");
    assert_merged_on_the_integration_branch(&repo, &resumed);

    let (_legacy_tree, legacy) = temp_engine_repo("legacy-return-hint");
    let report =
        run_with(&options(&legacy), &fake(Effect::EditFile)).expect("the legacy run completes");
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:#?}");
    assert_eq!(
        git_in(&legacy, &["rev-parse", "--abbrev-ref", "HEAD"]).trim(),
        report.branch,
        "the control: a legacy run checks its branch out"
    );
    let summary = report.render();
    assert!(
        summary
            .lines()
            .any(|line| line == format!("branch: {} (return with: git switch -)", report.branch)),
        "{summary}"
    );
    git_in(&legacy, &["switch", "-"]);
    assert_eq!(
        git_in(&legacy, &["rev-parse", "--abbrev-ref", "HEAD"]).trim(),
        "main",
        "and the instruction it prints takes the operator back to where the run began"
    );
}

/// A committed legacy (schema-3) run whose public half holds one open question
/// file, as `answer`'s own suite seeds one: the first line is written by hand, so
/// the fixture pins the wire rather than agreeing with the writer.
fn legacy_question_run(repo: &Path, run_id: &str, question_id: &str) {
    let public = rundir::public_dir(repo, run_id);
    let questions = public.join("questions");
    fs::create_dir_all(&questions).expect("the legacy run's questions directory");
    fs::write(
        public.join("events.jsonl"),
        format!(
            "{{\"ts\":\"2026-08-20T00:00:00Z\",\"event\":\"run_started\",\
             \"data\":{{\"schema\":3,\"run_id\":\"{run_id}\"}}}}\n"
        ),
    )
    .expect("the legacy run's committed first line");
    let record = QuestionRecord::open(Question {
        id: QuestionId::from(question_id),
        kind: QuestionKind::Unblock,
        affected_tasks: vec![TaskId::from("t1")],
        context: format!("the legacy question {question_id}"),
        options: vec!["retry".to_owned(), interaction::GIVE_UP_OPTION.to_owned()],
    });
    interaction::write_question(&questions, &record).expect("the legacy question file");
}

/// Every answer file under every run directory of `repo`.
fn answer_files(repo: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for run_id in rundir::run_dir_names(repo) {
        let Ok(entries) = fs::read_dir(rundir::public_dir(repo, &run_id).join("answers")) else {
            continue;
        };
        files.extend(entries.flatten().map(|entry| entry.path()));
    }
    files
}

/// `answer` and `show` for one typed id, as the one result both refuse with.
fn answer_and_show(repo: &Path, wanted: &str) -> [Result<String, UpstrokeError>; 2] {
    [
        crate::answer::answer(
            repo,
            wanted,
            crate::answer::Reply::Text("the first".to_owned()),
        )
        .map(|answered| answered.question_id),
        crate::answer::show(repo, wanted),
    ]
}

/// PR12-ANSWER-CROSS-SCHEMA-PREFIX (root, 7021b42d). A prefix two legacy
/// questions share with a parked schema-4 question selects none of them:
/// `answer` and `show` refuse it, naming all three, and write nothing; so does a
/// prefix one legacy question shares with it. Each full id still answers, and a
/// prefix only one question carries is still that question's.
///
/// `L` is not a Crockford base32 digit, so no question id a run mints continues
/// a prefix with it: each legacy id below leaves the schema-4 id exactly where
/// its `LEGACY` begins.
#[test]
fn a_prefix_two_legacy_questions_share_with_a_schema_4_question_selects_none_of_them() {
    let (_tree, repo) = temp_engine_repo("topology-answer-cross-prefix");
    let source = SharedTopologyStubs(std::sync::Arc::new(TopologyStub::new(
        TOPOLOGY_STUB_EDIT,
        1,
    )));
    let parked = run_with(&topology_options(&repo), &source).expect("the run parks");
    assert_eq!(parked.outcome(), RunOutcome::Parked, "{parked:#?}");
    until_released(&repo, &parked.run_id);
    let [asked] = parked.questions.as_slice() else {
        panic!("one question: {parked:#?}");
    };
    let id = asked.question.id.to_string();
    let prefix = &id[..id.len() - 6];
    let first = format!("{prefix}LEGACY-A");
    let second = format!("{prefix}LEGACY-B");
    let near = &id[..id.len() - 2];
    let third = format!("{near}LEGACY-C");
    legacy_question_run(&repo, "00LEGACYA", &first);
    legacy_question_run(&repo, "00LEGACYB", &second);

    for refused in answer_and_show(&repo, prefix) {
        let message = refused
            .expect_err("a prefix three questions share selects none")
            .to_string();
        assert!(message.contains("matches 3 questions"), "{message}");
        for named in [&id, &first, &second] {
            assert!(message.contains(named.as_str()), "{named}: {message}");
        }
    }
    legacy_question_run(&repo, "00LEGACYC", &third);
    for refused in answer_and_show(&repo, near) {
        let message = refused
            .expect_err("a prefix a legacy and a schema-4 question share selects neither")
            .to_string();
        assert!(message.contains("matches 2 questions"), "{message}");
        for named in [&id, &third] {
            assert!(message.contains(named.as_str()), "{named}: {message}");
        }
    }
    for refused in answer_and_show(&repo, &format!("{prefix}LEGACY-")) {
        let message = refused
            .expect_err("two legacy questions, as today")
            .to_string();
        assert!(message.contains("matches 2 questions"), "{message}");
    }
    assert_eq!(
        answer_files(&repo),
        Vec::<PathBuf>::new(),
        "a refused id wrote an answer"
    );

    let shown = crate::answer::show(&repo, &id[..id.len() - 1])
        .expect("a prefix only the schema-4 question carries");
    assert!(shown.contains("widget formats"), "{shown}");
    let shown = crate::answer::show(&repo, &id).expect("the schema-4 question by its full id");
    assert!(shown.contains("widget formats"), "{shown}");
    for legacy in [&first, &second, &third] {
        let shown = crate::answer::show(&repo, legacy).expect("a legacy question by its full id");
        assert!(
            shown.contains(&format!("the legacy question {legacy}")),
            "{shown}"
        );
    }
    let recorded = crate::answer::answer(
        &repo,
        &first,
        crate::answer::Reply::Text("the first".to_owned()),
    )
    .expect("a legacy question answered by its full id");
    assert_eq!(
        (recorded.run_id.as_str(), recorded.question_id.as_str()),
        ("00LEGACYA", first.as_str())
    );
    let recorded = crate::answer::answer(
        &repo,
        &id,
        crate::answer::Reply::Text("the first".to_owned()),
    )
    .expect("the schema-4 question answered by its full id");
    assert_eq!(
        (recorded.run_id.as_str(), recorded.question_id.as_str()),
        (parked.run_id.as_str(), id.as_str())
    );
    let message = crate::answer::show(&repo, prefix)
        .expect_err("an answered question is still a candidate")
        .to_string();
    assert!(message.contains("matches 4 questions"), "{message}");
    for named in [&id, &first, &second, &third] {
        assert!(message.contains(named.as_str()), "{named}: {message}");
    }
}

/// PR12-ANSWER-CROSS-SCHEMA-PREFIX, the inverse: a prefix two parked schema-4
/// questions share selects neither, and once a legacy question shares it too it
/// selects none of the three. Each full id still answers.
#[cfg(unix)]
#[test]
fn a_prefix_one_legacy_question_shares_with_two_schema_4_questions_selects_none_of_them() {
    let (_tree, repo) = temp_engine_repo("topology-answer-cross-inverse");
    seed(
        &repo,
        TWO_INDEPENDENT_TASKS,
        Some("[engine]\nmax_parallel = 2\n"),
    );
    let source = SharedTopologyStubs(std::sync::Arc::new(TopologyStub::new(
        TOPOLOGY_STUB_EDIT,
        2,
    )));
    let parked = run_with(&topology_options(&repo), &source).expect("the run parks");
    assert_eq!(parked.outcome(), RunOutcome::Parked, "{parked:#?}");
    until_released(&repo, &parked.run_id);
    let [one, two] = parked.questions.as_slice() else {
        panic!("both tasks asked: {parked:#?}");
    };
    let (one, two) = (one.question.id.to_string(), two.question.id.to_string());
    let shared: String = one
        .chars()
        .zip(two.chars())
        .take_while(|(left, right)| left == right)
        .map(|(left, _)| left)
        .collect();
    assert!(
        shared.starts_with("q-") && shared.len() < one.len().min(two.len()),
        "the fixture's premise: two ids that share a proper prefix ({one}, {two})"
    );

    for refused in answer_and_show(&repo, &shared) {
        let message = refused
            .expect_err("a prefix two schema-4 questions share selects neither")
            .to_string();
        assert!(message.contains("matches 2 questions"), "{message}");
    }
    let legacy = format!("{shared}LEGACY");
    legacy_question_run(&repo, "00LEGACY", &legacy);
    for refused in answer_and_show(&repo, &shared) {
        let message = refused
            .expect_err("a prefix three questions share selects none")
            .to_string();
        assert!(message.contains("matches 3 questions"), "{message}");
        for named in [&one, &two, &legacy] {
            assert!(message.contains(named.as_str()), "{named}: {message}");
        }
    }
    assert_eq!(
        answer_files(&repo),
        Vec::<PathBuf>::new(),
        "a refused id wrote an answer"
    );
    for full in [&one, &two] {
        let shown = crate::answer::show(&repo, full).expect("a schema-4 question by its full id");
        assert!(shown.contains("widget formats"), "{shown}");
    }
    let shown = crate::answer::show(&repo, &legacy).expect("the legacy question by its full id");
    assert!(
        shown.contains(&format!("the legacy question {legacy}")),
        "{shown}"
    );
}

/// PR12-ANSWER-CROSS-SCHEMA-PREFIX, the unread census: a schema-4 run whose log
/// cannot be observed is carried into the decision rather than dropped from it,
/// so a prefix that only a legacy question carries among the runs that could be
/// read is refused, naming the run that could not, and nothing is written. The
/// legacy question's full id still answers. The log is made unobservable the way
/// a resume's refusal witness makes it unresumable: the frozen plan no longer
/// digests to what the record says.
#[test]
fn a_schema_4_run_that_cannot_be_read_keeps_a_lone_legacy_prefix_from_answering() {
    let (_tree, repo) = temp_engine_repo("topology-answer-cross-unread");
    let source = SharedTopologyStubs(std::sync::Arc::new(TopologyStub::new(
        TOPOLOGY_STUB_EDIT,
        1,
    )));
    let parked = run_with(&topology_options(&repo), &source).expect("the run parks");
    assert_eq!(parked.outcome(), RunOutcome::Parked, "{parked:#?}");
    until_released(&repo, &parked.run_id);
    let [asked] = parked.questions.as_slice() else {
        panic!("one question: {parked:#?}");
    };
    let id = asked.question.id.to_string();
    let legacy = "q-LEGACY-ONLY";
    legacy_question_run(&repo, "00LEGACY", legacy);
    let shown =
        crate::answer::show(&repo, "q-LEGACY").expect("a lone legacy prefix, every run read");
    assert!(
        shown.contains("the legacy question q-LEGACY-ONLY"),
        "{shown}"
    );

    let plan = rundir::public_dir(&repo, &parked.run_id).join("plan.normalized.json");
    let frozen = fs::read(&plan).expect("the frozen plan");
    let mut altered = frozen.clone();
    altered.extend_from_slice(b" ");
    fs::write(&plan, &altered).expect("an altered frozen plan");
    assert!(
        crate::engine::topology::observe::observe(&repo, &parked.run_id).is_err(),
        "the premise: the schema-4 log cannot be observed"
    );
    for refused in answer_and_show(&repo, "q-LEGACY") {
        let message = refused
            .expect_err("a lone legacy prefix beside a run that could not be read")
            .to_string();
        assert!(
            message
                .contains("matches one question (q-LEGACY-ONLY) among the runs that could be read"),
            "{message}"
        );
        assert!(
            message.contains(&format!(
                "the log of run `{}` could not be read",
                parked.run_id
            )),
            "{message}"
        );
    }
    let message = crate::answer::show(&repo, &id)
        .expect_err("a question in a run that could not be read is not found")
        .to_string();
    assert!(message.contains("no question with that id"), "{message}");
    assert!(message.contains(&parked.run_id), "{message}");
    assert_eq!(
        answer_files(&repo),
        Vec::<PathBuf>::new(),
        "a refused id wrote an answer"
    );
    let recorded = crate::answer::answer(
        &repo,
        legacy,
        crate::answer::Reply::Text("the first".to_owned()),
    )
    .expect("the legacy question's full id still answers");
    assert_eq!(recorded.run_id, "00LEGACY");

    fs::write(&plan, &frozen).expect("the frozen plan restored");
    let shown = crate::answer::show(&repo, &id[..id.len() - 6])
        .expect("read again, the schema-4 question answers to its prefix");
    assert!(shown.contains("widget formats"), "{shown}");
}

#[test]
fn a_run_whose_initial_log_read_fails_after_the_census_keeps_a_shared_prefix_from_answering() {
    let (_tree, repo) = temp_engine_repo("topology-answer-unread-initial");
    let mut parked = Vec::new();
    for _ in 0..2 {
        let stub = ObservedStub::new(1);
        let report =
            run_with(&topology_options(&repo), &BorrowedObserved(&stub)).expect("the run parks");
        assert_eq!(report.outcome(), RunOutcome::Parked, "{report:#?}");
        until_released(&repo, &report.run_id);
        let [asked] = report.questions.as_slice() else {
            panic!("one question: {report:#?}");
        };
        let id = asked.question.id.to_string();
        parked.push((report.run_id, id));
    }
    let [(unread_run, unread_id), (survivor_run, survivor_id)] = parked.as_slice() else {
        panic!("two parked runs: {parked:?}");
    };
    let shared: String = unread_id
        .chars()
        .zip(survivor_id.chars())
        .take_while(|(left, right)| left == right)
        .map(|(left, _)| left)
        .collect();
    assert!(
        shared.starts_with("q-") && shared.len() < unread_id.len().min(survivor_id.len()),
        "the fixture's premise: two ids that share a proper prefix ({unread_id}, {survivor_id})"
    );
    let unread_own = &unread_id[..=shared.len()];
    let survivor_own = &survivor_id[..=shared.len()];
    for run in [unread_run, survivor_run] {
        assert_eq!(
            rundir::classify_run_dir(&rundir::public_dir(&repo, run)),
            rundir::RunDirClass::Committed,
            "the premise: the census classifies run {run}'s committed header"
        );
    }
    let log = rundir::public_dir(&repo, unread_run).join(rundir::EVENT_LOG);
    let lone = |only: &str| {
        format!(
            "that prefix matches one question ({only}) among the runs that could be read, and a \
             run that could not may hold another; use the full question id; and the log of run \
             `{unread_run}` could not be read"
        )
    };
    let question = |id: &str| format!("question {id} ");

    let checkout = public_snapshot(&repo);
    let (refused, reads) = with_initial_read_failing(&log, || answer_and_show(&repo, &shared));
    let moved = moved_entries(&checkout, &public_snapshot(&repo));
    assert!(
        moved.is_empty(),
        "a prefix an unread run's question shares wrote in the checkout: {moved:?}"
    );
    assert_eq!(
        reads, 2,
        "answer and show each failed the unread run's initial read"
    );
    for refused in refused {
        let message = refused
            .expect_err("a prefix an unread run may share selects nothing")
            .to_string();
        assert!(message.contains(&lone(survivor_id)), "{message}");
    }
    let (status, reads) = with_initial_read_failing(&log, || {
        crate::status::load(&repo, Some(unread_run.as_str()))
    });
    assert_eq!(reads, 1, "status failed the unread run's initial read");
    assert_the_initial_read_refused(status.err(), &log, "status of the unread run");
    let (shown, _) = with_initial_read_failing(&log, || crate::answer::show(&repo, survivor_id));
    let shown = shown.expect("the survivor's full id answers whatever went unread");
    assert!(shown.contains(&question(survivor_id)), "{shown}");
    let (missing, _) = with_initial_read_failing(&log, || crate::answer::show(&repo, unread_id));
    let message = missing
        .expect_err("the unread run's question is not found while its log cannot be read")
        .to_string();
    assert!(message.contains("no question with that id"), "{message}");
    assert!(
        message.contains(&format!("the log of run `{unread_run}` could not be read")),
        "{message}"
    );

    for refused in answer_and_show(&repo, &shared) {
        let message = refused
            .expect_err("read again, a prefix two questions share selects neither")
            .to_string();
        assert!(
            message.contains("that prefix matches 2 questions"),
            "{message}"
        );
        for id in [unread_id, survivor_id] {
            assert!(message.contains(id.as_str()), "{id}: {message}");
        }
        assert!(!message.contains("could not be read"), "{message}");
    }
    for (own, id) in [(unread_own, unread_id), (survivor_own, survivor_id)] {
        let shown = crate::answer::show(&repo, own)
            .expect("read again, a prefix one question carries is that question's");
        assert!(shown.contains(&question(id)), "{own}: {shown}");
    }
    let moved = moved_entries(&checkout, &public_snapshot(&repo));
    assert!(
        moved.is_empty(),
        "a lookup wrote in the checkout: {moved:?}"
    );

    let legacy = format!("{unread_own}LEGACY");
    legacy_question_run(&repo, "00LEGACY", &legacy);
    let checkout = public_snapshot(&repo);
    let (refused, reads) = with_initial_read_failing(&log, || answer_and_show(&repo, unread_own));
    let moved = moved_entries(&checkout, &public_snapshot(&repo));
    assert!(
        moved.is_empty(),
        "a prefix a legacy question shares with an unread run's question wrote in the \
         checkout: {moved:?}"
    );
    assert_eq!(
        reads, 2,
        "answer and show each failed the unread run's initial read"
    );
    for refused in refused {
        let message = refused
            .expect_err("a lone legacy match beside an unread run selects nothing")
            .to_string();
        assert!(message.contains(&lone(&legacy)), "{message}");
    }
    let (shown, _) = with_initial_read_failing(&log, || crate::answer::show(&repo, &legacy));
    let shown = shown.expect("the legacy question's full id answers whatever went unread");
    assert!(
        shown.contains(&format!("the legacy question {legacy}")),
        "{shown}"
    );
    for refused in answer_and_show(&repo, unread_own) {
        let message = refused
            .expect_err(
                "read again, a prefix a legacy and a schema-4 question share selects neither",
            )
            .to_string();
        assert!(
            message.contains("that prefix matches 2 questions"),
            "{message}"
        );
        for id in [legacy.as_str(), unread_id.as_str()] {
            assert!(message.contains(id), "{id}: {message}");
        }
    }
    let moved = moved_entries(&checkout, &public_snapshot(&repo));
    assert!(
        moved.is_empty(),
        "a lookup wrote in the checkout: {moved:?}"
    );

    let (recorded, _) = with_initial_read_failing(&log, || {
        crate::answer::answer(
            &repo,
            survivor_id,
            crate::answer::Reply::Text("the first".to_owned()),
        )
    });
    let recorded = recorded.expect("the survivor's full id is answered whatever went unread");
    assert_eq!(
        (recorded.run_id.as_str(), recorded.question_id.as_str()),
        (survivor_run.as_str(), survivor_id.as_str())
    );
    let (recorded, _) = with_initial_read_failing(&log, || {
        crate::answer::answer(
            &repo,
            &legacy,
            crate::answer::Reply::Text("the first".to_owned()),
        )
    });
    let recorded =
        recorded.expect("the legacy question's full id is answered whatever went unread");
    assert_eq!(
        (recorded.run_id.as_str(), recorded.question_id.as_str()),
        ("00LEGACY", legacy.as_str())
    );
    let recorded = crate::answer::answer(
        &repo,
        unread_id,
        crate::answer::Reply::Text("the first".to_owned()),
    )
    .expect("read again, the once-unread run's question is answered by its full id");
    assert_eq!(
        (recorded.run_id.as_str(), recorded.question_id.as_str()),
        (unread_run.as_str(), unread_id.as_str())
    );
    let mut written = answer_files(&repo);
    written.sort();
    let mut expected = vec![
        rundir::public_dir(&repo, "00LEGACY")
            .join("answers")
            .join(format!("{legacy}.json")),
        rundir::public_dir(&repo, survivor_run)
            .join("answers")
            .join(format!("{survivor_id}.json")),
        rundir::public_dir(&repo, unread_run)
            .join("answers")
            .join(format!("{unread_id}.json")),
    ];
    expected.sort();
    assert_eq!(
        written, expected,
        "each full id wrote its own question's answer, and nothing else was written"
    );
}

/// PR12-ANSWER-CROSS-SCHEMA-PREFIX, settled questions: a schema-4 question its
/// log has settled stays a candidate for a prefix, as a legacy question's file
/// stays one once answered, so a prefix it shares with a legacy question selects
/// neither; alone, or by its full id, it refuses as no longer open.
#[test]
fn a_settled_schema_4_question_still_counts_against_a_prefix_and_takes_no_answer() {
    let (_tree, repo) = temp_engine_repo("topology-answer-cross-settled");
    let source = SharedTopologyStubs(std::sync::Arc::new(TopologyStub::new(
        TOPOLOGY_STUB_EDIT,
        1,
    )));
    let parked = run_with(&topology_options(&repo), &source).expect("the run parks");
    assert_eq!(parked.outcome(), RunOutcome::Parked, "{parked:#?}");
    until_released(&repo, &parked.run_id);
    let [asked] = parked.questions.as_slice() else {
        panic!("one question: {parked:#?}");
    };
    let id = asked.question.id.to_string();
    crate::answer::answer(
        &repo,
        &id,
        crate::answer::Reply::Text("the first".to_owned()),
    )
    .expect("the answer is recorded");
    let mut resume = resume_options(&repo, &parked.run_id);
    resume.private_root = None;
    let report = resume_with(&resume, &source).expect("the resume completes the run");
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:#?}");
    until_released(&repo, &parked.run_id);

    let prefix = &id[..id.len() - 6];
    let legacy = format!("{prefix}LEGACY");
    legacy_question_run(&repo, "00LEGACY", &legacy);
    for refused in answer_and_show(&repo, prefix) {
        let message = refused
            .expect_err("a prefix a settled question shares selects neither")
            .to_string();
        assert!(message.contains("matches 2 questions"), "{message}");
        for named in [&id, &legacy] {
            assert!(message.contains(named.as_str()), "{named}: {message}");
        }
    }
    for wanted in [&id[..id.len() - 1], id.as_str()] {
        for refused in answer_and_show(&repo, wanted) {
            let message = refused
                .expect_err("the settled question takes no answer")
                .to_string();
            assert!(message.contains("no longer open"), "{wanted}: {message}");
        }
    }
    assert!(
        !rundir::public_dir(&repo, "00LEGACY")
            .join("answers")
            .exists(),
        "a refused id wrote an answer"
    );
    let shown = crate::answer::show(&repo, &legacy).expect("the legacy question by its full id");
    assert!(
        shown.contains(&format!("the legacy question {legacy}")),
        "{shown}"
    );
}

/// A ladder from the mid tier onto the frontier for implementation tasks, an approval once the
/// run has reported half a cent of spend, and nobody at a terminal.
const SPEND_APPROVAL_CONFIG: &str = "[routing]\n\
     implement = { chain = [\"mid\", \"frontier\"], attempts_per = 1 }\n\n\
     [interaction]\nmode = \"never\"\nask_before = { frontier_escalation_over_usd = 0.005 }\n";

/// [`ObservedStub`], shareable across the coordinator's pipeline threads.
struct SharedObserved(std::sync::Arc<ObservedStub>);

impl AdapterSource for SharedObserved {
    fn get(&self, id: &str) -> Option<&dyn AgentAdapter> {
        (id == TOPOLOGY_STUB).then_some(&*self.0 as &dyn AgentAdapter)
    }

    fn shared(&self) -> Option<std::sync::Arc<dyn AdapterSource + Send + Sync>> {
        Some(std::sync::Arc::new(Self(std::sync::Arc::clone(&self.0))))
    }
}

/// One task whose ladder escalates from the mid tier onto the frontier.
fn escalating_plan() -> String {
    format!("## One\n<!-- upstroke: id=t1 kind=implement depends= -->\n{ESCALATES}\n")
}

/// J1, PR12 regression review #1 (P1). A schema-4 ladder that escalates onto the frontier once
/// the run's reported spend has reached `ask_before.frontier_escalation_over_usd` asks first,
/// as the sequential engine's `should_approve_spend` does (DESIGN §12). At the reviewed head the
/// schema-4 entry never supplied the control and the frontier ran unasked. Here, under the
/// coordinator, the question parks the run before any frontier worker starts, a resume with no
/// answer parks again and starts nothing, and the approval then runs the frontier attempt.
#[test]
fn a_schema_4_frontier_escalation_over_the_threshold_waits_for_its_approval() {
    let (_tree, repo) = temp_engine_repo("topology-approve-spend");
    seed(&repo, &escalating_plan(), Some(SPEND_APPROVAL_CONFIG));
    let stub = std::sync::Arc::new(ObservedStub::new(0));
    let source = SharedObserved(std::sync::Arc::clone(&stub));
    let parked = run_with(&topology_options(&repo), &source).expect("the run parks");
    assert_eq!(parked.outcome(), RunOutcome::Parked, "{parked:#?}");
    let [asked] = parked.questions.as_slice() else {
        panic!("one question: {parked:#?}");
    };
    assert_eq!(asked.question.kind, QuestionKind::ApproveSpend);
    for named in ["frontier", "$0.0100"] {
        assert!(
            asked.question.context.contains(named),
            "the question names `{named}`: {}",
            asked.question.context
        );
    }
    assert_eq!(
        stub.workers(),
        ["claude-sonnet-5"],
        "no frontier worker ran unapproved"
    );
    until_released(&repo, &parked.run_id);

    let mut resume = resume_options(&repo, &parked.run_id);
    resume.private_root = None;
    let again = resume_with(&resume, &source).expect("an unanswered resume parks again");
    assert_eq!(again.outcome(), RunOutcome::Parked, "{again:#?}");
    assert_eq!(
        stub.workers(),
        ["claude-sonnet-5"],
        "and still runs nothing on the frontier"
    );
    until_released(&repo, &parked.run_id);

    crate::answer::answer(
        &repo,
        &asked.question.id.to_string(),
        crate::answer::Reply::Option(1),
    )
    .expect("the approval is recorded");
    let report = resume_with(&resume, &source).expect("the approved run completes");
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:#?}");
    assert!(committed(&report, "t1"), "{report:#?}");
    assert_eq!(stub.workers(), ["claude-sonnet-5", FRONTIER_MODEL]);
    let events = log_events(&repo, &parked.run_id);
    let at = |kind: &str| events.iter().position(|event| event["event"] == kind);
    let raised = at("question_raised").expect("the question");
    let answered = at("question_answered").expect("its answer");
    let frontier = events
        .iter()
        .position(|event| event["event"] == "attempt_started" && event["data"]["rung"] == 1)
        .expect("the frontier attempt");
    assert!(
        raised < answered && answered < frontier,
        "asked, then approved, then run: {raised}, {answered}, {frontier}"
    );
}

/// J1: a declined spend approval fails the task through the halt policy with nothing spent on
/// the frontier, here through the step loop (the sequential engine's
/// `a_declined_spend_approval_fails_the_task_through_the_halt_policy`).
#[test]
fn a_schema_4_declined_spend_approval_fails_the_task_before_the_frontier_runs() {
    let (_tree, repo) = temp_engine_repo("topology-decline-spend");
    seed(&repo, &escalating_plan(), Some(SPEND_APPROVAL_CONFIG));
    let stub = ObservedStub::new(0);
    let source = BorrowedObserved(&stub);
    let parked = run_with(&topology_options(&repo), &source).expect("the run parks");
    assert_eq!(parked.outcome(), RunOutcome::Parked, "{parked:#?}");
    until_released(&repo, &parked.run_id);
    let [asked] = parked.questions.as_slice() else {
        panic!("one question: {parked:#?}");
    };
    crate::answer::answer(
        &repo,
        &asked.question.id.to_string(),
        crate::answer::Reply::Decline,
    )
    .expect("the decline is recorded");

    let mut resume = resume_options(&repo, &parked.run_id);
    resume.private_root = None;
    let report = resume_with(&resume, &source).expect("the declined run ends");
    assert_eq!(report.outcome(), RunOutcome::Halted, "{report:#?}");
    assert!(
        matches!(task(&report, "t1").status, TaskRunStatus::Failed { .. }),
        "{report:#?}"
    );
    assert_eq!(
        stub.workers(),
        ["claude-sonnet-5"],
        "declining spent nothing on the frontier"
    );
}

/// J1: while one task's escalation waits for its approval, an unrelated task runs to its merge
/// under the coordinator, and the run ends parked on the one question with nothing run on the
/// frontier.
#[test]
fn a_schema_4_spend_approval_parks_only_its_own_task() {
    let (_tree, repo) = temp_engine_repo("topology-approve-spend-alongside");
    seed(
        &repo,
        &format!(
            "{}\n## Two\n<!-- upstroke: id=t2 kind=implement depends= -->\nMake it.\n",
            escalating_plan()
        ),
        Some(&format!(
            "{SPEND_APPROVAL_CONFIG}\n[engine]\nmax_parallel = 2\n"
        )),
    );
    let stub = std::sync::Arc::new(ObservedStub::new(0));
    let source = SharedObserved(std::sync::Arc::clone(&stub));
    let parked = run_with(&topology_options(&repo), &source).expect("the run parks");
    assert_eq!(parked.outcome(), RunOutcome::Parked, "{parked:#?}");
    assert!(
        committed(&parked, "t2"),
        "the unrelated task merged: {parked:#?}"
    );
    assert!(
        matches!(task(&parked, "t1").status, TaskRunStatus::Parked { .. }),
        "{parked:#?}"
    );
    let [asked] = parked.questions.as_slice() else {
        panic!("one question: {parked:#?}");
    };
    assert_eq!(asked.question.kind, QuestionKind::ApproveSpend);
    assert!(
        !stub.workers().iter().any(|model| model == FRONTIER_MODEL),
        "{:?}",
        stub.workers()
    );
}

/// J1: under the threshold an escalation onto the frontier needs no approval, and its attempt
/// runs unasked.
#[test]
fn a_schema_4_frontier_escalation_under_the_threshold_runs_unasked() {
    let (_tree, repo) = temp_engine_repo("topology-spend-under-threshold");
    let config = SPEND_APPROVAL_CONFIG.replace("0.005", "1.0");
    seed(&repo, &escalating_plan(), Some(&config));
    let stub = ObservedStub::new(0);
    let report =
        run_with(&topology_options(&repo), &BorrowedObserved(&stub)).expect("the run completes");
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:#?}");
    assert!(committed(&report, "t1"), "{report:#?}");
    assert_eq!(stub.workers(), ["claude-sonnet-5", FRONTIER_MODEL]);
    assert!(
        !event_kinds(&repo, &report.run_id)
            .iter()
            .any(|kind| kind == "question_raised"),
        "nothing was asked"
    );
}

/// The words the J2 test answers with: an instruction no prompt holds unless the answer reached
/// it.
const UNIQUE_ANSWER: &str = "Use UNIQUE_ZEBRA_FORMAT_739 for the widget.";

/// J2, PR12 regression review #2 (P1), `PR249-ANSWER-TEXT-NOT-CARRIED`: a person's answer in their
/// own words reaches the resumed worker as an instruction and its reviewer as a decision, under
/// the coordinator and under the step loop, and the log keeps the words. At the reviewed head the
/// answer un-parked the task and reached no agent: `question_answered` kept only an option index
/// and the entry gave every review an empty list of decisions. Each mode parks on the worker's
/// question, takes `upstroke answer --text` and resumes.
#[test]
fn a_schema_4_answer_in_words_reaches_the_next_worker_and_its_reviewer() {
    for shared in [true, false] {
        let (_tree, repo) = temp_engine_repo(&format!("topology-answer-words-{shared}"));
        let stub = std::sync::Arc::new(ObservedStub::new(1));
        let shared_source = SharedObserved(std::sync::Arc::clone(&stub));
        let borrowed_source = BorrowedObserved(&stub);
        let source: &dyn AdapterSource = if shared {
            &shared_source
        } else {
            &borrowed_source
        };
        let parked = run_with(&topology_options(&repo), source).expect("the run parks");
        assert_eq!(parked.outcome(), RunOutcome::Parked, "{parked:#?}");
        until_released(&repo, &parked.run_id);
        let [asked] = parked.questions.as_slice() else {
            panic!("one question: {parked:#?}");
        };
        crate::answer::answer(
            &repo,
            &asked.question.id.to_string(),
            crate::answer::Reply::Text(UNIQUE_ANSWER.to_owned()),
        )
        .expect("the answer is recorded");
        let before = stub.built().len();

        let mut resume = resume_options(&repo, &parked.run_id);
        resume.private_root = None;
        let report = resume_with(&resume, source).expect("the resume completes the run");
        assert_eq!(report.outcome(), RunOutcome::Complete, "{report:#?}");
        let resumed = stub.built().split_off(before);
        let worker = resumed
            .iter()
            .find(|built| !built.reviewer)
            .expect("a resumed worker");
        assert!(
            worker.prompt.contains(UNIQUE_ANSWER)
                && worker
                    .prompt
                    .contains("This is an instruction from a person"),
            "shared {shared}: the worker is told:\n{}",
            worker.prompt
        );
        let reviewer = resumed
            .iter()
            .find(|built| built.reviewer)
            .expect("its reviewer");
        assert!(
            reviewer.prompt.contains(UNIQUE_ANSWER)
                && reviewer.prompt.contains("This is a decision from a person"),
            "shared {shared}: the reviewer is told:\n{}",
            reviewer.prompt
        );
        let answered = log_events(&repo, &parked.run_id)
            .into_iter()
            .find(|event| event["event"] == "question_answered")
            .expect("the answer's record");
        assert_eq!(
            answered["data"]["text"], UNIQUE_ANSWER,
            "shared {shared}: the log keeps the words"
        );
    }
}
