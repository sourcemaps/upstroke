# `src/agent/adapter.rs`

Extended notes for [`src/agent/adapter.rs`](../../../src/agent/adapter.rs).

The code is the authority for what it does; this file is the whole of its prose, moved out of
the source verbatim. Each section is headed by the line of code the comment sat above, spelled
as it is in the source, so the heading is the grep string that finds the code.

## Module

The agent adapter contract and its registry: `AuthState`, `Discovery`, `Caps` and `TaskRun`;
`probe_workspace` and `probe_request`; the `AgentAdapter` and `AdapterSource` traits,
`BuiltinAdapters`, `ADAPTERS` and `by_id`; the help-text readers; the rate-limit and reset readers;
and the tests that hold the three shipped adapters to one supervision table. `src/agent/mod.rs`
re-exports it (`pub use adapter::*`), so `upstroke::agent::AgentAdapter` and the rest resolve at
the root.

## `#![forbid(`

The three governed lints are `forbid` here: nothing in this file, its test modules included,
allows a governed lint (the file has no row in `effects/allowlist.toml`), so the fence compiles,
and under it a downgrade however written is `E0453` at the lint gate. The code lives here so that
the module root holds only declarations and re-exports: a `deny` at the root is a level an inner
`allow` lowers, and production code under a root `deny` is the route
`GUARD-DECISION-SILENT-PRODUCTION-FILES-OUTSIDE-THE-ROLL-CALL` recorded.

## `pub enum AuthState {`

Whether the vendor's CLI says it is signed in.

Three states, not two. "Could not tell" must never render as "not connected": `upstroke connect`
writes a file an operator then trusts, and a confident *wrong* "you are not logged in" sends them
to re-authenticate an account that was fine.

## `impl std::fmt::Display for AuthState {`

One rendering, used by `connect` and `capacity` alike, so the same fact reads the same from both
and "could not tell" never renders as "not connected" in either.

## `pub struct Discovery {`

What one agent's CLI could be got to say about itself, without the network and without touching
a credential (invariants 2 and 5).

## `pub struct Discovery` › `pub models: Vec<String>,`

The models the CLI itself advertises. Empty on Claude Code and Copilot, whose adapters list none.
Codex's adapter reads its local roster from `debug models` and reports the slugs here; `connect`
and `capacity` check a listing against the shipped catalog.

## `pub struct Discovery` › `pub shape: Option<PoolKind>,`

§13's pool-kind hint, read from whatever the CLI says about the account it is signed into. `None`
means it said nothing conclusive, and the caller picks a documented default rather than guessing.

## `pub struct Discovery` › `pub notes: Vec<String>,`

What the operator should know about how this was worked out, including what could not be.

## `impl Discovery` › `pub fn unknown() -> Self {`

What an adapter that does not implement discovery reports: nothing, said out loud.

## `pub struct Caps {`

Capabilities discovered by `probe()` at pre-flight (§14). Copilot's CLI has shipped breaking flag
removals, so capability probing is load-bearing, not decorative.

## `pub struct Caps` › `pub version: String,`

Version string as reported by the binary, best-effort.

## `pub struct TaskRun {`

Everything an adapter needs to build one attempt's subprocess. The engine materializes the prompt
(§14: body + acceptance + artifacts + conventions brief); adapters never re-derive it.

## `pub struct TaskRun` › `pub prompt: String,`

Fully materialized prompt, delivered on stdin.

## `pub struct TaskRun` › `pub workspace: PathBuf,`

Working directory for the subprocess (the workspace repo root).

## `pub struct TaskRun` › `pub gate_cmds: Vec<String>,`

The gate commands this profile may run, and nothing else (§20). Empty for reviewers, which run
nothing at all. Carried on the run rather than only handed to `materialize_permissions` because
Copilot's permission surface is argv, so its `build` needs them at command-construction time.

## `pub struct TaskRun` › `pub resume_session: Option<String>,`

Same-rung retry: resume this session with feedback instead of starting fresh (§11.4).

## `pub struct TaskRun` › `pub settings_path: Option<PathBuf>,`

Per-run permission settings file, materialized by the engine from
`claude::permission_settings`-style generators (§20).

## `pub fn probe_workspace() -> PathBuf {`

Where a pre-flight process runs: the coordinator's own working directory, since a probe asks a CLI
about itself and has no workspace of its own. Absolute (falling back to `"."` only when the working
directory cannot be read) because `runner::host::HostRunner::run` clears the environment, and on
Windows the `=X:` drive-relative variables go with it, so a probe must not resolve a drive-relative
path.

## `pub fn probe_request(`

One pre-flight process of `agent`, as a `RunnerRequest`.

Probes go through the Runner, as workers, gates and reviews do (design/21's runner commitments:
"Probes run through that same runner"). An agent probe is accounted the way an attempt is: the
role is `probe(<agent>)`, it is slotted, and `agent` is set so `host-v1` supplies that agent's
credential location (a probe that could not see the credential directory would certify a CLI in a
state the attempt never runs in). The wording quoted for this was the retired design packet's
`decisions.pr_sequence[5].scope` and its `INV-18`, which are not in this repository.

`ordinal` is **which of this adapter's pre-flight processes this is**. Invocation identities are
unique per process, so each adapter fixes a named ordinal per step rather than counting: a counter
would renumber every later step whenever an earlier one was skipped, and the identities of one
machine's pre-flight would stop being a function of the pre-flight.

### Errors

[`UpstrokeError::Refused`] when the adapter id cannot appear in an invocation
identity — see [`InvocationId::probe`]. Every shipped id is `[a-z-]`.

## `pub trait AgentAdapter: Send + Sync {`

design/08's `AgentAdapter`.

## `pub trait AgentAdapter: Send + Sync` › `fn probe(&self, runner: &dyn Runner) -> Result<Caps, UpstrokeError>;`

Locate the binary and report version + capabilities. Ran at pre-flight; a missing binary is a
refusal to start, not a task failure (§19). Takes the runner because design/08's signature does,
"probes the boundary that will execute", and design/21 says why: "Probes run through that same
runner, or pre-flight could certify a host CLI/version different from the one the attempt
executes."

### Errors

A missing or unusable binary, a CLI that has dropped a required flag,
or a runner refusal.

## `pub trait AgentAdapter: Send + Sync` › `fn build(&self, run: &TaskRun) -> Result<CommandSpec, UpstrokeError>;`

Turn one attempt into a **data-only** `CommandSpec`. An adapter "does not decide where the process
runs" (design/06): a live `std::process::Command` could carry a cwd, an environment, or a spawn
past the runner, so what comes back is a program, arguments, an environment **overlay** and stdin
bytes, and nothing that names a machine.

### Errors

A refusal to run this profile at all (§19/§20), or a binary that cannot
be located.

## `pub trait AgentAdapter: Send + Sync` › `fn parse(&self, out: &ProcessOutput) -> Result<Outcome, UpstrokeError>;`

Read one attempt's process output as an `Outcome`.

### Errors

Output this adapter cannot interpret at all.

## `pub trait AgentAdapter: Send + Sync` › `fn discover`

§13's `upstroke connect`: ask this agent's CLI about the account behind it — signed in or not,
what shape its quota is, which models it offers.

Subprocesses the vendor's own CLI and parses what came back. No HTTP, no token ever handled, no
credential file read: a vendor CLI talking to its own vendor is the design (invariant 2), the same
posture §9 sets for plan importers.

Takes the `Caps` the caller already probed rather than re-probing: discovery always runs beside a
probe (a CLI that cannot report its own version is in no state to be asked about its account), and
probing again would spawn the probe's processes a second time, each carrying the probe timeout.

The default reports nothing rather than being required, so an adapter cannot silently claim
discovery it does not do: `Discovery::unknown` is an honest "could not tell".

### Errors

Whatever asking this CLI about its account failed with. The default
never fails: it asks nothing.

## `pub trait AgentAdapter: Send + Sync` › `fn stdin_payload<'a>(&self, run: &'a TaskRun) -> &'a str {`

