---
id: PR5-RD-002-RESUME-DERIVES-THROUGH-A-TORN-ENUMERATION
severity: P2
disposition: deferred
category: crash-consistency
pr: 308
reviewed_sha: 99fd1f2ebf58febc566bc1b00336181414bff50a
location: src/workspace_manager.rs:1448
provenance: pre_existing
first_bad: PR5-RD-002
guard: the change that gives `WorkspaceManager` a caller outside `#[cfg(test)]`, or the next change to how `run_recovery_order` obtains its manager
---

## Failure sequence

`PR5-RD-002`'s mechanism at the two places a resume meets Git's enumeration before any of its steps
can repair what makes that enumeration die. A `git worktree add` killed while it writes `commondir`
leaves that file zero-length, and `git worktree list --porcelain -z` then dies before it emits any
record (`fatal: failed to read .git/worktrees/<name>/commondir: Success` on glibc, `Undefined error: 0`
on macOS). At `99fd1f2e` the repair runs inside `WorkspaceManager::remove_intent` and
`WorkspaceManager::verify_worktree`, when their revalidation refuses: the forced removal of every slot
an intent names whose registration is torn. Two earlier enumerations get no such repair.

1. **`WorkspaceManager::derive`**, `src/workspace_manager.rs:1448`, `manager.revalidate()?`. A resume
   runs in a new process, so it derives its manager after the add was killed: the derivation refuses,
   and the resume never starts. `run_recovery_order` takes the manager as a seam and takes the run lock
   itself (`LocksHeld::take`, its first statement), so the manager is always derived **before** the
   run lock is held.
2. **`WorkspaceManager::assert_publishable`**, `src/workspace_manager.rs:3131`, reached from the
   resume's `ensure_recorded_integration_ref` (`src/engine/topology/recover.rs:1370`, and through
   `ensure_integration_ref`, `src/engine/topology/create.rs:2189`) in every resume whose transaction is
   not `Prepared`, and from `integrate::publish` (`src/engine/topology/integrate.rs:464`) in step (f)'s
   `Prepared` arm. No step before either one is sure to remove an intent, so neither is sure to meet a
   store that `remove_intent`'s repair has already cleared. It is reached only by a manager derived
   before the tear, so no resume in a new process reaches it while (1) refuses first.

**Executed at `99fd1f2e`'s source**, a scratch test (not committed) over the recover suite's fixture
with an open generation whose own registration was torn, an add a killed conductor can leave:
`WorkspaceManager::derive` after the tear returned `Err(Git { message: "git worktree list --porcelain -z
failed in …/repo: fatal: failed to read .git/worktrees/k0-g0/commondir: Success" })`. A whole
`run_recovery_order` with a manager derived **before** the tear and the manager as its refs returned the
same error with no `Ref.CreateIntegration` phase observed (the last were `RunDir.RemoveMarker` and
`Event.ProvePrefixStable`), where the same resume with the recording refs double, whose
`assert_publishable` asks Git nothing, observed that phase and went on. So the refusal comes from
`ensure_integration_ref`, and read from the code, its `refs.assert_publishable` — the manager's, which
runs `git worktree list` — is the one call there that asks Git for the worktree list. With the
double, the resume refused next at step (g)'s `verify_worktree` before this change gave that method the
repair; it now converges there (`a_resume_over_a_torn_open_generation_recreates_its_worktree`).

## Grading

Graded on consequence, not inherited from `PR5-RD-002`'s label:

- **Fail-closed.** `derive` refuses before it constructs anything, and `assert_publishable` is a check
  that refuses before its callers write: `ensure_integration_ref` creates the ref only after it, and
  `publish` swaps the integration ref only after it. Nothing is written or deleted; the refusal repeats
  on every resume of the run.
- **Not reachable from the shipped binary.** At `99fd1f2e`, `git grep -n "WorkspaceManager::derive(" --
  src` returns 19 call sites, all in modules declared `#[cfg(test)]` (`src/workspace_manager/fixture.rs`,
  `src/workspace_manager/tests.rs`, `src/engine/topology/candidate/tests.rs`,
  `src/engine/topology/recover/tests.rs`).
- **No documented manual recovery.** No user-facing document (`README.md`, `DESIGN.md`, `design/`,
  `MAINTAINING.md`, `docs/`) describes one. Git's own tools do not remove the registration, measured on
  Git 2.43.0 over a registration torn this way: `git worktree remove --force`, and with `--force` twice,
  exits 128 with the same `fatal: failed to read …/commondir: Success`, and `git worktree prune` exits 0
  and leaves the registration in place. An operator has to delete
  `<common git dir>/worktrees/<name>` and the slot's checkout by hand, guided only by Git's message,
  which names the file.

The cost is liveness only — a resume that refuses, resumably, until an operator acts — which is the
consequence `PR8-CRASH-002-PACKED-REFS-LOCK` is graded P2 for. The missing documented recovery is what
keeps it above P3.

## What the change that takes this up should do

Decide, with the first production caller of `WorkspaceManager`, how a resume obtains its manager over a
torn store. The repair must run after the run lock is held — before it, a live coordinator of the same
run may be adding the slot the repair would remove — and before the manager's first enumeration: derive
after `LocksHeld::take`, through a path that takes the caller's hooks and, when the revalidation
refuses, runs the forced removal of every torn registration an intent names, as `remove_intent` and
`verify_worktree` now do. That path is a new effectful entry point, to be classified in
`effects/wrappers.toml`. With the store repaired there, `assert_publishable` needs nothing of its own.
The witness is a whole resume whose manager is derived by that path after the tear, over a torn open
generation, with the manager as its refs, recreating the generation's worktree.
