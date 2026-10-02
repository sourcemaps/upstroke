---
id: PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION
severity: P1
disposition: deferred
category: correctness
pr: 329
reviewed_sha: 85f5b09b5fbb2436acd092fa77388d1bea750aef
location: src/workspace_manager.rs:2649
provenance: pre_existing
first_bad: predates PR11: Git's prune decides on an entry before its `locked` exists and deletes it later without looking again, on 2.43.0, 2.50.1 and 2.55.0, so every engine `git worktree add` has both faces; executed at master 5c222ff2's add argv by PR #329's design rounds 7 and 8; prior IDs FUB-D6-PRUNE (face 1) and FUB-D7-R13 (face 2), from PR #329's design reviews 6 and 7, and the host-agent subset PR11-HOST-AGENT-PRUNE-RACES-AN-ENGINE-ADD
guard: before G6, which cannot pass with this open: a closure implemented and validated (the candidates below), or the owner's explicit ruling on its scope, asked in the PR11 orchestrator's one consolidated owner question after design reviews B8 (#329), C4 (#330) and D3 (#331); follow-ups C and D refer to this file and file no duplicate
---

## Failure sequence

**Git's mechanism.** `git worktree prune` decides on each entry of `<common git dir>/worktrees/` and then deletes
it, with nothing in between that reads the entry again (`prune_worktrees`, `builtin/worktree.c` 2.43.0 `:215-216`,
2.50.1 and 2.55.0 `:229-230`). An entry with neither `locked` nor `gitdir` is pruned whatever the expiry
(`should_prune_worktree`, `worktree.c` 2.43.0 `:735`, 2.50.1 `:930`, 2.55.0 `:959`). `git worktree add` creates its
entry and writes that entry's `locked` in two steps (2.43.0 `:458` and `:483`, 2.50.1 `:473` and `:498`, 2.55.0 `:507`
and `:532`). A prune that decides between the two deletes the entry at a later moment of its own. That moment can fall
during the add (face 1) or after the add returned (face 2). Git's line citations are PR #329's record §6.2.

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

**Face 1: the add fails after Git took its destination over.**

    an engine add creates its entry -> a prune decides on it and is held before deleting
    -> the add writes `locked` and takes its destination over
    -> the prune deletes the entry -> the add fails writing `gitdir`, `commondir` or `HEAD`, or its checkout's index
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

