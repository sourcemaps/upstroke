# `src/agent/adapter.rs`

Extended notes for [`src/agent/adapter.rs`](../../../src/agent/adapter.rs).

The code is the authority for what it does; this file is the whole of its prose, moved out of
the source verbatim. Each section is headed by the line of code the comment sat above, spelled
as it is in the source, so the heading is the grep string that finds the code.

## Module

The agent adapter contract and its registry: `AuthState`, `Discovery`, `Caps` and `TaskRun`; `probe_workspace` and `probe_request`; the `AgentAdapter` and `AdapterSource` traits, `BuiltinAdapters`, `ADAPTERS` and `by_id`; the help-text readers; and the tests that hold the three shipped adapters to one supervision table. Every item was `src/agent/mod.rs`'s until #318's third round moved it here, unchanged, and re-exported it from the root (`pub use adapter::*`), so `upstroke::agent::AgentAdapter` and the rest resolve as they did.

## `#![forbid(`

The three governed lints are `forbid` here: nothing in this file, its test
modules included, allows a governed lint (the file has no row in
`effects/allowlist.toml`), so the fence compiles, and under it a downgrade
however written or generated is `E0453` at the lint gate. The file exists so
that the module root can hold nothing but declarations and re-exports: a
`deny` at the root is a level an inner `allow` lowers, and a root that holds
production code under a `deny` was an open host for the route
`GUARD-DECISION-SILENT-PRODUCTION-FILES-OUTSIDE-THE-ROLL-CALL` recorded --
executed on the fenced tree in #318's third round and closed by this move
(`~/orch-pr10/repair-318-r3-evidence/plan-class/M4c-*`).

## `pub enum AuthState {`

Whether the vendor's CLI says it is signed in.

Three states, not two. "Could not tell" must never render as "not
connected": `upstroke connect` writes a file an operator then trusts, and a
confident *wrong* "you are not logged in" sends them to re-authenticate an
account that was fine.

## `impl std::fmt::Display for AuthState {`

One rendering, used by `connect` and `capacity` alike.

There were two: a terse `Display` here and a fuller `describe_auth` in
`connect`, so the same fact read as "not authenticated" from one command and
"NOT signed in — log in with the vendor's own CLI before running" from the
other, and an operator comparing them could not tell whether they described
the same thing. The rule this enum exists to enforce — "could not tell"
never renders as "not connected" — was then enforced in one place and merely
observed in the other.

## `pub struct Discovery {`

What one agent's CLI could be got to say about itself, without the network
and without touching a credential (invariants 2 and 5).

## `pub struct Discovery` › `pub models: Vec<String>,`

The models the CLI itself advertises.

Empty on Claude Code and Copilot today: as of Aug 2026 neither offers
non-interactive model enumeration. Codex exposes its local roster via
`debug models`; its adapter validates model × effort support at probe
and reports the slugs here. The seam lets every real listing be
cross-checked against the shipped catalog; [`Caps::model_list`] is the
gate.

## `pub struct Discovery` › `pub shape: Option<PoolKind>,`

§13's pool-kind hint, read from whatever the CLI says about the account
it is signed into. `None` means it said nothing conclusive, and the
caller picks a documented default rather than guessing.

## `pub struct Discovery` › `pub notes: Vec<String>,`

Everything the operator should know about how this was worked out —
including what could not be.

## `impl Discovery` › `pub fn unknown() -> Self {`

What an adapter that does not implement discovery reports: nothing,
said out loud.

## `pub struct Caps {`

Capabilities discovered by `probe()` at pre-flight (§14). Copilot's CLI
has shipped breaking flag removals, so capability probing is load-bearing,
not decorative.

## `pub struct Caps` › `pub version: String,`

Version string as reported by the binary, best-effort.

## `pub struct TaskRun {`

Everything an adapter needs to build one attempt's subprocess. The engine
materializes the prompt (§14: body + acceptance + artifacts + conventions
brief) — adapters never re-derive it.

## `pub struct TaskRun` › `pub prompt: String,`

Fully materialized prompt, delivered on stdin.

## `pub struct TaskRun` › `pub workspace: PathBuf,`

Working directory for the subprocess (the workspace repo root).

## `pub struct TaskRun` › `pub gate_cmds: Vec<String>,`

The gate commands this profile may run, and nothing else (§20). Empty
for reviewers, which run nothing at all.

Carried on the run rather than only handed to
[`AgentAdapter::materialize_permissions`] because not every agent has a
settings file to put them in: Copilot's permission surface is argv, so
its `build` needs them at command-construction time.

## `pub struct TaskRun` › `pub resume_session: Option<String>,`

Same-rung retry: resume this session with feedback instead of starting
fresh (§11.4).

## `pub struct TaskRun` › `pub settings_path: Option<PathBuf>,`

Per-run permission settings file, materialized by the engine from
[`claude::permission_settings`]-style generators (§20).

## `pub fn probe_workspace() -> PathBuf {`

Where a pre-flight process runs.

The coordinator's own working directory, which is exactly what a probe
inherited before probes went through the Runner — a probe asks a CLI about
itself and has no workspace of its own. Absolute rather than `"."` because
`runner::host::HostRunner::run` clears the environment, and on Windows the
`=X:` drive-relative variables go with it; every process it starts is given
an absolute directory so none of them can be resolving a drive-relative
path.

## `pub fn probe_request(`

One pre-flight process of `agent`, as a [`RunnerRequest`].

`decisions.pr_sequence[5].scope`: "**probes**, workers, gates, reviews go
through the Runner", and INV-18 accounts an agent probe the way it accounts
an attempt — "every agent CLI invocation **incl. agent probes** acquires
its atomic {agent, pool?} pair" — so the role is `probe(<agent>)`, it is
slotted, and `agent` is set so `host-v1` supplies that agent's credential
location (a probe that could not see the credential directory would certify
a CLI in a state the attempt never runs in).

`ordinal` is **which of this adapter's pre-flight processes this is**. A
pre-flight that runs `--version` and then `--help` runs two processes, and
"unique per process" is the packet's property, so each adapter fixes a
named ordinal per step rather than counting: a counter would renumber every
later step the first time an earlier one was skipped (codex's binary
resolution caches, so its second call skips one), and the identities of one
machine's pre-flight would stop being a function of the pre-flight.

### Errors

[`UpstrokeError::Refused`] when the adapter id cannot appear in an invocation
identity — see [`InvocationId::probe`]. Every shipped id is `[a-z-]`.

## `pub trait AgentAdapter: Send + Sync {`

DESIGN.md §8 `AgentAdapter`.

## `pub trait AgentAdapter: Send + Sync` › `fn probe(&self, runner: &dyn Runner) -> Result<Caps, UpstrokeError>;`

Locate the binary and report version + capabilities. Ran at pre-flight;
a missing binary is a refusal to start, not a task failure (§19).

Takes the runner because DESIGN.md:209 does — `probe(&self, runner:
&dyn Runner)`, annotated "probes the boundary that will execute" — and
DESIGN.md:612 says why: "Probes run through that same runner, or
pre-flight could certify a host CLI/version different from the one the
attempt executes."

### Errors

A missing or unusable binary, a CLI that has dropped a required flag,
or a runner refusal.

