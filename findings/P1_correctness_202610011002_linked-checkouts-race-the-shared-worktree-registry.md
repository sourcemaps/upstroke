---
id: PR11-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY
severity: P1
disposition: deferred
category: correctness
pr: 327
reviewed_sha: 92593723ab83116753974bbae1941cb4a878200a
location: src/workspace_manager.rs:1592
provenance: pre_existing
first_bad: predates the merge base 79979d24, where the two-process witness fails (not bisected); the worktree lock is keyed by each checkout's own git dir there as here (src/rundir.rs, worktree_lock_file)
guard: before G6 certifies R17 ("second coordinator refused") under concurrency, unless the owner reclassifies it
---

## Failure sequence

Review round 7's concurrency lens (`R7-CONC-1`, executed at `92593723`): two topology coordinators of
one repository, one in the main checkout and one in a linked checkout (`git worktree add`), each with
pipelines that add and remove snapshot worktrees.

    A takes the main checkout's worktree lock, B the linked checkout's — two different files, because
    the lock lives in each checkout's own git dir (`worktree_lock_file(worktree_git_dir)`), not in the
    common git dir — and each takes its own run's lock
    -> A's pipeline adds a snapshot: `git worktree add` writes its registration under the shared
       `.git/worktrees/` one file at a time
    -> B's pipeline removes its own snapshot, and its registration scan reads every registration —
       A's half-written one included; B's registry lock is process-local and does not exclude A
    -> B's removal gate refuses A's registration ("is locked and has no gitdir"), or B's
       `git worktree list` fails reading it, and B's funnel returns an error
    -> through the coordinator's pipeline-error path B cancels its other, valid pipelines and the
       command ends resumably

**Measured.** The lens ran two processes holding these production locks, each driving the production
`WorkspaceManager` snapshot funnels: 5 of 1,000 operations failed — four "locked and has no gitdir",
one reading another registration's `commondir`. Round R7 measured provenance (the PR11 record's §13,
"Review round 7 (round R7): narrowing"): a port of the same witness onto the merge base `79979d24`,
using only what the base has, failed 5 runs of 5 with 29 failed operations in 5,000; the lens's own
witness at the narrowed head failed 5 of 5 with 27 in 5,000; every failure was a registration another
process had half written or half removed. So the class is the base's: PR11 did not introduce it.

**What PR11 added.** A process-local registry lock keyed by the common git dir (the record's `R-X`),
which serializes the registry among one coordinator's threads and, by design, not across processes;
and a false claim: the record's `R-AN` read the worktree lock as excluding a second coordinator from
the whole repository. It excludes one only from the same checkout. `R-AN` is corrected (record §3).

**Consequence and reach.** The failing funnel returns an error, and the coordinator's pipeline-error
path "cancels otherwise valid work and ends the command resumably" (the lens's words; the record's
`R-AB`). No production path reaches the topology engine in this build (the record's `R-G`). The lens labeled it P1; the owner classifies whether it is serious
(`MAINTAINING.md`, "Serious P1"). Beside it: a host-runner agent's own `git worktree prune` racing an
engine add is `PR11-HOST-AGENT-PRUNE-RACES-AN-ENGINE-ADD`, the same registry raced from inside an agent.

## What the change that takes this up should do

The lens proposed two remedies; each has a cost.

- **Serialize registry access across processes by the common git dir**: a cross-process lock taken
  wherever `registry_lock_of` is taken today, keyed by the common git dir rather than the process.
  Cost: a new lock resource and a new effect site — packet rows for both (the resource accounting
  table beside R17, the effect-site inventory) and the effect census that classifies every site.
- **Exclude topology coordinators repository-wide**: one coordinator per common git dir. Cost: R17's
  meaning changes. The worktree lock is per checkout today, the legacy resume takes it
  (`src/engine/resume.rs:148`), and the frozen `recover.rs` acquires it, so it cannot be re-keyed
  without changing production paths the frozen set depends on; a second, repository-wide lock is the
  first remedy's cost again.

Whichever is chosen, the phase-5 proof should include linked checkouts: the two-process witness in
this file's measurement is the regression test, red at the merge base, at `92593723` (the lens's run)
and at round R7's narrowed code (round R7's run).
