---
id: G7-CI-MACOS-HOOK-COMMIT-FLAKE
severity: P2
disposition: deferred
category: correctness
pr: 358
reviewed_sha: 7bade79dba5dc3fb0933976e8a3bc993cf46cd60
location: src/runner/host/tests.rs:8600
provenance: pre_existing
first_bad:
guard: diagnose and stabilize a_hook_started_runs_slot_worker_leaves_the_users_commit_as_the_user_made_it on macOS while preserving its commit-content assertions and recording the outer Git process termination
---

## Failure sequence

The macOS suite runs
`runner::host::tests::a_hook_started_runs_slot_worker_leaves_the_users_commit_as_the_user_made_it`.
Its main-checkout case invokes `git commit -a`, whose pre-commit hook runs the isolated slot-worker
helper. The helper passes, but the surrounding Git command returns `ProcessOutput { code: None,
timed_out: false, output_limited: false, ... }`. The test refuses that result and the required CI
job fails. The linked-checkout case completes successfully.

The owner requested this be recorded as a flaky-test issue on 9 November 2026 and authorized one
fresh pipeline. That is the operational classification of this intermittent failure, not a
diagnosis of its cause or a claim that the failed execution passed. The current finding-only PR
does not change this test, its runner, the process machinery, or the workflow.

Two preserved macOS failures show the same main-checkout shape:

- [PR358 CI run 42680362168, attempt 1, job 129372919792](https://github.com/sourcemaps/upstroke/actions/runs/42680362168/job/129372919792),
  at reviewed head `7bade79dba5dc3fb0933976e8a3bc993cf46cd60`: the outer Git duration was
  636.639625 ms; the hook reported one passing test in 0.40 s. The library summary was
  3642 passed, 1 failed, 167 ignored. Main and probe suites were not reached.
- [Earlier PR357 CI run 42555647540, attempt 1, job 128960615865](https://github.com/sourcemaps/upstroke/actions/runs/42555647540/job/128960615865):
  the same test failed with `code: None`, no timeout or output limit, a 444.872209 ms outer
  duration, and a passing hook in 0.30 s. The relevant source is byte-identical. Later PR357
  validation and its post-merge CI passed; the earlier failure remains recorded.

The current complete Git Trace2 record shows the hook child exiting zero, followed by index and
cache-tree events. It contains no outer Git `exit`, `atexit`, or `signal` event. The linked-case
trace ends with exit zero. The signal, sender, and cause of the main-case termination are not
established. In particular, the panic's wording about a changed user commit does not establish
content corruption: this failure is at the command-exit check, before the committed-path and
content assertions.

## What the change that takes this up should do

Diagnose the outer Git process termination on macOS, preserving the original failed logs and
correlating process identity, wait status or signal, hook completion, and any cleanup actors.
Determine whether the defect is in the fixture, process supervision, or product behavior before
choosing a repair. Keep both main and linked checkout cases and their original path/content
assertions. A future fix needs a focused regression guard for the established mechanism and
native macOS validation.

Do not skip, weaken, or automatically retry the assertion inside the test. A successful fresh CI
run permits the ordinary merge checks to proceed but does not resolve this finding. No product
repair is included here, and the original failures are not regraded.

## Sightings and evidence, 2026-11-11

The test failed in the macOS test job three times on 2026-11-11. Each failure was on a change that
touches nothing under `src/runner` or `src/agent`:

- job 130375210731, run 42976030457, pull request #365;
- job 130416735154, run 42988928017, the first merge-queue run of #365;
- job 130469246954, pull request #367.

In the three jobs whose Git Trace2 dumps were read, the commit in the linked checkout ends 7 to
10 milliseconds after its hook exits with code 0:

| Hook `child_exit` | Last event of the commit |
| --- | --- |
| 07:25:31.607236 | 07:25:31.614849 |
| 09:12:09.781390 | 09:12:09.788499 |
| 11:04:49.605867 | 11:04:49.615496 |

None of the three traces has an `exit`, `atexit` or `signal` event. Every inner Git command the hook
started exits 0. In the same runs the commit in the main checkout reaches `exit` code 0.

By its code, the supervising funnel does not signal the group in this case: it kills the group only
after the child is already reaped, or on a timeout, an output limit or a stop, and the failing
output shows none of those.

**Hypothesis, not established:** the commit's process group is hung up by the kernel under macOS's
rule for an orphaned process group that holds a stopped member. Every supervised child's group
holds a member that stops itself (`spawn_group_anchor` in `src/agent/proc.rs` raises `SIGSTOP`),
and the hook's helper leaves short-lived children in the commit's group when it exits. This is the
mechanism another open finding records,
`findings/P2_correctness_202609281056_a-daemonizing-descendant-hangs-up-the-role-on-macos.md`. If
it is right, the defect is in the supervising code and this test is a witness of it.

What would settle it is the number of the signal that ends the commit. The hypothesis predicts 1,
`SIGHUP`. A change made beside this one makes the test's failure message carry that number.

Nothing is repaired by this section.
