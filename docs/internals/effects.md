# `src/effects.rs`

Extended notes for [`src/effects.rs`](../../src/effects.rs).

The code is the authority for what it does. These notes started as the module's source prose.
Each code fragment in a heading is an exact source substring. When a heading names an enclosing
item before `›`, find that item first, then the following fragment within it.

## Module

The compile-time enforcement layer: the effect denylist, the allowlist, the
wrapper classification, and the generated inventories.

The retired `decisions.effect_site_inventory.mechanism` packet described
this module in the four parts below. They record its original rationale.
[DESIGN.md](../../DESIGN.md) is the living authority for product design;
the implementation rules are in [standards §2](../../standards/02_standards_automated_baseline.md)
for lint placement and the effects census, [§3](../../standards/03_standards_design_principles.md)
for effect boundaries, and [§15](../../standards/15_standards_dependencies_and_features.md)
for dependency review. Packet quotations here preserve history and do not
override those rules.

1. **The denylist is rustc-resolved, not lexical.** `clippy.toml`'s
   `disallowed-methods` / `disallowed-types` / `disallowed-macros` name every
   effect primitive the crate can reach, and "aliases, re-exports, function
   values, method calls, and macro-expanded code in this crate resolve to the
   same DefId". [`tests::every_declared_effect_denial_refuses_for_the_reason_it_declares`]
   compiles one fixture per shape and asserts the lint each emits, because
   that sentence is a claim about a toolchain and not a law of nature.
2. **An allow of a governed lint lives only where the allowlist says.**
   Module-level, in a file listed in `effects/allowlist.toml`, whose legacy
   section is frozen, may only shrink, and never contains a topology module.
3. **Wrapper classification.** Every externally reachable `fn` of a legacy or
   shared module is classified; the effectful ones join the denylist, "so a
   topology module cannot reach an effect through a legacy wrapper".
4. **Dependency review.** A new dependency performing filesystem, process,
   lock or container effects has its API added to the denylist or is confined
   to a funnel module.

### This module performs no effect

The non-test portion contains parsers, classifiers and frozen lists that
compute from supplied values. Reading `clippy.toml`, writing
`effect_sites.json` and compiling fixtures all happen in the test region.
The retired packet's `outputs` key also described inventory generation
by a test. That is why this file is
in the funnel section of the allowlist while claiming something stronger than
any other entry there.

### Reading the historical references

References to `decisions.*`, `mechanism` and `outputs` below name keys in
the retired packet. They explain provenance; the current design and
standards remain authoritative. `*_verification_dispositions`,
`finding_dispositions[].rationale` and the `v4_`..`v15_` keys belong to the
packet's disposition history and are not reproduced here.

## `#![cfg_attr(`

The three governed lints are `forbid` in this file's production build since
#318's third round, and conditional because of what sits below it. This file
stated no level for any of them and inherited none, so each took its level
from `-D warnings` alone, which an inner `allow` the placement scan does not
read lowers (`GUARD-DECISION-SILENT-PRODUCTION-FILES-OUTSIDE-THE-ROLL-CALL`).
Its only allowances below are `src/effects/tests.rs` and the eight files
under it, every one a whole-file test module, so an unconditional `forbid` is
`E0453` at the lib test target and the production build -- the lib target
CI's three clippy legs check and the binary links -- can forbid: the shape
#318 gave `src/agent/bin.rs`. `file_level_lint_state` reads it as the
production build's statement; a generated `allow` in this file's production
region is `E0453` at the lint gate
(`~/orch-pr10/repair-318-r3-evidence/controls/C2-inheritance-reach-*`), and
the lib test target alone compiles it, as for every conditional fence.
`effects::tests::every_unclassified_production_file_states_each_governed_lint_or_inherits_its_forbid` names this file the day the fence is removed
(`controls/C1-unfence-effects-root-*`). The allowlist row for this file says
the same: no allowance, and since #318 a production fence.

## `use std::collections::BTreeSet;`

Allowlist placement: the **funnel section** of `effects/allowlist.toml`, and
the entry there records `allows = []`. This module carries no attribute at
all, which is the strongest form of the claim above: it reaches no denied
primitive, and the one `std::process::Command` its text contains is inside
`DENIAL_FIXTURES`, a string constant compiled elsewhere in order to be
refused. `decisions.effect_site_inventory.mechanism` (2).

## `pub const CLIPPY_TOML: &str = "clippy.toml";`

---------------------------------------------------------------------------
The artifacts, by the names `outputs` gives them
---------------------------------------------------------------------------

## `pub const CLIPPY_TOML: &str = "clippy.toml";`

`effect_site_inventory.outputs`: "clippy.toml".

## `pub const ALLOWLIST_TOML: &str = "effects/allowlist.toml";`

`effect_site_inventory.outputs`: "effects/allowlist.toml".

## `pub const WRAPPERS_TOML: &str = "effects/wrappers.toml";`

`effect_site_inventory.outputs`: "the wrapper classification".

## `pub const EFFECT_SITES_JSON: &str = "effect_sites.json";`

`effect_site_inventory.outputs`: "effect_sites.json (from the enums)".

## `pub const RESIDUE_CLASSES_JSON: &str = "effects/residue-classes.json";`

`effect_site_inventory.outputs`: "the residue-class evidence record (per
element: constructed, classified, recovered; per site: sampling N and
observed-class histogram)".

The *declarations* half. The histogram half is [`RESIDUE_HISTOGRAM_JSON`],
and the split is forced rather than chosen — see there.

## `pub const RESIDUE_HISTOGRAM_JSON: &str = "effects/residue-histogram.json";`

The **observed-class histogram** half of the same record (`PR5-CONF-004`).

`outputs` requires, per site, "sampling N **and observed-class histogram**".
[`RESIDUE_CLASSES_JSON`] is generated from the frozen enums and compared
byte-for-byte, which it must be — and a histogram is machine-varying by
construction, since which class a kill sample lands in is a race between the
kill and Git. A count cannot be byte-pinned and a byte-pinned file cannot
carry one, so the histogram is emitted to this path on every run of
`workspace_manager::tests::sampled_git_child_kills_every_residue_classified_
and_recovered`, which then reads it back. Not checked in: its contents are a
property of the machine that produced them, and a stale copy of somebody
else's numbers would be worse than no copy.

## `pub const SEQUENTIAL_RESIDUE_HISTOGRAM_JSON: &str = "residue-histogram-sequential.json";`

The same half for the five residue-classified sites PR5's four-command sampler does not
run, written on every run by
`engine::topology::coverage::tests::sampled_git_child_kills_of_the_remaining_residue_sites_are_classified_and_recovered`.
A file name rather than a path: that module's tests resolve it (`sequential_histogram_path`)
against the observation export's directory when `UPSTROKE_HOOK_OBSERVATIONS` names one, and
otherwise against the profile directory the test binary runs from — never against the source
tree. Until PR10's round 7 it was `effects/residue-histogram-sequential.json` under the
manifest directory, gitignored like PR5's, and every build of one checkout shared it: a suite
in one build slot truncated it while the merge check in another read it (the round-7
regression lens, P3). The merge check and the regenerator read it where the sampler wrote it;
the ordinary tests read the declarations.

## `pub const FUNNEL_MODULES_JSON: &str = "effects/funnel-modules.json";`

Where each site's funnel **bodies** actually are, where that is not what
[`EFFECT_SITES_JSON`]'s `module` column says (`PR5-CONF-018`).

`effect_sites.json` is generated from the frozen enums, so its `module`
column is `EffectSiteId::module()` — PR3's answer, and the packet's:
`mechanism` (2) places "the answer funnels in `src/interaction.rs`". PR5's
lane B put the three Answer funnel bodies in `src/rundir.rs` and left
`interaction::{write_question, write_answer, read_answer}` as delegations,
so for `Answer.Ingest`, `Answer.PublishRename` and `Answer.StageWrite` the
checked-in artifact states something that is not true of this tree — and the
artifact is attached to gate reports, where a reader has no way to know.

The generator is `src/topology/effects.rs`, frozen under the owner ruling of
2026-08-20, so the column cannot be corrected in place and the bodies are not
moved: `AnswerSite`'s three funnels close over `rundir`'s private `funnel`
and `RunDirHooks`, and relocating them to satisfy a column would be a slice
redesigning what it implements. What ships instead is this companion, which
carries the tree's own answer beside the inventory's for **every** site, so
the pair is true where either alone is not. Derived, compared byte-for-byte,
and regenerated by the same `REGENERATE` switch, so it cannot drift.

## `pub const REGENERATE: &str = "UPSTROKE_REGENERATE_EFFECT_ARTIFACTS";`

The environment variable that turns the generating tests into writers.

A generated artifact that is only ever *compared* rots into a chore nobody
can discharge; one that is only ever *written* proves nothing. Both, keyed on
this, is the ordinary resolution.

## `pub const GOVERNED_LINTS: &[&str] = &[`

---------------------------------------------------------------------------
(2) The governed lints and where an allow of one may live
---------------------------------------------------------------------------

## `pub const GOVERNED_LINTS: &[&str] = &[`

The six lints `mechanism` (2) governs, as bare names.

> "permits allow/expect of disallowed_methods, disallowed_types,
> disallowed_macros, clippy::style, clippy::all, or warnings only as
> module-level attributes in files listed in effects/allowlist.toml"

Bare, because an attribute may write either `disallowed_methods` or
`clippy::disallowed_methods` and the sentence names them both ways in one
breath. [`normalize_lint`] is the bridge.

## `pub const USED_GOVERNED_LINTS: &[&str] = &[`

The three governed lints this slice actually uses, fully qualified.

`clippy::style`, `clippy::all` and `warnings` are governed and **unused**:
each would suppress far more than an effect denial, and
[`tests::the_three_blunt_governed_lints_are_used_by_nobody`] asserts the
count is zero rather than leaving it to habit.

## `pub fn normalize_lint(entry: &str) -> Option<&'static str> {`

The bare lint name an attribute entry refers to, if it is governed.

`clippy::disallowed_methods` and `disallowed_methods` are the same lint;
`clippy::too_many_arguments` is not governed and answers `None`.

**It names a lint the way Clippy's lint store does**, from the same two tables
the prologue reader's [`lint_levels::what_a_lint_path_names`] uses: a raw
segment is the name it spells (`clippy::r#disallowed_methods`, `r#clippy::..`,
`clippy::r#all`), an old name under the `clippy` tool is the governed lint it
was renamed to ([`RENAMED_TO_A_GOVERNED_LINT`]), and a one-segment old group
name is its group ([`PREFIXLESS_GROUP_ALIASES`]). Clippy applies every one of
those to the governed lint: a raw name alone, a rename or an alias with
`renamed_and_removed_lints` allowed beside it, since `-D warnings` otherwise
refuses its deprecation warning. Measured with `clippy-driver` 0.1.97 and
`-D warnings` on a file that denies `disallowed_methods` and writes through
`std::fs::write`, each spelling's `allow` builds; and each is compiled against
a `forbid` in `an_allowance_rustc_reads_whatever_separates_or_spells_its_tokens_is_one_the_placement_census_reads`
and against a `deny` in `the_file_level_lint_reader_answers_what_rustc_does`.
Until the attribute-token repair of 2026-09-26 this read none of them, so
the placement census recorded none of them while the compiler applied each.

Two spellings are left alone because Clippy applies neither (measured the
same way, each still a build error): an old name with no tool
(`disallowed_method`) and an old group name under the tool
(`clippy::clippy_all`). Any other path is named by its last segment, as it
always was, which reads `rustdoc::disallowed_methods` as the lint Clippy
does not apply it to: that over-reads, and a census that over-reads refuses
more, never less. `the_placement_census_names_a_lint_as_clippy_does_and_no_further`
holds each spelling both ways.

## `const PREFIXLESS_GROUP_ALIASES: [&str; 2] = ["clippy_all", "clippy_style"];`

Clippy's old names for the two groups that hold the governed lints, which
rustc still applies as the group, with a deprecation warning. One table for
[`normalize_lint`] and [`lint_levels::what_a_lint_path_names`], so the
placement census and the prologue reader cannot name a lint differently.

## `const RENAMED_TO_A_GOVERNED_LINT: [(&str, &str); 2] = [`

The renames in Clippy's table whose new name is a governed lint: rustc
applies `clippy::disallowed_method` to `clippy::disallowed_methods` and
`clippy::disallowed_type` to `clippy::disallowed_types`, with a warning.
`disallowed_macros` has none, and the prefixless old names apply to nothing.
Shared the same way as [`PREFIXLESS_GROUP_ALIASES`].

## `pub struct GovernedAllow {`

One `allow`/`expect` of a governed lint, as the scan found it.

## `pub struct GovernedAllow` › `pub line: usize,`

1-based line of the attribute's `#`.

## `pub struct GovernedAllow` › `pub inner: bool,`

Whether it is an inner attribute (`#![…]`).

## `pub struct GovernedAllow` › `pub module_level: bool,`

Whether it is module-level: an inner attribute in the file's prologue, or
an outer attribute on a `mod` item.

## `pub struct GovernedAllow` › `pub lints: Vec<String>,`

The governed lints it names, normalized, in source order.

## `pub struct GovernedAllow` › `pub written: Vec<String>,`

Every lint it names, as written — so a widening is visible — with the
separators between its tokens taken out: `clippy :: disallowed_methods` is
recorded as `clippy::disallowed_methods`, which is the one lint rustc reads
either way, so a spacing is no difference from the allowlist's row. A raw or
renamed spelling is kept as written, so it is a difference.

## `pub struct GovernedAllow` › `pub keywords: Vec<&'static str>,`

Which attribute keywords the governed lints were found under: `allow`,
`expect`, or both if one attribute writes both.

The two are not the same permission and the placement rule now
distinguishes them. `allow` is unconditional and says nothing when the
thing it permits stops happening; `expect` is refused by the compiler
when it goes unfulfilled, which is what makes a per-site one a count the
build owns rather than a claim a reviewer has to re-check.

## `pub struct GovernedAllow` › `pub reasoned: bool,`

Whether it carries a `reason = "…"`.

## `pub const RUSTC_WHITESPACE: [char; 11] = [`

The eleven code points rustc's lexer reads as whitespace between tokens:
U+0009 TAB, U+000A LF, U+000B VT, U+000C FF, U+000D CR, U+0020 SPACE,
U+0085 NEL, U+200E LRM, U+200F RLM, U+2028 LS and U+2029 PS. **This is the one
definition of a token separator here**, and it is rustc's rather than a
library's: `u8::is_ascii_whitespace` leaves out U+000B and cannot see the five
that are not ASCII, and `char::is_whitespace` leaves out U+200E and U+200F and
adds code points rustc refuses between tokens, U+00A0 among them.
`every_separator_rustc_reads_is_one_every_reader_here_reads` pins the eleven by
value; that rustc builds each of them with nothing reported, and refuses
U+00A0 and U+200B (whitespace to the eye and to no predicate) as unknown
tokens, was compiled once for round 4 of #309 and is in that round's evidence,
not in the suite.

The fourth review of #309 measured the difference at `84123789`. Three
readers took three sets -- the module walk `u8::is_ascii_whitespace`, the name
reader `char::is_whitespace`, the facade whitelist `str::split_whitespace` --
and two ordinary declarations, written with `#[rustfmt::skip]` and no macro,
passed every gate while a topology caller wrote through them: `mod`, U+200E, a
child of `src/engine/attempt.rs` that no walk judged, and `fn`, U+200E, a name
in `src/engine/coordinator.rs` that was no classification obligation and no
count. In the facade the whitelist admitted `mod`, U+000B, a name, and the walk
did not read it; only the facade's own deny kept an effect from following.
rustc reports none of this: its text-direction lints cover literals and
comments, not the space between tokens.

