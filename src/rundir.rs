//! Run directory layout (DESIGN.md §15) and run discovery.
//!
//! §15 draws the whole run directory under `.upstroke/runs/<run-id>/`. This
//! module splits it in two, and the reason is enforcement rather than tidiness.
//!
//! A reviewer is a read-only agent pointed at the workspace, so every path
//! inside the workspace is reachable — including, before this split, the
//! implementer's own transcript. Invariant 3 says the diff is ground truth and
//! the transcript is not, so a reviewer reading the transcript is judging the
//! wrong evidence. Permission deny rules cannot close that on their own: gates
//! execute repository code the implementer just wrote, and that code reads any
//! workspace path the deny list never sees.
//!
//! So the split follows what each file is *for*. The ops surface — what
//! `status`, `resume`, `answer`, and any future pane read — stays in the repo
//! where §15 documents it and where CI can collect it. The agent-authored text
//! moves to a user-level directory no sandboxed agent has a path into. The
//! `run_started` event records where that directory is, so the record stays
//! self-describing rather than depending on this function's defaults.
// Allowlist placement: the **funnel section** of `effects/allowlist.toml`, which
// carries this module's review clause -- effects only inside site-taking APIs,
// no writable handle returned. `decisions.effect_site_inventory.mechanism` (2).
#![allow(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros
)]

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::UpstrokeError;
use crate::runner::policy::runner_policy_sha256;
use crate::topology::effects::{
    AnswerSite, EffectSiteId, HookHarness, HookPhase, Injection, LockSite, ReportSite, RunDirSite,
};
use crate::topology::events::RunnerPolicy;
use crate::util::{self, DurabilityLedger, DurableStep};
use crate::workspace::Workspace;

/// Created beside the repo: the run's own record and the human/UI surface.
const PUBLIC_DIRS: [&str; 3] = ["artifacts", "questions", "answers"];
/// Created outside the workspace: everything an agent wrote or that describes
/// an agent's sandbox.
const PRIVATE_DIRS: [&str; 5] = [
    "transcripts",
    "reviews",
    "settings",
    "gates",
    "gate-worktrees",
];

/// Where one run's files live, split by who is allowed to read them.
#[derive(Debug, Clone)]
pub struct RunPaths {
    /// `<repo>/.upstroke/runs/<run-id>` — `events.jsonl`, the frozen plan,
    /// artifacts, questions, answers, the lock. Git-ignored, but present
    /// beside the repository it describes.
    pub public: PathBuf,
    /// `~/.upstroke/runs/<run-id>` — transcripts, review verdicts, gate logs,
    /// and the per-attempt permission settings that define each sandbox.
    pub private: PathBuf,
}

impl RunPaths {
    /// Layout for a fresh run, with the private half at its default root.
    pub fn new(repo_root: &Path, run_id: &str) -> Self {
        Self::with_private_root(repo_root, run_id, &default_private_root())
    }

    /// Layout with an explicit private root — how tests stay out of the real
    /// `~/.upstroke`, and how a caller pins the location deliberately.
    pub fn with_private_root(repo_root: &Path, run_id: &str, private_root: &Path) -> Self {
        Self {
            public: public_dir(repo_root, run_id),
            private: private_root.join("runs").join(run_id),
        }
    }

    /// Rebuild from a private directory recorded in `run_started`. Resume and
    /// status use this so they read the run that actually happened rather than
    /// wherever today's defaults would have put it.
    pub fn from_parts(public: PathBuf, private: PathBuf) -> Self {
        Self { public, private }
    }

    /// Create both trees. Callers do this once at run start; every accessor
    /// below assumes it has happened.
    ///
    /// Behaviour-neutral relative to the `create_dir_all` loop this replaced —
    /// the same directories, in the same order — but now through the two sites
    /// that own them, so the effect is inventoried rather than ambient.
    pub fn create(&self) -> Result<(), UpstrokeError> {
        self.create_hooked(&mut NoHooks)
    }

    /// Reserve both roots for a new sequential run, then create their skeletons.
    ///
    /// Each root uses an exclusive create. The private root is reserved first
    /// because it is shared across repositories. Resume and schema-4 creation
    /// use their existing directory protocols instead.
    ///
    /// # Errors
    ///
    /// Refuses an occupied root without adopting its contents. If public
    /// reservation fails, removes only the empty private root just created;
    /// a failed removal reports both errors. Skeleton failures retain the
    /// partial directories for inspection, as [`Self::create`] does.
    pub fn create_fresh(&self) -> Result<(), UpstrokeError> {
        self.create_fresh_hooked(&mut NoHooks)
    }

    /// Fresh sequential creation's sites. This is separate from the existing
    /// idempotent skeleton API and the schema-4 marker protocol.
    fn create_fresh_hooked(&self, hooks: &mut dyn RunDirHooks) -> Result<(), UpstrokeError> {
        funnel(
            hooks,
            EffectSiteId::RunDir(RunDirSite::CreatePrivateDir),
            || create_fresh_root(&self.private, "reserve fresh private run directory"),
        )?;
        funnel(
            hooks,
            EffectSiteId::RunDir(RunDirSite::CreatePublicDir),
            || {
                if let Err(error) =
                    create_fresh_root(&self.public, "reserve fresh public run directory")
                {
                    return match fs::remove_dir(&self.private) {
                        Ok(()) => Err(error),
                        Err(cleanup) if cleanup.kind() == io::ErrorKind::NotFound => Err(error),
                        Err(cleanup) => Err(UpstrokeError::Refused {
                            message: format!(
                                "{error}; could not remove the empty private reservation {}: {cleanup}",
                                self.private.display()
                            ),
                        }),
                    };
                }
                Ok(())
            },
        )?;
        for name in PUBLIC_DIRS {
            create_fresh_skeleton(&self.public.join(name), RunDirSite::CreatePublicDir, hooks)?;
        }
        for name in PRIVATE_DIRS {
            create_fresh_skeleton(
                &self.private.join(name),
                RunDirSite::CreatePrivateDir,
                hooks,
            )?;
        }
        Ok(())
    }

    /// The same creation, observed: `RunDir.CreatePublicDir` (P0) then
    /// `RunDir.CreatePrivateDir` (P2/P3), each followed by its skeleton.
    pub fn create_hooked(&self, hooks: &mut dyn RunDirHooks) -> Result<(), UpstrokeError> {
        create_public_dir(&self.public, hooks)?;
        for name in PUBLIC_DIRS {
            create_dir(&self.public.join(name))?;
        }
        create_private_dir(&self.private, hooks)?;
        for name in PRIVATE_DIRS {
            create_dir(&self.private.join(name))?;
        }
        Ok(())
    }

    /// The append-only source of truth (§15).
    pub fn events(&self) -> PathBuf {
        self.public.join(EVENT_LOG)
    }

    /// The frozen plan this run is executing (§5).
    pub fn plan_json(&self) -> PathBuf {
        self.public.join(PLAN)
    }

    /// A projection of the log for humans and tooling — derived, never read
    /// back as state.
    pub fn report_json(&self) -> PathBuf {
        self.public.join("report.json")
    }

    /// Held for the lifetime of a run so two engines cannot drive one branch.
    pub fn lock_file(&self) -> PathBuf {
        lock_file(&self.public)
    }

    pub fn questions(&self) -> PathBuf {
        self.public.join("questions")
    }

    /// Where `upstroke answer` drops an answer for the engine to ingest.
    pub fn answers(&self) -> PathBuf {
        self.public.join("answers")
    }

    pub fn artifacts(&self) -> PathBuf {
        self.public.join("artifacts")
    }

    pub fn transcripts(&self) -> PathBuf {
        self.private.join("transcripts")
    }

    pub fn reviews(&self) -> PathBuf {
        self.private.join("reviews")
    }

    pub fn settings(&self) -> PathBuf {
        self.private.join("settings")
    }

    pub fn gates(&self) -> PathBuf {
        self.private.join("gates")
    }

    /// Durable intents and disposable directories for exact gate/review
    /// worktrees. This lives outside the candidate workspace, so a hard-killed
    /// engine can reclaim Git registrations before a resumed worker runs.
    pub fn gate_worktrees(&self) -> PathBuf {
        self.private.join("gate-worktrees")
    }
}

/// `~/.upstroke`, or a temp-dir equivalent when no home resolves.
///
/// The fallback is deliberately still outside the workspace. Falling back to
/// the repo would keep runs working on a machine with no `HOME` while silently
/// dropping the isolation this module exists for — a security property that
/// degrades quietly is worse than one that was never claimed.
pub fn default_private_root() -> PathBuf {
    util::user_upstroke_dir().unwrap_or_else(|| std::env::temp_dir().join("upstroke"))
}

/// `<repo>/.upstroke/runs/<run-id>` — §15's documented location.
pub fn public_dir(repo_root: &Path, run_id: &str) -> PathBuf {
    runs_root(repo_root).join(run_id)
}

pub fn runs_root(repo_root: &Path) -> PathBuf {
    repo_root.join(".upstroke").join("runs")
}

// ===========================================================================
// The funnel
// ===========================================================================

/// What a run-directory or lock funnel tells whoever is watching.
///
/// `decisions.effect_site_inventory.identity`: "every effectful funnel API
/// takes its group's site by value, and the funnel itself calls `hook(Before,
/// site) -> primitive -> hook(After, site)`, so hooks exist for every site by
/// construction".
///
/// Production passes [`NoHooks`], which answers [`Injection::Proceed`] to
/// everything. A suite passes an observer that records what was reached and
/// answers with whatever it armed. [`HarnessHooks`] wires the same calls onto
/// PR3's [`HookHarness`] so the ST-07 bijection can read them.
///
/// The two phases are not decoration. `RunDirSite::sub_effects()` is empty for
/// every site in the frozen inventory, so `Before` and `After` are the only
/// coordinates a fault can be placed at — and for the publication sites they
/// are exactly the two the fault matrix names: `T-RUNSTART`'s "kill between
/// stage and rename" is a kill at `Before`, and its "a `PublishCommitRecord`
/// error after which the record is present" is an error returned at `After`,
/// which is what [`InjectionMode::ErrorReturn`](crate::topology::effects::InjectionMode)
/// means — "the funnel returns `Err` from that point **after** performing or
/// partially performing the primitive".
pub trait RunDirHooks {
    /// The funnel reached `phase` of `site`. The answer says what to do there.
    fn hook(&mut self, site: EffectSiteId, phase: HookPhase) -> Injection;

    /// Where this observer wants the funnel's durability primitives recorded.
    ///
    /// The sibling of [`crate::workspace_manager::EffectHooks::durability_ledger`]
    /// and of `events::log::EventHooks::synced`, and for the reason
    /// `PR5-RUNDIR-057` measured: `run_creation` specifies each of the three
    /// atomic publications here as "write `<name>.tmp`, **fsync**, rename,
    /// **fsync the directory**", and with no ledger the two fsyncs were not
    /// observables at all — the whole suite stayed green with the staged file's
    /// `sync_all` deleted, because an unsynced file parses exactly like a
    /// synced one.
    ///
    /// A *handle*, taken before the funnel body runs, because `funnel` holds
    /// `&mut dyn RunDirHooks` across the body. The default records nothing.
    fn durability_ledger(&self) -> DurabilityLedger {
        DurabilityLedger::off()
    }
}

/// What production passes: nothing is armed and nothing is recorded.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoHooks;

impl RunDirHooks for NoHooks {
    fn hook(&mut self, _site: EffectSiteId, _phase: HookPhase) -> Injection {
        Injection::Proceed
    }
}

/// Wires these funnels onto PR3's [`HookHarness`], the way
/// [`crate::runner::HarnessHooks`] wires the process funnel onto it.
#[derive(Debug, Clone, Default)]
pub struct HarnessHooks {
    harness: crate::observations::Exported,
    ledger: DurabilityLedger,
}

impl HarnessHooks {
    /// Observe through `harness`.
    #[must_use]
    pub fn new(harness: Arc<Mutex<HookHarness>>) -> Self {
        Self {
            harness: crate::observations::Exported::new(harness),
            ledger: DurabilityLedger::off(),
        }
    }

    /// The harness this observer records into.
    #[must_use]
    pub fn harness(&self) -> &Arc<Mutex<HookHarness>> {
        self.harness.harness()
    }

    /// Also record every durability primitive the funnels perform.
    #[must_use]
    pub fn recording_durability(mut self) -> Self {
        self.ledger = DurabilityLedger::recording();
        self
    }

