//! Extended notes: `docs/internals/engine/topology/coordinator/tests/verification_charges.md`

use super::*;

const RECORD_KILL_TEMPORARY: &str = "tmp";

fn charged_event(sequence: SequenceId, reviews: &[ReviewRecord]) -> TopologyEvent {
    serde_json::from_value(serde_json::json!({
        "ts": "2026-10-08T00:00:00.000Z",
        "event": "merge_verification_charged",
        "data": {"sequence": sequence.0, "reviews": reviews},
    }))
    .expect("the vocabulary has merge_verification_charged")
}

fn started_at(events: &[TopologyEvent], sequence: SequenceId) -> usize {
    events
        .iter()
        .position(|event| {
            matches!(&event.body,
                TopologyEventBody::MergeVerificationStarted { data } if data.sequence == sequence)
        })
        .unwrap_or_else(|| panic!("sequence {} has a start", sequence.0))
}

fn alpha_then_beta(runner: &RecordingRunner) -> Scheduler<'_> {
    Scheduler::scripted(
        runner,
        Box::new(|view: &Quiescent<'_>| {
            if count(view.run.events(), "task_merged") == 0 {
                return released(view, |invocation| attempt_key(invocation) == Some(0));
            }
            released(view, |invocation| attempt_key(invocation) == Some(1))
                .or_else(|| released(view, sequence_invocation))
        }),
    )
}

fn toward_arrival(view: &Quiescent<'_>) -> Option<Option<Release>> {
    if count(view.run.events(), "task_merged") == 0 {
        return Some(released(view, |invocation| {
            attempt_key(invocation) == Some(0)
        }));
    }
    if view.run.fold().transaction().is_none() {
        return Some(released(view, |invocation| {
            attempt_key(invocation) == Some(1)
        }));
    }
    if verification_live(view) {
        return Some(
            released(view, |invocation| worker(invocation) == Some(TaskKey(2)))
                .or_else(|| released(view, sequence_invocation)),
        );
    }
    None
}

fn gamma_gate(view: &Quiescent<'_>) -> Option<InvocationId> {
    view.invoking
        .iter()
        .find(|invocation| {
            attempt_key(invocation) == Some(2)
                && matches!(role_of(invocation), Some(AttemptRole::Gate(_)))
        })
        .cloned()
}

fn halting_on_gamma_reviewing(tag: &str, verify_reviewers: usize, durable: bool) -> Wide {
    let tasks = three();
    let runner = RecordingRunner::new().answering(
        crate::engine::topology::scaffold::wide_responder_asking(&tasks, &[], &[2]),
    );
    runner.hold();
    let mut wide = if durable {
        Wide::durable(tag, &tasks, 3, reviewing(verify_reviewers), runner)
    } else {
        Wide::started_with(tag, &tasks, 3, reviewing(verify_reviewers), runner)
    };
    halting(&mut wide.env);
    wide
}

fn halting_at_the_third_pass(runner: &RecordingRunner) -> Scheduler<'_> {
    Scheduler::scripted(
        runner,
        Box::new(|view: &Quiescent<'_>| {
            if count(view.run.events(), "task_merged") == 0 {
                return released(view, |invocation| attempt_key(invocation) == Some(0));
            }
            if view.run.fold().transaction().is_none() {
                return released(view, |invocation| attempt_key(invocation) == Some(1));
            }
            if view
                .invoking
                .iter()
                .any(|invocation| review_pass(invocation, 2))
            {
                return released(view, |invocation| attempt_key(invocation) == Some(2));
            }
            released(view, sequence_invocation)
        }),
    )
}

fn pipelines_failing_the_snapshot_after(
    wide: &Wide,
    passing: u32,
    arm: Option<(
        crate::topology::effects::SubEffectPoint,
        crate::topology::effects::InjectionMode,
    )>,
) -> PipelineSeams {
    let mut pipelines = wide.env.pipelines();
    let harness = std::sync::Arc::clone(&wide.env.harness);
    pipelines.hooks = std::sync::Arc::new(move || {
        Box::new(ArmingOnTheFault {
            inner: crate::engine::topology::seams::HarnessTopologyHooks::new(
                std::sync::Arc::clone(&harness),
            ),
            effects: FailingOnce {
                effects: crate::workspace_manager::HarnessEffects::new(std::sync::Arc::clone(
                    &harness,
                )),
                fail_at: Some((snapshot_add(), passing)),
            },
            harness: std::sync::Arc::clone(&harness),
            arm,
            abort_at_the_next_append: false,
        }) as Box<dyn TopologyHooks + Send>
    });
    pipelines
}

fn ledger_row(
    wide: &Wide,
    sequence: SequenceId,
) -> (crate::engine::topology::report::LedgerRow, String) {
    let run_id = wide.run.fold().started().expect("started").run_id.clone();
    let report = crate::engine::topology::report::TopologyReport::derive(
        &run_id,
        wide.run.fold(),
        &wide.env.durable_events(),
    )
    .expect("the report derives");
    let row = report
        .integration_ledger
        .iter()
        .find(|row| row.sequence == sequence.0)
        .cloned()
        .expect("the sequence has a ledger row");
    (row, report.render())
}

#[test]
fn a_verifications_own_error_after_two_paid_passes_is_recorded_before_the_step_ends_at_width_one() {
    let mut wide = stale_at_width_one("coordinator-o14-r1", 3, unlimited());
    let mut hooks = failing_the_snapshot_after(&wide, 3);
    let seams = wide.env.seams();
    let error = wide
        .run
        .step(&seams, &mut hooks)
        .expect_err("the third reviewer's snapshot add fails");
    assert!(
        error.to_string().contains("was made to fail"),
        "the verification's own error ends the step: {error}"
    );
    let events = wide.env.durable_events();
    let sequence = SequenceId(2);
    assert_eq!(
        kinds_of(&events).last(),
        Some(&"merge_verification_charged"),
        "the record is the step's last append: {:?}",
        kinds_of(&events)
    );
    records_both_passes(&wide, sequence);
    assert!(
        verifying(wide.run.fold(), sequence),
        "the fold still holds sequence 2 open: {:?}",
        wide.run.fold().transaction()
    );
}

