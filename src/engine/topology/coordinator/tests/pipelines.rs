//! Extended notes: `docs/internals/engine/topology/coordinator/tests/pipelines.md`

use super::*;

#[test]
fn a_pipeline_error_cancels_the_others_and_ends_the_command_resumably() {
    let tasks = three();
    let base = wide_responder(&tasks, &[]);
    let runner = RecordingRunner::new().answering(Box::new(move |request| {
        if attempt_key(&request.invocation) == Some(0)
            && matches!(role_of(&request.invocation), Some(AttemptRole::Gate(_)))
        {
            return Err(crate::runner::RunnerError::unresolved(
                &request.invocation,
                UpstrokeError::Refused {
                    message: "the scaffold lost track of this gate".to_owned(),
                },
            ));
        }
        base(request)
    }));
    runner.hold();
    let mut wide = Wide::started_with(
        "coordinator-pipeline-error",
        &tasks,
        3,
        WidePlans::default(),
        runner,
    );
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let mut scheduler = Scheduler::scripted(
        &runner,
        Box::new(|view: &Quiescent<'_>| {
            released(view, |invocation| attempt_key(invocation) == Some(0))
        }),
    );
    let error = drive(&mut wide, Some(&mut scheduler)).expect_err("the command ends");
    drop(scheduler);
    assert!(
        error.to_string().contains("lost track of this gate"),
        "the pipeline's own error ends the command: {error}"
    );
    let events = wide.env.durable_events();
    let kinds = kinds_of(&events);
    assert_eq!(
        count(&events, "attempt_interrupted"),
        0,
        "no live `attempt_interrupted`: recovery settles the open attempts: {kinds:?}"
    );
    assert_eq!(count(&events, "attempt_started"), 3, "{kinds:?}");
    assert_eq!(
        kinds.last(),
        Some(&"attempt_started"),
        "nothing was appended after the error: {kinds:?}"
    );
    let mut cancelled: Vec<u32> = wide
        .env
        .runner
        .endings()
        .into_iter()
        .filter(|(_, ending)| *ending == crate::engine::topology::scaffold::Ending::Cancelled)
        .filter_map(|(invocation, _)| attempt_key(&invocation))
        .collect();
    cancelled.sort_unstable();
    assert_eq!(
        cancelled,
        vec![1, 2],
        "the other two workers were cancelled"
    );
    let gate = AttemptIdentities::new(TaskKey(0), GenerationId(0), AttemptNumber(1)).gate(0, 0);
    settled_but(&mut wide.run, &[gate]).unwrap_or_else(|ledger| {
        panic!(
            "the gate whose process the Runner could not establish as ended keeps its \
                 registration, and every other invocation is settled once: {ledger}"
        )
    });
    replay_equals_live(&wide);
}

#[test]
fn a_panicking_pipeline_ends_the_command_with_a_defined_error() {
    let tasks = three();
    let base = wide_responder(&tasks, &[]);
    let runner = RecordingRunner::new().answering(Box::new(move |request| {
        if attempt_key(&request.invocation) == Some(1)
            && matches!(role_of(&request.invocation), Some(AttemptRole::Gate(_)))
        {
            panic!("the scaffold's gate fell over");
        }
        base(request)
    }));
    let mut wide = Wide::started_with("coordinator-panic", &tasks, 3, WidePlans::default(), runner);
    let error = drive(&mut wide, None).expect_err("the command ends");
    assert!(
        error.to_string().contains("a pipeline panicked"),
        "the panic became the pipeline's completion: {error}"
    );
    let gate = AttemptIdentities::new(TaskKey(1), GenerationId(0), AttemptNumber(1)).gate(0, 0);
    settled_but(&mut wide.run, &[gate]).unwrap_or_else(|ledger| {
        panic!(
            "the gate the panic unwound through never reported its end, so it keeps its \
                 registration, and every other invocation is settled once: {ledger}"
        )
    });
}

#[test]
fn an_unresolved_end_keeps_its_pair_and_no_waiting_invocation_starts_on_it() {
    let tasks = [
        WideTask::independent("alpha"),
        WideTask::independent("beta"),
    ];
    let alpha = AttemptIdentities::new(TaskKey(0), GenerationId(0), AttemptNumber(1)).worker();
    let beta = AttemptIdentities::new(TaskKey(1), GenerationId(0), AttemptNumber(1)).worker();
    let base = wide_responder(&tasks, &[]);
    let lost = alpha.clone();
    let runner = RecordingRunner::new().answering(Box::new(move |request| {
        if request.invocation == lost {
            return Err(crate::runner::RunnerError::unresolved(
                &request.invocation,
                UpstrokeError::Refused {
                    message: "the scaffold stopped supervising alpha's worker and could not \
                                  establish that it ended"
                        .to_owned(),
                },
            ));
        }
        base(request)
    }));
    runner.hold();
    let mut wide = Wide::started_with(
        "coordinator-unresolved-pair",
        &tasks,
        2,
        WidePlans::default(),
        runner,
    );
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let mut scheduler = Scheduler::scripted(
        &runner,
        Box::new(|view: &Quiescent<'_>| {
            released(view, |invocation| worker(invocation) == Some(TaskKey(0)))
        }),
    );
    let mut hooks = wide.env.hooks();
    let pipelines =
        wide.env
            .pipelines_limited(crate::engine::topology::scaffold::SlotLimitsOf::Exactly(
                1, 1,
            ));
    let error = wide
        .run
        .run_concurrently(
            &wide.env.seams(),
            &pipelines,
            &mut hooks,
            Some(&mut scheduler),
        )
        .expect_err("an unresolved process ends the command");
    drop(scheduler);
    assert!(
        !wide
            .env
            .runner
            .ran()
            .iter()
            .any(|ran| ran.invocation == beta),
        "beta's worker never started on the pair alpha's unresolved process holds: {:?}",
        wide.env.runner.endings()
    );
    let message = error.to_string();
    assert!(
        message.contains("could not establish that it ended"),
        "the command ends with alpha's own error: {message}"
    );
    let events = wide.env.durable_events();
    assert_eq!(
        kinds_of(&events).last(),
        Some(&"attempt_started"),
        "nothing is appended for either attempt: {:?}",
        kinds_of(&events)
    );
    settled_but(&mut wide.run, std::slice::from_ref(&alpha)).unwrap_or_else(|ledger| {
        panic!("alpha's worker keeps its registration and its pair, beta's request is withdrawn: {ledger}")
    });
    replay_equals_live(&wide);
}

#[test]
fn an_unreported_end_keeps_its_pair_and_no_waiting_invocation_starts_on_it() {
    let tasks = [
        WideTask::independent("alpha"),
        WideTask::independent("beta"),
    ];
    let alpha = AttemptIdentities::new(TaskKey(0), GenerationId(0), AttemptNumber(1)).worker();
    let beta = AttemptIdentities::new(TaskKey(1), GenerationId(0), AttemptNumber(1)).worker();
    let runner = holding(&tasks, &[]);
    runner.panic_when_released(alpha.clone());
    let mut wide = Wide::started_with(
        "coordinator-unreported-end",
        &tasks,
        2,
        WidePlans::default(),
        runner,
    );
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let mut scheduler = Scheduler::scripted(
        &runner,
        Box::new(|view: &Quiescent<'_>| {
            released(view, |invocation| worker(invocation) == Some(TaskKey(0)))
        }),
    );
    let mut hooks = wide.env.hooks();
    let pipelines =
        wide.env
            .pipelines_limited(crate::engine::topology::scaffold::SlotLimitsOf::Exactly(
                1, 1,
            ));
    let error = wide
        .run
        .run_concurrently(
            &wide.env.seams(),
            &pipelines,
            &mut hooks,
            Some(&mut scheduler),
        )
        .expect_err("the panic ends the command");
    drop(scheduler);
    assert!(
        !wide
            .env
            .runner
            .ran()
            .iter()
            .any(|ran| ran.invocation == beta),
        "beta's worker never started on the pair alpha's unreported invocation holds: {:?}",
        wide.env.runner.endings()
    );
    let message = error.to_string();
    assert!(
        message.contains("a pipeline panicked") && message.contains("fell over"),
        "alpha's own completion ends the command: {message}"
    );
    settled_but(&mut wide.run, std::slice::from_ref(&alpha)).unwrap_or_else(|ledger| {
        panic!("alpha's worker keeps its registration and its pair, beta's request is withdrawn: {ledger}")
    });
    replay_equals_live(&wide);
}

#[test]
fn a_fatal_verification_completion_interrupts_on_receipt_without_waiting_for_the_snapshots() {
    let tasks = [
        WideTask::independent("alpha"),
        WideTask::independent("beta"),
        WideTask::independent("gamma"),
    ];
    let plans = WidePlans {
        panic_verifying: true,
        ..WidePlans::default()
    };
    let mut wide = Wide::started_with(
        "coordinator-fatal-verification",
        &tasks,
        3,
        plans,
        holding(&tasks, &[]),
    );
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let mut waited_after_arrival: Vec<Vec<InvocationId>> = Vec::new();
    let mut scheduler = Scheduler::scripted(
        &runner,
        Box::new(|view: &Quiescent<'_>| {
            let verifying = view.run.fold().transaction().is_some();
            let verification_live = view
                .live
                .iter()
                .any(|(_, identity)| matches!(identity, Identity::Verification { .. }));
            if verifying && !verification_live {
                waited_after_arrival.push(view.invoking.clone());
            }
            if count(view.run.events(), "task_merged") == 0 {
                return released(view, |invocation| attempt_key(invocation) == Some(0));
            }
            released(view, |invocation| worker(invocation) == Some(TaskKey(2)))
                .or_else(|| released(view, |invocation| attempt_key(invocation) == Some(1)))
                .or_else(|| released(view, |invocation| attempt_key(invocation) == Some(2)))
        }),
    );
    let error = drive(&mut wide, Some(&mut scheduler)).expect_err("the panic ends the command");
    drop(scheduler);
    assert!(
        waited_after_arrival.is_empty(),
        "the coordinator never waited, with the fatal completion received, for gamma's \
             snapshot to drain: it waited at {waited_after_arrival:?}"
    );
    let message = error.to_string();
    assert!(
        message.contains("a pipeline panicked") && message.contains("fell over"),
        "the verification's own error ends the command: {message}"
    );
    let review =
        AttemptIdentities::new(TaskKey(2), GenerationId(0), AttemptNumber(1)).review_pass(0, 0);
    assert!(
        wide.env
            .runner
            .endings()
            .contains(&(review, crate::engine::topology::scaffold::Ending::Cancelled)),
        "gamma's review, held when the panic arrived, was cancelled: {:?}",
        wide.env.runner.endings()
    );
    let events = wide.env.durable_events();
    assert_eq!(
        kinds_of(&events).last(),
        Some(&"merge_verification_started"),
        "nothing is appended after the verification started: {:?}",
        kinds_of(&events)
    );
    assert!(wide.run.invocations_balance(), "{:?}", wide.run.warnings());
    replay_equals_live(&wide);
}

#[test]
fn a_declined_embedded_question_stops_the_verification_its_lineage_had_started() {
    let tasks = [
        WideTask::independent("alpha"),
        WideTask::independent("beta"),
        WideTask::independent("gamma"),
    ];
    let armed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut wide = Wide::started_with(
        "coordinator-declined-verification",
        &tasks,
        3,
        WidePlans::default(),
        holding(&tasks, &[]),
    );
    wide.env.answers = std::sync::Arc::new(DecliningOnceArmed(std::sync::Arc::clone(&armed)));
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let arming = std::sync::Arc::clone(&armed);
    let mut planted: Option<Vec<TopologyEventBody>> = None;
    let mut scheduler = Scheduler::scripted(
        &runner,
        Box::new(move |view: &Quiescent<'_>| {
            if count(view.run.events(), "task_merged") == 0 {
                return released(view, |invocation| attempt_key(invocation) == Some(0));
            }
            let verifying = view
                .live
                .iter()
                .any(|(_, identity)| matches!(identity, Identity::Verification { .. }));
            if verifying && planted.is_none() {
                let source = view
                    .run
                    .fold()
                    .transaction()
                    .map(|open| open.candidate.clone())
                    .expect("beta's candidate is being verified");
                planted = Some(sibling_parked_on(view.run, TaskKey(1), source));
            }
            if let Some(events) = planted.as_mut() {
                if !events.is_empty() {
                    return Some(Release::Append(Box::new(events.remove(0))));
                }
                arming.store(true, std::sync::atomic::Ordering::SeqCst);
                return released(view, |invocation| attempt_key(invocation) == Some(2)).or_else(
                    || {
                        released(view, |invocation| {
                            matches!(invocation, InvocationId::Sequence { .. })
                        })
                    },
                );
            }
            released(view, |invocation| attempt_key(invocation) == Some(1)).or_else(|| {
                released(view, |invocation| {
                    matches!(invocation, InvocationId::Sequence { .. })
                })
            })
        }),
    );
    let progress = drive(&mut wide, Some(&mut scheduler))
        .expect("the decline fails beta's lineage and the run goes on to its end");
    drop(scheduler);
    assert_eq!(outcome_of(&progress), RunOutcome::Complete);
    let events = wide.env.durable_events();
    let kinds = kinds_of(&events);
    let decline = position(&kinds, "question_answered", 0);
    let first = SequenceId(1);
    cancelled_terminal_after(&events, decline, first, LINEAGE_CANCELLED);
    let gate = SequenceIdentities::new(first).gate(0, 0);
    assert!(
        wide.env
            .runner
            .endings()
            .contains(&(gate, crate::engine::topology::scaffold::Ending::Cancelled)),
        "the verification's gate, held when the decline was ingested, was cancelled: {:?}",
        wide.env.runner.endings()
    );
    assert!(
        wide.run.warnings().iter().any(|warning| {
            warning.contains("was cancelled: a decline, or a failed settlement, failed its lineage")
        }),
        "{:?}",
        wide.run.warnings()
    );
    assert_eq!(
        count(&events, "task_merged"),
        2,
        "alpha and gamma merge; beta's lineage failed: {kinds:?}"
    );
    assert!(wide.run.invocations_balance(), "{:?}", wide.run.warnings());
    replay_equals_live(&wide);
}

