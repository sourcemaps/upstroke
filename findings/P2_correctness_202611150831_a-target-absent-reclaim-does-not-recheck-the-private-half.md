---
id: PR369-TARGET-ABSENT-AUTHORITY-STALE
severity: P2
disposition: deferred
category: correctness
pr: 369
reviewed_sha: 3dc3b8fceedb2ea639d7cc9015bafe6ad5ffe71c
location: src/rundir.rs:1766
provenance: pre_existing
first_bad:
guard: the change that decides in `design/` what a target-absent reclaim re-observes immediately beside the removal, and states the window that remains
---

# A target-absent reclaim does not recheck the private half

## Failure sequence

Found by round four's frontier pass over pull request #369, by reading the code. It has not
been reproduced.

The startup census (`census_run_dirs` in `src/engine/topology/startup.rs`) scans every public
directory and plans its fate before it applies any plan. Under `UnboundShape::TargetAbsent`,
`remove_unbound_husk` in `src/rundir.rs` (line 1766) lists and empties the directory and
removes the marker last, without looking at the marker's private target again. So:

1. A public husk has a valid marker, but its private target is absent for a while because
   something moved it aside.
2. The census scans the husk. The ownership proof (`src/rundir/ownership.rs`) gets `NotFound`
   for the target and plans `NothingBound(TargetAbsent)`.
3. The private half is put back before `apply` runs.
4. The removal does not look at the target again. It removes the public marker along with the
   rest of the husk.
5. That marker was the only thing that located the restored private half, so the private half
   is now orphaned. No census or status command can reach it.

### Why it is the base's

At `71eac07c`, `apply` called `remove_public_husk` for every shape, so it removed a
target-absent husk the same way. The census already scanned every directory before applying
any plan. This pull request leaves the proof's target-absent step unchanged.

### Inferred

- Only something outside the engine moves a private half aside, so this needs an outside
  actor.
- Rechecking the target just before the removal would narrow the window but not close it. The
  private half could still come back between the recheck and the marker's removal.

## What the change that takes this up should do

- Decide in `design/` what a target-absent reclaim re-observes immediately beside the removal,
  for instance whether the marker's private target is still `NotFound`.
- State the window that remains between that observation and the marker's removal.
