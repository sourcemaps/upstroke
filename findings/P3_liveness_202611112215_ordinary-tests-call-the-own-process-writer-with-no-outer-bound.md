---
id: PR368-WRITER-HELPER-UNBOUNDED-CALLERS
severity: P3
disposition: deferred
category: liveness
pr: 368
reviewed_sha: 7201f8b71a1c44615b3efedcf8d6e5b8986f5f05
location: src/runner/container.rs:2370
provenance: pre_existing
first_bad:
guard: the change that gives each of the nine call sites an outer bound, by moving the write into an isolated child under a deadline as #368 did for the gate-probe test, or bounds the helper's end itself, with a witness that a stopped reaper ends one test and not the suite
---

# Ordinary tests call the own-process writer helper with no outer bound

## Failure sequence

`runner::container::write_program_in_its_own_process` (`src/runner/container.rs:2370`) runs its writer
through the process funnel under a one-minute timeout, but the funnel's end waits for its reaper's
acknowledged exit without a bound, by design (`FUA-I4-RELAY`). The helper's own notes say what follows: a
call from a test's own process can hold the whole suite for as long as that reaper is stopped.

1. An ordinary test calls the helper from the shared test process.
2. The helper's writer finishes.
3. The funnel's end waits for its reaper's acknowledged exit.
4. That reaper is stopped, or stalls, before it exits.
5. The wait has no bound, so the test never returns, and the shared test process, with every other test in
   it, is held until something outside ends it.

The helper has twelve call sites. A call is *bounded* when it is made inside an isolated child that a parent
test started with `run_test_isolated` under a deadline, and *unbounded* when an ordinary test makes it in the
shared test process. Nine are unbounded:

- `src/runner/container/tests.rs:1196`, in the ordinary test
  `an_attached_start_whose_client_ended_early_asks_its_diagnostic_within_its_own_bounds`, which writes its
  Docker stub in its own process before it starts its isolated child.
- `src/runner/container/tests.rs:9272` and `:9283`, both in the helper `capability_control` (line 9262): the
  first when it is given a fake, reached by ten ordinary tests (for example
  `the_capability_observer_rejects_an_ssh_endpoint_and_keeps_its_parts_out_of_the_record`), the second when it
  is given none, reached by `the_capability_observer_spawns_nothing_without_a_docker_executable`.
- `src/runner/container/exec/tests.rs:8876`, in the ordinary test
  `a_running_container_whose_observation_stalls_settles_at_its_deadline_or_cancellation_and_keeps_its_fate`,
  before its isolated children.
- `src/engine/tests.rs:13048`, in the helper `git_shim` (line 13008), reached by eleven ordinary tests (for
  example `a_run_that_dies_between_its_pins_commit_and_its_ref_loses_nothing_on_resume`).
- `src/engine/tests.rs:21390`, in the helper `a_fake_cmd` (line 21387), reached by
  `a_zero_gate_schema_4_resume_refuses_its_unavailable_recorded_shell_whatever_todays_config_names`.
- `src/engine/tests.rs:21819`, in the helper `delivery_program` (line 21814), which writes only for the
  never-exiting delivery fault, reached by
  `the_signal_tests_helper_is_settled_when_the_command_delivering_its_signal_never_exits`.
- `src/engine/tests.rs:27575` and `:27706`, in the ordinary tests
  `a_mount_inside_an_inherited_common_directory_is_refused_by_run_and_resume_before_the_runtime_is_asked` and
  `a_recorded_mount_whose_target_a_fresh_run_refuses_is_refused_by_the_resume_before_the_runtime_is_asked`,
  which write their Docker stubs in their own process.

The other three are the pattern to follow: `src/runner/host/tests.rs:4979` in `reaper_relay_writer_helper`,
`src/runner/container/fake.rs:916` in `reaper_relay_install_child`, and `src/engine/tests.rs:11043` in
`write_the_gate_probe_stand_in`, each made inside an isolated child its parent test starts through
`run_test_isolated` under a deadline (180, 120 and 300 seconds).

No such hold has been observed at these nine sites; this is a recorded exposure, not a sighting. In pull
request #368's first round of review, the regression lens reproduced the hold through this same helper from
an ordinary test thread: with the helper's reaper held at its exit, the call waited past sixty-five seconds.
The same mechanism was rated P2 in `FUA-I4-RELAY` and in that round. It is filed here at P3 because no hold
has been seen in CI at any of the nine sites and the reproduction needed a reaper stopped by hand; that is a
judgement the owner may revise.

**Found by** the frontier review of pull request #368 at `7201f8b7`. #368 bounded one caller, the gate-probe
test's `write_the_gate_probe_stand_in`, by moving its write into an isolated child under a deadline, and left
these nine as they were.

## What the change that takes this up should do

- Give each of the nine call sites an outer bound, by moving its write into an isolated child started with
  `run_test_isolated` under a deadline, as #368 did for the gate-probe test, or bound the helper's end itself.
- Keep the helper's reason for writing from another process, the text-file-busy window: a bound must not put
  the write back in the shared test process.
- Witness it: a stopped reaper ends one test's isolated child at its deadline, and the suite goes on.
