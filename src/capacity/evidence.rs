//! Capacity evidence: what one event log says about pools, in a form that can
//! be folded across runs (DESIGN.md §13).
//!
//! An event log speaks about capacity in three ways: a `pool_exhausted`
//! signal, a settled attempt whose record says which pools served and which
//! refused, and a review pass that drew on a pool. `observe` and
//! `exhausted_pools_of` used to read those straight off one log each. Tracking
//! capacity for the machine means reading them off every run of every
//! repository, in the order they happened, and a log's events cannot be
//! interleaved with another log's while they are still two different event
//! types. So each log is first reduced to [`Evidence`], which names pools and
//! nothing else, and [`fold`] is the one place the rules are applied, whether
//! the evidence came from one log or from fifty.
//!
//! Everything here is a pure function over values someone else read.

#![forbid(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros
)]

use std::collections::BTreeMap;

use crate::events::{AttemptRecord, Event, EventBody, PoolExhausted, ReviewRecord};
use crate::topology::events::{RejectionDisposition, TopologyEvent, TopologyEventBody};

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
#[derive(Debug, Clone, PartialEq)]
pub struct Evidence {
    /// Unix seconds of the event's own timestamp. `None` when the log's
    /// timestamp could not be read; such evidence keeps its place in its own
    /// log and is never dropped for being undated.
    pub at: Option<u64>,
    pub what: What,
}

fn at(ts: &str) -> Option<u64> {
    super::usage::unix_from_rfc3339(ts)
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
#[must_use]
pub fn of_topology(events: &[TopologyEvent]) -> Vec<Evidence> {
    let mut out = Vec::new();
    let mut held: Vec<Evidence> = Vec::new();
    for event in events {
        let when = at(&event.ts);
        let dated = |what| Evidence { at: when, what };
        match &event.body {
            TopologyEventBody::PoolExhausted { data } => held.push(dated(signal(data))),
            TopologyEventBody::AttemptFinished { data } => {
                out.push(dated(work(&data.record)));
                out.append(&mut held);
            }
            body => {
                out.append(&mut held);
                match body {
                    TopologyEventBody::CandidatePrepared { data } => {
                        out.push(dated(promoted(&data.attempt)));
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

/// Every run's evidence as one sequence, oldest first.
///
/// Within a run the log's order is kept exactly: undated evidence takes the
/// time of the dated evidence before it, and a timestamp that runs backwards
/// is raised to the latest already seen, so a clock stepped back mid-run
/// cannot reorder a signal past the settlement that retires it. Between runs
/// the order is by time, and at equal times by the order the runs were given.
#[must_use]
pub fn merge(runs: Vec<Vec<Evidence>>) -> Vec<Evidence> {
    let mut keyed: Vec<(u64, usize, usize, Evidence)> = Vec::new();
    for (run, evidence) in runs.into_iter().enumerate() {
        let mut latest = 0_u64;
        for (position, item) in evidence.into_iter().enumerate() {
            latest = latest.max(item.at.unwrap_or(latest));
            keyed.push((latest, run, position, item));
        }
    }
    keyed.sort_by_key(|(time, run, position, _)| (*time, *run, *position));
    keyed.into_iter().map(|(_, _, _, item)| item).collect()
}

/// Apply evidence, in the order given, to nothing.
///
/// A signal sets its pool's reset: the latest wins. Work retires the pools it
/// proves served, then marks the pool it proves refused, keeping a reset
/// already known for it. A draw is counted when its evidence is dated at or
/// after `spend_since`, and when it is undated: an estimate is a ceiling, and
/// leaving out a draw whose date is unknown would raise it.
#[must_use]
pub fn fold(evidence: &[Evidence], spend_since: u64) -> Observations {
    let mut obs = Observations::default();
    for item in evidence {
        match &item.what {
            What::Signal { pool, reset_at } => {
                obs.exhausted.insert(pool.clone(), reset_at.clone());
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
                    obs.exhausted.remove(pool);
                }
                if let Some(pool) = limited {
                    obs.exhausted.entry(pool.clone()).or_insert(None);
                }
            }
        }
    }
    obs
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

        let merged = merge(vec![refused.clone(), served_later]);
        assert!(
            exhausted(&merged).is_empty(),
            "a later attempt the pool served, in any repository, proves it is serving again"
        );

        let merged = merge(vec![served_earlier, refused]);
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
                what: What::Signal {
                    pool: "p".to_owned(),
                    reset_at: None,
                },
            },
            Evidence {
                at: Some(100),
                what: What::Work {
                    served: vec!["p".to_owned()],
                    limited: None,
                    draws: Vec::new(),
                },
            },
            Evidence {
                at: None,
                what: What::Signal {
                    pool: "q".to_owned(),
                    reset_at: None,
                },
            },
        ];
        let other = vec![Evidence {
            at: Some(150),
            what: What::Signal {
                pool: "r".to_owned(),
                reset_at: None,
            },
        }];
        let merged = merge(vec![stepped_back.clone(), other.clone()]);
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
    fn a_draw_before_the_period_is_not_counted_and_an_undated_one_is() {
        let evidence = [
            Evidence {
                at: Some(99),
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
}
