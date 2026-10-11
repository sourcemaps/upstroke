---
id: PR328-REAPER-DOCKER-WAIT-HAS-NO-DEADLINE-AFTER-SIGKILL
severity: P3
disposition: deferred
category: liveness
pr: 328
reviewed_sha: 793c3784d3a0bcf39f653b0464c056e4d7d61e5e
location: src/agent/proc.rs:4990
provenance: pre_existing
first_bad: 919a728e8fc20e7cf2043329a1d454c1b2638332
guard: the change that bounds the reaper's fork-side `docker` waits — `reap_bounded`'s wait after its `SIGKILL` — with a witness of a `docker` child that `SIGKILL` does not make collectable within the bound
---

## Failure sequence

On Unix, a coordinator dies while its containers run:

    its container reaper notices the death and runs `docker ps --all --quiet --no-trunc --filter …`
    -> the CLI hangs while keeping its stdout open
    -> `read_bounded` (src/agent/proc.rs:4934) reads the listing for up to `REAPER_DOCKER_TICKS` polls of
       10 ms, about 30 seconds, and returns what it has
    -> `list_labeled_containers` (:4861) calls `reap_bounded` (:4891, :4975), which polls `waitpid(…, WNOHANG)`
       for about 30 seconds more and then sends `SIGKILL` (:4987)
    -> the CLI does not become collectable — it is in uninterruptible sleep with the signal pending
    -> `waitpid(pid, NULL, 0)` (:4990) blocks with no deadline, made again for as long as it is interrupted
    -> the reaper never reaches its `kill` and `rm` calls; the dead coordinator's containers run on until the
       next write command's startup census

`spawn_docker` (:4895) ends each `kill` and `rm` call the same way. Every line above is at `92c4ca81`, and the
reviewed head `793c3784` changed no source file.

**Found by** the design review of PR #328 (PR11 follow-up A), regression lens, round 1, at `793c3784`
(`~/orch-pr11/reviews/review-328-d1-regression-793c3784.review.md`, `FUA-D1-REG-1`): the design claimed each
reaper call was bounded at 30 seconds, and the lens showed the deadline-less wait. The design's risk account is
corrected (the follow-up's record, §1.8); this file is the limitation itself, which that change did not bound.

**What it costs.** The orphan window `PR11-REAPER-CONTAINER-SCOPE-UNREGISTERED` closed reopens, for this case,
until the next write command's census — the window the packet documents for Windows. The wedged reaper lingers
until the CLI ends. A container runner's reaper holds no cleanup lease, so no successor is refused by it. A
host reaper also runs these calls when a container scope is registered process-wide
(`set_container_reclaim_scope`), and holds its leases until it exits — but that registration has only test
callers. Reaching the state needs a `docker` CLI that `SIGKILL` cannot end promptly: a hung filesystem or
device under it.

**Why it was not bounded in #328.** The loop is the fork-side code every reaper's container half shares, and a
bound needs a witness of a process `SIGKILL` cannot make collectable, which no test of this tree constructs yet;
the reviewer judged the bound work of its own, and the follow-up stayed within its scope.

## What the change that takes this up should do

Bound the wait after `SIGKILL` in `reap_bounded`: poll `waitpid(…, WNOHANG)` within a budget, as
`wait_for_an_ended_helper` does for the module's helpers, and leave a child still there for the reaper's own exit
to hand to init, so the reaper always reaches its next call. Witness it with a `docker` stand-in the wait never
collects within the budget — a seccomp policy answering the wait *not yet*, as `agent::proc`'s helper-ending
tests do, or an `LD_PRELOAD` fault — and show the reaper's `kill` and `rm` calls still made.
