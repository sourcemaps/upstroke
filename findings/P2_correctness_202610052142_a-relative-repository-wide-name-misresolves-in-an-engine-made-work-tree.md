---
id: R-C-RELSTORE
severity: P2
disposition: deferred
category: correctness
pr:
reviewed_sha: 5c222ff2162da28aa471433773bf4966a883a428
location: src/runner/host/environment.rs:349
provenance: pre_existing
first_bad:
guard: the owner's choice among the proposal's routes, none of them drafted, before G6, which cannot pass while it is applicable and open; the routes are a mechanism that gives a process in a work tree the engine made each relative repository-wide value made absolute against the started checkout, a specific owner exception, or a refusal of relative repository-wide names at the start
---

## Failure sequence

R-C-RELSTORE is the name the reviewed option (C) proposal gives this inherited defect:
`/home/ubuntu/orch-pr11/owner-package/O3R-OPTION-C-PROPOSAL.md`, sha256
`dc94ba82515d265fd98b4aa3c8f1f8b4c699a70733bd19fe245c6fbec023eedc`, §8's table and §8.2 (`:3811`,
`:3895-3989`), called "the proposal" below. It is a defect at `5c222ff2`, and option (C), which is not adopted, leaves
all of it standing except one part, stated below.

**What it is.** Git resolves a relative repository-wide path against the directory of the process that reads it. A work
tree the engine made is a different directory from the checkout the coordinator was started in. So a relative value
that is right at the started checkout names something else there, or nothing.

1. **The precondition.**
   - The coordinator's environment holds a relative value for a repository-wide name: `GIT_OBJECT_DIRECTORY`, an
     entry of `GIT_ALTERNATE_OBJECT_DIRECTORIES`, `GIT_SHALLOW_FILE`, `GIT_GRAFT_FILE` or `GIT_CONFIG`.
   - No launcher the proposal executed hands one (`E4/git/launchers.tsv`, proposal §4.4), so it takes an explicit
     export.
   - For silent misresolution, rather than a refusal, the relative position must also name an existing store or file
     from the work tree.
2. **The process.** A Git child the engine runs in a work tree it made, or a role started there, inherits the value:
   - the host runner's role base is the whole process environment (`src/runner/host/environment.rs:349`);
   - the builders keep the repository-wide names.

   The process resolves the value against its own directory.
3. **The consequence.**
   - **A store beside the slot** ("REL objects, sibling"): the slot worker's and the capture's objects land there, and
     the review-input check reads there. The candidate's commit at the base then exits 128, so the run fails closed,
     after it has written outside the repository.
   - **Nothing beside:** the run fails closed. On 2.43.0 and 2.50.1 it fails at the snapshot or slot add; on 2.55.0, at
     the snapshot verification or the slot worker.
   - **A gated run that fails closed at the snapshot** is followed by the coordinator's discard, which erases the
     worker's uncommitted output in the checkout (`src/engine/coordinator.rs:544-548`). The discard is reasoned; its
     reset and clean are executed in the proposal's §4.10 sequences.
   - **History and configuration:** a role in a slot reads a different shallow list, graft file or configuration file,
     or none (proposal §4.6).
   - **Legacy worker-only runs work:** at the root, the value resolves as the user's own Git resolves it there.

**Measured and reasoned.**
- **Executed** with ordinary Git by the proposal's standalone harness, on Git 2.43.0, 2.50.1 and 2.55.0, in temporary
  directories, with a store copied beside the work trees as the design lens built it:
  - `GIT_OBJECT_DIRECTORY` in W, G and T (proposal §4.2, rows "REL objects, …", `:2120-2122`);
  - a relative alternate, shallow, graft and `GIT_CONFIG` value, in a slot (proposal §4.6).

  Records: `E4/analysis/matrix.tsv` under `/home/ubuntu/orch-pr11/logs/pr11_envr_c4/`. The per-version count of
  R-C-RELSTORE rows is in §4.9 (12, 12 and 6, `:2500`).
- **Reasoned, not executed:**
  - relative `GIT_CONFIG_GLOBAL` and `GIT_CONFIG_SYSTEM`;
  - other relative spellings;
  - Windows' drive-relative and root-relative forms;
  - production's private-root layout;
  - other Git implementations;
  - the agent CLIs.

**Where option (C) stands.** These statements describe the conditional, unadopted option (C). None of R-C-RELSTORE's
rows is counted closed (proposal §4.9).
- **What it closes.** (C) closes `GIT_COMMON_DIR`'s part in every process of a work tree the engine made. Each such
  process is given the path the value names at the started checkout.
