# `src/runner/container/resolve.rs`

Extended notes for [`src/runner/container/resolve.rs`](../../../../src/runner/container/resolve.rs).

[Source on GitHub](https://github.com/sourcemaps/upstroke/blob/master/src/runner/container/resolve.rs).

The code is the authority for what it does; this file is the whole of its prose, moved out of
the source verbatim. Each section is headed by the line of code the comment sat above, spelled
as it is in the source, so the heading is the grep string that finds the code.

## Module

The container [`RunnerPolicy`]: resolution by read-only inspection, and the
rebuild-from-record path (INV-23's container portion).

#### Where this sits

[`crate::runner::policy`] owns the *shape* — PR3's one `RunnerPolicy` wire
type, its canonical encoding and its digest — and PR4's `resolve_host()`
beside it. Its doc comment states the boundary this module is on the other
side of: *"the inspection that can fail is the container runner's, and that
is PR6"*. So there is still exactly one record type, one canonical encoding
and one digest; what is here is the two questions a container runner has to
ask a runtime and nobody else does.

#### Read-only, and structurally so

INV-23: the policy *"is resolved once by read-only inspection **before the
worktree lock** (before the public directory, the marker, and any probe; the
runtime must already hold the image and the volumes must exist)"*.

Every function here takes a [`ContainerRuntime`] and values, and nothing
else: no run directory, no lock, no [`crate::runner::Runner`], no path it
could write. Configured mounts (root's recorded-mounts disposition,
2026-11-02) add filesystem *observation* and nothing more: `fs::canonicalize`,
`fs::symlink_metadata` and `MetadataExt`'s device and inode. The repository's
Git directories are not read here at all: the caller hands them in
([`RepositoryGit`]), because only the caller knows which ones Git takes for
this checkout. Until J10 this module read them from the checkout's `.git`
through `view::resolve`. None of these is a governed
primitive on the clippy denylist; they are recorded here as read-only API
use, not as an effect exemption, and
`tests::the_resolution_module_names_no_lock_no_write_and_no_spawn` admits
exactly those three `fs::` spellings and no other. On Windows the identity
is read through `windows_sys`, also read-only: `CreateFileW` with
`FILE_READ_ATTRIBUTES` access, the `OPEN_EXISTING` disposition and
`FILE_FLAG_OPEN_REPARSE_POINT`, then `GetFileInformationByHandle` and
`GetFileInformationByHandleEx(FileIdInfo)`, the handle closed by
`OwnedHandle`. None of these is on the clippy denylist either; the same
census admits each call exactly once, requires those three flags, and
refuses every write, create, truncate and delete-on-close constant, so this
classification is recorded where it is enforced. `ContainerSite` — the frozen effect inventory's eight container
sites — has **no inspection variant** and this slice may not add one, so an
inspection is not a funnel call and a funnel call is not an inspection. That
is why `probe`, `image_by_reference`, `image_by_id` and `volume_present` are
called directly here while `create`/`start`/`stop`/`remove` cannot be.

#### The refusal split, which is the contract's own phrase

`expected_failures_refusals[1]` is **two sets of three**, at two phases,
with two different ordering predicates:

| phase | refusals | predicate |
|---|---|---|
| resolution | unreachable runtime · image **reference** absent (no implicit pull) · credential volume absent | before any lock or effect |
| rebuild | unavailable runtime · recorded image **id** absent · credential volume absent | before any spawn |

The two sets are **not** the same three questions. Resolution asks the
runtime to turn a reference an operator wrote into an immutable id; a rebuild
already has the id and asks whether the runtime still holds it. A seam that
could only ask about references could not express the second, which is why
[`RuntimeOp::InspectImageById`] exists.

And the fourth rebuild behaviour is on the other side of that split
entirely: *"a recorded shell or agent CLI that fails inside the recorded
image is observed by the RunnerPreflight probes and refuses before any
recovery event or work spawn"*. `non_goals[2]` is "non-spawn shell/CLI
presence inspection" — so this module cannot answer that question at all,
and does not try. [`RunnerPreflight`] is the seam it consumes;
`crate::runner::container::exec` and `crate::runner::host::run_shell_probe`
are what implement it.

## `#![cfg_attr(not(test), forbid(clippy::disallowed_methods, clippy::disallowed_types))]`

`PR6-LANEF-004`: the Container funnel's module-level allow is an inner
attribute and a lint level is scoped by the module tree, so every
out-of-line child of `runner::container` states its own level, enforced by
`runner::container::tests::every_child_module_of_the_container_funnel_states_its_own_lint_level`.
This file stated `deny` of all three until 2026-09-23, and #318 executed why
a `deny` is not enough: a `macro_rules!` emitting an allowed `std::fs::write`
wrapper at the bottom of the production region, called from the production
body of `prepared_pin_ref` in `engine::topology::integrate`, passed clippy
over all targets and the effects suite and wrote its bytes
(`~/orch-pr10/repair-318-r2-evidence/witnesses-prefix/PW2-resolve-*`). The
only allowances below this file are its whole-file `tests.rs`'s --
`disallowed_methods` and `disallowed_types`, for the real Git repositories
its fixtures build -- and that module exists in the lib test target alone, so
both lints are `forbid` in every build without `cfg(test)` and the fixtures
compile as they did; `disallowed_macros`, allowed by nothing below, is
`forbid` outright. The same patch is `E0453` at the generated attribute
(`controls-final/PX2-resolve-clippy.log`), and
`effects::tests::no_deny_of_a_governed_lint_is_excused_by_test_code_alone`
names this file the day the conditional `forbid` goes back to `deny`.

## `pub enum InspectionRefusal {`

---------------------------------------------------------------------------
Refusals
---------------------------------------------------------------------------

## `pub enum InspectionRefusal {`

Why a container runner could not be established by read-only inspection.

A typed enum and not a formatted string, because the six refusals
`expected_failures_refusals[1]` enumerates have to be told apart by a test
that is not reading prose: a suite asserting `is_err()` holds none of the
orderings and is green when the fixture path is a typo.

## `pub enum InspectionRefusal` › `RuntimeUnavailable {`

The runtime could not be reached for one of the inspections.

## `pub enum InspectionRefusal` › `RuntimeFailed {`

The runtime was reached and the inspection failed. A different answer
from unreachable, and kept apart for the reason
[`crate::runner::container::runtime`] gives: a daemon that answers `ps`
and fails `inspect` classifies as reachable under one global boolean and
carries the failure past the point the refusal exists to hold.

## `pub enum InspectionRefusal` › `ImageReferenceAbsent { reference: String },`

`non_goals[1]`: no implicit image pull. A reference the runtime does not
already hold is a refusal, never a fetch.

## `pub enum InspectionRefusal` › `ImageIdAbsent { id: String },`

The rebuild's question, and a different one: the recorded **id**.

## `pub enum InspectionRefusal` › `ImageNotIdentified { reference: String },`

The runtime answered, and its answer identifies nothing.

## `pub enum InspectionRefusal` › `CredentialVolumeAbsent { agent: String, volume: String },`

R20 is operator-owned and "never created or pruned by a run", so an
absent volume is a refusal rather than something to create.

## `pub enum InspectionRefusal` › `NotAContainerSelection { kind: RunnerKind },`

A guard, not a contract refusal: this module was handed a selection that
does not ask for a container.

## `pub enum InspectionRefusal` › `SelectionWithoutImage,`

A guard: a container selection with no image reference to look up.

## `pub enum InspectionRefusal` › `MountRefused { mount: String, reason: String },`

A configured mount resolution refuses, before any lock: the mount as the
operator wrote it and the first rule it breaks.

## `pub enum InspectionRefusal` › `MountSourceMoved { mount: String, reason: String },`

A recorded mount whose source is gone, rebound or another object: the
rebuild's refusal, before any spawn.

## `pub enum InspectionRefusal` › `RecordedTargetRefused { mount: String, reason: String },`

A recorded mount whose target a fresh run refuses: the mount as the record
spells it and the first target rule it breaks, in the words
`refuse_target` or the nesting rule gives a fresh run
(`refuse_recorded_targets`). A different decision from `MountSourceMoved`:
a moved source can come back and the run resume, while a target is the
record's own and never changes, so a run whose record carries one cannot
continue. K1 (regular review 4): until it, a record that completeness
accepted reached recovery and the launch plan with `/`, a reserved target
or `/cache/../upstroke/settings`, which Docker normalizes into the reserved
`/upstroke/settings`.

## `pub enum InspectionRefusal` › `RecordIncomplete(RunnerRecordDefect),`

The record this resolution produced is one PR3's fold would refuse.
Checked here for the reason `resolve_host` checks it: a run must not
start with a record its own resume would reject.

## `impl InspectionRefusal` › `pub const fn is_runtime_unavailable(&self) -> bool {`

Whether the runtime itself was the thing that could not answer.

## `pub enum RebuildRefusal {`

Why a recorded container runner could not be re-established.

**The variant is the refusal split.** `Inspection` is the three refusals that
happen with no spawn at all; `Preflight` is the one that requires a spawn
because "this is the only observable boundary for shell/CLI availability: no
non-spawn inspection is claimed". [`Self::before_any_spawn`] is that
distinction as a value, so a test can assert the predicate rather than the
prose.

## `pub enum RebuildRefusal` › `Inspection(#[from] InspectionRefusal),`

Refused by read-only inspection, **before any spawn**.

## `pub enum RebuildRefusal` › `Preflight(#[source] UpstrokeError),`

Refused by the RunnerPreflight probe spawns — the only observation of
shell and CLI availability inside the boundary.

## `impl RebuildRefusal` › `pub const fn before_any_spawn(&self) -> bool {`

Whether this refusal was reached without spawning anything.

## `pub trait RunnerPreflight {`

---------------------------------------------------------------------------
The RunnerPreflight seam
---------------------------------------------------------------------------

## `pub trait RunnerPreflight {`

The one observation of shell and agent-CLI availability inside the boundary.

INV-23: the RunnerPreflight is *"one non-slotted shell probe (the recorded
shell executing `exit 0`) and one slotted probe per recorded agent, each a
registered invocation through the run's Runner"*, and it *"is the only
observation of shell and CLI availability inside the boundary"*.

This is a **seam and not an implementation**, and deliberately so: the probes
are `crate::runner::host::run_shell_probe` over a
`crate::runner::container::exec::ContainerRunner`, and building that runner
needs a run identity, a private root and a slot pair that `TopologyRun` owns
at PR7. The rebuild path's job is to *consume* the observation and refuse
correctly, which it can do against a seam.

## `pub trait RunnerPreflight` › `fn certify(&self, policy: &RunnerPolicy) -> Result<(), UpstrokeError>;`

Certify the recorded shell and every recorded agent CLI inside the
recorded image, by spawning them through the rebuilt runner.

### Errors

[`UpstrokeError::Refused`] when a probe does not come back clean. The
caller classifies it; `rebuild_from_record` turns it into
[`RebuildRefusal::Preflight`], which is the arm that did spawn.

## `pub fn resolve_container(`

There is deliberately no `SkipPreflight` implementation here. A caller that
has not wired the probes calls [`rebuild_by_inspection`], whose name says
which three of the four rebuild behaviours it holds; a no-op `RunnerPreflight`
would let the same caller call [`rebuild_from_record`] and appear to have all
four.

## `pub const BOUNDARY_ROOT: &str = "/upstroke";`

---------------------------------------------------------------------------
Configured mounts
---------------------------------------------------------------------------

The container path under which the runner composes every role's own
mounts: workspace, Git view, objects, `.git` pointer, credentials and
settings (`BoundaryLayout`'s defaults; the resolve tests check each one
lies under it). A configured target may not be it, under it or above it;
`/tmp`, the role's scratch, is reserved the same way.

## `pub const DEFAULT_CREDENTIAL_HOMES: [&str; 4] = [".claude", ".claude.json", ".codex", ".copilot"];`

The agents' default host credential homes under `$HOME`. They are
withheld from configured mounts as paths only, with no content opened and
no directory scanned, together with whatever `CLAUDE_CONFIG_DIR`,
`COPILOT_HOME` and `CODEX_HOME` name in the coordinator's environment. A
per-agent credential volume stays the only credential channel into a
container.

## `pub enum MountExclusion {`

What a configured mount source may not be, contain or lie inside:

- `RepositoryRoot`: the canonical repository root, which also covers its
  public run directories and, for an ordinary checkout, its `.git`;
- `AuthoritativeGit`: the checkout's Git directory and the effective common
  directory, as the caller hands them in ([`RepositoryGit`]), so a linked
  worktree's common directory elsewhere, and one an inherited
  `GIT_COMMON_DIR` places outside the checkout, are withheld too;
- `PrivateRoot`: the authorized private root, which holds every run's
  private half, the execution roots and their worktrees, the container
  intents and the recorded-objects include. A run's worktrees are under
  it, so they are refused with it;
- `CredentialLocation`: see `DEFAULT_CREDENTIAL_HOMES`.

All in both directions. The generated own-role mounts keep their own,
one-directional `Confinement::violations` check in `exec.rs`: a worktree
legitimately lies inside the private root, and a configured source never
may, which `Confinement::configured_violations` holds there in both
directions too.

## `pub struct RepositoryGit<'a> {`

The two Git directories of the checkout a run integrates into, as the
caller resolved them: `git_dir`, where Git keeps this checkout's `HEAD`
and index, and `common_dir`, the effective common directory, where its
refs and objects are. This module never derives them. The schema-4 entry
builds the value twice, from two sources:

- before any lock (`entry::checkout_mount_context`): the Git directory
  Git reports and the common directory Git's precedence gives (an
  inherited `GIT_COMMON_DIR` as the checkout names it, else the Git
  directory's `commondir`, else the Git directory);
- for the container runner: the same Git directory and the common
  directory of the `ManagedRepository` the coordinator holds, the one the
  host runner reads as `ObjectGraph::RecordedIn`.

J10 (regression review 2, finding 1): `bind_with` built its
`AuthoritativeGit` exclusions from `view::resolve` over the checkout's own
`.git`, which an inherited `GIT_COMMON_DIR` does not change. With the
common directory relocated outside the checkout and selected that way,
a writable mount of its `refs` was accepted, and a gate's launch plan
received it. DESIGN §26 refuses a source that is, contains or lies
inside "the repository root, its Git or common directory", and the common
directory Git uses is the one that checkout resolved (§26, the runner
contracts). Both are now named by the caller, and nothing here reads
`.git`.

## `pub struct MountContext {`

The read-only context `resolve_container` lacked: the exclusion list
above, bound once before any lock. `unbound()` binds nothing and accepts
no configured mount; the entry binds a context only when mounts are
configured, so a run without mounts reads nothing it did not read before.
`bind` reads the coordinator's credential locations; `bind_with` takes
them explicitly, for tests. `excluded` lends the list and
`into_excluded` gives it up (the runner keeps it without a copy); both
are empty for an unbound context, which no caller asks.

### Errors

`bind` and `bind_with` fail when the repository root cannot be
canonicalized, and refuse when either Git directory they are given is not
an absolute path: a relative one would be resolved against whatever the
process's directory is, and the comparison would then withhold some other
directory while accepting a mount of the real one.

## `pub fn canonical_spelling(path: &Path) -> PathBuf {`

A path as the filesystem resolves it, for comparison: the longest
existing ancestor canonicalized, the rest appended as written. A private
root or credential location need not exist yet to be withheld.

## `pub fn canonical(path: &Path) -> io::Result<PathBuf> {`

`fs::canonicalize`, with one Windows rewrite: a verbatim drive path
(`\\?\C:\...`), which is what canonicalization returns there, loses its
`\\?\` prefix, so every canonical path this module compares or records is
the drive-letter spelling. The two name the same path; keeping one of them
everywhere is what makes `Path` comparisons (component by component, the
prefix included) and the recorded string exact. A verbatim path that is
not a drive path (a network share, a volume GUID) keeps its prefix and is
then refused as a mount source. Elsewhere this is `fs::canonicalize`.

## `pub fn overlap(source: &Path, excluded: &Path) -> Option<&'static str> {`

`is`, `contains` or `lies inside`, by component (`Path::starts_with`), so
`/srv/ab` is not inside `/srv/a`.

## `pub fn refuse_target(target: &str) -> Result<(), String> {`

A target is a container path on every host, so it is checked as a string
with `/` separators, never as a host `Path`: it must be spellable in the
runtime's mount option (see `container.rs`'s `mount_option_refusal`),
absolute, not `/`, in normal form (no empty, `.` or `..` component, no
trailing `/`; a non-normal spelling is refused, never rewritten), and not
at or under `/upstroke` or `/tmp`.

