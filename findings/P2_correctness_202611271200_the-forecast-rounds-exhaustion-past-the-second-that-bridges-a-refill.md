---
id: PR441-FORECAST-MISSES-PRE-EXHAUSTION-SECOND
severity: P2
disposition: deferred
category: correctness
pr: 441
reviewed_sha: dc7af793fb624aa58d69b29ea3890a083a05b943
location: src/capacity_planner.rs:1161
provenance: fix_regression
first_bad: e1968e60, the repair whose parent is b8572735
guard: the change that first calls `plan`, which repairs this before it does, with the reviewer's reproduction as its test
---

# Rounding exhaustion upward omits the second that bridges a weekly refill

Filed deferred from #441's review of the reviewed commit. The defect is confined to the offline forecast, `plan`, and nothing in the crate calls it yet (`docs/internals/capacity_planner.md`, *Module*), so it fails nowhere today. The sequence starts at the first change that calls `plan`.

## Failure sequence

    supply a nine-hour job, a four-hour full weekly allowance, exhaustion at +3600.5 seconds,
      a fixed refill at +5 hours and grant expiry at +2 hours
    -> `Scenario::choose` rounds each baseline event up to a whole second (line 1161),
       so the candidate list scores +3601 and omits +3600
    -> the recommended time leaves half a second of work waiting for the next weekly refill

## Reproduction

The reviewer's, at the reviewed commit:

- Public `plan` returned +3601.
- The reviewed simulator finished at +622800.5 seconds with that choice, and at +32400 seconds with redemption at +3600.
- The same public input at `b8572735`, the repair's parent, returned +3600.

The repair in this history whose parent is `b8572735` is `e1968e60`, so this row is `fix_regression`.

## What the change that takes this up should do

Repair this before it first calls `plan`, with the reviewer's reproduction as its test. On the input above, the test fails at the reviewed commit, where `plan` recommends +3601, and passes once the recommended time completes the job at +32400 seconds, as `b8572735`'s +3600 does.
