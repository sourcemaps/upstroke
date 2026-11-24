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

use crate::events::Event;
use crate::topology::events::TopologyEvent;

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
    /// Runs folded, each from one log however many logs carry its id
    /// ([`gather`]).
    pub runs: usize,
    /// Distinct repositories the logs folded belong to.
    pub repositories: usize,
    /// Runs that were found and could not be read: a run none of whose logs
    /// either reader accepts, or an owner record under the machine directory
    /// that exists and leads to no log, because it cannot be read, is not an
    /// owner record, names another run or names no run directory. Each has a
    /// warning naming it, and [`Gathered::describe`] counts them. A run is
    /// counted once, by its id, and not at all when another of its logs was
    /// folded: the log that could not be read is then named in a warning
    /// beside the one folded. A private half with no owner record is not
    /// counted, being a run of schemas 1–3 or a creation that never wrote one,
    /// and neither is the record of a run this repository lists, which is
    /// read from there.
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
/// Never fails: a run none of whose logs can be read is skipped with a warning
/// and counted in [`Gathered::unread`], as is an owner record that leads to no
/// log, and a report that cannot be read is a warning and a note on its pool.
/// What is left is an estimate over less evidence, which every estimate here
/// already says it is.
///
/// A run is folded once, by its id. A repository copied to a second place
/// with its run artifacts holds a second log of each of its runs, and while
/// the machine's owner record still names the first, both are found from the
/// copy; folded once per path, every draw in them counted twice. Of the logs
/// that carry one id, `log_to_fold` says which is folded.
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

    let mut listed: BTreeMap<PathBuf, RunLog> = BTreeMap::new();
    for run in logs {
        // Canonical, so that a log listed from its repository and again from
        // its owner record is one log, still the one that record names. A
        // path that does not resolve is kept as written: the read then fails
        // and names the run, which is where a missing log is reported.
        let log = run.log();
        let identity = std::fs::canonicalize(&log).unwrap_or(log);
        if let Some(held) = listed.get_mut(&identity) {
            if held.run_id == run.run_id {
                held.named_by_owner |= run.named_by_owner;
            }
            continue;
        }
        listed.insert(identity, run);
    }
    let mut copies: BTreeMap<String, Vec<(PathBuf, RunLog)>> = BTreeMap::new();
    for (identity, run) in listed {
        copies
            .entry(run.run_id.clone())
            .or_default()
            .push((identity, run));
    }

    let mut repositories: BTreeSet<PathBuf> = BTreeSet::new();
    let mut runs = Vec::new();
    for (run_id, logs) in copies {
        match log_to_fold(&run_id, logs, warnings) {
            Some((log, found)) => {
                repositories.insert(repository_of(&log));
                runs.push(((run_id, log), found));
            }
            None => unread += 1,
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

/// One log of a run to fold: the run's id and the repository the log is in,
/// which is what the readers below are given.
struct RunLog {
    run_id: String,
    repo_root: PathBuf,
    /// Whether the machine's owner record for the run names this log, which
    /// is what decides between two logs of one run that differ.
    named_by_owner: bool,
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
            named_by_owner: false,
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
        named_by_owner: true,
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

/// The one of `copies` that run `run_id` is folded from, and the evidence
/// read from it. `copies` are the logs that carry the run's id, each by its
/// canonical path. `None` when none of them could be read; a warning then
/// names each.
///
/// Of the logs that could be read, the one folded is the one the machine's
/// owner record names, else the one holding the most events, else the first
/// by path. That reads the logs and the record and nothing else, so it is the
/// same choice whichever repository the command was started in and whatever
/// order the logs were listed in. Logs that hold the same events are one run
/// seen twice, and nothing is said of them. A log that holds other events, or
/// that could not be read, is named in a warning beside the log folded, and
/// nothing from it is shown.
///
/// Today a run has at most two logs in scope, the one this repository lists
/// and the one its owner record names, so where both can be read the record
/// decides; the other two rules make the choice total.
fn log_to_fold(
    run_id: &str,
    copies: Vec<(PathBuf, RunLog)>,
    warnings: &mut Vec<String>,
) -> Option<(PathBuf, Vec<Evidence>)> {
    let mut read = Vec::new();
    let mut unreadable = Vec::new();
    for (log, run) in copies {
        match evidence_in(&run) {
            Ok(found) => read.push((log, run.named_by_owner, found)),
            Err(why) => unreadable.push((log, why)),
        }
    }
    // The log the owner record names, then more events, then the path.
    read.sort_by(
        |(one, one_named, one_read), (other, other_named, other_read)| {
            other_named
                .cmp(one_named)
                .then_with(|| other_read.events.count().cmp(&one_read.events.count()))
                .then_with(|| one.cmp(other))
        },
    );
    unreadable.sort();
    let mut ranked = read.into_iter();
    let Some((folded, named, found)) = ranked.next() else {
        for (_, why) in unreadable {
            warnings.push(format!(
                "run {run_id} could not be read ({why}); nothing from it is shown, neither its \
                 rate-limit signals nor its self-metered spend"
            ));
        }
        return None;
    };
    for (other, _, theirs) in ranked {
        if theirs.events == found.events {
            continue;
        }
        let why = if named {
            "the log the run's owner record names"
        } else if found.events.count() > theirs.events.count() {
            "the log holding more events"
        } else {
            "the first by path"
        };
        warnings.push(format!(
            "run {run_id} is folded once, from {}, {why}; {} logs it too, with other events, \
             and nothing from that log is shown",
            folded.display(),
            other.display()
        ));
    }
    for (other, why) in unreadable {
        warnings.push(format!(
            "run {run_id} is folded once, from {}; it is also listed at {}, which could not be \
             read ({why})",
            folded.display(),
            other.display()
        ));
    }
    Some((folded, found.evidence))
}

/// What one log gave: the evidence to fold, and the committed events that
/// tell it from another log of its run.
struct Read {
    evidence: Vec<Evidence>,
    events: Logged,
}

/// A log's committed events, as the reader of its schema parsed them.
#[derive(PartialEq)]
enum Logged {
    Topology(Vec<TopologyEvent>),
    Legacy(Vec<Event>),
}

impl Logged {
    fn count(&self) -> usize {
        match self {
            Self::Topology(events) => events.len(),
            Self::Legacy(events) => events.len(),
        }
    }
}

/// One log's capacity evidence, read by the reader of its own schema, and its
/// committed events.
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
/// The events kept beside the evidence are the committed ones
/// (`Observed::events`), never the settlements `accounting_events` adds: those
/// are dated when the log is read, so two reads of one log would not hold the
/// same events, nor would two copies of it.
///
/// An incomplete final line is not reported from here: both readers fold the
/// committed lines, which is all the evidence there is, and what a torn tail
/// means for the run is `status`'s to say.
fn evidence_in(run: &RunLog) -> Result<Read, String> {
    match crate::engine::topology::observe::observe(&run.repo_root, &run.run_id) {
        Ok(Some(observed)) => Ok(Read {
            evidence: evidence::of_topology(&observed.accounting_events()),
            events: Logged::Topology(observed.events().to_vec()),
        }),
        Ok(None) => {
            let mut torn_tail = Vec::new();
            crate::events::read_all(&run.log(), &mut torn_tail)
                .map(|events| Read {
                    evidence: evidence::of_legacy(&events),
                    events: Logged::Legacy(events),
                })
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
/// machine's clock, or fetched from another source than the one the pool now
/// names (another provider, credential variable or account: `other_source`),
/// says nothing at all. Where several windows still stand, the tightest
/// binds, since a pool is spent when any one of its windows is.
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
    if let Some(why) = other_source(report, source) {
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

/// Why `report` was not fetched from the source `source` names now, or
/// `None` when it was. It is the one place `reading` decides whom a report
/// is about.
///
/// "Usage inside one window only grows" is true of one source, and a pool's
/// usage keys can be changed after its report was taken. So a report binds
/// only while each part of the source it records is the pool's as
/// configured now:
///
/// - its provider, the endpoint that answered;
/// - where the provider reports per account, its account. Such a provider
///   answers for the account in its address, so a reading of `account-a`
///   says nothing about `account-b`, however recent;
/// - for every provider, the variable its credential was read from. A
///   provider that reports per key answers for the organization its key
///   belongs to, so a reading taken with the key in `ORG_A_ADMIN_KEY` says
///   nothing about the organization of the key in `ORG_B_ADMIN_KEY`.
///
/// The account and the variable are held to what [`usage::account_for`] and
/// [`usage::token_env_for`] answer for the pool now, the helpers the tracker
/// records them with, so writer and reader cannot disagree. Each must be
/// known on both sides to match: a report that records none was stored
/// before it was recorded and cannot be told apart from another source's,
/// and a pool that names none is never refreshed, so no report is about
/// what it names. `None` never matches `None`. A provider that reports per
/// key reads no account, so a report that records one for it is not one the
/// tracker wrote, and one that records none is about no account to begin
/// with.
///
/// The variable is matched by its name, which is all a report holds of it:
/// a variable that now holds another key under the same name is not seen.
fn other_source(report: &UsageReport, source: &UsageSource) -> Option<String> {
    let provider = source.provider;
    let refresh = "`upstroke capacity --refresh` replaces it";
    let skipped = "`upstroke capacity --refresh` will not run for this pool until it names one";
    if report.provider != provider.to_string() {
        return Some(format!(
            "the stored provider report came from `{}` and the pool now names `{provider}`, so it \
             bounds nothing; {refresh}",
            report.provider
        ));
    }
    let account = match (report.account.as_deref(), usage::account_for(source)) {
        (Some(found), _) if !provider.needs_account() => Some(format!(
            "the stored provider report names account `{found}`, and `{provider}` reports per key \
             and not per account, so it bounds nothing; {refresh}"
        )),
        (None, _) if !provider.needs_account() => None,
        (Some(found), Some(named)) if found == named => None,
        (Some(found), Some(named)) => Some(format!(
            "the stored provider report came from account `{found}` and the pool now names \
             `{named}`, so it bounds nothing; {refresh}"
        )),
        (Some(found), None) => Some(format!(
            "the stored provider report came from account `{found}` and the pool now names no \
             `usage_account`, so it bounds nothing; {skipped}"
        )),
        (None, Some(_)) => Some(format!(
            "the stored provider report predates account recording, so which account it \
             measured is not known and it bounds nothing; {refresh}"
        )),
        (None, None) => Some(format!(
            "the stored provider report predates account recording and the pool names no \
             `usage_account`, so which account it measured is not known and it bounds nothing; \
             {skipped}"
        )),
    };
    if let Some(why) = account {
        return Some(why);
    }
    match (report.token_env.as_deref(), usage::token_env_for(source)) {
        (Some(found), Some(named)) if found == named => None,
        (Some(found), Some(named)) => Some(format!(
            "the stored provider report was fetched with the credential in `{found}` and the \
             pool's `usage_token_env` now names `{named}`, so it bounds nothing; {refresh}"
        )),
        (Some(found), None) => Some(format!(
            "the stored provider report was fetched with the credential in `{found}` and the \
             pool now names no `usage_token_env`, so it bounds nothing; {skipped}"
        )),
        (None, Some(_)) => Some(format!(
            "the stored provider report predates recording which variable its credential was \
             read from, so whose usage it measured is not known and it bounds nothing; {refresh}"
        )),
        (None, None) => Some(format!(
            "the stored provider report predates recording which variable its credential was \
             read from and the pool names no `usage_token_env`, so whose usage it measured is \
             not known and it bounds nothing; {skipped}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{schema_4_run_limited_then_drawing, schema_4_run_signalling};
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

    /// A report as the tracker writes one for [`pool`]: its provider, the
    /// variable its credential was read from, and no account.
    fn stored(fetched_at: u64, windows: Vec<UsageWindow>) -> UsageReport {
        UsageReport {
            pool: "anthropic-api".to_owned(),
            provider: "anthropic-cost".to_owned(),
            account: None,
            token_env: Some("TOKEN".to_owned()),
            fetched_at,
            windows,
        }
    }

    /// [`pool`], reading its credential from `variable`.
    fn keyed(variable: Option<&str>) -> Pool {
        Pool {
            usage: Some(UsageSource {
                provider: UsageProvider::AnthropicCost,
                token_env: variable.map(str::to_owned),
                account: None,
            }),
            ..pool()
        }
    }

    /// A report stored before the tracker recorded any part of its source,
    /// from `provider`, of 90 of the pool's 100 drawn this month at noon.
    fn stored_before_sources_were_recorded(pool: &str, provider: &str) -> String {
        serde_json::json!({
            "schema": 1,
            "pool": pool,
            "provider": provider,
            "fetched_at": crate::util::rfc3339_utc(NOON),
            "windows": [{
                "label": "month",
                "used_amount": 90.0,
                "resets_at": crate::util::rfc3339_utc(NOON + 200 * HOUR),
            }],
        })
        .to_string()
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
        assert_eq!(
            report,
            UsageReport {
                token_env: None,
                ..per_account_report(None)
            },
            "and records no account, nor the variable its credential was read from"
        );

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
        assert_eq!(
            bound, None,
            "a provider that reports per key has no account to record, and a report from before \
             records no variable either, so which key it was fetched with is not known"
        );
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert!(
            notes[0].contains("predates recording which variable its credential was read from")
                && notes[0].ends_with("`upstroke capacity --refresh` replaces it"),
            "{notes:?}"
        );
    }

    /// Finding PR409-USAGE-CACHE-MISSING-ACCOUNT's first witness, ported into
    /// the crate. A GitHub report stored before accounts were recorded holds
    /// no account, and a pool that names no `usage_account` has none either.
    /// The two compared equal, `None` to `None`, so a report of no known
    /// account bound as the pool's ceiling, although a refresh skips that
    /// pool and so would never replace it.
    #[test]
    fn missing_identity_invalidates_legacy_github_report() {
        let legacy = UsageReport::decode(
            stored_before_sources_were_recorded("copilot", "github-ai-credits").as_bytes(),
        )
        .expect("a legacy report still decodes");
        assert_eq!(
            (legacy.account.as_deref(), legacy.token_env.as_deref()),
            (None, None)
        );
        let unnamed = per_account(None);
        let expected = vec![
            "the stored provider report predates account recording and the pool names no \
             `usage_account`, so which account it measured is not known and it bounds nothing; \
             `upstroke capacity --refresh` will not run for this pool until it names one"
                .to_owned(),
        ];

        let (bound, notes) = reading(&legacy, &unnamed, NOON + HOUR);
        assert_eq!(
            bound, None,
            "a report of no known account is no account's ceiling"
        );
        assert_eq!(notes, expected);

        let keyed_only = UsageReport {
            token_env: Some("TOKEN".to_owned()),
            ..legacy
        };
        assert_eq!(
            reading(&keyed_only, &unnamed, NOON + HOUR),
            (None, expected),
            "the pool's own variable does not stand in for the account the provider reads"
        );
    }

    #[test]
    fn a_report_binds_only_while_the_pool_names_the_variable_it_was_fetched_with() {
        let report = UsageReport {
            token_env: Some("ORG_A_ADMIN_KEY".to_owned()),
            ..stored(NOON, vec![window("month", 90.0, Some(NOON + 200 * HOUR))])
        };
        let (bound, notes) = reading(&report, &keyed(Some("ORG_A_ADMIN_KEY")), NOON + HOUR);
        assert!(
            bound.is_some_and(|bound| (bound.remaining - 0.1).abs() < 1e-9),
            "the pool still reads its credential from the variable the report was fetched with"
        );
        assert!(notes.is_empty(), "{notes:?}");

        let (bound, notes) = reading(&report, &keyed(Some("ORG_B_ADMIN_KEY")), NOON + HOUR);
        assert_eq!(
            bound, None,
            "the key in ORG_A_ADMIN_KEY measured its own organization, not ORG_B_ADMIN_KEY's"
        );
        assert_eq!(
            notes,
            vec![
                "the stored provider report was fetched with the credential in \
                 `ORG_A_ADMIN_KEY` and the pool's `usage_token_env` now names \
                 `ORG_B_ADMIN_KEY`, so it bounds nothing; `upstroke capacity --refresh` \
                 replaces it"
                    .to_owned()
            ]
        );

        let (bound, notes) = reading(&report, &keyed(None), NOON + HOUR);
        assert_eq!(
            bound, None,
            "a pool that names no variable names none of the report's"
        );
        assert_eq!(
            notes,
            vec![
                "the stored provider report was fetched with the credential in \
                 `ORG_A_ADMIN_KEY` and the pool now names no `usage_token_env`, so it bounds \
                 nothing; `upstroke capacity --refresh` will not run for this pool until it \
                 names one"
                    .to_owned()
            ]
        );

        let unkeyed = UsageReport {
            token_env: None,
            ..report
        };
        let (bound, notes) = reading(&unkeyed, &keyed(None), NOON + HOUR);
        assert_eq!(
            bound, None,
            "a report of no known variable is not the report of a pool that names none"
        );
        assert_eq!(
            notes,
            vec![
                "the stored provider report predates recording which variable its credential \
                 was read from and the pool names no `usage_token_env`, so whose usage it \
                 measured is not known and it bounds nothing; `upstroke capacity --refresh` \
                 will not run for this pool until it names one"
                    .to_owned()
            ]
        );

        let other_token = Pool {
            usage: Some(UsageSource {
                provider: UsageProvider::GithubAiCredits,
                token_env: Some("OTHER_TOKEN".to_owned()),
                account: Some("account-a".to_owned()),
            }),
            ..per_account(Some("account-a"))
        };
        let (bound, notes) = reading(
            &per_account_report(Some("account-a")),
            &other_token,
            NOON + HOUR,
        );
        assert_eq!(
            bound, None,
            "the variable is held for a provider that reports per account too"
        );
        assert!(
            notes.len() == 1
                && notes[0].contains("fetched with the credential in `TOKEN`")
                && notes[0].contains("now names `OTHER_TOKEN`"),
            "{notes:?}"
        );
    }

    #[test]
    fn a_report_whose_provider_variable_and_account_all_match_still_binds() {
        let written = per_account_report(Some("account-a"))
            .encode()
            .expect("the report the tracker writes encodes");
        let report = UsageReport::decode(written.as_bytes()).expect("and reads back");
        assert_eq!(
            (
                report.provider.as_str(),
                report.token_env.as_deref(),
                report.account.as_deref()
            ),
            ("github-ai-credits", Some("TOKEN"), Some("account-a"))
        );
        let (bound, notes) = reading(&report, &per_account(Some("account-a")), NOON + HOUR);
        let bound = bound.expect("every part of the source it records is the pool's own");
        assert!((bound.remaining - 0.1).abs() < 1e-9, "{bound:?}");
        assert_eq!(bound.provider, UsageProvider::GithubAiCredits);
        assert!(notes.is_empty(), "{notes:?}");
    }

    #[test]
    fn a_per_key_report_binds_while_its_account_free_fields_match() {
        for provider in [UsageProvider::AnthropicCost, UsageProvider::OpenaiCost] {
            let report = UsageReport {
                provider: provider.to_string(),
                ..stored(NOON, vec![window("month", 90.0, Some(NOON + 200 * HOUR))])
            };
            for account in [None, Some("octo-cat"), Some("another-org")] {
                let configured = Pool {
                    usage: Some(UsageSource {
                        provider,
                        token_env: Some("TOKEN".to_owned()),
                        account: account.map(str::to_owned),
                    }),
                    ..pool()
                };
                let (bound, notes) = reading(&report, &configured, NOON + HOUR);
                assert!(
                    bound.is_some_and(|bound| (bound.remaining - 0.1).abs() < 1e-9),
                    "`{provider}` reads no account, so with its provider and its variable the \
                     pool's own the report binds whatever `usage_account` is ({account:?}): \
                     {notes:?}"
                );
                assert!(notes.is_empty(), "{notes:?}");
            }
        }
    }

    #[test]
    fn a_report_stored_before_variables_were_recorded_decodes_and_bounds_nothing() {
        let per_key = UsageReport::decode(
            stored_before_sources_were_recorded("anthropic-api", "anthropic-cost").as_bytes(),
        )
        .expect("an old-format report still decodes");
        assert_eq!(
            per_key,
            UsageReport {
                token_env: None,
                ..stored(NOON, vec![window("month", 90.0, Some(NOON + 200 * HOUR))])
            },
            "and records no variable"
        );
        let (bound, notes) = reading(&per_key, &pool(), NOON + HOUR);
        assert_eq!(
            bound, None,
            "which key, and so whose organization, it measured is not known"
        );
        assert_eq!(
            notes,
            vec![
                "the stored provider report predates recording which variable its credential \
                 was read from, so whose usage it measured is not known and it bounds nothing; \
                 `upstroke capacity --refresh` replaces it"
                    .to_owned()
            ]
        );

        let account_only = UsageReport {
            token_env: None,
            ..per_account_report(Some("account-a"))
        };
        let (bound, notes) = reading(&account_only, &per_account(Some("account-a")), NOON + HOUR);
        assert_eq!(
            bound, None,
            "a report that records its account and not its variable is of no known credential"
        );
        assert_eq!(
            notes.len(),
            1,
            "its account matches, so only the variable is said: {notes:?}"
        );
        assert!(
            notes[0].contains("predates recording which variable its credential was read from"),
            "{notes:?}"
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

    const COPIED_RUN: &str = "01CAPACITYCOPIED0000000001";
    const FIVE_THIRTY: &str = "2026-11-11T05:30:00Z";

    /// `run_id`'s log in `repo`, as `gather` lists it from that repository:
    /// by its canonical path, and named by no owner record.
    fn listed(repo: &Path, run_id: &str) -> (PathBuf, RunLog) {
        let run = RunLog {
            run_id: run_id.to_owned(),
            repo_root: repo.to_path_buf(),
            named_by_owner: false,
        };
        let log = std::fs::canonicalize(run.log()).expect("the fixture run's log");
        (log, run)
    }

    /// Two logs of one run and no owner record naming either, which `gather`
    /// does not meet today: of the two logs it can find, one is always the
    /// one the record names. The shorter log is the first by path, so it is
    /// the count of events that decides, and it decides alike whichever log
    /// is given first.
    #[test]
    fn with_no_owner_record_the_log_holding_more_events_is_folded() {
        let tree = crate::workspace_manager::fixture::scratch("capacity-copies-unowned");
        let shorter = tree.path().join("a");
        let longer = tree.path().join("b");
        drop(schema_4_run_signalling(
            &shorter,
            COPIED_RUN,
            "limited",
            FIVE_THIRTY,
        ));
        drop(schema_4_run_limited_then_drawing(
            &longer,
            COPIED_RUN,
            ("limited", FIVE_THIRTY),
            ("api", 20.0),
        ));
        let (short_log, short_run) = listed(&shorter, COPIED_RUN);
        let (long_log, long_run) = listed(&longer, COPIED_RUN);
        assert!(short_log < long_log);
        let drawn = evidence_in(&long_run).expect("the longer log").evidence;
        assert_ne!(
            drawn,
            evidence_in(&short_run).expect("the shorter log").evidence
        );

        for copies in [
            vec![listed(&shorter, COPIED_RUN), listed(&longer, COPIED_RUN)],
            vec![listed(&longer, COPIED_RUN), listed(&shorter, COPIED_RUN)],
        ] {
            let mut warnings = Vec::new();
            assert_eq!(
                log_to_fold(COPIED_RUN, copies, &mut warnings),
                Some((long_log.clone(), drawn.clone()))
            );
            assert_eq!(
                warnings,
                vec![format!(
                    "run {COPIED_RUN} is folded once, from {}, the log holding more events; {} \
                     logs it too, with other events, and nothing from that log is shown",
                    long_log.display(),
                    short_log.display()
                )]
            );
        }
    }

    /// With no owner record and as many events in each, the first log by path
    /// is folded; and logs that hold the same events fold once with nothing
    /// said.
    #[test]
    fn with_no_owner_record_and_as_many_events_the_first_log_by_path_is_folded() {
        let tree = crate::workspace_manager::fixture::scratch("capacity-copies-even");
        let first = tree.path().join("a");
        let other = tree.path().join("b");
        let same = tree.path().join("c");
        drop(schema_4_run_signalling(
            &first,
            COPIED_RUN,
            "limited",
            FIVE_THIRTY,
        ));
        drop(schema_4_run_signalling(
            &other,
            COPIED_RUN,
            "elsewhere",
            FIVE_THIRTY,
        ));
        drop(schema_4_run_signalling(
            &same,
            COPIED_RUN,
            "limited",
            FIVE_THIRTY,
        ));
        let (first_log, first_run) = listed(&first, COPIED_RUN);
        let (other_log, _) = listed(&other, COPIED_RUN);
        let signalled = evidence_in(&first_run).expect("the first log").evidence;

        let mut warnings = Vec::new();
        let copies = vec![listed(&other, COPIED_RUN), listed(&first, COPIED_RUN)];
        assert_eq!(
            log_to_fold(COPIED_RUN, copies, &mut warnings),
            Some((first_log.clone(), signalled.clone()))
        );
        assert_eq!(
            warnings,
            vec![format!(
                "run {COPIED_RUN} is folded once, from {}, the first by path; {} logs it too, \
                 with other events, and nothing from that log is shown",
                first_log.display(),
                other_log.display()
            )]
        );

        let mut warnings = Vec::new();
        let copies = vec![listed(&same, COPIED_RUN), listed(&first, COPIED_RUN)];
        assert_eq!(
            log_to_fold(COPIED_RUN, copies, &mut warnings),
            Some((first_log, signalled))
        );
        assert_eq!(
            warnings,
            Vec::<String>::new(),
            "two logs that hold the same events are one run seen twice"
        );
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
