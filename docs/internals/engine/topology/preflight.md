# `src/engine/topology/preflight.rs`

Extended notes for [`src/engine/topology/preflight.rs`](../../../../src/engine/topology/preflight.rs).

The code is the authority for what it does; this file is the whole of its prose, moved out of
the source verbatim. Each section is headed by the line of code the comment sat above, spelled
as it is in the source, so the heading is the grep string that finds the code.

## Module

The `RunnerPreflight`: the one observation of shell and CLI availability
inside the run's boundary.

[`crate::runner::container::resolve::RunnerPreflight`] is a **seam with no
production implementation**, and its own module says why: the probes are
`crate::runner::host::run_shell_probe` over the run's Runner, and building
that runner "needs a run identity, a private root and a slot pair that
`TopologyRun` owns at PR7". This is that implementation.

INV-23, which is the whole specification of this file:

> the `RunnerPreflight` — one non-slotted shell probe (the recorded shell
> executing `exit 0`) and one slotted probe per recorded agent, each a
> registered invocation through the run's Runner — executes at P4 and at
> every resume's recovery step (c) and is the only observation of shell and
> CLI availability inside the boundary.

### The asymmetry is the point

The shell probe is **non-slotted** and the agent probes are **slotted**.
`permits.agent_pool_slots` excludes two invocations by name — "gate
invocations and the shell probe acquire no slot" — and
[`crate::engine::topology::is_slotted`] is the total function of the
identity that decides it. Nothing here carries a second flag that could
disagree with the identity: the registration reads `is_slotted` and takes a
pair exactly when it answers `true`, so a shell probe that acquired a slot
would need `is_slotted` to be wrong about `ProbeTarget::Shell`.

### Why the runner is wrapped rather than the probes hand-built

An [`crate::agent::AgentAdapter`]'s `probe` runs **one or more** processes —
Codex runs ten: the version, two help texts, six strict-config checks and one
catalog read — and each is already a `RunnerRequest` carrying its own
`InvocationId` from [`crate::agent::probe_request`]. R4 is "invocation
registration (**all** Runner processes incl. gates, agent probes, and the
shell probe)", so registering only the identities this module could name in
advance would leave nine of Codex's ten unaccounted and the ledger would
balance while being wrong.

So the ledger sits **at the boundary**: [`Registering`] is a
[`crate::runner::Runner`] that registers, slots, runs and settles every
request that passes through it, whoever built it. That is also what makes
`R3`'s "released on complete or cancel" and `R4`'s "completed or cancelled
exactly once" true of a probe that *fails*, which is the case
`resume_refuses_by_preflight_probe_when_shell_or_cli_fails_before_any_recovery_event`
exercises.

### `certify` takes `&self`

The trait's signature is `fn certify(&self, policy: &RunnerPolicy)`, and an
implementation that allocates identities has to mutate a ledger behind it.
[`std::sync::Mutex`] rather than a `RefCell`: [`crate::runner::Runner`] is
`Send + Sync`, and [`Registering`] is a `Runner`.

## `pub struct Probed {`

What one pre-flight established.

The agents are carried in the order they were probed, because
`run_started(4).probed_agents` and `run_resumed(4).probed_agents` are lists
and a resume's list is compared against the record's.

## `pub struct Probed` › `pub agents: Vec<String>,`

Every agent whose CLI answered, in probe order.

## `pub struct Probed` › `pub caps: Vec<(String, Caps)>,`

What each agent's CLI reported about itself.

## `impl Probed` › `pub fn agents(&self) -> &[String] {`

The agent names, which is what the durable record carries.

## `const CLAUDE_CODE_OAUTH_TOKEN: &str = "CLAUDE_CODE_OAUTH_TOKEN";`

The variable through which Claude Code takes a long-lived OAuth token. When
it is set, Claude Code authenticates with it instead of the credentials in
its configuration directory, which is what a profile is. The host runner
passes the coordinator's environment to the child (it reserves `PATH`,
`HOME`, `USERPROFILE` and the credential-location keys, not this one), so a
token in the coordinator's environment reaches every profiled process.

## `const SIGN_IN_CHECKED: &[&str] = &[crate::agent::claude::ADAPTER_ID];`

The agents whose sign-in pre-flight checks: Claude Code
(`crate::agent::claude::ADAPTER_ID`) alone. For any other agent
[`refuse_signed_out`] returns at once, so pre-flight never calls its adapter's
`discover` and runs no discovery process for it
(`tests::certifies_codex_without_asking_its_sign_in_on_a_resume_and_a_fresh_run`).

Claude Code's discovery, `claude --setting-sources "" auth status --json`, sees
the routes its workers authenticate through. It loads no user, project or local
settings, as a worker loads none (`agent/claude.md`, `fn discover`); with no
login and `ANTHROPIC_API_KEY` set, `auth status --json` answers
`loggedIn: true`, `authMethod: api_key` (measured, on the bare
`auth status --json`, when `PR386-PROFILE-AUTH-NOT-PREFLIGHTED` was resolved);
and `CLAUDE_CODE_OAUTH_TOKEN` is the profile checks' concern
([`refuse_unusable_profiles`], check 5). So when it answers signed out, the run
would meet that account signed out at its first attempt. Until round three of
#408's review, the discovery was the bare `auth status --json`, which loads the
settings files, so a credential only a settings file supplied answered signed
in for an account every worker would meet signed out.

Codex's does not. `codex login status` reports the credentials Codex has
stored, and nothing else: with no stored login it prints `Not logged in`, which
the adapter's discovery reads as `NotAuthenticated` (`agent/codex.md`,
`discover`). Codex authenticates through routes that command cannot see: an
API key in its environment, and a model provider's configurable `env_key`,
which names whatever variable the user's configuration chose. A Codex set up
that way answers its discovery signed out and works, so a `NotAuthenticated`
answer is not evidence that the run would fail, and no fixed list of variables
read beside it could be complete. Codex's sign-in is therefore not checked at
all. A Codex that really is signed out fails its run's first attempt, as every
agent's signed-out account did before this check existed. That gap is recorded
as `PR408-CODEX-SIGN-IN-NOT-PREFLIGHTED`.

Until this list, every agent's discovery ran, and a Codex `NotAuthenticated`
answer was excused only while one of `CODEX_API_KEY`, `OPENAI_API_KEY` or
`CODEX_ACCESS_TOKEN` was set in the coordinator's environment (a table,
`ENVIRONMENT_CREDENTIALS`, that the sign-in check read through each path's
`environment`). A Codex that authenticated through a configured `env_key`, or
any route the table did not name, was refused though it worked.

