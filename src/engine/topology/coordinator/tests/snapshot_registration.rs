//! Extended notes: `docs/internals/engine/topology/coordinator/tests/snapshot_registration.md`

use super::*;

type FAct = Box<dyn FnMut()>;
type FFoldAct = (fn(&TopologyEventBody) -> bool, FAct);
type FSharedAct = Arc<dyn Fn() + Send + Sync>;
type FKept = Arc<std::sync::Mutex<Option<BTreeMap<PathBuf, Vec<u8>>>>>;
type FSite = crate::topology::effects::EffectSiteId;
type FPhase = crate::topology::effects::HookPhase;

const F_PICK: FSite = FSite::Object(crate::topology::effects::ObjectSite::ProposalCherryPick);
const F_WRITE_TREE: FSite = FSite::Object(crate::topology::effects::ObjectSite::CandidateWriteTree);

struct FEffects {
    effects: crate::workspace_manager::HarnessEffects,
    at_phase: Vec<(FSite, FPhase, FAct)>,
    before_access: Vec<(FSite, FPhase, crate::workspace_manager::fixture::AccessAct)>,
    planting: Option<crate::workspace_manager::fixture::AccessPlanting>,
    fail_at: Option<(FSite, FPhase)>,
}

impl crate::workspace_manager::EffectHooks for FEffects {
    fn phase(&mut self, site: FSite, phase: FPhase) -> crate::topology::effects::Injection {
        let answer = self.effects.phase(site, phase);
        if let Some(index) = self
            .at_phase
            .iter()
            .position(|(at, when, _)| *at == site && *when == phase)
        {
            let (_, _, mut act) = self.at_phase.remove(index);
            act();
        }
        if let Some(index) = self
            .before_access
            .iter()
            .position(|(at, when, _)| *at == site && *when == phase)
        {
            let (_, _, act) = self.before_access.remove(index);
            self.planting = Some(crate::workspace_manager::fixture::before_registry_access(
                1, act,
            ));
        }
        if self.fail_at == Some((site, phase)) {
            self.fail_at = None;
            return crate::topology::effects::Injection::Error;
        }
        answer
    }

    fn durability_ledger(&self) -> crate::util::DurabilityLedger {
        self.effects.durability_ledger()
    }

    fn refusal_cause(&self) -> Option<String> {
        self.effects.refusal_cause()
    }

    fn registry_pause(&mut self, pause: Duration) -> Result<(), UpstrokeError> {
        self.effects.registry_pause(pause)
    }
}

struct FHooks {
    inner: crate::engine::topology::seams::HarnessTopologyHooks,
    effects: FEffects,
    on_fold: Vec<FFoldAct>,
}

impl FHooks {
    fn over(wide: &Wide) -> Self {
        Self {
            inner: wide.env.hooks(),
            effects: FEffects {
                effects: crate::workspace_manager::HarnessEffects::new(Arc::clone(
                    &wide.env.harness,
                )),
                at_phase: Vec::new(),
                before_access: Vec::new(),
                planting: None,
                fail_at: None,
            },
            on_fold: Vec::new(),
        }
    }
}

impl TopologyHooks for FHooks {
    fn effects(&mut self) -> &mut dyn crate::workspace_manager::EffectHooks {
        &mut self.effects
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
        for event in events {
            if let Some(index) = self.on_fold.iter().position(|(when, _)| when(&event.body)) {
                let (_, mut act) = self.on_fold.remove(index);
                act();
            }
        }
    }
}

fn f_only_checkout_under(namespace: &std::path::Path) -> PathBuf {
    let mut found: Vec<PathBuf> = std::fs::read_dir(namespace)
        .expect("a slot namespace")
        .map(|entry| entry.expect("an entry").path())
        .filter(|path| path.is_dir())
        .collect();
    assert_eq!(
        found.len(),
        1,
        "one checkout under {}: {found:?}",
        namespace.display()
    );
    found.pop().expect("one checkout")
}

fn f_prune_whole(base: &std::path::Path, checkout: &std::path::Path) {
    let pointer = checkout.join(".git");
    let bytes = std::fs::read(&pointer).expect("the checkout's .git");
    crate::workspace_manager::fixture::remove_file(&pointer);
    crate::workspace_manager::fixture::git(base, &["worktree", "prune", "--expire=now"]);
    crate::workspace_manager::fixture::write_file(&pointer, &bytes);
}

fn f_cls_tasks() -> [WideTask; 3] {
    [
        WideTask::deleting_every_tracked_file("beta"),
        WideTask::independent("alpha"),
        WideTask::hinted("gamma", &["src/gamma/"], "a.txt"),
    ]
}

fn f_stale_tasks() -> [WideTask; 3] {
    [
        WideTask::independent("beta"),
        WideTask::independent("alpha"),
        WideTask::independent("gamma"),
    ]
}

fn f_rejected_conflict_of_gamma(events: &[TopologyEvent]) -> bool {
    events.iter().any(|event| {
        matches!(
            &event.body,
            TopologyEventBody::MergeRejected { data }
                if data.candidate.key == TaskKey(2)
                    && matches!(
                        data.disposition,
                        crate::topology::events::RejectionDisposition::Conflict { .. }
                    )
        )
    })
}

fn f_verification_started_of_gamma(events: &[TopologyEvent]) -> Vec<SequenceId> {
    events
        .iter()
        .filter_map(|event| match &event.body {
            TopologyEventBody::MergeVerificationStarted { data }
                if data.candidate.key == TaskKey(2) =>
            {
                Some(data.sequence)
            }
            _ => None,
        })
        .collect()
}

fn f_registry_refused(result: Result<Progress, UpstrokeError>, words: &[&str], wide: &Wide) {
    match result {
        Err(UpstrokeError::RegistryRefused { message }) => {
            for word in words {
                assert!(
                    message.contains(word),
                    "a refusal naming `{word}`: {message}"
                );
            }
        }
        other => panic!(
            "a registry refusal, not {other:?}: {:?}",
            kinds_of(&wide.env.durable_events())
        ),
    }
}

fn f_step_to_error(
    run: &mut crate::engine::topology::run::TopologyRun,
    seams: &crate::engine::topology::run::RunSeams<'_>,
    hooks: &mut dyn TopologyHooks,
) -> Result<Progress, UpstrokeError> {
    let mut steps = 0_u32;
    loop {
        steps += 1;
        assert!(steps < 200, "the width-1 loop did not end");
        match run.step(seams, hooks) {
            Ok(progress @ Progress::Finished { .. }) => return Ok(progress),
            Ok(_) => {}
            Err(error) => return Err(error),
        }
    }
}

fn f_gamma_queued(tag: &str, tasks: &[WideTask], runner: RecordingRunner) -> Wide {
    let mut wide = Wide::durable(tag, tasks, 3, WidePlans::default(), holding(tasks, &[]));
    let mut hooks = FHooks::over(&wide);
    hooks.effects.fail_at = Some((F_PICK, FPhase::Before));
    let pipelines = wide.env.pipelines();
    let held = Arc::clone(&wide.env.runner);
    let mut scheduler = if tasks.len() == 3 {
        Scheduler::scripted(&held, Box::new(alpha_waits_for_gammas_integration))
    } else {
        Scheduler::first(&held)
    };
    let ended = wide.run.run_concurrently(
        &wide.env.seams(),
        &pipelines,
        &mut hooks,
        Some(&mut scheduler),
    );
    drop(scheduler);
    drop(hooks);
    assert!(
        ended.is_err(),
        "the first command ends at gamma's pick: {:?}",
        kinds_of(&wide.env.durable_events())
    );
    assert!(f_verification_started_of_gamma(&wide.env.durable_events()).is_empty());
    let (_, resumed) = wide
        .resume(
            "inc-2",
            runner,
            crate::engine::topology::select::Ceiling::unlimited(),
        )
        .expect("the run resumes with gamma's stale candidate queued");
    resumed
}

fn f_finish(mut wide: Wide, incarnation: &str, tasks: &[WideTask]) -> Wide {
    let (_, mut resumed) = wide
        .resume(
            incarnation,
            holding(tasks, &[]),
            crate::engine::topology::select::Ceiling::unlimited(),
        )
        .expect("the next process resumes");
    let runner = Arc::clone(&resumed.env.runner);
    let mut scheduler = Scheduler::first(&runner);
    let progress = drive(&mut resumed, Some(&mut scheduler)).expect("the resumed run ends");
    drop(scheduler);
    assert!(
        matches!(progress, Progress::Finished { .. }),
        "{progress:?}"
    );
    wide = resumed;
    wide
}

fn f_delete_staging_index_at_the_classification(hooks: &mut FHooks, root: PathBuf, rewrite: bool) {
    hooks.effects.before_access.push((
        F_PICK,
        FPhase::Before,
        Box::new(move |_| {
            let checkout = f_only_checkout_under(&root.join("merge"));
            crate::workspace_manager::fixture::remove_file(&f_entry_of(&checkout).join("index"));
            if rewrite {
                crate::workspace_manager::fixture::git(&checkout, &["read-tree", "HEAD"]);
            }
        }),
    ));
}

fn f_cls_run_width_three(tag: &'static str, concurrency: bool) {
    bounded(tag, move || {
        let tasks = f_cls_tasks();
        let mut wide = Wide::durable(tag, &tasks, 3, WidePlans::default(), holding(&tasks, &[]));
        let root = wide.env.fixture.manager.execution_root().to_path_buf();
        let mut hooks = FHooks::over(&wide);
        f_delete_staging_index_at_the_classification(&mut hooks, root, false);
        let pipelines = wide.env.pipelines();
        let runner = Arc::clone(&wide.env.runner);
        let mut scheduler =
            Scheduler::scripted(&runner, Box::new(alpha_waits_for_gammas_integration));
        let result = wide.run.run_concurrently(
            &wide.env.seams(),
            &pipelines,
            &mut hooks,
            Some(&mut scheduler),
        );
        drop(scheduler);
        drop(hooks);
        f_registry_refused(result, &["classification", "index"], &wide);
        let events = wide.env.durable_events();
        assert!(
            f_verification_started_of_gamma(&events).is_empty(),
            "nothing is appended for gamma's sequence: {:?}",
            kinds_of(&events)
        );
        assert!(!f_rejected_conflict_of_gamma(&events));
        if concurrency {
            assert!(
                wide.run.discarded() >= 1,
                "the live attempt's completion is discarded: {}",
                wide.run.discarded()
            );
            assert!(
                !wide
                    .run
                    .warnings()
                    .iter()
                    .any(|warning| warning.contains("classification")),
                "{:?}",
                wide.run.warnings()
            );
        }
        let resumed = f_finish(wide, "inc-2", &tasks);
        let events = resumed.env.durable_events();
        assert!(
            f_rejected_conflict_of_gamma(&events),
            "the next resume integrates gamma again and rejects the conflict: {:?}",
            kinds_of(&events)
        );
        if concurrency {
            assert!(
                events.iter().any(|event| matches!(
                    &event.body,
                    TopologyEventBody::AttemptInterrupted { data } if data.key == TaskKey(1)
                )),
                "the attempt the refusal cancelled is settled interrupted: {:?}",
                kinds_of(&events)
            );
        }
    });
}

#[test]
fn a_stale_conflict_read_through_a_deleted_staging_index_ends_the_command_at_width_three() {
    f_cls_run_width_three("f-cls-run-width-three", false);
}

#[test]
fn a_classification_refused_beside_a_live_attempt_cancels_it_and_the_resume_integrates_again() {
    f_cls_run_width_three("f-conc-5-classification", true);
}

#[test]
fn a_stale_conflict_read_through_a_deleted_staging_index_ends_the_step_at_width_one() {
    bounded("f-cls-run-width-one", move || {
        let tasks = f_cls_tasks();
        let mut wide = f_gamma_queued(
            "f-cls-run-width-one",
            &tasks,
            RecordingRunner::new().answering(wide_responder(&tasks, &[])),
        );
        let root = wide.env.fixture.manager.execution_root().to_path_buf();
        let mut hooks = FHooks::over(&wide);
        f_delete_staging_index_at_the_classification(&mut hooks, root, false);
        let seams = wide.env.seams();
        let result = f_step_to_error(&mut wide.run, &seams, &mut hooks);
        drop(hooks);
        f_registry_refused(result, &["classification", "index"], &wide);
        assert!(f_verification_started_of_gamma(&wide.env.durable_events()).is_empty());
        let resumed = f_finish(wide, "inc-3", &tasks);
        assert!(f_rejected_conflict_of_gamma(&resumed.env.durable_events()));
    });
}

#[test]
fn a_staging_index_written_again_before_the_classification_reads_as_already_present() {
    bounded("f-residual-iii", move || {
        let tasks = f_cls_tasks();
        let mut wide = Wide::durable(
            "f-residual-iii",
            &tasks,
            3,
            WidePlans::default(),
            holding(&tasks, &[]),
        );
        let root = wide.env.fixture.manager.execution_root().to_path_buf();
        let mut hooks = FHooks::over(&wide);
        f_delete_staging_index_at_the_classification(&mut hooks, root, true);
        let pipelines = wide.env.pipelines();
        let runner = Arc::clone(&wide.env.runner);
        let mut scheduler =
            Scheduler::scripted(&runner, Box::new(alpha_waits_for_gammas_integration));
        let progress = wide
            .run
            .run_concurrently(
                &wide.env.seams(),
                &pipelines,
                &mut hooks,
                Some(&mut scheduler),
            )
            .expect("R-REWRITE: the rewritten index reads whole and the run goes on");
        drop(scheduler);
        drop(hooks);
        assert!(
            matches!(progress, Progress::Finished { .. }),
            "{progress:?}"
        );
        let events = wide.env.durable_events();
        assert!(
            events.iter().any(|event| matches!(
                &event.body,
                TopologyEventBody::MergeVerificationStarted { data }
                    if data.candidate.key == TaskKey(2)
                        && data.basis == crate::topology::events::VerificationBasis::AlreadyPresent
            )),
            "the conflict reads as an empty pick, not seen by any check: {:?}",
            kinds_of(&events)
        );
        assert!(!f_rejected_conflict_of_gamma(&events));
    });
}

fn f_prune_staging_when_gamma_verification_starts(
    hooks: &mut FHooks,
    root: PathBuf,
    base: PathBuf,
) {
    hooks.on_fold.push((
        |body| {
            matches!(
                body,
                TopologyEventBody::MergeVerificationStarted { data } if data.candidate.key == TaskKey(2)
            )
        },
        Box::new(move || {
            let checkout = f_only_checkout_under(&root.join("merge"));
            f_prune_whole(&base, &checkout);
        }),
    ));
}

fn f_interrupted_and_verified_again(resumed: &Wide, refused: SequenceId) {
    let events = resumed.env.durable_events();
    assert!(
        events.iter().any(|event| matches!(
            &event.body,
            TopologyEventBody::MergeVerificationInterrupted { data } if data.sequence == refused
        )),
        "the next resume settles the refused verification interrupted: {:?}",
        kinds_of(&events)
    );
    let started = f_verification_started_of_gamma(&events);
    assert!(
        started.iter().any(|sequence| *sequence != refused),
        "and verifies gamma again under a new sequence: {started:?}"
    );
    assert_eq!(
        count(&events, "merge_verification_unavailable"),
        0,
        "no deferral is spent: {:?}",
        kinds_of(&events)
    );
}

#[test]
fn a_verification_whose_staging_entry_is_pruned_before_its_diff_ends_resumably_at_width_three() {
    bounded("f-p1-verify-width-three", move || {
        let tasks = f_stale_tasks();
        let mut wide = Wide::durable(
            "f-p1-verify-width-three",
            &tasks,
            3,
            WidePlans::default(),
            holding(&tasks, &[]),
        );
        let root = wide.env.fixture.manager.execution_root().to_path_buf();
        let base = wide.env.fixture.base.clone();
        let mut hooks = FHooks::over(&wide);
        f_prune_staging_when_gamma_verification_starts(&mut hooks, root, base);
        let pipelines = wide.env.pipelines();
        let runner = Arc::clone(&wide.env.runner);
        let mut scheduler =
            Scheduler::scripted(&runner, Box::new(alpha_waits_for_gammas_integration));
        let result = wide.run.run_concurrently(
            &wide.env.seams(),
            &pipelines,
            &mut hooks,
            Some(&mut scheduler),
        );
        drop(scheduler);
        drop(hooks);
        f_registry_refused(result, &["merge", "worktree registration failed"], &wide);
        let events = wide.env.durable_events();
        let started = f_verification_started_of_gamma(&events);
        assert_eq!(started.len(), 1, "{:?}", kinds_of(&events));
        assert_eq!(
            kinds_of(&events).last(),
            Some(&"merge_verification_started"),
            "nothing after merge_verification_started"
        );
        let refused = started.first().copied().expect("one sequence");
        let resumed = f_finish(wide, "inc-2", &tasks);
        f_interrupted_and_verified_again(&resumed, refused);
    });
}

