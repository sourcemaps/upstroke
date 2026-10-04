---
id: PR329-A-CREATION-PREFIX-RESUME-REFUSED-ON-A-HELD-CLEANUP-LEASE
severity: P2
disposition: deferred
category: correctness
pr: 329
reviewed_sha: 519cfc9e55ff3138585cd67ae0c733b2455a4fbd
location: src/engine/topology/recover/tests.rs:17809
provenance: undetermined
first_bad:
guard: final-range G6, which counts every occurrence matching this fingerprint as red and uses it to classify no other failure; then the change that takes up `PR281-CLEANUP-LEASE-HOLD-OUTLIVED-AND-ITS-UNREADABLE-TWIN`, which re-runs this witness and deletes this file only on evidence of what held the lease
---

## Failure sequence

`engine::topology::recover::tests::a_resume_over_a_creation_that_stopped_after_creating_its_integration_ref_adopts_it`,
a frozen recovery test, builds a creation's prefix through the funnels, its integration ref included, and then resumes
the run (`resume_with_real_refs`, `src/engine/topology/recover/tests.rs:17809`). It is the fixture's first resume,
so it waits for no lease: `await_previous_incarnations_release` waits only before a fixture's later resumes. Then:

1. the resume takes the worktree lock, and `WorktreeLock::acquire_in_hooked` (`src/rundir.rs:1906`) observes each
   run's cleanup lease once;
2. the observation reads the lease held. A held lease reads so, and so does an observation that cannot be made: it
   fails closed (`observe_cleanup_hold`, `src/rundir.rs:2140`);
3. the resume refuses, and the test fails.

## The fingerprint

```
thread 'engine::topology::recover::tests::a_resume_over_a_creation_that_stopped_after_creating_its_integration_ref_adopts_it' (…) panicked at src/engine/topology/recover/tests.rs:17809:10:
the resume over the creation's prefix converges: Refused { message: "run `01KZTPR7E00000000000000001` still has a process of its own alive in worktree …/repo -- an agent-cleanup reaper, or a Git child writing one of its refs -- and that process holds the run's cleanup lease; refusing overlapping engine ownership" }
```

A red of this test with this refusal at this line matches the fingerprint. A red of this test with another message
does not, and neither does this refusal in another test, which is
`PR281-CLEANUP-LEASE-HOLD-OUTLIVED-AND-ITS-UNREADABLE-TWIN`'s record.

## The sightings

All three on Linux, the logs' lines in `~/orch-pr11/logs/pr11_fub_impl8/sightings/SIGHTINGS.txt`:

- **This one, repair round 7 of #329** (`pr11_fub_impl7`, 2026-10-04): the whole suite on a CRLF copy of `c3bde5fd`
  (`git archive`, 874 text files given CRLF endings), `--skip real_docker`. The library passed 3,115, failed 1, this
  test, and ignored 130, **with 23 filtered out**; the binary passed 10. The same executable then passed the test
  alone 5 of 5, and the library suite again with the same filter: 3,116 passed, 0 failed, 130 ignored, **23 filtered
  out**. Neither run was an unfiltered suite (`~/orch-pr11/logs/pr11_fub_impl7/crlf/`).
- **G5's S1 at `d724fb16`**, a tree before PR11 and before #329, unfiltered: the same test, the same refusal
  (`reviews/2026-09-25-gate-G5.md`, F16 and §2.1). G5 counted it red and classified it by the fingerprint, not by a
  cause.
- **Design round 6's prototype of #329**, its suite 3, unfiltered (`reviews/2026-10-01-pr11-follow-up-b-record.md`
  §5.8).

The test's text, its wrapper and the shared body it calls, and the four fixture functions it resumes through are the
same at `d724fb16`, master `5c222ff2` and `519cfc9e`
(`~/orch-pr11/logs/pr11_fub_impl8/sightings/creation-test-body-d724fb16-vs-519cfc9e.txt`).

## What is and is not established

- **A fingerprint, not a cause.** `PR281-CLEANUP-LEASE-HOLD-OUTLIVED-AND-ITS-UNREADABLE-TWIN` records this refusal from
  other recovery tests' resumes. It attributes them, by construction and not per sighting, to a copy of the run's lease
  descriptor that a sibling thread's fork inherited during a ref write, and says that which fork held it in each
  natural sighting is not identified. It does not name this test. So this sighting is compatible with that fingerprint, and what held the
  lease here, or whether the observation failed, is unproven. Five isolated passes and a green rerun prove no
  mechanism.
- **The fingerprint predates #329.** G5's S1 shows this test failing this way on a tree without #329's change. That
  is not a cause for this sighting either, so its provenance stays undetermined.
- **It classifies nothing else.** In particular, not `PR329-A-DROPPED-RESUMES-RUN-STILL-READ-AS-RUNNING`, a different
  assertion whose observation reads more than the lease.

## Final-range G6

- **Count every occurrence matching this fingerprint as red.**
- **A match is not a cause**, and it classifies no other failure.

## What the change that takes this up should do

1. **Make a red name its holder.** When the first resume's observation refuses, report what held the lease, or that
   the observation itself failed.
2. **Re-run this witness under the whole suite, unfiltered,** after the change that closes
   `PR281-CLEANUP-LEASE-HOLD-OUTLIVED-AND-ITS-UNREADABLE-TWIN`'s inheritance, and delete this file only on evidence of
   what held the lease here.
3. **Whether a fixture's first resume should wait out a held lease,** as #320 made its later resumes do (G5's §2.1
   notes that S1's refused resume was a fixture's first), is that change's to decide. The production refusal is not in
   question here.
