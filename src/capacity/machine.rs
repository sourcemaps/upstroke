//! Machine-level capacity observation (DESIGN.md §13).
//!
//! A subscription is the operator's, not a repository's: two repositories on
//! one machine drain the same Claude Max window, and a rate limit one of them
//! hit an hour ago is a fact about the pool the other is about to bind. So the
//! capacity views fold **every run on this machine**, not the latest run of
//! the repository they were started in, and add what the capacity tracker
//! stored from the provider's own usage endpoint.
//!
//! Nothing is recorded twice to make that possible. Every schema-4 run already
//! keeps its private half under the machine directory
//! (`~/.upstroke/runs/<run-id>/`, §15) with an `owner.json` naming its public
//! directory; that is the index, and each run's `events.jsonl` stays the one
//! source of truth for what it did. A run of schemas 1–3 has no such record,
//! so it is found only from inside its own repository.
//!
//! This module reads. It writes nothing, starts no process and opens no
//! socket; the provider reports it reads were written by `src/tracker.rs`,
//! which is the only part of upstroke that speaks HTTP (§4.2).

#![forbid(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros
)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;

use super::evidence::{self, Evidence};
use super::usage::{self, UsageReport};
use super::{Allowance, Basis, Observations, Pool, Reported, UsageSource};

/// Which runs feed the capacity views: `[capacity] scope` (§17).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Scope {
    /// Every run of every repository on this machine. The default.
    #[default]
    Machine,
    /// This repository's runs alone.
    Repo,
}

impl Scope {
    pub const ACCEPTED: [&'static str; 2] = ["machine", "repo"];

    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "machine" => Some(Self::Machine),
            "repo" => Some(Self::Repo),
            _ => None,
        }
    }
}

impl fmt::Display for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Machine => "machine",
            Self::Repo => "repo",
        })
    }
}

/// A window that names no reset bounds what is left only briefly: without a
/// reset there is no telling when its usage starts over, so past this age the
/// reading is dropped rather than trusted.
pub const UNDATED_WINDOW_MAX_AGE: Duration = Duration::from_secs(15 * 60);

/// How far ahead of this machine's clock a report may be dated before it is
/// taken for a clock fault rather than a report.
pub const CLOCK_SKEW_ALLOWANCE: Duration = Duration::from_secs(5 * 60);

/// Everything the capacity views fold, and what it was folded from.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Gathered {
    /// Signals and self-metered draw over `basis`. The draw counts the
    /// current calendar month (UTC), which is the period `monthly_allowance`
    /// is measured against.
    pub observations: Observations,
    /// Per pool, the ceiling the tracker's stored provider report still
    /// supports.
    pub reported: BTreeMap<String, Reported>,
    /// Per pool, what a stored report could not say, and why.
    pub report_notes: BTreeMap<String, Vec<String>>,
    pub basis: Basis,
    /// Event logs folded.
    pub runs: usize,
    /// Distinct repositories those logs belong to.
    pub repositories: usize,
    /// Runs that were found and could not be read: a log neither reader
    /// accepts, or an owner record under the machine directory that exists
    /// and leads to no log, because it cannot be read, is not an owner record,
    /// names another run or names no run directory. Each has a warning naming
    /// it, and [`Gathered::describe`] counts them. A private half with no
    /// owner record is not counted, being a run of schemas 1–3 or a creation
    /// that never wrote one, and neither is the record of a run this
    /// repository lists, which is read from there.
    pub unread: usize,
}

/// What to gather, and from where.
#[derive(Debug, Clone, Copy)]
pub struct GatherOptions<'a> {
    pub scope: Scope,
    pub repo_root: &'a Path,
    /// The machine directory, `~/.upstroke` by default: `runs/` under it
    /// indexes every schema-4 run, `capacity/usage/` holds the tracker's
    /// reports. `None` when no home directory resolves; then only this
    /// repository's runs are found and no report is read.
    pub machine_dir: Option<&'a Path>,
    pub pools: &'a [Pool],
    pub now_unix: u64,
}