#[test]
fn a_verification_whose_staging_entry_is_pruned_before_its_diff_ends_resumably_at_width_one() {
    bounded("f-p1-verify-width-one", move || {
        let tasks = f_stale_tasks();
        let mut wide = f_gamma_queued(
            "f-p1-verify-width-one",
            &tasks,
            RecordingRunner::new().answering(wide_responder(&tasks, &[])),
        );
        f_keep_the_store(&wide);
        let root = wide.env.fixture.manager.execution_root().to_path_buf();
        let base = wide.env.fixture.base.clone();
        let mut hooks = FHooks::over(&wide);
        f_prune_staging_when_gamma_verification_starts(&mut hooks, root, base);
        let seams = wide.env.seams();
        let result = f_step_to_error(&mut wide.run, &seams, &mut hooks);
        drop(hooks);
        f_registry_refused(result, &["merge", "worktree registration failed"], &wide);
        let events = wide.env.durable_events();
        let refused = f_verification_started_of_gamma(&events)
            .last()
            .copied()
            .expect("gamma's verification started");
        assert_eq!(
            kinds_of(&events).last(),
            Some(&"merge_verification_started")
        );
        let resumed = f_finish(wide, "inc-3", &tasks);
        f_interrupted_and_verified_again(&resumed, refused);
    });
}

type FCheckoutAct = Box<dyn Fn(&std::path::Path) + Send + Sync>;

struct FPolicy {
    namespace: &'static str,
    act: FCheckoutAct,
    once: bool,
    acted: std::sync::atomic::AtomicBool,
    calls: Arc<std::sync::atomic::AtomicUsize>,
}

impl FPolicy {
    fn acting(namespace: &'static str, act: FCheckoutAct) -> Self {
        Self {
            namespace,
            act,
            once: true,
            acted: std::sync::atomic::AtomicBool::new(false),
            calls: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }
}

impl crate::engine::topology::attempt::ReviewInputPolicy for FPolicy {
    fn problem(
        &self,
        worktree: &std::path::Path,
        tree: &str,
    ) -> Result<Option<String>, UpstrokeError> {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let here = worktree
            .parent()
            .and_then(std::path::Path::file_name)
            .is_some_and(|name| name == self.namespace);
        if here && !(self.once && self.acted.swap(true, std::sync::atomic::Ordering::SeqCst)) {
            (self.act)(worktree);
        }
        <crate::engine::attempt::LegacyReviewInputPolicy as crate::engine::topology::attempt::ReviewInputPolicy>::problem(
            &crate::engine::attempt::LegacyReviewInputPolicy,
            worktree,
            tree,
        )
    }
}

fn f_removing(name: &'static str) -> FCheckoutAct {
    Box::new(move |checkout| {
        crate::workspace_manager::fixture::remove_file(&f_entry_of(checkout).join(name));
    })
}

fn f_pruning(base: PathBuf) -> FCheckoutAct {
    Box::new(move |checkout| f_prune_whole(&base, checkout))
}

fn f_corrupting() -> FCheckoutAct {
    Box::new(|checkout| {
        crate::workspace_manager::fixture::write_file(
            &f_entry_of(checkout).join("index"),
            b"a corrupt index",
        );
    })
}

fn f_unstaging(and_delete_the_index: bool) -> FCheckoutAct {
    Box::new(move |checkout| {
        crate::workspace_manager::fixture::write_file(
            &checkout.join("a.txt"),
            b"an unstaged change\n",
        );
        if and_delete_the_index {
            crate::workspace_manager::fixture::remove_file(&f_entry_of(checkout).join("index"));
        }
    })
}

fn f_keep_the_store(wide: &Wide) {
    let keeper = wide.env.fixture.root.join("f-store-keeper");
    crate::workspace_manager::fixture::git(
        &wide.env.fixture.base,
        &[
            "worktree",
            "add",
            "-q",
            "--detach",
            keeper.to_str().expect("a fixture path is UTF-8"),
            &wide.env.fixture.head,
        ],
    );
}

fn f_verify_width_three(
    tag: &str,
    tasks: &[WideTask],
    keeper: bool,
    policy: FPolicy,
) -> (Wide, Result<Progress, UpstrokeError>) {
    let mut wide = Wide::durable(tag, tasks, 3, WidePlans::default(), holding(tasks, &[]));
    if keeper {
        f_keep_the_store(&wide);
    }
    let mut hooks = wide.env.hooks();
    let mut pipelines = wide.env.pipelines();
    pipelines.input_policy = Arc::new(policy);
    let runner = Arc::clone(&wide.env.runner);
    let mut scheduler = if tasks.len() == 3 {
        Scheduler::scripted(&runner, Box::new(alpha_waits_for_gammas_integration))
    } else {
        Scheduler::first(&runner)
    };
    let result = wide.run.run_concurrently(
        &wide.env.seams(),
        &pipelines,
        &mut hooks,
        Some(&mut scheduler),
    );
    drop(scheduler);
    drop(hooks);
    (wide, result)
}

fn f_verify_width_one(
    tag: &str,
    tasks: &[WideTask],
    keeper: bool,
    policy: FPolicy,
) -> (Wide, Result<Progress, UpstrokeError>) {
    let mut wide = f_gamma_queued(
        tag,
        tasks,
        RecordingRunner::new().answering(wide_responder(tasks, &[])),
    );
    if keeper {
        f_keep_the_store(&wide);
    }
    let mut hooks = wide.env.hooks();
    let seams = crate::engine::topology::run::RunSeams {
        input_policy: &policy,
        ..wide.env.seams()
    };
    let result = f_step_to_error(&mut wide.run, &seams, &mut hooks);
    drop(hooks);
    (wide, result)
}

fn f_two_stale_tasks() -> [WideTask; 2] {
    [
        WideTask::independent("beta"),
        WideTask::independent("gamma"),
    ]
}

fn f_policy_cases(width: usize) -> Vec<(String, Result<(), String>)> {
    let three = f_stale_tasks();
    let two = f_two_stale_tasks();
    let mut cases = Vec::new();
    let shapes: [(&str, bool, bool, Option<&'static str>); 4] = [
        ("pruned whole, the store kept", true, true, None),
        ("pruned whole, the store gone", false, false, None),
        ("HEAD removed", true, true, Some("HEAD")),
        ("commondir removed", true, true, Some("commondir")),
    ];
    for (index, (label, alpha, keeper, name)) in shapes.into_iter().enumerate() {
        let tag = format!("f-c1-4-w{width}-{index}");
        let tasks: &[WideTask] = if alpha { &three } else { &two };
        let act_for = move |base: PathBuf| -> FCheckoutAct {
            match name {
                Some(name) => f_removing(name),
                None => f_pruning(base),
            }
        };
        let policy_base = std::sync::Arc::new(std::sync::Mutex::new(None::<PathBuf>));
        let deferred: FCheckoutAct = {
            let policy_base = Arc::clone(&policy_base);
            Box::new(move |checkout| {
                let base = policy_base
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .clone()
                    .expect("the base is set before the run");
                act_for(base)(checkout);
            })
        };
        let policy = FPolicy::acting("merge", deferred);
        let (wide, result) = if width == 3 {
            let mut wide = Wide::durable(&tag, tasks, 3, WidePlans::default(), holding(tasks, &[]));
            *policy_base
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) =
                Some(wide.env.fixture.base.clone());
            if keeper {
                f_keep_the_store(&wide);
            }
            let mut hooks = wide.env.hooks();
            let mut pipelines = wide.env.pipelines();
            pipelines.input_policy = Arc::new(policy);
            let runner = Arc::clone(&wide.env.runner);
            let mut scheduler = if alpha {
                Scheduler::scripted(&runner, Box::new(alpha_waits_for_gammas_integration))
            } else {
                Scheduler::first(&runner)
            };
            let result = wide.run.run_concurrently(
                &wide.env.seams(),
                &pipelines,
                &mut hooks,
                Some(&mut scheduler),
            );
            drop(scheduler);
            drop(hooks);
            (wide, result)
        } else {
            let mut wide = f_gamma_queued(
                &tag,
                tasks,
                RecordingRunner::new().answering(wide_responder(tasks, &[])),
            );
            *policy_base
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) =
                Some(wide.env.fixture.base.clone());
            if keeper {
                f_keep_the_store(&wide);
            }
            let mut hooks = wide.env.hooks();
            let seams = crate::engine::topology::run::RunSeams {
                input_policy: &policy,
                ..wide.env.seams()
            };
            let result = f_step_to_error(&mut wide.run, &seams, &mut hooks);
            drop(hooks);
            (wide, result)
        };
        let outcome = match result {
            Err(UpstrokeError::RegistryRefused { message })
                if message.contains("is not used")
                    && message.contains("merge")
                    && !kinds_of(&wide.env.durable_events())
                        .contains(&"merge_verification_unavailable") =>
            {
                Ok(())
            }
            other => Err(format!(
                "{other:?}: {:?}",
                kinds_of(&wide.env.durable_events())
            )),
        };
        cases.push((label.to_owned(), outcome));
    }
    cases
}