## `pub fn nested_targets(first: &str, second: &str) -> bool {`

Two targets that are the same path or where one holds the other. Two
such configured mounts would make what the container sees there depend
on mount order, so resolution, every use of a record
(`refuse_recorded_targets`) and the plan refuse them, the plan also
against the role's own targets.

## `pub fn refuse_recorded_targets(mounts: &[RecordedMount]) -> Result<(), (&RecordedMount, String)> {`

The target rules a fresh run applies, applied to a recorded mount list:
each target through `refuse_target`, then every pair through
`refuse_nested_targets`. These are the two functions `resolve_mounts`
applies, so a recorded policy is refused for every target a fresh
configuration is refused for, by the same rule and in the same words, and
there is no second copy of a rule to drift (K1, regular review 4). It
answers the first mount that breaks one and the reason; each caller labels
the refusal in its own terms.

Completeness (`RunnerPolicy::completeness` in `topology::events`) checks
the record's shape only: an absolute-looking target, a well-formed source,
targets strictly ordered. It accepted `/`, `/upstroke/settings` and
`/cache/../upstroke/settings`, which Docker normalizes into the reserved
`/upstroke/settings`, so the record is held to the target rules where it
is used, not where it is folded; the topology layer does not reach into
this module. The uses, each before its effect:

