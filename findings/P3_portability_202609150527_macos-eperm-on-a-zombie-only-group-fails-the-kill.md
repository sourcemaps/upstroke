---
id: PR292-MACOS-ZOMBIE-ONLY-GROUP-EPERM-FAILS-KILL-TREE
severity: P3
disposition: deferred
category: portability
pr: 292
reviewed_sha: 30d80454aeca3a319cce7000015e6bf17a68625d
location: src/agent/proc.rs:584
provenance: pre_existing
first_bad: 1f338f3c3f1e4f8dd46f6392cfdc9d6e11c47aa3
guard: the change that next opens `kill_tree_primitive`'s Unix group signal; `signal_group_kill` should settle an `EPERM` by observing the group before it reports the group not gone, and `a_spawn_fault_whose_cleanup_termination_faults_after_its_primitive_reports_the_child_gone` should run on macOS again
---

## Failure sequence

**On macOS, `kill_tree` can report a process group it has just killed as not established gone.
When the group holds only zombies, the kernel answers the group signal with `EPERM`, and
`signal_group_kill` treats only `ESRCH` as gone.**

Lines are at `30d80454`.

1. The process funnel's spawn after phase answers an error (`run_with_timeout_and_limit`,
   `src/agent/proc.rs:245-254`). On Unix, that error path drops the termination `Supervisor`
   (`:251`) before it calls `kill_tree` (`:252`).
2. `register` has run (`:1690`), so the `Supervisor` is in `Phase::Group`, and its drop (`:1784`)
   calls `finish` (`:1707`). `finish` hands the group to the cleanup reaper. `cleanup_reaper_group`
   (`:2685`) sends `SIGKILL` to the group and loops until `group_has_non_zombie_members` answers
   `Some(false)` (`:2694`). The leader is this process's own child and nothing has waited for it,
   so the group now holds one zombie and nothing else.
3. `kill_tree` calls `kill_tree_primitive`, which calls `signal_group_kill` (`:575`), which calls
   `kill(-pgid, SIGKILL)`. On macOS this call answered `EPERM`. Only `ESRCH` is treated as gone
   (`:584`), so the error is kept.
4. The primitive still kills and reaps the direct child (`:557-558`), so the group is gone. The
   primitive nevertheless returns "terminating the agent process group did not establish it gone:
   the group signal failed (Operation not permitted (os error 1))".
5. `kill_tree` returns before its fate store (`:504`). The funnel's `ProcessFailure` reports
   `ProcessFate::Unresolved` for a child that is gone.

**Observed** on #292 at `30d80454`: CI run `34930901306`, job `104258732193` (`test (macos-latest)`),
in `agent::proc::tests::a_spawn_fault_whose_cleanup_termination_faults_after_its_primitive_reports_the_child_gone`.
In that job 2641 tests passed and this one failed, with:

> the spawn's error is returned with the cleanup termination's after-phase error beside it: the
> process funnel was made to fail at `Spawn` (after); additional cleanup failure: agent error:
> terminating the agent process group did not establish it gone: the group signal failed
> (Operation not permitted (os error 1))

The same run's `test (ubuntu-latest)` and `test (winguest)` jobs succeeded, and both compile and run
this test. On Linux the same signal lets the primitive complete. Windows has no `Supervisor` and
terminates through the job.

`EPERM` is what the kernel was observed to answer. This record did not read the XNU source to find
out why. `docs/internals/agent/proc.md` (under `leads_own_group`) already records the same zombie
state: XNU hides an exited, unreaped child from `proc_find`.

**This is not `PR281-MACOS-KILL-TREE-SETTLE-ONLY-THE-DIRECT-CHILD`.** That fingerprint is a live
grandchild still holding the pipes after `kill_tree` returns. This one is a group of zombies that is
refused a signal.

## Reach

Production does not reach this today.

- **`:252`** runs only when the spawn's after phase answers `Error`. `NoHooks`, which `HostRunner`
  holds by default (`src/runner/host.rs:114`), never answers it: `SpawnHooks::phase` answers
  `Proceed` by default (`src/agent/proc/hooks.rs:20-23`), and `NoHooks` does not override it
  (`:31-35`). A `HarnessHooks` answers `Error` only where a `HookHarness` has been armed to.
- **`:237`** runs only when `Supervisor::register` fails, and `register` fails only for a pid that
  does not fit an `i32` (`:1691-1693`). Even then, the `Supervisor` is still spawning, so its drop
  cancels the reaper and does not kill the group. The group would be zombie-only only if the child
  had already exited on its own.

**First bad.** `1f338f3c` added the spawn after phase's error path that drops the `Supervisor`
before `kill_tree`. `signal_group_kill` has treated only `ESRCH` as gone since it was introduced at
`287563f0`.

## Why it is P3

No production path reaches it. The funnel returns an error either way: the spawn's own error
leads, the termination's is beside it, and the child is reaped.

What is wrong is the fate and the second message. `Unresolved` says the funnel could not establish
the child gone, for a group that is gone. #292 paid for it with a red macOS leg, one extra push, and
a witness that no longer runs on macOS.

## What the change that takes this up should do

1. **Settle `EPERM` narrowly.** On macOS, `signal_group_kill` should treat `EPERM` as settled only
   when the group holds no non-zombie member, which is the observation `group_has_non_zombie_members`
   already makes for the reaper. Every other `EPERM` stays an error, because a host policy can
   refuse a signal to a live process (`REFUSED-KILL-UNBOUNDS-THE-BOUNDED-DOCKER-REAP` records an LSM
   or seccomp policy answering `SIGKILL` with `EPERM`).
2. **Run the witness on macOS again.** Remove `#[cfg(not(target_os = "macos"))]` from
   `a_spawn_fault_whose_cleanup_termination_faults_after_its_primitive_reports_the_child_gone` and
   its hooks struct in `src/agent/proc/tests.rs`, and remove the "Not on macOS" paragraph from its
   notes.
