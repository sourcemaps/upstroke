# `src/connect.rs`

Extended notes for [`src/connect.rs`](../../src/connect.rs).

These notes retain the module prose from the landed renderer implementation while
applying the comment migration. Source fragments in headings identify the matching code.

## Module

`upstroke connect` (DESIGN.md §13, §18): discover the agent CLIs on this
machine and write `~/.upstroke/pools.toml`.

**Invariant 2 is the one to watch here.** Connect subprocesses the vendors'
own CLIs and parses what they print. No HTTP, no token ever handled, no
credential file read — a vendor CLI talking to its own vendor is the design,
not a leak, and it is the same posture §9 sets for plan importers.

Two things this deliberately does not do:

- **It never invents a profile.** §13 says `connect` does not enumerate
  credential profiles, because no vendor lists them: the mechanism is a
  config-directory environment variable, not a registry. So `connect` writes
  one pool per agent, and `profile` is the operator's to add by hand, which
  `connect` then carries across a `--force`. See [`crate::capacity`]'s module
  docs for what a run does with a profile.
- **It never clobbers.** §17 calls the pools file hand-editable, and it is
  the file that says which subscriptions exist. An existing file whose
  *settings* differ is printed and the command exits asking for `--force`;
  one that already says the same thing reports "unchanged" and rewrites
  nothing. `--force` still carries the operator's own keys across, because
  `profile`, `monthly_allowance` and `endpoint` are things discovery cannot
  supply and replacing the file must not quietly delete.

## `#![allow(clippy::disallowed_methods, clippy::disallowed_macros)]`

LEGACY-EFFECT: this module is in the **frozen legacy section** of
`effects/allowlist.toml`, which carries its justification and the condition
under which the section shrinks. `decisions.effect_site_inventory.mechanism` (2).

## `pub pools_path: Option<PathBuf>,`

Where to write. `None` takes `~/.upstroke/pools.toml`; tests always set
it, so no test can reach the operator's real pools file.

## `pub force: bool,`

Overwrite an existing file that differs.

## `#[derive(Debug, Clone, Copy, PartialEq, Eq)]`

What `connect` did, so the CLI can render it and a test can assert on it
without parsing prose.

## `Written,`

The file did not exist, or `--force` replaced one that differed.

## `Unchanged,`

Configures exactly what is already there, and says exactly the same
thing about it — compared over [`settings_match`] and [`stable_content`]
rather than over bytes.

## `Refused,`

An existing file differs and `--force` was not given.

## `pub content: String,`

The file `connect` produced — written, or merely proposed when it
refused to clobber.

## `pub agents: Vec<AgentReport>,`

One entry per registered adapter, in registry order.

## `pub outcome: Result<Discovery, String>,`

`Err` means this agent contributed no pool. It never aborts the others:
a machine with Claude Code and no Copilot is the normal case, not a
broken one.

## `pub fn run(opts: &ConnectOptions) -> Result<ConnectReport, UpstrokeError> {`

Discover, render, and write — the whole command.

## `pub fn run_with<'a>(`

The injectable form: `adapters` supplies the implementations and `ids` the
registry order, so a test can drive scripted discovery with no CLI on the
machine at all.

### Errors

Returns a refusal when no destination can be determined, an I/O error
when the existing file cannot be read, or a filesystem error naming a
failed directory creation or file write.

## `Some(path) => path.clone(),`

The returned report owns its path independently of these options.

## `let existing_text = match fs::read_to_string(&path) {`

Read before anything is written: `--force` must not silently discard the
keys only an operator can supply.

## `if seen.contains(&id) {`

Two entries for one agent would render `[pools.<name>]` twice, and
TOML rejects duplicate keys — so `connect` would write a file that
`config::load` then refuses to read. The built-in registry has no
duplicates, but `run_with` is the public seam and takes any ids.

## `let runner = crate::runner::host::HostRunner::new();`

Probe first: §14 already treats a missing or broken binary as a
refusal to start, and discovery on a CLI that cannot even report its
version would be reading tea leaves.
Its own host Runner, for the reason `capacity` states: `connect`
drives no run, so there is no run's boundary to borrow, and it is
not a coordinator so its children are outside INV-18's ambient job.

