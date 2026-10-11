---
id: PR313-CONTAINER-CLAUSE-CITES-A-MECHANISM-THAT-MISSES-THE-CHECKOUTS-GIT
severity: P3
disposition: deferred
category: docs-contract
pr: 313
reviewed_sha: f00e342b18a13348c7bf103e92bf611af964f975
location: src/workspace_manager.rs:736
provenance: pre_existing   # found by #313's adversarial lens (claude-opus-5-5, owner waiver 2026-09-25) at f00e342b
first_bad:
guard: the change that next states the `GitWorkingDirectory` boundary per runner
---

## Failure sequence

The doc's stated reason for the boundary is that "the redirect gives such a writer nothing it does not already
hold". **That is false under the container runner, where the writer is confined** — so the clause reaches the
right conclusion by the wrong route, and cites a mechanism that does not cover the path it needs to.

1. The container clause says the party is "confined to its mounts" and cites `Withheld::AuthoritativeGit`.
2. The slot checkout is one of those mounts, **read-write**.
3. `Withheld::AuthoritativeGit` names `<repo_root>/.git` and the request's common git dir (`exec.rs:104`,
   `:407-410`). It does **not** name `<slot>/.git`.
4. What actually keeps the checkout's pointer out of reach is the **overlay** at
   `/upstroke/workspace/.git` (`exec.rs:494-503`). The doc does not mention it, though #313's body does
   ("the checkout's `.git` position is a bind mount").
5. So a reader who checks the doc against the mechanism it cites concludes the pointer is writable under the
   container runner. **It is not.**

**The confinement is in fact complete**, which is why this is a documentation row and not a security one:
`commondir`, `worktrees`, `packed-refs`, `refs`, `index` and `config` are never projected, and objects are
mounted read-only (`exec.rs:489-493`). The doc's shorthand understates its own strength.

**Executed** (six tests, in the lens's §1): the mount set, the overlay as the daemon holds it, read-only
objects, a read-only root, refusal of a repository-root workspace, Git inside resolving the role view, and
withheld host files byte-identical after a gate tries to write them. **Reasoned:** that the host pointer cannot
be reached through the overlay — a mount point returns `EBUSY` to unlink and rename, and `umount` needs
`CAP_SYS_ADMIN`, which is not granted.

## Reachability, against the owner's rule of 2026-09-11

**Neither limb holds. This is a doc.** No shipped path can even select the container runner: `upstroke run`
refuses `[runner] kind = "container"` — *"the container runner is selectable only by schema-4 runs"*
(`src/config/parse.rs:166-191`, via `src/config.rs:806-813`).

## What the change that takes this up should do

State the boundary **per runner** rather than giving one reason for both.

- **Host:** the writer already holds the user's authority — **with the narrowing in
  `PR313-ACCEPTED-RISK-RATIONALE-FALSE-FOR-A-SANDBOXED-IMPLEMENTER`**, which is not true of a Codex
  implementer.
- **Container:** no process the engine starts can reach these paths, because the checkout's `.git` is
  **overlaid by the role view's gitfile** and nothing of the common git dir is mounted except objects,
  read-only (`ContainerRunner::mounts`, `Confinement`). The only writers left are processes of the engine's
  user outside the engine, and they gain nothing.
- Also say that **no shipped path can select the container runner**, so the clause describes the design and its
  tests rather than a run today.

## Provenance

Found by #313's **adversarial** lens at `f00e342b`. #313's **fix-check** lens saw the same gap and read it as
the pull request understating its own strength rather than as a defect; the stricter reading is recorded here
because the citation does not support the sentence as written. Filed under the owner's ruling of
2026-09-25T17:51Z.
