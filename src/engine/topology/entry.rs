//! Extended notes: `docs/internals/engine/topology/entry.md`

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::agent::Caps;
use crate::capacity::Pool;
use crate::config::{self, Budgets, EngineLimits, OnTaskFailure};
use crate::engine::assembly::{FrozenPlans, SharedPlans};
use crate::engine::attempt::{LegacyReviewInputPolicy, LegacyReviewPasses};
use crate::engine::options::{Harness, ResumeOptions, RunOptions};
use crate::engine::preflight::{
    chain_summaries, gates_differ, normalized_plan_bytes, repo_relative,
};
use crate::engine::report::{RunOutcome as ReportedOutcome, RunReport};
use crate::error::UpstrokeError;
use crate::events::{self, GateSummary, RunOutcome};
use crate::gates::{self, ShellGate, ShellKind};
use crate::interaction::{self, AnswerSource, InteractionMode, Notifier, RealSleeper, Sleeper};
use crate::ir::{Answer, Plan, Question, QuestionId};
use crate::review::{self, ReviewPlan};
use crate::rundir::{self, RepoKey, RunPaths, WorktreeLock};
use crate::runner::Runner;
use crate::runner::container::env::ContainerEnvironment;
use crate::runner::container::exec::{ContainerRunner, RunIdentity};
use crate::runner::container::resolve::{
    InspectionRefusal, MountContext, RepositoryGit, mount_label, refuse_excluded,
    refuse_recorded_targets,
};
use crate::runner::container::runtime::{
    ContainerRuntime, ContainerTrace, LockProbe, RuntimeError,
};
use crate::runner::container::view::RoleGitView;
use crate::runner::host::{HostEnvironment, HostRunner, ManagedRepository, ObjectGraph};
use crate::topology::events::{
    CommitSha, GitRef, IncarnationId, RunStarted4, RunnerKind, RunnerPolicy, TopologyLimits,
};
use crate::topology::fold::FrozenInputs;
use crate::topology::paths::{PathGrammar, PathPolicy, PathPolicyVersion};
use crate::topology::registry::TaskRegistry;
use crate::topology::schema::{
    MAX_READABLE_SCHEMA, ReaderSelection, TOPOLOGY_SCHEMA, probe_header, select_for_schema,
};
use crate::validate::{self, Analysis, CapturedInputs, ValidateOptions};
use crate::workspace::Workspace;
use crate::workspace_manager::{EffectHooks, WorkspaceManager};

use super::coordinator::PipelineSeams;
use super::create::{ProbePair, Request, RunnerProbes, create_run};
use super::identity::{InvocationLedger, SlotLimits};
use super::preflight::RunPreflight;
use super::prelock::{self, PreLock, PreLockChecked, RunnerResolved};
use super::recover::{
    RecordsVerified, ResumeSeams, RootDerived, recover_verified, verify_for_recovery,
};
use super::run::{Progress, RunSeams, TopologyRun};
use super::seams::{IdSource, NoTopologyHooks, RealIds, SystemClock, TopologyHooks};
use super::select::Ceiling;
use super::startup::{CensusInputs, WorktreeLocked, startup_census};

pub struct FreshPrepared {
    validate_opts: ValidateOptions,
    captured: CapturedInputs,
    analysis: Analysis,
    review_plan: ReviewPlan,
    agents: Vec<String>,
    gate_cmds: Vec<GateSummary>,
    mode: InteractionMode,
    notifiers: Vec<&'static dyn Notifier>,
    ceiling: Ceiling,
    slots: SlotLimits,
    authorization: Authorization,
    image_environment: Option<Vec<(String, String)>>,
    warnings: Vec<String>,
}

pub struct FreshLeased {
    prepared: FreshPrepared,
    workspace: Workspace,
    worktree_git_dir: PathBuf,
    worktree: WorktreeLock,
    hooks: NoTopologyHooks,
}

enum Authorization {
    Checked(PreLockChecked),
    Pending {
        resolved: RunnerResolved,
        ids: DrawnIds,
        root: PathBuf,
    },
}

struct DrawnIds {
    run_id: String,
    incarnation: IncarnationId,
    pid: u32,
}

impl DrawnIds {
    fn draw(ids: &dyn IdSource) -> Self {
        Self {
            run_id: ids.run_id(),
            incarnation: ids.incarnation(),
            pid: ids.pid(),
        }
    }
}

impl IdSource for DrawnIds {
    fn run_id(&self) -> String {
        self.run_id.clone()
    }

    fn incarnation(&self) -> IncarnationId {
        self.incarnation.clone()
    }

    fn pid(&self) -> u32 {
        self.pid
    }

    fn question_id(&self) -> QuestionId {
        RealIds.question_id()
    }
}

impl Authorization {
    fn runner_policy(&self) -> &RunnerPolicy {
        match self {
            Self::Checked(checked) => checked.runner_policy(),
            Self::Pending { resolved, .. } => resolved.runner_policy(),
        }
    }
}

pub fn prepare_fresh(
    opts: &RunOptions,
    harness: &Harness<'_>,
) -> Result<FreshPrepared, UpstrokeError> {
    crate::workspace_manager::refuse_relative_repository_wide_names()?;
    let validate_opts = ValidateOptions {
        plan_path: opts.plan_path.clone(),
        config_path: opts.config_path.clone(),
        config_root: opts.repo_root.clone(),
        pools_path: opts.pools_path.clone(),
        engine_limits: EngineLimits::Topology,
    };
    let captured = CapturedInputs::capture(&validate_opts);
    let mut analysis = validate::analyze_captured(&captured, &validate_opts)?;
    let mut warnings = std::mem::take(&mut analysis.warnings);

    let runtime = container_runtime::for_inspection();
    let authorization = match &opts.private_root {
        Some(root) => {
            let mounts = mount_context(&analysis.config.runner, &opts.repo_root, root)?;
            Authorization::Checked(prelock::check(&PreLock {
                selection: &analysis.config.runner,
                runtime: Some(&*runtime),
                private_root: root,
                ids: &RealIds,
                mounts: &mounts,
            })?)
        }
        None => {
            let root = rundir::default_private_root();
            let mounts = mount_context(&analysis.config.runner, &opts.repo_root, &root)?;
            prelock::refuse_unspellable_default_root(&root)?;
            Authorization::Pending {
                resolved: prelock::resolve_runner(
                    &analysis.config.runner,
                    Some(&*runtime),
                    &mounts,
                )?,
                ids: DrawnIds::draw(&RealIds),
                root,
            }
        }
    };
    let kind = authorization.runner_policy().kind;
    let image_environment = match kind {
        RunnerKind::Host => None,
        RunnerKind::Container => Some(recorded_image_environment(
            &*runtime,
            authorization.runner_policy(),
        )?),
    };

    let review_plan = review::plan_for(
        &analysis.plan,
        &analysis.chains,
        &analysis.config,
        |id| harness.adapters.get(id).is_some(),
        &mut warnings,
    )?;
    events::validate_review_identity(&review_plan, analysis.plan.tasks.len(), &opts.plan_path)?;
    review_plan.pass_timeout()?;

    let mut agents: Vec<String> = analysis
        .chains
        .iter()
        .flat_map(|chain| chain.rungs.iter().map(|rung| rung.binding.agent.clone()))
        .chain(review_plan.agents().into_iter().map(str::to_owned))
        .collect();
    agents.sort_unstable();
    agents.dedup();

    let gate_cmds: Vec<GateSummary> = analysis.gates.iter().map(gate_summary).collect();
    if kind == RunnerKind::Host && !analysis.gates.is_empty() {
        let mut shells: Vec<ShellKind> = analysis.gates.iter().map(|gate| gate.shell).collect();
        shells.sort_unstable_by_key(|shell| shell.program());
        shells.dedup();
        for shell in shells {
            gates::shell_available(shell)?;
        }
        gates::resolve_programs(&analysis.gates, &opts.repo_root, &mut warnings)?;
    }

    refuse_unshareable_width(analysis.config.max_parallel, harness)?;
    let ceiling = ceiling_of(opts.budget_usd, &analysis.config.budgets)?;
    let slots = SlotLimits::new(analysis.config.max_per_agent, analysis.config.max_per_pool)?;
    let mode = opts.interaction.unwrap_or(analysis.config.interaction_mode);
    let notifiers = interaction::notifiers_for(&analysis.config.notify, &mut warnings);

    Ok(FreshPrepared {
        validate_opts,
        captured,
        analysis,
        review_plan,
        agents,
        gate_cmds,
        mode,
        notifiers,
        ceiling,
        slots,
        authorization,
        image_environment,
        warnings,
    })
}