**It is applied once, where every reader gets its text.** The module walk, the
name reader, the whitelist and the other readers in this module and its tests
each decide where a token ends, with a byte test, a `trim` or a
`split_whitespace`, and they read [`blank_comments_and_strings`] or
[`production_code`], which is made from it. So that function writes each of
the six separators those predicates do not all read -- U+000B, U+0085, U+200E,
U+200F, U+2028, U+2029 -- as spaces of the same byte length, and on its output
the three library predicates and rustc's agree. A reader added later inherits
that; a predicate repaired reader by reader is what rounds 1 to 3 did, each
leaving the next reader's blind spot. [`declared_fns`], which matches text
rather than testing bytes, names this definition itself; the `trait`, `impl`
and `for` readers of [`externally_reachable_fns`] read a keyword as a whole
word and ask nothing of what follows it ([`keyword_sites`]). The byte readers
([`census_domain`]'s `whitespace` among them) keep `u8::is_ascii_whitespace`,
which on that text is the whole set.

**What it does not reach.** A reader that reads the raw source instead of the
tokenizer's text is outside it; the module walk read an attribute's name that
way and now reads it from the tokenizer's text (see `scan_modules`).
[`attribute_open`] reads the source between an attribute's tokens on purpose,
because there a doc comment and a literal are not separators and the
tokenizer's text cannot tell them from one; what it skips is
[`is_rustc_whitespace`] and plain comments, nothing else.
[`blank_comments`] keeps literals and is not rewritten: its callers look for a
string's contents, not for where a token ends. And a separator is one thing a
recogniser can misread, not the only one: see `PR7-WRAPPERS-EMPTY-DOMAIN`.

## `pub fn is_rustc_whitespace(character: char) -> bool {`

Whether rustc reads `character` as whitespace between tokens:
[`RUSTC_WHITESPACE`] as a predicate.

## `pub fn blank_comments(source: &str) -> String {`

`source` with every comment **removed** and every string literal **kept**
verbatim. A comment's newlines survive, so line numbers do; the rest of its
bytes do not, so nothing after the first comment sits at the offset it had in
`source`. Length preservation is [`blank_comments_and_strings`]'s contract,
not this one's.

```text
in: /*why*/let x = "docker";
out: let x = "docker";
```

Raw strings (`r"…"`, `r#"…"#`), byte strings, char literals and escapes are
handled; a `'a` lifetime is not a char literal and is left alone.
Comments removed, **string literals kept**.

The other half of [`blank_comments_and_strings`], and a separate function
because a census whose needle lives *inside* a string cannot use that one:
it blanks a literal including its quotes, so a search for `"docker` in its
output looks for a byte sequence the haystack can no longer contain. That is
not hypothetical — it is what the `mechanism` (1) "docker invocation
helpers" census did until PR6, which is why it stayed green when a real
`const DOCKER_PROGRAM: &str = "docker"` landed in production.

**One implementation, one caller shape.** `PR5D-VISIBILITY-CHECK-DUPLICATED`
is the standing entry for a parser written twice in this tree, so this lives
here beside its sibling rather than in each census that wants it.

Line comments, block comments (nested), char literals, escapes and **raw
strings** (`r"…"`, `r#"…"#`, `b"…"`, `br#"…"#`) are all handled: this
function tokenises exactly as [`blank_comments_and_strings`] does and differs
only in keeping a literal's bytes instead of blanking them. Byte offsets are
not preserved; line breaks are.

#### Why raw strings are modelled, and the direction the old limit had wrong

This used to track only `"` and document the omission as safe: "the failure
mode is a needle this function does *not* find, which makes a census that
uses it report something missing — **loud** — rather than accept something
extra." **That is backwards for a census over an expected set, which is what
every caller here is** (`PR6-LANEF-005`).

`r#"x" //"#` closed the literal at the second `"`, so the `//` that followed
began a line comment and **the rest of that line was deleted** — including a
real `"docker"` literal after it. `every_declared_effect_denial_names_a_real_path`'s
"docker invocation helpers" block asserts that the set of files naming a
container runtime is exactly a table of four; a fifth file whose literal was
erased is *absent from the computed set*, the sets compare equal, and the
census is **green with an extra Docker-naming file present**. A missed needle
is a false negative, and a false negative in a set comparison is fail-open,
not loud. The reviewer built that mutation and measured it.

So the residual is now the same as its sibling's: an unterminated literal
runs to end of input, which is a file that does not compile.

## `pub fn blank_comments(source: &str) -> String` › `while i < bytes.len() && bytes[i] != b'\n' {`

The newline itself is left for the outer loop, so line numbers
survive.

## `pub fn blank_comments(source: &str) -> String` › `match literal_end(bytes, i) {`

`r"…"`, `r#"…"#`, `b"…"`, `br#"…"#` — and an identifier that
merely begins with one of these letters, which is why the
preceding byte is checked and why a non-literal falls through
to a single push.

## `pub fn blank_comments(source: &str) -> String` › `match char_literal_end(bytes, i) {`

`'"'` is the one that matters here: without this arm it opens
a string. [`char_literal_end`] decides, so this and its
sibling cannot drift apart.

## `fn char_literal_end(bytes: &[u8], from: usize) -> Option<usize> {`

Where the char literal starting at `from` ends, exclusive, or `None` when
`from` does not start one.

A char literal is `'`, then either an escape (`\n`, `\\`, `\'`, `\u{1F600}`)
or **one UTF-8 scalar**, then `'`. The scalar is one to four bytes, and that
is the whole reason this is a scan rather than a lookahead.

#### The desync a fixed lookahead produces, and how far it reaches

Both blankers used to answer the question with two bytes: `'` is a char
literal when the byte at `+2` is a quote. `'é'` closes at `+3`, so it was
classified as **not** a literal, scanning resumed on its closing quote, and
that quote was then read as an *opening* one. From there the tokeniser is out
of phase: in `('é','{')` the pairing shifts by one and the `{` that is inside
a char literal survives into the blanked text as visible **code**.

One unbalanced brace is enough to take a file out of every census that
consults [`production_code`]. [`matching`] counts it, so
[`configured_item_end`]'s brace arm walks past the item's real `}`, finds no
balancing brace and gives up — and giving up used to mean "blank to end of
file".

Measured end to end, twice. On `src/agent/claude.rs`, with the pair inside
that file's `#[cfg(test)] mod tests` and a forged item appended below it, the
region measured **8525** non-whitespace bytes with the attack and 8525
without — a zero-byte delta no floor can see — and every source census was
green. Then gate-clean, because the first form is not: `cargo fmt` rewrites
`('é','{')` to `('é', '{')` and the space defuses it, and
`clippy::items_after_test_module` refuses an item placed below a file's own
`mod tests`. Both are avoidable. `stringify! { ('é','{') }` is left alone by
rustfmt (macro bodies in braces are), and a `#[cfg(test)]` module not named
`tests` is not what that lint looks for. With the probe inside
`src/runner/container/view.rs`'s `#[cfg(test)] pub(crate) mod fixtures` and a
forged `RunnerRequest {` builder above the file's real test module,
`cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` both exit
0 and `runner::contract::tests::every_production_runner_request_is_built_by_its_roles_\
builder` passes — while the identical forged builder **without** the probe
fails it by name.

The preconditions are already in this tree: `src/status.rs`, `src/util.rs`
(twice on one line) and `src/engine/tests.rs` all hold non-ASCII char
literals. Only the adjacency was missing.

`'a` is a lifetime and is not a char literal; nor is `'_`, nor the `'static`
in `&'static str`. All three are refused by one rule — the byte after the
scalar is not a quote — rather than by a list.

## `fn char_literal_end(bytes: &[u8], from: usize) -> Option<usize> {` › `at += 2;`

An escape, read the way rustc's lexer reads one: past the escaped character,
then to the next quote that is not itself escaped. A line break before that
quote means this was no char literal, so a scan cannot run past its own line.

**It used to be a window, and the window was wrong.** This note said the
longest escape Rust spells is `\u{10FFFF}`, closing at `from + 11`, and the
reader stopped looking at `from + 13`. A Unicode escape is one to six hex
digits and **any number of underscores**, so `'\u{7b____________________}'` is
legal and longer than any window. The regression lens of #309's fourth review
executed what follows from that at `84123789`: the reader gave up on the
literal, took the `','` after it for a char literal, and the `}` of a
following `'}'` survived as code. Written twice in each of two `impl` blocks
inside `stringify! { .. }` -- once with `'}'` and once with `'{'`, which keeps
the braces balanced for the module walk -- it closed both impls early, so two
effectful `rf4_write`s read as file-scope twins and the sharing rule admitted
two paths under one denial; fmt, clippy and 188 tests passed while a topology
caller wrote through the undenied one. The same text with the underscores
removed was refused. **A structural reading is only as good as the
tokenization under it**: every brace position [`reachable_fn_owners`] compares
comes from this function. Pinned by
`a_multi_byte_char_literal_does_not_desync_the_blanker` (each legal escape
form beside a brace) and by
`a_fn_behind_any_separator_is_unclassified_and_a_legal_escape_hides_no_brace`
(the review's witness and its control, both refused).

## `fn char_literal_end(bytes: &[u8], from: usize) -> Option<usize> {` › `let width = match *bytes.get(at)? {`

One UTF-8 scalar, whose width its lead byte states. A continuation or an
otherwise invalid lead cannot begin one, and `source` is a `&str`, so the
remaining ranges are unreachable rather than merely unhandled.

## `fn literal_end(bytes: &[u8], from: usize) -> Option<usize> {`

Where the string literal starting at `from` ends, or `None` when `from` does
not start one.

Accepts `"…"`, `b"…"`, `r"…"`, `r#"…"#` and `br##"…"##`. An unterminated
literal ends at end of input — a file that does not compile.

## `pub fn blank_comments_and_strings(source: &str) -> String {`

`source` with every comment and string literal replaced by spaces of the same
length, newlines preserved, **and every token separator rustc reads that is
not ASCII whitespace written as spaces of its own byte length**
([`RUSTC_WHITESPACE`]: U+000B, U+0085, U+200E, U+200F, U+2028, U+2029). The
output is exactly as long as the input, so an offset, a line and a column
measured in it are `source`'s own.

This is the tokenizer every structural reader here starts from, which is why
the separators are settled here and not in each reader: after it, a byte test
for ASCII whitespace, `char::is_whitespace` and `split_whitespace` all read
exactly what rustc reads between tokens. A source that writes none of the six
comes back byte for byte as `code_bytes_only` made it. LF stays LF, and the
other four ASCII separators stay what they are, so a CRLF checkout reads as it
did.

```text
in: /*why*/let x = "docker";
out:        let x =         ;
```

The scan has to be blind to text that only *looks* like an attribute.
`PR4-CENSUS-COMMENT-ORACLE` is in the standing ledger because a source census
counted a doc comment; this module is worse placed than most, since its own
build-refusal fixtures are `#[allow(clippy::disallowed_methods)]` written
inside doc comments and string literals. Blanking rather than deleting keeps
every byte offset — and therefore every line number — exact.

A needle that lives *inside* a literal cannot be looked for in this output:
the quotes are blanked with it, so `"docker` is a byte sequence the haystack
can no longer contain. [`blank_comments`] is the half that keeps it, and
`the_notes_give_each_blanker_its_own_contract` runs both worked examples
above against the functions themselves.

## `fn code_bytes_only(source: &str) -> String {`

The blanking itself, unchanged since before the separators were settled above
it: comments and literals to spaces, code bytes kept, newlines kept.

## `fn code_bytes_only(source: &str) -> String` › `for (index, byte) in bytes.iter().enumerate() {`

Newlines survive so line numbers do.

## `fn code_bytes_only(source: &str) -> String` › `let mut j = i;`

`r"…"`, `r#"…"#`, `b"…"`, `br#"…"#`

## `fn code_bytes_only(source: &str) -> String` › `match char_literal_end(bytes, i) {`

[`char_literal_end`] decides, so this and its sibling in
[`blank_comments`] cannot drift apart.

## `pub fn production_region(source: &str) -> String {`

The production region: everything before the first `#[cfg(test)]` that is not
inside a comment or a string.

## `pub fn production_code(source: &str) -> String {`

The production **code** of `source`: comments and string literals blanked,
and every `#[cfg(test)]`-configured item removed.

[`production_region`] keeps its truncating answer for the censuses that pin
its cut point by name (`every_production_region_that_stops_early_stops_at_a_module`),
and it is no longer what the classification domain reads.
`PR7-WRAPPERS-EMPTY-DOMAIN` measured why: the truncating region was said to be
what a **domain** question wants, because everything above the cut is
certainly production -- but everything below it is certainly outside the
domain, and in six classified modules the cut was a `#[cfg(test)] use` among
the imports, so the domain was empty and an all-empty record passed. This
function's answer is production in both directions, and
[`externally_reachable_fns`] reads it. Three failures a prohibition census
pays for with a truncating region, all three measured on this tree:

* A file that declares its tests as `#[cfg(test)] mod tests;` — the
  `tests.rs` entries of `effects::tests::cfg::WHOLE_FILE_TEST_MODULES` —
  puts every line **below** that declaration outside the region.
  The declaration is usually the last item, so the hole is normally empty;
  appending to the file fills it. Legal Rust, no comment trick, and it
  defeated the barrier census, the process-start census and the container
  token census at once.
* A `#[cfg(test)]` inside a block comment or a string literal truncates a
  region that a `//`-only strip cannot see. `PR4-CENSUS-COMMENT-ORACLE`,
  in the shape a `//`-only strip does not close.
* Counting over unblanked text counts prose. `src/agent/proc.rs` names
  `run_with_timeout` eight times, five in code and three in doc comments, so
  a real ninth entry point could be paid for by deleting two sentences.

So this returns the **whole file**, blanked, with each `#[cfg(test)]` item
blanked out in place. Newlines survive, so a byte offset still maps to the
line it came from.

The item's extent is found by delimiter matching over the blanked text — a
brace body ends at its matching `}` (and takes a trailing `;` with it, for
`use a::{b, c};`), anything else ends at the first `;` or `,` outside a
nested delimiter, except that a recognized function return type keeps its
commas until the function body or semicolon. A closing delimiter that would
leave the enclosing block ends the item too. Angle brackets are not matched: a
`#[cfg(test)] field: BTreeMap<K, V>,` ends at the comma inside the generics
and leaves `V>,` behind. That is the safe direction — a region that is too
**large** can only make a census match more, never less.

## `pub fn production_code(source: &str) -> String` › `while let Some(at) = bytes`

Searched over bytes rather than `str::find`, because a cut offset is not
guaranteed to be a char boundary and slicing one panics.

## `pub fn production_code(source: &str) -> String` › `let mut start = at + ATTR.len();`

Any further attributes stacked on the same item belong to it.

## `fn configured_item_end(bytes: &[u8], start: usize) -> usize {`

Where the item beginning at `start` ends, exclusive. See [`production_code`].

**The two give-up paths return `start`, not `bytes.len()`.** Both are reached
only when the blanked text does not parse — an unbalanced brace, or an item
with no terminator before end of file — and neither is reachable from this
tree today (measured: zero occurrences over all 92 source files). What
decides the value is the *direction* they fail in. `bytes.len()` reads "the
item is the rest of the file" and blanks it, so a tokeniser that has lost
phase silently removes every production item below the attribute from every
census that consults this region — which is exactly what
[`char_literal_end`]'s desync used to buy. Returning `start` blanks the
attribute and nothing else, so the test module below it reads as production
and the censuses go **loud** instead. The larger region is always the safe
one here, for the same reason the doc above gives for not matching angle
brackets: it can only make a census match more, never less.

## `pub fn governed_allows(source: &str) -> Vec<GovernedAllow> {`

Every `allow`/`expect` of a governed lint in `source`, with where it sits.

Attributes are found in the blanked text and read out of the original, so a
fixture quoted in a doc comment is invisible and a real attribute is not.

**An attribute's tokens are read wherever rustc reads them.** `#`, `!`, `[`
and the keyword's `(` are separate tokens, and rustc reads whitespace and
plain comments between any two of them: `# ![allow(..)]`, `#! [allow(..)]`,
`#![allow (..)]`, `#/* c */[allow(..)]`, and U+200E or any other of
[`RUSTC_WHITESPACE`] between each pair are applied under `-D warnings`
exactly as the joined spelling is. This census wanted them touching until
the attribute-token repair of 2026-09-26, so an allow the compiler applied
was an allow no census recorded: the fifth review of #309 executed that,
behind `#[rustfmt::skip]`, with every gate green and the bytes written
(`PR7-WRAPPERS-EMPTY-DOMAIN`). [`attribute_open`] finds the attribute and
[`past_comments_and_whitespace`] the keyword's `(`, both in the source, the
way the prologue reader has found `#`, `!` and `[` since #318's fifth round;
the lint list is read in the blanked text, and a lint path is named by
[`normalize_lint`]. A doc comment is a token, not a separator: between the
tokens rustc refuses it, and it ends the attribute here too.

`the_placement_census_reads_an_attribute_whatever_rustc_reads_between_its_tokens`
holds every separator in every gap, against the joined spelling as the
control, and holds a doc comment, a literal and a string between the tokens
as no attribute; `an_allowance_rustc_reads_whatever_separates_or_spells_its_tokens_is_one_the_placement_census_reads`
compiles each spelling against a `forbid` so the reading is of an allowance
rustc applies.

## `fn attribute_open(source: &str, blanked: &[u8], hash: usize) -> Option<(bool, usize)> {`

Whether an attribute starts at `hash`, and if so whether it is inner and
where its `[` is. `hash` has to be a `#` in the blanked text, so a `#` in a
literal or a comment is none. The gaps after it are read in the source, past
rustc's whitespace and plain comments ([`past_comments_and_whitespace`]):
in the blanked text a doc comment and a literal are spaces like any other,
and rustc reads neither as a separator. The module walk, the placement
census, [`is_module_level`] and [`lint_levels::leading_inner_attributes`]
all find an attribute here, so they cannot disagree about where one is.

## `fn past_whitespace(bytes: &[u8], from: usize) -> usize {`

The first byte at or after `from` that is not ASCII whitespace. On the
blanked text that is rustc's separators and every comment, which is what the
readers that start from it want between an attribute and the item it is on.

## `fn past_comments_and_whitespace(source: &str, from: usize, doc_comments_too: bool) -> usize {`

The next token at or after `from`: past rustc's whitespace and plain
comments, and past doc comments too when asked. Here rather than in
[`lint_levels`] since the attribute-token repair, because the placement
census reads the gaps in an attribute with it.

## `pub(crate) fn is_doc_comment(bytes: &[u8], at: usize) -> bool {`

Whether the comment opening at `at` is a doc comment, as rustc's lexer
classes it: `//!` and `/*!` inner; `///` and `/**` outer; `////`, `/***` and
`/**/` plain. rustc lexes a doc comment as an attribute, not as whitespace,
so [`census_domain::with_literal_identity`] refuses one inside an attribute's
text, and [`attribute_open`] and [`lint_levels`]'s prologue walk refuse one
between an attribute's `#`, `!` and `[`; between inner attributes an inner
one is an inner attribute that states no level, and an outer one ends the
prologue.

## `pub(crate) fn block_comment_end(bytes: &[u8], from: usize) -> usize {`

The end of the nested block comment opening at `from`, as
`code_bytes_only` counts it: `/* /** */ */` is one plain comment, and the
`/**` inside it opens no doc comment. For [`past_comments_and_whitespace`],
which skips a comment between inner attributes, and between one's `#`, `!`
and `[`, by the same count.

## `fn matching(bytes: &[u8], open: usize, opener: u8, closer: u8) -> Option<usize> {`

The index of the bracket closing the one at `open`, or `None`.

## `fn is_module_level(source: &str, bytes: &[u8], hash: usize, close: usize, inner: bool) -> bool {`

An inner attribute in the file's prologue, or an outer attribute on a `mod`.

"Module-level" is the whole of the placement rule, so it is decided here
rather than by eye: an `#![allow(…)]` before the first item governs the file
module; a `#[allow(…)] mod inner { … }` governs that module; an attribute on
a function, a statement or an expression governs neither and is what the rule
exists to refuse.

The attributes are found by [`attribute_open`] and the words past
whitespace, so a prologue attribute written apart, `pub (crate)`,
`pub(in path)`, `pub(self)` and a `mod` followed by a tab or a line break are
read as rustc reads them. Until the attribute-token repair an inner allow
after a spaced first attribute, and an allow on any of those modules, read as
below module level, and the placement census refused a placement rustc
allows: that failed closed, so correcting it opens nothing.

## `fn is_module_level(source: &str, bytes: &[u8], hash: usize, close: usize, inner: bool) -> bool {` › `if inner {`

Nothing but whitespace and other attributes may precede it, from the file's
first byte.

## `fn is_module_level(source: &str, bytes: &[u8], hash: usize, close: usize, inner: bool) -> bool {` › `let mut at = past_whitespace(bytes, close + 1);`

Outer: skip further attributes and whitespace, then a visibility and its
restriction, then require the word `mod`.

## `fn word_at(bytes: &[u8], at: usize, word: &[u8]) -> bool {`

Whether `word` is written at `at` as a whole word, so `module` is not `mod`.

## `pub const FROZEN_LEGACY_ALLOWLIST: &[&str] = &[`

---------------------------------------------------------------------------
(2) The frozen legacy section
---------------------------------------------------------------------------

## `pub const FROZEN_LEGACY_ALLOWLIST: &[&str] = &[`

The legacy section of `effects/allowlist.toml` as PR5 freezes it.

> "the legacy section may only shrink after PR5 (the test compares against
> the frozen list) and never contains a topology module"

Held here rather than only in the TOML because the TOML is the thing under
test: a frozen list that lived in the file it freezes would agree with any
edit to that file.

One entry was added after PR5, by the owner's decision on #306
(`PR7-WRAPPERS-EMPTY-DOMAIN`), and removed again on 2026-09-20:
`src/engine/mod.rs`, the v0.1 conductor's facade, whose only denied calls were
the two conductor entry points denied by path in that change. The list and the
TOML grew in the same commit, which is the only way
`the_legacy_section_is_frozen_and_may_only_shrink` admits an entry, and the row
said how every module below the facade was kept from inheriting the allow. It
could not say the same of what the facade itself held: the fourth review of
#306 (`PR306-FACADE-INLINE-ESCAPE`) reached `std::fs::write` from
`engine::topology` through an inline module written in the facade and through
a function placed in it, both under that allow, neither classified. The row's
own `shrinks_when` was the remedy -- the entry points moved into
`src/engine/coordinator.rs` and `src/engine/resume.rs`, the facade re-exports
them and calls nothing denied -- so the row and this entry went together, and
the list is again what PR5 froze. Putting the facade back needs an edit here,
which is the point of holding the list in the code.

## `pub const TOPOLOGY_MODULES: &[&str] = &[`

The modules the legacy section may never contain, verbatim from `mechanism`.

> "never contains a topology module (src/topology/**, src/runner/**,
> src/workspace_manager.rs, src/engine/topology.rs)"

