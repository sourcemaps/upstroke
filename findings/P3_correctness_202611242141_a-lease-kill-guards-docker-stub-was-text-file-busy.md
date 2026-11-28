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

## A second sighting, 2026-11-28

| where | date | head | command |
|---|---|---|---|
| the build machine's pre-push baseline (Linux x86_64, 32 cores; load 1.36 when the try started) | 2026-11-28, the try starting at 08:49:19Z | `af0b64b7`, on the branch that became #446, before #446 was opened | `upstroke-build cargo test --all-targets --all-features`, step 03 of the build machine's baseline, with `UPSTROKE_REQUIRE_DOCKER=1` |

The try's start and its failed step are the baseline's own record
(`~/selfhost-docker-question-20261128/evidence/baseline-push1/baseline-af0b64b.log`, lines 2 and 5),
and its load the lane's record beside it (`push1-baseline.txt`, line 1).

Read from the build machine's own baseline log,
`~/selfhost-docker-question-20261128/evidence/baseline-push1/03-test.log`, SHA-256
`e887b3543b50c328cceae07043e2525a0a081909f3d02ab3d31e141b3de7125d` (the build machine's wrapper kept
a copy with the same SHA-256 at `~/eight-logs/af0b64b-failed-20261128T085437Z/03-test.log`):

- Line 3042:
  `test runner::container::exec::tests::a_killed_coordinators_paused_attached_start_keeps_run_admission_refused_until_it_is_settled ... FAILED`.
- Lines 4932-4933: the panic at `src/runner/container/exec/tests.rs:11099:5`,
  `attached-start: the bounded harness ended: status Some(ExitStatus(unix_wait_status(25856)))`.
- Lines 4937 and 4944, printed in that test's output, are the bounded harness's own lines:
  `test runner::container::exec::tests::lease_kill_harness_child ... FAILED` and
  `test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 4410 filtered out; finished in 2.03s`.
- Lines 4949-4950: `lease_kill_harness_child` panicked at
  `src/runner/container/exec/tests.rs:10909:13` with
  `attached-start: the coordinator ended (ExitStatus(unix_wait_status(15))) before its helper paused`.
  Line 4955 is the coordinator's
  `upstroke: fail-closed SIGTERM armed: cleanup reaper did not acknowledge CANCEL`.
- Lines 4957-4958: a worker thread panicked at `src/runner/container/exec/tests.rs:10770:13`: "the
  paused helper was to hold this worker until its coordinator was killed". The error it carries is
  `fate: NeverStarted`, `Refused`, and its message reads "the container runtime cannot be reached
  for `create`" and, of the `docker create` command,
  `` could not be started: agent error: failed to spawn `docker`: Text file busy (os error 26) ``.
- Line 4969:
  `test result: FAILED. 4203 passed; 1 failed; 207 ignored; 0 measured; 0 filtered out; finished in 193.73s`.
  Line 4971: `` error: test failed, to rerun pass `--lib` ``. Line 56 is the log's only `Running`
  line: it shows Cargo starting the library's test target (`unittests src/lib.rs`) and no other.

**What the test asserts**, by line at `af0b64b7`, where `src/runner/container/exec/tests.rs` is the
blob `master` has (`9bd5ba3e`, at `6513c22d` and at `d431d220`). It is a different test from this
file's first sighting, with a different case label: the test at `:11142` runs
`the_lease_kill_guard("attached-start")` (`:11143`), where the first sighting's, at `:11154`, runs
the `credential-volume` case (`:11156`). The guard, its `docker` stub (`:11072-11075`), its
assertion (`:11099-11102`) and the error, `Text file busy (os error 26)`, are the same. The command
the stub could not be started for was `docker create`; in the first sighting it was
`docker volume inspect`.

**What the head changed.** `git diff --name-only 6513c22d af0b64b7` names six paths: two notes, two
finding files, `src/runner/container.rs` and `src/runner/container/tests.rs`. None is under
`src/runner/container/exec/` or `src/agent/`, and `src/runner/container/exec.rs` is unchanged.

**The same head passed.** The next baseline at `af0b64b7`, its try starting at 09:07:39Z, passed all
nine commands. Its log,
`~/selfhost-docker-question-20261128/evidence/baseline-push1-requeue/03-test.log`, SHA-256
`47342db965185d12dad2d98cbdb4a04ed71eac7fea9f91a01a4c9d8fc72ae768`: line 3223 logs this test
`... ok`, and line 4877 reads
`test result: ok. 4204 passed; 0 failed; 207 ignored; 0 measured; 0 filtered out; finished in 202.31s`.

**And alone.** Five runs of this test by its exact name at `af0b64b7`, through the kit's
`cargo-iso.sh`, from 09:04:39Z to 09:06:18Z, each passed: each log reads `running 1 test`, the test
`... ok`, and
`test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 4410 filtered out; finished in 9.90s`
(`~/selfhost-docker-question-20261128/evidence/baseline-push1/isolated/`, `run-1.log` to
`run-5.log`, their SHA-256 values in `SHA256SUMS` one directory up and their times in `summary.txt`;
the command at `~/selfhost-docker-question-20261128/evidence/baseline-push1/match.md:132`).

**What is and is not established for this sighting.**

- **Established:** in the `attached-start` case too, the spawn of the guard's own `docker` stub
  failed with `Text file busy (os error 26)`, the runner refused rather than create, and the guard's
  harness failed because its helper never paused; the next baseline at the same head passed the
  test.
- **Not established:** which process held the stub open for writing.
- **Not established: the rate.** This file records two sightings, both on the build machine, and
  none in CI.
- `PR172-REAPER-ROUNDS-STUB-NEVER-RAN`
  (`findings/P3_correctness_202609052115_reaper-rounds-stub-never-ran.md:16-22`) records a different
  test, `the_reaper_performs_as_many_rounds_as_the_machine_needs`, whose own `docker` stub's `execv`
  was refused with `ETXTBSY`, in three sightings on tactusbox, this build machine, on 2026-09-05.

## What the change that takes this up should do

Not proposed here (ruling r39 step 7); the guard names the change that takes it up.
