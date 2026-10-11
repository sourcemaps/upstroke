---
id: PR320-R8-MAIN-003
severity: P3
disposition: deferred
category: correctness
pr: 320
reviewed_sha: 9b0b35f194b2abb668e26beaa6d2370b5bb98303
location: src/rundir/tests.rs:5313
provenance: introduced_by_feature
first_bad: 9b0b35f194b2abb668e26beaa6d2370b5bb98303
guard: the next change to `Lifeline::account` or `sentinel_closed_within` in `src/rundir/tests.rs`: pass the fact that EOF was observed, late or not, as the cut's `presence_closed`, with a regression on a late EOF and the macOS not-connected answer; the lifeline tests assert `when_the_wrapper_returned` before `cleaned()`, so meanwhile the defect misreports cleanup and does not pass a test falsely
---

## Failure sequence

Since `9b0b35f1`, `sentinel_closed_within` returns
`Err(SentinelHeldPastBound { eof_late: true, .. })` at `src/rundir/tests.rs:5313` for an EOF read
only after its bound. Before `9b0b35f1` it returned `Ok`. That error now carries two facts: the close
missed the deadline, and EOF had been observed. `Lifeline::account` keeps only the first:

1. A scenario's last holder of the lifeline's presence end closes it after `LEFTOVER_BOUND` (5 s,
   `:7240`). The observation made when the wrapper returns reads that EOF late and answers
   `Err(.. eof_late: true ..)`.
2. `account` passes `when_the_wrapper_returned.is_ok()`, which is `false`, to `cut_answered` as
   `presence_closed` (`:7452`–`:7455`).
3. Once nothing holds the cut's other end, native macOS answers the cut's `shutdown` with
   `ENOTCONN`, where Linux answers `0`
   (`PR320-R7-MACOS-LIFELINE-CUT-NOT-CONNECTED`, handled by
   `cutting_a_lifeline_whose_other_end_every_holder_has_closed_is_a_cut`). `cut_answered` (`:7413`)
   reads not-connected with `presence_closed == false` as "a process of the scenario still held the
   presence end: the two answers disagree, and the cut is not taken as made".
4. The next presence observation reads EOF at once, and no group has a live member. Even so,
   `Leftovers::cleaned()` (`:7335`) is `false`, because the cut is `Err`. The cleanup notes appended
   to the scenario's text say the cut was not made, although EOF had been observed and every holder
   had closed.

This is a defect in the cleanup report, on top of the correct refusal of the late close. At the
reviewed head, all four tests that assert `leftovers.cleaned()` assert
`leftovers.when_the_wrapper_returned.is_ok()` first (`:8436`, `:8505`, `:8559` and `:8628`).
A late EOF therefore fails them first, for the right reason, and the wrong cut verdict is only in the
failure's notes. No test passes falsely through this path.

## Evidence

MAIN raised this as a P3, reported in full in
https://github.com/sourcemaps/upstroke/pull/320#issuecomment-5825010746. The witness is an
append-only integration test. It calls the real, unchanged `Lifeline::account` on private socket
pairs. To force the late EOF it widens only the observer socket's tick to 8 s; `LEFTOVER_BOUND`
stays 5 s, and both peers close at about 5.2 s. A process-local shim reproduces the macOS
`ENOTCONN` answer after the real `shutdown`. The results:

- The Linux shutdown control passes.
- With the shim, the witness fails. The first observation is
  `Err(SentinelHeldPastBound { .. waited: 5.200030058s, .. eof_late: true })`, the cut is
  `Err("... the two answers disagree, and the cut is not taken as made")`, and the second
  observation is `Ok((1.343µs, 1))`.
- Keeping the known-EOF fact when building `presence_closed` makes the same witness pass, and the
  original missed-deadline error is still kept. The existing cut-contract test also passes with that
  change.

MAIN states the limit itself: this is **a Linux run of the documented macOS answer, not a native
macOS reproduction**. Widening the tick forces the late-EOF variant; how the caller reads it does not
depend on the tick. Native CI passed the ordinary lifeline tests at `9b0b35f1` (CI 36078103868) but
does not reach this combination. REGRESSION's report makes the same reading without filing it: a
late EOF "can withhold the `presence_closed` predicate used to accept macOS `ENOTCONN`". Evidence on
the build box: `~/orch-pr10/reviews/pr-320r8/main-evidence/lifeline-*` and `mac_shutdown_answer.c`.

## What the change that takes this up should do

In `account`, pass "EOF was observed" as `presence_closed`: `Ok`, or `Err` with `eof_late`. Keep
the late close as the first finding. Hold the change with a regression that combines a late EOF with
the not-connected answer. MAIN's causal patch is the shape. The regression must fail when `is_ok()`
is restored and leave `cutting_a_lifeline_whose_other_end_every_holder_has_closed_is_a_cut` passing.
