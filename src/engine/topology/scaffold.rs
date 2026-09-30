//! Extended notes: `docs/internals/engine/topology/scaffold.md`

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Condvar, Mutex};
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use crate::agent::proc::{ProcessOutput, SpawnHooks};
use crate::error::UpstrokeError;
use crate::events::log::{EventHooks, EventLog, TopologyLine, site_for};
use crate::events::{AttemptRecord, BindingSummary, ChainSummary, GateSummary, RunOutcome};
use crate::gates::ShellKind;
use crate::ir::{
    Artifact, ArtifactId, Effort, Plan, PlanSource, ResolvedEffortPolicy, Task, TaskId, TaskKind,
    Tier,
};
use crate::review::{PassBinding, ReviewPlan};
use crate::rundir::RunDirHooks;
use crate::runner::container::ContainerHooks;
use crate::runner::{
    AgentId, Cancellation, CommandSpec, ExecutionRole, InvocationId, ProbeTarget, RunFuture,
    Runner, RunnerCall, RunnerRequest,
};
use crate::topology::effects::{
    EffectSiteId, EventSite, HookHarness, HookPhase, Injection, InjectionMode, SubEffectPoint,
};
use crate::topology::events::{
    AttemptFinished4, AttemptNumber, AttemptSettlement, CommitSha, Epoch, FrozenSpawn,
    GenerationId, GitRef, IncarnationId, RunStarted4, RungBinding, RunnerContract, RunnerKind,
    RunnerPolicy, SessionId, SpawnAdmission, TaskSpawned, TopologyEvent, TopologyEventBody,
    TopologyLimits,
};
use crate::topology::fold::{FrozenInputs, GenerationClass, TaskFold, TaskState, TopologyFold};
use crate::topology::paths::{PathGrammar, PathPolicy, PathPolicyVersion, PathSet};
use crate::topology::registry::{Lineage, Origin, TaskKey, TaskRegistry, repair_display_id};
use crate::topology::schema::TOPOLOGY_SCHEMA;
use crate::util::DurabilityLedger;
use crate::workspace_manager::{
    EffectHooks, HarnessEffects, WorkspaceManager,
    fixture::{Fixture, died_by_abort, run_kill_child_within, write_file},
};

use super::attempt::{AttemptPlan, GatePlan, ReviewerPlan};
use super::dispatch::{DispatchKind, DispatchRequest, Dispatched, EventEmitter, dispatch};
use super::seams::TopologyHooks;

pub(super) const ALPHA: TaskKey = TaskKey(0);
pub(super) const BETA: TaskKey = TaskKey(1);

pub(super) const AGENT: &str = "claude-code";
pub(super) const REVIEW_AGENT: &str = "copilot";

fn probed_agents() -> Vec<String> {
    vec![AGENT.to_owned(), REVIEW_AGENT.to_owned()]
}

fn task_of(id: &str) -> Task {
    Task {
        id: TaskId::from(id),
        kind: TaskKind::Refactor,
        title: format!("{id} title"),
        body: format!("{id} body"),
        depends_on: Vec::new(),
        acceptance: vec![format!("{id} passes")],
        path_hints: vec![format!("src/{id}/")],
        suggested_tier: None,
        min_tier: None,
        artifacts_in: Vec::new(),
        artifacts_out: vec![ArtifactId::from(format!("{id}-out").as_str())],
    }
}

pub(super) fn plan() -> Plan {
    Plan {
        source: PlanSource {
            adapter: "markdown".to_owned(),
            hash: "scaffold-plan-hash".to_owned(),
        },
        tasks: vec![task_of("alpha"), task_of("beta")],
        artifacts: vec![Artifact {
            id: ArtifactId::from("alpha-out"),
            produced_by: Some(TaskId::from("alpha")),
        }],
    }
}

fn chain(task: &str) -> ChainSummary {
    let tiers = vec![Tier::Mid, Tier::Frontier];
    ChainSummary {
        task: task.to_owned(),
        attempts_per: 2,
        bindings: Some(
            tiers
                .iter()
                .map(|tier| BindingSummary {
                    tier: *tier,
                    agent: AGENT.to_owned(),
                    model: format!("{task}-{tier}-model"),
                    pinned: *tier == Tier::Frontier,
                })
                .collect(),
        ),
        tiers,
    }
}

pub(super) const NORMALIZED_DIGEST: &str =
    "sha256:1010101010101010101010101010101010101010101010101010101010101010";

fn run_started(fixture: &Fixture) -> RunStarted4 {
    run_started_of(fixture, &plan(), 1, false)
}

fn run_started_of(
    fixture: &Fixture,
    plan: &Plan,
    max_parallel: u32,
    second_opinion: bool,
) -> RunStarted4 {
    let unauthenticated = RunStarted4 {
        schema: TOPOLOGY_SCHEMA,
        upstroke_version: "0.2.0-scaffold".to_owned(),
        run_id: "01SCAFFOLD00000000000000AA".to_owned(),
        incarnation: IncarnationId("01SCAFFOLDINC0000000000000".to_owned()),
        runner: RunnerPolicy {
            kind: RunnerKind::Host,
            policy: RunnerContract::HostV1,
            image: None,
            credential_volumes: None,
        },
        probed_agents: probed_agents(),
        branch: "upstroke/run-01SCAFFOLD00000000000000AA".to_owned(),
        integration_ref: GitRef("refs/heads/upstroke/run-01SCAFFOLD00000000000000AA".to_owned()),
        base_sha: CommitSha(fixture.head.clone()),
        execution_root: fixture
            .manager
            .execution_root()
            .to_string_lossy()
            .into_owned(),
        private_dir: fixture.private.to_string_lossy().into_owned(),
        plan_path: "docs/plan.md".to_owned(),
        config_path: Some("upstroke.toml".to_owned()),
        plan_hash: plan.source.hash.clone(),
        normalized_plan_digest: NORMALIZED_DIGEST.to_owned(),
        registry_digest: String::new(),
        path_policy: PathPolicy {
            version: PathPolicyVersion::V2,
            case_fold: true,
            grammar: PathGrammar::Globset,
        },
        limits: TopologyLimits {
            max_parallel,
            max_defers: 2,
            max_merge_repairs: 3,
        },
        gates: vec!["fmt".to_owned()],
        gates_from_config: true,
        gate_cmds: vec![GateSummary {
            name: "fmt".to_owned(),
            cmd: "cargo fmt --check".to_owned(),
            timeout: Duration::from_secs(60),
            shell: ShellKind::Bash,
        }],
        interaction_mode: "never".to_owned(),
        chains: plan.tasks.iter().map(|t| chain(t.id.as_str())).collect(),
        effort_policy: ResolvedEffortPolicy {
            small: Effort::Low,
            mid: Effort::High,
            frontier: Effort::Max,
            review: Effort::Medium,
        },
        reviews: ReviewPlan {
            enabled: Some(true),
            alternative_available: Some(true),
            pass_timeout_secs: Some(900),
            primary: Some(PassBinding::new(AGENT, "opus")),
            alternative: Some(PassBinding::new(REVIEW_AGENT, "gpt")),
            second_opinion: vec![
                second_opinion.then(|| PassBinding::new(REVIEW_AGENT, "gpt"));
                plan.tasks.len()
            ],
        },
    };
    let digest = TaskRegistry::originals_with_agents(
        plan,
        &unauthenticated.registry_record(),
        &unauthenticated.probed_agents,
    )
    .expect("the fixture record derives a registry")
    .digest();
    RunStarted4 {
        registry_digest: digest,
        ..unauthenticated
    }
}

pub(super) struct FoldedEmitter {
    log: EventLog,
    fold: TopologyFold,
    hooks: Box<dyn EventHooks>,
    clock: super::seams::SystemClock,
}

impl FoldedEmitter {
    pub(super) fn fold(&self) -> &TopologyFold {
        &self.fold
    }

    pub(super) fn durable_events(&self) -> Vec<TopologyEvent> {
        let bytes = std::fs::read(self.log.path()).expect("read the log back");
        TopologyFold::parse_log(&bytes).expect("the log parses")
    }

    pub(super) fn durable_kinds(&self) -> Vec<&'static str> {
        self.durable_events()
            .iter()
            .map(|event| event.body.kind())
            .collect()
    }

    pub(super) fn task(&self, key: TaskKey) -> &TaskFold {
        self.fold.task(key).expect("the task is registered")
    }

    pub(super) fn generation_class(
        &self,
        key: TaskKey,
        generation: GenerationId,
    ) -> GenerationClass {
        self.task(key)
            .generations
            .iter()
            .find(|held| held.id == generation)
            .unwrap_or_else(|| panic!("task {key} has no generation {}", generation.0))
            .class
            .clone()
    }
}

impl EventEmitter for FoldedEmitter {
    fn emit(
        &mut self,
        body: TopologyEventBody,
        _hooks: &mut dyn super::seams::TopologyHooks,
    ) -> Result<(), crate::engine::topology::emit::EmitFailure> {
        let event = TopologyEvent {
            ts: <super::seams::SystemClock as super::seams::TimeSource>::now_rfc3339(&self.clock),
            body,
        };
        let (line, round_tripped) = TopologyLine::round_trip(&event)?;
        let delta =
            self.fold
                .plan_transition(&round_tripped)
                .map_err(|error| UpstrokeError::Refused {
                    message: format!("the fold refused `{}`: {error}", event.body.kind()),
                })?;
        let site = site_for(&round_tripped.body);
        self.log
            .append_topology_hooked(site, &line, self.hooks.as_mut())?;
        self.fold.apply_delta(delta);
        Ok(())
    }

    fn standing(&self, invocation: &InvocationId) -> super::select::Standing {
        super::select::Standing::of(&self.fold, invocation)
    }
}

#[derive(Clone, Default)]
pub(super) struct Timeline(Arc<Mutex<Vec<(EffectSiteId, HookPhase)>>>);

