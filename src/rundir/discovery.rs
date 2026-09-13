//! Run discovery: the readers `startup_census` names, and the husk report.
//!
//! `startup_census`: "every reader (`list_runs`, `latest_run`,
//! `resolve_run_id`, `find_question`, `status`) returns Committed directories
//! only, **whether or not a marker is present**". Every function here is one of
//! those readers or the census surface `upstroke status` renders, and every one
//! of them is read-only: the census *decides* here and *acts* in the parent,
//! where the deletion sites are.
//!
//! # What a reader here must preserve
//!
//! [`RunDirClass::Indeterminate`] is not a third kind of directory. It is the
//! **absence of an answer** about a directory that really is one of the other
//! two: the probe was interrupted past its allowance, or could not reserve the
//! memory to hold what it had read, so whether a run committed there was never
//! established (`SWEEP-CLASSIFY-001`).
//!
//! A census holding `k` of them is therefore not one picture of the run tree.
//! It is every tree the completed observation could have shown, and the rule
//! each reader below obeys is the one that follows from that:
//!
//! > **A reader answers only what every completion of its census agrees on.**
//! > Where the completions disagree it refuses, and names the directory whose
//! > observation did not finish.
//!
//! Three things follow, and together they are the whole of what resolution owes
//! an unfinished observation:
//!
//! 1. **An unknown is never filtered out ahead of a decision.** Dropping an
//!    `Indeterminate` entry from a candidate set *is* the assertion that it is
//!    not a run, which is the one thing its census did not establish. So
//!    [`resolve_in_census`] matches a typed id against the census itself and
//!    carries [`Matched::unread`] into the decision beside the runs, rather
//!    than filtering first and matching after. Measured with the filter first:
//!    a prefix naming one readable run and one directory that did not classify
//!    resolved to the readable one, and resume opens whatever resolution
//!    returns.
//! 2. **A claim about the *set* carries it.** "No runs found" and "known runs:
//!    …" are both false under the completion where the unread directory
//!    committed, so [`unread_note`] is appended to each — and to the husk
//!    sentence, which is a claim about one directory's contents reached from a
//!    set the operator never saw.
//! 3. **One observation per command.** Every answer a command gives comes out
//!    of the census [`census`] took, because a second observation can only
//!    replace the unknown with an answer *this* command did not have, and the
//!    sentence the operator then reads describes a directory in terms its
//!    decision never used. A classification that changes afterwards is a fact
//!    about the **next** command, and this module says so rather than
//!    promising what a later census will find.
//!
//! **No signature holds any of this, and two rounds of this repair claimed one
//! did.** A `&Path` hands back the repository root
//! (`runs_root.parent().and_then(Path::parent)`), `std::env::current_dir` is
//! ambient, and nothing in the language stops a body added later from opening a
//! run tree; a census of how often `classify_run_dir` is *named* cannot see a
//! second call of the one site at all. What holds it is a driver:
//! `resolution_answers_from_the_observation_it_took`, in this module's own
//! suite, runs [`resolve_observed`] over a **real** run tree with an
//! observation source that answers `Indeterminate` the first time it is asked
//! about one directory and reads the tree every time after. Any second
//! observation — through that source, or around it through the filesystem —
//! answers `Committed`, and every sentence the resolver gives changes. That is
//! the property; the rest of this module is that rule at each reader which
//! decides from a census, and [`latest_run`] is where it says it does not
//! hold.

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
///
/// **Where the classification comes from is a parameter one function over**,
/// and this is one of the two bodies that supply it. See [`census_observed`]:
/// the supply is what a test can replace, and replacing it is the only way a
/// suite on this platform can put a classification the filesystem will not
/// produce in front of a reader that has to decide from it.
fn census(repo_root: &Path) -> Vec<(String, RunDirClass)> {
    census_observed(repo_root, &mut classify_run_dir)
}

