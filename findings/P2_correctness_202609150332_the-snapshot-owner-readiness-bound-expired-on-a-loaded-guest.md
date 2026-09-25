---
id: PR291-SNAPSHOT-OWNER-READINESS-BOUND-EXPIRED-ON-A-LOADED-GUEST
severity: P2
disposition: deferred
category: correctness
pr: 291
reviewed_sha: af71216354ad81194c5bc10b290edeee2a6e1f93
location: src/workspace.rs:2856
provenance: undetermined
first_bad:
guard: the change that takes up the readiness bounds of `src/workspace.rs`'s helper-process tests under standards §12's readiness rule, that a bound bounds a wedged producer rather than timing a healthy one
---

## Failure sequence

`workspace::tests::hard_killed_snapshot_owner_is_reclaimed_before_resume` spawns this test binary
again as a snapshot owner (`gate_snapshot_owner_helper`) and waits for it to publish readiness,
with a 15 s bound (`readiness::await_signal(&ready, owner.child(), Duration::from_secs(15))`,
`src/workspace.rs:2856`, unchanged since `13f57335`, 2026-08-29).

1. The guest is loaded. In the one sighting it ran two suites at once, and each took an order of
   magnitude longer than its normal time.
2. The owner, still alive, publishes nothing within 15 s.
3. The test fails at `src\agent\proc\test_support\readiness.rs:37:38`, and the suite ends
   `cargo exit=101`. The same test passed alone and in the next full suite on the same tree.

## The fingerprint

On the Windows guest (`windowsguest`, 32 logical processors):

```
thread 'workspace::tests::hard_killed_snapshot_owner_is_reclaimed_before_resume' (4500) panicked at src\agent\proc\test_support\readiness.rs:37:38:
the snapshot owner never published readiness: the producer is still alive and had published nothing after 15s
```

A red whose panic is that assertion, at that location, is this finding until shown otherwise. A
red of the same test with another message is not. Box logs from before the readiness rework of
2026-08-29 show the test failing as *"snapshot was not materialized"*, a different assertion
(`~/tactus-artifacts/pr36/win-failures.partial-A.txt`).

## The one sighting, and the load it met

The sighting is `~/pr10-evidence/fix-g5-c/guest/guest-full-af712163.log`, lines 2985–2988: the
first of two guest full suites at `af712163`, #291's round-0 head.

- **Run 1 took 6155.78 s** and read `2488 passed; 1 failed`, `cargo exit=101`
  (`guest-state-af712163.log`: 23:56:02 to 01:38:38).
- **Run 2, on the same tree and binary, took 469.14 s** and read `2489 passed; 0 failed`, with
  `cargo exit=0` (`guest-full2-af712163.log`).
- **Another session's suite ran on the same guest over nearly the same window.** `fix_g5_a`'s
  driver ran from 23:57:04 to 01:38:30 (`~/pr10-evidence/fix-g5-a/guest/guest-driver-494b5c61.log`).
  Its `engine::topology::recover::tests::` module took 5686.12 s
  (`guest-recover-494b5c61.log`), where the same module had taken 241.94 s on the guest at 22:43 that
  evening (`guest-recover-846a5736.log`).

## The measured rate, and what it says about #291's matrix

All at `ddf7c56a`, whose `src/` is the tree the sighting ran plus #291's round-1 repairs. On the
guest, with `NUMBER_OF_PROCESSORS=32` and `RUST_TEST_THREADS` unset, the guest's CPU and cargo
processes read before every run, and the box's load average and `vmstat` before and after
(`~/pr10-evidence/fix-g5-c/r1/guest/`, driver `guest-r1.sh`, loads in `driver-load-ddf7c56a.log`):

| runs | failed | wall time of the test | log |
|---|---|---|---|
| the test alone, through cargo | 0 of 10 | 0.55–0.69 s | `r1-snapshot-alone-<n>-ddf7c56a.log` |
| `workspace::tests::` with both kill-matrix halves, one invocation | 0 of 3 | invocations 112.47–125.00 s | `r1-workspace-with-matrix-<n>-ddf7c56a.log` |
| `workspace::tests::` with the matrix skipped | 0 of 3 | invocations 3.59–4.88 s | `r1-workspace-without-matrix-<n>-ddf7c56a.log` |
| the test alone, run again and again while both matrix halves ran in another process | 0 of 156 | median 0.810 s, max 1.31 s | `overlap-loop-ddf7c56a.log`, `overlap-stats-ddf7c56a.log` |
| the same, after the matrix ended | 0 of 10 | median 0.775 s, max 0.84 s | `overlap-loop-ddf7c56a.log`, `overlap-stats-ddf7c56a.log` |

The module-level runs overlap the matrix for their first seconds only, since the workspace tests
finish long before the matrix's first cells. The overlap runs are the ones that measure the
matrix. Under both halves' sustained load, which runs 100 children of the test binary one at a
time per half, the test's median wall time moved from 0.775 s to 0.810 s and its maximum was
1.31 s, against a 15 s bound. **#291's matrix is not implicated.**

What the runs do not establish is the failure's origin. No run here reproduced it, and there is no
reproduction at a head that predates #291. Hence `provenance: undetermined`.

## Owner and consequence

The owner is the owner's triage, until a change takes this up (`guard` above). The consequence is
a red Windows full suite on a loaded guest or `test (winguest)` leg, with a working tree. A red
matching the fingerprint above is this finding until shown otherwise; one that does not match is
a regression.

## What the change that takes this up should do

Make the wait bound a wedged owner rather than time a healthy one, as §12's readiness rule states.
Two ways, either of which fits the rule:

- **Scale the bound.** Keep the producer-aware wait (`await_signal` already returns early when the
  owner dies) and set the bound so that a starved host cannot expire it.
- **Bound the owner's progress instead.** Have the owner publish a heartbeat, and bound the silence
  between heartbeats rather than the whole wait.

Then show that the test no longer fails under the load recipe above: two suites on one guest.
