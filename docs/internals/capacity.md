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
for a legacy log, `observe_topology` for a schema-4 one, and
`latest_run_observations`, which picks between them by the log's schema.

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

The estimator is a pure function over plain values ([`estimate`]), so every
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
3. **Trust order is ranked and fixed.** `Signal > SelfMetered > Assumed >
   Unknown` ([`Confidence`]), and a lower-ranked source can never overwrite
   a higher one — a rate-limit signal is ground truth, and a self-metered
   guess must not talk it back up.

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
v0.1, never read.

## `pub enum Source` › `LocalLogs,`

(3) ccusage-style parsing of the agent's own logs, which is what sees
the operator's interactive sessions. Parsed in v0.1, never read.

## `pub enum Source` › `SelfMetered,`

(2) Self-metering of everything this engine spawned.

## `pub enum Source` › `Signals,`

(1) Rate-limit signals from the CLIs — ground truth.

## `impl Source` › `pub fn read_in_v0_1(self) -> bool {`

Whether v0.1 actually reads this source. The two that are only parsed
get a note on the estimate rather than a pretend number.

## `pub enum Allowance {`

`monthly_allowance = "auto"` or a number of units.

`Auto` is honest rather than convenient: it means the size of the allowance
is not known to upstroke, which is different from it being zero and different
again from it being unlimited.

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

## `pub fn observe(events: &[Event]) -> Observations {` › `EventBody::PoolExhausted { data, .. } => {`

A signal sets its pool's reset to its own: the latest signal wins. The
engine writes a second `pool_exhausted` for a pool already exhausted only
when a later refusal names a different reset — the five-hour limit
clearing into the weekly one — and that later time is the one the
estimate must report, not the five-hour reset already past. A rate-limited
settlement, unlike a signal, never replaces a known reset (see
`retire_signals`).

## `pub fn observe_topology(events: &[TopologyEvent]) -> Observations {`

Fold a schema-4 run's events into observations: `observe`'s sibling for a
`TopologyEvent` log. `upstroke capacity` and `validate` reach it through
`latest_run_observations`.

`exhausted` is exactly `exhausted_pools_of(events)`, the map a resumed
`TopologyRun` starts from, so the report and the run cannot disagree about
which pools a log holds exhausted.

`self_spend` counts every settled attempt once. A schema-4 attempt settles
exactly once, in one of three ways. A failed one settles as
`attempt_finished`, and the fold refuses an `attempt_finished` whose record
says success. An accepted one settles as `candidate_prepared` and writes no
`attempt_finished`. Both carry the attempt's record, and it is summed
through `accumulate`, the worker's pool and each review's, as `observe`
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
adds nothing. That is `accumulate`'s rule for a record naming no pool.

An interrupted attempt used to add nothing here at all. A pool it had drawn
on then showed its dollars as complete, while the same run's report
(`RunReport::from_topology`) showed that pool's drain as a floor. The
legacy `attempt_interrupted` carries a record, and `observe` counts it.

`self_spend` also counts every integration review pass once. A schema-4
log records a verification's passes in one of four carriers: a verified
`merge_prepared` (`verification.reviews`), a `CodeRejected`
`merge_rejected` (`disposition.verification.reviews`),
`merge_verification_unavailable` (`reviews`) and
`merge_verification_charged` (`reviews`). After the attempt arms, the fold
reads them through `IntegrationReviews::replay(events).all()`, the replay
`RunReport::from_topology` adds to its pool rows, and adds each pass to the
pool it names through `Spend::add`: its amount summed when known, counted
unpriced when not. Reading all four carriers counts each pass once, because
the fold admits one record of a sequence's passes, so no two carriers hold
the same pass.

A pass naming no pool adds nothing, as in `accumulate`. An interrupted
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
this is a sibling rather than a change to it. `observe_topology` takes its
`exhausted` from here unchanged.

The rules are `observe`'s. A `pool_exhausted` sets its pool's reset. A
settled attempt's record retires the pools it proves serving and marks a
rate-limited pool with no reset if none is recorded (`retire_signals`). An
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
reset.

It is effect-free in `effects/wrappers.toml`.

## `fn record_signals<'a>(`

Record each held `pool_exhausted`: its pool, at its reset. A later one for
the same pool replaces the earlier, as in `observe`.

## `fn retire_signals`

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
`retire_served_pools`, which the live engine calls too.