#[test]
fn a_restart_after_two_recorded_passes_still_refuses_the_ceiling() {
    let mut wide = stale_at_width_one("coordinator-o14-r2", 3, unlimited());
    let before = wide.run.spend().run_usd();
    let mut hooks = failing_the_snapshot_after(&wide, 3);
    let seams = wide.env.seams();
    wide.run
        .step(&seams, &mut hooks)
        .expect_err("the third reviewer's snapshot add fails");
    drop(hooks);
    let live = wide.run.spend().run_usd();
    assert!(
        (live - before - 2.0 * PASS_COST).abs() < 1e-9,
        "the step charged two passes live: {before} -> {live}"
    );
    records_both_passes(&wide, SequenceId(2));
    let limit = before + 1.5 * PASS_COST;
    let (_, mut resumed) = wide
        .resume(
            "inc-3",
            answering_two(),
            crate::engine::topology::select::Ceiling {
                run_usd: Some(limit),
                task_usd: None,
            },
        )
        .expect("the next process resumes");
    let events = resumed.env.durable_events();
    let kinds = kinds_of(&events);
    assert_eq!(
        terminals_of(&events, SequenceId(2)),
        vec!["merge_verification_interrupted"],
        "recovery settles sequence 2 interrupted: {kinds:?}"
    );
    assert!(
        position(&kinds, "merge_verification_charged", 0)
            < position(&kinds, "merge_verification_interrupted", 1),
        "recovery's interrupted terminal follows the record: {kinds:?}"
    );
    let mut hooks = resumed.env.hooks();
    let seams = resumed.env.seams();
    let progress = resumed
        .run
        .step(&seams, &mut hooks)
        .expect("the first step of the restart");
    assert!(
        matches!(progress, Progress::BudgetExceeded),
        "the restart is refused where the incarnation before it was: {progress:?} (limit \
             {limit}, replayed {})",
        resumed.run.spend().run_usd()
    );
    assert!(
        !resumed
            .env
            .runner
            .ran()
            .iter()
            .any(|ran| review_pass(&ran.invocation, 0)),
        "no reviewer runs in the restart"
    );
}

#[test]
fn an_unresolved_reviewer_after_two_paid_passes_is_recorded_and_its_transaction_stays_open_at_width_one()
 {
    let mut wide = stale_at_width_one("coordinator-o14-r3", 3, unlimited());
    let sequence = SequenceId(2);
    wide.env
        .runner
        .ending_unresolved(SequenceIdentities::new(sequence).review_pass(2, 0));
    let mut hooks = wide.env.hooks();
    let seams = wide.env.seams();
    let error = wide
        .run
        .step(&seams, &mut hooks)
        .expect_err("the third reviewer's process ends unresolved");
    assert!(
        error.to_string().contains("could not establish"),
        "the unresolved reviewer's error ends the step: {error}"
    );
    let events = wide.env.durable_events();
    assert_eq!(
        kinds_of(&events).last(),
        Some(&"merge_verification_charged"),
        "{:?}",
        kinds_of(&events)
    );
    records_both_passes(&wide, sequence);
    assert!(
        terminals_of(&events, sequence).is_empty(),
        "no terminal settles the verification while its process may run: {:?}",
        kinds_of(&events)
    );
    assert!(verifying(wide.run.fold(), sequence));
    assert_eq!(
        crate::engine::topology::select::Entitlements::of(wide.run.fold()).merge_held(),
        1,
        "the open transaction still holds its entitlement"
    );
    let replayed = crate::engine::topology::select::Spend::replay(&events);
    let (_, resumed) = wide
        .resume("inc-3", answering_two(), unlimited())
        .expect("the next process resumes");
    let events = resumed.env.durable_events();
    assert_eq!(
        terminals_of(&events, sequence),
        vec!["merge_verification_interrupted"],
        "the census and recovery settle it interrupted: {:?}",
        kinds_of(&events)
    );
    let again = crate::engine::topology::select::Spend::replay(&events);
    let key = started_key(&events, sequence);
    assert!(
        (again.run_usd() - replayed.run_usd()).abs() < 1e-9
            && (again.task_usd(key) - replayed.task_usd(key)).abs() < 1e-9,
        "the interrupted terminal adds no cost and the record is counted once: {} -> {}",
        replayed.run_usd(),
        again.run_usd()
    );
    records_both_passes(&resumed, sequence);
}

#[test]
fn an_unresolved_reviewer_after_two_paid_passes_is_recorded_at_width_three() {
    let tasks = two();
    let mut wide = Wide::started_with(
        "coordinator-o14-r4",
        &tasks,
        3,
        reviewing(3),
        holding(&tasks, &[]),
    );
    let sequence = SequenceId(1);
    let third = SequenceIdentities::new(sequence).review_pass(2, 0);
    wide.env.runner.ending_unresolved(third.clone());
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let mut scheduler = alpha_then_beta(&runner);
    let error =
        drive(&mut wide, Some(&mut scheduler)).expect_err("the unresolved end fails the command");
    drop(scheduler);
    assert!(
        error.to_string().contains("unresolved"),
        "the unresolved end ends the command: {error}"
    );
    records_both_passes(&wide, sequence);
    let events = wide.env.durable_events();
    assert!(
        terminals_of(&events, sequence).is_empty(),
        "{:?}",
        kinds_of(&events)
    );
    settled_but(&mut wide.run, std::slice::from_ref(&third)).unwrap_or_else(|ledger| {
        panic!("the unresolved reviewer keeps its registration: {ledger}")
    });
    replay_equals_live(&wide);
}

#[test]
fn a_halt_after_the_verifications_two_pass_result_arrived_records_both_before_the_interrupted_terminal()
 {
    let mut wide = halting_on_gamma_reviewing("coordinator-o14-r5", 2, false);
    let polls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    wide.env.answers = std::sync::Arc::new(DecliningAtPoll(std::sync::Arc::clone(&polls)));
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let mut scheduler = Scheduler::scripted(
        &runner,
        Box::new(|view: &Quiescent<'_>| {
            if !view.run.fold().questions_open() {
                return released(view, |invocation| attempt_key(invocation) == Some(2));
            }
            if count(view.run.events(), "task_merged") == 0 {
                return released(view, |invocation| attempt_key(invocation) == Some(0));
            }
            if view.run.fold().transaction().is_none() {
                return released(view, |invocation| attempt_key(invocation) == Some(1));
            }
            let last = released(view, |invocation| review_pass(invocation, 1));
            if last.is_some() {
                polls.store(2, std::sync::atomic::Ordering::SeqCst);
                return last;
            }
            released(view, sequence_invocation)
        }),
    );
    let progress = drive(&mut wide, Some(&mut scheduler)).expect("the halted run ends");
    drop(scheduler);
    assert_eq!(outcome_of(&progress), RunOutcome::Halted);
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
        "{kinds:?}"
    );
    assert_eq!(
        wide.run.discarded(),
        0,
        "the result was accepted before the halt: {:?}",
        wide.run.warnings()
    );
    let sequence = SequenceId(1);
    records_both_passes(&wide, sequence);
    let (row, rendered) = ledger_row(&wide, sequence);
    assert_eq!(
        (
            row.reviews,
            row.terminal.as_str(),
            row.review_cost_incomplete
        ),
        (2, "merge_verification_interrupted", true),
        "{row:?}"
    );
    assert!(
        rendered.contains("(2 review(s), cost at least $0.5000 (?))"),
        "the report reads the record as a lower bound: {rendered}"
    );
    replay_equals_live(&wide);
}

