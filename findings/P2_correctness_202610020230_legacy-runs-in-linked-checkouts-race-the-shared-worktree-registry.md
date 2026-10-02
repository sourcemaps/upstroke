---
id: PR329-LEGACY-RUNS-IN-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY
severity: P2
disposition: deferred
category: correctness
pr: 329
reviewed_sha: 92c4ca81f9209d218df4534ee71d3445dc2906e1
location: src/workspace.rs:871
provenance: pre_existing
first_bad: predates PR11: the legacy engine's four registry Git children have run untolerant since before PR5 froze `src/workspace.rs`; measured at the Git level on master `92c4ca81`'s argv by PR #329's first design round
guard: the change that routes `src/workspace.rs`'s four registry Git children through the workspace manager's tolerant registry access, under the owner's decision to unfreeze the module for it: PR #329 under decision B1′ (its record, §2.10), or a later change
---

## Failure sequence

Two legacy (schema 1–3) runs of one repository, one in the main checkout and one in a linked
checkout. Each holds its own worktree lock, because that lock is per checkout, so both run.

    run A adds a gate snapshot: `git worktree add -q --detach --force` writes its registration
    into the shared `.git/worktrees/` one file at a time
    -> run B's gate-snapshot add, its `git worktree list` after a removal, its
       `git worktree remove --force`, or its resume's `git switch` enumerates the store
       (`add_gate_worktree` `src/workspace.rs:871`, `worktree_is_registered` `:1602`,
       `cleanup_gate_workspace` `:1549`, `switch_branch` `:450`) and dies on A's half-written
       entry: "failed to read …/commondir: Success", "failed to read '…/locked'", "Invalid path"
    -> the error returns through `?` (`src/engine/attempt.rs:154`, `:178`)
    -> the legacy coordinator answers any attempt error with `discard_uncommitted()`, which is
       `git reset --hard HEAD` and `git clean -fd`, and ends the command
       (`src/engine/coordinator.rs:544-548`)
    -> the worker's uncommitted edits for that attempt are gone, and the resume runs the
       attempt again, paying for it again

**Measured at the Git level.**
- Round 1 of PR #329 ran the legacy argv concurrently in two linked checkouts
  (`reviews/2026-10-01-pr11-follow-up-b-record.md` §1.2, table C and "The legacy race").
  - Four loops per checkout: 12 and 16 failed commands in 7,200.
  - One loop per checkout, the shape one legacy coordinator per checkout gives: 0 in 5,400.
- Round 3 ran the legacy argv untolerant in the main checkout beside the topology manager's
  tolerant cycle in a linked one (record §2.10). The legacy side failed 13, 10, 14, 5 and 13 times
  in 6,000 commands a run; the topology side failed none in 40,000.

**Not reachable as R7-CONC-1's consequence.** A legacy writer can no longer tear a topology
pipeline or verification, because PR #329's topology readers tolerate any writer (record §2.4).
What remains is the legacy engine's exposure to legacy and topology writers alike.

## Grading

P2, graded on consequence.
- **Cost.** One attempt's paid work and a resumable end. Nothing durable is spent: the legacy path
  has no deferral or park, its event log stays consistent, and the resume re-runs the attempt.
- **Rate.** Low at one coordinator per checkout. The windows are a registration's few file writes.
- **G6.** It does not apply. G6 certifies the topology engine's scheduling layer and R17 under
  concurrency, and claims nothing of the legacy engine, frozen at PR5. As a P2 it would not block
  G6 anyway.

## What the change that takes this up should do

Route the four legacy registry Git children through the workspace manager's tolerant registry
access (record §2.4), exposed crate-internally for the purpose. One private helper should resolve
the canonical common git dir the way `recorded_objects_scope` does.
- A failure the store shows contended is attempted again.
- One that stays contended past the bound refuses, never as Git state.
- Nothing else in the module moves, and no legacy engine module moves.

This needs the owner's decision to unfreeze `src/workspace.rs` for one change, recorded as the
module's second amendment in `effects/allowlist.toml`. The text is the record's §2.10, B1′. The
witness is the record's T10 with the roles swapped: a topology writer in the main checkout and the
legacy engine in a linked one, with 0 legacy failures.
