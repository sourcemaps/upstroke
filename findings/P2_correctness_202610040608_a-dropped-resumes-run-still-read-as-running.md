---
id: PR329-A-DROPPED-RESUMES-RUN-STILL-READ-AS-RUNNING
severity: P2
disposition: deferred
category: correctness
pr: 329
reviewed_sha: 519cfc9e55ff3138585cd67ae0c733b2455a4fbd
location: src/engine/topology/recover/tests.rs:7120
provenance: undetermined
first_bad:
guard: final-range G6, which keeps this sighting apart from every other, counts any recurrence of this witness failing its run observation as red, and classifies it pre_existing only on a reproduction before #329's change or on causal evidence; then the change that names which of `rundir::is_running`'s answers held the run
---

## Failure sequence

`engine::topology::recover::tests::unsynced_merge_prepared_two_crash_barrier_before_cas_then_power_loss_keeps_log_and_ref_agreeing`,
the two-crash proof, a frozen recovery test, begins with `the_runs_first_resume_by_an_incarnation_that_then_dies`:

1. a first incarnation resumes the run, records `run_resumed`, and its handle is dropped;
2. the helper asserts, in one observation, that `rundir::is_running` answers false for the run
   (`src/engine/topology/recover/tests.rs:7120`);
3. once, it answered true, and the test failed with `two-crash: and no process holds the run`.

**The one sighting.** Repair round 6 of #329 (`pr11_fub_impl6`), its stage s1, 2026-10-04 about 02:21Z: a whole
library suite on this box (Linux) of the round's first staged tree, which by its test list and its time is the
tree of `0edfc509` (the tree itself was not saved). **It was filtered:** 3,087 passed, 1 failed, 130 ignored and 23
filtered out, the 23 being exactly the tests whose names contain `real_docker`, so a `--skip real_docker` filter. The
binary's tests have no result line in that log. Evidence: `~/orch-pr11/logs/pr11_fub_impl6/stage/s1/test.log`, lines
3360, 3361 and 3367; the filter and the tree are read in
`~/orch-pr11/logs/pr11_fub_impl8/sightings/s1-filter-and-tree.txt`.

**What ran after it.** The same executable passed the test alone, once (`stage/s1/recover-rerun.log`). The next
unfiltered whole suites passed it: stage s2 (3,135 passed, 0 failed, 130 ignored, 0 filtered out) and round 6's test
gate at `a337efa7` (3,137, 0 and 130, 0 filtered out). The later suites are listed in
`~/orch-pr11/logs/pr11_fub_impl8/sightings/later-runs.txt`. None of that establishes that the failure predates #329,
or its cause: an isolated pass, later green suites and recovery source that is master's byte for byte leave both
open. #329 changes what runs beside this test (a timer thread per coordinator, registry waits, concurrent fixtures),
which can move its overlap with process and lease operations elsewhere in the suite. That is a possible influence,
not a cause in evidence.

## The fingerprint

```
thread 'engine::topology::recover::tests::unsynced_merge_prepared_two_crash_barrier_before_cas_then_power_loss_keeps_log_and_ref_agreeing' (…) panicked at src/engine/topology/recover/tests.rs:7120:5:
two-crash: and no process holds the run
```

**It names no holder.** `rundir::is_running` (`src/rundir.rs:2356`) answers true when any of these holds: this
process's own claim on the run is still registered; the run's lock file exists but cannot be opened; the primary lock
is reported held; the primary lock cannot be inspected; or, with the primary lock free, the cleanup lease's probe finds
it held or cannot be made, either of which reads as held. So the assertion does not tell the primary lock,
the cleanup lease and an inspection failure apart.

**No filed finding is this one's.** None under `findings/` names this test or this message.
`PR281-CLEANUP-LEASE-HOLD-OUTLIVED-AND-ITS-UNREADABLE-TWIN` records other witnesses and other messages, and does not
establish this one's attribution.

**What the records hold of the same assertion, and why none of it classifies this sighting.** The helper's
assertion has failed in two other tests, each of which passed alone afterwards
(`~/orch-pr11/logs/pr11_fub_impl8/sightings/SIGHTINGS.txt`):
- in follow-up A's regression lens's whole suite at A's head, a tree without #329, tagged `answer-file`
  (`reviews/2026-10-01-pr11-follow-up-a-record.md` §5.2). That lens produced the same message on A's merge base by
  construction, a parked fork holding a copy of the run's cleanup lease through the assertion. So a held lease copy is
  one way to fail it;
- in design round 5's prototype of #329, tagged `open-log-truncate-error`
  (`reviews/2026-10-01-pr11-follow-up-b-record.md` §4.3).

Neither is this witness, neither ran this witness before #329, and neither identifies what answered `is_running`
here.

## Final-range G6

- **Keep this sighting separately.** It is not the cleanup-lease refusal of
  `PR329-A-CREATION-PREFIX-RESUME-REFUSED-ON-A-HELD-CLEANUP-LEASE`, whose fingerprint classifies nothing here.
- **Count any recurrence of this witness failing its `is_running` observation as red.**
- **Classify it `pre_existing` only on a reproduction before #329's change, or on causal evidence of what held the
  run.** Never on an unchanged witness, an isolated pass or a later green suite.

## What the change that takes this up should do

1. **Make a red name its holder.** At this assertion, report which of `is_running`'s answers held: this process's
   claim, an unopenable lock file, a primary-lock holder, an uninspectable lock, or the cleanup lease, held or not
   observable.
2. **Run the witness under the whole suite with #329's change and without it** (master `5c222ff2`), under the same
   load, as many times as it takes to read a difference, and say what the counts establish and what they do not.
3. **Then classify it.** A lease copy a sibling fork inherited is
   `PR281-CLEANUP-LEASE-HOLD-OUTLIVED-AND-ITS-UNREADABLE-TWIN`'s to close, through the change that closes the
   inheritance. The primary lock, or this process's claim, still held after the handle's drop would be a defect in the
   drop, and this file's.
