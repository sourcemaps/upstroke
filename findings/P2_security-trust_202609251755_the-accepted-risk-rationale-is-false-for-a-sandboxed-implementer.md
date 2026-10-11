---
id: PR313-ACCEPTED-RISK-RATIONALE-FALSE-FOR-A-SANDBOXED-IMPLEMENTER
severity: P2
disposition: deferred
category: security-trust
pr: 313
reviewed_sha: f00e342b18a13348c7bf103e92bf611af964f975
location: src/workspace_manager.rs:751
provenance: pre_existing   # found by #313's adversarial lens (claude-opus-5-5, owner waiver 2026-09-25) at f00e342b
first_bad:
guard: a measurement of what Codex's sandbox leaves writable, or the change that takes up PR313-GIT-DISCOVERY-REDIRECT-INSIDE-THE-STATED-BOUNDARY
---

## Failure sequence

The accepted-risk rationale on `GitWorkingDirectory` is that a writer able to re-aim a discovery pointer
"could already write anywhere the engine's user can", so the redirect adds no capability. **That premise is
true of gate commands and of agents without an OS sandbox. It is false as written for a Codex implementer**,
which runs under the host runner with `--sandbox workspace-write` — a sandbox this project measured as
blocking writes outside the workspace on Linux.

A Codex implementer's workspace is the task slot, and that slot's `.git` is the discovery pointer: a `gitdir:`
file **at the root of the workspace**, inside the sandbox's writable area rather than outside it. So:

1. Suppose the sandbox leaves that file writable — **this is unmeasured** — and the task has no gate that
   executes the candidate's code.
2. One write there re-aims the capture.
3. The run loop goes from the worker straight to `capture` (`src/engine/topology/run.rs:1376-1384`), with no
   `verify_worktree` between them.
4. The capture's Git children run unconfined in the slot: `unresolved_conflicts`, then `candidate_stage`'s
   `git add` (`src/engine/topology/attempt.rs:454-512`). They set no `GIT_DIR`
   (`workspace_manager.rs:4904-4919`).
5. The effect lands in whichever git dir the pointer names — another repository, a sibling slot's, or the
   base's own, against the module header's contract that the user's checkout is "read only for base capture".
6. The `ForeignRepository` check in `quiescence` compares only the common dir
   (`workspace_manager.rs:2723-2727`), so a pointer into the same repository passes it even where it runs.

The engine would then write outside the workspace on behalf of an agent whose sandbox forbids exactly that —
which `codex.md:298-301` says the design rules out.

## Reachability, against the owner's rule of 2026-09-11

**Neither limb holds.** Not ordinary use: the shipped `upstroke run` is `coordinator::run` against the legacy
workspace (`src/engine/coordinator.rs:46-58`); `WorkspaceManager::derive` is called only from the test-only
`fixture` module and from tests, and the topology engine carries
`#![cfg_attr(not(test), allow(dead_code))]`. Not someone without push access: every writer that could re-aim a
pointer is a process the engine itself starts, running the operator's plan against the repository's content.

**Why P2 and not P1:** the consequence would reach a serious-P1 area — corruption in a user repository — but it
needs a precondition nobody has measured, and `MAINTAINING.md`'s serious-P1 rule classifies such a row down.

## What the change that takes this up should do

**Correct the sentence. No code change is required.** It appears in three places: the third bullet of the
boundary on `GitWorkingDirectory` (`src/workspace_manager.rs:751-761`), the "What the change that takes this up
should do" section of `PR313-GIT-DISCOVERY-REDIRECT-INSIDE-THE-STATED-BOUNDARY`, and #313's body under "The
stop condition, and what it measured".

- Name the writers the premise is true of: gate commands, and agents without an OS sandbox.
- Say that a Codex implementer holds the engine user's authority only **through a gate that executes its code**.
- Then either **measure** whether Codex's sandbox leaves the pointer writable — a Codex-adapter test in the
  style of its Windows-refusal measurements — or file the gate-less Codex implementer as the residual this
  boundary does not cover.

For whoever takes the residual, two directions, neither verified here:

- check the slot's pointer between the worker's exit and the capture: it should name the slot's own admin dir,
  whose `commondir` names the manager's common dir. This closes the case where the writer has exited, not one
  still running;
- run the slot's Git children with `GIT_DIR` and `GIT_WORK_TREE` taken from values the manager holds, which
  takes the pointer out of discovery altogether.

## Provenance

Found by #313's **adversarial** lens at `f00e342b`, reasoned rather than executed: settling it needs a write at
that path from inside Codex's sandbox, which the lens's brief excluded. Recorded here under the owner's ruling
of 2026-09-25T17:51Z that #313 merges under the 09-11 merge bar and the wording corrections live in the
findings rather than in that pull request.
