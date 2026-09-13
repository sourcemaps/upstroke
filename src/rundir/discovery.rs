//! Run discovery: the readers `startup_census` names, and the husk report.
//!
//! `startup_census`: "every reader (`list_runs`, `latest_run`,
//! `resolve_run_id`, `find_question`, `status`) returns Committed directories
//! only, **whether or not a marker is present**". Every function here is one of
//! those readers or the census surface `upstroke status` renders, and every one
//! of them is read-only: the census *decides* here and *acts* in the parent,
//! where the deletion sites are.

// **This child states its own lint level and inherits nothing.** A Rust lint
// level is scoped by the module tree and not by the file, so an out-of-line
// child of `src/rundir.rs` would otherwise inherit that file's inner
// `#![allow(clippy::disallowed_methods, disallowed_types, disallowed_macros)]`
// -- `PR6-LANEF-004`, measured twice in the Container subtree and made again,
// independently, by two W1 pull requests. Nothing here reaches a governed
// primitive, so all three are DENIED rather than allowed, and this module takes
// no `effects/allowlist.toml` row: an allowance is what that file records, and
// this module takes none.
//
// **Measured, not believed.** A probe of three lines -- a `std::fs::write`, a
// `std::process::Command` and a `println!` -- is refused three times here, once
// per lint, with this attribute cited as the level; the identical three lines in
// `src/rundir.rs` emit no `disallowed_*` at all, under that file's own allow. So
// the deny is load-bearing rather than a restatement of an ambient rule.
#![deny(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros
)]

use std::path::{Path, PathBuf};

use crate::error::UpstrokeError;

use super::{
    CreatingMarker, EVENT_LOG, MARKER, PrivateHalfOwnership, RepoKey, RetainReason, RunDirClass,
    UnboundShape, classify_run_dir, fs, prove_private_half_ownership, public_dir, runs_root,
};

/// Every run in this repo, oldest first.
///
/// Run ids are ULIDs with the millisecond timestamp in the high bits and
/// Crockford base32's digits-before-letters ordering, so a plain lexicographic
/// sort is chronological — no directory timestamps, which copying a repo would
/// scramble.
///
/// **Committed directories only.** `startup_census`: "every reader
/// (`list_runs`, `latest_run`, `resolve_run_id`, `find_question`, `status`)
/// returns Committed directories only, **whether or not a marker is present**",
/// and `run_creation` says it from the other side: "readers never return a
/// directory without a committed `run_started` and never hide one because of a
/// marker". Both halves are load-bearing and each is a separate test.
///
/// This is the slice's only change in behaviour: a legacy husk that today
/// shadows [`latest_run`] is no longer listed. A run whose log committed is
/// listed exactly as before, marker or no marker.
///
/// **A directory the probe could not classify is not listed either**
/// ([`RunDirClass::Indeterminate`], `SWEEP-CLASSIFY-001`): this returns
/// `Committed` directories and that is not one. It is not returned by
/// [`list_husks`] either, so nothing offers it for reclaim — which is the point
/// of the class. [`resolve_run_id`] says which of the two it met.
///
/// Ordering comes from [`run_dir_names`], which sorts before anything filters,
/// so the explicit re-sort this used to end with was a second statement of the
/// same fact.
pub fn list_runs(repo_root: &Path) -> Vec<String> {
    names_classified(&census(repo_root), listed_as_run)
}