/// Fold every run in scope and read every stored provider report.
///
/// Never fails: a run whose log cannot be read is skipped with a warning and
/// counted in [`Gathered::unread`], as is an owner record that leads to no
/// log, and a report that cannot be read is a warning and a note on its pool.
/// What is left is an estimate over less evidence, which every estimate here
/// already says it is.
///
/// Which repository it is started in changes what is in scope and nothing
/// about how it is folded: each run is handed to [`evidence::merge`] keyed by
/// its id and then its log's path, so that evidence of one second from two
/// runs is ordered by what it is and whose it is, never by which run was
/// listed first.
#[must_use]
pub fn gather(opts: &GatherOptions<'_>, warnings: &mut Vec<String>) -> Gathered {
    let mut logs = repository_logs(opts.repo_root);
    let mut unread = 0;
    let basis = match (opts.scope, opts.machine_dir) {
        (Scope::Machine, Some(machine_dir)) => {
            let (indexed, unfollowed) = machine_logs(machine_dir, &logs, warnings);
            logs.extend(indexed);
            unread += unfollowed;
            Basis::Machine
        }
        (Scope::Machine, None) => {
            warnings.push(
                "no home directory resolves, so there is no machine directory to find other \
                 repositories' runs in; capacity is folded from this repository's runs alone"
                    .to_owned(),
            );
            Basis::Repository
        }
        (Scope::Repo, _) => Basis::Repository,
    };

    let mut seen: BTreeSet<PathBuf> = BTreeSet::new();
    let mut repositories: BTreeSet<PathBuf> = BTreeSet::new();
    let mut runs = Vec::new();
    for run in logs {
        // Canonical, so that a run listed from its repository and again from
        // its owner record is folded once. A path that does not resolve is
        // kept as written: the read below then fails and names the run, which
        // is where a missing log is reported.
        let log = run.log();
        let identity = std::fs::canonicalize(&log).unwrap_or(log);
        if seen.contains(&identity) {
            continue;
        }
        let found = evidence_in(&run);
        let repository = repository_of(&identity);
        seen.insert(identity.clone());
        match found {
            Ok(found) => {
                repositories.insert(repository);
                runs.push(((run.run_id, identity), found));
            }
            Err(why) => {
                unread += 1;
                warnings.push(format!(
                    "run {} could not be read ({why}); nothing from it is shown, neither its \
                     rate-limit signals nor its self-metered spend",
                    run.run_id
                ));
            }
        }
    }
    let folded = runs.len();
    let observations = evidence::fold(&evidence::merge(runs), month_start(opts.now_unix));

    let mut gathered = Gathered {
        observations,
        basis,
        runs: folded,
        repositories: repositories.len(),
        unread,
        ..Gathered::default()
    };
    if let Some(machine_dir) = opts.machine_dir {
        read_reports(
            &usage::store_dir(machine_dir),
            opts,
            &mut gathered,
            warnings,
        );
    }
    gathered
}

impl Gathered {
    /// One line saying what the estimates were folded from, and what was
    /// found and could not be.
    ///
    /// A run that could not be read is never passed over in silence: with
    /// none folded the line says the runs that exist could not be read, not
    /// that there are none, and with some folded it says how many are
    /// missing. The warnings name each.
    #[must_use]
    pub fn describe(&self) -> String {
        let counted = |count: usize, one: &str, many: &str| match count {
            1 => format!("1 {one}"),
            count => format!("{count} {many}"),
        };
        let place = match self.basis {
            Basis::Run => {
                return "signals and self-metered draw are folded from one run".to_owned();
            }
            Basis::Machine => "on this machine",
            Basis::Repository => "in this repository",
        };
        let folded = match (self.runs, self.unread) {
            (0, 0) => {
                return format!(
                    "no run {place} yet, so nothing has been self-metered and no rate-limit \
                     signal is known"
                );
            }
            (0, 1) => {
                return format!(
                    "the 1 run found {place} could not be read, so no rate-limit signal and no \
                     self-metered draw is shown; a warning names it"
                );
            }
            (0, unread) => {
                return format!(
                    "none of the {unread} runs found {place} could be read, so no rate-limit \
                     signal and no self-metered draw is shown; the warnings name them"
                );
            }
            (runs, _) => match self.basis {
                Basis::Machine => format!(
                    "signals and self-metered draw are folded from {} across {} {place}; the \
                     draw counts this calendar month (UTC)",
                    counted(runs, "run", "runs"),
                    counted(self.repositories, "repository", "repositories"),
                ),
                Basis::Run | Basis::Repository => format!(
                    "signals and self-metered draw are folded from {} {place}; the draw counts \
                     this calendar month (UTC)",
                    counted(runs, "run", "runs"),
                ),
            },
        };
        match self.unread {
            0 => folded,
            1 => format!(
                "{folded}; 1 more run could not be read and nothing from it is shown (a warning \
                 names it)"
            ),
            unread => format!(
                "{folded}; {unread} more runs could not be read and nothing from them is shown \
                 (the warnings name them)"
            ),
        }
    }
}