fn f_report(cases: &[(String, Result<(), String>)]) {
    let failed: Vec<String> = cases
        .iter()
        .filter_map(|(label, outcome)| outcome.as_ref().err().map(|why| format!("{label}: {why}")))
        .collect();
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

#[test]
fn a_review_input_read_failing_in_a_staging_checkout_whose_registration_is_not_whole_is_refused_at_width_three()
 {
    bounded("f-c1-4-width-three", || f_report(&f_policy_cases(3)));
}

#[test]
fn a_review_input_read_failing_in_a_staging_checkout_whose_registration_is_not_whole_is_refused_at_width_one()
 {
    bounded("f-c1-4-width-one", || f_report(&f_policy_cases(1)));
}

fn f_unavailable_of_gamma(events: &[TopologyEvent]) -> bool {
    let gamma = f_verification_started_of_gamma(events);
    events.iter().any(|event| {
        matches!(
            &event.body,
            TopologyEventBody::MergeVerificationUnavailable { data } if gamma.contains(&data.sequence)
        )
    })
}

#[test]
fn a_review_input_git_error_in_a_whole_staging_registration_settles_an_outage_at_both_widths() {
    bounded("f-c1-4-control", || {
        for width in [3, 1] {
            let tasks = f_stale_tasks();
            let policy = FPolicy::acting("merge", f_corrupting());
            let tag = format!("f-c1-4-control-w{width}");
            let (wide, result) = if width == 3 {
                f_verify_width_three(&tag, &tasks, false, policy)
            } else {
                f_verify_width_one(&tag, &tasks, false, policy)
            };
            let progress = result.unwrap_or_else(|error| panic!("width {width}: {error}"));
            assert!(
                matches!(progress, Progress::Finished { .. }),
                "width {width}"
            );
            let events = wide.env.durable_events();
            assert!(
                f_unavailable_of_gamma(&events),
                "width {width}: a corrupt index in a whole registration is an outage: {:?}",
                kinds_of(&events)
            );
        }
    });
}

fn f_answer_case(width: usize, delete: bool) -> (Wide, Result<Progress, UpstrokeError>) {
    let tasks = f_stale_tasks();
    let policy = FPolicy::acting("merge", f_unstaging(delete));
    let tag = format!("f-c1-4-answer-w{width}-{delete}");
    if width == 3 {
        f_verify_width_three(&tag, &tasks, false, policy)
    } else {
        f_verify_width_one(&tag, &tasks, false, policy)
    }
}

#[test]
fn a_review_input_problem_read_through_a_deleted_staging_index_is_refused_not_hidden() {
    bounded("f-c1-4-answer", || {
        let mut cases = Vec::new();
        for width in [3, 1] {
            let (wide, result) = f_answer_case(width, true);
            let outcome = match result {
                Err(UpstrokeError::RegistryRefused { message })
                    if message.contains("the verification's review-input read")
                        && message.contains("index") =>
                {
                    Ok(())
                }
                other => Err(format!(
                    "{other:?}: {:?}",
                    kinds_of(&wide.env.durable_events())
                )),
            };
            cases.push((format!("width {width}"), outcome));
        }
        f_report(&cases);
    });
}

#[test]
fn a_review_input_problem_in_a_whole_staging_registration_is_a_review_input_failure() {
    bounded("f-c1-4-answer-control", || {
        for width in [3, 1] {
            let (wide, result) = f_answer_case(width, false);
            let progress = result.unwrap_or_else(|error| panic!("width {width}: {error}"));
            assert!(
                matches!(progress, Progress::Finished { .. }),
                "width {width}"
            );
            let events = wide.env.durable_events();
            assert!(
                !events.iter().any(|event| task_merged_of_gamma(&event.body)),
                "width {width}: the unstaged change is a review-input failure, and gamma is \
                     not published: {:?}",
                kinds_of(&events)
            );
        }
    });
}

fn f_one_task() -> [WideTask; 1] {
    [WideTask::independent("alpha")]
}

fn f_attempt_run(
    tag: &str,
    width: u32,
    policy: FPolicy,
    hooks_at: Option<(FSite, FPhase, FSharedAct)>,
    configure: impl FnOnce(&mut Wide),
    responder: Option<crate::engine::topology::scaffold::Responder>,
) -> (Wide, Result<Progress, UpstrokeError>) {
    let tasks = f_one_task();
    let runner =
        RecordingRunner::new().answering(responder.unwrap_or_else(|| wide_responder(&tasks, &[])));
    let mut wide = Wide::started_with(tag, &tasks, width, WidePlans::default(), runner);
    configure(&mut wide);
    let result = if width == 1 {
        let mut hooks = FHooks::over(&wide);
        if let Some((site, phase, act)) = hooks_at {
            hooks
                .effects
                .at_phase
                .push((site, phase, Box::new(move || act())));
        }
        let seams = crate::engine::topology::run::RunSeams {
            input_policy: &policy,
            ..wide.env.seams()
        };
        let result = f_step_to_error(&mut wide.run, &seams, &mut hooks);
        drop(hooks);
        result
    } else {
        let mut hooks = wide.env.hooks();
        let mut pipelines = wide.env.pipelines();
        pipelines.input_policy = Arc::new(policy);
        if let Some((site, phase, act)) = hooks_at {
            pipelines.hooks = f_pipeline_hooks(&wide, site, phase, act);
        }
        wide.run
            .run_concurrently(&wide.env.seams(), &pipelines, &mut hooks, None)
    };
    (wide, result)
}

struct FPipelineEffects {
    effects: crate::workspace_manager::HarnessEffects,
    at: (FSite, FPhase),
    act: FSharedAct,
}

impl crate::workspace_manager::EffectHooks for FPipelineEffects {
    fn phase(&mut self, site: FSite, phase: FPhase) -> crate::topology::effects::Injection {
        let answer = self.effects.phase(site, phase);
        if (site, phase) == self.at {
            (self.act)();
        }
        answer
    }

    fn durability_ledger(&self) -> crate::util::DurabilityLedger {
        self.effects.durability_ledger()
    }

    fn refusal_cause(&self) -> Option<String> {
        self.effects.refusal_cause()
    }

    fn registry_pause(&mut self, pause: Duration) -> Result<(), UpstrokeError> {
        self.effects.registry_pause(pause)
    }
}

struct FPipelineHooks {
    inner: crate::engine::topology::seams::HarnessTopologyHooks,
    effects: FPipelineEffects,
}

impl TopologyHooks for FPipelineHooks {
    fn effects(&mut self) -> &mut dyn crate::workspace_manager::EffectHooks {
        &mut self.effects
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
}

fn f_pipeline_hooks(
    wide: &Wide,
    site: FSite,
    phase: FPhase,
    act: FSharedAct,
) -> super::HooksFactory {
    let harness = Arc::clone(&wide.env.harness);
    Arc::new(move || {
        Box::new(FPipelineHooks {
            inner: crate::engine::topology::seams::HarnessTopologyHooks::new(Arc::clone(&harness)),
            effects: FPipelineEffects {
                effects: crate::workspace_manager::HarnessEffects::new(Arc::clone(&harness)),
                at: (site, phase),
                act: Arc::clone(&act),
            },
        }) as Box<dyn TopologyHooks + Send>
    })
}

fn f_once_in_the_task_slot(root: PathBuf, name: &'static str) -> FSharedAct {
    let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
    Arc::new(move || {
        if !done.swap(true, std::sync::atomic::Ordering::SeqCst) {
            let checkout = f_only_checkout_under(&root.join("tasks"));
            crate::workspace_manager::fixture::remove_file(&f_entry_of(&checkout).join(name));
        }
    })
}

#[test]
fn an_attempts_review_input_problem_read_through_a_deleted_index_is_refused_not_hidden() {
    bounded("f-c1-ap", || {
        let mut cases = Vec::new();
        for width in [1_u32, 3] {
            let policy = FPolicy::acting("tasks", f_unstaging(true));
            let (wide, result) = f_attempt_run(
                &format!("f-c1-ap-w{width}"),
                width,
                policy,
                None,
                |_| {},
                None,
            );
            let outcome = match result {
                Err(UpstrokeError::RegistryRefused { message })
                    if message.contains("the attempt's review-input read")
                        && message.contains("index") =>
                {
                    Ok(())
                }
                other => Err(format!(
                    "{other:?}: {:?}",
                    kinds_of(&wide.env.durable_events())
                )),
            };
            cases.push((format!("width {width}"), outcome));
        }
        f_report(&cases);
    });
}

#[test]
fn an_attempts_review_input_problem_in_a_whole_registration_fails_the_attempt() {
    bounded("f-c1-ap-control", || {
        for width in [1_u32, 3] {
            let policy = FPolicy::acting("tasks", f_unstaging(false));
            let (wide, result) = f_attempt_run(
                &format!("f-c1-ap-control-w{width}"),
                width,
                policy,
                None,
                |_| {},
                None,
            );
            let progress = result.unwrap_or_else(|error| panic!("width {width}: {error}"));
            assert!(
                matches!(progress, Progress::Finished { .. }),
                "width {width}"
            );
            let events = wide.env.durable_events();
            assert!(
                count(&events, "attempt_finished") == 1
                    && count(&events, "candidate_prepared") == 0,
                "width {width}: the attempt is judged on its review input and settled without a \
                     candidate: {:?}",
                kinds_of(&events)
            );
        }
    });
}

fn f_no_change_responder() -> crate::engine::topology::scaffold::Responder {
    let tasks = f_one_task();
    let base = wide_responder(&tasks, &[]);
    Box::new(move |request: &crate::runner::RunnerRequest| {
        if matches!(
            &request.invocation,
            InvocationId::Attempt {
                role: AttemptRole::Worker,
                ..
            }
        ) {
            return Ok(crate::engine::topology::scaffold::exited(
                0,
                "alpha worked\n".to_owned(),
            ));
        }
        base(request)
    })
}

#[test]
fn an_attempts_failure_assessed_in_a_slot_whose_registration_is_not_whole_is_refused_not_spent() {
    bounded("f-c1-p3", || {
        let mut cases = Vec::new();
        for width in [1_u32, 3] {
            for (label, name, erroring) in [
                ("(a) a failed worker, gitdir removed", "gitdir", true),
                ("(b) a failed worker, index removed", "index", true),
                ("(b) a worker with no change, index removed", "index", false),
            ] {
                let tag = format!("f-c1-p3-w{width}-{name}-{erroring}");
                let policy = FPolicy::acting("tasks", Box::new(|_| {}));
                let calls = Arc::clone(&policy.calls);
                let root_cell = Arc::new(std::sync::Mutex::new(None::<PathBuf>));
                let act: FSharedAct = {
                    let root_cell = Arc::clone(&root_cell);
                    let once = std::sync::OnceLock::<FSharedAct>::new();
                    Arc::new(move || {
                        let act = once.get_or_init(|| {
                            let root = root_cell
                                .lock()
                                .unwrap_or_else(std::sync::PoisonError::into_inner)
                                .clone()
                                .expect("the root is set before the run");
                            f_once_in_the_task_slot(root, name)
                        });
                        act();
                    })
                };
                let configure = {
                    let root_cell = Arc::clone(&root_cell);
                    move |wide: &mut Wide| {
                        *root_cell
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner) =
                            Some(wide.env.fixture.manager.execution_root().to_path_buf());
                        if erroring {
                            wide.env.adapters = Arc::new(
                                crate::engine::topology::scaffold::ScaffoldAdapters::erroring(),
                            );
                        }
                    }
                };
                let responder = (!erroring).then(f_no_change_responder);
                let (wide, result) = f_attempt_run(
                    &tag,
                    width,
                    policy,
                    Some((F_WRITE_TREE, FPhase::After, act)),
                    configure,
                    responder,
                );
                let outcome = match result {
                    Err(UpstrokeError::RegistryRefused { message })
                        if message.contains("the attempt's assessed failure")
                            && message.contains(name)
                            && calls.load(std::sync::atomic::Ordering::SeqCst) == 0 =>
                    {
                        Ok(())
                    }
                    other => Err(format!(
                        "{other:?}, the policy called {} time(s): {:?}",
                        calls.load(std::sync::atomic::Ordering::SeqCst),
                        kinds_of(&wide.env.durable_events())
                    )),
                };
                cases.push((format!("width {width}: {label}"), outcome));
            }
        }
        f_report(&cases);
    });
}

#[test]
fn an_attempts_failure_assessed_in_a_whole_registration_is_spent() {
    bounded("f-c1-p3-control", || {
        for width in [1_u32, 3] {
            let (wide, result) = f_attempt_run(
                &format!("f-c1-p3-control-w{width}"),
                width,
                FPolicy::acting("tasks", Box::new(|_| {})),
                None,
                |wide| {
                    wide.env.adapters =
                        Arc::new(crate::engine::topology::scaffold::ScaffoldAdapters::erroring());
                },
                None,
            );
            let progress = result.unwrap_or_else(|error| panic!("width {width}: {error}"));
            assert!(
                matches!(progress, Progress::Finished { .. }),
                "width {width}"
            );
            let events = wide.env.durable_events();
            assert!(
                events.iter().any(|event| matches!(
                    &event.body,
                    TopologyEventBody::AttemptFinished { data } if data.key == TaskKey(0)
                )),
                "width {width}: the failure is judged and the attempt settled: {:?}",
                kinds_of(&events)
            );
        }
    });
}

struct FReview {
    at_call: u32,
    act: FSharedAct,
    calls: std::sync::atomic::AtomicU32,
}

impl crate::engine::topology::attempt::ReviewPasses for FReview {
    fn run(
        &self,
        cx: &crate::review::ReviewCx<'_>,
        runner: &dyn crate::runner::Runner,
        invocations: &crate::review::ReviewInvocations,
    ) -> Result<crate::review::ReviewOutcome, UpstrokeError> {
        if self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == self.at_call {
            (self.act)();
        }
        <crate::engine::attempt::LegacyReviewPasses as crate::engine::topology::attempt::ReviewPasses>::run(
            &crate::engine::attempt::LegacyReviewPasses,
            cx,
            runner,
            invocations,
        )
    }
}

#[test]
fn a_promotions_path_read_through_a_deleted_index_ends_the_step_without_a_candidate() {
    bounded("f-ans-promo", || {
        let tasks = f_one_task();
        let mut wide = Wide::started_with(
            "f-ans-promo",
            &tasks,
            1,
            WidePlans::default(),
            RecordingRunner::new().answering(wide_responder(&tasks, &[])),
        );
        let root = wide.env.fixture.manager.execution_root().to_path_buf();
        let review = FReview {
            at_call: 0,
            act: f_once_in_the_task_slot(root, "index"),
            calls: std::sync::atomic::AtomicU32::new(0),
        };
        let mut hooks = wide.env.hooks();
        let seams = crate::engine::topology::run::RunSeams {
            reviews: &review,
            ..wide.env.seams()
        };
        let result = f_step_to_error(&mut wide.run, &seams, &mut hooks);
        drop(hooks);
        f_registry_refused(result, &["changed paths", "index"], &wide);
        let events = wide.env.durable_events();
        assert_eq!(
            count(&events, "candidate_prepared"),
            0,
            "no candidate is prepared from a deleted index: {:?}",
            kinds_of(&events)
        );
    });
}

fn f_prune_own_entry(checkout: &std::path::Path) {
    let entry = f_entry_of(checkout);
    let common = entry
        .parent()
        .and_then(std::path::Path::parent)
        .expect("an entry of a registration store")
        .to_path_buf();
    let pointer = checkout.join(".git");
    let bytes = std::fs::read(&pointer).expect("the checkout's .git");
    crate::workspace_manager::fixture::remove_file(&pointer);
    crate::workspace_manager::fixture::git(&common, &["worktree", "prune", "--expire=now"]);
    crate::workspace_manager::fixture::write_file(&pointer, &bytes);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FSubject {
    Attempt,
    Verification,
}

type FGate = Arc<dyn Fn(&std::path::Path) -> i32 + Send + Sync>;

fn f_gate_responder(
    tasks: &[WideTask],
    subject: FSubject,
    gate: FGate,
) -> crate::engine::topology::scaffold::Responder {
    let base = wide_responder(tasks, &[]);
    let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
    Box::new(move |request: &crate::runner::RunnerRequest| {
        let ours = matches!(
            (&request.invocation, subject),
            (
                InvocationId::Attempt {
                    role: AttemptRole::Gate(_),
                    ..
                },
                FSubject::Attempt,
            ) | (
                InvocationId::Sequence {
                    role: crate::runner::invocation::SequenceRole::Gate(_),
                    ..
                },
                FSubject::Verification,
            )
        );
        if ours && !done.swap(true, std::sync::atomic::Ordering::SeqCst) {
            let code = gate(&request.workspace);
            return Ok(crate::engine::topology::scaffold::exited(
                code,
                format!("the stub gate exited {code}\n"),
            ));
        }
        base(request)
    })
}

fn f_exit_of(output: &std::process::Output) -> i32 {
    output.status.code().unwrap_or(128)
}

fn f_construct(name: Option<&'static str>, snapshot: &std::path::Path) {
    match name {
        Some(name) => {
            crate::workspace_manager::fixture::remove_file(&f_entry_of(snapshot).join(name));
        }
        None => f_prune_own_entry(snapshot),
    }
}

fn f_rev_parse_gate(name: Option<&'static str>) -> FGate {
    Arc::new(move |snapshot| {
        f_construct(name, snapshot);
        f_exit_of(&crate::workspace_manager::fixture::git_out(
            snapshot,
            &["rev-parse", "--verify", "HEAD"],
        ))
    })
}

fn f_index_gate() -> FGate {
    Arc::new(|snapshot| {
        f_construct(Some("index"), snapshot);
        f_exit_of(&crate::workspace_manager::fixture::git_out(
            snapshot,
            &["ls-files", "--error-unmatch", "a.txt"],
        ))
    })
}

fn f_forbidden_gate(delete_the_index: bool) -> FGate {
    Arc::new(move |snapshot| {
        if delete_the_index {
            f_construct(Some("index"), snapshot);
        }
        let listed = crate::workspace_manager::fixture::git_out(
            snapshot,
            &["ls-files", "--", "*.forbidden"],
        );
        i32::from(!listed.stdout.is_empty() || !listed.status.success())
    })
}

fn f_blind_gate_in_a_pruned_snapshot() -> FGate {
    Arc::new(|snapshot| {
        f_construct(None, snapshot);
        0
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FAnswer {
    Unavailable,
    Pass,
    MiscountedPass,
}

struct FReviewStub {
    subject: FSubject,
    at_pass: u32,
    prune: bool,
    answer: FAnswer,
}

impl FReviewStub {
    fn is_target(&self, invocations: &crate::review::ReviewInvocations) -> bool {
        match (&invocations.pass, self.subject) {
            (
                InvocationId::Attempt {
                    role: AttemptRole::ReviewPass(pass),
                    ..
                },
                FSubject::Attempt,
            )
            | (
                InvocationId::Sequence {
                    role: crate::runner::invocation::SequenceRole::ReviewPass(pass),
                    ..
                },
                FSubject::Verification,
            ) => *pass == self.at_pass,
            _ => false,
        }
    }
}

impl crate::engine::topology::attempt::ReviewPasses for FReviewStub {
    fn run(
        &self,
        cx: &crate::review::ReviewCx<'_>,
        runner: &dyn crate::runner::Runner,
        invocations: &crate::review::ReviewInvocations,
    ) -> Result<crate::review::ReviewOutcome, UpstrokeError> {
        if !self.is_target(invocations) {
            return <crate::engine::attempt::LegacyReviewPasses as crate::engine::topology::attempt::ReviewPasses>::run(
                &crate::engine::attempt::LegacyReviewPasses,
                cx,
                runner,
                invocations,
            );
        }
        if self.prune {
            f_prune_own_entry(cx.workspace);
        }
        Ok(crate::review::ReviewOutcome {
            result: match self.answer {
                FAnswer::Unavailable => crate::review::ReviewResult::Unavailable {
                    status: crate::ir::OutcomeStatus::AgentError,
                    detail: "the stub reviewer is unavailable".to_owned(),
                    limit_message: None,
                },
                FAnswer::Pass | FAnswer::MiscountedPass => {
                    crate::review::ReviewResult::Judged(crate::ir::Verdict {
                        pass: true,
                        reasons: Vec::new(),
                        required_changes: Vec::new(),
                        needs_human: false,
                    })
                }
            },
            cost_usd: Some(0.25),
            invocations: u32::from(self.answer == FAnswer::MiscountedPass),
            transcript: PathBuf::new(),
            never_started: false,
        })
    }
}

fn f_drive_subject(
    tag: &str,
    tasks: &[WideTask],
    subject: FSubject,
    width: u32,
    responder: crate::engine::topology::scaffold::Responder,
    plans: WidePlans,
    reviews: Option<Arc<FReviewStub>>,
) -> (Wide, Result<Progress, UpstrokeError>) {
    let legacy = crate::engine::attempt::LegacyReviewPasses;
    match (subject, width) {
        (FSubject::Attempt, 1) => {
            let mut wide = Wide::started_with(
                tag,
                tasks,
                1,
                plans,
                RecordingRunner::new().answering(responder),
            );
            let mut hooks = wide.env.hooks();
            let seams = crate::engine::topology::run::RunSeams {
                reviews: match &reviews {
                    Some(stub) => &**stub,
                    None => &legacy,
                },
                ..wide.env.seams()
            };
            let result = f_step_to_error(&mut wide.run, &seams, &mut hooks);
            drop(hooks);
            (wide, result)
        }
        (FSubject::Attempt, _) => {
            let mut wide = Wide::started_with(
                tag,
                tasks,
                width,
                plans,
                RecordingRunner::new().answering(responder),
            );
            let mut hooks = wide.env.hooks();
            let mut pipelines = wide.env.pipelines();
            if let Some(stub) = reviews {
                pipelines.reviews = stub;
            }
            let result = wide
                .run
                .run_concurrently(&wide.env.seams(), &pipelines, &mut hooks, None);
            (wide, result)
        }
        (FSubject::Verification, 1) => {
            let mut wide = Wide::durable(tag, tasks, 3, plans, holding(tasks, &[]));
            let mut first = FHooks::over(&wide);
            first.effects.fail_at = Some((F_PICK, FPhase::Before));
            let pipelines = wide.env.pipelines();
            let held = Arc::clone(&wide.env.runner);
            let mut scheduler =
                Scheduler::scripted(&held, Box::new(alpha_waits_for_gammas_integration));
            let ended = wide.run.run_concurrently(
                &wide.env.seams(),
                &pipelines,
                &mut first,
                Some(&mut scheduler),
            );
            drop(scheduler);
            drop(first);
            assert!(ended.is_err(), "the first command ends at gamma's pick");
            let (_, mut wide) = wide
                .resume(
                    "inc-2",
                    RecordingRunner::new().answering(responder),
                    crate::engine::topology::select::Ceiling::unlimited(),
                )
                .expect("the run resumes with gamma queued");
            let mut hooks = wide.env.hooks();
            let seams = crate::engine::topology::run::RunSeams {
                reviews: match &reviews {
                    Some(stub) => &**stub,
                    None => &legacy,
                },
                ..wide.env.seams()
            };
            let result = f_step_to_error(&mut wide.run, &seams, &mut hooks);
            drop(hooks);
            (wide, result)
        }
        (FSubject::Verification, _) => {
            let runner = RecordingRunner::new().answering(responder);
            runner.hold();
            let mut wide = Wide::durable(tag, tasks, 3, plans, runner);
            let mut hooks = wide.env.hooks();
            let mut pipelines = wide.env.pipelines();
            if let Some(stub) = reviews {
                pipelines.reviews = stub;
            }
            let held = Arc::clone(&wide.env.runner);
            let mut scheduler =
                Scheduler::scripted(&held, Box::new(alpha_waits_for_gammas_integration));
            let result = wide.run.run_concurrently(
                &wide.env.seams(),
                &pipelines,
                &mut hooks,
                Some(&mut scheduler),
            );
            drop(scheduler);
            drop(hooks);
            (wide, result)
        }
    }
}

fn f_tasks_of(subject: FSubject, writes: &str) -> Vec<WideTask> {
    match subject {
        FSubject::Attempt => vec![WideTask::hinted("alpha", &["src/alpha/"], writes)],
        FSubject::Verification => vec![
            WideTask::independent("beta"),
            WideTask::independent("alpha"),
            WideTask::hinted("gamma", &["src/gamma/"], writes),
        ],
    }
}

fn f_refused_case(
    label: String,
    wide: &Wide,
    result: Result<Progress, UpstrokeError>,
    words: &[&str],
) -> (String, Result<(), String>) {
    let outcome = match result {
        Err(UpstrokeError::RegistryRefused { message })
            if words.iter().all(|word| message.contains(word)) =>
        {
            Ok(())
        }
        other => Err(format!(
            "{other:?}: {:?}",
            kinds_of(&wide.env.durable_events())
        )),
    };
    (label, outcome)
}

fn f_bounded_case(
    what: String,
    case: impl FnOnce() -> (String, Result<(), String>) + Send + 'static,
) -> (String, Result<(), String>) {
    let (sent, outcome) = std::sync::mpsc::channel();
    bounded(&what, move || {
        let _ = sent.send(case());
    });
    outcome
        .recv()
        .expect("a case that ended within its bound sent its outcome")
}

const F_MATRIX: [(FSubject, u32); 4] = [
    (FSubject::Attempt, 1),
    (FSubject::Attempt, 3),
    (FSubject::Verification, 1),
    (FSubject::Verification, 3),
];

#[test]
fn a_gate_verdict_read_in_a_snapshot_whose_registration_is_not_whole_is_refused() {
    let mut cases = Vec::new();
    for (subject, width) in F_MATRIX {
        for (label, gate) in [
            ("pruned whole", f_rev_parse_gate(None)),
            ("HEAD removed", f_rev_parse_gate(Some("HEAD"))),
            ("commondir removed", f_rev_parse_gate(Some("commondir"))),
            ("index removed, an index gate", f_index_gate()),
        ] {
            let case = format!("{subject:?} width {width}: {label}");
            cases.push(f_bounded_case(format!("f-c1-p2-gate {case}"), move || {
                let tasks = f_tasks_of(subject, "src/work.txt");
                let tag = format!("f-p2-gate-{subject:?}-w{width}-{}", label.len());
                let responder = f_gate_responder(&tasks, subject, gate);
                let (wide, result) = f_drive_subject(
                    &tag,
                    &tasks,
                    subject,
                    width,
                    responder,
                    WidePlans::default(),
                    None,
                );
                f_refused_case(case, &wide, result, &["the gates' verdicts"])
            }));
        }
    }
    f_report(&cases);
}

#[test]
fn a_genuinely_failing_gate_in_a_whole_snapshot_is_judged() {
    bounded("f-c1-p2-gate-control", || {
        for (subject, width) in F_MATRIX {
            let tasks = f_tasks_of(subject, "src/work.txt");
            let responder = f_gate_responder(&tasks, subject, Arc::new(|_| 1));
            let (wide, result) = f_drive_subject(
                &format!("f-p2-gate-control-{subject:?}-w{width}"),
                &tasks,
                subject,
                width,
                responder,
                WidePlans::default(),
                None,
            );
            let progress =
                result.unwrap_or_else(|error| panic!("{subject:?} width {width}: {error}"));
            assert!(matches!(progress, Progress::Finished { .. }));
            let events = wide.env.durable_events();
            match subject {
                FSubject::Attempt => assert!(
                    count(&events, "attempt_started") >= 2,
                    "width {width}: the failing gate spends the attempt: {:?}",
                    kinds_of(&events)
                ),
                FSubject::Verification => assert!(
                    events
                        .iter()
                        .any(|event| merge_rejected_of_gamma(&event.body)),
                    "width {width}: the failing gate rejects gamma: {:?}",
                    kinds_of(&events)
                ),
            }
        }
    });
}

#[test]
fn a_gate_that_passes_through_a_deleted_index_is_refused_not_published() {
    bounded("f-c1-p2-pass", || {
        let mut cases = Vec::new();
        for (subject, width) in F_MATRIX {
            let tasks = f_tasks_of(subject, "x.forbidden");
            let responder = f_gate_responder(&tasks, subject, f_forbidden_gate(true));
            let (wide, result) = f_drive_subject(
                &format!("f-p2-pass-{subject:?}-w{width}"),
                &tasks,
                subject,
                width,
                responder,
                WidePlans::default(),
                None,
            );
            cases.push(f_refused_case(
                format!("{subject:?} width {width}"),
                &wide,
                result,
                &["the gates' verdicts", "index"],
            ));
        }
        f_report(&cases);
    });
}

#[test]
fn a_gate_reading_a_whole_index_fails_on_the_forbidden_file() {
    bounded("f-c1-p2-pass-control", || {
        for (subject, width) in F_MATRIX {
            let tasks = f_tasks_of(subject, "x.forbidden");
            let responder = f_gate_responder(&tasks, subject, f_forbidden_gate(false));
            let (wide, result) = f_drive_subject(
                &format!("f-p2-pass-control-{subject:?}-w{width}"),
                &tasks,
                subject,
                width,
                responder,
                WidePlans::default(),
                None,
            );
            let progress =
                result.unwrap_or_else(|error| panic!("{subject:?} width {width}: {error}"));
            assert!(matches!(progress, Progress::Finished { .. }));
            let events = wide.env.durable_events();
            match subject {
                FSubject::Attempt => assert!(
                    count(&events, "attempt_started") >= 2,
                    "width {width}: the gate fails on the tracked file: {:?}",
                    kinds_of(&events)
                ),
                FSubject::Verification => assert!(
                    events
                        .iter()
                        .any(|event| merge_rejected_of_gamma(&event.body)),
                    "width {width}: the gate fails on the tracked file: {:?}",
                    kinds_of(&events)
                ),
            }
        }
    });
}

#[test]
fn a_passing_gate_that_reads_no_git_state_in_a_pruned_snapshot_is_refused() {
    bounded("f-c1-p2-pass-liveness", || {
        let mut cases = Vec::new();
        for (subject, width) in F_MATRIX {
            let tasks = f_tasks_of(subject, "src/work.txt");
            let responder = f_gate_responder(&tasks, subject, f_blind_gate_in_a_pruned_snapshot());
            let (wide, result) = f_drive_subject(
                &format!("f-p2-pass-liveness-{subject:?}-w{width}"),
                &tasks,
                subject,
                width,
                responder,
                WidePlans::default(),
                None,
            );
            cases.push(f_refused_case(
                format!("{subject:?} width {width}"),
                &wide,
                result,
                &["the gates' verdicts"],
            ));
        }
        f_report(&cases);
    });
}

fn f_gate_of(subject: FSubject, invocation: &InvocationId) -> bool {
    matches!(
        (invocation, subject),
        (
            InvocationId::Attempt {
                role: AttemptRole::Gate(_),
                ..
            },
            FSubject::Attempt,
        ) | (
            InvocationId::Sequence {
                role: crate::runner::invocation::SequenceRole::Gate(_),
                ..
            },
            FSubject::Verification,
        )
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FLaunch {
    HeadRemoved,
    PrunedWhole,
    Intact,
    CreateFailing,
    Lost,
    Cancelled,
    Unresolved,
}

#[derive(Debug, Clone)]
struct FLaunched {
    failed: Option<(crate::error::ProcessFate, bool)>,
    message: String,
    head: PathBuf,
    head_after: bool,
}

type FLaunches = Arc<std::sync::Mutex<Vec<FLaunched>>>;

fn f_launch(
    request: &crate::runner::RunnerRequest,
    launch: FLaunch,
) -> Result<crate::agent::ProcessOutput, crate::runner::RunnerError> {
    match launch {
        FLaunch::HeadRemoved | FLaunch::Lost | FLaunch::Cancelled | FLaunch::Unresolved => {
            crate::workspace_manager::fixture::remove_file(
                &f_entry_of(&request.workspace).join("HEAD"),
            );
        }
        FLaunch::PrunedWhole => f_prune_own_entry(&request.workspace),
        FLaunch::Intact | FLaunch::CreateFailing => {}
    }
    let stub = |what: &str| UpstrokeError::Refused {
        message: format!("the stub gate's run {what}"),
    };
    match launch {
        FLaunch::Lost => {
            return Err(crate::runner::RunnerError::gone(
                &request.invocation,
                stub("lost its process after it started"),
            ));
        }
        FLaunch::Cancelled => {
            return Err(crate::runner::RunnerError::cancelled(
                &request.invocation,
                crate::error::ProcessFate::NeverStarted,
            ));
        }
        FLaunch::Unresolved => {
            return Err(crate::runner::RunnerError::unresolved(
                &request.invocation,
                stub("could not establish that its process ended"),
            ));
        }
        FLaunch::HeadRemoved | FLaunch::PrunedWhole | FLaunch::Intact | FLaunch::CreateFailing => {}
    }
    let context = crate::workspace_manager::fixture::Fixture::new("f-container-gate");
    let runtime = crate::engine::topology::scaffold::container_host()
        .starting(crate::engine::topology::scaffold::exiting());
    if launch == FLaunch::CreateFailing {
        runtime.set_failing(crate::runner::container::runtime::RuntimeOp::Create);
    }
    let runner = crate::engine::topology::scaffold::container_runner(
        crate::runner::container::exec::RunIdentity {
            private_root: context.private.clone(),
            run_id: crate::workspace_manager::fixture::RUN_ID.to_owned(),
            run_dir: context.root.join("container-run"),
            incarnation: "f_container_gate".to_owned(),
            repo_key: "f_container_gate".to_owned(),
        },
        &context.base,
        Box::new(runtime),
        Duration::from_millis(1),
    )
    .with_view(Box::new(crate::runner::container::view::RoleGitView::new(
        crate::runner::container::runtime::ContainerTrace::off(),
    )));
    runner.run_blocking(request)
}

fn f_launching_gate_responder(
    tasks: &[WideTask],
    subject: FSubject,
    launch: FLaunch,
    launched: FLaunches,
) -> crate::engine::topology::scaffold::Responder {
    let base = wide_responder(tasks, &[]);
    Box::new(move |request: &crate::runner::RunnerRequest| {
        if !f_gate_of(subject, &request.invocation) {
            return base(request);
        }
        let head = f_entry_of(&request.workspace).join("HEAD");
        let result = f_launch(request, launch);
        launched
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(FLaunched {
                failed: result
                    .as_ref()
                    .err()
                    .map(|error| (error.fate, error.is_cancelled())),
                message: result
                    .as_ref()
                    .err()
                    .map(ToString::to_string)
                    .unwrap_or_default(),
                head_after: head.exists(),
                head,
            });
        result
    })
}

fn f_first_launch(launched: &FLaunches) -> Result<FLaunched, String> {
    launched
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .first()
        .cloned()
        .ok_or_else(|| "no gate of the subject was launched".to_owned())
}

fn f_names_path(message: &str, path: &std::path::Path) -> bool {
    let spelled = |text: &str| text.replace('\\', "/");
    spelled(message).contains(&spelled(&path.display().to_string()))
}

fn f_launch_witnessed(launched: &FLaunches, launch: FLaunch) -> Result<(), String> {
    let first = f_first_launch(launched)?;
    let fate = match launch {
        FLaunch::Lost => crate::error::ProcessFate::Gone,
        _ => crate::error::ProcessFate::NeverStarted,
    };
    let names_head = launch == FLaunch::Lost || f_names_path(&first.message, &first.head);
    if first.failed == Some((fate, false)) && names_head && !first.head_after {
        Ok(())
    } else {
        Err(format!(
            "the launch was not the failure the construction makes ({fate:?}, HEAD named \
                 and still absent): {first:?}"
        ))
    }
}

const F_WINGUEST_LAUNCHES: [(&str, FLaunch, &str, &str); 8] = [
    (
        "Attempt width 1",
        FLaunch::HeadRemoved,
        r"the Runner could not complete `k0.g0.a1.gate0.o0` (no process of it was started): failed to read C:\Users\Administrator\AppData\Local\Temp\upstroke-f-p2-launch-Attempt-w1-HeadRemoved-WT5SPRCJHR\repo\.git\worktrees\k0-g0-a1-gates_WG9T4H5MXCSP\HEAD: The system cannot find the file specified. (os error 2)",
        r"C:/Users/Administrator/AppData/Local/Temp/upstroke-f-p2-launch-Attempt-w1-HeadRemoved-WT5SPRCJHR/repo/.git/worktrees/k0-g0-a1-gates_WG9T4H5MXCSP\HEAD",
    ),
    (
        "Attempt width 1",
        FLaunch::PrunedWhole,
        r"the Runner could not complete `k0.g0.a1.gate0.o0` (no process of it was started): failed to read C:\Users\Administrator\AppData\Local\Temp\upstroke-f-p2-launch-Attempt-w1-PrunedWhole-7JXKYQN9EN\repo\.git\worktrees\k0-g0-a1-gates_WG9T4H5MXCSP\HEAD: The system cannot find the path specified. (os error 3)",
        r"C:/Users/Administrator/AppData/Local/Temp/upstroke-f-p2-launch-Attempt-w1-PrunedWhole-7JXKYQN9EN/repo/.git/worktrees/k0-g0-a1-gates_WG9T4H5MXCSP\HEAD",
    ),
    (
        "Attempt width 3",
        FLaunch::HeadRemoved,
        r"the Runner could not complete `k0.g0.a1.gate0.o0` (no process of it was started): failed to read C:\Users\Administrator\AppData\Local\Temp\upstroke-f-p2-launch-Attempt-w3-HeadRemoved-QTTBDR2DRH\repo\.git\worktrees\k0-g0-a1-gates_WG9T4H5MXCSP\HEAD: The system cannot find the file specified. (os error 2)",
        r"C:/Users/Administrator/AppData/Local/Temp/upstroke-f-p2-launch-Attempt-w3-HeadRemoved-QTTBDR2DRH/repo/.git/worktrees/k0-g0-a1-gates_WG9T4H5MXCSP\HEAD",
    ),
    (
        "Attempt width 3",
        FLaunch::PrunedWhole,
        r"the Runner could not complete `k0.g0.a1.gate0.o0` (no process of it was started): failed to read C:\Users\Administrator\AppData\Local\Temp\upstroke-f-p2-launch-Attempt-w3-PrunedWhole-ENJM7WFVHB\repo\.git\worktrees\k0-g0-a1-gates_WG9T4H5MXCSP\HEAD: The system cannot find the path specified. (os error 3)",
        r"C:/Users/Administrator/AppData/Local/Temp/upstroke-f-p2-launch-Attempt-w3-PrunedWhole-ENJM7WFVHB/repo/.git/worktrees/k0-g0-a1-gates_WG9T4H5MXCSP\HEAD",
    ),
    (
        "Verification width 1",
        FLaunch::HeadRemoved,
        r"the Runner could not complete `s1.gate0.o0` (no process of it was started): failed to read C:\Users\Administrator\AppData\Local\Temp\upstroke-f-p2-launch-Verification-w1-HeadRemoved-X183YXRX9H\repo\.git\worktrees\s1-integration_M8TPQSPRW3HB\HEAD: The system cannot find the file specified. (os error 2)",
        r"C:/Users/Administrator/AppData/Local/Temp/upstroke-f-p2-launch-Verification-w1-HeadRemoved-X183YXRX9H/repo/.git/worktrees/s1-integration_M8TPQSPRW3HB\HEAD",
    ),
    (
        "Verification width 1",
        FLaunch::PrunedWhole,
        r"the Runner could not complete `s1.gate0.o0` (no process of it was started): failed to read C:\Users\Administrator\AppData\Local\Temp\upstroke-f-p2-launch-Verification-w1-PrunedWhole-XJMT3Z76HF\repo\.git\worktrees\s1-integration_M8TPQSPRW3HB\HEAD: The system cannot find the path specified. (os error 3)",
        r"C:/Users/Administrator/AppData/Local/Temp/upstroke-f-p2-launch-Verification-w1-PrunedWhole-XJMT3Z76HF/repo/.git/worktrees/s1-integration_M8TPQSPRW3HB\HEAD",
    ),
    (
        "Verification width 3",
        FLaunch::HeadRemoved,
        r"the Runner could not complete `s1.gate0.o0` (no process of it was started): failed to read C:\Users\Administrator\AppData\Local\Temp\upstroke-f-p2-launch-Verification-w3-HeadRemoved-CEVDNZ555A\repo\.git\worktrees\s1-integration_WG9T4H5MXCSP\HEAD: The system cannot find the file specified. (os error 2)",
        r"C:/Users/Administrator/AppData/Local/Temp/upstroke-f-p2-launch-Verification-w3-HeadRemoved-CEVDNZ555A/repo/.git/worktrees/s1-integration_WG9T4H5MXCSP\HEAD",
    ),
    (
        "Verification width 3",
        FLaunch::PrunedWhole,
        r"the Runner could not complete `s1.gate0.o0` (no process of it was started): failed to read C:\Users\Administrator\AppData\Local\Temp\upstroke-f-p2-launch-Verification-w3-PrunedWhole-7YF46G19N6\repo\.git\worktrees\s1-integration_WG9T4H5MXCSP\HEAD: The system cannot find the path specified. (os error 3)",
        r"C:/Users/Administrator/AppData/Local/Temp/upstroke-f-p2-launch-Verification-w3-PrunedWhole-7YF46G19N6/repo/.git/worktrees/s1-integration_WG9T4H5MXCSP\HEAD",
    ),
];

const F_LINUX_LAUNCHES: [(&str, FLaunch, &str, &str); 2] = [
    (
        "Attempt width 1",
        FLaunch::HeadRemoved,
        "the Runner could not complete `k0.g0.a1.gate0.o0` (no process of it was started): failed to read /tmp/upstroke-f-p2-launch-Attempt-w1-HeadRemoved-378Z7T187T/repo/.git/worktrees/k0-g0-a1-gates_WG9T4H5MXCSP/HEAD: No such file or directory (os error 2)",
        "/tmp/upstroke-f-p2-launch-Attempt-w1-HeadRemoved-378Z7T187T/repo/.git/worktrees/k0-g0-a1-gates_WG9T4H5MXCSP/HEAD",
    ),
    (
        "Attempt width 1",
        FLaunch::PrunedWhole,
        "the Runner could not complete `k0.g0.a1.gate0.o0` (no process of it was started): failed to read /tmp/upstroke-f-p2-launch-Attempt-w1-PrunedWhole-7M7TD3C7JZ/repo/.git/worktrees/k0-g0-a1-gates_WG9T4H5MXCSP/HEAD: No such file or directory (os error 2)",
        "/tmp/upstroke-f-p2-launch-Attempt-w1-PrunedWhole-7M7TD3C7JZ/repo/.git/worktrees/k0-g0-a1-gates_WG9T4H5MXCSP/HEAD",
    ),
];

const F_LINUX_LOST_LAUNCH: (&str, &str) = (
    "the Runner could not complete `k0.g0.a1.gate0.o0` (its process is gone): the stub gate's run lost its process after it started",
    "/tmp/upstroke-f-p2-launch-Attempt-w1-Lost-TGWCYTJX30/repo/.git/worktrees/k0-g0-a1-gates_WG9T4H5MXCSP/HEAD",
);

fn f_premise_of(
    launch: FLaunch,
    failed: Option<(crate::error::ProcessFate, bool)>,
    message: &str,
    head: &str,
    head_after: bool,
) -> Result<(), String> {
    let launched = FLaunches::default();
    launched
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .push(FLaunched {
            failed,
            message: message.to_owned(),
            head: PathBuf::from(head),
            head_after,
        });
    f_launch_witnessed(&launched, launch)
}

#[test]
fn the_launch_premise_names_the_deleted_head_in_either_separator_spelling_and_nothing_else() {
    let never = Some((crate::error::ProcessFate::NeverStarted, false));
    let gone = Some((crate::error::ProcessFate::Gone, false));
    let mut cases = Vec::new();
    for (spelling, launches, raw) in [
        ("winguest", F_WINGUEST_LAUNCHES.as_slice(), false),
        ("linux", F_LINUX_LAUNCHES.as_slice(), true),
    ] {
        for &(cell, launch, message, head) in launches {
            cases.push((
                format!("{spelling} {cell}: {launch:?}, as printed"),
                match f_premise_of(launch, never, message, head, false) {
                    Ok(()) if message.contains(head) == raw => Ok(()),
                    other => Err(format!(
                        "{other:?}; the raw comparison finds the head: {}",
                        message.contains(head)
                    )),
                },
            ));
        }
    }
    let [winguest, winguest_elsewhere, ..] = F_WINGUEST_LAUNCHES;
    let [linux, linux_elsewhere] = F_LINUX_LAUNCHES;
    for (spelling, separator, (cell, launch, message, head), (_, _, elsewhere, _)) in [
        ("winguest", '\\', winguest, winguest_elsewhere),
        ("linux", '/', linux, linux_elsewhere),
    ] {
        let swapped = |from: &str, to: &str| {
            let text = message.replacen(from, to, 1);
            assert_ne!(text, message, "{spelling}: {from:?} is in the message");
            text
        };
        let other_entry = swapped("k0-g0-a1-gates", "k1-g0-a1-gates");
        let gitdir = swapped(
            &format!("{separator}HEAD: "),
            &format!("{separator}gitdir: "),
        );
        let directory = swapped(&format!("{separator}HEAD: "), ": ");
        let cut = message
            .split_once("failed to read ")
            .map(|(before, _)| before.to_owned())
            .expect("the message reads a path");
        for (what, judged) in [
            (
                "another snapshot's HEAD",
                f_premise_of(launch, never, elsewhere, head, false),
            ),
            (
                "another entry's HEAD in the same store",
                f_premise_of(launch, never, &other_entry, head, false),
            ),
            (
                "the entry's gitdir instead of HEAD",
                f_premise_of(launch, never, &gitdir, head, false),
            ),
            (
                "the entry directory alone",
                f_premise_of(launch, never, &directory, head, false),
            ),
            (
                "no path: the error cut before it",
                f_premise_of(launch, never, &cut, head, false),
            ),
            (
                "no path: no error text",
                f_premise_of(launch, never, "", head, false),
            ),
            (
                "HEAD there after the launch",
                f_premise_of(launch, never, message, head, true),
            ),
            (
                "a cancellation",
                f_premise_of(
                    launch,
                    Some((crate::error::ProcessFate::NeverStarted, true)),
                    message,
                    head,
                    false,
                ),
            ),
            (
                "no failure",
                f_premise_of(launch, None, message, head, false),
            ),
            (
                "the Lost fate under the container's construction",
                f_premise_of(launch, gone, message, head, false),
            ),
            (
                "the container's fate under the Lost construction",
                f_premise_of(FLaunch::Lost, never, message, head, false),
            ),
        ] {
            cases.push((
                format!("{spelling} {cell}: {launch:?}, refused: {what}"),
                match judged {
                    Err(_) => Ok(()),
                    Ok(()) => Err("accepted".to_owned()),
                },
            ));
        }
    }
    let (lost_message, lost_head) = F_LINUX_LOST_LAUNCH;
    cases.push((
        "linux Attempt width 1: Lost, kept apart: its stub error names no path".to_owned(),
        f_premise_of(FLaunch::Lost, gone, lost_message, lost_head, false),
    ));
    f_report(&cases);
}

#[test]
fn a_gate_run_failing_in_a_snapshot_whose_registration_is_not_whole_is_refused() {
    let mut cases = Vec::new();
    for (subject, width) in F_MATRIX {
        for launch in [FLaunch::HeadRemoved, FLaunch::PrunedWhole, FLaunch::Lost] {
            let label = format!("{subject:?} width {width}: {launch:?}");
            cases.push(f_bounded_case(
                format!("f-c1-p2-launch {label}"),
                move || {
                    let tasks = f_tasks_of(subject, "src/work.txt");
                    let launched = FLaunches::default();
                    let responder =
                        f_launching_gate_responder(&tasks, subject, launch, Arc::clone(&launched));
                    let (wide, result) = f_drive_subject(
                        &format!("f-p2-launch-{subject:?}-w{width}-{launch:?}"),
                        &tasks,
                        subject,
                        width,
                        responder,
                        WidePlans::default(),
                        None,
                    );
                    match f_launch_witnessed(&launched, launch) {
                        Ok(()) => {
                            f_refused_case(label, &wide, result, &["'s run, which failed", "HEAD"])
                        }
                        Err(why) => (label, Err(why)),
                    }
                },
            ));
        }
    }
    f_report(&cases);
}

#[test]
fn a_container_gate_launched_in_a_whole_snapshot_is_judged() {
    bounded("f-c1-p2-launch-control", || {
        for (subject, width) in F_MATRIX {
            let tasks = f_tasks_of(subject, "src/work.txt");
            let launched = FLaunches::default();
            let responder =
                f_launching_gate_responder(&tasks, subject, FLaunch::Intact, Arc::clone(&launched));
            let (wide, result) = f_drive_subject(
                &format!("f-p2-launch-control-{subject:?}-w{width}"),
                &tasks,
                subject,
                width,
                responder,
                WidePlans::default(),
                None,
            );
            let progress =
                result.unwrap_or_else(|error| panic!("{subject:?} width {width}: {error}"));
            assert_eq!(outcome_of(&progress), RunOutcome::Complete);
            let launched = launched
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            assert!(
                !launched.is_empty() && launched.iter().all(|one| one.failed.is_none()),
                "{subject:?} width {width}: the real Git-view reader materialized every \
                     gate's view: {launched:?}"
            );
            let events = wide.env.durable_events();
            assert_eq!(
                count(&events, "merge_verification_unavailable"),
                0,
                "{subject:?} width {width}: no outage: {:?}",
                kinds_of(&events)
            );
        }
    });
}

#[test]
fn a_gate_launch_failing_in_a_whole_snapshot_is_the_outage_it_was() {
    bounded("f-c1-p2-launch-outage", || {
        for (subject, width) in F_MATRIX {
            let tasks = f_tasks_of(subject, "src/work.txt");
            let launched = FLaunches::default();
            let responder = f_launching_gate_responder(
                &tasks,
                subject,
                FLaunch::CreateFailing,
                Arc::clone(&launched),
            );
            let (wide, result) = f_drive_subject(
                &format!("f-p2-launch-outage-{subject:?}-w{width}"),
                &tasks,
                subject,
                width,
                responder,
                WidePlans::default(),
                None,
            );
            let first = f_first_launch(&launched)
                .unwrap_or_else(|why| panic!("{subject:?} width {width}: {why}"));
            assert!(
                first.failed == Some((crate::error::ProcessFate::NeverStarted, false))
                    && first.head_after,
                "{subject:?} width {width}: the create fails in a whole snapshot: {first:?}"
            );
            let events = wide.env.durable_events();
            match subject {
                FSubject::Attempt => assert!(
                    matches!(
                        result,
                        Err(UpstrokeError::Runner {
                            fate: crate::error::ProcessFate::NeverStarted,
                            ..
                        })
                    ),
                    "width {width}: the Runner's error ends the command as it did: {result:?}"
                ),
                FSubject::Verification => {
                    let progress = result.unwrap_or_else(|error| panic!("width {width}: {error}"));
                    assert!(matches!(progress, Progress::Finished { .. }));
                    assert!(
                        count(&events, "merge_verification_unavailable") >= 1,
                        "width {width}: the failed launch settles an outage: {:?}",
                        kinds_of(&events)
                    );
                }
            }
        }
    });
}

#[test]
fn a_cancelled_or_unresolved_gate_run_ends_the_command_as_it_did_whatever_the_registration() {
    bounded("f-c1-p2-launch-fates", || {
        let mut cases = Vec::new();
        for (subject, width) in F_MATRIX {
            for launch in [FLaunch::Cancelled, FLaunch::Unresolved] {
                let tasks = f_tasks_of(subject, "src/work.txt");
                let launched = FLaunches::default();
                let responder =
                    f_launching_gate_responder(&tasks, subject, launch, Arc::clone(&launched));
                let (wide, result) = f_drive_subject(
                    &format!("f-p2-launch-fates-{subject:?}-w{width}-{launch:?}"),
                    &tasks,
                    subject,
                    width,
                    responder,
                    WidePlans::default(),
                    None,
                );
                let as_it_did = match (launch, &result) {
                    (FLaunch::Cancelled, Err(UpstrokeError::Runner { source, .. })) => {
                        source.to_string().contains("cancelled")
                    }
                    (
                        FLaunch::Unresolved,
                        Err(UpstrokeError::Runner {
                            fate: crate::error::ProcessFate::Unresolved,
                            ..
                        }),
                    ) => width == 1,
                    (FLaunch::Unresolved, Err(UpstrokeError::Refused { message })) => {
                        width != 1 && message.contains("ended with its process unresolved")
                    }
                    _ => false,
                };
                let witnessed = f_first_launch(&launched)
                    .map(|first| first.failed.is_some() && !first.head_after)
                    .unwrap_or(false);
                cases.push((
                    format!("{subject:?} width {width}: {launch:?}"),
                    if as_it_did && witnessed {
                        Ok(())
                    } else {
                        Err(format!(
                            "{result:?}; the launch removed HEAD and failed: {witnessed}; {:?}",
                            kinds_of(&wide.env.durable_events())
                        ))
                    },
                ));
            }
        }
        f_report(&cases);
    });
}

fn f_two_reviewers() -> WidePlans {
    WidePlans {
        reviewers: 2,
        verify_reviewers: 2,
        ..WidePlans::default()
    }
}

#[test]
fn a_review_verdict_read_in_a_snapshot_whose_registration_is_not_whole_is_refused() {
    let mut cases = Vec::new();
    for (subject, width) in F_MATRIX {
        for (label, at_pass, answer) in [
            ("pass 0 unavailable", 0, FAnswer::Unavailable),
            (
                "pass 1 unavailable after pass 0 passed",
                1,
                FAnswer::Unavailable,
            ),
            ("pass 0 passing", 0, FAnswer::Pass),
        ] {
            let case = format!("{subject:?} width {width}: {label}");
            cases.push(f_bounded_case(
                format!("f-c1-p2-review {case}"),
                move || {
                    let tasks = f_tasks_of(subject, "src/work.txt");
                    let stub = Arc::new(FReviewStub {
                        subject,
                        at_pass,
                        prune: true,
                        answer,
                    });
                    let (wide, result) = f_drive_subject(
                        &format!("f-p2-review-{subject:?}-w{width}-{at_pass}-{answer:?}"),
                        &tasks,
                        subject,
                        width,
                        wide_responder(&tasks, &[]),
                        f_two_reviewers(),
                        Some(stub),
                    );
                    f_refused_case(
                        case,
                        &wide,
                        result,
                        &[&format!("the verdict of review pass {at_pass}")],
                    )
                },
            ));
        }
    }
    f_report(&cases);
}

#[test]
fn a_review_verdict_read_in_a_whole_snapshot_is_judged() {
    bounded("f-c1-p2-review-control", || {
        for (subject, width) in F_MATRIX {
            let tasks = f_tasks_of(subject, "src/work.txt");
            let stub = Arc::new(FReviewStub {
                subject,
                at_pass: 0,
                prune: false,
                answer: FAnswer::Unavailable,
            });
            let (wide, result) = f_drive_subject(
                &format!("f-p2-review-control-{subject:?}-w{width}"),
                &tasks,
                subject,
                width,
                wide_responder(&tasks, &[]),
                f_two_reviewers(),
                Some(stub),
            );
            let progress =
                result.unwrap_or_else(|error| panic!("{subject:?} width {width}: {error}"));
            assert!(matches!(progress, Progress::Finished { .. }));
            let events = wide.env.durable_events();
            match subject {
                FSubject::Attempt => assert!(
                    count(&events, "attempt_finished") >= 1,
                    "width {width}: the unavailable review is judged: {:?}",
                    kinds_of(&events)
                ),
                FSubject::Verification => assert!(
                    f_unavailable_of_gamma(&events),
                    "width {width}: the unavailable review settles an outage: {:?}",
                    kinds_of(&events)
                ),
            }
        }
    });
}

#[test]
fn a_review_whose_process_count_disagrees_keeps_that_refusal_when_its_snapshot_is_not_whole() {
    bounded("f-c1-p2-order", || {
        let tasks = f_tasks_of(FSubject::Attempt, "src/work.txt");
        let stub = Arc::new(FReviewStub {
            subject: FSubject::Attempt,
            at_pass: 0,
            prune: true,
            answer: FAnswer::MiscountedPass,
        });
        let (wide, result) = f_drive_subject(
            "f-p2-order",
            &tasks,
            FSubject::Attempt,
            1,
            wide_responder(&tasks, &[]),
            WidePlans::default(),
            Some(stub),
        );
        match result {
            Err(UpstrokeError::Refused { message }) => {
                assert!(message.contains("process(es)"), "{message}");
            }
            other => panic!(
                "the process-count refusal, which precedes placement 2: {other:?}: {:?}",
                kinds_of(&wide.env.durable_events())
            ),
        }
    });
}

#[test]
fn a_review_input_read_over_a_lost_shared_index_settles_an_outage() {
    bounded("f-residual-i", || {
        let tasks = f_stale_tasks();
        let policy = FPolicy::acting(
            "merge",
            Box::new(|checkout| {
                crate::workspace_manager::fixture::git(
                    checkout,
                    &["update-index", "--split-index"],
                );
                let entry = f_entry_of(checkout);
                let shared: Vec<PathBuf> = std::fs::read_dir(&entry)
                    .expect("the entry")
                    .map(|found| found.expect("an entry").path())
                    .filter(|path| {
                        path.file_name()
                            .and_then(std::ffi::OsStr::to_str)
                            .is_some_and(|name| name.starts_with("sharedindex."))
                    })
                    .collect();
                assert!(!shared.is_empty(), "the split index wrote its shared file");
                for path in shared {
                    crate::workspace_manager::fixture::remove_file(&path);
                }
            }),
        );
        let (wide, result) = f_verify_width_three("f-residual-i", &tasks, false, policy);
        let progress = result.expect("R-OUTSIDE: the check reads whole and the outage settles");
        assert!(matches!(progress, Progress::Finished { .. }));
        assert!(
            f_unavailable_of_gamma(&wide.env.durable_events()),
            "{:?}",
            kinds_of(&wide.env.durable_events())
        );
    });
}

#[test]
fn a_gate_index_written_again_before_the_check_reads_whole_and_is_judged() {
    bounded("f-residual-ii", || {
        let tasks = f_tasks_of(FSubject::Verification, "src/work.txt");
        let gate: FGate = Arc::new(|snapshot| {
            f_construct(Some("index"), snapshot);
            crate::workspace_manager::fixture::git(snapshot, &["read-tree", "HEAD"]);
            1
        });
        let responder = f_gate_responder(&tasks, FSubject::Verification, gate);
        let (wide, result) = f_drive_subject(
            "f-residual-ii",
            &tasks,
            FSubject::Verification,
            3,
            responder,
            WidePlans::default(),
            None,
        );
        let progress = result.expect("R-REWRITE: the rewritten index reads whole");
        assert!(matches!(progress, Progress::Finished { .. }));
        assert!(
            wide.env
                .durable_events()
                .iter()
                .any(|event| merge_rejected_of_gamma(&event.body)),
            "{:?}",
            kinds_of(&wide.env.durable_events())
        );
    });
}

fn f_files(root: &std::path::Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut files = BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).expect("list a kept directory") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let bytes = std::fs::read(&path).expect("a kept file");
                files.insert(
                    path.strip_prefix(root)
                        .expect("under the root")
                        .to_path_buf(),
                    bytes,
                );
            }
        }
    }
    files
}

fn f_retained_of_alpha(body: &TopologyEventBody) -> bool {
    matches!(
        body,
        TopologyEventBody::AttemptFinished { data }
            if data.key == TaskKey(0)
                && matches!(data.settlement, crate::topology::events::AttemptSettlement::Retained { .. })
    )
}

fn f_retained_retry(
    tag: &str,
    width: u32,
    act: FCheckoutAct,
) -> (Wide, Result<Progress, UpstrokeError>, FKept) {
    let tasks = f_one_task();
    let mut wide = Wide::started_with(
        tag,
        &tasks,
        width,
        WidePlans::default(),
        RecordingRunner::new().answering(wide_responder(&tasks, &[(0, 1)])),
    );
    f_keep_the_store(&wide);
    let root = wide.env.fixture.manager.execution_root().to_path_buf();
    let kept = Arc::new(std::sync::Mutex::new(None));
    let mut hooks = FHooks::over(&wide);
    {
        let kept = Arc::clone(&kept);
        hooks.on_fold.push((
            f_retained_of_alpha,
            Box::new(move || {
                let checkout = f_only_checkout_under(&root.join("tasks"));
                act(&checkout);
                *kept
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(f_files(&checkout));
            }),
        ));
    }
    let result = if width == 1 {
        let seams = wide.env.seams();
        f_step_to_error(&mut wide.run, &seams, &mut hooks)
    } else {
        let pipelines = wide.env.pipelines();
        wide.run
            .run_concurrently(&wide.env.seams(), &pipelines, &mut hooks, None)
    };
    drop(hooks);
    (wide, result, kept)
}

#[test]
fn a_retained_retry_over_a_pruned_populated_slot_refuses_and_keeps_the_generation_retained() {
    bounded("f-c2-retained", || {
        let mut cases = Vec::new();
        for width in [1_u32, 3] {
            let (wide, result, kept) = f_retained_retry(
                &format!("f-c2-retained-w{width}"),
                width,
                Box::new(f_prune_own_entry),
            );
            let events = wide.env.durable_events();
            let class = wide
                .run
                .fold()
                .task(TaskKey(0))
                .and_then(|task| task.generations.first())
                .map(|generation| generation.class.clone());
            let root = wide.env.fixture.manager.execution_root().to_path_buf();
            let before = kept
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone()
                .expect("the retained fold acted");
            let slot_kept = || {
                std::fs::read_dir(root.join("tasks"))
                    .is_ok_and(|mut entries| entries.next().is_some())
                    && f_files(&f_only_checkout_under(&root.join("tasks"))) == before
            };
            let outcome = match result {
                Err(UpstrokeError::RegistryRefused { message })
                    if message.contains("the retry of retained generation")
                        && message.contains("kept for the operator")
                        && kinds_of(&events).last() == Some(&"attempt_finished")
                        && wide.run.entitlements_held() == 0
                        && matches!(
                            class,
                            Some(crate::topology::fold::GenerationClass::RetainedIdle { .. })
                        )
                        && slot_kept() =>
                {
                    Ok(())
                }
                other => Err(format!(
                    "{other:?}, entitlements held {}, class {class:?}: {:?}",
                    wide.run.entitlements_held(),
                    kinds_of(&events)
                )),
            };
            cases.push((format!("width {width}"), outcome));
        }
        f_report(&cases);
    });
}

fn f_closed_worktree_missing_of_alpha(events: &[TopologyEvent]) -> bool {
    events.iter().any(|event| {
        matches!(
            &event.body,
            TopologyEventBody::GenerationClosed { data }
                if data.key == TaskKey(0)
                    && data.reason == crate::topology::events::GenerationCloseReason::WorktreeMissing
        )
    })
}

#[test]
fn a_retained_retry_over_a_whole_slot_that_no_longer_holds_its_tree_closes_and_scrubs() {
    bounded("f-c2-retained-control", || {
        for width in [1_u32, 3] {
            let (wide, result, _) = f_retained_retry(
                &format!("f-c2-retained-control-w{width}"),
                width,
                Box::new(|checkout| {
                    crate::workspace_manager::fixture::write_file(
                        &checkout.join("left-by-hand.txt"),
                        b"no longer the retained tree\n",
                    );
                    crate::workspace_manager::fixture::git(checkout, &["add", "left-by-hand.txt"]);
                }),
            );
            let progress = result.unwrap_or_else(|error| panic!("width {width}: {error}"));
            assert!(matches!(progress, Progress::Finished { .. }));
            assert!(
                f_closed_worktree_missing_of_alpha(&wide.env.durable_events()),
                "width {width}: a genuine mismatch in a whole registration closes as on master: {:?}",
                kinds_of(&wide.env.durable_events())
            );
        }
    });
}

#[test]
fn a_retained_retry_reading_residue_closes_and_scrubs_whatever_the_registration() {
    bounded("f-c2-retained-residue", || {
        for width in [1_u32, 3] {
            let (wide, result, _) = f_retained_retry(
                &format!("f-c2-retained-residue-w{width}"),
                width,
                Box::new(|checkout| {
                    let entry = f_entry_of(checkout);
                    crate::workspace_manager::fixture::write_file(&entry.join("index.lock"), b"");
                    crate::workspace_manager::fixture::remove_file(&entry.join("index"));
                }),
            );
            let progress = result.unwrap_or_else(|error| panic!("width {width}: {error}"));
            assert!(matches!(progress, Progress::Finished { .. }));
            assert!(
                f_closed_worktree_missing_of_alpha(&wide.env.durable_events()),
                "width {width}: residue closes and scrubs as before: {:?}",
                kinds_of(&wide.env.durable_events())
            );
        }
    });
}

fn f_open_generation_left(tag: &str) -> Wide {
    let tasks = f_one_task();
    let mut wide = Wide::durable(
        tag,
        &tasks,
        1,
        WidePlans::default(),
        RecordingRunner::new().answering(wide_responder(&tasks, &[])),
    );
    f_keep_the_store(&wide);
    let mut first = FHooks::over(&wide);
    first.effects.fail_at = Some((
        FSite::Worktree(crate::topology::effects::WorktreeSite::Add),
        FPhase::After,
    ));
    let seams = wide.env.seams();
    let ended = f_step_to_error(&mut wide.run, &seams, &mut first);
    drop(first);
    assert!(
        ended.is_err(),
        "the dispatch's add ends the command after its event"
    );
    assert_eq!(
        kinds_of(&wide.env.durable_events()).last(),
        Some(&"task_dispatched"),
        "an open generation, before its first attempt"
    );
    wide
}

#[test]
fn an_open_generation_continued_live_over_a_pruned_populated_slot_is_kept_and_refused() {
    bounded("f-c2-open-live", || {
        let tasks = f_one_task();
        let wide = f_open_generation_left("f-c2-open-live");
        let (_, mut wide) = wide
            .resume(
                "inc-2",
                RecordingRunner::new().answering(wide_responder(&tasks, &[])),
                crate::engine::topology::select::Ceiling::unlimited(),
            )
            .expect("step (g) recreates the open generation");
        let root = wide.env.fixture.manager.execution_root().to_path_buf();
        let checkout = f_only_checkout_under(&root.join("tasks"));
        crate::workspace_manager::fixture::write_file(
            &checkout.join("untracked.txt"),
            b"the operator's file\n",
        );
        f_prune_own_entry(&checkout);
        let before = f_files(&checkout);
        let appended = wide.env.durable_events().len();
        let mut hooks = wide.env.hooks();
        let seams = wide.env.seams();
        let result = f_step_to_error(&mut wide.run, &seams, &mut hooks);
        drop(hooks);
        f_registry_refused(
            result,
            &["the reuse of generation 0", "kept for the operator"],
            &wide,
        );
        assert_eq!(
            wide.env.durable_events().len(),
            appended,
            "nothing is appended"
        );
        assert!(matches!(
            wide.run
                .fold()
                .task(TaskKey(0))
                .and_then(|task| task.generations.first())
                .map(|generation| generation.class.clone()),
            Some(crate::topology::fold::GenerationClass::OpenNoAttempt)
        ));
        assert_eq!(f_files(&checkout), before, "the slot and its file are kept");
    });
}

#[test]
fn a_resume_over_an_open_generation_whose_populated_slot_is_pruned_refuses_before_run_resumed() {
    bounded("f-c2-open-step-g", || {
        let tasks = f_one_task();
        let wide = f_open_generation_left("f-c2-open-step-g");
        let root = wide.env.fixture.manager.execution_root().to_path_buf();
        let checkout = f_only_checkout_under(&root.join("tasks"));
        crate::workspace_manager::fixture::write_file(
            &checkout.join("untracked.txt"),
            b"the operator's file\n",
        );
        f_prune_own_entry(&checkout);
        let before = f_files(&checkout);
        let Wide { run, env } = wide;
        drop(run);
        let runtime = crate::runner::container::FakeRuntime::new(
            crate::runner::container::runtime::ContainerTrace::off(),
        );
        let liveness = crate::runner::container::FakeOwnerLiveness::new();
        let mut hooks = env.hooks();
        let (refused, env) = match env.try_resume_over(
            "inc-2",
            RecordingRunner::new().answering(wide_responder(&tasks, &[])),
            crate::engine::topology::select::Ceiling::unlimited(),
            &crate::engine::topology::scaffold::ResumingOver {
                runtime: &runtime,
                liveness: &liveness,
                preflight: &crate::engine::topology::scaffold::Certifying,
                awaits_release: true,
            },
            &mut hooks,
        ) {
            Ok(_) => {
                panic!("the resume recreated a populated slot whose registration is not whole")
            }
            Err(refused) => *refused,
        };
        drop(hooks);
        match &refused {
            UpstrokeError::RegistryRefused { message } => assert!(
                message.contains("kept for the operator")
                    && message.contains(&checkout.display().to_string()),
                "{message}"
            ),
            other => panic!("step (g)'s kept-slot refusal, not {other:?}"),
        }
        assert!(
            !kinds_of(&env.durable_events()).contains(&"run_resumed"),
            "the resume refuses before run_resumed: {:?}",
            kinds_of(&env.durable_events())
        );
        assert_eq!(f_files(&checkout), before, "the slot and its file are kept");
    });
}

fn f_run_finished(body: &TopologyEventBody) -> bool {
    matches!(body, TopologyEventBody::RunFinished { .. })
}

fn f_converges_with_the_store_gone(tag: &str, halted: bool) {
    let tasks = f_one_task();
    let responder = if halted {
        crate::engine::topology::scaffold::wide_responder_asking(&tasks, &[], &[0])
    } else {
        wide_responder(&tasks, &[])
    };
    let mut wide = Wide::started_with(
        tag,
        &tasks,
        1,
        WidePlans::default(),
        RecordingRunner::new().answering(responder),
    );
    if halted {
        halting(&mut wide.env);
    }
    let manager = wide.env.fixture.manager.clone();
    let head = wide.env.fixture.head.clone();
    let planted = Arc::new(std::sync::Mutex::new(None::<PathBuf>));
    let mut hooks = FHooks::over(&wide);
    {
        let planted = Arc::clone(&planted);
        hooks.on_fold.push((
            f_run_finished,
            Box::new(move || {
                let slot =
                    crate::engine::topology::dispatch::task_slot(TaskKey(0), GenerationId(7));
                manager
                    .write_intent(&mut crate::workspace_manager::NoHooks, &slot)
                    .expect("the left slot's intent");
                let checkout = manager
                    .add_worktree(&mut crate::workspace_manager::NoHooks, &slot, &head)
                    .expect("the left slot");
                crate::workspace_manager::fixture::write_file(
                    &checkout.join("worker.txt"),
                    b"what the slot still holds\n",
                );
                f_prune_own_entry(&checkout);
                assert!(
                    !manager.common_git_dir().join("worktrees").exists(),
                    "the last entry's prune took the store"
                );
                *planted
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(checkout);
            }),
        ));
    }
    let seams = wide.env.seams();
    let result = f_step_to_error(&mut wide.run, &seams, &mut hooks);
    drop(hooks);
    let progress = result.unwrap_or_else(|error| {
        panic!(
            "finalization converges with the store gone: {error}: {:?}",
            kinds_of(&wide.env.durable_events())
        )
    });
    assert_eq!(
        outcome_of(&progress),
        if halted {
            RunOutcome::Halted
        } else {
            RunOutcome::Complete
        }
    );
    let checkout = planted
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
        .expect("the slot was planted");
    assert!(!checkout.exists(), "the slot is removed");
    assert!(
        !wide.env.fixture.manager.execution_root().exists(),
        "and the execution root with it"
    );
}

#[test]
fn a_complete_runs_finalization_converges_over_a_populated_slot_with_the_store_gone() {
    bounded("f-c3-2-complete", || {
        f_converges_with_the_store_gone("f-c3-2-complete", false);
    });
}

#[test]
fn a_halted_runs_finalization_converges_over_a_populated_slot_with_the_store_gone() {
    bounded("f-c3-2-halted", || {
        f_converges_with_the_store_gone("f-c3-2-halted", true);
    });
}

#[test]
fn a_verification_refused_at_placement_two_discards_a_waiting_attempt_and_the_resume_verifies_again()
 {
    bounded("f-conc-1", || {
        let tasks = f_tasks_of(FSubject::Verification, "src/work.txt");
        let responder = f_gate_responder(&tasks, FSubject::Verification, f_rev_parse_gate(None));
        let runner = RecordingRunner::new().answering(responder);
        runner.hold();
        let mut wide = Wide::durable("f-conc-1", &tasks, 3, WidePlans::default(), runner);
        let mut hooks = wide.env.hooks();
        let pipelines = wide.env.pipelines();
        let held = Arc::clone(&wide.env.runner);
        let mut scheduler = Scheduler::scripted(&held, Box::new(alpha_waits_for_gammas_merge));
        let result = wide.run.run_concurrently(
            &wide.env.seams(),
            &pipelines,
            &mut hooks,
            Some(&mut scheduler),
        );
        drop(scheduler);
        drop(hooks);
        f_registry_refused(result, &["the gates' verdicts"], &wide);
        assert!(
            wide.run.discarded() >= 1,
            "the waiting attempt's completion is discarded"
        );
        let refused = f_verification_started_of_gamma(&wide.env.durable_events())
            .last()
            .copied()
            .expect("gamma's verification started");
        let resumed = f_finish(wide, "inc-2", &tasks);
        let events = resumed.env.durable_events();
        assert!(
            events.iter().any(|event| matches!(
                &event.body,
                TopologyEventBody::AttemptInterrupted { data } if data.key == TaskKey(1)
            )),
            "the next resume settles the waiting attempt interrupted: {:?}",
            kinds_of(&events)
        );
        f_interrupted_and_verified_again(&resumed, refused);
    });
}

type FSignal = Arc<(std::sync::Mutex<bool>, std::sync::Condvar)>;

fn f_signal() -> FSignal {
    Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()))
}

fn f_raise(signal: &FSignal) {
    let (lock, wake) = &**signal;
    *lock
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = true;
    wake.notify_all();
}

fn f_await(signal: &FSignal) -> bool {
    let (lock, wake) = &**signal;
    let guard = lock
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (guard, _) = wake
        .wait_timeout_while(guard, BOUND, |raised| !*raised)
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *guard
}

fn f_two_tasks_alpha_asks() -> [WideTask; 2] {
    [
        WideTask::independent("alpha"),
        WideTask::independent("beta"),
    ]
}

fn f_beta_refusing_after(blocked: FSignal, release: FSignal) -> FPolicy {
    FPolicy::acting(
        "tasks",
        Box::new(move |checkout| {
            f_raise(&blocked);
            assert!(
                f_await(&release),
                "the coordinator released the refusing pipeline"
            );
            crate::workspace_manager::fixture::write_file(
                &checkout.join("a.txt"),
                b"an unstaged change\n",
            );
            crate::workspace_manager::fixture::remove_file(&f_entry_of(checkout).join("index"));
        }),
    )
}

fn f_only_beta(policy: FPolicy) -> FPolicy {
    let FPolicy { act, calls, .. } = policy;
    FPolicy {
        namespace: "tasks",
        act: Box::new(move |checkout| {
            let beta = checkout
                .file_name()
                .and_then(std::ffi::OsStr::to_str)
                .is_some_and(|name| name.starts_with("k1-"));
            if beta {
                act(checkout);
            }
        }),
        once: false,
        acted: std::sync::atomic::AtomicBool::new(false),
        calls,
    }
}

struct FAfterBetaBlocks {
    inner: Arc<RecordingRunner>,
    blocked: FSignal,
}

impl Runner for FAfterBetaBlocks {
    fn run<'a>(
        &'a self,
        request: &'a crate::runner::RunnerRequest,
        call: crate::runner::RunnerCall<'a>,
    ) -> crate::runner::RunFuture<'a> {
        if matches!(
            &request.invocation,
            InvocationId::Attempt {
                key: TaskKey(0),
                role: AttemptRole::Worker,
                ..
            }
        ) {
            assert!(
                f_await(&self.blocked),
                "alpha's worker answers only once beta is blocked in its deletion policy"
            );
        }
        self.inner.run(request, call)
    }
}

