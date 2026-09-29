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

## `impl Workspace` › `pub fn prepare_commit_from_candidate(`

Prepare and pin a commit from the exact candidate identities already
used by gates and review. This never rereads the mutable index.

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

## `let _ = fs::remove_dir_all(path);`

`worktree remove` normally removes the directory too. Once Git confirms
no registration remains, these exact private paths are safe to remove
even if a partially failed add populated only part of either one.

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