/// One run to fold: its id and the repository it belongs to, which is what
/// the readers below are given.
struct RunLog {
    run_id: String,
    repo_root: PathBuf,
}

impl RunLog {
    fn log(&self) -> PathBuf {
        crate::rundir::public_dir(&self.repo_root, &self.run_id).join(crate::rundir::EVENT_LOG)
    }
}

fn repository_logs(repo_root: &Path) -> Vec<RunLog> {
    crate::rundir::list_runs(repo_root)
        .into_iter()
        .map(|run_id| RunLog {
            run_id,
            repo_root: repo_root.to_path_buf(),
        })
        .collect()
}

/// The repository a public run directory belongs to.
///
/// `<repo>/.upstroke/runs/<run-id>` is three components below its repository.
/// `None` for a directory of any other shape: a run is read by repository and
/// id, and the reader derives the directory, so a record naming a directory
/// that derivation would not arrive at names nothing it can read.
fn repository_holding(public: &Path, run_id: &str) -> Option<PathBuf> {
    let root = public.parent()?.parent()?.parent()?;
    (crate::rundir::public_dir(root, run_id) == public).then(|| root.to_path_buf())
}

/// The part of a run's `owner.json` (§15) that says where its log is.
///
/// Read with a view of its own rather than the creation protocol's
/// `OwnerRecord`: that type refuses a field it does not know, which is right
/// for a census deciding whether a directory may be deleted and wrong for a
/// reader that only wants a path and must keep working when the record grows.
#[derive(Debug, Deserialize)]
struct Located {
    run_id: String,
    public_dir: String,
}

/// The runs the machine index locates, and how many of its owner records
/// lead to no log.
///
/// `listed` are the runs this repository lists. One of them is read from
/// there, so a record of it that leads nowhere leaves nothing out of the
/// estimates, and it is neither warned of nor counted.
fn machine_logs(
    machine_dir: &Path,
    listed: &[RunLog],
    warnings: &mut Vec<String>,
) -> (Vec<RunLog>, usize) {
    let runs = machine_dir.join("runs");
    let entries = match std::fs::read_dir(&runs) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return (Vec::new(), 0),
        Err(error) => {
            warnings.push(format!(
                "{} could not be listed ({error}), so runs in other repositories are missing \
                 from the estimates below",
                runs.display()
            ));
            return (Vec::new(), 0);
        }
    };
    let mut names: Vec<std::ffi::OsString> = Vec::new();
    for entry in entries {
        match entry {
            Ok(entry) => names.push(entry.file_name()),
            Err(error) => {
                warnings.push(format!(
                    "{} could not be listed to its end ({error}), so some runs in other \
                     repositories may be missing from the estimates below",
                    runs.display()
                ));
                break;
            }
        }
    }
    names.sort();

    let read_from_here = |name: &std::ffi::OsStr| {
        listed
            .iter()
            .any(|run| name.to_str() == Some(run.run_id.as_str()))
    };
    let mut logs = Vec::new();
    let mut unfollowed = 0;
    for name in names {
        match locate(&runs.join(&name).join(crate::rundir::OWNER_RECORD), &name) {
            Ok(located) => logs.extend(located),
            Err(_) if read_from_here(&name) => {}
            Err(why) => {
                warnings.push(why);
                unfollowed += 1;
            }
        }
    }
    (logs, unfollowed)
}

/// The run the owner record at `record`, in the private half `name`, leads
/// to: `None` where there is no record, and why it leads nowhere where there
/// is one that does.
fn locate(record: &Path, name: &std::ffi::OsStr) -> Result<Option<RunLog>, String> {
    let bytes = match crate::util::read_file_bounded(record) {
        Ok(bytes) => bytes,
        // No owner record: a run of schemas 1-3, or a creation that never
        // reached it. Neither can be located from here, and neither is a
        // fault worth a line on every listing.
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
            ) =>
        {
            return Ok(None);
        }
        Err(error) => {
            return Err(format!(
                "{} could not be read ({error}), so that run is missing from the estimates \
                 below",
                record.display()
            ));
        }
    };
    let located: Located = serde_json::from_slice(&bytes).map_err(|error| {
        format!(
            "{} is not an owner record ({error}), so that run is missing from the estimates \
             below",
            record.display()
        )
    })?;
    if name.to_str() != Some(located.run_id.as_str()) {
        return Err(format!(
            "{} names run `{}`, not the directory it is in, so it is not followed",
            record.display(),
            located.run_id
        ));
    }
    let public = PathBuf::from(&located.public_dir);
    let Some(repo_root) = repository_holding(&public, &located.run_id) else {
        return Err(format!(
            "{} names `{}`, which is not that run's directory under a repository's \
             `.upstroke/runs/`, so it is not followed",
            record.display(),
            located.public_dir
        ));
    };
    Ok(Some(RunLog {
        run_id: located.run_id,
        repo_root,
    }))
}

