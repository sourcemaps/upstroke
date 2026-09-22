---
id: PR311-SHARED-PLANTS-WRITE-A-CANDIDATE-PIN-NO-CRASH-LEAVES
severity: P3
disposition: deferred
category: correctness
pr: 311
reviewed_sha: 4231db052fc20b3e7f758d12336c6bf0c3aa5d19
location: src/engine/topology/recover/tests.rs:6758
provenance: pre_existing
first_bad:
guard: the change that next takes up the shared candidate plants of `src/engine/topology/recover/tests.rs` (`plant_queued_candidate_events`, `plant_published_beta_editing`, `plant_queued_beta`, `plant_stale_verification_of_a_test_candidate`); until then a witness credited over one of them completes it with `prune_the_planted_candidate_pins` and holds its recovery to `assert_no_repair_of_the_creation_prefix`
---

## Failure sequence

Four planting helpers of `engine::topology::recover::tests` write a candidate's
`candidate-prepared` pin with `git update-ref` beside a durable `task_candidate_created` and leave
it standing with no generation worktree: `plant_queued_candidate_events` (alpha, and through it
`plant_queued_candidate`, `publish_alpha`, `plant_stale_verification`, `plant_rejected_repair` and
their callers), `plant_published_beta_editing`, `plant_queued_beta` and
`plant_stale_verification_of_a_test_candidate`. One test,
`two_lineages_publish_in_lineage_order_and_the_younger_candidate_waits_behind_the_older`, plants
the same inline.

Production leaves no such state. `TopologyRun::promote_candidate` returns only after
`reclaim_after_creation` has deleted the pin, and it deletes the pin before it removes the
generation's worktree. Nor is the pin inert: `candidate::recovery_for` reads a standing pin as an
unfinished promotion and `recover::finish_promotions` deletes it (`Ref.DeleteCandidatePin`). So
the first resume over one of these plants repairs damage no crash left.

Where that resume is a witness's **credited** recovery, the witness is inexact under ST-07's first
obligation. That is the defect PR #311's two review rounds found and the pull request repaired
where it bit: round 1 in the five witnesses of rows 15, 18, 33, 34, 57, 98, 122 and 133, which now
prune the pins their plants leave; round 2 in the finalization kill matrix (19 of the 26 cells
Gate 5's audit credits to it), whose planting now completes the promotion; round 3 moved the pin
the matrices then planted as declared damage at their two `Ref.DeleteCandidatePin` cells out of
both, into two tests nothing cites, and made the cited test's two cells of that site the
promotion kills, so no credited test plants the pin at all. The helper bodies are
identical on `a738abcb`, the pull request's base (the round-2 regression lens's hash comparison).

**What remains.** The helpers still write the pin. Executed on this box at `ae673768`, the
pull request's round-2 code, one process per test over the 570 tests of `engine::topology` with an
observation-only probe at the two ref funnels (`~/orch-pr10/clause2-evidence-r2/census/`):

- 71 tests have `reclaim_after_creation` delete a pin no `pin_candidate` made. Twelve of them are
  credited witnesses: nine that Gate 5's audit selected and three that #305 added. None of the
  twelve deletes it in its credited recovery. Rows 1, 2, 112, 126, 127, 159 and 160, rows 43 and
  58, and #305's three other staging-path kills lose it to a resume before their credited crash
  (boundary probes, all green: zero candidate-prepared pins at the boundary, and zero deletions in
  the credited recovery where that was counted); rows 9 and 10 share a witness the audit graded
  `none` on the first obligation for another reason. The other 59 are behaviour tests with no
  exactness claimed of them.
- The round-2 regression lens's direct probes still read what they read: a resume straight over
  `plant_stale_verification`, `plant_rejected_repair`, `plant_queued_beta` and
  `plant_stale_verification_of_a_test_candidate` performs 2, 2, 1 and 2 pin deletions, and zero
  over the same plants pruned.

So nothing credited is inexact for this today. The trap is what a future witness inherits: one
credited over any of these plants, without the prune, passes while its recovery repairs a pin its
coordinate's crash never left, and nothing fails it unless it also calls
`assert_no_repair_of_the_creation_prefix`.

## What the change that takes this up should do

Stop writing the pin in the four helpers and the one inline plant, so that a planted candidate is
what a completed promotion leaves: its candidates ref and no `candidate-prepared` ref. In the same
change remove the five round-1 witnesses' `prune_the_planted_candidate_pins` calls (the helper
fails by name when a plant left nothing to prune, which is why this cannot be done under them),
keep `assert_the_creation_prefix_is_complete` and `assert_no_repair_of_the_creation_prefix`, and
re-run the two mutation rows that pin them (PR #311's C4 and C5). `plant_finished_run_with` then
loses its own prune. The tests that need a pin to exist keep getting one by name: the orphan and
promotion witnesses through `pin_candidate`, and the finalization sweep's tests through
`with_a_candidate_pin_no_crash_leaves`.

It was not done in PR #311's second repair round because that round was scoped to the credited
prefixes and told to leave the 28 established witnesses as two reviews graded them; the round-2
regression lens asked for the remainder to be recorded `deferred` until resolved.
