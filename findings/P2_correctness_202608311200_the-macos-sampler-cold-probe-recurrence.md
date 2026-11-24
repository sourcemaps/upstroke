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

## What the change that takes this up should do

Give this sampler the discipline already applied once to its sibling — warm-up probe
discarded, median-of-three, recalibration against actual duration, bounded retry — and then
demonstrate on a controlled macOS repetition that a kill actually lands, without masking the
vacuity oracle that caught this. The oracle is doing its job; the schedule is what is wrong.

Recorded in `48f2af7a:reviews/FINDINGS.md` §40. The row carries **P2** in the ledger's own words.