/// `<repo>/.upstroke/runs/<run-id>/events.jsonl` is four components below its
/// repository. A log somewhere else counts as its own repository rather than
/// being guessed into one.
fn repository_of(log: &Path) -> PathBuf {
    log.ancestors()
        .nth(4)
        .map_or_else(|| log.to_path_buf(), Path::to_path_buf)
}

/// One run's capacity evidence, read by the reader of its log's own schema.
///
/// A schema-4 log is read by the observer `status` reads it with
/// (`engine::topology::observe`): a stable read of its committed lines and a
/// checked replay against the run's frozen inputs, so a log the engine would
/// refuse to resume contributes nothing here either, and the caller says so.
/// It never truncates, syncs or locks, so a run that is live in another
/// repository is read as it stands. What it folds is what `status`'s ledger
/// folds (`Observed::accounting_events`): the committed log of a run that
/// nothing holds and that has not finished is followed by the interrupted
/// settlements its resume will append, so each attempt it left in flight
/// counts once, unpriced, on the pool its `attempt_started` named, and a run
/// another process holds is folded as committed. A log of schemas 1–3 is read
/// by the sequential reader.
///
/// An incomplete final line is not reported from here: both readers fold the
/// committed lines, which is all the evidence there is, and what a torn tail
/// means for the run is `status`'s to say.
fn evidence_in(run: &RunLog) -> Result<Vec<Evidence>, String> {
    match crate::engine::topology::observe::observe(&run.repo_root, &run.run_id) {
        Ok(Some(observed)) => Ok(evidence::of_topology(&observed.accounting_events())),
        Ok(None) => {
            let mut torn_tail = Vec::new();
            crate::events::read_all(&run.log(), &mut torn_tail)
                .map(|events| evidence::of_legacy(&events))
                .map_err(|error| error.to_string())
        }
        Err(error) => Err(error.to_string()),
    }
}

/// Unix seconds of 00:00:00 UTC on the first day of `now`'s calendar month.
#[must_use]
pub fn month_start(now_unix: u64) -> u64 {
    let days = i64::try_from(now_unix / 86_400).unwrap_or(0);
    let (year, month, _) = crate::util::civil_from_days(days);
    let first = crate::util::days_from_civil(year, month, 1);
    u64::try_from(first).map_or(0, |days| days.saturating_mul(86_400))
}

fn read_reports(
    store: &Path,
    opts: &GatherOptions<'_>,
    gathered: &mut Gathered,
    warnings: &mut Vec<String>,
) {
    for pool in opts.pools {
        let Some(source) = &pool.usage else {
            continue;
        };
        let mut notes = Vec::new();
        match usage::read(store, &pool.name) {
            Ok(None) => notes.push(format!(
                "no provider report is stored for this pool; `upstroke capacity --refresh` asks \
                 `{}` for one",
                source.provider
            )),
            Err(error) => {
                warnings.push(format!("pool `{}`: {error}", pool.name));
                notes.push(
                    "the stored provider report could not be read, so it bounds nothing; \
                     `upstroke capacity --refresh` replaces it"
                        .to_owned(),
                );
            }
            Ok(Some(report)) => {
                let (reading, why_not) = reading(&report, pool, opts.now_unix);
                if let Some(reading) = reading {
                    gathered.reported.insert(pool.name.clone(), reading);
                }
                notes.extend(why_not);
            }
        }
        if !notes.is_empty() {
            gathered.report_notes.insert(pool.name.clone(), notes);
        }
    }
}

