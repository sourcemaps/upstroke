//! Extended notes: `docs/internals/engine/topology/coordinator/tests/registry_waits.md`

use super::*;

fn registry_drive(
    wide: &mut Wide,
    policy: Arc<dyn ReviewInputPolicy + Send + Sync>,
) -> Result<Progress, UpstrokeError> {
    let mut hooks = wide.env.hooks();
    let mut pipelines = wide.env.pipelines();
    pipelines.input_policy = policy;
    let runner = Arc::clone(&wide.env.runner);
    let mut scheduler = Scheduler::first(&runner);
    wide.run.run_concurrently(
        &wide.env.seams(),
        &pipelines,
        &mut hooks,
        Some(&mut scheduler),
    )
}

#[test]
fn a_verification_beside_another_processs_registration_write_in_flight_spends_no_deferral() {
    let tasks = [
        WideTask::independent("alpha"),
        WideTask::independent("beta"),
    ];
    let mut wide = Wide::started_with(
        "registry-verification-transient",
        &tasks,
        2,
        WidePlans::default(),
        holding(&tasks, &[]),
    );
    let _deadline = crate::workspace_manager::RegistryDeadline::hold(
        wide.env.fixture.manager.common_git_dir(),
        WITNESS_REGISTRY_DEADLINE,
    );
    let common = wide.env.fixture.manager.common_git_dir().to_path_buf();
    let before = crate::workspace_manager::contended_attempts(&common);
    let (policy, _admin) = tearing(&wide, "foreign-in-flight", true);
    let progress = registry_drive(&mut wide, Arc::<TearsAForeignRegistration>::clone(&policy))
        .expect("a registration another process finishes writing ends nothing");
    policy
        .finish()
        .expect("an access failed on the tear, and only then did its writer finish it");
    assert_eq!(outcome_of(&progress), RunOutcome::Complete);
    let events = wide.run.events();
    assert_eq!(
        count(events, "merge_verification_unavailable"),
        0,
        "{:?}",
        kinds_of(events)
    );
    assert_eq!(count(events, "task_merged"), 2);
    assert!(
        crate::workspace_manager::contended_attempts(&common) > before,
        "the verification's registry access failed on the write in flight first"
    );
    assert!(wide.run.invocations_balance());
    replay_equals_live(&wide);
}

#[test]
fn a_verification_beside_a_registration_that_stays_torn_ends_resumably_and_its_resume_reverifies() {
    let tasks = [
        WideTask::independent("alpha"),
        WideTask::independent("beta"),
    ];
    let mut wide = Wide::durable(
        "registry-verification-static",
        &tasks,
        2,
        WidePlans::default(),
        holding(&tasks, &[]),
    );
    let (policy, admin) = tearing(&wide, "foreign-torn", false);
    let error = registry_drive(&mut wide, policy)
        .expect_err("a registration that stays torn ends the command");
    assert!(
        matches!(&error, UpstrokeError::RegistryRefused { message } if message.contains("commondir")),
        "{error:?}"
    );
    let kinds = kinds_of_log(&wide);
    for absent in [
        "merge_verification_unavailable",
        "question_asked",
        "run_finished",
    ] {
        assert!(!kinds.contains(&absent), "{absent} in {kinds:?}");
    }
    assert_eq!(
        kinds.last(),
        Some(&"merge_verification_started"),
        "nothing durable after the verification began: {kinds:?}"
    );
    for file in ["HEAD", "gitdir", "commondir"] {
        crate::workspace_manager::fixture::remove_file(&admin.join(file));
    }
    crate::workspace_manager::fixture::remove_dir(&admin);
    let (_, mut resumed) = wide
        .resume(
            "inc-2",
            holding(&tasks, &[]),
            crate::engine::topology::select::Ceiling::unlimited(),
        )
        .expect("the next process resumes once the operator removed the registration");
    let runner = Arc::clone(&resumed.env.runner);
    let mut scheduler = Scheduler::first(&runner);
    let progress = drive(&mut resumed, Some(&mut scheduler)).expect("the resumed run completes");
    drop(scheduler);
    assert_eq!(outcome_of(&progress), RunOutcome::Complete);
    let kinds = kinds_of_log(&resumed);
    assert_eq!(
        count(
            &resumed.env.durable_events(),
            "merge_verification_interrupted"
        ),
        1,
        "{kinds:?}"
    );
    assert_eq!(
        count(
            &resumed.env.durable_events(),
            "merge_verification_unavailable"
        ),
        0,
        "{kinds:?}"
    );
    assert_eq!(count(&resumed.env.durable_events(), "task_merged"), 2);
    let sequences: std::collections::BTreeSet<u32> = resumed
        .env
        .durable_events()
        .iter()
        .filter_map(|event| match &event.body {
            TopologyEventBody::MergeVerificationStarted { data } => Some(data.sequence.0),
            _ => None,
        })
        .collect();
    assert!(
        sequences.len() >= 2,
        "the interrupted verification verified again under a new sequence: {kinds:?}"
    );
    replay_equals_live(&resumed);
}

fn planted_alone(
    tag: &str,
    watchdog: Duration,
) -> (
    crate::rundir::scratch_tree::ScratchTree,
    TearsAForeignRegistration,
    PathBuf,
    PathBuf,
) {
    let tree = crate::workspace_manager::fixture::scratch(tag);
    let common = tree.path().join("common");
    let admin = common.join("worktrees").join("foreign");
    let policy = TearsAForeignRegistration::new(
        admin.clone(),
        "a".repeat(40),
        common.clone(),
        true,
        watchdog,
    );
    policy
        .problem(&tree.path().join("merge").join("s1"), "")
        .expect("the verification plants the tear");
    (tree, policy, admin, common)
}

#[test]
fn a_foreign_writer_whose_handshake_never_arrives_writes_nothing_and_says_so() {
    let (_tree, policy, admin, _common) =
        planted_alone("foreign-writer-unmet", Duration::from_millis(20));
    let ended = policy.settle(false);
    assert!(
        matches!(&ended, Err(why) if why.contains("within 20ms") && why.contains("wrote nothing")),
        "the writer reports its watchdog: {ended:?}"
    );
    assert_eq!(
        std::fs::read(admin.join("commondir")).expect("the planted commondir"),
        b"",
        "a writer whose handshake never came leaves the tear as planted"
    );
}

