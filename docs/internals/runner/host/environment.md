# `src/runner/host/environment.rs`

Extended notes for [`src/runner/host/environment.rs`](../../../../src/runner/host/environment.rs).

The code is the authority for what it does; this file is the whole of its prose, moved out of
the source verbatim. Each section is headed by the line of code the comment sat above, spelled
as it is in the source, so the heading is the grep string that finds the code.

## Module

`host-v1`'s environment contract: the platform's name rule, and the
composition of base, reserved values and overlay (DESIGN.md:258-264).

The vocabulary this composes over -- `RESERVED_ALWAYS`,
`CREDENTIAL_LOCATIONS`, [`reserved_keys`] and [`supplies_credentials`] --
stays in the parent, which is "the one list" `runner::container::env` reads.
What lives here is the rule that decides *which* variables a request gets
and in what order, and it performs no effect: the base is handed in.

## `#![forbid(`

**This child states its own lint level and inherits nothing.** A Rust lint
level is scoped by the module tree and not by the file, so an out-of-line
child of `src/runner/host.rs` would otherwise inherit that file's inner
`#![allow(clippy::disallowed_methods, disallowed_types, disallowed_macros)]`
-- `PR6-LANEF-004`, and the mistake two W1 pull requests each made
independently. Nothing here reaches a governed primitive, so all three are
DENIED rather than allowed, and this module takes no `effects/allowlist.toml`
row: an allowance is what that file records, and this module takes none.
`runner::container::tests::every_child_module_of_the_container_funnel_states_\
its_own_lint_level` already walks `src/runner/host/`, so this file was graded
against all three from its first commit.

## `pub enum KeyCase {`

How the platform compares environment variable names.

A type rather than a `cfg!` at each comparison. `cfg!(windows)` is false on
a Linux developer box and on the Linux CI cell, so a rule written as a
`cfg!` is a rule whose Windows arm no test on those machines can reach —
both sides of the pin move together. [`Self::ALL`] is what the grids run
over; [`Self::current`] is what production selects. The same shape
[`crate::topology::effects::Host`] uses, for the same reason.

## `pub enum KeyCase` › `Sensitive,`

Unix: `Path` and `PATH` are two variables.

## `pub enum KeyCase` › `Insensitive,`

Windows: `Path` and `PATH` are one variable, and a child that received
both would receive whichever the block happened to list last.

## `impl KeyCase` › `pub const ALL: &'static [Self] = &[Self::Sensitive, Self::Insensitive];`

Both rules. Every grid runs over this, not over [`Self::current`].

## `impl KeyCase` › `pub const fn current() -> Self {`

The rule this machine's process environment obeys.

## `impl KeyCase` › `pub fn same_key(self, left: &OsStr, right: &OsStr) -> bool {`

Whether these two names are the same variable under this rule.

## `pub enum ObjectGraph {`

Which object graph the Git children of a composed environment read, and
where, and the one thing about it a conductor gets to choose.

`Recorded` is the default and the schema-4 rule: the objects the
repository holds, never the objects `refs/replace/*` points at them
(`design/15`, "What an exact snapshot is exact against"). It is what
`compose` writes [`NO_REPLACEMENT_OBJECTS`](../../../../src/workspace_manager.rs)
for, process-wide: every repository a child touches loses its
replacements, a repository a gate creates for itself included. That is
`PR326-SCHEMA4-ROLE-ENVIRONMENT-REFUSES-REPLACEMENTS-EVERYWHERE` (P2,
latent: no supported configuration activates the schema-4 conductor),
and it is why the v0.1 runner no longer reads it.

`RecordedIn` is the v0.1 conductor's: the recorded graph in one
repository, the one the run manages, and whatever the configuration says
everywhere else. Its section below says how.

`AsReplaced` composes nothing, so a child reads whatever its base says
about `refs/replace/*`. No conductor installs it. It is a test
instrument: the witnesses that must show a replacement is live in a
role process's environment (`src/workspace_manager/tests.rs`), or that
reading the replaced graph over a snapshot written from the recorded one
fails (`src/gates.rs`), build a runner reading it.