#[derive(Debug, Clone)]
struct FHanded {
    changed: bool,
    index_gone: bool,
    answered: bool,
    not_whole: Option<String>,
}

struct FBetaRead {
    inner: FPolicy,
    common: PathBuf,
    handed: Arc<std::sync::Mutex<Vec<FHanded>>>,
}

impl ReviewInputPolicy for FBetaRead {
    fn problem(
        &self,
        worktree: &std::path::Path,
        tree: &str,
    ) -> Result<Option<String>, UpstrokeError> {
        let answer = self.inner.problem(worktree, tree);
        let beta = worktree
            .file_name()
            .and_then(std::ffi::OsStr::to_str)
            .is_some_and(|name| name.starts_with("k1-"));
        if beta {
            let handed = FHanded {
                changed: std::fs::read(worktree.join("a.txt"))
                    .is_ok_and(|bytes| bytes == b"an unstaged change\n"),
                index_gone: !f_entry_of(worktree).join("index").exists(),
                answered: matches!(answer, Ok(_) | Err(UpstrokeError::Git { .. })),
                not_whole: match crate::engine::topology::dispatch::registration_whole(
                    worktree,
                    &self.common,
                    &crate::workspace_manager::ComposedCommonDir::Unknown,
                ) {
                    crate::engine::topology::dispatch::Whole::No(found) => Some(found),
                    crate::engine::topology::dispatch::Whole::Yes => None,
                },
            };
            self.handed
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(handed);
        }
        answer
    }
}