## `let missing = crate::catalog::missing_from(id, &discovery.models);`

D1's cross-check, at the moment the roster's provenance is
being written into the file. Claude Code and Copilot report
no roster today; Codex reports its local `debug models`
catalog. Any real listing is where a stale shipped entry
should first be caught.

## `let outcome = match &existing {`

Two comparisons, because two different questions are being asked.

*May* this file be replaced turns on the **settings** — the operator's
hand edits are what must not be clobbered, and a comment carries none.
*Should* it be rewritten turns on everything except the one genuinely
volatile line, the header's timestamp. Collapsing the two into a single
settings comparison meant a login between two connects reported
`unchanged` and left the file still saying NOT signed in; collapsing them
the other way made every re-connect a conflict resolvable only by
`--force`, the flag that discards hand edits.

## `write_pools(&path, &content)?;`

The write boundary already names the operation and failed path.

## `fn write_pools(path: &std::path::Path, content: &str) -> Result<(), UpstrokeError> {`

Replace the file after the caller has decided that replacement is allowed.
This reports creation and write failures separately; it provides no atomic
publication or durability guarantee beyond the underlying filesystem calls.

## `fn settings_match(existing: &str, proposed: &str) -> bool {`

Compare complete TOML documents, preserving the values of every key.

Parsing handles quoted keys, escapes, multiline strings, and comments with
one grammar. The previous line scanner collapsed whitespace inside strings
and could overwrite an operator's renamed pool. Formatting and table order
do not change parsed settings; integer and float values remain distinct so
comparison never rounds an integer into a different value.

A parse failure means there is no evidence that replacement preserves the
settings. The caller reports Refused unless the operator supplied --force.

## `fn stable_content(text: &str) -> Vec<&str> {`

Everything except the first line when it is the generated timestamp header.

The header records when `connect` ran, so comparing whole bytes would call
two runs a second apart different. Everything else — including every
discovery note and the auth line — is content a reader relies on being
current, so it belongs in the comparison that decides whether to rewrite.

## `fn pool_for_agent(agent: &str, discovery: &Discovery) -> Pool {`

One pool per (agent × discovered account) — today exactly one per agent,
because nothing enumerates credential profiles (see the module docs).

## `let kind = discovery.shape.unwrap_or(match agent {`

§13's default where the CLI could not say: Copilot's post-Jun-2026
billing is credits, and everything else that reports nothing is treated
as a subscription window — the shape whose estimator is the most
conservative of the two. The rendered file carries a comment saying so,
because a default the operator cannot see is a guess wearing a fact's
clothes.

## `Pool::discovered(`

§13's trust order, minus the sources v0.1 does not read: writing
`local-logs` into a fresh file would promise interactive-usage awareness
that has not been built. An operator who wants it recorded can add it —
the parser accepts it and the estimate says it is unread.

## `#[derive(Debug, Default, PartialEq)]`

The keys only an operator can supply, carried across a `--force`.

`connect` discovers subscriptions; it cannot discover *which account*
(`profile`), *how big* an allowance is (`monthly_allowance`), or where a
local model lives (`endpoint`). All three are hand-written, and rewriting
the file without them would delete the operator's own work — with the
refusal message that recommends `--force` never saying so. `profile` in
particular is the entire point of §13's multi-account seam, and
`monthly_allowance` is the only thing that makes a self-metered estimate
possible at all (`Auto` yields `Unknown`).

Since 2026-11-23 there are six: `usage_provider`, `usage_token_env` and
`usage_account` opt a pool into the capacity tracker (§13, §17), and they are
an operator's for the same reason the first three are. Which billing endpoint
answers for a pool, and which environment variable holds the key, are not
things discovery can find, and `connect` writes none of them itself.

The struct is no longer deserialized. `read` fills it from one pool's table,
key by key, because a typed read of the whole `[pools]` table is what let one
mistyped key cost every pool its keys (see `operator_keys`).

## `mistyped: Vec<&'static str>,`

The known keys this pool wrote with a value of the wrong type, in the order
`read` takes them. Only the key's name is kept. `apply` names the key and the
type it needed, and the value goes no further: it may be a credential pasted
under the wrong key, or an array or table of any size.

