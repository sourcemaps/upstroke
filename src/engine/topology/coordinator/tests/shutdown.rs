//! Extended notes: `docs/internals/engine/topology/coordinator/tests/shutdown.md`

use super::*;

#[test]
fn halt_interrupts_verification() {
    let mut wide = halting_on_gamma("coordinator-halt-in-verify");
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let run_id = wide.run.fold().started().expect("started").run_id.clone();
    let pin = crate::engine::topology::integrate::prepared_pin_ref(&run_id, SequenceId(1));
    let manager = wide.env.fixture.manager.clone();
    let mut while_verifying: Option<(Vec<String>, Option<String>)> = None;
    let mut scheduler = Scheduler::scripted(
        &runner,
        Box::new(|view: &Quiescent<'_>| {
            let merges = count(view.run.events(), "task_merged");
            let verifying = view.run.fold().transaction().is_some();
            if merges == 0 {
                return released(view, |invocation| attempt_key(invocation) == Some(0));
            }
            if !verifying {
                return released(view, |invocation| attempt_key(invocation) == Some(1));
            }
            if while_verifying.is_none() {
                let intents = manager.intents().expect("the intents are listable");
                while_verifying = Some((
                    snapshot_names(&intents),
                    manager
                        .direct_ref_target(pin.as_str())
                        .expect("the pin is readable"),
                ));
            }
            released(view, |invocation| attempt_key(invocation) == Some(2))
        }),
    );
    let (progress, watching) = drive_watching(&mut wide, &mut scheduler);
    drop(scheduler);
    let progress = progress.expect("the halted run closes and ends");
    assert_eq!(outcome_of(&progress), RunOutcome::Halted);
    let (snapshots, pinned) = while_verifying.expect("the verification was open");
    assert!(
        snapshots.contains(&"s1-integration".to_owned()) && pinned.is_some(),
        "while it was open the verification held its snapshot and its pin: {snapshots:?}"
    );

    let events = wide.env.durable_events();
    let kinds = kinds_of(&events);
    let halted = position(&kinds, "question_answered", 0);
    assert_eq!(
        &kinds[halted..],
        &[
            "question_answered",
            "merge_verification_interrupted",
            "run_finished"
        ],
        "the halt inside `verify` is settled by the closure: {kinds:?}"
    );
    assert!(
        events.iter().any(|event| matches!(&event.body,
            TopologyEventBody::MergeVerificationInterrupted { data }
                if data.sequence == SequenceId(1) && data.detail.contains("halted"))),
        "{kinds:?}"
    );
    for terminal in [
        "merge_prepared",
        "merge_rejected",
        "merge_verification_unavailable",
    ] {
        assert_eq!(
            count(&events, terminal),
            usize::from(terminal == "merge_prepared"),
            "halting never publishes unverified work; `{terminal}`: {kinds:?}"
        );
    }
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
    let at_interrupt = watching.intents_when("merge_verification_interrupted");
    assert!(
        snapshot_names(at_interrupt).contains(&"s1-integration".to_owned()),
        "the terminal precedes the reclaim: {at_interrupt:?}"
    );
    let cancelled_sequence = wide
        .env
        .runner
        .endings()
        .into_iter()
        .any(|(invocation, ending)| {
            matches!(invocation, InvocationId::Sequence { .. })
                && ending == crate::engine::topology::scaffold::Ending::Cancelled
        });
    assert!(
        cancelled_sequence,
        "the verification's process was terminated before the closure settled it"
    );
    assert!(wide.run.fold().transaction().is_none());
    assert!(wide.run.invocations_balance(), "{:?}", wide.run.warnings());
    assert_eq!(wide.run.broker_duplicates(), 0);
    replay_equals_live(&wide);
}

#[test]
fn stale_duplicate_and_mismatched_completions_are_discarded_with_a_warning_and_counted() {
    let tasks = three();
    let runner = holding(&tasks, &[]);
    let mut wide = Wide::started_with(
        "coordinator-injected",
        &tasks,
        3,
        WidePlans::default(),
        runner,
    );
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let mut injected = 0_u32;
    let mut stage = 0_u32;
    let mut settled: Option<(PipelineId, Identity)> = None;
    let mut seen: BTreeMap<PipelineId, Identity> = BTreeMap::new();
    let mut scheduler = Scheduler::scripted(
        &runner,
        Box::new(|view: &Quiescent<'_>| {
            for (pipeline, identity) in &view.live {
                seen.insert(*pipeline, identity.clone());
            }
            if stage == 0 {
                stage = 1;
                let (alpha, alpha_identity) = view
                    .live
                    .first()
                    .cloned()
                    .expect("the first pipeline is live");
                let Identity::Attempt {
                    key, generation, ..
                } = alpha_identity.clone()
                else {
                    panic!("the first pipeline is an attempt");
                };
                let superseded = Identity::Attempt {
                    key,
                    generation,
                    attempt: AttemptNumber(2),
                };
                let (beta, beta_identity) = view.live.get(1).cloned().expect("a second pipeline");
                let messages = vec![
                    ToCoordinator::Judged {
                        pipeline: alpha,
                        identity: Identity::Attempt {
                            key,
                            generation: GenerationId(1),
                            attempt: AttemptNumber(1),
                        },
                        outcome: Ok(forged()),
                    },
                    ToCoordinator::Judged {
                        pipeline: alpha,
                        identity: superseded,
                        outcome: Ok(forged()),
                    },
                    ToCoordinator::Judged {
                        pipeline: PipelineId(999),
                        identity: alpha_identity.clone(),
                        outcome: Ok(forged()),
                    },
                    ToCoordinator::Judged {
                        pipeline: beta,
                        identity: beta_identity,
                        outcome: Ok(forged()),
                    },
                    ToCoordinator::Ended {
                        pipeline: alpha,
                        invocation: AttemptIdentities::new(key, generation, AttemptNumber(1))
                            .worker(),
                        end: InvocationEnd::Completed,
                    },
                    ToCoordinator::Verified {
                        pipeline: alpha,
                        identity: Identity::Verification {
                            sequence: SequenceId(0),
                            candidate: CandidateRef {
                                key,
                                generation,
                                commit_sha: crate::topology::events::CommitSha("0".repeat(40)),
                                candidate_ref: crate::topology::events::GitRef(
                                    "refs/forged".to_owned(),
                                ),
                            },
                        },
                        outcome: Box::new(Ok(Judgement {
                            gates: Vec::new(),
                            reviews: Vec::new(),
                            failure: None,
                        })),
                        charged: Vec::new(),
                    },
                ];
                for message in messages {
                    assert!(view.injector.inject(message));
                    injected += 1;
                }
                return Some(Release::Injected);
            }
            if stage == 1 {
                let first = seen
                    .first_key_value()
                    .map(|(pipeline, identity)| (*pipeline, identity.clone()));
                if let Some((pipeline, identity)) = first {
                    let gone = !view.live.iter().any(|(live, _)| *live == pipeline);
                    if gone {
                        settled = Some((pipeline, identity.clone()));
                        stage = 2;
                        let Identity::Attempt {
                            key,
                            generation,
                            attempt,
                        } = identity.clone()
                        else {
                            panic!("an attempt");
                        };
                        assert!(view.injector.inject(ToCoordinator::Judged {
                            pipeline,
                            identity,
                            outcome: Ok(forged()),
                        }));
                        assert!(view.injector.inject(ToCoordinator::Ended {
                            pipeline,
                            invocation: AttemptIdentities::new(key, generation, attempt).worker(),
                            end: InvocationEnd::Completed,
                        }));
                        injected += 2;
                        return Some(Release::Injected);
                    }
                }
            }
            if stage == 2 {
                let beta = view.live.iter().find(|(_, identity)| {
                    matches!(identity, Identity::Attempt { key, .. } if *key == TaskKey(1))
                });
                if let Some((pipeline, _)) = beta {
                    let past_its_worker = view.invoking.iter().any(|invocation| {
                        attempt_key(invocation) == Some(1)
                            && role_of(invocation) != Some(AttemptRole::Worker)
                    });
                    if past_its_worker {
                        stage = 3;
                        assert!(
                            view.injector.inject(ToCoordinator::Ended {
                                pipeline: *pipeline,
                                invocation: AttemptIdentities::new(
                                    TaskKey(1),
                                    GenerationId(0),
                                    AttemptNumber(1),
                                )
                                .worker(),
                                end: InvocationEnd::Completed,
                            })
                        );
                        return Some(Release::Injected);
                    }
                    return released(view, |invocation| attempt_key(invocation) == Some(1));
                }
            }
            None
        }),
    );
    let progress = drive(&mut wide, Some(&mut scheduler)).expect("the run completes");
    drop(scheduler);
    assert_eq!(outcome_of(&progress), RunOutcome::Complete);
    assert!(
        settled.is_some(),
        "the duplicate was delivered after its pipeline settled"
    );
    assert_eq!(stage, 3, "the duplicate `Ended` was delivered");
    assert_eq!(
        wide.run.broker_duplicates(),
        1,
        "a second end of an invocation that had ended reached the ledger, which ignored and \
             counted it"
    );
    assert_eq!(
        wide.run.discarded(),
        injected,
        "every injected completion and operation was discarded and counted: {:?}",
        wide.run.warnings()
    );
    assert!(
        wide.run
            .warnings()
            .iter()
            .any(|warning| warning.contains("not live (stale or duplicate)")),
        "{:?}",
        wide.run.warnings()
    );
    let refused_by = |rule: &str| {
        wide.run
            .warnings()
            .iter()
            .filter(|warning| warning.starts_with("a completion for") && warning.contains(rule))
            .count()
    };
    assert_eq!(
        (refused_by(BINDING), refused_by("was injected for pipeline")),
        (2, 1),
        "alpha's two completions for another generation and another attempt were refused by \
             the binding to its pipeline's identity, and only beta's own, which nothing else \
             refuses, by the injector: an injected completion meets every check a pipeline's does \
             before it is refused as injected: {:?}",
        wide.run.warnings()
    );
    let events = wide.env.durable_events();
    assert_eq!(
        count(&events, "candidate_prepared"),
        3,
        "one settlement per attempt, whatever was re-delivered"
    );
    assert_eq!(count(&events, "task_merged"), 3);
    assert!(wide.run.invocations_balance());
    replay_equals_live(&wide);
}

#[derive(Debug, PartialEq, Eq)]
struct Received {
    interrupted: Option<String>,
    live: Vec<(PipelineId, bool)>,
    discarded: u32,
    appended: usize,
}

