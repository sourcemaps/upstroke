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

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    /// The legacy writer of schemas 1–3, which reads the capacity module
    /// directly and is the census's one named exception.
    const LEGACY_WRITER: [&str; 2] = ["src/engine/coordinator.rs", "src/engine/resume.rs"];

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

    /// Whether the code before `at` is inside a `use` declaration: the last
    /// word-bounded `use` before it has no `;` after it.
    fn in_a_use_declaration(code: &str, at: usize) -> bool {
        let head = &code[..at];
        head.rmatch_indices("use")
            .find(|(start, word)| {
                bounded(head[..*start].chars().next_back())
                    && bounded(head[start + word.len()..].chars().next())
            })
            .is_some_and(|(start, _)| !head[start..].contains(';'))
    }

    /// Whether `code`, with comments, literals and test-only items blanked,
    /// names `capacity` as a path segment that does not go on to `::client`.
    ///
    /// An occurrence is a path segment when `::` joins it to the path before
    /// or after it, or when a `use` declaration names it (`use
    /// crate::{capacity as c};`). It is not one after a `.` (a method or field
    /// such as `v.capacity()`), nor after a type's `::` (`Vec::capacity`,
    /// `Vec::<u8>::capacity`): neither is the module. A binding or field
    /// called `capacity` (`capacity: Box<dyn CapacityClient>`) is not joined
    /// to a path and is not a segment either.
    fn reaches_capacity_around_the_seam(code: &str) -> bool {
        const NAME: &str = "capacity";
        code.match_indices(NAME).any(|(at, _)| {
            let raw_before = &code[..at];
            let raw_after = &code[at + NAME.len()..];
            if !bounded(raw_before.chars().next_back()) || !bounded(raw_after.chars().next()) {
                return false;
            }
            let before = raw_before.trim_end();
            let after = raw_after.trim_start();
            if before.ends_with('.') {
                return false;
            }
            let owner = before.strip_suffix("::").map(str::trim_end);
            let owned_by_a_type = owner.is_some_and(|owner| {
                owner.ends_with('>')
                    || owner
                        .rsplit(|character: char| !is_identifier(character))
                        .next()
                        .and_then(|segment| segment.chars().next())
                        .is_some_and(char::is_uppercase)
            });
            if owned_by_a_type {
                return false;
            }
            let segment =
                owner.is_some() || after.starts_with("::") || in_a_use_declaration(code, at);
            let through_the_client = after
                .strip_prefix("::")
                .map(str::trim_start)
                .and_then(|rest| rest.strip_prefix("client"))
                .is_some_and(|rest| bounded(rest.chars().next()));
            segment && !through_the_client
        })
    }

    /// The engine's production files, but the legacy writer's, whose code
    /// reaches the capacity module other than through `capacity::client`.
    fn bypasses_the_seam(sources: &[(String, String)]) -> Vec<&str> {
        sources
            .iter()
            .filter(|(path, _)| {
                path.starts_with("src/engine/") && !LEGACY_WRITER.contains(&path.as_str())
            })
            .filter(|(_, source)| {
                reaches_capacity_around_the_seam(&crate::effects::production_code(source))
            })
            .map(|(path, _)| path.as_str())
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
             the replacement never sees. The legacy writer of schemas 1–3 ({LEGACY_WRITER:?}) \
             is the one named exception"
        );
    }

    #[test]
    fn the_capacity_census_sees_a_bypass_and_ignores_prose() {
        let planted: Vec<(String, String)> = [
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
                "src/engine/seam.rs",
                "use crate::capacity::client::{CapacityClient, Pool};\n",
            ),
            (
                "src/engine/prose.rs",
                "// the capacity a pool has left\nconst S: &str = \"capacity\";\n\
                 fn f(v: &Vec<u8>) -> usize { v.capacity() }\n",
            ),
            (
                "src/engine/method.rs",
                "fn f(v: &Vec<u8>) -> usize { Vec::capacity(v) + Vec::<u8>::capacity(v) }\n",
            ),
            (
                "src/engine/binding.rs",
                "struct Run { capacity: Box<dyn CapacityClient> }\n\
                 fn f(capacity: Box<dyn CapacityClient>) -> Run { Run { capacity } }\n",
            ),
            (
                "src/engine/tested.rs",
                "fn f() {}\n#[cfg(test)]\nmod tests {\n    use crate::capacity::pool_for;\n}\n",
            ),
            (
                "src/engine/coordinator.rs",
                "fn f() { let _ = crate::capacity::observe; }\n",
            ),
            ("src/cli.rs", "use crate::capacity;\n"),
        ]
        .into_iter()
        .map(|(path, source)| (path.to_owned(), source.to_owned()))
        .collect();
        assert_eq!(
            bypasses_the_seam(&planted),
            vec![
                "src/engine/planted.rs",
                "src/engine/aliased.rs",
                "src/engine/bare.rs",
                "src/engine/grouped.rs",
            ]
        );
    }
}