impl Timeline {
    fn push(&self, site: EffectSiteId, phase: HookPhase) {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((site, phase));
    }

    pub(super) fn positions(&self, site: EffectSiteId, phase: HookPhase) -> Vec<usize> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .enumerate()
            .filter(|(_, seen)| **seen == (site, phase))
            .map(|(index, _)| index)
            .collect()
    }

    pub(super) fn mark(&self) -> usize {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len()
    }
}

struct TimelineEvents {
    inner: crate::events::log::HarnessEventHooks,
    timeline: Timeline,
}

impl EventHooks for TimelineEvents {
    fn phase(&mut self, site: EventSite, phase: HookPhase) {
        self.timeline.push(EffectSiteId::Event(site), phase);
        self.inner.phase(site, phase);
    }

    fn point(&mut self, site: EventSite, point: SubEffectPoint, mode: InjectionMode) -> Injection {
        self.inner.point(site, point, mode)
    }
}

pub(super) struct ArmedEffects {
    inner: HarnessEffects,
    timeline: Timeline,
    armed: Vec<(EffectSiteId, HookPhase, Injection)>,
}

impl ArmedEffects {
    fn new(harness: &Arc<Mutex<HookHarness>>, timeline: &Timeline) -> Self {
        Self {
            inner: HarnessEffects::new(Arc::clone(harness)).recording_durability(),
            timeline: timeline.clone(),
            armed: Vec::new(),
        }
    }

    pub(super) fn arm(&mut self, site: EffectSiteId, phase: HookPhase, injection: Injection) {
        self.armed.push((site, phase, injection));
    }
}

impl EffectHooks for ArmedEffects {
    fn phase(&mut self, site: EffectSiteId, phase: HookPhase) -> Injection {
        self.timeline.push(site, phase);
        let shared = self.inner.phase(site, phase);
        for (armed_site, armed_phase, injection) in &self.armed {
            if *armed_site == site && *armed_phase == phase {
                return crate::observations::Exported::new(Arc::clone(self.inner.harness()))
                    .carried(*injection);
            }
        }
        shared
    }

    fn durability_ledger(&self) -> DurabilityLedger {
        self.inner.durability_ledger()
    }

    fn refusal_cause(&self) -> Option<String> {
        self.inner.refusal_cause()
    }
}

pub(super) struct Hooks {
    effects: ArmedEffects,
    rundir: crate::rundir::HarnessHooks,
    events: crate::events::log::HarnessEventHooks,
    container: crate::runner::container::HarnessHooks,
    spawn: crate::runner::HarnessHooks,
}

impl Hooks {
    fn new(harness: &Arc<Mutex<HookHarness>>, timeline: &Timeline) -> Self {
        Self {
            effects: ArmedEffects::new(harness, timeline),
            rundir: crate::rundir::HarnessHooks::new(Arc::clone(harness)),
            events: crate::events::log::HarnessEventHooks::new(Arc::clone(harness)),
            container: crate::runner::container::HarnessHooks::new(Arc::clone(harness)),
            spawn: crate::runner::HarnessHooks::new(Arc::clone(harness)),
        }
    }

    pub(super) fn arm(&mut self, site: EffectSiteId, phase: HookPhase, injection: Injection) {
        self.effects.arm(site, phase, injection);
    }
}

impl super::seams::TopologyHooks for Hooks {
    fn effects(&mut self) -> &mut dyn EffectHooks {
        &mut self.effects
    }

    fn rundir(&mut self) -> &mut dyn RunDirHooks {
        &mut self.rundir
    }

    fn events(&mut self) -> &mut dyn EventHooks {
        &mut self.events
    }

    fn container(&mut self) -> &mut dyn ContainerHooks {
        &mut self.container
    }

    fn spawn(&mut self) -> &mut dyn SpawnHooks {
        &mut self.spawn
    }
}

#[derive(Debug, Clone)]
pub(super) struct Ran {
    pub(super) invocation: InvocationId,
    pub(super) role: ExecutionRole,
    pub(super) workspace: PathBuf,
    pub(super) agent: Option<AgentId>,
    pub(super) command: CommandSpec,
    pub(super) durable_at_spawn: Vec<String>,
    pub(super) head_at_spawn: Option<String>,
    pub(super) request: RunnerRequest,
    pub(super) policy: RunnerPolicy,
    pub(super) image_id: Option<String>,
}

pub(super) const GATE_DIAGNOSTIC: &str = "scaffold gate rejected the diff";

pub(super) const PASSING_VERDICT: &str =
    "```json\n{\"pass\": true, \"reasons\": [], \"required_changes\": []}\n```";

pub(super) const WORKER_QUESTION: &str = "UPSTROKE-QUESTION: the spec names two incompatible \
                                           formats and I should not pick one alone";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ProbeFailure {
    Exit { code: i32, stderr: String },
    NeverStarted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Ending {
    Completed,
    Failed,
    Cancelled,
    Abandoned,
}

#[derive(Debug)]
struct Held {
    invocation: InvocationId,
    waker: Option<Waker>,
    delivered: Option<Result<ProcessOutput, crate::runner::RunnerError>>,
}

#[derive(Debug, Default)]
struct Control {
    declared: Option<RunnerPolicy>,
    holding: bool,
    failing: Vec<(ProbeTarget, ProbeFailure)>,
    held: Vec<Held>,
    endings: Vec<(InvocationId, Ending)>,
    refused: u32,
}

pub(super) type Responder =
    Box<dyn Fn(&RunnerRequest) -> Result<ProcessOutput, crate::runner::RunnerError> + Send + Sync>;

#[derive(Default)]
pub(super) struct RecordingRunner {
    ran: Mutex<Vec<Ran>>,
    codes: Mutex<Vec<i32>>,
    log: Mutex<Option<PathBuf>>,
    control: Mutex<Control>,
    changed: Condvar,
    respond: Mutex<Option<Responder>>,
}

impl std::fmt::Debug for RecordingRunner {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RecordingRunner")
            .field("ran", &self.ran)
            .field("control", &self.control)
            .finish_non_exhaustive()
    }
}

impl RecordingRunner {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn set_codes(&self, codes: Vec<i32>) {
        *self
            .codes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = codes;
    }

    pub(super) fn failing_with(codes: Vec<i32>) -> Self {
        Self {
            codes: Mutex::new(codes),
            ..Self::default()
        }
    }

    pub(super) fn watching(&self, log: &Path) {
        *self
            .log
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(log.to_path_buf());
    }

    pub(super) fn declaring(self, policy: RunnerPolicy) -> Self {
        self.control().declared = Some(policy);
        self
    }

    pub(super) fn hold(&self) {
        self.control().holding = true;
    }

    pub(super) fn answering(self, respond: Responder) -> Self {
        *self
            .respond
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(respond);
        self
    }

    fn respond_to(
        &self,
        request: &RunnerRequest,
    ) -> Option<Result<ProcessOutput, crate::runner::RunnerError>> {
        self.respond
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .map(|respond| respond(request))
    }

    pub(super) fn await_held(&self, invocation: &InvocationId, within: Duration) -> bool {
        let deadline = std::time::Instant::now() + within;
        let mut control = self.control();
        loop {
            if control
                .held
                .iter()
                .any(|held| held.invocation == *invocation && held.delivered.is_none())
            {
                return true;
            }
            let now = std::time::Instant::now();
            if now >= deadline {
                return false;
            }
            control = self
                .changed
                .wait_timeout(control, deadline - now)
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
        }
    }

    pub(super) fn release(
        &self,
        invocation: &InvocationId,
        within: Duration,
    ) -> Result<(), String> {
        if !self.await_held(invocation, within) {
            return Err(format!("`{invocation}` was not held within {within:?}"));
        }
        let request = self
            .ran()
            .into_iter()
            .rev()
            .find(|ran| ran.invocation == *invocation)
            .map(|ran| ran.request)
            .ok_or_else(|| format!("`{invocation}` is held and was never recorded"))?;
        let result = self
            .respond_to(&request)
            .unwrap_or_else(|| Ok(exited(0, String::new())));
        self.complete(invocation, result)
    }

    pub(super) fn stop_holding(&self) {
        self.control().holding = false;
    }

    pub(super) fn fail_probe(&self, target: ProbeTarget, failure: ProbeFailure) {
        self.control().failing.push((target, failure));
    }

    pub(super) fn waiting(&self) -> Vec<InvocationId> {
        waiting_in(&self.control())
    }

    pub(super) fn await_waiting(&self, count: usize, within: Duration) -> Vec<InvocationId> {
        let deadline = std::time::Instant::now() + within;
        let mut control = self.control();
        loop {
            let waiting = waiting_in(&control);
            let now = std::time::Instant::now();
            if waiting.len() >= count || now >= deadline {
                return waiting;
            }
            control = self
                .changed
                .wait_timeout(control, deadline - now)
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
        }
    }

    pub(super) fn complete(
        &self,
        invocation: &InvocationId,
        result: Result<ProcessOutput, crate::runner::RunnerError>,
    ) -> Result<(), String> {
        let mut control = self.control();
        let Some(held) = control
            .held
            .iter_mut()
            .find(|held| held.invocation == *invocation)
        else {
            control.refused += 1;
            return Err(format!("`{invocation}` is not held by this runner"));
        };
        if held.delivered.is_some() {
            control.refused += 1;
            return Err(format!("`{invocation}` was already completed"));
        }
        let ending = if result.is_ok() {
            Ending::Completed
        } else {
            Ending::Failed
        };
        held.delivered = Some(result);
        if let Some(waker) = held.waker.take() {
            waker.wake();
        }
        control.endings.push((invocation.clone(), ending));
        self.changed.notify_all();
        Ok(())
    }

    pub(super) fn endings(&self) -> Vec<(InvocationId, Ending)> {
        self.control().endings.clone()
    }

