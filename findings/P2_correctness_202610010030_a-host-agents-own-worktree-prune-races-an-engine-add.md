---
id: PR11-HOST-AGENT-PRUNE-RACES-AN-ENGINE-ADD
severity: P2
disposition: deferred
category: correctness
pr: 327
reviewed_sha: 1fe988cd140dab27731206db4812ab2a927618b7
location: src/workspace_manager.rs:1596
provenance: introduced_by_feature
first_bad: 1e0cd0fe9af7f440f0abcb5005694ba20e33114d
guard: PR12, the slice that lets a production run go above max_parallel = 1 — before a host-runner agent runs beside another pipeline of its run
---

## Failure sequence

At `max_parallel` above one, with the **host** runner: pipeline A's agent runs `git worktree prune`
(or `git worktree add`) in its own task worktree — an agent edits files and runs commands, and Git's
worktree housekeeping is among them — while the coordinator adds a snapshot for pipeline B through
the same repository's registry.

    the engine's `git worktree add` creates `.git/worktrees/<name>` and writes `locked`, `gitdir`
    and `commondir` one at a time
    -> A's `git worktree prune`, which is outside this process's registry lock, finds the entry
       with no `gitdir` and removes the administrative directory
    -> the engine's add fails ("failed to read .../commondir") or finishes over a registration
       that is gone, and a later funnel's `git worktree list` or removal scan refuses it
    -> B's judgement fails with a Git error, and what that costs depends on B:
       - an attempt's: the command ends resumably over a registry another process tore (the
         coordinator's pipeline-error path)
       - a verification's: `run::verified` (`src/engine/topology/run.rs:279`) maps it to an outage of
         the sequence, and `merge_verification_unavailable` is appended, spending one of the
         candidate's deferrals or, at `max_defers`, parking it with an unblock question; the prune
         being over undoes neither

The engine serializes every registry access it makes with a process-local lock keyed by the common
Git directory (`registry_lock_of`, the PR11 record's `R-X`), measured: the witness
`concurrent_snapshot_adds_and_removals_on_one_repository_never_fail` passes 20 of 20 runs with the
lock and fails in 7 of 10 with the lock made per call. A host agent's Git is a different process and
takes no such lock; the race it re-opens is the one the design memo measured, two to three failed
adds in 600 with four concurrent add-and-prune loops (`~/orch-pr11/logs/pr11_research_c/`). The
container runner does not have it: an agent there works in a disposable Git view that cannot reach
the repository's registry (`design/26_design_merge_queue_protocol.md`).

**The verification's branch** is the one `PR11-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY`
names (review round 8's `R8-CONC-1`; the PR11 record's §13, "Review round 8 (round R8)"): whatever
process tore the registry, a Git error inside a verification's judgement is `run::verified`'s foreign
Git state, a durable deferral or park and not a resumable end. Round R8 executed that mapping with a
registration left half written at a verification's snapshot add; for this finding's prune it is read,
not executed.

**Latent.** No production run reaches the topology engine above width 1 in this build: a fresh run's
`max_parallel > 1` is refused by the configuration and production writes schema 3 (the PR11 record,
`R-G`). P2 for that reason — a real race masked by the shipped width, as
`PR7-R3-ATTEMPT-002-REVIEWERS-TAKE-NO-SLOT` was until PR11 closed it.

## What the change that takes this up should do

Before host-runner agents run beside each other, one of: refuse `max_parallel > 1` with the host
runner (the container runner's view already prevents the race); give a host agent a Git view that
cannot reach the shared registry, as the container runner does; or make the engine's registry
operations survive an external prune — verify each add under the lock and retry a torn one before
it reaches a judgement. The last must cover the verification's branch too: a torn registration
that reaches `run::verified` spends a deferral, so retrying the attempt's pipeline error alone is not
enough. Whichever it is, measure it with an agent-side prune loop against the engine's adds, as the
memo's race scripts measured the engine against itself, with a verification among them.

## Reconciliation (2026-10-02, PR #329's design round 8)

`PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`
(`findings/P1_correctness_202610021308_an-external-prune-deletes-an-engine-worktrees-registration.md`, P1) is the one
finding for a prune no engine process starts. It covers every starter, this agent's included, and both faces. It
carries their grading and their G6 disposition: it blocks G6 absent a closure or the owner's ruling. This file no longer
grades either face.
- **The branch "finishes over a registration that is gone"** is that finding's face 2 (PR #329's R13). Its
  consequences reach a valid candidate's `MergeRejected`, a durable deferral or park, recovery deleting unpinned
  edits, and a finalization that does not converge.
- **The branch "the engine's add fails"** is that finding's face 1. Once PR #329 is implemented, a failure after Git
  took the add's destination over refuses at once as a resumable registry refusal, never as Git state. The
  verification's deferral or park no longer follows from it, and the resumable end that remains is face 1's.

**What this file keeps** is a host agent's Git reaching the shared registry at all, and its remedies under its PR12
guard: refuse `max_parallel > 1` with the host runner, or give the agent a Git view that cannot reach the registry, as
the container runner does.

**Corrected 2026-10-02, PR #329's design round 9** (its record §8.7, answering design review round 8's
FUB-D8-HOSTWIDTH). Round 8 said either remedy also removes this starter from the P1. Only the second does:
- refusing `max_parallel > 1` removes the overlap inside one run. Two width-one runs in two linked checkouts of one
  repository are each admitted by their own checkout's lock, so one run's host agent can still prune while the other
  run adds, in either face of the P1;
- a Git view that cannot reach the registry, the container runner's or an equivalent on the host, removes the agent as
  a starter.
