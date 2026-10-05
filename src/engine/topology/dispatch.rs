//! Extended notes: `docs/internals/engine/topology/dispatch.md`

use std::path::{Path, PathBuf};

use crate::error::UpstrokeError;
use crate::events::RunOutcome;
use crate::topology::events::{
    CandidateRef, CommitSha, GenerationCloseReason, GenerationClosed, GenerationId,
    LeaseDisposition, LeaseGrant, Materialization, TaskDispatched, TopologyEventBody,
};
use crate::topology::paths::PathSet;
use crate::topology::registry::TaskKey;
use crate::workspace_manager::{Materialized, Quiescence, Slot, VerifyFailure, WorkspaceManager};

use super::seams::TopologyHooks;

pub trait EventEmitter {
    fn emit(
        &mut self,
        body: TopologyEventBody,
        hooks: &mut dyn TopologyHooks,
    ) -> Result<(), super::emit::EmitFailure>;

    fn standing(&self, invocation: &crate::runner::InvocationId) -> super::select::Standing;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchKind {
    Ordinary { paths: PathSet },
    Repair { root: TaskKey, source: CandidateRef },
}

impl DispatchKind {
    fn grant(&self) -> LeaseGrant {
        match self {
            Self::Ordinary { paths } => LeaseGrant::Predicted {
                paths: paths.clone(),
            },
            Self::Repair { root, .. } => LeaseGrant::InheritedLineage { root: *root },
        }
    }

    fn source_candidate(&self) -> Option<CandidateRef> {
        match self {
            Self::Ordinary { .. } => None,
            Self::Repair { source, .. } => Some(source.clone()),
        }
    }

    fn closing_disposition(&self) -> LeaseDisposition {
        self.lease().expected(false)
    }

