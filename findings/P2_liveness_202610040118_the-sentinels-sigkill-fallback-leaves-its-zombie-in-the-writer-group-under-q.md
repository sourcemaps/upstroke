---
id: FUC-D3-SENTINELFALLBACK
severity: P2
disposition: deferred
category: liveness
pr: 330
reviewed_sha: a0464f432f3d93134360f5d5a88e60b653cd6fb4
location: reviews/2026-10-02-pr11-follow-up-c-record.md:2036
provenance: introduced_by_feature
first_bad: a0464f432f3d93134360f5d5a88e60b653cd6fb4 (design round 3's release protocol for Q's sentinel)
guard: only if the owner's decision O5 (#330's D2) selects Q, the quiescence path, instead of U: the round that builds Q closes it or carries it as the stated residual of the record's section 4.9, with its crash-table row corrected and an oracle for the fallback; while U is selected, as implemented on #330, nothing builds the sentinel and this does not arise
---

## Failure sequence

Reasoned by all three lenses of #330's design review round 3 at `a0464f43`
(`~/orch-pr11/reviews/review-330-d3-{design,concurrency,regression}-a0464f43.review.md`; the triage
`~/orch-pr11/reviews/review-330-d3-triage.md`, FUC-D3-SENTINELFALLBACK). It is a finding against Q's sentinel, the
member that keeps a writer group non-empty (`reviews/2026-10-02-pr11-follow-up-c-record.md`, §2.4 and §3.5), which #330
does not build.

1. Release sends the sentinel `SIGTERM` and then `SIGCONT`; its handler would call `setsid()` and `_exit(0)`.
2. The sentinel has not run its handler within the one-second bound, and release sends the permitted `SIGKILL`.
3. The coordinator dies before it reaps the sentinel.
4. A non-reaping adopter keeps the sentinel's zombie in `P`, and every later acquisition of the checkout refuses until
   the adopter reaps it or the machine restarts.

Sending a signal does not establish that `setsid()` has run, so the crash-table row "after the signal and before the
reap: `P` holds no sentinel" is false: `P` holds no sentinel once the handler has run, and before that, and after the
fallback, it may hold the sentinel's zombie.

## What the change that takes this up should do

The record's §4.9: close the fallback, or carry it as a stated residual (P2, liveness, the operator's remedy), with the
crash-table row corrected and a test for the fallback path. Round 3 had marked it fixed; it is not.

## Filed

At #330's repair round 3 (2026-10-04), on the implementation review of `83516466` (the orchestrator's triage
`~/orch-pr11/reviews/review-330-i1-triage.md`, C-I3): #330's ledger had carried this as a `rejected` row with no file,
as not relevant to U as built, and the review held that every open finding of the design reviews is filed whatever
mechanism is selected.
