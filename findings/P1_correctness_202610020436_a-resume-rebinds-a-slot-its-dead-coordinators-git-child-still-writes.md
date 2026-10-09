---
id: PR329-A-RESUME-REBINDS-A-SLOT-ITS-DEAD-COORDINATORS-GIT-CHILD-STILL-WRITES
severity: P1
disposition: deferred
category: correctness
pr: 329
reviewed_sha: 8dd2214cff10edb777e842c181b66357e0c72012
location: src/engine/topology/dispatch.rs:251
provenance: pre_existing
first_bad: predates PR11, and the engine's own Git children have never been killed with their coordinator on Unix; prior ID PR136-REMOVE-WORKTREE-VS-A-GIT-CHILD-NOTHING-KILLED (the same class, its liveness face, filed on fix/sampler-kill-and-inspection, which never merged); executed on master 92c4ca81's code by PR #329's design rounds 2 and 3
guard: follow-up C, the fix-P1 change named after this file, due before G6 and briefed by the PR11 orchestrator in fu-c-orphan-git-writers-before-slot-reuse.md under its briefs/followups directory
---

## Failure sequence

One coordinator, one run, one checkout. No second engine is involved.

    coordinator A is SIGKILLed inside an engine Git write -- `git worktree add` for a task
    slot tasks/k<key>-g<gen>, or a `reset`, or an `update-ref`
    -> on Unix nothing kills A's Git children: the Git process G, the subprocesses Git
       itself starts (`update-ref` and `reset` under `worktree add`, a configured filter,
       `checkout--worker` under parallel checkout) and anything those start outlive A
    -> A's resume takes the worktree and run locks; `verify_worktree` reads Git's
       `locked: initializing` as `VerifyFailure::Unpopulated`
       (`src/workspace_manager.rs:2771-2772`)
    -> `dispatch::verify_or_recreate` removes the slot and adds it again at the same path
       (`src/engine/topology/dispatch.rs:248-253`); Git names a registration after its
       path's basename, so the recreated registration has the same administrative directory
    -> the worker runs in the recreated slot and writes its paid edits
    -> the orphan acts on the slot's paths:
       - a late `reset --hard`, run with `GIT_DIR=<slot>/.git`, resets the recreated
         checkout, and the edits go back to `base`; `git status` is clean, so nothing
         records the loss
       - a smudge filter's background helper, which Git fed through a pipe and which kept
         an absolute slot path, does the same after Git has exited
       - the orphaned add's `remove_junk` deletes the recreated registration and checkout,
         by path, edits and all
       - an orphaned `update-ref` rewrites the replacement registration's `HEAD`
    -> the diff the engine judges is no longer the agent's (`DESIGN.md` §4, "ground truth is
       the diff"), or the paid work is gone

**Windows.** The same sequence is reasoned and not executed there. The coordinator's kill-on-close
job ends its children when it dies, but job termination is asynchronous, like process termination,
and pending I/O must finish or be cancelled. Nothing in the successor establishes that the dead
coordinator's job has emptied before `dispatch.rs:251` removes and recreates the slot. The cleanup
lease's probe is a no-op on Windows (`src/rundir.rs:2522`).

**Cross-run too.** A dead run's freed administrative name can be taken by another run's add with the
same basename, and `k1-g1` is every run's first task. Design review round 2's concurrency lens
executed an orphaned `update-ref` rewriting such a replacement's `HEAD`.

## Evidence

- **Design review round 2** (`0874bcf3`), executed on Git 2.43.0 (FUB-D2-DESC). Git's `update-ref`
  and `reset` children of `worktree add` outlived their parent and wrote after recovery. A worktree
  file went from "paid worker edits" back to "base", and a replacement registration's `HEAD` was
  rewritten (`~/orch-pr11/reviews/review-329-d2-{design,concurrency}-0874bcf3.review.md`).
