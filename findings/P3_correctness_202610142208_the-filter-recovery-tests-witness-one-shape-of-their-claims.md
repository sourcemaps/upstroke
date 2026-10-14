---
id: FUD-F1R-UNWITNESSEDARMS
severity: P3
disposition: deferred
category: correctness
pr: 331
reviewed_sha: dd13d447065c83b3926d750b2d8738462fc387e8
location: src/engine/resume.rs:566
provenance: introduced_by_feature
first_bad: 015131b0599df9d64d30f02a08120b25bfa76acb
guard: an owner-directed follow-up after #331: by the owner's direction for #331 of 2026-10-14 (babysit's `d-owner-final-docs-findings-merge-direction-20261014.json`, sha256 `1128d021…`) this is filed open and deferred, with no new test or source fix in #331; a change to follow-up D's tests and notes after #331, owned by the PR11 orchestrator (`orch_pr11b`) for the owner, adds the witnesses below, so that the two mutations named here turn a test red, and deletes this file
---

# The filter-recovery tests witness one shape of their claims

F1R-3 of the owner-session regression review of #331's F1 delta (`1263f2a3..dd13d447`), P3 there and here; its
evidence level: reasoned, with one executed Git check (item 4, Git 2.43.0); no engine run, no test run and no mutation
run. Line numbers are at `dd13d447`.

## Failure sequence

The F1 round's four tests, `a_filtered_output_the_pin_cannot_capture_is_kept_and_its_warning_says_how_to_resume`
(`src/engine/tests.rs:16103`), `a_resume_over_a_filtered_output_refuses_saying_how_and_pins_it_once_the_attribute_is_gone`
(`:16144`), `no_engine_git_child_runs_the_filter_a_worker_attributes_its_output_to` (`:16230`) and
`a_tracked_filtered_path_moved_out_stays_refused_and_is_pinned_once_its_attribute_is_gone` (`:16350`), each witness one
shape of what the F1 round claims. Four shapes have no witness:

1. **The arms that return the listing's error unchanged.** With no attempt in flight, or two or more, the resume returns
   the listing's own error (`src/engine/resume.rs:566-571`), and the notes say "the listing's error returns as before"
   (`docs/internals/engine/resume.md:274-275`). A change of `let [attempt] =` to `let [attempt, ..] =` at
   `src/engine/resume.rs:566` -> a resume with two attempts in flight gets the one-attempt refusal -> no test fails
   (reasoned).
2. **The reviewed-candidate arms keep the warning they had.** `kept_on_error` appends the advice only when it has no
   candidate (`src/engine/coordinator.rs:292-298`), and the notes say N2's arms keep their warning
   (`docs/internals/engine/coordinator.md:245-247`). Appending the advice for `candidate.is_some()` as well -> every
   N1 and N2 test still passes, since they match the warning with `contains` -> no test fails (reasoned).
3. **A listing failure that is not a filter's.** With exactly one attempt in flight the conversion turns every failure
   of `uncommitted_summary` into the resume's refusal, filter or not; no test pins what that refusal says for another
   cause. The weakest of the four, as the review says: an unreadable index already fails earlier, at
   `refuse_sparse_checkout`'s `ls-files -t` (`src/workspace.rs:398`).
4. **A tracked attribute file.** Both resume tests use an untracked, worker-written `.gitattributes` and clear the
   refusal with `fs::remove_file` (`src/engine/tests.rs:16184`, `:16418`). A tracked `.gitattributes` deleted from the
   checkout still sets the filter for `git check-attr`, which falls back to the index's copy (executed by the reviewer
   on Git 2.43.0), so the guard still refuses -> the notes' "the attribute file copied out and removed"
   (`docs/internals/engine/tests.md:3256` for the second test, `:3274` for the fourth) holds only for an untracked
   attribute file. The advice's own words, "remove the attribute that sets it", stay correct. And the one check that
   the withdrawn move-out advice stays withdrawn is `!text.contains("move that path out")` (`src/engine/tests.rs:16466`):
   a rewording such as "move the path out" passes it.

No functional defect is shown: the review found no functional regression in the F1 delta, and these are gaps in what
its tests and one notes sentence witness.

## Evidence and its limits

Found by the owner-session regression review of the F1 delta of #331 (a reviewer the owner session reports as Fable
5.1 at `max`, static and read-only; CHANGES_REQUIRED on three P3 items, no P0, P1 or P2), read here only from its neutral
copy `/home/ubuntu/babysit-pr11/evidence/d-owner-f1-delta-regression-report-20261014.md` (sha256 `5cbece51…`), with
babysit's intake `/home/ubuntu/babysit-pr11/evidence/d-owner-f1-delta-regression-intake-20261014.json` (sha256
`d1cadbda…`). That review is the evidence; this file re-reads the cited lines at `dd13d447` and runs nothing. Its
limits hold here too: items 1 to 3 are reasoned, and the two mutations were not run; item 4's Git check ran on Git
2.43.0 only, not on 2.55.0; Windows, macOS and a real Git LFS installation were not exercised; and the engine was not
run.

## Category and provenance

- **Category `correctness`:** two mutations of the shipped behaviour (items 1 and 2) survive every test, and one notes
  sentence claims a recovery for every attribute file that holds only for an untracked one. The closed vocabulary has
  no test-coverage category; this ledger files an unwitnessed shape as `correctness`, as
  `findings/P3_correctness_202609040706_tests-diff-flags-ext-and-textconv-unwitnessed.md` does.
- **Provenance `introduced_by_feature`:** the conversion (`src/engine/resume.rs:564-585`), the advice
  (`src/engine/coordinator.rs:292-298`), the first three tests and the second test's notes are the F1 round's first
  commit, `015131b0`; the fourth test, its notes and the withdrawn-advice check are DF1R-1's, `dd13d447`. Master has none
  of them.

## What the change that takes this up should do

- A hand-built event log with no attempt in flight and one with two, each with a listing that fails, asserting the
  listing's own error comes back unchanged; this kills the `let [attempt, ..] =` mutation.
- An N2 arm whose publication fails, asserting its warning carries no advice; this kills the appended-advice mutation.
- Optionally, a listing failure of another kind, if one can be produced past `refuse_sparse_checkout`.
- A sentence in `docs/internals/engine/tests.md` saying the removal shown is for an untracked attribute file and that
  a tracked one is edited, not deleted; and a withdrawn-advice check that refuses any move-out alternative, not one
  spelling of it.

## Disposition: open, by the owner's direction

The owner's direction for #331 of 2026-10-14
(`/home/ubuntu/babysit-pr11/evidence/d-owner-final-docs-findings-merge-direction-20261014.json`, sha256 `1128d021…`) says,
in the owner's words: "Fix F1R-1 and F1R-2 (docs only), file F1R-3 and the ignored-path capture gap under findings/, run
the baseline, get CI green and merge #331." Its disposition of F1R-3: an open deferred follow-up, with no new tests or
source fix in that round. This file is that record. It is not fixed, not an accepted risk and not invalid, and its
filing asks no further permission of anyone. It stays open until a change to follow-up D's tests and notes after #331,
owned by the PR11 orchestrator (`orch_pr11b`) for the owner, adds the witnesses above and deletes this file with a
`fixed` ledger row.
