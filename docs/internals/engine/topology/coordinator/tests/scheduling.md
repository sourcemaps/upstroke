# `src/engine/topology/coordinator/tests/scheduling.rs`

Extended notes for [`src/engine/topology/coordinator/tests/scheduling.rs`](../../../../../../src/engine/topology/coordinator/tests/scheduling.rs).

The code is the authority for what it does; this file is the whole of its prose but for the
concurrency protocol standards §10 places at its site. Each section
here is headed by the line of code it describes, spelled as it is in the source, so the heading is
the grep string that finds the code.

## `fn two_independent_tasks_overlap_merge_through_one_queue_and_the_dependent_starts_on_both() {`

Acceptance item 2, with `disjoint_hints_dispatch_together_overlapping_and_absent_hints_serialize`.

## `fn the_coordinator_settles_promotes_and_dispatches_while_a_verification_is_open() {`

R-E and R-V: the coordinator inside `verify`, with `halt_interrupts_verification`.

## `fn over_budget_prefix_without_budget_exceeded_is_not_ending_at_width_three() {`

With `a_budget_stop_drains_live_pipelines_to_their_settlements_and_ends_budget_exceeded`, G6's budget
row: `budget_exceeded` before any budget-driven end, the drain, the end. `Recording` keeps the fold
after every append, and `ending_as` asks it what the fold would say to a `run_finished`. The drain
test also holds the overshoot's shape (round R5, `FULL-SC-4`): each pipeline live at the stop starts
its review pass after `budget_exceeded`, which `Ran::durable_at_spawn` shows, so the exposure is
every admitted pipeline's remaining work, not one invocation each.

## `fn run_finished_budget_exceeded_refused_after_halting_drain_settlement_at_width_three() {`

In this build a halting settlement is a declined answer, which the fold refuses while a budget stop
is current, so a halting drain settlement is planted on a width-three prefix (R-AH). It proves
**outcome precedence only**: the prefix is planted after a shutdown, with nothing live, so the halt
the planted settlement records has nothing to cancel; the fold derives Halted, refuses
`run_finished(BudgetExceeded)`, and the next coordinator's closure ends the run Halted. The
conversion of a live drain is the next test's (round R5, the full review's `FULL-SC-3`).

## `fn a_halting_settlement_drained_after_a_budget_stop_cancels_the_live_worker_and_ends_halted_at_width_three()`

Closure step (2)'s conversion, live: alpha crosses the ceiling and `budget_exceeded` is recorded with
beta's and gamma's workers inside the Runner; the scheduler then drains a halting settlement of beta's
(`Release::Append`, as R-AH says it must be planted in this build) while gamma's worker is still
live. The next admission pass acts on the halt: gamma's worker is cancelled through the Runner, the
closure runs as Halted from step (2) and settles gamma's attempt `attempt_interrupted`, nothing is
published, and the run finishes Halted at beta. With the halt branch disabled during a budget drain,
gamma's worker runs on and the command fails on gamma's next snapshot request instead.

## `fn a_halt_whose_cancelled_process_is_unresolved_ends_the_command_without_closing() {`

R-AF's guard, the phase-4 side of the early review's `R1-CONC-1`: the double reports a cancelled
worker's process unresolved (`RecordingRunner::unresolved_when_cancelled`), and the closure is not
entered. Since round R1 the unresolved fate reaches the coordinator in the worker's `Ended` too, and
the worker keeps its registration.

## `fn a_closure_never_settles_in_flight_work_its_coordinator_did_not_cancel() {`

R-AF's vouching, from the other side: a second coordinator in the same process, which cancelled
nothing, meets a halt with two attempts still in flight from the first; its closure refuses them,
appending nothing, and the next process's recovery settles them.

## `fn a_verifications_review_spend_is_charged_before_the_next_selection() {`

R-AH's current spend, the phase-4 side of the early review's `R1-CONC-4`: a verification's
completion arrives while an attempt snapshot keeps `verify` waiting, a third task settles, and the
selection it opens meets a ceiling only the verification's review spend has crossed.

## `fn one_seed_reproduces_one_run_down_to_what_each_process_saw_when_it_started() {`

R-AD's claim, pinned: three runs of one seed release the same invocations in the same order, start
the same processes in the same order on the same workspaces with the same log durable at each
start, and write the same canonical log.

## `fn out_of_order_completions_bind_to_their_own_identities_under_seeded_permutations() {`

ST-03 and ST-08 under seeded permutations, with
`independent_tasks_dispatch_together_and_keep_per_key_projections_under_every_seed` and
`a_chain_plan_projects_identically_at_widths_three_and_one`. This test and
`independent_tasks_dispatch_together_and_keep_per_key_projections_under_every_seed` assert
`attempt_level_problems` empty for keys 0, 1 and 2 at every seed before comparing that seed's
`attempt_level` with the first seed's.

## `fn selecting_attempt_level_events_by_a_kind_field_keeps_nothing_and_is_rejected() {`

The regression for G6-ST08-EMPTY-PER-KEY-PROJECTION: on one run of three independent tasks, the
former selection — a top-level `kind` field of the serialized event, as it was written — keeps
nothing for keys 0, 1 and 2, `attempt_level_problems` rejects all three, and `attempt_level` on the
same log passes it.

## `fn a_keys_attempt_level_projection_depends_on_its_own_trace_alone() {`

`attempt_level`'s label scope, on one run in which beta's first gate fails. Beta's retry is retained —
the same generation, no second dispatch — so beta projects its first attempt's `attempt_finished` and
a second `attempt_started`, while alpha and gamma project `ONE_SUCCEEDING_ATTEMPT`. A copy of the log
with the keys' attempt-level events in the reverse of the order their candidates were prepared in,
and every other event after them, projects every key exactly as the log does; labels numbered over
the whole log differ between the two, which is the control that the reordering renumbers. Alpha's
projection changes, and only alpha's, when its `task_candidate_created` names its base rather than
its prepared commit, and when its `candidate_prepared` carries gamma's tree OID.

## `fn concurrent_attempts_at_one_generation_and_attempt_use_distinct_snapshot_slots() {`

ST-04 — at most one provisional reservation, read through `Reservations::peak` — with distinct
snapshot names (R-Y); `live_state_equals_replay_after_every_append_at_width_three` is ST-10.

## `fn a_retained_retry_is_admitted_beside_other_pipelines_and_regates_on_fresh_snapshots() {`

ST-15.

## `fn adversarial_orders_with_one_slot_per_agent_and_pool_always_reach_run_finished() {`

Deadlock freedom: one slot per agent and per pool, seeded orders, every run reaches
`run_finished`.
