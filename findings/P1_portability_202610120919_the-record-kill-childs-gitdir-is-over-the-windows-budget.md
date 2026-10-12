---
id: PR334-RECORD-KILL-CHILD-GITDIR-OVER-THE-WINDOWS-BUDGET
severity: P1
disposition: deferred
category: portability
pr: 334
reviewed_sha: 697de9c6ca35dcb93d34403efc83bbf6321bd11d
location: src/engine/topology/coordinator.rs:19605
provenance: introduced_by_feature
first_bad: 697de9c6ca35dcb93d34403efc83bbf6321bd11d
guard: this change, branch fix-P1/portability_the-record-kill-childs-gitdir-is-over-the-windows-budget, which files this finding and repairs it in the same pull request
---

## Failure sequence

O14's two charged-record kill tests,
`engine::topology::coordinator::tests::a_kill_before_the_charged_record_leaves_the_verification_to_recovery_with_unknown_spend`
and `…::a_torn_charged_record_is_truncated_and_recovered_as_before_it`, run the kill child
`charged_record_kill_child` (`src/engine/topology/coordinator.rs:19596`) with `TMPDIR`, `TMP` and `TEMP` pointing at
a scratch tree the parent holds (`killed_at_the_record`: `kill_dir("tmp")` through `scaffold::child_temporary_of`).
The child builds its fixture inside that tree under the 27-byte tag `coordinator-o14-record-kill` (`:19605`).

1. The Windows suite of the required `upstroke-ci` context runs on pull requests and pushes on the self-hosted
   guest, `test (winguest)`, whose temporary directory is `C:\Users\Administrator\AppData\Local\Temp`: 41 bytes.
2. The child's first verification adds a reviewer snapshot worktree at
   `<temporary>\upstroke-tmp-<10>\upstroke-coordinator-o14-record-kill-<10>\private\workspaces\<repo key>\01KZSWEEP00000000000000001\snapshots\k0-g0-a1-review0_<instance tag>`.
   Its `$GIT_DIR`, as Git for Windows would be handed it, is the temporary directory plus 180 bytes: 221.