#[test]
fn a_foreign_writer_is_cancelled_and_joined_before_its_fixture_is_reclaimed() {
    let (tree, policy, admin, common) = planted_alone("foreign-writer-joined", BOUND);
    let root = tree.path().to_path_buf();
    let ended = policy.finish();
    assert!(
        matches!(&ended, Err(why) if why.contains("cancelled") && why.contains("wrote nothing")),
        "the writer was cancelled and joined, and says so: {ended:?}"
    );
    assert_eq!(
        std::fs::read(admin.join("commondir")).expect("the planted commondir"),
        b"",
        "the cancelled writer wrote nothing"
    );
    drop(policy);
    drop(tree);
    assert!(!root.exists(), "the fixture's guard reclaimed its root");
    {
        let mut table = crate::workspace_manager::CONTENDED_ATTEMPTS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let count = table.entry(common).or_default();
        *count = count.saturating_add(1);
    }
    assert!(
        !root.exists(),
        "nothing recreated the reclaimed fixture at {}",
        root.display()
    );
}

fn lone_plant(tag: &str) -> (crate::rundir::scratch_tree::ScratchTree, Plant) {
    let tree = crate::workspace_manager::fixture::scratch(tag);
    let common = tree.path().join("common");
    let plant = Plant {
        admin: common.join("worktrees").join("foreign"),
        head: "a".repeat(40),
        runner: Arc::new(RecordingRunner::new()),
        wakes: Wakes::Contended,
        common,
        torn: Torn::CommondirEmpty,
    };
    (tree, plant)
}

fn cancelled_and_joined(prober: Prober) -> Result<(), String> {
    let _ = prober.cancel.send(());
    prober
        .handle
        .join()
        .unwrap_or_else(|_| Err("the prober panicked".to_owned()))
}

#[test]
fn a_prober_cancelled_after_the_attempt_it_waits_for_reports_it_and_finishes_the_tear() {
    let (_tree, plant) = lone_plant("prober-cancelled-after");
    let prober = plant.tear_sampling_every(BOUND);
    {
        let mut table = crate::workspace_manager::CONTENDED_ATTEMPTS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let count = table.entry(plant.common.clone()).or_default();
        *count = count.saturating_add(1);
    }
    assert_eq!(
        cancelled_and_joined(prober),
        Ok(()),
        "an attempt counted before the cancel was sent is what the prober waited for, however \
             long the wait the cancel ended"
    );
    assert_eq!(
        std::fs::read(plant.admin.join("commondir")).expect("the finished commondir"),
        b"../..\n",
        "the prober finished the tear"
    );
}

#[test]
fn a_prober_cancelled_before_anything_it_waits_for_leaves_the_tear_and_says_so() {
    let (_tree, plant) = lone_plant("prober-cancelled-before");
    let prober = plant.tear_sampling_every(BOUND);
    let ended = cancelled_and_joined(prober);
    assert!(
        matches!(&ended, Err(why) if why.contains("no registry access failed on the tear")
            && why.contains("stayed torn")),
        "the prober says what it did not see: {ended:?}"
    );
    assert_eq!(
        std::fs::read(plant.admin.join("commondir")).expect("the planted commondir"),
        b"",
        "the tear stands as planted"
    );
}

fn served_through(tag: &str, tasks: &[WideTask], at: TearAt) {
    let (what, tag, tasks) = (tag.to_owned(), tag.to_owned(), tasks.to_vec());
    bounded(&what, move || {
        let mut wide = Wide::started_with(
            &tag,
            &tasks,
            2,
            WidePlans::default(),
            RecordingRunner::new().answering(wide_responder(&tasks, &[])),
        );
        let common = wide.env.fixture.manager.common_git_dir().to_path_buf();
        let before = crate::workspace_manager::contended_attempts(&common);
        let mut hooks = TearHeld::new(&wide, "foreign-held", at);
        let pipelines = wide.env.pipelines();
        let slept = crate::workspace_manager::fixture::slept_pauses();
        let progress = wide
            .run
            .run_concurrently(&wide.env.seams(), &pipelines, &mut hooks, None)
            .expect("the access is passed once the coordinator granted a pipeline meanwhile");
        assert_eq!(
            crate::workspace_manager::fixture::slept_pauses(),
            slept,
            "no wait of a registry access the coordinator made slept on its thread"
        );
        hooks
            .finish()
            .expect("an invocation reached the runner while the access waited on the tear");
        assert_eq!(outcome_of(&progress), RunOutcome::Complete);
        assert!(
            crate::workspace_manager::contended_attempts(&common) > before,
            "the access failed on the tear first"
        );
        assert!(wide.run.invocations_balance());
    });
}

#[test]
fn a_pipeline_is_granted_while_a_dispatchs_head_check_waits_on_a_torn_registration() {
    served_through(
        "registry-served-head",
        &two_independent(),
        TearAt::Fold(attempt_started_of(0)),
    );
}

#[test]
fn a_pipeline_is_granted_while_a_dispatchs_revalidation_waits_on_a_torn_registration() {
    served_through(
        "registry-served-revalidate",
        &two_independent(),
        TearAt::Access(attempt_started_of(0), 2),
    );
}

#[test]
fn a_pipeline_is_granted_while_a_dispatchs_intent_waits_on_a_torn_registration() {
    served_through(
        "registry-served-intent",
        &two_independent(),
        TearAt::Fold(task_dispatched_of(1)),
    );
}

#[test]
fn a_pipeline_is_granted_while_a_dispatchs_add_gate_waits_on_a_torn_registration() {
    served_through(
        "registry-served-add-gate",
        &two_independent(),
        TearAt::Access(task_dispatched_of(1), 2),
    );
}

#[test]
fn a_pipeline_is_granted_while_a_dispatchs_add_waits_on_a_torn_registration() {
    served_through(
        "registry-served-add",
        &two_independent(),
        TearAt::Access(task_dispatched_of(1), 3),
    );
}

fn served_while_alpha_waits(tag: &str, failing: &'static [(u32, u32)], at: TearAt) {
    served_while_alpha_waits_with(tag, failing, at, RunOutcome::Complete, |_, _| {});
}