/// Every run directory with the classification **one** probe gave it, oldest
/// first.
///
/// **One observation per directory, and every answer a caller derives comes out
/// of it.** [`resolve_run_id`] asks three questions of the same directories —
/// is this a run, is it a husk, did it fail to classify — and asking them
/// through three separate calls to [`classify_run_dir`] made them three
/// separate observations of a source that need not answer the same way twice.
/// Measured on the round that introduced [`RunDirClass::Indeterminate`]: one
/// committed run, a source that answered `Interrupted` for the whole of the
/// first probe's allowance and then read normally, and resolution answered
/// **"no runs found"** — the first pass classified `Indeterminate` so the run
/// was not listed, the second and third passes read the log and classified
/// `Committed` so it was neither a husk nor unclassifiable, and the reason the
/// reader had refused was gone. `231c1aad`, which had no third class and one
/// pass, resolved it.
///
/// So the census is taken once and passed around. A classification that changes
/// between observations is then a fact about *later* commands, which is what it
/// is, rather than three disagreeing answers inside one.
///
/// **This body holds the only call to [`classify_run_dir`] in this module**,
/// and `this_module_classifies_a_run_directory_in_exactly_one_place` is the
/// assertion that it stays the only one: a second call site is a second
/// observation, and the three that were here are what lost the answer.
fn census(repo_root: &Path) -> Vec<(String, RunDirClass)> {
    run_dir_names(repo_root)
        .into_iter()
        .map(|run_id| {
            let class = classify_run_dir(&public_dir(repo_root, &run_id));
            (run_id, class)
        })
        .collect()
}

/// The ids in a census whose classification `wanted` claims, in census order.
///
/// The clone is stated rather than hidden (§6): the census owns the names and
/// every reader here hands back owned ids — [`list_runs`] and [`list_husks`]
/// return `Vec<String>` and the refusal sentences format them — so a borrowed
/// view would be owned again at each of the four call sites. What it costs is
/// one 26-character ULID per directory a predicate claims.
fn names_classified(
    census: &[(String, RunDirClass)],
    wanted: fn(RunDirClass) -> bool,
) -> Vec<String> {
    census
        .iter()
        .filter(|(_, class)| wanted(*class))
        .map(|(run_id, _)| run_id.clone())
        .collect()
}

/// Which classifications [`list_runs`] returns.
///
/// A named predicate rather than an inline `==`, and the reason is that the
/// fact it states cannot be measured over a directory: no fixture in this
/// suite classifies [`RunDirClass::Indeterminate`], because nothing here
/// arranges the signal that interrupts a read of an ordinary file, so the only
/// way to assert that this reader does not return one is to ask the predicate.
/// See
/// `a_directory_that_did_not_classify_is_neither_a_run_nor_a_husk`, which
/// crosses all three of these against all three classifications.
///
/// **An exhaustive `match` and not `matches!`, which is why it is three lines
/// rather than one.** `matches!(class, RunDirClass::Committed)` compiles
/// unchanged when a classification is added and silently answers `false` for
/// it, so the arm a fourth class needs would be one nobody is made to write.
/// Written this way the compiler names every one of these three predicates,
/// and `scan_classified` in `engine::topology::startup` beside them, before a
/// tree carrying a fourth classification builds at all. The answer that would
/// have been given silently is even the *safe* one here — not listed, not
/// offered for reclaim — and that is the objection to it: a reader added later
/// gets its default decided for it by a macro rather than by whoever adds the
/// class.
const fn listed_as_run(class: RunDirClass) -> bool {
    match class {
        RunDirClass::Committed => true,
        RunDirClass::Husk => false,
        RunDirClass::Indeterminate => false,
    }
}

/// Which classifications [`list_husks`] returns. See [`listed_as_run`], whose
/// note covers all three of these.
const fn listed_as_husk(class: RunDirClass) -> bool {
    match class {
        RunDirClass::Committed => false,
        RunDirClass::Husk => true,
        RunDirClass::Indeterminate => false,
    }
}

/// Which classifications [`no_match`] reports as unreadable rather than as an
/// absence. See [`listed_as_run`].
const fn unclassified(class: RunDirClass) -> bool {
    match class {
        RunDirClass::Committed => false,
        RunDirClass::Husk => false,
        RunDirClass::Indeterminate => true,
    }
}

/// Every directory under `<repo>/.upstroke/runs`, committed or not, oldest first.
///
/// Not a reader in `startup_census`'s sense and deliberately not filtered by
/// commitment: this is the enumeration a census walks and the one the worktree
/// lease's R28 check scans. A crashed run whose log never committed is exactly
/// the run whose reaper is most likely still holding its cleanup lease, so
/// filtering here would hide the hold that check exists to observe.
#[must_use]
pub fn run_dir_names(repo_root: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(runs_root(repo_root)) else {
        return Vec::new();
    };
    let mut runs: Vec<String> = entries
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    runs.sort();
    runs
}

