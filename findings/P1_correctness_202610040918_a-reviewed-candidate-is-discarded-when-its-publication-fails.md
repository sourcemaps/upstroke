---
id: PR331-A-REVIEWED-CANDIDATE-IS-DISCARDED-WHEN-ITS-PUBLICATION-FAILS
severity: P1
disposition: deferred
category: correctness
pr: 331
reviewed_sha: 88376a108e50362487ef1f374b633f63d71599ee
location: src/engine/coordinator.rs:707
provenance: pre_existing
first_bad: b441180e8a4f86952902c145127e80754a88c68c
guard: follow-up D's implementation round 3 (`pr11_fud_impl3`, on draft #331), PROPOSED conditional on the owner's decision O8 (decision B): part N of `~/orch-pr11/owner-package/RD1-PRESERVATION-PROPOSAL.md` (round 4; N as round 2 proposed it) pins the reviewed candidate at the attempt's kept pin and keeps the checkout, whatever `HEAD` is, while the publication guard still refuses; until that lands and is validated this file stays open, and nothing here accepts the loss
---

## Failure sequence

**First bad:** `b441180e` (2026-08-14) added `let _ = self.workspace.discard_uncommitted(); return Err(error);` when
the prepared commit cannot be made (`:707`); `c42dbbb3` (2026-08-14) added the same before refusing a candidate captured
on another branch (`:690`), and `acb67367` (2026-08-14) discards when the settlement cannot be appended (`:738`). All
three predate PR5's freeze of the legacy engine (`7a83e698`, of which each is an ancestor), and are at
`src/engine/coordinator.rs:677`, `:660` and `:708` at master `5c222ff2` (`~/orch-pr11/logs/pr11_rd1_design2/code/first-bad-search.txt`,
`more-citations-88376a10.txt`).

**Found** by the regular lens of R-D1's round-1 design review (`~/orch-pr11/reviews/review-rd1-regular-e9f78041.review.md`,
item 5; the orchestrator's triage `~/orch-pr11/reviews/review-rd1-e9f78041-triage.md`, N2), on D's head `88376a10`. The Git
sequence was executed; the engine routing is reasoned from code at D's head and at master. Re-executed, with the two
sibling sites, by `pr11_rd1_design2`'s standalone harness on Git 2.41.0, 2.43.0, 2.50.1 and 2.55.0
(`~/orch-pr11/logs/pr11_rd1_design2/runs/TABLE-ALL.txt`, cases `N2a` to `N2d`; ordinary Git, not a conductor run).

1. A legacy attempt's candidate is captured, passes its gates, and its review passes approve it.
2. Its publication then fails, and each failure discards the candidate (line numbers at D's head; master's in
   brackets):
   - **`:707` [`:677`]:** `prepare_commit_from_candidate` refuses the prepared pin. Another Git client advanced the run
     branch after the capture (`update-ref`, the checkout untouched: "HEAD moved from captured branch … refusing to
     prepare it", the reviewer's witness), or HEAD is detached or on another branch, or the store, the name or a lock
     refuses the pin (a ref under `refs/upstroke/prepared/<run>/<task>-<attempt>`, an unwritable `.git/refs`);
   - **`:690` [`:660`]:** the candidate was captured on another branch (the worker switched branches), and the
     publication is refused;
   - **`:738` [`:708`]:** the prepared pin is written, but the settlement event (`AttemptFinished`) cannot be appended.
     The checkout is discarded, and the next resume removes the attempt's prepared pin as an orphan
     (`src/engine/resume.rs:543-560`), the candidate's last ref.
3. `discard_uncommitted()` (`git reset --hard HEAD`, `git clean -qfd`) runs before the error is returned. Nothing pins
   the candidate first; the publication guard that refused it is right to refuse it, and wrong to destroy it.
4. The reviewed output is gone, and the resume runs the task again, paying again for the worker, the gates and every
   review pass.

**Executed** (harness, four Gits, identical): master and D's head lose the candidate in `N2a` (the branch advanced
after review), `N2ah` (the same, put back with `reset --hard`), `N2b` (a ref under the prepared pin's name), `N2c` (the
candidate captured on another branch) and `N2d` (the settlement append fails, and the resume removes the orphan pin).

