---
id: PR441-FORECAST-UNREAD-NAMED-MODEL
severity: P2
disposition: deferred
category: correctness
pr: 441
reviewed_sha: dc7af793fb624aa58d69b29ea3890a083a05b943
location: src/capacity_planner.rs:770
provenance: introduced_by_feature
first_bad:
guard: the change that first calls `plan`, which repairs this before it does, with the reviewer's reproduction as its test
---

# A completely unreported model window disappears from the decision

Filed deferred from #441's review of the reviewed commit. The defect is confined to the offline forecast, `plan`, and nothing in the crate calls it yet (`docs/internals/capacity_planner.md`, *Module*), so it fails nowhere today. The sequence starts at the first change that calls `plan`.

## Failure sequence

    queue one hour of explicitly named Fable work with ample session and weekly balances and a Fable grant
    -> omit model readings, as the reader can when model windows are unreported
    -> `plan` builds window states for the session, the weekly window and each window a reading names
       (lines 769 and 770), so it never constructs a Fable window state
    -> it lets the grant expire, despite lacking evidence about the job's model limit

## Reproduction

The reviewer's, at the reviewed commit, with the same job and grant each time:

- no Fable row: public `plan` returned `LetExpire(NoUnlock)`;
- one exhausted Fable row: `Unknown(Window(Model(Fable), InsufficientHistory))`;
- two readings establishing exhaustion: `Redeem`.

No evidence about the Fable window gives a firmer answer than one reading of it does.

## What the change that takes this up should do

Repair this before it first calls `plan`, with the reviewer's reproduction as its test. With no Fable row, the input above must not give `LetExpire(NoUnlock)`: the test fails at the reviewed commit and passes once a named model with no reading is not read as a model with no limit.