## `fn read(pool: &toml::Table) -> Self {`

One pool's operator keys, read key by key. A known key whose value has the type
the configuration reader takes for it is carried. That is a string for
`profile`, `endpoint` and the three usage keys, and any value at all for
`monthly_allowance`, whose own check in `apply` (`allowance_of`) already says
what it needs. A key of any other type is left out and its name goes into
`mistyped`, and the pool's other keys are read as if it were not there. Keys
`connect` does not carry (`kind`, `agent`, `sources` and the rest) are never
looked at, whatever their type.

## `fn apply(self, pool: &mut Pool) -> Vec<String> {`

Move the operator's valid keys into the new pool, and say what was not
moved. Each returned line is one problem, worded without the pool's name,
which the caller adds to make the visible warning.

An invalid allowance leaves the discovered Auto default. The usage keys are
carried as a set or not at all: `capacity::UsageSource::from_keys` is the
reader's own validation, so a set it refuses is one the configuration reader
would refuse too, and writing it would produce a pools file that no command
loads. The refusal's text never repeats a `usage_token_env` that is not a
name, since a key pasted there is the usual way to get one.

A key `read` left out for its type comes first, as one problem per key. It
names the key and the type it needed and never the value, so the warning reads
``pool `copilot`: `usage_provider` must be a string; it was not carried``.
The usage keys that did have their type then go through the set check as
though the mistyped one had never been written. A mistyped `usage_provider`
leaves any `usage_token_env` or `usage_account` beside it with no provider,
which is the reader's own no-provider defect, so they are not carried either
and a second line says so. When `usage_token_env` or `usage_account` is the
mistyped key, the provider and whatever else remains are carried if they pass
the check. Where the carried set then lacks what its provider needs (a
credential, or an account for a provider that reports per account), the
configuration reader warns about the gap and `upstroke capacity --refresh`
skips the pool, so it never asks about a different account than the one
written.

## `Err(error) => problems.push(format!("{error}; using auto")),`

The error identifies this setting; run_with supplies the pool
context and decides how to report the rejected value.

## `Ok(crate::capacity::Allowance::Units(*units as f64))`

Every positive i64 fits the finite f64 range. Conversion uses
the same capacity representation as config::read.

## `fn operator_keys(text: &str) -> std::collections::BTreeMap<String, OperatorKeys> {`

Pull the operator-written keys out of an existing pools file, by pool name.

Read pool by pool and key by key. The document is parsed as a plain TOML
table, and `OperatorKeys::read` reads each table under `[pools]` on its own.
A key with the wrong type is left out of its own pool's carried keys and
nothing else: that pool keeps its other valid keys, every other pool is read as
if the key were not there, and `apply` turns the key into one warning naming
the pool, the key and the type it needed.

The typed read this replaced is PR409-CONNECT-USAGE-TYPE-DROPS-SETTINGS. It
deserialized the whole `[pools]` table in one go and turned any failure into an
empty map. So `usage_provider = 42` in one pool sent `--force` on to rewrite
every other pool without its `profile`, `monthly_allowance` and `endpoint`, and
nothing said so. A mistyped `profile` or `endpoint` did the same before the
usage keys existed. The configuration reader refuses a file holding such a key,
because it reads each of these keys as a string, so every command that loads
pools stops on it. `--force` is how an operator gets a loadable file back, so
it is the command that must not cost the other pools their keys.

Still lenient about the document as a whole, on purpose: a file that is not
TOML at all is one `--force` was always going to replace, and failing the whole
command over it would be worse than losing keys that were unreadable anyway. A
`[pools]` that is not a table, or an entry under it that is not one, has no
keys to carry and is skipped the same way. A pool enters the map if it has an
operator key or a mistyped one, so a pool whose only operator key was mistyped
still gets its warning.

Only a pool `connect` writes has its keys applied, so only that pool's mistyped
keys are reported. A pool whose agent was skipped, or one the operator renamed,
is not written at all. The refusal already says that anything not in the
proposed text is lost, and a warning that one of that pool's keys "was not
carried" would suggest the rest were.

