# `src/capacity_planner.rs`

Extended notes for [`src/capacity_planner.rs`](../../src/capacity_planner.rs).

The code is the authority for what it does; this file is the whole of its prose. It is curated
rather than a source-order dump: orientation, the data boundary, units, the rules each stage
applies, tie behavior, the assumptions that can move a decision, and the fixtures the tests use.
Sections that describe one item are headed by that item's line of code, spelled as in the source.

## Module

The first offline, deterministic policy computation for the future machine-wide capacity handler
(`DESIGN.md` §13, *Banked-grant planning*). Given timed window readings, each account's reset-grant
inventory, the runnable demand of competing runs and the caller's project/feature priorities, it
recommends for each grant whether to **redeem** it now, **defer** to a stated second, or **let it
expire**, and says **unknown** whenever the evidence cannot settle the choice.

It is pure computation. It reads no file, clock, environment, credential or provider, starts no
process, opens no socket, redeems nothing, and `plan` keeps no state between calls: the caller
supplies the time (`Input::now_unix`) and every fact. The same `Input` always gives the same plan,
whatever the order of its readings. Nothing in the crate calls it yet; the handler that would
(§13, *The capacity handler*) is not built, and a recommendation here is advisory, never a
capacity permit or a redemption. Unredeemed grants never count as spendable quota: they enter only
as a possible event in a simulated future.

The second half of the module, `Ledger` (below), is a fixture-driven state machine for one
simulated grant on one account: from the planner's recommendation, through competing claims and a
simulated redemption reply, to the account snapshot read afterwards. It holds its state in memory,
in the value the caller owns, and is driven only by events the caller applies with a clock the
caller supplies. It is the same kind of computation: no I/O, no real redemption, no persistence,
no global lock and no live wiring.

## Data boundary

The facts are those the existing read-only readers report, with the time and identity a reader's
caller knows. This module takes them as values and never invokes a reader.

| Planner input | The read-only fact it carries |
|---|---|
| `Reading::used_percent`, `Reading::resets_at_unix` | `tracker::subscription::Window`'s `utilization` and `resets_at_unix` |
| `Reading::window` | `five_hour` → `WindowId::Session`, `seven_day` → `WindowId::Weekly`, a `ModelWindow` label → `WindowId::Model` (`"unknown model"` → `ModelLabel::Unknown`) |
| `Reading::observed_at_unix` | the `now_unix` the reader was called with |
| `Reading::account`, `Reading::plan` | caller-supplied: the reader reports neither |
| `Account::coverage` | caller-supplied: for Codex, the windows each plan's banked reset covers (`None` applies `effect` unnarrowed) |
| `Grant { resets_left, ends_at_unix, kind, availability }` | `tracker::reset::Grant`'s fields of the same names; `GrantKind` is `reset::Kind` variant for variant, `Availability` is `reset::Availability` |
| `Inventory::observed_at_unix`, `Inventory::grants` | `tracker::reset::Inventory`'s fields of the same names |
| `InventoryEvidence::Unknown` | `ResetReport::Unavailable(_)`, or no read at all |

`reset::Grant`'s `gives_back_weekly`, `paused` and `usable_now`, and the inventory's `eligible`,
are not repeated: the reader has already folded them into `kind` and `availability`.

The planner restates these as its own types rather than naming the tracker's. The tracker speaks
HTTP, and `tracker::tests::only_the_cli_reaches_the_tracker` holds that only `src/lib.rs` and
`src/main.rs` name it; this change adds one declaration to `src/lib.rs` and leaves the tracker as
it is. The adapter from a reader's report to an `Input` belongs to the handler stage that will
call both, and is parked with it.

`Source` labels where a value came from. `Source::Fixture` is synthetic evidence: there is no
Codex reader, so every Codex reading and inventory is a fixture, and Claude values may be too.
`AccountPlan::rests_on_fixture` is `true` when any reading or the inventory behind that account's
plan is a fixture.

## Units

- **Time**: Unix seconds (`u64`) on every input and output. The simulation measures seconds after
  `now_unix` as `f64`; a `Defer` is rounded to a whole second (below).
- **Usage**: percent of a window's allowance used, `0.0` through `100.0` inclusive. `100.0` is
  exhausted.
- **Burn**: percent of the window per hour (`Segment::percent_per_hour`,
  `WindowForecast::percent_per_hour`).
- **Demand**: `Demand::hours` is hours of runnable work at the account's observed burn: one hour of
  demand advances each window it draws on by that window's observed `percent_per_hour`. It must be
  finite and positive. Work draws on the session and weekly windows and on the model window named
  by `Demand::model`; another model's window neither limits it nor advances. Work whose model is
  `ModelLabel::Unknown` draws on every model window, and all work draws on an unlabeled model
  window (`WindowId::Model(ModelLabel::Unknown)`).
- **Priority**: `u32`, larger is more important.
- **Freshness**: `Input::stale_after_secs`, the oldest a window's latest reading or an inventory may
  be. It is the caller's; there is no default.

