---
id: PR441-FORECAST-UNREPORTED-NATURAL-REFILL
severity: P2
disposition: deferred
category: correctness
pr: 441
reviewed_sha: dc7af793fb624aa58d69b29ea3890a083a05b943
location: src/capacity_planner.rs:1098
provenance: introduced_by_feature
first_bad:
guard: the change that first calls `plan`, which repairs this before it does, with the reviewer's reproduction as its test
---

# An unknown second natural refill is treated as proof that a grant unlocks work

Filed deferred from #441's review of the reviewed commit. The defect is confined to the offline forecast, `plan`, and nothing in the crate calls it yet (`docs/internals/capacity_planner.md`, *Module*), so it fails nowhere today. The sequence starts at the first change that calls `plan`.

## Failure sequence

    supply an eight-hour Codex job, a covered session burning 20% per hour and its next reported refill at +1 hour
    -> after that refill, the no-redemption simulation removes the session's clock (line 1098)
       and stalls at hour 6
    -> `plan` recommends the grant at hour 6
    -> even though a compatible natural refresh then completes the job in the earliest possible eight hours

## Reproduction

The reviewer's, at the reviewed commit:

- Public `plan` returned `Defer` until +21600 seconds, and its baseline marked the job incomplete.
- In the in-memory comparison, the reviewer supplied a five-hour recurrence after the same first refresh. The job finished at +28800 seconds without a grant.

## What the change that takes this up should do

Repair this before it first calls `plan`, with the reviewer's reproduction as its test. The input above must not give `Defer` until +21600 seconds on the strength of a stall that an unreported natural refresh may remove: the test fails at the reviewed commit and passes once a refill the readings do not report is not read as no refill.
