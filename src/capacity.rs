//! Extended notes: `docs/internals/capacity.md`

#![forbid(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros
)]

use std::collections::BTreeMap;
use std::fmt;
use std::time::Duration;

use crate::events::{AttemptRecord, Event, PoolExhausted, ReviewPassOutcome, ReviewRecord};
use crate::ladder::{AttemptFailure, FailureKind, FailureOrigin};
use crate::topology::events::TopologyEvent;

pub mod evidence;
pub mod machine;
pub mod usage;

pub use self::machine::{Gathered, Scope};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageProvider {
    AnthropicCost,
    OpenaiCost,
    GithubAiCredits,
    GithubPremiumRequests,
}

impl UsageProvider {
    pub const ALL: [Self; 4] = [
        Self::AnthropicCost,
        Self::OpenaiCost,
        Self::GithubAiCredits,
        Self::GithubPremiumRequests,
    ];

    pub const ACCEPTED: [&'static str; 4] = [
        "anthropic-cost",
        "openai-cost",
        "github-ai-credits",
        "github-premium-requests",
    ];

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|provider| provider.to_string() == s)
    }

    pub const fn needs_account(self) -> bool {
        match self {
            Self::AnthropicCost | Self::OpenaiCost => false,
            Self::GithubAiCredits | Self::GithubPremiumRequests => true,
        }
    }
}

impl fmt::Display for UsageProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::AnthropicCost => "anthropic-cost",
            Self::OpenaiCost => "openai-cost",
            Self::GithubAiCredits => "github-ai-credits",
            Self::GithubPremiumRequests => "github-premium-requests",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageSource {
    pub provider: UsageProvider,
    pub token_env: Option<String>,
    pub account: Option<String>,
}

pub const MAX_USAGE_KEY_CHARS: usize = 128;

pub const MAX_TOKEN_ENV_CHARS: usize = 64;

const CREDENTIAL_PREFIXES: [&str; 9] = [
    "github_pat_",
    "ghp_",
    "gho_",
    "ghu_",
    "ghs_",
    "ghr_",
    "sk-",
    "sk-ant-",
    "xoxb-",
];

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UsageKeyDefect {
    #[error(
        "sets `{}` and no `usage_provider`, so nothing would ever be asked with it — name the \
         provider (one of: {}), or remove the key",
        .stray.join("` and `"),
        UsageProvider::ACCEPTED.join(", ")
    )]
    NoProvider { stray: Vec<&'static str> },
    #[error(
        "`usage_provider = \"{written}\"` is not recognized (accepted: {})",
        UsageProvider::ACCEPTED.join(", ")
    )]
    UnknownProvider { written: String },
    #[error(
        "`usage_token_env` must be the NAME of an environment variable that holds the \
         credential — letters, digits and underscores, not starting with a digit, at most \
         {MAX_TOKEN_ENV_CHARS} characters. What is written there is not a name, and it is not \
         repeated here in case it is the credential itself; a credential never belongs in this \
         file"
    )]
    TokenEnvNotAName,
    #[error(
        "`usage_token_env` must be the NAME of an environment variable that holds the \
         credential, and what is written there has the shape of a credential instead: it starts \
         with a prefix providers give their tokens (`{}`) or is longer than \
         {MAX_TOKEN_ENV_CHARS} characters. It is not repeated here; a credential never belongs \
         in this file — put it in an environment variable and write that variable's name",
        CREDENTIAL_PREFIXES.join("`, `")
    )]
    TokenEnvLooksLikeACredential,
    #[error(
        "`usage_account` must be an account name as the provider writes it — letters, digits, \
         `.`, `_` and `-`, not starting with `.` or `-`, at most {MAX_USAGE_KEY_CHARS} \
         characters — because it is placed in the request's address"
    )]
    AccountNotAName,
}

impl UsageSource {
    pub fn from_keys(
        provider: Option<&str>,
        token_env: Option<&str>,
        account: Option<&str>,
    ) -> Result<Option<Self>, UsageKeyDefect> {
        let Some(written) = provider else {
            let stray: Vec<&'static str> = [
                ("usage_token_env", token_env.is_some()),
                ("usage_account", account.is_some()),
            ]
            .into_iter()
            .filter_map(|(key, present)| present.then_some(key))
            .collect();
            return if stray.is_empty() {
                Ok(None)
            } else {
                Err(UsageKeyDefect::NoProvider { stray })
            };
        };
        let provider =
            UsageProvider::parse(written).ok_or_else(|| UsageKeyDefect::UnknownProvider {
                written: written.to_owned(),
            })?;
        if token_env.is_some_and(|name| !names_an_environment_variable(name)) {
            return Err(UsageKeyDefect::TokenEnvNotAName);
        }
        if token_env.is_some_and(looks_like_a_credential) {
            return Err(UsageKeyDefect::TokenEnvLooksLikeACredential);
        }
        if account.is_some_and(|name| !names_an_account(name)) {
            return Err(UsageKeyDefect::AccountNotAName);
        }
        Ok(Some(Self {
            provider,
            token_env: token_env.map(str::to_owned),
            account: account.map(str::to_owned),
        }))
    }

    pub fn gaps(&self) -> Vec<String> {
        let provider = self.provider;
        let mut gaps = Vec::new();
        if self.token_env.is_none() {
            gaps.push(format!(
                "names `usage_provider = \"{provider}\"` and no `usage_token_env`, so `upstroke \
                 capacity --refresh` has no credential to ask it with and will skip this pool"
            ));
        }
        match (&self.account, provider.needs_account()) {
            (None, true) => gaps.push(format!(
                "names `usage_provider = \"{provider}\"`, which reports per account, and no \
                 `usage_account`, so `upstroke capacity --refresh` will skip this pool"
            )),
            (Some(_), false) => gaps.push(format!(
                "sets `usage_account`, which `usage_provider = \"{provider}\"` does not read \
                 (ignored)"
            )),
            (Some(_), true) | (None, false) => {}
        }
        gaps
    }
}

pub fn names_an_environment_variable(text: &str) -> bool {
    let mut characters = text.chars();
    characters
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
        && text.len() <= MAX_USAGE_KEY_CHARS
}

fn looks_like_a_credential(text: &str) -> bool {
    CREDENTIAL_PREFIXES
        .into_iter()
        .any(|prefix| text.starts_with(prefix))
        || text.chars().count() > MAX_TOKEN_ENV_CHARS
}

pub fn names_an_account(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= MAX_USAGE_KEY_CHARS
        && text.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
        })
        && !text.starts_with(['.', '-'])
}

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
    pub usage: Option<UsageSource>,
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
            usage: None,
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
        if let Some(usage) = &self.usage {
            line.push_str(&format!(" usage={}", usage.provider));
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
    evidence::fold(&evidence::of_legacy(events), 0)
}

pub fn observe_topology(events: &[TopologyEvent]) -> Observations {
    evidence::fold(&evidence::of_topology(events), 0)
}

pub(crate) fn exhausted_pools_of(events: &[TopologyEvent]) -> BTreeMap<String, Option<String>> {
    evidence::exhausted(&evidence::of_topology(events))
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
    for pool in served_pools(record) {
        exhausted.remove(pool);
    }
}

fn served_pools(record: &AttemptRecord) -> Vec<&String> {
    let worker_served = record.failure.as_ref().is_none_or(|failure| {
        failure.kind != FailureKind::Interrupted
            && !(failure.kind == FailureKind::RateLimited
                && failure.origin == crate::ladder::FailureOrigin::Worker)
    });
    let worker = record.pool.as_ref().filter(|_| worker_served);
    let reviewers = record
        .reviews
        .iter()
        .filter(|review| review.outcome != ReviewPassOutcome::Unavailable)
        .filter_map(|review| review.pool.as_ref());
    worker.into_iter().chain(reviewers).collect()
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
    for (pool, cost) in draws_of(record) {
        drain.entry(pool.clone()).or_default().add(cost);
    }
}

fn draws_of(record: &AttemptRecord) -> impl Iterator<Item = (&String, Option<f64>)> {
    record
        .pool
        .as_ref()
        .map(|pool| (pool, record.cost_usd))
        .into_iter()
        .chain(review_draws(&record.reviews))
}

fn review_draws(reviews: &[ReviewRecord]) -> impl Iterator<Item = (&String, Option<f64>)> {
    reviews
        .iter()
        .filter_map(|review| review.pool.as_ref().map(|pool| (pool, review.cost_usd)))
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
    Reported,
    SelfMetered,
    Signal,
}

