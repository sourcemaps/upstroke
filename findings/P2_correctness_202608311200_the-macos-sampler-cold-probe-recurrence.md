---
id: PR80-MACOS-WORKSPACE-SAMPLER-COLD-PROBE-RECURRENCE
severity: P2
disposition: deferred
category: correctness
pr: 80
reviewed_sha: 2ba66b6e06fa40f9d9fe06dfd21e22517e14d2d6
location: src/workspace_manager.rs
provenance: pre_existing
first_bad: PR7-SAMPLER-SCHEDULES-FROM-A-COLD-PROBE
guard: the project owner — post-promotion sampler hardening
---

## Failure sequence

A recurrence of the `PR7-SAMPLER-SCHEDULES-FROM-A-COLD-PROBE` shape, in a sibling the earlier
fix did not reach. The workspace-manager sampler measures a one-shot probe budget; every scheduled
kill lands after its child has already completed; all 32 observations come back `Completed`; no
killed-child residue is ever sampled, and the required macOS CI leg correctly refuses the vacuous
run. Test:
`workspace_manager::tests::sampled_git_child_kills_every_residue_classified_and_recovered`, hosted
run `33421539013`, macOS job `99584874271`.

**Not attributable to the candidate's source tree**: the candidate's `src/` is byte-identical to
the green `50ed8c86ec60164011bfd393066c4c3696d3865b` source tree, so this is evidence, not a code
regression from that slice.

## A second sighting, 2026-11-24

