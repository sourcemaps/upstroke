# `src/engine/topology/coordinator/tests/shutdown.rs`

Extended notes for [`src/engine/topology/coordinator/tests/shutdown.rs`](../../../../../../src/engine/topology/coordinator/tests/shutdown.rs).

The code is the authority for what it does; this file is the whole of its prose but for the
concurrency protocol standards §10 places at its site. Each section
here is headed by the line of code it describes, spelled as it is in the source, so the heading is
the grep string that finds the code.

## `fn a_shutdown_with_pipelines_in_flight_is_settled_by_the_next_resume_whose_ledgers_start_empty()`

G6's shutdown row, resumed through the frozen recovery order itself (`Wide::durable`,
`Wide::resume`, `scaffold.md`).

## `fn append_error_under_concurrency_cancels_pipelines_and_folds_nothing_from_memory() {`

The slice contract's named test, at each T-APPEND error-return shape — a partial write, a flush
error after the full line, a sync error — injected at the settlement of one pipeline while two
others are in flight, and the resume following the surviving prefix (R-AI). Since round R5 the
ledger it asserts has no duplicate settlement: each of the two running registrations is settled once,
by its own pipeline's end report after its process terminated, and not by the protocol before.

## `fn an_append_error_releases_a_running_invocation_only_when_its_end_establishes_its_process_gone()`

The full review's `FULL-CONC-1` (its concurrency lens's witness): the same append error with beta's
cancelled process reported unresolved (`RecordingRunner::unresolved_when_cancelled`). Beta keeps its
registration and its pair, gamma is released by its end, nothing is settled twice, and the ledger
does not balance — it reports no release and no balance it has not observed. At `a9e88039` the
protocol had settled both before either pipeline ended: running none, holders none, balanced, two
duplicates.

## `fn kill_inside_closure_recovers_at_width_three() {`

T-FINISH at width three, with
`append_error_inside_closure_ends_command_and_resume_completes_closure_at_width_three`: `HaltArming`
arms the fault at the first append after the halt is folded, which is the closure's first terminal;
the child (`closure_kill_child_at_width_three`) dies inside it, torn or complete, and the next
process repeats the closure from the surviving prefix. Each child's temporary directory is a `kill_dir`
guard this test holds (`scaffold::child_temporary_of`), so the fixture the child builds and the neutral Git
configuration it writes are reclaimed with the guard rather than left in the suite's temporary directory.

## `fn authorized_publication_completed_at_run_end() {`

Closure step (4) for T-FAST: the fast integration's CAS is made to fail once (`CasFailing`), which
leaves `merge_prepared(fast)` without `task_merged` — a state no coordinator schedule reaches at a
live end (R-AG) — and after a decline halts the run the closure publishes it before `run_finished`.
With `prepared_publication_completed_at_run_end` (T-PREPARED: a stale-clean verified proposal,
built on the coordinator so that beta is dispatched beside alpha and integrates stale; the answer
source is armed only once the publication is pending) and
`promoting_completed_by_the_closure_at_run_end` (T-CAND-REF, closure step (3): the candidates ref
made to fail once). The frozen width-1 `promoting_completed_at_run_end` (`candidate/tests.rs`) tests
the completion functions themselves.

## `fn retained_generation_closed_at_run_end_at_width_three() {`

Closure step (5) for T-RETAINED at both endings: at Halted, beside an interrupted pipeline (the
decline is armed by the retained settlement's own append, `ArmsOnRetained`, so the halt lands at the
admission pass that would have admitted the retry); at BudgetExceeded, where the ceiling refuses the
retry the retained generation was ready for.

## `fn stale_duplicate_and_mismatched_completions_are_discarded_with_a_warning_and_counted() {`