- **Design round 3**, executed with one coordinator through the production `WorkspaceManager`
  funnels and the coordinator's real locks, on an out-of-tree probe built from `0874bcf3`
  (`reviews/2026-10-01-pr11-follow-up-b-record.md` §2.2; the files are under
  `~/orch-pr11/logs/pr11_fub_design3/witness/c-single/`):
  - `witness-exec.log`: the late `reset` reverts the edits;
  - `witness-filter.log` and `witness-filter-strace.log`: the orphaned add's `remove_junk` deletes
    the recreated registration and checkout;
  - `witness-locks-{exec,filter}-base.log`: unpatched, the resume proceeds and the edits are lost.
- **Design review round 3** (`8dd2214c`), all three lenses (FUB-D3-DESC-FILTER, executed;
  FUB-D3-DESC-WIN, reasoned).
  - A smudge filter started a background helper with its standard streams redirected. The filter
    failed, Git removed its junk and exited, and the helper stayed in the original process group,
    with no `setsid`. The resume recreated the slot, and the helper's delayed `git reset --hard HEAD`
    turned "paid worker edits" into "base"
    (`~/orch-pr11/reviews/329-d3-witnesses/pr329-d3-reg-recreated-hjzj6eur/result.txt`).
  - Git's `checkout--worker` also gets its standard input through a pipe (Git's
    `parallel-checkout.c`, v2.43.0, lines 431-450).
  - The Windows ordering above is reasoned in the concurrency, regression and design-recast lenses'
    texts.

## What failed as a remedy, and why

Each of #329's design rounds 1 to 3 tried to keep recovery from reusing a slot while a dead
coordinator's Git writer could still act on it. Each was found to miss a process.
- **A lock handed to the Git child** (round 1). On Windows its release is not ordered after the
  child's termination, and a filter's descendant can keep it.
- **A record of the holder and its children, by pid and start identity** (round 2). Updating it is
  not crash-safe; under `hidepid` a hidden live process reads as gone; and Git's own subprocesses
  are never in it.
- **The run's cleanup lease, handed to Git writers as their standard input** (round 3). Git feeds a
  filter, and `checkout--worker`, through pipes, so their descendants never hold the lease. On
  Windows the lease's probe does nothing.

The lesson all three teach: any remedy that must enumerate, or be inherited by, every process Git may
start will meet the next process it did not account for.

## Why it blocks G6

It applies to G6 under Q1, which asks that crash residue be "reclaimed or repaired per the
fault-injection registry before any slot reset, admission, or resource reuse". It applies under the
crash and recovery obligations ST-16 and ST-18 exercise, and under INV-22's resource accounting. An
applicable open high finding fails G6. **Filing it here is not a waiver**, and neither is its
deferral.

## Same class as PR136-REMOVE-WORKTREE-VS-A-GIT-CHILD-NOTHING-KILLED