| run | job | platform | date | pull request | checkout |
|---|---|---|---|---|---|
| `45479941648` (`pull_request`) | `138750047150`, `test (macos-latest, 3 of 5)` | a GitHub-hosted `macos-latest` runner, image `macos-26-arm64` version `20261117.0516.1` | 2026-11-24, 20:22:25Z to 20:39:17Z (16 min 52 s, inside the leg's 30-minute limit) | #415 at `b11ba8a1` | `27bbd92`, GitHub's merge of `b11ba8a1` into `master` at `c5061b81` |

From the job log (`evidence/job-45479941648-138750047150.log`, SHA-256
`33833a96961b6669730226f072ac7730ede2f5075e15f94522c4b17c6fec4612`):

- Line 1010:
  `test workspace_manager::tests::sampled_git_child_kills_every_residue_classified_and_recovered ... FAILED`.
- Lines 1015 to 1022: the four sites' budgets were 785585, 73707, 112444 and 61944 µs
  (`Object.CandidateStage`, `Object.CandidateWriteTree`, `Object.ProposalCherryPick`,
  `Worktree.Add`). Each site's summary has `unclassified=0`.
- Lines 1025 to 1032: for each of `git add`, `git write-tree`, `git cherry-pick` and
  `git worktree add`, the kill errors list eight times `Operation not permitted (os error 1)`, and
  the next line is `killed 0/8`: 32 kills, each answering `Operation not permitted (os error 1)`.
- Line 1034 is the panic at `src/workspace_manager/tests.rs:13517:9`. Line 1035: "not one of the
  32 sampled Git children died by the kill". Every end listed is `Completed`. This is the same
  test, the same floor and the same count of 32 that this file's first sighting records.
- Line 1041: `test result: FAILED. 568 passed; 1 failed; 19 ignored ... finished in 520.53s`.
  This test was the job's only failure.
- Line 717: in the same job,
  `engine::topology::recover::tests::sampled_cherry_pick_child_kills_every_residue_classified_and_recovered ... ok`.
- **The same tree passed.** Job `138567938828` of run `45428767910`, the same leg on the same
  image, checked out the same merge commit `27bbd92` (line 107 of each log). It passed this test
  (line 1008) and ran the same 588 tests in 159.13 s (line 1010). Its log:
  `evidence/job-45428767910-138567938828.log`, SHA-256
  `3ddd0df56cfdd80b7fbc0e9bce48552fc731875093401eed9f31c4eb2895bc78`.
- #415's diff is five files, all under `findings/`. It changes no `src/` file.

Two readings:

- **Inferred, not established:** the kills came late, each firing after its child had already
  exited.
- **Inferred, not established:** the kills were refused.

## A third sighting, 2026-11-27

| run | job | platform | date | pull request | checkout |
|---|---|---|---|---|---|
| `46020861627` (`pull_request`), attempt 1 | `140579863888`, `test (macos-latest, kill-path (matrices, samplers))` | a GitHub-hosted `macos-latest` runner, image `macos-26-arm64` version `20261117.0516.1` (log lines 17-18) | 2026-11-27, 14:59:44Z to 15:05:40Z (5 min 56 s, inside the leg's 30-minute limit) | #439 at `449fcfb1` | `c91f148`, GitHub's merge of `449fcfb1` into `master` at `ca058512` (log line 107) |

From the job's log as GitHub's API serves it, with ANSI escape sequences removed
(`~/selfhost-findings-20261128/evidence/job-46020861627-140579863888.log`, SHA-256
`f24c942c4ab7e09bdc4e28f0721bb0d3d8db106517b731054f80bd18071786aa`):

- Line 422:
  `test workspace_manager::tests::sampled_git_child_kills_every_residue_classified_and_recovered ... FAILED`.
- Lines 427, 429, 431 and 433: the four sites' budgets were 827468, 80294, 15237 and 31962 µs
  (`Object.CandidateStage`, `Object.CandidateWriteTree`, `Object.ProposalCherryPick`,
  `Worktree.Add`). Each site's summary (lines 428, 430, 432 and 434) has `unclassified=0`.
- Line 435: every killed group was gone within 5.083 µs of its leader's reap.
- Lines 436 to 443: for each of `git add`, `git write-tree`, `git cherry-pick` and
  `git worktree add`, the kill errors list eight times `Operation not permitted (os error 1)`, and
  the next line is `killed 0/8`.
- Line 445 is the panic at `src/workspace_manager/tests.rs:1652:9`. Line 446: "not one of the 32
  sampled Git children died by the kill". Every end it lists is `Completed`. This is the same test,
  the same floor and the same count of 32 that this file's earlier sightings record.
- Lines 448 to 479 are the 32 launches the test reports. For each, `after` is the delay its kill
  was aimed at and `fired` the time read just before the kill, both from the child's spawn:

  | line | command | after | fired | fired - after |
  |---|---|---|---|---|
  | 448 | `add` | 91.940949 ms | 176.652291 ms | 84.711342 ms |
  | 449 | `add` | 183.881898 ms | 195.426084 ms | 11.544186 ms |
  | 450 | `add` | 275.822847 ms | 422.545791 ms | 146.722944 ms |
  | 451 | `add` | 367.763796 ms | 472.317333 ms | 104.553537 ms |
  | 452 | `add` | 459.704746 ms | 588.279125 ms | 128.574379 ms |
  | 453 | `add` | 551.645695 ms | 666.048208 ms | 114.402513 ms |
  | 454 | `add` | 643.586644 ms | 791.898334 ms | 148.311690 ms |
  | 455 | `add` | 735.527593 ms | 790.324125 ms | 54.796532 ms |
  | 456 | `write-tree` | 8.921625 ms | 74.104458 ms | 65.182833 ms |
  | 457 | `write-tree` | 17.843250 ms | 64.879125 ms | 47.035875 ms |
  | 458 | `write-tree` | 26.764875 ms | 104.514666 ms | 77.749791 ms |
  | 459 | `write-tree` | 35.686500 ms | 78.347083 ms | 42.660583 ms |
  | 460 | `write-tree` | 44.608125 ms | 94.120542 ms | 49.512417 ms |
  | 461 | `write-tree` | 53.529750 ms | 56.012792 ms | 2.483042 ms |
  | 462 | `write-tree` | 62.451375 ms | 98.184417 ms | 35.733042 ms |
  | 463 | `write-tree` | 71.373000 ms | 120.002709 ms | 48.629709 ms |
  | 464 | `cherry-pick` | 1.693051 ms | 15.421042 ms | 13.727991 ms |
  | 465 | `cherry-pick` | 3.386102 ms | 23.905791 ms | 20.519689 ms |
  | 466 | `cherry-pick` | 5.079153 ms | 45.814875 ms | 40.735722 ms |
  | 467 | `cherry-pick` | 6.772204 ms | 23.209208 ms | 16.437004 ms |
  | 468 | `cherry-pick` | 8.465255 ms | 50.168542 ms | 41.703287 ms |
  | 469 | `cherry-pick` | 10.158306 ms | 74.693000 ms | 64.534694 ms |
  | 470 | `cherry-pick` | 11.851357 ms | 100.506792 ms | 88.655435 ms |
  | 471 | `cherry-pick` | 13.544408 ms | 19.773250 ms | 6.228842 ms |
  | 472 | `worktree` | 3.551403 ms | 32.407958 ms | 28.856555 ms |
  | 473 | `worktree` | 7.102806 ms | 34.057917 ms | 26.955111 ms |
  | 474 | `worktree` | 10.654208 ms | 95.084292 ms | 84.430084 ms |
  | 475 | `worktree` | 14.205611 ms | 99.091292 ms | 84.885681 ms |
  | 476 | `worktree` | 17.757014 ms | 81.439041 ms | 63.682027 ms |
  | 477 | `worktree` | 21.308417 ms | 163.349000 ms | 142.040583 ms |
  | 478 | `worktree` | 24.859819 ms | 166.081708 ms | 141.221889 ms |
  | 479 | `worktree` | 28.411222 ms | 93.740500 ms | 65.329278 ms |

- Every launch line gives `kill_error Some("Operation not permitted (os error 1)")` and
  `end Completed`. The least `fired - after` is 2.483042 ms (line 461) and the greatest
  148.311690 ms (line 454).
- Line 486:
  `test result: FAILED. 18 passed; 1 failed; 0 ignored; 0 measured; 4252 filtered out; finished in 179.01s`.
  This test was the job's only failure.

**What the test asserts**, by line at `449fcfb1` (`c91f148` has `449fcfb1`'s tree). The assertion
that failed is the kill floor, `assert!(killed > 0, …)` at `src/workspace_manager/tests.rs:1652`,
over every launch; `killed` counts the launches whose end is `LaunchEnd::Killed` (`:1645-1648`).
`kill_git_child` (`:2573`) spawns the child, sleeps for `after` (`:2575`) and then kills it, and
`SampledChild::kill` reads `fired` off the clock (`:2492`) before it sends the signal (`:2494`).

**What #439's diff does to this test.** #439 moves the workspace-manager tests into modules. At
`449fcfb1` this test, `src/workspace_manager/tests.rs:1323-1673`, is `master` `ca058512`'s
`:13196-13546` line for line; its schedule and kill helpers (`:1835-2597`) differ from `ca058512`'s
`:14172-14930` only by `pub(super)` and one rewrapped signature; and `src/workspace_manager.rs` and
`src/workspace_manager/fixture.rs` are unchanged. This is ruling r86 step 2's comparison, run again
for this record (`~/selfhost-findings-20261128/evidence/test-text-449fcfb1.txt`, SHA-256
`0889db60fdd82e769744ec0301060812732a9323f1769e9195c1e18f97d2d13f`).

**The same checkout passed.** Attempt 2 of run `46020861627`, under ruling r86, re-ran this job on
the same checkout, `c91f148` (line 107 of its log): job `140607672907`, 15:54:27Z to 15:57:05Z
(2 min 38 s), on image version `20261123.0526.1` (line 18). Line 419 logs the test `... ok`, and
line 421 reads
`test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 4252 filtered out; finished in 66.74s`.
Its log: `~/selfhost-findings-20261128/evidence/job-46020861627-140607672907.log`, SHA-256
`78c85779729080d611a35d572993625486b58003835a294e587b88c065e204af`.

**The two attempts' durations.** Attempt 1's job ran 5 min 56 s and its library harness 179.01 s
(line 486); attempt 2's job ran 2 min 38 s and its harness 66.74 s (line 421).

**What is and is not established.**

- **Established:** on one hosted macOS leg, every one of the 32 launches' `fired` is later than its
  `after`, by 2.483042 to 148.311690 ms; every kill answered
  `Operation not permitted (os error 1)`; every child ended `Completed`; and the kill floor failed.
  The same checkout passed the test on attempt 2 the same day.
- The log does not show when each child exited.
- The log does not show when each signal was sent: each `fired` is read before its signal.
- **Inferred, not established:** the kills came late, each firing after its child had already
  exited.
- **Inferred, not established:** the kills were refused.
- These readings are not exhaustive.
- **Not established: the rate.**

Other open findings, as facts and not as causes:

- `PR292-MACOS-ZOMBIE-ONLY-GROUP-EPERM-FAILS-KILL-TREE`
  (`findings/P3_portability_202609150527_macos-eperm-on-a-zombie-only-group-fails-the-kill.md:16-18`)
  records that on macOS, when a process group holds only zombies, the kernel answers the group
  signal with `EPERM`.
- `PR443-FIRING-TIMESTAMP-PRECEDES-SIGNAL`
  (`findings/P3_correctness_202611280251_the-sampled-kills-firing-time-is-read-before-its-signal.md:26-30`)
  records that `SampledChild::kill` reads `fired` before it sends the signal, so each reported
  firing is a lower bound on when the signal was sent.
- `PR443-PROBE-AND-KILL-CLOCK-ORIGINS`
  (`findings/P2_correctness_202611272326_the-census-samplers-probe-and-kill-are-timed-from-different-origins.md:18-19`)
  records that `measure_budget` includes the launch delay in the budget the kill ladder is cut
  from, while each sample's clock starts after the spawn.

**After this sighting.** #443, "the census kill sampler waits out each rung on the clock, not on a
sleep", merged on 2026-11-28 as `f84bee7b`. Nothing shows that change on macOS against this red:
the sampler's green hosted macOS runs on #443 say nothing about an intermittent red. This finding
stays open, and ruling r92 step 7 reads its next red.

## What the change that takes this up should do

Give this sampler the discipline already applied once to its sibling — warm-up probe
discarded, median-of-three, recalibration against actual duration, bounded retry — and then
demonstrate on a controlled macOS repetition that a kill actually lands, without masking the
vacuity oracle that caught this. The oracle is doing its job; the schedule is what is wrong.

Recorded in `48f2af7a:reviews/FINDINGS.md` §40. The row carries **P2** in the ledger's own words.