#[test]
fn a_shutdown_after_the_verifications_two_pass_result_arrived_records_both() {
    let tasks = three();
    let mut wide = Wide::started_with(
        "coordinator-o14-r6",
        &tasks,
        3,
        reviewing(2),
        holding(&tasks, &[]),
    );
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let mut shut = false;
    let mut scheduler = Scheduler::scripted(
        &runner,
        Box::new(move |view: &Quiescent<'_>| {
            if shut {
                return None;
            }
            if let Some(release) = toward_arrival(view) {
                return release;
            }
            assert!(
                gamma_gate(view).is_some(),
                "gamma's gate holds its snapshot"
            );
            shut = true;
            assert!(view.injector.inject(ToCoordinator::Shutdown));
            Some(Release::Injected)
        }),
    );
    let error = drive(&mut wide, Some(&mut scheduler)).expect_err("the shutdown ends the command");
    drop(scheduler);
    let message = error.to_string();
    assert!(
        message.contains("shut down")
            && message.contains(
                "nothing was appended but the record of the review passes a verification \
                     had charged"
            ),
        "the shutdown's error says what it appended: {message}"
    );
    let sequence = SequenceId(1);
    records_both_passes(&wide, sequence);
    let events = wide.env.durable_events();
    assert!(
        terminals_of(&events, sequence).is_empty(),
        "{:?}",
        kinds_of(&events)
    );
    replay_equals_live(&wide);
}

#[test]
fn another_pipelines_error_after_the_verifications_two_pass_result_arrived_records_both() {
    let tasks = three();
    let mut wide = Wide::started_with(
        "coordinator-o14-r7",
        &tasks,
        3,
        reviewing(2),
        holding(&tasks, &[]),
    );
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let panicking = std::sync::Arc::clone(&runner);
    let mut acted = false;
    let mut scheduler = Scheduler::scripted(
        &runner,
        Box::new(move |view: &Quiescent<'_>| {
            if acted {
                return None;
            }
            if let Some(release) = toward_arrival(view) {
                return release;
            }
            let gate = gamma_gate(view).expect("gamma's gate holds its snapshot");
            acted = true;
            panicking.panic_when_released(gate.clone());
            Some(Release::Invocation(gate))
        }),
    );
    let error = drive(&mut wide, Some(&mut scheduler)).expect_err("gamma's panic ends the command");
    drop(scheduler);
    assert!(
        error.to_string().contains("a pipeline panicked"),
        "the other pipeline's error ends the command: {error}"
    );
    let sequence = SequenceId(1);
    records_both_passes(&wide, sequence);
    let events = wide.env.durable_events();
    assert!(
        terminals_of(&events, sequence).is_empty(),
        "{:?}",
        kinds_of(&events)
    );
    replay_equals_live(&wide);
}

#[test]
fn a_verifications_own_error_after_two_paid_passes_at_width_three_is_recorded_before_the_command_ends()
 {
    let tasks = two();
    let mut wide = Wide::started_with(
        "coordinator-o14-r8",
        &tasks,
        3,
        reviewing(3),
        holding(&tasks, &[]),
    );
    let pipelines = pipelines_failing_the_snapshot_after(&wide, 3, None);
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let mut scheduler = alpha_then_beta(&runner);
    let mut hooks = wide.env.hooks();
    let error = wide
        .run
        .run_concurrently(
            &wide.env.seams(),
            &pipelines,
            &mut hooks,
            Some(&mut scheduler),
        )
        .expect_err("the third reviewer's snapshot add fails");
    drop(scheduler);
    assert!(
        error.to_string().contains("was made to fail"),
        "the verification's own error ends the command: {error}"
    );
    let events = wide.env.durable_events();
    assert_eq!(
        kinds_of(&events).last(),
        Some(&"merge_verification_charged"),
        "the record is the command's last append: {:?}",
        kinds_of(&events)
    );
    records_both_passes(&wide, SequenceId(1));
    replay_equals_live(&wide);
}

#[test]
fn a_halt_while_a_paid_verification_runs_records_the_two_passes_its_cancelled_pipeline_reports() {
    let mut wide = halting_on_gamma_reviewing("coordinator-o14-r9", 3, false);
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let mut scheduler = halting_at_the_third_pass(&runner);
    let progress = drive(&mut wide, Some(&mut scheduler)).expect("the halted run ends");
    drop(scheduler);
    assert_eq!(outcome_of(&progress), RunOutcome::Halted);
    let events = wide.env.durable_events();
    let kinds = kinds_of(&events);
    let halted = position(&kinds, "question_answered", 0);
    let record = position(&kinds, "merge_verification_charged", 0);
    let interrupted = position(&kinds, "merge_verification_interrupted", 0);
    let end = position(&kinds, "run_finished", 0);
    assert!(
        halted < record && record < interrupted && interrupted < end,
        "the record, then the closure's interrupted terminal, then the end: {kinds:?}"
    );
    let third = SequenceIdentities::new(SequenceId(1)).review_pass(2, 0);
    assert!(
        wide.env
            .runner
            .endings()
            .contains(&(third, crate::engine::topology::scaffold::Ending::Cancelled)),
        "the third pass, held when the halt came, was cancelled: {:?}",
        wide.env.runner.endings()
    );
    records_both_passes(&wide, SequenceId(1));
    replay_equals_live(&wide);
}