/// What a stored report still bounds for `pool` at `now`, and what it cannot.
///
/// A provider report is a point-in-time reading, and what makes an old one
/// still worth holding is that **usage inside one window only grows**: a
/// month with 60 of 100 drawn at noon has at least 60 drawn at one, so "at
/// most 40% left" stays true for as long as the window does not reset. That
/// is the whole validity rule. A window whose reset has passed says nothing (its usage
/// started over); a window that names no reset is believed for
/// [`UNDATED_WINDOW_MAX_AGE`] and then dropped; a report dated ahead of this
/// machine's clock, from a provider the pool no longer names, or about an
/// account other than the one the pool now names (`other_account`), says
/// nothing at all. Where several windows still stand, the tightest binds,
/// since a pool is spent when any one of its windows is.
///
/// The fraction returned is raw: the caller applies the pool's margin and
/// reserve.
#[must_use]
pub fn reading(
    report: &UsageReport,
    pool: &Pool,
    now_unix: u64,
) -> (Option<Reported>, Vec<String>) {
    let mut notes = Vec::new();
    let Some(source) = &pool.usage else {
        return (None, notes);
    };
    let fetched_at = crate::util::rfc3339_utc(report.fetched_at);
    if report.provider != source.provider.to_string() {
        notes.push(format!(
            "the stored provider report came from `{}` and the pool now names `{}`, so it \
             bounds nothing; `upstroke capacity --refresh` replaces it",
            report.provider, source.provider
        ));
        return (None, notes);
    }
    if let Some(why) = other_account(report, source) {
        notes.push(why);
        return (None, notes);
    }
    if report.fetched_at > now_unix.saturating_add(CLOCK_SKEW_ALLOWANCE.as_secs()) {
        notes.push(format!(
            "the stored provider report is dated {fetched_at}, ahead of this machine's clock, \
             so it bounds nothing"
        ));
        return (None, notes);
    }
    let age = now_unix.saturating_sub(report.fetched_at);

    let mut binding: Option<(f64, &usage::UsageWindow)> = None;
    for window in &report.windows {
        match window.resets_at {
            Some(reset) if reset <= now_unix => {
                notes.push(format!(
                    "the `{}` window of the report taken {fetched_at} reset at {}, so it says \
                     nothing about the window open now",
                    window.label,
                    crate::util::rfc3339_utc(reset)
                ));
                continue;
            }
            None if age > UNDATED_WINDOW_MAX_AGE.as_secs() => {
                notes.push(format!(
                    "the `{}` window of the report taken {fetched_at} names no reset, and a \
                     reading that old cannot be told from one a reset has emptied",
                    window.label
                ));
                continue;
            }
            Some(_) | None => {}
        }
        let left = match pool.monthly_allowance {
            Allowance::Units(allowance) if allowance > 0.0 => 1.0 - window.used / allowance,
            Allowance::Units(_) | Allowance::Auto => {
                notes.push(format!(
                    "`{}` reports {} used in its `{}` window, and the pool's \
                     `monthly_allowance` is \"auto\", so there is nothing to measure that \
                     against; give the allowance in the same units to turn it into a ceiling",
                    report.provider, window.used, window.label
                ));
                continue;
            }
        };
        let left = if left.is_finite() {
            left.clamp(0.0, 1.0)
        } else {
            0.0
        };
        if binding.is_none_or(|(held, _)| left < held) {
            binding = Some((left, window));
        }
    }
    if report.windows.is_empty() {
        notes.push(format!(
            "the report `{}` gave at {fetched_at} names no quota window, so it bounds nothing",
            report.provider
        ));
    }
    let reported = binding.map(|(remaining, window)| Reported {
        remaining,
        window: window.label.clone(),
        resets_at: window.resets_at.map(crate::util::rfc3339_utc),
        fetched_at,
        provider: source.provider,
    });
    (reported, notes)
}

