---
id: PR402-MIXED-SCHEMA-LOOKUP-SWALLOWS-READ-ERRORS
severity: P3
disposition: deferred
category: correctness
pr: 402
reviewed_sha: be4176d8991517c641a86ec98cf02f1d70ab5dfc
location: src/answer.rs:156
provenance: pre_existing
first_bad: predates #402: legacy_candidates in src/answer.rs skips every read_dir and entry error; #402 does not change src/answer.rs
guard: row 13's sweep of src/rundir/discovery.rs, or the first change to either walk's body that is more than a rename; both walks in one change
---

## Failure sequence

From #402 round two's frontier pass. `answer::locate` calls `rundir::find_question` only when no
schema-4 run answers and every directory classified (`src/answer.rs:182-184`). When a schema-4
candidate exists, it uses `legacy_candidates` instead. These two lines in that walk drop directory
errors and entry errors:

```rust
let Ok(entries) = std::fs::read_dir(public.join("questions")) else { continue; };
for entry in entries.flatten() {
```

The sequence:

    a schema-4 run holds open question `q-ABC`
    -> a committed legacy run whose `questions` directory cannot be listed holds `q-ABD`
    -> the operator enters prefix `q-AB`
    -> the legacy candidate disappears, `decide` sees one candidate, and the answer is written
       to `q-ABC` although the prefix is ambiguous

From the source, not run: `decide` returns a lone prefix match when `matched == 1` and both
`unclassified` and `topology.unread` are empty (`src/answer.rs:222-228`). The skipped legacy run
adds nothing to `matched`. It is committed, so it is not in `unclassified`. This file has not
checked that it is also absent from `topology.unread`; that comes from the review's sequence.

## Why it is deferred

Ruling r42. `src/answer.rs` is outside #402's diff. The walk is `rundir::find_question`'s:
`docs/internals/answer.md`, under `legacy_candidates`, calls it "`rundir::find_question`'s walk
over the census above". The two finders are held to the same sentences (same document, and the
test `the_composed_refusals_are_the_legacy_finders_sentences_where_only_legacy_runs_answer`), so
they are repaired in one change.

## What the change that takes this up should do

Repair this together with `PR402-QUESTION-SEARCH-SWALLOWS-READ-ERRORS`, in the same change and by
the same rule, which that file's last section states. The test is the sequence above: `q-ABC`
open in a schema-4 run, `q-ABD` in a committed legacy run whose `questions` cannot be listed, and
prefix `q-AB`.