fn served_while_alpha_waits_with(
    tag: &str,
    failing: &'static [(u32, u32)],
    at: TearAt,
    outcome: RunOutcome,
    prepare: impl FnOnce(&Wide, &mut TearHeld) + Send + 'static,
) {
    let (what, tag) = (tag.to_owned(), tag.to_owned());
    bounded(&what, move || {
        let tasks = two_independent();
        let plans = WidePlans {
            gates: 2,
            ..WidePlans::default()
        };
        let mut wide = Wide::started_with(&tag, &tasks, 2, plans, holding(&tasks, failing));
        let common = wide.env.fixture.manager.common_git_dir().to_path_buf();
        let before = crate::workspace_manager::contended_attempts(&common);
        let mut hooks = TearHeld::new(&wide, "foreign-held", at);
        prepare(&wide, &mut hooks);
        let pipelines = wide.env.pipelines();
        let runner = Arc::clone(&wide.env.runner);
        let mut scheduler = Scheduler::scripted(
            &runner,
            Box::new(beta_settles_while_alpha_holds_its_first_gate),
        );
        let slept = crate::workspace_manager::fixture::slept_pauses();
        let progress = wide
            .run
            .run_concurrently(
                &wide.env.seams(),
                &pipelines,
                &mut hooks,
                Some(&mut scheduler),
            )
            .expect("the access is passed once the coordinator served a pipeline meanwhile");
        assert_eq!(
            crate::workspace_manager::fixture::slept_pauses(),
            slept,
            "no wait of a registry access the coordinator made slept on its thread"
        );
        drop(scheduler);
        hooks
            .finish()
            .expect("an invocation reached the runner while the access waited on the tear");
        drop(hooks);
        assert_eq!(outcome_of(&progress), outcome);
        assert!(
            crate::workspace_manager::contended_attempts(&common) > before,
            "the access failed on the tear first"
        );
        assert!(wide.run.invocations_balance());
    });
}

fn candidate_prepared_of_beta(body: &TopologyEventBody) -> bool {
    matches!(body, TopologyEventBody::CandidatePrepared { data } if data.key == TaskKey(0))
}

fn generation_closed_of_beta(body: &TopologyEventBody) -> bool {
    matches!(body, TopologyEventBody::GenerationClosed { data } if data.key == TaskKey(0))
}

#[test]
fn a_pipeline_is_served_while_a_settlements_path_read_waits_on_a_torn_registration() {
    served_while_alpha_waits(
        "registry-served-changed-paths",
        &[],
        TearAt::Access(attempt_started_of(1), 1),
    );
}

#[test]
fn a_pipeline_is_served_while_a_settlements_parent_check_waits_on_a_torn_registration() {
    served_while_alpha_waits(
        "registry-served-commit-parent",
        &[],
        TearAt::Fold(candidate_prepared_of_beta),
    );
}

#[test]
fn a_pipeline_is_served_while_a_settlements_tree_check_waits_on_a_torn_registration() {
    served_while_alpha_waits(
        "registry-served-commit-tree",
        &[],
        TearAt::Access(candidate_prepared_of_beta, 2),
    );
}

#[test]
fn a_pipeline_is_served_while_a_settlements_worktree_removal_waits_on_a_torn_registration() {
    served_while_alpha_waits_with(
        "registry-served-reclaim-worktree",
        &[],
        TearAt::Fold(candidate_created_of_beta),
        RunOutcome::Complete,
        |_, hooks| hooks.tearing(Torn::GitdirMissing),
    );
}

#[test]
fn a_pipeline_is_served_while_a_settlements_intent_removal_waits_on_a_torn_registration() {
    served_while_alpha_waits(
        "registry-served-reclaim-intent",
        &[],
        TearAt::Access(candidate_created_of_beta, 2),
    );
}

#[test]
fn a_pipeline_is_served_while_an_integrations_decision_waits_on_a_torn_registration() {
    served_while_alpha_waits(
        "registry-served-decide",
        &[],
        TearAt::Access(candidate_created_of_beta, 3),
    );
}

#[test]
fn a_pipeline_is_served_while_an_integrations_publication_waits_on_a_torn_registration() {
    served_while_alpha_waits(
        "registry-served-publish",
        &[],
        TearAt::Fold(merge_prepared_of_beta),
    );
}

#[test]
fn a_pipeline_is_served_while_a_publications_swap_recheck_waits_on_a_torn_registration() {
    served_while_alpha_waits(
        "registry-served-swap-recheck",
        &[],
        TearAt::Access(merge_prepared_of_beta, 2),
    );
}

#[test]
fn a_pipeline_is_served_while_a_retrys_worktree_check_waits_on_a_torn_registration() {
    served_while_alpha_waits(
        "registry-served-retry-verify",
        &[(0, 1)],
        TearAt::Fold(retained_attempt_finished_of_beta),
    );
}

/// The follow-up C record's §6.11: C's instance-only repair (C-I1) waits
/// through the coordinator. The residue is an earlier incarnation's: its
/// intent of a slot, and the registration of its own instance of that slot,
/// torn as a killed `git worktree add` leaves it. The repair's plan names
/// that instance by its tag, and the removal that takes it meets a foreign
/// registration torn at its own start; a pipeline is served while it
/// waits. At `905ed0c8`, the provisional merge of #329's head `ce55ca91`
/// (not #329's final head), the instance's removal slept on the
/// coordinator's thread, no pipeline was served, and the access refused
/// at its deadline.
fn served_while_an_earlier_instances_repair_waits(
    tag: &str,
    failing: &[(u32, u32)],
    at: TearAt,
    torn: Torn,
) {
    let (what, tag, failing) = (tag.to_owned(), tag.to_owned(), failing.to_vec());
    bounded(&what, move || {
        let tasks = two_independent();
        let plans = WidePlans {
            gates: 2,
            ..WidePlans::default()
        };
        let mut wide = Wide::started_with(&tag, &tasks, 2, plans, holding(&tasks, &failing));
        let common = wide.env.fixture.manager.common_git_dir().to_path_buf();
        let leftover = common.join("worktrees").join("earlier-residue");
        let mut hooks = TearHeld::new(&wide, "foreign-held", at);
        hooks.tearing(torn);
        let earlier = WorkspaceManager::derive(
            &wide.env.fixture.base,
            &wide.env.fixture.private,
            crate::workspace_manager::fixture::RUN_ID,
            "inc-earlier",
        )
        .expect("a manager of the run's earlier incarnation");
        assert_ne!(
            earlier.instance_tag(),
            wide.env.fixture.manager.instance_tag(),
            "the residue is another incarnation's"
        );
        let slot = crate::engine::topology::dispatch::task_slot(TaskKey(9), GenerationId(0));
        earlier
            .write_intent(&mut crate::workspace_manager::NoHooks, &slot)
            .expect("the earlier incarnation's intent");
        let checkout = earlier.slot_path(&slot).join(".git");
        let (admin, head) = (leftover.clone(), wide.env.fixture.head.clone());
        let (TearAt::Fold(when) | TearAt::Access(when, _)) = at;
        hooks.also = Some((
            when,
            Box::new(move || {
                crate::workspace_manager::fixture::write_file(
                    &admin.join("HEAD"),
                    format!("{head}\n").as_bytes(),
                );
                crate::workspace_manager::fixture::write_file(
                    &admin.join("gitdir"),
                    format!(
                        "{}\n",
                        crate::workspace_manager::fixture::as_git_writes_it(&checkout)
                    )
                    .as_bytes(),
                );
                crate::workspace_manager::fixture::write_file(&admin.join("commondir"), b"");
            }),
        ));
        let planted = hooks.planted();
        let pipelines = wide.env.pipelines();
        let runner = Arc::clone(&wide.env.runner);
        let mut scheduler = Scheduler::scripted(&runner, alpha_waits_for_the_tear(planted));
        let contended = crate::workspace_manager::contended_attempts(&common);
        let slept = crate::workspace_manager::fixture::slept_pauses();
        let progress = wide
            .run
            .run_concurrently(
                &wide.env.seams(),
                &pipelines,
                &mut hooks,
                Some(&mut scheduler),
            )
            .expect("the access is passed once the coordinator served a pipeline meanwhile");
        assert_eq!(
            crate::workspace_manager::fixture::slept_pauses(),
            slept,
            "no wait of a registry access the coordinator made slept on its thread"
        );
        drop(scheduler);
        hooks
            .finish()
            .expect("an invocation reached the runner while the access waited on the tear");
        drop(hooks);
        assert_eq!(outcome_of(&progress), RunOutcome::Complete);
        assert!(
            crate::workspace_manager::contended_attempts(&common) > contended,
            "the access failed on the tear first"
        );
        assert!(
            !leftover.exists(),
            "the repair removed the earlier incarnation's torn instance"
        );
        assert!(wide.run.invocations_balance());
    });
}

