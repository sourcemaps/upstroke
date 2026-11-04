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
public function is one half of a command whose other half lives in the
legacy entry point (`engine::coordinator::run_harness`,
`engine::resume::resume_harness`): those two bodies publish the
recorded-objects include between this module's halves, because
`Workspace::recorded_objects_scope` is denied to every topology module
(`clippy.toml`) — `run_harness` between the read-only half and the effectful
half, and `resume_harness` between the proof that the run's private records
are its own ([`ResumePrepared::verify`]) and the effectful half
([`ResumeVerified::run`]), because on a resume the include is a write into a
private root that only those records authorize (INV-15, O18; PR12-CI-01). The
first effect of either command is
the run exclusion, which both bodies make before the include — `run_harness` just
before it, `resume_harness` just before the proof — for the same reason:
`Workspace::ensure_run_exclusions` is denied to every topology module too. `run_harness` makes it in the checkout's admission (L2), and
`resume_harness` on its own (L4).

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
   for the default one, which may not); then the review plan, the probed
   agent set, the gate shells and programs (host runs only), the D4 width
   check, the budget ceiling, the slot limits and the configured notifiers.
   The runner comes before the review plan so that a refused container
   selection has asked the adapter source nothing.
3. `run_harness` admits the checkout through the sequential entry's own rule,
   `engine::coordinator::refuse_dirty_tree` (L2; DESIGN §20 and §14's
   pre-flight, "clean working tree required"): the run exclusion, then the
   refusal of any staged, unstaged or untracked change. Nothing of the run
   exists yet, so a refused run has started no worker and created no run log.
   The sequential entry makes the same call later, under its worktree lock and
   after its pre-flight; here it is a refusal before the lock, as every check
   of the read-only half is.
4. `run_harness` publishes the include under [`FreshPrepared::private_root`].
   For the default root this is also how a missing root comes to exist,
   through the include's existing private-directory creation (root's Q4).
5. [`FreshPrepared::run`]: the default root is authorized; the worktree lock;
   the inputs re-captured and compared with the checked capture (a difference
   refuses); the startup census; the workspace manager; the execution root
   (`Worktree.CreateExecutionRoot`, which the inventory places before
   `run_started`, and removed again when creation ends with nothing retained);
   the case observation; `run_started(4)`; P0–P8; the handover to the loop;
   the loop; the report.

### The resume order

1. [`route_resume`], read-only and starting no process: the header of the
   committed first line selects the reader. A legacy log, or one whose
   header cannot be read, returns `None` and `resume_harness` runs today's
   sequential lines unchanged; a schema above the ceiling refuses with the
   probe's own refusal. For a topology log it returns `RootDerived::derive`
   (a0).
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
   limits; today's configured notifiers.
4. `resume_harness` makes the run exclusion (`Workspace::ensure_run_exclusions`,
   L4), as the sequential resume does after its prerequisites, so a run whose
   `.upstroke/.gitignore` went missing is ignored by Git again. This step is the
   exclusion alone: the dirty-tree refusal is the fresh run's admission (L2),
   and no refusal is added to the resume.
5. [`ResumePrepared::verify`], the proof, recovery step (a): the checkout's Git
   directory and repo key, this incarnation, the workspace manager (read-only,
   as before), then `recover::verify_for_recovery` — the worktree lease and the
   run lock, `owner.json` and `committed.json` verified against the committed
   first line, and the recorded mounts rechecked. Every refusal so far has
   written nothing private.
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
`RunnerPreflight`, the same way.

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
it, so the UTF-8 half of that authorization is asked here first, read-only,
against the path the root will resolve to. Until J14 (regular review 3,
finding 3; regression review 2, finding 5) a home no string spells was refused
only in [`FreshPrepared::run`], after `.upstroke/git/recorded-objects.gitconfig`
and its directories had been published under it, beside a refusal that said
nothing was created.

## `impl FreshPrepared` › `pub fn private_root(&self) -> &Path {`

Where the include is published: the authorized (canonical) explicit root,
or the default root as [`crate::rundir::default_private_root`] spells it.

## `impl FreshPrepared` › `pub fn run(`

The effectful half. The order is the module section's; the lock is held from
its acquisition through the census and the whole run, and handed to the
`RunHandle` by `WorktreeLocked::into_lock` with no release in between.

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
verifies `owner.json` and `committed.json` and rechecks the recorded mounts.
It writes nothing private: a refusal here leaves the private root, the log and
the refs exactly as they were, which is what lets `resume_harness` publish the
include only after it (PR12-CI-01).

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
common directory, as in [`FreshPrepared::run`]. The include's spelling, the
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