    /// The durability ledger this observer records into.
    #[must_use]
    pub fn ledger(&self) -> DurabilityLedger {
        self.ledger.clone()
    }
}

impl RunDirHooks for HarnessHooks {
    fn durability_ledger(&self) -> DurabilityLedger {
        self.ledger.clone()
    }

    fn hook(&mut self, site: EffectSiteId, phase: HookPhase) -> Injection {
        self.harness.hook(site, phase)
    }
}

/// Do what a hook answered.
///
/// [`Injection::Kill`] aborts rather than panicking or exiting, for the same
/// reason [`crate::agent::proc`] aborts: the claim under test is what a
/// coordinator that runs **no** cleanup leaves behind, and both of the other
/// two run destructors.
fn apply(injection: Injection, site: EffectSiteId, phase: HookPhase) -> Result<(), UpstrokeError> {
    match injection {
        Injection::Proceed => Ok(()),
        Injection::Kill => std::process::abort(),
        Injection::Error => Err(UpstrokeError::Refused {
            message: format!("the run-directory funnel was made to fail at `{site}` ({phase})"),
        }),
    }
}

/// One effect, between its two hook phases.
///
/// An `Err` from the `After` phase is returned *after* the primitive ran, which
/// is the whole point of the error-return mode and the reason the commit
/// record's post-error helper has to stat rather than infer.
fn funnel<T>(
    hooks: &mut dyn RunDirHooks,
    site: EffectSiteId,
    primitive: impl FnOnce() -> Result<T, UpstrokeError>,
) -> Result<T, UpstrokeError> {
    apply(hooks.hook(site, HookPhase::Before), site, HookPhase::Before)?;
    let produced = primitive()?;
    apply(hooks.hook(site, HookPhase::After), site, HookPhase::After)?;
    Ok(produced)
}

// ---------------------------------------------------------------------------
// The names on disk
// ---------------------------------------------------------------------------

mod names;
pub use names::{
    COMMIT_RECORD, COMMIT_RECORD_STAGED, EVENT_LOG, MARKER, MARKER_STAGED, OWNER_RECORD,
    OWNER_RECORD_STAGED, PLAN, REPORT, REPORT_STAGING_PREFIX, REPORT_STAGING_RECORD,
    REPORT_STAGING_RECORD_STAGED,
};

// ---------------------------------------------------------------------------
// The records
// ---------------------------------------------------------------------------

/// `<public>/.creating` — what P1 publishes.
///
/// `workspace_candidates.run_creation`: "write `<public>/.creating.tmp` (JSON:
/// run_id, repo_key, private_dir = `<authorized private root>/runs/<run_id>` as
/// a canonical path, incarnation, pid, runner_policy_sha256)". Six fields, and
/// `deny_unknown_fields` because a marker is read back by a census that decides
/// whether a directory may be deleted: a field this build does not understand
/// is a marker this build must not act on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreatingMarker {
    pub run_id: String,
    pub repo_key: String,
    /// `<authorized private root>/runs/<run_id>`, canonical.
    pub private_dir: String,
    pub incarnation: String,
    pub pid: u32,
    pub runner_policy_sha256: String,
}

/// `<private>/owner.json` — what P3b publishes.
///
/// `run_creation`: "(JSON: run_id, repo_key, public_dir as the canonical path
/// of the public run directory, incarnation, runner: the full RunnerPolicy)".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnerRecord {
    pub run_id: String,
    pub repo_key: String,
    /// The canonical path of the public run directory.
    pub public_dir: String,
    pub incarnation: String,
    /// The full policy, not its digest: the marker carries the digest and the
    /// proof compares the two, so a record carrying only the digest could not
    /// be checked against anything.
    pub runner: RunnerPolicy,
}

/// `<private>/committed.json` — what P5b publishes.
///
/// `run_creation`: "{run_id, repo_key, public_dir, incarnation,
/// run_started_sha256 = the digest of the exact run_started line bytes about to
/// be appended}". Its presence is the one deletion boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommitRecord {
    pub run_id: String,
    pub repo_key: String,
    pub public_dir: String,
    pub incarnation: String,
    /// The digest of the exact `run_started` line bytes about to be appended.
    pub run_started_sha256: String,
}

/// This repository's identity, as the marker and both private records carry it.
///
/// `workspace_candidates.execution_root`: "repo_key v1 = hex16(sha256(
/// 'upstroke-repo-key-v1' NUL canonical common git dir bytes))". `hex16` is read
/// as sixteen hex characters — the first eight bytes of the digest — because
/// the same passage uses the key as a path component of the execution root and
/// a 64-character component is not what "key" describes there.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RepoKey(String);

impl RepoKey {
    /// The v1 key over a canonical common git dir.
    ///
    /// The path's bytes, not its display form: a path is bytes on Unix and
    /// `to_string_lossy` would map two distinguishable repositories onto one
    /// key exactly where a non-UTF-8 path makes the difference matter.
    #[must_use]
    pub fn v1(canonical_common_git_dir: &Path) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(b"upstroke-repo-key-v1");
        hasher.update([0u8]);
        hasher.update(canonical_common_git_dir.as_os_str().as_encoded_bytes());
        let digest = format!("{:x}", hasher.finalize());
        Self(digest[..16].to_owned())
    }

    /// The v1 key for a repository, from the git dir of one of its worktrees.
    ///
    /// A linked worktree's git dir is `<common>/worktrees/<name>` (Git's own
    /// layout), and every worktree of one repository must produce one key —
    /// otherwise a run created in the main checkout and a census run from a
    /// linked one would each call the other foreign. A main worktree's git dir
    /// is already the common one.
    pub fn for_worktree_git_dir(worktree_git_dir: &Path) -> Result<Self, UpstrokeError> {
        Ok(Self::v1(&canonical(common_git_dir(worktree_git_dir))?))
    }

    /// The v1 key for the repository `repo_root` is a worktree of.
    pub fn for_repo(repo_root: &Path) -> Result<Self, UpstrokeError> {
        let workspace = Workspace::open(repo_root)?;
        Self::for_worktree_git_dir(&workspace.worktree_git_dir()?)
    }

    /// The key as it is written into a marker or a record.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Read a key back off disk. No validation: a marker carrying a key this
    /// repository does not have is a mismatch to report, never a parse error.
    #[must_use]
    pub fn from_recorded(recorded: &str) -> Self {
        Self(recorded.to_owned())
    }
}

impl std::fmt::Display for RepoKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// The common git dir behind a worktree git dir.
fn common_git_dir(worktree_git_dir: &Path) -> PathBuf {
    let mut parts = worktree_git_dir.iter().rev();
    let last = parts.next();
    let penultimate = parts.next();
    match (last, penultimate) {
        // `<common>/worktrees/<name>` — two levels up is the common dir.
        (Some(_), Some(dir)) if dir == std::ffi::OsStr::new("worktrees") => worktree_git_dir
            .parent()
            .and_then(Path::parent)
            .map_or_else(|| worktree_git_dir.to_path_buf(), Path::to_path_buf),
        _ => worktree_git_dir.to_path_buf(),
    }
}

fn canonical(path: PathBuf) -> Result<PathBuf, UpstrokeError> {
    fs::canonicalize(&path).map_err(|source| UpstrokeError::Io { path, source })
}

// ---------------------------------------------------------------------------
// Atomic publication
// ---------------------------------------------------------------------------

/// Write JSON to `path` and make the bytes durable.
///
/// The staging half of every atomic publication here: `run_creation` says
/// "write `<name>.tmp`, fsync, rename, fsync the directory", and this is the
/// first two steps. `preserve` is what the staged file keeps of the file the
/// publication replaces, when that file's mode is the operator's to keep (the
/// report, see [`write_report`]): on Unix the file is created at the
/// preserved mode **less its group bits**, given the file's group before its
/// first byte where the process may ([`keep_group`]), and widened to the
/// preserved mode after ([`settle_mode`]); `None` leaves the mode the create
/// gave it. Until PR10's round 5 the mode was applied by a `set_permissions`
/// after the write, which left the bytes readable at the umask's mode until
/// then — by whoever opened the file in between, and for good after a death
/// in between (the round-5 regression lens, P2). The ledger's
/// [`DurableStep::Staged`] entry is taken from the file's own metadata the
/// moment it exists — before the group, before [`settle_mode`] and before
/// the first byte — and carries the creation mode and the real length, zero,
/// so a test holds the mode from creation rather than reading the source
/// order (until PR10's round 6 it was taken after `settle_mode` could chmod,
/// with a literal zero for the length: the round-6 fix-check and regression
/// lenses); the [`DurableStep::GroupGiven`] entry is taken at the group
/// transition, bracketing it ([`give_group`]).
fn stage_json<T: Serialize>(
    path: &Path,
    value: &T,
    ledger: &DurabilityLedger,
    preserve: Option<Preserved>,
) -> Result<(), UpstrokeError> {
    let mut json = serde_json::to_string_pretty(value).map_err(|error| UpstrokeError::Parse {
        message: format!("serializing {}: {error}", path.display()),
    })?;
    json.push('\n');
    let io = |source| UpstrokeError::Io {
        path: path.to_path_buf(),
        source,
    };
    let mut file = create_staged(path, preserve.as_ref()).map_err(io)?;
    let created = file.metadata().map_err(io)?;
    ledger.record(DurableStep::Staged, path, created.len());
    let preserve = keep_group(&file, &created, preserve, path, ledger).map_err(io)?;
    let after_write = settle_mode(&file, preserve).map_err(io)?;
    file.write_all(json.as_bytes()).map_err(io)?;
    if let Some(permissions) = after_write {
        file.set_permissions(permissions).map_err(io)?;
    }
    sync_file_recorded(&file, path, ledger)
}

/// What a rewritten file keeps of the file it replaces.
///
/// `fs::write` kept the inode, and with it everything the operator had set
/// on the file; the staged publication makes a fresh inode, so what is kept
/// is what this carries: the mode, and on Unix the group. The owner is not
/// carried — `chown` to another user is the superuser's, and the writer is
/// the owner of what it wrote — and on Windows an access-control list is
/// not carried either: the staged file takes the entries its directory
/// hands a new file, and an entry the operator placed on the report itself
/// is not copied (a `Permissions` there is the read-only bit alone).
struct Preserved {
    permissions: fs::Permissions,
    /// The group the existing file belongs to, given to the staged file
    /// before its first byte where the process may ([`keep_group`]).
    #[cfg(unix)]
    gid: u32,
}

impl Preserved {
    fn of(existing: &fs::Metadata) -> Self {
        Self {
            permissions: existing.permissions(),
            #[cfg(unix)]
            gid: {
                use std::os::unix::fs::MetadataExt as _;
                existing.gid()
            },
        }
    }

    /// The mode the staged file is created at: the preserved mode without
    /// its group bits, since the group has yet to be given, so that no
    /// instant exists at which the empty file is open to the writer's own
    /// group — a reader admitted at that instant would keep its descriptor
    /// through the write that follows. [`keep_group`] gives the group and
    /// [`settle_mode`] then widens to the preserved mode. Unix only: the
    /// other platforms' `Permissions` is the read-only bit, set after the
    /// write, and [`create_staged`] does not consult this there.
    #[cfg(unix)]
    fn creation_permissions(&self) -> fs::Permissions {
        use std::os::unix::fs::PermissionsExt as _;
        fs::Permissions::from_mode(self.permissions.mode() & 0o7707)
    }
}

/// Give the empty staged file the group the file it replaces belongs to,
/// before any byte reaches it, and hand back the permissions the write
/// then settles to.
///
/// The schema-3 `fs::write` kept the group because it kept the inode, and
/// an operator who had given `report.json` a group and a group-readable
/// mode had it replaced, since PR10's round 3, by a file in the writer's
/// primary group (the round-7 regression lens, P2): the intended readers
/// lost access and the writer's own group gained it. `fchown` gives the
/// staged file the existing group where the process may — the owner of the
/// file, for a group it is a member of. Where it may not (`EPERM`: a group
/// the operator chose that this process is not in, which no writer outside
/// the group could ever create a file in) the publication proceeds at the
/// writer's own group **with the preserved mode's group bits withheld**: the
/// operator's readers cannot be kept, and the alternative — the mode's
/// group bits handed to a group the operator did not name — would admit
/// readers nobody chose, so the file is published readable by its owner and
/// by whoever the other bits admit, and by no group (`0600` for a `0640`
/// report). Every other error is the caller's. The transition itself is
/// [`give_group`], which reads the mode off the descriptor in the statement
/// before the `fchown` and in the statement after it, and records both.
#[cfg(unix)]
fn keep_group(
    file: &File,
    created: &fs::Metadata,
    preserve: Option<Preserved>,
    path: &Path,
    ledger: &DurabilityLedger,
) -> std::io::Result<Option<fs::Permissions>> {
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
    let Some(kept) = preserve else {
        return Ok(None);
    };
    if created.gid() == kept.gid {
        return Ok(Some(kept.permissions));
    }
    match give_group(file, path, kept.gid, ledger) {
        Ok(()) => Ok(Some(kept.permissions)),
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => Ok(Some(
            fs::Permissions::from_mode(kept.permissions.mode() & 0o7707),
        )),
        Err(error) => Err(error),
    }
}