Yes. That finding's file was
`reviews/findings/P2_correctness_202609042055_remove-worktree-vs-a-git-child-nothing-killed.md` on
`fix/sampler-kill-and-inspection` (PR #145, closed unmerged). Its sequence is "The engine dies …
while `WorkspaceManager::add_worktree` has a `git worktree add` in flight. Nothing kills that child
… Its descendants … keep writing into the new worktree". Recovery's forced removal then fails
`DirectoryNotEmpty` and does not converge. That is the **liveness** face. This file adds the
**corruption** face of the same precondition: the removal succeeds and the orphan writes afterwards.
PR136 named the missing capability as telling the in-flight window from the residue, which "needs
liveness of the writer". On master that id is cited only from
`findings/P3_docs-contract_202609050648_unbindable-task-registration-has-no-design-sentence.md`.
This file tracks both faces.

## What the change that takes this up should do

Establish, before any slot is reset or reused, that no process the dead coordinator's Git writes
started can still act on that slot's paths. Do that on Unix and on Windows, and with no list of
processes that a filter, a helper or Git's own worker can escape. Then witness it with this file's
sequences.

**Requirements, from rounds 1 to 3:**
- It must hold for processes Git starts that the engine never sees: `update-ref`, `reset`, a
  configured filter and anything it starts, and `checkout--worker`.
- It must not depend on inheriting a descriptor through standard input, or on a record updated in
  place, or on `/proc` visibility.
- On Windows it must observe completion, not just initiate termination: that the old job is empty.
- It must keep the resume's refusal resumable and say what it waits for. A hung writer may hold
  recovery until an operator ends it, as a stuck cleanup reaper does today, and that must be
  stated.

**Candidate directions** (the follow-up C brief's), to be evaluated with evidence, choosing the
simplest sound one:
1. **Group or job emptiness before reuse.** Each engine Git write runs in its own process group
   (Unix) or job (Windows), recorded durably before it starts. Recovery reuses a slot only once that
   group or job is established empty. A process that leaves the group (`setsid`) escapes it; state
   that as a residual and grade it.
2. **Unique slot paths per incarnation**, so that an orphan's path is never reused. The packet names
   the slot paths literally (`tasks/k<key>-g<gen>`), so this is a packet change and the owner's
   decision. Escalate it; do not assume it.
3. **Disable configured filters and parallel checkout for the engine's Git writes**, saying what
   that changes for users (LFS is one example).
4. **A documented residual for user-installed helpers that persist**, which is the owner's call and
   only if 1 to 3 leave it, with its severity argued.

Keep the change's hunks in `src/workspace_manager.rs` disjoint from #329's, which are the registry
access wrapper and targeted removal. Whichever of the two merges second rebases.

## What #329's implementation keeps for this finding's follow-up (2026-10-03)

Follow-up C's U depends on #329's removal (#330's R-P). #329 runs no explicit engine `git worktree prune`, binds
removal to the instance's own registration, and adds no global-prune fallback
(`no_production_argv_of_the_manager_names_prune`,
`no_removal_prunes_another_processs_registration_and_the_store_goes_only_when_empty`). This finding is unchanged and
open.

## What #330's implementation closes, and why this finding stays open (2026-10-03)

**Closed: the slot-reuse routes.** PR11 follow-up C's implementation (#330, `reviews/2026-10-02-pr11-follow-up-c-record.md`,
its "Implementation" section) builds U: a slot instance per coordinator incarnation, named by a tag of the incarnation's
id, whose production form carries a draw from the host; every walk reaching every incarnation's instance; no retention;
a final sweep at terminal finalization; and the switch set (maintenance, rerere, relative paths, lazy fetch, transports,
prompts) on every engine Git child. No resume recreates or verifies a slot at a path or registration name an earlier
incarnation used, so the sequences above — the late `reset --hard`, the junk removal of the recreated registration and
checkout, the late `HEAD` write, a filter's helper — act on the dead incarnation's own instance. Two routes are witnessed
through the production funnels: `desc_filter_route_a_dead_adds_junk_removal_cannot_reach_the_successors_slot` and
`desc_helper_route_a_dead_filters_late_helper_cannot_reach_the_successors_slot`, each red when every incarnation renders
one tag.

**Why it stays open.** The record's §5.8 leaves residuals of this finding's class under U that the implementation does
not close, each filed where its owner decides it:
- **R-REF**, the Windows ref write a terminated `update-ref` can land after its successor reclaimed the lock:
  `PR330-A-DEAD-COORDINATORS-WINDOWS-REF-WRITE-CAN-LAND-AFTER-ITS-RESUME-RECLAIMED-THE-LOCK`, the owner's D5 (O7).
- **R-UR**, an instance a still-running earlier writer creates after the final sweep, and its accounting:
  `FUC-D5-ACCOUNT`, the owner's D1 (O4).
- **R-GU**, a prune no engine process starts: `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION` (O1).
- **R-G2**, legacy-started maintenance pruning a registration in its add window: `FUC-D2-RG`, follow-up D's (O8, O10).
- **FUC-D5-GITINDEXFILE**, an inherited `GIT_INDEX_FILE` defeating instance isolation: filed with `FUB-D9-ENV` (O3 or O3-R).

The two frozen oracles §5.8 also lists are replaced and demonstrated by this change (R-O1 to R-O3 with their mutations),
so they are not among the residuals. The record does not say this change closes the whole finding; it is deleted when
the residuals above are closed or accepted by the owner, and #330's merge itself waits on the owner's adoption of the
record's erratum E-FUC-3, whose naming the implementation follows.
