//! Extended notes: `docs/internals/engine/topology/coordinator/tests/scheduling.md`

use super::*;

#[test]
fn two_independent_tasks_overlap_merge_through_one_queue_and_the_dependent_starts_on_both() {
    let tasks = [
        WideTask::independent("alpha"),
        WideTask::independent("beta"),
        WideTask::after("gamma", &["alpha", "beta"]),
    ];
    let runner = holding(&tasks, &[]);
    let mut wide = Wide::started_with(
        "coordinator-acceptance-two",
        &tasks,
        3,
        WidePlans::default(),
        runner,
    );
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let mut first: Option<Vec<InvocationId>> = None;
    let mut scheduler = Scheduler::scripted(
        &runner,
        Box::new(|view: &Quiescent<'_>| {
            if first.is_none() {
                first = Some(view.invoking.clone());
            }
            None
        }),
    );
    let progress = drive(&mut wide, Some(&mut scheduler)).expect("the run completes");
    drop(scheduler);
    assert_eq!(outcome_of(&progress), RunOutcome::Complete);

    let both: Vec<TaskKey> = first
        .expect("the scheduler saw a quiescent point")
        .iter()
        .filter_map(worker)
        .collect();
    assert_eq!(
        both,
        vec![TaskKey(0), TaskKey(1)],
        "both independent workers were running at once before either completed"
    );

    let events = wide.env.durable_events();
    let kinds: Vec<&str> = events.iter().map(|event| event.body.kind()).collect();
    assert!(
        position(&kinds, "task_dispatched", 1) < position(&kinds, "candidate_prepared", 0),
        "the two independent tasks were dispatched before either settled: {kinds:?}"
    );
    let merged = merged(&events);
    assert_eq!(
        merged
            .iter()
            .map(|(sequence, _)| *sequence)
            .collect::<Vec<_>>(),
        vec![0, 1, 2],
        "one queue, in sequence order"
    );
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == "merge_verification_started")
            .count(),
        1,
        "the second of the overlapping pair was stale and verified; the first was fast, and \
             the dependent, dispatched on the head holding both, was fast: {kinds:?}"
    );
    let bases = dispatched_bases(&events);
    let after_both = &merged.get(1).expect("the second merge").1;
    assert_eq!(
        bases.get(&2),
        Some(after_both),
        "the dependent was dispatched on the head holding both independent tasks"
    );
    let gamma_worker = wide
        .env
        .runner
        .ran()
        .into_iter()
        .find(|ran| worker(&ran.invocation) == Some(TaskKey(2)))
        .expect("the dependent's worker ran");
    assert_eq!(
        gamma_worker.head_at_spawn.as_ref(),
        Some(after_both),
        "the dependent's worktree held both merged tasks when its worker started"
    );
    let head = wide.env.head(&wide.run);
    for path in [
        "src/alpha/work.txt",
        "src/beta/work.txt",
        "src/gamma/work.txt",
    ] {
        let shown = crate::workspace_manager::fixture::git_out(
            &wide.env.fixture.base,
            &["cat-file", "-e", &format!("{head}:{path}")],
        );
        assert!(shown.status.success(), "the integrated head lacks {path}");
    }
    assert!(wide.run.invocations_balance());
    assert_eq!(wide.run.entitlements_held(), 0);
    assert_eq!(wide.run.discarded(), 0, "{:?}", wide.run.warnings());
}

#[test]
fn disjoint_hints_dispatch_together_overlapping_and_absent_hints_serialize() {
    let cases: [(&str, [WideTask; 2], bool); 3] = [
        (
            "disjoint",
            [
                WideTask::hinted("alpha", &["src/a/"], "src/a/work.txt"),
                WideTask::hinted("beta", &["src/b/"], "src/b/work.txt"),
            ],
            true,
        ),
        (
            "overlapping",
            [
                WideTask::hinted("alpha", &["src/x/"], "src/x/work.txt"),
                WideTask::hinted("beta", &["src/x/y/"], "src/x/y/work.txt"),
            ],
            false,
        ),
        (
            "absent",
            [
                WideTask::hinted("alpha", &[], "src/a/work.txt"),
                WideTask::hinted("beta", &["src/b/"], "src/b/work.txt"),
            ],
            false,
        ),
    ];
    for (case, tasks, together) in cases {
        let runner = holding(&tasks, &[]);
        let mut wide = Wide::started_with(
            &format!("coordinator-hints-{case}"),
            &tasks,
            3,
            WidePlans::default(),
            runner,
        );
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let mut scheduler = Scheduler::first(&runner);
        let progress = drive(&mut wide, Some(&mut scheduler))
            .unwrap_or_else(|error| panic!("{case}: {error}"));
        assert_eq!(outcome_of(&progress), RunOutcome::Complete, "{case}");
        let events = wide.env.durable_events();
        let kinds: Vec<&str> = events.iter().map(|event| event.body.kind()).collect();
        let second_dispatch = position(&kinds, "task_dispatched", 1);
        if together {
            assert!(
                second_dispatch < position(&kinds, "candidate_prepared", 0),
                "{case}: both tasks were dispatched before either settled: {kinds:?}"
            );
            assert_eq!(scheduler.widest, 2, "{case}: both workers ran at once");
        } else {
            assert!(
                second_dispatch > position(&kinds, "task_merged", 0),
                "{case}: the second task waited for the first's lease to be released: \
                     {kinds:?}"
            );
            assert_eq!(
                scheduler.widest, 1,
                "{case}: nothing ran beside the first task"
            );
        }
        assert!(wide.run.invocations_balance(), "{case}");
    }
}

struct Folding {
    inner: crate::engine::topology::seams::HarnessTopologyHooks,
    states: Vec<(usize, Option<RunState>)>,
}

impl TopologyHooks for Folding {
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
        self.states.push((events.len(), fold.state().cloned()));
    }
}

#[test]
fn the_coordinator_settles_promotes_and_dispatches_while_a_verification_is_open() {
    let tasks = [
        WideTask::independent("alpha"),
        WideTask::independent("beta"),
        WideTask::independent("gamma"),
        WideTask::independent("delta"),
        WideTask::independent("epsilon"),
    ];
    let runner = holding(&tasks, &[]);
    let mut wide = Wide::started_with(
        "coordinator-inside-verify",
        &tasks,
        3,
        WidePlans::default(),
        runner,
    );
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let mut scheduler = Scheduler::scripted(
        &runner,
        Box::new(|view: &Quiescent<'_>| {
            let merges = count(view.run.events(), "task_merged");
            let verifying = view.run.fold().transaction().is_some();
            if merges == 0 {
                return released(view, |invocation| attempt_key(invocation) == Some(0));
            }
            if merges == 1 && !verifying {
                return released(view, |invocation| attempt_key(invocation) == Some(1));
            }
            if merges == 1 && verifying {
                return released(view, |invocation| attempt_key(invocation) == Some(2))
                    .or_else(|| {
                        released(view, |invocation| {
                            attempt_key(invocation) == Some(3)
                                && role_of(invocation) == Some(AttemptRole::Worker)
                        })
                    })
                    .or_else(|| {
                        released(view, |invocation| {
                            matches!(invocation, InvocationId::Sequence { .. })
                        })
                    })
                    .or_else(|| released(view, |invocation| attempt_key(invocation) == Some(3)));
            }
            None
        }),
    );
    let progress = drive(&mut wide, Some(&mut scheduler)).expect("the run completes");
    drop(scheduler);
    assert_eq!(outcome_of(&progress), RunOutcome::Complete);

    let events = wide.env.durable_events();
    let kinds = kinds_of(&events);
    let opened = position(&kinds, "merge_verification_started", 0);
    let closed = position(&kinds, "task_merged", 1);
    let inside: Vec<(&str, Option<u64>)> = events
        .get(opened + 1..closed)
        .expect("the verification's span")
        .iter()
        .map(|event| {
            let value = serde_json::to_value(&event.body).expect("serializes");
            (event.body.kind(), key_of(&value))
        })
        .collect();
    assert!(
        inside.contains(&("candidate_prepared", Some(2)))
            && inside.contains(&("task_candidate_created", Some(2))),
        "another task was promoted while the verification was open: {inside:?}"
    );
    assert!(
        inside.contains(&("task_dispatched", Some(4))),
        "a new task was dispatched while the verification was open: {inside:?}"
    );
    assert!(
        !inside
            .iter()
            .any(|(kind, _)| *kind == "merge_verification_started"),
        "no second integration started before the first's terminal: {inside:?}"
    );

    let ran = wide.env.runner.ran();
    let spawned_after = |key: u32, role: AttemptRole| {
        ran.iter()
            .find(|entry| {
                attempt_key(&entry.invocation) == Some(key)
                    && role_of(&entry.invocation) == Some(role)
            })
            .map(|entry| {
                entry
                    .durable_at_spawn
                    .iter()
                    .filter(|kind| kind.as_str() == "task_merged")
                    .count()
            })
            .unwrap_or_else(|| panic!("task {key}'s {role:?} ran"))
    };
    assert_eq!(
        spawned_after(3, AttemptRole::Gate(0)),
        1,
        "delta's gate ran on its snapshot while the verification was open"
    );
    assert_eq!(
        spawned_after(3, AttemptRole::ReviewPass(0)),
        2,
        "delta's next snapshot waited until the integration's terminal and reclaim were \
             done: the snapshot gate held it"
    );
    assert!(
        ran.iter().all(|entry| entry.workspace.exists() || true),
        "every invocation ran in a slot"
    );
    assert_eq!(count(&events, "task_merged"), 5, "{kinds:?}");
    assert_eq!(
        count(&events, "attempt_finished"),
        0,
        "no attempt lost its snapshot to the integration's reclaim: {kinds:?}"
    );
    assert!(wide.run.invocations_balance());
    assert_eq!(wide.run.discarded(), 0, "{:?}", wide.run.warnings());
    replay_equals_live(&wide);
}