## `pub fn observed_burn(readings: &[Reading], now_unix: u64) -> Vec<WindowHistory> {`

Burn is computed per account and window, from that pair's readings alone, sorted by
`observed_at_unix`. Readings observed after `now_unix` are dropped and the history's `latest`
becomes `Latest::FromFuture`. Readings of one second are one reading when they agree exactly
(plan, percent, reset); when they disagree the second is `Latest::Contradictory` and breaks the
history.

A reading is comparable only with a plan, a percent in range and a reset after its own observation.
Anything else is `Latest::Incomplete` and breaks the history. A history is split into `Segment`s:

| `Boundary` | Opens a segment when |
|---|---|
| `First` | the first comparable reading |
| `PlanChange` | the plan differs from the segment's, or any reading of the account, in any window, from the segment's last reading through this one names another plan |
| `Refill` | the percent fell, or the reset moved after the segment's reset time had passed |
| `ResetMoved` | the reset moved without the old one passing |
| `Discontinuity` | the reading follows an incomplete or contradictory one |

A segment's burn is its rise from first to last reading over the time between them. Within a
segment the percent never falls, so burn is never negative. One reading, or readings of a single
second, give no rate. The plan is the account's, not the window's: a weekly reading under one plan,
a session reading under another and a weekly reading under the first again put the two weekly
readings in different segments, so no rate is taken across the change.

## `fn forecast(`

Only the newest segment forecasts, and only when every one of these holds; otherwise the window is
`WindowState::Unknown` with the first failing reason:

- the window is not an unlabeled model window (`UnlabeledModel`) and has a reading (`NoReading`);
- its latest reading is usable (`Incomplete`, `Contradictory`, `FromFuture`);
- the account's newest plan is unambiguous (`PlanContradiction`: two plans in one second) and is
  the segment's plan (`PlanChanged`);
- the segment's last reading is no older than `stale_after_secs` (`Stale`);
- its reset has not passed by `now_unix` (`Refilled`: the window refilled since it was read, and
  its new state is unread);
- the segment has a rate (`InsufficientHistory`).

A known forecast extrapolates the last reading to `now_unix` at the observed rate, capped at
`100.0`. `exhaustion_unix` is when that rate reaches `100.0`, rounded up to the second, and `None`
at a zero rate; it may fall after `refill_unix`, and the comparison accounts for that. Every
account reports `Session` and `Weekly`, read or not, and each model window it has a reading for.

## `pub const fn effect(provider: Provider, kind: GrantKind, window: WindowId) -> Effect {`

Reset effects are rules per provider, grant and window (§13). `Restores` sets the window's use to
zero. `RestoresBelowFull` does so only for a window with any use; one already at full remaining is
not restarted. `Unchanged` leaves the window. `Unknown` is not guessed (below).

| Provider | Grant | Session | Weekly | Fable model | Other model |
|---|---|---|---|---|---|
| Claude | `Weekly` | `Unchanged` | `Restores` | `Unknown` | `Unknown` |
| Claude | `Session` | `Restores` | `Unchanged` | `Unknown` | `Unknown` |
| Claude | `Full` | `Restores` | `Restores` | `Unknown` | `Unknown` |
| Claude | `Fable` | `Restores` | `Unknown` | `Restores` | `Unknown` |
| Codex | `Weekly` | `Unchanged` | `RestoresBelowFull` | `Unknown` | `Unknown` |
| Codex | `Session` | `RestoresBelowFull` | `Unchanged` | `Unknown` | `Unknown` |
| Codex | `Full` | `RestoresBelowFull` | `RestoresBelowFull` | `Unknown` | `Unknown` |
| either | `Unknown`, or `Fable` on Codex | `Unknown` | `Unknown` | `Unknown` | `Unknown` |

A Fable grant restores the weekly use spent on Fable. That is the Fable model window, and an
unknown share of the weekly one.

An account with `Account::coverage` narrows the rule by the plan it is on, the same way the ledger
narrows a claim by `Setup::coverage`: a window the account's plan covers keeps the rule, a window it
does not cover is `Unchanged`, and every window is `Unknown` when the plan is not listed. The
simulation takes each window's effect from that narrowed rule, so a Codex `plus` reset, which covers
the weekly allowance only, restores nothing in an exhausted session window.

## `pub const fn schedule_after_restore(provider: Provider, window: WindowId) -> Schedule {`

Claude's weekly reset day and time are fixed, so a restored Claude weekly window keeps its refill
(`Schedule::Kept`). Every other restored window has a new schedule that must be read again
(`MustReRead`). A restarted Codex window's new schedule replaces its old refill, and the fixed
weekly clock does not establish Claude's five-hour scheduling. In the simulation a `MustReRead`
window loses its old refill and gains no new one: if it exhausts again, nothing refills it within
the comparison. That undervalues such a redemption rather than inventing a refill.

