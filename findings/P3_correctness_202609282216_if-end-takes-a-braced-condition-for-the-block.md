---
id: PR325-IF-END-TAKES-A-BRACED-CONDITION-FOR-THE-BLOCK
severity: P3
disposition: deferred
category: correctness
pr: 325
reviewed_sha: 141fae347fb196548defcfc4d8ed3116b441edc9
location: src/effects.rs:685
provenance: introduced_by_feature
first_bad: 462faf0496b81f881cde945c2463b7da81faedff
guard: project owner — a later change to `if_end`; filed by #325's filing pass, not repaired
---

## Failure sequence

`production_code` ends a test-only `if` statement through `statement_end` and `if_end`
(`src/effects.rs`), which takes the statement's block to be the first `{` outside a group
(`first_block_brace`). Its notes gave the reason as "rustc refuses a struct literal in a condition unless
it is parenthesised", which is true and does not cover every brace a condition can hold:

1. A function holds a test-only `if` whose condition is a `match`, an `unsafe` block or a block
   expression: `#[cfg(all(unix, test))] if match x { 0 => false, _ => true } { g!(); }`.
2. `if_end` takes the `match`'s `{` for the `if`'s block, closes it at the `match`'s `}`, finds no
   `else` after it -- the next token is the `if`'s own `{` -- and ends the element there.
3. `production_code` removes `if match x { 0 => false, _ => true }` and keeps `{ g!(); }`, the `if`'s
   block, and its `else` chain where there is one, as production code.
4. A census that counts over the region counts what that block holds: test code.

Measured by #325's filing pass at `141fae34`, on rustc 1.97.1 and 1.85.0 alike, each input the body of
`fn f(x: u8) { .. x; }`:

| the test-only statement | what `production_code` keeps of the function |
|---|---|
| `if x > 0 { g!(); }` (control) | `fn f(x: u8) { x; }` |
| `if match x { 0 => false, _ => true } { g!(); }` | `fn f(x: u8) { { g!(); } x; }` |
| `if unsafe { h() } { g!(); }` | `fn f(x: u8) { { g!(); } x; }` |
| `if { x > 0 } { g!(); }` | `fn f(x: u8) { { g!(); } x; }` |
| the `match` condition with `else { g!(); }` | `fn f(x: u8) { { g!(); } else { g!(); } x; }` |
| `if let S { a } = s { g!(); }`, the case the notes admitted | `fn f(x: u8) { = s { g!(); } x; }` |

rustc accepts each of the three conditions: a file of four functions using them compiles with
`rustc --edition 2024 --crate-type lib` on 1.97.1 and 1.85.0, exit 0 both, the block condition drawing
an `unused_braces` warning.

## Reachability

- **Conservative.** The element ends early and test code stays in; no production code is removed --
  the direction of the `if let` braced-struct-pattern case the notes already admitted, and of the element
  rule. A census over the region can count more because of it, never less.
- **Where it occurs, measured over `141fae34`'s readers**: 2 test-only gates stand on an `if` in the
  tree's 192 files and 20 in the registry's 11,041, and at none of them does `if_end`'s end come
  straight before a `{`, which is what the misreading leaves. That count finds a condition its braced
  expression ends; one with more condition after the braces -- `if match x { .. } == y { .. }` -- would
  be missed.
- **What it would cost if reached**: a whole-tree census over the region demanding a row or refusing a
  shape for code that is test-only -- a false positive in an instrument, the class #325's fifth round
  repaired elsewhere. Not measured on any census.
- `if_end` is outside the two readers the fix-check review of `141fae34` was about; the rule and its
  prose are both #325's, from its fourth round (`462faf04`).

The filing pass corrected the notes (`docs/internals/effects.md`, `if_end`) to say what the rule does
with these conditions; the rule itself is unchanged.

## What the change that takes this up should do

Find the `if`'s block as rustc does: pass over a brace group that belongs to the condition -- after
`match` and its scrutinee, after `unsafe`, `loop` or `const`, or one the condition starts with -- or
read the condition as an expression. Add rows for each shape, and for the `if let` struct pattern,
requiring the whole statement removed.

## Provenance

`if_end` and its notes were written in #325's fourth round (`462faf04`). Found by the fix-check review
of `141fae34`, `claude-opus-5-5` at `max`
(https://github.com/sourcemaps/upstroke/pull/325#issuecomment-5879449999), which notes that the case is
conservative and outside the two readers it reviewed. Probe and outputs in `~/pr325-filing-evidence/`
on the build box.