#[test]
fn a_decline_while_a_paid_verification_runs_records_its_two_passes_against_the_released_sequence() {
    let tasks = three();
    let armed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut wide = Wide::started_with(
        "coordinator-o14-r10",
        &tasks,
        3,
        reviewing(3),
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
        .expect("the decline fails beta's lineage and the run goes on to its end");
    drop(scheduler);
    assert_eq!(outcome_of(&progress), RunOutcome::Complete);
    let events = wide.env.durable_events();
    let kinds = kinds_of(&events);
    let decline = position(&kinds, "question_answered", 0);
    let record = position(&kinds, "merge_verification_charged", 0);
    assert!(
        decline < record,
        "the record follows the decline that released the verification: {kinds:?}"
    );
    recorded_before_its_terminal(&events, decline, SequenceId(1), LINEAGE_CANCELLED);
    records_both_passes(&wide, SequenceId(1));
    replay_equals_live(&wide);
}

#[test]
fn a_decline_after_the_verifications_two_pass_result_arrived_records_both_against_the_released_sequence()
 {
    let tasks = three();
    let armed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut wide = Wide::started_with(
        "coordinator-o14-r11",
        &tasks,
        3,
        reviewing(2),
        holding(&tasks, &[]),
    );
    wide.env.answers = std::sync::Arc::new(DecliningOnceArmed(std::sync::Arc::clone(&armed)));
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let arming = std::sync::Arc::clone(&armed);
    let mut planted: Option<Vec<TopologyEventBody>> = None;
    let mut scheduler = Scheduler::scripted(
        &runner,
        Box::new(move |view: &Quiescent<'_>| {
            if let Some(events) = planted.as_mut() {
                if !events.is_empty() {
                    return Some(Release::Append(Box::new(events.remove(0))));
                }
                if !arming.swap(true, std::sync::atomic::Ordering::SeqCst) {
                    return gamma_gate(view).map(Release::Invocation);
                }
                return None;
            }
            if let Some(release) = toward_arrival(view) {
                return release;
            }
            assert!(
                gamma_gate(view).is_some(),
                "gamma's gate holds its snapshot"
            );
            let open = view
                .run
                .fold()
                .transaction()
                .expect("beta's verification is open")
                .candidate
                .clone();
            planted = Some(sibling_parked_on(view.run, TaskKey(1), open));
            Some(Release::Append(Box::new(
                planted.as_mut().expect("planted").remove(0),
            )))
        }),
    );
    let progress = drive(&mut wide, Some(&mut scheduler))
        .expect("the decline fails beta's lineage and the run goes on to its end");
    drop(scheduler);
    assert_eq!(outcome_of(&progress), RunOutcome::Complete);
    let events = wide.env.durable_events();
    let kinds = kinds_of(&events);
    let decline = position(&kinds, "question_answered", 0);
    let record = position(&kinds, "merge_verification_charged", 0);
    assert!(decline < record, "{kinds:?}");
    assert_eq!(
        wide.run.discarded(),
        0,
        "the result was accepted before the decline: {:?}",
        wide.run.warnings()
    );
    recorded_before_its_terminal(&events, decline, SequenceId(1), LINEAGE_CANCELLED);
    records_both_passes(&wide, SequenceId(1));
    replay_equals_live(&wide);
}

#[test]
fn the_charged_records_append_error_replaces_the_verifications_error_as_the_commands_outcome() {
    use crate::topology::effects::{InjectionMode, SubEffectPoint};
    for (shape, point, present) in [
        ("partial-write", SubEffectPoint::Written, false),
        ("sync", SubEffectPoint::Synced, true),
    ] {
        let tasks = two();
        let mut wide = Wide::durable(
            &format!("coordinator-o14-r20-{shape}"),
            &tasks,
            3,
            reviewing(3),
            holding(&tasks, &[]),
        );
        let pipelines = pipelines_failing_the_snapshot_after(
            &wide,
            3,
            Some((point, InjectionMode::ErrorReturn)),
        );
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let mut scheduler = alpha_then_beta(&runner);
        let mut hooks = wide.env.hooks();
        let error = wide
            .run
            .run_concurrently(
                &wide.env.seams(),
                &pipelines,
                &mut hooks,
                Some(&mut scheduler),
            )
            .expect_err("the record's own append fails");
        drop(scheduler);
        let message = error.to_string();
        assert!(
            message.contains("merge_verification_charged")
                && message.contains("was entered and returned"),
            "{shape}: the command's error is the record's append error: {message}"
        );
        assert!(
            wide.run.warnings().iter().any(|warning| warning
                .contains("ends the command in place of what had ended the verification")
                && warning.contains("was made to fail")),
            "{shape}: a warning keeps the verification's own error: {:?}",
            wide.run.warnings()
        );
        assert!(wide.run.fold().is_poisoned(), "{shape}");
        let durable = kinds_of_log(&wide);
        assert_eq!(
            durable.last(),
            Some(if present {
                &"merge_verification_charged"
            } else {
                &"merge_verification_started"
            }),
            "{shape}: nothing follows the failed line: {durable:?}"
        );
        let (_, resumed) = wide
            .resume("inc-2", answering_two(), unlimited())
            .unwrap_or_else(|error| panic!("{shape}: the next process resumes: {error}"));
        let events = resumed.env.durable_events();
        assert_eq!(
            terminals_of(&events, SequenceId(1)),
            vec!["merge_verification_interrupted"],
            "{shape}: {:?}",
            kinds_of(&events)
        );
        if present {
            records_both_passes(&resumed, SequenceId(1));
        } else {
            assert!(
                charged_records(&events).is_empty(),
                "{shape}: the record's line did not survive, so its passes are unknown spend"
            );
        }
    }
}

#[test]
fn the_charged_records_append_error_in_finish_replaces_the_halt_and_runs_no_closure() {
    use crate::topology::effects::{InjectionMode, SubEffectPoint};
    for (shape, point, present) in [
        ("partial-write", SubEffectPoint::Written, false),
        ("sync", SubEffectPoint::Synced, true),
    ] {
        let mut wide = halting_on_gamma_reviewing(&format!("coordinator-o14-r21-{shape}"), 3, true);
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let mut scheduler = halting_at_the_third_pass(&runner);
        let mut hooks = HaltArming::over(&wide, point, InjectionMode::ErrorReturn);
        let pipelines = wide.env.pipelines();
        let error = wide
            .run
            .run_concurrently(
                &wide.env.seams(),
                &pipelines,
                &mut hooks,
                Some(&mut scheduler),
            )
            .expect_err("the record's own append fails");
        drop(scheduler);
        let message = error.to_string();
        assert!(
            message.contains("merge_verification_charged")
                && message.contains("was entered and returned"),
            "{shape}: the command's error is the record's append error, not the halt: \
                 {message}"
        );
        assert!(
            wide.run.warnings().iter().any(|warning| warning
                .contains("ends the command in place of what had ended the verification")
                && warning.contains("a halting settlement")),
            "{shape}: a warning names the halting settlement: {:?}",
            wide.run.warnings()
        );
        let durable = kinds_of_log(&wide);
        let halted = position(&durable, "question_answered", 0);
        assert!(
            !durable[halted..].contains(&"merge_verification_interrupted")
                && !durable.contains(&"run_finished"),
            "{shape}: no closure ran: {durable:?}"
        );
        assert_eq!(
            durable.last(),
            Some(if present {
                &"merge_verification_charged"
            } else {
                &"question_answered"
            }),
            "{shape}: {durable:?}"
        );
        assert!(
            wide.run.fold().halted_at().is_some(),
            "{shape}: the halt is recorded in the fold"
        );
        let (_, resumed) = finish_resumed_halted(wide, shape);
        if present {
            records_both_passes(&resumed, SequenceId(1));
        } else {
            assert!(charged_records(&resumed.env.durable_events()).is_empty());
        }
    }
}

