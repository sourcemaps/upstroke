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
This describes the two failures of 9 November recorded below; on 2026-11-11 either case was seen
to end with `code: None`, the linked-checkout case in six runs and the main-checkout case in three,
as the last section records.

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

## Quarantined on macOS, 2026-11-11

The owner decided on 2026-11-11 to quarantine the test on macOS until this finding is repaired.

This replaces, for macOS only and until the repair, the instruction above not to skip the test.
Everywhere else, and on macOS once the finding is repaired, that instruction stands as written. The
assertion is not weakened, and nothing retries it.

The reason for the decision: on 2026-11-11 the test failed on macOS in nine of the fifty-eight
completed `test (macos-latest)` jobs that ran it, 9 of 58, between fifteen and sixteen per cent.

How that was counted. The operator measured it from the GitHub API and the failed jobs' logs at
00:55 UTC on 2026-11-12, after every job of every run created on 2026-11-11 UTC had ended. The
population is every run of the `CI` workflow created on 2026-11-11 UTC: 86 runs, on every event
(79 pull-request, 4 merge-queue, 3 push). Each has one `test (macos-latest)` job. Of the 86 jobs,
50 succeeded, 12 failed, 24 were cancelled (superseded by a later push or edit) and none was still
running. Of the 62 completed jobs, 4 ran with this test already ignored on macOS (three on the
branch of pull request #370 and one on the branch that added this section), so 58 completed jobs
ran the test, and it failed in 9 of them. The other three failed jobs failed on other tests only:
`agent::proc::tests::unix_reaper_kills_labeled_containers` in run 43028506871; a test added by pull
request #340 in run 43112671690; and
`engine::topology::recover::tests::sampled_cherry_pick_child_kills_every_residue_classified_and_recovered`
in run 43118198459. An earlier version of this section gave a count cut off at about 23:45 UTC on
2026-11-11 (84 runs, 9 of 56) that missed two runs created at 23:39 UTC; this count replaces it.

In all nine the panic is the test's final assertion, reported at `src/runner/host/tests.rs:8600:5`,
with the message `1 launcher(s) changed the user's commit:` followed by one case's label and
`the commit failed: ProcessOutput { code: None, ...`, with the hook's own test reported passing in
the captured output. In each of the nine exactly one of the two cases failed: the linked checkout
(`a plain commit in a linked checkout`) or the main checkout
(`` `git commit -a` in the main checkout ``).

These are the nine, as the CI run and job of `test (macos-latest)`, the head, the event, and the
case that failed:

- run 42976030457, job 130375210731, head `e11840e4`, pull request: linked checkout. The same job
  also failed `agent::proc::tests::terminal_input_and_output_stops_cover_the_isolated_tree`;
- run 42988928017, job 130416735154, head `8a392819`, merge queue: linked checkout;
- run 43004727185, job 130469246954, head `cca5582c`, pull request: linked checkout;
- run 43033695241, job 130566916120, head `757850f6`, push to `master`: main checkout;
- run 43058994038, job 130656096459, head `27ce7533`, pull request: main checkout;
- run 43059735704, job 130658756924, head `5df9364c`, pull request: linked checkout;
- run 43065890345, job 130680680012, head `5bb6d4c2`, pull request: linked checkout;
- run 43094224878, job 130778802196, head `7201f8b7`, pull request: linked checkout;
- run 43094291655, job 130779022448, head `0478e8ba`, pull request: main checkout.

So six failed in the linked-checkout case and three in the main-checkout case. None of the nine
heads changes this test, the runner or the process code it exercises; `757850f6` is `master`
itself.

While the quarantine stands, the macOS job does not witness this failure. Its absence from CI is
therefore not evidence that the defect is gone.

Lifting the quarantine is part of what the repair must do, with the native macOS validation this
finding already requires.
