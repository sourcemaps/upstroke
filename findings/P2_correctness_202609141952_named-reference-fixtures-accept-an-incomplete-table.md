---
id: PR286-R4-NAMED-REFERENCE-FIXTURES-ACCEPT-AN-INCOMPLETE-TABLE
severity: P2
disposition: deferred
category: correctness
pr: 286
reviewed_sha: d2bf5ae8fcf6885daec83b275f088a80a7416838
location: .github/scripts/test-pr-ready-audit.sh:1941
provenance: introduced_by_feature
first_bad:
guard: a gate change whose named-reference rows fail when a name stops resolving
---

## Failure sequence

**The parser is correct here; the gate would not notice it stop being correct.** At `d2bf5ae8`,
`scripts/pr-review-parse.py` resolves the named references below and refuses each comment, and
`markdown-it-py` 3.0.0 renders each one as a `language-json` block holding the blocking verdict. What
is missing is a fixture in `.github/scripts/test-pr-ready-audit.sh` that fails when a name stops
resolving. This is a gap in what the gate would catch next time, not a defect in what the parser does
now, which is why it is P2 and filed rather than blocking #286.

`rendered_language` looks a named reference up in `NAMED_REFERENCE`
(`scripts/pr-review-parse.py:230`), which holds 2,125 names built from `html.entities.html5`. Four
gate rows carry a named reference. Only one of them depends on the name resolving:

| row | required | name resolved | name left as written |
|---|---|---|---|
| `enc_swallow '&Tab;json'`, `:1888` | refuse | `json`, refused | `&Tab;json`, parsed, and the row fails |
| `enc_swallow 'jso&NewLine;'`, `:1941` | parse | `jso` | `jso&NewLine;` |
| `enc_swallow 'jso&notareal;n'`, `:1943` | parse | not a name | not a name |
| `example_tagged 'json&amp;#133;x'`, `:2227` | be read | `json&#133;x` | `json&amp;#133;x` |

The second and fourth rows get the same answer either way, so no row asserts any name except `Tab`.
Sixteen names hold a character that ends a word, and each can make a hidden fence `json` the way
`&NewLine;json` does: `MediumSpace NewLine NonBreakingSpace Tab ThickSpace ThinSpace VeryThinSpace
emsp emsp13 emsp14 ensp hairsp nbsp numsp puncsp thinsp`. Only `Tab` has a row.

Executed 2026-09-14 on a `git archive` of `d2bf5ae8` in a scratch directory, with the complete,
unmodified gate. The mutant is #286's second conformance draw's own, byte-identical to the one that
draw recorded (sha256 `0449ba69ead5f59bf2d8b863d6b3c10416faa4dbeae414064b0dde0865e60683`):

| parser | complete gate |
|---|---|
| unmodified | exit `0`, `test-pr-ready-audit: ok` |
| `NewLine` dropped from `NAMED_REFERENCE` at `:230` | **exit `0`, `test-pr-ready-audit: ok`** |

That mutant produces a merge. The witness is the gate's own `entity_witness` (`:2153`) with the
hidden fence tagged as below, and each cell is `<parser exit>/<audit state>/<gh pr merge calls>`:

| hidden fence tagged | unmodified | `NewLine` dropped |
|---|---|---|
| `&NewLine;json` | `1/NOT-READY/0` | **`0/READY/1`** |
| `jso&#110;&NewLine;x` | `1/NOT-READY/0` | **`0/READY/1`** |
| `jso&#110;&#10;x` (the same character as a number) | `1/NOT-READY/0` | `1/NOT-READY/0` |

`markdown-it-py` 3.0.0 renders the first two as one visible `language-json` block holding the
`CHANGES_REQUIRED` verdict. The draw also named two spellings that no row carries and that a
renderer reads as `json`: `json&NonBreakingSpace;x` and `json&ensp;x`. Both are refused at this
head, `1/NOT-READY/0` each.

**Where the evidence comes from.** #286's second conformance draw on `d2bf5ae8`
(`review-286-conformance2-123145.log`) returned this, and it was re-run with the draw's own patch
when this file was written. Only `NewLine` was run as a mutant. That dropping any of the other
fifteen names also passes the gate follows from the first table; it was not run name by name.

## What the change that takes this up should do

Compute the named-reference rows from the table rather than choosing names. Run each of the sixteen
names above, as `&NAME;json` in `entity_witness`, as a required-red row that counts the merge call.
Replace the `jso&NewLine;` control with one whose answer changes if `NewLine` stops resolving. Then
run the mutant above against the gate; it must exit `1`.
