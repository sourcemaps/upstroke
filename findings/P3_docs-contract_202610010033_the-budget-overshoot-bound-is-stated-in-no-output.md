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
    -> the drain lets the two others run to their natural settlements, as the closure requires
    -> the run ends BudgetExceeded having spent past its ceiling by up to two invocations' spend
    -> no output says the overshoot is bounded, or by what: the report carries no width and no such
       field, and the bound — the unknown spend of at most max_parallel live pipelines, one invocation
       each — is stated in the coordinator's notes alone

The packet's risk register lists exactly this risk — "spend overshoot under parallel in-flight work",
mitigation "ledger states bound; max_parallel = 1 for narrow stop", evidence needed "ledger output"
(`risks`) — and PR11 delivers the bound to the notes only. The PR11 record's `R-AB` planned "the report
text" from phase 4, and `R-AH` and §11 record why it was not done: the report is
`deny_unknown_fields`, so a field is a report-schema change PR11 does not make.

**Latent, and P3.** No production run goes above width 1 in this build (the PR11 record, `R-G`), and at
width 1 there is no in-flight spend to overshoot by.

## What the change that takes this up should do

State the bound where a reader of a budget-stopped run finds it — the report (a schema change, with
its version and validator) or the status line — whenever the run's recorded `max_parallel` is above
one, and keep "max_parallel = 1 for a narrow stop" in the user documentation beside it.
