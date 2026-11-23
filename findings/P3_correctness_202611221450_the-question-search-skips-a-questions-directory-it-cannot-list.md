---
id: PR402-QUESTION-SEARCH-SWALLOWS-READ-ERRORS
severity: P3
disposition: deferred
category: correctness
pr: 402
reviewed_sha: 26bd9916d3c67af7b95df21e694994f91ea8a9d5
location: src/rundir/discovery.rs:778
provenance: pre_existing
first_bad: predates #402: the walk skipped every read_dir and entry error before SWEEP-NAMES-004's QUESTIONS_DIR substitution
guard: row 13's sweep of src/rundir/discovery.rs, or the first change to either walk's body that is more than a rename; both walks in one change
---

## Failure sequence

From #402 round one's frontier pass:

    #402's `QUESTIONS_DIR` substitution touches `find_question_observed`
    -> every `read_dir` error, and every entry error, in that walk is read as "no question here"
    -> an existing question can be reported absent, or another prefix reported unambiguous

The two lines that do it:

```rust
let Ok(entries) = fs::read_dir(public.join(QUESTIONS_DIR)) else { continue; };
for entry in entries.flatten() {
```

At the head this file was written against, they are `src/rundir/discovery.rs:781` and `:784`.

The walk covers the runs the census classified as runs (`names_classified(&census, listed_as_run)`).
The refusals' unread note is built from the runs that did not classify
(`names_classified(&census, unclassified)`). A classified run whose `questions` cannot be listed
is in neither list, so no refusal names it. Inferred from the source, not run:

- if the only run holding a question cannot be listed, the question is refused as absent
  (`no question with that id`);
- if that run holds `q-ABD`, another run holds `q-ABC`, and the prefix is `q-AB`, then `q-ABC`
  answers as the lone match.

## Why it is deferred

Ruling r42 on #402. `QUESTIONS_DIR` is `"questions"` (`src/rundir/names.rs:185`), so the
substitution renames the same bytes and activates nothing. The defect is pre-existing, in a file
that has not been swept yet (`standards/SWEEP.md`, row 13, still on the queue).

#402's first repair made the walk return the error at once. That repair also refused a full
question id beside an unrelated run whose `questions` could not be listed, which is a witnessed
P2, `PR402-FULL-ID-REFUSED-BESIDE-AN-UNLISTABLE-RUN`. So the repair was withdrawn. The test
`a_full_question_id_answers_beside_runs_whose_questions_cannot_be_listed`
(`src/rundir/discovery.rs`) now holds the full-id contract.

## What the change that takes this up should do

- Treat only `NotFound` as a run without questions. Any other `read_dir` error, or any entry
  error, leaves that run unread.
- A full id found in a run that was read still answers.
  `a_full_question_id_answers_beside_runs_whose_questions_cannot_be_listed` stays green.
- A prefix or an absent id beside an unread run is refused, and the refusal names the run and the
  path. Both walks use one means for this: either an `Io` error or the refusal's unread note. The
  change states which.
- Tests on both paths. This finding is repaired together with
  `PR402-MIXED-SCHEMA-LOOKUP-SWALLOWS-READ-ERRORS`, because `legacy_candidates` in `src/answer.rs`
  is the same walk.