/// Every husk under `<repo>/.upstroke/runs`, oldest first.
///
/// `Husk` and nothing else: a directory whose classification did not finish is
/// [`RunDirClass::Indeterminate`] and is absent from this list, so the two
/// readers of it — `status`'s husk answer and [`resolve_run_id`]'s refusal
/// message — cannot call it a husk, and neither can a caller added later that
/// reclaims from it. Those are the two readers in this repository at this SHA,
/// by `grep -rn 'list_husks' src/`.
#[must_use]
pub fn list_husks(repo_root: &Path) -> Vec<String> {
    names_classified(&census(repo_root), listed_as_husk)
}

/// What `status` says about a husk id it was asked for by name.
///
/// `startup_census`: "status is read-only: it ignores husks and, asked
/// explicitly for a husk id, reports an unstarted husk that the next write
/// command reclaims, a retained husk with its reason and locator, or a possibly
/// committed run whose public log has no valid committed first line".
#[derive(Debug)]
pub struct HuskReport {
    pub run_id: String,
    pub public: PathBuf,
    /// The private locator the marker records, when a marker parses.
    pub locator: Option<PathBuf>,
    pub disposition: HuskDisposition,
}

/// What the next write command's census would do with a husk it may reclaim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reclaimable {
    /// Nothing private is bound, so the public half alone is reclaimed.
    PublicOnly(UnboundShape),
    /// The ownership proof holds and no commit record exists: the private half
    /// is reclaimed through the proof-token funnel, then the public directory
    /// with the marker last.
    BothHalves,
}

/// The trichotomy `status` reports a husk id by.
#[derive(Debug)]
pub enum HuskDisposition {
    /// Nothing has started here: the next write command reclaims it.
    Unstarted(Reclaimable),
    /// Retained and reported until the deferred prune command removes it.
    /// [`RetainReason::PossiblyCommitted`] is the third of the three sentences.
    Retained(RetainReason),
}

impl HuskDisposition {
    /// The operator-facing sentence, which names which of the three this is.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::Unstarted(Reclaimable::BothHalves) => "an unstarted husk, bound to a private \
                 half that never committed, that the next write command reclaims"
                .to_owned(),
            Self::Unstarted(Reclaimable::PublicOnly(shape)) => format!(
                "an unstarted husk ({}) that the next write command reclaims",
                match shape {
                    UnboundShape::Bare => "a bare directory",
                    UnboundShape::StagedMarkerOnly => "only a staged marker",
                    UnboundShape::TargetAbsent => "its recorded private half is gone",
                }
            ),
            Self::Retained(RetainReason::PossiblyCommitted) => {
                "a possibly committed run whose public log has no valid committed first line; \
                 nothing is deleted"
                    .to_owned()
            }
            Self::Retained(reason) => format!("a retained husk: {reason}"),
        }
    }
}

/// Report a husk by id, for `status` and for the census report.
///
/// Read-only from end to end. The authorized private root is the one the
/// command is configured with, which for a read-only `status` is the default.
#[must_use]
pub fn husk_report(
    repo_root: &Path,
    run_id: &str,
    repo_key: &RepoKey,
    authorized_root: &Path,
) -> HuskReport {
    let public = public_dir(repo_root, run_id);
    let locator = fs::read_to_string(public.join(MARKER))
        .ok()
        .and_then(|text| serde_json::from_str::<CreatingMarker>(&text).ok())
        .map(|marker| PathBuf::from(marker.private_dir));
    let disposition = match prove_private_half_ownership(&public, repo_key, authorized_root) {
        // A token means the husk is provably this run's and never committed —
        // reclaimable, both halves, by the next write command. The token is
        // dropped unspent: `status` is read-only.
        PrivateHalfOwnership::Proven(_) => HuskDisposition::Unstarted(Reclaimable::BothHalves),
        PrivateHalfOwnership::NothingBound(shape) => {
            HuskDisposition::Unstarted(Reclaimable::PublicOnly(shape))
        }
        PrivateHalfOwnership::Retained(reason) => HuskDisposition::Retained(reason),
    };
    HuskReport {
        run_id: run_id.to_owned(),
        public,
        locator,
        disposition,
    }
}

