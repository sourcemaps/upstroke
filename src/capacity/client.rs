//! The capacity client: the questions the engine asks about pools while a
//! run is live, and the built-in answer to each (DESIGN.md §13).
//!
//! The engine asks which pool an attempt binds, whether a refused attempt
//! fails over to another pool, which recorded reset a deferred run waits for,
//! and what a settled attempt says about the pools it drew on. It reads the
//! fold and the frozen plans and hands the client plain values: candidate
//! pools in file order, the run's events, the records of settled attempts.
//! [`BuiltinClient`] answers from the pools the run's signals hold exhausted,
//! as the engine did before the seam.

#![forbid(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros
)]

use std::collections::{BTreeMap, BTreeSet};

use crate::events::{AttemptRecord, PoolExhausted};
use crate::interaction;
use crate::ladder::AttemptFailure;
use crate::topology::events::{TopologyEvent, TopologyEventBody};
use crate::util::unix_seconds_of;

pub use super::{Pool, drain_of, pool_for};

/// The questions the engine asks about capacity while a run is live.
pub trait CapacityClient {
    /// The pool an attempt binds, among its agent's pools in file order
    /// (`candidates`), judged at `now_unix` against what the run's `events`
    /// say of spent resets. `None` when the agent names no pool.
    fn grant(
        &self,
        candidates: &[String],
        events: &[TopologyEvent],
        now_unix: u64,
    ) -> Option<String>;

    /// Whether a refused attempt is retried at once on another of its agent's
    /// pools (`candidates`, in file order) instead of deferred: a worker's
    /// rate limit, on a recorded pool, with some other candidate no signal
    /// holds exhausted.
    fn fails_over(
        &self,
        failure: &AttemptFailure,
        record: &AttemptRecord,
        candidates: &[String],
    ) -> bool;

    /// The recorded reset a deferred run waits for, chosen among the resets
    /// the `limits` wait on, at `now_unix`, with spent resets read from the
    /// run's `events`. `None` when the run backs off as before.
    fn awaited_reset(
        &self,
        limits: &[DeferredLimit<'_>],
        events: &[TopologyEvent],
        now_unix: u64,
    ) -> Option<AwaitedReset>;

    /// A settled or promoted attempt's record: the pools it proves served are
    /// no longer held exhausted.
    fn served(&mut self, record: &AttemptRecord);

    /// The `pool_exhausted` signal a rate-limited attempt's record calls for,
    /// for the attempt's `agent`, at `now_unix`. `None` when there is none to
    /// record, or the pool is held with that reset already.
    fn exhaustion(
        &self,
        failure: &AttemptFailure,
        record: &AttemptRecord,
        agent: &str,
        now_unix: u64,
    ) -> Option<PoolExhausted>;

    /// A `pool_exhausted` signal the run has just recorded: its pool is held,
    /// with the reset it named.
    fn held(&mut self, exhausted: &PoolExhausted);
}

/// A task the fold holds `Deferred` whose last settled attempt failed
/// `RateLimited`, as the engine reads it for [`CapacityClient::awaited_reset`].
#[derive(Debug, Clone)]
pub struct DeferredLimit<'a> {
    /// The last settled attempt's record, whose failure is the rate limit.
    pub record: &'a AttemptRecord,
    /// The pools of the agent bound on the task's rung, in file order, when
    /// the failure's origin is the worker. Empty for a reviewer's limit.
    pub worker_pools: Vec<String>,
}

/// What [`CapacityClient::awaited_reset`] chose: the pool and reset a
/// deferred run waits for, and the unix second the wait runs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AwaitedReset {
    /// The pool whose reset the run waits for.
    pub pool: String,
    /// The reset as its signal recorded it.
    pub reset_at: String,
    /// The unix second the wait runs to: the reset plus the margin.
    pub until_unix: u64,
}

