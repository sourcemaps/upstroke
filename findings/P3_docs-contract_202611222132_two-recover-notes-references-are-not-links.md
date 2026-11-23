---
id: PR402-RECOVER-NOTES-SHORTCUT-REFERENCES-UNLINKED
severity: P3
disposition: accepted-risk
category: docs-contract
pr: 402
reviewed_sha: 61c0f6e48990247f1a13a3f530ede781924a9c25
location: docs/internals/engine/topology/recover.md:518
provenance: introduced_by_feature
first_bad: a6354e4c, #402's commit-digest note
guard: the next change to docs/internals/engine/topology/recover.md
---

## Failure sequence

From #402 round three's review:

    #402's commit-digest note names two functions as rustdoc shortcut references
    -> the note has no Markdown reference definitions for them
    -> GitHub renders each as a bracketed code span, not a link

The lines, at `docs/internals/engine/topology/recover.md:517` to `:519`, added by a6354e4c:

```markdown
never well-formed. The predicate is
[`rundir::is_run_started_sha256`], the recogniser for the one
spelling [`rundir::run_started_sha256`] writes, so the value
```

Both functions are defined in `src/rundir/classify.rs` (`:887` and `:871`) and re-exported from
`src/rundir.rs:2136`.

The same batch's repair of `PR159-ASTRA-002` (abf53644) replaced a rustdoc link in
`docs/internals/status.md`, ``[`RunState::apply`](crate::events::RunState::apply)``, with a
relative Markdown link, `(../../src/events/mod.rs)`.

The note uses the shortcut form elsewhere too, for instance ``[`UpstrokeError::Refused`]`` at `:492`
in the same function's note. `git blame` gives those lines to commits before #402; this finding
covers only the two a6354e4c added.

## Why it is accepted

- Unwitnessed, and P3: it affects only how the note renders.
- The text still names the right functions.
- There is no further run in #402 (ruling r42, step 7).

## What a change that takes this up should do

Write the two names as plain code spans, or as relative links to `src/rundir.rs`
(`../../../../src/rundir.rs` from the note's directory).