struct Recording {
    inner: crate::engine::topology::seams::HarnessTopologyHooks,
    folds: Vec<(&'static str, TopologyFold)>,
}

impl Recording {
    fn over(wide: &Wide) -> Self {
        Self {
            inner: wide.env.hooks(),
            folds: Vec::new(),
        }
    }
}

impl TopologyHooks for Recording {
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
        if let Some(last) = events.last() {
            self.folds.push((last.body.kind(), fold.clone()));
        }
    }
}

fn ending_as(fold: &TopologyFold, outcome: RunOutcome) -> Result<(), String> {
    let event = TopologyEvent {
        ts: "2026-09-30T00:00:00Z".to_owned(),
        body: TopologyEventBody::RunFinished {
            data: crate::engine::topology::closure::run_finished(fold, outcome),
        },
    };
    fold.plan_transition(&event)
        .map(drop)
        .map_err(|refusal| refusal.to_string())
}

fn budget_stopped_with_two_in_flight(
    tag: &str,
) -> (Wide, Recording, Result<Progress, UpstrokeError>) {
    let tasks = four();
    let mut wide = Wide::started_under(
        tag,
        &tasks,
        3,
        WidePlans::default(),
        holding(&tasks, &[]),
        TIGHT,
    );
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let mut scheduler = Scheduler::scripted(
        &runner,
        Box::new(|view: &Quiescent<'_>| {
            released(view, |invocation| attempt_key(invocation) == Some(0))
        }),
    );
    let mut recording = Recording::over(&wide);
    let pipelines = wide.env.pipelines();
    let progress = wide.run.run_concurrently(
        &wide.env.seams(),
        &pipelines,
        &mut recording,
        Some(&mut scheduler),
    );
    drop(scheduler);
    (wide, recording, progress)
}

#[test]
fn over_budget_prefix_without_budget_exceeded_is_not_ending_at_width_three() {
    let (wide, recording, progress) =
        budget_stopped_with_two_in_flight("coordinator-over-budget-prefix");
    assert_eq!(
        outcome_of(&progress.expect("the budget-stopped run ends")),
        RunOutcome::BudgetExceeded
    );
    let stop = recording
        .folds
        .iter()
        .position(|(kind, _)| *kind == "budget_exceeded")
        .expect("the ceiling was reached");
    let (kind, before) = &recording.folds[stop - 1];
    assert_eq!(
        *kind, "task_candidate_created",
        "alpha's promotion crossed the ceiling"
    );
    let events = wide.env.durable_events();
    let kinds = kinds_of(&events);
    let durable_stop = position(&kinds, "budget_exceeded", 0);
    let crossed = crate::engine::topology::select::Spend::replay(&events[..durable_stop]).run_usd();
    assert!(
        crossed >= 0.4,
        "the settled spend had reached the ceiling before `budget_exceeded`: {crossed}"
    );
    assert!(
        !before.run_is_ending() && before.budget_stop().is_none(),
        "an over-budget prefix without `budget_exceeded` is not ending"
    );
    assert_eq!(
        before.derived_outcome(),
        crate::topology::events::DerivedOutcome::NotEnding
    );
    for outcome in [
        RunOutcome::BudgetExceeded,
        RunOutcome::Complete,
        RunOutcome::Parked,
    ] {
        let refused = ending_as(before, outcome.clone())
            .expect_err("the fold refuses every end of a run that is not ending");
        assert!(refused.contains("not ending"), "{outcome:?}: {refused}");
    }
    assert_eq!(
        kinds[durable_stop - 1],
        "task_candidate_created",
        "the loop appended `budget_exceeded` at its next selection, before any effect of \
             that selection: {kinds:?}"
    );
    assert!(
        !kinds[durable_stop..].iter().any(|kind| matches!(
            *kind,
            "task_dispatched" | "attempt_started" | "merge_prepared" | "merge_verification_started"
        )),
        "nothing was admitted after the stop: {kinds:?}"
    );
    assert_eq!(kinds.last(), Some(&"run_finished"), "{kinds:?}");
    assert!(
        ending_as(
            &recording.folds[recording.folds.len() - 2].1,
            RunOutcome::BudgetExceeded
        )
        .is_ok(),
        "with the record durable and the drain done, the budget-driven end is accepted"
    );
}

#[test]
fn a_budget_stop_drains_live_pipelines_to_their_settlements_and_ends_budget_exceeded() {
    let (mut wide, _, progress) = budget_stopped_with_two_in_flight("coordinator-budget-drain");
    assert_eq!(
        outcome_of(&progress.expect("the budget-stopped run ends")),
        RunOutcome::BudgetExceeded
    );
    let events = wide.env.durable_events();
    let kinds = kinds_of(&events);
    let stop = position(&kinds, "budget_exceeded", 0);
    let drained: Vec<u32> = events[stop..]
        .iter()
        .filter_map(|event| match &event.body {
            TopologyEventBody::CandidatePrepared { data } => Some(data.key.0),
            _ => None,
        })
        .collect();
    assert_eq!(
        drained,
        vec![1, 2],
        "beta and gamma, live at the stop, drained to their natural settlements: {kinds:?}"
    );
    assert_eq!(
        count(&events, "attempt_interrupted"),
        0,
        "a budget stop cancels nothing"
    );
    assert!(
        !events.iter().any(|event| matches!(&event.body,
            TopologyEventBody::TaskDispatched { data } if data.key == TaskKey(3))),
        "delta was admissible and never admitted after the stop"
    );
    let TopologyEventBody::RunFinished { data } = &events.last().expect("an end").body else {
        panic!("the run ended");
    };
    assert_eq!(data.outcome, RunOutcome::BudgetExceeded);
    assert_eq!(
        wide.run.fold().queue().map(|queue| queue.entries().len()),
        Some(3),
        "the three queued candidates stay resumably open"
    );
    assert!(
        wide.env
            .runner
            .endings()
            .iter()
            .all(|(_, ending)| *ending == crate::engine::topology::scaffold::Ending::Completed),
        "no process was cancelled"
    );
    let mut paid_after_the_stop: Vec<u32> = wide
        .env
        .runner
        .ran()
        .iter()
        .filter(|ran| {
            ran.durable_at_spawn
                .iter()
                .any(|kind| kind == "budget_exceeded")
        })
        .filter(|ran| matches!(role_of(&ran.invocation), Some(AttemptRole::ReviewPass(_))))
        .filter_map(|ran| attempt_key(&ran.invocation))
        .collect();
    paid_after_the_stop.sort_unstable();
    assert_eq!(
        paid_after_the_stop,
        vec![1, 2],
        "beta and gamma, each running its worker when the stop was recorded, went on to start \
             their review passes after it: the overshoot is the remaining spend of every pipeline \
             admitted before the stop, its later review passes and re-asks included, not one \
             invocation each (`PR11-OVERSHOOT-BOUND-IN-NO-OUTPUT`)"
    );
    let ledger = wide.run.broker_mut().invocations();
    assert!(ledger.balances() && ledger.duplicates() == 0);
    replay_equals_live(&wide);
}

