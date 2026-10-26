---
id: FUC-D2-PGIDREUSE
severity: P1
disposition: deferred
category: correctness
pr: 330
reviewed_sha: a9be94bc360b2e461a47c73e4d488d8a57599a15
location: reviews/2026-10-02-pr11-follow-up-c-record.md:728
provenance: introduced_by_feature
first_bad: a9be94bc360b2e461a47c73e4d488d8a57599a15 (design round 2's writer group, Q's mechanism)
guard: only if the owner's decision O5 (#330's D2) selects Q, the quiescence path, instead of U: the round that builds Q fixes this before Q is used, by the ordering the record's section 4.9 states, and before G6, which it blocks under Q; while U is selected, as implemented on #330, nothing builds the writer group and this does not arise
---

## Failure sequence

Reasoned by #330's design review round 2, concurrency lens, at `a9be94bc`
(`~/orch-pr11/reviews/review-330-d2-concurrency-a9be94bc.review.md`, finding 1; the triage
`~/orch-pr11/reviews/review-330-d2-triage.md`, FUC-D2-PGIDREUSE); not executed end to end. It is a finding against
Q, the per-checkout writer group of PR11 follow-up C's record (`reviews/2026-10-02-pr11-follow-up-c-record.md`, §2.4
and §3.5), which #330 does not build.

1. Coordinator A publishes its writer group `P` for a checkout, and spawns a Git child, which is created but has not
   yet executed `setpgid(0, P)`: glibc applies the group attribute inside the child, after the fork.
2. A dies. The sentinel `S` observes the death and leaves `P`.
3. The successor B observes `P` empty (`ESRCH`), removes the record and reuses the slot.
4. PID allocation recycles `P`, and another process creates a group `P` in A's original session.
5. The delayed child's `setpgid(0, P)` succeeds, since `setpgid` checks the group and session that exist, not their
   incarnation, and it runs Git against the slot B reused.

## What the change that takes this up should do

The record's §4.9, as round 3's review leaves Q: the child joins `P` in `pre_exec` and then checks that its original
parent is still its parent, exiting without `exec` if not. If the parent had died, the child refuses; if it was alive
at the check, the child was already in `P`, so any later emptiness observation sees it (FUC-D3-PGIDLAYER corrected
round 3's "needs a new layer"). Its cost: a `pre_exec` closure takes `std::process::Command` off `posix_spawn`, about
three times the suite's runtime as #329's round 3 measured it. Unbuilt and unreviewed.

**G6:** under Q, P1 and G6-blocking; under U it does not arise.

## Filed

At #330's repair round 3 (2026-10-04), on the implementation review of `83516466` (the orchestrator's triage
`~/orch-pr11/reviews/review-330-i1-triage.md`, C-I3): #330's ledger had carried this as a `rejected` row with no file,
as not relevant to U as built, and the review held that every open finding of the design reviews is filed whatever
mechanism is selected.