3. `WorkspaceManager::add_worktree` refuses that add on Windows before any registry access
   (`refuse_git_dir_over_budget`, called at `src/workspace_manager.rs:3713`, against `WINDOWS_GIT_DIR_BUDGET`, 220,
   Git for Windows' `PATH_MAX - 40`, `:6817`). The refusal is right: Git for Windows itself dies on a `$GIT_DIR` that
   long with `'$GIT_DIR' too big`.
4. So the child's first `drive` ends with that refusal instead of the scripted shutdown, `stale_at_width_one`'s
   assertion that it shut down panics (`:18451`), and the child exits 101 in its setup. No kill is armed and the
   charged record is never reached.
5. Both parents then fail at `died_by_abort` (`:19651`): "<shape>: the child died at the record's append:
   ExitStatus(ExitStatus(101))". Their subject, the kill at the record's append and the recovery from it, never
   executes on that lane, and the red reads like the crash-consistency regression it is not.

**Observed: CI, read from the job logs.** Master's push run `38205768792`, `test (winguest)` job `114668800387`, at
`697de9c6`: both tests failed at `src\engine\topology\coordinator.rs:19651:9` with `ExitStatus(ExitStatus(101))`,
3004 passed, 2 failed, 89 ignored. #337's run `38215961060`, job `114700875130`, at its merge `29b82db4` (master plus
A-12, whose diff is the topology ledger's source, tests and notes and one finding file): the same two failed and the
control,
`kill_inside_closure_recovers_at_width_three`, passed. The same two tests passed on the queue's hosted
`windows-latest` entry at `697de9c6` (run `38202717688`, job `114659401406`), on the guest lane at `4fa98776`
(runs `38194036297` and `38197442250`), and on Ubuntu and macOS, where `add_worktree` does not call the guard.
`run_kill_child_within` sends a kill child's standard output and error to the null device, so a CI log shows the
parent's assertion and nothing of the child: on CI the cause is not captured.

**Captured: the cause, on the persistent diagnostic guest.** One filtered run at `697de9c6`, under the same
temporary directory, with a declared patch that kept the kill children's standard error
(`~/orch-pr11/logs/pr11_a12_diag1/RESULTS.md` on the build box; the children's records under
`runs/probe-master-697de9c6-a1/guest/children/`, listed with their sha256 in that run's `SHA256SUMS`). Both
charged-record children, shapes `torn` and `before`, exited 101 after 1,973 ms, each panicking at
`coordinator.rs:18451:9` with the product's refusal:

> refusing `git worktree add`: the `$GIT_DIR` Git for Windows would be handed,
> `C:/Users/Administrator/AppData/Local/Temp/upstroke-tmp-3VMH6PQXPC/upstroke-coordinator-o14-record-kill-GY7CRE02H5/private/workspaces/04d51fe1285502ed/01KZSWEEP00000000000000001/snapshots/k0-g0-a1-review0_WG9T4H5MXCSP/.git`,
> is 221 bytes in UTF-8, over Git's budget of 220 (`PATH_MAX - 40`)

Both control children aborted with `0xc0000409`, as `died_by_abort` requires, and both parents failed as CI's did.
The guest is not CI's runner and the run's build profile differed from CI's; the claim is for this mechanism only:
the same temporary directory, the same code and the same parent fingerprint.

**Why it repeats: arithmetic on the code, and conditional on it.** Every component below the temporary directory has
a fixed length: the two scratch names' ten-character suffixes (`rundir::scratch_tree`'s `NAME_ENTROPY`), the 16-hex
repo key (`REPO_KEY_HEX_CHARS`), the fixture's run id, the slot's name, and the 12-character instance tag
(`InstanceTag::LEN`) of the fixed incarnation `inc-1`. Under this code the path is the temporary directory plus 180
bytes, so the add is admitted only where Git for Windows' spelling of the temporary directory is 40 bytes or fewer. For
this code, this construction and this temporary directory, that makes the red a prediction from the code; it is not
a measured rate, and what was observed is the two CI failures and the one capture above. The hosted queue's log spells
its temporary directory `C:\Users\RUNNER~1\AppData\Local\Temp`, 36 bytes, at which the path is 216. The guard measures
the spelling `canonical_prefix` returns, which that run did not record, so its pass establishes only that the spelling
it measured was 40 bytes or fewer.

**Provenance: derived from history and the job logs, not executed at these commits.** #330 (`c80a1931`) introduced
`InstanceTag`, which made every slot instance `<component>_<12-character tag>`, 13 bytes longer, and the guard; neither
exists at `4fa98776`. #334 added the charged-record kill tests, whose functions are textually identical at `4fa98776`
and `697de9c6`, as are the blobs of `scaffold.rs`, `scratch_tree.rs` and `fixture.rs`. At `4fa98776`, #334 on
`10f7bf02` before #330, the same path is 208 bytes and nothing refuses it: #334's two guest-lane runs passed there.
Its merge-queue entry ran on hosted `windows-latest` only. So `697de9c6`, #334's merge onto `c80a1931`, is the first
commit carrying both #330's naming and #334's tests, and master's own push was the first guest-lane run of that
composition. The provenance is #334's feature as merged: its kill child's fixture, one scratch tree below the
temporary directory and under a 27-byte tag, fitted the budget only until #330's instance tag spent 13 more bytes of
it.

**Severity: P1, by consequence.** For this code under the guest lane's own temporary directory the red is
deterministic (above) on the Windows leg of the required `upstroke-ci` context, for master's push and, by the same
arithmetic, for a pull request run whose tree carries both #330's naming and #334's tests unchanged; and a
crash-consistency witness of O14 is absent from the self-hosted Windows lane while its red reads as a regression of
what it witnesses. It is not a serious P1 in `MAINTAINING.md`'s sense: the product is correct, and no §4 invariant,
durable state or user data is reached. The nearest open row of this class,
`G5RUN3-LEGACY-ENGINE-TESTS-NEED-A-SHORT-WINDOWS-TEMP-PATH`, is P3, and its failure needs a temporary directory 12
bytes longer than the lane's; this one fires under the lane's own.

**Relation to existing findings.**
- `PR299-HOSTED-QUEUE-DOES-NOT-PROVE-GUEST-BEHAVIOUR` (P2, accepted-risk) is the route by which this reached master:
  a guest-lane red on the push to `master` after a green hosted queue entry. It is recorded there as a sighting, with
  its disposition unchanged; this row is the test defect that route let through.
- `CLASS-INTERMITTENT-SUBPROCESS-KILL-SETTLE-RESIDUE-FAILURES`: not a member. That row limits itself to four named
  fingerprints and forbids attribution by resemblance, and this failure is neither intermittent nor unexplained.
- The same class in other tests: `G5RUN3-LEGACY-ENGINE-TESTS-NEED-A-SHORT-WINDOWS-TEMP-PATH` (in-process legacy engine
  tests, refused by Git itself), and #292's review round 8, recorded in `docs/internals/engine/topology/scaffold.md`,
  where a nested kill child's 223-character task worktree was refused on the guest and the scratch tree's tag was
  shortened to `kill`. A nested kill child's tags are spent from the same 220 bytes as the slot names below them.

## What the change that takes this up should do

Shorten the record-kill child's fixture path, keeping every abort and recovery assertion, `died_by_abort`, both kill
shapes and the control. Add a test, red at `697de9c6` on every platform, that pins the child's deepest `$GIT_DIR` under
the guest lane's 41-byte temporary directory within `WINDOWS_GIT_DIR_BUDGET` with a stated margin, derived from the
naming the child and the product actually use rather than from copied numbers; and sweep the other kill children whose
fixtures nest under a scratch tree. Not a repair: changing the guard, the budget, `InstanceTag` or the slot naming,
which are #330's design and keep a user's run from Git for Windows' own refusal; changing the lane's temporary
directory, the queue or CI; or skipping, ignoring or cfg-ing out the tests.
