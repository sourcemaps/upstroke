---
id: CI-FLAKE-DISPATCH-SAMPLER-NO-KILL-WHILE-WRITING
severity: P3
disposition: deferred
category: correctness
pr: TBD
reviewed_sha: fe2b9bc58af6c3eae0d0a9da64ac25a159db374f
location: src/engine/topology/dispatch/tests.rs:1578
provenance: undetermined
first_bad:
guard: the change that makes the dispatch sampler's requirement that at least one kill lands while the pick is writing hold on a slow or unevenly scheduled machine, or replaces the timing sample with a deterministic interruption, keeping the requirement itself
---

# The dispatch sampler's kills never landed while the pick was writing on a hosted Windows runner

## Failure sequence

    the sampler times four probe picks and aims its kills inside the median it measured
    -> one sample's pick completes before its kill, and the ladder re-aims every later kill inside that completion
    -> each kill lands before its child has written anything (`None`), or once the pick has finished (`After`)
    -> the loop reaches `MAX_SPAWNS` without a kill while the pick was writing
    -> the `while_writing` assertion fails the test

`engine::topology::dispatch::tests::sampled_repair_materialization_child_kills_every_residue_classified_and_recovered`
(`src/engine/topology/dispatch/tests.rs`, line 1369) runs `git cherry-pick --no-commit` children
and kills each at a delay. `fixture::KillBudget` sets the delay. It starts from the median of three
probe picks, timed after a discarded warm-up. It then re-aims inside every pick that completed
before its kill.

A completed pick is a control, not a sample. The loop runs until `SAMPLING_N` (8) children have
died by the kill and at least one of those kills landed while the pick was writing. It stops at
`MAX_SPAWNS` (64) spawns. `KilledSample::while_writing` (line 1610) counts a kill as landing while
writing when it left `Internal` residue, or `After` without `MERGE_MSG`. `MERGE_MSG` is the last
thing a completed `--no-commit` pick writes. The last assertion of the test is that one such kill
happened (`kills.iter().any(KilledSample::while_writing)`, line 1578). **That is the assertion that
failed.**

## The one sighting

| run | job | platform | date | pull request | head |
|---|---|---|---|---|---|
| `44975798003` (`pull_request`) | `137065317614`, `test (windows-latest, 10 of 10)` | a GitHub-hosted `windows-latest` runner | 2026-11-22 | #399 | `fe4bfa4e`, an empty commit on `144485c7` |

Read from the raw job log (`evidence/job-137065317614.log`, SHA256
`f1d33a829afe39a9952de72a8202683a9445054c9c81daaa7939feee9dd280b3`):

- Line 1154: `test
  engine::topology::dispatch::tests::sampled_repair_materialization_child_kills_every_residue_classified_and_recovered
  ... FAILED`.
- The panic, at `src\engine\topology\dispatch\tests.rs:1578`, is the `while_writing` assertion. It
  reads "none of the 63 kills in 64 spawns landed while the pick was writing". Its schedule says:
  - the probe measured **58.9378 ms**, and the ladder followed **one completion**: sample 5
    completed in **34.013 ms**, `After`;
  - every later kill was aimed between **3.779 ms and 30.234 ms**;
  - of the 63 kills, **62 found nothing written** (`None`). **One**, sample 54, was killed at
    44.15 ms and found `After`. Because it did not count as landing while writing, `MERGE_MSG` was
    in place: the pick had finished.
- Line 2469: `test result: FAILED. 2020 passed; 1 failed; 39 ignored; 0 measured; 1607 filtered out;
  finished in 549.87s`. This test was the job's only failure.

**The same tree passed in the previous run.** CI run 44968824584's job 137043887481,
`test (windows-latest, 10 of 10)` on `144485c7`, succeeded. `fe4bfa4e` is an empty commit on that
tree.

**#399 adds no test to this leg** (ruling r36). Leg 10 skips `engine::topology::coordinator::`,
`engine::topology::recover::` and `workspace_manager::`. Every test #399 adds or renames is under
those prefixes. `master`'s leg 10 (run 44919497444 at `b73d347a`) ran with 1,601 filtered out in
535 s; this run had 1,607 filtered out and took 550 s.