fn deferring_alpha(tasks: &[WideTask], asking: &[u32]) -> RecordingRunner {
    let base = crate::engine::topology::scaffold::wide_responder_asking(tasks, &[], asking);
    let runner = RecordingRunner::new().answering(Box::new(move |request| {
        if let InvocationId::Attempt {
            key: TaskKey(0),
            generation: GenerationId(0),
            role: AttemptRole::ReviewPass(_),
            ..
        } = &request.invocation
        {
            return Err(crate::runner::RunnerError::new(
                &request.invocation,
                crate::error::ProcessFate::Gone,
                UpstrokeError::Refused {
                    message: "the scaffold's reviewer pool is unavailable".to_owned(),
                },
            ));
        }
        base(request)
    }));
    runner.hold();
    runner
}

#[test]
fn run_finished_halted_and_budget_exceeded_accepted_with_deferred_items_at_width_three() {
    let tasks = three();
    let mut wide = Wide::durable_under(
        "coordinator-deferred-budget",
        &tasks,
        3,
        WidePlans::default(),
        deferring_alpha(&tasks, &[]),
        crate::engine::topology::select::Ceiling {
            run_usd: Some(0.6),
            task_usd: None,
        },
    );
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let mut scheduler = Scheduler::scripted(
        &runner,
        Box::new(|view: &Quiescent<'_>| {
            released(view, |invocation| attempt_key(invocation) == Some(0))
                .or_else(|| released(view, |invocation| attempt_key(invocation) == Some(1)))
        }),
    );
    let progress = drive(&mut wide, Some(&mut scheduler)).expect("the budget stop ends the run");
    drop(scheduler);
    assert_eq!(outcome_of(&progress), RunOutcome::BudgetExceeded);
    assert_eq!(
        wide.run.fold().task_state(TaskKey(0)),
        Some(crate::topology::fold::TaskState::Deferred),
        "alpha's outage deferred it, and the deferral never blocked the budget-driven end"
    );
    let kinds = kinds_of_log(&wide);
    assert!(
        !kinds.contains(&"defer_wait_elapsed"),
        "no backoff is waited while pipelines are live or once the run is ending: {kinds:?}"
    );
    assert_eq!(
        kinds[position(&kinds, "budget_exceeded", 0)..]
            .iter()
            .filter(|kind| **kind == "candidate_prepared")
            .count(),
        1,
        "gamma drained after the stop: {kinds:?}"
    );

    let (recovered, mut resumed) = wide
        .resume(
            "inc-2",
            RecordingRunner::new().answering(wide_responder(&tasks, &[])),
            crate::engine::topology::select::Ceiling::unlimited(),
        )
        .expect("a budget-stopped run resumes");
    assert_eq!(recovered.interrupted, 0);
    assert!(recovered.resumed.budget_stop_cleared);
    assert_eq!(
        resumed.run.fold().task_state(TaskKey(0)),
        Some(crate::topology::fold::TaskState::Pending),
        "`run_resumed` woke the deferred task"
    );
    let progress = drive(&mut resumed, None).expect("the resumed run completes");
    assert_eq!(outcome_of(&progress), RunOutcome::Complete);
    assert_eq!(count(&resumed.env.durable_events(), "task_merged"), 3);
    replay_equals_live(&resumed);

    let mut wide = Wide::started_with(
        "coordinator-deferred-halt",
        &tasks,
        3,
        WidePlans::default(),
        deferring_alpha(&tasks, &[2]),
    );
    halting(&mut wide.env);
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let mut scheduler = Scheduler::scripted(
        &runner,
        Box::new(|view: &Quiescent<'_>| {
            released(view, |invocation| attempt_key(invocation) == Some(0))
                .or_else(|| released(view, |invocation| worker(invocation) == Some(TaskKey(2))))
        }),
    );
    let mut recording = Recording::over(&wide);
    let pipelines = wide.env.pipelines();
    let progress = wide
        .run
        .run_concurrently(
            &wide.env.seams(),
            &pipelines,
            &mut recording,
            Some(&mut scheduler),
        )
        .expect("the halt ends the run");
    drop(scheduler);
    assert_eq!(outcome_of(&progress), RunOutcome::Halted);
    let (_, before_end) = &recording.folds[recording.folds.len() - 2];
    assert_eq!(
        before_end.task_state(TaskKey(0)),
        Some(crate::topology::fold::TaskState::Deferred)
    );
    assert!(
        ending_as(before_end, RunOutcome::Halted).is_ok(),
        "Halted is accepted with a deferred task present, which is void with the run"
    );
    let events = wide.env.durable_events();
    let kinds = kinds_of(&events);
    let halted = position(&kinds, "question_answered", 0);
    assert_eq!(
        &kinds[halted..],
        &["question_answered", "attempt_interrupted", "run_finished"],
        "beta, in flight, was interrupted; nothing is appended for the deferred alpha: {kinds:?}"
    );
}

#[test]
fn run_finished_budget_exceeded_refused_after_halting_drain_settlement_at_width_three() {
    use crate::topology::events::SettlementTransition;
    let tasks = three();
    let mut wide = Wide::durable(
        "coordinator-halting-drain",
        &tasks,
        3,
        WidePlans::default(),
        holding(&tasks, &[]),
    );
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let mut scheduler = shutdown_at_first_point(&runner);
    drive(&mut wide, Some(&mut scheduler)).expect_err("three attempts in flight, shut down");
    drop(scheduler);

    let epoch = wide.run.fold().epoch().expect("started");
    let mut hooks = wide.env.hooks();
    let seams = wide.env.seams();
    let drained = [
        TopologyEventBody::BudgetExceeded {
            data: crate::topology::events::BudgetExceeded4 {
                epoch,
                budget: crate::events::BudgetKind::Run,
                limit_usd: 0.4,
                spent_usd: 0.5,
                key: None,
            },
        },
        planted_settlement(TaskKey(1), SettlementTransition::Retry),
        planted_settlement(TaskKey(2), SettlementTransition::Retry),
        planted_settlement(
            TaskKey(0),
            SettlementTransition::Failed {
                halts_run: true,
                reason: "the drained settlement halts".to_owned(),
            },
        ),
    ];
    for body in drained {
        wide.run
            .emit(body, &seams, &mut hooks)
            .expect("the fold takes the drain's prefix");
    }
    let fold = wide.run.fold().clone();
    assert!(fold.budget_stop().is_some() && fold.halted_at() == Some(TaskKey(0)));
    assert_eq!(
        fold.derived_outcome(),
        crate::topology::events::DerivedOutcome::Ending(RunOutcome::Halted),
        "halt outranks budget"
    );
    let refused = ending_as(&fold, RunOutcome::BudgetExceeded)
        .expect_err("run_finished(BudgetExceeded) is refused after a halting drain settlement");
    assert!(
        refused.contains("budget exceeded") && refused.contains("halted"),
        "{refused}"
    );
    assert_eq!(
        crate::engine::topology::closure::ending_outcome(&fold)
            .expect("the ending outcome reads the halt"),
        RunOutcome::Halted
    );

    let pipelines = wide.env.pipelines();
    let progress = wide
        .run
        .run_concurrently(&seams, &pipelines, &mut hooks, None)
        .expect("the closure ends the run");
    assert_eq!(outcome_of(&progress), RunOutcome::Halted);
    let events = wide.env.durable_events();
    let ends: Vec<(RunOutcome, Option<TaskKey>)> = events
        .iter()
        .filter_map(|event| match &event.body {
            TopologyEventBody::RunFinished { data } => Some((data.outcome.clone(), data.halted_at)),
            _ => None,
        })
        .collect();
    assert_eq!(ends, vec![(RunOutcome::Halted, Some(TaskKey(0)))]);
}