That lower bound can show a redemption is worth making, never that it is not. When the comparison
finds no benefit, `decide` runs it once more under the best schedule (`Assumption::BestSchedule`):
each restored window whose schedule must be re-read, including a window of unknown effect, which is
then assumed restored, never limits work again, as if its new schedule refilled it whenever it ran
out. A benefit there makes the grant `Unknown(Schedule)`; only a redemption that gains nothing even
then is `LetExpire(NoUnlock)`.

## `const fn natural_clock(provider: Provider, window: WindowId) -> Clock {`

This is what a window does after the refill its reading reported. Claude's weekly window refills
every seven days from it (`Clock::Fixed`). Claude's session window opens a new five-hour window at
its next use (`Clock::Rolling`); that is a material assumption, listed below. Model windows and all
Codex windows are `Clock::Unestablished`: after the reported refill nothing further is assumed.

## `pub fn rank_demand(runs: &[Run], demand: &[Demand], priorities: &Priorities) -> Vec<Ranked> {`

Each demand item takes its run's priority. A `Purpose::Feature` run takes its feature's priority
from `Priorities::features[project][feature]` where one is given, and otherwise its project's from
`Priorities::projects`. A run with neither is `Rank::Unprioritized`. A `Purpose::ResearchFor` run
takes its parent's rank, transitively, so research on behalf of a feature competes as that feature.

A demand item is `Rank::Invalid` when its hours are not finite and positive (`InvalidHours`), its
run is not listed (`RunMissing`), its run is listed twice (`RunConflict`), a research chain names an
unlisted parent (`ParentMissing`), or a chain returns to itself (`ParentCycle`).

The queue is ordered by priority from high to low, then `Unprioritized`, then `Invalid`; equal ranks
are ordered by run id. That last order is arbitrary but deterministic. Competing runs on one account
share its windows and are served in that order, one after another, at the observed burn.

## `pub fn plan(input: &Input) -> Vec<AccountPlan> {`

One `AccountPlan` per listed account, in input order, carrying its window states, its ranked queue
and its grant assessment. An account listed twice has its grants `Unknown(Conflict)`. The inventory
is `Unknown` when unread (`NotRead`), observed after `now_unix` (`FromFuture`) or older than
`stale_after_secs` (`Stale`). Known and empty is `Grants::KnownEmpty`, which is a different answer
from any unknown. Demand for an account that is not listed is not planned.

## `fn decide(context: &Context<'_>, grant: &Grant) -> Decision {`

Checks are applied in this order, and the first that settles the grant decides it:

1. A grant past its `ends_at_unix` or with `resets_left == Some(0)` is `LetExpire(NotRedeemable)`,
   whatever the forecast.
2. `Availability::NotUsable` or `Availability::Unknown`, or a usable grant without a known positive
   count or end, is `Unknown(Availability)`. Not usable now is not evidence that the grant stays
   unusable until its end: the reader folds states that can change before then, such as
   `usable_now: false`, into `NotUsable`, and the same count may become usable a second later.
3. An account with no runnable demand is `LetExpire(NoUnlock)`, since a redemption can unlock
   nothing.
4. Any of the account's windows unknown is `Unknown(Window(id, reason))`, because any window may be
   the one that limits.
5. Any invalid demand item on the account is `Unknown(Demand(problem))`.
6. Otherwise the comparison below decides.

## `impl Scenario` › `fn simulate(&self, redemption: Option<Redemption>) -> Result<Outcome, TooLong> {`

The simulation starts at `now_unix` from each window's forecast. It runs the queue in order while
no window the current item draws on (see *Demand* under Units) is exhausted, advancing those
windows at their burn, and waits while any of them is exhausted. Waiting ends at such a window's
next refill or at the pending redemption. A window the item does not draw on neither blocks it nor
advances. Refills follow the reported reset and then `natural_clock`. A redemption applies each
window's effect (`effect`, narrowed by `Account::coverage`) and `schedule_after_restore` at its
moment. The simulation ends when the queue completes or when every exhausted window the current
item draws on has nothing left to refill it; the work left over is then incomplete. It never
stops at the grant's expiry. Expiry is the last moment a redemption may happen, not a deadline for
spending what it restored, so work that runs after expiry on restored allowance counts in full.
`MAX_STEPS` bounds the work: every step of the simulation and every refill it passes count against
one budget, so a fixed weekly clock recurring through one long wait is bounded as well. A scenario
that exceeds it is `Unknown(Bound)`.

## `impl Scenario` › `fn choose(&self, assumption: Assumption) -> Result<Option<f64>, TooLong> {`

The baseline is the queue with the grant never redeemed. The candidate redemption times are now,
every refill and exhaustion moment the baseline passes before the grant's end, and the last whole
second before the end. More than `MAX_CANDIDATES` is `Unknown(Bound)`. Each candidate is simulated
and scored, and a candidate is chosen only when it is strictly better than the baseline. Nominal
restored quota is therefore never valued: only useful work done sooner, or completed at all,
counts. The cost of moving a refill schedule is part of every candidate's simulated future.

