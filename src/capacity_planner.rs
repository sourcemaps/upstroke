//! Extended notes: `docs/internals/capacity_planner.md`

#![forbid(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros
)]

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

const FULL_PERCENT: f64 = 100.0;
const SECS_PER_HOUR: f64 = 3_600.0;
const SESSION_SECS: f64 = 5.0 * SECS_PER_HOUR;
const WEEK_SECS: f64 = 7.0 * 24.0 * SECS_PER_HOUR;
const TIME_EPSILON_SECS: f64 = 1e-6;
const PERCENT_EPSILON: f64 = 1e-9;
const TIE_SECS_PER_ITEM: f64 = 1.0;
const MAX_STEPS: usize = 100_000;
const MAX_CANDIDATES: usize = 1_024;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AccountId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlanId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RunId(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    Claude,
    Codex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Reader,
    Fixture,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ModelLabel {
    Sonnet,
    Opus,
    Haiku,
    Fable,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WindowId {
    Session,
    Weekly,
    Model(ModelLabel),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Reading {
    pub account: AccountId,
    pub plan: Option<PlanId>,
    pub window: WindowId,
    pub observed_at_unix: u64,
    pub used_percent: Option<f64>,
    pub resets_at_unix: Option<u64>,
    pub source: Source,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub observed_at_unix: u64,
    pub used_percent: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Boundary {
    First,
    PlanChange,
    Refill,
    ResetMoved,
    Discontinuity,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub plan: PlanId,
    pub resets_at_unix: u64,
    pub first: Point,
    pub last: Point,
    pub readings: usize,
    pub opened_by: Boundary,
}

impl Segment {
    #[must_use]
    pub fn percent_per_hour(&self) -> Option<f64> {
        let span = self
            .last
            .observed_at_unix
            .checked_sub(self.first.observed_at_unix)?;
        if span == 0 {
            return None;
        }
        Some((self.last.used_percent - self.first.used_percent) * SECS_PER_HOUR / secs(span))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Latest {
    Usable,
    Incomplete,
    Contradictory,
    FromFuture,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WindowHistory {
    pub account: AccountId,
    pub window: WindowId,
    pub segments: Vec<Segment>,
    pub latest: Latest,
}

#[must_use]
pub fn observed_burn(readings: &[Reading], now_unix: u64) -> Vec<WindowHistory> {
    let mut grouped: BTreeMap<(&AccountId, WindowId), Vec<&Reading>> = BTreeMap::new();
    let mut accounts: BTreeMap<&AccountId, Vec<&Reading>> = BTreeMap::new();
    for reading in readings {
        grouped
            .entry((&reading.account, reading.window))
            .or_default()
            .push(reading);
        accounts.entry(&reading.account).or_default().push(reading);
    }
    let plans: BTreeMap<&AccountId, Vec<(u64, &PlanId)>> = accounts
        .into_iter()
        .map(|(account, readings)| (account, plan_log(&readings)))
        .collect();
    grouped
        .into_iter()
        .map(|((account, window), readings)| {
            let reported = plans.get(account).map_or(&[][..], Vec::as_slice);
            let (segments, latest) = history(
                readings,
                |from, to| named_between(reported, from, to),
                now_unix,
            );
            WindowHistory {
                account: account.clone(),
                window,
                segments,
                latest,
            }
        })
        .collect()
}

fn plan_log<'a>(readings: &[&'a Reading]) -> Vec<(u64, &'a PlanId)> {
    let mut log: Vec<(u64, &PlanId)> = readings
        .iter()
        .filter_map(|reading| Some((reading.observed_at_unix, reading.plan.as_ref()?)))
        .collect();
    log.sort_by_key(|(at, _)| *at);
    log
}

fn named_between<'p, 'a>(
    plans: &'p [(u64, &'a PlanId)],
    from: u64,
    to: u64,
) -> &'p [(u64, &'a PlanId)] {
    let start = plans.partition_point(|(at, _)| *at < from);
    let end = plans.partition_point(|(at, _)| *at <= to);
    plans.get(start..end).unwrap_or_default()
}

fn names_another(named: &[(u64, &PlanId)], plan: &PlanId) -> bool {
    named.iter().any(|(_, other)| *other != plan)
}

fn history<'p>(
    mut readings: Vec<&Reading>,
    named: impl Fn(u64, u64) -> &'p [(u64, &'p PlanId)],
    now_unix: u64,
) -> (Vec<Segment>, Latest) {
    let from_future = readings
        .iter()
        .any(|reading| reading.observed_at_unix > now_unix);
    readings.retain(|reading| reading.observed_at_unix <= now_unix);
    readings.sort_by_key(|reading| reading.observed_at_unix);
    let mut segments: Vec<Segment> = Vec::new();
    let mut latest = Latest::FromFuture;
    let mut broken = false;
    for group in readings.chunk_by(|a, b| a.observed_at_unix == b.observed_at_unix) {
        let Some((first, rest)) = group.split_first() else {
            continue;
        };
        let agreed = rest.iter().all(|other| same_values(first, other));
        let Some((plan, used, resets)) = usable(first).filter(|_| agreed) else {
            broken = true;
            latest = if agreed {
                Latest::Incomplete
            } else {
                Latest::Contradictory
            };
            continue;
        };
        latest = Latest::Usable;
        let point = Point {
            observed_at_unix: first.observed_at_unix,
            used_percent: used,
        };
        let opened_by = match segments.last_mut() {
            None if broken => Boundary::Discontinuity,
            None => Boundary::First,
            Some(segment) => {
                let since = named(segment.last.observed_at_unix, point.observed_at_unix);
                match split(segment, plan, point, resets, since) {
                    Some(boundary) => boundary,
                    None if broken => Boundary::Discontinuity,
                    None => {
                        segment.last = point;
                        segment.readings += 1;
                        continue;
                    }
                }
            }
        };
        broken = false;
        segments.push(Segment {
            plan: plan.clone(),
            resets_at_unix: resets,
            first: point,
            last: point,
            readings: 1,
            opened_by,
        });
    }
    if from_future {
        latest = Latest::FromFuture;
    }
    (segments, latest)
}

fn usable(reading: &Reading) -> Option<(&PlanId, f64, u64)> {
    let plan = reading.plan.as_ref()?;
    let used = reading
        .used_percent
        .filter(|used| (0.0..=FULL_PERCENT).contains(used))?;
    let resets = reading
        .resets_at_unix
        .filter(|resets| *resets > reading.observed_at_unix)?;
    Some((plan, used, resets))
}

fn same_values(a: &Reading, b: &Reading) -> bool {
    a.plan == b.plan
        && a.used_percent.map(f64::to_bits) == b.used_percent.map(f64::to_bits)
        && a.resets_at_unix == b.resets_at_unix
}