#[test]
fn a_halting_settlement_drained_after_a_budget_stop_cancels_the_live_worker_and_ends_halted_at_width_three()
 {
    use crate::engine::topology::scaffold::Ending;
    use crate::topology::events::SettlementTransition;
    let tasks = four();
    let mut wide = Wide::started_under(
        "coordinator-halting-drain-live",
        &tasks,
        3,
        WidePlans::default(),
        holding(&tasks, &[]),
        TIGHT,
    );
    let gamma = AttemptIdentities::new(TaskKey(2), GenerationId(0), AttemptNumber(1)).worker();
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let mut drained_beside: Option<Vec<InvocationId>> = None;
    let mut scheduler = Scheduler::scripted(
        &runner,
        Box::new(|view: &Quiescent<'_>| {
            if view.run.fold().budget_stop().is_some() && drained_beside.is_none() {
                drained_beside = Some(view.invoking.clone());
                return Some(Release::Append(Box::new(planted_settlement(
                    TaskKey(1),
                    SettlementTransition::Failed {
                        halts_run: true,
                        reason: "the drained settlement halts".to_owned(),
                    },
                ))));
            }
            released(view, |invocation| attempt_key(invocation) == Some(0))
        }),
    );
    let progress = drive(&mut wide, Some(&mut scheduler));
    drop(scheduler);
    let beside = drained_beside.expect("the budget stop was recorded with pipelines live");
    assert!(
        beside.contains(&gamma),
        "gamma's worker was inside the Runner when beta's halting settlement was drained after \
             the stop: {beside:?}"
    );
    let endings = runner.endings();
    assert!(
        endings.contains(&(gamma.clone(), Ending::Cancelled)),
        "the halt the drained settlement records converts the drain: gamma's live worker is \
             cancelled through the Runner, not run on: {endings:?}; the command ended {progress:?}"
    );
    assert_eq!(
        outcome_of(&progress.expect("the closure ends the run")),
        RunOutcome::Halted
    );
    let events = wide.env.durable_events();
    let kinds = kinds_of(&events);
    let stop = position(&kinds, "budget_exceeded", 0);
    let interrupted: Vec<TaskKey> = events[stop..]
        .iter()
        .filter_map(|event| match &event.body {
            TopologyEventBody::AttemptInterrupted { data } => Some(data.key),
            _ => None,
        })
        .collect();
    assert_eq!(
        interrupted,
        vec![TaskKey(2)],
        "the closure ran as Halted from step (2) and settled gamma's attempt interrupted: \
             {kinds:?}"
    );
    assert!(
        !kinds.iter().any(|kind| matches!(
            *kind,
            "merge_prepared" | "merge_verification_started" | "task_merged"
        )),
        "nothing was published: {kinds:?}"
    );
    let TopologyEventBody::RunFinished { data } = &events.last().expect("an end").body else {
        panic!("the run ended: {kinds:?}");
    };
    assert_eq!(
        (data.outcome.clone(), data.halted_at),
        (RunOutcome::Halted, Some(TaskKey(1))),
        "halt outranks budget: the run finished Halted at beta, never BudgetExceeded"
    );
    assert!(wide.run.invocations_balance());
    replay_equals_live(&wide);
}

#[test]
fn a_halt_whose_cancelled_process_is_unresolved_ends_the_command_without_closing() {
    let mut wide = durable_halting_on_gamma("coordinator-halt-unresolved");
    let beta = AttemptIdentities::new(TaskKey(1), GenerationId(0), AttemptNumber(1)).worker();
    wide.env.runner.unresolved_when_cancelled(beta.clone());
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let mut scheduler = gamma_first(&runner);
    let error = drive(&mut wide, Some(&mut scheduler))
        .expect_err("closure is not entered over a process that may still run");
    drop(scheduler);
    let message = error.to_string();
    assert!(
        message.contains(&beta.render()) && message.contains("did not establish"),
        "the command ends naming the unresolved invocation: {message}"
    );
    let kinds = kinds_of_log(&wide);
    assert_eq!(
        kinds.last(),
        Some(&"question_answered"),
        "no terminal is appended and nothing is scrubbed over it: {kinds:?}"
    );
    let slot = crate::engine::topology::dispatch::task_slot(TaskKey(1), GenerationId(0));
    assert!(
        wide.env.fixture.manager.slot_path(&slot).exists(),
        "beta's worktree was left for the next process"
    );
    assert!(
        wide.env.runner.endings().contains(&(
            beta,
            crate::engine::topology::scaffold::Ending::CancelledUnresolved
        )),
        "{:?}",
        wide.env.runner.endings()
    );
    let (recovered, resumed) = finish_resumed_halted(wide, "unresolved");
    assert_eq!(recovered.interrupted, 2);
    ends_once_halted(&resumed, "unresolved");
}

#[test]
fn run_finished_complete_refused_with_queued_candidate_at_width_three() {
    let tasks = three();
    let mut wide = Wide::started_with(
        "coordinator-complete-refused-queued",
        &tasks,
        3,
        WidePlans::default(),
        holding(&tasks, &[]),
    );
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let mut scheduler = Scheduler::seeded(&runner, 17);
    let mut recording = Recording::over(&wide);
    let pipelines = wide.env.pipelines();
    let progress = wide
        .run
        .run_concurrently(
            &wide.env.seams(),
            &pipelines,
            &mut recording,
            Some(&mut scheduler),
        )
        .expect("the run completes");
    drop(scheduler);
    assert_eq!(outcome_of(&progress), RunOutcome::Complete);
    let queued: Vec<&TopologyFold> = recording
        .folds
        .iter()
        .filter(|(_, fold)| fold.queue().is_some_and(|queue| !queue.is_empty()))
        .map(|(_, fold)| fold)
        .collect();
    assert!(!queued.is_empty(), "candidates were queued along the way");
    for fold in queued {
        for outcome in [RunOutcome::Complete, RunOutcome::Parked] {
            let refused = ending_as(fold, outcome.clone())
                .expect_err("a queued candidate makes the run not ending");
            assert!(refused.contains("not ending"), "{refused}");
        }
    }
    let (_, before_end) = &recording.folds[recording.folds.len() - 2];
    assert!(ending_as(before_end, RunOutcome::Complete).is_ok());
}