What to write to the child's stdin. Delivery is the adapter's call: CLIs that take the prompt as
an argument instead return empty here.

## `pub trait AgentAdapter: Send + Sync` › `fn materialize_permissions(`

Materialize this agent's permission surface (§20) into `dir`, returning the file the command
should reference. Claude Code writes a settings JSON and returns it; Copilot and Codex take
permissions as argv flags, write an audit-only JSON record and return `None`, as the default does
without writing anything.

## `pub trait AdapterSource {`

Where a caller finds agent adapters. Injectable so the engine, `connect` and `capacity` are
testable without any real agent CLI on the machine. Lives here rather than in `engine` because
resolving an adapter id has nothing to do with running a plan.

## `pub trait AdapterSource` › `fn shared(&self) -> Option<Arc<dyn AdapterSource + Send + Sync>> {`

An owned, thread-shareable handle on the same registry, or `None`.

The schema-4 coordinator (`engine::topology::coordinator`) runs each pipeline on a thread of its
own and hands every one an `Arc<dyn AdapterSource + Send + Sync>`, while the public entry points
(`run_with`, `run_harness`, `resume_with`, `resume_harness`) take a borrowed `&dyn AdapterSource`.
A borrow cannot become an owned, `'static` handle, so a source says whether it can give one. The
default is `None`: a test double that holds borrowed state, or one that is not `Sync`, keeps
compiling and simply cannot be shared. The production entry (`engine::topology::entry`) refuses a
run wider than one before any lock when its source answers `None`, and drives a width-1 run on the
single-threaded step loop instead.

## `impl AdapterSource for BuiltinAdapters` › `fn shared(&self) -> Option<Arc<dyn AdapterSource + Send + Sync>> {`

The built-in registry is a unit struct over a `static` table, so sharing it is a fresh `Arc` of
the same zero-sized value, and every handle resolves an id to the same `&'static dyn AgentAdapter`.

## `pub static ADAPTERS: &[&dyn AgentAdapter] = &[`

Registry in routing order; ids match `WorkerProfile.agent`.

## `pub(crate) fn missing_effort_levels(help: &str) -> Vec<Effort> {`

The shared effort levels the help entry for `--effort` does not advertise: all of them when help
has no `--effort` line. Looking only for the flag proves too little, since CLI versions have
exposed `--effort` with a narrower enum. The option's own wrapped help block is parsed so unrelated
words elsewhere in `--help` cannot masquerade as a supported value.

## `pub(crate) fn advertises_flag(help: &str, flag: &str) -> bool {`

Whether help advertises `flag` as a whole option token. Short flags need this instead of substring
search: `-p` occurs inside `--permission-mode`.

## `pub fn looks_rate_limited(text: &str) -> bool {`

Rate-limit signals are ground truth for the capacity engine (§13), so the shipped adapters read
from one vocabulary rather than several that drift apart. The twelve phrases cover the
subscription-window wording Claude Code prints ("5-hour limit reached", "Weekly limit reached"),
Copilot's credit and premium-request wording (§13's two billing shapes), and API-level errors
underneath either.

