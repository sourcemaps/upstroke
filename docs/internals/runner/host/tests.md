# `src/runner/host/tests.rs`

Extended notes for [`src/runner/host/tests.rs`](../../../../src/runner/host/tests.rs).

The code is the authority for what it does; this file is the whole of its prose, moved out of
the source verbatim. Each section is headed by the line of code the comment sat above, spelled
as it is in the source, so the heading is the grep string that finds the code.

## `#![allow(`

Allowlist placement: the **funnel section** of `effects/allowlist.toml`, whose entry for this file
carries its review clause. This is the Process funnel's test module, out of line from
`src/runner/host.rs`: the fixtures build and tear down scratch trees, mark programs executable and
spawn real child processes.

It states the level **of its own**. A lint level is scoped by the module tree, not the file, so an
out-of-line child of a funnel would otherwise inherit the parent's allow silently, and the funnel's
child-module census (`runner::container::tests::every_child_module_of_the_container_funnel_states_its_own_lint_level`)
refuses a child that does not state its own. All three lints are needed; the counts are in the
review clause. `decisions.effect_site_inventory.mechanism` (2).

## `use std::time::Duration;`

`src/runner/host.rs` does not import `Duration`, so `use super::*` does not supply it here.

## `fn os(value: &str) -> OsString {`

The helpers start here.

## `fn gate_invocation() -> InvocationId {`

A gate identity, for the tests whose subject is not the identity.

## `fn shell_probe_invocation() -> InvocationId {`

The pre-flight shell probe's identity: the probe form, target `Shell`.

## `fn worker_invocation() -> InvocationId {`

One attempt's worker identity: the attempt form, role `worker`.

## `fn review_invocation() -> InvocationId {`

One review pass's identity: the same form, role `review_pass0`.

## `fn fixture_agent() -> AgentId {`

The agent a worker or a reviewer is bound to. `ExecutionRole::all` names the same adapter for its
agent-probe target, so the grid's three slotted roles carry a real, shipped id, which is what
`host-v1` looks a credential location up by.

## `fn synthetic_base() -> Vec<(OsString, OsString)> {`

A base written by the test rather than read from the machine, so a composition fixture asserts the
same thing on every machine.

## `fn native() -> ShellKind {`

The shell every platform has, invoked exactly as `gates` invokes it.

## `fn new_resolves_the_same_record_as_resolve_host() {`

The policy tests start here.

## `fn environment_composition_fixtures() {`

The environment composition fixtures (proof test: "environment composition fixtures") start here.

## `fn environment_composition_fixtures()` › `struct Fixture {`

Every fixture is (role, agent, overlay, what the composition must say). The expected values are
written from `design/08_design_trait_surface.md:65-71`, not read back out of `compose`.

## `fn environment_composition_fixtures()` › `("CLAUDE_CONFIG_DIR", None),`

The base carries all three; a gate is repository code and receives none of them.

## `fn environment_composition_fixtures()` › `("CODEX_HOME", None),`

A claude-code worker is not told where codex's or copilot's credentials live.

## `fn environment_composition_fixtures()` › `("PATH", Some("/usr/bin:/bin")),`

A reviewer runs an agent CLI, so it is supplied the unscoped values too.

## `fn environment_composition_fixtures()` › `("CLAUDE_CONFIG_DIR", None),`

The host knows no credential location for this agent, so it supplies none, and the base's three do
not leak in its place.

## `fn environment_composition_fixtures()` › `let roles: BTreeSet<String> = fixtures.iter().map(|f| f.role.label()).collect();`

Fixture hostility as distinct-value counts.

## `fn environment_composition_fixtures()` › `let mut seen: Vec<&OsString> = Vec::new();`

One variable, one entry: a duplicated key is an environment whose meaning depends on which end the
child's runtime reads.

## `fn every_composed_environment_disables_replacement_objects() {`

Every role composes the replacement isolation, from any base and under any overlay.

