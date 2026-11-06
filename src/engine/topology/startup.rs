//! Extended notes: `docs/internals/engine/topology/startup.md`

use std::path::{Path, PathBuf};

use crate::error::UpstrokeError;
use crate::rundir::{
    self, HuskDisposition, PrivateHalfOwnership, PrivateHalfProof, Reclaimable, RepoKey,
    RetainReason, RunDirClass, UnboundShape,
};
use crate::runner::container::GitView;
use crate::runner::container::census::{
    Boundary, Census, CensusComplete, CensusReport, CensusStart, StagedDisposition,
    run_startup_census,
};
use crate::runner::container::runtime::{ContainerRuntime, OwnerLiveness};
use crate::workspace_manager::WorkspaceManager;

use super::create::{
    LocationRecord, RETIRED_ROOT, claim_token, read_location_record, unsettleable,
};
use super::seams::TopologyHooks;

pub use witness::{FreshCensused, WorktreeLocked};

pub struct CensusInputs<'a> {
    pub repo_root: &'a Path,
    pub repo_key: &'a RepoKey,
    pub authorized_root: &'a Path,
    pub incarnation: &'a str,
    pub runtime: &'a dyn ContainerRuntime,
    pub liveness: &'a dyn OwnerLiveness,
    pub view: &'a dyn GitView,
}

impl std::fmt::Debug for CensusInputs<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CensusInputs")
            .field("repo_root", &self.repo_root)
            .field("repo_key", &self.repo_key)
            .field("authorized_root", &self.authorized_root)
            .field("incarnation", &self.incarnation)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailedStep {
    PublicHalf,
    PrivateHalf,
    PublicHalfAfterPrivate,
    StaleMarker,
    ExecutionRoot,
}