**Re-executed** by `pr11_rd1_round3`'s harness on the same four Gits (`~/orch-pr11/logs/pr11_rd1_round3/runs/TABLE-ALL.txt`, cases `N2a` to `N2d`): master's and D's head's verdicts are round 2's, row for row (`~/orch-pr11/logs/pr11_rd1_round3/runs/ROUND2-MODELS-UNCHANGED.txt`); ordinary Git, not a conductor run.

## Severity: P1

A loss of paid, reviewed output in the user's repository (`MAINTAINING.md:176-190`), with an ordinary precondition:
another Git client advancing the run branch, or one failed append.

## G6

**Its non-applicability to G6 is a claim, not an established classification.** Its executed triggers are legacy-only,
and G6 claims nothing of the legacy engine frozen at PR5 (the legacy finding's G6 section, (e1)); that is the claim. It
needs classifying on the final range -- the commits that land it, after their review -- including:
- **the Q6 / topology-writer face:** whether a topology writer sharing the worktree registry can make a reviewed legacy
  candidate's publication fail at `:690`, `:707` or `:738` (a prepared pin the store or a lock refuses, a branch moved
  under it), so that this finding applies through Q6 as the legacy finding's (e2) does. Not established here;
- **the R-C-RELSTORE face:** none is known on these three arms -- option (C)'s §8.2 loss reaches the error arm of
  `PR331-AN-ATTEMPT-ERROR-AFTER-THE-WORKER-RAN-DISCARDS-ITS-OUTPUT`, not a publication -- and the final range confirms
  that or classifies one.

There is no G6 waiver. Grading and filing clear no G6 predicate.

## What the change that takes this up should do

Keep the publication guard and keep the candidate: on each of the three failures, pin the captured candidate (its
branch, parent and tree, never the live index) at the attempt's `prepared_pin_ref` followed by `-kept` through
`prepare_commit_from_candidate`, whatever `HEAD` is (part K), keep the checkout, and return the error unchanged with a
warning naming the pin or its failure. The resume's guarded discard (part G4) removes the checkout's copy only once the
pin and a durable copy of it hold it, after the resume's own moved-branch refusal, which names `git update-ref`, has been
answered. Tests and mutations: the proposal's §10 (`T-N2a` to `T-N2c`, and the mutations `n2-discard`, and `k-headcheck`,
which `N2ah` kills).

## At #331's implementation round 3 (2026-10-04): filed, and implemented as a draft

Filed from its draft (`~/orch-pr11/logs/pr11_rd1_round4/drafts/`), with its severity, review pin (`reviewed_sha`, D's
head), provenance and first-bad commit as the draft gives them; only the guard's first words name this round.

**Implemented on draft #331, PROPOSED conditional on the owner's decision O8**, as part N of R-D1's preservation
design, round 4 (sha256 `f9e81c07…`), with the coordinator's `kept_on_error` and `Workspace::pin_checkout`
(`reviews/2026-10-02-pr11-follow-up-d-record.md` §5.17). Its engine witnesses, each red at D's head as the merge has it
and green here:
  `a_reviewed_candidate_refused_on_a_moved_head_is_kept_and_pinned`,
  `a_candidate_captured_on_another_branch_is_kept_and_pinned`,
  `a_reviewed_candidate_whose_settlement_is_not_appended_is_kept`;
red under `n2-discard`, and `k-headcheck` for the `reset --hard` form. **This file stays open** until that pull request merges — after O8's adoption, PR #329, and the
required reviews — and is deleted then, with a `fixed` ledger row naming these tests.

**G6: still a claim.** Its non-applicability to G6 is not established here: it needs classifying on the final range,
the Q6 / topology-writer face and the R-C-RELSTORE face included, as the section above says. There is no waiver, and
filing clears no G6 predicate.

