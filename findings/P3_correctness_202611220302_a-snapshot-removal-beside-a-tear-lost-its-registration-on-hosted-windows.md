---
id: CI-FLAKE-WINDOWS-WORKSPACE-TEAR-REGISTRATION-GONE
severity: P3
disposition: deferred
category: correctness
pr: TBD
reviewed_sha: fe2b9bc58af6c3eae0d0a9da64ac25a159db374f
location: src/workspace.rs:5821
provenance: undetermined
first_bad:
guard: the change that makes this test's failure report the registration's state and the writer's end beside the removal, meaning the removal access's result, its attempts and when the writer finished the tear relative to them, or any change to this test; keeping the assertion that the snapshot's registration is gone
---

# A snapshot's removal beside a tear its writer finished left the snapshot registered on a hosted Windows runner

## Failure sequence

    the test makes a gate snapshot and plants a torn foreign registration beside it
    -> a writer thread waits for the registry access to fail once on the tear, then finishes the tear
    -> the snapshot is dropped, and its removal runs as a tolerant registry access
    -> the writer finished (the removal failed on the tear at least once), and the snapshot's directory is gone
    -> `git worktree list --porcelain` still lists the snapshot, as prunable
    -> the assertion at `src/workspace.rs:5821` fails: "the snapshot's registration is gone"

`workspace::tests::a_snapshot_removal_beside_a_tear_its_writer_finishes_takes_the_registration`
(`src/workspace.rs:5797`) is T-L4 of follow-up D (`docs/internals/workspace.md`, the note on this
test). It:

1. makes a gate snapshot in a private store (`:5803-5806`);
2. plants a torn registration, `foreign-remove`, in the repository's `.git/worktrees/`: a `gitdir`
   and an empty `commondir`, with no `HEAD` (`plant_a_torn_registration`, `:5548`);
3. starts `OnceContended` (`:5564`), a thread that polls `contended_attempts` every 1 ms, for up to
   60 s. Once the count moves, meaning one registry access has failed and decided to attempt again,
   it finishes the tear: it writes `HEAD` and then `commondir` (`finish_the_registration`, `:5559`);
4. drops the snapshot (`:5812`) and joins the writer (`:5813`);
5. asserts, in this order: the writer finished (`finished.is_some()`, `:5814`); the snapshot's
   directory is gone (`:5818`); `git worktree list --porcelain` lists no `upstroke-gates-` entry
   (`:5821`); the store's `intents` directory is empty (`:5825`).

The drop is `impl Drop for GateWorkspace` (`:2664`). It calls `cleanup_gate_workspace` (`:2467`)
and **discards its result**: an `Err` only skips the ephemeral store's removal. The cleanup runs
the removal and its list as one attempt of `tolerant_registry_access`
(`src/workspace_manager.rs:2512`), with a veto that always answers `Again::Attempt`. The attempt
runs `git worktree remove --force <path>` and then `git worktree list --porcelain -z`. It succeeds
when the list does not register the path (`worktree_is_registered`, `:2593`), whatever the
removal's exit status. Only after the access succeeds does the cleanup run `remove_dir_all` on the
snapshot's directory and remove its intent (`:2508-2519`). In test builds the access's deadline is
**500 ms** (`src/workspace_manager.rs:9502`); in production it is 10 s (`:2319`).

The assertion takes a listed `upstroke-gates-` entry to mean that the removal did not take the
snapshot's registration. Its message prints the listing. It does not print the cleanup's result,
how many attempts the access made, what each removal said, or when the writer finished the tear
relative to those attempts. The drop has already discarded the cleanup's result, so the test cannot
print it today.

## The sighting

| run | job | platform | date | pull request | head |
|---|---|---|---|---|---|
| `44983136029` (`pull_request`) | `137087841949`, `test (windows-latest, 10 of 10)` | a GitHub-hosted `windows-latest` runner | 2026-11-22 | #399 | `2e72fba8`, an empty commit whose tree is `144485c7`'s |

Read from the job's log as GitHub's API serves it, with ANSI escape sequences removed (`evidence/job-137087841949.log`, SHA256
`ec4fa44094f98ccde842958734d710f5256eb2b53f721253665dbe84bac29ea2`):