impl fmt::Display for Confidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Unknown => "unknown",
            Self::Assumed => "assumed",
            Self::Reported => "provider-reported",
            Self::SelfMetered => "self-metered",
            Self::Signal => "signal",
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Reported {
    pub remaining: f64,
    pub window: String,
    pub resets_at: Option<String>,
    pub fetched_at: String,
    pub provider: UsageProvider,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Basis {
    #[default]
    Run,
    Repository,
    Machine,
}

impl Basis {
    fn drew(self) -> &'static str {
        match self {
            Self::Run => "this run drew",
            Self::Repository => "this repository's runs drew, this month,",
            Self::Machine => "upstroke on this machine drew, this month,",
        }
    }

    fn unseen(self) -> &'static str {
        match self {
            Self::Run => {
                "this counts only what upstroke spawned in this run, so earlier runs, other \
                 repositories, and your own interactive sessions have all drawn against the same \
                 allowance unseen"
            }
            Self::Repository => {
                "this counts only what upstroke spawned in this repository this calendar month \
                 (UTC), so other repositories, other machines, and your own interactive sessions \
                 have all drawn against the same allowance unseen"
            }
            Self::Machine => {
                "this counts only what upstroke spawned on this machine this calendar month \
                 (UTC), so other machines and your own interactive sessions have drawn against \
                 the same allowance unseen"
            }
        }
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
    pub basis: Basis,
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
            let drew = self.basis.drew();
            match spend.usd {
                Some(usd) => line.push_str(&format!(
                    " — {drew} ${usd:.4}{unpriced} over {} attempt(s)",
                    spend.attempts
                )),
                None => line.push_str(&format!(
                    " — {drew} {} attempt(s), none of which reported spend",
                    spend.attempts
                )),
            }
        }
        line
    }
}

pub fn estimate(pools: &[Pool], obs: &Observations) -> Vec<PoolEstimate> {
    pools
        .iter()
        .map(|pool| estimate_one(pool, obs, None, &[], Basis::Run))
        .collect()
}

pub fn estimate_gathered(pools: &[Pool], gathered: &Gathered) -> Vec<PoolEstimate> {
    pools
        .iter()
        .map(|pool| {
            estimate_one(
                pool,
                &gathered.observations,
                gathered.reported.get(&pool.name),
                gathered
                    .report_notes
                    .get(&pool.name)
                    .map_or(&[], Vec::as_slice),
                gathered.basis,
            )
        })
        .collect()
}