impl FreshPrepared {
    #[must_use]
    pub fn private_root(&self) -> &Path {
        match &self.authorization {
            Authorization::Checked(checked) => checked.private_root(),
            Authorization::Pending { root, .. } => root,
        }
    }

    pub fn lease(self, opts: &RunOptions) -> Result<FreshLeased, UpstrokeError> {
        let mut hooks = NoTopologyHooks::new();
        let workspace = Workspace::open(&opts.repo_root)?;
        let worktree_git_dir = workspace.worktree_git_dir()?;
        let worktree =
            WorktreeLock::acquire_in_hooked(&opts.repo_root, &worktree_git_dir, hooks.rundir())?;
        Ok(FreshLeased {
            prepared: self,
            workspace,
            worktree_git_dir,
            worktree,
            hooks,
        })
    }
}

impl FreshLeased {
    #[must_use]
    pub fn private_root(&self) -> &Path {
        self.prepared.private_root()
    }

    pub fn run(
        self,
        opts: &RunOptions,
        harness: &Harness<'_>,
        repository: ManagedRepository,
    ) -> Result<RunReport, UpstrokeError> {
        let Self {
            prepared:
                FreshPrepared {
                    validate_opts,
                    captured,
                    analysis,
                    review_plan,
                    agents,
                    gate_cmds,
                    mode,
                    notifiers,
                    ceiling,
                    slots,
                    authorization,
                    image_environment,
                    mut warnings,
                },
            workspace,
            worktree_git_dir,
            worktree,
            mut hooks,
        } = self;
        let checked = match authorization {
            Authorization::Checked(checked) => checked,
            Authorization::Pending {
                resolved,
                ids,
                root,
            } => resolved.authorize(&root, &ids)?,
        };
        let runtime = container_runtime::for_inspection();
        let view = RoleGitView::new(ContainerTrace::off());

        confirm_unchanged(&validate_opts, &captured)?;
        let repo_key = RepoKey::for_worktree_git_dir(&worktree_git_dir)?;
        let include_spelling = repository.common_dir().to_path_buf();
        let runner: Arc<dyn Runner> = match image_environment {
            None => Arc::new(host_runner(repository)),
            Some(base) => Arc::new(container_runner(
                checked.runner_policy(),
                RunIdentity {
                    private_root: checked.private_root().to_path_buf(),
                    run_id: checked.run_id().to_owned(),
                    run_dir: rundir::public_dir(&opts.repo_root, checked.run_id()),
                    incarnation: checked.incarnation().0.clone(),
                    repo_key: repo_key.as_str().to_owned(),
                },
                &opts.repo_root,
                RepositoryGit {
                    git_dir: &worktree_git_dir,
                    common_dir: repository.common_dir(),
                },
                base,
            )?),
        };
        let census = startup_census(
            WorktreeLocked::from(worktree),
            &mut hooks,
            &CensusInputs {
                repo_root: &opts.repo_root,
                repo_key: &repo_key,
                authorized_root: checked.private_root(),
                incarnation: &checked.incarnation().0,
                runtime: &*runtime,
                liveness: &LockProbe,
                view: &view,
            },
        )?;
        let (locked, _census) = census.into_parts();

        let manager = WorkspaceManager::derive(
            &opts.repo_root,
            checked.private_root(),
            checked.run_id(),
            &checked.incarnation().0,
        )?;
        one_spelling(&include_spelling, &manager)?;
        let base_sha = workspace.head_sha_full()?;

        let normalized_plan = normalized_plan_bytes(&analysis.plan, &opts.plan_path)?;
        let inputs = FrozenInputs {
            plan: analysis.plan.clone(),
            normalized_plan_digest: events::normalized_plan_digest(&normalized_plan),
        };
        let record = fresh_record(&FreshRecord {
            opts,
            analysis: &analysis,
            checked: &checked,
            manager: &manager,
            agents: &agents,
            gate_cmds: &gate_cmds,
            review_plan: &review_plan,
            mode,
            base_sha,
            normalized_plan_digest: &inputs.normalized_plan_digest,
        })?;

        let probes = RunnerProbes::new(
            analysis.config.shell,
            crate::agent::probe_workspace(),
            harness.adapters,
            checked.runner_policy_sha256().to_owned(),
        );
        let ledger = Mutex::new(InvocationLedger::new());
        let request = Request {
            repo_root: &opts.repo_root,
            repo_key,
            normalized_plan: &normalized_plan,
            inputs: inputs.clone(),
            record,
            root: &manager,
            agents: &agents,
            probes: &probes,
            refs: &manager,
            clock: &SystemClock,
            runner: &*runner,
            pair: ProbePair::grant(&ledger),
        };
        let started = create_run(checked, request, &mut hooks, &mut warnings)
            .map_err(super::create::Refused::into_error)?;

        let paths = started.paths().clone();
        let handle = started.into_handle(locked.into_lock());
        let driving = Driving {
            manager,
            runner,
            paths,
            gates: analysis.gates,
            pools: analysis.config.pools,
            caps: probes.probed_caps(),
            worker_timeout: opts.attempt_timeout,
            mode,
            wait_on_block: opts.wait_on_block.unwrap_or(analysis.config.wait_on_block),
            halts_run: analysis.config.on_task_failure == OnTaskFailure::Halt,
            slots,
        };
        let run = TopologyRun::resumed(handle, inputs, ceiling)
            .with_spend_approval(analysis.config.ask_before.frontier_escalation_over_usd)
            .with_notifiers(notifiers);
        finish(run, &driving, harness, warnings)
    }
}

