---
id: PR441-FORECAST-PRIOR-PLAN-INVENTORY
severity: P2
disposition: deferred
category: correctness
pr: 441
reviewed_sha: dc7af793fb624aa58d69b29ea3890a083a05b943
location: src/capacity_planner.rs:849
provenance: introduced_by_feature
first_bad:
guard: the change that first calls `plan`, which repairs this before it does, with the reviewer's reproduction as its test
---

# A grant observed before a known plan change is recommended on the new plan

Filed deferred from #441's review of the reviewed commit. The defect is confined to the offline forecast, `plan`, and nothing in the crate calls it yet (`docs/internals/capacity_planner.md`, *Module*), so it fails nowhere today. The sequence starts at the first change that calls `plan`.

## Failure sequence

    observe one usable grant on plan A
    -> switch to plan B and supply two current B window readings showing weekly exhaustion
    -> keep A's inventory within the age limit, without a B inventory read
    -> `assess_grants` checks the inventory's age (lines 836 to 845) and decides each grant (line 849)
    -> `plan` returns `Redeem`, using eligibility and a count established only on A

## Reproduction

The reviewer's, at the reviewed commit:

- Public `plan`, with the inventory and an A reading at minus 900 seconds and B readings at minus 300 seconds and now, returned `Redeem`.
- A current zero-count inventory with the same B windows returned `LetExpire(NotRedeemable)`.

## What the change that takes this up should do

Repair this before it first calls `plan`, with the reviewer's reproduction as its test. An inventory observed before the latest known plan change must not give `Redeem` on the new plan: the test fails at the reviewed commit and passes once the input above no longer does.
