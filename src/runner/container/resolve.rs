//! Extended notes: `docs/internals/runner/container/resolve.md`

#![cfg_attr(
    not(test),
    forbid(clippy::disallowed_methods, clippy::disallowed_types)
)]
#![forbid(clippy::disallowed_macros)]
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use thiserror::Error;

use super::env::BoundaryLayout;
use super::runtime::{ContainerRuntime, ImageInspection, RuntimeError, RuntimeOp};
use crate::config::{RunnerMount, RunnerSelection};
use crate::error::UpstrokeError;
use crate::topology::events::{
    ImageIdentity, MountSourceIdentity, RecordedMount, RunnerContract, RunnerField, RunnerKind,
    RunnerPolicy, RunnerRecordDefect,
};

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InspectionRefusal {
    #[error(
        "the container runtime cannot be reached for `{operation}` ({detail}); the runner this \
         run needs cannot be established without it"
    )]
    RuntimeUnavailable {
        operation: RuntimeOp,
        detail: String,
    },
    #[error("the container runtime refused `{operation}` ({detail})")]
    RuntimeFailed {
        operation: RuntimeOp,
        detail: String,
    },
    #[error(
        "the container runtime does not hold the image reference `{reference}`; nothing is \
         pulled implicitly, so pull or build it before the run"
    )]
    ImageReferenceAbsent { reference: String },
    #[error(
        "the container runtime no longer holds the recorded image id `{id}`; this run records \
         that id as its execution identity and creates every container from it, so it cannot \
         continue until the runtime holds it again"
    )]
    ImageIdAbsent { id: String },
    #[error("the container runtime resolved `{reference}` to an image it reported no id for")]
    ImageNotIdentified { reference: String },
    #[error(
        "the per-agent credential volume `{volume}` for agent `{agent}` does not exist; \
         credential volumes are operator-owned and a run never creates one"
    )]
    CredentialVolumeAbsent { agent: String, volume: String },
    #[error("[runner] selects the `{kind:?}` runner, so there is no container policy to resolve")]
    NotAContainerSelection { kind: RunnerKind },
    #[error("[runner] selects the container runner without an image reference to resolve")]
    SelectionWithoutImage,
    #[error("the resolved container runner is not a usable RunnerPolicy: {0}")]
    RecordIncomplete(RunnerRecordDefect),
    #[error("[runner] the mount {mount} is refused: {reason}")]
    MountRefused { mount: String, reason: String },
    #[error(
        "the recorded mount {mount} cannot be re-established: {reason}. A run keeps the mounts it \
         started with, so it cannot continue until that source is back as recorded"
    )]
    MountSourceMoved { mount: String, reason: String },
}

impl InspectionRefusal {
    #[must_use]
    pub const fn is_runtime_unavailable(&self) -> bool {
        matches!(
            self,
            Self::RuntimeUnavailable { .. } | Self::RuntimeFailed { .. }
        )
    }

    fn from_runtime(error: RuntimeError) -> Self {
        match error {
            RuntimeError::Unreachable { operation, detail } => {
                Self::RuntimeUnavailable { operation, detail }
            }
            RuntimeError::Failed { operation, detail }
            | RuntimeError::Unresolved { operation, detail } => {
                Self::RuntimeFailed { operation, detail }
            }
        }
    }
}

impl From<InspectionRefusal> for UpstrokeError {
    fn from(refusal: InspectionRefusal) -> Self {
        Self::Refused {
            message: refusal.to_string(),
        }
    }
}