#[test]
fn a_declined_embedded_question_stops_a_running_sibling_attempt() {
    let tasks = [
        WideTask::independent("alpha"),
        WideTask::independent("beta"),
    ];
    let armed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut wide = Wide::started_with(
        "coordinator-declined-sibling",
        &tasks,
        2,
        WidePlans::default(),
        holding(&tasks, &[]),
    );
    wide.env.answers = std::sync::Arc::new(DecliningOnceArmed(std::sync::Arc::clone(&armed)));
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let arming = std::sync::Arc::clone(&armed);
    let head = wide.env.head(&wide.run);
    let mut planted: Option<Vec<TopologyEventBody>> = None;
    let mut scheduler = Scheduler::scripted(
        &runner,
        Box::new(move |view: &Quiescent<'_>| {
            let beta_held = view
                .invoking
                .iter()
                .any(|invocation| worker(invocation) == Some(TaskKey(1)));
            if beta_held && planted.is_none() {
                let source = CandidateRef {
                    key: TaskKey(1),
                    generation: GenerationId(0),
                    commit_sha: crate::topology::events::CommitSha(head.clone()),
                    candidate_ref: crate::topology::events::GitRef(
                        "refs/upstroke/planted/sibling-source".to_owned(),
                    ),
                };
                planted = Some(sibling_parked_on(view.run, TaskKey(1), source));
            }
            if let Some(events) = planted.as_mut() {
                if !events.is_empty() {
                    return Some(Release::Append(Box::new(events.remove(0))));
                }
                arming.store(true, std::sync::atomic::Ordering::SeqCst);
            }
            released(view, |invocation| attempt_key(invocation) == Some(0))
                .or_else(|| released(view, |invocation| attempt_key(invocation) == Some(1)))
                .or_else(|| {
                    released(view, |invocation| {
                        matches!(invocation, InvocationId::Sequence { .. })
                    })
                })
        }),
    );
    let progress = drive(&mut wide, Some(&mut scheduler))
        .expect("the decline fails beta's lineage and the run goes on to its end");
    drop(scheduler);
    assert_eq!(outcome_of(&progress), RunOutcome::Complete);
    let beta = AttemptIdentities::new(TaskKey(1), GenerationId(0), AttemptNumber(1));
    assert!(
        wide.env.runner.endings().contains(&(
            beta.worker(),
            crate::engine::topology::scaffold::Ending::Cancelled
        )),
        "beta's worker, held when the decline was ingested, was cancelled: {:?}",
        wide.env.runner.endings()
    );
    assert!(
        !wide
            .env
            .runner
            .ran()
            .iter()
            .any(|ran| attempt_key(&ran.invocation) == Some(1) && ran.invocation != beta.worker()),
        "no gate or review of beta's closed generation ever started: {:?}",
        wide.env.runner.endings()
    );
    let events = wide.env.durable_events();
    assert!(
        !events.iter().any(|event| matches!(&event.body,
            TopologyEventBody::CandidatePrepared { data } if data.key == TaskKey(1))),
        "beta's late result was never settled: {:?}",
        kinds_of(&events)
    );
    assert_eq!(count(&events, "task_merged"), 1, "{:?}", kinds_of(&events));
    assert!(wide.run.invocations_balance(), "{:?}", wide.run.warnings());
    replay_equals_live(&wide);
}

#[test]
fn a_halt_answers_a_request_its_intake_still_buffers() {
    bounded("the halted run", || {
        let tasks = [
            WideTask::independent("alpha"),
            WideTask::independent("beta"),
        ];
        let armed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let runner = RecordingRunner::new().answering(
            crate::engine::topology::scaffold::wide_responder_asking(&tasks, &[], &[0]),
        );
        runner.hold();
        let mut wide = Wide::started_with(
            "coordinator-halt-buffered",
            &tasks,
            2,
            WidePlans::default(),
            runner,
        );
        halting(&mut wide.env);
        wide.env.answers = std::sync::Arc::new(DecliningOnceArmed(std::sync::Arc::clone(&armed)));
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let mut scheduler = Scheduler::scripted(
            &runner,
            Box::new(move |view: &Quiescent<'_>| {
                let parked = view
                    .run
                    .fold()
                    .open_questions()
                    .is_some_and(|open| !open.is_empty());
                if !parked {
                    return released(view, |invocation| worker(invocation) == Some(TaskKey(0)));
                }
                armed.store(true, std::sync::atomic::Ordering::SeqCst);
                released(view, |invocation| attempt_key(invocation) == Some(1))
            }),
        );
        let progress = drive(&mut wide, Some(&mut scheduler)).expect("the halt ends the run");
        drop(scheduler);
        assert_eq!(outcome_of(&progress), RunOutcome::Halted);
        let events = wide.env.durable_events();
        let kinds = kinds_of(&events);
        assert_eq!(
            count(&events, "attempt_interrupted"),
            1,
            "beta, whose snapshot request was buffered when the halt cancelled it, is settled \
                 interrupted: {kinds:?}"
        );
        assert!(wide.run.invocations_balance(), "{:?}", wide.run.warnings());
        replay_equals_live(&wide);
    });
}

#[test]
fn a_stop_answers_a_request_its_intake_still_buffers() {
    bounded("the run with a stopped verification", || {
        let tasks = [
            WideTask::independent("alpha"),
            WideTask::independent("beta"),
        ];
        let armed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut wide = Wide::started_with(
            "coordinator-stop-buffered",
            &tasks,
            3,
            WidePlans::default(),
            holding(&tasks, &[]),
        );
        wide.env.answers = std::sync::Arc::new(DecliningOnceArmed(std::sync::Arc::clone(&armed)));
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let mut planted: Option<Vec<TopologyEventBody>> = None;
        let mut scheduler = Scheduler::scripted(
            &runner,
            Box::new(move |view: &Quiescent<'_>| {
                if count(view.run.events(), "task_merged") == 0 {
                    return released(view, |invocation| attempt_key(invocation) == Some(0));
                }
                let verifying = view
                    .live
                    .iter()
                    .any(|(_, identity)| matches!(identity, Identity::Verification { .. }));
                if verifying && planted.is_none() {
                    let source = view
                        .run
                        .fold()
                        .transaction()
                        .map(|open| open.candidate.clone())
                        .expect("beta's candidate is being verified");
                    planted = Some(sibling_parked_on(view.run, TaskKey(1), source));
                }
                if let Some(events) = planted.as_mut() {
                    if !events.is_empty() {
                        return Some(Release::Append(Box::new(events.remove(0))));
                    }
                    armed.store(true, std::sync::atomic::Ordering::SeqCst);
                }
                released(view, |invocation| attempt_key(invocation) == Some(1)).or_else(|| {
                    released(view, |invocation| {
                        matches!(invocation, InvocationId::Sequence { .. })
                    })
                })
            }),
        );
        let progress = drive(&mut wide, Some(&mut scheduler))
            .expect("the decline fails beta's lineage and the run goes on to its end");
        drop(scheduler);
        assert_eq!(outcome_of(&progress), RunOutcome::Complete);
        let events = wide.env.durable_events();
        assert_eq!(
            terminals_of(&events, SequenceId(1)),
            vec!["merge_verification_interrupted"],
            "the stopped verification has exactly one terminal: {:?}",
            kinds_of(&events)
        );
        assert!(
            !wide.env.runner.ran().iter().any(
                |ran| matches!(ran.invocation, InvocationId::Sequence { role, .. }
                    if role != crate::runner::invocation::SequenceRole::Gate(0))
            ),
            "the verification's review, requested before the decline and applied after it, \
                 never started: {:?}",
            wide.env.runner.endings()
        );
        assert!(wide.run.invocations_balance(), "{:?}", wide.run.warnings());
        replay_equals_live(&wide);
    });
}

#[test]
fn a_halt_after_the_verifications_result_arrived_interrupts_it_and_publishes_nothing() {
    let mut wide = halting_on_gamma("coordinator-halt-after-arrival");
    let polls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    wide.env.answers = std::sync::Arc::new(DecliningAtPoll(std::sync::Arc::clone(&polls)));
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let run_id = wide.run.fold().started().expect("started").run_id.clone();
    let manager = wide.env.fixture.manager.clone();
    let mut last_released: Option<(SequenceId, Option<String>)> = None;
    let mut scheduler = Scheduler::scripted(
        &runner,
        Box::new(|view: &Quiescent<'_>| {
            if !view.run.fold().questions_open() {
                return released(view, |invocation| attempt_key(invocation) == Some(2));
            }
            if count(view.run.events(), "task_merged") == 0 {
                return released(view, |invocation| attempt_key(invocation) == Some(0));
            }
            let Some(open) = view.run.fold().transaction() else {
                return released(view, |invocation| attempt_key(invocation) == Some(1));
            };
            let last = released(view, |invocation| {
                matches!(
                    invocation,
                    InvocationId::Sequence {
                        role: crate::runner::invocation::SequenceRole::ReviewPass(0),
                        ..
                    }
                )
            })?;
            assert_eq!(
                view.live.len(),
                1,
                "the verification is the only live pipeline: {:?}",
                view.live
            );
            assert!(last_released.is_none(), "its last process is released once");
            let pin = crate::engine::topology::integrate::prepared_pin_ref(&run_id, open.sequence);
            last_released = Some((
                open.sequence,
                manager
                    .direct_ref_target(pin.as_str())
                    .expect("the pin is readable"),
            ));
            polls.store(2, std::sync::atomic::Ordering::SeqCst);
            Some(last)
        }),
    );
    let (progress, watching) = drive_watching(&mut wide, &mut scheduler);
    drop(scheduler);
    let progress = progress.expect("the halted run closes and ends");
    assert_eq!(outcome_of(&progress), RunOutcome::Halted);
    let (sequence, pinned) = last_released.expect("the verification's last process was released");
    assert!(
        pinned.is_some(),
        "the verification held its pin while it ran"
    );

    let events = wide.env.durable_events();
    let kinds = kinds_of(&events);
    let halted = position(&kinds, "question_answered", 0);
    assert_eq!(
        &kinds[halted..],
        &[
            "question_answered",
            "merge_verification_charged",
            "merge_verification_interrupted",
            "run_finished"
        ],
        "the verification whose result had arrived is settled interrupted by the closure, \
             never prepared: {kinds:?}"
    );
    assert!(
        !events.iter().any(|event| matches!(&event.body,
            TopologyEventBody::MergePrepared { data } if data.sequence == sequence)
            || matches!(&event.body,
                TopologyEventBody::TaskMerged { data } if data.sequence == sequence)),
        "halting never publishes unverified work: {kinds:?}"
    );
    assert!(
        events.iter().any(|event| matches!(&event.body,
            TopologyEventBody::MergeVerificationInterrupted { data }
                if data.sequence == sequence && data.detail.contains("halted"))),
        "{kinds:?}"
    );
    assert_eq!(
        wide.run.discarded(),
        0,
        "the verification's result was accepted before the halt was ingested, not \
             discarded as a cancelled pipeline's: {:?}",
        wide.run.warnings()
    );
    let verification_endings: Vec<_> = wide
        .env
        .runner
        .endings()
        .into_iter()
        .filter(|(invocation, _)| matches!(invocation, InvocationId::Sequence { .. }))
        .collect();
    assert!(
        !verification_endings.is_empty()
            && verification_endings.iter().all(|(_, ending)| {
                *ending == crate::engine::topology::scaffold::Ending::Completed
            }),
        "every process of the verification ran to its end before the halt: \
             {verification_endings:?}"
    );
    let pin = crate::engine::topology::integrate::prepared_pin_ref(&run_id, sequence);
    assert_eq!(
        manager
            .direct_ref_target(pin.as_str())
            .expect("the pin is readable"),
        None,
        "the prepared pin was deleted expected-old"
    );
    let at_end = watching.intents_when("run_finished");
    assert!(
        !at_end
            .iter()
            .any(|slot| matches!(slot, crate::workspace_manager::Slot::Staging { .. }))
            && snapshot_names(at_end).is_empty(),
        "before `run_finished` the closure had removed the staging and this sequence's \
             snapshots: {at_end:?}"
    );
    assert!(wide.run.fold().transaction().is_none());
    assert!(wide.run.invocations_balance(), "{:?}", wide.run.warnings());
    assert_eq!(wide.run.broker_duplicates(), 0);
    replay_equals_live(&wide);
}

