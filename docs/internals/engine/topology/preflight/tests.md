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

## `fn a_probe_refusal_ends_saying_the_run_is_resumable_and_without_a_full_stop() {`

The ending of [`refused`], the refusal every failed shell or CLI probe takes:
its message ends "so the run is resumable", with no full stop after it, since
`Display` text takes no trailing period (`standards/07`). Checked under both
policies, because the sentence before it names a different boundary for each:
the host, or the recorded image.

This test builds the error directly instead of driving `certify`, because the
ending is in the helper's own text, the same for whatever probe failed.
[`PROFILE_REFUSAL_CLOSING`] pins `profile_refused`'s ending, and before this
test nothing pinned `refused`'s, so its period could come back with every test
still passing (`PR400-PROBE-REFUSAL-PERIOD-UNPINNED`).

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
alike: no agent CLI was started and no event was appended, and the operator
corrects the pools file or the environment and runs the same command again. It
carries the full stop that ends the sentence before it, so `ends_with` holds the
closing to being a sentence of its own, and none at its own end, so `ends_with`
also holds the message to `Display` text's lack of a trailing period
(`standards/07`). Spelled out here rather than read from
`profile_refused`, so a change of wording is a change to this test as well, and
to `engine::tests`' `assert_a_profile_refusal_created_nothing`, which spells the
same closing for the fresh path. It replaces `refused`'s closing, which said the
run was resumable: on a fresh run the profile refusal comes before the lease and
`create_run`, so there is no run to resume (`PR386-FRESH-REFUSAL-NOT-RESUMABLE`).

It says only what is true where each path refuses: a fresh run in `entry`'s
`prepare_fresh`, before the lease, with nothing published, locked or created
(Git children ran, read-only); a resume first in `certify`, under its locks and
after its run exclusion and include, before any probe or recovery event. Until
the fresh refusal moved into `prepare_fresh` the closing was "Nothing was
created, spawned or appended", though the fresh refusal followed the lease, the
exclusion and the include then, and the resume's follows its own.

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

## `const SECOND_DIRECTORY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/agent");`

---------------------------------------------------------------------------
Sign-in, under each account
---------------------------------------------------------------------------

## `const SECOND_DIRECTORY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/agent");`

A second profile directory that exists in every checkout, for a second
profiled pool, as [`EXISTING_DIRECTORY`] is for the first. Nothing is read
from it: the profile only has to pass `refuse_unusable_profiles` and be a
path the CLI double can tell apart.

## `const THIRD_DIRECTORY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/engine");`

A third, so a run has two further pools and the identities of two further
pools' discoveries have something to collide with.

## `const CLAUDE_HELP: &str = concat!(`

The `claude --help` text the double answers: every flag and effort level the
Claude Code adapter's probe requires, as `agent::claude`'s own help tests
spell them, so the real adapter's probe passes and reaches its discovery.

## `const LOGGED_IN: &str = concat!(`

What `claude auth status --json` prints for a signed-in account.

## `const LOGGED_OUT: &str = concat!(`

What it prints for a signed-out one, which it answers with exit 1.

## `fn answered(code: i32, stdout: &str) -> ProcessOutput {`

A process that exited `code` and printed `stdout`.

## `fn answers_signed_in(_profile: Option<&Path>) -> ProcessOutput {`

The double's `auth status --json` answers, by the profile the request
carries: signed in under any profile, or none.

## `fn answers_signed_out(_profile: Option<&Path>) -> ProcessOutput {`

Signed out under any profile, or none.

## `fn answers_signed_in_under_the_first_profile_only(profile: Option<&Path>) -> ProcessOutput {`

Signed in under [`EXISTING_DIRECTORY`], the first pool's profile, and signed
out under every other, the second pool's included.

## `fn answers_signed_in_under_a_profile_only(profile: Option<&Path>) -> ProcessOutput {`

Signed in under any profile, and signed out under none: the agent's default
account is signed out and every profile directory signed in.

## `struct ClaudeCli {`

A runner that plays the Claude Code CLI to the real adapter
(`crate::agent::BuiltinAdapters`): `--version` and `--help` as above,
`auth status --json` by the request's profile through `auth_status`, and
exit 0 for anything that is not `claude` (the shell probe). It records every
request, so a test reads which identities ran under which profiles.