    const fn lease(&self) -> crate::topology::leases::GenerationLease {
        match self {
            Self::Ordinary { .. } => crate::topology::leases::GenerationLease::Own,
            Self::Repair { root, .. } => {
                crate::topology::leases::GenerationLease::InheritedLineage { root: *root }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dispatched {
    pub key: TaskKey,
    pub generation: GenerationId,
    pub base: CommitSha,
    pub slot: Slot,
    pub worktree: PathBuf,
    pub kind: DispatchKind,
    pub materialized: Option<Materialization>,
}

impl Dispatched {
    #[must_use]
    pub fn quiescence(&self) -> Quiescence {
        Quiescence::AtBase(self.base.0.clone())
    }

    #[must_use]
    pub fn open_generation(&self) -> OpenGeneration {
        OpenGeneration {
            key: self.key,
            generation: self.generation,
            base: self.base.clone(),
            slot: self.slot.clone(),
            source: self.source().cloned(),
        }
    }

    #[must_use]
    pub fn closing_disposition(&self) -> LeaseDisposition {
        self.kind.closing_disposition()
    }

    #[must_use]
    pub const fn source(&self) -> Option<&CandidateRef> {
        match &self.kind {
            DispatchKind::Ordinary { .. } => None,
            DispatchKind::Repair { source, .. } => Some(source),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenGeneration {
    pub key: TaskKey,
    pub generation: GenerationId,
    pub base: CommitSha,
    pub slot: Slot,
    pub source: Option<CandidateRef>,
}

impl OpenGeneration {
    #[must_use]
    pub fn quiescence(&self) -> Quiescence {
        Quiescence::AtBase(self.base.0.clone())
    }
}

#[must_use]
pub fn task_slot(key: TaskKey, generation: GenerationId) -> Slot {
    Slot::Task {
        key: key.0.to_string(),
        generation: generation.0,
    }
}

pub trait DispatchJournal {
    fn emit(&mut self, body: TopologyEventBody) -> Result<(), super::emit::EmitFailure>;

    fn hooks(&mut self) -> &mut dyn TopologyHooks;
}

struct Emitting<'a> {
    hooks: &'a mut dyn TopologyHooks,
    emitter: &'a mut dyn EventEmitter,
}

impl DispatchJournal for Emitting<'_> {
    fn emit(&mut self, body: TopologyEventBody) -> Result<(), super::emit::EmitFailure> {
        self.emitter.emit(body, self.hooks)
    }

    fn hooks(&mut self) -> &mut dyn TopologyHooks {
        &mut *self.hooks
    }
}

pub fn dispatch(
    manager: &WorkspaceManager,
    hooks: &mut dyn TopologyHooks,
    emitter: &mut dyn EventEmitter,
    request: &DispatchRequest,
) -> Result<Dispatched, super::emit::EmitFailure> {
    dispatch_through(manager, &mut Emitting { hooks, emitter }, request)
}

pub fn dispatch_through(
    manager: &WorkspaceManager,
    journal: &mut dyn DispatchJournal,
    request: &DispatchRequest,
) -> Result<Dispatched, super::emit::EmitFailure> {
    manager.revalidate_pausing(journal.hooks().effects())?;
    if let DispatchKind::Repair { source, .. } = &request.kind {
        refuse_absent_source(manager, source)?;
    }

    let slot = task_slot(request.key, request.generation);
    let worktree = manager.slot_path(&slot);

    journal.emit(TopologyEventBody::TaskDispatched {
        data: TaskDispatched {
            key: request.key,
            generation: request.generation,
            base_sha: request.base.clone(),
            worktree_path: worktree.to_string_lossy().into_owned(),
            lease: request.kind.grant(),
            source_candidate: request.kind.source_candidate(),
        },
    })?;

    let mut dispatched = Dispatched {
        key: request.key,
        generation: request.generation,
        base: request.base.clone(),
        slot,
        worktree,
        kind: request.kind.clone(),
        materialized: None,
    };
    dispatched.worktree = create_worktree(manager, journal.hooks(), &dispatched.open_generation())?;

    if dispatched.source().is_some() {
        dispatched.materialized = Some(materialize_repair(
            manager,
            journal.hooks(),
            &dispatched.open_generation(),
        )?);
    }
    Ok(dispatched)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchRequest {
    pub key: TaskKey,
    pub generation: GenerationId,
    pub base: CommitSha,
    pub kind: DispatchKind,
}

fn create_worktree(
    manager: &WorkspaceManager,
    hooks: &mut dyn TopologyHooks,
    open: &OpenGeneration,
) -> Result<PathBuf, UpstrokeError> {
    manager.write_intent(hooks.effects(), &open.slot)?;
    manager.add_worktree(hooks.effects(), &open.slot, &open.base.0)
}

fn refuse_absent_source(
    manager: &WorkspaceManager,
    source: &CandidateRef,
) -> Result<(), UpstrokeError> {
    if !manager.object_exists(&source.commit_sha.0)? {
        return Err(UpstrokeError::Refused {
            message: format!(
                "refusing to dispatch a repair of candidate {} of task {}: its commit `{}` is not \
                 an object in this repository, and `T-DISPATCH` refuses a dispatch whose source \
                 candidate object is missing",
                source.generation.0, source.key, source.commit_sha.0
            ),
        });
    }
    match manager.direct_ref_target(&source.candidate_ref.0)? {
        Some(target) if target.eq_ignore_ascii_case(&source.commit_sha.0) => Ok(()),
        Some(target) => Err(UpstrokeError::Refused {
            message: format!(
                "refusing to dispatch a repair of candidate {} of task {}: `{}` names `{target}` \
                 and the recorded candidate is `{}`",
                source.generation.0, source.key, source.candidate_ref.0, source.commit_sha.0
            ),
        }),
        None => Err(UpstrokeError::Refused {
            message: format!(
                "refusing to dispatch a repair of candidate {} of task {}: its authoritative ref \
                 `{}` does not exist, and it is what keeps the candidate reachable",
                source.generation.0, source.key, source.candidate_ref.0
            ),
        }),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reuse {
    Verified,
    Recreated { failure: VerifyFailure },
}

impl Reuse {
    #[must_use]
    pub const fn reused(&self) -> bool {
        matches!(self, Self::Verified)
    }
}

pub fn verify_or_recreate(
    manager: &WorkspaceManager,
    hooks: &mut dyn TopologyHooks,
    open: &OpenGeneration,
    quiescence: &Quiescence,
) -> Result<Reuse, UpstrokeError> {
    match verify_reuse(manager, hooks, open, quiescence)? {
        Ok(()) => Ok(Reuse::Verified),
        Err(failure) => {
            if may_follow_a_deletion(&failure) {
                let kept = kept(manager, hooks.effects(), &open.slot)?;
                if !kept.is_empty() {
                    return Err(kept_slot_refusal(
                        &open.slot,
                        &format!(
                            "the reuse of generation {} of task {}, whose worktree failed \
                             verification ({failure})",
                            open.generation.0, open.key
                        ),
                        &kept,
                        "the open generation's checkout, from before its first attempt",
                        "the next resume recreates the worktree",
                    ));
                }
            }
            manager.remove_worktree(hooks.effects(), &open.slot)?;
            manager.remove_intent(hooks.effects(), &open.slot)?;
            create_worktree(manager, hooks, open)?;
            Ok(Reuse::Recreated { failure })
        }
    }
}

pub fn verify_reuse(
    manager: &WorkspaceManager,
    hooks: &mut dyn TopologyHooks,
    open: &OpenGeneration,
    quiescence: &Quiescence,
) -> Result<Result<(), VerifyFailure>, UpstrokeError> {
    manager.verify_worktree(hooks.effects(), &open.slot, quiescence)
}

pub fn materialize_repair(
    manager: &WorkspaceManager,
    hooks: &mut dyn TopologyHooks,
    open: &OpenGeneration,
) -> Result<Materialization, UpstrokeError> {
    let Some(source) = open.source.as_ref() else {
        return Err(UpstrokeError::Refused {
            message: format!(
                "task {} generation {} is an ordinary dispatch and has no recorded \
                 materialization to reproduce",
                open.key, open.generation.0
            ),
        });
    };
    let observed = manager.repair_materialize(hooks.effects(), &open.slot, &source.commit_sha.0)?;
    Ok(observed_kind(observed))
}

const fn observed_kind(observed: Materialized) -> Materialization {
    match observed {
        Materialized::Clean => Materialization::Clean,
        Materialized::Conflict => Materialization::Conflict,
        Materialized::Empty => Materialization::Empty,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resumed {
    pub reuse: Reuse,
    pub materialized: Option<Materialization>,
}

pub fn resume_open_no_attempt(
    manager: &WorkspaceManager,
    hooks: &mut dyn TopologyHooks,
    open: &OpenGeneration,
) -> Result<Resumed, UpstrokeError> {
    let reuse = verify_or_recreate(manager, hooks, open, &open.quiescence())?;
    let materialized = if open.source.is_some() {
        Some(materialize_repair(manager, hooks, open)?)
    } else {
        None
    };
    Ok(Resumed {
        reuse,
        materialized,
    })
}

pub fn close_at_run_end(
    manager: &WorkspaceManager,
    hooks: &mut dyn TopologyHooks,
    emitter: &mut dyn EventEmitter,
    dispatched: &Dispatched,
    outcome: RunOutcome,
) -> Result<(), super::emit::EmitFailure> {
    emitter.emit(
        TopologyEventBody::GenerationClosed {
            data: GenerationClosed {
                key: dispatched.key,
                generation: dispatched.generation,
                reason: GenerationCloseReason::RunEnding { outcome },
                lease: dispatched.kind.closing_disposition(),
            },
        },
        hooks,
    )?;
    Ok(scrub(manager, hooks, &dispatched.slot)?)
}

pub(super) fn scrub(
    manager: &WorkspaceManager,
    hooks: &mut dyn TopologyHooks,
    slot: &Slot,
) -> Result<(), UpstrokeError> {
    manager.remove_worktree(hooks.effects(), slot)?;
    manager.remove_intent(hooks.effects(), slot)
}

pub(super) fn refuse_kept_slot(
    manager: &WorkspaceManager,
    slot: &Slot,
    base: &CommitSha,
) -> Result<(), UpstrokeError> {
    let kept = kept(manager, &mut crate::workspace_manager::NoHooks, slot)?;
    if kept.is_empty() {
        return Ok(());
    }
    Err(kept_slot_refusal(
        slot,
        &format!("recovery's reclaim of a generation it has closed (recorded at base {base})"),
        &kept,
        "the attempt's unpinned output",
        "the next resume reclaims the slot",
    ))
}

pub(super) fn kept(
    manager: &WorkspaceManager,
    hooks: &mut dyn crate::workspace_manager::EffectHooks,
    slot: &Slot,
) -> Result<Vec<(PathBuf, String)>, UpstrokeError> {
    manager.kept_instances(hooks, slot)
}

pub(super) fn kept_slot_refusal(
    slot: &Slot,
    what: &str,
    kept: &[(PathBuf, String)],
    holds: &str,
    then: &str,
) -> UpstrokeError {
    let instances = kept
        .iter()
        .map(|(path, found)| format!("{} ({found})", path.display()))
        .collect::<Vec<_>>()
        .join("; ");
    UpstrokeError::RegistryRefused {
        message: format!(
            "{what} refused: the task worktree {slot:?} is kept for the operator, at {instances}. \
             It holds something while the check of its worktree registration fails, as named \
             beside each path. What the directory holds is {holds}. Take what it \
             holds, then remove the directory; {then}. Git cannot register the checkout again \
             when its whole entry is gone (`git worktree repair` exits 1 there); when only \
             `gitdir` is gone, `git worktree repair` writes it again, after which the next resume \
             treats the slot as a whole one, and recovery's reclaim removes it with what it holds."
        ),
    }
}

pub(super) const fn may_follow_a_deletion(failure: &VerifyFailure) -> bool {
    matches!(
        failure,
        VerifyFailure::NotRegistered | VerifyFailure::Missing | VerifyFailure::TreeMismatch { .. }
    )
}

pub(super) fn read_in_whole_checkout<T>(
    manager: &WorkspaceManager,
    checkout: &Path,
    read: &str,
    result: Result<T, UpstrokeError>,
) -> Result<T, UpstrokeError> {
    match result {
        Ok(answer) => match registration_whole(checkout, manager.common_git_dir()) {
            Whole::Yes => Ok(answer),
            Whole::No(found) => Err(read_refused(checkout, read, &found)),
        },
        Err(UpstrokeError::Git { message }) => {
            match registration_whole(checkout, manager.common_git_dir()) {
                Whole::Yes => Err(UpstrokeError::Git { message }),
                Whole::No(found) => Err(read_refused(checkout, &message, &found)),
            }
        }
        Err(other) => Err(other),
    }
}

pub(super) fn read_refused(checkout: &Path, what: &str, found: &str) -> UpstrokeError {
    UpstrokeError::RegistryRefused {
        message: format!(
            "{what}, in the checkout {}, is not used: the check of its worktree registration \
             failed ({found}). Git can answer without failing from a registration that is not \
             whole (a missing index reads as an empty one), so nothing read there is judged; the \
             command ends resumably",
            checkout.display()
        ),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Whole {
    Yes,
    No(String),
}

pub(super) fn registration_whole(checkout: &Path, common_git_dir: &Path) -> Whole {
    match registration_found(checkout, common_git_dir) {
        Ok(()) => Whole::Yes,
        Err(found) => Whole::No(found),
    }
}

fn registration_found(checkout: &Path, common_git_dir: &Path) -> Result<(), String> {
    let pointer = checkout.join(".git");
    let text = registration_file(&pointer, true)?;
    let Some(named) = text.strip_prefix(b"gitdir:") else {
        return Err(format!("{} does not begin `gitdir:`", pointer.display()));
    };
    let entry = registration_path(checkout, named.trim_ascii(), &pointer)?;
    let store = common_git_dir.join("worktrees");
    let in_store = match entry.parent() {
        Some(parent) => registration_canonical(parent)? == registration_canonical(&store)?,
        None => false,
    };
    if !in_store {
        return Err(format!(
            "{} names {}, which is not an entry of this repository's registration store {}",
            pointer.display(),
            entry.display(),
            store.display()
        ));
    }
    for (name, non_empty) in [
        ("gitdir", true),
        ("commondir", true),
        ("HEAD", true),
        ("index", false),
    ] {
        registration_metadata(&entry.join(name), non_empty)?;
    }
    let commondir = entry.join("commondir");
    let named = registration_file(&commondir, false)?;
    let common = registration_path(&entry, named.trim_ascii(), &commondir)?;
    if registration_canonical(&common)? != registration_canonical(common_git_dir)? {
        return Err(format!(
            "{} names {}, which is not this repository's common git dir {}",
            commondir.display(),
            common.display(),
            common_git_dir.display()
        ));
    }
    let gitdir = entry.join("gitdir");
    let named = registration_file(&gitdir, false)?;
    let named_checkout = registration_path(&entry, named.trim_ascii(), &gitdir)?;
    if registration_canonical(&named_checkout)? != registration_canonical(&pointer)? {
        return Err(format!(
            "{} names {}, which is not this checkout's {}",
            gitdir.display(),
            named_checkout.display(),
            pointer.display()
        ));
    }
    Ok(())
}

fn registration_metadata(path: &Path, non_empty: bool) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| format!("{} cannot be read: {error}", path.display()))?;
    let kind = metadata.file_type();
    if !kind.is_file() {
        return Err(format!(
            "{} is {}, not a regular file",
            path.display(),
            if kind.is_dir() {
                "a directory"
            } else if kind.is_symlink() {
                "a symbolic link"
            } else {
                "neither a regular file nor a directory"
            }
        ));
    }
    if non_empty && metadata.len() == 0 {
        return Err(format!("{} is empty", path.display()));
    }
    Ok(())
}

fn registration_file(path: &Path, checked: bool) -> Result<Vec<u8>, String> {
    if checked {
        registration_metadata(path, false)?;
    }
    std::fs::read(path).map_err(|error| format!("{} cannot be read: {error}", path.display()))
}

fn registration_path(base: &Path, named: &[u8], file: &Path) -> Result<PathBuf, String> {
    let path = registration_decode(named).map_err(|error| {
        format!(
            "{} names a path that is not UTF-8 from byte {}",
            file.display(),
            error.valid_up_to()
        )
    })?;
    Ok(if path.is_absolute() {
        path
    } else {
        base.join(path)
    })
}

#[cfg(unix)]
fn registration_decode(bytes: &[u8]) -> Result<PathBuf, std::str::Utf8Error> {
    use std::os::unix::ffi::OsStringExt as _;
    Ok(PathBuf::from(std::ffi::OsString::from_vec(bytes.to_vec())))
}

#[cfg(not(unix))]
fn registration_decode(bytes: &[u8]) -> Result<PathBuf, std::str::Utf8Error> {
    std::str::from_utf8(bytes)
        .map(|text| PathBuf::from(text.replace('/', std::path::MAIN_SEPARATOR_STR)))
}

fn registration_canonical(path: &Path) -> Result<PathBuf, String> {
    std::fs::canonicalize(path)
        .map_err(|error| format!("{} cannot be resolved: {error}", path.display()))
}

#[cfg(test)]
mod tests;