The ban is on the **legacy** section alone, which is why
`src/runner/{host,container,invocation}.rs` and `src/workspace_manager.rs`
are in the funnel section without contradiction — the same sentence lists
them there.

`src/engine/topology/` is the fifth entry and is **not** in the packet
sentence, which names `src/engine/topology.rs` only. It is here because
[`topology_modules_among`] matches with `str::starts_with`, and
`"src/engine/topology/create.rs"` does not start with
`"src/engine/topology.rs"` — the sentence's four shapes were written when
the schema-4 engine was one file, and PR7 makes it a directory. Without
this entry the ban silently stops covering every submodule of the module it
exists to cover. Widening a ban is not a relaxation of the packet, and
`the_legacy_section_never_contains_a_topology_module` executes the gap: it
asserts the four-entry list misses a submodule that the five-entry list
catches.

The `src/workspace_manager/` entry is **that paragraph again**, for the same
reason and with the same evidence. The sentence names
`src/workspace_manager.rs` — a file — and `"src/workspace_manager/residue.rs"`
does not start with it. That cost nothing while the directory held only
`fixture.rs` and `tests.rs`, both `#[cfg(test)]`; the `m4-workspace` split
puts eight **production** modules there, and without this entry the ban would
silently stop covering the funnel's own production code in the very commit
that created it. Restoring coverage a split removed is neutrality rather than
a widening of the packet, and the gap is executed below just as the
`src/engine/topology/` one is.

**What is reachable through the hole, stated rather than dressed up.** The
ban is on the legacy section alone, and
`the_legacy_section_is_frozen_and_may_only_shrink` pins that section by
length *and* by exact set equality, so it cannot grow at all. Reaching this
hole therefore means first editing a PR5-frozen production constant. It is
lost defence-in-depth, not a live escape — and it is closed here because
`m4-workspace` is the only split that opens it: `src/topology/` and
`src/runner/` are already prefixes, and `src/rundir.rs` and
`src/agent/proc.rs` are not in this list at all.

**Why this list takes a directory prefix and [`CLASSIFIED_MODULES`] does
not**, since one commit does both and the two answers look contradictory.
They are matched differently on purpose. Entries here are matched with
`str::starts_with`, so a prefix is the only form that covers a module tree,
and a ban that covers more is strictly better. Entries there are joined onto
the manifest root and **read as source files** by
`reachable_fns_are_classified`, so a directory would name nothing at all;
that list is a roll-call whose whole point is per-module review, which is why
its children are enrolled one path each (`C-002`).

## `pub fn legacy_growth<'a>(frozen: &[&str], current: &[&'a str]) -> Vec<&'a str> {`

Entries of `current` that the frozen list does not contain — i.e. growth.

A pure function over its inputs precisely so the refusal can be *executed*
against a list that does grow, rather than inferred from one that does not.

## `pub fn topology_modules_among<'a>(paths: &[&'a str]) -> Vec<&'a str> {`

Entries of `paths` that name a topology module.

## `pub const CLASSIFIED_MODULES: &[&str] = &[`

---------------------------------------------------------------------------
(3) Wrapper classification
---------------------------------------------------------------------------

## `pub const CLASSIFIED_MODULES: &[&str] = &[`

The modules whose externally reachable `fn`s `mechanism` (3) classifies.

> "at PR5 every pubfn of a legacy or shared module is classified effectful or
> effect-free by review"

**Legacy** is the frozen legacy section. **Shared** is the modules the slice
`scope` names — "shared primitives (locks, run-dir creation and marker,
answer staging/ingestion, util JSON write, the exact-snapshot primitive incl.
its ephemeral commit, the event-log writer) moved behind funnels with Shared
sites" — plus the process funnel, whose `Shared` sites PR4 landed.

`src/topology/effects.rs` and `src/effects.rs` are deliberately outside the
domain: both are in the allowlist's funnel section, and neither is legacy nor
shared. Between them they declare 208 + n functions that touch nothing, and
classifying them would bury the rows that matter.

## `"src/workspace_manager.rs",`

shared

## `"src/rundir/classify.rs",`

The five production children the `m3-rundir` split gave `src/rundir.rs`.
They are named **one path each**, which is the only form this list has:
`reachable_fns_are_classified` joins every entry onto the manifest root
and reads it as a source file, so a directory prefix would name nothing
and the entry above cannot be widened into one. `C-002` is the standing
finding that this roll-call is hand-maintained rather than derived, and it
is not this split's to repair. (`TOPOLOGY_MODULES` above is the list that
*does* match with `starts_with`, and it does not name `src/rundir.rs` at
all -- neither the ban it serves nor the prefix question reaches here.)

They are here because the split moved seventeen externally reachable
`fn`s out of `src/rundir.rs`, and a name that leaves the domain is a name
nobody has to classify any more. Listing the children keeps the whole of
the run-directory subsystem inside `mechanism` (3)'s classification with
the same names accounted for on the other side of the move; the funnels,
and every effect site, stayed in the parent.

## `"src/runner/host/environment.rs",`

The three production children the `m5-host` split gave
`src/runner/host.rs`: `environment.rs`, `naming.rs` and `probe.rs`, named
**one path each**. That is the only form this list has:
`reachable_fns_are_classified` joins every entry onto the manifest root
and reads it as a source file, so a directory prefix would name nothing,
and `"src/runner/host.rs"` is therefore left as an exact path rather than
widened into `"src/runner/host"`. `C-002` is the standing finding that
this roll-call is hand-maintained rather than derived, and it is not this
split's to repair. (`TOPOLOGY_MODULES` is the list that *does* match with
`starts_with`, and it already names `src/runner/`, so neither the ban it
serves nor the prefix question reaches here.)

They are named because the split moved ten externally reachable `fn`s out
of `src/runner/host.rs`, and a name that leaves the domain is a name
nobody has to classify any more. Naming the children keeps the whole of
the host boundary inside `mechanism` (3)'s classification with the same
names accounted for on the other side of the move; the funnel, both
`ProcessSite` values, the `Contained` mint and the reserved-key
vocabulary stayed in `src/runner/host.rs`.

## `"src/runner/container.rs",`

The third of `mechanism` (2)'s `src/runner/{host,container,invocation}.rs`,
added by PR6. It is here rather than only in the allowlist because it
denies six of its own paths — the "docker invocation helpers" the same
sentence enumerates — and `every_effectful_wrapper_is_on_the_disallowed_list`
requires a `upstroke::` denial to be a row somebody classified.

## `"src/runner/container/view.rs",`

The body of the Container funnel's R19 view, added by PR7's census
repair. It carries `#![allow(clippy::disallowed_methods)]` over its
production region and was the **only** non-test production module in the
tree in that position and absent from this list. The consequence was not
theoretical: with no row here, none of its `pub fn` needed classifying,
so `every_effectful_wrapper_is_on_the_disallowed_list` could never force
one onto the denylist — a module that may reach `fs` under its own allow,
and whose reachable surface nobody had to account for.

## `"src/engine/coordinator.rs",`

legacy

## `"src/agent/proc/ambient.rs",`

The three production children the `m6-proc` split gave
`src/agent/proc.rs`: `hooks.rs` (the observation and injection surface),
`ambient.rs` (the ambient Job Object and the reclaim scope) and `drain.rs`
(the pipe reader), named **one path each**. That is the only form this
list has: `reachable_fns_are_classified` joins every entry onto the
manifest root and reads it as a source file, so a directory prefix would
name nothing, and `"src/agent/proc.rs"` is therefore left as an exact path
rather than widened into `"src/agent/proc"`. `C-002` is the standing
finding that this roll-call is hand-maintained rather than derived, and it
is not this split's to repair.

They are named because the split moved externally reachable `fn`s out of
`src/agent/proc.rs` and made previously private ones `pub(super)` in the
children -- the same visibility a private item of `proc` had, and the
visibility `externally_reachable_fns` counts. Naming the children keeps
every one of those names accounted for on the other side of the move.
Stated as a property and not as a tally: this list merges as a sorted
union and a count beside it does not, so a number here would be wrong at
the next edit to any of these files rather than at a merge anyone reads.
The funnel entry point, both `ProcessSite` values, the no-degraded-mode
memo, `windows_job` and `termination` stayed in `src/agent/proc.rs`.

## `pub fn externally_reachable_fns(source: &str) -> Vec<String> {`

Every `fn` of `source`'s production code that is reachable from outside its
module. The region is [`production_code`]: comments and string literals
blanked, every `#[cfg(test)]` item removed in place.

**The region was the truncating one until `PR7-WRAPPERS-EMPTY-DOMAIN`.**
[`production_region`] cuts a file at its first `#[cfg(test)]`, and in six
classified modules -- `src/engine/{attempt,coordinator,resume}.rs` and
`src/agent/{claude,codex,copilot}.rs` -- that is a `use` among the imports, so
the derived domain was empty, `effects/wrappers.toml` recorded all four
buckets as `[]`, and `reachable_fns_are_classified` compared an empty set
with an empty set: forty-seven names classified by a census that read none
of them, and a `pub(super) fn` below the cut, called from a live topology
module, passed clippy and the suite. `src/agent/proc.rs` was cut inside
`windows_job` and lost fifteen names the same way. Pinned by
`a_configured_item_above_a_production_fn_does_not_hide_it_from_the_domain`
(the shape) and `every_classified_module_that_declares_a_visible_fn_has_a_domain`
(the six modules by name, and every classified module that declares a
visible fn).

Three shapes, because "pubfn" in the packet's sentence has three of them in
this tree and a classification that saw one would be complete against a
domain nobody drew:

* `pub fn` / `pub(crate) fn` / `pub(super) fn` / `pub(in a::b) fn` items, free
  or in an inherent `impl`;
* every `fn` inside an `impl <Trait> for <Type>` block, which is reachable
  through the trait whatever its own visibility says;
* associated `fn`s of a public trait's default bodies, which are the same
  case.

Names are returned once each, sorted. Two `impl` blocks with a `new` apiece
are one row: the classification is of a *name in a module*, and a name that
is effectful in one impl is a name the denylist has to carry anyway.

**The third shape was documented and not implemented until repair round F1**
(`PR6-REACHABLE-FN-PARSER-MISSES-TRAIT-DEFAULTS`, refiled as
`PR6-LANEF-007`). The predicate was `visible || in_trait_impl`, and a default
body inside a `pub trait` declaration is neither: it carries no visibility of
its own and it is not in an `impl … for …` block. Lane F filed it as narrow
because no such body reached an effect; the reviewer **built one** —
`fn remove_without_a_site(&self, path: &Path) { let _ = fs::remove_file(path); }`
as a default method on the public `ContainerHooks` — and clippy, all 79
effects tests and all 38 container tests passed. A default body is the one
place in this tree where an effect could be added to a *classified* module
without appearing in its classification.

A trait method **declaration** (no body) is deliberately still excluded: it
performs nothing, and every implementation of it is reached by the
`impl … for …` shape above.

## `pub fn externally_reachable_fns(source: &str) -> Vec<String>` › `for (start, after) in keyword_sites(&region, "trait") {`

`pub trait X: Y { … }` — the bodies inside are reachable through the
trait, exactly as a trait impl's are.

**`trait` as a whole word, whatever follows it.** This was the text `trait` and
one U+0020, and the `impl` reader below wanted one U+0020 or `<` after `impl`
and one U+0020 on each side of `for`. A tab or a line break is enough to miss
all three, no Unicode needed: `pub trait`, a tab, a name left every default
body of that trait outside the domain, and `impl`, a tab, a trait, `for` left
every fn of that impl outside it. Found while settling the separators in round
4 of #309, by reading; the tree writes none of them and the domain did not
move (759 names in the 54 classified modules).
`the_reachable_fn_parser_finds_each_shape_this_tree_uses` holds all eleven
separators after `fn`, `trait` and `impl` and around `for`.

## `pub fn externally_reachable_fns(source: &str) -> Vec<String>` › `for (_, after) in keyword_sites(&region, "impl") {`

`impl <something> for <something> {` — the `for` is what makes it a trait
impl; an inherent `impl Type {` has none before the brace.

**Both keywords are whole words and nothing more is asked of their
neighbours**, because rustc asks nothing more: `impl::path::Trait for Thing`,
`impl<T> Glued<T>for Thing<T>` and `impl Trait for&'static str` are trait
impls, and a reader that wants a separator after `impl`, or on each side of
`for`, leaves their methods outside the domain (found by attacking round 4's
own first draft, which had widened "one U+0020" to "a separator" and no
further; executed as parser omissions only). The price is an over-reading,
and it is pinned: an inherent impl whose header holds a higher-ranked bound,
`impl<F: for<'a> Fn(&'a u8)> Holder<F>`, reads as a trait impl, so its
private fns are in the domain. That costs a row and fails closed. The next
token cannot tell the two apart -- `impl Tr for <X as Y>::Out` is a trait impl
whose `for` is followed by `<` too -- and the tree holds no such header: no
name moved.

## `pub fn externally_reachable_fns(source: &str) -> Vec<String>` › `let is_default_body = public_trait_spans`

A default body in a public trait, and only a default *body*:
`find_header_brace` answers `None` at the `;` of a declaration.

## `fn declares_visibility(prefix: &str) -> bool {`

Whether the text immediately before a `fn` declares it visible outside its
module — with the `pub const fn` / `pub unsafe fn` / `pub async fn`
modifiers stripped first.

**One copy, deliberately.** This was written twice — once for the bare case
and once inside the modifier-stripping fallback — and a mutation that broke
the `pub(crate)` arm of the first copy left the whole suite green, because
the second copy still caught it. Two hand-maintained lists of three strings
disagree eventually, and the one that disagreed silently would be this one.
Measured, mutation `the-parser-misses-pub-crate`.

**Any restriction, not a list of two.** The arm read `pub`, `pub(crate)` and
`pub(super)`, and `pub(in crate::engine) fn` is as visible to a module under
`engine::topology` as `pub(super) fn` written in a child of `engine` is, so a
function spelled that way in an allowed, classified file would have stood
outside the domain: unclassified, undenied, and reachable. Found by reading
this function while closing `PR306-FACADE-INLINE-ESCAPE` on 2026-09-20, not by
a witness, and widened then: a `pub` followed by a parenthesised restriction of
any content counts, which admits `pub(self)` too and fails closed by doing so
-- a name the domain holds needlessly costs a row, and a name it misses costs
the guarantee. The widening added no name to any classified module at that
head; `the_reachable_fn_parser_finds_each_shape_this_tree_uses` pins both the
path form and a spaced one.

**`extern` is a modifier too.** The review of `409a6138` handed the widened
recogniser `pub(in crate::engine) extern "Rust" fn probe() {}` and got no
name: the ABI string is blanked with every other literal, which leaves
`extern` as the word before `fn`, and the list of modifiers stripped from the
end of the prefix did not hold it. It is stripped first now, being the last
modifier Rust's grammar admits before `fn`, and the shape test pins the path
form and `pub unsafe extern "C" fn`. This is a recogniser of the shapes the
tree and its reviews have produced, not of Rust's item grammar, and nothing
here should be read as saying otherwise.

## `pub fn reachable_fn_multiplicity(source: &str) -> BTreeMap<String, usize> {`

For each name [`externally_reachable_fns`] derives, how many `fn` declarations
of the file's production code bear it -- every one, whatever its own
visibility, wherever in the file it is written.

The record classifies by bare name, so a name is the whole of a callable's
identity there, and a denial names one path. One name borne by two callables
is one row answering for both. That is ordinary -- two `Display` impls, a
`#[cfg(unix)]` and a `#[cfg(windows)]` twin, a trait's declaration and its
impl -- and it is also how a function the source writes is added to a
classified module without the record changing: give it a name that is already
classified
(`PR309-INLINE-WRAPPER-NAME-COLLISION`). The census pins every count above one
in the record's `shared` table, so the second bearer of a name is a
disagreement like a new name is.

## `pub enum OwnerHeader {`

What the header of one pair of braces was read as, for
[`reachable_fn_owners`]: an inline `mod`, a `trait` declaration, a trait's
impl with the trait's name as the header writes it, an inherent impl, or
`Unread` -- everything else, which is most braces: a function body, a `const`
block, a `match`, a macro's definition or invocation, an `extern` block, and
any header the reading below cannot place. An inherent impl carries no type
name on purpose. Nothing downstream may compare two impls by what their
headers spell, and the way to be sure nothing does is for the spelling not to
be there.

## `pub struct OwnerScope {`

One pair of braces open at a declaration: the byte its `{` sits at in the
file's production code, and what its header was read as. **The offset is the
scope's identity.** Two scopes are the same scope when they open at the same
byte and at no other time, whatever their headers say, because a header is
text and text is what an alias, a `use` inside a block or a generic argument
changes without changing the path -- or changes the path without changing the
text.

## `pub fn reachable_fn_owners(source: &str) -> BTreeMap<String, Vec<Vec<OwnerScope>>> {`

