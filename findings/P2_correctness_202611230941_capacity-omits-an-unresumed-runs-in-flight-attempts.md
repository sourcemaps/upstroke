---
id: PR403-PRE-RESUME-IN-FLIGHT-ATTEMPT-UNCOUNTED
severity: P2
disposition: deferred
category: correctness
pr: 403
reviewed_sha: afbf76b8ef3a088ef8366ed630628f7e3efd2499
location: src/capacity.rs:694
provenance: introduced_by_feature
first_bad: 04235641b3291fa7c6afe2be023c307d63563c5e
guard: fix-P2/correctness_capacity-omits-an-unresumed-runs-in-flight-attempts, opened when #403 merges (ruling r45)
---

## Failure sequence

Found by round three's frontier pass, which reasoned from the code and did not run it.

1. Attempt A settles on pool P for $1.
2. Attempt B starts on P, and the coordinator dies before it writes B's settlement.
3. The run lock is released, but the log's valid lines end at B's `attempt_started`.
4. `upstroke status` counts B. For a run that is neither held nor finished,
   `Observed::accounting` (`src/engine/topology/observe.rs:278-293`) takes the committed log and
   appends an interrupted settlement for each attempt `closure::in_flight` finds in flight. The run
   report counts each interrupted settlement on the pool its `attempt_started` named, with no cost
   (`src/engine/report.rs:243-259`, `:528-548`). The per-pool drain line therefore reads
   `P: 2 attempt(s), $1.0000?` (`src/engine/report.rs:1082-1090`).
5. `capacity` and `validate` do not count B. `latest_run_observations` folds only the committed log
   (`src/capacity.rs:694-695`, `observed.events()`), and `observe_topology` counts an interrupted
   attempt only when the log holds its `attempt_interrupted` (`src/capacity.rs:242-249`).
   `validate` reads the same function (`src/validate.rs:241`). Both report A alone: the pool line
   reads `this run drew $1.0000 over 1 attempt(s)` with no `?` (`src/capacity.rs:464-470`), and
   with a monthly allowance the note that unpriced attempts make the ceiling's draw a floor is
   missing as well (`src/capacity.rs:531-536`). This lasts until the run is resumed.

So for one run on one log, `status` and `capacity` disagree on how many attempts drew on P, and only
`status` marks the spend as unknown.

Why this is not serious, and why it is deferred:

- **The log is right, and nothing binds on the figure.** The resume appends the interrupted
  settlement, and the capacity report counts it from then on. The two callers of
  `latest_run_observations` are `capacity::report` (`src/capacity.rs:743`) and `validate`'s
  capacity echo (`src/validate.rs:214-245`), and each builds a report from it for display. The
  coordinator's own capacity snapshot does not read it: it folds the coordinator's in-memory
  records through `capacity::drain_of` (`src/engine/coordinator.rs:960`).
- **Ruling r45 files it** rather than opening a fourth review round in #403.
- **The legacy (schema-3) reader has the same gap, and had it before #403.** This is inferred, not
  run. Legacy `status` settles in-flight attempts as interrupted when the run is not held
  (`src/status.rs:167-172`), while the legacy branch of `latest_run_observations` folds only the
  logged events (`src/capacity.rs:697-698`) and counts only an `attempt_interrupted` the log holds
  (`src/capacity.rs:214-216`).

## What the change that takes this up should do

This follows ruling r45 step 8. It lands in its own pull request on the guard's branch.

- **Test first.** Copy
  `a_schema_4_logs_interrupted_attempt_counts_once_unpriced_on_the_pool_it_started_on`
  (`src/capacity.rs:2118`) without its `run.push(interrupted(BETA))`. Assert that `claude-max` is
  `usd: Some(1.0)`, `attempts: 2`, `unpriced: 1`; this must fail on `master`. Assert that
  `crate::status` renders `claude-max: 2 attempt(s), $1.0000?` for the run. With
  `crate::rundir::RunLock::acquire` held, assert `attempts: 1`, `unpriced: 0`.
- **Code.** Add one accessor on `Observed` that returns the events `accounting` folds, and make
  `accounting` read through it. Then make `latest_run_observations` fold that accessor's events in
  place of `events()`.
- **Stops.**
  - Change nothing else under `src/engine/`.
  - Add no new visible function to `src/capacity.rs`.
  - Change no existing test.
  - If the `effects::` gate names a row, stop: that row is the owner's to approve.
  - Leave the legacy reader as it is.
