# `src/engine/topology/profiled.rs`

Extended notes for [`src/engine/topology/profiled.rs`](../../../../src/engine/topology/profiled.rs).

The source defines behavior; these notes hold the module's contracts and rationale.
Each code span in a section heading is an exact source fragment. Search it as a fixed string
in the linked module, using the enclosing item to distinguish repeated lines.

## Module

Each agent request carries its agent's bound credential profile.

A pool in the pools file, `~/.upstroke/pools.toml`, may name a `profile`: the
credential directory its agent's CLI is to run under. The runners already honour
`RunnerRequest::credential_profile` (the host runner composes the child's
environment from it; the container runner refuses it before starting anything);
what was missing is anything that sets it. This module is that: a private
wrapper over the run's `Arc<dyn Runner>`, installed by `entry` on both the
fresh and the resume path right after the runner is built, so every request
the run makes — the creation-time probes, `RunPreflight`'s probes on resume,
workers and reviewers on either driver — passes through it.

It is engine-side and private (`mod profiled;`, not `pub mod`) because it is
an assembly detail of `entry`: nothing outside the topology engine builds a
run's runner.

**What it does not do.** It does not check the profile. Pre-flight does,
where a refusal can be reported before any agent CLI is started: a fresh run
refuses in `prepare_fresh`, before its lease, and a resume in its pre-flight,
under its locks. The profile must be an absolute path and an existing
directory. The agent's capability probe then runs under the profile, through
this wrapper. Nothing checks sign-in, so a profile that is not signed in
passes pre-flight and fails at its first attempt
(`PR386-PROFILE-AUTH-NOT-PREFLIGHTED`). It does not expand `~/`: the
configuration has no helper for that (the only home lookup is
`util::user_upstroke_dir`, which finds the home but does not rewrite paths),
so the profile is the string written in the pools file,
`~/.upstroke/pools.toml`, used as a path, and pre-flight refuses a `~/` one
as relative.

## `pub(super) fn profiled(inner: Arc<dyn Runner>, pools: &[Pool]) -> Arc<dyn Runner> {`

The one constructor. With no profiled pool it returns `inner` itself — the
same `Arc`, no wrapper — so a run whose configuration names no profile is
exactly the run it was before this module existed, down to the pointer.
That is the choice the task left open ("wrap nothing, or wrap with an empty
map"): wrapping nothing makes the pass-through a property of the type rather
than of a code path, and no existing test can observe a difference.

## `fn profiles(pools: &[Pool]) -> BTreeMap<String, PathBuf> {`

The agent-to-profile map, built once per run from the run's pools. For each
agent named by a pool, the agent's pool is `capacity::pool_for` — the first
pool for that agent, the same binding the attempt plans, the slot table and
the capacity report use — and only that pool's `profile` counts. A later
pool for the same agent never lends its profile to the first: if the agent's
bound pool has `profile: None`, the agent contributes nothing.

## `struct Profiled {`

The wrapper: the inner runner and the map. It holds nothing per request.

## `impl Profiled` › `fn stamped(&self, request: &RunnerRequest) -> Option<RunnerRequest> {`

`Some` only when the request has an agent and that agent has a profile; the
stamped request is the original with `credential_profile` replaced and every
other field unchanged. A request with no agent (a gate, the shell probe) is
never stamped. A request whose `credential_profile` was already set by its
builder is overwritten by the agent's bound profile — today no builder sets
one, and the pool binding is the authority.

## `impl Runner for Profiled {` › `fn run<'a>(&'a self, request: &'a RunnerRequest, call: RunnerCall<'a>) -> RunFuture<'a> {`

An unstamped request is handed to the inner runner as is, the same reference
and the same call. A stamped one is owned by the returned future, and the
call is re-lent at the future's local lifetime: `RunnerCall<'a>` is invariant
in `'a` (its hook references are `&mut dyn`), so the original call cannot be
passed alongside a borrow of a local. The re-lending takes the call apart
with `into_parts` and rebuilds it from exactly those parts — the same
cancellation, leases and hooks — so the inner runner sees the same call.

## `impl Runner for Profiled {` › `fn composed_common_dir(&self, request: &RunnerRequest) -> ComposedCommonDir {`

Delegated, over the same request `run` would hand the inner runner. Without
it the trait's default (`Unknown`) would replace the host runner's answer and
change what the workspace manager's closure checks see.

## `mod tests {`

The wrapper's own contract, through a recording inner runner. The run-level
proof — a run assembled with a profiled pool hands its worker that profile —
is `recover::tests::a_run_assembled_with_a_profiled_pool_hands_its_worker_that_profile`,
because the fixture that produces a real resumed run is that file's.

Requests are compared through their `Debug` rendering: `RunnerRequest` has
no `PartialEq`, and the rendering covers every field.

## `mod tests {` › `fn a_request_from_an_agent_with_a_profiled_pool_carries_that_profile() {`

A worker whose agent has a profiled pool arrives with that profile, and with
nothing else changed. A reviewer for a second agent takes its own agent's
profile, so the map is keyed by agent rather than "the" profile.

## `mod tests {` › `fn a_request_from_an_agent_whose_pool_has_no_profile_goes_through_untouched() {`

An agent whose pool names no profile is untouched even while another agent's
pool names one — the wrapper is installed then, and must still pass this
request through unchanged.

## `mod tests {` › `fn the_first_pool_for_an_agent_decides_its_profile_as_everywhere_else() {`

`capacity::pool_for` binds an agent to its first pool. A second pool for the
same agent with a profile must not be applied to an agent whose bound pool
has none.

## `mod tests {` › `fn a_request_with_no_agent_goes_through_untouched() {`

A gate request (no agent) is untouched even when a profiled pool exists.

## `mod tests {` › `fn with_no_profiled_pool_the_runner_is_not_wrapped_at_all() {`

The pass-through choice, held by pointer identity: with no pools, and with
pools that name no profile, `profiled` returns the runner it was given.

## `mod tests {` › `fn a_profile_is_the_configured_string_used_as_a_path() {`

A `~/` profile is passed as written. The configuration has no `~/` expansion
helper; if one is added, this test is the one to change with it.
