# `src/runner/host/environment.rs`

Extended notes for [`src/runner/host/environment.rs`](../../../../src/runner/host/environment.rs).

The code is the authority for what it does; this file is the whole of its prose, moved out of
the source verbatim. Each section is headed by the line of code the comment sat above, spelled
as it is in the source, so the heading is the grep string that finds the code.

## Module

`host-v1`'s environment contract: the platform's name rule, and the composition of base,
reserved values and overlay (`design/08_design_trait_surface.md:65-76`).

The vocabulary it composes over -- `RESERVED_ALWAYS`, `CREDENTIAL_LOCATIONS`, `reserved_keys`,
`WITHHELD_ALWAYS` and `supplies_credentials` -- stays in the parent, `src/runner/host.rs`, because
that is the one list `runner::container::env` also reads. This module decides which variables a request gets and in
what order, and performs no effect: the base is handed in.

## `#![forbid(`

This child states its own lint level. A Rust lint level is scoped by the module tree, not the
file, so without it this module would inherit `src/runner/host.rs`'s
`#![allow(clippy::disallowed_types)]`. Nothing here reaches a governed primitive, so all three
lints are forbidden and the module has no `effects/allowlist.toml` row.
`runner::container::tests::every_child_module_of_the_container_funnel_states_its_own_lint_level`
walks `src/runner/host/` and checks it.

## `pub enum KeyCase {`

How the platform compares environment variable names.

A type rather than a `cfg!` at each comparison: `cfg!(windows)` is false on a Linux box and on the
Linux CI cell, so a `cfg!` rule's Windows arm would be reachable by no test there. `Self::ALL` is
what the grids run over; `Self::current` is what production selects. `crate::topology::effects::Host`
has the same shape for the same reason.

## `pub enum KeyCase` › `Sensitive,`

Unix: `Path` and `PATH` are two variables.

## `pub enum KeyCase` › `Insensitive,`

Windows: `Path` and `PATH` are one variable, and a child that received both would get whichever
the block listed last.

## `impl KeyCase` › `pub const ALL: &'static [Self] = &[Self::Sensitive, Self::Insensitive];`

Both rules. Every grid runs over this, not over `Self::current`.

## `impl KeyCase` › `pub const fn current() -> Self {`

The rule this machine's process environment obeys.

## `impl KeyCase` › `pub fn same_key(self, left: &OsStr, right: &OsStr) -> bool {`

Whether two names are the same variable under this rule.

## `pub enum ObjectGraph {`

Which object graph the Git children of a composed environment read, and where; the one thing
about it a conductor chooses.

`Recorded` is the default: the objects the repository holds, never what `refs/replace/*` points
them at (`design/15`, "What an exact snapshot is exact against"). For it `compose` writes
[`NO_REPLACEMENT_OBJECTS`](../../../../src/workspace_manager.rs) process-wide, so every repository a
child touches loses its replacements, including one a gate creates for itself. That is
`PR326-SCHEMA4-ROLE-ENVIRONMENT-REFUSES-REPLACEMENTS-EVERYWHERE` (P2), and it is why neither
conductor reads it: both build their host runner on `RecordedIn` (`HostRunner::for_legacy_workspace`,
`engine::topology::entry`).

`RecordedIn` is the conductors' reading: the recorded graph in the repository the run manages,
and whatever the configuration says elsewhere, as far as a condition on the spelling of a Git
directory can tell the two apart. Its section says how, and where spelling and filesystem
disagree.

`AsReplaced` composes nothing, so a child reads whatever its base says about `refs/replace/*`. No
conductor installs it. It is a test instrument for witnesses that must show a replacement is
live in a role's environment (`src/workspace_manager/tests.rs`) or that reading the replaced
graph over a snapshot written from the recorded one fails (`src/gates.rs`).