- **What stays.** Git's own checkout child of `git worktree add` keeps the relative `GIT_COMMON_DIR`. It inherits it
  from the `git worktree` command run at the started checkout, and on 2.43.0 and 2.50.1 it reads objects through it
  from the new work tree, so the add fails closed (proposal §3.2, P4; §4.1).
- **What it leaves alone.** Every other name above.

## Severity: P2 (the proposal's proposed grade, not adopted)

**The proposal proposes P2** (`provenance: pre_existing`; `first_bad` not traced; §8.2, `:3944-3953`). That proposal is
not adopted, and the owner classifies (`MAINTAINING.md:189`).
- **Why not P1.** The precondition is an explicit relative export of a repository-wide name. For silent misresolution
  it also needs an existing store at the relative position. That is the kind of precondition for which a P1 "is
  reclassified down" (`MAINTAINING.md:189-190`).
- **What stands against a lower grade.** A gated run that fails closed at the snapshot loses the worker's uncommitted
  output to the coordinator's discard, at `5c222ff2` and under (C) alike. That is a loss of data in a user repository
  (`MAINTAINING.md:176-190`).
- **Why not P3.** When the precondition holds, objects are written outside the repository's store.
- **Whether the precondition is speculative,** and so the grade, is the owner's call.

## Applicability to G6

**Applicable, conditionally on its precondition**, under the host runner, for the integrated tree (proposal §8.2,
`:3955-3963`).
- **Q2 and INV-22:** objects are written outside the repository's store, outside every inventory row. Executed: the
  store beside the slot changes.
- **Q6:** a role's or a check's view of history or of objects is read from outside its repository. Executed: the
  review-input check in the slot reads the store beside it, and a role in a slot reads another shallow, graft or
  configuration file.
- **Not Q4,** reasoned (proposal `:3962-3963`).

**Its relation to the O3-R finding** (`PR329-HOST-ROLES-INHERIT-THE-COORDINATORS-GIT-REPOSITORY-CONTEXT`).
- **Different preconditions.** This finding's precondition is a relative repository-wide name; O3-R's is an absolute
  `GIT_DIR` or `GIT_INDEX_FILE`.
- **Where both hold** ("REL objects, sibling, selectors"), O3-R's literal guarantee is not met for the object store.
- **Who decides.** Whether O3-R's clearance must also cover that combination is for the owner and G6's reviewers
  (proposal `:3964-3971`).

**What clears it.** A lower grade is not clearance (proposal `:3972-3973`). Neither is filing it, nor its `deferred`
disposition, which means pending review and the owner's decision, not deferred past G6.

## Step 5 and the witness

**`MAINTAINING.md:41-45`.** This finding carries executed reproductions: the harness rows above. Under step 5's rule, a
finding carrying a reproduction witness is fixed whatever its severity. It may be rejected only by a row showing the
witness does not reproduce on the head. Which of two things it is, triage and the owner decide:
- relevant to a change that implements option (C);
- a pre-existing finding, logged under `:38-40`.

**The witness obligation.** A change that takes this up must show, failing at its base and passing after it, that a
process in a work tree the engine made, under each relative name above, addresses the repository's own store, history
files and configuration, or is refused before it starts. The "REL objects, sibling" construction is the witness, and
legacy worker-only runs must keep working.

## The owner's choices (none is made here)

From the proposal's §8.2 (`:3974-3982`):
- **A fix.** Give a process in a work tree the engine made each relative value, made absolute against the started
  checkout's top level, as (C) now does for `GIT_COMMON_DIR` alone. For the other names that is "OPEN: NEEDS A
  MECHANISM", not drafted. Its known costs:
  - the same path to every launcher;
  - parsing `GIT_ALTERNATE_OBJECT_DIRECTORIES`'s list;
  - R-DELIBERATE's measured consequence for a role's Git in another repository, widened to the object store and the
    history files. R-DELIBERATE is the proposal's own label; it is not adopted here.
- **A specific owner exception.**
- **A refusal of relative repository-wide names at the start.** That is a new refusal of legacy worker-only runs that
  work at `5c222ff2`, and that is its cost.

## What the change that takes this up should do

**No remedy is chosen here.** The change follows from the owner's choice, and the witness obligation above binds it.

## Provenance

**`pre_existing`.** Row for row, the same at `5c222ff2` as under (C), except `GIT_COMMON_DIR`'s part in a work tree's
own processes. `first_bad` was not traced.

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