/// Give `file` the group `gid`, and record — as the ledger's
/// [`DurableStep::GroupGiven`] entry for `path` — the mode the file carried
/// at the instant before the `fchown` and the mode it carried at the instant
/// after it, each read by `fstat` on the descriptor itself, in the statement
/// immediately before the call and the statement immediately after it.
///
/// The property a test holds is that the group bits stay withheld until the
/// group is the operator's. Until PR10's round 8 the test read the creation
/// and the publication and nothing between; round 8 recorded the mode by a
/// `stat` of the path in the statement before the `fchown`, which a `chmod`
/// inserted *after* the record and before the syscall still slipped past
/// (the round-8 and round-9 fix-check lenses, P1). Two observations that
/// bracket the syscall leave that insertion nowhere to go: a widening before
/// the first `fstat` is in the first record, a widening between the two is
/// in the second, and the group bits the publication means to grant arrive
/// only afterwards, in [`settle_mode`]. What the record does not show is the
/// syscall's own duration — nothing a program observes can — and the test's
/// doc says so.
#[cfg(unix)]
fn give_group(
    file: &File,
    path: &Path,
    gid: u32,
    ledger: &DurabilityLedger,
) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    let before = file.metadata()?.permissions().mode() & 0o7777;
    let given = std::os::unix::fs::fchown(file, None, Some(gid));
    let after = file.metadata()?.permissions().mode() & 0o7777;
    ledger.record_transition(DurableStep::GroupGiven, path, Some(before), Some(after));
    given
}

#[cfg(not(unix))]
fn keep_group(
    _file: &File,
    _created: &fs::Metadata,
    preserve: Option<Preserved>,
    _path: &Path,
    _ledger: &DurabilityLedger,
) -> std::io::Result<Option<fs::Permissions>> {
    Ok(preserve.map(|kept| kept.permissions))
}

/// Open the staged file, empty, at the mode it will carry its bytes at — a
/// file of this writer's own making.
///
/// `create_new`, and nothing at the name is ever cleared first: the report is
/// staged inside a directory this write has just made for itself, exclusively
/// ([`REPORT_STAGING_PREFIX`], [`write_report`]), the staging record is
/// staged into the run's own private half ([`begin_report_staging`]), and the three run-creation
/// records are staged into a directory the same process created moments
/// before, under the creation's lock, so a file already at any of these names
/// is not this writer's and `create_new`'s `AlreadyExists` is the right
/// answer — a name that is somebody else's (a hard link to an operator's
/// file, a symlink, a directory, an operator's draft) is never truncated,
/// written through or removed. Until PR10's round 6 the path was opened
/// `create(true).truncate(true)`, which truncated an alias and filled the
/// operator's file with report bytes, on the schema-3 path too (the round-6
/// regression lens, P2-1); until round 8 a single-link regular file at the
/// fixed name `report.json.tmp` was removed as the writer's own, in round 8
/// a file wearing the shape `report.json.<ulid>.tmp` was, and in round 9 a
/// directory standing at the fixed `.report-staging` was, which
/// `standards/08` forbids each way (the round-8, round-9 and round-10
/// regression lenses, P2). On Unix the preserved mode —
/// less its group bits, until [`keep_group`] has given the file its group
/// ([`Preserved::creation_permissions`]) — is the `open`'s own creation
/// mode (`OpenOptions::mode`), so no instant exists at which the file is
/// wider than the file it replaces; the umask still narrows it, which
/// [`settle_mode`] corrects before the first byte. Elsewhere a `Permissions`
/// is the read-only bit, which is set after the write as it always was.
fn create_staged(path: &Path, preserve: Option<&Preserved>) -> std::io::Result<File> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    if let Some(kept) = preserve {
        use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
        options.mode(kept.creation_permissions().mode() & 0o7777);
    }
    #[cfg(not(unix))]
    let _ = preserve;
    options.open(path)
}

/// Give the empty staged file its preserved mode before any byte reaches it,
/// and hand back what is left to apply after the write.
///
/// On Unix that is nothing: the creation mode, which the umask may have
/// narrowed and which withheld the group bits until [`keep_group`] gave the
/// file its group, is widened to the preserved mode here — before the first
/// byte, and after the ledger has recorded the creation mode — and the
/// answer is `None`. On the other platforms the read-only bit is the whole
/// `Permissions`, no umask reads it, and it is set after the write as before,
/// so `preserve` is handed back for [`stage_json`] to apply then.
fn settle_mode(
    file: &File,
    preserve: Option<fs::Permissions>,
) -> std::io::Result<Option<fs::Permissions>> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        if let Some(permissions) = preserve {
            let created = file.metadata()?.permissions().mode() & 0o7777;
            if created != permissions.mode() & 0o7777 {
                file.set_permissions(permissions)?;
            }
        }
        Ok(None)
    }
    #[cfg(not(unix))]
    {
        let _ = file;
        Ok(preserve)
    }
}

/// fsync `file` and record what was made durable, in one call.
///
/// Fused for the reason `events::log::sync_log_file` gives: with the sync and
/// its ledger entry written as two statements, a mutation can be placed between
/// them. It does not close the residual boundary — deleting the `sync_all` line
/// *inside here* leaves the record and is undetectable on a machine that does
/// not lose power — and nothing here claims it does. What it does close is the
/// mutation the catalogue actually names: removing the durability step from a
/// publication sequence, which now removes its evidence too.
fn sync_file_recorded(
    file: &File,
    path: &Path,
    ledger: &DurabilityLedger,
) -> Result<(), UpstrokeError> {
    let io = |source| UpstrokeError::Io {
        path: path.to_path_buf(),
        source,
    };
    let outcome = util::fsync_file_at(file, path);
    let len = file.metadata().map(|meta| meta.len()).unwrap_or(0);
    ledger.record(DurableStep::SyncedFile, path, len);
    outcome.map_err(io)
}

/// Rename the staged file onto its published name and make the directory entry
/// durable.
fn publish(
    staged: &Path,
    published: &Path,
    ledger: &DurabilityLedger,
) -> Result<(), UpstrokeError> {
    fs::rename(staged, published).map_err(|source| UpstrokeError::Io {
        path: published.to_path_buf(),
        source,
    })?;
    ledger.record(
        DurableStep::Renamed,
        published,
        fs::metadata(published).map(|meta| meta.len()).unwrap_or(0),
    );
    match published.parent() {
        Some(dir) => sync_dir(dir, ledger),
        None => Ok(()),
    }
}

/// [`publish`] for the report, whose staged file lives in a directory of the
/// write's own: the rename up onto the published name, then the staging
/// directory — empty now — removed, then the one directory barrier that makes
/// both public changes durable, and only then the record that named the
/// directory. The record outlives the deletion it names: a barrier refused
/// after the rename (the fresh-branch tests' shape: the report visible under
/// its name and not yet durable) leaves the record naming a directory that is
/// gone, which a loss may restore and which the next write reclaims by that
/// record ([`reclaim_report_staging`]); until the fix of
/// `PR10-RECLAIM-RECORD-DROPPED-BEFORE-DURABLE-DELETION` the record went before
/// the barrier, and a loss after a refused barrier restored a staging
/// directory no record named. The record's removal is not synced on its own: a
/// record the disk restores names a directory whose deletion is durable, which
/// the next write removes on its own.
fn publish_report(
    staged: &Path,
    published: &Path,
    staging: &Path,
    record: &Path,
    ledger: &DurabilityLedger,
) -> Result<(), UpstrokeError> {
    fs::rename(staged, published).map_err(|source| UpstrokeError::Io {
        path: published.to_path_buf(),
        source,
    })?;
    ledger.record(
        DurableStep::Renamed,
        published,
        fs::metadata(published).map(|meta| meta.len()).unwrap_or(0),
    );
    fs::remove_dir(staging).map_err(|source| UpstrokeError::Io {
        path: staging.to_path_buf(),
        source,
    })?;
    if let Some(dir) = published.parent() {
        sync_dir(dir, ledger)?;
    }
    fs::remove_file(record).map_err(|source| UpstrokeError::Io {
        path: record.to_path_buf(),
        source,
    })
}