#[test]
fn a_decline_that_cancels_the_open_verification_goes_on_to_integrate_the_queued_candidate() {
    let tasks = [
        WideTask::independent("alpha"),
        WideTask::independent("beta"),
        WideTask::independent("gamma"),
        WideTask::independent("delta"),
    ];
    let armed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut wide = Wide::started_with(
        "coordinator-declined-with-queued",
        &tasks,
        3,
        WidePlans::default(),
        holding(&tasks, &[]),
    );
    wide.env.answers = std::sync::Arc::new(DecliningOnceArmed(std::sync::Arc::clone(&armed)));
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let mut planted: Option<Vec<TopologyEventBody>> = None;
    let mut at_decline: Option<(SequenceId, bool)> = None;
    let mut scheduler = Scheduler::scripted(
        &runner,
        Box::new(|view: &Quiescent<'_>| {
            if count(view.run.events(), "task_merged") == 0 {
                return released(view, |invocation| attempt_key(invocation) == Some(0));
            }
            if at_decline.is_some() {
                return None;
            }
            let Some(open) = view.run.fold().transaction() else {
                return released(view, |invocation| attempt_key(invocation) == Some(1));
            };
            let queued = view
                .run
                .fold()
                .queue()
                .is_some_and(|queue| queue.holds_task(TaskKey(2)));
            if !queued {
                return released(view, |invocation| attempt_key(invocation) == Some(2));
            }
            let events = planted.get_or_insert_with(|| {
                sibling_parked_on(view.run, TaskKey(1), open.candidate.clone())
            });
            if !events.is_empty() {
                return Some(Release::Append(Box::new(events.remove(0))));
            }
            let gate = SequenceIdentities::new(open.sequence).gate(0, 0);
            at_decline = Some((open.sequence, view.invoking.contains(&gate)));
            armed.store(true, std::sync::atomic::Ordering::SeqCst);
            released(view, |invocation| worker(invocation) == Some(TaskKey(3)))
        }),
    );
    let progress = drive(&mut wide, Some(&mut scheduler))
        .expect("the decline fails beta's lineage and the run goes on to its end");
    drop(scheduler);
    let (first, gate_held) = at_decline.expect("the decline was armed");
    assert!(
        gate_held,
        "the verification's gate was held when the decline was armed"
    );
    assert_eq!(outcome_of(&progress), RunOutcome::Complete);
    let events = wide.env.durable_events();
    let kinds = kinds_of(&events);
    let decline = position(&kinds, "question_answered", 0);
    let queued_before = events.iter().take(decline).any(|event| {
        matches!(&event.body,
            TopologyEventBody::TaskCandidateCreated { data } if data.candidate.key == TaskKey(2))
    });
    assert!(
        queued_before,
        "gamma's candidate was queued before the decline: {kinds:?}"
    );
    let after = events.get(decline..).unwrap_or_default();
    let terminal = cancelled_terminal_after(&events, decline, first, LINEAGE_CANCELLED);
    let gamma = after
        .iter()
        .find_map(|event| match &event.body {
            TopologyEventBody::MergePrepared { data } if data.key == TaskKey(2) => {
                Some(data.sequence)
            }
            _ => None,
        })
        .expect("gamma's queued candidate was integrated after the decline");
    assert!(
        after.iter().any(|event| matches!(&event.body,
            TopologyEventBody::TaskMerged { data } if data.sequence == gamma)),
        "{kinds:?}"
    );
    let gamma_prepared = events
        .iter()
        .position(|event| {
            matches!(&event.body,
            TopologyEventBody::MergePrepared { data } if data.sequence == gamma)
        })
        .expect("gamma's publication is in the log");
    assert!(
        terminal < gamma_prepared,
        "sequence 1's terminal precedes gamma's `merge_prepared`: {kinds:?}"
    );
    let gate = SequenceIdentities::new(first).gate(0, 0);
    assert!(
        wide.env
            .runner
            .endings()
            .contains(&(gate, crate::engine::topology::scaffold::Ending::Cancelled)),
        "the verification's gate, held when the decline was ingested, was cancelled: {:?}",
        wide.env.runner.endings()
    );
    assert!(
        wide.run.warnings().iter().any(|warning| {
            warning.contains("was cancelled: a decline, or a failed settlement, failed its lineage")
        }),
        "{:?}",
        wide.run.warnings()
    );
    assert_eq!(
        count(&events, "task_merged"),
        3,
        "alpha, gamma and delta merge; beta's lineage failed: {kinds:?}"
    );
    assert!(wide.run.invocations_balance(), "{:?}", wide.run.warnings());
    replay_equals_live(&wide);
}

const RECOVERY_CANCELLED: &str = "recovery step (f) settles it interrupted, and its \
                                      candidate, whose task failed, is not verified again";

fn failing_sibling(
    run: &TopologyRun,
    root: TaskKey,
    source: CandidateRef,
    halts_run: bool,
) -> Vec<TopologyEventBody> {
    let mut events = sibling_parked_on(run, root, source);
    if let Some(TopologyEventBody::AttemptFinished { data }) = events.last_mut() {
        if let Some(failure) = data.record.failure.as_mut() {
            failure.kind = crate::ladder::FailureKind::GateFailed;
            failure.reason = "the sibling's gates failed and its ladder is spent".to_owned();
        }
        data.settlement = crate::topology::events::AttemptSettlement::Closed {
            transition: crate::topology::events::SettlementTransition::Failed {
                halts_run,
                reason: "the sibling's ladder is spent".to_owned(),
            },
            lease: crate::topology::events::LeaseDisposition::LineageHeld,
        };
    }
    events
}

fn sibling_failed_on(
    run: &TopologyRun,
    root: TaskKey,
    source: CandidateRef,
) -> Vec<TopologyEventBody> {
    failing_sibling(run, root, source, false)
}

fn sibling_failed_halting_on(
    run: &TopologyRun,
    root: TaskKey,
    source: CandidateRef,
) -> Vec<TopologyEventBody> {
    failing_sibling(run, root, source, true)
}

fn sibling_parked_and_declined_on(
    run: &TopologyRun,
    root: TaskKey,
    source: CandidateRef,
) -> Vec<TopologyEventBody> {
    let key = TaskKey(
        u32::try_from(
            run.fold()
                .registry()
                .expect("a started run has a registry")
                .len(),
        )
        .expect("a small registry"),
    );
    let mut events = sibling_parked_on(run, root, source);
    events.push(TopologyEventBody::QuestionAnswered {
        data: crate::topology::events::QuestionAnswered4 {
            key,
            question: crate::ir::QuestionId("sibling".to_owned()),
            answer: crate::topology::events::Answer4::Declined {
                decline_halts_run: false,
            },
            via: "the scheduler declines the sibling's question".to_owned(),
            text: None,
        },
    });
    events
}

fn gamma_or_the_verification(view: &Quiescent<'_>) -> Option<Release> {
    released(view, |invocation| attempt_key(invocation) == Some(2)).or_else(|| {
        released(view, |invocation| {
            matches!(invocation, InvocationId::Sequence { .. })
        })
    })
}

fn open_generations(fold: &TopologyFold) -> usize {
    let tasks = fold.registry().map_or(0, |registry| registry.len());
    (0..tasks)
        .filter_map(|index| u32::try_from(index).ok())
        .filter_map(|index| fold.task(TaskKey(index)))
        .filter(|task| {
            task.generations.iter().any(|generation| {
                matches!(
                    generation.class,
                    crate::topology::fold::GenerationClass::OpenNoAttempt
                        | crate::topology::fold::GenerationClass::InFlight { .. }
                        | crate::topology::fold::GenerationClass::Promoting
                )
            })
        })
        .count()
}

struct CancelSeen {
    kind: &'static str,
    sequence: Option<SequenceId>,
    pinned: bool,
    staging: bool,
    snapshots: Vec<String>,
    merge_held: usize,
    pipeline_held: usize,
    open_generations: usize,
}

struct CancelWatch {
    inner: crate::engine::topology::seams::HarnessTopologyHooks,
    manager: crate::workspace_manager::WorkspaceManager,
    pin: String,
    staging: crate::workspace_manager::Slot,
    seen: Vec<CancelSeen>,
}

impl CancelWatch {
    fn over(wide: &Wide, sequence: SequenceId) -> Self {
        let run_id = wide.run.fold().started().expect("started").run_id.clone();
        Self {
            inner: wide.env.hooks(),
            manager: wide.env.fixture.manager.clone(),
            pin: crate::engine::topology::integrate::prepared_pin_ref(&run_id, sequence)
                .as_str()
                .to_owned(),
            staging: crate::engine::topology::integrate::staging_slot(sequence),
            seen: Vec::new(),
        }
    }
}

impl TopologyHooks for CancelWatch {
    fn effects(&mut self) -> &mut dyn crate::workspace_manager::EffectHooks {
        self.inner.effects()
    }

    fn rundir(&mut self) -> &mut dyn crate::rundir::RunDirHooks {
        self.inner.rundir()
    }

    fn events(&mut self) -> &mut dyn crate::events::log::EventHooks {
        self.inner.events()
    }

    fn container(&mut self) -> &mut dyn crate::runner::container::ContainerHooks {
        self.inner.container()
    }

    fn spawn(&mut self) -> &mut dyn crate::agent::proc::SpawnHooks {
        self.inner.spawn()
    }

    fn folded(&mut self, fold: &TopologyFold, events: &[TopologyEvent]) {
        self.inner.folded(fold, events);
        let Some(last) = events.last() else {
            return;
        };
        let intents = self.manager.intents().expect("the intents are listable");
        self.seen.push(CancelSeen {
            kind: last.body.kind(),
            sequence: sequence_named(last),
            pinned: self
                .manager
                .direct_ref_target(&self.pin)
                .expect("the pin is readable")
                .is_some(),
            staging: intents.contains(&self.staging),
            snapshots: snapshot_names(&intents),
            merge_held: Entitlements::of(fold).merge_held(),
            pipeline_held: fold.pipeline_held(),
            open_generations: open_generations(fold),
        });
    }
}

#[test]
fn a_cancelled_verification_is_settled_interrupted_before_its_residue_is_reclaimed() {
    let tasks = three();
    let armed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut wide = Wide::started_with(
        "coordinator-cancelled-reclaimed",
        &tasks,
        3,
        WidePlans::default(),
        holding(&tasks, &[]),
    );
    wide.env.answers = std::sync::Arc::new(DecliningOnceArmed(std::sync::Arc::clone(&armed)));
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let mut scheduler = cancelling(&runner, sibling_parked_on, move |view| {
        armed.store(true, std::sync::atomic::Ordering::SeqCst);
        gamma_or_the_verification(view)
    });
    let mut watch = CancelWatch::over(&wide, SequenceId(1));
    let pipelines = wide.env.pipelines();
    let progress = wide.run.run_concurrently(
        &wide.env.seams(),
        &pipelines,
        &mut watch,
        Some(&mut scheduler),
    );
    drop(scheduler);
    let progress =
        progress.expect("the decline fails beta's lineage and the run goes on to its end");
    assert_eq!(outcome_of(&progress), RunOutcome::Complete);

    let seen = &watch.seen;
    let folded: Vec<&str> = seen.iter().map(|state| state.kind).collect();
    let decline = position(&folded, "question_answered", 0);
    let terminal = seen
        .iter()
        .position(|state| {
            state.kind == "merge_verification_interrupted" && state.sequence == Some(SequenceId(1))
        })
        .unwrap_or_else(|| panic!("sequence 1 was settled interrupted: {folded:?}"));
    assert!(decline < terminal, "{folded:?}");
    for state in &seen[decline..terminal] {
        assert_eq!(
            (state.merge_held, state.pipeline_held),
            (1, state.open_generations + 1),
            "after `{}`, between the decline and the terminal, the cancelled verification \
                 holds the merge entitlement and one pipeline entitlement: {folded:?}",
            state.kind
        );
    }
    let settled = &seen[terminal];
    assert!(
        settled.pinned
            && settled.staging
            && settled.snapshots.contains(&"s1-integration".to_owned()),
        "the terminal is appended while the pin, the staging worktree and its intent, and \
             the snapshots are still present, so before they are reclaimed: {:?}",
        settled.snapshots
    );
    assert_eq!(
        (settled.merge_held, settled.pipeline_held),
        (0, settled.open_generations),
        "the terminal releases both of the transaction's entitlements"
    );
    for state in &seen[terminal + 1..] {
        assert!(
            !state.pinned
                && !state.staging
                && !state.snapshots.contains(&"s1-integration".to_owned()),
            "the pin, the staging worktree and the snapshots are reclaimed right after the \
                 terminal, before `{}`: {folded:?}",
            state.kind
        );
    }
    assert!(
        seen[terminal + 1..]
            .iter()
            .any(|state| state.kind == "merge_verification_started")
            && folded.last() == Some(&"run_finished"),
        "a later integration and the run's end follow the reclaim: {folded:?}"
    );

    let events = wide.env.durable_events();
    let kinds = kinds_of(&events);
    cancelled_terminal_after(
        &events,
        position(&kinds, "question_answered", 0),
        SequenceId(1),
        LINEAGE_CANCELLED,
    );
    let report: crate::engine::topology::report::TopologyReport = serde_json::from_slice(
        &std::fs::read(wide.env.paths.public.join("report.json")).expect("the report"),
    )
    .expect("the report parses");
    let row = report
        .integration_ledger
        .iter()
        .find(|row| row.sequence == 1)
        .expect("sequence 1 has a ledger row");
    assert_eq!(
        (row.terminal.as_str(), row.review_cost_usd),
        ("merge_verification_interrupted", None),
        "the ledger counts the cancelled verification's one terminal, with unknown spend"
    );
    let rendered = report.render();
    assert!(
        rendered.lines().any(|line| {
            line.trim_start().starts_with("s1 ")
                && line.contains("merge_verification_interrupted")
                && line.ends_with("cost ? (unknown))")
        }),
        "{rendered}"
    );
    assert!(wide.run.invocations_balance(), "{:?}", wide.run.warnings());
    replay_equals_live(&wide);
}

