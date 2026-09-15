---
id: PR280-R14-MATERIALISED-LINK-FIXTURE-ABORTS-WITHOUT-SYMLINKS
severity: P2
disposition: deferred
category: portability
pr: 280
reviewed_sha: 364b5a212568497a017205192a9ee5fe88d73d2d
location: .github/scripts/test-pr-policy.sh:3740
provenance: fix_regression   # review-280-fix-check-211125.log, round 14's fix-check lens at 364b5a21
first_bad: a02c580e
guard: a change to the materialised-deployment fixture that builds its 120000 entry without creating a symlink on disk, with the complete gate exiting 1 under symlink denial before it and 0 after
---

## Failure sequence

**Where a symlink cannot be made, the gate stops at this fixture instead of skipping it.** Round 14's
fix-check lens found this on `364b5a21` (`review-280-fix-check-211125.log` in the orchestrator's
directory: *"Captured gate exits: master `0`, head `1`"*, under
`strace -f -e inject=symlinkat:error=EPERM:when=1+`). It was executed again for this filing, at the
same head, with that instrument and a second one, and it reproduces.

The gate runs under `set -euo pipefail` (`:2`). Its symlink cases sit behind a probe (`:1194`–`:1195`)
whose comment says why: the suite is run by hand on all three platforms, and *"Windows refuses a
symlink without developer mode; a printed skip says more than a red that is about the checkout rather
than about the gate."* The materialised-deployment fixture `a02c580e` added (`:3737`–`:3772`) is not
behind that probe:

1. The probe's `ln -s` fails, and the three symlink sections before `:3740` print their skip notes.
2. `new_repo "$mat_wt"` (`:3739`) succeeds.
3. `ln -s P2_correctness_202609100005_a-materialised-link.md "$mat_wt/findings"` (`:3740`) exits `1`.
4. `set -e` ends the gate with status `1`. The fixture's own skip guard (`:3745`) comes after the
   `ln -s` and never runs. Nothing after it runs either: not the materialised rows, not the
   selected-index section from `:3774`, not the rest of the gate. Stdout is empty; none of the gate's
   `fixtures passed` lines is printed.

Sequence: symlink creation unavailable -> the probe fails and the earlier symlink sections skip ->
`ln -s` at `:3740` exits 1 -> `set -e` exits the gate 1 -> no case after `:3740` runs.

Executed 2026-09-14, 22:35–22:43Z, on Linux (git 2.43.0, bash 5.2.21). Each run invoked the complete
gate by path from its tree's root, as CI does. `364b5a21` is this worktree, with `git status
--porcelain` empty before and after every run; the other trees are `git archive` copies. Two
instruments made symlink creation unavailable:

- **seccomp:** `nosymlink_exec.py` installs a filter under which `symlink(2)` and `symlinkat(2)` fail
  with `EPERM` in every process, then execs the gate. Its self-test shows `symlink` failing with
  `EPERM` while a hard link still succeeds.
- **strace:** the lens's own injection, `strace -f -e inject=symlinkat:error=EPERM:when=1+`, with the
  trace written to a file of its own.

| tree | symlink creation | complete gate exit |
|---|---|---|
| `364b5a21` | available | `0`, empty stderr |
| `364b5a21` | denied, seccomp | **`1`** |
| `364b5a21` | denied, strace | **`1`** |
| `231c1aad` (merge base) | denied, seccomp | `0`, 3 skip notes |
| `3bfac430` (`a02c580e`'s parent) | denied, seccomp | `0`, 4 skip notes |
| `a02c580e` | denied, seccomp | **`1`** |

Every `1` ends its stderr with the same line:
`ln: failed to create symbolic link '<fixture_dir>/materialised-deployment/wt/findings': Operation not permitted`.
Under `bash -x` with the seccomp filter, the last command before the exit trap is
`+test-pr-policy.sh:3740: ln -s P2_correctness_202609100005_a-materialised-link.md <fixture_dir>/materialised-deployment/wt/findings`.
The strace run records exactly two `symlinkat` calls, the probe's and `:3740`'s, each
`-1 EPERM (Operation not permitted) (INJECTED)`.

The two instruments differ in one respect. The lens's injection covers `symlinkat` alone, so `git
init`'s own capability check, `symlink("testing", ...)`, still succeeded, 62 times. The seccomp filter
denies both calls, as a platform without the privilege does. The gate exits `1` either way.

**Boundary.** This is Linux syscall denial. Native Windows was not run, by the lens or for this
filing. The lens's words: *"This execution used Linux syscall injection; native Windows was not run."*

Evidence: `~/findings-sweep/orch-p1/evidence-file-280-p2s-364b5a21/` (`run-gate.sh`,
`nosymlink_exec.py`, `out/gate-*/`).

## What the change that takes this up should do

Build the fixture's `120000` entry without a symlink on disk, which is the lens's own proposal: write
the target text as a blob (`git hash-object -w --stdin`), add it with
`git update-index --add --cacheinfo 120000,<blob>,findings`, commit, set `core.symlinks false`, and
check `findings` out. Everything from `:3745` on stays as it is.

Executed in a scratch copy of `364b5a21` with only `:3740`–`:3744` rewritten that way
(`mutate_fixture_3740.py`, `out/mutation-fixture-3740.diff`):

- symlink creation denied (seccomp): complete gate exit **`0`**, and the materialised case printed no
  skip note, so its rows ran and passed;
- symlink creation available: exit `0`, empty stderr.

That also shows `:3740` is the only place the gate aborts under denial. Moving the fixture behind the
probe would stop the abort too, but it would drop these rows exactly where no symlink can be made,
which is where git checks a `120000` entry out as a regular file and the rows have something to test.