pub struct ResumePrepared {
    root: RootDerived,
    settled: ResumeSettled,
}

pub struct ResumeVerified {
    records: RecordsVerified,
    settled: ResumeSettled,
    worktree_git_dir: PathBuf,
    repo_key: RepoKey,
    incarnation: IncarnationId,
    manager: WorkspaceManager,
    hooks: NoTopologyHooks,
}

struct ResumeSettled {
    started: RunStarted4,
    inputs: FrozenInputs,
    config: config::Config,
    shell: ShellKind,
    mode: InteractionMode,
    notifiers: Vec<&'static dyn Notifier>,
    ceiling: Ceiling,
    slots: SlotLimits,
    image_environment: Option<Vec<(String, String)>>,
    warnings: Vec<String>,
}

pub fn route_resume(opts: &ResumeOptions) -> Result<Option<RootDerived>, UpstrokeError> {
    let run_id = rundir::resolve_run_id(&opts.repo_root, &opts.run_id)?;
    let log = rundir::public_dir(&opts.repo_root, &run_id).join(rundir::EVENT_LOG);
    let bytes = read_initial_log(&log)?;
    let Ok(header) = probe_header(&bytes) else {
        return Ok(None);
    };
    match select_for_schema(header.schema, MAX_READABLE_SCHEMA) {
        Ok(ReaderSelection::Topology) => {}
        Ok(ReaderSelection::Legacy { .. }) => return Ok(None),
        Err(refusal) => {
            return Err(UpstrokeError::Refused {
                message: format!("{refusal} ({})", log.display()),
            });
        }
    }

    RootDerived::derive(&opts.repo_root, &run_id, opts.private_root.as_deref()).map(Some)
}

pub(super) fn read_initial_log(log: &Path) -> Result<Vec<u8>, UpstrokeError> {
    initial_log_read_fault::refuse(log)?;
    crate::util::read_file_bounded(log).map_err(|source| UpstrokeError::Io {
        path: log.to_path_buf(),
        source,
    })
}

#[cfg(not(test))]
mod initial_log_read_fault {
    use std::path::Path;

    use crate::error::UpstrokeError;

    #[inline]
    pub(super) fn refuse(_log: &Path) -> Result<(), UpstrokeError> {
        Ok(())
    }
}

pub fn prepare_resume(
    opts: &ResumeOptions,
    harness: &Harness<'_>,
    root: RootDerived,
) -> Result<ResumePrepared, UpstrokeError> {
    crate::workspace_manager::refuse_relative_repository_wide_names()?;
    let run_id = root.run_id().to_owned();
    let started = root.started().clone();
    match std::fs::symlink_metadata(root.private_dir()) {
        Ok(metadata) if metadata.is_dir() => {}
        _ => {
            return Err(UpstrokeError::Refused {
                message: format!(
                    "run `{run_id}` records its private half at {}, which is not a directory \
                     here. A resume never creates a missing private half, so nothing was \
                     published, locked or written",
                    root.private_dir().display()
                ),
            });
        }
    }

    let paths = RunPaths::from_parts(
        root.public_dir().to_path_buf(),
        root.private_dir().to_path_buf(),
    );
    let inputs = frozen_inputs(&paths, &started)?;

    let config_path = opts.config_path.clone().or_else(|| {
        started
            .config_path
            .as_ref()
            .map(|path| opts.repo_root.join(path))
    });
    let mut warnings = Vec::new();
    let config = config::load_limits(
        config_path.as_deref(),
        &opts.repo_root,
        opts.pools_path.as_deref(),
        EngineLimits::TopologyResume,
        &mut warnings,
    )?;
    compare_recorded_limits(&config, &started, &mut warnings);
    compare_recorded_gates(&config, &opts.repo_root, &started, &mut warnings);

    refuse_unshareable_width(started.limits.max_parallel, harness)?;
    hold_recorded_case_policy(&started, &mut warnings)?;
    refuse_recorded_mount_targets(&started)?;
    refuse_withheld_recorded_mounts(&started, &opts.repo_root, root.private_root())?;
    let image_environment = match started.runner.kind {
        RunnerKind::Host => None,
        RunnerKind::Container => Some(recorded_image_environment(
            &*container_runtime::for_inspection(),
            &started.runner,
        )?),
    };
    let ceiling = ceiling_of(opts.budget_usd, &config.budgets)?;
    let slots = resumed_slot_limits(&config, started.limits.max_parallel)?;
    let shell = recorded_shell(&started.run_id, started.shell, config.shell, &mut warnings);
    let mode = opts.interaction.unwrap_or(config.interaction_mode);
    let notifiers = interaction::notifiers_for(&config.notify, &mut warnings);

    Ok(ResumePrepared {
        root,
        settled: ResumeSettled {
            started,
            inputs,
            config,
            shell,
            mode,
            notifiers,
            ceiling,
            slots,
            image_environment,
            warnings,
        },
    })
}

impl ResumePrepared {
    pub fn verify(self, opts: &ResumeOptions) -> Result<ResumeVerified, UpstrokeError> {
        let Self { root, settled } = self;
        let mut hooks = NoTopologyHooks::new();

        let workspace = Workspace::open(&opts.repo_root)?;
        let worktree_git_dir = workspace.worktree_git_dir()?;
        let repo_key = RepoKey::for_worktree_git_dir(&worktree_git_dir)?;
        let incarnation = RealIds.incarnation();
        let manager = WorkspaceManager::derive(
            &opts.repo_root,
            root.private_root(),
            root.run_id(),
            &incarnation.0,
        )?;
        let records = verify_for_recovery(
            root,
            &opts.repo_root,
            &worktree_git_dir,
            &repo_key,
            &mut hooks,
        )?;
        hold_recorded_execution_root(records.locks().root(), &manager)?;
        Ok(ResumeVerified {
            records,
            settled,
            worktree_git_dir,
            repo_key,
            incarnation,
            manager,
            hooks,
        })
    }
}

impl ResumeVerified {
    #[must_use]
    pub fn private_root(&self) -> &Path {
        self.records.locks().root().private_root()
    }