## `impl ClaudeCli` › `fn discoveries(&self) -> Vec<(String, Option<PathBuf>)> {`

The `auth status --json` requests, in the order they ran: each one's rendered
identity and the profile it reached the CLI with.

## `fn claude_code_preflight<'a>(runner: &'a dyn Runner, pools: &[Pool]) -> RunPreflight<'a> {`

A resume's pre-flight of one recorded agent, Claude Code, with the real
adapter, the run's pools, and no token in the environment.

## `fn profiled_over(cli: &Arc<ClaudeCli>, pools: &[Pool]) -> Arc<dyn Runner> {`

The double with `profiled` over it, as `entry` assembles the run's runner, so
a discovery that carries no profile is stamped as a run's would be.

## `fn two_profiled_pools() -> [Pool; 2] {`

Two profiled Claude Code pools, `max` on [`EXISTING_DIRECTORY`] and then
`spare` on [`SECOND_DIRECTORY`]: both absolute, existing directories, so
`refuse_unusable_profiles` passes them on the host runner.

## `const SIGN_IN_REFUSAL_CLOSING: &str = concat!(`

The closing sentence of the sign-in refusal, up to the parenthesis that says
how to sign in: nothing was spawned for the run, no event was appended, and the
same command runs again once that account is signed in. Spelled out here, as
[`PROFILE_REFUSAL_CLOSING`] is, so a change of wording is a change to this test.
It is the sentence `signed_out` writes once for both paths, and it differs from
both neighbours on purpose: `refused`'s says the run is resumable, false on a
fresh run, and `profile_refused`'s says no agent CLI was started, false once
the probe and the discovery ran.

## `fn a_sign_in_refusal(error: &UpstrokeError) -> String {`

What every sign-in refusal shares, asserted once: a `Refused` that opens
with `pre-flight: agent`, the agent's name and `is not signed in under`, carries
[`SIGN_IN_REFUSAL_CLOSING`] once, ends with how to sign in and no full stop,
and carries nothing of `refused`'s probe closing. Returns the message for the
test's own pins.

## `fn a_second_profiled_pool_that_is_signed_out_refuses_naming_that_pool_and_its_profile() {`

The witness. Two profiled Claude Code pools, whose `auth status --json`
answers `loggedIn: true` under the first profile and `loggedIn: false`, exit
1, under the second; the runner is `profiled` over the double, as `entry`
builds it. `certify` refuses, naming the second pool, its profile and how to
sign it in, and not the first. The first pool's discovery ran at the adapter's
own identity (`o2`) and the second's at its own (`o18`), each under its own
profile. Nothing was certified and the ledger balances.

On the code before the sign-in check (`a6412450`), `certify` ran no discovery
and certified this run: a profile that was not signed in failed at its first
attempt instead (`PR386-PROFILE-AUTH-NOT-PREFLIGHTED`).

## `fn every_profiled_pool_signed_in_certifies_and_every_discovery_is_registered_and_settled() {`

Three profiled pools, all signed in: `certify` passes. The requests are the
shell probe, Claude Code's two capability probes, then one discovery per pool
in file order, at `o2`, `o18` and `o34`, so two further pools' discoveries
registered distinct identities. Each ran under its own pool's profile. Six
processes were registered and each completed once; nothing is running and
the ledger balances.

## `fn an_unprofiled_agent_whose_default_account_is_signed_out_refuses_naming_that_account() {`

An agent whose only pool names no profile, signed out: `certify` refuses,
naming the pool, `max`, and that as it names no credential profile it runs
under the agent's default account, and `claude auth login` with no
`CLAUDE_CONFIG_DIR`. One discovery ran, under no profile. Until an unprofiled
pool's default account was checked beside profiled pools, the refusal said
"under its default account, as no pool of this run names a credential profile
for it"; it names the pool now, as every account's refusal does.

## `fn a_discovery_that_cannot_say_whether_the_account_is_signed_in_certifies() {`

`auth status --json` that prints no JSON (a CLI that predates the subcommand),
or that times out, is an unknown sign-in, and an unknown one passes. Both,
under the default account and under two profiled pools: `certify` passes, one
discovery ran per profiled pool (or one with none), and the ledger balances.