fn completion_on_the_pipeline_channel(
    tag: &str,
    closing: Option<TopologyEventBody>,
    from: PipelineId,
    identity: Identity,
) -> (Wide, Received, Vec<String>) {
    let tasks = three();
    let mut wide = Wide::started_with(tag, &tasks, 3, WidePlans::default(), holding(&tasks, &[]));
    let mut hooks = wide.env.hooks();
    let seams = wide.env.seams();
    let mut live = BTreeMap::new();
    for (pipeline, key) in [(PipelineId(1), 0), (PipelineId(2), 1)] {
        let job = crate::engine::topology::run::begin_dispatch(
            &mut crate::engine::topology::run::Stepping {
                run: &mut wide.run,

                seams: &seams,

                hooks: &mut hooks,
            },
            seams.manager,
            TaskKey(key),
            GenerationId(0),
            false,
        )
        .expect("alpha's and beta's attempts start");
        live.insert(
            pipeline,
            Live {
                identity: attempt_identity(key),
                cancel: Cancellation::new(),
                cancelled: false,
                busy: Busy::Running,
                running: None,
                job: Some(job),
            },
        );
    }
    let closes = closing.is_some();
    if let Some(event) = closing {
        wide.run
            .emit(event, &seams, &mut hooks)
            .expect("a fold-valid settlement of beta's attempt");
    }
    let pipelines = wide.env.pipelines();
    let (outbox, inbox) = mpsc::unbounded_channel();
    let (injector, injected) = mpsc::unbounded_channel();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .build()
        .expect("a runtime");
    let warned = wide.run.warnings().len();
    let received = {
        let mut coordinator = Coordinator {
            run: &mut wide.run,
            seams: &seams,
            hooks: &mut hooks,
            pipelines: &pipelines,
            observer: None,
            leases: Vec::new(),
            live,
            replies: BTreeMap::new(),
            gate: SnapshotGate::default(),
            next: 2,
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
            attempt_identity(0).open_in(coordinator.run)
                && attempt_identity(1).open_in(coordinator.run) != closes,
            "alpha's attempt is open, and beta's is open unless the test settled it"
        );
        let appended = coordinator.run.events().len();
        let discarded = coordinator.run.discarded();
        assert!(
            coordinator.client(from, true).send(ToCoordinator::Judged {
                pipeline: from,
                identity,
                outcome: Ok(forged()),
            }),
            "the coordinator's inbox is open"
        );
        coordinator.receive_one();
        Received {
            interrupted: coordinator
                .interrupt
                .as_ref()
                .map(|interrupt| match interrupt {
                    Interrupt::Failed(error) => error.to_string(),
                    other => other.describe().to_owned(),
                }),
            live: coordinator
                .live
                .iter()
                .map(|(pipeline, live)| (*pipeline, live.cancelled))
                .collect(),
            discarded: coordinator.run.discarded() - discarded,
            appended: coordinator.run.events().len() - appended,
        }
    };
    let warnings = wide.run.warnings()[warned..].to_vec();
    (wide, received, warnings)
}

#[test]
fn a_pipelines_completion_naming_another_attempt_of_its_task_is_discarded_and_its_pipeline_kept() {
    let (_wide, received, warnings) = completion_on_the_pipeline_channel(
        "coordinator-channel-wrong-attempt",
        None,
        PipelineId(1),
        Identity::Attempt {
            key: TaskKey(0),
            generation: GenerationId(0),
            attempt: AttemptNumber(2),
        },
    );
    assert_eq!(
        received,
        Received {
            interrupted: None,
            live: vec![(PipelineId(1), false), (PipelineId(2), false)],
            discarded: 1,
            appended: 0,
        },
        "alpha's pipeline sent a completion for attempt 2 of its own task on the channel every \
             pipeline sends on: it is discarded and counted, nothing is settled, and the pipeline \
             stays live to send its own; warnings {warnings:?}"
    );
    assert!(
        warnings
            .iter()
            .any(|warning| warning.contains("came from pipeline 1, ") && warning.contains(BINDING)),
        "the binding of a completion to its pipeline's identity refused it: {warnings:?}"
    );
}

#[test]
fn a_pipelines_completion_carrying_another_live_pipelines_identity_is_discarded_and_settles_nothing()
 {
    let (wide, received, warnings) = completion_on_the_pipeline_channel(
        "coordinator-channel-crossed",
        None,
        PipelineId(1),
        attempt_identity(1),
    );
    assert_eq!(
        received,
        Received {
            interrupted: None,
            live: vec![(PipelineId(1), false), (PipelineId(2), false)],
            discarded: 1,
            appended: 0,
        },
        "alpha's pipeline sent a completion carrying beta's identity, which the fold holds \
             open: it is discarded and counted, neither attempt is settled, and both pipelines stay \
             live; warnings {warnings:?}"
    );
    assert!(
        warnings
            .iter()
            .any(|warning| warning.contains("came from pipeline 1, ") && warning.contains(BINDING)),
        "the binding refused it: {warnings:?}"
    );
    assert!(
        !kinds_of_log(&wide).contains(&"candidate_prepared"),
        "nothing was settled from the crossed completion"
    );
}

#[test]
fn a_pipelines_completion_for_an_identity_the_fold_closed_is_discarded_and_settles_nothing() {
    let (wide, received, warnings) = completion_on_the_pipeline_channel(
        "coordinator-channel-closed",
        Some(planted_settlement(
            TaskKey(1),
            crate::topology::events::SettlementTransition::Retry,
        )),
        PipelineId(2),
        attempt_identity(1),
    );
    assert!(!attempt_identity(1).open_in(&wide.run));
    assert_eq!(
        received,
        Received {
            interrupted: None,
            live: vec![(PipelineId(1), false)],
            discarded: 1,
            appended: 0,
        },
        "beta's pipeline sent its own completion after the fold had settled beta's attempt and \
             before any pass stopped the pipeline: the completion is discarded and counted, the \
             pipeline retired, and nothing is settled twice; warnings {warnings:?}"
    );
    assert!(
        warnings
            .iter()
            .any(|warning| warning.contains("names an identity the fold does not hold open")),
        "the fold's own check refused it: {warnings:?}"
    );
    assert_eq!(
        kinds_of_log(&wide)
            .iter()
            .filter(|kind| **kind == "attempt_finished")
            .count(),
        1,
        "beta's attempt was settled once, by the fold's own settlement"
    );
}

#[test]
fn a_shutdown_cancels_every_live_pipeline_and_ends_the_command_resumably() {
    let tasks = three();
    let runner = holding(&tasks, &[]);
    let mut wide = Wide::started_with(
        "coordinator-shutdown",
        &tasks,
        3,
        WidePlans::default(),
        runner,
    );
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let mut granted: Option<Vec<InvocationId>> = None;
    let mut scheduler = Scheduler::scripted(
        &runner,
        Box::new(|view: &Quiescent<'_>| {
            if granted.is_some() {
                return None;
            }
            assert_eq!(view.live.len(), 3, "three pipelines are in flight");
            granted = Some(view.invoking.clone());
            assert!(view.injector.inject(ToCoordinator::Shutdown));
            Some(Release::Injected)
        }),
    );
    let error = drive(&mut wide, Some(&mut scheduler)).expect_err("a shutdown ends the command");
    drop(scheduler);
    assert!(error.to_string().contains("shut down"), "{error}");
    let events = wide.env.durable_events();
    let kinds = kinds_of(&events);
    assert_eq!(
        kinds.last(),
        Some(&"attempt_started"),
        "nothing is settled or appended by a shutdown: {kinds:?}"
    );
    let mut granted = granted.expect("the shutdown was injected at a quiescent point");
    granted.sort();
    assert_eq!(
        granted.iter().filter_map(worker).collect::<Vec<_>>(),
        vec![TaskKey(0), TaskKey(1), TaskKey(2)],
        "at the shutdown every live pipeline's worker was granted, and nothing else was"
    );
    let mut endings = wide.env.runner.endings();
    endings.sort_by(|left, right| left.0.cmp(&right.0));
    assert_eq!(
        endings,
        granted
            .iter()
            .map(|invocation| (
                invocation.clone(),
                crate::engine::topology::scaffold::Ending::Cancelled
            ))
            .collect::<Vec<_>>(),
        "a shutdown \"cancels/releases granted and non-slotted invocations after \
             termination\": every granted worker's process had started and was terminated as a \
             cancellation"
    );
    let ledger = wide.run.broker_mut().invocations();
    assert_eq!(
        (
            ledger.registered(),
            ledger.cancelled(),
            ledger.completed(),
            ledger.duplicates()
        ),
        (3, 3, 0, 0),
        "and each was released once, as a cancellation, when its pipeline reported the end \
             of the process: a release before the termination would make that report a counted \
             duplicate"
    );
    assert_eq!(wide.run.discarded(), 3, "and every completion discarded");
    assert!(wide.run.invocations_balance(), "{:?}", wide.run.warnings());
    replay_equals_live(&wide);
}

#[test]
fn a_shutdown_releases_each_invocation_once_whether_pending_unstarted_running_or_finished() {
    let tasks = [
        WideTask::independent("alpha"),
        WideTask::independent("beta"),
        WideTask::independent("gamma"),
        WideTask::independent("delta"),
    ];
    let worker_of =
        |key: u32| AttemptIdentities::new(TaskKey(key), GenerationId(0), AttemptNumber(1)).worker();
    let (unstarted, running, finished, pending) =
        (worker_of(0), worker_of(1), worker_of(2), worker_of(3));
    let runner = holding(&tasks, &[]);
    runner.bar(unstarted.clone());
    let mut wide = Wide::started_with(
        "coordinator-shutdown-positions",
        &tasks,
        4,
        WidePlans::default(),
        runner,
    );
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let mut granted: Option<Vec<InvocationId>> = None;
    let mut scheduler = Scheduler::scripted(
        &runner,
        Box::new(|view: &Quiescent<'_>| {
            if granted.is_some() {
                return None;
            }
            granted = Some(view.invoking.clone());
            runner
                .release(&finished, BOUND)
                .unwrap_or_else(|error| panic!("finishing `{finished}`: {error}"));
            assert!(view.injector.inject(ToCoordinator::Shutdown));
            Some(Release::Injected)
        }),
    );
    let mut hooks = wide.env.hooks();
    let pipelines =
        wide.env
            .pipelines_limited(crate::engine::topology::scaffold::SlotLimitsOf::Exactly(
                3, 3,
            ));
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
    assert!(error.to_string().contains("shut down"), "{error}");
    let mut granted = granted.expect("the shutdown was injected at a quiescent point");
    granted.sort();
    assert_eq!(
        granted,
        vec![unstarted.clone(), running.clone(), finished.clone()],
        "three agent slots: three workers granted and the fourth pending at the shutdown"
    );

    let mut endings = wide.env.runner.endings();
    endings.sort_by(|left, right| left.0.cmp(&right.0));
    assert_eq!(
        endings,
        vec![
            (
                unstarted.clone(),
                crate::engine::topology::scaffold::Ending::CancelledBeforeStart
            ),
            (
                running.clone(),
                crate::engine::topology::scaffold::Ending::Cancelled
            ),
            (
                finished.clone(),
                crate::engine::topology::scaffold::Ending::Completed
            ),
        ],
        "the granted worker whose process had not started was cancelled before it started, \
             the running one was terminated, the one that had finished completed, and the pending \
             one was never handed to the runner"
    );
    let started: Vec<InvocationId> = wide
        .env
        .runner
        .ran()
        .into_iter()
        .map(|ran| ran.invocation)
        .collect();
    assert_eq!(
        started,
        vec![running, finished],
        "no process of the unstarted or the pending worker ever started"
    );
    let ledger = wide.run.broker_mut().invocations();
    assert!(
        ledger.settled(&pending) && ledger.settled(&unstarted),
        "the pending request was withdrawn and the unstarted grant released"
    );
    assert_eq!(
        (
            ledger.registered(),
            ledger.completed(),
            ledger.cancelled(),
            ledger.duplicates()
        ),
        (4, 1, 3, 0),
        "each registration was released exactly once: the finished one as a completion, the \
             pending, unstarted and running ones as cancellations"
    );
    assert!(ledger.balances(), "no registration or slot is left held");
    assert_eq!(wide.run.discarded(), 4, "every completion discarded");
    let kinds = kinds_of(&wide.env.durable_events());
    assert_eq!(
        kinds.last(),
        Some(&"attempt_started"),
        "nothing is settled or appended by a shutdown: {kinds:?}"
    );
    replay_equals_live(&wide);
}

