---
id: ENVC-SLOT-REVIEW-INPUT-READS-THE-INHERITED-SELECTION
severity: P2
disposition: deferred
category: correctness
pr:
reviewed_sha: 5c222ff2162da28aa471433773bf4966a883a428
location: src/engine/attempt.rs:251
provenance: pre_existing
first_bad:
guard: a change that gives the schema-4 driver's review-input check a slot's own Git context while the legacy ladder's check at the started checkout keeps the inherited one (option (C)'s seam method problem_in_work_tree is one such change, conditional and not adopted), or FUD-D4-ENV's guard's change where it removes the inherited context from git_command; pending review and the owner's decision, not deferred past G6
---

## Failure sequence

1. **The precondition.** The coordinator inherits D1's names: `GIT_DIR` selecting `B.git`, and `GIT_WORK_TREE` naming
   `A/work`, inside another repository `A`. D1 is a supported shape.
2. **The capture.** The schema-4 driver captures a task's tree in its slot, then asks the review-input seam,
   `ReviewInputPolicy::problem(staging, tree)`, at `src/engine/topology/run.rs:388`.
3. **The open.** `LegacyReviewInputPolicy::problem` opens the slot with `Workspace::open` (`src/engine/attempt.rs:251`).
   Its Git children come from the legacy builder `git_command` and inherit both names.
4. **The refusal.** The inherited `GIT_WORK_TREE` excludes the slot from the work tree Git opens, and the check fails
   "not a git worktree". The attempt is refused, and the run fails closed.

**Measured and reasoned.**
- **Executed with ordinary Git** by the reviewed option (C) proposal's standalone harness
  (`owner-package/O3R-OPTION-C-PROPOSAL.md`, sha256
  `dc94ba82515d265fd98b4aa3c8f1f8b4c699a70733bd19fe245c6fbec023eedc`, §0 item 2, `:234-236`): at `5c222ff2` the check
  fails in 6 slot shapes.
- **Executed through this code:** the slot and driver rows of
  `the_review_input_seams_methods_read_a_slot_as_a_slot_and_the_checkout_as_the_checkout` are red at `5c222ff2`.
- **Not affected:** the legacy ladder's same check at the started checkout passes in all 114 legacy runs that reach it
  (the proposal, `:231-233`).
- **Not executed:** a whole topology run through the conductor, Windows, other Git implementations and the agent CLIs.

## Its relationship to FUD-D4-ENV

FUD-D4-ENV is the legacy builder's finding ("the legacy Git children inherit the coordinator's Git repository
context"), P1.
- **The same builder and the same class.** This sequence runs through that builder. Its causes, `GIT_DIR` and
  `GIT_WORK_TREE`, are in FUD-D4-ENV's reasoned bullet: "the other repository-context variables … reach every legacy
  child the same way; nothing was executed for them here".
- **Not a sequence FUD-D4-ENV files.** Its filed and executed sequences are all `GIT_INDEX_FILE`-driven, in legacy
  runs. This one has three things no filed text states: D1's selection as its trigger, the schema-4 driver's seam in a
  slot as its path, and a fail-closed refusal of a supported shape's topology run as its consequence.
- **So it is filed here, as its own sequence, at its own proposed grade.** FUD-D4-ENV's guard's change, removing the
  inherited context from `git_command`, would also take it up. FUD-D4-ENV's file records this relationship in its
  implementation-status section.

## Severity: P2 (proposed; the owner classifies)

- **Why not P1.** It fails closed: no data loss in a user repository is shown, and the run is refused before any
  commit.
- **Why not P3.** It refuses the topology runs of supported shapes: D1, and the others among the 6.
- **It is not raised for any branch name.**

## Applicability to G6

No G6 reviewer has assessed it, and this file claims no applicability. Reasoned: it fails closed, and no write outside
the slot is shown. Filing it is not clearance.

## Implementation status (2026-10-05T22:15Z; a conditional draft implementation, not adopted)

**What is implemented.** The reviewed option (C), in the branch `pr11-envc-impl1`'s commits `5c222ff2..6c8ee6b1`
(unpushed). It adds the seam's provided method `problem_in_work_tree`, which opens a slot with
`Workspace::open_work_tree` and the checkout it was made from. The ladder's `problem` at the started checkout is
unchanged.

**The witness:**
- `the_review_input_seams_methods_read_a_slot_as_a_slot_and_the_checkout_as_the_checkout` is red at `5c222ff2` in its
  slot and driver rows, and green with the change;
- `a_legacy_run_under_an_inherited_selection_checks_its_review_input_at_the_selected_repository` is the ladder's
  control, green at both;
- each of the mutations MC13, MC14 and MC15 turns one of them red.

**Why it stays open (`deferred`):** option (C) is not adopted, and this record awaits review and the owner's decision.

## What the change that takes this up should do

Show, failing at its base and passing after it:
- the check in a slot reads the slot;
- the ladder's check at the started checkout still reads the selected repository;
- each index outside the slot is byte-identical.

The witness is D1's layout through `AttemptContext::assess`.

## Provenance

`pre_existing`: at `5c222ff2` the seam opens a slot with `Workspace::open`, whose children inherit the coordinator's
context. `first_bad` was not traced.

## Filed

- **When and by whom:** 2026-10-05, by `pr11_envc_impl1` (`claude-opus-5-5`).
- **On whose direction:** babysit_pr11's direction
  `/home/ubuntu/babysit-pr11/evidence/envc-package3-sources-and-preparation-direction-20261005.md` (sha256
  `4002be0ea860811dbf43b6a8c078ab4536a94e9b8cfc1bfdfc4e4dbb19f599ea`), carried by orch_pr11b's answer
  `/home/ubuntu/orch-pr11/answers/pr11_envc_impl1-3.md`.
- **From what:** package 3's draft D3, after comparing it with FUD-D4-ENV's assigned canonical scope.
- **What filing does not do.** It adopts nothing, and it is not clearance for merge or G6.