- Line 2410: `test
  workspace::tests::a_snapshot_removal_beside_a_tear_its_writer_finishes_takes_the_registration
  ... FAILED`.
- Line 2461: the test panicked at `src\workspace.rs:5821:9`. Lines 2462-2474 give the message in
  full:

      the snapshot's registration is gone: worktree C:/Users/runneradmin/AppData/Local/Temp/upstroke-legacy-remove-tear-SZGYRRRJ3T/repo
      HEAD 959e2a3c9b42b007f492676aa77daef00b111197
      branch refs/heads/main

      worktree C:/Users/runneradmin/AppData/Local/Temp/upstroke-legacy-remove-tear-SZGYRRRJ3T/gate-store/worktrees/upstroke-gates-7068-01M80Q6Z1BYBCG32G15T2J4HWF
      HEAD 9430a5f7270444ad7b8897dbe33de740099a200f
      detached
      prunable gitdir file points to non-existent location

      worktree C:/Users/RUNNER~1/AppData/Local/Temp/upstroke-legacy-remove-tear-SZGYRRRJ3T/foreign-remove-checkout
      HEAD 959e2a3c9b42b007f492676aa77daef00b111197
      detached
      prunable gitdir file points to non-existent location

- Line 2482: `test result: FAILED. 2020 passed; 1 failed; 39 ignored; 0 measured; 1607 filtered
  out; finished in 468.68s`. This test was the job's only failure.

What the listing shows:

- **The snapshot is still registered, and its directory is gone.** Its entry is marked `prunable
  gitdir file points to non-existent location`: the registration in `.git/worktrees/` remains, and
  the checkout its `gitdir` names does not. That agrees with the assertion at `:5818`, which passed.
- **The tear was finished.** The foreign entry has a `HEAD`, which only the writer writes, and
  `finished.is_some()` at `:5814` passed. So at least one removal attempt failed and the access
  decided to attempt again. The foreign entry is prunable because the fixture never makes its
  checkout; that is expected, and the assertion does not look at it.
- **Two spellings of the temp root.** The foreign entry is spelled `C:/Users/RUNNER~1/…`, the 8.3
  short name the fixture wrote into its `gitdir` (`:5551-5554`). The repository and the snapshot
  are spelled `C:/Users/runneradmin/…`. The log does not show how Upstroke spelled the snapshot's
  path.