None of the twelve matches Claude Code's newer wording, `You've hit your 5-hour limit · resets
5:30am (UTC)`, so one rule sits beside the phrases, in `hits_a_quota_window`: the text, in lower
case, contains "hit your " followed immediately by a quota window and then " limit" as a whole
word (the boundary is set out under `after_a_whole_limit`). Three windows signal on their own: a
number of hours, written as one or more ASCII digits and then "-hour" ("5-hour"); "session"; and
"weekly". A fourth form signals only with a reset: one of the names of Claude's model families,
"Opus", "Sonnet", "Haiku" or "Fable", when the next word after "limit" is "resets" ("hit your Fable
limit · resets Nov 14, 9am (UTC)"). The names are compared in lower case, like the rest of the
rule. Every occurrence of "hit your " is checked, not only the first, so an ordinary "the patch hit
your branch" earlier in the text does not hide a limit message after it. "hit your usage limit"
needs no window: "usage limit" is one of the twelve phrases.

The rule is narrow on purpose. A rate-limit signal is ground truth about capacity (§13): the
ladder does not count the attempt, defers it, and after three deferrals asks a human. So "You've
hit your context-window limit", "hit your rate of change limit", "hit your token limit", "hit your
context limit · resets after compaction", "hit your request limit · resets 9am (UTC)" and a bare
"hit your limit" do not signal. They are task failures, to be retried or escalated like any other.

The rule splits the already lower-cased text on "hit your " and reads each piece with
`trim_start_matches`, `strip_prefix`, `get` and `starts_with`. Those return slices of the same
string or `None`, so it allocates nothing more, and nothing in it indexes or unwraps, so it cannot
panic.

The five-hour, session and model forms were seen from Claude Code; the weekly form has not been,
and is covered on the expectation that it is worded alike. A limit message in any other wording is
not recognised and is read as `AgentError`. That spends an attempt, which is the safer mistake: a
false rate-limit signal would tell the capacity engine an untouched pool is exhausted.

Consulted only for a failed attempt: a successful task *about* rate limiting ("added backoff for
429 responses") must never be read as the pool being exhausted, or verified work gets rolled back.

## `fn hits_a_quota_window(lower: &str) -> bool {`

The quota-window rule described in the section above: some piece after "hit your " starts with a
quota window and its limit, as `after_a_quota_window` reads it.

## `fn after_a_quota_window(after: &str) -> Option<&str> {`

What follows " limit", when `after`, the lower-cased text after a "hit your ", starts with a quota
window and its limit; otherwise `None`. It is read one of two ways:

- If it starts with ASCII digits, the window is those digits and "-hour".
- Otherwise the window is the run of ASCII letters it starts with.
  "session" and "weekly" signal on their own. "opus", "sonnet", "haiku" and
  "fable", Claude's model names, signal only if `when_it_resets` finds a
  reset in what follows the limit. Any other word, and an empty run, gives
  `None`.

Either way the window must be followed by " limit" as a whole word, which `after_a_whole_limit`
checks. The window is one word: "context window" and "context-window" stop at the space or the
hyphen, which is not " limit", and "fable4" stops at the digit. So none of them signals, with a
reset or without.

It is one function, rather than the body of `hits_a_quota_window`, so that `the_limits_reset`
recognises exactly the limits that signal, and nothing else, when it looks for the reset that
follows one. What it returns is a suffix of `after`, so a caller that knows where `after` ends
knows where the limit ends. The window is `after` cut short by the length of what follows it, with
`get`, not stripped with `strip_suffix`: from `the_limits_reset` the rest runs to the end of the
text, and comparing it byte by byte would cost the rest of the text at every limit.

A model name signals only with a reset because, unlike "session" and "weekly", it is not a window:
a message that only names a model says what was spent, and the reset is what says the allowance
comes back. "session", "weekly" and the hours window signal with or without a reset.

The names are a closed list: "opus", "sonnet", "haiku" and "fable", and nothing else. This matcher
is shared by every agent, and a reset alone does not make a limit a quota: a Copilot or Codex
message `You've hit your context limit · resets after compaction` read as a rate limit would make
the coordinator record `pool_exhausted` for a pool that is not, and the ladder defer the task three
times and ask a human, where it should retry or escalate. "context", "token" and "request" limits
are about the prompt or the call, and waiting fixes none of them.

The list is closed, rather than open with a list of words to refuse, because the mistakes are not
alike. A model limit message with a name not in the list is read as `AgentError` and spends an
attempt, the safer mistake set out above. A word wrongly let in tells the capacity engine that an
untouched pool is exhausted. And the words that are not model names cannot be listed, while the
model names can: they are the names of Claude's model families, the only models whose own allowance
Claude Code reports in this form. A new family name has to be added here by hand, with a test, when
it is first seen.

## `fn after_a_whole_limit(window_end: &str) -> Option<&str> {`

What follows " limit" right after a window, if "limit" is a whole word there. "limit" must end at
the end of the text, or at a character that is not an ASCII letter, ASCII digit or underscore. A
full stop, a space, a semicolon or the " · resets" of the observed five-hour message all end it;
"limiter", "limitation" and "limited" do not, and give `None`. Without the boundary, a failed
attempt whose error read `The request hit your session limiter and returned an internal error.`
would be read as `RateLimited`: the ladder would not count it, defer it three times and ask a
human, when a real failure should be retried or escalated.

The check strips " limit" with `strip_prefix` and asks `starts_with` whether the next character is
a word character. Both work on slices of the same string, so the rule still allocates nothing more
and cannot panic. The character test is ASCII only, like the lower-casing before it.

## `fn when_it_resets(after_limit: &str) -> Option<&str> {`

What follows "resets " when the next word after the limit is "resets", followed by a space, as in
" · resets Nov 14, 9am (UTC)"; otherwise `None`. Only characters that are neither letters, digits
(in any script) nor underscores may come between: spaces, `·`, `∙`, punctuation. So "hit your
token limit; the cache resets 9am (UTC)" does not count: the next word is "the". The gap test uses
Unicode letters and digits, not ASCII ones, so that a word in another script between "limit" and
"resets" is a word and stops the match, rather than being skipped as punctuation.

`after_a_quota_window` asks only whether there is one: the word, not a reset that parses. A reset
in another zone, which `reset_at_from_limit_message` refuses, still says the allowance comes back,
so the message is still a rate limit; its `reset_at` is just unknown. `the_limits_reset` uses what
this returns as the reset to read, so both read one rule for which reset belongs to the limit.

## `pub fn names_a_quota_limit(text: &str) -> bool {`

Whether the text holds a limit `reset_at_from_limit_message` would read a reset from: a "hit your"
limit that `after_a_quota_window` accepts, the limits `hits_a_quota_window` signals on (see
`the_limits_reset` for the one spelling where the two differ). It says nothing about whether that
limit gave a reset, or whether the reset parses. The older "5-hour limit reached" and the other
fixed phrases of `looks_rate_limited` are not such limits: they signal, but nothing in them says
which reset is theirs.

The coordinator and `capacity` have more than one text for a failure (the limit message, then the
reason) and read the first that names a limit and only that one: a text with a limit whose reset
is unreadable decides `None`, and the next text is read only when this one names no limit at all.
It is `the_limits_reset` asked only whether it found a limit, so the two cannot disagree about what
a limit is.

## `pub fn reset_at_from_limit_message(text: &str, now_unix: u64) -> Option<String> {`

When a usage limit says it lifts, read from the limit message itself, as an RFC 3339 UTC
timestamp (`YYYY-MM-DDTHH:MM:SSZ`). The coordinator records it as `reset_at` on the
`pool_exhausted` event, and calls this only when the agent that failed is Claude Code: the shape
below is Claude Code's, and another vendor's text that happens to fit it is not a reset this knows
how to read.

Two messages carry one. Both were printed by Claude Code:

- `You've hit your 5-hour limit · resets 5:30am (UTC)`;
- `You've hit your Fable limit · resets Nov 14, 9am (UTC)`.

So a reset is the word `resets` and a space, then an optional date, then a time of day, then a
space and `(UTC)`. The date is a three-letter month (`Jan` to `Dec`), a space, a day of the month
in one or two digits, a comma and a space. The time of day is an hour from 1 to 12 in one or two
digits, optionally `:` and exactly two minute digits, then `am` or `pm`. `12am` is midnight and
`12pm` noon. The spelling is matched exactly, case included, as it was printed.

Neither message gives a year, and the time-only one gives no date. Both are completed against
`now_unix`, the moment the message is read, by one rule: the reset is the occurrence nearest to
`now_unix`. There are three candidates, in order:

- for a time only, that time yesterday, today and tomorrow;
- for a month and day, that date at that time in the year before
  `now_unix`'s, in its year and in the year after. A candidate that is not a
  real date, such as 29 February in a common year, is not a candidate, and
  if none is, the answer is `None`.

The candidate with the smallest absolute distance from `now_unix` is the reset, and on a tie the
later one. So a message read well before its reset gets the coming one: `5:30am` read the evening
before is the next morning's, and `Jan 2` read on 30 December is the coming 2 January. A message
read just after its reset keeps its own reset, a moment in the past, rather than one a day or a
year away: `5:30am` read at 05:30:01 is that morning's, and `Dec 31, 11pm` read at 00:30 on 1
January is the day before's, in the year before. Either way the reset is never more than half a
day from `now_unix` for a time only, nor more than half a year for a date every year has. `Feb 29`
is the exception, since at most one of the three years has it: read in 2027 it is 29 February
2028, more than a year ahead; read in 2029 it is 29 February 2028, in the past; read in 2026, with
no leap year among 2025, 2026 and 2027, it is `None`.

`None` means the text gave no reset this can stand behind: no recognised limit, no reset that
belongs to one, a zone other than `(UTC)`, or any part of the limit's reset that does not parse (an
hour outside 1 to 12, minutes outside 0 to 59 or not two digits, a month not in the list, a day the
month does not have, a missing comma or space). A reset in another zone is refused rather than
converted: converting `(Europe/London)` needs a time-zone database, which is a dependency, and a
guess at the offset would be a wrong timestamp recorded as fact. `None` means the reset is
unknown.

Which reset is read is decided before any of it is parsed, and only a reset the limit itself
gives is ever read (`the_limits_reset`). A limit is one `hits_a_quota_window` would accept, and the
reset it gives is:

- `resets` as the next word after the limit, on the limit's line;
- or, when nothing after the limit on its line is a `resets`, a `resets`
  that begins the very next line, as when the message is wrapped after
  "limit".

If the text has several limits, the first that gives a reset is the one read. A reset anywhere
else is not the limit's and is never read: not one with no limit before it, not one on a later line
that does not directly follow the limit's, not one after another word on the limit's line ("the
queue resets"). Claude Code's stderr can carry other lines before the limit message, and a reset
taken from one of them would record another thing's reset as the pool's.

So `MCP cache resets 9am (UTC)` on one line and `You've hit your 5-hour limit · resets 5:30am
(UTC)` on the next give 05:30, in either order, and `You've hit your session limit` with `resets
5:30am (UTC)` on the next line gives 05:30 whatever other lines the text has. `MCP cache resets
5:30am (UTC)` alone gives `None`, as does the session limit with that reset a line further down.

The chosen reset is the only one parsed. When the limit's reset is malformed or in another zone
the answer is `None`, even if another reset in the text parses: the limit's is the one the message
gave, and the other is about something else. A limit with no reset of its own gives `None`, an
unknown reset, which is the honest answer.

It never panics, on any text and any `now_unix`. The text is read with `split_inclusive`,
`match_indices`, `strip_prefix`, `strip_suffix`, `starts_with`, `get` and `split_at_checked`, each
of which returns a slice of the same string or `None`; the only indices come from `find`,
`match_indices` and the lengths of the lines `split_inclusive` gives, or are a string's length less
that of one of its suffixes (with `checked_sub`), so they fall on character boundaries. The clock
arithmetic is checked: a `now_unix` past `i64::MAX`, or a candidate whose seconds would overflow,
gives `None`. So does a nearest candidate before 1970, which has no unix time to format; only a
clock read near the epoch can meet one.

So does a nearest candidate whose year is outside 0000 to 9999, and that check comes before
`util::rfc3339_utc` is called, so it is never handed an instant whose year it cannot write in four
digits. RFC 3339 has only a four-digit year, and `rfc3339_utc` writes the year with `{year:04}`,
which pads to four digits and does not stop there, so a reset in 10000 would be a `reset_at` a
strict reader of the `pool_exhausted` event refuses (finding `PR380-RESET-PARSER-NON-RFC3339-YEAR`). The check is on the
chosen candidate, not on `now_unix` nor on the other candidates: `Dec 31, 11pm` read at 00:30 on 1
January 10000 is still 9999-12-31T23:00:00Z, and a candidate in 10000 that is not the nearest
changes nothing. Only a clock read in 9999 or later can meet the bound (`Feb 29` read anywhere in
9999 is 29 February 10000), and since an instant before 1970 is refused anyway, it is the upper end
that is met.

## `enum LimitsReset<'a> {`

What a text says about its limit's reset, before any of it is parsed: `NoLimit`, it holds no
recognised limit; `NoReset`, it holds one and no limit in it gives a reset; `Reset`, the text after
`resets ` that the first limit giving one gives, to the end of that line.

The two `None`s that `reset_at_from_limit_message` returns differ here, and the difference is what
the coordinator needs. `NoLimit` says the text is about something other than a limit, so another
text about the same failure may still be read. `NoReset`, and a `Reset` that does not parse, say
the text's limit was found and gave nothing this can read, so no other reset, in this text or
another, may stand in for it.

## `fn the_limits_reset(text: &str) -> LimitsReset<'_> {`

The reset of the first recognised limit that gives one. The text is lower-cased once, for
`after_a_quota_window`, and walked line by line, the limits on each line (`after_each_limit_on`)
taken left to right. Lines are those of `str::lines`, so a `\r\n` ending is a line ending too.
`to_ascii_lowercase` changes only ASCII letters, byte for byte, so the two strings have the same
length, with characters at the same offsets. The limit's end is found in the lower-cased text,
where "Fable" reads as a window, and carried to the original line by its distance from the end.
The reset is read from the original, so "resets " must be printed in lower case, as the rest of the
grammar is matched exactly.

On the limit's own line, the reset is `when_it_resets`'s: `resets` as the next word. Failing that,
and only when nothing after the limit on its line is a `resets` at all (`carries_a_reset`), a
`resets ` that begins the very next line, at its first character, is the limit's: the message
wrapped after "limit", as in `You've hit your session limit` then `resets 5:30am (UTC)`. A line
with another reset after the limit (`; the queue resets 5:30am (UTC)`) is about something else,
and so is anything after it. A reset with a blank line, or any other line, between it and the limit
is not the limit's either.

The reading is linear in the text's length, because each limit has only its next word read, which
ends before the next "hit your " begins, and the rest of a line is searched for a `resets` once,
after the line's last limit, rather than once for every limit on it. Reading the rest of the line
at each limit is quadratic in a line that repeats a limit, and the coordinator reads a result after
the agent's process timeout no longer bounds the run.

Asking only the last limit on a line gives the answer asking each one would. What follows a later
limit is a suffix of what follows an earlier one on the same line, and it begins with a character
that is not a word character (`after_a_whole_limit`), so a `resets` that begins a word after the
later limit begins one after the earlier limit too. So if any limit on the line has no `resets`
after it, the last has none; and if a later limit's next word is `resets`, every earlier limit has
a `resets` after it and could not have taken the next line's reset first.

A limit that gives no reset makes the answer `NoReset` rather than `NoLimit`, and the walk goes on
to the next limit, so `You've hit your session limit` on one line and `You've hit your weekly limit
· resets Nov 14, 5pm (UTC)` on the next read the weekly limit's reset: it is a limit's own, which
is the rule. The walk stops at the first limit that gives a reset at all, before it is parsed, so a
malformed reset there is `None` and no later limit's stands in.

Each "hit your " is tried as in `hits_a_quota_window`, so the two recognise the same limits, but
the piece after it runs to the end of the text rather than to the next "hit your ". The two differ
only for a limit with "hit your " directly against it ("limithit your"), which one reads as a limit
with nothing after it and the other not as a limit; neither has a reset to take.

## `fn after_each_limit_on<'text>(`

What follows each recognised limit on `line`, to the end of the line, in order. `lower_from_line`
is the lower-cased text from the line's start to the end of the text, not only to the end of the
line: `after_a_quota_window` reads the limit there, so a model limit whose `resets` begins the next
line ("hit your Fable limit" then "resets Nov 14, 9am (UTC)") is still a limit.

## `fn without_its_ending(line: &str) -> &str {`

A line of `split_inclusive('\n')` as `str::lines` would give it: without its `\n`, and without a
`\r` just before that. `the_limits_reset` walks `split_inclusive` rather than `lines` because it
needs each line's length with its ending, to know where the line starts in the lower-cased text.

## `fn carries_a_reset(text: &str) -> bool {`

Whether `resets ` begins a word anywhere in the text: at its start, or after a character that is
not an ASCII letter, ASCII digit or underscore, the boundary `after_a_whole_limit` uses. So `loaded
presets 5:30am (UTC)` after a limit does not stop the next line's reset from being the limit's.

## `const MONTHS: [&str; 12] = [`

The twelve abbreviations, in order, so the position gives the month's number. `Sept` and
`November` are not in it, and are refused rather than guessed at: neither has been printed.

## `fn leading_month(text: &str) -> Option<(i64, &str)> {`

A month abbreviation and the space after it. `None` here is not yet a refusal: the reset may be a
time only. A text that starts with neither a month nor an hour is refused by `time_of_day`.

## `fn leading_digits(text: &str) -> Option<(i64, &str)> {`

One or two ASCII digits at the start of the text, as a number. Three digits or none are refused,
and so is a sign, which `str::parse` would otherwise accept.

## `fn time_of_day(text: &str) -> Option<(i64, &str)> {`

The time of day as seconds after midnight, and what follows it. `hour % 12` is what makes `12am`
midnight and `12pm` noon: the twelve wraps to zero and `pm` adds twelve hours.

## `fn epoch_day(year: i64, month: i64, day: i64) -> Option<i64> {`

The day number of a date, counted from 1970-01-01, or `None` when the month does not have that
day. `util::days_from_civil` counts a missing day on into the next month, so the date is converted
back with `util::civil_from_days` and must come back unchanged. That is how `Feb 29` is a candidate
in 2028 and not in 2027, and how `Nov 31` is a candidate in no year, so it gives `None`. It returns
days rather than seconds so that a date the month lacks and a time that overflows stay apart: the
first only drops a candidate, the second refuses the whole reset.

## `mod tests` › `fn a_successful_answer_from(id: &str) -> String {`

A stdout each shipped adapter parses as a **success**, in that adapter's own answer shape. It is
what makes the supervision grid below hostile: with a failure payload, dropping the supervision
checks would still report `AgentError` and the cells would pass for the wrong reason.

## `mod tests` › `fn every_adapter_maps_every_supervision_result_the_same_way() {`

Each adapter in `ADAPTERS` maps each of the grid's six supervision shapes to the same status (18
cells, three distinct statuses). The supervisor's two flags are inputs to parsing: `output_limited`
means the tree was terminated with the transcript truncated, and `timed_out` that it was terminated
for exceeding its wall clock. A truncated transcript authorizes nothing, and a timeout is a distinct
ladder input from a generic agent failure.

The domain is `ADAPTERS`, so no adapter is left out of the grid, and the expectations are literals,
a table rather than a re-derivation of the branch order: the two rows where an exit code of 0 meets
a set flag are the rows a re-derivation would get wrong in the same direction the code would.

## `fn every_adapter_maps_every_supervision_result_the_same_way` › `type SupervisionCell = (`

One supervision shape: name, exit code, `timed_out`, `output_limited`, the status every adapter must
report, and a substring the detail must carry (empty: not checked).

## `fn every_adapter_maps_every_supervision_result_the_same_way` › `assert_eq!(outcome.duration, out.duration, "{cell}: duration");`

The duration is the supervisor's, in every cell.

## `fn every_adapter_maps_every_supervision_result_the_same_way` › `assert_ne!(OutcomeStatus::Timeout, OutcomeStatus::AgentError);`

A `Timeout` is a different answer from an `AgentError`, which is what makes cells 4 and 5 (the two
timeout rows) worth having: the ladder acts on it.

## `mod tests` › `fn an_agent_probe_request_is_slotted_names_its_agent_and_carries_the_probe_identity() {`

What an agent probe is: slotted, and naming its agent in its role, its `agent` field and its
identity (`p.agent-claude-code.o0`), never the shell probe, the other probe target, which is not
slotted. The expected values are literals, not read back from the request under test.

## `fn an_agent_probe_request_is_slotted_names_its_agent_and_carries_the_probe_identity` › `assert_ne!(request.invocation.render(), "p.shell.o0");`

The role and the identity's target name the same agent, and neither is the shell probe's: a request
whose identity said `shell` would be a non-slotted process wearing a slotted role.

## `fn an_agent_probe_request_is_slotted_names_its_agent_and_carries_the_probe_identity` › `let ids: std::collections::BTreeSet<String> = ADAPTERS`

Each of the three shipped adapter ids gives a probe request, and the three are distinct identities
at the same ordinal: the target is a field, not decoration.

## `fn an_agent_probe_request_is_slotted_names_its_agent_and_carries_the_probe_identity` › `let ordinals: std::collections::BTreeSet<String> = (0..4)`

Four ordinals of one adapter give four identities, which keeps identities unique per process for a
pre-flight that starts several processes, such as `--version` and then `--help`.

## `mod tests` › `fn a_probe_request_refuses_an_agent_id_that_would_not_survive_a_container_name() {`

An id that could not survive in an invocation identity is refused rather than carried:
`claude.code`, `clau de`, the empty id and `codex/../etc` are refused, and the three shipped ids and
`aider_2` accepted. An invocation id names a container and an intent file (`validate` in
`src/runner/invocation.rs`), and `.` is the identity's own field separator.

## `mod tests` › `fn the_observed_hit_your_five_hour_limit_message_signals() {`

The message Claude Code printed when an account reached its limit, middle dot included. None of the
twelve phrases matches it, so only the quota-window rule's hours window makes it signal.

## `mod tests` › `fn a_hit_your_weekly_limit_message_signals() {`

The weekly window in the same wording. This text has not been observed from the CLI; the test pins
what the rule claims to cover, on the expectation that the weekly message is worded like the
five-hour and session ones.

## `mod tests` › `fn the_recorded_hit_your_session_limit_message_signals() {`

`You've hit your session limit`, as a worker that returned rc=1 printed it. None of the twelve
phrases matches it, so removing "session" from the windows fails this test.

## `mod tests` › `fn a_hit_your_context_window_limit_message_does_not_signal() {`

A context-window limit is a fault in the task, not a quota window: a shorter prompt fixes it and
waiting does not. The word after "hit your " is "context", which is none of the windows or model
names, so the text does not signal.

## `mod tests` › `fn hit_your_limit_with_no_window_does_not_signal() {`

"you hit your limit" names no window, so it does not signal; a rule that made the window optional
would fail this test.

## `mod tests` › `fn an_hours_window_is_spelt_hour_then_limit() {`

"hit your 5-hours limit" has digits and "-hour", then "s limit", not " limit", so it does not
signal. A rule that looked for " limit" anywhere after the window, or accepted "-hours", would fail
this test.

## `mod tests` › `fn a_window_after_a_later_hit_your_signals() {`

The first "hit your " is followed by "branch", the second by "weekly limit". The text signals because
each occurrence of "hit your " is checked; a rule that read only the first, as `split_once` does,
would fail this test.

## `mod tests` › `fn limit_before_hit_your_and_none_after_does_not_signal() {`

"limit" before "hit your", with no window after it, does not signal: the rule reads only what follows
"hit your ". The first text, "no limit was hit; your change is in", never contains "hit your " (the
semicolon), so it is a fair negative that cannot tell one rule from another. The second has a real
"hit your " followed by "branch", with "limit" only before it: a rule matching "limit" anywhere in a
text that contains "hit your" would signal on it and fail this test.

## `mod tests` › `fn hit_your_without_limit_does_not_signal() {`

"hit your" alone is ordinary English, as in a patch that hit your branch, not a limit message; a
rule that signalled on those words alone would fail this test.

## `mod tests` › `fn a_session_limiter_does_not_signal() {`

"hit your session" and then " limiter": "limit" is followed by "e", a letter, so it does not end
there and the text does not signal. A rule with no boundary after " limit" would fail this test.

## `mod tests` › `fn a_weekly_limitation_does_not_signal() {`

The weekly window followed by " limitation", so the text does not signal; the same unbounded rule
would fail this test through the weekly window.

## `mod tests` › `fn a_five_hour_limited_quota_does_not_signal() {`

The hours window followed by " limited", so the text does not signal. With the two tests above, each
of the three windows has a boundary negative of its own.

## `mod tests` › `fn a_full_stop_after_limit_signals() {`

"limit" ended by a full stop, which is not a word character, so the text signals. A boundary that
accepted only a space or the end of the text would fail this test.

## `mod tests` › `fn limit_at_the_very_end_of_the_text_signals() {`

"limit" as the last word, with nothing after it, so the text signals. The text is the one
`a_hit_your_weekly_limit_message_signals` uses, here to pin the end-of-text half of the boundary
beside the full stop above: a boundary that demanded a following character fails this test.

## `mod tests` › `fn the_observed_hit_your_fable_limit_message_signals() {`

The dated message Claude Code printed when the Fable model's own allowance was spent (`FABLE`, the
text the reset tests below read). None of the twelve phrases matches it, so only the name form of the
quota-window rule makes it signal.