impl FailedStep {
    pub const ALL: &'static [Self] = &[
        Self::PublicHalf,
        Self::PrivateHalf,
        Self::PublicHalfAfterPrivate,
        Self::StaleMarker,
        Self::ExecutionRoot,
    ];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::PublicHalf => "public-half",
            Self::PrivateHalf => "private-half",
            Self::PublicHalfAfterPrivate => "public-half-after-private",
            Self::StaleMarker => "stale-marker",
            Self::ExecutionRoot => "execution-root",
        }
    }

    #[must_use]
    pub const fn what_failed(self) -> &'static str {
        match self {
            Self::PublicHalf => {
                "the public half could not be reclaimed; no private half existed by ordering, so \
                 nothing private is at risk"
            }
            Self::PrivateHalf => {
                "the private half could not be removed, so the public directory was left in place \
                 with its marker — `.creating` is that private half's only locator, and removing \
                 it would orphan a directory no census, no `status` and no deferred \
                 `upstroke runs prune` could ever reach again"
            }
            Self::PublicHalfAfterPrivate => {
                "the private half went through the proof-token funnel and the public directory \
                 could not be removed after it, so a husk whose marker names an absent target is \
                 left, which the next census reclaims public-only"
            }
            Self::StaleMarker => {
                "the stale `.creating` marker could not be removed; the run itself is untouched \
                 and the marker is residue the next census repairs"
            }
            Self::ExecutionRoot => {
                "the execution root this run published could not be retired into its private \
                 half, so neither half was touched: the location record and the marker still \
                 locate the root and its claim token still proves it is this run's, so the \
                 next census retires it first and then reclaims both halves"
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunDirOutcome {
    ReclaimedPublicOnly(UnboundShape),
    ReclaimedBothHalves,
    ReclaimedRootAndBothHalves {
        root: PathBuf,
    },
    Retained(RetainReason),
    RepairedStaleMarker,
    Committed,
    Skipped,
    Unreclaimable {
        step: FailedStep,
        detail: String,
    },
    RootRetained {
        root: Option<PathBuf>,
        detail: String,
    },
}

impl RunDirOutcome {
    pub const KINDS: &'static [&'static str] = &[
        "reclaimed-public-only",
        "reclaimed-both-halves",
        "reclaimed-root-and-both-halves",
        "retained",
        "repaired-stale-marker",
        "committed",
        "skipped",
        "unreclaimable",
        "root-retained",
    ];

    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::ReclaimedPublicOnly(_) => "reclaimed-public-only",
            Self::ReclaimedBothHalves => "reclaimed-both-halves",
            Self::ReclaimedRootAndBothHalves { .. } => "reclaimed-root-and-both-halves",
            Self::Retained(_) => "retained",
            Self::RepairedStaleMarker => "repaired-stale-marker",
            Self::Committed => "committed",
            Self::Skipped => "skipped",
            Self::Unreclaimable { .. } => "unreclaimable",
            Self::RootRetained { .. } => "root-retained",
        }
    }

    #[must_use]
    pub const fn reclaimed_anything(&self) -> bool {
        matches!(
            self,
            Self::ReclaimedPublicOnly(_)
                | Self::ReclaimedBothHalves
                | Self::ReclaimedRootAndBothHalves { .. }
                | Self::Unreclaimable {
                    step: FailedStep::PublicHalfAfterPrivate,
                    ..
                }
        )
    }

    #[must_use]
    pub const fn deleted_a_private_half(&self) -> bool {
        matches!(
            self,
            Self::ReclaimedBothHalves
                | Self::ReclaimedRootAndBothHalves { .. }
                | Self::Unreclaimable {
                    step: FailedStep::PublicHalfAfterPrivate,
                    ..
                }
        )
    }

    #[must_use]
    pub const fn may_have_deleted_a_private_half(&self) -> bool {
        matches!(
            self,
            Self::ReclaimedBothHalves
                | Self::ReclaimedRootAndBothHalves { .. }
                | Self::Unreclaimable {
                    step: FailedStep::PrivateHalf | FailedStep::PublicHalfAfterPrivate,
                    ..
                }
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunDirEntry {
    pub run_id: String,
    pub public: PathBuf,
    pub locator: Option<PathBuf>,
    pub class: RunDirClass,
    pub outcome: RunDirOutcome,
    pub left_alone: Vec<PathBuf>,
}

impl RunDirEntry {
    #[must_use]
    pub const fn retain_reason(&self) -> Option<&RetainReason> {
        match &self.outcome {
            RunDirOutcome::Retained(reason) => Some(reason),
            _ => None,
        }
    }

    #[must_use]
    pub const fn is_possibly_committed(&self) -> bool {
        matches!(
            self.outcome,
            RunDirOutcome::Retained(RetainReason::PossiblyCommitted)
        )
    }

    #[must_use]
    pub fn describe(&self) -> String {
        let what = match &self.outcome {
            RunDirOutcome::ReclaimedPublicOnly(shape) => format!(
                "reclaimed the public half alone ({})",
                match shape {
                    UnboundShape::Bare => "a bare directory",
                    UnboundShape::StagedMarkerOnly => "only a staged marker",
                    UnboundShape::TargetAbsent => "its recorded private half is gone",
                }
            ),
            RunDirOutcome::ReclaimedBothHalves => {
                "reclaimed the private half under the ownership proof, then the public \
                 directory with the marker last"
                    .to_owned()
            }
            RunDirOutcome::ReclaimedRootAndBothHalves { root } => format!(
                "retired the execution root {} this run published, which its location record \
                 names and its claim token proves, into the private half; then reclaimed the \
                 private half under the ownership proof and the public directory with the \
                 marker last",
                root.display()
            ),
            RunDirOutcome::Retained(reason) => {
                format!("retained, nothing deleted: {reason}")
            }
            RunDirOutcome::RepairedStaleMarker => {
                "a committed run: removed its stale `.creating` marker".to_owned()
            }
            RunDirOutcome::Committed => "a committed run: nothing to do".to_owned(),
            RunDirOutcome::Skipped => {
                "skipped: its `run.lock` is held by a live process".to_owned()
            }
            RunDirOutcome::Unreclaimable { step, detail } => format!(
                "retained with the error recorded, and the census carried on: {} ({detail})",
                step.what_failed()
            ),
            RunDirOutcome::RootRetained { root, detail } => format!(
                "retained, nothing deleted: {} ({detail}), so the husk stays as that \
                 directory's locator",
                root.as_ref().map_or_else(
                    || "where this run's execution root is cannot be told".to_owned(),
                    |root| format!("a directory at {} is kept", root.display())
                )
            ),
        };
        let what = if self.left_alone.is_empty() {
            what
        } else {
            format!(
                "{what}; left untouched, because nothing proves them this run's: {}",
                self.left_alone
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        match &self.locator {
            Some(locator) => format!(
                "{} at {}: {what} (private locator {})",
                self.run_id,
                self.public.display(),
                locator.display()
            ),
            None => format!("{} at {}: {what}", self.run_id, self.public.display()),
        }
    }

    #[must_use]
    pub fn needs_operator(&self) -> bool {
        matches!(
            self.outcome,
            RunDirOutcome::Retained(_)
                | RunDirOutcome::RootRetained { .. }
                | RunDirOutcome::Unreclaimable { .. }
        ) || !self.left_alone.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunDirCensusReport {
    entries: Vec<RunDirEntry>,
}

impl RunDirCensusReport {
    #[must_use]
    pub fn entries(&self) -> &[RunDirEntry] {
        &self.entries
    }

    #[must_use]
    pub fn of(&self, run_id: &str) -> Option<&RunDirEntry> {
        self.entries.iter().find(|entry| entry.run_id == run_id)
    }

    #[must_use]
    pub fn reclaimed(&self) -> Vec<&RunDirEntry> {
        self.with(|outcome| outcome.reclaimed_anything())
    }

    #[must_use]
    pub fn repaired(&self) -> Vec<&RunDirEntry> {
        self.with(|outcome| matches!(outcome, RunDirOutcome::RepairedStaleMarker))
    }

    #[must_use]
    pub fn retained(&self) -> Vec<&RunDirEntry> {
        self.with(|outcome| matches!(outcome, RunDirOutcome::Retained(_)))
    }

    #[must_use]
    pub fn possibly_committed(&self) -> Vec<&RunDirEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.is_possibly_committed())
            .collect()
    }

    #[must_use]
    pub fn skipped(&self) -> Vec<&RunDirEntry> {
        self.with(|outcome| matches!(outcome, RunDirOutcome::Skipped))
    }

    #[must_use]
    pub fn unreclaimable(&self) -> Vec<&RunDirEntry> {
        self.with(|outcome| matches!(outcome, RunDirOutcome::Unreclaimable { .. }))
    }

    fn with(&self, keep: impl Fn(&RunDirOutcome) -> bool) -> Vec<&RunDirEntry> {
        self.entries
            .iter()
            .filter(|entry| keep(&entry.outcome))
            .collect()
    }

    #[must_use]
    pub fn operator_lines(&self) -> Vec<String> {
        self.entries
            .iter()
            .filter(|entry| entry.needs_operator())
            .map(|entry| format!("startup census: run directory {}", entry.describe()))
            .collect()
    }
}

#[must_use]
pub fn container_lines(report: &CensusReport) -> Vec<String> {
    let reclaimed = report.reclaimed.iter().map(|entry| {
        format!(
            "startup census: reclaimed container {} of run {} (incarnation {}), {}",
            entry.name,
            entry.run_id,
            entry.incarnation,
            match &entry.boundary {
                Boundary::FromIntent(digest) => {
                    format!("boundary from its intent: runner policy {digest}")
                }
                Boundary::NoIntentRecord => {
                    "no intent recorded its boundary, so none is named".to_owned()
                }
            }
        )
    });
    let staged = report
        .staged
        .iter()
        .filter(|entry| entry.disposition == StagedDisposition::RetainedForeignOwner)
        .map(|entry| {
            format!(
                "startup census: retained staged container intent {} for {}: another owner's, so \
                 nothing was removed",
                entry.path.display(),
                entry.name
            )
        });
    reclaimed.chain(staged).collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupCensus {
    containers: CensusComplete,
    run_dirs: RunDirCensusReport,
}

impl StartupCensus {
    #[must_use]
    pub const fn containers(&self) -> &CensusComplete {
        &self.containers
    }

    #[must_use]
    pub const fn run_dirs(&self) -> &RunDirCensusReport {
        &self.run_dirs
    }

    #[must_use]
    #[allow(dead_code)]
    pub fn into_parts(self) -> (CensusComplete, RunDirCensusReport) {
        (self.containers, self.run_dirs)
    }

    #[must_use]
    pub fn operator_lines(&self) -> Vec<String> {
        let mut lines = container_lines(self.containers.report());
        lines.extend(self.run_dirs.operator_lines());
        lines
    }
}

pub fn startup_census(
    locked: WorktreeLocked,
    hooks: &mut dyn TopologyHooks,
    inputs: &CensusInputs<'_>,
) -> Result<FreshCensused, UpstrokeError> {
    FreshCensused::establish(locked, hooks, inputs)
}

fn both_halves(
    hooks: &mut dyn TopologyHooks,
    inputs: &CensusInputs<'_>,
    start: &CensusStart,
) -> Result<StartupCensus, UpstrokeError> {
    let census = Census {
        private_root: inputs.authorized_root,
        start,
        runtime: inputs.runtime,
        liveness: inputs.liveness,
        view: inputs.view,
    };
    let containers = run_startup_census(hooks.container(), &census)?;
    let run_dirs = census_run_dirs(hooks, inputs, start.own_run())
        .map_err(|error| error.with_warnings(container_lines(containers.report())))?;
    Ok(StartupCensus {
        containers,
        run_dirs,
    })
}

pub(crate) fn census_run_dirs(
    hooks: &mut dyn TopologyHooks,
    inputs: &CensusInputs<'_>,
    own_run: Option<&str>,
) -> Result<RunDirCensusReport, UpstrokeError> {
    let scanned: Vec<Scanned> = enumerate(inputs.repo_root)?
        .iter()
        .map(|run_id| scan(run_id, inputs, own_run))
        .collect();

    let mut entries = Vec::with_capacity(scanned.len());
    for item in scanned {
        let outcome = apply(hooks, &item.public, item.plan);
        entries.push(RunDirEntry {
            run_id: item.run_id,
            public: item.public,
            locator: item.locator,
            class: item.class,
            outcome,
            left_alone: item.left_alone,
        });
    }
    Ok(RunDirCensusReport { entries })
}

fn enumerate(repo_root: &Path) -> Result<Vec<String>, UpstrokeError> {
    let names = rundir::run_dir_names(repo_root);
    if names.is_empty() {
        let runs = rundir::runs_root(repo_root);
        match std::fs::read_dir(&runs) {
            Ok(_) => {}
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => return Err(UpstrokeError::Io { path: runs, source }),
        }
    }
    Ok(names)
}

#[derive(Debug)]
struct Scanned {
    run_id: String,
    public: PathBuf,
    locator: Option<PathBuf>,
    class: RunDirClass,
    plan: Planned,
    left_alone: Vec<PathBuf>,
}

#[derive(Debug)]
enum Planned {
    ReclaimPublicOnly(UnboundShape),
    ReclaimBothHalves(PrivateHalfProof),
    SettleRootThenBothHalves {
        manager: Box<WorkspaceManager>,
        token: String,
        into: PathBuf,
        ours: bool,
        proof: PrivateHalfProof,
    },
    Retain(RetainReason),
    KeepForRoot {
        root: Option<PathBuf>,
        detail: String,
    },
    RepairStaleMarker,
    Committed,
    Skip,
}

fn scan(run_id: &str, inputs: &CensusInputs<'_>, own_run: Option<&str>) -> Scanned {
    let public = rundir::public_dir(inputs.repo_root, run_id);
    let class = rundir::classify_run_dir(&public);
    scan_classified(run_id, public, class, inputs, own_run)
}

fn scan_classified(
    run_id: &str,
    public: PathBuf,
    class: RunDirClass,
    inputs: &CensusInputs<'_>,
    own_run: Option<&str>,
) -> Scanned {
    let plain = |public: PathBuf, locator: Option<PathBuf>, plan: Planned| Scanned {
        run_id: run_id.to_owned(),
        public,
        locator,
        class,
        plan,
        left_alone: Vec::new(),
    };
    match class {
        RunDirClass::Indeterminate => plain(
            public,
            None,
            Planned::Retain(RetainReason::ClassificationIncomplete),
        ),

        RunDirClass::Committed => {
            let lock_held = rundir::is_running(&public);
            let own = own_run == Some(run_id);
            let plan = if !stale_marker_present(&public) {
                Planned::Committed
            } else if lock_held && !own {
                Planned::Skip
            } else {
                Planned::RepairStaleMarker
            };
            plain(public, None, plan)
        }

        RunDirClass::Husk => {
            if rundir::is_running(&public) {
                return plain(public, None, Planned::Skip);
            }

            let report = rundir::husk_report(
                inputs.repo_root,
                run_id,
                inputs.repo_key,
                inputs.authorized_root,
            );
            let (plan, left_alone) = match report.disposition {
                HuskDisposition::Unstarted(Reclaimable::PublicOnly(shape)) => (
                    unbound_unless_rooted(&report.public, run_id, inputs, shape),
                    Vec::new(),
                ),
                HuskDisposition::Unstarted(Reclaimable::BothHalves) => {
                    match rundir::prove_private_half_ownership(
                        &report.public,
                        inputs.repo_key,
                        inputs.authorized_root,
                    ) {
                        PrivateHalfOwnership::Proven(proof) => {
                            match plan_root(&report.public, run_id, proof.target(), inputs) {
                                RootPlan::Nothing { left_alone } => {
                                    (Planned::ReclaimBothHalves(proof), left_alone)
                                }
                                RootPlan::Settle {
                                    manager,
                                    token,
                                    into,
                                    ours,
                                    left_alone,
                                } => (
                                    Planned::SettleRootThenBothHalves {
                                        manager,
                                        token,
                                        into,
                                        ours,
                                        proof,
                                    },
                                    left_alone,
                                ),
                                RootPlan::Keep { root, detail } => {
                                    (Planned::KeepForRoot { root, detail }, Vec::new())
                                }
                            }
                        }
                        PrivateHalfOwnership::NothingBound(shape) => (
                            unbound_unless_rooted(&report.public, run_id, inputs, shape),
                            Vec::new(),
                        ),
                        PrivateHalfOwnership::Retained(reason) => {
                            (Planned::Retain(reason), Vec::new())
                        }
                    }
                }
                HuskDisposition::Retained(reason) => (Planned::Retain(reason), Vec::new()),
            };
            Scanned {
                run_id: report.run_id,
                public: report.public,
                locator: report.locator,
                class,
                plan,
                left_alone,
            }
        }
    }
}

fn apply(hooks: &mut dyn TopologyHooks, public: &Path, plan: Planned) -> RunDirOutcome {
    match plan {
        Planned::ReclaimPublicOnly(shape) => {
            match rundir::remove_public_husk(public, hooks.rundir()) {
                Ok(()) => RunDirOutcome::ReclaimedPublicOnly(shape),
                Err(error) => unreclaimable(FailedStep::PublicHalf, &error),
            }
        }
        Planned::ReclaimBothHalves(proof) => reclaim_both_halves(hooks, public, proof, None),
        Planned::SettleRootThenBothHalves {
            manager,
            token,
            into,
            ours,
            proof,
        } => {
            let root = manager.execution_root().to_path_buf();
            match manager.retire_execution_root(hooks.effects(), &into, &token) {
                Err(error) => unreclaimable(FailedStep::ExecutionRoot, &error),
                Ok(true) => reclaim_both_halves(hooks, public, proof, Some(root)),
                Ok(false) if !ours => reclaim_both_halves(hooks, public, proof, None),
                Ok(false) => RunDirOutcome::RootRetained {
                    root: Some(root),
                    detail: "it changed while the census was settling it, so nothing was \
                             retired and both halves stay"
                        .to_owned(),
                },
            }
        }
        Planned::Retain(reason) => RunDirOutcome::Retained(reason),
        Planned::KeepForRoot { root, detail } => RunDirOutcome::RootRetained { root, detail },
        Planned::RepairStaleMarker => match rundir::remove_marker(public, hooks.rundir()) {
            Ok(()) => RunDirOutcome::RepairedStaleMarker,
            Err(error) => unreclaimable(FailedStep::StaleMarker, &error),
        },
        Planned::Committed => RunDirOutcome::Committed,
        Planned::Skip => RunDirOutcome::Skipped,
    }
}

fn reclaim_both_halves(
    hooks: &mut dyn TopologyHooks,
    public: &Path,
    proof: PrivateHalfProof,
    retired: Option<PathBuf>,
) -> RunDirOutcome {
    if let Err(error) = rundir::remove_private_husk(proof, hooks.rundir()) {
        return unreclaimable(FailedStep::PrivateHalf, &error);
    }
    match rundir::remove_public_husk(public, hooks.rundir()) {
        Ok(()) => retired.map_or(RunDirOutcome::ReclaimedBothHalves, |root| {
            RunDirOutcome::ReclaimedRootAndBothHalves { root }
        }),
        Err(error) => unreclaimable(FailedStep::PublicHalfAfterPrivate, &error),
    }
}

#[derive(Debug)]
pub(crate) enum RootPlan {
    Nothing {
        left_alone: Vec<PathBuf>,
    },
    Settle {
        manager: Box<WorkspaceManager>,
        token: String,
        into: PathBuf,
        ours: bool,
        left_alone: Vec<PathBuf>,
    },
    Keep {
        root: Option<PathBuf>,
        detail: String,
    },
}

pub(crate) fn plan_root(
    public: &Path,
    run_id: &str,
    private: &Path,
    inputs: &CensusInputs<'_>,
) -> RootPlan {
    plan_root_at(
        public,
        run_id,
        private,
        inputs.repo_root,
        inputs.authorized_root,
    )
}

pub(crate) fn plan_root_at(
    public: &Path,
    run_id: &str,
    private: &Path,
    repo_root: &Path,
    authorized_root: &Path,
) -> RootPlan {
    if !could_have_a_root(run_id) {
        return RootPlan::Nothing {
            left_alone: Vec::new(),
        };
    }
    let base = match std::fs::canonicalize(authorized_root) {
        Ok(base) => base,
        Err(error) => {
            return RootPlan::Keep {
                root: None,
                detail: format!(
                    "the authorized private root {} cannot be resolved: {error}",
                    authorized_root.display()
                ),
            };
        }
    };
    let found = match roots_named(run_id, std::slice::from_ref(&base)) {
        Ok(found) => found,
        Err((root, detail)) => {
            return RootPlan::Keep {
                root: Some(root),
                detail,
            };
        }
    };
    let retired_tree = std::fs::symlink_metadata(private.join(RETIRED_ROOT)).is_ok();
    let key = match read_location_record(private) {
        LocationRecord::Keyed(key) => key,
        LocationRecord::Absent if !retired_tree => {
            return found.into_iter().next().map_or(
                RootPlan::Nothing {
                    left_alone: Vec::new(),
                },
                |root| RootPlan::Keep {
                    root: Some(root),
                    detail: "this run's private half records no location for an execution \
                             root, so nothing proves the directory is this run's: a run \
                             created before that record existed, or another actor's"
                        .to_owned(),
                },
            );
        }
        LocationRecord::Absent => {
            return RootPlan::Keep {
                root: found.into_iter().next(),
                detail: "this run's private half holds a retired execution root and no \
                         location record, so where that root was cannot be made durable"
                    .to_owned(),
            };
        }
        LocationRecord::Unreadable(why) => {
            if found.is_empty() && !retired_tree {
                return RootPlan::Nothing {
                    left_alone: Vec::new(),
                };
            }
            return RootPlan::Keep {
                root: found.into_iter().next(),
                detail: format!(
                    "this run's location record proves nothing ({why}), so nothing proves a \
                     directory is this run's or makes its state durable"
                ),
            };
        }
    };
    let root = base.join("workspaces").join(&key).join(run_id);
    let mut left_alone: Vec<PathBuf> = found
        .iter()
        .filter(|path| **path != root)
        .cloned()
        .collect();
    let incarnation = match marker_incarnation(public) {
        Ok(incarnation) => incarnation,
        Err(detail) => {
            return RootPlan::Keep {
                root: Some(root),
                detail,
            };
        }
    };
    let token = claim_token(&incarnation);
    let ours = if found.contains(&root) {
        match std::fs::symlink_metadata(root.join(&token)) {
            Err(absent) if absent.kind() == std::io::ErrorKind::NotFound => {
                left_alone.push(root.clone());
                false
            }
            Ok(found) if found.is_dir() => true,
            Ok(_) => {
                return RootPlan::Keep {
                    root: Some(root),
                    detail: format!("its entry {token} is not a claim token this run made"),
                };
            }
            Err(error) => {
                return RootPlan::Keep {
                    root: Some(root),
                    detail: format!("its claim token cannot be read: {error}"),
                };
            }
        }
    } else {
        false
    };
    let manager = match WorkspaceManager::derive(repo_root, authorized_root, run_id, &incarnation) {
        Ok(manager) => manager,
        Err(error) => {
            return RootPlan::Keep {
                root: Some(root),
                detail: format!(
                    "this run published its execution root under repository key {key}, and \
                     this command cannot derive the repository context to make that directory's \
                     state durable or to settle it: {error}"
                ),
            };
        }
    };
    if manager.repo_key() != key {
        return RootPlan::Keep {
            root: Some(root),
            detail: format!(
                "this run published its execution root under repository key {key}, and this \
                 command's effective common directory gives key {}: a write command in the \
                 repository context the run was created in settles it",
                manager.repo_key()
            ),
        };
    }
    if ours {
        if let Some(why) = unsettleable(manager.execution_root(), &token) {
            return RootPlan::Keep {
                root: Some(root),
                detail: why,
            };
        }
    }
    RootPlan::Settle {
        manager: Box::new(manager),
        token,
        into: private.join(RETIRED_ROOT),
        ours,
        left_alone,
    }
}

pub(crate) fn husk_root_view(
    repo_root: &Path,
    run_id: &str,
    repo_key: &RepoKey,
    authorized_root: &Path,
    disposition: &HuskDisposition,
) -> Option<String> {
    let public = rundir::public_dir(repo_root, run_id);
    let kept = |root: Option<PathBuf>, detail: String| {
        format!(
            "a retained husk, nothing deleted: {} ({detail}), so the husk stays as that \
             directory's locator",
            root.map_or_else(
                || "where this run's execution root is cannot be told".to_owned(),
                |root| format!("a directory at {} is kept", root.display())
            )
        )
    };
    let untouched = |left_alone: &[PathBuf]| {
        left_alone
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    };
    match disposition {
        HuskDisposition::Unstarted(Reclaimable::BothHalves) => {
            let PrivateHalfOwnership::Proven(proof) =
                rundir::prove_private_half_ownership(&public, repo_key, authorized_root)
            else {
                return None;
            };
            match plan_root_at(&public, run_id, proof.target(), repo_root, authorized_root) {
                RootPlan::Keep { root, detail } => Some(kept(root, detail)),
                RootPlan::Settle {
                    ours: true,
                    manager,
                    left_alone,
                    ..
                } => Some(format!(
                    "an unstarted husk that the next write command reclaims: it first retires \
                     the execution root {} this run published, then both halves{}",
                    manager.execution_root().display(),
                    if left_alone.is_empty() {
                        String::new()
                    } else {
                        format!(
                            "; it leaves {} untouched, because nothing proves them this run's",
                            untouched(&left_alone)
                        )
                    }
                )),
                RootPlan::Settle { left_alone, .. } | RootPlan::Nothing { left_alone }
                    if !left_alone.is_empty() =>
                {
                    Some(format!(
                        "an unstarted husk that the next write command reclaims, both halves; \
                         it leaves {} untouched, because nothing proves them this run's",
                        untouched(&left_alone)
                    ))
                }
                RootPlan::Settle { .. } | RootPlan::Nothing { .. } => None,
            }
        }
        HuskDisposition::Unstarted(Reclaimable::PublicOnly(UnboundShape::TargetAbsent)) => {
            unbound_root(&public, run_id, authorized_root).map(|(root, detail)| kept(root, detail))
        }
        HuskDisposition::Unstarted(Reclaimable::PublicOnly(_)) | HuskDisposition::Retained(_) => {
            None
        }
    }
}

fn unbound_unless_rooted(
    public: &Path,
    run_id: &str,
    inputs: &CensusInputs<'_>,
    shape: UnboundShape,
) -> Planned {
    if shape != UnboundShape::TargetAbsent {
        return Planned::ReclaimPublicOnly(shape);
    }
    match unbound_root(public, run_id, inputs.authorized_root) {
        Some((root, detail)) => Planned::KeepForRoot { root, detail },
        None => Planned::ReclaimPublicOnly(shape),
    }
}

pub(crate) fn unbound_root(
    public: &Path,
    run_id: &str,
    authorized_root: &Path,
) -> Option<(Option<PathBuf>, String)> {
    if !could_have_a_root(run_id) {
        return None;
    }
    let mut bases: Vec<PathBuf> = std::fs::canonicalize(authorized_root).into_iter().collect();
    if let Some(own) = marker_private_root(public, run_id) {
        if !bases.contains(&own) {
            bases.push(own);
        }
    }
    match roots_named(run_id, &bases) {
        Ok(found) => found.into_iter().next().map(|root| {
            (
                Some(root),
                "no private half is bound to this husk any more, so no location record says \
                 whose the directory is"
                    .to_owned(),
            )
        }),
        Err((root, detail)) => Some((Some(root), detail)),
    }
}

fn roots_named(run_id: &str, bases: &[PathBuf]) -> Result<Vec<PathBuf>, (PathBuf, String)> {
    let mut found = Vec::new();
    for base in bases {
        let workspaces = base.join("workspaces");
        let listed = match std::fs::read_dir(&workspaces) {
            Ok(listed) => listed,
            Err(absent) if absent.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err((
                    workspaces,
                    format!("where this run's execution root could be cannot be listed: {error}"),
                ));
            }
        };
        for entry in listed {
            let entry = entry.map_err(|error| {
                (
                    workspaces.clone(),
                    format!("where this run's execution root could be cannot be listed: {error}"),
                )
            })?;
            let candidate = entry.path().join(run_id);
            match std::fs::symlink_metadata(&candidate) {
                Ok(_) => found.push(candidate),
                Err(absent)
                    if matches!(
                        absent.kind(),
                        std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
                    ) => {}
                Err(_) => found.push(candidate),
            }
        }
    }
    found.sort();
    Ok(found)
}

fn marker_incarnation(public: &Path) -> Result<String, String> {
    std::fs::read_to_string(public.join(rundir::MARKER))
        .map_err(|error| error.to_string())
        .and_then(|text| {
            serde_json::from_str::<rundir::CreatingMarker>(&text).map_err(|error| error.to_string())
        })
        .map(|marker| marker.incarnation)
        .map_err(|error| format!("its marker cannot be read again: {error}"))
}

fn marker_private_root(public: &Path, run_id: &str) -> Option<PathBuf> {
    let text = std::fs::read_to_string(public.join(rundir::MARKER)).ok()?;
    let marker = serde_json::from_str::<rundir::CreatingMarker>(&text).ok()?;
    let locator = PathBuf::from(marker.private_dir);
    let runs = locator.parent()?;
    let shaped = locator.file_name() == Some(std::ffi::OsStr::new(run_id))
        && runs.file_name() == Some(std::ffi::OsStr::new("runs"));
    if !shaped {
        return None;
    }
    std::fs::canonicalize(runs.parent()?).ok()
}

fn could_have_a_root(run_id: &str) -> bool {
    const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    run_id.len() == 26
        && run_id.bytes().all(|byte| CROCKFORD.contains(&byte))
        && run_id
            .as_bytes()
            .first()
            .is_some_and(|first| (b'0'..=b'7').contains(first))
}

fn unreclaimable(step: FailedStep, error: &UpstrokeError) -> RunDirOutcome {
    RunDirOutcome::Unreclaimable {
        step,
        detail: error.to_string(),
    }
}

fn stale_marker_present(public: &Path) -> bool {
    std::fs::symlink_metadata(public.join(rundir::MARKER)).is_ok()
        || std::fs::symlink_metadata(public.join(rundir::MARKER_STAGED)).is_ok()
}

mod witness {
    pub use fresh::FreshCensused;
    pub use locked::WorktreeLocked;

    mod locked {
        use crate::rundir::WorktreeLock;

        #[derive(Debug)]
        pub struct WorktreeLocked {
            #[allow(dead_code)]
            lock: WorktreeLock,
        }

        impl WorktreeLocked {
            #[must_use]
            pub fn from(lock: WorktreeLock) -> Self {
                Self { lock }
            }

            #[must_use]
            #[allow(dead_code)]
            pub const fn lock(&self) -> &WorktreeLock {
                &self.lock
            }

            #[must_use]
            pub fn into_lock(self) -> WorktreeLock {
                self.lock
            }
        }
    }

    mod fresh {
        use super::super::{CensusInputs, StartupCensus, both_halves};
        use super::locked::WorktreeLocked;
        use crate::engine::topology::seams::TopologyHooks;
        use crate::error::UpstrokeError;
        use crate::runner::container::census::CensusStart;

        #[derive(Debug)]
        pub struct FreshCensused {
            locked: WorktreeLocked,
            census: StartupCensus,
        }

        impl FreshCensused {
            pub(in crate::engine::topology::startup) fn establish(
                locked: WorktreeLocked,
                hooks: &mut dyn TopologyHooks,
                inputs: &CensusInputs<'_>,
            ) -> Result<Self, UpstrokeError> {
                let start = CensusStart::FreshRun {
                    incarnation: inputs.incarnation.to_owned(),
                };
                let census = both_halves(hooks, inputs, &start)?;
                Ok(Self { locked, census })
            }

            #[must_use]
            pub const fn census(&self) -> &StartupCensus {
                &self.census
            }

            #[must_use]
            #[allow(dead_code)]
            pub const fn locked(&self) -> &WorktreeLocked {
                &self.locked
            }

            #[must_use]
            pub fn into_parts(self) -> (WorktreeLocked, StartupCensus) {
                (self.locked, self.census)
            }
        }
    }
}

#[cfg(test)]
mod tests;