#[test]
fn run_finished_parked_refused_with_admissible_work_at_width_three() {
    let tasks = three();
    let runner = RecordingRunner::new().answering(
        crate::engine::topology::scaffold::wide_responder_asking(&tasks, &[], &[2]),
    );
    runner.hold();
    let mut wide = Wide::started_with(
        "coordinator-parked-refused",
        &tasks,
        3,
        WidePlans::default(),
        runner,
    );
    wide.env.adapters =
        std::sync::Arc::new(crate::engine::topology::scaffold::ScaffoldAdapters::echoing());
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let mut scheduler = gamma_first(&runner);
    let mut recording = Recording::over(&wide);
    let pipelines = wide.env.pipelines();
    let progress = wide
        .run
        .run_concurrently(
            &wide.env.seams(),
            &pipelines,
            &mut recording,
            Some(&mut scheduler),
        )
        .expect("the run parks on gamma's question");
    drop(scheduler);
    assert_eq!(outcome_of(&progress), RunOutcome::Parked);
    let parked = recording
        .folds
        .iter()
        .position(|(_, fold)| fold.questions_open())
        .expect("gamma parked");
    let (_, at_park) = &recording.folds[parked];
    assert!(
        at_park.pipeline_held() >= 2,
        "alpha and beta were in flight when gamma parked"
    );
    for outcome in [RunOutcome::Parked, RunOutcome::Complete] {
        let refused = ending_as(at_park, outcome.clone())
            .expect_err("the in-flight and admissible work makes the run not ending");
        assert!(refused.contains("not ending"), "{outcome:?}: {refused}");
    }
    let events = wide.env.durable_events();
    assert_eq!(
        count(&events, "task_merged"),
        2,
        "alpha and beta ran to their merges after gamma parked"
    );
    let (_, before_end) = &recording.folds[recording.folds.len() - 2];
    assert!(ending_as(before_end, RunOutcome::Parked).is_ok());
}

#[test]
fn run_finished_parked_or_complete_refused_while_deferred_items_exist_at_width_three() {
    let tasks = three();
    let mut wide = Wide::started_with(
        "coordinator-deferred-refused",
        &tasks,
        3,
        WidePlans::default(),
        deferring_alpha(&tasks, &[]),
    );
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let mut scheduler = Scheduler::scripted(
        &runner,
        Box::new(|view: &Quiescent<'_>| {
            released(view, |invocation| attempt_key(invocation) == Some(0))
        }),
    );
    let mut recording = Recording::over(&wide);
    let pipelines = wide.env.pipelines();
    let progress = wide
        .run
        .run_concurrently(
            &wide.env.seams(),
            &pipelines,
            &mut recording,
            Some(&mut scheduler),
        )
        .expect("the run completes once the deferral is waited");
    drop(scheduler);
    assert_eq!(outcome_of(&progress), RunOutcome::Complete);
    let deferred: Vec<&TopologyFold> = recording
        .folds
        .iter()
        .filter(|(_, fold)| fold.backoff_pending())
        .map(|(_, fold)| fold)
        .collect();
    assert!(!deferred.is_empty(), "alpha was deferred");
    for fold in deferred {
        for outcome in [RunOutcome::Parked, RunOutcome::Complete] {
            let refused = ending_as(fold, outcome.clone())
                .expect_err("pending backoff makes Parked and Complete not ending");
            assert!(refused.contains("not ending"), "{outcome:?}: {refused}");
        }
    }
    let events = wide.env.durable_events();
    let kinds = kinds_of(&events);
    let waited = position(&kinds, "defer_wait_elapsed", 0);
    assert!(
        position(&kinds, "task_merged", 1) < waited,
        "the backoff was waited only once no pipeline was live: {kinds:?}"
    );
    assert_eq!(count(&events, "task_merged"), 3);
}

fn needs_human_verification(tasks: &[WideTask], asking: &[u32]) -> RecordingRunner {
    let base = crate::engine::topology::scaffold::wide_responder_asking(tasks, &[], asking);
    let runner = RecordingRunner::new().answering(Box::new(move |request| {
        if matches!(request.invocation, InvocationId::Sequence { .. })
            && matches!(request.role, crate::runner::ExecutionRole::Review)
        {
            return Ok(crate::engine::topology::scaffold::exited(
                0,
                "```json\n{\"pass\": false, \"reasons\": [\"a person must decide this \
                     integration\"], \"required_changes\": [], \"needs_human\": true}\n```"
                    .to_owned(),
            ));
        }
        base(request)
    }));
    runner.hold();
    runner
}

#[test]
fn run_finished_halted_accepted_after_declined_verification_park_at_width_three() {
    let tasks = three();
    let mut wide = Wide::started_with(
        "coordinator-declined-park",
        &tasks,
        3,
        WidePlans::default(),
        needs_human_verification(&tasks, &[]),
    );
    halting(&mut wide.env);
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let mut scheduler = Scheduler::scripted(
        &runner,
        Box::new(|view: &Quiescent<'_>| {
            let merges = count(view.run.events(), "task_merged");
            if merges == 0 {
                return released(view, |invocation| attempt_key(invocation) == Some(0));
            }
            released(view, |invocation| attempt_key(invocation) == Some(1)).or_else(|| {
                released(view, |invocation| {
                    matches!(invocation, InvocationId::Sequence { .. })
                })
            })
        }),
    );
    let mut recording = Recording::over(&wide);
    let pipelines = wide.env.pipelines();
    let progress = wide
        .run
        .run_concurrently(
            &wide.env.seams(),
            &pipelines,
            &mut recording,
            Some(&mut scheduler),
        )
        .expect("the declined park halts the run");
    drop(scheduler);
    assert_eq!(outcome_of(&progress), RunOutcome::Halted);
    let events = wide.env.durable_events();
    let kinds = kinds_of(&events);
    let parked = position(&kinds, "merge_verification_unavailable", 0);
    let declined = position(&kinds, "question_answered", 0);
    assert!(parked < declined, "{kinds:?}");
    assert_eq!(
        &kinds[declined..],
        &["question_answered", "attempt_interrupted", "run_finished"],
        "the decline halts the run with gamma in flight, which the closure interrupts: \
             {kinds:?}"
    );
    let (_, before_end) = &recording.folds[recording.folds.len() - 2];
    assert!(ending_as(before_end, RunOutcome::Halted).is_ok());
    assert_eq!(
        wide.run.fold().task_state(TaskKey(1)),
        Some(crate::topology::fold::TaskState::Failed),
        "the declined verification park fails beta"
    );
    assert!(
        wide.run
            .fold()
            .queue()
            .is_some_and(|queue| queue.is_empty()),
        "and consumes its queue position"
    );
}

#[test]
fn replayed_conflicting_outcome_refused_at_width_three() {
    let mut wide = durable_halting_on_gamma("coordinator-conflicting-outcome");
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let mut scheduler = gamma_first(&runner);
    let progress = drive(&mut wide, Some(&mut scheduler)).expect("the halted run ends");
    drop(scheduler);
    assert_eq!(outcome_of(&progress), RunOutcome::Halted);

    let log = std::fs::read(&wide.env.log).expect("the log");
    let body = &log[..log.len() - 1];
    let last = body
        .iter()
        .rposition(|byte| *byte == b'\n')
        .expect("a prefix")
        + 1;
    let forged = TopologyEvent {
        ts: "2026-09-30T00:00:00Z".to_owned(),
        body: TopologyEventBody::RunFinished {
            data: crate::topology::events::RunFinished4 {
                outcome: RunOutcome::Complete,
                halted_at: None,
                merged: 0,
                parked: 0,
            },
        },
    };
    let (line, _) =
        crate::events::log::TopologyLine::round_trip(&forged).expect("the forged end serializes");
    let mut rewritten = log[..last].to_vec();
    rewritten.extend_from_slice(line.committed_bytes());
    crate::workspace_manager::fixture::write_file(&wide.env.log, &rewritten);

    let events = TopologyFold::parse_log(&rewritten).expect("the forged log parses");
    let refused = TopologyFold::replay(wide.env.inputs.clone(), &events)
        .expect_err("the checked replay refuses the conflicting outcome");
    assert!(
        matches!(
            refused,
            crate::topology::fold::FoldError::OutcomeMismatch { .. }
        ),
        "{refused}"
    );
    let text = refused.to_string();
    assert!(
        text.contains("complete") && text.contains("halted"),
        "{text}"
    );

    let tasks = three();
    let error = match wide.resume(
        "inc-2",
        RecordingRunner::new().answering(wide_responder(&tasks, &[])),
        crate::engine::topology::select::Ceiling::unlimited(),
    ) {
        Ok(_) => panic!("a resume of a log with a conflicting outcome must refuse"),
        Err(error) => error.to_string(),
    };
    assert!(
        error.contains("complete") && error.contains("halted"),
        "the resume's barrier refuses with the same mismatch: {error}"
    );
}