    pub fn run(
        self,
        opts: &ResumeOptions,
        harness: &Harness<'_>,
        repository: ManagedRepository,
    ) -> Result<RunReport, UpstrokeError> {
        let Self {
            records,
            settled:
                ResumeSettled {
                    started,
                    inputs,
                    config,
                    shell,
                    mode,
                    notifiers,
                    ceiling,
                    slots,
                    image_environment,
                    mut warnings,
                },
            worktree_git_dir,
            repo_key,
            incarnation,
            manager,
            mut hooks,
        } = self;
        let runtime = container_runtime::for_inspection();
        let view = RoleGitView::new(ContainerTrace::off());

        let root = records.locks().root();
        let include_spelling = repository.common_dir().to_path_buf();
        let runner: Arc<dyn Runner> = match image_environment {
            None => Arc::new(host_runner(repository)),
            Some(base) => Arc::new(container_runner(
                &started.runner,
                RunIdentity {
                    private_root: root.private_root().to_path_buf(),
                    run_id: root.run_id().to_owned(),
                    run_dir: root.public_dir().to_path_buf(),
                    incarnation: incarnation.0.clone(),
                    repo_key: repo_key.as_str().to_owned(),
                },
                &opts.repo_root,
                RepositoryGit {
                    git_dir: &worktree_git_dir,
                    common_dir: repository.common_dir(),
                },
                base,
            )?),
        };
        one_spelling(&include_spelling, &manager)?;
        let paths = RunPaths::from_parts(
            root.public_dir().to_path_buf(),
            root.private_dir().to_path_buf(),
        );
        let preflight = RunPreflight::new(
            &*runner,
            harness.adapters,
            shell,
            &crate::agent::probe_workspace(),
            started.probed_agents,
        );
        let (_recovered, handle) = recover_verified(
            records,
            &ResumeSeams {
                repo_root: &opts.repo_root,
                worktree_git_dir: &worktree_git_dir,
                repo_key: &repo_key,
                incarnation: &incarnation,
                inputs: inputs.clone(),
                today: &config.runner,
                runtime: &*runtime,
                liveness: &LockProbe,
                view: &view,
                preflight: &preflight,
                refs: &manager,
                manager: &manager,
                clock: &SystemClock,
            },
            &mut hooks,
            &mut warnings,
        )?;
        let caps = preflight
            .probed()
            .map(|probed| probed.caps)
            .unwrap_or_default();
        drop(preflight);

        let driving = Driving {
            manager,
            runner,
            paths,
            gates: started
                .gate_cmds
                .iter()
                .map(ShellGate::from_record)
                .collect(),
            pools: config.pools,
            caps,
            worker_timeout: opts.attempt_timeout,
            mode,
            wait_on_block: opts.wait_on_block.unwrap_or(config.wait_on_block),
            halts_run: config.on_task_failure == OnTaskFailure::Halt,
            slots,
        };
        let run = TopologyRun::resumed(handle, inputs, ceiling)
            .with_spend_approval(config.ask_before.frontier_escalation_over_usd)
            .with_notifiers(notifiers);
        finish(run, &driving, harness, warnings)
    }
}

struct FreshRecord<'a> {
    opts: &'a RunOptions,
    analysis: &'a Analysis,
    checked: &'a PreLockChecked,
    manager: &'a WorkspaceManager,
    agents: &'a [String],
    gate_cmds: &'a [GateSummary],
    review_plan: &'a ReviewPlan,
    mode: InteractionMode,
    base_sha: String,
    normalized_plan_digest: &'a str,
}

fn fresh_record(fresh: &FreshRecord<'_>) -> Result<RunStarted4, UpstrokeError> {
    let run_id = fresh.checked.run_id();
    let mut record = RunStarted4 {
        schema: TOPOLOGY_SCHEMA,
        upstroke_version: env!("CARGO_PKG_VERSION").to_owned(),
        run_id: run_id.to_owned(),
        incarnation: fresh.checked.incarnation().clone(),
        runner: fresh.checked.runner_policy().clone(),
        probed_agents: fresh.agents.to_vec(),
        shell: fresh.analysis.config.shell,
        branch: format!("upstroke/run-{run_id}"),
        integration_ref: GitRef(format!("refs/heads/upstroke/run-{run_id}")),
        base_sha: CommitSha(fresh.base_sha.clone()),
        execution_root: recorded_path(fresh.manager.execution_root(), "execution root")?,
        private_dir: recorded_path(&fresh.checked.private_dir(), "private half")?,
        plan_path: repo_relative(&fresh.opts.repo_root, &fresh.opts.plan_path),
        config_path: fresh
            .opts
            .config_path
            .as_ref()
            .map(|path| repo_relative(&fresh.opts.repo_root, path)),
        plan_hash: fresh.analysis.plan.source.hash.clone(),
        normalized_plan_digest: fresh.normalized_plan_digest.to_owned(),
        registry_digest: String::new(),
        path_policy: PathPolicy {
            version: PathPolicyVersion::V2,
            case_fold: false,
            grammar: PathGrammar::Globset,
        },
        limits: TopologyLimits {
            max_parallel: fresh.analysis.config.max_parallel,
            max_defers: fresh.opts.max_defers,
            max_merge_repairs: fresh.analysis.config.max_merge_repairs,
        },
        gates: fresh
            .gate_cmds
            .iter()
            .map(|gate| gate.name.clone())
            .collect(),
        gates_from_config: fresh.analysis.gates_from_config,
        gate_cmds: fresh.gate_cmds.to_vec(),
        interaction_mode: fresh.mode.to_string(),
        chains: chain_summaries(fresh.analysis),
        effort_policy: fresh.analysis.config.resolved_effort_policy(),
        reviews: fresh.review_plan.clone(),
    };
    record.registry_digest = TaskRegistry::originals_with_agents(
        &fresh.analysis.plan,
        &record.registry_record(),
        &record.probed_agents,
    )
    .map_err(|error| UpstrokeError::Refused {
        message: format!("the run record does not derive a task registry: {error}"),
    })?
    .digest();
    Ok(record)
}

/// A path the run record spells as text, spelled exactly.
///
/// `prelock` refuses a private root that is not valid UTF-8 before any lock, and both recorded
/// paths hang from that root by ASCII names, so this refuses nothing a fresh run can reach. It
/// replaces `to_string_lossy`, which recorded a different path than the run used.
fn recorded_path(path: &Path, what: &str) -> Result<String, UpstrokeError> {
    match path.to_str() {
        Some(text) => Ok(text.to_owned()),
        None => Err(UpstrokeError::Refused {
            message: format!(
                "the run's {what} {} is not valid UTF-8, so its record cannot spell it exactly",
                path.display()
            ),
        }),
    }
}

impl super::create::RootCreation for WorkspaceManager {
    fn create(
        &self,
        hooks: &mut dyn EffectHooks,
    ) -> Result<(), crate::workspace_manager::FreshRootFailure> {
        crate::workspace_manager::FreshExecutionRoot(self).create_execution_root(hooks)
    }