For each name [`externally_reachable_fns`] derives, where each of its bearers
is declared: the chain of braces open at the `fn`, outermost first, the file's
top level being the empty chain. It exists for one question -- whether the
bearers of an effectful name are one path or several
(`PR309-SHARED-EFFECTFUL-PIN-CANNOT-RECORD-ITS-DENIAL`) -- and answers no
other. Each bearer owns its chain, so the open stack is cloned once per
reachable declaration; a chain is a handful of small values.

**What it returned before, and why it does not now.** Until the review of
`993f080d` a chain was a string of labels (`block > impl Rf3Target`) and the
caller compared strings. Both lenses of that review passed two receivers as
one with ordinary source-written Rust and no expansion. `impl RfFirst<{ 1 + 1 }>`
and `impl RfSecond<{ 1 + 1 }>`: the braces of the const argument are braces,
closing them restarted the header, what was left of each `impl` header was
`>`, and both owners read `block`. Two `const _: () = { .. }` blocks each
importing a different receiver as `Rf3Target`: both owners read
`block > impl Rf3Target`. In each case the record classified and denied the
first path, a production body under `engine::topology` called the second, and
clippy exited 0 with 186 tests passing. Writing `2` for `{ 1 + 1 }`, or
renaming one alias, made the same check fail -- so the verdict followed the
spelling and not the path. Every `{` still opens a scope here, a const
argument's included; what changed is that a scope is known by where it opens,
and a header that does not read as an item is `Unread` rather than something
two blocks can have in common.

**Parentheses and square brackets open scopes too, always `Unread`.** No item
is declared inside either in ordinary Rust, and the one thing that does put a
`fn` there is a macro invocation, `place!( pub fn held() {} );`, which can set
its tokens down anywhere. Found by attacking the first draft of this repair,
which tracked braces alone: twins inside one invocation's parentheses were
two bearers at the file's top level, one scope, admitted. A closer leaves only
the scope its own opener made, so a stray one cannot close an `impl`; and a
`;` ends a header only when the innermost thing open is a brace, which is what
lets `impl View for [u8; 4]` be read as the impl it is.

It is a lexical reading like every other in this file. It sees the
declarations the source writes, it does not evaluate `cfg`, and it resolves no
name.

## `fn owner_header(header: &str) -> OwnerHeader {`

The text between the last `;`, `{` or `}` and an opening brace, read as an
item header or not at all. Leading attributes are stepped over by matching
their brackets, a visibility is dropped, generic argument lists are dropped
(an `->` inside one does not close it), and then **the first word has to be
the keyword**, after at most one `unsafe`: `mod name`, `trait Name`, or
`impl`. An `impl` header is a trait's impl when the word
`for` is its second word after `impl` and the first is a plain identifier --
`impl View for A`, not `impl fmt::Display for A`, whose trait this reading
would have to resolve -- and an inherent impl when it holds no `for` at all.
Anything else is `Unread`.

The keyword has to lead because the earlier reading took the last of `trait`,
`impl` or `mod` found *anywhere* in the header, and answered `impl Sized)` for
`fn host(value: impl Sized) {` (measured at `993f080d`). That direction -- a
scope read as an item it is not -- is the one that could admit something, so
it is the strict one. The other direction is deliberately loose: a header this
cannot place is refused by the caller, so a brace inside the header
(`impl Wide<{ 1 + 1 }>`), a trait named by a path, an `impl const`, a
non-ASCII name and a negative impl all come back `Unread`, and the cost is
that the author of such a header gives the other callable its own name.

## `fn without_visibility(item: &str) -> &str {`

An item header with its `pub`, `pub(crate)` or `pub(in path)` removed. `pub`
has to end at whitespace or a parenthesis, so an item whose first word merely
begins with those letters keeps it and is read as what it is.

## `fn is_identifier(word: &str) -> bool {`

An ASCII identifier: the name a `mod`, a `trait` or the trait of an impl has
to be for the header to be placed. A raw or non-ASCII identifier is not one,
which leaves its header `Unread`.

## `fn keyword_sites<'a>(text: &'a str, keyword: &'a str) -> impl Iterator<Item = (usize, usize)> + 'a {`

Where `keyword` is written in `text` as a whole word -- no identifier byte
before it, no identifier character after it -- as `(start, end)`. That is all
rustc asks of a keyword, so it is all the `trait`, `impl` and `for` readers
ask; [`declared_fns`] goes on to want a separator after `fn`, because a name
has to follow and `fn(` is a type. One reading of "a keyword is written here"
for the four, where there were three and each wanted its own neighbour. A
non-ASCII character before the keyword is taken for a boundary, which
over-reads.

## `fn declared_fns(region: &str) -> Vec<(usize, &str)> {`

Every `fn name` a region declares, with its offset: the one reading the
domain, its multiplicity and its owners are all made from, so they cannot
disagree about what a declaration is.

**A name is read by what ends it, not by what it is made of.** After `fn` and
any run of separators, the name is the text up to the first `(`, `<` or
separator, less a leading `r#` -- a separator being one of
[`RUSTC_WHITESPACE`], which this reader names itself, so it reads `fn`, U+200E,
a name on text no tokenizer has rewritten as well (round 3 had widened it to
`char::is_whitespace`, which reads neither U+200E nor U+200F, and the review
of `84123789` executed that: see [`RUSTC_WHITESPACE`]). It used to be `fn`,
exactly one space, and a
run of ASCII letters, digits and underscores, and round 3 of #309 measured
what that left out at `993f080d`, each with the control `pub fn plain() {}`
read beside it: `pub fn /* between */ commented() {}` was unread, because the
blanker turns the comment into spaces and the first piece after `fn ` is then
empty; so was a name after a line break; so was a non-ASCII name; and
`pub fn r#raw() {}` was read as `r`. Each is a function the source writes and
the record never asked about, or asked about under another name, and the
count this file's sharing rule rests on was short by it. Executed as parser
omissions; the bypass each would allow is reasoned. The tree writes none of
them, and the fix moves nothing at that head: 759 names in the 54 classified
modules, 2267 across all 186 source files, byte-identical, and the 69 pins
still exact.

One thing that follows `fn` is deliberately not a name: a macro metavariable.
`pub fn $name() {}` names nothing the text holds, reading it as `$name` would
make expansion look covered when it is not
(`PR7-WRAPPERS-EMPTY-DOMAIN`), and `$` cannot begin an identifier, so the
exclusion costs no real name. `the_reachable_fn_parser_finds_each_shape_this_tree_uses`
pins all five.

## `fn find_header_brace(region: &str, from: usize) -> Option<usize> {`

The `{` that opens an `impl` block's body, skipping generics and where-clauses.

**A `;` or a `{` inside square brackets is not the header's end.** It used to
be: the scan counted angle brackets and parentheses and stopped at the first
`;` outside them, so `impl Trait for [u8; 4] { fn f(&self) {} }` gave no span,
`f` declares no visibility, and the method was outside the classification
domain; a public trait's default body returning `[u8; 4]` went the same way.
Found in round 3 of #309 while pinning what [`reachable_fn_owners`] reads, by
a shape test that came back empty: executed as a parser omission, the bypass
it would allow reasoned. Brackets are counted now. It adds no name at that
head -- 759 names in the 54 classified modules and 2267 across all 186 source
files, byte-identical with the hunk reverted -- and
`the_reachable_fn_parser_finds_each_shape_this_tree_uses` pins both shapes.
Like `declares_visibility` above, this is a recogniser of the shapes the tree
and its reviews have produced, not of Rust's item grammar.

## `pub struct DenialFixture {`

---------------------------------------------------------------------------
The four build-failure refusals whose reason must be pinned
---------------------------------------------------------------------------

## `pub struct DenialFixture {`

One shape `mechanism` (1) claims rustc resolution defeats, as a fixture.

`proof_tests[4]`: "injected renamed-import / re-export / function-value /
legacy-wrapper call fixtures fail the build". A fixture asserting "this does
not build" is green whether it failed for the intended reason or a typo, so
each row carries the lint it must emit **and** the resolved path clippy must
name — and the harness runs a control that must compile first.

## `pub struct DenialFixture` › `pub shape: &'static str,`

What the shape is called in `proof_tests[4]`.

## `pub struct DenialFixture` › `pub source: &'static str,`

The fixture body, compiled as its own crate against this crate's rlib.

## `pub struct DenialFixture` › `pub lint: &'static str,`

The lint the fixture must emit, and nothing else.

## `pub struct DenialFixture` › `pub resolves_to: &'static str,`

The path clippy's message must name — the *resolved* one, which is the
whole claim: a renamed import reports as `std::fs::write`, not as `w`.

## `pub const DENIAL_FIXTURES: &[DenialFixture] = &[`

The fixture set. One row per shape `proof_tests[4]` names, plus the two the
mechanism sentence names that the proof test does not (a method call and a
macro), because "aliases, re-exports, function values, method calls, and
macro-expanded code" is five shapes and a grid short of its domain is the
class this project has recorded four times.

## `pub const DENIAL_CONTROL: &str = "pub fn go(p: &std::path::Path) -> bool {\n\`

A fixture that must compile clean, so a mis-wired invocation cannot make
every refusal above "pass".

`PR5-C-DOCTEST-FIXTURES-NEVER-RAN` is in the standing ledger because three
build-refusal fixtures were green having never executed. The control is the
difference between "the compiler refused this" and "the compiler could not
find a crate to refuse it against".

## `pub(crate) mod census_domain {`

-- test-only declarations ----------------------------------------------
At the BOTTOM, and a `mod` rather than a bare `fn`: `production_region` cuts a
file at its first `#[cfg(test)]`, and
`effects::tests::every_production_region_that_stops_early_stops_at_a_module`
pins by name the ten files whose cut lands on something that is not a module.
This file is not one of them and must not become one.

## `pub(crate) mod census_domain {`

The **domain** every whole-tree census draws, derived once.

`PR5D-VISIBILITY-CHECK-DUPLICATED`: a value two places both maintain by hand
disagree eventually, and the one that disagrees silently is the one that
decides what a census is allowed to see. This derivation was written twice —
`runner::tests::whole_file_test_module_declarations` and
`events::log::tests::declared_whole_file_test_modules`, identical by hand,
each deciding which files four whole-tree censuses skip. It lives beside
[`production_code`] now, which is the region those same censuses count over.

## `pub(crate) mod census_domain` › `pub(crate) fn production_calls(code: &str, name: &str, form: Call) -> usize {`

Calls to `name` in `code`: neither its definition, nor a longer identifier
that merely ends in it.

The second half is the one that was missing. A needle built as
`format!("{name}(")` is a plain substring search, so `expected_refs(` is
satisfied by every `refuse_unexpected_refs(` in the tree — and a census whose
entry is proved by a different function's call sites proves nothing about its
own. Measured on this tree: `workspace_manager.rs` carries four occurrences of
the substring `expected_refs(` and **zero** calls to `expected_refs` — one of
the four survives into `production_code`'s region, and it is the *definition
line* of `refuse_unexpected_refs`, which the "calls, not definitions" filter
does not see because the text before the match is `pub fn refuse_un`.

The boundary is "the byte before the match is not an identifier byte", which
keeps `crate::a::b::expected_refs(` — `:` is not one — and rejects
`unexpected_refs(`. Not a rename, which is how
`the_barrier_is_the_only_topology_route_from_a_proven_prefix_to_an_append_handle`
closed the same class: that census's needle is a constant it could choose,
this one's is eleven names the packet chose.

## `pub(crate) fn production_calls(code: &str, name: &str, form: Call) -> usize {` › `.filter(|(at, _)| {`

Not the tail of a longer identifier.

## `pub(crate) fn production_calls(code: &str, name: &str, form: Call) -> usize {` › `.filter(|(at, _)| !code[..*at].trim_end().ends_with("fn"))`

Calls, not definitions.

## `pub(crate) fn production_calls(code: &str, name: &str, form: Call) -> usize {` › `.filter(|(at, _)| {`

And the form the clause is written in, which is what tells three
items of one name apart.

## `pub(crate) mod census_domain` › `pub(crate) enum Call {`

How a clause's function is **called** in production.

Not decoration. `settle_interrupted` names three unrelated production items
in this tree — `recover::settle_interrupted` (the `T-ATTEMPT` clause, a free
function), `AttemptContext::settle_interrupted` and
`events::RunState::settle_interrupted` (both methods, both called) — and a
census counting the bare name is satisfied by either of the other two.
Measured by S5 round 4: deleting step (d)'s only production call left the
census **and the entire suite** green, with `attempt_interrupted` appended
by no run. `reviews/FINDINGS.md` §4's "a refutation must name which item it
inspected" is the same rule; this is it applied to the instrument.

## `pub(crate) enum Call` › `Free,`

`name(…)` or `path::name(…)` — never `receiver.name(…)`.

## `pub(crate) enum Call` › `Method,`

`receiver.name(…)`.

## `pub(crate) mod census_domain` › `pub(crate) fn whole_file_test_modules(`

The **files** [`declared_whole_file_test_modules`] resolves to, as a set a
census can test membership in.

The resolution loop — assert exactly one of the two candidates exists,
collect it — was written out at each caller, and a third caller wrote a
different rule instead: `path.file_stem() == "tests"`. That covers the
files named `tests.rs` — the entries of
`effects::tests::cfg::WHOLE_FILE_TEST_MODULES` whose file stem is
`tests` — and **not** the six that are not: `scaffold`, `premove`,
`fake`, `fixture`, `scratch_tree` and `readiness`. The whole set, that
subset and the difference
between them are all read off that one list; the four the rule misses
are the ones a census is
most likely to trip over, because a scaffold, a fake and a readiness
protocol exist to *name* the things production names. Found by S5 round
5's `seams`, `attempt` and `settle` lenses independently; the
consolidation had been filed one commit earlier in
`reviews/FINDINGS.md` §20 as tidiness.

### Panics

When a declaration resolves to no file or to both candidates — a skip
path naming no file is a skip that has stopped meaning anything — when
two declarations resolve to one file, when the declaration graph is
cyclic, or when fewer than `floor` declarations are derived, which is
the control against a derivation that has silently stopped finding
anything.

## `pub(crate) mod census_domain` › `assert!(`

**The declaration graph is a forest.** Directory-derived candidates
descend, so a cycle is not reachable from this tree — which is the
reason to check rather than a reason not to: an unreachable path is
one nobody would notice becoming reachable. A `#[path]` attribute is
the one construct that could build one, and the scanner refuses those
rather than resolving them, so this assertion and that refusal are
one control with two halves.

## `pub(crate) mod census_domain` › `assert!(`

**The control that binds every caller**, and it belongs here rather
than at each of them.
`the_whole_file_test_modules_are_resolved_from_the_declarations_not_the_file_names`
asserts what this *returns*; it says nothing about whether a census
calls it, which is the defect `3a91626` repaired for two censuses and
this witness then reproduced one commit later (`R6-SETTLE-003`). A
caller cannot reach the set without passing through this line.

## `pub(crate) mod census_domain` › `pub(crate) fn declaration_cycle(edges: &[(PathBuf, PathBuf)]) -> Option<Vec<PathBuf>> {`

A cycle in `edges`, as the path that closes it, or `None`.

`edges` is (declaring file, declared file). The derivation treats that
relation as a forest — every guard is read from the file *above* — so a
cycle means the traversal would either not terminate or attribute a
guard to a file that does not inherit it.

Pure and separately driven, because the real tree cannot produce one: a
census control that is only ever exercised on input that satisfies it is
a control nobody has seen refuse anything.

## `pub(crate) fn declaration_cycle(edges: &[(PathBuf, PathBuf)]) -> Option<Vec<PathBuf>> {` › `enum Colour {`

Depth-first search state. `Grey` is "on the path being walked", and
reaching a `Grey` node is what a back edge *is*.

## `pub(crate) fn declaration_cycle(edges: &[(PathBuf, PathBuf)]) -> Option<Vec<PathBuf>> {` › `let mut adjacency: BTreeMap<&PathBuf, Vec<&PathBuf>> = BTreeMap::new();`

**The full adjacency, not the first edge out of each node.** The
first version followed `edges.iter().find(…)`, which walks one
outgoing edge per node — so a node with two children whose *second*
child closes the loop reported no cycle. `a -> b`, `a -> c`,
`c -> a` was the shape, and it read as acyclic.

## `pub(crate) fn declaration_cycle(edges: &[(PathBuf, PathBuf)]) -> Option<Vec<PathBuf>> {` › `let mut stack: Vec<(&PathBuf, usize)> = vec![(start, 0)];`

(node, how many of its outgoing edges have been taken). The stack
IS the current path, which is what makes the cycle reportable.

## `pub(crate) mod census_domain` › `pub(crate) fn sole_present<'a>(`

The one of `candidates` that `exists` accepts, or how many it accepted.

Zero is a declaration naming no file, which is a skip that has stopped
meaning anything. Two is `name.rs` and `name/mod.rs` both present, which
Rust itself refuses to compile and which a resolver that took the first
match would silently pick a side in. Both are refusals.

`exists` is a parameter rather than a `Path::is_file` call so the two
refusals can be driven: neither is reachable from this tree, and a
control that has only ever seen compliant input is a control nobody has
watched refuse anything. It is also what keeps this body free of an
effect — the funnel section of the allowlist records `allows = []` for
this file and that claim is stronger than any other entry there.

## `pub(crate) mod census_domain` › `pub(crate) enum CandidateRefusal {`

Why a declaration's candidate files cannot be named.

## `pub(crate) enum CandidateRefusal` › `OutsideThePackage {`

The declaring file is not inside the package the inventory was read
for, so that inventory does not say whether it is a crate root.

## `pub(crate) mod census_domain` › `pub(crate) enum InventoryRefusal {`

Why a package's target inventory could not be established.

Every variant is a **refusal to guess**. The resolution below turns on
which files Cargo compiles as crate roots, that is a fact only the
manifest holds, and the previous derivation held it as a rule about file
stems instead. A rule cannot be wrong quietly the way a stem test can:
when the authority is unavailable the census stops.

## `pub(crate) enum InventoryRefusal` › `NotRun {`

`cargo metadata` could not be started at all.

## `pub(crate) enum InventoryRefusal` › `Failed {`

It ran and exited non-zero.

## `pub(crate) enum InventoryRefusal` › `Unreadable {`

Its output is not the JSON document this reads.

## `pub(crate) enum InventoryRefusal` › `NoPackage {`

No package in the document has that manifest path.

## `pub(crate) enum InventoryRefusal` › `NoTargets {`

The package has no targets, so nothing is a crate root.

## `pub(crate) mod census_domain` › `pub(crate) struct CrateRoots {`