## `pub trait AgentAdapter: Send + Sync` › `fn build(&self, run: &TaskRun) -> Result<CommandSpec, UpstrokeError>;`

Turn one attempt into a **data-only** [`CommandSpec`].

DESIGN.md:117: an adapter "does not decide where the process runs". A
`build` that returned a live `std::process::Command` could carry a cwd,
an environment, or a spawn past the runner, and PR6's container runner
would inherit the hole — so what comes back is a value with a program,
arguments, an environment **overlay** and stdin bytes, and nothing that
names a machine.

### Errors

A refusal to run this profile at all (§19/§20), or a binary that cannot
be located.

## `pub trait AgentAdapter: Send + Sync` › `fn parse(&self, out: &ProcessOutput) -> Result<Outcome, UpstrokeError>;`

Read one attempt's process output as an [`Outcome`].

### Errors

Output this adapter cannot interpret at all.

## `pub trait AgentAdapter: Send + Sync` › `fn discover`

§13's `upstroke connect`: ask this agent's CLI about the account behind
it — signed in or not, what shape its quota is, which models it offers.

Subprocesses the vendor's own CLI and parses what came back. No HTTP, no
token ever handled, no credential file read: a vendor CLI talking to its
own vendor is the design (invariant 2), the same posture §9 sets for
plan importers.

Takes the `Caps` the caller already probed rather than re-probing:
discovery always runs beside a probe (a CLI that cannot report its own
version is in no state to be asked about its account), and an adapter
that called `probe()` again spawned `--version` and `--help` a second
time — four subprocesses where two would do, each carrying the probe
timeout.

The default reports nothing rather than being required, so an adapter
cannot silently claim discovery it does not do — [`Discovery::unknown`]
is an honest "could not tell", and every consumer treats it as one.

### Errors

Whatever asking this CLI about its account failed with. The default
never fails: it asks nothing.

## `pub trait AgentAdapter: Send + Sync` › `fn stdin_payload<'a>(&self, run: &'a TaskRun) -> &'a str {`

What to write to the child's stdin. Delivery is the adapter's call:
CLIs that take the prompt as an argument instead return empty here.

## `pub trait AgentAdapter: Send + Sync` › `fn materialize_permissions(`

Materialize this agent's permission surface (§20) into `dir`, returning
the file the command should reference. Claude Code writes a settings
JSON; Copilot will encode permissions as argv flags and write nothing.

## `pub trait AdapterSource {`

Where a caller finds agent adapters. Injectable so the engine, `connect`
and `capacity` are all fully testable without any real agent CLI on the
machine.

Lives here rather than in `engine` because resolving an adapter id has
nothing to do with running a plan: `capacity` documents itself as a pure
estimator over plain values, and `connect` executes nothing at all, yet both
had to import the execution engine for this two-line trait.

## `pub trait AdapterSource` › `fn shared(&self) -> Option<Arc<dyn AdapterSource + Send + Sync>> {`

An owned, thread-shareable handle on the same registry, or `None`.

