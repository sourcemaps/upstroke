---
id: PR369-CREATOR-CLEANUP-FAILURE-MISREPORTED
severity: P3
disposition: deferred
category: correctness
pr: 369
reviewed_sha: 3dc3b8fceedb2ea639d7cc9015bafe6ad5ffe71c
location: src/engine/topology/create.rs:1885
provenance: pre_existing
first_bad:
guard: the change that gives the creator's cleanup a `Disposition` for a failed public removal, on both arms of `stat_after_error`, and tests that failure path
---

# The creator's cleanup reports a removal that failed

## Failure sequence

Found by round four's frontier pass over pull request #369, by reading the code. It has not
been reproduced.

The creator's cleanup, `stat_after_error` in `src/engine/topology/create.rs`, removes the
public half of a stopped creation by the shape its ownership proof answered. So:

1. A creation stops. The cleanup's proof answers `NothingBound(Bare)`.
2. Before the removal, the public directory gains a file.
3. `remove_unbound_husk` (`src/rundir.rs`) removes a `Bare` husk with one `remove_dir`, which
   refuses the directory that is no longer empty. It returns an error whose source is
   `DirectoryNotEmpty`, and nothing is removed.
4. `stat_after_error` discards that error (`let _ = …` at line 1885) and returns
   `Disposition::PublicHalfRemoved(Bare)` anyway.
5. The operator is told "the public run directory was reclaimed (bare)" (`describe`), and
   `removed_anything()` answers `true`. Neither reclamation happened: the directory and the
   file are still there.

### Why it is the base's

At `71eac07c`, both arms already discard the public removal's result and report a removal
unconditionally: the proven arm at line 1854 (`BothHalvesRemoved`) and the unbound arm at line
1873 (`PublicHalfRemoved`). At the reviewed SHA they are lines 1854 and 1885. This pull request's
`363defe6` changed only which removal the unbound arm calls, from `remove_public_husk` to
`remove_unbound_husk`.

### Inferred

- Nothing is lost. The leftover husk is one the next census classifies again, as the comment
  over line 1885 says. What is wrong is the report.
- The proven arm has the same defect: a failed `remove_public_husk` after a successful private
  removal is still reported as `BothHalvesRemoved`.

## What the change that takes this up should do

- Add a `Disposition` for a public removal that failed, carrying the error's detail, and
  return it from both arms of `stat_after_error` instead of discarding the error.
- Make `describe` and `removed_anything()` answer for it truthfully.
- Test that failure path on both arms.