    fn case_fold(&self, warnings: &mut Vec<String>) -> bool {
        resolve_case_fold(
            self.execution_root(),
            &observe_case_at(self.execution_root(), "tasks"),
            &recorded_ignorecase(self),
            warnings,
        )
    }
}

struct Driving {
    manager: WorkspaceManager,
    runner: Arc<dyn Runner>,
    paths: RunPaths,
    gates: Vec<ShellGate>,
    pools: Vec<Pool>,
    caps: Vec<(String, Caps)>,
    worker_timeout: Duration,
    mode: InteractionMode,
    wait_on_block: Duration,
    halts_run: bool,
    slots: SlotLimits,
}

fn finish(
    mut run: TopologyRun,
    driving: &Driving,
    harness: &Harness<'_>,
    mut warnings: Vec<String>,
) -> Result<RunReport, UpstrokeError> {
    let outcome = drive(&mut run, driving, harness)?;
    warnings.extend(run.warnings().iter().cloned());
    let mut report = RunReport::from_topology(run.fold(), run.events(), warnings);
    let reported = reported_outcome(&outcome);
    if report.outcome() != reported {
        report.warnings.push(format!(
            "the run finished {reported:?}, and this report's own reading of the log says {:?}; \
             the run's `report.json` is the authority",
            report.outcome()
        ));
    }
    Ok(report)
}

fn drive(
    run: &mut TopologyRun,
    driving: &Driving,
    harness: &Harness<'_>,
) -> Result<RunOutcome, UpstrokeError> {
    let sleeper: &dyn Sleeper = harness.sleeper.unwrap_or(&RealSleeper);
    let default_answers = DeliveredFirst::new(
        driving.paths.answers(),
        interaction::answers_for(
            driving.mode,
            driving.paths.answers(),
            driving.wait_on_block,
            sleeper,
        ),
        sleeper,
    );
    let answers: &dyn AnswerSource = harness.answers.unwrap_or(&default_answers);
    let plans = FrozenPlans {
        adapters: harness.adapters,
        paths: &driving.paths,
        gates: &driving.gates,
        pools: &driving.pools,
        caps: &driving.caps,
        worker_timeout: driving.worker_timeout,
        decisions: &[],
    };
    let seams = RunSeams {
        manager: &driving.manager,
        clock: &SystemClock,
        sleeper,
        runner: &*driving.runner,
        adapters: harness.adapters,
        paths: &driving.paths,
        plans: &plans,
        reviews: &LegacyReviewPasses,
        input_policy: &LegacyReviewInputPolicy,
        answers,
        ids: &RealIds,
        halts_run: driving.halts_run,
    };
    let mut hooks = NoTopologyHooks::new();
    let mut progress = match harness.adapters.shared() {
        Some(adapters) => {
            let pipelines = PipelineSeams {
                manager: driving.manager.clone(),
                runner: Arc::clone(&driving.runner),
                adapters: Arc::clone(&adapters),
                paths: driving.paths.clone(),
                plans: Arc::new(SharedPlans {
                    adapters,
                    paths: driving.paths.clone(),
                    gates: driving.gates.clone(),
                    pools: driving.pools.clone(),
                    caps: driving.caps.clone(),
                    worker_timeout: driving.worker_timeout,
                    decisions: Vec::new(),
                }),
                reviews: Arc::new(LegacyReviewPasses),
                input_policy: Arc::new(LegacyReviewInputPolicy),
                hooks: Arc::new(|| {
                    Box::new(NoTopologyHooks::new()) as Box<dyn TopologyHooks + Send>
                }),
                slots: driving.slots,
            };
            run.run_concurrently(&seams, &pipelines, &mut hooks, None)?
        }
        None => run.step(&seams, &mut hooks)?,
    };
    loop {
        if let Progress::Finished { outcome, .. } = progress {
            return Ok(outcome);
        }
        progress = run.step(&seams, &mut hooks)?;
    }
}

struct DeliveredFirst<'a> {
    delivered: interaction::EventLogAnswers<'a>,
    attended: Box<dyn AnswerSource + 'a>,
    answered_by: Mutex<&'static str>,
}

impl<'a> DeliveredFirst<'a> {
    fn new(
        answers: PathBuf,
        attended: Box<dyn AnswerSource + 'a>,
        sleeper: &'a dyn Sleeper,
    ) -> Self {
        let answered_by = Mutex::new(attended.id());
        Self {
            delivered: interaction::EventLogAnswers::new(answers, Duration::ZERO, sleeper),
            attended,
            answered_by,
        }
    }

    fn answered(&self, by: &'static str, answer: Answer) -> Answer {
        if let Ok(mut answered_by) = self.answered_by.lock() {
            *answered_by = by;
        }
        answer
    }
}

impl AnswerSource for DeliveredFirst<'_> {
    fn id(&self) -> &'static str {
        self.answered_by
            .lock()
            .map_or_else(|_| self.attended.id(), |answered_by| *answered_by)
    }

    fn poll(&self, question: &Question) -> Result<Answer, UpstrokeError> {
        let delivered = self.delivered.poll(question)?;
        if delivered != Answer::Unanswered {
            return Ok(self.answered(self.delivered.id(), delivered));
        }
        let attended = self.attended.poll(question)?;
        Ok(self.answered(self.attended.id(), attended))
    }

    fn resolve(&self, question: &Question) -> Result<Answer, UpstrokeError> {
        let delivered = self.delivered.poll(question)?;
        if delivered != Answer::Unanswered {
            return Ok(self.answered(self.delivered.id(), delivered));
        }
        let attended = self.attended.resolve(question)?;
        Ok(self.answered(self.attended.id(), attended))
    }
}

fn reported_outcome(outcome: &RunOutcome) -> ReportedOutcome {
    match outcome {
        RunOutcome::Complete => ReportedOutcome::Complete,
        RunOutcome::Parked => ReportedOutcome::Parked,
        RunOutcome::Halted => ReportedOutcome::Halted,
        RunOutcome::BudgetExceeded => ReportedOutcome::BudgetExceeded,
    }
}

fn one_spelling(include: &Path, manager: &WorkspaceManager) -> Result<(), UpstrokeError> {
    if include == manager.common_git_dir() {
        return Ok(());
    }
    Err(UpstrokeError::Refused {
        message: format!(
            "the recorded-objects include names the managed repository's common directory as \
             {}, and the workspace manager registers and verifies every slot against {}. A \
             role's `.git` file is spelled from the second and the include's conditions from the \
             first, so the two must be one spelling; that they differ is a defect, not a refusal \
             of this layout. No role was started",
            include.display(),
            manager.common_git_dir().display()
        ),
    })
}