    pub(super) fn refused_completions(&self) -> u32 {
        self.control().refused
    }

    fn control(&self) -> std::sync::MutexGuard<'_, Control> {
        self.control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn durable_now(&self) -> Vec<String> {
        let path = self
            .log
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let Some(path) = path else {
            return Vec::new();
        };
        let Ok(bytes) = std::fs::read(&path) else {
            return Vec::new();
        };
        let complete = bytes
            .iter()
            .rposition(|byte| *byte == b'\n')
            .and_then(|end| bytes.get(..=end))
            .unwrap_or_default();
        TopologyFold::parse_log(complete)
            .map(|events| {
                events
                    .iter()
                    .map(|event| event.body.kind().to_owned())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(super) fn ran(&self) -> Vec<Ran> {
        self.ran
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn start(&self, request: &RunnerRequest) -> Started {
        let durable_at_spawn = self.durable_now();
        let head_at_spawn = {
            let output = crate::workspace_manager::fixture::git_out(
                &request.workspace,
                &["rev-parse", "--verify", "--quiet", "HEAD"],
            );
            output
                .status
                .success()
                .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        };
        let mut control = self.control();
        let policy = control
            .declared
            .clone()
            .unwrap_or_else(crate::runner::policy::host_policy);
        let image_id = policy.image.as_ref().map(|image| image.id.clone());
        self.ran
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(Ran {
                invocation: request.invocation.clone(),
                role: request.role.clone(),
                workspace: request.workspace.clone(),
                agent: request.agent.clone(),
                command: request.command.clone(),
                durable_at_spawn,
                head_at_spawn,
                request: request.clone(),
                policy,
                image_id,
            });
        if let ExecutionRole::Probe(target) = &request.role {
            if let Some(index) = control
                .failing
                .iter()
                .position(|(failing, _)| failing == target)
            {
                let (_, failure) = control.failing.remove(index);
                let ending = match failure {
                    ProbeFailure::Exit { .. } => Ending::Completed,
                    ProbeFailure::NeverStarted => Ending::Failed,
                };
                control.endings.push((request.invocation.clone(), ending));
                self.changed.notify_all();
                return Started::Ended(match failure {
                    ProbeFailure::Exit { code, stderr } => Ok(ProcessOutput {
                        code: Some(code),
                        stdout: String::new(),
                        stderr,
                        duration: Duration::from_millis(1),
                        timed_out: false,
                        output_limited: false,
                    }),
                    ProbeFailure::NeverStarted => Err(crate::runner::RunnerError::never_started(
                        &request.invocation,
                        UpstrokeError::Refused {
                            message: format!(
                                "the scaffold runner was told to fail `{}` before it started",
                                request.invocation
                            ),
                        },
                    )),
                });
            }
        }
        if control.holding {
            control.held.push(Held {
                invocation: request.invocation.clone(),
                waker: None,
                delivered: None,
            });
            self.changed.notify_all();
            return Started::Held;
        }
        drop(control);
        if let Some(result) = self.respond_to(request) {
            let ending = if result.is_ok() {
                Ending::Completed
            } else {
                Ending::Failed
            };
            self.control()
                .endings
                .push((request.invocation.clone(), ending));
            self.changed.notify_all();
            return Started::Ended(result);
        }
        self.control()
            .endings
            .push((request.invocation.clone(), Ending::Completed));
        let mut codes = self
            .codes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let code = if codes.is_empty() { 0 } else { codes.remove(0) };
        Started::Ended(Ok(ProcessOutput {
            code: Some(code),
            stdout: if code == 0 {
                String::new()
            } else {
                format!("{GATE_DIAGNOSTIC} (exit {code})\n")
            },
            stderr: String::new(),
            duration: Duration::from_millis(1),
            timed_out: false,
            output_limited: false,
        }))
    }

    fn settle_held(
        &self,
        invocation: &InvocationId,
        cancellation: &Cancellation,
        waker: &Waker,
    ) -> Poll<Result<ProcessOutput, crate::runner::RunnerError>> {
        let mut control = self.control();
        let Some(index) = control
            .held
            .iter()
            .position(|held| held.invocation == *invocation)
        else {
            return Poll::Ready(Err(crate::runner::RunnerError::unresolved(
                invocation,
                UpstrokeError::Refused {
                    message: format!("`{invocation}` vanished from the scaffold runner's hold"),
                },
            )));
        };
        let delivered = control
            .held
            .get_mut(index)
            .and_then(|held| held.delivered.take());
        if let Some(result) = delivered {
            control.held.remove(index);
            self.changed.notify_all();
            return Poll::Ready(result);
        }
        if cancellation.register(waker) {
            control.held.remove(index);
            control
                .endings
                .push((invocation.clone(), Ending::Cancelled));
            self.changed.notify_all();
            return Poll::Ready(Err(crate::runner::RunnerError::cancelled(
                invocation,
                crate::error::ProcessFate::Gone,
            )));
        }
        if let Some(held) = control.held.get_mut(index) {
            held.waker = Some(waker.clone());
        }
        Poll::Pending
    }

    fn abandon(&self, invocation: &InvocationId) {
        let mut control = self.control();
        if let Some(index) = control
            .held
            .iter()
            .position(|held| held.invocation == *invocation)
        {
            let held = control.held.remove(index);
            if held.delivered.is_none() {
                control
                    .endings
                    .push((invocation.clone(), Ending::Abandoned));
            }
            self.changed.notify_all();
        }
    }
}

pub(super) fn exited(code: i32, stdout: String) -> ProcessOutput {
    ProcessOutput {
        code: Some(code),
        stdout,
        stderr: String::new(),
        duration: Duration::from_millis(1),
        timed_out: false,
        output_limited: false,
    }
}

fn waiting_in(control: &Control) -> Vec<InvocationId> {
    control
        .held
        .iter()
        .filter(|held| held.delivered.is_none())
        .map(|held| held.invocation.clone())
        .collect()
}

enum Started {
    Ended(Result<ProcessOutput, crate::runner::RunnerError>),
    Held,
}

struct Invocation<'a> {
    runner: &'a RecordingRunner,
    request: &'a RunnerRequest,
    cancellation: Cancellation,
    held: bool,
    ended: bool,
}

impl Future for Invocation<'_> {
    type Output = Result<ProcessOutput, crate::runner::RunnerError>;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        if !self.held {
            if self.cancellation.is_cancelled() {
                self.ended = true;
                return Poll::Ready(Err(crate::runner::RunnerError::cancelled(
                    &self.request.invocation,
                    crate::error::ProcessFate::NeverStarted,
                )));
            }
            match self.runner.start(self.request) {
                Started::Ended(result) => {
                    self.ended = true;
                    return Poll::Ready(result);
                }
                Started::Held => self.held = true,
            }
        }
        let polled = self.runner.settle_held(
            &self.request.invocation,
            &self.cancellation,
            context.waker(),
        );
        if polled.is_ready() {
            self.ended = true;
        }
        polled
    }
}

impl Drop for Invocation<'_> {
    fn drop(&mut self) {
        if self.held && !self.ended {
            self.runner.abandon(&self.request.invocation);
        }
    }
}

impl Runner for RecordingRunner {
    fn run<'a>(&'a self, request: &'a RunnerRequest, call: RunnerCall<'a>) -> RunFuture<'a> {
        Box::pin(Invocation {
            runner: self,
            request,
            cancellation: call.into_parts().cancellation,
            held: false,
            ended: false,
        })
    }
}

pub(super) struct AnsweringAdapter {
    id: &'static str,
    verdict: &'static str,
    status: crate::ir::OutcomeStatus,
    echo: bool,
}

impl AnsweringAdapter {
    pub(super) const fn erroring(id: &'static str) -> Self {
        Self {
            status: crate::ir::OutcomeStatus::AgentError,
            ..Self::passing(id)
        }
    }

    pub(super) const fn asking(id: &'static str) -> Self {
        Self {
            verdict: "UPSTROKE-QUESTION: the spec names two incompatible \
                      formats and I should not pick one alone",
            ..Self::passing(id)
        }
    }

    pub(super) const fn rate_limited(id: &'static str) -> Self {
        Self {
            status: crate::ir::OutcomeStatus::RateLimited,
            ..Self::passing(id)
        }
    }

    pub(super) const fn passing(id: &'static str) -> Self {
        Self {
            id,
            verdict: PASSING_VERDICT,
            status: crate::ir::OutcomeStatus::Completed,
            echo: false,
        }
    }

    pub(super) const fn echoing(id: &'static str) -> Self {
        Self {
            echo: true,
            ..Self::passing(id)
        }
    }
}

impl crate::agent::AgentAdapter for AnsweringAdapter {
    fn id(&self) -> &'static str {
        self.id
    }

    fn probe(&self, _runner: &dyn Runner) -> Result<crate::agent::Caps, UpstrokeError> {
        panic!("the scaffold's attempts do not pre-flight; `preflight.rs` owns that path")
    }

    fn build(&self, run: &crate::agent::TaskRun) -> Result<CommandSpec, UpstrokeError> {
        let spec = CommandSpec::new(self.id)
            .arg("--prompt")
            .arg(run.prompt.clone());
        match &run.resume_session {
            Some(session) => Ok(spec.arg("--resume").arg(session.clone())),
            None => Ok(spec),
        }
    }

    fn parse(
        &self,
        out: &crate::agent::ProcessOutput,
    ) -> Result<crate::ir::Outcome, UpstrokeError> {
        Ok(crate::ir::Outcome {
            status: self.status,
            diff: String::new(),
            detail: Some(if self.echo {
                out.stdout.clone()
            } else {
                self.verdict.to_owned()
            }),
            session_id: Some(format!("{}-session", self.id)),
            usage: None,
            cost_usd: Some(0.25),
            transcript_path: PathBuf::new(),
            duration: out.duration,
        })
    }

    fn materialize_permissions(
        &self,
        _profile: &crate::ir::WorkerProfile,
        _gate_cmds: &[String],
        _dir: &Path,
        _stem: &str,
    ) -> Result<Option<PathBuf>, UpstrokeError> {
        Ok(None)
    }
}