#[test]
fn a_shutdown_with_pipelines_in_flight_is_settled_by_the_next_resume_whose_ledgers_start_empty() {
    let tasks = three();
    let mut wide = Wide::durable(
        "coordinator-shutdown-resume",
        &tasks,
        3,
        WidePlans::default(),
        holding(&tasks, &[]),
    );
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let mut scheduler = shutdown_at_first_point(&runner);
    let error = drive(&mut wide, Some(&mut scheduler)).expect_err("a shutdown ends the command");
    drop(scheduler);
    assert!(error.to_string().contains("shut down"), "{error}");
    assert!(
        error
            .to_string()
            .contains("every pending request was withdrawn"),
        "{error}"
    );
    let kinds = kinds_of_log(&wide);
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| **kind == "attempt_started")
            .count(),
        3,
        "{kinds:?}"
    );
    assert_eq!(
        kinds.last(),
        Some(&"attempt_started"),
        "a shutdown appends nothing"
    );
    assert_eq!(
        wide.env
            .runner
            .endings()
            .iter()
            .filter(|(_, ending)| *ending == crate::engine::topology::scaffold::Ending::Cancelled)
            .count(),
        3,
        "every granted process was terminated"
    );
    let ledger = wide.run.broker_mut().invocations();
    assert_eq!(
        (
            ledger.registered(),
            ledger.cancelled(),
            ledger.completed(),
            ledger.duplicates()
        ),
        (3, 3, 0, 0),
        "each was released once, after its termination"
    );
    assert!(ledger.balances());
    assert_eq!(
        wide.run.entitlements_held(),
        0,
        "no provisional reservation outlives it"
    );
    assert_eq!(wide.run.discarded(), 3, "every completion discarded");

    let (recovered, mut resumed) = wide
        .resume(
            "inc-2",
            holding(&tasks, &[]),
            crate::engine::topology::select::Ceiling::unlimited(),
        )
        .expect("the next process resumes the shut-down run");
    assert_eq!(
        recovered.interrupted, 3,
        "the resume settles every in-flight identity interrupted"
    );
    assert!(
        resumed.run.invocations_balance()
            && resumed.run.entitlements_held() == 0
            && resumed.run.reservations_peak() == 0
            && resumed.run.broker_duplicates() == 0,
        "the coordinator's ledgers are empty at restart"
    );
    let kinds = kinds_of_log(&resumed);
    let resumed_at = position(&kinds, "run_resumed", 0);
    assert_eq!(
        kinds[resumed_at - 3..resumed_at],
        [
            "attempt_interrupted",
            "attempt_interrupted",
            "attempt_interrupted"
        ],
        "{kinds:?}"
    );
    let runner = std::sync::Arc::clone(&resumed.env.runner);
    let mut scheduler = Scheduler::first(&runner);
    let progress = drive(&mut resumed, Some(&mut scheduler)).expect("the resumed run completes");
    drop(scheduler);
    assert_eq!(outcome_of(&progress), RunOutcome::Complete);
    assert_eq!(count(&resumed.env.durable_events(), "task_merged"), 3);
    assert!(resumed.run.invocations_balance());
    replay_equals_live(&resumed);
}

struct StoppedInItsWait {
    at: TearAt,
    torn: Torn,
    finish_at_shutdown: bool,
}

fn stopped_in_its_wait(
    tag: &str,
    stop: StoppedInItsWait,
    build: impl FnOnce(&str) -> Wide + Send + 'static,
    order: impl FnOnce(&TearHeld) -> Script<'static> + Send + 'static,
    check: impl FnOnce(&Wide, usize) + Send + 'static,
) {
    let (what, tag) = (tag.to_owned(), tag.to_owned());
    bounded(&what, move || {
        let mut wide = build(&tag);
        let common = wide.env.fixture.manager.common_git_dir().to_path_buf();
        let contended = crate::workspace_manager::contended_attempts(&common);
        let mut hooks = TearHeld::waking(&wide, "foreign-held", stop.at, Wakes::Never);
        hooks.tearing(stop.torn);
        let injected = std::rc::Rc::new(std::cell::Cell::new(None));
        let script = a_shutdown_injected_once_the_tear_stands(
            hooks.planted(),
            std::rc::Rc::clone(&injected),
            stop.finish_at_shutdown.then(|| hooks.plant.clone()),
            order(&hooks),
        );
        let pipelines = wide.env.pipelines();
        let runner = Arc::clone(&wide.env.runner);
        let mut scheduler = Scheduler::scripted(&runner, script);
        let slept = crate::workspace_manager::fixture::slept_pauses();
        let error = wide
            .run
            .run_concurrently(
                &wide.env.seams(),
                &pipelines,
                &mut hooks,
                Some(&mut scheduler),
            )
            .expect_err("a shutdown answered inside the access's wait ends the command");
        drop(scheduler);
        hooks.finish().expect("the tear was planted");
        drop(hooks);
        let Some(at_shutdown) = injected.get() else {
            panic!(
                "the shutdown was injected while the tear stood: it was not, and the command \
                     ended on {error}"
            );
        };
        let warnings = wide.run.warnings().join("\n");
        assert!(
            error.to_string().contains("shut"),
            "the command ends on the shutdown: {error}"
        );
        assert!(
            crate::workspace_manager::contended_attempts(&common) > contended,
            "the access failed on the tear first, so the shutdown came inside its wait"
        );
        assert_eq!(
            crate::workspace_manager::fixture::slept_pauses(),
            slept,
            "no wait of a registry access the coordinator made slept on its thread"
        );
        let events = wide.run.events();
        assert_eq!(
            events.len(),
            at_shutdown,
            "nothing was appended after the wait answered the shutdown: {:?}",
            kinds_of(events.get(at_shutdown..).unwrap_or_default())
        );
        published_as_the_log_authorizes(&wide);
        assert!(wide.run.invocations_balance(), "{warnings}");
        assert!(
            wide.run.stopped().is_none(),
            "the run reopens when its coordinator is gone"
        );
        check(&wide, at_shutdown);
    });
}

fn two_held(tag: &str) -> Wide {
    let tasks = two_independent();
    Wide::started_with(tag, &tasks, 2, WidePlans::default(), holding(&tasks, &[]))
}

fn two_held_gated(failing: &'static [(u32, u32)]) -> impl FnOnce(&str) -> Wide + Send {
    move |tag| {
        let tasks = two_independent();
        let plans = WidePlans {
            gates: 2,
            ..WidePlans::default()
        };
        Wide::started_with(tag, &tasks, 2, plans, holding(&tasks, failing))
    }
}

fn first_invoking(view: &Quiescent<'_>) -> Option<Release> {
    let mut invoking = view.invoking.clone();
    invoking.sort();
    invoking.first().cloned().map(Release::Invocation)
}

fn first_released(_: &TearHeld) -> Script<'static> {
    Box::new(first_invoking)
}

fn beta_settles_first(_: &TearHeld) -> Script<'static> {
    Box::new(beta_settles_while_alpha_holds_its_first_gate)
}

fn workers_of(wide: &Wide, key: u32) -> usize {
    wide.env
        .runner
        .ran()
        .iter()
        .filter(|ran| worker(&ran.invocation) == Some(TaskKey(key)))
        .count()
}

#[test]
fn a_shutdown_answered_inside_a_dispatchs_head_check_dispatches_and_spawns_nothing() {
    stopped_in_its_wait(
        "shutdown-wait-head",
        StoppedInItsWait {
            at: TearAt::Fold(attempt_started_of(0)),
            torn: Torn::CommondirEmpty,
            finish_at_shutdown: true,
        },
        two_held,
        first_released,
        |wide, _| {
            let events = wide.run.events();
            assert_eq!(
                of_key(events, "task_dispatched", 1),
                0,
                "{:?}",
                kinds_of(events)
            );
            assert_eq!(
                workers_of(wide, 1),
                0,
                "no pipeline was spawned for the stopped dispatch"
            );
        },
    );
}

#[test]
fn a_shutdown_answered_inside_a_dispatchs_intent_starts_no_attempt_and_spawns_nothing() {
    stopped_in_its_wait(
        "shutdown-wait-intent",
        StoppedInItsWait {
            at: TearAt::Fold(task_dispatched_of(1)),
            torn: Torn::CommondirEmpty,
            finish_at_shutdown: true,
        },
        two_held,
        first_released,
        |wide, _| {
            let events = wide.run.events();
            assert_eq!(
                of_key(events, "task_dispatched", 1),
                1,
                "{:?}",
                kinds_of(events)
            );
            assert_eq!(
                of_key(events, "attempt_started", 1),
                0,
                "{:?}",
                kinds_of(events)
            );
            assert_eq!(
                workers_of(wide, 1),
                0,
                "no pipeline was spawned for the stopped dispatch"
            );
        },
    );
}

