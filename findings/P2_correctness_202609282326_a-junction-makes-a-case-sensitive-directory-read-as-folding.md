---
id: PR326-A-JUNCTION-MAKES-A-CASE-SENSITIVE-DIRECTORY-READ-AS-FOLDING
severity: P2
disposition: deferred
category: correctness
pr: 326
reviewed_sha: c453705f114b475b0a5b1484cc64cfda5c5a38e2
location: src/runner/host/environment.rs:278
provenance: fix_regression   # e85e51b9 (#326 round 5) introduced the lookup; found by the round-5 review of c453705f (gpt-6-astra, max); filed, not repaired, on the owner's ruling of 2026-09-28
first_bad: e85e51b91c95a3e3fff582eab11cc4a14ebc953b
guard: the change that next takes up how the v0.1 scope decides whether a directory folds case (`ManagedRepository::new`); owner ruling 2026-09-28 files this in #326 rather than repairing it there. Nothing in the suite reproduces it: the case-sibling witness creates `rEPO` as a directory of its own, never as an alias
---

## Failure sequence

The v0.1 scope's include reaches a repository it must not, when an alias makes a directory that keeps
case look as though it folds case.

1. On Windows, a directory made case-sensitive (`fsutil file setCaseSensitiveInfo <dir> enable`) holds
   two independently initialised repositories, `Repo` and `repo`. `repo` installs a replacement with
   `git replace`.
2. The same directory holds a junction `rEPO` pointing at `Repo` (`mklink /J rEPO Repo`). `rEPO` is
   exactly the spelling the scope looks `Repo` up under: every ASCII letter's case swapped
   (`in_the_other_case`).
3. A v0.1 run manages `Repo`. `ManagedRepository::new` asks whether that directory finds `Repo` under
   the other case. On Windows `same_entry` compares `std::fs::canonicalize` of both spellings
   (`src/runner/host/environment.rs:278` at the reviewed SHA). `canonicalize` follows the junction, so
   both give `…\Repo`, and the directory is judged to fold case.
4. The component is spelled `[Rr][eE][pP][oO]`, and the condition also matches `repo`'s Git
   directory.
5. A role of that run reading `repo`, for example a gate whose command runs
   `git -C ..\repo cat-file -p <blob>`, reads the recorded object where `repo` installed a
   replacement. A gate whose assertion depends on that replacement fails, and the task is retried and
   parked.

#326 round 6 does not change this: `folds_case` and `same_entry` are unchanged at `5c1e0292`
(`same_entry`'s Windows arm is at `:280` there), and round 6's new lookup, `refs` in the common
directory, goes through the same comparison.

## Evidence

- **Natively on Windows.** The round-5 review of `c453705f` measured it on Git for Windows 2.50.1:
  the sibling read `replacing` before the junction was created and `recorded` after. That is recorded
  here as the orchestrator's round-6 brief relays it; this file's author did not repeat it on
  Windows.
- **The same class on Linux, measured at `c453705f` and at `5c1e0292`, with the same result at
  both.** An out-of-tree program called `ManagedRepository::new` over `Repo` in a case-sensitive
  ext4 directory, and plain Git 2.43.0 read `repo` with the `GIT_CONFIG_PARAMETERS` it returned:

  | `rEPO` | `Repo`'s component in the condition | `Repo` reads | `repo` reads |
  |---|---|---|---|
  | absent | `Repo` | recorded | replacing |
  | a symbolic link to `Repo` | `Repo` | recorded | replacing |
  | a bind mount of `Repo` (`sudo mount --bind Repo rEPO`) | `[Rr][eE][pP][oO]` | recorded | **recorded** |

  On Unix `same_entry` compares `(st_dev, st_ino)` from `lstat`, so a symbolic link is an entry of
  its own; a bind mount's root carries its source's device and inode, so it is not.
- **Why the suite does not see it.** `a_repository_whose_path_differs_from_the_managed_one_only_in_case_keeps_its_replacements`
  creates `repo` and `rEPO` as directories of their own. On CI's `test (winguest)` its
  case-sensitive half passes (`fsutil`, no alias).

## Reachability

**What a reader must do to reach it.** Create, in a directory that keeps case and holds a component
of the managed repository's canonical path, an alias named as that component with every ASCII
letter's case swapped, resolving to the component itself; and have, beside the managed repository,
another repository whose name differs from that component only in ASCII case, with replacements
installed.

- On Windows the alias is a junction (`mklink /J`) or a directory symbolic link (`mklink /D`), in a
  directory `fsutil file setCaseSensitiveInfo` has made case-sensitive. Windows documents the
  symbolic-link privilege, or developer mode, as needed for the second and not for the first; that is
  not measured here. An ordinary Windows directory folds case, and there `repo` is `Repo`.
- On Linux the alias is a bind mount, which needs root, or a mount namespace the run itself is started
  in (not measured here); this measurement used `sudo`. A symbolic link does not reach it.
- The alias can stand at any component of the canonical common directory's path, and the repository
  it exposes is the one beside that component. Since #326 round 6 the scope also asks the common
  directory about `refs`: an alias `REFS` → `refs` inside a common directory that keeps case spells
  `worktrees` in classes, which reaches only Git directories inside the managed repository's own
  common directory (`<common>/WORKTREES/*`).

**What they see.** Roles of a v0.1 run that manages `Repo` read the objects `repo` records instead
of its replacements. The run's own repository is unaffected: its roles read the recorded graph, as
they should. Ordinary use does not create an alias spelled as a path component in its other case,
which is why this is a P2 and not a merge blocker on the owner's ruling of 2026-09-28.

## What the change that takes this up should do

1. **Decide whether a directory folds case from something an alias cannot fake.** Two spellings
   resolving to one entry is not enough. One option: list the directory and treat it as folding only
   when the other spelling resolves to the entry and no entry of the directory is named with that
   spelling. A junction, a symbolic link and a bind mount's mount point are each listed under their
   own name, and a folding directory lists only one. On Windows, reading the other spelling's stored
   name (`FindFirstFileW`) should give the same answer; neither is measured. Refusing the scope when
   the other spelling is a reparse point or a mount point is the narrower alternative.
2. **Give the case-sibling witness an alias half.** On Windows, a junction `rEPO` → `Repo` in the
   directory `fsutil` makes case-sensitive, beside an independent `repo`, which must still read
   `replacing`. On Linux, a bind mount where the runner has root, or no half.
3. **Keep the inward direction.** Whatever replaces the comparison must still find a folding
   directory folding, or the scope stops matching the managed repository spelled in another case.
   The round-5 and round-6 witnesses on a case-folding directory cover that.
