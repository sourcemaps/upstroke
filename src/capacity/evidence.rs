//! Capacity evidence: what one event log says about pools, in a form that can
//! be folded across runs (DESIGN.md §13).
//!
//! An event log speaks about capacity in four ways: a `pool_exhausted`
//! signal, a settled attempt whose record says which pools served and which
//! refused, an interrupted attempt that drew on the pool it started on, and a
//! review pass that drew on a pool. Tracking capacity for the machine means
//! reading those off every run of every repository, in the order they
//! happened, and a log's events cannot be interleaved with another log's
//! while they are still two different event types. So each log is first
//! reduced to [`Evidence`], which names pools and nothing else, and [`fold`]
//! is the one place the rules are applied, whether the evidence came from one
//! log or from fifty: `observe`, `observe_topology` and `exhausted_pools_of`
//! are one call each into it.
//!
//! Everything here is a pure function over values someone else read.

#![forbid(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros
)]

use std::collections::{BTreeMap, VecDeque};

use crate::events::{AttemptRecord, Event, EventBody, PoolExhausted, ReviewRecord};
use crate::topology::events::{
    AttemptNumber, RejectionDisposition, TopologyEvent, TopologyEventBody,
};

use super::{Observations, draws_of, limited_pool, review_draws, served_pools};

/// One attempt or review pass attributed to a pool, with the dollars it
/// reported, if it reported any.
#[derive(Debug, Clone, PartialEq)]
pub struct Draw {
    pub pool: String,
    pub usd: Option<f64>,
}

/// What one event established.
#[derive(Debug, Clone, PartialEq)]
pub enum What {
    /// A `pool_exhausted`: the pool refused, and this is the reset it named.
    Signal {
        pool: String,
        reset_at: Option<String>,
    },
    /// Settled work. `served` are the pools the record proves answered,
    /// `limited` the one it proves refused, `draws` what it drew.
    Work {
        served: Vec<String>,
        limited: Option<String>,
        draws: Vec<Draw>,
    },
}

/// One piece of evidence and when its event was written.
#[derive(Debug, Clone)]
pub struct Evidence {
    /// Unix seconds of the event's own timestamp. `None` when the log's
    /// timestamp could not be read; such evidence keeps its place in its own
    /// log and is never dropped for being undated.
    pub at: Option<u64>,
    /// For a schema-4 attempt's record, the Unix second its worker finished:
    /// the latest `attempt_started` of that attempt before the record, plus
    /// the record's duration in whole seconds, and never later than `at`. It
    /// dates the worker's pool and each reviewer's pool alike, since the
    /// worker's end is the earliest a review of it can have run. `None` where
    /// the log holds no dated start for the attempt, for evidence that is no
    /// attempt's record, and for every record of a log of schemas 1–3.
    pub served_at: Option<u64>,
    /// Whether the evidence is of a log of schemas 1–3, which dates no
    /// service: its successes retire refusals as every success did before
    /// service was dated (see [`fold`]).
    legacy: bool,
    /// The run the evidence came from: its place among the runs [`merge`]
    /// was given, in key order. Evidence of one log is one run.
    run: usize,
    pub what: What,
}

/// Two pieces of evidence are equal when they say the same thing of the same
/// time. The run each came from is where [`merge`] found it, not what it
/// says.
impl PartialEq for Evidence {
    fn eq(&self, other: &Self) -> bool {
        self.at == other.at
            && self.served_at == other.served_at
            && self.legacy == other.legacy
            && self.what == other.what
    }
}

fn at(ts: &str) -> Option<u64> {
    super::usage::unix_from_rfc3339(ts)
}

/// When the worker of `record` finished, from its start: see
/// [`Evidence::served_at`].
fn served_at(started: Option<u64>, record: &AttemptRecord, settled: Option<u64>) -> Option<u64> {
    let ended = started?.saturating_add(record.duration.as_secs());
    Some(settled.map_or(ended, |settled| ended.min(settled)))
}

fn signal(data: &PoolExhausted) -> What {
    What::Signal {
        pool: data.pool.clone(),
        reset_at: data.reset_at.clone(),
    }
}

fn work(record: &AttemptRecord) -> What {
    What::Work {
        served: served_pools(record).into_iter().cloned().collect(),
        limited: limited_pool(record).cloned(),
        draws: draws_of(record)
            .map(|(pool, usd)| Draw {
                pool: pool.clone(),
                usd,
            })
            .collect(),
    }
}

/// A promoted attempt's record: what it served and drew. It marks nothing
/// refused, because a candidate is prepared from an attempt that passed, and
/// `retire_served_pools` is all the live run applies to one.
fn promoted(record: &AttemptRecord) -> What {
    match work(record) {
        What::Work { served, draws, .. } => What::Work {
            served,
            limited: None,
            draws,
        },
        signal @ What::Signal { .. } => signal,
    }
}

/// One attempt that drew on `pool` and recorded no cost.
fn unpriced(pool: &str) -> What {
    What::Work {
        served: Vec::new(),
        limited: None,
        draws: vec![Draw {
            pool: pool.to_owned(),
            usd: None,
        }],
    }
}

fn reviews(reviews: &[ReviewRecord]) -> What {
    What::Work {
        served: Vec::new(),
        limited: None,
        draws: review_draws(reviews)
            .map(|(pool, usd)| Draw {
                pool: pool.clone(),
                usd,
            })
            .collect(),
    }
}

