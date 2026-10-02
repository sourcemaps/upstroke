---
id: PR329-LEGACY-RUNS-IN-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY
severity: P1
disposition: deferred
category: correctness
pr: 329
reviewed_sha: 92c4ca81f9209d218df4534ee71d3445dc2906e1
location: src/workspace.rs:871
provenance: pre_existing
first_bad: predates PR11: the legacy engine's four registry Git children have run untolerant since before PR5 froze `src/workspace.rs`; measured at the Git level on master `92c4ca81`'s argv by PR #329's first design round
guard: two changes under the owner's decision B, both PR5 unfreezes: corrected B1′, which routes `src/workspace.rs`'s three registry Git children through the workspace manager's tolerant registry access with the legacy retry predicate and the removal's success decision inside its attempt, and B-PRESERVE, which keeps a registry-refused attempt's output (`src/engine/coordinator.rs`, `src/engine/resume.rs`); PR #329 if the owner takes them before its implementation (its record, §4.4 and §4.5), or a later change
---

## Failure sequence

A legacy (schema 1–3) run in one checkout of a repository, and another run, legacy or topology, in
another checkout of it, the main one or a linked one. Each holds its own worktree lock, because that
lock is per checkout, so both run.

    the other run writes a registration into the shared `.git/worktrees/`, one file at a
    time: an add of a gate snapshot or a slot, or a removal
    -> the legacy run's gate-snapshot add, its `git worktree list` after a removal, its
       `git worktree remove --force`, or its resume's `git switch` enumerates the store
       (`add_gate_worktree` `src/workspace.rs:871`, `worktree_is_registered` `:1602`,
       `cleanup_gate_workspace` `:1549`, `switch_branch` `:450`) and dies on the half-written
       entry: "failed to read …/commondir: Success", "failed to read '…/locked'",
       "Invalid path"
    -> the error returns through `?` (`src/engine/attempt.rs:154`, `:178`)
    -> the legacy coordinator answers any attempt error with `discard_uncommitted()`
       (`src/engine/coordinator.rs:544-548`), which is `git reset --hard HEAD` and
       `git clean -fd` (`src/workspace.rs:1230-1235`), and ends the command
    -> the worker's uncommitted edits for that attempt are gone, and the resume runs the
       attempt again, paying for it again

**The same discard follows a registration that stays torn.** A writer killed mid-registration leaves
the entry half written, and the legacy reader dies on that residue exactly as it dies on a write in
flight. The residue stays until its own run's resume repairs it or an operator removes it.

**Executed through the real legacy engine** by PR #329's design round 5
(`~/orch-pr11/logs/pr11_fub_design5/census/probe-v/witness-runs/TABLE.txt`, `preserve_run`, unpatched tree): a legacy
run with one gate, a foreign registration torn right after the candidate is captured and left torn. The run fails with
"failed to read .git/worktrees/d5-static-residue/commondir: Success" and the checkout is clean afterwards: the paid
output was discarded. The resume fails the same way until the residue is removed, and then pays for the attempt again.

**Executed** by design review round 3 of PR #329, by two lenses (the PR11 orchestrator's
`~/orch-pr11/reviews/329-d3-witnesses/`):
- **Regression lens** (`pr329-d3-reg-legacy-chbhff16/result.txt`). With one Git writer per
  checkout, A was held right after opening its registration's `commondir`. B's exact legacy
  snapshot-add argv exited 128: "failed to read …/worktrees/gate-C/commondir: Success". Running
  `discard_uncommitted()`'s reset and clean then restored B's tracked file to `base` and removed its
  new file. A then completed.
- **Concurrency lens** (`pr329-d3-conc-confirm-bygs96b1/result.json`). With A's registration held in
  its empty-`commondir` state, B's legacy snapshot add exited 128, and the discard turned "paid
  worker edits" into "base".

**Measured at the Git level.**
- Round 1 of PR #329 ran the legacy argv concurrently in two linked checkouts
  (`reviews/2026-10-01-pr11-follow-up-b-record.md` §1.2, table C and "The legacy race"). With four
  loops per checkout, 12 and 16 commands failed in 7,200. With one loop per checkout, the shape one
  legacy coordinator per checkout gives, none failed in 5,400.
- Round 3 ran the legacy argv untolerant in the main checkout beside the topology manager's
  tolerant cycle in a linked one (record §2.10). The legacy side failed 13, 10, 14, 5 and 13 times
  in 6,000 commands a run; the topology side failed none in 40,000.

**What PR #329 changes about it.** PR #329's topology readers tolerate any writer (record §3.3), so a
legacy writer no longer tears a topology pipeline or verification. What remains is the legacy
engine's own exposure, to legacy and topology writers alike.