#[test]
fn a_verification_that_fails_before_any_charge_records_nothing() {
    let mut wide = stale_at_width_one("coordinator-o14-c1", 3, unlimited());
    let mut hooks = failing_the_snapshot_after(&wide, 1);
    let seams = wide.env.seams();
    wide.run
        .step(&seams, &mut hooks)
        .expect_err("the first reviewer's snapshot add fails");
    let events = wide.env.durable_events();
    assert_eq!(
        kinds_of(&events).last(),
        Some(&"merge_verification_started"),
        "nothing is appended for a verification that charged nothing: {:?}",
        kinds_of(&events)
    );
    assert!(charged_records(&events).is_empty());
    let replayed = crate::engine::topology::select::Spend::replay(&events);
    assert!((replayed.run_usd() - wide.run.spend().run_usd()).abs() < 1e-9);
}

#[test]
fn a_publication_failure_after_merge_prepared_records_nothing() {
    let tasks = two();
    let mut wide = Wide::started_with(
        "coordinator-o14-c2",
        &tasks,
        3,
        reviewing(2),
        holding(&tasks, &[]),
    );
    let mut hooks = CasFailing::over(&wide);
    hooks.fail_after(
        crate::topology::effects::RefSite::CompareAndSwapIntegration,
        1,
    );
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let mut scheduler = alpha_then_beta(&runner);
    let pipelines = wide.env.pipelines();
    let error = wide
        .run
        .run_concurrently(
            &wide.env.seams(),
            &pipelines,
            &mut hooks,
            Some(&mut scheduler),
        )
        .expect_err("beta's verified publication fails its CAS");
    drop(scheduler);
    assert!(
        error.to_string().contains("CompareAndSwapIntegration"),
        "{error}"
    );
    let events = wide.env.durable_events();
    assert_eq!(
        kinds_of(&events).last(),
        Some(&"merge_prepared"),
        "nothing follows the terminal that carried the passes: {:?}",
        kinds_of(&events)
    );
    assert!(charged_records(&events).is_empty());
    assert!(
        !wide
            .run
            .warnings()
            .iter()
            .any(|warning| warning.contains("merge_verification_charged")),
        "the fold's refusal of a record a terminal made redundant warns of nothing: {:?}",
        wide.run.warnings()
    );
}

fn killed_at_the_record(shape: &str) {
    let handoff =
        crate::engine::topology::scaffold::kill_dir(&format!("coordinator-o14-kill-{shape}"));
    let temporary = crate::engine::topology::scaffold::kill_dir(RECORD_KILL_TEMPORARY);
    let mut environment = vec![
        ("UPSTROKE_TEST_KILL_DIR", handoff.path().as_os_str()),
        ("UPSTROKE_TEST_KILL_SHAPE", std::ffi::OsStr::new(shape)),
    ];
    environment.extend(crate::engine::topology::scaffold::child_temporary_of(
        temporary.path(),
    ));
    let status = crate::workspace_manager::fixture::run_kill_child_within(
        "engine::topology::coordinator::tests::charged_record_kill_child",
        &environment,
        crate::engine::topology::scaffold::KILL_CHILD_BOUND,
    )
    .unwrap_or_else(|| panic!("{shape}: the kill child did not end in time"));
    assert!(
        crate::workspace_manager::fixture::died_by_abort(&status),
        "{shape}: the child died at the record's append: {status:?}"
    );
    let root = std::fs::read_to_string(handoff.path().join(RECORD_HANDOFF))
        .unwrap_or_else(|error| panic!("{shape}: the child left no handoff: {error}"));
    let env = crate::engine::topology::scaffold::WideEnv::adopted(
        std::path::PathBuf::from(root),
        &two(),
        3,
        reviewing(3),
    );
    let started = env.durable_events();
    assert_eq!(
        kinds_of(&started).last(),
        Some(&"merge_verification_started"),
        "{shape}: the child died before its record's line was durable: {:?}",
        kinds_of(&started)
    );
    let bytes = std::fs::read(&env.log).expect("the child's log survives it");
    let tail = bytes
        .rsplit(|byte| *byte == b'\n')
        .next()
        .map(String::from_utf8_lossy)
        .unwrap_or_default();
    if shape == "torn" {
        assert!(
            tail.contains("merge_verification_charged"),
            "the child died part-way through writing its record: {tail:?}"
        );
    } else {
        assert!(
            tail.is_empty()
                && !String::from_utf8_lossy(&bytes).contains("merge_verification_charged"),
            "the child died before writing any of its record: {tail:?}"
        );
    }
    let (_, resumed) = env
        .resume("inc-3", answering_two(), unlimited())
        .unwrap_or_else(|error| panic!("{shape}: the next process resumes: {error}"));
    left_to_recovery_with_unknown_spend(&resumed, shape);
}

fn left_to_recovery_with_unknown_spend(resumed: &Wide, shape: &str) {
    let events = resumed.env.durable_events();
    let sequence = SequenceId(2);
    assert_eq!(
        terminals_of(&events, sequence),
        vec!["merge_verification_interrupted"],
        "{shape}: the census and step (f) settle the verification interrupted: {:?}",
        kinds_of(&events)
    );
    assert!(
        charged_records(&events).is_empty(),
        "{shape}: no record survived: {:?}",
        kinds_of(&events)
    );
    let opened = started_at(&events, sequence);
    let before = crate::engine::topology::select::Spend::replay(&events[..opened]);
    let after = crate::engine::topology::select::Spend::replay(&events);
    assert!(
        (after.run_usd() - before.run_usd()).abs() < 1e-9,
        "{shape}: the replayed total lacks both passes, which are unknown spend: {} -> {}",
        before.run_usd(),
        after.run_usd()
    );
}

#[test]
fn a_kill_before_the_charged_record_leaves_the_verification_to_recovery_with_unknown_spend() {
    killed_at_the_record("before");
}

#[test]
fn a_torn_charged_record_is_truncated_and_recovered_as_before_it() {
    killed_at_the_record("torn");
}

const GUEST_TEMPORARY: &str = r"C:\Users\Administrator\AppData\Local\Temp";

const GUEST_TEMPORARY_SLACK: usize = 12;

