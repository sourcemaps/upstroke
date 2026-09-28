---
id: PR326-MACOS-A-DAEMONIZING-DESCENDANT-HANGS-UP-THE-ROLE
severity: P2
disposition: deferred
category: correctness
pr: 326
reviewed_sha: e94cef9e41b02386d15e80b92c76df859576cc1a
location: src/agent/proc.rs:2776
provenance: pre_existing
first_bad: 17a2870a891c2164b71ca4327b747d857dce8f37
guard: project owner / the change that next opens agent::proc's Unix group supervision, with a controlled macOS environment to measure it; until then the round-4 v0.1 role witnesses of PR 326 fail on macos-latest with this fingerprint
---

## Failure sequence

On macOS, a role that `agent::proc` supervises is killed by `SIGHUP` when one of its own descendants
daemonizes.

1. On Unix, each supervised role gets its own process group. Before exec, its cleanup reaper
   forks an anchor into that group (`spawn_group_anchor`, `src/agent/proc.rs:2765`). The anchor
   stops itself at `:2776` and stays stopped unless the group is continued.
2. A descendant of the role then daemonizes: it forks, its parent exits, and the child calls
   `setsid()`. Git 2.55.0 does this on every `git commit`. The commit runs
   `git maintenance run --auto --quiet --detach`, which calls `daemonize()` whether or not the
   repository needs maintenance (`builtin/gc.c` and `setup.c` at `v2.55.0`; traced with
   `GIT_TRACE2_EVENT`). Git 2.42.0 runs the same maintenance without `--detach`.
3. The role process is killed by `SIGHUP`. That is the signal the kernel's orphaned-process-group
   rule sends, followed by `SIGCONT`, to every member of a newly orphaned group that holds a stopped
   member.
4. The supervisor reaps a child killed by a signal. A gate fails with `exit code: None`. A worker
   or reviewer attempt ends without output.
5. On macOS, then, a v0.1 gate fails for a reason that is not its own if its command runs
   `git commit`, for example a test suite that builds Git fixtures. So does any role whose tools
   start a daemon. The task is retried, escalated and parked.

## Evidence

**CI run `36410362642`, job `108888858097`, `test (macos-latest)`, at `c31928a4`.** That commit
was a TEMPORARY diagnostic on top of `e94cef9e` and has since been reset off the branch. Its product
code is `e94cef9e`'s. Each worker, gate and reviewer probe ran under a shell that traps HUP, INT,
TERM, QUIT, USR1, USR2 and ALRM and records the probe's wait status. Each probe logged its group's
members at start and every step against a millisecond clock.

- 20 probes reached `git commit` in their own fixtures. 15 of them, 7 gates and 8 workers, ended
  with status 129, that is signal 1 (`SIGHUP`). Each was waiting on that `git commit` when it died.
  The other 5 completed with status 0.
- Every killed probe's shell survived, because it traps HUP, and recorded the status. The probes
  were not killed by a whole-group `SIGKILL`.
- Every group, listed at start, was the shell (the leader), the anchor (state `T`, its parent the
  reaper), the probe and `ps`. The probes that completed had the same shape.
- The `test (ubuntu-latest)` leg of the same run used the same Git 2.55.0 and the same code. All
  33 probes there ended with status 0.
- The supervisor did not do it:
  - The reason names no timeout or output limit (`src/gates.rs:163`–`181`).
  - The conductor survived every attempt, so the monitor's termination path did not run.
  - The log has no fail-closed notice, so the reaper acknowledged CLEANUP after every killed
    role. A reaper that had killed the group itself would have `_exit`ed without acknowledging
    (`settle_after_coordinator_death`), and the conductor would have armed a fail-closed
    `SIGTERM` and ended.

**Earlier sightings.** `test (macos-latest)` failed the same five witnesses at `283d7be3` (run
`36401788004`) and at `e94cef9e` (run `36408891723`), with twelve gates ending `exit code: None`
in each. At `283d7be3` those gates' stderr was empty, and the attempts they ended lasted 209 to
483 ms against a 600 s gate timeout.

## Not established

- **That the `SIGHUP` is the kernel's orphaned-group hang-up and no other sender's.** The shells
  did not record whether they received it themselves, and the `SIGCONT` was not observed.
- **Why XNU treats the group as orphaned.** The leader's parent, the conductor, is in another
  process group of the same session the whole time. POSIX does not call such a group orphaned, and
  Linux does not treat it as one.

## What the change that takes this up should do

1. **Establish the mechanism on a controlled macOS machine.** Use the trap-shell diagnostic, with
   the shell recording its own HUP. Vary one thing at a time: `-c maintenance.autoDetach=false`,
   an anchor that pauses instead of stopping, and no anchor at all.
2. **Remove the precondition the product controls, rather than asking roles to survive `SIGHUP`.**
   That precondition is a member of every role's group that stays stopped for the role's life. The
   anchor exists to keep the group id from being reused while the reaper can still signal it. A
   member that is not stopped could do that job, once the reaper's readiness wait and its mirrored
   stop are adapted to it.
3. **Add a macOS regression.** It should run a role one of whose children forks, exits and calls
   `setsid()`, and assert that the role completes.