A score is compared priority level by priority level, from the highest. At each level, fewer
incomplete items wins. With those equal, the smaller sum of completion times wins, with a tolerance
of `TIE_SECS_PER_ITEM` (one second) per item at that level. A lower level is consulted only when
every higher level ties, so useful high-priority work outweighs any amount of lower-priority work.

## Tie behavior

- Within the comparison, differences under one second per item are ties.
- A redemption that ties the baseline is not chosen: the grant is `LetExpire(NoUnlock)`, or
  `Unknown(Schedule)` where the best schedule for a restarted window would still beat the baseline
  (above). A grant is never redeemed for quota that unlocks no work.
- Among candidate times that tie, the **latest** is chosen. A grant stays banked as long as banking
  it costs no useful work, so `Redeem` means that now is strictly better than every later
  candidate. `Defer` means waiting loses nothing, or gains something. A deferral to the grant's last
  second (`ends_at_unix - 1`) is a real answer: any time up to then serves equally well, and the
  handler would plan again before it.
- A chosen time is rounded down to the whole second. Under one second from now is `Redeem`;
  otherwise it is `Defer { until_unix }`.
- An account's grants are decided independently, each as if no other were redeemed. Because a
  redemption requires balances and timestamps to be read again before restored capacity is
  allocated (§13), at most one grant per account is `Redeem`. It is the one with the earliest
  `ends_at_unix`, with the earlier inventory position breaking a tie. Every other grant that was
  `Redeem`, `Defer` or `LetExpire(NoUnlock)` becomes `Unknown(AwaitsReRead)`.

## Unknowns

An unknown is never filled with a guess:

- Unknown inventory (`Grants::Unknown`) is distinct from known-empty inventory
  (`Grants::KnownEmpty`).
- An unknown window, availability or demand leaves the grant `Decision::Unknown` with the specific
  reason; no balance, rate, date or spendable grant is invented. A grant not usable now has unknown
  availability; it is not taken to expire unusable.
- The schedule a restarted window gets after a redemption is unknown. Its lower bound (no refill)
  may choose a redemption; a no-benefit answer stands only when the best schedule gains nothing
  either, and is `Unknown(Schedule)` otherwise.
- Where the grant has an `Effect::Unknown` on any of the account's windows (by rule, or because the
  account's plan is missing from its `Account::coverage`), the comparison runs twice. The cautious bound assumes the unknown windows are not restored; the hopeful bound assumes
  they are fully restored with their schedule kept. The decision stands only when both bounds agree,
  and is `Unknown(Effect)` otherwise.

## Material assumptions

Each of these can change a decision:

1. **Burn continues at the observed rate while work runs.** Each window's rate is the average over
   its newest segment. The rate applies whenever queued work that draws on the window runs and
   stops when the queue waits or empties. A window observed idle (rate `0`) is never forecast to
   bind, however much demand is queued.
2. **Demand is serial at the account's burn.** Competing runs on one account are served one after
   another in rank order. Their hours add; they do not overlap or multiply the rate.
3. **Work draws on its own model's window.** An item stated as one model is limited by the session,
   weekly and that model's window only. An item whose model is `ModelLabel::Unknown` is assumed to
   draw on every model window, so an exhausted model window blocks it.
4. **Claude's session window rolls.** After its reported refill, a Claude session window opens a new
   five-hour window at its next use. Weekly Claude windows refill every seven days from the
   reported reset. Nothing is assumed of model or Codex windows after their reported refill.
5. **A schedule that must be re-read gains no refill** within the comparison (above). That is a
   lower bound on such a redemption's worth. It may choose a redemption; a `LetExpire(NoUnlock)`
   stands only if the best schedule gains nothing either.
6. **One redemption per grant.** A grant with `resets_left > 1` is planned for its next redemption
   only. A later one is planned after the re-read.
7. **Readings are the account's whole state.** The planner does not know what a provider has not
   reported, so a window with no reading is unknown and blocks the decision.

## Fixtures

The module's inline `mod tests` uses explicit time (`NOW = 1_800_000_000`) and
synthetic in-memory evidence only. `burning(window, used_now, per_hour, resets_in)` is two
readings, one hour apart, ending at `NOW`. All other helpers build on it:

- `exhausted_weekly()`: weekly at 100% (2%/h, refills in 72 h) and session at 10% (10%/h, refills in
  4 h). A weekly grant redeemed now unblocks waiting work.
- `filling_weekly(resets_in)`: weekly at 60% burning 2%/h, so it is exhausted 20 h out unless it
  refills first, and an idle session window refilling in 5 h.
- `one_job(hours)`: one feature run of priority 5. Every demand item the helpers build has the model
  `ModelLabel::Unknown`, so it draws on every model window; a test that names a model sets it.
- `input(..)` and `claude(..)`: one `operator` account with a fresh `Source::Reader` inventory, no
  `Account::coverage`, and `stale_after_secs` of one hour.

Worked examples the tests pin:

| Scene | Decision |
|---|---|
| `exhausted_weekly`, weekly grant, 4 h job | `Redeem` |
| `filling_weekly(72 h)`, weekly grant ending in 240 h, 100 h job | `Defer` to `NOW + 20 h` (forecast exhaustion) |
| `filling_weekly(15 h)`, 25 h job | `LetExpire(NoUnlock)`: the refill comes first |
| `filling_weekly(72 h)`, 25 h job; then 10 h job; then grant ending in 10 h; then in 1 s | `Defer` 20 h; `LetExpire`; `Defer` to `NOW + 10 h - 1`; `Redeem` |
| weekly exhausted and session exhausted until 2 h | `Defer` to `NOW + 2 h` |
| weekly 90% at 10%/h, grant ending in 30 min, 5 h job | `Defer` to the last second; the restored allowance is spent for hours after expiry |
| session grant, alpha (p10, 2 h) before beta (p1, 10 h); then the priorities swapped | `Redeem`; then `Defer` to `NOW + 10 h` |
| weekly exhausted, refilling in 1 h, 3 h job: Claude (kept clock) vs a Codex fixture (restart) | `Redeem` vs `Defer` to `NOW + 3 h` |
| several usable grants on one exhausted account | `Redeem` for the earliest-ending one; `Unknown(AwaitsReRead)` for the rest |
| session at 100% (25%/h) until 3 h, session grant ending in 30 min: 6 h job; 3 h job; and a session at 10% (10%/h) with a 2 h job | `Unknown(Schedule)`: with no refill after the restart the job stays incomplete, while a refill the new schedule may bring beats the baseline's 10 h; `Redeem`; `LetExpire(NoUnlock)`, since even the best schedule gains nothing |
| `exhausted_weekly`, weekly grant, 4 h job: inventory read `NotUsable` at `NOW - 1`, then `Usable` at `NOW` | `Unknown(Availability)`, then `Redeem` |
| session at 100% refilling `MAX_STEPS + 1` weeks out, weekly at 30% (72 h) recurring, weekly grant, 1 h job; then refilling 2 weeks out | `Unknown(Bound)`: the weekly refills passed in the one wait exceed `MAX_STEPS`; `LetExpire(NoUnlock)` |

## `pub struct Ledger {`

One simulated grant on one account, and the capacity permits issued to work on that account. The
two are kept apart on purpose:

- The **banked grant** (`GrantState`) is the account's: a voluntary reset that can be redeemed
  once. Claiming it reserves the right to redeem it; it is never capacity.
- A **capacity permit** (`Permit`) is issued to a request for work, and draws percent from named
  windows (`Permit::draws`). It is allocated only from a balance a snapshot reported; a claim, a
  reply or a recommendation never issues one or enlarges what one may draw.

`Setup` names the grant (`GrantRef`: account, kind and use-by second, which stay the same while
its count changes), the provider, the freshness bound and, for Codex, the plan-specific covered
allowances (`Setup::coverage`). The initial `Snapshot` seeds the windows and inventory.

The state is small and plain: the accepted readings (which `recommend` hands to `plan`), the
latest accepted inventory, one `WindowView` per window with the plan its reading named, every plan
the account's readings have named and the second each was read, the `GrantState` (with, when the
latest inventory settled a `Replied` grant, its second and both states), the registered claims, the outstanding permits, the draws of returned permits that no newer reading has yet
covered (with the second each was returned) and a `generation` that every accepted change
advances. Nothing is
derived from the wall clock: `apply(now_unix, event)` moves the ledger's clock forward and refuses
an earlier one (`Refusal::ClockBehind`).

### `impl Ledger` › `pub fn recommend(&self, work: &Work) -> Recommendation {`

Builds an `Input` from the ledger's readings and inventory at its own clock, with
`Setup::coverage` as the account's `Account::coverage`, and calls `plan`, the same planner as
above, unchanged. The forecast and the claim therefore narrow the grant's effect by the same
covered allowances. The `Recommendation` carries the planner's decision for this
grant (`None` when the inventory is unknown or does not list it), the grant it was read for
(`Setup::grant`), the generation and second it was read at, and the earliest refill among the
windows the planner forecast: the first second at which the planner, given the same state, would
read one of them as `Refilled`. It changes nothing: two requests that ask at the same generation
get equal recommendations, and the ledger is untouched. Its fields are private, so code outside
the module cannot forge one.

### `impl Ledger` › `fn claim(&mut self, request: RequestId, grant: &GrantRef, basis: Recommendation) -> Transition {`

The only way to reserve the grant. Checks, in order: the request and its basis both name this
grant (`WrongGrant`), so a recommendation read for another account or another grant authorises
nothing here, even at the same generation and second; the grant is `Banked` (otherwise
`Held { by }`, `Spent`, `Expired` or `Absent`); the basis is of the current generation, no older
than the freshness bound, and the earliest refill it recorded has not been reached (`StaleBasis`:
the clock alone advances no generation, yet once a window it forecast refills, a fresh
recommendation no longer reads that window as it did); the basis says `Redeem`
(`NotRecommended`, carrying what it did say). The first claim to pass is
registered (`claims`), the grant becomes `Reserved`, and the generation advances, so the other
request's basis, read from the same state, can no longer pass even where the grant to be held is
released again. In either order exactly one of two claims from one reading is registered.

