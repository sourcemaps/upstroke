# `src/engine/topology/preflight/tests.rs`

Extended notes for [`src/engine/topology/preflight/tests.rs`](../../../../../src/engine/topology/preflight/tests.rs).

The code is the authority for what it does; this file is the whole of its prose, moved out of
the source verbatim. Each section is headed by the line of code the comment sat above, spelled
as it is in the source, so the heading is the grep string that finds the code.

## Module

The `RunnerPreflight` implementation, on its own.

`recover/tests.rs` drives this through the whole recovery order; these are
the claims about the pre-flight itself — what it probes, in which order,
which of its processes take a slot pair, and that both ledgers balance on
the refusal path as well as the successful one (every refusal here is of a
process the Runner saw end; one it could not is kept, `preflight.md`).

## `struct Recording {`

---------------------------------------------------------------------------
Doubles
---------------------------------------------------------------------------

## `struct Recording {`

A runner that records every request and can fail one program.

## `struct Recording` › `refuses: bool,`

Refuse to run at all — the shape a spawn failure has, as distinct from
a process that ran and exited non-zero.

## `struct Stub(&'static str);`

An adapter that runs one probe process and reports what it printed.

## `fn a_preflight_is_the_shell_probe_then_one_probe_per_recorded_agent() {`

---------------------------------------------------------------------------
What a pre-flight is
---------------------------------------------------------------------------

## `fn a_preflight_is_the_shell_probe_then_one_probe_per_recorded_agent() {`

One non-slotted shell probe, then one slotted probe per recorded agent, in
the recorded order.

Every clause of INV-23's sentence is a separate assertion, because they fail
independently: the count, the order, the shell going first, and the slot
asymmetry. Two agents rather than one, so "in the recorded order" has
something to be wrong about.

## `fn every_probe_carries_the_identity_the_preflight_mints_for_it() {`

The shell probe carries the identity [`PreflightIdentities::shell`] mints,
and each agent probe the one [`PreflightIdentities::agent`] mints.

Asserted as an equality against those constructors rather than against a
literal: the identities are what a container name is derived from, and a
test that spelled them out by hand would agree with a wrong implementation
as easily as with a right one.

## `fn a_failing_shell_refuses_before_any_agent_is_probed() {`

A failing shell refuses **before any agent is probed**, and names the image
the probe ran inside.

The second half is not decoration: "a recorded shell or agent CLI that fails
**inside the recorded image**" is a different fact from the same CLI failing
on the host, and an operator told only the first has nothing to fix.

## `fn a_failing_agent_cli_refuses_naming_the_agent_and_releases_its_slot() {`

A failing agent CLI refuses naming the agent, and every slot it took is
released.

The slot half is the one a `Drop`-based release would get wrong and a test
asserting only the message would miss: a pre-flight that refused while still
holding the pair would make the run's next slotted invocation refuse against
its own assertion.

## `fn a_refused_spawn_cancels_its_registration_rather_than_completing_it() {`

A runner that refuses to start a process at all settles the invocation as
**cancelled**, not completed.

R3's two lifecycle rows are "released on cancel" and "released on complete
or cancel", so the two are told apart on purpose. A ledger that completed a
process which never ran would balance and be wrong.

## `fn an_agent_with_no_adapter_refuses_naming_it() {`

An agent the record names and no adapter provides refuses, naming it.

`run_started(4).probed_agents` is the allow-list every binding is drawn
from, so an agent in it that this build cannot probe is a run that cannot be
certified — not one that quietly certifies fewer agents than it recorded.

## `fn a_slotted_invocation_without_an_agent_binding_is_refused() {`

A slotted invocation with no agent binding is refused, and its registration
is cancelled rather than left running.

Reachable only through a hand-built request — [`crate::agent::probe_request`]
always sets `agent` — which is exactly why it is worth holding: the pair a
slotted invocation takes is `{agent, pool?}`, and there is no agent to name.