// Protocol (standards §10): the slow witness owns the worker that holds its
// access's first contended answer. The build step spawns the worker and
// sends it, with the sender that cancels its wait for that answer, over a
// channel to the check step, its one owner past the run. `finish` cancels
// what is left of that wait and joins the worker; a worker that panicked
// dropped its release unsent, which let the held access go on at once, and
// the witness fails on it rather than passing without the delay it is
// about. A scenario that unwinds first drops the channel, and the drop
// cancels and joins the worker in the same way, saying what it found.
struct DelayedRelease {
    cancel: std::sync::mpsc::Sender<()>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl DelayedRelease {
    fn finish(&mut self) -> Result<(), String> {
        let Some(worker) = self.worker.take() else {
            return Ok(());
        };
        let _ = self.cancel.send(());
        worker.join().map_err(|_| {
            "the worker that held the access's first contended answer panicked, so its \
                 release dropped unsent and the access went on without the delay"
                .to_owned()
        })
    }
}

impl Drop for DelayedRelease {
    fn drop(&mut self) {
        if let Err(why) = self.finish() {
            if std::thread::panicking() {
                crate::workspace_manager::fixture::say(&why);
            } else {
                panic!("{why}");
            }
        }
    }
}

#[test]
fn a_delayed_release_whose_worker_panicked_fails_its_witness() {
    let (cancel, _cancelled) = std::sync::mpsc::channel::<()>();
    let worker = std::thread::spawn(|| {
        panic!("an injected failure of a delayed release's worker, before its delay");
    });
    let mut delayed = DelayedRelease {
        cancel,
        worker: Some(worker),
    };
    let finished = delayed.finish();
    assert!(
        matches!(&finished, Err(why) if why.contains("panicked")
            && why.contains("without the delay")),
        "a worker that panicked is reported, not taken for its delay: {finished:?}"
    );
}

#[test]
fn a_shutdown_answered_inside_a_slow_dispatchs_intent_starts_no_attempt_and_spawns_nothing() {
    let (handing, handed) = std::sync::mpsc::channel::<DelayedRelease>();
    stopped_in_its_wait(
        "shutdown-wait-intent-slow",
        StoppedInItsWait {
            at: TearAt::Fold(task_dispatched_of(1)),
            torn: Torn::CommondirEmpty,
            finish_at_shutdown: true,
        },
        move |tag| {
            let wide = two_held(tag);
            let common = wide.env.fixture.manager.common_git_dir().to_path_buf();
            let before = crate::workspace_manager::contended_attempts(&common);
            let release = crate::workspace_manager::hold_next_contended(&common);
            let (cancel, cancelled) = std::sync::mpsc::channel::<()>();
            let worker = std::thread::spawn(move || {
                let deadline = std::time::Instant::now() + BOUND;
                while crate::workspace_manager::contended_attempts(&common) == before
                    && cancelled.try_recv() == Err(std::sync::mpsc::TryRecvError::Empty)
                    && std::time::Instant::now() < deadline
                {
                    std::thread::sleep(Duration::from_millis(1));
                }
                std::thread::sleep(Duration::from_millis(700));
                let _ = release.send(());
            });
            handing
                .send(DelayedRelease {
                    cancel,
                    worker: Some(worker),
                })
                .expect("the check step holds the worker's channel");
            wide
        },
        first_released,
        move |wide, _| {
            let mut delayed = handed
                .try_recv()
                .expect("the build step handed its worker over");
            if let Err(why) = delayed.finish() {
                panic!("{why}");
            }
            let events = wide.run.events();
            assert_eq!(
                of_key(events, "attempt_started", 1),
                0,
                "{:?}",
                kinds_of(events)
            );
            assert_eq!(
                workers_of(wide, 1),
                0,
                "no pipeline was spawned for the stopped dispatch"
            );
        },
    );
}

#[test]
fn a_shutdown_answered_inside_a_continued_dispatchs_wait_starts_no_attempt() {
    stopped_in_its_wait(
        "shutdown-wait-continued",
        StoppedInItsWait {
            at: TearAt::Access(attempt_started_of(0), 1),
            torn: Torn::CommondirEmpty,
            finish_at_shutdown: true,
        },
        |tag| {
            let tasks = [
                WideTask::independent("alpha"),
                WideTask::independent("beta"),
            ];
            let mut wide = Wide::durable(
                tag,
                &tasks,
                2,
                WidePlans::default(),
                RecordingRunner::new().answering(wide_responder(&tasks, &[])),
            );
            let beta_slot =
                wide.env
                    .fixture
                    .manager
                    .slot_path(&crate::engine::topology::dispatch::task_slot(
                        TaskKey(1),
                        GenerationId(0),
                    ));
            let blocked = beta_slot.clone();
            let mut breaking = TearHeld::new(&wide, "foreign-unused", TearAt::Fold(|_| false));
            breaking.also = Some((
                task_dispatched_of(1),
                Box::new(move || {
                    crate::workspace_manager::fixture::write_file(
                        &blocked,
                        b"a file where beta's checkout goes\n",
                    );
                }),
            ));
            let pipelines = wide.env.pipelines();
            let error = wide
                .run
                .run_concurrently(&wide.env.seams(), &pipelines, &mut breaking, None)
                .expect_err("beta's add meets a file at its destination after `task_dispatched`");
            drop(breaking);
            assert!(
                matches!(&error, UpstrokeError::Git { message } if message.contains("already exists")),
                "{error:?}"
            );
            crate::workspace_manager::fixture::remove_file(&beta_slot);
            let (_, resumed) = wide
                .resume(
                    "inc-2",
                    holding(&tasks, &[]),
                    crate::engine::topology::select::Ceiling::unlimited(),
                )
                .expect("the next process resumes with beta's generation open and no attempt");
            resumed
        },
        first_released,
        |wide, _| {
            let events = wide.run.events();
            assert_eq!(
                of_key(events, "attempt_started", 1),
                0,
                "the continuation started no attempt: {:?}",
                kinds_of(events)
            );
            assert_eq!(
                workers_of(wide, 1),
                0,
                "no pipeline was spawned for the continuation"
            );
        },
    );
}

#[test]
fn a_shutdown_answered_inside_a_retrys_wait_starts_no_retry() {
    stopped_in_its_wait(
        "shutdown-wait-retry",
        StoppedInItsWait {
            at: TearAt::Fold(retained_attempt_finished_of_beta),
            torn: Torn::CommondirEmpty,
            finish_at_shutdown: true,
        },
        two_held_gated(&[(0, 1)]),
        beta_settles_first,
        |wide, _| {
            let events = wide.run.events();
            assert_eq!(
                of_key(events, "attempt_started", 0),
                1,
                "beta's retry was not started: {:?}",
                kinds_of(events)
            );
        },
    );
}

#[test]
fn a_shutdown_answered_inside_a_settlements_wait_promotes_nothing() {
    stopped_in_its_wait(
        "shutdown-wait-settle",
        StoppedInItsWait {
            at: TearAt::Access(attempt_started_of(1), 1),
            torn: Torn::CommondirEmpty,
            finish_at_shutdown: true,
        },
        two_held_gated(&[]),
        beta_settles_first,
        |wide, _| {
            let events = wide.run.events();
            assert_eq!(
                of_key(events, "candidate_prepared", 0),
                0,
                "{:?}",
                kinds_of(events)
            );
            assert_eq!(
                of_key(events, "attempt_finished", 0),
                0,
                "{:?}",
                kinds_of(events)
            );
        },
    );
}

#[test]
fn a_shutdown_answered_inside_a_verifications_wait_starts_no_retry() {
    stopped_in_its_wait(
        "shutdown-wait-verify",
        StoppedInItsWait {
            at: TearAt::Access(retained_attempt_finished_of_beta, 2),
            torn: Torn::CommondirEmpty,
            finish_at_shutdown: true,
        },
        two_held_gated(&[(0, 1)]),
        |hooks| alpha_waits_for_the_tear(hooks.planted()),
        |wide, _| {
            let events = wide.run.events();
            assert_eq!(
                of_key(events, "attempt_started", 0),
                1,
                "the verified retry was not started: {:?}",
                kinds_of(events)
            );
        },
    );
}

#[test]
fn a_shutdown_answered_inside_a_repairs_wait_starts_no_repair_attempt() {
    stopped_in_its_wait(
        "shutdown-wait-repair",
        StoppedInItsWait {
            at: TearAt::Access(repair_dispatched, 4),
            torn: Torn::CommondirEmpty,
            finish_at_shutdown: true,
        },
        |tag| {
            let tasks = [
                WideTask::independent("beta"),
                WideTask::independent("alpha"),
                WideTask::hinted("gamma", &["src/gamma/"], "src/beta/work.txt"),
            ];
            Wide::started_with(tag, &tasks, 3, WidePlans::default(), holding(&tasks, &[]))
        },
        |_| Box::new(alpha_waits_for_gammas_integration),
        |wide, _| {
            let events = wide.run.events();
            let repairs: Vec<u32> = events
                .iter()
                .filter_map(|event| match &event.body {
                    TopologyEventBody::TaskDispatched { data }
                        if data.source_candidate.is_some() =>
                    {
                        Some(data.key.0)
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(repairs.len(), 1, "{:?}", kinds_of(events));
            for key in repairs {
                assert_eq!(
                    of_key(events, "attempt_started", key),
                    0,
                    "the repair started no attempt: {:?}",
                    kinds_of(events)
                );
                assert_eq!(workers_of(wide, key), 0);
            }
        },
    );
}

#[test]
fn a_shutdown_answered_inside_an_integrations_decision_prepares_and_publishes_nothing() {
    stopped_in_its_wait(
        "shutdown-wait-decide",
        StoppedInItsWait {
            at: TearAt::Access(candidate_created_of_beta, 3),
            torn: Torn::CommondirEmpty,
            finish_at_shutdown: true,
        },
        two_held_gated(&[]),
        beta_settles_first,
        |wide, _| {
            let events = wide.run.events();
            assert_eq!(count(events, "merge_prepared"), 0, "{:?}", kinds_of(events));
            assert_eq!(count(events, "task_merged"), 0, "{:?}", kinds_of(events));
        },
    );
}

#[test]
fn a_shutdown_answered_inside_a_publications_wait_publishes_nothing() {
    stopped_in_its_wait(
        "shutdown-wait-publish",
        StoppedInItsWait {
            at: TearAt::Fold(merge_prepared_of_beta),
            torn: Torn::CommondirEmpty,
            finish_at_shutdown: true,
        },
        two_held_gated(&[]),
        beta_settles_first,
        |wide, _| {
            let events = wide.run.events();
            assert_eq!(count(events, "merge_prepared"), 1, "{:?}", kinds_of(events));
            assert_eq!(count(events, "task_merged"), 0, "{:?}", kinds_of(events));
        },
    );
}

#[test]
fn a_shutdown_answered_inside_a_publications_swap_recheck_publishes_nothing() {
    stopped_in_its_wait(
        "shutdown-wait-swap-recheck",
        StoppedInItsWait {
            at: TearAt::Access(merge_prepared_of_beta, 2),
            torn: Torn::CommondirEmpty,
            finish_at_shutdown: true,
        },
        two_held_gated(&[]),
        beta_settles_first,
        |wide, _| {
            let events = wide.run.events();
            assert_eq!(count(events, "merge_prepared"), 1, "{:?}", kinds_of(events));
            assert_eq!(count(events, "task_merged"), 0, "{:?}", kinds_of(events));
        },
    );
}

#[test]
fn a_shutdown_answered_inside_a_stale_picks_wait_classifies_nothing_into_the_log() {
    stopped_in_its_wait(
        "shutdown-wait-pick",
        StoppedInItsWait {
            at: TearAt::Access(candidate_created_of_gamma, 7),
            torn: Torn::CommondirEmpty,
            finish_at_shutdown: true,
        },
        |tag| {
            let tasks = [
                WideTask::independent("beta"),
                WideTask::independent("alpha"),
                WideTask::independent("gamma"),
            ];
            Wide::started_with(tag, &tasks, 3, WidePlans::default(), holding(&tasks, &[]))
        },
        |_| Box::new(alpha_waits_for_gammas_integration),
        |wide, at_shutdown| {
            let events = wide.run.events();
            let after = events.get(at_shutdown..).unwrap_or_default();
            assert_eq!(count(after, "merge_verification_started"), 0);
            assert_eq!(count(after, "merge_rejected"), 0);
        },
    );
}

fn run_finished(body: &TopologyEventBody) -> bool {
    matches!(body, TopologyEventBody::RunFinished { .. })
}

fn attempt_interrupted(body: &TopologyEventBody) -> bool {
    matches!(body, TopologyEventBody::AttemptInterrupted { .. })
}

fn gamma_halts_while_alpha_gates(view: &Quiescent<'_>) -> Option<Release> {
    let alpha_gating = view.invoking.iter().any(|invocation| {
        attempt_key(invocation) == Some(0)
            && matches!(role_of(invocation), Some(AttemptRole::Gate(_)))
    });
    if alpha_gating {
        released(view, |invocation| worker(invocation) == Some(TaskKey(2)))
    } else {
        released(view, |invocation| worker(invocation) == Some(TaskKey(0)))
    }
}

#[test]
fn a_shutdown_answered_inside_a_closures_wait_closes_nothing_further() {
    stopped_in_its_wait(
        "shutdown-wait-closure",
        StoppedInItsWait {
            at: TearAt::Fold(attempt_interrupted),
            torn: Torn::GitdirMissing,
            finish_at_shutdown: true,
        },
        halting_on_gamma,
        |_| Box::new(gamma_halts_while_alpha_gates),
        |wide, _| {
            let events = wide.run.events();
            assert_eq!(
                count(events, "attempt_interrupted"),
                1,
                "{:?}",
                kinds_of(events)
            );
            assert_eq!(count(events, "run_finished"), 0, "{:?}", kinds_of(events));
        },
    );
}

#[test]
fn a_shutdown_answered_inside_a_finalizations_wait_finalizes_nothing_further() {
    stopped_in_its_wait(
        "shutdown-wait-finalize",
        StoppedInItsWait {
            at: TearAt::Fold(run_finished),
            torn: Torn::CommondirEmpty,
            finish_at_shutdown: true,
        },
        two_held,
        first_released,
        |wide, _| {
            let events = wide.run.events();
            assert_eq!(kinds_of(events).last(), Some(&"run_finished"));
            assert!(
                wide.env.fixture.manager.execution_root().exists(),
                "the finalization stopped before it removed the execution root"
            );
        },
    );
}

/// The follow-up C record's §6.11, with the proposed frozen hunk C-R1: a
/// shutdown answered inside a wait of the finalization scrub's registry
/// read ends the command there. Nothing is scrubbed and the execution root
/// stays, and the run is resumable.
#[test]
fn a_shutdown_answered_inside_a_finalizations_scrub_discovery_finalizes_nothing_further() {
    stopped_in_its_wait(
        "shutdown-wait-scrub",
        StoppedInItsWait {
            at: TearAt::Access(run_finished, 1),
            torn: Torn::GitdirUnreadable,
            finish_at_shutdown: true,
        },
        two_held,
        first_released,
        |wide, _| {
            let events = wide.run.events();
            assert_eq!(kinds_of(events).last(), Some(&"run_finished"));
            assert!(
                wide.env.fixture.manager.execution_root().exists(),
                "the finalization stopped before the execution root was removed"
            );
        },
    );
}

/// The follow-up C record's §6.11: a shutdown answered inside a wait of
/// the final sweep's registry read ends the command there. The sweep
/// removes nothing further, the execution root stays, and the run is
/// resumable, as B's contract has it for every wait on the coordinator.
#[test]
fn a_shutdown_answered_inside_a_final_sweeps_wait_removes_nothing_further() {
    stopped_in_its_wait(
        "shutdown-wait-sweep",
        StoppedInItsWait {
            at: TearAt::Access(run_finished, 6),
            torn: Torn::GitdirUnreadable,
            finish_at_shutdown: true,
        },
        two_held,
        first_released,
        |wide, _| {
            let events = wide.run.events();
            assert_eq!(kinds_of(events).last(), Some(&"run_finished"));
            assert!(
                wide.env.fixture.manager.execution_root().exists(),
                "the sweep stopped before the execution root was removed"
            );
        },
    );
}

fn plant_commondir_empty(wide: &Wide, name: &str) -> PathBuf {
    let admin = wide
        .env
        .fixture
        .manager
        .common_git_dir()
        .join("worktrees")
        .join(name);
    crate::workspace_manager::fixture::write_file(
        &admin.join("HEAD"),
        format!("{}\n", wide.env.fixture.head).as_bytes(),
    );
    crate::workspace_manager::fixture::write_file(
        &admin.join("gitdir"),
        format!(
            "{}\n",
            crate::workspace_manager::fixture::as_git_writes_it(&admin.join("checkout/.git"))
        )
        .as_bytes(),
    );
    crate::workspace_manager::fixture::write_file(&admin.join("commondir"), b"");
    admin
}

#[test]
fn a_shutdown_answered_during_a_registry_pause_dispatches_nothing_and_appends_nothing() {
    bounded("the review's dispatch witness", || {
        let tasks = [
            WideTask::independent("alpha"),
            WideTask::independent("beta"),
        ];
        let mut wide = Wide::started_with(
            "shutdown-in-pause-dispatch",
            &tasks,
            2,
            WidePlans::default(),
            holding(&tasks, &[]),
        );
        let _deadline = crate::workspace_manager::RegistryDeadline::hold(
            wide.env.fixture.manager.common_git_dir(),
            WITNESS_REGISTRY_DEADLINE,
        );
        let mut hooks = wide.env.hooks();
        let seams = wide.env.seams();
        let alpha = crate::engine::topology::run::begin_dispatch(
            &mut crate::engine::topology::run::Stepping {
                run: &mut wide.run,
                seams: &seams,
                hooks: &mut hooks,
            },
            seams.manager,
            TaskKey(0),
            GenerationId(0),
            false,
        )
        .expect("alpha starts first");
        let cancel = Cancellation::new();
        let watched = cancel.clone();
        let live = BTreeMap::from([(
            PipelineId(1),
            Live {
                identity: attempt_identity(0),
                cancel,
                cancelled: false,
                busy: Busy::Running,
                running: None,
                job: Some(alpha),
            },
        )]);
        let pipelines = wide.env.pipelines();
        let admin = plant_commondir_empty(&wide, "shutdown-held");
        let repair = std::thread::spawn(move || {
            let until = std::time::Instant::now() + Duration::from_secs(5);
            while !watched.is_cancelled() && std::time::Instant::now() < until {
                std::thread::sleep(Duration::from_millis(1));
            }
            let seen = watched.is_cancelled();
            crate::workspace_manager::fixture::write_file(&admin.join("commondir"), b"../..\n");
            seen
        });
        let before = wide.run.events().len();
        let (was_shutdown, result) = {
            let mut coordinator =
                coordinator_of(&mut wide.run, &seams, &mut hooks, &pipelines, live);
            coordinator
                .outbox
                .send(ToCoordinator::Shutdown)
                .expect("shutdown reaches inbox");
            let result = crate::engine::topology::run::begin_dispatch(
                &mut coordinator,
                seams.manager,
                TaskKey(1),
                GenerationId(0),
                false,
            );
            (
                matches!(coordinator.interrupt, Some(Interrupt::Shutdown)),
                result,
            )
        };
        assert!(
            repair.join().expect("the owned repair thread finishes"),
            "the coordinator processed the shutdown inside the registry pause"
        );
        let appended = kinds_of(&wide.run.events()[before..]);
        assert!(was_shutdown, "the shutdown was handled mid-pause");
        assert!(
            result.is_err() && appended.is_empty(),
            "a handled shutdown must not dispatch beta or append: {appended:?}"
        );
    });
}

#[test]
fn a_shutdown_answered_during_an_admitted_dispatchs_pause_spawns_no_pipeline() {
    bounded("the admission's dispatch", || {
        let tasks = [
            WideTask::independent("alpha"),
            WideTask::independent("beta"),
        ];
        let mut wide = Wide::started_with(
            "shutdown-in-pause-admission",
            &tasks,
            2,
            WidePlans::default(),
            holding(&tasks, &[]),
        );
        let _deadline = crate::workspace_manager::RegistryDeadline::hold(
            wide.env.fixture.manager.common_git_dir(),
            WITNESS_REGISTRY_DEADLINE,
        );
        let mut hooks = wide.env.hooks();
        let seams = wide.env.seams();
        let pipelines = wide.env.pipelines();
        plant_commondir_empty(&wide, "admission-held");
        let before = wide.run.events().len();
        let (shutdown, spawned, admitted) = {
            let mut coordinator = coordinator_of(
                &mut wide.run,
                &seams,
                &mut hooks,
                &pipelines,
                BTreeMap::new(),
            );
            coordinator
                .outbox
                .send(ToCoordinator::Shutdown)
                .expect("shutdown reaches inbox");
            let admitted = coordinator.admit();
            (
                matches!(coordinator.interrupt, Some(Interrupt::Shutdown)),
                (coordinator.live.len(), coordinator.handles.len()),
                admitted,
            )
        };
        assert!(shutdown, "the admission's wait answered the shutdown");
        assert_eq!(
            spawned,
            (0, 0),
            "no pipeline was spawned after the shutdown"
        );
        let error = admitted.expect_err("the stopped dispatch ends the admission pass");
        assert!(error.to_string().contains("met a shutdown"), "{error}");
        assert!(
            wide.run.events()[before..].is_empty(),
            "{:?}",
            kinds_of(&wide.run.events()[before..])
        );
        assert!(
            wide.env.runner.ran().is_empty(),
            "no worker reached the runner"
        );
    });
}

#[test]
fn a_shutdown_consumed_during_a_registry_wait_publishes_no_candidate() {
    bounded("the review's integration witness", || {
        let tasks = two_independent();
        let mut wide = Wide::started_with(
            "shutdown-in-wait-integration",
            &tasks,
            2,
            WidePlans::default(),
            RecordingRunner::new().answering(wide_responder(&tasks, &[])),
        );
        let _deadline = crate::workspace_manager::RegistryDeadline::hold(
            wide.env.fixture.manager.common_git_dir(),
            WITNESS_REGISTRY_DEADLINE,
        );
        let mut hooks = wide.env.hooks();
        let seams = wide.env.seams();
        let progress = wide
            .run
            .step(&seams, &mut hooks)
            .expect("prepare one candidate");
        assert!(
            matches!(progress, Progress::Settled { accepted: true, .. }),
            "{progress:?}"
        );
        let Admitted::Integrate { candidate } = wide.run.admitted().expect("select a candidate")
        else {
            panic!("expected an integration");
        };
        let before = wide.run.events().len();
        let plant = Plant {
            admin: seams
                .manager
                .common_git_dir()
                .join("worktrees")
                .join("review-shutdown-tear"),
            head: wide.env.fixture.head.clone(),
            runner: Arc::clone(&wide.env.runner),
            wakes: Wakes::Contended,
            common: seams.manager.common_git_dir().to_path_buf(),
            torn: Torn::CommondirEmpty,
        };
        let prober = plant.tear();
        let pipelines = wide.env.pipelines();
        let (result, shutdown) = {
            let mut coordinator = coordinator_of(
                &mut wide.run,
                &seams,
                &mut hooks,
                &pipelines,
                BTreeMap::new(),
            );
            coordinator
                .outbox
                .send(ToCoordinator::Shutdown)
                .expect("queue the shutdown before the registry wait");
            let result = coordinator.integrate(*candidate);
            (
                result,
                matches!(coordinator.interrupt, Some(Interrupt::Shutdown)),
            )
        };
        let _ = prober.cancel.send(());
        prober
            .handle
            .join()
            .expect("the prober joined")
            .expect("the access reached contention");
        assert!(shutdown, "the shutdown was consumed in the wait");
        let appended = &wide.run.events()[before..];
        assert!(
            appended.is_empty(),
            "result={result:?}; events appended after the wait consumed Shutdown: {:?}",
            kinds_of(appended)
        );
        assert!(
            matches!(result, Ok(false)),
            "the interrupted integration ends the admission pass: {result:?}"
        );
        published_as_the_log_authorizes(&wide);
    });
}

#[test]
fn a_dispatch_begun_after_a_wait_answered_a_shutdown_appends_nothing() {
    bounded("a dispatch after a stopped wait", || {
        let tasks = two_independent();
        let mut wide = Wide::started_with(
            "stopped-then-dispatch",
            &tasks,
            2,
            WidePlans::default(),
            holding(&tasks, &[]),
        );
        let mut hooks = wide.env.hooks();
        let seams = wide.env.seams();
        let pipelines = wide.env.pipelines();
        let before = wide.run.events().len();
        let (stopped, dispatched) = {
            let mut coordinator = coordinator_of(
                &mut wide.run,
                &seams,
                &mut hooks,
                &pipelines,
                BTreeMap::new(),
            );
            coordinator
                .outbox
                .send(ToCoordinator::Shutdown)
                .expect("shutdown reaches inbox");
            let stopped = crate::workspace_manager::EffectHooks::registry_pause(
                &mut coordinator,
                Duration::from_millis(5),
            );
            let dispatched = crate::engine::topology::run::begin_dispatch(
                &mut coordinator,
                seams.manager,
                TaskKey(0),
                GenerationId(0),
                false,
            );
            (stopped, dispatched)
        };
        let stopped = stopped.expect_err("a wait that answered a shutdown ends in an error");
        assert!(stopped.to_string().contains("met a shutdown"), "{stopped}");
        let refused = dispatched.expect_err("the stopped run appends nothing for a dispatch");
        assert!(
            refused
                .to_string()
                .contains("nothing further is appended in this process"),
            "the run's emitter refused the dispatch's first append: {refused}"
        );
        assert!(
            wide.run.events()[before..].is_empty(),
            "{:?}",
            kinds_of(&wide.run.events()[before..])
        );
        assert!(wide.run.stopped().is_some());
        wide.run.reopen();
    });
}

#[test]
fn a_publication_begun_after_a_wait_answered_a_shutdown_moves_no_ref() {
    bounded("a publication after a stopped wait", || {
        let tasks = two_independent();
        let mut wide = Wide::started_with(
            "stopped-then-publish",
            &tasks,
            2,
            WidePlans::default(),
            RecordingRunner::new().answering(wide_responder(&tasks, &[])),
        );
        let mut hooks = wide.env.hooks();
        let seams = wide.env.seams();
        wide.run
            .step(&seams, &mut hooks)
            .expect("prepare one candidate");
        let Admitted::Integrate { candidate } = wide.run.admitted().expect("select a candidate")
        else {
            panic!("expected an integration");
        };
        let request = wide
            .run
            .integration_request(&candidate)
            .expect("an integration request");
        let authorized = crate::engine::topology::integrate::Authorized {
            sequence: request.sequence,
            key: candidate.key,
            expected_head: request.authorized.head.clone(),
            proposed_sha: candidate.commit_sha.clone(),
            satisfies: request.satisfies.clone(),
            lease_release: request.lease_release.clone(),
            integration_ref: request.integration_ref.clone(),
            pin: None,
            staging: None,
        };
        let before = wide.run.events().len();
        let pipelines = wide.env.pipelines();
        let published = {
            let mut coordinator = coordinator_of(
                &mut wide.run,
                &seams,
                &mut hooks,
                &pipelines,
                BTreeMap::new(),
            );
            coordinator
                .outbox
                .send(ToCoordinator::Shutdown)
                .expect("shutdown reaches inbox");
            let _ = crate::workspace_manager::EffectHooks::registry_pause(
                &mut coordinator,
                Duration::from_millis(5),
            );
            crate::engine::topology::integrate::publish(
                &mut DrivenJournal(&mut coordinator),
                seams.manager,
                authorized,
            )
        };
        let refused = published.expect_err("the stopped run publishes nothing");
        assert!(
            refused
                .to_string()
                .contains("nothing further is done in this process"),
            "the compare-and-swap was refused at its `before` hook: {refused}"
        );
        assert!(wide.run.events()[before..].is_empty());
        published_as_the_log_authorizes(&wide);
        wide.run.reopen();
    });
}

struct CountsRawPauses {
    inner: crate::engine::topology::seams::HarnessTopologyHooks,
    ledger: crate::util::DurabilityLedger,
    poisoned_cause: Option<String>,
    raw: usize,
}

impl CountsRawPauses {
    fn over(wide: &Wide) -> Self {
        let mut inner = wide.env.hooks();
        let ledger = inner.effects().durability_ledger();
        Self {
            inner,
            ledger,
            poisoned_cause: None,
            raw: 0,
        }
    }
}

impl crate::workspace_manager::EffectHooks for CountsRawPauses {
    fn phase(
        &mut self,
        site: crate::topology::effects::EffectSiteId,
        phase: crate::topology::effects::HookPhase,
    ) -> crate::topology::effects::Injection {
        let effects = self.inner.effects();
        let answer = effects.phase(site, phase);
        self.poisoned_cause = effects.refusal_cause();
        answer
    }

    fn refusal_cause(&self) -> Option<String> {
        self.poisoned_cause.clone()
    }

    fn durability_ledger(&self) -> crate::util::DurabilityLedger {
        self.ledger.clone()
    }

    fn registry_pause(&mut self, pause: Duration) -> Result<(), UpstrokeError> {
        self.raw += 1;
        self.inner.effects().registry_pause(pause)
    }
}

struct RawPausesUnder {
    counting: CountsRawPauses,
    tear: TearHeld,
}

impl TopologyHooks for RawPausesUnder {
    fn effects(&mut self) -> &mut dyn crate::workspace_manager::EffectHooks {
        &mut self.counting
    }

    fn rundir(&mut self) -> &mut dyn crate::rundir::RunDirHooks {
        self.tear.rundir()
    }

    fn events(&mut self) -> &mut dyn crate::events::log::EventHooks {
        self.tear.events()
    }

    fn container(&mut self) -> &mut dyn crate::runner::container::ContainerHooks {
        self.tear.container()
    }

    fn spawn(&mut self) -> &mut dyn crate::agent::proc::SpawnHooks {
        TopologyHooks::spawn(&mut self.tear)
    }

    fn folded(&mut self, fold: &TopologyFold, events: &[TopologyEvent]) {
        self.tear.folded(fold, events);
    }
}

fn waits_through_the_coordinator(
    what: &'static str,
    build: impl FnOnce() -> Wide + Send + 'static,
    at: TearAt,
    torn: Torn,
    order: fn(&Quiescent<'_>) -> Option<Release>,
    outcome: RunOutcome,
) {
    bounded(what, move || {
        let mut wide = build();
        let common = wide.env.fixture.manager.common_git_dir().to_path_buf();
        let contended = crate::workspace_manager::contended_attempts(&common);
        let mut tear = TearHeld::waking(&wide, "foreign-held", at, Wakes::Contended);
        tear.tearing(torn);
        let mut hooks = RawPausesUnder {
            counting: CountsRawPauses::over(&wide),
            tear,
        };
        let pipelines = wide.env.pipelines();
        let runner = Arc::clone(&wide.env.runner);
        let mut scheduler = Scheduler::scripted(&runner, Box::new(order));
        let slept = crate::workspace_manager::fixture::slept_pauses();
        let progress = wide
            .run
            .run_concurrently(
                &wide.env.seams(),
                &pipelines,
                &mut hooks,
                Some(&mut scheduler),
            )
            .expect("the access is passed once its tear is finished");
        drop(scheduler);
        hooks
            .tear
            .finish()
            .expect("the access failed on the tear, and only then was it finished");
        assert_eq!(outcome_of(&progress), outcome);
        assert!(
            crate::workspace_manager::contended_attempts(&common) > contended,
            "the access failed on the tear first"
        );
        assert_eq!(
            hooks.counting.raw, 0,
            "the coordinator's caller's raw pause was never used: every wait went through \
                 the coordinator"
        );
        assert_eq!(
            crate::workspace_manager::fixture::slept_pauses(),
            slept,
            "no wait of a registry access the coordinator made slept on its thread"
        );
    });
}

#[test]
fn a_finalizations_registry_wait_answers_on_the_coordinator() {
    waits_through_the_coordinator(
        "a finalization meeting a tear",
        || two_held("waits-finalization"),
        TearAt::Fold(run_finished),
        Torn::CommondirEmpty,
        first_invoking,
        RunOutcome::Complete,
    );
}

#[test]
fn a_closures_registry_wait_answers_on_the_coordinator() {
    waits_through_the_coordinator(
        "a halt's closure meeting a tear",
        || halting_on_gamma("waits-closure"),
        TearAt::Fold(attempt_interrupted),
        Torn::GitdirMissing,
        gamma_halts_while_alpha_gates,
        RunOutcome::Halted,
    );
}

/// The follow-up C record's §6.11, the proposed frozen hunk C-R1: the
/// frozen finalization's scrub enumerates through the coordinator's hooks
/// (`finalize.rs`'s `scrub_slots`, `intents_pausing`), so the registry read
/// that discovers earlier incarnations' instances waits through the
/// coordinator. It is the first registry access after `run_finished`. Without
/// C-R1 (`2c3e413c`) the read slept on the coordinator's thread.
#[test]
fn a_finalizations_scrub_discovery_answers_on_the_coordinator() {
    waits_through_the_coordinator(
        "a finalization scrub's registry read meeting an unreadable registration",
        || two_held("waits-scrub-discovery"),
        TearAt::Access(run_finished, 1),
        Torn::GitdirUnreadable,
        first_invoking,
        RunOutcome::Complete,
    );
}

#[test]
fn a_final_sweeps_registry_read_answers_on_the_coordinator() {
    waits_through_the_coordinator(
        "a final sweep's registry read meeting an unreadable registration",
        || two_held("waits-sweep-read"),
        TearAt::Access(run_finished, 6),
        Torn::GitdirUnreadable,
        first_invoking,
        RunOutcome::Complete,
    );
}

#[test]
fn a_closures_snapshot_discovery_answers_on_the_coordinator() {
    waits_through_the_coordinator(
        "a closure's snapshot reclaim's registry read meeting an unreadable registration",
        || halting_on_gamma("waits-closure-discovery"),
        TearAt::Access(attempt_interrupted, 1),
        Torn::GitdirUnreadable,
        gamma_halts_while_alpha_gates,
        RunOutcome::Halted,
    );
}

#[test]
fn append_error_under_concurrency_cancels_pipelines_and_folds_nothing_from_memory() {
    use crate::topology::effects::SubEffectPoint;
    let tasks = three();
    for (shape, point, present) in [
        ("partial-write", SubEffectPoint::Written, false),
        ("flush-after-full-line", SubEffectPoint::WrittenFull, true),
        ("sync", SubEffectPoint::Synced, true),
    ] {
        let mut wide = Wide::durable(
            &format!("coordinator-append-error-{shape}"),
            &tasks,
            3,
            WidePlans::default(),
            holding(&tasks, &[]),
        );
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let harness = std::sync::Arc::clone(&wide.env.harness);
        let mut scheduler = arming_on(&runner, harness, point);
        let error =
            drive(&mut wide, Some(&mut scheduler)).expect_err("the append error ends the command");
        drop(scheduler);
        let text = error.to_string();
        assert!(
            text.contains("candidate_prepared") && text.contains("was entered and returned"),
            "{shape}: the protocol's report names the event kind: {text}"
        );
        assert!(
            text.contains(if present {
                "the proven prefix contains the line"
            } else {
                "the proven prefix does not contain the line"
            }),
            "{shape}: the report says what the reopened prefix holds: {text}"
        );
        assert!(
            wide.run.fold().is_poisoned(),
            "{shape}: the fold is poisoned"
        );
        assert!(
            matches!(
                wide.run
                    .fold()
                    .task(TaskKey(0))
                    .and_then(|task| task.generations.first())
                    .map(|generation| &generation.class),
                Some(crate::topology::fold::GenerationClass::InFlight { .. })
            ),
            "{shape}: nothing was folded from memory: alpha is still in flight in this \
                 process's fold"
        );
        assert!(
            !wide
                .run
                .events()
                .iter()
                .any(|event| event.body.kind() == "candidate_prepared"),
            "{shape}: the entered append is not in this process's events"
        );
        let durable = kinds_of_log(&wide);
        assert_eq!(
            durable.last(),
            Some(if present {
                &"candidate_prepared"
            } else {
                &"attempt_started"
            }),
            "{shape}: nothing was appended after the error, and the surviving prefix is the \
                 one the report names: {durable:?}"
        );
        let cancelled: Vec<u32> = wide
            .env
            .runner
            .endings()
            .into_iter()
            .filter(|(_, ending)| *ending == crate::engine::topology::scaffold::Ending::Cancelled)
            .filter_map(|(invocation, _)| attempt_key(&invocation))
            .collect();
        let mut cancelled = cancelled;
        cancelled.sort_unstable();
        assert_eq!(
            cancelled,
            vec![1, 2],
            "{shape}: the in-flight pipelines were cancelled through the Runner"
        );
        assert_eq!(
            wide.run.discarded(),
            2,
            "{shape}: their completions were discarded, none applied to the poisoned fold"
        );
        let ledger = wide.run.broker_mut().invocations();
        assert!(ledger.balances(), "{shape}");
        assert_eq!(
            ledger.duplicates(),
            0,
            "{shape}: the protocol withdrew only what was waiting, and each running \
                 registration was settled once, by its own pipeline's end report after its process \
                 terminated (R-AI, as corrected in round R5)"
        );
        assert!(
            !wide.env.paths.public.join("report.json").exists(),
            "{shape}: no report from the poisoned fold"
        );
        assert!(
            wide.env.fixture.manager.execution_root().exists(),
            "{shape}: and no cleanup"
        );

        let (recovered, mut resumed) = wide
            .resume(
                "inc-2",
                holding(&tasks, &[]),
                crate::engine::topology::select::Ceiling::unlimited(),
            )
            .unwrap_or_else(|error| panic!("{shape}: the next process resumes: {error}"));
        assert_eq!(
            (recovered.interrupted, recovered.finished.clone()),
            if present {
                (2, vec![TaskKey(0)])
            } else {
                (3, Vec::new())
            },
            "{shape}: the resume follows the surviving prefix: a present line is a promotion \
                 it completes, an absent one leaves alpha in flight to settle interrupted"
        );
        let runner = std::sync::Arc::clone(&resumed.env.runner);
        let mut scheduler = Scheduler::first(&runner);
        let progress = drive(&mut resumed, Some(&mut scheduler))
            .unwrap_or_else(|error| panic!("{shape}: the resumed run completes: {error}"));
        drop(scheduler);
        assert_eq!(outcome_of(&progress), RunOutcome::Complete, "{shape}");
        assert_eq!(
            count(&resumed.env.durable_events(), "task_merged"),
            3,
            "{shape}"
        );
        replay_equals_live(&resumed);
    }
}

#[test]
fn an_append_error_releases_a_running_invocation_only_when_its_end_establishes_its_process_gone() {
    use crate::engine::topology::scaffold::Ending;
    let tasks = three();
    let mut wide = Wide::durable(
        "coordinator-append-error-unresolved",
        &tasks,
        3,
        WidePlans::default(),
        holding(&tasks, &[]),
    );
    let beta = AttemptIdentities::new(TaskKey(1), GenerationId(0), AttemptNumber(1)).worker();
    let gamma = AttemptIdentities::new(TaskKey(2), GenerationId(0), AttemptNumber(1)).worker();
    wide.env.runner.unresolved_when_cancelled(beta.clone());
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let harness = std::sync::Arc::clone(&wide.env.harness);
    let mut scheduler = arming_on(
        &runner,
        harness,
        crate::topology::effects::SubEffectPoint::WrittenFull,
    );
    let error =
        drive(&mut wide, Some(&mut scheduler)).expect_err("the append error ends the command");
    drop(scheduler);
    assert!(wide.run.fold().is_poisoned(), "{error}");
    let endings = wide.env.runner.endings();
    assert!(
        endings.contains(&(beta.clone(), Ending::CancelledUnresolved))
            && endings.contains(&(gamma.clone(), Ending::Cancelled)),
        "the Runner did not establish that beta's process ended, and did establish gamma's: \
             {endings:?}"
    );
    let named = beta.render();
    assert!(
        wide.run
            .warnings()
            .iter()
            .any(|warning| warning.contains(&named) && warning.contains("did not establish")),
        "the command's end names the invocation it still holds: {:?}",
        wide.run.warnings()
    );
    let ledger = wide.run.broker_mut().invocations();
    let holders: Vec<String> = ledger
        .slots()
        .holders()
        .iter()
        .map(|holder| holder.render())
        .collect();
    assert_eq!(
        (
            ledger.running(),
            holders,
            ledger.balances(),
            ledger.settled(&gamma),
            ledger.duplicates()
        ),
        (vec![named.as_str()], vec![named.clone()], false, true, 0),
        "beta's end did not establish that its process is gone, so beta keeps its registration \
             and its pair, and the ledger does not balance; gamma's end did, and released it once. \
             Neither was settled by the append-error protocol before its pipeline reported its end: \
             (running, pair holders, balances, gamma settled, duplicate settlements)"
    );
}

#[test]
fn append_error_inside_closure_ends_command_and_resume_completes_closure_at_width_three() {
    use crate::topology::effects::{InjectionMode, SubEffectPoint};
    for (shape, point, survived) in [
        ("partial-write", SubEffectPoint::Written, 0_usize),
        ("sync", SubEffectPoint::Synced, 1),
    ] {
        let mut wide = durable_halting_on_gamma(&format!("coordinator-closure-error-{shape}"));
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let mut scheduler = gamma_first(&runner);
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
            .expect_err("the closure's append error ends the command");
        drop(scheduler);
        assert!(
            error.to_string().contains("attempt_interrupted"),
            "{shape}: the protocol names the closure's first append: {error}"
        );
        assert!(wide.run.fold().is_poisoned(), "{shape}");
        let kinds = kinds_of_log(&wide);
        assert_eq!(
            kinds
                .iter()
                .filter(|kind| **kind == "attempt_interrupted")
                .count(),
            survived,
            "{shape}: the surviving prefix is the one the report names: {kinds:?}"
        );
        assert!(
            !kinds.contains(&"run_finished"),
            "{shape}: nothing after the error"
        );
        assert!(
            !wide.env.paths.public.join("report.json").exists(),
            "{shape}: no report from the poisoned fold"
        );
        let (recovered, resumed) = finish_resumed_halted(wide, shape);
        assert_eq!(
            recovered.interrupted,
            2 - survived,
            "{shape}: recovery settles what the surviving prefix leaves in flight"
        );
        ends_once_halted(&resumed, shape);
    }
}

#[test]
fn kill_inside_closure_recovers_at_width_three() {
    for (shape, survived) in [("torn", 0_usize), ("complete", 1)] {
        let handoff = crate::engine::topology::scaffold::kill_dir(&format!(
            "coordinator-closure-kill-{shape}"
        ));
        let temporary = crate::engine::topology::scaffold::kill_dir("tmp");
        let mut environment = vec![
            ("UPSTROKE_TEST_KILL_DIR", handoff.path().as_os_str()),
            ("UPSTROKE_TEST_KILL_SHAPE", std::ffi::OsStr::new(shape)),
        ];
        environment.extend(crate::engine::topology::scaffold::child_temporary_of(
            temporary.path(),
        ));
        let status = crate::workspace_manager::fixture::run_kill_child_within(
            "engine::topology::coordinator::tests::closure_kill_child_at_width_three",
            &environment,
            crate::engine::topology::scaffold::KILL_CHILD_BOUND,
        )
        .unwrap_or_else(|| panic!("{shape}: the kill child did not end in time"));
        assert!(
            crate::workspace_manager::fixture::died_by_abort(&status),
            "{shape}: the child died inside the closure: {status:?}"
        );
        let root = std::fs::read_to_string(handoff.path().join(KILL_HANDOFF))
            .unwrap_or_else(|error| panic!("{shape}: the child left no handoff: {error}"));
        let mut env = crate::engine::topology::scaffold::WideEnv::adopted(
            std::path::PathBuf::from(root),
            &three(),
            3,
            WidePlans::default(),
        );
        halting(&mut env);
        let kinds: Vec<&str> = kinds_of(&env.durable_events());
        assert!(
            kinds.contains(&"question_answered") && !kinds.contains(&"run_finished"),
            "{shape}: the child halted and died before the end: {kinds:?}"
        );
        assert_eq!(
            kinds
                .iter()
                .filter(|kind| **kind == "attempt_interrupted")
                .count(),
            survived,
            "{shape}: {kinds:?}"
        );
        let tasks = three();
        let (recovered, resumed) = env
            .resume(
                "inc-2",
                RecordingRunner::new().answering(wide_responder(&tasks, &[])),
                crate::engine::topology::select::Ceiling::unlimited(),
            )
            .unwrap_or_else(|error| panic!("{shape}: the next process resumes: {error}"));
        assert_eq!(recovered.interrupted, 2 - survived, "{shape}");
        let mut resumed = resumed;
        let progress = drive(&mut resumed, None).unwrap_or_else(|error| panic!("{shape}: {error}"));
        assert_eq!(outcome_of(&progress), RunOutcome::Halted, "{shape}");
        ends_once_halted(&resumed, shape);
    }
}

#[test]
fn authorized_publication_completed_at_run_end() {
    let tasks = [
        WideTask::independent("alpha"),
        WideTask::independent("beta"),
    ];
    let mut wide = halted_by_the_last_task("coordinator-fast-at-end", &tasks);
    let mut hooks = CasFailing::over(&wide);
    steps_to(
        &mut wide,
        &mut hooks,
        |progress| {
            matches!(
                progress,
                Progress::Settled {
                    key: TaskKey(0),
                    accepted: true,
                    ..
                }
            )
        },
        "alpha settles",
    );
    hooks.fail_the_next(crate::topology::effects::RefSite::CompareAndSwapIntegration);
    let seams = wide.env.seams();
    let error = wide
        .run
        .step(&seams, &mut hooks)
        .expect_err("the fast integration's CAS is made to fail once");
    assert!(
        error.to_string().contains("CompareAndSwapIntegration"),
        "{error}"
    );
    assert!(
        matches!(
            wide.run.fold().transaction().map(|open| &open.class),
            Some(crate::topology::fold::TransactionClass::Prepared {
                disposition: crate::topology::events::PreparedDisposition::Fast,
                ..
            })
        ),
        "`merge_prepared(fast)` is durable and `task_merged` is not: the state no \
             coordinator schedule reaches at a live end (R-AG)"
    );
    let before = wide.env.head(&wide.run);
    steps_to(
        &mut wide,
        &mut hooks,
        |progress| {
            matches!(
                progress,
                Progress::Settled {
                    key: TaskKey(1),
                    accepted: false,
                    ..
                }
            )
        },
        "beta parks",
    );
    steps_to(
        &mut wide,
        &mut hooks,
        |progress| matches!(progress, Progress::Answered { declined: true, .. }),
        "the decline is ingested",
    );
    let end = steps_to(
        &mut wide,
        &mut hooks,
        |progress| matches!(progress, Progress::Finished { .. }),
        "the closure ends the run",
    );
    assert_eq!(outcome_of(&end), RunOutcome::Halted);
    assert_eq!(
        after_the_halt(&wide),
        ["question_answered", "task_merged", "run_finished"],
        "the closure completed the authorized publication before it ended the run"
    );
    let after = wide.env.head(&wide.run);
    assert_ne!(after, before, "the CAS moved the integration ref");
    assert!(wide.run.fold().transaction().is_none());
    assert!(wide.run.invocations_balance());
    replay_equals_live(&wide);
}

#[test]
fn prepared_publication_completed_at_run_end() {
    let tasks = three();
    let armed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let runner = RecordingRunner::new().answering(
        crate::engine::topology::scaffold::wide_responder_asking(&tasks, &[], &[2]),
    );
    runner.hold();
    let mut wide = Wide::started_with(
        "coordinator-prepared-at-end",
        &tasks,
        3,
        WidePlans::default(),
        runner,
    );
    halting(&mut wide.env);
    wide.env.answers = std::sync::Arc::new(DecliningOnceArmed(std::sync::Arc::clone(&armed)));
    let mut hooks = CasFailing::over(&wide);
    hooks.fail_after(
        crate::topology::effects::RefSite::CompareAndSwapIntegration,
        1,
    );
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let mut scheduler = Scheduler::scripted(
        &runner,
        Box::new(|view: &Quiescent<'_>| {
            released(view, |invocation| worker(invocation) == Some(TaskKey(2)))
                .or_else(|| released(view, |invocation| attempt_key(invocation) == Some(0)))
                .or_else(|| released(view, |invocation| attempt_key(invocation) == Some(1)))
        }),
    );
    let pipelines = wide.env.pipelines();
    let error = wide
        .run
        .run_concurrently(
            &wide.env.seams(),
            &pipelines,
            &mut hooks,
            Some(&mut scheduler),
        )
        .expect_err("beta's stale integration verifies, prepares, and its CAS fails once");
    drop(scheduler);
    assert!(
        error.to_string().contains("CompareAndSwapIntegration"),
        "{error}"
    );
    let run_id = wide.run.fold().started().expect("started").run_id.clone();
    let pin = crate::engine::topology::integrate::prepared_pin_ref(&run_id, SequenceId(1));
    let manager = wide.env.fixture.manager.clone();
    assert!(
        matches!(
            wide.run.fold().transaction().map(|open| &open.class),
            Some(crate::topology::fold::TransactionClass::Prepared {
                disposition: crate::topology::events::PreparedDisposition::StaleClean,
                ..
            })
        ),
        "a stale-clean `merge_prepared` is durable and its publication is pending: {:?}",
        wide.run.fold().transaction()
    );
    let proposed = manager
        .direct_ref_target(pin.as_str())
        .expect("the pin is readable")
        .expect("the verified proposal is pinned");
    assert!(
        manager.intents().expect("intents").contains(
            &crate::engine::topology::integrate::staging_slot(SequenceId(1))
        ),
        "and its staging is still there"
    );

    armed.store(true, std::sync::atomic::Ordering::SeqCst);
    steps_to(
        &mut wide,
        &mut hooks,
        |progress| matches!(progress, Progress::Answered { declined: true, .. }),
        "gamma's decline is ingested",
    );
    let end = steps_to(
        &mut wide,
        &mut hooks,
        |progress| matches!(progress, Progress::Finished { .. }),
        "the closure ends the run",
    );
    assert_eq!(outcome_of(&end), RunOutcome::Halted);
    assert_eq!(
        after_the_halt(&wide),
        ["question_answered", "task_merged", "run_finished"],
        "the closure published the verified proposal before it ended the run"
    );
    assert_eq!(
        wide.env.head(&wide.run),
        proposed,
        "the CAS put the pinned proposal there"
    );
    assert_eq!(
        manager
            .direct_ref_target(pin.as_str())
            .expect("the pin is readable"),
        None,
        "and the pin was pruned"
    );
    replay_equals_live(&wide);
}

#[test]
fn promoting_completed_by_the_closure_at_run_end() {
    let tasks = [
        WideTask::independent("alpha"),
        WideTask::independent("beta"),
    ];
    let mut wide = halted_by_the_last_task("coordinator-promoting-at-end", &tasks);
    let mut hooks = CasFailing::over(&wide);
    hooks.fail_the_next(crate::topology::effects::RefSite::CreateCandidates);
    let seams = wide.env.seams();
    let error = wide
        .run
        .step(&seams, &mut hooks)
        .expect_err("alpha's candidates ref is made to fail once");
    assert!(error.to_string().contains("CreateCandidates"), "{error}");
    assert!(
        matches!(
            wide.run
                .fold()
                .task(TaskKey(0))
                .and_then(|task| task.generations.first())
                .map(|generation| &generation.class),
            Some(crate::topology::fold::GenerationClass::Promoting)
        ),
        "`candidate_prepared` is durable and `task_candidate_created` is not"
    );
    steps_to(
        &mut wide,
        &mut hooks,
        |progress| {
            matches!(
                progress,
                Progress::Settled {
                    key: TaskKey(1),
                    accepted: false,
                    ..
                }
            )
        },
        "beta parks",
    );
    steps_to(
        &mut wide,
        &mut hooks,
        |progress| matches!(progress, Progress::Answered { declined: true, .. }),
        "the decline is ingested",
    );
    let end = steps_to(
        &mut wide,
        &mut hooks,
        |progress| matches!(progress, Progress::Finished { .. }),
        "the closure ends the run",
    );
    assert_eq!(outcome_of(&end), RunOutcome::Halted);
    assert_eq!(
        after_the_halt(&wide),
        [
            "question_answered",
            "task_candidate_created",
            "run_finished"
        ],
        "the closure completed the promotion before it ended the run"
    );
    let run_id = wide.run.fold().started().expect("started").run_id.clone();
    let names = crate::engine::topology::candidate::CandidateNames::of(
        &run_id,
        TaskKey(0),
        GenerationId(0),
    );
    let manager = wide.env.fixture.manager.clone();
    assert!(
        manager
            .direct_ref_target(names.candidate_ref.as_str())
            .expect("the candidates ref is readable")
            .is_some(),
        "the candidates ref was created, and Halted retains it"
    );
    assert_eq!(
        manager
            .direct_ref_target(names.prepared_ref.as_str())
            .expect("the pin is readable"),
        None,
        "the candidate-prepared pin was pruned"
    );
    replay_equals_live(&wide);
}

struct ArmsOnRetained {
    inner: crate::engine::topology::seams::HarnessTopologyHooks,
    armed: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl TopologyHooks for ArmsOnRetained {
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
        let retained = events.last().is_some_and(|event| {
            matches!(&event.body, TopologyEventBody::AttemptFinished { data }
                if matches!(data.settlement,
                    crate::topology::events::AttemptSettlement::Retained { .. }))
        });
        if retained {
            self.armed.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }
}

fn generation_closed_run_ending(events: &[TopologyEvent]) -> Vec<(u32, RunOutcome)> {
    events
        .iter()
        .filter_map(|event| match &event.body {
            TopologyEventBody::GenerationClosed { data } => match &data.reason {
                crate::topology::events::GenerationCloseReason::RunEnding { outcome } => {
                    Some((data.key.0, outcome.clone()))
                }
                _ => None,
            },
            _ => None,
        })
        .collect()
}

#[test]
fn retained_generation_closed_at_run_end_at_width_three() {
    let tasks = three();
    let armed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let runner = RecordingRunner::new().answering(
        crate::engine::topology::scaffold::wide_responder_asking(&tasks, &[(0, 1)], &[2]),
    );
    runner.hold();
    let mut wide = Wide::started_with(
        "coordinator-retained-halted",
        &tasks,
        3,
        WidePlans::default(),
        runner,
    );
    halting(&mut wide.env);
    wide.env.answers = std::sync::Arc::new(DecliningOnceArmed(std::sync::Arc::clone(&armed)));
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let mut scheduler = Scheduler::scripted(
        &runner,
        Box::new(|view: &Quiescent<'_>| {
            released(view, |invocation| worker(invocation) == Some(TaskKey(2)))
                .or_else(|| released(view, |invocation| attempt_key(invocation) == Some(0)))
        }),
    );
    let mut hooks = ArmsOnRetained {
        inner: wide.env.hooks(),
        armed,
    };
    let pipelines = wide.env.pipelines();
    let progress = wide
        .run
        .run_concurrently(
            &wide.env.seams(),
            &pipelines,
            &mut hooks,
            Some(&mut scheduler),
        )
        .expect("the halt ends the run");
    drop(scheduler);
    assert_eq!(outcome_of(&progress), RunOutcome::Halted);
    assert_eq!(
        after_the_halt(&wide),
        [
            "question_answered",
            "attempt_interrupted",
            "generation_closed",
            "run_finished"
        ],
        "beta, in flight, is interrupted and alpha's retained generation closed run-ending"
    );
    assert_eq!(
        generation_closed_run_ending(&wide.env.durable_events()),
        vec![(0, RunOutcome::Halted)]
    );
    replay_equals_live(&wide);

    let mut wide = Wide::started_under(
        "coordinator-retained-budget",
        &tasks,
        3,
        WidePlans::default(),
        holding(&tasks, &[(0, 1)]),
        crate::engine::topology::select::Ceiling {
            run_usd: Some(0.2),
            task_usd: None,
        },
    );
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let mut scheduler = Scheduler::scripted(
        &runner,
        Box::new(|view: &Quiescent<'_>| {
            released(view, |invocation| attempt_key(invocation) == Some(0))
        }),
    );
    let progress = drive(&mut wide, Some(&mut scheduler)).expect("the budget stop ends the run");
    drop(scheduler);
    assert_eq!(outcome_of(&progress), RunOutcome::BudgetExceeded);
    let events = wide.env.durable_events();
    let kinds = kinds_of(&events);
    let stop = position(&kinds, "budget_exceeded", 0);
    assert!(
        kinds[..stop].contains(&"attempt_finished") && !kinds[stop..].contains(&"attempt_started"),
        "the retry the retained generation was ready for is what the ceiling refused: {kinds:?}"
    );
    assert_eq!(
        generation_closed_run_ending(&events),
        vec![(0, RunOutcome::BudgetExceeded)],
        "{kinds:?}"
    );
    replay_equals_live(&wide);
}