At the claim, each window the redemption may touch is marked `awaiting_re_read`: those where
`effect` (for Codex, narrowed by the plan's covered allowances) is `Restores` or `Unknown`, and a
`RestoresBelowFull` window not observed full. A window that `schedule_after_restore` says
`MustReRead` also loses its refill (`Refill::AwaitingReRead`); a `Kept` one, Claude's weekly,
keeps its fixed boundary. A Codex covered allowance observed full is not restarted, so it is not
marked and keeps its schedule; a window the plan does not cover is unrelated and stays as it was.
A marked window keeps its pre-claim balance as its allocatable balance: a reset only lowers use,
so that balance is a lower bound, and nothing restored is credited. A newer reading that shows more
use than the window holds is evidence that the balance fell even where it cannot be the re-read,
because it was observed before the reply or lacks the next refill its window must report (below):
it becomes the window's balance at once, and the window stays marked with its refill unchanged.

### `impl Ledger` › `fn reply(&mut self, claim: ClaimId, reply: Reply) -> Transition {`

The simulated redemption response. A reply to the claim holding the grant moves it from
`Reserved` to `Replied`, recording the reply and its second. `Accepted`, `Refused` and `Unknown`
all leave the grant held and the marked windows awaiting a re-read: no reply is evidence of a
balance. A second reply to the same claim is `Ignored(Duplicate)`; it moves nothing, except that a
later `Accepted` is remembered over a `Refused` or `Unknown` so that an unchanged count can no
longer release the grant. A reply to a claim that no longer holds the grant is `Ignored(Stale)`,
except an `Accepted` one while the grant is `Banked`: the grant was released after that claim, and
the acknowledgment says its redemption went through, so the grant is held again by that claim, as
`Replied` with `Accepted` at the acknowledgment's second, and the generation advances. No other
claim passes until an inventory observed after that second settles it, and after `Accepted` only a
fallen count or an unlisted grant does. The windows are left as they are. A claim never registered
is `Refused(UnknownClaim)`.

### `impl Ledger` › `fn observe(&mut self, snapshot: Snapshot) -> Observation {`

A snapshot is readings plus inventory evidence, each with its own observation second. Each
reading, oldest first, is:

| Verdict | When |
|---|---|
| rejected | another account, observed after the ledger's clock, a percent out of range or a reset not after its observation |
| missing | no percent (the window was not reported), or a re-read after the reply that reports no next refill for a window whose schedule must be re-read (below) and shows no more use than the window holds; the window keeps its last view |
| delayed | older than the window's view, or, for a window awaiting a re-read, not after the window's view, or observed before the reply (or with no reply yet) and showing no more use than the window holds |
| repeated | the same second and values as the window's view |
| conflicting | the same second with different values: the window's balance and clock become unknown (`Refill::Conflicting`) until a later reading |
| adopted | otherwise: the reading becomes the window's view and its mark is cleared, except that a reading of a marked window that is not yet its re-read becomes its balance only, as above, and leaves the mark and the refill as they were |

An adopted reading's refill is the reset it reports. One that reports none keeps a refill the view
already knows when that refill falls after the reading, since the window has not reached it:
Claude's fixed weekly boundary, or the clock of a Codex allowance a redemption did not restart.
Otherwise the refill is `Refill::Unreported`.

Every reading of the account observed by the ledger's clock is evidence of the plan it names,
whatever its verdict, and is recorded with its second (see `usable` below).

The inventory is judged the same way by its second. An adopted inventory settles the grant:
`Banked` and `Absent` follow the count; a `Replied` grant settles only on an inventory observed
after the reply's second, since one of that second may have been read before the claim or the
reply. Then a fallen count, or a grant no longer listed, is `Spent` (once, for the claim that held
it), and an unchanged count after a `Refused` or `Unknown` reply releases it to `Banked`. After
`Accepted`, an unchanged count settles nothing. Two inventories of one second that disagree leave
the inventory unknown, which the planner reports as no decision. Where the first of them settled a
`Replied` grant and nothing has moved the grant since, the conflict undoes that settlement and the
grant is `Replied` again, so the evidence of one second settles nothing in either order of arrival,
and a later inventory decides.

Settling the grant does not settle a window: a restored window stays at its pre-claim balance
until a reading of its own, observed after the reply, is adopted. The two are independent, so a
snapshot that confirms the spend but omits the window credits nothing. A window whose schedule
must be re-read (`Refill::AwaitingReRead`) needs that reading to report its next refill as well,
since §13 has the reported refresh checked rather than one calculated from the redemption: a
restarted Codex allowance read at 0% with no refresh stays at its pre-claim balance. The one
exception is Claude's session window, whose new five-hour window opens only at its next use
(`Clock::Rolling`), so its re-read may report no reset. Repeated snapshots change nothing and do
not advance the generation.

### `impl Ledger` › `pub fn usable(&self, window: WindowId) -> Result<f64, Blocked> {`

The percent a new permit may draw: `100` less the window's observed use, less what outstanding
permits draw from it, less what returned permits drew from it that the window's reading does not
yet show (below), floored at `0`. It is `Blocked` when the window was never read (`NoReading`),
its reports conflict (`Conflicting`), any reading of the account at or after the window's reading
names a plan other than the one the window was read under (`PlanChanged`: its balance is the
previous plan's allowance, even after a change back, until the window is read again), its view is
older than the freshness bound (`Stale`), or its reported reset has passed (`Refilled`: the refill
is real but unread, and is not credited). A window read with no plan is `PlanChanged` once any
plan is reported at or after it. A permit
(`Event::Permit`) is issued only when every window it draws has the balance; the first window in
`WindowId` order that lacks it is named in `Refusal::Blocked`. A draw may exceed the balance by
`PERCENT_EPSILON`, to absorb rounding, but only where some balance is left: an exhausted window
(`usable` of `0`) refuses every positive draw, however small, so the tolerance never issues quota
the reading does not report.

`Event::Release` returns a permit whose work has ended, and a second release of it is
`Refused(UnknownPermit)`. Returning it frees nothing at once: the work spent its draws, and the
reading the balance came from does not show that use. Each draw stays debited from its window
until a reading of that window observed after the release second is adopted, whose use then
stands for the work; `observe` drops the debit then. A reading of the release second itself, or
one that is delayed, missing, repeated or conflicting, leaves it in place. Release cannot tell
finished work from cancelled work, so either keeps its draws until that re-read.

A permit outlives the grant: expiry (below) removes no balance and no permit.

### `impl Ledger` › `fn expire(&mut self) {`

When the clock reaches the grant's use-by second, a `Banked` or `Absent` grant becomes `Expired`,
and a later inventory that still lists it does not revive it. A grant already claimed is left to
its reply and snapshot, since the redemption may have been sent before the end. Window views and
permits are untouched: expiry loses the chance to restore capacity, never existing quota.

### Driving it from handler code

A later handler stage can drive the ledger exactly as the tests do, without any of the parts this
slice leaves out:

1. Build a `Snapshot` from a reader's report (the data boundary above) and `Ledger::new` it with
   the grant's `GrantRef` and the machine's clock.
2. For each request, call `recommend` with the runs, demand and priorities it is arbitrating, and
   pass the `Recommendation` back in `Event::Claim`. Only `Transition::Claimed` authorises the
   holder to redeem.
3. Apply the redemption's response as `Event::Reply`, and each later read as `Event::Observe`.
4. Issue work only through `Event::Permit`, return the permit with `Event::Release` when the work
   ends, and read `usable`, `window` and `grant_state` for display.

Everything the ledger refuses comes back as a value (`Transition::Refused`, `Ignored`), never a
panic, so a handler can log it. What it would add around this is outside the slice: persisting
the ledger between processes, one machine-wide lock around `apply`, the real redemption call,
timeouts for a holder that never replies, and run-specific policy.

### Fixtures for the ledger

All are synthetic (`Source::Fixture` or the planner fixtures above) and timed from `NOW`, with
`MIN` = 60 s, a freshness bound of one hour and requests `ALPHA` and `BETA`.

| Fixture | Windows |
|---|---|
| `claude_session_exhausted()` | session 100% (25%/h, refills in 2 h); weekly 30% (1%/h, 72 h) |
| `exhausted_weekly()` (above) | weekly 100% (2%/h, 72 h); session 10% (10%/h, 4 h) |
| `claude_fable_exhausted()` | Fable model 100% (10%/h, 50 h); session 20% (3 h); weekly 40% (72 h) |
| `crossed(with_fable)` | weekly 100% (72 h) and session 100% (2 h), and Fable 100% (30 h) when asked |
| `codex_covered_allowances()` | Codex plan `plus` covers weekly only; `pro` covers session and weekly |
| `codex_pro_session_spent()` | `pro`: session 100% (3 h), weekly 0% (full, 100 h) |
| `codex_plus_weekly_spent()` | `plus`: weekly 100% (2%/h, 50 h), session 30% (2 h, uncovered) |

| Scene | What the tests pin |
|---|---|
| two requests, one reading, either order | one claim registered; the other `Held`, also on a fresh `Redeem`; duplicate reply ignored; repeated snapshot changes nothing; `Spent` by the winner; weekly 0 → 100 only after the snapshot |
| all 120 orders of two claims, two replies, one snapshot | at most one claim; `Spent` exactly when the snapshot follows a reply to it; weekly allocatable only after that, or when no claim was pending |
| `Accepted` then a snapshot with an unchanged count and no weekly reading | still `Replied`; the other claim `Held`; permits refused at weekly 0 |
| `Refused`, delayed count, unchanged count, late `Unknown`, second claim with `Unknown` | released to `Banked` only on the count read after the reply; the late reply `Stale`; second claim `Spent` |
| `Unknown`, unchanged count, then a late `Accepted` before another claim | the release is undone: `Replied` with `Accepted` by the first claim, another claim `Held`, an unchanged count after it settles nothing, a fallen one is `Spent` by the first claim |
| count captured in the second of the claim and its `Unknown` reply, delivered a second later | adopted, but the grant stays `Replied` and another claim is `Held`; a count read a second after the reply releases it |
| `Refused`, then counts 0 and 1 of one second, in either order, then 1 a minute later | `Replied` and another claim `Held` after the conflicting pair; `Banked` with one reset in both orders |
| Claude session grant | session clock `AwaitingReRead` then `Unreported` (the five-hour clock is unknown); weekly untouched; session 0 → 100 |
| Claude weekly grant | weekly keeps `At(NOW + 72 h)` throughout; 0 → 100 only after the snapshot |
| Claude Fable grant | Fable and session must be re-read; weekly keeps its boundary and shows the reported share (60 → 64) |
| Codex `pro`, full grant | session adopts the reported refresh, then a revised one, then conflicts and resolves; full weekly keeps `At(NOW + 100 h)` and is never marked |
| Codex `plus`, full grant | uncovered session serves a permit while the weekly is pending, refills on its own clock; a delayed and a missing weekly report credit nothing; the weekly is usable once reported |
| `crossed(true)`, `crossed(false)`, Fable-exhausted with a weekly grant | `Unknown(Effect)`, `Defer` to `NOW + 2 h`, `Unknown(Effect)`; no claim; Sonnet work permitted where only the Fable window blocks |
| `crossed(false)` waited out | `Defer` refused at once; after the session refills, the old basis is `StaleBasis`, a new one `Redeem`, and the job is permitted after the snapshot |
| weekly grant ending in 30 min | `Expired` at the end; balances and the permit unchanged; a lagging inventory does not revive it |
| Codex `plus`, full grant: weekly full (0%, 100 h), uncovered session 100% (2 h), 1 h job | `LetExpire(NoUnlock)`; the claim refused; neither window's clock is touched |
| `claude_fable_exhausted()`, Fable grant, 1 h of Sonnet work; then the same hour as Fable work | `LetExpire(NoUnlock)` and the claim refused, while the Sonnet work is permitted from existing session and weekly balance; `Redeem` for the Fable work |
| Claude Fable grant; the session read at 100% before the reply, then at 0% before it, then at 0% after it | the 100% reading is adopted and the session stays marked and `AwaitingReRead`, so a 10% draw is refused at 0; the 0% reading before the reply is delayed; the re-read restores 100 |
| Codex `pro`, full grant; the session re-read at 0% with no refresh, then with one | `missing`: the session stays marked at 0 and a 100% draw is refused; with its refresh it is `At` the refresh and the draw is permitted |
| Claude weekly grant, weekly exhausted until 30 min; the re-read reports no reset | the weekly keeps `At(NOW + 30 min)`, the same report again is `repeated`, and a draw a second past the boundary is `Refilled` |
| Codex `pro`, full grant, weekly full until 30 min; the re-read omits the weekly's reset | the unrestarted weekly keeps `At(NOW + 30 min)` and is `Refilled` a second past it, while the session draws on its reported refresh |
| weekly at 0% and session under `max`; a session reading under `pro`; another under `max`; then the weekly at 20% under `max` | the weekly is `PlanChanged` after each change; once read again it serves 80, and a 100% draw is refused |
| weekly at 0%; a 100% permit released; the weekly read at 30% in the release second, then at 60% a minute later | after the release the weekly is still 0 and a second 100% draw is refused; a second release is `UnknownPermit`; the 30% reading keeps the draw debited; the 60% one lifts it and serves 40 |
| weekly at 100% and session at 70% | a `1e-9` draw on the weekly is refused twice at 0; the session serves 30 plus that tolerance, then refuses the same dust at 0 |
| `exhausted_weekly()` with a weekly grant on `operator` (one reset) and on `other` (count 0), and a session grant on `operator`, all at one generation and second | `operator`'s `Redeem` is `WrongGrant` on `other`'s grant and on the session grant; `other`'s own `LetExpire(NotRedeemable)` is `NotRecommended`; neither registers a claim, while `operator`'s own weekly claim does |
| weekly 100% (2%/h) refilling in 2 min, session 10% (4 h), weekly grant, 4 h job | `Redeem`; that basis claims a second before the refill, and is `StaleBasis` at the refill and a second after it, with the generation unchanged, where a fresh one is `Unknown(Window(Weekly, Refilled))`; nothing is claimed and the weekly is `Refilled` |

A planner test pins the history rule above: weekly readings under `max` two hours apart, with a
session reading under `pro` between them, are two segments (`First`, `PlanChange`) and the weekly
window is `Unknown(InsufficientHistory)`; with that session reading under `max` they are one
segment at 10%/h.