#[test]
fn a_pipeline_is_served_while_an_earlier_instances_repair_removal_waits_on_a_torn_registration() {
    served_while_an_earlier_instances_repair_waits(
        "served-earlier-repair-removal",
        &[],
        TearAt::Access(candidate_created_of_beta, 4),
        Torn::GitdirMissing,
    );
}

#[test]
fn a_pipeline_is_served_while_a_verifications_earlier_instance_repair_removal_waits() {
    served_while_an_earlier_instances_repair_waits(
        "served-earlier-verify-repair",
        &[(0, 1)],
        TearAt::Access(retained_attempt_finished_of_beta, 3),
        Torn::GitdirMissing,
    );
}

#[test]
fn a_pipeline_is_served_while_an_intent_removals_repair_removal_waits_on_a_torn_registration() {
    served_while_a_repair_waits(
        "registry-served-intent-repair-removal",
        &[],
        TearAt::Access(candidate_created_of_beta, 4),
        Torn::GitdirMissing,
        true,
    );
}

#[test]
fn a_pipeline_is_served_while_an_intent_removals_second_revalidation_waits_on_a_torn_registration()
{
    served_while_a_repair_waits(
        "registry-served-intent-second-revalidation",
        &[],
        TearAt::Access(candidate_created_of_beta, 5),
        Torn::CommondirEmpty,
        true,
    );
}

#[test]
fn a_pipeline_is_served_while_a_verifications_repair_plan_waits_on_a_torn_registration() {
    served_while_a_repair_waits(
        "registry-served-verify-repair-plan",
        &[(0, 1)],
        TearAt::Access(retained_attempt_finished_of_beta, 2),
        Torn::GitdirMissing,
        true,
    );
}

#[test]
fn a_pipeline_is_served_while_a_verifications_second_revalidation_waits_on_a_torn_registration() {
    served_while_a_repair_waits(
        "registry-served-verify-second-revalidation",
        &[(0, 1)],
        TearAt::Access(retained_attempt_finished_of_beta, 4),
        Torn::CommondirEmpty,
        true,
    );
}

#[test]
fn a_pipeline_is_served_while_a_verifications_worktree_read_waits_on_a_torn_registration() {
    served_while_a_repair_waits(
        "registry-served-verify-worktree-read",
        &[(0, 1)],
        TearAt::Access(retained_attempt_finished_of_beta, 2),
        Torn::CommondirEmpty,
        false,
    );
}

fn closing_attempt_finished_of_beta(body: &TopologyEventBody) -> bool {
    matches!(body, TopologyEventBody::AttemptFinished { data }
        if data.key == TaskKey(0)
            && matches!(data.settlement, crate::topology::events::AttemptSettlement::Closed { .. }))
}

#[test]
fn a_pipeline_is_served_while_a_closed_retrys_worktree_scrub_waits_on_a_torn_registration() {
    served_while_a_closed_retry_scrubs("registry-served-retry-close-worktree", Torn::GitdirMissing);
}

#[test]
fn a_pipeline_is_served_while_a_closed_retrys_intent_scrub_waits_on_a_torn_registration() {
    served_while_a_closed_retry_scrubs("registry-served-retry-close-intent", Torn::CommondirEmpty);
}

fn served_while_a_closed_retry_scrubs(tag: &str, torn: Torn) {
    served_while_alpha_waits_with(
        tag,
        &[(0, 1)],
        TearAt::Fold(generation_closed_of_beta),
        RunOutcome::Complete,
        move |wide, hooks| {
            hooks.tearing(torn);
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
                Box::new(move || {
                    crate::workspace_manager::fixture::write_file(
                        &slot.join("left-by-hand.txt"),
                        b"the retained worktree no longer holds the retained tree\n",
                    );
                    crate::workspace_manager::fixture::git(&slot, &["add", "left-by-hand.txt"]);
                }),
            ));
        },
    );
}

#[test]
fn a_pipeline_is_served_while_a_failed_settlements_worktree_scrub_waits_on_a_torn_registration() {
    served_while_a_failed_settlement_scrubs(
        "registry-served-settle-scrub-worktree",
        Torn::GitdirMissing,
    );
}

#[test]
fn a_pipeline_is_served_while_a_failed_settlements_intent_scrub_waits_on_a_torn_registration() {
    served_while_a_failed_settlement_scrubs(
        "registry-served-settle-scrub-intent",
        Torn::CommondirEmpty,
    );
}