#[derive(Debug, Error)]
pub enum RebuildRefusal {
    #[error("the recorded container runner cannot be re-established: {0}")]
    Inspection(#[from] InspectionRefusal),
    #[error("the recorded container runner's RunnerPreflight refused: {0}")]
    Preflight(#[source] UpstrokeError),
}

impl RebuildRefusal {
    #[must_use]
    pub const fn before_any_spawn(&self) -> bool {
        matches!(self, Self::Inspection(_))
    }
}

impl From<RebuildRefusal> for UpstrokeError {
    fn from(refusal: RebuildRefusal) -> Self {
        Self::Refused {
            message: refusal.to_string(),
        }
    }
}

pub trait RunnerPreflight {
    fn certify(&self, policy: &RunnerPolicy) -> Result<(), UpstrokeError>;
}

pub const BOUNDARY_ROOT: &str = "/upstroke";

pub const DEFAULT_CREDENTIAL_HOMES: [&str; 4] = [".claude", ".claude.json", ".codex", ".copilot"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MountExclusion {
    RepositoryRoot,
    AuthoritativeGit,
    PrivateRoot,
    CredentialLocation,
}

impl MountExclusion {
    #[must_use]
    pub const fn describe(self) -> &'static str {
        match self {
            Self::RepositoryRoot => "the repository this run integrates into",
            Self::AuthoritativeGit => {
                "authoritative Git, which never crosses the boundary (DESIGN.md:612)"
            }
            Self::PrivateRoot => {
                "the private root, which holds every run's private artifacts, worktrees and \
                 container intents (DESIGN.md:400)"
            }
            Self::CredentialLocation => {
                "a host credential location; a per-agent credential volume is the only \
                 credential channel into a container"
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountContext {
    excluded: Option<Vec<(MountExclusion, PathBuf)>>,
}

impl MountContext {
    #[must_use]
    pub const fn unbound() -> Self {
        Self { excluded: None }
    }

    pub fn for_selection(
        selection: &RunnerSelection,
        repo_root: &Path,
        private_root: &Path,
    ) -> Result<Self, UpstrokeError> {
        if selection.mounts.is_empty() {
            return Ok(Self::unbound());
        }
        Self::bind(repo_root, private_root)
    }

    pub fn bind(repo_root: &Path, private_root: &Path) -> Result<Self, UpstrokeError> {
        Self::bind_with(repo_root, private_root, &host_credential_locations())
    }

    pub fn bind_with(
        repo_root: &Path,
        private_root: &Path,
        credential_locations: &[PathBuf],
    ) -> Result<Self, UpstrokeError> {
        let root = std::fs::canonicalize(repo_root).map_err(|source| UpstrokeError::Io {
            path: repo_root.to_path_buf(),
            source,
        })?;
        let Some(layout) = super::view::resolve(&root)? else {
            return Err(UpstrokeError::Refused {
                message: format!(
                    "[runner] mounts are configured and `{}` has no `.git`, so the repository's \
                     Git directories, which a mount must never reach, cannot be found",
                    root.display()
                ),
            });
        };
        let mut excluded = vec![
            (MountExclusion::RepositoryRoot, root),
            (
                MountExclusion::AuthoritativeGit,
                canonical_spelling(&layout.git_dir),
            ),
            (
                MountExclusion::AuthoritativeGit,
                canonical_spelling(&layout.common_dir),
            ),
            (
                MountExclusion::PrivateRoot,
                canonical_spelling(private_root),
            ),
        ];
        for location in credential_locations {
            excluded.push((
                MountExclusion::CredentialLocation,
                canonical_spelling(location),
            ));
        }
        Ok(Self {
            excluded: Some(excluded),
        })
    }

    #[must_use]
    pub fn excluded(&self) -> &[(MountExclusion, PathBuf)] {
        self.excluded.as_deref().unwrap_or_default()
    }

    #[must_use]
    pub const fn is_bound(&self) -> bool {
        self.excluded.is_some()
    }
}

#[must_use]
pub fn host_credential_locations() -> Vec<PathBuf> {
    let mut locations: Vec<PathBuf> = crate::runner::host::CREDENTIAL_LOCATIONS
        .iter()
        .filter_map(|(_, key)| std::env::var_os(key))
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .collect();
    if let Some(home) = crate::util::user_upstroke_dir()
        .as_deref()
        .and_then(Path::parent)
    {
        locations.extend(DEFAULT_CREDENTIAL_HOMES.iter().map(|name| home.join(name)));
    }
    locations
}

#[must_use]
pub fn canonical_spelling(path: &Path) -> PathBuf {
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let mut existing = absolute.as_path();
    let mut below = Vec::new();
    loop {
        if let Ok(mut canonical) = std::fs::canonicalize(existing) {
            canonical.extend(below.iter().rev());
            return canonical;
        }
        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) => {
                below.push(name.to_os_string());
                existing = parent;
            }
            _ => return absolute,
        }
    }
}

#[must_use]
pub fn mount_label(source: &str, target: &str) -> String {
    format!("`{source}` -> `{target}`")
}

#[must_use]
pub fn overlap(source: &Path, excluded: &Path) -> Option<&'static str> {
    if source == excluded {
        Some("is")
    } else if excluded.starts_with(source) {
        Some("contains")
    } else if source.starts_with(excluded) {
        Some("lies inside")
    } else {
        None
    }
}

pub fn refuse_excluded(
    source: &Path,
    excluded: &[(MountExclusion, PathBuf)],
) -> Result<(), String> {
    for (category, path) in excluded {
        if let Some(relation) = overlap(source, path) {
            return Err(format!(
                "its source {relation} `{}`, {}",
                path.display(),
                category.describe()
            ));
        }
    }
    Ok(())
}

pub fn refuse_target(target: &str) -> Result<(), String> {
    if let Some(reason) = super::mount_option_refusal(target) {
        return Err(format!("its target {reason}"));
    }
    let Some(below_root) = target.strip_prefix('/') else {
        return Err(
            "its target is not an absolute path; a target names a path inside the container"
                .to_owned(),
        );
    };
    if below_root.is_empty() {
        return Err("its target is the container's root, which no mount may replace".to_owned());
    }
    if below_root
        .split('/')
        .any(|name| name.is_empty() || name == "." || name == "..")
    {
        return Err(
            "its target is not in normal form; spell it without an empty, `.` or `..` \
             component and without a trailing `/`"
                .to_owned(),
        );
    }
    for reserved in [BOUNDARY_ROOT, BoundaryLayout::DEFAULT_SCRATCH] {
        if target == reserved
            || target
                .strip_prefix(reserved)
                .is_some_and(|rest| rest.starts_with('/'))
        {
            return Err(format!(
                "its target is at or under `{reserved}`, which the runner reserves for the \
                 workspace, Git view, objects, credentials, settings and scratch it composes"
            ));
        }
    }
    Ok(())
}

#[must_use]
pub fn nested_targets(first: &str, second: &str) -> bool {
    first == second
        || second
            .strip_prefix(first)
            .is_some_and(|rest| rest.starts_with('/'))
        || first
            .strip_prefix(second)
            .is_some_and(|rest| rest.starts_with('/'))
}

fn resolve_source(source: &Path) -> Result<(String, MountSourceIdentity), String> {
    if !source.is_absolute() {
        return Err(format!(
            "its source `{}` is not an absolute host path, and a relative one would depend on \
             the directory `upstroke` was started from",
            source.display()
        ));
    }
    let canonical = std::fs::canonicalize(source).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            "its source does not exist".to_owned()
        } else {
            format!("its source cannot be resolved: {error}")
        }
    })?;
    let Some(spelled) = canonical.to_str() else {
        return Err(format!(
            "its source resolves to `{}`, which is not valid UTF-8, so the runtime's mount \
             option cannot carry it exactly",
            canonical.display()
        ));
    };
    if let Some(reason) = super::mount_option_refusal(spelled) {
        return Err(format!(
            "its source resolves to `{spelled}`, and that spelling {reason}"
        ));
    }
    let identity = source_identity(&canonical)?;
    Ok((spelled.to_owned(), identity))
}

