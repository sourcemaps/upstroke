---
id: PR366-PROC-PROBE-SAMPLING-GAP
severity: P3
disposition: deferred
category: correctness
pr: 366
reviewed_sha: 2f36c0fb1a8d0be9f48afa8935b98b9db84376a6
location: src/runner/container/exec/tests.rs:6353
provenance: introduced_by_feature
first_bad:
guard: the change that decides the probe's visibility by a read that cannot fall between two polls, instead of sampling `/proc` while the invocation runs
---

# The process-visibility probe can miss a locally visible late start

## Failure sequence

From the review:

> a local Docker start completes with less than the sampler's fifty-millisecond interval left
> before the invocation deadline -> the sleep starts and is removed between two polls ->
> run_blocking returns an accepted timed-out output while visible remains false -> the
> containment test falsely skips, or fails when UPSTROKE_REQUIRE_DOCKER is set

The probe runs `sleep <marker>` in a container. A sampler thread checks `/proc` for the marker
every fifty milliseconds until the invocation returns. A timed-out output counts as a good probe.
If the container starts late, its `sleep` can start and be removed between two polls. Then the
probe reports a timeout and `visible` is still false. The test reads that as "this host cannot see
container processes", even though it can. It skips, or it fails when `UPSTROKE_REQUIRE_DOCKER` is
set.

## How it was found

By reading the code, in a Codex frontier review of pull request #366. It was not reproduced.

## Why it is deferred

The owner ruled on 2026-11-14 that #366's repair rounds end after round ten, and this was found
after that. It is also unwitnessed. Nobody has seen a late start fall between two polls.

## What the change that takes this up should do

Decide visibility by a read that cannot fall between two polls, instead of sampling while the
invocation runs. Two ways to do it:

- Keep the probe container alive until the probe has looked. Remove it only after the `/proc`
  check has run.
- Read the container's host pid from the daemon, then check `/proc/<pid>` for it.
