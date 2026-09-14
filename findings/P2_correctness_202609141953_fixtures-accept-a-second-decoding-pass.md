---
id: PR286-R4-FIXTURES-ACCEPT-A-SECOND-DECODING-PASS
severity: P2
disposition: deferred
category: correctness
pr: 286
reviewed_sha: d2bf5ae8fcf6885daec83b275f088a80a7416838
location: .github/scripts/test-pr-ready-audit.sh:2227
provenance: introduced_by_feature
first_bad:
guard: a gate change adding a required-green example whose tag names json only when decoded twice
---

## Failure sequence

**The parser is correct here; the gate would not notice it stop being correct.** At `d2bf5ae8`,
`scripts/pr-review-parse.py` decodes an info string once, as `markdown-it-py` 3.0.0 does, and reads
each comment below the way that renderer shows it. What is missing is a fixture in
`.github/scripts/test-pr-ready-audit.sh` that fails if the parser decodes twice. This is a gap in
what the gate would catch next time, not a defect in what the parser does now, which is why it is P2
and filed rather than blocking #286.

**Corroborated: two draws found it independently.** #286's first and second conformance draws on
`d2bf5ae8` were the same prompt sent twice. Each returned this finding, in its own words: *"the
fixtures do not enforce single-pass decoding"* (`review-286-conformance-121215.log`) and *"the
controls do not enforce decoding exactly once"* (`review-286-conformance2-123145.log`). Both used the
same witness and gave the same reason the existing control misses it. No other finding from the three
lens runs on that head was returned by more than one run.

`rendered_language` resolves escapes and references in ONE left-to-right pass (`INFO_ESCAPE.sub` at
`scripts/pr-review-parse.py:482`), so nothing a reference resolves to is read again. Many rows assert
that the parser decodes at least once; no row asserts it decodes at most once. The row that appears
to, `example_tagged 'json&amp;#133;x'` at `:2227`, is required to be read, and it is read either way.
One pass leaves `json&#133;x`. A second pass leaves `&#133;` as written, because `referable` refuses
U+0085. So that row cannot tell one pass from two.

Executed 2026-09-14 on a `git archive` of `d2bf5ae8` in a scratch directory, with the complete,
unmodified gate. The mutant is the second draw's own, byte-identical to the one it recorded (sha256
`ccf74b61df6d8845c6e24f6ef01c9bd13e13159725cc870449271ea2fe6b3d12`):

| parser | complete gate |
|---|---|
| unmodified | exit `0`, `test-pr-ready-audit: ok` |
| `INFO_ESCAPE.sub` applied twice at `:482` | **exit `0`, `test-pr-ready-audit: ok`** |

The mutant refuses valid reviews. The witness is the gate's own `example_tagged` (`:2222`): an
ordinary example block, tagged as below, in front of the review's one real `PASS`. Each cell is
`<parser exit>/<audit state>/<gh pr merge calls>`:

| example tagged | unmodified | applied twice |
|---|---|---|
| `jso&amp;#110;` | `0/READY/1` | **`1/NOT-READY/0`** |
| `jso\&#110;` | `0/READY/1` | **`1/NOT-READY/0`** |
| `json&amp;#133;x` (the control at `:2227`) | `0/READY/1` | `0/READY/1` |

The mutant refuses with *"the review carries 2 places a verdict could be read from"*.
`markdown-it-py` 3.0.0 renders the first two examples with the language `jso&#110;`, beside one
`language-json` block holding `PASS`. Refusing that comment is the kind of refusal no rewording of
the comment can clear. It is the same direction as the round-2 over-refusal this pull request fixed,
`json&#133;x`.

## What the change that takes this up should do

Add required-green `example_tagged` rows for `jso&amp;#110;` and `jso\&#110;` beside the ampersand
control. Each should assert the parser row, `enqueued #999` and one merge call, as the other
`no-rendering-*` rows do. Keep the ampersand control: it is the byte-identical-HTML twin of
`json&#133;x`, which is what the comment at `:2080` says it is for. Then run the mutant above against
the gate; it must exit `1`. The second draw added the `jso&amp;#110;` assertion to a copy of the
section and recorded control `0`, mutant `1`.