ST-01, ST-02 and ST-06, through the injector; `a_shutdown_cancels_every_live_pipeline_and_ends_the_command_resumably`
follows them. An injected completion meets every check a pipeline's does before the injector refuses
it as injected, so a count of discards cannot tell which check held; since round R5 (the full
review's `FULL-SC-2`) the test asserts which one refused each: the binding to the pipeline's identity
refuses alpha's two completions for another generation and another attempt, and the injector only
beta's own, which nothing earlier refuses. The binding and the fold's check are proved on the channel
a pipeline sends on by the three tests below.

## `fn completion_on_the_pipeline_channel(`

Round R5's assembled coordinator for `FULL-SC-2`: alpha's and beta's attempts started at width
three, their pipelines live with their jobs, the fold optionally given a settlement of beta's attempt
first; the one message — a completion — is sent by the named pipeline's own `Client` into the
coordinator's inbox and taken by `receive_one` with no observer, the production intake, so it
arrives as `Origin::Pipeline` and meets `accepts` as a real pipeline's completion does. What the
coordinator did with it is returned whole (`Received`): whether it interrupted, which pipelines are
still live and cancelled, the discards counted and the events appended.

## `fn a_pipelines_completion_naming_another_attempt_of_its_task_is_discarded_and_its_pipeline_kept()`

The binding, for a completion naming attempt 2 of the pipeline's own task: discarded and counted, by
the binding's warning, and the pipeline left live to send its own. With the binding removed the fold's
check refuses it instead and retires the live pipeline — whose own completion would then be stale.

## `fn a_pipelines_completion_carrying_another_live_pipelines_identity_is_discarded_and_settles_nothing()`

The binding, for alpha's pipeline carrying beta's identity, which the fold holds open (the
concurrency lens's case): discarded and counted, nothing appended, both pipelines live. With the
binding removed nothing else refuses it: alpha's pipeline is retired and its job settled with a
completion that was not its own.

## `fn a_pipelines_completion_for_an_identity_the_fold_closed_is_discarded_and_settles_nothing() {`

The fold's check: beta's attempt is settled first (a planted `attempt_finished`), and beta's pipeline
then reports its own completion before any pass has stopped it. Discarded, counted and retired,
nothing appended. With the check removed the completion is settled against an attempt the fold has
already settled, which the fold refuses and the command ends on.

## `fn a_shutdown_cancels_every_live_pipeline_and_ends_the_command_resumably() {`

R-AB's shutdown, injected at the first quiescent point with three workers granted — the
interleaving phase 3's CI failed on (the working record's §13, round C1). It states the packet's
`cancellation` sentence — a shutdown "cancels/releases granted and non-slotted invocations after
termination" — on both sides: in the runner every granted worker's process had started and was
terminated as a cancellation; in the ledger each was released exactly once, as a cancellation, by
its pipeline's own report of the end. A release before the termination would make that report a
counted duplicate.

## `fn a_shutdown_releases_each_invocation_once_whether_pending_unstarted_running_or_finished() {`

Every place a cancellation can find an invocation, at one shutdown: four workers at width four with
three agent slots, so one request is pending; one granted worker is barred at the runner's door
(`RecordingRunner::bar`: granted, its process not started — the interleaving the deterministic
intake no longer produces by itself, and production can); one is running; and one has just finished,
its result delivered and its end not yet reported. Each is released exactly once: the pending one
withdrawn without reaching the runner, the unstarted one cancelled before it started, the running
one terminated, the finished one completed. The ledger balances with no duplicate (R3, R4; R-AE).

## `struct StoppedInItsWait {`

Where a shutdown witness plants its tear, in which shape, and whether the shutdown finishes it.

## `fn stopped_in_its_wait(`

The shutdown witness of one transition (I2-1), bounded. The run meets a tear in the transition's
access; the scheduler injects `Shutdown` inside the access's wait and finishes the tear. The command
must end on the shutdown, the access must have failed on the tear first (so the shutdown came inside
its wait), no wait may have slept on the coordinator's thread, **nothing may be appended after the
shutdown**, the integration ref must be where the log authorizes it, the invocations must balance,
and the run must be reopened once the coordinator is gone; `check` adds what the transition must not
have done. At `a58c2ce3`, and with the stop undone, the access passes once the tear is finished and
the transition appends on. When no shutdown was injected, its panic keeps the message
"the shutdown was injected while the tear stood" and adds what the command ended on, so a
recurrence says which path it took (fix P).

## `fn two_held(tag: &str) -> Wide {`

Two independent tasks at width 2 behind a holding runner.

## `fn two_held_gated(failing: &'static [(u32, u32)]) -> impl FnOnce(&str) -> Wide + Send {`

The same with two gates, failing as `failing` says: `served_while_alpha_waits`'s scenario.

## `fn first_invoking(view: &Quiescent<'_>) -> Option<Release> {`

Release the first invocation in id order.

## `fn first_released(_: &TearHeld) -> Script<'static> {`

[`first_invoking`], as a shutdown witness's order.

## `fn beta_settles_first(_: &TearHeld) -> Script<'static> {`

`beta_settles_while_alpha_holds_its_first_gate`, as a shutdown witness's order.

## `fn workers_of(wide: &Wide, key: u32) -> usize {`

How many of a task's workers reached the runner.

## `fn a_shutdown_answered_inside_a_dispatchs_head_check_dispatches_and_spawns_nothing() {`

Dispatch: the second task's head check meets the tear; after the shutdown no `task_dispatched` for
it, and no worker of it reached the runner, so its pipeline was never spawned.

## `fn a_shutdown_answered_inside_a_dispatchs_intent_starts_no_attempt_and_spawns_nothing() {`

Dispatch after its `task_dispatched`: the intent's access meets the tear; no `attempt_started` for
the task, and no pipeline.

## `struct DelayedRelease {`

The worker that holds the slow dispatch witness's first contended answer, and its owner: the sender
that cancels the worker's wait for that answer, and the handle `finish`, or the drop, joins
(standards §10; the protocol stays in the source, above the type). Repair round 11 (B6; I6-1, the
follow-up B record's §9.23): fix P spawned the worker and discarded its handle, so a worker that
panicked before its delay dropped its release unsent, the held access went on at once, and the
witness passed without the delay it is about. Both i6 review lenses showed it passing in 0.04 s
with the deadline seam undone and a panic injected into the worker.

## `impl DelayedRelease` › `fn finish(&mut self) -> Result<(), String> {`

Cancel what is left of the worker's wait for an attempt, join the worker, and turn a panic into the
witness's failure: a worker that panicked dropped its release unsent, and the access went on
without the delay. A second call finds nothing to join.

## `impl Drop for DelayedRelease` › `fn drop(&mut self) {`

Finish a worker no step finished: when the scenario already unwinds, say what the join found on
stderr; otherwise fail with it.

## `fn a_delayed_release_whose_worker_panicked_fails_its_witness() {`

`finish` over a worker that panicked returns the failure, never `Ok`: an owner that drops the
worker's verdict, as the witness's discarded handle did before repair round 11, fails here.

## `fn a_shutdown_answered_inside_a_slow_dispatchs_intent_starts_no_attempt_and_spawns_nothing() {`

The dispatch's intent witness above, with its access slow to come back to its wait: the access's
first failed attempt is held (`workspace_manager::hold_next_contended`) for 700 ms, longer than the
suite's 500 ms registry deadline, before its deadline check, as three slow Git processes held it on
the Windows leg (the coordinator's grant to the waiting pipeline runs the scaffold runner's
`git rev-parse`, and the access's second attempt is another `git worktree list`). Its repository
waits to `WITNESS_REGISTRY_DEADLINE`, so, while time remains when the access reads its clock after
the 700 ms, the access still reaches its wait, the shutdown is still injected there, and the
dispatch still starts no attempt; a deadline passed by then, wherever the time went, leaves no wait
asked for (§9.32, §9.34). With the deadline seam undone it fails with the Windows leg's message.
Fix P; the Windows reading is a Linux stand-in's, not a measured cause of the guest's failure.

The worker that holds the answer and releases it after the 700 ms is owned (`DelayedRelease`, repair
round 11, I6-1): the build step spawns it and hands it over a channel to the check step, which joins
it before its own assertions and fails when it panicked, since its release then dropped unsent and
the access went on without the delay. Its wait for the first attempt also ends at a cancel, so a
scenario that unwinds first joins it within the delay. The hold, its 700 ms and the witness's cost
are unchanged.

## `fn a_shutdown_answered_inside_a_continued_dispatchs_wait_starts_no_attempt() {`

Continuation: beta's add is broken after its `task_dispatched` (the R1 witness's set-up), a fresh
process resumes with beta's generation open and no attempt, and the continuation's worktree check
meets the tear; beta never starts an attempt.

## `fn a_shutdown_answered_inside_a_retrys_wait_starts_no_retry() {`

Retry: beta's retained retry's worktree verification meets the tear; the retry's `attempt_started`
is never appended.

## `fn a_shutdown_answered_inside_a_settlements_wait_promotes_nothing() {`

Settlement: beta's settlement's path read meets the tear; no `candidate_prepared` and no
`attempt_finished` for beta.

## `fn a_shutdown_answered_inside_a_verifications_wait_starts_no_retry() {`

Verification: the worktree record a retry's verification reads inside its funnel meets the tear; no
retry starts.

## `fn a_shutdown_answered_inside_a_repairs_wait_starts_no_repair_attempt() {`

Repair: a conflict's repair is dispatched and its materialization meets the tear; the repair task
starts no attempt and no worker of it runs.

## `fn a_shutdown_answered_inside_an_integrations_decision_prepares_and_publishes_nothing() {`

Integration: the frozen `decide`'s publishability check (H1) meets the tear; no `merge_prepared`, no
`task_merged`, and the ref is unmoved.

## `fn a_shutdown_answered_inside_a_publications_wait_publishes_nothing() {`

Publication: after `merge_prepared`, the frozen `publish`'s check (H1) meets the tear; no
`task_merged`, and no compare-and-swap moved the ref.

## `fn a_shutdown_answered_inside_a_publications_swap_recheck_publishes_nothing() {`

Publication, CAS-1: after `merge_prepared` and `publish`'s own check, the compare-and-swap's
publishability re-check meets the tear; no `task_merged`, and the ref is where the log authorizes it.

## `fn a_shutdown_answered_inside_a_stale_picks_wait_classifies_nothing_into_the_log() {`

The one place the frozen integration discards an error: a stale integration's cherry-pick, whose
gate meets the tear. The pick's error is set aside and the proposal's state is read, with the tear
finished, so a transition that went on would classify and append; nothing is appended after the
shutdown.

## `fn run_finished(body: &TopologyEventBody) -> bool {`

Any `run_finished`.

## `fn attempt_interrupted(body: &TopologyEventBody) -> bool {`

Any `attempt_interrupted`.

## `fn gamma_halts_while_alpha_gates(view: &Quiescent<'_>) -> Option<Release> {`

`halt_cancels_in_flight_attempt_at_width_three`'s order: alpha's worker first, then, while alpha is
at its gate, gamma's worker, whose declined question halts the run.

## `fn a_shutdown_answered_inside_a_closures_wait_closes_nothing_further() {`

Closure: the halt's closure appends alpha's `attempt_interrupted` and its reclaim meets the tear; no
second `attempt_interrupted` and no `run_finished` follow.

## `fn a_shutdown_answered_inside_a_finalizations_wait_finalizes_nothing_further() {`

Finalization: after `run_finished`, the frozen `finalize` meets the tear; it stops, and the execution
root it would have removed is still there.

## `fn plant_commondir_empty(wide: &Wide, name: &str) -> PathBuf {`

A registration with an empty `commondir`, spelt as Git writes it, in the run's own store.

## `fn a_shutdown_answered_during_a_registry_pause_dispatches_nothing_and_appends_nothing() {`

The regular review's dispatch witness at `a58c2ce3`, kept: alpha is live, a shutdown is queued and
beta's dispatch meets a tear; an owned thread finishes the tear once alpha is cancelled. The
shutdown is handled inside the wait, and beta's dispatch now fails and appends nothing. Its
repository's accesses wait to `WITNESS_REGISTRY_DEADLINE` (§9.31), so the access reaches its wait
only if time remains when it reads its clock after its first failed attempt's veto and count. If the
deadline has passed by then, wherever the time went (the attempt, its veto, the count, or a
descheduled thread), no wait is asked for: the final attempt is made, or was the failed one (§9.32),
and if it fails the access refuses without servicing the queued shutdown (§9.34).

## `fn a_shutdown_answered_during_an_admitted_dispatchs_pause_spawns_no_pipeline() {`

The same through the admission pass, the real call site of the spawn: `admit` ends with the stop's
error, and no pipeline, no handle and no worker exist. It waits to `WITNESS_REGISTRY_DEADLINE` as the
dispatch witness above does.

## `fn a_shutdown_consumed_during_a_registry_wait_publishes_no_candidate() {`

The regression review's integration witness at `a58c2ce3`, kept: a prepared candidate's integration
meets a tear with a shutdown queued, and a prober finishes the tear. Nothing is appended, the ref is
unmoved, and the interrupted integration ends the admission pass. Its access ends at the shutdown,
not at the tear, so nothing waits for the prober: that the access failed on the tear first rests on
the sample the prober takes after the cancel (`fn tear_sampling_every`, repair round 7). Should the
wait not stop the integration, the prober still finishes the tear in time for the access's next
attempt, and the integration then appends and publishes, which the witness refuses. Its
repository's accesses wait to `WITNESS_REGISTRY_DEADLINE` (§9.31), so the access reaches its wait
only if time remains when it reads its clock after its first failed attempt's veto and count. If the
deadline has passed by then, wherever the time went (the attempt, its veto, the count, or a
descheduled thread), no wait is asked for: the final attempt is made, or was the failed one (§9.32),
and if it fails the access refuses without servicing the queued shutdown (§9.34).

## `fn a_dispatch_begun_after_a_wait_answered_a_shutdown_appends_nothing() {`

The append half of the stop, on its own: a wait answers a queued shutdown and stops the run, and a
dispatch begun after it, with no tear to meet, is refused at its first append by the run's emitter.
This is what holds for a transition that carries on past the wait's error, whatever it does with it.

## `fn a_publication_begun_after_a_wait_answered_a_shutdown_moves_no_ref() {`

The effect half: after a stopped wait, the frozen `publish` over the coordinator reaches its
compare-and-swap, which the coordinator's effect hooks refuse at `before`; the ref is unmoved.

## `struct CountsRawPauses {`

The caller's effect hooks, counting their own `registry_pause`: a call is a wait the coordinator did
not answer itself.

## `struct RawPausesUnder {`

Those hooks under a `TearHeld`'s plant and prober.

## `fn waits_through_the_coordinator(`

The routing witness of closure and finalization (I2-2), bounded: the tear is finished once the
access has failed on it, the run ends as expected, and the caller's raw pause was never called and no
wait slept on the coordinator's thread. At `a58c2ce3` closure and finalization were handed the
caller's hooks, and the raw pause was the wait.

## `fn a_finalizations_registry_wait_answers_on_the_coordinator() {`

The regular review's finalization witness, made strict: a tear at `run_finished`, met by the frozen
`finalize`.

## `fn a_closures_registry_wait_answers_on_the_coordinator() {`

The same for the halt's closure: a tear at its first `attempt_interrupted`, met by the reclaim.
