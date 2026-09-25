---
id: PR308-R3-SKIPPED-PRUNE-KEEPS-ANOTHER-RUNS-TORN-REGISTRATION
severity: P2
disposition: deferred
category: crash-consistency
pr: 308
reviewed_sha: b3a8dbfe408478aaab3f7d192ad95be46a9a18c0
location: src/workspace_manager.rs:2821
provenance: fix_regression
first_bad: PR5-RD-002
guard: the change that settles `PR5-RD-003-A-PRUNE-STRANDS-A-CHECKOUT-WHOSE-GITDIR-IS-GONE`, taken up with it: the next change to the empty-`commondir` branch of `remove_worktree_proving` or to the missing-store branch of `revalidate_removal_proving`
---

## Failure sequence

Round 3 of #308 (`93a78abc`, a repair within `PR5-RD-002`'s fix) made the empty-`commondir` branch of
`remove_worktree_proving` return once it has removed the slot's proved registration
(`src/workspace_manager.rs:2821`). Before, the branch fell through to `git worktree prune`. Taking the
prune away fixed `PR308-R2-REPAIR-PRUNES-A-PASSED-OVER-CHECKOUTS-STORE`, where it deleted a registration
with no `gitdir` and then the emptied store while that slot's checkout still stood. The prune also
deleted registrations that Git prunes because the checkout their `gitdir` names is gone, and one of
those can be another run's torn registration, which no repair of this run may plan.

1. Two runs share a repository, each with its own manager and execution root. Run A's intended slot
   `bravo` and run C's intended slot `charlie` each have a registration torn the way a killed
   `git worktree add` leaves one: `locked` holding `initializing` and a zero-length `commondir`
   (`tear_registration`).
2. Run C's forced removal of `charlie` stops after it has deleted the checkout and unlinked `locked`
   (`src/workspace_manager.rs:2769`), before it deletes the administrative directory (`:2807`). The
   registration keeps a `gitdir` naming the deleted checkout and its empty `commondir`, and run C's
   intent stays. `git worktree prune` deletes that entry, since it has no `locked` and its checkout is
   gone, and Git's enumeration dies on it.
3. Run A's `verify_worktree(bravo)`: the revalidation refuses, and `repair_torn_registrations` plans
   from run A's intents only, so it runs `bravo`'s forced removal and no other. That removal deletes
   `bravo`'s checkout and registration and returns at `:2821`. Before `93a78abc` it then ran
   `git worktree prune`, which deleted `charlie`'s registration too.
4. The revalidation after the repair dies on `charlie`'s `commondir`, and the verification returns
   `Err(Git { … fatal: failed to read .git/worktrees/kcharlie-g1/commondir: Success })`. Retries return
   the same error (three attempts measured): `bravo`'s registration is gone, so the plan is empty, and
   `charlie`'s intent is run C's.

The same return shows without a verification. `bravo`'s own forced removal returns `Ok(())` and leaves
Git's enumeration dying on `charlie`'s registration. Run A's `reclaim_intents` over the setup refused
with the same error. Its first pass is that removal, and read from the code, the error comes from its
second pass's `remove_intent(bravo)`, whose repair excludes `bravo` and finds nothing else of run A's
to plan. Its second attempt converged: `bravo`'s removal then binds nothing and takes the ordinary
path, whose prune deletes `charlie`'s registration.

**Executed** with scratch tests that were not committed, appended to a copy of
`src/workspace_manager/tests.rs` outside the worktree, over `src/workspace_manager.rs` as it is at
`b3a8dbfe`, at `db67a82b` and at `db523cd3`, with both managers derived before the tears (Git 2.43.0,
Linux):

| after the setup | `b3a8dbfe` | `db67a82b` | `db523cd3` |
|---|---|---|---|
| `remove_worktree(bravo)`, then `git worktree list --porcelain -z` | `Ok(())`, then exit 128 on `kcharlie-g1/commondir` | `Ok(())`, then exit 0 | `Ok(())`, then exit 0 |
| `verify_worktree(bravo)`, then a retry | the `Err` above, twice | `Ok(Err(NotRegistered))`, twice | `Err` on `kbravo-g1/commondir`, twice |
| run A's `reclaim_intents`, twice | `Err` on `kcharlie-g1/commondir`, then `Ok` | `Ok`, then `Ok` with nothing left | `Ok`, then `Ok` with nothing left |

The first two rows are the round-3 regression lens's two sequences. Run together, cargo exited 101 at
`b3a8dbfe`, 0 at `db67a82b` and 101 at `db523cd3`, where the first alone exited 0. At `db523cd3` the
verification refuses on `bravo`'s own torn registration, which is `PR5-RD-002` itself.

