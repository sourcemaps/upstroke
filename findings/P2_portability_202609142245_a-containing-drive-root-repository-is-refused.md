---
id: PR280-R14-DRIVE-ROOT-CONTAINER-REFUSED
severity: P2
disposition: deferred
category: portability
pr: 280
reviewed_sha: 364b5a212568497a017205192a9ee5fe88d73d2d
location: .github/scripts/validate-pr-branch.sh:1821
provenance: fix_regression   # review-280-regression-214915.log, round 14's regression (invariance) lens at 364b5a21
first_bad: 339a238b
guard: a change to the containment test in enclosing_work_tree that admits a work-tree root git answers with its separator on the end, with a drive-root container row among the drive-root walk rows in test-pr-policy.sh that fails before it and passes after
---

## Failure sequence

**A repository at a Windows drive root is refused as not containing a repository inside it.** Round
14's regression lens reported this on `364b5a21` (`review-280-regression-214915.log` in the
orchestrator's directory). **Its own boundary, in its own words: *"Executed function reproduction;
full Windows sequence reasoned."*** Its automatic approval refused to copy repository code to
`windowsguest` without specific authorisation. So the function-level defect is executed and the
end-to-end Windows path is argued. This filing executed the function again, on Linux, and ran nothing
on Windows, so that boundary stands as the lens drew it.

The shape, as the lens gives it: a repository at `C:/` that ignores `repo/`, and a clean nested
repository at `C:/repo` with one committed finding at its root. That finding's branch is validated
with `.`.

1. `.` names the work tree's own root, so its name in its own index is empty and the gitlink ascent
   runs: `enclosing_work_tree 'C:/repo' ...` (`:2478`).
2. `path_parent 'C:/repo'` answers `C:/`, since a root keeps its separator (rule 3, `:1245`).
3. `git -C C:/ rev-parse --is-inside-work-tree` answers `true`, and `--show-toplevel` answers `C:/`.
   The `C:/` answer, exit `0`, is the lens's capture from the Windows guest's read-only Git query. It
   was not re-run for this filing.
4. `:1821` gives a root its own arm, and only `/` gets it. The comment above it (`:1814`–`:1820`) is
   why a root needs one: *"`/` followed by a component is `//...`, which matches nothing -- a work tree
   at the filesystem root would otherwise be refused as not containing what it plainly contains."*
   `C:/` falls through to `:1825`, whose pattern `"$probe_text"/?*` is `C://?*`, and `C:/repo` cannot
   match it.
5. `enclosing_work_tree` returns 1: *"git says the work tree at 'C:/repo' is inside the one at 'C:/',
   which does not contain it ... was not judged."* A legitimate containing repository is refused. This
   is a false red, not a false green.

Sequence: `.` at `C:/repo` under a work tree at `C:/` -> the ascent asks about `C:/` -> git answers
`true` and `C:/` -> `:1821`'s root arm matches only `/` -> `:1825`'s `C://?*` does not match
`C:/repo` -> return 1 and the branch is not judged.

**This pull request added the check, and it is separate from the disclosed native-path
component-chain defect.** That defect is `locate_listing`'s component chain reading a native `C:/...`
spelling *written by the caller* as a relative one; the body records `.` answering there. Here the
caller writes `.`, and the native root comes from git's own `--show-toplevel` answer during the
ascent.

Executed 2026-09-14 on Linux, with git's answers supplied by a stub `git_probe`.
`drive-root-harness.sh` extracts `path_parent` (`:1425`–`:1458`) and `enclosing_work_tree`
(`:1757`–`:1836`) byte for byte from the named revision. The stub answers only the questions in its
table and exits `99` on any other. Return status of `enclosing_work_tree <root> .`:

| case | root | stubbed `--show-toplevel` | `364b5a21` | `:1825` admits a root ending in `/` |
|---|---|---|---|---|
| drive root (the lens's shape) | `C:/repo` | `C:/` | **`1`**, *"'C:/', which does not contain it"* | `0`, `enclosing_root='C:/'` |
| drive root, one level down | `C:/x/repo` | `C:/` | **`1`** | `0` |
| POSIX root | `/repo` | `/` | `0` | `0` |
| ordinary Windows parent | `C:/work/repo` | `C:/work` | `0` | `0` |
| UNC share | `//server/share/repo` | `//server/share` | `0` | `0` |
| answer on another drive | `D:/repo` | `C:/` | `1` | `1` |

The change in the last column is one line: `:1825`'s `"$probe_text"/?*)` becomes
`"${probe_text%/}"/?*)`. The last row is a control added for this filing: an answer that does not
contain the root is still refused.

The gate's own harness for this walk (`test-pr-policy.sh:2727`–`:2773`) answers the same way:
`rc=1 root= asked=1 at=[C:/]` for the drive-root container at `364b5a21`, and `rc=0 root=C:/ asked=1
at=[C:/]` with the one-line change. Its POSIX-root, Windows-parent and UNC-share controls answer `rc=0`
both times.

**No fixture pins it.** The existing walk rows (`:2782`–`:2838`) never put a work tree *at* a drive
root. Complete-gate runs, the modified ones in scratch copies:

| gate | validator | complete gate exit |
|---|---|---|
| unmodified | `364b5a21` | `0` |
| unmodified | `:1825` changed | `0` |
| plus one row after `:2805`: `walk_case 'a work tree at a drive root contains the one below it' 'C:/repo' 'C:/' 'rc=0 root=C:/ asked=1 at=[C:/]'` | `364b5a21` | **`1`**, failing on that row |
| plus the same row | `:1825` changed | `0` |

**First bad `339a238b`**, measured by running the harness at every commit that touched the validator
from `04432736` to `364b5a21`. `339a238b` added `enclosing_work_tree` with this check (`git blame`
of `:1807`–`:1836`); `231c1aad` and `04432736` have no such function. The one-level-down case returns
`1` from `339a238b` on. The lens's exact `C:/repo` shape reaches the pattern only from `3403b7fb` on.
From `339a238b` to `a870321d` the walk asked `git -C C:` for it instead, which is the separately
recorded and fixed `DRIVE-ROOT-LOSES-ITS-SEPARATOR`, and which this stub does not model (exit `99`).

Evidence: `~/findings-sweep/orch-p1/evidence-file-280-p2s-364b5a21/` (`drive-root-harness.sh`,
`out/harness/`, `out/repo-walk-harness-results.txt`, `add_drive_root_row.py`,
`out/mutation-validator-1825.diff`, `out/gate-head-*`).

## What the change that takes this up should do

Compare containment in a way that handles a root answer ending in its separator, and add the missing
affirmative fixture. In the lens's words: *"Use a containment comparison that handles roots ending in
a separator, and add the missing affirmative drive-root fixture."*

Stripping one trailing `/` from the answer before appending the separator, as measured above, admits
`C:/` and still refuses an answer on another drive. The fixture is the one row above, added among the
drive-root walk rows: it fails at `364b5a21` and passes with the change.