`HostRunner::run` clears the ambient environment and installs exactly what `compose` returns, so a
pair not composed there reaches no child. The base carries a value of its own and one overlay
restates the key, because the pair is upserted *after* the overlay so that neither can decide it:
an exact snapshot is measured against the objects the repository holds (`design/15`, "What an
exact snapshot is exact against"), and an adapter gets no vote on that. The key is not a reserved
one, so without the upsert the base's own value would survive.

## `fn the_v1_conductors_environment_confines_the_recorded_graph_to_its_repository() {`

The v0.1 conductor's environment confines the recorded graph to the repository the run manages,
through includes rather than the process-wide `GIT_NO_REPLACE_OBJECTS`: a variable reaches every
repository a role touches, and on Git 2.41 it loses to a configured `true`.

It asserts four things. The constructor reads `ObjectGraph::RecordedIn` with the repository it was
given. Over every role, both name rules and five sources of `GIT_CONFIG_PARAMETERS` (none, empty,
the base's `true`, an overlay's, and both), the composed value is the inherited one, a space and the
two includes, or the includes alone: never before an inherited entry, never with a leading space,
and one variable however the base spelled its name. `GIT_NO_REPLACE_OBJECTS` is never composed, and
an operator who exports it keeps their own value. And the constructor's own environment, over this
process's base, ends with the includes.

Its repository is a real directory under a scratch tree (`managed_repository`), because the scope
asks each directory on the path whether it folds case and refuses a path that is not there. It also
asks the common directory, through the `refs` every common directory holds, so the fixture creates
`refs` too.

## `fn the_includes_refuse_a_common_directory_git_cannot_read_in_a_key() {`

A newline in the path and a relative directory or include are refused before any directory is
asked anything, and a common directory that is not there is refused because the scope cannot ask it
whether it folds case. Unix-only, because a Unix absolute path is not absolute on Windows. The
entries themselves depend on what each directory on the path answers, so
`src/runner/host/environment.rs`'s inline tests pin them, supplying the answers.

## `fn the_windows_rule_spells_a_path_as_git_for_windows_does() {`

`GitdirRule::Windows` over the shapes `std::fs::canonicalize` gives on Windows
(`\\?\C:\…`, `\\?\UNC\server\share\…`) and over already-plain ones, on every
platform: no prefix, forward slashes, `//server/share` for a share. Also that
the keyword is `gitdir:` under both rules, and that the native rule is `Windows`
on Windows and `Posix` everywhere else, macOS included.

## `fn a_v1_role_never_starts_without_the_include_that_confines_its_recorded_graph() {`

`HostRunner::run` under `RecordedIn` refuses a gate while the include is missing, holds `true`, or
is empty, as `ProcessFate::NeverStarted` naming the file, and runs it once the file holds the exact
bytes. Its common directory is created first, with `refs`, because the scope asks each directory on
the path, and the common directory itself through `refs`, whether it folds case.

## `fn a_repository_whose_path_differs_from_the_managed_one_only_in_case_keeps_its_replacements() {`

The negative witness: the v0.1 include must not reach a repository whose path differs from the
managed one only in case. The work is `v1_case_sibling_helper`'s, in a child whose environment
`run_replacement_witness_child` has cleared of every ambient replacement control. On a filesystem
that keeps case the folding checks do not run, so only a case-folding temporary directory exercises
them.

## `fn case_git(dir: &Path, args: &[&str]) -> String {`

The witness's own Git, in its own process rather than through the runner, with an identity and
without auto-maintenance, so no background process outlives a commit in the scratch tree.

## `fn replaced_repository(repo: &Path) -> String {`

A repository whose committed blob `recorded` is replaced by `replacing`, checked live outside any
run before anything is measured; it returns the blob's id.

## `fn read_case_siblings(dir: &Path, include: &Path) -> bool {`

One layout: `Repo` managed, and beside it `repo` and `rEPO`, the second being the spelling the scope
itself looks up. When the directory keeps case they are repositories of their own with their own
replacements; when it folds they are `Repo`, and creating them finds it there. For every rule this
platform's Git reads (both on Unix; on Windows only `Windows`, since the `Posix` rule keeps a Windows
path's backslashes and Git for Windows spells its Git directory with forward slashes), a scope over
`Repo`'s canonical common directory and `HostRunner::for_legacy_workspace` run gates that read:

- `Repo` and its linked worktree: `recorded`;
- each sibling, from `Repo` and from the sibling itself: `replacing` when it is a repository of its
  own, `recorded` when it is `Repo`;
- when the directory folds, `Repo` through a Git directory named `REPO/.GIT` under the layout's
  directory spelled in capitals: `recorded`. Given with `--git-dir`, that spelling reaches Git's
  condition as spelled on Linux, whose realpath does not look the case up; reached with `-C`, Git
  names the repository in its stored case, as `pwd -P` does, so the `-C` reads alone cannot tell a
  scope that never folds from one that does;
- when the directory folds, `Repo`'s linked worktree through its Git directory given with
  `--git-dir`, spelled as Git stores it, with `worktrees` alone in capitals, and with every component
  the test controls in capitals: `recorded`. The linked worktree's name under `worktrees` is taken
  from Git (`rev-parse --absolute-git-dir`), not assumed, so a scope that matched `worktrees` as a
  literal fails here.

It returns whether the directory kept case.

## `fn v1_case_sibling_helper() {`

Runs `read_case_siblings` in the temporary directory, and on Windows again in a directory `fsutil
file setCaseSensitiveInfo` makes case-sensitive inside the folding temporary directory. It prints
which halves ran, in lines beginning `v1-case-sibling:`, so a CI log shows it.

Where the leg declares `UPSTROKE_TEST_TEMP_FOLDS_CASE=1` (macOS and Windows) the temporary directory
must fold, or the declaration and the filesystem disagree; elsewhere its behaviour is observed, not
required, as `the_temporary_object_scan_resolves_case_aliases_as_the_filesystem_does` treats the
same declaration. So the case-sensitive half runs where the temporary directory keeps case, and on
Windows wherever `fsutil` succeeds; macOS runs the folding half only, since case sensitivity there
is a property of a whole volume. A Windows machine where `fsutil` fails prints that and runs the
folding half.

## `fn a_reserved_key_the_base_does_not_carry_is_not_supplied()` › `let environment =`

"Set but empty" and "unset" are different environments, and CLIs read them differently.

## `fn a_reserved_key_in_the_overlay_is_a_preflight_error() {`

The refusal is `expected_failures_refusals[0]`: "reserved env conflict -> pre-flight error".

## `fn a_reserved_key_in_the_overlay_is_a_preflight_error()` › `let expected_reserved = [`

Written out from the runner contract for role-scoped `HOME`, `PATH` and credential locations
(`design/26_design_merge_queue_protocol.md:457-459` names the three credential-location variables),
not read from `reserved_keys()`.

## `fn a_reserved_key_is_refused_at_every_position_in_the_overlay() {`

Refused **wherever it sits in the overlay**. The other reserved-key fixtures hand `compose` a
one-pair overlay, so the conflict is always `overlay.first()`, and a scan that stopped after the
first pair would pass them. `invariants_introduced[0]`, "reserved keys refused pre-flight", is a
claim over the whole vector, so the grid puts the conflict at every position of an overlay of four:
first, two interior, and last.

## `fn a_reserved_key_is_refused_at_every_position_in_the_overlay() {` › `let harmless: Vec<(String, String)> = HARMLESS`

The control: four harmless pairs compose, so a refusal below is the reserved key and not the shape
of the overlay.

## `fn a_reserved_key_is_refused_at_every_position_in_the_overlay() {` › `assert_eq!(refusals, 6 * 4 * KeyCase::ALL.len());`

Six keys × four positions × two key cases, counted so a grid that stopped traversing fails rather
than passing smaller.

## `fn an_overlay_that_restates_a_reserved_key_with_the_runners_own_value_is_still_refused() {`

Refused by **key**, whatever the value, including the runner's own. Neither
`invariants_introduced[0]` nor `expected_failures_refusals[0]` ("reserved env conflict -> pre-flight
error naming the key") excepts an overlay that agrees with the runner: one allowed to restate `PATH`
because the value matches has taken ownership of the key, and breaks silently when the runner's
value changes. Every other reserved-key fixture supplies a *different* value, so equality is the
one case they cannot see.

## `fn windows_treats_reserved_keys_case_insensitively_and_unix_does_not() {` › `let overlay = vec![("Path".to_owned(), "C:\\hijack".to_owned())];`

The platform axis is a value here, not a `cfg!`, so both arms run on every host and a Linux run
proves the Windows rule.

## `fn windows_treats_reserved_keys_case_insensitively_and_unix_does_not() {` › `let composed = insensitive`

The same-key rule also decides an upsert, not only a refusal.

## `fn probe_and_execution_compose_the_same_environment() {`

`design/08_design_trait_surface.md:69-71`: "Probe and execution compose the same base, mounts,
reserved values, and overlay, so pre-flight certifies the environment that will actually spend."

## `fn environment_dump_helper() {`

Prints this process's whole environment between two markers.

## `fn dumped_environment(runner: &HostRunner, request: &RunnerRequest) -> Vec<String> {`

One run of the dump helper, as the environment its child actually carried.

## `fn a_probe_child_and_an_execution_child_of_one_adapter_carry_the_same_environment() {`

The same passage (`design/08_design_trait_surface.md:69-71`) over two real children of one adapter
and one runner, rather than two composed maps, so a base or overlay replaced on the way to the
process is seen. The probe half is built by production (`agent::probe_request`), so a substitution
made there is inside the comparison. Two sentinels, because two empty environments are also equal:
one comes only from the base, the other only from the overlay, and both must arrive.

## `fn a_probe_child_and_an_execution_child_of_one_adapter_carry_the_same_environment() {` › `let execution = crate::runner::worker_request(`

The worker half comes from production's builder, so it is the request an attempt sends: the bound
agent and the worker identity.

## `fn dump_command(overlay: &[(&str, &str)]) -> CommandSpec {`

The environment dump helper as a command, with `overlay` as its request's overlay.

## `fn received_common_dir(runner: &HostRunner, request: &RunnerRequest) -> Vec<String> {`

The `GIT_COMMON_DIR` entries of the environment a real child of `runner` dumped for `request`.

## `fn the_host_runner_reports_the_common_dir_its_child_is_started_with() {`

`HostRunner::composed_common_dir` is the value the child is started with. It is checked against a
real child's dumped environment, over a custom `HostEnvironment` with an overlay rather than
`HostRunner::new` alone:

- a base value;
- an overlay's value over the base's;
- none anywhere, `Absent`;
- a relative value, which a gate does not receive as it is (the work-tree context, in
  `child_environment`). With the run's repository held, by `ObjectGraph::RecordedIn` over
  `managed_repository`, the report and the child's value are both that repository's common
  directory. With none held, the report is `Unknown` and the same composition refuses the run
  `NeverStarted`. One assertion compares both reports, so a work-tree step outside
  `child_environment`, which would make the report a second composition beside the child's, shows
  both reports wrong at once: each would be the relative value;
- a repeated name, where the later entry is the one the child receives and the one reported;
- an overlay naming a reserved key, which reports `Unknown` while the same composition refuses the run.

The base, overlay and repeated values are absolute on the platform that runs the test (`/…` on Unix,
`C:\…` on Windows), because a gate's inherited value that is not absolute meets the work-tree
context.

On Unix, two more rows:

- a base built with `KeyCase::Insensitive` that holds `git_common_dir`. This reports `Absent`, and
  the child receives the lowercase name and not the variable;
- a value that is not UTF-8, reported byte for byte.

## `fn every_credential_supplied_role_composes_one_environment_per_binding() {`

The same claim over **every** role production binds to an agent and **every** shipped binding, in
the children rather than in a map (`design/08_design_trait_surface.md:65-71`: each runner "supplies
role-scoped `HOME`, `PATH`, and credential locations", and probe and execution compose the same
environment).

The test above holds one pair, `Probe(Agent(claude-code))` against `Implement`, and
`supplies_credentials` names three roles. So the domain is `ExecutionRole::all()` filtered by
`supplies_credentials`, and a role added later is covered or fails here. Three sentinels rather
than equality alone, because two identical absences are also equal: one value only the base can
supply, one only the overlay can, and the credential location, which only the **binding** can, and
which a dropped binding removes.

## `fn every_credential_supplied_role_composes_one_environment_per_binding() {` › `for (_, key) in CREDENTIAL_LOCATIONS {`

A credential location already in the base: composition strips reserved keys, so only the agent
binding can put the value back.

## `fn every_credential_supplied_role_composes_one_environment_per_binding() {` › `assert!(`

The binding's own contribution. `key` is reserved, so a child carrying it carries a value
composition put there, and a role whose binding was dropped on the way to `compose` carries nothing
under this name.

## `fn the_reserved_values_every_role_gets_are_the_host_boundarys_own() {`

What `host-v1` supplies for `HOME`, `PATH` and `USERPROFILE`, asserted from the passages that decide
it rather than as a count of distinct values. `design/08_design_trait_surface.md:67` names three
things role-scoped (`HOME`, `PATH` and credential locations) and `host-v1` scopes only the last;
that reading is recorded as `PR4-DESIGN-ROLE-SCOPED-ENV`. A count such as "`PATH` took one value
across the five roles" is a claim no passage makes and would fail the plainest reading of :67, so
the test states three claims, one per passage:

* **Every role is supplied each of the three keys the base carries.** The runner *supplies* them; a
  role handed none satisfies any count of distinct values.
* **The roles a passage pairs are supplied identical reserved sets.**
  `design/08_design_trait_surface.md:69-71` ("probe and execution compose the same base, mounts,
  reserved values, and overlay") over `probe(<agent>)`, `implement` and `review`;
  `design/26_design_merge_queue_protocol.md:470-473` ("gate-shell/program availability is checked
  inside the same boundary") over `probe(shell)` and `gate`.
* **The value is the host boundary's own.** `design/26_design_merge_queue_protocol.md:441-442`: "the
  host base starts from the Upstroke process environment". The expected values are read out of the
  base vector this fixture wrote, never out of [`HostEnvironment::lookup`]: `reserved_values` calls
  `lookup` too, and a function used as its own oracle asserts nothing.

The two groups' union is compared against `ExecutionRole::all()` as a set, so a role dropped from
both cannot go unasserted.

## `fn the_reserved_values_every_role_gets_are_the_host_boundarys_own() {` › `let agent = AgentId::new(claude::ADAPTER_ID);`

The agent `ExecutionRole::all()` names, so the groups below can be compared against it as sets.

## `fn the_reserved_values_every_role_gets_are_the_host_boundarys_own() {` › `let from_the_boundary = ["PATH", "HOME", "USERPROFILE"];`

Written here, not read from `RESERVED_ALWAYS`: the set is the claim.

## `fn the_reserved_values_every_role_gets_are_the_host_boundarys_own() {` › `let in_the_base = |key: &str| -> OsString {`

The independent oracle: the base this fixture wrote.

## `fn the_reserved_values_every_role_gets_are_the_host_boundarys_own() {` › `let certified: &[(&str, Vec<ExecutionRole>)] = &[`

The roles each passage holds together, and the passage, so a failure names the sentence broken.

## `fn credential_locations_are_role_scoped() {`

The half `host-v1` **does** scope: credential locations. The expected split is written from what
each role executes (a worker, a review and an agent probe run an agent CLI; a gate is repository
code and the shell probe is a shell), not computed from `supplies_credentials` or `is_slotted`.

## `fn compose_gives_a_child_the_credential_location_of_its_own_agent_and_no_other() {`

The same split, asserted about the **environment the child receives** (`compose`) rather than the
list the runner supplies (`reserved_values`): a `compose` that copied the whole base first would
hand a gate the coordinator's `CODEX_HOME` whatever `supplies_credentials` said.

The grid crosses every role with every agent, including one the host knows no location for, under
both name rules. The expected value of each credential key is written from the rule ("the bound
agent's location, and only for a role that runs an agent CLI"), not read from `supplies_credentials`
or `reserved_values`.

## `fn compose_gives_a_child_the_credential_location_of_its_own_agent_and_no_other() {` › `let runs_an_agent_cli = |role: &ExecutionRole| match role {`

Written from what each role executes, as `credential_locations_are_role_scoped` writes it.

## `fn compose_gives_a_child_the_credential_location_of_its_own_agent_and_no_other() {` › `for (key, expected) in [`

The host boundary's own keys are still there for every role: `host-v1` supplies the boundary's
value of `HOME` and `PATH` to all five roles (see
`the_reserved_values_every_role_gets_are_the_host_boundarys_own`).

## `fn compose_gives_a_child_the_credential_location_of_its_own_agent_and_no_other() {` › `for (key, expected) in [("LANG", "C.UTF-8"), ("UPSTROKE_RUN", "01ABCDEF")] {`

Nothing else was dropped on the way through.

## `fn compose_gives_a_child_the_credential_location_of_its_own_agent_and_no_other() {` › `assert_eq!(supplied_count + denied_count, 2 * 5 * 5 * 3);`

Hostility as counts: 2 rules × 5 roles × 5 agents × 3 keys = 150 decisions, and both answers occur.

## `const BASE_WITNESS: &[(&str, &str)] = &[`

The variables the subprocess witness below inherits, and their values. The values are awkward (a
non-ASCII character, an `=`, and the encoding's own separator), so a collector that split or
filtered on any of them loses the entry.

The second name, a drive-working-directory sentinel, exists so that a filter dropping that key
inside `from_process` has something to drop: a filter keyed on a name no test process carries is
inert and no oracle can observe it. The entry-for-entry equality is the general statement; this is
the named guard.

## `fn base_witness_helper() {`

The child half of [`the_base_of_a_process_environment_is_the_process_environment`].

A subprocess rather than `set_var`: the suite is multi-threaded, and mutating this process's
environment while another test reads it is the race `std::env::set_var` is `unsafe` for. The same
shape `proc::tests::sigchld_reaper_host_helper` uses, for the same reason.

## `fn base_witness_helper()` › `let expected: Vec<(OsString, OsString)> = std::env::vars_os().collect();`

The general statement, made where the environment is known to carry awkward entries: any entry
`from_process` drops, filters or reorders shows up here, whatever it is named.

## `fn base_witness_helper()` › `let composed = base`

And each injected entry reaches a composed child environment, for a role handed nothing else.

## `fn the_base_of_a_process_environment_is_the_process_environment() {`

The composed base **is** the Upstroke process environment, entry for entry
(`design/08_design_trait_surface.md:65-66`: "`CommandSpec.env` overlays a runner-owned base rather
than replacing it. The host runner starts from the Upstroke environment"). `run` calls `env_clear()`
before installing the composed block, so anything `from_process` fails to collect is gone from
every child, and every composition fixture supplies its own base through `with_base`, so none of
them would notice.

Two halves. The equality is over whatever this machine carries, which on Windows includes the
`=C:`-style per-drive working directories `vars_os` yields. The subprocess adds awkward entries, so
the equality is not satisfied merely by an environment with nothing interesting in it.

## `fn the_base_of_a_process_environment_is_the_process_environment() {` › `"runner::host::tests::base_witness_helper",`

The **full path**. `--exact` matches the whole test name, so a bare `base_witness_helper` filters
every test out and the child exits 0 having run nothing.

## `fn the_base_of_a_process_environment_is_the_process_environment() {` › `assert!(`

Assert the **count**, never a bare `ok`: `ok. 0 passed` is what a filter that matched nothing
prints, and the exit code cannot tell it from success.

## `fn the_credential_location_is_the_bound_agents_and_no_others_value() {` › `for (agent, key, expected) in [`

An independent table maps each adapter id to its credential-location variable, written out here
rather than read from the code under test.

## `fn handed_config_dirs(environment: &[String]) -> Vec<&str> {`

Every value a dumped child environment carries under `CLAUDE_CONFIG_DIR`, in order. A list rather
than the first match, so a child handed the key twice (from the profile and from the base) fails
the comparisons below instead of passing on whichever entry came first.

## `fn a_worker_request_with_a_profile_hands_its_agent_that_directory_as_claude_config_dir() {`

A worker request carrying a credential profile hands its agent that directory as its credential
location, and not the coordinator's.

Asserted on the **child's environment** (`environment_dump_helper` through `HostRunner::run`), not
on `reserved_values` or `compose`: the profile has to travel from the request through
`child_environment` to the spawned process, and a runner that composed it correctly but read the
request's fields in the wrong place would pass a composition-level test. The request is
production's `worker_request` with `with_credential_profile` added.

The base is `synthetic_base`, which carries the coordinator's own `CLAUDE_CONFIG_DIR`
(`/home/upstroke/.claude`), so there is a value to leak: the child carries exactly one
`CLAUDE_CONFIG_DIR`, the profile, and the coordinator's directory appears nowhere in its
environment. The profile is a scratch directory apart from the workspace, so no other entry can
contain its path by accident.

## `fn the_same_worker_request_with_no_profile_hands_its_agent_the_coordinators_claude_config_dir() {`

The control: the same worker request with no profile hands its agent the coordinator's own value,
exactly once. It also pins that production's `worker_request` carries no profile. Without it, a
runner that dropped `CLAUDE_CONFIG_DIR` whenever the base's value was involved would pass the test
above.

## `fn a_gate_request_carrying_a_profile_hands_its_child_no_credential_location() {`

A gate, which `supplies_credentials` does not cover, carrying a profile is handed neither the
profile nor any credential location.

Two shapes. The gate production builds names no agent, so a runner that ignored the role and keyed
only on the agent would pass it; the second binds the claude agent to the gate (the
`hostile_bindings` shape the container suite uses), so the role check is all that stands between the
profile and the child. For each, the child received the base (`LANG`), so the absences are not
those of an empty environment, and none of the three credential keys, and no entry contains the
profile's path.

## `struct ParityFixture {`

The supervision parity fixtures (proof test: "supervision parity tests") start here.

## `struct ParityFixture` › `floor: Option<Duration>,`

The shortest this child can have taken, as a fact about the child rather than the machine: a
sleeper cannot finish before it has slept, and a child killed for exceeding its timeout ran at least
that long. `None` where the fixture finishes as fast as the machine runs it. This is the lower half
of the duration pin; the upper half is wall clock the test measures around each call. Neither is
the runner's own arithmetic.

## `fn parity_fixtures() -> Vec<ParityFixture>` › `script: "echo problem 1>&2",`

`1>&2` redirects the same way in `cmd` and in `sh`.

## `fn parity_fixtures() -> Vec<ParityFixture>` › `name: "a quoted argument survives the spec",`

`build_command`'s `cmd.exe` rule: std would re-quote this tail as `\"quoted arg\"`, which `cmd.exe`
does not un-escape, so the child would print something else. On Unix nothing special happens and
the fixture still has to agree.

## `fn parity_fixtures() -> Vec<ParityFixture>` › `name: "a sleeping child is measured, not the timeout",`

Pins duration **from below** without naming the timeout. Every other fixture finishes as fast as
the machine can run it, so a duration reported too small would be caught by nothing: `> 0` admits a
nanosecond. A sleeping child gives a floor that holds on any machine, since load only widens the
interval. One second nominal, asserted at half, because `ping -n 2` is "two pings a second apart"
rather than a sleep of exactly a second.

## `fn parity_fixtures() -> Vec<ParityFixture>` › `"ping -n 2 127.0.0.1 > NUL"`

`cmd` has no `sleep`; the timeout fixture below relies on the same shape.

## `fn parity_fixtures() -> Vec<ParityFixture>` › `floor: Some(Duration::from_millis(400)),`

The timeout, written again rather than read from the field beside it: a child killed for exceeding
its timeout ran at least that long, which is why the two values are the same number.

## `fn supervision_parity_tests()` › `let direct_started = std::time::Instant::now();`

Wall clock around the call, measured by the test. It bounds any honest `duration` from above by
construction: the funnel starts its own clock after this instant and stops it before the drain, so
`duration <= elapsed` holds however loaded the machine is.

## `fn supervision_parity_tests()` › `assert_eq!(actual.stdout, expected.stdout, "{}: stdout", fixture.name);`

Byte for byte, line endings included. Both sides run the same script through the same recorded
shell on the same machine, so there is nothing legitimate for a normalization to absorb, and
normalizing both sides would hide a runner that rewrote CRLF to LF on the one platform that
produces CRLF. `invariants_preserved[0]` says output capture is unchanged.

## `fn supervision_parity_tests()` › `for (what, measured, bound) in [`

Duration is part of the record an attempt keeps (`Outcome.duration`), so "supervision unchanged"
includes measuring **this child**. It is not compared for equality with the other supervisor's
duration, because two runs of one script take two times. It is pinned from both sides instead,
against two oracles that are not the runner's arithmetic: the wall clock the test measured around
the call (an upper bound by construction), and `fixture.floor` where the fixture has one.

Positivity alone would admit a runner that reported `request.timeout` as the duration: every
ordinary fixture has a 30s timeout, and the timeout fixture reports exactly its timeout, which its
own floor admits. The elapsed bound is what that cannot satisfy, since a child that echoes and exits
has not taken thirty seconds.

## `fn supervision_parity_tests()` › `assert!(codes.len() >= 3, "distinct exit codes: {codes:?}");`

Hostility as counts: a parity suite whose fixtures all produce the same output proves that two
functions agree about nothing.

## `fn supervision_parity_tests()` › `assert_eq!(`

The duration pin is counted: two fixtures state a floor, one from a sleep and one from a timeout
kill. A grid that lost them would still assert `> 0` on every row.

## `const TRANSPARENT_STDOUT: &[&str] = &[`

The output transparency tests (`invariants_preserved[0]`: "output capture … unchanged") start here.

## `const TRANSPARENT_STDOUT: &[&str] = &[`

What the transparency shim prints on stdout: JSON Lines, the shape whose *first* lines carry the
meaning. A Codex transcript's `thread.started` (the session) and `item.completed` (the verdict)
both precede the final `turn.completed`.

## `const TRANSPARENT_STDERR: &[&str] = &["tracing line one", "tracing line two"];`

And on stderr, captured by the same funnel, where a Codex run puts its tracing log.

## `fn transparency_shim(dir: &Path, name: &str) -> String {`

A launcher that ignores its arguments and prints [`TRANSPARENT_STDOUT`] and [`TRANSPARENT_STDERR`].
Not [`forwarding_shim`], whose child is this test binary with whatever `libtest` prints: here the
test chooses the child's output, so what the child produced and what the runner returned can be
compared.

Every payload byte is `echo`-safe in both dialects: JSON carries none of `&`, `<`, `>`, `|`, `^`,
and `cmd`'s `echo` prints a `"` literally.

**The redirection goes first on Windows.** `cmd`'s `echo` prints everything between the command and
the redirection operator, so `echo foo 1>&2` emits `foo` and a trailing space; `1>&2 echo foo` does
not. A test that trimmed instead could not see a runner that trimmed.

## `fn captured_lines(stream: &str) -> Vec<String> {`

A captured stream as lines, with the platform's terminator folded away.

## `fn transparency_run(agent: &str, resume: Option<&str>) -> crate::agent::TaskRun {`

One `TaskRun`, so each adapter's own `build_args` can be asked for production's argument vector.

## `fn the_runner_returns_the_childs_whole_output_for_every_production_request_shape() {`

`HostRunner::run` hands back **the child's whole output** for every request shape production sends
(`invariants_preserved[0]`: "process supervision, timeout, **output capture**, adapter parsing
unchanged"). A runner that kept only the last stdout line for, say, a codex `exec` review would lose
the session and the verdict, so the review would be re-asked and could end as `ReviewFailed`.

Three axes, varied independently, because a suppression can key on any of them:

* **role**, built by production's own builder;
* **agent binding**, all three shipped ids;
* **the argument vector**, each adapter's real one from its own `pub fn build_args`, so `exec`, `-p`
  and the bare-prompt form all appear. The other grids in this file send `["--exact",
  NO_SUCH_TEST]`, where an `args[0]`-keyed suppression has nowhere to fail.

The resumed shape is carried too: `codex exec resume <id>` moves the subcommand's position, so a
check on `args[0]` and one on "is this an exec" are different predicates.

## `fn the_runner_returns_the_childs_whole_output_for_every_production_request_shape() {` › `for adapter in crate::agent::ADAPTERS {`

The probe role too: pre-flight reads a CLI's own answer, and a truncated one is a capability read
wrong.

## `fn the_runner_returns_the_childs_whole_output_for_every_production_request_shape() {` › `let script = if cfg!(windows) {`

And the unbound role, whose program is the recorded shell rather than an agent CLI: the other half
of the program-shape partition.

## `fn the_runner_returns_the_childs_whole_output_for_every_production_request_shape() {` › `TRANSPARENT_STDERR`

Redirection first, for the trailing-space reason on `transparency_shim`.

## `fn the_runner_returns_the_childs_whole_output_for_every_production_request_shape() {` › `assert_eq!(`

Hostility as counts.

## `fn build_args_for(id: &str, resume: Option<&str>) -> Vec<String> {`

Each adapter's production argument vector, from the adapter itself.

## `fn assert_transparent(cell: &str, output: &ProcessOutput) {`

Every line the child wrote came back, in order, on the stream it was written to.

## `fn assert_transparent(cell: &str, output: &ProcessOutput)` › `assert!(`

Named separately, because "the last line survived" is the assertion a truncating runner would still
pass.

## `fn the_runner_bounds_output_at_the_same_allowance_the_direct_funnel_does() {`

The bounded capture allowance is the same on the direct funnel entry and through `HostRunner`, and
it is the real one. `HostRunner` reaches the funnel through `proc::run_with_timeout_classified`, and
the proc tests drive the private `run_with_timeout_and_limit` with limits of their own, so only a
test like this one sees the production constant. It drives that constant through both paths with a
child that writes far past it.

The expected bound is written here (16 MiB per stream) rather than read from `proc`, so changing the
constant fails here rather than agreeing with itself.

## `fn the_runner_bounds_output_at_the_same_allowance_the_direct_funnel_does() {` › `const EXPECTED_LIMIT: usize = 16 * 1024 * 1024;`

`proc`'s `OUTPUT_LIMIT_BYTES`, transcribed. Per stream.

## `fn the_runner_bounds_output_at_the_same_allowance_the_direct_funnel_does() {` › `const HELPER_BUDGET: usize = 64 * 1024 * 1024;`

How much the helper writes before exiting, as a decimal byte count: four times the allowance, and
**finite**. A funnel that bounds kills the child while it is blocked on a full pipe, far short of
this. A funnel that does not bound captures 64 MiB and the child exits 0, which fails
`output_limited` below by name instead of running the parent out of memory. The `const` assertion
keeps the budget above `EXPECTED_LIMIT`.

## `fn the_runner_bounds_output_at_the_same_allowance_the_direct_funnel_does() {` › `let mut direct = Command::new(&exe);`

(a) The direct funnel entry.

## `fn the_runner_bounds_output_at_the_same_allowance_the_direct_funnel_does() {` › `let workspace = scratch("output-limit");`

(b) The same child through the Runner.

## `fn the_runner_bounds_output_at_the_same_allowance_the_direct_funnel_does() {` › `env: vec![(`

Not a reserved key: the helper reads it to know it is the helper rather than an ordinary test run,
and to size its output.

## `fn the_runner_bounds_output_at_the_same_allowance_the_direct_funnel_does() {` › `assert!(`

And it had to *reach* the allowance: a limit of zero would also satisfy the bound above.

## `fn the_runner_executes_in_the_requested_workspace()` › `let template = shell.command("echo here > marker.txt");`

A relative redirect target resolves against the child's cwd in both shells, which is the point of
the fixture.

## `fn a_gate_child_is_told_where_no_agents_credentials_live() {`

The role scoping is in the child's own environment block, not only in the vector the runner
assembled. A gate is repository-controlled code, which no agent permission surface bounds, and the
credential location is scoped by role (`design/26_design_merge_queue_protocol.md:462-463`: "A gate
or shell probe gets none of the three"). This runs a real process under a base that carries all
three locations and asks the child what it received.

## `fn a_gate_child_is_told_where_no_agents_credentials_live()` › `let runner = HostRunner::new().with_environment(HostEnvironment::with_base(`

A base that carries every credential location, so "absent from the child" cannot be an artefact of
this machine.

## `fn a_gate_child_is_told_where_no_agents_credentials_live()` › `.filter(|(name, _)| !KeyCase::current().same_key(name, OsStr::new("PATH")))`

The shell must still be findable, so the real `PATH` wins over the synthetic one.

## `fn a_gate_child_is_told_where_no_agents_credentials_live()` › `for value in [`

Windows `echo` prints the literal `%VAR%` when the variable is unset; `sh` prints nothing. Both mean
"not set to a credential directory", and neither is the value the base carries.

## `fn a_gate_child_is_told_where_no_agents_credentials_live()` › `let worker = runner`

The same base, the role that runs codex's CLI: its own location arrives and the other two still do
not. Without this half the assertion above would be satisfied by supplying nothing at all.

## `struct StubRunner(Box<dyn Fn() -> Result<ProcessOutput, UpstrokeError> + Send + Sync>);`

The shell probe tests start here.

## `struct StubRunner(Box<dyn Fn() -> Result<ProcessOutput, UpstrokeError> + Send + Sync>);`

A runner that never spawns, so the probe's *classification* of a failure can be tested on a machine
where every `ShellKind` is installed.

## `fn a_shell_probe_that_did_not_exit_zero_is_a_preflight_error_however_it_failed() {`

Every way a `ProcessOutput` can say the probe did not succeed. `expected_failures_refusals[3]` is "a
failing shell probe -> returned pre-flight error to the caller", and the funnel reports failure
three independent ways: the exit code, `timed_out` and `output_limited`. An output-limited run can
still carry `code: Some(0)`, because the limit is observed during the final drain, and a
signal-killed child reports `code: None` with `timed_out: false`, so a probe that reads only one
field certifies a shell that did not run `exit 0`. The grid is written from the three fields rather
than from `run_shell_probe`'s branches, so a field it stops reading fails here.

## `fn a_shell_probe_that_did_not_exit_zero_is_a_preflight_error_however_it_failed() {` › `name: "killed, with no exit code and no timeout",`

Killed by a signal, or anything else that leaves no exit code: `None` is not `Some(0)`.

## `fn a_shell_probe_that_did_not_exit_zero_is_a_preflight_error_however_it_failed() {` › `name: "output-limited after exiting zero",`

The bounded-output contract terminated the owned tree; the funnel can still report the code the
leader exited with, because the limit is observed during the final drain.

## `const MISSING_SHELL: ShellKind = ShellKind::Pwsh;`

The recorded shell the missing-shell case probes for. `pwsh` because it is the one [`ShellKind`] not
reachable from any directory a program search consults once `PATH` is replaced: `cmd` is always in
the Windows system directory, `powershell` is in a subdirectory of it on every Windows `PATH`, and
`sh` and `bash` are `/bin` programs on Unix and Git-for-Windows programs on the runners. PowerShell 7
installs outside all of those, and the helper asserts that rather than assuming it.

## `const MISSING_SHELL_MARKER: &str = "UPSTROKE_MISSING_SHELL_PROBE";`

Set by the parent test on the helper it spawns, so the helper is inert when `cargo test --
--ignored` runs it directly.

## `const MISSING_SHELL_OK: &str = "<<MISSING-SHELL-REFUSED";`

Printed by the helper after it has asserted, so the parent can tell "the helper ran and refused"
from "the helper never ran".

## `fn windows_program_search_dirs() -> Vec<PathBuf> {`

The directories std's search for a bare Windows program name consults **besides** `PATH`: the
application directory, the system directory and the Windows directory (std's Windows
`search_paths`). The helper controls `PATH` by construction; these it can only *check*, which makes
the check a premise.

## `fn shell_probe_missing_shell_helper() {`

The missing-shell half of
[`host_shell_probe_succeeds_with_recorded_shell_and_fails_when_shell_missing`], in a child process
because the `PATH` that decides the answer belongs to the **process**: `HostRunner::new()` composes
from this process's environment, and an in-process test cannot rewrite its own `PATH` without racing
every other test in the binary. So the absence is constructed in a child whose entire `PATH` is one
directory this suite created and asserts is empty.

Everything here is a **premise check followed by the claim**. If the construction stops
constructing the absence (a `PATH` that is not the empty directory, a directory that is not empty,
a `pwsh.exe` in one of the directories searched regardless), this fails on the premise and says
which, rather than passing for the wrong reason.

## `fn shell_probe_missing_shell_helper()` › `let path = std::env::var_os("PATH").expect("the parent supplies a PATH");`

Premise 1: this process's `PATH` is exactly one directory, and it is empty. A controlled `PATH`
rather than an absent one, because without `PATH` `execvp` falls back to the confstr default
`/bin:/usr/bin`, where an image may ship `/usr/bin/pwsh`.

## `fn shell_probe_missing_shell_helper()` › `for dir in windows_program_search_dirs() {`

Premise 2: the directories Windows searches whatever `PATH` says do not hold this shell either.

## `fn shell_probe_missing_shell_helper()` › `let workspace = scratch("missing-shell");`

Premise 3: the workspace **exists**. The proof test composes an existing workspace, an absent shell
and `HostRunner::shell_probe`; a probe that failed because its directory was missing would test
something else. `HostRunner::run` gives every child an absolute `current_dir`, so this separates
"the shell is not there" from "the directory is not there".

## `fn shell_probe_missing_shell_helper()` › `let error = HostRunner::new()`

The claim. `HostRunner::new()` composes from this process's environment, so the child gets the same
one-empty-directory `PATH`: production's own composition. The refusal must be the spawn failure,
not an exit code; a `pwsh` that ran and exited non-zero would mean the machine has one after all.

## `fn host_shell_probe_succeeds_with_recorded_shell_and_fails_when_shell_missing() {`

`decisions.pr_sequence[5].slice_contract.proof_tests[8]`, by name.

The contract names one test for the whole shell probe, composing three things: the **recorded
shell** succeeding, a **missing** shell failing, and both going through [`HostRunner::shell_probe`]
(the `RunnerPreflight` entry point) rather than the free [`run_shell_probe`] or `Runner::run`.
Separately-tested layers lose that composition: without the missing-shell case, a `shell_probe` body
of

```text
match run_shell_probe(self, shell, workspace.to_path_buf(), invocation) {
    Err(error) if workspace.exists() && error.to_string().contains("os error 2") => Ok(()),
    outcome => outcome,
}
```

survives every remaining case: (a) succeeds, (c) has no workspace, (d) does not use the method, and
(e) does not spawn.

## `fn host_shell_probe_succeeds_with_recorded_shell_and_fails_when_shell_missing() {` › `runner`

(a) The recorded shell, actually spawned. `gates::shell_available` is a `PATH` check; only a spawn
establishes availability.

## `fn host_shell_probe_succeeds_with_recorded_shell_and_fails_when_shell_missing() {` › `let empty_path_dir = scratch("empty-path");`

(b) A **missing** recorded shell, with (a)'s workspace still in place, through the same method. It
runs in a child because the absence has to be constructed out of the process's `PATH`; see
[`shell_probe_missing_shell_helper`], which holds the assertions. The parent requires `1 passed`,
so a filter that matched nothing fails.

## `fn host_shell_probe_succeeds_with_recorded_shell_and_fails_when_shell_missing() {` › `let absent = workspace.join(format!("absent-{}", crate::ulid::ulid()));`

(c) and (d): the other two ways the host can fail to complete a probe. Both fail because of
something this test constructs and then checks, so they fail identically on a machine with every
shell installed.

## `fn host_shell_probe_succeeds_with_recorded_shell_and_fails_when_shell_missing() {` › `let absent = workspace.join(format!("absent-{}", crate::ulid::ulid()));`

(c) The recorded shell, asked to run in a directory that is not there. `HostRunner::run` gives every
child an absolute `current_dir`, and starting a process in a missing directory is refused
everywhere this crate runs: `chdir` answers `ENOENT` on Unix (whether std reaches it through `fork`
or `posix_spawn_file_actions_addchdir_np`), and `CreateProcessW` fails with `ERROR_DIRECTORY` on
Windows. Both surface as `Err` from `Command::spawn`, and the caller must get a pre-flight error
naming the shell rather than an `Ok`.

## `fn host_shell_probe_succeeds_with_recorded_shell_and_fails_when_shell_missing() {` › `let mut request = shell_probe_request(native(), workspace.clone(), shell_probe_invocation());`

(d) The missing-program fault at the `Runner::run` layer, expressed so that no installed program can
satisfy it: an absolute path inside the directory (c) established does not exist. It contains a
path separator, so it is taken verbatim; no `PATH` search is made for it. (b) is the same fault one
layer up, through the method and against the recorded shell; this pins the layer beneath.

## `fn host_shell_probe_succeeds_with_recorded_shell_and_fails_when_shell_missing() {` › `let refusing = StubRunner(Box::new(|| Ok(stub_output(Some(127), false))));`

(e) A shell that runs and refuses, one that hangs, and one whose spawn fails. Each is a pre-flight
error, and none is a `ProcessOutput` the caller has to interpret.

## `fn the_shell_probe_spells_every_shell_the_way_gates_do() {`

The probe spells every shell exactly as `gates::ShellGate` does, so it
certifies the invocation the gates will use.

## `const WINDOWS_POINTS: &[SubEffectPoint] = &[`

The containment sub-effect hook point tests (ST-07 subset) start here.

## `const WINDOWS_POINTS: &[SubEffectPoint] = &[`

The eight points and the host each belongs to, transcribed from
`decisions.effect_site_inventory.containment_sub_effects`:

> Windows: Spawn.AmbientJobJoined …, Spawn.CreatedSuspended …,
> Spawn.PrivateJobAssigned, Spawn.Resumed … ; Unix: Spawn.ReaperStarted
> …, Spawn.PreExecPgidAndRegister …, Spawn.Exec, Spawn.Registered

## `fn observed_points(runner_hooks: HarnessHooks) -> BTreeSet<SubEffectPoint> {`

Runs one trivial command through the runner and reports which containment points its funnel
reached.

## `struct OrderedHooks(Arc<Mutex<Vec<SubEffectPoint>>>);`

Records the points a spawn reaches **in the order it reaches them**.

## `fn point_order() -> Vec<SubEffectPoint> {`

The containment points of one spawn, in order.

## `fn the_containment_points_of_a_spawn_fire_in_the_packets_order() {`

The points fire in the packet's order, so each label sits at the coordinate it names.
`containment_sub_effects` lists them as a sequence (quoted on `WINDOWS_POINTS`), and the order is
the whole content of two of them: `PrivateJobAssigned` promises the child is *still suspended* and
`Resumed` promises it is not. A set cannot tell those apart, so swapping the two labels in
production would leave the ST-07 evidence claiming a pre-resume coordinate for a hook that fires
after the child can execute. The expected sequence is transcribed from the packet, not read back
from the funnel.

## `fn the_containment_points_of_a_spawn_fire_in_the_packets_order() {` › `vec![`

`AmbientJobJoined` is a write-command startup step rather than a per-spawn one, so a spawn reaches
the other three.

## `fn the_containment_points_of_a_spawn_fire_in_the_packets_order() {` › `assert_eq!(`

Each point once: a point consulted twice in one spawn would make the sequence ambiguous about which
coordinate it names.

## `fn windows_containment_points_execute_and_unix_points_do_not() {` › `for point in &[`

`AmbientJobJoined` is a write-command startup step, so running a command does not reach it; it is
exercised by `windows_ambient_job_unavailable_refuses_before_effects` and by the coordinator-death
tests.

## `fn the_two_points_whose_operation_is_not_parent_side_are_named_and_counted() {`

Where each containment point's **operation** happens, and where its **injection** is controlled.
Transcribed from `decisions.effect_site_inventory.containment_sub_effects`, which annotates one
point's coordinate ("Spawn.PreExecPgidAndRegister (in the child before exec)") and one other as
parent-side ("Spawn.Registered (parent-side registration)").

Two of the eight operate outside the parent, and both are injected in the parent, for two reasons:

* their only declared mode is `Kill`, a **coordinator** death, and the coordinator is the parent. A
  kill inside the forked child would end the fork, not the coordinator, and the packet's claim for
  these points ("a coordinator kill after any of these leaves a group the reaper settles while
  holding R28") needs a group that exists;
* an observer cannot run between `fork` and `exec` at all: only async-signal-safe calls are
  permitted there, and every real observer locks and allocates.

The packet allows both ("ST-07 evidence executes each point on its platform (these are parent-side
**or** pre-exec points the harness controls)"), and `InjectionMode` is "how a fault is introduced at
a **parent-side** sub-effect point". The count is asserted so the boundary cannot grow: a third
point that stops executing in the parent fails this test.

## `enum Coordinate` › `Parent,`

In the coordinator process.

## `enum Coordinate` › `ForkedChildBeforeExec,`

In the forked child, before `exec`.

## `enum Coordinate` › `Child,`

In the child, at `exec` itself.

## `fn the_two_points_whose_operation_is_not_parent_side_are_named_and_counted() {` › `let table: &[(SubEffectPoint, Coordinate, Coordinate)] = &[`

(point, where the operation happens, where the injection is controlled)

## `fn the_two_points_whose_operation_is_not_parent_side_are_named_and_counted() {` › `for point in &elsewhere {`

The reason, asserted: both declare `Kill` alone, and `Kill` is a coordinator death. The expected
modes are the packet's split: "Kill is universal … error-return is narrower … Windows'
AmbientJobJoined has one".

## `fn containment_points() -> Vec<SubEffectPoint> {`

The containment points, observed at runtime on **every** role, start here.

## `fn containment_points() -> Vec<SubEffectPoint> {`

**Every** containment point this host declares, read out of the frozen inventory rather than
transcribed from it. `SPAWN_SITE.sub_effects()` is `Process.Spawn`'s own list
(`src/topology/effects/sites.rs`) and `SubEffectPoint::platform()` is each point's host
(`src/topology/effects/vocab.rs`), so a point added to the site is in this domain the moment it
exists. A hand-written list can omit a point, and a grid over it then claims coverage of a point
it never executed.

## `fn per_spawn_points() -> Vec<SubEffectPoint> {`

The containment points one **spawn** reaches on this platform: every point of
[`containment_points`] that is not a startup step.

`AmbientJobJoined` is a write-command *startup* step, reached by
[`HostRunner::start_write_command`] rather than by running a command. It is role-free by
construction (the ambient job is a property of the process, established once before anything can
spawn), and is witnessed for error-return by `windows_ambient_job_unavailable_refuses_before_effects`
and for kill by `a_kill_armed_at_any_containment_point_actually_kills`, which iterates the *whole*
domain rather than this subset.

## `const STARTUP_POINTS: &[SubEffectPoint] = &[SubEffectPoint::AmbientJobJoined];`

The containment points a write command reaches at **startup**, before it has spawned anything.

## `fn the_startup_and_per_spawn_domains_partition_this_platforms_points() {`

The two domains partition the platform's four points: the startup domain is `AmbientJobJoined` on
Windows and empty on Unix. The partition is what makes `per_spawn_points` safe to derive by
subtraction: a point added to `Process.Spawn` lands in one of the two by construction, never in
neither. It is asserted against `containment_points`, itself derived from the inventory, so the
only way to lose a point from both is to delete it from `src/topology/effects/vocab.rs`.

## `fn the_startup_and_per_spawn_domains_partition_this_platforms_points() {` › `let expected: &[SubEffectPoint] = if cfg!(windows) {`

Derived, not transcribed: the domain agrees with the packet's platform split for this host.

## `fn shell_command() -> CommandSpec {`

The child of a role whose production program is **the recorded shell**: the gate
(`gates::ShellGate::check` sends `ShellKind::spec`) and the shell probe ([`shell_probe_request`]
sends the same). Built by [`ShellKind::spec`] rather than round-tripped through a `Command`, so the
program string is the one [`shell_probe_request`] carries. No overlay and no stdin, because
`gates::ShellKind::spec` writes `env: Vec::new(), stdin: Vec::new()` and `gates` asserts it.

## `const NO_SUCH_TEST: &str = "upstroke_pr4_role_grid_matches_no_test";`

A filter that matches no test in this binary, so the child below runs nothing and exits 0.

## `fn agent_cli_command(stdin: &[u8]) -> CommandSpec {`

The child of a role whose production program is **an agent CLI**: the worker, the reviewer and the
agent probe, which run the CLI `bin::Invocation::spec` names and never a shell. Production names the
CLI by its bare name and `HostRunner::run` resolves it on the composed `PATH` to an absolute path;
this test binary, at its absolute path, stands in for that resolved program, and with a filter that
matches nothing it runs no test and exits 0.

**Why the grid may not run one child for every role.** `HostRunner::run` chooses the observer it
hands the funnel, so a grid whose every child was the recorded shell would leave

```text
let selected = if is_a_shell(&request.command.program) { hooks } else { &mut NoHooks };
```

green while every real worker, reviewer and agent probe ran with no containment hooks and no fault
injection.

## `fn this_test_binary() -> String {`

This test binary's own path, as the `String` a [`CommandSpec`] carries.

## `fn agent_cli_command_at(program: &str, stdin: &[u8]) -> CommandSpec {`

[`agent_cli_command`] against an arbitrary launcher for this binary, so the *program shape* can
vary while everything else stays production's.

## `struct ProgramShape {`

One shape the program the runner spawns can take on this platform.

## `struct ProgramShape` › `what: &'static str,`

Where the shape comes from in production.

## `struct ProgramShape` › `reports: bool,`

Whether the child ends up being this test binary, so its `libtest` report is readable on stdout.
False for the recorded shell, which answers `exit 0` and prints nothing.

## `fn forwarding_shim(dir: &Path, name: &str) -> String {`

Writes a launcher into `dir` that forwards its arguments and its stdin to this test binary, and
returns its absolute path. The two spellings are the two an installer produces: on Windows npm
writes a `.cmd` (or `.bat`) batch shim, and on Unix an extensionless shebang script. Neither is a
native executable, and `CreateProcessW`/`execve` reach both only through an interpreter, which is
why a runner that treated them as a different kind of program is a defect this suite must see.

## `fn forwarding_shim(dir: &Path, name: &str) -> String` › `format!("@echo off\r\n\"{exe}\" %*\r\n")`

`@echo off` so the batch text does not reach stdout, and the target quoted because its path may
contain a space.

## `const A_DIRECTORY_WITH_A_SPACE: &str = "John Smith";`

A directory name with a space, which makes the path one std must quote. It transcribes the
production-shaped value in `src/agent/bin.rs`'s own tests: `C:\Users\John Smith\npm\copilot.cmd`.

## `fn program_shapes(root: &Path, stdin: &[u8]) -> Vec<ProgramShape> {`

Every **program shape** the runner can be asked to spawn on this platform, materialised under
`root`. There are two producers of `CommandSpec.program`: `bin::Invocation::spec`, which carries an
agent CLI's bare name, and `gates::ShellKind::spec`, which carries the recorded shell's bare name.
`HostRunner::run` resolves an agent CLI's name on the composed `PATH`, where the installation
decides what it finds: a native executable, a batch shim (`.cmd` or `.bat`, which is what
npm-installed `claude`, `codex` and `copilot` are on Windows), or on Unix a shebang script. The
fixture hands the runner those resolved shapes as absolute paths, plus the shell's bare name.

Two axes, varied independently, because a suppression can key on either: the **kind of file**
(native / batch / script / bare name) and whether the **path needs quoting** (a directory with a
space in it).

## `const WORKER_STDIN: &str = "## Task one\n\nthe materialized worker prompt, delivered on \`

What production writes to a **worker's** stdin: the materialized task prompt
(`AgentAdapter::stdin_payload`, put on the spec by `WorkerAssembly::command` in
`src/engine/assembly.rs`). Non-empty, which is the point; see [`agent_cli_command`] and the grid
below.

## `const REVIEW_STDIN: &str = "review the candidate diff and answer with the structured \`

What production writes to a **reviewer's** stdin, from the same seam in `review::run_review`. A
different payload from the worker's, so the grid carries two distinct non-empty ones.

## `const AGENT_PROBE_TIMEOUT: Duration = Duration::from_secs(60);`

Every adapter's `PROBE_TIMEOUT`, transcribed: `claude.rs`, `copilot.rs` and `codex.rs` each declare
`const PROBE_TIMEOUT: Duration = Duration::from_secs(60)`, private to their own module.

## `fn production_request(`

The request **production** sends for `role`, from production's own
builder — five roles, five builders, and this fixture writes none of
them:

* `Probe(Shell)` — [`shell_probe_request`], which is what
  [`run_shell_probe`] (and therefore [`HostRunner::shell_probe`], the
  `RunnerPreflight` entry point ordered at P4 by
  `decisions.workspace_candidates.run_creation`) builds;
* `Probe(Agent)` — [`crate::agent::probe_request`], the builder every
  adapter's `probe` calls;
* `Implement` — [`crate::runner::worker_request`], which
  `engine::attempt::run_attempt` calls;
* `Gate` — [`crate::runner::gate_request`], which `gates::ShellGate::
  check` calls;
* `Review` — [`crate::runner::review_request`], which
  `review::run_review` calls.

**The builders are only half of it.** A builder fixes the role, the binding and the identity
(production sends `agent: Some(<the adapter>)` and a worker or review identity for the in-attempt
agent roles); everything it is *handed* (program, arguments, overlay, stdin payload, timeout) is
still this fixture's choice, and each is a field `HostRunner::run` could key an observer selection
on. So each gets production's own value for that role rather than one constant shared by all five:

* **program and args**: [`agent_cli_command`] for the three roles that run an agent CLI,
  [`shell_command`] for the two that run the recorded shell;
* **stdin**: the adapter's prompt for the worker and the reviewer (`AgentAdapter::stdin_payload`,
  put on the spec by `WorkerAssembly::command` and `review::run_review`), empty for the gate and
  both probes, which is what their specs carry;
* **env**: empty for all five, because that is what each role's production spec carries:
  `ShellKind::spec` and `bin::Invocation::spec` both write `env: Vec::new()`, and no worker,
  reviewer, gate or adapter-probe call site adds an overlay entry. The one production overlay is
  the subscription reader's, on an agent probe that also carries a payload. That shape is a second
  value for the agent-probe role, not a replacement for this one, so it has its own test:
  `a_subscription_probe_with_an_overlay_and_a_payload_reaches_the_containment_points`. The census
  that finds every overlay site is
  `runner::contract::tests::every_production_command_spec_payload_is_classified`;
* **timeout**: each role's own production default, five distinct values.

## `agent_cli_command(b""),`

A probe asks a CLI about itself: an agent binary, and no prompt on stdin.

## `fn run_in_role(`

Starts one child in `role`, through the entry point production uses for that role, and waits for
it. `Probe(Shell)` goes through [`HostRunner::shell_probe`], the entry point pre-flight calls, which
adds the probe's own refusals and builds its request with [`shell_probe_request`], the value
[`production_request`] returns for that role. Every other role is [`production_request`] executed
through `run`.

## `struct RoleWitness {`

Everything one spawn's containment observers can see, for one role.

`point` delegates to [`HarnessHooks`], the production wiring onto [`HookHarness`], so the evidence
lands where ST-07 reads it, and records the order beside it, because a set cannot tell
`PrivateJobAssigned` (the child is still suspended) from `Resumed` (it is not). `child_created` is
the funnel's other observation: on Unix it asks the kernel whether the containment *operation*
happened for this role rather than only its hook, as a `GroupObservation`, so the look carries
whether the child had already exited, what `getpgid` answered and, on macOS, what the exited record
answered. The grid's children are short-lived and the look comes after `spawn` returns, so the
exited case is ordinary here (`W2-MACOS-HOST-CONTAINMENT-ROLE-GROUP-FINGERPRINT`).

## `fn group_leadership(observations: &[proc::GroupObservation]) -> (Vec<bool>, String) {`

The decisions and the rendered records side by side, so an assertion compares the former and prints
the latter. The role grid also prints the records once the assertion passes.

## `impl RoleWitness` › `fn handle(&self) -> Self {`

A second handle on the same recordings, for the runner to own.

## `fn the_role_grid_sends_the_shapes_production_sends() {`

The grid's fixtures vary every independently meaningful field independently, and the counts say
so: a grid that moved two fields together would prove only that *some* combination reaches the
funnel.

The field list is taken from the type. Of [`RunnerRequest`]'s fields and its `command`'s, this
asserts nine: role, agent binding, invocation identity, workspace, timeout, and the spec's program,
args, env and stdin. It does not vary `credential_profile` or the spec's `settings_file`. Each
asserted field is one an observer suppression in `HostRunner::run` could key on (an empty stdin,
the program, the timeout), so each must take production's values here.

Nothing asserted is read back out of the fixture: the bindings are R3's predicate (`is_slotted`),
the payload split is `AgentAdapter::stdin_payload`'s rule, the program split is "a bound process
runs its agent's CLI", and the timeouts are production's own constants, with the adapters' private
probe timeout transcribed as `AGENT_PROBE_TIMEOUT`.

## `fn the_role_grid_sends_the_shapes_production_sends()` › `let labels: BTreeSet<String> = requests.iter().map(|r| r.role.label()).collect();`

Field 1: the role. Five distinct values, one per member.

## `fn the_role_grid_sends_the_shapes_production_sends()` › `let bound = requests.iter().filter(|r| r.agent.is_some()).count();`

Field 2: the agent binding. Three bound, two not, and *which* three is R3's rule, not this
fixture's.

## `fn the_role_grid_sends_the_shapes_production_sends()` › `let ids: BTreeSet<String> = requests.iter().map(|r| r.invocation.render()).collect();`

Field 3: the identity. Five distinct renderings, and the form follows the role: the two probes
carry probe identities, the three in-attempt roles attempt identities with three distinct role
tokens.

## `fn the_role_grid_sends_the_shapes_production_sends()` › `let paired: BTreeSet<(String, bool, String)> = requests`

And the pairing, so no two fields can be swapped unnoticed.

## `fn the_role_grid_sends_the_shapes_production_sends()` › `let carrying_a_payload: BTreeSet<String> = requests`

Field 4: the stdin payload. `AgentAdapter::stdin_payload` is put on a spec only for the worker
(`WorkerAssembly::command`) and the reviewer (`review::run_review`), so exactly those two carry
bytes; a gate's spec carries none (`gates::ShellKind::spec`), and neither probe does.

## `fn the_role_grid_sends_the_shapes_production_sends()` › `let shell_program = native().spec(SHELL_PROBE_COMMAND).program;`

Field 5: the program, and with it the args. Production's two spec constructors are
`gates::ShellKind::spec` (the recorded shell) and `bin::Invocation::spec` (an agent CLI); which one
a role gets is decided by whether it runs an agent CLI.

## `fn the_role_grid_sends_the_shapes_production_sends()` › `for request in &requests {`

The program and the binding move together by production's rule, not as this fixture's shortcut: a
bound process runs its agent's CLI, an unbound one the recorded shell. Asserted, so a fixture that
broke the correspondence would have to say why.

## `fn the_role_grid_sends_the_shapes_production_sends()` › `assert_ne!(`

The payload split is *not* the binding split, so the two cannot be mistaken for one field: the
agent probe is bound and runs a CLI and still carries no prompt.

## `fn the_role_grid_sends_the_shapes_production_sends()` › `let timeouts: BTreeSet<(String, Duration)> = requests`

Field 6: the timeout. Each role's own production default: five values, all distinct, none of them
this fixture's choice.

## `fn the_role_grid_sends_the_shapes_production_sends()` › `assert!(`

Field 7: the overlay. Empty for all five roles, because both spec constructors write
`env: Vec::new()` and no role's own call site adds an entry. Production now carries one overlay:
the subscription reader's empty reset-bearer binding, on an agent probe with a stdin payload
(the `src/tracker/subscription.rs` row of
`runner::contract::tests::every_production_command_spec_payload_is_classified`, the tripwire).
That made the overlay a varying dimension, and the varying value lives in
`a_subscription_probe_with_an_overlay_and_a_payload_reaches_the_containment_points`, not in this
grid. That test sends the grid's own agent probe with both payload fields filled through the same
observers and faults. Folding the shape into this grid instead would give the agent-probe role two
payloads, and that would break the payload and binding split asserted above. This assertion still
holds for the five-role grid.

## `fn the_role_grid_sends_the_shapes_production_sends()` › `let workspaces: BTreeSet<&Path> = requests.iter().map(|r| r.workspace.as_path()).collect();`

Field 8: the workspace. Two distinct values, and which is which is production's: the agent probe
runs in the coordinator's directory (`agent::probe_request` chooses `agent::probe_workspace`), and
every other role, the shell probe included, runs in the workspace it is given. Every workspace is
absolute, because `HostRunner::run` clears the environment and a drive-relative path would resolve
against nothing.

## `fn every_production_invocation_identity_reaches_the_containment_points() {`

The **identity** dimension, over the whole space production builds rather than the five the role
grid carries (`worker`, `gate0`, `review_pass0`, and the two probes at ordinal 0). Every identity
is a value `HostRunner::run` could key an observer selection on. Enumerated from `AttemptRole`'s
variants and the call sites that build them:

* `AttemptRole::ReviewReask(n)`: `review::run_review`'s re-ask, a second reviewer process inside one
  pass, which the five-role grid has no slot for;
* non-zero role indices: gates are numbered by position (`Gate(index)`) and review passes by pass
  (`ReviewPass(pass)`), so the grid's `0` is the first of several;
* non-zero probe ordinals: each adapter fixes one per pre-flight step (claude's `probe_ordinal`:
  version 0, help 1, auth status 2).

The `Attempt` form's own `ordinal` is always 0 in production (`engine::attempt::AttemptCx::invocation`
passes 0), so the test does not vary it.

## `fn every_production_invocation_identity_reaches_the_containment_points() {` › `let grid: BTreeSet<String> = ExecutionRole::all()`

Every identity here renders differently from every identity the role grid sends, or this test would
re-prove the grid.

## `fn every_shipped_agent_binding_reaches_the_containment_points() {`

The **agent binding** dimension, over every shipped adapter rather than the one the role grid
names. `ExecutionRole::all` and `fixture_agent` bind only `claude-code`, so the rest of the
containment grid runs on one of the three shipped ids. `host-v1` already branches on `agent`
(`HostEnvironment::compose` gives each agent its own credential location), so `if request.agent ==
Some(AgentId::new("copilot")) { NoHooks }` would run a third of real runs unobserved and leave the
five-role grid green.

The roster is [`CREDENTIAL_LOCATIONS`]' own, so a fourth adapter appears in this test the moment
`host-v1` learns where its credentials live.

## `fn every_production_program_shape_reaches_the_containment_points() {`

The **program shape** dimension, over every shape the runner can be asked to spawn rather than the
one the role grid carries. Every agent role in the five-role grid runs `std::env::current_exe()`, a
native `.exe` on Windows, and `agent::bin::tests::a_batch_shim_runs_and_receives_its_argument` runs
its `.cmd` with `build_command(&spec).output()`, bypassing `HostRunner` and its hooks. Before this
test existed,

```text
let mut no_hooks = NoHooks;
… if request.command.program.to_ascii_lowercase().ends_with(".cmd") {
       &mut no_hooks as &mut dyn SpawnHooks
   } else { &mut **hooks }
```

stayed green across the suite while every npm-installed Windows agent CLI (`claude.cmd`,
`codex.cmd`, `copilot.cmd`) ran with no containment observation and no fault injection.
`the_cli_roles_of_the_grid_run_a_shim_shaped_program_through_the_funnel` now covers the same
mutation from the role side.

The same two claims as every other axis in this file, that the points are reached and that the
observer's answer is honoured, so a shape that is *observed* but not *injectable* fails too.

## `fn every_production_program_shape_reaches_the_containment_points() {` › `assert_eq!(`

The axes, as counts, so the list cannot shrink in silence.

## `fn every_production_program_shape_reaches_the_containment_points() {` › `let mut injections = 0_usize;`

And the observer's answer is honoured for every shape, at every point: observation and injection
are two claims.

## `fn the_cli_roles_of_the_grid_run_a_shim_shaped_program_through_the_funnel() {`

Every role that runs an agent CLI (implement, review, `probe(claude-code)`), each built by its own
production builder, carries a batch-shim program through the funnel to every per-spawn containment
point. `every_production_program_shape_reaches_the_containment_points` varies the shape against one
role; this varies the role, each role carrying the same batch shim (a `.cmd` on Windows), so a
suppression keyed on a CLI role with that shim, such as a `.cmd` in the reviewer's hands, fails
here. A pair neither grid crosses, such as a reviewer's `.bat`, is not covered by either.
`gate` and `probe(shell)` are left out because production runs the recorded shell for them
(`the_role_grid_sends_the_shapes_production_sends`); the case count is asserted against that rule.

## `fn the_cli_roles_of_the_grid_run_a_shim_shaped_program_through_the_funnel() {` › `std::fs::create_dir_all(&request.workspace).expect("the request's workspace");`

A probe chooses its own workspace, so the run happens where the request says.

## `fn the_grids_agent_cli_child_runs_no_test_and_exits_zero() {`

`agent_cli_command` executes this test binary with a filter that must match no test. A filter that
matched would run tests inside the grid's fixtures; one that stopped exiting 0 would fail the grid
for a reason unrelated to containment. The grid reads only the exit code, so this reads libtest's
own count, which must be `0 passed`, and requires the child to have settled.

## `fn every_role_reaches_the_containment_points_of_this_platform() {`

Every role's spawn, not only the gate's, is observed at the containment points. The packet's
`scope` sends probes, workers, gates and reviews through the runner's funnel with containment hook
points, and `proof_tests[3]` asks for containment sub-effect hook tests (ST-07 subset). The probes
matter most: `decisions.workspace_candidates.run_creation` orders P4 `RunnerPreflight` before P6
`run_started`, so their spawns are the prefixes ST-07 evidence over `Process.Spawn` is read as
covering.

Three observations per role: the points reached, their order, and on Unix the kernel's answer that
the containment operation ran for this child. A funnel that fired the hooks but skipped the
operation passes the first and fails the third.

`runner::contract::tests::the_spawn_site_files_every_role_under_one_context_and_the_count_says_which`
does not discharge this: counting that two roles fall outside the site's declared context proves
the mismatch exists, not that the hooks run for them. This is the runtime proof, asserted for all
five roles because a suppression keyed on any one role is the same defect.

## `fn every_role_reaches_the_containment_points_of_this_platform() {` › `assert_eq!(`

The same points in the packet's order, each exactly once.

## `fn every_role_reaches_the_containment_points_of_this_platform() {` › `assert_eq!(`

One spawn, one child: the funnel's `child_created` observation, which a fault injected there would
need.

## `fn every_role_reaches_the_containment_points_of_this_platform() {` › `assert_eq!(`

Unix: the containment operation, not only its hook, ran for this role; the witness is the kernel.
`GroupObservation::leads_own_group` also accepts, on macOS, an exited, unreaped child whose
`proc_pidinfo` zombie record names its own pid as group and has status `SZOMB`
([`agent/proc` notes](../../agent/proc.md)).

A passing role prints its record to stderr after the assertion. On macOS the record carries what
`proc_pidinfo`, asked for an exited record, answered, so a record can show whether that call answers
for a live process. That question is still open: the `agent/proc` notes read the answer from XNU's
source, not from a printed record.

`before the look running` alone does not make a record answer it. It is the first `waitid` poll,
taken before `getpgid` and `proc_pidinfo`; `observe_child_group` makes each observation in a
separate call and the child can exit between any two, so a child seen running can have `getpgid`
answer its own pid, exit, and then have `proc_pidinfo` return its zombie's record. Check two more
fields:

- the status `proc_pidinfo` answered: `status 5` is `SZOMB`, a zombie's record, which says nothing
  about a live process;
- `after the look`, the poll taken after `proc_pidinfo` returned: `running` means the child had not
  exited by the end of the call; `exited, unreaped` beside `before the look running` means it exited
  between the polls, before or after the call.

Only a non-`SZOMB` status with `after the look running` can show the call answering for a live
child, and even that leaves one case open: a child already exiting, which `proc_find_zombref` also
finds ([`agent/proc` notes](../../agent/proc.md)), but not yet waitable at the second poll. A call
failing with `No such process` with both polls `running` is the other answer. The grid's children
are short-lived, so a run may hold no record that answers either way.

libtest shows a passing test's output only under `--nocapture` or `--show-output`, which CI's suite
step does not pass; the measurement is a run of this test with one of them on the macOS runner,
read by the checks above, possibly more than once.

## `const SPAWN_KILL_POINT: &str = "UPSTROKE_SPAWN_KILL_POINT";`

-----------------------------------------------------------------------
Kill mode, actually executed
-----------------------------------------------------------------------

## `const SPAWN_KILL_POINT: &str = "UPSTROKE_SPAWN_KILL_POINT";`

Which containment point the kill helper is to die at.

## `struct KillAtPoint(SubEffectPoint);`

A hook that kills the funnel at one named point and nowhere else.

## `impl SpawnHooks for KillAtPoint` › `fn point_mode(`

A point consulted at two coordinates is killed only at its kill coordinate. `Spawn.AmbientJobJoined`
is consulted for its error return before the join and for its kill after it. The inherited default
answers `point()` to both, so a hook armed for a kill would abort before there is an ambient handle
to close: the grid would still see an abort and pass while witnessing a coordinate the packet does
not name, the false-witness shape of `PR5-RD-002`.

## `fn spawn_funnel_kill_helper() {`

The child half of `a_kill_armed_at_any_containment_point_actually_kills`.

The kill is `std::process::abort`, as `apply` in `src/agent/proc/hooks.rs` does, because the claim is
what a coordinator that dies without running any cleanup leaves. `panic!` unwinds and runs
destructors, including the one that closes the job handle whose close-on-death is the mechanism;
`std::process::exit` runs no stack destructors but does run the platform's process-exit handlers;
`abort` runs neither. So the kill needs a process of its own.

It establishes containment first. On Windows a kill at `CreatedSuspended` leaves a suspended stub by
construction (the state INV-18 exists for), and only the ambient job's handle closing when this
process dies reaps it; without the step each point would leak a suspended `cmd.exe` onto the machine
running the suite. On Unix the per-invocation reaper settles the group.

A startup point (`STARTUP_POINTS`) is armed on the startup call itself, because that call is the only
place it is consulted; establishing containment with `NoHooks` first would leave it unreachable in
kill mode (`PR5-RD-002`).

## `fn spawn_funnel_kill_helper()` › `let _ = start_write_command(&mut KillAtPoint(point));`

The kill coordinate is inside this call, after the real ambient join. Returning from it means the
kill never fired, and the parent reads the clean exit that follows as exactly that.

## `fn spawn_funnel_kill_helper()` › `let _ = runner.run_blocking(&crate::runner::gate_request(`

Every per-spawn point is reached by an ordinary spawn
(`every_role_reaches_the_containment_points_of_this_platform`); the gate role is the cheapest.

## `fn spawn_funnel_kill_helper()` › `std::process::exit(0);`

Reached only if the kill did not fire, which the parent detects as a clean exit.

## `fn a_kill_armed_at_any_containment_point_actually_kills() {`

`Injection::Kill` aborts at every containment point that declares the mode on this platform.
`decisions.effect_site_inventory.scope` requires every parent-side sub-effect point to be observed
executed in every injection mode it supports, and every containment point declares `Kill`
(`SubEffectPoint::modes`). The reach tests arm nothing and the fault grid injects `Injection::Error`,
because an abort would take the test binary with it, so this grid is what executes the kill mode
behind ST-07's kill claims about `Process.Spawn`.

Same idiom as `events::log::tests::a_kill_at_each_append_point_leaves_the_shape_the_packet_tables`:
a subprocess helper, and the death checked, not assumed — not a clean exit, no `panicked at` on
stderr, and on Unix `SIGABRT` rather than another way of dying.

The domain is `containment_points()`, every point this platform declares, read from
`Process.Spawn`'s `sub_effects()` and each point's `platform()`, so a later point is covered by
construction; a hand-written list once omitted `AmbientJobJoined` (`PR5-RD-002`). Both appliers are
inside it: on Unix all four points go through `apply`; on Windows `AmbientJobJoined` goes through
`apply` and the three per-spawn points through `apply_io`.

The helper's output goes to files and the test waits on the process, not its pipes:
`Command::output()` returns when the pipe write ends close, and on Windows `CreateProcessW` inherits
handles, so the grandchild the helper leaves suspended by design would hold the pipe and `output()`
would block for ever.

## `fn a_kill_armed_at_any_containment_point_actually_kills()` › `assert_eq!(`

Four points on both platforms, as the frozen inventory declares, so a shrunken domain fails rather
than passing vacuously.

## `struct FailAt(SubEffectPoint);`

A hook that fails the funnel at one named point and nowhere else.

## `fn a_fault_armed_at_any_containment_point_stops_any_role() {`

The hooks' answer is honoured, not merely asked: every role is fault-injectable at every per-spawn
containment point. Observation and injection are separate claims and ST-07 spends the second; a
funnel that ignored the observer's answer would satisfy
`every_role_reaches_the_containment_points_of_this_platform` and inject nothing. The armed point is
named in the caller's failure, so a funnel that collapsed the points into one arming site fails.

`Injection::Error`, because `Kill` aborts the process; the kill mode is exercised in subprocesses by
`a_kill_armed_at_any_containment_point_actually_kills` and the Windows coordinator-death tests. The
packet gives only `AmbientJobJoined` an error contract, so an `Error` at these points comes only from
an observer, which is what a fault injection is, and `apply`/`apply_io` surface it.

## `const OVERLAID_TO_EMPTY: &str = "UPSTROKE_SYNTHETIC_BLANKED_BEARER";`

A synthetic variable name, not the reset bearer's real one. This file may not name the module that
defines that constant (`tracker::tests::only_the_cli_reaches_the_tracker` reads test files too). The
shape is what the grid has to carry, not the name: one overlay entry whose value is empty, so the
child sees an empty value where the runner's base had a non-empty one.

## `fn overlaid_probe_request() -> Result<RunnerRequest, UpstrokeError> {`

The payload shape of the subscription reader's native route, `tracker::subscription::read_cli`. It
is the grid's own agent probe with both payload fields filled: one overlay entry set to empty, and a
one-line stdin payload. It is built by the same production builder, `crate::agent::probe_request`,
for the same adapter and ordinal. The reader's timeout (`tracker::transport::PROCESS_TIMEOUT`) is
not copied here. The role grid already varies the timeout over five distinct values, and keeping
the grid's `AGENT_PROBE_TIMEOUT` makes the payload the only difference from the grid's request. The
reader's exact bytes, overlay, role, binding and identity are pinned at the source by
`tracker::tests::subscription_cli_is_one_exact_probe_with_its_profile_and_no_bearer`, over a
recording Runner, with no process.

## `fn inheriting_the_overlaid_key() -> HostEnvironment {`

The process environment, plus a synthetic non-empty value under the overlaid name. Without that
value, the overlay's empty value would replace nothing, and the composition check below would pass
even if the overlay had never been applied.

## `fn a_subscription_probe_with_an_overlay_and_a_payload_reaches_the_containment_points() {`

The combined shape from the `src/tracker/subscription.rs` row of
`runner::contract::tests::every_production_command_spec_payload_is_classified`. That row is the
first production spec with an overlay, and so the first with an overlay and a stdin payload
together. The tracker transport's `curl` probe carries a payload alone; its row says why no role
grid varies it. The role grid sends neither field on a probe: its overlay is empty for all five
roles, and its agent probe carries no payload. An observer suppression keyed on "a probe with a
payload", or on "an overlay", would therefore be invisible to the grid (`PR4-CONF-006`). This
test closes that gap, in four parts:

* **Shape.** The request equals the role grid's agent probe in every request and spec field except
  `env` and `stdin`. So it adds exactly the two census columns to a shape the grid already covers.
* **Composition.** `HostRunner::child_environment` is the vector `supervise` passes to the child
  after `env_clear`. It carries the overlay's empty value, not the inherited one. A probe role
  starts in the started checkout, so `give_work_tree_context` never edits that vector.
* **Observation.** It runs through the funnel under `RoleWitness`, the same observers and
  assertions as `every_role_reaches_the_containment_points_of_this_platform`: every per-spawn
  point reached, in the funnel's order, one child, and on Unix a child that leads its own group.
* **Injection.** It runs once per point under `FailAt`, as in
  `a_fault_armed_at_any_containment_point_stops_any_role`. The failure names the armed point, and a
  count keeps the shape from dropping out of the grid unnoticed.

The child is the grid's agent CLI (this test binary, running no test). It is never `claude`, and
nothing here reads a credential or reaches a provider.

## `fn the_pre_exec_containment_step_runs_in_the_forked_child() {`

`Spawn.PreExecPgidAndRegister`'s operation ran in the forked child, and the funnel reached the four
Unix points in the packet's order. The witness is the kernel: `getpgid(pid) == pid` exactly when the
closure's `setpgid(0, 0)` ran, asked at `child_created`, the first instant the parent knows the pid.

## `fn a_containment_proof_exists_only_where_containment_was_established() {`

A containment proof exists exactly when containment was established. `Contained` is what the
engine's write coordinator requires and `containment_establishments` is what an entry-point census
reads, so both are worth something only if a token cannot appear without the step. The token's own
constructor increments the count, so this asserts the constructor is reachable no other way,
including the refusal path, which returns before it.

Establishing twice must work: the CLI establishes containment at dispatch and the engine establishes
it again. The ambient job is a process-wide singleton (`join_ambient` memoises), so the second call
joins nothing new, still returns a proof and is counted, and on Windows the process is still a member
afterwards.

## `fn a_containment_proof_exists_only_where_containment_was_established() {` › `let _first = contain_write_command(&mut NoHooks).expect("containment establishes on this host");`

The tokens are held: two live proofs at once is what a coordinator entered from an already-contained
CLI has.

## `fn a_containment_proof_exists_only_where_containment_was_established() {` › `{`

A refused establishment mints nothing. Only Windows can refuse; on Unix the step is the reaper and
the isolated process group, and nothing can fail.

## `struct RefuseAmbientJoin {`

-----------------------------------------------------------------------
Windows ambient job (INV-18)
-----------------------------------------------------------------------

## `struct RefuseAmbientJoin {`

An observer that refuses the ambient join, for
`windows_ambient_job_unavailable_refuses_before_effects`.

## `fn windows_ambient_job_unavailable_refuses_before_effects()` › `struct Reporting {`

The observer must be readable after the run and `with_hooks` takes ownership, so it reports through
a channel.

## `fn windows_ambient_job_unavailable_refuses_before_effects()` › `assert_eq!(`

The simulated failure established nothing, asserted on the per-thread, per-call count rather than on
`ambient_job_established`: that is a process-wide latch other tests in this binary legitimately set,
because the library's write coordinator establishes containment
(`engine::tests::every_public_write_coordinator_entry_point_establishes_containment`).

## `fn the_production_containment_mint_propagates_a_join_refusal_and_mints_nothing() {`

`contain_write_command` itself propagates a join refusal and mints nothing: `invariants_introduced[3]`
(INV-18 host portion, refusal before any effect if the ambient job cannot be established) and
`expected_failures_refusals[1]` (the write command refuses at startup with a diagnostic).

It is the function `engine::run_harness`, `engine::resume_harness` and `src/main.rs`'s dispatch
reach, and the only place that mints `Contained`. When this test was written, the other simulated
ambient failures went through `HostRunner::start_write_command` or a closure injected at
`run_contained`, and a mint that discarded the join's outcome and returned `Ok(Contained::new())`
left them green while every Windows run dispatched with no ambient job;
`HostRunner::start_write_command` now delegates to this mint, so
`windows_ambient_job_unavailable_refuses_before_effects` would also fail. Without the ambient job, a
coordinator killed between `CreateProcessW` and private-job assignment would then leave an ownerless
suspended stub, which the ambient job exists to prevent.

Windows only, by invariant: on Unix `proc::join_ambient_job` returns `Ok` without consulting the
observer, so a Linux cell cannot record a Windows containment point as executed, and there is no
failure to propagate. The Linux suite cannot catch that mutation and does not claim to.

## `fn the_production_containment_mint_propagates_a_join_refusal_and_mints_nothing() {` › `let mut refusing = RefuseAmbientJoin::default();`

The observer is borrowed, not owned, so it is read directly afterwards with no channel.

## `fn the_production_containment_mint_propagates_a_join_refusal_and_mints_nothing() {` › `let message = error.to_string();`

The diagnostic reaches the caller. The three fragments come from `AMBIENT_REFUSAL_PREFIX` and
`AMBIENT_REFUSAL_SIMULATED` in `src/agent/proc/ambient.rs`, named rather than matched whole: the
operator must learn that it is the ambient job, which invariant it enforces, and that nothing ran.

## `fn the_production_containment_mint_propagates_a_join_refusal_and_mints_nothing() {` › `assert_eq!(`

No effect precedes the refusal: the funnel reached the join's own coordinate and nothing past it, and
no child exists.

## `fn the_production_containment_mint_propagates_a_join_refusal_and_mints_nothing() {` › `assert_eq!(`

No proof was minted, the half a mint that discarded the refusal breaks. The count is per-thread and
per-call, unlike the process-wide `proc::ambient_job_established` latch.

## `fn the_production_containment_mint_propagates_a_join_refusal_and_mints_nothing() {` › `let _proof = contain_write_command(&mut NoHooks).expect("the real join succeeds on this host");`

The success direction, so the assertion above is about the refusal and not a function that never
mints.

## `fn the_production_containment_mint_propagates_a_join_refusal_and_mints_nothing() {` › `let mut refusing = RefuseAmbientJoin::default();`

`start_write_command` is the same step for `src/main.rs`'s dispatch, which has no proof to carry, and
it has a body of its own that could drop the refusal.

## `const JOIN_RECORD: &str = "UPSTROKE_PR4_JOIN_RECORD";`

Where the join-ordering helper writes what it saw.

## `struct WitnessAmbientJoin {`

Records, at each consultation of `Spawn.AmbientJobJoined`, whether the ambient job existed at that
instant. The singleton is set only by a successful `AssignProcessToJobObject`.

## `fn windows_the_ambient_join_is_observed_on_both_sides_of_the_join() {`

`containment_sub_effects` gives `AmbientJobJoined` an error contract (failure refuses the write
command, in place of establishing the job) and a kill claim (a coordinator kill after it leaves no
host process because the ambient handle closes): opposite sides of one operation. So the
error-return coordinate must see no job (`ErrorReturn 0`) and the kill coordinate one (`Kill 1`).
It runs in a subprocess because "not yet established" is observable once per process, and a test
that relied on running first in its binary would depend on test order.

## `const POISON_AMBIENT: &str = "UPSTROKE_POISON_AMBIENT";`

Set for the child that is to carry a real memoised ambient failure.

## `fn poisoned_ambient_helper() {`

The child half of `a_real_memoised_ambient_failure_refuses_the_write_command`. It spends the
process's one ambient cell (`AMBIENT` is a `OnceLock`), which the binary's other tests need unspent,
so it runs in a process of its own.

## `fn poisoned_ambient_helper()` › `let error = proc::join_ambient_job(&mut proc::NoHooks)`

(1) The funnel's own entry point reports the remembered failure.

## `fn poisoned_ambient_helper()` › `let before = containment_establishments();`

(2) The production mint refuses, and mints nothing.

## `fn poisoned_ambient_helper()` › `start_write_command(&mut proc::NoHooks).expect_err("the CLI write path refuses too");`

(3) And the CLI's unit-returning entry, which `src/main.rs` calls before any dispatch arm.

## `fn a_real_memoised_ambient_failure_refuses_the_write_command() {`

A real memoised ambient failure refuses the write command (`crash_reconstruction`: refuse at startup
with a diagnostic before any workspace effect, no degraded mode; `expected_failures_refusals[1]`).
An observer refusing at `Spawn.AmbientJobJoined` fires before the memo is consulted, so it cannot
show that a remembered `Err` survives `join_ambient`'s match; this carries a real one through it. An
`Err(_) => Ok(())` there would have `contain_write_command` mint `Contained` and `run`/`resume` take
workspace effects with no kill-on-close job.

Windows only, by invariant: `join_ambient_job` is a no-op on Unix with no memo to poison. The
platform-independent half, that a remembered failure comes back as that failure, is
`agent::proc::tests::a_memoised_establishment_failure_reaches_every_later_caller`.

## `const STUB_RECORD: &str = "UPSTROKE_PR4_STUB_RECORD";`

Where the coordinator helper writes the identity of the stub it created before it dies.

## `fn point(&mut self, point: SubEffectPoint) -> crate::topology::effects::Injection {` › `crate::topology::effects::Injection::Kill`

The kill hook: after `CreateProcess` and before private-job assignment. `apply_io` aborts, so no
destructor runs and only the kernel closes the ambient handle.

## `fn windows_ambient_coordinator_helper()` › `let _ = runner.run_blocking(&request);`

Aborts inside `run`, at `CreatedSuspended`.

## `fn one_crash_cycle(tag: &str) -> (u32, u64, bool) {`

Runs one coordinator-death cycle and returns the stub's identity and whether it was an ambient-job
member at creation.

## `fn naming_grid() -> Vec<ProgramNaming> {`

-----------------------------------------------------------------------
program resolution
-----------------------------------------------------------------------

## `fn naming_grid() -> Vec<ProgramNaming> {`

Both naming rules, exhaustively: the `match` makes a later variant fail to compile here rather than
silently leave every grid below, the hand-written-domain failure of `PR5-RD-002`.

## `fn composed(pairs: &[(&str, &OsStr)]) -> Vec<(OsString, OsString)> {`

A composed environment holding exactly the pairs given.

## `fn path_of(dirs: &[&Path]) -> OsString {`

`PATH` as an `OsString`, from directories.

## `fn program_file(dir: &Path, file_name: &str) -> PathBuf {`

An empty file the platform accepts as a program: on Unix the execute bit is set, because `execvp`
requires it and `ProgramNaming::Posix` therefore checks it.

## `fn unexecutable_file(dir: &Path, file_name: &str) -> PathBuf {`

A file that exists and is not a program: no execute bit. Unix only; Windows has no execute bit, so
the state cannot be constructed there.

## `const REAL_PATHEXT: &str = ".COM;.EXE;.BAT;.CMD;.VBS;.VBE;.JS;.JSE;.WSF;.WSH;.MSC;.CPL";`

The `PATHEXT` a Windows machine ships, in its order.

## `fn shim_file_name(name: &str) -> String {`

The file name npm installs a bare `name` under: a batch shim on Windows, an extensionless script on
Unix.

## `fn marker_shim(dir: &Path, file_name: &str, marker: &str) -> PathBuf {`

A runnable shim that prints `marker` and its first argument. The file name is the caller's, so a
test can vary the extension with the content fixed, and the marker is the caller's, so "a shim ran"
and "this shim ran" are different observations. `marker_shim`, `transparency_shim` and
`forwarding_shim` all publish their scripts through `write_shim`.

## `fn write_shim(path: &Path, script: &str) {`

On Unix a supervised `/bin/sh` child writes the script (`printf '%s'` with positional arguments), so
the harness never opens the inode for writing and a fork in another harness thread cannot inherit a
writer; the writer closes when that child exits, and the 60-second supervision bound fails and cleans
up a stuck one. Closing the harness's own descriptor, or renaming the inode, would not do: an
inherited copy would stay alive and the later exec would fail `ETXTBSY`. Windows writes the batch
file directly. This repairs `W2-HOST-TESTS-WRITE-THEN-EXEC-ETXTBSY` (`48f2af7a:reviews/FINDINGS.md` §43) for
the marker, transparency and forwarding fixtures.

## `pub(crate) mod inherited_writer {`

Linux only. `marker_shims_do_not_leave_writers_in_another_threads_fork` runs its helper in a process
of its own under a 180-second process-tree timeout, so the helper's held forks cannot inherit
descriptors from other tests in the outer harness.

The regular-file control opens a writer, forks from another thread, closes the original descriptor,
and requires `ETXTBSY` from executing that inode; execution succeeds once the forked holder exits.

The ownership witness runs the real `marker_shim` against a FIFO whose capacity is smaller than the
script. `F_SETPIPE_SZ` rounds a request up to the page size and returns the capacity installed (the
4096-byte request installs 65536 on a 64 KiB-page kernel), so `marker_exceeding` sizes the payload
from the returned capacity plus `MARKER_MARGIN`, not from the request. The margin is needed: the
oracle drains one byte before the fork, so the script must exceed the pipe by more than that for the
writer still to be blocked when the fork happens; a payload that fit would let the writer finish
first and the witness observe nothing. `the_shim_script_exceeds_every_installed_pipe_capacity`
checks the sizing for each page size Linux supports without needing such a kernel, and
`shim_script` is the one spelling of the expected script that it and the oracle both read.

Reading the first byte proves the writer is open; the undrained rest keeps it blocked while another
thread forks. After draining exactly the expected script and joining the writer, the reader must see
EOF while the forked holder is still alive, checked immediately before and after the EOF read. An
in-process writer would leave its descriptor in the holder and the read would give `WouldBlock`.
The path and marker hold shell metacharacters to check literal transport.

Socket readiness and release order the interleaving. Reads and writer supervision are bounded, the
holder is reaped on return or unwinding (`HeldFork`'s drop), and the outer supervisor contains a
wedged helper and its descendants. The fork safety argument and the lifetime protocol are under
`HeldFork` and the `unsafe` headings below; the source comments at those sites carry only their
first line. Linux CI executes this witness; the native Linux and macOS marker tests execute the Unix
writer, and Windows keeps its batch-shim writer.

## `fn environment_on_path(dirs: &[&Path], pathext: Option<&str>) -> HostEnvironment {`

This process's environment with `PATH` and `PATHEXT` replaced. The rest is kept because a batch shim
on Windows runs through `cmd.exe`, and a child stripped to two variables would test something else.

## `fn named_request(program: &str, argument: &str, workspace: &Path) -> RunnerRequest {`

A gate request for `program` with one argument.

## `const NAME_TABLE: &[(&str, bool, bool)] = &[`

A program string, and whether each naming rule calls it a name (to search for) rather than a
location (to use as given). Written from the two platforms' rules, not read from the code under
test. The four rows the rules disagree on are the point: on Unix a backslash and a colon are
ordinary file-name characters, and a rule that ignored its `self` would agree with itself on every
row containing only `/`.

## `("claude", true, true),`

(program, is a name under Posix, is a name under Windows)

## `fn a_program_that_names_a_location_is_used_as_given_and_only_a_name_is_searched() {`

A program that names a location is handed to `Command` as given, and only a bare name is searched
for, so an absolute `program` spawns exactly as it would without resolution. Asserted by
construction: the environment has no `PATH`, so a resolution that searched would refuse, and every
location row instead comes back byte for byte. The environment is held constant; the program shape
and naming rule vary, and the four rows on which the rules must disagree are counted, so an
`is_bare_name` that ignored its platform is caught.

## `fn path_directory_order_decides_between_installations_and_pathext_only_within_one() {`

`PATH` order decides between installations; `PATHEXT` order decides only within one directory. A
shell and `std::process::Command` answer this differently: std appends `.exe` only, so an earlier
directory's `claude.cmd` is invisible to it and a later `claude.exe` wins, while a shell, and this
runner, take the earlier directory. Both axes vary independently and the resolved files are counted
as distinct values, so swapped loops collapse the count.

Driven under `ProgramNaming::Windows` on both platforms, which is why that type exists: a Windows
rule only the Windows suite could execute would be tested by no Linux run.

## `fn path_directory_order_decides_between_installations_and_pathext_only_within_one() {` › `assert_eq!(`

`first/x.cmd`, `second/x.exe`, `both/x.com`, `both/x.cmd`, `both/x.exe`: one per way the two axes
can decide; fewer means an axis is not varying the answer.

## `fn a_windows_name_is_pathext_and_never_the_extensionless_file() {`

A Windows name resolves through `PATHEXT`, never to the extensionless file beside it. Three errors,
each with its own row: widening (an extensionless `claude` shadowing the real `claude.exe`, which
neither `CreateProcessW`, appending `.exe`, nor `cmd.exe`, appending `PATHEXT`, would run); not
reading `PATHEXT` (a hard-coded list matches the default and diverges once an operator sets one);
and treating an unusable `PATHEXT` as "no programs" rather than as a malformed variable. The
directory and file set are fixed; only `PATHEXT` and the naming rule vary.

## `fn a_windows_name_is_pathext_and_never_the_extensionless_file() {` › `for absent in [None, Some(""), Some(";;;"), Some("exe"), Some(".")] {`

The default is the platform's, in its order: `.COM` first.

## `fn a_windows_name_is_pathext_and_never_the_extensionless_file() {` › `assert_eq!(`

`PATHEXT` is read, not assumed: one naming an extension nobody ships picks that file over the `.exe`
beside it.

## `fn a_windows_name_is_pathext_and_never_the_extensionless_file() {` › `assert!(`

The widening that must not happen: `x` exists and is never chosen.

## `fn a_windows_name_is_pathext_and_never_the_extensionless_file() {` › `for pathext in [None, Some(REAL_PATHEXT), Some(".FOO")] {`

Unix has no extensions: the bare file is the only answer, and `PATHEXT` changes nothing.

## `fn a_windows_name_is_pathext_and_never_the_extensionless_file() {` › `let value = OsString::from(".FOO");`

A name carrying an extension is tried verbatim first, under a `PATHEXT` that would otherwise send it
elsewhere.

## `fn a_windows_name_is_pathext_and_never_the_extensionless_file() {` › `let shadow = root.join("shadow");`

A directory is not a program, under either rule.

## `fn a_candidate_without_the_execute_bit_is_skipped_the_way_execvp_skips_it() {`

`execvp` walks past a non-executable file to the real installation further along `PATH`, so a
resolution that stopped at the first existing file would refuse, or spawn into `EACCES`, where
`execvp` runs. Two directories, one name; the answer must be the second.

## `fn a_candidate_without_the_execute_bit_is_skipped_the_way_execvp_skips_it() {` › `assert_eq!(`

Windows has no such bit, so the same file is a program there: the two rules differ.

## `fn a_name_that_matches_nothing_is_refused_and_an_empty_path_entry_is_never_searched() {`

A name that matches nothing is refused, naming the name and the boundary, and an empty `PATH` entry
is never searched. Fail-closed, because handing the name to `Command` anyway gives a `NotFound` that
names no boundary, which on Windows an operator cannot diagnose. An empty `PATH` entry means "the
current directory" to some shells, and this runner's current directory is the workspace:
repository content, under automation.

## `fn a_name_that_matches_nothing_is_refused_and_an_empty_path_entry_is_never_searched() {` › `let mixed = path_of(&[Path::new(""), &dir]);`

The empty entry is skipped, not treated as "here", and a real directory beside it is still searched;
the count is the observable.

## `fn a_bare_name_that_only_pathext_resolves_runs_through_the_host_runner() {`

A bare name that only `PATHEXT` resolves, the npm-installed agent CLI, is executed by the host
runner. `CreateProcessW` and Rust's `Command` append `.exe` and nothing else, so without the
runner's resolution a `CommandSpec.program` of `claude` with `claude.cmd` on `PATH` fails with
`NotFound` for every role. Fixtures that use an absolute `.cmd` path test a different property.

The platform fact is executed here, not cited: the same bare name goes to `std::process::Command`
under the same composed environment first and must fail with `NotFound`, or the claim would pass
where the problem does not exist. Two shims, extensions, markers and arguments, counted as distinct
values so a fixture that ran one shim twice is caught; the `PATH` directory, the runner and the
composed environment are fixed.

## `fn a_bare_name_that_only_pathext_resolves_runs_through_the_host_runner() {` › `let stem = format!("upstroke-d1-{}", crate::ulid::ulid());`

Unique per run, so nothing on this machine's real `PATH`, application directory or system
directories can satisfy it.

Both arguments are benign, for `bin.rs`'s reason: `%~1` strips the child's quotes, so a `&` would be
re-parsed by `cmd.exe` as a command separator inside the shim, and the case would measure batch
re-parsing instead of resolution. Argument escaping through a batch target is
`agent::bin::tests::arguments_reach_the_command_untouched`'s subject and does not depend on which
file a name resolved to.

## `fn a_bare_name_that_only_pathext_resolves_runs_through_the_host_runner() {` › `let mut direct = Command::new(name);`

The platform fact, executed. `std` searches the child `PATH`, the application directory, the system
directories and the parent `PATH`, appending `.exe` to each, so a `.cmd`/`.bat` on `PATH` is
invisible to it.

## `fn a_bare_name_that_only_pathext_resolves_runs_through_the_host_runner() {` › `let output = runner`

The claim: the same name, through the runner.

## `fn two_runners_in_one_process_resolve_one_name_against_their_own_environments() {`

Two runners in one process resolve one name against their own environments. A resolution remembered
anywhere process-wide (a `OnceLock`, a static cache) would hand the first boundary's answer to the
second, which no single-runner test can see. `agent::adapter::built_program_tests` holds this for the
adapters; this holds it for the boundary, with real spawns, in both orders (first-caller-wins is a
property of order), with the markers counted as distinct values.

## `fn an_absolute_program_is_spawned_as_given_even_when_path_holds_that_name() {`

An absolute program is spawned as given even when a `PATH` directory holds a different file of the
same name. Fixtures that put the only copy at that path cannot see a re-resolution; here two copies
make "used as given" and "searched for" produce different output. The space in the directory name
is `bin.rs`'s production shape, `C:\Users\John Smith\npm\`.

## `fn an_absolute_program_is_spawned_as_given_even_when_path_holds_that_name() {` › `let by_name = runner`

The control: the same runner and the bare name reach the other file, so the two are
distinguishable and the row above is not passing by coincidence.

## `struct ResolutionWitness {`

Everything one spawn's containment observers can see about resolution.

## `struct ResolutionWitness` › `at_points: Arc<Mutex<Vec<(SubEffectPoint, u64)>>>,`

`program_resolutions()` as each containment point saw it, in order.

## `fn a_program_is_resolved_once_per_spawn_before_any_of_it_and_never_before_compose_refuses() {`

A program name is resolved once per spawn, before any of the spawn, and not at all when the request
is refused earlier. These are independently droppable predicates that "the right file ran" does not
hold. The observable is `program_resolutions` read at every containment point between
`CreateProcess`/`fork` and the running child: a second resolution shows a second increment, a lazy
one shows the baseline at the first point, and one before `compose` shows an increment on the
request `compose` refuses.

Both program shapes, because "once" must hold on both branches: a bare name that is searched, and an
absolute path that is not. The bare name is a `PATHEXT`-resolved batch shim, a name to resolve that
`CreateProcessW` reaches only through an interpreter.

## `fn a_program_is_resolved_once_per_spawn_before_any_of_it_and_never_before_compose_refuses() {` › `let witness = ResolutionWitness::new();`

A request the environment refuses is refused before anything is resolved: `compose` runs first, so a
reserved-key overlay reaches neither the filesystem nor a containment point.

## `fn a_program_is_resolved_once_per_spawn_before_any_of_it_and_never_before_compose_refuses() {` › `let witness = ResolutionWitness::new();`

A name that resolves to nothing is refused after resolution and before any of the spawn: the count
moves, the points do not.

## `fn every_bare_program_this_crate_ships_goes_through_one_resolution_rule() {`

Every bare program this crate ships goes through one resolution rule. With two rules, the shells'
bare names work because a shell is an `.exe` while the agent CLIs' bare names, the same shape, fail.
The names are written here rather than read from `ShellKind::program` and the adapter constants, and
the exhaustive `match` ties the table to the enum, so a new shell fails to compile rather than
leaving the grid. The directory, shim content and naming rule are fixed; the name varies, and the
resolved files are counted, so a rule that special-cased one name collapses the count.

## `fn every_bare_program_this_crate_ships_goes_through_one_resolution_rule() {` › `const NAMES: [&str; 8] = [`

The five shells `gates::ShellKind::spec` can put in a spec, and the three agent CLIs
`bin::Invocation::named` can.

## `fn a_resolved_cmd_keeps_the_raw_tail_rule() {`

Resolving `cmd` does not change `cmd.exe`'s raw-tail rule. `build_command`'s one Windows rule is
keyed on the program, and the runner hands it the resolved program, an absolute path where the spec
carried a bare name. `gates::ShellKind::command` says why the tail must reach the child un-re-quoted;
a gate whose command line changed meaning with resolution would break that.

This has to spawn: `Command::get_args` yields the same sequence for `arg` and `raw_arg`, so the
built `Command` cannot tell them apart. Only the child's output can: std escapes an embedded quote
as `\"`, which `cmd.exe` does not un-escape, so a re-quoted tail echoes `\"quoted`.

The resolved path is the one this runner resolves, not a transcribed `C:\Windows\System32\cmd.exe`,
so the case holds wherever the shell lives; that it differs from `cmd` is asserted first.

## `fn a_resolved_cmd_keeps_the_raw_tail_rule()` › `let mut stdouts = BTreeSet::new();`

Both spellings, spawned: the quotes arrive as written, from the resolved path as from the bare name.

## `fn a_resolved_cmd_keeps_the_raw_tail_rule()` › `let workspace = scratch("raw-tail");`

And through the production route, where the gate ships the bare name and the runner resolves it.

## `fn a_resolved_cmd_keeps_the_raw_tail_rule()` › `let shim = build_command_at(&spec, Path::new(r"C:\npm\claude.cmd"));`

The control: a program that is not `cmd` does not get the rule. An npm shim `claude.cmd` has the
stem `claude`, so a rule keyed on the extension rather than the stem would give it a raw tail.

## `fn role_request_for(`

-----------------------------------------------------------------------
one boundary, one executable
-----------------------------------------------------------------------

## `fn role_request_for(`

The request production sends for `role`, carrying this program. `production_request` fixes the
program per role because there the role is the subject; here the program is, so each role's own
production builder gets the same spec. `Probe(Shell)` returns `None` because its program is the
recorded shell (`shell_probe_request`), and the `match` is exhaustive, so a new role must be
classified here rather than silently leaving the grid.

## `fn one_boundary_executes_one_file_for_a_name_across_a_probe_and_the_attempt() {`

One boundary executes one file for a name, even when the filesystem moves between pre-flight and the
attempt. The runner design commitments in `design/21_design_versioned_scope.md` route probes through
the same runner so that pre-flight cannot certify a host CLI different from the one the attempt
executes. Routing is necessary, not sufficient: with `PATH=first:second` and the name in both, the
probe certifies `first/<name>`, `first/<name>` is removed, and a runner that searched per spawn would
hand the attempt `second/<name>` under an unchanged `CommandSpec.program`. Comparing program strings
cannot see that.

The control is the oracle: a fresh runner over the same environment, after the removal, reaches
`second/<name>`, so the files are distinguishable, the removal changes the answer, and the memoised
runner's refusal is the memo. One `HostEnvironment` serves every row; the role (`Probe(Agent)` then
`Implement`) and the filesystem between them vary.

Fail-closed on purpose: the attempt gets a failure naming the file pre-flight certified, which tells
the operator the CLI moved under a running run, rather than a `Caps.version` that silently stopped
describing anything.

## `fn one_boundary_executes_one_file_for_a_name_across_a_probe_and_the_attempt() {` › `std::fs::remove_file(&first).expect("remove the certified installation");`

The CLI moves under the run: the file pre-flight executed is gone, and the other one (same name,
same `PATH`, different executable) is still there.

## `fn one_boundary_executes_one_file_for_a_name_across_a_probe_and_the_attempt() {` › `let fresh = HostRunner::new().with_environment(environment());`

The oracle: a boundary that has not decided yet reaches `second`, so per-spawn resolution would
change the answer.

## `fn one_boundary_executes_one_file_for_a_name_across_a_probe_and_the_attempt() {` › `let outcome = runner.run_blocking(&attempt);`

The claim: the runner that certified `first` does not run `second`. The failure's shape is the
platform's: on Unix spawning the vanished file is `ENOENT` and the runner returns an error naming it;
on Windows `std` runs a `.cmd` through `cmd.exe`, so the spawn succeeds and the interpreter exits
non-zero. The claim is that the attempt neither ran the other installation nor reported success, so
it is asserted over whatever the boundary returned. The text read is the child's own, not a `{:?}`
rendering, which would escape every backslash and hide a Windows path.

## `fn one_name_is_searched_once_for_a_boundary_and_asked_for_once_per_spawn() {`

One name is searched once per boundary and asked for once per spawn. The two predicates drop
independently, so there are two counters: `program_resolutions` moves once per spawn
(`a_program_is_resolved_once_per_spawn_…` holds that and its position) and `program_searches` once
per boundary. A memo that never hits satisfies the first and breaks the pre-flight/attempt
agreement; this is the fixture for the second.

Across roles on purpose: `host-v1` supplies credential locations by role (`supplies_credentials`),
so a probe's composed environment and a gate's differ, asserted first as a distinct-value count. A
memo keyed on the composed environment would miss on exactly the pre-flight/attempt pair that must
agree; the key is the fields that decide the answer, not the environment.

`Probe(Shell)` is absent because its program is the recorded shell; `role_request_for` is exhaustive
over `ExecutionRole`, so it is absent by classification.

## `fn one_name_is_searched_once_for_a_boundary_and_asked_for_once_per_spawn() {` › `let on_path = environment_on_path(&[&bin], Some(REAL_PATHEXT));`

`host-v1` supplies a credential location only when the base carries it, so the base must carry one
for the roles to differ; without it every role would compose the same environment.

## `fn one_name_is_searched_once_for_a_boundary_and_asked_for_once_per_spawn() {` › `let composed: BTreeSet<Vec<(OsString, OsString)>> = requests`

The premise: these roles compose different environments, so one answer for all of them is a claim,
not a tautology.

## `fn a_resolution_question_is_the_program_and_the_environment_that_answers_it() {`

The memo is keyed on the program and the environment that answers for it. A memo keyed on the name
alone would certify the wrong executable, and no fixture above could see it, because in production
`PATH` is reserved and constant for a run. So this asks one boundary the same name under two
environments and requires two answers.

It calls `program_for` directly because through `run` the only composed value a caller can vary is
`PATHEXT` (an overlay may not name the reserved `PATH`), and `PATHEXT` decides nothing on Unix;
calling it directly exercises both fields of the key on both platforms. The runner, name and naming
rule are fixed; the composed `PATH`, and on Windows `PATHEXT`, vary, and the resolved files are
counted as distinct values.

## `fn a_resolution_question_is_the_program_and_the_environment_that_answers_it() {` › `{`

The other field of the key, where it decides anything.

## `fn a_refused_name_is_refused_identically_without_asking_the_filesystem_again() {`

A name this boundary refused stays refused, in the same words, without asking the filesystem again.
Without this, a run whose pre-flight could not find `claude` would find one at a later attempt
because something installed it meanwhile: pre-flight certifying an absence the attempt ignores.

The control is the oracle: after the CLI appears, a fresh boundary runs it, so the second refusal is
the memo holding. The replayed error must equal the first byte for byte, which is what makes storing
the refusal as its message safe: `UpstrokeError::Refused` displays as exactly its message.

## `fn a_refused_name_is_refused_identically_without_asking_the_filesystem_again() {` › `marker_shim(&bin, &shim_file_name(&name), "LATE");`

The CLI appears under the run.

## `fn a_refused_name_is_refused_identically_without_asking_the_filesystem_again() {` › `let fresh = HostRunner::new().with_environment(environment());`

The oracle: it is installed now.

## `fn production_reaches_a_spawn_through_one_host_runner_per_run() {`

Production reaches every spawn of a run through one `HostRunner`. The memo is per boundary, so the
probe and the attempts agree only while they share a runner, and nothing in the types enforces it:
`run_harness_on` takes `&dyn Runner`, and an engine that built one per attempt would keep the suite
green and break the agreement. Structural rather than behavioural, for
`the_adapters_hold_no_process_wide_resolution_state`'s reason: per-attempt and per-run runners
differ only in how often the filesystem is asked, which only a whole engine run against a moving
filesystem could observe.

The expectation is written out, not counted from the tree (a count read from the tree grows with
it): three construction sites, `run_harness` in `src/engine/coordinator.rs`, `resume_harness` in
`src/engine/resume.rs`, and the schema-4 run's in `src/engine/topology/entry.rs`, and none in the
engine's other files. The facade's entry points live in the conductor modules and the facade
re-exports them, so the facade constructs nothing (`PR306-FACADE-INLINE-ESCAPE`).

Both constructor spellings in `CONSTRUCTORS`, `HostRunner::new` and
`HostRunner::for_legacy_workspace`, are counted and both appear in the control; a spelling the
census did not know would read as no runner rather than a second one.

## `fn production_reaches_a_spawn_through_one_host_runner_per_run() {` › `const SITES: [(&str, usize); 6] = [`

Where a `HostRunner` is constructed in the engine's production code, and how many times in each
file.

## `fn production_reaches_a_spawn_through_one_host_runner_per_run() {` › `assert_eq!(`

The control holds comments, literals, a test-only function and one later production construction
per constructor spelling. It must add exactly `CONSTRUCTORS.len()` to the count alone and when
appended to each of the seven source files.

## `fn production_reaches_a_spawn_through_one_host_runner_per_run() {` › `let conductors = [`

`run_harness` and `resume_harness`, each read from the conductor module that defines it, construct
their runner in their own body, which then serves pre-flight and every attempt. The body runs to the
first column-0 `}` after line endings are normalised to LF, so a CRLF checkout cuts at the same
place.

## `fn an_npm_style_installation_runs_by_bare_name_exactly_as_it_runs_by_path() {`

An npm-style installation of each of the three agent CLIs runs by bare name exactly as by path.
`agent::adapter::built_program_tests::the_host_runner_executes_a_bare_program_name_as_it_executes_the_resolved_path`
claims this equivalence with `git`, a native `.exe` that `CreateProcessW` reaches from a bare name
whether or not this runner resolves anything. `npm install -g` writes `claude.cmd`, `codex.cmd` and
`copilot.cmd`, reachable only through `PATHEXT`, so a fixture without that shape cannot fail.

All three names, each with its own marker, counted, so a rule that special-cased one name collapses
the count. The installation is `<name>.cmd` on Windows and an extensionless script on Unix, with no
`.exe` beside it, on a `PATH` this test wrote. The bare name and the absolute path of the file it
must resolve to both go through `HostRunner::run` and must produce the same output; the two program
strings are asserted to differ first.

It runs on both platforms. On Unix `execvp` would satisfy it anyway, which is why the Windows arm
needs a fixture that can fail; `a_bare_name_that_only_pathext_resolves_…` holds the `NotFound`
platform fact on Windows, and this holds the equivalence everywhere. The CLI name and the program's
spelling vary; the runner, environment, argument and files are fixed, so a difference in output can
only be which file ran.

## `fn an_npm_style_installation_runs_by_bare_name_exactly_as_it_runs_by_path() {` › `const CLIS: [&str; 3] = ["claude", "codex", "copilot"];`

The three names `bin::Invocation::named` ships, written here rather than read from the adapters'
private `CLI` constants; `agent::adapter::built_program_tests::an_adapters_program_is_the_boundarys_…`
ties each adapter to its name.

## `fn an_npm_style_installation_runs_by_bare_name_exactly_as_it_runs_by_path() {` › `let mut contents: Vec<String> = std::fs::read_dir(&bin)`

The installation is the failing shape: one file per CLI, and on Windows none is an `.exe`.

## `const PATH_ENTRY_TABLE: &[(&str, bool, bool)] = &[`

-----------------------------------------------------------------------
the workspace is not a PATH directory
-----------------------------------------------------------------------

## `const PATH_ENTRY_TABLE: &[(&str, bool, bool)] = &[`

A `PATH` entry, and whether each platform calls it a location on its own. Written from the
platforms' rules, then checked against `Path::is_absolute` for the platform running the test. No
entry holds either `PATH` separator, so each stays one entry under `std::env::split_paths`, and none
holds a `"`, so `path_of` returns each as written.

## `("", false, false),`

(entry, is a location under Unix, is a location under Windows)

The empty entry: POSIX gives a null prefix the meaning "the current directory".

## `("/usr/local/bin", true, false),`

Rooted on Unix; on Windows a leading separator is relative to the current drive, so it is still a
current-directory question.

## `(r"\\server\share\bin", false, true),`

A UNC share names a location on Windows and is an ordinary file name on Unix. Which share it names
is never asked: the test's program name carries an interior NUL, so the first candidate under this
entry is refused before any request leaves the process, and no SMB answer ("no such share",
`os error 64`, or none) can reach the verdict.

## `fn every_path_entry_this_runner_searches_names_a_location_on_its_own() {`

Every `PATH` entry this runner searches names a location on its own. A non-absolute entry resolves
against a current directory, and this boundary has two: the coordinator's, which
`ProgramNaming::is_program` inspects, and the workspace, where the child runs. Such an entry would
let the runner certify one file and execute another, and the one executed is repository content
under automation.

The written table is checked against `Path::is_absolute` first, so the row is a claim about
`resolve_program`, not `std`. The entry varies; the program name, candidates and composed
environment are fixed.

The table answers which entries are searched. It is kept apart from what a searched entry's
candidates answer: asserting a full walk of each absolute entry would make the UNC row depend on
what `\\server\share` answers on the machine (`NotFound` is walked past, but `os error 64` stops the
search). The policy is unchanged and decided here by construction: an unanswerable candidate stops
the search, mere absence is walked past, on any machine whose temporary directory can hold the
positive case's own paths (below).

## `fn every_path_entry_this_runner_searches_names_a_location_on_its_own() {` › `fn search_summary(message: &str) -> Option<&str> {`

What a refusal says about the search: the parenthesised clause after "this runner composes", giving
the directories searched, the entries skipped as not absolute, and the candidates. Every assertion
on a refusal's text reads that clause and nothing else.

The rest of the message echoes the `PATH`, which carries the temporary directory's spelling, which
belongs to the machine: a temporary directory named `temp-skipped`, or one holding
`1 directory searched,`, would otherwise decide the result. In a refusal the clause comes before the
echo and holds only text this test constructs (the counts, its program name and the default
extensions), so the first opening is the resolver's own and no spelling of the temporary directory
can reach, extend or close it. A refusal without the clause fails the assertion that reads it.

That holds only of a refusal, so both readers, for the relative entries and the absent entry, first
require `UpstrokeError::Refused` and fail, naming the variant, on anything else before reading its
text. Another error's message is no summary: a `Filesystem` error's is "failed to stat" and the
candidate's path, which begins with the temporary directory and can contain the clause's opening.

## `fn every_path_entry_this_runner_searches_names_a_location_on_its_own() {` › `let program = "upstroke\u{0}no-such-program";`

The program every table entry is asked about: a bare name, so the walk runs, with an interior NUL,
so no candidate under any entry can be answered. std refuses such a path before any system call
(the property `undeterminable_directory` in `naming.rs`'s tests stands on), and the fixture asserts
that for each entry it searches: the refusal is not `NotFound` and carries no OS error code, so
nothing on the machine was asked. A platform that answered otherwise fails the fixture, not the
rule.

So an absolute entry's walk stops at its first candidate, and "searched" is read from that stop: a
`stat` failure whose path is a candidate directly under the entry and whose source is that refusal.
A relative entry is never stat'ed and must report itself skipped. Neither depends on what exists at
`/usr/local/bin` or what a share answers. The stop is the policy
`an_undetermined_candidate_stops_the_search_before_a_later_match` pins; this test relies on it, so a
change to that policy fails both.

## `fn every_path_entry_this_runner_searches_names_a_location_on_its_own() {` › `let bin = root.join("bin");`

The count of a finished search, `1 directory searched,`, is asserted only for an entry absent by
construction: the positive case's own entry, `bin` under the scratch root, asked for the positive
case's program `x` before either exists, checked to be `NotFound` first. It asks exactly the paths
the positive case asks once `program_file` installs `x`, while their directory does not exist, so
each answers `NotFound` and the walk runs to its end.

The candidates are the positive case's on purpose. A missing directory does not make every path
below it `NotFound`: a path longer than the platform allows is refused for its length first (Linux
`ENAMETOOLONG`, os error 36). Asking only the positive case's paths, this search runs to its end
under any temporary directory short enough for the positive case to work; under a longer one the
test cannot pass at all and this case fails openly, at its absence check or at the refusal it
requires, never on a count read from a path.

Its `PATH` is built by `path_of`, like every `PATH` this test passes, and like both witnesses' with
`std::env::join_paths`, so the one entry searched is the directory just checked. A raw path would
split on a temporary directory spelled with the separator (`;` on Windows, `:` on Unix). `join_paths`
quotes a `;` on Windows and the search unquotes it. A spelling no `PATH` can carry (`:` on Unix, `"`
on Windows) is refused by `path_of`, as by `join_paths`, and the test fails there rather than search
elsewhere.

## `fn every_path_entry_this_runner_searches_names_a_location_on_its_own() {` › `let found = program_file(&bin, &shim_file_name("x"));`

"Searched" means searched: the entry that is not skipped finds the program once it is installed.

## `fn an_empty_path_entry_never_reaches_the_workspaces_own_copy_of_a_bare_name() {`

An empty `PATH` entry never reaches the workspace's own copy of a bare name. A coordinator whose
`PATH` holds an empty segment (`:/usr/bin`, what a profile appending to an unset `PATH` produces)
and a workspace containing an executable `claude`: POSIX makes the null prefix the current
directory, and this runner's current directory is the workspace, so a bare name handed to `Command`
would run repository content with the coordinator's authority as the agent.

The platform fact is executed: each row first spawns the bare name through `std::process::Command`
with the same composed environment and working directory and compares it with a written
expectation. In three of the four rows the raw spawn reaches the workspace and the runner must not,
and that count is asserted.

Unix only: the empty-entry rule is POSIX's and the installations are shell scripts. The rule that
closes it is executed on both platforms by
`every_path_entry_this_runner_searches_names_a_location_on_its_own`. Where the empty entry sits,
and whether a real installation is on `PATH`, vary; the workspace, its planted executable, the name
and the argument are fixed.

## `fn an_empty_path_entry_never_reaches_the_workspaces_own_copy_of_a_bare_name() {` › `marker_shim(&workspace, &file, "WORKSPACE");`

Repository content, under automation: the workspace's own copy.

## `fn an_empty_path_entry_never_reaches_the_workspaces_own_copy_of_a_bare_name() {` › `let rows: [(&str, String, &str, &str); 4] = [`

(what, PATH, what a raw spawn reaches, what the runner must reach)

## `fn an_empty_path_entry_never_reaches_the_workspaces_own_copy_of_a_bare_name() {` › `let mut direct = Command::new(&name);`

The platform fact: `execvp` searches the child's `PATH` from the child's working directory, so the
empty entry is the workspace.

## `fn an_empty_path_entry_never_reaches_the_workspaces_own_copy_of_a_bare_name() {` › `let witness = ResolutionWitness::new();`

The claim, through the boundary, with an observer so a refusal is shown to come before any spawn.

## `fn a_relative_path_entry_is_refused_even_when_it_names_a_real_directory() {`

A relative `PATH` entry that names a real directory is refused, not searched. The empty entry is its
degenerate case; here the two current directories visibly differ: the entry resolves from the
coordinator's working directory (asserted, so the row is hostile), while the child would resolve it
from the workspace. Searching it would hand `Command` a relative program and certify a file that is
not the one that runs.

The entry climbs with `..` to the root and descends again, so it is relative and resolvable without
writing inside the repository. Unix only: on Windows the construction depends on the temporary and
working directories sharing a drive, and a row that silently stops applying is worse than none.

## `fn a_relative_path_entry_is_refused_even_when_it_names_a_real_directory() {` › `let mut workspace = root.clone();`

Deeper than the coordinator's directory, so the same relative entry names a different place from
each; a workspace at the same depth would hide the hazard.

## `fn a_relative_path_entry_is_refused_even_when_it_names_a_real_directory() {` › `assert!(`

The row's own premise, both halves.

## `fn a_relative_path_entry_is_refused_even_when_it_names_a_real_directory() {` › `let reachable = HostRunner::new()`

The oracle: the same directory, named as a location, does run.

## `#![allow(`

Allowlist placement: the funnel section of `effects/allowlist.toml`, which carries this module's
review clause. `effect_site_inventory.mechanism` (2).

## `struct HeldFork {`

Owns one forked child's lifetime. `hold_inherited_descriptors` forks it from a separate thread, so it
inherits this process's descriptors as a fork in another harness thread would. The child closes its
copy of the parent's socket endpoint, writes one readiness byte, then blocks reading its own
endpoint; it exits 0 when the read sees EOF (released) and 1 otherwise, including when the endpoint's
60-second read timeout expires. The parent's endpoint also reads with a 60-second timeout, so
readiness is bounded.

Drop is the release: it shuts down the write half of the parent's endpoint, which the child reads as
EOF on success and failure alike, then waits for that exact pid, never a group, retrying on `EINTR`.
Outside a panic it requires the release, the reap and a zero status, so the holder must have
completed its socket protocol.

## `let waited = unsafe { libc::waitpid(self.pid, &mut status, libc::WNOHANG) };`

SAFETY: pid is this guard's unreaped child and status is writable. WNOHANG observes whether it
exited without waiting for release.

## `let result = unsafe { libc::waitpid(self.pid, &mut status, 0) };`

SAFETY: pid is our unreaped direct child; status is a live, writable c_int. The socket's read
timeout bounds the child.

## `unsafe {`

SAFETY: only the parent returns into Rust after fork. The child uses only close/write/read/_exit,
all async-signal-safe, with live inherited fds and one-byte stack buffers. It never allocates,
unwinds, drops Rust owners, or touches inherited locks. Closing parent_fd removes its copy of the
release endpoint. The separate child endpoint stays live until _exit closes all inherited fds.

## `let polled = unsafe { libc::poll(&mut ready, 1, 1000) };`

SAFETY: ready is one initialized pollfd and remains live for this call; its descriptor is borrowed
from reader. The wait is bounded.

## `assert_eq!(`

SAFETY: name is a live NUL-terminated path in our private scratch directory, and 0600 grants access
only to this test's user.

## `let installed = unsafe { libc::fcntl(reader.as_raw_fd(), libc::F_SETPIPE_SZ, 4096) };`

SAFETY: reader owns a live FIFO descriptor. F_SETPIPE_SZ takes an integer size, and shrinking an
empty pipe needs no extra privilege.

## `const CONTROL: &str = r##"`

STRIP-CONTROL goes through the same whole-file blanker and counter as production. Its production
constructions, one per spelling in `CONSTRUCTORS`, follow a test-only item, so truncating at the
first `#[cfg(test)]` also fails this control. Each spelling also appears in a comment, a nested block
comment, a string, a raw string, a byte string, a raw byte string and a test-only function, so no
spelling is counted from prose or test code.

## `let engine = crate::effects::production_code(include_str!("../../engine/mod.rs"));`

Each conductor's production code, with CRLF made LF, is cut from its entry point (`fn run_harness(`
or `fn resume_harness(`) to the first column-0 `}`, and that body must hold a construction: the runner
it then lends to pre-flight and every attempt.

## `struct Witness {`

A per-invocation `SpawnHooks` observer: the pids `child_created` reported, the containment points in
the order they fired, and the thread each child was created from. A test hands it to one call through
`RunnerCall::observed_by`, so it records that invocation and no other.

## `fn scratch_tree(tag: &str) -> crate::rundir::scratch_tree::ScratchTree {`

Scratch from `rundir::scratch_tree::acquire`, which refuses an occupied root and reclaims the tree
when the guard drops, unlike this file's older `scratch`, which leaves its directory behind.

## `fn wait_for_file(path: &Path, within: Duration) -> bool {`

The cancellation tests' handshake: the child publishes a marker once running and the test acts only
after seeing it, within a bound, so a child that never starts fails the test instead of hanging it.
A condition with a deadline, not a sleep.

## `fn the_blocking_adapter_supervises_the_process_on_the_calling_thread() {`

The blocking adapter creates the child from the thread that called `run_blocking_with`, so the
thread-scoped state on the spawn path (the reaper's cleanup-lease paths and the observation
export's attribution) is the caller's. The call's own observer also sees the spawn's containment
points.

## `fn a_call_cancelled_before_it_starts_spawns_nothing() {`

A call cancelled before the runner looks at it is refused as `NeverStarted`: nothing spawned, nothing
observed, and the program name not resolved (the resolution counter does not move).

## `fn cancelling_a_running_invocation_terminates_it_and_reports_it_cancelled() {`

The protocol's order on a real child, on every platform: the child announces itself, the test
cancels from another thread, and the invocation reports `RunnerError::cancelled` with fate `Gone`,
the runner having terminated the process before reporting. The time from cancel to report is bounded
against a child that would otherwise run 300 seconds.

## `fn cancelling_an_invocation_ends_every_process_of_its_tree() {`

Unix. The tree, not just the child: the child opens a fifo for writing, starts a background
grandchild that inherits the descriptor, and announces itself. The fifo's reading end reports
`WouldBlock` (writers exist) before the cancel, so the end of file after it is not vacuous. Once the
runner reports the invocation cancelled, every writer is gone and the fifo reads end of file: the
oracle `agent::proc`'s `kill_tree_settles_the_whole_unix_group_before_it_returns` uses for a timeout,
with the same bounded read loop while the kernel closes a killed process's descriptors.

## `fn rendezvous_helper() {`

Subprocess helper (`--ignored`): publish this child's marker, wait, bounded, for the peer's, and
print this process's pid. Two children waiting for each other both succeed only if they ran at the
same time.

## `fn two_invocations_on_one_host_runner_run_at_once_each_carrying_its_own_observation() {`

Two invocations on one runner, each carrying its own observer, run at once (both rendezvous
children exit 0), and each observer saw exactly its own child's pid and one spawn's containment
points, the sequence a lone invocation records.

## `fn a_runner_level_observer_still_takes_invocations_one_at_a_time() {`

The control, and the contract kept for every test that installs an observer: under `with_hooks` the
same pair takes turns, so exactly one child outlives its shortened wait for the other. It also shows
the rendezvous oracle can fail.

## `fn a_call_carrying_the_runs_cleanup_leases_is_reaped_by_a_reaper_that_holds_them() {`

A pipeline thread has not entered the run's cleanup scope (the scope is thread-local and owned by
the coordinator's thread), so the Unix reaper of a process it spawns holds the run's R28 lease only
if the call carries the lease paths. The control carries none, and nothing holds the lease while its
child runs. After each call the hold must end with the reaper, read as a wait bounded by the test's
own 60 seconds, the bound its readiness handshake uses: a sibling test thread's fork can hold a copy
of the lease briefly after the reaper exits
(`PR281-CLEANUP-LEASE-HOLD-OUTLIVED-AND-ITS-UNREADABLE-TWIN`), and a hold that outlives the bound
fails the test.

## `mod inherited_writer` › `pub(crate) fn writes_leave_no_writer_in_another_threads_fork(`

The FIFO oracle: a writer fills a FIFO larger than its pipe while another thread forks a holder of
this process's inherited descriptors; once the writer is done, the reader sees end of file only if
no fork holds a writer. It judges the marker shims, the container fake's relay writer, and, from the
engine suite with `shim_script`, the gate role's stand-in writer `write_the_gate_probe_stand_in`
(`engine::tests::gate_probe_writer_helper`), which is why this module, the oracle and `shim_script`
are `pub(crate)`.

## `mod inherited_writer` › `fn the_reaper_relay_writer_leaves_no_writer_in_another_threads_fork() {`

The relay stub a container reaper execs is written by
`runner::container::write_program_in_its_own_process`, in a process of its own, so no fork of the
test process holds a writer of it and its `execv` cannot fail `ETXTBSY`; written in-process, the
reader would never see end of file.

Its helper runs under `proc::test_support::run_test_isolated` with a 180-second deadline, not
through the host funnel (`FUA-I4-RELAY`): the funnel's end waits without a bound, by design, for its
reaper's acknowledged exit, so a stopped reaper there would hold this test and the suite.


## `const INHERITED_SELECTORS: [(&str, &str); 4] = [`

The four work-tree selectors with values a role must not be handed in a work
tree the engine made; and below, `KEPT_INHERITED`, the repository-wide names,
the command scope and the pathspec and attribute globals, which every role keeps.

## `fn role_request(`

A request for any role, through the builder production uses for it, with the
command and workspace a test chooses; a probe's identity is the probe target's.

## `fn dumped_or_refused(`

The environment a role's process actually received, read from the dump helper
the runner starts as that role, or the `RunnerError` that refused it before it
started.

## `const fn keeps_the_inherited_context(legacy: bool, role: &ExecutionRole) -> bool {`

The tests' own statement of which roles keep the inherited Git context:
every probe, and the implement role of the runner built for the legacy
workspace. It is the rule, written again here, so a change to the runner's
copy of it is a difference the test sees.

## `fn a_role_in_a_work_tree_the_engine_made_sheds_only_the_work_tree_context() {`

The runner's half of option (C), through `supervise` and a real child per row,
under both `KeyCase` rules, for every role and both constructors:

- under `for_legacy_workspace`, which holds a managed repository, gate and
  review shed exactly the four selectors, a `GIT_COMMON_DIR` that is relative
  (`../common`, `common`) or empty is replaced by the repository's
  `common_dir`, and an absolute one is kept; the implement role and every
  probe keep them all;
- under `new()`, which holds none, implement, gate and review shed the four and
  keep an absolute `GIT_COMMON_DIR`, and with a relative one each is refused
  before it starts, `NeverStarted`; every probe keeps them all and starts;
- an overlay's own value of any of the five names is kept, a relative
  `GIT_COMMON_DIR` included, and nothing is refused;
- the repository-wide names, the command scope, the globals and
  `GIT_NO_REPLACE_OBJECTS` are what composition made them;
- `git_dir` and a relative `git_common_dir` spelled in lower case are shed and
  replaced under `Insensitive` and kept under `Sensitive`; an absolute one is
  kept under both.

`keeps_common_dir` is asked directly too, with the platform's own absolute
values: `/abs/common` and an absolute path holding a byte no UTF-8 spells on
Unix; `C:\abs\common` and `\\server\share\common` on Windows, where
`\abs\common`, `C:abs\common` and `..\common` are not absolute. Each row's
mismatches are collected, so one run reports every row that fails.

## `fn windows_a_role_in_a_work_tree_the_engine_made_sheds_the_work_tree_context_in_any_spelling() {`

Windows only, natively, under a runner that holds the managed repository: its
control first shows that Git for Windows reads `git_dir` and `Git_Index_File`
as `GIT_DIR` and `GIT_INDEX_FILE`, and fails as unmeasured if it does not; then
the implement, gate and review roles shed both, and a `git_common_dir` that is
relative, drive-relative (`C:x`) or root-relative (`\x`) is replaced by the
repository's `common_dir`, while a drive-prefixed or UNC value is kept. Its last
rows measure `Path::join` on the two forms that are not absolute there: a
root-relative value takes the checkout's drive, and a drive-relative value
stays as it is, which is the builders' reasoned limit on Windows. The control
gives Git the Git directory as the manager spells it, without the verbatim
prefix `fs::canonicalize` returns there (the fixture's `canonical`), because Git
for Windows refuses a verbatim Git directory as "not a git repository"; Git's
answer is compared canonical.

## `fn slot_roles_under_an_inherited_context_address_only_their_own_work_tree() {`

The O3-R report's construction and its steps 01 to 25, run through
`Runner::run_blocking` in an isolated child whose environment alone carries
each case's names: `control`, `index`, `dir`, `dir-index` and
`relative-index`. `managed` holds four linked work trees made in this
process's environment, `other` a staged file its user then edited, and `alt`
a copy of `other`'s index. Every implement, gate and review role of
`HostRunner::new()`, and the legacy runner's gate, answers
`--absolute-git-dir` and `--git-path index` with its own work tree's; the
probes and the legacy worker keep the inherited context, so their answers are
not asserted. Afterwards `other`'s index and objects and `alt`'s index are
byte-identical, no lock is left there, and the gate killed while
`git update-index` held the index lock left that lock in its own snapshot's
administrative directory.

Three cases run their implement role under a runner that holds the managed
repository, as a topology runner that starts roles in slots must: a relative
`GIT_COMMON_DIR` with a common directory beside the slot; an absolute `GIT_COMMON_DIR` with the
run's `worktrees` directory a link into another repository (C4); and a relative one that names the
run's common directory from the base and from the slot behind the same link (C5). Each role's
`--git-common-dir` is the run's, its `git add` writes into the run's object store, and the directory
beside the slot, or the other repository's objects and refs, are byte-identical. `control` passes
with or without the shedding; the other report cases and the relative case fail at `5c222ff2`; C4
and C5 pass there but fail when `GIT_COMMON_DIR` is removed in a work tree, and C5 also when only a
relative value is.

## `fn slot_roles_helper() {`

The child of the test above: one case's steps, under its names.

## `fn a_hook_started_runs_slot_worker_leaves_the_users_commit_as_the_user_made_it() {`

A real `git commit -a` in a main checkout and a plain commit in a linked
checkout, each with a `pre-commit` hook that starts this binary's helper as
the coordinator: it derives a manager at the hook's directory, adds a slot and
has an implement role of `HostRunner::new()` add a file there. The user's
commit then holds exactly the user's paths and the user's change. At
`5c222ff2`, in both, the worker's file enters the commit and the user's change
is lost from it, because the slot add and the worker wrote the index the
commit is being made from.

On macOS CI the linked-checkout case has failed with the commit's `ProcessOutput` showing code
`None` (on Unix, an end by signal, whose number the stored output does not keep), no timeout and no
output limit, while the hook's helper had passed. The signal, its sender and the cause are unknown
(`G7-CI-MACOS-HOOK-COMMIT-FLAKE`).

**The Trace2 diagnostic.** Each case sets `GIT_TRACE2_EVENT` on the outer commit's `Command` alone,
to a fresh absolute file directly under the case's scratch root (`trace2-main.event` or
`trace2-linked.event`), outside `M`, `L`, the hooks directory and the private root; the process
environment and Git's configuration are unchanged. Git normally passes the variable on to the hook
and the Git commands under it, so their events can share the file, which widens the trace but does
not prove which process wrote which event. When the funnel returns, `report_hook_commit_trace`
prints the report into the captured output before the exit code is checked, while the scratch tree
still exists; a passing run discards it, a failing or `--nocapture` run shows it. The command, the
funnel's 180 s bound, both cases, the hook, its helper, every assertion and the control flow are as
they would be without it.

**Limits.**
- An `Err` from `run_with_timeout` panics at once with no report, because covering that path would
  change its control flow.
- Git writes a `signal` event only for signals it handles, and nothing for `SIGKILL`, so a trace
  without one rules nothing out.
- The report is a snapshot taken when the funnel returns.

It is a diagnostic, not a fix, and does not claim the failure is explained.

**Quarantined on macOS.** A `cfg_attr` ignores the test on macOS with a reason naming
`G7-CI-MACOS-HOOK-COMMIT-FLAKE`. It still runs on Linux and other Unix systems, and on macOS under
`--ignored`. While the quarantine stands the macOS job does not witness the failure; lifting it is
part of the finding's repair.

## `const HOOK_COMMIT_TRACE_HEAD: u64 = 64 * 1024;`

The read bound of [`report_hook_commit_trace`], together with
`HOOK_COMMIT_TRACE_TAIL` (192 KiB). A trace of at most 256 KiB is read whole. A
longer one is read only in its first 64 KiB and its last 192 KiB, and the
bytes between them are counted and skipped by a seek. The ends are the parts
kept because the outer commit's first events open the file, and its last
ones, if it wrote any, normally come after the hook's.

## `fn report_hook_commit_trace(label: &str, trace: &Path) {`

Prints one case's trace with `eprintln!`, so that libtest captures it. The
output, in order:
1. A line naming the case (`label`), the path, the size when the file was
   opened, and the number of bytes omitted.
2. Each newline-ended piece of the bytes read, as `  | ` followed by the piece
   escaped by `<[u8]>::escape_ascii`.
3. For a long file only: a line giving the number of bytes omitted, followed
   by the tail's pieces in the same form.
4. A closing line with the number of bytes shown, or the error that stopped
   the read.

The escaping turns quotes, backslashes, control bytes and non-ASCII bytes into
escapes, and shows a record's newline as `\n`. No trace byte then reaches a
terminal as a control character, and the bytes read can be recovered exactly. The
output is at most nine characters per byte read: four for an escape, plus the
prefix and line end of a piece that can be a single byte long. Three framing
lines come on top of that.

It reports and never asserts:
- a path that is not absolute was never given to Git, and is reported as
  such;
- a missing file is reported as `missing`;
- an open or size error is reported as `unreadable`;
- a read error is reported after whatever was read before it.

It reads only that one file. It does not panic on a read problem and returns
nothing to the test, so it can neither replace the commit's `ProcessOutput`
and failure nor make a failing case pass.

## `fn hook_started_slot_worker_helper() {`

The coordinator the hook starts, under exactly the names Git handed the hook.