Two variants bound the precondition, run at the same three revisions. When `charlie` is run A's own
intended slot, the plan names it and the verification converges at `b3a8dbfe` and at `db67a82b`
(`db523cd3` refuses on `bravo`). When only `charlie`'s registration is torn and `bravo`'s is healthy,
the verification refuses at all three: no removal runs, so nothing prunes, and a torn registration that
no intent of the root names is left in place by design (`repair_torn_registrations`).

## Grading

Graded on consequence:

- **Fail-closed.** The verification refuses with Git's own error. After it, `charlie`'s administrative
  directory, run C's execution root and run A's intents directory were each identical, entry by entry,
  to what they were before it (`tree_bytes`). Of what the setup holds, it deleted `bravo`'s checkout
  and registration, which its repair exists to delete, and `bravo`'s intent stays.
- **Not reachable from the shipped binary.** `git grep -n "WorkspaceManager::derive(" b3a8dbfe -- src`
  returns 19 call sites in four files, each a module declared `#[cfg(test)]`:
  `src/workspace_manager/fixture.rs`, `src/workspace_manager/tests.rs`,
  `src/engine/topology/candidate/tests.rs` and `src/engine/topology/recover/tests.rs`. The struct's one
  literal outside test code is in `derive` (`src/workspace_manager.rs:1439`), and its derived `Clone`
  copies a manager that already exists. The search covers those spellings only.
- **Recoverable by a full reclaim, or by Git's prune.** At `b3a8dbfe`, after three failed
  verifications, run A's `reclaim_intents` converged on its first attempt, and Git enumerated again
  (exit 0). Run C's `reclaim_intents` also converged, after which run A's verification returned
  `Ok(Err(NotRegistered))`. `git worktree prune` exited 0 and deleted `charlie`'s registration, and the
  same verification then converged. Retrying the verification did not recover (three attempts). Every
  reclaim here was made by a manager derived before the tears. A manager derived afterwards, as a
  resume in a new process derives one, refused in `derive` for both runs with the same `kcharlie-g1`
  error; after `git worktree prune`, run A's derived. That refusal is
  `PR5-RD-002-RESUME-DERIVES-THROUGH-A-TORN-ENUMERATION`'s, except that here Git's own prune removes the
  cause, because `charlie`'s registration has no `locked`.
- **A regression against `db523cd3` for the direct-removal sequence**, the first row. There
  `bravo`'s forced removal left Git enumerating (exit 0), and here it does not (exit 128). Against
  `db67a82b`, all three rows regress. The verification sequence is not a regression against the base,
  because there it refused from the first attempt on `bravo`'s own torn registration, the P1 this pull
  request closes.
- **Three crash events across two runs.** The sequence takes a torn add in each run, run A's while
  the manager that verifies already exists (one derived after the tear refuses in `derive`, above),
  and run C's forced removal stopped between unlinking `locked` and deleting the administrative
  directory. It was constructed here, as the lens constructed it, not produced by killing a process.

The cost is liveness, which is the consequence `PR8-CRASH-002-PACKED-REFS-LOCK` is graded P2 for,
and the grade the round-3 regression lens gave it. It stays P2 rather than P3 for two reasons: the
manager's retry of the call that failed does not converge, and no document this search covers names
the one operator recovery measured, `git worktree prune`, as a recovery.
`git grep -n -i "worktree prune" -- README.md DESIGN.md design docs MAINTAINING.md CONTRIBUTING.md`
returns two lines, both in `docs/internals/`, and each describes a prune the engine makes.

## What the change that takes this up should do

Settle this together with `PR5-RD-003-A-PRUNE-STRANDS-A-CHECKOUT-WHOSE-GITDIR-IS-GONE`, because the
prune is the trade-off between the two. With the prune in the empty-`commondir` branch (`db67a82b`),
a torn slot's removal can delete another slot's registration that has no `gitdir`, then the emptied
store, and that slot's removal then refuses its checkout
(`PR308-R2-REPAIR-PRUNES-A-PASSED-OVER-CHECKOUTS-STORE`). Without it (`b3a8dbfe`), a registration that
Git would prune outlives the removal, and when that is another run's torn registration, this run's
enumeration dies on it until something else removes it. Round 3 traded the first for the second.

That finding offers two shapes. In the first, the gate converges on an absent store for a checkout an
intent names. That would let this branch prune again without stranding that finding's checkout, and
with the prune, both of the lens's sequences converged at `db67a82b`. In the second, no forced removal
prunes an entry whose checkout may stand. That leaves this finding open unless whatever replaces the
prune still deletes an entry Git prunes because the checkout its `gitdir` names is gone.

Commit the scratch sequence as the witness: with run C's interrupted removal in the store, run A's
verification of `bravo` and its retry both return `Ok(Err(NotRegistered))`, and `bravo`'s forced
removal leaves Git's enumeration working.
