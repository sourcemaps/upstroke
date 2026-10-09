# `src/workspace.rs`

Extended notes for [`src/workspace.rs`](../../src/workspace.rs).

The code is the authority for what it does. This file preserves the migrated prose. Each section
names the source item and, where needed, the line its comment described. Each code snippet in a
heading is a literal source lookup string.

The source retains the `LEGACY-EFFECT` allowlist-placement marker above its governed lint
allowance. Lint reason strings stay in their attributes.

## Module

Workspace (DESIGN.md §6): the engine owns git. Agents edit files; only the
engine stages, commits, branches, and rolls back (invariant 1). Every git
operation is a subprocess of the system `git` binary — no library binding.

### LEGACY-EFFECT

`decisions.effect_site_inventory.mechanism` puts this module in the **frozen
legacy section** of `effects/allowlist.toml` by name: "legacy modules frozen
at PR5 (… legacy branch/checkout/commit operations in src/workspace.rs …)
each carrying a LEGACY-EFFECT justification". The justification is that
sentence's own: these are the schema-1..3 engine's Git operations, they are
reached only by legacy paths, and `invariants_preserved[1]` requires their
behaviour to be untouched by this slice. The schema-4 primitives —
execution root, detached worktrees with intents, exact snapshots, engine
refs, and the Git-object creation contexts — live behind typed funnels in
[`crate::workspace_manager`] instead, and nothing here calls them.

The freeze was amended once, to close
`LEGACY-WORKSPACE-READS-REPLACEMENT-OBJECTS`, and the allowlist row says what
it covers. The Git children are built by `git_command` (its section below),
which sets `crate::workspace_manager::NO_REPLACEMENT_OBJECTS` and passes
`-c core.useReplaceRefs=false`, so they read the objects the repository holds
rather than whatever `refs/replace/*` points at them.
`Workspace::recorded_objects_scope` gives the v0.1 runner the repository and
the include file its role processes read that graph through. And
`ensure_execution_prerequisites` refuses a Git older than 2.41 by name. This
module reads that constant and calls none of the manager's funnels.