/// The built-in client: each pool the run's signals hold exhausted, with the
/// reset its latest signal named.
#[derive(Debug, Default)]
pub struct BuiltinClient {
    exhausted_pools: BTreeMap<String, Option<String>>,
}

impl BuiltinClient {
    /// The client a resumed run starts from: the pools its `events` hold
    /// exhausted, as `capacity::observe` reads them.
    #[must_use]
    pub fn replay(events: &[TopologyEvent]) -> Self {
        Self {
            exhausted_pools: super::exhausted_pools_of(events),
        }
    }

    fn resets_awaited_by(
        &self,
        limit: &DeferredLimit<'_>,
        spent: &BTreeSet<String>,
        now_unix: u64,
    ) -> Option<Vec<(u64, &String, &String)>> {
        let record = limit.record;
        let failure = record.failure.as_ref()?;
        let candidates = match failure.origin {
            crate::ladder::FailureOrigin::Worker => limit.worker_pools.clone(),
            crate::ladder::FailureOrigin::Reviewer => {
                vec![super::limited_pool(record)?.clone()]
            }
        };
        let fails_over =
            failure.origin == crate::ladder::FailureOrigin::Worker && candidates.len() > 1;
        let held = candidates
            .iter()
            .map(|pool| self.exhausted_pools.get_key_value(pool))
            .collect::<Option<Vec<_>>>()?;
        Some(
            held.into_iter()
                .filter(|(pool, _)| !spent.contains(*pool))
                .filter_map(|(pool, reset_at)| {
                    let reset_at = reset_at.as_ref()?;
                    let reset = unix_seconds_of(reset_at)?;
                    (fails_over || reset > now_unix).then_some((reset, pool, reset_at))
                })
                .collect(),
        )
    }
}

impl CapacityClient for BuiltinClient {
    fn grant(
        &self,
        candidates: &[String],
        events: &[TopologyEvent],
        now_unix: u64,
    ) -> Option<String> {
        serving_pool(
            candidates,
            &held_unspent(&self.exhausted_pools, events),
            now_unix,
        )
        .cloned()
    }

    fn fails_over(
        &self,
        failure: &AttemptFailure,
        record: &AttemptRecord,
        candidates: &[String],
    ) -> bool {
        if failure.kind != crate::ladder::FailureKind::RateLimited
            || failure.origin != crate::ladder::FailureOrigin::Worker
        {
            return false;
        }
        let Some(refused) = &record.pool else {
            return false;
        };
        candidates
            .iter()
            .any(|pool| pool != refused && !self.exhausted_pools.contains_key(pool))
    }