## `fn a_fresh_runs_probes_refuse_a_signed_out_pool_in_the_resumes_words() {`

The fresh path. `RunnerProbes`, given the run's pools with `with_pools`,
probes Claude Code through a `ProbePair`'s agent probe over `profiled` and the
double, as `create_run`'s P4 does. With the second pool signed out it refuses,
records no capabilities, ran the two discoveries at their two identities, and
settled every process in the pair. Its refusal is the same text a resume's
`certify` returns for the same pools: one refusal, worded once, on both paths.

## `fn a_profiled_pool_on_the_container_runner_is_refused_before_any_discovery() {`

On the container runner, a profiled pool is refused by
`refuse_unusable_profiles`, as before, in its own closing. No discovery ran,
nor any other process, and nothing was registered.

## `fn a_default_pool_then_a_profiled_spare() -> [Pool; 2] {`

---------------------------------------------------------------------------
Every account, the default included
---------------------------------------------------------------------------

## `fn a_default_pool_then_a_profiled_spare() -> [Pool; 2] {`

Two Claude Code pools, `max` first, naming no profile, so it runs under the
agent's default account, then `spare` on [`SECOND_DIRECTORY`]. With
`profiled` over them nothing is stamped on the agent's probes, as `max`, the
agent's bound pool, names no profile.

## `const MAX_DEFAULT_ACCOUNT: &str = concat!(`

How the refusal names `max`'s account: the pool, and that as it names no
credential profile it runs under the agent's default account. Spelled out, as
[`SIGN_IN_REFUSAL_CLOSING`] is, so a change of wording is a change to these
tests.

## `fn lists_each_account_an_agents_pools_use_once_in_file_order() {`

`Account::of_agent` over pools for two agents, in which Claude Code's are an
unprofiled pool, a profiled one, a second unprofiled one and a second naming
the same profile: two accounts, the default account named by `max` and then
the profile named by `spare`, and nothing of the other agent's pool. An agent
with no pool in the run has its default account alone, named by no pool.

## `fn refuses_a_first_pools_signed_out_default_account_though_its_profiled_spare_is_signed_in() {`

[`a_default_pool_then_a_profiled_spare`], with the default account signed out
and the spare signed in, on a resume: `certify` over `profiled_over` refuses,
naming `max` and the agent's default account ([`MAX_DEFAULT_ACCOUNT`]) with
`claude auth login` and no `CLAUDE_CONFIG_DIR`, and not the spare. Both
discoveries reached the CLI, in file order: the default account's at the
adapter's own identity (`o2`) under no profile, then the spare's at its own
(`o18`) under its profile. Nothing was certified and the ledger balances.

The witness of the default account checked beside a profile. On the code
before it, the agent's accounts were its profiled pools alone, so the one
discovery ran under `spare`'s profile, which answers signed in, and `certify`
passed though `max`'s workers would run under a signed-out account.

## `fn certifies_a_signed_in_default_account_and_profiled_spare_settling_every_discovery() {`

The same pools with both accounts signed in: `certify` passes. The requests
are the shell probe, Claude Code's two capability probes, then the default
account's discovery at `o2` and the spare's at `o18`, under no profile and
under the spare's. Five processes were registered and each completed once;
nothing is running and the ledger balances, as
`every_profiled_pool_signed_in_certifies_and_every_discovery_is_registered_and_settled`
holds for profiled pools alone.

## `fn refuses_a_signed_out_default_account_on_a_fresh_run_in_the_resumes_words() {`

The fresh path, as `a_fresh_runs_probes_refuse_a_signed_out_pool_in_the_resumes_words`
drives it, over the same pools: `RunnerProbes`, given them with `with_pools`
and an empty environment with `reading_environment`, refuses naming `max`'s
default account, records no capabilities, ran the two discoveries at `o2` and
`o18`, under no profile and the spare's, and settled every process in the
pair. Its refusal is the text a resume's `certify` returns for the same pools.

## `fn refuses_a_later_unprofiled_pools_signed_out_default_account_discovered_under_no_profile() {`

