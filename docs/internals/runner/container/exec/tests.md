# `src/runner/container/exec/tests.rs`

Extended notes for [`src/runner/container/exec/tests.rs`](../../../../../src/runner/container/exec/tests.rs).

[Source on GitHub](https://github.com/sourcemaps/upstroke/blob/master/src/runner/container/exec/tests.rs).

The code is the authority for what it does; this file is the whole of its prose, moved out of
the source verbatim. Each section is headed by the line of code the comment sat above, spelled
as it is in the source, so the heading is the grep string that finds the code.

## `#![allow(clippy::disallowed_methods)]`

Allowlist placement: the **funnel section** of `effects/allowlist.toml`, which
carries this file's own review clause. This is the extracted test module of
the `ContainerRunner` funnel -- the region that lived inline in
`src/runner/container/exec.rs`, under an outer allow on its declaration
there, moved out unchanged. That outer attribute is GONE, deleted by the same
change: with the level stated here it allowed one lint twice for one module,
which is what `clippy::duplicated_attributes` is, and `exec.rs`'s own
allowlist row records the empty `allows` that leaves. The set of calls
permitted here is unchanged by the move: the fixtures build worktrees and
scratch trees and drive the runtime seam directly, exactly as they did
inline. What moved is where the permission is stated, not what it permits.

`PR6-LANEF-004`: it states that level **of its own** rather than inheriting
one. A lint level is scoped by the MODULE TREE and not by the file, so an
out-of-line child of a funnel is covered by the parent's allow unless it says
otherwise, and the funnel's child-module census requires each child to say so.
The two lints this file does not need are re-denied, so either one appearing
here is still a build error.
`decisions.effect_site_inventory.mechanism` (2).

## `fn repo_key() -> &'static str {`

This module's repository key: **the build slot's**, through the one
place that derives it.

The derivation and the reason it is not a constant moved to
[`crate::runner::container::fake::slot_repo_key`], beside the pre-clean it is a
precondition of. It lived here, and `census/tests.rs` — the other caller
of `fake::preclean_names` — went on using a fixed `"cccccccccccccccc"`,
which is `PR7-R3-CONTRACT-001` still live on that path. A rule with one
implementation per caller is a rule each caller can be missing.

## `fn a_container_name_is_scoped_to_its_build_slot() {`

**A pre-clean reclaims its own slot's residue and nobody else's.**

The class boundary, not the instance. `PR7-R3-CONTRACT-001` is that
`fake::preclean_names` kills by name with no liveness check, and a
*fixed* name means the container it kills may be a live stranger's.

**A liveness check would not have fixed it.** The state this helper
exists for is a SIGKILLed run whose container is still *running*, so
"don't kill running ones" defeats the helper. The fix is that two
concurrent runs never ask for the same name, and this asserts that
property rather than the instance that prompted it.

## `const EVENT_LOG_MARKER: &str = "COORDINATOR-EVENT-LOG-a5f2";`

Written into the run's public log, so a container that could read it
would be caught by content rather than by the absence of a file.

## `const IMAGE_PATH: &str = "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin";`

The `PATH` the fake fixtures' image environment carries.

Absolute-only, which is what `ContainerEnvironment::certify_path` now
requires: a runner whose composed environment names no `PATH`, or names
one with a working-directory-relative component, refuses every
invocation (`PR6-CORRECTNESS-006`). The value is the one every image
this suite discovers actually carries, read off `docker image inspect`
for `upstroke-test/git:v1`, `alpine:3.20` and `busybox:latest`.

## `const CWD_RELATIVE_PATH: &str = "/usr/local/bin:.:/usr/bin";`

A `PATH` whose second component is the working directory.

`.` explicitly rather than an empty component, so the two shapes
`cwd_dependent_path_components` classifies are exercised by different
fixtures rather than by one.

## `fn image_environment() -> ContainerEnvironment {`

The image environment the fake fixtures compose over.

Explicit rather than `ContainerEnvironment::inherited()`, which is now a
base that refuses: DESIGN.md:260 has the runner supply `PATH`, and an
empty base supplies nothing.

## `struct Scripted {`

-----------------------------------------------------------------------
A runtime that can finish, and that a test keeps a handle on
-----------------------------------------------------------------------

## `struct Scripted {`

The fake, wrapped so a test can hold it while the runner owns it, and so
a container can be made to **finish**.

`FakeRuntime::start` leaves a container `Running` and nothing in a
synchronous `Runner::run` could move it afterwards, so the success path
would be unreachable and only the timeout path would ever be measured. A
decorator that exits the container at `start` — and, when asked, gives
it an exit status and output — is what makes both paths constructible;
the plain fake still drives the timeout.

## `impl Runtime` › `fn scripts(&self, execution: ContainerExecution) -> Self {`

What every container of this runtime reports when it finishes.

## `struct Fixture {`

-----------------------------------------------------------------------
A realistic run layout
-----------------------------------------------------------------------

## `struct Fixture {`

One run, laid out where the engine really puts things.

Every path here comes from the type that owns it — [`RunPaths`] for the
two halves of a run directory, `workspace_manager::execution_root_of`
for the worktrees — rather than from string literals, so a layout change
moves the fixture with it. A hand-built layout is a fixture that keeps
passing after the thing it describes has moved.

## `struct Layout {`

The fixture's paths, from the root its scratch tree holds: what `Fixture::new` builds and what a
bounded child builds for `Fixture::built_in_a_bounded_child`, derived once so the two cannot
disagree about where anything is.

## `impl Layout` › `fn build(&self) {`

The fixture's tree on disk, in the order the fixture always built it: the repository and its two
commits, the run's two halves, the public log and a private artifact, the container namespace,
the three worktrees, the sibling's file. The repository and the worktrees are Git through the
view fixture, which runs it through `HostRunner`: host launches.

## `#[cfg(unix)]` › `fn fixture_tree_child() {`

The isolated builder `Fixture::built_in_a_bounded_child` runs: `Layout::build` over the root its
parent names. Ignored, so libtest never runs it in its own process.

## `impl Fixture` › `fn built_in_a_bounded_child(tag: &str, exit_on_start: bool) -> Self {`

`Fixture::new` with the tree built by `fixture_tree_child` under `run_test_isolated` and
`FIXTURE_CHILD_BOUND`, the scratch tree acquired and kept here. It requires the child's success
and libtest's `1 passed`. The six reaper tests this follow-up adds build their fixture this way
(`FUA-I4-RELAY`, PR #328's implementation review round 4): the fixture's Git is ten host
launches, each ending in a wait for its reaper's acknowledged exit that is unbounded by design,
and made in the test's own process one stopped reaper held the test, and the suite, for as long
as it stayed stopped. In the child it holds only the child, which the deadline kills. The suite's
other users of `Fixture::new` predate this follow-up and are unchanged.

## `impl Fixture` › `fn confinement(&self) -> Confinement {`

Everything this run withholds.

**The two sibling worktrees are no longer added by hand.** They were
until repair R1, and `PR6-CORRECTNESS-013` is what that cost:
`Confinement::of_run` named no worktree path at all, so the helper
production uses withheld nothing about worktrees and only the
fixtures pretended otherwise. `of_run` now derives the execution
root and its three namespaces, which is what makes this method a
plain delegation — and what makes the sibling assertions in
`the_mount_set_is_the_roles_own_and_reaches_nothing_of_the_coordinators`
statements about the production helper rather than about the
fixture.

The repository's Git it is given, like every runner and mount context
the fixtures here build, is `resolve::fixtures::plain_checkout` over the
repository's `.git`: what Git reports for an ordinary checkout, whose
`.git` is both its Git directory and its common directory. A test about
a relocated common directory names that directory itself.

## `impl Fixture` › `fn withheld(&self) -> Vec<(Withheld, PathBuf)> {`

The concrete host paths this run withholds, as a table a test can
iterate — derived from the same accessors the layout is built from.

## `fn requests(workspace: &Path) -> Vec<RunnerRequest> {`

A request in every role, over one workspace, with the binding each role
takes in production.

## `fn hostile_bindings(workspace: &Path) -> Vec<RunnerRequest> {`

Requests whose **role and agent binding are varied independently**.

`runner::gate_request` and `host::shell_probe_request` bind no agent, so
a grid built only from the production builders never asks the question
the role rule exists to answer: what happens to a role that takes no
credentials and names an agent anyway. `host-v1`'s own
`reserved_values` says it in as many words — "neither is told where an
agent's credentials live, **whatever agent the request happens to
name**" — and until this grid existed, deleting the role check from the
container's mount plan changed nothing any test could see (measured:
mutation `M8-credential-volume-for-every-role` survived the whole
suite). That is `PR4-CONF-002`'s class exactly: a predicate keyed on a
field no fixture varies on its own.

## `fn sources(mounts: &[Mount]) -> Vec<PathBuf> {`

Every host path a mount hands over.

## `fn sources(mounts: &[Mount]) -> Vec<PathBuf>` › `Mount::Volume { .. } | Mount::Tmpfs { .. } => None,`

Neither carries a host path, so neither can hand one over.

## `fn the_view_path_the_census_prunes_is_the_one_the_invocation_mounts() {`

-----------------------------------------------------------------------
1. Mounts, and the negative space
-----------------------------------------------------------------------

## `fn the_view_path_the_census_prunes_is_the_one_the_invocation_mounts() {`

The mount set is the role's one worktree, its view, its borrowed object
store and its credential volume — and **nothing that reaches the
coordinator**.

Both halves, because either alone passes on a wrong implementation:
a positive check ("the worktree is mounted") passes on a container that
also mounts `/`, and a negative check alone passes on a container that
mounts nothing at all. The withheld set is derived from [`RunPaths`] and
`workspace_manager::execution_root_of`, so it moves when the layout does.

Second field held constant: the role (`Implement`) and the agent
binding; what varies is which withheld path is offered.
The view path an invocation **mounts** is the view path a census
**prunes**, taken from the plan the runner actually builds.

`<R>/views/<container-name>` is a convention with a producer in one lane
and a consumer in another, and the six intent fields the packet fixes
carry no view path — so the census has to *derive* it. An independent
review measured what two copies cost: changing only the producer to
`<R>/views-v2/<name>` passed the entire suite while silently orphaning
every view the census would have pruned, which is R19 quietly ceasing to
balance after a crash. There is now one definition; this is what fails
if a second one appears.

The oracle is not either function: the expected value is the literal
`<R>` joined with `views` joined with the name, written here.

Second field held constant: one run identity and one private root; the
only thing that moves is the invocation, and with it the container name.

## `fn the_view_path_the_census_prunes_is_the_one_the_invocatio…` › `let expected = fixture.private_root.join("views").join(name.as_str());`

The literal convention, written out rather than called.

## `fn the_view_path_the_census_prunes_is_the_one_the_invocatio…` › `assert!(`

And the mount the container is actually given carries that path,
so this is the path on the machine and not only in a field.

## `fn the_mount_set_is_the_roles_own_and_reaches_nothing_of_th…` › `let mounts = plan.mounts();`

Positive: five mounts, each with its target and its disposition.

## `fn the_mount_set_is_the_roles_own_and_reaches_nothing_of_th…` › `assert_eq!(`

The scratch surface is a tmpfs and therefore carries **no host
source**: it is the one writable place that is neither the role's own
worktree nor anything of the coordinator's, which is what lets
`CreateSpec::read_only_root` close the container layer without making
`sh` unusable.

## `fn the_mount_set_is_the_roles_own_and_reaches_nothing_of_th…` › `let withheld = fixture.withheld();`

Negative: no mount source is a withheld path or an ancestor of one.

## `fn the_mount_set_is_the_roles_own_and_reaches_nothing_of_th…` › `let hostile = vec![Mount::Path {`

The control: the same check over a mount set that *does* reach the
coordinator finds every category. Without it a `violations` that
always returned an empty vector would pass the assertion above.

## `fn an_agents_own_settings_file_is_bound_read_only_and_named_by_its_container_path() {`

PR12-CONTAINER-CLAUDE-SETTINGS-HOST-PATH, narrowed by root's D-A. A worker
and a reviewer each record their own materialized settings file as
`CommandSpec::settings_file`, and name it in their arguments. Each receives
exactly that file, bound read-only at `/upstroke/settings/<name>`, and its
argument names that container path instead of the host's. The two files stay
distinct: each container has one settings mount, its own.

Nothing else changes:
- the role's own mounts are those of the census above, with the
  coordinator's user set on every one of these containers;
- every withheld path stays withheld: the run's private half, the settings
  directory included, is mounted nowhere.

## `fn only_the_invocations_own_settings_file_is_bound_and_no_argument_grants_another() {`

Root's D-A negative cases. Another invocation's settings file named in a
worker's argv gets no mount, and its argument stays the host path it was:
only the intended argument is rewritten. Nor does any of these get a
settings mount, and each keeps its own argv:
- an argument naming a settings file with no metadata, the shape the first
  version of the bind (`b87af4f6`) mounted;
- metadata that no argument names, as a `codex` or `copilot` command has;
- a gate, and an agent probe, carrying both.

## `fn an_own_settings_file_that_is_not_a_regular_file_of_the_runs_settings_directory_is_refused() {`

The file identity root's D-A keeps. Recorded metadata naming a directory in
the settings directory, a link there to a private transcript (Unix), or a
file elsewhere in the private half refuses the plan, each for its reason, and
before any effect. The control is the regular file the invocation
materialized, which plans. The link case is built on Unix only and chained in
as an `Option`, so a Windows build compiles the list without a mutable binding
it never mutates (V225 found the `unused_mut` the earlier spelling left there).

## `#[cfg(target_os = "linux")]` › `fn effective_ids() -> ContainerUser {`

The oracle for the coordinator's user on Linux, read another way than
production reads it: the effective field of `/proc/self/status`' `Uid:` and
`Gid:` lines, where `coordinator_user` asks `geteuid` and `getegid`.

## `#[cfg(all(unix, not(target_os = "linux")))]` › `fn effective_ids() -> ContainerUser {`

On the other Unix targets the oracle is the same two queries production
makes, so there the test shows that every role and probe carries the
effective ids and that the argv says so, not that the queries are right. It
runs only where those targets build, in native CI: a Linux run is no macOS
result.

## `#[cfg(unix)]` › `fn every_container_is_created_as_the_coordinators_user() {`

Root's D-B, focused. Both probes, the worker, the gate and the reviewer are
each created as the coordinator's effective uid and gid, in the spec and in
the `docker create` argv, where `--user` comes before the image. The mount
census and the confinement above hold with the user set.

## `#[cfg(not(unix))]` › `fn every_container_keeps_the_images_user_off_unix() {`

On Windows nothing is mapped, and no container is given `--user`.

## `fn prompt(tag: &str, bytes: usize) -> Vec<u8> {`

A payload of `bytes` bytes that starts with a marker and its tag and goes on
with xorshift bytes, so two prompts differ and a payload larger than a pipe
buffer is not a run of one byte.

## `fn a_commands_stdin_reaches_its_own_container_exactly_and_through_nothing_else() {`

Root's D-C on the fake. A worker's 200 KiB payload, larger than a pipe
buffer, and a reviewer's own reach their containers byte for byte, each its
own. Each container is created with stdin open, and the marker the prompts
carry is in no argv, environment entry, label, mount or `docker create`
argument. A gate and both probes, which carry no input, keep a closed stdin,
and their starts are fed nothing.

## `fn a_workspace_that_contains_a_withheld_path_is_refused_before_any_effect() {`

A workspace that contains a withheld path is refused, by name, before
anything is created.

This is the assertion a membership test cannot make. The repository root
contains the public log and authoritative Git; `/` contains everything.
Both are plausible values for `RunnerRequest.workspace` — the second is
what a path-joining mistake produces — and both are refused with the
paths named.

Second field held constant: the role, the agent and the image; what
varies is the workspace.

## `fn a_workspace_that_contains_a_withheld_path_is_refused_bef…` › `"the filesystem root",`

The **volume** root of the fixture's own tree, not a bare
`Component::RootDir`. `Path::starts_with` is component-wise,
and on Windows `C:\\x` begins with a `Prefix` component that
a bare `\\` does not have — so a bare root contains nothing
there and the refusal would not fire. Measured on the
Windows guest, where the first spelling of this row was the
slice's only guest failure.

## `fn a_workspace_that_contains_a_withheld_path_is_refused_bef…` › `assert!(`

And nothing was created on the way to any of those refusals: the
refusal is in `plan`, which performs no effect at all.

## `fn only_the_reviewer_receives_a_read_only_worktree() {`

Only the reviewer's worktree is read-only.

DESIGN.md:610: "a `:ro` mount makes the reviewer's read-only
**mechanically** perfect instead of flag-deep." A count, not a spot
check: exactly one of the five roles gets `:ro`, and the other four do
not — a runner that made every mount read-only would pass a test that
only looked at the reviewer.

Second field held constant: the workspace, the image and the agent
binding each role takes in production; what varies is the role.

## `fn only_the_reviewer_receives_a_read_only_worktree()` › `assert_eq!(`

The two probe roles receive no worktree at all — a probe has none.

## `fn every_role_and_probe_gets_the_recorded_mounts_and_review_and_probes_get_them_read_only() {`

The per-role census for configured mounts, over all five roles of
`requests()`. Each gets both recorded mounts with the recorded source and
target. The read-write one is read-only for exactly `probe(claude-code)`,
`probe(shell)` and `review`, and as recorded for `implement` and `gate`:
a gate is given an explicitly read-write mount. Each role's own mounts are
exactly what the same request plans without configured mounts, and the
confinement finds nothing.

## `fn the_docker_argument_names_exactly_the_recorded_mount() {`

For every role, the `--mount` arguments `docker create` receives contain
exactly one per planned mount. For each configured mount the argument is
`type=bind,source=<the recorded source>,target=<the recorded
target>[,readonly]`, character for character, so the record and the
runtime's argument cannot name different things.

## `fn a_recorded_mount_that_moved_or_reaches_a_withheld_path_refuses_the_plan_before_any_effect() {`

The plan-time recheck. The recorded source moved away refuses the plan
(`is gone`) and the invocation; moved back, the plan succeeds again. A
record whose mount names the private root, with that root's true
identity, refuses against the bound exclusions. No intent and no
container exists afterwards.

## `fn a_recorded_policy_whose_targets_a_fresh_run_refuses_starts_no_container() {`

K1 (regular review 4: recorded mounts bypass target validation), at the
launch plan. The mounted policy's first mount, read back through
`serde_json` at `/data` (`resolve::fixtures::with_recorded_targets`), plans:
the control. Each set of `resolve::fixtures::REFUSED_RECORDED_TARGETS` (the
container's root, `/upstroke/settings`, `/cache/../upstroke/settings`, and
`/cache` with `/cache/inner`) is planted the same way and is a complete
record. A gate invocation over each ends `NeverStarted`, refused for this
container with the words of the target rule. Afterwards the runtime's
journal holds no create, no intent was written and no container exists:
the plan refuses before `Reaping::cover`, so no reaper is armed either.

With the shared `resolve::refuse_recorded_targets` validator bypassed, the root cell's invocation
runs to exit 0 and the test fails (measured before the commit).

## `fn a_recorded_mount_inside_the_common_directory_the_runner_is_given_refuses_the_plan() {`

J10 (regression review 2, finding 1), at the runner: the common directory
is relocated outside the checkout, and the mounted policy's first mount is
re-recorded at its `refs`. Over the checkout's own Git
(`resolve::fixtures::plain_checkout`, as before the repair) the gate plans: the
source is an ordinary directory there. Given that relocated directory as
the run's common directory, as the entry gives the managed repository's,
the same policy refuses the plan, naming the directory as authoritative
Git, with no intent and no container.

## `fn a_configured_mount_that_lies_inside_a_withheld_directory_is_refused_and_a_roles_own_is_not() {`

J10's second half, at the confinement, with no runner and no repository:
a confinement withholding a common directory outside the checkout reports
a configured mount that is it, contains it or lies inside it, once each,
naming the relation and the authoritative-Git passage. The rule for a
role's own mounts stays one-directional: `violations` refuses the first
two and not the third, which is how a role's object store lies inside the
common directory. A directory beside every withheld path is neither.

## `fn a_mount_the_runtimes_option_cannot_carry_exactly_refuses_the_plan_before_any_effect() {`

Generated mounts too: a workspace whose path holds a comma, a quote or a
trailing space cannot be spelled in a `--mount` option exactly, and the
plan refuses naming the mount, where the earlier code would have bound a
different path or failed inside `docker create`. No intent, no container.

## `fn the_credential_volume_is_mounted_exactly_when_its_location_is_supplied() {`

The credential volume is mounted **exactly** when its location is
supplied, and both follow one predicate.

The intersection that makes this worth writing: {role} × {volume
recorded}. A rule keyed only on the role mounts a volume the record does
not name; a rule keyed only on the record hands a gate an agent's
credentials. And the mount and the environment variable are asserted to
agree cell by cell — two rules that happen to agree today is the shape
this project keeps paying for.

## `fn the_credential_volume_is_mounted_exactly_when_its_locati…` › `let in_env = key.and_then(|key| {`

`filter(non-empty)`: since `PR6-CORRECTNESS-007` a location
the role is **not** given is named with nothing rather than
omitted, because `docker create --env` overlays the image's
environment and an omitted key is one the image decides. So
"supplied" is a value and not a presence.

## `fn the_credential_volume_is_mounted_exactly_when_its_locati…` › `assert_eq!(`

The one predicate, asserted rather than assumed.

## `fn the_credential_volume_is_mounted_exactly_when_its_locati…` › `let mut hostile_cells = 0_usize;`

The cell the production builders cannot reach: a role that takes no
credentials, carrying an agent whose volume the record **does** name.
Without it the role check in the mount plan is unmeasured, because
`agent.is_some()` already excludes every such role in production.

## `fn the_credential_volume_is_mounted_exactly_when_its_locati…` › `assert_eq!(`

Named with **nothing**, not omitted: an omitted key is one the
recorded image decides, and this role must be told it has no
location rather than left to inherit one
(`PR6-CORRECTNESS-007`).

## `fn every_container_is_created_from_the_recorded_id_even_after_the_reference_moves() {`

-----------------------------------------------------------------------
2. Creation from the recorded image id
-----------------------------------------------------------------------

## `fn every_container_is_created_from_the_recorded_id_even_after_the_reference_moves() {`

Every container is created from the **recorded id**, and a moved
reference does not change what executes.

The intersection: {image id recorded} × {reference moved}. A runner that
resolved the reference at each invocation passes every test that never
moves the tag, which is every test that does not build this cell.

## `fn every_container_is_created_from_the_recorded_id_even_aft…` › `fixture`

The reference now names another image, and the old id stays.

## `fn every_container_is_created_from_the_recorded_id_even_aft…` › `fixture.trace.clear();`

And it really runs from the recorded id. The trace is cleared first
so the fixture's own `image_by_reference` — which is how the moved
tag was verified above — cannot be mistaken for one the runner made.

## `fn a_substituted_reported_image_id_refuses_before_start_in_both_phases() {`

A reported image id that differs from the record never reaches
`Container.Start`, and **the two phases arrive at the caller as
different things**.

`expected_failures_refusals[3]` gives the mismatch two outcomes: "refused
before start (**pre-flight/rebuild**)" or "settled as a
**`RunnerSpawnFailure` outage** (mid-run)". The shipped code returned one
`UpstrokeError::Refused` for both, so the mid-run half was unreachable to
any caller — `PR6-CORRECTNESS-001`, whose surviving mutation was to
change the variant and keep the message, because the test checked only
`expect_err`, substrings, `Start`'s absence and cleanup.

So the grid is `{pre-flight probe, in-run worker, in-run integration
sequence} × {mismatch}` and the assertion is on the **variant**, not on
prose. What is common to every cell — never started, nothing left behind
— is asserted in every cell too, because a fix that distinguished the
phases by *starting* one of them would otherwise pass.

The settlement event itself is PR7's: `invariants_introduced` makes the
container transition "test-only until PR7 wires TopologyRun", and
`src/topology/**` is frozen. What this slice owes is the distinction,
and `ImageIdMismatch` is where PR7 reads it.

Second field held constant: the runtime is reachable throughout and the
same image id is substituted in every cell, so only the invocation's
phase moves.

## `fn a_substituted_reported_image_id_refuses_before_start_in_…` › `assert_eq!(`

The phase, as the error's own shape. A caller settles from this.

## `fn a_substituted_reported_image_id_refuses_before_start_in_…` › `assert!(`

Before start, and that is asserted as an absence rather than as
an error having come back.

## `fn a_substituted_reported_image_id_refuses_before_start_in_…` › `assert!(`

R26 and R19 balance: no container, no intent, no view.

## `fn the_image_mismatch_phase_is_read_from_the_invocation() {`

The phase is read from the invocation, over every form an invocation has.

The oracle is an independent table over `InvocationId`'s three variants —
derived from the type, not from `ImageIdMismatch::of` — and the
distinct-value count is asserted, so a classifier that collapsed to one
answer fails here whatever it collapsed to.

## `fn the_image_mismatch_phase_is_read_from_the_invocation()` › `let name =`

And the error the classification builds carries it in its variant
rather than in prose.

## `fn a_policy_that_is_not_a_container_policy_is_refused_at_construction() {`

A policy that is not a usable container policy is refused at
construction, before a runner exists to execute anything.

## `fn a_policy_that_is_not_a_container_policy_is_refused_at_co…` › `let runner = ContainerRunner::new(`

The control: the good one is accepted, and its digest is the record's.

## `fn the_shell_probe_runs_through_this_runner_as_a_registered_container_invocation() {`

-----------------------------------------------------------------------
3. Probes through the runner
-----------------------------------------------------------------------

## `fn the_shell_probe_runs_through_this_runner_as_a_registered_container_invocation() {`

The `RunnerPreflight` shell probe executes through **this** runner, as a
registered container invocation created from the recorded image id.

`decisions.sequential_substrate.runner`: "both implement the
RunnerPreflight shell probe (the recorded shell executing `exit 0`
through the Runner: on the host as an ordinary supervised process, **in
a container from the recorded image id**)". The probe is not
re-implemented here — `host::run_shell_probe` is a free function over
`&dyn Runner`, and this is the same call the host makes, with the runner
varied and everything else held fixed.

## `fn the_shell_probe_runs_through_this_runner_as_a_registered…` › `let rendered = fixture.trace.rendered();`

It was a container invocation, in the contract's order, from the
recorded id — and the intent that owns it was written first.

## `fn the_shell_probe_runs_through_this_runner_as_a_registered…` › `assert!(invocation.probe_target().is_some());`

The command really was the recorded shell executing `exit 0`, and the
probe carries a probe-role identity.

## `fn the_shell_probe_runs_through_this_runner_as_a_registered…` › `assert!(`

A registered invocation: the intent named it, and the record carried
the runner digest. The intent is gone now, so the evidence is the
container the runtime saw.

## `fn failing_preflight_probe_on_resume_refuses_before_recovery_event_and_reclaims_probe_containers() {`

T-CONTAINER (17), PR6 half: a failing pre-flight probe refuses and its
probe containers are reclaimed.

**What PR7 completes**, and this test does not claim: the *ordering*
against a recovery event ("refuses before any recovery event") and the
resume that produces one. `decisions.pr_sequence[8].scope` puts "rebuild
of the recorded Runner … with **RunnerPreflight before any recovery
event**" in PR7, and this slice's `permitted_transitions` says the
container transition is "test-only until PR7 wires TopologyRun". What is
held here is the half the mechanism owns: the probe spawn is the only
thing that observes the failure, the refusal names the shell, and the
probe's container, view and intent are all gone afterwards so the run
stays resumable.

Both probe kinds, because `expected_failures_refusals` names both — "a
recorded **shell or agent CLI** that fails inside the recorded image".
Second field held constant: the image id, which matches the record in
every cell, so what varies is only what the process did.

#### Both cells go through the thing that turns failure into refusal

`PR6-ENUM-007`. The agent cell called `runner.run` directly and asserted
a nonzero `ProcessOutput`, which is not what the clause says: the shell
cell reaches its refusal through `host::run_shell_probe`, and the agent
half's equivalent is [`AgentAdapter::probe`], which is where a nonzero
`--version` becomes a `UpstrokeError`. Deleting that refusal left this
named test green, because "the runner returned code 1" was all it
asked. It now drives `ClaudeCodeAdapter::probe` over the container
runner and asserts the **refusal**, so the cell holds
`proof_tests[3]`'s "an agent probe **fails** when the CLI is absent".

## `fn failing_preflight_probe_on_resume_refuses_before_recover…` › `if tag == "shell" {`

The observation is a spawn, not an inspection: `non_goals[2]` is
"non-spawn shell/CLI presence inspection", and the container was
created and started before anything knew the answer.

## `fn failing_preflight_probe_on_resume_refuses_before_recover…` › `let adapter: &dyn crate::agent::AgentAdapter = &crate::agent::claude::ClaudeCodeAdapter;`

Through the adapter, which is what turns a nonzero
`--version` into a refusal. `runner.run` alone returns
`Ok(ProcessOutput { code: Some(1) })` — a spawn that
succeeded — and a run that stopped there would settle a
resume as *certified* on a CLI that is not there.

## `fn failing_preflight_probe_on_resume_refuses_before_recover…` › `assert!(`

**The `--version` refusal specifically.** `probe` runs
`--version` and then `--help`, and the second has refusals of
its own; an `expect_err` alone is satisfied by either, so
deleting the nonzero-exit check on `--version` still left
this cell green (measured). The clause is "an agent probe
**fails when the CLI is absent**", and what observes that is
the first spawn's exit status.

## `fn failing_preflight_probe_on_resume_refuses_before_recover…` › `let request = crate::agent::probe_request(`

The control: the spawn itself succeeded, so the refusal is
the adapter's reading of the result and not a spawn failure
dressed up as one.

## `fn failing_preflight_probe_on_resume_refuses_before_recover…` › `assert!(`

The probe containers are reclaimed, and the run stays resumable:
no container, no intent, no view.

## `fn failing_preflight_probe_on_resume_refuses_before_recover…` › `assert!(fixture.paths.events().exists());`

And the run's own record is untouched by any of it.

## `fn two_incarnations_of_one_probe_are_two_container_invocations() {`

One probe identity, two incarnations, two container invocations.

The intersection {probe kind} × {epoch}. `InvocationId::probe` is
deterministic **by construction**, so the same probe of a resumed run
carries the same identity; without the incarnation in the name the
second epoch's intent would overwrite the first's and the census would
lose the evidence it needs. This is that property at the *runner* level:
two runners differing in nothing but the incarnation.

## `fn two_incarnations_of_one_probe_are_two_container_invocati…` › `assert_eq!(shell_probe_id().render(), shell_probe_id().render());`

The identity repeats across incarnations — which is why the name may
not — and the fixture proves that rather than assuming it.

## `fn probe_and_execution_compose_through_one_code_path() {`

-----------------------------------------------------------------------
4. Environment composition, and parity with the host
-----------------------------------------------------------------------

## `fn probe_and_execution_compose_through_one_code_path() {`

Probe and execution compose through **one** code path, and produce the
same environment.

DESIGN.md:263. "Two call sites that happen to agree today" is the shape
this sentence is most often satisfied by, so both halves are asserted:
a source census that there is one composition site and one plan site in
this module's production region, and a runtime comparison of the pair
the sentence names.

The one difference is stated rather than hidden: a probe receives no
worktree ([`receives_a_worktree`]), so its mount set is the execution's
minus the worktree, the view and the borrowed object store. Everything
that decides what the process *is* — the image id, the credential
volume, the reserved values, the overlay — is identical.

## `fn probe_and_execution_compose_through_one_code_path()` › `let source = std::fs::read_to_string(`

(a) the source census.

## `fn probe_and_execution_compose_through_one_code_path()` › `let fixture = Fixture::new("parity-probe", true);`

(b) the pair the sentence names, composed.

## `fn probe_and_execution_compose_through_one_code_path()` › `let composed = |plan: &InvocationPlan| -> BTreeMap<String, String> {`

The overlay differs only where the *request* differs, so the
composed environments are compared as sets of (key, value).

## `fn probe_and_execution_compose_through_one_code_path()` › `let probe_targets: BTreeSet<&str> = probed.mounts().iter().map(Mount::target).collect();`

Mounts: the probe's set is the execution's minus the worktree.

## `fn host_and_container_compose_the_same_environment_for_every_role() {`

`decisions.tests_acceptance.parity`: "host and container runners produce
identical … **environment composition**".

The runner is varied and **everything else is held fixed**: one explicit
base, one name rule, one overlay, and all five `ExecutionRole` values
including both probe targets — `ExecutionRole::all()` returns five for
exactly this reason. The base is explicit rather than each runner's own,
because the two bases are *supposed* to differ (the Upstroke environment
and the image environment) and a comparison of those would be a
comparison of two fixtures rather than of two composition rules.

One place they legitimately differ is stated as an assertion rather
than skipped: a credential *location* is a path at the boundary that
executes, so the host names a host directory and the container names its
mount target. Both are supplied for exactly the same three roles. Since
PR12's Q2 there is a second, stated the same way: the host environment's
default object graph (`Recorded`) composes the process-wide
`GIT_NO_REPLACE_OBJECTS`, and the container composes no process-wide
replacement setting, because the role's projected view states that policy
in its own `config`.

## `fn host_and_container_compose_the_same_environment_for_ever…` › `.remove(crate::workspace_manager::NO_REPLACEMENT_OBJECTS.0)`

The replacement-objects delta (PR12, Q2), named rather than skipped: the
host's entry is taken out of the comparison and its value asserted (`1`),
and the container's composed vector is asserted to carry no such key.
Every other key is still compared exactly below (`env.md`, "No
process-wide replacement setting since PR12").

## `fn host_and_container_compose_the_same_environment_for_ever…` › `let withheld: BTreeMap<String, String> = container_env`

Same keys, for every role — **plus** the locations the container
withholds explicitly.

This is the one structural asymmetry between the two boundaries
and it is stated rather than skipped (`PR6-CORRECTNESS-007`): the
host runner calls `env_clear()` and installs the composed vector
as the *whole* environment -- less, in a work tree the engine
made, the work-tree context it sheds there, the four selectors,
and with a relative `GIT_COMMON_DIR` given as the absolute path
it names at the started checkout -- so a key it omits is
genuinely absent; the container runner's vector is `docker create --env`,
which **overlays** the image's environment, so a key it omits is
a key the image decides. Naming what is withheld is how the
container reaches the same *effective* environment the host
reaches by omission — which is what parity is about.

## `fn host_and_container_compose_the_same_environment_for_ever…` › `let location = agent.as_ref().and_then(host::credential_location);`

Same values everywhere except the credential location.

## `fn host_and_container_compose_the_same_environment_for_ever…` › `for reserved in host::reserved_keys() {`

And both refuse the same overlay keys.

## `fn host_and_container_compose_the_same_environment_for_ever…` › `assert!(base.iter().any(|(key, _)| key == "CLAUDE_CONFIG_DIR"));`

The base really did carry a credential location, so "the reserved
copies are dropped" is a statement about this fixture.

## `fn a_completed_invocation_releases_in_the_contracts_order_and_reports_the_result() {`

-----------------------------------------------------------------------
5. Supervision, release, and the resource ledgers
-----------------------------------------------------------------------

## `fn a_completed_invocation_releases_in_the_contracts_order_and_reports_the_result() {`

A completed invocation stops, removes, unmounts the view and removes the
intent — in that order — and reports what the container did.

`side_effect_vs_event_ordering`: "stop/rm, view removal, intent removal
**after completion**". Asserted as a sequence of positions in one
ordered trace, not as membership: a release that performed the same four
operations in any other order would satisfy a set.

Second field held constant: the image id, which matches the record; what
varies is only that the container finished.

## `fn a_completed_invocation_releases_in_the_contracts_order_a…` › `let order = [`

The whole sequence, in one chain.

## `fn a_completed_invocation_releases_in_the_contracts_order_a…` › `assert!(`

The three clauses of `side_effect_vs_event_ordering`, each stated on
its own rather than only as a link in the chain above — a chain is
one assertion and the contract is three predicates.

## `fn a_completed_invocation_releases_in_the_contracts_order_a…` › `assert!(at("view:materialized") < at("rt:create"), "{rendered:#?}");`

And the view really is materialised before the create, which is the
physical constraint the module docs record.

## `fn a_completed_invocation_releases_in_the_contracts_order_a…` › `assert!(at("rt:collect") < at("rt:remove"), "{rendered:#?}");`

Collected **before** the release, because `docker logs` answers for a
running container and not for a removed one.

## `fn a_completed_invocation_releases_in_the_contracts_order_a…` › `assert!(fixture.runtime.fake().container_names().is_empty());`

R26 and R19 balance.

## `fn a_container_that_outlives_its_timeout_is_stopped_and_removed() {`

A container that outlives its timeout is stopped and removed, and the
output says so.

`slice_contract.cancellation`: "timeout or shutdown **stops and
removes** the container". The fixture's timeout is `Duration::ZERO`, so
the deadline has passed by the first observation and the supervisor
makes exactly one round trip — `determinism` forbids sleeps and a poll
loop with a real timeout would be one.

Second field held constant: everything except whether the container
terminates — the same image, the same role, the same workspace as the
completing case above.

## `fn a_container_that_outlives_its_timeout_is_stopped_and_rem…` › `let payload = prompt("timeout", 128 * 1024);`

Since PR12's D-C the timed-out command carries input, more than a pipe
buffer of it, and the timeout path is the same: the payload went to the
container at its start, and the stop and removal follow in their order.

## `fn a_container_that_outlives_its_timeout_is_stopped_and_rem…` › `let rendered = fixture.trace.rendered();`

Stopped and removed, in that order, and the ledgers balance.

## `fn a_container_that_outlives_its_timeout_is_stopped_and_rem…` › `assert_eq!(`

Exactly one observation: no sleeps, and the loop is bounded by the
deadline rather than by a count.

## `fn output_beyond_the_bound_is_truncated_and_reported_as_limited() {`

Output beyond the bound is truncated and reported as limited.

Without it `ProcessOutput::output_limited` would be `false` for every
container invocation, and `host::run_shell_probe`'s bounded-output
refusal — a real arm of that function — would be reachable at the host
boundary and unreachable at this one. A pre-flight that certifies less
than the one it is paired with is not the parity the packet asks for.

Second field held constant: the exit status, which is 0 in both cells,
so what varies is only how much the container printed.

## `fn output_beyond_the_bound_is_truncated_and_reported_as_lim…` › `let probe = host::run_shell_probe(`

And the probe refusal really is reachable at this boundary.

## `fn a_credential_volume_is_never_created_or_pruned_by_any_disposition() {`

R20: a credential volume is **never created or pruned by a run**, in
every disposition this runner can reach.

`resource_accounting.rows[R20]` is `operator_owned` and
`persistent_output` in **all five** `at_run_end` outcomes — `Complete`,
`Parked`, `Halted`, `BudgetExceeded`, `NoRunFinished`. A run-end outcome
is a fold over the event log and PR6 has no events at all
(`durable_events`: "none"), so what this slice can measure is the set of
dispositions the runner itself reaches, and the five outcomes differ
only in *which* of them ends the last invocation. Each is driven here
and the volume is asserted present afterwards.

The failure this prevents is one no ordinary test looks at: a runner
that tidied up a volume it mounted would destroy operator credentials,
and CLIs "rotate refresh tokens on use, and a discarded rotation forces
re-login" (DESIGN.md:612).

## `fn a_credential_volume_is_never_created_or_pruned_by_any_di…` › `type Disposition = (&'static str, fn(&Fixture));`

One way an invocation of this runner can end.

## `fn container_subtree_production_regions() -> (Vec<(String, String)>, BTreeSet<String>) {`

Every `.rs` file of the container subtree, with each file's **own**
production region.

#### Why this is a function and not three lines at a call site

`PR6-ACCT-002`, which is `PR5-R1-CFG-TEST-SHRINKS-THE-DOMAIN` for the
third time in this slice. The census below concatenated the sources and
called `production_region` **once**: that function cuts a source at its
**first** `#[cfg(test)]`, and `src/runner/container.rs` has one, so
everything appended after it — `runtime.rs`, `intent.rs`, `exec.rs`,
`env.rs`, `view.rs` — was cut away entirely. `census.rs` and `resolve.rs`
were not in the list at all. The census was reading one file and
reporting on seven, and its positive control happened to live before the
cut, so it stayed green while measuring almost nothing.

So: the directory is **enumerated**, not listed; each file is cut on its
own; and the caller is handed the per-file regions so it can assert the
domain did not shrink.
The two answers: the production regions in the domain, and the files
deliberately outside it.

Membership is derived from **where `container.rs` declares the module**,
which is the tree's own rule and is written at the top of that file:
"Keep every `#[cfg(test)]` declaration at the BOTTOM". `production_region`
cuts at the first `#[cfg(test)]`, so a `mod x;` above the cut is a
production module and one below it is test-only. Deriving it this way
rather than listing exclusions is what makes a *new* file a failure here
instead of a silent addition to either set.

A test-only file has no `#[cfg(test)]` of its own — its gate is at the
declaration site — so `production_region` of it is the **whole file**,
and including one would put every fixture's `docker volume create` into
a census of production vocabulary.

## `fn a_launch_that_fails_after_a_committed_effect_still_releases_everything() {`

Every exit of `launch` releases what it reached **even when the effect
was already committed**.

`PR6-ACCT-003`, the axis the fail-fast grid beside this one does not
carry. That grid makes a *primitive* fail — a runtime armed failing, a
view that refuses to materialise — so at every exit the effect never
happened. The funnel's other failure mode is the opposite one: it runs
the primitive and *then* consults the `After` phase
(`container::funnel`), which is what an `Injection::Error` at `After`
models and what a real `docker create` that succeeds and whose following
inspect fails does. The state at the exit is therefore strictly larger:
the record is published, the directory exists, the container exists.

The intersection is **{which site} × {effect committed}**, and the
committed column is the one that was empty. `Container.WriteIntent` is
in it because that exit was a bare `?` until this round: a `Before`
failure there has written nothing, so the exit looked harmless, and an
`After` failure leaves a durable R26 record with no container and no
view.

In every cell: R26's container and record and R19's view are all gone,
and R20's volume is untouched.

Second field held constant: the request, the role and the recorded image
id are identical in all eight cells; only the armed site and phase move.

## `fn a_launch_that_fails_after_a_committed_effect_still_relea…` › `if phase == HookPhase::After {`

The premise of the committed column: at `After` the
primitive really did run, so there was something to release.

## `fn a_launch_that_fails_after_a_committed_effect_still_relea…` › `assert!(`

R26 and R19 balance, whichever cell this is.

## `fn a_launch_that_fails_after_a_committed_effect_still_relea…` › `for (_, volume) in VOLUMES {`

R20 is untouched in every cell.

## `fn every_at_run_end_outcome_is_driven_through_its_mechanism_and_the_ledgers_balance() {`

The five `at_run_end` outcomes are **driven**, each through the
mechanism its row names, and every physical resource is checked
afterwards.

`PR6-ACCT-008`. The tables were transcribed and counted — five constant
strings, four `"released"` values, one seeded census — and a table copied
into a test is a table. There was no R19 outcome table at all, and the
R20 disposition grid asserted "one per outcome" by checking that a vector
had five elements.

#### What this slice can and cannot drive

A run-end outcome is a fold over the event log and PR6 has
`durable_events: "none"`, so `Complete`, `Parked`, `Halted` and
`BudgetExceeded` are not states this slice can enter. What each of them
*names* is a **mechanism**, and R26's lifecycle sentence enumerates
them: "released on complete (stop/rm, view removed, intent removed),
**cancel**, or **shutdown**", with `NoRunFinished` "reclaimed … at the
next write-command start". Every mechanism in that sentence is
reachable here, so the table below maps each outcome to one and the
mapping is asserted **total** — a sixth outcome, or an outcome with no
mechanism, fails here rather than being counted.

#### Per resource, not per site

INV-22 is "every physical or logical owned resource has, for every
lifecycle state, exactly one accounting class and exactly one
non-overlapping inventory row". The site-mapping test proves one row per
*effect site*, which is a different statement: it cannot see the staged
intent record, the implicitly created named volume, or a standalone
view. So the ledger here is over the **resources** a container
invocation owns, each named with its row, and each observed after every
mechanism.

## `fn every_at_run_end_outcome_is_driven_through_its_mechanism…` › `enum Mechanism {`

The mechanism a run-end outcome disposes of R19/R26 through.

## `enum Mechanism` › `Complete,`

"released on **complete** (stop/rm, view removed, intent
removed)".

## `enum Mechanism` › `Cancel,`

"…, **cancel**, …" — the invocation was stopped before it
finished on its own. `slice_contract.cancellation`: "timeout or
shutdown stops and removes the container".

## `enum Mechanism` › `Shutdown,`

"…, or **shutdown**" — the launch itself was refused and
everything it reached was released.

## `enum Mechanism` › `Census,`

R26's fifth cell: "reclaimed when the owner or its incarnation
is dead", at the next write-command start.

## `fn every_at_run_end_outcome_is_driven_through_its_mechanism…` › `const OUTCOMES: &[(&str, &str, &str, &str, Mechanism)] = &[`

`decisions.resource_accounting.rows[{R19,R20,R26}].at_run_end`,
transcribed, with the mechanism this slice drives each through.

## `fn every_at_run_end_outcome_is_driven_through_its_mechanism…` › `let mechanisms: BTreeSet<Mechanism> =`

Total: every outcome has a mechanism, and every mechanism the
lifecycle sentence names is used by an outcome.

## `fn every_at_run_end_outcome_is_driven_through_its_mechanism…` › `assert!(`

R20's premise: the operator's volume is there before anything
runs. `persistent_output` is a claim about a resource that
exists.

## `fn every_at_run_end_outcome_is_driven_through_its_mechanism…` › `let fixture = Fixture::new(&format!("outcome-{outcome}-live"), false);`

The cancellation clause: "timeout or shutdown stops and
removes the container". The container is left **running**
by the fake, so the supervisor really does reach its
deadline with something to stop — a fixture whose
container exits on start would take the ordinary
completion path and be a fifth copy of the cell above.

## `fn every_at_run_end_outcome_is_driven_through_its_mechanism…` › `fixture`

The launch is refused mid-sequence and releases what it
reached. R26's "shutdown" is the run stopping before an
invocation could finish, which is the same disposal path.

## `fn every_at_run_end_outcome_is_driven_through_its_mechanism…` › `let mut hooks = RecordingHooks::new(fixture.trace.clone());`

Seeded rather than run: the owner is a *dead* incarnation,
which by construction is not this process.

## `fn every_at_run_end_outcome_is_driven_through_its_mechanism…` › `assert!(`

The per-resource ledger, for the two mechanisms that fall
through: R26's container, R26's record (both halves), R19's
directory, R20's volume.

## `fn every_physical_resource_of_a_container_invocation_maps_to_exactly_one_row() {`

Every physical resource a container invocation owns has exactly one row.

`PR6-ACCT-008`'s second half, and INV-22's own sentence: "exactly one
accounting class and exactly one **non-overlapping** inventory row per
`decisions.resource_accounting`". The site-mapping census proves one row
per *effect site*; a site is not a resource, and the resources with no
site of their own are exactly the ones nothing was checking — the staged
intent record, the anonymous volume a `VOLUME` declaration creates, and
the named volume `docker create` creates implicitly.

The table is the resources, not the sites, and each row names where in
this tree that resource is disposed of. Everything here is asserted
against the packet's row text rather than against the code.

## `fn every_physical_resource_of_a_container_invocation_maps_t…` › `const RESOURCES: &[(&str, &str, &str, &str)] = &[`

`(resource, row, class while it exists, what disposes of it)`.

## `fn every_physical_resource_of_a_container_invocation_maps_t…` › `let rows: BTreeSet<&str> = RESOURCES.iter().map(|(_, row, ..)| *row).collect();`

(1) One row per resource, and the rows are the three this slice owns.

## `fn every_physical_resource_of_a_container_invocation_maps_t…` › `let mut by_row: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();`

(2) One class per row, over every resource in it: a row whose
resources disagreed about their class would be two rows.

## `fn every_physical_resource_of_a_container_invocation_maps_t…` › `for (_, _, class, _) in RESOURCES {`

Every class named is one `decisions.resource_accounting.classes`
declares.

## `fn every_physical_resource_of_a_container_invocation_maps_t…` › `let undisposed: Vec<&str> = RESOURCES`

(3) The R20 row is the only one with no disposer, and that is the
whole of `enforcement_domains.operator_owned`.

## `fn every_physical_resource_of_a_container_invocation_maps_t…` › `let site_names: BTreeSet<&str> = ContainerSite::ALL.iter().map(|site| site.name()).collect();`

(4) Every disposer that names a site names one of the frozen eight,
so a resource cannot be disposed of by something outside the funnel.

## `fn the_container_subtree_can_only_inspect_a_volume() {`

Nothing in the container subtree can create or prune a volume.

The runtime assertion above measures the dispositions a test drove; this
measures the *domain* — `enforcement_domains.operator_owned`: "R20
credential volumes: **never created or pruned by a run**". The seam has
one volume method and it returns a `bool`, and the `docker` CLI issues
exactly one volume subcommand, which is `inspect`.

**The domain has a control** (`PR6-ACCT-002`): every file of the subtree
is enumerated from the directory rather than listed by hand, every one
contributes a non-trivial production region, and the *set* of files is
asserted to contain the seven this slice wrote. A future `#[cfg(test)]`
hoisted to the top of any of them, or a new module added beside them,
fails here rather than silently emptying the census.

## `fn the_container_subtree_can_only_inspect_a_volume()` › `let files: BTreeSet<&str> = regions.iter().map(|(name, _)| name.as_str()).collect();`

-- the control on the domain itself --------------------------------

## `fn the_container_subtree_can_only_inspect_a_volume()` › `assert_eq!(`

The positive control: the read-only inspection really is there, so a
census that had stopped finding anything fails here rather than
reporting silence. It lives in `container.rs` **below** the old cut
point's neighbours, so it is also evidence the domain is whole.

## `fn the_container_subtree_can_only_inspect_a_volume()` › `let funnel = &regions`

-- and the vocabulary census is not the whole claim -----------------
`docker create` creates an absent named volume **implicitly**, with
no `volume create` anywhere (measured, docker 29.7.2). No search over
this subtree's text can see that, so the domain census is paired with
the guard that can: every `Mount::Volume` is re-inspected before the
create, in `container::create_container`.
`a_create_whose_named_volume_is_absent_is_refused_before_any_effect`
drives it; this asserts the guard is *in the production region*, so
it cannot be deleted while the vocabulary census stays green.

## `fn the_container_subtree_can_only_inspect_a_volume()` › `let seam = &regions`

And the seam has one volume method.

## `fn the_container_runner_is_object_safe_and_send_and_sync() {`

The container runner is a `Runner` like any other: object-safe, `Send`
and `Sync`.

PR11 turns `run` into a boxed `Send` future behind the same `&dyn
Runner` its callers hold, so a container runner that stopped being
object-safe would fail to compile here rather than at the migration —
the same guard `runner::contract::tests::the_runner_trait_is_object_safe` gives
the host.

## `fn the_container_runner_is_object_safe_and_send_and_sync()` › `let view: Box<dyn GitView> = Box::new(RoleGitView::new(ContainerTrace::off()));`

And a `GitView` is object-safe too, which is what lets the funnel
take `&dyn GitView` and this module hand it a projection.

## `type SeenIntents = Arc<Mutex<Vec<(String, Option<Vec<u8>>)>>>;`

-----------------------------------------------------------------------
5b. Repair R1: confinement, the intent capability, and cwd-independence
-----------------------------------------------------------------------

## `type SeenIntents = Arc<Mutex<Vec<(String, Option<Vec<u8>>)>>>;`

One row per invocation: the container's name, and the bytes of its
intent record — `None` when there was none, which is the state
catalogue survivor `PR6-INTENT-020` describes.

## `struct IntentPeek {`

A `GitView` that reads the container's intent record at the moment the
view is materialised.

`Container.MountGitView` runs **after** `Container.WriteIntent` and
**before** `Container.Create`, so this observes `<R>/containers` at
exactly the point the contract says the record must already be there:
"intent synced before docker create". The record is removed again when
the invocation completes, which is why a test that looked afterwards
could only ever see an absence.

## `fn materialize(&self, request: &GitViewRequest) -> Result<P…` › `let name = request`

The view directory is `<R>/views/<container-name>`, so its file
name is the container's.

## `struct FailingView;`

A `GitView` whose materialisation fails, so `Container.MountGitView` can
be the step a launch dies at.

## `fn the_public_constructor_withholds_every_category_from_every_role() {`

The **public constructor** withholds every category from every role that
receives a worktree, with no builder call at all.

`PR6-CORRECTNESS-011` / `PR6-ENUM-002`. `ContainerRunner::new` used to
default to `Confinement::none()` with the real set added by an optional
`with_confinement`, so the runner a caller gets by construction
withheld nothing: `PR6-ENUM-002`'s mutation is literally "omit
`with_confinement` at a construction site and submit
`identity.private_root` as a worker workspace", and `-011`'s is "apply
configured confinement only to `ExecutionRole::Implement`". This runner
is built with **`new` and nothing else**, and the grid crosses all four
withheld categories with all five roles, so a rule that held for one
role fails here.

The two probe roles are the other half of the grid rather than an
omission: a probe receives no worktree at all
([`receives_a_worktree`]), so a hostile workspace cannot reach it — and
that is asserted as "no mount source is under the hostile path" rather
than assumed.

## `fn the_public_constructor_withholds_every_category_from_eve…` › `let hostile: Vec<(Withheld, PathBuf)> = vec![`

One hostile workspace per category, each written from the type that
owns that path rather than read back out of `Confinement`.

## `fn the_public_constructor_withholds_every_category_from_eve…` › `let mut planned = 0_usize;`

The control: the role's own worktree plans on the same runner, so the
refusals above are about the path and not about `plan` being broken.

## `fn the_public_constructor_withholds_every_category_from_eve…` › `assert!(`

And nothing was created on the way to any of it.

## `fn the_execution_root_and_its_worktree_namespaces_are_withheld_and_one_worktree_is_not() {`

The execution root **and each of its three worktree namespaces** are
withheld, and any one worktree is not.

`PR6-CORRECTNESS-013`. `Confinement::of_run` named no worktree path at
all — the fixtures added siblings by hand, so the production helper was
unmeasured — and a Gate handed the run's execution root received every
task and merge worktree in one mount. Withholding only the root would
leave `<root>/tasks`, which still holds two, so the namespaces are
withheld too.

The expected paths are built from
[`crate::workspace_manager::execution_root_of`] and the packet's own
three namespace names, **not** read back from `Confinement::entries` —
that would be the function's own oracle.

Second field held constant: the run identity and the repository; what
varies is only which directory of the worktree namespace is offered.

## `fn the_execution_root_and_its_worktree_namespaces_are_withh…` › `let namespaces: Vec<PathBuf> = vec![`

`decisions.workspace_candidates.manager`: "tasks/k<key>-g<gen>,
merge/s<seq>", plus `snapshots`.

## `fn the_execution_root_and_its_worktree_namespaces_are_withh…` › `let confinement = Confinement::of_run(`

(a) `of_run` names each of them, under the sibling-worktree category.

## `fn the_execution_root_and_its_worktree_namespaces_are_withh…` › `let mut refused = 0_usize;`

(b) and a role handed one is refused, naming the category.

## `fn the_execution_root_and_its_worktree_namespaces_are_withh…` › `let mut planned = 0_usize;`

(c) The over-refusal control, which is what this fix could most
easily get wrong: withholding the namespace must not withhold the
worktrees inside it. Each of the run's three real worktrees plans,
for each worktree role — a container receives *one* worktree, and
which one is the engine's decision, not this runner's.

## `fn every_role_writes_and_syncs_its_own_six_field_intent_before_its_container_is_created() {`

Every role — **both probe kinds included** — writes and syncs its own
six-field intent record, and it is on disk before its container is
created.

Catalogue survivor `PR6-INTENT-020`: an agent probe container created
with no intent record passed, because the suite exercised both probe
kinds and never asserted that each writes its own record.
`T-CONTAINER.boundary` is "the RunnerPreflight shell and agent probe
containers are container invocations **like every other**", so the grid
is all five roles rather than the two the finding names.

The record is read at `Container.MountGitView`, which is between the
write and the create: after the invocation completes the record is gone,
so a test that looked afterwards could only ever see an absence. The six
field values are literals from this module's constants and from the
request's own `InvocationId` — never from `plan.launch.intent`, which is
the runner's own answer.

Second field held constant: the run, the incarnation and the image;
what varies is the role, and with it the invocation.

## `fn every_role_writes_and_syncs_its_own_six_field_intent_bef…` › `assert_eq!(`

R1 wrote this against the backslash-rewrite encoding that repair
R2 replaced with a percent-encoding. Decode through the accessor
the census itself uses, which asserts the round-trip rather than
one side of it -- strictly stronger than comparing the stored
bytes to a hand-written transform.

## `fn every_role_writes_and_syncs_its_own_six_field_intent_bef…` › `let rendered = fixture.trace.rendered();`

**Synced**, and before the create. The durability trio is counted
across the five invocations and each record's own two entries are
ordered against that container's own `create`.

## `fn a_container_is_created_and_started_only_under_its_own_intent_record() {`

A container cannot be created or started except under **its own**
published intent record.

`PR6-CORRECTNESS-012` / `PR6-ENUM-001`.
`expected_failures_refusals[6]` is "container start without an intent is
**impossible by construction**", and it was impossible only by nobody
having written the bypass: `create_container` and `start_container` were
public, took a bare `ContainerName`, and a
`ContainerRunner::start_existing(name)` added tomorrow would have
compiled. They now take an `IntentWritten`, and this pins the two things
the type system cannot say on its own:

1. the proof cannot be minted for a record that is not there, or that is
   not a `ContainerIntent`;
2. a proof for **another** container is refused, before any effect — so
   "an intent was written" cannot stand in for "this container's intent
   was written".

The third leg was a compile error with no test: `start_container` had no
parameter that names a container other than the proof. Since
`FUA-I2-MACRO-WS` it also takes the cover's start proof, which names one
too; the two must agree, and
`a_start_cover_starts_only_the_container_it_was_minted_for` holds that a
disagreement is refused before the runtime is asked.

## `fn a_container_is_created_and_started_only_under_its_own_in…` › `let fixture = Fixture::new("intent-capability", false);`

`exit_on_start: false`, so a started container stays `Running` and the
control below observes the start rather than the decorator's own
exit.

## `fn a_container_is_created_and_started_only_under_its_own_in…` › `let refusal = crate::runner::container::intent::IntentWritten::certify(&root, &mine)`

(1a) Absent: the proof cannot be minted at all.

## `fn a_container_is_created_and_started_only_under_its_own_in…` › `std::fs::create_dir_all(crate::runner::container::intent::containers_dir(&root))`

(1b) Present and not a record: still no proof. "The record could not
be parsed" and "the record is gone" are different answers and only
one of them is an absence.

## `fn a_container_is_created_and_started_only_under_its_own_in…` › `let mut hooks = RecordingHooks::new(fixture.trace.clone());`

The control: a real record certifies, so (1a) and (1b) are about the
record and not about `certify` never succeeding.

## `fn a_container_is_created_and_started_only_under_its_own_in…` › `fixture.trace.clear();`

(2) A proof for another container is refused, before any effect.

## `fn a_container_is_created_and_started_only_under_its_own_in…` › `crate::runner::container::create_container(`

The control: the same call with the matching proof creates, so the
refusal above is about the name and not about the spec.

## `fn a_start_cover_starts_only_the_container_it_was_minted_for() {`

`FUA-I2-MACRO-WS`'s binding of the start proof: a container created under its own intent, then a
start handed a proof minted for another worker's container — refused with `attempted: false`, the
intent clause's message naming the other container, nothing reaching the runtime, the container
still `Exited`; the proof minted for it then starts it. With the check removed (`r2-b1`) the
runtime is asked to start the other name and the refusal is an attempted one.

## `fn a_launch_that_fails_at_any_step_releases_everything_it_reached() {`

A launch that fails at **any** step releases everything it reached, and
answers with the original cause.

`PR6-CORRECTNESS-003` / `PR6-ENUM-003`. There are four ways out of
`ContainerRunner::launch` before a `Launched` value exists, and until
repair R1 three of them returned through `?` with nothing to release —
so `Runner::run`'s own release never ran and the container, the view and
the intent all survived. The fourth, the reported-image-id mismatch,
released **fail-fast**: a failing `Container.Stop` skipped the rm, the
view and the intent *and masked the integrity refusal*.

The grid is {failure point} × {cleanup healthy, `Container.Stop`
failing} — the intersection, because a cleanup that runs and a cleanup
that runs *after an earlier step failed* are different claims. In every
cell: the error names the original cause, and R19/R26 balance.

Second field held constant: the request, the role and the recorded image
id, which match in every cell except the one whose subject is a
mismatch.

## `fn a_launch_that_fails_at_any_step_releases_everything_it_r…` › `assert!(`

(1) The cause, never the cleanup's own failure.

## `fn a_launch_that_fails_at_any_step_releases_everything_it_r…` › `assert!(`

(2) R26 and R19 balance: nothing survives, whichever step
failed. A failing `Stop` does not stop the rm — `docker rm
--force` removes a running container — so the ledgers still
balance and the residue clause is the honest record of which
step could not be taken.

## `fn a_launch_that_fails_at_any_step_releases_everything_it_r…` › `if *point != Where::Start {`

(3) The container was never started, except in the cell whose
subject is a failing start.

## `fn a_launch_that_fails_at_any_step_releases_everything_it_r…` › `let attempts_stop = matches!(point, Where::Create | Where::Mismatch | Where::Start);`

(4) A failing `Stop` is *named* rather than swallowed, at
every exit where the cancel attempts one.

That is every exit **past** the create call, including the
one where the create itself failed (`PR6-ACCT-003`): the
funnel runs its primitive before consulting the `After`
phase, and `DockerCli::create` reads the reported image id
back afterwards, so a `Container.Create` that returns `Err`
may have left a container. The cancel cannot tell, so it
attempts the stop and the removal — both tolerant of
already-gone against a real daemon — and reports whatever the
runtime answered. This fake is armed to fail *every* stop,
including one against a container that was never created,
which is why the Create cell now names a stop failure too.

## `fn a_launch_that_fails_at_any_step_releases_everything_it_r…` › `let sites: Vec<String> = fixture`

And every later step was still attempted: the remove, the
view and the intent all have their sites in the trace
after the stop that failed.

## `fn the_working_directory_is_the_runners_own_for_every_role() {`

The working directory is the **runner's**, for every role, and never the
image's.

`PR6-CORRECTNESS-006`, the half `CreateSpec` can carry. A probe's
`workdir` was `None`, which hands the working directory to the image's
`WORKDIR` — so what a probe certified depended on a value the runner had
not read, and the finding's mutation (`None` -> `Some("/")`) changed
nothing any test could see. Both values are pinned here, so the mutation
dies in either direction.

## `fn the_working_directory_is_the_runners_own_for_every_role()` › `for request in requests(&fixture.task_a) {`

Both values are declared mounts, so no role runs in a directory the
runner did not give it.

## `fn a_cwd_relative_or_absent_path_refuses_every_role_before_any_effect() {`

A `PATH` that resolves against the working directory is refused, for
every role, before any effect.

`PR6-CORRECTNESS-006`, the half that matters. A probe has no worktree and
an attempt has one, so their working directories differ **by design**;
with a relative `PATH` component the repository's own worktree is on the
executable search path and repository content decides which `claude` the
attempt runs while pre-flight certified another. Both refusals are here
— the relative component, and the empty base that was the production
default and supplied no `PATH` at all.

Second field held constant: the workspace, the image and the run; what
varies is the base and the role.

## `fn a_cwd_relative_or_absent_path_refuses_every_role_before_…` › `assert!(`

Nothing was created on the way to any of them: the refusal is in
`plan`, which performs no effect.

## `fn a_cwd_relative_or_absent_path_refuses_every_role_before_…` › `let runner = fixture.runner();`

The control: the same runner over an absolute-only base plans and
supplies that value to every role, probe and attempt alike — which is
the property the refusals exist to protect.

## `fn every_role_gets_a_read_only_root_and_one_ephemeral_scratch_mount() {`

Every role's container gets a **read-only root** and exactly one
ephemeral scratch mount, and no other writable surface without a host
source.

`PR6-CORRECTNESS-008` / `PR6-ENUM-005`, the half a machine with no
container runtime can assert — which is the Windows guest, and which is
why it is here as well as in the gated test that runs the write.

## `fn every_role_gets_a_read_only_root_and_one_ephemeral_scrat…` › `for mount in plan.mounts() {`

Every other mount has a source the coordinator can name — a host
path or an operator-owned volume — so the mount list is the whole
of what the container may write and none of it is anonymous.

## `const PREFERRED_IMAGES: &[&str] = &[`

-----------------------------------------------------------------------
6. Docker-gated: what the fake cannot prove
-----------------------------------------------------------------------

## `const PREFERRED_IMAGES: &[&str] = &[`

The references the gated tests prefer, in order.

**These tests never pull.** `non_goals[1]` is "implicit image pull", and
a fixture that pulled would exercise the behaviour the slice forbids on
the very runtime the refusal is meant to be proven against. So the image
is *discovered* among what the machine already holds. `upstroke-test/git:v1`
is first because it is the only local image carrying both a shell and
`git`, and because its `UPSTROKE_IMAGE_MARKER` is how "the container runner
starts from the **image** environment" is measured rather than asserted.

## `const GIT_IMAGES: &[&str] = &["upstroke-test/git:v1"];`

Images that carry `git`. A subset, named separately because the
Git-view proof needs one and the others do not.

**One entry, and `alpine/git` is deliberately not the second.** That
image declares `VOLUME /git`, so every container created from it leaves
an anonymous volume behind that `docker rm --force` does not remove —
measured here, 29 of them from one run of this suite, which is
`PR6A-ANONYMOUS-VOLUMES-LEAK`. A fallback that breaks
`DOCKER-SUBSTRATE.md`'s "leave the daemon as you found it" on somebody
else's machine is worse than a loud, counted absence.

## `const MARKER_IMAGE: &str = "upstroke-test/git:v1";`

The image whose environment carries a marker this suite can recognise.

## `const CREDENTIAL_ENV_IMAGE: &str = "upstroke-test/credenv:v1";`

The image whose **own environment** sets credential-location variables.

`PR6-CORRECTNESS-007` cannot be measured against an image that sets
none: the defect is that `docker create --env` overlays the image
environment, so a key the runner omits is a key the image supplies.
`DOCKER-SUBSTRATE.md` records how it is built, from a base the machine
already holds and with no network.

## `const CREDENTIAL_ENV_CONTROL: (&str, &str) = ("GH_CONFIG_DIR", "/image/gh");`

The image variable that is **not** a credential location, so "the
withheld keys were overridden" is distinguishable from "the image
environment was wiped".

## `const ROOM_FOR_A_SLOW_CONTAINER_START: Duration = Duration::from_secs(30);`

The deadline the two real-Docker tests that run an invocation to its timeout give it: the
never-read invocation of `real_docker_a_containerized_program_reads_its_exact_stdin_then_eof` and
the gate of `real_docker_a_container_contains_a_daemonised_descendant`. An invocation's deadline
also covers Docker creating and starting its container, and a start the deadline overtakes is
refused rather than timed out. The two tests had three seconds, so a slow daemon turned the
timeout they wait for into the runner's refusal of `start`; thirty seconds is room for a slow
start and still far below what either program sleeps (`CI-FLAKE-REAL-DOCKER-START-DEADLINE`).
One value for both, named for what it is for, and
`real_docker_a_container_slow_to_start_still_reaches_its_timeout` is what fails if it shrinks
back.

## `fn skipped(reason: &str) {`

What a Docker-gated test does when there is no runtime.

It **reads** the reason rather than returning silently, so a skip that
stopped saying why would not compile.

## `fn no_image(reason: &str) {`

What a Docker-gated test does when the runtime holds no usable image.

Loud under the same variable as a missing runtime: a machine with Docker
and no image would otherwise pass these tests without touching it.

## `fn discover(docker: &dyn ContainerRuntime, preferred: &[&str]) -> Result<(String, String), String> {`

A reference the runtime holds, with its id, or the reason there is none.

## `fn real_policy(image_id: &str) -> RunnerPolicy {`

A container policy naming a real image id, and no credential volumes.

R20 volumes are **operator-owned** and `persistent_output`; a test that
created one would be creating operator state on the machine it runs on,
which is the very thing the row forbids a run from doing. So the gated
suite records none, and `a_credential_volume_is_never_created_or_pruned_by_any_disposition`
carries the volume obligation against the fake.

## `fn image_environment_of(`

The **reserved** part of the recorded image's own environment, read from
the daemon.

DESIGN.md:259-260: "the container runner [starts] from the image
environment; each supplies role-scoped `HOME`, `PATH`, and credential
locations". A gated fixture is the one place in this suite that can
honour the first clause literally, because it has a real image to read;
the fake fixtures state an equivalent base as a literal. Either way the
runner is given a base carrying an absolute-only `PATH`, which
`ContainerEnvironment::certify_path` now requires.

**Filtered to the reserved keys**, and that is the point rather than an
economy: `docker create --env` *overlays* the image environment, so a
variable the runner does not name still reaches the child. Passing the
whole image environment back would restate every one of those in the
runner's own `--env` list and make
"overlays a runner-owned base rather than replacing it" unmeasurable —
the marker assertion below would be comparing the fixture with itself.

**Not a self-oracle.** This reads the *image's* declared environment out
of the daemon; the assertions are about what a process *inside a
container* sees and resolves, which a different mechanism decides.

## `fn real_identity(root: &Path, repo: &Path, run_id: &str) -> RunIdentity {`

A `RunIdentity` for a gated test, under a scratch private root.

`run_id` is a **parameter** because the container name is
`upstroke-<repo_key>-<run_id>-<incarnation>-<invocation-hash>` and carries
no private root: two gated tests sharing a run id and an invocation
ordinal produce the same container name, and `cargo test` runs them
concurrently. Measured: the first version of this suite failed with
`Conflict. The container name ... is already in use`. In production the
run id is a ULID and the collision cannot arise; in a fixture it is
whatever the fixture writes, which is why it is written per test.

## `const GATED_RUNS: &[(&str, &str)] = &[`

One run id per gated test. Distinct by construction, and asserted so.

## `("outside", "01KZR1GATED000000000000001"),`

Repair R1. The ids carry the round rather than continuing the
`…GATED<letter>` sequence, and that is not cosmetic: a container name
is `upstroke-<repo_key>-<run_id>-<incarnation>-<invocation-hash>` and
carries no worktree, so two *trees* whose gated suites pick the same
run id and invocation ordinal fight over one container name on a
shared daemon. Measured: repair round R3 added a gated test with
`01KZGATEDG000000000000000G` at the same time this one did, and the
two collided with `Conflict. The container name … is already in use`.

## `("credenv", "01KZR3BGATED00000000000001"),`

Repair R3b, with its own round prefix for the reason above.

## `("slowstart", "01KZCFGATED000000000000001"),`

The run id of `slow_docker_start_child`'s container, for
`real_docker_a_container_slow_to_start_still_reaches_its_timeout`. `CF` for the CI flake it
guards, `CI-FLAKE-REAL-DOCKER-START-DEADLINE`, a prefix of its own for the reason above.

## `struct LeaveNoResidue {`

Leave the daemon as we found it, even if an assertion panics.

`DOCKER-SUBSTRATE.md`'s first rule. `Drop` rather than a line at the end
of each test, because the line at the end of a test does not run when
the test fails — which is exactly when a container is most likely to be
left behind.

## `fn real_docker_runs_from_the_recorded_image_id_and_composes_over_the_image_environment() {`

The container runner executes the recorded image id, composes **over**
the image environment, and runs in the role's worktree.

Three separately droppable claims against the real runtime:

* the runner **supplies** `PATH` — DESIGN.md:260, "each supplies
  role-scoped `HOME`, `PATH`, and credential locations" — and the value
  the child sees is the one the runner named. A key the runner did *not*
  name and the image did (`UPSTROKE_IMAGE_MARKER`) reaches the child
  anyway, which is what "overlays a runner-owned base rather than
  replacing it" means and is the half a `PATH` assertion alone cannot
  make.
* the adapter's overlay key lands.
* the working directory is the role's worktree mount.

**The first claim used to be its opposite** — "the composed
`CreateSpec.env` does not name `PATH`" — and repair R1 inverted it
deliberately, not to make anything pass. `ContainerEnvironment::inherited`
composed an empty base, so the *image's* `PATH` decided which binary a
bare program name resolved to; with a relative component in it, a probe
(no worktree) and the attempt it certifies (a worktree) resolve
different binaries. `PR6-CORRECTNESS-006`. The old assertion was true
and was a statement that the runner supplied nothing.

Second field held constant: the role and the workspace; what varies
across the three claims is which part of the environment is read.

## `fn real_docker_runs_from_the_recorded_image_id_and_composes…` › `let plan = runner.plan(&request).expect("plans");`

The runner supplies `PATH`, and every component of it is absolute.

## `fn real_docker_runs_from_the_recorded_image_id_and_composes…` › `assert!(`

And it did not *replace* the image environment: the marker key is one
the runner never names, and it is read back from inside the container
below. Without this the `PATH` assertion above would be consistent
with a runner that had thrown the image environment away.

## `fn real_docker_runs_from_the_recorded_image_id_and_composes…` › `assert_eq!(`

R26 and R19 balance against the real daemon.

## `fn real_docker_refuses_a_reviewer_write_to_its_read_only_mount() {`

`expected_failures_refusals`: "**reviewer write attempt fails**".

DESIGN.md:610: "a `:ro` mount makes the reviewer's read-only
*mechanically* perfect instead of flag-deep". The control is the same
command in the `Implement` role over the same workspace: it writes, and
the file appears on the host. Without it, a test in which nothing could
write would pass.

Second field held constant: the command, the image and the workspace;
what varies is the role.

## `fn real_docker_refuses_a_reviewer_write_to_its_read_only_mo…` › `let spec =`

The redirection is captured **inside** the container, because
`DockerCli::collect` returns only what `docker logs` wrote to its
own stdout and discards the container's stderr entirely
(`PR6A-DOCKERCLI-MERGES-STDERR-INTO-STDOUT`). Measured on docker
29.7.2: `docker logs` really does separate the two streams, so
that is a repairable defect in the CLI adapter rather than a
property of the runtime — and this test does not depend on
either way.

## `fn real_docker_refuses_a_reviewer_write_to_its_read_only_mo…` › `outcomes.push((`

Both streams, because `DockerCli::collect` merges the container's
stderr into its stdout — `docker logs` interleaves them on a
container without a TTY. Recorded as
`PR6A-DOCKERCLI-MERGES-STDERR-INTO-STDOUT`; the assertion is
written so it holds either way rather than pinning the residual.

## `fn real_docker_a_containerized_agent_reads_its_own_settings_and_cannot_write_them() {`

PR12-CONTAINER-CLAUDE-SETTINGS-HOST-PATH on a real runtime, as root's D-A
narrowed it. A containerized agent reads its own settings, which its command
records as `settings_file`, byte for byte, at the container path it is given.
It cannot write them, and the host file is unchanged. Another attempt's
settings file sits beside its own on the host: the container's
`/upstroke/settings` lists only its own, the other is absent, and the run's
private directory is not there. A filesystem witness, registered in
`DOCKER_GATED_TESTS`; it accepts no real CLI's policy.

## `#[cfg(target_os = "linux")]` › `fn real_docker_a_container_runs_as_the_coordinators_user_and_leaves_what_it_can_reclaim() {`

Root's D-B on a real runtime. A worker container reports the coordinator's
effective uid and gid from `id`, and makes a nested directory and a file in
its worktree. The host sees all three as the coordinator's, and the
coordinator removes them. A gate then runs Git in the same worktree,
through the role view, with no `safe.directory` at all: it succeeds, reads
the exact HEAD and finds the worktree clean. What the test establishes is
the observed ownership, the reclamation and the gate's success, for this
fixture's image; a rootless or remapped daemon is not covered.

## `fn real_docker_a_containerized_program_reads_its_exact_stdin_then_eof() {`

Root's D-C on a real runtime, three containers:
- a 300 KiB payload, past any pipe buffer, read by `cat` to its end, which
  `cat` reaches only at EOF: the byte count and the SHA-256 match what was
  sent;
- a program that reads 16 bytes of a mebibyte and exits 7: the run reports
  exit 7, promptly, and the undelivered rest holds nothing;
- a program that never reads a mebibyte of input, with a thirty-second
  timeout: the timeout path stops and removes it, with the write still
  pending, within a bound.

The timeout also covers Docker creating and starting the container, so it is
`ROOM_FOR_A_SLOW_CONTAINER_START`, thirty seconds: room for a slow daemon to
start it, and still far below the program's six-hundred-second `sleep`
(`CI-FLAKE-REAL-DOCKER-START-DEADLINE`).

## `struct MountedReal {`

The real-runtime fixture for the three mount witnesses: a canonical
scratch root, a real repository, the run identity of the witness's own
gated run id, and the image discovered without a pull. `resolve` binds the
production mount context (`MountContext::bind`, the coordinator's
credential locations included) and resolves the configured mounts against
the real runtime, so each witness exercises resolution, the record and the
runner end to end, not a hand-built policy.

## `fn real_docker_a_read_only_configured_mount_is_readable_and_not_writable() {`

A read-only operator mount, given to a gate (a role that is *not* forced
read-only, so the disposition is the record's): the container reads the
host marker through it, a write through it fails with the runtime's
read-only error, and the host directory holds no new file.

## `fn real_docker_a_read_write_configured_mount_is_writable_as_the_coordinators_user() {`

A read-write operator mount, given to an implementer: the container
creates a directory and a file through it, the host sees the content, and
both are owned by the coordinator's effective uid and gid (D-B's `--user`),
so the coordinator can reclaim them, which it does. The same recorded
mount given to a reviewer is forced read-only and its write fails. Linux
only, like the D-B witness it extends.

## `fn real_docker_a_resume_with_changed_mount_config_keeps_the_recorded_mounts() {`

Record-wins on a real runtime. A record with one read-only mount is
rebuilt by `rebuild_by_inspection`, the resume's own rebuild, under a
config that moves the mount to another source, makes it writable and adds
a second. The rebuild returns the record and warns that the configured
mount set is ignored. The resumed container sees the recorded source at
the recorded target, not today's source and not the extra target, and its
write is refused read-only. With the recorded source moved away, the
rebuild refuses as `MountSourceMoved` and no container of the run exists.
This is the runner-level resume path (the rebuild and the runner it
feeds), not a whole `upstroke resume` of a parked run.

## `fn real_docker_confines_a_gate_to_its_mount() {`

`expected_failures_refusals`: "**gate write outside mount fails**", and
DESIGN.md:400's whole sentence.

Repository-controlled gate code — "which no agent permission surface can
ever bound" (DESIGN.md:610) — is given every withheld path by absolute
name and asked to read it and to write it. The assertions are on the
**host**, because that is what the claim is about: a container is free
to create whatever it likes inside its own writable layer, and none of
it may reach the coordinator.

The control is in the same command: the gate reads its own workspace,
which it *can* see. A test in which the container could read nothing at
all would pass without the confinement doing anything.

## `fn real_docker_confines_a_gate_to_its_mount()` › `assert!(`

The control: the gate can read its own worktree.

## `fn real_docker_confines_a_gate_to_its_mount()` › `for marker in [`

And it saw none of the withheld content.

## `fn real_docker_confines_a_gate_to_its_mount()` › `for ((category, path), original) in withheld.iter().zip(&before) {`

The host is byte-identical: whatever the container wrote stayed in
the container.

## `fn real_docker_confines_a_gate_to_its_mount()` › `assert_eq!(repo::git_ok(&repo_dir, &["rev-parse", "HEAD"]), head);`

And the coordinator's Git is unmoved.

## `fn real_docker_a_gate_write_outside_every_declared_mount_fails() {`

`expected_failures_refusals[5]`: "**gate write outside mount fails**" —
the refusal itself, observed inside the container.

`PR6-CORRECTNESS-008` / `PR6-ENUM-005`, and the distinction that produced
them. `real_docker_confines_a_gate_to_its_mount` proves **"the host is
unharmed"**: it explicitly permits container-layer writes and asserts on
host bytes. That is true, and it is weaker than the contract's sentence —
with no read-only root filesystem the gate's
`printf owned >/outside-role-mount` exited **0**, and a test can prove a
true, weaker statement indefinitely while the stated guarantee does not
hold. So this test asserts the **write fails**, from the container's own
report, and never looks at the host at all.

The grid is {a path outside every declared mount} × {a declared writable
mount}, and the second column is not decoration: a container in which
*nothing* could be written would satisfy the first column while being
unusable, so the two controls — the role's own worktree and the declared
scratch surface — are what say the confinement is a boundary rather than
a brick.

The hostile paths are chosen to cover the three shapes a write can take:
the root of the container filesystem, a directory the image itself
populates, and — the interesting one — a **sibling of the role's own
mount**, `/upstroke/escape`, which a naive "only paths under the mount
targets are writable" implementation would let through.

## `fn real_docker_a_gate_write_outside_every_declared_mount_fa…` › `const OUTSIDE: &[&str] = &[`

Outside every declared mount, and inside the two that are writable.

## `fn real_docker_a_gate_write_outside_every_declared_mount_fa…` › `"/upstroke/escape",`

A sibling of the role's own mount target.

## `fn real_docker_a_gate_write_outside_every_declared_mount_fa…` › `let plan = runner.plan(&request).expect("plans");`

The mount set the claim is about, taken from the plan rather than
assumed, so "outside every declared mount" is checked against the
list the container is actually given.

## `fn real_docker_a_gate_write_outside_every_declared_mount_fa…` › `for path in &inside {`

The controls: the role's own worktree and the declared scratch
surface are writable, so the refusals above are a boundary and not a
container that can do nothing.

## `fn real_docker_the_daemon_holds_exactly_the_specs_mounts_and_a_read_only_root() {`

The daemon's own container carries **exactly** the spec's mounts and a
read-only root.

`PR6-ENUM-005`'s surviving mutation is not about the spec at all: it is
"append `--mount type=bind,source=/tmp,target=/outside` directly to
Docker's argv, **bypassing `CreateSpec.mounts`**". Every fake test sees
the unchanged spec, and a gated test that only writes to paths it knows
about never asks the daemon what the container really has. So this test
asks: it reads `.Mounts` and `.HostConfig.ReadonlyRootfs` back off the
created container and compares them to the plan, and an argv-appended
mount is a destination the plan does not name.

The container is created through the funnel and never started, which is
what lets it be inspected: `Runner::run` releases on both paths, so a
container that had run would be gone before anything could look at it.

## `fn real_docker_the_daemon_holds_exactly_the_specs_mounts_an…` › `crate::runner::container::fake::preclean_names(`

Pre-clean before the create, not after the teardown.

`reviews/FINDINGS.md` §16: `LeaveNoResidue` above is correct and
cannot help, because no in-process cleanup runs when the process is
SIGKILLed. Every component of this name is fixed — `repo_key()`, the
run id from `GATED_RUNS`, `INCARNATION_1`, and `gate_id(0)`'s
deterministic invocation hash — so the name a previous SIGKILLed run
left behind is exactly the name this `docker create` is about to ask
for. The recurrence is what makes the pre-clean meaningful.

## `fn real_docker_the_daemon_holds_exactly_the_specs_mounts_an…` › `let mut hooks = crate::runner::container::NoHooks;`

Write the intent, materialise the view, create — and stop there.
The view is the runner's own projection rather than a bare directory:
a linked worktree's `.git` pointer file is a bind **source** of the
create, so a directory-only view fails `docker create` outright.

## `fn real_docker_the_daemon_holds_exactly_the_specs_mounts_an…` › `assert!(planned.len() >= 4, "{planned:?}");`

The control: the comparison is not two empty sets.

## `fn real_docker_a_worktree_binary_cannot_shadow_the_certified_cli() {`

A binary planted in the worktree cannot become the CLI the attempt runs,
and a probe and an attempt resolve the same name to the same thing.

`PR6-CORRECTNESS-006`, end to end. DESIGN.md:612: "Probes run through
that same runner, or pre-flight could certify a host CLI/version
different from the one the attempt executes." A probe has no worktree
and an attempt has one, so their working directories differ **by
design**; with `PATH=.:/usr/bin` the attempt's `claude` is whatever the
repository put in the worktree and pre-flight certified something else.

Two cells, and they are the two halves of the repair:

1. an image environment whose `PATH` resolves against the working
   directory is **refused before any effect** — no container is created
   at all, checked against the daemon by label;
2. under the absolute-only `PATH` the runner supplies, the planted
   binary is not resolvable by name in either the probe or the attempt,
   and a name that *is* on the path resolves to the same absolute file
   in both.

The control is inside the same command: the gate proves the shim is
really there and really executable in its own worktree, so "not found"
is a statement about resolution and not about the fixture.

## `fn real_docker_a_worktree_binary_cannot_shadow_the_certifie…` › `let shim = mine.join("claude");`

The shim the repository controls, named as a CLI this engine drives.

## `fn real_docker_a_worktree_binary_cannot_shadow_the_certifie…` › `let hostile = ContainerRunner::new(`

(1) The refusal, before any effect.

## `fn real_docker_a_worktree_binary_cannot_shadow_the_certifie…` › `let runner = ContainerRunner::new(`

(2) The guarantee, under the PATH the runner supplies.

## `fn real_docker_a_worktree_binary_cannot_shadow_the_certifie…` › `assert_eq!(answers[0].1["PWD"], BoundaryLayout::DEFAULT_WORKSPACE);`

The two really do run in different working directories — the premise
of the finding, asserted rather than assumed.

## `fn real_docker_a_worktree_binary_cannot_shadow_the_certifie…` › `assert_eq!(`

The control: the shim is there, and it is executable.

## `fn real_docker_a_worktree_binary_cannot_shadow_the_certifie…` › `assert_eq!(`

Neither resolves it, and both resolve a name that is on the path to
the same absolute file.

## `fn real_docker_a_git_dependent_gate_sees_only_the_role_view() {`

`proof_tests[1]`: "**Git-dependent gate sees only the role view**",
against a real container.

The four properties of DESIGN.md:612, each read out of a real `git`
running inside the boundary: the exact detached HEAD, the exact index
(`status --porcelain` is empty on a clean worktree), no engine refs, and
objects that resolve. The coordinator's refs are re-read afterwards, so
"without exposing **or mutating**" is both halves.

## `fn real_docker_a_git_dependent_gate_sees_only_the_role_view…` › `let git = "git -C /upstroke/workspace";`

No `safe.directory`. This line carried `-c safe.directory='*'` until PR12's
D-B, because the host paths are owned by the coordinator's user and the
container's process was the image's user: an ownership check, not a
confinement one. Every container now runs as the coordinator's numeric
uid and gid, so Git inside it reads a worktree and a view its own user owns,
and this test drops the wildcard. It is the second witness of that, beside
`real_docker_a_container_runs_as_the_coordinators_user_and_leaves_what_it_can_reclaim`.

## `fn real_docker_a_git_dependent_gate_sees_only_the_role_view…` › `for name in &planted {`

Nothing was exposed and nothing was mutated: the coordinator's refs
are all still there and still where they were.

## `fn real_docker_adapter_parsing_matches_the_host_table() {`

`decisions.tests_acceptance.parity`: "host and container runners produce
identical **adapter parsing**".

The table, the fixtures and the expectations are PR4's — `runner::tests::
adapter_parse_parity` was written for exactly this and its doc comment
says so — and the **only** thing this test varies is the `&dyn Runner`.
It is a real chain, spec -> runner -> `ProcessOutput` -> `AgentAdapter::
parse`, because the claim is about the seam: an adapter never learns
which runner produced the output it reads, and nothing but a runner
actually producing it proves that.

## `fn real_docker_adapter_parsing_matches_the_host_table()` › `assert_eq!(container_rows.len(), 3);`

The table is not empty and really did vary, so equality is a claim.

## `fn real_docker_withholds_an_image_credential_variable_from_a_role_that_takes_none() {`

The recorded image sets a credential location, and a role that takes
none does not receive it **inside the container**.

`PR6-CORRECTNESS-007`, against the daemon. The unit-level assertion is
about the composed vector, and the composed vector is not the
container's environment: `docker create --env` **overlays** the image's
own, so a key the runner omits is a key the image decides. This runs a
gate — repository-controlled code, the one thing no agent permission
surface bounds — inside an image whose `ENV` sets `CODEX_HOME` and
`CLAUDE_CONFIG_DIR`, and reads the variables back from inside.

Second field held constant: the same image and the same command in both
halves; only the role moves. `GH_CONFIG_DIR` is the control — an image
variable that is not a credential location — so a runner that wiped the
image environment rather than overriding two keys of it fails here.

## `fn real_docker_withholds_an_image_credential_variable_from_…` › `let declared = docker`

The premise, read from the daemon rather than assumed: this image
really does set a credential location.

## `fn real_docker_withholds_an_image_credential_variable_from_…` › `let runner = ContainerRunner::new(`

A policy that DOES record a credential volume for codex would be
creating operator state; the withholding is about the *variable*, and
the mount is separately asserted by
`the_credential_volume_is_mounted_exactly_when_its_location_is_supplied`.

## `fn real_docker_a_container_contains_a_daemonised_descendant() {`

A container contains its **descendants**, including one that has left
the leader's session.

`PR6-ENUM-006`. `invariants_introduced[0]` is "container contains
descendants", and nothing in this suite observed one: the timeout
fixture runs a single `sleep` and checks one container liveness bit, so
a cancellation that terminated or forgot only the leader would pass it.

The descendant is `setsid`-detached, which is exactly what escapes a
host process-group kill (`agent::proc`'s whole subject), and it is
observed **in the host's own process table** rather than by timing: a
container's processes are ordinary host processes in another namespace,
so `/proc/<pid>/cmdline` can name them. Each `sleep` carries a marker
argument this run computes from its own pid, pid namespace and clock,
so another run sharing it is ruled out by construction rather than left
to chance, short of a wall clock set back to the nanosecond (see the
markers below; `CONTAINER-DESCENDANT-TEST-SCANS-THE-WHOLE-HOST`).

The intersection: {leader, detached descendant} × {container running,
container reclaimed}. The two "running" cells are the control — without
them a test in which nothing ever started would pass.

## `fn real_docker_a_container_contains_a_daemonised_descendant…` › `let clock = std::time::SystemTime::now()`

The two markers, the run's own. `pids_with` reads every process on the
host, so a fixed pair (it was `903222` and `903111`) was shared by every
run of the suite on one machine: a run that had removed its container
found another run's live `sleep` and reported it as its own leaked
descendant (`CONTAINER-DESCENDANT-TEST-SCANS-THE-WHOLE-HOST`).

Each marker is a decimal number. Its integer part is one role digit
(`1` the leader, `2` the descendant) and this process's id in seven
digits (the kernel's ceiling, 4 194 304, fits). Its fraction is the
inode of this process's pid namespace (`/proc/self/ns/pid`) in twenty
digits and the wall clock's whole reading, nanoseconds since the epoch,
untruncated, in thirty-nine. Every field has a fixed width, so no two
(role, pid, namespace, clock) tuples spell the same string.

- Two runs alive at once differ by construction: in one pid namespace
  their pids differ, and in two live namespaces the namespace inodes
  differ, so the local pid each sees may be the same without the
  markers being so.
- A run and a `sleep` left by an earlier run whose pid and namespace it
  reuses differ by the clock, which is kept whole rather than wrapped,
  so they meet only if the wall clock has been set back to the very
  nanosecond the earlier run read.

That last case is the one the construction does not rule out; nothing
else shares the markers. Distinct roles, so the leader and the
descendant are told apart by value and not by order.

They are the **durations** the two `sleep` calls are given, so they
have to be numbers — `sleep 999111r3b` answers `invalid number` and
exits 1, which is a container that ends before its timeout and a
fixture that measures nothing. The fraction is where the identity
goes so that the duration itself stays an ordinary one: the integer
part is at most about 3·10⁷ seconds, far beyond the thirty-second
timeout, and a `sleep` that parses fractions reads the long tail as a
fraction of a second. A `sleep` that refused it would exit at once, and
the controls in `let seen` would fail rather than pass vacuously. Where
`/proc` is missing the namespace reads as zero, but there `pids_with`
finds nothing either and those controls fail the same way.

## `fn real_docker_a_container_contains_a_daemonised_descendant…` › `fn pids_with(marker: &str) -> Vec<String> {`

Every pid whose argv contains `marker`, read from `/proc`.

Equality with whole argv entries rather than a substring of the whole
buffer, so the `sh -c` that runs the fixture, whose single script
argument mentions both markers, cannot match; only the `sleep`s, which
take a marker as an argument of its own, do. It reads the whole host on purpose: the
container's processes are host processes in another namespace, and the
markers being the run's own is what makes the whole host safe to read.

## `fn real_docker_a_container_contains_a_daemonised_descendant…` › `let mut request = gate_request(`

The leader spawns a detached descendant and then blocks. The runner's
own timeout is what stops the container, which is
`slice_contract.cancellation`: "timeout or shutdown stops and removes
the container".

The timeout also covers Docker creating and starting the container, so it
is `ROOM_FOR_A_SLOW_CONTAINER_START`, thirty seconds: room for a slow
daemon to start it, and still far below the two `sleep`s, each more than
ten days long (`CI-FLAKE-REAL-DOCKER-START-DEADLINE`).

## `fn real_docker_a_container_contains_a_daemonised_descendant…` › `let seen = std::thread::scope(|scope| {`

The controls, sampled from another thread while the invocation is in
flight: both processes really are on this machine.

The sampler polls every 50 ms until it has seen both or `returned` is
set. A `SetOnDrop` guard held across `run_blocking` sets it when the
call ends either way: on return, before unwrapping or asserting on the
result and before the join, and on unwind, when the guard is dropped as
the panic leaves its block. So neither an invocation that fails nor one
that panics can leave the scope waiting on a sampler with nothing to
see; the scope joins the sampler and the panic carries on. It has no
deadline of its own because the invocation's end already bounds it,
and one of its own could expire during a slow Docker start the
thirty-second timeout still allows, failing the controls although both
processes ran (`CI-FLAKE-REAL-DOCKER-START-DEADLINE`).

## `fn real_docker_a_container_contains_a_daemonised_descendant…` › `let leader = pids_with(&leader_marker);`

The container is gone, and so is everything it contained. The
descendant left the leader's session, so a cancellation that killed
only the leader's process group would leave it here.

## `fn real_docker_a_container_contains_a_daemonised_descendant…` › `assert!(`

The descendant first: it is the invariant's subject, and a leader
assertion that fires before it would report the wrong thing about a
cancellation that left both alive.

## `fn real_docker_a_container_slow_to_start_still_reaches_its_timeout() {`

The regression for `CI-FLAKE-REAL-DOCKER-START-DEADLINE`: a container whose `docker start` is
held five seconds still reaches the timeout `ROOM_FOR_A_SLOW_CONTAINER_START` gives its
invocation, which ends in a timed-out output and not in the runner's refusal of `start`. Put
three seconds back in the constant and this test fails, its helper reporting the refusal where it
expects the output; at thirty it passes. Measured before the commit: at three seconds the
helper's `expect` met the runner's refusal of `start`, its container "still created when the
invocation's deadline passed, so it never started", while the other four
`real_docker_a_contain` tests passed; restored to thirty, all five passed together in 43 seconds.

It skips exactly as every other real-Docker test does: through `docker_gate`, which is why it is
registered in `DOCKER_GATED_TESTS`, and through `discover` when the runtime holds none of
`PREFERRED_IMAGES`. It does not look for the real `docker` itself: it takes its own inherited
`PATH`, an `OsString` from `std::env::var_os` and never a string, so a `PATH` that is not UTF-8
is carried unchanged, and the stand-in defers to the ordinary command search under that `PATH`
(below). No path to the program is computed or written anywhere. It makes a scratch tree with an
empty `bin` in it, and runs `slow_docker_start_child` under `run_test_isolated`, bounded at two
minutes: this test binary again, selecting that one ignored test with `--exact`, `--ignored` and
`--nocapture`, with a `PATH` that is the scratch `bin` and then this process's own, and told the
scratch tree (`UPSTROKE_TEST_SLOW_START_SCRATCH`), this process's original `PATH`
(`UPSTROKE_TEST_SLOW_START_ORIGINAL_PATH`) and the stand-in's mark, `delayed-starts` in the
scratch tree (`UPSTROKE_TEST_SLOW_START_MARK`, an `OsStr` of a `PathBuf`). `DockerCli` runs `docker` by name, so a stand-in
first on the child's `PATH` is the program every runtime question in it reaches, and only a
child can be given a `PATH` of its own. The parent requires the child's success and libtest's
`1 passed`, a misnamed helper running no test at all, and prints the child's output when either
is missing.

It costs about thirty-five seconds, most of it the thirty-second timeout itself, inside which the
five-second hold falls; the rest is the helper's repository and the container's removal.

What it shows holds while the runner starts a container with `start` as `docker`'s first
argument, as it does today (`docker start --attach --interactive` for an invocation with input):
a start spelled any other way would pass through the stand-in unheld, and this test would no
longer fail at three seconds. That case, and a hold that never happened, no longer pass
silently: the helper fails when the stand-in left no mark of a delayed `start`.

Two mutations were applied, observed and restored before the commit. With
`ROOM_FOR_A_SLOW_CONTAINER_START` at three seconds this test failed, the helper's `expect`
meeting the runner's refusal of `start` ("still created when the invocation's deadline passed").
With the stand-in's `sleep` replaced by `false` it failed too, the helper's `expect` meeting the
refusal of a `start` whose client "ended (exit code 1)": the stand-in's own failure, with
`docker` never run. Both were applied again once the hold had stopped reading the clock and run
`sleep` under the parent's `PATH`: at three seconds the same refusal ("still created when the
invocation's deadline passed"); with `sleep` replaced by a program name that does not exist, the
refusal of a `start` whose attached client "ended (exit code 1)", the stand-in failing when the
command search found no such program, before it wrote the mark and without running `docker`.

## `fn slow_docker_start_child() {`

The isolated helper `real_docker_a_container_slow_to_start_still_reaches_its_timeout` runs.
Ignored, so libtest never runs it in its own process.

It asserts, with `std::env::var_os`, that its parent set `UPSTROKE_TEST_SLOW_START_ORIGINAL_PATH`
on it, and takes the mark's path from `UPSTROKE_TEST_SLOW_START_MARK` the same way, into a
`PathBuf`. Its first act, before it starts any process, is to write the
stand-in `docker` into the `bin` its parent put first on its `PATH`, and make it executable: a
`/bin/sh` script that, when its first argument is `start`, holds it `SLOW_START_HOLD_SECONDS`
(five) seconds and then, whatever the arguments, replaces itself with
`PATH="$UPSTROKE_TEST_SLOW_START_ORIGINAL_PATH" exec docker "$@"`.

The hold runs `sleep 5` and then appends one fixed line, `delayed 5`, one `start` to a line, to
the file `$UPSTROKE_TEST_SLOW_START_MARK` names. If either does not succeed, the script prints why
on its standard error and exits 1 without running `docker`, so a hold that fails is a refused
`start`, never a start that ran at once. The evidence that the start was held is `sleep`'s own exit
status and the mark written after it: a `sleep` that is missing or fails ends the script before the
mark is written and before `docker` runs, so a mark line exists only for a `sleep 5` that
succeeded. `sleep` runs with `PATH="$UPSTROKE_TEST_SLOW_START_ORIGINAL_PATH"` for that one command,
the parent's `PATH`, so it is found where the parent's `docker` is found, by the same command
search. No fixed path is named: a fixed `PATH=/bin:/usr/bin` is not portable, and on NixOS, a
supported Linux, those directories hold only `sh` and `env`, so the stand-in failed there. A
`PATH` that finds `docker` and no `sleep` fails this test loudly, never holding nothing unnoticed.
`/bin/sh` has no waiting of its own (`read -t` is not POSIX and `dash` lacks it), so the external
`sleep` is the hold. The script reads no clock: two `date +%s` readings are wall-clock times, and a
clock that steps back or slews during the sleep made a correct five-second hold, five seconds by a
monotonic clock, read as three and fail the test; the mark therefore carries no time at all. It
runs `docker` by name under the parent's original `PATH`, so the system's own command
search picks the program exactly as it would with no stand-in, skipping a `docker` that is not
executable as that search does; the test never decides which program that is. The script's
text holds the variable's name and no path: the parent set the variable, as an `OsStr`, on this
process, which is the process whose runner runs the stand-in, and the runner starts `docker`
(`docker_command`) with the environment it inherits, so the stand-in reads the `PATH`'s bytes as
they are, with nothing to quote. Every other
question reaches the daemon unchanged, the held start is the real one once the hold ends, and no
shell is left between the runner and the attached client it supervises. The helper then asserts that the stand-in is the
`docker` this process resolves, so a `PATH` that put it anywhere but first fails here rather than
holding nothing.

The helper writes the stand-in, and not the parent, because the parent shares libtest's process
with tests that start children: a child forked while the new file is open for writing inherits
that descriptor until it execs, and a file any process holds open for writing cannot be executed
(`ETXTBSY`). This process runs the one test, and nothing in it has started a child yet.

Then one invocation shaped like the never-read one of
`real_docker_a_containerized_program_reads_its_exact_stdin_then_eof`: a worker running
`sleep 600` with a mebibyte of input it never reads, under `ROOM_FOR_A_SLOW_CONTAINER_START`,
through a `ContainerRunner` built as that test builds its own (the image `discover` finds,
`real_policy`, `real_identity` over the scratch tree with the `slowstart` run id,
`image_environment_of`, a `LeaveNoResidue` guard). The result must be an output, and one that
timed out: a start the deadline overtakes is an error, which fails the `expect`.

After the invocation, the helper reads the mark. A mark that is missing or empty fails with "no
`docker start` was delayed": the timeout was reached without the slow start this test exists to
hold. Every line must be exactly `delayed 5`, formatted from `SLOW_START_HOLD_SECONDS`, the line
the stand-in writes only after its `sleep` succeeded; a line that is anything else fails, naming
the line. The helper does no arithmetic on times: `sleep`'s success is the proof of the hold.

## `fn every_gated_test_of_this_lane_is_counted() {`

Every Docker-gated test this lane adds is on the list that counts them.

`every_docker_gated_test_is_named_and_present` in the substrate's own
suite closes both directions across `src/runner/**`; this is the lane's
half, so a name added here without being listed fails in this file
rather than in another lane's.

## `fn every_gated_test_of_this_lane_is_counted()` › `"real_docker_a_gate_write_outside_every_declared_mount_fails",`

Repair round R1.

## `fn every_gated_test_of_this_lane_is_counted()` › `"real_docker_withholds_an_image_credential_variable_from_a_role_that_takes_none",`

Repair round R3b.

## `fn every_gated_test_of_this_lane_is_counted()` › `"real_docker_a_containerized_agent_reads_its_own_settings_and_cannot_write_them",`

PR12-CONTAINER-CLAUDE-SETTINGS-HOST-PATH.

## `fn every_gated_test_of_this_lane_is_counted()` › `"real_docker_a_container_runs_as_the_coordinators_user_and_leaves_what_it_can_reclaim",`

PR12's D-B.

## `fn every_gated_test_of_this_lane_is_counted()` › `"real_docker_a_containerized_program_reads_its_exact_stdin_then_eof",`

PR12's D-C.

## `fn every_gated_test_of_this_lane_is_counted()` › `"real_docker_a_read_only_configured_mount_is_readable_and_not_writable",`

Root's recorded-mounts disposition (2026-11-02): the three mount witnesses
it requires, registered here, in `GATED_RUNS` (`mountro`, `mountrw`,
`mountresume`) and in `DOCKER_GATED_TESTS`. Sixteen in all.

## `fn every_gated_test_of_this_lane_is_counted()` › `"real_docker_a_container_slow_to_start_still_reaches_its_timeout",`

The regression for `CI-FLAKE-REAL-DOCKER-START-DEADLINE` (2026-11-11), registered here, in
`GATED_RUNS` (`slowstart`, for its helper's container) and in `DOCKER_GATED_TESTS`. Seventeen in
all.

## `fn removal_in_progress_diagnostic(name: &str) -> String {`

The daemon's answer names the container it is about, and since the cover review
of `9ee9784e` nothing reads an answer about another container as this one's
settlement, so the fixture is armed after the container's name is known rather
than from a literal. A constant naming `upstroke-c` beside a generated container
name is a diagnostic the repaired normalizer correctly refuses.

## `struct CancelAfter {`

A per-invocation `ContainerHooks` observer that cancels its own call right after one site's after
phase and otherwise records like `RecordingHooks` (its trace is the fixture's). Cancelling from the
hook makes the moment deterministic without a second thread: the container has just been started,
and the supervisor's first tick sees the cancellation.

## `fn a_cancelled_container_invocation_is_released_through_the_reclaim_steps_before_it_reports() {`

A container cancelled while it runs is released through the reclaim steps a timed-out one goes
through — `Start` before `Stop` before `Remove` before `UnmountGitView` before `RemoveIntent` — and
leaves no container, intent or view; it is not collected, and only then does the invocation report
`RunnerError::cancelled` with fate `Gone`. `permits.protocol`: "cancelled after the Runner terminated
its process or container". The same assertions as
`a_container_that_outlives_its_timeout_is_stopped_and_removed`, which is the disposition's other
cause.

Since PR12's D-C the cancelled command carries input, and the cancellation path is the same: the
payload went to the container at its start, and the release follows in its order.

## `fn a_launch_that_fails_after_a_cancellation_reports_the_cancellation() {`

Root's START-DEADLINE finding, at the runner. A cancellation requested after
the create, and a start that then fails, as an attached start fails when it
is cancelled while its container is still `created`: the invocation reports
`RunnerError::cancelled`, with the fate the reclaim established, and leaves
no container and no intent.

## `fn an_invocation_whose_removal_leaves_a_process_unresolved_is_never_reported_gone() {`

Root's DELIVERY-OWNERSHIP follow-through, at the runner. A removal that
leaves a process unresolved, as an attached client whose supervision ended
`Unresolved` does, yields an invocation whose fate is `Unresolved` with that
fact as its error: on the cancellation path, not a plain cancellation, and on
the completed path, not a success, even though the container was observed
exited. The assertions are `assert_unresolved_removal_fate`'s. Unix runs each
path through `unresolved_removal_in_an_owned_helper`; Windows, which has no
Unix container reaper, runs the same assertions directly.

PR12-R9-REMOVAL-GUARD-RETAINS-UNOWNED-REAPER and PR12-REG7-REMOVAL-GUARD-REAPER,
two records of one defect: from `c08d9935`, which introduced this guard, until
repair10 both paths ran in libtest's own process. Each built a real
`ContainerRunner`, whose `Reaping::cover` forks a real Unix reaper, and its
deliberately unresolved removal left that reaper to the production
retained-reaper backstop (`Armed::kept_until_the_process_exits`) when the
runner dropped, with no owner in the test's scope, on ordinary return and on
unwinding alike. The backstop is production's and intentional; what was
missing was the test's ownership, the lifecycle L5 gave J6's and K2's guards
and R6-N1 the stalled observation's. No historical failure, hang or residue is
attributed to it.

## `fn assert_unresolved_removal_fate(fixture: &Fixture, cancelled: bool) {`

The guard's assertions for one path, unchanged: a worker request whose
container the fake leaves a process unresolved at removal
(`leave_a_process_unresolved_at_removal`), cancelled at the container's start
or once the container exited and its intent is being removed. The invocation
is not a plain cancellation, its fate is `Unresolved`, and its error says it
could not establish every process. Its runner drops when it returns, which is
what hands the reaper to the backstop.

## `fn unresolved_removal_in_an_owned_helper(cancelled: bool) {`

Runs `unresolved_removal_child` for one path (`cancelled` or `completed`,
through `UNRESOLVED_REMOVAL_PATH`) under `run_test_isolated` and
`FIXTURE_CHILD_BOUND`, twice: ordinary return and deliberate unwinding after
the runner dropped (`UNRESOLVED_REMOVAL_ENDING`). As
`unresolved_process_in_an_owned_helper` does for the start, observation,
pre-start-cleanup and volume-inspection cases, which stay as they are, the
outer assertion requires the helper's successful exit, libtest's `1 passed`,
exactly one retained-mock-reaper collection marker and one job-control-guard
collection marker, and the final ECHILD marker, all arriving on captured
stderr through the fixture's `say` funnel, which also reports the successful
output. A failed fate assertion, an uncollected reaper or probe, or failed
unwind cleanup cannot pass.

## `fn unresolved_removal_child() {`

Ignored because only the owner above supplies its path and ending. The shape
is `unresolved_process_child`'s: `IsolatedJobControl` before the fixture,
whose setup starts the process-global job-control guard and its probe;
`RetainedMockReaper` for the runner's exact scope (`private_root`,
`INCARNATION_1`) before the invocation; the completed path's fixture exits its
container on start, as the in-process guard's did. After
`assert_unresolved_removal_fate` returns, the runner has dropped through the
unchanged production backstop, and the helper requires the scoped reaper
still armed. On ordinary return it collects the reaper and the guard
explicitly; on the deliberate panic their Drops collect them. The catch accepts
only that exact panic. Finally `assert_no_children_in_isolated_test` requires
ECHILD before the owner collects the helper.

ECHILD is a direct-child claim, as for `unresolved_process_child`: this
retained fake-runtime reaper has no supervised command group or anchor,
receives no cleanup command while the helper lives, and is killed and reaped
before the helper's exit could start its runtime cleanup, so the
no-grandchildren argument in `agent/proc.md` is the same one. It says nothing
about arbitrary descendants.

Measured before commit as negative control B1 (the scoped `RetainedMockReaper`
and its collection removed from this helper, which is the guard's state before
repair10): the first path, `cancelled/return`, fails at the helper's
direct-child census (`waitpid=0`) with no collection marker, and the owner's
assertion fails on it.

## `fn a_container_call_cancelled_before_it_starts_writes_no_intent() {`

A call cancelled before it starts reaches nothing: no intent, no view, no container, an empty trace,
and the report is `NeverStarted`.

## `impl ContainerRuntime for Runtime` › `fn reaper_program(&self) -> PathBuf {`

The scripted double delegates to its fake, so its runners arm their reapers over the fake's
no-op program (or its relay) and never over a real `docker` (PR11 follow-up A).

## `const UNARMED_LAUNCHERS: &[&str] = &["launch", "create_container", "start_container"];`

The funnel entries that start a container without a cover: the free `launch` and the two
funnels it and `ContainerRunner::launch` call. The runtime's own `create` and `start` are the
primitives below them, read separately.

## `fn macro_arguments(code: &str) -> Vec<(usize, usize)> {`

The span of every macro invocation's argument in blanked code: an identifier immediately
followed by `!`, then, after any whitespace, `(`, `[` or `{`, to its matching delimiter, every
kind of bracket counted. `x != (y)` opens nothing — the `!` follows a space — and neither does
`macro_rules! name {`, whose body is read as ordinary code: a launcher it names there is named
by the ordinary rules. A macro invocation's argument is a token tree the macro can splice
anywhere, which is why `namings` treats it apart (`FUA-I1-MACRO`).

## `fn inside_a_macro_argument(spans: &[(usize, usize)], at: usize) -> bool {`

Whether `at` falls strictly inside one of those spans — nested invocations included.

## `fn namings(code: &str, name: &str) -> Vec<(Naming, usize)> {`

Every naming of `name` in blanked code (`FUA-D1-DES-1`, made conservative by `FUA-I1-MACRO`).
Inside a macro's argument every occurrence is a naming, `MacroArgument`, whatever precedes or
follows it: the macro decides what the tokens become, so `uncovered_delegate!(launch: hooks, …)`
expanding to `super::launch(hooks, …)` — the implementation review's mutation, which every check
passed while the census skipped `launch:` as a field initializer — is named. Outside one, an
occurrence that is its definition (`fn launch(`) or a method call or field access (`.launch`) is
not a naming; every other is, classified as a call, a function value, an import, an alias, a
re-export or a field. Inside a `use` declaration it is an import, an alias when `as` follows it, a
re-export when the `use` is `pub` or `pub(…)`; elsewhere a call when `(` follows, a field when a
single `:` does — a field's declaration or initializer, a struct pattern's field or a typed
binding, none of which names the function outside a macro, but each is pinned rather than
skipped, so no colon is passed over unread — and a value otherwise. Every alias, function value
and re-export names the original identifier somewhere, so counting namings rather than calls
closes the gap the design review found: `use super::launch as start_uncovered;` and a call of
`start_uncovered(` passed the census as designed.

What no reading of source text can see: an identifier a procedural macro builds from pieces, a
path inside a string an attribute hands a derive (`#[serde(with = "…")]`; strings are blanked),
and a file `include!`d from outside `src/**/*.rs`. That is why the compiler's refusal is the
stronger guard where it can be had: the free `launch` is test-only (`mod uncovered` in
`container.rs`), so no production build can name it at all, and `create_container` and
`start_container` are `pub(in crate::runner::container)`, so outside the container module tree
no spelling of either compiles. The census is the guard where the compiler cannot be: inside the
module tree, and in test builds, where the free `launch` exists.

**It is not the guard any more, and it never was a proof** (`FUA-I2-MACRO-WS`, implementation
review round 2). This reader requires an identifier byte immediately before `!`; rustfmt keeps
`delegate ! (plan.start_container, …)` as written inside a nested macro definition, the reader
missed the invocation, passed `.start_container` over as a field access, and the expansion
called `super::start_container` with no cover — every test green, the container running with no
reaper. Round 1 had repaired the reader for one spelling and round 2 found the next; a reading of
text has spellings it cannot see. Since round 2 every start primitive takes a proof only
`Reaping::cover` mints (`exec.md`, `mod cover`), so a start with no cover is a compile error
whatever spells it — that reviewer's mutation fails with `error[E0308]` and `error[E0061]` in
production and test builds alike — and this census is a lexical backstop: it is not made to see
`name ! (…)`, and nothing depends on it doing so.

## `fn enclosing_fn(code: &str, at: usize) -> Option<String> {`

The innermost `fn` whose body's braces contain `at`, read in blanked code, where every brace is
code. A `fn(` pointer type has no name and is skipped.

## `fn the_naming_reader_names_an_alias_a_function_value_and_a_re_export_of_an_unarmed_launcher() {`

The reader over written snippets: an alias, two re-exports (`pub use … as`, `pub(crate) use`),
an import, a function value, a call by bare name after a glob import, a call by path and a field
initializer are named; a definition, a method call and a field access are not.

## `fn the_naming_reader_names_a_launcher_inside_a_macro_argument_whatever_follows_it() {`

`FUA-I1-MACRO`'s census witness, red on the census as reviewed at `17d7c605`, which read none of
these occurrences: the implementation review's macro verbatim and its invocation
`uncovered_delegate!(launch: hooks, runtime, view, plan)`, the same with braces and
`start_container:`, a field access `plan.launch` inside `spliced![…]`, all named as macro
arguments, and a struct field `launch: LaunchPlan` named as a field; `a != (b)` opens no macro.
The primitives' reader over a second snippet names `start:` inside a macro argument and a field
`start: usize`, and still passes over `OpenOptions::new().create(true)`.

## `fn every_container_start_in_production_is_reached_only_through_a_covered_launch() {`

The domination census (§1.4 of the follow-up's record), over every production region of
`src/` (`effects::production_code`, skipping the whole-file test modules, as the fold census
does), five checks:

1. the namings of `create_container` and `start_container` are exactly `exec.rs`'s import and
   its two calls inside `ContainerRunner::launch` — `container.rs`'s two calls inside the free
   `launch` left the production region when it became test-only (`FUA-I1-MACRO`);
2. the free `launch` has no production naming at all but the two `launch:` fields of `exec.rs`'s
   `InvocationPlan`, its declaration and its initializer in `plan`, pinned as fields; any
   occurrence inside a macro's argument is a naming, whatever follows it. The free `launch`
   itself is test-only now, so in a production build a naming does not compile; this check is
   the guard in test builds, where it exists;
3. in every production module whose effective `clippy::disallowed_methods` level is `allow` —
   stated in its header or inherited from a parent that states it, twenty-seven modules today —
   where clippy therefore cannot refuse `ContainerRuntime::create` or `::start`, each call, path,
   field or macro-argument occurrence of `create` or `start` is one of twenty-one pinned namings:
   `runtime.create(` and `runtime.start(` inside the two funnels, and nineteen others none of
   which is a container runtime's (`File::create(`, `Drain::start(`, `PrivateHooksDir::create(`,
   the parameters `create:` and `start:`, `util::tail`'s local `start` inside `format!` and the
   like); and `RuntimeOp::Create`/`RuntimeOp::Start` occur only inside the real runtime's own
   `create` and `start`;
4. in `exec.rs`, `fn launch(`'s signature names both proofs, `CoveredCreate<` and
   `CoveredStart<`; `self.launch(` occurs once, in `contain`, after `contain`'s one `.cover(`;
   `Covered {`, `CoveredCreate {` and `CoveredStart {` are each constructed once, in `cover`;
   and the module `cover` declares no module of its own in production, since a child module
   sees its private fields (`FUA-I2-MACRO-WS`);
5. the control: the only region naming `start_container` is `exec.rs` (`container.rs` named it
   only inside the free `launch`), and the walk read more than 40 files and 750,000
   non-whitespace bytes.

Since review round 2 it also holds the type-level guarantee's shape, so the guarantee cannot be
taken away in silence: `create_container`, `start_container` and `ContainerRuntime`'s `create`
and `start` each name their proof in their signature, and no production region names
`without_a_reaper`, the test-only mint — in a test build it exists, and the reviewer's macro
handed that mint compiles there (`r2-c4`; a production build refuses it, `r2-p3`). These, too,
read text; what they guard is that the types keep doing the work.

In every other production module the lint is denied or forbidden outside tests, so a path to the
primitives there — a call or a function value — is a clippy error, measured in a forbidding module
(`fua-p1-clippy-refuses-a-function-value-of-the-start-primitive`); and a call planted in a module
that allows the lint, where clippy says nothing, fails check 3 (`fua-c8`).

## `fn reaper_spec(private_root: &Path, incarnation: &str) -> CreateSpec {`

A spec carrying the two labels a reaper's scope lists by, as `ContainerIntent::labels` writes
them: `Reaping::cover` takes the spec it validates, since the create proof it mints carries it.

## `fn armed_for(private_root: &Path, incarnation: &str) -> bool {`

Whether a container reaper this process armed, and has not cancelled, lists that scope.

## `fn a_scope_that_does_not_select_its_containers_labels_is_refused_with_no_reaper_armed() {`

`FUA-D1-DES-2`: an unarmed runner's cover is handed another incarnation's labels and refuses —
and no reaper exists for either scope, in the runner's own state or the process's observation.
The control arms on its own labels, and its established end disarms at the drop. With the
comparison moved after the arming (`fua-m14`) a reaper is armed before the refusal.

## `fn a_container_whose_labels_differ_from_the_armed_scope_is_refused_before_its_intent() {`

`R7-D2`'s per-launch half: a runner whose reaper was armed for another incarnation — as a
caller-built reaper was in round R6 — refuses its own container before its intent is written:
`NeverStarted`, no intent, nothing created, and no second reaper.

## `fn a_runner_whose_container_is_unresolved_keeps_its_reaper_armed_past_its_drop() {`

`R6-C1` at the runner: the runtime cannot observe, stop or remove the container, so the
invocation ends `Unresolved`, and the reaper is still armed after the runner is dropped.

## `fn a_runner_whose_containers_all_ended_disarms_its_reaper_at_its_drop() {`

The control: one invocation completes, one ends `Gone` with its observation lost; the drop
disarms.

## `fn a_panic_inside_contain_leaves_the_reaper_armed() {`

An observer panics after the start; the cover is dropped by the unwinding without a settle, and
the reaper is still armed after the runner's drop (`fua-m7`).

## `const REAPER_TESTS: &[(&str, &[&str])] = &[`

The functions of this follow-up's reaper tests, by file. `no_reaper_test_reads_a_hold_as_released_once`
reads each, so a renamed one fails the census rather than leaving it.

## `fn negated_hold_reads_outside_a_bounded_wait(body: &str) -> Vec<String> {`

Every `!…observe_cleanup_hold(` or `!…is_running(` not inside a `loop` or `while` whose block
mentions a `deadline` or an `elapsed()`.

## `fn no_reaper_test_reads_a_hold_as_released_once() {`

Requirement 8 of `PR11-REAPER-CONTAINER-SCOPE-UNREGISTERED` (round R6's CI flake): a sibling
test thread's fork holds an inherited cleanup-lease descriptor for a moment
(`PR281-CLEANUP-LEASE-HOLD-OUTLIVED-AND-ITS-UNREADABLE-TWIN`), so a "not held" read must poll
within a bound. A positive read is safe and is not refused: a fork can make a released lease
look held, never a held one look released. `fua-m12` adds one one-shot read to the lease test's
child and turns this census red.

## `fn primitive_namings(code: &str, name: &str) -> Vec<(String, usize)> {`

The namings of a primitive's name that could reach it: a call by any receiver, a path, a naming
inside a `use`, a field or typed binding (`start:`), and every occurrence inside a macro's
argument, whatever surrounds it (`FUA-I1-MACRO`: a macro can splice `start` after any receiver);
outside a macro, not a definition, a module path segment (`create::`), or a local variable used
as a value — a trait method is reachable there only by a call or a path. A call whose only
argument is a `bool` literal is skipped outside a macro: neither
`ContainerRuntime::create(&CreateSpec)` nor `::start(&str)` takes one, and
`OpenOptions::create(true)` is the common case. Each is spelled with its receiver or path segment
and the character after it, and `in a macro argument` after that when it is one, so the pinned
list is readable.

## `fn stated_disallowed_methods(source: &str) -> Option<bool> {`

A module's own statement of `clippy::disallowed_methods` in production: its leading inner
attributes, comments blanked, read in order. An `allow` that holds outside tests — unconditional,
under `cfg_attr(not(test), …)`, or under any other `cfg_attr` predicate, conservatively — makes the
module unguarded; a `deny`, `forbid` or `warn` that holds in production guards it; `cfg_attr(test,
…)` is ignored; nothing stated is `None`.

## `fn parent_module(src: &Path, module: &Path) -> Option<PathBuf> {`

The file of a module's parent, by the path: `a/b/c.rs` and `a/b/c/mod.rs` are children of `a/b.rs`
or `a/b/mod.rs`, a top-level module is a child of `lib.rs`, and the crate roots have none. A parent
that is neither file fails the census rather than being guessed.

## `fn clippy_refuses_the_primitives_in(src: &Path, file: &Path) -> bool {`

The nearest stated level up the parents; with none stated anywhere, the lint's default `warn`,
which the gates' `-D warnings` makes an error.

## `fn the_lint_level_walk_reads_a_stated_or_inherited_allowance_and_a_production_forbid() {`

The walk over a written tree: a stated allowance, a silent child that inherits it, a production
forbid beneath it, and an allowance that holds in tests alone.


## `struct Scripted {` › `observation_unresolved: std::sync::atomic::AtomicBool,`

Set by `Runtime::leaves_its_observation_unresolved`: every later observation
answers `RuntimeError::Unresolved` before it reaches the fake, as a bounded
observation whose supervisor could not reap its process does. The fake's own
trace therefore records no `Observe` for it.

## `struct Scripted {` › `volume_inspection_unresolved: std::sync::atomic::AtomicBool,`

Set by `Runtime::leaves_its_volume_inspections_unresolved`: every later
credential-volume inspection is still asked of the fake, which records it in
the trace, and then answers `RuntimeError::Unresolved` whatever the fake said,
as a bounded `docker volume inspect` whose supervisor could not reap its process
does. The flag wins over a failure the fake was armed with, so one fixture can
run the failed-inspection control before it.

## `fn an_observation_that_leaves_a_process_unresolved_keeps_the_invocation_unresolved_after_a_clean_release()`

K2, "preserved process fate" (regular review 4, finding 2). A started
container's first observation reports a process it could not establish gone;
the release then stops and removes the container cleanly. The invocation
reports `ProcessFate::Unresolved`, with that description in its message: the
release established the container gone, not the observation's process. The
runtime is the scripted fake, whose removal knows nothing of the observation,
so only `contain`'s own rule can keep the fate. With `contain`'s fate read from
the release alone (the rule before K2) this reports `Gone` and fails.

L5 includes this companion guard because it has the same retained-reaper
lifecycle as J6's unresolved-start guard. Its assertions now live in
`assert_unresolved_observation_fate`. Unix executes them through
`unresolved_process_in_an_owned_helper`; Windows, which has no Unix container
reaper, executes the same assertions directly.

## `fn a_prestart_failure_whose_cleanup_leaves_a_process_unresolved_keeps_the_invocation_unresolved()`

PR12-R8-PRESTART-CLEANUP-FATE (regular review 8, P2). An ordinary failure
before `docker start` — a reported image id that differs from the record, then
a create the runtime refuses — whose cancel's removal answers
`RuntimeError::Unresolved` (the fake's `leave_a_process_unresolved_at_removal`,
as an unresolved question or attached client would). Each invocation must
report `ProcessFate::Unresolved`, not a plain cancellation, with the R19 view
and the R26 intent kept, the ledger's registration and slot pair kept, and no
`RunnerSpawnFailure` verdict; its message carries both the primary failure and
the removal's own fact, and the container the runtime created is removed. The
same-shape control runs first on the same runner: a mismatch whose cleanup is
clean reports `NeverStarted` and releases all four. The fate matrix
(`the_runner_reports_what_it_established_about_the_process_when_it_fails`)
keeps the other controls: a create the runtime refuses and a created container
that reports another image (clean pre-start, `NeverStarted`), a runtime down
before create and a funnel refusal before `docker start` whose cancel cannot
reach the runtime (ordinary cleanup failures, still `NeverStarted`), and a
refused start whose cancel completes (confirmed post-start `Gone`).
`an_invocation_whose_removal_leaves_a_process_unresolved_is_never_reported_gone`
remains the started-path guard. With `cancelled` reading the cause's fate
alone, the rule before repair9, the first unresolved case reports
`NeverStarted` and this fails.

Unix runs the assertions through `unresolved_process_in_an_owned_helper` (case
`prestart-cleanup`): the unresolved fates leave the runner's reaper to the
production retained-reaper backstop, the lifecycle L5 gave J6's and K2's
guards. Windows runs them directly.

## `fn an_unresolved_credential_volume_inspection_keeps_the_invocation_unresolved_and_held() {`

PR12-R8-VOLUME-INSPECTION-FATE and PR12-REG6-VOLUME-INSPECTION-FATE, two
reports of one defect. The run's initial resolution
(`resolve::resolve_container` over the scripted runtime, whose
`inspect_volumes` is the preflight's own question) finds every recorded
credential volume. Then an agent probe, which mounts its agent's volume, meets
an invocation-time inspection that answers `RuntimeError::Unresolved`, and its
cancel's stop and removal are answered cleanly for a container that was never
created. It must report `Unresolved` with the R19 view and the R26 intent kept,
the registration and the slot pair held, no `RunnerSpawnFailure` verdict, and
the contextual R20 refusal in its message, and no create may have been asked.
Controls on the same runner run first: an absent volume (the R20 refusal; the
volume is never created) and an inspection the fake fails, both `NeverStarted`
with everything released. `a_create_whose_named_volume_is_absent_is_refused_before_any_effect`
stays the create-level absent and unreachable control. With the blanket
`Refused` the inspection had before repair9 the fate is `NeverStarted` and
this fails; with the fate typed but the cancel told `CauseFate::Settled` the
view and the intent are released and the retained-resources assertion fails,
so a relabelled fate alone does not pass. Unix runs through the owned helper
(case `volume-inspection`); Windows runs directly.

## `struct ProbeEnd {`

What one admitted agent probe's failed launch left: its fate, whether it was a
plain cancellation, its message, whether its R19 view and R26 intent exist,
whether `InvocationLedger` kept its registration and its slot pair after `end`,
and whether verification's classifier (`run::verified`) would settle a
`RunnerSpawnFailure` outage over it.

## `fn agent_probe_named(ordinal: u32) -> (RunnerRequest, ContainerName) {`

An agent probe for `claude-code`, which receives a credential volume and a slot
pair, and its container's name. Distinct ordinals are distinct containers on
one runner, so one runner (one retained reaper) carries a guard's controls and
its cases.

## `fn run_admitted(`

One probe, accounted for by the production decisions rather than copies of
them. `InvocationLedger::register_slotted` under `Standing::preflight()` admits
it with its pair, and the premise asserts the slot is held while it runs; the
runner runs it; `InvocationEnd::of` hands the outcome to
`InvocationLedger::end` as `RegisteringRunner::settle` does; and `run::verified`
classifies the runner's error as an integration verification would. The R19
view and the R26 intent are read from the fixture afterwards.

## `fn assert_settled_and_released(case: &str, end: &ProbeEnd) {`

A settled pre-start failure: `NeverStarted`, the view and the intent released,
the registration and the slot released, and the `RunnerSpawnFailure` outage
verification settles over it. The controls make the ledger and verifier probes
discriminating.

## `fn assert_unresolved_and_held(case: &str, end: &ProbeEnd) {`

An unresolved one: `Unresolved` and no plain cancellation; the view and the
intent kept; the registration and the slot kept; no spawn outage; and a message
naming the fate and the deliberately retained residue. The resource assertions
come before the message so that a cancel which releases them fails at them.

## `fn assert_unresolved_prestart_cleanup_fate(fixture: &Fixture) {`

The assertions of
`a_prestart_failure_whose_cleanup_leaves_a_process_unresolved_keeps_the_invocation_unresolved`:
the clean control, the two unresolved cases, and no `docker start` anywhere.

## `fn assert_unresolved_volume_inspection_fate(fixture: &Fixture) {`

The assertions of
`an_unresolved_credential_volume_inspection_keeps_the_invocation_unresolved_and_held`:
the initial resolution, the absent and failed controls, the unresolved case,
and the trace (no `Create`; each cancel's `Stop` and `Remove` asked; no
container left).

## `const STALLED_OBSERVATION_DEADLINE: Duration = Duration::from_secs(10);`

The timeout the deadline and cleanup modes give their invocation (R6-N2,
regular review 6, P2; the concurrency lens's PR12-CI-02 and the regression
lens's PR12-K2-OBSERVATION-PREMISE-DEADLINE are the same defect).
It was 300 ms, chosen without counting what a deadline here covers. `contain`
starts the deadline before reaper coverage and the launch, and
`DockerCli::exec_within` asks nothing once it has passed, so a launch that
spent the 300 ms never asked the observation this witness stalls, and the
child failed its premise instead of testing it. Native10's macOS deadline child
failed exactly there, at `observed.exists()` (run 41904743958, job
126836335262; its whole test, fixture included, took 2.94 s). That run does not
say which phase was slow, and no failure rate was measured; what the repair
removes is the budget. On the idle build box the observation is asked about
171 ms after the helper calls `run_blocking_with`, a moment before `contain`
starts the deadline, so roughly half the old deadline was spent before the
stub was reached.

What the deadline covers before the first observation, read from the source:

- arming the runner's container reaper (`Reaping::cover`, `arm_container_reaper`):
  a fork and its READY handshake, which production lets take
  `HELPER_READY_BUDGET` (2 s) before it gives up;
- writing the intent and materializing the Git view: file work, no process;
- three supervised questions, `docker create`, `docker container inspect` and
  `docker start`: each starts its own cleanup reaper, under the same 2 s
  budget, and the stub's `sh`;
- the observation's own cleanup reaper and `sh`, up to the marker.

The job-control guard is not among them: the fixture's setup started it. Five
helper starts that production tolerates up to 2 s each make 10 s, and the four
shell starts and the file work are small beside them. That is the stated
assumption: a launch whose helpers each start within their own production
budget reaches the observation inside the deadline. A slower launch fails the
premise with the invocation's result, its elapsed time and its timeout, and is
a slow or wedged launch, never a deadline or fate defect.

No clock seam can force the interleaving instead: `contain` and `supervise`
read `Instant::now()` directly, and this repair leaves `exec.rs` byte-for-byte
unchanged. A witness of the production deadline over the production `contain`
therefore needs its launch to fit inside the deadline it was given; the repair
makes that a budget with stated phases rather than a narrow interval, and
establishes readiness at the observation boundary itself, the stub's marker,
reporting on every run how long it took to reach it.

## `const STALLED_OBSERVATION_SETTLES: Duration = Duration::from_secs(20);`

How long an invocation may take to settle after what ends it: after the
deadline in the deadline and cleanup modes, whose bound is the deadline plus
this, and after its start in the cancel mode, whose cancellation follows the
stub's marker. It is K2's 20 s bound with its meaning unchanged: far above what
a loaded runner needs for the bounded questions a mode asks after its end (the
cleanup mode's stalled stop and removal take about 2.2 s on the build box),
and far below the minute the stub stalls, so an observation that neither the
deadline nor the cancellation ended fails it.

## `const STALLED_OBSERVATION_DOCKER: &str = "#!/bin/sh\n\`

The stub `docker` of the stalled-observation witness, written by a process of
its own (`write_program_in_its_own_process`, so no fork of this process holds
it open for writing when the child executes it). A `ps` filtered by name,
which only the supervision asks, records that it was asked in the file
`UPSTROKE_TEST_OBSERVED` names and then sleeps a minute; a `ps` filtered by
label, the container reaper's, lists nothing. `create` and `logs` answer at
once, `start` at once or after the whole seconds `UPSTROKE_TEST_START_HELD`
names, `container inspect` answers the recorded image id or an exit status
of 0, and `stop` and `rm` answer at once, or sleep a minute when
`UPSTROKE_TEST_CLEANUP_STALLS` is set. Anything else exits 93.

Nothing else asks the name-filtered `ps` here. The release's stop and removal
either answer, or stall and end at the question bound with no daemon answer
for `settle` to confirm by observation, and the collection lists nothing. So
the marker exists only if the supervision asked its observation, which
`exec_within` does only before the deadline. Each stall `exec`s into `sleep`,
so it stays the supervised process itself.

## `fn a_running_container_whose_observation_stalls_settles_at_its_deadline_or_cancellation_and_keeps_its_fate()`

K2, regular review 4, finding 2 (P2, liveness; prior
PR7-STD-CONTAINER-EXEC-UNBOUNDED). The reviewer's witness: an invocation with a
100 ms timeout took 1.063 s against a stub whose observation slept one second.
Here the stub's observation sleeps a minute, and the whole production path
runs, `ContainerRunner` over the real `DockerCli`, in an isolated child of
this test binary whose `PATH` alone carries the stub
(`run_test_isolated`, which keeps the child's output for the failure message
and kills its process group at two minutes). Three modes, a shell probe each:
- `deadline`: a `STALLED_OBSERVATION_DEADLINE` (10 s) timeout. The observation
  ends at the deadline, the invocation is collected as timed out (no exit
  code) and released, within `STALLED_OBSERVATION_SETTLES` of the deadline;
- `cancel`: a ten-minute timeout and a cancellation once the stub records the
  observation. The invocation reports the cancellation with fate `Gone`
  within `STALLED_OBSERVATION_SETTLES`: the release's stop and removal
  answered;
- `cleanup`: the deadline mode with `stop` and `rm` stalled too and the
  runtime's own bound shortened to one second (`with_question_bound`). Each
  cleanup question ends at that bound, and the invocation reports fate
  `Unresolved`, with its intent kept for the census: neither the stop nor the
  removal was confirmed, and the bound does not turn that into `Gone`. Its
  `docker start` is also held for 2 s (`UPSTROKE_TEST_START_HELD`): the
  in-suite setup-delay case the concurrency lens (PR12-CI-02) and the
  regression lens (PR12-K2-OBSERVATION-PREMISE-DEADLINE) asked for. Every run
  shows a launch delayed far past the former 300 ms deadline still reaching
  the stalled observation, and the helper requires the observation to come
  after the hold, so a hold that never happened cannot pass as one. The hold
  costs no time: this mode's length is its deadline's.

Each mode runs in two helpers (R6-N1), one ending in an ordinary return and
one in a deliberate unwind after its runner dropped. The parent requires the
helper's successful exit, libtest's `1 passed`, exactly one job-control-guard
collection marker and one ECHILD marker, and the mode's reaper evidence: in
the cleanup mode exactly one `collected retained stub reaper` marker and no
disarm marker; in the deadline and cancel modes exactly one `disarmed its
container reaper on drop` marker and no retained-reaper collection. A helper
that settled with another fate, kept a reaper it should have dropped, dropped
one it should have kept, or left a direct child cannot pass. Each helper's
captured output is reported through the fixture's `say` funnel.

Measured on the build box before commit (K2): with the supervision's
observation asked under the runtime's own bound instead of the invocation's,
the deadline mode took 60.4 s and failed; with the cleanup questions given ten
minutes, the cleanup mode was killed at the parent's two-minute bound and
failed. Repair7, same box (Linux; not macOS evidence): all six helpers pass.
The deadline and cancel modes asked their observation about 171 ms after the
invocation began, the cleanup mode, its start held 2 s, after 2.14 s; the
deadline mode settled after 10.26 s, the cleanup mode after 12.19 s and the
cancel mode after 0.34 s; the parent took 49 s.

Bounded controls on the same box, each one exact replacement restored
byte-for-byte. Replacing the cleanup mode's `reaper.collect()` with
`std::mem::forget(reaper)` (R6-N1's omission witness) fails `cleanup/return`
at the helper's direct-child census (`waitpid=0`), and so the parent, while
the deadline and cancel helpers still pass. Holding the stub's `docker start`
5 s in every mode still reaches the stalled observation in all six helpers,
about 5.15 s in. Holding it 12 s, past the deadline, fails `deadline/return`
at the premise, whose message names the start that ended at the deadline
(`ended before it answered`), 10.16 s and the 10 s timeout; nothing hangs.
Two temporary controls of `container.rs` answer the regression lens's
proposed guard; `exec.rs` is never touched. Asking the supervision's
observation under the runtime's own bound instead of the invocation's fails
`deadline/return` at the settle bound after 60.39 s (K2's original bypass
took 60.4 s). A release that reports its container gone fails
`cleanup/return` with fate `Gone` where `Unresolved` is required.

## `fn stalled_observation_child() {`

Ignored because only the parent above supplies its mode, ending, marker and
stub. R6-N1 (regular review 6, P2): until repair7 this helper returned with
its cleanup mode's container reaper, which `Reaping::drop` deliberately keeps
(`kept_until_the_process_exits`), left to outlive the helper, and in every
mode with the job-control guard its fixture's setup started left to the
helper's exit; `run_test_isolated` collects the helper, not the helper's
children. It now owns both, as L5's `unresolved_process_child` does.
`IsolatedJobControl` is constructed before the fixture, whose setup starts the
process-global guard and its probe. `RetainedStubReaper` is constructed
before the invocation, for the runner's exact scope, with the `DockerCli`'s
own `reaper_program()` and the stub the parent wrote.
`assert_a_stalled_observation_settles` runs the invocation and drops its
runner. The cleanup mode then requires the scope still registered, the
production backstop having kept the reaper; the deadline and cancel modes
require it disarmed, their runners having settled `Gone` and cancelled and
waited for their own reapers on drop, and say so. On ordinary return the helper
collects the retained reaper (cleanup) and the guard explicitly; on the
deliberate unwind their Drops do, and in the other modes the reaper owner finds
nothing registered. The catch accepts only that exact deliberate panic; any
other failure still fails the helper. Finally
`assert_no_children_in_isolated_test` requires ECHILD before the parent
collects the helper.

ECHILD is a direct-child claim. It covers the helper's own children: the
job-control guard, the container reaper, and each cleanup reaper and stub
process the `docker` questions started, which their supervisions end and
collect before the questions return; the guard's collection requires that no
command group is still registered. It is not proof about arbitrary
descendants. The retained reaper's no-grandchildren argument is in
`agent/proc.md` (`RetainedStubReaper`), and the guard's distinct collected
status proves its probe reaped. The environment keys select the mode, ending,
marker, image and stub, the cleanup stalls and the start hold; the scratch
trees and output files keep their RAII owners.

## `fn assert_a_stalled_observation_settles(`

One mode's invocation: the runner over the real `DockerCli` (its question bound
shortened to one second in the cleanup mode), the shell probe with the mode's
timeout, a watcher, and the assertions; the runner drops when this returns or
unwinds, before the helper looks at its reaper. The watcher polls for the
marker every 10 ms until it appears, the invocation ends or a minute passes,
records how long after the invocation began it appeared, and in the cancel
mode then cancels (at the minute if the marker never came). The premise is
the unchanged `observed.exists()`; when it fails, it names the invocation's
result, elapsed time and timeout, which tell a launch refused or ended at the
deadline (`docker start` not asked, or ended before it answered) from one that
started and was never observed. A launch the parent held at `docker start`
must have reached its observation after the hold. Then the bound, `took`
under the deadline plus `STALLED_OBSERVATION_SETTLES`
(`STALLED_OBSERVATION_SETTLES` alone in the cancel mode), and the unchanged
outcomes: timed out with no exit code; cancelled with fate `Gone`; fate
`Unresolved` with the intent kept. Every run says how long the observation
took to reach and the invocation to settle.

## `fn review_timeouts_obey_the_platform_deadline_range() {`

The K3 regression runs `review_deadline_child` through `HostRunner` and its existing
cross-platform process supervisor with a two-minute bound. It requires a non-timeout exit, code zero and libtest's
`1 passed`, so a wedged helper, failed assertion or misnamed test fails the parent. The bound
covers fixture setup and the whole helper lifecycle, independently of the review timeout.

K3 originally named this guard
`a_review_timeout_the_configuration_accepts_and_no_deadline_can_hold_is_refused_before_anything_is_written`.
That test assumed every platform rejects `i64::MAX` seconds. Its fake never exited after
start and its zero-delay watch kept recording observations on platforms that could represent
the deadline. Repair6 corrects the premise and bounds that lifecycle; it does not attribute
the historical native Windows allocation failure to this test.

## `fn review_deadline_child() {`

An owned helper, ignored in the default suite and always invoked by the parent above. The
original configuration, `[routing] review = { tier = "frontier", timeout_secs =
9223372036854775807 }`, still passes through `config::load`, `review::plan_for` and
`ReviewPlan::pass_timeout`; the exact accepted duration is asserted. That duration is sent
unchanged to the runner, with the outcome selected by the platform's `Instant::checked_add`
range. Two additional cases assert their premises before invoking the runner: sixty seconds
is representable, and `Duration::MAX` is not. Both outcomes therefore run on every platform;
the overflowing case cannot silently become an untested branch.

Each case has its own fixture, whose fake exits immediately when started. Its ten-millisecond
poll prevents a non-exiting regression from accumulating an unbounded rate of trace entries
while the parent's supervisor enforces the helper bound. No sleep establishes readiness.
The request owns a copy of the task path while the fixture retains its cleanup paths.

All invocations remain under `catch_unwind`. An unrepresentable deadline must return a
non-cancellation `NeverStarted` refusal naming the supplied seconds, with an empty funnel
and runtime trace, no intent and no container. A representable deadline must complete with
code zero, without timing out, and with start, collection and removal recorded. Every case
finishes with no intent or container. Restoring unchecked addition fails the overflow case;
requiring refusal independently of the platform range fails the representable control.

## `fn a_start_that_left_a_process_unresolved_stays_unresolved_after_a_clean_cancel() {`

`PR12-J6-OBSERVATION-FATE-DROPPED`: a start that reports a process of its own
unresolved keeps that fate when cancellation stops and removes the container.
The removal establishes the container gone, not the other process. DockerCli
also holds such a process under the container, so its removal reports it;
this scripted runtime's removal does not. `assert_unresolved_start_fate`
retains the original fate, message, Start/Remove and empty-container assertions.

L5 moves this invocation into an owned, bounded helper on Unix. The original
test dropped a temporary runner in libtest's process; its unresolved fate
selected the production retained-reaper backstop, leaving the reaper beyond
that test executable's exit. Windows retains the direct assertion because that
Unix reaper does not exist there.

## `fn unresolved_process_in_an_owned_helper(case: &str) {`

J6's start case, K2's observation case, and repair9's pre-start-cleanup and
volume-inspection cases run `unresolved_process_child`
under `run_test_isolated` and `FIXTURE_CHILD_BOUND`. Each runs twice: ordinary
return and deliberate unwinding after the runner dropped. The outer assertion
requires the collected helper's successful exit, libtest's `1 passed`, and
exactly one retained-mock-reaper collection marker and one job-control-guard
collection marker, plus the final ECHILD marker. These markers arrive on captured
stderr through the fixture's `say` funnel. The successful captured output,
including the collected pids, is reported through that same funnel for the
guard's evidence. A wrong helper name, failed fate assertion, uncollected
reaper/probe or failed unwind cleanup cannot pass.

## `fn unresolved_process_child() {`

Ignored because only the owner above supplies its case and ending.
`IsolatedJobControl` is constructed before scratch-fixture setup, which starts
the process-global job-control guard and its probe. `RetainedMockReaper` is
constructed before the invocation; the runner's temporary then drops through
the unchanged production backstop. The helper
requires the scoped reaper to remain armed. On ordinary return it explicitly
collects both owners; on the deliberate panic their Drops collect their
children. The catch accepts only that exact deliberate panic; other failures
still fail the child. Finally `assert_no_children_in_isolated_test` requires
ECHILD, before the outer owner collects this helper.

ECHILD is a direct-child claim. The separate no-grandchildren argument is in
`agent/proc.md`: this retained fake-runtime reaper has no supervised command
process group or anchor,
receives no cleanup command while the helper lives, and is killed and reaped
before helper exit can trigger the runtime-cleanup fork path. No real container
is started. The separate job-control probe is reaped by its own guard using a
test-only shutdown command, and only that successful wait earns the guard's
distinct exit status, which its owner requires. The two environment keys only
select the fixture and return/unwind mode. Scratch trees and output files retain
their existing RAII owners.

Measured omission witness: replacing the explicit `reaper.collect()` with
`std::mem::forget(reaper)` in the return case fails both guards at the helper's
direct-child census (`waitpid=0`). The job-control guard and probe have already
been collected, so that check sees the retained reaper omitted by the mutation.
The unchanged fate assertions still run. The witness covers omitted normal
collection; the same collection routine is exercised during the deliberate
unwind, not separately mutation-tested there.

## `#[cfg(target_os = "linux")]` › `const LEASE_KILL_CASE: &str = "UPSTROKE_TEST_LEASE_KILL_CASE";`

`PR12-INV9-CONTAINER-HELPER-CLEANUP-LEASE`, the carriage: a killed coordinator's Docker
helpers keep admission refused until they are settled. Four cases, each a real
`ContainerRunner` over `DockerCli` and the process funnel, invoked from a worker thread inside
no cleanup scope with the `Carried` call a pipeline is given, by a coordinator that holds its
worktree lock and run lock as an admitted run does: the synchronous `docker create`, the
attached start's `docker start --attach` (its own thread), the credential volume's
`docker volume inspect` (asked before any create) and the `docker stop` of the cancellation
after a refused create. The stub pauses the case's question; the harness kills the
coordinator and requires both the run's admission (`RunLock`, the exclusive cleanup probe)
and a different run's worktree admission (`WorktreeLock`'s scan of every run's cleanup hold)
refused while the paused helper's reaper is live, then admission only once every covered
helper, reaper and backstop has ended, then every descendant collected. The harness, its
collector and the coordinator handle are adapted from an earlier candidate's: the
coordinator now also takes its worktree lock, the admission asks include the different
run's, and the handle fails closed (below).

## `#[cfg(target_os = "linux")]` › `fn children_of(pid: &str) -> Result<Vec<libc::pid_t>, String> {`

Why this helper answers only from a pass over a process's tasks that nothing changed under
(`CI-FLAKE-HANDOFF-CHILDREN-LIST-RACE`, a defect of the test helper: no product code reads
these lists). The helper lists a process's tasks, `/proc/<pid>/task`, and then reads each
listed task's children list. The listing and the reads are separate system calls, not one
snapshot, and a thread can end between them. In CI run 42918059287, job 130185502003 (the
Ubuntu test job of pull request #360, a change to documentation alone),
`handoff_harness_child`'s `expect` on the coordinator's children lists panicked on a list
that was not found: no such file or directory. Which of the coordinator's threads had ended
is not known. One candidate is the thread of `handoff_coordinator_child` that reports the
backstops: it returns as soon as its report is written, and that report is what the harness
waits for before it reads the lists.

**A vanished task is not read as a task with no children.** When a thread ends, the kernel
hands its children to another live thread of the same process, and that may be a thread
whose list the pass has already read. A thread that ends during a pass shows in one of two
ways. Its list is not found, when it ended before the list was opened. Or its list is read
empty, when it ended after the list was opened: the read of the already-open file succeeds
with nothing in it. The first repair answered only the first way, by listing again on a
not-found read. A review reproduced the second: that repair took the empty read as the
thread's answer and made no further pass, and the child the thread had handed to a task
already read was in neither list.

**A task still listed may already be ending.** The second repair compared the two listings
of a pass, which catches a thread that has left the listing. A second review reproduced what
that misses: the kernel hands an ending thread's children on (`exit_notify`) before it takes
the thread out of the task listing (`release_task`). So: list A and B; read A's list, empty;
B begins to end and its child goes to A while B is still listed; read B's list, empty; list
A and B again. The two listings are equal, the pass counted, and the child was in neither
answer.

**The rule.** `children_in_passes` holds it, apart from the filesystem. A pass lists the
task ids, reads every listed task's list, asks of every listed task whether it is still
running normally, and lists the task ids again. It counts only if no read failed with
not-found, every listed task was still running normally when asked, after the reads, and
the second listing holds exactly the task ids of the first. A thread that ended during the
pass is missing from the second listing, whichever way its read went; a thread that began
to end during the pass and is still listed is reported as not running normally. A pass that
does not count is discarded whole, the lists it did read included, and another is made,
within the same five, after a pause. A pass that counts answers its children sorted and
without repeats.

**The pauses.** A thread that is ending stays listed, in its dead state, until the kernel
releases it, and it can be descheduled in that window. Five passes made one straight after
another take a few microseconds, so all five could see the same ending thread, be
discarded, and end in the error, which callers that `expect` the helper turn into the
intermittent failure this repair set out to remove. So between a discarded pass and the
next the helper waits, and the wait grows: 1 ms before the second pass, 5 ms before the
third, 20 ms before the fourth and 50 ms before the fifth, about 76 ms in all, and none
after the fifth. The waits are a margin for the release, not a measurement of it.

**Running normally,** as the helper reads it, is the state field of the task's own
`/proc/<pid>/task/<tid>/stat`, the first field after the command, which closes at the last
`)`. A task whose state is zombie (`Z`) or dead (`X`, or `x` on older kernels) is ending,
and so is a task whose `stat` cannot be read because it has gone (not found, or `ESRCH`).
Every other state, sleeping and stopped included, is running normally. Any other failure to
read the state, or a `stat` with no state field, is an error returned at once. The kernel
sets an ending thread's exit state in the same step, `exit_notify`, in which it hands its
children on. So if a thread's state, asked after every list was read, is still not an exit
state, it had not handed its children on when its own list was read, and they were in that
list; and if it had handed them on before that, its state, asked later, says so.

**The bound.** At most five passes are made. One more pass answers the interleaving that was
seen, and the rest absorb a few threads ending in quick succession; five is that allowance,
not a measurement. If the fifth pass does not count either, the helper returns an error that
says what changed in it, the lists not found, the tasks not running normally and the two
listings, and never an answer; a kernel with no children lists at all fails the same way,
after five passes. Every other failure is returned at once, as before: a task directory that
cannot be listed (as when the process itself is gone), a list or a state that fails for any
other reason, and a child that does not parse as a pid. Every caller therefore still
receives an error for a list that stays unreadable, and what each does with one is
unchanged. Each error names the task directory, `/proc/<pid>/task`, and every path is built
by joining `Path` components, never by formatting.

**What remains outside the rule.** A process's main thread that ends before its other
threads stays listed as a zombie until the whole process ends. Under this rule every pass
over such a process is refused, so the helper answers it with an error after five passes,
never with children: it fails closed, and no child is lost to it. No process this harness
reads is known to end its main thread first. The argument that the state closes the
hand-off window is read from the kernel's exit path, not observed in a run: the reads of
the lists and of the states take no lock in common with the exit, so it rests on the order
of the steps in `exit_notify`, not on one snapshot of the process. And, as before, a child
forked during a pass by a thread already read is not in the answer: a list read is
discovery, never evidence that no child remains.

**What witnesses it.** The interleavings cannot be produced on demand on a live process, so
the rule is tested apart from the filesystem: the tests below drive `children_in_passes`
with scripted closures.

## `#[cfg(target_os = "linux")]` › `fn children_in_passes<T: Ord + std::fmt::Debug>(`

The rule of `children_of`, over four closures: `list_tasks` lists a process's task ids,
`read_children` reads one task's children list, `running_normally` says whether one task
is still running normally, neither ending nor gone, and `pause` waits the time it is given.
`children_of` gives it the three readers of the proc filesystem: the entry names of
`/proc/<pid>/task`, which are the task ids, the text of `<task>/children`, and the state
field of `<task>/stat` (zombie, dead, or a `stat` gone is not running normally); and, as
`pause`, `std::thread::sleep`. The core chooses the waits, 1, 5, 20 and 50 ms, and asks
`pause` for the next one before each pass after the first, that is after each discarded
pass that has a pass after it, so at most four pauses and none after the fifth pass. The
scripted tests give a `pause` that records the wait it is asked for and does not sleep.
A read's error keeps its kind, which is how a not-found read
is told from every other failure. After a pass's reads the core asks `running_normally` of
every task of the first listing, in order, and only then lists the tasks again; a task
reported not running normally discards the pass, and a failure to answer is returned at
once. The two listings of a pass are compared as sets. The task id is generic, so the tests
below name their tasks (`"A"`, `"B"`, or a number) and read no proc filesystem. The core's
errors name the task; `children_of` puts the task directory in front of each.

## `#[cfg(target_os = "linux")]` › `fn an_engines_children_are_not_lost_when_a_task_ends_after_its_list_was_opened() {`

The review's interleaving, scripted. The first listing holds A and B. A's list reads empty;
B's list, opened before B ended, reads empty too, and B's child, 4242, is now A's, whose
list this pass has already read; the second listing holds only A. The next pass lists A,
reads A's list holding 4242, and lists A again. The core must answer 4242 from that second
pass: four listings, and the reads of A, B and A in that order.

Measured witness, restored byte-for-byte: with the comparison of the two listings made
`if false`, so that a pass counts without it, this test fails at its first assertion,
`left: Ok([])` against `right: Ok([4242])`: the first pass counted and the child was lost.
The test below with tasks changing under every pass fails too, answering `Ok([4242])` where
the error is required; the not-found test still passes. Its `running_normally` answers every
task running, so the listing comparison alone discards its first pass.

## `#[cfg(target_os = "linux")]` › `fn an_engines_children_are_not_lost_when_a_task_still_listed_is_ending() {`

The second review's interleaving, scripted. The first pass lists A and B, reads A's list
empty and B's list empty, and is then told that A is running normally and that B is not: B
began to end after its list was read and handed its child, 4242, to A, whose list this pass
had already read. B is still listed, so the second listing holds A and B again and agrees
with the first. The core must discard that pass and make another: it lists A, reads A's list
holding 4242, finds A running normally, lists A again, and answers 4242. The test records
every call to the three closures in one log and requires that order exactly: listing, the
reads, the states asked after the reads, the second listing, then the same for the second
pass.

Measured witness, restored byte-for-byte: with the core ignoring `running_normally`'s
answer (its `Ok(true)` arm made `Ok(_)`, so a task reported ending is taken as running),
this test fails at its first assertion, `left: Ok([])` against `right: Ok([4242])`: the
first pass counted and the child was lost. The other three tests of the core still pass.

## `#[cfg(target_os = "linux")]` › `fn an_engines_children_are_found_after_pauses_when_a_task_stays_listed_while_ending() {`

The ending thread that stays listed, scripted across passes. B has handed its child, 4242,
to A and is reported not running normally on each of the first three passes, while both
listings of each of those passes hold A and B and both lists read empty. On the fourth pass
B is gone: both listings hold A alone, and A's list holds 4242. The core must answer 4242
from that fourth pass, and must have asked `pause` for 1, 5 and 20 ms, one wait after each
of the three discarded passes, growing. The test records every call to the four closures
in one log and requires that order exactly, each discarded pass followed by its pause, and
requires that every scripted listing, read and state was used.

Measured witness, restored byte-for-byte: with the core's call to `pause` removed, so that
the passes follow one another with no wait, this test fails at its first assertion, on its
count of pauses, `left: []` against `right: [1ms, 5ms, 20ms]`. The test of tasks changing
under every pass fails too, on its pauses, `left: []` against
`right: [1ms, 5ms, 20ms, 50ms]`; the other three tests of the core still pass.

## `#[cfg(target_os = "linux")]` › `fn an_engines_children_are_read_in_another_pass_when_a_list_is_not_found() {`

A read that fails with not-found makes another pass. The first pass's two listings agree, A
and B both times, so the not-found read of B's list is the only thing that discards it, and
its read of A, which found child 7, is discarded with it. The next pass lists A, reads A's
list holding 8, and lists A again; its answer, 8 alone, is the one returned.

Measured witness, restored byte-for-byte: with the not-found arm returning its error at once
instead of discarding the pass, this test alone fails at its first assertion,
`left: Err("the children list of task \"B\" was not found (entity not found)")` against
`right: Ok([8])`.

## `#[cfg(target_os = "linux")]` › `fn an_engines_tasks_that_change_under_every_pass_end_in_an_error_after_five_passes() {`

Every listing holds a task id that no listing before it held, so the two listings of every
pass differ, while every read finds a child. The core must answer an error after exactly
five passes, ten listings, saying what changed in the last pass, and never the child its
reads found. The scripted lister refuses an eleventh listing, so a core that lost its bound
fails here rather than looping. The core must also have asked `pause` for exactly four
waits, 1, 5, 20 and 50 ms, one between each pass and the next and none after the fifth.

## `#[cfg(target_os = "linux")]` › `fn helper_reaper_candidates(`

Which process the lease-kill harness calls the paused helper's reaper (finding
`PR356-LEASE-KILL-REAPER-SELECTION`, a test-fixture identity fix). The process funnel
forks the command reaper before it spawns the command. The reaper takes a process group of
its own and holds the run's cleanup leases. The command, the stub that pauses, takes its own
group and registers it with the reaper, which forks the group's anchor into that group. When
the coordinator dies, the reaper kills the group and then settles under its leases for the
test-build delay. That reaper is the process whose liveness the refusal assertions require,
before and after the admission asks.

**The ambiguity.** Through 5008aaac the harness took any child of the coordinator that had a
child of its own in the helper's group, and when several matched it silently kept the last.
The helper is a child of the coordinator too. It publishes its acknowledgement with an
external `mv` before it execs `sleep`, and until that `mv` is gone it is a child of the
helper, in the helper's group. So by construction the helper could be named its own reaper.
The real reaper would then not be the one watched.

**The selection now:**

- A candidate is a child of the coordinator that is neither the helper nor a member of the
  helper's group.
- The candidate's anchor is a child of the candidate's own that is in the helper's group and
  is not the helper.
- Both are identified by pid and start time.
- Exactly one pair must exist, and a second scan must find the same pair. None or several
  fails the case with the candidates listed, never resolved by the order of a scan.
- A line names the helper, the anchor and the reaper with their command names.

Everything after the selection is as before:

- both refusals and the before/after `Live` witness of the chosen reaper;
- the admission loop, and its ending check over the same covered set (helper, reaper,
  backstops);
- the natural settlement to the kernel's account, and the collection accounting;
- the four cases and every bound.

**The native red this answers** (raw, not regraded). In CI run 42390677121, attempt 1, job
128457402436 `test (ubuntu-latest)` at 5008aaac, the `pre-create-cancel` case failed at the
live-reaper premise. Two facts were recorded:

- The chosen reaper, pid 307163 with recorded command name `sleep`, was already
  `Ended("exited (state Z) and not yet collected")` before and after the ask.
- Both the run's admission and a different run's worktree admission were refused.

The drop then reported `complete: true`, 5 collected, no errors and none left. That is the
bounded cleanup after a failed assertion. The natural-settlement path never ran, so it is
not a natural-settlement success. The test file is byte-identical to PR355's landing. The
coordinator's fingerprint records the same test passing in PR355's three Linux runs, as
history only.

**What this does and does not claim.** The recorded command name `sleep` is consistent with
the helper having been chosen as its own reaper. The trace holds neither the helper's
acknowledged pid nor a parent/anchor snapshot, so that self-selection is not established. No
runtime cause is claimed, and the failure is not called flaky. The change removes the
ambiguity from the source by construction. No focused or repeated run was made for it; the
four cases run as ordinary tests in the suite.

## `#[cfg(target_os = "linux")]` › `struct OwnedDescendants {`

The collector every Group A harness ends with. It makes the harness a child subreaper, so
everything a killed coordinator (or a contender) leaves is re-parented to it; it identifies
children by pid and start time, collects only children the kernel names ended, and signals
only its own listed, unreaped children. **One waiter per child:** a direct child a
`RunningTestChild` handle owns is collected by that handle, and no harness calls the
collector while a handle still owns a child; the collector reaps only adopted descendants,
once every handle has collected its own.

## `#[cfg(target_os = "linux")]` › `fn handoff_stub() -> String {`

The handoff guards' shared `docker`: the killed coordinator's runner, its backstop and the
contender's census all see one set of containers, kept as files with their labels, and every
call is logged with its caller's pid so each removal is attributed to its owner.

## `#[cfg(target_os = "linux")]` › `fn handoff_coordinator_child() {`

The coordinator a handoff harness kills between two helper questions: a real
`ContainerRunner` invocation from a worker thread with the `Carried` call, paused by its
observer at `Container.Start`'s `Before` -- after the intent, the view and the container
exist and every earlier question has returned, so none is in flight. The backstop it armed
renders the test gate the harness names.

## `#[cfg(target_os = "linux")]` › `fn handoff_contender_child() {`

The distinct fresh run a harness admits to the same physical worktree, as a process of its
own so the harness forks nothing itself: it takes `WorktreeLock` (admission and its cleanup
scan), runs the real startup container census as a fresh run of another incarnation on the
harness's word, reports what it reclaimed, and lets the lock go on the harness's word.

## `#[cfg(target_os = "linux")]` › `fn handoff_harness_child() {`

`PR12-INV9-CONTAINER-HELPER-CLEANUP-LEASE`, the handoff: who owns a dead coordinator's
container cleanup when an engine is admitted to the same worktree around its backstop's
ending. The backstop is parked at test-build gates; three schedules: **early** (the contender
is admitted before the backstop lists), **middle** (admitted after the backstop has listed,
at the point where a backstop that took its cleanup hold before the worktree's lock would
take it; released, the backstop is parked again before it ends, and the contender's census
runs inside that interval -- the run shows no hold, the census classifies the container a
dead owner's and reclaims it with its intent and view, and the backstop then ends having
removed nothing) and **late** (the backstop holds the worktree and its lease; a distinct
run's admission and the run's own are refused; it removes the container and ends; the
contender then reclaims the intent and view). Exclusive responsibility is read from the
stub's per-caller log; convergence from the stub's containers, the intents and the view;
collection from the kernel's own account.

## `#[cfg(target_os = "linux")]` › `fn a_running_test_child_whose_status_another_waiter_took_is_neither_signalled_nor_credited() {`

The focused check of `RunningTestChild`'s fail-closed cleanup: when another waiter has
already collected the handle's child, ending the handle must send no signal -- its own
`try_wait` answers `ECHILD`, so the group it would signal is not known to be its own --
record `running test child collection: not established`, and answer `None`. The parent
tests refuse collection credit and keep their fixture whenever that record appears.
