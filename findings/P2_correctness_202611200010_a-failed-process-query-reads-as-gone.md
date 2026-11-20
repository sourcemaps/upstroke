---
id: PR391-WINDOWS-GONE-CHECK-FAILS-OPEN
severity: P2
disposition: deferred
category: correctness
pr: 391
reviewed_sha: 4751a88e3d819738e95492994be46a1cd4f7bc4a
location: src/engine/topology/coverage/tests.rs:1430
provenance: introduced_by_feature
first_bad: 55da6d207ed0d1535504a0be232286b3ef717b27
guard: the change that gives agent::proc a three-outcome process probe and moves the Windows gone-checks onto it
---

## Failure sequence

Found by reading the code, not by running it: this box cannot run the Windows path.

1. `process_alive` answers `false` when `OpenProcess` or `GetProcessTimes` fails, and for every
   wait result but `WAIT_TIMEOUT` (`src/agent/proc.rs:1223-1234`). A `false` therefore means
   either "this process is gone" or "this process could not be looked at", and the caller cannot
   tell which.
2. The kill-at-every-point test reads the grandchild that the killed child recorded, then waits
   for it to go. The loop at `src/engine/topology/coverage/tests.rs:1430` ends on the first
   `false`. So a grandchild whose query fails (an access denial on `OpenProcess`, a failed
   `GetProcessTimes`, or a `WAIT_FAILED`) is read as gone, and the test passes while it may still
   be running. The check fails open, and it fails open in the case where a survivor is most likely
   to go unseen.
3. The same `false` is read as gone by other callers on `master`:
   - `src/engine/topology/recover/tests.rs:17535`: the wait for the worker spawned by a dead
     coordinator;
   - `src/runner/host/tests.rs:4757-4765`: `wait_until_gone`, which returns `true` on the first
     `false`;
   - `src/agent/proc/tests.rs:366`: `terminate_fault_helper_gone`, which negates
     `ambient::process_alive`;
   - `src/agent/proc/tests.rs:1907`: `still_running_after`, which reports "not still running" on
     the first `false`.

What still catches a survivor:

- **The Windows suite step.** After the suite ends it lists every `upstroke*` process that is
  still running and fails if there is one. It also fails when it could not make that listing and
  the suite had otherwise passed (`.github/workflows/ci.yml:492-495` for the full suite,
  `:682-685` for the sharded legs). A grandchild that the gone-check missed is still caught there,
  but only at the end of the whole run, with no link to the test that left it.
- **The order pin `the_spawn_kill_child_is_contained_before_it_spawns`.** It requires
  `spawn_kill_child` to contain itself before it spawns, which is what keeps a grandchild from
  outliving the kill in the first place. The gone-check is a second, independent witness of that
  same property, and this finding is about that second witness.

## What the change that takes this up should do

- **Add a probe with three outcomes** to `src/agent/proc.rs`'s `windows_job`: alive, gone, or
  unobserved. `reviews/2026-10-02-pr11-follow-up-c-record.md` §2.5.2 (lines 983–1007) sets it out.
  When `OpenProcess` fails, `ERROR_INVALID_PARAMETER` is gone and any other error is unobserved. A
  failed `GetProcessTimes` is unobserved. A different creation time is gone. `WAIT_OBJECT_0` is
  gone, `WAIT_TIMEOUT` is alive, and any other wait result is unobserved. The record's `proc.rs`
  line numbers are about six lines behind the current file: `process_alive` is now at
  `:1223-1234`.
- **An unobserved result fails the test.** Every gone-check listed under the failure sequence
  moves to the probe: `coverage/tests.rs:1430`, `recover/tests.rs:17535`,
  `runner/host/tests.rs:4757-4765`, and `agent/proc/tests.rs:366,1907`.
- **Why the probe cannot be written in `coverage/tests.rs`.** `src/engine/topology.rs:4-8` sets
  `forbid(clippy::disallowed_methods)` across the whole topology tree, and `clippy.toml` (near
  line 141) denies
  `windows_sys::Win32::System::Threading::OpenProcess`. A probe in that tree would not compile, and
  a `forbid` cannot be allowed back locally. The probe has to live where `process_alive` already
  lives, and the tests call it from there.
- **Not done here.** The probe is unsafe Windows code that this box cannot run, and adding it
  needs a row in `effects/wrappers.toml` (beside the `process_alive` rows) that the owner should
  read and approve. Both are reasons this finding is deferred rather than fixed in PR 391.
