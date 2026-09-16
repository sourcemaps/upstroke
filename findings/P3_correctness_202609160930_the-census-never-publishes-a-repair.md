---
id: G5-CLAUSE1-CENSUS-NO-REPAIR-IS-EVER-PUBLISHED
severity: P3
disposition: deferred
category: correctness
pr: 
reviewed_sha: 5ac786f36c4e6c90b5000816557572cf270ba5f9
location: src/topology/census.rs:968
provenance: pre_existing
first_bad: 
guard: the change that builds the census generator's publications from the fold — `satisfies` from `satisfies_closure`, the lease release from the candidate's lineage — and re-measures every member of the family
---

## Failure sequence

`merge_prepared_for`, the builder every `merge_prepared/*` offer of `classes` and
`integration_path_classes` goes through, records `satisfies: vec![key]` -> for a merge repair the
fold derives the publication's closure as the lineage root and the repair together
(`satisfies_closure` in `src/topology/fold/check_integration.rs`) -> `check_merge_prepared`
refuses every publication of a repair as `InvalidSatisfies`; measured 2026-09-16 at `5ac786f3`
on the seeded fan-out census as "`merge_prepared` settles [3], and the fold derives [2, 3] as
this publication's closure" and on the 20,000-state prefix as "settles [3], and the fold derives
[0, 3]" -> no member of the census family ever accepts a repair's `merge_prepared` or
`task_merged` (the prefix's accepted labels for r3 and r4 hold dispatches, attempts, candidates,
rejections and verification starts, and no publication), so the census never executes
`apply_task_merged` with a multi-key `satisfies` or a `MergeLeaseRelease::Lineage` release ->
the notes for the seeded census said "the repairs integrated" until 2026-09-16, a coverage the
census does not have.

The `plan_transition` arm clause is unaffected: `task_merged` is executed by the originals'
publications. The fold's own tests cover the lineage release ("the lineage lease is held until
the publication that satisfies its root", `src/topology/fold/tests.rs`).

## What the change that takes this up should do

Build the publication offers from the fold: `satisfies` from `fold.satisfies_closure(key)`, and
for a candidate whose task has a lineage, `task_merged`'s `lease_release` as
`MergeLeaseRelease::Lineage { root }`. Expect every member's explored set to change — the seeded
censuses will integrate their repairs and reach `Complete` through them, and the chain may then
reach a second lineage — so re-measure the family artifact and re-derive the notes' figures. It
is a generator change that alters the evidence Gate 5 reads, and belongs in its own pull request
with its own review.
