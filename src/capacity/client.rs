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
    /// names the capacity module other than through `capacity::client`.
    ///
    /// A word-bounded `capacity` that does not follow a `.` is a segment of
    /// the module's path when `::` follows it (after optional whitespace), or
    /// when a `use` declaration names it (`use crate::capacity;`, `use
    /// crate::{capacity as c};`). It goes through the seam only when that
    /// `::` is followed by a word-bounded `client`. What comes before it is
    /// not read: `crate::capacity`, `super::capacity` and `Root::capacity`
    /// after `use crate as Root;` all name the module, whatever the case of
    /// their owner. A method or field after a `.` (`v.capacity()`), a type's
    /// associated function (`Vec::capacity(v)`, `Vec::<u8>::capacity(v)`,
    /// followed by `(`) and a binding or field called `capacity` (`capacity:
    /// Box<dyn CapacityClient>`) are neither followed by `::` nor named by a
    /// `use`, so none is a segment.
    ///
    /// A text census reads what is written, not what a macro expands to: a
    /// macro that builds the path (`reach!(capacity)` over a body that writes
    /// `crate::$module::pool_for`) names the module where `capacity` is not
    /// followed by `::`, and passes unseen.
    fn reaches_capacity_around_the_seam(code: &str) -> bool {
        const NAME: &str = "capacity";
        code.match_indices(NAME).any(|(at, _)| {
            let before = &code[..at];
            let after = &code[at + NAME.len()..];
            if !bounded(before.chars().next_back()) || !bounded(after.chars().next()) {
                return false;
            }
            if before.trim_end().ends_with('.') {
                return false;
            }
            let joined = after.trim_start().strip_prefix("::");
            if joined.is_none() && !in_a_use_declaration(code, at) {
                return false;
            }
            let through_the_client = joined
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

    /// Each `use` declaration in `code`, from its word-bounded `use` to the
    /// `;` that ends it.
    fn use_declarations(code: &str) -> impl Iterator<Item = &str> {
        word_occurrences(code, "use").into_iter().map(move |at| {
            let rest = &code[at..];
            rest.find(';').map_or(rest, |end| &rest[..end])
        })
    }

    const BUILTIN_CLIENT: &str = "BuiltinClient";

    /// The engine's production files, but the legacy writer's, that name
    /// `BuiltinClient` outside a `use` declaration, each with how many times
    /// it does: every such name is a site that can build a built-in client
    /// (`BuiltinClient::replay(..)`, `<BuiltinClient as Default>::default()`,
    /// `type B = BuiltinClient;`).
    fn builtin_client_sites(sources: &[(String, String)]) -> Vec<(&str, usize)> {
        engine_production_code(sources)
            .map(|(path, code)| {
                let sites = word_occurrences(&code, BUILTIN_CLIENT)
                    .into_iter()
                    .filter(|at| !in_a_use_declaration(&code, *at))
                    .count();
                (path, sites)
            })
            .filter(|(_, sites)| *sites > 0)
            .collect()
    }

    /// Whether a `use` declaration renames `BuiltinClient` (`BuiltinClient
    /// as B`), so the file can build one under a name the sites do not count.
    fn renames_the_builtin_client(declaration: &str) -> bool {
        word_occurrences(declaration, BUILTIN_CLIENT)
            .into_iter()
            .any(|at| {
                declaration[at + BUILTIN_CLIENT.len()..]
                    .trim_start()
                    .strip_prefix("as")
                    .is_some_and(|rest| bounded(rest.chars().next()))
            })
    }

    /// Whether a `use` declaration, its whitespace dropped, globs the client
    /// module: `capacity::client::*`, or a `*` among the items of the group
    /// that follows `capacity::client::` (`capacity::client::{Pool, *}`), so
    /// the file has `BuiltinClient` without naming it.
    fn globs_the_client_module(declaration: &str) -> bool {
        const MODULE: &str = "capacity::client::";
        let compact: String = declaration
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();
        compact.match_indices(MODULE).any(|(at, _)| {
            let imported = &compact[at + MODULE.len()..];
            let Some(group) = imported.strip_prefix('{') else {
                return imported.starts_with('*');
            };
            let mut depth = 0_usize;
            let mut previous = '{';
            for character in group.chars() {
                match character {
                    '*' if depth == 0 && matches!(previous, '{' | ',') => return true,
                    '{' => depth += 1,
                    '}' if depth == 0 => return false,
                    '}' => depth -= 1,
                    _ => {}
                }
                previous = character;
            }
            false
        })
    }

    /// The engine's production files, but the legacy writer's, with a `use`
    /// declaration that lets the file build a `BuiltinClient` without naming
    /// it: one that renames it, or globs the client module.
    ///
    /// Like the seam's census, this reads text: a macro that builds the name,
    /// or a glob through an alias of the module (`use crate::capacity::client
    /// as c; use c::*;`), is not seen.
    fn hides_the_builtin_client(sources: &[(String, String)]) -> Vec<&str> {
        engine_production_code(sources)
            .filter(|(_, code)| {
                use_declarations(code).any(|declaration| {
                    renames_the_builtin_client(declaration) || globs_the_client_module(declaration)
                })
            })
            .map(|(path, _)| path)
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
                "src/engine/rooted.rs",
                "use crate as Root;\nuse Root::capacity::drain_of;\n",
            ),
            (
                "src/engine/upper.rs",
                "fn f() { let _ = Root::capacity::pool_for; }\n",
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
                "src/engine/rooted.rs",
                "src/engine/upper.rs",
            ]
        );
    }

    #[test]
    fn only_the_run_constructor_builds_the_builtin_client() {
        const RUN: &str = "src/engine/topology/run.rs";
        let sources = production_sources();
        assert!(
            crate::effects::production_code(source_of(&sources, RUN))
                .contains("BuiltinClient::replay(&handle.events)"),
            "`TopologyRun::resumed` in `{RUN}` no longer builds the run's client with \
             `BuiltinClient::replay(&handle.events)`, so the one site this census allows is not \
             the run's constructor"
        );
        assert_eq!(
            builtin_client_sites(&sources),
            vec![(RUN, 1)],
            "the run asks the client it installed (`self.capacity`, which `TopologyRun::resumed` \
             builds), so a second built-in client, built anywhere else in the engine, answers \
             capacity questions an installed handler never sees. The legacy writer of schemas \
             1–3 ({LEGACY_WRITER:?}) is the one named exception"
        );
        assert_eq!(
            hides_the_builtin_client(&sources),
            Vec::<&str>::new(),
            "a `use` that renames `BuiltinClient` or globs `capacity::client` lets a file build a \
             second built-in client without naming it, and that client answers capacity \
             questions an installed handler never sees"
        );
    }

    #[test]
    fn the_builtin_client_census_sees_a_second_client() {
        let planted: Vec<(String, String)> = [
            (
                "src/engine/topology/run.rs",
                "use crate::capacity::client::{BuiltinClient, CapacityClient};\n\
                 fn resumed(handle: H) -> Box<dyn CapacityClient> {\n    \
                 Box::new(BuiltinClient::replay(&handle.events))\n}\n",
            ),
            (
                "src/engine/second.rs",
                "fn f(e: &[E]) { BuiltinClient::replay(e).grant(&[], e, 0); }\n",
            ),
            (
                "src/engine/renamed.rs",
                "use crate::capacity::client::BuiltinClient as B;\n",
            ),
            ("src/engine/globbed.rs", "use crate::capacity::client::*;\n"),
            (
                "src/engine/grouped.rs",
                "use crate::capacity::client::{\n    CapacityClient, *,\n};\n",
            ),
            (
                "src/engine/qualified.rs",
                "fn f() { let _ = <BuiltinClient as Default>::default(); }\n",
            ),
            (
                "src/engine/prose.rs",
                "// a BuiltinClient answers from the run's signals\n\
                 const S: &str = \"BuiltinClient::replay(e)\";\n",
            ),
            (
                "src/engine/tested.rs",
                "fn f() {}\n#[cfg(test)]\nmod tests {\n    \
                 use crate::capacity::client::BuiltinClient as B;\n    \
                 fn g(e: &[E]) { B::replay(e); BuiltinClient::replay(e); }\n}\n",
            ),
            (
                "src/engine/coordinator.rs",
                "fn f(e: &[E]) { BuiltinClient::replay(e); }\n",
            ),
            (
                "src/cli.rs",
                "use crate::capacity::client::*;\nfn f(e: &[E]) { BuiltinClient::replay(e); }\n",
            ),
        ]
        .into_iter()
        .map(|(path, source)| (path.to_owned(), source.to_owned()))
        .collect();
        assert_eq!(
            builtin_client_sites(&planted),
            vec![
                ("src/engine/topology/run.rs", 1),
                ("src/engine/second.rs", 1),
                ("src/engine/qualified.rs", 1),
            ]
        );
        assert_eq!(
            hides_the_builtin_client(&planted),
            vec![
                "src/engine/renamed.rs",
                "src/engine/globbed.rs",
                "src/engine/grouped.rs",
            ]
        );
    }
}
