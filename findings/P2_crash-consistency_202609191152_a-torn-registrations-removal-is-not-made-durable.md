---
id: PR5-RD-002-TORN-REMOVAL-NOT-DURABLE
severity: P2
disposition: deferred
category: crash-consistency
pr: 
reviewed_sha: 2104c36d4ed295d723531d99a59cbf3461276566
location: src/workspace_manager.rs:2759
provenance: pre_existing
first_bad: PR5-RD-002
guard: the next change to the empty-`commondir` branch of `remove_worktree_proving`, or to how a removal makes Git's administrative state durable
---

## Failure sequence

Reasoned from the code at `2104c36d`, and not executed: it needs a power loss.

`remove_worktree_proving` repairs a registration whose `commondir` is zero-length by deleting the
administrative directory it has bound to the slot (`remove_tree_once_handles_close(admin)`,
`src/workspace_manager.rs:2759`), then runs `git worktree prune` and returns. The checkout's deletion
is made durable inside the same funnel (`sync_checkout_removed`, the `SyncedDirectory` barrier on
the slot kind's directory), but nothing syncs `<common git dir>/worktrees` after the administrative
directory is deleted, and Git's prune does not either. The caller then removes the slot's intent,
and `remove_intent` syncs the intents directory.

On a filesystem that does not persist one directory's earlier deletion when another directory is
synced — or with the private root and the repository on different filesystems — a power loss after
that sync can keep the intent's removal and lose the administrative directory's. The torn
registration then comes back (`locked`, a zero-length `commondir`, a `gitdir` naming an absent
checkout) with no intent naming its slot. Git's enumeration dies on it; `reclaim_intents` deletes an
administrative directory directly only when it has bound it to an intent's slot, and the
`git worktree prune` its removals run skips a registration holding `locked`; so every call that
revalidates refuses until an operator runs `rm -rf <common-git-dir>/worktrees/<slot>`. That is a
refusal and deletes nothing, but convergence needs an operator.
`a_torn_registration_no_intent_names_still_refuses_the_reclaim` pins the reclaim's refusal over a
torn registration that no intent names, in two shapes whose checkouts are present; the state this
leaves has none.

A healthy registration brought back the same way does no such harm: Git enumerates it, and a later
prune removes it because its checkout is gone.

Which of the filesystems the engine runs on reorder these two operations was not measured. Not
reachable in the shipped binary: at `2104c36d` no code outside `#[cfg(test)]` constructs a
`WorkspaceManager`.

## What the change that takes this up should do

Measure first whether a supported filesystem reorders them. If one does, make the administrative
directory's deletion durable before the funnel returns — a directory barrier on
`<common git dir>/worktrees`, recorded on the durability ledger as the checkout's barrier is — so
the intent's removal cannot outlive it, with a witness that reads the ledger rather than the
directory's state, as the checkout barrier's witnesses do.
