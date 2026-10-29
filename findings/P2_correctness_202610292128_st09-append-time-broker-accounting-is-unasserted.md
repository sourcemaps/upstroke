---
id: G6-ST09-APPEND-TIME-BROKER-ACCOUNTING-UNASSERTED
severity: P2
disposition: deferred     # open in this commit only: implemented by 386b3145 and 7e7caaf5 below it; the next commit closes it; independent acceptance awaits this pull request's fresh reviews
category: correctness
pr:                       # assigned when the pull request is created
reviewed_sha: e0932045b8eb6b7194c677dc7da598b7e7057130
location: src/engine/topology/emit.rs:251
provenance: undetermined
first_bad:                # undetermined
guard: ledger_problems in every_provisional_reservation_converts_at_its_first_append_under_seeded_permutations and reduced_limits_and_adverse_completion_orders_reach_run_finished_within_the_step_bound
---

## Failure sequence

ST-09 (v17 `decisions.tests_acceptance.seam_tests[8]`) requires the logical rows to be "asserted from
fold and ledger state at every append".

At `e0932045` the only per-append hook is `folded` (`src/engine/topology/emit.rs:251`), and it
receives the fold and the event list and nothing else. The broker that holds the actual slot,
invocation and provisional-reservation state belongs to `TopologyRun` and reaches no hook.

So the width-three seeded witnesses check:
- R1 and R2 after every append;
- the full fold against replay after every append;
- R3, R4 and R13 only at process start, at quiescent points and at the end.

`reviews/2026-09-30-pr11-record.md` §R-AT records the same gap.

ST-09's append-time accounting assertion over the actual broker state is therefore not established.
This is an assertion-coverage gap for Gate 6. No runtime accounting defect is observed.

## What the change that takes this up should do

Observe the real broker state at each successful append, read-only. In the existing width-three
seeded witnesses, assert in-flight conservation and identity at that boundary:
- reservations taken = converted + cancelled + outstanding, with each first append's provisional
  phase;
- slot pairs granted = released + held;
- the pair holders and waiting requests match the running and pending slotted invocations;
- the registrations partition by state.

Endpoint balances and zero holders are not required during valid work.

This is implemented on this branch:
- `386b3145` adds a read-only `TopologyHooks::brokered`, a no-op by default, called after each
  successful append at the four sites in `run.rs` that hold the broker.
- `7e7caaf5` adds `ledger_problems` to the two witnesses.

The implementing session recorded a negative control. Independent acceptance awaits this pull
request's fresh reviews.