#[cfg(unix)]
fn source_identity(canonical: &Path) -> Result<MountSourceIdentity, String> {
    use std::os::unix::fs::MetadataExt as _;

    use crate::topology::events::MountSourceKind;
    let metadata = std::fs::symlink_metadata(canonical)
        .map_err(|error| format!("its source cannot be inspected: {error}"))?;
    let kind = if metadata.file_type().is_dir() {
        MountSourceKind::Directory
    } else if metadata.file_type().is_file() {
        MountSourceKind::File
    } else if metadata.file_type().is_symlink() {
        return Err("its source changed into a link while it was being resolved".to_owned());
    } else {
        return Err(
            "its source is neither a directory nor a regular file; a socket, device or FIFO is \
             never mounted"
                .to_owned(),
        );
    };
    Ok(MountSourceIdentity {
        kind,
        device: metadata.dev(),
        object: metadata.ino(),
    })
}

#[cfg(not(unix))]
fn source_identity(_canonical: &Path) -> Result<MountSourceIdentity, String> {
    Err(
        "this platform's build records no file identity for a mount source, and a mount is \
         recorded with one, so `[runner] mounts` is refused here"
            .to_owned(),
    )
}

pub fn resolve_mounts(
    configured: &[RunnerMount],
    context: &MountContext,
) -> Result<Vec<RecordedMount>, InspectionRefusal> {
    if configured.is_empty() {
        return Ok(Vec::new());
    }
    let mut recorded = Vec::with_capacity(configured.len());
    for mount in configured {
        let refused = |reason: String| InspectionRefusal::MountRefused {
            mount: mount_label(&mount.source.display().to_string(), &mount.target),
            reason,
        };
        if !context.is_bound() {
            return Err(refused(
                "no repository context was bound to check it against, so no configured mount \
                 is accepted"
                    .to_owned(),
            ));
        }
        refuse_target(&mount.target).map_err(refused)?;
        let (source, source_identity) = resolve_source(&mount.source).map_err(refused)?;
        refuse_excluded(Path::new(&source), context.excluded()).map_err(refused)?;
        recorded.push(RecordedMount {
            source,
            target: mount.target.clone(),
            read_only: mount.read_only,
            source_identity,
        });
    }
    recorded.sort_by(|left, right| left.target.cmp(&right.target));
    for (index, first) in recorded.iter().enumerate() {
        for second in recorded.iter().skip(index + 1) {
            if nested_targets(&first.target, &second.target) {
                return Err(InspectionRefusal::MountRefused {
                    mount: mount_label(&second.source, &second.target),
                    reason: format!(
                        "its target and that of {} are the same path or one holds the other, so \
                         which source the container sees there would depend on mount order",
                        mount_label(&first.source, &first.target)
                    ),
                });
            }
        }
    }
    Ok(recorded)
}