## `fn default_pool_name(agent: &str) -> &str {`

The pool name for an agent: the agent's own id.

Deliberately not a plan name. Naming every Claude Code pool `claude-max`
asserted a subscription tier discovery never established — a Pro subscriber,
or someone on API-key billing, got a pool claiming a plan they do not have,
in the one file whose whole purpose is to describe their actual
subscriptions, from a module that marks its other defaults as defaults. It
also put a per-agent alias table here, so adding an adapter meant editing
`connect`. Renaming the pool is the operator's call, and the file is
hand-editable precisely so they can make it.

The name this returns is frozen (§17): it never changes across versions.
`--force` carries `profile`, `monthly_allowance` and `endpoint` over by pool
name, so a default that changed would stop carrying an operator's keys into
the pool it now names. `the_pool_keys_and_the_default_pool_names_are_frozen`
pins each agent's name.

## `pub fn render_report(report: &ConnectReport) -> String {`

What the CLI prints.

The body is `render::report`. This name stays here because it is the one
`main` calls and the one `effects/wrappers.toml` classifies, and moving it
would change a public path and a census anchor rather than a file boundary.

## `impl ConnectReport {`

A refusal to clobber is not an error the operator can fix by retrying, and
exit status is how a script tells the difference.

## `struct FakeAdapter {`

A scripted stand-in, so these tests run on a machine with no agent CLI
installed at all.

## `FakeAdapter {`

Installed nowhere: the normal single-vendor machine.

## `let old = first.content.replace(`

The previous renderer used f64 Display, which spells 1e16 as this
valid i64. TOML integers and floats remain distinct in comparison.

## `let annotated = format!("{}{} operator note\n", first.content, render::WRITTEN_BY);`

The first line is the only generated timestamp header. A later
comment using the same words remains part of the content comparison.

## `let tree = scratch("roundtrip");`

The round trip is the whole contract: a file this command writes must
be one `config::load` accepts, or `upstroke capacity` reports on
something `connect` cannot produce.

## `fn the_pool_keys_and_the_default_pool_names_are_frozen() {`

§17's frozen rule, pinned with every value spelled out rather than derived
from the code it checks. One pool sets all ten keys the configuration reader
accepts, each to a value the reader could not have supplied as a default, and
`config::load` must read every one back without a warning. Each agent's
default pool name is listed beside the agent. The test sits here because
`default_pool_name` is private to this module, and the keys go through the
same public reader as `upstroke capacity` and `upstroke run`.

A failure is a change the rule forbids. The rule lets a new key be added
beside the old ones, and an added key leaves this test passing, since it
names only the frozen keys. A renamed or dropped key fails it, and so does a
changed default name, which would break `--force`'s carrying by pool name.

## `let tree = scratch("clobber");`

§17 says the file is hand-editable, so silently overwriting a hand
edit destroys the operator's own record of their subscriptions.

## `let forced = connect(&path, true);`

--force is the escape hatch, and it really does replace — but it
carries the operator's own keys across. `profile` is the whole point
of §13's multi-account seam and discovery cannot supply it, so a
replacement that dropped it would silently delete the one setting
the refusal above existed to protect.

## `let tree = scratch("escapes");`

§13 calls `profile` a config-directory path, and on Windows a path
holds backslashes. The parent parses the operator's spelling and the
renderer writes the value back; written raw, `\U` and `\.` are TOML
escapes, so `--force` produced a file `config::load` refused with a
parse error and the next `connect` read no keys from — the loss the
carrying exists to prevent, on the path that recommends `--force`.

## `let again = connect(&path, false);`

The written spelling reads back into the same keys, so a second
connect carries them again and finds nothing to rewrite — which is
the round trip the two comparisons in `run_with` depend on.

## `let machine = Machine {`

Pass 1 of PR #168: `run_with` is public and takes any adapter id, and
the renderer now quotes a name that is not a bare key, so an id
holding `"` followed by `#` writes `[pools."x\"#A"]`. A `strip_comment`
that read `\"` as the closing quote cut both that header and the
operator's renamed `[pools."x\"#B"]` down to `[pools."x\"`, called
the settings equal, and — since the text differed — rewrote the file,
undoing the rename without `--force`.

