//! The names one run's files and directories carry on disk, as constants.
//!
//! The marker, the two private records and their staging siblings, the event
//! log, the frozen plan, the report and the three names its staging takes, and
//! the eight directories of the run's skeleton. They are consts rather than a
//! literal at each call site because the sites that have to agree on the byte
//! string sit in different files — or, for the report and the skeleton, in
//! different functions of one file. Until `SWEEP-NAMES-004` moved them here,
//! each of those nine was spelt once by what makes it and again by the
//! `RunPaths` accessor that returns it, so a skeleton directory could be
//! created at one name and read at another.
//!
//! **Who writes each, exactly.** Of the marker, the records, the log and the
//! plan, seven are published by a funnel in `src/rundir.rs`: `stage_marker` and
//! `publish_marker`, the owner and commit-record pairs, and `write_plan` for
//! [`PLAN`]. [`EVENT_LOG`] is the exception and is worth knowing about —
//! `src/rundir.rs` only *names* it, in `RunPaths::events` and in the re-export;
//! the log itself is opened and appended by `EventLog` in `src/events/log.rs`,
//! which takes the path it is given. So this module names a file that `rundir`
//! does not write. [`REPORT`] and its three staging names belong to
//! `write_report`, which publishes the one and makes and reclaims what the
//! other three name. The eight directories `rundir` *makes* and nothing more:
//! `RunPaths::create_hooked` and its fresh-run sibling iterate `PUBLIC_DIRS` and
//! `PRIVATE_DIRS`, which are built from these constants, as
//! `create_private_skeleton` iterates the second. What goes inside each is
//! named by whoever takes the accessor's path, not here.
//!
//! Reading them back: the classifier in `classify.rs` (`EVENT_LOG`), the
//! ownership proof in `ownership.rs`, below, the `RunPaths` accessors, and
//! `find_question` in `discovery.rs`, which lists [`QUESTIONS_DIR`] for each run
//! without building a `RunPaths`. Pinning them:
//! `the_names_on_disk_are_the_names_the_packet_writes` in `tests.rs` binds
//! every const to its literal. Three more fail on an **accessor drift** — a
//! `RunPaths` accessor spelling something other than what its writer or
//! creator used. `the_event_log_and_plan_accessors_return_the_paths_their_constants_name`
//! holds [`EVENT_LOG`] and [`PLAN`]; it does not fail on a change to the const,
//! which moves both sides of its assertion, and it cannot witness the
//! substitution that put accessor and const in agreement at all; its own doc
//! says why.
//! `the_skeleton_accessors_name_exactly_the_directories_the_skeleton_arrays_create`
//! asserts each directory accessor's last component against the element of
//! `PUBLIC_DIRS` or `PRIVATE_DIRS` that the creator iterates, never against a
//! literal, so it fails from either end — an accessor that leaves its array, or
//! an array element that leaves its accessor. And
//! `the_report_accessor_returns_the_file_write_report_publishes` writes a report
//! through the funnel and reads it back through `RunPaths::report_json`.
//!
//! **One source of truth for `rundir`, not yet for the crate.** [`EVENT_LOG`]
//! and [`PLAN`] name the two files a command outside this module opens by path
//! rather than through `RunPaths`, and no production code outside `rundir`
//! spells either as a run-directory path any longer: the sites that did now
//! go through the constant. (`validate --emit-json`
//! writes a file of [`PLAN`]'s name into the working directory, which is not a
//! run directory.) The nine this module took from `src/rundir.rs` have not had
//! that pass, and **six production sites in three files** still spell three of
//! them for themselves: four of `questions` (three in `answer.rs`, and one in
//! `engine/report.rs`, in the path its summary prints), one of `answers`
//! (`answer.rs`) and one of `report.json` (`engine/topology/finalize.rs`). A
//! rename of [`QUESTIONS_DIR`], [`ANSWERS_DIR`] or [`REPORT`] is therefore still
//! a multi-file edit. The other six directories are not, and neither are the
//! six names of the creation protocol: no production code outside `rundir`
//! spells any of them as a path. One test does — `engine/topology/emit/tests.rs`,
//! joining `committed.json` — and that one is load-bearing rather than untidy;
//! see below and `SWEEP-NAMES-003`.
//!
//! **What the proof consults, exactly.** `prove_private_half_ownership` reads
//! two of these back and parses each ([`MARKER`], then [`OWNER_RECORD`] at the
//! locator the marker names); *stats* a third, because conjunct 12 is
//! [`COMMIT_RECORD`]'s **existence** and never its content; and compares a lone
//! directory entry against [`MARKER_STAGED`] to tell a bare husk from a staged
//! one. Four names, three kinds of use — a reader replacing the stat with a
//! read would be weakening the deletion boundary, not tidying it.
//!
//! **What has product authority here, and what does not.** `DESIGN.md` §15's
//! run-directory drawing carries [`EVENT_LOG`] and [`PLAN`], and all nine this
//! module took from `src/rundir.rs`: [`REPORT`] and the eight skeleton
//! directories. The six names of the creation protocol — the marker and the two
//! private records, each with its staging sibling — appear in no `design/`
//! section at all, so this repository states no rule about renaming them — and
//! for these six a rename is not a refactor. A run killed after old P5b still
//! holds a file called `committed.json`; a proof built on the new name would
//! stat it, find it absent, mint a deletion token on that absence and delete a
//! private half that had crossed the deletion boundary. The missing
//! compatibility contract is `SWEEP-NAMES-008`, deferred: it belongs in §15 and
//! `DESIGN.md` is the owner's.
//!
//! One test stands between that and a green suite today, and it is not in this
//! module: `engine/topology/emit/tests.rs` joins `"committed.json"` by hand.
//! `SWEEP-NAMES-003` holds the measurement and states the count; it is not
//! repeated here, because a number restated in four places is a number that
//! drifts in three of them.
//!
//! Nothing in `src/` stands in for that, and nothing here tries to. A retired
//! decision record establishes no product contract, and neither does
//! implementation or test code quoting one, so no citation of
//! `decisions.workspace_candidates.run_creation` appears above. Until §15 says
//! otherwise, the creation protocol's six byte strings are pinned by
//! `the_names_on_disk_are_the_names_the_packet_writes` and by nothing with
//! design standing.

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
#![forbid(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros
)]

