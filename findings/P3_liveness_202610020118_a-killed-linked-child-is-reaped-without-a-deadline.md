---
id: PR328-LINKED-CHILD-KILL-WAITS-WITHOUT-A-DEADLINE
severity: P3
disposition: deferred
category: liveness
pr: 328
reviewed_sha: 17d7c6052d455ff4311991d4ba01f396fb5603d3
location: src/workspace_manager/fixture.rs:3107
provenance: pre_existing
first_bad: ad1a36b53e75ece367f66b647b41531ee8ec2b2b
guard: PR #329's implementation phase (PR11 follow-up B, which owns `src/workspace_manager*`) — `LinkedChild::kill` and `LinkedChild`'s `Drop` collect a killed child within a bound and report one still not collectable, with a witness of a child the kill cannot end
---

## Failure sequence

A two-process coordinator test kills its child coordinator:

    `scaffold::Served::kill` (src/engine/topology/scaffold.rs:3420) calls `LinkedChild::kill`
    (src/workspace_manager/fixture.rs:3107)
    -> `child.kill()` sends SIGKILL
    -> the child does not become collectable: a tracer holds it at its exit stop, or it is in uninterruptible
       sleep with the signal pending
    -> `child.wait()` blocks with no deadline
    -> the test never reaches its assertions, and its thread holds the suite until the child ends

The same holds when a test fails before its kill. `Served`'s `Drop` (scaffold.rs:3441) and `LinkedChild`'s `Drop`
(fixture.rs:3146) both call `kill`, so an unwinding test wedges in the same wait. Every line above is at
`17d7c605`, and `fixture.rs` and `scaffold.rs` are unchanged since the merge base `92c4ca81`.

**Callers at `17d7c605`.** Six PR11 phase-5 tests and helpers in `src/engine/topology/coordinator.rs`:
- `a_foreign_census_reclaims_a_dead_coordinators_containers_and_leaves_a_live_coordinators_running` (:8808);
- `killed_fresh_incarnation` (:9079);
- `a_resuming_incarnation_reclaims_its_earlier_incarnations_containers_before_its_ledgers_probes_and_admission`
  (:9152);
- `crashes_across_three_incarnations_with_pipelines_in_flight_leave_every_orphan_reclaimed_and_no_name_twice`
  (:9391);
- `a_foreign_census_and_a_resuming_incarnation_converge_on_one_dead_container_as_two_processes` (:9599);
- `a_killed_coordinators_reapers_settle_its_pipelines_processes_under_r28_before_the_next_one_resumes_at_width_three`
  (:9941).

PR #328 adds two:
- `a_resume_killed_inside_its_pre_flight_probe` (:13518), the helper of
  `a_resuming_incarnations_pre_flight_probe_container_is_killed_by_its_reaper_when_the_coordinator_dies_inside_it`
  and `the_reapers_scope_is_the_runners_identity_and_selects_its_first_probe`;
- `a_fresh_runs_p4_probe_container_is_killed_by_its_reaper_before_run_started`
  (src/engine/topology/create/tests.rs:4812).

**Found by** PR #328's implementation review round 1 repair, in the sweep `FUA-I1-WATCHDOG` asked for. The
regression lens showed `agent::proc`'s isolated-child watchdog, `run_isolated`, blocked in `wait4` after its own
`SIGKILL`. The sweep then looked for the same shape, a kill followed by an unbounded wait, in every harness the new
tests use. #328 bounded every instance in its own code. This one is the only instance it did not fix: it predates
the follow-up (first bad `ad1a36b5`, PR11 phase 5), and it lives in `src/workspace_manager*`, which follow-up B
(#329) owns and whose own two-process tests lean on `LinkedChild`. The orchestrator ruled a hunk there not worth the
conflict (`~/orch-pr11/answers/pr11_fua_r1-1.md`).

**What it costs.** It is test infrastructure only; no production path reaches it. The suite wedges instead of
failing, and only when a killed child stays uncollectable: a child coordinator held by a tracer, as the regression
lens held `run_isolated`'s child, or wedged in a hung filesystem.

## What the change that takes this up should do

Bound the wait after the kill in `LinkedChild::kill`. Poll `try_wait` within a bound, as `LinkedChild::wait_within`
already does for an exit, and fail the test with the child's stderr when the child is still not collectable. Do the
same in `Drop`, which must not panic while unwinding: leave a child still there to the process's exit and say so.

Witness it with a child the kill cannot end. `agent::proc`'s
`the_watchdog_fails_its_test_rather_than_wait_for_a_child_its_kill_did_not_end` refuses `kill` on the killing
thread with a seccomp policy; the same works here.
