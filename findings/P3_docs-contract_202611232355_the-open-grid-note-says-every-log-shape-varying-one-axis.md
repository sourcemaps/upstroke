---
id: PR407-GRID-EVERY-SHAPE-OVERSTATED
severity: P3
disposition: accepted-risk
category: docs-contract
pr: 407
reviewed_sha: b7ce5fd5620c52bd3e31a725ce3b2a0988596188
location: docs/internals/events/log/tests.md:217
provenance: pre_existing
first_bad: predates #407: the sentence is in origin/master's version of docs/internals/events/log/tests.md (line 238 there); #407 kept it while cutting the note
guard: the next change to open_grid or its note, or a later docs PR 4 batch that edits docs/internals/events/log/tests.md
---

## Failure sequence

From #407 round three's review:

    the note under `open_grid` says the grid holds every log shape the open path can meet
    -> `open_grid` has one prefixless tail, the 11-byte ASCII one ("torn tail only, no prefix")
    -> its two-byte split-UTF-8 tail always follows a committed line
    -> the grid's test asserts only aggregate counts
    -> a regression that only a prefixless two-byte split-UTF-8 tail would expose leaves the grid
       and its count assertions green, so "every" overstates what is covered

The note, at `docs/internals/events/log/tests.md:217`:

```text
Every log shape the open path can meet, each varying one axis.
```

The split-UTF-8 tail is built as a committed line, a newline, then two bytes, at
`src/events/log/tests.rs:642`:

```rust
    let mut split_utf8 = good.clone();
    split_utf8.push(b'\n');
    split_utf8.extend_from_slice(&[0xE2, 0x82]);
```

The only case with no committed prefix, at `src/events/log/tests.rs:678`:

```rust
        (
            "torn tail only, no prefix",
            Some(b"{\"ts\":\"2026".to_vec()),
        ),
```

`the_grid_varies_shape_and_tail_length_and_tail_content_independently`
(`src/events/log/tests.rs:700`) asserts thirteen cells, six distinct tail lengths, at least four
distinct committed-line counts and exactly one absent file. None of those counts ties a tail's
content to whether a prefix precedes it.

The regression is the reviewer's reasoning; no mutation was run, in the review or here.

The same sentence is in origin/master's version of the note (line 238 there); #407 kept it while
cutting the note.

## Why it is accepted

- Unwitnessed, and P3: the gap was reasoned, not run; there is no failing test, reproduction or
  mutation witness.
- It is a note's wording and carries no `MUST`.
- The paragraph that follows the sentence (`docs/internals/events/log/tests.md:219`) already says
  "tail length and prefix are not varied independently", so a careful reader is told the limit.
- #407 is a docs batch bounded at round three.

## What a change that takes this up should do

- Narrow the sentence to the shapes the grid selects; the reviewer suggests "the thirteen selected
  shapes".
- Separately, as a test change and not a note: a prefixless split-UTF-8 case would close the gap
  the reviewer describes.
