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

pub(crate) use super::{Pool, drain_of, pool_for};

/// The questions the engine asks about capacity while a run is live.
pub(crate) trait CapacityClient {
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

    /// The `pool` and `reset_at` of a `pool_exhausted` signal the run has
    /// just recorded: the pool is held, with the reset the signal named.
    fn held(&mut self, pool: String, reset_at: Option<String>);
}

/// A task the fold holds `Deferred` whose last settled attempt failed
/// `RateLimited`, as the engine reads it for [`CapacityClient::awaited_reset`].
#[derive(Debug, Clone)]
pub(crate) struct DeferredLimit<'a> {
    /// The last settled attempt's record, whose failure is the rate limit.
    pub(crate) record: &'a AttemptRecord,
    /// The pools of the agent bound on the task's rung, in file order, when
    /// the failure's origin is the worker. Empty for a reviewer's limit.
    pub(crate) worker_pools: Vec<String>,
}

/// What [`CapacityClient::awaited_reset`] chose: the pool and reset a
/// deferred run waits for, and the unix second the wait runs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AwaitedReset {
    /// The pool whose reset the run waits for.
    pub(crate) pool: String,
    /// The reset as its signal recorded it.
    pub(crate) reset_at: String,
    /// The unix second the wait runs to: the reset plus the margin.
    ///
    /// The run's deferral is given it when the wait starts
    /// (`Deferral::wait_toward`) and holds it while later slices are slept
    /// toward it (`Deferral::held_until`), and `Progress::Waiting` reports it.
    pub(crate) until_unix: u64,
}

/// The client a resumed run starts from: the built-in one, holding the pools
/// its `events` hold exhausted ([`BuiltinClient::replay`]). It is the one way
/// to build a [`BuiltinClient`] outside this file, so the client a run
/// installs is the only one it asks.
#[must_use]
pub(crate) fn builtin_capacity_client_from_log(
    events: &[TopologyEvent],
) -> Box<dyn CapacityClient> {
    Box::new(BuiltinClient::replay(events))
}

/// The built-in client: each pool the run's signals hold exhausted, with the
/// reset its latest signal named.
#[derive(Debug, Default)]
struct BuiltinClient {
    exhausted_pools: BTreeMap<String, Option<String>>,
}

impl BuiltinClient {
    /// The client a resumed run starts from: the pools its `events` hold
    /// exhausted, as `capacity::observe` reads them.
    #[must_use]
    fn replay(events: &[TopologyEvent]) -> Self {
        Self {
            exhausted_pools: super::exhausted_pools_of(events),
        }
    }