pub(super) struct ScaffoldAdapters {
    primary: AnsweringAdapter,
    second: AnsweringAdapter,
}

impl ScaffoldAdapters {
    pub(super) const fn erroring() -> Self {
        Self {
            primary: AnsweringAdapter::erroring(AGENT),
            second: AnsweringAdapter::passing(REVIEW_AGENT),
        }
    }

    pub(super) const fn asking() -> Self {
        Self {
            primary: AnsweringAdapter::asking(AGENT),
            second: AnsweringAdapter::passing(REVIEW_AGENT),
        }
    }

    pub(super) const fn rate_limiting() -> Self {
        Self {
            primary: AnsweringAdapter::rate_limited(AGENT),
            second: AnsweringAdapter::passing(REVIEW_AGENT),
        }
    }

    pub(super) const fn new() -> Self {
        Self {
            primary: AnsweringAdapter::passing(AGENT),
            second: AnsweringAdapter::passing(REVIEW_AGENT),
        }
    }

    pub(super) const fn echoing() -> Self {
        Self {
            primary: AnsweringAdapter::echoing(AGENT),
            second: AnsweringAdapter::echoing(REVIEW_AGENT),
        }
    }
}

impl crate::agent::AdapterSource for ScaffoldAdapters {
    fn get(&self, id: &str) -> Option<&dyn crate::agent::AgentAdapter> {
        if id == AGENT {
            Some(&self.primary)
        } else if id == REVIEW_AGENT {
            Some(&self.second)
        } else {
            None
        }
    }
}

fn scaffold_run_paths(fixture: &Fixture) -> crate::rundir::RunPaths {
    let paths = crate::rundir::RunPaths::with_private_root(
        &fixture.base,
        "01SCAFFOLD00000000000000AA",
        &fixture.root.join("home"),
    );
    paths.create().expect("the scaffold's run directories");
    paths
}

pub(super) struct Run {
    pub(super) paths: crate::rundir::RunPaths,
    pub(super) harness: Arc<Mutex<HookHarness>>,
    pub(super) hooks: Hooks,
    pub(super) timeline: Timeline,
    pub(super) emitter: FoldedEmitter,
    pub(super) runner: RecordingRunner,
    pub(super) invocations: crate::engine::topology::identity::InvocationLedger,
    pub(super) reservations: crate::engine::topology::identity::Reservations,
    pub(super) verify_gates: Vec<super::attempt::GatePlan>,
    pub(super) verify_reviewers: Vec<super::attempt::ReviewerPlan>,
    pub(super) verify_review: VerifyReview,
    pub(super) ids_source: super::seams::RealIds,
    pub(super) fixture: Fixture,
}

impl super::integrate::IntegrationJournal for Run {
    fn emit(&mut self, body: TopologyEventBody) -> Result<(), UpstrokeError> {
        self.emitter
            .emit(body, &mut self.hooks)
            .map_err(|failure| failure.discharging(&mut self.invocations))
    }

    fn fold(&self) -> &TopologyFold {
        self.emitter.fold()
    }

    fn hooks(&mut self) -> &mut dyn super::seams::TopologyHooks {
        &mut self.hooks
    }

    fn converted(&mut self, key: TaskKey) -> Result<(), UpstrokeError> {
        self.reservations.convert(
            key,
            crate::engine::topology::identity::ReservationKind::Integration,
        )
    }
}

#[derive(Debug, Clone)]
pub(super) enum VerifyReview {
    Passed,
    NeedsChanges,
    NeedsHuman,
    Unavailable(crate::ir::OutcomeStatus),
}

struct ScaffoldReviews {
    outcome: VerifyReview,
}

impl super::attempt::ReviewPasses for ScaffoldReviews {
    fn run(
        &self,
        cx: &crate::review::ReviewCx<'_>,
        runner: &dyn Runner,
        invocations: &crate::review::ReviewInvocations,
    ) -> Result<crate::review::ReviewOutcome, UpstrokeError> {
        let request = crate::runner::review_request(
            CommandSpec::new(cx.adapter.id()).arg("--review"),
            cx.workspace.to_path_buf(),
            AgentId::new(cx.adapter.id()),
            cx.timeout,
            invocations.pass.clone(),
        );
        if let Err(error) = runner.run_blocking(&request) {
            if error.is_cancelled() || error.fate.is_unresolved() {
                return Err(error.into());
            }
            let never_started = matches!(error.fate, crate::error::ProcessFate::NeverStarted);
            return Ok(crate::review::ReviewOutcome {
                result: crate::review::ReviewResult::Unavailable {
                    status: crate::ir::OutcomeStatus::AgentError,
                    detail: format!("review process failed: {error}"),
                },
                cost_usd: None,
                invocations: 0,
                transcript: PathBuf::new(),
                never_started,
            });
        }
        let result = match &self.outcome {
            VerifyReview::Passed => crate::review::ReviewResult::Judged(crate::ir::Verdict {
                pass: true,
                reasons: Vec::new(),
                required_changes: Vec::new(),
                needs_human: false,
            }),
            VerifyReview::NeedsChanges => crate::review::ReviewResult::Judged(crate::ir::Verdict {
                pass: false,
                reasons: vec!["the proposed tree regresses merged behaviour".to_owned()],
                required_changes: vec!["restore it".to_owned()],
                needs_human: false,
            }),
            VerifyReview::NeedsHuman => crate::review::ReviewResult::Judged(crate::ir::Verdict {
                pass: false,
                reasons: vec!["a person must decide this integration".to_owned()],
                required_changes: Vec::new(),
                needs_human: true,
            }),
            VerifyReview::Unavailable(status) => crate::review::ReviewResult::Unavailable {
                status: *status,
                detail: "the integration reviewer was unavailable".to_owned(),
            },
        };
        Ok(crate::review::ReviewOutcome {
            result,
            cost_usd: Some(0.2),
            invocations: 1,
            transcript: PathBuf::new(),
            never_started: false,
        })
    }
}

impl super::integrate::Verification for Run {
    fn verify(
        &mut self,
        request: &super::integrate::VerifyRequest<'_>,
    ) -> Result<super::integrate::Verified, UpstrokeError> {
        use super::attempt::{
            Judge, JudgeIdentities, JudgeNames, SnapshotDisposal, SnapshotOf, Subject,
        };
        let manager = self.fixture.manager.clone();
        let parent = if request.already_present {
            self.base().0
        } else {
            request.head.0.clone()
        };
        let tree = if request.already_present {
            request.candidate.commit_sha.0.clone()
        } else {
            request.proposed.0.clone()
        };
        let diff = manager.candidate_diff(request.staging, &parent, &tree)?;
        let inputs = super::attempt::ReviewInputs {
            title: "integration".to_owned(),
            body: String::new(),
            acceptance: vec!["it integrates".to_owned()],
            diff,
            artifacts: Vec::new(),
            decisions: Vec::new(),
            stem: format!("s{}", request.sequence.0),
        };
        let gates = self.verify_gates.clone();
        let reviewers = self.verify_reviewers.clone();
        let reviews = ScaffoldReviews {
            outcome: self.verify_review.clone(),
        };
        let adapters = ScaffoldAdapters::new();
        let proposed = crate::workspace_manager::ObjectId::new(request.proposed.0.clone())
            .expect("the proposed commit is an object id");
        let identities = super::identity::SequenceIdentities::new(request.sequence);
        let mut judge = Judge {
            manager: &manager,
            hooks: &mut self.hooks,
            runner: &self.runner,
            standing: super::select::Standing::of(self.emitter.fold(), &identities.gate(0, 0)),
            registrar: &std::sync::Mutex::new(&mut self.invocations),
            carried: &super::preflight::Carried::default(),
            adapters: &adapters,
            paths: &self.paths,
            reviews: &reviews,
        };
        match judge.judge(
            &Subject {
                snapshot: SnapshotOf::Commit(proposed),
                disposal: SnapshotDisposal::AfterTheTerminal,
                names: JudgeNames::Integration {
                    sequence: u64::from(request.sequence.0),
                },
                identities: JudgeIdentities::Sequence(identities),
                stem: format!("integration-s{}", request.sequence.0),
                gates: &gates,
                reviewers: &reviewers,
                inputs: &inputs,
                prior_failure: None,
                invocations: &move |pass| crate::review::ReviewInvocations {
                    pass: identities.review_pass(pass, 0),
                    reask: identities.review_reask(pass, 0),
                },
            },
            &mut super::attempt::NoReviewAccount,
        ) {
            Ok(judgement) => Ok(super::integrate::Verified::Judged(judgement)),
            Err(super::attempt::JudgeError::Runner(error)) => {
                Ok(super::integrate::Verified::Unavailable {
                    kind: crate::topology::events::InfrastructureKind::RunnerSpawnFailure,
                    detail: error.to_string(),
                    reviews: Vec::new(),
                })
            }
            Err(super::attempt::JudgeError::Other(error)) => Err(error),
        }
    }

    fn ids(&self) -> &dyn super::seams::IdSource {
        &self.ids_source
    }
}

impl Run {
    pub(super) fn started(tag: &str) -> Self {
        let mut run = Self::bare(tag);
        let started = run_started(&run.fixture);
        Self::begin(&mut run, started);
        run
    }