## `mod tests` › `fn a_hit_your_opus_limit_message_with_a_reset_signals() {`

The name form with another model's name and a time-only reset, not observed. A rule that listed only
the observed name would fail this test.

## `mod tests` › `fn each_claude_model_name_with_a_reset_signals() {`

"Opus", "Sonnet", "Haiku" and "Fable", each followed by a limit and a reset, with time-only and dated
resets, and two in another case ("SONNET", "haiku"). Only Fable has been observed. Each of the four
names is a case, so dropping one from the list, or comparing the names case included, fails this
test.

## `mod tests` › `fn a_context_token_or_request_limit_with_a_reset_does_not_signal() {`

"hit your context limit · resets after compaction" is a Copilot or Codex message about the context,
not the pool; the other three are "context", "token" and "request" limits with a reset that parses.
Each is one word of ASCII letters with "resets" as the next word after "limit", and none is one of
Claude's four names, so none signals. A rule that took any one-word name with a reset fails this
test.

## `mod tests` › `fn a_name_that_is_not_one_of_claudes_four_with_a_reset_does_not_signal() {`

Names close to the list but not on it: "Codex", "Copilot", "Opuses" and "Fables". The name is
compared whole, so none signals; a rule that took any name, or asked only whether a name started
with one of the four, fails this test.