## `fn retire_signals` › `if let Some(pool) = limited_pool(record) {`

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

## `fn retire_signals` › `exhausted.entry(pool.clone()).or_insert(None);`

Mark the pool, but keep a reset already known for it. The sequential
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

`retire_signals` marks this pool exhausted. The schema-4 run's
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

## `fn retire_served_pools` › `for review in &record.reviews {`

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

## `fn accumulate` › `if let Some(pool) = &record.pool {`

An attempt naming no pool contributes nothing: unattributed cost belongs
to no subscription, and guessing which would be worse than leaving the
pool unmeasured.

## `fn accumulate` › `for review in &record.reviews {`

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
this engine spawned in this repository, so `1 − draw/allowance` is what
is left *if nothing else drew on the pool* — and something else almost
always did: earlier runs, other repositories, and the operator's own
interactive sessions (§13's source 3, which v0.1 parses and does not
read). Every one of those can only reduce what is left, never increase
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

## `pub enum Confidence` › `SelfMetered,`

Self-metering of what this engine spawned.

## `pub enum Confidence` › `Signal,`

A rate-limit signal from the CLI itself.

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

## `fn estimate_one` › `let unread: Vec<String> = pool`

The two sources v0.1 parses but does not read. Saying so is the point: a
pool that lists `local-logs` and gets an estimate anyway would read as
though interactive usage had been accounted for.

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

`upstroke capacity [--config <path>] [--pools <path>]` (§18).

## `pub struct CapacityOptions` › `pub repo_root: std::path::PathBuf,`

Where to look for runs to self-meter from. Outside a git repository
there simply are none, which the report says rather than erroring on.

## `pub struct CapacityReport` › `pub agents: Vec<AgentStatus>,`

Live probe + discovery per agent named by a pool. **This** is where
probing belongs: `capacity` is allowed to spawn the vendors' CLIs, and
`validate` is not (§18).

## `pub struct CapacityReport` › `pub run_id: Option<String>,`

The latest run in this repository. `Some` whenever there is one, whether or
not its log could be folded. It used to be `None` for a run whose log could
not be read, and the report then said "no run in this repository yet" about
a repository that had one.

## `pub struct CapacityReport` › `pub run_folded: bool,`

Whether the observations behind `estimates` were folded from `run_id`'s
log. This is the one field added to tell a run that was read from a run
that could not be: `render` says which, by id, and says "no run" only when
`run_id` is `None`. It is `false` when there is no run.

## `pub struct LatestRun {`

What `latest_run_observations` answers: the observations, the latest run's
id, and whether the observations were folded from that run. `Default` is
the repository with no run: nothing observed, no id, not folded.

## `pub fn latest_run_observations(`

The one way `upstroke capacity` (`report`) and `validate` read "the latest
run", whichever schema wrote it. Each used to carry its own copy of a
legacy-only read. A schema-4 log made that read fail, so on a repository
whose latest run was schema 4 both showed no exhausted pool and no spend,
and both warned of a log that was in fact intact.

- It asks `engine::topology::observe::observe` first. `Some(observed)` is a
  schema-4 log, already checked by a full replay, and `observe_topology`
  folds `observed.accounting_events()`, the events `status`'s ledger
  folds.
- `None` is a legacy log. It is read as before: `events::read_all`, then
  `observe`. `read_all`'s torn-tail notice goes to `warnings`.

**A schema-4 run's in-flight attempts count as `status` counts them.** If
nothing holds the run and it has not finished, its coordinator died before
the attempts it had started settled. `accounting_events` follows the
committed log with the interrupted settlements its resume will append, so
each such attempt counts once on the pool its `attempt_started` named, with
its cost unknown. That is how `status` already counted it. A run another
process holds is folded as committed: its live coordinator still owns those
attempts, and they count when they settle. Until this read went through
`accounting_events` it folded `observed.events()`, the committed log alone.
Between a coordinator's death and its resume, `status` counted the attempt
in flight and `capacity` and `validate` did not. The legacy reader keeps
that gap: it folds the committed log alone, and `status::load`'s
`settle_interrupted` is not applied here.
- An error from either reader answers `Observations::default()`, with the
  run's id and `folded: false`, and pushes one warning naming the run and
  the error.

The warning says nothing from that run is shown, neither its signals nor
its spend. The two copies it replaces said "showing signals only" and
"rests on rate-limit signals alone". Neither was true: the signals are in
the same log, and a log that cannot be read yields none of them.