#[test]
fn the_record_kill_childs_deepest_git_dir_fits_the_windows_budget_under_the_guest_lanes_temporary_directory()
 {
    assert_eq!(GUEST_TEMPORARY.len(), 41, "{GUEST_TEMPORARY}");
    let temporary = crate::engine::topology::scaffold::kill_dir(RECORD_KILL_TEMPORARY);
    let fixture = crate::workspace_manager::fixture::Fixture::created(RECORD_KILL_TAG);
    let checkout = fixture
        .manager
        .slot_path(&crate::workspace_manager::Slot::Snapshot {
            name: crate::workspace_manager::SnapshotName::review(0, 0, 1, 0),
        });
    let nested = temporary
        .path()
        .file_name()
        .expect("the child's temporary directory is named");
    let root = fixture
        .root
        .file_name()
        .expect("the child's fixture root is named");
    let private = fixture
        .private
        .strip_prefix(&fixture.root)
        .expect("the private root is inside the fixture's root");
    let slot = checkout
        .strip_prefix(fixture.manager.private_root())
        .expect("the reviewer snapshot is inside the private root");
    let longer = format!("{GUEST_TEMPORARY}{}", "x".repeat(GUEST_TEMPORARY_SLACK));
    for temporary_directory in [GUEST_TEMPORARY, longer.as_str()] {
        let child = std::path::Path::new(temporary_directory)
            .join(nested)
            .join(root)
            .join(private)
            .join(slot);
        crate::workspace_manager::fixture::refuse_git_dir_over_the_windows_budget(&child)
            .unwrap_or_else(|refusal| panic!("under `{temporary_directory}`: {refusal}"));
    }
}

#[test]
fn an_append_error_on_the_charged_record_follows_the_surviving_prefix() {
    use crate::topology::effects::{InjectionMode, SubEffectPoint};
    for (shape, point, present) in [
        ("partial-write", SubEffectPoint::Written, false),
        ("sync", SubEffectPoint::Synced, true),
    ] {
        let mut wide =
            stale_at_width_one(&format!("coordinator-o14-append-{shape}"), 3, unlimited());
        let mut hooks = ArmingOnTheFault::over(&wide, 3, Some((point, InjectionMode::ErrorReturn)));
        let seams = wide.env.seams();
        let error = wide
            .run
            .step(&seams, &mut hooks)
            .expect_err("the record's own append fails");
        drop(hooks);
        let message = error.to_string();
        assert!(
            message.contains("merge_verification_charged")
                && message.contains("was entered and returned"),
            "{shape}: the step's error is the append error: {message}"
        );
        assert!(
            wide.run.warnings().iter().any(|warning| warning
                .contains("ends the command in place of what had ended the verification")
                && warning.contains("was made to fail")),
            "{shape}: a warning carries the verification's own error: {:?}",
            wide.run.warnings()
        );
        assert!(wide.run.fold().is_poisoned(), "{shape}");
        let (_, resumed) = wide
            .resume("inc-3", answering_two(), unlimited())
            .unwrap_or_else(|error| panic!("{shape}: the next process resumes: {error}"));
        let events = resumed.env.durable_events();
        assert_eq!(
            terminals_of(&events, SequenceId(2)),
            vec!["merge_verification_interrupted"],
            "{shape}: {:?}",
            kinds_of(&events)
        );
        if present {
            records_both_passes(&resumed, SequenceId(2));
        } else {
            left_to_recovery_with_unknown_spend(&resumed, shape);
        }
    }
}

#[test]
fn a_resume_after_a_charged_record_settles_the_verification_interrupted_and_counts_its_passes_once()
{
    let mut wide = stale_at_width_one("coordinator-o14-resume", 3, unlimited());
    let mut hooks = failing_the_snapshot_after(&wide, 3);
    let seams = wide.env.seams();
    wide.run
        .step(&seams, &mut hooks)
        .expect_err("the third reviewer's snapshot add fails");
    drop(hooks);
    let sequence = SequenceId(2);
    let (_, reviews) = charged_records(&wide.env.durable_events())
        .into_iter()
        .next()
        .expect("the record is durable");
    let run_id = wide.run.fold().started().expect("started").run_id.clone();
    let (_, mut resumed) = wide
        .resume("inc-3", answering_two(), unlimited())
        .expect("the next process resumes");
    let events = resumed.env.durable_events();
    let kinds = kinds_of(&events);
    assert_eq!(
        terminals_of(&events, sequence),
        vec!["merge_verification_interrupted"],
        "{kinds:?}"
    );
    let manager = resumed.env.fixture.manager.clone();
    let intents = manager.intents().expect("the intents are listable");
    assert!(
        !intents.contains(&crate::engine::topology::integrate::staging_slot(sequence))
            && !snapshot_names(&intents)
                .iter()
                .any(|name| name.starts_with("s2-")),
        "the staging and the snapshots of sequence 2 are reclaimed: {intents:?}"
    );
    assert_eq!(
        manager
            .direct_ref_target(
                crate::engine::topology::integrate::prepared_pin_ref(&run_id, sequence).as_str()
            )
            .expect("the pin is readable"),
        None,
        "the pin is deleted"
    );
    let second = resumed
        .run
        .fold()
        .plan_transition(&charged_event(sequence, &reviews))
        .expect_err("a second record for the sequence is refused after the resume");
    assert!(
        second
            .to_string()
            .contains("already records the review passes"),
        "the resumed fold still knows the sequence's record: {second}"
    );
    let carrying = TopologyEvent {
        ts: "2026-10-08T00:00:00.000Z".to_owned(),
        body: TopologyEventBody::MergeVerificationUnavailable {
            data: crate::topology::events::MergeVerificationUnavailable {
                sequence,
                cause: crate::topology::events::UnavailableCause::Infrastructure {
                    kind: crate::topology::events::InfrastructureKind::ReviewerTimeout,
                },
                outcome: crate::topology::events::UnavailableOutcome::Deferred { defers: 1 },
                reviews,
            },
        },
    };
    assert!(
        resumed.run.fold().plan_transition(&carrying).is_err(),
        "a carrying terminal for the sequence is refused after the resume"
    );
    let mut hooks = resumed.env.hooks();
    let seams = resumed.env.seams();
    let progress = resumed
        .run
        .step(&seams, &mut hooks)
        .expect("the candidate verifies again");
    assert!(
        matches!(progress, Progress::Integrated { sequence: again, .. } if again == SequenceId(3)),
        "the candidate re-verifies under the next sequence: {progress:?}"
    );
    drop(hooks);
    records_both_passes(&resumed, sequence);
}