fn hold_recorded_execution_root(
    root: &RootDerived,
    manager: &WorkspaceManager,
) -> Result<(), UpstrokeError> {
    let recorded = &root.started().execution_root;
    if Path::new(recorded) == manager.execution_root() {
        return Ok(());
    }
    Err(UpstrokeError::Refused {
        message: format!(
            "run `{}` records its execution root as {recorded}, and the workspace manager derives \
             {} for it today, from the canonical private root {} and the repository's effective \
             common directory {}. Every slot, intent, snapshot and staging directory the run owns \
             is under the root it recorded, and recovery and finalization act only on what their \
             manager's root holds, so a resume never adopts another root: nothing was published, \
             recovered, reclaimed or created. Resume it under the Git environment it started \
             under, whose common directory derives the root it recorded",
            root.run_id(),
            manager.execution_root().display(),
            manager.private_root().display(),
            manager.common_git_dir().display()
        ),
    })
}

fn host_runner(repository: ManagedRepository) -> HostRunner {
    HostRunner::new().with_environment(
        HostEnvironment::from_process().reading(ObjectGraph::RecordedIn(repository)),
    )
}

fn gate_summary(gate: &ShellGate) -> GateSummary {
    GateSummary {
        name: gate.name.clone(),
        cmd: gate.cmd.clone(),
        timeout: gate.timeout,
        shell: gate.shell,
    }
}

