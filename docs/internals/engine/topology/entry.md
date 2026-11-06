# `src/engine/topology/entry.rs`

Extended notes for [`src/engine/topology/entry.rs`](../../../../src/engine/topology/entry.rs).

The source defines behavior; these notes hold the module's contracts and rationale.
Each code span in a section heading is an exact source fragment. Search it as a fixed string
in the linked module, using the enclosing item to distinguish repeated lines.

## Module

The production entry of the schema-4 topology (PR12): what `upstroke run`
and `upstroke resume` reach once the topology is active.

It composes functions that already exist, in the spec's order
(`workspace_candidates.run_creation`, `sequential_substrate.startup_census`,
`recovery_order`, `selection`), and adds no effect site of its own. Each
public function is one step of a command whose other steps live in the
legacy entry point (`engine::coordinator::run_harness`,
`engine::resume::resume_harness`): those two bodies make the run exclusion
and publish the recorded-objects include between this module's steps,
because `Workspace::ensure_run_exclusions` and
`Workspace::recorded_objects_scope` are denied to every topology module
(`clippy.toml`).

Both are publications — between them they create directories, write a file,
stage and rename another and tighten a directory's mode — so both follow the
command's first effect, the physical worktree lease (`scope_and_constraints[19]`:
"the first effect of any write command is the worktree lock"; R17), and both
are made under the lease the rest of the command keeps: `run_harness` between
[`FreshPrepared::lease`] and [`FreshLeased::run`], and `resume_harness`
between the proof that the run's private records are its own
([`ResumePrepared::verify`], which takes both leases) and
[`ResumeVerified::run`], because on a resume the include is a write into a
private root that only those records authorize (INV-15, O18; PR12-CI-01).
`run_harness` makes the exclusion in the checkout's admission (L2), and
`resume_harness` on its own (L4).

Until repair10 (PR12-R9-PUBLIC-PUBLICATION-BEFORE-WORKTREE-LOCK,
PR12-REG7-PRELOCK-WRITES and PR12-N4-PRELOCK-PUBLICATION, three records of one
defect) both bodies made the exclusion before any lease: `run_harness` before
the include, and the include before `FreshPrepared::run` took the lease;
`resume_harness` before the proof. A command that then lost the lease, to a
live conductor or to a surviving reaper's cleanup hold, had already created
`.upstroke`, written `.upstroke/.gitignore` and, on a fresh run, created the
private root's `git` directory, staged and renamed the include into place and
tightened that directory's mode. The fresh exclusion entered that path in
`8cbd9983` (L2), the resumed one in `5316d668` (L4), and the fresh include's
publication before the lease with this module (`fffa73fa`). The legacy (schema
1–3) entries publish their include before their leases too; that order is the
activation base's, older than all three, and unchanged here.

### The fresh order

1. `run_harness` establishes containment.
2. [`prepare_fresh`], read-only: the plan, the config and the pools are
   captured and analysed under [`EngineLimits::Topology`]; when `[runner]`
   configures mounts, a `MountContext` is bound to the repository root, the
   checkout's Git directory and effective common directory as Git takes them
   ([`checkout_mount_context`]), the private root (explicit, else the
   default, which need not exist yet) and the coordinator's credential
   locations, read only; for the default root, a refusal if the path it
   will resolve to once created is not valid UTF-8
   ([`prelock::refuse_unspellable_default_root`]);
   the runner policy, its configured mounts included,
   is resolved by read-only inspection ([`prelock::check`] for an explicit
   private root, which must already exist, or [`prelock::resolve_runner`]
   for the default one, which may not); for the default root the run id and
   the incarnation are drawn next ([`DrawnIds`]), as [`prelock::check`]
   draws them for an explicit root; then the review plan, the probed
   agent set, the gate shells and programs (host runs only), the D4 width
   check, the budget ceiling, the slot limits and the configured notifiers.
   The runner comes before the review plan so that a refused container
   selection has asked the adapter source nothing.
3. [`FreshPrepared::lease`]: the checkout's Git directory, read, then the
   physical worktree lease (`Lock.CreateWorktreeLockFile`,
   `Lock.AcquireWorktree`), the command's first effect. A conductor holding
   it, or a surviving reaper's cleanup hold on any run of the checkout (R28,
   which the acquisition observes), refuses here, and the refused command has
   published nothing.
4. `run_harness`, under the lease, admits the checkout through the
   sequential entry's own rule, `engine::coordinator::refuse_dirty_tree` (L2;
   DESIGN §20 and §14's pre-flight, "clean working tree required"): the run
   exclusion, then the refusal of any staged, unstaged or untracked change.
   Nothing of the run exists yet, so a refused run has started no worker and
   created no run log. The sequential entry makes the same call under its
   worktree lock too, after its pre-flight. The admission is no read-only
   check: the exclusion it makes first is a write, and the dirty-tree test
   needs it (L2), so it follows the lease rather than joining the read-only
   half.