It was the schema-1..3 exemption from PR #271 (`a55f7049`) until
`LEGACY-WORKSPACE-READS-REPLACEMENT-OBJECTS` closed. The rule behind it
still stands: a consumer must read what its own producer wrote. The v0.1
workspace (`src/workspace.rs`) then set no pair on its Git children and
was frozen, so its checkout of a commit whose recorded tree carries a
replacement materialised the *replacing* tree. Composing the pair for a
gate over that checkout put the two on different graphs and failed `git
diff --exit-code HEAD` on a workspace the engine itself had just written
(measured on git 2.43, PR #271 round 1's regression finding), and the v0.1
conductor installed `AsReplaced` to match its producer. The exemption
existed because of the freeze. Closing the finding amended the freeze:
every Git child of the module now refuses replacements, and the same gate
over an untouched snapshot then failed under `AsReplaced` and passed under
`Recorded` (git 2.43). So the v0.1 conductor reads the recorded graph
too, first as `Recorded` and, since #326 round 4, as `RecordedIn`.

It is a field of the environment rather than of the request because it is
a property of the *conductor* — one schema per run, chosen once where the
runner is built — and because defaulting it here makes the isolated
reading the one a new spawn site gets without asking.

## `pub enum ObjectGraph` › `RecordedIn(ManagedRepository),`

The recorded graph in the repository [`ManagedRepository`] names, through
two conditional includes `compose` appends to `GIT_CONFIG_PARAMETERS`.

Not the process-wide variable, because a variable does not know which
repository it is for. Under `Recorded` a v0.1 gate that ran `git init`,
`commit` and `git replace` in a fixture of its own read the original
object, failed and parked the run, in a managed repository with nothing
under `refs/replace/` (#326's round-2 regression review). And on Git 2.41 a
configured `core.useReplaceRefs = true` beats the variable, so the
producer wrote recorded bytes while the roles read replacements (the
round-3 review of record).

Not the repository's own configuration either. A v0.1 run manages the
operator's checkout, and every gate snapshot is a linked worktree of it,
so `--local` would write the operator's common `.git/config`, visible from
unrelated sibling worktrees and outliving the run; and a
`config.worktree` or an inherited `GIT_CONFIG_COUNT` or
`GIT_CONFIG_PARAMETERS` outranks that file anyway (the design check of
2026-09-27, on Git 2.40.0, 2.41.0 and 2.43.0).

Not `GIT_CONFIG_COUNT` pairs: Git reads `GIT_CONFIG_PARAMETERS` after the
counted pairs, so an inherited `'core.useReplaceRefs'='true'` there would
win. Appended to `GIT_CONFIG_PARAMETERS`, after every entry the base or the
overlay carries, the includes are the last configuration any role's Git
reads, except a `-c` that Git command is given itself.

Measured with plain Git on 2.40.0, 2.41.0, 2.42.0 and 2.43.0, with `true`
in the system, global, repository and worktree files, in
`GIT_CONFIG_COUNT` and in inherited `GIT_CONFIG_PARAMETERS`: the managed
repository's main and linked worktrees read the recorded object, and a
fixture repository inside the linked worktree and one outside it kept
their replacements, under a path with spaces and `[x]*?` in it. On 2.40.0,
`git merge-tree` over a replaced commit read the replacement with the
includes, and with `-c core.useReplaceRefs=false` too; from 2.41.0 it read
the recorded commit. That is why the floor is 2.41
(`src/workspace.rs`'s `require_git_floor`).

**Which repository the conditions reach is decided by what each directory
does, not by the platform** (#326 round 5). Until then macOS and Windows
matched the whole path under `gitdir/i:`, which folds every component
whatever the directory holding it does. On a case-sensitive directory the
include therefore also reached a repository whose path differs from the
managed one only in case, and a gate reading that repository got the
recorded object where the repository had installed a replacement (the
round-4 regression lens, which also reproduced it natively on Windows with
NTFS case sensitivity enabled on one directory). Each component is now matched with
case unless the directory holding it finds it under the other case;
[`ManagedRepository::new`]'s section says how that is measured, and what it
costs.

**What it does not do.** A role's own `git -c core.useReplaceRefs=true`
comes after the includes and wins, and a role that clears its environment
before running Git loses them. Configuration is not an enforcement
boundary, and the policy claims none.

**Each decision fails a witness when it is undone** (#326 round 4, Git 2.43.0;
the witnesses are in `src/engine/tests.rs`, `src/runner/host/tests.rs`,
`src/workspace.rs` and `src/gates.rs`, and "ok" means the mutation survived that
one):

| mutation | run and resume | own fixture | path shapes | siblings | refusal, interruption | composition | exact entries | spawn refusal | Git floor | gate |
|---|---|---|---|---|---|---|---|---|---|---|
| none | ok | ok | ok | ok | ok | ok | ok | ok | ok | ok |
| the process-wide variable again | FAILED | FAILED | FAILED | FAILED | FAILED | FAILED | ok | FAILED | ok | ok |
| the includes before the inherited entries | FAILED | ok | ok | ok | FAILED | FAILED | ok | ok | ok | ok |
| no `/worktrees/*` condition | FAILED | FAILED | FAILED | FAILED | FAILED | ok | FAILED | ok | ok | FAILED |
| no glob escaping | ok | ok | FAILED | ok | ok | ok | FAILED | ok | ok | ok |
| no quote escaping | ok | ok | FAILED | ok | ok | ok | FAILED | ok | ok | ok |
| one `<common dir>/` condition instead of two | FAILED | FAILED | FAILED | ok | FAILED | ok | FAILED | ok | ok | ok |
| the include never written | FAILED | FAILED | FAILED | FAILED | FAILED | ok | ok | ok | ok | FAILED |
| no check before a role | ok | ok | ok | ok | ok | ok | ok | FAILED | ok | ok |
| no Git floor | ok | ok | ok | ok | ok | ok | ok | ok | FAILED | ok |
| the includes before the overlay | FAILED | ok | ok | ok | FAILED | FAILED | ok | ok | ok | ok |

The gate witness is `a_v1_gate_judges_the_tree_its_own_workspace_materialised`,
whose fixture pins `true` in the repository: on Git 2.41 it also fails under the
process-wide variable. The siblings run in linked worktrees only, which a single
`<common dir>/` condition does reach.

## `pub const CONFIG_PARAMETERS: &str = "GIT_CONFIG_PARAMETERS";`

The variable Git reads command-line configuration from, and hands its own
children: `git -c` appends to it. Undocumented by `git-config(1)` and read
by every Git since 2.31 in its `'key'='value'` form, which is the form the
includes are written in.

## `pub const RECORDED_OBJECTS_INCLUDE: &[u8] = b"[core]\n\tuseReplaceRefs = false\n";`

The whole of the include file, byte for byte. The runner compares the file
with it before every role it starts, so a file that holds anything else,
nothing, or is not there refuses the role rather than including nothing.
A missing include is otherwise silent: Git skips an include whose file does
not exist.

## `pub enum GitdirRule {`

How a canonical path is spelled for an `includeIf.gitdir` condition on
this platform. A type rather than a `cfg!` at each use, for the reason
[`KeyCase`] is one: `ALL` lets a grid on one machine cover every rule.

It decides the spelling and nothing else. Whether a component is matched
with case is not a property of the platform: a macOS volume or a Windows
directory can be case-sensitive, and a Linux directory can fold case, so
[`ManagedRepository::new`] asks each directory on the path. Until #326
round 5 macOS had a rule of its own, `PosixFoldingCase`, and it and the
Windows rule matched under `gitdir/i:`; the `keyword` section says why that
went.

## `pub enum GitdirRule` › `Posix,`

Every Unix target, macOS included: the path's own bytes.

## `pub enum GitdirRule` › `Windows,`

Git for Windows compares the realpath `GetFinalPathNameByHandleW` gives,
with its `\\?\` prefix removed, `UNC\` turned into `//`, and every
backslash a slash. `std::fs::canonicalize` is the same call, so its result
spelled that way is Git's own text. The prefix, a drive letter or a UNC
server and share, is matched as that call spells it, and each component
below it by what the directory holding it does.

## `impl GitdirRule` › `pub const fn native() -> Self {`

The spelling this machine's Git uses.

## `impl GitdirRule` › `pub const fn keyword(self) -> &'static str {`

The condition's keyword: `gitdir:`, under every rule.

`gitdir/i:` folds every component whatever the directory holding it does.
On a case-sensitive directory, then, it also matched a repository whose
path differs from the managed one only in case, and kept that repository's
gates off the replacements it had installed: the blocking finding of #326's
round-4 regression review, which the case-sibling witness reproduces (it
fails at `49243a24` under both rules that used `gitdir/i:`). Where a
directory does fold case, the component is spelled with both cases of each
ASCII letter instead, which reaches that component in any ASCII case and
reaches no other component in any case but its own.

## `impl GitdirRule` › `pub fn spelling(self, path: &[u8]) -> Vec<u8> {`

The path as Git spells it under this rule, before any escaping.

## `pub struct ManagedRepository {`

The repository a v0.1 run manages, as `compose` names it to Git: its
canonical common directory, the include file, and the two
`GIT_CONFIG_PARAMETERS` entries they make, built once so every role gets
the same bytes.

## `impl ManagedRepository` › `pub fn new(common_dir: &Path, include: &Path, rule: GitdirRule) -> Result<Self, UpstrokeError> {`

The entries, for a canonical common directory `C`:

```
'includeIf.gitdir:C.path'='<include>' 'includeIf.gitdir:C/worktrees/*.path'='<include>'
```

- **Matched as each directory matches.** For each component of `C`,
  [`folds_case`] asks the directory holding it whether it finds that name
  under the other ASCII case. Where it does, each ASCII letter of the
  component is spelled as a class of both cases (`Repo` becomes
  `[Rr][eE][pP][oO]`); where it does not, the component is matched
  exactly. So a path Git reaches through a folding directory in another
  case still names the managed repository, and a repository beside it,
  in a directory that keeps case, whose name differs only in case, does
  not. Where nothing on the path folds, as on this box's ext4, the entries
  are byte for byte what round 4 wrote; on a volume that folds case
  throughout, every component is spelled in classes. The prefix of a
  Windows path is matched
  as spelled (the `Windows` rule's section), and so are letters outside
  ASCII, which Git's own `gitdir/i:` does not fold either.
- **Two conditions.** `C` is the main worktree's Git directory, and
  `C/worktrees/*` every linked worktree's, the gate and review snapshots
  among them. `*` does not cross a `/`, so the Git directory of a
  submodule, under `C/modules/`, is not matched, and neither is anything
  deeper. A bare `C/` would have matched both.
- **Escaped.** The condition is a wildmatch pattern, so each `[`, `]`,
  `*`, `?` and `\` of `C` is escaped, in a folding component as in any
  other; the only globs are the worktrees component and the classes the
  folding components are spelled with. Unescaped, a `[` silently matches
  nothing and a `*` matches other repositories.
- **Quoted.** Each key and value is single-quoted; a quote inside is
  closed, escaped and reopened (`'\''`), which is what Git's own
  `sq_dequote` reads.
- **Refused, never approximated:** a relative common directory or include,
  which Git rejects from the command line; a newline in `C`, which Git
  rejects in a key; on Windows a path that is not valid Unicode, which
  cannot reach Git for Windows through its environment; a `.` or `..`
  component, which the realpath Git matches never holds; and a component
  whose directory cannot be asked, which leaves no way to tell whether it
  must be matched with case. A pattern that silently failed to match would
  put the roles back on the replaced graph with nothing to say so, and one
  that matched too much would take another repository's replacements away.

**What the lookups cost, and what happens when one fails.** Two lookups for
each component that holds an ASCII letter (two `lstat`s on Unix, two
`std::fs::canonicalize` calls elsewhere), once per run and once per resume:
`Workspace::recorded_objects_scope` builds the scope there, and no role
repeats it. Measured on the build box through this constructor, over 10,000
constructions each: six `statx` calls and 2.7 µs for `/srv/tactus/.git`,
and 12.7 µs for a path of twelve components.

A lookup that fails with anything but "not found" refuses the run before
its first role, naming the path and the error (`UpstrokeError::Refused`, from
`run` and `resume` alike); nothing falls back to either answer. On a folding
directory the other spelling names the entry just found, and on a
case-sensitive one it is not found unless a sibling bears exactly that
spelling, which the identity check tells apart. None of the layouts measured
for this change reached the refusal: a case-sensitive ext4 directory, a
case-folding one, and a case-sensitive directory inside a case-folding one.
A directory missing from the path does reach it
(`the_includes_refuse_a_common_directory_git_cannot_read_in_a_key`); by
reasoning, not measurement, so would an I/O error, a directory removed
between the two lookups, or on Windows a case-sibling the process may not
open.

**Each round-5 decision fails a test when it is undone** (Git 2.43.0, with
`TMPDIR` on this box's ext4, which keeps case, and in a case-folding directory
of a loop-mounted `mkfs.ext4 -O casefold` image; "ok" means the mutation
survived that test on both, and a layout named means it failed there alone).
The witness is
`a_repository_whose_path_differs_from_the_managed_one_only_in_case_keeps_its_replacements`;
the inline tests are this module's own; the host tests are
`the_includes_refuse_a_common_directory_git_cannot_read_in_a_key` and
`the_windows_rule_spells_a_path_as_git_for_windows_does`:

| mutation | witness | inline: exact entries | inline: refusals | host: refusals | host: rules |
|---|---|---|---|---|---|
| none | ok | ok | ok | ok | ok |
| `gitdir/i:` again under the `Windows` rule | FAILED (ext4) | ok | ok | ok | FAILED |
| `gitdir/i:` under both rules | FAILED (ext4) | FAILED | ok | ok | FAILED |
| every component folds | FAILED (ext4) | ok | ok | FAILED | ok |
| no component folds | FAILED (case-folding) | ok | ok | FAILED | ok |
| any entry under the other spelling counts as the same one | FAILED (ext4) | ok | ok | ok | ok |
| a failed lookup reads as "keeps case" | ok | ok | FAILED | FAILED | ok |
| a component takes the answer of the one before it | ok | FAILED | ok | ok | ok |

The five v0.1 witnesses in `src/engine/tests.rs` survived every row on both
layouts. On ext4 nothing folds. On the case-folding directory Git named the
managed repository in its stored case whenever it found it from a working
directory: `git -C <mnt>/ci/PROBE rev-parse --absolute-git-dir` printed
`<mnt>/ci/Probe/.git`, as `pwd -P` there printed `Probe`, while
`git --git-dir=<mnt>/ci/PROBE/.GIT` printed the path as given. The witness's
read through a Git directory spelled in capitals is what catches "no component
folds" there.

## `impl ManagedRepository` › `fn matching(`

[`Self::new`] with the lookup supplied, so the inline tests can pin the
entries for any answer without a directory to measure. It walks `C` once,
root first, and asks about each normal component with the path of the
directory that holds it.

## `impl ManagedRepository` › `pub fn verify_include(&self) -> Result<(), UpstrokeError> {`

Whether the include holds [`RECORDED_OBJECTS_INCLUDE`] exactly. The
runner asks before every role it starts and does not start one on `Err`
(`ProcessFate::NeverStarted`). A read, not an effect: the file is written
by `Workspace::recorded_objects_scope`, which every run and resume call
before their first role.

## `fn folds_case(parent: &Path, name: &OsStr) -> Result<bool, String> {`

Whether the directory `parent` finds `name` under the other ASCII case.
It looks up `name` with the case of every ASCII letter swapped (`Repo` as
`rEPO`) and compares what it finds with `name` itself: the same entry means
the directory folds case, "not found" or a different entry means it keeps
case, and any other error is the caller's refusal. A name with no ASCII
letter has no other spelling to look up, and is matched exactly.

## `fn in_the_other_case(name: &OsStr) -> Option<OsString> {`

`name` with the case of every ASCII letter swapped, or `None` when it has
none.

## `const fn other_case(byte: u8) -> u8 {`

The other ASCII case of a letter; any other byte as it is.

## `fn same_entry(entry: &Path, other: &Path) -> Result<bool, String> {`

Whether two spellings name one entry. On Unix by `(st_dev, st_ino)` of
each, without following a final symbolic link: Linux's realpath does not
correct case, so comparing canonical paths would call a folding directory
case-sensitive there. Elsewhere by `std::fs::canonicalize`, which on Windows
returns the name as the directory stores it: the standard library's file
identity there, `MetadataExt::file_index`, is unstable on 1.85.0 and on
1.97.1 alike (E0658, `windows_by_handle`). A second spelling that is not
found is `false`; an error on either lookup is the caller's refusal.

## `fn matched(into: &mut Vec<u8>, spelled: &[u8], folded: bool) {`

One component of the condition: each ASCII letter as a class of both cases
when the component folds, and every character wildmatch treats as syntax
escaped with a backslash.

## `fn single_quoted(text: &[u8]) -> Vec<u8> {`

POSIX single quoting, as Git's `sq_quote_buf` writes it.

## `pub struct HostEnvironment {`

`host-v1`'s environment contract.

Holds its base explicitly so a test can compose against a base it wrote
rather than against whatever variables happen to be set on the machine
running the suite.

## `impl HostEnvironment` › `pub fn from_process() -> Self {`

The Upstroke process environment, under this platform's name rule.

## `impl HostEnvironment` › `pub fn with_base(base: Vec<(OsString, OsString)>, case: KeyCase) -> Self {`

An explicit base, for grids that must cover both name rules.

## `impl HostEnvironment` › `pub fn reading(mut self, objects: ObjectGraph) -> Self {`

The object graph this environment's children read. Owned by whoever
builds the runner, never by an adapter or an overlay.

## `impl HostEnvironment` › `pub const fn objects(&self) -> &ObjectGraph {`

Which graph is in force, so a witness can assert what a conductor
installed rather than infer it from a composed vector.

## `impl HostEnvironment` › `pub fn base(&self) -> &[(OsString, OsString)] {`

The base this runner composes from.

## `impl HostEnvironment` › `pub const fn case(&self) -> KeyCase {`

The name rule in force.

## `impl HostEnvironment` › `pub fn reserved_values(`

The reserved values the runner supplies for this request.

A reserved key the base does not carry is **not** supplied: setting an
absent variable to the empty string is a different environment from not
setting it, and several CLIs read "set but empty" as an instruction.

DESIGN.md:259-262 — "the host runner starts from the Upstroke environment
and the container runner from the image environment; **each** supplies
role-scoped `HOME`, `PATH`, and credential locations" — resolved for
`host-v1` as follows, and the split is deliberate:

* **credential locations are role-scoped**, by
  [`supplies_credentials`]. A gate is repository-controlled code and the
  shell probe is a shell; neither runs an agent CLI, so neither is told
  where an agent's credentials live, whatever agent the request happens
  to name. This is the sentence's own word "role-scoped" doing work.
* **`HOME`, `PATH` and `USERPROFILE` are supplied to every role at the
  host boundary's own value.** That is a boundary, and "one machine,
  one user" is a rationale rather than a basis for it, so it is drawn
  from live passages — three of them, each forbidding a different part
  of a per-role value:

  1. DESIGN.md:263 — "Probe and execution compose the **same** base,
     mounts, reserved values, and overlay, so pre-flight certifies the
     environment that will actually spend." `probe(<agent>)`,
     `implement` and `review` are the probe and the execution that
     sentence pairs; a `HOME` differing across them would make
     pre-flight certify an environment the attempt never runs in.
  2. `design/26_design_merge_queue_protocol.md:388-389` (§26) —
     "gate-shell/program availability is checked inside the same
     boundary." The shell probe certifies the shell a gate will run; a
     `PATH` differing between `probe(shell)` and `gate` would certify a
     different program from the one that runs.
  3. The same section, :398 — "Host runner behavior remains
     available and honestly provides **no OS boundary** around gate
     code." Handing gate code a different `HOME` on this host would
     assert an isolation the host does not have: repository-controlled
     code reads the real home directory by absolute path either way.
     What the host *can* honestly do is not disclose a location it
     would otherwise hand over, and that is [`supplies_credentials`].

  The value comes from the base rather than from anything this runner
  invents, because the same section says where the base is (:378-379):
  "**The host base starts from the Upstroke process environment**, while
  the container base starts from the image environment." A process
  environment carries one value per key under [`KeyCase`] — so one
  value is what a correct `host-v1` *produces*, not a narrowing this
  slice chose. The container runner differs not because its `HOME`
  string differs per role but because each role's container is its own
  filesystem; PR4's `production_effect` is "same behavior plus stronger
  Windows crash containment", and no passage describes a per-role home
  directory on the host for it to grow into.

  Asserted from those passages, not commented, by
  `the_reserved_values_every_role_gets_are_the_host_boundarys_own` — so
  a `host-v1` that ever does scope `HOME` has to change a passage
  first, rather than a count.

A reserved key the base does not carry is **not** supplied: setting an
absent variable to the empty string is a different environment from not
setting it, and several CLIs read "set but empty" as an instruction.

## `impl HostEnvironment` › `pub fn compose(`

Base, then reserved values, then overlay — DESIGN.md:263's own order
("the same base, mounts, reserved values, and overlay").

The base's own copies of the **reserved** keys are dropped before the
runner supplies them, and that is what makes "role-scoped" a property
of the child's environment rather than of a vector nothing reads.
Cloning the base and then upserting would leave every credential
location the Upstroke process happens to carry in a gate's environment —
a gate is repository-controlled code, and `CODEX_HOME` reaching it is
exactly the thing [`supplies_credentials`] exists to prevent. It would
also make this step *output-equivalent to deleting it*, because
[`Self::reserved_values`] reads the values back out of the same base.
So the reserved keys arrive from one place — this function's supply
step, which is role-scoped — or not at all.

Then the object graph, **after** the overlay and not before it. Under
[`ObjectGraph::Recorded`], the schema-4 runners' reading,
[`NO_REPLACEMENT_OBJECTS`](../../../../src/workspace_manager.rs). Under
[`ObjectGraph::RecordedIn`], the v0.1 conductor's, the repository's two
includes appended to whatever `GIT_CONFIG_PARAMETERS` the base and the
overlay left, separated by one space, or alone when there is none:
Git refuses a leading space there. `AsReplaced` is the test instrument its
section above describes.
`HostRunner::run` clears the ambient environment
and installs exactly what this returns, so a pair that is not composed here
reaches no child: a gate or a reviewer inside an exact snapshot would read
whatever `git replace` points at the judged objects, and measured on git 2.43 it
did — `git show HEAD:f` returned the replacement and `git status --porcelain`
called an untouched snapshot modified. `design/15`'s "What an exact snapshot is
exact against" is the product sentence; the pair is one constant named at each
of the four boundaries that starts a child which can run Git over a snapshot
this engine produced, and at the one builder every Git child of the v0.1
workspace starts from (`src/workspace.rs`'s `git_command`).

It is **asserted, not reserved**, and the two are different things. The reserved
keys are values this boundary reads *from its host* and re-supplies role-scoped,
which is why they are stripped from the base first and why `preflight` refuses
an overlay that restates one — a gate permitted to set `PATH` is a hijack. This
one is a constant the runner states; there is nothing in the base to re-supply,
an overlay restating it is not a hijack but a no-op, and refusing it would add a
failure mode without adding a guarantee. The ordering is what supplies the
guarantee: last write wins, and this is the last write. Git 2.43 reads the
*presence* of the variable rather than its value (measured: `=0` and `=false`
both disable replacement as `=1` does), so the value `1` is correct under either
reading and an overlay could not re-enable the mechanism even if it outranked
this step — the ordering is what makes that true of a future Git that does read
the value.

### Errors

[`UpstrokeError::Refused`] naming the key when the overlay names a
reserved one. That is the contract's `expected_failures_refusals[0]`,
"reserved env conflict -> pre-flight error", and it is refused by
**key**: `invariants_introduced[0]` says "reserved keys refused
pre-flight", and an overlay permitted to restate `PATH` today because
the value happens to match is an overlay that breaks silently the day
the runner's value changes.

## `impl HostEnvironment` › `pub fn preflight(&self, overlay: &[(String, String)]) -> Result<(), UpstrokeError> {`

The reserved-key refusal on its own, so a caller can certify an overlay
without building an environment.

### Errors

[`UpstrokeError::Refused`] naming the offending key and the reserved key
it collides with.
