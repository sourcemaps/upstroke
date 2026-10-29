---
id: R-C-SELECTION
severity: P1
disposition: accepted-risk
category: correctness
pr:
reviewed_sha: 5c222ff2162da28aa471433773bf4966a883a428
location: src/workspace_manager.rs:1626
provenance: pre_existing
first_bad: PR329-HOST-ROLES-INHERIT-THE-COORDINATORS-GIT-REPOSITORY-CONTEXT
guard: the owner's answer to it under O3 and O3-R, which is one of acceptance by name, a refusal of that shape, or a mechanism the owner commissions (none is drafted); it is open pending that review and decision, and it is not deferred past G6, which cannot pass while it is open
---

## Failure sequence

R-C-SELECTION is the name the reviewed option (C) proposal gives this residual:
`/home/ubuntu/orch-pr11/owner-package/O3R-OPTION-C-PROPOSAL.md`, sha256
`dc94ba82515d265fd98b4aa3c8f1f8b4c699a70733bd19fe245c6fbec023eedc`, §8's table and §8.1 (`:3810`,
`:3825-3893`), called "the proposal" below.

**It is part of the original O3-R damage.** The O3-R finding is
`PR329-HOST-ROLES-INHERIT-THE-COORDINATORS-GIT-REPOSITORY-CONTEXT`, filed on PR #329's branch as
`findings/P1_correctness_202610031242_host-roles-inherit-the-coordinators-git-repository-context.md` (at `492325c4`,
not on master). This file neither replaces nor narrows that finding, and `first_bad` names it as the finding this is
part of. Option (C), a conditional draft that has not been adopted, closes the roles' part of O3-R's damage and not
this part (proposal §0 item 2, §8.1).

1. **The precondition.** A coordinator started in the user's checkout (`managed`) inherits an absolute `GIT_DIR`
   naming another repository's Git directory (`other`). This is the O3-R report's case `dir`. It takes an explicit
   export, such as a `GIT_DIR` workflow or a wrapper script: the third source of the precondition in O3-R's file.
2. **The selection.** At the topology base, `WorkspaceManager::derive` (`src/workspace_manager.rs:1616-1641`) takes
   `common_git_dir(base)` (`:1626`). Under the inherited name, Git answers with `other`'s common directory.
3. **The consequence.** The manager makes its slots there, so the slot, the capture and the run's commit are all
   `other`'s.
   - At `5c222ff2`, the slot worker also addresses `other`'s main index. That is the roles' damage, O3-R's executed
     sequence.
   - Under option (C), the slot worker addresses its own slot and `other`'s main index is untouched. The run still
     operates in `other`.
   - The legacy conductor refuses these shapes, at the clean check or at the open, both at `5c222ff2` and under (C)
     (proposal §4.2: "already refused").

**Measured and reasoned.**
- **Executed** with ordinary Git by the proposal's standalone harness, not through the conductor, on Git 2.43.0,
  2.50.1 and 2.55.0. The rows are "R dir", "R dir-index" and "R all-eighteen", sequence T (proposal §4.2,
  `:2067-2069`). §4.9 counts these 3 rows apart from the closed rows (`:2499`). The harness's records are under the
  proposal's evidence, `/home/ubuntu/orch-pr11/logs/pr11_envr_c4/` (`E4/analysis/matrix.tsv`).
- **Executed: the conflict with the supported baseline.** The selection probe (`E4/git/selection.tsv`, proposal §8.1)
  asks Git, at each shape's started checkout, what the inherited names select and what discovery alone would find:
  - V4 is a supported shape that works at `5c222ff2`, and it is `R dir`'s shape exactly;
  - D1, V1 to V3, V6, E1 contrived, F4 and DOT have the same form;
  - Git records no intent that separates them.

  So any mechanism that refuses or moves `R dir`'s selection also refuses or moves those.
- **Not executed:**
  - a whole topology run through the conductor;
  - production's private-root layout;
  - Windows;
  - other Git implementations;
  - the agent CLIs (proposal §7).

## Severity: P1 (provisional)

**This is a provisional assessment, not a classification.** The proposal states no grade for this residual: "OPEN:
the owner's (not accepted; no refusal chosen; no mechanism drafted)", §8's table.

**The basis.**
- It is part of O3-R's damage, which O3-R's file grades P1.
- A run that operates in another repository makes its slots, objects and run commit there, which is the class
  `MAINTAINING.md:186` names: "loss or corruption of data in a user repository — the engine owns git".
- Its precondition is an explicit export. `MAINTAINING.md:189-190` reclassifies a P1 down when its failure needs
  speculative preconditions.

**Who decides.** The owner classifies (`MAINTAINING.md:189`). This grade is not lowered to pass any gate.

## Applicability to G6

**Applicable** under the host runner, for the integrated tree, as part of O3-R's damage.
- **Q6:** "under R-C-SELECTION the run itself operates in another repository" (proposal §5.3, `:3100`).
- **O3-R's own G6 section applies with it:** Q6, Q4, and Q2 with INV-22, conditionally on the precondition.
- **What clears it.** By O3-R's file, an applicable open P1 fails G6. Only a fix implemented and validated in G6's
  range clears it, or a specific, applicable owner exception (`/home/ubuntu/orch-pr11/g6/PLAN.md:160-168`, as O3-R's
  file cites it).