struct FInputsOf {
    inner: Arc<dyn AttemptPlans + Send + Sync>,
    key: TaskKey,
    asked: Arc<std::sync::atomic::AtomicUsize>,
}

impl AttemptPlans for FInputsOf {
    fn inputs(
        &self,
        request: &crate::engine::topology::attempt::InputsRequest<'_>,
    ) -> Result<crate::engine::topology::attempt::ReviewInputs, UpstrokeError> {
        if request.entry.key == self.key {
            self.asked.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
        self.inner.inputs(request)
    }

    fn pool_for(&self, agent: &str) -> Option<String> {
        self.inner.pool_for(agent)
    }

    fn plan(
        &self,
        request: &crate::engine::topology::attempt::PlanRequest<'_>,
    ) -> Result<crate::engine::topology::attempt::AttemptPlan, UpstrokeError> {
        self.inner.plan(request)
    }

    fn verification(
        &self,
        request: &crate::engine::topology::attempt::VerificationRequest<'_>,
    ) -> Result<crate::engine::topology::attempt::VerificationPlan, UpstrokeError> {
        self.inner.verification(request)
    }
}

#[test]
fn a_refusal_arriving_after_a_halt_is_discarded_unread_and_the_halt_scrubs_its_slot() {
    bounded("f-conc-2", || {
        let tasks = f_two_tasks_alpha_asks();
        let blocked = f_signal();
        let release = f_signal();
        let responder = crate::engine::topology::scaffold::wide_responder_asking(&tasks, &[], &[0]);
        let mut wide = Wide::started_with(
            "f-conc-2",
            &tasks,
            3,
            WidePlans::default(),
            RecordingRunner::new().answering(responder),
        );
        halting(&mut wide.env);
        let root = wide.env.fixture.manager.execution_root().to_path_buf();
        let mut hooks = FHooks::over(&wide);
        {
            let release = Arc::clone(&release);
            hooks.on_fold.push((
                |body| matches!(body, TopologyEventBody::QuestionAnswered { .. }),
                Box::new(move || f_raise(&release)),
            ));
        }
        let mut pipelines = wide.env.pipelines();
        pipelines.runner = Arc::new(FAfterBetaBlocks {
            inner: Arc::clone(&wide.env.runner),
            blocked: Arc::clone(&blocked),
        });
        let handed = Arc::new(std::sync::Mutex::new(Vec::new()));
        pipelines.input_policy = Arc::new(FBetaRead {
            inner: f_only_beta(f_beta_refusing_after(Arc::clone(&blocked), release)),
            common: wide.env.fixture.manager.common_git_dir().to_path_buf(),
            handed: Arc::clone(&handed),
        });
        let asked = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        pipelines.plans = Arc::new(FInputsOf {
            inner: Arc::clone(&pipelines.plans),
            key: TaskKey(1),
            asked: Arc::clone(&asked),
        });
        let progress = wide
            .run
            .run_concurrently(&wide.env.seams(), &pipelines, &mut hooks, None)
            .expect("the halt's closure ends the run");
        drop(hooks);
        assert_eq!(outcome_of(&progress), RunOutcome::Halted);
        assert!(
            *blocked
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            "beta entered its deletion policy"
        );
        let handed = handed
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        assert!(
            matches!(
                handed.as_slice(),
                [read] if read.changed
                    && read.index_gone
                    && read.answered
                    && read.not_whole.as_deref().is_some_and(|found| found.contains("index"))
            ),
            "beta's policy handed back its checkout damaged (the change written, the index \
                 gone, the registration not whole), so the check after it refuses: {handed:?}"
        );
        assert_eq!(
            asked.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "beta's pipeline ended at its refused review-input read: it never asked for its \
                 review inputs"
        );
        assert!(
            wide.run.discarded() >= 1,
            "the refused completion is counted"
        );
        assert!(
            !wide
                .run
                .warnings()
                .iter()
                .any(|warning| warning.contains("review-input")
                    || warning.contains("registration")),
            "no warning names the refusal: {:?}",
            wide.run.warnings()
        );
        let left: Vec<PathBuf> = std::fs::read_dir(root.join("tasks"))
            .map(|entries| {
                entries
                    .map(|entry| entry.expect("an entry").path())
                    .collect()
            })
            .unwrap_or_default();
        assert!(
            left.is_empty(),
            "the halt's closure scrubbed the in-flight slots, the pruned one included: {left:?}"
        );
    });
}

#[test]
fn a_kept_slots_read_waits_on_a_contended_registry_through_the_coordinators_hooks() {
    let what = "f-conc-3";
    bounded(what, move || {
        let tasks = two_independent();
        let plans = WidePlans {
            gates: 2,
            ..WidePlans::default()
        };
        let mut wide = Wide::started_with(what, &tasks, 2, plans, holding(&tasks, &[(0, 1)]));
        let common = wide.env.fixture.manager.common_git_dir().to_path_buf();
        let before = crate::workspace_manager::contended_attempts(&common);
        let mut hooks = TearHeld::waking(
            &wide,
            "foreign-held",
            TearAt::Access(retained_attempt_finished_of_beta, 3),
            Wakes::Ended,
        );
        hooks.tearing(Torn::GitdirUnreadable);
        let slot =
            wide.env
                .fixture
                .manager
                .slot_path(&crate::engine::topology::dispatch::task_slot(
                    TaskKey(0),
                    GenerationId(0),
                ));
        hooks.also = Some((
            retained_attempt_finished_of_beta,
            Box::new(move || f_prune_own_entry(&slot)),
        ));
        let pipelines = wide.env.pipelines();
        let runner = Arc::clone(&wide.env.runner);
        let mut scheduler = Scheduler::scripted(
            &runner,
            Box::new(beta_settles_while_alpha_holds_its_first_gate),
        );
        let slept = crate::workspace_manager::fixture::slept_pauses();
        let result = wide.run.run_concurrently(
            &wide.env.seams(),
            &pipelines,
            &mut hooks,
            Some(&mut scheduler),
        );
        assert_eq!(
            crate::workspace_manager::fixture::slept_pauses(),
            slept,
            "no wait of the kept slot's read slept on the coordinator's thread"
        );
        drop(scheduler);
        hooks
            .finish()
            .expect("an invocation ended while the kept slot's read waited on the tear");
        drop(hooks);
        f_registry_refused(
            result,
            &["the retry of retained generation", "kept for the operator"],
            &wide,
        );
        assert!(
            crate::workspace_manager::contended_attempts(&common) > before,
            "the kept slot's read failed on the tear first"
        );
    });
}

struct FEvents {
    armed_by: FSignal,
    poisoned: FSignal,
    injected: bool,
}

impl crate::events::log::EventHooks for FEvents {
    fn point(
        &mut self,
        site: crate::topology::effects::EventSite,
        point: crate::topology::effects::SubEffectPoint,
        mode: crate::topology::effects::InjectionMode,
    ) -> crate::topology::effects::Injection {
        let armed = *self
            .armed_by
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !self.injected
            && armed
            && site == crate::topology::effects::EventSite::Append
            && point == crate::topology::effects::SubEffectPoint::Synced
            && mode == crate::topology::effects::InjectionMode::ErrorReturn
        {
            self.injected = true;
            f_raise(&self.poisoned);
            return crate::topology::effects::Injection::Error;
        }
        crate::topology::effects::Injection::Proceed
    }
}

struct FAppendFailing {
    inner: FHooks,
    events: FEvents,
}

impl TopologyHooks for FAppendFailing {
    fn effects(&mut self) -> &mut dyn crate::workspace_manager::EffectHooks {
        self.inner.effects()
    }

