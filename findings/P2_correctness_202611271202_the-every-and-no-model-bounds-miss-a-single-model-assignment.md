---
id: PR441-FORECAST-SINGLE-MODEL-ASSIGNMENT
severity: P2
disposition: deferred
category: correctness
pr: 441
reviewed_sha: dc7af793fb624aa58d69b29ea3890a083a05b943
location: src/capacity_planner.rs:957
provenance: introduced_by_feature
first_bad:
guard: the change that first calls `plan`, which repairs this before it does, with the reviewer's reproduction as its test
---

# Agreement between the every-model and no-model cases misses a feasible Fable assignment

Filed deferred from #441's review of the reviewed commit. The defect is confined to the offline forecast, `plan`, and nothing in the crate calls it yet (`docs/internals/capacity_planner.md`, *Module*), so it fails nowhere today. The sequence starts at the first change that calls `plan`.

## Failure sequence

    supply exhausted Fable and Opus windows refilling in two and eight hours,
      a Fable grant expiring in one hour whose coverage excludes Opus, and one hour of unlabeled work
    -> `decide` resolves the unlabeled work twice: drawing on every model window, it still waits for Opus;
       drawing on none, it runs immediately
    -> both comparisons let the grant expire, and line 957 takes their agreement as the decision
    -> although assigning the work to Fable alone makes immediate redemption useful

## Reproduction

The reviewer's: public `plan`, with only the demand's label varied.

- Unlabeled, Opus and Sonnet returned `LetExpire(NoUnlock)`.
- Fable returned `Redeem`.

The same outputs reproduced at `be60d55e` and at the reviewed commit, so this row is `introduced_by_feature`.

## What the change that takes this up should do

Repair this before it first calls `plan`, with the reviewer's reproduction as its test. Unlabeled, the input above must not give `LetExpire(NoUnlock)` while the same input labeled Fable gives `Redeem`: the test fails at the reviewed commit and passes once a single-model assignment of unlabeled work can no longer be passed over by the two bounds agreeing.
