---
id: PR441-FORECAST-UNKNOWN-EFFECT-KEEPS-OLD-CLOCK
severity: P2
disposition: deferred
category: correctness
pr: 441
reviewed_sha: dc7af793fb624aa58d69b29ea3890a083a05b943
location: src/capacity_planner.rs:1123
provenance: introduced_by_feature
first_bad:
guard: the change that first calls `plan`, which repairs this before it does, with the reviewer's reproduction as its test
---

# Neither effect bound considers an unknown restoration that displaces an imminent refill

Filed deferred from #441's review of the reviewed commit. The defect is confined to the offline forecast, `plan`, and nothing in the crate calls it yet (`docs/internals/capacity_planner.md`, *Module*), so it fails nowhere today. The sequence starts at the first change that calls `plan`.

## Failure sequence

    queue 2.5 hours of Fable work with weekly exhaustion until +0.5 hours
      and a half-used model allowance refilling at +1 hour
    -> the weekly grant's effect on that model window is unknown
    -> `resolve` compares no restoration with full restoration; under full restoration `restore`
       keeps the window's +1-hour clock for an unknown effect (line 1123), so both comparisons
       retain it and recommend `Redeem`
    -> if the grant also restores the model window and its reported replacement refill is +5 hours,
       redeeming delays completion

## Reproduction

The reviewer's, at the reviewed commit:

- Public `plan` returned `Redeem`.
- The reviewed simulator finished at 3 hours without redemption, and at 2.5 hours under both implemented effect bounds.
- Applying the model restoration and supplying the compatible +5-hour refresh produced completion at 5.5 hours.

## What the change that takes this up should do

Repair this before it first calls `plan`, with the reviewer's reproduction as its test. The input above must not give `Redeem` while a compatible outcome of the redemption, the restoration with its +5-hour refresh, completes later than not redeeming: the test fails at the reviewed commit and passes once that outcome is within what the decision considers.
