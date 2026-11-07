//! Extended notes: `docs/internals/engine/topology/prelock.md`

use std::path::{Path, PathBuf};

use crate::config::RunnerSelection;
use crate::error::UpstrokeError;
use crate::runner::container::resolve::{MountContext, resolve_container};
use crate::runner::container::runtime::ContainerRuntime;
use crate::runner::policy::{resolve_host, runner_policy_sha256};
use crate::topology::events::{IncarnationId, RunnerKind, RunnerPolicy};

use super::seams::IdSource;

pub use checked::PreLockChecked;

mod checked {
    use super::{IncarnationId, Path, PathBuf, RunnerPolicy};

    #[derive(Debug)]
    pub struct PreLockChecked {
        run_id: String,
        incarnation: IncarnationId,
        pid: u32,
        policy: RunnerPolicy,
        policy_sha256: String,
        private_root: PathBuf,
    }

    impl PreLockChecked {
        pub(super) fn new(
            run_id: String,
            incarnation: IncarnationId,
            pid: u32,
            policy: RunnerPolicy,
            policy_sha256: String,
            private_root: PathBuf,
        ) -> Self {
            Self {
                run_id,
                incarnation,
                pid,
                policy,
                policy_sha256,
                private_root,
            }
        }

        #[must_use]
        pub fn run_id(&self) -> &str {
            &self.run_id
        }

        #[must_use]
        pub fn incarnation(&self) -> &IncarnationId {
            &self.incarnation
        }

        #[must_use]
        pub const fn pid(&self) -> u32 {
            self.pid
        }

        #[must_use]
        pub const fn runner_policy(&self) -> &RunnerPolicy {
            &self.policy
        }

        #[must_use]
        pub fn runner_policy_sha256(&self) -> &str {
            &self.policy_sha256
        }

        #[must_use]
        pub fn private_root(&self) -> &Path {
            &self.private_root
        }

        #[must_use]
        pub fn private_dir(&self) -> PathBuf {
            self.private_root.join("runs").join(&self.run_id)
        }
    }
}

pub struct PreLock<'a> {
    pub selection: &'a RunnerSelection,
    pub runtime: Option<&'a dyn ContainerRuntime>,
    pub private_root: &'a Path,
    pub ids: &'a dyn IdSource,
    pub mounts: &'a MountContext,
}

pub fn check(inputs: &PreLock<'_>) -> Result<PreLockChecked, UpstrokeError> {
    crate::workspace_manager::refuse_relative_repository_wide_names()?;
    let private_root = authorized_private_root(inputs.private_root)?;
    let resolved = resolve_policy(inputs.selection, inputs.runtime, inputs.mounts)?;
    Ok(resolved.checked(private_root, inputs.ids))
}

pub fn resolve_runner(
    selection: &RunnerSelection,
    runtime: Option<&dyn ContainerRuntime>,
    mounts: &MountContext,
) -> Result<RunnerResolved, UpstrokeError> {
    crate::workspace_manager::refuse_relative_repository_wide_names()?;
    resolve_policy(selection, runtime, mounts)
}

#[derive(Debug)]
pub struct RunnerResolved {
    policy: RunnerPolicy,
    policy_sha256: String,
}

impl RunnerResolved {
    #[must_use]
    pub const fn runner_policy(&self) -> &RunnerPolicy {
        &self.policy
    }

    pub fn authorize(
        self,
        private_root: &Path,
        ids: &dyn IdSource,
    ) -> Result<PreLockChecked, UpstrokeError> {
        let private_root = authorized_private_root(private_root)?;
        Ok(self.checked(private_root, ids))
    }

    fn checked(self, private_root: PathBuf, ids: &dyn IdSource) -> PreLockChecked {
        let run_id = ids.run_id();
        let incarnation = ids.incarnation();
        let pid = ids.pid();

        PreLockChecked::new(
            run_id,
            incarnation,
            pid,
            self.policy,
            self.policy_sha256,
            private_root,
        )
    }
}

