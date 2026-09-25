---
id: PR5-RD-003-A-PRUNE-STRANDS-A-CHECKOUT-WHOSE-GITDIR-IS-GONE
severity: P2
disposition: deferred
category: crash-consistency
pr: 308
reviewed_sha: db67a82b48f308d4b933830c57784a3e902c726b
location: src/workspace_manager.rs:4833
provenance: pre_existing
first_bad: PR5-RD-003
guard: an owner decision on which of `PR5-RD-003`'s two convergence rules gives way, taken up by the next change to the missing-store branch of `revalidate_removal_proving`
---

## Failure sequence

Two rules `PR5-RD-003` settled, both owner-authorized (`reviews/FINDINGS.md` §24), meet in one state.
A registration whose `gitdir` is already absent is forced-cleanup convergence, and "Git prune handles
its own metadata"; and "a missing whole worktrees store refuses while the checkout target exists".

1. A slot's registration has lost its `gitdir`, with no `locked`, while its checkout and its intent
   stand: the state `an_absent_registration_gitdir_is_already_gone_for_forced_cleanup` converges from.
   No registration binds the checkout; Git's enumeration skips the entry, and `git worktree prune`
   deletes it.
2. Another slot's forced removal ends in `git worktree prune`. It deletes that entry and, when nothing
   else is registered, `<common git dir>/worktrees` itself.
3. The slot's own forced removal then refuses on every attempt. `revalidate_removal_proving` finds the
   store absent and the checkout present and returns `Io { path: "…/.git/worktrees", … NotFound }`
   before the funnel, so every call of `remove_worktree` or `remove_worktree_proving` for the slot,
   `reclaim_intents` among them, refuses with it.

**Executed** with a scratch test that was not committed, over `src/workspace_manager.rs` as it is at
`db523cd3`, at `db67a82b` and at the round-3 fix of #308. The setup was a healthy `alpha` and a
`charlie` whose `gitdir` had been deleted. At all three, `remove_worktree(alpha)` returned `Ok(())`
and the store was gone, and both of `charlie`'s removals returned that `Io` error, with its checkout
still in place.

**After the repair.** `remove_intent`'s repair (`PR5-RD-002-ENGINE-RECLAIM-LOOPS`) removes a torn
registration, and while that registration stood, `git worktree prune` could not empty the store (it
skips a `locked` entry). The executed sequence was: `alpha`'s worktree removed; `bravo` torn;
`charlie` with no `gitdir`; `delta` healthy; then `remove_intent(alpha)`, `remove_worktree(delta)` and
`remove_worktree(charlie)` twice. At `db67a82b` and at the round-3 fix, the repair removed `bravo`'s
registration and `delta`'s prune emptied the store, so `charlie` refused twice. At `db523cd3`,
`remove_intent(alpha)` refused on Git's enumeration dying on `bravo`, and `bravo`'s registration
stayed. `delta`'s prune therefore left the store in place, and `charlie` converged. In that run the
store survived only because the registration that makes every enumeration refuse was still in it.

**Graded P2, on consequence.** The failure is fail-closed: the refusal comes before any deletion. The
cost is liveness, the consequence `PR8-CRASH-002-PACKED-REFS-LOCK` is graded P2 for. It is
unreachable from the shipped binary, because no non-test code constructs a `WorkspaceManager`. The
precondition is not one a killed `git worktree add` leaves: traced with `strace` on Git 2.43.0, the
add writes `locked` before `gitdir` and unlinks it only after the checkout is populated. Reaching
this state takes a `gitdir` lost after a finished add, or an outside edit. There is a manual
recovery, executed but documented nowhere: recreate the empty `<common git dir>/worktrees` and run
again. The slot's removal then converges, and its own prune deletes the directory again.

## What the change that takes this up should do

The owner has to decide which rule gives way in this state, because both are theirs. Two shapes, not
taken in #308:

- The missing-store branch of `revalidate_removal_proving` converges for a checkout an intent names.
  `a_missing_stored_worktree_directory_refuses_before_checkout_deletion` pins the refusal for exactly
  such a checkout (its slot has an intent) and would change with it. A store moved away and a store
  Git pruned are the same state on disk.
- Or no forced removal prunes an entry whose checkout may stand. That means removing only the slot's
  own proved registration in place of `git worktree prune`, which the ordinary path cannot do for a
  registration that no longer names its checkout.

Commit the scratch sequence above as the witness: `charlie`'s removal converging twice after `alpha`'s
removal has pruned the store.

Round 3 took the prune out of the empty-`commondir` branch, and
`PR308-R3-SKIPPED-PRUNE-KEEPS-ANOTHER-RUNS-TORN-REGISTRATION` records what that cost, so the prune is the
trade-off between the two findings and one change should settle both.