    /// The recorded resets one deferred `limit` waits on, each as unix
    /// seconds with its pool and its text.
    ///
    /// `None` when a pool it would run on is free. A pool a later attempt
    /// proved serving, for another task perhaps, can take this one at its
    /// next wake, and waiting out the reset of a pool it no longer needs
    /// would only hold it back.
    ///
    /// Which pools those are depends on who was limited. A worker's limit
    /// waits on every pool of its agent (`limit.worker_pools`): the
    /// candidates [`serving_pool`] chooses among when the task wakes, so the
    /// two agree about where it can run. It defers only when it could not
    /// fail over ([`CapacityClient::fails_over`]), which leaves every one of
    /// them held, and the task can run again as soon as any one resets. A
    /// reviewer's limit waits on the reviewer's pool, the one `limited_pool`
    /// names: a reviewer runs on its agent's first pool whatever is held, so
    /// no other pool's reset helps it.
    ///
    /// A held candidate gives no reset when none is recorded, when its reset
    /// does not read as a date ([`unix_seconds_of`]), or when it is in
    /// `spent`. When the limit cannot fail over, a worker's on an agent with
    /// one pool or a reviewer's, a reset at or behind `now_unix` gives none
    /// too. Only a limit that fails over can leave a passed reset on another
    /// pool than the one that refused, so only there does
    /// [`CapacityClient::awaited_reset`] wait out its margin.
    fn resets_awaited_by(
        &self,
        limit: &DeferredLimit<'_>,
        spent: &BTreeSet<String>,
        now_unix: u64,
    ) -> Option<Vec<(u64, &String, &String)>> {
        let record = limit.record;
        let failure = record.failure.as_ref()?;
        let candidates: &[String] = match failure.origin {
            crate::ladder::FailureOrigin::Worker => &limit.worker_pools,
            crate::ladder::FailureOrigin::Reviewer => {
                std::slice::from_ref(super::limited_pool(record)?)
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

    fn held(&mut self, pool: String, reset_at: Option<String>) {
        self.exhausted_pools.insert(pool, reset_at);
    }
}

/// `exhausted_pools` with each spent reset ([`spent_resets`], read from
/// `events`) read as no reset: the held map [`CapacityClient::grant`] gives
/// [`serving_pool`].
///
/// So a pool whose reset is spent is held with no known reset: it serves
/// only when no pool is free and no other held pool's reset and margin have
/// passed, as a pool whose limit named no reset does. `exhausted_pools`
/// itself keeps the reset, so [`CapacityClient::fails_over`] and
/// [`CapacityClient::exhaustion`] see the pool held.
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

/// The pools whose recorded reset is spent: the pool refused an attempt
/// again, two minutes or more past that reset, after the `pool_exhausted`
/// that recorded it.
///
/// Read from the log alone, so a resumed run reads the same set as the
/// process that wrote it. A `pool_exhausted` records the reset it names, or
/// none, and its pool is not spent. An `attempt_finished` limited on the
/// pool, its `ts` at or past the recorded reset plus
/// [`interaction::RESET_WAIT_MARGIN`], spends it, unless it is the refusal
/// that recorded the reset: the `attempt_finished` that follows its
/// `pool_exhausted` with nothing between, as `exhausted_pools_of` reads it.
/// That one is not counted, even when the reset it named was already behind
/// the clock. Any other event has no effect on its pool.
///
/// The margin is added as [`serving_pool`] adds it: the instant a held pool
/// serves from, and the one a wait toward the reset runs to. A pool can come
/// back a little after the reset it names, which is what the margin is for,
/// so a refusal inside it says nothing yet about whether the reset brought
/// the pool back.
///
/// A later refusal past the reset and its margin is the evidence that the
/// reset did not bring the pool back. It names the same reset, or none it
/// can read, so `pool_exhausted_to_record` writes no new signal for it, and
/// without this rule the run would keep the old reset and serve the pool
/// from it again and again. A later `pool_exhausted` for the pool, with a
/// reset of its own, makes it unspent.
///
/// [`held_unspent`] (for [`serving_pool`]) and
/// [`BuiltinClient::resets_awaited_by`] read it; `exhausted_pools` itself
/// and the failover do not.
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

/// The pool a new attempt binds, among `candidates`: its agent's pools in
/// file order.
///
/// `held` is each pool held exhausted, with its reset ([`held_unspent`]'s
/// map, as [`CapacityClient::grant`] gives it), and `now_unix` the run's
/// clock.
///
/// 1. The first candidate `held` does not contain. Whether a held pool
///    earlier in the order has a reset behind the clock does not matter
///    here: a pool no signal holds is taken before any pool one does.
/// 2. With every candidate held, the first whose recorded reset, plus
///    [`interaction::RESET_WAIT_MARGIN`], is at or before `now_unix`. That
///    is the moment a wait toward the reset ends
///    ([`interaction::reset_wait_until`]), so a pool whose reset the run
///    waited out counts as passed when the task it deferred wakes. A pool
///    held with no reset, or with one that does not read as a date
///    ([`unix_seconds_of`]), has not passed.
/// 3. With every candidate held and none passed, the first candidate. The
///    attempt runs into the limit again, and the ladder's deferral bounds
///    how often.
/// 4. `None` when there are no candidates: an agent no pool names binds
///    none.
///
/// A pool that is held but is not a candidate is another agent's and
/// changes nothing.
fn serving_pool<'a>(
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

/// What a test outside this file reaches the built-in client through: each
/// function hands back a `dyn CapacityClient` or plain values, never the
/// client's own type.
#[cfg(test)]
pub(crate) mod test_support {
    use std::collections::BTreeMap;

    use super::{BuiltinClient, CapacityClient};
    use crate::topology::events::TopologyEvent;

    /// A client holding `exhausted_pools`, for a test that sets the map.
    pub(crate) fn holding(
        exhausted_pools: BTreeMap<String, Option<String>>,
    ) -> Box<dyn CapacityClient> {
        Box::new(BuiltinClient { exhausted_pools })
    }

    /// The client a resumed run starts from (`BuiltinClient::replay` of
    /// `events`), with `pool` taken out of its map.
    pub(crate) fn replayed_releasing(
        events: &[TopologyEvent],
        pool: &str,
    ) -> Box<dyn CapacityClient> {
        let mut client = BuiltinClient::replay(events);
        client.exhausted_pools.remove(pool);
        Box::new(client)
    }

    /// The client a resumed run starts from (`BuiltinClient::replay` of
    /// `events`), with `pool` put in its map, held with `reset_at`.
    pub(crate) fn replayed_holding(
        events: &[TopologyEvent],
        pool: String,
        reset_at: Option<String>,
    ) -> Box<dyn CapacityClient> {
        let mut client = BuiltinClient::replay(events);
        client.exhausted_pools.insert(pool, reset_at);
        Box::new(client)
    }

    /// The map of the client a resumed run starts from
    /// (`BuiltinClient::replay` of `events`), with each spent reset read from
    /// `events` dropped, as [`super::CapacityClient::grant`] gives it to
    /// `serving_pool`.
    pub(crate) fn replayed_held_unspent(
        events: &[TopologyEvent],
    ) -> BTreeMap<String, Option<String>> {
        super::held_unspent(&BuiltinClient::replay(events).exhausted_pools, events)
    }

    /// The pool `serving_pool` binds among `candidates`, with `held` and
    /// `now_unix` as [`super::CapacityClient::grant`] gives them.
    pub(crate) fn serving_pool<'a>(
        candidates: &'a [String],
        held: &BTreeMap<String, Option<String>>,
        now_unix: u64,
    ) -> Option<&'a String> {
        super::serving_pool(candidates, held, now_unix)
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    /// The legacy writer of schemas 1–3, which reads the capacity module
    /// directly and is the seam census's one named exception.
    const LEGACY_WRITER: [&str; 2] = ["src/engine/coordinator.rs", "src/engine/resume.rs"];

    /// The `pub(crate)` constructor of the built-in client, the one way to
    /// build one outside this file.
    const CONSTRUCTOR: &str = "builtin_capacity_client_from_log";

    /// The production files that name [`CONSTRUCTOR`], each with how many
    /// times: its definition here, and its one call, in `TopologyRun::resumed`.
    const CONSTRUCTOR_SITES: [(&str, usize); 2] = [
        ("src/capacity/client.rs", 1),
        ("src/engine/topology/run.rs", 1),
    ];

    /// Production source files, by repository-relative path: every `.rs`
    /// file under `src/`, read as the tracker's census reads them, less the
    /// whole-file test modules the crate's own `#[cfg(test)] mod …;`
    /// declarations name.
    fn production_sources() -> Vec<(String, String)> {
        fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
            let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
                .unwrap_or_else(|error| panic!("{} lists: {error}", dir.display()))
                .map(|entry| entry.expect("a directory entry").path())
                .collect();
            entries.sort();
            for path in entries {
                if path.is_dir() {
                    walk(&path, out);
                } else if path.extension().is_some_and(|extension| extension == "rs") {
                    out.push(path);
                }
            }
        }
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let source_root = root.join("src");
        let mut files = Vec::new();
        walk(&source_root, &mut files);
        let test_modules =
            crate::effects::census_domain::whole_file_test_modules(&source_root, &files, 13);
        files
            .into_iter()
            .filter(|path| !test_modules.contains(path))
            .map(|path| {
                let relative = path
                    .strip_prefix(root)
                    .expect("a file under the root")
                    .to_string_lossy()
                    .replace('\\', "/");
                let source = std::fs::read_to_string(&path)
                    .unwrap_or_else(|error| panic!("{} reads: {error}", path.display()));
                (relative, source)
            })
            .collect()
    }

    fn is_identifier(character: char) -> bool {
        character.is_alphanumeric() || character == '_'
    }

    fn bounded(character: Option<char>) -> bool {
        character.is_none_or(|character| !is_identifier(character))
    }

    /// The byte offsets of the word-bounded occurrences of `word` in `code`.
    fn word_occurrences(code: &str, word: &str) -> Vec<usize> {
        code.match_indices(word)
            .map(|(at, _)| at)
            .filter(|at| {
                bounded(code[..*at].chars().next_back())
                    && bounded(code[at + word.len()..].chars().next())
            })
            .collect()
    }

    /// Whether `code`, with comments, literals and test-only items blanked,
    /// writes `capacity` other than as `capacity::client`: rule one, the seam.
    ///
    /// The census reads words, not paths. Each word-bounded `capacity` that
    /// does not follow a `.` (after optional whitespace) must be followed,
    /// after optional whitespace, by `::` and then, after optional
    /// whitespace, by the word-bounded `client`; any other is a reach around
    /// the seam. Nothing before the word is read, so `crate::capacity`,
    /// `super::capacity` and `Root::capacity` after `use crate as Root;` are
    /// read alike. A method or field after a `.` (`v.capacity()`,
    /// `run.capacity`) is no segment of a path, and passes.
    ///
    /// Every other `capacity` fails, whether or not it names the module: a
    /// rename (`use crate::{capacity as c};`), a bare import (`use
    /// crate::capacity;`), a group (`use crate::capacity::{client, Pool};`,
    /// and `use crate::capacity::{client, client::Pool};` too, though each of
    /// its items is the client's), a type's associated function
    /// (`Vec::capacity(v)`, `Vec::<u8>::capacity(v)`) and a binding or field
    /// declared `capacity`. Telling those apart would take a parser of Rust
    /// paths; refusing them leaves one spelling, `capacity::client::<item>`,
    /// written in full.
    ///
    /// A text census reads the words a file writes: a path a macro assembles
    /// from pieces, or a re-export by a file outside the engine (`pub(crate)
    /// use crate::capacity::pool_for as reach;` in `src/util.rs`, then
    /// `crate::util::reach`), writes no `capacity` in the engine and passes
    /// unseen.
    fn reaches_capacity_around_the_seam(code: &str) -> bool {
        const NAME: &str = "capacity";
        word_occurrences(code, NAME).into_iter().any(|at| {
            if code[..at].trim_end().ends_with('.') {
                return false;
            }
            let through_the_client = code[at + NAME.len()..]
                .trim_start()
                .strip_prefix("::")
                .map(str::trim_start)
                .and_then(|rest| rest.strip_prefix("client"))
                .is_some_and(|rest| bounded(rest.chars().next()));
            !through_the_client
        })
    }

    /// The engine's production files, but the legacy writer's, each with its
    /// production code: comments, literals and test-only items blanked.
    fn engine_production_code(
        sources: &[(String, String)],
    ) -> impl Iterator<Item = (&str, String)> {
        sources
            .iter()
            .filter(|(path, _)| {
                path.starts_with("src/engine/") && !LEGACY_WRITER.contains(&path.as_str())
            })
            .map(|(path, source)| (path.as_str(), crate::effects::production_code(source)))
    }

    /// The engine's production files, but the legacy writer's, whose code
    /// reaches the capacity module other than through `capacity::client`.
    fn bypasses_the_seam(sources: &[(String, String)]) -> Vec<&str> {
        engine_production_code(sources)
            .filter(|(_, code)| reaches_capacity_around_the_seam(code))
            .map(|(path, _)| path)
            .collect()
    }

    /// The production files whose code, with comments, literals and
    /// test-only items blanked, names [`CONSTRUCTOR`] as a word, each with how
    /// many times it does.
    ///
    /// Every word counts, whatever is around it: a definition, a call, a
    /// call behind `impl CapacityClient + use<>`, a rename in a `use`, a
    /// function value. A file that builds a client through a wrapper counts
    /// nothing, but the wrapper's own file counts its call.
    fn constructor_sites(sources: &[(String, String)]) -> Vec<(&str, usize)> {
        sources
            .iter()
            .map(|(path, source)| {
                let code = crate::effects::production_code(source);
                (path.as_str(), word_occurrences(&code, CONSTRUCTOR).len())
            })
            .filter(|(_, sites)| *sites > 0)
            .collect()
    }

    /// Rule two, the constructor: whether production code names
    /// [`CONSTRUCTOR`] exactly at [`CONSTRUCTOR_SITES`], so the client the run
    /// installs is the one built-in client there is.
    fn only_the_run_builds_the_client(sources: &[(String, String)]) -> bool {
        constructor_sites(sources) == CONSTRUCTOR_SITES
    }

    /// Owned `(path, source)` pairs, for a planted census.
    fn planted(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(path, source)| ((*path).to_owned(), (*source).to_owned()))
            .collect()
    }

