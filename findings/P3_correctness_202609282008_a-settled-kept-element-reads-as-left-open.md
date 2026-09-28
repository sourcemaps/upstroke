---
id: PR325-A-SETTLED-KEPT-ELEMENT-READS-AS-LEFT-OPEN
severity: P3
disposition: deferred
category: correctness
pr: 325
reviewed_sha: 3c135f42db47560f072bf5a2d135b9c7240be2c1
location: src/effects/tests/source_oracles.rs:2123
provenance: introduced_by_feature
first_bad: 323f18bb98b69622ec0cd1f60dfed5fc8a9b380a
guard: project owner — a later change to `compare_above_the_cut`; #325's seventh round found and filed it, not repaired
---

## Failure sequence

`effects::tests::the_production_code_region_contains_the_truncated_one` checks `production_code`
against the truncating region through `compare_above_the_cut`
(`src/effects/tests/source_oracles.rs`). Above the file's first `#[cfg(test)]`, the cut, it requires
the whole file's reading to agree with the reading of the text above the cut, and it stops comparing
at a gate whose element crosses the cut. Since #325's sixth round (`323f18bb`) one test for such an
element is that the reading of the text above the cut keeps the byte where the element starts: the
element is *left open*. The reader keeps an element whole when it cannot find its end, and a text that
stops inside the element is one reason it cannot.

It is not the only reason. `configured_item_end` also keeps an element whole when a token the text
above the cut already fixes stops it before the element's own end: a `)`, `]` or `}` that ends a header
before any body, an initializer before its `;` or a generic parameter before its `,` or `>`; a `;`
inside a header's angle brackets or a generic parameter; an unmatched `>` in a header; a `fn` item name
where a header or an initializer goes on; or a closer straight after the gate, with no element at all.
Nothing after the cut changes those, so such an element is not one the cut stopped inside, and yet it
reads as left open:

1. A gate whose predicate entails `test` stands above the cut, and its element is kept whole for one
   of those reasons -- `stringify!(#[cfg(all(unix, test))] fn t())`, where the macro's own `)` ends the
   header `fn t()`.
2. The reading of the text above the cut keeps `fn t()`, so the element counts as left open.
3. A whole-file reading that removes everything from that gate to the cut -- the element and the
   production code after it -- is then compared only above the gate, and the test accepts it.

Measured with such a whole reading forged, over `3c135f42`'s readers: both `be993da5`'s oracle and
`3c135f42`'s accept it, on the non-Rust row `f(#[cfg(all(unix, test))] fn t()) {}` and on the
`stringify!` edit below. The seventh round's other test, *cut short*, does not share the gap: it needs
a second reading of the same text, with the groups left open at the cut closed after the gate, to
remove everything from the gate to the cut, and the `)` that stops the element stands above the cut,
so no appended text makes it do so -- 0 of 56 continuations, on both inputs.

**Not an instance of the other two gaps the notes state.** Gap (a) is a tokeniser that reads a group
as open at the cut when it is not; here the text is tokenised correctly and nothing is open. Gap (b) is
a whole reading that removes code the text above the cut leaves undecided; here the text above the
cut decides it. Each of the three needs a different repair.

## Reachability

- **What an input has to look like.** A test-only gate above the cut whose element the reader keeps
  for one of the reasons above. Read from `configured_item_end`'s paths, each of those reasons is a
  token where Rust's grammar puts none in an item or a statement rustc parses: an item or statement
  ends before the group around it closes, a header holds no bare `;` or unmatched `>`, and a nested
  `fn` item sits inside braces. The rows of `a_test_only_element_is_removed_to_where_rustc_ends_it`
  that reach them are not Rust, and neither is `f(#[cfg(all(unix, test))] fn t()) {}`, the example
  #325's seventh-round body and notes first gave. An attribute is always followed by what it applies
  to, so a closer straight after one is not Rust either.
- **Honest code reaches it inside a token tree rustc does not parse as items**: a macro's arguments,
  measured below, and by the same reading an unexpanded `macro_rules!` body or an attribute's
  arguments. With `let _ = stringify!(#[cfg(all(unix, test))] fn t());` added at the start of
  `util::civil_from_days`, above `src/util.rs`'s cut, in an export of #325's `45c81ab0`:
  `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings` on rustc 1.97.1 and
  `RUSTFLAGS='-D warnings' cargo +1.85.0 check --locked --all-targets --all-features` exit 0, and
  `cargo test --lib -- effects::` passes 211 of 211. The reader keeps `fn t()` in the whole reading and
  in the reading above the cut, which is why the test passes.
- **On its own, honest code cannot make the test fail, or pass wrongly.** The gap excuses a *wrong*
  whole reading, and the reader's whole reading of such a file keeps the element as the reading above
  the cut does. What it costs is this test's power to catch a future reader defect that removed such an
  element, and the code after it, to the cut. It is a gap in a control's robustness, not a defect that
  touches real work.
- **Where it occurs, measured over `3c135f42`'s readers**: over its tree's 192 files (184 test-only
  gates, 11 of them above a cut) and the 11,041 files of the build box's cargo registry (2,373 gates,
  166 above a cut), the whole reading keeps no gate's element whole and no gate above a cut is left
  open. The `stringify!` edit makes one of each. So the gap is reached by no file of either corpus.

## What the change that takes this up should do

Tell a kept element the cut stopped inside from one the reader keeps for a settled reason, for the
left-open test as the cut-short test already does. The direct form: count an element as left open only
where some text appended after the gate at the cut lets the reader end it past the cut -- which needs
continuations that finish a `static`, `const` or `let` (a `;`) and a generic parameter (a `>`), since
the closers and blocks `closers_after_the_cut` appends finish only `if` chains, bodies and blocks, and
the round-6 rows' `const _` needs a `;`. Hold it with a row that hands the comparison the forged reading
above over the `stringify!` shape and requires the refusal, and keep every round-6 and round-7 row
passing. Not repaired in #325: its seventh round was scoped to the cut-short shape.

## Provenance

Found by #325's seventh round while writing the cut-short condition, by reading `configured_item_end`'s
paths against the left-open test, and measured there: evidence in `~/pr325-r7-evidence/gap-c/` and
`~/pr325-r7-evidence/differential/out/forge-3c135f42/settled.tsv` on the build box. The left-open test
dates from `323f18bb`, #325's sixth round, and the gap with it.