    fn bare(tag: &str) -> Self {
        let fixture = Fixture::created(tag);
        let harness = Arc::new(Mutex::new(HookHarness::new()));
        let timeline = Timeline::default();
        let log = EventLog::open(
            EventSite::OpenLog,
            &fixture.private.join("events.jsonl"),
            &mut Vec::new(),
        )
        .expect("open the schema-4 log");
        Self {
            emitter: FoldedEmitter {
                log,
                fold: TopologyFold::new(FrozenInputs {
                    plan: plan(),
                    normalized_plan_digest: NORMALIZED_DIGEST.to_owned(),
                }),
                hooks: Box::new(TimelineEvents {
                    inner: crate::events::log::HarnessEventHooks::new(Arc::clone(&harness)),
                    timeline: timeline.clone(),
                }),
                clock: super::seams::SystemClock,
            },
            hooks: Hooks::new(&harness, &timeline),
            invocations: crate::engine::topology::identity::InvocationLedger::new(),
            reservations: crate::engine::topology::identity::Reservations::new(),
            runner: RecordingRunner::new(),
            verify_gates: Vec::new(),
            verify_reviewers: Vec::new(),
            verify_review: VerifyReview::Passed,
            ids_source: super::seams::RealIds,
            timeline,
            harness,
            paths: scaffold_run_paths(&fixture),
            fixture,
        }
    }

    fn begin(run: &mut Self, started: RunStarted4) {
        let integration_ref = started.integration_ref.clone();
        run.emitter
            .emit(
                TopologyEventBody::RunStarted {
                    data: Box::new(started),
                },
                &mut run.hooks,
            )
            .expect("run_started");
        run.fixture
            .manager
            .create_ref_zero_old(
                run.hooks.effects(),
                crate::topology::effects::RefSite::CreateIntegration,
                integration_ref.as_str(),
                &run.fixture.head,
            )
            .expect("the integration ref");
        run.runner.watching(run.emitter.log.path());
    }

    pub(super) fn started_with_max_defers(tag: &str, max_defers: u32) -> Self {
        let mut run = Self::bare(tag);
        let mut started = run_started(&run.fixture);
        started.limits.max_defers = max_defers;
        Self::begin(&mut run, started);
        run
    }

    pub(super) fn wake_deferred(&mut self) {
        self.emitter
            .emit(
                TopologyEventBody::DeferWaitElapsed {
                    data: crate::topology::events::DeferWaitElapsed4 {
                        waited_ms: 1,
                        round: 1,
                    },
                },
                &mut self.hooks,
            )
            .expect("defer_wait_elapsed");
    }

    pub(super) fn manager(&self) -> &WorkspaceManager {
        &self.fixture.manager
    }

    pub(super) fn base(&self) -> CommitSha {
        CommitSha(self.fixture.head.clone())
    }

    pub(super) fn predicted(&self, key: TaskKey) -> PathSet {
        self.emitter
            .fold()
            .predicted_region(key)
            .expect("the scaffold's run has started, so its registry answers")
    }

    pub(super) fn observed(&self, site: EffectSiteId, phase: HookPhase) -> bool {
        self.harness
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .observed(site, phase)
    }

    pub(super) fn order_of(&self, site: EffectSiteId, phase: HookPhase) -> Option<usize> {
        self.timeline.positions(site, phase).first().copied()
    }

    pub(super) fn must_order_of(&self, site: EffectSiteId, phase: HookPhase) -> usize {
        self.order_of(site, phase)
            .unwrap_or_else(|| panic!("nothing drove `{site}` at its `{phase}` phase"))
    }

    pub(super) fn mark(&self) -> usize {
        self.timeline.mark()
    }

    pub(super) fn order_after(&self, mark: usize, site: EffectSiteId, phase: HookPhase) -> usize {
        self.timeline
            .positions(site, phase)
            .into_iter()
            .find(|position| *position >= mark)
            .unwrap_or_else(|| {
                panic!("nothing drove `{site}` at its `{phase}` phase after position {mark}")
            })
    }

    pub(super) fn count_after(&self, mark: usize, site: EffectSiteId, phase: HookPhase) -> usize {
        self.timeline
            .positions(site, phase)
            .into_iter()
            .filter(|position| *position >= mark)
            .count()
    }

    pub(super) fn arm(&mut self, site: EffectSiteId, phase: HookPhase, injection: Injection) {
        self.hooks.arm(site, phase, injection);
    }

    pub(super) fn arm_point(
        &mut self,
        site: EffectSiteId,
        point: SubEffectPoint,
        mode: InjectionMode,
    ) {
        self.harness
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .arm(site, point, mode)
            .expect("the site exposes this point in this mode");
    }

    pub(super) fn task_state(&self, key: TaskKey) -> TaskState {
        self.emitter.task(key).state
    }
}

impl Run {
    pub(super) fn adopt(root: PathBuf) -> Self {
        let fixture = Fixture::adopt(root);
        let harness = Arc::new(Mutex::new(HookHarness::new()));
        let timeline = Timeline::default();
        let log_path = fixture.private.join("events.jsonl");
        let bytes = std::fs::read(&log_path).expect("the child's log survives it");
        let events = TopologyFold::parse_log(&bytes).expect("the child's log parses");
        let fold = TopologyFold::replay(
            FrozenInputs {
                plan: plan(),
                normalized_plan_digest: NORMALIZED_DIGEST.to_owned(),
            },
            &events,
        )
        .expect("the child's log replays");
        let log = EventLog::open(EventSite::OpenLog, &log_path, &mut Vec::new())
            .expect("reopen the schema-4 log");
        let adopted = Self {
            emitter: FoldedEmitter {
                log,
                fold,
                hooks: Box::new(TimelineEvents {
                    inner: crate::events::log::HarnessEventHooks::new(Arc::clone(&harness)),
                    timeline: timeline.clone(),
                }),
                clock: super::seams::SystemClock,
            },
            hooks: Hooks::new(&harness, &timeline),
            runner: RecordingRunner::new(),
            invocations: crate::engine::topology::identity::InvocationLedger::new(),
            reservations: crate::engine::topology::identity::Reservations::new(),
            verify_gates: Vec::new(),
            verify_reviewers: Vec::new(),
            verify_review: VerifyReview::Passed,
            ids_source: super::seams::RealIds,
            timeline,
            harness,
            paths: scaffold_run_paths(&fixture),
            fixture,
        };
        adopted.runner.watching(adopted.emitter.log.path());
        adopted
    }

    pub(super) fn hand_off(&self, dir: &Path) {
        write_file(
            &dir.join(HANDOFF),
            self.fixture.root.to_string_lossy().as_bytes(),
        );
    }

    pub(super) fn dispatch(&mut self, key: TaskKey, generation: u32) -> Dispatched {
        self.try_dispatch(key, generation).expect("dispatch")
    }

    pub(super) fn try_dispatch(
        &mut self,
        key: TaskKey,
        generation: u32,
    ) -> Result<Dispatched, UpstrokeError> {
        let request = DispatchRequest {
            key,
            generation: GenerationId(generation),
            base: self.base(),
            kind: DispatchKind::Ordinary {
                paths: self.predicted(key),
            },
        };
        dispatch(
            &self.fixture.manager,
            &mut self.hooks,
            &mut self.emitter,
            &request,
        )
        .map_err(|failure| failure.discharging(&mut self.invocations))
    }

    pub(super) fn spawn_repair(&mut self, root: TaskKey) -> TaskKey {
        let registry = self
            .emitter
            .fold()
            .registry()
            .expect("the run has a registry");
        let key = TaskKey(u32::try_from(registry.len()).expect("a small fixture registry"));
        let parent = registry.get(root).expect("the root is registered").clone();
        let mut entry = parent.clone();
        entry.key = key;
        entry.display_id =
            crate::ir::TaskId::from(repair_display_id(1, &parent.display_id).as_str());
        entry.origin = Origin::MergeRepair;
        entry.deps = Vec::new();
        entry.display_deps = Vec::new();
        entry.lineage = Some(Lineage {
            root,
            parent: root,
            index: 1,
        });
        self.emitter
            .emit(
                TopologyEventBody::TaskSpawned {
                    data: Box::new(TaskSpawned {
                        spawn: FrozenSpawn {
                            key,
                            entry,
                            admission: SpawnAdmission::Runnable,
                        },
                    }),
                },
                &mut self.hooks,
            )
            .expect("task_spawned");
        key
    }
}

impl Run {
    pub(super) fn commit_with(
        &self,
        parent: &str,
        file: &str,
        content: &str,
        message: &str,
    ) -> String {
        let repo = &self.fixture.base;
        let scratch = self.fixture.root.join(format!("scratch-{message}"));
        write_file(&scratch, content.as_bytes());
        let blob = crate::workspace_manager::fixture::git(
            repo,
            &[
                "hash-object",
                "-w",
                scratch.to_str().expect("a utf-8 scratch path"),
            ],
        );
        crate::workspace_manager::fixture::git(repo, &["read-tree", parent]);
        crate::workspace_manager::fixture::git(
            repo,
            &[
                "update-index",
                "--add",
                "--cacheinfo",
                &format!("100644,{blob},{file}"),
            ],
        );
        let tree = crate::workspace_manager::fixture::git(repo, &["write-tree"]);
        let commit = crate::workspace_manager::fixture::git(
            repo,
            &["commit-tree", &tree, "-p", parent, "-m", message],
        );
        crate::workspace_manager::fixture::git(repo, &["read-tree", "HEAD"]);
        commit
    }

