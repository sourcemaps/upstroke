---
id: CI-FLAKE-HANDOFF-CHILDREN-LIST-RACE
severity: P3
disposition: deferred
category: correctness
pr: TBD
reviewed_sha: 7d96f365761e1187805bf3dba2f80043f8fa487f
location: src/runner/container/exec/tests.rs:9390
provenance: undetermined
first_bad:
guard: the change that makes `children_of` retry when a task's list vanishes between the listing and the read
---

## Failure sequence

Seen once: CI run 42918059287, job 130185502003, the Ubuntu test job, on pull request #360 on
2026-11-10, a change that touched only documentation. The library summary was 3736 passed,
1 failed, 203 ignored. The comparison is a re-run of the same commit; this finding does not claim
that the failed execution passed.

The test is
`an_engine_admitted_between_a_backstops_listing_and_its_ending_owns_and_converges_the_cleanup`,
with its subprocess `handoff_harness_child`, both in `src/runner/container/exec/tests.rs`.

1. **`children_of` lists, then reads.** The helper (line 9390) lists the tasks of a process from
   its task directory under the proc filesystem, `/proc/<pid>/task`, and then reads each task's
   children list, `/proc/<pid>/task/<tid>/children`.
2. **A thread of the coordinator can exit between the listing and the read.** Its list is then
   gone, and the read fails with not-found.
3. **`children_of` returns the error.** Its own comment says an unreadable list is an error and
   never an answer of no children.
4. **The caller's `expect` (line 10774, in `handoff_harness_child`) panics** with the message that
   the coordinator's children lists could not be read: no such file or directory.

**It is a defect of the test helper, not of the engine.** No product code reads these lists.

## What the change that takes this up should do

- **When a task's list has vanished, list the tasks again and read again,** a bounded number of
  times, and return the error only if the lists never read consistently.
- **A vanished task must not be read as a task with no children.** That is the rule the helper's
  own comment states.