**The files Cargo compiles as crate roots**, read from the manifest via
`cargo metadata` rather than inferred from their names.

A crate root owns its own directory; every other file owns a directory
named after it. Which files are roots is a property of the *manifest*,
and the previous derivation decided it from the file's stem: `lib.rs` or
`main.rs` at the source root was a root, the same stem anywhere else was
refused, and anything else was an ordinary module. Both halves are wrong
against a manifest that says otherwise, and the second half is wrong
**silently**:

* `[[bin]] path = "src/tools/odd.rs"` is a crate root with an arbitrary
  name. The stem rule reads it as the ordinary module `tools::odd`, so a
  `mod helper;` inside it resolves to `src/tools/odd/helper.rs` when
  Cargo compiles `src/tools/helper.rs`. That is a **different file** —
  the same competing-sibling hazard the nested-`lib.rs` refusal was
  written for, arriving through the door that refusal left open, and it
  does not announce itself: with no `src/tools/odd/helper.rs` present the
  wrong reading resolves rather than refusing.
* `examples/probe.rs` is this tree's live instance. It is an `example`
  target — a crate root — and `effects::tests::scanned_sources` walks
  `examples/**`, so the stem rule already answers `examples/probe` for a
  directory Cargo calls `examples`.
* A nested `src/a/lib.rs` the manifest never names is the ordinary module
  `a::lib`, which is decidable rather than ambiguous once the manifest is
  read. The old refusal was the honest answer to not knowing; this is the
  answer.

Kinds are **not** filtered. `lib`, `bin`, `example`, `test`, `bench` and
`custom-build` are each a crate root of their own, and a census that
looked only at `lib`/`bin` would re-introduce the same class one kind at
a time.

## `impl CrateRoots` › `pub(crate) fn from_metadata_json(`

The inventory in one `cargo metadata --format-version 1` document,
for the package whose manifest is `manifest`.

Pure over the document, which is what makes every refusal below
drivable: the acquisition is a process start and lives in
[`crate::effects::tests`], where this crate's governance puts one.

## `impl CrateRoots` › `pub(crate) fn package_dir(&self) -> &std::path::Path {`

The directory the package's manifest sits in.

## `impl CrateRoots` › `pub(crate) fn roots(&self) -> impl Iterator<Item = &std::path::Path> {`

Every crate root, absolute, in sorted order.

## `impl CrateRoots` › `pub(crate) fn is_root(&self, path: &std::path::Path) -> bool {`

Whether `path` is one of them.

## `impl CrateRoots` › `pub(crate) fn is_root_relative(&self, relative: &str) -> bool {`

Whether the package-relative, `/`-separated `relative` is one.

The second caller reads the tree as repo-relative slash strings
rather than as paths, and one authority answering both is the point:
`PR5D-VISIBILITY-CHECK-DUPLICATED` is the standing entry for the rule
that got written twice, and the stem test *was* written twice — here
and in `effects::tests::cfg::module_dir`.

## `pub(crate) mod census_domain` › `pub(crate) fn module_directory(`

The directory an out-of-line child of `declared_in` lives in.

**A crate root owns its directory; an ordinary module owns a directory
named after it.** `mod.rs` is the first case wherever it sits — that is
what `mod.rs` means. Everything else is the first case exactly when the
manifest names it as a target's path, which is what [`CrateRoots`] reads
and what no rule about file names can answer.

Refused rather than decided when `declared_in` is not inside the package
the inventory was read for: an inventory is a statement about one
package, and a file outside it is one the inventory is silent on.

## `pub(crate) mod census_domain` › `pub(crate) fn candidates_for(`

The two files `mod <name>;` can name, given where it was written.

`declared_in` is the declaring file; `inline_path` is the inline modules
enclosing the declaration, outermost first. **The inline path is part of
the directory**, which is the half a resolver reading only the file name
gets wrong: `mod readiness;` inside `proc.rs`'s inline `test_support`
names `proc/test_support/readiness.rs`, and flattened to
`proc/readiness.rs` it names nothing — a zero-candidate refusal if you
are lucky and the wrong file if you are not.

`roots` is the package's target inventory, and [`module_directory`] is
why that is a parameter rather than a test on the file's stem.

## `pub(crate) mod census_domain` › `pub(crate) fn contained_in(base: &std::path::Path, candidate: &std::path::Path) -> bool {`

Whether `candidate` stays inside `base` through plain path components.

A module name is an identifier and a candidate is `base` joined with
identifiers, so this holds by construction — and is asserted anyway,
because the construction is what a `#[path = "../.."]` attribute would
change, and the failure it would cause is a census reading a file
outside the tree as declared inside it.

## `pub(crate) mod census_domain` › `pub(crate) struct TestModuleDeclaration {`

One out-of-line `mod <name>;` the crate declares as test-only.

## `pub(crate) struct TestModuleDeclaration` › `pub(crate) declared_in: PathBuf,`

The file the declaration is written in.

## `pub(crate) struct TestModuleDeclaration` › `pub(crate) name: String,`

The declared module's name.

## `pub(crate) struct TestModuleDeclaration` › `pub(crate) inline_path: Vec<String>,`

The **inline** modules enclosing the declaration, outermost first.
Empty when the declaration sits at the file's top level.

## `pub(crate) struct TestModuleDeclaration` › `pub(crate) guard: String,`

The effective `cfg` predicate, rendered — the conjunction of every
enclosing inline module's predicate and the declaration's own.

## `pub(crate) struct TestModuleDeclaration` › `pub(crate) candidates: [PathBuf; 2],`

`[<dir>/<name>.rs, <dir>/<name>/mod.rs]`, where `<dir>` is the
declaring file's module directory joined with [`Self::inline_path`].

## `impl TestModuleDeclaration` › `fn render_guard(&self) -> String {`

The guard and the inline path it was read through, for a diagnostic.

## `pub(crate) mod census_domain` › `pub(crate) fn declared_whole_file_test_modules(`

Every out-of-line module declaration the crate compiles **only** under
`cfg(test)`, structurally resolved.

Such a file is test code end to end. A region function has nothing to
remove in one, so it would count the whole of it as production — a
fixture that names a census's needle would then read as a production
offender. The set is read out of the declarations rather than listed by
hand: it was `src/engine/tests.rs` alone until PR5 moved the Event funnel
into `src/events/log.rs` with two test modules of its own, and the census
failed on the first file the hand-maintained list did not know about.

**Read out of the blanked source, and every candidate returned rather
than assumed.** The split used to be over the raw text, so a `//` line
containing `#[cfg(test)] mod policy;` derived a skip for
`src/runner/policy.rs` and removed that file from every census below —
measured, with a `git push` planted in it that the census then did not
see. Over the whole tree the raw split derived 50 skip paths of which
**34 named no file at all**, and a skip path naming no file is a skip
that has stopped meaning anything, so [`whole_file_test_modules`] asserts
that exactly one of the two candidates exists.

### Structure, not a literal `#[cfg(test)] mod name;`

The predicate used to be exactly that string, and it had two holes a
**text** rule cannot close and a structural one closes together:

* **A visibility qualifier hid the declaration.** `#[cfg(test)]
  pub(crate) mod helpers;` was not matched, because the rule read `mod `
  immediately after the attribute. That direction was chosen as the safe
  one — failing to derive a skip leaves a test file in a census's domain,
  where a fixture reads as an offender and someone looks — and it is
  still the safe direction. It stopped being *necessary*: the scan below
  reads the item, so a qualifier is transparent rather than fatal.
* **An inline ancestor carried the guard.** `#[cfg(test)] mod
  test_support { … mod readiness; }` compiles `readiness.rs` only under
  `cfg(test)`, and the declaration inside carries no attribute at all.
  `src/agent/proc/test_support/readiness.rs` is that file; without the
  ancestry it is a whole test file with no `#[cfg(test)]` anywhere in it,
  which is precisely the shape every census here exists to skip.

So the scan walks each file's **module structure**: brace depth, the
inline modules open at each point, and the `cfg` predicates on each of
them. A declaration is test-only when the conjunction of its own
predicate and every enclosing inline module's predicate is false
wherever `test` is false — [`entails_test`].

### What it deliberately does not do

**No transitive closure over files.** `src/effects/tests.rs` is itself a
whole-file test module and declares `mod policy;`, so Rust compiles
`src/effects/tests/policy.rs` only under `cfg(test)` too — and this
derivation does not say so. Every census in this crate reads
[`super::production_code`], which removes `#[cfg(test)]` items from the
files it keeps, and those second-level files carry their own inline
`cfg(test)` modules and their own `#![deny]` prologues for exactly that
reason (`effects/tests/classification.rs` and its siblings say so at
length). Closing over the file graph would widen the skip set by a dozen
files whose contents no census has been measured against, which is a
change to what every census can see and not a bug fix. The measured
domain is the set
`the_whole_file_test_modules_are_resolved_from_the_declarations_not_the_file_names`
names — the literal `#[cfg(test)] mod tests;` declarations, plus
`scaffold`, `premove`, `fake`, `fixture`, `scratch_tree` and
`readiness` — and it is listed, path
by path, in `effects::tests::cfg::WHOLE_FILE_TEST_MODULES`. One more
arrives with the slice that adds it, and that slice adds its path to
that list: the population lives in one place, so adding a module is one
edit rather than a sweep over every comment that had restated its size.

**No `#[path]`.** A `#[path]` attribute on a module is refused rather
than resolved: it is the one construct that can point a declaration
outside its own directory, and there are none in this tree.

**No `cfg_attr` that applies a `cfg`, and this one is a hole rather
than a choice.** [`scan_module_declarations`] treats a `cfg_attr` as
significant only when it contains `path`, so `#[cfg_attr(all(),
cfg(test))] mod hidden_tests;` — which rustc applies as `#[cfg(test)]`
and compiles only under test — is read here as an unconditional
declaration, and the file it names stays in every census's domain as
production. There is no such declaration in this tree; the form is
stated because nothing in the crate would notice one. Measured by
writing one and reverting it: the module's own `#[test]` ran, so rustc
had applied the `cfg(test)`, while
`the_whole_file_test_modules_are_resolved_from_the_declarations_not_the_file_names`
stayed green with the file outside the population it resolves. It is
invisible to every reading of this derivation at once,
`whole_file_test_modules` included, and to the hand-maintained
`effects::tests::cfg::WHOLE_FILE_TEST_MODULES` that pins the result, so
no assertion over the population can be the thing that catches it. PR
#101's reviewer found it, it predates that change, and widening the
scan to decide `cfg_attr` predicates is its own change with its own
review.

### Panics

When a file cannot be read structurally at all — an attribute that never
closes, a brace that closes one too many, a `mod` with no name or no
terminator, a `cfg` predicate the entailment grammar cannot read, a
`#[path]`, or one name declared twice in one module. Every one of those
means the scan does not know what the file declares, and a scan that
does not know must not answer.

## `pub(crate) mod census_domain` › `assert!(`

**The inventory has to describe the tree being walked.** `source_root`
is the caller's claim about where the crate's sources live, and the
manifest's is the target paths; a `source_root` no target sits under
means the two are about different trees, and every answer below would
be resolved against an inventory that says nothing about the files in
hand. Fail closed, in the same breath as the acquisition itself.

## `pub(crate) mod census_domain` › `pub(crate) struct ScannedDeclaration {`

One `mod` declaration as the scan read it out of a file's structure.

## `pub(crate) struct ScannedDeclaration` › `pub(crate) name: String,`

The declared module's name.

## `pub(crate) struct ScannedDeclaration` › `pub(crate) inline_path: Vec<String>,`

The inline modules enclosing it, outermost first.

## `pub(crate) struct ScannedDeclaration` › `pub(crate) guard: String,`

The effective predicate, rendered.

## `pub(crate) struct ScannedDeclaration` › `pub(crate) test_only: bool,`

Whether that predicate is false wherever `test` is false.

## `pub(crate) mod census_domain` › `pub(crate) struct ScannedInlineModule {`

One inline `mod name { … }`, at whatever depth it is written.

[`ScannedDeclaration`] is a module that has a file. This is the other kind, and
until 2026-09-20 the scan recorded nothing for it: the inline branch opened a
scope, so that a declaration *inside* it carried the right `inline_path`, and
moved on. No census could judge a module the scan never reported, and the
fourth review of #306 (`PR306-FACADE-INLINE-ESCAPE`) used exactly that -- an
attribute-free inline module in `src/engine/mod.rs`, under that file's allow,
walked by no guard. An inline module has no file and no allowlist row; it
inherits the level of the file it is written in and can write attributes of
its own, outside its braces or inside them, so a census that wants to answer
for it needs both.

## `pub(crate) struct ScannedInlineModule` › `pub(crate) inline_path: Vec<String>,`

The inline modules enclosing this one, outermost first; empty at the top level
of the file. Its length is the depth.

## `pub(crate) struct ScannedInlineModule` › `pub(crate) guard: String,`

The effective `cfg` predicate, as [`ScannedDeclaration::guard`] renders it:
what the module inherits from the inline modules around it and what is written
on it.

## `pub(crate) struct ScannedInlineModule` › `pub(crate) outer_attributes: String,`

The run of outer attributes written directly above the item, verbatim from the
raw source, comments between them included; empty when the item has none. The
run starts at the first `#[` since the last token that was not an attribute,
so an attribute on a neighbouring item is never handed to this one --
`the_module_scan_reports_inline_modules_at_every_depth_with_what_they_write`
pins that against an attributed item and an attributed declaration directly
above an attribute-free inline module.

## `pub(crate) struct ScannedInlineModule` › `pub(crate) body: String,`

The text between the braces, verbatim. Inner attributes are the head of it,
which [`super::lint_levels::leading_inner_attributes`] reads. A body that never
closes runs to the end of the file rather than refusing, so that
[`scan_module_declarations`] refuses exactly what it refused before this
record existed.

## `pub(crate) mod census_domain` › `pub(crate) struct ScannedModules {`

Both halves of one scan: the declarations that name a file, and the inline
modules that do not.

## `pub(crate) mod census_domain` › `pub(crate) enum ScanRefusal {`

Why a file's structure could not be read, and where.

Every variant is a refusal rather than a guess. The direction is the one
[`declared_whole_file_test_modules`] argues for: a scan that cannot tell
what a file declares must not answer, because both wrong answers are
silent — a missing skip reports a fixture as an offender, and a spurious
one removes a production file from every census below.

## `pub(crate) enum ScanRefusal` › `UnclosedAttribute {`

`#[…` with no `]`.

## `pub(crate) enum ScanRefusal` › `UnbalancedBraces {`

A `}` with no `{`.

## `pub(crate) enum ScanRefusal` › `MalformedDeclaration {`

`mod` with no name, or a name followed by neither `;` nor `{`.

## `pub(crate) enum ScanRefusal` › `UnreadablePredicate {`

