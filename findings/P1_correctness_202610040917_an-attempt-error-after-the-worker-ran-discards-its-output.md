---
id: PR331-AN-ATTEMPT-ERROR-AFTER-THE-WORKER-RAN-DISCARDS-ITS-OUTPUT
severity: P1
disposition: deferred
category: correctness
pr: 331
reviewed_sha: 88376a108e50362487ef1f374b633f63d71599ee
location: src/engine/coordinator.rs:577
provenance: pre_existing
first_bad: 372f0c02c3b6af3eb19503e9f16872206dd4906e
guard: follow-up D's implementation round 3 (`pr11_fud_impl3`, on draft #331), PROPOSED conditional on the owner's decision O8 (decision B): part N of `~/orch-pr11/owner-package/RD1-PRESERVATION-PROPOSAL.md` (round 4; N as round 2 proposed it, its pin's capture now G4's) keeps the checkout and pins what it holds at the attempt's kept pin on every such error; until that lands and is validated this file stays open, and nothing here accepts the loss
---

## Failure sequence

**First bad:** `372f0c02` (2026-08-09) added the arm `let _ = workspace.discard_uncommitted(); return Err(error);`
for every `run_attempt` error; it predates PR5's freeze of the legacy engine (`7a83e698`, of which it is an ancestor),
and is at `src/engine/coordinator.rs:544-548` at master `5c222ff2` (`~/orch-pr11/logs/pr11_rd1_design2/code/first-bad-search.txt`,
`more-citations-88376a10.txt`).

**Found** by the regular lens of R-D1's round-1 design review (`~/orch-pr11/reviews/review-rd1-regular-e9f78041.review.md`,
item 4; the orchestrator's triage `~/orch-pr11/reviews/review-rd1-e9f78041-triage.md`, N1), on D's head `88376a10`. The Git
sequence was executed; the engine routing is reasoned from code at D's head and at master. Re-executed, with every other
trigger below, by `pr11_rd1_design2`'s standalone harness on Git 2.41.0, 2.43.0, 2.50.1 and 2.55.0
(`~/orch-pr11/logs/pr11_rd1_design2/runs/TABLE-ALL.txt`, cases `N1a` to `N1f` and `RD1-5c`; ordinary Git, not a
conductor run).

1. A legacy attempt's worker writes its output into the run's checkout.
2. `run_attempt` (`src/engine/attempt.rs:113`) returns an error that is not the worktree registry's refusal. After the
   worker has run, every one of these reaches the coordinator's arm (line numbers at D's head; master's in brackets):
   - the runner's error after the worker started (`:139` [`:116`]): a process fate other than "never started", or an
     I/O failure collecting its output;
   - the transcript or stderr log write (`:143`, `:148` [`:120`, `:125`]): a full or unwritable private root;
   - the adapter's parse of the worker's output (`:151` [`:128`]);
   - `capture_candidate` (`:152` [`:129`]): `HEAD` detached, symbolic or moved while capturing (another Git client
     advancing the run branch with `update-ref`, the reviewer's witness); a clean or smudge filter on a worktree path
     (a worker that adds a `.gitattributes` naming `filter=lfs`); `git add -A` failing (an unreadable file, a nested
     repository with no commit, another process holding `index.lock`); `write-tree` failing; or the review diff that is
     not valid UTF-8 (a worker that writes a text file in Latin-1, or a text write interrupted mid-character), which
     `Workspace::git` refuses (`src/workspace.rs:165-173`; the frozen `non_utf8_text_diff_is_refused_before_review`
     pins that refusal);
   - the review-input policy's Git reads (`:171` [`:148`]);
   - a gate or review snapshot that fails for a reason that is not the registry's (`:191`, `:225` [`:158`, `:182`]):
     a store that cannot be made, a `worktree add` that fails ("unable to read tree"), a verification that fails —
     option (C)'s R-C-RELSTORE loss is this shape (`~/orch-pr11/owner-package/O3R-OPTION-C-PROPOSAL.md` §8.2, executed
     in `~/orch-pr11/reviews/envr-c-witnesses/round4/envrc4-review-8ncx6d7a/relstore-discard-2/`);
   - the gates' or the review passes' own runner errors (`:200`, `:249` [`:167`, `:206`]).

   Before the worker runs (`:132`, `:139` when its process never started), the checkout holds output only when an
   earlier attempt's was kept for a same-session retry (`Next::RetrySameRung { resume: true }`,
   `src/engine/coordinator.rs:803-805`).
3. The arm (`src/engine/coordinator.rs:576-578`; master `:544-548`) runs `discard_uncommitted()` — `git reset --hard
   HEAD` and `git clean -qfd` (`src/workspace.rs:1264-1269`) — and returns the error. Nothing pins the output first.
4. The worker's output is gone: no ref names it, and the next resume runs the attempt again, paying for it again. When
   the trigger was a branch moved during the capture, that resume also refuses on the moved branch until the operator
   moves it back.