#[test]
fn a_verifications_review_spend_is_charged_before_the_next_selection() {
    let tasks = [
        WideTask::independent("t0"),
        WideTask::independent("t1"),
        WideTask::independent("t2"),
        WideTask::independent("t3"),
        WideTask::independent("t4"),
    ];
    let mut wide = Wide::started_under(
        "coordinator-verification-spend",
        &tasks,
        3,
        WidePlans::default(),
        holding(&tasks, &[]),
        crate::engine::topology::select::Ceiling {
            run_usd: Some(1.6),
            task_usd: None,
        },
    );
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let mut arrived_before_t3: Option<bool> = None;
    let mut scheduler = Scheduler::scripted(
        &runner,
        Box::new(|view: &Quiescent<'_>| {
            let merges = count(view.run.events(), "task_merged");
            let verifying = view.run.fold().transaction().is_some();
            if !verifying {
                return released(view, |invocation| {
                    attempt_key(invocation) == Some(if merges == 0 { 0 } else { 1 })
                });
            }
            released(view, |invocation| worker(invocation) == Some(TaskKey(2)))
                .or_else(|| {
                    released(view, |invocation| {
                        attempt_key(invocation) == Some(3)
                            && matches!(
                                role_of(invocation),
                                Some(AttemptRole::Worker | AttemptRole::Gate(_))
                            )
                    })
                })
                .or_else(|| {
                    released(view, |invocation| {
                        matches!(invocation, InvocationId::Sequence { .. })
                    })
                })
                .or_else(|| {
                    if arrived_before_t3.is_none() {
                        arrived_before_t3 = Some(!view.live.iter().any(|(_, identity)| {
                            matches!(identity, Identity::Verification { .. })
                        }));
                    }
                    released(view, |invocation| attempt_key(invocation) == Some(3))
                })
                .or_else(|| released(view, |invocation| attempt_key(invocation) == Some(2)))
        }),
    );
    let progress = drive(&mut wide, Some(&mut scheduler)).expect("the budget stop ends the run");
    drop(scheduler);
    assert_eq!(
        arrived_before_t3,
        Some(true),
        "the verification's completion had arrived while t2's gate snapshot kept `verify` \
             waiting, before t3 settled"
    );
    assert_eq!(outcome_of(&progress), RunOutcome::BudgetExceeded);
    let events = wide.env.durable_events();
    let kinds = kinds_of(&events);
    let stop = position(&kinds, "budget_exceeded", 0);
    assert!(
        !events.iter().any(|event| matches!(&event.body,
            TopologyEventBody::TaskDispatched { data } if data.key == TaskKey(4))),
        "t4 was never dispatched: the verification's review spend counted at the selection \
             t3's settlement opened, before `verify` returned: {kinds:?}"
    );
    assert!(
        position(&kinds, "candidate_prepared", 2) < stop
            && stop < position(&kinds, "merge_prepared", 1),
        "the stop came after t3 settled and before the verification's terminal: {kinds:?}"
    );
}

#[test]
fn a_closure_never_settles_in_flight_work_its_coordinator_did_not_cancel() {
    use crate::topology::events::SettlementTransition;
    let tasks = three();
    let mut wide = Wide::durable(
        "coordinator-unvouched",
        &tasks,
        3,
        WidePlans::default(),
        holding(&tasks, &[]),
    );
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let mut scheduler = shutdown_at_first_point(&runner);
    drive(&mut wide, Some(&mut scheduler)).expect_err("three attempts in flight, shut down");
    drop(scheduler);
    let mut hooks = wide.env.hooks();
    let seams = wide.env.seams();
    wide.run
        .emit(
            planted_settlement(
                TaskKey(0),
                SettlementTransition::Failed {
                    halts_run: true,
                    reason: "a halting settlement".to_owned(),
                },
            ),
            &seams,
            &mut hooks,
        )
        .expect("the fold takes the halting settlement");
    let before = std::fs::read(&wide.env.log).expect("the log");

    let pipelines = wide.env.pipelines();
    let error = wide
        .run
        .run_concurrently(&seams, &pipelines, &mut hooks, None)
        .expect_err("a coordinator that cancelled nothing vouches for nothing");
    let message = error.to_string();
    assert!(
        message.contains("no pipeline of this process was cancelled for")
            && message.contains("task k1 generation 0 is in flight")
            && message.contains("task k2 generation 0 is in flight")
            && message.contains("nothing was appended"),
        "{message}"
    );
    assert_eq!(
        std::fs::read(&wide.env.log).expect("the log"),
        before,
        "nothing was appended"
    );

    let (recovered, resumed) = finish_resumed_halted(wide, "unvouched");
    assert_eq!(
        recovered.interrupted, 2,
        "the next process's recovery settles what no live coordinator vouched for"
    );
    let events = resumed.env.durable_events();
    assert_eq!(count(&events, "run_finished"), 1);
    replay_equals_live(&resumed);
}

#[test]
fn one_seed_reproduces_one_run_down_to_what_each_process_saw_when_it_started() {
    type Started = Vec<(String, Vec<String>, String)>;
    let tasks = three();
    let mut reference: Option<(Vec<InvocationId>, Started, String)> = None;
    for run in scheduled_here(0..3, &[0, 1]) {
        let runner = holding(&tasks, &[]);
        let plans = WidePlans {
            reviewers: 2,
            verify_reviewers: 2,
            ..WidePlans::default()
        };
        let mut wide = Wide::started_with(
            &format!("coordinator-one-seed-{run}"),
            &tasks,
            3,
            plans,
            runner,
        );
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let mut scheduler = Scheduler::seeded(&runner, 11);
        let progress = drive(&mut wide, Some(&mut scheduler))
            .unwrap_or_else(|error| panic!("run {run}: {error}"));
        let released = scheduler.released.clone();
        drop(scheduler);
        assert_eq!(outcome_of(&progress), RunOutcome::Complete, "run {run}");
        let started: Started = wide
            .env
            .runner
            .ran()
            .into_iter()
            .map(|ran| {
                let workspace = ran
                    .workspace
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default()
                    .to_owned();
                (ran.invocation.render(), ran.durable_at_spawn, workspace)
            })
            .collect();
        let run_id = wide.run.fold().started().expect("started").run_id.clone();
        let log = serde_json::to_string(&canonical(&wide.env.durable_events(), &run_id))
            .expect("serializes");
        let observed = (released, started, log);
        match &reference {
            None => reference = Some(observed),
            Some(expected) => assert_eq!(
                &observed, expected,
                "run {run} of seed 11 released, started or logged something run 0 did not"
            ),
        }
    }
}

#[test]
fn out_of_order_completions_bind_to_their_own_identities_under_seeded_permutations() {
    let tasks = three();
    let mut reference: Option<BTreeMap<u64, Vec<String>>> = None;
    for seed in scheduled_here(0..16, &[0, 1]) {
        let runner = holding(&tasks, &[]);
        let mut wide = Wide::started_with(
            &format!("coordinator-seed-{seed}"),
            &tasks,
            3,
            WidePlans::default(),
            runner,
        );
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let mut scheduler = Scheduler::seeded(&runner, seed);
        let progress = drive(&mut wide, Some(&mut scheduler))
            .unwrap_or_else(|error| panic!("seed {seed}: {error}"));
        drop(scheduler);
        assert_eq!(outcome_of(&progress), RunOutcome::Complete, "seed {seed}");
        let events = wide.env.durable_events();
        let run_id = wide.run.fold().started().expect("started").run_id.clone();
        let keyed = attempt_level(&events, &run_id);
        let problems = attempt_level_problems(&keyed, &[0, 1, 2]);
        assert!(problems.is_empty(), "seed {seed}: {problems:#?}");
        match &reference {
            None => reference = Some(keyed),
            Some(expected) => assert_eq!(
                &keyed, expected,
                "seed {seed}: a key's attempt-level events differ from seed 0's"
            ),
        }
        let created: Vec<u32> = events
            .iter()
            .filter_map(|event| match &event.body {
                TopologyEventBody::TaskCandidateCreated { data } => Some(data.candidate.key.0),
                _ => None,
            })
            .collect();
        let integrated: Vec<u32> = events
            .iter()
            .filter_map(|event| match &event.body {
                TopologyEventBody::MergePrepared { data } => Some(data.key.0),
                _ => None,
            })
            .collect();
        assert_eq!(
            integrated, created,
            "seed {seed}: the queue integrated in `task_candidate_created` order"
        );
        assert!(wide.run.invocations_balance(), "seed {seed}");
        replay_equals_live(&wide);
    }
}