#[test]
fn refusals_across_resumes_each_record_their_passes_and_the_ceiling_stops_the_run() {
    let mut wide = stale_at_width_one("coordinator-o14-refusals", 3, unlimited());
    let start = wide.run.spend().run_usd();
    let limit = start + 4.0 * PASS_COST;
    let ceiling = crate::engine::topology::select::Ceiling {
        run_usd: Some(limit),
        task_usd: None,
    };
    for incarnation in ["inc-3", "inc-4"] {
        let mut hooks = failing_the_snapshot_after(&wide, 3);
        let seams = wide.env.seams();
        wide.run
            .step(&seams, &mut hooks)
            .expect_err("the persistent fault refuses the verification again");
        drop(hooks);
        wide = wide
            .resume(incarnation, answering_two(), ceiling)
            .expect("the next process resumes")
            .1;
    }
    let events = wide.env.durable_events();
    let records: Vec<(u32, usize)> = charged_records(&events)
        .iter()
        .map(|(sequence, reviews)| (sequence.0, reviews.len()))
        .collect();
    assert_eq!(
        records,
        vec![(2, 2), (3, 2)],
        "one record of two passes per refused sequence: {:?}",
        kinds_of(&events)
    );
    let replayed = crate::engine::topology::select::Spend::replay(&events);
    let without = crate::engine::topology::select::Spend::replay(&unrecorded(&events));
    assert!(
        (replayed.run_usd() - without.run_usd() - 4.0 * PASS_COST).abs() < 1e-9,
        "each sequence's passes are counted once: {} -> {}",
        without.run_usd(),
        replayed.run_usd()
    );
    let mut hooks = wide.env.hooks();
    let seams = wide.env.seams();
    let progress = wide.run.step(&seams, &mut hooks).expect("the next step");
    assert!(
        matches!(progress, Progress::BudgetExceeded),
        "the ceiling stops the run: {progress:?}"
    );
}

fn refused_at_the_third_review(
    wide: &mut Wide,
    planter: &Arc<dyn ReviewInputPolicy + Send + Sync>,
) -> UpstrokeError {
    let mut pipelines = wide.env.pipelines();
    let (harness, planter) = (Arc::clone(&wide.env.harness), Arc::clone(planter));
    pipelines.hooks = Arc::new(move || {
        Box::new(PlantingHooks::new(&harness, &planter, 3)) as Box<dyn TopologyHooks + Send>
    });
    let runner = Arc::clone(&wide.env.runner);
    let mut scheduler = alpha_then_beta(&runner);
    let mut hooks = wide.env.hooks();
    wide.run
        .run_concurrently(
            &wide.env.seams(),
            &pipelines,
            &mut hooks,
            Some(&mut scheduler),
        )
        .expect_err("the third reviewer's snapshot refuses")
}

fn recorded_and_still_open(wide: &Wide, sequence: SequenceId) {
    let events = wide.env.durable_events();
    assert_eq!(
        kinds_of(&events).last(),
        Some(&"merge_verification_charged"),
        "the record is the command's last append: {:?}",
        kinds_of(&events)
    );
    assert!(
        terminals_of(&events, sequence).is_empty(),
        "the record is no terminal: {:?}",
        kinds_of(&events)
    );
    assert!(
        verifying(wide.run.fold(), sequence),
        "the fold still holds the refused verification open: {:?}",
        wide.run.fold().transaction()
    );
    records_both_passes(wide, sequence);
    replay_equals_live(wide);
}

fn resumed_to_completion(wide: Wide, tasks: &[WideTask], key: TaskKey, sequence: SequenceId) {
    let live = wide.run.spend().clone();
    let (_, mut resumed) = wide
        .resume("inc-2", holding(tasks, &[]), unlimited())
        .expect("the next process resumes once the fault is repaired");
    resumed_spend_is_the_live_one(&resumed, key, &live, sequence);
    let runner = Arc::clone(&resumed.env.runner);
    let mut scheduler = Scheduler::first(&runner);
    let progress = drive(&mut resumed, Some(&mut scheduler)).expect("the resumed run completes");
    drop(scheduler);
    assert_eq!(outcome_of(&progress), RunOutcome::Complete);
    records_both_passes(&resumed, sequence);
}

#[test]
fn a_registry_refusal_after_two_paid_passes_is_recorded_with_both_at_width_three() {
    let tasks = two();
    let mut wide = Wide::durable(
        "coordinator-o14-registry-refusal",
        &tasks,
        3,
        reviewing(3),
        holding(&tasks, &[]),
    );
    let common = wide.env.fixture.manager.common_git_dir().to_path_buf();
    let _deadline =
        crate::workspace_manager::RegistryDeadline::hold(&common, WITNESS_REGISTRY_DEADLINE);
    let (tear, admin) = tearing(&wide, "foreign-at-the-third-review", false);
    let tear: Arc<dyn ReviewInputPolicy + Send + Sync> = tear;
    let error = refused_at_the_third_review(&mut wide, &tear);
    refused_by_the_tear(&error);
    let sequence = SequenceId(1);
    recorded_and_still_open(&wide, sequence);
    let key = started_key(&wide.env.durable_events(), sequence);
    untear(&admin);
    resumed_to_completion(wide, &tasks, key, sequence);
}

#[cfg(unix)]
#[test]
fn a_registry_refusal_after_the_takeover_at_the_third_review_is_recorded_with_both_passes() {
    let tasks = two();
    let mut wide = Wide::durable(
        "coordinator-o14-takeover-refusal",
        &tasks,
        3,
        reviewing(3),
        holding(&tasks, &[]),
    );
    let common = wide.env.fixture.manager.common_git_dir().to_path_buf();
    let _deadline =
        crate::workspace_manager::RegistryDeadline::hold(&common, WITNESS_REGISTRY_DEADLINE);
    let base = wide.env.fixture.base.clone();
    let info = common.join("info");
    let filter: Arc<dyn ReviewInputPolicy + Send + Sync> = Arc::new(RequiresAFailingFilter {
        base: base.clone(),
        info: info.clone(),
        remaining: std::sync::atomic::AtomicUsize::new(1),
    });
    let error = refused_at_the_third_review(&mut wide, &filter);
    assert!(
        matches!(&error, UpstrokeError::RegistryRefused { message } if message.contains("b329fails") || message.contains("is gone")),
        "the third reviewer's checkout fails after the takeover and refuses at once: {error:?}"
    );
    let sequence = SequenceId(1);
    recorded_and_still_open(&wide, sequence);
    let key = started_key(&wide.env.durable_events(), sequence);
    crate::workspace_manager::fixture::remove_file(&info.join("attributes"));
    crate::workspace_manager::fixture::git(
        &base,
        &["config", "--remove-section", "filter.b329fails"],
    );
    resumed_to_completion(wide, &tasks, key, sequence);
}

fn retained_attempt_finished_of_delta(body: &TopologyEventBody) -> bool {
    matches!(body, TopologyEventBody::AttemptFinished { data } if data.key == TaskKey(3))
}