## Grading

**P1.** It was filed at P2 by PR #329's design round 3. Design review round 3 graded it P1 in all
three lenses, and design round 4 re-graded it.
- **The criterion.** MAINTAINING's serious P1 includes "loss or corruption of data in a user
  repository — the engine owns git". `discard_uncommitted()` discards the worker's paid output in
  the user's checkout. Neither a consistent event log, nor a resumable command, nor a low measured
  rate restores it, and the sequence is the production error path.
- **Rate.** Low at one coordinator per checkout. A write in flight is torn for a few file writes,
  but residue stays torn until someone repairs it.

**G6.** All three lenses of design review round 4 of PR #329 agree
(`~/orch-pr11/reviews/review-329-d4-triage.md`):
- **(e1) and (e1′)**, legacy against legacy, a write in flight or residue that stays torn: they do not apply to G6,
  which claims nothing of the legacy engine frozen at PR5. Each remains a P1.
- **(e2)**, a topology writer tearing a legacy reader while it writes: it applies through Q6 across the shared registry
  and R17, and blocks G6 until corrected B1′ is implemented and validated.
- **(e2′)**, a topology writer's static residue (a killed writer's torn registration) or contention that outlasts the
  deadline: the legacy access refuses, and the coordinator then discards the paid output. It applies through Q6, and a
  crash producer engages Q1; no surviving writer is needed, so it is distinct from
  `PR329-A-RESUME-REBINDS-A-SLOT-ITS-DEAD-COORDINATORS-GIT-CHILD-STILL-WRITES`. It blocks G6 **even after B1′**, until
  B-PRESERVE is implemented and validated, unless the owner rules otherwise.
- **Filing waives neither case.**

## What the change that takes this up should do

**Two changes, both the owner's: each unfreezes PR5-frozen legacy behaviour** (decision B; the PR11 orchestrator's
escalation item 7). PR #329's record, §4.4 and §4.5, gives both with their exact unfreeze texts.

**Corrected B1′ closes (e1) and (e2): every write in flight that finishes within the deadline, from any writer.**
- `src/workspace.rs`'s three registry Git children each run as one attempt of the workspace manager's tolerant
  registry access: `switch_branch`'s `git switch`, `add_gate_worktree`'s `git worktree add`, and
  `cleanup_gate_workspace`'s `git worktree remove` together with the `git worktree list` that decides its success.
- The add is attempted again only while its destination is still the empty directory `PendingGateWorkspace` made for
  it and no registration names it. Git takes the destination over only after its sibling scan, so an untouched
  destination means the failure came first.
- The removal's success decision is inside its attempt, so a removal a torn sibling fails is attempted again; an
  already-unregistered destination still counts as reclaimed.
- One private helper resolves the canonical common git dir as `recorded_objects_scope` does; nothing else in the
  module moves.
- The access's name joins the manager's `effect_free` list in `effects/wrappers.toml`, and `src/workspace.rs`'s
  `legacy_effect` text in `effects/allowlist.toml` takes a second amendment.

**B-PRESERVE closes (e1′) and (e2′): residue that outlasts the deadline.** It needs B1′, whose refusal it reads.
- When an attempt ends in a registry refusal, `src/engine/coordinator.rs` does not discard the checkout. It pins the
  attempt's captured candidate through `Workspace::prepare_commit_from_candidate`, at the attempt's prepared-pin name
  followed by `-kept`, and its refusal names the pin.
- `src/engine/resume.rs` discards the checkout's copy as before, so the attempt runs again from a clean tree. It finds
  the kept pin, names it in its warning, and never removes it.
- Both modules' `legacy_effect` texts take an amendment, and the regression tests are appended to the frozen
  `src/engine/tests.rs`; no existing test changes.

**The witnesses** (PR #329's record, §4.8): T-L1 to T-L6 for corrected B1′ and T-P1 to T-P5 for B-PRESERVE, among them
(e2′) itself with a topology slot's torn registration as the residue. T-L1 to T-L4 and T-P1 to T-P3 were executed on
a scratch prototype, and `switch_branch`'s sequence at the Git level
(`~/orch-pr11/logs/pr11_fub_design5/census/probe-v/SUMMARY.txt`); every existing legacy test passed on it.

**If the owner declines.** Without B-PRESERVE, (e2′) remains an applicable P1 and blocks G6, and (e1′) remains a P1;
this file stays open, narrowed to the residue. Without either, (e2) and (e2′) both block G6 and the file stays as it
is. The owner may instead rule that the mixed residue case does not block G6, with this P1 kept filed; without that
ruling no waiver is inferred.