pub fn recheck_recorded_source(mount: &RecordedMount) -> Result<(), String> {
    let recorded = Path::new(&mount.source);
    let canonical = std::fs::canonicalize(recorded).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            "its source is gone".to_owned()
        } else {
            format!("its source cannot be resolved: {error}")
        }
    })?;
    if canonical != recorded {
        return Err(format!(
            "its source path now resolves to `{}`, so a link or a move rebound it",
            canonical.display()
        ));
    }
    let now = source_identity(&canonical)?;
    if now != mount.source_identity {
        return Err(format!(
            "its source path now names another object ({:?}, device {}, object {}; the run \
             recorded {:?}, device {}, object {})",
            now.kind,
            now.device,
            now.object,
            mount.source_identity.kind,
            mount.source_identity.device,
            mount.source_identity.object
        ));
    }
    Ok(())
}

pub fn resolve_container(
    runtime: &dyn ContainerRuntime,
    selection: &RunnerSelection,
    context: &MountContext,
) -> Result<RunnerPolicy, InspectionRefusal> {
    if selection.kind != RunnerKind::Container {
        return Err(InspectionRefusal::NotAContainerSelection {
            kind: selection.kind,
        });
    }
    let Some(reference) = selection.image.as_deref() else {
        return Err(InspectionRefusal::SelectionWithoutImage);
    };
    let mounts = resolve_mounts(&selection.mounts, context)?;
    runtime.probe().map_err(InspectionRefusal::from_runtime)?;
    let inspection = runtime
        .image_by_reference(reference)
        .map_err(InspectionRefusal::from_runtime)?
        .ok_or_else(|| InspectionRefusal::ImageReferenceAbsent {
            reference: reference.to_owned(),
        })?;
    if inspection.id.is_empty() {
        return Err(InspectionRefusal::ImageNotIdentified {
            reference: reference.to_owned(),
        });
    }
    inspect_volumes(runtime, &selection.credential_volumes)?;
    let policy = RunnerPolicy {
        kind: RunnerKind::Container,
        policy: RunnerContract::ContainerV1,
        image: Some(ImageIdentity {
            reference: reference.to_owned(),
            id: inspection.id.clone(),
            digest: reported_digest(&inspection),
        }),
        credential_volumes: Some(selection.credential_volumes.clone()),
        mounts,
    };
    policy
        .completeness()
        .map_err(InspectionRefusal::RecordIncomplete)?;
    Ok(policy)
}

fn reported_digest(inspection: &ImageInspection) -> Option<String> {
    inspection
        .digest
        .as_ref()
        .filter(|digest| !digest.is_empty())
        .cloned()
}

fn inspect_volumes(
    runtime: &dyn ContainerRuntime,
    volumes: &BTreeMap<String, String>,
) -> Result<(), InspectionRefusal> {
    for (agent, volume) in volumes {
        let present = runtime
            .volume_present(volume)
            .map_err(InspectionRefusal::from_runtime)?;
        if !present {
            return Err(InspectionRefusal::CredentialVolumeAbsent {
                agent: agent.clone(),
                volume: volume.clone(),
            });
        }
    }
    Ok(())
}