    /// One commit on `parent` changing several paths at once: `Some(content)`
    /// writes the path, `None` removes it. A cherry-pick applies one commit's
    /// change against that commit's parent, so a candidate that has to
    /// conflict in several files must carry every one of those changes
    /// itself; a chain of one-file commits would pick only its tip.
    pub(super) fn commit_changing(
        &self,
        parent: &str,
        changes: &[(&str, Option<&str>)],
        message: &str,
    ) -> String {
        use crate::workspace_manager::fixture::git;

        let repo = &self.fixture.base;
        git(repo, &["read-tree", parent]);
        for (index, (file, content)) in changes.iter().enumerate() {
            match content {
                Some(content) => {
                    let scratch = self.fixture.root.join(format!("scratch-{message}-{index}"));
                    write_file(&scratch, content.as_bytes());
                    let blob = git(
                        repo,
                        &[
                            "hash-object",
                            "-w",
                            scratch.to_str().expect("a utf-8 scratch path"),
                        ],
                    );
                    git(
                        repo,
                        &[
                            "update-index",
                            "--add",
                            "--cacheinfo",
                            &format!("100644,{blob},{file}"),
                        ],
                    );
                }
                None => {
                    git(repo, &["update-index", "--force-remove", "--", file]);
                }
            }
        }
        let tree = git(repo, &["write-tree"]);
        let commit = git(repo, &["commit-tree", &tree, "-p", parent, "-m", message]);
        git(repo, &["read-tree", "HEAD"]);
        commit
    }

    pub(super) fn protect_candidate(
        &mut self,
        commit: &str,
    ) -> crate::topology::events::CandidateRef {
        let refname = "refs/upstroke/runs/run-1/candidates/k0/0".to_owned();
        self.fixture
            .manager
            .create_ref_zero_old(
                self.hooks.effects(),
                crate::topology::effects::RefSite::CreateCandidates,
                &refname,
                commit,
            )
            .expect("the authoritative candidates ref");
        crate::topology::events::CandidateRef {
            key: ALPHA,
            generation: GenerationId(0),
            commit_sha: CommitSha(commit.to_owned()),
            candidate_ref: GitRef(refname),
        }
    }
}

impl super::candidate::CandidateJournal for Run {
    fn emit(&mut self, body: TopologyEventBody) -> Result<(), UpstrokeError> {
        self.emitter
            .emit(body, &mut self.hooks)
            .map_err(|failure| failure.discharging(&mut self.invocations))
    }

    fn fold(&self) -> &TopologyFold {
        self.emitter.fold()
    }
}

impl Run {
    pub(super) fn queue_candidate(
        &mut self,
        key: TaskKey,
    ) -> crate::topology::events::CandidateRef {
        let path = format!("{key}-work.txt");
        let content = format!("work of task {key}\n");
        self.queue_candidate_editing(key, &path, &content)
    }

    pub(super) fn queue_candidate_editing(
        &mut self,
        key: TaskKey,
        path: &str,
        content: &str,
    ) -> crate::topology::events::CandidateRef {
        use super::candidate::{
            JudgedTree, append_candidate_created, append_candidate_prepared, create_candidates_ref,
            pin_candidate, reclaim_after_creation, write_candidate_commit,
        };

        let generation =
            u32::try_from(self.emitter.task(key).generations.len()).expect("a small fixture");
        let dispatched = self.dispatch(key, generation);
        let binding = self.binding(key, 0);
        self.emitter
            .emit(
                TopologyEventBody::AttemptStarted {
                    data: crate::topology::events::AttemptStarted4 {
                        key,
                        generation: dispatched.generation,
                        attempt: AttemptNumber(1),
                        rung: 0,
                        binding,
                        pool: Some("scaffold-pool".to_owned()),
                        resume_session: None,
                        materialization_observed: None,
                    },
                },
                &mut self.hooks,
            )
            .expect("attempt_started");
        write_file(&dispatched.worktree.join(path), content.as_bytes());
        let manager = self.fixture.manager.clone();
        manager
            .candidate_stage(self.hooks.effects(), &dispatched.slot, &[])
            .expect("stage");
        let tree = manager
            .candidate_write_tree(self.hooks.effects(), &dispatched.slot)
            .expect("write-tree");
        let actual_paths = manager
            .changed_paths(&dispatched.slot, dispatched.base.as_str())
            .expect("changed paths");
        let judged = JudgedTree {
            key,
            generation: dispatched.generation,
            attempt: Box::new(AttemptRecord {
                attempt: 1,
                tier: "mid".to_owned(),
                model: format!("{}-mid-model", self.display_id(key)),
                pool: Some("scaffold-pool".to_owned()),
                resumed: false,
                duration: Duration::from_millis(5),
                cost_usd: Some(0.5),
                reviews: self
                    .emitter
                    .fold()
                    .registry()
                    .expect("a registry")
                    .get(key)
                    .expect("the task is registered")
                    .reviews
                    .obliged_lenses()
                    .into_iter()
                    .map(|lens| crate::events::ReviewRecord {
                        pass: lens.name().to_owned(),
                        agent: REVIEW_AGENT.to_owned(),
                        model: "scaffold-review-model".to_owned(),
                        adapter: None,
                        preflight_cli_version: None,
                        effort: None,
                        pool: None,
                        cost_usd: Some(0.1),
                        outcome: crate::events::ReviewPassOutcome::Passed,
                    })
                    .collect(),
                session_id: None,
                usage: None,
                failure: None,
            }),
            base_sha: dispatched.base.clone(),
            tree_sha: CommitSha(tree),
            message: format!("upstroke: {} attempt 1", self.display_id(key)),
            actual_paths: actual_paths.clone(),
            lease_effect: crate::topology::events::CandidateLeaseEffect::ReplacesPredicted {
                paths: actual_paths,
            },
        };
        let run_id = self
            .emitter
            .fold()
            .started()
            .expect("started")
            .run_id
            .clone();
        let unpinned =
            write_candidate_commit(&manager, &mut self.hooks, &run_id, judged).expect("commit");
        let pinned = pin_candidate(&manager, &mut self.hooks, unpinned).expect("pin");
        let promoting = append_candidate_prepared(self, pinned).expect("candidate_prepared");
        let candidate = promoting.candidate().clone();
        let referenced =
            create_candidates_ref(&manager, &mut self.hooks, promoting).expect("candidates ref");
        let created = append_candidate_created(self, referenced).expect("task_candidate_created");
        reclaim_after_creation(&manager, &mut self.hooks, &dispatched.slot, created)
            .expect("scrub");
        candidate
    }

    pub(super) fn display_id(&self, key: TaskKey) -> String {
        self.emitter
            .fold()
            .registry()
            .expect("a registry")
            .get(key)
            .expect("the task is registered")
            .display_id
            .as_str()
            .to_owned()
    }

    pub(super) fn integration_ref(&self) -> GitRef {
        self.emitter
            .fold()
            .started()
            .expect("started")
            .integration_ref
            .clone()
    }

    pub(super) fn head(&self) -> Option<String> {
        self.fixture
            .manager
            .direct_ref_target(self.integration_ref().as_str())
            .expect("read the integration ref")
    }

    pub(super) fn replay_twice_equal(&self) {
        let events = self.emitter.durable_events();
        let inputs = FrozenInputs {
            plan: plan(),
            normalized_plan_digest: NORMALIZED_DIGEST.to_owned(),
        };
        let first = TopologyFold::replay(inputs.clone(), &events).expect("the log replays");
        let second = TopologyFold::replay(inputs, &events).expect("the log replays again");
        assert_eq!(first.state(), second.state(), "two replays disagree");
        assert_eq!(
            self.emitter.fold().state(),
            first.state(),
            "the live fold and a replay of its own log disagree"
        );
    }
}

pub(super) const RETAINED_SESSION: &str = "session-01SCAFFOLD";

impl Run {
    pub(super) fn binding(&self, key: TaskKey, rung: u32) -> RungBinding {
        let entry = self
            .emitter
            .fold()
            .registry()
            .expect("a registry")
            .get(key)
            .expect("the task is registered");
        let frozen = entry
            .ladder
            .rungs
            .get(rung as usize)
            .expect("the ladder has this rung");
        let effort = entry.ladder.effort.implementation_for(frozen.tier);
        RungBinding::from_frozen(frozen, effort)
    }

    pub(super) fn attempt_plan(&self, key: TaskKey, attempt: u32) -> AttemptPlan {
        AttemptPlan {
            attempt: AttemptNumber(attempt),
            rung: 0,
            binding: self.binding(key, 0),
            pool: Some("scaffold-pool".to_owned()),
            resume_session: None,
            materialization_observed: None,
            agent: AgentId::new(AGENT),
            session_resume: true,
            worker: CommandSpec::new("worker").arg("--implement"),
            worker_timeout: Duration::from_secs(300),
            gates: vec![{
                let (command, timeout) = crate::gates::ShellGate {
                    name: "scaffold".to_owned(),
                    cmd: "gate --check".to_owned(),
                    timeout: Duration::from_secs(60),
                    shell: crate::gates::ShellKind::native(),
                }
                .command();
                GatePlan {
                    name: "scaffold".to_owned(),
                    command,
                    timeout,
                }
            }],
            reviewers: vec![
                ReviewerPlan {
                    agent: AgentId::new(AGENT),
                    profile: crate::review::profile_for(
                        AGENT,
                        "scaffold-model",
                        "primary",
                        Effort::High,
                    ),
                    lens: crate::review::Lens::Acceptance,
                    preflight_cli_version: Some("scaffold-cli/1".to_owned()),
                    timeout: Duration::from_secs(120),
                },
                ReviewerPlan {
                    agent: AgentId::new(REVIEW_AGENT),
                    profile: crate::review::profile_for(
                        REVIEW_AGENT,
                        "scaffold-second-model",
                        "second_opinion",
                        Effort::High,
                    ),
                    lens: crate::review::Lens::SecondOpinion,
                    preflight_cli_version: None,
                    timeout: Duration::from_secs(120),
                },
            ],
        }
    }

    pub(super) fn review_inputs(&self) -> super::attempt::ReviewInputs {
        super::attempt::ReviewInputs {
            title: "scaffold task".to_owned(),
            body: String::new(),
            acceptance: vec!["it works".to_owned()],
            diff: "diff --git a/a b/a\n".to_owned(),
            artifacts: Vec::new(),
            decisions: Vec::new(),
            stem: "scaffold".to_owned(),
        }
    }