fn served_while_a_failed_settlement_scrubs(tag: &str, torn: Torn) {
    served_while_alpha_waits_with(
        tag,
        &[(0, 1), (0, 2)],
        TearAt::Fold(closing_attempt_finished_of_beta),
        RunOutcome::Parked,
        move |_, hooks| hooks.tearing(torn),
    );
}

fn served_while_a_stale_integration_waits(tag: &str, conflicting: bool, at: TearAt) {
    served_while_a_stale_integration_waits_under(
        tag,
        conflicting,
        at,
        Torn::CommondirEmpty,
        alpha_waits_for_gammas_integration,
    );
}

fn served_while_a_stale_integration_waits_under(
    tag: &str,
    conflicting: bool,
    at: TearAt,
    torn: Torn,
    order: fn(&Quiescent<'_>) -> Option<Release>,
) {
    let (what, tag) = (tag.to_owned(), tag.to_owned());
    bounded(&what, move || {
        let tasks = [
            WideTask::independent("beta"),
            WideTask::independent("alpha"),
            if conflicting {
                WideTask::hinted("gamma", &["src/gamma/"], "src/beta/work.txt")
            } else {
                WideTask::independent("gamma")
            },
        ];
        let mut wide =
            Wide::started_with(&tag, &tasks, 3, WidePlans::default(), holding(&tasks, &[]));
        let common = wide.env.fixture.manager.common_git_dir().to_path_buf();
        let before = crate::workspace_manager::contended_attempts(&common);
        let mut hooks = TearHeld::waking(&wide, "foreign-held", at, Wakes::Ended);
        hooks.tearing(torn);
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
            .expect("the access is passed once the coordinator served a pipeline meanwhile");
        assert_eq!(
            crate::workspace_manager::fixture::slept_pauses(),
            slept,
            "no wait of a registry access the coordinator made slept on its thread"
        );
        drop(scheduler);
        hooks
            .finish()
            .expect("an invocation left the runner while the access waited on the tear");
        drop(hooks);
        assert_eq!(
            outcome_of(&progress),
            if conflicting {
                RunOutcome::Parked
            } else {
                RunOutcome::Complete
            },
            "a conflict's repair leaves the scaffold's conflict unresolved and parks"
        );
        assert!(
            crate::workspace_manager::contended_attempts(&common) > before,
            "the access failed on the tear first"
        );
        let events = wide.env.durable_events();
        assert_eq!(
            count(&events, "merge_rejected"),
            usize::from(conflicting),
            "{:?}",
            kinds_of(&events)
        );
        assert!(wide.run.invocations_balance());
    });
}

#[test]
fn a_pipeline_is_served_while_a_stale_integrations_intent_waits_on_a_torn_registration() {
    served_while_a_stale_integration_waits(
        "registry-served-stale-intent",
        false,
        TearAt::Access(candidate_created_of_gamma, 4),
    );
}

#[test]
fn a_pipeline_is_served_while_a_stale_integrations_add_gate_waits_on_a_torn_registration() {
    served_while_a_stale_integration_waits(
        "registry-served-stale-add-gate",
        false,
        TearAt::Access(candidate_created_of_gamma, 5),
    );
}

#[test]
fn a_pipeline_is_served_while_a_stale_integrations_add_waits_on_a_torn_registration() {
    served_while_a_stale_integration_waits(
        "registry-served-stale-add",
        false,
        TearAt::Access(candidate_created_of_gamma, 6),
    );
}

#[test]
fn a_pipeline_is_served_while_a_stale_integrations_pick_waits_on_a_torn_registration() {
    served_while_a_stale_integration_waits(
        "registry-served-stale-pick",
        false,
        TearAt::Access(candidate_created_of_gamma, 7),
    );
}

#[test]
fn a_pipeline_is_served_while_a_publications_staging_removal_waits_on_a_torn_registration() {
    served_while_a_stale_integration_waits_under(
        "registry-served-staging-removal",
        false,
        TearAt::Fold(task_merged_of_gamma),
        Torn::GitdirMissing,
        alpha_waits_for_gammas_merge,
    );
}

#[test]
fn a_pipeline_is_served_while_a_publications_staging_intent_removal_waits_on_a_torn_registration() {
    served_while_a_stale_integration_waits_under(
        "registry-served-staging-intent-removal",
        false,
        TearAt::Fold(task_merged_of_gamma),
        Torn::CommondirEmpty,
        alpha_waits_for_gammas_merge,
    );
}

/// The follow-up C record's §6.11, the proposed frozen hunk C-R1: an
/// integration's snapshot reclaim enumerates through its journal's hooks
/// (`integrate.rs`'s `reclaim_snapshots`, `intents_pausing`), so the registry
/// read that discovers earlier incarnations' instances answers the
/// coordinator's messages while a pipeline is live. It is the eighth registry
/// access after gamma's candidate. Without C-R1 (`2c3e413c`) the read slept
/// on the coordinator's thread to its deadline, no pipeline was served, and
/// the integration was refused.
#[test]
fn a_pipeline_is_served_while_an_integrations_snapshot_discovery_waits_on_an_unreadable_registration()
 {
    served_while_a_stale_integration_waits_under(
        "served-integration-discovery",
        false,
        TearAt::Access(candidate_created_of_gamma, 8),
        Torn::GitdirUnreadable,
        alpha_waits_for_gammas_merge,
    );
}

#[test]
fn a_pipeline_is_served_while_a_conflicts_classification_waits_on_a_torn_registration() {
    served_while_a_stale_integration_waits(
        "registry-served-proposal-state",
        true,
        TearAt::Access(candidate_created_of_gamma, 8),
    );
}

#[test]
fn a_pipeline_is_served_while_a_rejections_staging_reclaim_waits_on_a_torn_registration() {
    served_while_a_stale_integration_waits_under(
        "registry-served-staging-reclaim",
        true,
        TearAt::Fold(merge_rejected_of_gamma),
        Torn::GitdirMissing,
        alpha_waits_for_gammas_integration,
    );
}

#[test]
fn a_pipeline_is_served_while_a_rejections_staging_intent_reclaim_waits_on_a_torn_registration() {
    served_while_a_stale_integration_waits(
        "registry-served-staging-intent-reclaim",
        true,
        TearAt::Fold(merge_rejected_of_gamma),
    );
}

#[test]
fn a_pipeline_is_served_while_a_repairs_materialization_waits_on_a_torn_registration() {
    served_while_a_stale_integration_waits(
        "registry-served-repair-materialize",
        true,
        TearAt::Access(repair_dispatched, 4),
    );
}

#[test]
fn a_pipeline_is_granted_while_a_continued_dispatchs_worktree_check_waits_on_a_torn_registration() {
    let what = "a_pipeline_is_granted_while_a_continued_dispatchs_worktree_check_waits_on_a_torn_registration";
    bounded(what, move || {
        let tasks = [
            WideTask::independent("alpha"),
            WideTask::independent("beta"),
        ];
        let mut wide = Wide::durable(
            "registry-served-continued",
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
        let (_, mut resumed) = wide
            .resume(
                "inc-2",
                RecordingRunner::new().answering(wide_responder(&tasks, &[])),
                crate::engine::topology::select::Ceiling::unlimited(),
            )
            .expect("the next process resumes with beta's generation open and no attempt");
        let common = resumed.env.fixture.manager.common_git_dir().to_path_buf();
        let before = crate::workspace_manager::contended_attempts(&common);
        let mut hooks = TearHeld::new(
            &resumed,
            "foreign-held",
            TearAt::Access(attempt_started_of(0), 1),
        );
        let pipelines = resumed.env.pipelines();
        let slept = crate::workspace_manager::fixture::slept_pauses();
        let progress = resumed
            .run
            .run_concurrently(&resumed.env.seams(), &pipelines, &mut hooks, None)
            .expect("the continuation's access is passed once alpha was granted meanwhile");
        assert_eq!(
            crate::workspace_manager::fixture::slept_pauses(),
            slept,
            "no wait of a registry access the coordinator made slept on its thread"
        );
        hooks
            .finish()
            .expect("an invocation reached the runner while the continuation waited on the tear");
        drop(hooks);
        assert_eq!(outcome_of(&progress), RunOutcome::Complete);
        assert!(
            crate::workspace_manager::contended_attempts(&common) > before,
            "the continuation's access failed on the tear first"
        );
        let events = resumed.env.durable_events();
        let dispatched_beta = events
            .iter()
            .filter(|event| {
                matches!(&event.body, TopologyEventBody::TaskDispatched { data } if data.key == TaskKey(1))
            })
            .count();
        assert_eq!(
            dispatched_beta,
            1,
            "beta's generation was continued, not dispatched again: {:?}",
            kinds_of(&events)
        );
        assert!(resumed.run.invocations_balance());
    });
}

#[test]
fn the_width_one_step_runs_the_same_transitions_and_its_access_waits_by_sleeping() {
    let what = "the_width_one_step_runs_the_same_transitions_and_its_access_waits_by_sleeping";
    bounded(what, move || {
        let tasks = two_independent();
        let mut narrow = Wide::started_with(
            "registry-width-one",
            &tasks,
            1,
            WidePlans::default(),
            RecordingRunner::new().answering(wide_responder(&tasks, &[])),
        );
        let common = narrow.env.fixture.manager.common_git_dir().to_path_buf();
        let before = crate::workspace_manager::contended_attempts(&common);
        let mut hooks = TearHeld::waking(
            &narrow,
            "foreign-held",
            TearAt::Fold(task_dispatched_of(1)),
            Wakes::Contended,
        );
        let seams = narrow.env.seams();
        let slept = crate::workspace_manager::fixture::slept_pauses();
        let mut steps = 0_u32;
        let outcome = loop {
            steps += 1;
            assert!(steps < 200, "the width-1 loop did not finish");
            if let Progress::Finished { outcome, .. } =
                narrow.run.step(&seams, &mut hooks).expect("a step")
            {
                break outcome;
            }
        };
        hooks
            .finish()
            .expect("the dispatch's access failed on the tear, and only then was it finished");
        drop(hooks);
        assert_eq!(outcome, RunOutcome::Complete);
        assert!(
            crate::workspace_manager::fixture::slept_pauses() > slept,
            "the width-1 access waited by sleeping on its own thread"
        );
        assert!(
            crate::workspace_manager::contended_attempts(&common) > before,
            "the width-1 dispatch's intent met the tear and was attempted again"
        );
        assert!(narrow.run.invocations_balance());
    });
}

struct FoldsSeen {
    inner: crate::engine::topology::seams::HarnessTopologyHooks,
    seen: Vec<&'static str>,
}

impl TopologyHooks for FoldsSeen {
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
        TopologyHooks::spawn(&mut self.inner)
    }

    fn folded(&mut self, fold: &TopologyFold, events: &[TopologyEvent]) {
        self.inner.folded(fold, events);
        if let Some(last) = events.last() {
            self.seen.push(last.body.kind());
        }
    }
}

#[test]
fn every_append_of_the_width_one_step_a_retrys_settlement_included_folds_through_the_callers_hooks()
{
    let tasks = two_independent();
    let plans = WidePlans {
        gates: 2,
        ..WidePlans::default()
    };
    let mut narrow = Wide::started_with(
        "width-one-retry-hooks",
        &tasks,
        1,
        plans,
        RecordingRunner::new().answering(wide_responder(&tasks, &[(0, 1)])),
    );
    let mut hooks = FoldsSeen {
        inner: narrow.env.hooks(),
        seen: Vec::new(),
    };
    let seams = narrow.env.seams();
    let before = narrow.run.events().len();
    let mut steps = 0_u32;
    let mut retried = false;
    loop {
        steps += 1;
        assert!(steps < 200, "the width-1 loop did not finish");
        let retrying = matches!(narrow.run.admitted(), Ok(Admitted::Retry { .. }));
        let progress = narrow.run.step(&seams, &mut hooks).expect("a step");
        if retrying {
            retried = true;
            assert!(
                matches!(progress, Progress::Settled { accepted: true, .. }),
                "the retry was judged and settled in the same step: {progress:?}"
            );
        }
        if matches!(progress, Progress::Finished { .. }) {
            break;
        }
    }
    assert!(
        retried,
        "the width-1 step retried beta's retained generation"
    );
    let appended = kinds_of(&narrow.run.events()[before..]);
    assert_eq!(
        hooks.seen, appended,
        "every event the width-1 step appended, the retry's settlement included, was folded \
             through the hooks its caller passed"
    );
    assert_eq!(
        narrow
            .run
            .events()
            .iter()
            .filter(|event| matches!(&event.body,
                TopologyEventBody::CandidatePrepared { data } if data.key == TaskKey(0)))
            .count(),
        1,
        "beta's retry settled as its candidate: {appended:?}"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn a_wait_answers_a_queued_shutdown_when_no_thread_can_be_started() {
    bounded("a wait under thread exhaustion", || {
        let tasks = [WideTask::independent("alpha")];
        let mut wide = Wide::started_with(
            "wait-thread-exhaustion",
            &tasks,
            1,
            WidePlans::default(),
            holding(&tasks, &[]),
        );
        let mut hooks = wide.env.hooks();
        let seams = wide.env.seams();
        let pipelines = wide.env.pipelines();
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
            .expect("shutdown is queued");
        crate::workspace_manager::fixture::refuse_syscall_on_this_thread(
            libc::SYS_clone3,
            libc::EAGAIN,
        );
        let refused = std::thread::Builder::new()
            .spawn(|| ())
            .map(|thread| thread.join().is_ok())
            .expect_err("no thread can be started on this thread any more");
        assert_eq!(refused.raw_os_error(), Some(libc::EAGAIN), "{refused}");
        let slept = crate::workspace_manager::fixture::slept_pauses();
        let stopped = crate::workspace_manager::EffectHooks::registry_pause(
            &mut coordinator,
            Duration::from_millis(30),
        );
        assert!(
            matches!(coordinator.interrupt, Some(Interrupt::Shutdown)),
            "the wait answered the queued shutdown"
        );
        assert_eq!(coordinator.inbox.len(), 0, "nothing was left unanswered");
        assert_eq!(
            crate::workspace_manager::fixture::slept_pauses(),
            slept,
            "the coordinator never slept for the wait"
        );
        assert!(
            stopped
                .expect_err("the wait that answered a shutdown ends in an error")
                .to_string()
                .contains("met a shutdown")
        );
        assert!(
            !coordinator
                .run
                .warnings()
                .iter()
                .any(|warning| warning.contains("slept")),
            "{:?}",
            coordinator.run.warnings()
        );
    });
}

#[cfg(target_os = "linux")]
#[test]
fn a_coordinator_whose_timer_cannot_start_refuses_before_it_appends_anything() {
    bounded("a coordinator under thread exhaustion", || {
        let tasks = two_independent();
        let mut wide = Wide::started_with(
            "timer-thread-exhaustion",
            &tasks,
            2,
            WidePlans::default(),
            holding(&tasks, &[]),
        );
        let mut hooks = wide.env.hooks();
        let pipelines = wide.env.pipelines();
        let before = wide.run.events().len();
        let slept = crate::workspace_manager::fixture::slept_pauses();
        crate::workspace_manager::fixture::refuse_syscall_on_this_thread(
            libc::SYS_clone3,
            libc::EAGAIN,
        );
        let error = wide
            .run
            .run_concurrently(&wide.env.seams(), &pipelines, &mut hooks, None)
            .expect_err("a coordinator that cannot start its timer refuses");
        assert!(
            matches!(&error, UpstrokeError::Refused { message }
                if message.contains("wait timer could not be started")
                    && message.contains("nothing was spawned or appended")),
            "{error:?}"
        );
        assert_eq!(wide.run.events().len(), before, "nothing was appended");
        assert!(wide.env.runner.ran().is_empty(), "nothing was spawned");
        assert_eq!(
            crate::workspace_manager::fixture::slept_pauses(),
            slept,
            "nothing slept"
        );
    });
}

fn unwinding_sleep(_: Duration) {
    panic!("the timer's sleep unwinds, as a timer thread that dies would");
}

#[test]
fn a_wait_whose_timer_unwinds_is_still_woken_and_the_timer_serves_the_next() {
    bounded("a wait whose timer unwinds", || {
        let tasks = [WideTask::independent("alpha")];
        let mut wide = Wide::started_with(
            "timer-unwinds",
            &tasks,
            1,
            WidePlans::default(),
            holding(&tasks, &[]),
        );
        let mut hooks = wide.env.hooks();
        let seams = wide.env.seams();
        let pipelines = wide.env.pipelines();
        let mut coordinator = coordinator_of(
            &mut wide.run,
            &seams,
            &mut hooks,
            &pipelines,
            BTreeMap::new(),
        );
        coordinator.timer = Timer::starting(coordinator.outbox.clone(), unwinding_sleep)
            .expect("the wait timer starts");
        for wait in 1..=2 {
            crate::workspace_manager::EffectHooks::registry_pause(
                &mut coordinator,
                Duration::from_millis(5),
            )
            .unwrap_or_else(|error| {
                panic!("wait {wait} was answered by its wake and nothing else: {error}")
            });
        }
        assert!(coordinator.interrupt.is_none());
        coordinator.join();
        assert!(
            coordinator
                .run
                .warnings()
                .iter()
                .any(|warning| warning.contains("unwound inside 2 of its sleeps")),
            "{:?}",
            coordinator.run.warnings()
        );
    });
}

#[test]
fn a_wait_that_begins_after_a_shutdown_does_not_wait() {
    bounded("a wait after a shutdown", || {
        let tasks = [WideTask::independent("alpha")];
        let mut wide = Wide::started_with(
            "wait-after-shutdown",
            &tasks,
            1,
            WidePlans::default(),
            holding(&tasks, &[]),
        );
        let mut hooks = wide.env.hooks();
        let seams = wide.env.seams();
        let pipelines = wide.env.pipelines();
        let mut coordinator = coordinator_of(
            &mut wide.run,
            &seams,
            &mut hooks,
            &pipelines,
            BTreeMap::new(),
        );
        coordinator
            .handle(Origin::Pipeline, ToCoordinator::Shutdown)
            .expect("the shutdown is applied");
        let started = std::time::Instant::now();
        let stopped = crate::workspace_manager::EffectHooks::registry_pause(
            &mut coordinator,
            Duration::from_secs(60),
        )
        .expect_err("a wait that begins after a shutdown stops at once");
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "the wait did not wait out its length"
        );
        assert!(stopped.to_string().contains("met a shutdown"), "{stopped}");
        assert_eq!(coordinator.wakes, 0, "no wake was asked for");
    });
}

#[test]
fn a_wait_after_its_timer_has_stopped_refuses_at_once_and_never_sleeps() {
    bounded("a wait with no timer", || {
        let tasks = [WideTask::independent("alpha")];
        let mut wide = Wide::started_with(
            "timer-stopped",
            &tasks,
            1,
            WidePlans::default(),
            holding(&tasks, &[]),
        );
        let mut hooks = wide.env.hooks();
        let seams = wide.env.seams();
        let pipelines = wide.env.pipelines();
        let mut coordinator = coordinator_of(
            &mut wide.run,
            &seams,
            &mut hooks,
            &pipelines,
            BTreeMap::new(),
        );
        assert_eq!(coordinator.timer.stop(), None);
        let slept = crate::workspace_manager::fixture::slept_pauses();
        let started = std::time::Instant::now();
        let stopped = crate::workspace_manager::EffectHooks::registry_pause(
            &mut coordinator,
            Duration::from_secs(60),
        )
        .expect_err("a wait that cannot be timed refuses");
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "the wait did not wait out its length"
        );
        assert_eq!(crate::workspace_manager::fixture::slept_pauses(), slept);
        assert!(
            matches!(&coordinator.interrupt, Some(Interrupt::Failed(UpstrokeError::Refused { message }))
                if message.contains("wait timer has stopped")),
            "the command ends on the typed refusal"
        );
        assert!(
            stopped
                .to_string()
                .contains("an error that ends the command"),
            "{stopped}"
        );
    });
}

#[cfg(unix)]
#[test]
fn a_verification_whose_snapshot_checkout_fails_after_the_takeover_ends_resumably_and_its_resume_reverifies()
 {
    let tasks = [
        WideTask::independent("alpha"),
        WideTask::independent("beta"),
    ];
    let mut wide = Wide::durable(
        "registry-verification-genuine",
        &tasks,
        2,
        WidePlans::default(),
        holding(&tasks, &[]),
    );
    let base = wide.env.fixture.base.clone();
    let info = wide.env.fixture.manager.common_git_dir().join("info");
    let policy = Arc::new(RequiresAFailingFilter {
        base: base.clone(),
        info: info.clone(),
        remaining: std::sync::atomic::AtomicUsize::new(1),
    });
    let error = match registry_drive(&mut wide, policy) {
        Err(error) => error,
        Ok(progress) => panic!(
            "a snapshot whose checkout fails after the takeover ends the command, not \
                 {progress:?}: {:?}",
            kinds_of_log(&wide)
        ),
    };
    assert!(
        matches!(&error, UpstrokeError::RegistryRefused { message } if message.contains("b329fails") || message.contains("is gone")),
        "{error:?}"
    );
    let kinds = kinds_of_log(&wide);
    assert!(
        !kinds.contains(&"merge_verification_unavailable") && !kinds.contains(&"run_finished"),
        "nothing durable: {kinds:?}"
    );
    assert_eq!(
        kinds.last(),
        Some(&"merge_verification_started"),
        "{kinds:?}"
    );
    crate::workspace_manager::fixture::remove_file(&info.join("attributes"));
    crate::workspace_manager::fixture::git(
        &base,
        &["config", "--remove-section", "filter.b329fails"],
    );
    let (_, mut resumed) = wide
        .resume(
            "inc-2",
            holding(&tasks, &[]),
            crate::engine::topology::select::Ceiling::unlimited(),
        )
        .expect("the next process resumes once the environment is repaired");
    let runner = Arc::clone(&resumed.env.runner);
    let mut scheduler = Scheduler::first(&runner);
    let progress = drive(&mut resumed, Some(&mut scheduler)).expect("the resumed run completes");
    drop(scheduler);
    assert_eq!(outcome_of(&progress), RunOutcome::Complete);
    let events = resumed.env.durable_events();
    assert_eq!(count(&events, "merge_verification_interrupted"), 1);
    assert_eq!(count(&events, "merge_verification_unavailable"), 0);
    assert_eq!(count(&events, "task_merged"), 2);
    replay_equals_live(&resumed);
}

#[cfg(unix)]
struct DeniesTheSnapshots {
    snapshots: PathBuf,
    remaining: std::sync::atomic::AtomicUsize,
}

#[cfg(unix)]
impl ReviewInputPolicy for DeniesTheSnapshots {
    fn problem(
        &self,
        worktree: &std::path::Path,
        _tree: &str,
    ) -> Result<Option<String>, UpstrokeError> {
        let verifying = worktree.parent().and_then(std::path::Path::file_name)
            == Some(std::ffi::OsStr::new("merge"));
        if verifying
            && self
                .remaining
                .compare_exchange(
                    1,
                    0,
                    std::sync::atomic::Ordering::SeqCst,
                    std::sync::atomic::Ordering::SeqCst,
                )
                .is_ok()
        {
            crate::workspace_manager::fixture::set_mode(&self.snapshots, 0o555);
        }
        Ok(None)
    }
}

#[cfg(unix)]
struct RestoresTheSnapshots {
    inner: crate::engine::topology::seams::HarnessTopologyHooks,
    snapshots: PathBuf,
}

#[cfg(unix)]
impl crate::engine::topology::seams::TopologyHooks for RestoresTheSnapshots {
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
            .is_some_and(|event| event.body.kind() == "merge_verification_unavailable")
        {
            crate::workspace_manager::fixture::set_mode(&self.snapshots, 0o755);
        }
    }
}