The other order: `max` on [`EXISTING_DIRECTORY`] first, then `spare` with no
profile; the profile signed in and the default account signed out. `certify`
refuses naming `spare` and the agent's default account, not `max` or its
profile. The first discovery ran at `o2` under `max`'s profile, and the
second, `spare`'s, at `o18` reached the CLI with no profile.

That last is `profiled`'s further-account rule (`profiled.md`,
`Profiled::stamped`): `profiled` stamps `max`'s profile on any of the agent's
probes that carries none, and without the rule `spare`'s discovery would have
run under `max`'s profile, answered signed in, and certified a default account
no one asked.

## `fn discovers_each_account_once_however_many_pools_use_it() {`

Two unprofiled pools, and two pools naming one profile: each is one account,
so one discovery runs, at `o2`, under no profile or under that profile, and
`certify` passes when it is signed in. Signed out, the refusal names `max`, the
first pool that uses the account, and not `spare`, and still one discovery
ran.

## `const CODEX: &str = crate::agent::codex::ADAPTER_ID;`

---------------------------------------------------------------------------
Environment credentials
---------------------------------------------------------------------------

## `const CODEX: &str = crate::agent::codex::ADAPTER_ID;`

The agent with environment credentials (`preflight.md`,
`ENVIRONMENT_CREDENTIALS`), by the id its row is keyed by.

## `const PLACEHOLDER: &str = "placeholder-credential-value";`

What a fixture environment gives a variable it sets: recognisable, so a
refusal can be asserted not to carry it, and no credential.

## `struct SignedOutCodex;`

An adapter registered as `codex` whose probe is [`Stub`]'s and whose
`discover` answers `NotAuthenticated` and runs nothing: Codex as its
`login status` answers with no stored login, whatever its environment holds.

## `struct SignedOutCodexAdapters;`

The registry holding [`SignedOutCodex`] alone.

## `fn codex_preflight(`

A resume's pre-flight of one recorded agent, `codex`, with no pools, reading
`environment`.

## `fn codex_api_key_set(key: &str) -> Option<OsString> {`

Environments with exactly one of Codex's variables set to [`PLACEHOLDER`]:
`CODEX_API_KEY` here, `OPENAI_API_KEY` and `CODEX_ACCESS_TOKEN` in the two
after it.

## `fn codex_variables_empty_and_others_set(key: &str) -> Option<OsString> {`

Codex's three variables present and empty, and every other variable set to
[`PLACEHOLDER`]: none of Codex's set to a value, and a value within reach of
any refusal that printed one.

## `fn certifies_a_signed_out_codex_account_while_one_of_its_environment_credentials_is_set() {`

The reviewer's witness, ported. Codex's discovery answers `NotAuthenticated`,
and with `CODEX_API_KEY` set in the environment pre-flight reads, `certify`
passes; again with `OPENAI_API_KEY`, and with `CODEX_ACCESS_TOKEN`. On the code
before the environment-credential table, `NotAuthenticated` refused whatever
the environment held, so this run, which works, was refused.

## `fn refuses_a_signed_out_codex_account_with_no_environment_credential_naming_never_valuing_them() {`

The same agent with none of its variables set: nothing set at all, or each
present and empty with every other variable set. `certify` refuses, naming
`codex` and its default account (no pool of this run names it), in the sign-in
closing, with a hint that names the three variables, and carries no
[`PLACEHOLDER`].

## `fn refuses_a_signed_out_claude_code_account_whatever_codex_variable_is_set() {`

The variables are per agent: Claude Code with no pool, its default account
signed out, and `CODEX_API_KEY` set, refuses as it would with nothing set,
naming its default account as no pool of this run names it, with `claude auth
login`, and nothing of Codex's variable.

## `fn the_refusal_notes_keep_a_probe_whose_process_is_unresolved() {`

Review round 2's `R2-REG-2` (the PR11 record, §13, round R2). The `certify` section of
`docs/internals/engine/topology/preflight.md` promised that every registration made before a refusal is
cancelled and every pair released, so the ledger balances on both paths — false since round R1, which
keeps a probe whose process the Runner could not establish as ended registered, with its pair, until the
process exits. The pin holds the exception and that the unconditional promise does not come back.