A repository with no run answers `LatestRun::default()` and warns of
nothing. "The latest run" is `rundir::latest_run`'s answer, with the
limits its own notes state.

It is effect-free in `effects/wrappers.toml`: it reads through
`rundir::latest_run`, the topology observer and `events::read_all`, and
reaches no primitive of its own.

## `pub fn report(`

Collect everything `upstroke capacity` reports: pools from config, the
rate-limit signals and self-metered spend of the latest run in this repo,
legacy or schema 4, and a live probe per agent.

## `let latest = latest_run_observations(&opts.repo_root, &mut warnings);`

Self-metering needs a repository with runs in it. Pools are user-level
(§17), so a listing outside a repo is still worth having — it just cannot
say what has been drawn. Reporting that beats refusing to run. A run that
is there but cannot be read is reported as that, with a warning, and the
listing still renders.

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

## `Some(run_id) if self.run_folded => {`

The line under the estimates has three forms, and each is true of what the
figures above it rest on. A run that was folded is named as the source of
the draw. A run that exists but could not be read is named too: "the
latest run in this repository (id) could not be read, so estimates rest on
configuration alone", beside the warning that carries the error. Only a
repository with no run says "no run in this repository yet". The middle
form used to be the last one, which told an operator with an unreadable
log that there was no run to look at.

## `pub(crate) mod tests {`

`pub(crate)` so that `validate`'s tests can write the same run directories
through `schema_4_run_signalling` and
`legacy_run_with_a_line_that_is_not_json`. This file forbids the governed
write primitives in its tests too, so every fixture here writes through a
funnel (`rundir::create_public_dir`, `rundir::write_plan`, `EventLog`) or
through `workspace_manager::fixture::write_file`.

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

## `pub(crate) fn legacy_run_with_a_line_that_is_not_json(`

A run directory the census lists, whose log neither reader accepts: a
legacy `run_started`, then a committed line that is not JSON. The first
line has to be a committed `run_started`, or the directory is a husk and
not a run, and `latest_run` never names it.

## `struct NoAdapters;`

`report` probes the CLI of each pool's agent through its adapter. With no
adapter it records "no adapter in this build" and spawns nothing, so the
tests below can call `report` itself and read what it renders.

## `fn the_latest_run_reader_holds_a_schema_4_logs_signalled_pool_at_its_reset() {`

The defect this reader was written for. The latest run is schema 4 and its
log holds a `pool_exhausted` that nothing later retires. The reader
answers that pool at that reset, the run's id, and no warning, and
`report` renders the pool as `exhausted [signal]`. The legacy-only read
refused the log at its second line (`task_dispatched` is not a legacy
event), so the pool read `unknown` and the run id was `None`.

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
coordinator died and nobody has resumed it. The pool reads $1.00 over two
attempts, one of them unpriced, as when the attempt had settled as
interrupted. `status` prints the same draw for this run,
`claude-max: 2 attempt(s), $1.0000?`, and `report` renders it as
`$1.0000? over 2 attempt(s)`. Then the test takes the run's lock, as a
live coordinator holds it, and the pool reads one priced attempt: the
attempt in flight is that coordinator's, not yet settled. A reader that
folds only the committed log fails the first assertion with $1.00 over one
attempt, with no mark that anything is missing.

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

## `fn the_latest_run_reader_folds_a_legacy_log_as_observe_over_read_all_does() {`

The unchanged half. A legacy log, with a signal and a priced attempt so
that the comparison is not of two empty values, is answered with exactly
`observe(&read_all(..))`, its run id, and no warning.

## `fn a_latest_run_neither_reader_accepts_is_reported_unread_and_shows_nothing_of_it() {`

Both ways a log can fail: a legacy log the legacy reader refuses at line
2, and a schema-4 log the topology observer refuses at line 6. Each
answers empty observations, the run's id, `folded: false`, and one warning
that names the run, carries the reader's error, and says nothing from the
run is shown. It does not say "signals only". `report` then renders "the
latest run in this repository (id) could not be read" and does not render
"no run in this repository yet".

## `fn a_repository_with_no_run_reports_no_run_and_warns_of_nothing() {`

The other side of the same line: with no run directory at all the reader
answers the default, and "no run in this repository yet" is what `report`
renders.

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
