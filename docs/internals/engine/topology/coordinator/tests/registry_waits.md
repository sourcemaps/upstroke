# `src/engine/topology/coordinator/tests/registry_waits.rs`

Extended notes for [`src/engine/topology/coordinator/tests/registry_waits.rs`](../../../../../../src/engine/topology/coordinator/tests/registry_waits.rs).

The code is the authority for what it does; this file is the whole of its prose but for the
concurrency protocol standards §10 places at its site. Each section
here is headed by the line of code it describes, spelled as it is in the source, so the heading is
the grep string that finds the code.

## `fn registry_drive(`

`run_concurrently` with the review-input policy replaced, and the first-released scheduler.

## `fn a_verification_beside_another_processs_registration_write_in_flight_spends_no_deferral() {`

The record's T2: a registration another process finishes writing never reaches the verification
as Git state. The verification's registry access fails on the write in flight, is attempted
again, and passes once the writer finishes; nothing is deferred and both candidates merge. The
policy's `finish` returns `Ok`: an access failed on the tear, and only then did the writer finish
it.
Before #329 the same interleaving was durable: one `merge_verification_unavailable` (Deferred),
or a park at `max_defers` (round 3's measurement of the round 8 witness). Its repository's accesses
wait to `WITNESS_REGISTRY_DEADLINE` (§9.31).

## `fn a_verification_beside_a_registration_that_stays_torn_ends_resumably_and_its_resume_reverifies()`

The record's T2′: a registration that stays torn — nobody finishes it — ends the command at the
access's deadline as `UpstrokeError::RegistryRefused`, naming the entry's `commondir`, with
nothing durable after `merge_verification_started`: no `merge_verification_unavailable`, no
question, no `run_finished`. The operator's remedy (remove the registration) and a resume settle
the open verification interrupted and verify the candidate again under a new sequence, and the
run completes with nothing deferred. Mutation m2 (the deadline's refusal typed `Git`) makes the
verification defer here.

## `fn planted_alone(`

The tear as `TearsAForeignRegistration` plants it, with no run: a scratch tree of its own, the
registration's directory at `<root>/common/worktrees/foreign`, planted through `problem` at a
worktree path whose parent is `merge`, as a verification's snapshot is.

## `fn a_foreign_writer_whose_handshake_never_arrives_writes_nothing_and_says_so() {`

Repair round 3's R3, the shape of the regular lens's watchdog witness: no access meets the tear,
so the writer's 20 ms watchdog expires. Joined without a cancel, it reports the watchdog, and
`commondir` is still the empty file planted. Before the repair the writer wrote `commondir` at
its watchdog anyway.

## `fn a_foreign_writer_is_cancelled_and_joined_before_its_fixture_is_reclaimed() {`

R3, the shape of the regression lens's witness: `finish` cancels and joins the writer before the
fixture goes, and says it was cancelled, and the cancelled writer wrote nothing. The fixture is
then reclaimed, and the wake the writer waited for arrives late (`CONTENDED_ATTEMPTS` moved by
hand, as a late access would move it): nothing recreates the root. Before the repair the writer's
handle was discarded, and that late wake recreated the reclaimed directory.

## `fn lone_plant(tag: &str) -> (crate::rundir::scratch_tree::ScratchTree, Plant) {`

A tear with an empty `commondir` in a store of its own, in a scratch tree, its prober waiting for a
contended attempt on that store: the two tests below.

## `fn cancelled_and_joined(prober: Prober) -> Result<(), String> {`

Cancel a prober, join it, and return its report.

## `fn a_prober_cancelled_after_the_attempt_it_waits_for_reports_it_and_finishes_the_tear() {`

Repair round 7's order, forced: the prober's wait is as long as the test, so the attempt is counted
and the cancel sent while it waits, with no sample between them. It reports the attempt and
finishes the tear. A prober that reports a cancel before it samples, as it did before round 7,
reports the tear standing here, every time.

## `fn a_prober_cancelled_before_anything_it_waits_for_leaves_the_tear_and_says_so() {`

The other side of that order: cancelled with nothing counted, the prober leaves the tear as planted
and says what it did not see. A prober that took a cancel for progress fails here.

## `fn served_through(tag: &str, tasks: &[WideTask], at: TearAt) {`

The dispatch witness of R1, without an observer: the first task's pipeline is spawned and asks for
its worker while the coordinator dispatches the second, whose access meets the tear. The access is
passed only because the coordinator granted the worker while it waited. Since round 6 the
coordinator call runs [`bounded`], and no wait of the coordinator's slept on its thread
(`fixture::slept_pauses`, unchanged across the call), as in every R1 witness.

## `fn a_pipeline_is_granted_while_a_dispatchs_head_check_waits_on_a_torn_registration() {`

Census A1: the dispatch's head check, the first access of every dispatch: the frozen
`integrate::dispatch_head` over `run::PausingRefs` since follow-up B's repair round 6 (I2-7), which
withdrew round 3's split `dispatch_head_at` (H2).

## `fn a_pipeline_is_granted_while_a_dispatchs_revalidation_waits_on_a_torn_registration() {`

Census A2: the dispatch's own revalidation before `task_dispatched`.

## `fn a_pipeline_is_granted_while_a_dispatchs_intent_waits_on_a_torn_registration() {`

Census A3, the implementation review's witness: the intent's revalidation after `task_dispatched`.

## `fn a_pipeline_is_granted_while_a_dispatchs_add_gate_waits_on_a_torn_registration() {`

Census A4: the add's gate.

## `fn a_pipeline_is_granted_while_a_dispatchs_add_waits_on_a_torn_registration() {`

Census A4: the add itself, whose waits run inside its funnel (`funnel_lending`).

## `fn served_while_alpha_waits(tag: &str, failing: &'static [(u32, u32)], at: TearAt) {`

`served_while_alpha_waits_with`, expecting the run to complete.

## `fn served_while_alpha_waits_with(`

The observed witness for a settlement, an integration or a retry of beta while alpha holds its first
gate: two gates, so that alpha's next grant asks for no registry access first. The scenario is built
and run inside [`bounded`], so `prepare` is `Send`.

## `fn candidate_prepared_of_beta(body: &TopologyEventBody) -> bool {`

Beta's `candidate_prepared`.

## `fn generation_closed_of_beta(body: &TopologyEventBody) -> bool {`

Beta's `generation_closed`.

## `fn a_pipeline_is_served_while_a_settlements_path_read_waits_on_a_torn_registration() {`

Census D1: the promotion's changed paths.

## `fn a_pipeline_is_served_while_a_settlements_parent_check_waits_on_a_torn_registration() {`

Census D2: the candidate's parent check.

## `fn a_pipeline_is_served_while_a_settlements_tree_check_waits_on_a_torn_registration() {`

Census D2: the candidate's tree check.

## `fn a_pipeline_is_served_while_a_settlements_worktree_removal_waits_on_a_torn_registration() {`

Census D3: the reclaim's worktree removal — its scan, met by a registration that is `locked` and has
no `gitdir`.
## `fn a_pipeline_is_served_while_a_settlements_intent_removal_waits_on_a_torn_registration() {`

Census D3: the reclaim's intent removal.

## `fn a_pipeline_is_served_while_an_integrations_decision_waits_on_a_torn_registration() {`

Census C1: `integrate::decide`'s publishability check (H1).

## `fn a_pipeline_is_served_while_an_integrations_publication_waits_on_a_torn_registration() {`

Census C2: `integrate::publish`'s publishability check (H1).

## `fn a_pipeline_is_served_while_a_publications_swap_recheck_waits_on_a_torn_registration() {`

Census C6: the publishability re-check that `WorkspaceManager::compare_and_swap_ref` makes before its
funnel, through the hooks `integrate::publish` hands it (CAS-1, the record's §9.21). The tear is
planted before the second registry access after beta's `merge_prepared`, so `publish`'s own check
(C2) passes and the swap's re-check meets it.

## `fn a_pipeline_is_served_while_a_retrys_worktree_check_waits_on_a_torn_registration() {`

Census B1: the retained retry's worktree verification.

## `fn a_pipeline_is_served_while_an_intent_removals_repair_removal_waits_on_a_torn_registration() {`

The reclaim's intent removal: its repair's forced removal of the residue (the scan) meets the tear.

## `fn a_pipeline_is_served_while_an_intent_removals_second_revalidation_waits_on_a_torn_registration()`

The reclaim's intent removal: the revalidation after its repair meets the tear.

## `fn a_pipeline_is_served_while_a_verifications_repair_plan_waits_on_a_torn_registration() {`

The retained retry's verification: its repair's plan meets the tear.

## `fn a_pipeline_is_served_while_a_verifications_second_revalidation_waits_on_a_torn_registration()`

The retained retry's verification: the revalidation after its repair meets the tear.

## `fn a_pipeline_is_served_while_a_verifications_worktree_read_waits_on_a_torn_registration() {`

The retained retry's verification: its read of the worktree's record, inside its funnel, meets the
tear.

## `fn closing_attempt_finished_of_beta(body: &TopologyEventBody) -> bool {`

Beta's `attempt_finished` that closes its generation.

## `fn a_pipeline_is_served_while_a_closed_retrys_worktree_scrub_waits_on_a_torn_registration() {`

Census B2: a retry whose retained worktree was changed by hand closes, and its scrub's worktree
removal (the scan) meets the tear.

## `fn a_pipeline_is_served_while_a_closed_retrys_intent_scrub_waits_on_a_torn_registration() {`

Census B2: the same scrub's intent removal meets the tear; the scan passes over an empty `commondir`.

## `fn served_while_a_closed_retry_scrubs(tag: &str, torn: Torn) {`

The B2 witnesses: beta's first attempt fails and retains its worktree, a file is staged in it by
hand, the retry's verification fails, and the generation closes.
## `fn a_pipeline_is_served_while_a_failed_settlements_worktree_scrub_waits_on_a_torn_registration()`

Census D4: beta's second failed attempt escalates and closes the generation, and its scrub's
worktree removal (the scan) meets the tear; the task then parks, as the scaffold's failing gates
leave it.

## `fn a_pipeline_is_served_while_a_failed_settlements_intent_scrub_waits_on_a_torn_registration() {`

Census D4: the same scrub's intent removal meets the tear.

## `fn served_while_a_failed_settlement_scrubs(tag: &str, torn: Torn) {`

The D4 witnesses: both of beta's attempts fail their first gate.
## `fn served_while_a_stale_integration_waits(tag: &str, conflicting: bool, at: TearAt) {`

`served_while_a_stale_integration_waits_under` with `alpha_waits_for_gammas_integration`.

## `fn served_while_a_stale_integration_waits_under(`

The stale witness: beta merges first, so gamma's integration is stale; with `conflicting`, gamma
writes beta's file, its pick conflicts, and the rejection's repair is dispatched. The tear takes the
shape `torn`. The prober wakes on an ending: alpha's released worker ends, and the coordinator
applies that end, inside the wait.
## `fn a_pipeline_is_served_while_a_stale_integrations_intent_waits_on_a_torn_registration() {`

Census C4: the stale arm's staging intent.

## `fn a_pipeline_is_served_while_a_stale_integrations_add_gate_waits_on_a_torn_registration() {`

Census C4: the staging add's gate.

## `fn a_pipeline_is_served_while_a_stale_integrations_add_waits_on_a_torn_registration() {`

Census C4: the staging add.

## `fn a_pipeline_is_served_while_a_stale_integrations_pick_waits_on_a_torn_registration() {`

Census C4: the cherry-pick's revalidation.

## `fn a_pipeline_is_served_while_a_publications_staging_removal_waits_on_a_torn_registration() {`

Census C5: the publication's staging removal — its scan.

## `fn a_pipeline_is_served_while_a_publications_staging_intent_removal_waits_on_a_torn_registration()`

Census C5: the publication's staging intent removal.
## `fn a_pipeline_is_served_while_a_conflicts_classification_waits_on_a_torn_registration() {`

Census C3: `integrate_stale`'s `proposal_state` after the conflicting pick (H1).

## `fn a_pipeline_is_served_while_a_rejections_staging_reclaim_waits_on_a_torn_registration() {`

Census C5: the rejection's staging reclaim — its scan.

## `fn a_pipeline_is_served_while_a_rejections_staging_intent_reclaim_waits_on_a_torn_registration()`

Census C5: the rejection's staging intent removal.
## `fn a_pipeline_is_served_while_a_repairs_materialization_waits_on_a_torn_registration() {`

Census A5: the repair dispatch's materialization.

## `fn a_pipeline_is_granted_while_a_continued_dispatchs_worktree_check_waits_on_a_torn_registration()`

Census A6: the first process stops after beta's `task_dispatched`, at a file planted at beta's
destination; the next process resumes, starts alpha's attempt, and continues beta's open generation,
whose worktree check meets the tear.

## `fn the_width_one_step_runs_the_same_transitions_and_its_access_waits_by_sleeping() {`

R-T's control: the width-1 `step` runs the same dispatch with its own hooks, whose waits sleep; the
tear is finished once the access has answered `Attempt`, and the run completes. The sleep is seen:
`fixture::slept_pauses` moves on the stepping thread.

## `struct FoldsSeen {`

Hooks that record the kind of every event folded through them, forwarding the rest to the harness.

## `fn every_append_of_the_width_one_step_a_retrys_settlement_included_folds_through_the_callers_hooks()`

R-T in behaviour (the follow-up B record's §9.16, I2-4). Beta's first attempt fails its gate and is
retained, so the width-1 `step` takes the retry arm: begin the retry, judge it inline, settle it. Every
event `step` appended — the retry's `attempt_started` and its settlement's `candidate_prepared`
included — must have been folded through the hooks `step` was handed. A transition run with any
other hooks (the review's single-call mutation hands the retry's settlement fresh `NoTopologyHooks`)
leaves events folded through hooks the caller never sees, and the lists differ.

## `fn a_wait_answers_a_queued_shutdown_when_no_thread_can_be_started() {`

The regular review's thread-exhaustion witness (I2-3), Linux: with `clone3` answering `EAGAIN` on the
coordinator's thread — checked by a spawn that fails so — a wait still answers the queued shutdown,
leaves nothing in the inbox and sleeps nothing, because no wait starts a thread. At `a58c2ce3` the
wait slept 30 ms and answered nothing.

## `fn a_coordinator_whose_timer_cannot_start_refuses_before_it_appends_anything() {`

The one place a thread is still needed, at the coordinator's start: with `clone3` refused, the run
refuses as typed, before the runtime is built, appending, spawning and sleeping nothing.

## `fn unwinding_sleep(_: Duration) {`

A timer's sleep that unwinds.

## `fn a_wait_whose_timer_unwinds_is_still_woken_and_the_timer_serves_the_next() {`

I2-5's lost producer, injected: the timer's sleep unwinds, and two waits in a row are each woken, the
second by the same thread; the join reports both unwinds. No natural timer panic is known (the
thread's calls are a channel receive, a sleep and a send); this is the injected one.

## `fn a_wait_that_begins_after_a_shutdown_does_not_wait() {`

A wait asked for once a shutdown is recorded stops at once with the stop's error, asking the timer
for nothing: a minute's wait is not waited out.

## `fn a_wait_after_its_timer_has_stopped_refuses_at_once_and_never_sleeps() {`

A wait whose timer cannot be asked refuses as typed, at once, and records the command's error; a
minute's wait is not slept.

## `fn a_verification_whose_snapshot_checkout_fails_after_the_takeover_ends_resumably_and_its_resume_reverifies()`

The record's T15, verification face, and R14
(`PR329-A-GENUINE-CHECKOUT-FAILURE-AFTER-THE-TAKEOVER-REFUSES`): the judge's snapshot add fails
after the takeover, which nothing outside Git tells from a prune deleting the snapshot's
registration, so it refuses at once, resumably, and nothing durable is appended. With the
environment repaired, a resume settles the verification interrupted and verifies again; the run
completes. Before #329 the same failure was Git state, and the verification deferred and then
parked at `max_defers`; R14 is that narrowing, which needs the owner's disposition before G6.
Two tasks, because a single candidate already on the integration head merges with no
verification.

## `struct DeniesTheSnapshots {`

Round 5's construction: at the first verification, the execution root's `snapshots/` is made
unwritable, so the judge's snapshot destination cannot be made.

## `struct RestoresTheSnapshots {`

Restores `snapshots/` once the verification's terminal is durable, so the next verification
proceeds.

## `fn a_verification_whose_snapshot_destination_cannot_be_made_defers_as_before() {`

The control beside T15: a destination that cannot be made is Git state at once, before `git worktree add`
runs (`workspace_manager`'s destination step, after the add's gate), so `decisions.repairs.not_repairs` applies as it did:
one `merge_verification_unavailable` (Deferred), and the run completes with both candidates
merged. Green before #329 too, where Git's own add failed to make the destination.
