use super::*;
use crate::events::FailureRecord;
use crate::ir::{Question, QuestionId, QuestionKind, TaskId};
use crate::ladder::FailureOrigin;

fn report() -> RunReport {
    RunReport {
        run_id: "run-1".to_owned(),
        branch: "upstroke/run-1".to_owned(),
        gates: vec!["check".to_owned()],
        gates_from_config: true,
        warnings: Vec::new(),
        tasks: vec![TaskReport {
            id: "t1".to_owned(),
            title: "do the task".to_owned(),
            model: "worker".to_owned(),
            status: TaskRunStatus::Committed {
                sha: "abc123".to_owned(),
            },
            duration: Duration::from_secs(1),
            cost_usd: Some(0.01),
            review_models: vec!["reviewer".to_owned()],
            review_cost_usd: Some(0.02),
            review_cost_incomplete: false,
            session_id: None,
            attempts: vec![AttemptRecord {
                attempt: 1,
                tier: "small".to_owned(),
                model: "worker".to_owned(),
                pool: None,
                resumed: false,
                duration: Duration::from_secs(1),
                cost_usd: Some(0.01),
                reviews: Vec::new(),
                session_id: None,
                usage: None,
                failure: None,
            }],
        }],
        halted_at: None,
        questions: vec![QuestionRecord::open(Question {
            id: QuestionId::from("q-1"),
            kind: QuestionKind::Unblock,
            affected_tasks: vec![TaskId::from("t1")],
            context: "need a decision".to_owned(),
            options: Vec::new(),
        })],
        budget_stop: None,
        total_cost_usd: 0.03,
        pool_drain: vec![PoolDrainRow {
            pool: "worker-pool".to_owned(),
            attempts: 1,
            cost_usd: Some(0.01),
            unpriced: 0,
        }],
        running: false,
        interrupted: false,
        questions_in_log: false,
        unattributed_cost_incomplete: false,
    }
}

