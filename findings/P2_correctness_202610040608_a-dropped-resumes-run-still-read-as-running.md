---
id: PR329-A-DROPPED-RESUMES-RUN-STILL-READ-AS-RUNNING
severity: P2
disposition: deferred
category: correctness
pr: 329
reviewed_sha: 519cfc9e55ff3138585cd67ae0c733b2455a4fbd
location: src/engine/topology/recover/tests.rs:7120
provenance: pre_existing
first_bad: 523dac5feb4bdefed6abf530a98653bb7a4843d7
guard: the owner's freeze ruling on the proposed frozen hunk H3 (`reviews/2026-10-01-pr11-follow-up-b-record.md` §9.20, revised at §9.22), which makes this observation wait, bounded, for this process's own copies of the run's cleanup lease, fail at once on an observation that fails, and name its holder; on a yes, the change that merges H3 deletes this file; until then, final-range G6 keeps this sighting apart from every other and counts any recurrence of this witness failing its run observation as red
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

## The step-5 diagnosis and the proposed repair (2026-10-04)

After the supervisor's step-5 merge triage made this finding mandatory merge work for #329, `pr11_fub_step5`
diagnosed it in isolation: `~/orch-pr11/logs/pr11_fub_step5/REPORT.md`, whose directory the paths
below are relative to. `reviewed_sha` is unchanged. This section supersedes the provenance and the empty first bad
recorded above.

**Two sightings this file did not list,** both on C's branch, which contains #329 at `54a1ff14` plus C's change, so
neither predates #329 or classifies anything (`~/orch-pr11/logs/pr11_fuc_impl2/gates/`):
- `83516466`, gate 03 attempt 1, unfiltered (3,096 passed, 2 failed, 132 ignored): this witness, "two-crash: and no
  process holds the run";
- `3b4d1d79`, gate 03 attempt 1, unfiltered (3,097 passed, 1 failed, 132 ignored):
  `a_resume_over_a_stale_queued_candidate_with_nothing_staged_takes_the_staging_path_and_publishes_the_proposal`, which
  shares this helper, "stale-nothing-staged: and no process holds the run".

**What held the run, where it could be shown** (`repro/natural/`). Copies of master and of #329's head carried an
attribution that changes nothing on a passing path, and ran 11 natural whole-suite runs each (`--skip real_docker`):
- **At master,** this helper's observation failed in
  `an_error_after_the_logs_torn_tail_is_truncated_refuses_the_resume_before_any_effect_and_the_next_resume_converges`
  (`base-08.log`), through the cleanup lease: the primary lock free, the lease file open, `flock` answering
  `EWOULDBLOCK`. The first resume's one lease-holding ref write had lasted 30.8 ms, and the hold was gone about 0.6 ms
  after the observation, before the scan. No reaper of the run exists there, and the ref-writing Git child had exited.
  So the holder was a copy of that write's descriptor in a process the test process forked during it:
  `PR281-CLEANUP-LEASE-HOLD-OUTLIVED-AND-ITS-UNREADABLE-TWIN`'s class. The process itself was not captured.
- **This witness itself** did not fail, at master or at #329's head.

**By construction** (`repro/CONSTRUCTED.txt`). A sibling thread that forks every millisecond, its children keeping
their inherited descriptors 30 ms, fails this witness with this fingerprint 3 of 10 at `d724fb16`, 7 of 10 at master,
6 of 10 at #329's head, and 4 of 10 at `523dac5f`, which introduced this assertion. The forks are named as the holders.
When the children close their inherited descriptors first, it fails 0 of 10. A held lease copy, the primary lock held by
another process, this process's claim, and an unopenable `run.lock` or `cleanup.lock` each give this message; the
attribution tells them apart.

**Provenance `pre_existing`, first bad `523dac5f`** (`REPORT.md` §4.2). The defect is the helper's single observation
right after a lease-holding ref write made in this process. That observation failed naturally at master with the lease
attributed, and the witness reproduces by construction from the assertion's first commit. This test's two archived
sightings, round 6's s1 and C's `83516466`, stay individually unattributed.

**The repair, proposed:** H3 (the record's §9.20; commit `3ce7bb46`). The helper's last check is
`assert_no_process_holds_the_run`. It first makes the wait every later resume makes, bounded, then reads `is_running`
once, and a red names the wait's result and what acquiring the run lock answers. `recover/tests.rs` is G6-frozen, so H3
is proposed in RULING P-1's form, conditional on the owner's freeze ruling, and **not adopted**. This file stays until
the change that merges H3 deletes it.

## H3 revised at the B4 round (2026-10-04)

`pr11_fub_impl10` revised H3 after the i5 review's I5-1 (`reviews/2026-10-01-pr11-follow-up-b-record.md` §9.22.2 and
§9.22.3). `reviewed_sha`, provenance and first bad are unchanged, and the guard above now names the revised hunk.

- **What was wrong with H3 as proposed.** The repair described above, as first proposed, made the wait every later
  resume makes, #320's `wait_for_cleanup_hold_release_observing`. That wait observes through `cleanup::is_held`, which
  reads an inspection error as held. So an unreadable `cleanup.lock` was waited on as a holder and, once readable
  again, passed: this helper's observation, which failed on that error at once before H3, could pass after it.
- **The revision.** `assert_no_process_holds_the_run` now waits through `await_own_lease_copies_release`, which keeps
  #320's bound, rest and acknowledgement but observes through the fixture's `observe_cleanup_lease`, answering held,
  free or the error. Only a lease found held is waited on, and an observation that fails fails the helper at once,
  naming its error. Then `rundir::is_running` is read once, as before.
- **Its regression test.** `a_lease_observation_that_fails_still_fails_the_first_incarnations_death_at_once` is red at
  `d7865780` with the witnesses alone, and green under the reviewer's pre-H3 control and at the revised code
  (`~/orch-pr11/logs/pr11_fub_impl10/repro/SUMMARY.txt`).
- **Still proposed.** The revised H3 is in RULING P-1's form, conditional on the owner's freeze ruling, and not adopted.
  This file stays until the change that merges it deletes the file.