fn stopped_and_recorded(
    wide: &Wide,
    error: &UpstrokeError,
    at_shutdown: usize,
    sequence: SequenceId,
    retried: TaskKey,
) {
    let events = wide.env.durable_events();
    let kinds = kinds_of(&events);
    let key = started_key(&events, sequence);
    let replayed = crate::engine::topology::select::Spend::replay(&events);
    let live = wide.run.spend();
    let (run_live, run_replayed) = (live.run_usd(), replayed.run_usd());
    let (task_live, task_replayed) = (live.task_usd(key), replayed.task_usd(key));
    assert!(
        (run_live - run_replayed).abs() < 1e-9 && (task_live - task_replayed).abs() < 1e-9,
        "the passes charged before the stop are recorded, so replay counts what was charged \
             live: the run ${run_live:.2} live and ${run_replayed:.2} replayed, the verifying task \
             {} ${task_live:.2} live and ${task_replayed:.2} replayed, {} record(s), and after the \
             shutdown {:?}",
        key.index(),
        count(&events, "merge_verification_charged"),
        kinds_of(wide.run.events())
            .get(at_shutdown..)
            .unwrap_or_default()
    );
    records_both_passes(wide, sequence);
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
        "the retry's wait answered the shutdown and stopped the run: {warnings}"
    );
    assert!(
        !warnings.contains("the append recording the review passes a verification charged failed"),
        "the record's append did not fail: {warnings}"
    );
    assert_eq!(
        kinds_of(wide.run.events()).get(at_shutdown..),
        Some(&["merge_verification_charged"][..]),
        "nothing but the record was appended after the wait answered the shutdown"
    );
    assert_eq!(
        of_key(&events, "attempt_started", retried.0),
        1,
        "the stopped retry started no attempt: {kinds:?}"
    );
    assert!(
        terminals_of(&events, sequence).is_empty(),
        "the record is no terminal: {kinds:?}"
    );
    assert!(
        verifying(wide.run.fold(), sequence),
        "the fold still holds the verification open: {:?}",
        wide.run.fold().transaction()
    );
    replay_equals_live(wide);
    published_as_the_log_authorizes(wide);
    assert!(wide.run.invocations_balance(), "{warnings}");
    assert!(
        wide.run.stopped().is_none(),
        "the run reopens when its coordinator is gone"
    );
}

fn sequence_endings(
    wide: &Wide,
    sequence: SequenceId,
) -> Vec<(InvocationId, crate::engine::topology::scaffold::Ending)> {
    wide.env
        .runner
        .endings()
        .into_iter()
        .filter(|(invocation, _)| {
            matches!(invocation, InvocationId::Sequence { sequence: of, .. } if *of == sequence)
        })
        .collect()
}

#[test]
fn a_shutdown_answered_inside_a_retrys_wait_records_the_passes_its_cancelled_verification_charged()
{
    let tasks = [
        WideTask::independent("beta"),
        WideTask::independent("alpha"),
        WideTask::independent("gamma"),
    ];
    let mut wide = Wide::durable(
        "coordinator-o14b2-cancelled",
        &tasks,
        3,
        reviewing(3),
        holding(&tasks, &[(0, 1)]),
    );
    let (error, at_shutdown) = stopped_inside_a_retrys_wait(
        &mut wide,
        TearAt::Fold(retained_attempt_finished_of_beta),
        Box::new(|view: &Quiescent<'_>| {
            if count(view.run.events(), "task_merged") == 0 {
                return released(view, |invocation| attempt_key(invocation) == Some(1));
            }
            if view
                .invoking
                .iter()
                .any(|invocation| review_pass(invocation, 2))
            {
                return released(view, |invocation| attempt_key(invocation) == Some(0))
                    .or(Some(Release::Nothing));
            }
            released(view, |invocation| attempt_key(invocation) == Some(2)).or_else(|| {
                released(view, |invocation| {
                    sequence_invocation(invocation) && !review_pass(invocation, 2)
                })
            })
        }),
    );
    let sequence = SequenceId(1);
    let endings = sequence_endings(&wide, sequence);
    assert!(
        endings
            .iter()
            .any(|(invocation, ending)| review_pass(invocation, 2)
                && *ending == crate::engine::topology::scaffold::Ending::Cancelled),
        "the third reviewer was still running, and the shutdown cancelled it: {endings:?}"
    );
    stopped_and_recorded(&wide, &error, at_shutdown, sequence, TaskKey(0));
    let key = started_key(&wide.env.durable_events(), sequence);
    resumed_to_completion(wide, &tasks, key, sequence);
}

#[test]
fn a_shutdown_answered_inside_a_retrys_wait_after_the_result_arrived_records_both() {
    let tasks = [
        WideTask::independent("alpha"),
        WideTask::independent("beta"),
        WideTask::independent("gamma"),
        WideTask::independent("delta"),
    ];
    let mut wide = Wide::durable(
        "coordinator-o14b2-arrived",
        &tasks,
        4,
        reviewing(2),
        holding(&tasks, &[(3, 1)]),
    );
    let (error, at_shutdown) = stopped_inside_a_retrys_wait(
        &mut wide,
        TearAt::Fold(retained_attempt_finished_of_delta),
        Box::new(|view: &Quiescent<'_>| {
            if count(view.run.events(), "task_merged") == 0 {
                return released(view, |invocation| attempt_key(invocation) == Some(0));
            }
            if view.run.fold().transaction().is_none() {
                return released(view, |invocation| attempt_key(invocation) == Some(1));
            }
            if verification_live(view) {
                return released(view, |invocation| {
                    matches!(worker(invocation), Some(TaskKey(2) | TaskKey(3)))
                })
                .or_else(|| released(view, sequence_invocation));
            }
            released(view, |invocation| {
                attempt_key(invocation) == Some(3)
                    && matches!(role_of(invocation), Some(AttemptRole::Gate(_)))
            })
            .or_else(|| released(view, |_| true))
        }),
    );
    let sequence = SequenceId(1);
    let endings = sequence_endings(&wide, sequence);
    assert!(
        endings.len() == 3
            && endings.iter().all(|(_, ending)| {
                *ending == crate::engine::topology::scaffold::Ending::Completed
            }),
        "the verification's gate and both reviewers ended before the shutdown, so its \
             result had arrived: {endings:?}"
    );
    assert!(
        sequence_endings(&wide, sequence)
            .iter()
            .all(|(invocation, _)| !review_pass(invocation, 2)),
        "two reviewers verify"
    );
    stopped_and_recorded(&wide, &error, at_shutdown, sequence, TaskKey(3));
    let key = started_key(&wide.env.durable_events(), sequence);
    resumed_to_completion(wide, &tasks, key, sequence);
}