5. `run_harness` publishes the include under [`FreshLeased::private_root`],
   still under the lease. For the default root this is also how a missing
   root comes to exist, through the include's existing private-directory
   creation (root's Q4).
6. [`FreshLeased::run`]: the default root is authorized — canonicalized now
   that it exists, and refused unless it is a real directory, with the ids
   drawn in step 2; the inputs re-captured and compared with the checked
   capture (a difference refuses); the startup census, under the lease step 3
   took; the workspace manager; the execution root
   (`Worktree.CreateExecutionRoot`, which the inventory places before
   `run_started`, and removed again when creation ends with nothing retained);
   the case observation; `run_started(4)`; P0–P8; the handover to the loop;
   the loop; the report.

So the checks before any lock are the configuration's validation, the runner
policy's read-only inspection and the run id and incarnation for either root,
plus, for an explicit root, its authorization (an existing real directory,
canonical, spelled in UTF-8: [`prelock::check`]) and, for the default root,
the spelling of the path it will resolve to
([`prelock::refuse_unspellable_default_root`]). What follows the lease for
the default root alone is what cannot precede the root's existence: its
creation by the include's publication and its canonicalization and
real-directory check by `RunnerResolved::authorize`.

### The resume order

1. [`route_resume`], read-only and starting no process: the header of the
   committed first line selects the reader. An id `rundir::resolve_run_id`
   refuses — a retained husk's among them — refuses with the resolver's own
   refusal, first of all; a legacy log, or one whose header does not parse,
   returns `None` and `resume_harness` runs today's sequential lines
   unchanged; a log the router cannot read refuses with that read's own
   error; a schema above the ceiling refuses with the probe's own refusal.
   For a topology log it returns `RootDerived::derive` (a0).
2. `resume_harness` establishes containment.
3. [`prepare_resume`], read-only: the recorded private half must already be
   a directory (a resume never creates it); the frozen plan is read from
   the run directory and its digest checked against the record; today's
   config is read under [`EngineLimits::TopologyResume`] and compared with
   the recorded limits and the recorded gates; the D4 width check against
   the recorded width; the Q3 case observation; a recorded mount whose
   target a fresh run refuses ([`refuse_recorded_mount_targets`]), and one
   that reaches the checkout's Git directories, the repository root, the
   private root or a credential location
   ([`refuse_withheld_recorded_mounts`]), each refused before the runtime
   is asked anything; a recorded container image's environment; the slot
   limits; the recorded shell ([`recorded_shell`]); today's configured
   notifiers.
4. [`ResumePrepared::verify`], the proof, recovery step (a): the checkout's Git
   directory and repo key, this incarnation, the workspace manager (read-only,
   as before), then `recover::verify_for_recovery` — the worktree lease and the
   run lock, `owner.json` and `committed.json` verified against the committed
   first line, and the recorded mounts rechecked — and then the workspace
   manager bound to the execution root that first line records
   ([`hold_recorded_execution_root`]). Every refusal so far has written
   nothing, public or private: a resume that loses the worktree lease to a
   conductor or to a surviving reaper's cleanup hold, or whose manager derives
   another execution root, has made no exclusion and no include.
5. `resume_harness`, holding both leases, makes the run exclusion
   (`Workspace::ensure_run_exclusions`, L4), as the sequential resume does
   under its locks after its prerequisites, so a run whose
   `.upstroke/.gitignore` went missing is ignored by Git again. This step is
   the exclusion alone: the dirty-tree refusal is the fresh run's admission
   (L2), and no refusal is added to the resume.
6. `resume_harness` publishes the include under [`ResumeVerified::private_root`],
   the root the records proved, with both locks held.
7. [`ResumeVerified::run`]: the runner from the record, the `RunnerPreflight`
   over the recorded probe set, `recover::recover_verified` (steps (a1)–(h)),
   the loop, the report.

### The runner

A host policy runs on `HostRunner::new()` reading
`ObjectGraph::RecordedIn(repository)` (the adopted ENV option C, held
unconditionally) with `worker_in_checkout = false`; the repository-scoping
residual that shape leaves (Q1) is open. A container policy runs on one
`ContainerRunner` per incarnation, over the resolved (fresh) or recorded
(resume) policy, whose base environment is the image's own, read by
`recorded_image_environment` from the recorded image id in the read-only
half. The managed repository's recorded graph is stated, for a container
role, by the role's own Git view (`core.useReplaceRefs = false` in the view's
`config`, `runner::container::view`), and composition adds no process-wide
`GIT_NO_REPLACE_OBJECTS` (Q2's paired candidate; the real-container
acceptance is owed, and its limits are in `runner/container/env.md`).

Both runners take the repository's Git from what the coordinator holds. The
container runner is given the worktree Git directory and the common directory
of the same `ManagedRepository` the host runner reads, and its configured-mount
exclusions and its confinement's authoritative-Git entries come from those two
(`runner::container::resolve::RepositoryGit`). J10 (regression review 2,
finding 1): they came from the checkout's own `.git`, so with the common
directory relocated outside the checkout and selected by an inherited
`GIT_COMMON_DIR`, a configured writable mount of its `refs` was accepted and
reached a gate's launch plan.

### The probe set

Schema 4 probes the shell and every agent the run's chains and review plan
name, at P4, and fails the run's creation on any failure, before anything is
committed: a configured reviewer whose CLI is missing fails the run where the
sequential engine dropped an unprobeable alternative (root's D9, "the recorded
probe set is binding"). A resume probes the recorded set, through
`RunnerPreflight`, the same way. The recorded set is `run_started(4).shell` and
`run_started(4).probed_agents`: the shell is recorded on its own, whether or not
the run has gates, because a gate's recorded shell is no record at all for a run
with none (PR12-CI-03).

### Slot limits

`permits.agent_pool_slots`: "per-agent slots (max_per_agent, default
max_parallel) and per-pool slots keyed by pool name (max_per_pool, default
max_parallel); process-lifetime ephemeral scheduler state". Neither is
recorded (`TopologyLimits` holds `max_parallel`, `max_defers` and
`max_merge_repairs`), so both are the command's configuration: a fresh run
takes `[engine] max_per_agent`/`max_per_pool` as resolved (defaulting to the
configured width it records), and a resume takes the values written today,
defaulting to the **recorded** `max_parallel` (`resumed_slot_limits`).