## `let tree = scratch("unparseable");`

`operator_keys` reads the existing file leniently: one it cannot
parse carries no keys at all. The refusal must therefore not tell the
operator their keys "are carried" — pass 1 of PR #168 found that it
did — but send them to the proposed text, which is what `--force`
writes, and say when carrying happens.

## `let tree = scratch("idempotent");`

The header names the write date, so a byte comparison would call
every second run a conflict — and the only way past a conflict is
`--force`, the flag that discards hand edits. A refusal an operator
is trained to bypass protects nothing, so the comparison is over
settings, not bytes.

## `fs::write(&path, format!("# my own note\n{first}")).expect("annotate");`

A comment-only difference is never a *conflict* — settings are what
may not be clobbered — but it is a rewrite, because the comments are
where discovery's findings live. The trade is deliberate: a note an
operator adds is regenerated away, and in exchange a login between
two connects cannot leave the file insisting they are signed out.
Their real edits (`profile`, `monthly_allowance`, `endpoint`) survive
both paths — see `an_existing_file_that_differs_is_never_clobbered`.

## `let tree = scratch("relogin");`

Auth state is rendered only as a comment, so a settings-only
comparison reported `unchanged` and left the file telling an operator
who had just logged in that they were not signed in.

## `let machine = Machine {`

D1's guard. It cannot fire against a real CLI today — neither
enumerates models — so it is driven through a scripted discovery
that does, which is the shape the check exists for.

## `models: [`

A roster that has moved on without the catalog.
Overlaps the roster — zero overlap is a format
mismatch, not a stale catalog — but has moved on from
the frontier slug the second opinion depends on.

## `let machine = Machine {`

The Copilot case: §13 gives it two billing shapes and the CLI
distinguishes neither. A default is fine; a silent default is not.

## `let runner = crate::runner::host::HostRunner::new();`

§13's discovery is a claim about a real CLI, so it is checked against
one where the machine has it — and skipped cleanly where it does not,
which is the shape every other binary-touching test here takes.

## `assert!(`

Whatever it answers, it must be one of the three states and it must
explain itself — including when the answer is "could not tell".

## `fn both_agents_signed_in() -> Machine {`

Two usable agents, so `connect` writes two pools. Only a pool it writes has its
keys applied, so a mistyped key in one pool and the keys of another can only be
watched together on a machine that writes both.

## `fn mistyped_usage_key_must_not_delete_other_pools_profiles() {`

The witness from PR409-CONNECT-USAGE-TYPE-DROPS-SETTINGS's review, ported.
`usage_provider = 42` in `copilot` used to make the whole read fail, and
`--force` then wrote `claude-code` without its `profile`, `monthly_allowance`
and `endpoint`, with no warning. The test reads those three back through
`config::load`, pins the one warning word for word, and finds it as a
`warning:` line in what `render_report` prints, which is the text the operator
sees.

## `fn review409_bad_usage_type_preserves_other_pools_operator_keys() {`

The patch's witness, widened to every usage key and to each kind of TOML value
that is not a string: an integer, a float, a boolean, a date-time, an array and
an inline table. Each value carries a marker that would show if a warning
repeated it.
The marker is checked against the warning and not the file, because `true` is
also how the file spells `weekly`. A usage key alone in its pool, once left
out, leaves no set to carry, so the file gains no `usage_` line.

## `fn a_pool_whose_own_usage_account_is_mistyped_keeps_its_own_profile() {`

The rule holds inside the bad pool too. Its `profile` is carried, and so are
the usage keys that had their type: `usage_provider` and `usage_token_env`
without the account pass the reader's set check. The configuration reader then
reports the missing account as a gap, and `--refresh` skips the pool until the
account is written as a string.

## `fn operator_keys_are_read_pool_by_pool_and_key_by_key() {`

The reader alone, on one document holding each case: a pool with three
mistyped keys beside two good ones, a pool with only good ones, a pool with no
operator key, and an entry under `[pools]` that is not a table. A `[pools]`
that is not a table, and a file that is not TOML, still carry nothing.
