//! The capacity client: the questions the engine asks about pools while a
//! run is live, and the built-in answer to each (DESIGN.md §13).
//!
//! The engine asks which pool an attempt binds, whether a refused attempt
//! fails over to another pool, which recorded reset a deferred run waits for,
//! and what a settled attempt says about the pools it drew on. It reads the
//! fold and the frozen plans and hands the client plain values: candidate
//! pools in file order, the run's events, the records of settled attempts.
//! Where the engine before the seam read a value only once a guard passed, or
//! one task at a time, it hands a closure or an iterator instead, so the
//! client reads it at the same point. [`BuiltinClient`] answers from the pools
//! the run's signals hold exhausted, as the engine did before the seam.

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
    /// The pools the client holds exhausted, each with its reset, and the
    /// resets the run's `events` show spent read as none: the scan
    /// [`CapacityClient::grant`] judges by.
    ///
    /// A value of its own, so the engine scans the log before it reads the
    /// clock `grant` judges at, in one argument list, as it did before the
    /// seam.
    fn held_unspent(&self, events: &[TopologyEvent]) -> HeldUnspent;

    /// The pool an attempt binds, among its agent's pools in file order
    /// (`candidates`), judged at `now_unix` against `held`, the client's
    /// [`CapacityClient::held_unspent`]. `None` when the agent names no pool.
    fn grant(&self, candidates: &[String], held: &HeldUnspent, now_unix: u64) -> Option<String>;

    /// Whether a refused attempt is retried at once on another of its agent's
    /// pools instead of deferred: a worker's rate limit, on a recorded pool,
    /// with some other candidate no signal holds exhausted.
    ///
    /// `candidates` gives the agent's pools in file order, and is called only
    /// once the failure is a worker's rate limit on a recorded pool.
    fn fails_over(
        &self,
        failure: &AttemptFailure,
        record: &AttemptRecord,
        candidates: &dyn Fn() -> Vec<String>,
    ) -> bool;

    /// The recorded reset a deferred run waits for, chosen among the resets
    /// the `limits` wait on, at `now_unix`, with spent resets read from the
    /// run's `events` before the first limit is drawn. `None` when the run
    /// backs off as before.
    fn awaited_reset(
        &self,
        limits: &mut dyn Iterator<Item = DeferredLimit<'_>>,
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

/// What [`CapacityClient::held_unspent`] read: each pool held exhausted, with
/// the reset [`CapacityClient::grant`] judges it by.
///
/// Its field is private, so only this file builds one or reads it: a client
/// elsewhere hands on the one a built-in client scanned.
#[derive(Debug)]
pub(crate) struct HeldUnspent {
    held: BTreeMap<String, Option<String>>,
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
    /// exhausted, as `capacity::exhausted_pools_of` reads them.
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
    fn held_unspent(&self, events: &[TopologyEvent]) -> HeldUnspent {
        HeldUnspent {
            held: held_unspent(&self.exhausted_pools, events),
        }
    }

    fn grant(&self, candidates: &[String], held: &HeldUnspent, now_unix: u64) -> Option<String> {
        serving_pool(candidates, &held.held, now_unix).cloned()
    }

    fn fails_over(
        &self,
        failure: &AttemptFailure,
        record: &AttemptRecord,
        candidates: &dyn Fn() -> Vec<String>,
    ) -> bool {
        if failure.kind != crate::ladder::FailureKind::RateLimited
            || failure.origin != crate::ladder::FailureOrigin::Worker
        {
            return false;
        }
        let Some(refused) = &record.pool else {
            return false;
        };
        candidates()
            .iter()
            .any(|pool| pool != refused && !self.exhausted_pools.contains_key(pool))
    }

    fn awaited_reset(
        &self,
        limits: &mut dyn Iterator<Item = DeferredLimit<'_>>,
        events: &[TopologyEvent],
        now_unix: u64,
    ) -> Option<AwaitedReset> {
        let spent = spent_resets(events);
        let known: Vec<(u64, &String, &String)> = limits
            .filter_map(|limit| self.resets_awaited_by(&limit, &spent, now_unix))
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
/// `events`) read as no reset: the held map the built-in
/// [`CapacityClient::held_unspent`] scans, which [`CapacityClient::grant`]
/// gives [`serving_pool`].
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

    use super::{BuiltinClient, CapacityClient, HeldUnspent};

    /// A client holding `exhausted_pools`, for a test that sets the map.
    pub(crate) fn holding(
        exhausted_pools: BTreeMap<String, Option<String>>,
    ) -> Box<dyn CapacityClient> {
        Box::new(BuiltinClient { exhausted_pools })
    }

    /// The map a client's [`CapacityClient::held_unspent`] scanned: each pool
    /// held exhausted, with the reset [`CapacityClient::grant`] judges it by.
    pub(crate) fn pools_held(scan: &HeldUnspent) -> BTreeMap<String, Option<String>> {
        scan.held.clone()
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

    /// The two engine files rule one does not read: the legacy writer of
    /// schemas 1–3, each of whose files writes `use crate::capacity;`.
    ///
    /// The exception is by file, so it covers everything in them, the
    /// schema-4 dispatch at the head of each (`run_harness` in
    /// `coordinator.rs`, `resume_harness` in `resume.rs`) included.
    const LEGACY_WRITER: [&str; 2] = ["src/engine/coordinator.rs", "src/engine/resume.rs"];

    /// The word rule two counts: the `pub(crate)` constructor of the
    /// built-in client, whose type is private to this file.
    const CONSTRUCTOR: &str = "builtin_capacity_client_from_log";

    /// The counts rule two accepts: each production file whose code names
    /// [`CONSTRUCTOR`] as a word, with how many times. The definition here
    /// names it once, and `TopologyRun::resumed` once.
    const CONSTRUCTOR_SITES: [(&str, usize); 2] = [
        ("src/capacity/client.rs", 1),
        ("src/engine/topology/run.rs", 1),
    ];

    /// Production source files, each keyed by its path below the repository
    /// root ([`key_of`]): every `.rs` file under `src/`, read as the
    /// tracker's census reads them, less the whole-file test modules the
    /// crate's own `#[cfg(test)] mod …;` declarations name.
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
                let key = key_of(path.strip_prefix(root).expect("a file under the root"));
                let source = std::fs::read_to_string(&path)
                    .unwrap_or_else(|error| panic!("{} reads: {error}", path.display()));
                (key, source)
            })
            .collect()
    }

    /// A file's key: `relative`, its path below the repository root, as its
    /// components joined by `/`.
    ///
    /// Each component is taken whole and must be UTF-8: a `\` in a Unix file
    /// name stays in its component, and a component that is not UTF-8
    /// panics. No platform allows a `/` inside a component, so the `/`s of a
    /// key are exactly the boundaries between its components.
    fn key_of(relative: &Path) -> String {
        relative
            .components()
            .map(|component| {
                component.as_os_str().to_str().unwrap_or_else(|| {
                    panic!(
                        "`{}` has a path component that is not UTF-8",
                        relative.display()
                    )
                })
            })
            .collect::<Vec<_>>()
            .join("/")
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

    /// Rule one: whether `code` writes a word-bounded `capacity` that is not
    /// followed, after optional whitespace, by `::` and then, after optional
    /// whitespace, by a word-bounded `client`.
    ///
    /// `code` is a file's code with comments, literals and test-only items
    /// blanked. Only what follows the word is read, never what is before it,
    /// so a `capacity` after `crate::`, `super::`, `Root::`, `.` or `..` is
    /// held to the same test. `crate::capacity::client::Pool` and
    /// `Root::capacity::client::Pool` pass it. Each of these fails it,
    /// whether or not it names the module: a path to another item
    /// (`crate::capacity::pool_for`, `(..capacity::x()).end`), a rename (`use
    /// crate::{capacity as c};`), a bare import (`use crate::capacity;`), a
    /// group (`use crate::capacity::{client, Pool};`, and `use
    /// crate::capacity::{client, client::Pool};` too), a method or field
    /// after a `.` (`v.capacity()`, `run.capacity`), an associated function
    /// (`Vec::capacity(v)`, `Vec::<u8>::capacity(v)`) and a binding or field
    /// declared `capacity`.
    ///
    /// Its limits: it reads words and resolves no path or name. Code that
    /// writes no word-bounded `capacity` passes, whatever its paths resolve
    /// to: a path a macro assembles from pieces, or a module alias or
    /// re-export declared by a file outside the engine (`pub(crate) use
    /// crate::capacity as cap;` in `src/util.rs`, then
    /// `crate::util::cap::exhausted_pools_of`).
    fn reaches_capacity_around_the_seam(code: &str) -> bool {
        const NAME: &str = "capacity";
        word_occurrences(code, NAME).into_iter().any(|at| {
            let through_the_client = code[at + NAME.len()..]
                .trim_start()
                .strip_prefix("::")
                .map(str::trim_start)
                .and_then(|rest| rest.strip_prefix("client"))
                .is_some_and(|rest| bounded(rest.chars().next()));
            !through_the_client
        })
    }

    /// The files rule one reads, each with its code: comments, literals and
    /// test-only items blanked. A file is read when its key's first two
    /// components are `src` and `engine` and it is not one of
    /// [`LEGACY_WRITER`].
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

    /// The files rule one reads ([`engine_production_code`]) whose code meets
    /// [`reaches_capacity_around_the_seam`], in the order of `sources`.
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
    /// nothing, and the wrapper's own file counts the word it writes.
    ///
    /// Its limit: it counts words, not calls. One naming bound to a local and
    /// called twice counts once.
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
    /// [`CONSTRUCTOR`] exactly at [`CONSTRUCTOR_SITES`], file for file and
    /// count for count ([`constructor_sites`]).
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
            "only {} production source files were read, where the census expects more than 150",
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
            "the census reads `src/engine/topology/run/tests.rs`, a whole-file test module, as \
             production"
        );
        assert!(
            reaches_capacity_around_the_seam(&crate::effects::production_code(source_of(
                &sources,
                "src/engine/coordinator.rs"
            ))),
            "rule one refuses nothing in `src/engine/coordinator.rs`'s production code, so the \
             legacy writer's exception exempts nothing there"
        );
        assert!(
            crate::effects::production_code(source_of(&sources, "src/engine/topology/run.rs"))
                .contains("crate::capacity::client::"),
            "the production code of `src/engine/topology/run.rs` does not contain \
             `crate::capacity::client::`"
        );
        assert_eq!(
            bypasses_the_seam(&sources),
            Vec::<&str>::new(),
            "these files under `src/engine/`, the legacy writer of schemas 1–3 \
             ({LEGACY_WRITER:?}) excepted, write a word-bounded `capacity` that is not followed \
             by `::` and a word-bounded `client`, whitespace allowed around the `::`. Rule one \
             reads no word before `capacity`, so it refuses every other `capacity`: a path to \
             another item, a group (`capacity::{{client, ..}}`), a rename, and a binding, field, \
             method or associated function called `capacity`, after a `.` or not"
        );
    }

    #[test]
    fn the_builtin_client_is_built_only_by_the_run() {
        assert_eq!(
            constructor_sites(&production_sources()),
            CONSTRUCTOR_SITES,
            "production code names `{CONSTRUCTOR}` as a word in other files, or other times, \
             than rule two accepts: once in `src/capacity/client.rs`, its definition, and once \
             in `src/engine/topology/run.rs`, in `TopologyRun::resumed`. Every word counts, \
             whatever is around it; a file missing from the counts names it nowhere"
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
                "src/engine/range.rs",
                "fn f() -> usize {\n    (..capacity::x()).end\n}\n",
            ),
            // Accepted: the seam, prose, test code, a file outside the engine,
            // and the legacy writer.
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
                "src/engine/method.rs",
                "src/engine/dotted.rs",
                "src/engine/range.rs",
            ]
        );

        // A limit of rule one, accepted: a module alias declared by a file
        // outside the engine. The engine file reaches a function the client
        // does not re-export, and writes no `capacity`.
        let aliased = planted(&[
            ("src/util.rs", "pub(crate) use crate::capacity as cap;\n"),
            (
                "src/engine/realiased.rs",
                "fn f(e: &[E]) -> Held {\n    crate::util::cap::exhausted_pools_of(e)\n}\n",
            ),
        ]);
        assert_eq!(
            bypasses_the_seam(&aliased),
            Vec::<&str>::new(),
            "a limit of rule one: it reads no file outside `src/engine/`, and an engine file \
             that reaches the capacity module through a module alias declared there writes no \
             `capacity`"
        );
    }

    #[test]
    fn the_census_keys_a_file_by_its_path_components() {
        let run = ["src", "engine", "topology", "run.rs"]
            .into_iter()
            .collect::<PathBuf>();
        assert_eq!(key_of(&run), "src/engine/topology/run.rs");
        if cfg!(unix) {
            // On Unix `\` is a character of a file name, so this is a file in
            // `src/` named `engine\planted.rs`, not a file in `src/engine/`.
            let named = Path::new("src").join("engine\\planted.rs");
            assert_eq!(named.components().count(), 2);
            let key = key_of(&named);
            assert_eq!(key, "src/engine\\planted.rs");
            let sources = vec![(
                key,
                "fn f() { let _ = crate::capacity::pool_for; }\n".to_owned(),
            )];
            assert_eq!(
                bypasses_the_seam(&sources),
                Vec::<&str>::new(),
                "rule one read `src/engine\\planted.rs`, whose first two components are `src` \
                 and `engine\\planted.rs`, as a file under `src/engine/`"
            );
        }
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

        // A limit of rule two, accepted: one naming, bound to a local and
        // called twice. The run builds two clients and names the constructor
        // once.
        let called_twice = planted(&[
            (CLIENT, DEFINITION),
            (
                RUN,
                "fn resumed(handle: H) -> TopologyRun {\n    \
                 let build = crate::capacity::client::builtin_capacity_client_from_log;\n    \
                 let capacity_client = build(&handle.events);\n    \
                 let _second = build(&handle.events);\n    \
                 TopologyRun { capacity_client }\n}\n",
            ),
        ]);
        assert_eq!(
            constructor_sites(&called_twice),
            CONSTRUCTOR_SITES,
            "a limit of rule two: it counts words, not calls, and one naming called twice is one \
             word"
        );
        assert!(
            only_the_run_builds_the_client(&called_twice),
            "a limit of rule two: it counts words, not calls, and one naming called twice is one \
             word"
        );

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
