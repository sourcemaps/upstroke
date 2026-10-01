---
id: PR11-OVERSHOOT-BOUND-IN-NO-OUTPUT
severity: P3
disposition: deferred
category: docs-contract
pr: 327
reviewed_sha: 1fe988cd140dab27731206db4812ab2a927618b7
location: docs/internals/engine/topology/coordinator.md:52
provenance: introduced_by_feature
first_bad: 1e0cd0fe9af7f440f0abcb5005694ba20e33114d
guard: PR12, the slice that lets a production run go above max_parallel = 1 — before a budget-stopped run at width above one reaches a user
---

## Failure sequence

    a run at max_parallel = 3 under a ceiling has three pipelines live, each running one invocation
    -> one settles and its spend crosses the ceiling; the next selection appends budget_exceeded
    -> the drain lets the two others run to their natural settlements, as the closure requires: each
       finishes the worker it was running and then starts its gates, its review passes and their
       re-asks — paid invocations started after budget_exceeded (with two reviewers per task, the two
       drained pipelines start four review passes after the stop)
    -> the run ends BudgetExceeded having spent past its ceiling by the remaining spend of every
       pipeline admitted before the stop, which no count of invocations bounds
    -> no output says the run overshot, or what the exposure is: the report carries no width and no
       such field, and the exposure — the unknown remaining spend of at most max_parallel admitted
       pipelines, their later review passes and re-asks included — is stated in the coordinator's
       notes alone

The packet's risk register lists exactly this risk — "spend overshoot under parallel in-flight work",
mitigation "ledger states bound; max_parallel = 1 for narrow stop", evidence needed "ledger output"
(`risks`) — and PR11 delivers the bound to the notes only. The PR11 record's `R-AB` planned "the report
text" from phase 4, and `R-AH` and §11 record why it was not done: the report is
`deny_unknown_fields`, so a field is a report-schema change PR11 does not make.

**Latent, and P3.** No production run goes above width 1 in this build (the PR11 record, `R-G`), and at
width 1 there is no in-flight spend to overshoot by.

## What the change that takes this up should do

State the exposure where a reader of a budget-stopped run finds it — the report (a schema change,
with its version and validator) or the status line — whenever the run's recorded `max_parallel` is
above one: the remaining spend of the pipelines admitted before the stop, their later review passes
and re-asks included, which is unknown when the stop is recorded and is not a count of invocations;
and keep "max_parallel = 1 for a narrow stop" in the user documentation beside it.

**Corrected in PR11's round R5** (the full review's `FULL-SC-4`, executed by its scope lens at
`a9e88039`): this file first said the drained pipelines overshoot "by up to two invocations' spend"
and stated the bound as "one invocation each". The witness the scope lens ran started four review
passes after `budget_exceeded` at width three with two reviewers per task;
`coordinator::tests::a_budget_stop_drains_live_pipelines_to_their_settlements_and_ends_budget_exceeded`
asserts the same shape with one reviewer — each drained pipeline starts its review pass after the
stop.