- **What does not clear it:** filing this file, its `deferred` disposition (which means pending review and the
  owner's decision), and option (C)'s closure of the roles' part.

## Step 5 and the witness

**`MAINTAINING.md:41-45`.** This finding carries an executed reproduction: the harness rows above. Under step 5's rule,
a finding carrying a reproduction witness is fixed whatever its severity. It may be rejected only by a row showing
that the witness does not reproduce on the head. Which of two things it is, triage and the owner decide:
- relevant to a change that implements option (C);
- a pre-existing finding, logged under `:38-40`.

**The witness obligation** (from O3-R's file and the proposal's §8.1). A change that takes this up must show, failing at
its base and passing after it, that:
- a run whose started checkout's inherited names select another repository no longer operates in it, with `R dir` at
  the topology base as the witness;
- every supported shape of the baseline (D1, V1 to V4, V6, E1 contrived, F4, DOT) keeps working, or carries the
  owner's disposition for each one the change refuses.

## The owner's choices (none is made here)

From the proposal's §8.1:
- **accept it by name;**
- **answer it with a refusal of that shape.** On the executed evidence, such a refusal also refuses the supported
  shapes above. The narrowest drafted so far is (A)'s question asked at the started checkout only, with (A)'s F1 and
  F2 obligations and its A-WORKER costs. It is not drafted here;
- **commission a mechanism.** None is drafted.

## What the change that takes this up should do

**No remedy is chosen here.** The change follows from the owner's choice, and the witness obligation above binds it.

## Provenance

**`pre_existing`.** At `5c222ff2` the T rows already operate in `other`, and also write its main index. `first_bad`
was not traced. The field names the O3-R finding, of which this is a part.

## Filed

- **When and by whom:** 2026-10-05, by `pr11_envc_impl1` (`claude-opus-5-5`).
- **On whose direction:** babysit_pr11's direction
  `/home/ubuntu/babysit-pr11/evidence/envc-package2-filing-disclosure-direction-20261005.md` (sha256
  `133149a64b24b625fe6457794d7ea7629c9ab2a40dfb0d75642bf475b8b34aef`), carried by orch_pr11b's answer
  `/home/ubuntu/orch-pr11/answers/pr11_envc_impl1-2.md`.
- **As what:** a record of the conditional draft implementation of option (C) on this branch. That implementation is
  unpushed and not adopted.
- **What filing does not do.** It adopts nothing: not option (C), not this residual's label, not any grade, not any
  acceptance. It is not clearance for merge or G6.

## Accepted by name (2026-10-26, recording the owner's adoption of 2026-10-15)

**The owner's act.** On 2026-10-15 the owner adopted the ENV package, "adopt all three recommended packages" (the
owner's adoption record `818e7675…`, `adopted.ENV.choices[2]`: "R-C-SELECTION exact acceptance by name, conditional
on(C) merged and validated."). It is the first of the owner's choices above, made by the owner: a finding that carries
a reproduction witness is not an author's or a reviewer's to accept (`MAINTAINING.md` step 5). This file records it,
with disposition `accepted-risk`, in the pull request that merges option (C).

**The acceptance, verbatim** (the ENV decision report `68042083…`, §2.3a):

> **R-C-SELECTION, accepted by name.** A run's repository is the repository Git resolves at the checkout the
> coordinator was started in, under the coordinator's inherited environment, as `Workspace::open` and
> `WorkspaceManager::derive` accept it (option (C), rule 1). Where an exported `GIT_DIR`, `GIT_WORK_TREE` or
> `GIT_COMMON_DIR` selects a repository other than the one discovery alone would find from that directory, a topology
> run makes its slots, captures, candidate refs, objects and its `upstroke/run-<ulid>` ref in the selected repository,
> as every Git command in that environment would; under option (C) each role, and each engine Git child in a work tree
> the engine made, addresses only that work tree there. No refusal is added: Git records nothing that separates an
> intended selection (D1, V1 to V4, V6, E1 contrived, F4, DOT) from an unintended one (`R dir`). The finding stays filed,
> at its grade, with this acceptance recorded.

**When it holds.** With option (C) merged and validated, as adopted. Without (C), `R dir` also writes the other
repository's main index, which this acceptance does not cover.

**What G6 states.** The gate cites this ruling with its narrowed claim: Q6, and the parent's Q2 and Q4, are answered
for the run's repository as this acceptance defines it (`/home/ubuntu/orch-pr11/g6/PLAN.md:160-168`, as O3-R's file
cites it).

**Kept as filed.** The grade, P1 and provisional, which the owner classifies, and the text above this section, except
one fact that moved: O3-R's file, cited above at PR #329's branch, is on master at
`findings/P1_correctness_202610031242_host-roles-inherit-the-coordinators-git-repository-context.md`.