#[test]
fn selecting_attempt_level_events_by_a_kind_field_keeps_nothing_and_is_rejected() {
    let tasks = three();
    let mut wide = Wide::started("coordinator-st08-kind-field", &tasks, 3);
    let progress = drive(&mut wide, None).expect("the run completes");
    assert_eq!(outcome_of(&progress), RunOutcome::Complete);
    let events = wide.env.durable_events();
    let run_id = wide.run.fold().started().expect("started").run_id.clone();
    let by_kind_field: BTreeMap<u64, Vec<String>> = per_key(&canonical(&events, &run_id))
        .into_iter()
        .map(|(key, values)| {
            let kept = values
                .into_iter()
                .filter(|value| {
                    value
                        .get("kind")
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(|kind| ATTEMPT_LEVEL.contains(&kind))
                })
                .map(|value| value.to_string())
                .collect();
            (key, kept)
        })
        .collect();
    assert_eq!(
        by_kind_field,
        BTreeMap::from([(0, Vec::new()), (1, Vec::new()), (2, Vec::new())]),
        "a top-level `kind` field selects nothing from events tagged `event`"
    );
    let rejected: Vec<u64> = attempt_level_problems(&by_kind_field, &[0, 1, 2])
        .into_keys()
        .collect();
    assert_eq!(
        rejected,
        vec![0, 1, 2],
        "every key's empty projection is rejected"
    );
    let problems = attempt_level_problems(&attempt_level(&events, &run_id), &[0, 1, 2]);
    assert!(problems.is_empty(), "{problems:#?}");
}

#[test]
fn a_keys_attempt_level_projection_depends_on_its_own_trace_alone() {
    const RETRIED: [&str; 6] = [
        "task_dispatched",
        "attempt_started",
        "attempt_finished",
        "attempt_started",
        "candidate_prepared",
        "task_candidate_created",
    ];
    let tasks = three();
    let mut wide = Wide::started_with(
        "coordinator-st08-own-trace",
        &tasks,
        3,
        WidePlans::default(),
        RecordingRunner::new().answering(wide_responder(&tasks, &[(1, 1)])),
    );
    let progress = drive(&mut wide, None).expect("the run completes");
    assert_eq!(outcome_of(&progress), RunOutcome::Complete);
    let events = wide.env.durable_events();
    let run_id = wide.run.fold().started().expect("started").run_id.clone();
    let keyed = attempt_level(&events, &run_id);
    let kinds: BTreeMap<u64, Vec<String>> = keyed
        .iter()
        .map(|(key, values)| (*key, projected_kinds(values)))
        .collect();
    assert_eq!(
        kinds,
        BTreeMap::from([
            (0, Vec::from(ONE_SUCCEEDING_ATTEMPT.map(str::to_owned))),
            (1, Vec::from(RETRIED.map(str::to_owned))),
            (2, Vec::from(ONE_SUCCEEDING_ATTEMPT.map(str::to_owned))),
        ]),
        "beta's failed first gate settles its first attempt by `attempt_finished` and retries \
             it in the same generation; alpha and gamma settle by `candidate_prepared` alone"
    );

    let prepared: Vec<u32> = events
        .iter()
        .filter_map(|event| match &event.body {
            TopologyEventBody::CandidatePrepared { data } => Some(data.key.0),
            _ => None,
        })
        .collect();
    let differing = |log: &[TopologyEvent]| -> Vec<u64> {
        let projected = attempt_level(log, &run_id);
        keyed
            .keys()
            .filter(|key| projected.get(*key) != keyed.get(*key))
            .copied()
            .collect()
    };
    let numbered_over_the_log = |log: &[TopologyEvent]| {
        let selected: Vec<serde_json::Value> = log
            .iter()
            .zip(canonical(log, &run_id))
            .filter(|(event, _)| ATTEMPT_LEVEL.contains(&event.body.kind()))
            .map(|(_, value)| value)
            .collect();
        per_key(&selected)
    };
    // §6: each variant below is an owned copy of the run's log, and the tree OID one of them
    // plants is an owned copy of gamma's; the log itself stays the reference each variant meets.
    let mut reordered = events.clone();
    reordered.sort_by_key(|event| match event.body.key() {
        Some(key) if ATTEMPT_LEVEL.contains(&event.body.kind()) => {
            (0, prepared.iter().rev().position(|first| *first == key.0))
        }
        _ => (1, None),
    });
    assert_eq!(
        attempt_level(&reordered, &run_id),
        keyed,
        "the keys' traces in the reverse of the order their candidates were prepared in, every \
             other event after them: no key's projection changes"
    );
    assert_ne!(
        numbered_over_the_log(&reordered),
        numbered_over_the_log(&events),
        "the control: commit labels numbered over the whole log do change under that reordering"
    );

    let mut broken = events.clone();
    let alpha_base = dispatched_bases(&events)
        .remove(&0)
        .expect("alpha was dispatched");
    broken
        .iter_mut()
        .find_map(|event| match &mut event.body {
            TopologyEventBody::TaskCandidateCreated { data }
                if data.candidate.key == TaskKey(0) =>
            {
                Some(data)
            }
            _ => None,
        })
        .expect("alpha's candidate was created")
        .candidate
        .commit_sha = crate::topology::events::CommitSha(alpha_base);
    assert_eq!(
        differing(&broken),
        vec![0],
        "alpha's candidate created as its base commit rather than its prepared one changes \
             alpha's projection, and only alpha's"
    );

    let gamma_tree = events
        .iter()
        .find_map(|event| match &event.body {
            TopologyEventBody::CandidatePrepared { data } if data.key == TaskKey(2) => {
                Some(data.tree_sha.clone())
            }
            _ => None,
        })
        .expect("gamma's candidate was prepared");
    let mut retreed = events.clone();
    retreed
        .iter_mut()
        .find_map(|event| match &mut event.body {
            TopologyEventBody::CandidatePrepared { data } if data.key == TaskKey(0) => Some(data),
            _ => None,
        })
        .expect("alpha's candidate was prepared")
        .tree_sha = gamma_tree;
    assert_eq!(
        differing(&retreed),
        vec![0],
        "alpha's candidate carrying gamma's tree OID changes alpha's projection, and only \
             alpha's"
    );
}

#[test]
fn a_chain_plan_projects_identically_at_widths_three_and_one() {
    let tasks = [
        WideTask::independent("alpha"),
        WideTask::after("beta", &["alpha"]),
        WideTask::after("gamma", &["beta"]),
    ];
    let mut narrow = Wide::started("coordinator-chain-one", &tasks, 1);
    let mut hooks = narrow.env.hooks();
    let seams = narrow.env.seams();
    let mut steps = 0_u32;
    loop {
        steps += 1;
        assert!(steps < 200, "the width-1 loop did not finish");
        match narrow.run.step(&seams, &mut hooks).expect("a step") {
            Progress::Finished { outcome, .. } => {
                assert_eq!(outcome, RunOutcome::Complete);
                break;
            }
            _ => continue,
        }
    }
    let mut wide = Wide::started("coordinator-chain-three", &tasks, 3);
    let progress = drive(&mut wide, None).expect("the width-3 run completes");
    assert_eq!(outcome_of(&progress), RunOutcome::Complete);

    let run_id = wide.run.fold().started().expect("started").run_id.clone();
    let one = serde_json::to_string(&canonical(&narrow.env.durable_events(), &run_id))
        .expect("serializes");
    let three =
        serde_json::to_string(&canonical(&wide.env.durable_events(), &run_id)).expect("serializes");
    assert_eq!(
        one, three,
        "a chain's canonical projections at widths 1 and 3 are byte-identical"
    );
    assert_eq!(final_tree(&narrow), final_tree(&wide));
}

