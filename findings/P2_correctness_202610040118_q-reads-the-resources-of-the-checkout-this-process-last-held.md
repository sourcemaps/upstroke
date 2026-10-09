---
id: FUC-D3-OBSERVE
severity: P2
disposition: deferred
category: correctness
pr: 330
reviewed_sha: a0464f432f3d93134360f5d5a88e60b653cd6fb4
location: reviews/2026-10-02-pr11-follow-up-c-record.md:2041
provenance: introduced_by_feature
first_bad: a0464f432f3d93134360f5d5a88e60b653cd6fb4 (design round 3's FROZENTESTS repair for Q's resource rows)
guard: only if the owner's decision O5 (#330's D2) selects Q, the quiescence path, instead of U: the round that builds Q binds the observation of R29 and R30 to the observed run and checkout, as the record's section 4.9 requires, before Q's accounting is relied on; while U is selected, as implemented on #330, neither row exists and this does not arise
---

## Failure sequence

Reasoned by #330's design review round 3, concurrency lens, at `a0464f43`
(`~/orch-pr11/reviews/review-330-d3-concurrency-a0464f43.review.md`, finding 8; the triage
`~/orch-pr11/reviews/review-330-d3-triage.md`, FUC-D3-OBSERVE). It is a finding against Q's accounting rows R29 and R30
(`reviews/2026-10-02-pr11-follow-up-c-record.md`, §2.7 and §3.5), which #330 does not build.

1. Round 3's FROZENTESTS repair has `ledger::observe` read R29 and R30 from "the checkout this process last held", a
   process-global selection, so that the frozen recovery tests' unchanged calls need no new field.
2. In one process, fixture A releases a checkout's lease while it keeps a writer record and a surviving group.
3. Fixture B then holds another checkout's lease and finishes with neither resource.
4. Observing A reads the checkout this process last held, B's, and reports A's resources absent.

Concurrent test fixtures allow this order, and sequential observations of different checkouts suffice.

## What the change that takes this up should do

The record's §4.9: bind the observation to the observed run and checkout. A binding that the frozen tests' unchanged
`ledger::observe(fold, events, physical, process)` calls can carry would have to come from the fold or the events they
pass, the run's identity; that is unverified, so FROZENTESTS is open again under Q.

## Filed

At #330's repair round 3 (2026-10-04), on the implementation review of `83516466` (the orchestrator's triage
`~/orch-pr11/reviews/review-330-i1-triage.md`, C-I3): #330's ledger had carried this as a `rejected` row with no file,
as not relevant to U as built, and the review held that every open finding of the design reviews is filed whatever
mechanism is selected.