    pub(super) fn retain(&mut self, key: TaskKey, generation: GenerationId, attempt: u32) {
        self.emitter
            .emit(
                TopologyEventBody::AttemptFinished {
                    data: Box::new(AttemptFinished4 {
                        key,
                        generation,
                        attempt: AttemptNumber(attempt),
                        record: Box::new(AttemptRecord {
                            attempt,
                            tier: "mid".to_owned(),
                            model: "alpha-mid-model".to_owned(),
                            pool: Some("scaffold-pool".to_owned()),
                            resumed: false,
                            duration: Duration::from_millis(7),
                            cost_usd: None,
                            reviews: Vec::new(),
                            session_id: Some(RETAINED_SESSION.to_owned()),
                            usage: None,
                            failure: Some(crate::events::FailureRecord {
                                kind: crate::ladder::FailureKind::GateFailed,
                                origin: crate::ladder::FailureOrigin::Worker,
                                reason: "the scaffold's judged failure".to_owned(),
                                detail: None,
                            }),
                        }),
                        settlement: AttemptSettlement::Retained {
                            retained_session: SessionId(RETAINED_SESSION.to_owned()),
                            retained_incarnation: Epoch(0),
                        },
                    }),
                },
                &mut self.hooks,
            )
            .expect("attempt_finished(retained)");
    }
}

const HANDOFF: &str = "fixture-root";

pub(super) const KILL_CHILD_BOUND: Duration = Duration::from_secs(120);

/// The handoff directory a kill child is pointed at, guarded.
///
/// It was `temp_dir()/upstroke-topo-<tag>-<pid>-<ordinal>`, created by the
/// parent and removed by nobody (`PR7-SCRATCH-FIXTURE-LEAK`, measured at 8
/// surviving directories per green suite run). This is the **parent's**
/// directory and the parent returns normally, so its guard runs; the child's
/// own fixture root is adopted and reclaimed separately
/// (`workspace_manager::fixture::Fixture::adopt`).
pub(super) fn kill_dir(tag: &str) -> crate::rundir::scratch_tree::ScratchTree {
    let parent = std::env::temp_dir();
    match crate::rundir::scratch_tree::acquire(&parent, tag) {
        Ok(tree) => tree,
        Err(refusal) => panic!(
            "a handoff directory for `{tag}` under {}: {refusal:?}",
            parent.display()
        ),
    }
}

pub(super) fn kill_child_and_adopt(test: &str, dir: &Path, site: &str) -> Run {
    launch_the_kill_child_and_adopt(test, dir, site, &[])
}

pub(super) fn kill_child_and_adopt_in_a_scratch_tree(
    test: &str,
    site: &str,
) -> (crate::rundir::scratch_tree::ScratchTree, Run) {
    let tree = crate::rundir::scratch_tree::acquire(&std::env::temp_dir(), "kill").unwrap_or_else(
        |refusal| panic!("`{site}`: a scratch tree for the kill child: {refusal:?}"),
    );
    let temporary = tree.path().as_os_str();
    let run = launch_the_kill_child_and_adopt(
        test,
        tree.path(),
        site,
        &[
            ("TMPDIR", temporary),
            ("TMP", temporary),
            ("TEMP", temporary),
        ],
    );
    (tree, run)
}

fn launch_the_kill_child_and_adopt(
    test: &str,
    dir: &Path,
    site: &str,
    temporary: &[(&str, &std::ffi::OsStr)],
) -> Run {
    let mut env = vec![
        ("UPSTROKE_TEST_KILL_DIR", dir.as_os_str()),
        ("UPSTROKE_TEST_KILL_SITE", std::ffi::OsStr::new(site)),
    ];
    env.extend_from_slice(temporary);
    let Some(status) = run_kill_child_within(test, &env, KILL_CHILD_BOUND) else {
        panic!(
            "`{site}`: the kill child `{test}` did not end within {KILL_CHILD_BOUND:?}, and was \
             killed and reaped"
        );
    };
    assert!(
        died_by_abort(&status),
        "`{site}`: the child must have died by `std::process::abort()`, and it ended {status:?} \
         — a child that reached its own `unreachable!` panics instead, which means the injection \
         stopped killing"
    );
    let root = std::fs::read_to_string(dir.join(HANDOFF))
        .unwrap_or_else(|error| panic!("`{site}`: the child left no handoff: {error}"));
    Run::adopt(PathBuf::from(root))
}

pub(super) fn kill_child_environment() -> (PathBuf, String) {
    (
        PathBuf::from(std::env::var("UPSTROKE_TEST_KILL_DIR").expect("UPSTROKE_TEST_KILL_DIR")),
        std::env::var("UPSTROKE_TEST_KILL_SITE").expect("UPSTROKE_TEST_KILL_SITE"),
    )
}

pub(super) const OUTCOME: RunOutcome = RunOutcome::Complete;

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct NoSleep;

impl crate::interaction::Sleeper for NoSleep {
    fn sleep(&self, _duration: Duration) {}
}

#[derive(Debug, Clone)]
pub(super) struct WideTask {
    pub(super) id: &'static str,
    pub(super) hints: Vec<String>,
    pub(super) depends_on: Vec<&'static str>,
    pub(super) writes: String,
}

impl WideTask {
    pub(super) fn independent(id: &'static str) -> Self {
        Self {
            id,
            hints: vec![format!("src/{id}/")],
            depends_on: Vec::new(),
            writes: format!("src/{id}/work.txt"),
        }
    }

    pub(super) fn after(id: &'static str, depends_on: &[&'static str]) -> Self {
        Self {
            depends_on: depends_on.to_vec(),
            ..Self::independent(id)
        }
    }

    pub(super) fn hinted(id: &'static str, hints: &[&str], writes: &str) -> Self {
        Self {
            id,
            hints: hints.iter().map(|hint| (*hint).to_owned()).collect(),
            depends_on: Vec::new(),
            writes: writes.to_owned(),
        }
    }
}

pub(super) fn wide_plan(tasks: &[WideTask]) -> Plan {
    Plan {
        source: PlanSource {
            adapter: "markdown".to_owned(),
            hash: "scaffold-plan-hash".to_owned(),
        },
        tasks: tasks
            .iter()
            .map(|task| Task {
                id: TaskId::from(task.id),
                kind: TaskKind::Refactor,
                title: format!("{} title", task.id),
                body: format!("{} body", task.id),
                depends_on: task.depends_on.iter().map(|id| TaskId::from(*id)).collect(),
                acceptance: vec![format!("{} passes", task.id)],
                path_hints: task.hints.clone(),
                suggested_tier: None,
                min_tier: None,
                artifacts_in: Vec::new(),
                artifacts_out: Vec::new(),
            })
            .collect(),
        artifacts: Vec::new(),
    }
}

pub(super) struct WidePlans {
    pub(super) gates: usize,
    pub(super) reviewers: usize,
    pub(super) verify_gates: usize,
    pub(super) verify_reviewers: usize,
    pub(super) pool: Option<String>,
}

impl WidePlans {
    fn gate_plans(count: usize) -> Vec<GatePlan> {
        (0..count)
            .map(|index| {
                let (command, timeout) = crate::gates::ShellGate {
                    name: format!("scaffold-{index}"),
                    cmd: "gate --check".to_owned(),
                    timeout: Duration::from_secs(60),
                    shell: crate::gates::ShellKind::native(),
                }
                .command();
                GatePlan {
                    name: format!("scaffold-{index}"),
                    command,
                    timeout,
                }
            })
            .collect()
    }

    fn reviewer_plans(&self, count: usize) -> Vec<ReviewerPlan> {
        [
            (
                AGENT,
                "scaffold-model",
                "primary",
                crate::review::Lens::Acceptance,
            ),
            (
                REVIEW_AGENT,
                "scaffold-second-model",
                "second_opinion",
                crate::review::Lens::SecondOpinion,
            ),
        ]
        .into_iter()
        .take(count)
        .map(|(agent, model, name, lens)| {
            let mut profile = crate::review::profile_for(agent, model, name, Effort::High);
            profile.pool = self.pool.clone().unwrap_or_default();
            ReviewerPlan {
                agent: AgentId::new(agent),
                profile,
                lens,
                preflight_cli_version: None,
                timeout: Duration::from_secs(120),
            }
        })
        .collect()
    }
}

impl super::attempt::AttemptPlans for WidePlans {
    fn inputs(
        &self,
        request: &super::attempt::InputsRequest<'_>,
    ) -> Result<super::attempt::ReviewInputs, UpstrokeError> {
        Ok(super::attempt::ReviewInputs {
            title: request.entry.spec.title.clone(),
            body: request.entry.spec.body.clone(),
            acceptance: request.entry.spec.acceptance.clone(),
            diff: request.diff.clone(),
            artifacts: Vec::new(),
            decisions: Vec::new(),
            stem: crate::util::filename_component(request.entry.display_id.as_str()),
        })
    }

    fn pool_for(&self, _agent: &str) -> Option<String> {
        self.pool.clone()
    }

    fn plan(
        &self,
        request: &super::attempt::PlanRequest<'_>,
    ) -> Result<AttemptPlan, UpstrokeError> {
        let display = request.entry.display_id.as_str().to_owned();
        let worker = CommandSpec::new(request.binding.agent.as_str())
            .arg("--implement")
            .arg(display);
        let worker = match &request.resume_session {
            Some(session) => worker.arg("--resume").arg(session.0.clone()),
            None => worker,
        };
        Ok(AttemptPlan {
            attempt: request.attempt,
            rung: request.rung,
            binding: request.binding.clone(),
            pool: self.pool.clone(),
            resume_session: request.resume_session.clone(),
            materialization_observed: request.materialization_observed,
            agent: AgentId::new(&request.binding.agent),
            session_resume: true,
            worker,
            worker_timeout: Duration::from_secs(300),
            gates: Self::gate_plans(self.gates),
            reviewers: self.reviewer_plans(self.reviewers),
        })
    }

