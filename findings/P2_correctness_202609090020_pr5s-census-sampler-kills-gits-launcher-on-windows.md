---
id: PR249-KILL-SAMPLER-WINDOWS-WRAPPER
severity: P2
disposition: deferred
category: correctness
pr: 249
reviewed_sha: 56ea88c9779f7787800bffbd6c64011c069edbc8
location: src/workspace_manager/tests.rs:9673
provenance: pre_existing
first_bad: PR5's four-command residue kill sampler (`SampledChild::spawn`), which spawns `git` by name; every kill it has delivered on the Windows lane since has ended Git for Windows' launcher, not git
guard: the PR9 and T-ATTEMPT samplers' fixture spawns the real binary (`fixture::sampled_git`) and the PR9 sampler's `KilledSample::while_writing` floor fails when no kill landed mid-write; PR5's census sampler has no such floor and is the open half
---

## Failure sequence

A kill sampler on the Windows lane spawns `git` by name and kills the child at a chosen moment.

    Command::new("git") on Windows -> resolves Git for Windows' cmd\git.exe, a 46 KB launcher
    -> the launcher starts the real mingw64\bin\git.exe as its own child and waits for it
    -> Child::kill terminates the launcher; the real git is never signalled
    -> the pick either had not begun (residue None) or runs on to completion (After, MERGE_MSG in place)
    -> the sampler records a kill that interrupted nothing, and every recovery it then proves is a
       recovery from a state a kill never left

Measured on the winguest lane at `56ea88c9`, by the PR9 repair-materialization sampler before its
fixture was changed: 30 kills in 32 spawns, ten `None`, twenty `After` with `MERGE_MSG` written,
none between the pick's first write and its last. On Linux the same sampler's kills land mid-write
about one time in six.

## What this pull request changed, and what it did not

`KillableGitChild::spawn` (`src/workspace_manager/fixture.rs`), the fixture of the PR9
materialization sampler and of the T-ATTEMPT two-command sampler, now spawns the real binary:
`git --exec-path` names `<prefix>/mingw64/libexec/git-core`, and `<prefix>/mingw64/bin/git.exe`
beside its DLLs is the child (`fixture::sampled_git`, resolved once; `git` where that layout is
absent). The PR9 sampler's floor — at least one kill while the pick was writing — therefore binds on
Windows too.

PR5's four-command census sampler, `SampledChild::spawn` in `src/workspace_manager/tests.rs`, still
spawns `git` by name. Its kill floors count "kills that landed" by exit status, which the launcher's
death satisfies, so its Windows histogram has been describing the launcher's death, not git's. This
is reasoned from the same spawn, not measured on that sampler.

## Why it is deferred, and what is and is not frozen

`effects/residue-classes.json` freezes the census's **declarations** — per site, the residue
elements and classes, and the sampling N — and says of the observed histogram that it "is
machine-varying and cannot be pinned in a file compared byte-for-byte", so it is emitted to
`effects/residue-histogram.json` on every run and checked only for accounting for every sample.
Routing `SampledChild::spawn` through `fixture::sampled_git` therefore changes no declaration and
no frozen count: the Windows histogram would change, and that file is not frozen. What defers the
change is its scope, not a frozen contract: the census is PR5's evidence, its samples cover four
commands (`worktree add`, `add -A`, `write-tree`, `cherry-pick`), and kills that reach git on
Windows for the first time need the guest to show every one of those commands' recoveries against
residue a launcher's death never left — a measurement with its own review, which this pull request
did not make and does not claim. (The first version of this file said the census's evidence record
was "PR5's frozen contract" and that the owner decides what its Windows samples must show; PR #249's
third-round record review read the evidence file and corrected it.)

## What the change that takes this up should do

Route `SampledChild::spawn` through `fixture::sampled_git`, run the census on the Windows guest,
and re-read its Windows kill floors against kills that now reach git, recording the histogram those
kills produce for each of the four commands. If a later pass labels this P1, the disposition
becomes escalate-to-owner rather than still-deferred, because the census's evidence is PR5's and a
change to what it shows on Windows is the owner's to accept.

## A second fingerprint, on PR11 (pull request #327, 2026-09-30)

CI run 36767206421 at `4bb04a6568377435cf9d47ebc9883fad9204e7df`, job `test (winguest)` 110064296977,
failed this sampler alone — `workspace_manager::tests::sampled_git_child_kills_every_residue_classified_and_recovered`,
in its `Worktree.Add` sample:

    forced removal converges: Io { path: "C:\Users\Administrator\AppData\Local\Temp\upstroke-sample-add-B5WFK90B4N\repo\.git\worktrees\kalpha-g3\locked",
        source: Os { code: 5, kind: PermissionDenied, message: "Access is denied." } }