- the public resume, read-only, before Git or the runtime is asked
  anything (`engine::topology::entry`'s `refuse_recorded_mount_targets`);
- recovery, before any recovery effect, and every rebuild of the runner,
  before the runtime is asked (`recheck_recorded_mounts`);
- every container plan, before any intent, reaper or container
  (`exec`'s `configured_mounts`).

Pure: it reads the strings and nothing else, so it can run first at each.

## `fn refuse_nested_targets(mounts: &[RecordedMount]) -> Result<(), (&RecordedMount, String)> {`

The nesting rule over a mount list: every pair, in list order, and the
later mount of the first pair that `nested_targets` joins, with the
earlier one named in the reason. `resolve_mounts` applies it to the
resolved list sorted by target, so its refusal names what it named when
the loop was inline there; a recorded list is applied as recorded.

## `fn resolve_source(source: &Path) -> Result<(String, MountSourceIdentity), String> {`

The source as recorded: absolute (a relative one would depend on the
directory `upstroke` was started from), existing, canonicalized
(symlinks resolved at selection), on Windows a drive-letter path (a
network share or device path is refused), spellable exactly in the
runtime's mount option, and a directory or regular file with its
identity.

Canonicalizing does not make later replacement impossible, and nothing
here claims an atomic bind. The rebuild and every plan recheck the
recorded path and identity before use (`recheck_recorded_source`); the
window between that recheck and the runtime's own bind remains, as it does
for any path the runtime is handed.

## `fn source_identity(canonical: &Path) -> Result<MountSourceIdentity, String> {`

Unix: `symlink_metadata` of the canonical path, so a path that turned into
a link since it was resolved is refused rather than followed; directory or
regular file; device and inode from `MetadataExt`.

Windows (root's `PR12-MOUNT-WINDOWS-IDENTITY` follow-through, adapting
`rundir::scratch_tree`'s read-only directory-identity precedent, which is
left unchanged): one handle opened on the object itself with
`FILE_READ_ATTRIBUTES` access, `OPEN_EXISTING`, `FILE_FLAG_BACKUP_SEMANTICS`
(so a directory opens) and `FILE_FLAG_OPEN_REPARSE_POINT` (so a reparse
point is opened as itself and refused, never followed). Its attributes
refuse a reparse point (a link, a junction, or a path that became one
since it was resolved) and a device; the kind is directory or file.
`GetFileInformationByHandleEx(FileIdInfo)` then gives the volume serial
number and the full 128-bit file id, recorded as 32 lowercase hex digits,
never truncated. Unreadable attributes or id refuse. The handle is an
`OwnedHandle`, closed once on every path. std has no stable API for this,
and `fs::OpenOptions` is a denied type here, hence the direct calls, each
with its `SAFETY:` obligation in the source.

Other targets, neither Unix nor Windows, refuse configured mounts rather
than record a guessed identity.

## `pub fn described(identity: &MountSourceIdentity) -> String {`

An identity as a refusal names it: the kind, then device and inode, or
volume and file id.

## `pub fn resolve_mounts(`

The recorded list for today's configured mounts, or the first refusal.
It is called by `resolve_container` before the runtime is asked
anything, so a refused mount reaches no runtime operation, no lock and no
probe. Every configured mount is checked; the list is sorted by target
and refused if two targets are the same or nested
(`refuse_nested_targets`, which every use of a record applies too).

## `pub fn recheck_recorded_source(mount: &RecordedMount) -> Result<(), String> {`

Before every use of a recorded mount: the rebuild of a resumed run, and
each container plan. The recorded path must still exist, still
canonicalize to itself (a link or a move rebinds it otherwise) and still
name the recorded object (kind, device, object). A failure refuses: the
record is the boundary, and a different object at the same path is not
it.

## `pub fn recheck_recorded_mounts(record: &RunnerPolicy) -> Result<(), InspectionRefusal> {`

Every recorded mount of `record` has a target a fresh run accepts, and
still names the object the run recorded (DESIGN §26).

Read-only: the targets are checked first, as strings
(`refuse_recorded_targets`), then each source is resolved and identified;
nothing changes. So recovery asks it before any recovery effect, and every
rebuild of the runner asks it again.

### Errors

`InspectionRefusal::RecordedTargetRefused` for the first mount whose target
a fresh run refuses, before any source is looked at; otherwise
`InspectionRefusal::MountSourceMoved` for the first source that is gone,
rebound or another object.

## `pub fn resolve_container(`

---------------------------------------------------------------------------
Resolution
---------------------------------------------------------------------------

## `pub fn resolve_container(`

Resolve the container runner's policy by read-only inspection.

`decisions.pr_sequence[7].scope`, which is five obligations in one sentence:

> Container variant resolved by read-only inspection (runtime reachable;
> image reference already present in the runtime — no implicit pull — its
> immutable image id and manifest digest **when reported**, per-agent
> credential volume names present by volume inspection, policy
> `container-v1`)

The inspections happen in that order and the trace records them, so "the
runtime must already hold the image and the volumes must exist" is a
sequence and not a set. The first failure refuses: the answer to "may this
run start" does not get better by asking the remaining questions, and asking
them would put runtime calls after a refusal that is supposed to be the end
of the command.

**The recorded reference is the operator's, not the runtime's.** An
[`ImageInspection`] carries every reference the runtime says resolves to the
id — possibly none, possibly several, possibly one nobody wrote — and taking
the record's `reference` from there would make the record its own oracle and
"the recorded reference now names another image" unconstructible.

### Errors

The first [`InspectionRefusal`] the inspection produces.

## `let mounts = resolve_mounts(&selection.mounts, context)?;`

(0) configured mounts, which ask the runtime nothing.

## `runtime.probe().map_err(InspectionRefusal::from_runtime)?;`

(1) runtime reachable.

## `let inspection = runtime`

(2) the reference is already present — no implicit pull.

## `inspect_volumes(runtime, &selection.credential_volumes)?;`

(3) the credential volumes exist.

## `let policy = RunnerPolicy {`

(4) the record: policy `container-v1`, the operator's reference, the
runtime's id, its digest when it reported one, and the resolved mounts.

## `fn reported_digest(inspection: &ImageInspection) -> Option<String> {`

The manifest digest **when reported**.

INV-23 records `digest: Option<...>` and says "the manifest digest when
reported", and [`ImageInspection::digest`] says `None` when the runtime
reports none. A runtime that answers with an empty string has not reported a
digest — it has answered the question badly — and recording `Some("")` would
put a value in the record that names no manifest while still being a
*present* digest to every reader of the record.

So absent and present-but-empty collapse **here, at the inspection seam**,
and nowhere else. The record's own encoding still separates them
(`crate::runner::policy::canonical_bytes` writes `1:0;` for `None` and
`1:1;0:;` for `Some("")`), because a record that acquired an empty digest by
some other route — a hand-edited `owner.json`, a future runtime — must not
silently equal one that has none. Both halves are asserted; see
`tests::a_runtime_that_reports_no_digest_and_one_that_reports_an_empty_string_both_resolve_to_none`.

## `fn inspect_volumes(`

Every credential volume, in the map's own sorted order.

R20 is `operator_owned` and `persistent_output` in all five `at_run_end`
outcomes — "never created or pruned by a run" — so the only operation this
module performs on a volume is the read-only presence question.

## `pub fn rebuild_by_inspection(`

---------------------------------------------------------------------------
The rebuild-from-record path
---------------------------------------------------------------------------

## `pub fn rebuild_by_inspection(`

Re-establish the recorded runner by inspection alone — three of the four
rebuild behaviours.

From `decisions.sequential_substrate.runner` and INV-23:

| behaviour | what happens |
|---|---|
| today's `[runner]` config **differs** | warn **naming the difference**, and ignore it — the record wins |
| the recorded reference **now names another image** | warn, and use the recorded id |
| the record cannot be re-established (runtime / recorded id / volume / a recorded mount source gone or rebound / a recorded mount target a fresh run refuses) | **refuse, before any spawn** |

The fourth — a recorded shell or CLI that fails inside the image — is not
here and cannot be: it is observed only by spawns. See
[`rebuild_from_record`].

**The record wins, exactly.** The returned policy is the recorded one, field
for field: a run's boundary and image are fixed for its life, so nothing
today's config says may reach the value the next `run_resumed(4).runner` has
to equal.

#### Why refusals come before warnings

Both warnings describe a run that is about to continue. A rebuild that
refuses is not one, and a warning that the operator's config differs from a
record whose run just stopped is noise pointing at the wrong thing. So every
inspection that can refuse runs first, and a refused rebuild emits **no**
warnings at all.

### Errors

The first [`InspectionRefusal`] — an incomplete record, a recorded mount
target a fresh run refuses, a recorded mount source that moved, an
unavailable runtime, a recorded image id the runtime no longer holds, or an
absent credential volume.

## `for mount in &record.mounts {`

`recheck_recorded_mounts`, which the rebuild asks before the runtime is
asked anything. A recorded target a fresh run refuses is refused first,
as `RecordedTargetRefused`: the record is not one this run may continue
from, whatever its sources. Then a recorded mount source that is gone,
rebound or another object refuses. Both refusals are `Inspection` ones, so
they come before any spawn; a moved source leaves the run resumable: put
the source back and the next resume re-establishes the record. Only the
recorded targets and identities are inspected; nothing today's config
names is opened, so a source configured today that does not exist cannot
refuse or replace the record.

## `runtime.probe().map_err(InspectionRefusal::from_runtime)?;`

-- the three refusals, before anything is spawned or warned about ------

## `if runtime`

The **recorded id**, which is a different question from the reference.

## `match runtime`

-- the two warnings, on a rebuild that is going to succeed -------------
A reference that now resolves elsewhere, or no longer resolves at all, is
not a refusal: `expected_failures_refusals[1]` names the *id*, and INV-23
says "a reference that now names another image warns while the recorded
id is used ... so a moved reference cannot change what executes".

## `pub fn rebuild_from_record(`

Re-establish the recorded runner, and certify it with the RunnerPreflight.

This is the whole of INV-23's rebuild: [`rebuild_by_inspection`] for the
three refusals that need no spawn, then the [`RunnerPreflight`] for the one
that does. `expected_failures_refusals[2]`: "a recorded shell or agent CLI
that fails inside the recorded image is observed by the RunnerPreflight probe
spawns (registered container invocations, reclaimed like every probe) and
refuses before any recovery event or work spawn".

**The split is in the control flow, not in a comment.** No probe can run
until every inspection has passed, because the probe call is after the `?`
that carries the inspection refusal out. [`RebuildRefusal::before_any_spawn`]
reports which side of it a refusal came from.

### Errors

[`RebuildRefusal::Inspection`] before any spawn, or
[`RebuildRefusal::Preflight`] from the probe spawns.

## `static EMPTY: BTreeMap<String, String> = BTreeMap::new();`

An empty volume map, for a record whose `credential_volumes` is `None`.

`completeness()` refuses that record before this is reached; the binding
exists so the refusal is the one that reports, rather than a panic.

## `fn configured_difference(record: &RunnerPolicy, today: &RunnerSelection) -> Option<RunnerField> {`

Which field of the recorded runner today's `[runner]` config disagrees with,
if any.

ST-20 names exactly three: *"a resume under a `[runner]` config that differs
(**kind, image reference, or credential volumes**) warns naming the
difference"*. Those are the three a config can move; the immutable image id
and the manifest digest are the runtime's answers and are not written in
`upstroke.toml` at all.

So the comparison is the record with the config's three fields overlaid, and
[`RunnerPolicy::difference`] — PR3's, which *"names **which** field moved
rather than"* merely reporting inequality — is what reads it. Taking the id
and digest from the record rather than leaving them empty is what keeps
[`RunnerField::ImageId`] and [`RunnerField::ImageDigest`] unreachable here:
a warning that said "image id" would be naming a field the operator cannot
edit and cannot fix.

#### An absent `[runner]` section is a selection, not an exemption

`PR6-CORRECTNESS-015`. This began with `if !today.from_config { return None
}`, on the reading that "a resume whose repository never configured a runner
must not be told its runner kind moved". That sentence is true, and the
guard is not what makes it true: [`RunnerSelection::host_default`] — what an
absent section means — renders through [`today_in_record_shape`] as exactly
`{Host, HostV1, None, None}`, which is exactly what a host run records, so
[`RunnerPolicy::difference`] already answers `None` for it. The guard bought
nothing there and cost the case it was hiding: a run that recorded a
**container** runner and whose `[runner]` section was then **deleted**.
Today's effective selection is the host default, which is as real an edit as
changing the kind in place, and the operator got no warning at all — the one
clause `decisions.sequential_substrate.runner` states about a differing
config ("warns naming the difference and is ignored") silently did not apply
to the largest possible difference.

So the comparison is unconditional and the *rendering* carries the rule:
absent means host-default, and a host record with no section still produces
no warning because there is genuinely no difference.

#### Mounts are compared by meaning

`RunnerField::Mounts` is the fourth field a config can move. Today's
mounts cannot be put into the record's shape and compared whole: they
carry no identity, so every config would differ. `today_in_record_shape`
copies the recorded mounts instead, and `same_configured_mounts` compares
what the operator wrote, by meaning: each source canonicalized where it
resolves today (spelled as written where it does not), the target, and
`read_only`, ordered by target. An unchanged config, however its sources
are spelled, does not warn; a moved target, disposition or source, or a
mount added or removed, warns `configured mount set` and is ignored.

## `fn same_configured_mounts(recorded: &[RecordedMount], configured: &[RunnerMount]) -> bool {`

See `configured_difference`.

## `fn today_in_record_shape(record: &RunnerPolicy, today: &RunnerSelection) -> RunnerPolicy {`

Today's `[runner]` selection, expressed in the recorded record's shape.

## `fn today_in_record_shape(record: &RunnerPolicy, today: &Run…` › `id: record`

From the record: see `configured_difference`.

## `#[cfg(test)]` › `pub(crate) mod fixtures {`

Test-only, after every production item, and a module so that the first
`#[cfg(test)]` of the file is one (`effects::tests::every_production_region_that_stops_early_stops_at_a_module`).
`plain_checkout` is what Git reports for an ordinary checkout, whose `.git`
directory is both its Git directory and its common directory: the shape of
every fixture that builds a runner or a context over one repository. A test
about any other layout names the two directories itself.

`REFUSED_RECORDED_TARGETS` is K1's guard set, the four kinds of recorded
target regular review 4 named: the container's root, a reserved target
(`/upstroke/settings`), a target not in normal form
(`/cache/../upstroke/settings`, which Docker normalizes into the reserved
one) and two nested targets, each with the words its refusal carries. Each
is a self-consistent record: absolute, strictly ordered, so completeness
accepts it and only the target rule can refuse it. The resolve, exec,
recover and engine guards share it, so the four uses are tested against
one set.

`with_recorded_targets` is a record as a run's files hold it with the given
targets: one copy of the record's first mount per target (its source and
identity, so the source recheck passes), the policy written with
`serde_json` and read back, and the read required to equal what was
written. A recorded policy deserializes whatever its targets are; the
refusal has to come from where the record is used.