pub fn rebuild_by_inspection(
    runtime: &dyn ContainerRuntime,
    record: &RunnerPolicy,
    today: &RunnerSelection,
    warnings: &mut Vec<String>,
) -> Result<RunnerPolicy, InspectionRefusal> {
    if record.kind != RunnerKind::Container {
        return Err(InspectionRefusal::NotAContainerSelection { kind: record.kind });
    }
    record
        .completeness()
        .map_err(InspectionRefusal::RecordIncomplete)?;
    let image = record
        .image
        .as_ref()
        .ok_or(InspectionRefusal::RecordIncomplete(
            RunnerRecordDefect::ContainerWithoutImage,
        ))?;
    for mount in &record.mounts {
        recheck_recorded_source(mount).map_err(|reason| InspectionRefusal::MountSourceMoved {
            mount: mount_label(&mount.source, &mount.target),
            reason,
        })?;
    }

    runtime.probe().map_err(InspectionRefusal::from_runtime)?;
    if runtime
        .image_by_id(&image.id)
        .map_err(InspectionRefusal::from_runtime)?
        .is_none()
    {
        return Err(InspectionRefusal::ImageIdAbsent {
            id: image.id.clone(),
        });
    }
    inspect_volumes(
        runtime,
        record.credential_volumes.as_ref().unwrap_or(&EMPTY),
    )?;

    match runtime
        .image_by_reference(&image.reference)
        .map_err(InspectionRefusal::from_runtime)?
    {
        Some(now) if now.id != image.id => warnings.push(format!(
            "[runner] the recorded image reference `{}` now names image `{}` in the container \
             runtime; this run continues from its recorded image id `{}`, so a moved reference \
             cannot change what executes",
            image.reference, now.id, image.id
        )),
        None => warnings.push(format!(
            "[runner] the recorded image reference `{}` no longer resolves in the container \
             runtime; this run continues from its recorded image id `{}`, which the runtime \
             still holds",
            image.reference, image.id
        )),
        Some(_) => {}
    }
    if let Some(field) = configured_difference(record, today) {
        warnings.push(format!(
            "[runner] in the config differs from the runner this run recorded: {field}. A run \
             keeps the boundary and image it started with, so the recorded runner is rebuilt and \
             the configured one is ignored.",
        ));
    }
    Ok(record.clone())
}

pub fn rebuild_from_record(
    runtime: &dyn ContainerRuntime,
    record: &RunnerPolicy,
    today: &RunnerSelection,
    preflight: &dyn RunnerPreflight,
    warnings: &mut Vec<String>,
) -> Result<RunnerPolicy, RebuildRefusal> {
    let rebuilt = rebuild_by_inspection(runtime, record, today, warnings)?;
    preflight
        .certify(&rebuilt)
        .map_err(RebuildRefusal::Preflight)?;
    Ok(rebuilt)
}

static EMPTY: BTreeMap<String, String> = BTreeMap::new();

fn configured_difference(record: &RunnerPolicy, today: &RunnerSelection) -> Option<RunnerField> {
    record
        .difference(&today_in_record_shape(record, today))
        .or_else(|| {
            (!same_configured_mounts(&record.mounts, &today.mounts)).then_some(RunnerField::Mounts)
        })
}

fn same_configured_mounts(recorded: &[RecordedMount], configured: &[RunnerMount]) -> bool {
    if recorded.len() != configured.len() {
        return false;
    }
    let mut today: Vec<(String, &str, bool)> = Vec::with_capacity(configured.len());
    for mount in configured {
        let spelled = std::fs::canonicalize(&mount.source)
            .ok()
            .and_then(|canonical| canonical.to_str().map(str::to_owned))
            .or_else(|| mount.source.to_str().map(str::to_owned));
        let Some(spelled) = spelled else {
            return false;
        };
        today.push((spelled, mount.target.as_str(), mount.read_only));
    }
    today.sort_by(|left, right| left.1.cmp(right.1));
    recorded
        .iter()
        .zip(&today)
        .all(|(mount, (source, target, read_only))| {
            mount.source == *source && mount.target == *target && mount.read_only == *read_only
        })
}

fn today_in_record_shape(record: &RunnerPolicy, today: &RunnerSelection) -> RunnerPolicy {
    match today.kind {
        RunnerKind::Host => RunnerPolicy {
            kind: RunnerKind::Host,
            policy: RunnerContract::HostV1,
            image: None,
            credential_volumes: None,
            mounts: Vec::new(),
        },
        RunnerKind::Container => RunnerPolicy {
            kind: RunnerKind::Container,
            policy: RunnerContract::ContainerV1,
            image: Some(ImageIdentity {
                reference: today.image.clone().unwrap_or_default(),
                id: record
                    .image
                    .as_ref()
                    .map(|image| image.id.clone())
                    .unwrap_or_default(),
                digest: record.image.as_ref().and_then(|image| image.digest.clone()),
            }),
            credential_volumes: Some(today.credential_volumes.clone()),
            mounts: record.mounts.clone(),
        },
    }
}

#[cfg(test)]
mod tests;