/// The evidence in a sequential (schema 1–3) log, in log order.
#[must_use]
pub fn of_legacy(events: &[Event]) -> Vec<Evidence> {
    events
        .iter()
        .filter_map(|event| {
            let what = match &event.body {
                EventBody::PoolExhausted { data, .. } => signal(data),
                EventBody::AttemptFinished { data, .. }
                | EventBody::AttemptInterrupted { data, .. } => work(data),
                _ => return None,
            };
            Some(Evidence {
                at: at(&event.ts),
                served_at: None,
                legacy: true,
                run: 0,
                what,
            })
        })
        .collect()
}

/// The evidence in a schema-4 log, in the order the live run applied it.
///
/// **That order is not the log's.** The run writes `pool_exhausted` before its
/// attempt's `attempt_finished`, so that a stop between the two leaves the
/// signal durable; in memory it retires the pools that attempt served first
/// and records the signal after. Applied in log order, a reviewer refused on
/// the pool its worker had just served would lose its reset to that worker's
/// success. So a signal is held until the event after it: if that event is the
/// attempt's settlement, the settlement goes first.
///
/// Review passes of a merge verification draw on pools without settling an
/// attempt. They are evidence of spend and of nothing else, on the same four
/// events `engine::topology::select::Spend::replay` charges.
///
/// An interrupted attempt settles with no record, so nothing says what it
/// cost. It still drew: it is one unpriced draw on the pool its
/// `attempt_started` named, and none where that start is missing or named no
/// pool.
///
/// A settled or promoted attempt's record is dated twice: `at` by the event
/// that carries it, which the run writes after the attempt's gates and its
/// judgement, and `served_at` by the attempt's own start and duration.
#[must_use]
pub fn of_topology(events: &[TopologyEvent]) -> Vec<Evidence> {
    let mut out = Vec::new();
    let mut held: Vec<Evidence> = Vec::new();
    let mut started_on = BTreeMap::new();
    for event in events {
        let when = at(&event.ts);
        let dated = |what| Evidence {
            at: when,
            served_at: None,
            legacy: false,
            run: 0,
            what,
        };
        match &event.body {
            TopologyEventBody::PoolExhausted { data } => held.push(dated(signal(data))),
            TopologyEventBody::AttemptFinished { data } => {
                let started = started_on
                    .get(&(data.key, data.generation, data.attempt))
                    .and_then(|&(_, started)| started);
                out.push(Evidence {
                    served_at: served_at(started, &data.record, when),
                    ..dated(work(&data.record))
                });
                out.append(&mut held);
            }
            body => {
                out.append(&mut held);
                match body {
                    TopologyEventBody::AttemptStarted { data } => {
                        started_on.insert(
                            (data.key, data.generation, data.attempt),
                            (data.pool.as_ref(), when),
                        );
                    }
                    TopologyEventBody::AttemptInterrupted { data } => {
                        let pool = started_on
                            .get(&(data.key, data.generation, data.attempt))
                            .and_then(|&(pool, _)| pool);
                        if let Some(pool) = pool {
                            out.push(dated(unpriced(pool)));
                        }
                    }
                    TopologyEventBody::CandidatePrepared { data } => {
                        let attempt = AttemptNumber(data.attempt.attempt);
                        let started = started_on
                            .get(&(data.key, data.generation, attempt))
                            .and_then(|&(_, started)| started);
                        out.push(Evidence {
                            served_at: served_at(started, &data.attempt, when),
                            ..dated(promoted(&data.attempt))
                        });
                    }
                    TopologyEventBody::MergeVerificationUnavailable { data } => {
                        out.push(dated(reviews(&data.reviews)));
                    }
                    TopologyEventBody::MergeVerificationCharged { data } => {
                        out.push(dated(reviews(&data.reviews)));
                    }
                    TopologyEventBody::MergePrepared { data } => {
                        if let Some(verification) = &data.verification {
                            out.push(dated(reviews(&verification.reviews)));
                        }
                    }
                    TopologyEventBody::MergeRejected { data } => {
                        if let RejectionDisposition::CodeRejected { verification } =
                            &data.disposition
                        {
                            out.push(dated(reviews(&verification.reviews)));
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    out.append(&mut held);
    out.retain(|evidence| match &evidence.what {
        What::Signal { .. } => true,
        What::Work {
            served,
            limited,
            draws,
        } => !(served.is_empty() && limited.is_none() && draws.is_empty()),
    });
    out
}

/// Every run's evidence as one sequence, oldest first, each item marked with
/// the run it came from, which is what [`fold`] retires a refusal by.
///
/// Each run is given with a key that names it: `gather` gives the run's id,
/// then its log's path.
///
/// Within a run the log's order is kept exactly: undated evidence takes the
/// time of the dated evidence before it, and a timestamp that runs backwards
/// is raised to the latest already seen, so a clock stepped back mid-run
/// cannot reorder a signal past the settlement that retires it. So the order
/// alone cannot say whether a success came after another run's refusal: a
/// settlement is placed by when it was written, and re-dating it to when its
/// worker finished would only see it raised back past the run's later
/// evidence. `fold` asks whose refusal it is and when the service was given
/// instead. Between runs the order is by time, and at an equal time by the
/// evidence and the keys, never by the order the runs are given in: evidence
/// that marks a pool refused waits while another run's evidence of that
/// second still to come proves the pool served, and otherwise the run whose
/// key sorts first goes first. A schema-4 success's service is never dated
/// after its own settlement, so it retires no refusal another run wrote in
/// that second or later, whichever goes first. A success of a log of schemas
/// 1–3 is not dated, and for it the wait is still what keeps another run's
/// refusal of its second standing. The order also decides which reset a pool
/// refused by two runs shows.
#[must_use]
pub fn merge<K: Ord>(mut runs: Vec<(K, Vec<Evidence>)>) -> Vec<Evidence> {
    runs.sort_by(|(one, _), (other, _)| one.cmp(other));
    let mut seconds: BTreeMap<u64, Vec<VecDeque<Evidence>>> = BTreeMap::new();
    for (run, (_, evidence)) in runs.into_iter().enumerate() {
        let mut latest = 0_u64;
        let mut lane_at = None;
        for mut item in evidence {
            item.run = run;
            latest = latest.max(item.at.unwrap_or(latest));
            let lanes = seconds.entry(latest).or_default();
            match lanes.last_mut() {
                Some(lane) if lane_at == Some(latest) => lane.push_back(item),
                _ => {
                    lanes.push(VecDeque::from([item]));
                    lane_at = Some(latest);
                }
            }
        }
    }
    let mut merged = Vec::new();
    for lanes in seconds.into_values() {
        interleave(lanes, &mut merged);
    }
    merged
}

/// One second's evidence, a lane per run in key order, onto the end of
/// `merged`, each lane's own order kept.
fn interleave(mut lanes: Vec<VecDeque<Evidence>>, merged: &mut Vec<Evidence>) {
    while let Some(item) = next_lane(&lanes).and_then(|lane| lanes.get_mut(lane)?.pop_front()) {
        merged.push(item);
    }
}

/// The lane whose head goes next: the first whose head need not wait. A head
/// waits while it marks a pool refused that evidence still in another lane
/// proves served. Where every head waits, the first lane goes. That takes
/// runs whose refusals and successes of one second cross, one refusing `p`
/// before it serves `q` and another refusing `q` before it serves `p`: no
/// order puts both refusals after both successes and keeps both runs' own
/// order, and it is the runs' own order that is kept. Where the successes are
/// schema-4 ones, both refusals stand all the same: [`fold`] lets such a
/// success retire another run's refusal only when its service is dated
/// strictly after that refusal, and a success written in the refusal's second
/// is not. Where a success is of a log of schemas 1–3, which is not dated, the
/// run whose key sorts first goes first and one refusal is cleared, as before.
fn next_lane(lanes: &[VecDeque<Evidence>]) -> Option<usize> {
    let heads = || {
        lanes
            .iter()
            .enumerate()
            .filter_map(|(lane, items)| Some((lane, items.front()?)))
    };
    let waits = |lane: usize, head: &Evidence| {
        refused(head).is_some_and(|pool| {
            let others = lanes.iter().enumerate().filter(|&(other, _)| other != lane);
            others
                .flat_map(|(_, items)| items)
                .any(|item| serves(item, pool))
        })
    };
    heads()
        .find(|&(lane, head)| !waits(lane, head))
        .or_else(|| heads().next())
        .map(|(lane, _)| lane)
}

/// The pool `evidence` marks refused, if it marks one.
fn refused(evidence: &Evidence) -> Option<&str> {
    match &evidence.what {
        What::Signal { pool, .. } => Some(pool.as_str()),
        What::Work { limited, .. } => limited.as_deref(),
    }
}

/// Whether `evidence` proves `pool` served.
fn serves(evidence: &Evidence, pool: &str) -> bool {
    match &evidence.what {
        What::Signal { .. } => false,
        What::Work { served, .. } => served.iter().any(|name| name == pool),
    }
}

/// Apply evidence, in the order given, to nothing.
///
/// Refusals are held per pool and per run: a pool is exhausted while any
/// run's refusal of it stands, and it shows the reset of the refusal applied
/// last. A signal adds or replaces its run's refusal of its pool, with its own
/// reset: the latest wins. Work retires refusals of the pools it proves
/// served, then marks the pool it proves refused, adding or replacing its
/// run's refusal of it with the reset the pool already shows, if it shows
/// one. Work retires its own run's refusal of a pool it served always, in
/// that run's order, which is all one log has. It retires another run's only
/// when its `served_at` is known and strictly later than that refusal's `at`:
/// a success settled after another run's refusal, from service given before
/// it, proves nothing about it. An undated refusal yields to its own run
/// alone, and so does every refusal to a schema-4 success with no
/// `served_at`. A log of schemas 1–3 dates no service, and its successes keep
/// the rule every success had before: they retire every run's refusal of the
/// pool that comes before them in the order given, which [`merge`] makes wait
/// for another run's success of its own second. Inside one run a late
/// settlement still retires that run's own newer refusal, and so does a
/// legacy log's late settlement another run's.
///
/// A draw is counted when its evidence is dated at or after `spend_since`, and
/// when it is undated: an estimate is a ceiling, and leaving out a draw whose
/// date is unknown would raise it.
#[must_use]
pub fn fold(evidence: &[Evidence], spend_since: u64) -> Observations {
    let mut obs = Observations::default();
    let mut refusals: BTreeMap<String, Vec<Refusal>> = BTreeMap::new();
    for (applied, item) in evidence.iter().enumerate() {
        match &item.what {
            What::Signal { pool, reset_at } => {
                refuse(&mut refusals, pool, item, reset_at.clone(), applied);
            }
            What::Work {
                served,
                limited,
                draws,
            } => {
                if item.at.is_none_or(|at| at >= spend_since) {
                    for draw in draws {
                        obs.self_spend
                            .entry(draw.pool.clone())
                            .or_default()
                            .add(draw.usd);
                    }
                }
                for pool in served {
                    if let Some(standing) = refusals.get_mut(pool) {
                        standing.retain(|refusal| !retires(item, refusal));
                    }
                }
                if let Some(pool) = limited {
                    let known = refusals
                        .get(pool)
                        .and_then(|standing| shown(standing))
                        .cloned()
                        .flatten();
                    refuse(&mut refusals, pool, item, known, applied);
                }
            }
        }
    }
    obs.exhausted = refusals
        .into_iter()
        .filter_map(|(pool, standing)| Some((pool, shown(&standing)?.clone())))
        .collect();
    obs
}

/// One run's refusal of a pool, while it stands.
struct Refusal {
    run: usize,
    at: Option<u64>,
    reset_at: Option<String>,
    applied: usize,
}

/// The reset a pool shows: that of its standing refusal applied last.
fn shown(standing: &[Refusal]) -> Option<&Option<String>> {
    standing
        .iter()
        .max_by_key(|refusal| refusal.applied)
        .map(|refusal| &refusal.reset_at)
}

fn refuse(
    refusals: &mut BTreeMap<String, Vec<Refusal>>,
    pool: &str,
    item: &Evidence,
    reset_at: Option<String>,
    applied: usize,
) {
    let standing = refusals.entry(pool.to_owned()).or_default();
    standing.retain(|refusal| refusal.run != item.run);
    standing.push(Refusal {
        run: item.run,
        at: item.at,
        reset_at,
        applied,
    });
}

/// Whether `service`, which proves the refusal's pool served, retires it.
fn retires(service: &Evidence, refusal: &Refusal) -> bool {
    service.legacy
        || refusal.run == service.run
        || matches!(
            (service.served_at, refusal.at),
            (Some(served), Some(refused)) if served > refused
        )
}

/// The exhausted pools alone, for a caller that wants no spend.
#[must_use]
pub fn exhausted(evidence: &[Evidence]) -> BTreeMap<String, Option<String>> {
    fold(evidence, 0).exhausted
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::events::{FailureRecord, ReviewPassOutcome};
    use crate::ladder::{FailureKind, FailureOrigin};

    fn record(pool: &str, cost: Option<f64>, failure: Option<FailureKind>) -> AttemptRecord {
        AttemptRecord {
            attempt: 1,
            tier: "small".to_owned(),
            model: "m".to_owned(),
            pool: Some(pool.to_owned()),
            resumed: false,
            duration: Duration::ZERO,
            cost_usd: cost,
            reviews: Vec::new(),
            session_id: None,
            usage: None,
            failure: failure.map(|kind| FailureRecord {
                kind,
                origin: FailureOrigin::Worker,
                reason: "r".to_owned(),
                detail: None,
            }),
        }
    }

    fn review(pool: &str, cost: Option<f64>) -> ReviewRecord {
        ReviewRecord {
            pass: "review".to_owned(),
            agent: "codex".to_owned(),
            model: "m".to_owned(),
            adapter: None,
            preflight_cli_version: None,
            effort: None,
            pool: Some(pool.to_owned()),
            cost_usd: cost,
            outcome: ReviewPassOutcome::Passed,
        }
    }

    fn legacy(ts: &str, body: EventBody) -> Event {
        Event {
            ts: ts.to_owned(),
            body,
        }
    }

    fn finished(ts: &str, record: AttemptRecord) -> Event {
        legacy(
            ts,
            EventBody::AttemptFinished {
                task: "t1".to_owned(),
                attempt: record.attempt,
                rung: 0,
                profile: "p".to_owned(),
                parking: None,
                transition: None,
                prepared_commit: None,
                data: Box::new(record),
            },
        )
    }

    fn signalled(ts: &str, pool: &str, reset_at: Option<&str>) -> Event {
        legacy(
            ts,
            EventBody::PoolExhausted {
                task: "t1".to_owned(),
                data: PoolExhausted {
                    pool: pool.to_owned(),
                    agent: "claude-code".to_owned(),
                    reset_at: reset_at.map(str::to_owned),
                    detail: "limit".to_owned(),
                },
            },
        )
    }

    const NOON: &str = "2026-11-23T12:00:00Z";
    const ONE: &str = "2026-11-23T13:00:00Z";
    const TWO: &str = "2026-11-23T14:00:00Z";
    const RESET: &str = "2026-11-23T17:00:00Z";

    #[test]
    fn a_legacy_log_reduces_to_its_signals_and_its_settled_work_and_nothing_else() {
        let mut limited = record("claude-max", None, Some(FailureKind::RateLimited));
        limited.reviews = vec![review("codex-pro", Some(0.25))];
        let events = [
            legacy(
                NOON,
                EventBody::RunFinished {
                    data: crate::events::RunFinished {
                        outcome: crate::events::RunOutcome::Complete,
                        halted_at: None,
                        committed: 0,
                        parked: 0,
                    },
                },
            ),
            finished(NOON, limited),
            signalled(ONE, "claude-max", Some(RESET)),
        ];
        assert_eq!(
            of_legacy(&events),
            vec![
                Evidence {
                    at: Some(1_795_435_200),
                    served_at: None,
                    legacy: true,
                    run: 0,
                    what: What::Work {
                        served: vec!["codex-pro".to_owned()],
                        limited: Some("claude-max".to_owned()),
                        draws: vec![
                            Draw {
                                pool: "claude-max".to_owned(),
                                usd: None
                            },
                            Draw {
                                pool: "codex-pro".to_owned(),
                                usd: Some(0.25)
                            },
                        ],
                    },
                },
                Evidence {
                    at: Some(1_795_438_800),
                    served_at: None,
                    legacy: true,
                    run: 0,
                    what: What::Signal {
                        pool: "claude-max".to_owned(),
                        reset_at: Some(RESET.to_owned()),
                    },
                },
            ]
        );
    }

    #[test]
    fn one_runs_signal_is_retired_by_another_runs_later_success_on_the_same_pool() {
        let refused = of_legacy(&[
            finished(
                NOON,
                record("claude-max", None, Some(FailureKind::RateLimited)),
            ),
            signalled(NOON, "claude-max", Some(RESET)),
        ]);
        let served_later = of_legacy(&[finished(TWO, record("claude-max", Some(1.0), None))]);
        let served_earlier = of_legacy(&[finished(
            "2026-11-23T11:00:00Z",
            record("claude-max", Some(1.0), None),
        )]);

        let merged = merge(vec![
            ("refusing", refused.clone()),
            ("serving", served_later),
        ]);
        assert!(
            exhausted(&merged).is_empty(),
            "a later attempt the pool served, in any repository, proves it is serving again"
        );

        let merged = merge(vec![("serving", served_earlier), ("refusing", refused)]);
        assert_eq!(
            exhausted(&merged).get("claude-max"),
            Some(&Some(RESET.to_owned())),
            "an earlier success proves nothing about a refusal after it, whichever run is \
             listed first"
        );
    }

    #[test]
    fn merging_keeps_each_runs_own_order_whatever_its_timestamps_say() {
        let stepped_back = vec![
            Evidence {
                at: Some(200),
                served_at: None,
                legacy: false,
                run: 0,
                what: What::Signal {
                    pool: "p".to_owned(),
                    reset_at: None,
                },
            },
            Evidence {
                at: Some(100),
                served_at: None,
                legacy: false,
                run: 0,
                what: What::Work {
                    served: vec!["p".to_owned()],
                    limited: None,
                    draws: Vec::new(),
                },
            },
            Evidence {
                at: None,
                served_at: None,
                legacy: false,
                run: 0,
                what: What::Signal {
                    pool: "q".to_owned(),
                    reset_at: None,
                },
            },
        ];
        let other = vec![Evidence {
            at: Some(150),
            served_at: None,
            legacy: false,
            run: 0,
            what: What::Signal {
                pool: "r".to_owned(),
                reset_at: None,
            },
        }];
        let merged = merge(vec![
            ("stepped-back", stepped_back.clone()),
            ("other", other.clone()),
        ]);
        let own: Vec<&Evidence> = merged
            .iter()
            .filter(|item| stepped_back.contains(item))
            .collect();
        assert_eq!(own, stepped_back.iter().collect::<Vec<_>>());
        assert_eq!(merged.first(), other.first(), "150 sorts before 200");
        assert_eq!(
            exhausted(&merged).keys().collect::<Vec<_>>(),
            vec!["q", "r"],
            "the settlement written after the signal still retires it"
        );
    }

    #[test]
    fn a_success_in_the_second_of_another_runs_refusal_does_not_clear_it() {
        let refused = of_legacy(&[
            finished(
                NOON,
                record("claude-max", None, Some(FailureKind::RateLimited)),
            ),
            signalled(NOON, "claude-max", Some(RESET)),
        ]);
        let served = of_legacy(&[finished(NOON, record("claude-max", Some(1.0), None))]);

        for (refusing, serving) in [("01A", "01B"), ("01B", "01A")] {
            for runs in [
                vec![(refusing, refused.clone()), (serving, served.clone())],
                vec![(serving, served.clone()), (refusing, refused.clone())],
            ] {
                let merged = merge(runs);
                assert_eq!(merged.len(), refused.len() + served.len());
                assert_eq!(
                    exhausted(&merged),
                    BTreeMap::from([("claude-max".to_owned(), Some(RESET.to_owned()))]),
                    "refused in run {refusing} and served in run {serving} in the same second: \
                     neither which run is given first nor whose id sorts first decides it"
                );
            }
        }
    }

    #[test]
    fn a_runs_own_success_in_the_second_of_its_refusal_still_clears_it() {
        let recovered = of_legacy(&[
            signalled(NOON, "claude-max", Some(RESET)),
            finished(NOON, record("claude-max", Some(1.0), None)),
        ]);
        assert!(
            exhausted(&merge(vec![("01A", recovered.clone())])).is_empty(),
            "within one run the log's order stands, and the success was written after the refusal"
        );

        let served = of_legacy(&[finished(NOON, record("claude-max", Some(1.0), None))]);
        for (own, other) in [("01A", "01B"), ("01B", "01A")] {
            for runs in [
                vec![(own, recovered.clone()), (other, served.clone())],
                vec![(other, served.clone()), (own, recovered.clone())],
            ] {
                assert!(
                    exhausted(&merge(runs)).is_empty(),
                    "run {own}'s success after its own refusal still clears it beside run \
                     {other}'s success of the same second"
                );
            }
        }
    }

    #[test]
    fn where_two_runs_refusals_and_successes_cross_the_order_given_still_decides_nothing() {
        let refused_then_served = |refused: &str, served: &str| {
            of_legacy(&[
                signalled(NOON, refused, Some(RESET)),
                finished(NOON, record(served, Some(1.0), None)),
            ])
        };
        let one = refused_then_served("claude-max", "codex-pro");
        let other = refused_then_served("codex-pro", "claude-max");
        assert_eq!(
            exhausted(&merge(vec![("01A", one.clone()), ("01B", other.clone())])),
            exhausted(&merge(vec![("01B", other), ("01A", one)])),
        );
    }

    #[test]
    fn a_draw_before_the_period_is_not_counted_and_an_undated_one_is() {
        let evidence = [
            Evidence {
                at: Some(99),
                served_at: None,
                legacy: false,
                run: 0,
                what: What::Work {
                    served: Vec::new(),
                    limited: None,
                    draws: vec![Draw {
                        pool: "p".to_owned(),
                        usd: Some(5.0),
                    }],
                },
            },
            Evidence {
                at: Some(100),
                served_at: None,
                legacy: false,
                run: 0,
                what: What::Work {
                    served: Vec::new(),
                    limited: None,
                    draws: vec![Draw {
                        pool: "p".to_owned(),
                        usd: Some(2.0),
                    }],
                },
            },
            Evidence {
                at: None,
                served_at: None,
                legacy: false,
                run: 0,
                what: What::Work {
                    served: Vec::new(),
                    limited: None,
                    draws: vec![Draw {
                        pool: "p".to_owned(),
                        usd: None,
                    }],
                },
            },
        ];
        let spend = fold(&evidence, 100).self_spend;
        let drawn = spend.get("p").expect("the pool drew");
        assert_eq!(drawn.usd, Some(2.0));
        assert_eq!(drawn.attempts, 2);
        assert_eq!(drawn.unpriced, 1);
        assert_eq!(
            fold(&evidence, 0).self_spend.get("p").map(|s| s.attempts),
            Some(3)
        );
    }

    #[test]
    fn a_period_filter_never_hides_a_signal_or_the_work_that_retires_one() {
        let evidence = of_legacy(&[
            signalled(NOON, "claude-max", Some(RESET)),
            finished(
                ONE,
                record("codex-pro", None, Some(FailureKind::RateLimited)),
            ),
        ]);
        let later = fold(&evidence, u64::MAX);
        assert!(later.self_spend.is_empty());
        assert_eq!(
            later.exhausted,
            BTreeMap::from([
                ("claude-max".to_owned(), Some(RESET.to_owned())),
                ("codex-pro".to_owned(), None),
            ])
        );
    }

    const SIXTEEN: &str = "2026-11-23T16:00:00Z";

    fn topology(ts: &str, body: TopologyEventBody) -> TopologyEvent {
        TopologyEvent {
            ts: ts.to_owned(),
            body,
        }
    }

    fn started4(ts: &str, key: u32, pool: &str) -> TopologyEvent {
        use crate::topology::events::{AttemptStarted4, GenerationId, RungBinding};
        topology(
            ts,
            TopologyEventBody::AttemptStarted {
                data: AttemptStarted4 {
                    key: crate::topology::registry::TaskKey(key),
                    generation: GenerationId(0),
                    attempt: AttemptNumber(1),
                    rung: 0,
                    binding: RungBinding {
                        tier: crate::ir::Tier::Mid,
                        agent: "claude-code".to_owned(),
                        model: "m".to_owned(),
                        pinned: false,
                        effort: crate::ir::Effort::High,
                    },
                    pool: Some(pool.to_owned()),
                    resume_session: None,
                    materialization_observed: None,
                },
            },
        )
    }

    fn settled4(ts: &str, key: u32, record: AttemptRecord) -> TopologyEvent {
        use crate::topology::events::{
            AttemptFinished4, AttemptSettlement, GenerationId, LeaseDisposition,
            SettlementTransition,
        };
        topology(
            ts,
            TopologyEventBody::AttemptFinished {
                data: Box::new(AttemptFinished4 {
                    key: crate::topology::registry::TaskKey(key),
                    generation: GenerationId(0),
                    attempt: AttemptNumber(record.attempt),
                    record: Box::new(record),
                    settlement: AttemptSettlement::Closed {
                        transition: SettlementTransition::Succeeded,
                        lease: LeaseDisposition::PredictedReleased,
                    },
                }),
            },
        )
    }

    fn promoted4(ts: &str, key: u32, record: AttemptRecord) -> TopologyEvent {
        use crate::topology::events::{
            CandidateLeaseEffect, CandidatePrepared, CommitSha, GenerationId, GitRef,
        };
        use crate::topology::paths::PathSet;
        let sha = |digit: &str| CommitSha(digit.repeat(40));
        topology(
            ts,
            TopologyEventBody::CandidatePrepared {
                data: Box::new(CandidatePrepared {
                    key: crate::topology::registry::TaskKey(key),
                    generation: GenerationId(0),
                    attempt: Box::new(record),
                    base_sha: sha("a"),
                    parent_sha: sha("a"),
                    tree_sha: sha("b"),
                    commit_sha: sha("c"),
                    message: "upstroke: t1 attempt 1".to_owned(),
                    prepared_ref: GitRef("refs/upstroke/prepared/t1".to_owned()),
                    candidate_ref: GitRef("refs/upstroke/candidates/t1".to_owned()),
                    actual_paths: PathSet::RepoWide,
                    lease_effect: CandidateLeaseEffect::ReplacesPredicted {
                        paths: PathSet::RepoWide,
                    },
                }),
            },
        )
    }

    fn signalled4(ts: &str, pool: &str, reset_at: &str) -> TopologyEvent {
        topology(
            ts,
            TopologyEventBody::PoolExhausted {
                data: PoolExhausted {
                    pool: pool.to_owned(),
                    agent: "claude-code".to_owned(),
                    reset_at: Some(reset_at.to_owned()),
                    detail: "limit".to_owned(),
                },
            },
        )
    }

    fn worked(pool: &str, seconds: u64) -> AttemptRecord {
        AttemptRecord {
            duration: Duration::from_secs(seconds),
            ..record(pool, Some(1.0), None)
        }
    }

    fn refused4(ts: &str, key: u32, pool: &str, reset_at: &str) -> Vec<TopologyEvent> {
        vec![
            started4(ts, key, pool),
            signalled4(ts, pool, reset_at),
            settled4(ts, key, record(pool, None, Some(FailureKind::RateLimited))),
        ]
    }

    fn refused_legacy(ts: &str, pool: &str, reset_at: &str) -> Vec<Evidence> {
        of_legacy(&[
            finished(ts, record(pool, None, Some(FailureKind::RateLimited))),
            signalled(ts, pool, Some(reset_at)),
        ])
    }

    fn held(pool: &str, reset_at: &str) -> BTreeMap<String, Option<String>> {
        BTreeMap::from([(pool.to_owned(), Some(reset_at.to_owned()))])
    }

    fn both_ways(one: &[Evidence], other: &[Evidence]) -> [Vec<Evidence>; 2] {
        [
            merge(vec![("01A", one.to_vec()), ("01B", other.to_vec())]),
            merge(vec![("01B", other.to_vec()), ("01A", one.to_vec())]),
        ]
    }

    #[test]
    fn a_record_is_dated_by_its_start_and_duration_and_never_after_its_settlement() {
        let served_at = |events: &[TopologyEvent]| -> Vec<Option<u64>> {
            of_topology(events)
                .iter()
                .map(|item| item.served_at)
                .collect()
        };
        let eleven = 1_795_431_600;
        assert_eq!(
            served_at(&[
                started4("2026-11-23T11:00:00Z", 1, "shared"),
                promoted4("2026-11-23T11:20:00Z", 1, worked("shared", 60)),
            ]),
            vec![Some(eleven + 60)]
        );
        assert_eq!(
            served_at(&[
                started4("2026-11-23T10:00:00Z", 1, "shared"),
                started4("2026-11-23T11:00:00Z", 1, "shared"),
                settled4("2026-11-23T11:20:00Z", 1, worked("shared", 90)),
            ]),
            vec![Some(eleven + 90)],
            "the latest start of the attempt dates it"
        );
        assert_eq!(
            served_at(&[
                started4("2026-11-23T11:00:00Z", 1, "shared"),
                settled4("2026-11-23T11:20:00Z", 1, worked("shared", 7200)),
            ]),
            vec![Some(eleven + 1200)],
            "never later than the settlement"
        );
        assert_eq!(
            served_at(&[
                started4("2026-11-23T11:00:00Z", 2, "shared"),
                settled4("2026-11-23T11:20:00Z", 1, worked("shared", 60)),
            ]),
            vec![None],
            "another attempt's start does not date this one"
        );
        assert!(
            of_legacy(&[finished(NOON, worked("shared", 60))])
                .iter()
                .all(|item| item.served_at.is_none())
        );
    }

    #[test]
    fn a_success_settled_after_another_runs_refusal_from_service_before_it_leaves_it_standing() {
        let served = of_topology(&[
            started4("2026-11-23T11:00:00Z", 1, "shared"),
            promoted4("2026-11-23T11:20:00Z", 1, worked("shared", 60)),
        ]);
        let refused = of_topology(&refused4("2026-11-23T11:05:00Z", 2, "shared", SIXTEEN));
        assert_eq!(exhausted(&refused), held("shared", SIXTEEN));

        let mut pool = crate::capacity::Pool::discovered(
            "shared",
            crate::capacity::PoolKind::SubscriptionWindow,
            "claude-code",
            vec![
                crate::capacity::Source::Signals,
                crate::capacity::Source::SelfMetered,
            ],
        );
        pool.safety_margin = 0.0;
        pool.reserve = 0.0;
        pool.monthly_allowance = crate::capacity::Allowance::Units(100.0);
        for merged in both_ways(&served, &refused) {
            let observations = fold(&merged, 0);
            assert_eq!(
                observations.exhausted,
                held("shared", SIXTEEN),
                "the worker served `shared` from 11:00:00 to 11:01:00 and settled at 11:20; the \
                 other run's refusal at 11:05 came after that service and stands"
            );
            let estimates = crate::capacity::estimate(std::slice::from_ref(&pool), &observations);
            assert_eq!(
                estimates[0].remaining,
                crate::capacity::Remaining::Exhausted
            );
        }

        let refused = refused_legacy("2026-11-23T11:05:00Z", "shared", SIXTEEN);
        for merged in both_ways(&served, &refused) {
            assert_eq!(
                exhausted(&merged),
                held("shared", SIXTEEN),
                "a refusal in a log of schemas 1-3 stands by the same rule"
            );
        }
    }

    #[test]
    fn the_serving_runs_own_later_evidence_does_not_carry_its_success_past_the_refusal() {
        let served = of_topology(&[
            started4("2026-11-23T11:00:00Z", 1, "shared"),
            started4("2026-11-23T11:08:00Z", 3, "other"),
            settled4("2026-11-23T11:10:00Z", 3, worked("other", 60)),
            promoted4("2026-11-23T11:20:00Z", 1, worked("shared", 60)),
        ]);
        let refused = of_topology(&refused4("2026-11-23T11:05:00Z", 2, "shared", SIXTEEN));
        for merged in both_ways(&served, &refused) {
            assert_eq!(
                exhausted(&merged),
                held("shared", SIXTEEN),
                "the serving run's attempt on `other` at 11:10 is between the refusal and the \
                 settlement, and changes nothing"
            );
        }
    }

    #[test]
    fn the_later_refusers_stale_success_leaves_the_other_runs_refusal_standing() {
        let earlier = of_topology(&refused4("2026-11-23T11:05:00Z", 1, "shared", SIXTEEN));
        let mut later_events = vec![started4("2026-11-23T11:00:00Z", 2, "shared")];
        later_events.extend(refused4("2026-11-23T11:10:00Z", 3, "shared", RESET));
        later_events.push(settled4("2026-11-23T11:15:00Z", 2, worked("shared", 120)));
        let later = of_topology(&later_events);
        for merged in both_ways(&earlier, &later) {
            assert_eq!(
                exhausted(&merged),
                held("shared", SIXTEEN),
                "served from 11:00 to 11:02 and settled at 11:15: the success retires its own \
                 run's refusal, as one log does, and the other run's refusal at 11:05 stands"
            );
        }

        let earlier = refused_legacy("2026-11-23T11:05:00Z", "shared", SIXTEEN);
        for merged in both_ways(&earlier, &later) {
            assert_eq!(exhausted(&merged), held("shared", SIXTEEN));
        }
    }

    #[test]
    fn a_worker_that_served_after_another_runs_refusal_retires_it() {
        let served = of_topology(&[
            started4("2026-11-23T11:04:00Z", 1, "shared"),
            promoted4("2026-11-23T11:20:00Z", 1, worked("shared", 120)),
        ]);
        let refused = of_topology(&refused4("2026-11-23T11:05:00Z", 2, "shared", SIXTEEN));
        for merged in both_ways(&served, &refused) {
            assert!(
                exhausted(&merged).is_empty(),
                "the worker served `shared` until 11:06, after the refusal at 11:05"
            );
        }
        let legacy = refused_legacy("2026-11-23T11:05:00Z", "shared", SIXTEEN);
        for merged in both_ways(&served, &legacy) {
            assert!(exhausted(&merged).is_empty());
        }

        let undated = of_topology(&refused4("not a time", 2, "shared", SIXTEEN));
        for merged in both_ways(&served, &undated) {
            assert_eq!(
                exhausted(&merged),
                held("shared", SIXTEEN),
                "an undated refusal yields to its own run alone"
            );
        }
    }

    #[test]
    fn a_reviewers_pool_is_dated_by_the_end_of_the_worker_it_reviewed() {
        let reviewed = |seconds| {
            let mut record = worked("worker", seconds);
            record.reviews = vec![review("shared", Some(0.25))];
            record
        };
        let refused = of_topology(&refused4("2026-11-23T11:05:00Z", 2, "shared", SIXTEEN));

        let before = of_topology(&[
            started4("2026-11-23T11:00:00Z", 1, "worker"),
            settled4("2026-11-23T11:20:00Z", 1, reviewed(60)),
        ]);
        for merged in both_ways(&before, &refused) {
            assert_eq!(
                exhausted(&merged),
                held("shared", SIXTEEN),
                "the worker ended at 11:01, the earliest its review can have run, before the \
                 refusal"
            );
        }

        let after = of_topology(&[
            started4("2026-11-23T11:04:00Z", 1, "worker"),
            settled4("2026-11-23T11:20:00Z", 1, reviewed(120)),
        ]);
        for merged in both_ways(&after, &refused) {
            assert!(
                exhausted(&merged).is_empty(),
                "the worker ended at 11:06, so its review ran after the refusal"
            );
        }
    }

    #[test]
    fn a_success_with_no_attempt_started_retires_only_its_own_runs_refusal() {
        let mut serving_events = refused4("2026-11-23T11:02:00Z", 3, "shared", RESET);
        serving_events.push(promoted4("2026-11-23T11:20:00Z", 1, worked("shared", 60)));
        let serving = of_topology(&serving_events);
        let refused = of_topology(&refused4("2026-11-23T11:05:00Z", 2, "shared", SIXTEEN));
        for merged in both_ways(&serving, &refused) {
            assert_eq!(
                exhausted(&merged),
                held("shared", SIXTEEN),
                "with no dated start the success retires its own run's refusal at 11:02 and not \
                 the other run's at 11:05"
            );
        }

        let refused = refused_legacy("2026-11-23T11:05:00Z", "shared", SIXTEEN);
        for merged in both_ways(&serving, &refused) {
            assert_eq!(exhausted(&merged), held("shared", SIXTEEN));
        }
    }
}
