# `src/config/read.rs`

Extended notes for [`src/config/read.rs`](../../../src/config/read.rs).

The code is the authority for what it does; this file is the whole of its prose, moved out of
the source verbatim. Each section is headed by the line of code the comment sat above, spelled
as it is in the source, so the heading is the grep string that finds the code.

## Module

Turning captured bytes into raw and typed shapes.

Every function here that reads a file's contents takes a [`FileSnapshot`]
and never a path. That is the whole of what makes a pre-lock validation
worth its ordering: the bytes that were validated and the bytes that are
parsed are the same object, so there is no second read for the file to
change between. A reader here that reached for the path instead would
reintroduce exactly the window the capture exists to close.

`parse_pool` is the exception to the shape and not to the rule: it takes a
`&Path` and never reads through it. The path is what it names in the error
it returns and in one diagnostic string, nothing more. A later change that
wants a file's contents here reaches for the snapshot, never for that path.

`[pools]` keeps the temperament the rest of the configuration surface has:
anything that would silently change what the estimator does is an error,
anything that only degrades what it can say is a warning that names the key.
Pool order is file order, and file order is preference — the span each entry
carries is what preserves it through a map.

**A configuration error never repeats the file's text.** Every refusal a
TOML parser or serde gives here, and every one `super::parse` and the parent
receive from a section's `try_into`, becomes words in one place,
[`toml_error_message`]: the line, the column and what is wrong, with any long
value the message quotes withheld.

## `#![forbid(`

