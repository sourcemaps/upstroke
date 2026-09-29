---
id: PR326-A-SPELLING-THE-FILESYSTEM-EQUATES-READS-REPLACEMENTS-IN-THE-MANAGED-REPOSITORY
severity: P1
disposition: deferred   # filed, not repaired, on the owner's ruling of 2026-09-29, which waives the no-P1 merge bar for #326 alone
category: correctness
pr: 326
reviewed_sha: 6acf1216410f0a1485ef2bfbbd23994c44618d92
location: src/runner/host/environment.rs:290
provenance: fix_regression   # corrected on the owner's ruling of 2026-09-29: at master d6f65cbb the gate passes through the missed spelling, and #326's repair makes it fail; the Provenance section gives the measurement
first_bad: b2e6dc60c45916b57759ff5bc310a86a3f511bc1   # the path-scoped include it escapes; prior finding LEGACY-WORKSPACE-V1-RUNNER-READS-THE-REPLACED-GRAPH-OVER-A-RECORDED-SNAPSHOT
guard: a decision by the owner about repository identity and supported layouts, not another repair round. Nothing in the suite reproduces it: the case-sibling witness spells only ASCII case
---

## Failure sequence

A v0.1 role reads the replaced graph over a snapshot the v0.1 workspace wrote from the recorded one, so the
producer and the role disagree.

1. The repository a v0.1 run manages has `refs/replace/<recorded tree> -> <replacing tree>` installed, and its
   path holds a component the filesystem equates with another spelling: a case-folding ext4 directory
   (measured), where `café` is also reached as `CAFÉ` and as `café` with the accent decomposed (NFD).
2. `Workspace::gate_snapshot_for_candidate` materialises the recorded tree: the snapshot holds the recorded
   bytes.
3. `HostRunner::for_legacy_workspace` runs the gate with its two `includeIf.gitdir:` conditions. Each ASCII
   letter of a component whose directory folds case is spelled as a class of both cases, and every other byte
   literally (`matched`, `src/runner/host/environment.rs:290` at the reviewed SHA).
4. The gate names its linked worktree's Git directory through `--git-dir` in a spelling the filesystem equates
   with the stored one but the conditions do not match: `…/CAFÉ/…` or the NFD `…/café/…` for a directory stored
   NFC as `café`. Git keeps that spelling in the realpath it matches on Linux, so neither include applies.
5. `git show HEAD:f.txt` returns the replacing bytes, `git diff --exit-code HEAD` exits 1, and the gate returns
   `Fail` over a snapshot nothing has touched. The task is retried and parked for a reason that is not its own.

## Measurement

**Plain Git over each constructor's own conditions** (`ManagedRepository::new` through an out-of-tree probe),
reading a replaced blob with `git --git-dir=<spelling> cat-file -p` on a case-folding ext4 mount (`mkfs.ext4 -O
casefold`, a `chattr +F` directory), with nothing configured and nothing cleared from the environment. The same at
`c453705f` and at `6acf1216`, and on Git 2.41.0 and 2.43.0:

| spelling given to `--git-dir` | repository stored NFC as `café` | repository stored NFD as `café` |
|---|---|---|
| `café`, NFC | recorded | **replacing** |
| `CAFé`, the ASCII letters in capitals | recorded | **replacing** |
| `CAFÉ`, NFC | **replacing** | **replacing** |
| `café`, NFD | **replacing** | recorded |
| `CAFÉ`, NFD | **replacing** | recorded |

So the scope built from an NFC directory misses its NFD spellings, and the scope built from an NFD directory
misses its NFC spellings.

**Through production code.** A scratch test, not committed, in `git archive` copies of `6acf1216` and `c453705f`,
modelled on `a_v1_gate_judges_the_tree_its_own_workspace_materialised` and run in its replacement-free child:
`Workspace::open`, `recorded_objects_scope`, `gate_snapshot_for_candidate` over a replaced tree, and
`ShellGate::check` through `HostRunner::for_legacy_workspace`, with the managed repository at `<TMPDIR>/café` on
the case-folding mount. The snapshot held `A`, the recorded bytes. Each gate ran
`git --git-dir=<the snapshot's Git directory, respelled> show HEAD:f.txt; git --git-dir=<…> diff --exit-code HEAD -- f.txt`:

| gate's spelling | stored NFC | stored NFD |
|---|---|---|
| no `--git-dir` (Git finds it from the working directory) | `Pass`, `A` | `Pass`, `A` |
| as stored | `Pass`, `A` | `Pass`, `A` |
| `café`, NFC | `Pass`, `A` | **`Fail`**, `B`, diff exit 1 |
| `CAFÉ`, NFC | **`Fail`**, `B`, diff exit 1 | **`Fail`**, `B`, diff exit 1 |
| `café`, NFD | **`Fail`**, `B`, diff exit 1 | `Pass`, `A` |

The same with Git 2.41.0 and 2.43.0 at `6acf1216`, and with 2.43.0 at `c453705f`. The round-6 fix-check review of
`6acf1216` (`gpt-6-astra`, `max`) measured the same consequence independently, as the owner's ruling of 2026-09-29
relays it.

