---
id: PR325-A-NESTED-GATE-READS-AS-CUT-SHORT
severity: P2
disposition: deferred
category: correctness
pr: 325
reviewed_sha: 141fae347fb196548defcfc4d8ed3116b441edc9
location: src/effects/tests/source_oracles.rs:2127
provenance: fix_regression
first_bad: 3c135f42db47560f072bf5a2d135b9c7240be2c1
guard: project owner — a later change to `compare_above_the_cut`; filed by #325's filing pass on the owner's ruling that there is no eighth round without a P1, not repaired
---

## Failure sequence

`effects::tests::the_production_code_region_contains_the_truncated_one` checks `production_code`
through `compare_above_the_cut` (`src/effects/tests/source_oracles.rs`): above a file's first
`#[cfg(test)]`, the cut, the whole file's reading must agree with the reading of the text above the
cut, and the comparison stops at the first gate whose element crosses the cut. Since #325's seventh
round (`3c135f42`) one test for such a gate is *cut short*: the reading of the text above the cut
keeps something between the gate and the cut, and a second reading, with the closers of the groups
left open at the cut appended after the gate there, removes all of it. That test is decided over the
**span** from the gate to the cut, not over the gate's own element, and the second reading removes
the span when it removes an element enclosing the gate as much as when it removes the gate's own:

1. A test-only element crosses the cut, and above the cut, inside it, stands a second test-only gate
   whose element the text above the cut ends:
   `#[cfg(all(unix, test))] pub fn helper() { #[cfg(all(windows, test))] let inner = 0; let kept = 0;`
   followed by the file's first `#[cfg(test)]`.
2. The reading of the text above the cut keeps `pub fn helper() {` (left open), removes
   `let inner = 0;` and keeps `let kept = 0;`. The second reading, with `}` appended, removes the
   whole function.
3. So the nested gate counts as cut short: the first reading keeps `let kept = 0;` after it, and the
   second removes everything from it to the cut -- because it removed `helper`, not because the `let`
   reaches the cut.
4. A whole reading that keeps `pub fn helper() {` and removes everything from the nested gate to the
   cut -- which runs `let inner = 0;` on past the cut, and equally removes `let kept = 0;` from a
   function it keeps -- makes the nested gate the one the comparison stops at. The readings are
   compared only above it, where they agree, and the comparison accepts the reading.
5. Round 6's rule, at `be993da5`, refused it: the nested `let` is not left open, so nothing crossed,
   the comparison ran to the cut, and the whole reading lacks `let kept = 0;` there.

Measured by #325's filing pass, on rustc 1.97.1 and 1.85.0, with a probe in `git archive` exports of
`141fae34` and `be993da5` that hands each commit's `compare_above_the_cut` a whole reading forged
from the reading of the text above the cut by blanking everything from the nested gate to the cut,
and reads the honest file through `read_above_the_cut`:

| shape, then the cut | `141fae34`: honest / forged | `be993da5`: honest / forged |
|---|---|---|
| W1: the function above | accepted, stops at `helper`'s gate / **accepted**, stops at the nested gate | accepted / **refused** |
| W2: `let inner` and `let kept` in the `else` block of a test-only `if` cut short there | accepted / **accepted**, at the nested gate | refused (the round-7 shape) / **refused** |
| W3: `#[cfg(all(windows, test))] fn inner() {} fn kept() {}` in a test-only `mod` left open | accepted / **accepted**, at the nested gate | accepted / **refused** |
| round 6's and 7's four committed misreading rows, forged as the committed test forges them | refused, each | refused, each |

Rows two and four of that test's misreadings remove a statement from an element the whole reading
keeps. Their own form on W1's and W2's text -- `let kept` removed and the rest as the text above the
cut reads it -- is the same reading as the forged one, since that reading already removes the nested
`let`, and it is accepted at `141fae34` and refused at `be993da5` the same way. The standard those
rows state, that an inconsistent reading of a kept element is refused, is not met once a test-only
gate stands in the element above the code removed.

## Two readings of the grade

Both reviews of `141fae34` found this independently and graded it differently. It is filed at the
more conservative grade; a later reviewer may reclassify.