/// Why `report` is not about the account `source` now names, or `None` when
/// it is.
///
/// "Usage inside one window only grows" is true of one account. A provider
/// that reports per account answers for the account in its address, so a
/// reading of `account-a` says nothing about `account-b`, however recent,
/// and a pool's `usage_account` can be changed after its report was taken.
/// So a report binds only while the account it records is the one
/// [`usage::account_for`] answers for the pool as configured now, which is
/// the account a refresh would ask about. One that records none, for a
/// provider that reads one, was stored before accounts were recorded and
/// cannot be told apart from another account's. A provider that reports per
/// key reads no account, so a report that records one for it is not one the
/// tracker wrote.
fn other_account(report: &UsageReport, source: &UsageSource) -> Option<String> {
    let found = report.account.as_deref();
    if found == usage::account_for(source) {
        return None;
    }
    let provider = source.provider;
    let refresh = "`upstroke capacity --refresh` replaces it";
    Some(match (found, source.account.as_deref()) {
        (Some(found), _) if !provider.needs_account() => format!(
            "the stored provider report names account `{found}`, and `{provider}` reports per key \
             and not per account, so it bounds nothing; {refresh}"
        ),
        (Some(found), Some(named)) => format!(
            "the stored provider report came from account `{found}` and the pool now names \
             `{named}`, so it bounds nothing; {refresh}"
        ),
        (Some(found), None) => format!(
            "the stored provider report came from account `{found}` and the pool now names no \
             `usage_account`, so it bounds nothing"
        ),
        (None, _) => format!(
            "the stored provider report predates account recording, so which account it \
             measured is not known and it bounds nothing; {refresh}"
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::super::usage::UsageWindow;
    use super::super::{PoolKind, Source, UsageProvider};
    use super::*;

    const NOON: u64 = 1_795_435_200;
    const HOUR: u64 = 3_600;

    fn pool() -> Pool {
        let mut pool = Pool::discovered(
            "anthropic-api",
            PoolKind::ApiKey,
            "claude-code",
            vec![Source::Signals, Source::SelfMetered],
        );
        pool.monthly_allowance = Allowance::Units(100.0);
        pool.usage = Some(UsageSource {
            provider: UsageProvider::AnthropicCost,
            token_env: Some("TOKEN".to_owned()),
            account: None,
        });
        pool
    }

    fn window(label: &str, used: f64, resets_at: Option<u64>) -> UsageWindow {
        UsageWindow {
            label: label.to_owned(),
            used,
            resets_at,
        }
    }

    fn stored(fetched_at: u64, windows: Vec<UsageWindow>) -> UsageReport {
        UsageReport {
            pool: "anthropic-api".to_owned(),
            provider: "anthropic-cost".to_owned(),
            account: None,
            fetched_at,
            windows,
        }
    }

    /// A pool whose provider reports per account, naming `account`.
    fn per_account(account: Option<&str>) -> Pool {
        Pool {
            name: "copilot".to_owned(),
            usage: Some(UsageSource {
                provider: UsageProvider::GithubAiCredits,
                token_env: Some("TOKEN".to_owned()),
                account: account.map(str::to_owned),
            }),
            ..pool()
        }
    }

    /// That provider's report of 90 of the pool's 100 drawn this month, taken
    /// at noon for `account`.
    fn per_account_report(account: Option<&str>) -> UsageReport {
        UsageReport {
            pool: "copilot".to_owned(),
            provider: "github-ai-credits".to_owned(),
            account: account.map(str::to_owned),
            ..stored(NOON, vec![window("month", 90.0, Some(NOON + 200 * HOUR))])
        }
    }

    #[test]
    fn the_tightest_window_still_open_is_the_ceiling() {
        let report = stored(
            NOON,
            vec![
                window("day", 25.0, Some(NOON + 3 * HOUR)),
                window("month", 75.0, Some(NOON + 90 * HOUR)),
            ],
        );
        let (bound, notes) = reading(&report, &pool(), NOON + HOUR);
        let bound = bound.expect("two windows are open");
        assert!((bound.remaining - 0.25).abs() < 1e-9, "{bound:?}");
        assert_eq!(bound.window, "month");
        assert_eq!(
            bound.resets_at.as_deref(),
            Some(crate::util::rfc3339_utc(NOON + 90 * HOUR).as_str())
        );
        assert_eq!(bound.fetched_at, "2026-11-23T12:00:00Z");
        assert_eq!(bound.provider, UsageProvider::AnthropicCost);
        assert!(notes.is_empty(), "{notes:?}");
    }

    #[test]
    fn an_old_report_still_bounds_a_window_that_has_not_reset() {
        let report = stored(NOON, vec![window("month", 60.0, Some(NOON + 100 * HOUR))]);
        let (bound, _) = reading(&report, &pool(), NOON + 99 * HOUR);
        assert!(
            bound.is_some_and(|bound| (bound.remaining - 0.4).abs() < 1e-9),
            "usage inside one window only grows, so four days on the ceiling still holds"
        );
    }

    #[test]
    fn a_window_that_has_reset_bounds_nothing_and_the_others_still_do() {
        let report = stored(
            NOON,
            vec![
                window("day", 90.0, Some(NOON + HOUR)),
                window("month", 50.0, Some(NOON + 90 * HOUR)),
            ],
        );
        let (bound, notes) = reading(&report, &pool(), NOON + HOUR);
        let bound = bound.expect("the month window is still open");
        assert_eq!(bound.window, "month");
        assert!((bound.remaining - 0.5).abs() < 1e-9);
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert!(
            notes
                .iter()
                .any(|note| note.contains("`day` window") && note.contains("reset at")),
            "{notes:?}"
        );

        let (bound, notes) = reading(&report, &pool(), NOON + 90 * HOUR);
        assert_eq!(bound, None);
        assert_eq!(notes.len(), 2, "{notes:?}");
    }

    #[test]
    fn a_window_naming_no_reset_is_believed_briefly_and_then_dropped() {
        let report = stored(NOON, vec![window("month", 0.0, None)]);
        let limit = UNDATED_WINDOW_MAX_AGE.as_secs();
        let (fresh, _) = reading(&report, &pool(), NOON + limit);
        assert!(fresh.is_some_and(|bound| bound.remaining == 1.0 && bound.resets_at.is_none()));
        let (stale, notes) = reading(&report, &pool(), NOON + limit + 1);
        assert_eq!(stale, None);
        assert!(
            notes.iter().any(|note| note.contains("names no reset")),
            "{notes:?}"
        );
    }

    #[test]
    fn an_amount_becomes_a_ceiling_only_against_a_known_allowance() {
        let report = stored(NOON, vec![window("month", 25.0, Some(NOON + 200 * HOUR))]);
        let (bound, notes) = reading(&report, &pool(), NOON);
        assert!(bound.is_some_and(|bound| (bound.remaining - 0.75).abs() < 1e-9));
        assert!(notes.is_empty(), "{notes:?}");

        let mut unsized_pool = pool();
        unsized_pool.monthly_allowance = Allowance::Auto;
        let (unpriced, notes) = reading(&report, &unsized_pool, NOON);
        assert_eq!(
            unpriced, None,
            "25 of an unknown allowance is not a fraction"
        );
        assert!(
            notes.iter().any(|note| note.contains("\"auto\"")),
            "{notes:?}"
        );

        let overspent = stored(NOON, vec![window("month", 250.0, Some(NOON + 200 * HOUR))]);
        let (bound, _) = reading(&overspent, &pool(), NOON);
        assert!(
            bound.is_some_and(|bound| bound.remaining == 0.0),
            "more drawn than the allowance holds is nothing left, never a negative share"
        );
    }

    #[test]
    fn a_report_from_another_provider_or_from_the_future_bounds_nothing() {
        let open = vec![window("month", 50.0, Some(NOON + 3 * HOUR))];
        let mut other = stored(NOON, open.clone());
        other.provider = "openai-cost".to_owned();
        let (bound, notes) = reading(&other, &pool(), NOON);
        assert_eq!(bound, None);
        assert!(
            notes.iter().any(|note| note.contains("`openai-cost`")),
            "{notes:?}"
        );

        let skew = CLOCK_SKEW_ALLOWANCE.as_secs();
        let ahead = stored(NOON + skew + 1, open.clone());
        let (bound, notes) = reading(&ahead, &pool(), NOON);
        assert_eq!(bound, None);
        assert!(
            notes
                .iter()
                .any(|note| note.contains("ahead of this machine's clock")),
            "{notes:?}"
        );
        let within = stored(NOON + skew, open);
        assert!(reading(&within, &pool(), NOON).0.is_some());

        let mut unasked = pool();
        unasked.usage = None;
        assert_eq!(
            reading(&within, &unasked, NOON),
            (None, Vec::new()),
            "a pool that names no provider reads no report"
        );
    }

    #[test]
    fn a_report_binds_only_while_the_pool_names_the_account_it_was_fetched_for() {
        let report = per_account_report(Some("account-a"));
        let (bound, notes) = reading(&report, &per_account(Some("account-a")), NOON + HOUR);
        assert!(
            bound.is_some_and(|bound| (bound.remaining - 0.1).abs() < 1e-9),
            "the pool still names the account the report was fetched for, so its reading stands"
        );
        assert!(notes.is_empty(), "{notes:?}");

        let (bound, notes) = reading(&report, &per_account(Some("account-b")), NOON + HOUR);
        assert_eq!(
            bound, None,
            "account-a's reading says nothing about account-b"
        );
        assert_eq!(
            notes,
            vec![
                "the stored provider report came from account `account-a` and the pool now names \
                 `account-b`, so it bounds nothing; `upstroke capacity --refresh` replaces it"
                    .to_owned()
            ]
        );

        let (bound, notes) = reading(&report, &per_account(None), NOON + HOUR);
        assert_eq!(
            bound, None,
            "a pool that names no account names none of the report's"
        );
        assert!(
            notes.iter().any(|note| {
                note.contains("came from account `account-a`")
                    && note.contains("the pool now names no `usage_account`")
            }),
            "{notes:?}"
        );
    }

    #[test]
    fn an_old_report_with_no_account_bounds_nothing_where_the_provider_reads_one() {
        let stored_before = serde_json::json!({
            "schema": 1,
            "pool": "copilot",
            "provider": "github-ai-credits",
            "fetched_at": crate::util::rfc3339_utc(NOON),
            "windows": [{
                "label": "month",
                "used_amount": 90.0,
                "resets_at": crate::util::rfc3339_utc(NOON + 200 * HOUR),
            }],
        })
        .to_string();
        let report = UsageReport::decode(stored_before.as_bytes())
            .expect("a report stored before accounts were recorded still decodes");
        assert_eq!(report, per_account_report(None), "and records no account");

        let (bound, notes) = reading(&report, &per_account(Some("account-a")), NOON + HOUR);
        assert_eq!(bound, None, "there is no telling which account it measured");
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert!(
            notes[0].contains("predates account recording")
                && notes[0].ends_with("`upstroke capacity --refresh` replaces it"),
            "{notes:?}"
        );

        let per_key = UsageReport::decode(
            stored_before
                .replacen("github-ai-credits", "anthropic-cost", 1)
                .as_bytes(),
        )
        .expect("a per-key provider's report from before decodes too");
        let (bound, notes) = reading(&per_key, &pool(), NOON + HOUR);
        assert!(
            bound.is_some_and(|bound| (bound.remaining - 0.1).abs() < 1e-9),
            "a provider that reports per key has no account to record, so its earlier reports \
             still bind: {notes:?}"
        );
    }

    #[test]
    fn a_provider_that_reports_per_key_is_held_to_no_account() {
        let ignored = Pool {
            usage: Some(UsageSource {
                provider: UsageProvider::AnthropicCost,
                token_env: Some("TOKEN".to_owned()),
                account: Some("octo-cat".to_owned()),
            }),
            ..pool()
        };
        let open = vec![window("month", 50.0, Some(NOON + 3 * HOUR))];
        let (bound, notes) = reading(&stored(NOON, open.clone()), &ignored, NOON);
        assert!(
            bound.is_some(),
            "a `usage_account` the provider does not read changes nothing about its report: \
             {notes:?}"
        );

        let recorded = UsageReport {
            account: Some("octo-cat".to_owned()),
            ..stored(NOON, open)
        };
        let (bound, notes) = reading(&recorded, &ignored, NOON);
        assert_eq!(
            bound, None,
            "the tracker records no account for this provider"
        );
        assert!(
            notes
                .iter()
                .any(|note| note.contains("`anthropic-cost` reports per key")),
            "{notes:?}"
        );
    }

    #[test]
    fn a_report_with_no_window_says_so() {
        let (bound, notes) = reading(&stored(NOON, Vec::new()), &pool(), NOON);
        assert_eq!(bound, None);
        assert!(
            notes
                .iter()
                .any(|note| note.contains("names no quota window")),
            "{notes:?}"
        );
    }

    #[test]
    fn the_month_starts_at_midnight_utc_on_the_first() {
        assert_eq!(
            crate::util::rfc3339_utc(month_start(NOON)),
            "2026-11-01T00:00:00Z"
        );
        let first = month_start(NOON);
        assert_eq!(month_start(first), first);
        assert_eq!(
            crate::util::rfc3339_utc(month_start(first - 1)),
            "2026-10-01T00:00:00Z"
        );
        assert_eq!(month_start(0), 0);
    }

    #[test]
    fn a_log_is_attributed_to_the_repository_four_levels_above_it() {
        let log = Path::new("/work/alpha/.upstroke/runs/01RUN/events.jsonl");
        assert_eq!(repository_of(log), Path::new("/work/alpha"));
        let short = Path::new("events.jsonl");
        assert_eq!(repository_of(short), short);
    }

    #[test]
    fn scope_spellings_round_trip() {
        for scope in [Scope::Machine, Scope::Repo] {
            assert_eq!(Scope::parse(&scope.to_string()), Some(scope));
        }
        assert_eq!(Scope::parse(" MACHINE "), Some(Scope::Machine));
        assert_eq!(Scope::parse("global"), None);
        assert_eq!(Scope::default(), Scope::Machine);
    }
}