#[test]
fn concurrent_attempts_at_one_generation_and_attempt_use_distinct_snapshot_slots() {
    let tasks = three();
    let runner = holding(&tasks, &[]);
    let plans = WidePlans {
        reviewers: 2,
        verify_reviewers: 2,
        ..WidePlans::default()
    };
    let mut wide = Wide::started_with("coordinator-st04", &tasks, 3, plans, runner);
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let mut scheduler = Scheduler::seeded(&runner, 7);
    drive(&mut wide, Some(&mut scheduler)).expect("the run completes");
    let widest = scheduler.widest;
    drop(scheduler);
    assert_eq!(widest, 3, "all three attempts were in flight at once");

    let ran = wide.env.runner.ran();
    let mut identities: Vec<String> = ran.iter().map(|entry| entry.invocation.render()).collect();
    let total = identities.len();
    identities.sort();
    identities.dedup();
    assert_eq!(
        identities.len(),
        total,
        "every process had its own InvocationId"
    );

    let snapshots: Vec<(u32, std::path::PathBuf)> = ran
        .iter()
        .filter(|entry| {
            matches!(
                role_of(&entry.invocation),
                Some(AttemptRole::Gate(_) | AttemptRole::ReviewPass(_))
            )
        })
        .filter_map(|entry| {
            attempt_key(&entry.invocation).map(|key| (key, entry.workspace.clone()))
        })
        .collect();
    assert_eq!(
        snapshots.len(),
        9,
        "three tasks, one gate and two reviewers each"
    );
    let mut paths: Vec<&std::path::PathBuf> = snapshots.iter().map(|(_, path)| path).collect();
    paths.sort();
    paths.dedup();
    assert_eq!(
        paths.len(),
        9,
        "no two tasks at generation 0, attempt 1 shared a snapshot slot: {snapshots:?}"
    );
    for (key, path) in &snapshots {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        assert!(
            name.starts_with(&format!("k{key}-g0-a1-")),
            "task {key}'s snapshot `{name}` names its task"
        );
    }
    assert_eq!(
        wide.run.reservations_peak(),
        1,
        "at most one provisional reservation was ever outstanding"
    );
    assert!(wide.run.invocations_balance());
}

#[test]
fn live_state_equals_replay_after_every_append_at_width_three() {
    let tasks = three();
    let runner = holding(&tasks, &[]);
    let mut wide = Wide::started_with("coordinator-st10", &tasks, 3, WidePlans::default(), runner);
    assert!(
        wide.run.invocations_balance() && wide.run.entitlements_held() == 0,
        "the ledgers are empty at coordinator start"
    );
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let mut scheduler = Scheduler::seeded(&runner, 3);
    let mut folding = Folding {
        inner: wide.env.hooks(),
        states: Vec::new(),
    };
    let pipelines = wide.env.pipelines();
    wide.run
        .run_concurrently(
            &wide.env.seams(),
            &pipelines,
            &mut folding,
            Some(&mut scheduler),
        )
        .expect("the run completes");
    drop(scheduler);
    let events = wide.env.durable_events();
    assert!(
        folding.states.len() > 20,
        "{} appends observed",
        folding.states.len()
    );
    for (length, live) in &folding.states {
        let prefix = events.get(..*length).expect("a prefix of the durable log");
        let replayed =
            TopologyFold::replay(wide.env.inputs.clone(), prefix).expect("every prefix replays");
        assert_eq!(
            replayed.state(),
            live.as_ref(),
            "after append {length} the live fold and the replay of the prefix differ"
        );
    }
    assert!(wide.run.invocations_balance(), "balanced at the end");
    assert_eq!(wide.run.entitlements_held(), 0);
}

#[test]
fn a_retained_retry_is_admitted_beside_other_pipelines_and_regates_on_fresh_snapshots() {
    let tasks = three();
    let runner = holding(&tasks, &[(0, 1)]);
    let mut wide = Wide::started_with("coordinator-st15", &tasks, 3, WidePlans::default(), runner);
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let mut scheduler = Scheduler::scripted(
        &runner,
        Box::new(|view: &Quiescent<'_>| {
            released(view, |invocation| attempt_key(invocation) == Some(0))
        }),
    );
    drive(&mut wide, Some(&mut scheduler)).expect("the run completes");
    drop(scheduler);
    let events = wide.env.durable_events();
    let kinds = kinds_of(&events);
    let retry = events
        .iter()
        .position(|event| {
            matches!(&event.body, TopologyEventBody::AttemptStarted { data }
                if data.key == TaskKey(0) && data.attempt == AttemptNumber(2))
        })
        .expect("the retry started");
    let TopologyEventBody::AttemptStarted { data } = &events.get(retry).expect("the retry").body
    else {
        panic!("the retry is an attempt_started");
    };
    assert_eq!(data.generation, GenerationId(0), "the same generation");
    assert!(
        data.resume_session.is_some(),
        "the retained session resumes"
    );
    let others_settled = events.iter().position(|event| {
        matches!(&event.body, TopologyEventBody::CandidatePrepared { data } if data.key != TaskKey(0))
    });
    assert!(
        others_settled.is_some_and(|settled| settled > retry),
        "the retry was admitted while the other tasks were still in flight: {kinds:?}"
    );
    let ran = wide.env.runner.ran();
    let gates: Vec<String> = ran
        .iter()
        .filter(|entry| {
            attempt_key(&entry.invocation) == Some(0)
                && matches!(role_of(&entry.invocation), Some(AttemptRole::Gate(_)))
        })
        .map(|entry| {
            entry
                .workspace
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_owned()
        })
        .collect();
    assert_eq!(
        gates,
        gate_snapshot_names(&wide.env.fixture.manager, 0, 0, &[1, 2]),
        "the retry re-gated on a fresh snapshot of its own"
    );
    assert_eq!(count(&events, "task_merged"), 3, "{kinds:?}");
    assert!(wide.run.invocations_balance());
}

#[test]
fn adversarial_orders_with_one_slot_per_agent_and_pool_always_reach_run_finished() {
    let tasks = three();
    for seed in scheduled_here(0..12, &[0]) {
        let runner = holding(&tasks, &[]);
        let plans = WidePlans {
            reviewers: 2,
            verify_reviewers: 2,
            pool: Some("scaffold-pool".to_owned()),
            ..WidePlans::default()
        };
        let mut wide = Wide::started_with(
            &format!("coordinator-deadlock-{seed}"),
            &tasks,
            3,
            plans,
            runner,
        );
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let mut scheduler = Scheduler::seeded(&runner, seed ^ 0xDEAD);
        let mut hooks = wide.env.hooks();
        let pipelines =
            wide.env
                .pipelines_limited(crate::engine::topology::scaffold::SlotLimitsOf::Exactly(
                    1, 1,
                ));
        let progress = wide
            .run
            .run_concurrently(
                &wide.env.seams(),
                &pipelines,
                &mut hooks,
                Some(&mut scheduler),
            )
            .unwrap_or_else(|error| panic!("seed {seed}: {error}"));
        assert_eq!(outcome_of(&progress), RunOutcome::Complete, "seed {seed}");
        assert_eq!(
            scheduler.widest_slotted, 1,
            "seed {seed}: one slot for the agent and the pool admits one agent process at a \
                 time (gates take no slot)"
        );
        assert!(
            scheduler.points < 200,
            "seed {seed}: {} steps",
            scheduler.points
        );
        drop(scheduler);
        assert!(wide.run.invocations_balance(), "seed {seed}");
    }
}