/// fsync a directory, on every platform (`PR5-CONF-013`).
///
/// This was Unix-only, with a comment saying Windows had no directory handle a
/// program could `FlushFileBuffers` "without `FILE_FLAG_BACKUP_SEMANTICS` and a
/// raw handle". That is the *recipe*, not an obstacle, and `run_creation`'s
/// "fsync the directory" carries no platform exception — so
/// [`crate::util::fsync_dir`] performs it and this function stays what it always
/// was: the site's ledger entry beside the barrier.
fn sync_dir(dir: &Path, ledger: &DurabilityLedger) -> Result<(), UpstrokeError> {
    util::fsync_dir(dir).map_err(|source| UpstrokeError::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    ledger.record(DurableStep::SyncedDirectory, dir, 0);
    Ok(())
}

/// [`sync_dir`] taken to make one entry's removal durable, with that entry
/// observed: whether `entry` is present is read by `symlink_metadata` in the
/// statement immediately before the barrier and carried on the ledger's
/// [`DurableStep::SyncedDirectory`] record as the entry observed
/// ([`util::EntryObserved`]), as `workspace_manager::sync_checkout_removed`
/// carries the checkout's. A record carrying the entry observed absent is the
/// proof that the barrier followed the removal; a record of the directory
/// alone is written just the same by a barrier taken before it (Gate 5's
/// review round 3, on the power-loss witness of
/// `PR10-RECLAIM-RECORD-DROPPED-BEFORE-DURABLE-DELETION`). As with
/// [`sync_dir`], the record is written once the barrier has returned, so a
/// record is a barrier that held.
///
/// # Errors
///
/// An I/O error reading `entry` other than its absence, or the barrier's I/O
/// error naming the directory.
fn sync_dir_observing(
    dir: &Path,
    entry: &Path,
    ledger: &DurabilityLedger,
) -> Result<(), UpstrokeError> {
    let present = match fs::symlink_metadata(entry) {
        Ok(_) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(source) => {
            return Err(UpstrokeError::Io {
                path: entry.to_path_buf(),
                source,
            });
        }
    };
    util::fsync_dir(dir).map_err(|source| UpstrokeError::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    ledger.record_entry(
        DurableStep::SyncedDirectory,
        dir,
        util::EntryObserved {
            path: entry.to_path_buf(),
            present,
        },
    );
    Ok(())
}

/// Create a directory and everything above it.
fn create_dir(dir: &Path) -> Result<(), UpstrokeError> {
    fs::create_dir_all(dir).map_err(|source| UpstrokeError::Io {
        path: dir.to_path_buf(),
        source,
    })
}

/// Reserve a fresh run root without accepting an existing directory or link.
/// Only the parents are created recursively. The final component is exclusive.
fn create_fresh_root(path: &Path, operation: &'static str) -> Result<(), UpstrokeError> {
    let Some(parent) = path.parent() else {
        return Err(UpstrokeError::Refused {
            message: format!("cannot {operation} {} without a parent", path.display()),
        });
    };
    fs::create_dir_all(parent).map_err(|source| UpstrokeError::Filesystem {
        operation: "create fresh run directory parents",
        path: parent.to_path_buf(),
        source,
    })?;
    fs::create_dir(path).map_err(|source| UpstrokeError::Filesystem {
        operation,
        path: path.to_path_buf(),
        source,
    })
}

fn create_fresh_skeleton(
    path: &Path,
    site: RunDirSite,
    hooks: &mut dyn RunDirHooks,
) -> Result<(), UpstrokeError> {
    funnel(hooks, EffectSiteId::RunDir(site), || {
        fs::create_dir_all(path).map_err(|source| UpstrokeError::Filesystem {
            operation: "create fresh run skeleton directory",
            path: path.to_path_buf(),
            source,
        })
    })
}

// ---------------------------------------------------------------------------
// The run-directory funnels
// ---------------------------------------------------------------------------

/// P0 — `RunDir.CreatePublicDir`.
pub fn create_public_dir(public: &Path, hooks: &mut dyn RunDirHooks) -> Result<(), UpstrokeError> {
    funnel(
        hooks,
        EffectSiteId::RunDir(RunDirSite::CreatePublicDir),
        || create_dir(public),
    )
}

/// P1a — `RunDir.StageMarker`. Writes `<public>/.creating.tmp` and syncs it.
pub fn stage_marker(
    public: &Path,
    marker: &CreatingMarker,
    hooks: &mut dyn RunDirHooks,
) -> Result<(), UpstrokeError> {
    let ledger = hooks.durability_ledger();
    funnel(hooks, EffectSiteId::RunDir(RunDirSite::StageMarker), || {
        stage_json(&public.join(MARKER_STAGED), marker, &ledger, None)
    })
}

/// P1b — `RunDir.PublishMarker`. The atomic rename onto `<public>/.creating`.
pub fn publish_marker(public: &Path, hooks: &mut dyn RunDirHooks) -> Result<(), UpstrokeError> {
    let ledger = hooks.durability_ledger();
    funnel(
        hooks,
        EffectSiteId::RunDir(RunDirSite::PublishMarker),
        || publish(&public.join(MARKER_STAGED), &public.join(MARKER), &ledger),
    )
}

/// P7 — `RunDir.RemoveMarker`, once `run_started` is durable.
///
/// Idempotent: a census and the owning resume both remove a stale marker, and
/// `resource_accounting` has a stale marker removed "by a census with the lock
/// free **or** by its owner on resume".
pub fn remove_marker(public: &Path, hooks: &mut dyn RunDirHooks) -> Result<(), UpstrokeError> {
    funnel(
        hooks,
        EffectSiteId::RunDir(RunDirSite::RemoveMarker),
        || {
            remove_file_if_present(&public.join(MARKER))?;
            remove_file_if_present(&public.join(MARKER_STAGED))
        },
    )
}

/// P2/P3 — `RunDir.CreatePrivateDir`.
pub fn create_private_dir(
    private: &Path,
    hooks: &mut dyn RunDirHooks,
) -> Result<(), UpstrokeError> {
    funnel(
        hooks,
        EffectSiteId::RunDir(RunDirSite::CreatePrivateDir),
        || create_dir(private),
    )
}

/// The private skeleton directories, after the owner record.
///
/// `run_creation`: the owner record is published "before any other private
/// content (`RunDir.StageOwnerRecord`, `RunDir.PublishOwnerRecord`), **then the
/// private skeleton directories**" — which is why this is a separate call and
/// not part of [`create_private_dir`]. [`RunPaths::create_hooked`] creates them
/// *before* the record, which is the ordering O08 refuses and the reason the
/// schema-4 creator does not use it.
///
/// Each directory goes through [`create_private_dir`], because each one **is** a
/// private directory creation and `RunDir.CreatePrivateDir` is the site that
/// owns that effect. The pre-move loop used the bare helper and so created five
/// directories under no site at all.
///
/// # Errors
///
/// Whatever the funnel returns for the first directory that fails.
pub fn create_private_skeleton(
    private: &Path,
    hooks: &mut dyn RunDirHooks,
) -> Result<(), UpstrokeError> {
    for name in PRIVATE_DIRS {
        create_private_dir(&private.join(name), hooks)?;
    }
    Ok(())
}

/// P3a — `RunDir.StageOwnerRecord`.
pub fn stage_owner_record(
    private: &Path,
    owner: &OwnerRecord,
    hooks: &mut dyn RunDirHooks,
) -> Result<(), UpstrokeError> {
    let ledger = hooks.durability_ledger();
    funnel(
        hooks,
        EffectSiteId::RunDir(RunDirSite::StageOwnerRecord),
        || stage_json(&private.join(OWNER_RECORD_STAGED), owner, &ledger, None),
    )
}

/// P3b — `RunDir.PublishOwnerRecord`, before any other private content.
pub fn publish_owner_record(
    private: &Path,
    hooks: &mut dyn RunDirHooks,
) -> Result<(), UpstrokeError> {
    let ledger = hooks.durability_ledger();
    funnel(
        hooks,
        EffectSiteId::RunDir(RunDirSite::PublishOwnerRecord),
        || {
            publish(
                &private.join(OWNER_RECORD_STAGED),
                &private.join(OWNER_RECORD),
                &ledger,
            )
        },
    )
}

/// P5a — `RunDir.StageCommitRecord`.
pub fn stage_commit_record(
    private: &Path,
    record: &CommitRecord,
    hooks: &mut dyn RunDirHooks,
) -> Result<(), UpstrokeError> {
    let ledger = hooks.durability_ledger();
    funnel(
        hooks,
        EffectSiteId::RunDir(RunDirSite::StageCommitRecord),
        || stage_json(&private.join(COMMIT_RECORD_STAGED), record, &ledger, None),
    )
}

/// P5b — `RunDir.PublishCommitRecord`, the one deletion boundary.
///
/// `effect_site_inventory.identity`: "after this site returns, or when a
/// read-only stat after its error shows the record present, no path — creator
/// or census — deletes the private half". The stat is
/// [`commit_record_after_error`], and it is a separate call precisely because
/// an error here does not say which side of the boundary the run is on.
pub fn publish_commit_record(
    private: &Path,
    hooks: &mut dyn RunDirHooks,
) -> Result<(), UpstrokeError> {
    let ledger = hooks.durability_ledger();
    funnel(
        hooks,
        EffectSiteId::RunDir(RunDirSite::PublishCommitRecord),
        || {
            publish(
                &private.join(COMMIT_RECORD_STAGED),
                &private.join(COMMIT_RECORD),
                &ledger,
            )
        },
    )
}

/// Which side of the deletion boundary an errored publication left the run on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommitRecordPresence {
    /// The record is on disk. Nothing deletes either half from here on.
    Present,
    /// It is not. The creator, which holds both locks and knows the run never
    /// committed, may remove both halves.
    Absent,
    /// The filesystem would not say. Not an answer, and treated as `Present`
    /// by every caller, because the cost of being wrong is asymmetric: a
    /// retained husk is reported until an operator prunes it, and a deleted
    /// committed run is gone.
    Unknown(String),
}

impl CommitRecordPresence {
    /// Whether deletion is still permitted. `Unknown` is not.
    #[must_use]
    pub const fn permits_deletion(&self) -> bool {
        matches!(self, Self::Absent)
    }
}

/// The read-only stat after a staging or publication error.
///
/// It **stats**. It does not read the error: `run_creation` distinguishes "a
/// P5b error after which the record is absent" from "a P5b error after which
/// the record is present", and the error is the same value in both cases —
/// the funnel's error-return mode returns `Err` *after* performing the rename.
/// Inferring absence from an error would delete a private half that had
/// already crossed the boundary.
#[must_use]
pub fn commit_record_after_error(private: &Path) -> CommitRecordPresence {
    let path = private.join(COMMIT_RECORD);
    match fs::symlink_metadata(&path) {
        Ok(_) => CommitRecordPresence::Present,
        Err(error) if error.kind() == io::ErrorKind::NotFound => CommitRecordPresence::Absent,
        Err(error) => CommitRecordPresence::Unknown(format!("{}: {error}", path.display())),
    }
}

/// P5 — `RunDir.WritePlan`.
pub fn write_plan(
    public: &Path,
    normalized: &[u8],
    hooks: &mut dyn RunDirHooks,
) -> Result<(), UpstrokeError> {
    funnel(hooks, EffectSiteId::RunDir(RunDirSite::WritePlan), || {
        let path = public.join(PLAN);
        fs::write(&path, normalized).map_err(|source| UpstrokeError::Io { path, source })
    })
}

/// `RunDir.WriteReport` — the derived projection, never read back as state.
///
/// Published the way every other record of the run directory is: staged as
/// `report.json` inside a directory this write makes for itself under a name
/// unique to the write (`<public>/.report-staging-<ulid>/`,
/// [`REPORT_STAGING_PREFIX`], created exclusively), synced, renamed up onto
/// `report.json`, the emptied staging directory removed, the run directory
/// synced ([`publish_report`]). DESIGN.md §26 lets the refs a finalization
/// prunes go only "after the report is durable", and a resume that finds the
/// report current prunes without writing it again — so a report present under
/// its name has to hold durable bytes by construction, which the rename after
/// the sync gives it: a rename that survives a power loss was made after its
/// file's bytes were, and one that does not survive leaves no report, which
/// the next finalization regenerates. The *name* is durable only once the
/// directory's barrier has completed, which is why a finalization that finds
/// the report current still takes [`sync_report_dir`] before it prunes.
/// Written with a plain `std::fs::write` until PR10's round 3, which synced
/// nothing (the crash lens, P2).
///
/// The v0.1 coordinator publishes through here too (`drain_and_report`,
/// schema 3): the bytes are the ones `fs::write` wrote; the mode is the
/// existing report's, and so is its group where the process may give it —
/// the staged file is created at the preserved mode less its group bits,
/// given the group before its first byte, and widened to the preserved mode
/// then ([`Preserved`], [`keep_group`]); where the process is not in that
/// group (`EPERM`) the report is published at the writer's group with the
/// group bits withheld, `0600` for a `0640` report — since `fs::write`
/// truncated in place and kept mode, group and every other property of the
/// inode, while a fresh file takes the umask's mode and the writer's group
/// (the round-4 regression lens, P2-1, and the round-7 one, P2;
/// `the_payload_writers_keep_their_exact_legacy_bytes` holds the bytes and
/// the mode, `rewriting_report_preserves_its_group` the group and the
/// creation without group bits). What the inode carried beyond mode and
/// group — a POSIX ACL — is not carried (`PR10-LEGACY-REPORT-REWRITE-DROPS-ITS-POSIX-ACL`).
///
/// **Which staging directory is this run's is proven by a record, never read
/// off a name** (`standards/08`; the round-10 lenses, P2, after rounds 8 and 9
/// had removed by the shape of a name and then by a fixed name's type). Before
/// the directory is made, the write records its name durably in the run's
/// private half — `<private>/report-staging.json`
/// ([`REPORT_STAGING_RECORD`]; staged, synced, renamed, the private directory
/// synced, [`begin_report_staging`]) — and the ledger's entry for the
/// directory's creation carries, read in the statement immediately before
/// `create_dir`, whether that record stood. What a writer that died between
/// its stage and its rename leaves is the record and the directory it names,
/// with the staged file inside, and the next write — here, and on the fresh
/// branch of a terminal finalization ([`sync_report_dir`]) — removes the
/// directory the record names as the run's own tree, by the record, and, once
/// the public directory's barrier has made that removal durable, the record
/// ([`reclaim_report_staging`]); a record naming a directory that is gone is
/// itself removed, after the same barrier. `remove_dir_all` on that directory is a
/// deletion the record authorises, the way the private half's token
/// authorises the private half's: the record is the run's own writing in the
/// run's own tree, made before the name existed. A directory of the staging
/// shape that no record names — an operator's, or a writer's from before the
/// record existed — is not the protocol's: it is left as found, named in
/// what this function returns so the caller can say so, never removed and
/// never adopted, and the write proceeds under its own fresh name. Nothing at
/// any other name under the run directory is opened, removed or reasoned
/// about: an operator's draft at `report.json.tmp`, at
/// `report.json.ZZZZZZZZZZZZZZZZZZZZZZZZZZ.tmp`, at a name the ULID producer
/// could have emitted, or a whole `.report-staging/` of the operator's,
/// survives the write byte for byte.
///
/// # Errors
///
/// An injected error at either phase of either site; the record's or the
/// staged file's I/O errors; a record in the private half that does not parse
/// or names something that is not a staging directory's name (the run's own
/// record, corrupted: the write refuses rather than guess).
///
/// # Returns
///
/// The staging-shaped entries under `public` that no record of this run's
/// names, passed over and left as found — empty in the ordinary case.
pub fn write_report<T: Serialize>(
    public: &Path,
    private: &Path,
    report: &T,
    hooks: &mut dyn RunDirHooks,
) -> Result<Vec<PathBuf>, UpstrokeError> {
    let run_dir = EffectSiteId::RunDir(RunDirSite::WriteReport);
    let report_site = EffectSiteId::Report(ReportSite::Write);
    let ledger = hooks.durability_ledger();
    apply(
        hooks.hook(run_dir, HookPhase::Before),
        run_dir,
        HookPhase::Before,
    )?;
    apply(
        hooks.hook(report_site, HookPhase::Before),
        report_site,
        HookPhase::Before,
    )?;
    let published = public.join(REPORT);
    let preserved = match fs::metadata(&published) {
        Ok(existing) => Some(Preserved::of(&existing)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(source) => {
            return Err(UpstrokeError::Io {
                path: published,
                source,
            });
        }
    };
    let passed_over = reclaim_report_staging(public, private, &ledger)?;
    let staging = begin_report_staging(public, private, &ledger)?;
    let staged = staging.join(REPORT);
    stage_json(&staged, report, &ledger, preserved)?;
    publish_report(
        &staged,
        &published,
        &staging,
        &report_staging_record(private),
        &ledger,
    )?;
    apply(
        hooks.hook(report_site, HookPhase::After),
        report_site,
        HookPhase::After,
    )?;
    apply(
        hooks.hook(run_dir, HookPhase::After),
        run_dir,
        HookPhase::After,
    )?;
    Ok(passed_over)
}

/// The directory barrier of `Report.Write`, on its own.
///
/// For the finalization that finds `report.json` current and writes nothing:
/// the rename that put the report under its name was made after its bytes
/// were synced, so the bytes are durable, but the *name* is durable only once
/// the directory's barrier completed — and a resume that finds the report
/// runs after a first finalization that may have died, or failed, between the
/// rename and that barrier. DESIGN.md §26 prunes only after a durable report,
/// so the fresh branch takes this before its first pruning effect (the
/// round-4 crash lens, P2). Through the report's own two sites, exactly as
/// [`write_report`] is — `RunDir.WriteReport` and then `Report.Write` around
/// the barrier, which is the funnel's whole primitive on this branch — so a
/// fault matrix can select the coordinate on a restart and the typed
/// inventory accounts for every external effect a finalization makes; until
/// PR10's round 5 it took the hooks for their ledger alone and reached no
/// site (the round-5 contract lens, F1). The hooks' [`DurabilityLedger`]
/// records the directory synced, as before.
///
/// What a dead report writer left is reclaimed here as in [`write_report`]
/// ([`reclaim_report_staging`]): the directory the record in `private` names,
/// the public directory's barrier that makes its removal durable, and only
/// then the record — all before this branch's own barrier. A staging-shaped
/// directory no record names is left as found and returned, as there.
///
/// # Errors
///
/// An injected error at either phase of either site, the reclaim's error, or
/// the barrier's I/O error naming the directory.
///
/// # Returns
///
/// The staging-shaped entries under `public` that no record of this run's
/// names, passed over and left as found.
pub fn sync_report_dir(
    public: &Path,
    private: &Path,
    hooks: &mut dyn RunDirHooks,
) -> Result<Vec<PathBuf>, UpstrokeError> {
    let run_dir = EffectSiteId::RunDir(RunDirSite::WriteReport);
    let report_site = EffectSiteId::Report(ReportSite::Write);
    let ledger = hooks.durability_ledger();
    apply(
        hooks.hook(run_dir, HookPhase::Before),
        run_dir,
        HookPhase::Before,
    )?;
    apply(
        hooks.hook(report_site, HookPhase::Before),
        report_site,
        HookPhase::Before,
    )?;
    let passed_over = reclaim_report_staging(public, private, &ledger)?;
    sync_dir(public, &ledger)?;
    apply(
        hooks.hook(report_site, HookPhase::After),
        report_site,
        HookPhase::After,
    )?;
    apply(
        hooks.hook(run_dir, HookPhase::After),
        run_dir,
        HookPhase::After,
    )?;
    Ok(passed_over)
}

/// The record of the staging directory a report write makes for itself:
/// `<private>/report-staging.json` ([`REPORT_STAGING_RECORD`]).
#[must_use]
pub fn report_staging_record(private: &Path) -> PathBuf {
    private.join(REPORT_STAGING_RECORD)
}

/// What the record in the private half says: the name, under `public`, of
/// the staging directory the write that wrote it made or was about to make.
#[derive(Debug, Serialize, Deserialize)]
struct ReportStagingRecord {
    directory: String,
}

/// The staging directory the record in `private` names, when a record stands:
/// `<public>/<name>`, the name checked to be one this protocol produces
/// ([`REPORT_STAGING_PREFIX`] and a ULID, one path component) — the run's own
/// record, and a record that says anything else is refused rather than
/// followed. `None` when no record stands.
///
/// # Errors
///
/// An I/O error reading the record, a record that does not parse, or a
/// record whose name is not a staging directory's.
pub fn recorded_report_staging(
    public: &Path,
    private: &Path,
) -> Result<Option<PathBuf>, UpstrokeError> {
    let record = report_staging_record(private);
    let bytes = match fs::read(&record) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(UpstrokeError::Io {
                path: record,
                source,
            });
        }
    };
    let parsed: ReportStagingRecord =
        serde_json::from_slice(&bytes).map_err(|error| UpstrokeError::Parse {
            message: format!(
                "{} is the run's record of its report staging directory and does not parse: \
                 {error}",
                record.display()
            ),
        })?;
    let name = parsed.directory;
    let ulid = name.strip_prefix(REPORT_STAGING_PREFIX).unwrap_or("");
    if ulid.len() != 26 || !ulid.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
        return Err(UpstrokeError::Refused {
            message: format!(
                "{} is the run's record of its report staging directory and names `{name}`, \
                 which is not a name this protocol produces; nothing is removed",
                record.display()
            ),
        });
    }
    Ok(Some(public.join(name)))
}

