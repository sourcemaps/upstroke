---
id: PR411-MEDIAN-TARGET-STILL-MISSED
severity: P2
disposition: deferred
category: performance
pr: 411
reviewed_sha: 857a912b4ea483e2fa79b178d4cfae365cbdc649
location: .github/workflows/ci.yml:351
provenance: introduced_by_feature
first_bad: 43f5f8f6432af07d33766cddbe8d2017c05c62ea
guard: five complete runs of these tables, judged by ruling r52 step 7; fix-P2/performance_the-rebalanced-shard-tables-have-no-measured-medians
---

# The rebalanced shard tables have no measured medians

Found by the frontier review's round three over #411, and filed deferred under ruling r52, by the owner's
exception for this finding alone to the rule that a finding carrying a reproduction is fixed (`MAINTAINING.md`,
step 5).

## Failure sequence

The review's, as ruling r52 reads it:

    the acceptance criterion is each leg's median job seconds over several runs, at about 10 minutes
      (the owner, 2026-11-23 23:12Z)
    -> round three (`43f5f8f6`) re-packed the tables by round two's five-run medians, so round three's tables
      have one complete run, 45347920342 on `857a912b`
    -> pooling that run with round two's five gives medians of 665 and 677.5 s for Windows legs 12 and 13 and
      693 s for macOS leg 5, but those pooled runs mostly time tables this head no longer has; on its own run,
      legs 12 and 13 ran 567 and 557 s
    -> so no median of these tables exists yet, and macOS legs 3 and 5 ran 748 and 701 s on their one run

## What is established

Ruling r52's findings, read and not run:

- The review's arithmetic is right and applies the body's own sentence, which pooled round two's five runs
  with round three's one. No run could have met it: with round two's samples fixed, legs 12 and 13 pool to at
  least 665 and 677.5 s (computed). `43f5f8f6` changed those legs' filters, so the witness mostly times tables
  the head lacks.
- What stands: the head's tables have one run, so no median, and macOS legs 3 and 5 ran 748 and 701 s. The
  workflow says as much: `.github/workflows/ci.yml:94` and `:351` both read "median job time is about ten
  minutes or less", followed by "The rebalance was not timed before it landed: the CI run of the pull request
  that landed it is its first measurement."
- Round three's run, job seconds: Windows legs 1-14: 402, 543, 567, 586, 593, 592, 614, 589, 418, 575, 395,
  567, 557, 600; macOS 1-5: 557, 612, 748, 631, 701; Linux 362, 500. Predicted medians: Windows 555-586 s,
  macOS 601-614 s.
- P2, performance; no `MUST`. Its only repair is more runs, which the merge supplies; hence the owner's
  exception.
- Inferred from start times: legs no longer set a run's length. Run 45347920342 took 15.2 minutes end to end,
  and 45342808145, on `master`'s ten Windows legs, 15.5; seven of the first's jobs started one to six minutes
  late, each as another finished (27 jobs behind 20 concurrent slots).

## What the change that takes this up should do

Ruling r52 step 7, written out:

- **Measure the first five complete runs of these tables** (45347920342; the findings-only push's run; the
  merge queue's; the `master` push's; the next) in the operator's `shard-measurements.md`: each leg's
  seconds, the run's end-to-end minutes, its late starts.
- Then, per runner: every median at or under 630 s: delete this file in the next findings pull request.
- A leg over 630 s and more than 45 s above the mean of its runner's leg medians: one re-pack pull request on
  the guard's branch, `fix-P2/performance_the-rebalanced-shard-tables-have-no-measured-medians`, merged on the
  owner's word for that diff; a leg still over after it goes to the owner.
- A leg over 630 s but balanced (within 45 s of its runner's mean): no re-pack; the owner is asked.