## `mod tests` › `fn a_hit_your_token_limit_message_without_a_reset_does_not_signal() {`

"hit your token limit" alone and "hit your context limit" with a semicolon and advice: both are about
the prompt, not the pool, and neither "token" nor "context" is a model name, so neither signals.

## `mod tests` › `fn a_model_named_limit_without_a_reset_does_not_signal() {`

`FABLE` with its reset cut off does not signal. A model's name says what was spent; the reset is what
makes the message a quota, which "session", "weekly" and the hours window do not need.

## `mod tests` › `fn a_named_limit_whose_next_word_is_not_resets_does_not_signal() {`

"resets" appears after the limit, but the next word after "limit" is "the". The reset has to follow
the limit, not merely appear later in the text. The "Fable" text is the case that tells this rule
from one that looks for "resets" anywhere after the limit; "token" does not signal with any reset,
so its text cannot.

## `mod tests` › `fn a_named_limiter_with_a_reset_does_not_signal() {`

`FABLE` with "limiter" for "limit". The name form keeps the whole-word boundary of the three
windows, so the reset after it does not help; a name form that skipped `after_a_whole_limit` would
fail this test.

## `mod tests` › `fn a_name_of_more_than_one_word_with_a_reset_does_not_signal() {`

"context window", "context-window" and "Fable4", each followed by a limit and a reset. The name stops
at the space, the hyphen and the digit, and what follows is not " limit", so none signals. Since
"context" is refused whatever follows it, "Fable4", which starts with a listed name, is the case that
pins where the name ends.

