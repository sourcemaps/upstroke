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

## Data boundary

The facts are those the existing read-only readers report, with the time and identity a reader's
caller knows. This module takes them as values and never invokes a reader.

| Planner input | The read-only fact it carries |
|---|---|
| `Reading::used_percent`, `Reading::resets_at_unix` | `tracker::subscription::Window`'s `utilization` and `resets_at_unix` |
| `Reading::window` | `five_hour` → `WindowId::Session`, `seven_day` → `WindowId::Weekly`, a `ModelWindow` label → `WindowId::Model` (`"unknown model"` → `ModelLabel::Unknown`) |
| `Reading::observed_at_unix` | the `now_unix` the reader was called with |
| `Reading::account`, `Reading::plan` | caller-supplied: the reader reports neither |
| `Account::coverage` | caller-supplied: for Codex, the windows each plan's banked reset covers (`None` applies `effect` unnarrowed on Claude and leaves every Codex effect `Unknown`) |
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

The account's plan observations, from readings in every window, are kept sorted by second. Each
pair of adjacent readings finds the observations of its own interval by binary search and examines
only those, so a longer history adds work in proportion to its length rather than to its square.

## `fn forecast(`

Only the newest segment forecasts, and only when every one of these holds; otherwise the window is
`WindowState::Unknown` with the first failing reason:

- the window is not an unlabeled model window (`UnlabeledModel`) and has a reading (`NoReading`);
- its latest reading is usable (`Incomplete`, `Contradictory`, `FromFuture`);
- the account's newest plan is unambiguous (`PlanContradiction`: two plans in one second), is the
  segment's plan, and no reading of the account, in any window, from the segment's last reading
  through `now_unix` names another plan (`PlanChanged`). This is the `PlanChange` rule above,
  applied up to now: a change and a change back leave the window unread under the plan it is on
  again, so its forecast waits for a reading of its own;
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

An account with `Account::coverage` narrows the rule by the plan it is on: a window the account's
plan covers keeps the rule, a window it does not cover is `Unchanged`, and every window is `Unknown`
when the plan is not listed. The simulation takes each window's effect from that narrowed rule, so
a Codex `plus` reset, which covers the weekly allowance only, restores nothing in an exhausted
session window.

Without `Account::coverage`, a Claude account takes the rule unnarrowed. A Codex account's banked
reset covers plan-specific allowances (§13), so with no map its every effect is `Unknown`: the
generic rule is not assumed to hold on whatever plan the account is on.

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
- Where the grant has an `Effect::Unknown` on any of the account's windows (by rule, because the
  account's plan is missing from its `Account::coverage`, or because a Codex account has none),
  the comparison runs twice. The cautious bound assumes the unknown windows are not restored; the hopeful bound assumes
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
  `Account::coverage`, and `stale_after_secs` of one hour. A Codex test that expects known effects
  sets `codex_covered_allowances()` (below).
- `claude_fable_exhausted()`: the Fable model window at 100% (10%/h, refills in 50 h), session at
  20% (10%/h, 3 h) and weekly at 40% (1%/h, 72 h). Only work that draws on the Fable window waits.
- `codex_covered_allowances()`: the windows a Codex banked reset covers, per plan: `plus` covers
  weekly only; `pro` covers session and weekly. Synthetic, since no Codex reader exists; the test
  that uses it sets it as the account's `Account::coverage`.
- `on_plan(plan, readings)`: the same readings under the named plan, as `Source::Fixture`.

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
| weekly exhausted, refilling in 1 h, 3 h job: Claude (kept clock) vs a Codex `pro` fixture with `codex_covered_allowances()` (restart) | `Redeem` vs `Defer` to `NOW + 3 h` |
| several usable grants on one exhausted account | `Redeem` for the earliest-ending one; `Unknown(AwaitsReRead)` for the rest |
| session at 100% (25%/h) until 3 h, session grant ending in 30 min: 6 h job; 3 h job; and a session at 10% (10%/h) with a 2 h job | `Unknown(Schedule)`: with no refill after the restart the job stays incomplete, while a refill the new schedule may bring beats the baseline's 10 h; `Redeem`; `LetExpire(NoUnlock)`, since even the best schedule gains nothing |
| `exhausted_weekly`, weekly grant, 4 h job: inventory read `NotUsable` at `NOW - 1`, then `Usable` at `NOW` | `Unknown(Availability)`, then `Redeem` |
| session at 100% refilling `MAX_STEPS + 1` weeks out, weekly at 30% (72 h) recurring, weekly grant, 1 h job; then refilling 2 weeks out | `Unknown(Bound)`: the weekly refills passed in the one wait exceed `MAX_STEPS`; `LetExpire(NoUnlock)` |
| Codex `plus` with `codex_covered_allowances()`, full grant ending in 48 h: weekly at 0% (100 h), uncovered session at 100% (25%/h, 2 h), 1 h job | `LetExpire(NoUnlock)`: on `plus` the reset leaves the session unchanged, and the weekly has no use to restore |
| the same `plus` scene with no `Account::coverage` | `Unknown(Effect)`: every effect is unknown, and the hopeful bound redeems where the cautious one does not |
| weekly under `max` at 98% and 100% (50 and 40 min ago), weekly grant, 4 h job, session readings 30 min ago, 20 min ago and now, the first under `pro`, the others under `max` | the weekly is `Unknown(PlanChanged)` and the grant `Unknown(Window(Weekly, PlanChanged))`; with the first session reading under `max` too, the weekly is forecast and the grant is `Redeem` |
| `claude_fable_exhausted()`, Fable grant ending in 48 h, 1 h of Sonnet work; then the same hour as Fable work | `LetExpire(NoUnlock)`: the Sonnet work runs now on the existing session and weekly balance; `Redeem` for the Fable work |

A planner test pins the history rule above: weekly readings under `max` two hours apart, with a
session reading under `pro` between them, are two segments (`First`, `PlanChange`) and the weekly
window is `Unknown(InsufficientHistory)`; with that session reading under `max` they are one
segment at 10%/h.

Another counts the plan observations the history examines, with no timing: 128 and 256 weekly
readings on one unchanged plan, a second apart, are one segment each, and the history examines
exactly the two observations in each adjacent pair's interval, 254 and 510 in all.