**A second amendment, adopted:** B-W924-R1, by the owner's ruling of 2026-10-08
(the follow-up B record, `reviews/2026-10-01-pr11-follow-up-b-record.md`, §9.25
and §9.28). The snapshot path's three registry commands run again past another
registration's empty `commondir`, and nothing else in the module changes. It
governs on master. In this draft it is superseded at those three commands, the
whole of its scope, by the third amendment below, under the owner's ruling
B-W924-R1-S, which is **proposed, effective only together with O8, and not
adopted**: its call-site wrapping, its helper, its predicate, its destination
check and its two constants are withdrawn by its documented ordered rollback (the
follow-up B record's §9.25.4), at those commands only, and each of its regression
tests is mapped to a witness of the third amendment's access (the follow-up D
record's §5.20). The allowlist row records all three amendments.

A third amendment is **proposed, conditional on the owner's decision O8 (decision B)
and on the owner's ruling B-W924-R1-S, and not granted**: follow-up D (#331, a draft), to close
`PR329-LEGACY-RUNS-IN-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY` and R-G. The
row's text in `effects/allowlist.toml` gives it exactly, marked there as proposed, and
the record `reviews/2026-10-02-pr11-follow-up-d-record.md` (§1 to §4, and its
Implementation section) is why. It is three things. The three Git children that
enumerate the shared worktree registry — `switch_branch`'s `git switch`,
`add_gate_worktree`'s `git worktree add`, and `cleanup_gate_workspace`'s
`git worktree remove` with the `git worktree list` that decides it — each run as one
attempt of `crate::workspace_manager::tolerant_registry_access`, follow-up B's
(#329); `git_command` refuses Git's automatic maintenance; and, from R-D1's
preservation design (`~/orch-pr11/owner-package/RD1-PRESERVATION-PROPOSAL.md`, round 4,
sha256 `f9e81c07…`, D's implementation round 3, its record §5.17), a legacy attempt's
output is kept: a kept pin is written whatever `HEAD` is (part K), `pin_checkout` pins
what the checkout holds (part N's capture), and `discard_into_kept_pin` removes the
checkout's copy only once a kept pin and a copy of it the same process made durable
both hold it (part G4). Their sections below say how. Nothing it adds is in force until the owner adopts O8 and B-W924-R1-S and the pull request
merges. The module still calls no funnel of the manager: the access takes no site.

The section "may only shrink after PR5 (the test compares against the frozen
list)", so this attribute is a ceiling rather than a licence.

## `pub struct CapturedCandidate {`

The immutable candidate captured immediately after staging. Every gate,
review, prepared commit, and CAS uses these object identities rather than
consulting a mutable index again.

## `pub struct CapturedCandidate` › `pub branch_ref: String,`

The exact direct branch ref that owned `parent_oid` when this candidate
was captured. An object id alone is insufficient: two branches may
legitimately point at the same commit while only one belongs to the run.

## `pub(crate) const REVIEW_DIFF_FLAGS: &[&str] = &[`

The fixed arguments of a reviewable diff, before its two revisions.

**Shared because there are now two callers and one meaning.** Schemas 1–3
capture the diff from the task workspace ([`Workspace::capture_candidate`]);
the schema-4 driver captures a tree and asks
[`crate::workspace_manager::WorkspaceManager::candidate_diff`] for the diff
of that tree against its parent. Both produce the text a reviewer judges and
`classify::diff_failure` reads, so both must be the *same* text.

Every flag is load-bearing and each one defends against operator config
rather than against Git's defaults. A configured `diff.external`
(difftastic and friends) replaces the output wholesale; `color.ui` injects
escape codes; `textconv` substitutes a rendered form for the bytes. Any of
those corrupts every downstream check that reads the diff — and
`capture_diff_is_immune_to_user_diff_config` is the test that says so.

## `pub(crate) const KEPT_PIN_SUFFIX: &str = "-kept";`

The kept pin of a legacy attempt is the attempt's prepared-pin name with this appended:
`refs/upstroke/prepared/<run>/<task index>-<attempt>-kept`. It is unique per run, task and
attempt, because an attempt number is never reused in a run, and no prepared-commit path
names it: the resume's orphan removal names `prepared_pin_ref` exactly, the schema-3
settlement check builds the exact expected pin, and the topology's pins live under
`refs/upstroke/runs/`. Defined here since R-D1's part K, which `prepare_commit_from_candidate`
reads; the coordinator and the resume import it. No engine path removes a kept pin; the
operator does (R-D7).

## `const CAPTURE_CONTROLS: [&str; 2] = ["-c", "core.sparseCheckout=false"];`

Passed ahead of every subcommand of the kept-output capture and revert (`controlled`), so
that a sparse checkout configured since the resume's own refusal of one
(`refuse_sparse_checkout`) cannot narrow what the capture takes or what the revert writes.

## `const PRIVATE_INDEX_CONTROLS: [&str; 4] = [`

R-D1 round 4's RD3-1 (the proposal's §4.6): `-c core.splitIndex=false -c
splitIndex.sharedIndexExpire=never`, passed by the two private-hooks builders with
`GIT_INDEX_FILE` on a workspace value that carries a private index file, and on no other.
The private index file sits in the checkout's own Git directory and takes the checkout's
configuration by design, so under `core.splitIndex=true` Git would write it split — a new
`sharedindex.*` beside the checkout's own — and every split write that creates a shared
index file unlinks each other shared index of that directory older than
`splitIndex.sharedIndexExpire`, the one the checkout's own index names included. Round 3's
capture did exactly that (RD3-1: `git status` exit 128 afterwards). The first control writes
the private index whole, so no split write runs for it; the second makes whatever still
splits it — an inherited `GIT_TEST_SPLIT_INDEX` — unlink nothing, and so leaves orphan
shared index files in the checkout's Git directory. Git removes an orphan only at a later
split write of the checkout's own index that creates a shared index file, and only once the
orphan is older than the checkout's configured `splitIndex.sharedIndexExpire`. With the
default, `2.weeks.ago`, an orphan stays until it is older than two weeks and such a write
follows; with `never`, automatic expiry never reclaims it; and repeated captures can
accumulate them. A disclosed cost, left to the owner's choice and not accepted here; the
variable is O3's class, with FUD-D4-ENV's. Both are command-scope settings, like
`core.fsmonitor=false`: they decide how the private file is laid out and what its writes may
unlink, never what the capture takes.

## `const REPLACE_REFS_REFUSED: [&str; 2] = ["-c", "core.useReplaceRefs=false"];`

The second half of what `git_command` sets, and the half the floor needs. Git
2.41, the oldest version `README.md` supports since #326 round 4, and 2.40 before
it, read `core.useReplaceRefs` in their default configuration as
`read_replace_refs = git_config_bool(...)`, which overwrites what
`GIT_NO_REPLACE_OBJECTS` set; 2.42 made the variable final (`disable_replace_refs`).
So on 2.40 and 2.41 a `core.useReplaceRefs = true` in the system, global,
repository or worktree configuration, or in command-line configuration a child
inherits, turned replacements back on for a child that carried only the
variable (measured on both, at each of those five places). With `-c
core.useReplaceRefs=false` as well, the child read the recorded graph at every
place and on 2.40.0, 2.41.0, 2.42.0 and 2.43.0 (measured): a `-c` on the command
line is applied after every file and after inherited command-line
configuration, and Git passes it on to the children it starts, as `worktree
add` starts `reset --hard`.

This module's own replacement witnesses showed it before anything else did:
their fixtures pin `core.useReplaceRefs = true` in the repository, for the
reason `pin_replacement_refs_in` gives, and with the variable alone both failed
on 2.40.0 and 2.41.0 and passed on 2.42.0 and 2.43.0 (`r3/` in the pull
request's evidence). With this, both pass on all four.

## `const AUTO_MAINTENANCE_REFUSED: [&str; 8] = [`

`-c maintenance.auto=false -c gc.auto=0 -c gc.autoDetach=false -c
maintenance.autoDetach=false`, which `git_command` passes after `REPLACE_REFS_REFUSED`
(follow-up D's R-G, proposed, conditional on O8). Git's automatic maintenance can
start a `git worktree prune` that deletes a registration with no `gitdir` at once,
whatever the expiry — the state another checkout's add is in before its first
write (`should_prune_worktree`). Five builtins start it (`am`, `commit`, `fetch`,
`merge`, `rebase`); the legacy engine runs none of them in production, so its one
route is a promisor lazy fetch a legacy child starts in a partial clone, and at Git
2.55.0 the default `geometric` strategy prunes worktrees. A `-c` setting outranks
every configuration file and reaches every Git process the child starts through
`GIT_CONFIG_PARAMETERS`, and `maintenance.auto=false` stops `maintenance run --auto`
before it is built (`prepare_auto_maintenance`), so no Git child of this module, and
no Git process one of them starts, runs automatic maintenance. On Windows maintenance
would run attached, and the settings stop it the same way. What this does not reach:
maintenance or a prune that a role's own Git, or the user's, starts (R-D10).
`every_git_child_of_this_module_runs_with_automatic_maintenance_off` reads the four
values back through the builder over a repository configuring the opposite.

## `fn git_command(directory: &Path) -> Command {`

Where every Git child this file's production code starts is built, and so the
one place their environment is set. It supplies the `-C <directory>` each of the fourteen
production sites supplied for itself until
`LEGACY-WORKSPACE-READS-REPLACEMENT-OBJECTS`, in the same position, and two
things none of them did: `NO_REPLACEMENT_OBJECTS`, and `REPLACE_REFS_REFUSED`
right after the directory, before anything the call site adds.

`git replace P Q` makes Git read `Q` wherever `P` is named while `rev-parse`
still prints `P`. Measured on git 2.43.0 with this module's code before the
builder: `capture_candidate` took `P` from `rev-parse HEAD` and diffed it
against the staged tree, so with the replacement installed its review payload
was `-two +agent-edit` where without it the payload was `-one +agent-edit`,
while `commit-tree -p P` writes `parent P` either way; and a gate snapshot of a tree `A` with
`refs/replace/A -> B` installed checked out `B`'s bytes under a commit that
records `A`. With the pair set, both read what the repository holds.
`design/15`, "What an exact snapshot is exact against", is the rule for the
schema-4 path, which sets the same pair from the same constant.

The variable decides by its presence: set to `1`, to `0` or to nothing, and
with `core.useReplaceRefs=true` given by `-c` or in the repository's own
configuration, `git show P:f` read `P`'s own content (git 2.43.0, measured). That
holds from Git 2.42 on. On 2.40 and 2.41 the configuration wins, which is why
`REPLACE_REFS_REFUSED` is set beside it; see its section. So a call site gets
the replaced graph back by building a `Command` of its own or by taking either
half off the one it was given, and the census below refuses the shapes it can
see.

`every_git_child_of_this_module_is_built_where_replacements_are_refused` holds
the shape; see its section.

Follow-up D (proposed, conditional on O8) adds `AUTO_MAINTENANCE_REFUSED` after
`REPLACE_REFS_REFUSED`; see its section. The census's assertions about the builder
still hold: one `Command::new(`, the pair set, `REPLACE_REFS_REFUSED` passed, and the
fourteen call sites in the same functions in the same order.

## `impl Workspace` › `pub fn open(root: &Path) -> Result<Self, UpstrokeError> {`

Open an existing git worktree, normalizing to its top level. Running
from a subdirectory would otherwise scope `git clean` to that
subdirectory while staging stays whole-tree, so rollback would leave
residue above the current directory.

## `impl Workspace` › `pub(crate) fn worktree_git_dir(&self) -> Result<PathBuf, UpstrokeError> {`

The administrative directory private to this physical worktree.

A linked worktree's `.git` is a pointer into the common repository, so
joining the visible `.git` path would either fail or collapse distinct
worktrees onto one lease. Git resolves the exact per-worktree directory
for us without changing tracked or working-tree state.

## `impl Workspace` › `pub fn recorded_objects_scope(`

The repository the v0.1 runner keeps its role processes on the recorded graph
of, as `crate::runner::host::ManagedRepository` names it. `engine::run_harness`
and `engine::resume_harness` call it before anything else, so every run and
every resume writes the include again before its first role starts, whatever
became of it in between.

- **The common directory is Git's answer, made canonical.** `rev-parse
  --path-format=absolute --git-common-dir`, through `git_command` like every
  child here, then `fs::canonicalize`: Git compares an `includeIf.gitdir`
  condition with the realpath of each command's Git directory, so a symlinked
  spelling of the same directory would not match. A linked worktree, a
  separate Git directory and a `.git` behind a symbolic link all resolve to the
  one directory every worktree of the repository shares.
- **The include lives under the private root**, at
  `<private root>/git/recorded-objects.gitconfig`: engine-owned, outside every
  worktree, and, unless no home directory resolves, outside the system temporary
  directory that sweepers and `systemd-tmpfiles` age out. No Git configuration
  file names it; only the roles' environment does. `private_root` defaults as a run's private
  half does (`rundir::default_private_root`), so a test that passes its own
  root keeps the file inside its own tree.
- **One file per private root, shared.** Its bytes never vary, so runs at once
  in sibling worktrees can each write it; a run finds it whole and leaves it, or
  stages its own copy beside it and renames that into place.

## `const RECORDED_OBJECTS_INCLUDE_NAME: &str = "recorded-objects.gitconfig";`

The include's file name under the private root's `git` directory.

## `fn write_recorded_objects_include(directory: &Path) -> Result<PathBuf, UpstrokeError> {`

Leave the include alone if it already holds
`crate::runner::host::RECORDED_OBJECTS_INCLUDE`; otherwise create the directory
owner-private, write the bytes to a uniquely named sibling (`create_new`, so
two writers never share one), and rename it over the include. A rename that
fails is accepted only when the file it would have replaced now holds the exact
bytes: a writer that lost a race to a sibling run can fail to replace a file
that is open elsewhere on Windows, and the bytes it wanted are there anyway
(reasoned, not measured on Windows). Anything else is an error, and the run
refuses before any role starts. An altered file is replaced, never adopted.

## `impl Workspace` › `fn git_path(&self, args: &[&str]) -> Result<PathBuf, UpstrokeError> {`

Decode one path printed by Git without requiring Unix path bytes to be
UTF-8. Git appends a platform line ending; remove only that delimiter,
never legal leading or trailing path bytes.

## `pub struct Workspace` › `private_index: Option<PathBuf>,`

`None` for every workspace the engine opens (`open`'s two values, the gate workspace's, and
`canonical_common_dir`'s probe). `Some` only on a *view* of the same checkout that
`with_private_index` makes for the kept-output capture, the revert's second index and the
post-check: its two private-hooks builders — `run_git_with_private_hooks` (and through it
`git_with_private_hooks` and `git_output_with_private_hooks`) and `git_output_with_input` —
then set `GIT_INDEX_FILE` to that file, with `PRIVATE_INDEX_CONTROLS`, ahead of the
subcommand. Git names an index file only through that variable, never by an argument or a
configuration key, and another Git directory reads another configuration (the proposal's
§4.2-§4.3), so this is the one faithful private capture. The plain builder (`git`,
`git_output`, `git_path`) never sets it. The two `.env(` it adds are the payload census's row
for this module, five to seven (`src/runner/contract.rs`), an additional conditional proposal
with O8's texts.

## `impl Workspace` › `pub(crate) fn run_git_with_private_hooks(`

Run a Git command with every repository-configured hook and fsmonitor
disabled. Keep this raw-output primitive reusable by reference updates,
whose expected compare-and-swap failures need the real exit status.

## `impl Workspace` › `let writer = std::thread::spawn(move || stdin.write_all(&input));`

Read stdout/stderr while feeding the complete NUL-delimited path
list. A large index can otherwise fill check-attr's stdout pipe and
deadlock the parent while it is still writing stdin.

## `impl Workspace` › `.env("GIT_AUTHOR_NAME", "upstroke")`

Environment identity overrides repository/global config and any
inherited GIT_AUTHOR_* or GIT_COMMITTER_* values.

## `impl Workspace` › `pub fn is_clean(&self) -> Result<bool, UpstrokeError> {`

§14 pre-flight: the engine refuses dirty trees.

## `impl Workspace` › `pub fn ensure_execution_prerequisites(&self) -> Result<(), UpstrokeError> {`

Repository prerequisites whose absence would make the captured tree
incomplete or its attribute policy unverifiable, or would let a role read
another object graph than the one the engine writes. Run this before any
worker is dispatched on both fresh and resumed runs.

The Git floor comes first. Git 2.40's `git merge-tree` reads a replaced commit
whatever its configuration says (measured with the includes and with `-c
core.useReplaceRefs=false`; 2.41.0 read the recorded commit with either), so no
configuration keeps a v0.1 role on the recorded graph there, and the owner
raised the floor to 2.41 on 2026-09-28 rather than have another mechanism
attempted. The version asked is that of the `git` on `PATH`, the one every role
inherits; a role that runs another Git is outside what this can see.

## `const GIT_FLOOR: (u32, u32) = (2, 41);`

The oldest Git `README.md` supports.

## `fn require_git_floor(reported: &str) -> Result<(), UpstrokeError> {`

Read the first two numbers after `git version ` and refuse anything below
[`GIT_FLOOR`], and anything it cannot read, naming what `git version` printed.
Vendor suffixes (`.windows.1`, ` (Apple Git-154)`) follow the two numbers and
are ignored.

## `fn refuse_sparse_checkout(&self) -> Result<(), UpstrokeError> {` › `let index = self.git_output_with_private_hooks(&["ls-files", "-t", "-z"])?;`

`-t` reports the skip-worktree tag as an uppercase `S` even when an
entry is also marked assume-unchanged. (`-v` would lowercase that
tag and could let a manually sparse index evade this preflight.)

## `impl Workspace` › `pub fn current_branch_ref(&self) -> Result<String, UpstrokeError> {`

The full direct branch ref currently checked out by this worktree.
Prepared publication is deliberately unavailable from detached HEAD or
through a symbolic branch alias: the run records one concrete local ref.

## `impl Workspace` › `pub fn head_sha_full(&self) -> Result<String, UpstrokeError> {`

Full HEAD sha. The event log records these rather than short ones
because `--short` picks its length from `core.abbrev` and the repo's
object count — a sha written by one checkout would not compare equal to
the same sha read by another, which is exactly the check §15 asks
`resume` to make.

## `impl Workspace` › `pub fn parent_sha(&self, sha: &str) -> Result<Option<String>, UpstrokeError> {`

The full sha of a commit's first parent — `None` at a root commit.

How `resume` tells a commit sitting directly on its own record apart
from history that arrived some other way.

## `pub fn parent_sha(&self, sha: &str) -> Result<Option<String>, UpstrokeError> {` › `return Ok(None);`

A root commit has no parent. That is an answer, not a failure.

## `impl Workspace` › `pub fn commit_subject(&self, sha: &str) -> Result<String, UpstrokeError> {`

A commit's subject — the first line of its message.

## `impl Workspace` › `pub fn switch_branch(&self, name: &str) -> Result<(), UpstrokeError> {`

Move to an existing branch — how `resume` gets back onto the run's own
branch when the operator has wandered off it.

`git switch` refuses a branch checked out elsewhere by scanning the worktree
registry (`die_if_checked_out`) before it changes anything, so it dies on another
checkout's registration half written. Under follow-up D (proposed, conditional on
O8) the switch is one attempt of `tolerant_registry_access`, with no hold and a
veto that always answers `Again::Attempt`: a failed attempt changed nothing, so the
next attempt is the same attempt. A failure that outlasts the access's deadline —
contention, residue, or a genuine failure of the switch, which the access cannot
tell apart — refuses as `UpstrokeError::RegistryRefused` instead of a Git error at
once (R-D6). The resume calls this only over a clean checkout, so the refusal
discards nothing. `a_branch_switch_beside_a_tear_its_writer_finishes_switches`.

## `impl Workspace` › `pub fn branch_exists(&self, name: &str) -> Result<bool, UpstrokeError> {`

Whether a branch exists locally.

## `impl Workspace` › `pub fn uncommitted_summary(&self) -> Result<Vec<String>, UpstrokeError> {`

A one-line-per-path summary of everything uncommitted, for telling the
operator what a resume is about to discard.

## `impl Workspace` › `pub fn ensure_run_exclusions(&self) -> Result<(), UpstrokeError> {`

Keep `.upstroke/` (run dirs, transcripts) out of `status` and out of the
engine's own commits.

This is a self-ignoring `.upstroke/.gitignore` containing `*` (the
pattern cargo uses for `target/`) rather than an entry in
`.git/info/exclude`: it needs no read-modify-write of a file the user
owns, disappears with the directory, and — unlike `info/exclude` under
`--git-dir` — behaves correctly in a linked worktree, where git reads
excludes only from the common directory.

## `impl Workspace` › `pub fn capture_candidate(&self) -> Result<CapturedCandidate, UpstrokeError> {`

Stage everything, freeze one parent and tree object, and return their
complete diff. The diff names those frozen objects rather than rereading
HEAD or the index, so all three values remain one candidate even if a
ref or the index changes afterward.

The diff must be a plain unified diff regardless of user config: a
configured `diff.external` (difftastic and friends) would replace it
wholesale and `color.ui` would inject escape codes, corrupting every
downstream check that reads it.

## `impl Workspace` › `pub fn capture_diff(&self) -> Result<String, UpstrokeError> {`

Backward-compatible diff-only capture for existing callers.

## `fn worktree_filter_problem(&self, operation: &str) -> Result<Option<String>, UpstrokeError> {` › `let paths = self.git_output_with_private_hooks(&[`

Commands that inspect or update worktree entries (`add`, `status`,
`switch`, `commit`) can run clean/process filters before a later
tree policy check. Enumerate tracked and addable untracked paths
without refreshing fsmonitor, then evaluate the worktree's
attributes without invoking a driver.

## `impl Workspace` › `pub fn review_input_problem(&self) -> Result<Option<String>, UpstrokeError> {`

Refuse staged evidence whose bytes are not the bytes a gate would see,
or whose worktree still contains unstaged nested state after `git add`.
A clean/smudge filter makes the cached diff describe the transformed
blob while gates see the smudged file. Dirty submodules similarly hide
executable inputs behind an unchanged gitlink. Neither can be reviewed
completely, so both are policy failures rather than gate results.

## `impl Workspace` › `pub fn review_input_problem_for_tree(`

Inspect live nested-worktree state, then bind every semantic input check
to one captured tree rather than to an index that may have moved since
its diff was produced.

## `fn tree_input_problem(&self, tree_oid: &str) -> Result<Option<String>, UpstrokeError> {` › `let entries = self.git_output(&["ls-tree", "-r", "-z", "--full-tree", tree_oid])?;`

A captured .gitattributes can attach a filter to an otherwise
unchanged file, so changed names are insufficient. `ls-tree`
enumerates every path in the exact candidate and exposes gitlinks.

## `impl Workspace` › `pub fn staged_tree_oid(&self) -> Result<String, UpstrokeError> {`

Read the full object ID of the index tree once. Callers that run more
than one verifier can retain this identity and materialize the same
bytes for each verifier even if the source index later changes.

## `impl Workspace` › `pub fn gate_snapshot(&self) -> Result<GateWorkspace, UpstrokeError> {`

A clean detached worktree whose HEAD tree is exactly the staged tree.
Kept for existing callers; new callers that need more than one snapshot
should retain `capture_candidate()` and use
`gate_snapshot_for_candidate()`.

## `impl Workspace` › `pub fn gate_snapshot_for_tree(&self, tree_oid: &str) -> Result<GateWorkspace, UpstrokeError> {`

Materialize a clean detached worktree for one exact tree object ID.
Gates run here, never in the worker's workspace, so ignored files,
build residue, and gate side-effects cannot influence or contaminate the
commit under review.

## `impl Workspace` › `pub fn gate_snapshot_for_candidate(`

Materialize one frozen candidate. Both object IDs are supplied so a
concurrent ref move cannot silently change the ephemeral commit's
parent after the candidate was reviewed.

## `impl Workspace` › `pub fn gate_snapshot_for_candidate_in_store(`

Materialize a candidate under a durable, caller-owned snapshot store.
The intent is synced before Git registers the worktree, allowing resume
to reclaim a snapshot whose owner was terminated without running Drop.

## `impl Workspace` › `pub fn reclaim_gate_workspaces(&self, store: &Path) -> Result<usize, UpstrokeError> {`

Reclaim every durable gate-worktree intent in `store`. Callers must use
the same repository that created the store; intent names contain no
path supplied by the candidate and cannot escape these fixed children.

## `impl Workspace` › `let workspace = Workspace {`

The exact path is already known to be the new worktree's top level.
Avoid round-tripping it through Git's textual path output, which is
not necessarily UTF-8 on Unix.

## `impl Workspace` › `fn add_gate_worktree(`

The snapshot's add, as one attempt of `tolerant_registry_access` (follow-up D,
proposed, conditional on O8): the `git worktree add` child and its exit check. It
holds the registry lock shared, as the manager's adds do, under the canonical common
git dir `canonical_common_dir` resolves, so a legacy process's lock key and refusal
text match the manager's for the same repository. Its veto is `legacy_add_veto`.
Git's add scans its siblings, makes its entry and that entry's `locked`, and only then
takes the destination over; on any failure after that its junk removal removes the
entry and the destination. So the add is attempted again only over the empty
directory `PendingGateWorkspace` made, and anything else refuses at once. The add's
own failure is never returned as Git state: no arm of the veto answers
`Again::Return`. What that costs a genuine checkout failure after the takeover (a
path the snapshot cannot hold, a failing filter, a store that refuses a write, a
missing object): a resumable refusal with the output pinned, where master discarded
it; each resume over a cause that persists pays one more worker attempt, as master's
did, and keeps one more pin.

A deletion that leaves the add successful — a prune that decided in the add's window
and deletes the registration before or after the add returns — never reaches the
veto, and the verification below then fails as Git state: that is R-D9, inside
`PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, not closed here.

## `impl Workspace` › `pub fn prepare_commit_from_candidate(`

Prepare and pin a commit from the exact candidate identities already
used by gates and review. This never rereads the mutable index.

R-D1's part K (proposed, conditional on O8): `HEAD` is observed and compared with the
captured branch and parent only for a pin whose name does not end with `KEPT_PIN_SUFFIX`. A
kept pin, which nothing publishes, is written from the parent and tree it is given whatever
`HEAD` is — moved, detached, on another branch, unborn or symbolic — with every other
check, the create-only write and its verification as before; a publication pin still refuses
a moved `HEAD` (T-K1). Without K, a branch another client moved and moved back (A1, A1h), or
moved after the capture (A2, A2h), refused the pin and the output went with the next discard.

## `impl Workspace` › `pub fn commit(&self, message: &str) -> Result<String, UpstrokeError> {`

Commit whatever `capture_diff` staged. §14: commit-per-task,
`[upstroke] <task-id>: <title>`.

## `impl Workspace` › `fn symbolic_ref_target(&self, refname: &str) -> Result<Option<String>, UpstrokeError> {`

Return the immediate symbolic target without dereferencing it.

## `impl Workspace` › `pub fn remove_orphan_prepared_pin(&self, pin_ref: &str) -> Result<(), UpstrokeError> {`

Remove a private pin for an attempt that never durably recorded a
successful settlement. The target is read then supplied as the expected
old value, so even cleanup is compare-and-swap.

## `impl Workspace` › `pub fn discard_uncommitted(&self) -> Result<(), UpstrokeError> {`

Discard everything since the last commit: staged, unstaged, and
untracked (ignored files survive). This is both the §14 rollback on a
failed attempt and the post-commit scrub that keeps gate side-effects
(build artifacts, lockfile churn) from leaking into the next task's
captured diff.

## `impl Workspace` › `pub(crate) fn pin_checkout(`

Part N's pin (the coordinator's `kept_on_error`, for an attempt error after the worker ran):
the checkout captured into a private index (`capture_checkout`, footprint included) and pinned
at the kept name through `prepare_commit_from_candidate` under K. `Ok(None)` when the capture
is `HEAD`'s tree: nothing to keep. It discards nothing.

## `impl Workspace` › `pub(crate) fn discard_into_kept_pin(`

Part G4, the resume's guarded exact discard of the attempt in flight (the proposal's §5.2):
the capture, then `discard_held`, then the private index removed whatever happened, then
`uncommitted_summary` for what stayed (`KeptDiscard::left_in_place`). It removes nothing a
kept pin of upstroke's and a copy of it this same process made durable do not both hold, and
refuses, discarding nothing it does not hold, when any step fails.

## `impl Workspace` › `fn discard_held(`

The order is the guarantee. A capture equal to `HEAD`'s tree resets only the checkout's
index. Otherwise: the attempt's copies are read before any pin is written (`attempt_copies`);
copies naming different commits refuse, naming both; then the pin — none and no copy:
written from the capture under K; none and a copy: put back from the copy (`restore_pin`,
recorded as `restored_from`) and checked; a pin and a copy: they must name the same commit,
else refuse naming both (RD2-1m); a pin alone: checked (`held_by_pin`). Submodule paths and
new paths the pin never held are left in place (`leave_out`). Then a fresh copy, written and
made durable by this resume whatever copies the attempt already has (`copy_pin`; round 4's
RD3-2: a copy found is never relied on for durability), the discard's own filter check on
`HEAD`'s tree, the two-phase revert, the index reset, and the post-check.

## `impl Workspace` › `fn held_by_pin(`

An existing pin must be upstroke's kept commit of the attempt (`prepared_commit_matches`
with the commit's own parent and tree, and the kept message), else "is not upstroke's kept
commit of this attempt" (C3). Then containment, not equality (after a part-way discard every
path is `HEAD`'s or the pin's, so the next resume finishes with no operator step, G-RESET): a
path the checkout deleted is allowed; a path where the capture and the pin agree is held; a
path new in the capture and absent from the pin is left in place — unless it is in the
discard's way (`CheckoutCapture::footprint_holds`), which refuses ("move them out of the
way", RD2-2p); anything else refuses, naming the paths (X3).

## `impl Workspace` › `fn leave_out(`

The paths left in place take `HEAD`'s entry in the capture's private index (`update-index
-z --index-info` from `diff-tree`'s old side; a path `HEAD` lacks reads `000000 <zeros>` and
leaves the index), so neither phase of the revert ever touches them; `write-tree` gives R.

## `impl Workspace` › `fn attempt_copies(`

The run's public directory, read once: names `kept-<task>-<attempt>-` + 26 ULID characters
+ `.bundle` (`is_attempt_copy`), sorted, each kept only if `git bundle list-heads` names
exactly one head, a full object id at the kept name. A `.partial`, a copy whose `list-heads`
fails, or any other name is never a copy.

## `impl Workspace` › `fn restore_pin(`

`git bundle unbundle` (whose `index-pack` checks the whole pack, so a copy cut short fails
here, where `list-heads` and `verify` pass it), the commit checked as upstroke's kept commit
of the attempt, and a create-only `update-ref --no-deref` with the message "upstroke:
restore kept pin from its copy", verified.

## `impl Workspace` › `fn copy_pin(`

A new name every time — `<stem>-<ULID>` — so no copy is ever written over and the first
complete copy survives every later resume. On failure the `.partial` is removed if it is
still there; a final name whose directory fsync failed stays, never relied on (R-D7 lists it).

## `impl Workspace` › `fn write_copy(`

`git bundle create <partial> <pin> ^<head>` (Git fsyncs no bundle, under any `core.fsync`;
the proposal's §2.3), its one head checked with `list-heads`, then the module's own
durability idiom: `File::sync_all` with write access (`fsync`, `F_FULLFSYNC` on macOS,
`FlushFileBuffers` on Windows), `rename` to the final name, `sync_parent` (the directory's
`fsync` on Unix, nothing on Windows). Each failure refuses before anything is removed.
Whether the calls are those system calls, and what a successful fsync means on a disk, is
reasoned; the order was traced on Linux (record §5.17).

## `impl Workspace` › `fn relative_to_root(`

The copy's path relative to the checkout's root, computed from canonical paths and spelled
with `/`, so that every Git child names it by an argument relative to `-C <root>`. A copy
outside the checkout, or under a name that is not UTF-8, refuses.

## `impl Workspace` › `fn revert(`

Phase A, `read-tree -m -u R T` on the capture's index, T being `HEAD`'s tree less the paths R
lacks (`tree_without`): every path of T is a path of R, so it creates nothing, and rewrites
or removes only R's own paths, each after Git's up-to-date check; no untracked or ignored
file is in its way. Phase B, for the paths R lacks: their `HEAD` entries into the capture's
index and `checkout-index -z --stdin` with no `-f`, which writes only where nothing stands
(`O_EXCL`) and refuses elsewhere; its refusal is returned, not raised, for the post-check to
name. The instant between Git's up-to-date check of a file and its unlink is the disclosed
phase-A window (the proposal's §9.3).

## `impl Workspace` › `fn tree_without(`

T on a second private index file (`-tree`), removed whatever happened; no work tree is
touched (`tree_of`: `read-tree`, `update-index --force-remove`, `write-tree`).

## `impl Workspace` › `fn check_reverted(`

A second capture, with no footprint, on its own private index: every path R changed from
`HEAD` must be back at `HEAD` and phase B must have refused nothing, else "stopped part-way",
naming the paths and phase B's refusal; then `HEAD` must not have moved ("the run branch moved
… during the discard", X19). The next resume finishes a part-way discard.

## `impl Workspace` › `fn capture_checkout(`

The checkout's own filter refusal first (`refuse_worktree_filters_before("git add")`, as the
coordinator's capture does), then a view with a fresh private index file
(`private_index_file`) filled by `fill_capture`; the file is removed on any failure.

## `impl Workspace` › `fn fill_capture(`

`read-tree <head>`, `add -A --ignore-errors` (exit 0 or 1; a path `add` cannot take is left
out, never fatal), `write-tree`; then the footprint (the proposal's §3.2): the paths `HEAD` has
and the capture lacks, walked by `in_the_way`; what stands in the revert's way is added again
with `--literal-pathspecs add -A -f --ignore-errors --pathspec-from-file=-
--pathspec-file-nul`, NUL-separated on standard input, and the tree written again. Every
child names the private index, so none reads the checkout's own index, and every `diff-tree`
it runs reads the capture's (round 4).

## `impl Workspace` › `fn in_the_way(&self, missing: &[Vec<u8>]) -> Vec<Vec<u8>> {`

For each missing path, its first leading component that exists and is not a directory (a
symbolic link counts as not a directory), or else the path itself if anything is there:
`symlink_metadata` only, nothing changes. A path this platform cannot name is skipped, and
phase B then refuses for it.

## `impl Workspace` › `fn checkout_path(&self, path: &[u8]) -> Option<PathBuf> {`

A path Git printed, as bytes, joined to the root: any bytes on Unix, UTF-8 only elsewhere.

## `impl Workspace` › `fn private_index_file(&self, role: &str) -> Result<PathBuf, UpstrokeError> {`

`<git dir>/upstroke-kept-<pid>-<ULID><role>.index` in the checkout's own Git directory (for a
linked checkout, its entry under the common directory's `worktrees/`): Git reads no unknown
file there, and a crash leaves it inert (R-D7).

## `impl Workspace` › `fn with_private_index(&self, index: PathBuf) -> Self {`

A view of this same checkout with only its index file changed: the module's builders read
the root from the value they are given, so the view owns a copy of it.

## `impl Workspace` › `fn changed_paths(&self, from: &str, to: &str) -> Result<Vec<TreeChange>, UpstrokeError> {`

`diff-tree -r -z --no-renames --raw --ignore-submodules=none` through the private-hooks
builder with `controlled`, on the value it is called on: the capture's view (or the
post-check's), so it reads that private index and never the checkout's. Round 2's and round
3's ran it through the plain builder on the checkout's index, which ran the checkout's
fsmonitor hook and, on Git 2.55.0, started its daemon (RD3-1f). Each record keeps `HEAD`'s
side's mode and object id (`TreeChange`), which `leave_out` and phase B write back.

## `pub(crate) struct KeptPin<'a> {`

The kept pin's branch ref, message and name, built by the coordinator and the resume from
the attempt's prepared pin. `KeptCopies` is the run's public directory and the stem
`kept-<task index>-<attempt>`; `KeptDiscard` what a guarded discard did: the kept commit, the
copy it wrote, the copy a missing pin was put back from (`restored_from`), and what stayed.

## `impl CheckoutCapture` › `fn footprint_holds(&self, path: &[u8]) -> bool {`

Whether `path` is a footprint path or under one: what the capture took only because it stood
in the revert's way.

## `fn is_attempt_copy(name: &str, stem: &str) -> bool {`

`<stem>-<26 characters of 0-9 and A-Z>.bundle` exactly: a ULID name, so a `.partial` or a
name another tool made is never read as a copy.

## `enum SnapshotStoreMode` › `EphemeralUnderRoot,`

`store_or_root` is a shared parent such as the system temp directory.
Create one atomically private child and remove it after normal cleanup.

## `enum SnapshotStoreMode` › `ExactDurable,`

`store_or_root` is the stable per-run store whose intents resume scans.

## `impl PendingGateWorkspace` › `fn create(source_root: &Path, temp_root: &Path) -> Result<Self, UpstrokeError> {`

Create a uniquely named, owner-private store beneath a caller-owned
root. The root may be shared (notably `/tmp`) and is never chmodded.

## `impl PendingGateWorkspace` › `fn create_in_store(source_root: &Path, store: &Path) -> Result<Self, UpstrokeError> {`

Use the exact stable store whose synced intents resume will reclaim.

## `fn cleanup_gate_workspace(`

A snapshot's removal and the list that decides whether it took the registration are
**one** attempt of `tolerant_registry_access` (follow-up D, proposed, conditional on
O8), with no hold and a veto that always answers `Again::Attempt`. The attempt
succeeds when the list does not register the path, whatever the removal's exit
status — the existing treatment, which counts an already-unregistered destination
("is not a working tree") as reclaimed — and fails with the removal's words while the
list registers it, or with the list's own error when the list fails. Wrapping the
list apart from the removal (round 4's B1′) let a removal the tear failed be decided
by a list that ran after the tear finished, which still registered the path
(FUB-D4-B1REMOVE; `a_snapshot_removal_beside_a_tear_its_writer_finishes_takes_the_registration`).
Everything after the attempt is as it was. A snapshot's drop now waits up to the
access's deadline when the store is in the way, where it failed at once and left
residue for the resume; a refused add costs two such waits, the add's and then this
cleanup's, each plus its last attempt's runtime.

## `let _ = fs::remove_dir_all(path);`

`worktree remove` normally removes the directory too. Once Git confirms
no registration remains, these exact private paths are safe to remove
even if a partially failed add populated only part of either one.

## `fn canonical_common_dir(root: &Path) -> Result<PathBuf, UpstrokeError> {`

`git rev-parse --path-format=absolute --git-common-dir` through `git_path`, and so
through `git_command`, then `fs::canonicalize`: the two steps
`recorded_objects_scope` takes, and the spelling `tolerant_registry_access` requires
of its key (the manager's `common_git_dir` is the same). The tests read the access's
`CONTENDED_ATTEMPTS` handshake under the same spelling.

## `fn legacy_registry_pause(pause: std::time::Duration) -> Result<(), UpstrokeError> {`

The pause each of the three legacy registry accesses waits out between its attempts: a
sleep on the calling thread, always `Ok`. Follow-up B's head `55029628` (#329: its repair
round 3's R1, `eef97e41`, and round 6's I2-1, `22d70ef6`) takes the wait out of
`tolerant_registry_access` and has every caller hand it one (`pause_for`, whose error ends
the access), the manager's own calls passing their hooks'
`EffectHooks::registry_pause`, whose default is this same sleep. The legacy module has no
hooks, so it passes this function, and its accesses wait as they did when the access
slept inside itself. Part of follow-up D's draft (#331), proposed, conditional on the
owner's decision O8, adapted to B's provisional head by the merge that took it.

## `fn legacy_add_veto(path: &Path) -> Again {`

The snapshot add's veto: the removal predicate, described by what it observes
(FUD-D3-TAKEOVERWORD), as follow-up B's round 8 dates it for both adds. After a
failed attempt it reads the destination with `symlink_metadata`, and `read_dir` when
it is a directory, and starts no Git child:

| The destination after the failed attempt | The answer |
|---|---|
| an empty directory this access can remove | it is removed and made again (`remake_destination`), and the answer is `Again::Attempt` |
| gone | `Again::Undecidable` |
| an empty directory this access cannot remove | `Again::Undecidable` |
| a directory that is not empty, anything that is not a directory (a link included), or metadata it cannot read | `Again::Undecidable` |

`Again::Undecidable` makes the access refuse at once as
`UpstrokeError::RegistryRefused`, naming why, and B-PRESERVE keeps the captured
candidate (`src/engine/coordinator.rs`). Why attempting again over an empty,
removable destination is safe needs no history: it holds nothing to lose, because the
worker's output is the captured candidate in the run's own checkout, and the next
attempt is bounded — it ends in success, or at the deadline in a refusal that keeps.
What reaches that arm, none of it told apart: a failure before Git took the
destination over, the case the arm is for; a takeover whose junk removal emptied the
destination and could not remove it, since become removable (R-D4); a takeover cut
short by a signal before Git wrote there (R-D3). A destination gone means the failure
came after Git took it over — the add's own (a checkout that cannot be made) or a
prune that deleted the add's registration (FUD-D2-PRUNE) — and nothing outside Git
tells the two apart, so the output is kept in both. On Windows std exposes no stable
file identity at the MSRV, and the rule needs none: a delete-pending destination that
cannot be made again is `Undecidable`, a refusal that keeps.
`the_snapshot_add_veto_attempts_again_only_over_an_empty_destination_it_can_remove`
calls it on each shape.

## `fn remake_destination(path: &Path) -> Again {`

Makes the destination again as the private directory `PendingGateWorkspace` made
(`create_private_dir`, mode 0700 on Unix) and answers `Again::Attempt`; a destination
it removed and cannot make again is `Again::Undecidable`.

## `fn sparse_checkout_is_refused_before_worker_spend()` › `run_git(&repo, &["update-index", "--skip-worktree", "README.md"]);`

Set these in separate commands: update-index applies only the final
mode option from one invocation. The combination guards against a
detector accidentally keying off assume-unchanged's presentation.

## `fn clean_detection_and_rollback()` › `let readme = fs::read_to_string(repo.join("README.md")).expect("read");`

core.autocrlf may legitimately restore CRLF on Windows checkouts.

## `fn branch_diff_commit_cycle()` › `let full = ws.head_sha_full().expect("full sha");`

What `resume` reads to recognise a commit as its own.

## `fn captured_candidate_keeps_one_parent_tree_and_diff()` › `fs::write(repo.join("README.md"), "second candidate\n").expect("second edit");`

Advance the index after capture, then prove the supplied tree still
materializes the first candidate rather than rereading that index.

## `fn open_normalizes_to_the_worktree_toplevel()` › `let expected = fs::canonicalize(&repo).expect("canonical repo");`

Compare canonically: temp dirs may be reached via a symlinked path.

## `fn capture_diff_is_immune_to_user_diff_config()` › `let set = |k: &str, v: &str| {`

Simulate a user with difftastic-style config and forced color.

## `fn opaque_git_diffs_are_rejected_before_review()` › `fs::write(repo.join(".gitattributes"), "hidden.rs -diff\n").expect("attributes");`

A candidate controls .gitattributes. Without --binary, marking a
source path -diff replaces all changed bytes with the tiny sentence
"Binary files differ", which a read-only reviewer cannot recover.

## `fn filtered_paths_are_refused_before_gates_and_review()` › `run_git(&repo, &["add", "-A"]);`

Preserve the independent post-stage guard for a caller opening an
index prepared outside Workspace::capture_candidate.

## `fn filter_on_unchanged_tracked_path_is_refused_before_materialization() {` › `fs::write(`

Only the attributes file changes. The filter target itself is absent
from `diff --cached --name-only` but is still a gate input.

## `fn capture_candidate_refuses_filter_before_candidate_helper_executes() {` › `run_git(&repo, &["add", "-A"]);`

Control: the exact raw command that capture used to run executes the
fixture, proving marker absence above is suppression rather than a
helper that could never run on this platform.

## `fn failed_gate_snapshot_add_cleans_registered_worktree()` › `ws.add_gate_worktree(path, hooks_path, commit)?;`

Model the dangerous failure boundary: Git has registered and
populated the worktree, then the overall add operation is
reported as failed (as a failing post-checkout hook did).

## `fn gate_snapshot_owner_helper()` › `let root = snapshot.workspace().root().to_path_buf();`

The snapshot's *identifier*, not its path. CODING_STANDARDS.md §12:
"a path is not safely a line: an ancestor may contain the delimiter,
or bytes that are not text at all. Send an identifier the receiver can
rejoin to a root it already knows." The parent supplied `store`, and
`create_in_store_inner` puts every snapshot at `<store>/worktrees/
<name>`, so the name is all the parent is missing -- and unlike the
path it is `upstroke-gates-<pid>-<ulid>`, which no ancestor can spoil.

## `fn gate_snapshot_owner_helper()` › `readiness::publish(&ready, &[name]).expect("publish the snapshot identity");`

Published last, and atomically. This used to be `fs::write` straight
to `ready`, which creates the name and then fills it -- so the parent,
which polls for the path and then reads it, could read nothing.

## `fn hard_killed_snapshot_owner_is_reclaimed_before_resume()` › `let mut owner = readiness::Producer::adopt(`

Adopted, so a panicking assertion anywhere below still terminates and
reaps this child rather than leaving it to sleep out its thirty
seconds holding a registered worktree.

## `fn hard_killed_snapshot_owner_is_reclaimed_before_resume()` › `let published = readiness::await_signal(&ready, owner.child(), Duration::from_secs(15))`

Producer-aware, and the bound is the one this test already used. The
wait it replaces polled only for the path, so an owner that died
before publishing -- a failed `Workspace::open`, a store the helper
could not create -- was reported fifteen seconds later as a producer
that had never published, which is the clock talking rather than the
death (CODING_STANDARDS.md §12).

## `fn hard_killed_snapshot_owner_is_reclaimed_before_resume()` › `let snapshot_path = store.join("worktrees").join(name);`

Rejoined to the root the parent already knew.

## `fn a_replaced_parent_leaves_the_captured_candidate_unchanged() {`

The witness `LEGACY-WORKSPACE-READS-REPLACEMENT-OBJECTS` names: one candidate,
captured with no replacement installed and again with
`refs/replace/<parent> -> <another commit>` in place, must be the same
`CapturedCandidate` -- branch, parent, tree and payload. Against this module's
code before `git_command` it fails, the payload reading `-two +agent-edit`
where the first capture read `-one +agent-edit` (measured).

The body runs in a child through `run_replacement_witness_child`, for the
reason every replacement witness in the crate does: a suite started with
`GIT_NO_REPLACE_OBJECTS` exported, or with `core.useReplaceRefs=false`
configured, would hand the code under test the protection this witness exists
to prove it sets. Before the second capture it asserts its own premise: a Git
child that does not refuse the refs reads the replacing commit wherever the
parent is named.

## `fn a_replaced_candidate_tree_materialises_as_recorded() {`

The checkout half. A gate snapshot of a tree that carries a replacement must
hold the tree its ephemeral commit records. Against the code before
`git_command` the snapshot held `replacing` (measured), under a commit that
records the other tree.

This is the producer's half only. A gate a runner starts in that snapshot gets
the environment the runner composes, and `HostRunner::for_legacy_workspace`,
which the v0.1 conductor installs, reads the recorded graph in this
repository for that reason (`ObjectGraph::RecordedIn`): a runner reading the
replaced graph over these recorded bytes failed `git diff --exit-code HEAD` on
a snapshot nothing had touched (measured).
`src/gates.rs`'s `a_v1_gate_judges_the_tree_its_own_workspace_materialised`
drives the two halves together.

## `fn test_module_span(code: &str) -> Result<(usize, usize), String> {`

Where the census's one untouched span begins and ends, in code with comments and
strings already blanked. `mod tests` must occur once, bounded on both sides by
something that cannot continue an identifier, directly under `#[cfg(test)]`. The
next token decides the rest: `{` is an inline body, taken through the brace that
balances it; `;` is a declaration whose body lives in another file, taken through
the `;` alone, since that file is not part of what the census reads; anything else
is an error. A shape the census cannot place is refused, never read.

## `fn the_census_reads_past_a_test_module_declared_out_of_line() {`

`test_module_span` over synthetic sources: an inline module (the code around it
stays, its body goes), a `#[path]` declaration followed by a helper that builds a
`Command` (the helper stays, so the census counts it), and four shapes it must
refuse — no body and no `;`, an unclosed body, no `#[cfg(test)]`, and two test
modules.

## `fn a_rollback_over_a_replaced_tree_restores_the_recorded_bytes() {`

The behaviour the census guards on the rollback path, witnessed directly: an edit
over a HEAD whose tree carries a replacement, then `discard_uncommitted`, must
leave the bytes HEAD records. Until round 4 only the census held that path.

## `fn the_recorded_objects_scope_names_the_repositorys_own_common_directory() {`

The scope's common directory is the canonical `.git`, the include is under the
given private root with its exact bytes and verifies, an altered include is
replaced by the next scope, a linked worktree of the same repository yields the
same entries, and no staged copy is left in the include's directory.

## `fn a_git_older_than_the_floor_is_refused_by_name() {`

The floor's parser over what `git version` prints: 2.40.x, Apple's 2.39.5 and
1.99.9 refused; 2.41.0, a Windows build, 2.43.0, a development `2.45.GIT` and
3.0.0 accepted; a version it cannot read refused. Each refusal names the floor
and repeats what Git printed.

## `fn execution_prerequisites_ask_git_its_version_and_refuse_2_40() {`

The floor at its real call site. A child runs `ensure_execution_prerequisites`
with a `git` first on `PATH` that answers `version` with `git version 2.40.0` and
hands everything else to the real Git, recording every argument list it is given.
The refusal must name the floor, the reported version and `merge-tree`, and the
record must hold the version question, so the refusal is known to come from
asking and not from something else failing. Unix-only: the stub is a shell
script.

## `fn every_git_child_of_this_module_is_built_where_replacements_are_refused() {`

What keeps the finding closed as the module changes. It reads the whole file,
with comments and string literals blanked by
`crate::effects::blank_comments_and_strings`, and takes one span out: the file's
one test module, from the `#[cfg(test)]` above `mod tests` to the brace that
closes an inline body, or to the `;` of a declaration whose body lives in
another file. Any other shape is refused rather than read (`test_module_span`,
its section below). It does not read `crate::effects::production_region`, which ends at
the first `#[cfg(test)]` in the file. Below a `#[cfg(test)] mod test_support {}`
placed at the end of production, a raw Git child without the pair, reached from
`parent_sha` or from `discard_uncommitted`'s `reset -q --hard HEAD`, passed the
census as it stood at `915c0646` (rows G4 and G5 below; the second review of
#326 found it). Nor does it read `crate::effects::production_code`, whose
blanking of `#[cfg(test)]` items another pull request is repairing. What it
keeps from `crate::effects` is the blanking, and it asserts the one property it
relies on: every byte stays at its offset, so the span it takes out is the test
module's.

It then asserts that `Command::new(` occurs once, inside `git_command`; that
`git_command` sets `NO_REPLACEMENT_OBJECTS`' pair and passes
`REPLACE_REFS_REFUSED`, whose value it reads directly, since the blanking
empties string literals; that `Command` is never
renamed with `as` and is named nowhere but the process import and `git_command`;
that `git_command` is named nowhere but its definition and its calls, and that
the functions calling it are, in source order, the fourteen it lists; and that
neither `env_remove(` nor `env_clear(` occurs. The list of functions at its top
is the positive control: a region cut short or blanked away fails there rather
than passing every count below.

The list of callers is what a new Git child moves. Every child of this module
comes from `git_command`, so a new one changes no `Command::new(` count, here or
in `runner::contract`'s census, where the builder took this file from fourteen
to one. It adds a function to that list instead, and whoever adds it names the
function there.

Each clause went red under a mutation, with the two witnesses above run beside
it (the census, then the capture and snapshot witnesses):

| mutation | census | capture | snapshot |
|---|---|---|---|
| none (control) | ok | ok | ok |
| a raw `Command::new("git")` in `head_sha_full` | FAILED | ok | ok |
| a fully qualified `std::process::Command::new("git")` in `branch_exists` | FAILED | ok | ok |
| `Command as Git` in the import list, used in `branch_exists` | FAILED | ok | ok |
| `use std::process::Command as Git;` on a line of its own, used in `branch_exists` | FAILED | ok | ok |
| `git_command` without the pair | FAILED | FAILED | FAILED |
| `.env_remove(NO_REPLACEMENT_OBJECTS.0)` in `git_output` | FAILED | FAILED | ok |
| `.env_clear()` in `branch_exists` | FAILED | ok | ok |

The five the witnesses cannot see are why the census exists: `rev-parse HEAD`
prints the raw id with or without a replacement, and `branch_exists` is on
neither witness's path.

Round 3 ran the census alone, and wrote each row's expectation before it ran
(`r3/` in the pull request's evidence). F4 and F5, the first attempt at the
review's shape, are not in the table: they put the block above `impl Workspace`,
and the positive control failed there under both versions of the test.

| mutation | at this head | as `915c0646` had it |
|---|---|---|
| none (control) | ok | -- |
| a new `git_command` call in `head_sha_full` | FAILED | -- |
| `git_command` taken as a value in `head_sha_full`, then called | FAILED | -- |
| G1: the block, then a raw child below it that `parent_sha` calls | FAILED | ok |
| G2: the block, then a raw child below it that the rollback's `reset -q --hard HEAD` calls | FAILED | ok |
| G3: the block alone | ok | -- |

Round 4, after the review of `6e3e618f` executed the shape below: the span ended
at the first `{` after `mod tests`, whether or not the module was inline. With
the test module moved out of line by `#[path]` and a helper that builds a raw
`Command` placed right after the declaration, that brace was the helper's, the
census blanked the helper, and routing `discard_uncommitted`'s reset through it
wrote the replacing bytes. The census with its span rule as `8be5540d` had it,
and as it is now, over that one shape (the rollback witness is new in round 4):

| shape | census, `8be5540d` | census, round 4 | `a_rollback_over_a_replaced_tree_restores_the_recorded_bytes` |
|---|---|---|---|
| none (control) | -- | ok | ok |
| the test module out of line, a raw helper after it, the rollback's reset through the helper | ok (blind) | FAILED: `Command::new(` counted 2 | FAILED: `replacing` on disk |

What it cannot see: a Git child that another module's code starts on this
module's behalf, or a `Command` value this file is handed without naming its
type. The production region calls no function of another crate module but
`crate::ulid::ulid`. What it reads as production and a compiler would not: a
`#[cfg(test)]` item outside the test module. One that builds a `Command` fails
the census rather than hiding from it.

## `enum Served {`

*B-W924-R1's witnesses, SYNTHETIC CONTROLS (the follow-up B record's §9.25).* Under the owner's
ruling B-W924-R1-S, **proposed** and effective only together with O8, which supersedes B-W924-R1 at
its three commands, they witness the tolerant access there instead (the follow-up D record's §5.20).
What a planted registration's `commondir` gives the next reader:
`Served::Torn`, zero bytes, or `Served::Whole`, `../..` and a newline.

## `fn make_fifo(path: &Path) -> std::io::Result<()> {`

`mkfifo` the command, so that the witnesses name no `libc` item for it.

## `fn serving_commondir<T>(`

Plants another checkout's registration's `commondir` as a FIFO and serves it:
each Git that opens it for reading blocks until the server has opened it for
writing, gets the next reading the script names, and reads end of file when the
server closes it. Git writes a regular file, never a FIFO; this is how a test
makes exactly the reads it names empty, and every later read whole, with no
timing. Each reading goes through a FIFO of its own: before a reader is
released, a fresh FIFO, or after the last reading a whole regular file, is
renamed over the path, so a reader that is slow to close its end cannot take
the next reading too, and no reader is left waiting on a FIFO nobody serves.
(The first version opened the same FIFO again for each reading, and in a loaded
suite one `git worktree remove` still holding its end took all three of the
cleanup witness's readings.) The four commands it serves each open another
registration's `commondir` once, on Git 2.43.0 and 2.55.0. When the act returns,
the server is stopped and, if it is still waiting for a reader, released by a
reader the test opens without blocking. A server that fails, by an error or by
a panic, releases every reader of its FIFO and puts the whole file in place
(`release_commondir_readers`) before its failure goes on: its own thread
catches the unwinding, releases, and resumes it, so the act's next Git reads
the whole file rather than waiting on a FIFO nobody serves, the act returns,
and the join carries the server's panic or error out. The act's panic is
carried out first. (Until the B9 round only an error released: a panic, such as
an `on_serve` callback's failed `expect`, closed the writer, the act's next
attempt opened the abandoned FIFO and waited for ever, and the join that would
carry the panic out was never reached. B-I8-2, the follow-up B record's §9.26.)
Linux-only, as its tests are: they assert the `Success` line, whose rendering
was executed only on Linux with glibc.

## `fn release_commondir_readers(commondir: &Path, whole: &Path) {`

The fixture's release, in an order meant to strand no reader: a reader opened
on the path without blocking, which is what lets the writer opened next without
blocking succeed; that writer, whose arrival wakes every Git blocked opening the
FIFO; the whole file renamed over the path, so every later open reads it; then
both closed, so every reader of the FIFO reads end of file, which Git takes for
a torn read and attempts again, now of the whole file. Whatever is at the path,
the FIFO the server was serving or the next one, it is released; if the whole
file is already in place the rename fails and is ignored. The order is reasoned
from Linux's FIFO semantics: the tests do not force a reader into the gap
between a writer's close and the rename, which the earlier order (a writer
opened without blocking, then the rename) left open.

## `const WEDGED_TEAR_FIXTURE: Duration = Duration::from_secs(30);`

TEST-HARNESS CONTAINMENT, not a bound of the code under test: how long a witness waits for an
act that a planted tear or a FIFO may have wedged before it releases the fixture itself and fails.
Thirty seconds, the value B-W924-R1's witnesses waited (three of its ten-second deadlines), kept
when B-W924-R1-S (proposed) withdrew that ruling's `REGISTRY_TEAR_DEADLINE` with its helper. A
healthy act ends long before it.

## `fn refusal_attempts(message: &str) -> usize {`

The attempt count a registry refusal names, read from its `<count> attempt(s)`. The witnesses of
the tolerant access at B-W924-R1's three commands compare it with `contended_attempts`, so that a
refusal is shown to follow attempts the access made again, not a single attempt.

## `fn a_snapshot_add_is_attempted_again_past_another_registrations_empty_commondir() {`

A durable snapshot added while another checkout's registration reads empty
once: the add's first attempt reads the tear and dies, the attempt after it
reads the registration whole, and the snapshot is made, registered, and
reclaimed by its drop. With the repair reverted, the add fails as the CI
witness did: `git worktree add failed: fatal: failed to read …/commondir:
Success`. Under B-W924-R1-S (proposed) the repair it
witnesses is the tolerant access at the add, and the test is unchanged.

## `fn a_snapshots_removal_and_its_list_are_attempted_again_past_another_registrations_empty_commondir()`

*Under B-W924-R1-S (proposed): B-W924-R1's witness, its script changed for the tolerant access.* A
snapshot's drop while another registration reads empty, whole, whole, then empty. The cleanup runs
the removal and the list that decides it as one attempt of the access: the first attempt's removal
reads the tear and its list the whole registration, which still lists the snapshot; the next
attempt's removal reads it whole and removes the snapshot, and its list, after that removal has
passed, reads the tear; the attempt after that finds the snapshot gone. The registration, the
directory, its hooks and its intent are all gone. With the cleanup's access returning its first
failure, the snapshot stays registered; with only the list's failure returned at once, the cleanup
stops before its intent is removed; with the list run once after the access, the list reads the
registration whole and the fourth reading is never taken. B-W924-R1's script, empty, whole, empty,
would show only the removal past a tear here: the list would read the registration whole both
times.

## `fn an_empty_commondir_of_the_snapshots_own_registration_is_attempted_again_until_the_deadline_and_refused()`

*Under B-W924-R1-S (proposed), in place of B-W924-R1's
`an_empty_commondir_of_the_snapshots_own_registration_is_returned_at_once`: a change O8 proposes,
not an equivalence.* The snapshot's own registration's `commondir` emptied. The cleanup's access
classifies nothing, so it attempts the removal and its list again until the access's nominal
deadline (500 ms in a test build, an admission rule checked after each attempt, with no hard elapsed
bound), and then refuses as a registry refusal naming the list's Git failure, which names the
registration. Each failed attempt was answered `Attempt`, and the refusal came no earlier than the
deadline. Restored, the snapshot is reclaimed. B-W924-R1 returned Git's failure at once: its own
registration is no other process's write in flight.

## `fn another_registrations_commondir_git_cannot_read_is_attempted_again_until_the_deadline_and_refused()`

*Under B-W924-R1-S (proposed), in place of B-W924-R1's
`another_registrations_commondir_git_cannot_read_is_returned_at_once`: a change O8 proposes, not an
equivalence.* Another registration's `commondir` a directory: Git dies reading it, `Is a
directory`, and the add's access attempts it again, over an empty destination it removes and makes
again each time, until the nominal deadline, then refuses as a registry refusal naming the add's
Git failure. The snapshot's cleanup after it meets the same directory and refuses too, which its
drop ignores. B-W924-R1 waited only on an empty read, and returned this at once.

## `fn another_registrations_empty_commondir_that_outlasts_the_deadline_refuses_naming_the_adds_error()`

*Under B-W924-R1-S (proposed), in place of B-W924-R1's
`another_registrations_empty_commondir_that_outlasts_the_deadline_returns_the_adds_error`.* Another
registration's `commondir` left empty. `add_gate_worktree`, called directly so that the snapshot's
cleanup after it is not timed with it, is attempted again, each time over the empty destination it
removes and makes again, and refuses as a registry refusal no earlier than the access's nominal
deadline (500 ms in a test build). The refusal names two attempts or more, each answered
`Attempt`, and as the last failure the add's own Git error naming the registration; the destination
is still an empty directory. Kept from B-W924-R1's test: the add's error named, attempts made again,
an empty destination, and no silent outcome. Changed, as O8 proposes: the outcome's kind, from the
add's Git error to the refusal, and the timing, from B-W924-R1's ten-second deadline with a
five-second allowance to the access's nominal deadline, an admission rule with no hard elapsed
bound, of which no upper limit is asserted. Bounded: if the add has not returned at
`WEDGED_TEAR_FIXTURE`, the test writes the `commondir` whole so that the add ends, and fails.

## `fn a_destination_no_longer_empty_refuses_at_once_naming_the_adds_first_error() {`

*Under B-W924-R1-S (proposed), in place of B-W924-R1's
`a_destination_no_longer_empty_returns_the_adds_first_error`.* The add's destination written to
while Git is blocked reading the tear: the add's veto finds a directory that is not empty and
refuses at once, after one attempt and with no `Attempt` answer, naming the add's first failure as
it was. Kept: the first failure, with no further attempt. Changed, as O8 proposes: the outcome's
kind, a registry refusal rather than the add's Git error. Run again, the add would fail on the
destination instead.

## `fn serving_commondir_under_containment(`

TEST-HARNESS CONTAINMENT, not the repair. Runs `serving_commondir` with a
snapshot as the act, beside a containment thread that waits for the act's end:
if it has not ended at `WEDGED_TEAR_FIXTURE`, the containment
releases the fixture's readers itself and puts the whole file in place, so that
the act's Git can end, every later read is whole and nothing is left waiting,
and reports that it had to. It carries its own copy of the release rather than
calling `release_commondir_readers`, so that a defect in the fixture's release
cannot disable it: a mutation of that function that skips the rename wedged
the witness for good while the containment called it (the B9 round's matrix).
The bound bounds a wedged fixture, not a healthy one: a healthy server's act
ends in milliseconds. It returns what `serving_commondir` carried out, the
snapshot's own result, and whether the containment released. The containment
thread is joined.

## `fn a_commondir_server_that_panics_releases_its_reader_and_its_panic_reaches_the_caller() {`

The B-I8-2 witness. The server's `on_serve` callback panics at the first
reading, after the add's Git has opened the FIFO: the add's first attempt reads
end of file and dies, and the server's own unwinding releases the FIFO and puts
the whole file in place, so the add's next attempt reads it and the snapshot is
made. The containment did not release, the panic that reaches the caller is the
callback's own, and the add succeeded. Before the repair the add's next Git
waited on the abandoned FIFO until the containment released it, thirty seconds
in, and the test fails on that.

## `fn a_commondir_server_that_fails_releases_its_reader_and_its_error_reaches_the_caller() {`

A control: an ordinary error of the server, a file planted where its next FIFO
would go, so its `mkfifo` fails at the first reading. The add's first attempt
reads end of file, the release puts the whole file in place, the add's next
attempt reads it, the containment does not release, and the caller receives the
server's error through `serving_commondir`'s `expect`, naming `mkfifo`.

## `fn a_commondir_server_that_serves_its_script_ends_without_the_containment() {`

A control: the same harness with a server that serves its one torn reading and
fails nothing. The containment does not release, the server reports the tear
it served, and the add goes on past it.

## `fn common_git_dir_of(repo: &Path) -> PathBuf {`

The canonical common git dir the way `canonical_common_dir` computes it, so a test
keys `crate::workspace_manager::contended_attempts` exactly as the access counts. It
runs the plain `git` of the test's own repository, not this module's builder, so the
tests compile and run against the module as master had it (their first-bad shape).

## `fn plant_a_torn_registration(repo: &Path, name: &str) -> PathBuf {`

A foreign registration torn the way a writer killed while it writes `commondir` leaves
it: `gitdir` written, as Git writes it (`GitdirRule::native().spelling`: `/` on
Windows, the path's own bytes elsewhere), and `commondir` opened and empty. Every
enumeration of the store dies on it ("failed to read …/commondir: Success" on glibc):
`git worktree add`'s sibling scan, `git worktree list`, `git worktree remove`, and
`git switch`'s `die_if_checked_out`. `finish_the_registration` is what its writer
writes last: `HEAD`, then `commondir`.

## `struct OnceContended<T> {`

The writer of a tear, on its own thread, gated by the access's handshake rather than
by time: it waits until `contended_attempts` for the repository exceeds its value
before the access began — the access failed on the tear and decided to attempt again
— and only then runs `finish`, returning how many attempts were contended and what
`finish` saw. A sixty-second watchdog bounds it, and `join` stops it once the access
has returned, so a mutation that never contends fails at its assertion rather than
wedging the suite.

## `fn a_snapshot_add_beside_a_tear_its_writer_finishes_is_attempted_past() {`

T-L1. A snapshot add beside a tear its writer finishes after the add's first failed
attempt succeeds, and the snapshot holds the candidate. The writer looks at the
destination before it finishes the tear: one destination, the empty directory the
snapshot made, because the add fails in its sibling scan before Git takes the
destination over and the veto made it again. Red at master (the add's Git error at
once) and under the mutation that makes the veto refuse over an empty destination.

## `fn a_snapshot_add_beside_a_tear_that_stays_refuses_as_the_registrys_and_leaves_its_intent() {`

T-L2, as split by FUD-D2-TL2: only what is portable. A tear that stays: the add
refuses as `UpstrokeError::RegistryRefused`, never Git; it was attempted again at
least once; the refusal names the store and its deadline; and the snapshot's intent is
left for the reclaim, which takes it once the operator removes the residue. No attempt
count is asserted: one attempt slower than the deadline is the only one, correctly.
The final-attempt rule is follow-up B's to test, apart from real Git.

## `fn a_snapshot_whose_checkout_cannot_be_made_refuses_after_one_attempt_and_leaves_nothing() {`

T-L3. A candidate no checkout can make — a `.git/` path, on every platform, and on Unix
a 300-byte name — fails after Git took the destination over, and Git's junk removal
takes the destination. The veto finds it gone and answers `Undecidable`, so the add
refuses after exactly one attempt (the handshake's count does not move), naming why,
and nothing is left at the destination or registered. Red under the `Return` mutation
(Git state, the round-2 veto's answer) and under the always-`Attempt` mutation (the
checkout attempted again until the deadline).

## `fn a_snapshot_removal_beside_a_tear_its_writer_finishes_takes_the_registration() {`

T-L4. A snapshot's drop beside a tear its writer finishes after the removal's first
failed attempt: the snapshot's directory, registration and intent are gone. It also
distinguishes round 4's B1′, which ran the removal once with its status ignored and
wrapped the list apart: there the first removal fails, the tear finishes, the separate
list then succeeds and still registers the snapshot, and the drop leaves it.

## `fn a_branch_switch_beside_a_tear_its_writer_finishes_switches() {`

T-L5. `switch_branch` beside a tear its writer finishes after the first failed attempt
switches, and a failed attempt changes neither `HEAD` nor the checkout. It runs the
access twice. The first meets the tear unrepaired and refuses as
`UpstrokeError::RegistryRefused`, having attempted again; `HEAD` and the status are read
after it returns, with no access running, because the access's deadline would time any
Git child read between two of its attempts (standards §12: a deadline bounds a wedged
producer, it does not time a healthy one). The second access's writer finishes the tear
on the handshake, with two file writes, and the switch succeeds. Red at master, where
the switch's Git error returns at once, and when the tear is finished before the second
access, which then never fails on it.

## `fn every_git_child_of_this_module_runs_with_automatic_maintenance_off() {`

T-L7, R-G's regression. A repository configuring `maintenance.auto=true`,
`gc.auto=6700`, `gc.autoDetach=true` and `maintenance.autoDetach=true`: through the
builder, `git config --get` of each reads `false`, `0`, `false` and `false`, because a
command-line setting outranks the repository's. It executes Git, so it holds on every
Git at or above the 2.41 floor. Red at master and with the four settings dropped from
the builder.

## `fn the_snapshot_add_veto_attempts_again_only_over_an_empty_destination_it_can_remove() {`

T-L8. The veto called directly, with no Git: an empty destination is `Attempt` and is an
empty directory again; one gone is `Undecidable` and nothing is made there; one holding
a file, a file in its place, a link to an empty directory (Unix), and an empty one
under a parent nothing can write (Unix) are `Undecidable`, and each is left as it was.
The read-only parent's case refuses to run, rather than passing vacuously, where the
mode bit does not bind (root, or `CAP_DAC_OVERRIDE`).

## `fn a_kept_pin_is_written_whatever_head_is_and_a_publication_pin_is_not() {`

T-K1 (R-D1's part K): one capture, then `HEAD` moved, detached, on another branch, its branch
deleted, or its branch ref made symbolic. A pin at a `-kept` name is written on the captured
parent with the captured tree; a pin at a publication name refuses. Red under `k-headcheck`,
under `m-k-observe-only` (detached, deleted and symbolic rows) and, for the publication half,
under `m-k-all`.

## `fn the_private_index_notes_tie_an_orphan_shared_index_to_the_checkouts_expiry() {`

The pin of D-I2-1 (#331's review round i2, regression lens, executed on Git 2.43.0 in ordinary
and linked checkouts). It reads these notes, finds the `PRIVATE_INDEX_CONTROLS` section by its
heading, collapses its whitespace, and asserts that it states what removes an orphan shared index
file — a later split write of the checkout's own index that creates a shared index file, once
the orphan is older than the checkout's `splitIndex.sharedIndexExpire` — with the default's
window, `never`'s none, the accumulation and the variable's O3 class, and that the retired claim,
that Git's next split write of the checkout's own index removes the orphans, is absent. Notes,
not the record: the package excludes `reviews/` and `findings/`. Git's behaviour is unchanged,
so no behaviour test can guard the sentence; the reviewer's sequence, executed again on Git
2.43.0 and 2.55.0, is the evidence. Red on round 4's notes.