/// What the report's protocol has left staged: the record's own staging file
/// if a dead writer left one, the record, the directory the record names when
/// a directory stands there, and its entries in name order — what a writer
/// that died between its stage and its rename leaves, and what the next write
/// removes before it stages ([`reclaim_report_staging`]). Empty when no
/// record stands. A staging-shaped directory no record names is not the
/// protocol's and is not listed here ([`unrecorded_report_staging`]).
///
/// # Errors
///
/// An I/O error reading the record or the directory; only an actual not-found
/// is an empty answer (§7).
pub fn report_staging_leftovers(
    public: &Path,
    private: &Path,
) -> Result<Vec<PathBuf>, UpstrokeError> {
    let mut found = Vec::new();
    let staged_record = private.join(REPORT_STAGING_RECORD_STAGED);
    if fs::symlink_metadata(&staged_record).is_ok() {
        found.push(staged_record);
    }
    let Some(recorded) = recorded_report_staging(public, private)? else {
        return Ok(found);
    };
    found.push(report_staging_record(private));
    let metadata = match fs::symlink_metadata(&recorded) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(found),
        Err(source) => {
            return Err(UpstrokeError::Io {
                path: recorded,
                source,
            });
        }
    };
    if !metadata.file_type().is_dir() {
        return Ok(found);
    }
    let entries = fs::read_dir(&recorded).map_err(|source| UpstrokeError::Io {
        path: recorded.clone(),
        source,
    })?;
    let mut inside = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| UpstrokeError::Io {
            path: recorded.clone(),
            source,
        })?;
        inside.push(entry.path());
    }
    inside.sort();
    found.push(recorded);
    found.extend(inside);
    Ok(found)
}

/// The staging-shaped entries under `public` that no record of this run's
/// names — every entry whose name begins with [`REPORT_STAGING_PREFIX`],
/// whatever its type, other than the one the record in `private` names — in
/// name order. Not the protocol's: the write passes them over, left as found,
/// and names them.
///
/// # Errors
///
/// An I/O error listing `public` or reading the record.
pub fn unrecorded_report_staging(
    public: &Path,
    private: &Path,
) -> Result<Vec<PathBuf>, UpstrokeError> {
    let recorded = recorded_report_staging(public, private)?;
    let entries = fs::read_dir(public).map_err(|source| UpstrokeError::Io {
        path: public.to_path_buf(),
        source,
    })?;
    let mut found = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| UpstrokeError::Io {
            path: public.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let shaped = entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.starts_with(REPORT_STAGING_PREFIX));
        if shaped && recorded.as_deref() != Some(path.as_path()) {
            found.push(path);
        }
    }
    found.sort();
    Ok(found)
}

/// Remove what a dead report writer left, by its record and by nothing else:
/// the directory the record in `private` names, when a directory stands
/// there, as the run's own tree — the record is the run's own writing, made
/// before the name existed — then the public directory's barrier, which makes
/// that deletion durable, and only then the record; a record whose directory is
/// gone is removed on its own, after the same barrier, since an absent name is
/// not proof its deletion reached one; something that is not a directory
/// standing at the recorded name is not what this writer made and is left as
/// found, the record removed after the same barrier. Then the staging-shaped
/// entries no record names are listed, for the caller to name: passed over,
/// never removed, never adopted. The barrier goes between the two removals
/// because the two halves need not share a filesystem: a record removed ahead
/// of it is lost to a power loss that restores the directory it named — and the
/// write that follows publishes its own record over the name — leaving a
/// directory of the run's own making that no record names and every later
/// write passes over (`PR10-RECLAIM-RECORD-DROPPED-BEFORE-DURABLE-DELETION`).
/// The barrier carries the recorded name as it found it
/// ([`sync_dir_observing`]), so its ledger record shows whether it followed
/// the removal.
///
/// # Errors
///
/// An I/O error reading the record, removing the tree or the record, taking
/// the public directory's barrier, or listing `public`.
fn reclaim_report_staging(
    public: &Path,
    private: &Path,
    ledger: &DurabilityLedger,
) -> Result<Vec<PathBuf>, UpstrokeError> {
    if let Some(recorded) = recorded_report_staging(public, private)? {
        match fs::symlink_metadata(&recorded) {
            Ok(metadata) if metadata.file_type().is_dir() => {
                fs::remove_dir_all(&recorded).map_err(|source| UpstrokeError::Io {
                    path: recorded.clone(),
                    source,
                })?;
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(UpstrokeError::Io {
                    path: recorded,
                    source,
                });
            }
        }
        sync_dir_observing(public, &recorded, ledger)?;
        remove_file_if_present(&report_staging_record(private))?;
    }
    unrecorded_report_staging(public, private)
}

