---
id: PR286-R4-BOUNDARY-FIXTURES-OMIT-LF-AND-CR
severity: P2
disposition: deferred
category: correctness
pr: 286
reviewed_sha: d2bf5ae8fcf6885daec83b275f088a80a7416838
location: .github/scripts/test-pr-ready-audit.sh:2276
provenance: introduced_by_feature
first_bad:
guard: a gate change that asserts every word-ending code point a numeric reference resolves to, U+000A and U+000D first
---

## Failure sequence

**The parser is correct here; the gate would not notice it stop being correct.** At `d2bf5ae8`,
`scripts/pr-review-parse.py` refuses every comment below, and `markdown-it-py` 3.0.0 renders each
one as a `language-json` block holding the blocking verdict, so refusing is right. What is missing
is a fixture in `.github/scripts/test-pr-ready-audit.sh` that fails if the parser stops refusing.
This is a gap in what the gate would catch next time, not a defect in what the parser does now,
which is why it is P2 and filed rather than blocking #286.

`rendered_language` (`scripts/pr-review-parse.py:450`) resolves an info string's references, then
takes the first `str.split()` word. `str.split()` ends a word at 29 code points, and `referable`
resolves a numeric reference to 23 of them. Spelled as a reference, each of those 23 ends the word
and can leave `json` as the tag. Six have a row that must refuse:

- U+0009, U+000C, U+00A0, U+2002 and U+3000, in the required-red loop at
  `.github/scripts/test-pr-ready-audit.sh:2276`;
- U+0020, as `json&#32;extra` at `:1889` and `:2250` (`&Tab;json` at `:1888` is U+0009 again).

The other seventeen have none: **U+000A, U+000D**, U+1680, U+2000, U+2001, U+2003 to U+200A,
U+2028, U+2029, U+202F and U+205F. The gate's only reference to U+000A is `jso&NewLine;` at
`:1941`. That row is a control, and it reads `jso` whether or not the word ends there.

A parser that stops ending the word at one of these therefore passes the whole gate, and a hidden
blocking verdict gets through. Executed 2026-09-14: each mutant was applied to a `git archive` of
`d2bf5ae8` in a scratch directory, and the complete, unmodified gate was run from that tree's root:

| parser | complete gate |
|---|---|
| unmodified | exit `0`, `test-pr-ready-audit: ok` |
| the word split at `:482` skips LF: `re.split(r"[^\S\n]+", ...)` in place of `.split(None, 1)` | **exit `0`, `test-pr-ready-audit: ok`** |
| `referable` refuses U+000A: `or point == 0x0a` added at `:446` | **exit `0`, `test-pr-ready-audit: ok`** |
| `referable` refuses U+000D: `or point == 0x0d` added at `:446` | **exit `0`, `test-pr-ready-audit: ok`** |
| the word split at `:482` skips CR | exit `1`: `MUT-BLOCK-CONTENT-NOT-MATERIAL [crlf]`, whose fence line ends in a **literal** CR |

Every surviving mutant produces a merge. The witness is the gate's own `entity_witness` (`:2153`):
round one's blockquoted swallow, with a clean `PASS` after it and the hidden fence tagged as below.
Each cell is `<parser exit>/<audit state>/<gh pr merge calls>`, run through
`scripts/pr-ready-audit.sh --enqueue` against the gate's recording stub:

| hidden fence tagged | unmodified | mutant |
|---|---|---|
| `jso&#110;&NewLine;x` | `1/NOT-READY/0` | `0/READY/1` when the split skips LF |
| `jso&#110;&#10;x` | `1/NOT-READY/0` | `0/READY/1` when the split skips LF, and when `referable` refuses U+000A |
| `jso&#110;&#13;x` | `1/NOT-READY/0` | `0/READY/1` when `referable` refuses U+000D |

`markdown-it-py` 3.0.0 renders each of those comments as exactly one visible block. That block is
`language-json` and holds the `CHANGES_REQUIRED` verdict; the `PASS` sits inside an unclosed HTML
comment.

**Where the evidence comes from.** #286's first conformance draw on `d2bf5ae8`
(`review-286-conformance-121215.log`) returned the LF mutant, the count of seventeen and the first
two witness rows. Its mutant was not kept on disk, so the LF split above was rewritten from the
draw's description. The two `referable` mutants and the CR split were run when this file was written.
They show that the draw's "LF, CR" is true for a CR written as a reference, and not for a literal CR,
which `[crlf]` covers.

## What the change that takes this up should do

Compute the required-red loop at `:2276` instead of sampling five points. It should be the
complement of the required-green loop at `:2266`: every code point `str.split()` ends a word at that
`referable` resolves, all 23. Add `entity_witness` rows for `jso&#110;&#10;x` and `jso&#110;&#13;x`
that count the merge call, because a boundary row asserts the parser's refusal and not the audit's.
Then run the three surviving mutants above against the gate; each must exit `1`.