#[cfg(unix)]
#[test]
fn a_verification_whose_snapshot_destination_cannot_be_made_defers_as_before() {
    let tasks = [
        WideTask::independent("alpha"),
        WideTask::independent("beta"),
    ];
    let mut wide = Wide::started_with(
        "registry-verification-destination",
        &tasks,
        2,
        WidePlans::default(),
        holding(&tasks, &[]),
    );
    let snapshots = wide.env.fixture.manager.execution_root().join("snapshots");
    let _restore = crate::workspace_manager::fixture::ModeRestored::of(&snapshots);
    let mut pipelines = wide.env.pipelines();
    pipelines.input_policy = Arc::new(DeniesTheSnapshots {
        snapshots: snapshots.clone(),
        remaining: std::sync::atomic::AtomicUsize::new(1),
    });
    let mut hooks = RestoresTheSnapshots {
        inner: wide.env.hooks(),
        snapshots,
    };
    let runner = Arc::clone(&wide.env.runner);
    let mut scheduler = Scheduler::first(&runner);
    let progress = wide
        .run
        .run_concurrently(
            &wide.env.seams(),
            &pipelines,
            &mut hooks,
            Some(&mut scheduler),
        )
        .expect("a destination that cannot be made is Git state the verification defers on");
    drop(scheduler);
    assert_eq!(outcome_of(&progress), RunOutcome::Complete);
    let events = wide.run.events();
    assert_eq!(
        count(events, "merge_verification_unavailable"),
        1,
        "{:?}",
        kinds_of(events)
    );
    assert_eq!(count(events, "task_merged"), 2);
    replay_equals_live(&wide);
}
