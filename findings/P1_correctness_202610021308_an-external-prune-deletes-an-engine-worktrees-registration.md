---
id: PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION
severity: P1
disposition: deferred
category: correctness
pr: 329
reviewed_sha: 85f5b09b5fbb2436acd092fa77388d1bea750aef
location: src/workspace_manager.rs:2649
provenance: pre_existing
first_bad: predates PR11: Git's prune decides on an entry before its `locked` exists and deletes it later without looking again, on 2.43.0, 2.50.1 and 2.55.0, so every engine `git worktree add` has both faces; executed at master 5c222ff2's add argv by PR #329's design rounds 7 to 9; prior IDs FUB-D6-PRUNE (face 1) and FUB-D7-R13 (face 2), from PR #329's design reviews 6 and 7, face 2's boundary corrected by FUB-D8-DURINGADD (review 8), and the host-agent subset PR11-HOST-AGENT-PRUNE-RACES-AN-ENGINE-ADD
guard: before G6, which cannot pass with this open: closures implemented and validated for each applicable consequence (the candidates below), or the owner's explicit ruling on its scope per face and starter, asked in the PR11 orchestrator's one consolidated owner question after design reviews B9 (#329), C5 (#330) and D3 (#331); follow-ups C and D refer to this file and file no duplicate
---

## Failure sequence

**Git's mechanism.** `git worktree prune` decides on each entry of `<common git dir>/worktrees/` and then deletes
it, with nothing in between that reads the entry again (`prune_worktrees`, `builtin/worktree.c` 2.43.0 `:215-216`,
2.50.1 and 2.55.0 `:229-230`). An entry with neither `locked` nor `gitdir` is pruned whatever the expiry
(`should_prune_worktree`, `worktree.c` 2.43.0 `:735`, 2.50.1 `:930`, 2.55.0 `:959`). `git worktree add` creates its
entry and writes that entry's `locked` in two steps (2.43.0 `:458` and `:483`, 2.50.1 `:473` and `:498`, 2.55.0 `:507`
and `:532`). A prune that decides between the two deletes the entry at a later moment of its own. The deletion removes
the entry's names one at a time, in the filesystem's directory order, and then the directory (`dir.c`
`remove_dir_recurse`). Git's line citations are PR #329's record §6.2 and `~/orch-pr11/logs/pr11_fub_design9/git-src/citations-d9.txt`.

**Who starts the prune: never an engine process once #329, #330 and #331 land.**
- #329 deletes the engine's explicit prunes (its record §3.5).
- #330's and #331's Git builders turn off the automatic maintenance of the engine's own commands.

The starters left:
- **A host-runner agent's Git** in its task worktree: `git worktree prune`, `git gc`, or the automatic maintenance
  its own commits run. `PR11-HOST-AGENT-PRUNE-RACES-AN-ENGINE-ADD` (P2, guarded by PR12) is this subset; its
  reconciliation says what it keeps.
- **The user's or an IDE's Git** in any checkout of the repository: an explicit `git worktree prune` or `git gc`.
  PR #329's record calls this R3′, #330's R-GU and #331's R-D9.
- **Git's automatic maintenance** after a `git commit`, `git fetch` or `git merge` in any checkout. It reaches the
  prune two ways:
  - through gc, when gc is due, on every version;
  - on 2.55.0, through the default geometric strategy's `worktree-prune` task. That task runs whenever one entry looks
    prunable (`builtin/gc.c` 2.55.0 `:391-427`, `:1916`, `:1974`), and an add in its window is such an entry.
