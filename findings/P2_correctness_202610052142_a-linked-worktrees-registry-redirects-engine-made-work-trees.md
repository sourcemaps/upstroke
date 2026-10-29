---
id: R-C-REGISTRY
severity: P2
disposition: deferred
category: correctness
pr:
reviewed_sha: 5c222ff2162da28aa471433773bf4966a883a428
location: src/workspace_manager.rs:2619
provenance: pre_existing
first_bad:
guard: the owner's choice among the proposal's undrafted routes, before G6, which cannot pass while it is applicable and open; the routes are giving every process in a work tree the engine made the run's common directory, rewriting the work tree's record after the add, refusing a linked registry at the start, or a specific owner exception
---

## Failure sequence

R-C-REGISTRY is the name the reviewed option (C) proposal gives this existing defect:
`/home/ubuntu/orch-pr11/owner-package/O3R-OPTION-C-PROPOSAL.md`, sha256
`dc94ba82515d265fd98b4aa3c8f1f8b4c699a70733bd19fe245c6fbec023eedc`, §8's table and §8.3 (`:3812`,
`:3991-4101`), called "the proposal" below. It is existing behaviour of `5c222ff2` that option (C), which is not
adopted, neither causes nor reaches.

**What it is.**
- **Registration.** A work tree the engine made, a gate or review snapshot or a slot, is registered in the `worktrees`
  directory of the run's common directory. The manager's slot add is `WORKTREE_ADD_ARGV` (`src/workspace_manager.rs:2619`);
  the legacy snapshot add is at `src/workspace.rs:871-906`.
- **The work tree's record.** Its `commondir` record, `../..`, is resolved through the real path of its administrative
  directory. Where the run's `worktrees` is a symbolic link into another repository's, that record names the other
  repository's common directory.
- **What Git reads from where.** An explicit `GIT_COMMON_DIR` comes before the record for objects and configuration.
  Git's ref backends never take it: shared refs come from the record (proposal §3.2, P5 and P7).

1. **The precondition.** The run's `worktrees` directory is a symbolic link into another repository, made by hand.
   `git worktree add` creates it as a directory, and no document of upstroke names such a link. The engine accepts it:
   the legacy constructor's checks pass, and each linked-registry shape's worker-only run works at `5c222ff2`
   (proposal §8.3, `:4029-4041`).
2. **Part 1: nothing inherited** ("symlinked registry, nothing inherited").
   - The gated run is refused at the snapshot add: Git's own checkout child reads the record through the link and
     cannot find the snapshot's commit. The coordinator's discard follows the error
     (`src/engine/coordinator.rs:544-548`, reasoned). Its reset and clean erase the worker's uncommitted output in
     the checkout; the discard's own steps are executed in the proposal's §4.10 sequences.
   - The slot run's worker and capture write their objects into the linked repository, and the commit at the base
     fails.
3. **Part 2: a `GIT_COMMON_DIR` inherited** (C4's and C5's layouts).
   - Objects, configuration and the common directory Git reports are the run's.
   - Branches, tags and every other shared ref are the linked repository's: a role that writes one in its work tree
     writes it there, and a gate or reviewer that reads one reads the linked repository's.
   - No executed sequence writes one: across the 138 rows of the six linked-registry shapes, the other repository's
     refs change in none (`E4/analysis/other-refs-census.txt`).

**Measured and reasoned.**
- **Executed** with ordinary Git by the proposal's standalone harness, on Git 2.43.0, 2.50.1 and 2.55.0, in temporary
  directories with the lenses' constructions, the regular lens's production layout included:
  - part 1's gated and slot rows (proposal §4.2, `:2126`; 2 rows on each version, §4.9);
  - Git's rule for shared refs, by P7 with the files backend.

  Records: `/home/ubuntu/orch-pr11/logs/pr11_envr_c4/`, `E4/analysis/matrix.tsv` and `E4/analysis/other-refs-census.txt`.
- **Reasoned:** the coordinator's discard after the refusal.
- **Not executed** (proposal `:4096-4101`):
  - a link inside the common directory;
  - a bind mount instead of a link;
  - Windows junctions;
  - the reftable backend;
  - other Git implementations;
  - the agent CLIs.
- **Where option (C) stands.** The relative-value part that an earlier round counted here has left this finding. Under
  the conditional option (C), an inherited relative `GIT_COMMON_DIR` reaches each process of a work tree the engine
  made as the absolute path it names at the started checkout. On 2.43.0 and 2.50.1, that shape's remainder is
  R-C-RELSTORE's (proposal `:4004-4012`).

