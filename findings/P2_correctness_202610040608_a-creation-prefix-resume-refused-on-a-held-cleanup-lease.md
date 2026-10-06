---
id: PR329-A-CREATION-PREFIX-RESUME-REFUSED-ON-A-HELD-CLEANUP-LEASE
severity: P2
disposition: deferred
category: correctness
pr: 329
reviewed_sha: 519cfc9e55ff3138585cd67ae0c733b2455a4fbd
location: src/engine/topology/recover/tests.rs:17809
provenance: pre_existing
first_bad: 6a5324e72cba7440afc6024569991a81f29dc0a8
guard: the owner's freeze ruling on the proposed frozen hunk H3 (`reviews/2026-10-01-pr11-follow-up-b-record.md` §9.20, revised at §9.22), which makes this witness's first resume wait, bounded, for this process's own copies of the run's cleanup lease and fail at once on an observation that fails; on a yes, the change that merges H3 deletes this file; until then, final-range G6 counts every occurrence matching this fingerprint as red and uses it to classify no other failure
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

## The step-5 diagnosis and the proposed repair (2026-10-04)

After the supervisor's step-5 merge triage made this finding mandatory merge work for #329, `pr11_fub_step5`
diagnosed it in isolation: `~/orch-pr11/logs/pr11_fub_step5/REPORT.md`, whose directory the paths
below are relative to. `reviewed_sha` is unchanged. This section supersedes the provenance and the empty first bad
recorded above.

**What held the lease, in a natural sighting at #329's head** (`repro/natural/head-05.log`). The attribution was the same
as for W1's file, over 11 natural whole-suite runs of each tree (`--skip real_docker`):
- The first resume's worktree-lock scan read the lease held: the file opened and `flock` answered `EWOULDBLOCK`, and a
  re-probe 27 µs later was still held.
- P8's ref write had held the lease for 16.6 ms, and the hold was gone before the scan reached it.
- No reaper of the run exists in this fixture, and P8's Git child had exited; it runs with no hooks and no fsmonitor,
  so it leaves no descendant. So the holder was a copy of P8's lease descriptor in a process the test process forked
  during that write: `PR281-CLEANUP-LEASE-HOLD-OUTLIVED-AND-ITS-UNREADABLE-TWIN`'s class, now shown for this witness. The
  process itself was not captured.
- At master this witness did not fail in 11 runs. The archive holds three natural failures before #329: `d724fb16`,
  `e980146` and `78f99c70` (`diag/census/windows.txt`).

**By construction** (`repro/CONSTRUCTED.txt`). The same forker fails this witness 8 of 10 at `d724fb16`, 4 of 10 at
master and 6 of 10 at #329's head, and 0 of 10 with the closing control. At `6a5324e7`, where this body's P8 first ran
on the repository's own ref, it fails 4 of 10; at its parent `d1c82601`, where P8 ran on the fake refs, 0 of 10. A
copy planted before the resume refuses with this fingerprint, and #320's wait, run before the resume, waits the copy
out.

**Why #320's remedy did not cover it.** Its wait (`await_previous_incarnations_release`) runs only before a fixture's
second and later resumes, and this resume is the fixture's first. The prefix is the creator's work done in this
process, which counts as no resume.

**Provenance `pre_existing`, first bad `6a5324e7`** (`REPORT.md` §4.1).

**The repair, proposed:** H3 (the record's §9.20; commit `3ce7bb46`). The body counts the prefix it wrote through the
funnels as a previous incarnation, so its first resume waits for this process's own copies as every later resume does.
A regression test plants a copy of P8's lease before the resume, and the body adopts the ref. `recover/tests.rs` is
G6-frozen, so H3 is proposed in RULING P-1's form, conditional on the owner's freeze ruling, and **not adopted**. This
file stays until the change that merges H3 deletes it.

## H3 revised at the B4 round (2026-10-04)

`pr11_fub_impl10` revised H3 after the i5 review's I5-1 (`reviews/2026-10-01-pr11-follow-up-b-record.md` §9.22.2 and
§9.22.3). `reviewed_sha`, provenance and first bad are unchanged, and the guard above now names the revised hunk.

- **What was wrong with H3 as proposed.** It counted the creation's prefix as a previous incarnation, as the section
  above says, so the first resume waited through #320's `await_previous_incarnations_release`. That wait observes
  through `cleanup::is_held`, which reads an inspection error as held. So an unreadable `cleanup.lock` was waited on as
  a holder, and the resume converged once it was readable again, where before H3 the resume refused on that error at
  once.
- **The revision.** The body no longer counts its prefix in `resume_attempts`. Before its first resume it waits through
  `await_own_lease_copies_release`, which keeps #320's bound, rest and acknowledgement but observes through the
  fixture's `observe_cleanup_lease`. Only a lease found held is waited on, and an observation that fails fails the body
  at once, naming its error. A copy past the bound leaves the resume to production, whose refusal carries the expired
  wait's note, as before.
- **Its regression tests.** `a_lease_observation_that_fails_still_fails_a_creation_prefixs_first_resume_at_once` is red
  at `d7865780` with the witnesses alone, and green under the reviewer's pre-H3 control and at the revised code
  (`~/orch-pr11/logs/pr11_fub_impl10/repro/SUMMARY.txt`).
  `a_lease_copy_that_outlives_the_bound_still_refuses_a_creation_prefixs_first_resume` holds the expired wait's note.
- **Still proposed.** The revised H3 is in RULING P-1's form, conditional on the owner's freeze ruling, and not adopted.
  This file stays until the change that merges it deletes the file.