## `fn one_identity_cannot_be_registered_twice() {`

Two invocations sharing one identity are refused: that is aliasing, not a
duplicate completion.

## `fn a_gate_invocation_through_the_boundary_takes_no_slot() {`

A gate identity acquires no slot pair, so the pre-flight's boundary does not
take one for it either.

The rule lives in [`is_slotted`], which is a total function of the identity;
this is the assertion that the boundary reads it rather than carrying a
second opinion.

## `fn a_gate_invocation_through_the_boundary_takes_no_slot()` › `agent: None,`

A gate carries no agent binding; a boundary that slotted it would
refuse here for want of one, which is what makes this assertion
sharp rather than incidental.

## `struct Composing {`

A test runner that runs through the recording runner and reports a fixed composition of
`GIT_COMMON_DIR`. It exists only to drive the boundary's forwarding. It is not evidence about any
production runner's environment: that is `runner::host`'s test.

## `fn the_boundary_forwards_its_runners_report_and_keeps_one_per_admitted_request() {`

T-REG-REPORT, the integration correction's boundary (F). The boundary reports what the runner it
wraps composes. It keeps one report per request it admitted and handed on, and none for a request it
refused at admission, as the request also never reached the runner. The ledgers balance.

## `fn the_debug_rendering_names_the_boundary_and_not_the_ledgers() {`

The `Debug` rendering names what the pre-flight is for without printing its
ledgers.

Asserted because the impl is hand-written: a derived one would print the
`Mutex`es, and a `RunPreflight` in a refusal message would then carry every
invocation identity the run has issued.

## `const EXISTING_DIRECTORY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src");`

---------------------------------------------------------------------------
Profiled pools
---------------------------------------------------------------------------

## `const EXISTING_DIRECTORY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src");`

Profile paths that already exist (or, for `MISSING`, do not) in every
checkout of this crate. This module is a topology module, where creating a
directory is a build error, and the profile check only reads metadata, so a
fixture that made a directory would add an effect without strengthening
anything.

## `const TOKEN: &str = "sk-ant-oat01-fixture-token-value";`

A recognisable token value, so the refusal can be asserted not to carry
it.

## `fn no_token(_key: &str) -> Option<OsString> {`

The coordinator environments the token check reads, standing in for the
process environment (`RunPreflight::reading_environment`). `token_set` has
`CLAUDE_CODE_OAUTH_TOKEN` and nothing else.

## `const PROFILE_REFUSAL_CLOSING: &str = concat!(`

The sentence every profile refusal ends in, on a fresh run and on a resume
alike: nothing was created, spawned or appended, and the operator corrects the
pools file or the environment and runs the same command again. It carries the
full stop that ends the sentence before it, so `ends_with` holds the closing to
being a sentence of its own. Spelled out here rather than read from
`profile_refused`, so a change of wording is a change to this test as well. It replaces `refused`'s closing, which said the
run was resumable: on a fresh run the profile refusal comes before
`create_run`, so there is no run to resume (`PR386-FRESH-REFUSAL-NOT-RESUMABLE`).

## `fn refused_before_anything_ran(`

What every profile refusal shares, asserted once: it is a `Refused` that
opens `pre-flight: ` and ends in the one closing sentence every profile
refusal takes ([`PROFILE_REFUSAL_CLOSING`]), with nothing of `refused`'s
probe sentence and no claim that the run is resumable; nothing reached the
runner (not even the shell probe), nothing was registered or certified, and
the ledger balances. Returns the message for the test's own pins.

## `fn a_profiled_pool_on_the_container_runner_refuses_before_anything_runs() {`

Check 1: a profiled pool on the container runner is refused from the
policy's `kind`, naming the pool, before the container runner's own refusal
of the stamped probe request could be reached. The directory exists, so the
runner kind is the only thing wrong.

## `fn a_profiled_pool_whose_agent_is_not_claude_code_refuses_naming_the_pool_and_agent() {`