**This child states its own lint level and inherits nothing.** A Rust lint
level is scoped by the module tree rather than by the file, so an out-of-line
child of `src/config.rs` inherits that file's inner
`#![allow(clippy::disallowed_methods)]` unless it says otherwise --
`PR6-LANEF-004`, and the mistake two W1 pull requests then made
independently (#100 and #102). Nothing here reaches a governed primitive, so
all three governed lints are DENIED and this module takes no
`effects/allowlist.toml` row: a row records an allowance, and this module
takes none.

The three are not equally load-bearing, and which is which is worth stating.
`src/config.rs` allows `clippy::disallowed_methods` and that lint alone, so
the first line below is the one that restores a level the parent removed
outright: without it, a denied method here raises no diagnostic at all. The
other two raise this module from clippy's default `warn` to `deny`, so a
denied type or macro fails here on its own rather than only under CI's
`-D warnings`. All three are written out because what decides the first one
is a property of the parent's attribute rather than of this file, and a
parent's attribute can widen without this file changing.

## `pub(super) fn read_repo_config(`

An absent, explicit `--config` path is an error ("file not found"); an absent,
discovered one is the normal fresh case and returns [`RawRepoConfig::default`]
silently — the same explicit/discovered asymmetry [`read_pools`] states below
for `--pools`. Once bytes exist, `toml::from_str` decides the rest, and what it
says reaches the operator through [`toml_error_message`] — its line, column and
message, never the file's text — with nothing added: whatever `RawRepoConfig`
accepts or refuses at the wire, this function's own contract is only the two
file-absence branches above. What the wire refuses includes a misspelled top-level section
name — `[budgts]`, `[interation]`, `[runer]` — which `RawRepoConfig` used to
deserialize into nothing with no warning (`SWEEP-CONFIG-PARSE-007`, guarded to
whichever of this file or `src/config.rs` swept first): the struct now denies
unknown fields, the refusal names the typo and the accepted eight sections, and
the regression through `load_captured`, on a capture built in memory, lives in
the parent's suite beside the struct.

## `pub(super) fn read_machine_settings(`

The machine-level settings file (§17), parsed twice from one captured text:
as [`RawRepoConfig`], which is what refuses an unknown section with the
line and column of the typo, and as a plain table, which is what
`layers::merge` lays the repository's file over. An absent file is `None`,
never an error: the machine level is optional. A refusal says the file is
the machine-level one, since an operator who is told only a path under their
home directory may not remember why a run in some repository read it; the
sentence that says so follows [`toml_error_message`]'s rendering, so the line
and column still lead.

## `pub(super) fn settings_table(`

The repository's settings file as a plain table, for the merge. An absent
file is an empty table, so a repository with no file of its own takes the
machine's settings whole.

## `pub(super) fn read_pools(`

Read `~/.upstroke/pools.toml` into typed pools (§17).

Temperament matches the rest of this file: anything that would silently
change what the estimator does is an error, and anything that only degrades
what it can say is a warning.

- unknown `kind` → **error**; it decides which estimator rule runs.
- unknown `sources` entry → **error**; dropping `signals` by typo would
  discard §13's ground truth while the file still claims to have it.
- `safety_margin` / `reserve` outside `0.0..=1.0` → **error**; both are
  fractions, and a "150% margin" has no reading that is merely degraded.
- `agent` with no adapter in this build → **warn**, pool kept and marked
  unusable. §17's own example ships `[pools.local] agent = "aider"`, so
  erroring would brick anyone who copied the documented file.
- unknown keys → **warn**, by name.

An **explicit** `--pools` path that does not exist is an error, the way an
explicit `--config` is in [`read_repo_config`]: a path someone typed and
that is not there is a typo, and answering it with "no pools connected —
run `upstroke connect`" sends them to regenerate a file that was never the
problem. A *discovered* one that is absent is the normal fresh case and
stays silent.

## `let mut entries: Vec<(String, toml::Spanned<toml::Value>)> =`

Back into the order they were written in — see [`RawPools`].

## `if name.trim().is_empty() {`

A pool's name is its identity everywhere downstream — it is what an
attempt is attributed to and what the ledger prints. A blank one is
indistinguishable from "no pool" by the time it reaches the engine
(`pool_option` maps `""` to `None`), so the attribution would vanish
while the pool still matched for routing. Same reasoning as the
non-empty `[[gates]]` `name`.

## `let usage = UsageSource::from_keys(`

The three usage keys (§17), validated by the one function `connect` also
carries them through, so a file `connect` wrote is a file this reads. A
refusal is prefixed with the pool's table and is otherwise
`UsageKeyDefect`'s own text, which never repeats a `usage_token_env` that is
not a variable's name.

A pool that names a provider has `provider-endpoint` among its sources
whether or not the file lists it. The provider key is the opt-in; requiring
the source to be listed as well would be two switches for one decision, and
`connect`, which writes `sources`, would then have to write one of them.

## `fn converts_exactly(units: i64) -> bool {`

An `as` narrowing needs a nearby invariant that proves the range (§5), and
this one is *checked* rather than assumed. TOML hands `monthly_allowance` the
whole `i64` range, and an `f64` carries 53 significant bits, so not every
integer survives the cast: written `9223372036854775807`, an operator would
have got `9223372036854775808` back — a ceiling nobody set. So the integer
branch refuses, by name, any integer the cast would change, and what survives
is the number that was written.

What decides it is the span between the highest and the lowest set bit of the
magnitude: an integer converts exactly when that span fits in the mantissa,
whatever its size. `2^53 + 1` needs 54 bits and is refused; `2^53 + 2` and
`10^16` (the older writer's integer spelling `design/17` names, which is
`2^16 × 152587890625`) fit and are accepted unchanged. An earlier version of
this check was a blanket ceiling at `2^53`, which refused those two; and the
version before that argued the range instead of enforcing it, reasoning that a
hand-typed spend ceiling is never that large — a claim about operators, not
about accepted input. Both were wrong on the input the parser actually takes.
The predicate is integer arithmetic only, so the proof of the cast does not
itself lean on a cast; the sign is refused just below, where `i64::MIN`, exact
though it is, fails the same check every negative value fails.

The finiteness and sign check below applies uniformly to both the integer
and float branches; for the integer branch it can only ever pass, since a
cast from a finite `i64` is never `NaN` or infinite, and it is written once
for both rather than duplicated per branch. The float branch takes no
exactness check: a `toml` float is already an `f64`, so there is no conversion
to lose anything, and `is_finite` is the whole of what it needs.
[`connect::render::toml_number`] writes an allowance back out as an integer
only below `2^53` and as a float otherwise; that is the writer's choice of
spelling and is unaffected by what the reader accepts.

## `pub(super) fn toml_error_message(error: &toml::de::Error, text: Option<&str>) -> String {`

**A configuration error never repeats the file's text.** This is the one
place a `toml::de::Error` becomes words: the repository file, the
machine-level file, [`settings_table`] and the pools file here, each pool's
body in [`parse_pool`], every section `super::parse` reads, and in the parent
`[routing.effort]`, the routing entries and the merged settings
`load_captured_with` interprets (`PR409-USAGE-TOKEN-TOML-ERROR-LEAK`). The
error's `Display` quotes the offending source line under a caret, so a
credential pasted unquoted — `usage_token_env = github_pat_…`, which is not
TOML — reached the terminal, and any log that captured it, whole.
`UsageSource::from_keys` refuses a credential-shaped `usage_token_env`
without repeating it, but it never ran: the file did not parse.

What it says instead is `line L, column C: <message>`. L and C are 1-based
and counted from the start of `error.span()` against the file's text, and
`<message>` is `error.message()`, the parser's or serde's statement of what is
wrong, which never holds the source line. An error with no span, or with no
text to place it against, is the message alone. Every `try_into` of an
already-parsed `toml::Value` or table is that case, since the value has
dropped its spans; those callers pass `None`, because there is no file text
where they stand to pass. The key path `Display` adds to such an error
(``in `reserve` ``) is not carried either, because `toml` gives it only
through `Display`. The caller names the section, and within it the type the
message expected is what points at the key: a type error in `[pools.p]` reads
`invalid type: string "…", expected f64`, without saying that the key was
`reserve`.

The message is not the file's text, but it can quote a value from it: serde
names a string it could not use (`invalid type: string "…", expected f64`), a
variant it does not know, and the key it refused. So it goes through
[`redact_quoted`] before anything else sees it.

## `fn position(text: &str, offset: usize) -> (usize, usize) {`

The line and column of a byte offset, both 1-based: lines counted by `\n`,
columns by characters, so a multi-byte character before the error moves the
column by one. It walks the characters before the offset rather than slicing
at it, so no offset can panic: one past the end is the position after the
last character, and one inside a character counts that character as before
it. The parser's spans start on a character, so the second never happens on
an error it produced.

## `const LONGEST_QUOTED_VALUE: usize = 16;`

The operator's rule for what a message may quote, ruled on
`PR409-USAGE-TOKEN-TOML-ERROR-LEAK`: any run between double quotes, single
quotes or backticks that is longer than 16 characters is replaced by
`<redacted: N characters>`, N its character count, and the delimiters stay.
A short run is kept, so the message still says what is wrong: a type, a
short value, the parser's own `` `"` `` and `` `'` ``.

The rule is by length, not by shape. Nothing here asks whether a run looks
like a credential: `looks_like_a_credential` in `src/capacity.rs` stays
private to the usage keys, and a value that has to look like a token to be
withheld is a value a token of a new shape walks past.

## `const LONGEST_QUOTED_NAME: usize = 64;`

The bound for a name. A key or schema name is exempt from the 16-character
rule, because 16 also caught serde's name for a misspelled key:
`wait_on_block_sec` is 17 characters, and
`an_unknown_interaction_key_is_refused_and_named` in `super::parse` requires
the refusal to name it. So a name where [`slot`] recognises one stays whole up
to 64 characters and past that is redacted as a value is, because a
credential pasted as a key — `github_pat_…` is a valid bare key — must not be
echoed either. 64 is well above every name the schema has (the longest,
`frontier_escalation_over_usd`, is 28) and below a fine-grained GitHub token
(93). A shorter secret written as a key, such as a 40-character classic
token, is repeated: that is the cost of naming every key, and the ruling
accepted it for keys only.

## `const NAME_FOLLOWS: [&str; 5] = [`

The leads after which a backtick run names a key: serde's `unknown field`,
`missing field` and `duplicate field`, and the parser's `duplicate key` and
`dotted key`. These five and no others. The ``in table `a.b` `` that follows
a duplicate key is a table path rather than one of them, so it is measured as
a value, and a path longer than 16 characters is redacted; the line and
column are what locate it.

## `const LIST_FOLLOWS: [&str; 2] = ["unknown field ", "unknown variant "];`

The two serde messages that end with the names it would have accepted:
``unknown field `x`, expected one of `a`, `b`, `c` ``, and the same for a
variant. The list's names are exempt whatever its subject was. The subject
itself is exempt only after `unknown field`: an unknown variant is a value
the operator wrote (`tier = "…"`), and it is measured as one.

## `#[derive(Clone, Copy)]`

What the run before this one was, which is all [`slot`] needs in order to
tell whether a run continues serde's list: `Subject` is the run an `unknown
field` or `unknown variant` names, `Entry` a name already in the list, and
`Other` anything else.

## `fn redact_quoted(message: &str) -> String {`

One pass over the message. A quote opens a run that the same character
closes, and the other two quote characters inside it are text: the parser's
``expected `"`, `'` `` keeps its `"` and `'`, and winnow's ``'`'``, its
spelling of an expected backtick, is a one-character run. Inside double
quotes a backslash carries the next character with it. serde writes a string
value with Rust's debug escaping, so a value that holds `"` shows it as `\"`,
and ending the run there would let the rest of the value out unquoted. A
quote that is never closed runs to the end of the message and is measured
like any other run; neither the parser nor serde writes one, and a run is not
let through for want of its closing quote. N counts what the message shows
between the delimiters, escapes included.

## `fn slot(quote: char, plain: &str, previous: Previous) -> (usize, Previous) {`

The longest a run may be where it stands, and what it leaves for the next
run. Only a backtick run can be a name: serde and the parser quote names in
backticks, and serde quotes a string value in double quotes. A list
continues only from its own subject and only through serde's separators:
`, expected ` or `, expected one of ` straight after the subject (one name,
or three or more), then a comma and a space between names, or the word `or`
with a space on each side in the two-name form. The text between two runs is
compared whole there, not by its ending,
so an `expected` anywhere else — the parser's ``expected `.`, `]` ``, serde's
`expected a sequence` — starts no list, and a run after it is measured as a
value.

## `fn absent(tag: &str, required: bool) -> FileSnapshot {`

The absent-file tests need a snapshot whose capture found no file, and they
build one directly rather than pointing `snapshot_file` at a path that is
supposed not to exist. A path is only as absent as the filesystem says it is
at the moment of the read: a fixed name can be satisfied by a leftover, and a
per-process name can be satisfied by a leftover from an earlier process that
was issued the same id. `FileSnapshot`'s fields are private to `src/config.rs`
and visible here because this module is its child, and `Ok(None)` is exactly
what `snapshot_file` records for a file that is not there, so the fixture is
the captured state itself with no filesystem in the oracle. Reaching for `fs`
to manage a real directory would put a governed primitive in a module that
denies all three and takes no `effects/allowlist.toml` row.

## `fn holding(name: &str, text: &str) -> FileSnapshot {`

A snapshot that captured `text`, built directly for the reason [`absent`] is:
what is under test is a refusal of bytes, and `Ok(Some(bytes))` is what
`snapshot_file` records for a file it read.

## `fn an_integer_allowance_that_converts_exactly_is_accepted_unchanged() {`

The expected values are written out as float literals rather than spelled
`written as f64`, which is the very cast under test: the expected value has to
come from outside the code being proved. The refusal test's `i64::MAX` case is
the value that motivated the check — it casts to `9223372036854775808.0`, one
more than it is, and accepting it would silently store an allowance the
operator did not write.

## `fn unquoted_pasted_token_is_not_echoed_by_configuration_error() {`

The witness of `PR409-USAGE-TOKEN-TOML-ERROR-LEAK`, ported: a 93-character
`github_pat_` token pasted unquoted into the pools file and into the
repository file, each loaded through `load_captured` from a capture built in
memory, the path `config::load` takes once its capture exists. Neither
refusal may contain the token, and each must still name its file, the line
and column of the value, and what is wrong. The assertions compare and never
print: no failure message carries the refusal, because a refusal that repeats
the token is the very failure being caught. The type-error and long-key tests
below hold to the same rule wherever a token is involved.

## `fn a_quoted_run_is_measured_by_where_it_stands() {`

The redaction alone, over messages written out: the shapes serde and the
parser produce, and two that pin a rule — a backtick run after an `expected`
that does not follow an unknown field or variant is measured as a value, and
a quote that is never closed runs to the end of the message.
