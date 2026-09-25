---
id: PR320-R8-REG-001
severity: P2
disposition: deferred
category: correctness
pr: 320
reviewed_sha: 9b0b35f194b2abb668e26beaa6d2370b5bb98303
location: src/rundir/tests.rs:9385
provenance: introduced_by_feature
first_bad: 9b0b35f194b2abb668e26beaa6d2370b5bb98303
guard: the next change to the rundir suite's real-socket reader regressions, or any fix-P2/ branch taking this up: time the close in `a_sentinel_eof_read_after_the_bound_on_a_real_socket_is_refused` from the reader's first read and keep a delayed-entry control; until then the test can fail a healthy Unix CI run under a legal schedule
---

## One defect, two IDs

Both lenses of #320's eighth and final review, of `9b0b35f1`, raised this defect. REGRESSION filed it
as `PR320-R8-REG-001` at **P2**; MAIN filed it as `PR320-R8-MAIN-001` at **P3**. It is filed once,
under the higher severity, and both labels stand as the reviewers gave them. #320's ledger has a
`deferred` row for each ID. Both reports are quoted in full in
https://github.com/sourcemaps/upstroke/pull/320#issuecomment-5825010746. The disposition is the
owner's round cap for #320: no further repair round.

## Failure sequence

`9b0b35f1` added `a_sentinel_eof_read_after_the_bound_on_a_real_socket_is_refused`
(`src/rundir/tests.rs:9382`) as the regression for `PR320-R8-SENTINEL-READER-ACCEPTS-A-LATE-EOF`.
The test takes `started` at `:9385` and spawns a thread that drops the sentinel at
`started + 20 ms + 20 ms`. It then calls `sentinel_closed_within(&mut observer, 20 ms, ..)` at
`:9393`. The reader starts its own clock at `:5305`, when it is entered. The two clocks therefore
start at different moments:

1. The test thread takes `started` and spawns the closer.
2. The test thread is delayed before it enters the reader. By the time it gets there, either the
   closer has dropped the sentinel, or more than 20 ms has passed and the drop falls inside the
   reader's own bound.
3. The reader starts its 20 ms bound. The first read returns EOF at once, or within the bound.
4. The reader correctly returns `Ok`, a close within its bound. The test panics with "an EOF read
   only after the bound is not a close within it".

The defect is confined to tests: no production code is involved, and the reader is correct. Under
a legal scheduling delay, a healthy default Unix CI run fails. The test's assertion already allows
for a late closer, but not for a late reader.

## Evidence

Both witnesses force the schedule. Neither lens claims a rate of spontaneous failure. Under ordinary
scheduling the committed test passes. Both lenses restored the old EOF order in five runs, and the
test failed all five.

- **REGRESSION.** The witness moves the existing `closing.join()` to just before the reader call,
  which forces the closer to finish first. The helper, the socket, the close timer and the assertion
  are unchanged. The named test failed 3 of 3 runs, exit 101, accepting EOF after 2.635 µs,
  2.485 µs and 1.954 µs against 20 ms. Its timely control,
  `a_sentinel_eof_read_within_the_bound_on_a_real_socket_is_accepted`, passed each time.
- **MAIN.** Before the unchanged reader call, the witness inserts only
  `while !closing.is_finished() { std::thread::yield_now(); }`. The committed test failed on an EOF
  after 4.047 µs against 20 ms.
- **The repair shape keeps the test's power.** REGRESSION's corrective control has the reader's
  first `read` send the instant the close timer starts from. It sleeps 60 ms before entering the
  reader. It passes at `9b0b35f1`, refusing the late EOF at 40.093 ms. With the old EOF arm restored
  it fails, reading `Ok` at 40.077 ms.

Evidence on the build box: `~/orch-pr10/reviews/pr-320r8/regression-evidence/sentinel-reader-scheduled-after-close/`,
`regression-evidence/synchronized-sentinel-control{,-M-A2}/`, and
`~/orch-pr10/reviews/pr-320r8/main-evidence/sentinel-reader-stalled.{patch,log}`.

## What the change that takes this up should do

Start the close from the reader's first read, as REGRESSION's control does, or give the closer and
the reader one shared absolute deadline. A pause before the reader is entered must not turn a timely
EOF into a late one. Keep a control that delays the reader's entry. Keep the old-order mutation as
well: restoring `ac1cdfbf`'s EOF arm, which returned `Ok((started.elapsed(), observations))` without
reading the bound, must still fail the repaired test.