The schema-4 coordinator (`engine::topology::coordinator`) runs each
pipeline on a thread of its own and hands every one an
`Arc<dyn AdapterSource + Send + Sync>`, while the public entry points
(`run_with`, `run_harness`, `resume_with`, `resume_harness`) take a borrowed
`&dyn AdapterSource`. A borrow cannot become an owned, `'static` handle, so
a source says whether it can give one. The default is `None`: a test
double that holds borrowed state, or one that is not `Sync`, keeps
compiling unchanged and simply cannot be shared. The production entry
(`engine::topology::entry`) refuses a run wider than one before any lock
when its source answers `None`, and drives a width-1 run on the
single-threaded step loop instead (PR12's D4).

## `impl AdapterSource for BuiltinAdapters` › `fn shared(&self) -> Option<Arc<dyn AdapterSource + Send + Sync>> {`

The built-in registry is a unit struct over a `static` table, so sharing it
is a fresh `Arc` of the same zero-sized value: every handle resolves every
id to the same `&'static dyn AgentAdapter`.

## `pub static ADAPTERS: &[&dyn AgentAdapter] = &[`

Registry in routing order; ids match `WorkerProfile.agent`.

## `pub(crate) fn missing_effort_levels(help: &str) -> Vec<Effort> {`

Shared effort levels the help entry for `--effort` actually advertises.

Looking only for the flag proves too little: several CLI versions exposed
`--effort` with a narrower enum. The option's own wrapped help block is
parsed so unrelated words elsewhere in `--help` cannot masquerade as a
supported value.

## `pub(crate) fn advertises_flag(help: &str, flag: &str) -> bool {`

Whether help advertises `flag` as a whole option token.

Short flags need this instead of substring search: `-p` occurs inside
`--permission-mode`, and `-s` inside several unrelated long options.

## `pub fn looks_rate_limited(text: &str) -> bool {`

Rate-limit signals are ground truth for the capacity engine (§13), so both
adapters read from one vocabulary rather than two that drift apart.

Phrases cover the subscription-window wording Claude Code prints ("5-hour
limit reached", "Weekly limit reached"), Copilot's credit and premium-request
wording (§13's two billing shapes), and API-level errors underneath either.

Claude Code has since changed that wording. In run
`01M740JX5N8HB94RNC7W2MM777` (2026-11-11), the CLI exited with code 1 and
the event log recorded `You've hit your 5-hour limit · resets 5:30am (UTC)`.
None of the twelve phrases matches that text. Before this rule existed,
twenty-one attempts in that run were read as `AgentError` and spent an
attempt each, when they should have been read as `RateLimited` and deferred.
Twelve tasks parked without a commit.

So one rule sits beside the phrases, in `hits_a_quota_window`: the text, in
lower case, contains "hit your " followed immediately by a quota window and
then " limit" as a whole word (the boundary is set out in the next section).
There are three windows that signal on their own: a number of hours, written
as one or more ASCII digits and then "-hour" ("5-hour"); "session"; and
"weekly". A fourth form signals only with a reset: any other name of one
word of ASCII letters, such as a model's ("Fable", "Opus"), when the next
word after "limit" is "resets" ("hit your Fable limit · resets Nov 14, 9am
(UTC)"). Every occurrence of "hit your " is checked, not only the first, so
an ordinary "the patch hit your branch" earlier in the text does not hide a
limit message after it. "hit your usage limit" needs no window: "usage limit"
is one of the twelve phrases.

The rule is narrow on purpose. A rate-limit signal is ground truth about
capacity (§13): the ladder does not count the attempt, defers it, and after
three deferrals asks a human. So "You've hit your context-window limit",
"hit your rate of change limit", "hit your token limit" with no reset after
it and a bare "hit your limit" do not signal. They are task failures, to be
retried or escalated like any other.

The rule splits the already lower-cased text on "hit your " and reads each
piece with `trim_start_matches`, `strip_prefix`, `strip_suffix` and
`starts_with`. Those return slices of the same string or `None`, so it
allocates nothing more, and nothing in it indexes or unwraps, so it cannot
panic.

Three forms have been observed. The five-hour form is the one from run
`01M740JX5N8HB94RNC7W2MM777` above (2026-11-11, exit code 1). The session
form, `You've hit your session limit`, is in `reviews/FINDINGS.md`, in the
record of a worker that returned rc=1 with that message while its wrapper
reported success. The model form, `You've hit your Fable limit · resets Nov
14, 9am (UTC)`, was printed on the build machine on 2026-11-12, when one
model's weekly allowance was spent. The weekly form is covered on the
expectation that it is worded alike. It has not been observed. A limit
message in any other wording is not recognised and is read as `AgentError`.
That spends an attempt, which is the safer mistake: a false rate-limit
signal would tell the capacity engine an untouched pool is exhausted.

Only ever consulted for a FAILED attempt: a successful task *about* rate
limiting ("added backoff for 429 responses") must never be read as the pool
being exhausted, or verified work gets rolled back.

## `fn hits_a_quota_window(lower: &str) -> bool {`

The quota-window rule described in the section above. Each piece after
"hit your " is read one of two ways:

- If it starts with ASCII digits, the window is those digits and "-hour".
- Otherwise the window is the run of ASCII letters it starts with, which
  must not be empty. "session" and "weekly" signal on their own. Any other
  word, a model name such as "fable" or "opus", signals only if
  `says_when_it_resets` holds for what follows the limit.

Either way the window must be followed by " limit" as a whole word, which
`after_a_whole_limit` checks. The window is one word: "context window" and
"context-window" stop at the space or the hyphen, which is not " limit", and
"fable4" stops at the digit. So none of them signals, with a reset or
without.

The model form was added after the pull request that reads resets from limit
messages. Its parser test read the observed Fable message with
`reset_at_from_limit_message` directly, and nothing checked that the message
was a rate limit at all. It was not: none of the twelve phrases matches it,
and this rule then accepted only the three windows. So an attempt that
printed it was read as `AgentError`, the coordinator never recorded
`pool_exhausted`, and the ladder kept spending attempts against an exhausted
pool.

A name signals only with a reset because, unlike "session" and "weekly", it
is not a known window. "token" and "context" are names too, and "You've hit
your token limit" with nothing after it is about the prompt, not the pool.
The reset is what makes it a quota: the message says when the allowance comes
back. "session", "weekly" and the hours window are unchanged, and still
signal with or without a reset.

## `fn after_a_whole_limit(window_end: &str) -> Option<&str> {`

What follows " limit" right after a window, if "limit" is a whole word
there. "limit" must end at the end of the text, or at a character that is
not an ASCII letter, ASCII digit or underscore. A full stop, a space, a
semicolon or the " · resets" of the observed five-hour message all end it;
"limiter", "limitation" and "limited" do not, and give `None`.

The boundary was added after a round-two review of the first form, which
checked only that the rest began with " limit". That accepted "limiter",
"limitation" and "limited" as well, so a failed attempt whose error read
`The request hit your session limiter and returned an internal error.` was
read as `RateLimited`: the ladder did not count it, deferred it three times
and asked a human, when a real failure should be retried or escalated.

The check strips " limit" with `strip_prefix` and asks `starts_with` whether
the next character is a word character. Both work on slices of the same
string, so the rule still allocates nothing more and cannot panic. The
character test is ASCII only, like the lower-casing before it.

## `fn says_when_it_resets(after_limit: &str) -> bool {`

Whether the next word after the limit is "resets", followed by a space, as
in " · resets Nov 14, 9am (UTC)". Only characters that are neither letters,
digits (in any script) nor underscores may come between: spaces, `·`, `∙`,
punctuation. So "hit your token limit; the cache resets 9am (UTC)" does not
count: the next word is "the".

The test is for the word only, not for a reset that parses. A reset in
another zone, which `reset_at_from_limit_message` refuses, still says the
allowance comes back, so the message is still a rate limit; its `reset_at`
is just unknown. "resets " with a space is the same word
`reset_at_from_limit_message` looks for.

The gap test uses Unicode letters and digits, not ASCII ones, so that a word
in another script between "limit" and "resets" is a word and stops the
match, rather than being skipped as punctuation.

## `pub fn reset_at_from_limit_message(text: &str, now_unix: u64) -> Option<String> {`

When a usage limit says it lifts, read from the limit message itself, as an
RFC 3339 UTC timestamp (`YYYY-MM-DDTHH:MM:SSZ`). The coordinator records it as
`reset_at` on the `pool_exhausted` event.

Two messages carry one. Both were printed by Claude Code on the build machine:

- `You've hit your 5-hour limit · resets 5:30am (UTC)` (2026-11-11);
- `You've hit your Fable limit · resets Nov 14, 9am (UTC)` (2026-11-12).

So a reset is the word `resets` and a space, then an optional date, then a
time of day, then a space and `(UTC)`. The date is a three-letter month
(`Jan` to `Dec`), a space, a day of the month in one or two digits, a comma
and a space. The time of day is an hour from 1 to 12 in one or two digits,
optionally `:` and exactly two minute digits, then `am` or `pm`. `12am` is
midnight and `12pm` noon. The spelling is matched exactly, case included, as
it was printed.

Neither message gives a year, and the time-only one gives no date. Both are
completed against `now_unix`, the moment the message is read, by one rule:
the reset is the occurrence nearest to `now_unix`. There are three
candidates, in order:

- for a time only, that time yesterday, today and tomorrow;
- for a month and day, that date at that time in the year before
  `now_unix`'s, in its year and in the year after. A candidate that is not a
  real date, such as 29 February in a common year, is not a candidate, and
  if none is, the answer is `None`.

The candidate with the smallest absolute distance from `now_unix` is the
reset, and on a tie the later one. So a message read well before its reset
gets the coming one: `5:30am` read the evening before is the next morning's,
and `Jan 2` read on 30 December is the coming 2 January. A message read just
after its reset keeps its own reset, a moment in the past, rather than one a
day or a year away: `5:30am` read at 05:30:01 is that morning's, and
`Dec 31, 11pm` read at 00:30 on 1 January is the day before's, in the year
before. Either way the reset is never more than half a day from `now_unix`
for a time only, nor more than half a year for a date every year has.
`Feb 29` is the exception, since at most one of the three years has it: read
in 2027 it is 29 February 2028, more than a year ahead; read in 2029 it is
29 February 2028, in the past; read in 2026, with no leap year among 2025,
2026 and 2027, it is `None`.

This rule replaced two, after the frontier review found a defect in each. A
time only was the first such time at or after `now_unix`, so a message read
a second after its reset was recorded a day ahead. A month and day was in
`now_unix`'s year unless that was more than a day past, so the previous year
was never a candidate and a `Dec 31` reset read on 1 January was recorded a
year ahead.

`None` means the text gave no reset this can stand behind: no `resets` at
all, a zone other than `(UTC)`, or any part that does not parse (an hour
outside 1 to 12, minutes outside 0 to 59 or not two digits, a month not in
the list, a day the month does not have, a missing comma or space). A
reset in another zone is refused rather than converted: converting
`(Europe/London)` needs a time-zone database, which is a dependency, and a
guess at the offset would be a wrong timestamp recorded as fact. `None` is
what `reset_at` held before this existed, and the estimate already says
what it means: the reset is unknown.

If several resets appear, the first is read, and only the first. A text
whose first reset is malformed gives `None` even when a later one parses:
the first is the one the message gave, and a later one may be about
something else.

It never panics, on any text and any `now_unix`. The text is read with
`match_indices`, `strip_prefix`, `starts_with`, `get` and
`split_at_checked`, each of which returns a slice of the same string or
`None`; the only indices come from `find` and `match_indices`, so they
fall on character boundaries. The clock arithmetic is checked: a
`now_unix` past `i64::MAX`, or a candidate whose seconds would overflow,
gives `None`. So does a nearest candidate before 1970, which has no unix
time to format; only a clock read near the epoch can meet one.

## `fn first_reset(text: &str) -> Option<&str> {`

What follows the first `resets ` that begins a word: at the start of the
text, or after a character that is not an ASCII letter, ASCII digit or
underscore, the boundary `after_a_whole_limit` uses. Without it,
`loaded presets 5:30am (UTC)` would read as a reset.

## `const MONTHS: [&str; 12] = [`

The twelve abbreviations, in order, so the position gives the month's
number. `Sept` and `November` are not in it, and are refused rather than
guessed at: neither has been printed.

## `fn leading_month(text: &str) -> Option<(i64, &str)> {`

A month abbreviation and the space after it. `None` here is not yet a
refusal: the reset may be a time only. A text that starts with neither a
month nor an hour is refused by `time_of_day`.

## `fn leading_digits(text: &str) -> Option<(i64, &str)> {`

One or two ASCII digits at the start of the text, as a number. Three digits
or none are refused, and so is a sign, which `str::parse` would otherwise
accept.

## `fn time_of_day(text: &str) -> Option<(i64, &str)> {`

The time of day as seconds after midnight, and what follows it. `hour % 12`
is what makes `12am` midnight and `12pm` noon: the twelve wraps to zero and
`pm` adds twelve hours.

## `fn epoch_day(year: i64, month: i64, day: i64) -> Option<i64> {`

The day number of a date, counted from 1970-01-01, or `None` when the month
does not have that day. `util::days_from_civil` counts a missing day on into
the next month, so the date is converted back with `util::civil_from_days`
and must come back unchanged. That is how `Feb 29` is a candidate in 2028
and not in 2027, and how `Nov 31` is a candidate in no year, so it gives
`None`. It returns days rather than seconds so that a date the month lacks
and a time that overflows stay apart: the first only drops a candidate, the
second refuses the whole reset.

## `mod tests` › `fn a_successful_answer_from(id: &str) -> String {`

A stdout every adapter would call a **success**, written in that
adapter's own answer shape.

Load-bearing rather than convenient: it is what makes the supervision
grid below hostile. With a failure payload, dropping the supervision
checks would still report `AgentError` and the cells would pass for the
wrong reason.

## `mod tests` › `fn every_adapter_maps_every_supervision_result_the_same_way() {`

**Every** adapter maps **every** supervision result the same way.

`invariants_preserved[0]` is "process supervision, timeout, output
capture, **adapter parsing** unchanged", and the supervisor's two flags
are inputs to parsing, not to it alone: `output_limited` means the tree
was terminated with the transcript truncated, and `timed_out` means it
was terminated for exceeding its wall clock. A truncated transcript
authorizes nothing (`PR5-CORRECTNESS-013`) and a timeout is a distinct
ladder input from a generic agent failure (`PR5-CORRECTNESS-014`).

The domain is `ADAPTERS`, from the type, and the expectations are
literals — a table, not a re-derivation of the branch order. That is the
point: the two rows where an exit code of 0 meets a set flag are exactly
the rows a re-derivation would get wrong in the same direction the code
would.

Claude and Copilot had direct flag-to-status tests; Codex's flag
fixtures exercised its strict-config *preflight validators*, so no test
had ever parsed an output-limited or timed-out Codex execution. That is
the "guarantee proved for the variant that was looked at" class, and the
guard for it is a domain taken from the type.

## `fn every_adapter_maps_every_supervision_result_the_same_way` › `type SupervisionCell = (`

One supervision shape: name, exit code, `timed_out`,
`output_limited`, the status every adapter must report, and a
substring the detail must carry.

## `fn every_adapter_maps_every_supervision_result_the_same_way` › `assert_eq!(outcome.duration, out.duration, "{cell}: duration");`

The duration is the supervisor's, on every route.

## `fn every_adapter_maps_every_supervision_result_the_same_way` › `assert_ne!(OutcomeStatus::Timeout, OutcomeStatus::AgentError);`

A `Timeout` really is a different answer from an `AgentError`, which
is what makes cells 4 and 5 worth having: the ladder acts on it.

## `mod tests` › `fn an_agent_probe_request_is_slotted_names_its_agent_and_carries_the_probe_identity() {`

What an agent probe *is*, against the two passages that say so.

INV-18: "every agent CLI invocation **incl. agent probes** acquires its
atomic {agent, pool?} pair while gates and the shell probe register
without slots" — so it is slotted and it names its agent.
`decisions.admission_and_leases.permits.invocation_identity`: the third
form is "(probe, target: Agent(name) | Shell, ordinal) at pre-flight",
and the shell probe is the *other* target — so an agent probe's
identity names the agent, never `shell`.

The expected values are written from those sentences, not read back
from the request under test.

## `fn an_agent_probe_request_is_slotted_names_its_agent_and_carries_the_probe_identity` › `assert_ne!(request.invocation.render(), "p.shell.o0");`

The role the request carries and the target its identity carries are
the same agent, and neither is the shell probe's. A request whose
identity said `shell` would be a non-slotted process wearing a
slotted role.

## `fn an_agent_probe_request_is_slotted_names_its_agent_and_carries_the_probe_identity` › `let ids: std::collections::BTreeSet<String> = ADAPTERS`

Every shipped adapter id can be one, and the three are three
distinct identities at the same ordinal — the target is a field, not
decoration.

## `fn an_agent_probe_request_is_slotted_names_its_agent_and_carries_the_probe_identity` › `let ordinals: std::collections::BTreeSet<String> = (0..4)`

And one adapter's successive pre-flight processes are successive
identities, which is what makes "unique per process" hold for a
probe that runs `--version` and then `--help`.

## `mod tests` › `fn a_probe_request_refuses_an_agent_id_that_would_not_survive_a_container_name() {`

An id that could not survive a container name is refused rather than
carried. `decisions.pr_sequence[7].scope` puts an invocation id inside
`<R>/containers/<name>.intent`, and `.` is the identity's own field
separator.

## `mod tests` › `fn the_observed_hit_your_five_hour_limit_message_signals() {`

This is the message Claude Code printed when an account reached its limit,
copied verbatim from the event log of run `01M740JX5N8HB94RNC7W2MM777`
(2026-11-11, exit code 1), middle dot included. None of the twelve phrases
matches it, so only the quota-window rule can make it signal, through its
hours window. Remove the rule and this test fails.

## `mod tests` › `fn a_hit_your_weekly_limit_message_signals() {`

The weekly window in the same wording. This text has not been observed from
the CLI. The test pins what the rule claims to cover on the expectation that
the weekly message is worded like the five-hour and session ones.

## `mod tests` › `fn the_recorded_hit_your_session_limit_message_signals() {`

The session window, in the words `reviews/FINDINGS.md` records from a worker
that returned rc=1 with `You've hit your session limit`. None of the twelve
phrases matches it. Remove "session" from the windows and this test fails.

## `mod tests` › `fn a_hit_your_context_window_limit_message_does_not_signal() {`

A context-window limit is a fault in the task, not a quota window: a shorter
prompt fixes it and waiting does not. "hit your " is followed by
"context-window", which is none of the three windows and not a name of one
word, so the text does not signal. Under the earlier rule, which took any
"limit" after "hit your", it
did, and the ladder would have deferred a real failure three times and asked
a human.

## `mod tests` › `fn hit_your_limit_with_no_window_does_not_signal() {`

"you hit your limit" names no window. The rule needs a window or a name
between "hit your " and " limit", so this does not signal. A rule that made
the window optional would fail this test.

## `mod tests` › `fn an_hours_window_is_spelt_hour_then_limit() {`

"hit your 5-hours limit" has digits and "-hour", but then "s limit", not
" limit". The window must be followed immediately by " limit", so this does
not signal. A rule that looked for " limit" anywhere after the window, or
accepted "-hours", would fail this test.

## `mod tests` › `fn a_window_after_a_later_hit_your_signals() {`

The first "hit your " is followed by "branch", the second by "weekly limit".
The text signals because every occurrence of "hit your " is checked. A rule
that read only the first occurrence, as `split_once` does, would miss it and
fail this test.

## `mod tests` › `fn limit_before_hit_your_and_none_after_does_not_signal() {`

"limit" before "hit your", with no window after it, does not signal. The
rule reads only what follows "hit your ", so a "limit" earlier in the text
plays no part.

The first text, "no limit was hit; your change is in", is the example from
the task that added the first form of the rule. Its semicolon means it never
contains "hit your " as a substring, so it is a fair negative but cannot
tell one rule from another. The second text has a real "hit your " followed
by "branch", with "limit" only before it. It witnesses that a rule matching
"limit" anywhere in a text that contains "hit your" would be wrong: that
rule would signal on it and this test would fail.

## `mod tests` › `fn hit_your_without_limit_does_not_signal() {`

"hit your" alone is ordinary English, as in a patch that hit your branch. It
is not a limit message. It witnesses that the rule needs more than the
words "hit your": a rule that signalled on them alone would fail this test.
The tests above pin what more it needs, a window and then " limit".

## `mod tests` › `fn a_session_limiter_does_not_signal() {`

The text the round-two review used to show the first form of the rule was
too wide: "hit your session" and then " limiter". "limit" is followed by
"e", a letter, so it does not end there and the text does not signal. A
rule that checked only that " limit" came next, with no boundary after it,
would signal and fail this test.

## `mod tests` › `fn a_weekly_limitation_does_not_signal() {`

The weekly window followed by " limitation". "limit" is followed by "a", so
the text does not signal. The same unbounded rule would fail this test,
through the weekly window rather than the session one.

## `mod tests` › `fn a_five_hour_limited_quota_does_not_signal() {`

The hours window followed by " limited". "limit" is followed by "e", so the
text does not signal. The same unbounded rule would fail this test, through
the hours window, so each of the three windows has a negative of its own.

## `mod tests` › `fn a_full_stop_after_limit_signals() {`

"limit" ended by a full stop, a character that is not a word character, so
the text signals. A boundary that accepted only a space or the end of the
text, or that refused punctuation, would fail this test.

## `mod tests` › `fn limit_at_the_very_end_of_the_text_signals() {`

"limit" as the last word, with nothing after it, so the text signals. The
text is the one `a_hit_your_weekly_limit_message_signals` uses; it is here
again to pin the end-of-text half of the boundary beside the full stop
above. A boundary that demanded a following character, as a check reading
the next character without allowing for none would, fails this test.

## `mod tests` › `fn the_observed_hit_your_fable_limit_message_signals() {`

The message Claude Code printed on the build machine on 2026-11-12, when the
Fable model's own weekly allowance was spent: `FABLE`, the same text the
reset tests below read. None of the twelve phrases matches it, so only the
name form of the quota-window rule can make it signal. Before that form
existed this test failed, and an attempt that printed the message was an
`AgentError` that spent an attempt against an exhausted pool.

## `mod tests` › `fn a_hit_your_opus_limit_message_with_a_reset_signals() {`

The same form with another model's name and a time-only reset. It has not
been observed. It witnesses that the rule accepts any one-word name and not
"fable" alone: a rule that listed the observed name would fail this test.

## `mod tests` › `fn a_hit_your_token_limit_message_without_a_reset_does_not_signal() {`

"hit your token limit" and "hit your context limit", the first with nothing
after it and the second with a semicolon and advice. Both are about the
prompt, not the pool. "token" and "context" are one-word names, but neither
text says when anything resets, so neither signals. A rule that took a name
without a reset would fail this test.

## `mod tests` › `fn a_model_named_limit_without_a_reset_does_not_signal() {`

The observed Fable message with its reset cut off. A model's name is not
enough on its own: the rule cannot tell "Fable" from "token", so it is the
reset that makes the message a quota. It witnesses that the reset is required
for every name, model names included.

## `mod tests` › `fn a_named_limit_whose_next_word_is_not_resets_does_not_signal() {`

"resets" appears after the limit, but the next word after "limit" is "the".
The reset has to follow the limit, as in the observed message, not merely
appear later in the text. A rule that looked for "resets" anywhere after the
limit would fail this test.

## `mod tests` › `fn a_named_limiter_with_a_reset_does_not_signal() {`

The observed Fable message with "limiter" for "limit". The name form keeps
the same whole-word boundary as the three windows, so the reset after it does
not help. A name form that skipped `after_a_whole_limit` would fail this
test.

## `mod tests` › `fn a_name_of_more_than_one_word_with_a_reset_does_not_signal() {`

Three names that are not one word of ASCII letters, each followed by a
limit and a reset: "context window", "context-window" and "Fable4". The name
stops at the space, the hyphen and the digit, and what follows is not
" limit", so none signals. A rule that took everything up to " limit" as the
name would fail this test.

## `mod tests` › `const NOV_11_2026: u64 = 1_794_355_200;`

The reset tests read their messages at fixed instants, never at the
clock's. Each constant is a UTC midnight written as a literal, and
`the_reset_fixtures_are_the_midnights_they_are_named_for` checks each
against the date it is named for, so a miscounted constant fails there,
by name, rather than as a wrong reset further down.

## `mod tests` › `fn the_observed_five_hour_message_resets_later_the_same_day() {`

The five-hour message as Claude Code printed it, read at 03:00 on the day
it was seen: the reset is 05:30 that day.

## `mod tests` › `fn the_observed_fable_message_resets_on_its_date() {`

The dated message as Claude Code printed it, read on 12 November 2026: the
reset is 14 November 2026 at 09:00.

## `mod tests` › `fn a_time_only_reset_read_just_after_itself_is_its_own() {`

The first defect the frontier review found. The five-hour message read at
05:30 exactly, at 05:30:01 and at 06:00 gives 05:30 that morning each time.
Under the rule this replaced, "the first such time at or after", the last
two gave the next morning's, a day ahead of the reset the message named.
This test replaced `a_time_only_reset_already_passed_today_is_tomorrows`,
which pinned that rule.

## `mod tests` › `fn a_time_only_reset_read_the_evening_before_is_the_next_mornings() {`

Read at 20:00, `5:30am` is the next morning's, 9.5 hours ahead rather than
14.5 hours past. 17:30 is halfway between the two mornings: one second
before it the morning just gone is the nearer, and at it the two are as
near as each other and the later is taken, so both sides of the tie are
pinned.

## `mod tests` › `fn twelve_am_is_midnight_and_twelve_pm_is_noon() {`

Read at 01:00: `12am` and `12:45am` are an hour and a quarter of an hour
past, and are today's 00:00 and 00:45; `12pm` and `12:45pm` are today's
12:00 and 12:45. `1am`, `1pm` and `11:59pm` pin the hours either side of the
wrap: `1am` is now, `1pm` is twelve hours either way and so the later,
today's, and `11:59pm` is yesterday's, an hour past. Reading `12am` as
12:00, as a parser that only adds twelve for `pm` would, fails here.

The read time is the one this test had under the rule this replaced, when
`12am`, `12:45am` and `11:59pm` gave tomorrow's 00:00, tomorrow's 00:45 and
today's 23:59. The nearest-occurrence rule moved those three expectations,
and the wrap is pinned just as well by the new ones.

## `mod tests` › `fn a_dated_reset_is_its_own_until_the_next_years_is_nearer() {`

The `Nov 14, 9am` message read on 14 November at 20:00, at 09:00 on 15
November, and a second after that gives 14 November 2026. Under the rule
this replaced, that last read was past its day of tolerance and gave 14
November 2027; this test replaced
`a_dated_reset_read_just_after_itself_stays_in_its_year`, which pinned it.
The two resets are 365 days apart, so halfway between them is 21:00 on 15
May 2027, which the test checks first. One second before it the reset is
still 2026's, and at it the two are as near as each other and the later,
2027's, is taken.

## `mod tests` › `fn a_dated_reset_across_a_year_end_is_next_years() {`

Read at 20:00 on 31 December 2026, `Jan 2` is 2 January 2027 and a time-only
`5am` is 1 January 2027, nine hours ahead. `Dec 31, 11pm` is later that same
day. Read at noon on 30 December, `Jan 2` is still the coming 2 January, not
the one just under a year past.

The read time was noon until the nearest-occurrence rule: at noon, `5am` is
seven hours past and the rule gives that morning's, which is no longer a
year-end case, so the read moved to 20:00 rather than the expectation.

## `mod tests` › `fn a_reset_read_just_after_a_year_end_is_last_years() {`

The second defect the frontier review found. At 00:30 on 1 January 2027,
`Dec 31, 11pm` is 31 December 2026, an hour and a half past, and not 31
December 2027, the date in the year read from the clock, which the rule
this replaced gave. A time-only `11pm` read then is the same instant,
yesterday's.

## `mod tests` › `fn feb_29_is_a_neighbouring_years_leap_day_or_none() {`

`Feb 29, 3pm` read on 27 February of four years in a row. In 2028 it is
29 February 2028. In 2027 and in 2029, years without that day, it is 29
February 2028, the leap day in the next year and in the year before: the
nearest leap year is a candidate when it is a neighbouring year, whether that
puts the reset a year ahead or in the past. In 2026 it is `None`, since none
of 2025, 2026 and 2027 has the day; it is never moved to 1 March. The test
was `feb_29_resets_in_a_leap_year_and_is_refused_in_another`, which gave
`None` in 2027 because only the current year and, after it, the next were
read; with the next year a candidate whatever the date, 2027's expectation
changed.

## `mod tests` › `fn a_zone_other_than_utc_gives_none() {`

The five-hour message with `(Europe/London)` in place of `(UTC)` gives
`None`, and so do `(GMT)`, a lower-case `(utc)`, `UTC` without parentheses
or without the space before them, and no zone at all.

## `mod tests` › `fn text_without_a_reset_gives_none() {`

Texts with no reset in them: the empty text, the session-limit message, the
older "5-hour limit reached", a time and zone with no `resets` before them,
`reset` without the `s`, `resets` with nothing after it, and `presets`,
which contains `resets ` but not as a word.

## `mod tests` › `fn malformed_hours_minutes_months_and_days_give_none() {`

One malformation per text, each otherwise in the observed shape: hours 0,
13 and 123, a signed hour, a space before `am`, upper-case `AM`, no `am` or
`pm`, minutes 60 and 99, one or three minute digits, a `.` for the `:`, a
misspelt, lower-case, unabbreviated or four-letter month, 31 November,
31 April, 30 February, day 0, a three-digit day, no day, no comma, no space
after the comma, and a dated reset with an hour of 13 or no time.

## `mod tests` › `fn the_first_of_several_resets_is_read() {`

Two well-formed resets give the first, for a time-only and a dated first.
A malformed first reset gives `None`; it is not passed over for a later
one.

## `mod tests` › `fn arbitrary_non_ascii_text_never_panics() {`

Texts with non-ASCII characters where the reset's parts would be (a
full-width digit, an accented month or `am`, full-width `UTC`, a
no-break space, a middle dot in place of each space, Japanese before the
message, an emoji or a NUL against `resets`), each read at five instants
from 0 to `u64::MAX`. With them, every prefix and every suffix of both
observed messages, cut at each character boundary, so a cut lands inside
every part of the grammar, including just after the multibyte middle dot.
Each prefix stops before the closing `)` and must give `None`. None of
these may panic, and at `u64::MAX`, past what an `i64` second count holds,
both messages give `None`.

## `mod built_program_tests` › `struct Boundary {`

A boundary with an agent CLI installation the test invents.

**This is the test's oracle, and it is deliberately not this machine's
filesystem.** The property being measured — *which* environment an
adapter's program was resolved against — cannot be measured with a
predicate over the same filesystem production consults: `is_file()` is
true of a host-resolved path whether the resolution was right or wrong,
so the oracle blesses either answer (`PR4-CONF-012`). A boundary the
test invents has an installation the test knows about and the host does
not, so "the boundary decided" and "the coordinator host decided" become
different observations.

It refuses any program that is not the one it has, the way a real
boundary would, and records every request so a boundary that was
**never asked** is distinguishable from one that answered. It also
reports a **version of its own**, because `Caps.version` certifying the
host's CLI while the attempt runs the image's is DESIGN.md:612's
sentence and a distinct failure from either of the other two.

## `struct Boundary` › `installed: String,`

The only program string this boundary will execute.

## `struct Boundary` › `version: String,`

What `--version` prints here. Unforgeable by the host: no machine
has an agent CLI at this version.

## `mod built_program_tests` › `fn expected_name(id: &str) -> &'static str {`

The bare name each adapter's CLI is installed under, **written here**
rather than read from the adapter's own constant: a name compared only
against the code that produced it proves nothing.

## `mod built_program_tests` › `fn scripted_help(cli: &str) -> &'static str {`

The help text each adapter's `--help` contract demands, written here
from the flags each adapter requires rather than captured from a CLI.

## `mod built_program_tests` › `fn scripted_answer(cli: &str, args: &[String], version: &str) -> (i32, String, String) {`

A boundary that can satisfy a whole pre-flight, answering **by
argument** and never by program.

Keying an answer on the program string would make the fixture agree
with whatever the adapter sent, which is the self-oracle this whole
file exists to avoid. The one place the program is consulted is
[`Boundary::run`]'s presence check, which is the boundary's own
question — "do I have this?" — and the thing under test.

## `fn scripted_answer` › `if joined.contains("upstroke_probe_deliberately_unknown") {`

Codex's six strict-config parser probes. The two assignments are
transcribed here rather than read from that adapter's private
constants, so a renamed probe key fails this fixture loudly instead
of silently agreeing with itself.

## `mod built_program_tests` › `fn an_adapters_program_is_the_boundarys_and_the_coordinator_host_is_never_asked() {`

An adapter's program is the **boundary's**, and the coordinator host is
never asked what it has.

This replaces `an_adapters_program_is_the_coordinator_hosts_and_the_
boundary_supplies_none`, which pinned PR4's behaviour deliberately and
which a correct PR6 must fail by name. The property that **moved
across** is the old test's claim 2 — "what pre-flight sends and what the
attempt would send are one program", DESIGN.md:612 in the only form PR4
could hold it. It is now held by
[`the_program_preflight_certifies_is_the_program_the_attempt_would_run`]
in both call orders, because the ordering PR4 needed was a property of
the process-wide resolution cache and that cache is gone. The old
claims 1 and 3 are **inverted** here: the boundary is asked, and a CLI
the coordinator host cannot resolve is no longer refused before it is.

`PR4-ADAPTER-RESOLVES-ON-THE-HOST`'s three separate failures, and where
each dies:

1. *the normal container case is refused before the runtime is asked* —
   every adapter certifies against a boundary the coordinator host does
   not share, and
   [`a_cli_this_host_does_not_have_still_certifies_at_the_boundary`]
   witnesses the absent-on-the-host half on a machine that really lacks
   one;
2. *every spec carries a path that names nothing at the boundary* — the
   program every request carries is one path component, not absolute,
   and equal to the name written in [`expected_name`]; and
3. *`Caps.version` certifies the host's CLI while the attempt runs the
   image's* — the version returned is the boundary's invented `9.9.9`,
   which no installation on any machine reports, and
   [`two_boundaries_in_one_process_each_certify_their_own_cli`] holds it
   across two boundaries.

The second field this holds constant is **what this machine has
installed**: the assertions above are true whichever branch a machine
takes, and both branches are counted so a machine cannot make the test
mean less by having more.

## `fn an_adapters_program_is_the_boundarys_and_the_coordinator_host_is_never_asked` › `match crate::util::find_program(name) {`

The discrimination against the old behaviour, on either kind of
machine. Where this host has the CLI, the old code would have
sent that absolute path and this boundary would have refused it;
where it does not, the old code refused before asking at all.

## `mod built_program_tests` › `fn a_cli_this_host_does_not_have_still_certifies_at_the_boundary() {`

A CLI this coordinator host does not have certifies anyway, because the
boundary has it.

`PR4-ADAPTER-RESOLVES-ON-THE-HOST` failure (1), witnessed rather than
argued: DESIGN.md:612's normal container case is "an image with
version-pinned CLIs", and until PR6 that image's CLI was refused at
pre-flight because *this* machine had no such file.

The premise is asserted rather than hoped for: with all three CLIs
installed here the absence half is unobservable, and a silent skip would
measure nothing while looking green. Both machines this slice is
measured on satisfy it — this box has `claude` and `codex` and no
`copilot`, and the Windows guest has none of the three.

## `mod built_program_tests` › `fn a_cli_this_host_does_not_have_is_refused_by_name_and_by_boundary() {`

The other side of that: when the boundary **is** this host and this host
does not have the CLI, the operator is told so, by name and by boundary.

The fail-closed half of `PR6D-001`'s repair, end to end. The runner's own
refusal is asserted in `runner::host::tests`; this is the sentence the
operator actually reads, which is [`bin::boundary_refused`] wrapping it,
and the two had never been composed. What it must not be is a bare
`NotFound`: before the runner resolved names, an npm-installed CLI on
Windows failed with "program not found" naming no boundary and no
remedy, which is the failure nobody could diagnose from a log.

The premise — that this machine lacks at least one of the three — is
asserted rather than hoped for, the same way its sibling above asserts
it, so a machine with all three installed reports that it cannot observe
this instead of passing vacuously. Both measured machines satisfy it:
this box has no `copilot`, the Windows guest has none of the three.

## `fn a_cli_this_host_does_not_have_is_refused_by_name_and_by_boundary` › `assert!(`

And it is the **resolution's** refusal, not a spawn's. This is
the fail-closed clause itself: a runner that handed the name to
`Command` anyway would produce a `NotFound` here, which names no
boundary and is what an operator cannot act on.

## `fn a_cli_this_host_does_not_have_is_refused_by_name_and_by_boundary` › `assert_eq!(`

One distinct sentence per absent CLI: a refusal that named the first
adapter for all of them would collapse this to one.

## `mod built_program_tests` › `fn the_program_preflight_certifies_is_the_program_the_attempt_would_run() {`

Pre-flight certifies the program the attempt would run — in **both call
orders**.

This is the property that moved here from the test this replaced.
There, `build` had to run **first**, and its comment said why: each
adapter memoised its resolution in a process-wide `OnceLock`, so a probe
reaching an unfilled cache wrote the fixture's answer into it and
changed what every sibling test in the binary resolved. The ordering was
load-bearing because the answer was *state*. It is now a function of its
argument, so the order cannot matter — and asserting both orders is what
says so.

The second field held constant is the adapter; what varies is the order,
and the two orders must agree exactly.

## `mod built_program_tests` › `fn two_boundaries_in_one_process_each_certify_their_own_cli() {`

Two boundaries in one process each certify **their own** CLI.

The hazard this exists for: with two runners in one process — which is
exactly what the container runner introduces — a resolution cached on
nothing hands one boundary's answer to the other, and a value that is
correct on first use and wrong on the second is invisible to any test
that constructs one runner. The repair is that there is no cache; this
is what says so, and it is what fails if one comes back.

Two independently varying fields, and their intersection is the test:
**which boundary** (two, reporting different versions) x **in which
order** (both). Two distinct versions (1.1.1 and 2.2.2), probed in both
orders, are asserted as a distinct-value count rather than described, so
a fixture that lost a version reports it instead of agreeing with itself.

## `mod built_program_tests` › `fn the_adapters_hold_no_process_wide_resolution_state() {`

No adapter holds process-wide resolution state, and this is the answer
to "what is the cache keyed on".

**Nothing is cached, so there is no key.** Each adapter used to memoise
its resolved binary in a `static RESOLVED: OnceLock<Option<Invocation>>`
— a cell keyed on nothing, which with two runners in one process hands
one boundary's answer to the other. A resolution that is correct on
first use and wrong on the second is the hardest shape in this slice
precisely because it is invisible to any test that constructs one
runner, and it stays invisible to a *behavioural* test even with two,
because a cache of a constant is indistinguishable from no cache.

So the claim is structural and the census is the only thing that can
make it: an adapter that starts remembering an answer fails here on the
line that declares the cell, before any behaviour depends on it.

The checker uses `effects::production_code`, which blanks comments and literals
and removes test-only items. A string constant containing `OnceLock` does not
declare state. The same checker reads all four source files and the controls:
inert comments and literals, excluded test-only state, and real holder
declarations. A production declaration after a test-only item remains visible.
The `static ` needle excludes a preceding apostrophe, so a `'static` lifetime
does not count as a static declaration; the harmless-code control covers it.

## `fn the_adapters_hold_no_process_wide_resolution_state()` › `const HOLDERS: [&str; 4] = ["static ", "thread_local!", "OnceLock", "LazyLock"];`

Every way an adapter could hold a value across calls. Written out
rather than derived: a list read from the tree would shrink with it.

`static ` rather than a list of cell types, and that is deliberate.
Every adapter is a unit struct with no state of its own, so the only
way one remembers anything between calls is a module-level item —
and naming `OnceLock` alone would miss the `static X: Mutex<_>` that
a `const fn` constructor makes just as easy. `OnceLock` and
`LazyLock` are named as well so a `let`-bound or field-held cell is
caught too.

## `fn the_adapters_hold_no_process_wide_resolution_state()` › `assert!(`

The controls run actual holder declarations through the checker and assert the
detected holder names. They also combine declarations with inert literal/comment
text and test-only items, so those regions cannot hide later production state.

## `mod built_program_tests` › `fn the_host_runner_executes_a_bare_program_name_as_it_executes_the_resolved_path() {`

The host runner executes a bare program name as it executes the path
the adapters used to resolve.

The other direction of this repair, and the one every existing test and
the whole v0.1 product depends on: a change that resolves correctly for
a container boundary and quietly changes what the **host** runner runs is
a defect. The suite cannot show this on its own — it was written against
the old shape and largely still passes — so this spawns the same program
twice through the real [`crate::runner::host::HostRunner`], once named
and once at the absolute path `util::find_program` picks (which is the
resolution the adapters performed), and requires the two to agree.

`git` rather than an agent CLI because every machine that can build this
repository has it, so the observation is not a property of what happens
to be installed. The two program strings are asserted to actually differ
first; if they did not, this would compare a thing with itself.

**What this row cannot express, and where that lives instead.** `git` is
a native `.exe`, and on Windows `CreateProcessW` appends `.exe` to a bare
name, so the bare spelling reaches it whether or not this runner resolves
anything — which is exactly the installation shape `PR6D-001` did *not*
break. As a witness for the Windows case this row is therefore
correlated: it was green while the defect was live. It is kept as the
control it can honestly be — a real program on this machine's real
`PATH`, which the repair must not have changed — and the axis it cannot
vary, an npm-style `.cmd`-only installation reachable only through
`PATHEXT`, is
`runner::host::tests::an_npm_style_installation_runs_by_bare_name_exactly_as_it_runs_by_path`.
That test lives beside the resolution because writing a shim needs the
effect allowance `effects/allowlist.toml` gives `src/runner/host.rs` and
does not give this module.

## `mod probe_identity_tests` › `fn each_agent_probe_request_names_its_own_agent_in_every_field() {`

Each agent probe names **its own** agent, in every field that names one.

`invariants_introduced[1]` — "RunnerRequest carries a typed
InvocationId (… probes included; the probe role carries target
`Agent(name)` | `Shell`)". Every probe fixture in this suite probes one
agent, so a `probe_request` that filled the target with the first
configured adapter's name would agree with itself on every one of them.
Two independently named probes, and each request checked against the
name it was asked for rather than against the other request.

Both iteration orders, because "the first configured agent" is a
property of order: a fixture that only ever built them in one order
would pass for the agent that happened to be first.

## `fn each_agent_probe_request_names_its_own_agent_in_every_field` › `const NAMES: [&str; 3] = ["claude-code", "codex", "copilot"];`

Written here, not read from the adapter registry: the names are the
expected values.

## `fn each_agent_probe_request_names_its_own_agent_in_every_field` › `assert_eq!(roles.len(), 3, "{roles:?}");`

Hostility as counts: three names in, three distinct values out of
each field that carries one.

## `mod probe_identity_tests` › `fn use_sites(source: &str) -> Vec<String> {`

The file minus its `#[cfg(test)] mod tests { … }` block and minus the
`mod probe_ordinal { … }` declaration, so what is left is the
production code that *uses* an ordinal.

## `fn use_sites(source: &str) -> Vec<String>` › `if trimmed.contains("probe_ordinal::")`

Prose mentions an ordinal too, and a comment starts no
process.

## `mod probe_identity_tests` › `fn every_probe_call_site_passes_its_own_ordinal_constant() {`

Every ordinal an adapter's pre-flight passes to the Runner, **read
from the call sites** rather than from the table beside them.

`decisions.admission_and_leases.permits.invocation_identity`: an
invocation identity is "unique **per process**", and "every
RunnerRequest carries it". Each adapter's
`every_preflight_process_has_its_own_ordinal` builds its set from the
`probe_ordinal::ALL` array, so what it asserts is that a *table* has
distinct entries — which stays true when a call site passes another
entry's constant (codex's `debug models` step passing `VERSION`) or an
arithmetic expression over one (`HELP.saturating_sub(1)`). Two
processes then carry `p.agent-<name>.o0` and the ledger cannot tell
them apart, which is exactly what "unique per process" forbids.

This asserts the property one step later, at the point of use: each
declared constant is used **once**, every one is used, and the only
non-bare uses are the one block codex documents — a base plus an index,
for its six strict-config parser probes.

Codex had a second such block until PR6, one process per PATH
candidate, because it resolved its binary by spawning each candidate on
the coordinator host. `PR4-ADAPTER-RESOLVES-ON-THE-HOST` removed the
resolution, so `RESOLUTION_BASE` and its call site are gone and the
counts below drop by one.

## `struct Adapter` › `declared: &'static [&'static str],`

The constants the module declares, written out here from the
steps the adapter performs rather than read from the module.

## `struct Adapter` › `block_parts: &'static [&'static str],`

Constants that may appear in an expression, with the block they
open. Everything else must reach the Runner as a bare
`probe_ordinal::NAME` argument.

## `struct Adapter` › `call_sites: usize,`

How many processes this adapter starts through
`probe_request`, counting each variable-length block as one
call site.

## `fn every_probe_call_site_passes_its_own_ordinal_constant()` › `block_parts: &["CONFIG_BASE", "CONFIG_PER_SURFACE"],`

The one variable-length step: six strict-config parser
probes (two surfaces x three assignments). `probe_ordinal`
documents it as a block precisely because it cannot be one
constant.

## `fn every_probe_call_site_passes_its_own_ordinal_constant()` › `let collisions: Vec<(&String, &usize)> =`

No constant is used twice: that is the collision this exists to
catch, and it is what a table-only test cannot see.

## `fn every_probe_call_site_passes_its_own_ordinal_constant()` › `let declared: BTreeSet<&str> = adapter.declared.iter().copied().collect();`

And every declared constant is used: an ordinal declared and
never passed is a step whose identity came from somewhere else.

## `fn every_probe_call_site_passes_its_own_ordinal_constant()` › `let calls = adapter`

The number of processes, counted from the call sites rather
than from the table.

## `fn every_probe_call_site_passes_its_own_ordinal_constant()` › `assert_eq!(total_sites, 12, "the adapters' probe call sites moved");`

Hostility as a count: 3 + 2 + 7 across the three adapters, written
from what each pre-flight does. Codex was 8 until PR6 removed the
per-PATH-candidate resolution spawn
(`PR4-ADAPTER-RESOLVES-ON-THE-HOST`).
