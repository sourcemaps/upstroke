# `src/capacity.rs`

Extended notes for [`src/capacity.rs`](../../src/capacity.rs).

[Source on GitHub](https://github.com/sourcemaps/upstroke/blob/master/src/capacity.rs).

The code defines current behavior. These notes preserve contracts and implementation
history. Search each backticked heading fragment separately in the source.

References below to `INV-18` use retired v0.2 planning identifiers.
They record implementation history and do not add current requirements.
[DESIGN.md](https://github.com/sourcemaps/upstroke/blob/master/DESIGN.md#retired-records)
is the living design authority.

## Module

The capacity engine (DESIGN.md §13) — **read-only in v0.1**.

Three things live here: the pool types `~/.upstroke/pools.toml` parses into,
the estimator that turns observations into a per-pool remaining figure, and
the folds that collect those observations from a run's event log: `observe`
for a legacy log and `observe_topology` for a schema-4 one. `upstroke
capacity` and `validate` reach the same rules through
`capacity::machine::gather`, which reads every run in scope by the reader of
its log's schema.

Since 2026-11-23 the module has three children, each with its own rustdoc in
its source rather than a notes file. `src/capacity/evidence.rs` is where the
fold's rules are applied: a log is first reduced to evidence that names pools
and nothing else, so that the logs of several runs can be put in one sequence,
and `observe`, `observe_topology` and `exhausted_pools_of` below are one call
each into it.
`src/capacity/machine.rs` gathers that evidence from every run on the machine
and reads the provider reports the capacity tracker stored.
`src/capacity/usage.rs` is the schema of those reports. The tracker itself,
the one module that speaks HTTP, is `src/tracker.rs`, and nothing in this
module or its children starts its transport: they read what it wrote.

§13's sequencing is the reason no estimate routes anything yet: v0.1 ships
the estimator so `upstroke capacity` and the dry-run preview can show what each
strategy *would* do, and v0.2 wires it into the binder once the estimates
have been watched for a while. The binder still picks models from the catalog
and pins, exactly as it did before this module existed. One thing does choose
between pools, and it reads no estimate: a schema-4 run binds a new attempt to
the first pool of its agent that the run's own rate-limit signals do not hold
exhausted, and a worker's rate limit fails over to another such pool (see
[Credential profiles](#credential-profiles) below). That choice is made in
`src/engine/topology/run.rs` (`serving_pool`), not here. `pool_for` answers an
agent's first pool, which still names the pool of its reviews and selects the
profile its review and probe processes run under.

The estimator is a pure function over plain values ([`estimate`], and
[`estimate_gathered`] over what the machine-level gatherer read), so every
rule in §13 is exercisable with no CLI installed and no file on disk. Only
collection touches the world, and even that is a fold over events someone
else read.

Three properties hold by construction and are pinned by tests:

1. **Never optimistic.** No observation means [`Remaining::Unknown`], never
   "full". A pool nobody has measured is not a pool with capacity.
2. **Conservative.** Effective remaining is
   `max(0, raw × (1 − safety_margin) − reserve)` — the margin covers usage
   on other machines that local parsing cannot see, and the reserve is
   headroom the engine leaves for the operator's own interactive work.
3. **Trust order is ranked and fixed.** `Signal > SelfMetered > Reported >
   Assumed > Unknown` ([`Confidence`]), and a lower-ranked source can never
   overwrite a higher one — a rate-limit signal is ground truth, and a
   self-metered guess must not talk it back up. A provider's report
   (`Reported`) is the case the rule was sharpened for: it may *lower* a
   self-metered ceiling, since both are ceilings and the lower of two true
   ceilings is true, and it may never raise one.

### Credential profiles

DESIGN.md §13 calls for one vendor backing several pools (two Claude Max
accounts, say), told apart "through the provider's own profile mechanism, an
environment variable on the subprocess rather than a token the engine ever
handles". That mechanism is real on both vendors and is a config-*directory*
variable: `COPILOT_HOME` (documented) and `CLAUDE_CONFIG_DIR` (works,
undocumented as of Aug 2026). Login stays a user-driven interactive step the
engine never automates.

[`Pool::profile`] is that directory, and it is acted on for Claude Code on the
host runner (§17). On a schema-4 run a worker runs under the `profile` of the
pool its attempt is bound to, and a review or probe process under the `profile`
of its agent's first pool, which is what [`pool_for`] answers. The attempt's
pool is chosen in `src/engine/topology/run.rs` (`serving_pool`): the first pool
naming the agent, in file order, that the run's signals do not hold exhausted;
with every one held, the first whose recorded reset is two minutes past, or
else the first. A reset is spent once its pool refuses again two minutes or
more past it, and then counts as none. A retry that resumes its session
keeps its pool. The host runner then sets the profile as `CLAUDE_CONFIG_DIR`
on the process (`src/engine/assembly.rs` puts the bound pool's on a worker's attempt plan,
`src/engine/topology/profiled.rs` stamps a review or probe request, and
`src/runner/host/environment.rs` composes it). The value is the
configured string used as a path, with no `~/` expansion. A profile the run
cannot honour is refused before anything runs for it, by one function
(`src/engine/topology/preflight.rs`) that both paths call: a fresh run before
its creation begins, so nothing of the run exists, and a resume at its
pre-flight. What pre-flight checks of a profile is that it is an absolute
path and an existing directory (besides the host runner, Claude Code and no
`CLAUDE_CODE_OAUTH_TOKEN`). The agent's capability probe then runs under the
profile. Nothing checks sign-in, so a profile that is not signed in passes
pre-flight and fails at its first attempt (`PR386-PROFILE-AUTH-NOT-PREFLIGHTED`).
The legacy coordinator refuses any profiled pool.
A rate limit moves a task between pools, and so between profiles. When a
schema-4 worker's limit would defer its task and another pool of its agent is
not held exhausted, the task runs again at once on that pool, in a fresh
session under its profile (`TopologyRun::settle_failure`; DESIGN.md §19).
Beyond that binding and this failover, nothing routes on a pool or a profile.
Both read the run's own signals and no estimate, and the failover stays within
one agent and one model. The rest of what several profiles buy (per-profile
attribution, asymmetric reserve, affinity) is capacity-*driven* behaviour,
which is v0.2 by sequencing.

A resume reads today's configuration. A profile changed between a run's
start and its resume is therefore used from the resume on, both in the
resume's pre-flight and by every later attempt, because the run's record does
not carry the profile it started under. Whether to record it is an owner
question.

## `pub enum PoolKind {`

§13's pool shapes. Which one a pool is decides which estimator rule applies,
so an unknown value is a config error rather than a warning.

## `pub enum PoolKind` › `SubscriptionWindow,`

Claude Max and friends: a rolling window plus a weekly cap.

## `pub enum PoolKind` › `Credits,`

Copilot on AI-credit billing: a monthly allowance plus pay-as-you-go.

## `pub enum PoolKind` › `RequestPool,`

Copilot on a legacy annual plan: premium requests × per-model multiplier.

## `pub enum PoolKind` › `ApiKey,`

Direct API billing — dollars, no reset, budget-only.

## `pub enum PoolKind` › `Unmetered,`

A local endpoint: hardware-bound rather than quota-bound.

## `impl PoolKind` › `pub const ACCEPTED: [&'static str; 5] = [`

Accepted spellings, named once so the parser and its error message
cannot disagree about what is legal.

## `pub enum Source {`

§13's estimation sources, in trust order. Listing one is a statement about
where a pool's numbers may come from — dropping `signals` by typo would
discard ground truth, which is why an unknown entry errors.

## `pub enum Source` › `ProviderEndpoint,`

(4) Provider usage endpoints — fragile, never load-bearing. Parsed in
v0.1 and never read there. Read since 2026-11-23, for a pool that names a
`usage_provider`: the capacity tracker (`src/tracker.rs`) asks the endpoint
and stores what it reports, and `src/capacity/machine.rs` reads that back.
Listed with no provider named, it is still parsed and not read, and the
estimate says so.

## `pub enum Source` › `LocalLogs,`

(3) ccusage-style parsing of the agent's own logs, which is what sees
the operator's interactive sessions. Parsed in v0.1, never read.

## `pub enum Source` › `SelfMetered,`

(2) Self-metering of everything this engine spawned.

## `pub enum Source` › `Signals,`

(1) Rate-limit signals from the CLIs — ground truth.

## `pub enum Allowance {`

`monthly_allowance = "auto"` or a number of units.

`Auto` is honest rather than convenient: it means the size of the allowance
is not known to upstroke, which is different from it being zero and different
again from it being unlimited.

## `pub enum UsageProvider {`

The provider usage endpoints the capacity tracker knows, as the configuration
vocabulary names them. The enum is here, with the other pool vocabulary, and
what each variant means on the wire is `src/tracker/provider.rs`'s: a match
there is exhaustive, so a variant added here does not compile until it has a
request and a parser.

**The admission rule is DESIGN.md §13's**: an endpoint its provider documents,
taking a credential the provider issues for the purpose. The four here meet
it. The subscription-window endpoints that most usage tools read do not —
Claude Code's is undocumented and takes the Claude.ai OAuth credential, which
Anthropic's terms reserve for its own applications — and are absent on
purpose, not for want of a parser.

## `impl UsageProvider` › `pub fn parse(s: &str) -> Option<Self> {`

Exact. `PoolKind::parse` and `Source::parse` trim and fold case, and this
does not, for a reason of its own: `connect` carries the key across a rewrite
by writing the parsed value back, so a spelling the reader accepted and the
writer would not reproduce is a file `connect` refuses to replace on its next
run, for a difference the operator cannot see.

## `impl UsageProvider` › `pub const fn needs_account(self) -> bool {`

Whether the endpoint reports per account, and so needs `usage_account` in its
address. GitHub's billing usage does; the two organization cost endpoints
answer for the organization the key belongs to. It also decides whether a
stored report records an account, and so whether changing `usage_account`
retires the report (see [`Reported`]).

## `pub const MAX_TOKEN_ENV_CHARS: usize = 64;`

The longest `usage_token_env` the reader takes; a longer value is refused as
having a credential's shape ([`looks_like_a_credential`]). `src/tracker.rs`
reads the same bound and prints a configured name longer than it cut short,
so whether a name is printed whole is decided by the length the reader
accepts. It is shorter than `MAX_USAGE_KEY_CHARS`, which still bounds the
character-class checks and `usage_account`.

## `pub enum UsageKeyDefect {`

Why three strings from a pools file are not a usage source. The messages are
the configuration reader's refusals and `connect`'s warnings alike, which is
why they live here and not in either.

`TokenEnvNotAName` deliberately does not say what was written. The value of
`usage_token_env` is a variable's name, and the usual way for it not to be
one is that a key was pasted there; an error that quoted it would put the key
on a terminal and in whatever captured the output.

`TokenEnvLooksLikeACredential` keeps the same rule for a value that is a name
by its characters and still has a credential's shape. It lists the shapes it
refuses and never the value. Like `TokenEnvNotAName` it carries no field, so
there is nothing it could repeat a pasted key from; the reader prefixes the
pool's table and `connect` the pool's name. Since 2026-11-23
`TokenEnvNotAName` states the 64 characters of `MAX_TOKEN_ENV_CHARS` and not
the 128 of `MAX_USAGE_KEY_CHARS`: a name between the two is refused too, as
credential-shaped, and the bound worth stating is the one an operator can
meet.

## `impl UsageSource` › `pub fn from_keys(`

The one validation of the usage keys, shared by the configuration reader and
by `connect`'s carry, so that `connect` cannot write a file the reader then
refuses. A key with no provider is a refusal rather than a no-op because the
likeliest cause is `usage_provider` misspelled, which is otherwise a pool
that is silently never asked.

`usage_token_env` takes the name of an environment variable, never a
credential, and two checks hold it to that. `names_an_environment_variable`
refuses what is not a name by its characters (`TokenEnvNotAName`), and then a
value with a credential's shape is refused ([`looks_like_a_credential`],
`TokenEnvLooksLikeACredential`). The first check alone was not enough. A
GitHub fine-grained token is `github_pat_` followed by letters, digits and
underscores, so it is a name by its characters. Until 2026-11-23 the reader
took one, the tracker looked it up as a variable, found nothing, and printed
it whole in the line saying the variable was not set
(`PR409-PASTED-GITHUB-TOKEN-LEAK`). The order of the two checks decides only
which refusal is given to a value that fails both, such as one starting
`sk-`. Neither refusal repeats the value.

## `impl UsageSource` › `pub fn gaps(&self) -> Vec<String> {`

What a valid source still lacks for a refresh: no credential variable, or no
account where the provider needs one. Warnings and not refusals, because the
tracker's inputs are not a reason to refuse a run, and the tracker says the
same thing itself when it skips the pool.

## `fn looks_like_a_credential(text: &str) -> bool {`

Whether a value written for `usage_token_env` has the shape of a credential
and not of a variable's name: it starts with one of `CREDENTIAL_PREFIXES`, or
it is longer than `MAX_TOKEN_ENV_CHARS` (64) characters.

The prefixes are GitHub's `github_pat_`, `ghp_`, `gho_`, `ghu_`, `ghs_` and
`ghr_`, the `sk-` of OpenAI's keys and the `sk-ant-` of Anthropic's, and
Slack's `xoxb-`. They are matched exactly, case included: providers issue them
in lower case, and an upper-case name such as `GHP_TOKEN` is an ordinary
variable. `sk-ant-` is already covered by `sk-` and is listed so the list
names every shape the finding names. The three with a `-` are never names by
their characters anyway; they are listed so that this rule says what a
credential looks like by itself, whatever the character check beside it
allows. The length is the catch-all for a token whose prefix is not listed:
a variable's name longer than 64 characters is rare, and many credentials are
longer (a fine-grained GitHub token is 93). The rule leans towards refusing.
A wrong refusal costs the operator a shorter variable name, and a wrong pass
prints a credential.

It is private, since nothing outside this module needs it.
`src/tracker.rs` guards by length alone, through `MAX_TOKEN_ENV_CHARS`.

## `pub fn names_an_account(text: &str) -> bool {`

Letters, digits, `.`, `_` and `-`, not starting with `.` or `-`. The account
is placed in the request's address, so the check is what stands between a
pools file and a path the operator did not mean. `src/tracker/provider.rs`
checks it again where the address is built: constructing a `UsageSource`
confers no authority (CODING_STANDARDS.md §14).

## `pub const DEFAULT_SAFETY_MARGIN: f64 = 0.15;`

§13's default margins, applied when the pools file is silent. Both are also
what `upstroke connect` writes, so a hand-edited file and a generated one mean
the same thing.

## `pub struct Pool {`

One `[pools.<name>]` entry (§17).

## `pub struct Pool` › `pub agent: String,`

Which agent drains it. A pool naming an agent this build has no adapter
for is kept and marked unusable rather than rejected — §17's own example
ships `[pools.local] agent = "aider"`.

## `pub struct Pool` › `pub window: Option<Duration>,`

Rolling window, e.g. `"5h"`.

## `pub struct Pool` › `pub profile: Option<String>,`

§13's credential-profile seam: an operator-writable config-directory
path identifying *which account* this pool draws from. Parsed, carried
and displayed, and acted on for Claude Code on the host runner of a
schema-4 run, which sets it as `CLAUDE_CONFIG_DIR` on the work, review
and probe processes of the agent whose bound pool this is (see the
module docs).

## `pub struct Pool` › `pub usage: Option<UsageSource>,`

Which provider usage endpoint the capacity tracker asks for this pool, and
with what: `usage_provider`, `usage_token_env` and `usage_account` (§17).
`None` for every pool `connect` writes; only an operator opts a pool in.

## `pub struct Pool` › `pub usable: bool,`

No adapter in this build for [`Pool::agent`]. The pool is still listed —
it describes the operator's subscriptions, not this binary's features.

## `impl Pool` › `pub fn discovered(name: &str, kind: PoolKind, agent: &str, sources: Vec<Source>) -> Self {`

A pool as `connect` writes one: §13's defaults, nothing invented.

## `impl Pool` › `pub fn describe(&self) -> String {`

One line for a preview or a listing: what this pool is and whose it is.

## `pub fn pool_for<'a>(agent: &str, pools: &'a [Pool]) -> Option<&'a Pool> {`

An agent's first pool: the first matching entry, table
order as preference — the same convention `different_family_at` uses, so
moving a pool up the file promotes it.

**Attribution and profile, never routing.** Nothing routes on the answer.
It names the pool a review drains, so the ledger can say which subscription
paid, and it selects the profile an agent's review and probe processes run
under (`src/engine/topology/profiled.rs`). The legacy coordinator also takes
it as every attempt's pool. It does not decide a schema-4 worker's pool:
`src/engine/topology/run.rs` chooses that (`serving_pool`) among every pool
naming the agent, and the worker's attempt plan carries the chosen pool's
`profile` (`src/engine/assembly.rs`). While no signal holds any of the agent's
pools exhausted, that choice is this answer.

## `pub struct Spend {`

---------------------------------------------------------------------------
Observations
---------------------------------------------------------------------------

## `pub struct Spend {`

What one pool has drained *through this engine*, in the run being folded.

## `pub struct Spend` › `pub usd: Option<f64>,`

Reported api-equivalent dollars. `None` means nothing reported any —
which is not the same as nothing costing anything (§13: the Copilot
route reports no spend at all).

## `pub struct Spend` › `pub unpriced: u32,`

Attempts whose route reported no cost, so `usd` above is a floor.

## `pub struct Observations {`

Everything the estimator is allowed to look at.

Deliberately plain data: [`estimate`] cannot reach past this into a file, a
process, or a clock, which is what makes every §13 rule testable without a
CLI installed.

## `pub struct Observations` › `pub exhausted: BTreeMap<String, Option<String>>,`

§13 source 1 — pools a rate-limit signal marked exhausted, with the
reset time where the signal carried one. Ground truth.

## `pub struct Observations` › `pub self_spend: BTreeMap<String, Spend>,`

§13 source 2 — self-metering of what this engine spawned.

## `pub fn observe(events: &[Event]) -> Observations {`

Fold a legacy run's events (schemas 1–3) into observations. A schema-4 log
is folded by `observe_topology`.

A pure function over events someone else read, so the estimator's inputs are
derived by exactly the mechanism every other reader uses (§15: the log is
the source of truth). Attempts that name no pool contribute nothing — an
unattributed cost belongs to no subscription, and guessing which would be
worse than leaving the pool unmeasured.

## `pub fn observe(events: &[Event]) -> Observations {` › `evidence::fold(&evidence::of_legacy(events), 0)`

The fold itself is `src/capacity/evidence.rs`'s since 2026-11-23, so that the
rules below are written once and apply alike to one run's log and to every
run on the machine. `of_legacy` reduces the log to its `pool_exhausted`
signals and its settled attempts; `fold` applies them in order; the `0` is
"count every draw", since one run's fold has no period to cut at.

A signal sets its pool's reset to its own: the latest signal wins. The
engine writes a second `pool_exhausted` for a pool already exhausted only
when a later refusal names a different reset — the five-hour limit
clearing into the weekly one — and that later time is the one the
estimate must report, not the five-hour reset already past. A rate-limited
settlement, unlike a signal, never replaces a known reset (see
[The fold's rules](#pub-mod-evidence) below).

## `pub fn observe_topology(events: &[TopologyEvent]) -> Observations {`

Fold a schema-4 run's events into observations: `observe`'s sibling for a
`TopologyEvent` log. Like `observe` it is one call into the evidence fold,
`evidence::fold(&evidence::of_topology(events), 0)`, so the rules below are
`evidence::of_topology`'s. They are applied to one run's log here, and to
every run in scope by `capacity::machine::gather`, which is how `upstroke
capacity` and `validate` reach them.

`exhausted` is exactly `exhausted_pools_of(events)`, the map a resumed
`TopologyRun` starts from, so the report and the run cannot disagree about
which pools a log holds exhausted.

`self_spend` counts every settled attempt once. A schema-4 attempt settles
exactly once, in one of three ways. A failed one settles as
`attempt_finished`, and the fold refuses an `attempt_finished` whose record
says success. An accepted one settles as `candidate_prepared` and writes no
`attempt_finished`. Both carry the attempt's record, and its draws are
counted through `draws_of`, the worker's pool and each review's, as `observe`
does.

The third settlement is `attempt_interrupted`, which a resume writes for an
attempt the process ended on. It carries no record in a schema-4 log, so
its pool is read from the `attempt_started` of the same
`(key, generation, attempt)`, as `RunReport::from_topology` reads it. That
pool gets one attempt of unknown cost: `attempts` and `unpriced` each rise
by one and `usd` does not move, so the pool's dollars read as a floor (the
`?` in `PoolEstimate::describe`). An interrupted attempt has no review
passes to count. The three arms name disjoint attempts, so reading all
three counts each attempt once.

When the log records no `attempt_started` for the interrupted attempt, or
its start names no pool, nothing can be attributed to a pool and the fold
adds nothing. That is `draws_of`'s rule for a record naming no pool.

An interrupted attempt used to add nothing here at all. A pool it had drawn
on then showed its dollars as complete, while the same run's report
(`RunReport::from_topology`) showed that pool's drain as a floor. The
legacy `attempt_interrupted` carries a record, and `observe` counts it.

`self_spend` also counts every integration review pass once. A schema-4
log records a verification's passes in one of four carriers: a verified
`merge_prepared` (`verification.reviews`), a `CodeRejected`
`merge_rejected` (`disposition.verification.reviews`),
`merge_verification_unavailable` (`reviews`) and
`merge_verification_charged` (`reviews`). `evidence::of_topology` reads them
off those four events, the same four `IntegrationReviews::replay` reads for
the pool rows of `RunReport::from_topology`, each dated by its own event, and
the fold adds each pass to the pool it names through `Spend::add`: its amount
summed when known, counted unpriced when not. Reading all four carriers counts each pass once, because
the fold admits one record of a sequence's passes, so no two carriers hold
the same pass.

A pass naming no pool adds nothing, as for an attempt's own passes. An interrupted
verification (`merge_verification_interrupted`) records no pass and names
no pool, so only the passes its `merge_verification_charged` recorded are
counted, as in the run report's pool rows.

Integration review passes used to add nothing here. `upstroke capacity` and
`validate` then omitted spend the same run's report counts on its pools,
and printed that draw as exact.

It is effect-free in `effects/wrappers.toml`.

## `pub(crate) fn exhausted_pools_of(events: &[TopologyEvent]) -> BTreeMap<String, Option<String>> {`

The same memory as `observe(..).exhausted`, read from a schema-4 log. A
resumed `TopologyRun` starts from it, as the legacy resume starts its
`exhausted_pools` from `observe`. Without it the resumed run forgets every
signal its log holds, and the next refusal naming the same reset writes a
second `pool_exhausted`. `observe` reads `Event`, not `TopologyEvent`, so
this is a sibling rather than a change to it: the two differ in how a log
becomes evidence (`evidence::of_legacy`, `evidence::of_topology`) and share
the fold that applies it. `observe_topology` is the same evidence with the
spend kept, so its `exhausted` is this map.

The rules are `observe`'s. A `pool_exhausted` sets its pool's reset. A
settled attempt's record retires the pools it proves serving and marks a
rate-limited pool with no reset if none is recorded. An
accepted attempt settles as `candidate_prepared`, not `attempt_finished`,
and its record retires the same way, as the live run's `record_promoted`
does. `attempt_interrupted` carries no record and changes nothing.

**The order differs from the legacy log's.** The sequential engine writes
`pool_exhausted` after its attempt's `attempt_finished`. The schema-4 run
writes it before (`settle_failure`), so a log cut between the two still
names the subscription that refused. But in memory, the live run retires
that attempt's served pools first and records the signal after. Applied in
log order, a reviewer refused on the pool its worker had just served would
lose the reset: the worker's success would retire the signal written just
before it. So a `pool_exhausted` is held until the event after it. If that
event is `attempt_finished`, the record retires first and the signal is
recorded after. If it is anything else, or the log ends there, the signal
is recorded at once. That is the stop this ordering leaves: the signal is
durable, the settlement is not, and the resume holds the pool at that
reset. The holding is `evidence::of_topology`'s: it emits a held signal after
the settlement that follows it, so the order it hands the fold is the order
the live run applied, and the fold needs no notion of holding.

It is effect-free in `effects/wrappers.toml`.

## `pub mod evidence;`

**The fold's rules.** They were three private functions of this file until
2026-11-23 — `record_signals`, `retire_signals` and the body of `observe` —
and are `evidence::fold` now. What follows is their record, kept because the
reasons outlive the names. In `fold`'s terms: a `What::Signal` records its
pool at its reset, a later one for the same pool replacing the earlier; a
`What::Work` first retires the pools it proves served, then marks the pool
it proves refused.

Withdraw the exhausted mark from any pool this attempt proves is serving
again.

Without this a rate limit is permanent: `exhausted` was only ever inserted
into, nothing emits a recovery event, and [`Confidence::Signal`] outranks
every other source **by design** — so the one thing that could correct the
record was the one thing forbidden from doing so. A pool that refused an
attempt at 10:00, came back at 10:05 and served the rest of the run still
read `exhausted [signal]` at midnight, on the same line that reported the
successful attempts it had served since.

Events arrive in order, so a later signal re-marks a pool this retired —
which is right: the pool went down again. The retiring half is
`retire_served_pools`, which the live engine calls too; both it and the
fold take the pools from `served_pools`, so the two cannot disagree about
which a record proves.

**The refused pool is marked from the settlement.** `limited_pool` names it.

In a legacy log the attempt settlement itself is the durable source of a
rate-limit signal. The legacy writer, which runs only when a run of
schemas 1–3 resumes, appends `pool_exhausted` after the attempt's
`attempt_finished` (`record_pool_exhausted`). `pool_exhausted` remains
useful detail, but a crash between the two appends must not make replay
forget which subscription refused work. The schema-4 engine appends the
signal first (`settle_failure`), so a crash between the two there leaves
the signal durable and the settlement not, and `exhausted_pools_of` holds
the pool at the signal's reset. In either log a rate-limited settlement
that no signal accompanies still marks its pool here.

**Mark the pool, but keep a reset already known for it**
(`obs.exhausted.entry(pool.clone()).or_insert(None)` in `fold`). The sequential
engine writes one `pool_exhausted` per outage, carrying the reset it read
from the first refusal (and one more only when a later refusal names a
different reset), and every other deferral after it settles as one more
rate-limited attempt with no signal of its own. Overwriting with `None`
here let the first deferral — about a minute later on the default
back-off — erase that reset, so `validate` said the signal carried none
and a resume's `capacity_snapshot` wrote `reset_at: null` beside an open
signal that named one. A rate limit with no earlier signal still marks
the pool, with no reset; a success or an available review still retires
it first, in `retire_served_pools`.

## `pub(crate) fn limited_pool(record: &AttemptRecord) -> Option<&String> {`

The pool a rate-limited attempt names as refusing: the attempt's own pool
for a worker failure, the last review's pool for a reviewer failure. Any
other failure, or none, names no pool, and so does an attempt that ran on
no pool.

`evidence::fold` marks this pool exhausted. The schema-4 run's
`back_off` reads the same pool from a deferred task's last
`attempt_finished` when a reviewer was limited, to find the reset it waits
toward, so the two cannot disagree about which subscription refused. For
a worker's limit it reads every pool of the worker's agent instead
(`TopologyRun::resets_awaited_by`): the task can run again on whichever of
them resets first, and that need not be the one that refused last. When
the earliest of those resets has passed (`TopologyRun::awaited_reset`),
the wait runs only until two minutes past it, and once the clock is past
that there is no reset wait, only the ordinary back-off. Only a worker's
limit on an agent with more than one pool keeps a passed reset; with one
pool, or for a reviewer's limit, it is dropped and only a reset ahead is
waited for, as before the failover. A reset is spent once its pool
refuses again two minutes or more past it (`run::spent_resets`), and is
not waited for.
`limited_pool` is effect-free in `effects/wrappers.toml`.

## `pub(crate) fn retire_served_pools(`

Remove every pool this attempt proves is serving again, reset and all.

What counts as proof is deliberately narrow. A *completed* attempt reached
the model and got an answer, whatever the verdict on its code — a gate
failure says nothing about the subscription. A rate-limited one proves the
opposite. An interrupted one proves nothing at all: the engine died without
ever learning whether a reply was coming.

The engine's coordinator calls this on the record it has just settled, so
its own `exhausted_pools` and this fold retire a pool on the same attempt.
It used to carry a copy of the rule instead, and the copy sat on the failure
branch only. A success forgot nothing, so the coordinator went on
remembering the pool and its reset while this fold had retired it. A later
task refused on the same pool, naming the same reset, then looked
already recorded and wrote no `pool_exhausted`. The fold, which had
retired the pool, re-marked it from that refusal with no reset, and the
estimate said "unknown" where the log had once named the time.
`a_rate_limit_after_a_success_retired_its_pool_writes_pool_exhausted_again`
in `engine::tests` holds the two together.

It is effect-free in `effects/wrappers.toml`, like the rest of this module:
it reads the record it is given and removes entries from the map it is
given, and reaches no primitive and no path.

## `fn served_pools(record: &AttemptRecord) -> Vec<&String> {` › `.filter(|review| review.outcome != ReviewPassOutcome::Unavailable)`

A review pass that reached a verdict proves its own pool served, which on
a cross-vendor second opinion is a different subscription entirely.

## `pub(crate) fn pool_exhausted_to_record(`

Which `pool_exhausted` a settled failure writes, if any. It applies the
rules of the legacy coordinator's `record_pool_exhausted`, with the parts
that touch the world taken out: the clock comes in as `now_unix`, the
pools already recorded come in as a map, and the answer goes back to the
caller to emit. So every rule can be tested on plain values.

- Only a rate-limited failure writes one.
- A worker failure names the attempt's own pool and the agent it was given.
  A reviewer failure names the last review's pool and *that review's*
  agent. A cross-vendor second opinion drains a different subscription
  than the worker, so the reset is read in the reviewer's dialect.
- No pool means no signal. An unattributed refusal belongs to no
  subscription.
- A reset is read only from Claude Code's limit message: the captured
  limit text first, then the reason. Any other agent answers
  `reset_at: None`.
- A pool already recorded answers nothing, unless this refusal names a
  different reset from the one recorded. That is the five-hour limit
  clearing into the weekly one, and `observe` then reports the later
  reset. A refusal that names no reset never replaces a known one.

The legacy coordinator keeps its own copy for now. Its code is left as
it is. The schema-4 run calls this one from `settle_failure`, and appends
the answer before the attempt's `attempt_finished`. The map it is given
outlives the process: a resumed run rebuilds it from its log with
`exhausted_pools_of`.

It is effect-free in `effects/wrappers.toml`. The census gate asks for a
row for every externally reachable function of this module.

## `pub fn drain_of<'a>(`

The same fold, over attempt records rather than events — what the ledger
needs, and what a reader holding folded state rather than raw events has.

Shared with [`observe`] and `observe_topology` rather than written twice:
the ledger's per-pool column and the estimator's self-metered source must
be the same number, or one of them is wrong and nothing says which. All
three sum through `accumulate`.

## `fn draws_of(record: &AttemptRecord) -> impl Iterator<Item = (&String, Option<f64>)> {`

An attempt naming no pool contributes nothing: unattributed cost belongs
to no subscription, and guessing which would be worse than leaving the
pool unmeasured.

## `fn review_draws(reviews: &[ReviewRecord]) -> impl Iterator<Item = (&String, Option<f64>)> {`

A cross-vendor second opinion drains a different subscription than the
implementer it judged (§11.3), so each pass is attributed on its own.

## `pub enum Remaining {`

---------------------------------------------------------------------------
The estimate
---------------------------------------------------------------------------

## `pub enum Remaining {`

How much of a pool is left, after margins.

## `pub enum Remaining` › `Unknown,`

Nothing measured this pool. **Never rendered as "full"** — the whole
point of the variant is that "we do not know" and "there is plenty" are
different answers (§13, invariant 7).

## `pub enum Remaining` › `Exhausted,`

A rate-limit signal said so. Ground truth.

## `pub enum Remaining` › `AtMost(f64),`

An **upper bound** on the fraction still available, after
`safety_margin` and `reserve`, clamped to `0.0..=1.0`.

Deliberately not "the fraction remaining". Self-metering sees only what
this engine spawned — in one run, in this repository this month, or on
this machine this month, by the basis — so `1 − draw/allowance` is what
is left *if nothing else drew on the pool* — and something else almost
always did: other machines, and the operator's own interactive sessions
(§13's source 3, which is parsed and not read). The machine basis closes
the two gaps this note used to list first, earlier runs and other
repositories, and those two only. Every one of those can only reduce what is left, never increase
it, so the figure is sound as a ceiling and false as a measurement.
Rendered with `≤` for exactly that reason.

## `pub enum Remaining` › `Unmetered,`

Hardware-bound rather than quota-bound (§13's local pools).

## `pub enum Confidence {`

Where an estimate came from, ranked. §13's trust order made into a type, so
"a lower-ranked source can never overwrite a higher one" is enforced by
[`Ord`] rather than by every call site remembering it.

## `pub enum Confidence` › `Unknown,`

Nothing measured it.

## `pub enum Confidence` › `Assumed,`

Derived from the pool's declared shape rather than from anything
observed — e.g. a local endpoint that cannot run out.

## `pub enum Confidence` › `Reported,`

The provider's own usage endpoint said so, as the capacity tracker stored
it. Below self-metering, as §13's trust order has it: an endpoint can change
or fail without notice, and what this engine spawned it knows for certain.

## `pub enum Confidence` › `SelfMetered,`

Self-metering of what this engine spawned.

## `pub enum Confidence` › `Signal,`

A rate-limit signal from the CLI itself.

## `pub struct Reported {`

The ceiling a stored provider report still supports for one pool, already
reduced to a fraction: `remaining` is raw, before the pool's margin and
reserve. `src/capacity/machine.rs` builds it (`reading`), since what a report
is worth depends on the clock and on the pool's allowance, and the estimator
is given neither.

It depends on the account as well. A provider that reports per account
([`UsageProvider::needs_account`]) answers for the account in the request's
address, and "usage inside one window only grows" is true of one account.
Until 2026-11-23 a stored report did not record which account it was, and
`reading` checked only its provider: a pool refreshed for `account-a` at 90
of 100 drawn and then switched to `account-b` showed `account-a`'s "≤10%
[provider-reported]" for `account-b`, with no note, until the window reset
(`PR409-USAGE-CACHE-ACCOUNT-IDENTITY`). A report now records the account it
was fetched for: the pool's `usage_account` at the refresh, and none for a
provider that reports per key, whatever the pool sets. One function decides
that, `capacity::usage::account_for`; the tracker writes with it and
`reading` checks with it, so the two cannot disagree. `reading` builds no
`Reported` from a report whose account is not the one the pool names now.
Its note says which account the report came from and which the pool now
names, as the note for a changed `usage_provider` does, and that
`upstroke capacity --refresh` replaces it. A report that records no account,
for a provider that needs one, was stored before accounts were recorded. It
cannot be told apart from another account's, so it bounds nothing either,
with a note saying it predates account recording.

Until 2026-11-24 that last rule had a hole. `reading` took a report's
account to match when it equalled what `account_for` answered for the pool,
so a report that records no account and a pool that names no
`usage_account` matched, `None` to `None`: a GitHub report stored before
accounts were recorded still bound such a pool, although the refresh skips
that pool and so would never replace the report
(`PR409-USAGE-CACHE-MISSING-ACCOUNT`). Where the provider reads an account,
the account must now be known on both sides and equal, and `None` matches
nothing. The note for a pool that names no `usage_account` says the refresh
will not run for it until it names one.

It depends on the credential too. A provider that reports per key
(`anthropic-cost`, `openai-cost`) answers for the organization its key
belongs to, and its report records no account. Until 2026-11-24 nothing
recorded which key it was, so a report fetched with the key in
`ORG_A_ADMIN_KEY`, at 90 of 100 spent, still supplied the ceiling after the
pool's `usage_token_env` was changed to `ORG_B_ADMIN_KEY`, another
organization's, with no note (`PR409-USAGE-CACHE-CREDENTIAL-IDENTITY`). A
report now records the name of the environment variable the tracker read
its credential from, for every provider, and `reading` binds it only while
that name is the pool's `usage_token_env`. As with the account, a report
that records none, or a pool that names none, matches nothing. The name is
what is recorded and compared, so a variable that comes to hold another
organization's key under the same name is not seen. The binding is to the
configuration, not to the credential, which a report never holds.

One function decides whether a report is about the source the pool names
now: `other_source` in `src/capacity/machine.rs`. It checks the provider,
then the account where the provider reads one, then the variable, and the
first that differs gives the note. It compares with
`capacity::usage::account_for` and `capacity::usage::token_env_for`, the
helpers the tracker records the two fields with, and the tracker also reads
the credential from the variable `token_env_for` names, so the writer and
the reader cannot disagree. Every refusal is a note in the style of the
provider's: what the report came from and what the pool names now, or that
the report predates recording it, and that `upstroke capacity --refresh`
replaces it. Where the pool names no `usage_account` its provider needs, or
no `usage_token_env`, the note says instead that the refresh will not run
for the pool until it names one. A note names variables and accounts, never
a credential.

Both fields are optional, so a report stored before either still decodes.
The tracker leaves the account out of a per-key provider's report and
writes the variable in every report. A report stored before the variable
was recorded therefore bounds nothing, with its note, until a refresh
replaces it. A build from before a field refuses a report that has it, as
an unknown field, and that report then bounds nothing there. Every report
now carries `token_env`, so a build from before 2026-11-24 can read none of
the reports a later build writes. The report stores names, never a
credential. It refuses an account that is not an account name
(`names_an_account`), and a variable that is not a variable's name or is
longer than `MAX_TOKEN_ENV_CHARS`, on the way in and on the way out. That
bound is the one within which `src/tracker.rs` prints a name whole, so a
note never prints whole a recorded name the tracker would have cut short.

## `pub enum Basis {`

What the observations were folded from, which decides how the draw is worded:
"this run drew", "this repository's runs drew, this month,", "upstroke on this
machine drew, this month,". The sentence used to be fixed at "this run", which
is still true of the snapshot a run writes about itself and false of the
capacity views since they became machine-level. The self-metered note takes
its list of what went unseen from the same place, so a machine-level ceiling
no longer says other repositories are missing from it.

## `pub struct PoolEstimate {`

One pool's estimated state.

## `pub struct PoolEstimate` › `pub reset_at: Option<String>,`

When the signal said the window reopens, where it said so.

## `pub struct PoolEstimate` › `pub self_spend: Option<Spend>,`

What this engine has drawn from the pool in the run being folded.

## `pub struct PoolEstimate` › `pub notes: Vec<String>,`

Everything the estimate could not account for, said out loud.

## `impl PoolEstimate` › `pub fn describe(&self) -> String {`

One line: the pool, what is left, and how confident that is.

## `pub fn estimate_gathered(pools: &[Pool], gathered: &Gathered) -> Vec<PoolEstimate> {`

[`estimate`] over what `machine::gather` read: the same rules, with the
provider reports and the basis the gatherer established. `estimate` is the
one-run form, with no report and the `Run` basis, and is what a run's own
`capacity_snapshot` still uses.

The notes the gatherer attached to a pool's report are cloned into the
estimate (`notes.extend(report_notes.iter().cloned())`): an estimate owns its
notes, as it owns every other line of itself, and the gatherer's copy is
borrowed for every pool in turn.

## `pub fn estimate(pools: &[Pool], obs: &Observations) -> Vec<PoolEstimate> {`

§13's estimator: pools plus observations in, one estimate per pool out.

Pure and total. Every branch is reachable from plain values, so the three
properties in the module docs are testable without a CLI, a repo, or a file.

## `fn estimate_one` › `let mut remaining = Remaining::Unknown;`

Ranked candidates, strongest first. `take` refuses anything that does not
outrank what is already held, so the trust order cannot be inverted by
adding a rule below an existing one.

## `fn estimate_one` › `if let Some(reset) = obs.exhausted.get(&pool.name) {`

(1) Signals — ground truth, and the only thing that can say "exhausted".

## `fn estimate_one` › `if pool.kind == PoolKind::Unmetered {`

A pool that cannot run out is a fact about its shape, not a measurement.

## `fn estimate_one` › `if let (Some(spend), Allowance::Units(allowance)) = (&self_spend, pool.monthly_allowance) {`

(2) Self-metering. It bounds what is left only when the allowance's size
is known: `spend / auto` is not a fraction of anything. Otherwise the
draw is reported beside an Unknown remaining rather than dressed up as
one — §13's conservatism is about never overstating what is left, and
"we measured some spend" is not a measurement of the ceiling.

## `fn estimate_one` › `if let Some(report) = reported {`

(4) The provider's report, last and below everything above it. **It only
ever sets or lowers a ceiling.** Against a signal it is a note and the
signal stands; an unmetered pool it says nothing about; a self-metered
ceiling it replaces only when its own is lower, and otherwise the note says
the report allowed more and was not used. That is "never load-bearing" made
into arithmetic: both figures are ceilings, the lower of two true ceilings is
true, and an endpoint that breaks high can cost nothing while one that breaks
low costs only caution. It never yields `Exhausted`, whatever it reports: a
report of everything drawn is `AtMost(0.0)`, and "exhausted" stays the word
of a rate-limit signal alone.

This arm sits outside `take` on purpose. `take` is rank-ordered replacement,
and replacement is exactly what a lower-ranked source must not do here.

## `fn estimate_one` › `let unread: Vec<String> = pool`

The sources that are parsed and not read: `local-logs` always, and
`provider-endpoint` where the pool names no `usage_provider`, which is then
the thing to add. Saying so is the point: a pool that lists `local-logs` and
gets an estimate anyway would read as though interactive usage had been
accounted for.

## `pub fn effective_remaining(raw: f64, pool: &Pool) -> f64 {`

§13's conservatism, in one place: `max(0, raw × (1 − safety_margin) −
reserve)`, clamped into `0.0..=1.0`.

The margin covers what local measurement cannot see (the same subscription
used from another machine); the reserve is headroom deliberately left for the
operator's own interactive work. Applied multiplicatively then additively
because they are different claims: the margin says the measurement may be
wrong, the reserve says some of what is left is not ours to spend.

## `pub fn parse_duration(raw: &str) -> Option<Duration> {`

---------------------------------------------------------------------------
Durations
---------------------------------------------------------------------------

## `pub fn parse_duration(raw: &str) -> Option<Duration> {`

Parse §17's window spellings: `"5h"`, `"30m"`, `"7d"`, `"90s"`.

Dependency-free, and deliberately narrow — a window is one number and one
unit, so anything else is a typo worth naming rather than a format worth
guessing at.

## `pub fn render_duration(duration: Duration) -> String {`

The inverse, in the largest unit that divides exactly — so a window read
from a file and written back out again is the same string.

## `pub fn strategy_preview(mode: &str, estimates: &[PoolEstimate]) -> Vec<String> {`

---------------------------------------------------------------------------
Strategy preview
---------------------------------------------------------------------------

## `pub fn strategy_preview(mode: &str, estimates: &[PoolEstimate]) -> Vec<String> {`

What each §13 strategy *would* do with these estimates — the read-only half
of the capacity engine, and the whole of what v0.1 ships.

Phrased as "would", every line of it, because none of it is wired to the
binder. A preview that reads as a description of what is about to happen
would be a lie by tense.

## `pub struct CapacityOptions {`

---------------------------------------------------------------------------
`upstroke capacity`
---------------------------------------------------------------------------

## `pub struct CapacityOptions {`

`upstroke capacity [--config <path>] [--pools <path>] [--scope machine|repo]`
(§18). `--refresh` is not here: it is the capacity tracker's, a different
module (`src/tracker.rs`), which the CLI runs before it asks for this report.
Nothing in this file starts it.

## `pub struct CapacityOptions` › `pub repo_root: std::path::PathBuf,`

This repository, whose own run directory is always listed: it is where a run
of schemas 1–3, which keeps no owner record under the machine directory, can
be found at all. Outside a git repository there are no such runs, which the
report says rather than erroring on.

## `pub struct CapacityOptions` › `pub scope: Option<Scope>,`

`--scope`. `None` takes `[capacity] scope` from the settings, whose own
default is the machine (§17), so the flag outranks the file.

## `pub struct CapacityOptions` › `pub now_unix: Option<u64>,`

The moment the report is taken at, which decides which calendar month the
draw counts and whether a stored provider report's window has reset. `None`
reads the clock; a test gives the moment, so that neither is a function of
when the suite ran.

## `pub struct CapacityReport` › `pub agents: Vec<AgentStatus>,`

Live probe + discovery per agent named by a pool. **This** is where
probing belongs: `capacity` is allowed to spawn the vendors' CLIs, and
`validate` is not (§18).

## `pub struct CapacityReport` › `pub gathered: Gathered,`

What the estimates were folded from: the observations, the provider reports
still in force, how many runs in how many repositories, and how many runs were
found and could not be read. It replaced `run_id` and `run_folded`: the one
run the figures used to come from, and whether that run's log had been read.
What `run_folded` existed to tell apart is kept, over many runs:
`Gathered::unread` counts the runs that were found and not read, and `render`
says "no run" only when none was found.

## `pub fn report(`

Collect everything `upstroke capacity` reports: pools from config, the signals
and the self-metered draw of every run in scope, legacy or schema 4, the
provider reports the tracker stored, and a live probe per agent. It reads
files and starts each agent's probe; it writes nothing and asks no provider.

## `let gathered = machine::gather(`

Until 2026-11-23 this was `latest_run_observations`: the latest run of this
repository, read by whichever reader its schema needs. That function was
itself a repair. `report` and `validate` had each carried a copy of a
legacy-only read (`rundir::latest_run`, then `events::read_all`), a schema-4
log made that read fail (`line 2 is not a valid event`), and so on a
repository whose latest run was schema 4 both showed no exhausted pool and no
spend, and both warned of a log that was in fact intact.

What it left was the unit. A pool is drained by every repository on the
machine, and a limit met in one is the state of the pool in all; and an
allowance is a month's, not a run's. `machine::gather` keeps that reader's
rules and applies them to every run in scope:

- It asks `engine::topology::observe::observe` first, in `evidence_in`.
  `Some(observed)` is a schema-4 log, already checked by a full replay, and
  `evidence::of_topology` reduces `observed.accounting_events()`, the events
  `status`'s ledger folds.
- `None` is a legacy log. It is read by `events::read_all` and reduced by
  `evidence::of_legacy`. `read_all`'s torn-tail notice is not passed on: the
  committed lines are all the evidence there is, the schema-4 reader has
  never reported its own torn tail here, and `status` is where an incomplete
  final line is said.

**A schema-4 run's in-flight attempts count as `status` counts them**, in
every run the gatherer reads, whether its scope is the machine or this
repository. If nothing holds a run and it has not finished, its coordinator
died before the attempts it had started settled. `accounting_events` follows
that run's committed log with the interrupted settlements its resume will
append, so each such attempt counts once on the pool its `attempt_started`
named, with its cost unknown. That is how `status` counts it. A run another
process holds, in this repository or another, is folded as committed: its
live coordinator still owns those attempts, and they count when they settle.
A settlement added here is dated when the log is read, as the one a resume
writes is dated at the resume, so the draw of the month it is read in counts
it, whenever its attempt started. Until the gatherer's read went through
`accounting_events` it folded `observed.events()`, the committed log alone,
so between a coordinator's death and its resume `status` counted the attempt
in flight and `capacity` and `validate` did not. The legacy reader keeps that
gap: it folds the committed log alone, and `status::load`'s
`settle_interrupted` is not applied here.

- An error from either reader folds nothing from that log. Where no other
  log of the run can be read, it counts the run in `Gathered::unread` and
  pushes one warning naming the run and the error; where one can, that log is
  folded and the warning names both (below).

The warning says nothing from that run is shown, neither its signals nor its
spend. Earlier copies said "showing signals only" and "rests on rate-limit
signals alone". Neither was true: the signals are in the same log, and a log
that cannot be read yields none of them.

An owner record under the machine directory that exists and leads to no log
is counted in `Gathered::unread` too, beside its warning: one that cannot be
read, is not an owner record, names another run than the private half it is
in, or names a directory that is not that run's under a repository's
`.upstroke/runs/`. Until 2026-11-23 these were warned of and not counted, so
a machine whose one run had a torn `owner.json` was described as having no
run, under a warning naming that run's record
(`PR409-UNREAD-OWNER-SCOPE-COUNT`). Two private halves are still not counted.
One with no owner record is a run of schemas 1–3 or a creation that never
wrote one: neither can be located from here, and neither is a fault worth a
line on every listing. And one whose run this repository lists is read from
that listing, so its record leaves nothing out, whatever it holds. It is
neither warned of nor counted, since counting it would call a folded run
unread.

**A run is folded once, by its id.** Until 2026-11-24 `gather` folded each
log once by its canonical path. That still makes a log listed from its
repository and again from its owner record one log, but it never made two
logs of one run one run. A repository copied to a second directory with its
run artifacts, `.upstroke/` included, holds a second log of each of its runs,
with the same id and the same events. While the machine's owner record still
names the original, both logs are found from the copy, and from anywhere else
only the original's: a run that spent $20 counted $40 from the copy and $20
from the original, and nothing said so (`PR409-COPIED-RUN-COUNTED-TWICE`).
Before the gatherer, when only the latest run of one repository was read, a
copy could not count twice.

The logs that carry one id, each once by its canonical path, are now one run,
and `log_to_fold` folds one of them. Of those that can be read, that is the
one the run's owner record names, else the one holding the most committed
events, else the first by path. The choice is made from the logs and the
record alone, never from the repository the command was started in or the
order the logs were listed in. Logs that hold the same committed events are
one run seen twice, and nothing is said of them. A log that holds other
events is named in a warning beside the one folded, with the run's id and why
that one was chosen, and so is a log that cannot be read beside one that can.
Neither is counted in `Gathered::unread`, which counts a run once, by its id,
and only when none of its logs can be read; `Gathered::runs` and
`Gathered::repositories` count the logs folded. So the scope line still says
what the estimates rest on: a run whose evidence is shown is never also "1
more run could not be read". The events compared are the committed ones
(`Observed::events`) and not `accounting_events`, whose added settlements are
dated when the log is read. Today a run has at most two logs in scope, the one
this repository lists and the one its owner record names, so where both can
be read the record decides. The other two rules make the choice total, and
`capacity::machine`'s own tests pin them.

The evidence of every run is put in one sequence by `evidence::merge`, by
time, with each run's own order kept. Event times are whole seconds, so two
runs can have evidence of the same second. Until 2026-11-23 that evidence
went in the order the runs were listed, which puts the runs of the repository
`gather` was started in first. A refusal on a pool in one repository's run
and a success on it in another's, in the same second, then cleared the pool
when asked from the first repository and left it exhausted when asked from
the second: one machine, two answers (`PR409-MACHINE-CWD-TIE-ORDER`). Each
run is now handed to `merge` keyed by its id and then its log's path, and at
an equal time the order follows the evidence and the keys, never the listing.
Evidence that marks a pool refused waits while another run's evidence of that
second still to come proves the pool served. So a success in one run does not
clear a refusal another run met in the same second, and the pool reads
exhausted, which is the conservative reading of two facts of one second.
Otherwise the run whose key sorts first goes first. A run's own order is kept
throughout, so a refusal and then a success of one second in one run still
clear the pool. Two runs whose refusals and successes of one second cross,
one refusing `p` before it serves `q` and the other refusing `q` before it
serves `p`, have no order that puts both refusals after both successes, and
there the run whose key sorts first goes first. Until 2026-11-24 one of the
two refusals was then cleared. Where both successes are schema-4 runs', both
refusals now stand, by the rule below; where a success is a legacy log's, the
refusal it follows is still cleared, as before. Either way the answer does not
depend on which repository asked.

**Whose refusal a success retires.** The order alone cannot say whether a
success came after another run's refusal
(`PR409-DELAYED-SETTLEMENT-CLEARS-NEWER-LIMIT`). A schema-4 run proves a pool
served at the event that carries the attempt's record, `attempt_finished` or
`candidate_prepared`, which it writes after the attempt's gates and its
judgement, and `fold` used to retire every refusal of each pool the record
served. So a worker that served `shared` from 11:00:00 to 11:01:00 and settled
at 11:20 cleared another run's refusal of `shared` at 11:05, reset at 16:00,
although nothing served `shared` after 11:05: that run alone held the pool to
16:00, and the two merged gave the witness's pool at most 99% left. Dating the
evidence by its service instead does not work: `merge` raises a run's
timestamps that run backwards to the latest already seen, so the serving run's
own later evidence would raise it back past the refusal.

So `fold` asks whose refusal it is. It holds refusals per pool and per run: a
pool is exhausted while any run's refusal of it stands, and it shows the reset
of the refusal applied last. A signal adds or replaces its run's refusal of
its pool, with its own reset, and a rate-limited settlement does the same,
keeping the reset the pool shows. Each piece of evidence carries its run,
which `merge` records as the run's place in key order. Evidence of one log is
one run, so one log alone folds exactly as it did, and a resume, which replays
`exhausted_pools_of` (§4.4), sees no change. A schema-4 attempt's record also
carries `served_at`: the latest `attempt_started` of that attempt before it,
plus the record's duration in whole seconds, never later than the record's
own event. A success retires its own run's refusal of a pool always, in that
run's order, and another run's only when its `served_at` is known and strictly
later than that refusal's time. A refusal whose time cannot be read yields to
its own run alone, and so does every refusal to a schema-4 success whose
attempt has no dated start. A success of a log of schemas 1–3 has no
`served_at` and keeps the rule every success had before: it retires every
run's refusal of the pool that the merged order puts before it, and `merge`'s
wait is what keeps another run's refusal of its own second standing.
`evidence.rs`'s tests pin each case, the witness ported among them.

Three limits remain, each stated here because the rule cannot see past it. A
reviewer's pool is dated by its worker's end, the earliest its review can
have run, so a review that served after another run's refusal leaves that
refusal standing when its worker ended before it. Inside one run a late
settlement still retires that run's own newer refusal: a worker that served
before a refusal its own run met, and settled after it, clears that refusal,
which is `master`'s rule for one run and is filed `deferred` as
`PR409-LATE-SETTLEMENT-RETIRES-OWN-RUNS-NEWER-REFUSAL`. And a legacy log's
late settlement still retires another run's newer refusal, since nothing in
that log dates its service.

Pools are user-level (§17), so a listing outside a repo is still worth
having, and with the machine scope it now says what other repositories drew.
A repository with no run, on a machine with none, warns of nothing.

## `let runner = crate::runner::host::HostRunner::new();`

`capacity` runs no run, so it has no run's Runner to borrow: it
makes its own host one. That is still the Runner seam rather than a
bare spawn — `invariants_introduced[0]` is "every CLI and gate
process executes through Runner" — but it is deliberately *not*
inside INV-18's ambient job, which is the coordinator's and which
this command is not (`main::the_commands_that_spawn_outside_a_run_
are_named_and_counted`).

## `let missing = crate::catalog::missing_from(&pool.agent, &discovery.models);`

D1's cross-check: where the CLI can actually list its models,
say so when the shipped catalog names one it does not offer.
Load-bearing rather than tidy — a stale frontier slug fails
every cross-vendor second opinion at runtime, on exactly the
paths §11.3 exists to protect.

## `Err(error) => agents.push(AgentStatus {`

A CLI that is not installed is a fact worth reporting, not a
reason to refuse the whole listing: an operator asking about
capacity on a machine missing one vendor still wants the other.

## `let _ = writeln!(out, "scope: {} — {}", self.scope, self.gathered.describe());`

The line under the estimates says what the figures above it rest on, and each
of its forms is true of that (`Gathered::describe`, in
`src/capacity/machine.rs`). Runs that were folded are counted, each once
however many logs carry its id, with the repositories of the logs folded.
Runs that were found and could not be read are counted too, beside the
warnings that carry each error, whether it was the log that could not be
read or an owner record that leads to no log: "1 more
run could not be read and nothing from it is shown", or, with none readable,
"the 1 run found on this machine could not be read". Only a scope with no run
at all says "no run on this machine yet". The unreadable form used to be that
last one, which told an operator with an unreadable log that there was no run
to look at; it then named the one latest run, and now counts every run it
applies to.

## `pub(crate) mod tests {`

`pub(crate)` so that `validate`'s tests can write the same run directories
through `schema_4_run_signalling` and
`legacy_run_with_a_line_that_is_not_json`, and `tracker`'s through
`schema_4_run_limited_then_drawing`. This file forbids the governed
write primitives in its tests too, so every fixture here writes through a
funnel (`rundir::create_public_dir`, `rundir::write_plan`, `EventLog`) or
through `workspace_manager::fixture::write_file` and
`workspace_manager::fixture::create_dir`.

## `fn an_unmeasured_pool_is_unknown_never_full()` › `let estimates = estimate(&[pool("claude-max")], &Observations::default());`

Property 1. The trap this exists to stop: rendering "no observation"
as 100% would make a dry subscription look like the best pool to
route to, which is exactly backwards.

## `fn margins_apply_multiplicatively_then_subtract_the_reserve` › `let pool = pool("claude-max");`

Property 2, on the arithmetic itself: 0.5 × 0.85 − 0.20 = 0.225.

## `fn margins_apply_multiplicatively_then_subtract_the_reserve` › `assert_eq!(effective_remaining(0.1, &pool), 0.0);`

Never negative, never above one, and never NaN-propagating.

## `fn a_self_metered_estimate_is_conservative_end_to_end()` › `let Remaining::AtMost(left) = estimates[0].remaining else {`

Half the allowance spent, and the margins take it down from there —
it must never come out at the raw 50%.

## `fn a_self_metered_estimate_is_conservative_end_to_end()` › `assert!(`

And it is presented as the ceiling it is, not as a measurement: this
counts one run's draw against a *monthly* allowance, so every earlier
run and every interactive session is unseen.

## `fn a_pool_that_serves_again_stops_reading_as_exhausted()` › `use crate::events::{AttemptRecord, Event, EventBody, PoolExhausted};`

A signal is ground truth about the moment it was recorded, not
forever. Without retirement `Confidence::Signal` outranks every
source that could correct it, so one rate limit at 10:00 makes the
pool read as empty at midnight — on the same line that reports the
attempts it served in between.

## `fn a_pool_that_serves_again_stops_reading_as_exhausted()` › `let obs = observe(&[signal.clone(), record(None)]);`

Signal, then an attempt that completed: the pool is serving.

## `fn a_pool_that_serves_again_stops_reading_as_exhausted()` › `let obs = observe(&[signal.clone(), record(Some(FailureKind::GateFailed))]);`

A gate failure also proves the model answered — the verdict on the
code says nothing about the subscription.

## `fn a_pool_that_serves_again_stops_reading_as_exhausted()` › `for still_down in [FailureKind::RateLimited, FailureKind::Interrupted] {`

A second rate limit proves the opposite, and an interrupted attempt
proves nothing at all — the engine died without learning whether a
reply was coming.

## `fn a_pool_that_serves_again_stops_reading_as_exhausted()` › `let obs = observe(&[signal.clone(), record(None), signal]);`

And order is respected: recovery then a fresh outage stays down.

## `fn a_pool_that_serves_again_stops_reading_as_exhausted()` › `let reviewer_limited = Event::now(EventBody::AttemptFinished {`

A reviewer-side limit is attached to the failed review pool, while
the same settlement proves the worker and earlier reviewers served.

## `fn a_later_rate_limit_keeps_the_reset_its_pool_exhausted_recorded()` › `let obs = observe(&[record(1, Some(FailureKind::RateLimited))]);`

A rate limit with no signal before it marks the pool exhausted with no
reset — the settlement alone is enough to remember the outage.

## `fn a_later_rate_limit_keeps_the_reset_its_pool_exhausted_recorded()` › `let obs = observe(&events);`

The sequential engine's shape: the refusal, the one `pool_exhausted`
carrying the reset read from it, then a deferral that is rate limited
again and writes no signal of its own. That second attempt must not
erase the reset, or `validate` and a resume's `capacity_snapshot` report
none for a pool whose open signal names one.

## `fn a_later_rate_limit_keeps_the_reset_its_pool_exhausted_recorded()` › `events.push(record(3, None));`

Keeping the reset does not make the outage sticky: the attempt that
finally succeeds still retires the pool.

## `fn a_later_pool_exhausted_with_a_different_reset_replaces_the_earlier_one() {`

Signal A (the five-hour limit, resetting 05:30), a rate-limited
settlement, then signal B (the weekly limit, resetting 17:00 on Nov 14):
the fold reports B. A settlement keeps a known reset, but a signal is a
newer statement of when the pool comes back, so it replaces one. A
further rate limit after B keeps B, and the log cut before B still
reports A — the replacement is the signal's doing, not the settlement's.

## `fn a_topology_log_ending_in_pool_exhausted_holds_that_pool_at_that_reset() {`

The stop the schema-4 ordering leaves: `pool_exhausted` is durable and its
attempt's `attempt_finished` is not. The log may end there, or a resume
may have written `attempt_interrupted` after it. Either way the pool is
held at the signal's reset, because nothing after it settled a record.

## `fn a_pool_exhausted_is_recorded_after_its_own_attempt_retires_the_pools_it_served() {`

The case the held signal exists for. The worker served on `claude-max`,
and the reviewer, on the same pool, was then refused. The log holds the
signal and then the attempt's `attempt_finished`. In log order, the
worker's success would retire the signal written just before it, and only
the settlement's mark, with no reset, would be left. The live run retired
first and recorded the signal after, so the replay does the same and
keeps the reset.

## `fn schema_4_run_started(`

The `run_started` of the schema-4 runs these tests write, in the shape of
`engine::topology::scaffold`'s: two tasks, `alpha` and `beta`, a host
runner, and one obliged review pass per task. Its registry digest is
derived from the plan and its plan digest from the bytes written beside
the log, because `engine::topology::observe::observe` checks both.

## `pub(crate) struct Schema4Run {`

A schema-4 run directory under `<repo>/.upstroke/runs/<id>/`: the frozen
plan and an `events.jsonl` the topology observer accepts. It is written
the way `scaffold`'s durable run writes its log: each event goes through
`TopologyLine::round_trip`, is planned and applied by a live
`TopologyFold`, and is appended through `EventLog::append_topology`. The
fold is the check. An event the replay would refuse panics in the fixture
with the fold's own refusal, so no test here reads a log that
`upstroke status` could not.

## `fn begin_attempt(&mut self, key: crate::topology::registry::TaskKey, pool: &str) {`

`task_dispatched` and `attempt_started` for a task's first attempt, with
the region and the rung binding the fold itself derives. The settlement
that follows is the existing `finished` (task 0) or `prepared` (task 1), or
`interrupted` for either.

## `fn rewritten_with(self, line: &[u8]) {`

Add a committed line after the log's last one, outside the funnel. It
consumes the run, so the `EventLog` is closed first.

## `pub(crate) fn schema_4_run_signalling(`

A schema-4 run whose one attempt was refused: `task_dispatched`,
`attempt_started`, a `pool_exhausted` for the pool at the reset, then the
rate-limited `attempt_finished`. Nothing after it serves the pool.

## `pub(crate) fn schema_4_run_limited_then_drawing(`

`schema_4_run_signalling`, then an accepted attempt on the other task that
drew a stated amount on a stated pool: a real schema-4 run with a signal in
it and spend in it. `tracker`'s end-to-end tests fold it from another
repository through its owner record. The gatherer reads a schema-4 log with
the checked observer, which replays it, so a log written by hand is refused
and a fixture has to be a run the fold accepts line by line. The accepted
attempt's one obliged review pass names no pool, so it adds no draw.

## `pub(crate) fn legacy_run_with_a_line_that_is_not_json(`

A run directory the census lists, whose log neither reader accepts: a
legacy `run_started`, then a committed line that is not JSON. The first
line has to be a committed `run_started`, or the directory is a husk and
not a run, and `latest_run` never names it.

## `struct NoAdapters;`

`report` probes the CLI of each pool's agent through its adapter. With no
adapter it records "no adapter in this build" and spawns nothing, so the
tests below can call `report` itself and read what it renders.

## `fn the_gatherer_holds_a_schema_4_logs_signalled_pool_at_its_reset() {`

The defect the schema-aware read was written for, when it was
`latest_run_observations`. The repository's run is schema 4 and its log
holds a `pool_exhausted` that nothing later retires. The gatherer answers
that pool at that reset, one run folded and none unread, and no warning, and
`report` renders the pool as `exhausted [signal]`. The legacy-only read
refused the log at its second line (`task_dispatched` is not a legacy
event), so the pool read `unknown` and no run was reported.

## `const FIXTURE_NOW: u64 = 1_794_369_600;`

2026-11-11T04:00:00Z, an hour after the instant `topology` stamps its
events with. The gatherer counts the draw of the calendar month its clock is
in, so a test that read the real clock would lose these fixtures' spend on
the first of the next month. `folded` is the gatherer over one repository
with that clock and no machine directory; `capacity_report` is `report` with
it.

## `fn a_schema_4_logs_spend_counts_every_settled_attempt_once_worker_and_reviews_alike() {`

Two attempts on two tasks. One is refused in review and settles as
`attempt_finished`: $0.50 on the worker's pool, $0.25 on the reviewer's.
The other is accepted and settles as `candidate_prepared`: $1.25 and
$0.75. Each pool reads the sum over exactly two attempts: $1.75 and
$1.00. An accepted attempt has no `attempt_finished`, so a fold reading
only that arm would report $0.50 and $0.25 over one attempt each.

## `fn a_schema_4_logs_interrupted_attempt_counts_once_unpriced_on_the_pool_it_started_on() {`

The third settlement. One attempt is refused and settles as
`attempt_finished`: $1.00 on `claude-max`. A second, on the other task,
starts on the same pool and is settled by `attempt_interrupted`. The pool
reads $1.00 over two attempts, one of them unpriced, and `report` renders
its draw as `$1.0000? over 2 attempt(s)`. A fold that skips
`attempt_interrupted` reports $1.00 over one attempt, with no mark that
anything is missing from it.

## `fn an_unresumed_runs_in_flight_attempt_counts_once_unpriced_on_the_pool_it_started_on() {`

The same run as the test above, stopped one line earlier: the log ends at
the second attempt's `attempt_started`, and nothing holds the run. Its
coordinator died and nobody has resumed it. The gatherer, over the
repository the run is in, folds the one run with no warning, and the pool
reads $1.00 over two attempts, one of them unpriced, as when the attempt had
settled as interrupted. `status` prints the same draw for this run,
`claude-max: 2 attempt(s), $1.0000?`, and `report` renders it as
`upstroke on this machine drew, this month, $1.0000? over 2 attempt(s)`.
The interrupted settlement the gatherer adds is dated when it reads the
log, after the start of `FIXTURE_NOW`'s month, so it is counted whenever the
suite runs. Then the test takes the run's lock, as a live coordinator holds
it, and the pool reads one priced attempt: the attempt in flight is that
coordinator's, not yet settled. A gatherer that folds only the committed log
fails the first assertion on the pool with $1.00 over one attempt, with no
mark that anything is missing.

## `fn an_interrupted_attempt_whose_start_names_no_pool_adds_to_no_pools_spend() {`

The two cases with no pool to charge, folded from plain events with no run
directory. In the first, no `attempt_started` names the interrupted
attempt; the other task's start is in the log, on a pool, and is not taken
for it. In the second, its start names no pool. Each leaves every pool's
`Spend` as the settled attempt alone left it: $1.00 over one attempt.

## `fn a_schema_4_logs_integration_review_passes_count_once_on_the_pools_they_drew_on() {`

The four carriers, folded from plain events; the run directory supplies
only the started fold the report reads. A $1.00 attempt settles on
`claude-max`. On `reviewer-pool`: a verified `merge_prepared` holding a
$2.00 pass and an unpriced one, a `CodeRejected` `merge_rejected` holding
$1.00, a started `merge_verification_unavailable` holding $0.50, a started
`merge_verification_charged` holding $0.25 whose verification is then
interrupted, and an unstarted `merge_verification_unavailable` holding
$0.125, attributed to no task. `reviewer-pool` reads $3.875 over six
passes, one unpriced, and `claude-max` still reads $1.00 over one attempt.
The map equals `RunReport::from_topology`'s `pool_drain`, row for row. A
fold reading only attempt settlements reports no `reviewer-pool` at all.

## `fn the_gatherer_folds_a_legacy_log_as_observe_over_read_all_does() {`

The unchanged half. A legacy log, with a signal and a priced attempt so
that the comparison is not of two empty values, is answered with exactly
`observe(&read_all(..))`, one run in one repository, and no warning.

## `fn a_run_neither_reader_accepts_is_reported_unread_and_shows_nothing_of_it() {`

Both ways a log can fail: a legacy log the legacy reader refuses at line
2, and a schema-4 log the topology observer refuses at line 6. Each
answers empty observations, no run folded and one unread, and one warning
that names the run, carries the reader's error, and says nothing from the
run is shown. It does not say "signals only". `report` then renders "the 1
run found on this machine could not be read" and does not render "no run on
this machine yet".

## `fn a_repository_with_no_run_reports_no_run_and_warns_of_nothing() {`

The other side of the same line: with no run directory at all the gatherer
answers nothing folded and nothing unread, its own line reads "no run in
this repository yet" for the repository scope it was asked for, and "no run
on this machine yet" is what `report`, which folds the machine, renders.

## `fn on_the_machine(`

The gatherer over every run on a machine directory, started in `repo`, with
`FIXTURE_NOW` for its clock and no pools: `folded` with the machine scope.
`owner_record` writes a private half's `owner.json` with the bytes it is
given, and `owner_record_of` writes the one a schema-4 run keeps, naming the
run and its public directory, in the two fields the gatherer reads.

## `fn review409_machine_signal_does_not_depend_on_launch_repository() {`

The witness from `PR409-MACHINE-CWD-TIE-ORDER`'s review, ported. Two
repositories on one machine, each with a schema-4 run that an owner record
locates. The `refusing` run's attempt was refused on `shared`, with a
signalled reset. The `serving` run was refused on `claude-max` and then landed
an attempt on `shared`. The fixture stamps every event with one second, so
that success and the other run's refusal are of the same second. Started in
either repository, the gatherer folds two runs across two repositories and
holds the same pools, `claude-max` and `shared`, each at its reset. Before the
tie rule, started in `refusing`, the success came after the refusal and
cleared `shared`, and started in `serving` it came first and `shared` stayed
exhausted. The refusing run's id sorts first, so an order by run id alone
would give one answer from both repositories with `shared` cleared. That is
why the test asserts which pools are held, and not only that the two answers
agree.

## `fn unreadable_owner_must_be_counted_in_the_scope_line() {`

The witness from `PR409-UNREAD-OWNER-SCOPE-COUNT`'s review, ported. The
machine has one private half, whose `owner.json` is `{`, and the repository
has no run. The gatherer folds nothing, counts one run unread, and warns once,
naming the record as not an owner record. The scope line says the one run
found on this machine could not be read, where it used to say there was no
run on this machine yet.

## `fn a_private_half_with_no_owner_record_is_not_counted() {`

The other side: a private half with no `owner.json`, which a run of schemas
1–3 leaves. Nothing is folded, counted or warned of, and the line says there
is no run on this machine yet.

## `fn an_owner_record_that_leads_nowhere_is_not_counted_for_a_run_read_from_its_repository() {`

The repository lists a schema-4 run, and that run's private half holds a torn
`owner.json`. Started in that repository, the gatherer folds the run from the
listing and holds its pool at its reset, and it neither warns of the record
nor counts it, since nothing is missing. Started in another repository, the
record is the only way to the run, and it is counted.

## `fn copy_run(from: &std::path::Path, to: &std::path::Path, run_id: &str) {`

A repository copied with its run artifacts: every file of a run's public
directory under `from`, written to the same place under `to`. `canonical_log`
is a run's log as the gatherer names it, by its canonical path, which is how
the warnings below name both logs.

## `fn a_copied_run_is_not_counted_as_a_second_machine_draw() {`

The witness from `PR409-COPIED-RUN-COUNTED-TWICE`'s review, ported. A schema-4
run in `original` was refused on `limited` and then drew $20 on `api`. Its
public directory is copied to `copied`, and the machine's owner record names
the original. The pools file beside that record gives `api` an allowance of
100 with no margin and no reserve. Asked from either repository, `report`
warns of nothing and shows `api`'s draw as $20 over one attempt: from the copy
the two logs hold the same events, so the run is folded once and nothing is
said. Before the fix the copy counted $40 over two attempts, so at most 60% of
the pool was left where the original showed 80%. The witness restamped the
log's events with the time it ran and read the wall clock; the port reads at
`FIXTURE_NOW`, in the month the fixture's events are dated, so it does not
depend on the day it runs.

## `fn copies_of_a_run_that_differ_fold_the_one_its_owner_record_names_and_warn() {`

Two logs of one run that differ. The original's, which the owner record names,
holds the refusal on `limited`. The copy's holds that refusal and then a $20
draw on `api`, so it holds more events. From the original only its own log is
found; from the copy both are, and the one the owner record names is folded,
although the other is longer, with one warning naming the run, both logs and
why. The two answers are one `Gathered`: one run across one repository, `limited`
held at its reset, and no draw on `api`.

## `fn a_copy_that_cannot_be_read_beside_one_that_is_folded_is_named_and_not_counted() {`

A copy whose log the reader refuses at its sixth line, beside the original the
owner record names. The run is folded from the original, and the copy is not
counted unread: one warning names both logs and the refusal, and the scope
line says one run was folded. Counting it would add "1 more run could not be
read and nothing from it is shown", which is false of a run whose evidence is
shown.

## `fn self_metering_cannot_talk_an_exhausted_pool_back_up()` › `let mut p = pool("claude-max");`

Property 3, in the form that matters: a signal said the pool is
empty, and a self-metered figure computed from a generous allowance
must not overwrite it. Getting this backwards would route work at a
pool the CLI has already refused.

## `fn self_metering_cannot_talk_an_exhausted_pool_back_up()` › `assert_eq!(`

The draw is still reported — the signal says the pool is empty, not
that this run drew nothing.

## `fn an_unknown_allowance_reports_the_draw_without_inventing_a_ceiling` › `let mut obs = Observations::default();`

`spend / auto` is not a fraction of anything.

## `fn every_strategy_preview_says_it_changes_nothing()` › `for mode in ["conserve", "value-max", "deadline", "something-else"] {`

The read-only promise is the one line that must survive every mode:
a preview that reads as a description of what is about to happen
would be a lie by tense (§13's sequencing).