## `pub(super) const MAX_ACCOUNTS_PER_AGENT: usize = 16;`

The most distinct accounts [`Account::of_agent`] lists for one agent, the
default account counting as one. A pools file that gives an agent one more is
refused ([`too_many_accounts`]) before any of that agent's discoveries runs.

The cap bounds what pre-flight does for an agent: at most this many
discoveries, each a CLI process run in turn before the run starts, and a
listing that compares each pool against at most this many accounts, so it is
linear in the number of pools however many there are. Many pools naming the
same account are one account and one discovery
(`tests::discovers_once_for_a_thousand_pools_naming_one_profile`).

## `struct ProfiledPool {`

A pool that names a `profile`: its name and agent, which the refusals cite,
and the profile as a path. As in `profiled.rs`, the configured string is
used as written, with no `~/` expansion, and a relative one is refused rather
than resolved ([`refuse_unusable_profiles`], check 3).

Private: [`refuse_unusable_profiles`] is its one reader. The sign-in check
lists accounts rather than profiled pools ([`Account::of_agent`]). Until it
did, this was `pub(super)` with an `of_agent` of its own, because the fresh
run's `RunnerProbes` (`create.rs`) handed an agent's profiled pools to
[`refuse_signed_out`].

## `impl ProfiledPool` › `fn of(pool: &Pool) -> Option<Self> {`

`None` for a pool that names no profile. Every pool that names one counts,
not only an agent's bound pool (`capacity::pool_for`): a profile in the
pools file that this run could not honour is refused wherever it is
written, rather than passed over because a pool earlier in the file binds
the agent.

## `pub(super) enum Account {`

An account an agent's runs use: its default account (`Default`), which is
whatever its CLI finds with no profile set, or a credential profile directory
(`Profile`). Each carries the name of the pool that names it, which the
refusal cites, and `Default` carries none when the agent has no pool in the
run. An enum rather than a pool with an optional profile, so a profile with no
pool to name it cannot be written.

## `impl Account` › `pub(super) fn of_agent(pools: &[Pool], agent: &str) -> Result<Vec<Self>, UpstrokeError> {`

The accounts `agent`'s pools use, in pools-file order: the accounts
[`refuse_signed_out`] checks the agent's sign-in under.

- A pool with a `profile` stands for that profile directory, and a pool with
  none for the agent's default account. The default account is an account the
  run uses as much as a profile is: a worker planned on an unprofiled pool runs
  under it (`profiled.md`, the module section).
- Each account appears once, named by the first pool that uses it. Two
  unprofiled pools are one default account, and two pools naming the same
  directory are one profile (compared as paths, as configured). A second
  discovery of the same account could only repeat the first's answer.
- An agent with no pool in the run uses its default account alone, named by no
  pool.
- At most [`MAX_ACCOUNTS_PER_AGENT`] distinct accounts. A pool whose account
  is already listed adds nothing, however many are listed. A pool that would
  list one more distinct account refuses with [`too_many_accounts`], and the
  scan stops there: no pool after it is read. Each pool is compared against at
  most the accounts already listed, never more than the cap, so the work is
  linear in the number of pools.

So the list is never empty and never longer than the cap. Both paths build it
here, from the run's pools — `RunPreflight::certify` from the pools
`with_pools` gave it, `RunnerProbes::agent` from the pools
`RunnerProbes::with_pools` gave it — so the list cannot differ between them.
Both build it before [`refuse_signed_out`] and return its refusal with `?`, so
an agent over the cap is refused before any discovery, in the same words on
both paths (`tests::refuses_seventeen_profiles_with_no_discovery_and_certifies_sixteen`,
`tests::lists_sixteen_accounts_and_refuses_a_seventeenth_distinct_one`). It is
built for every agent, Claude Code or not, since it reads no CLI; in a run
that passed the profile checks, an agent other than Claude Code has its default
account alone, as a profile is refused for it ([`refuse_unusable_profiles`],
check 2).

Until it listed accounts, this was `ProfiledPool::of_agent`, the agent's
profiled pools alone. An agent with a profiled pool then had its default
account checked by no one, though an unprofiled pool of the same agent ran its
workers under it: on that code the run in
`tests::refuses_a_first_pools_signed_out_default_account_though_its_profiled_spare_is_signed_in`
certifies, its one discovery run under the spare's profile.

## `impl Account` › `fn profile(&self) -> Option<&Path> {`

The profile directory, or `None` for the default account: what a discovery
under this account carries, and what two accounts are told apart by.

## `pub(in crate::engine) fn coordinator_environment(key: &str) -> Option<OsString> {`

The coordinator's own environment, read when the profile checks run: the one
the host runner composes each child's environment from
(`HostEnvironment::from_process`). A function rather than a snapshot so that a
run with no profiled pool reads nothing, and so that tests can stand in for it
without changing the process environment, which every test in the binary
shares.

Two readers hand it to [`refuse_unusable_profiles`]: a `RunPreflight`, as its
default `environment`, on a resume; and `RunOptions::profile_environment` on a
fresh run. That is why it is visible to `engine` and not only here: the fresh
path's reader is `RunOptions`', in `engine::options`. The sign-in check reads
no environment. Until it stopped checking Codex ([`SIGN_IN_CHECKED`]), it read
this one too, through the same `RunPreflight` field on a resume and through a
`RunnerProbes` field of its own on a fresh run, which is gone with it.

## `pub struct RunPreflight<'a> {`

The run's `RunnerPreflight`: a shell probe and one probe per recorded agent.

Holds no `&mut` anything: the [`RunnerPreflight`] trait's `certify` takes
`&self` because `rebuild_from_record` holds the runner and the record while
it calls it.

## `pub struct RunPreflight<'a>` › `pools: Vec<Pool>,`

The run's pools, in file order. Empty unless the caller supplied pools with
[`RunPreflight::with_pools`]. [`refuse_unusable_profiles`] picks out the ones
that name a profile, and when there are none, every profile check is skipped.
Each agent's sign-in discovery runs once under each account that agent's pools
use, or once under its default account when it has no pool
([`Account::of_agent`], [`refuse_signed_out`]).

## `pub struct RunPreflight<'a>` › `environment: fn(&str) -> Option<OsString>,`

Where the token check looks ([`refuse_unusable_profiles`], check 5):
[`coordinator_environment`] in production, and a fixture in tests
(`reading_environment`).

## `pub struct RunPreflight<'a>` › `ledger: Mutex<InvocationLedger>,`

