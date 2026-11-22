---
id: CI-FLAKE-WINDOWS-INTERLEAVING-TIME-BOUNDS
severity: P3
disposition: deferred
category: liveness
pr: TBD
reviewed_sha: b73d347af70806578051ddd58a439c0db9269a7f
location: src/engine/topology/coordinator.rs:11345
provenance: undetermined
first_bad:
guard: the change that records per-test durations on the hosted Windows legs and makes each interleaving bound report, when it passes, how long its scenario ran and how far it got (the run's configuration and order and its scheduler points for a reduced-limit run; the cell and whether the kill child had reported its kill for a finalization cell), so that a recurrence tells a slow runner, where the scenario was still advancing when the bound passed while other long tests ran beside it, from a real stall, where it stopped advancing well before the bound; or a recurrence of either failure on any leg, whichever comes first
---

# Two interleaving tests ran past their time bounds on one hosted Windows leg

## Failure sequence

    leg 4 of the hosted Windows suite runs 76 tests, several of them long interleaving scenarios, in parallel
    -> (inferred) the leg's runner is slow or oversubscribed, so each scenario takes several times longer than usual
    -> a reduced-limit run of the coordinator does not end within its 120 s `BOUND`
       -> `bounded` panics with its fixed timeout text, which names a deadlock
    -> a finalization kill child, re-run for cell 5, does not end within its 120 s `KILL_CHILD_BOUND`
       -> the child is killed and reaped, and the parent panics "the kill child did not end in time"
    -> the leg fails on these two tests; the change under test touched neither test nor the code they drive

The two bounds, at the reviewed commit:

- **`bounded`** (`src/engine/topology/coordinator.rs`, lines 11330 to 11353) runs a scenario on a thread of its own
  and waits for it with `recv_timeout(BOUND)`, where `BOUND` is 120 s (line 2181). When the wait times out it panics
  at line 11345 with:

  > {what} did not end within {BOUND:?}: the coordinator waited on its inbox for a pipeline that was itself
  > waiting on the coordinator

  That text is fixed. The `Timeout` arm prints it whatever kept the scenario from ending: a deadlock, a livelock, or
  a scenario that was still advancing, only slowly. Nothing in the arm looks at the scenario's state. **The message
  names a deadlock shape; it is not evidence of one.** The arm does not stop the scenario's thread either; the thread
  is abandoned and runs on in the harness's process.
- **`reduced_limits_and_adverse_completion_orders_reach_run_finished_within_the_step_bound`** (line 19557) runs each
  of four reduced-limit configurations under up to five adverse completion orders. On Windows, `scheduled_here`
  keeps one order per configuration (line 19566), so it makes four runs, each in its own `bounded("a reduced-limit
  run", ...)` call (line 19589) with its own 120 s. The "step bound" in its name is `STEPS`, 256 scheduler points
  (line 17523), not a time. The `what` it passes names no configuration or order, so a timeout does not say which
  run it was.
- **`a_concurrent_complete_run_finalizes_through_the_frozen_finalization_and_converges_after_a_kill_at_every_cell`**
  (line 20780) calls `finalization_kill_matrix(false)` (line 20791). That runs the finalization once to list its
  cells, then for each cell re-runs the test executable as the child `FINALIZE_CHILD` (line 20576), which runs the
  finalization again and aborts at that cell. Each child is bounded by `run_kill_child_within` with
  `KILL_CHILD_BOUND`, 120 s (`src/engine/topology/scaffold.rs`, line 2229). Past the bound it kills and reaps the
  child (`src/workspace_manager/fixture.rs`, lines 1246 to 1253) and returns `None`, and the test panics at line
  20844. The child's standard output and error are discarded (lines 1235 and 1236), so how far it got is not
  recorded anywhere.

## The one sighting

| run | job | platform | date | pull request | head |
|---|---|---|---|---|---|
| `44741248743` (`pull_request`) | `136305204188`, `test (windows-latest, 4 of 10)` | a GitHub-hosted `windows-latest` runner | 2026-11-20, 13:09:04Z to 13:22:37Z | #392 | `2c18bb4c` |

Read from the raw job log (`evidence/job-44741248743-136305204188.log`, SHA256
`74ea60a1468d200cdbaca42e1268284b9eac3fc0e6b1a0f98b5b650c0ec8fe0d`):

- Two tests of `engine::topology::coordinator::tests::interleaving` FAILED:
  - `reduced_limits_and_adverse_completion_orders_reach_run_finished_within_the_step_bound`, panicking at
    `src\engine\topology\coordinator.rs:11345:64` with:

    > a reduced-limit run did not end within 120s: the coordinator waited on its inbox for a pipeline that was
    > itself waiting on the coordinator

  - `a_concurrent_complete_run_finalizes_through_the_frozen_finalization_and_converges_after_a_kill_at_every_cell`,
    panicking at `src\engine\topology\coordinator.rs:20844:36` with:

    > complete, cell 5 (Ref.DeleteCandidatesRef after): the kill child did not end in time

- Both printed libtest's notice that a test is still running after 60 seconds:

      test engine::topology::coordinator::tests::interleaving::reduced_limits_and_adverse_completion_orders_reach_run_finished_within_the_step_bound has been running for over 60 seconds
      test engine::topology::coordinator::tests::interleaving::a_concurrent_complete_run_finalizes_through_the_frozen_finalization_and_converges_after_a_kill_at_every_cell has been running for over 60 seconds

  Several other tests of the same module printed the same notice and then passed.
- `test result: FAILED. 74 passed; 2 failed; 7 ignored; 0 measured; 3554 filtered out; finished in 590.79s`. These
  two were the job's only failures.

Pull request #392 re-split the hosted Windows legs. The Rust it changes is four test files (`src/effects/tests.rs`,
`src/effects/tests/ci_model.rs`, `src/effects/tests/workflow.rs` and `src/engine/tests.rs`); it changes neither failing
test nor the code they drive. The same tree, `dc3e7e3e`, passed every Windows leg, these two tests included, in CI run
`44736631711` on 2026-11-20 at 12:32Z, and failed here in run `44741248743` 35 minutes later. Run `44741248743` was
started by an edit to #392's description, not by a new commit. The next run to pass was of a different tree,
`83bb3c8b`, so no rerun of `dc3e7e3e` passed after this failure. Leg 4 holds 76 tests at
`2c18bb4c`, against 59 before the re-split (`docs/internals/effects/tests/ci_model.md`, the shard paragraphs: the
re-split's counts, 50, 27, 99, 76, ..., against the 2026-11-19 counts, 50, 27, 116, 59, ...).

## What is and is not established

**Established.** On one hosted Windows leg, one reduced-limit run of the coordinator did not end within 120 s, and
one finalization kill child, at cell 5 of the complete run, did not end within 120 s. Both tests, and several other
tests of the module that passed, had been running for over 60 seconds. The change under test touched neither failing
test nor the code they drive, and the same tree, `dc3e7e3e`, passed both tests on every Windows leg 35 minutes
earlier.

**Established: the first panic's wording says nothing about the cause.** It is the fixed text of `bounded`'s
`Timeout` arm (line 11345), printed for any scenario that has not ended within `BOUND`.

**Not established: which reduced-limit run timed out, or how far either scenario got.** The `what` names no
configuration or order, the timeout reports neither the scenario's scheduler points nor its elapsed time, and the
kill child's output is discarded. Whether the child at cell 5 had reached the cell and failed to die, or was still
on its way to it, is not known.

**Inferred, not established: a slow runner, not a deadlock.** That both bounds were reached because the leg's runner
was slow or oversubscribed while several long interleaving tests ran in parallel, not because the coordinator or the
finalization stalled. What supports it: the same tree passed both tests on every Windows leg 35 minutes earlier;
several tests of the module passed the 60-second notice in this job; the leg holds 17 more tests than it did before
the re-split; and the two failures are of different scenarios through different bounds. Two things may have made it
worse, and neither is established: the abandoned thread of the timed-out reduced-limit run kept running beside the
other tests, and `Child::kill` kills the kill child but not any process the child started. **What would establish
it:** per-test durations on that leg, which the hosted Windows step does not record today, and a count of reruns of
the same tree in which these tests pass or fail.

**Not established: the rate.** One sighting; the other Windows jobs of 2026-11-20 were not counted.

**Related, open:** pull request #394 splits the second test into two parts by cell parity. That shortens each part,
and so the time it runs beside others, but changes neither bound nor what a bound reports when it passes.

## What the change that takes this up should do

Gather what tells a slow runner from a stall first, and only then decide whether a bound or the code changes.

- **Record per-test durations on the hosted Windows legs**, for instance by having the suite step write each test's
  start and end, or its duration, into the job's log or an artifact, so that a recurrence shows how long each test
  ran and which long tests overlapped it.
- **Make each bound report how long its scenario ran and how far it got** when it passes:
  - `bounded`: replace the fixed deadlock text with the time waited and a progress reading the scenario publishes
    as it runs; for a reduced-limit run, the configuration and order (its `index`, `limits.name`, `order` and
    `seed`) and its count of scheduler points against `STEPS`. Keep the deadlock shape as one possible reading, not
    the message's claim;
  - `run_kill_child_within` in the finalization matrix: report the time waited, and whether the child had written
    its `FIRED` report before it was killed, which tells a child that reached its cell and did not die from one
    still on its way there. Keeping the last lines of the child's output, rather than discarding them, would say
    more.
- **Then decide.** If a recurrence shows the scenario still advancing when the bound passed, the bound measures the
  runner: widen it on Windows or rebalance the leg. If it shows no progress for most of the bound, it is a stall to
  take up as a correctness finding in the coordinator or the finalization.
- **Do not retry or skip the tests** to get past this, and do not widen either bound before a recurrence says which
  case it is.