That is a new fingerprint of this row, not a new row: the sampler's forced removal could not delete
the `locked` file of the registration a `git worktree add` creates, which fits a real git still
running the add after the kill ended its launcher — the reading is consistent with this file's
failure sequence and not established by the log. The round that pushed `4bb04a65` changed no
`workspace_manager` path; the same test passed on the same leg at `8eae559a` (run 36757374671) and
at `4bb04a65` on ubuntu, macOS and the build box's gates; and the re-run at the same head, run
36769385877, passed every leg, `test (winguest)` included. Attribution and logs: the build box's
`~/orch-pr11/logs/pr11_repair_r3/ci/red-leg-attribution.txt` and the job logs beside it; the re-run's
jobs, `~/orch-pr11/logs/pr11_impl_g/findings/run-36769385877-jobs.tsv`. Nothing here changes the
change that takes this up (above).

## A third fingerprint, 2026-11-12

The same test, `workspace_manager::tests::sampled_git_child_kills_every_residue_classified_and_recovered`,
in the `test (winguest)` job (the self-hosted Windows guest), panicked at
`src\workspace_manager\tests.rs:13274:9` with the message `every sample recovered by its classified
action` — the sampler's assertion that each sample's classified recovery succeeded
(`record.recovered`), reached after the assertions that every sample was accounted for and that none
was unclassified had passed. It did so three times on 2026-11-12, each time the only failing test of
its job, on pull requests none of which changes the sampler (#375 changes comments elsewhere in the
same test file, below):

- CI run 43221659775, job 131194404548, pull request #366 at `583e7e6b`: 3369 passed, 1 failed.
- CI run 43234617706, job 131238552041, pull request #373 at `72ebee2f`: 3388 passed, 1 failed.
- CI run 43289831147, job 131432857144, pull request #375 at `274431d0`, the run checking out
  `35d068bc`, GitHub's merge of that head into `master`: the same panic.

In the first, the log's per-site summary showed:

    Object.CandidateStage       n=8 none=0 internal=1 after=7 unclassified=0
    Object.CandidateWriteTree   n=8 none=7 internal=1 after=0 unclassified=0
    Object.ProposalCherryPick   n=8 none=0 internal=3 after=5 unclassified=0
    Worktree.Add                n=8 none=1 internal=0 after=7 unclassified=0

The failing site was `Worktree.Add`, the site of this row's second fingerprint. The test loops over
`SAMPLED_SITES` in the order `CandidateStage`, `CandidateWriteTree`, `ProposalCherryPick`,
`Worktree.Add` (`src/workspace_manager/tests.rs`, near line 14125); each iteration runs
`sample_site`, prints that site's `n=… none=…` line (near line 13201), and then asserts on that
site, ending with `record.recovered` (near line 13274), so a failing `record.recovered` ends the
loop. The log holds all four summary lines, so the first three sites passed their assertions and
the fourth, `Worktree.Add`, failed. Lines 13100-14200 of `src/workspace_manager/tests.rs`, which
hold the test (near line 13196), `sample_site` (near line 13933) and `SAMPLED_SITES` (near line
14125), are byte-identical at `583e7e6b`, `72ebee2f`, `274431d0` and `9992447b`. The file as a
whole differs at `274431d0`, where #375 changes only comments, in another test
(`the_temporary_object_scan_resolves_case_aliases_as_the_filesystem_does` and the constants and
helpers above it, from near line 15199 down). The third run checked out
`35d068bc086afc6fd93de07b441991f4e4c845c0`, GitHub's merge of `274431d0` into `master` at
`2b16cec2` (its two parents); in that merge `src/workspace_manager/tests.rs` is blob `7a732f18`,
the same blob as at `274431d0`, so the sampler's lines 13100-14200 are byte-identical in the tree
the run tested too. No rate is given: the number of guest jobs that ran this test that day
was not counted for this record.

This is a third fingerprint of this row, not a new row: a recovery that fails is consistent with
this file's failure sequence — a real `git` still running after the kill ended its launcher — and
that reading is not established by the logs. The guest was heavily loaded that day. Reasoned from
the code and not run: on Windows the kill reaches only the direct child
(`kill_process_group` is `child.kill()`, `src/workspace_manager/fixture.rs`, near lines 3596-3599)
and `await_group_end` returns at once (near lines 3636-3639), so the children `git worktree add`
itself starts are neither killed nor waited for, and routing the spawn through
`fixture::sampled_git`, the change prescribed above, is not shown to cure this fingerprint.