    fn rundir(&mut self) -> &mut dyn crate::rundir::RunDirHooks {
        self.inner.rundir()
    }

    fn events(&mut self) -> &mut dyn crate::events::log::EventHooks {
        &mut self.events
    }

    fn container(&mut self) -> &mut dyn crate::runner::container::ContainerHooks {
        self.inner.container()
    }

    fn spawn(&mut self) -> &mut dyn crate::agent::proc::SpawnHooks {
        self.inner.spawn()
    }

    fn folded(&mut self, fold: &TopologyFold, events: &[TopologyEvent]) {
        self.inner.folded(fold, events);
    }
}

#[test]
fn a_refusal_arriving_after_an_append_error_is_discarded_on_the_poisoned_fold() {
    bounded("f-conc-4", || {
        let tasks = f_two_tasks_alpha_asks();
        let blocked = f_signal();
        let poisoned = f_signal();
        let mut wide = Wide::durable(
            "f-conc-4",
            &tasks,
            3,
            WidePlans::default(),
            RecordingRunner::new().answering(wide_responder(&tasks, &[])),
        );
        let mut hooks = FAppendFailing {
            inner: FHooks::over(&wide),
            events: FEvents {
                armed_by: Arc::clone(&blocked),
                poisoned: Arc::clone(&poisoned),
                injected: false,
            },
        };
        let mut pipelines = wide.env.pipelines();
        pipelines.input_policy = Arc::new(f_alpha_after_beta_blocks(
            Arc::clone(&blocked),
            f_only_beta(f_beta_refusing_after(blocked, poisoned)),
        ));
        let result = wide
            .run
            .run_concurrently(&wide.env.seams(), &pipelines, &mut hooks, None);
        drop(hooks);
        match &result {
            Err(UpstrokeError::RegistryRefused { message }) => {
                panic!("the append error is the command's error, not the refusal: {message}")
            }
            Err(_) => {}
            Ok(progress) => panic!("the append error ends the command: {progress:?}"),
        }
        assert!(
            wide.run.discarded() >= 1,
            "the refused completion is counted"
        );
        assert!(
            !wide
                .run
                .warnings()
                .iter()
                .any(|warning| warning.contains("review-input")
                    || warning.contains("registration")),
            "no warning names the refusal: {:?}",
            wide.run.warnings()
        );
        let root = wide.env.fixture.manager.execution_root().to_path_buf();
        let Wide { run, env } = wide;
        drop(run);
        let runtime = crate::runner::container::FakeRuntime::new(
            crate::runner::container::runtime::ContainerTrace::off(),
        );
        let liveness = crate::runner::container::FakeOwnerLiveness::new();
        let mut hooks = env.hooks();
        let (refused, env) = match env.try_resume_over(
            "inc-2",
            holding(&tasks, &[]),
            crate::engine::topology::select::Ceiling::unlimited(),
            &crate::engine::topology::scaffold::ResumingOver {
                runtime: &runtime,
                liveness: &liveness,
                preflight: &crate::engine::topology::scaffold::Certifying,
                awaits_release: true,
            },
            &mut hooks,
        ) {
            Ok(_) => panic!("the guard keeps the refused attempt's slot, whose index is gone"),
            Err(refused) => *refused,
        };
        drop(hooks);
        let kept = match &refused {
            UpstrokeError::RegistryRefused { message }
                if message.contains("recovery's reclaim") && message.contains("index") =>
            {
                std::fs::read_dir(root.join("tasks"))
                    .expect("the task namespace")
                    .map(|entry| entry.expect("an entry").path())
                    .find(|path| {
                        path.file_name()
                            .and_then(std::ffi::OsStr::to_str)
                            .is_some_and(|name| name.starts_with("k1-"))
                            && message.contains(&path.display().to_string())
                    })
                    .expect("the refusal names beta's kept slot")
            }
            other => panic!("the guard's refusal at the next resume, not {other:?}"),
        };
        let interrupted = count(&env.durable_events(), "attempt_interrupted");
        assert!(
            interrupted >= 1,
            "the resume followed the surviving prefix, settling the attempts interrupted: {:?}",
            kinds_of(&env.durable_events())
        );
        let held: Vec<PathBuf> = std::fs::read_dir(&kept)
            .expect("the kept slot")
            .map(|entry| entry.expect("an entry").path())
            .collect();
        for path in held {
            if path.is_dir() {
                f_remove_tree_by_hand(&path);
            } else {
                crate::workspace_manager::fixture::remove_file(&path);
            }
        }
        crate::workspace_manager::fixture::remove_dir(&kept);
        let (_, mut resumed) = env
            .resume(
                "inc-3",
                holding(&tasks, &[]),
                crate::engine::topology::select::Ceiling::unlimited(),
            )
            .expect("once the operator removed the kept slot the next resume reclaims it");
        let runner = Arc::clone(&resumed.env.runner);
        let mut scheduler = Scheduler::first(&runner);
        let progress = drive(&mut resumed, Some(&mut scheduler)).expect("the run ends");
        drop(scheduler);
        assert!(
            matches!(progress, Progress::Finished { .. }),
            "{progress:?}"
        );
    });
}

fn f_remove_tree_by_hand(root: &std::path::Path) {
    let entries: Vec<PathBuf> = std::fs::read_dir(root)
        .expect("a directory to remove")
        .map(|entry| entry.expect("an entry").path())
        .collect();
    for path in entries {
        if path.is_dir() {
            f_remove_tree_by_hand(&path);
        } else {
            crate::workspace_manager::fixture::remove_file(&path);
        }
    }
    crate::workspace_manager::fixture::remove_dir(root);
}

fn f_alpha_after_beta_blocks(blocked: FSignal, beta: FPolicy) -> FPolicy {
    let FPolicy { act, calls, .. } = beta;
    FPolicy {
        namespace: "tasks",
        act: Box::new(move |checkout| {
            let alpha = checkout
                .file_name()
                .and_then(std::ffi::OsStr::to_str)
                .is_some_and(|name| name.starts_with("k0-"));
            if alpha {
                assert!(f_await(&blocked), "beta reached its refusal point");
            } else {
                act(checkout);
            }
        }),
        once: false,
        acted: std::sync::atomic::AtomicBool::new(false),
        calls,
    }
}

#[test]
fn a_verification_refused_after_its_passes_were_charged_records_them_under_o14b() {
    bounded("f-o14b", || {
        for width in [1, 3] {
            for answer in [FAnswer::Unavailable, FAnswer::Pass] {
                let case = format!("width {width}, pass 1 {answer:?}");
                let tasks = f_tasks_of(FSubject::Verification, "src/work.txt");
                let stub = Arc::new(FCostedReviewStub {
                    target: FReviewStub {
                        subject: FSubject::Verification,
                        at_pass: 1,
                        prune: true,
                        answer,
                    },
                });
                let (wide, result) =
                    f_drive_costed(&format!("f-o14b-w{width}-{answer:?}"), &tasks, width, stub);
                f_registry_refused(result, &["the verdict of review pass 1"], &wide);
                let events = wide.env.durable_events();
                let records = charged_records(&events);
                let [(refused, reviews)] = records.as_slice() else {
                    panic!(
                        "{case}: one merge_verification_charged before the command ends: {:?}",
                        kinds_of(&events)
                    );
                };
                let refused = *refused;
                assert_eq!(
                    reviews
                        .iter()
                        .map(|review| review.cost_usd)
                        .collect::<Vec<_>>(),
                    vec![Some(PASS_COST), Some(PASS_COST)],
                    "{case}: the record holds pass 0 and the refused pass 1, as charged"
                );
                assert!(
                    terminals_of(&events, refused).is_empty(),
                    "{case}: the refused verification has no terminal yet: {:?}",
                    kinds_of(&events)
                );
                let live = wide.run.spend().run_total();
                let logged = crate::engine::topology::select::Spend::replay(&events).run_total();
                let without = crate::engine::topology::select::Spend::replay(&unrecorded(&events))
                    .run_total();
                assert!(
                    (live - logged).abs() < 1e-9,
                    "{case}: the log holds both passes the live spend holds: live {live}, \
                         logged {logged}"
                );
                assert!(
                    (logged - without - 2.0 * PASS_COST).abs() < 1e-9,
                    "{case}: the record carries both passes: {without} -> {logged}"
                );
                let next = if width == 1 { "inc-3" } else { "inc-2" };
                let (_, resumed) = wide
                    .resume(
                        next,
                        holding(&tasks, &[]),
                        crate::engine::topology::select::Ceiling::unlimited(),
                    )
                    .expect("the next process resumes");
                let events = resumed.env.durable_events();
                assert_eq!(
                    terminals_of(&events, refused),
                    vec!["merge_verification_interrupted"],
                    "{case}: the next resume settles the refused verification interrupted: {:?}",
                    kinds_of(&events)
                );
                assert_eq!(
                    charged_records(&events).len(),
                    1,
                    "{case}: the resume records no second set of passes"
                );
                let replayed = crate::engine::topology::select::Spend::replay(&events).run_total();
                assert!(
                    (replayed - logged).abs() < 1e-9,
                    "{case}: after the resume the replay counts both passes once: {replayed} \
                         vs {logged}"
                );
            }
        }
    });
}

struct FCostedReviewStub {
    target: FReviewStub,
}

impl crate::engine::topology::attempt::ReviewPasses for FCostedReviewStub {
    fn run(
        &self,
        cx: &crate::review::ReviewCx<'_>,
        runner: &dyn crate::runner::Runner,
        invocations: &crate::review::ReviewInvocations,
    ) -> Result<crate::review::ReviewOutcome, UpstrokeError> {
        let verification = matches!(&invocations.pass, InvocationId::Sequence { .. });
        if verification && !self.target.is_target(invocations) {
            return Ok(crate::review::ReviewOutcome {
                result: crate::review::ReviewResult::Judged(crate::ir::Verdict {
                    pass: true,
                    reasons: Vec::new(),
                    required_changes: Vec::new(),
                    needs_human: false,
                }),
                cost_usd: Some(0.25),
                invocations: 0,
                transcript: PathBuf::new(),
                never_started: false,
            });
        }
        <FReviewStub as crate::engine::topology::attempt::ReviewPasses>::run(
            &self.target,
            cx,
            runner,
            invocations,
        )
    }
}

fn f_drive_costed(
    tag: &str,
    tasks: &[WideTask],
    width: u32,
    stub: Arc<FCostedReviewStub>,
) -> (Wide, Result<Progress, UpstrokeError>) {
    let mut wide = Wide::durable(tag, tasks, 3, f_two_reviewers(), holding(tasks, &[]));
    if width == 1 {
        let mut first = FHooks::over(&wide);
        first.effects.fail_at = Some((F_PICK, FPhase::Before));
        let pipelines = wide.env.pipelines();
        let held = Arc::clone(&wide.env.runner);
        let mut scheduler =
            Scheduler::scripted(&held, Box::new(alpha_waits_for_gammas_integration));
        let ended = wide.run.run_concurrently(
            &wide.env.seams(),
            &pipelines,
            &mut first,
            Some(&mut scheduler),
        );
        drop(scheduler);
        drop(first);
        assert!(ended.is_err(), "the first command ends at gamma's pick");
        let (_, mut wide) = wide
            .resume(
                "inc-2",
                RecordingRunner::new().answering(wide_responder(tasks, &[])),
                crate::engine::topology::select::Ceiling::unlimited(),
            )
            .expect("the run resumes with gamma queued");
        let mut hooks = wide.env.hooks();
        let seams = crate::engine::topology::run::RunSeams {
            reviews: &*stub,
            ..wide.env.seams()
        };
        let result = f_step_to_error(&mut wide.run, &seams, &mut hooks);
        drop(hooks);
        return (wide, result);
    }
    let mut hooks = wide.env.hooks();
    let mut pipelines = wide.env.pipelines();
    pipelines.reviews = stub;
    let held = Arc::clone(&wide.env.runner);
    let mut scheduler = Scheduler::scripted(&held, Box::new(alpha_waits_for_gammas_integration));
    let result = wide.run.run_concurrently(
        &wide.env.seams(),
        &pipelines,
        &mut hooks,
        Some(&mut scheduler),
    );
    drop(scheduler);
    drop(hooks);
    (wide, result)
}

fn f_promoting_left(tag: &str, keep_the_store: bool) -> Wide {
    let tasks = f_one_task();
    let mut wide = Wide::durable(
        tag,
        &tasks,
        1,
        WidePlans::default(),
        RecordingRunner::new().answering(wide_responder(&tasks, &[])),
    );
    if keep_the_store {
        f_keep_the_store(&wide);
    }
    let mut first = FHooks::over(&wide);
    first.effects.fail_at = Some((
        FSite::Ref(crate::topology::effects::RefSite::CreateCandidates),
        FPhase::Before,
    ));
    let seams = wide.env.seams();
    let ended = f_step_to_error(&mut wide.run, &seams, &mut first);
    drop(first);
    assert!(
        ended.is_err(),
        "the candidates ref's creation ends the command after `candidate_prepared`"
    );
    assert_eq!(
        kinds_of(&wide.env.durable_events()).last(),
        Some(&"candidate_prepared"),
        "a promoting generation: `candidate_prepared` is durable and \
             `task_candidate_created` is not"
    );
    wide
}

fn f_pruned_promoting_checkout(wide: &Wide) -> PathBuf {
    let manager = &wide.env.fixture.manager;
    let checkout = f_only_checkout_under(&manager.execution_root().join("tasks"));
    crate::workspace_manager::fixture::write_file(
        &checkout.join("untracked.txt"),
        b"the attempt's output its candidate does not pin\n",
    );
    f_prune_own_entry(&checkout);
    checkout
}

fn f_resume_fresh(
    wide: Wide,
    tasks: &[WideTask],
) -> (
    Result<Vec<TaskKey>, UpstrokeError>,
    Vec<TopologyEvent>,
    crate::engine::topology::scaffold::WideEnv,
) {
    let Wide { run, env } = wide;
    drop(run);
    let runtime = crate::runner::container::FakeRuntime::new(
        crate::runner::container::runtime::ContainerTrace::off(),
    );
    let liveness = crate::runner::container::FakeOwnerLiveness::new();
    let mut hooks = env.hooks();
    let resumed = env.try_resume_over(
        "inc-2",
        RecordingRunner::new().answering(wide_responder(tasks, &[])),
        crate::engine::topology::select::Ceiling::unlimited(),
        &crate::engine::topology::scaffold::ResumingOver {
            runtime: &runtime,
            liveness: &liveness,
            preflight: &crate::engine::topology::scaffold::Certifying,
            awaits_release: true,
        },
        &mut hooks,
    );
    drop(hooks);
    match resumed {
        Ok((recovered, wide)) => {
            let Wide { run, env } = wide;
            drop(run);
            let events = env.durable_events();
            (Ok(recovered.finished), events, env)
        }
        Err(failed) => {
            let (error, env) = *failed;
            let events = env.durable_events();
            (Err(error), events, env)
        }
    }
}

fn f_promotion_kept(
    resumed: Result<Vec<TaskKey>, UpstrokeError>,
    events: &[TopologyEvent],
    checkout: &std::path::Path,
    before: &BTreeMap<PathBuf, Vec<u8>>,
) {
    match resumed {
        Ok(finished) => panic!(
            "the resume finished the promotion of {finished:?} over a populated slot whose \
                 registration is not whole: {:?}",
            kinds_of(events)
        ),
        Err(UpstrokeError::RegistryRefused { message }) => assert!(
            message.contains("kept for the operator")
                && message.contains("completion of the promotion")
                && message.contains(&checkout.display().to_string()),
            "{message}"
        ),
        Err(other) => panic!("the promotion's kept-slot refusal, not {other:?}"),
    }
    let kinds = kinds_of(events);
    assert!(
        !kinds.contains(&"task_candidate_created") && !kinds.contains(&"run_resumed"),
        "the resume refuses before the candidate is created and before run_resumed: {kinds:?}"
    );
    assert_eq!(
        &f_files(checkout),
        before,
        "the slot and what it holds are kept"
    );
}

#[test]
fn a_resume_keeps_a_promoting_generations_pruned_populated_slot_with_the_store_gone() {
    bounded("f-promoting-store-gone", || {
        let tasks = f_one_task();
        let wide = f_promoting_left("f-promoting-store-gone", false);
        let checkout = f_pruned_promoting_checkout(&wide);
        assert!(
            !wide
                .env
                .fixture
                .manager
                .common_git_dir()
                .join("worktrees")
                .exists(),
            "the last entry's prune took the store"
        );
        let before = f_files(&checkout);
        let (resumed, events, env) = f_resume_fresh(wide, &tasks);
        f_promotion_kept(resumed, &events, &checkout, &before);
        drop(env);
    });
}

#[test]
fn a_resume_keeps_a_promoting_generations_pruned_populated_slot_while_the_store_stays() {
    bounded("f-promoting-store-kept", || {
        let tasks = f_one_task();
        let wide = f_promoting_left("f-promoting-store-kept", true);
        let checkout = f_pruned_promoting_checkout(&wide);
        assert!(
            wide.env
                .fixture
                .manager
                .common_git_dir()
                .join("worktrees")
                .is_dir(),
            "another registration keeps the store"
        );
        let before = f_files(&checkout);
        let (resumed, events, env) = f_resume_fresh(wide, &tasks);
        f_promotion_kept(resumed, &events, &checkout, &before);
        drop(env);
    });
}

#[test]
fn the_kept_slot_comparison_fails_once_a_kept_file_is_removed_by_hand() {
    bounded("f-promoting-kept-control", || {
        let tasks = f_one_task();
        let wide = f_promoting_left("f-promoting-kept-control", false);
        let checkout = f_pruned_promoting_checkout(&wide);
        let before = f_files(&checkout);
        let (resumed, events, env) = f_resume_fresh(wide, &tasks);
        let refused = resumed.expect_err("the resume refuses the promotion's completion");
        let kept = checkout.join("untracked.txt");
        assert!(kept.is_file(), "the refused resume kept {}", kept.display());
        crate::workspace_manager::fixture::remove_file(&kept);
        let payload = std::panic::catch_unwind(AssertUnwindSafe(|| {
            f_promotion_kept(Err(refused), &events, &checkout, &before);
        }))
        .expect_err("the comparison fails once a kept file is gone");
        let message = payload
            .downcast_ref::<String>()
            .map(String::as_str)
            .unwrap_or_default();
        assert!(
            message.contains("the slot and what it holds are kept"),
            "the comparison, not an earlier check, fails: {message:?}"
        );
        drop(env);
    });
}

#[test]
fn a_resume_finishes_the_promotion_once_the_operator_has_removed_its_kept_slot() {
    bounded("f-promoting-released", || {
        let tasks = f_one_task();
        let wide = f_promoting_left("f-promoting-released", false);
        let checkout = f_pruned_promoting_checkout(&wide);
        f_remove_tree_by_hand(&checkout);
        let (resumed, events, env) = f_resume_fresh(wide, &tasks);
        let finished =
            resumed.unwrap_or_else(|error| panic!("the resume finishes the promotion: {error}"));
        assert_eq!(
            finished,
            vec![TaskKey(0)],
            "step (f) finishes the promotion"
        );
        let kinds = kinds_of(&events);
        assert!(
            kinds.contains(&"task_candidate_created") && kinds.contains(&"run_resumed"),
            "{kinds:?}"
        );
        assert!(!checkout.exists(), "nothing is left at the slot");
        drop(env);
    });
}

#[test]
fn a_live_promotion_still_removes_its_pruned_populated_slot_with_the_store_gone() {
    bounded("f-promotion-live-cleanup", || {
        let tasks = f_one_task();
        let mut wide = Wide::started_with(
            "f-promotion-live-cleanup",
            &tasks,
            1,
            WidePlans::default(),
            RecordingRunner::new().answering(wide_responder(&tasks, &[])),
        );
        let manager = wide.env.fixture.manager.clone();
        let planted = Arc::new(std::sync::Mutex::new(None::<PathBuf>));
        let mut hooks = FHooks::over(&wide);
        {
            let planted = Arc::clone(&planted);
            hooks.effects.at_phase.push((
                FSite::Ref(crate::topology::effects::RefSite::CreateCandidates),
                FPhase::Before,
                Box::new(move || {
                    let checkout = f_only_checkout_under(&manager.execution_root().join("tasks"));
                    crate::workspace_manager::fixture::write_file(
                        &checkout.join("untracked.txt"),
                        b"the attempt's output its candidate does not pin\n",
                    );
                    f_prune_own_entry(&checkout);
                    assert!(
                        !manager.common_git_dir().join("worktrees").exists(),
                        "the last entry's prune took the store"
                    );
                    *planted
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(checkout);
                }),
            ));
        }
        let seams = wide.env.seams();
        let result = f_step_to_error(&mut wide.run, &seams, &mut hooks);
        drop(hooks);
        let progress = result.unwrap_or_else(|error| {
            panic!(
                "the run completes: {error}: {:?}",
                kinds_of(&wide.env.durable_events())
            )
        });
        assert_eq!(outcome_of(&progress), RunOutcome::Complete);
        let checkout = planted
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
            .expect("the slot was pruned at the promotion");
        assert!(
            !checkout.exists(),
            "the live promotion's reclaim removed the slot with what it held"
        );
        assert!(
            kinds_of(&wide.env.durable_events()).contains(&"task_candidate_created"),
            "the promotion was completed live"
        );
    });
}

#[test]
fn the_run_end_closure_still_removes_a_promoting_generations_pruned_populated_slot() {
    bounded("f-promoting-at-run-end", || {
        let tasks = [
            WideTask::independent("alpha"),
            WideTask::independent("beta"),
        ];
        let mut wide = halted_by_the_last_task("f-promoting-at-run-end", &tasks);
        let mut hooks = CasFailing::over(&wide);
        hooks.fail_the_next(crate::topology::effects::RefSite::CreateCandidates);
        let seams = wide.env.seams();
        let error = wide
            .run
            .step(&seams, &mut hooks)
            .expect_err("alpha's candidates ref is made to fail once");
        assert!(error.to_string().contains("CreateCandidates"), "{error}");
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
        let checkout =
            wide.env
                .fixture
                .manager
                .slot_path(&crate::engine::topology::dispatch::task_slot(
                    TaskKey(0),
                    GenerationId(0),
                ));
        crate::workspace_manager::fixture::write_file(
            &checkout.join("untracked.txt"),
            b"the attempt's output its candidate does not pin\n",
        );
        f_prune_own_entry(&checkout);
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
        assert!(
            !checkout.exists(),
            "run-end closure removed the slot with what it held"
        );
    });
}

#[test]
fn a_clean_run_resumed_under_a_linked_registry_is_refused_until_the_link_is_gone() {
    assert!(
        std::env::var_os(crate::workspace_manager::COMMON_DIR).is_none(),
        "the condition under test is that no `GIT_COMMON_DIR` is inherited, and this process \
             inherits one"
    );
    let tasks = [WideTask::independent("alpha")];
    let Wide { run, env } = Wide::durable(
        "resume-linked-registry",
        &tasks,
        1,
        WidePlans::default(),
        RecordingRunner::new().answering(wide_responder(&tasks, &[])),
    );
    drop(run);
    let registry = env.fixture.base.join(".git").join("worktrees");
    let elsewhere = env.fixture.root.join("other").join("worktrees");
    crate::workspace_manager::fixture::create_dir(&elsewhere);
    crate::workspace_manager::fixture::link_directory(&elsewhere, &registry);
    let log = std::fs::read(&env.log).expect("the run's log");
    let execution_root = env.fixture.manager.execution_root().to_path_buf();
    let trace = crate::runner::container::runtime::ContainerTrace::recording();
    let runtime = crate::runner::container::FakeRuntime::new(trace.clone());
    let liveness = crate::runner::container::FakeOwnerLiveness::new();
    let mut hooks = env.hooks();
    let (refused, env) = match env.try_resume_over(
        "inc-2",
        RecordingRunner::new().answering(wide_responder(&tasks, &[])),
        crate::engine::topology::select::Ceiling::unlimited(),
        &crate::engine::topology::scaffold::ResumingOver {
            runtime: &runtime,
            liveness: &liveness,
            preflight: &crate::engine::topology::scaffold::Certifying,
            awaits_release: false,
        },
        &mut hooks,
    ) {
        Ok(_) => panic!("the resume went ahead under a registry linked out of the run"),
        Err(refused) => *refused,
    };
    let message = refused.to_string();
    let target = std::path::Path::new("other").join("worktrees");
    assert!(
        message.contains("refusing to start or resume a schema-4 run")
            && message.contains("no `GIT_COMMON_DIR` is inherited")
            && message.contains(&target.display().to_string()),
        "the resume refuses, naming the entry and its target: {message}"
    );
    assert_eq!(
        std::fs::read(&env.log).expect("the run's log"),
        log,
        "the refused resume appended nothing"
    );
    assert!(
        trace.entries().is_empty(),
        "no runtime inspection or probe ran after the refusal: {:?}",
        trace.rendered()
    );
    for (directory, what) in [
        (execution_root.join("tasks"), "slot"),
        (execution_root.join("intents"), "intent"),
        (elsewhere, "registration in the linked directory"),
    ] {
        assert_eq!(
            std::fs::read_dir(&directory)
                .expect("a directory of the run")
                .count(),
            0,
            "the refused resume made no {what}"
        );
    }
    crate::workspace_manager::fixture::unlink_directory(&registry);
    let (_, mut resumed) = env
        .resume(
            "inc-2",
            RecordingRunner::new().answering(wide_responder(&tasks, &[])),
            crate::engine::topology::select::Ceiling::unlimited(),
        )
        .expect("once the link is gone the run resumes");
    let progress = drive(&mut resumed, None).expect("the resumed run completes");
    assert_eq!(outcome_of(&progress), RunOutcome::Complete);
}

#[cfg(unix)]
#[test]
fn a_relative_object_directory_refuses_derive_with_the_resumes_arguments() {
    let tasks = [WideTask::independent("alpha")];
    let Wide { run, env } = Wide::durable(
        "resume-relative-objects",
        &tasks,
        1,
        WidePlans::default(),
        RecordingRunner::new().answering(wide_responder(&tasks, &[])),
    );
    drop(run);
    let log = std::fs::read(&env.log).expect("the run's log");
    let execution_root = env.fixture.manager.execution_root().to_path_buf();
    let ended = crate::agent::proc::test_support::run_test_isolated(
        "engine::topology::coordinator::tests::relative_name_derive_boundary_child",
        &[
            ("GIT_OBJECT_DIRECTORY", std::ffi::OsStr::new("../objects")),
            (BOUNDARY_ROOT, env.fixture.root.as_os_str()),
        ],
        BOUND,
    );
    assert!(
        ended.status.is_some_and(|status| status.success()) && ended.stdout.contains("1 passed"),
        "{ended}"
    );
    assert_eq!(
        std::fs::read(&env.log).expect("the run's log"),
        log,
        "the refused `derive` left the run's log as it was"
    );
    for name in ["tasks", "intents"] {
        assert_eq!(
            std::fs::read_dir(execution_root.join(name))
                .expect("a directory of the run")
                .count(),
            0,
            "the refused `derive` made nothing in `{name}`"
        );
    }
    let (_, mut resumed) = env
        .resume(
            "inc-2",
            RecordingRunner::new().answering(wide_responder(&tasks, &[])),
            crate::engine::topology::select::Ceiling::unlimited(),
        )
        .expect("without the relative value the run resumes");
    let progress = drive(&mut resumed, None).expect("the resumed run completes");
    assert_eq!(outcome_of(&progress), RunOutcome::Complete);
}