fn split(
    segment: &Segment,
    plan: &PlanId,
    point: Point,
    resets: u64,
    since: &[(u64, &PlanId)],
) -> Option<Boundary> {
    let moved = resets != segment.resets_at_unix;
    if *plan != segment.plan || names_another(since, &segment.plan) {
        Some(Boundary::PlanChange)
    } else if point.used_percent < segment.last.used_percent
        || (moved && segment.resets_at_unix <= point.observed_at_unix)
    {
        Some(Boundary::Refill)
    } else if moved {
        Some(Boundary::ResetMoved)
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowUnknown {
    NoReading,
    UnlabeledModel,
    FromFuture,
    Incomplete,
    Contradictory,
    PlanContradiction,
    PlanChanged,
    Stale,
    Refilled,
    InsufficientHistory,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WindowForecast {
    pub plan: PlanId,
    pub used_percent: f64,
    pub percent_per_hour: f64,
    pub refill_unix: u64,
    pub exhaustion_unix: Option<u64>,
    pub since_unix: u64,
    pub readings: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WindowState {
    Known(WindowForecast),
    Unknown(WindowUnknown),
}

enum PlanEvidence<'a> {
    Absent,
    One(&'a PlanId),
    Contradictory,
}

fn current_plan<'a>(readings: &[&'a Reading], now_unix: u64) -> PlanEvidence<'a> {
    let mut newest: Option<(u64, &PlanId, bool)> = None;
    for reading in readings {
        let Some(plan) = reading.plan.as_ref() else {
            continue;
        };
        let at = reading.observed_at_unix;
        if at > now_unix {
            continue;
        }
        newest = match newest {
            Some((seen, other, conflict)) if seen == at => {
                Some((at, other, conflict || other != plan))
            }
            Some((seen, _, _)) if seen > at => newest,
            _ => Some((at, plan, false)),
        };
    }
    match newest {
        None => PlanEvidence::Absent,
        Some((_, _, true)) => PlanEvidence::Contradictory,
        Some((_, plan, false)) => PlanEvidence::One(plan),
    }
}

struct Freshness {
    now_unix: u64,
    stale_after_secs: u64,
}

fn window_state(
    window: WindowId,
    history: Option<&WindowHistory>,
    plan: &PlanEvidence<'_>,
    named: &[(u64, &PlanId)],
    freshness: &Freshness,
) -> WindowState {
    match forecast(window, history, plan, named, freshness) {
        Ok(forecast) => WindowState::Known(forecast),
        Err(reason) => WindowState::Unknown(reason),
    }
}

fn forecast(
    window: WindowId,
    history: Option<&WindowHistory>,
    plan: &PlanEvidence<'_>,
    named: &[(u64, &PlanId)],
    freshness: &Freshness,
) -> Result<WindowForecast, WindowUnknown> {
    if window == WindowId::Model(ModelLabel::Unknown) {
        return Err(WindowUnknown::UnlabeledModel);
    }
    let history = history.ok_or(WindowUnknown::NoReading)?;
    match history.latest {
        Latest::Usable => {}
        Latest::Incomplete => return Err(WindowUnknown::Incomplete),
        Latest::Contradictory => return Err(WindowUnknown::Contradictory),
        Latest::FromFuture => return Err(WindowUnknown::FromFuture),
    }
    let segment = history.segments.last().ok_or(WindowUnknown::NoReading)?;
    let plan = match plan {
        PlanEvidence::One(plan) => *plan,
        PlanEvidence::Contradictory => return Err(WindowUnknown::PlanContradiction),
        PlanEvidence::Absent => return Err(WindowUnknown::NoReading),
    };
    let since = named_between(named, segment.last.observed_at_unix, freshness.now_unix);
    if segment.plan != *plan || names_another(since, &segment.plan) {
        return Err(WindowUnknown::PlanChanged);
    }
    let age = freshness
        .now_unix
        .saturating_sub(segment.last.observed_at_unix);
    if age > freshness.stale_after_secs {
        return Err(WindowUnknown::Stale);
    }
    if segment.resets_at_unix <= freshness.now_unix {
        return Err(WindowUnknown::Refilled);
    }
    let rate = segment
        .percent_per_hour()
        .ok_or(WindowUnknown::InsufficientHistory)?;
    let used = (segment.last.used_percent + rate * secs(age) / SECS_PER_HOUR).min(FULL_PERCENT);
    let exhaustion_unix = if rate > 0.0 {
        Some(
            freshness
                .now_unix
                .saturating_add(whole_secs_up((FULL_PERCENT - used) * SECS_PER_HOUR / rate)),
        )
    } else {
        None
    };
    Ok(WindowForecast {
        plan: plan.clone(),
        used_percent: used,
        percent_per_hour: rate,
        refill_unix: segment.resets_at_unix,
        exhaustion_unix,
        since_unix: segment.first.observed_at_unix,
        readings: segment.readings,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrantKind {
    Weekly,
    Session,
    Full,
    Fable,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Availability {
    Usable,
    NotUsable,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grant {
    pub resets_left: Option<u64>,
    pub ends_at_unix: Option<u64>,
    pub kind: GrantKind,
    pub availability: Availability,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inventory {
    pub observed_at_unix: u64,
    pub grants: Vec<Grant>,
    pub source: Source,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InventoryEvidence {
    Unknown,
    Known(Inventory),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    pub id: AccountId,
    pub provider: Provider,
    pub inventory: InventoryEvidence,
    pub coverage: Option<BTreeMap<PlanId, BTreeSet<WindowId>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    Restores,
    RestoresBelowFull,
    Unchanged,
    Unknown,
}

#[must_use]
pub const fn effect(provider: Provider, kind: GrantKind, window: WindowId) -> Effect {
    match provider {
        Provider::Claude => match (kind, window) {
            (GrantKind::Weekly, WindowId::Weekly)
            | (GrantKind::Session, WindowId::Session)
            | (GrantKind::Full, WindowId::Session | WindowId::Weekly)
            | (GrantKind::Fable, WindowId::Session | WindowId::Model(ModelLabel::Fable)) => {
                Effect::Restores
            }
            (GrantKind::Weekly, WindowId::Session) | (GrantKind::Session, WindowId::Weekly) => {
                Effect::Unchanged
            }
            (GrantKind::Weekly | GrantKind::Session | GrantKind::Full, WindowId::Model(_))
            | (GrantKind::Fable, WindowId::Weekly | WindowId::Model(_))
            | (GrantKind::Unknown, _) => Effect::Unknown,
        },
        Provider::Codex => match (kind, window) {
            (GrantKind::Weekly, WindowId::Weekly)
            | (GrantKind::Session, WindowId::Session)
            | (GrantKind::Full, WindowId::Session | WindowId::Weekly) => Effect::RestoresBelowFull,
            (GrantKind::Weekly, WindowId::Session) | (GrantKind::Session, WindowId::Weekly) => {
                Effect::Unchanged
            }
            (GrantKind::Weekly | GrantKind::Session | GrantKind::Full, WindowId::Model(_))
            | (GrantKind::Fable | GrantKind::Unknown, _) => Effect::Unknown,
        },
    }
}

fn covered_effect(
    provider: Provider,
    kind: GrantKind,
    window: WindowId,
    coverage: Option<&BTreeMap<PlanId, BTreeSet<WindowId>>>,
    plan: Option<&PlanId>,
) -> Effect {
    let rule = effect(provider, kind, window);
    let Some(coverage) = coverage else {
        return match provider {
            Provider::Claude => rule,
            Provider::Codex => Effect::Unknown,
        };
    };
    match plan.and_then(|plan| coverage.get(plan)) {
        Some(covered) if covered.contains(&window) => rule,
        Some(_) => Effect::Unchanged,
        None => Effect::Unknown,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Schedule {
    Kept,
    MustReRead,
}

#[must_use]
pub const fn schedule_after_restore(provider: Provider, window: WindowId) -> Schedule {
    match (provider, window) {
        (Provider::Claude, WindowId::Weekly) => Schedule::Kept,
        (Provider::Claude, WindowId::Session | WindowId::Model(_)) | (Provider::Codex, _) => {
            Schedule::MustReRead
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Clock {
    Fixed(f64),
    Rolling(f64),
    Unestablished,
}

const fn natural_clock(provider: Provider, window: WindowId) -> Clock {
    match (provider, window) {
        (Provider::Claude, WindowId::Weekly) => Clock::Fixed(WEEK_SECS),
        (Provider::Claude, WindowId::Session) => Clock::Rolling(SESSION_SECS),
        (Provider::Claude, WindowId::Model(_)) | (Provider::Codex, _) => Clock::Unestablished,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Purpose {
    Feature {
        project: String,
        feature: Option<String>,
    },
    ResearchFor {
        parent: RunId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub id: RunId,
    pub purpose: Purpose,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Demand {
    pub run: RunId,
    pub account: AccountId,
    pub hours: f64,
    pub model: ModelLabel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Priorities {
    pub projects: BTreeMap<String, u32>,
    pub features: BTreeMap<String, BTreeMap<String, u32>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DemandProblem {
    RunMissing,
    RunConflict,
    ParentMissing,
    ParentCycle,
    InvalidHours,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rank {
    Priority(u32),
    Unprioritized,
    Invalid(DemandProblem),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Ranked {
    pub run: RunId,
    pub account: AccountId,
    pub hours: f64,
    pub model: ModelLabel,
    pub rank: Rank,
}

#[must_use]
pub fn rank_demand(runs: &[Run], demand: &[Demand], priorities: &Priorities) -> Vec<Ranked> {
    let mut purposes: BTreeMap<&RunId, Option<&Purpose>> = BTreeMap::new();
    for run in runs {
        purposes
            .entry(&run.id)
            .and_modify(|seen| *seen = None)
            .or_insert(Some(&run.purpose));
    }
    let mut ranked: Vec<Ranked> = demand
        .iter()
        .map(|item| Ranked {
            run: item.run.clone(),
            account: item.account.clone(),
            hours: item.hours,
            model: item.model,
            rank: if item.hours.is_finite() && item.hours > 0.0 {
                rank_of(&item.run, &purposes, priorities)
            } else {
                Rank::Invalid(DemandProblem::InvalidHours)
            },
        })
        .collect();
    ranked.sort_by(|a, b| {
        order_key(b.rank)
            .cmp(&order_key(a.rank))
            .then_with(|| a.run.cmp(&b.run))
    });
    ranked
}

const fn order_key(rank: Rank) -> (u8, u32) {
    match rank {
        Rank::Priority(priority) => (2, priority),
        Rank::Unprioritized => (1, 0),
        Rank::Invalid(_) => (0, 0),
    }
}

fn rank_of<'a>(
    run: &'a RunId,
    purposes: &BTreeMap<&'a RunId, Option<&'a Purpose>>,
    priorities: &Priorities,
) -> Rank {
    let mut visited: BTreeSet<&RunId> = BTreeSet::new();
    let mut current = run;
    let mut absent = DemandProblem::RunMissing;
    loop {
        if !visited.insert(current) {
            return Rank::Invalid(DemandProblem::ParentCycle);
        }
        match purposes.get(current) {
            None => return Rank::Invalid(absent),
            Some(None) => return Rank::Invalid(DemandProblem::RunConflict),
            Some(Some(Purpose::ResearchFor { parent })) => {
                current = parent;
                absent = DemandProblem::ParentMissing;
            }
            Some(Some(Purpose::Feature { project, feature })) => {
                let by_feature = feature
                    .as_ref()
                    .and_then(|feature| priorities.features.get(project)?.get(feature));
                return by_feature
                    .or_else(|| priorities.projects.get(project))
                    .map_or(Rank::Unprioritized, |priority| Rank::Priority(*priority));
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Input {
    pub now_unix: u64,
    pub stale_after_secs: u64,
    pub accounts: Vec<Account>,
    pub readings: Vec<Reading>,
    pub runs: Vec<Run>,
    pub demand: Vec<Demand>,
    pub priorities: Priorities,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpireReason {
    NotRedeemable,
    NoUnlock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Undecided {
    Availability,
    Window(WindowId, WindowUnknown),
    Demand(DemandProblem),
    Model,
    Effect,
    Schedule,
    Bound,
    AwaitsReRead,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Redeem,
    Defer { until_unix: u64 },
    LetExpire(ExpireReason),
    Unknown(Undecided),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InventoryUnknown {
    NotRead,
    Stale,
    FromFuture,
    Conflict,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Grants {
    Unknown(InventoryUnknown),
    KnownEmpty,
    Decided(Vec<Decision>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct AccountPlan {
    pub account: AccountId,
    pub windows: Vec<(WindowId, WindowState)>,
    pub queue: Vec<Ranked>,
    pub grants: Grants,
    pub rests_on_fixture: bool,
}

#[must_use]
pub fn plan(input: &Input) -> Vec<AccountPlan> {
    let freshness = Freshness {
        now_unix: input.now_unix,
        stale_after_secs: input.stale_after_secs,
    };
    let histories = observed_burn(&input.readings, input.now_unix);
    let by_window: BTreeMap<(&AccountId, WindowId), &WindowHistory> = histories
        .iter()
        .map(|history| ((&history.account, history.window), history))
        .collect();
    let ranked = rank_demand(&input.runs, &input.demand, &input.priorities);
    input
        .accounts
        .iter()
        .map(|account| {
            let readings: Vec<&Reading> = input
                .readings
                .iter()
                .filter(|reading| reading.account == account.id)
                .collect();
            let plan_evidence = current_plan(&readings, input.now_unix);
            let named = plan_log(&readings);
            let mut ids: BTreeSet<WindowId> = BTreeSet::from([WindowId::Session, WindowId::Weekly]);
            ids.extend(readings.iter().map(|reading| reading.window));
            let windows: Vec<(WindowId, WindowState)> = ids
                .into_iter()
                .map(|id| {
                    let history = by_window.get(&(&account.id, id)).copied();
                    let state = window_state(id, history, &plan_evidence, &named, &freshness);
                    (id, state)
                })
                .collect();
            let queue: Vec<Ranked> = ranked
                .iter()
                .filter(|item| item.account == account.id)
                .cloned()
                .collect();
            let duplicated = input
                .accounts
                .iter()
                .filter(|other| other.id == account.id)
                .count()
                > 1;
            let context = Context {
                now_unix: input.now_unix,
                provider: account.provider,
                coverage: account.coverage.as_ref(),
                windows: &windows,
                queue: &queue,
            };
            let grants = if duplicated {
                Grants::Unknown(InventoryUnknown::Conflict)
            } else {
                assess_grants(&context, &account.inventory, &freshness)
            };
            let rests_on_fixture = readings
                .iter()
                .any(|reading| reading.source == Source::Fixture)
                || matches!(
                    &account.inventory,
                    InventoryEvidence::Known(inventory) if inventory.source == Source::Fixture
                );
            AccountPlan {
                account: account.id.clone(),
                windows,
                queue,
                grants,
                rests_on_fixture,
            }
        })
        .collect()
}

struct Context<'a> {
    now_unix: u64,
    provider: Provider,
    coverage: Option<&'a BTreeMap<PlanId, BTreeSet<WindowId>>>,
    windows: &'a [(WindowId, WindowState)],
    queue: &'a [Ranked],
}

fn assess_grants(
    context: &Context<'_>,
    inventory: &InventoryEvidence,
    freshness: &Freshness,
) -> Grants {
    let InventoryEvidence::Known(inventory) = inventory else {
        return Grants::Unknown(InventoryUnknown::NotRead);
    };
    if inventory.observed_at_unix > freshness.now_unix {
        return Grants::Unknown(InventoryUnknown::FromFuture);
    }
    if freshness
        .now_unix
        .saturating_sub(inventory.observed_at_unix)
        > freshness.stale_after_secs
    {
        return Grants::Unknown(InventoryUnknown::Stale);
    }
    if inventory.grants.is_empty() {
        return Grants::KnownEmpty;
    }
    let decisions = inventory
        .grants
        .iter()
        .map(|grant| decide(context, grant))
        .collect();
    Grants::Decided(one_redemption_at_a_time(&inventory.grants, decisions))
}

fn one_redemption_at_a_time(grants: &[Grant], decisions: Vec<Decision>) -> Vec<Decision> {
    let kept = decisions
        .iter()
        .zip(grants)
        .enumerate()
        .filter(|(_, (decision, _))| **decision == Decision::Redeem)
        .min_by_key(|(index, (_, grant))| (grant.ends_at_unix, *index))
        .map(|(index, _)| index);
    let Some(kept) = kept else {
        return decisions;
    };
    decisions
        .into_iter()
        .enumerate()
        .map(|(index, decision)| match decision {
            _ if index == kept => decision,
            Decision::Redeem
            | Decision::Defer { .. }
            | Decision::LetExpire(ExpireReason::NoUnlock) => {
                Decision::Unknown(Undecided::AwaitsReRead)
            }
            Decision::LetExpire(ExpireReason::NotRedeemable) | Decision::Unknown(_) => decision,
        })
        .collect()
}

fn decide(context: &Context<'_>, grant: &Grant) -> Decision {
    if grant
        .ends_at_unix
        .is_some_and(|ends| ends <= context.now_unix)
        || grant.resets_left == Some(0)
    {
        return Decision::LetExpire(ExpireReason::NotRedeemable);
    }
    match grant.availability {
        Availability::NotUsable | Availability::Unknown => {
            return Decision::Unknown(Undecided::Availability);
        }
        Availability::Usable => {}
    }
    let (Some(ends_at_unix), Some(1..)) = (grant.ends_at_unix, grant.resets_left) else {
        return Decision::Unknown(Undecided::Availability);
    };
    if context.queue.is_empty() {
        return Decision::LetExpire(ExpireReason::NoUnlock);
    }
    let mut windows = Vec::with_capacity(context.windows.len());
    for (id, state) in context.windows {
        let drawn_on = context
            .queue
            .iter()
            .any(|item| draws_on(*id, item.model, Reach::EveryModel));
        if !drawn_on {
            continue;
        }
        match state {
            WindowState::Known(forecast) => {
                windows.push(SimWindow::new(context, grant.kind, *id, forecast));
            }
            WindowState::Unknown(reason) => {
                return Decision::Unknown(Undecided::Window(*id, *reason));
            }
        }
    }
    let mut ranks = Vec::with_capacity(context.queue.len());
    let mut work = Vec::with_capacity(context.queue.len());
    let mut models = Vec::with_capacity(context.queue.len());
    for item in context.queue {
        if let Rank::Invalid(problem) = item.rank {
            return Decision::Unknown(Undecided::Demand(problem));
        }
        ranks.push(item.rank);
        work.push(item.hours * SECS_PER_HOUR);
        models.push(item.model);
    }
    let scenario = Scenario {
        provider: context.provider,
        windows,
        ranks,
        work,
        models,
        expiry_secs: secs(ends_at_unix.saturating_sub(context.now_unix)),
        unlabeled: Reach::EveryModel,
    };
    let now_unix = context.now_unix;
    let reach_matters = scenario.models.contains(&ModelLabel::Unknown)
        && scenario.windows.iter().any(|window| {
            draws_on(window.id, ModelLabel::Unknown, Reach::EveryModel)
                != draws_on(window.id, ModelLabel::Unknown, Reach::NoModel)
        });
    let resolved = if reach_matters {
        let on_every = resolve(&scenario, now_unix);
        let on_none = resolve(
            &Scenario {
                unlabeled: Reach::NoModel,
                ..scenario
            },
            now_unix,
        );
        match (on_every, on_none) {
            (Ok(every), Ok(none)) if every == none => Ok(every),
            (Ok(_), Ok(_)) => Ok(Decision::Unknown(Undecided::Model)),
            (Err(too_long), _) | (_, Err(too_long)) => Err(too_long),
        }
    } else {
        resolve(&scenario, now_unix)
    };
    resolved.unwrap_or(Decision::Unknown(Undecided::Bound))
}

fn resolve(scenario: &Scenario, now_unix: u64) -> Result<Decision, TooLong> {
    let cautious = scenario
        .choose(Assumption::NoRestore)
        .map(|choice| decision(choice, now_unix));
    let touches_unknown = scenario
        .windows
        .iter()
        .any(|window| window.effect == Effect::Unknown);
    let resolved = if touches_unknown {
        let hopeful = scenario
            .choose(Assumption::FullRestore)
            .map(|choice| decision(choice, now_unix));
        match (cautious, hopeful) {
            (Ok(low), Ok(high)) if low == high => Ok(low),
            (Ok(_), Ok(_)) => Ok(Decision::Unknown(Undecided::Effect)),
            (Err(too_long), _) | (_, Err(too_long)) => Err(too_long),
        }
    } else {
        cautious
    };
    match resolved {
        Ok(Decision::LetExpire(ExpireReason::NoUnlock)) => {
            scenario.choose(Assumption::BestSchedule).map(|choice| {
                choice.map_or(Decision::LetExpire(ExpireReason::NoUnlock), |_| {
                    Decision::Unknown(Undecided::Schedule)
                })
            })
        }
        other => other,
    }
}

fn decision(choice: Option<f64>, now_unix: u64) -> Decision {
    let Some(at) = choice else {
        return Decision::LetExpire(ExpireReason::NoUnlock);
    };
    if at < 1.0 {
        Decision::Redeem
    } else {
        Decision::Defer {
            until_unix: now_unix.saturating_add(whole_secs_up(at)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reach {
    EveryModel,
    NoModel,
}

fn draws_on(window: WindowId, model: ModelLabel, unlabeled: Reach) -> bool {
    match (window, model) {
        (WindowId::Session | WindowId::Weekly | WindowId::Model(ModelLabel::Unknown), _) => true,
        (WindowId::Model(_), ModelLabel::Unknown) => unlabeled == Reach::EveryModel,
        (WindowId::Model(label), _) => label == model,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Assumption {
    NoRestore,
    FullRestore,
    BestSchedule,
}

#[derive(Debug, Clone, Copy)]
struct Redemption {
    at: f64,
    assumption: Assumption,
}

#[derive(Debug, Clone, Copy)]
struct SimWindow {
    id: WindowId,
    used: f64,
    rate: f64,
    refill: Option<f64>,
    clock: Clock,
    effect: Effect,
    awaiting_use: bool,
}

impl SimWindow {
    fn new(
        context: &Context<'_>,
        kind: GrantKind,
        id: WindowId,
        forecast: &WindowForecast,
    ) -> Self {
        Self {
            id,
            used: forecast.used_percent,
            rate: forecast.percent_per_hour / SECS_PER_HOUR,
            refill: Some(secs(forecast.refill_unix.saturating_sub(context.now_unix))),
            clock: natural_clock(context.provider, id),
            effect: covered_effect(
                context.provider,
                kind,
                id,
                context.coverage,
                Some(&forecast.plan),
            ),
            awaiting_use: false,
        }
    }

    fn exhausted(&self) -> bool {
        self.used >= FULL_PERCENT - PERCENT_EPSILON
    }

    fn limits(&self, model: ModelLabel, unlabeled: Reach) -> bool {
        draws_on(self.id, model, unlabeled)
    }

    fn refill_due(
        &mut self,
        clock: f64,
        events: &mut Vec<f64>,
        budget: &mut usize,
    ) -> Result<(), TooLong> {
        while let Some(refill) = self
            .refill
            .filter(|refill| *refill <= clock + TIME_EPSILON_SECS)
        {
            *budget = budget.checked_sub(1).ok_or(TooLong)?;
            self.used = 0.0;
            events.push(refill);
            (self.refill, self.awaiting_use) = match self.clock {
                Clock::Fixed(period) => (Some(refill + period), false),
                Clock::Rolling(_) => (None, true),
                Clock::Unestablished => (None, false),
            };
        }
        Ok(())
    }
}

fn restore(provider: Provider, windows: &mut [SimWindow], redemption: &Redemption) {
    for window in windows {
        let rule = window.effect;
        let restored = match rule {
            Effect::Restores => true,
            Effect::RestoresBelowFull => window.used > 0.0,
            Effect::Unchanged => false,
            Effect::Unknown => redemption.assumption != Assumption::NoRestore,
        };
        if !restored {
            continue;
        }
        window.used = 0.0;
        if schedule_after_restore(provider, window.id) == Schedule::Kept {
            continue;
        }
        if redemption.assumption == Assumption::BestSchedule {
            window.rate = 0.0;
        } else if rule == Effect::Unknown {
            continue;
        }
        window.refill = None;
        window.awaiting_use = false;
    }
}

struct TooLong;

struct Outcome {
    completions: Vec<Option<f64>>,
    events: Vec<f64>,
}

#[derive(Debug, Clone, Copy, Default)]
struct Level {
    incomplete: usize,
    total: f64,
    items: usize,
}

struct Scenario {
    provider: Provider,
    windows: Vec<SimWindow>,
    ranks: Vec<Rank>,
    work: Vec<f64>,
    models: Vec<ModelLabel>,
    expiry_secs: f64,
    unlabeled: Reach,
}

impl Scenario {
    fn choose(&self, assumption: Assumption) -> Result<Option<f64>, TooLong> {
        let baseline = self.simulate(None)?;
        let mut candidates: Vec<f64> = baseline
            .events
            .iter()
            .map(|at| (at - TIME_EPSILON_SECS).ceil())
            .filter(|at| *at >= 1.0 && *at < self.expiry_secs)
            .collect();
        let last_chance = self.expiry_secs - 1.0;
        if last_chance > 0.0 {
            candidates.push(last_chance);
        }
        candidates.push(0.0);
        candidates.sort_by(f64::total_cmp);
        candidates.dedup();
        if candidates.len() > MAX_CANDIDATES {
            return Err(TooLong);
        }
        let mut best = None;
        let mut best_score = self.score(&baseline);
        for at in candidates.into_iter().rev() {
            let outcome = self.simulate(Some(Redemption { at, assumption }))?;
            let score = self.score(&outcome);
            if compare(&score, &best_score) == Ordering::Greater {
                best = Some(at);
                best_score = score;
            }
        }
        Ok(best)
    }

    fn simulate(&self, redemption: Option<Redemption>) -> Result<Outcome, TooLong> {
        let mut windows = self.windows.clone();
        let mut pending = redemption;
        let mut left = self.work.clone();
        let mut completions = vec![None; left.len()];
        let mut events = Vec::new();
        let mut clock = 0.0_f64;
        let mut current = 0_usize;
        let mut budget = MAX_STEPS;
        let reach = self.unlabeled;
        loop {
            budget = budget.checked_sub(1).ok_or(TooLong)?;
            for window in &mut windows {
                window.refill_due(clock, &mut events, &mut budget)?;
            }
            if let Some(due) = pending.filter(|due| due.at <= clock + TIME_EPSILON_SECS) {
                restore(self.provider, &mut windows, &due);
                pending = None;
            }
            let (Some(remaining), Some(&model)) = (left.get_mut(current), self.models.get(current))
            else {
                return Ok(Outcome {
                    completions,
                    events,
                });
            };
            let redeem_at = pending.map(|due| due.at);
            let blocked = |window: &SimWindow| window.limits(model, reach) && window.exhausted();
            if windows.iter().any(blocked) {
                let unblock = windows
                    .iter()
                    .filter(|window| blocked(window))
                    .filter_map(|window| window.refill)
                    .chain(redeem_at)
                    .min_by(f64::total_cmp);
                let Some(at) = unblock else {
                    return Ok(Outcome {
                        completions,
                        events,
                    });
                };
                clock = at;
                continue;
            }
            for window in &mut windows {
                if window.limits(model, reach) && window.awaiting_use && window.rate > 0.0 {
                    if let Clock::Rolling(period) = window.clock {
                        window.refill = Some(clock + period);
                    }
                    window.awaiting_use = false;
                }
            }
            let mut step = *remaining;
            for window in &windows {
                if window.limits(model, reach) && window.rate > 0.0 {
                    step = step.min((FULL_PERCENT - window.used) / window.rate);
                }
                if let Some(refill) = window.refill {
                    step = step.min(refill - clock);
                }
            }
            if let Some(at) = redeem_at {
                step = step.min(at - clock);
            }
            let step = step.max(0.0);
            for window in &mut windows {
                if window.limits(model, reach) && window.rate > 0.0 {
                    if (FULL_PERCENT - window.used) / window.rate - step <= TIME_EPSILON_SECS {
                        window.used = FULL_PERCENT;
                        events.push(clock + step);
                    } else {
                        window.used += window.rate * step;
                    }
                }
            }
            clock += step;
            *remaining -= step;
            if *remaining <= TIME_EPSILON_SECS {
                if let Some(slot) = completions.get_mut(current) {
                    *slot = Some(clock);
                }
                current += 1;
            }
        }
    }

    fn score(&self, outcome: &Outcome) -> Vec<Level> {
        let mut levels: Vec<(Rank, Level)> = Vec::new();
        for (rank, completion) in self.ranks.iter().zip(&outcome.completions) {
            if levels.last().is_none_or(|(seen, _)| seen != rank) {
                levels.push((*rank, Level::default()));
            }
            if let Some((_, level)) = levels.last_mut() {
                level.items += 1;
                match completion {
                    Some(at) => level.total += at,
                    None => level.incomplete += 1,
                }
            }
        }
        levels.into_iter().map(|(_, level)| level).collect()
    }
}

fn compare(a: &[Level], b: &[Level]) -> Ordering {
    for (x, y) in a.iter().zip(b) {
        match y.incomplete.cmp(&x.incomplete) {
            Ordering::Equal => {}
            decided => return decided,
        }
        let tolerance = TIE_SECS_PER_ITEM * count(x.items);
        if x.total + tolerance < y.total {
            return Ordering::Greater;
        }
        if y.total + tolerance < x.total {
            return Ordering::Less;
        }
    }
    Ordering::Equal
}

fn secs(value: u64) -> f64 {
    value as f64
}

fn count(value: usize) -> f64 {
    value as f64
}

fn whole_secs_up(value: f64) -> u64 {
    value.ceil() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    const NOW: u64 = 1_800_000_000;
    const HOUR: u64 = 3_600;

    fn operator() -> AccountId {
        AccountId("operator".to_owned())
    }

    fn plan_id(name: &str) -> PlanId {
        PlanId(name.to_owned())
    }

    fn run_id(name: &str) -> RunId {
        RunId(name.to_owned())
    }

    fn raw(
        plan: Option<&str>,
        window: WindowId,
        observed_at_unix: u64,
        used_percent: Option<f64>,
        resets_at_unix: u64,
    ) -> Reading {
        Reading {
            account: operator(),
            plan: plan.map(plan_id),
            window,
            observed_at_unix,
            used_percent,
            resets_at_unix: Some(resets_at_unix),
            source: Source::Reader,
        }
    }

    fn reading_on(plan: &str, window: WindowId, ago: u64, used: f64, resets_in: u64) -> Reading {
        raw(Some(plan), window, NOW - ago, Some(used), NOW + resets_in)
    }

    fn reading(window: WindowId, ago: u64, used: f64, resets_in: u64) -> Reading {
        reading_on("max", window, ago, used, resets_in)
    }

    fn burning(window: WindowId, used_now: f64, per_hour: f64, resets_in: u64) -> Vec<Reading> {
        vec![
            reading(window, HOUR, used_now - per_hour, resets_in),
            reading(window, 0, used_now, resets_in),
        ]
    }

    fn usable(kind: GrantKind, ends_in: u64) -> Grant {
        Grant {
            resets_left: Some(1),
            ends_at_unix: Some(NOW + ends_in),
            kind,
            availability: Availability::Usable,
        }
    }

    fn inventory(grants: Vec<Grant>) -> InventoryEvidence {
        InventoryEvidence::Known(Inventory {
            observed_at_unix: NOW,
            grants,
            source: Source::Reader,
        })
    }

    fn input(
        provider: Provider,
        readings: Vec<Reading>,
        grants: Vec<Grant>,
        work: &[(&str, u32, f64)],
    ) -> Input {
        Input {
            now_unix: NOW,
            stale_after_secs: HOUR,
            accounts: vec![Account {
                id: operator(),
                provider,
                inventory: inventory(grants),
                coverage: None,
            }],
            readings,
            runs: work
                .iter()
                .map(|(run, _, _)| Run {
                    id: run_id(run),
                    purpose: Purpose::Feature {
                        project: (*run).to_owned(),
                        feature: None,
                    },
                })
                .collect(),
            demand: work
                .iter()
                .map(|(run, _, hours)| Demand {
                    run: run_id(run),
                    account: operator(),
                    hours: *hours,
                    model: ModelLabel::Unknown,
                })
                .collect(),
            priorities: Priorities {
                projects: work
                    .iter()
                    .map(|(run, priority, _)| ((*run).to_owned(), *priority))
                    .collect(),
                features: BTreeMap::new(),
            },
        }
    }

    fn claude(readings: Vec<Reading>, grant: Grant, work: &[(&str, u32, f64)]) -> Input {
        input(Provider::Claude, readings, vec![grant], work)
    }

    fn only_plan(input: &Input) -> AccountPlan {
        let mut plans = plan(input);
        let only = plans.pop().expect("one account is planned");
        assert!(plans.is_empty(), "{plans:#?}");
        only
    }

    fn decisions(input: &Input) -> Vec<Decision> {
        match only_plan(input).grants {
            Grants::Decided(decisions) => decisions,
            other => panic!("expected decided grants, got {other:?}"),
        }
    }

    fn decision(input: &Input) -> Decision {
        match decisions(input).as_slice() {
            [one] => *one,
            other => panic!("expected one decision, got {other:?}"),
        }
    }

    fn state(plan: &AccountPlan, window: WindowId) -> WindowState {
        plan.windows
            .iter()
            .find(|(id, _)| *id == window)
            .map(|(_, state)| state.clone())
            .expect("the window is reported")
    }

    fn states(readings: Vec<Reading>) -> AccountPlan {
        only_plan(&input(Provider::Claude, readings, Vec::new(), &[]))
    }

    fn close(actual: f64, expected: f64) -> bool {
        (actual - expected).abs() < 1e-9
    }

    fn exhausted_weekly() -> Vec<Reading> {
        let mut readings = burning(WindowId::Weekly, 100.0, 2.0, 72 * HOUR);
        readings.extend(burning(WindowId::Session, 10.0, 10.0, 4 * HOUR));
        readings
    }

    fn filling_weekly(weekly_resets_in: u64) -> Vec<Reading> {
        let mut readings = burning(WindowId::Weekly, 60.0, 2.0, weekly_resets_in);
        readings.extend(burning(WindowId::Session, 0.0, 0.0, 5 * HOUR));
        readings
    }

    fn one_job(hours: f64) -> [(&'static str, u32, f64); 1] {
        [("job", 5, hours)]
    }

    #[test]
    fn burn_is_the_rise_between_the_first_and_last_comparable_readings() {
        let readings = [
            reading(WindowId::Weekly, 2 * HOUR, 10.0, 72 * HOUR),
            reading(WindowId::Weekly, 0, 26.0, 72 * HOUR),
            reading(WindowId::Weekly, HOUR, 20.0, 72 * HOUR),
        ];
        let histories = observed_burn(&readings, NOW);
        let [history] = histories.as_slice() else {
            panic!("one window history, got {histories:#?}");
        };
        assert_eq!(history.latest, Latest::Usable);
        let [segment] = history.segments.as_slice() else {
            panic!("one segment, got {history:#?}");
        };
        assert_eq!(segment.opened_by, Boundary::First);
        assert_eq!(segment.readings, 3);
        assert_eq!(segment.first.observed_at_unix, NOW - 2 * HOUR);
        assert_eq!(segment.last.observed_at_unix, NOW);
        assert!(
            segment
                .percent_per_hour()
                .is_some_and(|rate| close(rate, 8.0)),
            "{segment:#?}"
        );
    }

    #[test]
    fn history_splits_at_refills_plan_changes_moved_resets_and_discontinuities() {
        let weekly = |plan: Option<&str>, ago: u64, used: Option<f64>, resets_in: u64| {
            raw(plan, WindowId::Weekly, NOW - ago, used, NOW + resets_in)
        };
        let readings = [
            weekly(Some("max"), 10 * HOUR, Some(10.0), 72 * HOUR),
            weekly(Some("max"), 9 * HOUR, Some(20.0), 72 * HOUR),
            weekly(Some("max"), 8 * HOUR, Some(5.0), 72 * HOUR),
            weekly(Some("pro"), 7 * HOUR, Some(8.0), 72 * HOUR),
            weekly(Some("pro"), 6 * HOUR, Some(9.0), 80 * HOUR),
            weekly(Some("pro"), 5 * HOUR, None, 80 * HOUR),
            weekly(Some("pro"), 4 * HOUR, Some(12.0), 80 * HOUR),
            weekly(Some("pro"), 3 * HOUR, Some(13.0), 80 * HOUR),
            raw(
                Some("pro"),
                WindowId::Session,
                NOW - 3 * HOUR,
                Some(50.0),
                NOW - 2 * HOUR,
            ),
            raw(
                Some("pro"),
                WindowId::Session,
                NOW - HOUR,
                Some(60.0),
                NOW + 3 * HOUR,
            ),
        ];
        let histories = observed_burn(&readings, NOW);
        let shape = |window: WindowId| -> Vec<(Boundary, usize)> {
            histories
                .iter()
                .find(|history| history.window == window)
                .expect("the window has a history")
                .segments
                .iter()
                .map(|segment| (segment.opened_by, segment.readings))
                .collect()
        };
        assert_eq!(
            shape(WindowId::Weekly),
            vec![
                (Boundary::First, 2),
                (Boundary::Refill, 1),
                (Boundary::PlanChange, 1),
                (Boundary::ResetMoved, 1),
                (Boundary::Discontinuity, 2),
            ]
        );
        assert_eq!(
            shape(WindowId::Session),
            vec![(Boundary::First, 1), (Boundary::Refill, 1)]
        );
    }

    #[test]
    fn the_forecast_uses_only_the_segment_after_the_last_boundary() {
        let mut readings = vec![
            reading(WindowId::Weekly, 3 * HOUR, 80.0, 72 * HOUR),
            reading(WindowId::Weekly, 2 * HOUR, 90.0, 72 * HOUR),
            reading(WindowId::Weekly, HOUR, 2.0, 72 * HOUR),
            reading(WindowId::Weekly, 0, 4.0, 72 * HOUR),
        ];
        readings.extend(burning(WindowId::Session, 0.0, 0.0, 5 * HOUR));
        let WindowState::Known(forecast) = state(&states(readings), WindowId::Weekly) else {
            panic!("the weekly window is forecast");
        };
        assert!(close(forecast.percent_per_hour, 2.0), "{forecast:#?}");
        assert!(close(forecast.used_percent, 4.0), "{forecast:#?}");
        assert_eq!(forecast.since_unix, NOW - HOUR);
        assert_eq!(forecast.readings, 2);
    }

    #[test]
    fn a_current_forecast_extrapolates_the_observed_rate_to_now() {
        let mut readings = vec![
            reading(WindowId::Weekly, 90 * 60, 20.0, 10 * HOUR),
            reading(WindowId::Weekly, 30 * 60, 30.0, 10 * HOUR),
        ];
        readings.extend(burning(WindowId::Session, 0.0, 0.0, 5 * HOUR));
        let WindowState::Known(forecast) = state(&states(readings), WindowId::Weekly) else {
            panic!("the weekly window is forecast");
        };
        assert_eq!(forecast.plan, plan_id("max"));
        assert!(close(forecast.percent_per_hour, 10.0), "{forecast:#?}");
        assert!(close(forecast.used_percent, 35.0), "{forecast:#?}");
        assert_eq!(forecast.refill_unix, NOW + 10 * HOUR);
        assert_eq!(forecast.exhaustion_unix, Some(NOW + 23_400));
        let WindowState::Known(idle) = state(&states(filling_weekly(HOUR)), WindowId::Session)
        else {
            panic!("the idle session window is forecast");
        };
        assert_eq!(idle.exhaustion_unix, None);
    }

    #[test]
    fn missing_contradictory_or_stale_evidence_yields_no_forecast() {
        let session = || burning(WindowId::Session, 0.0, 0.0, 5 * HOUR);
        let weekly_state = |weekly: Vec<Reading>| {
            let mut readings = session();
            readings.extend(weekly);
            state(&states(readings), WindowId::Weekly)
        };
        let cases: Vec<(Vec<Reading>, WindowUnknown)> = vec![
            (Vec::new(), WindowUnknown::NoReading),
            (
                vec![reading(WindowId::Weekly, 0, 10.0, HOUR)],
                WindowUnknown::InsufficientHistory,
            ),
            (
                vec![
                    reading(WindowId::Weekly, HOUR, 10.0, HOUR),
                    raw(Some("max"), WindowId::Weekly, NOW, None, NOW + HOUR),
                ],
                WindowUnknown::Incomplete,
            ),
            (
                vec![
                    reading(WindowId::Weekly, HOUR, 10.0, HOUR),
                    raw(Some("max"), WindowId::Weekly, NOW, Some(20.0), NOW),
                ],
                WindowUnknown::Incomplete,
            ),
            (
                vec![
                    reading(WindowId::Weekly, HOUR, 10.0, HOUR),
                    reading(WindowId::Weekly, 0, 12.0, HOUR),
                    reading(WindowId::Weekly, 0, 14.0, HOUR),
                ],
                WindowUnknown::Contradictory,
            ),
            (
                vec![
                    reading(WindowId::Weekly, HOUR, 10.0, HOUR),
                    raw(
                        Some("max"),
                        WindowId::Weekly,
                        NOW + 60,
                        Some(12.0),
                        NOW + HOUR,
                    ),
                ],
                WindowUnknown::FromFuture,
            ),
            (
                vec![
                    reading(WindowId::Weekly, 3 * HOUR, 10.0, HOUR),
                    reading(WindowId::Weekly, 2 * HOUR, 12.0, HOUR),
                ],
                WindowUnknown::Stale,
            ),
            (
                vec![
                    raw(
                        Some("max"),
                        WindowId::Weekly,
                        NOW - HOUR,
                        Some(10.0),
                        NOW - 60,
                    ),
                    raw(
                        Some("max"),
                        WindowId::Weekly,
                        NOW - 30 * 60,
                        Some(12.0),
                        NOW - 60,
                    ),
                ],
                WindowUnknown::Refilled,
            ),
            (
                vec![
                    reading_on("pro", WindowId::Weekly, 2 * HOUR, 10.0, 9 * HOUR),
                    reading_on("pro", WindowId::Weekly, HOUR, 12.0, 9 * HOUR),
                ],
                WindowUnknown::PlanChanged,
            ),
        ];
        for (weekly, expected) in cases {
            let described = format!("{weekly:#?}");
            assert_eq!(
                weekly_state(weekly),
                WindowState::Unknown(expected),
                "{described}"
            );
        }

        let duplicated = vec![
            reading(WindowId::Weekly, HOUR, 10.0, HOUR),
            reading(WindowId::Weekly, 0, 12.0, HOUR),
            reading(WindowId::Weekly, 0, 12.0, HOUR),
        ];
        let WindowState::Known(forecast) = weekly_state(duplicated) else {
            panic!("identical readings of one second are one reading");
        };
        assert_eq!(forecast.readings, 2);

        let contradicted = only_plan(&input(
            Provider::Claude,
            vec![
                reading_on("max", WindowId::Session, 0, 1.0, HOUR),
                reading_on("pro", WindowId::Weekly, 0, 1.0, HOUR),
            ],
            Vec::new(),
            &[],
        ));
        for window in [WindowId::Session, WindowId::Weekly] {
            assert_eq!(
                state(&contradicted, window),
                WindowState::Unknown(WindowUnknown::PlanContradiction)
            );
        }

        let mut unlabeled = filling_weekly(72 * HOUR);
        unlabeled.extend(burning(
            WindowId::Model(ModelLabel::Unknown),
            5.0,
            1.0,
            9 * HOUR,
        ));
        assert_eq!(
            state(&states(unlabeled), WindowId::Model(ModelLabel::Unknown)),
            WindowState::Unknown(WindowUnknown::UnlabeledModel)
        );
    }

    #[test]
    fn readings_of_different_accounts_or_windows_never_share_a_segment() {
        let mut other = reading(WindowId::Weekly, 0, 50.0, 72 * HOUR);
        other.account = AccountId("other".to_owned());
        let readings = [
            reading(WindowId::Weekly, HOUR, 10.0, 72 * HOUR),
            other,
            reading(WindowId::Session, 0, 20.0, 4 * HOUR),
        ];
        let histories = observed_burn(&readings, NOW);
        assert_eq!(histories.len(), 3);
        for history in &histories {
            let [segment] = history.segments.as_slice() else {
                panic!("one segment each, got {history:#?}");
            };
            assert_eq!(segment.readings, 1);
            assert_eq!(segment.percent_per_hour(), None);
        }
    }

    #[test]
    fn research_takes_its_parents_priority_and_features_override_projects() {
        let feature = |id: &str, project: &str, feature: Option<&str>| Run {
            id: run_id(id),
            purpose: Purpose::Feature {
                project: project.to_owned(),
                feature: feature.map(str::to_owned),
            },
        };
        let research = |id: &str, parent: &str| Run {
            id: run_id(id),
            purpose: Purpose::ResearchFor {
                parent: run_id(parent),
            },
        };
        let runs = [
            feature("login", "alpha", Some("login")),
            feature("plain", "alpha", None),
            feature("other", "gamma", None),
            research("research", "login"),
            research("nested", "research"),
            research("orphan", "gone"),
            research("loop-a", "loop-b"),
            research("loop-b", "loop-a"),
            feature("twice", "alpha", None),
            feature("twice", "gamma", None),
        ];
        let demand: Vec<Demand> = [
            ("other", 1.0),
            ("plain", 1.0),
            ("research", 1.0),
            ("orphan", 1.0),
            ("nested", 1.0),
            ("login", 1.0),
            ("loop-a", 1.0),
            ("twice", 1.0),
            ("absent", 1.0),
            ("login", f64::NAN),
            ("plain", 0.0),
        ]
        .into_iter()
        .map(|(run, hours)| Demand {
            run: run_id(run),
            account: operator(),
            hours,
            model: ModelLabel::Unknown,
        })
        .collect();
        let priorities = Priorities {
            projects: BTreeMap::from([("alpha".to_owned(), 3)]),
            features: BTreeMap::from([(
                "alpha".to_owned(),
                BTreeMap::from([("login".to_owned(), 9)]),
            )]),
        };
        let ranked: Vec<(String, Rank)> = rank_demand(&runs, &demand, &priorities)
            .into_iter()
            .map(|item| (item.run.0, item.rank))
            .collect();
        let expected: Vec<(String, Rank)> = [
            ("login", Rank::Priority(9)),
            ("nested", Rank::Priority(9)),
            ("research", Rank::Priority(9)),
            ("plain", Rank::Priority(3)),
            ("other", Rank::Unprioritized),
            ("absent", Rank::Invalid(DemandProblem::RunMissing)),
            ("login", Rank::Invalid(DemandProblem::InvalidHours)),
            ("loop-a", Rank::Invalid(DemandProblem::ParentCycle)),
            ("orphan", Rank::Invalid(DemandProblem::ParentMissing)),
            ("plain", Rank::Invalid(DemandProblem::InvalidHours)),
            ("twice", Rank::Invalid(DemandProblem::RunConflict)),
        ]
        .into_iter()
        .map(|(run, rank)| (run.to_owned(), rank))
        .collect();
        assert_eq!(ranked, expected);
    }

    #[test]
    fn demand_that_cannot_be_placed_leaves_the_accounts_grants_undecided() {
        let mut orphaned = claude(
            exhausted_weekly(),
            usable(GrantKind::Weekly, 240 * HOUR),
            &one_job(4.0),
        );
        orphaned.runs.push(Run {
            id: run_id("helper"),
            purpose: Purpose::ResearchFor {
                parent: run_id("gone"),
            },
        });
        orphaned.demand.push(Demand {
            run: run_id("helper"),
            account: operator(),
            hours: 1.0,
            model: ModelLabel::Unknown,
        });
        assert_eq!(
            decision(&orphaned),
            Decision::Unknown(Undecided::Demand(DemandProblem::ParentMissing))
        );
    }

    #[test]
    fn an_exhausted_window_with_waiting_work_is_redeemed_now() {
        let scene = claude(
            exhausted_weekly(),
            usable(GrantKind::Weekly, 240 * HOUR),
            &one_job(4.0),
        );
        assert_eq!(decision(&scene), Decision::Redeem);
        assert!(!only_plan(&scene).rests_on_fixture);
    }

    #[test]
    fn redeeming_at_forecast_exhaustion_beats_redeeming_now() {
        let scene = claude(
            filling_weekly(72 * HOUR),
            usable(GrantKind::Weekly, 240 * HOUR),
            &one_job(100.0),
        );
        assert_eq!(
            decision(&scene),
            Decision::Defer {
                until_unix: NOW + 20 * HOUR
            }
        );
    }

    #[test]
    fn a_window_that_refills_before_it_exhausts_lets_the_grant_expire() {
        let scene = claude(
            filling_weekly(15 * HOUR),
            usable(GrantKind::Weekly, 240 * HOUR),
            &one_job(25.0),
        );
        assert_eq!(
            decision(&scene),
            Decision::LetExpire(ExpireReason::NoUnlock)
        );
    }

    #[test]
    fn no_runnable_demand_lets_a_redeemable_grant_expire() {
        let scene = claude(
            exhausted_weekly(),
            usable(GrantKind::Weekly, 240 * HOUR),
            &[],
        );
        assert_eq!(
            decision(&scene),
            Decision::LetExpire(ExpireReason::NoUnlock)
        );
    }

    #[test]
    fn each_of_demand_expiry_and_refill_can_change_the_recommendation() {
        let scene = |resets_in: u64, ends_in: u64, hours: f64| {
            decision(&claude(
                filling_weekly(resets_in),
                usable(GrantKind::Weekly, ends_in),
                &one_job(hours),
            ))
        };
        assert_eq!(
            scene(72 * HOUR, 240 * HOUR, 25.0),
            Decision::Defer {
                until_unix: NOW + 20 * HOUR
            }
        );
        assert_eq!(
            scene(72 * HOUR, 240 * HOUR, 10.0),
            Decision::LetExpire(ExpireReason::NoUnlock),
            "less demand"
        );
        assert_eq!(
            scene(15 * HOUR, 240 * HOUR, 25.0),
            Decision::LetExpire(ExpireReason::NoUnlock),
            "an earlier refill"
        );
        assert_eq!(
            scene(72 * HOUR, 10 * HOUR, 25.0),
            Decision::Defer {
                until_unix: NOW + 10 * HOUR - 1
            },
            "an expiry before the forecast exhaustion"
        );
        assert_eq!(
            scene(72 * HOUR, 1, 25.0),
            Decision::Redeem,
            "an expiry one second away"
        );
    }

    #[test]
    fn another_exhausted_window_defers_the_redemption_until_it_refills() {
        let grant = usable(GrantKind::Weekly, 240 * HOUR);
        assert_eq!(
            decision(&claude(exhausted_weekly(), grant.clone(), &one_job(4.0))),
            Decision::Redeem
        );
        let mut limited = burning(WindowId::Weekly, 100.0, 2.0, 72 * HOUR);
        limited.extend(burning(WindowId::Session, 100.0, 10.0, 2 * HOUR));
        assert_eq!(
            decision(&claude(limited, grant, &one_job(4.0))),
            Decision::Defer {
                until_unix: NOW + 2 * HOUR
            }
        );
    }

    #[test]
    fn the_restored_allowance_is_spent_after_the_grant_expires() {
        let mut readings = burning(WindowId::Weekly, 90.0, 10.0, 72 * HOUR);
        readings.extend(burning(WindowId::Session, 0.0, 0.0, 5 * HOUR));
        let scene = claude(readings, usable(GrantKind::Weekly, 30 * 60), &one_job(5.0));
        assert_eq!(
            decision(&scene),
            Decision::Defer {
                until_unix: NOW + 30 * 60 - 1
            }
        );
        let before_any_limit = claude(
            filling_weekly(72 * HOUR),
            usable(GrantKind::Weekly, HOUR),
            &one_job(25.0),
        );
        assert_eq!(
            decision(&before_any_limit),
            Decision::Defer {
                until_unix: NOW + HOUR - 1
            }
        );
    }

    #[test]
    fn an_unredeemable_grant_is_left_to_expire_whatever_the_forecast() {
        let expired = Grant {
            ends_at_unix: Some(NOW),
            ..usable(GrantKind::Weekly, 240 * HOUR)
        };
        let spent = Grant {
            resets_left: Some(0),
            ..usable(GrantKind::Weekly, 240 * HOUR)
        };
        let scene = input(
            Provider::Claude,
            exhausted_weekly(),
            vec![expired, spent, usable(GrantKind::Weekly, 240 * HOUR)],
            &one_job(4.0),
        );
        let not_redeemable = Decision::LetExpire(ExpireReason::NotRedeemable);
        assert_eq!(
            decisions(&scene),
            vec![not_redeemable, not_redeemable, Decision::Redeem]
        );
    }

    #[test]
    fn only_the_first_expiring_of_several_redeemable_grants_is_redeemed_before_a_re_read() {
        let scene = input(
            Provider::Claude,
            exhausted_weekly(),
            vec![
                usable(GrantKind::Weekly, 240 * HOUR),
                usable(GrantKind::Weekly, 100 * HOUR),
                Grant {
                    availability: Availability::Unknown,
                    ..usable(GrantKind::Weekly, 50 * HOUR)
                },
            ],
            &one_job(4.0),
        );
        assert_eq!(
            decisions(&scene),
            vec![
                Decision::Unknown(Undecided::AwaitsReRead),
                Decision::Redeem,
                Decision::Unknown(Undecided::Availability),
            ]
        );
    }

    #[test]
    fn unknown_availability_or_windows_leave_a_grant_undecided() {
        let grant = usable(GrantKind::Weekly, 240 * HOUR);
        let unknown = Grant {
            availability: Availability::Unknown,
            ..grant.clone()
        };
        let uncounted = Grant {
            resets_left: None,
            ..grant.clone()
        };
        for undecided in [unknown, uncounted] {
            assert_eq!(
                decision(&claude(exhausted_weekly(), undecided, &one_job(4.0))),
                Decision::Unknown(Undecided::Availability)
            );
        }
        let mut thin = burning(WindowId::Weekly, 100.0, 2.0, 72 * HOUR);
        thin.push(reading(WindowId::Session, 0, 10.0, 4 * HOUR));
        assert_eq!(
            decision(&claude(thin, grant.clone(), &one_job(4.0))),
            Decision::Unknown(Undecided::Window(
                WindowId::Session,
                WindowUnknown::InsufficientHistory
            ))
        );
        let session_only = burning(WindowId::Session, 10.0, 10.0, 4 * HOUR);
        assert_eq!(
            decision(&claude(session_only, grant, &one_job(4.0))),
            Decision::Unknown(Undecided::Window(
                WindowId::Weekly,
                WindowUnknown::NoReading
            ))
        );
    }

    #[test]
    fn unknown_inventory_is_not_known_empty() {
        let mut scene = claude(
            exhausted_weekly(),
            usable(GrantKind::Weekly, 240 * HOUR),
            &one_job(4.0),
        );
        let mut grants_with = |inventory: InventoryEvidence| {
            if let Some(account) = scene.accounts.first_mut() {
                account.inventory = inventory;
            }
            only_plan(&scene).grants
        };
        assert_eq!(
            grants_with(InventoryEvidence::Unknown),
            Grants::Unknown(InventoryUnknown::NotRead)
        );
        assert_eq!(grants_with(inventory(Vec::new())), Grants::KnownEmpty);
        let observed = |observed_at_unix: u64| {
            InventoryEvidence::Known(Inventory {
                observed_at_unix,
                grants: Vec::new(),
                source: Source::Reader,
            })
        };
        assert_eq!(
            grants_with(observed(NOW - 2 * HOUR)),
            Grants::Unknown(InventoryUnknown::Stale)
        );
        assert_eq!(
            grants_with(observed(NOW + 1)),
            Grants::Unknown(InventoryUnknown::FromFuture)
        );
        let mut twice = claude(
            exhausted_weekly(),
            usable(GrantKind::Weekly, 240 * HOUR),
            &one_job(4.0),
        );
        twice.accounts.extend(twice.accounts.clone());
        for planned in plan(&twice) {
            assert_eq!(planned.grants, Grants::Unknown(InventoryUnknown::Conflict));
        }
    }

    #[test]
    fn an_unknown_effect_decides_only_where_both_bounds_agree() {
        let weekly = usable(GrantKind::Weekly, 240 * HOUR);
        let mut idle_model = exhausted_weekly();
        idle_model.extend(burning(
            WindowId::Model(ModelLabel::Fable),
            5.0,
            0.0,
            100 * HOUR,
        ));
        assert_eq!(
            decision(&claude(idle_model, weekly.clone(), &one_job(4.0))),
            Decision::Redeem
        );
        let mut binding_model = exhausted_weekly();
        binding_model.extend(burning(
            WindowId::Model(ModelLabel::Fable),
            100.0,
            1.0,
            100 * HOUR,
        ));
        let mut binding = claude(binding_model, weekly, &one_job(4.0));
        for item in &mut binding.demand {
            item.model = ModelLabel::Fable;
        }
        assert_eq!(decision(&binding), Decision::Unknown(Undecided::Effect));
        let unknown_kind = usable(GrantKind::Unknown, 240 * HOUR);
        assert_eq!(
            decision(&claude(
                exhausted_weekly(),
                unknown_kind.clone(),
                &one_job(4.0)
            )),
            Decision::Unknown(Undecided::Effect)
        );
        assert_eq!(
            decision(&claude(exhausted_weekly(), unknown_kind, &[])),
            Decision::LetExpire(ExpireReason::NoUnlock)
        );
    }

    #[test]
    fn competing_priorities_change_the_recommendation() {
        let mut readings = burning(WindowId::Session, 100.0, 25.0, HOUR);
        readings.extend(burning(WindowId::Weekly, 0.0, 0.0, 100 * HOUR));
        let grant = usable(GrantKind::Session, 48 * HOUR);
        let short_first = claude(
            readings.clone(),
            grant.clone(),
            &[("alpha", 10, 2.0), ("beta", 1, 10.0)],
        );
        assert_eq!(decision(&short_first), Decision::Redeem);
        let long_first = claude(readings, grant, &[("alpha", 1, 2.0), ("beta", 10, 10.0)]);
        let queue: Vec<RunId> = only_plan(&long_first)
            .queue
            .into_iter()
            .map(|item| item.run)
            .collect();
        assert_eq!(queue, vec![run_id("beta"), run_id("alpha")]);
        assert_eq!(
            decision(&long_first),
            Decision::Defer {
                until_unix: NOW + 10 * HOUR
            }
        );
    }

    #[test]
    fn a_restart_that_replaces_an_imminent_refill_defers_where_a_kept_clock_redeems() {
        let mut readings = burning(WindowId::Weekly, 100.0, 50.0, HOUR);
        readings.extend(burning(WindowId::Session, 0.0, 0.0, 3 * HOUR));
        let grant = usable(GrantKind::Weekly, 48 * HOUR);
        assert_eq!(
            decision(&claude(readings.clone(), grant.clone(), &one_job(3.0))),
            Decision::Redeem
        );
        let mut fixture = input(
            Provider::Codex,
            on_plan("pro", readings),
            vec![grant],
            &one_job(3.0),
        );
        for account in &mut fixture.accounts {
            account.coverage = Some(codex_covered_allowances());
            if let InventoryEvidence::Known(inventory) = &mut account.inventory {
                inventory.source = Source::Fixture;
            }
        }
        assert_eq!(
            decision(&fixture),
            Decision::Defer {
                until_unix: NOW + 3 * HOUR
            }
        );
        assert!(only_plan(&fixture).rests_on_fixture);
    }

    #[test]
    fn reset_effects_are_rules_per_provider_grant_and_window() {
        let fable = WindowId::Model(ModelLabel::Fable);
        let sonnet = WindowId::Model(ModelLabel::Sonnet);
        let claude_rules = [
            (GrantKind::Weekly, WindowId::Weekly, Effect::Restores),
            (GrantKind::Weekly, WindowId::Session, Effect::Unchanged),
            (GrantKind::Weekly, sonnet, Effect::Unknown),
            (GrantKind::Session, WindowId::Session, Effect::Restores),
            (GrantKind::Session, WindowId::Weekly, Effect::Unchanged),
            (GrantKind::Full, WindowId::Weekly, Effect::Restores),
            (GrantKind::Full, WindowId::Session, Effect::Restores),
            (GrantKind::Fable, fable, Effect::Restores),
            (GrantKind::Fable, WindowId::Session, Effect::Restores),
            (GrantKind::Fable, WindowId::Weekly, Effect::Unknown),
            (GrantKind::Fable, sonnet, Effect::Unknown),
            (GrantKind::Unknown, WindowId::Weekly, Effect::Unknown),
        ];
        for (kind, window, expected) in claude_rules {
            assert_eq!(
                effect(Provider::Claude, kind, window),
                expected,
                "{kind:?} on {window:?}"
            );
        }
        assert_eq!(
            effect(Provider::Codex, GrantKind::Weekly, WindowId::Weekly),
            Effect::RestoresBelowFull
        );
        assert_eq!(
            effect(Provider::Codex, GrantKind::Fable, fable),
            Effect::Unknown
        );
        assert_eq!(
            schedule_after_restore(Provider::Claude, WindowId::Weekly),
            Schedule::Kept
        );
        for (provider, window) in [
            (Provider::Claude, WindowId::Session),
            (Provider::Claude, fable),
            (Provider::Codex, WindowId::Weekly),
        ] {
            assert_eq!(
                schedule_after_restore(provider, window),
                Schedule::MustReRead
            );
        }
    }

    #[test]
    fn the_plan_is_deterministic_and_ignores_reading_order() {
        let scene = claude(
            filling_weekly(72 * HOUR),
            usable(GrantKind::Weekly, 240 * HOUR),
            &one_job(100.0),
        );
        let mut reversed = scene.clone();
        reversed.readings.reverse();
        assert_eq!(plan(&scene), plan(&scene));
        assert_eq!(plan(&scene), plan(&reversed));
    }

    #[test]
    fn unknown_post_reset_schedule_cannot_prove_no_unlock() {
        let mut readings = burning(WindowId::Session, 100.0, 25.0, 3 * HOUR);
        readings.extend(burning(WindowId::Weekly, 30.0, 1.0, 72 * HOUR));
        let grant = usable(GrantKind::Session, 30 * 60);
        assert_eq!(
            decision(&claude(readings.clone(), grant.clone(), &one_job(6.0))),
            Decision::Unknown(Undecided::Schedule)
        );
        assert_eq!(
            decision(&claude(readings, grant.clone(), &one_job(3.0))),
            Decision::Redeem
        );
        let mut unbound = burning(WindowId::Session, 10.0, 10.0, 4 * HOUR);
        unbound.extend(burning(WindowId::Weekly, 30.0, 1.0, 72 * HOUR));
        assert_eq!(
            decision(&claude(unbound, grant, &one_job(2.0))),
            Decision::LetExpire(ExpireReason::NoUnlock)
        );
    }

    #[test]
    fn temporary_unavailability_cannot_establish_grant_will_expire_unusable() {
        let read = |observed_at_unix: u64, availability: Availability| {
            let grant = Grant {
                availability,
                ..usable(GrantKind::Weekly, 240 * HOUR)
            };
            let mut scene = claude(exhausted_weekly(), grant, &one_job(4.0));
            for account in &mut scene.accounts {
                if let InventoryEvidence::Known(inventory) = &mut account.inventory {
                    inventory.observed_at_unix = observed_at_unix;
                }
            }
            decision(&scene)
        };
        assert_eq!(
            read(NOW - 1, Availability::NotUsable),
            Decision::Unknown(Undecided::Availability)
        );
        assert_eq!(read(NOW, Availability::Usable), Decision::Redeem);
    }

    #[test]
    fn weekly_refills_passed_in_one_wait_count_against_the_step_bound() {
        let week = 7 * 24 * HOUR;
        let waiting = |weeks: u64| {
            let mut readings = burning(WindowId::Session, 100.0, 25.0, weeks * week);
            readings.extend(burning(WindowId::Weekly, 30.0, 1.0, 72 * HOUR));
            decision(&claude(
                readings,
                usable(GrantKind::Weekly, 48 * HOUR),
                &one_job(1.0),
            ))
        };
        assert_eq!(waiting(2), Decision::LetExpire(ExpireReason::NoUnlock));
        let horizon = u64::try_from(MAX_STEPS).expect("the step bound fits in u64") + 1;
        assert_eq!(waiting(horizon), Decision::Unknown(Undecided::Bound));
    }

    const FABLE: WindowId = WindowId::Model(ModelLabel::Fable);

    fn claude_fable_exhausted() -> Vec<Reading> {
        let mut readings = burning(FABLE, 100.0, 10.0, 50 * HOUR);
        readings.extend(burning(WindowId::Session, 20.0, 10.0, 3 * HOUR));
        readings.extend(burning(WindowId::Weekly, 40.0, 1.0, 72 * HOUR));
        readings
    }

    fn codex_covered_allowances() -> BTreeMap<PlanId, BTreeSet<WindowId>> {
        BTreeMap::from([
            (plan_id("plus"), BTreeSet::from([WindowId::Weekly])),
            (
                plan_id("pro"),
                BTreeSet::from([WindowId::Session, WindowId::Weekly]),
            ),
        ])
    }

    fn on_plan(plan: &str, readings: Vec<Reading>) -> Vec<Reading> {
        readings
            .into_iter()
            .map(|reading| Reading {
                plan: Some(plan_id(plan)),
                source: Source::Fixture,
                ..reading
            })
            .collect()
    }

    #[test]
    fn recommendation_respects_codex_plan_coverage() {
        let mut readings = burning(WindowId::Session, 100.0, 25.0, 2 * HOUR);
        readings.extend(burning(WindowId::Weekly, 0.0, 0.0, 100 * HOUR));
        let mut scene = input(
            Provider::Codex,
            on_plan("plus", readings),
            vec![usable(GrantKind::Full, 48 * HOUR)],
            &one_job(1.0),
        );
        for account in &mut scene.accounts {
            account.coverage = Some(codex_covered_allowances());
        }
        assert_eq!(
            decision(&scene),
            Decision::LetExpire(ExpireReason::NoUnlock)
        );
    }

    fn fable_grant_for(model: ModelLabel) -> Decision {
        let mut scene = claude(
            claude_fable_exhausted(),
            usable(GrantKind::Fable, 48 * HOUR),
            &one_job(1.0),
        );
        for item in &mut scene.demand {
            item.model = model;
        }
        decision(&scene)
    }

    #[test]
    fn fable_grant_cannot_unlock_already_runnable_sonnet_work() {
        assert_eq!(
            fable_grant_for(ModelLabel::Sonnet),
            Decision::LetExpire(ExpireReason::NoUnlock)
        );
        assert_eq!(fable_grant_for(ModelLabel::Fable), Decision::Redeem);
    }

    #[test]
    fn unidentified_work_does_not_prove_model_demand() {
        assert_eq!(
            fable_grant_for(ModelLabel::Sonnet),
            Decision::LetExpire(ExpireReason::NoUnlock)
        );
        assert_eq!(fable_grant_for(ModelLabel::Fable), Decision::Redeem);
        assert_eq!(
            fable_grant_for(ModelLabel::Unknown),
            Decision::Unknown(Undecided::Model)
        );
    }

    #[test]
    fn unrelated_model_reading() {
        let scene = |model: ModelLabel, fable_read: bool| {
            let mut readings = exhausted_weekly();
            if fable_read {
                readings.push(reading(FABLE, 0, 50.0, 100 * HOUR));
            }
            let mut built = claude(
                readings,
                usable(GrantKind::Weekly, 240 * HOUR),
                &one_job(4.0),
            );
            for item in &mut built.demand {
                item.model = model;
            }
            built
        };
        assert_eq!(
            decision(&scene(ModelLabel::Sonnet, false)),
            Decision::Redeem
        );
        let sonnet = scene(ModelLabel::Sonnet, true);
        assert_eq!(
            state(&only_plan(&sonnet), FABLE),
            WindowState::Unknown(WindowUnknown::InsufficientHistory)
        );
        assert_eq!(decision(&sonnet), Decision::Redeem);
        let unread =
            Decision::Unknown(Undecided::Window(FABLE, WindowUnknown::InsufficientHistory));
        for model in [ModelLabel::Fable, ModelLabel::Unknown] {
            assert_eq!(decision(&scene(model, true)), unread, "{model:?}");
        }
    }

    #[test]
    fn subsecond_exhaustion_is_rounded_before_comparison() {
        let per_hour = 60.0;
        let mut readings = burning(
            WindowId::Weekly,
            FULL_PERCENT - per_hour * 0.5 / SECS_PER_HOUR,
            per_hour,
            72 * HOUR,
        );
        readings.extend(burning(WindowId::Session, 0.0, 0.0, 5 * HOUR));
        let scene = claude(
            readings,
            usable(GrantKind::Weekly, 240 * HOUR),
            &one_job(6_000.5 / SECS_PER_HOUR),
        );
        let WindowState::Known(weekly) = state(&only_plan(&scene), WindowId::Weekly) else {
            panic!("the weekly window is forecast");
        };
        assert_eq!(weekly.exhaustion_unix, Some(NOW + 1));
        assert_eq!(
            decision(&scene),
            Decision::Defer {
                until_unix: NOW + 1
            }
        );
    }

    #[test]
    fn an_intervening_account_plan_change_breaks_window_rate_history() {
        let across = |session_plan: &str| {
            vec![
                reading_on("max", WindowId::Weekly, 2 * HOUR, 10.0, 72 * HOUR),
                reading_on(session_plan, WindowId::Session, HOUR, 5.0, 4 * HOUR),
                reading_on("max", WindowId::Weekly, 0, 30.0, 72 * HOUR),
            ]
        };
        let weekly_shape = |readings: &[Reading]| -> Vec<(Boundary, usize)> {
            observed_burn(readings, NOW)
                .into_iter()
                .find(|history| history.window == WindowId::Weekly)
                .expect("the weekly window has a history")
                .segments
                .iter()
                .map(|segment| (segment.opened_by, segment.readings))
                .collect()
        };

        let unchanged = across("max");
        assert_eq!(weekly_shape(&unchanged), vec![(Boundary::First, 2)]);
        let WindowState::Known(forecast) = state(&states(unchanged), WindowId::Weekly) else {
            panic!("one plan throughout gives a weekly rate");
        };
        assert!(close(forecast.percent_per_hour, 10.0), "{forecast:#?}");

        let changed = across("pro");
        assert_eq!(
            weekly_shape(&changed),
            vec![(Boundary::First, 1), (Boundary::PlanChange, 1)]
        );
        assert_eq!(
            state(&states(changed), WindowId::Weekly),
            WindowState::Unknown(WindowUnknown::InsufficientHistory)
        );
    }

    #[test]
    fn plan_roundtrip_without_weekly_reread() {
        let scene = |between: &str| {
            let readings = vec![
                reading_on("max", WindowId::Weekly, 50 * 60, 98.0, 72 * HOUR),
                reading_on("max", WindowId::Weekly, 40 * 60, 100.0, 72 * HOUR),
                reading_on(between, WindowId::Session, 30 * 60, 5.0, 4 * HOUR),
                reading_on("max", WindowId::Session, 20 * 60, 5.0, 4 * HOUR),
                reading_on("max", WindowId::Session, 0, 10.0, 4 * HOUR),
            ];
            claude(
                readings,
                usable(GrantKind::Weekly, 240 * HOUR),
                &one_job(4.0),
            )
        };

        let unchanged = scene("max");
        assert!(matches!(
            state(&only_plan(&unchanged), WindowId::Weekly),
            WindowState::Known(_)
        ));
        assert_eq!(decision(&unchanged), Decision::Redeem);

        let roundtrip = scene("pro");
        let planned = only_plan(&roundtrip);
        assert!(matches!(
            state(&planned, WindowId::Session),
            WindowState::Known(_)
        ));
        assert_eq!(
            state(&planned, WindowId::Weekly),
            WindowState::Unknown(WindowUnknown::PlanChanged)
        );
        assert_eq!(
            decision(&roundtrip),
            Decision::Unknown(Undecided::Window(
                WindowId::Weekly,
                WindowUnknown::PlanChanged
            ))
        );
    }

    #[test]
    fn no_codex_coverage() {
        let mut readings = burning(WindowId::Session, 100.0, 25.0, 2 * HOUR);
        readings.extend(burning(WindowId::Weekly, 0.0, 0.0, 100 * HOUR));
        let covered_by = |coverage: Option<BTreeMap<PlanId, BTreeSet<WindowId>>>| {
            let mut scene = input(
                Provider::Codex,
                on_plan("plus", readings.clone()),
                vec![usable(GrantKind::Full, 48 * HOUR)],
                &one_job(1.0),
            );
            for account in &mut scene.accounts {
                account.coverage.clone_from(&coverage);
            }
            decision(&scene)
        };
        assert_eq!(
            covered_by(Some(codex_covered_allowances())),
            Decision::LetExpire(ExpireReason::NoUnlock)
        );
        assert_eq!(covered_by(None), Decision::Unknown(Undecided::Effect));
        for window in [WindowId::Session, WindowId::Weekly] {
            assert_eq!(
                covered_effect(
                    Provider::Codex,
                    GrantKind::Full,
                    window,
                    None,
                    Some(&plan_id("plus"))
                ),
                Effect::Unknown
            );
            assert_eq!(
                covered_effect(
                    Provider::Claude,
                    GrantKind::Full,
                    window,
                    None,
                    Some(&plan_id("max"))
                ),
                Effect::Restores
            );
        }
    }

    #[test]
    fn plan_history_comparison_count() {
        let examined = |count: u64| {
            let readings: Vec<Reading> = (0..count)
                .map(|index| reading(WindowId::Weekly, count - index, 10.0, 72 * HOUR))
                .collect();
            let refs: Vec<&Reading> = readings.iter().collect();
            let log = plan_log(&refs);
            let visits = Cell::new(0_usize);
            let (segments, latest) = history(
                refs,
                |from, to| {
                    let named = named_between(&log, from, to);
                    visits.set(visits.get() + named.len());
                    named
                },
                NOW,
            );
            assert_eq!(latest, Latest::Usable);
            let [segment] = segments.as_slice() else {
                panic!("one unchanged plan is one segment, got {segments:#?}");
            };
            assert_eq!(segment.readings, readings.len());
            visits.get()
        };
        assert_eq!(examined(128), 2 * 127);
        assert_eq!(examined(256), 2 * 255);
    }
}
