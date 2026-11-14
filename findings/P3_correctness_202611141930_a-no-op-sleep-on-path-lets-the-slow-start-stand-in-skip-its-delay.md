---
id: PR366-SLOW-START-NOOP-SLEEP
severity: P3
disposition: deferred
category: correctness
pr: 366
reviewed_sha: 8c7dba47b1b6ddc992e6edbe0aea8ed065777f71
location: src/runner/container/exec/tests.rs:6528
provenance: introduced_by_feature
first_bad:
guard: the change that holds the start with a `ContainerHooks` double at `ContainerSite::Start`, `HookPhase::Before`, and deletes the stand-in `docker`
---

# A successful sleep shim defeats the slow-start mutation guard

## Failure sequence

From the review:

> PATH resolves docker normally but resolves sleep to a successful no-op -> the stand-in writes
> delayed 5 without delaying start -> with the deadline mutated back to 3 seconds the container
> starts immediately and times out normally -> the regression remains green and does not guard
> the repair

The stand-in `docker` sleeps through whatever `sleep` the inherited `PATH` resolves. It then
writes its mark. The test reads the mark as proof that the start was held. If that `sleep`
succeeds without waiting, the mark is still written but nothing was held. The test passes, and it
would still pass if the repair it guards were taken out.

## How it was found

By reading the code, in a Codex frontier review of pull request #366. It was not reproduced.

## Why it is deferred

The owner ruled on 2026-11-14 that #366's repair rounds end after round ten, and this was found
after that. It is also unwitnessed. Nobody has run the test with a no-op `sleep` on `PATH`.

## What the change that takes this up should do

Hold the start from inside the process instead of through `PATH`. Use a `ContainerHooks` double
that holds at `ContainerSite::Start`, `HookPhase::Before` and counts its holds. `CancelAfter`,
near `src/runner/container/exec/tests.rs:7117-7138`, shows the pattern. Delete the stand-in
`docker`, `slow_docker_start_child` and the `SLOW_START_*` variables. The test should assert two
things: the output is a timeout, and the double held exactly once.

Witnesses. Run each one red, then restore it:

- Set the deadline constant back to three seconds.
- Remove the hold.