/// Record the name of the staging directory this write is about to make,
/// durably, in the run's private half, and then make the directory —
/// exclusively — under `public`; the answer is the directory's path.
///
/// The record goes first: staged at [`REPORT_STAGING_RECORD_STAGED`] (a
/// staged record a dead writer left is removed first — the private half's
/// own), synced, renamed onto [`REPORT_STAGING_RECORD`], the private
/// directory synced; then, in the statement immediately before `create_dir`,
/// whether the record stands is read, and the ledger's
/// [`DurableStep::DirectoryCreated`] entry for the directory carries it as
/// the entry observed ([`util::EntryObserved`]), so a creation ahead of its
/// record shows on the record as `present: false`
/// (`a_report_staging_directory_is_recorded_before_it_is_made`; the recipe
/// `staging-created-before-its-record`).
///
/// # Errors
///
/// The record's I/O errors, or `create_dir`'s.
fn begin_report_staging(
    public: &Path,
    private: &Path,
    ledger: &DurabilityLedger,
) -> Result<PathBuf, UpstrokeError> {
    let name = format!("{REPORT_STAGING_PREFIX}{}", crate::ulid::ulid());
    let staging = public.join(&name);
    let record = report_staging_record(private);
    let staged_record = private.join(REPORT_STAGING_RECORD_STAGED);
    remove_file_if_present(&staged_record)?;
    stage_json(
        &staged_record,
        &ReportStagingRecord { directory: name },
        ledger,
        None,
    )?;
    publish(&staged_record, &record, ledger)?;
    let recorded = match fs::symlink_metadata(&record) {
        Ok(metadata) => metadata.file_type().is_file(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(source) => {
            return Err(UpstrokeError::Io {
                path: record,
                source,
            });
        }
    };
    let made = fs::create_dir(&staging);
    ledger.record_entry(
        DurableStep::DirectoryCreated,
        &staging,
        util::EntryObserved {
            path: record,
            present: recorded,
        },
    );
    made.map_err(|source| UpstrokeError::Io {
        path: staging.clone(),
        source,
    })?;
    Ok(staging)
}

/// `RunDir.WriteQuestionPayload` — written before the question is announced.
pub fn write_question_payload<T: Serialize>(
    questions: &Path,
    component: &str,
    payload: &T,
    hooks: &mut dyn RunDirHooks,
) -> Result<(), UpstrokeError> {
    funnel(
        hooks,
        EffectSiteId::RunDir(RunDirSite::WriteQuestionPayload),
        || util::write_json(&questions.join(format!("{component}.json")), payload),
    )
}

/// `RunDir.RemovePublicHusk` — the public half, with the marker last.
///
/// `startup_census`: "then the public directory is removed with the marker
/// last … so a kill mid-census leaves a husk the next census completes".
/// Removing the marker first would leave a marker-less husk carrying content,
/// which the next census retains rather than finishes.
pub fn remove_public_husk(public: &Path, hooks: &mut dyn RunDirHooks) -> Result<(), UpstrokeError> {
    funnel(
        hooks,
        EffectSiteId::RunDir(RunDirSite::RemovePublicHusk),
        || {
            for entry in read_dir_names(public) {
                if entry == MARKER {
                    continue;
                }
                let path = public.join(&entry);
                let removed = if path.is_dir() {
                    fs::remove_dir_all(&path)
                } else {
                    fs::remove_file(&path)
                };
                removed.map_err(|source| UpstrokeError::Io { path, source })?;
            }
            remove_file_if_present(&public.join(MARKER))?;
            fs::remove_dir(public).map_err(|source| UpstrokeError::Io {
                path: public.to_path_buf(),
                source,
            })
        },
    )
}

/// `RunDir.RemovePrivateHusk` — and the only way to reach it is a token.
///
/// `resource_accounting.completeness_rule`: "a private-half deletion outside
/// the proof-token funnel fails to compile". The token is taken **by value**,
/// so it is spent here and cannot authorise a second deletion, and
/// [`PrivateHalfProof`] has no other constructor, no `Clone`, no `Copy` and no
/// `Default` — see [`ownership`].
pub fn remove_private_husk(
    proof: PrivateHalfProof,
    hooks: &mut dyn RunDirHooks,
) -> Result<(), UpstrokeError> {
    funnel(
        hooks,
        EffectSiteId::RunDir(RunDirSite::RemovePrivateHusk),
        || {
            let target = proof.target();
            fs::remove_dir_all(target).map_err(|source| UpstrokeError::Io {
                path: target.to_path_buf(),
                source,
            })
        },
    )
}

fn remove_file_if_present(path: &Path) -> Result<(), UpstrokeError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(UpstrokeError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn read_dir_names(dir: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

// ---------------------------------------------------------------------------
// The answer funnels
// ---------------------------------------------------------------------------

/// `Answer.StageWrite` — `answers/<qid>.json.partial`, writer-owned residue
/// that every reader ignores and no coordinator ever prunes (R21).
pub fn stage_answer<T: Serialize>(
    answers: &Path,
    component: &str,
    answer: &T,
    hooks: &mut dyn RunDirHooks,
) -> Result<(), UpstrokeError> {
    funnel(hooks, EffectSiteId::Answer(AnswerSite::StageWrite), || {
        util::write_json(&answers.join(format!("{component}.json.partial")), answer)
    })
}

/// `Answer.PublishRename` — the answer exists for the engine from here.
pub fn publish_answer(
    answers: &Path,
    component: &str,
    hooks: &mut dyn RunDirHooks,
) -> Result<(), UpstrokeError> {
    funnel(
        hooks,
        EffectSiteId::Answer(AnswerSite::PublishRename),
        || {
            let staged = answers.join(format!("{component}.json.partial"));
            let published = answers.join(format!("{component}.json"));
            fs::rename(&staged, &published).map_err(|source| UpstrokeError::Io {
                path: published,
                source,
            })
        },
    )
}

/// `Answer.Ingest` — read-only observation, no effect.
///
/// Hooked all the same: the site is in the frozen inventory with
/// `is_read_only()`, and a read-only site that never calls its hooks cannot be
/// shown to have executed.
pub fn ingest_answer(
    answers: &Path,
    component: &str,
    hooks: &mut dyn RunDirHooks,
) -> Result<Option<String>, UpstrokeError> {
    funnel(hooks, EffectSiteId::Answer(AnswerSite::Ingest), || {
        let path = answers.join(format!("{component}.json"));
        match fs::read_to_string(&path) {
            Ok(text) => Ok(Some(text)),
            Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(source) => Err(UpstrokeError::Io { path, source }),
        }
    })
}

// ===========================================================================
// Classification
// ===========================================================================

mod classify;
pub use classify::{RunDirClass, classify_run_dir, run_started_sha256};

// ===========================================================================
// The private half's ownership
// ===========================================================================

mod retention;
pub use retention::{OwnerField, PrivateHalfOwnership, RetainReason, UnboundShape};

mod ownership;
pub use ownership::{PrivateHalfProof, prove_private_half_ownership};

// ---------------------------------------------------------------------------
// Discovery
// ---------------------------------------------------------------------------

mod discovery;
pub use discovery::{
    FoundQuestion, HuskDisposition, HuskReport, Reclaimable, find_question, husk_report,
    latest_run, list_husks, list_runs, resolve_run_id, run_dir_names,
};

/// The lock beside one run's ops surface.
///
/// Takes the public directory rather than a whole [`RunPaths`] because the
/// lock lives in the public half by construction. Two callers only ever want
/// to know whether a run is live — `upstroke answer`, and the resume that must
/// claim the run *before* it has read where the private half went — and
/// neither has a private path to offer. Asking them for one invited passing
/// the public path twice, which would have quietly become wrong the moment
/// liveness consulted anything but the lock.
pub fn lock_file(public: &Path) -> PathBuf {
    public.join("run.lock")
}

fn worktree_lock_file(worktree_git_dir: &Path) -> PathBuf {
    worktree_git_dir.join("upstroke-worktree.lock")
}

/// An exclusive lease on the physical worktree shared by every run directory.
///
/// A per-run lock protects one log, but two distinct runs still share HEAD, the
/// index, and every working-tree byte. The engine therefore holds this outer
/// lease before either a fresh run or a resume can inspect or mutate Git state.
#[derive(Debug)]
pub struct WorktreeLock {
    _file: Option<File>,
    claim: PathBuf,
}

impl Drop for WorktreeLock {
    fn drop(&mut self) {
        release_claim_after_file(self._file.take(), &self.claim, || {});
    }
}

impl WorktreeLock {
    /// Acquire the lease for `repo_root` without placing coordination state in
    /// the working tree. Kept as the public convenience API for existing
    /// callers; the engine already has the resolved [`Workspace`] and uses
    /// [`Self::acquire_in`] to avoid opening it twice.
    pub fn acquire(repo_root: &Path) -> Result<Self, UpstrokeError> {
        let workspace = Workspace::open(repo_root)?;
        let worktree_git_dir = workspace.worktree_git_dir()?;
        Self::acquire_in(workspace.root(), &worktree_git_dir)
    }

    pub(crate) fn acquire_in(
        repo_root: &Path,
        worktree_git_dir: &Path,
    ) -> Result<Self, UpstrokeError> {
        Self::acquire_in_hooked(repo_root, worktree_git_dir, &mut NoHooks)
    }

    /// The same lease, observed.
    ///
    /// Two sites, two rows: `Lock.CreateWorktreeLockFile` is the file itself
    /// (R25, repository-scoped, "created on first acquisition by any write
    /// command through the lock funnel … spans runs; never removed by a run"),
    /// and `Lock.AcquireWorktree` is this process's hold on it (R17, "released
    /// at process exit"). One `open` serves both because `create(true)` is how
    /// the file comes to exist; the funnel names the create even when the file
    /// was already there, since the alternative is a stat that another process
    /// can invalidate between the question and the answer.
    pub(crate) fn acquire_in_hooked(
        repo_root: &Path,
        worktree_git_dir: &Path,
        hooks: &mut dyn RunDirHooks,
    ) -> Result<Self, UpstrokeError> {
        let path = worktree_lock_file(worktree_git_dir);
        let claim = claim_key(worktree_git_dir).join("upstroke-worktree.lock");
        if !claims().insert(claim.clone()) {
            return Err(worktree_refused(repo_root, &path, Some(std::process::id())));
        }
        let taken = funnel(
            hooks,
            EffectSiteId::Lock(LockSite::CreateWorktreeLockFile),
            || {
                File::options()
                    .create(true)
                    .truncate(false)
                    .write(true)
                    .read(true)
                    .open(&path)
                    .map_err(|source| UpstrokeError::Io {
                        path: path.clone(),
                        source,
                    })
            },
        )
        .and_then(|file| {
            funnel(
                hooks,
                EffectSiteId::Lock(LockSite::AcquireWorktree),
                || match imp::take(&file) {
                    Holder::Nobody => Ok(()),
                    Holder::Someone { pid } => Err(worktree_refused(repo_root, &path, pid)),
                    Holder::Unknown(source) => Err(UpstrokeError::Io {
                        path: path.clone(),
                        source,
                    }),
                },
            )
            .map(|()| file)
        });
        match taken {
            Ok(file) => {
                // A killed conductor releases the primary worktree lease, but
                // its Unix cleanup reaper deliberately retains the old run's
                // cleanup lease until every agent process is gone, and so does
                // a `git update-ref` it spawned, for as long as that child
                // lives (`hold_cleanup_lease_for_child`). Check only after
                // taking the primary lease, closing the race where the
                // conductor dies between a scan and this acquisition.
                //
                // `run_dir_names`, not `list_runs`: the reader returns
                // committed directories only, and the run most likely to have
                // a reaper still settling its groups is precisely the one that
                // died before its log committed. Scanning the readers' view
                // would leave R28 held and unobserved for exactly that run.
                if let Some(cleaning) = run_dir_names(repo_root)
                    .into_iter()
                    .map(|run_id| public_dir(repo_root, &run_id))
                    .find(|public| observe_cleanup_hold(public, hooks))
                {
                    release_claim_after_file(Some(file), &claim, || {});
                    return Err(UpstrokeError::Refused {
                        message: format!(
                            "run `{}` still has a process of its own alive in worktree {} -- an agent-cleanup reaper, or a Git child writing one of its refs -- and that process holds the run's cleanup lease; refusing overlapping engine ownership",
                            cleaning.file_name().unwrap_or_default().to_string_lossy(),
                            repo_root.display()
                        ),
                    });
                }
                Ok(Self {
                    _file: Some(file),
                    claim,
                })
            }
            Err(error) => {
                claims().remove(&claim);
                Err(error)
            }
        }
    }
}

/// A second, Unix-only lock used as a crash-cleanup lease. Each external agent
/// reaper opens its own shared hold; `resume` needs the exclusive side, so a
/// hard-killed conductor cannot hand over the run before cleanup is complete.
#[cfg(unix)]
fn cleanup_lock_file(public: &Path) -> PathBuf {
    public.join("cleanup.lock")
}

/// An exclusive hold on one run, released when this value drops.
///
/// Two engines on one run directory would interleave events into the log and
/// fight over the same git branch and working tree. An advisory OS lock is the
/// right shape for that because the operating system releases the primary
/// hold when the conductor dies. On Unix, live crash reapers retain only the
/// shared cleanup lease until their agent groups are quiescent. Neither hold
/// leaves a stale marker to clear by hand.
///
/// Which OS lock, though, is not a detail. See [`imp`].
#[derive(Debug)]
pub struct RunLock {
    _file: Option<File>,
    _cleanup: cleanup::CleanupLease,
    /// The run this claimed in [`claims`], given back on drop.
    claim: PathBuf,
}

impl Drop for RunLock {
    fn drop(&mut self) {
        self.release_file_then(|| {});
    }
}

impl RunLock {
    /// Close the process-scoped OS lock before publishing this process's claim
    /// as free. On POSIX, closing the old descriptor after another thread has
    /// acquired the same inode would release *all* of this process's locks on
    /// that inode, silently stripping the new owner's exclusion.
    fn release_file_then(&mut self, after_close: impl FnOnce()) {
        release_claim_after_file(self._file.take(), &self.claim, after_close);
    }

    /// Take the lock on a run's public directory, or explain who has it.
    pub fn acquire(public: &Path) -> Result<Self, UpstrokeError> {
        Self::acquire_hooked(public, &mut NoHooks)
    }

    /// The same lock, observed. `Lock.AcquireRun` (R17) around the hold, and
    /// `Lock.ProbeCleanupExclusive` (R17, Unix) around the momentary exclusive
    /// probe that refuses while a surviving reaper still holds R28.
    pub fn acquire_hooked(
        public: &Path,
        hooks: &mut dyn RunDirHooks,
    ) -> Result<Self, UpstrokeError> {
        let path = lock_file(public);
        let claim = claim_key(public);
        // This process first, and not only as an optimisation: the OS lock
        // below is per-*process*, so it cannot tell one thread here from
        // another. `claims` is what makes two `acquire`s in one process behave
        // the way two engines do, and it is exact rather than advisory.
        if !claims().insert(claim.clone()) {
            return Err(refused(public, &path, Some(std::process::id())));
        }
        let taken = funnel(hooks, EffectSiteId::Lock(LockSite::AcquireRun), || {
            File::options()
                .create(true)
                .truncate(false)
                .write(true)
                .read(true)
                .open(&path)
                .map_err(|source| UpstrokeError::Io {
                    path: path.clone(),
                    source,
                })
                .and_then(|file| match imp::take(&file) {
                    Holder::Nobody => Ok(file),
                    Holder::Someone { pid } => Err(refused(public, &path, pid)),
                    // A lock that cannot be taken is not a lock that was taken.
                    // Say what actually failed rather than blaming an engine
                    // that may not exist.
                    Holder::Unknown(source) => Err(UpstrokeError::Io {
                        path: path.clone(),
                        source,
                    }),
                })
        });
        match taken {
            Ok(file) => match funnel(
                hooks,
                EffectSiteId::Lock(LockSite::ProbeCleanupExclusive),
                || cleanup::take(public),
            ) {
                Ok(cleanup) => Ok(Self {
                    _file: Some(file),
                    _cleanup: cleanup,
                    claim,
                }),
                Err(error) => {
                    release_claim_after_file(Some(file), &claim, || {});
                    Err(error)
                }
            },
            Err(error) => {
                claims().remove(&claim);
                Err(error)
            }
        }
    }

    /// Give the hold back, naming `Lock.Release`.
    ///
    /// `Drop` does the same thing through [`NoHooks`], so the release happens
    /// whether or not anybody asks for it — including when the process dies and
    /// the OS does it. This exists so the site can be observed executing.
    pub fn release(mut self, hooks: &mut dyn RunDirHooks) {
        let _ = funnel(hooks, EffectSiteId::Lock(LockSite::Release), || {
            self.release_file_then(|| {});
            Ok::<(), UpstrokeError>(())
        });
    }

    /// Bind subprocess cleanup started on this thread to this run.
    ///
    /// The lock itself remains `Send`; callers enter the scope only while
    /// synchronously driving the run, so a future executor can move ownership
    /// first and establish the context on its actual worker thread.
    ///
    /// The scope is owned rather than borrowed from the lock: the topology run
    /// carries its lock inside the handle it drives, and a scope borrowing that
    /// lock could not coexist with the `&mut self` every step needs. Each site
    /// that spawns host processes under the lock enters a scope for exactly
    /// that stretch and drops it before it returns, so no scope outlives the
    /// hold it names. The cover review of `8a5f59e8` found the integration
    /// path's reapers holding no lease at all (`PR8-R4-CLEANUP-LEASE`).
    pub(crate) fn enter_cleanup_scope(&self) -> cleanup::CleanupScope {
        cleanup::enter(&self._cleanup)
    }
}

/// `Lock.ObserveCleanupHold` — R28, observed and never owned.
///
/// `resource_accounting` R28: "a surviving Unix cleanup reaper's shared
/// `cleanup.lock` hold (one per reaper; a reaper may outlive the coordinator
/// while it settles its process groups) … observed (never owned or reset) by
/// the next coordinator through `cleanup::is_held` at worktree-lease
/// acquisition and through the exclusive cleanup probe at run-lock
/// acquisition, **both of which refuse until the hold is released**".
///
/// Read-only, which is why `LockSite::ObserveCleanupHold::is_read_only()` is
/// the one `true` in its group — and hooked all the same, because a site that
/// never calls its hooks cannot be shown to have executed.
#[must_use]
pub fn observe_cleanup_hold(public: &Path, hooks: &mut dyn RunDirHooks) -> bool {
    funnel(
        hooks,
        EffectSiteId::Lock(LockSite::ObserveCleanupHold),
        || Ok::<bool, UpstrokeError>(cleanup::is_held(public)),
    )
    .unwrap_or(
        // An observation that was made to fail is not an observation that
        // found nothing. R28 held is the fail-closed answer, exactly as
        // `is_running` treats a lock the OS will not report on.
        true,
    )
}

/// Make the child a `Command` will spawn hold the run's cleanup lease (R28) for
/// as long as it lives, and return the descriptor this process keeps until the
/// child has been spawned.
///
/// The lease is the shared `flock` every surviving Unix cleanup reaper holds,
/// and `RunLock::acquire` probes its exclusive side and refuses the run while
/// anyone holds it. A Git child that writes a ref holds it the same way, so a
/// coordinator killed mid-write leaves a child whose liveness the next resume
/// sees as a kernel fact -- not a guess from a clock or a process table -- and
/// a lock file that child leaves is reclaimable only once the kernel has
/// released the lease: `WorkspaceManager::reclaim_own_ref_lock`, fact 1.
///
/// **Shared, and blocking.** Shared because several holders may be alive at
/// once, reapers and this child, and a shared take never conflicts with
/// another shared hold. Blocking because the only exclusive takers are the two
/// momentary probes of this module, `cleanup::take` and `cleanup::is_held`,
/// each of which unlocks in the statement after it locks: a non-blocking take
/// racing one of them would refuse a ref write over a hold that is already
/// gone. `EINTR` is retried.
///
/// **Inherited by this child only.** `File::open` sets `CLOEXEC`; the
/// `pre_exec` clears it in the child between `fork` and `exec`, so no other
/// process this coordinator spawns receives the descriptor. A `fork` without
/// `exec` in this process -- the Unix reaper -- would inherit it, and the
/// reaper's own hold on the same file is shared already, so nothing changes.
///
/// # Errors
///
/// An I/O error naming the lease file. `RunLock::acquire` created it, in a run
/// directory that exists for every run a coordinator holds.
#[cfg(unix)]
pub(crate) fn hold_cleanup_lease_for_child(
    command: &mut std::process::Command,
    public: &Path,
) -> Result<Option<File>, UpstrokeError> {
    use std::os::fd::AsRawFd as _;
    use std::os::unix::process::CommandExt as _;

    let path = cleanup_lock_file(public);
    let file = File::options()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&path)
        .map_err(|source| UpstrokeError::Io {
            path: path.clone(),
            source,
        })?;
    loop {
        // SAFETY: `file` owns a live descriptor, and `flock` takes an integer
        // and a flag and reads nothing through a pointer.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_SH) } == 0 {
            break;
        }
        let source = io::Error::last_os_error();
        if source.kind() != io::ErrorKind::Interrupted {
            return Err(UpstrokeError::Io { path, source });
        }
    }
    let fd = file.as_raw_fd();
    // SAFETY: the closure runs in the child between `fork` and `exec` and calls
    // only `fcntl`, which is async-signal-safe, on the descriptor the child
    // inherited from this process's open `file`; it allocates nothing and
    // touches no state of this process.
    unsafe {
        command.pre_exec(move || {
            if libc::fcntl(fd, libc::F_SETFD, 0) == -1 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    Ok(Some(file))
}

/// On Windows the coordinator's ambient kill-on-close Job Object ends every
/// child with the coordinator (INV-18), so a child cannot outlive the process
/// whose death a resume follows, and no lease is needed to say so.
#[cfg(not(unix))]
pub(crate) fn hold_cleanup_lease_for_child(
    _command: &mut std::process::Command,
    _public: &Path,
) -> Result<Option<File>, UpstrokeError> {
    Ok(None)
}

/// Release a process-scoped POSIX lock before another thread can observe the
/// in-process claim as free. This ordering is shared by ordinary `Drop` and by
/// rollback after the primary lock succeeded but the cleanup lease did not.
fn release_claim_after_file(file: Option<File>, claim: &Path, after_close: impl FnOnce()) {
    drop(file);
    after_close();
    claims().remove(claim);
}

fn refused(public: &Path, path: &Path, pid: Option<u32>) -> UpstrokeError {
    let who = match pid {
        Some(pid) => format!(" (pid {pid})"),
        None => String::new(),
    };
    UpstrokeError::Refused {
        message: format!(
            "another upstroke process{who} is already driving run `{}` (lock held on {}). Two \
             engines would interleave events and fight over the same branch — wait for it to \
             finish, or stop it first.",
            public.file_name().unwrap_or_default().to_string_lossy(),
            path.display()
        ),
    }
}

fn worktree_refused(repo_root: &Path, path: &Path, pid: Option<u32>) -> UpstrokeError {
    let who = match pid {
        Some(pid) => format!(" (pid {pid})"),
        None => String::new(),
    };
    UpstrokeError::Refused {
        message: format!(
            "another upstroke process{who} is already driving worktree {} (lock held on {}). Different run ids still share HEAD, the index, and working-tree bytes; wait for it to finish, or stop it first.",
            repo_root.display(),
            path.display()
        ),
    }
}

/// Who holds a run's lock.
#[derive(Debug)]
enum Holder {
    Nobody,
    /// Somebody does. `pid` where the platform will say.
    Someone {
        pid: Option<u32>,
    },
    /// The call failed without answering the question.
    Unknown(io::Error),
}

/// Run and worktree locks this process holds, so that two `acquire`s here
/// behave like two engines.
///
/// It also keeps [`is_running`] away from a lock file this process already
/// holds — which on Unix is not tidiness but a correctness requirement, because
/// closing *any* descriptor for a file releases every `fcntl` lock this process
/// has on it. A bare `File::open` + drop in the holder would silently hand the
/// run away. Answering from here means that open never happens.
fn claims() -> &'static Claims {
    static CLAIMS: Claims = Claims {
        runs: Mutex::new(BTreeSet::new()),
    };
    &CLAIMS
}

#[derive(Debug)]
struct Claims {
    runs: Mutex<BTreeSet<PathBuf>>,
}

impl Claims {
    /// `true` if this process did not already hold it.
    fn insert(&self, key: PathBuf) -> bool {
        self.held().insert(key)
    }

    fn remove(&self, key: &Path) {
        self.held().remove(key);
    }

    fn contains(&self, key: &Path) -> bool {
        self.held().contains(key)
    }

    /// A panic in a lock holder must not take the run lock's bookkeeping with
    /// it: the set is still exactly as valid as it was before the panic.
    fn held(&self) -> std::sync::MutexGuard<'_, BTreeSet<PathBuf>> {
        self.runs
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// The run's identity for [`claims`], resolved so that two spellings of one
/// directory cannot look like two runs.
fn claim_key(public: &Path) -> PathBuf {
    fs::canonicalize(public).unwrap_or_else(|_| public.to_path_buf())
}

/// Whether a run is being driven right now, without disturbing the holder.
///
/// Read-only with respect to the run record. On Unix, `F_GETLK` asks who holds
/// the primary lock without taking one. Only when that lock is free does the
/// probe momentarily try the exclusive side of the cleanup lease; it never
/// creates or changes either file. A primary file that does not exist means
/// the run never started.
pub fn is_running(public: &Path) -> bool {
    // Asked and answered without touching the file. On Unix this is the branch
    // that keeps `fcntl`'s release-on-any-close from applying to us at all.
    if claims().contains(&claim_key(public)) {
        return true;
    }
    let file = match File::open(lock_file(public)) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return false,
        // An existing run whose lock cannot be inspected is not safe to call
        // dead. `acquire` will report the concrete IO error if a resume tries.
        Err(_) => return true,
    };
    match imp::holder(&file) {
        Holder::Nobody => observe_cleanup_hold(public, &mut NoHooks),
        Holder::Someone { .. } => true,
        // The opened-fine-but-cannot-be-locked case, which is not the same as
        // the unopenable file above and does not get the same answer. Locking
        // fails with `ENOLCK` or `EOPNOTSUPP` on filesystems that do not carry
        // locks — NFS, SMB, some container overlays — and it does so whether or
        // not an engine is driving the run.
        //
        // So the question is which way to be wrong when the OS refuses to say.
        // Answering "not running" makes `status` settle a working attempt as
        // cut off and print `state: interrupted … Continue it with: upstroke
        // resume <id>`, sending the operator to start a second engine on a
        // live run. Answering "running" costs a `status` that declines to
        // settle and says another process holds the run. One of those invents
        // a fact the operator will act on; the other admits the run may still
        // be going. `acquire` is the real guard against two engines either
        // way, and it reports this case as the IO error it is.
        Holder::Unknown(_) => true,
    }
}

#[cfg(unix)]
mod cleanup {
    use super::{cleanup_lock_file, refused};
    use crate::error::UpstrokeError;
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::fs::File;
    use std::marker::PhantomData;
    use std::os::fd::AsRawFd;
    use std::path::{Path, PathBuf};
    use std::rc::Rc;

    thread_local! {
        // v0.1 drives a run synchronously inside an explicit scope. Thread-
        // local registration gives concurrent library/test runs the exact
        // cleanup path for their own reapers instead of conservatively leasing
        // every run active in the process.
        static ACTIVE: RefCell<BTreeMap<PathBuf, usize>> = const { RefCell::new(BTreeMap::new()) };
    }

    #[derive(Debug)]
    pub(super) struct CleanupLease {
        path: PathBuf,
    }

    #[derive(Debug)]
    pub(crate) struct CleanupScope {
        path: PathBuf,
        _thread: PhantomData<Rc<()>>,
    }

    impl Drop for CleanupScope {
        fn drop(&mut self) {
            ACTIVE.with(|active| {
                let mut active = active.borrow_mut();
                let remove = if let Some(count) = active.get_mut(&self.path) {
                    *count = count.saturating_sub(1);
                    *count == 0
                } else {
                    false
                };
                if remove {
                    active.remove(&self.path);
                }
            });
        }
    }

    pub(super) fn enter(lease: &CleanupLease) -> CleanupScope {
        ACTIVE.with(|active| {
            let mut active = active.borrow_mut();
            *active.entry(lease.path.clone()).or_default() += 1;
        });
        CleanupScope {
            path: lease.path.clone(),
            _thread: PhantomData,
        }
    }

    pub(super) fn take(public: &Path) -> Result<CleanupLease, UpstrokeError> {
        let path = cleanup_lock_file(public);
        let file = File::options()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|source| UpstrokeError::Io {
                path: path.clone(),
                source,
            })?;
        let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if result != 0 {
            let source = std::io::Error::last_os_error();
            if matches!(
                source.raw_os_error(),
                Some(code) if code == libc::EWOULDBLOCK || code == libc::EAGAIN
            ) {
                return Err(refused(public, &path, None));
            }
            return Err(UpstrokeError::Io { path, source });
        }
        // This probe proves no prior crash reaper remains. Do not retain the
        // lock in the conductor: arbitrary forked children would inherit its
        // open file description and recreate the false-liveness window the
        // primary fcntl lock deliberately avoids. Each cleanup reaper instead
        // reopens `path` and owns an independent shared hold.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_UN) } != 0 {
            return Err(UpstrokeError::Io {
                path,
                source: std::io::Error::last_os_error(),
            });
        }
        Ok(CleanupLease { path })
    }

    pub(super) fn is_held(public: &Path) -> bool {
        let path = cleanup_lock_file(public);
        let file = match File::options().read(true).write(true).open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return false,
            // An existing lease file that cannot be inspected is not evidence
            // that cleanup finished. Keep liveness fail-closed just as the
            // primary lock does for an unreportable holder.
            Err(_) => return true,
        };
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
            let _ = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_UN) };
            false
        } else {
            true
        }
    }

    pub(crate) fn active_paths() -> Vec<PathBuf> {
        ACTIVE.with(|active| active.borrow().keys().cloned().collect())
    }
}