**Face 2: the deletion lands after the add returned** (PR #329's R13).

    the prune decides in the add's window and is held until after the access returned Ok
    -> the engine uses the checkout: an agent's slot, a judgement's snapshot, a verification's staging checkout
    -> the prune deletes the entry; when it was the store's last, Git removes the store too
       (`delete_worktrees_dir_if_empty`)
    -> every Git command in that checkout exits 128, "not a git repository"
    -> (a verification's staging diff) `candidate_diff` (`src/workspace_manager.rs:4793`) returns Git state;
       `src/engine/topology/run.rs:279` maps it to `Verified::Unavailable`; the frozen
       `src/engine/topology/integrate.rs:871` appends `merge_verification_unavailable`, spending a deferral or
       parking a valid candidate
    -> (a verification's gate) a gate such as `git rev-parse --verify HEAD` exits 128 in the snapshot;
       `src/engine/topology/attempt.rs:990` classifies it, `src/engine/classify.rs:54` makes it `GateFailed`,
       and the frozen `src/engine/topology/integrate.rs:779` appends `MergeRejected` for a valid candidate
    -> (an attempt's gate) the same verdict settles a spent attempt (`src/engine/topology/run.rs:950`)
    -> (an attempt's own slot) the engine's capture in the slot fails, and the command ends resumably.
       The resume's `quiescence` answers `NotRegistered` (`src/workspace_manager.rs:2769`).
       `dispatch::verify_or_recreate` then removes and recreates the slot (`src/engine/topology/dispatch.rs:248-253`),
       deleting the unpinned edits it held
    -> (the store gone) the removal scan refuses a populated target when no store exists
       (`src/workspace_manager.rs:5144-5164`), on every attempt, so the frozen finalization's `scrub_slots`
       (`src/engine/topology/finalize.rs:251`) refuses on every resume and does not converge
    -> (a legacy run, #331's R-D9) `verify_gate_worktree`'s `git status` (`src/workspace.rs:908-944`) fails as Git
       state, and the legacy coordinator discards paid output (`src/engine/coordinator.rs:544-548`)

## Evidence

**Executed at the Git level, Linux, on upstream 2.43.0, 2.50.1 and 2.55.0.** The logs are under
`~/orch-pr11/logs/pr11_fub_design7/` (`d7/`) and `~/orch-pr11/logs/pr11_fub_design8/` (`d8/`); PR #329's record §6.2
and §7 cite them.

Face 1:
- **Interleaved with a pause shim**, the prune held between decision and deletion (`d8/witness/MATRIX.txt`,
  `FIGURES.txt`, three rounds per version):
  - the pruners: `git worktree prune` at four post-takeover points, `git gc`, and `git maintenance run --auto` on
    2.55.0;
  - round 8's access refused every one at once;
  - option (i) returned every one as Git state.
- **Unpaused,** 2,000 adds against four `git worktree prune` loops (`d8/witness/stress-*-r8.json`). Round 8 refused
  15, 5 and 4 adds and returned Git for none; option (i) returned 16, 7 and 7 as Git.

Face 2 (`d8/witness/r13.txt`, `r13-summary.txt`):
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

**Reasoned from the code at `5c222ff2`:** the engine consequences above. Design review round 7's three lenses traced
them independently (`~/orch-pr11/reviews/review-329-d7-{design,concurrency,regression}-85f5b09b.review.md`).

## Grading

**P1.** All three lenses of PR #329's design review round 7 graded face 2 P1, "independently of the host-agent filing's
P2 label" (`~/orch-pr11/reviews/review-329-d7-triage.md`, FUB-D7-R13). Its consequences are durable and wrong for valid
work:
- a valid candidate rejected (`MergeRejected`), deferred or parked;
- an attempt spent;
- unpinned edits deleted by recovery;
- a finalization that never converges.

For a legacy run it is the discard of paid output, which is MAINTAINING's serious-P1 criterion.

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

**It blocks G6** until a closure is implemented and validated, or the owner rules explicitly on its scope. Filing is
no waiver (PR #329's record §7.7).

## Reconciliation

- **`PR11-HOST-AGENT-PRUNE-RACES-AN-ENGINE-ADD`** (P2, PR12) keeps the host agent's access to the shared registry and
  its remedies: the container runner's Git view, or refusing `max_parallel > 1` with the host runner. This file
  carries both prune faces for every starter, the agent included, at P1, with its G6 disposition.
- **#330 (follow-up C)** and **#331 (follow-up D)** refer to this file for R-GU and R-D9 and file no duplicate.
  - #330 gives the class's analysis from DESC's side.
  - #331 gives the legacy face's consequence and its G6 applicability, separately for legacy-only and mixed runs.

## What the change that takes this up should do

PR #329 proposes none of these. Its round 8 adds no machinery. Each candidate is scoped exactly, with the frozen status
of every file it touches (the PR11 record's R-D).

1. **Re-check the checkout's registration before any durable negative outcome.**
   - **The rule.** Where a Git failure, or a failing gate, in an engine checkout would become a durable outcome, the
     engine first checks that the checkout is still registered. Its `.git` file must name
     `<common git dir>/worktrees/<name>`, and that entry's `gitdir` must name the checkout back. When it is not, the
     result is a resumable refusal.
   - **Why it is sound.** No engine path re-creates a registration that a prune deleted. So a registration present
     after the failure was present at the failure, and that failure was the checkout's own.
   - **Where.**
     - `WorkspaceManager::candidate_diff`, and the manager's other commands run in a slot (`src/workspace_manager.rs`);
     - the gate verdict before `classify::gate_failure` (`src/engine/topology/attempt.rs:990`);
     - or once, at `run::verified`'s Git arm (`src/engine/topology/run.rs:279`).

     **None is frozen.** The frozen `src/engine/topology/integrate.rs` (its `:779` and `:871`) and
     `src/engine/topology/finalize.rs` only receive what these return.
   - **What it closes.** Face 2's verification, gate and attempt consequences.
   - **What it leaves.** The slot's edits (item 2) and the store-gone finalization (item 3).
2. **Keep a live checkout whose registration is gone, instead of recreating its slot.**
   - **The problem.** `dispatch::verify_or_recreate` (`src/engine/topology/dispatch.rs:248-253`, not frozen) removes
     the checkout.
   - **Re-registering it needs new code.** Git has no command for it: `git worktree repair` refuses and
     `git worktree add` refuses the populated directory (executed, `d8/witness/r13-summary.txt`). The engine would
     write Git's registration files itself, which depends on Git's internal layout, or move the checkout aside, add a
     fresh one and move the work back.
   - **The smaller form refuses instead.** A `NotRegistered` slot whose checkout is populated refuses resumably and is
     kept for the operator.
   - **Scope.** `dispatch.rs` and `src/workspace_manager.rs`, neither frozen.
3. **Let a store-gone reclaim converge.**
   - **The rule.** The removal scan's store-absent branch (`src/workspace_manager.rs:5144-5164`, not frozen) would bind
     nothing for a contained target whose `.git` file names an entry in this repository's own absent store, which is
     face 2's exact shape. `remove_bound` then removes it, as it does when the store exists.
   - **What it changes.** The pinned refusal `a_missing_stored_worktree_directory_refuses_before_checkout_deletion`
     (`src/workspace_manager/tests.rs:6952`, not frozen).
   - **What it needs first.** Item 2, so that no unpinned edits are removed.
   - **What it leaves alone.** `finalize.rs` (frozen) is unchanged.
4. **Environmental requirements.**
   - **What the repository's shared configuration can stop.** `maintenance.auto=false` and `gc.auto=0` stop the
     automatic starters in every checkout, the user's included. So does `maintenance.worktree-prune.auto=0` for
     2.50.1's and 2.55.0's `worktree-prune` task (`builtin/gc.c` 2.50.1 `:359-361`, 2.55.0 `:399-401`;
     `d8/git-src/prune-config-lines.txt`).
   - **What it cannot stop.** `gc.worktreePruneExpire=never` stops nothing here, because the no-`gitdir` rule ignores
     the expiry. No configuration stops an explicit `git worktree prune` or `git gc`, or an IDE's.
   - **How it would be applied.** Either the engine writes the user's repository configuration, which it does not own
     (Q6's untouched user checkout), or a preflight refuses a run without it (new code, in the non-frozen topology
     preflight), plus documentation. Either way the explicit starters remain, and need item 5.
5. **The owner's ruling on scope.** For example: a prune that no engine process starts, during a run, is outside the
   topology's G6 claims, as an operator deleting the run directory is. This P1 stays filed, and the documentation
   names the requirement. The ruling can be made per face or per starter.

**What does not close it:**
- **A retry rule, or any access.** No access can see a deletion that lands after it returned.
- **`git worktree lock` after the add.** The prune decided before the lock existed.
- **`gc.worktreePruneExpire=never`.**
- **Turning off the engine's own maintenance** (#330, #331): that removes only the engine starters.
- **PR #329's round-7 registry-free checkout probe.** It is withdrawn: two executed P1s, FUB-D7-SPLITINDEX and
  FUB-D7-CONFIG.