    fn awaited_reset(
        &self,
        limits: &[DeferredLimit<'_>],
        events: &[TopologyEvent],
        now_unix: u64,
    ) -> Option<AwaitedReset> {
        let spent = spent_resets(events);
        let known: Vec<(u64, &String, &String)> = limits
            .iter()
            .filter_map(|limit| self.resets_awaited_by(limit, &spent, now_unix))
            .flatten()
            .collect();
        let margin = interaction::RESET_WAIT_MARGIN.as_secs();
        let &(earliest, ..) = known.iter().min_by_key(|(reset, ..)| *reset)?;
        let until_unix = if earliest <= now_unix {
            let available = earliest.checked_add(margin)?;
            if available <= now_unix {
                return None;
            }
            available
        } else {
            interaction::reset_wait_until(now_unix, known.iter().map(|(reset, ..)| *reset))?
        };
        let chosen = until_unix.checked_sub(margin)?;
        let (_, pool, reset_at) = known.iter().find(|(reset, ..)| *reset == chosen)?;
        Some(AwaitedReset {
            pool: (*pool).clone(),
            reset_at: (*reset_at).clone(),
            until_unix,
        })
    }

    fn served(&mut self, record: &AttemptRecord) {
        super::retire_served_pools(&mut self.exhausted_pools, record);
    }

    fn exhaustion(
        &self,
        failure: &AttemptFailure,
        record: &AttemptRecord,
        agent: &str,
        now_unix: u64,
    ) -> Option<PoolExhausted> {
        super::pool_exhausted_to_record(failure, record, agent, &self.exhausted_pools, now_unix)
    }

    fn held(&mut self, exhausted: &PoolExhausted) {
        let (pool, reset_at) = (exhausted.pool.clone(), exhausted.reset_at.clone());
        self.exhausted_pools.insert(pool, reset_at);
    }
}

fn held_unspent(
    exhausted_pools: &BTreeMap<String, Option<String>>,
    events: &[TopologyEvent],
) -> BTreeMap<String, Option<String>> {
    let spent = spent_resets(events);
    exhausted_pools
        .iter()
        .map(|(pool, reset_at)| {
            let reset_at = reset_at.clone().filter(|_| !spent.contains(pool));
            (pool.clone(), reset_at)
        })
        .collect()
}

fn spent_resets(events: &[TopologyEvent]) -> BTreeSet<String> {
    let margin = interaction::RESET_WAIT_MARGIN.as_secs();
    let mut recorded: BTreeMap<&String, u64> = BTreeMap::new();
    let mut spent = BTreeSet::new();
    let mut signalled: Vec<&String> = Vec::new();
    for event in events {
        match &event.body {
            TopologyEventBody::PoolExhausted { data } => {
                spent.remove(&data.pool);
                match data.reset_at.as_deref().and_then(unix_seconds_of) {
                    Some(reset) => recorded.insert(&data.pool, reset),
                    None => recorded.remove(&data.pool),
                };
                signalled.push(&data.pool);
            }
            TopologyEventBody::AttemptFinished { data } => {
                if let Some(pool) = super::limited_pool(&data.record) {
                    let refused_past = recorded
                        .get(pool)
                        .and_then(|reset| reset.checked_add(margin))
                        .zip(unix_seconds_of(&event.ts))
                        .is_some_and(|(spent_from, at)| at >= spent_from);
                    if refused_past && !signalled.contains(&pool) {
                        spent.insert(pool.clone());
                    }
                }
                signalled.clear();
            }
            _ => signalled.clear(),
        }
    }
    spent
}

pub(crate) fn serving_pool<'a>(
    candidates: &'a [String],
    held: &BTreeMap<String, Option<String>>,
    now_unix: u64,
) -> Option<&'a String> {
    let reset_passed = |pool: &&String| {
        held.get(*pool)
            .and_then(Option::as_deref)
            .and_then(unix_seconds_of)
            .and_then(|reset| reset.checked_add(interaction::RESET_WAIT_MARGIN.as_secs()))
            .is_some_and(|serving_from| serving_from <= now_unix)
    };
    candidates
        .iter()
        .find(|pool| !held.contains_key(*pool))
        .or_else(|| candidates.iter().find(reset_passed))
        .or_else(|| candidates.first())
}

#[cfg(test)]
mod test_support {
    use std::collections::BTreeMap;

    use super::BuiltinClient;
    use crate::topology::events::TopologyEvent;

    impl BuiltinClient {
        /// A client holding `exhausted_pools`, for a test that sets the map.
        pub(crate) fn holding(exhausted_pools: BTreeMap<String, Option<String>>) -> Self {
            Self { exhausted_pools }
        }

        /// The map itself, for a test that takes a pool out or puts one in.
        pub(crate) fn exhausted_pools_mut(&mut self) -> &mut BTreeMap<String, Option<String>> {
            &mut self.exhausted_pools
        }

        /// The map with each spent reset read from `events` dropped, as
        /// [`super::CapacityClient::grant`] gives it to `serving_pool`.
        pub(crate) fn held_unspent(
            &self,
            events: &[TopologyEvent],
        ) -> BTreeMap<String, Option<String>> {
            super::held_unspent(&self.exhausted_pools, events)
        }
    }
}
