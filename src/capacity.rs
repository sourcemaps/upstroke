//! Extended notes: `docs/internals/capacity.md`

#![forbid(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros
)]

use std::collections::BTreeMap;
use std::fmt;
use std::time::Duration;

use crate::events::{AttemptRecord, Event, EventBody, PoolExhausted, ReviewPassOutcome};
use crate::ladder::{AttemptFailure, FailureKind, FailureOrigin};
use crate::topology::events::{TopologyEvent, TopologyEventBody};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolKind {
    SubscriptionWindow,
    Credits,
    RequestPool,
    ApiKey,
    Unmetered,
}

impl PoolKind {
    pub const ACCEPTED: [&'static str; 5] = [
        "subscription-window",
        "credits",
        "request-pool",
        "api-key",
        "unmetered",
    ];

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "subscription-window" => Some(Self::SubscriptionWindow),
            "credits" => Some(Self::Credits),
            "request-pool" => Some(Self::RequestPool),
            "api-key" => Some(Self::ApiKey),
            "unmetered" => Some(Self::Unmetered),
            _ => None,
        }
    }
}

impl fmt::Display for PoolKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::SubscriptionWindow => "subscription-window",
            Self::Credits => "credits",
            Self::RequestPool => "request-pool",
            Self::ApiKey => "api-key",
            Self::Unmetered => "unmetered",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Source {
    ProviderEndpoint,
    LocalLogs,
    SelfMetered,
    Signals,
}

impl Source {
    pub const ACCEPTED: [&'static str; 4] = ["signals", "self", "local-logs", "provider-endpoint"];

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "signals" => Some(Self::Signals),
            "self" => Some(Self::SelfMetered),
            "local-logs" => Some(Self::LocalLogs),
            "provider-endpoint" => Some(Self::ProviderEndpoint),
            _ => None,
        }
    }

    pub fn read_in_v0_1(self) -> bool {
        matches!(self, Self::Signals | Self::SelfMetered)
    }
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Signals => "signals",
            Self::SelfMetered => "self",
            Self::LocalLogs => "local-logs",
            Self::ProviderEndpoint => "provider-endpoint",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Allowance {
    Auto,
    Units(f64),
}

impl fmt::Display for Allowance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Auto => f.write_str("auto"),
            Self::Units(units) => write!(f, "{units}"),
        }
    }
}

pub const DEFAULT_SAFETY_MARGIN: f64 = 0.15;
pub const DEFAULT_RESERVE: f64 = 0.20;

#[derive(Debug, Clone, PartialEq)]
pub struct Pool {
    pub name: String,
    pub kind: PoolKind,
    pub agent: String,
    pub window: Option<Duration>,
    pub weekly: bool,
    pub sources: Vec<Source>,
    pub safety_margin: f64,
    pub reserve: f64,
    pub monthly_allowance: Allowance,
    pub endpoint: Option<String>,
    pub profile: Option<String>,
    pub usable: bool,
}

impl Pool {
    pub fn discovered(name: &str, kind: PoolKind, agent: &str, sources: Vec<Source>) -> Self {
        Self {
            name: name.to_owned(),
            kind,
            agent: agent.to_owned(),
            window: (kind == PoolKind::SubscriptionWindow).then(|| Duration::from_secs(5 * 3600)),
            weekly: kind == PoolKind::SubscriptionWindow,
            sources,
            safety_margin: DEFAULT_SAFETY_MARGIN,
            reserve: DEFAULT_RESERVE,
            monthly_allowance: Allowance::Auto,
            endpoint: None,
            profile: None,
            usable: true,
        }
    }

    pub fn describe(&self) -> String {
        let mut line = format!("{} [{}] agent={}", self.name, self.kind, self.agent);
        if let Some(window) = self.window {
            line.push_str(&format!(" window={}", render_duration(window)));
        }
        if self.weekly {
            line.push_str(" +weekly");
        }
        if let Some(profile) = &self.profile {
            line.push_str(&format!(" profile={profile}"));
        }
        if let Some(endpoint) = &self.endpoint {
            line.push_str(&format!(" endpoint={endpoint}"));
        }
        line.push_str(&format!(
            " margin={:.2} reserve={:.2}",
            self.safety_margin, self.reserve
        ));
        if !self.usable {
            line.push_str(" (no adapter in this build)");
        }
        line
    }
}

pub fn pool_for<'a>(agent: &str, pools: &'a [Pool]) -> Option<&'a Pool> {
    pools.iter().find(|pool| pool.agent == agent)
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Spend {
    pub usd: Option<f64>,
    pub attempts: u32,
    pub unpriced: u32,
}

