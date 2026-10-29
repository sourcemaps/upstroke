---
id: G6-ST08-EMPTY-PER-KEY-PROJECTION
severity: P2
disposition: deferred     # open in this commit only: implemented by 30cabe08, 340bd2a0 and dd1fb697 below it; the next commit closes it; independent acceptance awaits this pull request's fresh reviews
category: correctness
pr:                       # assigned when the pull request is created
reviewed_sha: e0932045b8eb6b7194c677dc7da598b7e7057130
location: src/engine/topology/coordinator.rs:2766
provenance: undetermined
first_bad:                # undetermined
guard: selecting_attempt_level_events_by_a_kind_field_keeps_nothing_and_is_rejected and a_keys_attempt_level_projection_depends_on_its_own_trace_alone, with the nonvacuity check in the per-key callers
---

## Failure sequence

At `e0932045`, `attempt_level` (`src/engine/topology/coordinator.rs:2766-2782`) serializes each
`TopologyEventBody` through `canonical`, groups the values by key, and keeps only those whose
top-level `kind` names one of the five `ATTEMPT_LEVEL` events (2758-2764). `TopologyEventBody`
is `#[serde(tag = "event")]` (`src/topology/events.rs:1243-1245`), so no serialized value has a
top-level `kind`. Every key's attempt-level projection is empty, its digest is the SHA-256 of empty
input, and per-key equality across seeds holds vacuously.

Gate 6's export at `e0932045` shows it: in both export runs `projection/independent` carries 16
seeds × 3 per-key digests, and all 48 are the SHA-256 of empty input. The callers' final-tree,
dispatch and order assertions are separate checks and were not affected. No product defect was
observed.

## What the change that takes this up should do

Select the five attempt-level kinds by the event's own tag, group by task key in log order, and
apply the unchanged `canonical()` to each complete per-key trace, so that each key's commit labels
are numbered within its own trace. That is ST-08's per-key scope under the adopted spec's
canonical-trace projection. The global and chain canonicalization stay unchanged.

Require each expected key to carry its successful-attempt events, and reject an empty selection.
`candidate_prepared` is the sole successful settlement (`design/26_design_merge_queue_protocol.md`
lines 158-162), so `attempt_finished` is selected when present but not required.

This is implemented on this branch by `30cabe08`, `340bd2a0` and `dd1fb697`, which change only
`coordinator.rs`'s tests and `docs/internals/engine/topology/coordinator.md`. Independent acceptance
awaits this pull request's fresh reviews.