#[test]
fn a_failed_sibling_settlement_settles_the_verification_its_lineage_had_started_interrupted() {
    let tasks = three();
    let mut wide = Wide::started_with(
        "coordinator-cancelled-by-failure",
        &tasks,
        3,
        WidePlans::default(),
        holding(&tasks, &[]),
    );
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let mut scheduler = cancelling(&runner, sibling_failed_on, gamma_or_the_verification);
    let progress = drive(&mut wide, Some(&mut scheduler))
        .expect("the failed settlement fails beta's lineage and the run goes on to its end");
    drop(scheduler);
    assert_eq!(outcome_of(&progress), RunOutcome::Complete);
    let events = wide.env.durable_events();
    let kinds = kinds_of(&events);
    let carrier = events
        .iter()
        .position(|event| {
            matches!(&event.body, TopologyEventBody::AttemptFinished { data }
            if data.key == TaskKey(3)
                && matches!(data.settlement,
                    crate::topology::events::AttemptSettlement::Closed {
                        transition: crate::topology::events::SettlementTransition::Failed { .. },
                        ..
                    }))
        })
        .unwrap_or_else(|| panic!("the sibling's failed settlement was appended: {kinds:?}"));
    cancelled_terminal_after(&events, carrier, SequenceId(1), LINEAGE_CANCELLED);
    assert_eq!(
        terminals_of(&events, SequenceId(1)),
        vec!["merge_verification_interrupted"],
        "{kinds:?}"
    );
    for key in [TaskKey(1), TaskKey(3)] {
        assert_eq!(
            wide.run.fold().task_state(key),
            Some(crate::topology::fold::TaskState::Failed)
        );
    }
    assert_eq!(
        count(&events, "task_merged"),
        2,
        "alpha and gamma merge; beta's lineage failed: {kinds:?}"
    );
    assert!(wide.run.invocations_balance(), "{:?}", wide.run.warnings());
    replay_equals_live(&wide);
}

#[test]
fn a_halting_decline_settles_the_cancelled_verification_in_closure_before_run_finished() {
    let plants: [(&str, Carrier, &str); 2] = [
        ("decline", sibling_parked_on, "question_answered"),
        (
            "failed-settlement",
            sibling_failed_halting_on,
            "attempt_finished",
        ),
    ];
    for (shape, plant, carrier) in plants {
        let tasks = three();
        let armed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut wide = Wide::started_with(
            &format!("coordinator-cancelled-halting-{shape}"),
            &tasks,
            3,
            WidePlans::default(),
            holding(&tasks, &[]),
        );
        wide.env.answers = std::sync::Arc::new(DecliningOnceArmed(std::sync::Arc::clone(&armed)));
        wide.env.halts_run = true;
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let run_id = wide.run.fold().started().expect("started").run_id.clone();
        let pin = crate::engine::topology::integrate::prepared_pin_ref(&run_id, SequenceId(1));
        let manager = wide.env.fixture.manager.clone();
        let mut scheduler = cancelling(&runner, plant, move |view| {
            armed.store(true, std::sync::atomic::Ordering::SeqCst);
            released(view, |invocation| worker(invocation) == Some(TaskKey(2)))
                .or_else(|| gamma_or_the_verification(view))
        });
        let (progress, watching) = drive_watching(&mut wide, &mut scheduler);
        drop(scheduler);
        let progress =
            progress.unwrap_or_else(|error| panic!("{shape}: the halted run ends: {error}"));
        assert_eq!(outcome_of(&progress), RunOutcome::Halted, "{shape}");
        let events = wide.env.durable_events();
        let kinds = kinds_of(&events);
        let at = events
            .iter()
            .rposition(|event| event.body.kind() == carrier)
            .unwrap_or_else(|| panic!("{shape}: the carrier was appended: {kinds:?}"));
        assert_eq!(
            &kinds[at..],
            &[
                carrier,
                "attempt_interrupted",
                "merge_verification_interrupted",
                "run_finished"
            ],
            "{shape}: the closure settles gamma's attempt and then the cancelled \
                 verification, before the run's end: {kinds:?}"
        );
        assert!(
            matches!(&events[at + 1].body,
                TopologyEventBody::AttemptInterrupted { data } if data.key == TaskKey(2)),
            "{shape}: {kinds:?}"
        );
        cancelled_terminal_after(&events, at, SequenceId(1), LINEAGE_CANCELLED);
        assert_eq!(
            manager
                .direct_ref_target(pin.as_str())
                .expect("the pin is readable"),
            None,
            "{shape}: the prepared pin was deleted expected-old"
        );
        let at_end = watching.intents_when("run_finished");
        assert!(
            !at_end
                .iter()
                .any(|slot| matches!(slot, crate::workspace_manager::Slot::Staging { .. }))
                && snapshot_names(at_end).is_empty(),
            "{shape}: before `run_finished` the closure had removed the staging and the \
                 snapshots: {at_end:?}"
        );
        assert!(wide.run.fold().transaction().is_none(), "{shape}");
        assert!(
            wide.run.invocations_balance(),
            "{shape}: {:?}",
            wide.run.warnings()
        );
        replay_equals_live(&wide);
    }
}

fn resumed_into_the_interrupted_terminal(wide: Wide, shape: &str) {
    let tasks = three();
    let run_id = wide.run.fold().started().expect("started").run_id.clone();
    let pin = crate::engine::topology::integrate::prepared_pin_ref(&run_id, SequenceId(1));
    let before = wide.env.durable_events();
    let carried = before.len();
    assert!(
        terminals_of(&before, SequenceId(1)).is_empty(),
        "{shape}: the first process ended with no terminal for the cancelled verification: \
             {:?}",
        kinds_of(&before)
    );
    let (_, mut resumed) = wide
        .resume(
            "inc-2",
            RecordingRunner::new().answering(wide_responder(&tasks, &[])),
            crate::engine::topology::select::Ceiling::unlimited(),
        )
        .unwrap_or_else(|error| panic!("{shape}: the next process resumes: {error}"));
    let events = resumed.env.durable_events();
    let kinds = kinds_of(&events);
    let resumed_at = position(&kinds, "run_resumed", 0);
    let terminal = cancelled_terminal_after(&events, carried, SequenceId(1), RECOVERY_CANCELLED);
    assert!(
        terminal < resumed_at,
        "{shape}: recovery settles the cancelled verification before `run_resumed`: {kinds:?}"
    );
    assert_eq!(
        resumed
            .env
            .fixture
            .manager
            .direct_ref_target(pin.as_str())
            .expect("the pin is readable"),
        None,
        "{shape}: the prepared pin was deleted expected-old"
    );
    let intents = resumed
        .env
        .fixture
        .manager
        .intents()
        .expect("the intents are listable");
    assert!(
        !intents
            .iter()
            .any(|slot| matches!(slot, crate::workspace_manager::Slot::Staging { .. }))
            && !snapshot_names(&intents)
                .iter()
                .any(|name| name.starts_with("s1-")),
        "{shape}: the staging worktree and the sequence's snapshots were reclaimed: \
             {intents:?}"
    );
    let progress = drive(&mut resumed, None)
        .unwrap_or_else(|error| panic!("{shape}: the resumed run completes: {error}"));
    assert_eq!(outcome_of(&progress), RunOutcome::Complete, "{shape}");
    let events = resumed.env.durable_events();
    assert_eq!(
        terminals_of(&events, SequenceId(1)),
        vec!["merge_verification_interrupted"],
        "{shape}: one terminal for sequence 1 across both processes: {:?}",
        kinds_of(&events)
    );
    assert_eq!(count(&events, "task_merged"), 2, "{shape}");
    replay_equals_live(&resumed);
}

struct ShutdownOnDecline {
    inner: crate::engine::topology::seams::HarnessTopologyHooks,
    sender: std::sync::Arc<std::sync::Mutex<Option<mpsc::UnboundedSender<ToCoordinator>>>>,
}

impl TopologyHooks for ShutdownOnDecline {
    fn effects(&mut self) -> &mut dyn crate::workspace_manager::EffectHooks {
        self.inner.effects()
    }

    fn rundir(&mut self) -> &mut dyn crate::rundir::RunDirHooks {
        self.inner.rundir()
    }

    fn events(&mut self) -> &mut dyn crate::events::log::EventHooks {
        self.inner.events()
    }

    fn container(&mut self) -> &mut dyn crate::runner::container::ContainerHooks {
        self.inner.container()
    }

    fn spawn(&mut self) -> &mut dyn crate::agent::proc::SpawnHooks {
        self.inner.spawn()
    }

    fn folded(&mut self, fold: &TopologyFold, events: &[TopologyEvent]) {
        self.inner.folded(fold, events);
        if events
            .last()
            .is_some_and(|event| event.body.kind() == "question_answered")
        {
            if let Some(sender) = self
                .sender
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take()
            {
                assert!(
                    sender.send(ToCoordinator::Shutdown).is_ok(),
                    "the coordinator takes the shutdown"
                );
            }
        }
    }
}

#[test]
fn a_shutdown_after_the_decline_leaves_the_cancelled_verification_to_the_resume() {
    for shape in ["pipeline-running", "pipeline-stopped"] {
        let tasks = three();
        let mut wide = Wide::durable(
            &format!("coordinator-cancelled-shutdown-{shape}"),
            &tasks,
            3,
            WidePlans::default(),
            holding(&tasks, &[]),
        );
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let gate = SequenceIdentities::new(SequenceId(1)).gate(0, 0);
        let error = if shape == "pipeline-running" {
            let mut shut = false;
            let mut scheduler = cancelling(&runner, sibling_parked_and_declined_on, move |view| {
                if shut {
                    return None;
                }
                shut = true;
                assert!(
                    view.invoking.contains(&gate),
                    "the cancelled verification's gate still runs at the shutdown"
                );
                assert!(view.injector.inject(ToCoordinator::Shutdown));
                Some(Release::Injected)
            });
            let error =
                drive(&mut wide, Some(&mut scheduler)).expect_err("a shutdown ends the command");
            drop(scheduler);
            error
        } else {
            let armed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            wide.env.answers =
                std::sync::Arc::new(DecliningOnceArmed(std::sync::Arc::clone(&armed)));
            let sender = std::sync::Arc::new(std::sync::Mutex::new(None));
            let handed = std::sync::Arc::clone(&sender);
            let mut scheduler = cancelling(&runner, sibling_parked_on, move |view| {
                *handed
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) =
                    Some(view.injector.0.clone());
                armed.store(true, std::sync::atomic::Ordering::SeqCst);
                released(view, |invocation| attempt_key(invocation) == Some(2))
            });
            let mut hooks = ShutdownOnDecline {
                inner: wide.env.hooks(),
                sender,
            };
            let pipelines = wide.env.pipelines();
            let error = wide
                .run
                .run_concurrently(
                    &wide.env.seams(),
                    &pipelines,
                    &mut hooks,
                    Some(&mut scheduler),
                )
                .expect_err("a shutdown ends the command");
            drop(scheduler);
            assert!(
                wide.run.warnings().iter().any(|warning| {
                    warning.contains("which the fold no longer holds open; it was cancelled")
                }),
                "{shape}: the coordinator had stopped the cancelled verification's pipeline \
                     before the shutdown: {:?}",
                wide.run.warnings()
            );
            error
        };
        assert!(error.to_string().contains("shut down"), "{shape}: {error}");
        let kinds = kinds_of_log(&wide);
        assert!(
            kinds.contains(&"question_answered"),
            "{shape}: the decline is durable: {kinds:?}"
        );
        resumed_into_the_interrupted_terminal(wide, shape);
    }
}

#[test]
fn a_synced_append_error_on_the_decline_leaves_the_cancelled_verification_to_the_resume() {
    let tasks = three();
    let armed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut wide = Wide::durable(
        "coordinator-cancelled-decline-append-error",
        &tasks,
        3,
        WidePlans::default(),
        holding(&tasks, &[]),
    );
    wide.env.answers = std::sync::Arc::new(DecliningOnceArmed(std::sync::Arc::clone(&armed)));
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let harness = std::sync::Arc::clone(&wide.env.harness);
    let mut scheduler = cancelling(&runner, sibling_parked_on, move |view| {
        if !armed.swap(true, std::sync::atomic::Ordering::SeqCst) {
            harness
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .arm(
                    crate::topology::effects::EffectSiteId::Event(
                        crate::topology::effects::EventSite::Append,
                    ),
                    crate::topology::effects::SubEffectPoint::Synced,
                    crate::topology::effects::InjectionMode::ErrorReturn,
                )
                .expect("the Event append supports an error return after its sync");
        }
        released(view, |invocation| attempt_key(invocation) == Some(2))
    });
    let error =
        drive(&mut wide, Some(&mut scheduler)).expect_err("the append error ends the command");
    drop(scheduler);
    let text = error.to_string();
    assert!(
        text.contains("question_answered") && text.contains("was entered and returned"),
        "the protocol's report names the decline: {text}"
    );
    assert!(
        text.contains("the proven prefix contains the line"),
        "the decline's line survives: {text}"
    );
    assert!(wide.run.fold().is_poisoned());
    assert_eq!(
        kinds_of_log(&wide).last(),
        Some(&"question_answered"),
        "nothing was appended after the error"
    );
    resumed_into_the_interrupted_terminal(wide, "synced-append-error");
}

