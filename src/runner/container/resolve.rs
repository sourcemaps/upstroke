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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepositoryGit<'a> {
    pub git_dir: &'a Path,
    pub common_dir: &'a Path,
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

    pub fn bind(
        repo_root: &Path,
        git: RepositoryGit<'_>,
        private_root: &Path,
    ) -> Result<Self, UpstrokeError> {
        Self::bind_with(repo_root, git, private_root, &host_credential_locations())
    }

    pub fn bind_with(
        repo_root: &Path,
        git: RepositoryGit<'_>,
        private_root: &Path,
        credential_locations: &[PathBuf],
    ) -> Result<Self, UpstrokeError> {
        let root = canonical(repo_root).map_err(|source| UpstrokeError::Io {
            path: repo_root.to_path_buf(),
            source,
        })?;
        if let Some(relative) = [git.git_dir, git.common_dir]
            .into_iter()
            .find(|directory| !directory.is_absolute())
        {
            return Err(UpstrokeError::Refused {
                message: format!(
                    "[runner] mounts are configured and `{}`, a Git directory of the repository \
                     this run integrates into, is not an absolute path, so whether a mount \
                     reaches it cannot be decided",
                    relative.display()
                ),
            });
        }
        let mut excluded = vec![
            (MountExclusion::RepositoryRoot, root),
            (
                MountExclusion::AuthoritativeGit,
                canonical_spelling(git.git_dir),
            ),
            (
                MountExclusion::AuthoritativeGit,
                canonical_spelling(git.common_dir),
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
    pub fn into_excluded(self) -> Vec<(MountExclusion, PathBuf)> {
        self.excluded.unwrap_or_default()
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
        if let Ok(mut resolved) = canonical(existing) {
            resolved.extend(below.iter().rev());
            return resolved;
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

pub fn canonical(path: &Path) -> io::Result<PathBuf> {
    std::fs::canonicalize(path).map(without_verbatim_disk_prefix)
}

#[cfg(windows)]
fn without_verbatim_disk_prefix(path: PathBuf) -> PathBuf {
    let plain = path
        .to_str()
        .and_then(|spelled| spelled.strip_prefix(r"\\?\"))
        .filter(|rest| is_drive_absolute(rest))
        .map(PathBuf::from);
    plain.unwrap_or(path)
}

#[cfg(not(windows))]
const fn without_verbatim_disk_prefix(path: PathBuf) -> PathBuf {
    path
}

fn is_drive_absolute(spelled: &str) -> bool {
    matches!(
        spelled.as_bytes(),
        [letter, b':', b'\\', ..] if letter.is_ascii_alphabetic()
    )
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
    let resolved = canonical(source).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            "its source does not exist".to_owned()
        } else {
            format!("its source cannot be resolved: {error}")
        }
    })?;
    let Some(spelled) = resolved.to_str() else {
        return Err(format!(
            "its source resolves to `{}`, which is not valid UTF-8, so the runtime's mount \
             option cannot carry it exactly",
            resolved.display()
        ));
    };
    if cfg!(windows) && !is_drive_absolute(spelled) {
        return Err(format!(
            "its source resolves to `{spelled}`, which is not a path on a drive letter; a \
             network share or a device path is not mounted"
        ));
    }
    if let Some(reason) = super::mount_option_refusal(spelled) {
        return Err(format!(
            "its source resolves to `{spelled}`, and that spelling {reason}"
        ));
    }
    let identity = source_identity(&resolved)?;
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
    Ok(MountSourceIdentity::Unix {
        kind,
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}

#[cfg(windows)]
fn source_identity(canonical: &Path) -> Result<MountSourceIdentity, String> {
    use std::os::windows::ffi::OsStrExt as _;
    use std::os::windows::io::{AsRawHandle as _, FromRawHandle as _, OwnedHandle};

    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, CreateFileW, FILE_ATTRIBUTE_DEVICE, FILE_ATTRIBUTE_DIRECTORY,
        FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
        FILE_ID_INFO, FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
        FileIdInfo, GetFileInformationByHandle, GetFileInformationByHandleEx, OPEN_EXISTING,
    };

    use crate::topology::events::MountSourceKind;

    let mut wide: Vec<u16> = canonical.as_os_str().encode_wide().collect();
    if wide.contains(&0) {
        return Err("its source path holds a NUL, so it cannot be opened".to_owned());
    }
    wide.push(0);
    // SAFETY: `wide` is a live NUL-terminated UTF-16 path that outlives the call, and the two
    // pointer arguments are the documented "no security attributes" and "no template" nulls.
    // The access is FILE_READ_ATTRIBUTES and the disposition OPEN_EXISTING, so nothing is
    // created, written or truncated; FILE_FLAG_OPEN_REPARSE_POINT opens a reparse point itself
    // rather than what it names, and FILE_FLAG_BACKUP_SEMANTICS lets a directory be opened.
    let raw = unsafe {
        CreateFileW(
            wide.as_ptr(),
            FILE_READ_ATTRIBUTES,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            std::ptr::null_mut(),
        )
    };
    if raw == INVALID_HANDLE_VALUE {
        return Err(format!(
            "its source cannot be opened to read its identity: {}",
            io::Error::last_os_error()
        ));
    }
    // SAFETY: `raw` is the valid handle CreateFileW just returned and nothing else owns it; the
    // OwnedHandle closes it exactly once, when it drops at the end of this function.
    let handle = unsafe { OwnedHandle::from_raw_handle(raw) };

    let mut information = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: the handle is live for the whole call, and `information` is an initialized,
    // writable BY_HANDLE_FILE_INFORMATION that only this call writes.
    if unsafe { GetFileInformationByHandle(handle.as_raw_handle(), &mut information) } == 0 {
        return Err(format!(
            "its source's attributes cannot be read: {}",
            io::Error::last_os_error()
        ));
    }
    let attributes = information.dwFileAttributes;
    if attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(
            "its source is a link, junction or other reparse point, or became one while it was \
             being resolved"
                .to_owned(),
        );
    }
    if attributes & FILE_ATTRIBUTE_DEVICE != 0 {
        return Err("its source is a device, neither a directory nor a regular file".to_owned());
    }
    let kind = if attributes & FILE_ATTRIBUTE_DIRECTORY == 0 {
        MountSourceKind::File
    } else {
        MountSourceKind::Directory
    };

    let mut id = FILE_ID_INFO::default();
    let length = u32::try_from(std::mem::size_of::<FILE_ID_INFO>())
        .map_err(|_| "FILE_ID_INFO exceeds the Windows buffer-size field".to_owned())?;
    // SAFETY: the handle is live for the whole call. The writable, initialized buffer is exactly
    // one FILE_ID_INFO, with the FileIdInfo class and that struct's byte length; no pointer
    // escapes and no ownership is transferred.
    let read = unsafe {
        GetFileInformationByHandleEx(
            handle.as_raw_handle(),
            FileIdInfo,
            std::ptr::from_mut(&mut id).cast(),
            length,
        )
    };
    if read == 0 {
        return Err(format!(
            "its source's file id cannot be read: {}",
            io::Error::last_os_error()
        ));
    }
    Ok(MountSourceIdentity::Windows {
        kind,
        volume_serial: id.VolumeSerialNumber,
        file_id: id
            .FileId
            .Identifier
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    })
}

#[cfg(not(any(unix, windows)))]
fn source_identity(_canonical: &Path) -> Result<MountSourceIdentity, String> {
    Err(
        "this platform's build records no file identity for a mount source (Unix records the \
         device and inode, Windows the volume and the 128-bit file id), and a mount is recorded \
         with one, so `[runner] mounts` is refused here"
            .to_owned(),
    )
}

#[must_use]
pub fn described(identity: &MountSourceIdentity) -> String {
    match identity {
        MountSourceIdentity::Unix {
            kind,
            device,
            inode,
        } => format!("{kind:?}, device {device}, inode {inode}"),
        MountSourceIdentity::Windows {
            kind,
            volume_serial,
            file_id,
        } => format!("{kind:?}, volume {volume_serial:#018x}, file id {file_id}"),
    }
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
    let resolved = canonical(recorded).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            "its source is gone".to_owned()
        } else {
            format!("its source cannot be resolved: {error}")
        }
    })?;
    if resolved != recorded {
        return Err(format!(
            "its source path now resolves to `{}`, so a link or a move rebound it",
            resolved.display()
        ));
    }
    let now = source_identity(&resolved)?;
    if now != mount.source_identity {
        return Err(format!(
            "its source path now names another object ({}; the run recorded {})",
            described(&now),
            described(&mount.source_identity)
        ));
    }
    Ok(())
}

/// Every recorded mount of `record` still names the object the run recorded (DESIGN §26).
///
/// Read-only: it resolves and identifies each source and changes nothing. So recovery asks it
/// before any recovery effect, and every rebuild of the runner asks it again.
pub fn recheck_recorded_mounts(record: &RunnerPolicy) -> Result<(), InspectionRefusal> {
    for mount in &record.mounts {
        recheck_recorded_source(mount).map_err(|reason| InspectionRefusal::MountSourceMoved {
            mount: mount_label(&mount.source, &mount.target),
            reason,
        })?;
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
    recheck_recorded_mounts(record)?;

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
        let spelled = canonical(&mount.source)
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
pub(crate) mod fixtures {
    use std::path::Path;

    use super::RepositoryGit;

    pub(crate) const fn plain_checkout(git_dir: &Path) -> RepositoryGit<'_> {
        RepositoryGit {
            git_dir,
            common_dir: git_dir,
        }
    }
}

#[cfg(test)]
mod tests;
