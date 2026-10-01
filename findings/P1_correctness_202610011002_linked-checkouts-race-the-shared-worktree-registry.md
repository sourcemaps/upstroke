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
guard: the follow-up change that makes registry access exclusive across coordinator processes, closing the verification path too, after PR11 and before G6 certifies R17 ("second coordinator refused") under concurrency
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
    -> B's pipeline reads the registry while A's registration is half written: its snapshot add's
       `git worktree list`, or its snapshot removal's registration scan; B's registry lock is
       process-local and does not exclude A
    -> B's funnel returns a Git error: the removal gate refuses A's registration ("is locked and has
       no gitdir"), or `git worktree list` fails reading it
    -> what that error costs depends on the pipeline B's funnel ran in:
       - an attempt, whose judgement adds and removes its snapshots in the pipeline: the error is the
         pipeline's (`src/engine/topology/coordinator.rs:1413`), so B cancels its other, valid
         pipelines and the command ends resumably
       - a verification, whose judgement adds its snapshots in the pipeline: `run::verified`
         (`src/engine/topology/run.rs:279`) maps the Git error to `Verified::Unavailable`, and the
         frozen `integrate.rs` appends `merge_verification_unavailable` ("foreign Git state observed
         by the verification of sequence …"), its outcome chosen at `:871`: Deferred, spending one
         of the candidate's deferrals, or, once the deferrals taken plus this one reach
         `max_defers`, Parked with an unblock question; with nothing else admissible the run then
         finishes Parked

**Measured.** The lens ran two processes holding these production locks, each driving the production
`WorkspaceManager` snapshot funnels: 5 of 1,000 operations failed — four "locked and has no gitdir",
one reading another registration's `commondir`. Round R7 measured provenance (the PR11 record's §13,
"Review round 7 (round R7): narrowing"): a port of the same witness onto the merge base `79979d24`,
using only what the base has, failed 5 runs of 5 with 29 failed operations in 5,000; the lens's own
witness at the narrowed head failed 5 of 5 with 27 in 5,000; every failure was a registration another
process had half written or half removed. So the class is the base's: PR11 did not introduce it.

**The verification path, executed** (review round 8's `R8-CONC-1`, reproduced in round R8: the PR11
record's §13, "Review round 8 (round R8)"). The lens's witness leaves the registration B would meet —
`HEAD` and `gitdir` written, `commondir` empty — at a verification's snapshot add, deterministically,
against unchanged production code at `d5da7485`. One such read appended one
`merge_verification_unavailable`, Deferred, and the run completed after its backoff; two, with
`max_defers` 2, appended Deferred and then Parked, an unblock question and `run_finished(Parked)`.
Each time the torn registration was removed right after the terminal was appended, and nothing of the
terminal was undone; the live state equalled replay. The cross-process order is reasoned, as the
lens's is: the witness writes the state another process's add leaves, rather than racing one.

**What PR11 added.** A process-local registry lock keyed by the common git dir (the record's `R-X`),
which serializes the registry among one coordinator's threads and, by design, not across processes;
and a false claim: the record's `R-AN` read the worktree lock as excluding a second coordinator from
the whole repository. It excludes one only from the same checkout. `R-AN` is corrected (record §3).

**Consequence and reach.** Two paths, and on both the log stays consistent and replay reproduces it.
In an attempt the failing funnel's error is a pipeline error, and the coordinator's pipeline-error
path "cancels otherwise valid work and ends the command resumably" (review round 7's lens; the record's
`R-AB`): nothing is appended for it, and the next resume settles the open attempts interrupted. In a
verification the same error is an outage of the sequence, and it is durable: each one spends one of
the candidate's deferrals, and the one that reaches `max_defers` parks the candidate with an unblock
question someone must answer — retry it, or skip it and block its dependents. A's add completing does
not undo it: the deferrals stay spent and the question stays open, over a registry that was whole a
moment later. The coordinator's own registry work around a verification — the staging add before
`merge_verification_started`, the staging and snapshot reclaim after the terminal — fails as a
coordinator-side error and ends the command resumably, after the terminal when it is the reclaim. No
production path reaches the topology engine in this build (the record's `R-G`). It stays P1,
`deferred`: on 2026-10-01 the owner kept PR11's narrowed scope and split this finding out into a
follow-up change due after PR11 and before G6, without reclassifying it and without waiving G6 (the
record's §12). Beside it: a host-runner agent's own `git worktree prune` racing an engine add is
`PR11-HOST-AGENT-PRUNE-RACES-AN-ENGINE-ADD`, the same registry raced from inside an agent, with the
same two paths.

## What the change that takes this up should do

The follow-up change, after PR11 and before G6 certifies R17 under concurrency. **Its remedy must close
the verification path as well as the pipeline path**: a registration another coordinator is half-way
through writing must never reach `run::verified` as foreign Git state, so that no deferral is spent and
no candidate parked on it. A remedy that only makes the pipeline path resumable, or retries a failed
funnel inside an attempt, leaves the verification's durable deferral or park in place.

The lens proposed two remedies; each has a cost, and each removes the race at its cause, so each closes
both paths.

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
and at round R7's narrowed code (round R7's run). It should drive a verification as well as attempts,
and with the race closed no `merge_verification_unavailable` is appended for another process's
registration. Review round 8's witness writes the torn state itself, past any lock, so it cannot tell a
fixed race from an open one; what it holds is the mapping, which stays right for foreign Git state
nobody can exclude.