## `mod tests` › `const NOV_11_2026: u64 = 1_794_355_200;`

The reset tests read their messages at fixed instants, never at the clock's. The six dated
constants from `NOV_11_2026` to `FEB_27_2029` are UTC midnights written as literals, and
`the_reset_fixtures_are_the_midnights_they_are_named_for` checks each against the date it is named
for, so a miscounted constant fails there, by name, rather than as a wrong reset further down.
`JAN_1_10000` is checked there by its day count from `util::days_from_civil` instead, since its
year has five digits and `util::rfc3339_utc` writing it is not what the check is for.

## `mod tests` › `fn after_a_limit(reset: &str) -> String {`

A reset as the session limit gives it: `You've hit your session limit · ` before the reset's text.
A reset is read only from a limit, so a bare one gives `None` whatever it says, and a malformation
test written bare would pass with a parser that read nothing. The session limit is used because it
signals alone and takes either form of reset, time-only or dated.

## `mod tests` › `fn the_observed_five_hour_message_resets_later_the_same_day() {`

`FIVE_HOUR` read at 03:00 on 11 November 2026: the reset is 05:30 that day.

## `mod tests` › `fn the_observed_fable_message_resets_on_its_date() {`

`FABLE` read on 12 November 2026: the reset is 14 November 2026 at 09:00.

## `mod tests` › `fn a_time_only_reset_read_just_after_itself_is_its_own() {`

`FIVE_HOUR` read at 05:30 exactly, at 05:30:01 and at 06:00 gives 05:30 that morning each time, not
the next morning's.

## `mod tests` › `fn a_time_only_reset_read_the_evening_before_is_the_next_mornings() {`

Read at 20:00, `5:30am` is the next morning's, 9.5 hours ahead rather than 14.5 hours past. 17:30 is
halfway between the two mornings: one second before it the morning just gone is the nearer, and at
it the two are equally near and the later is taken, so both sides of the tie are pinned.

## `mod tests` › `fn twelve_am_is_midnight_and_twelve_pm_is_noon() {`

Read at 01:00: `12am` and `12:45am` are today's 00:00 and 00:45, an hour and a quarter of an hour
past; `12pm` and `12:45pm` are today's 12:00 and 12:45. `1am` is now, `1pm` is twelve hours either
way and so the later, today's, and `11:59pm` is yesterday's, just over an hour past. Reading `12am`
as 12:00, as a parser that only adds twelve for `pm` would, fails here.

## `mod tests` › `fn a_dated_reset_is_its_own_until_the_next_years_is_nearer() {`

`FABLE` read at 20:00 on 14 November, at 09:00 on 15 November and a second after that gives 14
November 2026. The two years' resets are 365 days apart, so halfway between them is 21:00 on 15 May
2027, which the test asserts before using it: one second before it the reset is still 2026's, and
at it the two are equally near and the later, 2027's, is taken.

## `mod tests` › `fn a_dated_reset_across_a_year_end_is_next_years() {`

Read at 20:00 on 31 December 2026, `Jan 2` is 2 January 2027, a time-only `5am` is 1 January 2027,
nine hours ahead, and `Dec 31, 11pm` is later that same day. Read at noon on 30 December, `Jan 2`
is still the coming 2 January, not the one just under a year past.

## `mod tests` › `fn a_reset_read_just_after_a_year_end_is_last_years() {`

At 00:30 on 1 January 2027, `Dec 31, 11pm` is 31 December 2026, an hour and a half past, not 31
December 2027, the date in the year read from the clock. A time-only `11pm` read then is the same
instant, yesterday's.

## `mod tests` › `fn feb_29_is_a_neighbouring_years_leap_day_or_none() {`

`Feb 29, 3pm` read on 27 February of 2026 to 2029. In 2028 it is 29 February 2028. In 2027 and 2029,
years without that day, it is also 29 February 2028, the leap day in the next year and in the year
before: a neighbouring leap year is a candidate whether that puts the reset a year ahead or in the
past. In 2026 it is `None`, since none of 2025, 2026 and 2027 has the day; it is never moved to 1
March.

## `mod tests` › `fn a_zone_other_than_utc_gives_none() {`

The five-hour message with `(Europe/London)` in place of `(UTC)` gives `None`, and so do, each
after a limit (`after_a_limit`), `(GMT)`, a lower-case `(utc)`, `UTC` without parentheses or without
the space before them, no zone at all, and a dated reset in `(America/New_York)`.

## `mod tests` › `fn text_without_a_reset_gives_none() {`

Texts with no reset in them: the empty text, the session-limit message, the older "5-hour limit
reached", a time and zone with no `resets` before them, `reset` without the `s`, `resets` with
nothing after it, and `presets`, which contains `resets ` but not as a word. Each gives `None` alone
and after a limit, so adding a limit does not make them read.

## `mod tests` › `fn malformed_hours_minutes_months_and_days_give_none() {`

One malformation per text, each otherwise in the observed shape: hours 0, 13 and 123, a signed hour,
a space before `am`, upper-case `AM`, no `am` or `pm`, no hour, minutes 60 and 99, no, one or three
minute digits, a `.` for the `:`, a misspelt, lower-case, unabbreviated or four-letter month, 31
November, 31 April, 30 February, day 0, a three-digit day, no day, no comma, no space after the
comma, and a dated reset with an hour of 13 or no time. Each is read after a limit, so it is the
malformation that gives `None`, not the want of a limit.

## `mod tests` › `fn the_reset_after_the_limit_is_read_not_an_earlier_unrelated_one() {`

`MCP cache resets 9am (UTC)` on the line before the five-hour message gives 05:30, not 09:00. So
does the same with `\r\n` line endings, with the lines the other way round, with a session limit
below a bare reset, and with the unrelated reset on a line that has a "hit your" which is not a
limit: a context-window limit or "the patch hit your branch". `FABLE` under the same line gives its
own 14 November.

## `mod tests` › `fn several_resets_and_none_after_a_limit_give_none() {`

