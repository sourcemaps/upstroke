---
id: BOX-FLAKE-LEASE-KILL-DOCKER-STUB-TEXT-FILE-BUSY
severity: P3
disposition: deferred
category: correctness
pr: 416
reviewed_sha: 39747d82566955152f72fa30f301ff9b59518bcf
location: src/runner/container/exec/tests.rs:11074
provenance: pre_existing
first_bad:
guard: the change that next edits `the_lease_kill_guard`'s stub (`src/runner/container/exec/tests.rs:11072-11075`), or the change that takes up the write-then-exec class recorded as `W2-HOST-TESTS-WRITE-THEN-EXEC-ETXTBSY`
---

# A lease-kill guard's docker stub could not be started: Text file busy on the build box

## Failure sequence

    `the_lease_kill_guard("credential-volume")` writes its `docker` stub with `std::fs::write`, sets its mode to 0o755, and starts the bounded harness (`src/runner/container/exec/tests.rs:11072-11075`)
    -> the coordinator's runner spawns `docker volume inspect upstroke-creds-claude --format {{.Name}}`
    -> the spawn fails with `Text file busy (os error 26)`
    -> the runner refuses the create, because the runtime "could not be asked whether the credential volume ... exists", and the paused helper never pauses
    -> the coordinator ends on signal 15 before its helper paused
    -> `lease_kill_harness_child` panics at `:10909`
    -> the harness exits 101 (wait status 25856), and the test panics at `:11099` with `credential-volume: the bounded harness ended: status Some(ExitStatus(unix_wait_status(25856)))`

`a_killed_coordinators_paused_credential_volume_inspection_keeps_run_admission_refused_until_it_is_settled`
(`:11154`) runs `the_lease_kill_guard("credential-volume")`, which writes the stub and runs
`lease_kill_harness_child` in a bounded child; that harness starts a coordinator with the stub's
directory first on its `PATH` and waits for the paused helper and the backstops to report their
pids, panicking at `:10909` if the coordinator ends first (`:10880-10912`). After the coordinator
is killed while the stub holds `docker volume inspect` paused, the harness reports that admission
was refused while the paused helper's reaper still owed its settlement, that it was admitted only
after every covered helper, reaper and backstop had ended, and that every process the coordinator
left ended on its own and was collected, and the guard asserts that the harness exited
successfully with `1 passed` and printed each of those lines (`:11040-11110`).

## The sighting

| where | date | head | command |
|---|---|---|---|
| the build machine's pre-push baseline (32 cores, load about 4.8) | 2026-11-24, about 13:34Z to 13:41Z | `39747d82` (#416's round-one repair) | `cargo test --all-targets --all-features` through the build machine's wrapper |

Read from the build machine's own baseline log,
`/home/ubuntu/selfhost-samplerfix-20261124/evidence/baseline-39747d8-container-flake.log`, SHA-256
`330c3280407c95486f0ddfddbf2d863fe3993e851fd9b88407cc3228b1995310`:

- Line 2978:
  `test runner::container::exec::tests::a_killed_coordinators_paused_credential_volume_inspection_keeps_run_admission_refused_until_it_is_settled ... FAILED`.
- Line 4850: the panic at `src/runner/container/exec/tests.rs:11099:5`,
  `credential-volume: the bounded harness ended: status Some(ExitStatus(unix_wait_status(25856)))`.
- The harness's stderr: `lease_kill_harness_child` panicked at
  `src/runner/container/exec/tests.rs:10909:13` with
  `credential-volume: the coordinator ended (ExitStatus(unix_wait_status(15))) before its helper paused`.
  The coordinator's stderr carries
  `upstroke: fail-closed SIGTERM armed: cleanup reaper did not acknowledge CANCEL`.
- Line 4876: a worker thread panicked at `src/runner/container/exec/tests.rs:10770:13`: "the
  paused helper was to hold this worker until its coordinator was killed". The error it carries is
  `fate: NeverStarted`, `Refused`, and its message contains
  `` `docker volume inspect upstroke-creds-claude --format {{.Name}}` could not be started: agent error: failed to spawn `docker`: Text file busy (os error 26) ``.
- Line 4887: `test result: FAILED. 4172 passed; 1 failed; 207 ignored`. This test was the run's
  only failure.
- **The same tree passed.** The next baseline at the same head `39747d8`, started 15:56:17Z, passed
  all nine commands (`cargo test` among them, in 185 s).
- #416's diff at `39747d82` touches only `src/workspace_manager/tests.rs` and
  `docs/internals/engine/topology/dispatch/tests.md`.

## What is and is not established

- **Established:** the spawn of the test's own `docker` stub failed with `ETXTBSY`, the runner
  refused rather than create, and the guard's harness failed because its helper never paused.
- **Not established:** which process held the stub open for writing.
- One reading, **inferred, not established**: another thread of the test process forked while the
  stub's write descriptor was open, and the forked child still held it when the runner's `docker`
  was executed.
- **Not established: the rate.** One sighting, on the build machine; none is known in CI.

## What the change that takes this up should do

Not proposed here (ruling r39 step 7); the guard names the change that takes it up.
