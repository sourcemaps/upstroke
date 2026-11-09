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