Every invocation this pre-flight registered, in one process-local
ledger, with the slot table the agent probes take their pairs from inside
it. R3 and R4 balance at process end.

The `Mutex` is `certify`'s `&self`'s, below: the probes register through a
boundary that is a `Runner`, and a `Runner` is `Sync`. The probes run one at
a time, so it is never contended.

## `pub struct RunPreflight<'a>` › `probed: Mutex<Option<Probed>>,`

What the last `certify` established, for the caller that has to record
`probed_agents`.

## `impl<'a> RunPreflight<'a>` › `pub fn new(`

The pre-flight of a run whose recorded shell is `shell` and whose
recorded agents are `agents`.

`workspace` is where the shell probe runs. The agent probes take
[`crate::agent::probe_workspace`] through
[`crate::agent::probe_request`] — "a probe asks a CLI about itself and
has no workspace of its own" — and that decision is the adapter
module's, not this one's.

A pre-flight built with `new` alone carries no pools, so it makes no
profile check, and each agent's sign-in discovery runs once, under its
default account.

## `impl<'a> RunPreflight<'a>` › `pub fn with_pools(mut self, pools: &[Pool]) -> Self {`

The run's pools, so `certify` can refuse a profile the run cannot honour, and
check that each account the pools use is signed in, the default account
included.
`entry` passes today's configured pools on the resume path, the same slice
it hands `profiled`, so the pools the probes are stamped from are the pools
checked here. A fresh run builds no `RunPreflight`. Its `prepare_fresh`
calls [`refuse_unusable_profiles`] itself, before the lease, over the same
slice `FreshLeased::run` later hands `profiled`, and gives the same slice to
its `RunnerProbes` (`RunnerProbes::with_pools`), whose agent probe makes the
sign-in check. A builder rather than a fifth argument to `new`: a caller with
no pools, which includes every test fixture that predates profiles, keeps
the unprofiled pre-flight without having to say so.

## `impl<'a> RunPreflight<'a>` › `fn reading_environment(mut self, environment: fn(&str) -> Option<OsString>) -> Self {`

For the tests: the token check reads `environment` instead of the process
environment. A test that changed the real variable would race every other test
in the binary that reads it. It is private and no production path calls it.
It had a fresh-path partner, `RunnerProbes::reading_environment`
(`create.rs`), while the sign-in check read the environment; that went with the
check's environment read, and a fresh run's token check reads
`RunOptions::profile_environment`, as it did before. It is not `#[cfg(test)]`, because a `#[cfg(test)]` item inside the impl
would end the file's production region there
(`effects::tests::every_production_region_that_stops_early_stops_at_a_module`)
and hide the rest of the file from every census that reads that region.

## `impl<'a> RunPreflight<'a>` › `pub fn probed(&self) -> Option<Probed> {`

What the last successful [`RunnerPreflight::certify`] established.

## `impl<'a> RunPreflight<'a>` › `pub fn ledgers_balance(&self) -> bool {`

Whether every invocation this pre-flight registered was settled exactly
once, and no slot pair is still held or waited for.

`resource_accounting` R3/R4 at `NoRunFinished`: "released (process
death; empty at restart)" — but a *refusal* is not a process death, and
a pre-flight that refused while still holding a slot would leave the
resume's own later invocations refused a pair its own leak holds. This is
what `resume_preflight_probe_containers_reclaimed_after_refusal`
asserts alongside the containers.

## `impl<'a> RunPreflight<'a>` › `pub fn settlements(&self) -> (usize, usize) {`

How many probe invocations settled completed, and how many cancelled.

Two numbers because R3 has two rows. A refused spawn is a **cancel** —
the process either never started or produced no outcome this run may act
on — and a boundary that completed it would leave a balanced ledger
saying the opposite of what happened.

## `impl<'a> RunPreflight<'a>` › `pub fn running(&self) -> Vec<String> {`

Every identity still registered as running. Empty once `certify`
returns, on either path.

## `impl<'a> RunPreflight<'a>` › `fn registering(&self) -> Registering<'_, Mutex<InvocationLedger>> {`

The runner every probe of this pre-flight actually executes through:
the run's Runner, with the registration wrapped around it, on the
probe boundary ([`Slots::Probe`]) — the shell probe registers without a
pair, each agent probe with its own.

## `impl RunnerPreflight for RunPreflight<'_>` › `fn certify(&self, policy: &RunnerPolicy) -> Result<(), UpstrokeError> {`

The shell probe, then one probe per recorded agent, in that order.

**The order is the contract's**, and it is not alphabetical or
arbitrary: `recovery_order` (c) writes "the non-slotted shell probe
(recorded shell, exit 0) **and** one slotted probe per recorded agent",
and `runner` adds "probes execute through it **sequentially** at
pre-flight". A shell that cannot run a command is the cheaper refusal
and the one that explains every agent failure that would follow it, so
it goes first and the agent probes are never reached when it fails.

Before either probe come the profile checks
([`refuse_unusable_profiles`], over the pools `with_pools` gave it and its
`environment`), which spawn nothing. A profile the run cannot honour is
refused before the shell probe registers or runs anything, so that refusal
leaves an empty ledger behind it. With no profiled pool they are skipped. On
a resume this is recovery step (c): under the worktree lease and the run
lock, after the run exclusion and the include `resume_harness` made, and
before any probe or recovery event. A fresh run makes the same call in
`entry`'s `prepare_fresh`, before its lease, so a profile is refused in the
same words on both paths.

`policy` decides one thing here: whether a profiled pool can run at all,
which turns on its `kind`. Nothing else in it is consulted, and that is
deliberate: the runner this executes through was *built* from the record
— `rebuild_from_record` verified the runtime, the recorded image id and
the volumes before calling this — so re-reading the record to decide
anything else would be this module deciding a question the rebuild
already answered. It is named in every refusal, which is what a caller
needs it for.

The agent probes run through the runner `entry` built, which `profiled`
wraps, so a profiled pool's Claude Code CLI is probed under that pool's
profile (`tests::the_claude_code_probe_of_a_profiled_pool_runs_under_that_profile`).

After each agent's probe, and before its capabilities are kept, that agent's
accounts are listed ([`Account::of_agent`] of the pools `with_pools` gave it,
which refuses an agent over [`MAX_ACCOUNTS_PER_AGENT`] before any discovery),
and then, for Claude Code alone ([`SIGN_IN_CHECKED`]), its sign-in is checked
under those accounts in order until one fails ([`refuse_signed_out`], through
the same `registering` boundary). A fresh run makes the same two calls in
`RunnerProbes::agent`, so a signed-out account, and an agent over the cap, is
refused in the same words on both paths
(`tests::a_fresh_runs_probes_refuse_a_signed_out_pool_in_the_resumes_words`,
`tests::refuses_a_signed_out_default_account_on_a_fresh_run_in_the_resumes_words`,
`tests::refuses_seventeen_profiles_with_no_discovery_and_certifies_sixteen`).

