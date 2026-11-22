---
id: PR395-NAMING-REFUSAL-PAST-MARGIN-LEAVES-RESET-UNSPENT
severity: P2
disposition: deferred
category: correctness
pr: 395
reviewed_sha: 09cff626ec8ceef9f995aa8509604f8ab136d6bc
location: src/engine/topology/run.rs:3140
provenance: introduced_by_feature
first_bad: 3fd1877c81150a06d48e592c8257f2f75acec0c7
guard: the change that builds design 13's routing strategies, or the next change to spent_resets, awaited_reset or serving_pool
---

## Failure sequence

This is the frontier reviewer's sequence, from the fourth round of review at `09cff626`, with the
trace ruling r37 added to it. Every fact below was read from the code. None of it was run. All
times are UTC, on one day.

- `spent_resets` marks a pool's recorded reset spent when the pool rate-limits an attempt whose
  `attempt_finished` is stamped at or after that reset plus `RESET_WAIT_MARGIN`, two minutes. It
  skips the settlement that follows the pool's own `pool_exhausted`, through
  `!signalled.contains(&pool)` (`src/engine/topology/run.rs:3140`). The skip ignores the
  settlement's timestamp. So the refusal that records reset R never spends R, even when it is
  stamped at or after R plus two minutes. Rulings r33 and r35 have no such condition, and the pull
  request body says that any rate-limited settlement at or after that point spends the reset.
- The setup: a worker's agent has two pools, `primary` then `spare`. The task has two deferrals
  behind it and one left. `primary` refuses for the first time at 03:05, naming the reset 03:01.
  Two minutes past that reset is 03:03, so by the rule the reset is already spent. The run fails
  over, and `spare` refuses, naming 05:30. Both pools are now held, and the task is deferred for
  the last time.
- At 03:05, `awaited_reset` still counts 03:01 as unspent. It is the earliest reset, and two
  minutes past it is already behind the clock, so `awaited_reset` answers `None`
  (`src/engine/topology/run.rs:1851-1855`). `back_off` takes the ordinary back-off.
- At 03:06, `serving_pool` binds `primary`, the first held pool whose recorded reset is two
  minutes past (`src/engine/topology/run.rs:3157-3167`). `primary` refuses again. The task has
  spent its deferrals, so it asks a human (`src/ladder.rs:181-185`).
- By the rule, 03:01 is spent at 03:05, and the only reset left is `spare`'s. The run should wait
  to 05:32 and go on there on `spare`.
- The tests cover a first refusal inside the margin, and repeat refusals after it. None covers a
  first refusal that is already past the margin.
- `DESIGN.md` §19 says a reset is spent once its pool "refuses again" two minutes or more past it
  (`design/19_design_failure_handling.md:52-54`). The frontier pass read that as a silent
  narrowing of the ruling. It is what the code does. Only the pull request body states the broader
  rule.

**The cost, and why it is P2.** A task at its last deferral asks a human at 03:06, where the rule
says it waits for `spare`'s reset. `master` asks at the same 03:06
(`b73d347a:src/engine/topology/run.rs:1859-1860`, traced), and the regression pass agrees. Before
the last deferral, the repeat refusal at 03:06 spends 03:01 and the run then waits for `spare`
(inferred): one attempt and one deferral more than the rule allows. The frontier pass called this
a P1. It reaches none of the five things that make a P1 serious (`MAINTAINING.md:268-279`), so
ruling r37 reclassifies it P2 (`MAINTAINING.md:281-282`). It needs a refusal that names a reset
already two minutes past, where reading a reset allows for a second's lag
(`design/13_design_capacity_engine.md:15`), and a task with one deferral left.

**Why it is filed and not fixed.** Removing the exemption does not repair a slip. `3fd1877c` added
it while the comparison had no margin. `7662b5e4` added the margin to the comparison and left the
exemption as it was. Without the exemption, the other case goes wrong instead: if `primary` would
serve at 03:06, the run idles to 05:32 waiting for `spare`, where `master` and this head retry
after 60 s (inferred). That is round one's P1 again, and its repair would wait for a review pass.
Ruling r37 files it.

## What the change that takes this up should do

- Choose what a first refusal at or past the margin does to the reset it records: spend it at
  once, or give the pool one retry first. Or find one rule that serves both cases below. Spending
  at once is right when `primary` refuses again, and idles the run when `primary` would serve. One
  retry first is right when `primary` serves, and costs a deferral, or at the last deferral the
  question, when it refuses.
- Write the tests first. Each starts from a first refusal at or past the margin. There are four:
  `primary` refusing again at 03:06, and `primary` serving at 03:06, each with the task at its last
  deferral and before it.
- `DESIGN.md` §19 states the rule chosen.
