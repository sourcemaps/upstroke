---
id: PR325-THE-SECOND-READING-ARGUMENT-CONTRADICTS-THE-FALLBACK
severity: P3
disposition: deferred
category: docs-contract
pr: 325
reviewed_sha: 141fae347fb196548defcfc4d8ed3116b441edc9
location: docs/internals/effects/tests/source_oracles.md:1293
provenance: introduced_by_feature
first_bad: 45c81ab0f66d49f8f2d1fe78adae82c93921c8ab
guard: project owner — the next change to `compare_above_the_cut` or its notes; filed by #325's filing pass, not repaired
---

## Failure sequence

The notes on `compare_above_the_cut` (`docs/internals/effects/tests/source_oracles.md`, "Why the second
reading cannot excuse what the comparison is for"), and #325's round-7 body with them, argue that the
second reading -- the text above the cut read again with the closers of its open groups appended after
the gate at the cut -- cannot make the comparison excuse production code. The premise:

> an element the text above the cut ends, it ends at the same place whatever follows the gate at the
> cut: every end it finds before the cut, it finds by looking no further than that gate.

The same section, two paragraphs earlier, describes the fallback the seventh round's repair is for:

1. A test-only `if` whose first block closes above the cut, with the cut in its `else` block, is one
   `if_end` cannot end from the text above the cut.
2. `configured_item_end` falls back to ending the element after its first block -- an end found
   *before* the cut -- so the first reading removes the `if` and that block and keeps `else {`.
3. Read with the closers after the gate, the chain is finished and the reader ends the element past
   the cut, removing `else {` with the rest.
4. So an end found before the cut depends on what follows the gate: the premise is false of the
   fallback. And the conclusion's own definition of settled production -- a byte the text places after
   the end of every test-only element starting before it -- takes in `else {`, which the first reading
   places after the fallback's end and the second removes. Taken as written, the argument says the
   cut-short test cannot fire where it does.

Measured by #325's filing pass at `141fae34`: W2 of `PR325-A-NESTED-GATE-READS-AS-CUT-SHORT`, a
test-only `if` cut short in its `else` block, is accepted with the comparison stopping at the `if`'s
gate although the element's first byte is gone from the first reading -- so it stopped because the
second reading removed what the first kept after the fallback's end. The committed rows of
`the_whole_region_contains_the_truncated_one` whose cut is in an `else` block, an `else if`'s block, or
a call or a macro's arguments in an `else if`'s condition pass the same way.

## Reachability

- **Prose only.** No reader or test reads the argument; the comparison does what the code says, and
  #325's filing pass changes neither.
- **What rests on it** is the notes' and the body's statement that the second reading cannot excuse
  code the text above the cut settles as production. Round 7 measured that property on the inputs it
  named -- the committed rows, the tree and the registry: wherever production code stands between a
  gate and the cut, none of 56 continuations appended after the gate removes everything to the cut --
  so on those inputs it holds; beyond them, nothing written establishes it.
- The notes now say the premise is false of the fallback and point here; the argument itself is not
  restated.

## What the change that takes this up should do

Restate the argument on a premise every path of `configured_item_end` satisfies, the `if` fallback
included -- the fallback's end is the one this section shows depending on what follows the gate, so
the premise has to except it, and the bytes it leaves have to be shown to lie inside an element the
text above the cut does not end -- and check the premise against each return of
`configured_item_end`. Not repaired in #325's filing pass, which files the reviews of `141fae34`.

## Provenance

The argument was written in #325's seventh round (`45c81ab0`, notes for `3c135f42`). Found by the
fix-check review of `141fae34`, `claude-opus-5-5` at `max`
(https://github.com/sourcemaps/upstroke/pull/325#issuecomment-5879449999).