#[cfg(not(unix))]
mod cleanup {
    use crate::error::UpstrokeError;
    use std::marker::PhantomData;
    use std::path::Path;
    use std::rc::Rc;

    #[derive(Debug)]
    pub(super) struct CleanupLease;

    #[derive(Debug)]
    pub(crate) struct CleanupScope {
        _thread: PhantomData<Rc<()>>,
    }

    impl Drop for CleanupScope {
        fn drop(&mut self) {}
    }

    pub(super) fn take(_: &Path) -> Result<CleanupLease, UpstrokeError> {
        Ok(CleanupLease)
    }

    pub(super) fn is_held(_: &Path) -> bool {
        false
    }

    pub(super) fn enter(_lease: &CleanupLease) -> CleanupScope {
        CleanupScope {
            _thread: PhantomData,
        }
    }
}

pub(crate) use cleanup::CleanupScope;

#[cfg(unix)]
pub(crate) fn active_cleanup_lease_paths() -> Vec<PathBuf> {
    cleanup::active_paths()
}

/// The lock primitive, and why it is not `std`'s.
///
/// `File::try_lock` is `flock(2)` on Unix, and `flock` locks are held by the
/// *open file description*. `fork` duplicates every descriptor, so a child
/// inherits this lock and keeps holding it until it execs — which means an
/// engine that has finished and let go stays "locked" for as long as some
/// unrelated subprocess spawn is between `fork` and `exec`. Measured: hold a
/// lock, fork, release it, and a fresh probe still reports it taken.
///
/// That was papered over with a 500ms grace — believe contention only if it
/// persists — which is a timing proxy for a property the platform states
/// outright, and only ever probabilistic: a fork window longer than the grace
/// on a loaded machine still refuses a run that nothing is driving, and every
/// `status` of a live run paid the full half-second to find out.
///
/// `fcntl(F_SETLK)` locks are held by the *process*, and are documented not to
/// be inherited across `fork`. The grace disappears rather than being tuned.
///
/// Two things come with that, both measured rather than assumed:
///
/// - They do not exclude the same process from itself, which is what [`claims`]
///   is for.
/// - Closing **any** descriptor for the file releases every lock this process
///   holds on it, so a holder must never open its own lock file again.
///   [`is_running`] answers from [`claims`] before it would.
///
/// `F_OFD_SETLK` is not an escape from the first two: it is scoped to the open
/// file description, exactly like `flock`, and is inherited across `fork` in
/// exactly the same way.
///
/// Windows has neither hazard — `LockFileEx` is per-handle and there is no
/// `fork` — so it keeps std's implementation and this module is where the two
/// meet.
#[cfg(unix)]
mod imp {
    use super::Holder;
    use std::fs::File;
    use std::io;
    use std::os::fd::AsRawFd;