## `pub fn prepare_fresh(`

The read-only half of a fresh schema-4 run. Every refusal it can raise
precedes every effect of the command: nothing is published, locked or
created when it returns an error.

Its first statement is R5's check of the seven repository-wide names
(`workspace_manager::refuse_relative_repository_wide_names`), before the
capture and the analysis start any Git child: a relative value names
something else, or nothing, in a Git child's directory, and P12-W8 measured
the public path's first Git child failing on one with Git's own "not a git
repository" before `prelock::check` could name the variable. `prelock::check`
keeps its own first-statement check.

The mount context is bound only when mounts are configured ([`mount_context`]),
so a run without them reads nothing it did not read before; it asks Git,
read-only, for the checkout's Git directory, reads `GIT_COMMON_DIR` and, without
it, the Git directory's `commondir` ([`checkout_mount_context`]), canonicalizes
paths and reads three environment variables and `HOME`, and writes nothing. It
is bound to the root the run will use: the explicit one, or the default one,
computed once and handed on to the pending authorization. Until J10
(regression review 2, finding 1) it read the checkout's `.git` instead
(`view::resolve`), which an inherited `GIT_COMMON_DIR` does not change, and a
configured mount inside a common directory relocated outside the checkout was
resolved and recorded.

The analysis's warnings are moved into the command's, not copied: nothing
reads them in the analysis afterwards (§6).

A default root is authorized only after the include's publication has created
it, and that publication follows the worktree lease, so the UTF-8 half of that
authorization is asked here first, read-only, against the path the root will
resolve to. The run id and the incarnation are drawn here for the default root
too, right after the runner's resolution ([`DrawnIds`]), so they precede the
lease whichever root the run uses; only the root's own canonicalization and
real-directory check wait for the root. Until J14 (regular review 3, finding 3;
regression review 2, finding 5) a home no string spells was refused only in
the effectful half (`FreshPrepared::run` then), after
`.upstroke/git/recorded-objects.gitconfig` and its directories had been
published under it, beside a refusal that said nothing was created.

## `pub struct FreshLeased {`

A [`FreshPrepared`] whose command holds the physical worktree lease and has
done nothing else: what `run_harness` makes the run exclusion and publishes
the include under. It owns the lease (`worktree`), the hooks its acquisition
was observed by, and the opened workspace and its Git directory, which the
effectful half reads. Dropping it — a refused admission, an include that
cannot be written — releases the lease.

## `struct DrawnIds {`

The run id, the incarnation and the pid of a fresh run over the default
root, drawn before the lease ([`prepare_fresh`]) and handed to
`RunnerResolved::authorize` once the root exists. `run_creation` lists their
generation ("no effect") among a fresh run's pre-lock steps, and
[`prelock::check`] draws them there for an explicit root; drawing them here
keeps that order for the default root, whose authorization can only follow the
root's creation under the lease. Its `IdSource` answers each with the value
drawn — the clones are the trait's owned returns of one draw, not second
draws — and a question id fresh from `RealIds`, which authorization never
asks for.

## `impl FreshPrepared` › `pub fn private_root(&self) -> &Path {`

Where the include is published: the authorized (canonical) explicit root,
or the default root as [`crate::rundir::default_private_root`] spells it.

## `impl FreshPrepared` › `pub fn lease(self, opts: &RunOptions) -> Result<FreshLeased, UpstrokeError> {`

The command's first effect, after every read-only refusal of
[`prepare_fresh`]: the checkout's Git directory, read, then
`WorktreeLock::acquire_in_hooked` — the lease file if it is missing (R25) and
this process's hold on it (R17) — which refuses a lease another conductor
holds, and one a surviving reaper's cleanup hold on any run of the checkout
(R28) keeps from it. A refusal here leaves the checkout, the private root and
every ref as they were.

## `impl FreshLeased` › `pub fn private_root(&self) -> &Path {`

[`FreshPrepared::private_root`], for `run_harness`'s include.

## `impl FreshLeased` › `pub fn run(`

The effectful half, under the lease [`FreshPrepared::lease`] took. The order
is the module section's; that lease is held from its acquisition through the
census and the whole run, and handed to the `RunHandle` by
`WorktreeLocked::into_lock` with no release in between. The default root's
authorization comes first, because the include `run_harness` published just
before it is what created the root
(`engine::tests::a_fresh_schema_4_run_over_the_default_root_that_loses_the_worktree_lease_creates_nothing_under_home`
holds a run over a default root that does not exist yet to both halves of
that: nothing under the home while the lease is held, the root created and
authorized once it is not).