### Errors

[`UpstrokeError::Refused`] naming the pool whose profile cannot be
honoured, the shell, the agent whose CLI did not answer, the agent whose pools
use more accounts than the cap ([`too_many_accounts`]'s refusal, as it
stands), or the agent and the account it is not signed in under
([`signed_out`]'s refusal, as it stands; a discovery that did not run to an
answer takes [`refused`]'s shape, like a failed probe). A profile
refusal precedes every registration. Every invocation registered before
any other refusal is settled —
completed, or cancelled with its slot pair released — so the ledger
balances on both paths, with one exception: a probe whose process the
Runner could not establish as ended (`ProcessFate::Unresolved`) is not
settled. Its registration stays running and keeps its pair until this
process exits (`InvocationLedger::end`, round R1 of the PR11 record), so
after that refusal `running()` names it and the ledger does not balance.

## `fn certify(&self, policy: &RunnerPolicy) -> Result<(), UpstrokeError> {` › `let shell_id = PreflightIdentities::shell(0)?;`

(1) The shell. Non-slotted, and `is_slotted` is what makes that true
rather than an argument here.

## `fn certify(&self, policy: &RunnerPolicy) -> Result<(), UpstrokeError> {` › `let mut caps = Vec::with_capacity(self.agents.len());`

(2) One probe per recorded agent, sequentially, in recorded order.

## `fn certify(&self, policy: &RunnerPolicy) -> Result<(), UpstrokeError> {` › `let accounts = Account::of_agent(&self.pools, agent)?;`

(3) Each agent's accounts, after its probe: [`Account::of_agent`] over the
pools `with_pools` gave it. Its cap refusal is returned as it stands, with `?`,
before any discovery runs: its wording is shared with the fresh path and true
here.

## `fn certify(&self, policy: &RunnerPolicy) -> Result<(), UpstrokeError> {` › `refuse_signed_out(agent, adapter, &probed, &registering, &accounts).map_err(`

(4) Each agent's sign-in, under those accounts: [`refuse_signed_out`] with the
caps the probe just returned. A [`SignIn::SignedOut`] refusal is returned as it
stands, because its wording is shared with the fresh path and its closing is
already true here. A [`SignIn::Undiscovered`] error is a discovery process that
did not run to an answer, the same kind of failure as a probe's, so it takes
[`refused`]'s shape, naming the agent's sign-in discovery.

## `pub(super) fn refuse_unusable_profiles(`

What a profiled run must hold before anything is spawned for it. This is the
one place the profile refusals are made, and both paths call it:

- **A resume:** `RunPreflight::certify`, first, at recovery step (c): under
  the resume's worktree lease and run lock, after its run exclusion and its
  include, and before any probe or recovery event. It passes the pools
  `with_pools` gave it and its `environment`.
- **A fresh run:** `entry`'s `prepare_fresh`, directly after the runner
  policy is resolved (and, for an explicit private root, authorized), before
  the worktree lease. A refusal there has published, locked and created
  nothing: no run exclusion, no include, no lease, nothing of the run;
  the children it started are the read-only ones of the checks before it
  (Git's, and a container selection's runtime inspection), and no agent CLI.
  It passes today's configured pools, the policy the authorization holds, and
  `RunOptions::profile_environment`, which is [`coordinator_environment`] in
  production.

So both paths refuse the same profiles in the same words. Only the pools that
name a profile count (`ProfiledPool::of`). With none, it returns at once,
reads no environment and leaves the run as it was before profiles existed.

Each check refuses through [`profile_refused`], naming the pool:

1. **The host runner.** A profile is a host directory handed to the child as
   `CLAUDE_CONFIG_DIR`. The container runner has no way to supply one: it
   refuses a request carrying a profile before starting it (`runner/
   container/exec.md`). Refusing here, from the policy's `kind`, keeps that
   from surfacing as a failed agent probe.
2. **Claude Code.** This stage supports Claude Code's profile only
   (`crate::agent::claude::ADAPTER_ID`). The host runner would place a
   profile under any agent's credential-location key (`CODEX_HOME`,
   `COPILOT_HOME`), so the limit is held here rather than by the runner.
   For any other agent the refusal names the pool and that agent.