    fn verification(
        &self,
        _request: &super::attempt::VerificationRequest<'_>,
    ) -> Result<super::attempt::VerificationPlan, UpstrokeError> {
        Ok(super::attempt::VerificationPlan {
            gates: Self::gate_plans(self.verify_gates),
            reviewers: self.reviewer_plans(self.verify_reviewers),
        })
    }
}

impl Default for WidePlans {
    fn default() -> Self {
        Self {
            gates: 1,
            reviewers: 1,
            verify_gates: 1,
            verify_reviewers: 1,
            pool: None,
        }
    }
}

pub(super) fn wide_responder(tasks: &[WideTask], failing_gates: &[(u32, u32)]) -> Responder {
    wide_responder_asking(tasks, failing_gates, &[])
}

pub(super) fn wide_responder_asking(
    tasks: &[WideTask],
    failing_gates: &[(u32, u32)],
    asking: &[u32],
) -> Responder {
    let writes: Vec<(String, &'static str)> = tasks
        .iter()
        .map(|task| (task.writes.clone(), task.id))
        .collect();
    let failing = failing_gates.to_vec();
    let asking = asking.to_vec();
    Box::new(move |request: &RunnerRequest| {
        let role = match &request.invocation {
            InvocationId::Attempt { role, .. } => Some(*role),
            InvocationId::Sequence { .. } | InvocationId::Probe { .. } => None,
        };
        if matches!(request.role, ExecutionRole::Review) {
            return Ok(exited(0, PASSING_VERDICT.to_owned()));
        }
        let InvocationId::Attempt { key, attempt, .. } = &request.invocation else {
            return Ok(exited(0, String::new()));
        };
        let Some(role) = role else {
            return Ok(exited(0, String::new()));
        };
        match role {
            crate::runner::invocation::AttemptRole::Worker if asking.contains(&key.0) => {
                Ok(exited(0, WORKER_QUESTION.to_owned()))
            }
            crate::runner::invocation::AttemptRole::Worker => {
                let (path, id) = writes
                    .get(key.0 as usize)
                    .cloned()
                    .unwrap_or_else(|| (format!("unknown-{}.txt", key.0), "unknown"));
                write_file(
                    &request.workspace.join(&path),
                    format!("{id} attempt {}\n", attempt.0).as_bytes(),
                );
                Ok(exited(0, format!("{id} worked\n")))
            }
            crate::runner::invocation::AttemptRole::Gate(_)
                if failing.contains(&(key.0, attempt.0)) =>
            {
                Ok(exited(1, format!("{GATE_DIAGNOSTIC} (exit 1)\n")))
            }
            _ => Ok(exited(0, String::new())),
        }
    })
}

pub(super) struct WideEnv {
    pub(super) answers: Arc<dyn crate::interaction::AnswerSource + Send + Sync>,
    pub(super) halts_run: bool,
    pub(super) harness: Arc<Mutex<HookHarness>>,
    pub(super) runner: Arc<RecordingRunner>,
    pub(super) adapters: Arc<ScaffoldAdapters>,
    pub(super) plans: Arc<WidePlans>,
    pub(super) paths: crate::rundir::RunPaths,
    pub(super) log: PathBuf,
    pub(super) inputs: FrozenInputs,
    pub(super) max_parallel: u32,
    pub(super) fixture: Fixture,
}

pub(super) struct Wide {
    pub(super) run: super::run::TopologyRun,
    pub(super) env: WideEnv,
}

impl Wide {
    pub(super) fn started(tag: &str, tasks: &[WideTask], max_parallel: u32) -> Self {
        Self::started_with(
            tag,
            tasks,
            max_parallel,
            WidePlans::default(),
            RecordingRunner::new().answering(wide_responder(tasks, &[])),
        )
    }

    pub(super) fn started_with(
        tag: &str,
        tasks: &[WideTask],
        max_parallel: u32,
        plans: WidePlans,
        runner: RecordingRunner,
    ) -> Self {
        let fixture = Fixture::created(tag);
        let plan = wide_plan(tasks);
        let started = run_started_of(&fixture, &plan, max_parallel, plans.reviewers >= 2);
        let inputs = FrozenInputs {
            plan,
            normalized_plan_digest: NORMALIZED_DIGEST.to_owned(),
        };
        let public =
            crate::rundir::public_dir(&fixture.base, crate::workspace_manager::fixture::RUN_ID);
        let lock = crate::rundir::RunLock::acquire(&public).expect("the run lock");
        let worktree =
            crate::rundir::WorktreeLock::acquire_in(&fixture.base, &fixture.base.join(".git"))
                .expect("the worktree lease");
        let log_path = fixture.private.join("events.jsonl");
        let mut log = EventLog::open(EventSite::OpenLog, &log_path, &mut Vec::new())
            .expect("open the schema-4 log");
        let event = TopologyEvent {
            ts: <super::seams::SystemClock as super::seams::TimeSource>::now_rfc3339(
                &super::seams::SystemClock,
            ),
            body: TopologyEventBody::RunStarted {
                data: Box::new(started.clone()),
            },
        };
        let (line, checked) = TopologyLine::round_trip(&event).expect("run_started round-trips");
        let mut fold = TopologyFold::new(inputs.clone());
        let delta = fold
            .plan_transition(&checked)
            .expect("the fold takes run_started");
        log.append_topology_hooked(
            site_for(&checked.body),
            &line,
            &mut crate::events::log::NoEventHooks,
        )
        .expect("append run_started");
        fold.apply_delta(delta);
        fixture
            .manager
            .create_ref_zero_old(
                &mut crate::workspace_manager::NoHooks,
                crate::topology::effects::RefSite::CreateIntegration,
                started.integration_ref.as_str(),
                &fixture.head,
            )
            .expect("the integration ref");
        let digest = crate::events::log::first_line_digest(line.committed_bytes())
            .expect("a committed first line");
        let mut handle =
            super::recover::RunHandle::created(started, digest, log, fold, lock, worktree);
        handle.events.push(checked);
        let run = super::run::TopologyRun::resumed(
            handle,
            inputs.clone(),
            super::select::Ceiling::unlimited(),
        );
        runner.watching(&log_path);
        let paths = scaffold_run_paths(&fixture);
        Self {
            run,
            env: WideEnv {
                answers: Arc::new(crate::interaction::UnattendedAnswers),
                halts_run: false,
                harness: Arc::new(Mutex::new(HookHarness::new())),
                runner: Arc::new(runner),
                adapters: Arc::new(ScaffoldAdapters::new()),
                plans: Arc::new(plans),
                paths,
                log: log_path,
                inputs,
                max_parallel,
                fixture,
            },
        }
    }
}

impl WideEnv {
    pub(super) fn seams(&self) -> super::run::RunSeams<'_> {
        super::run::RunSeams {
            manager: &self.fixture.manager,
            clock: &super::seams::SystemClock,
            sleeper: &NoSleep,
            runner: &*self.runner,
            adapters: &*self.adapters,
            paths: &self.paths,
            plans: &*self.plans,
            reviews: &crate::engine::attempt::LegacyReviewPasses,
            input_policy: &crate::engine::attempt::LegacyReviewInputPolicy,
            answers: &*self.answers,
            ids: &super::seams::RealIds,
            halts_run: self.halts_run,
        }
    }

    pub(super) fn pipelines(&self) -> super::coordinator::PipelineSeams {
        self.pipelines_limited(SlotLimitsOf::Defaulted)
    }

    pub(super) fn pipelines_limited(
        &self,
        limits: SlotLimitsOf,
    ) -> super::coordinator::PipelineSeams {
        let harness = Arc::clone(&self.harness);
        let slots = match limits {
            SlotLimitsOf::Defaulted => super::identity::SlotLimits::defaulted(self.max_parallel),
            SlotLimitsOf::Exactly(per_agent, per_pool) => {
                super::identity::SlotLimits::new(per_agent, per_pool).expect("slot limits")
            }
        };
        let runner: Arc<dyn Runner> = Arc::clone(&self.runner) as Arc<dyn Runner>;
        let adapters: Arc<dyn crate::agent::AdapterSource + Send + Sync> =
            Arc::clone(&self.adapters) as Arc<dyn crate::agent::AdapterSource + Send + Sync>;
        let plans: Arc<dyn super::attempt::AttemptPlans + Send + Sync> =
            Arc::clone(&self.plans) as Arc<dyn super::attempt::AttemptPlans + Send + Sync>;
        super::coordinator::PipelineSeams {
            manager: self.fixture.manager.clone(),
            runner,
            adapters,
            paths: self.paths.clone(),
            plans,
            reviews: Arc::new(crate::engine::attempt::LegacyReviewPasses),
            input_policy: Arc::new(crate::engine::attempt::LegacyReviewInputPolicy),
            hooks: Arc::new(move || {
                Box::new(super::seams::HarnessTopologyHooks::new(Arc::clone(
                    &harness,
                ))) as Box<dyn super::seams::TopologyHooks + Send>
            }),
            slots,
        }
    }

    pub(super) fn hooks(&self) -> super::seams::HarnessTopologyHooks {
        super::seams::HarnessTopologyHooks::new(Arc::clone(&self.harness))
    }

    pub(super) fn durable_events(&self) -> Vec<TopologyEvent> {
        let bytes = std::fs::read(&self.log).expect("read the log back");
        TopologyFold::parse_log(&bytes).expect("the log parses")
    }

    pub(super) fn head(&self, run: &super::run::TopologyRun) -> String {
        let integration = run
            .fold()
            .started()
            .expect("started")
            .integration_ref
            .clone();
        self.fixture
            .manager
            .direct_ref_target(integration.as_str())
            .expect("read the integration ref")
            .expect("the integration ref exists")
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum SlotLimitsOf {
    Defaulted,
    Exactly(u32, u32),
}