- **P3.** What it lets through is only the removal of test code: the bytes it excuses lie inside the
  enclosing test-only element, so the comparison's stated purpose -- no production code hidden --
  still holds, and what fails is the sentences in the notes that said these readings are refused.
- **P2.** It is a regression against round 6, reachable by ordinary compilable Rust, and it
  contradicts the comparison's stated contract. What it costs is the oracle's power to catch a future
  reader defect in an element nested inside one that crosses the cut -- a defect that could then run
  past the cut, where only the test's end-of-file sentinel check remains.

## Reachability, and why it does not block the merge

- **It cannot refuse honest code.** The cut-short test only adds gates the comparison may stop at, and
  stopping earlier compares a shorter prefix; `compare_above_the_cut`'s other two assertions do not
  read where it stops. Measured: `read_above_the_cut` at `141fae34` refuses no file of `141fae34`'s tree (192
  files), of #325's base `daa09b1e`'s tree (189) or of the build box's cargo registry (11,041), on
  both toolchains.
- **It hides no production code.** The bytes it excuses lie between the nested gate and the cut, which
  the second reading removes only by removing the enclosing test-only element; an honest whole reading
  removes that element too.
- **No file of either tree, nor of the registry, reaches it.** In the same three corpora no gate above
  a cut is left open and none counts as cut short (0 and 0, over 11, 11 and 166 gates above a cut), so
  no whole reading of any of those files, however forged, is compared short of its cut.
- **Its harm needs a future defect in the reader itself**: an edit to `src/effects.rs`, which needs push
  access.

That this P2 does not block the merge is the orchestrator's judgement, recorded in #325's body, on the
four points above.

## Not gap (a), (b) or (c)

The notes on `compare_above_the_cut` (`docs/internals/effects/tests/source_oracles.md`) state it as
gap (d), beside three it is none of. **(a)** is a tokeniser that reads a group as open at the cut when
it is not; here the text is tokenised correctly. **(b)** is a whole reading that removes code the text
above the cut leaves unsettled, inside an element that text does not end; here the misread element is
one that text *ends* -- the case the notes called "compared in full" -- and the bytes excused are ones
an honest whole reading removes as well. **(c)**, `PR325-A-SETTLED-KEPT-ELEMENT-READS-AS-LEFT-OPEN`, is
an element the reader keeps for a settled reason counted as left open; here the nested element is
removed, and the test that excuses it is cut short.

## The open P1 row

A review of `141fae34` observes that this class falls within what `PR7-WRAPPERS-EMPTY-DOMAIN` records --
code a macro writes inside a function body, which the macro-position census admits -- and that the row
does not describe this mechanism. Agreed in part, as measured above: W1 and W2 hold the nested gate in
a function body, the position that census admits every invocation in; W3 holds it at item position in
a test-only module, inside no function body, and is accepted the same way. Either way, what this row
costs is a control's power to catch a reader defect, and so a weaker check on that row's standing class:
the readers are recognisers, not rustc's parser. That row is not edited.

## What the change that takes this up should do

Decide cut short over the gate's own element, not the span: count a gate as cut short only where the
second reading ends *its* element at or past the cut -- `configured_item_end` over the second
reading's blanked text, from the element's start, say -- so an enclosing element's closure no longer
counts for a gate inside it. Add rows that hand the comparison W1, W2 and W3 and require the refusal,
and keep every round-6 and round-7 row passing. Measured as a direction only: in a scratch copy of
`141fae34` with that one condition added to `cut_short`, `the_production_code_region_contains_the_truncated_one`
passes alone and the probe's W1, W2 and W3 forged readings are refused while their honest files and
the four committed misreading rows behave as before (rustc 1.97.1; never committed, and no other test
run against it). Not repaired in #325: the owner ruled out an eighth round unless a review found a P1,
and neither review of `141fae34` did.

## Provenance

Introduced by #325's seventh round (`3c135f42`), the repair of
`PR325-THE-WHOLE-REGION-ORACLE-REFUSES-AN-ELEMENT-REMOVED-IN-PART`, which added the cut-short test;
found by both reviews of `141fae34`, `claude-opus-5-5` at `max`
(https://github.com/sourcemaps/upstroke/pull/325#issuecomment-5879449999). Measured by #325's filing
pass: probe, exports and outputs in `~/pr325-filing-evidence/` on the build box.