A `cfg` predicate the entailment grammar cannot read, written directly or
applied through a `cfg_attr` (the `cfg_attr`'s own predicate included), or a
`cfg`/`cfg_attr` attribute whose comments and literals do not read the way
the blanked text does ([`with_literal_identity`] answers `None`).

## `pub(crate) enum ScanRefusal` › `UnsupportedPathAttribute {`

`#[path = "…"]`, or a `cfg_attr` that could apply one.

## `pub(crate) enum ScanRefusal` › `UnsupportedInnerCfg {`

An inner `#![cfg(…)]`, or an inner `#![cfg_attr(…)]` that can apply one,
which gates the module it is written in.

## `pub(crate) enum ScanRefusal` › `DuplicateDeclaration {`

One module name declared twice in one module.

## `pub(crate) enum ScanRefusal` › `ModuleShapedMacroBody {`

A macro body holding a module-shaped token sequence.

## `pub(crate) mod census_domain` › `pub(crate) fn scan_module_declarations(`

Every `mod` declaration in `source`, with the inline modules enclosing it
and the effective `cfg` predicate it inherits.

The out-of-line half of [`scan_modules`], and nothing else. The branch that
emits a declaration is the one it always was; what holds that is the scan
tests written before inline modules were reported, which pass unedited -- not
the comparison of this entry point with [`scan_modules`] in
`the_module_scan_reports_inline_modules_at_every_depth_with_what_they_write`,
which compares a function with its own delegate and pins only the adapter.

## `pub(crate) mod census_domain` › `pub(crate) fn scan_modules(source: &str) -> Result<ScannedModules, ScanRefusal> {`

The scan itself: one pass, both kinds of module.

Pure over `&str`, which is what makes the refusals above drivable: the
tree satisfies every one of them, so the only way to see one is to hand
this a source that does not.

Comments and string literals are blanked first —
[`super::blank_comments_and_strings`], which also handles raw strings,
byte strings and char literals — so a `mod` written in prose is spaces.
A `cfg` or `cfg_attr` attribute's text is then read through
[`with_literal_identity`] at the same offsets: comments blanked, and every
string literal kept as a token that names it exactly. Blanking alone erased
the value, so `feature = "x"` would arrive as `feature = "   "`; reading the
raw span instead, as this did until #318's fourth round, handed the grammar
string contents it split as structure -- `all(feature = "a\"", test,
feature = "b\"")` has `test` at top level, and the quote-toggling splitter
saw one atom and called a test-only declaration production.

## `pub(crate) mod census_domain` › `struct Scope {`

An inline `mod name { … }` that is open at the current position.

## `struct Scope` › `open_depth: usize,`

The brace depth *outside* the module's body.

## `pub(crate) mod census_domain` › `if byte == b'#' {`

-- an attribute, which belongs to whatever item comes next -----

## `pub(crate) mod census_domain` › `"cfg" | "cfg_attr" => {`

A `cfg` gates the item that follows, and so does a `cfg` that a `cfg_attr`
applies: `#[cfg_attr(P, cfg(Q))] mod x;` is compiled where `P` fails or `Q`
holds, so it is pushed as `any(not(P), Q)`, and a nested `cfg_attr` conjoins
its predicates into `P`. This scan read only the literal `cfg` until #318's
fourth round, so `#[cfg_attr(not(test), cfg(test))] mod x;` -- a file no
production build compiles -- was a production declaration, and
[`declared_whole_file_test_modules`] left it out of the population every
production census skips. The expansion is [`super::lint_levels::applied_attributes`],
the one reading of `cfg_attr` the crate has; a predicate on the way to a
generated `cfg` that the grammar cannot read is refused rather than dropped,
and an inner `cfg_attr` that can apply a `cfg` is refused like an inner `cfg`.
A `cfg_attr` that applies no `cfg` gates nothing and is passed over, as it
always was.

## `pub(crate) mod census_domain` › `"path" => pending_path = true,`

`path` names the file directly; `cfg_attr` can apply one
conditionally. Both are refused where they could reach a
module, which is decided when the item is read.

The attribute's **name** is read from the tokenizer's text and its `cfg`
predicate from [`with_literal_identity`]'s reading of the source, which keeps
the string values the tokenizer blanks. The name used to come from the source too, through `trim_start`, which
does not read U+200E or U+200F: `#[`, U+200E, `path = ".."]` was then an
attribute with no name, the refusal never fired, and the walk went on to the
file the declaration's own name resolves to while rustc compiled the one the
attribute names. Found by reading in round 4 of #309 while settling the
separators ([`RUSTC_WHITESPACE`]), not by a compiled witness;
`every_separator_rustc_reads_is_one_every_reader_here_reads` holds the refusal
for all eleven.

Two more readings of the same attribute were the walk's own until the
attribute-token repair of 2026-09-26. Its `#`, `!` and `[` had to touch, so
`#`, a space or U+200E, `[path = ".."] mod y;` was a plain `mod y;` to the
walk, where the joined spelling is refused (the fifth review of #309 executed
that at the reader); they are now found by [`super::attribute_open`], past
whitespace and plain comments, as rustc finds them. And the name was read as
the run of identifier characters it starts with, so `#[r#path = ".."]` named
`r`: rustc reads a raw name as the name it spells and compiled the file the
attribute names (measured, rustc 1.97.1). The name now comes from
[`super::lint_levels::attribute_name`], which reads `r#path` as `path`, as it
reads `r#cfg` and `r#cfg_attr` for the prologue reader.
`the_module_walk_reads_an_attribute_whatever_rustc_reads_between_its_tokens`
holds a `path`, a `cfg_attr` path, a `cfg` gate and an inner `cfg` for every
separator and both raw names, in the real `src/engine/attempt.rs`, with the
joined spellings as the control and a doc comment or a string between the
tokens as no attribute.

## `pub(crate) mod census_domain` › `if let Some(invocation) = macro_at(bytes, i) {`

-- a macro, whose body is token trees and not items ------------

`mod x;` inside `macro_rules! m { () => { mod x; } }` is not a
declaration, and `#[cfg(test)] mod x;` inside one is not a
test-only declaration: the tokens are only *shaped* like an item
until something expands them. Walking into a macro body therefore
invents declarations, which is the direction that removes a real
production file from every census below.

A macro invoked at item position **can** expand to a module,
though, and this scan cannot tell which does. So the body is
discarded when it holds nothing module-shaped and refused when it
does: the discard is what stops the false positives, and the
refusal is what stops the discard from becoming a blind spot.
Measured on this tree: zero macro bodies hold one.

## `pub(crate) mod census_domain` › `pending.clear();`

Attributes stacked above a macro invocation belong to it.

## `pub(crate) mod census_domain` › `if let Some(shape) = module_at(bytes, i) {`

-- a `mod` item, with any visibility qualifier in front of it ---

## `pub(crate) mod census_domain` › `i = bytes[name_at..]`

Past the `;`.

## `pub(crate) mod census_domain` › `pending.clear();`

-- anything else: the attributes above it are not a module's ---

## `pub(crate) mod census_domain` › `i = word(bytes, i).end;`

**Past the whole token, `r#` included.** This advanced by
`is_ident_byte`, and a raw identifier is not a run of
identifier bytes: `r#mod` is `r`, a `#`, and `mod`. So the
scan consumed the `r`, met the `#`, stepped over it as a
non-attribute byte, and then read the *inside* of the token
as though it stood at item position. `let r#mod = 1;` — valid
Rust — became `mod = 1;`, a `mod` item with no name, and the
whole file was refused; `use std::r#mod as tests;` inside a
`#[cfg(test)]` module became `mod as;`, a test-only
declaration the crate never wrote, whose skip names a file
that does not exist. [`word`] is the token this scan reads
everywhere else, and the fallback reads it too now.

## `pub(crate) mod census_domain` › `struct MacroInvocation {`

A macro invocation or `macro_rules!` definition and its delimited body.

## `struct MacroInvocation` › `name: String,`

The macro's name, for the diagnostic.

## `struct MacroInvocation` › `open: usize,`

The index of the body's opening delimiter.

## `struct MacroInvocation` › `close: usize,`

The index of the matching closing delimiter.

## `pub(crate) mod census_domain` › `fn macro_at(bytes: &[u8], at: usize) -> Option<MacroInvocation> {`

[`MacroInvocation`] beginning at `at`, or `None`.

The shape is an identifier, `!`, an optional second identifier — that is
`macro_rules! name { … }`, the one form that has one — and a delimited
group. Requiring the group is what keeps `a != b` out: after that `!`
comes `=`, which opens nothing. Requiring the second identifier only
after `macro_rules` is what keeps `if !condition { … }` out, which
otherwise reads as an invocation of `if` whose body is the block.

## `fn macro_at(bytes: &[u8], at: usize) -> Option<MacroInvocation> {` › `if !name.raw && is_keyword(name.text) {`

**A keyword before a `!` is unary negation, not a macro name.**
`if !(cond)`, `while !(cond)`, `return !(x)` are identifier, `!`,
delimited group -- the same three tokens as `foo!(…)` -- so reading
them as macros skips the grouped expression, and a `mod` written
inside it (`if !({ mod local {} true })` is valid Rust) then reads as
a module-shaped macro body and refuses the whole file. A macro's path
segment cannot be a keyword unless it is written raw, and `r#if!(…)`
is a macro called `if`, so the test is on the plain spelling only.

## `fn macro_at(bytes: &[u8], at: usize) -> Option<MacroInvocation> {` › `let bang = whitespace(bytes, after_name);`

**Whitespace and comments may sit between the name and its `!`.**
`macro_rules ! m { … }` and `quote /* why */ ! { … }` are both valid
Rust, and `#[rustfmt::skip]` keeps either spelling in a real file —
so requiring the `!` to be the very next byte made the guard miss
exactly the macros somebody had gone out of their way to space out.
Comments are already spaces in the view this reads, so one skip
covers both.

## `fn macro_at(bytes: &[u8], at: usize) -> Option<MacroInvocation> {` › `if !name.raw && name.text == b"macro_rules" {`

`macro_rules! name { … }` is the **only** form carrying a name
between the `!` and the body, and reading one for every macro is
what would make `if !condition { … }` an invocation of `if` once the
gap above is allowed: identifier, `!`, identifier, delimiter — and
the whole block would be skipped. Keyed on the one name that has it.

## `fn macro_at(bytes: &[u8], at: usize) -> Option<MacroInvocation> {` › `let defined = word(bytes, cursor);`

The defined name may itself be raw -- `macro_rules! r#mod { … }`
is how a macro takes a keyword for a name.

## `pub(crate) mod census_domain` › `pub(crate) struct OutsideInvocation {`

A macro invocation written outside every function body: the 1-based line
of its `!` and the name before it, for the diagnostic.

## `pub(crate) mod census_domain` › `pub(crate) fn macro_invocations_outside_function_bodies(`

Every macro invocation in `source`'s production code that is not inside a
function body. Added on 2026-09-26 (PR #325) for the expansion half of
`PR7-WRAPPERS-EMPTY-DOMAIN`, and read by
`effects::tests::every_macro_invocation_where_a_governed_lint_is_not_forbidden_is_inside_a_function_body`.

**Why the line is the function body.** What a macro invocation expands to
is in no text a reader here reads, so a census cannot follow it; what a
census can do is refuse the positions where what it expands to would
matter. Inside a function body, an item a macro writes is local to that
body: nothing outside can name it, and the two ways it can reach out by
name -- an `impl` of a type or trait declared outside the body, and a
`#[macro_export]` macro -- are `non_local_definitions`, which the crate
roots forbid. Outside a function body, what it writes is an item of the
module, the `impl`, the `trait` or the `extern` block it sits in, or of a
`const` or `static` initializer, and in a module-level `const _`
initializer rustc exempts even a non-local `impl` from the lint (measured,
`effects::tests::every_crate_root_forbids_non_local_definitions`). So the
reading is one question, is this `!` inside a function body, rather than
a list of the positions outside one: a list is a recogniser of the
positions someone thought of, and the brace test holds for the ones they
did not. An attribute's arguments are outside too, deliberately: a
derive's helper attribute hands its tokens to code in a dependency, which
decides where they expand.

What a function body does not stop is an item inside it that is reached
by symbol rather than by name: a `#[unsafe(no_mangle)]` function a macro
writes into an existing body is callable from any module that declares it
in an `unsafe extern` block, and the lint does not apply to it. That route
is recorded, executed, in `PR7-WRAPPERS-EMPTY-DOMAIN`'s Remaining.

Production code only: `production_code` blanks every `#[cfg(test)]`
item first, and its exact-spelling match can only leave test code in,
which refuses more rather than less.

## `pub(crate) mod census_domain` › `fn is_identifier_byte(byte: u8) -> bool {`

ASCII alphanumerics, `_`, and every non-ASCII byte. In the blanked view a
byte above 0x7F outside a comment or a literal is part of an identifier:
rustc's six non-ASCII separators are already spaces there
(`RUSTC_WHITESPACE`), and any other non-ASCII character outside an
identifier is a lexer error. Reading them as identifier bytes is what
lets `\u{e9}!()` read as an invocation and `fn \u{e9}()` as a header;
`is_ident_byte`, which the module walk uses, stops at them.

## `pub(crate) mod census_domain` › `fn identifier_end(bytes: &[u8], from: usize) -> usize {`

One past the last identifier byte from `from`.

## `pub(crate) mod census_domain` › `fn raw_prefix_before(bytes: &[u8], start: usize) -> bool {`

Whether the identifier starting at `start` is written raw: `r#`
immediately before it, and no identifier byte before the `r`.

## `pub(crate) mod census_domain` › `fn token_end(bytes: &[u8], from: usize) -> usize {`

One past the identifier at `from`, reading an `r#` prefix as part of it.

## `pub(crate) mod census_domain` › `fn function_bodies(bytes: &[u8]) -> Vec<(usize, usize)> {`

The brace pair of every function body: every non-raw `fn` followed by
whitespace and an identifier, so an item or an associated or foreign
function, whatever qualifiers and visibility precede it, and not a
function-pointer type, where `(` follows `fn`. A declaration without a
body contributes nothing. A function nested in another's body, or
written inside a `const _`, is found the same way, and its body is inside
the answer on its own.

## `pub(crate) mod census_domain` › `fn body_brace(bytes: &[u8], from: usize) -> Option<usize> {`

The opening brace of the body of the header that starts at `from`, or
`None` for a declaration.

A header holds braces that are not the body: a const block in a generic
argument or default (`Foo<{ 1 }>`, `<const N: usize = { 1 }>`) and, in a
parameter or return type, an array length (`[u8; { 1 }]`). Parenthesised
and bracketed groups are skipped whole, and `<` and `>` are counted --
except the `>` of `->`, the only other `>` a header writes outside a
group -- so the body is the first `{` at angle depth zero. A const block
inside angle brackets is skipped whole too, so a `<` or `>` in it counts
for nothing. A `;` or an unmatched closer at depth zero ends a header that
has no body. Reading a const block as the body would put a real body's
macros outside and a header's inside -- the two readings the fixtures
`a header holding a const block` and `a const-generic default` pin.

## `pub(crate) mod census_domain` › `fn macro_bangs(bytes: &[u8]) -> Vec<(usize, String)> {`

Every macro invocation's `!` and the name before it.

Read from the `!` rather than from the name: a `!` that is not `!=`,
whose preceding token is an identifier -- not a keyword unless raw, not
a number -- and whose next token is a delimiter, or an identifier and
then a delimiter (`macro_rules! name { .. }`). The keyword test is what
keeps unary negation out -- `if !(x)`, `return !(x)`, `while !done {` --
and, because no other identifier stands before a `!` in valid Rust, it
also lets the second identifier be read after any name rather than after
`macro_rules` alone: `if !condition {` is refused by the keyword, not by
the name. A path's last segment is the name (`std::thread_local!`), a
comment or a line break between the tokens is a space in this view, and
an alias is an invocation like any other, so `use std::include as rd;`
then `rd!(..)` reads as `rd`.

## `pub(crate) mod census_domain` › `fn module_shaped_between(bytes: &[u8], from: usize, to: usize) -> Option<usize> {`

Where a module-shaped token sequence starts inside `from..to`, if any.

"Module-shaped" is the word `mod`, a name, and a `;` or `{` — the same
three tokens [`module_at`] reads, minus the visibility prefix, because
what matters here is only whether the body *could* expand to a module.

## `fn module_shaped_between(bytes: &[u8], from: usize, to: usize) -> Option<usize> {` › `let declared = word(bytes, name_at);`

The name may be raw: `mod r#type;` inside a macro body is
as module-shaped as `mod tests;` is.

## `pub(crate) mod census_domain` › `fn identifier(bytes: &[u8], from: usize) -> (usize, &[u8]) {`

The identifier at `from`, and where it ends. Empty when there is none.

## `pub(crate) mod census_domain` › `struct Word<'a> {`

One identifier token, raw or plain.

## `struct Word<'a>` › `end: usize,`

Where the token ends, `r#` included.

## `struct Word<'a>` › `raw: bool,`

Whether it was written `r#name`.

## `struct Word<'a>` › `text: &'a [u8],`

The name, without any `r#`.

## `pub(crate) mod census_domain` › `fn word(bytes: &[u8], from: usize) -> Word<'_> {`

The identifier token at `from`, reading `r#name` as one token.

**A raw identifier is one token and its name may be a keyword.** That is
the whole reason this exists: `mod r#type;` declares a module called
`type`, and a reader that stopped at the `#` saw `mod r` followed by
something that is not a terminator and refused the file. `raw` is an
ordinary identifier that merely begins with the same letter, so the
prefix counts only when a `#` and an identifier byte follow it.

## `pub(crate) mod census_domain` › `const KEYWORDS: &[&[u8]] = &[`

Rust's keywords, strict and reserved.

**A keyword cannot be a macro's path segment**, and that is the only
structural thing separating `if !(…)` from `foo!(…)`: both are an
identifier, a `!` and a delimited group. Written raw it can --
`r#if!(…)` is a macro named `if` -- which is why [`Word`] carries that
bit rather than only the text.

## `pub(crate) mod census_domain` › `fn is_keyword(text: &[u8]) -> bool {`

Whether `text` is a Rust keyword written plainly.

## `pub(crate) mod census_domain` › `fn whitespace(bytes: &[u8], from: usize) -> usize {`

The first non-whitespace index at or after `from`. ASCII whitespace is the
whole of it **because `bytes` is the tokenizer's text**, where every other
separator rustc reads is already spaces ([`RUSTC_WHITESPACE`]). At `84123789`
the tokenizer's text still held them as written, this stopped at U+000B and
at the five that are not ASCII, and that is how `mod`, U+200E, a name went
unread.

## `pub(crate) mod census_domain` › `struct ModuleShape {`

A `mod` item beginning at `at`, past any visibility qualifier.

## `struct ModuleShape` › `name_at: usize,`

Where the module's name starts.

## `struct ModuleShape` › `body: Option<usize>,`

The index of the body's `{`, or `None` for `mod name;`.

## `pub(crate) mod census_domain` › `fn module_at(bytes: &[u8], at: usize) -> Option<ModuleShape> {`

[`ModuleShape`] at `at`, or `None` when this is not a `mod` item.

`pub`, `pub(crate)`, `pub(super)` and `pub(in a::b)` are transparent:
they are read and stepped over rather than treated as the start of some
other item, which is the whole of what "visibility-qualified declaration"
costs a structural scan. A text rule keyed on `mod ` immediately after
the attribute could not do it, and that is the hole this closes.

## `fn module_at(bytes: &[u8], at: usize) -> Option<ModuleShape>` › `let mut token = word(bytes, at);`

Raw-aware throughout: `r#pub` and `r#mod` are identifiers named for
keywords, not the keywords, and neither opens a module item.

## `fn module_at(bytes: &[u8], at: usize) -> Option<ModuleShape>` › `let after_keyword = whitespace(bytes, token.end);`

`mod` and the name must be separated: `models` is not `mod els`.

## `fn module_at(bytes: &[u8], at: usize) -> Option<ModuleShape>` › `let declared = word(bytes, after_keyword);`

The declared name is the identifier without its `r#`: `mod r#type;`
names `type.rs`, the way rustc resolves it.

## `fn module_at(bytes: &[u8], at: usize) -> Option<ModuleShape>` › `_ => Some(ModuleShape {`

A name with neither terminator is malformed, and the caller
refuses it. Reported through an empty-bodied shape so the caller
sees the position rather than silently skipping the item.

## `pub(crate) mod census_domain` › `pub(crate) enum Predicate {`

A `cfg` predicate, reduced to the one question this module asks of it.

`effects::tests::cfg` models predicates *properly* — every `target_os`,
every CI valuation, which platform compiles which body — and answers a
different question with them. This decides one: is the predicate false
wherever `test` is false. So every atom that is not `test` collapses to
[`Predicate::Other`], and the grammar below is the whole of what the
derivation reads. A predicate it cannot parse is a refusal, not a guess.

## `pub(crate) enum Predicate` › `Test,`

The `test` atom itself.

## `pub(crate) enum Predicate` › `Other(String),`

Any other atom: a bare name, or `key = "value"`.

## `pub(crate) enum Predicate` › `All(Vec<Predicate>),`

`all(…)`, and the conjunction an inline ancestry composes.

## `pub(crate) enum Predicate` › `Any(Vec<Predicate>),`

`any(…)`.

## `pub(crate) enum Predicate` › `Not(Box<Predicate>),`

`not(…)`.

## `impl Predicate` › `fn all(parts: Vec<Predicate>) -> Self {`

The conjunction of `parts`, flattened; the empty one is `All([])`,
which is true and entails nothing.

## `impl Predicate` › `pub(crate) fn render(&self) -> String {`

The predicate as it reads, for a diagnostic.

## `pub(crate) mod census_domain` › `pub(crate) fn entails_test(predicate: &Predicate) -> bool {`

Whether `predicate` is false wherever `test` is false.

Three-valued, with `test` bound to false and every other atom left
*unknown* — which is the only sound reading, because this module knows
nothing about platforms or features and must not pretend to. `all(test,
unix)` entails; `any(test, unix)` does not, because a Unix build without
`test` compiles it; `not(test)` does not.

## `pub(crate) mod census_domain` › `pub(crate) fn decide_without_test(predicate: &Predicate) -> Option<bool> {`

`predicate` with `test = false` and every other atom unknown.

`pub(crate)` since #318: [`super::lint_levels`] decides a `cfg_attr`'s
predicate through it, as the module scan decides a declaration's gate, so
those readers cannot disagree about what `all(test, unix)` means. The
production-fence rule in `tests.rs` asked it first until #318's fifth round;
it now decides an allowance against CI's production valuations, which know
`unix` and `target_os` where this leaves them unknown, and agrees with this on
every predicate this decides (`test` false in every production build).

## `fn decide_without_test(predicate: &Predicate) -> Option<bool> {` › `Predicate::All(parts) => {`

Short-circuiting, and the `None` arms are the point: one
undecidable conjunct does not make a conjunction undecidable if
another is already false, and one undecidable disjunct does not
make a disjunction undecidable if another is already true. The
empty forms answer as `cfg` does -- `all()` is true, `any()` is
false.

## `pub(crate) mod census_domain` › `pub(crate) fn parse_predicate(written: &str) -> Result<Predicate, String> {`

`written` as a [`Predicate`], or why it cannot be read.

The grammar is `all(…)`, `any(…)`, `not(P)`, and an atom — a bare name
or `name = "value"`. Anything else is refused: an unknown combinator, an
unbalanced paren, `not` with other than one argument, an empty atom.

Its splitter tracks a string only by toggling at `"`, so it is handed
[`with_literal_identity`]'s text, never raw source: there a literal is a
token with no quote, escape or separator inside it, and the token is the
literal's decoded value. An atom's text is its identity (`Predicate::Other`),
so `target_os = "linux"` and `target_os = "windows"` are two atoms -- what
#318's third reviews executed the absence of -- and `target_os = "linux"`,
`target_os = r"linux"` and `target_os = "lin\x75x"` are one, because rustc
gives the three one value -- what #318's fourth reviews executed the absence
of (`R4-MAIN-01`, `R4-REG-01`). Two different predicates over related atoms
-- `unix` and `target_family = "unix"` -- are still two atoms here; the
production-fence rule decides them against CI's valuations, where they are
related, and the lint reader treats them as independent, which only adds an
outcome.

## `pub(crate) fn parse_predicate(written: &str) -> Result<Predicate, String> {` › `if name.is_empty() {`

An atom: `test`, `unix`, or `key = "value"`.

## `pub(crate) mod census_domain` › `fn split_arguments(text: &str) -> Result<Vec<&str>, String> {`

The comma-separated arguments of a parenthesised group starting at `(`.

## `pub(crate) mod census_domain` › `pub(crate) fn with_literal_identity(raw: &str, blanked: &str) -> Option<String> {`

An attribute's text -- `raw` between its brackets, with `blanked` the same
span of [`super::blank_comments_and_strings`] -- with every comment blanked
and every string or char literal replaced by a token that **names its value
exactly and carries nothing a splitter reads as structure**; `None` when the
two spans disagree about where the code is, or when the text holds a doc
comment.

**Why it exists: the value is the predicate's identity.** Every reader of a
`cfg` or `cfg_attr` here used to parse the blanked text, where
`target_os = "linux"` and `target_os = "windows"` are both `target_os =`
followed by spaces: the grammar refused both, and the lint reader then made
the refused text one condition, so the two were assumed to hold together.
`#![deny(L)] #![cfg_attr(target_os = "linux", allow(L))]
#![cfg_attr(target_os = "windows", deny(L))]` read as a definite `deny`
while clippy-driver on Linux applied the `allow` -- 12 of 57 compiler cases
across the three governed lints in #318's third MAIN review, and the same
aliasing in the third regression review (`R3-MAIN-02`, `R3-REG-02`). The
production-fence rule in `tests.rs` dropped the whole unreadable predicate
instead, so `cfg(all(test, target_os = "linux"))` lost its `test` and an
allowance no production build compiles excused a `deny` (`R3-MAIN-01`).
Parsing the raw text is not the repair either: the grammar's splitter
toggles at every `"`, so an escaped quote inside a value moves top-level
commas into and out of the atom.

**One value is one token, whatever its spelling.** Every string literal is
read for the value rustc gives it in a `cfg` predicate
([`string_literal_value`]) and written back as a token for that value
([`literal_token`]): the value itself when it is ASCII letters, digits, `_`,
`-` and `.`, so a predicate still renders as the source usually spells it,
and otherwise `"%"` and the hex of the value's UTF-8 bytes. `"linux"`,
`r"linux"`, `r###"linux"###`, `"lin\x75x"`, `"lin\u{7_5}x"` and a `"lin\`
continued on the next line as `ux"` are one token; `r"lin\x75x"`, whose
backslash a raw string keeps, is another, and so is every other value. Until
#318's fifth round the token was the spelling -- a plain literal kept, any
other one its source text in hex -- so `all(target_os = "linux",
not(target_os = r"linux"))`, which no configuration satisfies, was two
independent atoms the production-fence rule satisfied with the first true
and the second false: it excused a `deny` for an allowance nothing applies,
and a macro-written `allow` behind that `deny` wrote 47 bytes through an
ordinary library with all ten local gates green (`R4-MAIN-01`,
`~/orch-pr10/reviews/pr-318r4/main-evidence/X4-raw*`; `R4-REG-01`, 63 bytes,
`regression-evidence/r4-equivalent-literals*`). The same split refused the
legitimate `all(target_os = "linux", target_os = r"linux")`, which Linux
applies.

**What it cannot decode, it does not name.** A literal it cannot read with
certainty -- a byte or C string, a suffix, an escape rustc refuses, a raw
string past 255 `#`s -- becomes `"?"` and the hex of its source text: no two
different literals share that token, and [`literal_token_value`] reads no
value from it, so the production-fence rule counts the atom it sits in as
unknown and never as evidence, and the lint reader counts it as a condition
of its own. rustc refuses every such literal in a `cfg` predicate.

**The structure is always `blanked`'s.** The walk mirrors
`code_bytes_only`'s rules -- comments first, then [`super::literal_end`]
where `code_bytes_only` would read a string, then [`super::char_literal_end`]
-- and every byte it keeps as code must be the byte `blanked` holds there,
every comment or literal must be spaces there; a rustc whitespace character
`blanked` turned to spaces is a space. Any disagreement is `None`, which
every caller treats as an attribute it cannot read, never as one it can. A
doc comment is the one comment that is not whitespace: rustc lexes `///`,
`//!`, `/**` and `/*!` as attribute tokens ([`is_doc_comment`]), and inside
an attribute's brackets one is an error, so the text has no reading.

## `pub(crate) mod census_domain` › `fn literal_token(literal: &str) -> String {`

The token a string literal is read as: its value when
[`string_literal_value`] decodes it -- the value itself when every byte is
one [`is_kept_value_byte`] keeps, otherwise `"%"` and the hex of the value's
bytes -- and `"?"` and the hex of the literal's source text when it does not.
The three forms share no token: a kept value holds neither `%` nor `?`.

## `pub(crate) mod census_domain` › `fn is_kept_value_byte(byte: u8) -> bool {`

The bytes a value may hold and still be written as itself: ASCII letters,
digits, `_`, `-` and `.`, which is every value a `cfg` in this tree names.

## `pub(crate) mod census_domain` › `pub(crate) fn literal_token_value(token: &str) -> Option<String> {`

The value a token [`literal_token`] wrote stands for: a kept value as it
stands, a `%` token's hex decoded; `None` for a `?` token -- a literal that
could not be decoded -- and for any text that is not a token at all. The
production-fence rule reads an atom's value through this and nothing else,
so a value it cannot name is an atom it cannot decide.

## `pub(crate) mod census_domain` › `const MOST_RAW_STRING_HASHES: usize = 255;`

rustc's limit on a raw string's `#`s; past it rustc refuses the literal and
this reads no value from it.

## `pub(crate) mod census_domain` › `fn string_literal_value(literal: &str) -> Option<String> {`

A string literal's value as rustc reads it in a `cfg` predicate, or `None`.
Plain `"…"`: `\n`, `\r`, `\t`, `\\`, `\0`, `\'` and `\"`; `\x` and two hex
digits up to `7F`; `\u{…}` with one to six hex digits, underscores after the
first, naming a character; and a backslash ending a line, which drops the
line break and the spaces, tabs and line breaks after it. Raw `r"…"`, with
up to 255 `#`s: the text between, as written. A CRLF is read as LF, as rustc
reads the file, and a bare CR is refused. Anything else -- a byte string, a
C string, an escape a string may not carry -- is `None`.

Measured, not assumed: clippy-driver applies a `cfg_attr` on Linux under
each of the escaped, underscored, line-continued and raw spellings of
`linux`, and refuses a byte string, a C string and a suffix there
(`~/orch-pr10/repair-318-r5-evidence/measure/clippy-semantics/`);
`effects::tests::the_production_fence_rule_reads_the_effective_activation_of_every_allowance`
compiles every spelling it names under the fence the rule asks for.

## `pub(crate) mod census_domain` › `fn unescape(characters: &mut std::str::Chars<'_>, value: &mut String) -> Option<()> {`

One escape after a backslash, pushed onto `value`; `None` for one a string
literal may not carry.

## `pub(crate) mod lint_levels {`

The **file-module-level lint state** reader, for the governance censuses.

`#[cfg(test)]` and `pub(crate)`, and both halves are the point. This is a
census instrument, not a product API: nothing the binary does consults it,
and a `pub fn` here would have been a shipped surface added for a test to
call. It sits at the BOTTOM beside [`census_domain`] for the same reason
that module does — `production_region` cuts a file at its first
`#[cfg(test)]` and
`effects::tests::every_production_region_that_stops_early_stops_at_a_module`
pins the ten files whose cut lands on something that is not a module. This
file is not one of them and must not become one.

## `pub(crate) mod lint_levels` › `pub(crate) struct Resolution {`

How a file's prologue resolves for one lint in the production build: the
level **in force**, whether rustc refuses the prologue outright, and
whether every production valuation agrees on the answer at all.

## `pub(crate) struct Resolution` › `pub(crate) level: Option<&'static str>,`

The level governing the file module, or `None` when its prologue
states none and the lint is left at whatever it inherits.

## `pub(crate) struct Resolution` › `pub(crate) refused_downgrade: bool,`

A later attribute tried to weaken a `forbid`. rustc answers `E0453`
and the crate does not compile, so this is not a level at all — it is
the file failing to build, and a reader that folded it into a level
would report a governance state for a file that has none.

## `pub(crate) struct Resolution` › `pub(crate) undecided: bool,`

The prologue names the lint under a predicate the production build does
not decide -- a platform's, a feature's -- and the valuations disagree; or it
states or may change the lint in a way the reader does not answer for: under
a predicate or in an attribute it cannot read (one rustc refuses -- a list
entry that is not a lint path, an unknown lint tool, a custom inner
attribute, a doc comment inside or between an attribute's tokens, an
attribute [`super::census_domain::with_literal_identity`] cannot read),
through an `allow` or `expect` the placement census does not read, or with a
`warnings` level where the lint's own is `warn` or unstated. No single level
is claimed: `level` is `None` and `refused_downgrade` is false. Undecided is
not "unstated", and the difference is the failure direction: a census that
treats it as unstated fails loud (the roll-call pin counts the pair), and a
census that asks for a `deny` or a `forbid` is told neither. #318's second
regression review executed the alternative on the reader as it was: it
skipped a nested `cfg_attr` it did not read and kept the level before it, so
`#![cfg_attr(not(test), deny(L), cfg_attr(not(test), allow(L)))]` was called
a `deny` while clippy-driver applied the `allow` (`R2-REG-02`,
`~/orch-pr10/reviews/pr-318r2/regression-evidence/probes/nested-parity.*`),
and #318's fourth reviews executed it again on attribute syntax: after a
`deny`, an `allow` written `r#allow`, through `r#cfg_attr`, with `#`, `!` and
`[` spaced or commented apart, naming `clippy::r#<lint>`, or naming
`clippy::all` or `clippy::style` was skipped and the `deny` answered
definite -- 27 of 48 clippy-driver cases in MAIN's sweep, 18 of 30 in
REGRESSION's (`R4-MAIN-02`, `R4-REG-02`).

**What each census does with it.** [`file_level_lint_state`] answers `None`
for it, and no census reads `None` as a fence. The roll-call guard
(`tests::unclassified_production_files_leaving_a_governed_lint_unfenced`)
names the file unless an ancestor states a `forbid`, which no lowering in the
file survives, and names a silent child of an undecided ancestor, which
states nothing the child can inherit. The classified-module pin counts the
pair as unstated, so the count moves and the pin fails. The placement
census's per-site-expectation rule (`tests::file_level_denies`) does not take
it for the `deny` an expectation needs. The container funnel census
(`runner::container::tests`, `closes_the_hole`) names it as stating no level.
The fence sweep reads the written `deny(` lists itself and still names one
nothing below allows. And the production-fence rule asks only about a `deny`,
which an undecided prologue is not: the roll-call or the pin reaches it
first. `tests::an_undecided_prologue_is_no_fence_to_the_censuses_that_read_one`
holds the fixture-fed half of that.

## `pub(crate) mod lint_levels` › `pub(crate) fn file_level_lint_resolution(source: &str, lint: &str) -> Resolution {`

[`Resolution`] for `lint` over `source`'s file-module prologue.

"File-module level" is the whole of the claim, and it is narrower than
"somewhere in the file". A lint level is scoped by the module tree, so
`#![deny(clippy::disallowed_types)]` in a file's prologue governs the file
and everything nested in it — while `#[deny(clippy::disallowed_types)]`
written on a single `fn` governs that function and says nothing whatever
about the file, which goes on inheriting whatever its ancestors allow.
A scan that accepts the second in place of the first reports a module as
having stated its own level when it has not, which is `PR6-LANEF-004`
answered by the wrong evidence.

So the walk is: from the first token -- past a byte-order mark and a
shebang line, which rustc strips before it lexes -- over whitespace, comments
and **inner** attributes only, stopping at the first token that is none of
them. That is exactly the region an `#![…]` may govern the file module from,
and it is the rule [`super::is_module_level`] applies to the inner half of
its answer. An attribute is its tokens, as rustc lexes them: `#`, `!` and `[`
with whitespace and plain comments between them are one inner attribute, and
a raw identifier is the name it spells.

### Ordered, because rustc is ordered

`PR72-LEVELS-001`. This used to return at the **first** attribute naming
the lint, which is not what a prologue means. Lint levels at one scope
are applied in source order and the last one wins, so
`#![deny(L)] #![allow(L)]` is a file where `L` is **allowed** — and the
first-match reader called it a denial. That is the failure direction that
matters: a census asking "has this module closed the hole" was told yes
by a prologue whose second line reopens it, and the reopening line is
exactly what an author adding an exception writes.

`forbid` is not symmetrical with the rest and is not modelled as if it
were. Once a lint is forbidden at a scope, a later `allow`, `warn` or
`expect` of it is `E0453` — the crate does not compile — while a later
`deny` or `forbid` is accepted and leaves the forbid in force. Both halves
are **measured** rather than reasoned: every row of
`effects::tests::the_file_level_lint_reader_answers_what_rustc_does` is
compiled by `clippy-driver` and this reader's answer is checked against
the diagnostics that come back, so no sentence here is the authority for
what the compiler does.

### Decided by every production valuation, or by none

The production build is not one valuation: three clippy legs compile the
lib target on three platforms, and a `cfg_attr` in the prologue may hold
on one and not another. The reader enumerates ([`file_level_lint_worlds`])
the answer under every assignment of the predicates it cannot decide and
returns a level only when all of them agree; otherwise it is
[`Resolution::undecided`] and claims nothing. What it *can* decide is what
[`super::census_domain::decide_without_test`] decides -- `test` false,
`not(test)` true, `all`/`any`/`not` evaluated, `all()` true and `any()`
false as `cfg` has them -- so `#![cfg_attr(not(test), forbid(L))]` is a
plain `forbid` here, `#![cfg_attr(test, ..)]` is nothing, and a nested
`cfg_attr` is expanded like the outer one. The same function decides the
module scan's whole-file test modules; the production-fence rule decides an
allowance against CI's valuations instead, where the reader must stay
undecided, because one file's prologue is read for every platform at once.

### A lint is named the way Clippy's lint store names it

A list entry is a lint path -- a raw identifier read as the name it spells,
spacing and comments between segments dropped -- and it names the governed
lint when it is `clippy::<lint>` or the prefixless `<lint>`, which rustc
applies to the Clippy lint with a deprecation warning; `clippy::disallowed_method`
or `clippy::disallowed_type`, Clippy's renames onto two of the three; or a
group that holds it -- `clippy::all` and `clippy::style`, the only two of
Clippy's ten groups that do, the prefixless `all` and `style`, and the
aliases `clippy_all` and `clippy_style` ([`what_a_lint_path_names`]). Any
other name under `clippy::`, `rustdoc::` or `rustc::`, and any other name of
one segment, is another lint or no lint, and changes nothing here; a name is
compared with its case, as rustc compares it. Each of those facts was read
from the driver -- `-W help` for the groups, its rename table, a
clippy-driver compile for every name
(`~/orch-pr10/repair-318-r5-evidence/measure/clippy-semantics/`) -- and each
is a compiled row of
`effects::tests::the_file_level_lint_reader_answers_what_rustc_does`. Until
#318's fifth round an entry was compared by its last `::` segment alone:
`allow(clippy::all)`, `allow(clippy::style)` and
`allow(clippy::r#disallowed_methods)` changed nothing here while Clippy
applied them (`R4-MAIN-02`, `R4-REG-02`), and the census
[`tests::the_three_blunt_governed_lints_are_used_by_nobody`], which refuses a
group allowance anywhere in the tree, was the only thing holding the answer
honest.

**A `forbid` a group states is not a fence.** rustc refuses a later `allow`,
`warn` or `expect` of a `forbid` (`E0453`) -- unless the `forbid` came from a
lint group, when it applies the lower level and warns
`forbidden_lint_groups`, which `-D warnings` does not promote (measured:
`forbid(clippy::all)` then `allow(<lint>)` builds under `-D warnings` with
that one warning). A later group `forbid` turns an earlier direct one into
that kind, and a later direct `forbid` turns it back. A `forbid` whose last
source is a group is reported as `deny`: an error level an inner attribute
lowers, which is the question every census here asks.

### What it will not answer for

**An allowance only this reader reads.** An `allow` or `expect` of the lint
spelled so that the placement census (`super::governed_allows`) does not
read it is a lowering no row of `effects/allowlist.toml` accounts for, and
the censuses here take a stated `allow` for one the placement census
recorded. So when such a statement sets the level the reader answers
undecided rather than `allow` ([`recorded_by_the_placement_census`]); the
same spelling the production-fence rule refuses to count as an excuse. Until
the attribute-token repair of 2026-09-26 this was a list -- `#`, `!` and `[`
spaced or commented apart, a raw lint name, a prefixless group alias, a
rename -- and the placement census now reads every one of them
([`super::normalize_lint`], [`super::attribute_open`]): each is a decided row
of `the_file_level_lint_reader_answers_what_rustc_does`, compiled. No
spelling this reader reads and the census does not is known; the undecided
answer stays for the one a later reading misses.

**`warnings` over a `warn`.** A `warnings` level replaces a lint's `warn`,
the default included, and leaves a stated `allow`, `expect`, `deny` or
`forbid` alone: `#![deny(L)] #![allow(warnings)]` is a `deny`, and
`#![warn(L)] #![allow(warnings)]` compiles silent. Where the lint's level in
the file is `warn` or unstated, the answer turns on what the file inherits,
which one file does not say, so a `warnings` statement leaves it undecided.

**What rustc refuses.** A list entry that is not a lint path, `reason` before
a lint, a tool other than `clippy`, `rustdoc` or `rustc`, a path of three
segments, a custom inner attribute (a path, which stable rustc refuses at
inner position), a level attribute with no parenthesised list, a doc comment
inside or between an attribute's tokens, and a `#!` that opens no attribute
are read as nothing the file can be answered for.

Comments and string literals are blanked first, so a level quoted in a doc
comment or inside a `&str` is invisible — `PR4-CENSUS-COMMENT-ORACLE`, and
this crate's effect fixtures are written as exactly those two shapes. Inside
an attribute the prologue walk found, a literal is read back as the token
[`super::census_domain::with_literal_identity`] gives it: a predicate's value
keeps its identity, and a `reason = "…"` that spells a lint name is still no
entry of the level's list.

`clippy::disallowed_methods` and `disallowed_methods` are the same lint;
[`what_a_lint_path_names`] is the bridge for this reader, and
[`super::normalize_lint`] for the placement census and every text scan.

## `pub(crate) mod lint_levels` › `pub(crate) type World = (Option<&'static str>, bool);`

One production valuation's answer: the level in force, and whether the
prologue is `E0453` under it.

## `pub(crate) mod lint_levels` › `struct Statement {`

One statement the prologue makes about the lint, in source order: what it
does ([`Effect`]), and the undecided predicates that all have to hold for it
to apply. A statement under `cfg_attr(not(test), ..)` carries no condition;
one under `cfg_attr(unix, ..)` carries `unix`; one nested in both carries
`unix`.

## `pub(crate) mod lint_levels` › `enum Effect {`

A level for the lint -- with whether a lint group stated it, which decides
whether a `forbid` holds, and whether the placement census reads the
attribute it sits in -- or a level for `warnings`, which replaces a `warn`.

## `pub(crate) mod lint_levels` › `struct Unreadable;`

A level attribute rustc refuses, or names the lint through a tool it does not
know: the prologue has no answer for the lint.

## `pub(crate) mod lint_levels` › `enum Named {`

What one lint path names, for one governed lint: the lint itself, a group
that holds it, `warnings`, another lint or none, or a path rustc refuses.

## `pub(crate) mod lint_levels` › `const GROUPS_NAMING_THE_GOVERNED_LINTS: [&str; 2] = ["all", "style"];`

The Clippy groups that hold the three governed lints. `clippy-driver -W help`
lists ten groups; `disallowed_methods`, `disallowed_types` and
`disallowed_macros` are in `all` and `style` and in no other, `restriction`
included.

## `pub(crate) mod lint_levels` › `const LINT_TOOLS_NAMING_NO_GOVERNED_LINT: [&str; 2] = ["rustdoc", "rustc"];`

The two lint tools besides `clippy` that rustc knows. Their names are no
governed lint; any other tool is `E0710`.

## `pub(crate) mod lint_levels` › `const MOST_UNDECIDED_PREDICATES: usize = 12;`

The valuations are enumerated, two to the power of the distinct undecided
predicates in the prologue; past this many the reader answers undecided
rather than enumerate. No prologue in the tree carries even one.

## `pub(crate) mod lint_levels` › `pub(crate) fn file_level_lint_worlds(source: &str, lint: &str) -> BTreeSet<World> {`

Every answer some production valuation gives for `lint` over `source`'s
prologue: the statements are replayed in order under each assignment of
the undecided predicates by [`replay`], and the distinct outcomes are the
set. One element is a decided prologue; more than one is what
[`Resolution::undecided`] reports; none is a prologue past
`MOST_UNDECIDED_PREDICATES`, one that states the lint under a predicate or in
an attribute the reader cannot read, or one some valuation of which `replay`
does not answer for.

Predicates are variables by their rendered text, and the text keeps each
string's value ([`super::census_domain::with_literal_identity`]): `unix`
written twice is one variable, `target_os = "linux"` and
`target_os = r"linux"` are one, `target_os = "linux"` and
`target_os = "windows"` are two, and two different predicates are
independent -- an over-approximation of the real valuations that can only
add outcomes, never remove one, so it can only make the reader refuse,
never decide wrongly. That argument held only while no two different
predicates shared a text, and until #318's fourth round they did: the
values were blanked, both OS predicates became one unreadable condition,
and the reader replayed an `allow` and a `deny` under one variable and
answered a definite `deny` that Linux compiles as `allow` (`R3-MAIN-02`,
`R3-REG-02`). A predicate whose identity cannot be read is not guessed at:
the file has no answer for the lint.

## `pub(crate) mod lint_levels` › `fn replay<'a>(statements: impl Iterator<Item = &'a Statement>) -> Option<World> {`

One valuation's answer: the statements that apply in it, in order. After a
direct `forbid`, `deny` is ignored, `forbid` stays, and anything weaker is
`E0453` -- the file not compiling rather than a level. After a `forbid` a
group stated, a weaker level applies, as `forbidden_lint_groups` lets it, and
the answer reports that `forbid` as the lowerable `deny` it is. Anything else
replaces what came before it. `None` when the level it ends on was set by an
`allow` or `expect` the placement census does not read, or when `warnings`
is stated and the lint's own level is `warn` or unstated.

## `pub(crate) mod lint_levels` › `fn lint_statements_in_the_prologue(source: &str, lint: &str) -> Option<Vec<Statement>> {`

The walk: from [`prologue_start`], over whitespace, comments, inner doc
comments and inner attributes only, each attribute's `#`, `!` and `[` found
past whitespace and plain comments, its text read through
[`super::census_domain::with_literal_identity`], expanded by
[`applied_attributes`] into the attributes it applies, and each statement
about the lint kept with the undecided predicates it is applied under.
`None` when the lint is stated under a predicate the grammar cannot read, in
an attribute whose text cannot be read, by a level attribute rustc refuses
([`stated_effects`]), by a `#!` that opens no attribute, or when an inner
attribute follows where the walk stopped ([`an_inner_attribute_follows`]):
the file then has no answer for the lint, rather than an answer that assumed
what the unreadable text said.

## `pub(crate) mod lint_levels` › `fn prologue_start(source: &str) -> usize {`

Where rustc starts lexing a file: past a byte-order mark, and past the first
line when it opens `#!` and the next token past whitespace and plain comments
is not `[` -- a shebang, which rustc strips, `#!/** doc */[..]` included
(measured: that first line applies nothing). Until #318's fifth round the
walk began at the first byte, so a mark or a shebang ended the prologue
before it began and every level below it was read as unstated.

## `pub(crate) mod lint_levels` › `fn past_inner_doc_comments(source: &str, from: usize) -> usize {`

The next token that is not an inner doc comment: `//!` and `/*!` are inner
`doc` attributes, which state no level. An outer doc comment is not skipped;
it ends the prologue.

## `pub(crate) mod lint_levels` › `fn an_inner_attribute_follows(source: &str, from: usize) -> bool {`

Whether an inner attribute starts at `from`, past every comment: one the walk
stopped short of -- after an outer doc comment, or with a doc comment between
its `#` and `!` -- which rustc refuses, so the walk has no answer rather than
the answer before it.

## `pub(crate) mod lint_levels` › `fn recorded_by_the_placement_census(attribute: &str, lint: &str) -> bool {`

Whether `super::governed_allows`, the placement census, reads an allowance of
the lint or of a group holding it in `attribute` -- the attribute's whole
source span, `#` to `]`. The allowlist census requires a row for what it
reads and the blunt-lint census refuses a group anywhere; an allowance it does
not read, no census records. The production-fence rule in `tests.rs` asks
the same question of an allowance before counting it as an excuse, so the two
answers cannot part.

## `fn lint_statements_in_the_prologue(source: &str, lint: &str) -> Option<Vec<Statement>> {` › `if bytes.get(open) != Some(&b'[') {`

A `#` and a `!` that open no `[` -- `#!/bin/sh` below the first line, or a
doc comment between the `!` and the `[` -- is a file rustc refuses.

## `fn lint_statements_in_the_prologue(source: &str, lint: &str) -> Option<Vec<Statement>> {` › `for applied in applied_attributes(attribute.trim()) {`

An inner attribute states what it applies in the production build, and an
attribute that states nothing about the lint is passed over whatever its
predicates are. For
every attribute but `cfg_attr` that is the attribute itself; for a
`cfg_attr` it is what its predicates apply, nested `cfg_attr`s included.
Each predicate on the way is decided by
[`super::census_domain::decide_without_test`]: true, and it adds no
condition; false, and the statement is not in the production build;
undecided -- a platform, a feature, any atom but `test` -- and it becomes a
condition that [`file_level_lint_worlds`] enumerates both ways. A predicate
rustc would refuse (`cfg_attr(, ..)`, `not(a, b)`, `not(te st)`) is no
condition at all: rustc does not compile such a file, and the reader claims
no level for it rather than repair the spelling.

## `pub(crate) mod lint_levels` › `fn stated_effects(attribute: &str, lint: &str, recorded: bool) -> Result<Vec<Effect>, Unreadable> {`

What an applied attribute states about the lint, in list order: nothing, for
an attribute that is not one of the five level attributes; for one that is,
an [`Effect`] per entry naming the lint, a group holding it, or `warnings`
([`what_a_lint_path_names`]). [`Unreadable`] for a custom inner attribute (a
path), a level attribute with no parenthesised list, an entry that is not a
lint path or a `reason`, an entry after the `reason`, and a lint tool rustc
does not know -- each a prologue rustc refuses.

**The valuation this reader answers for is the production build's**: the
lib target every clippy leg checks and the binary links, compiled without
`cfg(test)`. Why the reader learned to expand `cfg_attr`:
`src/agent/bin.rs`'s inline `#[cfg(test)] mod tests` allows
`disallowed_methods`, so an unconditional `forbid` is `E0453` at the lib
test target (measured by both of #318's first reviews) and the file carried
`deny` -- a level a macro-generated `allow` in its production region
lowered, executed by #318's MAIN review
(`PR318-DENY-THE-PRODUCTION-BUILD-COULD-FORBID`). The repair is a `forbid`
that exists in every build the test module does not, and a reader that did
not read it would have counted the pair unstated. The same shape was then
found and executed in `src/runner/container/census.rs`, `exec.rs` and
`resolve.rs` and under `src/engine/mod.rs`, and fenced the same way
(`effects::tests::no_deny_of_a_governed_lint_is_excused_by_test_code_alone`).
Why it learned nesting: the first form returned the outer `cfg_attr`'s
attributes unexpanded and the level loop skipped the nested one, so a later
`allow` nested under the same predicate was dropped and the earlier `deny`
kept -- a false fence, executed by #318's second regression review
(`R2-REG-02`). Why it keeps literal identity: see
[`super::census_domain::with_literal_identity`] (`R3-MAIN-02`, `R3-REG-02`).
Why it reads names and tokens as rustc does: see
[`file_level_lint_resolution`] (`R4-MAIN-02`, `R4-REG-02`).

Measured, not reasoned: every row of
`effects::tests::the_file_level_lint_reader_answers_what_rustc_does` is
compiled by `clippy-driver` without `--test`, which is exactly this
valuation, for each of the three governed lints; the nested, composite,
empty-combinator, `cfg_attr(test, ..)`, raw, spaced, group, group-`forbid`,
mark and shebang rows are among the decided ones, a second table of
platform, feature and malformed prologues checks that the reader refuses to
decide what the compiler decides differently per valuation, and that what it
cannot read does not compile, a third of string-valued predicates -- the
reviews' collisions among them -- checks that distinct values stay distinct,
and a fourth holds the prologues it will not answer for, each with what
clippy-driver did with it.

## `pub(crate) mod lint_levels` › `fn is_a_reason(entry: &str) -> bool {`

Whether a list entry is the `reason = "…"` rustc accepts last in a level
attribute, `r#reason` included. A reason that spells a lint name is no entry
of the list.

## `pub(crate) mod lint_levels` › `fn lint_path(entry: &str) -> Option<Vec<&str>> {`

A list entry's segments, each an identifier read as the name it spells, with
whitespace allowed around `::`; `None` for anything else -- a literal, a
nested list, a leading `::`.

## `pub(crate) mod lint_levels` › `fn what_a_lint_path_names(path: &[&str], lint: &str) -> Named {`

What a lint path names for one governed lint, as Clippy's lint store answers
it: see [`file_level_lint_resolution`] for the rule and its measurement. The
bridge between `clippy::disallowed_methods` and the prefixless
`disallowed_methods`, and the one place a group, a rename or `warnings` is
recognised.

## `pub(crate) mod lint_levels` › `pub(crate) struct Applied<'a> {`

One attribute an attribute applies, and the `cfg_attr` predicates it is
applied under, outermost first, each parsed or the reason it cannot be.

## `pub(crate) mod lint_levels` › `pub(crate) fn applied_attributes(attribute: &str) -> Vec<Applied<'_>> {`

**The crate's one reading of `cfg_attr`.** Every attribute `attribute`
applies, with the predicates it is applied under: for anything but a
`cfg_attr`, the attribute itself under none; for a `cfg_attr`, what each of
its attributes applies, under its predicate and theirs. Nothing is decided
here -- [`lint_statements_in_the_prologue`] decides for the production
build, [`super::census_domain::scan_modules`] turns an applied `cfg` into a
declaration's gate, and the production-fence rule in `tests.rs` turns the
predicates, an item's gates and its enclosing items' gates into an
allowance's effective activation. Before #318's fourth round each of those
read `cfg_attr` its own way, and the fence rule read only the outermost
predicate: `#[cfg_attr(unix, cfg_attr(test, allow(L)))]` was an allowance
under `unix` (`R3-MAIN-01`, `R3-REG-01`). Handed
[`super::census_domain::with_literal_identity`]'s text, so a predicate
keeps its values.

## `pub(crate) mod lint_levels` › `fn applied_under<'a>(`

The recursion: a `cfg_attr` with no parenthesised body applies nothing,
and its first argument is the predicate whatever it holds.

## `pub(crate) mod lint_levels` › `pub(crate) fn attribute_name(attribute: &str) -> &str {`

The path an attribute starts with, up to the first character that is not
an identifier's: `cfg`, `cfg_attr`, `allow`; a raw `r#cfg_attr` is
`cfg_attr`, which is what rustc resolves it as (measured). Until #318's fifth
round `r#allow` read as `r`, so the reader skipped it, and `r#cfg` and
`r#cfg_attr` gated nothing the production-fence rule could see (`R4-MAIN-02`,
`R4-REG-02`, and REGRESSION's `raw_cfg_gate` shapes).

## `pub(crate) mod lint_levels` › `fn attribute_name_token(attribute: &str) -> (usize, &str) {`

The name and where its token ends, `r#` included, so what follows the name
is found past the prefix.

## `pub(crate) mod lint_levels` › `pub(crate) fn attribute_arguments(attribute: &str) -> Option<&str> {`

What an attribute holds between the parentheses after its name.

## `pub(crate) mod lint_levels` › `pub(crate) fn top_level_arguments(body: &str) -> Vec<&str> {`

`cfg_attr`'s arguments, split at the commas that are not inside
parentheses, brackets, braces or a string: the predicate first, then
each attribute it applies. `pub(crate)` for the production-fence rule in
`tests.rs`, which splits an allowance's list of lints with it.

## `pub(crate) mod lint_levels` › `pub(crate) fn leading_inner_attributes(source: &str) -> &str {`

The inner attributes a file, or an inline module's body, opens with: the raw
text from its first byte to the end of the last `#![…]` before anything that
is not one. Comments and blank lines between them are kept, because the text
is handed on verbatim -- to `governed_allows`, to learn what an inline module
writes inside its braces, and to a compiled fixture, as the header of a crate
root that has to carry exactly what `src/engine/mod.rs` carries
(`the_engine_facade_allows_no_governed_lint_and_refuses_both_escape_routes`).
It stops where [`file_level_lint_resolution`] stops, for the same reason: an
inner attribute after the first item is not one rustc accepts. It finds each
attribute with [`super::attribute_open`], so `# ![..]` and `#! [..]` are
leading attributes as they are to rustc and to the prologue walk; until the
attribute-token repair they ended the prologue here, which handed
`governed_allows` an inline module's leading attributes with its spelled-apart
allow cut off (`the_prologue_readers_read_an_inner_attribute_whatever_rustc_reads_between_its_tokens`).

## `pub(crate) mod lint_levels` › `pub(crate) fn file_level_lint_state(source: &str, lint: &str) -> Option<&'static str> {`

The level in force for `lint` at `source`'s file-module scope, or none.

[`file_level_lint_resolution`] without the `E0453` bit, for the censuses
that ask only which level governs a module.

## `pub(crate) mod tests;`

`pub(crate)` so `cfg::WHOLE_FILE_TEST_MODULES` -- the crate's only statement
of the whole-file test-module population -- reaches the one census outside
this module that floors a count on it. Test-only either way: the module is
compiled only under `cfg(test)`.

## `if depth == 0`

A named function cannot be part of the preceding return type. If a
malformed test signature has no body, keep the following item visible
instead of taking its brace as the missing test body's boundary.

## `fn configured_function_return_start(bytes: &[u8], start: usize) -> Option<usize> {`

Recognize the return arrow of a named function item without type parameters.
A field's function-pointer type is not an item. Unknown prefixes, generic
parameter lists and incomplete signatures keep the conservative comma rule.
The input is already blanked, including any extern ABI string.

## `#[must_use]`

Every `allow`/`expect` of a governed lint in `source`, with where it sits.

Attributes are found in the blanked text and read out of the original, so a
fixture quoted in a doc comment is invisible and a real attribute is not.
