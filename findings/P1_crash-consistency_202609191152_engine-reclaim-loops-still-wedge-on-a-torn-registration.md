---
id: PR5-RD-002-ENGINE-RECLAIM-LOOPS
severity: P1
disposition: deferred
category: crash-consistency
pr: 300
reviewed_sha: 2104c36d4ed295d723531d99a59cbf3461276566
location: src/engine/topology/finalize.rs:251
provenance: pre_existing
first_bad: PR5-RD-002
guard: the change that gives `WorkspaceManager` a caller outside `#[cfg(test)]`, or the next change to `scrub_slots`, `reclaim_snapshot_residue` or `reclaim_stale_residue`
---

## Failure sequence

`PR5-RD-002`'s mechanism, left in the engine's loops by the change that closed it, which changed
`WorkspaceManager::reclaim_intents` and nothing in `src/engine/`.

A `git worktree add` killed while it writes `commondir` leaves that file zero-length, and
`git worktree list` then dies before it emits any record (`fatal: failed to read
.git/worktrees/<slot>/commondir: Success` on glibc, `Undefined error: 0` on macOS). The torn slot's
own forced removal repairs it without asking Git, deleting only the administrative directory it has
bound to that slot; but every `remove_intent` revalidates through Git's enumeration before it acts.
These three loops remove one slot's worktree and then its intent before moving to the next slot:

- finalization's `scrub_slots` (`src/engine/topology/finalize.rs:240-260`, the pair at `:251-256`);
- the resume's `reclaim_snapshot_residue` (`src/engine/topology/recover.rs:1158-1169`), snapshot
  slots only;
- the staging loop of the resume's `reclaim_stale_residue` (`src/engine/topology/recover.rs:1212-1219`),
  staging slots other than the live one.

So a torn registration of a slot the loop reaches after another slot refuses the loop at the earlier
slot's `remove_intent`, on every attempt, and the loop never reaches the removal that would repair
it. Read from the code and not executed: the two resume loops filter by slot kind, so a torn
registration of a slot of another kind — a task slot's — is never removed by either of them, and
refuses whichever of them has a slot of its own kind to reclaim, at that slot's `remove_intent`,
whatever the order. Whether an earlier step of the resume removes such a registration first was not
traced.

**Executed at `2104c36d`**, where `reclaim_intents` already removes every worktree before any
intent: the manager calls in `scrub_slots`' order — `remove_worktree_proving(slot,
WriterProof::NoWriterAlive)` then `remove_intent(slot)`, for each slot of `intents()` — over
`PR5-RD-002`'s fixture V8 (intents `alpha` and `bravo`, `bravo`'s `locked` reading `initializing`
and its `commondir` zero-length), three passes. Each pass returned
`Err(Git { message: "git worktree list --porcelain -z failed in …/repo: fatal: failed to read
.git/worktrees/kbravo-g1/commondir: Success" })`, and the intents stayed `[alpha, bravo]`. The
control, with the torn registration on `alpha`, converged in one pass. That drove the manager calls
in `scrub_slots`' order from a scratch test that was not committed, not the private `scrub_slots`
itself; the gate G5 report records the same result from `scrub_slots` itself, made `pub(crate)` in a
scratch copy at an earlier head (`reviews/2026-09-16-gate-G5.md`, its `PR5-RD-002` entry).

**Reachability.** At `2104c36d` no code outside `#[cfg(test)]` constructs a `WorkspaceManager`:
`git grep -n "WorkspaceManager::derive(" -- src` returns 19 call sites, all in modules declared
`#[cfg(test)]`, and each of the three loops is handed a manager by its caller. The shipped binary
does not reach them.

## What the change that takes this up should do

In `scrub_slots`, the two passes `reclaim_intents` now runs: every slot's worktree, then every
slot's intent, which keeps each slot's worktree-before-intent order and its durability barrier.

The two resume loops need a decision before that, because their kind filter makes a torn
registration of another kind not theirs to remove: reclaim every torn registration an intent names
before the kind-filtered loops run, or accept that each refuses until the step that owns that kind
has run. Either way the removal keeps the proof `remove_worktree_proving` uses — the administrative
directory bound to the slot being removed — and nothing enumerates around the torn entry:
`PR5-RD-002`'s round-7 repair, a directory scan when Git's enumeration failed, was reverted because
containment checks then ran over a list shorter than Git's.