**The same tree passed this test twice.** In CI run 44968824584's job 137043887481, `test
(windows-latest, 10 of 10)` on `144485c7`, it passed. In CI run 44975798003's job 137065317614, the
same leg on `fe4bfa4e` (an empty commit on that tree), it passed too; that job failed on another
test, recorded as `CI-FLAKE-DISPATCH-SAMPLER-NO-KILL-WHILE-WRITING`
(`findings/P3_correctness_202611220301_the-dispatch-sampler-landed-no-kill-while-writing-on-hosted-windows.md`).
`2e72fba8` is an empty commit whose tree is `144485c7`'s.

**#399 does not touch `src/workspace.rs`** (ruling r36).

The test source was read at `fe2b9bc5`, where the assertion is at line 5821, the line the panic
names.

## What is and is not established

**Established.** In one job on a hosted Windows runner, a gate snapshot's drop beside a torn
registration ended with the snapshot's directory gone and its registration still listed. The
removal had failed at least once and the writer had finished the tear. The test failed on that
assertion, and it was the job's only failure. #399 does not touch `src/workspace.rs`, and the same
tree passed this test in the two runs before.

**Not established: why the registration stayed.** There are two readings. Each is inferred, read
from the source, and not run. The log does not tell them apart.

Both start from one fact of the source. The cleanup removes the snapshot's directory itself only
after the access succeeds (`:2508`). So either the access failed and Git's own removal took the
directory and not the registration, or the access succeeded while Git still registered the path.

1. **The access refused at its 500 ms deadline, and the drop discarded the refusal.** *Inferred,
   read from source, not run.* A `git worktree remove --force` deleted the snapshot's directory but
   not its entry in `.git/worktrees/`: it failed on the tear after the directory was gone, or it
   could not delete the entry on Windows. Each attempt spawns two Git children, and the writer
   finishes the tear only after the first failed attempt is counted. If one or two attempts used the
   500 ms, the access returned `RegistryRefused` (`src/workspace_manager.rs:2562-2570`) before an
   attempt ran past the finished tear. Or the later attempts could not take a registration whose
   directory was already gone. The drop discarded the refusal (`:2672`), and the cleanup never
   reached the intent. The log does not say how long this test's Git children took. Which step of Git's
   removal the tear fails, and whether Git removes a registration whose directory is gone, were not
   read here.
2. **The access succeeded while Git still registered the snapshot.** *Inferred, read from source,
   not run.* An attempt's list did not match the snapshot's path, so the attempt counted as success,
   and the cleanup then removed the directory with `remove_dir_all` and removed the intent. Then
   the test's own list found the entry. On Windows, `git_path_field_matches` (`:2634-2638`) compares
   Git's field, with `/` turned into `\`, to the path as Upstroke spelled it, and the listing shows
   two spellings of the temp root on this runner. *Against it:* the test passed on this tree twice,
   and a removal attempt failed at least once in this run (`:5814`). A list that never matches would
   have counted the first attempt as success, the writer would never have fired, and the test would
   have failed at `:5814`. So this reading needs a match that failed only on some attempts, or a
   first attempt that failed with the list's own error. Nothing in the log shows either.

If the second reading holds, Upstroke deleted a registered checkout's directory and left its
registration behind, in production code. Severity should then be reconsidered.

**What would establish each.** The failure would print the cleanup's result: `Ok`, or the refusal
with its attempt count and last failure. The drop discards that result today. It would also print,
for each attempt, the removal's exit status and stderr, and whether its list matched the snapshot's
path. It would print when the writer finished the tear, relative to those attempts and to the
access's deadline, and the snapshot's path as Upstroke spelled it. Under the first reading the
cleanup returned a refusal, the intent remains, and a removal's stderr names the failure. Under the
second, the cleanup returned `Ok`, the intent is gone, and an attempt's list did not match a path
Git still listed. The intent assertion at `:5825` would separate the two on its own. It runs after
the failed assertion, so this run never reached it.

**Not established: the rate.** One sighting. Other runs of this test were not counted.

## Related, not the same

- **`PR329-LEGACY-RUNS-IN-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY`**
  (`findings/P1_correctness_202610020230_legacy-runs-in-linked-checkouts-race-the-shared-worktree-registry.md`)
  is the finding this test guards. It names it among follow-up D's regression tests for (e1) and
  (e2), a write in flight (`:169-174`), and names `cleanup_gate_workspace` among the accesses that
  died on a half-written entry (`:25`). This sighting is the regression test failing once on a
  hosted runner, not a recurrence that finding records. It is a relative, not a parent.
- **`CI-FLAKE-WINDOWS-ADD-REFUSAL-PAST-REGISTRY-DEADLINE`**
  (`findings/P3_correctness_202611211600_an-add-refused-at-once-took-three-seconds-on-hosted-windows.md`)
  records one whole `add_worktree` call taking 3.39 s, in another job on 2026-11-20, against the same 500 ms test deadline. It marks as inferred, not established, that the time was spent in the `git worktree add` attempt. That is a different test, with a different assertion: a wall-time bound on
  an add. Whether the two share a cause is not established.

Both ids were found as written. No other finding names this test or its tear fixture.

## What the change that takes this up should do

- **Make the failure report the registration's state and the writer's end beside the removal**,
  as above. That needs the cleanup's result where the test can read it, which the drop discards
  today. Then decide the reading from the next sighting.
- **If the access refused at its deadline**, decide whether the test deadline or the test's
  removal is the thing to change. A schedule that holds only where the runner is fast is not
  enough.
- **If the access succeeded while Git still registered the path**, fix the success decision in
  production code, and reconsider severity.
- **Keep the assertion** that the snapshot's registration is gone. Do not retry or skip the test,
  and do not weaken the assertion, to get past this.

**A proposal, not adopted and not reviewed:** the failure message gives the cleanup's result. For
an `Err`, that is the refusal's text, which names the attempt count and the last removal's words.
It gives each attempt's removal exit status and stderr, and whether its list matched the path. It
gives the time the writer finished the tear, measured from the access's start beside each attempt's
start and the deadline. It gives the snapshot's path as Upstroke spelled it, and whether the intent
still exists. The listing stays in the message. There would be no retry, no skip and no weaker
assertion.