#[test]
fn an_unresolved_end_in_a_cancelled_verification_ends_the_command_and_the_resume_settles_it() {
    use crate::engine::topology::scaffold::Ending;
    let tasks = three();
    let armed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut wide = Wide::durable(
        "coordinator-cancelled-unresolved",
        &tasks,
        3,
        WidePlans::default(),
        holding(&tasks, &[]),
    );
    wide.env.answers = std::sync::Arc::new(DecliningOnceArmed(std::sync::Arc::clone(&armed)));
    let gate = SequenceIdentities::new(SequenceId(1)).gate(0, 0);
    wide.env.runner.unresolved_when_cancelled(gate.clone());
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let mut scheduler = cancelling(&runner, sibling_parked_on, move |view| {
        armed.store(true, std::sync::atomic::Ordering::SeqCst);
        released(view, |invocation| attempt_key(invocation) == Some(2))
    });
    let error = drive(&mut wide, Some(&mut scheduler))
        .expect_err("nothing is appended over a process that may still run");
    drop(scheduler);
    let message = error.to_string();
    assert!(
        message.contains(&gate.render()) && message.contains("unresolved"),
        "the command ends naming the cancelled gate: {message}"
    );
    assert!(
        wide.env
            .runner
            .endings()
            .contains(&(gate, Ending::CancelledUnresolved)),
        "{:?}",
        wide.env.runner.endings()
    );
    let kinds = kinds_of_log(&wide);
    assert!(
        kinds.contains(&"question_answered"),
        "the decline is durable: {kinds:?}"
    );
    resumed_into_the_interrupted_terminal(wide, "unresolved-end");
}

struct ArmsOnDecline {
    inner: crate::engine::topology::seams::HarnessTopologyHooks,
    harness: std::sync::Arc<std::sync::Mutex<crate::topology::effects::HookHarness>>,
    point: crate::topology::effects::SubEffectPoint,
    declined: bool,
    armed: bool,
}

impl TopologyHooks for ArmsOnDecline {
    fn effects(&mut self) -> &mut dyn crate::workspace_manager::EffectHooks {
        self.inner.effects()
    }

    fn rundir(&mut self) -> &mut dyn crate::rundir::RunDirHooks {
        self.inner.rundir()
    }

    fn events(&mut self) -> &mut dyn crate::events::log::EventHooks {
        self.inner.events()
    }

    fn container(&mut self) -> &mut dyn crate::runner::container::ContainerHooks {
        self.inner.container()
    }

    fn spawn(&mut self) -> &mut dyn crate::agent::proc::SpawnHooks {
        self.inner.spawn()
    }

    fn folded(&mut self, fold: &TopologyFold, events: &[TopologyEvent]) {
        self.inner.folded(fold, events);
        let Some(last) = events.last() else {
            return;
        };
        if last.body.kind() == "question_answered" {
            self.declined = true;
        }
        if !self.armed
            && self.declined
            && matches!(&last.body, TopologyEventBody::MergeVerificationCharged { data }
                if data.sequence == SequenceId(1))
        {
            self.armed = true;
            self.harness
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .arm(
                    crate::topology::effects::EffectSiteId::Event(
                        crate::topology::effects::EventSite::Append,
                    ),
                    self.point,
                    crate::topology::effects::InjectionMode::ErrorReturn,
                )
                .expect("the Event append supports an error return at the point");
        }
    }
}

#[test]
fn a_failed_append_of_the_cancelled_verifications_terminal_resumes_from_the_surviving_prefix() {
    use crate::topology::effects::SubEffectPoint;
    let tasks = [
        WideTask::independent("alpha"),
        WideTask::independent("beta"),
    ];
    for (shape, point, present) in [
        ("sync", SubEffectPoint::Synced, true),
        ("partial-write", SubEffectPoint::Written, false),
    ] {
        let armed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut wide = Wide::durable(
            &format!("coordinator-cancelled-terminal-error-{shape}"),
            &tasks,
            3,
            WidePlans::default(),
            holding(&tasks, &[]),
        );
        wide.env.answers = std::sync::Arc::new(DecliningOnceArmed(std::sync::Arc::clone(&armed)));
        let run_id = wide.run.fold().started().expect("started").run_id.clone();
        let pin = crate::engine::topology::integrate::prepared_pin_ref(&run_id, SequenceId(1));
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let mut scheduler = cancelling(&runner, sibling_parked_on, move |view| {
            armed.store(true, std::sync::atomic::Ordering::SeqCst);
            released(view, |invocation| {
                matches!(invocation, InvocationId::Sequence { .. })
            })
        });
        let mut hooks = ArmsOnDecline {
            inner: wide.env.hooks(),
            harness: std::sync::Arc::clone(&wide.env.harness),
            point,
            declined: false,
            armed: false,
        };
        let pipelines = wide.env.pipelines();
        let result = wide.run.run_concurrently(
            &wide.env.seams(),
            &pipelines,
            &mut hooks,
            Some(&mut scheduler),
        );
        drop(scheduler);
        assert!(
            hooks.armed,
            "{shape}: the verification's `merge_verification_charged` was folded after the \
                 decline, so the fault was armed on the next append, the cancelled verification's \
                 terminal"
        );
        let error = result.expect_err("the terminal's append error ends the command");
        let text = error.to_string();
        assert!(
            text.contains("merge_verification_interrupted")
                && text.contains("was entered and returned"),
            "{shape}: the protocol's report names the cancelled verification's terminal: {text}"
        );
        assert!(
            text.contains(if present {
                "the proven prefix contains the line"
            } else {
                "the proven prefix does not contain the line"
            }),
            "{shape}: {text}"
        );
        let kinds = kinds_of_log(&wide);
        assert_eq!(
            kinds.last(),
            Some(if present {
                &"merge_verification_interrupted"
            } else {
                &"merge_verification_charged"
            }),
            "{shape}: the surviving prefix is the one the report names: {kinds:?}"
        );
        let carried = wide.env.durable_events().len();
        let (_, mut resumed) = wide
            .resume(
                "inc-2",
                RecordingRunner::new().answering(wide_responder(&tasks, &[])),
                crate::engine::topology::select::Ceiling::unlimited(),
            )
            .unwrap_or_else(|error| panic!("{shape}: the next process resumes: {error}"));
        let events = resumed.env.durable_events();
        let kinds = kinds_of(&events);
        let resumed_at = position(&kinds, "run_resumed", 0);
        let appended: Vec<&str> = kinds.get(carried..resumed_at).unwrap_or_default().to_vec();
        assert_eq!(
            appended.contains(&"merge_verification_interrupted"),
            !present,
            "{shape}: recovery appends the terminal only when the surviving prefix lacks it: \
                 {kinds:?}"
        );
        assert_eq!(
            resumed
                .env
                .fixture
                .manager
                .direct_ref_target(pin.as_str())
                .expect("the pin is readable"),
            None,
            "{shape}: the prepared pin is gone after the resume"
        );
        let progress = drive(&mut resumed, None)
            .unwrap_or_else(|error| panic!("{shape}: the resumed run completes: {error}"));
        assert_eq!(outcome_of(&progress), RunOutcome::Complete, "{shape}");
        let events = resumed.env.durable_events();
        assert_eq!(
            terminals_of(&events, SequenceId(1)),
            vec!["merge_verification_interrupted"],
            "{shape}: one terminal for sequence 1: {:?}",
            kinds_of(&events)
        );
        recorded_before_its_terminal(
            &events,
            position(&kinds_of(&events), "question_answered", 0),
            SequenceId(1),
            if present {
                LINEAGE_CANCELLED
            } else {
                RECOVERY_CANCELLED
            },
        );
        replay_equals_live(&resumed);
    }
}

#[test]
fn a_late_passing_result_of_a_cancelled_verification_is_discarded_and_nothing_is_published() {
    let tasks = three();
    let mut wide = Wide::started_with(
        "coordinator-cancelled-late-pass",
        &tasks,
        3,
        WidePlans::default(),
        holding(&tasks, &[]),
    );
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let mut scheduler = cancelling(&runner, sibling_parked_and_declined_on, |view| {
        released(view, |invocation| {
            matches!(invocation, InvocationId::Sequence { .. })
        })
        .or_else(|| released(view, |invocation| attempt_key(invocation) == Some(2)))
    });
    let progress = drive(&mut wide, Some(&mut scheduler))
        .expect("the decline fails beta's lineage and the run goes on to its end");
    drop(scheduler);
    assert_eq!(outcome_of(&progress), RunOutcome::Complete);
    let events = wide.env.durable_events();
    let kinds = kinds_of(&events);
    let gate = SequenceIdentities::new(SequenceId(1)).gate(0, 0);
    assert!(
        wide.env
            .runner
            .endings()
            .contains(&(gate, crate::engine::topology::scaffold::Ending::Completed)),
        "the verification's gate, held at the decline, was released after it to pass: {:?}",
        wide.env.runner.endings()
    );
    assert!(
        !events.iter().any(|event| {
            matches!(&event.body,
                TopologyEventBody::MergePrepared { data } if data.sequence == SequenceId(1))
                || matches!(&event.body,
                    TopologyEventBody::TaskMerged { data } if data.sequence == SequenceId(1))
        }),
        "nothing is published for the cancelled verification: {kinds:?}"
    );
    cancelled_terminal_after(
        &events,
        position(&kinds, "question_answered", 0),
        SequenceId(1),
        LINEAGE_CANCELLED,
    );
    assert!(
        wide.run.discarded() >= 1
            && wide
                .run
                .warnings()
                .iter()
                .any(|warning| warning.contains("its late result will be discarded")),
        "the late result was discarded: {:?}",
        wide.run.warnings()
    );
    assert_eq!(count(&events, "task_merged"), 2, "{kinds:?}");
    assert!(wide.run.invocations_balance(), "{:?}", wide.run.warnings());
    replay_equals_live(&wide);
}

#[test]
fn a_result_that_arrived_before_the_decline_is_dropped_and_its_verification_settles_interrupted() {
    let tasks = three();
    let armed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut wide = Wide::started_with(
        "coordinator-cancelled-after-arrival",
        &tasks,
        3,
        WidePlans::default(),
        holding(&tasks, &[]),
    );
    wide.env.answers = std::sync::Arc::new(DecliningOnceArmed(std::sync::Arc::clone(&armed)));
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let mut planted: Option<Vec<TopologyEventBody>> = None;
    let mut arrived_beside_a_snapshot = false;
    let mut scheduler = Scheduler::scripted(
        &runner,
        Box::new(|view: &Quiescent<'_>| {
            if count(view.run.events(), "task_merged") == 0 {
                return released(view, |invocation| attempt_key(invocation) == Some(0));
            }
            let Some(open) = view.run.fold().transaction() else {
                return released(view, |invocation| attempt_key(invocation) == Some(1));
            };
            let verifying = view
                .live
                .iter()
                .any(|(_, identity)| matches!(identity, Identity::Verification { .. }));
            let gamma_gating = view.invoking.iter().any(|invocation| {
                attempt_key(invocation) == Some(2)
                    && matches!(role_of(invocation), Some(AttemptRole::Gate(_)))
            });
            if verifying {
                if !gamma_gating {
                    return released(view, |invocation| worker(invocation) == Some(TaskKey(2)));
                }
                return released(view, |invocation| {
                    matches!(invocation, InvocationId::Sequence { .. })
                });
            }
            if planted.is_none() {
                arrived_beside_a_snapshot = gamma_gating;
                planted = Some(sibling_parked_on(
                    view.run,
                    TaskKey(1),
                    open.candidate.clone(),
                ));
            }
            if let Some(events) = planted.as_mut() {
                if !events.is_empty() {
                    return Some(Release::Append(Box::new(events.remove(0))));
                }
            }
            armed.store(true, std::sync::atomic::Ordering::SeqCst);
            released(view, |invocation| attempt_key(invocation) == Some(2))
        }),
    );
    let progress = drive(&mut wide, Some(&mut scheduler))
        .expect("the decline fails beta's lineage and the run goes on to its end");
    drop(scheduler);
    assert!(
        arrived_beside_a_snapshot,
        "the verification's result arrived while gamma's gate held its snapshot"
    );
    assert_eq!(outcome_of(&progress), RunOutcome::Complete);
    let events = wide.env.durable_events();
    let kinds = kinds_of(&events);
    assert!(
        !events.iter().any(|event| {
            matches!(&event.body,
                TopologyEventBody::MergePrepared { data } if data.sequence == SequenceId(1))
        }),
        "the arrived result is dropped, never prepared: {kinds:?}"
    );
    cancelled_terminal_after(
        &events,
        position(&kinds, "question_answered", 0),
        SequenceId(1),
        LINEAGE_CANCELLED,
    );
    assert_eq!(
        terminals_of(&events, SequenceId(1)),
        vec!["merge_verification_interrupted"],
        "{kinds:?}"
    );
    recorded_before_its_terminal(
        &events,
        position(&kinds, "question_answered", 0),
        SequenceId(1),
        LINEAGE_CANCELLED,
    );
    assert!(wide.run.invocations_balance(), "{:?}", wide.run.warnings());
    replay_equals_live(&wide);
}