/// The most recent run — what `upstroke status` reports when given no id.
pub fn latest_run(repo_root: &Path) -> Option<String> {
    list_runs(repo_root).pop()
}

/// Resolve a run id from any unambiguous prefix, so an operator can type the
/// first few characters of a 26-character ULID.
///
/// An exact match wins outright rather than being treated as one candidate
/// among several: a full id is never ambiguous, even if some other run happens
/// to extend it.
pub fn resolve_run_id(repo_root: &Path, wanted: &str) -> Result<String, UpstrokeError> {
    resolve_in_census(&census(repo_root), &runs_root(repo_root), wanted)
}

/// [`resolve_run_id`] over a census already taken.
///
/// **The census is taken once, and the three questions below are asked of that
/// one census rather than of three fresh ones.** This is the whole of
/// `SWEEP-CLASSIFY-001`'s repair on the resolution path, and without it the
/// repair loses here what it wins in the classifier: a first pass that answered
/// [`RunDirClass::Indeterminate`] dropped the directory from `runs`, and a
/// second and third pass that then read the log successfully found it neither a
/// husk nor unclassifiable, so the refusal fell through to "no runs found" and
/// the incomplete observation — the reason the reader refused — was gone.
/// [`census`] carries the measurement.
///
/// **This signature is the guard, not a convenience.** It is handed a census
/// and given no repository root, so the reader that decides cannot observe a
/// directory again — not through [`census`], not through [`list_husks`], and
/// not through anything a later change writes into its body without first
/// widening a parameter list that exists to refuse it. `runs_root` is passed
/// for the same reason rather than derived: it is a `Path` this function
/// prints in one sentence and cannot open a run tree with.
///
/// **Split from [`resolve_run_id`] so the third classification is also a value
/// a test can supply**, which is the split [`class_of`] takes one module over
/// and for the same reason: no `events.jsonl` a fixture here can write
/// classifies `Indeterminate`. See
/// `an_unfinished_observation_survives_every_question_resolution_asks`, and
/// `this_module_classifies_a_run_directory_in_exactly_one_place` for the half
/// a signature cannot state — that the census reaching it came from one site.
fn resolve_in_census(
    census: &[(String, RunDirClass)],
    runs_root: &Path,
    wanted: &str,
) -> Result<String, UpstrokeError> {
    let runs = names_classified(census, listed_as_run);
    let wanted_upper = wanted.to_ascii_uppercase();
    // The entry as it exists on disk, not the uppercased input. The comparison
    // is case-insensitive because a run directory can arrive from a
    // case-insensitive filesystem, and on a case-sensitive one only the real
    // name builds a path that opens — everything downstream joins this id.
    if let Some(matched) = runs.iter().find(|id| id.eq_ignore_ascii_case(wanted)) {
        return Ok(matched.clone());
    }
    let matches: Vec<&String> = runs
        .iter()
        .filter(|id| id.to_ascii_uppercase().starts_with(&wanted_upper))
        .collect();
    match matches.as_slice() {
        [only] => Ok((*only).clone()),
        [] => Err(UpstrokeError::Refused {
            message: no_match(census, runs_root, &runs, wanted),
        }),
        several => Err(UpstrokeError::Refused {
            message: format!(
                "that prefix matches {} runs ({}); use more characters",
                several.len(),
                several
                    .iter()
                    .map(|id| id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }),
    }
}

/// What [`resolve_in_census`] refuses with when no committed run matched.
///
/// Four sentences, and which one is given is decided **entirely from the census
/// it is handed**. Nothing here classifies anything: a second observation at
/// this point is what lost `SWEEP-CLASSIFY-001`'s answer on this path, since
/// the directory whose read did not finish is exactly the directory whose next
/// read may finish, and the refusal would then describe a directory in terms
/// the reader never established.
fn no_match(
    census: &[(String, RunDirClass)],
    runs_root: &Path,
    runs: &[String],
    wanted: &str,
) -> String {
    let unread = names_classified(census, unclassified);
    match matching(&names_classified(census, listed_as_husk), wanted) {
        // A directory is there, and it holds no committed `run_started`.
        // Saying "no run matches that id" of a directory the operator
        // can see is the answer that sends them looking for a bug.
        Some(husk) => format!(
            "`{husk}` never recorded a committed run_started, so there is no run to open there \
             — ask `upstroke status {husk}` for what it is and what happens to it{}",
            unread_note(&unread)
        ),
        // A directory is there and the probe could not read it. The
        // husk sentence above would be a *claim about its contents*
        // that nothing established, and it is the sentence `status`
        // prints too, so an operator would be told a run never started
        // when what happened is that the log could not be read.
        // `upstroke status` is not offered here because it resolves
        // through this same function and would print this same line.
        None => match matching(&unread, wanted) {
            // The one sentence that already names a directory whose observation
            // did not finish, so it is the one sentence the note below would
            // repeat.
            Some(named) => format!(
                "`{named}` could not be classified: its {EVENT_LOG} could not be read to a \
                 first line, so whether a run committed there is unknown — nothing has been \
                 deleted and the next census retains it"
            ),
            // The wanted id named none of them, and the two sentences left are
            // claims about the *set*: "there are no runs here" and "none of the
            // runs here is yours". Neither is true of a census that could not
            // read every directory it walked, so an unfinished observation is
            // carried into both rather than being dropped because the operator
            // happened to type a prefix that missed it.
            None if runs.is_empty() => format!(
                "no runs found under {}{}",
                runs_root.display(),
                unread_note(&unread)
            ),
            None => format!(
                "no run matches that id; known runs: {}{}",
                runs.join(", "),
                unread_note(&unread)
            ),
        },
    }
}

/// What a refusal adds when some directory the census walked did not classify.
///
/// Empty when every directory classified, so the three sentences it is appended
/// to are unchanged for every census that finished — which is every census
/// `run_dir_names` walks over ordinary regular files.
///
/// **The husk sentence carries it too, and that is not a spare.** It is a claim
/// about one directory's *contents* — "never recorded a committed
/// run_started" — reached by prefix from a set the operator did not see, and a
/// census holding both a husk and a directory it could not read answers it
/// while knowing less than it says. The one sentence this is not appended to is
/// the one that already names an unclassified directory.
fn unread_note(unread: &[String]) -> String {
    match unread {
        [] => String::new(),
        [only] => format!(
            "; and `{only}` could not be classified, so whether a run committed there is \
             unknown — nothing has been deleted and the next census retains it"
        ),
        several => format!(
            "; and {} directories could not be classified ({}), so whether a run committed in \
             them is unknown — nothing has been deleted and the next census retains them",
            several.len(),
            several.join(", ")
        ),
    }
}

/// The one id in `names` a wanted id names, exactly or by unambiguous prefix.
///
/// Only used to explain a refusal, so an ambiguous prefix answers `None`: the
/// operator is told to use more characters by [`resolve_id_within`]'s own
/// branch, not sent to one of the matches. One function rather than one per
/// set, so the husk sentence and the unclassifiable sentence cannot drift from
/// each other on what "unambiguous" means.
fn matching(names: &[String], wanted: &str) -> Option<String> {
    let wanted_upper = wanted.to_ascii_uppercase();
    if let Some(exact) = names.iter().find(|id| id.eq_ignore_ascii_case(wanted)) {
        return Some(exact.clone());
    }
    let mut prefixed = names
        .iter()
        .filter(|id| id.to_ascii_uppercase().starts_with(&wanted_upper));
    let first = prefixed.next()?;
    prefixed.next().is_none().then(|| first.clone())
}

/// A question id resolved to the run that raised it.
#[derive(Debug)]
pub struct FoundQuestion {
    pub run_id: String,
    /// The run's public directory — everything `upstroke answer` touches.
    pub public: PathBuf,
    /// The full question id, expanded from whatever prefix was typed.
    pub question_id: String,
}

/// Find the run holding a question, by full id or unambiguous prefix.
///
/// Scans every run rather than requiring the operator to remember which one
/// asked: the notifier hands them a question id, not a run id, so a question
/// id is what the command has to accept.
pub fn find_question(repo_root: &Path, wanted: &str) -> Result<FoundQuestion, UpstrokeError> {
    let wanted_upper = wanted.to_ascii_uppercase();
    let mut exact: Option<FoundQuestion> = None;
    let mut matches: Vec<FoundQuestion> = Vec::new();
    for run_id in list_runs(repo_root) {
        let public = public_dir(repo_root, &run_id);
        let Ok(entries) = fs::read_dir(public.join("questions")) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some(question_id) = name.strip_suffix(".json") else {
                continue;
            };
            let found = FoundQuestion {
                run_id: run_id.clone(),
                public: public.clone(),
                question_id: question_id.to_owned(),
            };
            if question_id.eq_ignore_ascii_case(wanted) {
                exact = Some(found);
            } else if question_id.to_ascii_uppercase().starts_with(&wanted_upper) {
                matches.push(found);
            }
        }
    }
    if let Some(found) = exact {
        return Ok(found);
    }
    match matches.len() {
        1 => matches.pop().ok_or_else(|| UpstrokeError::Refused {
            message: "question vanished while resolving it".to_owned(),
        }),
        0 => Err(UpstrokeError::Refused {
            message: format!(
                "no question with that id under {}",
                runs_root(repo_root).display()
            ),
        }),
        several => Err(UpstrokeError::Refused {
            message: format!(
                "that prefix matches {several} questions ({}); use more characters",
                matches
                    .iter()
                    .map(|found| found.question_id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{RunDirClass, listed_as_husk, listed_as_run, resolve_in_census, unclassified};

    /// The three classifications, crossed with the three readers that filter on
    /// them.
    ///
    /// **This table is the only driver these readers have for the third
    /// class.** No directory a test here can build classifies
    /// `Indeterminate`: that class comes from a read a signal interrupted, and
    /// nothing in this suite arranges a signal — a fixture's `events.jsonl` is
    /// an ordinary regular file whose reads deliver bytes or end. So
    /// `list_runs` and `list_husks` are measured over the classification rather
    /// than over a fixture, through the predicates they filter by — which is
    /// why those predicates are named functions.
    ///
    /// The property is the finding's: an observation that could not be
    /// completed is returned by **neither** reader. Not as a run, which would
    /// resume from a log nothing managed to read; and not as a husk, which is
    /// what `husk_report` and the census's reclaim arm are fed from, and
    /// therefore the answer that can end in a deletion.
    #[test]
    fn a_directory_that_did_not_classify_is_neither_a_run_nor_a_husk() {
        let mut seen = Vec::new();
        for class in [
            RunDirClass::Committed,
            RunDirClass::Husk,
            RunDirClass::Indeterminate,
        ] {
            // Exhaustive on purpose: a fourth classification stops compiling
            // here rather than passing this test without a row.
            let name = match class {
                RunDirClass::Committed => "committed",
                RunDirClass::Husk => "husk",
                RunDirClass::Indeterminate => "indeterminate",
            };
            let answers = [
                listed_as_run(class),
                listed_as_husk(class),
                unclassified(class),
            ];
            assert_eq!(
                answers.iter().filter(|answered| **answered).count(),
                1,
                "{name}: exactly one of the three readers claims a classification"
            );
            seen.push((name, answers));
        }
        assert_eq!(
            seen,
            vec![
                ("committed", [true, false, false]),
                ("husk", [false, true, false]),
                ("indeterminate", [false, false, true]),
            ],
            "SWEEP-CLASSIFY-001: an unfinished observation is neither listed as a run nor \
             offered as a husk"
        );
    }

    /// The runs root every refusal below is told about. Never opened: these
    /// drive [`resolve_in_census`], which reads nothing.
    fn runs_root() -> &'static Path {
        Path::new("/nowhere/.upstroke/runs")
    }

    /// A census, spelled the way the readers take one.
    fn census(entries: &[(&str, RunDirClass)]) -> Vec<(String, RunDirClass)> {
        entries
            .iter()
            .map(|(run_id, class)| ((*run_id).to_owned(), *class))
            .collect()
    }

    /// `SWEEP-CLASSIFY-001` on the resolution path: an observation that could
    /// not be completed reaches the operator, whatever they typed.
    ///
    /// **This is the driver the round-1 predicate table could not be.** That
    /// table crosses a classification against the three predicates that filter
    /// on it and is right about each one; what it cannot see is the reader
    /// *above* them, which asks three questions and has to answer from all
    /// three. The measured failure was there and not in the predicates: the run
    /// was correctly not listed, correctly not a husk and correctly not
    /// unclassifiable — each answer from a different observation — and the
    /// refusal that came out was "no runs found".
    ///
    /// So this drives the reader over a whole census rather than a
    /// classification over a predicate, and it asserts what the operator is
    /// told rather than which branch ran. Three shapes, because the id typed
    /// decides which sentence is reached and only one of them was ever the
    /// obvious case:
    ///
    /// 1. the typed id names the directory that did not classify;
    /// 2. it names nothing, and there is no committed run to fall back on —
    ///    the "no runs found" sentence, which is where the answer was lost;
    /// 3. it names nothing and there *are* committed runs — the "known runs"
    ///    sentence, which is a claim about the same incomplete set;
    /// 4. it names a **husk** — a sentence about one directory's contents,
    ///    reached from a set the operator did not see, answered by a census
    ///    that could not read all of it.
    #[test]
    fn an_unfinished_observation_survives_every_question_resolution_asks() {
        let unread = census(&[("01UNREAD", RunDirClass::Indeterminate)]);

        let named = resolve_in_census(&unread, runs_root(), "01UNREAD")
            .expect_err("a directory that did not classify is not a run")
            .to_string();
        assert!(
            named.contains("`01UNREAD` could not be classified"),
            "the id typed names it: {named}"
        );
        assert!(
            named.contains("nothing has been deleted"),
            "and says the directory is kept: {named}"
        );
        assert!(
            !named.contains("never recorded a committed run_started"),
            "and never the husk sentence, which is a claim about contents nothing read: {named}"
        );

        let missed = resolve_in_census(&unread, runs_root(), "01SOMETHINGELSE")
            .expect_err("no run matches")
            .to_string();
        assert!(
            missed.contains("01UNREAD") && missed.contains("could not be classified"),
            "a census that could not read every directory it walked cannot report an empty \
             runs root as a fact: {missed}"
        );

        let alongside = census(&[
            ("01COMMITTED", RunDirClass::Committed),
            ("01UNREAD", RunDirClass::Indeterminate),
        ]);
        let known = resolve_in_census(&alongside, runs_root(), "01SOMETHINGELSE")
            .expect_err("no run matches")
            .to_string();
        assert!(
            known.contains("known runs: 01COMMITTED"),
            "the runs it did read are still listed: {known}"
        );
        assert!(
            known.contains("01UNREAD") && known.contains("could not be classified"),
            "and the one it could not read is still reported: {known}"
        );

        let with_husk = census(&[
            ("01HUSK", RunDirClass::Husk),
            ("01UNREAD", RunDirClass::Indeterminate),
        ]);
        let husk = resolve_in_census(&with_husk, runs_root(), "01HUSK")
            .expect_err("a husk is not a run")
            .to_string();
        assert!(
            husk.contains("never recorded a committed run_started"),
            "the husk sentence is still the husk sentence: {husk}"
        );
        assert!(
            husk.contains("01UNREAD") && husk.contains("could not be classified"),
            "and a census that could not read every directory it walked says so even when \
             the id typed named one it could: {husk}"
        );
    }

    /// The controls for the test above: every answer that does **not** involve
    /// an unfinished observation is the answer it was before.
    ///
    /// Without these, a reader that appended the unclassifiable sentence to
    /// everything, or refused everything, would satisfy all three shapes above.
    #[test]
    fn a_census_that_finished_answers_exactly_as_it_did() {
        let runs = census(&[
            ("01AAA", RunDirClass::Committed),
            ("01BBB", RunDirClass::Committed),
            ("01HUSK", RunDirClass::Husk),
        ]);

        assert_eq!(
            resolve_in_census(&runs, runs_root(), "01aaa").expect("an exact id, case-folded"),
            "01AAA"
        );
        assert_eq!(
            resolve_in_census(&runs, runs_root(), "01A").expect("an unambiguous prefix"),
            "01AAA"
        );
        let ambiguous = resolve_in_census(&runs, runs_root(), "01")
            .expect_err("two runs share that prefix")
            .to_string();
        assert!(
            ambiguous.contains("use more characters"),
            "an ambiguous prefix is still ambiguous: {ambiguous}"
        );

        let husk = resolve_in_census(&runs, runs_root(), "01HUSK")
            .expect_err("a husk is not a run")
            .to_string();
        assert!(
            husk.contains("never recorded a committed run_started"),
            "the husk sentence is unchanged: {husk}"
        );
        assert!(
            !husk.contains("could not be classified"),
            "a husk is a completed observation, not an unfinished one: {husk}"
        );

        let none = resolve_in_census(&[], runs_root(), "01A")
            .expect_err("an empty runs root holds no run")
            .to_string();
        assert_eq!(
            none,
            format!("no runs found under {}", runs_root().display()),
            "a census that finished and found nothing says so, with nothing appended"
        );

        let missed = resolve_in_census(&runs, runs_root(), "01Z")
            .expect_err("no run matches")
            .to_string();
        assert_eq!(
            missed, "no run matches that id; known runs: 01AAA, 01BBB",
            "and nothing is appended to this one either"
        );
    }

    /// This module classifies a run directory in **one** place, and every
    /// reader in it derives from that one.
    ///
    /// **Two things guard the property the defect broke, and this is the
    /// second of them.** The first is a signature:
    /// [`resolve_in_census`] is handed a census and is given no path to the run
    /// tree, so the reader that decides *cannot* observe a directory again,
    /// whatever a later change does to its body. That is what makes
    /// `an_unfinished_observation_survives_every_question_resolution_asks` a
    /// statement about resolution and not only about a value: the three
    /// questions it asks are asked of one census because there is nothing else
    /// for them to be asked of.
    ///
    /// This is the other half: the census that reaches it comes from the only
    /// place in the module that classifies anything. `classify_run_dir` is
    /// named twice in the production region — once where it is imported and
    /// once where `census` calls it — and a third mention is a second
    /// observation site, which is exactly the shape the defect had:
    /// `list_husks` and an `unclassified_matching` beside it, each classifying
    /// the same directories over again.
    ///
    /// **What it does not catch, stated rather than left to be found.** Calling
    /// the one site *twice* — a `census(repo_root)` or a `list_husks(repo_root)`
    /// inside the refusal path — adds no mention and passes here. What stops
    /// that is the signature above: `no_match` has the runs root for one
    /// sentence and no repository root to hand either function, so the mistake
    /// does not typecheck without first widening a signature that exists to
    /// refuse it.
    ///
    /// Comments and string literals are blanked and the test region is cut off
    /// first, both through `crate::effects`' own derivations, which is what the
    /// effect census uses.
    #[test]
    fn this_module_classifies_a_run_directory_in_exactly_one_place() {
        let source = include_str!("discovery.rs");
        let code =
            crate::effects::blank_comments_and_strings(&crate::effects::production_region(source));

        // The positive control: a count of two is only evidence if this search
        // can see the module's code at all. The three readers a census feeds
        // are named, so a region that had been cut short or blanked away is a
        // failure here rather than a zero below.
        for present in [
            "fn census(repo_root: &Path)",
            "pub fn list_runs(",
            "pub fn list_husks(",
            "fn resolve_in_census(",
        ] {
            assert!(
                code.contains(present),
                "the blanked production region does not contain `{present}`, so the count \
                 below measures nothing: {} bytes",
                code.len()
            );
        }
        assert_eq!(
            code.matches("classify_run_dir").count(),
            2,
            "SWEEP-CLASSIFY-001: every reader here derives from the one census `census` takes \
             — the two mentions are its import and its call. A third is a second observation \
             of a directory already classified, which is how resolution lost an unfinished one."
        );
    }
}
