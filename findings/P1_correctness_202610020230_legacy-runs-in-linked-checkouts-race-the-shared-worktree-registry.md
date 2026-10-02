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
guard: the change that routes `src/workspace.rs`'s four registry Git children through the workspace manager's tolerant registry access, under the owner's decision to unfreeze the module for it (decision B, B1′): PR #329 if the owner takes B1′ before its implementation (its record, §3.10), or a later change
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

**G6, as the lenses split.** This file does not decide it.
- **The pure legacy-against-legacy case.** The concurrency and regression lenses hold that it does
  not apply to G6: it exercises the frozen legacy error path, not topology recovery, slots or
  ledgers. G6 claims nothing of the legacy engine frozen at PR5.
- **The mixed case:** a topology writer tearing a legacy reader. The design-recast lens holds that
  it applies through shared-registry R17 conformance and Q6, whose sequential guarantees include an
  untouched user checkout. On that view it blocks G6 while open.
- **Filing waives neither case.**

## What the change that takes this up should do

**B1′ is the remedy, and it needs the owner's decision to unfreeze `src/workspace.rs` for one change**
(decision B; the PR11 orchestrator's escalation item 7).
- Route the four registry Git children through the workspace manager's tolerant registry access
  (record §3.3), made callable crate-internally for the purpose.
- One private helper resolves the canonical common git dir in the two steps `recorded_objects_scope`
  already takes.
- A failure the store shows contended is attempted again, and one still contended at the access's
  deadline refuses, never as Git state.
- Nothing else in the module moves, and no other legacy module moves.
- The recorded form is the second amendment in `effects/allowlist.toml`'s `legacy_effect` text for
  the module, worded in record §3.10. The access's name joins the manager's `effect_free` list in
  `effects/wrappers.toml`.
- **The witnesses:** the record's T10 with the roles swapped (a topology writer in the main checkout
  and the legacy engine in a linked one: 0 legacy failures), and the two lenses' sequences above,
  under which B's snapshot add succeeds and nothing is discarded.

**What B1′ leaves.** It closes every write in flight that finishes within the deadline. It does not
close a registration that stays torn: the legacy access then refuses after the deadline, and the
frozen coordinator discards on that refusal as it does on any failed attempt. Closing that needs the
legacy coordinator not to discard on a registry refusal, which is a second unfreeze, of
`src/engine/coordinator.rs`. So under B1′ this file stays open, narrowed to that residue, at P1, with
that remedy.

**Under B2′,** with the module frozen, this file stays as it is. Both cases above go to G6's reviewers
as they stand, and the mixed case is an applicable open high finding if they agree with the
design-recast lens.