**Executed** (harness, four Gits, every verdict identical across them): master and D's head lose the output in `N1a` (the
branch moved during the capture), `N1ah` (the same, put back with `reset --hard`), `N1b` and `N1b2` (an unreadable file
fails the capture's add), `N1c` (any error after a successful capture), `N1d` (an error after the worker wrote, before the
capture), `N1e` (an earlier attempt's retained output, and an error before the new worker ran) and `RD1-5c` (an
undecodable diff). `N1f` is the control: an error before any output, nothing to keep.

**Re-executed** by `pr11_rd1_round3`'s harness on the same four Gits (`~/orch-pr11/logs/pr11_rd1_round3/runs/TABLE-ALL.txt`, cases `N1a` to `N1f` and `RD1-5c`): master's and D's head's verdicts are round 2's, row for row (`~/orch-pr11/logs/pr11_rd1_round3/runs/ROUND2-MODELS-UNCHANGED.txt`); ordinary Git, not a conductor run.

## Severity: P1

A loss of paid worker output in the user's repository (`MAINTAINING.md:176-190`). The preconditions are ordinary: an
attempt error after the worker ran (a full disk, a runner fault), a worker that writes Latin-1 text or adds an LFS
attribute, or another Git client advancing the run branch.

## G6

**Its non-applicability to G6 is a claim, not an established classification.** The executed triggers are legacy-only,
and G6 claims nothing of the legacy engine frozen at PR5 (the legacy finding's G6 section, (e1)); that is the claim. It
needs classifying on the final range -- the commits that land it, after their review -- and two faces need classifying
there, not here:
- **The Q6 / topology-writer face.** A topology writer (a schema-4 run sharing the worktree registry: the legacy
  finding's (e2) and (e2′) channel) may make a legacy attempt fail with an error the tolerant registry access does not
  classify as the registry's -- a snapshot `worktree add` that fails for another reason, a verification that fails --
  which then reaches this arm (`src/engine/coordinator.rs:576-577`). If one can, this finding applies through Q6 as (e2)
  does. Not established here.
- **The R-C-RELSTORE face.** Option (C)'s inherited defect (`~/orch-pr11/owner-package/O3R-OPTION-C-PROPOSAL.md`
  §8.2, sha256 `dc94ba82…`): a relative repository-wide name makes a gated run's snapshot fail closed, and this arm
  then discards the output (executed in `~/orch-pr11/reviews/envr-c-witnesses/round4/envrc4-review-8ncx6d7a/
  relstore-discard-2/`). That face keeps option (C)'s own applicability, conditional on its precondition, and is
  classified with it.

There is no G6 waiver. Grading and filing clear no G6 predicate.

## What the change that takes this up should do

Keep the checkout and pin what it holds on every such error, as D's arm already does for the registry's refusal:
`Workspace::pin_checkout`, a capture into a private index file of the checkout's own Git directory (no shared index, no
review diff, the checkout's own configuration; what stands where HEAD's deleted paths go added with `-f`) pinned at the
attempt's `prepared_pin_ref` followed by `-kept` whatever `HEAD` is (part K), with the error returned unchanged and a
warning naming the pin or the pin's failure; nothing discarded. The resume's guarded discard (part G4) then removes the
checkout's copy only once the pin and a durable copy of it hold it. Tests and mutations: the proposal's §10 (`T-N1a` to
`T-N1e`, and the mutations `n1-discard` and `n1-nopin`).

## At #331's implementation round 3 (2026-10-04): filed, and implemented as a draft

Filed from its draft (`~/orch-pr11/logs/pr11_rd1_round4/drafts/`), with its severity, review pin (`reviewed_sha`, D's
head), provenance and first-bad commit as the draft gives them; only the guard's first words name this round.

**Implemented on draft #331, PROPOSED conditional on the owner's decision O8**, as part N of R-D1's preservation
design, round 4 (sha256 `f9e81c07…`), with the coordinator's `kept_on_error` and `Workspace::pin_checkout`
(`reviews/2026-10-02-pr11-follow-up-d-record.md` §5.17). Its engine witnesses, each red at D's head as the merge has it
and green here:
  `an_attempt_error_that_is_not_a_registry_refusal_keeps_the_output_pinned`,
  `a_snapshot_failure_that_is_not_the_registrys_keeps_the_output_pinned`,
  `an_undecodable_diff_keeps_the_output_pinned`,
  `a_branch_moved_during_the_capture_keeps_the_output_pinned`,
  `an_error_after_the_worker_wrote_and_before_its_capture_keeps_the_output_pinned`,
  `an_earlier_attempts_retained_output_is_kept_when_the_next_worker_cannot_start`,
  `a_capture_leaves_a_split_index_readable_and_pins`,
  `an_inherited_split_index_variable_expires_nothing`;
red under `n1-discard` and `n1-nopin`. **This file stays open** until that pull request merges — after O8's adoption, PR #329, and the
required reviews — and is deleted then, with a `fixed` ledger row naming these tests.

**G6: still a claim.** Its non-applicability to G6 is not established here: it needs classifying on the final range,
the Q6 / topology-writer face and the R-C-RELSTORE face included, as the section above says. There is no waiver, and
filing clears no G6 predicate.