#[test]
fn a_halting_decline_records_the_passes_its_cancelled_verification_charged_before_the_closures_terminal()
 {
    let tasks = three();
    let armed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut wide = Wide::started_with(
        "coordinator-cancelled-halting-recorded",
        &tasks,
        3,
        reviewing(3),
        holding(&tasks, &[]),
    );
    wide.env.answers = std::sync::Arc::new(DecliningOnceArmed(std::sync::Arc::clone(&armed)));
    wide.env.halts_run = true;
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let arming = std::sync::Arc::clone(&armed);
    let mut planted: Option<Vec<TopologyEventBody>> = None;
    let mut scheduler = Scheduler::scripted(
        &runner,
        Box::new(move |view: &Quiescent<'_>| {
            if count(view.run.events(), "task_merged") == 0 {
                return released(view, |invocation| attempt_key(invocation) == Some(0));
            }
            if let Some(events) = planted.as_mut() {
                if !events.is_empty() {
                    return Some(Release::Append(Box::new(events.remove(0))));
                }
                arming.store(true, std::sync::atomic::Ordering::SeqCst);
                return released(view, |invocation| attempt_key(invocation) == Some(2))
                    .or_else(|| released(view, sequence_invocation));
            }
            let Some(open) = view.run.fold().transaction() else {
                return released(view, |invocation| attempt_key(invocation) == Some(1));
            };
            if view
                .invoking
                .iter()
                .any(|invocation| review_pass(invocation, 2))
            {
                planted = Some(sibling_parked_on(
                    view.run,
                    TaskKey(1),
                    open.candidate.clone(),
                ));
                return Some(Release::Append(Box::new(
                    planted.as_mut().expect("planted").remove(0),
                )));
            }
            released(view, sequence_invocation)
        }),
    );
    let progress = drive(&mut wide, Some(&mut scheduler))
        .expect("the halting decline ends the run through its closure");
    drop(scheduler);
    assert_eq!(outcome_of(&progress), RunOutcome::Halted);
    let events = wide.env.durable_events();
    let kinds = kinds_of(&events);
    let decline = position(&kinds, "question_answered", 0);
    let (record, terminal) =
        recorded_before_its_terminal(&events, decline, SequenceId(1), LINEAGE_CANCELLED);
    let interrupted: Vec<usize> = kinds
        .iter()
        .enumerate()
        .filter(|(_, kind)| **kind == "attempt_interrupted")
        .map(|(index, _)| index)
        .collect();
    assert!(
        !interrupted.is_empty()
            && interrupted
                .iter()
                .all(|index| record < *index && *index < terminal),
        "the closure settles the attempts still in flight after the record and before the \
             cancelled verification's terminal: {kinds:?}"
    );
    assert_eq!(
        kinds.last(),
        Some(&"run_finished"),
        "the run's end follows the terminal: {kinds:?}"
    );
    records_both_passes(&wide, SequenceId(1));
    assert!(wide.run.fold().transaction().is_none());
    assert!(wide.run.invocations_balance(), "{:?}", wide.run.warnings());
    replay_equals_live(&wide);
}

#[test]
fn a_registry_stop_after_a_lineage_failure_records_the_cancelled_verifications_passes_and_leaves_its_terminal_to_the_resume()
 {
    let tasks = three();
    let mut wide = Wide::durable(
        "coordinator-cancelled-registry-stop",
        &tasks,
        3,
        reviewing(3),
        holding(&tasks, &[]),
    );
    let mut planted: Option<Vec<TopologyEventBody>> = None;
    let (error, at_shutdown) = stopped_inside_a_retrys_wait(
        &mut wide,
        TearAt::Fold(candidate_created_of_gamma),
        Box::new(move |view: &Quiescent<'_>| {
            if count(view.run.events(), "task_merged") == 0 {
                return released(view, |invocation| attempt_key(invocation) == Some(0));
            }
            if let Some(events) = planted.as_mut() {
                if !events.is_empty() {
                    return Some(Release::Append(Box::new(events.remove(0))));
                }
                return released(view, |invocation| attempt_key(invocation) == Some(2));
            }
            let Some(open) = view.run.fold().transaction() else {
                return released(view, |invocation| attempt_key(invocation) == Some(1));
            };
            let gamma_reviewing = view.invoking.iter().any(|invocation| {
                attempt_key(invocation) == Some(2)
                    && matches!(role_of(invocation), Some(AttemptRole::ReviewPass(_)))
            });
            if gamma_reviewing
                && view
                    .invoking
                    .iter()
                    .any(|invocation| review_pass(invocation, 2))
            {
                planted = Some(sibling_failed_on(
                    view.run,
                    TaskKey(1),
                    open.candidate.clone(),
                ));
                return Some(Release::Append(Box::new(
                    planted.as_mut().expect("planted").remove(0),
                )));
            }
            released(view, |invocation| {
                attempt_key(invocation) == Some(2)
                    && !matches!(role_of(invocation), Some(AttemptRole::ReviewPass(_)))
            })
            .or_else(|| {
                released(view, |invocation| {
                    sequence_invocation(invocation) && !review_pass(invocation, 2)
                })
            })
        }),
    );
    let sequence = SequenceId(1);
    let events = wide.env.durable_events();
    let kinds = kinds_of(&events);
    let carrier = events
        .iter()
        .position(|event| {
            matches!(&event.body, TopologyEventBody::AttemptFinished { data }
            if data.key == TaskKey(3)
                && matches!(data.settlement,
                    crate::topology::events::AttemptSettlement::Closed {
                        transition: crate::topology::events::SettlementTransition::Failed { .. },
                        ..
                    }))
        })
        .unwrap_or_else(|| panic!("the sibling's failed settlement was appended: {kinds:?}"));
    let record = position(&kinds, "merge_verification_charged", 0);
    assert!(
        carrier < record,
        "the lineage failure precedes the record: {kinds:?}"
    );
    let message = error.to_string();
    assert!(
        message.contains("shut down")
            && message.contains(
                "nothing was appended but the record of the review passes a verification \
                     had charged"
            ),
        "the command ends on the shutdown, which says what it appended: {message}"
    );
    let warnings = wide.run.warnings().join("\n");
    assert!(
        warnings.contains("a registry access's wait inside a transition met a shutdown"),
        "gamma's settlement's wait answered the shutdown and stopped the run: {warnings}"
    );
    assert_eq!(
        kinds_of(wide.run.events()).get(at_shutdown..),
        Some(&["merge_verification_charged"][..]),
        "nothing but the record, made past the stop by `finish`, was appended after the \
             wait answered the shutdown"
    );
    records_both_passes(&wide, sequence);
    assert!(
        terminals_of(&events, sequence).is_empty(),
        "no terminal is appended for the cancelled verification in this process: {kinds:?}"
    );
    assert!(
        verifying(wide.run.fold(), sequence),
        "the fold still holds the cancelled verification open: {:?}",
        wide.run.fold().transaction()
    );
    assert_eq!(
        wide.run.fold().task_state(TaskKey(1)),
        Some(crate::topology::fold::TaskState::Failed)
    );
    replay_equals_live(&wide);
    published_as_the_log_authorizes(&wide);
    assert!(wide.run.invocations_balance(), "{warnings}");
    assert!(
        wide.run.stopped().is_none(),
        "the run reopens when its coordinator is gone"
    );
    resumed_into_the_interrupted_terminal(wide, "registry-stop");
}

fn the_decline_kill_resumes_into_the_interrupted_terminal(
    handoff: &std::path::Path,
    temporary: &std::path::Path,
) {
    let mut environment = vec![("UPSTROKE_TEST_KILL_DIR", handoff.as_os_str())];
    environment.extend(crate::engine::topology::scaffold::child_temporary_of(
        temporary,
    ));
    let status = crate::workspace_manager::fixture::run_kill_child_within(
        "engine::topology::coordinator::tests::cancelled_verification_kill_child",
        &environment,
        crate::engine::topology::scaffold::KILL_CHILD_BOUND,
    )
    .expect("the kill child ended in time");
    assert!(
        crate::workspace_manager::fixture::died_by_abort(&status),
        "the child died at the decline's synced append: {status:?}"
    );
    let env = crate::engine::topology::scaffold::WideEnv::adopted(
        the_handed_off_kill_root(handoff),
        &three(),
        3,
        WidePlans::default(),
    );
    let kinds = kinds_of(&env.durable_events());
    assert_eq!(
        kinds.last(),
        Some(&"question_answered"),
        "the child died with the decline synced and nothing after it: {kinds:?}"
    );
    let (_, resumed) = env
        .resume(
            "inc-2",
            RecordingRunner::new().answering(wide_responder(&three(), &[])),
            crate::engine::topology::select::Ceiling::unlimited(),
        )
        .expect("the next process resumes the killed run");
    let resumed_kinds = kinds_of_log(&resumed);
    assert!(resumed_kinds.contains(&"run_resumed"), "{resumed_kinds:?}");
    let carried = kinds.len();
    let events = resumed.env.durable_events();
    let resumed_at = position(&resumed_kinds, "run_resumed", 0);
    let terminal = cancelled_terminal_after(&events, carried, SequenceId(1), RECOVERY_CANCELLED);
    assert!(terminal < resumed_at, "{resumed_kinds:?}");
    let mut resumed = resumed;
    let progress = drive(&mut resumed, None).expect("the resumed run completes");
    assert_eq!(outcome_of(&progress), RunOutcome::Complete);
    let events = resumed.env.durable_events();
    assert_eq!(
        terminals_of(&events, SequenceId(1)),
        vec!["merge_verification_interrupted"],
        "{:?}",
        kinds_of(&events)
    );
    assert_eq!(count(&events, "task_merged"), 2);
    replay_equals_live(&resumed);
}

#[test]
fn a_kill_at_the_declines_synced_append_resumes_into_the_interrupted_terminal() {
    let handoff = crate::engine::topology::scaffold::kill_dir("coordinator-cancelled-kill");
    let temporary = crate::engine::topology::scaffold::kill_dir("tmp");
    the_decline_kill_resumes_into_the_interrupted_terminal(handoff.path(), temporary.path());
}

#[cfg(target_os = "linux")]
#[test]
fn a_kill_at_the_declines_synced_append_resumes_through_a_non_utf8_handoff_directory() {
    let tree = crate::workspace_manager::fixture::scratch("coordinator-kill-handoff");
    let handoff = tree.path().join(a_name_no_string_can_spell());
    crate::workspace_manager::fixture::create_dir(&handoff);
    assert!(
        handoff.to_str().is_none(),
        "the handoff directory's name is one no string can spell: {}",
        handoff.display()
    );
    let temporary = crate::engine::topology::scaffold::kill_dir("tmp");
    the_decline_kill_resumes_into_the_interrupted_terminal(&handoff, temporary.path());
}

#[test]
fn the_kill_handoff_hands_the_parent_back_a_root_no_string_can_spell() {
    let handoff = crate::workspace_manager::fixture::scratch("coordinator-kill-root");
    let root = handoff.path().join(a_name_no_string_can_spell());
    assert!(
        root.to_str().is_none(),
        "the root's name is one no string can spell: {}",
        root.display()
    );
    hand_the_kill_root_off(handoff.path(), &root);
    assert_eq!(
        the_handed_off_kill_root(handoff.path()).as_os_str(),
        root.as_os_str(),
        "the parent reads back the child's root, byte for byte"
    );
}

#[cfg(unix)]
fn the_handed_off_kill_root(handoff: &std::path::Path) -> std::path::PathBuf {
    use std::os::unix::ffi::OsStringExt as _;
    std::path::PathBuf::from(std::ffi::OsString::from_vec(
        std::fs::read(handoff.join(KILL_HANDOFF)).expect("the child left its fixture root"),
    ))
}

#[cfg(windows)]
fn the_handed_off_kill_root(handoff: &std::path::Path) -> std::path::PathBuf {
    use std::os::windows::ffi::OsStringExt as _;
    let bytes = std::fs::read(handoff.join(KILL_HANDOFF)).expect("the child left its fixture root");
    let units: Vec<u16> = bytes
        .chunks(2)
        .map(|pair| match *pair {
            [low, high] => u16::from_le_bytes([low, high]),
            _ => panic!(
                "the handoff holds whole UTF-16 code units, and it is {} bytes",
                bytes.len()
            ),
        })
        .collect();
    std::path::PathBuf::from(std::ffi::OsString::from_wide(&units))
}

#[cfg(unix)]
fn a_name_no_string_can_spell() -> std::ffi::OsString {
    use std::os::unix::ffi::OsStrExt as _;
    std::ffi::OsStr::from_bytes(b"unspellable-\xff").to_os_string()
}

#[cfg(windows)]
fn a_name_no_string_can_spell() -> std::ffi::OsString {
    use std::os::windows::ffi::OsStringExt as _;
    let mut units: Vec<u16> = "unspellable-".encode_utf16().collect();
    units.push(0xD800);
    std::ffi::OsString::from_wide(&units)
}