impl Spend {
    fn add(&mut self, cost: Option<f64>) {
        self.attempts = self.attempts.saturating_add(1);
        match cost {
            Some(cost) => self.usd = Some(self.usd.unwrap_or(0.0) + cost),
            None => self.unpriced = self.unpriced.saturating_add(1),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Observations {
    pub exhausted: BTreeMap<String, Option<String>>,
    pub self_spend: BTreeMap<String, Spend>,
}

impl Observations {
    pub fn is_empty(&self) -> bool {
        self.exhausted.is_empty() && self.self_spend.is_empty()
    }
}

pub fn observe(events: &[Event]) -> Observations {
    let mut obs = Observations::default();
    for event in events {
        match &event.body {
            EventBody::PoolExhausted { data, .. } => {
                obs.exhausted
                    .insert(data.pool.clone(), data.reset_at.clone());
            }
            EventBody::AttemptFinished { data, .. }
            | EventBody::AttemptInterrupted { data, .. } => {
                accumulate(&mut obs.self_spend, data);
                retire_signals(&mut obs.exhausted, data);
            }
            _ => {}
        }
    }
    obs
}

pub(crate) fn exhausted_pools_of(events: &[TopologyEvent]) -> BTreeMap<String, Option<String>> {
    let mut exhausted = BTreeMap::new();
    let mut signalled: Vec<&PoolExhausted> = Vec::new();
    for event in events {
        match &event.body {
            TopologyEventBody::PoolExhausted { data } => signalled.push(data),
            TopologyEventBody::AttemptFinished { data } => {
                retire_signals(&mut exhausted, &data.record);
                record_signals(&mut exhausted, signalled.drain(..));
            }
            body => {
                record_signals(&mut exhausted, signalled.drain(..));
                if let TopologyEventBody::CandidatePrepared { data } = body {
                    retire_served_pools(&mut exhausted, &data.attempt);
                }
            }
        }
    }
    record_signals(&mut exhausted, signalled);
    exhausted
}

fn record_signals<'a>(
    exhausted: &mut BTreeMap<String, Option<String>>,
    signals: impl IntoIterator<Item = &'a PoolExhausted>,
) {
    for signal in signals {
        exhausted.insert(signal.pool.clone(), signal.reset_at.clone());
    }
}

fn retire_signals(exhausted: &mut BTreeMap<String, Option<String>>, record: &AttemptRecord) {
    retire_served_pools(exhausted, record);
    if let Some(pool) = limited_pool(record) {
        exhausted.entry(pool.clone()).or_insert(None);
    }
}

pub(crate) fn limited_pool(record: &AttemptRecord) -> Option<&String> {
    let failure = record.failure.as_ref()?;
    if failure.kind != FailureKind::RateLimited {
        return None;
    }
    match failure.origin {
        FailureOrigin::Worker => record.pool.as_ref(),
        FailureOrigin::Reviewer => record
            .reviews
            .last()
            .and_then(|review| review.pool.as_ref()),
    }
}

pub(crate) fn retire_served_pools(
    exhausted: &mut BTreeMap<String, Option<String>>,
    record: &AttemptRecord,
) {
    let worker_served = record.failure.as_ref().is_none_or(|failure| {
        failure.kind != FailureKind::Interrupted
            && !(failure.kind == FailureKind::RateLimited
                && failure.origin == crate::ladder::FailureOrigin::Worker)
    });
    if worker_served {
        if let Some(pool) = &record.pool {
            exhausted.remove(pool);
        }
    }
    for review in &record.reviews {
        if review.outcome != ReviewPassOutcome::Unavailable {
            if let Some(pool) = &review.pool {
                exhausted.remove(pool);
            }
        }
    }
}

pub(crate) fn pool_exhausted_to_record(
    failure: &AttemptFailure,
    record: &AttemptRecord,
    agent: &str,
    recorded: &BTreeMap<String, Option<String>>,
    now_unix: u64,
) -> Option<PoolExhausted> {
    if failure.kind != FailureKind::RateLimited {
        return None;
    }
    let (pool, agent) = match failure.origin {
        FailureOrigin::Worker => (record.pool.as_ref(), agent),
        FailureOrigin::Reviewer => {
            let review = record.reviews.last()?;
            (review.pool.as_ref(), review.agent.as_str())
        }
    };
    let pool = pool?;
    let reset_at = if agent == crate::agent::claude::ADAPTER_ID {
        failure
            .limit_message()
            .into_iter()
            .chain(std::iter::once(failure.reason.as_str()))
            .find(|text| crate::agent::names_a_quota_limit(text))
            .and_then(|text| crate::agent::reset_at_from_limit_message(text, now_unix))
    } else {
        None
    };
    let already_recorded = recorded
        .get(pool)
        .is_some_and(|known| reset_at.is_none() || *known == reset_at);
    if already_recorded {
        return None;
    }
    Some(PoolExhausted {
        pool: pool.clone(),
        agent: agent.to_owned(),
        reset_at,
        detail: crate::util::head(&failure.reason, 400),
    })
}

pub fn drain_of<'a>(
    records: impl IntoIterator<Item = &'a AttemptRecord>,
) -> BTreeMap<String, Spend> {
    let mut drain = BTreeMap::new();
    for record in records {
        accumulate(&mut drain, record);
    }
    drain
}

fn accumulate(drain: &mut BTreeMap<String, Spend>, record: &AttemptRecord) {
    if let Some(pool) = &record.pool {
        drain.entry(pool.clone()).or_default().add(record.cost_usd);
    }
    for review in &record.reviews {
        if let Some(pool) = &review.pool {
            drain.entry(pool.clone()).or_default().add(review.cost_usd);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Remaining {
    Unknown,
    Exhausted,
    AtMost(f64),
    Unmetered,
}

impl fmt::Display for Remaining {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown => f.write_str("unknown"),
            Self::Exhausted => f.write_str("exhausted"),
            Self::AtMost(bound) => write!(f, "≤{:.0}%", bound * 100.0),
            Self::Unmetered => f.write_str("unmetered"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Confidence {
    Unknown,
    Assumed,
    SelfMetered,
    Signal,
}

impl fmt::Display for Confidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Unknown => "unknown",
            Self::Assumed => "assumed",
            Self::SelfMetered => "self-metered",
            Self::Signal => "signal",
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PoolEstimate {
    pub pool: String,
    pub agent: String,
    pub kind: PoolKind,
    pub profile: Option<String>,
    pub remaining: Remaining,
    pub confidence: Confidence,
    pub reset_at: Option<String>,
    pub self_spend: Option<Spend>,
    pub notes: Vec<String>,
}

impl PoolEstimate {
    pub fn describe(&self) -> String {
        let mut line = format!("{}: {} [{}]", self.pool, self.remaining, self.confidence);
        if let Some(reset) = &self.reset_at {
            line.push_str(&format!(" resets {reset}"));
        }
        if let Some(spend) = &self.self_spend {
            let unpriced = if spend.unpriced > 0 { "?" } else { "" };
            match spend.usd {
                Some(usd) => line.push_str(&format!(
                    " — this run drew ${usd:.4}{unpriced} over {} attempt(s)",
                    spend.attempts
                )),
                None => line.push_str(&format!(
                    " — this run drew {} attempt(s), none of which reported spend",
                    spend.attempts
                )),
            }
        }
        line
    }
}

pub fn estimate(pools: &[Pool], obs: &Observations) -> Vec<PoolEstimate> {
    pools.iter().map(|pool| estimate_one(pool, obs)).collect()
}

fn estimate_one(pool: &Pool, obs: &Observations) -> PoolEstimate {
    let mut notes = Vec::new();
    let self_spend = obs.self_spend.get(&pool.name).cloned();

    let mut remaining = Remaining::Unknown;
    let mut confidence = Confidence::Unknown;
    let mut reset_at = None;
    let mut take = |candidate: Remaining, rank: Confidence| {
        if rank > confidence {
            remaining = candidate;
            confidence = rank;
            true
        } else {
            false
        }
    };

    if let Some(reset) = obs.exhausted.get(&pool.name) {
        take(Remaining::Exhausted, Confidence::Signal);
        reset_at = reset.clone();
        if reset.is_none() {
            notes.push(
                "the rate-limit signal carried no reset time, so when it comes back is unknown"
                    .to_owned(),
            );
        }
    }

    if pool.kind == PoolKind::Unmetered {
        take(Remaining::Unmetered, Confidence::Assumed);
    }

    if let (Some(spend), Allowance::Units(allowance)) = (&self_spend, pool.monthly_allowance) {
        if allowance > 0.0 {
            if let Some(usd) = spend.usd {
                let raw = 1.0 - (usd / allowance);
                if take(
                    Remaining::AtMost(effective_remaining(raw, pool)),
                    Confidence::SelfMetered,
                ) {
                    notes.push(
                        "a ceiling, not a measurement: this counts only what upstroke spawned in this \
                         repository, so earlier runs, other repositories, and your own interactive \
                         sessions have all drawn against the same allowance unseen"
                            .to_owned(),
                    );
                    if spend.unpriced > 0 {
                        notes.push(format!(
                            "{} attempt(s) on this pool reported no spend, so even the draw behind that \
                             ceiling is a floor (§13)",
                            spend.unpriced
                        ));
                    }
                }
            }
        }
    }

    if confidence == Confidence::Unknown {
        notes.push(
            "nothing has measured this pool, so what is left is unknown — not full (§13)"
                .to_owned(),
        );
    }

    let unread: Vec<String> = pool
        .sources
        .iter()
        .filter(|source| !source.read_in_v0_1())
        .map(ToString::to_string)
        .collect();
    if !unread.is_empty() {
        notes.push(format!(
            "source(s) {} are parsed but not read in v0.1, so usage they would see — including \
             your own interactive sessions — is not in this figure",
            unread.join(", ")
        ));
    }
    if !pool.usable {
        notes.push(format!(
            "no adapter for agent `{}` in this build, so this engine can never draw from it",
            pool.agent
        ));
    }

    PoolEstimate {
        pool: pool.name.clone(),
        agent: pool.agent.clone(),
        kind: pool.kind,
        profile: pool.profile.clone(),
        remaining,
        confidence,
        reset_at,
        self_spend,
        notes,
    }
}

pub fn effective_remaining(raw: f64, pool: &Pool) -> f64 {
    if !raw.is_finite() {
        return 0.0;
    }
    let discounted = raw.clamp(0.0, 1.0) * (1.0 - pool.safety_margin) - pool.reserve;
    discounted.clamp(0.0, 1.0)
}

pub fn parse_duration(raw: &str) -> Option<Duration> {
    let text = raw.trim();
    let (digits, unit) = text.split_at(text.len().checked_sub(1)?);
    let value: u64 = digits.trim().parse().ok()?;
    let seconds = match unit {
        "s" | "S" => 1,
        "m" | "M" => 60,
        "h" | "H" => 3600,
        "d" | "D" => 86_400,
        _ => return None,
    };
    Some(Duration::from_secs(value.checked_mul(seconds)?))
}

pub fn render_duration(duration: Duration) -> String {
    let seconds = duration.as_secs();
    for (unit, size) in [("d", 86_400u64), ("h", 3600), ("m", 60)] {
        if seconds >= size && seconds % size == 0 {
            return format!("{}{unit}", seconds / size);
        }
    }
    format!("{seconds}s")
}

pub fn strategy_preview(mode: &str, estimates: &[PoolEstimate]) -> Vec<String> {
    let exhausted: Vec<&str> = estimates
        .iter()
        .filter(|e| e.remaining == Remaining::Exhausted)
        .map(|e| e.pool.as_str())
        .collect();
    let measured = estimates
        .iter()
        .any(|e| e.confidence >= Confidence::SelfMetered);

    let mut lines = Vec::new();
    lines.push(match mode {
        "value-max" => "value-max: prepaid capacity that expires unused has zero marginal cost, \
                        so surplus near a reset would bias default tiers UP (spend-down), bounded \
                        by each task's min/max and the pool reserve"
            .to_owned(),
        "deadline" => "deadline: wall-clock first — throughput within capacity, spilling to API \
                       dollars where a $/hour ceiling justified it"
            .to_owned(),
        _ => "conserve: route down aggressively, escalate only on failure, and defer \
              frontier-hungry tasks toward a window reset when a pool is projected to run dry"
            .to_owned(),
    });
    if !exhausted.is_empty() {
        lines.push(format!(
            "exhausted now: {} — under a capacity-driven binder these would demote or defer \
             (§13); today a rate limit only defers the task that hit it, except that a schema-4 \
             run runs a worker's task again at once on another pool of the same agent it does \
             not hold exhausted (§19)",
            exhausted.join(", ")
        ));
    }
    if !measured {
        lines.push(
            "no pool has a measured estimate yet, so every strategy above would be working from \
             the same absence of evidence"
                .to_owned(),
        );
    }
    lines.push(
        "v0.1 ships the capacity engine read-only (§13): none of the above changes what binds — \
         the binder still picks from the catalog and your pins."
            .to_owned(),
    );
    lines
}

#[derive(Debug, Clone, Default)]
pub struct CapacityOptions {
    pub config_path: Option<std::path::PathBuf>,
    pub pools_path: Option<std::path::PathBuf>,
    pub repo_root: std::path::PathBuf,
}

#[derive(Debug)]
pub struct CapacityReport {
    pub pools: Vec<Pool>,
    pub estimates: Vec<PoolEstimate>,
    pub agents: Vec<AgentStatus>,
    pub strategy: String,
    pub run_id: Option<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub struct AgentStatus {
    pub agent: String,
    pub auth: String,
    pub version: Option<String>,
    pub notes: Vec<String>,
}

pub fn report(
    opts: &CapacityOptions,
    adapters: &dyn crate::agent::AdapterSource,
) -> Result<CapacityReport, crate::error::UpstrokeError> {
    let mut warnings = Vec::new();
    let config = crate::config::load(
        opts.config_path.as_deref(),
        &opts.repo_root,
        opts.pools_path.as_deref(),
        &mut warnings,
    )?;

    let (observations, run_id) = match crate::rundir::latest_run(&opts.repo_root) {
        Some(run_id) => {
            let path = crate::rundir::public_dir(&opts.repo_root, &run_id).join("events.jsonl");
            match crate::events::read_all(&path, &mut warnings) {
                Ok(events) => (observe(&events), Some(run_id)),
                Err(error) => {
                    warnings.push(format!(
                        "could not fold run {run_id} for self-metered spend ({error}); showing \
                         signals only"
                    ));
                    (Observations::default(), None)
                }
            }
        }
        None => (Observations::default(), None),
    };

    let mut agents: Vec<AgentStatus> = Vec::new();
    for pool in &config.pools {
        if agents.iter().any(|a| a.agent == pool.agent) {
            continue;
        }
        let Some(adapter) = adapters.get(&pool.agent) else {
            agents.push(AgentStatus {
                agent: pool.agent.clone(),
                auth: "no adapter in this build".to_owned(),
                version: None,
                notes: Vec::new(),
            });
            continue;
        };
        let runner = crate::runner::host::HostRunner::new();
        match adapter.probe(&runner).and_then(|caps| {
            adapter
                .discover(&runner, &caps)
                .map(|discovery| (caps.version.clone(), discovery))
        }) {
            Ok((version, discovery)) => {
                let missing = crate::catalog::missing_from(&pool.agent, &discovery.models);
                if !missing.is_empty() {
                    warnings.push(format!(
                        "{} does not advertise catalogued model(s): {}. Pins and cross-family \
                         review may bind to a `--model` value this CLI rejects — upgrade upstroke \
                         or pin a model it lists.",
                        pool.agent,
                        missing.join(", ")
                    ));
                }
                agents.push(AgentStatus {
                    agent: pool.agent.clone(),
                    auth: discovery.auth.to_string(),
                    version: Some(version),
                    notes: discovery.notes,
                });
            }
            Err(error) => agents.push(AgentStatus {
                agent: pool.agent.clone(),
                auth: format!("could not probe: {error}"),
                version: None,
                notes: Vec::new(),
            }),
        }
    }

    let estimates = estimate(&config.pools, &observations);
    Ok(CapacityReport {
        pools: config.pools,
        estimates,
        agents,
        strategy: config.strategy.mode.clone(),
        run_id,
        warnings,
    })
}

impl CapacityReport {
    pub fn render(&self) -> String {
        use std::fmt::Write as _;

        let mut out = String::new();
        for warning in &self.warnings {
            let _ = writeln!(out, "warning: {warning}");
        }
        if self.pools.is_empty() {
            out.push_str(
                "no pools connected. Run `upstroke connect` to discover the agent CLIs on this \
                 machine and write ~/.upstroke/pools.toml.\n",
            );
            return out;
        }
        for status in &self.agents {
            let _ = writeln!(
                out,
                "{} {}: {}",
                status.agent,
                status.version.as_deref().unwrap_or("(version unknown)"),
                status.auth
            );
            for note in &status.notes {
                let _ = writeln!(out, "  {note}");
            }
        }
        out.push('\n');
        for (pool, estimate) in self.pools.iter().zip(&self.estimates) {
            let _ = writeln!(out, "{}", pool.describe());
            let _ = writeln!(out, "  {}", estimate.describe());
            for note in &estimate.notes {
                let _ = writeln!(out, "  - {note}");
            }
        }
        out.push('\n');
        match &self.run_id {
            Some(run_id) => {
                let _ = writeln!(
                    out,
                    "self-metered draw folded from run {run_id}, the latest in this repository"
                );
            }
            None => {
                let _ = writeln!(
                    out,
                    "no run in this repository yet, so estimates rest on rate-limit signals \
                     alone — estimates need a repo with runs"
                );
            }
        }
        let _ = writeln!(out, "strategy: {}", self.strategy);
        for line in strategy_preview(&self.strategy, &self.estimates) {
            let _ = writeln!(out, "  {line}");
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pool(name: &str) -> Pool {
        Pool::discovered(
            name,
            PoolKind::SubscriptionWindow,
            "claude-code",
            vec![Source::Signals, Source::SelfMetered],
        )
    }

    #[test]
    fn an_unmeasured_pool_is_unknown_never_full() {
        let estimates = estimate(&[pool("claude-max")], &Observations::default());
        assert_eq!(estimates[0].remaining, Remaining::Unknown);
        assert_eq!(estimates[0].confidence, Confidence::Unknown);
        assert!(
            estimates[0].notes.iter().any(|n| n.contains("not full")),
            "notes: {:?}",
            estimates[0].notes
        );
    }

    #[test]
    fn margins_apply_multiplicatively_then_subtract_the_reserve() {
        let pool = pool("claude-max");
        assert!((effective_remaining(0.5, &pool) - 0.225).abs() < 1e-9);
        assert_eq!(effective_remaining(0.1, &pool), 0.0);
        assert!((effective_remaining(2.0, &pool) - 0.65).abs() < 1e-9);
        assert_eq!(effective_remaining(f64::NAN, &pool), 0.0);

        let mut generous = pool.clone();
        generous.safety_margin = 0.0;
        generous.reserve = 0.0;
        assert!((effective_remaining(0.5, &generous) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn a_self_metered_estimate_is_conservative_end_to_end() {
        let mut p = pool("api");
        p.kind = PoolKind::ApiKey;
        p.monthly_allowance = Allowance::Units(100.0);
        let mut obs = Observations::default();
        obs.self_spend.insert(
            "api".to_owned(),
            Spend {
                usd: Some(50.0),
                attempts: 4,
                unpriced: 0,
            },
        );
        let estimates = estimate(&[p], &obs);
        assert_eq!(estimates[0].confidence, Confidence::SelfMetered);
        let Remaining::AtMost(left) = estimates[0].remaining else {
            panic!("expected an upper bound: {:?}", estimates[0].remaining);
        };
        assert!((left - 0.225).abs() < 1e-9, "left: {left}");
        assert!(
            estimates[0].describe().contains("≤22%"),
            "{}",
            estimates[0].describe()
        );
        assert!(
            estimates[0]
                .notes
                .iter()
                .any(|n| n.contains("a ceiling, not a measurement")),
            "notes: {:?}",
            estimates[0].notes
        );
    }

    #[test]
    fn a_pool_that_serves_again_stops_reading_as_exhausted() {
        use crate::events::{AttemptRecord, Event, EventBody, PoolExhausted};
        use crate::ladder::{FailureKind, FailureOrigin};

        let record = |failure: Option<FailureKind>| {
            Event::now(EventBody::AttemptFinished {
                task: "t1".to_owned(),
                attempt: 1,
                rung: 0,
                profile: "p".to_owned(),
                parking: None,
                transition: None,
                prepared_commit: None,
                data: Box::new(AttemptRecord {
                    attempt: 1,
                    tier: "small".to_owned(),
                    model: "m".to_owned(),
                    pool: Some("claude-max".to_owned()),
                    resumed: false,
                    duration: Duration::ZERO,
                    cost_usd: None,
                    reviews: Vec::new(),
                    session_id: None,
                    usage: None,
                    failure: failure.map(|kind| crate::events::FailureRecord {
                        kind,
                        origin: FailureOrigin::Worker,
                        reason: "r".to_owned(),
                        detail: None,
                    }),
                }),
            })
        };
        let signal = Event::now(EventBody::PoolExhausted {
            task: "t1".to_owned(),
            data: PoolExhausted {
                pool: "claude-max".to_owned(),
                agent: "claude-code".to_owned(),
                reset_at: None,
                detail: "5-hour limit reached".to_owned(),
            },
        });

        let obs = observe(&[signal.clone(), record(None)]);
        assert!(obs.exhausted.is_empty(), "{:?}", obs.exhausted);

        let obs = observe(&[signal.clone(), record(Some(FailureKind::GateFailed))]);
        assert!(obs.exhausted.is_empty(), "{:?}", obs.exhausted);

        for still_down in [FailureKind::RateLimited, FailureKind::Interrupted] {
            let obs = observe(&[signal.clone(), record(Some(still_down))]);
            assert!(
                obs.exhausted.contains_key("claude-max"),
                "{still_down:?} must not retire the signal"
            );
        }

        let obs = observe(&[signal.clone(), record(None), signal]);
        assert!(obs.exhausted.contains_key("claude-max"));

        let reviewer_limited = Event::now(EventBody::AttemptFinished {
            task: "t1".to_owned(),
            attempt: 2,
            rung: 0,
            profile: "p".to_owned(),
            parking: None,
            transition: None,
            prepared_commit: None,
            data: Box::new(AttemptRecord {
                attempt: 2,
                tier: "small".to_owned(),
                model: "worker".to_owned(),
                pool: Some("worker-pool".to_owned()),
                resumed: false,
                duration: Duration::ZERO,
                cost_usd: None,
                reviews: vec![
                    crate::events::ReviewRecord {
                        pass: "review".to_owned(),
                        agent: "codex".to_owned(),
                        model: "sol".to_owned(),
                        adapter: None,
                        preflight_cli_version: None,
                        effort: None,
                        pool: Some("recovered-reviewer".to_owned()),
                        cost_usd: None,
                        outcome: crate::events::ReviewPassOutcome::Passed,
                    },
                    crate::events::ReviewRecord {
                        pass: "second-opinion".to_owned(),
                        agent: "claude-code".to_owned(),
                        model: "opus".to_owned(),
                        adapter: None,
                        preflight_cli_version: None,
                        effort: None,
                        pool: Some("limited-reviewer".to_owned()),
                        cost_usd: None,
                        outcome: crate::events::ReviewPassOutcome::Unavailable,
                    },
                ],
                session_id: None,
                usage: None,
                failure: Some(crate::events::FailureRecord {
                    kind: FailureKind::RateLimited,
                    origin: FailureOrigin::Reviewer,
                    reason: "review pool limited".to_owned(),
                    detail: None,
                }),
            }),
        });
        let mut prior = Observations::default();
        for pool in ["worker-pool", "recovered-reviewer", "limited-reviewer"] {
            prior.exhausted.insert(pool.to_owned(), None);
        }
        let mut events = Vec::new();
        for pool in prior.exhausted.keys() {
            events.push(Event::now(EventBody::PoolExhausted {
                task: "t1".to_owned(),
                data: PoolExhausted {
                    pool: pool.clone(),
                    agent: "agent".to_owned(),
                    reset_at: None,
                    detail: "old signal".to_owned(),
                },
            }));
        }
        events.push(reviewer_limited);
        let obs = observe(&events);
        assert!(!obs.exhausted.contains_key("worker-pool"), "{obs:?}");
        assert!(!obs.exhausted.contains_key("recovered-reviewer"), "{obs:?}");
        assert!(obs.exhausted.contains_key("limited-reviewer"), "{obs:?}");
    }

    #[test]
    fn a_later_rate_limit_keeps_the_reset_its_pool_exhausted_recorded() {
        use crate::events::{AttemptRecord, Event, EventBody, PoolExhausted};
        use crate::ladder::{FailureKind, FailureOrigin};

        let record = |attempt: u32, failure: Option<FailureKind>| {
            Event::now(EventBody::AttemptFinished {
                task: "t1".to_owned(),
                attempt,
                rung: 0,
                profile: "p".to_owned(),
                parking: None,
                transition: None,
                prepared_commit: None,
                data: Box::new(AttemptRecord {
                    attempt,
                    tier: "small".to_owned(),
                    model: "m".to_owned(),
                    pool: Some("claude-max".to_owned()),
                    resumed: false,
                    duration: Duration::ZERO,
                    cost_usd: None,
                    reviews: Vec::new(),
                    session_id: None,
                    usage: None,
                    failure: failure.map(|kind| crate::events::FailureRecord {
                        kind,
                        origin: FailureOrigin::Worker,
                        reason: "You've hit your 5-hour limit · resets 5:30am (UTC)".to_owned(),
                        detail: None,
                    }),
                }),
            })
        };
        let signal = Event::now(EventBody::PoolExhausted {
            task: "t1".to_owned(),
            data: PoolExhausted {
                pool: "claude-max".to_owned(),
                agent: "claude-code".to_owned(),
                reset_at: Some("2026-08-09T05:30:00Z".to_owned()),
                detail: "You've hit your 5-hour limit · resets 5:30am (UTC)".to_owned(),
            },
        });
        let obs = observe(&[record(1, Some(FailureKind::RateLimited))]);
        assert_eq!(obs.exhausted.get("claude-max"), Some(&None), "{obs:?}");

        let mut events = vec![
            record(1, Some(FailureKind::RateLimited)),
            signal,
            record(2, Some(FailureKind::RateLimited)),
        ];

        let obs = observe(&events);
        assert_eq!(
            obs.exhausted.get("claude-max"),
            Some(&Some("2026-08-09T05:30:00Z".to_owned())),
            "a deferral's rate limit must not erase the reset the outage recorded: {:?}",
            obs.exhausted
        );

        events.push(record(3, None));
        let obs = observe(&events);
        assert!(obs.exhausted.is_empty(), "{:?}", obs.exhausted);
    }

    #[test]
    fn a_later_pool_exhausted_with_a_different_reset_replaces_the_earlier_one() {
        use crate::events::{AttemptRecord, Event, EventBody, PoolExhausted};
        use crate::ladder::{FailureKind, FailureOrigin};

        let signal = |reset_at: &str, detail: &str| {
            Event::now(EventBody::PoolExhausted {
                task: "t1".to_owned(),
                data: PoolExhausted {
                    pool: "claude-max".to_owned(),
                    agent: "claude-code".to_owned(),
                    reset_at: Some(reset_at.to_owned()),
                    detail: detail.to_owned(),
                },
            })
        };
        let rate_limited = Event::now(EventBody::AttemptFinished {
            task: "t1".to_owned(),
            attempt: 2,
            rung: 0,
            profile: "p".to_owned(),
            parking: None,
            transition: None,
            prepared_commit: None,
            data: Box::new(AttemptRecord {
                attempt: 2,
                tier: "small".to_owned(),
                model: "m".to_owned(),
                pool: Some("claude-max".to_owned()),
                resumed: false,
                duration: Duration::ZERO,
                cost_usd: None,
                reviews: Vec::new(),
                session_id: None,
                usage: None,
                failure: Some(crate::events::FailureRecord {
                    kind: FailureKind::RateLimited,
                    origin: FailureOrigin::Worker,
                    reason: "You've hit your weekly limit · resets Nov 14, 5pm (UTC)".to_owned(),
                    detail: None,
                }),
            }),
        });
        let five_hour = signal(
            "2026-11-14T05:30:00Z",
            "You've hit your 5-hour limit · resets 5:30am (UTC)",
        );
        let weekly = signal(
            "2026-11-14T17:00:00Z",
            "You've hit your weekly limit · resets Nov 14, 5pm (UTC)",
        );

        let obs = observe(&[five_hour.clone(), rate_limited.clone(), weekly.clone()]);
        assert_eq!(
            obs.exhausted.get("claude-max"),
            Some(&Some("2026-11-14T17:00:00Z".to_owned())),
            "the latest signal's reset wins over the one it replaced: {:?}",
            obs.exhausted
        );

        let obs = observe(&[
            five_hour.clone(),
            rate_limited.clone(),
            weekly,
            rate_limited.clone(),
        ]);
        assert_eq!(
            obs.exhausted.get("claude-max"),
            Some(&Some("2026-11-14T17:00:00Z".to_owned())),
            "a rate limit after the second signal keeps that signal's reset: {:?}",
            obs.exhausted
        );

        let obs = observe(&[five_hour, rate_limited]);
        assert_eq!(
            obs.exhausted.get("claude-max"),
            Some(&Some("2026-11-14T05:30:00Z".to_owned())),
            "before the second signal, the first one's reset stands: {:?}",
            obs.exhausted
        );
    }

    const FIVE_HOUR_LIMIT: &str = "You've hit your 5-hour limit · resets 5:30am (UTC)";
    const FABLE_LIMIT: &str = "You've hit your Fable limit · resets Nov 14, 9am (UTC)";
    const NOV_11_2026_AT_3AM: u64 = 1_794_355_200 + 3 * 3_600;

    fn settled(pool: Option<&str>, reviews: Vec<crate::events::ReviewRecord>) -> AttemptRecord {
        AttemptRecord {
            attempt: 1,
            tier: "small".to_owned(),
            model: "m".to_owned(),
            pool: pool.map(str::to_owned),
            resumed: false,
            duration: Duration::ZERO,
            cost_usd: None,
            reviews,
            session_id: None,
            usage: None,
            failure: None,
        }
    }

    fn review_on(agent: &str, pool: Option<&str>) -> crate::events::ReviewRecord {
        crate::events::ReviewRecord {
            pass: "review".to_owned(),
            agent: agent.to_owned(),
            model: "m".to_owned(),
            adapter: None,
            preflight_cli_version: None,
            effort: None,
            pool: pool.map(str::to_owned),
            cost_usd: None,
            outcome: ReviewPassOutcome::Unavailable,
        }
    }

    fn rate_limited(reason: &str) -> AttemptFailure {
        AttemptFailure::new(FailureKind::RateLimited, reason)
    }

    #[test]
    fn a_rate_limited_claude_code_worker_failure_naming_a_reset_answers_that_pool_and_reset() {
        let answer = pool_exhausted_to_record(
            &rate_limited(FIVE_HOUR_LIMIT),
            &settled(Some("claude-max"), Vec::new()),
            crate::agent::claude::ADAPTER_ID,
            &BTreeMap::new(),
            NOV_11_2026_AT_3AM,
        );
        assert_eq!(
            answer,
            Some(PoolExhausted {
                pool: "claude-max".to_owned(),
                agent: "claude-code".to_owned(),
                reset_at: Some("2026-11-11T05:30:00Z".to_owned()),
                detail: FIVE_HOUR_LIMIT.to_owned(),
            })
        );
    }

    #[test]
    fn a_pool_already_recorded_with_the_same_reset_answers_no_pool_exhausted() {
        let recorded = BTreeMap::from([(
            "claude-max".to_owned(),
            Some("2026-11-11T05:30:00Z".to_owned()),
        )]);
        let answer = pool_exhausted_to_record(
            &rate_limited(FIVE_HOUR_LIMIT),
            &settled(Some("claude-max"), Vec::new()),
            crate::agent::claude::ADAPTER_ID,
            &recorded,
            NOV_11_2026_AT_3AM,
        );
        assert_eq!(answer, None);
    }

    #[test]
    fn a_pool_already_recorded_with_a_different_reset_answers_one_with_the_new_reset() {
        let recorded = BTreeMap::from([(
            "claude-max".to_owned(),
            Some("2026-11-11T05:30:00Z".to_owned()),
        )]);
        let answer = pool_exhausted_to_record(
            &rate_limited(FABLE_LIMIT),
            &settled(Some("claude-max"), Vec::new()),
            crate::agent::claude::ADAPTER_ID,
            &recorded,
            NOV_11_2026_AT_3AM,
        );
        assert_eq!(
            answer,
            Some(PoolExhausted {
                pool: "claude-max".to_owned(),
                agent: "claude-code".to_owned(),
                reset_at: Some("2026-11-14T09:00:00Z".to_owned()),
                detail: FABLE_LIMIT.to_owned(),
            })
        );
    }

    #[test]
    fn a_rate_limited_reviewer_failure_takes_the_last_reviews_pool_and_agent() {
        let record = settled(
            Some("worker-pool"),
            vec![
                review_on("codex", Some("recovered-reviewer")),
                review_on("claude-code", Some("limited-reviewer")),
            ],
        );
        let answer = pool_exhausted_to_record(
            &rate_limited(FIVE_HOUR_LIMIT).from_reviewer(),
            &record,
            "codex",
            &BTreeMap::new(),
            NOV_11_2026_AT_3AM,
        );
        assert_eq!(
            answer,
            Some(PoolExhausted {
                pool: "limited-reviewer".to_owned(),
                agent: "claude-code".to_owned(),
                reset_at: Some("2026-11-11T05:30:00Z".to_owned()),
                detail: FIVE_HOUR_LIMIT.to_owned(),
            })
        );
    }

    #[test]
    fn a_rate_limited_failure_naming_no_pool_answers_no_pool_exhausted() {
        let worker = rate_limited(FIVE_HOUR_LIMIT);
        let reviewer = rate_limited(FIVE_HOUR_LIMIT).from_reviewer();
        for (failure, record) in [
            (&worker, settled(None, Vec::new())),
            (&reviewer, settled(Some("worker-pool"), Vec::new())),
            (
                &reviewer,
                settled(
                    Some("worker-pool"),
                    vec![
                        review_on("claude-code", Some("earlier-reviewer")),
                        review_on("claude-code", None),
                    ],
                ),
            ),
        ] {
            let answer = pool_exhausted_to_record(
                failure,
                &record,
                crate::agent::claude::ADAPTER_ID,
                &BTreeMap::new(),
                NOV_11_2026_AT_3AM,
            );
            assert_eq!(answer, None, "{failure:?} on {record:?}");
        }
    }

    #[test]
    fn a_rate_limited_non_claude_agent_answers_a_pool_exhausted_with_no_reset() {
        let answer = pool_exhausted_to_record(
            &rate_limited(FIVE_HOUR_LIMIT),
            &settled(Some("codex-pro"), Vec::new()),
            "codex",
            &BTreeMap::new(),
            NOV_11_2026_AT_3AM,
        );
        assert_eq!(
            answer,
            Some(PoolExhausted {
                pool: "codex-pro".to_owned(),
                agent: "codex".to_owned(),
                reset_at: None,
                detail: FIVE_HOUR_LIMIT.to_owned(),
            })
        );
    }

    #[test]
    fn a_failure_that_is_not_rate_limited_answers_no_pool_exhausted() {
        for kind in [
            FailureKind::AgentError,
            FailureKind::GateFailed,
            FailureKind::ReviewUnavailable,
            FailureKind::Interrupted,
        ] {
            let answer = pool_exhausted_to_record(
                &AttemptFailure::new(kind, FIVE_HOUR_LIMIT),
                &settled(Some("claude-max"), Vec::new()),
                crate::agent::claude::ADAPTER_ID,
                &BTreeMap::new(),
                NOV_11_2026_AT_3AM,
            );
            assert_eq!(answer, None, "{kind:?}");
        }
    }

    fn topology(body: TopologyEventBody) -> TopologyEvent {
        TopologyEvent {
            ts: "2026-11-11T03:00:00Z".to_owned(),
            body,
        }
    }

    fn signalled(pool: &str, reset_at: Option<&str>) -> TopologyEvent {
        topology(TopologyEventBody::PoolExhausted {
            data: PoolExhausted {
                pool: pool.to_owned(),
                agent: crate::agent::claude::ADAPTER_ID.to_owned(),
                reset_at: reset_at.map(str::to_owned),
                detail: FIVE_HOUR_LIMIT.to_owned(),
            },
        })
    }

    fn failed(record: AttemptRecord, kind: FailureKind, origin: FailureOrigin) -> AttemptRecord {
        AttemptRecord {
            failure: Some(crate::events::FailureRecord {
                kind,
                origin,
                reason: FIVE_HOUR_LIMIT.to_owned(),
                detail: None,
            }),
            ..record
        }
    }

    fn finished(record: AttemptRecord) -> TopologyEvent {
        use crate::topology::events::{
            AttemptFinished4, AttemptNumber, AttemptSettlement, GenerationId, LeaseDisposition,
            SettlementTransition,
        };
        topology(TopologyEventBody::AttemptFinished {
            data: Box::new(AttemptFinished4 {
                key: crate::topology::registry::TaskKey(0),
                generation: GenerationId(0),
                attempt: AttemptNumber(record.attempt),
                record: Box::new(record),
                settlement: AttemptSettlement::Closed {
                    transition: SettlementTransition::Deferred {
                        defers: 1,
                        reason: FIVE_HOUR_LIMIT.to_owned(),
                    },
                    lease: LeaseDisposition::PredictedReleased,
                },
            }),
        })
    }

    fn prepared(record: AttemptRecord) -> TopologyEvent {
        use crate::topology::events::{
            CandidateLeaseEffect, CandidatePrepared, CommitSha, GenerationId, GitRef,
        };
        use crate::topology::paths::PathSet;
        let sha = |digit: &str| CommitSha(digit.repeat(40));
        topology(TopologyEventBody::CandidatePrepared {
            data: Box::new(CandidatePrepared {
                key: crate::topology::registry::TaskKey(1),
                generation: GenerationId(0),
                attempt: Box::new(record),
                base_sha: sha("a"),
                parent_sha: sha("a"),
                tree_sha: sha("b"),
                commit_sha: sha("c"),
                message: "upstroke: beta attempt 1".to_owned(),
                prepared_ref: GitRef("refs/upstroke/prepared/beta".to_owned()),
                candidate_ref: GitRef("refs/upstroke/candidates/beta".to_owned()),
                actual_paths: PathSet::RepoWide,
                lease_effect: CandidateLeaseEffect::ReplacesPredicted {
                    paths: PathSet::RepoWide,
                },
            }),
        })
    }

    fn interrupted() -> TopologyEvent {
        use crate::topology::events::{
            AttemptInterrupted4, AttemptNumber, GenerationId, LeaseDisposition,
        };
        topology(TopologyEventBody::AttemptInterrupted {
            data: AttemptInterrupted4 {
                key: crate::topology::registry::TaskKey(0),
                generation: GenerationId(0),
                attempt: AttemptNumber(1),
                lease: LeaseDisposition::PredictedReleased,
                detail: "the process ended before the attempt settled".to_owned(),
            },
        })
    }

    fn exhausted(pools: &[(&str, Option<&str>)]) -> BTreeMap<String, Option<String>> {
        pools
            .iter()
            .map(|(pool, reset_at)| ((*pool).to_owned(), reset_at.map(str::to_owned)))
            .collect()
    }

    const FIVE_THIRTY: &str = "2026-11-11T05:30:00Z";
    const NOV_14_AT_9AM: &str = "2026-11-14T09:00:00Z";

    #[test]
    fn a_topology_log_holds_each_pool_at_the_reset_its_pool_exhausted_recorded() {
        let limited = || {
            finished(failed(
                settled(Some("claude-max"), Vec::new()),
                FailureKind::RateLimited,
                FailureOrigin::Worker,
            ))
        };
        let events = [
            signalled("claude-max", Some(FIVE_THIRTY)),
            limited(),
            limited(),
        ];
        assert_eq!(
            exhausted_pools_of(&events),
            exhausted(&[("claude-max", Some(FIVE_THIRTY))]),
            "a later rate-limited settlement with no signal of its own keeps the reset"
        );
        let events = [
            signalled("claude-max", Some(FIVE_THIRTY)),
            limited(),
            signalled("claude-max", Some(NOV_14_AT_9AM)),
            limited(),
        ];
        assert_eq!(
            exhausted_pools_of(&events),
            exhausted(&[("claude-max", Some(NOV_14_AT_9AM))]),
            "a later signal naming a different reset replaces the earlier one"
        );
    }

    #[test]
    fn a_topology_log_ending_in_pool_exhausted_holds_that_pool_at_that_reset() {
        for events in [
            vec![signalled("claude-max", Some(FIVE_THIRTY))],
            vec![signalled("claude-max", Some(FIVE_THIRTY)), interrupted()],
        ] {
            assert_eq!(
                exhausted_pools_of(&events),
                exhausted(&[("claude-max", Some(FIVE_THIRTY))]),
                "{events:?}"
            );
        }
    }

    #[test]
    fn a_pool_exhausted_is_recorded_after_its_own_attempt_retires_the_pools_it_served() {
        let record = failed(
            settled(
                Some("claude-max"),
                vec![review_on(
                    crate::agent::claude::ADAPTER_ID,
                    Some("claude-max"),
                )],
            ),
            FailureKind::RateLimited,
            FailureOrigin::Reviewer,
        );
        let events = [signalled("claude-max", Some(FIVE_THIRTY)), finished(record)];
        assert_eq!(
            exhausted_pools_of(&events),
            exhausted(&[("claude-max", Some(FIVE_THIRTY))]),
            "the worker served on the pool its reviewer was then refused on; the live run retires \
             the worker's pool before it writes the reviewer's signal, so the signal stands"
        );
    }

    #[test]
    fn a_rate_limited_settlement_with_no_pool_exhausted_marks_its_pool_with_no_reset() {
        let events = [finished(failed(
            settled(Some("codex-pro"), Vec::new()),
            FailureKind::RateLimited,
            FailureOrigin::Worker,
        ))];
        assert_eq!(
            exhausted_pools_of(&events),
            exhausted(&[("codex-pro", None)])
        );
    }

    #[test]
    fn a_later_attempt_the_pool_served_retires_its_signal_settled_or_promoted() {
        let signal = || signalled("claude-max", Some(FIVE_THIRTY));
        let limited = || {
            finished(failed(
                settled(Some("claude-max"), Vec::new()),
                FailureKind::RateLimited,
                FailureOrigin::Worker,
            ))
        };
        let gate_failed = finished(failed(
            settled(Some("claude-max"), Vec::new()),
            FailureKind::GateFailed,
            FailureOrigin::Worker,
        ));
        let promoted = prepared(settled(Some("claude-max"), Vec::new()));
        for served in [gate_failed, promoted] {
            let events = [signal(), limited(), served.clone()];
            assert_eq!(
                exhausted_pools_of(&events),
                BTreeMap::new(),
                "{}",
                served.body.kind()
            );
        }
    }

    #[test]
    fn self_metering_cannot_talk_an_exhausted_pool_back_up() {
        let mut p = pool("claude-max");
        p.monthly_allowance = Allowance::Units(100.0);
        let mut obs = Observations::default();
        obs.exhausted.insert(
            "claude-max".to_owned(),
            Some("2026-08-09T18:00:00Z".to_owned()),
        );
        obs.self_spend.insert(
            "claude-max".to_owned(),
            Spend {
                usd: Some(1.0),
                attempts: 1,
                unpriced: 0,
            },
        );
        let estimates = estimate(&[p], &obs);
        assert_eq!(estimates[0].remaining, Remaining::Exhausted);
        assert_eq!(estimates[0].confidence, Confidence::Signal);
        assert_eq!(
            estimates[0].reset_at.as_deref(),
            Some("2026-08-09T18:00:00Z")
        );
        assert_eq!(
            estimates[0].self_spend.as_ref().map(|s| s.attempts),
            Some(1)
        );
    }

    #[test]
    fn an_unknown_allowance_reports_the_draw_without_inventing_a_ceiling() {
        let mut obs = Observations::default();
        obs.self_spend.insert(
            "claude-max".to_owned(),
            Spend {
                usd: Some(3.0),
                attempts: 2,
                unpriced: 1,
            },
        );
        let estimates = estimate(&[pool("claude-max")], &obs);
        assert_eq!(estimates[0].remaining, Remaining::Unknown);
        assert!(
            estimates[0].describe().contains("$3.0000?"),
            "{}",
            estimates[0].describe()
        );
    }

    #[test]
    fn unread_sources_get_a_note_rather_than_a_pretend_estimate() {
        let mut p = pool("claude-max");
        p.sources = vec![Source::Signals, Source::SelfMetered, Source::LocalLogs];
        let estimates = estimate(&[p], &Observations::default());
        assert!(
            estimates[0]
                .notes
                .iter()
                .any(|n| n.contains("local-logs") && n.contains("not read")),
            "notes: {:?}",
            estimates[0].notes
        );
    }

    #[test]
    fn a_local_pool_is_unmetered_by_shape_not_by_measurement() {
        let mut p = Pool::discovered("local", PoolKind::Unmetered, "aider", vec![Source::Signals]);
        p.usable = false;
        let estimates = estimate(&[p], &Observations::default());
        assert_eq!(estimates[0].remaining, Remaining::Unmetered);
        assert_eq!(estimates[0].confidence, Confidence::Assumed);
        assert!(
            estimates[0].notes.iter().any(|n| n.contains("no adapter")),
            "notes: {:?}",
            estimates[0].notes
        );
    }

    #[test]
    fn pool_selection_is_first_match_in_file_order() {
        let pools = vec![pool("claude-max-a"), pool("claude-max-b")];
        assert_eq!(
            pool_for("claude-code", &pools).map(|p| p.name.as_str()),
            Some("claude-max-a")
        );
        assert!(pool_for("copilot", &pools).is_none());
    }

    #[test]
    fn durations_round_trip_through_their_own_spellings() {
        for (text, secs) in [
            ("5h", 18_000u64),
            ("30m", 1800),
            ("7d", 604_800),
            ("90s", 90),
        ] {
            let parsed = parse_duration(text).expect(text);
            assert_eq!(parsed.as_secs(), secs);
            assert_eq!(render_duration(parsed), text);
        }
        for bad in [
            "",
            "h",
            "5",
            "5 hours",
            "-1h",
            "5x",
            "999999999999999999999h",
        ] {
            assert!(parse_duration(bad).is_none(), "should reject: {bad:?}");
        }
    }

    #[test]
    fn every_strategy_preview_says_it_changes_nothing() {
        for mode in ["conserve", "value-max", "deadline", "something-else"] {
            let lines = strategy_preview(mode, &[]);
            assert!(
                lines.iter().any(|l| l.contains("read-only")),
                "{mode}: {lines:?}"
            );
        }
    }

    #[test]
    fn the_strategy_preview_names_the_failover_for_an_exhausted_pool() {
        let mut obs = Observations::default();
        obs.exhausted.insert("claude-max".to_owned(), None);
        let estimates = estimate(&[pool("claude-max")], &obs);
        let lines = strategy_preview("conserve", &estimates);
        let today = "today a rate limit only defers the task that hit it, except that a \
                     schema-4 run runs a worker's task again at once on another pool of the \
                     same agent it does not hold exhausted (§19)";
        assert!(
            lines
                .iter()
                .any(|l| l.starts_with("exhausted now: claude-max — ") && l.ends_with(today)),
            "{lines:?}"
        );
    }
}