**The execution root is not made here.** Everything fallible this body
prepares — the census, `WorkspaceManager::derive`, [`one_spelling`], the base
commit (`head_sha_full`, which an unborn branch refuses), the normalized plan,
the frozen inputs and [`fresh_record`] — runs before `create_run`, and none of
it creates R18, so an ordinary refusal there leaves nothing to account for: no
public run directory, no private half, no execution root. The root is
`create_run`'s, at P3c: published, after the marker that locates the run and
the owner record that authenticates it, with its location record in the
private half and its claim token inside it, and settled by the creator's own
cleanup before either half, or kept with both when it cannot be
(`engine::topology::create`'s module notes). The manager is handed to the
request as `root` (its `RootCreation` implementation is below), and a refusal
from `create_run` is its `Refused::into_error`, the disposition already
settled.

**The census reaches the operator (PR12-INV8-STARTUP-CENSUS-REPORT-DROPPED).**
The census's [`super::startup::StartupCensus::operator_lines`] — every run
directory it retained or could not reclaim, with locator and reason, and the
containers it reclaimed or left to another owner — are added to the run's
warnings, which the report renders, and every error after the census is
returned with them attached (`UpstrokeError::with_warnings`): the manager's
derive, [`one_spelling`], the base commit, the normalized plan, the record,
`create_run`'s refusal (a pre-flight probe that refuses is one) and the run
itself. A census that refused in its container half has no run-directory
report yet, and none is made up.

Until PR12-INV7-PREPUBLICATION-EXECUTION-ROOT-ORPHAN (first in `fffa73fa`)
this body made the root itself, before `head_sha_full`, the normalized plan
and the record, and pruned it only in `create_run`'s refusal arm: a failure
in between — an unborn branch, the clean repository with its plan and config
outside it, is one — returned with a root no run directory named, which no
census could find. Pruning on every error from here would not have closed it:
a removal that fails leaves the same root with no record naming it, so the
creation moved into the create protocol, after the records that locate it.

A container runner is given the worktree Git directory and `repository`'s
common directory (J10, see the module's runner section). The copies this body
makes are owned snapshots (§6): the include's spelling, kept for
[`one_spelling`] because a host run moves `repository` into its runner; the
run identity's strings and paths, which the runner owns; the frozen inputs'
plan, which [`fresh_record`] still reads from the analysis; and the request's
frozen inputs, which `create_run` consumes while the run keeps its own.

## `pub fn route_resume(opts: &ResumeOptions) -> Result<Option<RootDerived>, UpstrokeError> {`

Which engine resumes this run: `None` for a legacy log (or one whose header
the legacy path should diagnose), `RootDerived` for a topology log. It reads
files and starts no process, so the legacy path keeps its single
containment establishment.

**A log it cannot read is a refusal, never `None`.** `None` sends
`resume_harness` down the legacy lines, whose prelude publishes the
recorded-objects include under the command's selected or default root
(`Workspace::recorded_objects_scope`) before the legacy path reads the log,
takes a lease or proves anything. For a schema-4 run that is a private write
before the physical worktree lease and before INV-15's owner and commit proof,
under a root that need not be the one the run recorded, and it survives the
legacy path's later refusal. So the router's first read ([`read_initial_log`])
returns its failure as that read's own error, and the resume stops there with
nothing published, locked or written. Until
PR12-INV5-RESUME-READ-ERROR-LEGACY-PUBLICATION and
PR12-REG8-RESUME-READ-ERROR-LEGACY-PUBLICATION (one defect, first in
`fffa73fa`) the failure was `Ok(None)`.

**An id the resolver refuses is a refusal too, and the resolver's own.**
`rundir::resolve_run_id` answers a run id or an `UpstrokeError::Refused`: a
directory that never recorded a committed `run_started` (a husk), one whose
observation did not finish, a prefix it cannot resolve or that matches several
runs, and no match at all. Each is definitive about the id it was asked, and
the first step of the legacy lines after their prelude and their containment
(`resume_harness_inner_on`) resolves the same id and refuses with that same
value — by then the prelude has published the include. So folding the refusal
into `None` could admit nothing; it only published first. The router returns it,
and the resume stops before containment, any lease or any write, with the
resolver's sentence unchanged. Until PR12-REG9-HUSK-REFUSAL-LEGACY-PUBLICATION
(first in `fffa73fa`, whose activated lifecycle produces the retained P5b husk:
`create_run` publishes `committed.json` before its first append) the refusal
was `Ok(None)`, and a resume of a retained husk's exact id without
`--private-root`, or naming another root, published the include under today's
default or that root, whatever lease another conductor held. The legacy
prelude itself predates PR12 (base's resume published before resolving any id)
and is unchanged; only what reaches it changed.

One `None` is left besides a legacy schema, and it is not this repair's: a
header `probe_header` cannot read still goes to the legacy lines, since a
schema-less legacy header is one (`HeaderWithoutSchema`), and the legacy lines
diagnose it after their prelude has published the include. A resolved run's
header failing to probe needs the log to change between the census and this
read; that race is not separately reproduced here.

## `pub(super) fn read_initial_log(log: &Path) -> Result<Vec<u8>, UpstrokeError> {`

The first complete read of a run's event log: the bytes [`route_resume`] and
`observe::observe` select a reader from. It is `util::read_file_bounded` with
its failure mapped to `UpstrokeError::Io` carrying the log's path, as
`RootDerived::derive` maps its own read of the same file. Both callers reach
it after the census classified the run's committed header from its first
line, so its failure is neither "legacy" nor "absent" to either of them
([`route_resume`] above, and `observe`'s notes).

Before it reads, it asks `initial_log_read_fault::refuse`: the no-op below in
a production build, the test-only fault at the bottom of the file in a test
build.

## `mod initial_log_read_fault {`

The production half of the read's fault seam. Its `refuse` is `Ok(())`,
holds nothing and cannot be armed, so the call in [`read_initial_log`]
compiles away. The `#[cfg(test)]` half, the fault itself, is the module of
the same name at the **bottom** of this file, beside `mod tests`.

**Why the halves are split.** `effects::production_region` truncates a
source at its first `#[cfg(test)]`, whatever that attribute is on, and every
census that reads the region sees nothing below the cut. Until
PR12-FINAL17-INITIAL-READ-FAULT-CENSUS-CUT (first in `4d45a93b`) the read
called the fault through a `#[cfg(test)]` statement, with the test module
directly below it, so this file's region ended inside [`read_initial_log`]:
the read itself, [`prepare_resume`] and every item after them had left every
census that reads the region, and
`effects::tests::every_production_region_that_stops_early_stops_at_a_module`
failed with this file added to its set. Taking the attribute off the
statement alone would not have repaired it: the test module would then have
been the cut, and a module passes that test's shape check while still hiding
everything below it. `#[cfg(not(test))]` carries no cut, which is why this
half sits where it is read and the file's first `#[cfg(test)]` opens a tail
that holds test-only code alone. The class is
`PR5-R1-CFG-TEST-SHRINKS-THE-DOMAIN`; the twin shape is `workspace_manager`'s
`note_removal_attempt` and `engine::coordinator::legacy_append_hooks`'s.

## `pub fn prepare_resume(`

The read-only half of a topology resume. It begins with R5's check of the
repository-wide names, as `prepare_fresh` does, before the include's
publication starts a Git child.

A recorded container runner's mount targets are held to the rules a fresh
run applies ([`refuse_recorded_mount_targets`]), and its mounts are then
checked against today's exclusions ([`refuse_withheld_recorded_mounts`]),
both before the runtime is asked for the image's environment. The targets
come first because they need nothing but the record, while the exclusions
ask Git for the checkout's directories. The record was accepted when the
run began, against the
Git the checkout took then; under an inherited `GIT_COMMON_DIR` the same source
can now lie inside the common directory Git takes, and the resume refuses
before anything is published, locked or written, so the run stays resumable
once the environment is what it was.

`started` is an owned snapshot of the record `root` carries (§6): `root` is
consumed by the proof, and the run still reads the record's gates afterwards.

The pre-flight's shell is the record's ([`recorded_shell`]), never today's
`[engine] shell` and never a gate's. Until PR12-CI-03 it was
`started.gate_cmds.first()`'s shell, else today's `config.shell`: a run with no
gates recorded no shell anywhere, so its resume probed whatever today's config
named — refusing a run whose recorded shell still worked when today's named one
was missing, and continuing past a recorded shell that no longer ran when
today's did.

## `pub struct ResumeVerified {`

What a resume holds once its records are proved: the verified records — and
with them the worktree lease and the run lock, taken by
`recover::verify_for_recovery` and held until the run ends — the read-only
half's settlement, and the repository identity the proof was made against. It
is the only value whose `private_root` a resume publishes the include under,
and the only one with a `run`.

## `impl ResumePrepared` › `pub fn verify(self, opts: &ResumeOptions) -> Result<ResumeVerified, UpstrokeError> {`

The proof half of a resume, recovery step (a). The checkout's Git directory,
the repo key, the incarnation and the workspace manager are read before the
locks, as they were before the proof moved here; then
`recover::verify_for_recovery` takes the worktree lease and the run lock,
verifies `owner.json` and `committed.json` and rechecks the recorded mounts;
then [`hold_recorded_execution_root`] holds the manager to the execution root
the first line those records authenticated names. Its leases are the
command's first effects, and it publishes nothing: a refusal here — a lease
another conductor holds, a surviving reaper's cleanup hold, records that do
not prove the run, a manager that derives another execution root — leaves the
checkout, the private root, the log and the refs exactly as they were, which
is what lets `resume_harness` make the run exclusion (repair10) and publish
the include (PR12-CI-01) only after it. The leases' own files are created
once, on a run's first acquisition, and never rewritten, so a refusal after
them adds none.

## `impl ResumeVerified` › `pub fn private_root(&self) -> &Path {`

The root the verified records hang from: the one `RootDerived` derived from the
committed first line, now proved by the owner and commit records under it.

## `impl ResumeVerified` › `pub fn run(`

The effectful half of a resume. The runner is the recorded one: today's
`[runner]` reaches recovery only as the selection it is compared with. It is
built over `repository` only here, after the include was written, and drives
`recover::recover_verified` from the records the proof holds, so the recovery
order's steps (a1)–(h) follow the proof without taking the locks again.

A container runner is given the worktree Git directory and `repository`'s
common directory, as in [`FreshLeased::run`]. The include's spelling, the
run identity and the recovery seams' frozen inputs are owned snapshots (§6),
for the reasons given there. The probed capabilities default only where
nothing was probed, which cannot follow a recovery that returned: its step C
is `RunnerPreflight::certify`, which records what it probed before it returns
`Ok`, so the default discards no error (§7).

## `fn fresh_record(fresh: &FreshRecord<'_>) -> Result<RunStarted4, UpstrokeError> {`

`run_started(4)` before P0. The five fields `create_run` stamps from the
witnesses (`run_id`, `incarnation`, `runner`, `private_dir`,
`probed_agents`) are filled here with the same values, so that
`registry_digest` — computed over `registry_record()` and the probed agents —
is the digest of the record that is appended.

`path_policy.case_fold` is the sixth stamped field, and the one this function
cannot know: it is observed at the execution root, which `create_run` makes
at P3c, after this record exists. The `false` written here is never what is
committed — P5b stamps P3c's observation over it — and `registry_record()`
does not cover the path policy, so the digest is unaffected.

`shell` is `analysis.config.shell`, the shell `RunnerProbes` probes at P4, so
every resume probes the shell its creation probed, with gates or without
(PR12-CI-03). Every gate the analysis settled runs under that same shell, so for
a run with gates it equals each recorded gate's.

## `impl super::create::RootCreation for WorkspaceManager {`

The capability `create_run`'s P3c publishes R18 through. `key` is the
manager's repository key, which P3c writes in the location record. `create` is
`workspace_manager::FreshExecutionRoot`'s `create_execution_root`: the
`Worktree.CreateExecutionRoot` funnel building the root at the staging path in
the private half with the claim token and the scaffolding inside it, and
publishing it by a rename that never replaces anything, so a root already at
the path — empty scaffolding, an empty directory, a link — is refused and never
adopted, and its answer says whether the publication happened
(`FreshRootFailure::Claimed` or `Unclaimed`). `case_fold` is this module's
observation at the root just published: [`observe_case_at`] on its `tasks`
directory, resolved with the repository's `core.ignorecase`
([`resolve_case_fold`]), never at an ancestor. `path` and `retire` are the
manager's own (`impl UnpublishedRoot for WorkspaceManager`, in
`engine::topology::create`).

## `struct DeliveredFirst<'a> {`

The run's answer source: an answer already delivered to the run's answers
directory (by `upstroke answer`) is taken first, whatever the interaction
mode, and only then is the mode's own source asked. The legacy coordinator
does the same with its `sweep_answers`. Without it, `interaction = never`
(`UnattendedAnswers`) and a resume from a terminal (`TerminalAnswers`) never
read the answer file, so an answered question stayed open and the run parked
again (measured by P12-W6's resume, under `interaction = never`). The
delivered half is `EventLogAnswers` with no wait budget, so it reads and never
waits. `id` names the source that gave the last answer, so
`question_answered.via` says where an answer came from.

## `fn drive(`

The loop. A source that [`crate::agent::AdapterSource::shared`] can share
drives the run through `TopologyRun::run_concurrently` at any width; one
that cannot was refused above width 1 before any lock and is driven by
`TopologyRun::step` until `Progress::Finished`.

## `fn confirm_unchanged(`

Under the worktree lock, the inputs are captured again and compared, byte
for byte, with the capture every pre-lock check read. A difference refuses:
the runner, the review plan and the agents were resolved from that capture.

## `fn compare_recorded_limits(`

A run keeps the limits it started with. Today's `max_parallel` and
`max_merge_repairs` are compared with the record and a difference warns.
The slot limits are not recorded and are acted on (`resumed_slot_limits`).

## `ask_before` › the spend approval

Both halves hand the run `ask_before.frontier_escalation_over_usd`
(`TopologyRun::with_spend_approval`): a fresh run from its analysed config,
a resume from today's, as the sequential resume does. Without it the
schema-4 entry never asked before an escalation onto the frontier
(regression review #1, J1).

## `notify` › the configured notifiers

Both halves build the configured notifiers as the sequential preflight does
(`interaction::notifiers_for` over `[interaction] notify`): a fresh run from
its analysed config, a resume from today's, as the sequential resume does. A
choice the build cannot honour (an unavailable or unknown notifier) warns, and
the warning joins the command's warnings. Both `run` halves hand the notifiers
to the run (`TopologyRun::with_notifiers`), and the run's emitter announces
each question an append opens as soon as that append returns, whichever driver
runs it (`run.md`, `RunEmitter`). Questions already open when the command
starts are not announced again, as the sequential engine does not repeat them.

Until J11 (regression review 2, finding 2) the schema-4 entry built the answer
sources but never the notifiers. With `notify = ["cli"]`, unattended answers
and an independent task still working, the sequential engine printed the
question's id to stderr at once, and a schema-4 run printed nothing: the
operator learned of the question from the final report.

## `struct Driving {`

What `drive` runs the loop with. The runner is an `Arc` because the concurrent
driver hands the same runner to each pipeline it spawns on a thread of its own
(`PipelineSeams.runner`, whose sharing the coordinator states beside that
struct), and `Driving` keeps the coordinator's own handle.

## `fn compare_recorded_gates(`

A run keeps the gates it recorded, and the recorded gates are the ones the
resume runs (`ResumeVerified::run`). Today's gates are compared with them by
the sequential resume's own `preflight::gates_differ`, and a difference warns,
naming each edit: a command, a shell, a timeout, a gate only one side has
(regression review #4, J7; DESIGN §17). Today's gates are the ones a fresh run
would take: the config's `[[gates]]` under its shell, or, with none, the gates
derived from the repository's shape. A `[[gates]]` section today's config
cannot state is already announced as compared only
(`EngineLimits::TopologyResume`).

## `fn recorded_shell(`

The shell a resume's `RunnerPreflight` probes: the one `run_started(4)` recorded.
Today's `[engine] shell` that differs warns, naming both, and is ignored — as
INV-23 treats today's `[runner]` and [`compare_recorded_limits`] the recorded
ceilings — so changing today's config cannot change what the pre-flight probes,
in either direction. A recorded shell that no longer runs is the step-(c) shell
probe's refusal, before any recovery event, and the run stays resumable.

## `fn resumed_slot_limits(`

A resume's slot limits: each written value governs, and an absent one is
the recorded `max_parallel`, which is what the run's own
`PermitBroker::for_run` defaults to.

## `fn recorded_image_environment(`

The container runner's base environment, read-only, from the policy's
recorded image id: an absent image refuses (`InspectionRefusal::ImageIdAbsent`),
an unreachable or failing runtime refuses with its own diagnosis, and an
environment without a usable `PATH` refuses here, before any lock, by the
same `certify_path` the runner applies when it composes a role's
environment. Nothing is pulled and no other id is consulted.

## `fn container_runner(`

One `ContainerRunner` over the policy, the run's identity, the repository's
Git as the coordinator holds it and the image's environment, executing
through the production runtime. The runner owns its copy of the recorded
policy; the caller keeps the record (§6).

## `fn mount_context(`

The fresh run's mount context: unbound when `[runner]` configures no mounts,
so such a run asks Git nothing more, and otherwise [`checkout_mount_context`].

## `fn refuse_recorded_mount_targets(started: &RunStarted4) -> Result<(), UpstrokeError> {`

The resume's read-only check of a recorded runner's mount targets, by the
rules a fresh run applies (`resolve::refuse_recorded_targets`, the
functions `resolve_mounts` applies): each target in normal form, never `/`,
never at or under `/upstroke` or `/tmp`, spellable in the mount option, and
no two the same or nested. A target that breaks one is refused, naming the
run, the mount and the reason, with nothing published, locked or written:
before Git or the runtime is asked anything, so a record whose target a
fresh run refuses never reaches the image's environment, the runner's
construction or recovery (K1, regular review 4). Completeness, which the
record passed when it was read, checks its shape and not these rules.
Recovery and the rebuild hold the record to them again
(`resolve::recheck_recorded_mounts`), and the container runner at every
plan.

## `fn refuse_withheld_recorded_mounts(`

The resume's read-only check of a recorded runner's mounts, against the
exclusions a fresh run would bind today ([`checkout_mount_context`], over the
recorded private root). A mount that is, contains or lies inside one is
refused, naming the run, the mount and the reason
(`resolve::refuse_excluded`'s); a runner without mounts asks Git nothing. The
container runner checks every recorded mount again at each plan, against the
managed repository's own common directory.

## `fn checkout_mount_context(`

The `MountContext` of the checkout the command was started in, read-only and
before any lock: the Git directory is Git's own answer
(`Workspace::worktree_git_dir`, `rev-parse --absolute-git-dir`), and the common
directory follows Git's precedence ([`effective_common_dir`]).

It is not the `ManagedRepository`, which does not exist yet: the include's
publication builds it, after this half (J14's order, and root's Q4), and this
module may not call `recorded_objects_scope`. A `Workspace` function asking
Git for the common directory would be a new classified function of
`src/workspace.rs` (`effects/wrappers.toml`), so the precedence is applied
here instead, to the Git directory Git reports. It is the rule
`recorded_objects_scope`'s `rev-parse --git-common-dir`, run at the checkout's
top, answers by, and the container runner, given the managed repository's own
common directory, refuses at every plan whatever this half admitted.

## `fn effective_common_dir(`

The common directory Git takes for a checkout whose Git directory is
`git_dir`: an inherited `GIT_COMMON_DIR`, joined to the checkout's top, where
Git started from there resolves a relative one (an absolute value joins to
itself); else the Git directory's `commondir` record, relative to the Git
directory, without its line ending, as Git strips it; else the Git directory.
Only an actual not-found makes the record absent; any other failure to read it
is an `Io` error naming it, never the Git directory instead (§7).

## `fn one_spelling(include: &Path, manager: &WorkspaceManager) -> Result<(), UpstrokeError> {`

Root's D2 (b), one spelling source: the host runner's `ManagedRepository`
comes from the include's publication (`Workspace::recorded_objects_scope`,
outside this module), and every slot is registered and verified against
`WorkspaceManager::common_git_dir`. A role's `.git` file is spelled from the
second and the include's `includeIf.gitdir:` conditions from the first, so the
two are compared, after the manager is derived and before its first effect or
any role. A difference is returned as an error naming both: a defect signal,
not a refusal of the layout.

That signal fired on Windows for every schema-4 run and resume (CI run
41520547341): the include's spelling kept the verbatim prefix `\\?\` that the
manager strips. The repair is at the spelling's source, not here:
`recorded_objects_scope` now strips it with the manager's own `strip_verbatim`,
and this comparison is unchanged. `workspace`'s
`the_recorded_objects_scope_spells_the_common_directory_as_the_workspace_manager_does`
holds the two spellings equal over a real repository on every platform; it sits
there because this module's tests are denied `recorded_objects_scope`. Only a
Windows host produces the prefix, so only there does the test discriminate.

## `fn hold_recorded_execution_root(`

A resumed run's execution root is the one its committed `run_started(4)`
records (`decisions.workspace_candidates.execution_root`: recorded exactly,
per run), never the one today's environment would derive. Every slot,
intent, snapshot and staging directory the run owns hangs from that root, and
recovery and finalization act only on what their manager's root holds; a
manager over another root would recreate and inventory a root the run never
used, and finalize while the recorded one keeps its residue.

The two identities this resume holds are taken differently. The owner record
is checked against `RepoKey::for_worktree_git_dir`, the physical common
directory of the checkout's Git directory (`--absolute-git-dir`), while
`WorkspaceManager::derive` keys the root on Git's effective common directory
(`--git-common-dir`), which an inherited `GIT_COMMON_DIR` names, legally under
R5. So with `GIT_COMMON_DIR` naming another valid common directory, the owner
and commit proofs still pass and the manager names another root. Until
PR12-INV6-RESUME-COMMON-DIR-REBIND (first in `fffa73fa`) nothing compared the
two before recovery used the manager; [`one_spelling`] compares the include's
spelling with the manager's, both today's.

Called by [`ResumePrepared::verify`] after `verify_for_recovery`, so the
record compared is the one `committed.json` authenticated, and before any
publication, append, census, reclamation, finalization or root creation. The
comparison is `Path` equality of the recorded text with the derived path, and
reads nothing: both are the manager's own spelling (`fresh_record` records
`manager.execution_root()` exactly), `RootDerived` gives the private root the
run recorded, and `derive` canonicalizes Git's absolute answer before hashing
it. The checkout's own common directory therefore derives the recorded root
however `GIT_COMMON_DIR` spells it, absolute, relative or unset, and a root
that finalization pruned at that namespace compares equal without existing,
so recovery recreates it where it was (`recover`'s pruned-root step). A
difference is refused, naming the run, both roots, the canonical private root
and the effective common directory. Nothing is reconstructed —
`WorkspaceManager` has no constructor from a recorded root, and this repair
adds none — and no `GIT_COMMON_DIR` is refused as such: only a namespace
other than the recorded one is. A private root whose canonical form moved between epochs
differs the same way and is refused the same way.

## `fn hold_recorded_case_policy(`

The resume side of D7, at a0, read-only. The recorded `path_policy` is
execution identity: the fold uses it alone, it is never re-resolved, and
nothing here rewrites it.

What this does is re-observe the namespace the record was made for. Where
there is no execution root, because a finished run's root was pruned, the
lookup is of the private root's `workspaces` entry, whose tree the root is
created in. `case_policy_holds` then decides.

## `fn case_policy_holds(`

The decision, apart from its observation, so every combination is tested on
any filesystem. It applies root's Q3 and the plan addendum's three outcomes
(lines 129–132):
- a recorded `case_fold = false` with a root that now folds refuses before any
  lock or recovery, and the record is never rewritten;
- a recorded `case_fold = true` with a root that is case-sensitive continues
  on the record and warns, since it costs only parallelism;
- an observation that cannot answer warns, naming its reason, and the run
  continues on its record;
- a record the observation agrees with says nothing.

Before root's checkpoint-4 follow-through, the resume observed only behind a
recorded `false` and never warned. This box's filesystems cannot produce a
folding observation natively, so the refusal is witnessed with injected
observations
(`entry::tests::a_recorded_case_sensitive_policy_the_namespace_contradicts_refuses_and_nothing_else_does`).
The warning on a case-sensitive root is witnessed on a public resume as well
(P12-W11).

## `enum CaseObservation {`

What one lookup in the execution root says about case: the namespace folds,
it is case-sensitive, or the lookup could not say, with the reason it could
not. The reason is what the resolver's warning names.

## `enum IgnoreCase {`

Git's own record for the managed repository: `core.ignorecase` set true or
false, not set (Git's default, false), or not readable here, with the reason.

## `fn resolve_case_fold(`

D7's rule, as the plan states it: `case_fold = folds OR unknown OR
core.ignorecase == true`.
- An observation that could not answer folds, with a run warning naming the
  execution root and the reason. It is never silent, and it costs only
  parallelism: paths that differ only in case are leased as one.
- A known-sensitive namespace with `core.ignorecase` false or unset records
  `false`. Folding is never forced on it.
- `core.ignorecase = true` joins the union.
- A `core.ignorecase` record that could not be read folds and warns only where
  it decides the answer, over a known-sensitive namespace.

P12-W9 is the whole table
(`entry::tests::every_case_observation_and_ignorecase_record_resolves_to_the_planned_case_fold`).
Before PR12's brief 3 the fresh path took the observation alone: an unknown
answer folded without a word, and `core.ignorecase` was not read. The witness
found that, and this function is the repair.

## `fn recorded_ignorecase(manager: &WorkspaceManager) -> IgnoreCase {`

Git's effective `core.ignorecase` for the managed repository, as
`WorkspaceManager::core_ignorecase` reads it: `git config --type=bool --get
core.ignorecase` at the managed base, through the manager's builder. The
builder keeps `GIT_NO_REPLACE_OBJECTS`, the hooks path and the engine's
switches, and so the plan's no-replacement and object-selection rules.

Every scope Git reads answers, in Git's own precedence and with Git's own
boolean spellings:
- system and global files;
- the repository's file and the files it includes;
- the per-worktree file;
- `GIT_CONFIG_*` and `-c`.

A clean answer is `Set` or, when no scope sets the key, `Absent`. Anything
else is `Unreadable`, with Git's own words: a value Git does not read as a
boolean, a file Git cannot parse, or any message on the way.

Root's checkpoint-3 disposition (D7_CONFIG_REPAIR) replaced the handwritten
reader brief 3 had put here. That reader read `<common dir>/config` alone, so
a `true` set globally, by an include or in the environment read as `Absent` on
a case-sensitive filesystem. The manager's read, and its `effects/wrappers.toml`
row, are the one instrument change root authorized for it. The witness is
`workspace_manager::tests::core_ignorecase_is_gits_effective_answer_at_every_scope`:
- local, global, include and environment precedence;
- unset, true and false;
- a value that is not a boolean, and a file Git cannot parse;
- each with its control.

## `fn observe_case_at(directory: &Path, name: &str) -> CaseObservation {`

Whether `directory` resolves `name` and its ASCII upper-case spelling to the
same entry, read-only (`symlink_metadata`, and `canonicalize` off Unix).

When it cannot say, it says why:
- the name has no lowercase ASCII letter;
- the entry could not be read;
- the other spelling could not be looked up.

Its observation is of one directory. It does not establish how a
filesystem's descendants inherit case behaviour, nor anything about links or
reparse points (validation open).

## `pub(crate) mod initial_log_read_fault {`

Test-only: it fails [`read_initial_log`] for one log. The failure the guards
of that read need cannot be made through the filesystem: the census has
just read the same regular file's first line, and an ordinary file that read
once reads again, short of a race, a permission (the build box's user may be
privileged) or a process-wide exhaustion, none of them deterministic or
confined to one test. `arm(log)` makes every [`read_initial_log`] of that one
path on the arming thread fail with `INJECTED` until the returned `Armed`
drops, and `Armed::hits` counts the attempts, so a guard shows that the
failure struck the initial read and no other read. Thread-local, so tests
running in parallel never meet each other's fault; one log at a time per
thread. Nothing of it is compiled into a production build.

It is the file's first `#[cfg(test)]` and opens the test-only tail, so every
production item is above it. The production half beside
[`read_initial_log`] says why it is here.
