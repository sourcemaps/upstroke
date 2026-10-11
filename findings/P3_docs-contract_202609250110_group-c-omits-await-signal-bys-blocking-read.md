---
id: PR320-R8-MAIN-002
severity: P3
disposition: deferred
category: docs-contract
pr: 320
reviewed_sha: 9b0b35f194b2abb668e26beaa6d2370b5bb98303
location: findings/P2_correctness_202609131202_a-cancelled-job-hides-which-assertion-failed.md:733
provenance: introduced_by_feature
first_bad: 9b0b35f194b2abb668e26beaa6d2370b5bb98303
guard: the next change to that finding file's round-eight section, or to `await_signal_by` in `src/agent/proc/test_support/readiness.rs`: qualify the group-C classification; documented, not enforced, since no check reads the prose
---

## Failure sequence

In #320's round eight (`9b0b35f1`), the finding file
`findings/P2_correctness_202609131202_a-cancelled-job-hides-which-assertion-failed.md` classifies
twelve polls, at `:732`–`:735`. The same claim is in #320's merged body. The claim is that each poll
makes one observation after a rest capped at its deadline, that "each observation is one
non-blocking call", and that "the look after the deadline is late only by the wake, not by a
socket's tick". For the first of the twelve, `await_signal_by`, this is not so:

1. `await_signal_by` (`src/agent/proc/test_support/readiness.rs:159`) finds the published record
   already present (`signal.exists()`, `:167`, or the second path at `:173` after the producer has
   exited).
2. It returns `published(signal)`, which calls `read_published` (`:136`), a synchronous
   `std::fs::read_to_string`. It then returns `Ready` without reading the clock again.
3. A read that is delayed, or blocks, finishes after the bound and still answers `Ready`. Atomic
   publication means the record on disk is complete, but it does not make the open and the read
   non-blocking.

So the classification states something about this poll that is false. This is exactly the
distinction round eight set out to draw, between a completion observed in time and one a blocking
read delivered late.

The finding is `introduced_by_feature` because `9b0b35f1` wrote the classification (`git blame` on
`:732`–`:735`). The read it misdescribes is older: `:166`–`:175` blame to `1cd4b1eb`, 2026-08-30,
which is an ancestor of #320's merge base `5b16f727`.

## Evidence

MAIN is the lens that raised it, as a P3, reported in full in
https://github.com/sourcemaps/upstroke/pull/320#issuecomment-5825010746. Its witness publishes an
ordinary record through the real `readiness::publish`, checks that the file is regular, and calls the
unchanged waiter with a 20 ms bound. With no delay the test passes. The delayed run uses a
process-local `LD_PRELOAD` shim that delays only a read of that uniquely named file, by 100 ms, and
then calls the real `read`. The named test fails with `Ready(["published"])` after 100.107891 ms
against 20 ms. With a temporary clock check after the read, the same delayed completion becomes a
timeout and the witness passes. Evidence on the build box:
`~/orch-pr10/reviews/pr-320r8/main-evidence/readiness-*`.

REGRESSION did not file this, but its own table of the twelve says the same thing: `await_signal_by`
"does perform an ordinary synchronous read of an already-published regular file", and the words "one
non-blocking call" "should not be read literally".

## What the change that takes this up should do

Qualify the group-C statement where it stands:

- Consuming a published record is a blocking phase outside the capped poll.
- A drain turn, which both lenses note, is a bounded batch of up to 64 non-blocking reads, not one
  call.
- The two lease pollers keep their existing production-open qualification, which is filed as
  `PR320-R7-PRODUCTION-LEASE-PROBE-RETRIES-AN-INTERRUPTED-OPEN` and is not filed again here.

Whether `await_signal_by` should also check the clock after the read, as MAIN's causal control
did, is a choice about readiness semantics. That choice is for the change that takes this up, not
for a correction of the prose. MAIN said explicitly that it does not ask for it in the capped
round.
