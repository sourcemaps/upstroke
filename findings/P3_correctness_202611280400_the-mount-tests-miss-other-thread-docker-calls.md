---
id: PR444-MOUNT-TESTS-MISS-OTHER-THREAD-DOCKER-CALLS
severity: P3
disposition: deferred
category: correctness
pr: 444
reviewed_sha: 8ef7350e3721860b8eed592f04eebdaf0e4ced85
location: src/engine/topology/entry.rs:1646
provenance: fix_regression
first_bad: fea566da
guard: the test-only pull request, on its own claim branch, that makes both mount tests fail again when any thread of a child's process asks for an inspection before the child exits, as at b7482c2c
---

# The mount tests miss another thread's Docker inspection

## Failure sequence

The two mount tests in the engine's tests,
`a_mount_inside_an_inherited_common_directory_is_refused_by_run_and_resume_before_the_runtime_is_asked`
and
`a_recorded_mount_whose_target_a_fresh_run_refuses_is_refused_by_the_resume_before_the_runtime_is_asked`,
plant a stub `docker` first on their children's search path and assert afterwards that it was never
asked.

    a child arms the test-mode seam engine::topology::entry::container_runtime, and arming is
    thread-local
    -> another thread of the child's process asks for an inspection
    -> that thread meets the unarmed absent-runtime double, which asks nothing
    -> the stub never sees the inspection, and the parent's assertion that it was never asked
       passes

At `b7482c2c` every unarmed inspection constructed the real CLI, so the parents' check covered every
thread of the child's process until it exited.

Both reproductions are the reviewers':

- The regression lens's, on `8ef7350e`: a joined helper thread calling `for_inspection().probe()`
  just before the resume validates the recorded mount failed both tests at `b7482c2c`, where the
  stub recorded `version --format {{.Server.Version}}`, and passed both at the head, `8ef7350e`.
- The frontier's, on `a557a725`: a thread-exit callback registered before the resume's mount check,
  which joined a thread calling `for_inspection().probe()`, failed both tests at `b7482c2c` on the
  stub's version invocation and passed both at `a557a725`.

## What the change that takes this up should do

Round one of #444's repairs tried this, and ruling r102 withdrew it (commit `e0ddc9a2`, reviewed on
`a557a725`): the double counted its answers process-wide, and each child read the count once after
its engine calls. A read inside the child ends before the child does, so an inspection at teardown,
after the read, is seen by neither the count nor the stub, which the frontier's thread-exit
reproduction shows. With it withdrawn the finding is open again, filed under the owner's
reproduced-findings exception of 2026-11-27T08:41:42Z.

A proposal, not yet reviewed: an arming that every thread of the child's process sees, so that the
parents' stub observes all of them, as at `b7482c2c`.

Read from the code, not run: the parents' stub shares the three temporary-directory conditions found
in round one's stub (`PR444-UNARMED-TEST-MISSES-DOCKER-INVOCATIONS`):

- an apostrophe in the path, since its record path is interpolated into shell text;
- a `noexec` temporary directory;
- a colon in the path, which is joined into the search path.

The change keeps every assertion of both tests and the double's answers.