**Prior sighting.** The P1 class row records one sighting of this test
(`findings/P1_liveness_202609031117_intermittent-kill-settle-and-residue-failures.md:53-57`), on #283.
At head `bdd63edf342a`, the identical commit produced a `test (winguest)` failure (run
34782632803) and then a success (run 34783138109). Only the pull request's body changed between
the two runs, not the tree: "Red and green on one SHA." The row's next paragraph (`:59-66`) quotes
*"none of the 63 kills in 64 spawns landed"*, the opening words of this assertion's message. It
reads them as what a timing probe reports when it is descheduled, on a self-hosted host whose CPU
was overcommitted. It records that as a resource observation, not a proven cause. That sighting was
on the self-hosted guest. This one is on a GitHub-hosted runner.

## What is and is not established

**Established.** On one hosted Windows leg, the sampler spent all 64 spawns without a kill landing
while the pick was writing. 62 kills found nothing written, one found a finished pick, and one
pick completed. The aims of every kill after the completion fell between 3.779 ms and 30.234 ms.
The tree had passed the same leg in the previous run. #399 adds no test to this leg, and the run
was the empty commit `fe4bfa4e` on that tree.

**Inferred, not established: the aim missed the write window because neither measurement predicts
when a pick writes on a loaded hosted runner.** The probe's 58.9 ms and the one completed pick's
34.0 ms do not say when a pick begins and ends writing on this runner. 62 kills aimed as late as
30.2 ms found nothing written, yet sample 5 had finished within 34.0 ms. So either the write window
lies in the few milliseconds after the latest aim, or it moved from sample to sample as the runner's
load changed. The log has no utilisation data: nothing records the runner's CPU load, or what else
was running beside the test. **What would establish it:** per-sample write-start times, meaning when
each child first wrote (its `index.lock`) and when it wrote `MERGE_MSG`, beside the aim and the
time the kill returned. These would show where the write window was relative to the aims, and
whether it moved.

**Not established: the rate.** One sighting on a hosted runner. The other runs of this test on
2026-11-22 were not counted.

## Why this is its own row

- **The P1 class row says the kill samplers are not its members.** It limits itself to its four
  named fingerprints
  (`findings/P1_liveness_202609031117_intermittent-kill-settle-and-residue-failures.md:28-40`):

  > **This row limits itself to the four named fingerprints above, and that limit is
  > load-bearing.**

  > The four **kill samplers** below share this row's *shape* — an intermittent failure in a
  > subprocess kill path — and are **not** members of it.

  This test is one of the four samplers it lists. Recording this sighting there would also edit
  that row, which this finding does not do.
- **`PR249-KILL-SAMPLER-WINDOWS-WRAPPER`**
  (`findings/P2_correctness_202609090020_pr5s-census-sampler-kills-gits-launcher-on-windows.md`)
  covers the census sampler (`workspace_manager::tests::sampled_git_child_kills_…`) and Git for
  Windows' launcher. That is a different test and a different mechanism: a kill that ends the
  launcher and not Git. This sampler already runs the real binary (`fixture::sampled_git`, per its
  doc comment at lines 1358-1363).
- **`CI-FLAKE-CHERRY-PICK-SAMPLER-NO-PICK-UNDER-WAY`**
  (`findings/P3_correctness_202611120400_the-cherry-pick-sampler-interrupted-no-pick-under-way-on-macos.md`)
  is the same requirement in the recover sampler
  (`src/engine/topology/recover/tests.rs`). That is a different test, and its own row. This finding
  takes its guard from that one.

## What the change that takes this up should do

- **Record per-sample write-start times**, as above, so that a recurrence shows whether the kills
  land early because the children start late, or whether the write window moves between samples.
- **Make the requirement hold on a slow or unevenly scheduled machine, or interrupt the pick
  deterministically.** Either is acceptable. A schedule that holds only where the runner is fast and
  evenly scheduled is not.
- **Keep the requirement that at least one kill lands while the pick is writing.** Do not weaken
  `kills.iter().any(KilledSample::while_writing)`, and do not retry or skip the test to get past
  it.

**A proposal, not adopted and not reviewed:** have the child signal when it begins writing, for
instance by watching for the worktree's `index.lock`, and fire the kill from that signal rather
than from a delay measured in advance. The kill would then follow the pick's writes on any runner,
and the timing sample would no longer decide whether one lands.
