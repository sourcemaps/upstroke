---
id: PR287-R2-STAND-IN-DRIVERS-REQUIRE-A-REAPING-ANCESTOR
severity: P2
disposition: deferred
category: correctness
pr: 287
reviewed_sha: e787d440ff822940ba8641b8de45b77040995873
location: src/agent/proc.rs:8121
provenance: fix_regression
first_bad: c75e47ef
guard: a change to `run_a_stand_in_fixture` that collects the stand-in a fixture leaves behind, or reads an exited, uncollected stand-in as gone, with the subreaper sequence below exiting 101 before it and 0 after
---

## Failure sequence

**The bounded endings work. The tests that check them make a demand on whatever runs the suite.**
Round 2's invariance lens on `e787d440` found this and executed it
(`review-287-regression-204114.log` in the orchestrator's directory: *"`101` after 32.042 seconds"*
and *"`101` after 32.046 seconds"*). It was executed again for this filing, at the same head, and it
reproduces.

Two drivers at `e787d440` leave a live stand-in behind on purpose, then require it to be gone:
`an_ending_returns_within_its_budget_when_the_helper_will_not_die` (`:8142`) and
`a_cleanup_the_reaper_did_not_acknowledge_returns_within_its_budget` (`:8464`). Each runs its
fixture through `run_a_stand_in_fixture` (`:8164`).

1. The fixture forks the stand-in (`spawn_sigchld_target`, `:5098`). The stand-in blocks in `read`
   on a pipe whose write end the fixture holds until it exits (`:8008`, `:8438`).
2. The fixture drives the ending, which returns within its budget, and exits.
3. The stand-in reads end-of-file and calls `_exit(0)` (`:5139`).
4. `run_a_stand_in_fixture` then calls `assert_the_process_is_gone` (`:8184`). That function asks
   `kill(pid, 0)` every 10 ms and panics at `:8130` unless it answers `ESRCH` within 30 s.

The function's comment (`:8117`–`:8119`) states the assumption: the stand-in ends *"and `init`
collects it; until then `kill(pid, 0)` answers 0 for a live process and for a zombie alike."* But
the ancestor that adopts an orphan is what collects it. That is the nearest ancestor that set
`PR_SET_CHILD_SUBREAPER`, and `init` only when there is none. So the assertion holds only where
that ancestor collects adopted children within 30 s.

Sequence: a supervisor sets `PR_SET_CHILD_SUBREAPER` and waits for its test-process child by pid ->
the fixture exits and its stand-in is reparented to the supervisor -> the stand-in exits with status
`0` and stays a zombie, because the supervisor collects only its own child -> `kill(pid, 0)`
answers `0` for 30 s -> the driver panics at `src/agent/proc.rs:8130` and exits `101`. The ending
under test had returned within its budget, and the stand-in had exited.

Executed 2026-09-14, 21:36–21:40Z. Each tree was a detached scratch worktree at that commit, built
on a target base of its own; each build log shows `Compiling upstroke v0.1.0 (<that worktree>)`.
Each run executed the lib test binary directly, by exact name, as the only child of
`orphan_probe.py`. In `subreaper` mode that supervisor first sets `PR_SET_CHILD_SUBREAPER`, then
waits for its child by pid; in `plain` mode it sets nothing. Exit codes captured directly:

| driver | tree | `plain` | `subreaper` |
|---|---|---|---|
| `an_ending_returns_within_its_budget_when_the_helper_will_not_die` | `e787d440` | `0` in 10.06 s | **`101` in 32.02 s** |
| `a_cleanup_the_reaper_did_not_acknowledge_returns_within_its_budget` | `e787d440` | `0` in 10.11 s | **`101` in 32.03 s** |
| `an_ending_returns_within_its_budget_when_the_helper_will_not_die` | `1e806e20` | `0` in 8.05 s | **`101` in 32.01 s** |

Each `subreaper` run failed at its driver's first shape, `ready-failure` or `no-answer`. At 5.0 s,
while the driver was still running, the stand-in that shape recorded showed `State: Z (zombie)`, its
`PPid` was the supervisor's pid, and `waitid(P_PID, pid, WEXITED | WNOHANG | WNOWAIT)` answered
`CLD_EXITED` with status `0`. The panic at `e787d440`:

    the ending_a_helper_that_will_not_die_helper ready-failure stand-in: pid 3245991 is still held
    after the process that left it behind exited; kill(pid, 0) answered 0 with errno 2

The supervisor collected each stand-in, with status `0`, only after the driver had exited. In
`plain` mode something else on this box collected each stand-in, so both drivers were on their
third shape at 5 s and passed. `upstroke-ci` is green at `e787d440`, so its Linux test leg
collected them too.

**Where it came from.** `assert_the_process_is_gone` and its 30-second poll arrived in `c75e47ef`,
round 1's fix, with the first driver. `ea928980` moved the call into `run_a_stand_in_fixture` and
gave that helper a second driver. At `1e806e20` the first driver fails the same way (the table's
last row), and that tree's `src/agent/proc.rs` differs from `c75e47ef` in comment lines only.
`caf6bed0` has neither driver.

The runner, the supervisor, and every log and JSON report are in
`~/findings-sweep/orch-p1/evidence-file-287-p2s-e787d440/` (`batch.sh`, `runs.sh`,
`orphan_probe.py`, `out/head/orphan/`, `out/r1/orphan/`).

## What the change that takes this up should do

Keep what the assertion is for: a stand-in still *running* after its fixture has exited fails the
driver. Stop depending on an ancestor for the rest. Either:

- **Collect the stand-in inside the suite.** Run each stand-in fixture beneath a process of the
  suite's own. That process sets `PR_SET_CHILD_SUBREAPER`, waits for the fixture, then collects every
  adopted child until `ECHILD`, reporting each pid and status. The driver then asserts that the
  recorded stand-in was collected after exiting, whatever runs the suite.
- **Read the pid's state from the kernel.** These drivers are `cfg(target_os = "linux")`. A recorded
  pid whose `/proc/<pid>/stat` state is `Z` has exited. At the deadline, pass only a pid that is gone
  or a zombie.

Execute the sequence above before the change (`101` for both drivers under the supervisor) and after
it (`0`). Then run a stand-in kept alive past its fixture's exit, its write end held by a process
that outlives the fixture. It must still fail the driver.