#[derive(Default)]
struct GrantLog(Vec<InvocationId>);

impl Quiescence for GrantLog {
    fn granted(&mut self, invocation: &InvocationId) {
        self.0.push(invocation.clone());
    }

    fn quiescent(&mut self, _view: &Quiescent<'_>) -> Release {
        Release::Nothing
    }
}

struct Siblings {
    root: InvocationId,
    sibling: InvocationId,
    holder: InvocationId,
}

type Answer = oneshot::Receiver<Result<(), UpstrokeError>>;

struct ClosedSiblings<T> {
    wide: Wide,
    ids: Siblings,
    granted: Vec<InvocationId>,
    root: Answer,
    sibling: Answer,
    found: T,
}

fn with_two_closed_siblings<T>(
    tag: &str,
    body: impl FnOnce(&mut Coordinator<'_>, &Siblings) -> T,
) -> ClosedSiblings<T> {
    let tasks = [
        WideTask::independent("root"),
        WideTask::independent("holder"),
    ];
    let mut wide = Wide::started_with(tag, &tasks, 4, WidePlans::default(), holding(&tasks, &[]));
    wide.run
        .limit_slots(SlotLimits::new(1, 1).expect("positive limits"))
        .expect("an empty broker");
    let armed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    wide.env.answers = std::sync::Arc::new(DecliningOnceArmed(std::sync::Arc::clone(&armed)));
    let mut hooks = wide.env.hooks();
    let seams = wide.env.seams();
    for key in [TaskKey(0), TaskKey(1)] {
        crate::engine::topology::run::begin_dispatch(
            &mut crate::engine::topology::run::Stepping {
                run: &mut wide.run,
                seams: &seams,
                hooks: &mut hooks,
            },
            seams.manager,
            key,
            GenerationId(0),
            false,
        )
        .expect("the root and the unrelated holder start");
    }
    let source = CandidateRef {
        key: TaskKey(0),
        generation: GenerationId(0),
        commit_sha: crate::topology::events::CommitSha(wide.env.head(&wide.run)),
        candidate_ref: crate::topology::events::GitRef(
            "refs/upstroke/planted/sibling-source".to_owned(),
        ),
    };
    let mut running = sibling_parked_on(&wide.run, TaskKey(0), source.clone());
    running.pop();
    for event in running {
        wide.run
            .emit(event, &seams, &mut hooks)
            .expect("a fold-valid running sibling");
    }
    let ids = Siblings {
        root: AttemptIdentities::new(TaskKey(0), GenerationId(0), AttemptNumber(1)).worker(),
        sibling: AttemptIdentities::new(TaskKey(2), GenerationId(0), AttemptNumber(1)).worker(),
        holder: AttemptIdentities::new(TaskKey(1), GenerationId(0), AttemptNumber(1)).worker(),
    };
    for (invocation, agent, pool, admission) in [
        (&ids.holder, "a", None, Admission::Granted),
        (&ids.root, "a", Some("p"), Admission::Pending),
        (&ids.sibling, "b", Some("p"), Admission::Pending),
    ] {
        let standing = Standing::of(wide.run.fold(), invocation);
        let pair = SlotPair {
            agent: agent.to_owned(),
            pool: pool.map(str::to_owned),
        };
        assert_eq!(
            wide.run
                .broker_mut()
                .register(&standing, invocation, Some(pair))
                .expect("the identity is open"),
            admission,
            "`{invocation}`"
        );
    }
    let mut asking = sibling_parked_on(&wide.run, TaskKey(0), source);
    if let Some(TopologyEventBody::TaskSpawned { data }) = asking.first_mut() {
        data.spawn.entry.display_id = crate::ir::TaskId::from(
            crate::topology::registry::repair_display_id(2, &crate::ir::TaskId::from("root"))
                .as_str(),
        );
        if let Some(lineage) = data.spawn.entry.lineage.as_mut() {
            lineage.index = 2;
        }
    }
    for event in asking {
        wide.run
            .emit(event, &seams, &mut hooks)
            .expect("a fold-valid embedded question");
    }
    armed.store(true, std::sync::atomic::Ordering::SeqCst);
    wide.run
        .ingest_answers(&seams, &mut hooks)
        .expect("the decline is ingested")
        .expect("an answer");
    assert!(
        !wide.run.fold().run_is_ending(),
        "the decline does not halt"
    );

    let pipelines = wide.env.pipelines();
    let (outbox, inbox) = mpsc::unbounded_channel();
    let (injector, injected) = mpsc::unbounded_channel();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .build()
        .expect("a runtime");
    let (root_reply, root) = oneshot::channel();
    let (sibling_reply, sibling) = oneshot::channel();
    let live = BTreeMap::from([
        (
            PipelineId(1),
            Live {
                identity: attempt_identity(0),
                cancel: Cancellation::new(),
                cancelled: false,
                busy: Busy::Awaiting,
                running: None,
                job: None,
            },
        ),
        (
            PipelineId(2),
            Live {
                identity: attempt_identity(2),
                cancel: Cancellation::new(),
                cancelled: false,
                busy: Busy::Awaiting,
                running: None,
                job: None,
            },
        ),
        (
            PipelineId(3),
            Live {
                identity: attempt_identity(1),
                cancel: Cancellation::new(),
                cancelled: false,
                busy: Busy::Invoking(ids.holder.clone()),
                running: Some(ids.holder.clone()),
                job: None,
            },
        ),
    ]);
    let replies = BTreeMap::from([
        (ids.root.clone(), (PipelineId(1), root_reply)),
        (ids.sibling.clone(), (PipelineId(2), sibling_reply)),
    ]);
    let mut log = GrantLog::default();
    let found = {
        let mut coordinator = Coordinator {
            run: &mut wide.run,
            seams: &seams,
            hooks: &mut hooks,
            pipelines: &pipelines,
            observer: Some(&mut log),
            leases: Vec::new(),
            live,
            replies,
            gate: SnapshotGate::default(),
            next: 3,
            in_verify: None,
            arrived: None,
            abandoned: None,
            charged: None,
            interrupt: None,
            cancelled_work: closure::Cancelled::none(),
            unresolved: Vec::new(),
            buffer: Vec::new(),
            arrivals: 0,
            deferred: VecDeque::new(),
            wakes: 0,
            ledger: crate::util::DurabilityLedger::off(),
            refusal: None,
            handles: Vec::new(),
            timer: Timer::start(outbox.clone()).expect("the wait timer starts"),
            inbox,
            outbox,
            injected,
            injector: Injector(injector),
            runtime,
        };
        assert!(
            coordinator.live.iter().all(|(pipeline, live)| {
                live.identity.open_in(coordinator.run) == (*pipeline == PipelineId(3))
            }),
            "the decline closed the root's and the sibling's attempts, not the holder's"
        );
        body(&mut coordinator, &ids)
    };
    ClosedSiblings {
        wide,
        ids,
        granted: log.0,
        root,
        sibling,
        found,
    }
}

fn answered(answer: &mut Answer) -> Option<bool> {
    answer.try_recv().ok().map(|result| result.is_ok())
}

#[test]
fn stopping_two_closed_pipelines_grants_neither_the_pair_the_first_one_frees() {
    let mut closed = with_two_closed_siblings("coordinator-reconcile-grants", |coordinator, _| {
        coordinator.reconcile();
        (
            coordinator
                .live
                .get(&PipelineId(2))
                .map(|live| live.cancelled),
            coordinator.run.warnings().to_vec(),
        )
    });
    let (sibling_cancelled, warnings) = closed.found;
    assert_eq!(
        answered(&mut closed.root),
        Some(false),
        "the root's pending request was refused"
    );
    assert_eq!(
        answered(&mut closed.sibling),
        Some(false),
        "the sibling's request, granted the pair the root's withdrawal freed, was refused, \
             not answered with the grant: granted {:?}",
        closed.granted
    );
    assert!(
        closed.granted.is_empty(),
        "no invocation of a closed identity was handed out as granted: {:?}",
        closed.granted
    );
    assert_eq!(sibling_cancelled, Some(true), "the sibling was stopped too");
    assert!(
        warnings.iter().any(|warning| warning.contains(&format!(
            "the broker granted `{}` to pipeline 2",
            closed.ids.sibling
        ))),
        "{warnings:?}"
    );
    let ledger = closed.wide.run.broker_mut().invocations();
    assert!(
        ledger.settled(&closed.ids.root) && ledger.settled(&closed.ids.sibling),
        "both closed requests are settled cancelled: running {:?}, pending {:?}",
        ledger.running(),
        ledger.pending()
    );
    assert!(
        !ledger.slots().holds(&closed.ids.sibling) && ledger.slots().holds(&closed.ids.holder),
        "the sibling holds no pair and the unrelated holder keeps its own"
    );
    assert_eq!(ledger.running(), vec![closed.ids.holder.render().as_str()]);
    assert!(ledger.pending().is_empty());
}

#[test]
fn a_pipeline_whose_identity_closed_is_granted_nothing_before_it_is_stopped() {
    let sibling_gate =
        AttemptIdentities::new(TaskKey(2), GenerationId(0), AttemptNumber(1)).gate(0, 0);
    let mut closed =
        with_two_closed_siblings("coordinator-closed-granted-nothing", |coordinator, ids| {
            let (admit_reply, mut admitted) = oneshot::channel();
            coordinator
                .handle(
                    Origin::Pipeline,
                    ToCoordinator::Admit {
                        pipeline: PipelineId(2),
                        invocation: sibling_gate.clone(),
                        pair: None,
                        reply: admit_reply,
                    },
                )
                .expect("an admission is handled");
            let (snapshot_reply, mut snapshot) = oneshot::channel();
            coordinator
                .handle(
                    Origin::Pipeline,
                    ToCoordinator::SnapshotBegin {
                        pipeline: PipelineId(2),
                        reply: snapshot_reply,
                    },
                )
                .expect("a snapshot request is handled");
            let (waiting_reply, mut waited) = oneshot::channel();
            coordinator.gate.mode = GateMode::Closed;
            coordinator
                .gate
                .waiting
                .push_back((PipelineId(2), waiting_reply));
            coordinator.open_gate();
            coordinator
                .handle(
                    Origin::Pipeline,
                    ToCoordinator::Ended {
                        pipeline: PipelineId(3),
                        invocation: ids.holder.clone(),
                        end: InvocationEnd::Completed,
                    },
                )
                .expect("an end is handled");
            (
                answered(&mut admitted),
                answered(&mut snapshot),
                answered(&mut waited),
                coordinator.gate.live(),
            )
        });
    let (admitted, snapshot, waited, snapshots_held) = closed.found;
    let freed = (answered(&mut closed.root), answered(&mut closed.sibling));
    assert_eq!(
        (
            admitted,
            snapshot,
            waited,
            snapshots_held,
            freed,
            closed.granted.clone()
        ),
        (
            Some(false),
            Some(false),
            Some(false),
            0,
            (Some(false), Some(false)),
            Vec::new()
        ),
        "every grant a closed pipeline asked for or was queued for was refused: its gate's \
             registration, its snapshot request, its queued snapshot when the gate opened, and \
             the pair the holder's end freed (to the closed root, then to the closed sibling); \
             (admitted, snapshot, queued snapshot, snapshots held, (root, sibling), handed out)"
    );
    let ledger = closed.wide.run.broker_mut().invocations();
    assert!(
        ledger.running().is_empty() && ledger.pending().is_empty(),
        "running {:?}, pending {:?}",
        ledger.running(),
        ledger.pending()
    );
    assert!(
        !ledger.settled(&sibling_gate) && ledger.registered() == 3,
        "the closed sibling's gate was never registered"
    );
}

struct Waiting {
    root: InvocationId,
    holder: InvocationId,
    waiter: InvocationId,
    asker: InvocationId,
}

struct BehindADecline<T> {
    wide: Wide,
    ids: Waiting,
    granted: Vec<InvocationId>,
    root: Answer,
    waiter: Answer,
    found: T,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Decline {
    Halting,
    UnderABudgetStop,
}

fn with_a_waiter_behind_a_decline<T>(
    tag: &str,
    decline: Decline,
    body: impl FnOnce(&mut Coordinator<'_>, &Waiting) -> T,
) -> BehindADecline<T> {
    let tasks = [
        WideTask::independent("root"),
        WideTask::independent("holder"),
        WideTask::independent("waiter"),
        WideTask::independent("asker"),
    ];
    let mut wide = Wide::started_with(tag, &tasks, 5, WidePlans::default(), holding(&tasks, &[]));
    wide.run
        .limit_slots(SlotLimits::new(1, 1).expect("positive limits"))
        .expect("an empty broker");
    let armed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    wide.env.answers = std::sync::Arc::new(DecliningOnceArmed(std::sync::Arc::clone(&armed)));
    wide.env.halts_run = decline == Decline::Halting;
    let mut hooks = wide.env.hooks();
    let seams = wide.env.seams();
    for key in [TaskKey(0), TaskKey(1), TaskKey(2), TaskKey(3)] {
        crate::engine::topology::run::begin_dispatch(
            &mut crate::engine::topology::run::Stepping {
                run: &mut wide.run,
                seams: &seams,
                hooks: &mut hooks,
            },
            seams.manager,
            key,
            GenerationId(0),
            false,
        )
        .expect("four unrelated attempts start");
    }
    let worker =
        |key| AttemptIdentities::new(TaskKey(key), GenerationId(0), AttemptNumber(1)).worker();
    let ids = Waiting {
        root: worker(0),
        holder: worker(1),
        waiter: worker(2),
        asker: worker(3),
    };
    for (invocation, agent, pool, admission) in [
        (&ids.holder, "a", None, Admission::Granted),
        (&ids.root, "a", Some("p"), Admission::Pending),
        (&ids.waiter, "b", Some("p"), Admission::Pending),
    ] {
        let standing = Standing::of(wide.run.fold(), invocation);
        let pair = SlotPair {
            agent: agent.to_owned(),
            pool: pool.map(str::to_owned),
        };
        assert_eq!(
            wide.run
                .broker_mut()
                .register(&standing, invocation, Some(pair))
                .expect("the identity is open"),
            admission,
            "`{invocation}`"
        );
    }
    let source = CandidateRef {
        key: TaskKey(0),
        generation: GenerationId(0),
        commit_sha: crate::topology::events::CommitSha(wide.env.head(&wide.run)),
        candidate_ref: crate::topology::events::GitRef(
            "refs/upstroke/planted/sibling-source".to_owned(),
        ),
    };
    for event in sibling_parked_on(&wide.run, TaskKey(0), source) {
        wide.run
            .emit(event, &seams, &mut hooks)
            .expect("a fold-valid embedded question");
    }
    armed.store(true, std::sync::atomic::Ordering::SeqCst);
    wide.run
        .ingest_answers(&seams, &mut hooks)
        .expect("the decline is ingested")
        .expect("an answer");
    if decline == Decline::UnderABudgetStop {
        let epoch = wide.run.fold().epoch().expect("started");
        wide.run
            .emit(
                TopologyEventBody::BudgetExceeded {
                    data: crate::topology::events::BudgetExceeded4 {
                        epoch,
                        budget: crate::events::BudgetKind::Run,
                        limit_usd: 0.4,
                        spent_usd: 0.5,
                        key: None,
                    },
                },
                &seams,
                &mut hooks,
            )
            .expect("the fold takes the budget stop after the decline");
    }
    let fold = wide.run.fold();
    assert_eq!(
        (
            fold.run_is_ending(),
            fold.halted_at(),
            fold.budget_stop().is_some()
        ),
        match decline {
            Decline::Halting => (true, Some(TaskKey(4)), false),
            Decline::UnderABudgetStop => (true, None, true),
        },
        "the decline halts the run, or a budget stop follows a decline that does not"
    );

    let pipelines = wide.env.pipelines();
    let (outbox, inbox) = mpsc::unbounded_channel();
    let (injector, injected) = mpsc::unbounded_channel();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .build()
        .expect("a runtime");
    let (root_reply, root) = oneshot::channel();
    let (waiter_reply, waiter) = oneshot::channel();
    let entry = |key, busy, running| Live {
        identity: attempt_identity(key),
        cancel: Cancellation::new(),
        cancelled: false,
        busy,
        running,
        job: None,
    };
    let live = BTreeMap::from([
        (PipelineId(1), entry(0, Busy::Awaiting, None)),
        (PipelineId(2), entry(2, Busy::Awaiting, None)),
        (
            PipelineId(3),
            entry(
                1,
                Busy::Invoking(ids.holder.clone()),
                Some(ids.holder.clone()),
            ),
        ),
        (PipelineId(4), entry(3, Busy::Running, None)),
    ]);
    let replies = BTreeMap::from([
        (ids.root.clone(), (PipelineId(1), root_reply)),
        (ids.waiter.clone(), (PipelineId(2), waiter_reply)),
    ]);
    let mut log = GrantLog::default();
    let found = {
        let mut coordinator = Coordinator {
            run: &mut wide.run,
            seams: &seams,
            hooks: &mut hooks,
            pipelines: &pipelines,
            observer: Some(&mut log),
            leases: Vec::new(),
            live,
            replies,
            gate: SnapshotGate::default(),
            next: 4,
            in_verify: None,
            arrived: None,
            abandoned: None,
            charged: None,
            interrupt: None,
            cancelled_work: closure::Cancelled::none(),
            unresolved: Vec::new(),
            buffer: Vec::new(),
            arrivals: 0,
            deferred: VecDeque::new(),
            wakes: 0,
            ledger: crate::util::DurabilityLedger::off(),
            refusal: None,
            handles: Vec::new(),
            timer: Timer::start(outbox.clone()).expect("the wait timer starts"),
            inbox,
            outbox,
            injected,
            injector: Injector(injector),
            runtime,
        };
        assert!(
            coordinator.live.iter().all(|(pipeline, live)| {
                live.identity.open_in(coordinator.run) == (*pipeline != PipelineId(1))
            }),
            "the decline closed the root's attempt and none of the three unrelated ones"
        );
        body(&mut coordinator, &ids)
    };
    BehindADecline {
        wide,
        ids,
        granted: log.0,
        root,
        waiter,
        found,
    }
}

fn broker_grants(coordinator: &mut Coordinator<'_>) -> u32 {
    coordinator.run.broker_mut().invocations().slots().granted()
}

#[test]
fn a_recorded_halt_withdraws_every_waiting_request_before_reconciliation_frees_a_pair() {
    let mut behind = with_a_waiter_behind_a_decline(
        "coordinator-halt-before-reconcile",
        Decline::Halting,
        |coordinator, _| {
            let before = broker_grants(coordinator);
            coordinator.admit().expect("the pass acts on the halt");
            let after = broker_grants(coordinator);
            let vouched: Vec<(String, bool)> = closure::in_flight(coordinator.run.fold())
                .iter()
                .map(|item| (item.describe(), coordinator.cancelled_work.vouches(item)))
                .collect();
            (
                matches!(coordinator.interrupt, Some(Interrupt::Halt)),
                coordinator
                    .live
                    .iter()
                    .filter(|(_, live)| !live.cancelled)
                    .map(|(pipeline, _)| pipeline.0)
                    .collect::<Vec<_>>(),
                (before, after),
                vouched,
                coordinator.run.warnings().to_vec(),
            )
        },
    );
    let (halted, uncancelled, grants, vouched, warnings) = behind.found;
    assert_eq!(
        (
            halted,
            uncancelled,
            grants,
            answered(&mut behind.root),
            answered(&mut behind.waiter),
            behind.granted.clone()
        ),
        (
            true,
            Vec::new(),
            (1, 1),
            Some(false),
            Some(false),
            Vec::new()
        ),
        "the halt the fold records is acted on before reconciliation: every live pipeline is \
             cancelled and both waiting requests are withdrawn while pending and refused, so the \
             broker grants nothing and nothing is handed out; (halted, uncancelled pipelines, \
             broker grants before and after the pass, root, waiter, handed out)"
    );
    assert!(
        vouched.len() == 3 && vouched.iter().all(|(_, vouched)| *vouched),
        "the holder's, the waiter's and the asker's attempts are in flight and vouched for \
             the closure: {vouched:?}"
    );
    assert!(
        !warnings
            .iter()
            .any(|warning| warning.contains("the broker granted")),
        "no grant was computed to be withdrawn: {warnings:?}"
    );
    let ledger = behind.wide.run.broker_mut().invocations();
    assert!(
        ledger.settled(&behind.ids.root) && ledger.settled(&behind.ids.waiter),
        "both waiting requests are settled cancelled: running {:?}, pending {:?}",
        ledger.running(),
        ledger.pending()
    );
    assert_eq!(
        (ledger.running(), ledger.pending().is_empty()),
        (vec![behind.ids.holder.render().as_str()], true),
        "only the holder's invocation runs, to be released by its own end"
    );
    assert!(!ledger.slots().holds(&behind.ids.waiter));
}

#[test]
fn a_budget_stop_drains_so_the_pair_reconciliation_frees_reaches_an_open_waiter() {
    let mut behind = with_a_waiter_behind_a_decline(
        "coordinator-budget-before-reconcile",
        Decline::UnderABudgetStop,
        |coordinator, _| {
            coordinator.admit().expect("the pass drains");
            (
                coordinator.interrupt.is_some(),
                coordinator
                    .live
                    .iter()
                    .filter(|(_, live)| live.cancelled)
                    .map(|(pipeline, _)| pipeline.0)
                    .collect::<Vec<_>>(),
            )
        },
    );
    let (interrupted, cancelled) = behind.found;
    assert_eq!(
        (
            interrupted,
            cancelled,
            answered(&mut behind.root),
            answered(&mut behind.waiter),
            behind.granted.clone()
        ),
        (
            false,
            vec![1],
            Some(false),
            Some(true),
            vec![behind.ids.waiter.clone()]
        ),
        "a budget stop is not a halt: the pass stops only the root, whose identity the \
             decline closed, and the pair its withdrawal frees is granted to the open waiter, \
             which drains to its settlement; (interrupted, cancelled pipelines, root, waiter, \
             handed out)"
    );
    let ledger = behind.wide.run.broker_mut().invocations();
    assert_eq!(
        (
            ledger.settled(&behind.ids.root),
            ledger.running(),
            ledger.pending().is_empty()
        ),
        (
            true,
            vec![
                behind.ids.holder.render().as_str(),
                behind.ids.waiter.render().as_str()
            ],
            true
        )
    );
}

fn grant_sites(
    coordinator: &mut Coordinator<'_>,
    ids: &Waiting,
) -> (Option<bool>, Option<bool>, Option<bool>, u32) {
    let (admit_reply, mut admitted) = oneshot::channel();
    coordinator
        .handle(
            Origin::Pipeline,
            ToCoordinator::Admit {
                pipeline: PipelineId(4),
                invocation: ids.asker.clone(),
                pair: Some(SlotPair {
                    agent: "c".to_owned(),
                    pool: None,
                }),
                reply: admit_reply,
            },
        )
        .expect("an admission is handled");
    let (snapshot_reply, mut snapshot) = oneshot::channel();
    coordinator
        .handle(
            Origin::Pipeline,
            ToCoordinator::SnapshotBegin {
                pipeline: PipelineId(4),
                reply: snapshot_reply,
            },
        )
        .expect("a snapshot request is handled");
    let (waiting_reply, mut waited) = oneshot::channel();
    coordinator.gate.mode = GateMode::Closed;
    coordinator
        .gate
        .waiting
        .push_back((PipelineId(4), waiting_reply));
    coordinator.open_gate();
    coordinator
        .handle(
            Origin::Pipeline,
            ToCoordinator::Ended {
                pipeline: PipelineId(3),
                invocation: ids.holder.clone(),
                end: InvocationEnd::Completed,
            },
        )
        .expect("an end is handled");
    (
        answered(&mut admitted),
        answered(&mut snapshot),
        answered(&mut waited),
        coordinator.gate.live(),
    )
}

#[test]
fn no_grant_reaches_an_open_pipeline_between_a_recorded_halt_and_the_pass_that_acts_on_it() {
    let mut behind = with_a_waiter_behind_a_decline(
        "coordinator-halted-grant-sites",
        Decline::Halting,
        grant_sites,
    );
    let (admitted, snapshot, waited, snapshots_held) = behind.found;
    let freed = (answered(&mut behind.root), answered(&mut behind.waiter));
    assert_eq!(
        (
            admitted,
            snapshot,
            waited,
            snapshots_held,
            freed,
            behind.granted.clone()
        ),
        (
            Some(false),
            Some(false),
            Some(false),
            0,
            (Some(false), Some(false)),
            Vec::new()
        ),
        "with the halt recorded and not yet acted on, every grant an open pipeline asked for \
             or was queued for was refused: the asker's slotted registration, its snapshot \
             request, its queued snapshot when the gate opened, and the pair the holder's end \
             freed (to the closed root, then, withdrawn, to the open waiter); (admitted, \
             snapshot, queued snapshot, snapshots held, (root, waiter), handed out)"
    );
    let ledger = behind.wide.run.broker_mut().invocations();
    assert!(
        ledger.running().is_empty() && ledger.pending().is_empty(),
        "running {:?}, pending {:?}",
        ledger.running(),
        ledger.pending()
    );
    assert!(
        !ledger.settled(&behind.ids.asker) && ledger.registered() == 3,
        "the asker's worker was never registered"
    );
}

#[test]
fn under_a_budget_stop_every_grant_site_still_grants_an_open_pipeline() {
    let mut behind = with_a_waiter_behind_a_decline(
        "coordinator-budget-grant-sites",
        Decline::UnderABudgetStop,
        grant_sites,
    );
    let (admitted, snapshot, waited, snapshots_held) = behind.found;
    let freed = (answered(&mut behind.root), answered(&mut behind.waiter));
    assert_eq!(
        (
            admitted,
            snapshot,
            waited,
            snapshots_held,
            freed,
            behind.granted.clone()
        ),
        (
            Some(true),
            Some(true),
            Some(true),
            2,
            (Some(false), Some(true)),
            vec![behind.ids.asker.clone(), behind.ids.waiter.clone()]
        ),
        "a budget stop drains, so every grant an open pipeline asks for or waits for is \
             delivered and only the closed root is refused; (admitted, snapshot, queued \
             snapshot, snapshots held, (root, waiter), handed out)"
    );
    let ledger = behind.wide.run.broker_mut().invocations();
    assert_eq!(
        ledger.running(),
        vec![
            behind.ids.waiter.render().as_str(),
            behind.ids.asker.render().as_str()
        ]
    );
}