    /// `F_WRLCK` and `F_UNLCK` are `c_int` on Linux and `c_short` on macOS,
    /// while `flock.l_type` is `c_short` on both. `Into<c_int>` accepts either
    /// — the reflexive conversion on Linux, the widening one on macOS — so the
    /// narrowing to `l_type` happens here and nowhere else.
    fn l_type(kind: impl Into<libc::c_int>) -> libc::c_short {
        kind.into() as libc::c_short
    }

    /// Take the exclusive lock, or say who has it.
    pub(super) fn take(file: &File) -> Holder {
        match set_lock(file, l_type(libc::F_WRLCK)) {
            Ok(()) => Holder::Nobody,
            Err(error) if would_block(&error) => Holder::Someone {
                pid: holding_pid(file),
            },
            Err(error) => Holder::Unknown(error),
        }
    }

    /// Ask who holds it, taking nothing. There is no shared lock to give back
    /// here and so no window in which this call is itself the holder.
    pub(super) fn holder(file: &File) -> Holder {
        match query(file) {
            Ok(Some(pid)) => Holder::Someone { pid: Some(pid) },
            Ok(None) => Holder::Nobody,
            Err(error) => Holder::Unknown(error),
        }
    }

    fn describe(kind: libc::c_short) -> libc::flock {
        libc::flock {
            l_type: kind,
            l_whence: libc::SEEK_SET as libc::c_short,
            // A zero length locks the whole file, however long it grows. The
            // file's contents are never read; it exists to be locked.
            l_start: 0,
            l_len: 0,
            l_pid: 0,
        }
    }

    fn set_lock(file: &File, kind: libc::c_short) -> io::Result<()> {
        let request = describe(kind);
        // `F_SETLK` never blocks, so unlike `flock` it has no interruptible
        // wait to be cut short — the `EINTR` retry the old loop carried has
        // nothing left to guard.
        if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETLK, &request) } == 0 {
            return Ok(());
        }
        Err(io::Error::last_os_error())
    }

    /// `Some(pid)` if a conflicting lock exists, `None` if the file is free.
    fn query(file: &File) -> io::Result<Option<u32>> {
        let mut request = describe(l_type(libc::F_WRLCK));
        if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETLK, &mut request) } != 0 {
            return Err(io::Error::last_os_error());
        }
        if request.l_type == l_type(libc::F_UNLCK) {
            return Ok(None);
        }
        Ok(Some(u32::try_from(request.l_pid).unwrap_or_default()))
    }

    /// Best effort: the holder may let go between the refusal and the question,
    /// and a name that might be stale is worth more than no name at all.
    fn holding_pid(file: &File) -> Option<u32> {
        query(file).ok().flatten()
    }

    fn would_block(error: &io::Error) -> bool {
        // POSIX allows either, and says a portable caller must accept both.
        matches!(
            error.raw_os_error(),
            Some(code) if code == libc::EACCES || code == libc::EAGAIN
        )
    }
}

#[cfg(windows)]
mod imp {
    use super::Holder;
    use std::fs::File;
    use std::io;
    use std::os::windows::io::AsRawHandle;

    use windows_sys::Win32::Foundation::{ERROR_LOCK_VIOLATION, HANDLE};
    use windows_sys::Win32::Storage::FileSystem::{
        LOCKFILE_EXCLUSIVE_LOCK, LOCKFILE_FAIL_IMMEDIATELY, LockFileEx, UnlockFileEx,
    };
    use windows_sys::Win32::System::IO::OVERLAPPED;

    pub(super) fn take(file: &File) -> Holder {
        try_lock(file, true)
    }

    pub(super) fn holder(file: &File) -> Holder {
        match try_lock(file, false) {
            Holder::Nobody => match unlock(file) {
                Ok(()) => Holder::Nobody,
                Err(source) => Holder::Unknown(source),
            },
            other => other,
        }
    }

    fn try_lock(file: &File, exclusive: bool) -> Holder {
        let mut overlapped = OVERLAPPED::default();
        let flags = LOCKFILE_FAIL_IMMEDIATELY
            | if exclusive {
                LOCKFILE_EXCLUSIVE_LOCK
            } else {
                0
            };
        // SAFETY: `file` owns a live Windows handle, `overlapped` describes
        // offset zero, and the same whole-file range is used by every holder.
        let locked = unsafe {
            LockFileEx(
                file.as_raw_handle() as HANDLE,
                flags,
                0,
                u32::MAX,
                u32::MAX,
                &mut overlapped,
            )
        };
        if locked != 0 {
            return Holder::Nobody;
        }
        let source = io::Error::last_os_error();
        if source.raw_os_error() == Some(ERROR_LOCK_VIOLATION as i32) {
            // LockFileEx names no owner, and inventing one would be worse than
            // the shorter sentence.
            Holder::Someone { pid: None }
        } else {
            Holder::Unknown(source)
        }
    }

    fn unlock(file: &File) -> io::Result<()> {
        let mut overlapped = OVERLAPPED::default();
        // SAFETY: this releases exactly the range acquired in `try_lock`.
        if unsafe {
            UnlockFileEx(
                file.as_raw_handle() as HANDLE,
                0,
                u32::MAX,
                u32::MAX,
                &mut overlapped,
            )
        } != 0
        {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
}

// ===========================================================================
// The test build's scratch trees
// ===========================================================================

#[cfg(test)]
pub(crate) mod scratch_tree;

/// What a report writer that died between its stage and its rename leaves,
/// planted through the writer's own record-then-create helper — never by
/// hand, since a directory planted by hand carries no record and is not the
/// protocol's: the record in `private` naming the directory, the directory
/// under `public`, and inside it `report.json` half-written; the answer is
/// that file. Test builds only, after the census boundary above.
#[cfg(test)]
pub(crate) fn plant_report_staging_of_a_dead_writer(public: &Path, private: &Path) -> PathBuf {
    let staging = begin_report_staging(public, private, &DurabilityLedger::off())
        .expect("the writer's own record-then-create helper plants the dead writer's directory");
    let leftover = staging.join(REPORT);
    fs::write(&leftover, b"{\"half\":").expect("a dead writer's staged report");
    leftover
}

#[cfg(test)]
mod tests;