/// [`census`] with its one observation supplied — the seam the guard drives.
///
/// **Every class any reader here decides from comes through this `observe`.**
/// Production supplies [`classify_run_dir`] at the two call sites that name it,
/// and nothing else in the module classifies anything, so "the census was taken
/// once" is a statement about this parameter rather than about a call graph a
/// reader has to trace.
///
/// **Why a seam and not a fixture.** `RunDirClass::Indeterminate` is what the
/// first-line probe answers when a source interrupts it past its allowance or
/// refuses it the memory to hold the line, and no `events.jsonl` a test here
/// can write produces either: a fixture's log is an ordinary regular file whose
/// reads deliver bytes or end, and reads of one do not answer `Interrupted` on
/// any platform this crate builds for. The two reviews that measured the defect
/// injected 65,537 interruptions into a real log from outside the process. A
/// test that cannot do that can still supply the answer that read would have
/// given — and, more to the point, can supply one that **changes between
/// observations**, which is the condition the defect needs and which a static
/// fixture cannot express at all.
///
/// This is the split [`class_of`](super::classify) takes one module over and
/// for the same reason, one layer up: there it is a classification a fixture
/// cannot reach, here it is a *sequence* of them.
fn census_observed(
    repo_root: &Path,
    observe: &mut dyn FnMut(&Path) -> RunDirClass,
) -> Vec<(String, RunDirClass)> {
    run_dir_names(repo_root)
        .into_iter()
        .map(|run_id| {
            let class = observe(&public_dir(repo_root, &run_id));
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
///
/// **This is the one reader here that the module doc's rule does not hold for,
/// and saying so is this round's alternative to a fourth attempt at it.** "The
/// most recent run" is not stable across the completions of a census that did
/// not finish: with a newer directory unread and an older one committed, the
/// completion where the unread directory committed makes *it* the latest, and
/// this answers the older one. `Option<String>` has nowhere to put "the newest
/// directory did not classify", so closing it is a return type and its three
/// callers — `status`'s no-id arm, and the self-metered spend that
/// `validate` and `capacity` fold from the latest run — rather than a change
/// here.
///
/// What it costs meanwhile, stated exactly: a `status` with no id can name an
/// older run as the latest, and the two capacity readers can fold an older
/// run's events. No caller of this resumes, commits or deletes anything —
/// [`resolve_run_id`] is what resume consumes, and it is the path this round
/// makes satisfy the rule.
pub fn latest_run(repo_root: &Path) -> Option<String> {
    list_runs(repo_root).pop()
}

/// Resolve a run id from any unambiguous prefix, so an operator can type the
/// first few characters of a 26-character ULID.
///
/// An exact match wins outright rather than being treated as one candidate
/// among several: a full id is never ambiguous, even if some other run happens
/// to extend it.
///
/// **Resume opens whatever this returns**, which is why the module doc's rule
/// is written on this path first and hardest. An answer that is right under one
/// completion of the census and wrong under another is an answer that can
/// attach a resume to a different run than the operator asked about, and a
/// transient read failure is enough to choose between them.
pub fn resolve_run_id(repo_root: &Path, wanted: &str) -> Result<String, UpstrokeError> {
    resolve_observed(repo_root, wanted, &mut classify_run_dir)
}

/// [`resolve_run_id`] with its one observation supplied.
///
/// **The whole of [`resolve_run_id`] is this call**, so a test driving this
/// drives resolution itself — and the census below is the one *the resolver
/// takes*, not one handed to it already built. That distinction is the defect's
/// whole shape: `an_unfinished_observation_survives_every_question_resolution_asks`
/// supplies a finished census and can only measure the decision, while what
/// went wrong twice is *when* the classes are learnt. Two independent reviews
/// reintroduced the defect under that test in nine lines, by observing a second
/// time on the refusal path, and it passed.
/// `resolution_answers_from_the_observation_it_took` drives this instead, over
/// a real tree, with a source whose answer changes between observations.
fn resolve_observed(
    repo_root: &Path,
    wanted: &str,
    observe: &mut dyn FnMut(&Path) -> RunDirClass,
) -> Result<String, UpstrokeError> {
    resolve_in_census(
        &census_observed(repo_root, observe),
        &runs_root(repo_root),
        wanted,
    )
}

/// What the census said about each directory a typed id names.
///
/// **The unknown is a member of this, not a filter applied before it.** The
/// candidate set a prefix resolved against used to be
/// `names_classified(census, listed_as_run)` — the committed ids, everything
/// else dropped — and the dropping is where the answer went: an
/// [`RunDirClass::Indeterminate`] entry left the candidates as though the
/// census had established it was not a run, which is the one thing it did not
/// establish. Measured: two directories sharing `01AAA`, an observation of the
/// first that did not finish, and `01AAA` resolved to the **second** where
/// `231c1aad` had refused it as ambiguous.
///
/// Kept here, a directory the census could not read is a candidate whose
/// completion decides the answer, and [`resolve_in_census`] can see it at the
/// moment it decides.
struct Matched<'a> {
    /// Runs: a committed `run_started` first line was read there.
    committed: Vec<&'a str>,
    /// Husks: the directory was read and holds no committed run.
    husks: Vec<&'a str>,
    /// Neither — and **not** "not a run". The observation did not finish, so
    /// each of these is a directory that may be either of the two above.
    unread: Vec<&'a str>,
}

impl<'a> Matched<'a> {
    /// Every census entry `wanted` names as a case-folded prefix.
    ///
    /// Case-insensitive because a run directory can arrive from a
    /// case-insensitive filesystem; the ids kept are the entries as they exist
    /// on disk, since everything downstream joins one into a path.
    fn of(census: &'a [(String, RunDirClass)], wanted: &str) -> Self {
        let wanted_upper = wanted.to_ascii_uppercase();
        let mut matched = Self {
            committed: Vec::new(),
            husks: Vec::new(),
            unread: Vec::new(),
        };
        for (run_id, class) in census {
            if !run_id.to_ascii_uppercase().starts_with(&wanted_upper) {
                continue;
            }
            // Exhaustive for the reason `listed_as_run` is, with more resting
            // on it: where a fourth classification is placed here *is* the
            // decision about whether a prefix may resolve past one.
            match class {
                RunDirClass::Committed => matched.committed.push(run_id),
                RunDirClass::Husk => matched.husks.push(run_id),
                RunDirClass::Indeterminate => matched.unread.push(run_id),
            }
        }
        matched
    }
}

/// [`resolve_run_id`] over the census it took.
///
/// **Every question resolution asks is asked of that one census.** Without
/// this, the repair loses on this path what it wins in the classifier: a first
/// pass answering [`RunDirClass::Indeterminate`] dropped the directory from the
/// runs, and a second and third pass that then read the log found it neither a
/// husk nor unclassifiable, so the refusal fell through to "no runs found" and
/// the incomplete observation — the reason the reader refused — was gone.
/// [`census`] carries that measurement.
///
/// **The decision below is the module doc's rule written as one `match`.** The
/// resolving arm is the only shape every completion of the census agrees on,
/// and it names the unread set in its own pattern: a later change that wants to
/// answer past an unfinished observation has to delete `[]` from that arm,
/// which is a visible edit to the rule rather than a filter one function
/// earlier that reads like tidying.
///
/// **This signature is not a guard, and the two rounds before this one said it
/// was.** Being handed a census and no repository root does not stop a body
/// added later from observing again — `runs_root.parent().and_then(Path::parent)`
/// is the repository root, and a review wrote exactly that line here and
/// compiled it. Nor does the count of `classify_run_dir` mentions, which a
/// second *call* of the one site does not move. What holds the rule is
/// `resolution_answers_from_the_observation_it_took`, and this module claims
/// nothing the driver does not measure.
///
/// **Split from [`resolve_run_id`] so a census is also a value a test can
/// write**, which is the split [`class_of`](super::classify) takes one module
/// over and for the same reason: no `events.jsonl` a fixture here can write
/// classifies `Indeterminate`.
fn resolve_in_census(
    census: &[(String, RunDirClass)],
    runs_root: &Path,
    wanted: &str,
) -> Result<String, UpstrokeError> {
    // An exact id names one directory — a name is unique under one root — so
    // what the census said about *that* entry is the whole answer, and it is
    // the same answer under every completion, whatever else went unread. The
    // entry as it exists on disk is what comes back, not the uppercased input:
    // on a case-sensitive filesystem only the real name builds a path that
    // opens, and everything downstream joins this id.
    if let Some((run_id, class)) = census
        .iter()
        .find(|(run_id, _)| run_id.eq_ignore_ascii_case(wanted))
    {
        return match class {
            RunDirClass::Committed => Ok(run_id.clone()),
            RunDirClass::Husk => Err(refused(husk_sentence(
                run_id,
                &unread_note(&names_classified(census, unclassified)),
            ))),
            RunDirClass::Indeterminate => Err(refused(unread_sentence(run_id))),
        };
    }
    let matched = Matched::of(census, wanted);
    match (matched.committed.as_slice(), matched.unread.as_slice()) {
        // One run matches, and nothing that might have been a second one: the
        // same answer however the directories this census could not read would
        // have read.
        ([only], []) => Ok((*only).to_owned()),
        // A directory this prefix names did not classify. Under the completion
        // where it committed, the answer is that id or an ambiguity; under the
        // one where it is a husk, it is the run beside it. Two answers is no
        // answer, and resume opens what this returns.
        (_, [named, also @ ..]) => Err(refused(unresolvable_prefix(&matched, named, also))),
        // Nothing the census read is a run under that id.
        ([], []) => Err(refused(no_match(census, runs_root, &matched))),
        // Two or more runs the census read: the refusal `231c1aad` gave, for
        // the case it is still the right one for.
        (several, []) => Err(refused(format!(
            "that prefix matches {} runs ({}); use more characters",
            several.len(),
            several.join(", ")
        ))),
    }
}

/// A refusal, in the shape every sentence in this module is returned in.
fn refused(message: String) -> UpstrokeError {
    UpstrokeError::Refused { message }
}

/// What [`resolve_in_census`] refuses with when a prefix names a directory
/// whose observation did not finish.
///
/// **This is `231c1aad`'s ambiguity refusal, extended to the case it could not
/// have had.** With two classifications a prefix was ambiguous when two runs
/// answered to it; with three, it is also unresolvable when one run and one
/// *unknown* do, because the unknown is a run under half the completions. The
/// sentence therefore names the directory that could not be read, says what is
/// unknown about it, and names the runs the prefix does match — so the operator
/// can type a full id, which the exact-match arm answers whatever else the
/// census could not read.
fn unresolvable_prefix(matched: &Matched<'_>, named: &str, also: &[&str]) -> String {
    let runs = match matched.committed.as_slice() {
        [] => String::new(),
        [only] => format!(
            " — it also matches the run {only}, and answering with that could open a different \
             run than the one asked for"
        ),
        several => format!(
            " — it also matches {} runs ({}), and answering with one of those could open a \
             different run than the one asked for",
            several.len(),
            several.join(", ")
        ),
    };
    match also {
        [] => format!(
            "that prefix cannot be resolved: `{named}` could not be classified, so whether a run \
             committed there is unknown{runs}; type a full run id, or ask again once its \
             {EVENT_LOG} can be read"
        ),
        _ => format!(
            "that prefix cannot be resolved: {} directories could not be classified ({}), so \
             whether a run committed in them is unknown{runs}; type a full run id, or ask again \
             once their {EVENT_LOG} can be read",
            matched.unread.len(),
            matched.unread.join(", ")
        ),
    }
}

/// What [`resolve_in_census`] refuses with when no run the census read answers
/// to that id, and no directory it could not read does either.
///
/// Three sentences, and which one is given is decided **entirely from the
/// census it is handed**. Nothing here classifies anything: a second
/// observation at this point is what lost `SWEEP-CLASSIFY-001`'s answer on this
/// path, since the directory whose read did not finish is exactly the directory
/// whose next read may finish, and the refusal would then describe a directory
/// in terms the reader never established.
fn no_match(census: &[(String, RunDirClass)], runs_root: &Path, matched: &Matched<'_>) -> String {
    let runs = names_classified(census, listed_as_run);
    let unread = names_classified(census, unclassified);
    match matched.husks.as_slice() {
        // A directory is there, and it holds no committed `run_started`. Saying
        // "no run matches that id" of a directory the operator can see is the
        // answer that sends them looking for a bug.
        [husk] => husk_sentence(husk, &unread_note(&unread)),
        // Either nothing answers to that id at all, or several husks do and no
        // one of them is what was meant. The two sentences left are claims
        // about the *set* — "there are no runs here" and "none of the runs here
        // is yours" — and neither is true of a census that could not read every
        // directory it walked, so an unfinished observation is carried into
        // both rather than dropped because the operator happened to type a
        // prefix that missed it.
        _ if runs.is_empty() => format!(
            "no runs found under {}{}",
            runs_root.display(),
            unread_note(&unread)
        ),
        _ => format!(
            "no run matches that id; known runs: {}{}",
            runs.join(", "),
            unread_note(&unread)
        ),
    }
}

/// The sentence for a directory the census read and found no committed run in,
/// with the note its census owes whatever it says.
///
/// **It carries [`unread_note`] and that is not a spare.** It is a claim about
/// one directory's *contents* — "never recorded a committed run_started" —
/// reached by name out of a set the operator did not see, and a census holding
/// both a husk and a directory it could not read answers it while knowing less
/// than it says. The note is a parameter because both callers have already
/// derived it from the same census, and deriving it twice on one path invites
/// the two to drift.
fn husk_sentence(husk: &str, note: &str) -> String {
    format!(
        "`{husk}` never recorded a committed run_started, so there is no run to open there — ask \
         `upstroke status {husk}` for what it is and what happens to it{note}"
    )
}

/// The sentence for a directory whose observation did not finish, when the id
/// typed names it.
///
/// **It promises nothing about the next census, and it used to.** The sentence
/// ended "the next census retains it", which is a claim about a *later*
/// command's observation and not about anything this one did: measured on a
/// reclaimable directory with an uncommitted log, the census after the
/// interrupted one read the log normally, classified `Husk`, and reclaimed both
/// halves. What is true is what this command did — it read, it could not
/// finish, and it deleted nothing — and that a later command classifies the
/// directory again from its own read, which may be the read that finishes.
fn unread_sentence(named: &str) -> String {
    format!(
        "`{named}` could not be classified: its {EVENT_LOG} could not be read to a first line, \
         so whether a run committed there is unknown — nothing has been deleted, and the next \
         command classifies it again from its own read"
    )
}

/// What a refusal adds when some directory the census walked did not classify.
///
/// Empty when every directory classified, so the three sentences it is appended
/// to are unchanged for every census that finished — which is every census
/// `run_dir_names` walks over ordinary regular files.
///
/// The same correction [`unread_sentence`] carries: this says what the command
/// giving the refusal did, and not what a later one will find.
fn unread_note(unread: &[String]) -> String {
    match unread {
        [] => String::new(),
        [only] => format!(
            "; and `{only}` could not be classified, so whether a run committed there is \
             unknown — nothing has been deleted, and the next command classifies it again from \
             its own read"
        ),
        several => format!(
            "; and {} directories could not be classified ({}), so whether a run committed in \
             them is unknown — nothing has been deleted, and the next command classifies them \
             again from its own read",
            several.len(),
            several.join(", ")
        ),
    }
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
///
/// **One census, and the module doc's rule applied to the set it scans.** The
/// runs it walks come from the same census its refusals are written from, so a
/// directory that did not classify cannot be walked past here and dropped from
/// the sentence there. A full question id still answers: it names one file, and
/// no unread directory makes an exact name mean something else.
pub fn find_question(repo_root: &Path, wanted: &str) -> Result<FoundQuestion, UpstrokeError> {
    find_question_observed(repo_root, wanted, &mut classify_run_dir)
}

/// [`find_question`] with its one observation supplied, for the reason
/// [`resolve_observed`] is: the sentences below are decided from a census, and
/// a census carrying a directory that did not classify is not something a
/// fixture in this suite can write.
///
/// See `a_question_search_carries_the_run_it_could_not_classify`, in this
/// module's own test suite: `src/rundir/discovery.rs` is an
/// `effects::CLASSIFIED_MODULES` entry, so a `pub(super)` seam would need a row
/// in `effects/wrappers.toml` — an instrument this pull request does not touch.
/// The fixtures come across from `rundir::tests` instead, built there.
fn find_question_observed(
    repo_root: &Path,
    wanted: &str,
    observe: &mut dyn FnMut(&Path) -> RunDirClass,
) -> Result<FoundQuestion, UpstrokeError> {
    let census = census_observed(repo_root, observe);
    let wanted_upper = wanted.to_ascii_uppercase();
    let mut exact: Option<FoundQuestion> = None;
    let mut matches: Vec<FoundQuestion> = Vec::new();
    for run_id in names_classified(&census, listed_as_run) {
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
    // The rule the module doc states, on the reader beside resolution: every
    // answer below is a claim about the *set* of questions this repository
    // holds, and a run the census could not classify may hold another that
    // matches — so a lone match is an unambiguous one only where the census
    // finished, and each refusal carries what went unread.
    let unread = names_classified(&census, unclassified);
    if unread.is_empty() && matches.len() == 1 {
        return matches
            .pop()
            .ok_or_else(|| refused("question vanished while resolving it".to_owned()));
    }
    let named: Vec<&str> = matches
        .iter()
        .map(|found| found.question_id.as_str())
        .collect();
    Err(refused(match named.as_slice() {
        [] => format!(
            "no question with that id under {}{}",
            runs_root(repo_root).display(),
            unread_note(&unread)
        ),
        // Reached only with something unread, since a lone match answered
        // above: the question is there, and whether a second one answers to the
        // same prefix is what this command could not establish.
        [only] => format!(
            "that prefix matches one question ({only}) among the runs that classified, and a \
             run that did not may hold another; use the full question id{}",
            unread_note(&unread)
        ),
        several => format!(
            "that prefix matches {} questions ({}); use more characters{}",
            several.len(),
            several.join(", "),
            unread_note(&unread)
        ),
    }))
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{
        RunDirClass, UpstrokeError, classify_run_dir, find_question, find_question_observed,
        listed_as_husk, listed_as_run, public_dir, resolve_in_census, resolve_observed,
        resolve_run_id, unclassified,
    };
    use crate::rundir::tests::{commit_run, husk_run, question_in, scratch};

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

    /// A prefix an unfinished observation also names resolves to **nothing**,
    /// and an exact id still resolves.
    ///
    /// The rule this module opens with, at the one place where dropping the
    /// unknown does not merely lose a sentence: with the `Indeterminate` entry
    /// filtered out before the ambiguity check, `01A` below matched exactly one
    /// committed run and resolved to it — and `231c1aad`, which had no third
    /// class, refused the same prefix as ambiguous. Round 1 made resolution
    /// refuse where it should have answered; that made it answer where it
    /// cannot know, which is worse, because resume opens the id it returns.
    ///
    /// Four shapes and a control. The control is the one that says what the
    /// refusal is about: with a **husk** where the unknown was, the same prefix
    /// resolves, because a husk is an answer and this is not.
    #[test]
    fn a_prefix_an_unfinished_observation_names_resolves_to_nothing() {
        let mixed = census(&[
            ("01AAA", RunDirClass::Committed),
            ("01AXX", RunDirClass::Indeterminate),
        ]);

        let shared = resolve_in_census(&mixed, runs_root(), "01A")
            .expect_err("one run and one unknown share that prefix")
            .to_string();
        assert!(
            shared.contains("01AXX") && shared.contains("could not be classified"),
            "the refusal is about the directory the census could not read: {shared}"
        );
        assert!(
            shared.contains("01AAA"),
            "and names the run it does match, so a full id can be typed: {shared}"
        );

        // An exact id names one directory, so the unknown beside it changes
        // nothing: this is the answer under every completion.
        assert_eq!(
            resolve_in_census(&mixed, runs_root(), "01aaa").expect("an exact id, case-folded"),
            "01AAA"
        );

        // The unknown's own id answers with the unknown's own sentence.
        let named = resolve_in_census(&mixed, runs_root(), "01AXX")
            .expect_err("a directory that did not classify is not a run")
            .to_string();
        assert!(
            named.contains("`01AXX` could not be classified"),
            "the id typed names it: {named}"
        );

        // Two of them, named together rather than one of them guessed at.
        let both = census(&[
            ("01AAA", RunDirClass::Committed),
            ("01AXX", RunDirClass::Indeterminate),
            ("01AYY", RunDirClass::Indeterminate),
        ]);
        let two = resolve_in_census(&both, runs_root(), "01A")
            .expect_err("two unknowns share that prefix")
            .to_string();
        assert!(
            two.contains("2 directories could not be classified")
                && two.contains("01AXX")
                && two.contains("01AYY"),
            "both are named: {two}"
        );

        // The control: a husk in the same position is an answer, and the prefix
        // resolves past it exactly as it did before this rule existed.
        let with_husk = census(&[
            ("01AAA", RunDirClass::Committed),
            ("01AXX", RunDirClass::Husk),
        ]);
        assert_eq!(
            resolve_in_census(&with_husk, runs_root(), "01A")
                .expect("a husk is not a candidate for a prefix"),
            "01AAA",
            "what blocks resolution is the unknown, not a second directory"
        );
    }

    /// Nothing in this module observes a directory except through
    /// [`census_observed`]'s parameter, and `classify_run_dir` is what supplies
    /// it.
    ///
    /// **A census of mentions is not the guard, and for two rounds it was
    /// offered as one.** Two review lenses reintroduced the resolution defect
    /// by *calling* an existing site a second time — which moves no mention,
    /// changes no signature, and passed this assertion and every other test in
    /// this module. What holds the rule is
    /// `resolution_answers_from_the_observation_it_took`, which drives
    /// [`resolve_observed`] over a real run tree with a source whose answer
    /// changes between observations. This states the smaller fact that is still
    /// worth an assertion: *where* a classification can enter is three lines,
    /// so "which observation did this reader decide from" is read rather than
    /// traced.
    ///
    /// The four mentions are the import and the three supplies: [`census`] for
    /// the listing readers, and [`resolve_run_id`] and [`find_question`], each
    /// of which is one line handing [`classify_run_dir`] to the body that
    /// decides. A fifth is another way in.
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
            "fn census_observed(",
            "pub fn list_runs(",
            "pub fn list_husks(",
            "pub fn resolve_run_id(",
            "pub fn find_question(",
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
            4,
            "SWEEP-CLASSIFY-001: the import, and the three bodies that supply an observation — \
             `census` for the listing readers, and `resolve_run_id` and `find_question`, each \
             of which is one line of supply. A fifth mention is another way a classification \
             enters this module, and what it enters is a decision that has to be made from \
             one observation."
        );
    }

    /// An observation source whose answer **changes between observations**:
    /// the first look at one named directory did not finish, and every look
    /// after it — at that directory or any other — reads the tree as it is.
    ///
    /// **This is the 65,537-interruption sequence's shape**, which is the part the
    /// resolution defect turns on: a source that answers `Interrupted` through
    /// the whole of the probe's allowance and then reads normally classifies
    /// `Indeterminate` once and `Committed` after, and a reader that observes
    /// twice keeps the second answer. Nothing in this suite can arrange the
    /// signal itself — `RunDirClass::Indeterminate`'s own note says why, and
    /// the two reviews that measured the defect injected the interruptions
    /// into a real log from outside the process — so the answer is supplied
    /// where production supplies `classify_run_dir`, through
    /// `census_observed`'s parameter.
    ///
    /// Every later look calls the real classifier rather than returning a
    /// constant: the directories underneath are real and readable, so the tree
    /// disagrees with the first look, which is what makes a second observation
    /// visible however it is taken.
    struct FirstLookUnread {
        /// The public directory whose first observation does not finish.
        unreadable: PathBuf,
        /// Every directory this source was asked about, in order. The
        /// arguments rather than a count: what is asserted is which
        /// directories one resolution observed, not how many times something
        /// was called.
        asked: Vec<PathBuf>,
    }

    impl FirstLookUnread {
        fn of(public: &Path) -> Self {
            Self {
                unreadable: public.to_owned(),
                asked: Vec::new(),
            }
        }

        fn observe(&mut self, public: &Path) -> RunDirClass {
            let first_look =
                public == self.unreadable && !self.asked.iter().any(|seen| seen == public);
            self.asked.push(public.to_owned());
            if first_look {
                RunDirClass::Indeterminate
            } else {
                classify_run_dir(public)
            }
        }
    }

    /// Resolve `wanted` in `repo`, with the first observation of `unreadable`
    /// the one that did not finish. The answer, and the directories that
    /// resolution observed to reach it.
    ///
    /// One source per resolution: the script is one command's sequence of
    /// observations, which is the unit the rule is stated in.
    fn resolving_past_an_unread_first_look(
        repo: &Path,
        unreadable: &str,
        wanted: &str,
    ) -> (Result<String, UpstrokeError>, Vec<PathBuf>) {
        let mut source = FirstLookUnread::of(&public_dir(repo, unreadable));
        let answer = resolve_observed(repo, wanted, &mut |public| source.observe(public));
        (answer, source.asked)
    }

    /// Resolution answers from the observation it took, and takes one.
    ///
    /// **The third attempt at this guard and the first that drives anything.**
    /// Round 1 crossed the three classifications against the three reader
    /// predicates: right about each predicate, blind to the reader above them,
    /// which asks three questions and has to answer from all three. Round 2
    /// added a census of `classify_run_dir` mentions and a decision driven
    /// over a census handed to it already built. Two review lenses then
    /// reintroduced the original defect independently — one deriving the
    /// candidates from a first census and the refusal from a second, one in
    /// nine lines that re-classified on the refusal path through the existing
    /// `census` — and **no signature changed, no mention moved, all 103 tests
    /// in this module passed, and the original failure came back**.
    ///
    /// What both mutations have in common is a *second observation*, so that
    /// is what this drives. `resolve_observed` is the whole body of
    /// `resolve_run_id`; the source it is given answers `Indeterminate` the
    /// first time it is asked about one directory and reads the tree every
    /// time after. The tree is real and every directory in it is readable,
    /// which closes the other route: an observation taken *around* the source
    /// — another `census(repo_root)`, a `list_husks(repo_root)`, anything that
    /// reaches the filesystem — answers `Committed` for that same directory. A
    /// second observation therefore changes what the resolver says whichever
    /// way it is taken, and each assertion below says which sentence it
    /// changes into.
    ///
    /// The four shapes are the four answers an unfinished observation can
    /// reach: a prefix it shares with a run, the id that names it, the id of a
    /// directory that is a husk underneath, and an id that names none of them.
    #[test]
    fn resolution_answers_from_the_observation_it_took() {
        const FIRST: &str = "01M00000000000000000000000";
        const SECOND: &str = "01M00000000000000000000001";
        const HUSK: &str = "01H00000000000000000000000";

        let repo = scratch("resolveobserved").join("repo");
        commit_run(&repo, FIRST);
        commit_run(&repo, SECOND);
        let husk = husk_run(&repo, HUSK);

        // The control, and the half that makes every assertion below a
        // measurement of the supplied observation rather than of the fixture:
        // read as it is, through the public resolver with nothing supplied,
        // this tree holds two runs that share `01M` and one husk.
        assert_eq!(
            resolve_run_id(&repo, FIRST).expect("the tree really holds this run"),
            FIRST
        );
        let ambiguous = resolve_run_id(&repo, "01M")
            .expect_err("two runs share that prefix")
            .to_string();
        assert!(
            ambiguous.contains("matches 2 runs"),
            "the tree answers `Committed` for both: {ambiguous}"
        );
        let as_husk = resolve_run_id(&repo, HUSK)
            .expect_err("a husk is not a run")
            .to_string();
        assert!(
            as_husk.contains("never recorded a committed run_started"),
            "and `Husk` for the third: {as_husk}"
        );

        // (1) The prefix an unfinished observation shares with a run.
        //     Resolving it to the run is how a transient read failure
        //     redirects a resume: what `resolve_run_id` returns is what
        //     `upstroke resume` opens.
        let (answer, asked) = resolving_past_an_unread_first_look(&repo, FIRST, "01M");
        let shared = answer
            .expect_err("a prefix an unfinished observation also names cannot resolve")
            .to_string();
        assert!(
            shared.contains(FIRST) && shared.contains("could not be classified"),
            "the directory whose observation did not finish is what the refusal is about: {shared}"
        );
        assert!(
            !shared.contains("matches 2 runs"),
            "and not the ambiguity sentence, which is the answer a second observation of \
             {FIRST} produces: {shared}"
        );
        assert_eq!(
            asked,
            vec![
                husk.clone(),
                public_dir(&repo, FIRST),
                public_dir(&repo, SECOND)
            ],
            "one resolution observed each directory once, in census order"
        );

        // (2) The sequence round 1 lost: the id names the directory whose
        //     observation did not finish, and the answer says so rather than
        //     reporting an empty runs root.
        let (answer, _) = resolving_past_an_unread_first_look(&repo, FIRST, FIRST);
        let named = answer
            .expect_err("a directory that did not classify is not a run")
            .to_string();
        assert!(
            named.contains(FIRST) && named.contains("could not be classified"),
            "the id typed names it: {named}"
        );
        assert!(
            !named.contains("no runs found"),
            "and not round 1's answer, which is what a second observation restores: {named}"
        );

        // (3) A husk underneath whose read did not finish. The husk sentence
        //     is a claim about contents this observation did not establish,
        //     and it is the sentence a second observation reaches.
        let (answer, _) = resolving_past_an_unread_first_look(&repo, HUSK, HUSK);
        let unfinished = answer
            .expect_err("an unfinished observation is not a husk")
            .to_string();
        assert!(
            unfinished.contains("could not be classified"),
            "what the reader established is that it could not read it: {unfinished}"
        );
        assert!(
            !unfinished.contains("never recorded a committed run_started"),
            "and never the husk sentence, which is what the tree says and this command did not: \
             {unfinished}"
        );

        // (4) An id that names none of them. The sentence left is a claim
        //     about the set, and the set is what an unfinished observation
        //     makes incomplete.
        let (answer, _) = resolving_past_an_unread_first_look(&repo, FIRST, "02ZZZ");
        let missed = answer.expect_err("no run answers to that id").to_string();
        assert!(
            missed.contains("known runs") && missed.contains(SECOND),
            "the runs it did read are listed: {missed}"
        );
        assert!(
            missed.contains(FIRST) && missed.contains("could not be classified"),
            "and the one it could not read is carried into a claim about the set it belongs to; a \
             second observation drops the note and lists {FIRST} as a known run: {missed}"
        );
    }

    /// A question search carries the run it could not classify, and does not
    /// call a lone match unambiguous while one is unread.
    ///
    /// The same rule as `resolution_answers_from_the_observation_it_took`, on
    /// the reader beside it, driven the same way: `find_question` scans the
    /// runs a census listed and then writes a sentence about the *set* of
    /// questions this repository holds. A run whose classification did not
    /// finish is not scanned and may hold a question that answers to the same
    /// prefix, so "no question with that id" and "that prefix matches one
    /// question" are both claims the command cannot make on its own.
    ///
    /// A full question id still answers, and the first assertion is that
    /// control: it names one file, and an unread directory does not make an
    /// exact name mean something else.
    #[test]
    fn a_question_search_carries_the_run_it_could_not_classify() {
        const READ: &str = "01Q00000000000000000000000";
        const UNREAD: &str = "01Q00000000000000000000001";

        let repo = scratch("questionunread").join("repo");
        for (run, question) in [(READ, "q-ONE"), (READ, "q-ONLY"), (UNREAD, "q-TWO")] {
            question_in(&repo, run, question);
        }

        let search = |wanted: &str| {
            let mut source = FirstLookUnread::of(&public_dir(&repo, UNREAD));
            find_question_observed(&repo, wanted, &mut |public| source.observe(public))
        };

        // The control: the tree is readable and answers as it always did.
        assert_eq!(
            find_question(&repo, "q-TWO")
                .expect("the tree really holds this question")
                .run_id,
            UNREAD
        );

        // A full id is not a prefix of anything, so it answers.
        assert_eq!(
            search("q-ONE").expect("an exact question id").question_id,
            "q-ONE"
        );

        // One match among the runs that classified, and a run that did not.
        let lone = search("q-ONL")
            .expect_err("a lone match is unambiguous only across a census that finished")
            .to_string();
        assert!(
            lone.contains("q-ONLY")
                && lone.contains(UNREAD)
                && lone.contains("could not be classified"),
            "the match is named, and so is the run that may hold another: {lone}"
        );

        // The question really is in the unread run, and the refusal says the
        // set is incomplete rather than that nothing answers to the id.
        let absent = search("q-TW")
            .expect_err("the run holding it did not classify")
            .to_string();
        assert!(
            absent.contains("no question with that id") && absent.contains(UNREAD),
            "an absence is not a fact about a census that could not read every run: {absent}"
        );
    }
}