- **Git's scheduled maintenance:** the system scheduler's `git maintenance run --schedule=<f>` for each registered
  repository, when its task set includes `gc` or `worktree-prune` (PR #329's record §8.6). No `*.auto` setting stops it.
- **A prune already running,** which read its configuration before any requirement was set or checked.

**Face 1: the deletion fails the add, after Git took its destination over.**

    an engine add creates its entry -> a prune decides on it and is held before deleting
    -> the add writes `locked` and takes its destination over
    -> the prune deletes the entry before the add's last write into it
    -> the add fails writing `gitdir`, `commondir` or `HEAD`, or its checkout's index
    -> Git's junk removal removes the destination: the same end state as a checkout that cannot be made
    -> PR #329 (round 8) refuses that state at once as `RegistryRefused`, a resumable end, never Git state:
       - in an attempt (dispatch's slot add, or the judge's snapshot add), the coordinator's fail path cancels the
         other pipelines and ends the command; the interrupted work runs again on resume, paid again
       - in a verification's snapshot add, `run::verified` does not map it (`src/engine/topology/run.rs:289`),
         so the command ends and the candidate re-verifies under a new sequence; no deferral is spent
       - at integration's staging add (`src/engine/topology/integrate.rs:586`), a coordinator-side resumable end
    -> the same refusal answers a genuine checkout failure there, which no observation tells apart: that cost is
       `PR329-A-GENUINE-CHECKOUT-FAILURE-AFTER-THE-TAKEOVER-REFUSES` (P2)
    -> had #329 returned it as Git state (round 6's answer, option (i) in its record §7.3), a verification would have
       spent a deferral or parked a valid candidate (`src/engine/topology/run.rs:279`, then the frozen
       `src/engine/topology/integrate.rs:871`)

**Face 2: the add returns Ok, and its registration is deleted, wholly or in part, at any time from the prune's
decision onward** (PR #329's R13, its boundary corrected by its record §8.3). That includes a deletion during the add's
tail, before the add returns: after the checkout Git clears its junk flag, unlinks `locked` (a missing file is not an
error), runs `post-checkout` and returns, and nothing on that path reads the entry again.

    the prune decides in the add's window and is held past the add's last write into the entry
    -> the add returns Ok, so no veto runs; the engine uses the checkout: an agent's slot, a judgement's snapshot,
       a verification's staging checkout
    -> the prune deletes the entry, before or after the add returned; when it was the store's last, Git removes the
       store too (`delete_worktrees_dir_if_empty`)
    -> with `HEAD`, `commondir` or the whole entry gone, every Git command in that checkout exits 128, "not a git
       repository"; with only `index` gone, commands succeed and misread the checkout (`git status` reports a clean
       checkout as changed)
    -> (a verification's staging diff) `candidate_diff` (`src/workspace_manager.rs:4793`) returns Git state;
       `src/engine/topology/run.rs:279` maps it to `Verified::Unavailable`; the frozen
       `src/engine/topology/integrate.rs:871` appends `merge_verification_unavailable`, spending a deferral or
       parking a valid candidate
    -> (a verification's gate or review) a gate such as `git rev-parse --verify HEAD` exits 128 in the snapshot;
       `src/engine/topology/attempt.rs:993` builds `GateFailed` inside an `Ok(Judgement)`, `run.rs:258` returns
       `Verified::Judged`, and the frozen `src/engine/topology/integrate.rs:794` appends `MergeRejected` for a valid
       candidate; a review pass's failure (`attempt.rs:1087`) reaches the same judgement
    -> (an attempt's gate or review) the same verdict settles a spent attempt (`src/engine/topology/run.rs:950`)
    -> (an attempt's own slot) the engine's capture in the slot fails, and the command ends resumably.
       The slot's next verification, a resume's recovery or a live run's, answers `NotRegistered`
       (`src/workspace_manager.rs:2769`) or, when the deletion lands after the store was listed, `Missing` (`:2783`).
       `dispatch::verify_or_recreate` then removes and recreates the slot (`src/engine/topology/dispatch.rs:251`),
       deleting the unpinned edits it held
    -> (a retained generation's retry) `settle::retry` verifies the slot with `HoldsTree`
       (`src/engine/topology/settle.rs:285-289`) and answers `NotRegistered`, `Missing` or `TreeMismatch`; the
       generation is closed `WorktreeMissing` (`:297-300`), and the run appends `GenerationClosed` and scrubs the slot
       (`src/engine/topology/run.rs:1459-1466`)
    -> (the store gone) the removal scan refuses a populated target when no store exists
       (`src/workspace_manager.rs:5144-5164`), on every attempt, so the frozen finalization's `scrub_slots`
       (`src/engine/topology/finalize.rs:251`) refuses on every resume and does not converge
    -> (a legacy run in a mixed repository, #331's R-D9) the snapshot's verification fails as Git state, or a gate or
       a review fails and is judged, and the legacy coordinator discards paid output (#331's record §3.4)

## Evidence

**Executed at the Git level, Linux, on upstream 2.43.0, 2.50.1 and 2.55.0.** The logs are under
`~/orch-pr11/logs/pr11_fub_design7/` (`d7/`), `~/orch-pr11/logs/pr11_fub_design8/` (`d8/`) and
`~/orch-pr11/logs/pr11_fub_design9/` (`d9/`); PR #329's record §6.2, §7 and §8 cite them.

Face 1:
- **Interleaved with a pause shim**, the prune held between decision and deletion (`d8/witness/MATRIX.txt`,
  `FIGURES.txt`, three rounds per version):
  - the pruners: `git worktree prune` at four post-takeover points, `git gc`, and `git maintenance run --auto` on
    2.55.0;
  - round 8's access refused every one at once;
  - option (i) returned every one as Git state.
- **Unpaused,** 2,000 adds against four `git worktree prune` loops (`d8/witness/stress-*-r8.json`). Round 8 refused
  15, 5 and 4 adds and returned Git for none; option (i) returned 16, 7 and 7 as Git.

Face 2 after the return (`d8/witness/r13.txt`, `r13-summary.txt`):
- **42 of 42 runs** across the three versions, with `git worktree prune`, `git gc` and (2.55.0) maintenance as
  pruners, and with the store kept and the store emptied. In every run:
  - the access returned Ok and the registration was then deleted;
  - the gate command, the staging diff's argv, `git status` and `git add -A` in the checkout each exited 128;
  - the slot was no longer listed;
  - `git worktree repair` exited 1 and re-registered nothing;
  - `git worktree add` over the checkout exited 128, "already exists";
  - the edits stayed in the checkout.
- **Where the entry was the store's last** (21 runs), the store was gone and the slot populated, which is the removal
  scan's refusing shape.
- **Unpaused,** face 2 was not observed in 18,000 adds (`d8/witness/stress-*.json`,
  `ok_but_registration_gone_later` 0). It needs the pruner held between decision and deletion for the add's whole tail,
  which only scheduling provides.

Face 2 before the return (PR #329's record §8.3):
- **Design review round 8's concurrency lens** executed the prune's decision before `locked` and its deletion before
  the add unlinked `locked`: the add exited 0 and `git status` exited 128 on all three versions
  (`d9/witness/reviewer-d8/pr329-d8-prune-before-return-9o_2jeby.results.json`, hash-checked).
- **Re-executed with Git's own extension point** (`d9/witness/duringadd.jsonl`, 18 of 18): the add's `post-checkout`
  hook ran Git's own `git worktree prune`, which deleted the add's entry; the add exited 0 (trace2), after the prune's
  exit; `git status` and the gate exited 128.

Partial deletion (PR #329's record §8.4; `d9/witness/partial.jsonl`, 9 runs per name): with `HEAD`, `commondir` or
the checkout's `.git` gone, every command exits 128; with `index` gone, every command exits 0 and `git status` prints
`D  tracked` and `?? tracked` for a clean checkout; with `gitdir`, `logs`, `ORIG_HEAD` or `refs` gone, nothing
changes. On this box's ext4 a prune's pass unlinks `HEAD` first among those names. Under `core.splitIndex=true`
(`d9/witness/splitindex.jsonl`, 9 runs): with the entry's `sharedindex.<sha>` gone, `git status` and
`git ls-files --stage` exit 128 while the other four names remain, and that random name came before `HEAD` in 4 of 9
entries. #331's design review round 3 executed a removed `index` written again by `git read-tree HEAD`
(`~/orch-pr11/reviews/331-d3-witnesses/`).

**Reasoned from the code at `5c222ff2`:** the engine consequences above. Design review rounds 7 and 8's three lenses
traced them independently (`~/orch-pr11/reviews/review-329-d7-*-85f5b09b.review.md`,
`review-329-d8-*-f7a9256c.review.md`); the retained retry's close and scrub was found in PR #329's design round 9.

## Grading

**P1.** All three lenses of PR #329's design review round 7 graded face 2 P1, "independently of the host-agent filing's
P2 label" (`~/orch-pr11/reviews/review-329-d7-triage.md`, FUB-D7-R13), and round 8's lenses kept it. Its consequences
are durable and wrong for valid work:
- a valid candidate rejected (`MergeRejected`), deferred or parked;
- an attempt spent;
- unpinned edits deleted by recovery, and a retained generation closed and scrubbed;
- a finalization that never converges.

For a legacy run in a mixed repository it is the discard of paid output, which is MAINTAINING's serious-P1 criterion.

**Face 1** is, under PR #329 round 8, a resumable end of the command: the coordinator's pipeline-error consequence,
nothing durable. It is filed here because its cause is the same, and its closure is the same. Rate does not lower the
grade, as for the legacy race: both faces need a prune whose decision falls inside a window of a few calls, which is
rare, but the starters run in ordinary use, and on 2.55.0 every commit in any checkout can start one.

**G6.** The class applies to G6 through:
- **Q6 and R17**, the parallel checkouts' shared registry: a concurrent prune turns a valid candidate's verification
  into a durable negative outcome;
- **Q1**, recovery's reclaim before reuse;
- **ST-18**, terminal finalization's convergence;
- **INV-22**, accounting of Git's administrative residue with its worktree.

**It blocks G6** until closures are implemented and validated for each applicable consequence, or the owner rules
explicitly on its scope, per face and per starter. Filing is no waiver, and no single closure clears the class
(PR #329's record §8.9).

## Reconciliation

- **`PR11-HOST-AGENT-PRUNE-RACES-AN-ENGINE-ADD`** (P2, PR12) keeps the host agent's access to the shared registry and
  its remedies. Only isolating the agent's Git view (the container runner's, or an equivalent) removes that starter;
  refusing `max_parallel > 1` with the host runner does not, because two width-one runs in two linked checkouts overlap
  (PR #329's record §8.7). This file carries both prune faces for every starter, the agent included, at P1, with its
  G6 disposition.
- **#330 (follow-up C)** and **#331 (follow-up D)** refer to this file for R-GU and R-D9 and file no duplicate.
  - #330 gives the class's analysis from DESC's side.
  - #331 gives the legacy face: in a legacy-only repository it is outside G6; in a mixed one it applies and blocks G6
    as part of this class. D's C-SIDE, evaluated and not adopted there, is closure 1's legacy form.

## What the change that takes this up should do

PR #329 proposes none of these. Its rounds 8 and 9 add no machinery. Each candidate is scoped exactly, with the frozen
status of every file it touches (the PR11 record's R-D). PR #329's record §8.4 to §8.6 and §8.11 give the full text.

1. **Re-check the registration before any durable negative outcome (closure 1). A partial mitigation, not a
   closure** (PR #329's record §8.4, after the PR11 orchestrator's addendum of 2026-10-02T15:20Z).
   - **The check,** read after the failure, before the outcome, from files only: the checkout's `.git` names an entry
     in this repository's store; that entry holds `gitdir`, `commondir`, `HEAD` and `index`, regular files, the first
     three non-empty; `commondir` resolves to the common git dir; `gitdir` names the checkout's `.git`, compared as the
     same file. Not whole: a resumable `RegistryRefused`.
   - **What it catches.** A prune's pass only removes names, and `HEAD` and `commondir` cannot be written again from
     the checkout, since every command there exits 128 without them; nothing writes `gitdir` from it. So every deletion
     that leaves a checked name missing at the check is caught, a complete one included.
   - **R-REWRITE, a residual (P1 where it occurs).** A removed `index` is written again by an index-writing Git command
     in the checkout before the check, and the check then passes a failure the deletion caused. Executed by #331's
     design review round 3 (`git read-tree HEAD` after a gate failed; the four files then whole). No read after the
     fact tells a rewritten file from one never removed.
   - **R-OUTSIDE, a residual (P1 where it occurs).** The checkout needs a file the check does not read: a split
     index's `sharedindex.<sha>` (executed, `d9/witness/splitindex.jsonl`, 9 of 9: `git status` exits 128 while the
     four names remain; on this box's ext4 it came before `HEAD` in 4 of 9 entries), or `config.worktree` and
     `info/sparse-checkout` where the add copied them (reasoned). A Git read of the index in the check
     (`git ls-files --stage`) would cover the split index, at the cost of a Git child and an instrument row.
   - **Both need the owner's ruling to name them;** they keep the class open whatever else is chosen.
   - **Where.**
     - Git errors from the manager's commands in a slot, `WorkspaceManager::candidate_diff` among them
       (`src/workspace_manager.rs:4793`);
     - judgement verdicts, gates and reviews alike, in `Judge::judge` before the snapshot is released
       (`src/engine/topology/attempt.rs:990-1025`, `:1087-1119`), which covers verifications (`run.rs:258`) and
       attempts (`run.rs:950`);
     - an attempt's settlement of a failure assessed in its own slot (`attempt.rs:963`).

     **None is frozen.** The frozen `src/engine/topology/integrate.rs`, `recover.rs` and `finalize.rs` receive a
     refusal through paths that already pass resumable errors on. Private readers, as D's C-SIDE uses, need no
     effect-governance row.
   - **What it mitigates.** Face 2's false verdicts: deferral or park, `MergeRejected`, an attempt spent, wherever a
     checked name is missing at the check.
2. **Preserve a populated slot at the destructive boundary itself (closure 2).**
   - **Where:** immediately before `dispatch::verify_or_recreate` removes the slot
     (`src/engine/topology/dispatch.rs:251`), and before a retained generation's retry closes it and scrubs the slot
     (`src/engine/topology/settle.rs:297-300`, `run.rs:1459-1466`).
   - **When:** the verification answered `NotRegistered`, `Missing` or, under `HoldsTree`, `TreeMismatch`, the three a
     registration loss produces; closure 1's check does not find the registration whole, so a genuine mismatch keeps
     master's behaviour; and the slot's directory holds anything.
   - **What:** nothing is removed. A resumable refusal names the slot and keeps it for the operator, and a retained
     generation is not closed. Git cannot re-register the checkout: `git worktree repair` refuses and
     `git worktree add` refuses the populated directory (executed, `d8/witness/r13-summary.txt`).
   - **Scope.** `dispatch.rs`, `settle.rs`, `run.rs` and `src/workspace_manager.rs`, none frozen. The frozen test
     `a_resume_over_a_torn_open_generation_recreates_its_worktree` stands: the verification's own repair removes that
     checkout, whose `commondir` is empty, before the boundary.
   - **With #330's U,** a fresh-process resume recreates every open generation's slot and reclaims the earlier one, so
     across a resume the first boundary's loss is U's by design; within a live run, and for the retained retry, this
     closure applies.
3. **Let a store-gone reclaim converge (closure 3).**
   - **The rule.** The removal scan's store-absent branch (`src/workspace_manager.rs:5144-5164`, not frozen) would bind
     nothing for a contained target whose `.git` file names an entry in this repository's own absent store, which is
     face 2's exact shape. `remove_bound` then removes it, as it does when the store exists.
   - **What it changes.** The pinned refusal `a_missing_stored_worktree_directory_refuses_before_checkout_deletion`
     (`src/workspace_manager/tests.rs:6952`, not frozen).
   - **What it needs first.** Closure 2, so that no populated slot a recovery should keep is removed.
   - **What it leaves alone.** `finalize.rs` (frozen) is unchanged. It adds no prune, and binds removal to the
     instance's own entry.
4. **Environmental requirements (closure 4).** It reduces the automatic starters, excludes none, and stops no
   explicit prune.
   - **Command-triggered automatic maintenance:** `maintenance.auto=false`, `gc.auto=0`, and
     `maintenance.worktree-prune.auto=0` on 2.50.1 and later (`builtin/gc.c` 2.50.1 `:359-361`, 2.55.0 `:399-401`;
     `d8/git-src/prune-config-lines.txt`), each only where it is the effective value for the command that starts
     maintenance.
   - **Effective configuration is per checkout and per process:** a checkout's `config.worktree` overrides the shared
     configuration, and `-c` or `GIT_CONFIG_*` overrides both. Executed by design review round 8 on 2.55.0: a sibling's
     `config.worktree` re-enabled maintenance, and an ordinary commit there pruned the add's entry. A preflight cannot
     see a later checkout's or a process's own.
   - **Scheduled maintenance** reads no `*.auto` condition (`--auto` and `--schedule` are exclusive). It prunes when
     its task set includes `gc` or `worktree-prune`: on 2.43.0 and 2.50.1 a `maintenance.gc.schedule` is enough; on
     2.55.0 the `geometric` or `gc` strategy, or a task enabled and scheduled. `maintenance.<task>.enabled=false`
     drops the task, where effective. `git maintenance start`'s default strategy, `incremental`, schedules neither.
   - **A prune already running** keeps the configuration it read and deletes (executed by design review round 8).
     `git maintenance run` holds `objects/maintenance.lock` and `git gc` writes `gc.pid`; an explicit
     `git worktree prune` holds neither.
   - **`gc.worktreePruneExpire=never`** stops nothing here: the no-`gitdir` rule ignores the expiry.
   - **How it would be applied:** documentation, and at most a preflight in the non-frozen topology preflight. Writing
     the user's repository configuration is not the engine's (Q6's untouched user checkout).
5. **The owner's ruling on scope (closure 5),** per face and per starter, reflected in `DESIGN.md` and the G6
   assessment. It repairs nothing.

**The recommendation** (PR #329's record §8.11): closures 1, 2 and 3 in one new follow-up before G6, **closure 1 as a
partial mitigation,** closure 4 as documentation, and an owner ruling that a prune no engine process starts may stop a
run resumably or leave a kept slot for the operator (G6's claims for it are safety, not liveness), and that closure 1's
residuals, R-REWRITE and R-OUTSIDE, are accepted by name. No combination of closures clears the class. The alternatives,
and what each leaves open, are in that section: adding a Git read to closure 1 covers the split index at an instrument's
cost; closures 1 and 3 alone also leave recovery's and a retained retry's removal of a populated slot; closure 5 alone
leaves every durable consequence.

**What does not close it:**
- **A retry rule, or any access.** No access can see a deletion that lands after the add's own writes, before or after
  the add returns.
- **`git worktree lock` after the add.** The prune decided before the lock existed.
- **`gc.worktreePruneExpire=never`.**
- **Turning off the engine's own maintenance** (#330, #331): that removes only the engine starters.
- **Refusing width above one with the host runner:** two width-one runs in two checkouts still overlap.
- **A check of the two pointers alone** (round 8's closure 1): a partly deleted entry passes it.
- **Any check of names after the failure, alone:** a removed `index` written again, or a removed file the check does
  not read, passes it (closure 1's residuals).
- **PR #329's round-7 registry-free checkout probe.** It is withdrawn: two executed P1s, FUB-D7-SPLITINDEX and
  FUB-D7-CONFIG.

## At #329's implementation (2026-10-03)

**Face 1's narrowing is in force.** A deletion that fails an engine add after Git took its destination over now refuses
at once, resumably, and never returns as Git state. The suite reproduces the end state with a required filter that fails
the first checkout (`a_failure_after_the_takeover_is_refused_not_returned_and_not_attempted_again`). A deletion before
the takeover is attempted past (`an_add_whose_own_entry_cannot_be_made_once_succeeds_on_a_later_attempt`).

**For follow-up C's R-P.** Every explicit engine `git worktree prune` stays deleted, removal is bound to the instance's
own registration, and there is no global-prune fallback (`no_production_argv_of_the_manager_names_prune`,
`no_removal_prunes_another_processs_registration_and_the_store_goes_only_when_empty`).

**Face 2 is unchanged.** It is open, implements none of the closures above, and blocks G6 as this file says.

**Where the closures' code now is.** The line citations above are `8df42436`'s. #329 changed the removal scan's
store-absent branch that closure 3 would extend: it now also binds nothing for an empty directory at the target, the
destination an add makes before Git runs (`revalidate_removal_proving`, `src/workspace_manager.rs:5548`). A target
whose `.git` file names an entry in the absent store still refuses, as before.

## Design review round 9's findings against these closures (2026-10-02)

Recorded here so that the change that takes this up meets them. PR #329's ledger carries each as deferred to this file.
The source is the round's triage, `~/orch-pr11/reviews/review-329-d9-triage.md`, and the owner's decision appendix, §6.
- **FUB-D9-POLICY (P1):** closure 1 misses the verification's review-input policy. After a complete deletion the
  policy's `Workspace::open` fails its `rev-parse`, and the verification is `Unavailable` before any placement of the
  check is reached.
- **FUB-D9-HEADRECREATE (P1):** a ref transaction already prepared recreates a deleted `HEAD`. Executed on 2.43, 2.50
  and 2.55. The four-file check then passes the gate's failure, so R-REWRITE is not limited to `index`.
- **FUB-D9-RESUMESCRUB (P1):** closure 2's kept slot does not survive the next resume. Frozen recovery closes a
  retained generation and reclaims it before it recreates open ones (`src/engine/topology/recover.rs:1019-1021`).
- **FUB-D9-INFLIGHTSCRUB (P1):** closure 2 misses interrupted-attempt recovery. The resume settles the attempt
  interrupted, closes its generation and scrubs the checkout with its unpinned edits (`recover.rs:1461`).
- **FUB-D9-TASKSEL (P2; P3 in the design lens):** `maintenance.<task>.enabled=false` does not stop an explicit
  `git maintenance run --task=…`. Closure 4 is to be qualified.

The triage reads them together: closures 1 to 3 are partial even combined, and a complete technical closure of face 2
needs a change to G6-frozen recovery, or the owner's ruling on scope.