fn estimate_one(
    pool: &Pool,
    obs: &Observations,
    reported: Option<&Reported>,
    report_notes: &[String],
    basis: Basis,
) -> PoolEstimate {
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
                    notes.push(format!("a ceiling, not a measurement: {}", basis.unseen()));
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

    if let Some(report) = reported {
        let bound = effective_remaining(report.remaining, pool);
        let said = format!(
            "`{}` reported its {} window at {}",
            report.provider, report.window, report.fetched_at
        );
        match remaining {
            Remaining::Exhausted => notes.push(format!(
                "{said}; a rate-limit signal outranks a provider report, so the signal stands \
                 (§13)"
            )),
            Remaining::Unmetered => {}
            Remaining::AtMost(held) if held <= bound => notes.push(format!(
                "{said} at ≤{:.0}%, which is not below the self-metered ceiling; a provider \
                 report only ever lowers an estimate, so the self-metered ceiling stands (§13)",
                bound * 100.0
            )),
            Remaining::AtMost(_) | Remaining::Unknown => {
                remaining = Remaining::AtMost(bound);
                confidence = Confidence::Reported;
                reset_at = report.resets_at.clone();
                notes.push(format!(
                    "{said}; a ceiling, not a measurement: whatever has drawn on the pool since \
                     then has only lowered it, and a provider endpoint can change or fail \
                     without notice (§13)"
                ));
            }
        }
    }
    notes.extend(report_notes.iter().cloned());

    if confidence == Confidence::Unknown {
        notes.push(
            "nothing has measured this pool, so what is left is unknown — not full (§13)"
                .to_owned(),
        );
    }

    let unread: Vec<String> = pool
        .sources
        .iter()
        .filter(|source| match source {
            Source::Signals | Source::SelfMetered => false,
            Source::LocalLogs => true,
            Source::ProviderEndpoint => pool.usage.is_none(),
        })
        .map(ToString::to_string)
        .collect();
    if !unread.is_empty() {
        notes.push(format!(
            "source(s) {} are parsed but not read, so usage they would see — including your own \
             interactive sessions — is not in this figure{}",
            unread.join(", "),
            if pool.usage.is_none() && pool.sources.contains(&Source::ProviderEndpoint) {
                "; `provider-endpoint` is read once the pool names a `usage_provider` (§17)"
            } else {
                ""
            }
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
        basis,
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
        .any(|e| e.confidence >= Confidence::Reported);

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
    pub scope: Option<Scope>,
    pub now_unix: Option<u64>,
}

#[derive(Debug)]
pub struct CapacityReport {
    pub pools: Vec<Pool>,
    pub estimates: Vec<PoolEstimate>,
    pub agents: Vec<AgentStatus>,
    pub strategy: String,
    pub scope: Scope,
    pub gathered: Gathered,
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

    let scope = opts.scope.unwrap_or(config.capacity_scope);
    let gathered = machine::gather(
        &machine::GatherOptions {
            scope,
            repo_root: &opts.repo_root,
            machine_dir: config.machine_dir.as_deref(),
            pools: &config.pools,
            now_unix: opts.now_unix.unwrap_or_else(crate::util::unix_seconds_now),
        },
        &mut warnings,
    );

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

    let estimates = estimate_gathered(&config.pools, &gathered);
    Ok(CapacityReport {
        pools: config.pools,
        estimates,
        agents,
        strategy: config.strategy.mode.clone(),
        scope,
        gathered,
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
        let _ = writeln!(out, "scope: {} — {}", self.scope, self.gathered.describe());
        let _ = writeln!(out, "strategy: {}", self.strategy);
        for line in strategy_preview(&self.strategy, &self.estimates) {
            let _ = writeln!(out, "  {line}");
        }
        out
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::events::EventBody;
    use crate::topology::events::TopologyEventBody;

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

    fn started(key: crate::topology::registry::TaskKey, pool: Option<&str>) -> TopologyEvent {
        use crate::topology::events::{AttemptNumber, AttemptStarted4, GenerationId, RungBinding};
        topology(TopologyEventBody::AttemptStarted {
            data: AttemptStarted4 {
                key,
                generation: GenerationId(0),
                attempt: AttemptNumber(1),
                rung: 0,
                binding: RungBinding {
                    tier: crate::ir::Tier::Mid,
                    agent: crate::agent::claude::ADAPTER_ID.to_owned(),
                    model: "m".to_owned(),
                    pinned: false,
                    effort: crate::ir::Effort::High,
                },
                pool: pool.map(str::to_owned),
                resume_session: None,
                materialization_observed: None,
            },
        })
    }

    fn interrupted(key: crate::topology::registry::TaskKey) -> TopologyEvent {
        use crate::topology::events::{
            AttemptInterrupted4, AttemptNumber, GenerationId, LeaseDisposition,
        };
        topology(TopologyEventBody::AttemptInterrupted {
            data: AttemptInterrupted4 {
                key,
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
            vec![
                signalled("claude-max", Some(FIVE_THIRTY)),
                interrupted(ALPHA),
            ],
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

    const ALPHA: crate::topology::registry::TaskKey = crate::topology::registry::TaskKey(0);
    const BETA: crate::topology::registry::TaskKey = crate::topology::registry::TaskKey(1);
    const SCHEMA_4_RUN: &str = "01CAPACITYSCHEMA4000000001";
    const LEGACY_RUN: &str = "01CAPACITYLEGACY0000000001";
    const NOT_AN_EVENT: &[u8] = b"this committed line is not JSON\n";

    fn schema_4_plan() -> crate::ir::Plan {
        use crate::ir::{Artifact, ArtifactId, Plan, PlanSource, Task, TaskId, TaskKind};
        let task = |id: &str| Task {
            id: TaskId::from(id),
            kind: TaskKind::Refactor,
            title: format!("{id} title"),
            body: format!("{id} body"),
            depends_on: Vec::new(),
            acceptance: vec![format!("{id} passes")],
            path_hints: vec![format!("src/{id}/")],
            suggested_tier: None,
            min_tier: None,
            artifacts_in: Vec::new(),
            artifacts_out: vec![ArtifactId::from(format!("{id}-out").as_str())],
        };
        Plan {
            source: PlanSource {
                adapter: "markdown".to_owned(),
                hash: "capacity-plan-hash".to_owned(),
            },
            tasks: vec![task("alpha"), task("beta")],
            artifacts: vec![Artifact {
                id: ArtifactId::from("alpha-out"),
                produced_by: Some(TaskId::from("alpha")),
            }],
        }
    }

    fn schema_4_run_started(
        run_id: &str,
        plan: &crate::ir::Plan,
        normalized_plan_digest: &str,
    ) -> crate::topology::events::RunStarted4 {
        use crate::events::{BindingSummary, ChainSummary, GateSummary};
        use crate::gates::ShellKind;
        use crate::ir::{Effort, ResolvedEffortPolicy, Tier};
        use crate::review::{PassBinding, ReviewPlan};
        use crate::topology::events::{
            CommitSha, GitRef, IncarnationId, RunStarted4, RunnerContract, RunnerKind,
            RunnerPolicy, TopologyLimits,
        };
        use crate::topology::paths::{PathGrammar, PathPolicy, PathPolicyVersion};

        let chain = |task: &str| ChainSummary {
            task: task.to_owned(),
            tiers: vec![Tier::Mid, Tier::Frontier],
            attempts_per: 2,
            bindings: Some(vec![
                BindingSummary {
                    tier: Tier::Mid,
                    agent: crate::agent::claude::ADAPTER_ID.to_owned(),
                    model: format!("{task}-mid-model"),
                    pinned: false,
                },
                BindingSummary {
                    tier: Tier::Frontier,
                    agent: crate::agent::claude::ADAPTER_ID.to_owned(),
                    model: format!("{task}-frontier-model"),
                    pinned: true,
                },
            ]),
        };
        let unauthenticated = RunStarted4 {
            schema: crate::topology::schema::TOPOLOGY_SCHEMA,
            upstroke_version: "0.2.0-capacity".to_owned(),
            run_id: run_id.to_owned(),
            incarnation: IncarnationId("01CAPACITYINC0000000000000".to_owned()),
            runner: RunnerPolicy {
                kind: RunnerKind::Host,
                policy: RunnerContract::HostV1,
                image: None,
                credential_volumes: None,
                mounts: Vec::new(),
            },
            probed_agents: vec![
                crate::agent::claude::ADAPTER_ID.to_owned(),
                "copilot".to_owned(),
            ],
            shell: ShellKind::Bash,
            branch: format!("upstroke/run-{run_id}"),
            integration_ref: GitRef(format!("refs/heads/upstroke/run-{run_id}")),
            base_sha: CommitSha("a".repeat(40)),
            execution_root: "/var/lib/upstroke/execution".to_owned(),
            private_dir: format!("/var/lib/upstroke/runs/{run_id}"),
            plan_path: "docs/plan.md".to_owned(),
            config_path: Some("upstroke.toml".to_owned()),
            plan_hash: plan.source.hash.clone(),
            normalized_plan_digest: normalized_plan_digest.to_owned(),
            registry_digest: String::new(),
            path_policy: PathPolicy {
                version: PathPolicyVersion::V2,
                case_fold: true,
                grammar: PathGrammar::Globset,
            },
            limits: TopologyLimits {
                max_parallel: 1,
                max_defers: 2,
                max_merge_repairs: 3,
            },
            gates: vec!["fmt".to_owned()],
            gates_from_config: true,
            gate_cmds: vec![GateSummary {
                name: "fmt".to_owned(),
                cmd: "cargo fmt --check".to_owned(),
                timeout: Duration::from_secs(60),
                shell: ShellKind::Bash,
            }],
            interaction_mode: "never".to_owned(),
            chains: plan.tasks.iter().map(|t| chain(t.id.as_str())).collect(),
            effort_policy: ResolvedEffortPolicy {
                small: Effort::Low,
                mid: Effort::High,
                frontier: Effort::Max,
                review: Effort::Medium,
            },
            reviews: ReviewPlan {
                enabled: Some(true),
                alternative_available: Some(true),
                pass_timeout_secs: Some(900),
                primary: Some(PassBinding::new(crate::agent::claude::ADAPTER_ID, "opus")),
                alternative: Some(PassBinding::new("copilot", "gpt")),
                second_opinion: vec![None; plan.tasks.len()],
            },
        };
        let registry_digest = crate::topology::registry::TaskRegistry::originals_with_agents(
            plan,
            &unauthenticated.registry_record(),
            &unauthenticated.probed_agents,
        )
        .expect("the fixture record derives a registry")
        .digest();
        RunStarted4 {
            registry_digest,
            ..unauthenticated
        }
    }

    pub(crate) struct Schema4Run {
        log: crate::events::EventLog,
        path: std::path::PathBuf,
        fold: crate::topology::fold::TopologyFold,
    }

    impl Schema4Run {
        fn started(repo: &std::path::Path, run_id: &str) -> Self {
            let plan = schema_4_plan();
            let mut plan_bytes = serde_json::to_vec_pretty(&plan).expect("the plan serializes");
            plan_bytes.push(b'\n');
            let normalized_plan_digest = crate::events::normalized_plan_digest(&plan_bytes);
            let public = crate::rundir::public_dir(repo, run_id);
            let hooks = &mut crate::rundir::NoHooks;
            crate::rundir::create_public_dir(&public, hooks).expect("the run's public directory");
            crate::rundir::write_plan(&public, &plan_bytes, hooks).expect("the frozen plan");
            let path = public.join(crate::rundir::EVENT_LOG);
            let log = crate::events::EventLog::open(
                crate::topology::effects::EventSite::OpenLog,
                &path,
                &mut Vec::new(),
            )
            .expect("open the run's log");
            let started = schema_4_run_started(run_id, &plan, &normalized_plan_digest);
            let mut run = Self {
                log,
                path,
                fold: crate::topology::fold::TopologyFold::new(
                    crate::topology::fold::FrozenInputs {
                        plan,
                        normalized_plan_digest,
                    },
                ),
            };
            run.push(topology(TopologyEventBody::RunStarted {
                data: Box::new(started),
            }));
            run
        }

        #[track_caller]
        fn push(&mut self, event: TopologyEvent) {
            let (line, checked) = crate::events::log::TopologyLine::round_trip(&event)
                .expect("the event survives its own wire format");
            let delta = self.fold.plan_transition(&checked).unwrap_or_else(|error| {
                panic!(
                    "the fold refuses this fixture's `{}`: {error}",
                    checked.body.kind()
                )
            });
            self.log
                .append_topology(line.site(), &line)
                .expect("append the committed line");
            self.fold.apply_delta(delta);
        }

        #[track_caller]
        fn begin_attempt(&mut self, key: crate::topology::registry::TaskKey, pool: &str) {
            use crate::topology::events::{
                AttemptNumber, AttemptStarted4, CommitSha, GenerationId, LeaseGrant, TaskDispatched,
            };
            let paths = self
                .fold
                .predicted_region(key)
                .expect("the run has started and registers this task");
            self.push(topology(TopologyEventBody::TaskDispatched {
                data: TaskDispatched {
                    key,
                    generation: GenerationId(0),
                    base_sha: CommitSha("a".repeat(40)),
                    worktree_path: format!("/var/lib/upstroke/tasks/k{}-g0", key.0),
                    lease: LeaseGrant::Predicted { paths },
                    source_candidate: None,
                },
            }));
            let binding = self
                .fold
                .frozen_rung_binding(key, 0)
                .expect("the task's ladder has a first rung");
            self.push(topology(TopologyEventBody::AttemptStarted {
                data: AttemptStarted4 {
                    key,
                    generation: GenerationId(0),
                    attempt: AttemptNumber(1),
                    rung: 0,
                    binding,
                    pool: Some(pool.to_owned()),
                    resume_session: None,
                    materialization_observed: None,
                },
            }));
        }

        fn obliged_passes(&self, key: crate::topology::registry::TaskKey) -> Vec<&'static str> {
            self.fold
                .registry()
                .and_then(|registry| registry.get(key))
                .expect("the run has started and registers this task")
                .reviews
                .obliged_lenses()
                .into_iter()
                .map(crate::review::Lens::name)
                .collect()
        }

        fn rewritten_with(self, line: &[u8]) {
            let Self { log, path, .. } = self;
            drop(log);
            let mut bytes = crate::events::log::read_bytes(&path).expect("the log it wrote");
            bytes.extend_from_slice(line);
            crate::workspace_manager::fixture::write_file(&path, &bytes);
        }
    }

    pub(crate) fn schema_4_run_signalling(
        repo: &std::path::Path,
        run_id: &str,
        pool: &str,
        reset_at: &str,
    ) -> Schema4Run {
        let mut run = Schema4Run::started(repo, run_id);
        run.begin_attempt(ALPHA, pool);
        run.push(signalled(pool, Some(reset_at)));
        run.push(finished(failed(
            settled(Some(pool), Vec::new()),
            FailureKind::RateLimited,
            FailureOrigin::Worker,
        )));
        run
    }

    pub(crate) fn schema_4_run_limited_then_drawing(
        repo: &std::path::Path,
        run_id: &str,
        limited: (&str, &str),
        drawn: (&str, f64),
    ) -> Schema4Run {
        let (limited, reset_at) = limited;
        let (pool, usd) = drawn;
        let mut run = schema_4_run_signalling(repo, run_id, limited, reset_at);
        run.begin_attempt(BETA, pool);
        let passed = crate::events::ReviewRecord {
            outcome: ReviewPassOutcome::Passed,
            ..review_on("copilot", None)
        };
        run.push(prepared(priced(settled(Some(pool), vec![passed]), usd)));
        run
    }

    pub(crate) fn legacy_run_with_a_line_that_is_not_json(repo: &std::path::Path, run_id: &str) {
        let mut bytes = legacy_line(&legacy_run_started(run_id));
        bytes.extend_from_slice(NOT_AN_EVENT);
        let path = crate::rundir::public_dir(repo, run_id).join(crate::rundir::EVENT_LOG);
        crate::workspace_manager::fixture::write_file(&path, &bytes);
    }

    fn legacy_run_started(run_id: &str) -> Event {
        Event::now(EventBody::RunStarted {
            data: Box::new(crate::events::RunStarted {
                schema: crate::events::SCHEMA_VERSION,
                upstroke_version: "0.1.0-capacity".to_owned(),
                run_id: run_id.to_owned(),
                branch: format!("upstroke/run-{run_id}"),
                base_sha: "a".repeat(40),
                plan_path: "docs/plan.md".to_owned(),
                config_path: None,
                plan_hash: "capacity-plan-hash".to_owned(),
                normalized_plan_digest: None,
                private_dir: format!("/var/lib/upstroke/runs/{run_id}"),
                gates: Vec::new(),
                gates_from_config: false,
                interaction_mode: "never".to_owned(),
                chains: Vec::new(),
                effort_policy: None,
                gate_cmds: None,
                reviews: None,
            }),
        })
    }

    fn legacy_line(event: &Event) -> Vec<u8> {
        let mut line = serde_json::to_vec(event).expect("the event serializes");
        line.push(b'\n');
        line
    }

    fn priced(record: AttemptRecord, cost_usd: f64) -> AttemptRecord {
        AttemptRecord {
            cost_usd: Some(cost_usd),
            ..record
        }
    }

    fn priced_review(
        pass: &str,
        pool: &str,
        cost_usd: f64,
        outcome: ReviewPassOutcome,
    ) -> crate::events::ReviewRecord {
        crate::events::ReviewRecord {
            pass: pass.to_owned(),
            cost_usd: Some(cost_usd),
            outcome,
            ..review_on("copilot", Some(pool))
        }
    }

    fn claude_max_pools_file(repo: &std::path::Path) -> std::path::PathBuf {
        let pools = repo.join("pools.toml");
        crate::workspace_manager::fixture::write_file(
            &pools,
            b"[pools.claude-max]\nkind = \"subscription-window\"\nagent = \"claude-code\"\n\
              window = \"5h\"\nsources = [\"signals\", \"self\"]\n",
        );
        pools
    }

    struct NoAdapters;

    impl crate::agent::AdapterSource for NoAdapters {
        fn get(&self, _id: &str) -> Option<&dyn crate::agent::AgentAdapter> {
            None
        }
    }

    const FIXTURE_NOW: u64 = 1_794_369_600;

    fn capacity_report(repo: &std::path::Path) -> CapacityReport {
        report(
            &CapacityOptions {
                config_path: None,
                pools_path: Some(claude_max_pools_file(repo)),
                repo_root: repo.to_path_buf(),
                scope: None,
                now_unix: Some(FIXTURE_NOW),
            },
            &NoAdapters,
        )
        .expect("the capacity report")
    }

    fn folded(repo: &std::path::Path) -> (Gathered, Vec<String>) {
        let mut warnings = Vec::new();
        let gathered = machine::gather(
            &machine::GatherOptions {
                scope: Scope::Repo,
                repo_root: repo,
                machine_dir: None,
                pools: &[],
                now_unix: FIXTURE_NOW,
            },
            &mut warnings,
        );
        (gathered, warnings)
    }

    #[test]
    fn the_gatherer_holds_a_schema_4_logs_signalled_pool_at_its_reset() {
        let tree = crate::workspace_manager::fixture::scratch("capacity-schema4-signal");
        let repo = tree.path();
        let _run = schema_4_run_signalling(repo, SCHEMA_4_RUN, "claude-max", FIVE_THIRTY);

        let (gathered, warnings) = folded(repo);

        assert_eq!(warnings, Vec::<String>::new());
        assert_eq!(
            gathered.observations.exhausted,
            exhausted(&[("claude-max", Some(FIVE_THIRTY))]),
            "the legacy read refuses a schema-4 log at its second line and held no pool"
        );
        assert_eq!((gathered.runs, gathered.unread), (1, 0));
        assert_eq!(gathered.basis, Basis::Repository);

        let rendered = capacity_report(repo).render();
        assert!(
            rendered.contains(&format!(
                "claude-max: exhausted [signal] resets {FIVE_THIRTY}"
            )),
            "{rendered}"
        );
        assert!(
            rendered.contains(
                "scope: machine — signals and self-metered draw are folded from 1 run across 1 \
                 repository on this machine"
            ),
            "{rendered}"
        );
        assert!(!rendered.contains("warning:"), "{rendered}");
    }

    #[test]
    fn a_schema_4_logs_spend_counts_every_settled_attempt_once_worker_and_reviews_alike() {
        let tree = crate::workspace_manager::fixture::scratch("capacity-schema4-spend");
        let repo = tree.path();
        let mut run = Schema4Run::started(repo, SCHEMA_4_RUN);

        run.begin_attempt(ALPHA, "claude-max");
        let refused_in_review = failed(
            priced(
                settled(
                    Some("claude-max"),
                    vec![priced_review(
                        "review",
                        "copilot-pro",
                        0.25,
                        ReviewPassOutcome::Unavailable,
                    )],
                ),
                0.5,
            ),
            FailureKind::RateLimited,
            FailureOrigin::Reviewer,
        );
        run.push(finished(refused_in_review));

        run.begin_attempt(BETA, "claude-max");
        let passes = run.obliged_passes(BETA);
        assert_eq!(passes, ["review"], "the fixture's run obliges one pass");
        let accepted = priced(
            settled(
                Some("claude-max"),
                vec![priced_review(
                    "review",
                    "copilot-pro",
                    0.75,
                    ReviewPassOutcome::Passed,
                )],
            ),
            1.25,
        );
        run.push(prepared(accepted));

        let (gathered, warnings) = folded(repo);

        assert_eq!(warnings, Vec::<String>::new());
        assert_eq!((gathered.runs, gathered.unread), (1, 0));
        let spent = |usd: f64| Spend {
            usd: Some(usd),
            attempts: 2,
            unpriced: 0,
        };
        assert_eq!(
            gathered.observations.self_spend,
            BTreeMap::from([
                ("claude-max".to_owned(), spent(1.75)),
                ("copilot-pro".to_owned(), spent(1.0)),
            ]),
            "one `attempt_finished` and one `candidate_prepared`, each counted once"
        );
        assert_eq!(
            gathered.observations.exhausted,
            BTreeMap::new(),
            "the accepted attempt's review passed on the pool the earlier one was refused on"
        );
    }

    #[test]
    fn a_schema_4_logs_interrupted_attempt_counts_once_unpriced_on_the_pool_it_started_on() {
        let tree = crate::workspace_manager::fixture::scratch("capacity-schema4-interrupted");
        let repo = tree.path();
        let mut run = Schema4Run::started(repo, SCHEMA_4_RUN);

        run.begin_attempt(ALPHA, "claude-max");
        run.push(finished(failed(
            priced(settled(Some("claude-max"), Vec::new()), 1.0),
            FailureKind::RateLimited,
            FailureOrigin::Worker,
        )));

        run.begin_attempt(BETA, "claude-max");
        run.push(interrupted(BETA));

        let (gathered, warnings) = folded(repo);

        assert_eq!(warnings, Vec::<String>::new());
        assert_eq!((gathered.runs, gathered.unread), (1, 0));
        assert_eq!(
            gathered.observations.self_spend,
            BTreeMap::from([(
                "claude-max".to_owned(),
                Spend {
                    usd: Some(1.0),
                    attempts: 2,
                    unpriced: 1,
                },
            )]),
            "the interrupted attempt drew on the pool it started on, at a cost nothing recorded"
        );

        let rendered = capacity_report(repo).render();
        assert!(
            rendered.contains(
                "claude-max: exhausted [signal] — upstroke on this machine drew, this month, \
                 $1.0000? over 2 attempt(s)"
            ),
            "{rendered}"
        );
    }

    #[test]
    fn an_interrupted_attempt_whose_start_names_no_pool_adds_to_no_pools_spend() {
        let settled_on_the_pool = [
            started(ALPHA, Some("claude-max")),
            finished(failed(
                priced(settled(Some("claude-max"), Vec::new()), 1.0),
                FailureKind::RateLimited,
                FailureOrigin::Worker,
            )),
        ];
        let before = observe_topology(&settled_on_the_pool).self_spend;
        assert_eq!(
            before,
            BTreeMap::from([(
                "claude-max".to_owned(),
                Spend {
                    usd: Some(1.0),
                    attempts: 1,
                    unpriced: 0,
                },
            )]),
            "the premise: one settled attempt, priced, on the pool"
        );

        let mut no_start = settled_on_the_pool.to_vec();
        no_start.push(interrupted(BETA));
        assert_eq!(
            observe_topology(&no_start).self_spend,
            before,
            "no `attempt_started` names this attempt, and another attempt's start is not its own"
        );

        let mut no_pool = settled_on_the_pool.to_vec();
        no_pool.extend([started(BETA, None), interrupted(BETA)]);
        assert_eq!(
            observe_topology(&no_pool).self_spend,
            before,
            "its `attempt_started` names no pool"
        );
    }

    #[test]
    fn a_schema_4_logs_integration_review_passes_count_once_on_the_pools_they_drew_on() {
        use crate::events::ReviewRecord;
        use crate::topology::events::{
            CandidateRef, CommitSha, FrozenSpawn, GenerationId, GitRef, InfrastructureKind,
            MergePrepared, MergeRejected, MergeVerificationCharged, MergeVerificationInterrupted,
            MergeVerificationStarted, MergeVerificationUnavailable, PreparedDisposition,
            RejectionDisposition, RejectionLeaseEffect, SequenceId, SpawnAdmission,
            UnavailableCause, UnavailableOutcome, VerificationBasis, VerificationRecord,
            VerificationSource, VerificationVerdict,
        };
        use crate::topology::paths::PathSet;
        use crate::topology::registry::TaskKey;

        const REPAIR: TaskKey = TaskKey(2);
        let tree = crate::workspace_manager::fixture::scratch("capacity-schema4-integration");
        let run = Schema4Run::started(tree.path(), SCHEMA_4_RUN);

        let sha = |digit: &str| CommitSha(digit.repeat(40));
        let candidate = |key: TaskKey| CandidateRef {
            key,
            generation: GenerationId(0),
            commit_sha: sha("c"),
            candidate_ref: GitRef(format!("refs/upstroke/candidates/k{}", key.0)),
        };
        let pooled = |cost_usd: Option<f64>| ReviewRecord {
            cost_usd,
            outcome: ReviewPassOutcome::Passed,
            ..review_on("copilot", Some("reviewer-pool"))
        };
        let verification =
            |verdict: VerificationVerdict, reviews: Vec<ReviewRecord>| VerificationRecord {
                verdict,
                gates_passed: true,
                reviews,
                detail: "judged".to_owned(),
            };
        let verification_started = |sequence: u32, key: TaskKey| {
            topology(TopologyEventBody::MergeVerificationStarted {
                data: MergeVerificationStarted {
                    sequence: SequenceId(sequence),
                    candidate: candidate(key),
                    basis: VerificationBasis::StaleClean {
                        prepared_ref: GitRef(format!("refs/upstroke/select/prepared/{sequence}")),
                    },
                    expected_head: sha("d"),
                    proposed_sha: sha("e"),
                },
            })
        };
        let unavailable = |sequence: u32, reviews: Vec<ReviewRecord>| {
            topology(TopologyEventBody::MergeVerificationUnavailable {
                data: MergeVerificationUnavailable {
                    sequence: SequenceId(sequence),
                    cause: UnavailableCause::Infrastructure {
                        kind: InfrastructureKind::ReviewerTimeout,
                    },
                    outcome: UnavailableOutcome::Deferred { defers: 1 },
                    reviews,
                },
            })
        };
        let prepared = topology(TopologyEventBody::MergePrepared {
            data: Box::new(MergePrepared {
                sequence: SequenceId(1),
                disposition: PreparedDisposition::StaleClean,
                expected_head: sha("d"),
                proposed_sha: sha("e"),
                key: ALPHA,
                generation: GenerationId(0),
                candidate_sha: candidate(ALPHA).commit_sha,
                candidate_ref: candidate(ALPHA).candidate_ref,
                prepared_ref: Some(GitRef("refs/upstroke/select/prepared/1".to_owned())),
                verification_source: VerificationSource::Verification {
                    sequence: SequenceId(1),
                },
                verification: Some(verification(
                    VerificationVerdict::Passed,
                    vec![pooled(Some(2.0)), pooled(None)],
                )),
                satisfies: vec![ALPHA],
            }),
        });
        let repair_entry = {
            let mut entry = run
                .fold
                .registry()
                .and_then(|registry| registry.get(BETA))
                .expect("the run has started and registers beta")
                .clone();
            entry.key = REPAIR;
            entry
        };
        let rejected = topology(TopologyEventBody::MergeRejected {
            data: Box::new(MergeRejected {
                sequence: SequenceId(2),
                candidate: candidate(BETA),
                rejecting_head: sha("d"),
                disposition: RejectionDisposition::CodeRejected {
                    verification: verification(
                        VerificationVerdict::Rejected,
                        vec![pooled(Some(1.0))],
                    ),
                },
                repair: FrozenSpawn {
                    key: REPAIR,
                    entry: repair_entry,
                    admission: SpawnAdmission::Runnable,
                },
                lease_effect: RejectionLeaseEffect::CreatesLineage {
                    root: BETA,
                    paths: PathSet::RepoWide,
                },
            }),
        });
        let events = vec![
            started(ALPHA, Some("claude-max")),
            finished(failed(
                priced(settled(Some("claude-max"), Vec::new()), 1.0),
                FailureKind::RateLimited,
                FailureOrigin::Worker,
            )),
            prepared,
            rejected,
            verification_started(3, REPAIR),
            unavailable(3, vec![pooled(Some(0.5))]),
            verification_started(4, ALPHA),
            topology(TopologyEventBody::MergeVerificationCharged {
                data: MergeVerificationCharged {
                    sequence: SequenceId(4),
                    reviews: vec![pooled(Some(0.25))],
                },
            }),
            topology(TopologyEventBody::MergeVerificationInterrupted {
                data: MergeVerificationInterrupted {
                    sequence: SequenceId(4),
                    detail: "the coordinator that held this verification did not survive it"
                        .to_owned(),
                },
            }),
            unavailable(9, vec![pooled(Some(0.125))]),
        ];

        let observed = observe_topology(&events).self_spend;
        assert_eq!(
            observed,
            BTreeMap::from([
                (
                    "claude-max".to_owned(),
                    Spend {
                        usd: Some(1.0),
                        attempts: 1,
                        unpriced: 0,
                    },
                ),
                (
                    "reviewer-pool".to_owned(),
                    Spend {
                        usd: Some(3.875),
                        attempts: 6,
                        unpriced: 1,
                    },
                ),
            ]),
            "every pass of the four carriers once on the pool it drew on, and the attempt's pool \
             as its settlement alone left it"
        );

        let report = crate::engine::RunReport::from_topology(&run.fold, &events, Vec::new());
        let as_rows: Vec<crate::engine::PoolDrainRow> = observed
            .iter()
            .map(|(pool, spend)| crate::engine::PoolDrainRow {
                pool: pool.clone(),
                attempts: spend.attempts,
                cost_usd: spend.usd,
                unpriced: spend.unpriced,
            })
            .collect();
        assert_eq!(
            as_rows, report.pool_drain,
            "the capacity fold and the run report's pool rows read the same log alike"
        );
    }

    #[test]
    fn the_gatherer_folds_a_legacy_log_as_observe_over_read_all_does() {
        let tree = crate::workspace_manager::fixture::scratch("capacity-legacy-latest");
        let repo = tree.path();
        let limited = Event::now(EventBody::AttemptFinished {
            task: "t1".to_owned(),
            attempt: 1,
            rung: 0,
            profile: "p".to_owned(),
            parking: None,
            transition: None,
            prepared_commit: None,
            data: Box::new(failed(
                priced(settled(Some("claude-max"), Vec::new()), 0.5),
                FailureKind::RateLimited,
                FailureOrigin::Worker,
            )),
        });
        let signal = Event::now(EventBody::PoolExhausted {
            task: "t1".to_owned(),
            data: PoolExhausted {
                pool: "claude-max".to_owned(),
                agent: crate::agent::claude::ADAPTER_ID.to_owned(),
                reset_at: Some(FIVE_THIRTY.to_owned()),
                detail: FIVE_HOUR_LIMIT.to_owned(),
            },
        });
        let path = crate::rundir::public_dir(repo, LEGACY_RUN).join(crate::rundir::EVENT_LOG);
        let bytes: Vec<u8> = [legacy_run_started(LEGACY_RUN), limited, signal]
            .iter()
            .flat_map(legacy_line)
            .collect();
        crate::workspace_manager::fixture::write_file(&path, &bytes);

        let as_before = observe(
            &crate::events::read_all(&path, &mut Vec::new()).expect("the legacy log reads"),
        );
        assert_eq!(
            as_before,
            Observations {
                exhausted: exhausted(&[("claude-max", Some(FIVE_THIRTY))]),
                self_spend: BTreeMap::from([(
                    "claude-max".to_owned(),
                    Spend {
                        usd: Some(0.5),
                        attempts: 1,
                        unpriced: 0,
                    },
                )]),
            },
            "the premise: the legacy fold has something to report"
        );

        let (gathered, warnings) = folded(repo);

        assert_eq!(warnings, Vec::<String>::new());
        assert_eq!(gathered.observations, as_before);
        assert_eq!(
            (gathered.runs, gathered.repositories, gathered.unread),
            (1, 1, 0)
        );
    }

    #[test]
    fn a_run_neither_reader_accepts_is_reported_unread_and_shows_nothing_of_it() {
        let unread = |repo: &std::path::Path, run_id: &str| {
            let (gathered, warnings) = folded(repo);
            assert_eq!(
                gathered.observations,
                Observations::default(),
                "{warnings:?}"
            );
            assert_eq!(
                (gathered.runs, gathered.repositories, gathered.unread),
                (0, 0, 1),
                "{warnings:?}"
            );
            let [warning] = warnings.as_slice() else {
                panic!("one warning names the run and the error: {warnings:?}");
            };
            assert!(
                warning.contains(&format!("run {run_id}"))
                    && warning.contains("could not be read")
                    && warning.contains("nothing from it is shown"),
                "{warning}"
            );
            assert!(!warning.contains("signals only"), "{warning}");

            let rendered = capacity_report(repo).render();
            assert!(
                rendered.contains(
                    "scope: machine — the 1 run found on this machine could not be read, so no \
                     rate-limit signal and no self-metered draw is shown; a warning names it"
                ),
                "{rendered}"
            );
            assert!(
                !rendered.contains("no run on this machine yet"),
                "{rendered}"
            );
            assert!(
                rendered.contains(&format!("warning: {warning}")),
                "{rendered}"
            );
            assert!(
                rendered.contains("claude-max: unknown [unknown]"),
                "{rendered}"
            );
            warning.clone()
        };

        let tree = crate::workspace_manager::fixture::scratch("capacity-unread-legacy");
        legacy_run_with_a_line_that_is_not_json(tree.path(), LEGACY_RUN);
        let warning = unread(tree.path(), LEGACY_RUN);
        assert!(warning.contains("line 2 is not a valid event"), "{warning}");

        let tree = crate::workspace_manager::fixture::scratch("capacity-unread-schema4");
        schema_4_run_signalling(tree.path(), SCHEMA_4_RUN, "claude-max", FIVE_THIRTY)
            .rewritten_with(NOT_AN_EVENT);
        let warning = unread(tree.path(), SCHEMA_4_RUN);
        assert!(warning.contains("line 6"), "{warning}");
    }

    #[test]
    fn a_repository_with_no_run_reports_no_run_and_warns_of_nothing() {
        let tree = crate::workspace_manager::fixture::scratch("capacity-no-run");
        let repo = tree.path();

        let (gathered, warnings) = folded(repo);
        assert_eq!(
            gathered,
            Gathered {
                basis: Basis::Repository,
                ..Gathered::default()
            }
        );
        assert_eq!(warnings, Vec::<String>::new());
        assert!(
            gathered
                .describe()
                .starts_with("no run in this repository yet"),
            "{}",
            gathered.describe()
        );

        let rendered = capacity_report(repo).render();
        assert!(
            rendered.contains("scope: machine — no run on this machine yet"),
            "{rendered}"
        );
        assert!(!rendered.contains("could not be read"), "{rendered}");
    }

    const REFUSING_RUN: &str = "01CAPACITYREFUSING00000001";
    const SERVING_RUN: &str = "01CAPACITYSERVING000000001";
    const TORN_RUN: &str = "01CAPACITYTORN000000000001";

    fn on_the_machine(
        repo: &std::path::Path,
        machine_dir: &std::path::Path,
    ) -> (Gathered, Vec<String>) {
        let mut warnings = Vec::new();
        let gathered = machine::gather(
            &machine::GatherOptions {
                scope: Scope::Machine,
                repo_root: repo,
                machine_dir: Some(machine_dir),
                pools: &[],
                now_unix: FIXTURE_NOW,
            },
            &mut warnings,
        );
        (gathered, warnings)
    }

    fn owner_record(machine_dir: &std::path::Path, private_half: &str, bytes: &[u8]) {
        let record = machine_dir
            .join("runs")
            .join(private_half)
            .join(crate::rundir::OWNER_RECORD);
        crate::workspace_manager::fixture::write_file(&record, bytes);
    }

    fn owner_record_of(machine_dir: &std::path::Path, repo: &std::path::Path, run_id: &str) {
        let located = serde_json::json!({
            "run_id": run_id,
            "public_dir": crate::rundir::public_dir(repo, run_id),
        });
        owner_record(machine_dir, run_id, located.to_string().as_bytes());
    }

    #[test]
    fn review409_machine_signal_does_not_depend_on_launch_repository() {
        let tree = crate::workspace_manager::fixture::scratch("capacity-machine-tie");
        let machine_dir = tree.path().join("machine");
        let refusing = tree.path().join("refusing");
        let serving = tree.path().join("serving");
        let _refused = schema_4_run_signalling(&refusing, REFUSING_RUN, "shared", FIVE_THIRTY);
        let _served = schema_4_run_limited_then_drawing(
            &serving,
            SERVING_RUN,
            ("claude-max", FIVE_THIRTY),
            ("shared", 1.0),
        );
        owner_record_of(&machine_dir, &refusing, REFUSING_RUN);
        owner_record_of(&machine_dir, &serving, SERVING_RUN);

        let (from_refusing, warnings) = on_the_machine(&refusing, &machine_dir);
        assert_eq!(warnings, Vec::<String>::new());
        assert_eq!(
            (
                from_refusing.runs,
                from_refusing.repositories,
                from_refusing.unread
            ),
            (2, 2, 0)
        );
        let (from_serving, warnings) = on_the_machine(&serving, &machine_dir);
        assert_eq!(warnings, Vec::<String>::new());
        assert_eq!(
            from_refusing.observations.exhausted, from_serving.observations.exhausted,
            "one machine, one answer, whichever repository it is asked from"
        );
        assert_eq!(
            from_refusing.observations.exhausted,
            exhausted(&[
                ("claude-max", Some(FIVE_THIRTY)),
                ("shared", Some(FIVE_THIRTY)),
            ]),
            "the success on `shared` was written in the second of the other run's refusal, so it \
             proves nothing about the pool after it"
        );
    }

    #[test]
    fn unreadable_owner_must_be_counted_in_the_scope_line() {
        let tree = crate::workspace_manager::fixture::scratch("capacity-unread-owner");
        let machine_dir = tree.path().join("machine");
        owner_record(&machine_dir, TORN_RUN, b"{");

        let (gathered, warnings) = on_the_machine(&tree.path().join("repo"), &machine_dir);
        assert_eq!(
            (gathered.runs, gathered.repositories, gathered.unread),
            (0, 0, 1),
            "{warnings:?}"
        );
        let [warning] = warnings.as_slice() else {
            panic!("one warning names the record and the error: {warnings:?}");
        };
        assert!(
            warning.contains(TORN_RUN) && warning.contains("is not an owner record"),
            "{warning}"
        );
        assert_eq!(
            gathered.describe(),
            "the 1 run found on this machine could not be read, so no rate-limit signal and no \
             self-metered draw is shown; a warning names it"
        );
    }

    #[test]
    fn a_private_half_with_no_owner_record_is_not_counted() {
        let tree = crate::workspace_manager::fixture::scratch("capacity-no-owner");
        let machine_dir = tree.path().join("machine");
        crate::workspace_manager::fixture::create_dir(&machine_dir.join("runs").join(LEGACY_RUN));

        let (gathered, warnings) = on_the_machine(&tree.path().join("repo"), &machine_dir);
        assert_eq!(warnings, Vec::<String>::new());
        assert_eq!((gathered.runs, gathered.unread), (0, 0));
        assert!(
            gathered
                .describe()
                .starts_with("no run on this machine yet"),
            "{}",
            gathered.describe()
        );
    }

    #[test]
    fn an_owner_record_that_leads_nowhere_is_not_counted_for_a_run_read_from_its_repository() {
        let tree = crate::workspace_manager::fixture::scratch("capacity-listed-owner");
        let machine_dir = tree.path().join("machine");
        let repo = tree.path().join("repo");
        let _run = schema_4_run_signalling(&repo, SCHEMA_4_RUN, "claude-max", FIVE_THIRTY);
        owner_record(&machine_dir, SCHEMA_4_RUN, b"{");

        let (gathered, warnings) = on_the_machine(&repo, &machine_dir);
        assert_eq!(warnings, Vec::<String>::new());
        assert_eq!((gathered.runs, gathered.unread), (1, 0));
        assert_eq!(
            gathered.observations.exhausted,
            exhausted(&[("claude-max", Some(FIVE_THIRTY))])
        );

        let (elsewhere, warnings) = on_the_machine(&tree.path().join("elsewhere"), &machine_dir);
        assert_eq!(
            (elsewhere.runs, elsewhere.unread),
            (0, 1),
            "from another repository the same record is the only way to the run: {warnings:?}"
        );
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

    fn api_pool(allowance: f64) -> Pool {
        let mut pool = pool("api");
        pool.kind = PoolKind::ApiKey;
        pool.monthly_allowance = Allowance::Units(allowance);
        pool.usage = Some(UsageSource {
            provider: UsageProvider::AnthropicCost,
            token_env: Some("KEY".to_owned()),
            account: None,
        });
        pool
    }

    fn reported(remaining: f64) -> Reported {
        Reported {
            remaining,
            window: "month".to_owned(),
            resets_at: Some("2026-12-01T00:00:00Z".to_owned()),
            fetched_at: "2026-11-23T12:00:00Z".to_owned(),
            provider: UsageProvider::AnthropicCost,
        }
    }

    fn gathered(pool: &str, remaining: Option<f64>, drawn_usd: Option<f64>) -> Gathered {
        let mut gathered = Gathered {
            basis: Basis::Machine,
            ..Gathered::default()
        };
        if let Some(remaining) = remaining {
            gathered
                .reported
                .insert(pool.to_owned(), reported(remaining));
        }
        if let Some(usd) = drawn_usd {
            gathered.observations.self_spend.insert(
                pool.to_owned(),
                Spend {
                    usd: Some(usd),
                    attempts: 1,
                    unpriced: 0,
                },
            );
        }
        gathered
    }

    #[test]
    fn a_provider_report_sets_a_ceiling_where_nothing_else_has_one() {
        let estimates = estimate_gathered(&[api_pool(100.0)], &gathered("api", Some(0.5), None));
        assert_eq!(estimates[0].confidence, Confidence::Reported);
        let Remaining::AtMost(left) = estimates[0].remaining else {
            panic!("expected a ceiling: {:?}", estimates[0].remaining);
        };
        assert!(
            (left - 0.225).abs() < 1e-9,
            "half left, then the pool's margin and reserve like any other source: {left}"
        );
        assert_eq!(
            estimates[0].reset_at.as_deref(),
            Some("2026-12-01T00:00:00Z")
        );
        assert_eq!(
            estimates[0].describe(),
            "api: ≤22% [provider-reported] resets 2026-12-01T00:00:00Z"
        );
        assert!(
            estimates[0].notes.iter().any(|note| {
                note.starts_with(
                    "`anthropic-cost` reported its month window at 2026-11-23T12:00:00Z",
                ) && note.contains("a ceiling, not a measurement")
            }),
            "{:?}",
            estimates[0].notes
        );
        assert!(
            !estimates[0]
                .notes
                .iter()
                .any(|note| note.contains("not full")),
            "a reported pool is not an unmeasured one: {:?}",
            estimates[0].notes
        );
    }

    #[test]
    fn a_provider_report_lowers_a_self_metered_ceiling_and_never_raises_one() {
        let pool = api_pool(100.0);

        let lower = estimate_gathered(
            std::slice::from_ref(&pool),
            &gathered("api", Some(0.3), Some(20.0)),
        );
        assert_eq!(lower[0].confidence, Confidence::Reported);
        assert_eq!(
            lower[0].remaining,
            Remaining::AtMost(effective_remaining(0.3, &pool)),
            "the provider saw more drawn than this machine did, and both are ceilings"
        );

        let higher = estimate_gathered(
            std::slice::from_ref(&pool),
            &gathered("api", Some(0.95), Some(20.0)),
        );
        assert_eq!(higher[0].confidence, Confidence::SelfMetered);
        assert_eq!(
            higher[0].remaining,
            Remaining::AtMost(effective_remaining(0.8, &pool)),
            "a report that allows more than what upstroke itself drew cannot talk the ceiling up"
        );
        assert_eq!(higher[0].reset_at, None, "nor lend it its reset");
        assert!(
            higher[0]
                .notes
                .iter()
                .any(|note| note.contains("only ever lowers an estimate")),
            "{:?}",
            higher[0].notes
        );

        let equal = estimate_gathered(
            std::slice::from_ref(&pool),
            &gathered("api", Some(0.8), Some(20.0)),
        );
        assert_eq!(
            equal[0].confidence,
            Confidence::SelfMetered,
            "at a tie the higher-ranked source keeps the estimate"
        );
    }

    #[test]
    fn a_provider_report_never_speaks_over_a_signal_or_about_an_unmetered_pool() {
        let pool = api_pool(100.0);
        let mut signalled = gathered("api", Some(0.9), None);
        signalled
            .observations
            .exhausted
            .insert("api".to_owned(), Some("2026-11-23T17:00:00Z".to_owned()));
        let estimates = estimate_gathered(std::slice::from_ref(&pool), &signalled);
        assert_eq!(estimates[0].remaining, Remaining::Exhausted);
        assert_eq!(estimates[0].confidence, Confidence::Signal);
        assert_eq!(
            estimates[0].reset_at.as_deref(),
            Some("2026-11-23T17:00:00Z"),
            "the signal's reset, not the report's"
        );
        assert!(
            estimates[0]
                .notes
                .iter()
                .any(|note| note.contains("a rate-limit signal outranks a provider report")),
            "{:?}",
            estimates[0].notes
        );

        let mut local = api_pool(100.0);
        local.kind = PoolKind::Unmetered;
        let estimates = estimate_gathered(&[local], &gathered("api", Some(0.1), None));
        assert_eq!(estimates[0].remaining, Remaining::Unmetered);
        assert_eq!(estimates[0].confidence, Confidence::Assumed);
    }

    #[test]
    fn a_report_of_everything_drawn_is_a_ceiling_of_zero_and_never_the_word_exhausted() {
        let estimates = estimate_gathered(&[api_pool(100.0)], &gathered("api", Some(0.0), None));
        assert_eq!(estimates[0].remaining, Remaining::AtMost(0.0));
        assert_eq!(estimates[0].confidence, Confidence::Reported);
        assert!(
            estimates[0]
                .describe()
                .starts_with("api: ≤0% [provider-reported]"),
            "only a rate-limit signal says exhausted (§13): {}",
            estimates[0].describe()
        );
        let lines = strategy_preview("conserve", &estimates);
        assert!(
            !lines.iter().any(|line| line.starts_with("exhausted now:")),
            "{lines:?}"
        );
        assert!(
            !lines
                .iter()
                .any(|line| line.contains("no pool has a measured estimate")),
            "a provider's reading is a measurement, of the provider's: {lines:?}"
        );
    }

    #[test]
    fn what_a_stored_report_could_not_say_is_a_note_on_its_pool() {
        let mut unusable = gathered("api", None, None);
        unusable.report_notes.insert(
            "api".to_owned(),
            vec!["the `month` window of the report reset".to_owned()],
        );
        let estimates = estimate_gathered(&[api_pool(100.0)], &unusable);
        assert_eq!(estimates[0].remaining, Remaining::Unknown);
        assert_eq!(
            estimates[0].notes,
            vec![
                "the `month` window of the report reset".to_owned(),
                "nothing has measured this pool, so what is left is unknown — not full (§13)"
                    .to_owned(),
            ]
        );
    }

    #[test]
    fn the_trust_order_puts_a_provider_report_below_self_metering_and_above_a_shape() {
        let mut ranked = [
            Confidence::Signal,
            Confidence::Reported,
            Confidence::Unknown,
            Confidence::SelfMetered,
            Confidence::Assumed,
        ];
        ranked.sort();
        assert_eq!(
            ranked,
            [
                Confidence::Unknown,
                Confidence::Assumed,
                Confidence::Reported,
                Confidence::SelfMetered,
                Confidence::Signal,
            ]
        );
    }

    #[test]
    fn each_basis_says_whose_draw_it_counted_and_what_it_could_not_see() {
        let pool = api_pool(100.0);
        let cases = [
            (
                Basis::Run,
                "this run drew $50.0000 over 1 attempt(s)",
                "in this run, so earlier runs, other repositories, and your own",
            ),
            (
                Basis::Repository,
                "this repository's runs drew, this month, $50.0000 over 1 attempt(s)",
                "in this repository this calendar month (UTC), so other repositories, other machines",
            ),
            (
                Basis::Machine,
                "upstroke on this machine drew, this month, $50.0000 over 1 attempt(s)",
                "on this machine this calendar month (UTC), so other machines and your own",
            ),
        ];
        for (basis, drew, unseen) in cases {
            let mut folded = gathered("api", None, Some(50.0));
            folded.basis = basis;
            let estimates = estimate_gathered(std::slice::from_ref(&pool), &folded);
            let line = estimates[0].describe();
            assert!(line.ends_with(drew), "{basis:?}: {line}");
            let note = estimates[0]
                .notes
                .iter()
                .find(|note| note.starts_with("a ceiling, not a measurement: "))
                .unwrap_or_else(|| panic!("{basis:?}: {:?}", estimates[0].notes));
            assert!(note.contains(unseen), "{basis:?}: {note}");
            for text in estimates[0].notes.iter().chain(std::iter::once(&line)) {
                assert!(
                    !text.contains("  "),
                    "{basis:?}: a string continued across lines lost its continuation and \
                     kept its indentation: {text:?}"
                );
            }
        }
        let machine = estimate_gathered(
            std::slice::from_ref(&pool),
            &gathered("api", None, Some(50.0)),
        );
        assert!(
            !machine[0]
                .notes
                .iter()
                .any(|note| note.contains("other repositories")),
            "the machine basis folds other repositories, so it must not list them as unseen: {:?}",
            machine[0].notes
        );
        assert_eq!(
            estimate(std::slice::from_ref(&pool), &Observations::default())[0].basis,
            Basis::Run,
            "one run's own fold is the run basis"
        );
    }

    #[test]
    fn a_listed_provider_endpoint_is_unread_until_the_pool_names_a_provider() {
        let mut listed = pool("claude-max");
        listed.sources = vec![Source::Signals, Source::ProviderEndpoint];
        let estimates = estimate(std::slice::from_ref(&listed), &Observations::default());
        assert!(
            estimates[0].notes.iter().any(|note| {
                note.contains("source(s) provider-endpoint are parsed but not read")
                    && note.contains("once the pool names a `usage_provider`")
            }),
            "{:?}",
            estimates[0].notes
        );

        let mut named = api_pool(100.0);
        named.sources = vec![Source::Signals, Source::ProviderEndpoint, Source::LocalLogs];
        let estimates = estimate(std::slice::from_ref(&named), &Observations::default());
        let unread: Vec<&String> = estimates[0]
            .notes
            .iter()
            .filter(|note| note.contains("parsed but not read"))
            .collect();
        assert_eq!(unread.len(), 1, "{:?}", estimates[0].notes);
        assert!(
            unread[0].starts_with("source(s) local-logs are parsed but not read")
                && !unread[0].contains("usage_provider"),
            "a pool that names its provider has that source read: {unread:?}"
        );
    }

    #[test]
    fn a_usage_provider_is_spelled_exactly_and_every_accepted_spelling_parses() {
        assert_eq!(UsageProvider::ALL.len(), UsageProvider::ACCEPTED.len());
        for (provider, spelling) in UsageProvider::ALL.into_iter().zip(UsageProvider::ACCEPTED) {
            assert_eq!(provider.to_string(), spelling);
            assert_eq!(UsageProvider::parse(spelling), Some(provider));
        }
        for near in [
            "Anthropic-Cost",
            " openai-cost",
            "openai-cost ",
            "openai_cost",
            "",
        ] {
            assert_eq!(UsageProvider::parse(near), None, "{near:?}");
        }
        assert!(!UsageProvider::AnthropicCost.needs_account());
        assert!(!UsageProvider::OpenaiCost.needs_account());
        assert!(UsageProvider::GithubAiCredits.needs_account());
        assert!(UsageProvider::GithubPremiumRequests.needs_account());
    }

    #[test]
    fn usage_keys_become_a_source_or_a_defect_that_says_which_key() {
        assert_eq!(UsageSource::from_keys(None, None, None), Ok(None));
        assert_eq!(
            UsageSource::from_keys(
                Some("github-ai-credits"),
                Some("GH_TOKEN"),
                Some("octo-cat")
            ),
            Ok(Some(UsageSource {
                provider: UsageProvider::GithubAiCredits,
                token_env: Some("GH_TOKEN".to_owned()),
                account: Some("octo-cat".to_owned()),
            }))
        );
        assert_eq!(
            UsageSource::from_keys(None, Some("KEY"), Some("octo")),
            Err(UsageKeyDefect::NoProvider {
                stray: vec!["usage_token_env", "usage_account"],
            })
        );
        assert_eq!(
            UsageSource::from_keys(Some("claude-subscription"), None, None),
            Err(UsageKeyDefect::UnknownProvider {
                written: "claude-subscription".to_owned(),
            })
        );
        assert_eq!(
            UsageSource::from_keys(Some("openai-cost"), Some("sk-admin-LEAK"), None),
            Err(UsageKeyDefect::TokenEnvNotAName)
        );
        assert!(
            !UsageKeyDefect::TokenEnvNotAName
                .to_string()
                .contains("LEAK"),
            "the refusal carries no field to repeat a pasted key from"
        );
        assert_eq!(
            UsageSource::from_keys(Some("github-ai-credits"), Some("KEY"), Some("octo/cat")),
            Err(UsageKeyDefect::AccountNotAName)
        );
    }

    #[test]
    fn a_token_env_with_a_credentials_shape_is_refused_and_never_repeated() {
        use UsageKeyDefect::{TokenEnvLooksLikeACredential, TokenEnvNotAName};

        for (prefix, refusal) in [
            ("github_pat_", TokenEnvLooksLikeACredential),
            ("ghp_", TokenEnvLooksLikeACredential),
            ("gho_", TokenEnvLooksLikeACredential),
            ("ghu_", TokenEnvLooksLikeACredential),
            ("ghs_", TokenEnvLooksLikeACredential),
            ("ghr_", TokenEnvLooksLikeACredential),
            ("sk-", TokenEnvNotAName),
            ("sk-ant-", TokenEnvNotAName),
            ("xoxb-", TokenEnvNotAName),
        ] {
            let value = format!("{prefix}EXAMPLE_0nly");
            assert!(
                looks_like_a_credential(&value),
                "a value starting `{prefix}` has a credential's shape"
            );
            let defect = match UsageSource::from_keys(Some("openai-cost"), Some(&value), None) {
                Ok(_) => panic!("a value starting `{prefix}` was taken for a variable's name"),
                Err(defect) => defect,
            };
            assert_eq!(defect, refusal, "a value starting `{prefix}`");
            let message = defect.to_string();
            assert!(
                !message.contains(&value),
                "the refusal of a value starting `{prefix}` repeats it"
            );
            assert!(
                message
                    .starts_with("`usage_token_env` must be the NAME of an environment variable"),
                "{message}"
            );
        }

        let over_long = "K".repeat(65);
        assert!(
            names_an_environment_variable(&over_long),
            "by its characters alone a 65-character value is a name"
        );
        assert!(looks_like_a_credential(&over_long));
        let defect = match UsageSource::from_keys(Some("openai-cost"), Some(&over_long), None) {
            Ok(_) => panic!("a 65-character value was taken for a variable's name"),
            Err(defect) => defect,
        };
        assert_eq!(defect, TokenEnvLooksLikeACredential);
        assert!(
            !defect.to_string().contains(&over_long),
            "the refusal of a 65-character value repeats it"
        );

        let longest = "K".repeat(64);
        for ordinary in [
            "GITHUB_TOKEN",
            "ANTHROPIC_ADMIN_KEY",
            "GHP_TOKEN",
            longest.as_str(),
        ] {
            assert!(!looks_like_a_credential(ordinary), "{ordinary}");
            assert_eq!(
                UsageSource::from_keys(Some("openai-cost"), Some(ordinary), None),
                Ok(Some(UsageSource {
                    provider: UsageProvider::OpenaiCost,
                    token_env: Some(ordinary.to_owned()),
                    account: None,
                })),
                "{ordinary}"
            );
        }
    }

    #[test]
    fn a_source_says_what_it_still_lacks_for_a_refresh() {
        let source = |provider, token_env: Option<&str>, account: Option<&str>| UsageSource {
            provider,
            token_env: token_env.map(str::to_owned),
            account: account.map(str::to_owned),
        };
        assert!(
            source(UsageProvider::OpenaiCost, Some("KEY"), None)
                .gaps()
                .is_empty()
        );
        assert!(
            source(UsageProvider::GithubAiCredits, Some("KEY"), Some("octo"))
                .gaps()
                .is_empty()
        );
        let no_credential = source(UsageProvider::OpenaiCost, None, None).gaps();
        assert_eq!(no_credential.len(), 1);
        assert!(
            no_credential[0].contains("no `usage_token_env`"),
            "{no_credential:?}"
        );
        let nothing = source(UsageProvider::GithubPremiumRequests, None, None).gaps();
        assert_eq!(nothing.len(), 2, "{nothing:?}");
        assert!(nothing[1].contains("no `usage_account`"), "{nothing:?}");
        let extra = source(UsageProvider::AnthropicCost, Some("KEY"), Some("acme")).gaps();
        assert_eq!(extra.len(), 1);
        assert!(extra[0].contains("does not read (ignored)"), "{extra:?}");
    }

    #[test]
    fn a_variable_name_and_an_account_name_are_narrow_character_classes() {
        for name in ["KEY", "_k", "ANTHROPIC_ADMIN_KEY_2", "a"] {
            assert!(names_an_environment_variable(name), "{name}");
        }
        let long = "K".repeat(MAX_USAGE_KEY_CHARS + 1);
        for not in [
            "",
            "2KEY",
            "MY-KEY",
            "MY KEY",
            "KEY=1",
            "sk-ant-x",
            "ключ",
            long.as_str(),
        ] {
            assert!(!names_an_environment_variable(not), "{not:?}");
        }
        assert!(names_an_environment_variable(
            &"K".repeat(MAX_USAGE_KEY_CHARS)
        ));

        for name in ["octo-cat", "octo_cat.1", "0x", "A"] {
            assert!(names_an_account(name), "{name}");
        }
        for not in [
            "",
            ".hidden",
            "-flag",
            "a/b",
            "a b",
            "a?b",
            "a#b",
            "a%2Fb",
            "a\nb",
            "é",
            long.as_str(),
        ] {
            assert!(!names_an_account(not), "{not:?}");
        }
    }

    #[test]
    fn a_pool_says_which_provider_answers_for_it() {
        assert!(
            api_pool(100.0)
                .describe()
                .contains(" usage=anthropic-cost "),
            "{}",
            api_pool(100.0).describe()
        );
        assert!(!pool("claude-max").describe().contains("usage="));
    }

    #[test]
    fn one_runs_fold_is_the_same_through_the_evidence_as_it_was_before_it() {
        use crate::events::{Event, EventBody};

        let finished = |record: AttemptRecord| {
            Event::now(EventBody::AttemptFinished {
                task: "t1".to_owned(),
                attempt: record.attempt,
                rung: 0,
                profile: "p".to_owned(),
                parking: None,
                transition: None,
                prepared_commit: None,
                data: Box::new(record),
            })
        };
        let interrupted = |record: AttemptRecord| {
            Event::now(EventBody::AttemptInterrupted {
                task: "t1".to_owned(),
                attempt: record.attempt,
                rung: 0,
                profile: "p".to_owned(),
                data: Box::new(record),
            })
        };
        let mut priced = settled(
            Some("claude-max"),
            vec![review_on("codex", Some("codex-pro"))],
        );
        priced.cost_usd = Some(1.5);
        priced.reviews[0].cost_usd = Some(0.25);
        let cut_short = failed(
            settled(Some("claude-max"), Vec::new()),
            FailureKind::Interrupted,
            FailureOrigin::Worker,
        );
        let events = [finished(priced.clone()), interrupted(cut_short.clone())];

        let obs = observe(&events);
        assert_eq!(
            obs.self_spend,
            drain_of([&priced, &cut_short]),
            "the estimator's self-metered source and the ledger's per-pool column are one \
             number, whichever of the two folds produced it"
        );
        assert_eq!(
            obs.self_spend.get("claude-max"),
            Some(&Spend {
                usd: Some(1.5),
                attempts: 2,
                unpriced: 1,
            })
        );
        assert_eq!(
            obs.self_spend.get("codex-pro"),
            Some(&Spend {
                usd: Some(0.25),
                attempts: 1,
                unpriced: 0,
            })
        );
    }
}
