# `src/agent/bin.rs`

Extended notes for [`src/agent/bin.rs`](../../../src/agent/bin.rs).

[Source on GitHub](https://github.com/sourcemaps/upstroke/blob/master/src/agent/bin.rs).

The code defines current behavior. These notes preserve contracts and implementation
history. Search each backticked heading fragment separately in the source.

## Module

Naming and invoking an agent CLI — the parts every adapter needs and
none of them should own privately.

**This module used to *locate* the CLI, and it deliberately no longer
does.** An adapter names its CLI; the boundary that will execute it decides
which file that name is. `PR4-ADAPTER-RESOLVES-ON-THE-HOST` in
`reviews/FINDINGS.md` is the entry, [`Invocation::named`] is the repair, and
DESIGN.md:612 is the sentence it serves: "Probes run through that same
runner, **or pre-flight could certify a host CLI/version different from the
one the attempt executes**", with the normal container case being "an image
with version-pinned CLIs" that a coordinator host need not have at all.

Windows is why this module exists. Both agent CLIs ship as npm packages, so
the thing on PATH is frequently a `.cmd` shim rather than a native
executable, and `CreateProcess` cannot exec a batch script. That used to be
handled here by building a `cmd /C` command line by hand and passing it
through `raw_arg`.

**It is not any more, and the reason is worth recording.** `raw_arg` opts
out of everything the standard library does for batch targets, including the
argument escaping added in Rust 1.77.2 for CVE-2024-24576; this crate is
edition 2024, so that fix is unconditionally present. Measured against a
real npm-shape shim, the hand-rolled version expanded `%VAR%` inside
arguments — turning `--allow-tool=shell(echo %PATH%)` into the machine's
entire PATH — while `Command::args` carried every case through intact:
`&`, `|`, `%`, embedded quotes, `^`, spaces, and the empty argument.

Copilot is what made that matter. Its permission surface is argv, so gate
commands — strings a user writes in `upstroke.toml` — now reach a Windows
command line, and a mangled `--allow-tool=shell(<gate>)` is a permission
grant that no longer matches the command it is meant to authorize. The
module comment used to argue that two copies of the quoting logic would be
two chances to get it wrong. The right number was zero.

## `#![cfg_attr(not(test), forbid(clippy::disallowed_methods))]`

The three governed lints are `forbid` in this file's production build,
and `disallowed_methods` is conditional because of what sits at the bottom
of the file.

`forbid` is scoped by the module tree, so an unconditional
`#![forbid(clippy::disallowed_methods)]` reaches the inline
`#[cfg(test)] mod tests`, whose outer `#[allow]` of that lint (the Windows
batch-shim witness, recorded in `effects/allowlist.toml`) is then `E0453`
-- measured by both of #318's first reviews, and by clippy over all targets
at `9bb177ea`-style probes since. `deny` was the first answer, and it is
the wrong one: a `deny` is a level an inner `allow` lowers, and #318's MAIN
review generated such an `allow` with a `macro_rules!` on a production `fn`
here, called it from `engine::topology::integrate::prepared_pin_ref`, and
wrote bytes while all-target clippy exited 0 and every governance test
passed (`PR318-DENY-THE-PRODUCTION-BUILD-COULD-FORBID`).

So the `forbid` is applied in every build without `cfg(test)`: the lib
target that CI's three clippy legs check and that the binary links. Under
it a lowering `allow` in this file is `E0453` however it is written or
generated (`~/orch-pr10/repair-318-r2-evidence/controls-tree/B1-*`,
`controls/c03`, `c08`, `c09`). The lib test target -- the only build the
test module exists in -- gets no `forbid`; its level for this lint is
`-D warnings`', which the test module's allow lowers as it always did, and
which a generated `allow` in the production region lowers there too:
measured by a fully qualified run of the review's witness under
`cargo test --lib` alone, one test selected, one passed, the file written
to a path deleted first (`~/orch-pr10/repair-318-r3-evidence/probes/P4-L1-*`).
The earlier receipt for the same claim, `controls-tree/L1-*` in the
round-2 evidence, selected no test -- a bare name to `--exact` -- and
copied the file its `B1` run had already written; it is VOID and stands
as written. Every gate compiles the production build, and that
is where the lint gate refuses it. The enforcement is clippy's: rustc
resolves no `clippy::` lint, so `cargo build` and `cargo test` compile a
downgrade under this `forbid` as they compile every other fence's violation
(`controls/c15`-`c18`).

`effects::lint_levels::file_level_lint_state` reads this attribute as the
production build's statement, so
`every_classified_module_carries_a_file_level_fence_or_allowance_of_a_governed_lint`
counts the pair stated, and
`no_deny_of_a_governed_lint_is_excused_by_test_code_alone`
is the census that names this file the day it goes back to `deny`.

## `pub struct Invocation {`

An agent CLI as a program string, and how to spawn it.

The field is a [`PathBuf`] rather than a `String` because a program string
**may** be a path: [`Invocation::at`] still builds one from an absolute
path, and [`Invocation::spec`]'s refusal is the boundary at which a path
that a `String` cannot carry is named rather than silently rewritten
(`PR4-PROGRAM-PATH-NOT-UNICODE`). Production constructs only
[`Invocation::named`], whose input is a `&str` and therefore always
representable — so that conflict no longer has a production instance,
while the refusal that documents it stays reachable and tested.

## `impl Invocation` › `pub fn named(name: &str) -> Self {`

The agent CLI as the **boundary that will execute it** names it.

A bare program name, and that is the whole repair. The adapter knows
"an official CLI" (DESIGN.md:117); it does not know which filesystem
will hold it, and until this it answered that question anyway by
resolving against the coordinator host's `PATH` and serialising an
absolute host path into [`CommandSpec::program`]. With one runner whose
boundary *is* the host that was invisible. With a container runner it is
three separate failures, and `PR4-ADAPTER-RESOLVES-ON-THE-HOST` names
them: a CLI pinned in the image and absent on the host was refused
before the runtime was asked anything; every spec carried a path that
names nothing inside the image; and `Caps.version` certified the host's
CLI while the attempt ran the image's.

A bare name is not a new shape for this crate. [`crate::gates::ShellKind::spec`]
has always put one in a spec — `sh`, `bash`, `cmd`, `pwsh` — for every
gate and for the `RunnerPreflight` shell probe, and the host runner has
always executed it. The three agent CLIs were the exception; this makes
them the rule.

**There is no cache and nothing to key.** `probe` and `build` call this,
it is a function of its argument alone, and the two therefore agree by
construction rather than by an ordering between them. That is the
answer to "two runners in one process": a resolution that is correct on
first use and wrong on the second needs a resolution to be *remembered*,
and this remembers nothing.

## `impl Invocation` › `pub fn spec(&self, args: &[String]) -> Result<CommandSpec, UpstrokeError> {`

The command to run, as data: `args` are carried verbatim.

Nothing is quoted, escaped, or wrapped here on purpose: `std` knows
whether the resolved path is a batch shim and applies the right rules,
and every attempt to help it has been a way to get this wrong. The
escaping still happens in exactly one place — the runner, when it turns
this spec into a `Command` — and this returns a
[`CommandSpec`] rather than a `Command` because DESIGN.md:117 says an
adapter "does not decide where the process runs".

[`CommandSpec::program`] is a `String` (DESIGN.md:222), and a resolved
path that is not valid Unicode cannot become one **without becoming a
different path**. So this refuses rather than converting.

The rejected alternative was `to_string_lossy`, and it is worth
recording why: `String::from_utf8_lossy` replaces each invalid byte
with `U+FFFD`, so a `claude` inside a `PATH` directory whose name
carries a non-UTF-8 byte — legal on Unix, where a path is bytes —
arrives at the runner as a path that names *nothing*, and the run dies
at `CreateProcess`/`execvp` with "failed to spawn", pointing at a path
the operator never wrote. Before this slice the `PathBuf` reached
`Command::new` unchanged and that installation ran.

Neither behaviour is "legacy engine behavior unchanged"
(`invariants_preserved[1]`), because the frozen `CommandSpec.program:
String` cannot carry the input at all; the choice is between two ways
of failing. This one fails **at the boundary that cannot represent the
value**, names the path and says why, and cannot be mistaken for a
missing installation. Widening `CommandSpec.program` to an `OsString`
is the repair that would restore the old behaviour, and it is a change
to DESIGN.md:222 rather than to this function.

### Errors

[`UpstrokeError::Refused`] when the resolved path is not valid Unicode.

## `fn boundary_refused`

Rewrite a runner's refusal to execute `name` into something an operator can
act on, saying **where** the CLI is missing.

This is the operator-facing half of the repair, and it is needed *because*
of it. Before, the adapter refused with "claude binary not found on PATH …
install Claude Code" — a true sentence, because the only boundary was this
machine. Now the boundary may be a container image, and "not found" without
"not found *where*" sends the operator to install a CLI on a host that will
never execute it.

It reads this machine's `PATH` **only after the boundary has already
refused**, and only to say which of the two situations the operator is in.
Nothing it returns decides what runs. `install_hint` is the adapter's own
sentence about how its CLI is installed.

## `pub fn extract_version(stdout: &str) -> String {`

First `digits.digits.digits` token wins; otherwise the trimmed first line
verbatim (`--version` formats have churned before, in both CLIs).

## `pub fn extract_version(stdout: &str) -> String` › `.map(|t| {`

Trailing punctuation is not part of a version. The Copilot CLI ends
its line with a full stop — `GitHub Copilot CLI 1.0.78.` — which
otherwise rides along into `Caps.version` and out through every
message that quotes it (`upstroke capacity`, and the probe refusal that
names the version an adapter would not support).

## `impl Invocation {`

Test-only constructors.

Below every production item on purpose: `effects::production_region` cuts a
file at its **first** `#[cfg(test)]`, so a test-only item placed among the
production ones takes the rest of the file out of the wrapper-classification
domain — silently, and `mechanism` (3)'s "every pubfn … is classified" would
then be true of a domain nobody drew. That is `PR5D-VISIBILITY-CHECK-
DUPLICATED`'s shape one level out, and it was measured here: five of this
module's functions left the census the moment a `#[cfg(test)] fn` was added
above them.

## `impl Invocation` › `pub(crate) fn at(path: impl Into<PathBuf>) -> Self {`

An invocation naming `path`, for tests that need one without asking
this machine what it has installed.

Production's only constructor is [`Invocation::named`], whose argument
is a bare CLI name. This exists for the tests that must drive a spec
carrying an **absolute** program — the host runner's own fixture grids
pin the difference between a shell's bare name and an absolute native
executable, and [`Invocation::spec`]'s non-Unicode refusal has no other
input at all.

## `mod tests {`

LEGACY-EFFECT: this test module's Windows batch-shim witness is recorded in
`effects/allowlist.toml`; production carries only CommandSpec data. Its
`#[allow]` is the reason the file's `forbid` of `disallowed_methods` is
conditional on `not(test)` (the prologue's section above): an outer
attribute on an inline module is still under the file's level, so an
unconditional `forbid` would refuse it. The module is Unix and Windows
both -- `a_batch_shim_runs_and_receives_its_argument` is `cfg(windows)`,
and the non-Unicode fixture has an arm per platform -- while the file's
fence attributes are unconditional on platform.

## `fn arguments_reach_the_command_untouched()` › `let args: Vec<String> = [`

The property the deleted quoting code kept breaking. These are the
exact shapes Copilot's permission surface produces: a gate command
with spaces and parentheses, a cmd metacharacter, a percent sign, an
embedded quote, and an empty argument.

## `fn arguments_reach_the_command_untouched()` › `let cmd = build_command(&spec);`

And the same through the runner's own translation, which is what
actually spawns: the spec surviving intact would be worth nothing if
the step that turns it into a `Command` re-quoted it. The `cmd.exe`
raw-tail rule applies to `cmd`, and this program is not it.

## `fn version_extraction_handles_known_formats()` › `assert_eq!(`

Verbatim from the Copilot CLI: the sentence's full stop is not part
of the version, and rode into `Caps.version` when it was not trimmed.

## `mod tests` › `fn a_named_cli_carries_no_location() {`

A named CLI is the name and nothing else — no directory, no extension,
nothing this machine contributed.

The expected values are written here, not read from the adapters: a
constructor compared only against the code that produced it proves
nothing. What is asserted is the *shape* — one path component, not
absolute — because that is the property a coordinator-host resolution
cannot have, on either platform, whatever this machine happens to have
installed.

## `mod tests` › `fn naming_a_cli_is_a_function_of_its_argument_alone() {`

The same name, twice, is the same spec — which is what makes `probe`
and `build` agree without an ordering between them.

The old constructor memoised into a process-wide `OnceLock`, so the
*first* caller in the process decided the answer for every later one.
This asserts the property that replaced it: the constructor is a
function of its argument, so no call can be poisoned by an earlier one.
Both call orders, because "the first caller wins" is a property of
order.

## `mod tests` › `fn a_boundary_refusal_says_where_the_cli_is_missing() {`

A refusal from the boundary says which boundary, and whether this host
has the CLI — the two are different situations with different fixes.

Both branches, and both asserted rather than whichever this machine
happens to take: `upstroke-definitely-not-a-real-binary` is absent
everywhere by construction, and the present branch is driven with a
program every machine of each family has.

## `mod tests` › `fn a_batch_shim_runs_and_receives_its_argument() {`

A `.cmd` shim really does execute, and an argument really does arrive.

Asserting on the constructed `Command` proves we hand `std` the right
thing; only spawning proves `std` then does the right thing with a batch
target, which is the half the old hand-rolled code got wrong.

## `fn a_batch_shim_runs_and_receives_its_argument()` › `std::fs::write(&shim, "@echo off\r\necho GOT:%~1\r\n").expect("write shim");`

`%~1` strips the quotes the child got; a benign argument keeps this
about plumbing rather than about batch re-parsing.

## `mod tests` › `fn a_program_path_a_string_cannot_carry_is_refused_by_name() {`

A resolved path that a `String` cannot carry is refused by name, not
converted into a path that names nothing.

Both platforms have such a path and neither can be spelled in source as
a `&str`: on Unix a path is bytes, so `0xff` is legal and not UTF-8; on
Windows it is UTF-16, so an unpaired surrogate is legal and not UTF-8.
Every other fixture in this module is valid Unicode, which is why the
lossy conversion this replaced survived the suite while changing what a
supported installation did.

## `fn a_program_path_a_string_cannot_carry_is_refused_by_name()` › `assert!(message.contains(rendered), "{message}");`

The operator has to be able to find the entry. `display()` stays
lossy on purpose — it is a diagnostic, not a program.

## `fn a_program_path_a_string_cannot_carry_is_refused_by_name()` › `let fine = invocation("/usr/local/bin/claude")`

And the ordinary case is unaffected: same call, a Unicode path.

## `fn a_program_path_a_string_cannot_carry_is_refused_by_name()` › `let literal = "/opt/upstroke-\u{fffd}/claude";`

A path that legitimately *contains* `U+FFFD` is carried as itself.

`U+FFFD` is an ordinary character in a filename. It is only special
as `to_string_lossy`'s substitution marker, so every conversion that
treats it as one — `to_string_lossy()` followed by a `replace`, the
shape `PR4-SEAMS-004` names — silently renames a directory that
really is called that, and spawns something else or nothing.

Neither fixture above can see it: the refusal fixture's path is not
valid Unicode at all, and the ordinary fixture's path carries no
marker. This is the one input on which "refuse" and "substitute"
still disagree after the refusal is in place.