A consumer must read the graph its producer wrote. The v0.1 workspace's own Git children refuse
replacements (`src/workspace.rs`'s `git_command`), so its roles read the recorded graph too.

It is a field of the environment rather than of the request because it belongs to the
conductor -- one schema per run, chosen once where the runner is built -- and because defaulting
it here gives a new spawn site the isolated reading without asking.

## `pub enum ObjectGraph` › `RecordedIn(ManagedRepository),`

The recorded graph in the repository `ManagedRepository` names, through two conditional includes
`compose` appends to `GIT_CONFIG_PARAMETERS`.

- **Not the process-wide variable**, because a variable does not know which repository it is
  for: a gate that ran `git init`, `commit` and `git replace` in a fixture of its own would read
  the original object and fail. And on Git 2.41 a configured `core.useReplaceRefs = true` beats
  the variable, so the producer would write recorded bytes while the roles read replacements.
- **Not the repository's own configuration.** A v0.1 run manages the operator's checkout and
  every gate snapshot is a linked worktree of it, so `--local` would write the operator's common
  `.git/config`, visible from unrelated worktrees and outliving the run; and a `config.worktree`,
  an inherited `GIT_CONFIG_COUNT` or `GIT_CONFIG_PARAMETERS` outranks that file anyway.
- **Not `GIT_CONFIG_COUNT` pairs:** Git reads `GIT_CONFIG_PARAMETERS` after the counted pairs, so
  an inherited `'core.useReplaceRefs'='true'` there would win. Appended to
  `GIT_CONFIG_PARAMETERS`, after every entry the base or overlay carries, the includes are the
  last configuration a role's Git reads, except a `-c` given to that Git command itself.

Measured with plain Git 2.40.0 to 2.43.0, with `true` in the system, global, repository and
worktree files, in `GIT_CONFIG_COUNT` and in inherited `GIT_CONFIG_PARAMETERS`: the managed
repository's main and linked worktrees read the recorded object, and fixture repositories inside
and outside the linked worktree kept their replacements, under a path holding spaces and
`[x]*?`. On 2.40.0 `git merge-tree` over a replaced commit read the replacement even with
`-c core.useReplaceRefs=false`; from 2.41.0 it read the recorded commit. That is why the Git
floor is 2.41 (`src/workspace.rs`'s `require_git_floor`).

**Which repository the conditions reach is decided by what each directory does, not by the
platform.** `gitdir/i:` folds every component, so on a case-sensitive directory it also reaches
a repository whose path differs from the managed one only in case, whose gates then read the
recorded object where it installed a replacement. Each component is instead matched with case
unless the directory holding it finds it under the other case, and that includes the
`worktrees` component of the linked worktrees' condition: where the common directory folds, a
gate naming a linked worktree's Git directory `<common>/WORKTREES/<id>` must still read the
recorded object. `ManagedRepository::new`'s section says how this is measured and what it costs.

**What it does not do.** A role's own `git -c core.useReplaceRefs=true` comes after the includes
and wins, and a role that clears its environment before running Git loses them. Configuration is
not an enforcement boundary, and the policy claims none.

**Where the conditions reach further, or less far, than the managed repository.** Both are
measured; neither is repaired:

- **Further, through an alias.** A case-keeping directory holding an alias whose name is a
  component of the managed path with every ASCII letter's case swapped, and which resolves to
  that component, is judged to fold case: a junction on Windows (which `std::fs::canonicalize`
  follows) or a bind mount on Linux (whose root has the component's device and inode). The
  component is then spelled in classes, and the conditions also reach a sibling repository whose
  name differs only in case, whose roles read the recorded object where it installed a
  replacement. A Unix symbolic link of the same name does not do this.
  `PR326-A-JUNCTION-MAKES-A-CASE-SENSITIVE-DIRECTORY-READ-AS-FOLDING` (P2, deferred).
- **Less far, through a spelling the classes do not cover.** Only ASCII letters are spelled in
  classes (`matched`). On a case-folding ext4 directory, a Git directory named with a non-ASCII
  letter in its other case (`CAFÉ` for `café`), in the other Unicode normalization, or with a
  character the filesystem folds onto an ASCII letter (`worKtrees` with the Kelvin sign,
  `worktreeſ` with the long s) names the managed repository and reads the replaced object; a gate
  doing so over a snapshot holding the recorded bytes returns `Fail`. The scope matches a
  spelling while the filesystem decides which spellings name one directory, so the fix is a
  decision about repository identity, not another character class.
  `PR326-A-SPELLING-THE-FILESYSTEM-EQUATES-READS-REPLACEMENTS-IN-THE-MANAGED-REPOSITORY` (P1,
  deferred).

Undoing any of these decisions -- the process-wide variable instead of the includes, the
includes before the inherited entries or before the overlay, no `/worktrees/*` condition, one
`<common dir>/` condition instead of two, no glob or quote escaping, no include written, no
check before a role, no Git floor -- fails a witness in `src/engine/tests.rs`,
`src/runner/host/tests.rs`, `src/workspace.rs` or `src/gates.rs`. The gate witness,
`a_v1_gate_judges_the_tree_its_own_workspace_materialised`, pins `true` in the repository, so on
Git 2.41 it also fails under the process-wide variable.

## `pub const CONFIG_PARAMETERS: &str = "GIT_CONFIG_PARAMETERS";`

The variable Git reads command-line configuration from and hands its own children (`git -c`
appends to it). Undocumented by `git-config(1)`, and read by every Git since 2.31 in the
`'key'='value'` form the includes are written in.

## `pub const RECORDED_OBJECTS_INCLUDE: &[u8] = b"[core]\n\tuseReplaceRefs = false\n";`

The whole include file, byte for byte. The runner compares the file with it before every role it
starts, so a file holding anything else, nothing, or missing refuses the role. Without the check
a missing include would be silent: Git skips an include whose file does not exist.

## `pub enum GitdirRule {`

How a canonical path is spelled for an `includeIf.gitdir` condition on this platform. A type
rather than a `cfg!`, for the reason `KeyCase` is one: `ALL` lets a grid on one machine cover
every rule.

It decides the spelling only. Whether a component is matched with case is not a property of the
platform -- a macOS volume or a Windows directory can be case-sensitive, and a Linux directory
can fold case -- so `ManagedRepository::new` asks each directory on the path.

## `pub enum GitdirRule` › `Posix,`

Every Unix target, macOS included: the path's own bytes.

## `pub enum GitdirRule` › `Windows,`

Git for Windows compares the realpath `GetFinalPathNameByHandleW` gives, with its `\\?\` prefix
removed, `UNC\` turned into `//`, and every backslash a slash. `std::fs::canonicalize` is the same
call, so its result spelled that way is Git's own text. The prefix (a drive letter, or a UNC
server and share) is matched as spelled, and each component below it by what the directory
holding it does.

## `impl GitdirRule` › `pub const fn native() -> Self {`

The spelling this machine's Git uses.

## `impl GitdirRule` › `pub const fn keyword(self) -> &'static str {`

The condition's keyword: `gitdir:`, under every rule.

Not `gitdir/i:`, which folds every component whatever its directory does: on a case-sensitive
directory it also matches a repository whose path differs from the managed one only in case, and
keeps that repository's gates off the replacements it installed
(`a_repository_whose_path_differs_from_the_managed_one_only_in_case_keeps_its_replacements`).
Where a directory does fold, the component is spelled with both cases of each ASCII letter,
which reaches it in any ASCII case and reaches no other component.

## `impl GitdirRule` › `pub fn spelling(self, path: &[u8]) -> Vec<u8> {`

The path as Git spells it under this rule, before any escaping.

## `pub struct ManagedRepository {`

The repository a v0.1 run manages, as `compose` names it to Git: its canonical common directory,
the include file, and the two `GIT_CONFIG_PARAMETERS` entries they make, built once so every role
gets the same bytes.

## `impl ManagedRepository` › `pub fn new(common_dir: &Path, include: &Path, rule: GitdirRule) -> Result<Self, UpstrokeError> {`

The entries, for a canonical common directory `C`:

```
'includeIf.gitdir:C.path'='<include>' 'includeIf.gitdir:C/W/*.path'='<include>'
```

`W` is `worktrees`, spelled as `C` matches names.

- **Matched as each directory matches.** For each component of `C`, `folds_case` asks the
  directory holding it whether it finds that name under the other ASCII case. Where it does,
  each ASCII letter is spelled as a class of both cases (`Repo` becomes `[Rr][eE][pP][oO]`);
  where it does not, the component is matched exactly. So a path Git reaches through a folding
  directory in another case still names the managed repository, and a case-sensitive sibling
  whose name differs only in case does not. A Windows prefix is matched as spelled, and so are
  non-ASCII letters, which Git's own `gitdir/i:` does not fold either.
- **`worktrees` as `C` matches names.** `C` looks it up, so it is spelled in classes where `C`
  finds `refs` under the other case and exactly where it does not. `refs` is asked rather than
  `worktrees` itself because Git creates `worktrees` with the first linked worktree, usually
  after the scope is built, and `git worktree prune` removes it when empty, while every common
  directory Git accepts holds `refs` (Git's `is_git_directory` requires it). Not `HEAD`: Git
  rewrites it by renaming a lock file over it, so the two lookups could meet two entries.
- **Two conditions.** `C` is the main worktree's Git directory and `C/W/*` every linked
  worktree's, gate and review snapshots among them. `*` does not cross a `/`, so a submodule's
  Git directory under `C/modules/`, and anything deeper, is not matched. A bare `C/` would match
  both.
- **Escaped.** The condition is a wildmatch pattern, so each `[`, `]`, `*`, `?` and `\` of `C` is
  escaped, in folding components too; the only globs are the linked worktrees' `*` and the case
  classes. Unescaped, a `[` silently matches nothing and a `*` matches other repositories.
- **Quoted.** Each key and value is single-quoted, a quote inside written `'\''`, which is what
  Git's `sq_dequote` reads.
- **Refused, never approximated:** a relative common directory or include, which Git rejects from
  the command line; a newline in `C`, which Git rejects in a key; on Windows a path that is not
  valid Unicode, which cannot reach Git for Windows through its environment; a `.` or `..`
  component, which the realpath Git matches never holds; and a component whose directory cannot
  be asked, or a `C` that cannot be asked about `refs`, which leaves no way to tell whether to
  match with case. A pattern that silently failed to match would put the roles back on the
  replaced graph unannounced, and one that matched too much would take another repository's
  replacements away.

**Cost.** Two lookups per component holding an ASCII letter, and two for `refs` in `C` (`lstat`
on Unix, `std::fs::canonicalize` elsewhere), once per run and once per resume:
`Workspace::recorded_objects_scope` builds the scope there and no role repeats it. Measured in a
release build, a few microseconds for a typical path and about ten for one of twelve components.

**A failed lookup.** Any error but "not found" refuses the run before its first role, naming the
path and the error (`UpstrokeError::Refused`, from `run` and `resume` alike); nothing falls back
to either answer. On a folding directory the other spelling names the entry just found; on a
case-sensitive one it is not found unless a sibling bears exactly that spelling, which the
identity check tells apart, or an alias resolves it to the entry itself, which it does not (the
`RecordedIn` section). A directory missing from the path reaches the refusal
(`the_includes_refuse_a_common_directory_git_cannot_read_in_a_key`), as does a `C` with no `refs`,
which Git would not accept as a common directory; so would, by reasoning, an I/O error or on
Windows a case-sibling the process may not open. A directory removed between the two lookups
does not: its second lookup answers "not found", and the component is matched exactly.

## `impl ManagedRepository` › `fn matching(`

`Self::new` with the lookup supplied, so the inline tests can pin the entries for any answer
without a directory to measure. It walks `C` once, root first, asking about each normal
component with the path of the directory that holds it; then it asks `C` about `refs`, for
`worktrees`.

## `impl ManagedRepository` › `pub fn verify_include(&self) -> Result<(), UpstrokeError> {`

Whether the include holds `RECORDED_OBJECTS_INCLUDE` exactly. The runner asks before every role
it starts and does not start one on `Err` (`ProcessFate::NeverStarted`). A read, not an effect:
`Workspace::recorded_objects_scope` writes the file, and every run and resume calls it before
their first role.

## `fn folds_case(parent: &Path, name: &OsStr) -> Result<bool, String> {`

Whether `parent` finds `name` under the other ASCII case: it looks up `name` with every ASCII
letter's case swapped and compares the result with `name`. The same entry means the directory
folds; "not found" or a different entry means it keeps case; any other error is the caller's
refusal. A name with no ASCII letter has no other spelling and is matched exactly.

## `fn in_the_other_case(name: &OsStr) -> Option<OsString> {`

`name` with every ASCII letter's case swapped, or `None` when it has none.

## `const fn other_case(byte: u8) -> u8 {`

The other ASCII case of a letter; any other byte unchanged.

## `fn same_entry(entry: &Path, other: &Path) -> Result<bool, String> {`

Whether two spellings name one entry. On Unix by `(st_dev, st_ino)`, without following a final
symbolic link: Linux's realpath does not correct case, so comparing canonical paths would call a
folding directory case-sensitive. Elsewhere by `std::fs::canonicalize`, which on Windows returns
the name as the directory stores it; the standard library's Windows file identity,
`MetadataExt::file_index`, is unstable (E0658, `windows_by_handle`). A second spelling that is not
found is `false`; any other error on either lookup is the caller's refusal.

Neither comparison tells an alias from the entry itself: `canonicalize` follows a junction or
symbolic link on Windows, and a bind mount's root carries its source's device and inode on Linux,
so an alias spelled in the other case answers `true` in a directory that keeps case
(`PR326-A-JUNCTION-MAKES-A-CASE-SENSITIVE-DIRECTORY-READ-AS-FOLDING`). A Unix symbolic link does
not, since `lstat` reads the link itself.

## `fn matched(into: &mut Vec<u8>, spelled: &[u8], folded: bool) {`

One component of the condition: each ASCII letter as a class of both cases when the component
folds, and every wildmatch syntax character escaped with a backslash.

## `fn single_quoted(text: &[u8]) -> Vec<u8> {`

POSIX single quoting, as Git's `sq_quote_buf` writes it.

## `pub struct HostEnvironment {`

`host-v1`'s environment contract. It holds its base explicitly so a test can compose against a
base it wrote rather than whatever is set on the machine running the suite.

## `impl HostEnvironment` › `pub fn from_process() -> Self {`

The Upstroke process environment, under this platform's name rule.

## `impl HostEnvironment` › `pub fn with_base(base: Vec<(OsString, OsString)>, case: KeyCase) -> Self {`

An explicit base, for grids that must cover both name rules.

## `impl HostEnvironment` › `pub fn reading(mut self, objects: ObjectGraph) -> Self {`

The object graph this environment's children read. Owned by whoever builds the runner, never by
an adapter or an overlay.

## `impl HostEnvironment` › `pub const fn objects(&self) -> &ObjectGraph {`

Which graph is in force, so a witness can assert what a conductor installed rather than infer it
from a composed vector.

## `impl HostEnvironment` › `pub fn base(&self) -> &[(OsString, OsString)] {`

The base this runner composes from.

## `impl HostEnvironment` › `pub const fn case(&self) -> KeyCase {`

The name rule in force.

## `impl HostEnvironment` › `pub fn reserved_values(`

The reserved values the runner supplies for this request.

A reserved key the base does not carry is **not** supplied, except the credential profile below,
which is the request's value rather than the base's. Setting an absent variable to the empty
string is a different environment from not setting it, and several CLIs read "set but empty" as
an instruction.

`profile` is the request's credential profile
([`RunnerRequest::credential_profile`](../../../../src/runner/contract.rs)) and changes one value:
where a role that `supplies_credentials` is handed its bound agent's credential-location key, a
`Some` profile is the value, in place of the base's (the coordinator's own), whether or not the
base carries the key. `None` means the base's value or nothing. Nothing else reads `profile`: role
scoping decides whether the key is supplied at all, so a gate or shell probe carrying a profile
is told no credential location, and an agent with no known location (`credential_location`
returns `None`) is handed nothing under any key (`design/26_design_merge_queue_protocol.md:456-463`).

`design/08_design_trait_surface.md:65-68` says the host runner starts from the Upstroke
environment and "each supplies role-scoped `HOME`, `PATH`, and credential locations". For
`host-v1` the split is deliberate:

* **Credential locations are role-scoped**, by `supplies_credentials`. A gate is
  repository-controlled code and the shell probe is a shell; neither runs an agent CLI, so
  neither is told where an agent's credentials live, whatever agent the request names.
* **`HOME`, `PATH` and `USERPROFILE` are supplied to every role at the host boundary's own
  value.** "One machine, one user" is a rationale, not a basis; the basis is three passages, each
  forbidding a different part of a per-role value:

  1. `design/08_design_trait_surface.md:69-71` -- "Probe and execution compose the same base,
     mounts, reserved values, and overlay, so pre-flight certifies the environment that will
     actually spend". A `HOME` differing between `probe(<agent>)` and `implement` or `review`
     would certify an environment the attempt never runs in.
  2. `design/26_design_merge_queue_protocol.md:472-473` -- "gate-shell/program availability is
     checked inside the same boundary." A `PATH` differing between `probe(shell)` and `gate`
     would certify a different program from the one that runs.
  3. `design/26_design_merge_queue_protocol.md:483-484` -- "Host runner behavior remains
     available and honestly provides no OS boundary around gate code." A different `HOME` for
     gate code would assert an isolation the host lacks: repository code can read the real home
     directory by absolute path either way. What the host can honestly do is not disclose a
     location it would otherwise hand over, and that is `supplies_credentials`.

  The value comes from the base, not from anything this runner invents, because
  `design/26_design_merge_queue_protocol.md:441-442` puts the base there: "The host base starts
  from the Upstroke process environment". A process environment carries one value per key under
  `KeyCase`, so one value is what a correct `host-v1` produces, not a narrowing. The container
  runner differs because each role's container is its own filesystem, not because its `HOME`
  string differs per role.

  `the_reserved_values_every_role_gets_are_the_host_boundarys_own` asserts this per passage, so a
  `host-v1` that scopes `HOME` has to change a passage first.

## `impl HostEnvironment` › `pub fn compose(`

Base, then reserved values, then overlay: the order of "the same base, mounts, reserved values,
and overlay" (`design/08_design_trait_surface.md:69-70`).

The base's own copies of the **reserved** keys are dropped before the runner supplies them, which
makes "role-scoped" a property of the child's environment. Cloning the base and upserting would
leave every credential location the Upstroke process carries in a gate's environment -- a gate is
repository-controlled code, and `CODEX_HOME` reaching it is what `supplies_credentials` exists to
prevent -- and would make the supply step output-equivalent to deleting it, since
`Self::reserved_values` reads its values from the same base. So reserved keys arrive from the
role-scoped supply step or not at all.

The names in `WITHHELD_ALWAYS` are dropped from the base in the same way, and nothing supplies them,
so they reach a child only if its own overlay names them. That list holds the reset reader's
dedicated bearer, which no child reads: the reader sends it to `curl` on standard input. Without
the drop, a Codex probe or a gate inherited the bearer the operator exported
(`PR441-RESET-BEARER-REACHES-OTHER-CHILDREN`). `Self::base` still returns the base unchanged.
Only the composed vector omits them.

Every write after the base goes through `upsert`, which leaves one entry per name. So a value the
supply step, the overlay or the object graph writes is the value the child starts with, even when
the base carried that name more than once.

`profile` goes to that step unchanged (`HostRunner` passes the request's `credential_profile`).
It arrives through the supply step, not the overlay, so the overlay's refusal of a reserved key is
unchanged: an overlay naming `CLAUDE_CONFIG_DIR` is refused whether or not the request carries a
profile.

Then the object graph, **after** the overlay. Under `ObjectGraph::Recorded`,
[`NO_REPLACEMENT_OBJECTS`](../../../../src/workspace_manager.rs). Under `ObjectGraph::RecordedIn`,
the conductors' reading, the repository's two includes appended to whatever
`GIT_CONFIG_PARAMETERS` the base and overlay left, separated by one space, or alone when there is
none: Git refuses a leading space there. `AsReplaced` adds nothing. `HostRunner` clears the
ambient environment and installs this result, changed only in the Git work-tree selectors and
`GIT_COMMON_DIR` for a role outside the started checkout, so a setting not composed here reaches
no child. Without it a gate or reviewer inside an exact snapshot would read whatever `git replace`
points the judged objects at; measured on Git 2.43, `git show HEAD:f` returned the replacement and
`git status --porcelain` called an untouched snapshot modified. `design/15`'s "What an exact
snapshot is exact against" is the product rule; `NO_REPLACEMENT_OBJECTS` is the one constant named
at each boundary that starts a child able to run Git over a snapshot this engine produced, and at
the builder every Git child of the v0.1 workspace starts from (`src/workspace.rs`'s
`git_command`).

It is **asserted, not reserved**. Reserved keys are values this boundary reads from its host and
re-supplies role-scoped, which is why they are stripped from the base and why `preflight` refuses
an overlay that restates one: a gate permitted to set `PATH` is a hijack. The object-graph setting
is a constant the runner states; there is nothing in the base to re-supply, an overlay restating
it is a no-op, and refusing it would add a failure mode without adding a guarantee. The ordering
supplies the guarantee: last write wins, and this is the last write. Git 2.43 reads the presence of
`GIT_NO_REPLACE_OBJECTS` rather than its value (`=0` and `=false` both disable replacement), so
`1` is right under either reading, and the ordering keeps an overlay from re-enabling replacement
under a future Git that reads the value.

### Errors

`UpstrokeError::Refused` naming the key when the overlay names a reserved one: "An adapter that
conflicts with a runner-reserved environment key is a pre-flight error rather than an
order-dependent override" (`design/26_design_merge_queue_protocol.md:464-465`). It is refused by
**key**, not value: an overlay allowed to restate `PATH` because the value happens to match breaks
silently the day the runner's value changes.

## `impl HostEnvironment` › `pub fn preflight(&self, overlay: &[(String, String)]) -> Result<(), UpstrokeError> {`

The reserved-key refusal on its own, so a caller can certify an overlay without building an
environment.

### Errors

`UpstrokeError::Refused` naming the offending key and the reserved key it collides with.

## `fn upsert(into: &mut Vec<(OsString, OsString)>, case: KeyCase, key: OsString, value: OsString) {`

Sets `key` to `value`, matching names by `case`. The first entry of that name keeps its place and
its spelling and takes the value. Every later entry of that name is removed. With none, the pair is
appended.

The removal matters because a base can name one variable twice. `std::env::vars_os` yields every
entry of a Unix process environment, duplicates included, and on Windows two spellings of one name
can arrive in an explicit base. The runner hands the result to `Command::envs`, where a later entry
of a name replaces an earlier one. Rewriting only the first entry left the second in force. With
two marked entries in a synthetic Unix base, the subscription probe's blank override of the reset
bearer blanked the first, and the child started with the second's value
(`PR441-BLANK-OVERRIDE-MISSES-DUPLICATE-ENTRY`, P2). An override now decides the value whatever
the base holds. `tracker::tests::a_blank_override_empties_every_same_named_entry_in_the_base`
pins it, for the bearer's name and for another name, under both key rules.