3. **An absolute path.** A relative profile is refused, naming the pool and
   the value as written; it is never resolved or joined to a base. Each task
   runs in its own directory (the host runner starts every child in its
   request's `workspace`), and `profiled` stamps the string as written, so a
   relative path would name a different directory for each process, and a
   different one again from the coordinator's working directory, where check 4
   would read it. Pools are user-level, so no base directory is natural to
   resolve one against. `~` is not expanded (`profiled.md`), so
   `~/profiles/personal` is relative too, and the refusal says so.
4. **An existing directory.** `fs::metadata` follows a symlink, so a link
   to a directory passes. A missing path, or a dangling link, is refused as
   not existing. A path that exists and is not a directory is refused as
   such. Any other error, permission denied included, is refused with the
   error.
5. **No `CLAUDE_CODE_OAUTH_TOKEN`.** A token overrides the profile's
   credentials, so the run would not use the profile it names. A variable
   that is present counts as set, even with an empty value, and the refusal
   names the variable, never its value.

The checks are made in that order. The run-wide ones (1 and 5) name the
first profiled pool. 2, 3 and 4 are made pool by pool, in file order.

Every one of these refusals ends in [`profile_refused`]'s one closing
sentence, which is the same on a fresh run and a resume and true of both
(`engine::tests::topology_admission::a_fresh_schema_4_run_whose_pool_names_a_missing_profile_is_refused_before_anything_is_created`
and its relative-profile partner hold the two paths' texts equal, and with the
token test they hold that the fresh refusal left no `.upstroke`, an empty
private root and no worktree lease file).

Until the fresh call moved into `prepare_fresh`, it was `FreshLeased::run`'s,
right after the runner was profiled. That was before the startup census and
`create_run`, but after the worktree lease, the run exclusion and the
include's publication, so a refused fresh run left the lease file,
`<repo>/.upstroke` and the include behind, and the closing it ended in,
"Nothing was created, spawned or appended", was false of it; on a resume the
same sentence followed the resume's own exclusion and include. The closing
now says only what holds on both paths.

Until the relative check existed, `.` passed check 4 from the coordinator's
working directory and was then handed to every agent process, each of which
started somewhere else (`PR386-RELATIVE-PROFILE-CWD`;
`tests::a_relative_profile_refuses_naming_the_pool_and_the_value` began as the
witness of that). And until the profile refusals had their own closing, they
went through `refused`, whose last words are "so the run is resumable". On a
fresh run that was false: the refusal comes before `create_run`, so there is no
run to resume (`PR386-FRESH-REFUSAL-NOT-RESUMABLE`).

Until this function was shared, these were a method of `RunPreflight` and only
a resume made them. A fresh run whose pool named a missing directory, or whose
coordinator had `CLAUDE_CODE_OAUTH_TOKEN` set, was created and ran its
attempts to completion. Its Claude Code processes were handed a profile
directory that did not exist, or ran while the token overrode the profile's
credentials. `engine::tests`' missing-profile and token fresh-run tests
measured exactly that before the repair: each run completed.

## `fn profile_refused(what: &str) -> UpstrokeError {`

The refusal shape every profile refusal takes: `pre-flight: `, what is wrong,
and one closing sentence of its own — "No agent CLI was started and no event
was appended; correct the pools file or the environment and run the same
command again". Like [`refused`]'s, it ends without a full stop, since
`Display` text takes no trailing period (`standards/07`).

Not [`refused`], whose closing describes a probe: what was probed and inside
which boundary, and that the run is resumable. A profile refusal probes nothing,
and on a fresh run it precedes the lease and `create_run`, so there is no run to
resume. The sentence is true on both paths, and says nothing that holds on one
alone:
- a fresh run refuses in `entry`'s `prepare_fresh`, before the lease, with
  nothing published, locked or created; Git children ran, read-only, and no
  agent CLI;
- a resume refuses first in `certify`, under its locks and after its run
  exclusion and include, before any probe or recovery event.

So "nothing was created", which the closing said until the fresh call moved,
is not claimed: a resume has made its exclusion and include by then. It takes
no `policy`, so the wording cannot differ between the policy a fresh run was
authorized under and the one a resume rebuilt from the record.

## `fn refused(policy: &RunnerPolicy, what: &str) -> UpstrokeError {`

The refusal shape every failure of the shell and CLI probes takes. The
profile refusals, which precede the probes, take [`profile_refused`]'s.

Names the image the probe ran inside, because "a recorded shell or agent CLI
that fails **inside the recorded image**" is a different fact from the same
CLI failing on the host, and an operator who cannot tell which one has
nothing to fix.

## `pub(super) enum SignIn {`

Why [`refuse_signed_out`] stopped, in the two kinds its two callers word
differently.

## `pub(super) enum SignIn` › `SignedOut(UpstrokeError),`

An account the run would use reports itself signed out: [`signed_out`]'s
refusal, which both paths return as it stands.

## `pub(super) enum SignIn` › `Undiscovered(UpstrokeError),`

A discovery that did not run to an answer: the adapter's `discover` returned
an error, which for Claude Code is a process the boundary could not execute.
It is not a sign-in answer, so it is not passed as `Unknown`; each path words
it as it words a failed probe — a resume through [`refused`], a fresh run as
the adapter's error, as `RunnerProbes::agent` returns a probe's.

## `impl SignIn` › `pub(super) fn into_error(self) -> UpstrokeError {`

The error either kind carries, unworded: the fresh path's reading, where a
probe's error is returned as the adapter gave it.

## `pub(super) fn refuse_signed_out(`

The sign-in check, the one function both paths call: after `agent`'s probe,
its adapter's sign-in discovery (`AgentAdapter::discover`, with the caps that
probe returned), run through `through`, the agent probe's own registering
boundary, once for each of `accounts` ([`Account::of_agent`]: every account
the agent's pools use, the default account included, once each, in file
order), until one fails.

For an agent not in [`SIGN_IN_CHECKED`], that is every agent but Claude Code,
it returns `Ok` at once: no discovery runs and `accounts` is not read
(`tests::certifies_codex_without_asking_its_sign_in_on_a_resume_and_a_fresh_run`).

- **The first account, the default account**: through the boundary as it is.
  The first account is the default one only when the agent's first pool names
  no profile, or it has no pool at all, and then `profiled` stamps nothing on
  the agent's probes, so the discovery reaches the default account.
- **The first account, a profile**: through an [`UnderPool`] at further 0,
  which stamps that profile and keeps the adapter's own identities.
- **The `k`-th further account** (`k` = 1, 2, …): through an [`UnderPool`] at
  further `k`, which stamps its profile, or none for the default account, and
  moves each request to an identity of its own
  (`PreflightIdentities::further_pool`), because an adapter mints its
  discovery's identities with fixed ordinals and the ledger refuses a second
  registration of one identity (`InvocationLedger::open`).

The accounts are discovered in order, and the check stops at the first that
fails: one whose discovery answered `AuthState::NotAuthenticated`, refused with
[`signed_out`]'s refusal for that account; or one whose discovery did not run
to an answer, returned as that discovery's error. No later account is
discovered: the run is refused either way, so a later answer could not change
the outcome, and a later discovery through the same boundary as a failed one
would most likely fail the same way
(`tests::refuses_a_first_pools_signed_out_default_account_though_its_profiled_spare_is_signed_in`,
`tests::returns_a_first_discoverys_unresolved_error_and_discovers_no_second_account`).
`Authenticated` passes, and so does `Unknown`, and the check goes on to the
next account: a CLI whose `auth status` returns no JSON, or times out, has said
nothing about the account, and this check refuses only an account that reports
itself signed out
(`tests::a_discovery_that_cannot_say_whether_the_account_is_signed_in_certifies`).

**Why the first pool's discovery carries its profile too.** On both paths the
boundary is over `profiled`'s runner, which stamps a probe of the agent with
the profile of its first pool (`capacity::pool_for`), so when the first
account is a profile, the stamp here is the same profile. It is set here
anyway, so that what the discovery runs under is this function's choice and
the account the refusal names, not a consequence of `profiled`'s binding.
`profiled` passes a request that already carries a profile through unchanged
(`profiled.md`), so the stamp reaches the CLI.