Two resets and no limit's to choose give `None`: two resets with no limit at all (two texts), and,
each below an unrelated reset, a limit whose line has a reset after another word ("the queue
resets") and a context-window limit, which is not a quota window.

## `mod tests` › `fn a_reset_beginning_the_line_after_a_limit_with_none_is_the_limits() {`

When nothing after the limit on its line is a reset, a `resets` that begins the very next line is
the limit's: the session limit with `resets 5:30am (UTC)` under it gives 05:30, and so does the same
after a full stop with `\r\n` endings, below an unrelated reset, and above one, and a five-hour limit
followed by advice ("try again later"). A Fable limit wrapped the same way gives its 14 November:
the model name signals on the reset that follows it across the line break, and that reset is the
one read.

## `mod tests` › `fn a_reset_no_limit_gives_is_not_read() {`

A reset that no limit gives is not read, even when it is the text's only one: `MCP cache resets
5:30am (UTC)` alone, the same as a failure's reason, a session limit with `MCP cache resets 9am
(UTC)` after a blank line or after another line, a session limit with a bare `resets 9am (UTC)`
after a blank line, the cache's reset on the very next line (it does not begin the line), a reset
after "the queue" on the limit's line with or without a bare reset on the next, and a reset before
the limit on its line. Each gives `None`.

## `mod tests` › `fn names_a_quota_limit_recognises_the_limits_a_reset_is_read_from() {`

`names_a_quota_limit` is true for a session limit with no reset, a five-hour
limit whose reset is in another zone, a weekly limit below an unrelated
reset, and the observed Fable message, and false for the empty text, an
unrelated reset alone, a reason with no limit, the older "5-hour limit
reached", a context-window limit with a reset, and a Fable limit without
one. The coordinator reads only the first of a failure's texts that it is
true for, so a text it is wrongly true for blocks a later text's reset, and
one it is wrongly false for lets one through.

## `mod tests` › `fn a_malformed_reset_after_the_limit_is_not_passed_over() {`

A limit's reset with an hour of 13, or in `(Europe/London)`, gives `None`
though the text's other reset parses: an unrelated one above or below, a
bare `resets 9am (UTC)` on the next line, a later limit's own valid reset,
or, for a malformed reset on the line after a limit, an unrelated one below
it. The limit's reset is the one the walk stops at, so no other stands in. Two resets with no limit,
the first malformed, give `None` as well, because neither is a limit's.

## `mod tests` › `fn arbitrary_non_ascii_text_never_panics() {`

Texts with non-ASCII characters where the reset's parts would be (a full-width digit, an accented
month or `am`, full-width `UTC`, a no-break space, a middle dot in place of each space, Japanese
before the message, an emoji or a NUL against `resets`; and, for the limit's reset, limits with a
no-break space before `resets`, a non-ASCII model name, an accented `résets`, a limit in capitals, a
limit whose `resets` has nothing after it and a bare "hit your "), each read at five instants from 0
to `u64::MAX`, with both observed messages. With them, each proper prefix and each non-empty suffix
of both observed messages, cut at each character boundary, so a cut lands inside every part of the
grammar, including just after the multibyte middle dot. Each prefix stops before the closing `)`
and must give `None`. None of these may panic, and at `u64::MAX`, past what an `i64` second count
holds, both messages give `None`.

The Japanese text before a lone reset gives `None`, since it has no recognised limit, and the same
Japanese before the five-hour message gives 05:30: the multibyte prefix does not move the offsets
the limit's end is carried by. The full-width minute is read after a limit for the same reason as
`after_a_limit`'s other texts.

## `mod tests` › `fn a_reset_in_the_year_10000_gives_none() {`

Finding `PR380-RESET-PARSER-NON-RFC3339-YEAR`. `FIVE_HOUR` read at 10000-01-01T00:00:00Z
(`JAN_1_10000`) gives `None`: its nearest candidate is 05:30 that morning, whose year has five
digits. So do a time-only `5:30am` read at 23:00 on 31 December 9999, whose nearest candidate is the
next morning's, in 10000, and `Jan 1, 9am` read at that midnight and `Jan 2, 9am` read at noon on 31
December 9999, both nearest in 10000.

## `mod tests` › `fn a_reset_on_the_last_day_of_9999_is_four_digits() {`

The other side of the bound. `FIVE_HOUR` read at 03:00 on 31 December 9999 is
`9999-12-31T05:30:00Z`, and `11:59pm` read at its last second is `9999-12-31T23:59:00Z`, the last
minute a reset can name. `Dec 31, 11pm` read at noon that day is that evening, though one of its
candidates is in 10000. Read at 00:30 on 1 January 10000, `Dec 31, 11pm` and a time-only `11pm` are
both the evening before, in 9999: a clock past the bound does not refuse a reset that is not. A
check that refused a reset at or after the last day of 9999, or one that looked at `now_unix` rather
than the chosen reset, fails here.

## `mod built_program_tests` › `struct Boundary {`

A boundary with an agent CLI installation the test invents.

**This is the test's oracle, and it is deliberately not this machine's filesystem.** Which
environment an adapter's program was resolved against cannot be measured with a predicate over the
filesystem production consults: `is_file()` is true of a host-resolved path whether the resolution
was right or wrong. A boundary the test invents has an installation the test knows about and the
host does not, so "the boundary decided" and "the coordinator host decided" are different
observations.

It refuses any program but the one it has, as a real boundary would, and records every request, so
a boundary that was **never asked** is distinguishable from one that answered. It reports a
**version of its own**, because `Caps.version` certifying the host's CLI while the attempt runs the
image's is a failure of its own (design/21's runner commitments: pre-flight "could certify a host
CLI/version different from the one the attempt executes").

## `struct Boundary` › `installed: String,`

The only program string this boundary will execute.

## `struct Boundary` › `version: String,`

What `--version` prints here: an invented version, so one read from the host's CLI would not match
it.

## `mod built_program_tests` › `fn expected_name(id: &str) -> &'static str {`

The bare name each adapter's CLI is installed under, **written here** rather than read from the
adapter's own constant: a name compared only against the code that produced it proves nothing.

## `mod built_program_tests` › `fn scripted_help(cli: &str) -> &'static str {`

The help text each adapter's `--help` contract demands, written here from the flags each adapter
requires rather than captured from a CLI.

## `mod built_program_tests` › `fn scripted_answer(cli: &str, args: &[String], version: &str) -> (i32, String, String) {`

A boundary that can satisfy a whole pre-flight, answering **by argument** and by its own installed
CLI, never by the program a request carried. Keying an answer on that program would make the fixture
agree with whatever the adapter sent. The one place it is consulted is the presence check in
`Boundary`'s `run_inline`, which is the boundary's own question, "do I have this?", and the thing
under test.

## `fn scripted_answer` › `if joined.contains("upstroke_probe_deliberately_unknown") {`

Codex's six strict-config parser probes. The unknown key and the `model_reasoning_effort=`
assignment are transcribed here rather than read from `codex.rs`, so a renamed probe key fails this
fixture instead of agreeing with itself.

## `mod built_program_tests` › `fn an_adapters_program_is_the_boundarys_and_the_coordinator_host_is_never_asked() {`

An adapter's program is the **boundary's**, and the coordinator host is never asked what it has.
For each shipped adapter, `build`'s program is the name written in `expected_name`, one path
component and not absolute, so a spec never carries a path that names nothing at the boundary;
`probe` against a boundary holding that name certifies the boundary's invented `9.9.9`; and the
boundary was asked, every request carried that name, and the first was `--version`. That pre-flight
and the attempt send one program in both call orders is held by
`the_program_preflight_certifies_is_the_program_the_attempt_would_run`, that a CLI this host lacks
still certifies by `a_cli_this_host_does_not_have_still_certifies_at_the_boundary`, and a version
per boundary by `two_boundaries_in_one_process_each_certify_their_own_cli`.

The assertions hold whichever branch `find_program` takes on a machine; both branches are counted
and their sum checked against `ADAPTERS.len()`.

## `fn an_adapters_program_is_the_boundarys_and_the_coordinator_host_is_never_asked` › `match crate::util::find_program(name) {`

Where this host has the CLI, the built program must not be the host's resolved path; where it does
not, the probe above has already certified at the boundary.

## `mod built_program_tests` › `fn a_cli_this_host_does_not_have_still_certifies_at_the_boundary() {`

A CLI this coordinator host does not have certifies anyway, because the boundary has it: design/21's
container case is "an image with version-pinned CLIs". For each adapter whose CLI `find_program`
does not find here, `probe` certifies `9.9.9` and `build` succeeds.

The premise is asserted rather than hoped for: with all three CLIs installed here the absence half
is unobservable, and a silent skip would measure nothing while looking green, so the test fails
instead.

## `mod built_program_tests` › `fn a_cli_this_host_does_not_have_is_refused_by_name_and_by_boundary() {`

The other side of that: when the boundary **is** this host and this host does not have the CLI, the
operator is told so, by name and by boundary. The runner's own refusal is asserted in
`runner::host::tests`; this is the sentence the operator reads, `bin::boundary_refused` wrapping it.
It must name the CLI, the boundary that lacks it and the remedy. A bare `NotFound` names no boundary
and no remedy, and cannot be diagnosed from a log.

The premise, that this machine lacks at least one of the three, is asserted as in the sibling above,
so a machine with all three installed fails instead of passing vacuously.

## `fn a_cli_this_host_does_not_have_is_refused_by_name_and_by_boundary` › `assert!(`

And it is the **resolution's** refusal, not a spawn's: a runner that handed the name to `Command`
anyway would produce a `NotFound` here, which names no boundary and is what an operator cannot act
on.

## `fn a_cli_this_host_does_not_have_is_refused_by_name_and_by_boundary` › `assert_eq!(`

One distinct message per absent CLI: a refusal that named the first adapter for all of them would
collapse this to one.

## `mod built_program_tests` › `fn the_program_preflight_certifies_is_the_program_the_attempt_would_run() {`

Pre-flight certifies the program the attempt would run, in **both call orders**: `build` then
`probe`, and `probe` then `build`, each against a fresh boundary. The two builds must be equal, the
two boundaries must have been sent the same programs, and each program pre-flight sent must be the
one `build` produced. Resolution is a function of its argument, not state, so the order cannot
matter, and asserting both orders is what says so.

## `mod built_program_tests` › `fn two_boundaries_in_one_process_each_certify_their_own_cli() {`

Two boundaries in one process each certify **their own** CLI. With two runners in one process, as
the container runner brings, a resolution cached on nothing hands one boundary's answer to the
other, and a value correct on first use and wrong on the second is invisible to any test that
constructs one runner. This fails if an adapter hands one boundary's version to the next.

The test is the intersection of two varying fields: **which boundary** (two, reporting 1.1.1 and
2.2.2) x **in which order** (both). The distinct versions are asserted as a count, so a fixture that
lost a version reports it instead of agreeing with itself.

## `mod built_program_tests` › `fn the_adapters_hold_no_process_wide_resolution_state() {`

No adapter holds process-wide resolution state, and this is the answer to "what is the cache keyed
on": **nothing is cached, so there is no key.** A cell keyed on nothing would, with two runners in
one process, hand one boundary's answer to the other, and a cache of a constant is
indistinguishable from no cache to a *behavioural* test even with two runners. So the claim is
structural: an adapter that starts remembering an answer in a `static`, `thread_local!`, `OnceLock`
or `LazyLock` written in `bin.rs` or one of the three adapters' source files fails here on that
line, before any behaviour depends on it. State held another way, such as a `Mutex` field added to
an adapter struct, or declared in a file the checker does not read, such as `adapter.rs`, is not
seen.

The checker uses `effects::production_code`, which blanks comments and literals and removes
test-only items, so a string constant containing `OnceLock` does not declare state. It reads
`bin.rs` and the three adapters' source files, and first runs controls: inert comments and
literals, excluded test-only state, and four real holder declarations, alone and after the inert and
test-only text, so a production declaration after a test-only item remains visible. The `static `
needle skips a match preceded by an apostrophe, so a `'static` lifetime does not count as a static
declaration; the inert control's `&'static str` covers it.

## `fn the_adapters_hold_no_process_wide_resolution_state()` › `const HOLDERS: [&str; 4] = ["static ", "thread_local!", "OnceLock", "LazyLock"];`

The needles for a value held across calls, written out rather than derived: a list read from the
tree would shrink with it.

`static ` rather than a list of cell types: each adapter is a unit struct with no state of its own,
so the only way one remembers anything between calls is a module-level item, and naming `OnceLock`
alone would miss a `static X: Mutex<_>`. `OnceLock` and `LazyLock` are named as well so a
`let`-bound or field-held cell is caught too.

## `fn the_adapters_hold_no_process_wide_resolution_state()` › `assert!(`

The controls run real holder declarations through the checker and assert the holder names found,
alone and after inert literal and comment text and a test-only item, so neither region can hide
later production state.

## `mod built_program_tests` › `fn the_host_runner_executes_a_bare_program_name_as_it_executes_the_resolved_path() {`

The host runner executes a bare program name as it executes the absolute path `util::find_program`
picks for it: a change that resolves correctly for a container boundary must not change what the
**host** runner runs. This spawns `git --version` through the real
`crate::runner::host::HostRunner`, once named and once at that path, and requires the same exit code
and output. `git` rather than an agent CLI because every machine that can build this repository has
it. The two program strings are asserted to differ first; if they did not, this would compare a
thing with itself.

**What this row cannot express, and where that lives instead.** `git` is a native `.exe`, and on
Windows `CreateProcessW` appends `.exe` to a bare name, so the bare spelling reaches it whether or
not this runner resolves anything. As a witness for the Windows case this row is therefore
correlated. It is kept as a control, a real program on this machine's real `PATH`, and the axis it
cannot vary, an npm-style `.cmd`-only installation reachable only through `PATHEXT`, is
`runner::host::tests::an_npm_style_installation_runs_by_bare_name_exactly_as_it_runs_by_path`. That
test lives beside the resolution because writing a shim needs the effect allowance
`effects/allowlist.toml` gives `src/runner/host.rs` and does not give this module.

## `mod probe_identity_tests` › `fn each_agent_probe_request_names_its_own_agent_in_every_field() {`

Each agent probe names **its own** agent, in its role, its `agent` field and its rendered identity.
A fixture that probes one agent cannot tell this from a `probe_request` that filled the target with
the first configured adapter's name, so this builds three, each checked against the name it was
asked for rather than against another request. Both iteration orders, because "the first configured
agent" is a property of order.

## `fn each_agent_probe_request_names_its_own_agent_in_every_field` › `const NAMES: [&str; 3] = ["claude-code", "codex", "copilot"];`

Written here, not read from the adapter registry: the names are the expected values.

## `fn each_agent_probe_request_names_its_own_agent_in_every_field` › `assert_eq!(roles.len(), 3, "{roles:?}");`

Hostility as counts: three names in, three distinct values out of each field that carries one.

## `mod probe_identity_tests` › `fn use_sites(source: &str) -> Vec<String> {`

The file's lines that mention `probe_ordinal::`, skipping each `#[cfg(test)]` module, the
`mod probe_ordinal { … }` declaration and comment lines, so what is left is the production code that
*uses* an ordinal.

## `fn use_sites(source: &str) -> Vec<String>` › `if trimmed.contains("probe_ordinal::")`

Prose mentions an ordinal too, and a comment starts no process.

## `mod probe_identity_tests` › `fn every_probe_call_site_passes_its_own_ordinal_constant() {`

Every ordinal an adapter's pre-flight passes to the Runner, **read from the call sites** rather than
from the table beside them.

An invocation identity is unique per process. Each adapter's
`every_preflight_process_has_its_own_ordinal` builds its set from the `probe_ordinal::ALL` array, so
it asserts that a *table* has distinct entries, which stays true when a call site passes another
entry's constant (codex's `debug models` step passing `VERSION`) or an arithmetic expression over
one (`HELP.saturating_sub(1)`). Two processes then carry one identity and the ledger cannot tell
them apart.

This asserts the property at the point of use: each constant a use site names is declared, none is
used twice, every declared one is used, and the only non-bare uses are codex's documented block, a
base plus an index, for its six strict-config parser probes.

## `struct Adapter` › `declared: &'static [&'static str],`

The constants the module declares, written out here from the steps the adapter performs rather than
read from the module.

## `struct Adapter` › `block_parts: &'static [&'static str],`

Constants that may appear in an expression, with the block they open. Everything else must reach
the Runner as a bare `probe_ordinal::NAME` argument.

## `struct Adapter` › `call_sites: usize,`

How many processes this adapter starts through `probe_request`, counting each variable-length block
as one call site.

## `fn every_probe_call_site_passes_its_own_ordinal_constant()` › `block_parts: &["CONFIG_BASE", "CONFIG_PER_SURFACE"],`

The one variable-length step: six strict-config parser probes (two surfaces x three assignments).
`probe_ordinal` documents it as a block because it cannot be one constant.

## `fn every_probe_call_site_passes_its_own_ordinal_constant()` › `let collisions: Vec<(&String, &usize)> =`

No constant is used twice: that is the collision this exists to catch, and it is what a table-only
test cannot see.

## `fn every_probe_call_site_passes_its_own_ordinal_constant()` › `let declared: BTreeSet<&str> = adapter.declared.iter().copied().collect();`

And every declared constant is used: an ordinal declared and never passed is a step whose identity
came from somewhere else.

## `fn every_probe_call_site_passes_its_own_ordinal_constant()` › `let calls = adapter`

The number of processes, counted from the call sites rather than from the table.

## `fn every_probe_call_site_passes_its_own_ordinal_constant()` › `assert_eq!(total_sites, 12, "the adapters' probe call sites moved");`

Hostility as a count: 3 + 2 + 7 across the three adapters, written from what each pre-flight does.
