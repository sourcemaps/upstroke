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
recorded-objects include between this module's read-only half and its
effectful half, because `Workspace::recorded_objects_scope` is denied to
every topology module (`clippy.toml`), and the include is the first effect of
either command.

### The fresh order

1. `run_harness` establishes containment.
2. [`prepare_fresh`], read-only: the plan, the config and the pools are
   captured and analysed under [`EngineLimits::Topology`]; the runner policy
   is resolved by read-only inspection ([`prelock::check`] for an explicit
   private root, which must already exist, or [`prelock::resolve_runner`]
   for the default one, which may not); then the review plan, the probed
   agent set, the gate shells and programs (host runs only), the D4 width
   check, the budget ceiling and the slot limits. The runner comes before the
   review plan so that a refused container selection has asked the adapter
   source nothing.
3. `run_harness` publishes the include under [`FreshPrepared::private_root`].
   For the default root this is also how a missing root comes to exist,
   through the include's existing private-directory creation (root's Q4).
4. [`FreshPrepared::run`]: the default root is authorized; the worktree lock;
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
   the recorded limits; the D4 width check against the recorded width; the
   Q3 case observation; a recorded container image's environment; the slot
   limits.
4. `resume_harness` publishes the include under the recorded root.
5. [`ResumePrepared::run`]: the runner from the record, the manager, the
   `RunnerPreflight` over the recorded probe set, `run_recovery_order`, the
   loop, the report.

### The runner

A host policy runs on `HostRunner::new()` reading
`ObjectGraph::RecordedIn(repository)` (the adopted ENV option C, held
unconditionally) with `worker_in_checkout = false`; the repository-scoping
residual that shape leaves (Q1) is open. A container policy runs on one
`ContainerRunner` per incarnation, over the resolved (fresh) or recorded
(resume) policy, whose base environment is the image's own, read by
`recorded_image_environment` from the recorded image id in the read-only
half; the container's unconditional `GIT_NO_REPLACE_OBJECTS` is unchanged
(Q2 open).

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

## `impl FreshPrepared` › `pub fn private_root(&self) -> &Path {`

Where the include is published: the authorized (canonical) explicit root,
or the default root as [`crate::rundir::default_private_root`] spells it.

## `impl FreshPrepared` › `pub fn run(`

The effectful half. The order is the module section's; the lock is held from
its acquisition through the census and the whole run, and handed to the
`RunHandle` by `WorktreeLocked::into_lock` with no release in between.

## `pub fn route_resume(opts: &ResumeOptions) -> Result<Option<RootDerived>, UpstrokeError> {`

Which engine resumes this run: `None` for a legacy log (or one whose header
the legacy path should diagnose), `RootDerived` for a topology log. It reads
files and starts no process, so the legacy path keeps its single
containment establishment.

## `pub fn prepare_resume(`

The read-only half of a topology resume.

## `impl ResumePrepared` › `pub fn run(`

The effectful half of a resume. The runner is the recorded one: today's
`[runner]` reaches recovery only as the selection it is compared with.

## `fn fresh_record(fresh: &FreshRecord<'_>) -> Result<RunStarted4, UpstrokeError> {`

`run_started(4)` before P0. The five fields `create_run` stamps from the
witnesses (`run_id`, `incarnation`, `runner`, `private_dir`,
`probed_agents`) are filled here with the same values, so that
`registry_digest` — computed over `registry_record()` and the probed agents —
is the digest of the record that is appended.

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

One `ContainerRunner` over the policy, the run's identity and the image's
environment, executing through the production runtime.

## `fn refuse_unsafe_case_policy(started: &RunStarted4) -> Result<(), UpstrokeError> {`

Root's Q3: a recorded case-sensitive policy (`case_fold = false`) that an
observation of the execution namespace contradicts refuses before effectful
recovery, and the record is never rewritten. A conservative record
(`case_fold = true`) is never refused, and an observation that cannot answer
is not evidence of sensitivity.

## `fn folds_case_at(directory: &Path, name: &str) -> Option<bool> {`

Whether `directory` resolves `name` and its ASCII upper-case spelling to the
same entry: `Some(true)` folds, `Some(false)` does not, `None` could not say.
Read-only (`symlink_metadata`, and `canonicalize` off Unix). Its observation
is of one directory; it does not establish how a filesystem's descendants
inherit case behaviour, nor anything about links or reparse points
(validation open).