Check 2: a profile on a pool whose agent is not Claude Code is refused,
naming that pool and its agent, even while a valid Claude Code profile
stands beside it. Both directories exist, so the agent is the only thing
wrong.

## `fn a_profile_that_does_not_exist_refuses_naming_the_pool_and_the_path() {`

Check 4, a missing path: the refusal names the pool and the path as
configured.

## `fn a_profile_that_is_not_a_directory_refuses_naming_the_pool_and_the_path() {`

Check 4, a path that exists and is a file: refused as not a directory, so
the operator is told something other than that it is missing.

## `fn a_relative_profile_refuses_naming_the_pool_and_the_value() {`

Check 3: a relative profile is refused, whatever it would name from here.
Three values, each in a pool of its own name so that "naming its pool" has
something to be wrong about:
- `.`, which exists and is a directory from the coordinator's working
  directory, so only its being relative is wrong;
- `profiles/personal`, a plain relative path;
- `~/profiles/personal`, which is relative too, because `~` is not expanded.

Each refusal names its pool and the value as written, says that each task runs
in its own directory, so a relative path would name a different directory for
each process, says that `~` is not expanded, and asks for an absolute path.

`PR386-RELATIVE-PROFILE-CWD`. The check read the profile with `fs::metadata`
from the coordinator's working directory, `profiled` stamped the string as
written, and the host runner starts each child in its request's `workspace`.
So a relative profile was certified as one directory and handed to every agent
process as another. Measured before the repair, as a witness: a host-runner
Claude Code pool whose `profile` is `.` passed `refuse_unusable_profiles` on
the unchanged code. This test is that witness, turned into the refusal.

## `fn a_profile_that_cannot_be_inspected_refuses_with_the_error() {`

Check 4, a path `fs::metadata` cannot inspect: refused with the error, not as
missing. The path is absolute and carries a NUL byte, which no platform's
filesystem call accepts (`ErrorKind::InvalidInput`), so the case is reached
without creating or changing a permission on anything. It is here so the
closing sentence [`refused_before_anything_ran`] asserts holds for the last of
the profile refusals too.

## `fn an_oauth_token_in_the_coordinators_environment_refuses_without_printing_it() {`

Check 5: `CLAUDE_CODE_OAUTH_TOKEN` in the coordinator's environment would
override the profile's credentials, so the run would not use the profile it
names. The refusal names the variable, the pool and the profile, and never
the token's value.

## `fn a_profiled_claude_code_pool_on_the_host_with_its_directory_and_no_token_certifies() {`

The pass case: a Claude Code pool whose profile directory exists, on the host
runner, with no token set, certifies. The checks add no process of their
own: the probes are the shell and the one agent, as for an unprofiled run.

## `fn the_claude_code_probe_of_a_profiled_pool_runs_under_that_profile() {`

The pre-flight's agent probe runs under the profile. The runner is assembled
as `entry` assembles it, with `profiled` over the run's runner, and the
pre-flight probes through it. The Claude Code probe arrives with its pool's
profile. The shell probe, which has no agent, arrives with none.

## `fn pools_that_name_no_profile_leave_the_preflight_as_it_was() {`

An unprofiled run's pre-flight is unchanged. With pools that name no profile,
none of the five checks applies: not on the container runner, not for an
agent other than Claude Code, and not with a token set. The probes are the
ones the run always made.

## `fn the_refusal_notes_keep_a_probe_whose_process_is_unresolved() {`

Review round 2's `R2-REG-2` (the PR11 record, §13, round R2). The `certify` section of
`docs/internals/engine/topology/preflight.md` promised that every registration made before a refusal is
cancelled and every pair released, so the ledger balances on both paths — false since round R1, which
keeps a probe whose process the Runner could not establish as ended registered, with its pair, until the
process exits. The pin holds the exception and that the unconditional promise does not come back.
