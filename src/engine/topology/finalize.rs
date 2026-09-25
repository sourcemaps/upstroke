//! Extended notes: `docs/internals/engine/topology/finalize.md`

use std::path::Path;

use crate::error::UpstrokeError;
use crate::events::RunOutcome;
use crate::rundir;
use crate::topology::effects::RefSite;
use crate::topology::events::TopologyEvent;
use crate::topology::fold::TopologyFold;
use crate::workspace_manager::{Slot, WorkspaceManager, WriterProof};

use super::candidate::run_namespace;
use super::report::{TopologyReport, outcome_label};
use super::seams::TopologyHooks;

pub struct Finalize<'a> {
    pub manager: &'a WorkspaceManager,
    pub public: &'a Path,
    pub private: &'a Path,
    pub run_id: &'a str,
    pub fold: &'a TopologyFold,
    pub events: &'a [TopologyEvent],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CleanupStep {
    TaskWorktrees,
    Snapshots,
    Staging,
    Pins,
    CandidatesRefs,
    ExecutionRoot,
}

impl CleanupStep {
    pub const ORDER: [Self; 6] = [
        Self::TaskWorktrees,
        Self::Snapshots,
        Self::Staging,
        Self::Pins,
        Self::CandidatesRefs,
        Self::ExecutionRoot,
    ];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::TaskWorktrees => "task worktrees and intents",
            Self::Snapshots => "snapshots and intents",
            Self::Staging => "staging worktrees and intents",
            Self::Pins => "prepared and candidate-prepared pins",
            Self::CandidatesRefs => "candidates refs",
            Self::ExecutionRoot => "execution root",
        }
    }

    #[must_use]
    pub const fn applies_to(self, outcome: &RunOutcome) -> bool {
        match self {
            Self::Snapshots | Self::Staging | Self::Pins | Self::ExecutionRoot => true,
            Self::TaskWorktrees => matches!(outcome, RunOutcome::Complete | RunOutcome::Halted),
            Self::CandidatesRefs => matches!(outcome, RunOutcome::Complete),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finalized {
    pub outcome: RunOutcome,
    pub report_written: bool,
    pub removed: Vec<(CleanupStep, usize)>,
    pub execution_root_removed: bool,
    pub retained_candidates: usize,
    pub passed_over_registrations: Vec<std::path::PathBuf>,
    pub passed_over_staging: Vec<std::path::PathBuf>,
}

pub fn finalize(
    inputs: &Finalize<'_>,
    hooks: &mut dyn TopologyHooks,
) -> Result<Finalized, UpstrokeError> {
    let outcome = inputs
        .fold
        .finished()
        .cloned()
        .ok_or_else(|| UpstrokeError::Refused {
            message: format!(
                "run `{}` has no durable `run_finished`, and finalization acts only after a \
                 valid one; nothing was written and nothing was deleted",
                inputs.run_id
            ),
        })?;

    let report = TopologyReport::derive(inputs.run_id, inputs.fold, inputs.events)?;
    let report_path = inputs.public.join("report.json");
    let fresh = match std::fs::read(&report_path) {
        Ok(existing) => report.is_fresh_against(&existing),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(source) => {
            return Err(UpstrokeError::Io {
                path: report_path,
                source,
            });
        }
    };
    let passed_over_staging = if fresh {
        rundir::sync_report_dir(inputs.public, inputs.private, hooks.rundir())?
    } else {
        rundir::write_report(inputs.public, inputs.private, &report, hooks.rundir())?
    };

    let namespace = run_namespace(inputs.run_id);
    let mut removed = Vec::with_capacity(CleanupStep::ORDER.len());
    let mut execution_root_removed = false;
    let mut passed_over_registrations = Vec::new();
    for step in CleanupStep::ORDER {
        if !step.applies_to(&outcome) {
            continue;
        }
        let count = match step {
            CleanupStep::TaskWorktrees => scrub_slots(
                inputs.manager,
                hooks,
                &mut passed_over_registrations,
                |slot| matches!(slot, Slot::Task { .. }),
            )?,
            CleanupStep::Snapshots => scrub_slots(
                inputs.manager,
                hooks,
                &mut passed_over_registrations,
                |slot| matches!(slot, Slot::Snapshot { .. }),
            )?,
            CleanupStep::Staging => scrub_slots(
                inputs.manager,
                hooks,
                &mut passed_over_registrations,
                |slot| matches!(slot, Slot::Staging { .. }),
            )?,
            CleanupStep::Pins => {
                let prepared = delete_refs_under(
                    inputs.manager,
                    hooks,
                    &format!("{namespace}prepared/"),
                    RefSite::DeletePreparedPin,
                )?;
                let candidate_pins = delete_refs_under(
                    inputs.manager,
                    hooks,
                    &format!("{namespace}candidate-prepared/"),
                    RefSite::DeleteCandidatePin,
                )?;
                prepared.saturating_add(candidate_pins)
            }
            CleanupStep::CandidatesRefs => delete_refs_under(
                inputs.manager,
                hooks,
                &format!("{namespace}candidates/"),
                RefSite::DeleteCandidatesRef,
            )?,
            CleanupStep::ExecutionRoot => {
                inputs.manager.remove_staging_leftovers(hooks.effects())?;
                execution_root_removed = inputs.manager.remove_execution_root(hooks.effects())?;
                usize::from(execution_root_removed)
            }
        };
        removed.push((step, count));
    }

    passed_over_registrations.sort();
    passed_over_registrations.dedup();
    Ok(Finalized {
        outcome,
        report_written: !fresh,
        removed,
        execution_root_removed,
        retained_candidates: report.retained_candidates.len(),
        passed_over_registrations,
        passed_over_staging,
    })
}

pub fn refuse_continuation(run_id: &str, finalized: &Finalized) -> UpstrokeError {
    let passed_over = if finalized.passed_over_registrations.is_empty() {
        String::new()
    } else {
        format!(
            "; {} worktree registration(s) Git cannot list, prune or repair, left by an \
             interrupted `git worktree add`, passed over and left as found: {}",
            finalized.passed_over_registrations.len(),
            finalized
                .passed_over_registrations
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    let passed_over_staging = if finalized.passed_over_staging.is_empty() {
        String::new()
    } else {
        format!(
            "; {} entr{} of a report staging directory's shape under the run directory that no \
             record of this run's names, passed over and left as found: {}",
            finalized.passed_over_staging.len(),
            if finalized.passed_over_staging.len() == 1 {
                "y"
            } else {
                "ies"
            },
            finalized
                .passed_over_staging
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    UpstrokeError::Refused {
        message: format!(
            "run `{run_id}` already finished as `{}`, and a finished run does not continue. \
             Recovery step (b) finalized it first: the report was {}, {}{passed_over}\
             {passed_over_staging} and continuation is refused",
            outcome_label(&finalized.outcome),
            if finalized.report_written {
                "regenerated"
            } else {
                "already current"
            },
            finalized
                .removed
                .iter()
                .map(|(step, count)| format!("{count} {}", step.label()))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn scrub_slots(
    manager: &WorkspaceManager,
    hooks: &mut dyn TopologyHooks,
    passed_over: &mut Vec<std::path::PathBuf>,
    keep: impl Fn(&Slot) -> bool,
) -> Result<usize, UpstrokeError> {
    let mut count = 0;
    for slot in manager.intents()? {
        if !keep(&slot) {
            continue;
        }
        passed_over.extend(manager.remove_worktree_proving(
            hooks.effects(),
            &slot,
            WriterProof::NoWriterAlive,
        )?);
        manager.remove_intent(hooks.effects(), &slot)?;
        count += 1;
    }
    Ok(count)
}

fn delete_refs_under(
    manager: &WorkspaceManager,
    hooks: &mut dyn TopologyHooks,
    prefix: &str,
    site: RefSite,
) -> Result<usize, UpstrokeError> {
    let mut count = 0;
    for (refname, oid) in manager.refs_under(prefix)? {
        manager.delete_ref_expected_old(hooks.effects(), site, &refname, &oid)?;
        count += 1;
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::ffi::OsStr;
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::agent::proc::SpawnHooks;
    use crate::engine::topology::seams::NoTopologyHooks;
    use crate::events::log::EventHooks;
    use crate::rundir::RunDirHooks;
    use crate::runner::container::ContainerHooks;
    use crate::topology::effects::{
        EffectSiteId, HookPhase, Injection, SnapshotSite, WorktreeSite,
    };
    use crate::workspace_manager::fixture::{Fixture, git_os, registration_of, tear_registration};
    use crate::workspace_manager::{EffectHooks, NoHooks, ObjectId, SnapshotInput, SnapshotName};

    fn tasks(slot: &Slot) -> bool {
        matches!(slot, Slot::Task { .. })
    }

    fn snapshots(slot: &Slot) -> bool {
        matches!(slot, Slot::Snapshot { .. })
    }

    fn scrub(
        fixture: &Fixture,
        hooks: &mut dyn TopologyHooks,
        keep: impl Fn(&Slot) -> bool,
    ) -> Result<usize, UpstrokeError> {
        let mut passed_over = Vec::new();
        let count = scrub_slots(&fixture.manager, hooks, &mut passed_over, keep)?;
        assert!(
            passed_over.is_empty(),
            "no registration here names nothing: {passed_over:?}"
        );
        Ok(count)
    }

    fn add_snapshot(fixture: &Fixture, sequence: u64) -> (Slot, PathBuf) {
        let name = SnapshotName::integration(sequence);
        let snapshot = fixture
            .manager
            .add_snapshot(
                &mut NoHooks,
                &name,
                &SnapshotInput::Commit(ObjectId::new(fixture.head.clone()).expect("an object id")),
            )
            .expect("add a snapshot");
        (Slot::Snapshot { name }, snapshot.path().to_path_buf())
    }

    fn tree_bytes(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
        let mut out = BTreeMap::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).expect("list a directory the test made") {
                let path = entry.expect("a directory entry").path();
                let relative = path
                    .strip_prefix(root)
                    .expect("an entry is under its root")
                    .to_path_buf();
                if std::fs::symlink_metadata(&path)
                    .expect("entry metadata")
                    .is_dir()
                {
                    out.insert(relative, Vec::new());
                    stack.push(path);
                } else {
                    out.insert(relative, std::fs::read(&path).expect("read a file"));
                }
            }
        }
        out
    }

    fn assert_enumeration_dies_on(fixture: &Fixture, admin: &Path) {
        let name = admin
            .file_name()
            .expect("an administrative directory has a name")
            .to_string_lossy()
            .into_owned();
        let message = fixture
            .manager
            .worktree_records()
            .expect_err("Git's enumeration dies on a zero-length commondir")
            .to_string();
        assert!(
            message.contains(&name) && message.contains("commondir"),
            "the enumeration dies on {name}'s commondir: {message}"
        );
    }

    struct CrossKind {
        alpha: Slot,
        alpha_path: PathBuf,
        alpha_admin: PathBuf,
        snapshot: Slot,
        snapshot_path: PathBuf,
        snapshot_admin: PathBuf,
    }

    impl CrossKind {
        fn build(fixture: &Fixture) -> Self {
            let alpha = fixture.add_task(&mut NoHooks, "alpha", 1);
            let alpha_path = fixture.manager.slot_path(&alpha);
            let alpha_admin = registration_of(&fixture.manager, &alpha_path);
            let (snapshot, snapshot_path) = add_snapshot(fixture, 1);
            let snapshot_admin = tear_registration(&fixture.manager, &snapshot_path);
            assert_eq!(
                fixture.manager.intents().expect("intents"),
                vec![alpha.clone(), snapshot.clone()],
                "one task intent and one snapshot intent"
            );
            assert_enumeration_dies_on(fixture, &snapshot_admin);
            Self {
                alpha,
                alpha_path,
                alpha_admin,
                snapshot,
                snapshot_path,
                snapshot_admin,
            }
        }

        fn assert_converged(&self, fixture: &Fixture) {
            assert!(
                fixture.manager.intents().expect("intents").is_empty(),
                "both intents are reclaimed"
            );
            assert!(
                !self.alpha_path.exists() && !self.snapshot_path.exists(),
                "both checkouts are gone"
            );
            assert!(
                !self.alpha_admin.exists() && !self.snapshot_admin.exists(),
                "and both registrations"
            );
            let records = fixture
                .manager
                .worktree_records()
                .expect("Git enumerates again");
            assert!(
                records.iter().all(|record| {
                    !record.path().ends_with("kalpha-g1")
                        && !record.path().ends_with("s1-integration")
                }),
                "neither slot is registered: {records:?}"
            );
        }
    }

    #[test]
    fn scrub_slots_converges_past_a_torn_registration_of_the_kind_it_reclaims() {
        let fixture = Fixture::created("finalize-torn-same-kind");
        let alpha = fixture.add_task(&mut NoHooks, "alpha", 1);
        let bravo = fixture.add_task(&mut NoHooks, "bravo", 1);
        let alpha_path = fixture.manager.slot_path(&alpha);
        let bravo_path = fixture.manager.slot_path(&bravo);
        let bravo_admin = tear_registration(&fixture.manager, &bravo_path);
        assert_eq!(
            fixture.manager.intents().expect("intents"),
            vec![alpha.clone(), bravo.clone()],
            "the torn slot sorts second, behind an intent the scrub reaches first"
        );
        assert_enumeration_dies_on(&fixture, &bravo_admin);

        let count = scrub(&fixture, &mut NoTopologyHooks::new(), tasks)
            .expect("one torn registration does not wedge the scrub of the others");
        assert_eq!(count, 2);
        assert!(fixture.manager.intents().expect("intents").is_empty());
        assert!(!alpha_path.exists() && !bravo_path.exists());
        assert!(!bravo_admin.exists());
        let records = fixture
            .manager
            .worktree_records()
            .expect("Git enumerates again");
        assert!(
            records.iter().all(|record| {
                !record.path().ends_with("kalpha-g1") && !record.path().ends_with("kbravo-g1")
            }),
            "{records:?}"
        );
    }

    #[test]
    fn scrub_slots_repairs_a_torn_registration_of_a_kind_a_later_step_reclaims() {
        let fixture = Fixture::created("finalize-torn-other-kind");
        let slots = CrossKind::build(&fixture);

        let count = scrub(&fixture, &mut NoTopologyHooks::new(), tasks)
            .expect("a torn snapshot registration does not wedge the task scrub");
        assert_eq!(count, 1);
        assert!(!slots.alpha_path.exists());
        assert!(!fixture.manager.intent_path(&slots.alpha).exists());
        assert!(!slots.snapshot_admin.exists() && !slots.snapshot_path.exists());
        assert!(fixture.manager.intent_path(&slots.snapshot).exists());
        fixture
            .manager
            .worktree_records()
            .expect("Git enumerates again");

        let count = scrub(&fixture, &mut NoTopologyHooks::new(), snapshots)
            .expect("the snapshot scrub converges with nothing left to remove");
        assert_eq!(count, 1);
        slots.assert_converged(&fixture);
    }

    #[test]
    fn scrub_slots_converges_when_git_has_pruned_the_emptied_registration_store() {
        let fixture = Fixture::created("finalize-torn-store-pruned");
        let alpha = fixture.add_task(&mut NoHooks, "alpha", 1);
        let beta = fixture.add_task(&mut NoHooks, "beta", 1);
        let (snapshot, snapshot_path) = add_snapshot(&fixture, 1);
        let snapshot_admin = tear_registration(&fixture.manager, &snapshot_path);
        assert_eq!(
            fixture.manager.intents().expect("intents"),
            vec![alpha.clone(), beta.clone(), snapshot.clone()]
        );
        assert_enumeration_dies_on(&fixture, &snapshot_admin);
        let store = fixture.manager.common_git_dir().join("worktrees");

        let count = scrub(&fixture, &mut NoTopologyHooks::new(), tasks)
            .expect("a torn snapshot registration does not wedge the task scrub");
        assert_eq!(count, 2);
        assert!(
            !store.exists(),
            "beta's removal left the store empty, and Git's prune deleted it"
        );
        assert!(!snapshot_path.exists() && !snapshot_admin.exists());
        assert!(fixture.manager.intent_path(&snapshot).exists());

        let count = scrub(&fixture, &mut NoTopologyHooks::new(), snapshots)
            .expect("the snapshot scrub converges with no registration store at all");
        assert_eq!(count, 1);
        assert!(fixture.manager.intents().expect("intents").is_empty());
        for slot in [&alpha, &beta] {
            assert!(!fixture.manager.slot_path(slot).exists());
        }
    }

    struct StopEffects {
        stop: usize,
        seen: Vec<(EffectSiteId, HookPhase)>,
    }

    impl EffectHooks for StopEffects {
        fn phase(&mut self, site: EffectSiteId, phase: HookPhase) -> Injection {
            self.seen.push((site, phase));
            if self.seen.len() == self.stop {
                Injection::Error
            } else {
                Injection::Proceed
            }
        }

        fn refusal_cause(&self) -> Option<String> {
            None
        }
    }

    struct StopHooks {
        effects: StopEffects,
        rest: NoTopologyHooks,
    }

    impl TopologyHooks for StopHooks {
        fn effects(&mut self) -> &mut dyn EffectHooks {
            &mut self.effects
        }

        fn rundir(&mut self) -> &mut dyn RunDirHooks {
            self.rest.rundir()
        }

        fn events(&mut self) -> &mut dyn EventHooks {
            self.rest.events()
        }

        fn container(&mut self) -> &mut dyn ContainerHooks {
            self.rest.container()
        }

        fn spawn(&mut self) -> &mut dyn SpawnHooks {
            self.rest.spawn()
        }
    }

    #[test]
    fn a_cross_kind_scrub_stopped_at_any_phase_converges_on_the_next() {
        let mut stop = 0;
        let completed = loop {
            stop += 1;
            assert!(
                stop <= 32,
                "two scrubs of one slot each consult a bounded number of phases"
            );
            let fixture = Fixture::created(&format!("finalize-torn-stop-{stop}"));
            let slots = CrossKind::build(&fixture);
            let mut hooks = StopHooks {
                effects: StopEffects {
                    stop,
                    seen: Vec::new(),
                },
                rest: NoTopologyHooks::new(),
            };
            let stopped = scrub(&fixture, &mut hooks, tasks)
                .and_then(|_| scrub(&fixture, &mut hooks, snapshots));
            for (slot, path, admin) in [
                (&slots.alpha, &slots.alpha_path, &slots.alpha_admin),
                (&slots.snapshot, &slots.snapshot_path, &slots.snapshot_admin),
            ] {
                if path.exists() || admin.exists() {
                    assert!(
                        fixture.manager.intent_path(slot).exists(),
                        "stopped at phase {stop}: {} outlives its intent",
                        path.display()
                    );
                }
            }
            scrub(&fixture, &mut NoTopologyHooks::new(), tasks)
                .and_then(|_| scrub(&fixture, &mut NoTopologyHooks::new(), snapshots))
                .unwrap_or_else(|error| {
                    panic!("stopped at phase {stop}, the next scrubs converge: {error}")
                });
            slots.assert_converged(&fixture);
            if stopped.is_ok() {
                break hooks.effects.seen;
            }
        };
        let remove = EffectSiteId::Worktree(WorktreeSite::Remove);
        let remove_intent = EffectSiteId::Worktree(WorktreeSite::RemoveIntent);
        let snapshot_remove = EffectSiteId::Snapshot(SnapshotSite::Remove);
        let snapshot_remove_intent = EffectSiteId::Snapshot(SnapshotSite::RemoveIntent);
        assert_eq!(
            completed,
            vec![
                (remove, HookPhase::Before),
                (remove, HookPhase::After),
                (snapshot_remove, HookPhase::Before),
                (snapshot_remove, HookPhase::After),
                (remove_intent, HookPhase::Before),
                (remove_intent, HookPhase::After),
                (snapshot_remove, HookPhase::Before),
                (snapshot_remove, HookPhase::After),
                (snapshot_remove_intent, HookPhase::Before),
                (snapshot_remove_intent, HookPhase::After),
            ],
            "the torn snapshot registration is repaired under its own removal site, before the task \
             intent's removal enumerates"
        );
    }

    #[test]
    fn scrub_slots_still_refuses_a_torn_registration_no_intent_names() {
        let fixture = Fixture::created("finalize-torn-unintended");
        let alpha = fixture.add_task(&mut NoHooks, "alpha", 1);
        let unintended = fixture.manager.slot_path(&fixture.task("charlie", 1));
        git_os(
            &fixture.base,
            &[
                OsStr::new("worktree"),
                OsStr::new("add"),
                OsStr::new("-q"),
                OsStr::new("--detach"),
                unintended.as_os_str(),
                OsStr::new(&fixture.head),
            ],
        );
        let admin = tear_registration(&fixture.manager, &unintended);
        assert_enumeration_dies_on(&fixture, &admin);
        let admin_before = tree_bytes(&admin);
        let checkout_before = tree_bytes(&unintended);

        let message = scrub(&fixture, &mut NoTopologyHooks::new(), tasks)
            .expect_err("a torn registration no intent names refuses the scrub")
            .to_string();
        assert!(message.contains("commondir"), "{message}");
        assert_eq!(tree_bytes(&admin), admin_before);
        assert_eq!(tree_bytes(&unintended), checkout_before);
        assert!(fixture.manager.intent_path(&alpha).exists());
        assert_enumeration_dies_on(&fixture, &admin);
    }
}