fn resolve_policy(
    selection: &RunnerSelection,
    runtime: Option<&dyn ContainerRuntime>,
    mounts: &MountContext,
) -> Result<RunnerResolved, UpstrokeError> {
    let policy = match selection.kind {
        RunnerKind::Host => resolve_host()?,
        RunnerKind::Container => {
            let runtime = runtime.ok_or_else(|| UpstrokeError::Refused {
                message: format!(
                    "`[runner] kind = \"container\"` selects the container boundary, and this \
                     command was given no container runtime to inspect. INV-23 resolves the \
                     policy by read-only inspection before any lock — the runtime must be \
                     reachable, the image reference `{}` must already be present in it, and each \
                     credential volume must exist — so a missing runtime refuses here rather \
                     than being assumed available later",
                    selection.image.as_deref().unwrap_or("<unset>")
                ),
            })?;
            resolve_container(runtime, selection, mounts)?
        }
    };
    let policy_sha256 = runner_policy_sha256(&policy);
    Ok(RunnerResolved {
        policy,
        policy_sha256,
    })
}

pub fn refuse_unspellable_default_root(root: &Path) -> Result<(), UpstrokeError> {
    let absolute = std::path::absolute(root).map_err(|source| UpstrokeError::Io {
        path: root.to_path_buf(),
        source,
    })?;
    let mut existing = None;
    for ancestor in absolute.ancestors() {
        match std::fs::symlink_metadata(ancestor) {
            Ok(_) => {
                existing = Some(ancestor);
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(UpstrokeError::Io {
                    path: ancestor.to_path_buf(),
                    source,
                });
            }
        }
    }
    let resolved = match existing {
        Some(existing) => {
            let canonical =
                std::fs::canonicalize(existing).map_err(|source| UpstrokeError::Io {
                    path: existing.to_path_buf(),
                    source,
                })?;
            let missing: PathBuf = absolute
                .components()
                .skip(existing.components().count())
                .collect();
            canonical.join(missing)
        }
        None => absolute,
    };
    if resolved.to_str().is_some() {
        return Ok(());
    }
    Err(UpstrokeError::Refused {
        message: format!(
            "the default private root {} is not valid UTF-8 once resolved ({}); a schema-4 run \
             records its private half and execution root as text, so it would record another \
             path than this one. Nothing was locked or created: run upstroke with a home \
             directory whose path is valid UTF-8",
            root.display(),
            resolved.display()
        ),
    })
}

fn authorized_private_root(configured: &Path) -> Result<PathBuf, UpstrokeError> {
    let metadata = match std::fs::symlink_metadata(configured) {
        Ok(metadata) => metadata,
        Err(source) => {
            return Err(UpstrokeError::Io {
                path: configured.to_path_buf(),
                source,
            });
        }
    };
    let canonical = std::fs::canonicalize(configured).map_err(|source| UpstrokeError::Io {
        path: configured.to_path_buf(),
        source,
    })?;
    if !metadata.is_dir() && !canonical.is_dir() {
        return Err(UpstrokeError::Refused {
            message: format!(
                "the authorized private root {} is not a directory; every run's private half \
                 lives at <root>/runs/<run_id> and the root is checked read-only before any lock",
                configured.display()
            ),
        });
    }
    // A schema-4 run records its private half and its execution root, both of which hang from
    // this root by ASCII names, as text in its owner record and `run_started`. A root no string
    // spells exactly would be recorded as some other path and leave a run no resume can find,
    // so it is refused here, before any lock and before anything of the run exists
    // (CODING_STANDARDS §8: a lossy display string is never identity).
    if canonical.to_str().is_none() {
        return Err(UpstrokeError::Refused {
            message: format!(
                "the authorized private root {} is not valid UTF-8 once resolved; a schema-4 run \
                 records its private half and execution root as text, so it would record another \
                 path than this one. Nothing was locked or created: choose a private root whose \
                 path is valid UTF-8",
                canonical.display()
            ),
        });
    }
    Ok(canonical)
}

#[cfg(test)]
mod tests;
