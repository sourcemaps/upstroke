---
id: PR290-R3-THE-HEADING-COVERAGE-CLAIM-EXCEEDS-ITS-CENSUS
severity: P3
disposition: deferred
category: docs-contract
pr: 290
reviewed_sha: 0320e2df9e7ca959f0a80146ef64126323839125
location: reviews/2026-09-14-o3-attribution-record.md:603
provenance: introduced_by_feature
first_bad: 33a90025c5ae913b80b7f20635dd87232c3c4bba
guard: the next change that edits `reviews/2026-09-14-o3-attribution-record.md` — it either adds the missing heading and widens the enumeration to variant fields, or narrows the sentence to what the enumeration measures, naming what it excludes
---

## Failure sequence

The record (`reviews/2026-09-14-o3-attribution-record.md:603`–`:606`) says: "every new production
item — type, field, function — has a section headed by its source line (`docs/internals/README.md`'s
grep-string rule; the check per item is in … `c3688ada…/round2/b4-headings.txt`, which enumerates
every new `pub` item of the diff …)". The enumeration's own selection line
(`/home/ubuntu/o3-attribution-evidence/c3688ada0b9558fa35ba7b2ac9885261be7b887d/round2/b4-headings.txt:2`)
says what it selects: "added lines declaring a pub type, field or fn, or the Display impl and its
fn". A named field of a public enum variant is none of those, so the enumeration never sees it,
and one exists: `Convicted { citation: &'a str }` at `src/events/mod.rs:492`, a field of the new
`pub enum EffectiveAttribution<'a>`. Re-executed at `0320e2df` and saved in
`…/0320e2df…/residue/finding-2-3-commands.txt`: `rg -n '^## .*Convicted'
docs/internals/events/mod.md docs/internals/interaction.md` exits **1** with no match — the field
is described inside the enum's section and has no heading of its own (the round-3 fix-check lens's
B4). The claim's universal "field" exceeds what its census measures.

A successor reading the sentence concludes that every new field has a heading to grep for, and
finds none for the one that carries a conviction's citation.

## What the change that takes this up should do

Either add the heading `` ## `pub enum EffectiveAttribution<'a>` › `Convicted { citation: &'a str },` ``
to `docs/internals/events/mod.md` and widen the enumeration's selection to variant fields, or
narrow the record's sentence to "every new `pub` type, `pub` field and function, and the `Display`
impl", naming the variant field as outside it.