    fn source_of<'a>(sources: &'a [(String, String)], wanted: &str) -> &'a str {
        sources
            .iter()
            .find(|(path, _)| path == wanted)
            .map(|(_, source)| source.as_str())
            .unwrap_or_else(|| panic!("`{wanted}` is not in the census's domain"))
    }

    #[test]
    fn only_the_seam_reaches_capacity_from_the_engine() {
        let sources = production_sources();
        assert!(
            sources.len() > 150,
            "only {} production source files were read; the census is measuring nothing",
            sources.len()
        );
        for required in [
            "src/engine/topology/run.rs",
            "src/engine/assembly.rs",
            "src/engine/coordinator.rs",
        ] {
            assert!(
                sources.iter().any(|(path, _)| path == required),
                "`{required}` is not in the census's domain"
            );
        }
        assert!(
            sources
                .iter()
                .all(|(path, _)| path != "src/engine/topology/run/tests.rs"),
            "the census reads test modules as production, where a test may build any client it \
             likes"
        );
        assert!(
            reaches_capacity_around_the_seam(&crate::effects::production_code(source_of(
                &sources,
                "src/engine/coordinator.rs"
            ))),
            "`src/engine/coordinator.rs` no longer names the capacity module directly, so the \
             legacy writer's exception exempts nothing and is to be removed"
        );
        assert!(
            crate::effects::production_code(source_of(&sources, "src/engine/topology/run.rs"))
                .contains("crate::capacity::client::"),
            "the schema-4 run no longer asks capacity through `capacity::client`, so a census \
             finding no bypass proves nothing about the seam"
        );
        assert_eq!(
            bypasses_the_seam(&sources),
            Vec::<&str>::new(),
            "the schema-4 engine asks capacity only through `CapacityClient` \
             (`crate::capacity::client`), so a second client can replace the built-in one; a \
             file that names the capacity module any other way answers a capacity question \
             the replacement never sees. The census reads words, so it accepts one spelling: \
             `capacity::client::<item>`, written in full on every path, never grouped \
             (`capacity::{{client, ..}}`) or renamed; a binding, field or associated function \
             called `capacity` is refused with them, and a method or field is read only after a \
             `.`. The legacy writer of schemas 1–3 ({LEGACY_WRITER:?}) is the one named \
             exception"
        );
    }

    #[test]
    fn the_builtin_client_is_built_only_by_the_run() {
        assert_eq!(
            constructor_sites(&production_sources()),
            CONSTRUCTOR_SITES,
            "`{CONSTRUCTOR}` is the one way to build the built-in client outside \
             `src/capacity/client.rs`, so production code names it twice: its definition there \
             and its call in `TopologyRun::resumed` (`src/engine/topology/run.rs`). The run asks \
             the client it installed (`self.capacity_client`), so a second client anywhere, \
             another file's, a second one in the run, or one a wrapper builds, answers capacity \
             questions an installed handler never sees. A file missing from the counts means \
             the constructor was renamed or the run no longer calls it, and this census proves \
             nothing until it names the one that builds the run's client"
        );
    }

    #[test]
    fn the_capacity_census_sees_a_bypass_and_ignores_prose() {
        let sources = planted(&[
            // Refused: each names the module, or writes the word, other than
            // as `capacity::client`.
            (
                "src/engine/planted.rs",
                "fn f() { let _ = crate::capacity::pool_for; }\n",
            ),
            ("src/engine/aliased.rs", "use crate::{capacity as c};\n"),
            ("src/engine/bare.rs", "use crate::capacity;\n"),
            (
                "src/engine/grouped.rs",
                "use crate::capacity::{client, Pool};\n",
            ),
            (
                "src/engine/rooted.rs",
                "use crate as Root;\nuse Root::capacity::drain_of;\n",
            ),
            (
                "src/engine/upper.rs",
                "fn f() { let _ = Root::capacity::pool_for; }\n",
            ),
            (
                "src/engine/regrouped.rs",
                "use crate::capacity::{client, client::Pool};\n",
            ),
            (
                "src/engine/associated.rs",
                "fn f(v: &Vec<u8>) -> usize { Vec::capacity(v) }\n",
            ),
            (
                "src/engine/turbofish.rs",
                "fn f(v: &Vec<u8>) -> usize { Vec::<u8>::capacity(v) }\n",
            ),
            (
                "src/engine/field.rs",
                "struct Run { capacity: Box<dyn CapacityClient> }\n",
            ),
            (
                "src/engine/binding.rs",
                "fn f(capacity: Box<dyn CapacityClient>) -> Run {\n    \
                 Run { capacity_client: capacity }\n}\n",
            ),
            // Accepted: the seam, a method or field after a `.`, prose, test
            // code, a file outside the engine, and the legacy writer.
            ("src/engine/seam.rs", "use crate::capacity::client::Pool;\n"),
            (
                "src/engine/called.rs",
                "fn f(agent: &str, pools: &[Pool]) -> Option<&Pool> {\n    \
                 crate::capacity::client::pool_for(agent, pools)\n}\n",
            ),
            (
                "src/engine/spaced.rs",
                "fn f(agent: &str, pools: &[Pool]) -> Option<&Pool> {\n    \
                 crate::capacity\n        :: client::pool_for(agent, pools)\n}\n",
            ),
            (
                "src/engine/method.rs",
                "fn f(v: &Vec<u8>) -> usize {\n    v.capacity() + v\n        .capacity()\n}\n",
            ),
            (
                "src/engine/dotted.rs",
                "fn f(run: &Run, other: &Run) -> bool {\n    \
                 run.capacity.is_some() && other . capacity.is_some()\n}\n",
            ),
            (
                "src/engine/prose.rs",
                "// the capacity a pool has left; `capacity: Box<dyn CapacityClient>`\n\
                 /// `use crate::capacity;`\n\
                 const S: &str = \"let capacity = crate::capacity::pool_for;\";\n",
            ),
            (
                "src/engine/tested.rs",
                "fn f() {}\n#[cfg(test)]\nmod tests {\n    use crate::capacity::pool_for;\n    \
                 struct Run { capacity: u8 }\n    \
                 fn g(capacity: u8) -> Run { Run { capacity } }\n}\n",
            ),
            (
                "src/cli.rs",
                "use crate::capacity;\nfn f(capacity: u8) -> u8 { capacity }\n",
            ),
            (
                "src/engine/coordinator.rs",
                "fn f() { let _ = crate::capacity::observe; }\n",
            ),
            (
                "src/engine/resume.rs",
                "use crate::capacity;\nfn f(capacity: u8) -> u8 { capacity }\n",
            ),
        ]);
        assert_eq!(
            bypasses_the_seam(&sources),
            vec![
                "src/engine/planted.rs",
                "src/engine/aliased.rs",
                "src/engine/bare.rs",
                "src/engine/grouped.rs",
                "src/engine/rooted.rs",
                "src/engine/upper.rs",
                "src/engine/regrouped.rs",
                "src/engine/associated.rs",
                "src/engine/turbofish.rs",
                "src/engine/field.rs",
                "src/engine/binding.rs",
            ]
        );
    }

    #[test]
    fn the_constructor_census_sees_a_second_client() {
        const CLIENT: &str = "src/capacity/client.rs";
        const RUN: &str = "src/engine/topology/run.rs";
        const DEFINITION: &str = "pub(crate) fn builtin_capacity_client_from_log(\n    \
                                  events: &[TopologyEvent],\n) -> Box<dyn CapacityClient> {\n    \
                                  Box::new(BuiltinClient::replay(events))\n}\n";
        const RESUMED: &str = "fn resumed(handle: H) -> TopologyRun {\n    \
                               let capacity_client =\n        \
                               crate::capacity::client::builtin_capacity_client_from_log(\
                               &handle.events);\n    \
                               TopologyRun { capacity_client }\n}\n";
        const WRAPPER: &str = "pub(crate) fn client_of(e: &[E]) -> Box<dyn CapacityClient> {\n    \
                               crate::capacity::client::builtin_capacity_client_from_log(e)\n}\n";
        const RESUMED_THROUGH_THE_WRAPPER: &str = "fn resumed(handle: H) -> TopologyRun {\n    \
                                                   let capacity_client = \
                                                   crate::util::client_of(&handle.events);\n    \
                                                   TopologyRun { capacity_client }\n}\n";
        let twice_plain = format!(
            "{RESUMED}fn second(e: &[E]) -> Box<dyn CapacityClient> {{\n    \
             crate::capacity::client::builtin_capacity_client_from_log(e)\n}}\n"
        );
        let twice_behind_use = format!(
            "{RESUMED}fn f(e: &[E]) -> impl CapacityClient + use<> {{\n    \
             crate::capacity::client::builtin_capacity_client_from_log(e)\n}}\n"
        );

        let accepted = planted(&[
            (CLIENT, DEFINITION),
            (RUN, RESUMED),
            (
                "src/engine/prose.rs",
                "// builtin_capacity_client_from_log builds the run's client\n\
                 const S: &str = \"builtin_capacity_client_from_log(&handle.events)\";\n",
            ),
            (
                "src/engine/tested.rs",
                "fn f() {}\n#[cfg(test)]\nmod tests {\n    \
                 fn g(e: &[E]) { builtin_capacity_client_from_log(e); }\n}\n",
            ),
        ]);
        assert_eq!(constructor_sites(&accepted), CONSTRUCTOR_SITES);
        assert!(only_the_run_builds_the_client(&accepted));

        /// A refused case: what it plants, and the counts the census reads.
        type Refused<'a> = (&'a str, Vec<(String, String)>, Vec<(&'a str, usize)>);
        let refused: [Refused<'_>; 5] = [
            (
                "a second call in another production file",
                planted(&[
                    (CLIENT, DEFINITION),
                    (RUN, RESUMED),
                    (
                        "src/engine/second.rs",
                        "fn f(e: &[E]) -> Option<String> {\n    \
                         crate::capacity::client::builtin_capacity_client_from_log(e)\
                         .grant(&[], e, 0)\n}\n",
                    ),
                ]),
                vec![(CLIENT, 1), (RUN, 1), ("src/engine/second.rs", 1)],
            ),
            (
                "a second call in the run",
                planted(&[(CLIENT, DEFINITION), (RUN, twice_plain.as_str())]),
                vec![(CLIENT, 1), (RUN, 2)],
            ),
            (
                "a second call in the run, behind `impl CapacityClient + use<>`",
                planted(&[(CLIENT, DEFINITION), (RUN, twice_behind_use.as_str())]),
                vec![(CLIENT, 1), (RUN, 2)],
            ),
            (
                "a wrapper in `src/util.rs`, beside the run's own call",
                planted(&[
                    (CLIENT, DEFINITION),
                    (RUN, RESUMED),
                    ("src/util.rs", WRAPPER),
                ]),
                vec![(CLIENT, 1), (RUN, 1), ("src/util.rs", 1)],
            ),
            (
                "a wrapper in `src/util.rs`, which the run calls instead",
                planted(&[
                    (CLIENT, DEFINITION),
                    (RUN, RESUMED_THROUGH_THE_WRAPPER),
                    ("src/util.rs", WRAPPER),
                ]),
                vec![(CLIENT, 1), ("src/util.rs", 1)],
            ),
        ];
        for (case, sources, sites) in &refused {
            assert_eq!(&constructor_sites(sources), sites, "{case}");
            assert!(!only_the_run_builds_the_client(sources), "{case}");
        }
    }
}