The first account's discovery is the one that relies on `profiled`, and only
for the default account: it carries no profile, and `profiled` stamps nothing,
because the account comes first only when the agent's first pool names no
profile or the agent has no pool. A further account's discovery carries
exactly its own account, and `profiled` leaves it alone: it passes through
unchanged any agent probe whose ordinal is
`PreflightIdentities::FURTHER_POOL_STRIDE` or more, the block every further
account's identities are moved to (`profiled.md`, `Profiled::stamped`).
Without that, a later unprofiled pool's discovery, which carries no profile,
would be stamped with the agent's first pool's profile: it would run under
that profile, and the default account would go unchecked while its refusal,
or its pass, named it
(`tests::refuses_a_later_unprofiled_pools_signed_out_default_account_discovered_under_no_profile`).

**What it does not check.** An account no pool of this run uses: a profile in
another agent's pool, or the default account of an agent every one of whose
pools names a profile, since no request of this run reaches either. The
sign-in of any agent but Claude Code ([`SIGN_IN_CHECKED`];
`PR408-CODEX-SIGN-IN-NOT-PREFLIGHTED`). And any account after the first that
fails, which a run refused at that account never reaches.

Nor three things about Claude Code's own answer (`agent/claude.md`,
`fn discover`). That a credential the CLI reports as configured works:
`auth status` does not use it, so an expired or invalid key passes, and the run
meets it at its first attempt. Anything from a CLI that rejects
`--setting-sources` before `auth`: it answers with no JSON, which the adapter
reads as `Unknown`, and `Unknown` passes. And a worker given arguments the
discovery did not send: `build_args` appends a profile's `extra_args` after its
own, and nothing produces them today (`src/engine/assembly.rs:140`,
`src/review.rs:466`), but a producer for them would reopen the gap between
discovery's context and the worker's
(`tests::certifies_an_account_signed_in_with_no_settings_loaded_on_a_resume_and_a_fresh_run`
and `tests::refuses_an_account_signed_in_only_with_its_settings_loaded_on_a_resume_and_a_fresh_run`
hold the context that is checked).

Until it stopped at the first failing account, every account's discovery ran,
so each account's answer was registered and settled whichever of them was
signed out, and the refusal came after the last: a first pool's signed-out
default account was refused only once its spare's discovery had run too.

Until the default account was listed, an agent with a profiled pool had only
its profiled pools checked, and its default account was checked by no one,
even when another of its pools named no profile and ran its workers under it.
When that agent's first pool was profiled, no probe request could have reached
the default account, because `profiled` stamped the first pool's profile on
any agent probe that carried none; the further-account rule above is what
lets one reach it now
(`tests::refuses_a_first_pools_signed_out_default_account_though_its_profiled_spare_is_signed_in`
and `tests::refuses_a_later_unprofiled_pools_signed_out_default_account_discovered_under_no_profile`).

Until this check existed, nothing checked sign-in, so a profile that was not
signed in passed pre-flight and failed at its first attempt
(`PR386-PROFILE-AUTH-NOT-PREFLIGHTED`;
`tests::a_second_profiled_pool_that_is_signed_out_refuses_naming_that_pool_and_its_profile`
is the witness, which certified on the code before it).

### Errors

The first failure, in account order, and nothing after it:
[`SignIn::SignedOut`] when that account's discovery reported it signed out, and
[`SignIn::Undiscovered`], carrying the discovery's own error, when its
discovery returned an error. Either stops the check there.

Every process a discovery ran was registered through `through`, and it settles
each one that ended — completed, or cancelled with its slot pair released —
but not one whose process the Runner could not establish as ended
(`ProcessFate::Unresolved`). `InvocationLedger::end` keeps that registration
running, with its pair, until this process exits, so after its refusal the
ledger names that discovery as running and does not balance
(`tests::returns_a_first_discoverys_unresolved_error_and_discovers_no_second_account`).
Until this was corrected, this section said every discovery's process was
settled "on every outcome", which an unresolved one is not.

## `fn too_many_accounts(agent: &str) -> UpstrokeError {`

The cap's refusal, worded once for both paths: `pre-flight: `, the agent whose
pools use more than [`MAX_ACCOUNTS_PER_AGENT`] distinct accounts (credential
profiles, with the default account counting as one), that pre-flight checks the
sign-in of at most that many accounts per agent, then one closing sentence:
"Nothing was spawned for the run and no event was appended", and the same
command runs again once the agent's pools use at most that many. Like
[`refused`]'s, [`profile_refused`]'s and [`signed_out`]'s, it ends without a
full stop.

The closing is [`signed_out`]'s, for the same reason: it is true on both paths.
The shell and capability probes have run by then, and they were pre-flight's,
not the run's; a fresh run refuses at P4, before P5 opens the event log, and a
resume at recovery step (c), before any recovery event. [`refused`]'s closing
says the run was probed inside a boundary and is resumable, true of a resume
alone, and [`profile_refused`]'s says no agent CLI was started, false once the
probes ran.

## `fn signed_in(agent: &str, account: &Account, auth: AuthState) -> Result<(), SignIn> {`

One discovery's answer read as the check reads it: only `NotAuthenticated`
refuses. It reads no environment: an agent whose discovery cannot see how its
CLI authenticates is not checked at all ([`SIGN_IN_CHECKED`]), rather than
excused by variables read beside its answer.

## `fn signed_out(agent: &str, account: &Account) -> UpstrokeError {`

The sign-in refusal, worded once for both paths: `pre-flight: `, the agent,
and the account — the pool and its profile directory; or, for a pool that names
no profile, the pool and that, as it names no credential profile, it runs
under the agent's default account; or, when no pool of the run names the
agent, its default account — then one closing sentence: "Nothing was spawned
for the run and no event was appended; run the same command again once that
account is signed in", with how to sign it in in parentheses (for Claude Code,
`claude auth login`, with `CLAUDE_CONFIG_DIR` set to the profile when there is
one; for the default account it is unset, and the hint names none). Like
[`refused`]'s and [`profile_refused`]'s, it ends without a full stop.

For Claude Code, one sentence comes between the discovery's report and the
closing: "The check loads no user, project or local settings, as a worker loads
none, so a credential that only a settings file supplies does not count." It is
true on both paths: a fresh run and a resume ask through the same adapter
`discover`, which sends `--setting-sources ""` before `auth status --json`, the
pair every worker is given (`agent/claude.md`, `fn settings_isolation`). It is
there for round three of #408's review: an operator whose settings file signs
the CLI in (an `env` that sets `ANTHROPIC_API_KEY`, or an `apiKeyHelper`) sees
their own `claude auth status` answer signed in, and without it would read the
refusal as wrong. The sentence says why that credential does not count: no
worker would load it. Other agents' wording is unchanged, as the sentence is
about Claude Code's discovery; and only Claude Code's sign-in is checked
([`SIGN_IN_CHECKED`])
(`tests::refuses_an_account_signed_in_only_with_its_settings_loaded_on_a_resume_and_a_fresh_run`).