#[test]
fn settled_reports_escape_log_fields_without_inserting_layout_lines() {
    type Field = (&'static str, fn(&mut RunReport, &str));
    let fields: &[Field] = &[
        ("run id", |r, s| r.run_id = s.to_owned()),
        ("branch", |r, s| r.branch = s.to_owned()),
        ("gate", |r, s| r.gates[0] = s.to_owned()),
        ("warning", |r, s| r.warnings.push(s.to_owned())),
        ("task id", |r, s| r.tasks[0].id = s.to_owned()),
        ("title", |r, s| r.tasks[0].title = s.to_owned()),
        ("worker model", |r, s| r.tasks[0].model = s.to_owned()),
        ("review model", |r, s| {
            r.tasks[0].review_models[0] = s.to_owned()
        }),
        ("trail", |r, s| r.tasks[0].attempts[0].tier = s.to_owned()),
        ("commit", |r, s| {
            r.tasks[0].status = TaskRunStatus::Committed { sha: s.to_owned() }
        }),
        ("failed reason", |r, s| {
            r.tasks[0].status = TaskRunStatus::Failed {
                kind: FailureKind::AgentError,
                reason: s.to_owned(),
            }
        }),
        ("parked reason", |r, s| {
            r.tasks[0].status = TaskRunStatus::Parked {
                question: "q-1".to_owned(),
                reason: s.to_owned(),
            }
        }),
        ("parked question", |r, s| {
            r.tasks[0].status = TaskRunStatus::Parked {
                question: s.to_owned(),
                reason: "waiting".to_owned(),
            }
        }),
        ("blocked dependency", |r, s| {
            r.tasks[0].status = TaskRunStatus::Blocked { by: s.to_owned() }
        }),
        ("running tier", |r, s| {
            r.tasks[0].status = TaskRunStatus::Running {
                attempt: 1,
                tier: s.to_owned(),
                model: "worker".to_owned(),
            }
        }),
        ("running model", |r, s| {
            r.tasks[0].status = TaskRunStatus::Running {
                attempt: 1,
                tier: "small".to_owned(),
                model: s.to_owned(),
            }
        }),
        ("question id", |r, s| {
            r.questions[0].question.id = QuestionId::from(s)
        }),
        ("question context", |r, s| {
            r.questions[0].question.context = s.to_owned()
        }),
        ("halted task", |r, s| r.halted_at = Some(s.to_owned())),
        ("pool", |r, s| r.pool_drain[0].pool = s.to_owned()),
        ("budget task", |r, s| {
            r.budget_stop = Some(events::BudgetExceeded {
                budget: events::BudgetKind::Run,
                limit_usd: 1.0,
                spent_usd: 1.0,
                task: s.to_owned(),
            })
        }),
    ];
    for (field, set) in fields {
        let mut ordinary = report();
        set(&mut ordinary, "x[2Jyz");
        let mut hostile = report();
        set(&mut hostile, "x\u{1b}[2Jy\nz\r\t\u{7}");
        let before = serde_json::to_value(&hostile).expect("report serializes");
        let clean = ordinary.render() + &ordinary.render_ledger();
        let rendered = hostile.render() + &hostile.render_ledger();
        assert!(
            rendered.chars().all(|c| c == '\n' || !c.is_control()),
            "{field}: {rendered:?}"
        );
        assert_eq!(
            rendered.lines().count(),
            clean.lines().count(),
            "{field}: {rendered:?}"
        );
        assert!(
            rendered.contains("\\u{1b}[2J"),
            "field was escaped, not dropped: {field}: {rendered:?}"
        );
        assert_eq!(
            serde_json::to_value(&hostile).expect("report still serializes"),
            before
        );
    }
}

#[test]
fn an_agent_stderr_reason_is_visible_without_terminal_controls_in_a_report() {
    let mut report = report();
    report.tasks[0].status = TaskRunStatus::Failed {
        kind: FailureKind::AgentError,
        reason: "agent error (exit Some(1)): x\n\u{1b}[2Jy".to_owned(),
    };
    let rendered = report.render();
    assert!(
        rendered.contains("agent error (exit Some(1)): x \\u{1b}[2Jy"),
        "{rendered:?}"
    );
    assert!(!rendered.contains('\u{1b}'), "{rendered:?}");
    assert_eq!(
        rendered
            .lines()
            .filter(|line| line.contains("FAILED"))
            .count(),
        1
    );
}

#[test]
fn report_layout_keeps_the_budget_resume_command_on_its_own_line() {
    let mut report = report();
    report.budget_stop = Some(events::BudgetExceeded {
        budget: events::BudgetKind::Run,
        limit_usd: 1.0,
        spent_usd: 1.25,
        task: "t1".to_owned(),
    });
    let rendered = report.render();
    assert!(
        rendered.contains("continue with:\n    upstroke resume run-1 --budget <usd>\n"),
        "{rendered:?}"
    );
    assert!(rendered.starts_with("run: run-1\nbranch: upstroke/run-1 (return with: git switch -)\ngates: check [from config]\n"), "{rendered:?}");
}

/// A schema-1..3 run writes a payload file for each question and its summary
/// points at the directory; a schema-4 run keeps its questions in its log and
/// writes no such file, so its summary says how to reach one instead of naming
/// a directory that is not there (found by brief 3's S9 witness, V129 case E).
#[test]
fn open_questions_are_pointed_at_where_their_run_keeps_them() {
    let asked = QuestionRecord::open(Question {
        id: QuestionId::from("q-1"),
        kind: QuestionKind::Unblock,
        affected_tasks: vec![TaskId::from("t1")],
        context: "which widget format".to_owned(),
        options: Vec::new(),
    });
    let legacy = RunReport {
        questions: vec![asked.clone()],
        ..report()
    }
    .render();
    assert!(legacy.contains("open questions (1):"), "{legacy}");
    assert!(legacy.contains("  payloads: "), "{legacy}");
    assert!(
        !legacy.contains("upstroke answer <question id>"),
        "{legacy}"
    );

    let in_log = RunReport {
        questions: vec![asked],
        questions_in_log: true,
        ..report()
    }
    .render();
    assert!(in_log.contains("open questions (1):"), "{in_log}");
    assert!(!in_log.contains("payloads:"), "{in_log}");
    assert!(
        in_log.contains("`upstroke answer <question id>` shows and answers it"),
        "{in_log}"
    );
}

#[test]
fn a_schema_4_report_keeps_each_interrupted_attempt_with_its_own_start_and_unknown_spend() {
    use crate::engine::topology::settle::tests::{
        ALEPH, BET, binding, ev, label, record_failing, started,
    };
    use crate::topology::events::{
        AttemptFinished4, AttemptInterrupted4, AttemptNumber, AttemptSettlement, AttemptStarted4,
        GenerationId, LeaseDisposition, SettlementTransition, TopologyEventBody,
    };
    use crate::topology::registry::TaskKey;

    let fold = started();
    let start = |key: TaskKey, generation: u32, model: &str| {
        let mut bound = binding(&fold, key, 0);
        bound.model = model.to_owned();
        ev(TopologyEventBody::AttemptStarted {
            data: AttemptStarted4 {
                key,
                generation: GenerationId(generation),
                attempt: AttemptNumber(1),
                rung: 0,
                binding: bound,
                pool: Some(format!("{}-pool", label(key))),
                resume_session: None,
                materialization_observed: None,
            },
        })
    };
    let interrupted = |key: TaskKey, generation: u32| {
        ev(TopologyEventBody::AttemptInterrupted {
            data: AttemptInterrupted4 {
                key,
                generation: GenerationId(generation),
                attempt: AttemptNumber(1),
                lease: LeaseDisposition::PredictedReleased,
                detail: format!("{} was in flight when its coordinator died", label(key)),
            },
        })
    };
    let finished = ev(TopologyEventBody::AttemptFinished {
        data: Box::new(AttemptFinished4 {
            key: ALEPH,
            generation: GenerationId(1),
            attempt: AttemptNumber(1),
            record: Box::new(record_failing(
                1,
                Some(0.5),
                Some((FailureKind::GateFailed, FailureOrigin::Worker)),
            )),
            settlement: AttemptSettlement::Closed {
                transition: SettlementTransition::Retry,
                lease: LeaseDisposition::PredictedReleased,
            },
        }),
    });
    let events = vec![
        start(ALEPH, 0, "aleph-first"),
        start(BET, 0, "bet-first"),
        interrupted(BET, 0),
        interrupted(ALEPH, 0),
        start(ALEPH, 1, "aleph-second"),
        finished,
    ];

    let report = RunReport::from_topology(&fold, &events, Vec::new());
    let row = |key: TaskKey| {
        report
            .tasks
            .iter()
            .find(|task| task.id == label(key))
            .unwrap_or_else(|| panic!("a row for {}: {report:#?}", label(key)))
    };
    let shape = |record: &AttemptRecord| {
        (
            record.model.clone(),
            record.pool.clone(),
            record.cost_usd,
            record.failure.as_ref().map(|failure| failure.kind),
        )
    };
    assert_eq!(
        row(ALEPH).attempts.iter().map(shape).collect::<Vec<_>>(),
        [
            (
                "aleph-first".to_owned(),
                Some("aleph-pool".to_owned()),
                None,
                Some(FailureKind::Interrupted)
            ),
            (
                "aleph-Mid-model".to_owned(),
                None,
                Some(0.5),
                Some(FailureKind::GateFailed)
            ),
        ],
        "the interrupted attempt is kept once, in log order, with its own start's model and pool"
    );
    assert_eq!(
        row(BET).attempts.iter().map(shape).collect::<Vec<_>>(),
        [(
            "bet-first".to_owned(),
            Some("bet-pool".to_owned()),
            None,
            Some(FailureKind::Interrupted)
        )],
        "each task keeps its own interrupted attempt"
    );
    assert!(
        row(ALEPH).cost_incomplete() && row(BET).cost_incomplete(),
        "{report:#?}"
    );
    assert!(
        (report.total_cost_usd - 0.5).abs() < f64::EPSILON,
        "no price is invented for an interrupted attempt: {}",
        report.total_cost_usd
    );
    assert!(report.total_is_floor(), "{report:#?}");
    assert!(
        report.warnings.is_empty(),
        "every interrupted attempt has its start: {:?}",
        report.warnings
    );
    assert_eq!(
        report
            .pool_drain
            .iter()
            .map(|row| (row.pool.as_str(), row.attempts, row.cost_usd, row.unpriced))
            .collect::<Vec<_>>(),
        [("aleph-pool", 1, None, 1), ("bet-pool", 1, None, 1)],
        "each interrupted attempt drains its pool by an unpriced attempt"
    );

    let orphan = RunReport::from_topology(&fold, &[interrupted(BET, 3)], Vec::new());
    let kept = orphan
        .tasks
        .iter()
        .find(|task| task.id == label(BET))
        .map(|task| task.attempts.as_slice());
    assert!(
        matches!(kept, Some([record]) if record.model.is_empty() && record.cost_usd.is_none()),
        "an interrupted settlement with no start is still kept, naming nothing it lacks: \
         {orphan:#?}"
    );
    assert!(orphan.total_is_floor(), "{orphan:#?}");
    assert!(
        orphan
            .warnings
            .iter()
            .any(|warning| warning.contains("without recording its start")),
        "{:?}",
        orphan.warnings
    );
}

#[test]
fn an_interrupted_attempts_record_comes_from_the_one_record_builder_and_names_nothing_it_lacks() {
    use crate::engine::topology::settle::tests::{ALEPH, binding, started};
    use crate::topology::events::{
        AttemptInterrupted4, AttemptNumber, AttemptStarted4, GenerationId, LeaseDisposition,
        SessionId,
    };

    let fold = started();
    let mut bound = binding(&fold, ALEPH, 0);
    bound.tier = crate::ir::Tier::Frontier;
    bound.model = "aleph-resumed-model".to_owned();
    let start = AttemptStarted4 {
        key: ALEPH,
        generation: GenerationId(2),
        attempt: AttemptNumber(3),
        rung: 0,
        binding: bound,
        pool: Some("aleph-pool".to_owned()),
        resume_session: Some(SessionId("aleph-session".to_owned())),
        materialization_observed: None,
    };
    let interrupted = AttemptInterrupted4 {
        key: ALEPH,
        generation: GenerationId(2),
        attempt: AttemptNumber(3),
        lease: LeaseDisposition::PredictedReleased,
        detail: "aleph was in flight when its coordinator died".to_owned(),
    };
    let failure = Some(FailureRecord {
        kind: FailureKind::Interrupted,
        origin: FailureOrigin::Worker,
        reason: "aleph was in flight when its coordinator died".to_owned(),
        detail: None,
    });

    assert_eq!(
        interrupted_record(&interrupted, Some(&start)),
        AttemptRecord {
            attempt: 3,
            tier: "frontier".to_owned(),
            model: "aleph-resumed-model".to_owned(),
            pool: Some("aleph-pool".to_owned()),
            resumed: true,
            duration: Duration::ZERO,
            cost_usd: None,
            reviews: Vec::new(),
            session_id: None,
            usage: None,
            failure: failure.clone(),
        },
        "the start's tier, model, pool and resumption are kept, and no spend is invented"
    );
    assert_eq!(
        interrupted_record(&interrupted, None),
        AttemptRecord {
            attempt: 3,
            tier: String::new(),
            model: String::new(),
            pool: None,
            resumed: false,
            duration: Duration::ZERO,
            cost_usd: None,
            reviews: Vec::new(),
            session_id: None,
            usage: None,
            failure,
        },
        "with no start the record names no tier or model the log does not hold"
    );
}

#[test]
fn a_legacy_report_says_which_task_position_its_plan_does_not_hold_and_reports_the_rest() {
    let plan = crate::engine::topology::settle::tests::plan();
    let beyond = plan.tasks.len() + 4;
    let mut state = RunState::new(plan.tasks.iter().map(|task| task.id.to_string()).collect());
    state.order = vec![beyond, 0];
    let report = build_report(
        ReportHeader {
            run_id: "run-1",
            branch: "upstroke/run-1",
            gates: Vec::new(),
            gates_from_config: false,
            warnings: Vec::new(),
            running: false,
            interrupted: false,
        },
        &plan,
        &state,
    );
    assert_eq!(
        report
            .tasks
            .iter()
            .map(|task| task.id.as_str())
            .collect::<Vec<_>>(),
        plan.tasks
            .iter()
            .map(|task| task.id.as_str())
            .collect::<Vec<_>>(),
        "every task the plan holds has its row, in the state's order"
    );
    assert!(
        matches!(
            report.warnings.as_slice(),
            [warning] if warning.contains(&format!("task position {beyond}"))
                && warning.contains("no row for it")
        ),
        "the position the plan does not hold is named, not dropped: {:?}",
        report.warnings
    );
}

// K4: a committed task whose candidate's verification was interrupted before any review pass
// was charged has no review model on record and an unknown review cost. Its line says the
// review spend is unknown rather than reading as an unreviewed, free task, and a task with no
// review spend owed still says nothing about review.
#[test]
fn a_committed_task_whose_review_spend_is_unknown_says_so_though_no_pass_is_on_record() {
    let mut unknown = report();
    if let Some(task) = unknown.tasks.first_mut() {
        task.review_models = Vec::new();
        task.review_cost_usd = None;
        task.review_cost_incomplete = true;
    }
    let rendered = unknown.render();
    assert!(
        rendered.contains("worker $0.0100 + review $?)"),
        "{rendered}"
    );
    assert!(
        rendered.contains("total: $0.0300? (api-equivalent)"),
        "{rendered}"
    );

    let mut unreviewed = report();
    if let Some(task) = unreviewed.tasks.first_mut() {
        task.review_models = Vec::new();
        task.review_cost_usd = None;
    }
    let rendered = unreviewed.render();
    assert!(rendered.contains("worker $0.0100)"), "{rendered}");
    assert!(!rendered.contains("+ review"), "{rendered}");
}