## Severity: P2 (the proposal's proposed grade, for both parts, not adopted)

**The proposal proposes P2** (`provenance: pre_existing`; `first_bad` not traced; §8.3, `:4043-4053`). That proposal is
not adopted, and the owner classifies (`MAINTAINING.md:189-190`).
- **Why not P3.** Part 1 loses the worker's uncommitted output in a gated run, and writes another repository's object
  store in a slot run. Part 2 lets a role's shared-ref write land in another repository. Each is a data or integrity
  effect on a user repository.
- **Why below P1.** Both parts need a `worktrees` directory made, by hand, a symbolic link into another repository.
  Part 2 also needs a role that writes a shared ref in its work tree.
- **Whether those preconditions are speculative,** and so the grade, is the owner's call.

## Applicability to G6

**Applicable, conditionally on its precondition**, under the host runner, for the integrated tree (proposal
`:4055-4068`).
- **Q2 and INV-22:**
  - objects are written outside the run's store, outside every inventory row (part 1's slot run, executed);
  - a shared ref a role writes lands outside the run's repository (part 2, executed as Git's rule by P7).
- **Q6:**
  - a slot worker's view of objects and refs is another repository's (part 1, executed);
  - a gate's or reviewer's view of shared refs is another repository's (part 2, P7).
- **Not Q4,** reasoned.
- **Its relation to the O3-R finding** (`PR329-HOST-ROLES-INHERIT-THE-COORDINATORS-GIT-REPOSITORY-CONTEXT`). Neither part
  involves O3-R's precondition. Part 2 involves an inherited `GIT_COMMON_DIR`, but the link causes it, not the name.
  The proposal counts this finding closed under no reading.

**What clears it.** Neither filing it, nor its `deferred` disposition (which means pending review and the owner's
decision, not deferred past G6), nor its proposed grade.

## Step 5 and the witness

**`MAINTAINING.md:41-45`.** This finding carries executed reproductions: part 1's harness rows, and Git's rule for part 2
by P7. Under step 5's rule, a finding carrying a reproduction witness is fixed whatever its severity. It may be rejected
only by a row showing the witness does not reproduce on the head. Which of two things it is, triage and the owner
decide:
- relevant to a change that implements option (C);
- a pre-existing finding, logged under `:38-40`.

**The witness obligation.** A change that takes this up must show, failing at its base and passing after it, that a
gated run and a slot run under a linked registry keep their objects and shared refs in the run's repository, or are
refused before any work. "symlinked registry, nothing inherited" is the witness, with C4's and C5's layouts for part 2.

## The owner's choices (none is made here)

From the proposal's §8.3 (`:4073-4094`). None is drafted, and each comes with a cost grounded in Git's source:
- **Give every process in a work tree the engine made the run's common directory, inherited or not.**
  - It closes part 1's slot rows for objects.
  - It does not close part 2, or part 1's refs.
  - It closes part 1's gated rows only if the `git worktree` commands are given the value too.
  - Its costs: every role's composed environment changes in every run, and it reaches a role's Git in a scratch
    repository as an inherited value does.
- **Rewrite the work tree's record to the run's common directory, as an absolute path, after `git worktree add`.**
  - It closes both parts in that work tree's later processes.
  - Its costs: a new mechanism that writes Git's administrative files, and in this layout the write itself lands in
    the other repository. It does not close part 1's gated rows.
- **Refuse a linked registry at the start.**
  - It closes both parts by refusing them.
  - Its cost: it refuses C4's and C5's shapes, which work at `5c222ff2`.
- **A specific owner exception.**

## What the change that takes this up should do

**No remedy is chosen here.** The change follows from the owner's choice, and the witness obligation above binds it.

## Provenance

**`pre_existing`.** It holds at `5c222ff2` and under (C) alike, row for row (proposal §4.9). `first_bad` was not traced.

## Filed

- **When and by whom:** 2026-10-05, by `pr11_envc_impl1` (`claude-opus-5-5`).
- **On whose direction:** babysit_pr11's direction
  `/home/ubuntu/babysit-pr11/evidence/envc-package2-filing-disclosure-direction-20261005.md` (sha256
  `133149a64b24b625fe6457794d7ea7629c9ab2a40dfb0d75642bf475b8b34aef`), carried by orch_pr11b's answer
  `/home/ubuntu/orch-pr11/answers/pr11_envc_impl1-2.md`.
- **As what:** a record of the conditional draft implementation of option (C) on this branch. That implementation is
  unpushed and not adopted.
- **What filing does not do.** It adopts nothing: not option (C), not this residual's label, not its proposed grade. It
  is not clearance for merge or G6.