While Codex's sign-in was checked, the hint for an agent with environment
credentials also named them ("or by setting one of `CODEX_API_KEY`,
`OPENAI_API_KEY` or `CODEX_ACCESS_TOKEN` in the environment upstroke runs
in"). Only Claude Code's sign-in is checked now ([`SIGN_IN_CHECKED`]), so that
clause is gone, with the helper that listed the variables.

The default-account wording was "its default account, as no pool of this run
names a credential profile for it" for every default account, true when it was
written, as only an agent with no profiled pool had its default account
checked. Now an unprofiled pool's default account is checked beside profiled
pools, so the refusal names the pool, and the no-pool wording says what is
true of it: no pool of this run names the agent.

Neither of their closings is reused, because each is true of one path only.
[`refused`] says the run is resumable, which is false on a fresh run, where
the refusal aborts creation at P4 and there is no run to resume.
[`profile_refused`] says no agent CLI was started, which is false here: the
probe and the discovery ran. What this one says holds on both: the processes
that ran were pre-flight's, not the run's; a fresh run refuses at P4, before P5
opens the event log; a resume refuses at recovery step (c), before any recovery
event; and on either the same command runs again once the account is signed
in.

## `struct UnderPool<'a> {`

One account's sign-in discovery's boundary: `boundary` (the agent probe's
registering one) with the account's `profile` stamped on every request when
it is a profile, and nothing stamped for the default account, and, for the
`further`-th further account, each request's identity moved to that account's
block. `further` is 0 for the agent's first account when it is a profile,
whose identities are the adapter's own; the first account, when it is the
default one, takes no `UnderPool` at all. Everything else of each request, and
the whole of each call, passes through unchanged.

`profile` is optional since the default account became one of the accounts
checked: a further account may be the default one, whose discovery must carry
no profile, and still needs its identities moved.

## `impl UnderPool<'_>` › `fn under(&self, request: &RunnerRequest) -> Result<RunnerRequest, UpstrokeError> {`

The request as the account's discovery sends it: the profile set when there
is one, and for a further account the invocation replaced by
`PreflightIdentities::further_pool` of the same agent, at the adapter's
ordinal. A request whose invocation is not an agent probe is refused rather
than given some other identity; `discover` builds agent probes only.

## `impl Runner for UnderPool<'_>` › `fn run<'a>(&'a self, request: &'a RunnerRequest, call: RunnerCall<'a>) -> RunFuture<'a> {`

Hands the rewritten request to the boundary, which registers it under its new
identity. The call is re-lent at the future's local lifetime from its own
parts — the same cancellation, leases and hooks — as `Profiled::run` does, and
for the same reason (`profiled.md`). A request [`UnderPool::under`] refuses is
never handed on: the error is `NeverStarted`, under the request's original
identity, and nothing is registered.

## `impl Runner for UnderPool<'_>` › `fn composed_common_dir(&self, request: &RunnerRequest) -> ComposedCommonDir {`

Delegated over the request `run` would hand the boundary, so the trait's
default (`Unknown`) never replaces the boundary's answer.

## `pub(super) enum Slots {`

Which pairs a registering boundary gives the requests that cross it.

**Three because the boundaries are three.** INV-23's asymmetry is a real
one — "one non-slotted shell probe (the recorded shell executing `exit 0`)
**and** one slotted probe per recorded agent" — so the shell probe's
boundary ([`super::create::ShellProbe`]) holds no pairs at all, and a
slotted invocation arriving on it is refused rather than quietly run
unslotted. The agent probes' boundary gives each request the pair
`{agent, none}`. And since PR11 the review passes of an attempt or an
integration verification cross a boundary of their own, whose pair carries
the reviewer's pool and whose requests present the standing of the pipeline
they belong to. The alternative was a copy of register/run/settle per case,
and this file's own header is that there is **one place**.

## `pub(super) enum Slots` › `None,`

The shell probe's boundary: no pairs.

## `pub(super) enum Slots` › `Probe,`

The pre-flight's: each agent probe takes `{agent, none}`. A probe runs before
admission, so it holds no pipeline entitlement and needs none
([`crate::engine::topology::select::Standing::preflight`]); the pre-flight
holds no pool table, and its probes run one at a time.

## `pub(super) enum Slots` › `Pipeline {`

A pipeline's review passes and re-asks: the reviewer's pool, and the
pipeline's standing read from the fold by the judge that built the boundary.

## `pub trait Registrar: Sync {`

Where an invocation of an attempt, a verification or a pre-flight is admitted
(`admit`: registered, and granted its `{agent, pool?}` pair when it is slotted)
and ended (`ended`: with how its Runner call ended, `InvocationEnd` — completed
or cancelled, which releases the pair, or kept with it when the Runner could not
establish that the process ended), and where
an attempt asks for and reports its snapshots (`snapshot_begin`,
`snapshot_end`; no-ops by default). One trait, so that the one registering
boundary below and the judge's own calls reach the broker through the same
path whether they run on the coordinator's thread or a pipeline's (PR11 phase
3, the working record's R-U). Two implementations: the synchronous one here,
and the coordinator's channel client (`coordinator.md`), whose `admit` waits
on its own thread for the grant the coordinator loop sends. `Sync` because a
registering boundary is a `Runner`, and a `Runner` is `Send + Sync`.

## `impl<L: BorrowMut<InvocationLedger> + Send> Registrar for Mutex<L> {`

The synchronous registrar: the ledger, behind the `Mutex` a `Sync` trait needs,
admitting through [`InvocationLedger::register_at_once`] — a pair not grantable
at once is withdrawn inside the ledger and refused, because the caller is the
coordinator running the invocation on its own thread and may not wait
(INV-18; R-O). Generic over the ledger's owner: the pre-flight and run creation
own theirs (`L = InvocationLedger`); the width-1 loop and a judge borrow the
run's for one attempt or one pass (`L = &mut InvocationLedger`). The lock guards
the register and settle calls only, never the Runner call between them.

The source keeps this lock's protocol beside the `impl` (standards §10, §6).

## `pub struct Carried {`

What a pipeline carries into every Runner call it makes: its `Cancellation`, so
the coordinator can end the pipeline's processes from its own thread, and the
run's cleanup-lease paths, so the Unix reaper of each of them holds R28's lease
though the pipeline thread never entered the run's scope (R-Z). The default
carries a cancellation nothing fires and no paths, which is exactly the call
`run_blocking` makes, so the synchronous substrate is unchanged by it.

## `impl Carried` › `fn rebuild<'a>(&'a self, call: RunnerCall<'a>) -> RunnerCall<'a> {`

A caller's call with this cancellation and these leases in place of its own,
and its observers kept. A review pass builds its calls itself (`run_review`
calls `run_blocking`), so the boundary around it is the one place a pipeline's
context can be put into them.

## `pub(super) struct Registering<'a, R: ?Sized> {`

The run's Runner with R3 and R4 wrapped around every request.

Its own refusals — a registration the ledger refuses, a slotted request
on a boundary with no pairs or with no agent, a pair not grantable at once —
are `RunnerError`s with the fate `NeverStarted`, because they precede the
inner Runner; a settlement failure after the inner run carries the inner
outcome's fate (`Gone` for an output, the inner error's for an error), so
the boundary never claims more about the process than the Runner it wraps.

One place, so that "each a registered invocation" is true of a process an
adapter or a review pass built as much as of one this module built.

**`pub(super)` since 2026-08-27, because "one place" was not true.** Fresh
creation did not use it: `create.rs`'s P4 registered a single
`probe(agent, 0)` identity around the *whole* adapter call and handed the
adapter the raw Runner, so an adapter that runs ten processes — a current
Codex probe runs version, two help probes, six strict-config probes and the
model catalog — put one of them in the ledger. A failure at ordinal 1 was
recorded as ordinal 0 cancelled: the ledger named the process that
*succeeded* and held no record of the one that failed. The `bf927f3` review
found it as its third P1; the doc above was already the argument against it.

**Generic over the registrar since PR11 phase 3** (it was generic over the
ledger's owner in phase 2): the pre-flight and run creation hand it their
`Mutex`'d ledger, a judge its own registrar — the synchronous one at width 1,
the coordinator's channel client in a spawned pipeline. A boundary built
`carrying` a [`Carried`] rebuilds every call with it; one built without forwards
the caller's call unchanged.

## `pub(super) struct Registering<'a, R: ?Sized>` › `completed: AtomicU32,`

How many of this boundary's requests completed. A judge compares it with the
number of processes a review pass reports, which is how a pass that ran a
process outside the boundary is caught.

## `pub(super) struct Registering<'a, R: ?Sized>` › `composed: Mutex<Vec<ComposedCommonDir>>,`

What the inner Runner composes for the `GIT_COMMON_DIR` of each request this
boundary admitted, in admission order (`Runner::composed_common_dir`; the
external-prune follow-up's integration correction, F). A review pass's
processes run through the boundary a judge builds for that pass, and their
requests are built inside the review module. These reports are therefore the
only route by which closure 1's check after the pass sees the Git context of
the processes whose verdict it guards.

The lock's protocol, stated at the field as §10 places it: one push per
admitted request, made before the request is handed to the inner Runner,
with the lock held for that push alone and never across the inner run's
`.await`. One read by the owner after the last run has returned, by
consuming the boundary (`into_composed`). A poisoned lock keeps the reports
pushed before the panic. A request the boundary refuses at `admit` is never
handed on and leaves no report.

## `impl<'a, R: ?Sized> Registering<'a, R>` › `pub(super) fn into_composed(self) -> Vec<ComposedCommonDir> {`

The reports of the requests this boundary admitted, read once, after its
last run, as the boundary is consumed. A judge reads them for a review
pass's verdict (`attempt.rs`, placement 2). An empty list is a pass that
handed nothing on, for which no runner has anything to report, and the
judge checks it under the manager's own builder instead.

## `impl<R: Registrar + ?Sized> Runner for Registering<'_, R>` › `fn composed_common_dir(&self, request: &RunnerRequest) -> ComposedCommonDir {`

Forwarded to the inner Runner, so a boundary reports what the Runner it
wraps composes and never a value of its own.

## `impl<R: Registrar + ?Sized> Runner for Registering<'_, R>` › `fn run<'a>(&'a self, request: &'a RunnerRequest, call: RunnerCall<'a>) -> RunFuture<'a> {`

Register (with the pair when slotted), run, settle — and settle on the
failure path too. Since PR11 the inner Runner's `run` is a future, so the
wrapper is one too: `admit` (register, and take the pair) and `settle`
(complete or cancel, which releases the pair) are synchronous and run on
either side of the one `.await`, so no ledger guard is ever held across it —
which is also what keeps the future `Send`. A pipeline's `admit` blocks its own
thread for the coordinator's grant; that is the blocking thread R-I drives every
Runner future on, never the coordinator. The call is forwarded to the inner
Runner unchanged, or rebuilt with the carried context when the boundary carries
one, so the caller's observers always reach the process.

The settlement is not in a `Drop`: a `Drop` that swallowed a refusal
would make a ledger that did not balance look like one that did, and
`permits.protocol` asks for "registered/completed/cancelled **exactly
once**". A run that returns `Err` is a **cancel**, not a complete: the
process either never started or did not produce an outcome this run may
act on, and `R3`'s two lifecycle rows are "released on cancel" and
"released on complete or cancel" precisely so the two are told apart.

## `impl<R: Registrar + ?Sized> Registering<'_, R>` › `fn admit(&self, request: &RunnerRequest) -> Result<(), RunnerError> {`

Everything before the inner run: register, with the pair for a slotted
invocation, through the registrar — the synchronous one withdraws a pair not
grantable at once and refuses it, because its caller cannot wait (INV-18); a
pipeline's waits for the coordinator's grant. The refusals that precede the
registrar — a slotted request on a boundary with no pairs, or naming no agent —
are made before anything is registered, so every refusal here leaves the ledger
settled.

## `fn admit(&self, request: &RunnerRequest) -> Result<(), RunnerError>` › `Slots::None => {`

The non-slotted boundary cannot account a slotted invocation, and
running it anyway would put an agent probe through a path that
takes no pair — the process would execute with `permits.
agent_pool_slots` never consulted.

## `fn admit(&self, request: &RunnerRequest) -> Result<(), RunnerError>` › `let Some(agent) = request.agent.as_ref() else {`

The agent whose per-agent slot this is. A slotted invocation
without an agent binding cannot be accounted, and refusing is
the assertion rather than inventing a name for it.

## `impl<R: Registrar + ?Sized> Registering<'_, R>` › `fn settle(`

Everything after the inner run: end the registration exactly once with how the
inner run ended (`InvocationEnd::of`) — completed on an output, cancelled on an
error whose fate establishes the process gone, kept with its pair when the fate
is unresolved. A settlement failure carries the inner outcome's fate.
