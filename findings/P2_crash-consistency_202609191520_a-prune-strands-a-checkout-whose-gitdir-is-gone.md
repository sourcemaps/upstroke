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
guard: an owner decision on which of `PR5-RD-003`'s two convergence rules gives way, taken up with closure 3 of `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION` or by the next change to the missing-store branch of `revalidate_removal_proving` after #329
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

## At #329's implementation (2026-10-03)

**The engine no longer produces this state.** #329 removed every `git worktree prune` the manager ran: a forced removal
deletes only the registration its scan bound to its own slot, and the store only when that leaves it empty
(`remove_bound`; `reviews/2026-10-01-pr11-follow-up-b-record.md` §2.5 and §3.5). Step 2 above, another slot's removal ending in `git worktree prune`, no longer
happens. That is this file's second shape, "no forced removal prunes an entry whose checkout may stand", and neither of
§24's two rules gave way. Witnessed by `no_removal_prunes_another_processs_registration_and_the_store_goes_only_when_empty`.

**What remains.** A prune no engine process starts (a user's, an agent's, Git's automatic or scheduled maintenance) can
still empty the store while such a checkout stands, and the missing-store branch still refuses that checkout on every
attempt. That is the store-gone shape of `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, its closure 3.
#329 changed the missing-store branch only to let an empty directory at the target bind nothing (the destination an add
makes before Git runs), which a standing checkout is not, and took no owner decision, so this file stays open and its
guard carries to that closure or the next change to the branch.

## The owner's decision, and closure 3 in #333 (2026-10-15)

**The guard is answered: Choice A.** On 2026-10-15 the owner adopted, with the external-prune follow-up (F, #333) that
carries closure 3 of `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, "Choice A under PR5-RD-003,
including the specified populated-checkout deletion and contents where the store is absent and the own-store relation
holds, except recovery task reclaim" (the owner's adoption record, recorded 2026-10-15T22:09:21Z, on the owner's
message "adopt all three recommended packages"). Its mechanical recording, as the owner's adoption names it:

> The owner adopts Choice A. PR5-RD-003's rule "a missing whole worktrees store refuses while the checkout target exists" gives way
> for exactly the convergence addendum's §4.1 relation (`6a1d2142` :96-141, :275-306, :310-330). Rule 1 and case 3 are unchanged.
>
> The owner accepts, by name, this deletion: with `<common git dir>/worktrees` absent, and a populated real directory whose `.git`
> names a direct child of this repository's own store, every removal that reaches the scan except recovery's task reclaim deletes
> that checkout and everything in it, with no prior operator message (F report §5.1, :428-456). A store moved aside, or a dangling
> link, reads as absent.

The convergence addendum (v2, `6a1d2142…`) and the F decision report (`f460ac26…`) it cites are documents of the
owner's F package, outside this repository. RULING X-1, adopted the same day, does not answer this guard, and neither
does this file's label.

**What #333 changes.** `revalidate_removal_proving`'s store-absent branch gains one arm, before its
`Io { path: worktrees, .. }` answer: a target that is a real directory, not empty, whose `.git` is a regular file
`gitdir: <path>` naming a direct child of this repository's own `<common git dir>/worktrees`, compared after
`canonical_prefix` on both sides (`names_an_entry_of`), binds nothing, and `remove_bound` removes the checkout with
whatever it holds. No store is made, and nothing of an absent or moved-aside store is touched. The arm does not check
which child of the store the `.git` names. With the store absent, the scan has three cases:
1. an absent target or an empty directory converges, as #329's arms already made it;
2. a populated checkout in that relation converges, removed with whatever it holds, at every removal that reaches the
   scan except recovery's task reclaim, where RULING P-1's guard keeps a populated task slot for the operator first;
3. every other populated or unrecognized target keeps the refusal on every attempt: no `.git`; a `.git` that is a
   directory or a link, cannot be read, or has no `gitdir:` prefix; a path that does not decode or canonicalize; a
   `.git` naming anything but a child of this repository's own store, another repository's store among them.

Rule 1, "a registration whose `gitdir` is already absent is forced-cleanup convergence", is unchanged.

**The witnesses.** The pinned refusal this file names, `a_missing_stored_worktree_directory_refuses_before_checkout_deletion`,
is renamed in place to `a_missing_stored_worktree_directory_converges_removing_only_the_checkout` (T-C3-1): with the
store moved aside, the removal removes only the checkout and leaves the moved store's bytes as they were.
`a_complete_runs_finalization_converges_over_a_populated_slot_with_the_store_gone` and
`a_halted_runs_finalization_converges_over_a_populated_slot_with_the_store_gone` (T-C3-2) finalize a `Complete` and a
`Halted` run over a populated slot whose entry, the store's last, was pruned.
`a_removal_with_no_store_refuses_every_target_that_names_no_entry_of_its_own_store` (T-C3-CONTROLS) keeps case 3's
refusal for another repository's store and for a populated target with no `.git`, and case 1's convergence; #329's
`a_removal_with_no_store_takes_an_empty_destination_and_still_refuses_a_checkout` still refuses a `.git` that names
no entry of this store. Mutation M-C3, the arm binding nothing for any populated directory, turns those two red.

**What it means for this file.** The failure sequence above leaves a checkout whose `.git` still names its deleted
entry, which is case 2: it now converges at step 3, and the manual recovery is not needed for it. Case 3's shapes
still refuse on every attempt, and their convergence is not addressed. Whether this file is now deleted as resolved,
or kept open and re-guarded for case 3, is the ledger process's call (`findings/PROCESS.md`), not this update's; until
it is made, the frontmatter stands as filed.