/// `<public>/.creating` — the P1 marker.
pub const MARKER: &str = ".creating";
/// `<public>/.creating.tmp` — the P1 staging file.
pub const MARKER_STAGED: &str = ".creating.tmp";
/// `<private>/owner.json` — the P3b reciprocal ownership record.
pub const OWNER_RECORD: &str = "owner.json";
/// `<private>/owner.json.tmp`.
pub const OWNER_RECORD_STAGED: &str = "owner.json.tmp";
/// `<private>/committed.json` — the P5b private commit record.
pub const COMMIT_RECORD: &str = "committed.json";
/// `<private>/committed.json.tmp`.
pub const COMMIT_RECORD_STAGED: &str = "committed.json.tmp";
/// `<public>/events.jsonl`.
pub const EVENT_LOG: &str = "events.jsonl";
/// `<public>/report.json` — the projection of the log `write_report` publishes
/// and `RunPaths::report_json` returns.
pub const REPORT: &str = "report.json";

/// `<public>/.report-staging-<ulid>/` — the directory one report write makes
/// for itself, under a name unique to the write, exclusively (`create_dir`,
/// never `create_dir_all`), stages `report.json` inside, renames it up onto
/// the published name — the same filesystem, the rename atomic — and
/// removes, empty, before it returns. Which directory is this run's is not
/// read off the name: before the directory is made, the write records its
/// name durably in the run's private half ([`REPORT_STAGING_RECORD`]), and
/// only a directory that record names is ever removed — as the run's own
/// tree, by the record — before the next write makes its own. A directory of
/// this shape that no record names is not the protocol's: it is left as
/// found and named in the write's diagnostic, never removed and never
/// adopted, and the write proceeds under its own fresh name. Until PR10's
/// round 8 the report was staged at the fixed `report.json.tmp` beside its
/// published name and a single-link file an operator had left there was
/// removed as the writer's own; in round 8 under `report.json.<ulid>.tmp`,
/// and a recogniser over that shape removed whatever wore it, a name the
/// ULID producer cannot emit included; in round 9 under the fixed
/// `.report-staging/`, and whatever directory stood at that name was removed
/// by its name and type before the write's own exclusive creation, which
/// proved ownership of the replacement and nothing of what was deleted (the
/// round-8, round-9 and round-10 regression lenses, P2; `standards/08`).
pub const REPORT_STAGING_PREFIX: &str = ".report-staging-";
/// `<private>/report-staging.json` — the record of the staging directory the
/// report write is about to make under `public`, written durably (staged at
/// [`REPORT_STAGING_RECORD_STAGED`], synced, renamed, the private directory
/// synced) before `create_dir` runs, and removed after the rename that empties
/// the directory. It lives in the private half because that is the run's own
/// tree by the token-carried rule, where a record under the public directory
/// would itself stand at a fixed name the rule forbids removing by; what a
/// writer that died between its stage and its rename leaves is this record
/// and the directory it names, with the staged file inside, and the next
/// write removes the directory the record names and then the record.
pub const REPORT_STAGING_RECORD: &str = "report-staging.json";
/// `<private>/report-staging.json.tmp`.
pub const REPORT_STAGING_RECORD_STAGED: &str = "report-staging.json.tmp";
/// `<public>/plan.normalized.json`.
pub const PLAN: &str = "plan.normalized.json";

// The skeleton: the eight directories `RunPaths::create_hooked` makes by
// iterating `PUBLIC_DIRS` and `PRIVATE_DIRS` in `src/rundir.rs`, which are built
// from these, and that each `RunPaths` accessor of the same name returns.

/// `<public>/artifacts/`.
pub const ARTIFACTS_DIR: &str = "artifacts";
/// `<public>/questions/` — one rendered payload per question.
pub const QUESTIONS_DIR: &str = "questions";
/// `<public>/answers/` — where `upstroke answer` drops an answer.
pub const ANSWERS_DIR: &str = "answers";
/// `<private>/transcripts/`.
pub const TRANSCRIPTS_DIR: &str = "transcripts";
/// `<private>/reviews/`.
pub const REVIEWS_DIR: &str = "reviews";
/// `<private>/settings/` — the per-attempt permission surface.
pub const SETTINGS_DIR: &str = "settings";
/// `<private>/gates/`.
pub const GATES_DIR: &str = "gates";
/// `<private>/gate-worktrees/` — durable intents and disposable exact worktrees.
pub const GATE_WORKTREES_DIR: &str = "gate-worktrees";