fn confirm_unchanged(
    validate_opts: &ValidateOptions,
    captured: &CapturedInputs,
) -> Result<(), UpstrokeError> {
    if CapturedInputs::capture(validate_opts) == *captured {
        return Ok(());
    }
    Err(UpstrokeError::Refused {
        message: format!(
            "{} changed between the checks this run passed before the worktree lock and the \
             lock itself; refusing to create a run from inputs other than the ones it checked",
            captured
                .paths()
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    })
}

pub(super) fn frozen_inputs(
    paths: &RunPaths,
    started: &RunStarted4,
) -> Result<FrozenInputs, UpstrokeError> {
    let plan_path = paths.plan_json();
    let bytes = crate::util::read_file_bounded(&plan_path).map_err(|source| UpstrokeError::Io {
        path: plan_path.clone(),
        source,
    })?;
    let digest = events::normalized_plan_digest(&bytes);
    if digest != started.normalized_plan_digest {
        return Err(UpstrokeError::Refused {
            message: format!(
                "the frozen plan {} digests to `{digest}`, and run `{}` recorded `{}`; a resume \
                 adopts only the plan its run was created with",
                plan_path.display(),
                started.run_id,
                started.normalized_plan_digest
            ),
        });
    }
    let plan: Plan = serde_json::from_slice(&bytes).map_err(|error| UpstrokeError::Parse {
        message: format!("{}: {error}", plan_path.display()),
    })?;
    Ok(FrozenInputs {
        plan,
        normalized_plan_digest: digest,
    })
}

fn compare_recorded_limits(
    config: &config::Config,
    started: &RunStarted4,
    warnings: &mut Vec<String>,
) {
    let recorded = &started.limits;
    for (key, today, kept) in [
        ("max_parallel", config.max_parallel, recorded.max_parallel),
        (
            "max_merge_repairs",
            config.max_merge_repairs,
            recorded.max_merge_repairs,
        ),
    ] {
        if today != kept {
            warnings.push(format!(
                "[engine] `{key} = {today}` differs from the `{key} = {kept}` run `{}` recorded; \
                 a run keeps the limits it started with, so this resume continues at {kept}",
                started.run_id
            ));
        }
    }
}

/// A run keeps the gates it recorded, and its resume says how today's config differs, as the
/// sequential resume does (`preflight::gates_differ`, DESIGN §17). Today's gates are the ones a
/// fresh run would take: the config's `[[gates]]` under its shell, or, with none, the gates
/// derived from the repository's shape (`validate::analyze_captured`).
fn compare_recorded_gates(
    config: &config::Config,
    repo_root: &Path,
    started: &RunStarted4,
    warnings: &mut Vec<String>,
) {
    let today: Vec<GateSummary> = match &config.gates {
        Some(configured) => configured
            .iter()
            .map(|gate| GateSummary {
                name: gate.name.clone(),
                cmd: gate.cmd.clone(),
                timeout: gate.timeout,
                shell: config.shell,
            })
            .collect(),
        None => gates::derive(repo_root, config.shell)
            .iter()
            .map(gate_summary)
            .collect(),
    };
    if let Some(difference) = gates_differ(&started.gate_cmds, &today) {
        warnings.push(difference);
    }
}

fn recorded_shell(
    run_id: &str,
    recorded: ShellKind,
    today: ShellKind,
    warnings: &mut Vec<String>,
) -> ShellKind {
    if today != recorded {
        warnings.push(format!(
            "[engine] `shell = {today}` differs from the `shell = {kept}` run `{run_id}` \
             recorded; a run keeps the shell it started with, so this resume's pre-flight \
             probes `{kept}`",
            today = today.program(),
            kept = recorded.program(),
        ));
    }
    recorded
}

fn resumed_slot_limits(
    config: &config::Config,
    recorded_width: u32,
) -> Result<SlotLimits, UpstrokeError> {
    SlotLimits::new(
        config.explicit_max_per_agent.unwrap_or(recorded_width),
        config.explicit_max_per_pool.unwrap_or(recorded_width),
    )
}

fn recorded_image_environment(
    runtime: &dyn ContainerRuntime,
    policy: &RunnerPolicy,
) -> Result<Vec<(String, String)>, UpstrokeError> {
    let image = policy
        .image
        .as_ref()
        .ok_or_else(|| UpstrokeError::Refused {
            message: "the container runner policy records no image identity, so there is no image \
                  environment to read"
                .to_owned(),
        })?;
    let environment = runtime
        .image_environment(&image.id)
        .map_err(|error| match error {
            RuntimeError::Unreachable { operation, detail } => {
                InspectionRefusal::RuntimeUnavailable { operation, detail }
            }
            RuntimeError::Failed { operation, detail }
            | RuntimeError::Unresolved { operation, detail } => {
                InspectionRefusal::RuntimeFailed { operation, detail }
            }
        })?
        .ok_or_else(|| InspectionRefusal::ImageIdAbsent {
            id: image.id.clone(),
        })?;
    ContainerEnvironment::from_image(environment.clone()).certify_path(&environment)?;
    Ok(environment)
}

fn container_runner(
    policy: &RunnerPolicy,
    identity: RunIdentity,
    repo_root: &Path,
    git: RepositoryGit<'_>,
    base: Vec<(String, String)>,
) -> Result<ContainerRunner, UpstrokeError> {
    let runtime = container_runtime::for_runner(&identity);
    ContainerRunner::new(
        policy.clone(),
        identity,
        repo_root,
        git,
        ContainerEnvironment::from_image(base),
        runtime,
    )
}

#[cfg(not(test))]
mod container_runtime {
    use crate::runner::container::DockerCli;
    use crate::runner::container::exec::RunIdentity;
    use crate::runner::container::runtime::ContainerRuntime;

    #[inline]
    pub(super) fn for_inspection() -> Box<dyn ContainerRuntime> {
        Box::new(DockerCli::default())
    }

    #[inline]
    pub(super) fn for_runner(_identity: &RunIdentity) -> Box<dyn ContainerRuntime> {
        Box::new(DockerCli::default())
    }
}

fn mount_context(
    selection: &config::RunnerSelection,
    repo_root: &Path,
    private_root: &Path,
) -> Result<MountContext, UpstrokeError> {
    if selection.mounts.is_empty() {
        return Ok(MountContext::unbound());
    }
    checkout_mount_context(repo_root, private_root)
}

fn refuse_recorded_mount_targets(started: &RunStarted4) -> Result<(), UpstrokeError> {
    refuse_recorded_targets(&started.runner.mounts).map_err(|(mount, reason)| {
        UpstrokeError::Refused {
            message: format!(
                "run `{}` records the mount {}, which no container of this resume may receive: \
                 {reason}. A fresh run refuses that target, and so does every use of the \
                 record; nothing was published, locked or written",
                started.run_id,
                mount_label(&mount.source, &mount.target)
            ),
        }
    })
}

fn refuse_withheld_recorded_mounts(
    started: &RunStarted4,
    repo_root: &Path,
    private_root: &Path,
) -> Result<(), UpstrokeError> {
    if started.runner.mounts.is_empty() {
        return Ok(());
    }
    let context = checkout_mount_context(repo_root, private_root)?;
    for mount in &started.runner.mounts {
        if let Err(reason) = refuse_excluded(Path::new(&mount.source), context.excluded()) {
            return Err(UpstrokeError::Refused {
                message: format!(
                    "run `{}` records the mount {}, which no container of this resume may \
                     receive: {reason}. A run keeps the mounts it started with, so it cannot \
                     continue while that holds; nothing was published, locked or written",
                    started.run_id,
                    mount_label(&mount.source, &mount.target)
                ),
            });
        }
    }
    Ok(())
}

fn checkout_mount_context(
    repo_root: &Path,
    private_root: &Path,
) -> Result<MountContext, UpstrokeError> {
    let workspace = Workspace::open(repo_root)?;
    let git_dir = workspace.worktree_git_dir()?;
    let common_dir = effective_common_dir(
        workspace.root(),
        &git_dir,
        std::env::var_os(crate::workspace_manager::COMMON_DIR).as_deref(),
    )?;
    MountContext::bind(
        repo_root,
        RepositoryGit {
            git_dir: &git_dir,
            common_dir: &common_dir,
        },
        private_root,
    )
}

fn effective_common_dir(
    checkout: &Path,
    git_dir: &Path,
    inherited: Option<&std::ffi::OsStr>,
) -> Result<PathBuf, UpstrokeError> {
    if let Some(value) = inherited {
        return Ok(checkout.join(value));
    }
    let record = git_dir.join("commondir");
    match std::fs::read_to_string(&record) {
        Ok(text) => Ok(git_dir.join(text.trim_end_matches(['\n', '\r']))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(git_dir.to_path_buf()),
        Err(source) => Err(UpstrokeError::Io {
            path: record,
            source,
        }),
    }
}

fn refuse_unshareable_width(width: u32, harness: &Harness<'_>) -> Result<(), UpstrokeError> {
    if width <= 1 || harness.adapters.shared().is_some() {
        return Ok(());
    }
    Err(UpstrokeError::Refused {
        message: format!(
            "`max_parallel = {width}` runs pipelines on threads of their own, and the adapter \
             source this command was given cannot be shared across threads; nothing was \
             locked or created. Run at `max_parallel = 1`, or pass a source whose \
             `AdapterSource::shared` answers"
        ),
    })
}

fn hold_recorded_case_policy(
    started: &RunStarted4,
    warnings: &mut Vec<String>,
) -> Result<(), UpstrokeError> {
    let execution_root = PathBuf::from(&started.execution_root);
    let observed = if execution_root.is_dir() {
        observe_case_at(&execution_root, "tasks")
    } else {
        match execution_root
            .parent()
            .and_then(Path::parent)
            .and_then(|workspaces| workspaces.parent().map(|root| (root, workspaces)))
        {
            Some((root, workspaces)) => match workspaces.file_name().and_then(|name| name.to_str())
            {
                Some(name) => observe_case_at(root, name),
                None => CaseObservation::Unknown(format!(
                    "{} names no directory whose case can be looked up",
                    workspaces.display()
                )),
            },
            None => CaseObservation::Unknown(format!(
                "{} has no private root above it to observe",
                execution_root.display()
            )),
        }
    };
    case_policy_holds(
        &started.run_id,
        &started.execution_root,
        started.path_policy.case_fold,
        &observed,
        warnings,
    )
}

fn case_policy_holds(
    run_id: &str,
    execution_root: &str,
    recorded_fold: bool,
    observed: &CaseObservation,
    warnings: &mut Vec<String>,
) -> Result<(), UpstrokeError> {
    match (recorded_fold, observed) {
        (false, CaseObservation::Folds) => Err(UpstrokeError::Refused {
            message: format!(
                "run `{run_id}` recorded a case-sensitive path policy (`case_fold = false`), and \
                 the filesystem its execution root {execution_root} is on now folds case: two \
                 task paths that differ only in case would be leased as disjoint while naming one \
                 file. The record is never rewritten, so nothing was locked or recovered; resume \
                 where the execution root is case-sensitive, or start a new run here"
            ),
        }),
        (true, CaseObservation::Sensitive) => {
            warnings.push(format!(
                "run `{run_id}` recorded `case_fold = true`, and its execution root \
                 {execution_root} is case-sensitive now. The run keeps its record: task paths \
                 that differ only in case stay leased as one path, which costs only parallelism"
            ));
            Ok(())
        }
        (_, CaseObservation::Unknown(reason)) => {
            warnings.push(format!(
                "whether run `{run_id}`'s execution root {execution_root} folds case could not \
                 be observed ({reason}). The run keeps its record, `case_fold = {recorded_fold}`"
            ));
            Ok(())
        }
        (false, CaseObservation::Sensitive) | (true, CaseObservation::Folds) => Ok(()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CaseObservation {
    Folds,
    Sensitive,
    Unknown(String),
}

impl CaseObservation {
    const fn folds(&self) -> Option<bool> {
        match self {
            Self::Folds => Some(true),
            Self::Sensitive => Some(false),
            Self::Unknown(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum IgnoreCase {
    Set(bool),
    Absent,
    Unreadable(String),
}

fn resolve_case_fold(
    directory: &Path,
    observed: &CaseObservation,
    ignorecase: &IgnoreCase,
    warnings: &mut Vec<String>,
) -> bool {
    let fold = |why: String| {
        format!(
            "{why}, so this run records `case_fold = true`: task paths that differ only in case \
             are leased as one path, which costs only parallelism"
        )
    };
    match (observed, ignorecase) {
        (CaseObservation::Unknown(reason), _) => {
            warnings.push(fold(format!(
                "whether the execution root {} folds case could not be observed ({reason})",
                directory.display()
            )));
            true
        }
        (CaseObservation::Folds, _) | (CaseObservation::Sensitive, IgnoreCase::Set(true)) => true,
        (CaseObservation::Sensitive, IgnoreCase::Set(false) | IgnoreCase::Absent) => false,
        (CaseObservation::Sensitive, IgnoreCase::Unreadable(reason)) => {
            warnings.push(fold(format!(
                "the managed repository's `core.ignorecase` could not be read ({reason})"
            )));
            true
        }
    }
}

fn recorded_ignorecase(manager: &WorkspaceManager) -> IgnoreCase {
    match manager.core_ignorecase() {
        Ok(Some(set)) => IgnoreCase::Set(set),
        Ok(None) => IgnoreCase::Absent,
        Err(error) => IgnoreCase::Unreadable(error.to_string()),
    }
}

fn observe_case_at(directory: &Path, name: &str) -> CaseObservation {
    let other_name = name.to_ascii_uppercase();
    if other_name == name {
        return CaseObservation::Unknown(format!(
            "`{name}` has no lowercase ASCII letter to look up in the other case"
        ));
    }
    let spelled = directory.join(name);
    let other = directory.join(&other_name);
    let found = match std::fs::symlink_metadata(&spelled) {
        Ok(found) => found,
        Err(error) => {
            return CaseObservation::Unknown(format!(
                "{} could not be read: {error}",
                spelled.display()
            ));
        }
    };
    match std::fs::symlink_metadata(&other) {
        Ok(other_found) => same_entry(&spelled, &found, &other, &other_found),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => CaseObservation::Sensitive,
        Err(error) => CaseObservation::Unknown(format!(
            "{} could not be looked up: {error}",
            other.display()
        )),
    }
}

#[cfg(unix)]
fn same_entry(
    _spelled: &Path,
    found: &std::fs::Metadata,
    _other: &Path,
    other_found: &std::fs::Metadata,
) -> CaseObservation {
    use std::os::unix::fs::MetadataExt as _;
    if found.dev() == other_found.dev() && found.ino() == other_found.ino() {
        CaseObservation::Folds
    } else {
        CaseObservation::Sensitive
    }
}

#[cfg(not(unix))]
fn same_entry(
    spelled: &Path,
    _found: &std::fs::Metadata,
    other: &Path,
    _other_found: &std::fs::Metadata,
) -> CaseObservation {
    match (std::fs::canonicalize(spelled), std::fs::canonicalize(other)) {
        (Ok(spelled), Ok(other)) if spelled == other => CaseObservation::Folds,
        (Ok(_), Ok(_)) => CaseObservation::Sensitive,
        (Err(error), _) | (_, Err(error)) => CaseObservation::Unknown(format!(
            "{} and {} could not both be resolved: {error}",
            spelled.display(),
            other.display()
        )),
    }
}

fn ceiling_of(flag: Option<f64>, configured: &Budgets) -> Result<Ceiling, UpstrokeError> {
    if let Some(limit) = flag {
        config::check_budget("--budget", limit)
            .map_err(|message| UpstrokeError::Refused { message })?;
    }
    Ok(Ceiling {
        run_usd: flag.or(configured.run_usd),
        task_usd: configured.task_usd,
    })
}

#[cfg(test)]
pub(crate) mod initial_log_read_fault {
    use std::cell::RefCell;
    use std::path::{Path, PathBuf};

    use crate::error::UpstrokeError;

    pub(crate) const INJECTED: &str = "injected failure of the initial complete log read";

    thread_local! {
        static ARMED: RefCell<Option<(PathBuf, usize)>> = const { RefCell::new(None) };
    }

    #[must_use]
    pub(crate) struct Armed(());

    pub(crate) fn arm(log: &Path) -> Armed {
        ARMED.with(|armed| *armed.borrow_mut() = Some((log.to_path_buf(), 0)));
        Armed(())
    }

    impl Armed {
        pub(crate) fn hits(&self) -> usize {
            ARMED.with(|armed| armed.borrow().as_ref().map_or(0, |(_, hits)| *hits))
        }
    }

    impl Drop for Armed {
        fn drop(&mut self) {
            ARMED.with(|armed| *armed.borrow_mut() = None);
        }
    }

    pub(super) fn refuse(log: &Path) -> Result<(), UpstrokeError> {
        ARMED.with(|armed| match armed.borrow_mut().as_mut() {
            Some((armed_log, hits)) if armed_log.as_path() == log => {
                *hits += 1;
                Err(UpstrokeError::Io {
                    path: log.to_path_buf(),
                    source: std::io::Error::other(INJECTED),
                })
            }
            _ => Ok(()),
        })
    }
}

#[cfg(test)]
pub(crate) mod container_runtime {
    use std::cell::RefCell;
    use std::panic::Location;

    use crate::runner::container::DockerCli;
    use crate::runner::container::exec::RunIdentity;
    use crate::runner::container::observed::Journal;
    use crate::runner::container::runtime::ContainerRuntime;

    thread_local! {
        static ARMED: RefCell<Option<Journal>> = const { RefCell::new(None) };
    }

    #[must_use]
    pub(crate) struct Armed(());

    pub(crate) fn arm(journal: &Journal) -> Armed {
        ARMED.with(|armed| *armed.borrow_mut() = Some(journal.clone()));
        Armed(())
    }

    impl Drop for Armed {
        fn drop(&mut self) {
            ARMED.with(|armed| *armed.borrow_mut() = None);
        }
    }

    #[track_caller]
    pub(crate) fn for_inspection() -> Box<dyn ContainerRuntime> {
        observed_or_plain(Location::caller(), None)
    }

    #[track_caller]
    pub(crate) fn for_runner(identity: &RunIdentity) -> Box<dyn ContainerRuntime> {
        observed_or_plain(Location::caller(), Some(identity))
    }

    fn observed_or_plain(
        site: &Location<'_>,
        identity: Option<&RunIdentity>,
    ) -> Box<dyn ContainerRuntime> {
        match ARMED.with(|armed| armed.borrow().clone()) {
            Some(journal) => Box::new(journal.observe(
                &format!("{}:{}", site.file().replace('\\', "/"), site.line()),
                identity,
            )),
            None => Box::new(DockerCli::default()),
        }
    }
}

#[cfg(test)]
mod tests;