## Cause

`matched` (`src/runner/host/environment.rs:290` at the reviewed SHA) spells a folding component's **ASCII letters**
as classes and leaves every other byte literal. Git's `includeIf.gitdir:` is a byte pattern over the realpath of the
command's Git directory, and on Linux that realpath keeps the spelling it was given. The filesystem, not Git,
decides which other spellings name the same directory.

## Why a character patch will not close it

**Refusing, or spelling out, non-ASCII repository names would not close it.** With an entirely ASCII repository
path on the same mount, `worKtrees` (the Kelvin sign, U+212A, for `k`) and `worktreeſ` (the long s, U+017F, for
the last `s`) name the same directory as `worktrees`, and a linked worktree's Git directory named through either
read `replacing` at `6acf1216` and at `c453705f`, on Git 2.41.0 and 2.43.0 (plain Git over each constructor's
conditions; `WORKTREES` read `recorded` at `6acf1216`, the round-6 repair). The case-folding ext4 directory folds
those two characters onto ASCII letters, so an ASCII-only path is reachable through non-ASCII spellings too.

## Reachability

It needs two things: **live replacements** in the managed repository, and a role naming the repository's Git
directory by **an alternate explicit spelling** the filesystem equates with the stored one. **Ordinary discovery
from the working directory passed**: a gate that lets Git find the repository from its working directory read the
recorded bytes in every run below. No alias, no source modification and no privileged role operation are needed;
supported Git and ordinary filesystem name equivalence are enough. That is narrower than ordinary use unqualified:
it takes an explicit alternate spelling. The review grades it P1 on those conditions.

- **The filesystem.** Measured on a case-folding ext4 directory only. macOS volumes fold case by default and
  Windows directories fold case by default; neither was measured for this row, and Git for Windows takes its
  realpath from `GetFinalPathNameByHandleW`, which returns the stored name, so Windows may not reach it.
- **The repository's name.** The NFC and NFD rows need a letter outside ASCII in the path, which is ordinary (an
  accented name in a home directory or a project). The Kelvin-sign and long-s rows need none.
- **The spelling.** The role, its tools or its gate command must name the Git directory explicitly, in a
  spelling other than the one the scope was built from: through `--git-dir`, `GIT_DIR`, or a path built from
  input. In every run above, a gate that let Git find the repository from its working directory read the
  recorded bytes.

## Provenance

**`fix_regression`**, on the owner's ruling of 2026-09-29, which corrected an earlier `pre_existing`. The round-6
regression lens measured the gate at master, as the orchestrator's ruling relays it:

| revision | snapshot bytes | gate, stored spelling | gate, missed spelling |
|---|---|---|---|
| master `d6f65cbb` | replacing `B` | `Pass`, reads `B` | `Pass`, reads `B` |
| `c453705f` | recorded `A` | `Pass`, reads `A` | **`Fail`**, reads `B` |
| `6acf1216` | recorded `A` | `Pass`, reads `A` | **`Fail`**, reads `B` |

At master both spellings pass, because the producer and the roles agree: both read replacements. The gate's failure
is introduced by #326's repair. Every measurement in this file reproduces at `c453705f` as well as at `6acf1216`, so
it predates round 6, not #326. Within #326 its history is this. The include it escapes is #326's own, added at `b2e6dc60` (round 4), which matched
the path exactly on Linux and under `gitdir/i:` on macOS and Windows; `gitdir/i:` did not fold a letter outside
ASCII either (round 5 measured a `CAFÉ` condition missing a repository at `café`). The
disagreement that makes the gate fail needs the recorded producer, #326's `a9535c42`. Before #326, master's v0.1
roles read the replaced graph in every spelling and so did its producer, which is the finding #326 took up
(`LEGACY-WORKSPACE-READS-REPLACEMENT-OBJECTS`, whose producer sequence is closed). The prior finding for this
consumer disagreement is `LEGACY-WORKSPACE-V1-RUNNER-READS-THE-REPLACED-GRAPH-OVER-A-RECORDED-SNAPSHOT`, whose
mechanism, a runner reading `ObjectGraph::AsReplaced`, #326 repaired in round 2.

## The design question

The scope identifies the repository by **how its path is spelled**, while which spellings reach one directory is
decided by each filesystem's case-folding, Unicode normalisation and aliases, not by Git and not by this code. Git's
conditional includes do pattern matching over a spelling; filesystem lookup supplies a separate equivalence
relation. Rounds 5 and 6 of #326 each closed one named spelling class (case, per directory, in ASCII; then the
linked worktrees' component) and left others, and the alias finding
(`PR326-A-JUNCTION-MAKES-A-CASE-SENSITIVE-DIRECTORY-READ-AS-FOLDING`) is the same mismatch in the outward
direction. So the change that takes this up needs a decision about **repository identity and supported layouts**:
either a mechanism that does not identify the repository by spelling, or a stated set of layouts the v0.1 isolation
supports, refused or disclosed outside it. A fourth named class of characters is not the answer.
