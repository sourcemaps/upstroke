---
id: ADD-REFUSAL-WALL-BOUND-TIMES-THE-WHOLE-CALL
severity: P3
disposition: deferred
category: correctness
pr: TBD
reviewed_sha: d431d220f04a8fafd277b9bc55f4c113822167be
location: src/workspace_manager/tests.rs:18465
provenance: pre_existing
first_bad: 58c7c203
guard: the test-only pull request, on its own claim branch, that records the duration of the add's one Git attempt beside the refusal and bounds the rest of the call by it, changing no production line
---

# An add refusal's wall bound times the whole call

## Failure sequence

Read from the test as `CI-FLAKE-WINDOWS-ADD-REFUSAL-PAST-REGISTRY-DEADLINE`'s repair leaves it,
not run.

    an add whose checkout cannot be made is refused after one attempt, with no wait of the add
       slept
    -> on a host slow enough, the whole `add_worktree` call takes ten seconds or more
    -> the wall assertion, `took < PRODUCTION_REGISTRY_DEADLINE`, fails
    -> nothing in the failure says which part of the call took the time

The test is
`workspace_manager::tests::an_add_whose_checkout_cannot_be_made_refuses_after_one_attempt_and_leaves_nothing`
(`src/workspace_manager/tests.rs`). It runs two cases, `git-path` and, on Unix only,
`name-too-long`. Since that repair its add runs under the production registry deadline,
`PRODUCTION_REGISTRY_DEADLINE`, 10 s, and "at once" is judged by the add's own waits. After the
add it asserts, in this order:

1. the attempt assertion: `contended_attempts` for the repository is unchanged, "one attempt,
   never attempted again";
2. the slept-wait assertion: `slept_pauses`, the thread-local count of slept waits, is unchanged,
   "no wait of the add slept";
3. the wall assertion: `took < PRODUCTION_REGISTRY_DEADLINE`, "refused at once, not at its
   deadline";
4. the refusal's message says `1 attempt(s)` and `is gone`;
5. nothing is left at the slot.

Ruling r113 moved the third clause of `CI-FLAKE-WINDOWS-ADD-REFUSAL-PAST-REGISTRY-DEADLINE`'s
guard, recording the attempt's own duration beside the refusal, to this finding.

## Both sightings

Both are reds of the wall assertion as it was before that repair, in the `git-path` case on a
GitHub-hosted `windows-latest` runner. That assertion was `took < REGISTRY_ACCESS_DEADLINE`, and
that deadline is 500 ms in test builds. They are copied from the sections "The one sighting" and
"A second sighting, 2026-11-27" of
`findings/P3_correctness_202611211600_an-add-refused-at-once-took-three-seconds-on-hosted-windows.md`,
as that file states them.

| run | job | date | the add took |
|---|---|---|---|
| `44740932431` (`pull_request`) | `136304158953`, `test (windows-latest, 5 of 5)` | 2026-11-20 | 3.3885434 s |
| `46020861627` (`pull_request`), attempt 1 | `140579864108`, `test (windows-latest, workspace 2 of 2 (guarded_reads, manager, reclaim, refs, registry, snapshots))` | 2026-11-27 | 532.4028 ms |

- **2026-11-20.** The raw job log, `evidence/job-44740932431-136304158953.log`, SHA256
  `9ba03189797e33f292101dc466ed718d0a6de42aeced1c034ea9fa04809c7af0`. The test panicked with
  `git-path: refused at once: 3.3885434s`.
- **2026-11-27.** The job's log as GitHub's API serves it, with ANSI escape sequences removed,
  `~/selfhost-findings-20261128/evidence/job-46020861627-140579864108.log`, SHA256
  `c7c69b75be18ecd8d57caaa347f9f67f2b953de3192e2e63abdb501ea888ddb0`. The test panicked with
  `git-path: refused at once: 532.4028ms`.
- **The same checkout passed.** Attempt 2 of run `46020861627`, under ruling r86, re-ran that job
  on the same checkout, `c91f148`, as job `140607675103`, where the test was `... ok <1.445s>`
  (line 702). Its log:
  `~/selfhost-findings-20261128/evidence/job-46020861627-140607675103.log`, SHA256
  `4bae152bb84a904c9843568f07819ee735d1a28092073059d34337d79356fd88`.

In both reds the attempt assertion, which runs before the wall assertion, passed. Neither log
records the attempt's own duration or whether any wait of the access slept.

## What a red of the wall assertion cannot say

`took` is measured on the monotonic clock, from just before `add_worktree` is called to just after
it returns, so the wall assertion times the whole call. The deadline the add runs under bounds its
waits and its further attempts, not the runtime of an attempt or of its veto. The test's doc says
so, and so does `tolerant_registry_access`'s, under "The bound, end to end".

The attempt and slept-wait assertions run before the wall assertion, so a red of it comes with
both passing. It says that the call took ten seconds or more, with no wait of the add slept and
one attempt made. The message and slot assertions run after it, and such a red does not reach
them.

It cannot say where in the call the time went: in the gate's `git worktree list` child
(`WorkspaceManager::revalidate`), in the one `git worktree add` child, in the veto
(`Destination::untouched`), in the settling (`Destination::settle`), or with the test's thread
not scheduled. Nothing records the attempt's own duration.

## The narrowing

Before that repair the wall bound was the test build's 500 ms (`took < REGISTRY_ACCESS_DEADLINE`).
Since the repair, a delay in the add that is not a registry wait, and keeps the add under ten
seconds, passes the test. The test's doc states this, under "A guarantee this test no longer
makes".

## A later red is recorded here

A later red of the wall assertion is a sighting of this finding, and is recorded in this file. The
assertion's message is `{label}: refused at once, not at its deadline: {took:?}`.

## What the change that takes this up should do

It keeps the test's assertions as they now are, with no retry, no skip and no weaker assertion.

It makes the duration of the add's one Git attempt observable, recorded beside the refusal.
