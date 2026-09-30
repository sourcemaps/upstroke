//! Extended notes: `docs/internals/engine/topology/coordinator.md`

use std::collections::{BTreeMap, VecDeque};
use std::panic::AssertUnwindSafe;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};

use crate::agent::AdapterSource;
use crate::error::UpstrokeError;
use crate::events::ReviewRecord;
use crate::rundir::RunPaths;
use crate::runner::{Cancellation, InvocationId, Runner};
use crate::topology::events::{
    AttemptNumber, CandidateRef, GenerationId, SequenceId, TopologyEventBody,
};
use crate::topology::registry::TaskKey;
use crate::workspace_manager::WorkspaceManager;

use super::attempt::{
    AttemptJob, AttemptPlans, JudgeError, Judged, Judgement, ReviewInputPolicy, ReviewPasses, Work,
    attempt_body,
};
use super::closure;
use super::identity::{
    Admission, AttemptIdentities, InvocationEnd, SequenceIdentities, SlotLimits, SlotPair,
};
use super::integrate::{self, Verified, VerifyRequest};
use super::preflight::{Carried, Registrar};
use super::run::{
    DrivenJournal, Driver, Progress, Retrying, RunSeams, SpendAccount, TopologyRun,
    VerificationJob, verification_body, verified,
};
use super::seams::TopologyHooks;
use super::select::{Admitted, Entitlements, Holds, Standing};

pub type HooksFactory = Arc<dyn Fn() -> Box<dyn TopologyHooks + Send> + Send + Sync>;

pub type Reply = oneshot::Sender<Result<(), UpstrokeError>>;

// Owned seams a spawned pipeline takes to its thread. Each `Arc` is shared by the
// coordinator and every pipeline it spawns for the life of the entry, which is
// the multi-owner lifecycle standards §6 asks to be named.
#[derive(Clone)]
pub struct PipelineSeams {
    pub manager: WorkspaceManager,
    pub runner: Arc<dyn Runner>,
    pub adapters: Arc<dyn AdapterSource + Send + Sync>,
    pub paths: RunPaths,
    pub plans: Arc<dyn AttemptPlans + Send + Sync>,
    pub reviews: Arc<dyn ReviewPasses + Send + Sync>,
    pub input_policy: Arc<dyn ReviewInputPolicy + Send + Sync>,
    pub hooks: HooksFactory,
    pub slots: SlotLimits,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PipelineId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Identity {
    Attempt {
        key: TaskKey,
        generation: GenerationId,
        attempt: AttemptNumber,
    },
    Verification {
        sequence: SequenceId,
        candidate: CandidateRef,
    },
}

impl Identity {
    fn owns(&self, invocation: &InvocationId) -> bool {
        match (self, invocation) {
            (
                Self::Attempt {
                    key,
                    generation,
                    attempt,
                },
                InvocationId::Attempt {
                    key: of_key,
                    generation: of_generation,
                    attempt: of_attempt,
                    ..
                },
            ) => key == of_key && generation == of_generation && attempt == of_attempt,
            (Self::Verification { sequence, .. }, InvocationId::Sequence { sequence: of, .. }) => {
                sequence == of
            }
            _ => false,
        }
    }

    fn open_in(&self, run: &TopologyRun) -> bool {
        match self {
            Self::Attempt {
                key,
                generation,
                attempt,
            } => {
                let worker = AttemptIdentities::new(*key, *generation, *attempt).worker();
                Standing::of(run.fold(), &worker).holds() == Holds::Pipeline
            }
            Self::Verification {
                sequence,
                candidate,
            } => {
                let gate = SequenceIdentities::new(*sequence).gate(0, 0);
                Standing::of(run.fold(), &gate).holds() == Holds::PipelineAndMerge
                    && run
                        .fold()
                        .transaction()
                        .is_some_and(|open| open.candidate == *candidate)
            }
        }
    }
}

pub enum ToCoordinator {
    Admit {
        pipeline: PipelineId,
        invocation: InvocationId,
        pair: Option<SlotPair>,
        reply: Reply,
    },
    Ended {
        pipeline: PipelineId,
        invocation: InvocationId,
        end: InvocationEnd,
    },
    SnapshotBegin {
        pipeline: PipelineId,
        reply: Reply,
    },
    SnapshotEnd {
        pipeline: PipelineId,
    },
    Judged {
        pipeline: PipelineId,
        identity: Identity,
        outcome: Result<Box<Judged>, UpstrokeError>,
    },
    Verified {
        pipeline: PipelineId,
        identity: Identity,
        outcome: Box<Result<Judgement, JudgeError>>,
        charged: Vec<ReviewRecord>,
    },
    Shutdown,
}

impl ToCoordinator {
    const fn pipeline(&self) -> Option<PipelineId> {
        match self {
            Self::Admit { pipeline, .. }
            | Self::Ended { pipeline, .. }
            | Self::SnapshotBegin { pipeline, .. }
            | Self::SnapshotEnd { pipeline }
            | Self::Judged { pipeline, .. }
            | Self::Verified { pipeline, .. } => Some(*pipeline),
            Self::Shutdown => None,
        }
    }
}

pub trait Quiescence {
    fn granted(&mut self, invocation: &InvocationId);

    fn quiescent(&mut self, view: &Quiescent<'_>) -> Release;
}

#[derive(Debug, Clone, PartialEq)]
pub enum Release {
    Invocation(InvocationId),
    Injected,
    Append(Box<TopologyEventBody>),
    Nothing,
}

pub struct Quiescent<'a> {
    pub invoking: Vec<InvocationId>,
    pub live: Vec<(PipelineId, Identity)>,
    pub run: &'a TopologyRun,
    pub injector: &'a Injector,
}

pub struct Injector(mpsc::UnboundedSender<ToCoordinator>);

impl Injector {
    #[must_use]
    pub fn inject(&self, message: ToCoordinator) -> bool {
        self.0.send(message).is_ok()
    }
}

impl TopologyRun {
    pub fn run_concurrently(
        &mut self,
        seams: &RunSeams<'_>,
        pipelines: &PipelineSeams,
        hooks: &mut dyn TopologyHooks,
        observer: Option<&mut dyn Quiescence>,
    ) -> Result<Progress, UpstrokeError> {
        let _cleanup_scope = self.cleanup_scope();
        #[cfg(unix)]
        let leases = crate::rundir::active_cleanup_lease_paths();
        #[cfg(not(unix))]
        let leases = Vec::new();
        self.limit_slots(pipelines.slots)?;
        let width = Entitlements::of(self.fold()).max_parallel().max(1);
        let name = std::thread::current()
            .name()
            .unwrap_or("upstroke-coordinator")
            .to_owned();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .max_blocking_threads(usize::try_from(width).unwrap_or(usize::MAX))
            .thread_name(name)
            .build()
            .map_err(|error| UpstrokeError::Refused {
                message: format!(
                    "the coordinator's runtime could not be built ({error}); nothing was \
                     spawned or appended"
                ),
            })?;
        let (outbox, inbox) = mpsc::unbounded_channel();
        let (injector, injected) = mpsc::unbounded_channel();
        let mut coordinator = Coordinator {
            run: self,
            seams,
            hooks,
            pipelines,
            observer: match observer {
                Some(observer) => Some(observer),
                None => None,
            },
            leases,
            live: BTreeMap::new(),
            replies: BTreeMap::new(),
            gate: SnapshotGate::default(),
            next: 0,
            in_verify: None,
            arrived: None,
            abandoned: None,
            interrupt: None,
            cancelled_work: closure::Cancelled::none(),
            unresolved: Vec::new(),
            buffer: Vec::new(),
            arrivals: 0,
            handles: Vec::new(),
            inbox,
            outbox,
            injected,
            injector: Injector(injector),
            runtime,
        };
        let outcome = coordinator.drive();
        coordinator.join();
        outcome
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Busy {
    Running,
    Awaiting,
    Invoking(InvocationId),
    Done,
}

struct Live {
    identity: Identity,
    cancel: Cancellation,
    cancelled: bool,
    busy: Busy,
    running: Option<InvocationId>,
    job: Option<AttemptJob>,
}

enum Interrupt {
    Halt,
    Shutdown,
    Failed(UpstrokeError),
}

impl Interrupt {
    const fn describe(&self) -> &'static str {
        match self {
            Self::Halt => "a halting settlement",
            Self::Shutdown => "a shutdown",
            Self::Failed(_) => "an error that ends the command",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VerifyEnd {
    Arrived,
    Abandoned,
    Interrupted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Origin {
    Pipeline,
    Injected,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum GateMode {
    #[default]
    Open,
    Closed,
    Verifying {
        granting: bool,
    },
}

// The snapshot gate (the record's R-W). The frozen stale integration reclaims every
// snapshot intent of the execution root, so no attempt snapshot may be live while
// `integrate()` runs outside its verification: attempt pipelines ask before each
// snapshot they add (`SnapshotBegin`) and report after removing it (`SnapshotEnd`).
// Owned by the coordinator thread alone; a grant is a reply on the asking
// pipeline's oneshot. Open grants at once; Closed queues; Verifying grants until
// the verification's completion arrives and queues after it. A pipeline that ends
// releases what it held and what it was waiting for.
#[derive(Default)]
struct SnapshotGate {
    held: BTreeMap<PipelineId, u32>,
    waiting: VecDeque<(PipelineId, Reply)>,
    mode: GateMode,
}

impl SnapshotGate {
    fn live(&self) -> u32 {
        self.held.values().copied().fold(0, u32::saturating_add)
    }

    const fn granting(&self) -> bool {
        matches!(
            self.mode,
            GateMode::Open | GateMode::Verifying { granting: true }
        )
    }

    const fn closed(&self) -> bool {
        matches!(self.mode, GateMode::Closed)
    }

    fn draining(&self) -> bool {
        self.closed() && self.live() > 0
    }

    fn grant(&mut self, pipeline: PipelineId) {
        let held = self.held.entry(pipeline).or_insert(0);
        *held = held.saturating_add(1);
    }

    fn end(&mut self, pipeline: PipelineId) -> bool {
        match self.held.get_mut(&pipeline) {
            Some(held) if *held > 0 => {
                *held -= 1;
                if *held == 0 {
                    self.held.remove(&pipeline);
                }
                true
            }
            _ => false,
        }
    }

    fn release(&mut self, pipeline: PipelineId) {
        self.held.remove(&pipeline);
        self.waiting.retain(|(waiting, _)| *waiting != pipeline);
    }

    fn take_granted(&mut self) -> Vec<(PipelineId, Reply)> {
        if !self.granting() {
            return Vec::new();
        }
        let granted: Vec<(PipelineId, Reply)> = self.waiting.drain(..).collect();
        for (pipeline, _) in &granted {
            self.grant(*pipeline);
        }
        granted
    }
}

// The coordinator's protocol (standards §10; the record's R-R, R-U, R-AA, R-AD).
// Owner: this struct, on the caller's thread, owns the run, its fold, the broker
// and the live table; a pipeline owns its job, its hooks and its seams' handles
// and reaches shared state only by message. Linearization point: applying a
// message here, one at a time; every append, grant, settlement and discard is
// made there. Transitions of a pipeline: Running until it asks (`Admit`,
// `SnapshotBegin`: Awaiting), then granted (Invoking; Running for a snapshot) or
// refused (Running), an end reported (Running), its completion (Done); on
// cancellation an Invoking pipeline is Running, an Awaiting one only once its
// request is answered (it may still be buffered); only `retire` removes it. Under an observer a grant is handed to it at once and
// nothing else is done until it returns, which it does when the invocation is
// inside the Runner, so no Invoking pipeline is still on its way there when the
// coordinator next acts. Winner and loser: a completion is settled only when its
// pipeline is live, not cancelled, bound to the identity it names, and that
// identity is open in the fold; any other message is discarded and counted and
// releases nothing twice; a fatal completion interrupts as it is received. An
// invocation is released only when its end established that its process is
// gone: one that ended unresolved, or was never reported ended, keeps its
// registration and pair for the rest of this process, and interrupts. Every
// admission pass first stops each pipeline whose identity the fold has closed,
// and selects nothing while `verify`'s own transaction is gone. A grant, of a
// pair or a snapshot, reaches only a live, uncancelled pipeline whose identity
// the fold holds open, with no interrupt recorded; one freed for any other is
// withdrawn and its request refused. A halt interrupts every in-flight identity
// the fold shows, a verification whose result has arrived unprepared included.
// Cleanup: an interrupt cancels every live token, withdraws every pending
// registration and refuses every waiting reply; `finish` receives until no
// pipeline is live, and `join` awaits every handle, a panic being its
// pipeline's completion. The channels are bounded by this protocol, not
// by type: a pipeline has at most one request unanswered and at most three
// notifications (an end, a snapshot's end, its completion) sent since it, and the
// fold's entitlements bound the pipelines. The coordinator keeps the job it
// settles against; the pipeline takes its own copy.
struct Coordinator<'s> {
    run: &'s mut TopologyRun,
    seams: &'s RunSeams<'s>,
    hooks: &'s mut dyn TopologyHooks,
    pipelines: &'s PipelineSeams,
    observer: Option<&'s mut dyn Quiescence>,
    leases: Vec<PathBuf>,
    live: BTreeMap<PipelineId, Live>,
    replies: BTreeMap<InvocationId, (PipelineId, Reply)>,
    gate: SnapshotGate,
    next: u64,
    in_verify: Option<(PipelineId, Identity)>,
    arrived: Option<Verified>,
    abandoned: Option<SequenceId>,
    interrupt: Option<Interrupt>,
    cancelled_work: closure::Cancelled,
    unresolved: Vec<String>,
    buffer: Vec<(u64, ToCoordinator)>,
    arrivals: u64,
    handles: Vec<tokio::task::JoinHandle<()>>,
    inbox: mpsc::UnboundedReceiver<ToCoordinator>,
    outbox: mpsc::UnboundedSender<ToCoordinator>,
    injected: mpsc::UnboundedReceiver<ToCoordinator>,
    injector: Injector,
    runtime: tokio::runtime::Runtime,
}

impl Coordinator<'_> {
    fn drive(&mut self) -> Result<Progress, UpstrokeError> {
        loop {
            if let Err(error) = self.admit() {
                self.fail(error);
            }
            if self.interrupt.is_some() {
                return self.finish();
            }
            if self.live.is_empty() {
                match self.idle() {
                    Ok(progress @ Progress::Finished { .. }) => return Ok(progress),
                    Ok(_) => continue,
                    Err(error) => {
                        self.fail(error);
                        return self.finish();
                    }
                }
            }
            self.receive_one();
        }
    }

    fn receive_one(&mut self) {
        match self.next_message() {
            Ok((origin, message)) => {
                if let Err(error) = self.handle(origin, message) {
                    self.fail(error);
                }
            }
            Err(error) => self.fail(error),
        }
    }

    fn admit(&mut self) -> Result<(), UpstrokeError> {
        loop {
            if self.interrupt.is_some() {
                return Ok(());
            }
            let (poisoned, ending, halted) = {
                let fold = self.run.fold();
                (
                    fold.is_poisoned(),
                    fold.run_is_ending(),
                    fold.halted_at().is_some(),
                )
            };
            if poisoned {
                return Err(UpstrokeError::Refused {
                    message: "an append returned an error and this process's fold is \
                              poisoned; nothing further is admitted"
                        .to_owned(),
                });
            }
            self.reconcile();
            if ending {
                if halted && self.any_in_flight() {
                    self.halt();
                }
                return Ok(());
            }
            if self.abandoning() {
                return Ok(());
            }
            if self.gate.draining() {
                return Ok(());
            }
            if self.run.ingest_answers(self.seams, self.hooks)?.is_some() {
                continue;
            }
            match self.run.admitted()? {
                Admitted::BudgetExceeded(exceeded) => {
                    self.open_gate();
                    self.run.exceed_budget(*exceeded, self.seams, self.hooks)?;
                }
                Admitted::Integrate { candidate } => {
                    if self.in_verify.is_some() {
                        return Err(UpstrokeError::Refused {
                            message: format!(
                                "selection returned an integration of task {} while a \
                                 verification is open; the fold admits one transaction at a \
                                 time, so nothing was started",
                                candidate.key
                            ),
                        });
                    }
                    if !self.integrate(*candidate)? {
                        return Ok(());
                    }
                }
                Admitted::Retry {
                    key, generation, ..
                } => {
                    self.open_gate();
                    if let Retrying::Started(job) = self
                        .run
                        .begin_retry(key, generation, self.seams, self.hooks)?
                    {
                        self.spawn_attempt(*job);
                    }
                }
                Admitted::Dispatch {
                    key,
                    generation,
                    continuing,
                }
                | Admitted::RepairDispatch {
                    key,
                    generation,
                    continuing,
                } => {
                    self.open_gate();
                    let job = self
                        .run
                        .begin_dispatch(key, generation, continuing, self.seams, self.hooks)?;
                    self.spawn_attempt(job);
                }
                Admitted::Backoff | Admitted::HardBlock { .. } | Admitted::Closure(_) => {
                    self.open_gate();
                    return Ok(());
                }
            }
        }
    }

    fn any_in_flight(&self) -> bool {
        !self.live.is_empty() || !closure::in_flight(self.run.fold()).is_empty()
    }

    fn abandoning(&self) -> bool {
        self.in_verify
            .as_ref()
            .is_some_and(|(_, verification)| !verification.open_in(self.run))
    }

    fn integrate(&mut self, candidate: CandidateRef) -> Result<bool, UpstrokeError> {
        let request = self.run.integration_request(&candidate)?;
        let stale = request.base_sha != request.authorized.head;
        self.gate.mode = GateMode::Closed;
        if stale && self.gate.live() > 0 {
            return Ok(false);
        }
        let key = candidate.key;
        self.run.reserve_integration(key)?;
        let manager = self.seams.manager;
        let terminal = integrate::integrate(&mut DrivenJournal(self), manager, &request);
        let settled = self.run.integration_settled(key, terminal);
        self.open_gate();
        let abandoned = self.abandoned.take() == Some(request.sequence);
        match settled {
            Ok(_) => Ok(true),
            Err(_) if self.interrupt.is_some() => Ok(false),
            Err(_) if abandoned => Ok(true),
            Err(error) => Err(error),
        }
    }

    fn open_gate(&mut self) {
        if self.interrupt.is_some() || self.in_verify.is_some() {
            return;
        }
        self.gate.mode = GateMode::Open;
        self.grant_snapshots();
    }

    fn grant_snapshots(&mut self) {
        for (pipeline, reply) in self.gate.take_granted() {
            if !self.receives(pipeline) {
                self.gate.end(pipeline);
                let _ = reply.send(Err(UpstrokeError::Refused {
                    message: format!(
                        "pipeline {} is cancelled or serves an identity the fold no longer holds \
                         open, so the snapshot it waited for was not granted",
                        pipeline.0
                    ),
                }));
                self.set_busy(pipeline, Busy::Running);
            } else if reply.send(Ok(())).is_ok() {
                self.set_busy(pipeline, Busy::Running);
            } else {
                self.gate.end(pipeline);
            }
        }
    }

    fn idle(&mut self) -> Result<Progress, UpstrokeError> {
        match self.run.admitted()? {
            Admitted::Backoff => self.run.back_off(self.seams, self.hooks),
            Admitted::HardBlock { questions } => {
                self.run.hard_block(&questions, self.seams, self.hooks)
            }
            Admitted::Closure(_) => {
                self.run
                    .close_run(&closure::Cancelled::none(), self.seams, self.hooks)
            }
            other => Err(UpstrokeError::Refused {
                message: format!(
                    "no pipeline is live and selection admitted {other:?}, which the admission \
                     pass takes before the coordinator is idle; nothing was appended"
                ),
            }),
        }
    }

    fn next_pipeline(&mut self) -> PipelineId {
        self.next = self.next.saturating_add(1);
        PipelineId(self.next)
    }

    fn client(&self, pipeline: PipelineId, gated: bool) -> Client {
        Client {
            pipeline,
            outbox: self.outbox.clone(),
            gated,
        }
    }

    fn spawn_attempt(&mut self, job: AttemptJob) {
        let pipeline = self.next_pipeline();
        let identity = Identity::Attempt {
            key: job.key,
            generation: job.generation,
            attempt: job.plan.attempt,
        };
        let cancel = Cancellation::new();
        let standing = Standing::of(self.run.fold(), &job.identities().worker());
        let carried = Carried::new(cancel.clone(), self.leases.clone());
        let client = self.client(pipeline, true);
        let seams = self.pipelines.clone();
        let mut hooks = (self.pipelines.hooks)();
        let owned = job.clone();
        let sent = identity.clone();
        self.live.insert(
            pipeline,
            Live {
                identity,
                cancel,
                cancelled: false,
                busy: Busy::Running,
                running: None,
                job: Some(job),
            },
        );
        let handle = self.runtime.spawn_blocking(move || {
            let outcome = std::panic::catch_unwind(AssertUnwindSafe(|| {
                let mut work = Work {
                    manager: &seams.manager,
                    hooks: &mut *hooks,
                    runner: &*seams.runner,
                    standing,
                    registrar: &client,
                    carried: &carried,
                    adapters: &*seams.adapters,
                    paths: &seams.paths,
                    plans: &*seams.plans,
                    reviews: &*seams.reviews,
                    input_policy: &*seams.input_policy,
                };
                attempt_body(&mut work, &owned)
            }))
            .unwrap_or_else(|panic| Err(panicked(panic.as_ref())));
            client.send(ToCoordinator::Judged {
                pipeline,
                identity: sent,
                outcome: outcome.map(Box::new),
            });
        });
        self.handles.push(handle);
    }

    fn spawn_verification(&mut self, job: VerificationJob) -> PipelineId {
        let pipeline = self.next_pipeline();
        let identity = Identity::Verification {
            sequence: job.sequence,
            candidate: job.candidate.clone(),
        };
        let cancel = Cancellation::new();
        let standing = Standing::of(
            self.run.fold(),
            &SequenceIdentities::new(job.sequence).gate(0, 0),
        );
        let carried = Carried::new(cancel.clone(), self.leases.clone());
        let client = self.client(pipeline, false);
        let seams = self.pipelines.clone();
        let mut hooks = (self.pipelines.hooks)();
        let sent = identity.clone();
        self.live.insert(
            pipeline,
            Live {
                identity,
                cancel,
                cancelled: false,
                busy: Busy::Running,
                running: None,
                job: None,
            },
        );
        let handle = self.runtime.spawn_blocking(move || {
            let mut charged = Vec::new();
            let key = job.candidate.key;
            let outcome = std::panic::catch_unwind(AssertUnwindSafe(|| {
                let mut work = Work {
                    manager: &seams.manager,
                    hooks: &mut *hooks,
                    runner: &*seams.runner,
                    standing,
                    registrar: &client,
                    carried: &carried,
                    adapters: &*seams.adapters,
                    paths: &seams.paths,
                    plans: &*seams.plans,
                    reviews: &*seams.reviews,
                    input_policy: &*seams.input_policy,
                };
                let mut account = SpendAccount {
                    spend: None,
                    key,
                    charged: &mut charged,
                };
                verification_body(&mut work, &job, &mut account)
            }))
            .unwrap_or_else(|panic| Err(JudgeError::Other(panicked(panic.as_ref()))));
            client.send(ToCoordinator::Verified {
                pipeline,
                identity: sent,
                outcome: Box::new(outcome),
                charged,
            });
        });
        self.handles.push(handle);
        pipeline
    }

    fn verify_concurrently(
        &mut self,
        request: &VerifyRequest<'_>,
    ) -> Result<Verified, UpstrokeError> {
        if self.in_verify.is_some() {
            return Err(UpstrokeError::Refused {
                message: format!(
                    "the verification of sequence {} was asked for inside another \
                     verification; one transaction is open at a time, so nothing was spawned",
                    request.sequence.0
                ),
            });
        }
        let job = self.run.verification_job(request)?;
        let key = request.candidate.key;
        let sequence = request.sequence;
        let verification = Identity::Verification {
            sequence,
            candidate: request.candidate.clone(),
        };
        let pipeline = self.spawn_verification(job);
        self.in_verify = Some((pipeline, verification.clone()));
        self.arrived = None;
        self.gate.mode = GateMode::Verifying { granting: true };
        self.grant_snapshots();
        let ended = loop {
            if let Err(error) = self.admit() {
                self.fail(error);
            }
            if self.interrupt.is_some() {
                break VerifyEnd::Interrupted;
            }
            if !verification.open_in(self.run) {
                if !self.live.contains_key(&pipeline) {
                    break VerifyEnd::Abandoned;
                }
            } else if self.arrived.is_some() {
                self.gate.mode = GateMode::Verifying { granting: false };
                if self.gate.live() == 0 {
                    break VerifyEnd::Arrived;
                }
            }
            self.receive_one();
        };
        self.in_verify = None;
        self.gate.mode = GateMode::Closed;
        let arrived = self.arrived.take();
        match (ended, arrived) {
            (VerifyEnd::Arrived, Some(verified)) => Ok(verified),
            (VerifyEnd::Abandoned, _) => {
                self.abandoned = Some(sequence);
                let message = format!(
                    "the verification of sequence {} of task {key} was abandoned: the fold no \
                     longer holds its transaction open (a decline, or a failed settlement, \
                     failed its lineage), so its pipeline was cancelled, its late result \
                     discarded and nothing is appended for it",
                    sequence.0
                );
                self.run.warn(message.clone());
                Err(UpstrokeError::Refused { message })
            }
            (VerifyEnd::Arrived | VerifyEnd::Interrupted, _) => {
                self.cancelled_work.sequence(sequence);
                Err(UpstrokeError::Refused {
                    message: format!(
                        "the verification of sequence {} of task {key} was interrupted by {}; its \
                         pipeline was cancelled and nothing further is appended for it",
                        sequence.0,
                        self.interrupt
                            .as_ref()
                            .map_or("the coordinator", Interrupt::describe)
                    ),
                })
            }
        }
    }

    fn next_message(&mut self) -> Result<(Origin, ToCoordinator), UpstrokeError> {
        if self.observer.is_none() {
            let message = self.inbox.blocking_recv().ok_or_else(closed)?;
            self.note_receipt(&message);
            return Ok((Origin::Pipeline, message));
        }
        loop {
            if let Ok(message) = self.injected.try_recv() {
                return Ok((Origin::Injected, message));
            }
            while let Ok(message) = self.inbox.try_recv() {
                self.buffer_message(message);
            }
            if self.any_running() {
                let message = self.inbox.blocking_recv().ok_or_else(closed)?;
                self.buffer_message(message);
                continue;
            }
            if let Some(message) = self.take_canonical() {
                return Ok((Origin::Pipeline, message));
            }
            self.observe()?;
        }
    }

    fn buffer_message(&mut self, message: ToCoordinator) {
        self.note_receipt(&message);
        self.buffer.push((self.arrivals, message));
        self.arrivals = self.arrivals.saturating_add(1);
    }

    fn take_canonical(&mut self) -> Option<ToCoordinator> {
        let index = self
            .buffer
            .iter()
            .enumerate()
            .min_by_key(|(_, (arrival, message))| (message.pipeline(), *arrival))
            .map(|(index, _)| index)?;
        Some(self.buffer.remove(index).1)
    }

    fn any_running(&self) -> bool {
        self.live.values().any(|live| live.busy == Busy::Running)
    }

    fn observe(&mut self) -> Result<(), UpstrokeError> {
        let invoking: Vec<InvocationId> = self
            .live
            .values()
            .filter_map(|live| match &live.busy {
                Busy::Invoking(invocation) => Some(invocation.clone()),
                _ => None,
            })
            .collect();
        let live: Vec<(PipelineId, Identity)> = self
            .live
            .iter()
            .map(|(pipeline, entry)| (*pipeline, entry.identity.clone()))
            .collect();
        let described = format!(
            "{} pipeline(s) live, {} invocation(s) granted: {invoking:?}",
            live.len(),
            invoking.len()
        );
        let Some(observer) = self.observer.as_deref_mut() else {
            return Err(stuck(&described));
        };
        let view = Quiescent {
            invoking,
            live,
            run: self.run,
            injector: &self.injector,
        };
        match observer.quiescent(&view) {
            Release::Invocation(invocation) => {
                let owner = self
                    .live
                    .values_mut()
                    .find(|live| live.busy == Busy::Invoking(invocation.clone()));
                match owner {
                    Some(live) => {
                        live.busy = Busy::Running;
                        Ok(())
                    }
                    None => Err(stuck(&format!(
                        "the observer released `{invocation}`, which no live pipeline is \
                         running; {described}"
                    ))),
                }
            }
            Release::Injected if !self.injected.is_empty() => Ok(()),
            Release::Append(body) => self.run.emit(*body, self.seams, self.hooks),
            Release::Injected | Release::Nothing => Err(stuck(&described)),
        }
    }

    fn note_receipt(&mut self, message: &ToCoordinator) {
        let busy = match message {
            ToCoordinator::Admit { .. } | ToCoordinator::SnapshotBegin { .. } => Busy::Awaiting,
            ToCoordinator::Ended { .. } | ToCoordinator::SnapshotEnd { .. } => Busy::Running,
            ToCoordinator::Judged { .. } | ToCoordinator::Verified { .. } => Busy::Done,
            ToCoordinator::Shutdown => return,
        };
        if let Some(pipeline) = message.pipeline() {
            self.set_busy(pipeline, busy);
        }
    }

    fn set_busy(&mut self, pipeline: PipelineId, busy: Busy) {
        if let Some(live) = self.live.get_mut(&pipeline) {
            if live.busy != Busy::Done {
                live.busy = busy;
            }
        }
    }

    fn handle(&mut self, origin: Origin, message: ToCoordinator) -> Result<(), UpstrokeError> {
        match message {
            ToCoordinator::Admit {
                pipeline,
                invocation,
                pair,
                reply,
            } => {
                self.admit_invocation(origin, pipeline, invocation, pair, reply);
                Ok(())
            }
            ToCoordinator::Ended {
                pipeline,
                invocation,
                end,
            } => {
                self.end_invocation(origin, pipeline, &invocation, &end);
                Ok(())
            }
            ToCoordinator::SnapshotBegin { pipeline, reply } => {
                self.begin_snapshot(origin, pipeline, reply);
                Ok(())
            }
            ToCoordinator::SnapshotEnd { pipeline } => {
                if origin == Origin::Injected || !self.gate.end(pipeline) {
                    self.run.record_discard(Some(format!(
                        "pipeline {} reported the end of a snapshot it held none of; the gate \
                         was not moved",
                        pipeline.0
                    )));
                }
                Ok(())
            }
            ToCoordinator::Judged {
                pipeline,
                identity,
                outcome,
            } => self.judged(origin, pipeline, &identity, outcome),
            ToCoordinator::Verified {
                pipeline,
                identity,
                outcome,
                charged,
            } => {
                self.verified_arrived(origin, pipeline, &identity, *outcome, charged);
                Ok(())
            }
            ToCoordinator::Shutdown => {
                if self.interrupt.is_none() {
                    self.interrupt = Some(Interrupt::Shutdown);
                }
                self.cancel_all();
                Ok(())
            }
        }
    }

    fn admit_invocation(
        &mut self,
        origin: Origin,
        pipeline: PipelineId,
        invocation: InvocationId,
        pair: Option<SlotPair>,
        reply: Reply,
    ) {
        let refusal = match self.live.get(&pipeline) {
            _ if origin == Origin::Injected => Some(format!(
                "`{invocation}` was offered through the injector for pipeline {}",
                pipeline.0
            )),
            None => Some(format!(
                "`{invocation}` was offered by pipeline {}, which is not live",
                pipeline.0
            )),
            Some(live) if live.cancelled || self.interrupt.is_some() => {
                let _ = reply.send(Err(cancelled(&invocation)));
                if origin == Origin::Pipeline {
                    self.set_busy(pipeline, Busy::Running);
                }
                return;
            }
            Some(live) if live.identity.owns(&invocation) && !live.identity.open_in(self.run) => {
                let _ = reply.send(Err(cancelled(&invocation)));
                self.set_busy(pipeline, Busy::Running);
                return;
            }
            Some(live) if !live.identity.owns(&invocation) => Some(format!(
                "`{invocation}` was offered by pipeline {}, whose identity is {:?}",
                pipeline.0, live.identity
            )),
            Some(_) => None,
        };
        if let Some(warning) = refusal {
            let _ = reply.send(Err(UpstrokeError::Refused {
                message: format!("{warning}; the registration was refused"),
            }));
            self.run.record_discard(Some(format!(
                "{warning}; the registration was refused and counted"
            )));
            return;
        }
        let standing = Standing::of(self.run.fold(), &invocation);
        match self.run.broker_mut().register(&standing, &invocation, pair) {
            Ok(Admission::Runnable | Admission::Granted) => {
                if reply.send(Ok(())).is_ok() {
                    self.started(origin, pipeline, invocation);
                } else {
                    self.abandoned(&invocation);
                }
            }
            Ok(Admission::Pending) => {
                self.replies.insert(invocation, (pipeline, reply));
            }
            Err(error) => {
                let _ = reply.send(Err(error));
                if origin == Origin::Pipeline {
                    self.set_busy(pipeline, Busy::Running);
                }
            }
        }
    }

    fn started(&mut self, origin: Origin, pipeline: PipelineId, invocation: InvocationId) {
        if let Some(live) = self.live.get_mut(&pipeline) {
            live.running = Some(invocation.clone());
            if origin == Origin::Pipeline && live.busy != Busy::Done {
                if let Some(observer) = self.observer.as_deref_mut() {
                    observer.granted(&invocation);
                }
                live.busy = Busy::Invoking(invocation);
            }
        }
    }

    fn abandoned(&mut self, invocation: &InvocationId) {
        match self.run.broker_mut().cancel(invocation) {
            Ok(granted) => self.reply_granted(granted),
            Err(error) => self.run.warn(format!(
                "`{invocation}` was granted and not delivered, and withdrawing the grant \
                 failed: {error}"
            )),
        }
    }

    fn receives(&self, pipeline: PipelineId) -> bool {
        self.interrupt.is_none()
            && self
                .live
                .get(&pipeline)
                .is_some_and(|live| !live.cancelled && live.identity.open_in(self.run))
    }

    fn reply_granted(&mut self, granted: Vec<InvocationId>) {
        for invocation in granted {
            let Some((pipeline, reply)) = self.replies.remove(&invocation) else {
                self.run.warn(format!(
                    "the broker granted `{invocation}`, which no pipeline is waiting for"
                ));
                continue;
            };
            if !self.receives(pipeline) {
                let _ = reply.send(Err(cancelled(&invocation)));
                self.set_busy(pipeline, Busy::Running);
                self.run.warn(format!(
                    "the broker granted `{invocation}` to pipeline {}, which is cancelled or \
                     serves an identity the fold no longer holds open; the grant was withdrawn \
                     and the request refused, so no process of it started",
                    pipeline.0
                ));
                self.abandoned(&invocation);
                continue;
            }
            if reply.send(Ok(())).is_ok() {
                self.started(Origin::Pipeline, pipeline, invocation);
            } else {
                self.abandoned(&invocation);
            }
        }
    }

    fn end_invocation(
        &mut self,
        origin: Origin,
        pipeline: PipelineId,
        invocation: &InvocationId,
        end: &InvocationEnd,
    ) {
        let current = match self.live.get_mut(&pipeline) {
            None => {
                self.run.record_discard(Some(format!(
                    "the end of `{invocation}` came from pipeline {}, which is not live; it was \
                     discarded",
                    pipeline.0
                )));
                return;
            }
            Some(live) if !live.identity.owns(invocation) => {
                let warning = format!(
                    "the end of `{invocation}` came from pipeline {}, whose identity is {:?}; it \
                     was discarded",
                    pipeline.0, live.identity
                );
                self.run.record_discard(Some(warning));
                return;
            }
            Some(live) => {
                if origin == Origin::Pipeline && live.running.as_ref() == Some(invocation) {
                    live.running = None;
                    true
                } else {
                    false
                }
            }
        };
        if !current && !self.run.broker_mut().invocations().settled(invocation) {
            self.run.record_discard(Some(format!(
                "the end of `{invocation}` names an invocation pipeline {} is not running; it \
                 was discarded and nothing was released",
                pipeline.0
            )));
            return;
        }
        match self.run.broker_mut().end(invocation, end) {
            Ok(granted) => self.reply_granted(granted),
            Err(error) => self.run.record_discard(Some(format!(
                "the end of `{invocation}` was refused by the invocation ledger: {error}"
            ))),
        }
        if let InvocationEnd::Failed { detail, .. } = end {
            if current && end.unresolved() {
                self.hold_unresolved(
                    invocation,
                    format!("`{invocation}` ended with its process unresolved ({detail})"),
                );
            }
        }
    }

    fn hold_unresolved(&mut self, invocation: &InvocationId, cause: String) {
        let named = invocation.render();
        if !self.unresolved.contains(&named) {
            self.unresolved.push(named);
        }
        if self.interrupt.is_none() {
            self.fail(UpstrokeError::Refused {
                message: format!(
                    "{cause}: its registration and its slot pair stay held for the rest of this \
                     process, so nothing is granted on them; admission stopped, every live \
                     pipeline was cancelled and the command ends resumably, and the next \
                     process's census reclaims what is left"
                ),
            });
        }
    }

    fn begin_snapshot(&mut self, origin: Origin, pipeline: PipelineId, reply: Reply) {
        let known = origin == Origin::Pipeline && self.receives(pipeline);
        if !known {
            let _ = reply.send(Err(UpstrokeError::Refused {
                message: format!(
                    "pipeline {} asked for a snapshot and is not live, is being cancelled, or \
                     serves an identity the fold no longer holds open",
                    pipeline.0
                ),
            }));
            if origin == Origin::Pipeline {
                self.set_busy(pipeline, Busy::Running);
            }
            return;
        }
        if self.gate.granting() {
            if reply.send(Ok(())).is_ok() {
                self.gate.grant(pipeline);
                if origin == Origin::Pipeline {
                    self.set_busy(pipeline, Busy::Running);
                }
            }
        } else {
            self.gate.waiting.push_back((pipeline, reply));
        }
    }

    fn accepts(&mut self, origin: Origin, pipeline: PipelineId, identity: &Identity) -> bool {
        if self.run.fold().is_poisoned() {
            if origin == Origin::Pipeline {
                self.retire(pipeline);
            }
            self.run.record_discard(None);
            return false;
        }
        let Some(live) = self.live.get(&pipeline) else {
            self.run.record_discard(Some(format!(
                "a completion for {identity:?} came from pipeline {}, which is not live (stale \
                 or duplicate); it was discarded",
                pipeline.0
            )));
            return false;
        };
        if live.identity != *identity {
            let warning = format!(
                "a completion for {identity:?} came from pipeline {}, whose identity is {:?}; \
                 it was discarded",
                pipeline.0, live.identity
            );
            self.run.record_discard(Some(warning));
            return false;
        }
        if live.cancelled {
            if origin == Origin::Pipeline {
                self.retire(pipeline);
            }
            self.run.record_discard(None);
            return false;
        }
        if !identity.open_in(self.run) {
            if origin == Origin::Pipeline {
                self.retire(pipeline);
            }
            self.run.record_discard(Some(format!(
                "a completion for {identity:?} names an identity the fold does not hold open; \
                 it was discarded"
            )));
            return false;
        }
        if origin == Origin::Injected {
            self.run.record_discard(Some(format!(
                "a completion for {identity:?} was injected for pipeline {} while that \
                 pipeline is still running; it was discarded",
                pipeline.0
            )));
            return false;
        }
        true
    }

    fn retire(&mut self, pipeline: PipelineId) -> Option<Live> {
        self.gate.release(pipeline);
        let live = self.live.remove(&pipeline)?;
        if let Some(invocation) = live.running.as_ref() {
            self.hold_unresolved(
                invocation,
                format!(
                    "pipeline {} ended without reporting the end of `{invocation}`, so the \
                     Runner never established that its process ended",
                    pipeline.0
                ),
            );
        }
        Some(live)
    }

    fn reconcile(&mut self) {
        let closed: Vec<PipelineId> = self
            .live
            .iter()
            .filter(|(_, live)| !live.cancelled && !live.identity.open_in(self.run))
            .map(|(pipeline, _)| *pipeline)
            .collect();
        for pipeline in closed {
            self.stop(pipeline);
        }
    }

    fn stop(&mut self, pipeline: PipelineId) {
        let Some(live) = self.live.get_mut(&pipeline) else {
            return;
        };
        live.cancelled = true;
        live.cancel.cancel();
        if matches!(live.busy, Busy::Invoking(_)) {
            live.busy = Busy::Running;
        }
        let identity = live.identity.clone();
        let waiting: Vec<InvocationId> = self
            .replies
            .iter()
            .filter(|(_, (owner, _))| *owner == pipeline)
            .map(|(invocation, _)| invocation.clone())
            .collect();
        for invocation in waiting {
            if let Some((_, reply)) = self.replies.remove(&invocation) {
                let _ = reply.send(Err(cancelled(&invocation)));
                self.set_busy(pipeline, Busy::Running);
            }
            match self.run.broker_mut().cancel(&invocation) {
                Ok(granted) => self.reply_granted(granted),
                Err(error) => self.run.warn(format!(
                    "withdrawing `{invocation}` of stopped pipeline {} failed: {error}",
                    pipeline.0
                )),
            }
        }
        let (refused, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut self.gate.waiting)
            .into_iter()
            .partition(|(waiting, _)| *waiting == pipeline);
        self.gate.waiting = kept.into_iter().collect();
        for (_, reply) in refused {
            let _ = reply.send(Err(UpstrokeError::Refused {
                message: format!(
                    "pipeline {} was stopped while it waited for a snapshot",
                    pipeline.0
                ),
            }));
            self.set_busy(pipeline, Busy::Running);
        }
        self.run.warn(format!(
            "pipeline {} serves {identity:?}, which the fold no longer holds open; it was \
             cancelled and its late result will be discarded",
            pipeline.0
        ));
    }

    fn judged(
        &mut self,
        origin: Origin,
        pipeline: PipelineId,
        identity: &Identity,
        outcome: Result<Box<Judged>, UpstrokeError>,
    ) -> Result<(), UpstrokeError> {
        if !self.accepts(origin, pipeline, identity) {
            return Ok(());
        }
        let judged = match outcome {
            Ok(judged) => judged,
            Err(error) => {
                self.fail(error);
                self.retire(pipeline);
                return Ok(());
            }
        };
        let Some(live) = self.retire(pipeline) else {
            return Ok(());
        };
        let Some(job) = live.job else {
            return Err(UpstrokeError::Refused {
                message: format!(
                    "pipeline {} returned an attempt's completion and holds no attempt",
                    pipeline.0
                ),
            });
        };
        self.run
            .settle_judged(&job, &judged, self.seams, self.hooks)
            .map(drop)
    }

    fn verified_arrived(
        &mut self,
        origin: Origin,
        pipeline: PipelineId,
        identity: &Identity,
        outcome: Result<Judgement, JudgeError>,
        charged: Vec<ReviewRecord>,
    ) {
        if self.in_verify.as_ref().map(|(verifying, _)| *verifying) != Some(pipeline)
            || self.arrived.is_some()
        {
            if origin == Origin::Pipeline
                && self.live.get(&pipeline).is_some_and(|live| live.cancelled)
            {
                self.retire(pipeline);
                self.run.record_discard(None);
                return;
            }
            self.run.record_discard(Some(format!(
                "a verification completion for {identity:?} came from pipeline {}, which no \
                 open verification awaits; it was discarded",
                pipeline.0
            )));
            return;
        }
        if !self.accepts(origin, pipeline, identity) {
            return;
        }
        let Identity::Verification {
            sequence,
            candidate,
        } = identity
        else {
            self.retire(pipeline);
            return;
        };
        self.run.charge_reviews(candidate.key, &charged);
        match verified(outcome, charged, *sequence) {
            Ok(verified) => {
                self.retire(pipeline);
                self.arrived = Some(verified);
            }
            Err(error) => {
                self.fail(error);
                self.retire(pipeline);
            }
        }
    }

    fn halt(&mut self) {
        if self.interrupt.is_none() {
            self.interrupt = Some(Interrupt::Halt);
        }
        self.cancel_all();
    }

    fn fail(&mut self, error: UpstrokeError) {
        if self.interrupt.is_none() {
            self.interrupt = Some(Interrupt::Failed(error));
        } else {
            self.run.warn(format!(
                "after the command had begun to end, a further error: {error}"
            ));
        }
        self.cancel_all();
    }

    fn cancel_all(&mut self) {
        for live in self.live.values_mut() {
            if !live.cancelled {
                live.cancelled = true;
                live.cancel.cancel();
                match &live.identity {
                    Identity::Attempt {
                        key,
                        generation,
                        attempt,
                    } => self.cancelled_work.attempt(*key, *generation, *attempt),
                    Identity::Verification { sequence, .. } => {
                        self.cancelled_work.sequence(*sequence);
                    }
                }
            }
            if matches!(live.busy, Busy::Invoking(_)) {
                live.busy = Busy::Running;
            }
        }
        let (_, invocations) = self.run.broker_mut().halves();
        invocations.withdraw_pending();
        for (invocation, (pipeline, reply)) in std::mem::take(&mut self.replies) {
            let _ = reply.send(Err(cancelled(&invocation)));
            self.set_busy(pipeline, Busy::Running);
        }
        for (pipeline, reply) in std::mem::take(&mut self.gate.waiting) {
            let _ = reply.send(Err(UpstrokeError::Refused {
                message: format!(
                    "pipeline {} was cancelled while it waited for a snapshot",
                    pipeline.0
                ),
            }));
            self.set_busy(pipeline, Busy::Running);
        }
    }

    fn finish(&mut self) -> Result<Progress, UpstrokeError> {
        self.cancel_all();
        while !self.live.is_empty() {
            self.receive_one();
        }
        let unresolved = std::mem::take(&mut self.unresolved);
        match self.interrupt.take() {
            Some(Interrupt::Halt) if unresolved.is_empty() => {
                let cancelled = std::mem::take(&mut self.cancelled_work);
                self.run.close_run(&cancelled, self.seams, self.hooks)
            }
            Some(Interrupt::Halt) => Err(UpstrokeError::Refused {
                message: format!(
                    "the run halted, and the Runner did not establish that the process of {} \
                     ended; closure appends nothing and scrubs nothing over a process that may \
                     still run, so the command ends and the run is resumable: the next \
                     process's census reclaims what is left before its recovery settles the \
                     rest",
                    unresolved.join(", ")
                ),
            }),
            Some(Interrupt::Shutdown) => {
                let reserved = self.run.cancel_provisional();
                Err(UpstrokeError::Refused {
                    message: format!(
                        "the coordinator was shut down: every pending request was withdrawn, \
                         every live pipeline was cancelled and its completion discarded, \
                         {}nothing was settled or appended, and the run is resumable{}",
                        if reserved {
                            "a provisional reservation still held was cancelled, "
                        } else {
                            ""
                        },
                        if unresolved.is_empty() {
                            String::new()
                        } else {
                            format!(
                                "; the Runner did not establish that the process of {} ended, \
                                 and the next process's census reclaims it",
                                unresolved.join(", ")
                            )
                        }
                    ),
                })
            }
            Some(Interrupt::Failed(error)) => {
                if !unresolved.is_empty() {
                    self.run.warn(format!(
                        "the Runner did not establish that the process of {} ended; its \
                         registration and slot pair stay held for the rest of this process",
                        unresolved.join(", ")
                    ));
                }
                Err(error)
            }
            None => Err(UpstrokeError::Refused {
                message: "the coordinator ended with no interrupt recorded".to_owned(),
            }),
        }
    }

    fn join(&mut self) {
        for handle in std::mem::take(&mut self.handles) {
            if let Err(error) = self.runtime.block_on(handle) {
                self.run
                    .warn(format!("a pipeline's thread ended abnormally: {error}"));
            }
        }
    }
}

impl Drop for Coordinator<'_> {
    fn drop(&mut self) {
        for live in self.live.values() {
            live.cancel.cancel();
        }
        self.replies.clear();
        self.gate.waiting.clear();
    }
}

impl Driver for Coordinator<'_> {
    fn parts(&mut self) -> (&mut TopologyRun, &RunSeams<'_>, &mut dyn TopologyHooks) {
        (&mut *self.run, self.seams, &mut *self.hooks)
    }

    fn driven(&self) -> &TopologyRun {
        self.run
    }

    fn seams(&self) -> &RunSeams<'_> {
        self.seams
    }

    fn verify(&mut self, request: &VerifyRequest<'_>) -> Result<Verified, UpstrokeError> {
        self.verify_concurrently(request)
    }
}

// A spawned pipeline's side of the protocol: every message it sends is bound to
// its `PipelineId`, and it waits only for the replies the coordinator owes it.
struct Client {
    pipeline: PipelineId,
    outbox: mpsc::UnboundedSender<ToCoordinator>,
    gated: bool,
}

impl Client {
    fn send(&self, message: ToCoordinator) -> bool {
        self.outbox.send(message).is_ok()
    }

    fn ask(&self, message: impl FnOnce(Reply) -> ToCoordinator) -> Result<(), UpstrokeError> {
        let (reply, answer) = oneshot::channel();
        if !self.send(message(reply)) {
            return Err(closed());
        }
        answer.blocking_recv().unwrap_or_else(|_| Err(closed()))
    }
}

impl Registrar for Client {
    fn admit(
        &self,
        invocation: &InvocationId,
        slots: Option<(SlotPair, Standing)>,
    ) -> Result<(), UpstrokeError> {
        self.ask(|reply| ToCoordinator::Admit {
            pipeline: self.pipeline,
            invocation: invocation.clone(),
            pair: slots.map(|(pair, _)| pair),
            reply,
        })
    }

    fn ended(&self, invocation: &InvocationId, end: InvocationEnd) -> Result<(), UpstrokeError> {
        if self.send(ToCoordinator::Ended {
            pipeline: self.pipeline,
            invocation: invocation.clone(),
            end,
        }) {
            Ok(())
        } else {
            Err(closed())
        }
    }

    fn snapshot_begin(&self) -> Result<(), UpstrokeError> {
        if !self.gated {
            return Ok(());
        }
        self.ask(|reply| ToCoordinator::SnapshotBegin {
            pipeline: self.pipeline,
            reply,
        })
    }

    fn snapshot_end(&self) {
        if self.gated {
            self.send(ToCoordinator::SnapshotEnd {
                pipeline: self.pipeline,
            });
        }
    }
}

fn closed() -> UpstrokeError {
    UpstrokeError::Refused {
        message: "the coordinator stopped listening to this pipeline, which is ending".to_owned(),
    }
}

fn cancelled(invocation: &InvocationId) -> UpstrokeError {
    UpstrokeError::Refused {
        message: format!(
            "`{invocation}` was not started: its pipeline was cancelled while it waited for the \
             coordinator"
        ),
    }
}

fn stuck(described: &str) -> UpstrokeError {
    UpstrokeError::Refused {
        message: format!(
            "every live pipeline is waiting on the coordinator or its observer and the observer \
             released nothing ({described}); the run is resumable"
        ),
    }
}

fn panicked(payload: &(dyn std::any::Any + Send)) -> UpstrokeError {
    let what = payload
        .downcast_ref::<&str>()
        .map(|text| (*text).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "a payload that is not text".to_owned());
    UpstrokeError::Refused {
        message: format!("a pipeline panicked ({what}); its completion is this error"),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::time::Duration;

    use super::*;
    use crate::engine::topology::scaffold::{
        RecordingRunner, Wide, WidePlans, WideTask, wide_responder,
    };
    use crate::events::RunOutcome;
    use crate::runner::invocation::AttemptRole;
    use crate::topology::events::{TopologyEvent, TopologyEventBody};
    use crate::topology::fold::{RunState, TopologyFold};

    const BOUND: Duration = Duration::from_secs(120);

    struct Seeded(u64);

    impl Seeded {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        }

        fn below(&mut self, bound: usize) -> usize {
            let bound = u64::try_from(bound.max(1)).expect("a small bound");
            usize::try_from(self.next() % bound).expect("below a usize bound")
        }
    }

    type Script<'r> = Box<dyn FnMut(&Quiescent<'_>) -> Option<Release> + 'r>;

    enum Order<'r> {
        First,
        Seeded(Seeded),
        Script(Script<'r>),
    }

    struct Scheduler<'r> {
        runner: &'r RecordingRunner,
        order: Order<'r>,
        released: Vec<InvocationId>,
        points: usize,
        widest: usize,
        widest_slotted: usize,
    }

    impl<'r> Scheduler<'r> {
        fn first(runner: &'r RecordingRunner) -> Self {
            Self::with(runner, Order::First)
        }

        fn seeded(runner: &'r RecordingRunner, seed: u64) -> Self {
            Self::with(runner, Order::Seeded(Seeded(seed)))
        }

        fn scripted(runner: &'r RecordingRunner, script: Script<'r>) -> Self {
            Self::with(runner, Order::Script(script))
        }

        fn with(runner: &'r RecordingRunner, order: Order<'r>) -> Self {
            runner.enter_late();
            Self {
                runner,
                order,
                released: Vec::new(),
                points: 0,
                widest: 0,
                widest_slotted: 0,
            }
        }
    }

    impl Quiescence for Scheduler<'_> {
        fn granted(&mut self, invocation: &InvocationId) {
            assert!(
                self.runner.admit(invocation, BOUND),
                "`{invocation}` was granted and did not reach the runner within {BOUND:?}"
            );
        }

        fn quiescent(&mut self, view: &Quiescent<'_>) -> Release {
            for invocation in &view.invoking {
                assert!(
                    self.runner.inside(invocation),
                    "`{invocation}` is granted and not inside the runner at a quiescent point"
                );
            }
            self.points += 1;
            self.widest = self.widest.max(view.invoking.len());
            self.widest_slotted = self.widest_slotted.max(
                view.invoking
                    .iter()
                    .filter(|invocation| crate::engine::topology::identity::is_slotted(invocation))
                    .count(),
            );
            let mut invoking = view.invoking.clone();
            invoking.sort();
            let choice = match &mut self.order {
                Order::First => invoking.first().cloned().map(Release::Invocation),
                Order::Seeded(seeded) => {
                    let index = seeded.below(invoking.len());
                    invoking.get(index).cloned().map(Release::Invocation)
                }
                Order::Script(script) => {
                    script(view).or_else(|| invoking.first().cloned().map(Release::Invocation))
                }
            };
            match choice {
                Some(Release::Invocation(invocation)) => {
                    self.runner
                        .release(&invocation, BOUND)
                        .unwrap_or_else(|error| panic!("releasing `{invocation}`: {error}"));
                    self.released.push(invocation.clone());
                    Release::Invocation(invocation)
                }
                Some(other) => other,
                None => Release::Nothing,
            }
        }
    }

    fn holding(tasks: &[WideTask], failing_gates: &[(u32, u32)]) -> RecordingRunner {
        let runner = RecordingRunner::new().answering(wide_responder(tasks, failing_gates));
        runner.hold();
        runner
    }

    fn drive(
        wide: &mut Wide,
        observer: Option<&mut dyn Quiescence>,
    ) -> Result<Progress, UpstrokeError> {
        let mut hooks = wide.env.hooks();
        let pipelines = wide.env.pipelines();
        wide.run
            .run_concurrently(&wide.env.seams(), &pipelines, &mut hooks, observer)
    }

    fn outcome_of(progress: &Progress) -> RunOutcome {
        match progress {
            Progress::Finished { outcome, .. } => outcome.clone(),
            other => panic!("the run did not finish: {other:?}"),
        }
    }

    fn worker(invocation: &InvocationId) -> Option<TaskKey> {
        match invocation {
            InvocationId::Attempt {
                key,
                role: AttemptRole::Worker,
                ..
            } => Some(*key),
            _ => None,
        }
    }

    fn position(kinds: &[&str], kind: &str, nth: usize) -> usize {
        kinds
            .iter()
            .enumerate()
            .filter(|(_, seen)| **seen == kind)
            .nth(nth)
            .map(|(index, _)| index)
            .unwrap_or_else(|| panic!("no `{kind}` number {nth} in {kinds:?}"))
    }

    fn key_of(value: &serde_json::Value) -> Option<u64> {
        let data = value.get("data")?;
        data.get("key")
            .and_then(serde_json::Value::as_u64)
            .or_else(|| {
                data.get("candidate")
                    .and_then(|candidate| candidate.get("key"))
                    .and_then(serde_json::Value::as_u64)
            })
    }

    fn dispatched_bases(events: &[TopologyEvent]) -> BTreeMap<u32, String> {
        events
            .iter()
            .filter_map(|event| match &event.body {
                TopologyEventBody::TaskDispatched { data } => {
                    Some((data.key.0, data.base_sha.0.clone()))
                }
                _ => None,
            })
            .collect()
    }

    fn merged(events: &[TopologyEvent]) -> Vec<(u32, String)> {
        events
            .iter()
            .filter_map(|event| match &event.body {
                TopologyEventBody::TaskMerged { data } => {
                    Some((data.sequence.0, data.merged_sha.0.clone()))
                }
                _ => None,
            })
            .collect()
    }

    fn canonical(events: &[TopologyEvent], run_id: &str) -> Vec<serde_json::Value> {
        const DROPPED: [&str; 10] = [
            "run_id",
            "incarnation",
            "integration_ref",
            "execution_root",
            "private_dir",
            "upstroke_version",
            "plan_path",
            "config_path",
            "worktree_path",
            "runner",
        ];
        const COMMITS: [&str; 8] = [
            "commit_sha",
            "base_sha",
            "parent_sha",
            "expected_head",
            "proposed_sha",
            "candidate_sha",
            "rejecting_head",
            "merged_sha",
        ];
        fn scrub(
            value: &mut serde_json::Value,
            labels: &mut BTreeMap<String, String>,
            run_id: &str,
        ) {
            match value {
                serde_json::Value::Object(map) => {
                    for dropped in DROPPED {
                        map.remove(dropped);
                    }
                    if let Some(serde_json::Value::Object(limits)) = map.get_mut("limits") {
                        limits.remove("max_parallel");
                    }
                    map.retain(|_, inner| {
                        !matches!(inner, serde_json::Value::String(text) if text.contains(run_id))
                    });
                    for (field, inner) in map.iter_mut() {
                        match inner {
                            serde_json::Value::String(sha) if COMMITS.contains(&field.as_str()) => {
                                let next = format!("commit-{}", labels.len());
                                *sha = labels.entry(sha.clone()).or_insert(next).clone();
                            }
                            _ => scrub(inner, labels, run_id),
                        }
                    }
                }
                serde_json::Value::Array(items) => {
                    for item in items {
                        scrub(item, labels, run_id);
                    }
                }
                _ => {}
            }
        }
        let mut labels = BTreeMap::new();
        events
            .iter()
            .map(|event| {
                let mut value = serde_json::to_value(&event.body).expect("an event serializes");
                scrub(&mut value, &mut labels, run_id);
                value
            })
            .collect()
    }

    fn per_key(projection: &[serde_json::Value]) -> BTreeMap<u64, Vec<serde_json::Value>> {
        let mut keyed: BTreeMap<u64, Vec<serde_json::Value>> = BTreeMap::new();
        for value in projection {
            if let Some(key) = key_of(value) {
                keyed.entry(key).or_default().push(value.clone());
            }
        }
        keyed
    }

    #[test]
    fn two_independent_tasks_overlap_merge_through_one_queue_and_the_dependent_starts_on_both() {
        let tasks = [
            WideTask::independent("alpha"),
            WideTask::independent("beta"),
            WideTask::after("gamma", &["alpha", "beta"]),
        ];
        let runner = holding(&tasks, &[]);
        let mut wide = Wide::started_with(
            "coordinator-acceptance-two",
            &tasks,
            3,
            WidePlans::default(),
            runner,
        );
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let mut first: Option<Vec<InvocationId>> = None;
        let mut scheduler = Scheduler::scripted(
            &runner,
            Box::new(|view: &Quiescent<'_>| {
                if first.is_none() {
                    first = Some(view.invoking.clone());
                }
                None
            }),
        );
        let progress = drive(&mut wide, Some(&mut scheduler)).expect("the run completes");
        drop(scheduler);
        assert_eq!(outcome_of(&progress), RunOutcome::Complete);

        let both: Vec<TaskKey> = first
            .expect("the scheduler saw a quiescent point")
            .iter()
            .filter_map(worker)
            .collect();
        assert_eq!(
            both,
            vec![TaskKey(0), TaskKey(1)],
            "both independent workers were running at once before either completed"
        );

        let events = wide.env.durable_events();
        let kinds: Vec<&str> = events.iter().map(|event| event.body.kind()).collect();
        assert!(
            position(&kinds, "task_dispatched", 1) < position(&kinds, "candidate_prepared", 0),
            "the two independent tasks were dispatched before either settled: {kinds:?}"
        );
        let merged = merged(&events);
        assert_eq!(
            merged
                .iter()
                .map(|(sequence, _)| *sequence)
                .collect::<Vec<_>>(),
            vec![0, 1, 2],
            "one queue, in sequence order"
        );
        assert_eq!(
            kinds
                .iter()
                .filter(|kind| **kind == "merge_verification_started")
                .count(),
            1,
            "the second of the overlapping pair was stale and verified; the first was fast, and \
             the dependent, dispatched on the head holding both, was fast: {kinds:?}"
        );
        let bases = dispatched_bases(&events);
        let after_both = &merged.get(1).expect("the second merge").1;
        assert_eq!(
            bases.get(&2),
            Some(after_both),
            "the dependent was dispatched on the head holding both independent tasks"
        );
        let gamma_worker = wide
            .env
            .runner
            .ran()
            .into_iter()
            .find(|ran| worker(&ran.invocation) == Some(TaskKey(2)))
            .expect("the dependent's worker ran");
        assert_eq!(
            gamma_worker.head_at_spawn.as_ref(),
            Some(after_both),
            "the dependent's worktree held both merged tasks when its worker started"
        );
        let head = wide.env.head(&wide.run);
        for path in [
            "src/alpha/work.txt",
            "src/beta/work.txt",
            "src/gamma/work.txt",
        ] {
            let shown = crate::workspace_manager::fixture::git_out(
                &wide.env.fixture.base,
                &["cat-file", "-e", &format!("{head}:{path}")],
            );
            assert!(shown.status.success(), "the integrated head lacks {path}");
        }
        assert!(wide.run.invocations_balance());
        assert_eq!(wide.run.entitlements_held(), 0);
        assert_eq!(wide.run.discarded(), 0, "{:?}", wide.run.warnings());
    }

    #[test]
    fn disjoint_hints_dispatch_together_overlapping_and_absent_hints_serialize() {
        let cases: [(&str, [WideTask; 2], bool); 3] = [
            (
                "disjoint",
                [
                    WideTask::hinted("alpha", &["src/a/"], "src/a/work.txt"),
                    WideTask::hinted("beta", &["src/b/"], "src/b/work.txt"),
                ],
                true,
            ),
            (
                "overlapping",
                [
                    WideTask::hinted("alpha", &["src/x/"], "src/x/work.txt"),
                    WideTask::hinted("beta", &["src/x/y/"], "src/x/y/work.txt"),
                ],
                false,
            ),
            (
                "absent",
                [
                    WideTask::hinted("alpha", &[], "src/a/work.txt"),
                    WideTask::hinted("beta", &["src/b/"], "src/b/work.txt"),
                ],
                false,
            ),
        ];
        for (case, tasks, together) in cases {
            let runner = holding(&tasks, &[]);
            let mut wide = Wide::started_with(
                &format!("coordinator-hints-{case}"),
                &tasks,
                3,
                WidePlans::default(),
                runner,
            );
            let runner = std::sync::Arc::clone(&wide.env.runner);
            let mut scheduler = Scheduler::first(&runner);
            let progress = drive(&mut wide, Some(&mut scheduler))
                .unwrap_or_else(|error| panic!("{case}: {error}"));
            assert_eq!(outcome_of(&progress), RunOutcome::Complete, "{case}");
            let events = wide.env.durable_events();
            let kinds: Vec<&str> = events.iter().map(|event| event.body.kind()).collect();
            let second_dispatch = position(&kinds, "task_dispatched", 1);
            if together {
                assert!(
                    second_dispatch < position(&kinds, "candidate_prepared", 0),
                    "{case}: both tasks were dispatched before either settled: {kinds:?}"
                );
                assert_eq!(scheduler.widest, 2, "{case}: both workers ran at once");
            } else {
                assert!(
                    second_dispatch > position(&kinds, "task_merged", 0),
                    "{case}: the second task waited for the first's lease to be released: \
                     {kinds:?}"
                );
                assert_eq!(
                    scheduler.widest, 1,
                    "{case}: nothing ran beside the first task"
                );
            }
            assert!(wide.run.invocations_balance(), "{case}");
        }
    }

    fn attempt_key(invocation: &InvocationId) -> Option<u32> {
        match invocation {
            InvocationId::Attempt { key, .. } => Some(key.0),
            _ => None,
        }
    }

    fn role_of(invocation: &InvocationId) -> Option<AttemptRole> {
        match invocation {
            InvocationId::Attempt { role, .. } => Some(*role),
            _ => None,
        }
    }

    fn kinds_of(events: &[TopologyEvent]) -> Vec<&'static str> {
        events.iter().map(|event| event.body.kind()).collect()
    }

    fn count(events: &[TopologyEvent], kind: &str) -> usize {
        events
            .iter()
            .filter(|event| event.body.kind() == kind)
            .count()
    }

    fn released(view: &Quiescent<'_>, wanted: impl Fn(&InvocationId) -> bool) -> Option<Release> {
        let mut invoking = view.invoking.clone();
        invoking.sort();
        invoking
            .into_iter()
            .find(|invocation| wanted(invocation))
            .map(Release::Invocation)
    }

    fn replay_equals_live(wide: &Wide) {
        let replayed = TopologyFold::replay(wide.env.inputs.clone(), &wide.env.durable_events())
            .expect("the log replays");
        assert_eq!(
            replayed.state(),
            wide.run.fold().state(),
            "the live fold and a replay of its own log disagree"
        );
    }

    struct Declining;

    impl crate::interaction::AnswerSource for Declining {
        fn id(&self) -> &'static str {
            "declining"
        }

        fn resolve(
            &self,
            _question: &crate::ir::Question,
        ) -> Result<crate::ir::Answer, UpstrokeError> {
            Ok(crate::ir::Answer::Declined)
        }

        fn poll(
            &self,
            _question: &crate::ir::Question,
        ) -> Result<crate::ir::Answer, UpstrokeError> {
            Ok(crate::ir::Answer::Declined)
        }
    }

    fn forged() -> Box<Judged> {
        Box::new(Judged {
            capture: crate::engine::topology::attempt::Capture {
                tree: "4b825dc642cb6eb9a060e54bf8d69288fbee4904".to_owned(),
                parent: "4b825dc642cb6eb9a060e54bf8d69288fbee4904".to_owned(),
                unresolved: Vec::new(),
            },
            assessed: crate::engine::topology::attempt::Assessment {
                outcome: crate::ir::Outcome {
                    status: crate::ir::OutcomeStatus::Completed,
                    diff: "diff --git a/forged b/forged\n".to_owned(),
                    detail: None,
                    session_id: None,
                    usage: None,
                    cost_usd: None,
                    transcript_path: std::path::PathBuf::new(),
                    duration: Duration::from_millis(1),
                },
                failure: None,
            },
            judgement: Judgement {
                gates: Vec::new(),
                reviews: Vec::new(),
                failure: None,
            },
        })
    }

    struct Folding {
        inner: crate::engine::topology::seams::HarnessTopologyHooks,
        states: Vec<(usize, Option<RunState>)>,
    }

    impl TopologyHooks for Folding {
        fn effects(&mut self) -> &mut dyn crate::workspace_manager::EffectHooks {
            self.inner.effects()
        }

        fn rundir(&mut self) -> &mut dyn crate::rundir::RunDirHooks {
            self.inner.rundir()
        }

        fn events(&mut self) -> &mut dyn crate::events::log::EventHooks {
            self.inner.events()
        }

        fn container(&mut self) -> &mut dyn crate::runner::container::ContainerHooks {
            self.inner.container()
        }

        fn spawn(&mut self) -> &mut dyn crate::agent::proc::SpawnHooks {
            self.inner.spawn()
        }

        fn folded(&mut self, fold: &TopologyFold, events: &[TopologyEvent]) {
            self.inner.folded(fold, events);
            self.states.push((events.len(), fold.state().cloned()));
        }
    }

    fn three() -> [WideTask; 3] {
        [
            WideTask::independent("alpha"),
            WideTask::independent("beta"),
            WideTask::independent("gamma"),
        ]
    }

    const ATTEMPT_LEVEL: [&str; 5] = [
        "task_dispatched",
        "attempt_started",
        "attempt_finished",
        "candidate_prepared",
        "task_candidate_created",
    ];

    fn attempt_level(events: &[TopologyEvent], run_id: &str) -> BTreeMap<u64, Vec<String>> {
        per_key(&canonical(events, run_id))
            .into_iter()
            .map(|(key, values)| {
                let kept = values
                    .into_iter()
                    .filter(|value| {
                        value
                            .get("kind")
                            .and_then(serde_json::Value::as_str)
                            .is_some_and(|kind| ATTEMPT_LEVEL.contains(&kind))
                    })
                    .map(|value| value.to_string())
                    .collect();
                (key, kept)
            })
            .collect()
    }

    fn final_tree(wide: &Wide) -> String {
        let head = wide.env.head(&wide.run);
        crate::workspace_manager::fixture::git(
            &wide.env.fixture.base,
            &["rev-parse", &format!("{head}^{{tree}}")],
        )
    }

    #[test]
    fn the_coordinator_settles_promotes_and_dispatches_while_a_verification_is_open() {
        let tasks = [
            WideTask::independent("alpha"),
            WideTask::independent("beta"),
            WideTask::independent("gamma"),
            WideTask::independent("delta"),
            WideTask::independent("epsilon"),
        ];
        let runner = holding(&tasks, &[]);
        let mut wide = Wide::started_with(
            "coordinator-inside-verify",
            &tasks,
            3,
            WidePlans::default(),
            runner,
        );
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let mut scheduler = Scheduler::scripted(
            &runner,
            Box::new(|view: &Quiescent<'_>| {
                let merges = count(view.run.events(), "task_merged");
                let verifying = view.run.fold().transaction().is_some();
                if merges == 0 {
                    return released(view, |invocation| attempt_key(invocation) == Some(0));
                }
                if merges == 1 && !verifying {
                    return released(view, |invocation| attempt_key(invocation) == Some(1));
                }
                if merges == 1 && verifying {
                    return released(view, |invocation| attempt_key(invocation) == Some(2))
                        .or_else(|| {
                            released(view, |invocation| {
                                attempt_key(invocation) == Some(3)
                                    && role_of(invocation) == Some(AttemptRole::Worker)
                            })
                        })
                        .or_else(|| {
                            released(view, |invocation| {
                                matches!(invocation, InvocationId::Sequence { .. })
                            })
                        })
                        .or_else(|| {
                            released(view, |invocation| attempt_key(invocation) == Some(3))
                        });
                }
                None
            }),
        );
        let progress = drive(&mut wide, Some(&mut scheduler)).expect("the run completes");
        drop(scheduler);
        assert_eq!(outcome_of(&progress), RunOutcome::Complete);

        let events = wide.env.durable_events();
        let kinds = kinds_of(&events);
        let opened = position(&kinds, "merge_verification_started", 0);
        let closed = position(&kinds, "task_merged", 1);
        let inside: Vec<(&str, Option<u64>)> = events
            .get(opened + 1..closed)
            .expect("the verification's span")
            .iter()
            .map(|event| {
                let value = serde_json::to_value(&event.body).expect("serializes");
                (event.body.kind(), key_of(&value))
            })
            .collect();
        assert!(
            inside.contains(&("candidate_prepared", Some(2)))
                && inside.contains(&("task_candidate_created", Some(2))),
            "another task was promoted while the verification was open: {inside:?}"
        );
        assert!(
            inside.contains(&("task_dispatched", Some(4))),
            "a new task was dispatched while the verification was open: {inside:?}"
        );
        assert!(
            !inside
                .iter()
                .any(|(kind, _)| *kind == "merge_verification_started"),
            "no second integration started before the first's terminal: {inside:?}"
        );

        let ran = wide.env.runner.ran();
        let spawned_after = |key: u32, role: AttemptRole| {
            ran.iter()
                .find(|entry| {
                    attempt_key(&entry.invocation) == Some(key)
                        && role_of(&entry.invocation) == Some(role)
                })
                .map(|entry| {
                    entry
                        .durable_at_spawn
                        .iter()
                        .filter(|kind| kind.as_str() == "task_merged")
                        .count()
                })
                .unwrap_or_else(|| panic!("task {key}'s {role:?} ran"))
        };
        assert_eq!(
            spawned_after(3, AttemptRole::Gate(0)),
            1,
            "delta's gate ran on its snapshot while the verification was open"
        );
        assert_eq!(
            spawned_after(3, AttemptRole::ReviewPass(0)),
            2,
            "delta's next snapshot waited until the integration's terminal and reclaim were \
             done: the snapshot gate held it"
        );
        assert!(
            ran.iter().all(|entry| entry.workspace.exists() || true),
            "every invocation ran in a slot"
        );
        assert_eq!(count(&events, "task_merged"), 5, "{kinds:?}");
        assert_eq!(
            count(&events, "attempt_finished"),
            0,
            "no attempt lost its snapshot to the integration's reclaim: {kinds:?}"
        );
        assert!(wide.run.invocations_balance());
        assert_eq!(wide.run.discarded(), 0, "{:?}", wide.run.warnings());
        replay_equals_live(&wide);
    }

    struct Watching {
        inner: crate::engine::topology::seams::HarnessTopologyHooks,
        manager: crate::workspace_manager::WorkspaceManager,
        seen: Vec<(&'static str, Vec<crate::workspace_manager::Slot>)>,
    }

    impl Watching {
        fn over(wide: &Wide) -> Self {
            Self {
                inner: wide.env.hooks(),
                manager: wide.env.fixture.manager.clone(),
                seen: Vec::new(),
            }
        }

        fn intents_when(&self, kind: &str) -> &[crate::workspace_manager::Slot] {
            self.seen
                .iter()
                .find(|(seen, _)| *seen == kind)
                .map(|(_, intents)| intents.as_slice())
                .unwrap_or_else(|| panic!("no `{kind}` was appended"))
        }
    }

    impl TopologyHooks for Watching {
        fn effects(&mut self) -> &mut dyn crate::workspace_manager::EffectHooks {
            self.inner.effects()
        }

        fn rundir(&mut self) -> &mut dyn crate::rundir::RunDirHooks {
            self.inner.rundir()
        }

        fn events(&mut self) -> &mut dyn crate::events::log::EventHooks {
            self.inner.events()
        }

        fn container(&mut self) -> &mut dyn crate::runner::container::ContainerHooks {
            self.inner.container()
        }

        fn spawn(&mut self) -> &mut dyn crate::agent::proc::SpawnHooks {
            self.inner.spawn()
        }

        fn folded(&mut self, fold: &TopologyFold, events: &[TopologyEvent]) {
            self.inner.folded(fold, events);
            if let Some(last) = events.last() {
                let intents = self.manager.intents().expect("the intents are listable");
                self.seen.push((last.body.kind(), intents));
            }
        }
    }

    fn halting_on_gamma(tag: &str) -> Wide {
        let tasks = three();
        let runner = RecordingRunner::new().answering(
            crate::engine::topology::scaffold::wide_responder_asking(&tasks, &[], &[2]),
        );
        runner.hold();
        let mut wide = Wide::started_with(tag, &tasks, 3, WidePlans::default(), runner);
        wide.env.adapters =
            std::sync::Arc::new(crate::engine::topology::scaffold::ScaffoldAdapters::echoing());
        wide.env.answers = std::sync::Arc::new(Declining);
        wide.env.halts_run = true;
        wide
    }

    fn drive_watching(
        wide: &mut Wide,
        observer: &mut dyn Quiescence,
    ) -> (Result<Progress, UpstrokeError>, Watching) {
        let mut watching = Watching::over(wide);
        let pipelines = wide.env.pipelines();
        let progress =
            wide.run
                .run_concurrently(&wide.env.seams(), &pipelines, &mut watching, Some(observer));
        (progress, watching)
    }

    fn snapshot_names(intents: &[crate::workspace_manager::Slot]) -> Vec<String> {
        intents
            .iter()
            .filter_map(|slot| match slot {
                crate::workspace_manager::Slot::Snapshot { name } => Some(name.as_str().to_owned()),
                _ => None,
            })
            .collect()
    }

    fn task_slots(intents: &[crate::workspace_manager::Slot]) -> Vec<String> {
        intents
            .iter()
            .filter_map(|slot| match slot {
                crate::workspace_manager::Slot::Task { key, generation } => {
                    Some(format!("k{key}-g{generation}"))
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn halt_cancels_in_flight_attempt_at_width_three() {
        let mut wide = halting_on_gamma("coordinator-halt-in-flight");
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let mut held_at_halt: Option<Vec<String>> = None;
        let mut scheduler = Scheduler::scripted(
            &runner,
            Box::new(|view: &Quiescent<'_>| {
                let alpha_gating = view.invoking.iter().any(|invocation| {
                    attempt_key(invocation) == Some(0)
                        && matches!(role_of(invocation), Some(AttemptRole::Gate(_)))
                });
                if !alpha_gating {
                    return released(view, |invocation| worker(invocation) == Some(TaskKey(0)));
                }
                if held_at_halt.is_none() {
                    held_at_halt = Some(
                        view.invoking
                            .iter()
                            .filter(|invocation| attempt_key(invocation) != Some(2))
                            .map(InvocationId::render)
                            .collect(),
                    );
                }
                released(view, |invocation| worker(invocation) == Some(TaskKey(2)))
            }),
        );
        let (progress, watching) = drive_watching(&mut wide, &mut scheduler);
        drop(scheduler);
        let progress = progress.expect("the halted run closes and ends");
        assert_eq!(outcome_of(&progress), RunOutcome::Halted);
        let mut held_at_halt = held_at_halt.expect("alpha reached its gate");
        held_at_halt.sort();
        assert_eq!(
            held_at_halt.len(),
            2,
            "alpha's gate and beta's worker were in flight when gamma's decline halted the run: \
             {held_at_halt:?}"
        );

        let events = wide.env.durable_events();
        let kinds = kinds_of(&events);
        let halted = position(&kinds, "question_answered", 0);
        let interrupted: Vec<(u32, u32, String)> = events
            .iter()
            .skip(halted)
            .filter_map(|event| match &event.body {
                TopologyEventBody::AttemptInterrupted { data } => {
                    Some((data.key.0, data.attempt.0, format!("{:?}", data.lease)))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            interrupted,
            vec![
                (0, 1, "PredictedReleased".to_owned()),
                (1, 1, "PredictedReleased".to_owned())
            ],
            "one `attempt_interrupted` per in-flight attempt, after the halt, in key order: \
             {kinds:?}"
        );
        assert!(
            events.iter().any(|event| matches!(&event.body,
                TopologyEventBody::AttemptInterrupted { data } if data.detail.contains("halted"))),
            "the terminal says the run halted"
        );
        assert_eq!(
            kinds.last(),
            Some(&"run_finished"),
            "the closure ends the run: {kinds:?}"
        );
        let TopologyEventBody::RunFinished { data } = &events.last().expect("an end").body else {
            panic!("the last event is the end");
        };
        assert_eq!(
            (data.outcome.clone(), data.halted_at),
            (RunOutcome::Halted, Some(TaskKey(2)))
        );

        let at_end = watching.intents_when("run_finished");
        assert!(
            snapshot_names(at_end).is_empty() && task_slots(at_end).is_empty(),
            "before `run_finished` the closure had reclaimed each interrupted attempt's own \
             snapshot and scrubbed its worktree: {at_end:?}"
        );
        let at_interrupt = watching.intents_when("attempt_interrupted");
        assert!(
            snapshot_names(at_interrupt).contains(&"k0-g0-a1-gates".to_owned()),
            "the terminal is appended before the residue it leaves is reclaimed: {at_interrupt:?}"
        );

        let mut endings = wide.env.runner.endings();
        endings.retain(|(invocation, _)| attempt_key(invocation) != Some(2));
        endings.sort_by(|left, right| left.0.cmp(&right.0));
        assert_eq!(
            endings
                .iter()
                .filter(
                    |(_, ending)| *ending == crate::engine::topology::scaffold::Ending::Cancelled
                )
                .map(|(invocation, _)| invocation.render())
                .collect::<Vec<_>>(),
            held_at_halt,
            "the Runner terminated every in-flight process before the closure ran"
        );
        let ledger = wide.run.broker_mut().invocations();
        assert_eq!(
            (ledger.duplicates(), ledger.balances()),
            (0, true),
            "each in-flight invocation was released once, after its termination"
        );
        assert_eq!(wide.run.entitlements_held(), 0);
        for key in [TaskKey(0), TaskKey(1)] {
            assert_eq!(
                wide.run.fold().task_state(key),
                Some(crate::topology::fold::TaskState::Pending),
                "an interrupted attempt returns its task to Pending"
            );
        }
        replay_equals_live(&wide);
    }

    #[test]
    fn halt_interrupts_verification() {
        let mut wide = halting_on_gamma("coordinator-halt-in-verify");
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let run_id = wide.run.fold().started().expect("started").run_id.clone();
        let pin = crate::engine::topology::integrate::prepared_pin_ref(&run_id, SequenceId(1));
        let manager = wide.env.fixture.manager.clone();
        let mut while_verifying: Option<(Vec<String>, Option<String>)> = None;
        let mut scheduler = Scheduler::scripted(
            &runner,
            Box::new(|view: &Quiescent<'_>| {
                let merges = count(view.run.events(), "task_merged");
                let verifying = view.run.fold().transaction().is_some();
                if merges == 0 {
                    return released(view, |invocation| attempt_key(invocation) == Some(0));
                }
                if !verifying {
                    return released(view, |invocation| attempt_key(invocation) == Some(1));
                }
                if while_verifying.is_none() {
                    let intents = manager.intents().expect("the intents are listable");
                    while_verifying = Some((
                        snapshot_names(&intents),
                        manager
                            .direct_ref_target(pin.as_str())
                            .expect("the pin is readable"),
                    ));
                }
                released(view, |invocation| attempt_key(invocation) == Some(2))
            }),
        );
        let (progress, watching) = drive_watching(&mut wide, &mut scheduler);
        drop(scheduler);
        let progress = progress.expect("the halted run closes and ends");
        assert_eq!(outcome_of(&progress), RunOutcome::Halted);
        let (snapshots, pinned) = while_verifying.expect("the verification was open");
        assert!(
            snapshots.contains(&"s1-integration".to_owned()) && pinned.is_some(),
            "while it was open the verification held its snapshot and its pin: {snapshots:?}"
        );

        let events = wide.env.durable_events();
        let kinds = kinds_of(&events);
        let halted = position(&kinds, "question_answered", 0);
        assert_eq!(
            &kinds[halted..],
            &[
                "question_answered",
                "merge_verification_interrupted",
                "run_finished"
            ],
            "the halt inside `verify` is settled by the closure: {kinds:?}"
        );
        assert!(
            events.iter().any(|event| matches!(&event.body,
                TopologyEventBody::MergeVerificationInterrupted { data }
                    if data.sequence == SequenceId(1) && data.detail.contains("halted"))),
            "{kinds:?}"
        );
        for terminal in [
            "merge_prepared",
            "merge_rejected",
            "merge_verification_unavailable",
        ] {
            assert_eq!(
                count(&events, terminal),
                usize::from(terminal == "merge_prepared"),
                "halting never publishes unverified work; `{terminal}`: {kinds:?}"
            );
        }
        assert_eq!(
            manager
                .direct_ref_target(pin.as_str())
                .expect("the pin is readable"),
            None,
            "the prepared pin was deleted expected-old"
        );
        let at_end = watching.intents_when("run_finished");
        assert!(
            !at_end
                .iter()
                .any(|slot| matches!(slot, crate::workspace_manager::Slot::Staging { .. }))
                && snapshot_names(at_end).is_empty(),
            "before `run_finished` the closure had removed the staging and this sequence's \
             snapshots: {at_end:?}"
        );
        let at_interrupt = watching.intents_when("merge_verification_interrupted");
        assert!(
            snapshot_names(at_interrupt).contains(&"s1-integration".to_owned()),
            "the terminal precedes the reclaim: {at_interrupt:?}"
        );
        let cancelled_sequence =
            wide.env
                .runner
                .endings()
                .into_iter()
                .any(|(invocation, ending)| {
                    matches!(invocation, InvocationId::Sequence { .. })
                        && ending == crate::engine::topology::scaffold::Ending::Cancelled
                });
        assert!(
            cancelled_sequence,
            "the verification's process was terminated before the closure settled it"
        );
        assert!(wide.run.fold().transaction().is_none());
        assert!(wide.run.invocations_balance(), "{:?}", wide.run.warnings());
        assert_eq!(wide.run.broker_duplicates(), 0);
        replay_equals_live(&wide);
    }

    #[test]
    fn stale_duplicate_and_mismatched_completions_are_discarded_with_a_warning_and_counted() {
        let tasks = three();
        let runner = holding(&tasks, &[]);
        let mut wide = Wide::started_with(
            "coordinator-injected",
            &tasks,
            3,
            WidePlans::default(),
            runner,
        );
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let mut injected = 0_u32;
        let mut stage = 0_u32;
        let mut settled: Option<(PipelineId, Identity)> = None;
        let mut seen: BTreeMap<PipelineId, Identity> = BTreeMap::new();
        let mut scheduler = Scheduler::scripted(
            &runner,
            Box::new(|view: &Quiescent<'_>| {
                for (pipeline, identity) in &view.live {
                    seen.insert(*pipeline, identity.clone());
                }
                if stage == 0 {
                    stage = 1;
                    let (alpha, alpha_identity) = view
                        .live
                        .first()
                        .cloned()
                        .expect("the first pipeline is live");
                    let Identity::Attempt {
                        key, generation, ..
                    } = alpha_identity.clone()
                    else {
                        panic!("the first pipeline is an attempt");
                    };
                    let superseded = Identity::Attempt {
                        key,
                        generation,
                        attempt: AttemptNumber(2),
                    };
                    let (beta, beta_identity) =
                        view.live.get(1).cloned().expect("a second pipeline");
                    let messages = vec![
                        ToCoordinator::Judged {
                            pipeline: alpha,
                            identity: Identity::Attempt {
                                key,
                                generation: GenerationId(1),
                                attempt: AttemptNumber(1),
                            },
                            outcome: Ok(forged()),
                        },
                        ToCoordinator::Judged {
                            pipeline: alpha,
                            identity: superseded,
                            outcome: Ok(forged()),
                        },
                        ToCoordinator::Judged {
                            pipeline: PipelineId(999),
                            identity: alpha_identity.clone(),
                            outcome: Ok(forged()),
                        },
                        ToCoordinator::Judged {
                            pipeline: beta,
                            identity: beta_identity,
                            outcome: Ok(forged()),
                        },
                        ToCoordinator::Ended {
                            pipeline: alpha,
                            invocation: AttemptIdentities::new(key, generation, AttemptNumber(1))
                                .worker(),
                            end: InvocationEnd::Completed,
                        },
                        ToCoordinator::Verified {
                            pipeline: alpha,
                            identity: Identity::Verification {
                                sequence: SequenceId(0),
                                candidate: CandidateRef {
                                    key,
                                    generation,
                                    commit_sha: crate::topology::events::CommitSha("0".repeat(40)),
                                    candidate_ref: crate::topology::events::GitRef(
                                        "refs/forged".to_owned(),
                                    ),
                                },
                            },
                            outcome: Box::new(Ok(Judgement {
                                gates: Vec::new(),
                                reviews: Vec::new(),
                                failure: None,
                            })),
                            charged: Vec::new(),
                        },
                    ];
                    for message in messages {
                        assert!(view.injector.inject(message));
                        injected += 1;
                    }
                    return Some(Release::Injected);
                }
                if stage == 1 {
                    let first = seen
                        .first_key_value()
                        .map(|(pipeline, identity)| (*pipeline, identity.clone()));
                    if let Some((pipeline, identity)) = first {
                        let gone = !view.live.iter().any(|(live, _)| *live == pipeline);
                        if gone {
                            settled = Some((pipeline, identity.clone()));
                            stage = 2;
                            let Identity::Attempt {
                                key,
                                generation,
                                attempt,
                            } = identity.clone()
                            else {
                                panic!("an attempt");
                            };
                            assert!(view.injector.inject(ToCoordinator::Judged {
                                pipeline,
                                identity,
                                outcome: Ok(forged()),
                            }));
                            assert!(view.injector.inject(ToCoordinator::Ended {
                                pipeline,
                                invocation:
                                    AttemptIdentities::new(key, generation, attempt).worker(),
                                end: InvocationEnd::Completed,
                            }));
                            injected += 2;
                            return Some(Release::Injected);
                        }
                    }
                }
                if stage == 2 {
                    let beta = view.live.iter().find(|(_, identity)| {
                        matches!(identity, Identity::Attempt { key, .. } if *key == TaskKey(1))
                    });
                    if let Some((pipeline, _)) = beta {
                        let past_its_worker = view.invoking.iter().any(|invocation| {
                            attempt_key(invocation) == Some(1)
                                && role_of(invocation) != Some(AttemptRole::Worker)
                        });
                        if past_its_worker {
                            stage = 3;
                            assert!(
                                view.injector.inject(ToCoordinator::Ended {
                                    pipeline: *pipeline,
                                    invocation: AttemptIdentities::new(
                                        TaskKey(1),
                                        GenerationId(0),
                                        AttemptNumber(1),
                                    )
                                    .worker(),
                                    end: InvocationEnd::Completed,
                                })
                            );
                            return Some(Release::Injected);
                        }
                        return released(view, |invocation| attempt_key(invocation) == Some(1));
                    }
                }
                None
            }),
        );
        let progress = drive(&mut wide, Some(&mut scheduler)).expect("the run completes");
        drop(scheduler);
        assert_eq!(outcome_of(&progress), RunOutcome::Complete);
        assert!(
            settled.is_some(),
            "the duplicate was delivered after its pipeline settled"
        );
        assert_eq!(stage, 3, "the duplicate `Ended` was delivered");
        assert_eq!(
            wide.run.broker_duplicates(),
            1,
            "a second end of an invocation that had ended reached the ledger, which ignored and \
             counted it"
        );
        assert_eq!(
            wide.run.discarded(),
            injected,
            "every injected completion and operation was discarded and counted: {:?}",
            wide.run.warnings()
        );
        assert!(
            wide.run
                .warnings()
                .iter()
                .any(|warning| warning.contains("not live (stale or duplicate)")),
            "{:?}",
            wide.run.warnings()
        );
        let events = wide.env.durable_events();
        assert_eq!(
            count(&events, "candidate_prepared"),
            3,
            "one settlement per attempt, whatever was re-delivered"
        );
        assert_eq!(count(&events, "task_merged"), 3);
        assert!(wide.run.invocations_balance());
        replay_equals_live(&wide);
    }

    #[test]
    fn a_shutdown_cancels_every_live_pipeline_and_ends_the_command_resumably() {
        let tasks = three();
        let runner = holding(&tasks, &[]);
        let mut wide = Wide::started_with(
            "coordinator-shutdown",
            &tasks,
            3,
            WidePlans::default(),
            runner,
        );
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let mut granted: Option<Vec<InvocationId>> = None;
        let mut scheduler = Scheduler::scripted(
            &runner,
            Box::new(|view: &Quiescent<'_>| {
                if granted.is_some() {
                    return None;
                }
                assert_eq!(view.live.len(), 3, "three pipelines are in flight");
                granted = Some(view.invoking.clone());
                assert!(view.injector.inject(ToCoordinator::Shutdown));
                Some(Release::Injected)
            }),
        );
        let error =
            drive(&mut wide, Some(&mut scheduler)).expect_err("a shutdown ends the command");
        drop(scheduler);
        assert!(error.to_string().contains("shut down"), "{error}");
        let events = wide.env.durable_events();
        let kinds = kinds_of(&events);
        assert_eq!(
            kinds.last(),
            Some(&"attempt_started"),
            "nothing is settled or appended by a shutdown: {kinds:?}"
        );
        let mut granted = granted.expect("the shutdown was injected at a quiescent point");
        granted.sort();
        assert_eq!(
            granted.iter().filter_map(worker).collect::<Vec<_>>(),
            vec![TaskKey(0), TaskKey(1), TaskKey(2)],
            "at the shutdown every live pipeline's worker was granted, and nothing else was"
        );
        let mut endings = wide.env.runner.endings();
        endings.sort_by(|left, right| left.0.cmp(&right.0));
        assert_eq!(
            endings,
            granted
                .iter()
                .map(|invocation| (
                    invocation.clone(),
                    crate::engine::topology::scaffold::Ending::Cancelled
                ))
                .collect::<Vec<_>>(),
            "a shutdown \"cancels/releases granted and non-slotted invocations after \
             termination\": every granted worker's process had started and was terminated as a \
             cancellation"
        );
        let ledger = wide.run.broker_mut().invocations();
        assert_eq!(
            (
                ledger.registered(),
                ledger.cancelled(),
                ledger.completed(),
                ledger.duplicates()
            ),
            (3, 3, 0, 0),
            "and each was released once, as a cancellation, when its pipeline reported the end \
             of the process: a release before the termination would make that report a counted \
             duplicate"
        );
        assert_eq!(wide.run.discarded(), 3, "and every completion discarded");
        assert!(wide.run.invocations_balance(), "{:?}", wide.run.warnings());
        replay_equals_live(&wide);
    }

    #[test]
    fn a_shutdown_releases_each_invocation_once_whether_pending_unstarted_running_or_finished() {
        let tasks = [
            WideTask::independent("alpha"),
            WideTask::independent("beta"),
            WideTask::independent("gamma"),
            WideTask::independent("delta"),
        ];
        let worker_of = |key: u32| {
            AttemptIdentities::new(TaskKey(key), GenerationId(0), AttemptNumber(1)).worker()
        };
        let (unstarted, running, finished, pending) =
            (worker_of(0), worker_of(1), worker_of(2), worker_of(3));
        let runner = holding(&tasks, &[]);
        runner.bar(unstarted.clone());
        let mut wide = Wide::started_with(
            "coordinator-shutdown-positions",
            &tasks,
            4,
            WidePlans::default(),
            runner,
        );
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let mut granted: Option<Vec<InvocationId>> = None;
        let mut scheduler = Scheduler::scripted(
            &runner,
            Box::new(|view: &Quiescent<'_>| {
                if granted.is_some() {
                    return None;
                }
                granted = Some(view.invoking.clone());
                runner
                    .release(&finished, BOUND)
                    .unwrap_or_else(|error| panic!("finishing `{finished}`: {error}"));
                assert!(view.injector.inject(ToCoordinator::Shutdown));
                Some(Release::Injected)
            }),
        );
        let mut hooks = wide.env.hooks();
        let pipelines =
            wide.env
                .pipelines_limited(crate::engine::topology::scaffold::SlotLimitsOf::Exactly(
                    3, 3,
                ));
        let error = wide
            .run
            .run_concurrently(
                &wide.env.seams(),
                &pipelines,
                &mut hooks,
                Some(&mut scheduler),
            )
            .expect_err("a shutdown ends the command");
        drop(scheduler);
        assert!(error.to_string().contains("shut down"), "{error}");
        let mut granted = granted.expect("the shutdown was injected at a quiescent point");
        granted.sort();
        assert_eq!(
            granted,
            vec![unstarted.clone(), running.clone(), finished.clone()],
            "three agent slots: three workers granted and the fourth pending at the shutdown"
        );

        let mut endings = wide.env.runner.endings();
        endings.sort_by(|left, right| left.0.cmp(&right.0));
        assert_eq!(
            endings,
            vec![
                (
                    unstarted.clone(),
                    crate::engine::topology::scaffold::Ending::CancelledBeforeStart
                ),
                (
                    running.clone(),
                    crate::engine::topology::scaffold::Ending::Cancelled
                ),
                (
                    finished.clone(),
                    crate::engine::topology::scaffold::Ending::Completed
                ),
            ],
            "the granted worker whose process had not started was cancelled before it started, \
             the running one was terminated, the one that had finished completed, and the pending \
             one was never handed to the runner"
        );
        let started: Vec<InvocationId> = wide
            .env
            .runner
            .ran()
            .into_iter()
            .map(|ran| ran.invocation)
            .collect();
        assert_eq!(
            started,
            vec![running, finished],
            "no process of the unstarted or the pending worker ever started"
        );
        let ledger = wide.run.broker_mut().invocations();
        assert!(
            ledger.settled(&pending) && ledger.settled(&unstarted),
            "the pending request was withdrawn and the unstarted grant released"
        );
        assert_eq!(
            (
                ledger.registered(),
                ledger.completed(),
                ledger.cancelled(),
                ledger.duplicates()
            ),
            (4, 1, 3, 0),
            "each registration was released exactly once: the finished one as a completion, the \
             pending, unstarted and running ones as cancellations"
        );
        assert!(ledger.balances(), "no registration or slot is left held");
        assert_eq!(wide.run.discarded(), 4, "every completion discarded");
        let kinds = kinds_of(&wide.env.durable_events());
        assert_eq!(
            kinds.last(),
            Some(&"attempt_started"),
            "nothing is settled or appended by a shutdown: {kinds:?}"
        );
        replay_equals_live(&wide);
    }

    fn shutdown_at_first_point(runner: &RecordingRunner) -> Scheduler<'_> {
        let mut shut = false;
        Scheduler::scripted(
            runner,
            Box::new(move |view: &Quiescent<'_>| {
                if shut {
                    return None;
                }
                shut = true;
                assert!(view.injector.inject(ToCoordinator::Shutdown));
                Some(Release::Injected)
            }),
        )
    }

    fn kinds_of_log(wide: &Wide) -> Vec<&'static str> {
        kinds_of(&wide.env.durable_events())
    }

    #[test]
    fn a_shutdown_with_pipelines_in_flight_is_settled_by_the_next_resume_whose_ledgers_start_empty()
    {
        let tasks = three();
        let mut wide = Wide::durable(
            "coordinator-shutdown-resume",
            &tasks,
            3,
            WidePlans::default(),
            holding(&tasks, &[]),
        );
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let mut scheduler = shutdown_at_first_point(&runner);
        let error =
            drive(&mut wide, Some(&mut scheduler)).expect_err("a shutdown ends the command");
        drop(scheduler);
        assert!(error.to_string().contains("shut down"), "{error}");
        assert!(
            error
                .to_string()
                .contains("every pending request was withdrawn"),
            "{error}"
        );
        let kinds = kinds_of_log(&wide);
        assert_eq!(
            kinds
                .iter()
                .filter(|kind| **kind == "attempt_started")
                .count(),
            3,
            "{kinds:?}"
        );
        assert_eq!(
            kinds.last(),
            Some(&"attempt_started"),
            "a shutdown appends nothing"
        );
        assert_eq!(
            wide.env
                .runner
                .endings()
                .iter()
                .filter(
                    |(_, ending)| *ending == crate::engine::topology::scaffold::Ending::Cancelled
                )
                .count(),
            3,
            "every granted process was terminated"
        );
        let ledger = wide.run.broker_mut().invocations();
        assert_eq!(
            (
                ledger.registered(),
                ledger.cancelled(),
                ledger.completed(),
                ledger.duplicates()
            ),
            (3, 3, 0, 0),
            "each was released once, after its termination"
        );
        assert!(ledger.balances());
        assert_eq!(
            wide.run.entitlements_held(),
            0,
            "no provisional reservation outlives it"
        );
        assert_eq!(wide.run.discarded(), 3, "every completion discarded");

        let (recovered, mut resumed) = wide
            .resume(
                "inc-2",
                holding(&tasks, &[]),
                crate::engine::topology::select::Ceiling::unlimited(),
            )
            .expect("the next process resumes the shut-down run");
        assert_eq!(
            recovered.interrupted, 3,
            "the resume settles every in-flight identity interrupted"
        );
        assert!(
            resumed.run.invocations_balance()
                && resumed.run.entitlements_held() == 0
                && resumed.run.reservations_peak() == 0
                && resumed.run.broker_duplicates() == 0,
            "the coordinator's ledgers are empty at restart"
        );
        let kinds = kinds_of_log(&resumed);
        let resumed_at = position(&kinds, "run_resumed", 0);
        assert_eq!(
            kinds[resumed_at - 3..resumed_at],
            [
                "attempt_interrupted",
                "attempt_interrupted",
                "attempt_interrupted"
            ],
            "{kinds:?}"
        );
        let runner = std::sync::Arc::clone(&resumed.env.runner);
        let mut scheduler = Scheduler::first(&runner);
        let progress =
            drive(&mut resumed, Some(&mut scheduler)).expect("the resumed run completes");
        drop(scheduler);
        assert_eq!(outcome_of(&progress), RunOutcome::Complete);
        assert_eq!(count(&resumed.env.durable_events(), "task_merged"), 3);
        assert!(resumed.run.invocations_balance());
        replay_equals_live(&resumed);
    }

    fn arming_on(
        runner: &RecordingRunner,
        harness: std::sync::Arc<std::sync::Mutex<crate::topology::effects::HookHarness>>,
        point: crate::topology::effects::SubEffectPoint,
    ) -> Scheduler<'_> {
        Scheduler::scripted(
            runner,
            Box::new(move |view: &Quiescent<'_>| {
                let alpha = released(view, |invocation| attempt_key(invocation) == Some(0))?;
                if let Release::Invocation(invocation) = &alpha {
                    if matches!(role_of(invocation), Some(AttemptRole::ReviewPass(_))) {
                        harness
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .arm(
                                crate::topology::effects::EffectSiteId::Event(
                                    crate::topology::effects::EventSite::Append,
                                ),
                                point,
                                crate::topology::effects::InjectionMode::ErrorReturn,
                            )
                            .expect("the Event append supports an error return here");
                    }
                }
                Some(alpha)
            }),
        )
    }

    #[test]
    fn append_error_under_concurrency_cancels_pipelines_and_folds_nothing_from_memory() {
        use crate::topology::effects::SubEffectPoint;
        let tasks = three();
        for (shape, point, present) in [
            ("partial-write", SubEffectPoint::Written, false),
            ("flush-after-full-line", SubEffectPoint::WrittenFull, true),
            ("sync", SubEffectPoint::Synced, true),
        ] {
            let mut wide = Wide::durable(
                &format!("coordinator-append-error-{shape}"),
                &tasks,
                3,
                WidePlans::default(),
                holding(&tasks, &[]),
            );
            let runner = std::sync::Arc::clone(&wide.env.runner);
            let harness = std::sync::Arc::clone(&wide.env.harness);
            let mut scheduler = arming_on(&runner, harness, point);
            let error = drive(&mut wide, Some(&mut scheduler))
                .expect_err("the append error ends the command");
            drop(scheduler);
            let text = error.to_string();
            assert!(
                text.contains("candidate_prepared") && text.contains("was entered and returned"),
                "{shape}: the protocol's report names the event kind: {text}"
            );
            assert!(
                text.contains(if present {
                    "the proven prefix contains the line"
                } else {
                    "the proven prefix does not contain the line"
                }),
                "{shape}: the report says what the reopened prefix holds: {text}"
            );
            assert!(
                wide.run.fold().is_poisoned(),
                "{shape}: the fold is poisoned"
            );
            assert!(
                matches!(
                    wide.run
                        .fold()
                        .task(TaskKey(0))
                        .and_then(|task| task.generations.first())
                        .map(|generation| &generation.class),
                    Some(crate::topology::fold::GenerationClass::InFlight { .. })
                ),
                "{shape}: nothing was folded from memory: alpha is still in flight in this \
                 process's fold"
            );
            assert!(
                !wide
                    .run
                    .events()
                    .iter()
                    .any(|event| event.body.kind() == "candidate_prepared"),
                "{shape}: the entered append is not in this process's events"
            );
            let durable = kinds_of_log(&wide);
            assert_eq!(
                durable.last(),
                Some(if present {
                    &"candidate_prepared"
                } else {
                    &"attempt_started"
                }),
                "{shape}: nothing was appended after the error, and the surviving prefix is the \
                 one the report names: {durable:?}"
            );
            let cancelled: Vec<u32> = wide
                .env
                .runner
                .endings()
                .into_iter()
                .filter(|(_, ending)| {
                    *ending == crate::engine::topology::scaffold::Ending::Cancelled
                })
                .filter_map(|(invocation, _)| attempt_key(&invocation))
                .collect();
            let mut cancelled = cancelled;
            cancelled.sort_unstable();
            assert_eq!(
                cancelled,
                vec![1, 2],
                "{shape}: the in-flight pipelines were cancelled through the Runner"
            );
            assert_eq!(
                wide.run.discarded(),
                2,
                "{shape}: their completions were discarded, none applied to the poisoned fold"
            );
            let ledger = wide.run.broker_mut().invocations();
            assert!(ledger.balances(), "{shape}");
            assert_eq!(
                ledger.duplicates(),
                2,
                "{shape}: the protocol settled both running registrations at the error, and each \
                 pipeline's own end report after its process terminated was a counted duplicate \
                 that released nothing (R-AI)"
            );
            assert!(
                !wide.env.paths.public.join("report.json").exists(),
                "{shape}: no report from the poisoned fold"
            );
            assert!(
                wide.env.fixture.manager.execution_root().exists(),
                "{shape}: and no cleanup"
            );

            let (recovered, mut resumed) = wide
                .resume(
                    "inc-2",
                    holding(&tasks, &[]),
                    crate::engine::topology::select::Ceiling::unlimited(),
                )
                .unwrap_or_else(|error| panic!("{shape}: the next process resumes: {error}"));
            assert_eq!(
                (recovered.interrupted, recovered.finished.clone()),
                if present {
                    (2, vec![TaskKey(0)])
                } else {
                    (3, Vec::new())
                },
                "{shape}: the resume follows the surviving prefix: a present line is a promotion \
                 it completes, an absent one leaves alpha in flight to settle interrupted"
            );
            let runner = std::sync::Arc::clone(&resumed.env.runner);
            let mut scheduler = Scheduler::first(&runner);
            let progress = drive(&mut resumed, Some(&mut scheduler))
                .unwrap_or_else(|error| panic!("{shape}: the resumed run completes: {error}"));
            drop(scheduler);
            assert_eq!(outcome_of(&progress), RunOutcome::Complete, "{shape}");
            assert_eq!(
                count(&resumed.env.durable_events(), "task_merged"),
                3,
                "{shape}"
            );
            replay_equals_live(&resumed);
        }
    }

    struct HaltArming {
        inner: crate::engine::topology::seams::HarnessTopologyHooks,
        harness: std::sync::Arc<std::sync::Mutex<crate::topology::effects::HookHarness>>,
        point: crate::topology::effects::SubEffectPoint,
        mode: crate::topology::effects::InjectionMode,
        armed: bool,
    }

    impl HaltArming {
        fn over(
            wide: &Wide,
            point: crate::topology::effects::SubEffectPoint,
            mode: crate::topology::effects::InjectionMode,
        ) -> Self {
            Self {
                inner: wide.env.hooks(),
                harness: std::sync::Arc::clone(&wide.env.harness),
                point,
                mode,
                armed: false,
            }
        }
    }

    impl TopologyHooks for HaltArming {
        fn effects(&mut self) -> &mut dyn crate::workspace_manager::EffectHooks {
            self.inner.effects()
        }

        fn rundir(&mut self) -> &mut dyn crate::rundir::RunDirHooks {
            self.inner.rundir()
        }

        fn events(&mut self) -> &mut dyn crate::events::log::EventHooks {
            self.inner.events()
        }

        fn container(&mut self) -> &mut dyn crate::runner::container::ContainerHooks {
            self.inner.container()
        }

        fn spawn(&mut self) -> &mut dyn crate::agent::proc::SpawnHooks {
            self.inner.spawn()
        }

        fn folded(&mut self, fold: &TopologyFold, events: &[TopologyEvent]) {
            self.inner.folded(fold, events);
            if !self.armed && fold.halted_at().is_some() {
                self.armed = true;
                self.harness
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .arm(
                        crate::topology::effects::EffectSiteId::Event(
                            crate::topology::effects::EventSite::Append,
                        ),
                        self.point,
                        self.mode,
                    )
                    .expect("the Event append supports the mode at the point");
            }
        }
    }

    fn durable_halting_on_gamma(tag: &str) -> Wide {
        let tasks = three();
        let runner = RecordingRunner::new().answering(
            crate::engine::topology::scaffold::wide_responder_asking(&tasks, &[], &[2]),
        );
        runner.hold();
        let mut wide = Wide::durable(tag, &tasks, 3, WidePlans::default(), runner);
        halting(&mut wide.env);
        wide
    }

    fn halting(env: &mut crate::engine::topology::scaffold::WideEnv) {
        env.adapters =
            std::sync::Arc::new(crate::engine::topology::scaffold::ScaffoldAdapters::echoing());
        env.answers = std::sync::Arc::new(Declining);
        env.halts_run = true;
    }

    fn gamma_first(runner: &RecordingRunner) -> Scheduler<'_> {
        Scheduler::scripted(
            runner,
            Box::new(|view: &Quiescent<'_>| {
                released(view, |invocation| worker(invocation) == Some(TaskKey(2)))
            }),
        )
    }

    fn finish_resumed_halted(
        wide: Wide,
        tag: &str,
    ) -> (crate::engine::topology::recover::Recovered, Wide) {
        let tasks = three();
        let (recovered, mut resumed) = wide
            .resume(
                "inc-2",
                RecordingRunner::new().answering(wide_responder(&tasks, &[])),
                crate::engine::topology::select::Ceiling::unlimited(),
            )
            .unwrap_or_else(|error| panic!("{tag}: the next process resumes: {error}"));
        let progress = drive(&mut resumed, None).unwrap_or_else(|error| panic!("{tag}: {error}"));
        assert_eq!(
            outcome_of(&progress),
            RunOutcome::Halted,
            "{tag}: the next process repeats the closure from the surviving prefix"
        );
        (recovered, resumed)
    }

    fn ends_once_halted(wide: &Wide, tag: &str) {
        let events = wide.env.durable_events();
        let ends: Vec<&TopologyEvent> = events
            .iter()
            .filter(|event| event.body.kind() == "run_finished")
            .collect();
        assert_eq!(ends.len(), 1, "{tag}: exactly one end");
        let kinds = kinds_of(&events);
        let resumed = position(&kinds, "run_resumed", 0);
        let interrupted = kinds
            .iter()
            .filter(|kind| **kind == "attempt_interrupted")
            .count();
        assert_eq!(
            interrupted, 2,
            "{tag}: alpha and beta are settled interrupted once each, across the two processes: \
             {kinds:?}"
        );
        assert!(
            kinds[resumed + 1..] == ["run_finished"],
            "{tag}: after the resume the loop only closes: {kinds:?}"
        );
        replay_equals_live(wide);
    }

    #[test]
    fn append_error_inside_closure_ends_command_and_resume_completes_closure_at_width_three() {
        use crate::topology::effects::{InjectionMode, SubEffectPoint};
        for (shape, point, survived) in [
            ("partial-write", SubEffectPoint::Written, 0_usize),
            ("sync", SubEffectPoint::Synced, 1),
        ] {
            let mut wide = durable_halting_on_gamma(&format!("coordinator-closure-error-{shape}"));
            let runner = std::sync::Arc::clone(&wide.env.runner);
            let mut scheduler = gamma_first(&runner);
            let mut hooks = HaltArming::over(&wide, point, InjectionMode::ErrorReturn);
            let pipelines = wide.env.pipelines();
            let error = wide
                .run
                .run_concurrently(
                    &wide.env.seams(),
                    &pipelines,
                    &mut hooks,
                    Some(&mut scheduler),
                )
                .expect_err("the closure's append error ends the command");
            drop(scheduler);
            assert!(
                error.to_string().contains("attempt_interrupted"),
                "{shape}: the protocol names the closure's first append: {error}"
            );
            assert!(wide.run.fold().is_poisoned(), "{shape}");
            let kinds = kinds_of_log(&wide);
            assert_eq!(
                kinds
                    .iter()
                    .filter(|kind| **kind == "attempt_interrupted")
                    .count(),
                survived,
                "{shape}: the surviving prefix is the one the report names: {kinds:?}"
            );
            assert!(
                !kinds.contains(&"run_finished"),
                "{shape}: nothing after the error"
            );
            assert!(
                !wide.env.paths.public.join("report.json").exists(),
                "{shape}: no report from the poisoned fold"
            );
            let (recovered, resumed) = finish_resumed_halted(wide, shape);
            assert_eq!(
                recovered.interrupted,
                2 - survived,
                "{shape}: recovery settles what the surviving prefix leaves in flight"
            );
            ends_once_halted(&resumed, shape);
        }
    }

    const KILL_HANDOFF: &str = "wide-fixture-root";

    #[test]
    #[ignore = "spawned by kill_inside_closure_recovers_at_width_three"]
    fn closure_kill_child_at_width_three() {
        use crate::topology::effects::{InjectionMode, SubEffectPoint};
        let dir = std::path::PathBuf::from(
            std::env::var("UPSTROKE_TEST_KILL_DIR").expect("the parent names the handoff"),
        );
        let shape = match std::env::var("UPSTROKE_TEST_KILL_SHAPE").as_deref() {
            Ok("torn") => crate::events::log::WrittenShape::Torn,
            Ok("complete") => crate::events::log::WrittenShape::Complete,
            other => panic!("the parent names the kill shape: {other:?}"),
        };
        let mut wide = durable_halting_on_gamma("coordinator-closure-kill");
        crate::workspace_manager::fixture::write_file(
            &dir.join(KILL_HANDOFF),
            wide.env.fixture.root.to_string_lossy().as_bytes(),
        );
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let mut scheduler = gamma_first(&runner);
        let mut hooks = HaltArming {
            inner: wide.env.hooks().with_written_kill_shape(shape),
            ..HaltArming::over(&wide, SubEffectPoint::Written, InjectionMode::Kill)
        };
        let pipelines = wide.env.pipelines();
        let _ = wide.run.run_concurrently(
            &wide.env.seams(),
            &pipelines,
            &mut hooks,
            Some(&mut scheduler),
        );
        panic!("the kill inside the closure must have taken this process");
    }

    #[test]
    fn kill_inside_closure_recovers_at_width_three() {
        for (shape, survived) in [("torn", 0_usize), ("complete", 1)] {
            let handoff = crate::engine::topology::scaffold::kill_dir(&format!(
                "coordinator-closure-kill-{shape}"
            ));
            let status = crate::workspace_manager::fixture::run_kill_child_within(
                "engine::topology::coordinator::tests::closure_kill_child_at_width_three",
                &[
                    ("UPSTROKE_TEST_KILL_DIR", handoff.path().as_os_str()),
                    ("UPSTROKE_TEST_KILL_SHAPE", std::ffi::OsStr::new(shape)),
                ],
                crate::engine::topology::scaffold::KILL_CHILD_BOUND,
            )
            .unwrap_or_else(|| panic!("{shape}: the kill child did not end in time"));
            assert!(
                crate::workspace_manager::fixture::died_by_abort(&status),
                "{shape}: the child died inside the closure: {status:?}"
            );
            let root = std::fs::read_to_string(handoff.path().join(KILL_HANDOFF))
                .unwrap_or_else(|error| panic!("{shape}: the child left no handoff: {error}"));
            let mut env = crate::engine::topology::scaffold::WideEnv::adopted(
                std::path::PathBuf::from(root),
                &three(),
                3,
                WidePlans::default(),
            );
            halting(&mut env);
            let kinds: Vec<&str> = kinds_of(&env.durable_events());
            assert!(
                kinds.contains(&"question_answered") && !kinds.contains(&"run_finished"),
                "{shape}: the child halted and died before the end: {kinds:?}"
            );
            assert_eq!(
                kinds
                    .iter()
                    .filter(|kind| **kind == "attempt_interrupted")
                    .count(),
                survived,
                "{shape}: {kinds:?}"
            );
            let tasks = three();
            let (recovered, resumed) = env
                .resume(
                    "inc-2",
                    RecordingRunner::new().answering(wide_responder(&tasks, &[])),
                    crate::engine::topology::select::Ceiling::unlimited(),
                )
                .unwrap_or_else(|error| panic!("{shape}: the next process resumes: {error}"));
            assert_eq!(recovered.interrupted, 2 - survived, "{shape}");
            let mut resumed = resumed;
            let progress =
                drive(&mut resumed, None).unwrap_or_else(|error| panic!("{shape}: {error}"));
            assert_eq!(outcome_of(&progress), RunOutcome::Halted, "{shape}");
            ends_once_halted(&resumed, shape);
        }
    }

    struct FailingOnce {
        effects: crate::workspace_manager::HarnessEffects,
        fail_at: Option<(crate::topology::effects::EffectSiteId, u32)>,
    }

    impl crate::workspace_manager::EffectHooks for FailingOnce {
        fn phase(
            &mut self,
            site: crate::topology::effects::EffectSiteId,
            phase: crate::topology::effects::HookPhase,
        ) -> crate::topology::effects::Injection {
            let answer = self.effects.phase(site, phase);
            if phase != crate::topology::effects::HookPhase::Before {
                return answer;
            }
            match self.fail_at {
                Some((at, 0)) if at == site => {
                    self.fail_at = None;
                    crate::topology::effects::Injection::Error
                }
                Some((at, skip)) if at == site => {
                    self.fail_at = Some((at, skip - 1));
                    answer
                }
                _ => answer,
            }
        }

        fn durability_ledger(&self) -> crate::util::DurabilityLedger {
            self.effects.durability_ledger()
        }

        fn refusal_cause(&self) -> Option<String> {
            self.effects.refusal_cause()
        }
    }

    struct CasFailing {
        inner: crate::engine::topology::seams::HarnessTopologyHooks,
        effects: FailingOnce,
    }

    impl CasFailing {
        fn over(wide: &Wide) -> Self {
            Self {
                inner: wide.env.hooks(),
                effects: FailingOnce {
                    effects: crate::workspace_manager::HarnessEffects::new(std::sync::Arc::clone(
                        &wide.env.harness,
                    )),
                    fail_at: None,
                },
            }
        }

        fn fail_the_next(&mut self, site: crate::topology::effects::RefSite) {
            self.fail_after(site, 0);
        }

        fn fail_after(&mut self, site: crate::topology::effects::RefSite, passing: u32) {
            self.effects.fail_at =
                Some((crate::topology::effects::EffectSiteId::Ref(site), passing));
        }
    }

    impl TopologyHooks for CasFailing {
        fn effects(&mut self) -> &mut dyn crate::workspace_manager::EffectHooks {
            &mut self.effects
        }

        fn rundir(&mut self) -> &mut dyn crate::rundir::RunDirHooks {
            self.inner.rundir()
        }

        fn events(&mut self) -> &mut dyn crate::events::log::EventHooks {
            self.inner.events()
        }

        fn container(&mut self) -> &mut dyn crate::runner::container::ContainerHooks {
            self.inner.container()
        }

        fn spawn(&mut self) -> &mut dyn crate::agent::proc::SpawnHooks {
            self.inner.spawn()
        }
    }

    fn steps_to(
        wide: &mut Wide,
        hooks: &mut CasFailing,
        wanted: impl Fn(&Progress) -> bool,
        what: &str,
    ) -> Progress {
        let seams = wide.env.seams();
        let progress = wide
            .run
            .step(&seams, hooks)
            .unwrap_or_else(|error| panic!("{what}: {error}"));
        assert!(wanted(&progress), "{what}: {progress:?}");
        progress
    }

    fn halted_by_the_last_task(tag: &str, tasks: &[WideTask]) -> Wide {
        let asking = u32::try_from(tasks.len() - 1).expect("a small plan");
        let runner = RecordingRunner::new().answering(
            crate::engine::topology::scaffold::wide_responder_asking(tasks, &[], &[asking]),
        );
        let mut wide = Wide::started_with(tag, tasks, 3, WidePlans::default(), runner);
        halting(&mut wide.env);
        wide
    }

    fn after_the_halt(wide: &Wide) -> Vec<&'static str> {
        let kinds = kinds_of(&wide.env.durable_events());
        kinds[position(&kinds, "question_answered", 0)..].to_vec()
    }

    #[test]
    fn authorized_publication_completed_at_run_end() {
        let tasks = [
            WideTask::independent("alpha"),
            WideTask::independent("beta"),
        ];
        let mut wide = halted_by_the_last_task("coordinator-fast-at-end", &tasks);
        let mut hooks = CasFailing::over(&wide);
        steps_to(
            &mut wide,
            &mut hooks,
            |progress| {
                matches!(
                    progress,
                    Progress::Settled {
                        key: TaskKey(0),
                        accepted: true,
                        ..
                    }
                )
            },
            "alpha settles",
        );
        hooks.fail_the_next(crate::topology::effects::RefSite::CompareAndSwapIntegration);
        let seams = wide.env.seams();
        let error = wide
            .run
            .step(&seams, &mut hooks)
            .expect_err("the fast integration's CAS is made to fail once");
        assert!(
            error.to_string().contains("CompareAndSwapIntegration"),
            "{error}"
        );
        assert!(
            matches!(
                wide.run.fold().transaction().map(|open| &open.class),
                Some(crate::topology::fold::TransactionClass::Prepared {
                    disposition: crate::topology::events::PreparedDisposition::Fast,
                    ..
                })
            ),
            "`merge_prepared(fast)` is durable and `task_merged` is not: the state no \
             coordinator schedule reaches at a live end (R-AG)"
        );
        let before = wide.env.head(&wide.run);
        steps_to(
            &mut wide,
            &mut hooks,
            |progress| {
                matches!(
                    progress,
                    Progress::Settled {
                        key: TaskKey(1),
                        accepted: false,
                        ..
                    }
                )
            },
            "beta parks",
        );
        steps_to(
            &mut wide,
            &mut hooks,
            |progress| matches!(progress, Progress::Answered { declined: true, .. }),
            "the decline is ingested",
        );
        let end = steps_to(
            &mut wide,
            &mut hooks,
            |progress| matches!(progress, Progress::Finished { .. }),
            "the closure ends the run",
        );
        assert_eq!(outcome_of(&end), RunOutcome::Halted);
        assert_eq!(
            after_the_halt(&wide),
            ["question_answered", "task_merged", "run_finished"],
            "the closure completed the authorized publication before it ended the run"
        );
        let after = wide.env.head(&wide.run);
        assert_ne!(after, before, "the CAS moved the integration ref");
        assert!(wide.run.fold().transaction().is_none());
        assert!(wide.run.invocations_balance());
        replay_equals_live(&wide);
    }

    #[test]
    fn prepared_publication_completed_at_run_end() {
        let tasks = three();
        let armed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let runner = RecordingRunner::new().answering(
            crate::engine::topology::scaffold::wide_responder_asking(&tasks, &[], &[2]),
        );
        runner.hold();
        let mut wide = Wide::started_with(
            "coordinator-prepared-at-end",
            &tasks,
            3,
            WidePlans::default(),
            runner,
        );
        halting(&mut wide.env);
        wide.env.answers = std::sync::Arc::new(DecliningOnceArmed(std::sync::Arc::clone(&armed)));
        let mut hooks = CasFailing::over(&wide);
        hooks.fail_after(
            crate::topology::effects::RefSite::CompareAndSwapIntegration,
            1,
        );
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let mut scheduler = Scheduler::scripted(
            &runner,
            Box::new(|view: &Quiescent<'_>| {
                released(view, |invocation| worker(invocation) == Some(TaskKey(2)))
                    .or_else(|| released(view, |invocation| attempt_key(invocation) == Some(0)))
                    .or_else(|| released(view, |invocation| attempt_key(invocation) == Some(1)))
            }),
        );
        let pipelines = wide.env.pipelines();
        let error = wide
            .run
            .run_concurrently(
                &wide.env.seams(),
                &pipelines,
                &mut hooks,
                Some(&mut scheduler),
            )
            .expect_err("beta's stale integration verifies, prepares, and its CAS fails once");
        drop(scheduler);
        assert!(
            error.to_string().contains("CompareAndSwapIntegration"),
            "{error}"
        );
        let run_id = wide.run.fold().started().expect("started").run_id.clone();
        let pin = crate::engine::topology::integrate::prepared_pin_ref(&run_id, SequenceId(1));
        let manager = wide.env.fixture.manager.clone();
        assert!(
            matches!(
                wide.run.fold().transaction().map(|open| &open.class),
                Some(crate::topology::fold::TransactionClass::Prepared {
                    disposition: crate::topology::events::PreparedDisposition::StaleClean,
                    ..
                })
            ),
            "a stale-clean `merge_prepared` is durable and its publication is pending: {:?}",
            wide.run.fold().transaction()
        );
        let proposed = manager
            .direct_ref_target(pin.as_str())
            .expect("the pin is readable")
            .expect("the verified proposal is pinned");
        assert!(
            manager.intents().expect("intents").contains(
                &crate::engine::topology::integrate::staging_slot(SequenceId(1))
            ),
            "and its staging is still there"
        );

        armed.store(true, std::sync::atomic::Ordering::SeqCst);
        steps_to(
            &mut wide,
            &mut hooks,
            |progress| matches!(progress, Progress::Answered { declined: true, .. }),
            "gamma's decline is ingested",
        );
        let end = steps_to(
            &mut wide,
            &mut hooks,
            |progress| matches!(progress, Progress::Finished { .. }),
            "the closure ends the run",
        );
        assert_eq!(outcome_of(&end), RunOutcome::Halted);
        assert_eq!(
            after_the_halt(&wide),
            ["question_answered", "task_merged", "run_finished"],
            "the closure published the verified proposal before it ended the run"
        );
        assert_eq!(
            wide.env.head(&wide.run),
            proposed,
            "the CAS put the pinned proposal there"
        );
        assert_eq!(
            manager
                .direct_ref_target(pin.as_str())
                .expect("the pin is readable"),
            None,
            "and the pin was pruned"
        );
        replay_equals_live(&wide);
    }

    #[test]
    fn promoting_completed_by_the_closure_at_run_end() {
        let tasks = [
            WideTask::independent("alpha"),
            WideTask::independent("beta"),
        ];
        let mut wide = halted_by_the_last_task("coordinator-promoting-at-end", &tasks);
        let mut hooks = CasFailing::over(&wide);
        hooks.fail_the_next(crate::topology::effects::RefSite::CreateCandidates);
        let seams = wide.env.seams();
        let error = wide
            .run
            .step(&seams, &mut hooks)
            .expect_err("alpha's candidates ref is made to fail once");
        assert!(error.to_string().contains("CreateCandidates"), "{error}");
        assert!(
            matches!(
                wide.run
                    .fold()
                    .task(TaskKey(0))
                    .and_then(|task| task.generations.first())
                    .map(|generation| &generation.class),
                Some(crate::topology::fold::GenerationClass::Promoting)
            ),
            "`candidate_prepared` is durable and `task_candidate_created` is not"
        );
        steps_to(
            &mut wide,
            &mut hooks,
            |progress| {
                matches!(
                    progress,
                    Progress::Settled {
                        key: TaskKey(1),
                        accepted: false,
                        ..
                    }
                )
            },
            "beta parks",
        );
        steps_to(
            &mut wide,
            &mut hooks,
            |progress| matches!(progress, Progress::Answered { declined: true, .. }),
            "the decline is ingested",
        );
        let end = steps_to(
            &mut wide,
            &mut hooks,
            |progress| matches!(progress, Progress::Finished { .. }),
            "the closure ends the run",
        );
        assert_eq!(outcome_of(&end), RunOutcome::Halted);
        assert_eq!(
            after_the_halt(&wide),
            [
                "question_answered",
                "task_candidate_created",
                "run_finished"
            ],
            "the closure completed the promotion before it ended the run"
        );
        let run_id = wide.run.fold().started().expect("started").run_id.clone();
        let names = crate::engine::topology::candidate::CandidateNames::of(
            &run_id,
            TaskKey(0),
            GenerationId(0),
        );
        let manager = wide.env.fixture.manager.clone();
        assert!(
            manager
                .direct_ref_target(names.candidate_ref.as_str())
                .expect("the candidates ref is readable")
                .is_some(),
            "the candidates ref was created, and Halted retains it"
        );
        assert_eq!(
            manager
                .direct_ref_target(names.prepared_ref.as_str())
                .expect("the pin is readable"),
            None,
            "the candidate-prepared pin was pruned"
        );
        replay_equals_live(&wide);
    }

    struct ArmsOnRetained {
        inner: crate::engine::topology::seams::HarnessTopologyHooks,
        armed: std::sync::Arc<std::sync::atomic::AtomicBool>,
    }

    impl TopologyHooks for ArmsOnRetained {
        fn effects(&mut self) -> &mut dyn crate::workspace_manager::EffectHooks {
            self.inner.effects()
        }

        fn rundir(&mut self) -> &mut dyn crate::rundir::RunDirHooks {
            self.inner.rundir()
        }

        fn events(&mut self) -> &mut dyn crate::events::log::EventHooks {
            self.inner.events()
        }

        fn container(&mut self) -> &mut dyn crate::runner::container::ContainerHooks {
            self.inner.container()
        }

        fn spawn(&mut self) -> &mut dyn crate::agent::proc::SpawnHooks {
            self.inner.spawn()
        }

        fn folded(&mut self, fold: &TopologyFold, events: &[TopologyEvent]) {
            self.inner.folded(fold, events);
            let retained = events.last().is_some_and(|event| {
                matches!(&event.body, TopologyEventBody::AttemptFinished { data }
                    if matches!(data.settlement,
                        crate::topology::events::AttemptSettlement::Retained { .. }))
            });
            if retained {
                self.armed.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        }
    }

    struct DecliningOnceArmed(std::sync::Arc<std::sync::atomic::AtomicBool>);

    impl crate::interaction::AnswerSource for DecliningOnceArmed {
        fn id(&self) -> &'static str {
            "declining-once-armed"
        }

        fn resolve(
            &self,
            question: &crate::ir::Question,
        ) -> Result<crate::ir::Answer, UpstrokeError> {
            self.poll(question)
        }

        fn poll(
            &self,
            _question: &crate::ir::Question,
        ) -> Result<crate::ir::Answer, UpstrokeError> {
            Ok(if self.0.load(std::sync::atomic::Ordering::SeqCst) {
                crate::ir::Answer::Declined
            } else {
                crate::ir::Answer::Unanswered
            })
        }
    }

    fn generation_closed_run_ending(events: &[TopologyEvent]) -> Vec<(u32, RunOutcome)> {
        events
            .iter()
            .filter_map(|event| match &event.body {
                TopologyEventBody::GenerationClosed { data } => match &data.reason {
                    crate::topology::events::GenerationCloseReason::RunEnding { outcome } => {
                        Some((data.key.0, outcome.clone()))
                    }
                    _ => None,
                },
                _ => None,
            })
            .collect()
    }

    #[test]
    fn retained_generation_closed_at_run_end_at_width_three() {
        let tasks = three();
        let armed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let runner = RecordingRunner::new().answering(
            crate::engine::topology::scaffold::wide_responder_asking(&tasks, &[(0, 1)], &[2]),
        );
        runner.hold();
        let mut wide = Wide::started_with(
            "coordinator-retained-halted",
            &tasks,
            3,
            WidePlans::default(),
            runner,
        );
        halting(&mut wide.env);
        wide.env.answers = std::sync::Arc::new(DecliningOnceArmed(std::sync::Arc::clone(&armed)));
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let mut scheduler = Scheduler::scripted(
            &runner,
            Box::new(|view: &Quiescent<'_>| {
                released(view, |invocation| worker(invocation) == Some(TaskKey(2)))
                    .or_else(|| released(view, |invocation| attempt_key(invocation) == Some(0)))
            }),
        );
        let mut hooks = ArmsOnRetained {
            inner: wide.env.hooks(),
            armed,
        };
        let pipelines = wide.env.pipelines();
        let progress = wide
            .run
            .run_concurrently(
                &wide.env.seams(),
                &pipelines,
                &mut hooks,
                Some(&mut scheduler),
            )
            .expect("the halt ends the run");
        drop(scheduler);
        assert_eq!(outcome_of(&progress), RunOutcome::Halted);
        assert_eq!(
            after_the_halt(&wide),
            [
                "question_answered",
                "attempt_interrupted",
                "generation_closed",
                "run_finished"
            ],
            "beta, in flight, is interrupted and alpha's retained generation closed run-ending"
        );
        assert_eq!(
            generation_closed_run_ending(&wide.env.durable_events()),
            vec![(0, RunOutcome::Halted)]
        );
        replay_equals_live(&wide);

        let mut wide = Wide::started_under(
            "coordinator-retained-budget",
            &tasks,
            3,
            WidePlans::default(),
            holding(&tasks, &[(0, 1)]),
            crate::engine::topology::select::Ceiling {
                run_usd: Some(0.2),
                task_usd: None,
            },
        );
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let mut scheduler = Scheduler::scripted(
            &runner,
            Box::new(|view: &Quiescent<'_>| {
                released(view, |invocation| attempt_key(invocation) == Some(0))
            }),
        );
        let progress =
            drive(&mut wide, Some(&mut scheduler)).expect("the budget stop ends the run");
        drop(scheduler);
        assert_eq!(outcome_of(&progress), RunOutcome::BudgetExceeded);
        let events = wide.env.durable_events();
        let kinds = kinds_of(&events);
        let stop = position(&kinds, "budget_exceeded", 0);
        assert!(
            kinds[..stop].contains(&"attempt_finished")
                && !kinds[stop..].contains(&"attempt_started"),
            "the retry the retained generation was ready for is what the ceiling refused: {kinds:?}"
        );
        assert_eq!(
            generation_closed_run_ending(&events),
            vec![(0, RunOutcome::BudgetExceeded)],
            "{kinds:?}"
        );
        replay_equals_live(&wide);
    }

    struct Recording {
        inner: crate::engine::topology::seams::HarnessTopologyHooks,
        folds: Vec<(&'static str, TopologyFold)>,
    }

    impl Recording {
        fn over(wide: &Wide) -> Self {
            Self {
                inner: wide.env.hooks(),
                folds: Vec::new(),
            }
        }
    }

    impl TopologyHooks for Recording {
        fn effects(&mut self) -> &mut dyn crate::workspace_manager::EffectHooks {
            self.inner.effects()
        }

        fn rundir(&mut self) -> &mut dyn crate::rundir::RunDirHooks {
            self.inner.rundir()
        }

        fn events(&mut self) -> &mut dyn crate::events::log::EventHooks {
            self.inner.events()
        }

        fn container(&mut self) -> &mut dyn crate::runner::container::ContainerHooks {
            self.inner.container()
        }

        fn spawn(&mut self) -> &mut dyn crate::agent::proc::SpawnHooks {
            self.inner.spawn()
        }

        fn folded(&mut self, fold: &TopologyFold, events: &[TopologyEvent]) {
            self.inner.folded(fold, events);
            if let Some(last) = events.last() {
                self.folds.push((last.body.kind(), fold.clone()));
            }
        }
    }

    fn ending_as(fold: &TopologyFold, outcome: RunOutcome) -> Result<(), String> {
        let event = TopologyEvent {
            ts: "2026-09-30T00:00:00Z".to_owned(),
            body: TopologyEventBody::RunFinished {
                data: crate::engine::topology::closure::run_finished(fold, outcome),
            },
        };
        fold.plan_transition(&event)
            .map(drop)
            .map_err(|refusal| refusal.to_string())
    }

    fn four() -> [WideTask; 4] {
        [
            WideTask::independent("alpha"),
            WideTask::independent("beta"),
            WideTask::independent("gamma"),
            WideTask::independent("delta"),
        ]
    }

    const TIGHT: crate::engine::topology::select::Ceiling =
        crate::engine::topology::select::Ceiling {
            run_usd: Some(0.4),
            task_usd: None,
        };

    fn budget_stopped_with_two_in_flight(
        tag: &str,
    ) -> (Wide, Recording, Result<Progress, UpstrokeError>) {
        let tasks = four();
        let mut wide = Wide::started_under(
            tag,
            &tasks,
            3,
            WidePlans::default(),
            holding(&tasks, &[]),
            TIGHT,
        );
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let mut scheduler = Scheduler::scripted(
            &runner,
            Box::new(|view: &Quiescent<'_>| {
                released(view, |invocation| attempt_key(invocation) == Some(0))
            }),
        );
        let mut recording = Recording::over(&wide);
        let pipelines = wide.env.pipelines();
        let progress = wide.run.run_concurrently(
            &wide.env.seams(),
            &pipelines,
            &mut recording,
            Some(&mut scheduler),
        );
        drop(scheduler);
        (wide, recording, progress)
    }

    #[test]
    fn over_budget_prefix_without_budget_exceeded_is_not_ending_at_width_three() {
        let (wide, recording, progress) =
            budget_stopped_with_two_in_flight("coordinator-over-budget-prefix");
        assert_eq!(
            outcome_of(&progress.expect("the budget-stopped run ends")),
            RunOutcome::BudgetExceeded
        );
        let stop = recording
            .folds
            .iter()
            .position(|(kind, _)| *kind == "budget_exceeded")
            .expect("the ceiling was reached");
        let (kind, before) = &recording.folds[stop - 1];
        assert_eq!(
            *kind, "task_candidate_created",
            "alpha's promotion crossed the ceiling"
        );
        let events = wide.env.durable_events();
        let kinds = kinds_of(&events);
        let durable_stop = position(&kinds, "budget_exceeded", 0);
        let crossed =
            crate::engine::topology::select::Spend::replay(&events[..durable_stop]).run_usd();
        assert!(
            crossed >= 0.4,
            "the settled spend had reached the ceiling before `budget_exceeded`: {crossed}"
        );
        assert!(
            !before.run_is_ending() && before.budget_stop().is_none(),
            "an over-budget prefix without `budget_exceeded` is not ending"
        );
        assert_eq!(
            before.derived_outcome(),
            crate::topology::events::DerivedOutcome::NotEnding
        );
        for outcome in [
            RunOutcome::BudgetExceeded,
            RunOutcome::Complete,
            RunOutcome::Parked,
        ] {
            let refused = ending_as(before, outcome.clone())
                .expect_err("the fold refuses every end of a run that is not ending");
            assert!(refused.contains("not ending"), "{outcome:?}: {refused}");
        }
        assert_eq!(
            kinds[durable_stop - 1],
            "task_candidate_created",
            "the loop appended `budget_exceeded` at its next selection, before any effect of \
             that selection: {kinds:?}"
        );
        assert!(
            !kinds[durable_stop..].iter().any(|kind| matches!(
                *kind,
                "task_dispatched"
                    | "attempt_started"
                    | "merge_prepared"
                    | "merge_verification_started"
            )),
            "nothing was admitted after the stop: {kinds:?}"
        );
        assert_eq!(kinds.last(), Some(&"run_finished"), "{kinds:?}");
        assert!(
            ending_as(
                &recording.folds[recording.folds.len() - 2].1,
                RunOutcome::BudgetExceeded
            )
            .is_ok(),
            "with the record durable and the drain done, the budget-driven end is accepted"
        );
    }

    #[test]
    fn a_budget_stop_drains_live_pipelines_to_their_settlements_and_ends_budget_exceeded() {
        let (mut wide, _, progress) = budget_stopped_with_two_in_flight("coordinator-budget-drain");
        assert_eq!(
            outcome_of(&progress.expect("the budget-stopped run ends")),
            RunOutcome::BudgetExceeded
        );
        let events = wide.env.durable_events();
        let kinds = kinds_of(&events);
        let stop = position(&kinds, "budget_exceeded", 0);
        let drained: Vec<u32> = events[stop..]
            .iter()
            .filter_map(|event| match &event.body {
                TopologyEventBody::CandidatePrepared { data } => Some(data.key.0),
                _ => None,
            })
            .collect();
        assert_eq!(
            drained,
            vec![1, 2],
            "beta and gamma, live at the stop, drained to their natural settlements: {kinds:?}"
        );
        assert_eq!(
            count(&events, "attempt_interrupted"),
            0,
            "a budget stop cancels nothing"
        );
        assert!(
            !events.iter().any(|event| matches!(&event.body,
                TopologyEventBody::TaskDispatched { data } if data.key == TaskKey(3))),
            "delta was admissible and never admitted after the stop"
        );
        let TopologyEventBody::RunFinished { data } = &events.last().expect("an end").body else {
            panic!("the run ended");
        };
        assert_eq!(data.outcome, RunOutcome::BudgetExceeded);
        assert_eq!(
            wide.run.fold().queue().map(|queue| queue.entries().len()),
            Some(3),
            "the three queued candidates stay resumably open"
        );
        assert!(
            wide.env
                .runner
                .endings()
                .iter()
                .all(|(_, ending)| *ending == crate::engine::topology::scaffold::Ending::Completed),
            "no process was cancelled"
        );
        let ledger = wide.run.broker_mut().invocations();
        assert!(ledger.balances() && ledger.duplicates() == 0);
        replay_equals_live(&wide);
    }

    fn deferring_alpha(tasks: &[WideTask], asking: &[u32]) -> RecordingRunner {
        let base = crate::engine::topology::scaffold::wide_responder_asking(tasks, &[], asking);
        let runner = RecordingRunner::new().answering(Box::new(move |request| {
            if let InvocationId::Attempt {
                key: TaskKey(0),
                generation: GenerationId(0),
                role: AttemptRole::ReviewPass(_),
                ..
            } = &request.invocation
            {
                return Err(crate::runner::RunnerError::new(
                    &request.invocation,
                    crate::error::ProcessFate::Gone,
                    UpstrokeError::Refused {
                        message: "the scaffold's reviewer pool is unavailable".to_owned(),
                    },
                ));
            }
            base(request)
        }));
        runner.hold();
        runner
    }

    #[test]
    fn run_finished_halted_and_budget_exceeded_accepted_with_deferred_items_at_width_three() {
        let tasks = three();
        let mut wide = Wide::durable_under(
            "coordinator-deferred-budget",
            &tasks,
            3,
            WidePlans::default(),
            deferring_alpha(&tasks, &[]),
            crate::engine::topology::select::Ceiling {
                run_usd: Some(0.6),
                task_usd: None,
            },
        );
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let mut scheduler = Scheduler::scripted(
            &runner,
            Box::new(|view: &Quiescent<'_>| {
                released(view, |invocation| attempt_key(invocation) == Some(0))
                    .or_else(|| released(view, |invocation| attempt_key(invocation) == Some(1)))
            }),
        );
        let progress =
            drive(&mut wide, Some(&mut scheduler)).expect("the budget stop ends the run");
        drop(scheduler);
        assert_eq!(outcome_of(&progress), RunOutcome::BudgetExceeded);
        assert_eq!(
            wide.run.fold().task_state(TaskKey(0)),
            Some(crate::topology::fold::TaskState::Deferred),
            "alpha's outage deferred it, and the deferral never blocked the budget-driven end"
        );
        let kinds = kinds_of_log(&wide);
        assert!(
            !kinds.contains(&"defer_wait_elapsed"),
            "no backoff is waited while pipelines are live or once the run is ending: {kinds:?}"
        );
        assert_eq!(
            kinds[position(&kinds, "budget_exceeded", 0)..]
                .iter()
                .filter(|kind| **kind == "candidate_prepared")
                .count(),
            1,
            "gamma drained after the stop: {kinds:?}"
        );

        let (recovered, mut resumed) = wide
            .resume(
                "inc-2",
                RecordingRunner::new().answering(wide_responder(&tasks, &[])),
                crate::engine::topology::select::Ceiling::unlimited(),
            )
            .expect("a budget-stopped run resumes");
        assert_eq!(recovered.interrupted, 0);
        assert!(recovered.resumed.budget_stop_cleared);
        assert_eq!(
            resumed.run.fold().task_state(TaskKey(0)),
            Some(crate::topology::fold::TaskState::Pending),
            "`run_resumed` woke the deferred task"
        );
        let progress = drive(&mut resumed, None).expect("the resumed run completes");
        assert_eq!(outcome_of(&progress), RunOutcome::Complete);
        assert_eq!(count(&resumed.env.durable_events(), "task_merged"), 3);
        replay_equals_live(&resumed);

        let mut wide = Wide::started_with(
            "coordinator-deferred-halt",
            &tasks,
            3,
            WidePlans::default(),
            deferring_alpha(&tasks, &[2]),
        );
        halting(&mut wide.env);
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let mut scheduler = Scheduler::scripted(
            &runner,
            Box::new(|view: &Quiescent<'_>| {
                released(view, |invocation| attempt_key(invocation) == Some(0))
                    .or_else(|| released(view, |invocation| worker(invocation) == Some(TaskKey(2))))
            }),
        );
        let mut recording = Recording::over(&wide);
        let pipelines = wide.env.pipelines();
        let progress = wide
            .run
            .run_concurrently(
                &wide.env.seams(),
                &pipelines,
                &mut recording,
                Some(&mut scheduler),
            )
            .expect("the halt ends the run");
        drop(scheduler);
        assert_eq!(outcome_of(&progress), RunOutcome::Halted);
        let (_, before_end) = &recording.folds[recording.folds.len() - 2];
        assert_eq!(
            before_end.task_state(TaskKey(0)),
            Some(crate::topology::fold::TaskState::Deferred)
        );
        assert!(
            ending_as(before_end, RunOutcome::Halted).is_ok(),
            "Halted is accepted with a deferred task present, which is void with the run"
        );
        let events = wide.env.durable_events();
        let kinds = kinds_of(&events);
        let halted = position(&kinds, "question_answered", 0);
        assert_eq!(
            &kinds[halted..],
            &["question_answered", "attempt_interrupted", "run_finished"],
            "beta, in flight, was interrupted; nothing is appended for the deferred alpha: {kinds:?}"
        );
    }

    fn planted_settlement(
        key: TaskKey,
        transition: crate::topology::events::SettlementTransition,
    ) -> TopologyEventBody {
        TopologyEventBody::AttemptFinished {
            data: Box::new(crate::topology::events::AttemptFinished4 {
                key,
                generation: GenerationId(0),
                attempt: AttemptNumber(1),
                record: Box::new(crate::events::AttemptRecord {
                    attempt: 1,
                    tier: "mid".to_owned(),
                    model: "scaffold-model".to_owned(),
                    pool: None,
                    resumed: false,
                    duration: Duration::from_millis(5),
                    cost_usd: Some(0.5),
                    reviews: Vec::new(),
                    session_id: None,
                    usage: None,
                    failure: Some(crate::events::FailureRecord {
                        kind: crate::ladder::FailureKind::GateFailed,
                        origin: crate::ladder::FailureOrigin::Worker,
                        reason: "the planted drained settlement".to_owned(),
                        detail: None,
                    }),
                }),
                settlement: crate::topology::events::AttemptSettlement::Closed {
                    transition,
                    lease: crate::topology::events::LeaseDisposition::PredictedReleased,
                },
            }),
        }
    }

    #[test]
    fn run_finished_budget_exceeded_refused_after_halting_drain_settlement_at_width_three() {
        use crate::topology::events::SettlementTransition;
        let tasks = three();
        let mut wide = Wide::durable(
            "coordinator-halting-drain",
            &tasks,
            3,
            WidePlans::default(),
            holding(&tasks, &[]),
        );
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let mut scheduler = shutdown_at_first_point(&runner);
        drive(&mut wide, Some(&mut scheduler)).expect_err("three attempts in flight, shut down");
        drop(scheduler);

        let epoch = wide.run.fold().epoch().expect("started");
        let mut hooks = wide.env.hooks();
        let seams = wide.env.seams();
        let drained = [
            TopologyEventBody::BudgetExceeded {
                data: crate::topology::events::BudgetExceeded4 {
                    epoch,
                    budget: crate::events::BudgetKind::Run,
                    limit_usd: 0.4,
                    spent_usd: 0.5,
                    key: None,
                },
            },
            planted_settlement(TaskKey(1), SettlementTransition::Retry),
            planted_settlement(TaskKey(2), SettlementTransition::Retry),
            planted_settlement(
                TaskKey(0),
                SettlementTransition::Failed {
                    halts_run: true,
                    reason: "the drained settlement halts".to_owned(),
                },
            ),
        ];
        for body in drained {
            wide.run
                .emit(body, &seams, &mut hooks)
                .expect("the fold takes the drain's prefix");
        }
        let fold = wide.run.fold().clone();
        assert!(fold.budget_stop().is_some() && fold.halted_at() == Some(TaskKey(0)));
        assert_eq!(
            fold.derived_outcome(),
            crate::topology::events::DerivedOutcome::Ending(RunOutcome::Halted),
            "halt outranks budget"
        );
        let refused = ending_as(&fold, RunOutcome::BudgetExceeded)
            .expect_err("run_finished(BudgetExceeded) is refused after a halting drain settlement");
        assert!(
            refused.contains("budget exceeded") && refused.contains("halted"),
            "{refused}"
        );
        assert_eq!(
            crate::engine::topology::closure::ending_outcome(&fold)
                .expect("the ending outcome reads the halt"),
            RunOutcome::Halted
        );

        let pipelines = wide.env.pipelines();
        let progress = wide
            .run
            .run_concurrently(&seams, &pipelines, &mut hooks, None)
            .expect("the closure ends the run");
        assert_eq!(outcome_of(&progress), RunOutcome::Halted);
        let events = wide.env.durable_events();
        let ends: Vec<(RunOutcome, Option<TaskKey>)> = events
            .iter()
            .filter_map(|event| match &event.body {
                TopologyEventBody::RunFinished { data } => {
                    Some((data.outcome.clone(), data.halted_at))
                }
                _ => None,
            })
            .collect();
        assert_eq!(ends, vec![(RunOutcome::Halted, Some(TaskKey(0)))]);
    }

    #[test]
    fn a_halt_whose_cancelled_process_is_unresolved_ends_the_command_without_closing() {
        let mut wide = durable_halting_on_gamma("coordinator-halt-unresolved");
        let beta = AttemptIdentities::new(TaskKey(1), GenerationId(0), AttemptNumber(1)).worker();
        wide.env.runner.unresolved_when_cancelled(beta.clone());
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let mut scheduler = gamma_first(&runner);
        let error = drive(&mut wide, Some(&mut scheduler))
            .expect_err("closure is not entered over a process that may still run");
        drop(scheduler);
        let message = error.to_string();
        assert!(
            message.contains(&beta.render()) && message.contains("did not establish"),
            "the command ends naming the unresolved invocation: {message}"
        );
        let kinds = kinds_of_log(&wide);
        assert_eq!(
            kinds.last(),
            Some(&"question_answered"),
            "no terminal is appended and nothing is scrubbed over it: {kinds:?}"
        );
        let slot = crate::engine::topology::dispatch::task_slot(TaskKey(1), GenerationId(0));
        assert!(
            wide.env.fixture.manager.slot_path(&slot).exists(),
            "beta's worktree was left for the next process"
        );
        assert!(
            wide.env.runner.endings().contains(&(
                beta,
                crate::engine::topology::scaffold::Ending::CancelledUnresolved
            )),
            "{:?}",
            wide.env.runner.endings()
        );
        let (recovered, resumed) = finish_resumed_halted(wide, "unresolved");
        assert_eq!(recovered.interrupted, 2);
        ends_once_halted(&resumed, "unresolved");
    }

    #[test]
    fn run_finished_complete_refused_with_queued_candidate_at_width_three() {
        let tasks = three();
        let mut wide = Wide::started_with(
            "coordinator-complete-refused-queued",
            &tasks,
            3,
            WidePlans::default(),
            holding(&tasks, &[]),
        );
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let mut scheduler = Scheduler::seeded(&runner, 17);
        let mut recording = Recording::over(&wide);
        let pipelines = wide.env.pipelines();
        let progress = wide
            .run
            .run_concurrently(
                &wide.env.seams(),
                &pipelines,
                &mut recording,
                Some(&mut scheduler),
            )
            .expect("the run completes");
        drop(scheduler);
        assert_eq!(outcome_of(&progress), RunOutcome::Complete);
        let queued: Vec<&TopologyFold> = recording
            .folds
            .iter()
            .filter(|(_, fold)| fold.queue().is_some_and(|queue| !queue.is_empty()))
            .map(|(_, fold)| fold)
            .collect();
        assert!(!queued.is_empty(), "candidates were queued along the way");
        for fold in queued {
            for outcome in [RunOutcome::Complete, RunOutcome::Parked] {
                let refused = ending_as(fold, outcome.clone())
                    .expect_err("a queued candidate makes the run not ending");
                assert!(refused.contains("not ending"), "{refused}");
            }
        }
        let (_, before_end) = &recording.folds[recording.folds.len() - 2];
        assert!(ending_as(before_end, RunOutcome::Complete).is_ok());
    }

    #[test]
    fn run_finished_parked_refused_with_admissible_work_at_width_three() {
        let tasks = three();
        let runner = RecordingRunner::new().answering(
            crate::engine::topology::scaffold::wide_responder_asking(&tasks, &[], &[2]),
        );
        runner.hold();
        let mut wide = Wide::started_with(
            "coordinator-parked-refused",
            &tasks,
            3,
            WidePlans::default(),
            runner,
        );
        wide.env.adapters =
            std::sync::Arc::new(crate::engine::topology::scaffold::ScaffoldAdapters::echoing());
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let mut scheduler = gamma_first(&runner);
        let mut recording = Recording::over(&wide);
        let pipelines = wide.env.pipelines();
        let progress = wide
            .run
            .run_concurrently(
                &wide.env.seams(),
                &pipelines,
                &mut recording,
                Some(&mut scheduler),
            )
            .expect("the run parks on gamma's question");
        drop(scheduler);
        assert_eq!(outcome_of(&progress), RunOutcome::Parked);
        let parked = recording
            .folds
            .iter()
            .position(|(_, fold)| fold.questions_open())
            .expect("gamma parked");
        let (_, at_park) = &recording.folds[parked];
        assert!(
            at_park.pipeline_held() >= 2,
            "alpha and beta were in flight when gamma parked"
        );
        for outcome in [RunOutcome::Parked, RunOutcome::Complete] {
            let refused = ending_as(at_park, outcome.clone())
                .expect_err("the in-flight and admissible work makes the run not ending");
            assert!(refused.contains("not ending"), "{outcome:?}: {refused}");
        }
        let events = wide.env.durable_events();
        assert_eq!(
            count(&events, "task_merged"),
            2,
            "alpha and beta ran to their merges after gamma parked"
        );
        let (_, before_end) = &recording.folds[recording.folds.len() - 2];
        assert!(ending_as(before_end, RunOutcome::Parked).is_ok());
    }

    #[test]
    fn run_finished_parked_or_complete_refused_while_deferred_items_exist_at_width_three() {
        let tasks = three();
        let mut wide = Wide::started_with(
            "coordinator-deferred-refused",
            &tasks,
            3,
            WidePlans::default(),
            deferring_alpha(&tasks, &[]),
        );
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let mut scheduler = Scheduler::scripted(
            &runner,
            Box::new(|view: &Quiescent<'_>| {
                released(view, |invocation| attempt_key(invocation) == Some(0))
            }),
        );
        let mut recording = Recording::over(&wide);
        let pipelines = wide.env.pipelines();
        let progress = wide
            .run
            .run_concurrently(
                &wide.env.seams(),
                &pipelines,
                &mut recording,
                Some(&mut scheduler),
            )
            .expect("the run completes once the deferral is waited");
        drop(scheduler);
        assert_eq!(outcome_of(&progress), RunOutcome::Complete);
        let deferred: Vec<&TopologyFold> = recording
            .folds
            .iter()
            .filter(|(_, fold)| fold.backoff_pending())
            .map(|(_, fold)| fold)
            .collect();
        assert!(!deferred.is_empty(), "alpha was deferred");
        for fold in deferred {
            for outcome in [RunOutcome::Parked, RunOutcome::Complete] {
                let refused = ending_as(fold, outcome.clone())
                    .expect_err("pending backoff makes Parked and Complete not ending");
                assert!(refused.contains("not ending"), "{outcome:?}: {refused}");
            }
        }
        let events = wide.env.durable_events();
        let kinds = kinds_of(&events);
        let waited = position(&kinds, "defer_wait_elapsed", 0);
        assert!(
            position(&kinds, "task_merged", 1) < waited,
            "the backoff was waited only once no pipeline was live: {kinds:?}"
        );
        assert_eq!(count(&events, "task_merged"), 3);
    }

    fn needs_human_verification(tasks: &[WideTask], asking: &[u32]) -> RecordingRunner {
        let base = crate::engine::topology::scaffold::wide_responder_asking(tasks, &[], asking);
        let runner = RecordingRunner::new().answering(Box::new(move |request| {
            if matches!(request.invocation, InvocationId::Sequence { .. })
                && matches!(request.role, crate::runner::ExecutionRole::Review)
            {
                return Ok(crate::engine::topology::scaffold::exited(
                    0,
                    "```json\n{\"pass\": false, \"reasons\": [\"a person must decide this \
                     integration\"], \"required_changes\": [], \"needs_human\": true}\n```"
                        .to_owned(),
                ));
            }
            base(request)
        }));
        runner.hold();
        runner
    }

    #[test]
    fn run_finished_halted_accepted_after_declined_verification_park_at_width_three() {
        let tasks = three();
        let mut wide = Wide::started_with(
            "coordinator-declined-park",
            &tasks,
            3,
            WidePlans::default(),
            needs_human_verification(&tasks, &[]),
        );
        halting(&mut wide.env);
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let mut scheduler = Scheduler::scripted(
            &runner,
            Box::new(|view: &Quiescent<'_>| {
                let merges = count(view.run.events(), "task_merged");
                if merges == 0 {
                    return released(view, |invocation| attempt_key(invocation) == Some(0));
                }
                released(view, |invocation| attempt_key(invocation) == Some(1)).or_else(|| {
                    released(view, |invocation| {
                        matches!(invocation, InvocationId::Sequence { .. })
                    })
                })
            }),
        );
        let mut recording = Recording::over(&wide);
        let pipelines = wide.env.pipelines();
        let progress = wide
            .run
            .run_concurrently(
                &wide.env.seams(),
                &pipelines,
                &mut recording,
                Some(&mut scheduler),
            )
            .expect("the declined park halts the run");
        drop(scheduler);
        assert_eq!(outcome_of(&progress), RunOutcome::Halted);
        let events = wide.env.durable_events();
        let kinds = kinds_of(&events);
        let parked = position(&kinds, "merge_verification_unavailable", 0);
        let declined = position(&kinds, "question_answered", 0);
        assert!(parked < declined, "{kinds:?}");
        assert_eq!(
            &kinds[declined..],
            &["question_answered", "attempt_interrupted", "run_finished"],
            "the decline halts the run with gamma in flight, which the closure interrupts: \
             {kinds:?}"
        );
        let (_, before_end) = &recording.folds[recording.folds.len() - 2];
        assert!(ending_as(before_end, RunOutcome::Halted).is_ok());
        assert_eq!(
            wide.run.fold().task_state(TaskKey(1)),
            Some(crate::topology::fold::TaskState::Failed),
            "the declined verification park fails beta"
        );
        assert!(
            wide.run
                .fold()
                .queue()
                .is_some_and(|queue| queue.is_empty()),
            "and consumes its queue position"
        );
    }

    #[test]
    fn replayed_conflicting_outcome_refused_at_width_three() {
        let mut wide = durable_halting_on_gamma("coordinator-conflicting-outcome");
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let mut scheduler = gamma_first(&runner);
        let progress = drive(&mut wide, Some(&mut scheduler)).expect("the halted run ends");
        drop(scheduler);
        assert_eq!(outcome_of(&progress), RunOutcome::Halted);

        let log = std::fs::read(&wide.env.log).expect("the log");
        let body = &log[..log.len() - 1];
        let last = body
            .iter()
            .rposition(|byte| *byte == b'\n')
            .expect("a prefix")
            + 1;
        let forged = TopologyEvent {
            ts: "2026-09-30T00:00:00Z".to_owned(),
            body: TopologyEventBody::RunFinished {
                data: crate::topology::events::RunFinished4 {
                    outcome: RunOutcome::Complete,
                    halted_at: None,
                    merged: 0,
                    parked: 0,
                },
            },
        };
        let (line, _) = crate::events::log::TopologyLine::round_trip(&forged)
            .expect("the forged end serializes");
        let mut rewritten = log[..last].to_vec();
        rewritten.extend_from_slice(line.committed_bytes());
        crate::workspace_manager::fixture::write_file(&wide.env.log, &rewritten);

        let events = TopologyFold::parse_log(&rewritten).expect("the forged log parses");
        let refused = TopologyFold::replay(wide.env.inputs.clone(), &events)
            .expect_err("the checked replay refuses the conflicting outcome");
        assert!(
            matches!(
                refused,
                crate::topology::fold::FoldError::OutcomeMismatch { .. }
            ),
            "{refused}"
        );
        let text = refused.to_string();
        assert!(
            text.contains("complete") && text.contains("halted"),
            "{text}"
        );

        let tasks = three();
        let error = match wide.resume(
            "inc-2",
            RecordingRunner::new().answering(wide_responder(&tasks, &[])),
            crate::engine::topology::select::Ceiling::unlimited(),
        ) {
            Ok(_) => panic!("a resume of a log with a conflicting outcome must refuse"),
            Err(error) => error.to_string(),
        };
        assert!(
            error.contains("complete") && error.contains("halted"),
            "the resume's barrier refuses with the same mismatch: {error}"
        );
    }

    #[test]
    fn a_verifications_review_spend_is_charged_before_the_next_selection() {
        let tasks = [
            WideTask::independent("t0"),
            WideTask::independent("t1"),
            WideTask::independent("t2"),
            WideTask::independent("t3"),
            WideTask::independent("t4"),
        ];
        let mut wide = Wide::started_under(
            "coordinator-verification-spend",
            &tasks,
            3,
            WidePlans::default(),
            holding(&tasks, &[]),
            crate::engine::topology::select::Ceiling {
                run_usd: Some(1.6),
                task_usd: None,
            },
        );
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let mut arrived_before_t3: Option<bool> = None;
        let mut scheduler = Scheduler::scripted(
            &runner,
            Box::new(|view: &Quiescent<'_>| {
                let merges = count(view.run.events(), "task_merged");
                let verifying = view.run.fold().transaction().is_some();
                if !verifying {
                    return released(view, |invocation| {
                        attempt_key(invocation) == Some(if merges == 0 { 0 } else { 1 })
                    });
                }
                released(view, |invocation| worker(invocation) == Some(TaskKey(2)))
                    .or_else(|| {
                        released(view, |invocation| {
                            attempt_key(invocation) == Some(3)
                                && matches!(
                                    role_of(invocation),
                                    Some(AttemptRole::Worker | AttemptRole::Gate(_))
                                )
                        })
                    })
                    .or_else(|| {
                        released(view, |invocation| {
                            matches!(invocation, InvocationId::Sequence { .. })
                        })
                    })
                    .or_else(|| {
                        if arrived_before_t3.is_none() {
                            arrived_before_t3 = Some(!view.live.iter().any(|(_, identity)| {
                                matches!(identity, Identity::Verification { .. })
                            }));
                        }
                        released(view, |invocation| attempt_key(invocation) == Some(3))
                    })
                    .or_else(|| released(view, |invocation| attempt_key(invocation) == Some(2)))
            }),
        );
        let progress =
            drive(&mut wide, Some(&mut scheduler)).expect("the budget stop ends the run");
        drop(scheduler);
        assert_eq!(
            arrived_before_t3,
            Some(true),
            "the verification's completion had arrived while t2's gate snapshot kept `verify` \
             waiting, before t3 settled"
        );
        assert_eq!(outcome_of(&progress), RunOutcome::BudgetExceeded);
        let events = wide.env.durable_events();
        let kinds = kinds_of(&events);
        let stop = position(&kinds, "budget_exceeded", 0);
        assert!(
            !events.iter().any(|event| matches!(&event.body,
                TopologyEventBody::TaskDispatched { data } if data.key == TaskKey(4))),
            "t4 was never dispatched: the verification's review spend counted at the selection \
             t3's settlement opened, before `verify` returned: {kinds:?}"
        );
        assert!(
            position(&kinds, "candidate_prepared", 2) < stop
                && stop < position(&kinds, "merge_prepared", 1),
            "the stop came after t3 settled and before the verification's terminal: {kinds:?}"
        );
    }

    #[test]
    fn a_closure_never_settles_in_flight_work_its_coordinator_did_not_cancel() {
        use crate::topology::events::SettlementTransition;
        let tasks = three();
        let mut wide = Wide::durable(
            "coordinator-unvouched",
            &tasks,
            3,
            WidePlans::default(),
            holding(&tasks, &[]),
        );
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let mut scheduler = shutdown_at_first_point(&runner);
        drive(&mut wide, Some(&mut scheduler)).expect_err("three attempts in flight, shut down");
        drop(scheduler);
        let mut hooks = wide.env.hooks();
        let seams = wide.env.seams();
        wide.run
            .emit(
                planted_settlement(
                    TaskKey(0),
                    SettlementTransition::Failed {
                        halts_run: true,
                        reason: "a halting settlement".to_owned(),
                    },
                ),
                &seams,
                &mut hooks,
            )
            .expect("the fold takes the halting settlement");
        let before = std::fs::read(&wide.env.log).expect("the log");

        let pipelines = wide.env.pipelines();
        let error = wide
            .run
            .run_concurrently(&seams, &pipelines, &mut hooks, None)
            .expect_err("a coordinator that cancelled nothing vouches for nothing");
        let message = error.to_string();
        assert!(
            message.contains("no pipeline of this process was cancelled for")
                && message.contains("task k1 generation 0 is in flight")
                && message.contains("task k2 generation 0 is in flight")
                && message.contains("nothing was appended"),
            "{message}"
        );
        assert_eq!(
            std::fs::read(&wide.env.log).expect("the log"),
            before,
            "nothing was appended"
        );

        let (recovered, resumed) = finish_resumed_halted(wide, "unvouched");
        assert_eq!(
            recovered.interrupted, 2,
            "the next process's recovery settles what no live coordinator vouched for"
        );
        let events = resumed.env.durable_events();
        assert_eq!(count(&events, "run_finished"), 1);
        replay_equals_live(&resumed);
    }

    #[test]
    fn one_seed_reproduces_one_run_down_to_what_each_process_saw_when_it_started() {
        type Started = Vec<(String, Vec<String>, String)>;
        let tasks = three();
        let mut reference: Option<(Vec<InvocationId>, Started, String)> = None;
        for run in 0..3 {
            let runner = holding(&tasks, &[]);
            let plans = WidePlans {
                reviewers: 2,
                verify_reviewers: 2,
                ..WidePlans::default()
            };
            let mut wide = Wide::started_with(
                &format!("coordinator-one-seed-{run}"),
                &tasks,
                3,
                plans,
                runner,
            );
            let runner = std::sync::Arc::clone(&wide.env.runner);
            let mut scheduler = Scheduler::seeded(&runner, 11);
            let progress = drive(&mut wide, Some(&mut scheduler))
                .unwrap_or_else(|error| panic!("run {run}: {error}"));
            let released = scheduler.released.clone();
            drop(scheduler);
            assert_eq!(outcome_of(&progress), RunOutcome::Complete, "run {run}");
            let started: Started = wide
                .env
                .runner
                .ran()
                .into_iter()
                .map(|ran| {
                    let workspace = ran
                        .workspace
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or_default()
                        .to_owned();
                    (ran.invocation.render(), ran.durable_at_spawn, workspace)
                })
                .collect();
            let run_id = wide.run.fold().started().expect("started").run_id.clone();
            let log = serde_json::to_string(&canonical(&wide.env.durable_events(), &run_id))
                .expect("serializes");
            let observed = (released, started, log);
            match &reference {
                None => reference = Some(observed),
                Some(expected) => assert_eq!(
                    &observed, expected,
                    "run {run} of seed 11 released, started or logged something run 0 did not"
                ),
            }
        }
    }

    #[test]
    fn out_of_order_completions_bind_to_their_own_identities_under_seeded_permutations() {
        let tasks = three();
        let mut reference: Option<BTreeMap<u64, Vec<String>>> = None;
        for seed in 0..16_u64 {
            let runner = holding(&tasks, &[]);
            let mut wide = Wide::started_with(
                &format!("coordinator-seed-{seed}"),
                &tasks,
                3,
                WidePlans::default(),
                runner,
            );
            let runner = std::sync::Arc::clone(&wide.env.runner);
            let mut scheduler = Scheduler::seeded(&runner, seed);
            let progress = drive(&mut wide, Some(&mut scheduler))
                .unwrap_or_else(|error| panic!("seed {seed}: {error}"));
            drop(scheduler);
            assert_eq!(outcome_of(&progress), RunOutcome::Complete, "seed {seed}");
            let events = wide.env.durable_events();
            let run_id = wide.run.fold().started().expect("started").run_id.clone();
            let keyed = attempt_level(&events, &run_id);
            match &reference {
                None => reference = Some(keyed),
                Some(expected) => assert_eq!(
                    &keyed, expected,
                    "seed {seed}: a key's attempt-level events differ from seed 0's"
                ),
            }
            let created: Vec<u32> = events
                .iter()
                .filter_map(|event| match &event.body {
                    TopologyEventBody::TaskCandidateCreated { data } => Some(data.candidate.key.0),
                    _ => None,
                })
                .collect();
            let integrated: Vec<u32> = events
                .iter()
                .filter_map(|event| match &event.body {
                    TopologyEventBody::MergePrepared { data } => Some(data.key.0),
                    _ => None,
                })
                .collect();
            assert_eq!(
                integrated, created,
                "seed {seed}: the queue integrated in `task_candidate_created` order"
            );
            assert!(wide.run.invocations_balance(), "seed {seed}");
            replay_equals_live(&wide);
        }
    }

    #[test]
    fn independent_tasks_dispatch_together_and_keep_per_key_projections_under_every_seed() {
        let tasks = three();
        let mut reference: Option<(BTreeMap<u64, Vec<String>>, String)> = None;
        for seed in 0..8_u64 {
            let runner = holding(&tasks, &[]);
            let mut wide = Wide::started_with(
                &format!("coordinator-st08-{seed}"),
                &tasks,
                3,
                WidePlans::default(),
                runner,
            );
            let runner = std::sync::Arc::clone(&wide.env.runner);
            let mut scheduler = Scheduler::seeded(&runner, seed ^ 0x5EED);
            drive(&mut wide, Some(&mut scheduler))
                .unwrap_or_else(|error| panic!("seed {seed}: {error}"));
            drop(scheduler);
            let events = wide.env.durable_events();
            let kinds = kinds_of(&events);
            let first_settlement = position(&kinds, "candidate_prepared", 0);
            assert!(
                position(&kinds, "task_dispatched", 2) < first_settlement,
                "seed {seed}: every independent task was dispatched before any completion: \
                 {kinds:?}"
            );
            let bases: Vec<String> = dispatched_bases(&events).into_values().collect();
            assert!(
                bases.windows(2).all(|pair| pair.first() == pair.get(1)),
                "seed {seed}: the independent tasks share one base: {bases:?}"
            );
            let run_id = wide.run.fold().started().expect("started").run_id.clone();
            let observed = (attempt_level(&events, &run_id), final_tree(&wide));
            match &reference {
                None => reference = Some(observed),
                Some(expected) => assert_eq!(
                    &observed, expected,
                    "seed {seed}: per-key projection or final tree differs"
                ),
            }
        }
    }

    #[test]
    fn a_chain_plan_projects_identically_at_widths_three_and_one() {
        let tasks = [
            WideTask::independent("alpha"),
            WideTask::after("beta", &["alpha"]),
            WideTask::after("gamma", &["beta"]),
        ];
        let mut narrow = Wide::started("coordinator-chain-one", &tasks, 1);
        let mut hooks = narrow.env.hooks();
        let seams = narrow.env.seams();
        let mut steps = 0_u32;
        loop {
            steps += 1;
            assert!(steps < 200, "the width-1 loop did not finish");
            match narrow.run.step(&seams, &mut hooks).expect("a step") {
                Progress::Finished { outcome, .. } => {
                    assert_eq!(outcome, RunOutcome::Complete);
                    break;
                }
                _ => continue,
            }
        }
        let mut wide = Wide::started("coordinator-chain-three", &tasks, 3);
        let progress = drive(&mut wide, None).expect("the width-3 run completes");
        assert_eq!(outcome_of(&progress), RunOutcome::Complete);

        let run_id = wide.run.fold().started().expect("started").run_id.clone();
        let one = serde_json::to_string(&canonical(&narrow.env.durable_events(), &run_id))
            .expect("serializes");
        let three = serde_json::to_string(&canonical(&wide.env.durable_events(), &run_id))
            .expect("serializes");
        assert_eq!(
            one, three,
            "a chain's canonical projections at widths 1 and 3 are byte-identical"
        );
        assert_eq!(final_tree(&narrow), final_tree(&wide));
    }

    #[test]
    fn concurrent_attempts_at_one_generation_and_attempt_use_distinct_snapshot_slots() {
        let tasks = three();
        let runner = holding(&tasks, &[]);
        let plans = WidePlans {
            reviewers: 2,
            verify_reviewers: 2,
            ..WidePlans::default()
        };
        let mut wide = Wide::started_with("coordinator-st04", &tasks, 3, plans, runner);
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let mut scheduler = Scheduler::seeded(&runner, 7);
        drive(&mut wide, Some(&mut scheduler)).expect("the run completes");
        let widest = scheduler.widest;
        drop(scheduler);
        assert_eq!(widest, 3, "all three attempts were in flight at once");

        let ran = wide.env.runner.ran();
        let mut identities: Vec<String> =
            ran.iter().map(|entry| entry.invocation.render()).collect();
        let total = identities.len();
        identities.sort();
        identities.dedup();
        assert_eq!(
            identities.len(),
            total,
            "every process had its own InvocationId"
        );

        let snapshots: Vec<(u32, std::path::PathBuf)> = ran
            .iter()
            .filter(|entry| {
                matches!(
                    role_of(&entry.invocation),
                    Some(AttemptRole::Gate(_) | AttemptRole::ReviewPass(_))
                )
            })
            .filter_map(|entry| {
                attempt_key(&entry.invocation).map(|key| (key, entry.workspace.clone()))
            })
            .collect();
        assert_eq!(
            snapshots.len(),
            9,
            "three tasks, one gate and two reviewers each"
        );
        let mut paths: Vec<&std::path::PathBuf> = snapshots.iter().map(|(_, path)| path).collect();
        paths.sort();
        paths.dedup();
        assert_eq!(
            paths.len(),
            9,
            "no two tasks at generation 0, attempt 1 shared a snapshot slot: {snapshots:?}"
        );
        for (key, path) in &snapshots {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            assert!(
                name.starts_with(&format!("k{key}-g0-a1-")),
                "task {key}'s snapshot `{name}` names its task"
            );
        }
        assert_eq!(
            wide.run.reservations_peak(),
            1,
            "at most one provisional reservation was ever outstanding"
        );
        assert!(wide.run.invocations_balance());
    }

    #[test]
    fn live_state_equals_replay_after_every_append_at_width_three() {
        let tasks = three();
        let runner = holding(&tasks, &[]);
        let mut wide =
            Wide::started_with("coordinator-st10", &tasks, 3, WidePlans::default(), runner);
        assert!(
            wide.run.invocations_balance() && wide.run.entitlements_held() == 0,
            "the ledgers are empty at coordinator start"
        );
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let mut scheduler = Scheduler::seeded(&runner, 3);
        let mut folding = Folding {
            inner: wide.env.hooks(),
            states: Vec::new(),
        };
        let pipelines = wide.env.pipelines();
        wide.run
            .run_concurrently(
                &wide.env.seams(),
                &pipelines,
                &mut folding,
                Some(&mut scheduler),
            )
            .expect("the run completes");
        drop(scheduler);
        let events = wide.env.durable_events();
        assert!(
            folding.states.len() > 20,
            "{} appends observed",
            folding.states.len()
        );
        for (length, live) in &folding.states {
            let prefix = events.get(..*length).expect("a prefix of the durable log");
            let replayed = TopologyFold::replay(wide.env.inputs.clone(), prefix)
                .expect("every prefix replays");
            assert_eq!(
                replayed.state(),
                live.as_ref(),
                "after append {length} the live fold and the replay of the prefix differ"
            );
        }
        assert!(wide.run.invocations_balance(), "balanced at the end");
        assert_eq!(wide.run.entitlements_held(), 0);
    }

    #[test]
    fn a_retained_retry_is_admitted_beside_other_pipelines_and_regates_on_fresh_snapshots() {
        let tasks = three();
        let runner = holding(&tasks, &[(0, 1)]);
        let mut wide =
            Wide::started_with("coordinator-st15", &tasks, 3, WidePlans::default(), runner);
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let mut scheduler = Scheduler::scripted(
            &runner,
            Box::new(|view: &Quiescent<'_>| {
                released(view, |invocation| attempt_key(invocation) == Some(0))
            }),
        );
        drive(&mut wide, Some(&mut scheduler)).expect("the run completes");
        drop(scheduler);
        let events = wide.env.durable_events();
        let kinds = kinds_of(&events);
        let retry = events
            .iter()
            .position(|event| {
                matches!(&event.body, TopologyEventBody::AttemptStarted { data }
                    if data.key == TaskKey(0) && data.attempt == AttemptNumber(2))
            })
            .expect("the retry started");
        let TopologyEventBody::AttemptStarted { data } =
            &events.get(retry).expect("the retry").body
        else {
            panic!("the retry is an attempt_started");
        };
        assert_eq!(data.generation, GenerationId(0), "the same generation");
        assert!(
            data.resume_session.is_some(),
            "the retained session resumes"
        );
        let others_settled = events.iter().position(|event| {
            matches!(&event.body, TopologyEventBody::CandidatePrepared { data } if data.key != TaskKey(0))
        });
        assert!(
            others_settled.is_some_and(|settled| settled > retry),
            "the retry was admitted while the other tasks were still in flight: {kinds:?}"
        );
        let ran = wide.env.runner.ran();
        let gates: Vec<String> = ran
            .iter()
            .filter(|entry| {
                attempt_key(&entry.invocation) == Some(0)
                    && matches!(role_of(&entry.invocation), Some(AttemptRole::Gate(_)))
            })
            .map(|entry| {
                entry
                    .workspace
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default()
                    .to_owned()
            })
            .collect();
        assert_eq!(
            gates,
            vec!["k0-g0-a1-gates".to_owned(), "k0-g0-a2-gates".to_owned()],
            "the retry re-gated on a fresh snapshot of its own"
        );
        assert_eq!(count(&events, "task_merged"), 3, "{kinds:?}");
        assert!(wide.run.invocations_balance());
    }

    #[test]
    fn adversarial_orders_with_one_slot_per_agent_and_pool_always_reach_run_finished() {
        let tasks = three();
        for seed in 0..12_u64 {
            let runner = holding(&tasks, &[]);
            let plans = WidePlans {
                reviewers: 2,
                verify_reviewers: 2,
                pool: Some("scaffold-pool".to_owned()),
                ..WidePlans::default()
            };
            let mut wide = Wide::started_with(
                &format!("coordinator-deadlock-{seed}"),
                &tasks,
                3,
                plans,
                runner,
            );
            let runner = std::sync::Arc::clone(&wide.env.runner);
            let mut scheduler = Scheduler::seeded(&runner, seed ^ 0xDEAD);
            let mut hooks = wide.env.hooks();
            let pipelines = wide.env.pipelines_limited(
                crate::engine::topology::scaffold::SlotLimitsOf::Exactly(1, 1),
            );
            let progress = wide
                .run
                .run_concurrently(
                    &wide.env.seams(),
                    &pipelines,
                    &mut hooks,
                    Some(&mut scheduler),
                )
                .unwrap_or_else(|error| panic!("seed {seed}: {error}"));
            assert_eq!(outcome_of(&progress), RunOutcome::Complete, "seed {seed}");
            assert_eq!(
                scheduler.widest_slotted, 1,
                "seed {seed}: one slot for the agent and the pool admits one agent process at a \
                 time (gates take no slot)"
            );
            assert!(
                scheduler.points < 200,
                "seed {seed}: {} steps",
                scheduler.points
            );
            drop(scheduler);
            assert!(wide.run.invocations_balance(), "seed {seed}");
        }
    }

    #[test]
    fn a_pipeline_error_cancels_the_others_and_ends_the_command_resumably() {
        let tasks = three();
        let base = wide_responder(&tasks, &[]);
        let runner = RecordingRunner::new().answering(Box::new(move |request| {
            if attempt_key(&request.invocation) == Some(0)
                && matches!(role_of(&request.invocation), Some(AttemptRole::Gate(_)))
            {
                return Err(crate::runner::RunnerError::unresolved(
                    &request.invocation,
                    UpstrokeError::Refused {
                        message: "the scaffold lost track of this gate".to_owned(),
                    },
                ));
            }
            base(request)
        }));
        runner.hold();
        let mut wide = Wide::started_with(
            "coordinator-pipeline-error",
            &tasks,
            3,
            WidePlans::default(),
            runner,
        );
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let mut scheduler = Scheduler::scripted(
            &runner,
            Box::new(|view: &Quiescent<'_>| {
                released(view, |invocation| attempt_key(invocation) == Some(0))
            }),
        );
        let error = drive(&mut wide, Some(&mut scheduler)).expect_err("the command ends");
        drop(scheduler);
        assert!(
            error.to_string().contains("lost track of this gate"),
            "the pipeline's own error ends the command: {error}"
        );
        let events = wide.env.durable_events();
        let kinds = kinds_of(&events);
        assert_eq!(
            count(&events, "attempt_interrupted"),
            0,
            "no live `attempt_interrupted`: recovery settles the open attempts: {kinds:?}"
        );
        assert_eq!(count(&events, "attempt_started"), 3, "{kinds:?}");
        assert_eq!(
            kinds.last(),
            Some(&"attempt_started"),
            "nothing was appended after the error: {kinds:?}"
        );
        let mut cancelled: Vec<u32> = wide
            .env
            .runner
            .endings()
            .into_iter()
            .filter(|(_, ending)| *ending == crate::engine::topology::scaffold::Ending::Cancelled)
            .filter_map(|(invocation, _)| attempt_key(&invocation))
            .collect();
        cancelled.sort_unstable();
        assert_eq!(
            cancelled,
            vec![1, 2],
            "the other two workers were cancelled"
        );
        let gate = AttemptIdentities::new(TaskKey(0), GenerationId(0), AttemptNumber(1)).gate(0, 0);
        settled_but(&mut wide.run, &[gate]).unwrap_or_else(|ledger| {
            panic!(
                "the gate whose process the Runner could not establish as ended keeps its \
                 registration, and every other invocation is settled once: {ledger}"
            )
        });
        replay_equals_live(&wide);
    }

    #[test]
    fn a_panicking_pipeline_ends_the_command_with_a_defined_error() {
        let tasks = three();
        let base = wide_responder(&tasks, &[]);
        let runner = RecordingRunner::new().answering(Box::new(move |request| {
            if attempt_key(&request.invocation) == Some(1)
                && matches!(role_of(&request.invocation), Some(AttemptRole::Gate(_)))
            {
                panic!("the scaffold's gate fell over");
            }
            base(request)
        }));
        let mut wide =
            Wide::started_with("coordinator-panic", &tasks, 3, WidePlans::default(), runner);
        let error = drive(&mut wide, None).expect_err("the command ends");
        assert!(
            error.to_string().contains("a pipeline panicked"),
            "the panic became the pipeline's completion: {error}"
        );
        let gate = AttemptIdentities::new(TaskKey(1), GenerationId(0), AttemptNumber(1)).gate(0, 0);
        settled_but(&mut wide.run, &[gate]).unwrap_or_else(|ledger| {
            panic!(
                "the gate the panic unwound through never reported its end, so it keeps its \
                 registration, and every other invocation is settled once: {ledger}"
            )
        });
    }

    fn settled_but(run: &mut TopologyRun, held: &[InvocationId]) -> Result<(), String> {
        let ledger = run.broker_mut().invocations();
        let mut running: Vec<String> = ledger.running().into_iter().map(str::to_owned).collect();
        running.sort();
        let mut expected: Vec<String> = held.iter().map(InvocationId::render).collect();
        expected.sort();
        let mut holders: Vec<String> = ledger
            .slots()
            .holders()
            .into_iter()
            .map(InvocationId::render)
            .collect();
        holders.sort();
        let slotted: Vec<String> = held
            .iter()
            .filter(|invocation| crate::engine::topology::identity::is_slotted(invocation))
            .map(InvocationId::render)
            .collect();
        let settled = ledger.completed() + ledger.cancelled();
        if running != expected
            || holders != slotted
            || !ledger.pending().is_empty()
            || !ledger.slots().pending().is_empty()
            || settled + held.len() != ledger.registered()
            || ledger.duplicates() != 0
        {
            return Err(format!(
                "running {running:?} (expected {expected:?}), pair holders {holders:?}, pending \
                 {:?}, registered {}, settled {settled}, duplicates {}",
                ledger.pending(),
                ledger.registered(),
                ledger.duplicates()
            ));
        }
        Ok(())
    }

    #[test]
    fn an_unresolved_end_keeps_its_pair_and_no_waiting_invocation_starts_on_it() {
        let tasks = [
            WideTask::independent("alpha"),
            WideTask::independent("beta"),
        ];
        let alpha = AttemptIdentities::new(TaskKey(0), GenerationId(0), AttemptNumber(1)).worker();
        let beta = AttemptIdentities::new(TaskKey(1), GenerationId(0), AttemptNumber(1)).worker();
        let base = wide_responder(&tasks, &[]);
        let lost = alpha.clone();
        let runner = RecordingRunner::new().answering(Box::new(move |request| {
            if request.invocation == lost {
                return Err(crate::runner::RunnerError::unresolved(
                    &request.invocation,
                    UpstrokeError::Refused {
                        message: "the scaffold stopped supervising alpha's worker and could not \
                                  establish that it ended"
                            .to_owned(),
                    },
                ));
            }
            base(request)
        }));
        runner.hold();
        let mut wide = Wide::started_with(
            "coordinator-unresolved-pair",
            &tasks,
            2,
            WidePlans::default(),
            runner,
        );
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let mut scheduler = Scheduler::scripted(
            &runner,
            Box::new(|view: &Quiescent<'_>| {
                released(view, |invocation| worker(invocation) == Some(TaskKey(0)))
            }),
        );
        let mut hooks = wide.env.hooks();
        let pipelines =
            wide.env
                .pipelines_limited(crate::engine::topology::scaffold::SlotLimitsOf::Exactly(
                    1, 1,
                ));
        let error = wide
            .run
            .run_concurrently(
                &wide.env.seams(),
                &pipelines,
                &mut hooks,
                Some(&mut scheduler),
            )
            .expect_err("an unresolved process ends the command");
        drop(scheduler);
        assert!(
            !wide
                .env
                .runner
                .ran()
                .iter()
                .any(|ran| ran.invocation == beta),
            "beta's worker never started on the pair alpha's unresolved process holds: {:?}",
            wide.env.runner.endings()
        );
        let message = error.to_string();
        assert!(
            message.contains("could not establish that it ended"),
            "the command ends with alpha's own error: {message}"
        );
        let events = wide.env.durable_events();
        assert_eq!(
            kinds_of(&events).last(),
            Some(&"attempt_started"),
            "nothing is appended for either attempt: {:?}",
            kinds_of(&events)
        );
        settled_but(&mut wide.run, std::slice::from_ref(&alpha)).unwrap_or_else(|ledger| {
            panic!("alpha's worker keeps its registration and its pair, beta's request is withdrawn: {ledger}")
        });
        replay_equals_live(&wide);
    }

    #[test]
    fn an_unreported_end_keeps_its_pair_and_no_waiting_invocation_starts_on_it() {
        let tasks = [
            WideTask::independent("alpha"),
            WideTask::independent("beta"),
        ];
        let alpha = AttemptIdentities::new(TaskKey(0), GenerationId(0), AttemptNumber(1)).worker();
        let beta = AttemptIdentities::new(TaskKey(1), GenerationId(0), AttemptNumber(1)).worker();
        let runner = holding(&tasks, &[]);
        runner.panic_when_released(alpha.clone());
        let mut wide = Wide::started_with(
            "coordinator-unreported-end",
            &tasks,
            2,
            WidePlans::default(),
            runner,
        );
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let mut scheduler = Scheduler::scripted(
            &runner,
            Box::new(|view: &Quiescent<'_>| {
                released(view, |invocation| worker(invocation) == Some(TaskKey(0)))
            }),
        );
        let mut hooks = wide.env.hooks();
        let pipelines =
            wide.env
                .pipelines_limited(crate::engine::topology::scaffold::SlotLimitsOf::Exactly(
                    1, 1,
                ));
        let error = wide
            .run
            .run_concurrently(
                &wide.env.seams(),
                &pipelines,
                &mut hooks,
                Some(&mut scheduler),
            )
            .expect_err("the panic ends the command");
        drop(scheduler);
        assert!(
            !wide
                .env
                .runner
                .ran()
                .iter()
                .any(|ran| ran.invocation == beta),
            "beta's worker never started on the pair alpha's unreported invocation holds: {:?}",
            wide.env.runner.endings()
        );
        let message = error.to_string();
        assert!(
            message.contains("a pipeline panicked") && message.contains("fell over"),
            "alpha's own completion ends the command: {message}"
        );
        settled_but(&mut wide.run, std::slice::from_ref(&alpha)).unwrap_or_else(|ledger| {
            panic!("alpha's worker keeps its registration and its pair, beta's request is withdrawn: {ledger}")
        });
        replay_equals_live(&wide);
    }

    #[test]
    fn a_fatal_verification_completion_interrupts_on_receipt_without_waiting_for_the_snapshots() {
        let tasks = [
            WideTask::independent("alpha"),
            WideTask::independent("beta"),
            WideTask::independent("gamma"),
        ];
        let plans = WidePlans {
            panic_verifying: true,
            ..WidePlans::default()
        };
        let mut wide = Wide::started_with(
            "coordinator-fatal-verification",
            &tasks,
            3,
            plans,
            holding(&tasks, &[]),
        );
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let mut waited_after_arrival: Vec<Vec<InvocationId>> = Vec::new();
        let mut scheduler = Scheduler::scripted(
            &runner,
            Box::new(|view: &Quiescent<'_>| {
                let verifying = view.run.fold().transaction().is_some();
                let verification_live = view
                    .live
                    .iter()
                    .any(|(_, identity)| matches!(identity, Identity::Verification { .. }));
                if verifying && !verification_live {
                    waited_after_arrival.push(view.invoking.clone());
                }
                if count(view.run.events(), "task_merged") == 0 {
                    return released(view, |invocation| attempt_key(invocation) == Some(0));
                }
                released(view, |invocation| worker(invocation) == Some(TaskKey(2)))
                    .or_else(|| released(view, |invocation| attempt_key(invocation) == Some(1)))
                    .or_else(|| released(view, |invocation| attempt_key(invocation) == Some(2)))
            }),
        );
        let error = drive(&mut wide, Some(&mut scheduler)).expect_err("the panic ends the command");
        drop(scheduler);
        assert!(
            waited_after_arrival.is_empty(),
            "the coordinator never waited, with the fatal completion received, for gamma's \
             snapshot to drain: it waited at {waited_after_arrival:?}"
        );
        let message = error.to_string();
        assert!(
            message.contains("a pipeline panicked") && message.contains("fell over"),
            "the verification's own error ends the command: {message}"
        );
        let review =
            AttemptIdentities::new(TaskKey(2), GenerationId(0), AttemptNumber(1)).review_pass(0, 0);
        assert!(
            wide.env
                .runner
                .endings()
                .contains(&(review, crate::engine::topology::scaffold::Ending::Cancelled)),
            "gamma's review, held when the panic arrived, was cancelled: {:?}",
            wide.env.runner.endings()
        );
        let events = wide.env.durable_events();
        assert_eq!(
            kinds_of(&events).last(),
            Some(&"merge_verification_started"),
            "nothing is appended after the verification started: {:?}",
            kinds_of(&events)
        );
        assert!(wide.run.invocations_balance(), "{:?}", wide.run.warnings());
        replay_equals_live(&wide);
    }

    fn sibling_parked_on(
        run: &TopologyRun,
        root: TaskKey,
        source: CandidateRef,
    ) -> Vec<TopologyEventBody> {
        use crate::topology::events::{
            AttemptFinished4, AttemptSettlement, AttemptStarted4, FrozenQuestion, FrozenSpawn,
            LeaseDisposition, LeaseGrant, SettlementTransition, SpawnAdmission, TaskDispatched,
            TaskSpawned,
        };
        use crate::topology::registry::{Lineage, Origin, repair_display_id};
        let fold = run.fold();
        let registry = fold.registry().expect("a started run has a registry");
        let key = TaskKey(u32::try_from(registry.len()).expect("a small registry"));
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
        let binding = fold.rung_binding(root, 0).expect("the root's first rung");
        vec![
            TopologyEventBody::TaskSpawned {
                data: Box::new(TaskSpawned {
                    spawn: FrozenSpawn {
                        key,
                        entry,
                        admission: SpawnAdmission::Runnable,
                    },
                }),
            },
            TopologyEventBody::TaskDispatched {
                data: TaskDispatched {
                    key,
                    generation: GenerationId(0),
                    base_sha: source.commit_sha.clone(),
                    worktree_path: format!("tasks/k{}-g0", key.0),
                    lease: LeaseGrant::InheritedLineage { root },
                    source_candidate: Some(source),
                },
            },
            TopologyEventBody::AttemptStarted {
                data: AttemptStarted4 {
                    key,
                    generation: GenerationId(0),
                    attempt: AttemptNumber(1),
                    rung: 0,
                    binding,
                    pool: None,
                    resume_session: None,
                    materialization_observed: Some(crate::topology::events::Materialization::Clean),
                },
            },
            TopologyEventBody::AttemptFinished {
                data: Box::new(AttemptFinished4 {
                    key,
                    generation: GenerationId(0),
                    attempt: AttemptNumber(1),
                    record: Box::new(crate::events::AttemptRecord {
                        attempt: 1,
                        tier: "mid".to_owned(),
                        model: "scaffold-model".to_owned(),
                        pool: None,
                        resumed: false,
                        duration: Duration::from_millis(5),
                        cost_usd: Some(0.0),
                        reviews: Vec::new(),
                        session_id: None,
                        usage: None,
                        failure: Some(crate::events::FailureRecord {
                            kind: crate::ladder::FailureKind::NeedsHuman,
                            origin: crate::ladder::FailureOrigin::Worker,
                            reason: "the sibling asks which of two formats to keep".to_owned(),
                            detail: None,
                        }),
                    }),
                    settlement: AttemptSettlement::Closed {
                        transition: SettlementTransition::Parked {
                            question: FrozenQuestion {
                                id: crate::ir::QuestionId("sibling".to_owned()),
                                key,
                                kind: crate::ir::QuestionKind::Clarify,
                                context: "the sibling asks which of two formats to keep".to_owned(),
                                options: vec!["keep the first".to_owned()],
                            },
                        },
                        lease: LeaseDisposition::LineageHeld,
                    },
                }),
            },
        ]
    }

    #[test]
    fn a_declined_embedded_question_stops_the_verification_its_lineage_had_started() {
        let tasks = [
            WideTask::independent("alpha"),
            WideTask::independent("beta"),
            WideTask::independent("gamma"),
        ];
        let armed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut wide = Wide::started_with(
            "coordinator-declined-verification",
            &tasks,
            3,
            WidePlans::default(),
            holding(&tasks, &[]),
        );
        wide.env.answers = std::sync::Arc::new(DecliningOnceArmed(std::sync::Arc::clone(&armed)));
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let arming = std::sync::Arc::clone(&armed);
        let mut planted: Option<Vec<TopologyEventBody>> = None;
        let mut scheduler = Scheduler::scripted(
            &runner,
            Box::new(move |view: &Quiescent<'_>| {
                if count(view.run.events(), "task_merged") == 0 {
                    return released(view, |invocation| attempt_key(invocation) == Some(0));
                }
                let verifying = view
                    .live
                    .iter()
                    .any(|(_, identity)| matches!(identity, Identity::Verification { .. }));
                if verifying && planted.is_none() {
                    let source = view
                        .run
                        .fold()
                        .transaction()
                        .map(|open| open.candidate.clone())
                        .expect("beta's candidate is being verified");
                    planted = Some(sibling_parked_on(view.run, TaskKey(1), source));
                }
                if let Some(events) = planted.as_mut() {
                    if !events.is_empty() {
                        return Some(Release::Append(Box::new(events.remove(0))));
                    }
                    arming.store(true, std::sync::atomic::Ordering::SeqCst);
                    return released(view, |invocation| attempt_key(invocation) == Some(2))
                        .or_else(|| {
                            released(view, |invocation| {
                                matches!(invocation, InvocationId::Sequence { .. })
                            })
                        });
                }
                released(view, |invocation| attempt_key(invocation) == Some(1)).or_else(|| {
                    released(view, |invocation| {
                        matches!(invocation, InvocationId::Sequence { .. })
                    })
                })
            }),
        );
        let progress = drive(&mut wide, Some(&mut scheduler))
            .expect("the decline fails beta's lineage and the run goes on to its end");
        drop(scheduler);
        assert_eq!(outcome_of(&progress), RunOutcome::Complete);
        let events = wide.env.durable_events();
        let kinds = kinds_of(&events);
        let decline = position(&kinds, "question_answered", 0);
        let first = SequenceId(1);
        assert!(
            !events.get(decline..).unwrap_or_default().iter().any(|event| {
                matches!(&event.body,
                    TopologyEventBody::MergeVerificationStarted { data } if data.sequence == first)
                    || matches!(&event.body,
                        TopologyEventBody::MergeVerificationUnavailable { data } if data.sequence == first)
                    || matches!(&event.body,
                        TopologyEventBody::MergeVerificationInterrupted { data } if data.sequence == first)
                    || matches!(&event.body,
                        TopologyEventBody::MergePrepared { data } if data.sequence == first)
                    || matches!(&event.body,
                        TopologyEventBody::MergeRejected { data } if data.sequence == first)
                    || matches!(&event.body,
                        TopologyEventBody::TaskMerged { data } if data.sequence == first)
            }),
            "nothing is appended for the cancelled verification after the decline: {kinds:?}"
        );
        let gate = SequenceIdentities::new(first).gate(0, 0);
        assert!(
            wide.env
                .runner
                .endings()
                .contains(&(gate, crate::engine::topology::scaffold::Ending::Cancelled)),
            "the verification's gate, held when the decline was ingested, was cancelled: {:?}",
            wide.env.runner.endings()
        );
        assert!(
            wide.run
                .warnings()
                .iter()
                .any(|warning| warning.contains("no longer holds its transaction open")),
            "{:?}",
            wide.run.warnings()
        );
        assert_eq!(
            count(&events, "task_merged"),
            2,
            "alpha and gamma merge; beta's lineage failed: {kinds:?}"
        );
        assert!(wide.run.invocations_balance(), "{:?}", wide.run.warnings());
        replay_equals_live(&wide);
    }

    #[test]
    fn a_declined_embedded_question_stops_a_running_sibling_attempt() {
        let tasks = [
            WideTask::independent("alpha"),
            WideTask::independent("beta"),
        ];
        let armed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut wide = Wide::started_with(
            "coordinator-declined-sibling",
            &tasks,
            2,
            WidePlans::default(),
            holding(&tasks, &[]),
        );
        wide.env.answers = std::sync::Arc::new(DecliningOnceArmed(std::sync::Arc::clone(&armed)));
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let arming = std::sync::Arc::clone(&armed);
        let head = wide.env.head(&wide.run);
        let mut planted: Option<Vec<TopologyEventBody>> = None;
        let mut scheduler = Scheduler::scripted(
            &runner,
            Box::new(move |view: &Quiescent<'_>| {
                let beta_held = view
                    .invoking
                    .iter()
                    .any(|invocation| worker(invocation) == Some(TaskKey(1)));
                if beta_held && planted.is_none() {
                    let source = CandidateRef {
                        key: TaskKey(1),
                        generation: GenerationId(0),
                        commit_sha: crate::topology::events::CommitSha(head.clone()),
                        candidate_ref: crate::topology::events::GitRef(
                            "refs/upstroke/planted/sibling-source".to_owned(),
                        ),
                    };
                    planted = Some(sibling_parked_on(view.run, TaskKey(1), source));
                }
                if let Some(events) = planted.as_mut() {
                    if !events.is_empty() {
                        return Some(Release::Append(Box::new(events.remove(0))));
                    }
                    arming.store(true, std::sync::atomic::Ordering::SeqCst);
                }
                released(view, |invocation| attempt_key(invocation) == Some(0))
                    .or_else(|| released(view, |invocation| attempt_key(invocation) == Some(1)))
                    .or_else(|| {
                        released(view, |invocation| {
                            matches!(invocation, InvocationId::Sequence { .. })
                        })
                    })
            }),
        );
        let progress = drive(&mut wide, Some(&mut scheduler))
            .expect("the decline fails beta's lineage and the run goes on to its end");
        drop(scheduler);
        assert_eq!(outcome_of(&progress), RunOutcome::Complete);
        let beta = AttemptIdentities::new(TaskKey(1), GenerationId(0), AttemptNumber(1));
        assert!(
            wide.env.runner.endings().contains(&(
                beta.worker(),
                crate::engine::topology::scaffold::Ending::Cancelled
            )),
            "beta's worker, held when the decline was ingested, was cancelled: {:?}",
            wide.env.runner.endings()
        );
        assert!(
            !wide
                .env
                .runner
                .ran()
                .iter()
                .any(|ran| attempt_key(&ran.invocation) == Some(1)
                    && ran.invocation != beta.worker()),
            "no gate or review of beta's closed generation ever started: {:?}",
            wide.env.runner.endings()
        );
        let events = wide.env.durable_events();
        assert!(
            !events.iter().any(|event| matches!(&event.body,
                TopologyEventBody::CandidatePrepared { data } if data.key == TaskKey(1))),
            "beta's late result was never settled: {:?}",
            kinds_of(&events)
        );
        assert_eq!(count(&events, "task_merged"), 1, "{:?}", kinds_of(&events));
        assert!(wide.run.invocations_balance(), "{:?}", wide.run.warnings());
        replay_equals_live(&wide);
    }

    fn bounded(what: &'static str, body: impl FnOnce() + Send + 'static) {
        let name = std::thread::current()
            .name()
            .unwrap_or("coordinator-bounded")
            .to_owned();
        let (done, finished) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name(name)
            .spawn(move || {
                body();
                let _ = done.send(());
            })
            .expect("a thread for the bounded scenario");
        match finished.recv_timeout(BOUND) {
            Ok(()) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => panic!(
                "{what} did not end within {BOUND:?}: the coordinator waited on its inbox for a \
                 pipeline that was itself waiting on the coordinator"
            ),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                panic!("{what} failed; its own panic is reported above")
            }
        }
    }

    #[test]
    fn a_halt_answers_a_request_its_intake_still_buffers() {
        bounded("the halted run", || {
            let tasks = [
                WideTask::independent("alpha"),
                WideTask::independent("beta"),
            ];
            let armed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let runner = RecordingRunner::new().answering(
                crate::engine::topology::scaffold::wide_responder_asking(&tasks, &[], &[0]),
            );
            runner.hold();
            let mut wide = Wide::started_with(
                "coordinator-halt-buffered",
                &tasks,
                2,
                WidePlans::default(),
                runner,
            );
            halting(&mut wide.env);
            wide.env.answers =
                std::sync::Arc::new(DecliningOnceArmed(std::sync::Arc::clone(&armed)));
            let runner = std::sync::Arc::clone(&wide.env.runner);
            let mut scheduler = Scheduler::scripted(
                &runner,
                Box::new(move |view: &Quiescent<'_>| {
                    let parked = view
                        .run
                        .fold()
                        .open_questions()
                        .is_some_and(|open| !open.is_empty());
                    if !parked {
                        return released(view, |invocation| worker(invocation) == Some(TaskKey(0)));
                    }
                    armed.store(true, std::sync::atomic::Ordering::SeqCst);
                    released(view, |invocation| attempt_key(invocation) == Some(1))
                }),
            );
            let progress = drive(&mut wide, Some(&mut scheduler)).expect("the halt ends the run");
            drop(scheduler);
            assert_eq!(outcome_of(&progress), RunOutcome::Halted);
            let events = wide.env.durable_events();
            let kinds = kinds_of(&events);
            assert_eq!(
                count(&events, "attempt_interrupted"),
                1,
                "beta, whose snapshot request was buffered when the halt cancelled it, is settled \
                 interrupted: {kinds:?}"
            );
            assert!(wide.run.invocations_balance(), "{:?}", wide.run.warnings());
            replay_equals_live(&wide);
        });
    }

    #[test]
    fn a_stop_answers_a_request_its_intake_still_buffers() {
        bounded("the run with a stopped verification", || {
            let tasks = [
                WideTask::independent("alpha"),
                WideTask::independent("beta"),
            ];
            let armed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let mut wide = Wide::started_with(
                "coordinator-stop-buffered",
                &tasks,
                3,
                WidePlans::default(),
                holding(&tasks, &[]),
            );
            wide.env.answers =
                std::sync::Arc::new(DecliningOnceArmed(std::sync::Arc::clone(&armed)));
            let runner = std::sync::Arc::clone(&wide.env.runner);
            let mut planted: Option<Vec<TopologyEventBody>> = None;
            let mut scheduler = Scheduler::scripted(
                &runner,
                Box::new(move |view: &Quiescent<'_>| {
                    if count(view.run.events(), "task_merged") == 0 {
                        return released(view, |invocation| attempt_key(invocation) == Some(0));
                    }
                    let verifying = view
                        .live
                        .iter()
                        .any(|(_, identity)| matches!(identity, Identity::Verification { .. }));
                    if verifying && planted.is_none() {
                        let source = view
                            .run
                            .fold()
                            .transaction()
                            .map(|open| open.candidate.clone())
                            .expect("beta's candidate is being verified");
                        planted = Some(sibling_parked_on(view.run, TaskKey(1), source));
                    }
                    if let Some(events) = planted.as_mut() {
                        if !events.is_empty() {
                            return Some(Release::Append(Box::new(events.remove(0))));
                        }
                        armed.store(true, std::sync::atomic::Ordering::SeqCst);
                    }
                    released(view, |invocation| attempt_key(invocation) == Some(1)).or_else(|| {
                        released(view, |invocation| {
                            matches!(invocation, InvocationId::Sequence { .. })
                        })
                    })
                }),
            );
            let progress = drive(&mut wide, Some(&mut scheduler))
                .expect("the decline fails beta's lineage and the run goes on to its end");
            drop(scheduler);
            assert_eq!(outcome_of(&progress), RunOutcome::Complete);
            assert!(
                !wide.env.runner.ran().iter().any(
                    |ran| matches!(ran.invocation, InvocationId::Sequence { role, .. }
                        if role != crate::runner::invocation::SequenceRole::Gate(0))
                ),
                "the verification's review, requested before the decline and applied after it, \
                 never started: {:?}",
                wide.env.runner.endings()
            );
            assert!(wide.run.invocations_balance(), "{:?}", wide.run.warnings());
            replay_equals_live(&wide);
        });
    }

    struct DecliningAtPoll(std::sync::Arc<std::sync::atomic::AtomicUsize>);

    impl crate::interaction::AnswerSource for DecliningAtPoll {
        fn id(&self) -> &'static str {
            "declining-at-poll"
        }

        fn resolve(
            &self,
            question: &crate::ir::Question,
        ) -> Result<crate::ir::Answer, UpstrokeError> {
            self.poll(question)
        }

        fn poll(
            &self,
            _question: &crate::ir::Question,
        ) -> Result<crate::ir::Answer, UpstrokeError> {
            let armed = self.0.load(std::sync::atomic::Ordering::SeqCst);
            self.0
                .store(armed.saturating_sub(1), std::sync::atomic::Ordering::SeqCst);
            Ok(if armed == 1 {
                crate::ir::Answer::Declined
            } else {
                crate::ir::Answer::Unanswered
            })
        }
    }

    #[test]
    fn a_halt_after_the_verifications_result_arrived_interrupts_it_and_publishes_nothing() {
        let mut wide = halting_on_gamma("coordinator-halt-after-arrival");
        let polls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        wide.env.answers = std::sync::Arc::new(DecliningAtPoll(std::sync::Arc::clone(&polls)));
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let run_id = wide.run.fold().started().expect("started").run_id.clone();
        let manager = wide.env.fixture.manager.clone();
        let mut last_released: Option<(SequenceId, Option<String>)> = None;
        let mut scheduler = Scheduler::scripted(
            &runner,
            Box::new(|view: &Quiescent<'_>| {
                if !view.run.fold().questions_open() {
                    return released(view, |invocation| attempt_key(invocation) == Some(2));
                }
                if count(view.run.events(), "task_merged") == 0 {
                    return released(view, |invocation| attempt_key(invocation) == Some(0));
                }
                let Some(open) = view.run.fold().transaction() else {
                    return released(view, |invocation| attempt_key(invocation) == Some(1));
                };
                let last = released(view, |invocation| {
                    matches!(
                        invocation,
                        InvocationId::Sequence {
                            role: crate::runner::invocation::SequenceRole::ReviewPass(0),
                            ..
                        }
                    )
                })?;
                assert_eq!(
                    view.live.len(),
                    1,
                    "the verification is the only live pipeline: {:?}",
                    view.live
                );
                assert!(last_released.is_none(), "its last process is released once");
                let pin =
                    crate::engine::topology::integrate::prepared_pin_ref(&run_id, open.sequence);
                last_released = Some((
                    open.sequence,
                    manager
                        .direct_ref_target(pin.as_str())
                        .expect("the pin is readable"),
                ));
                polls.store(2, std::sync::atomic::Ordering::SeqCst);
                Some(last)
            }),
        );
        let (progress, watching) = drive_watching(&mut wide, &mut scheduler);
        drop(scheduler);
        let progress = progress.expect("the halted run closes and ends");
        assert_eq!(outcome_of(&progress), RunOutcome::Halted);
        let (sequence, pinned) =
            last_released.expect("the verification's last process was released");
        assert!(
            pinned.is_some(),
            "the verification held its pin while it ran"
        );

        let events = wide.env.durable_events();
        let kinds = kinds_of(&events);
        let halted = position(&kinds, "question_answered", 0);
        assert_eq!(
            &kinds[halted..],
            &[
                "question_answered",
                "merge_verification_interrupted",
                "run_finished"
            ],
            "the verification whose result had arrived is settled interrupted by the closure, \
             never prepared: {kinds:?}"
        );
        assert!(
            !events.iter().any(|event| matches!(&event.body,
                TopologyEventBody::MergePrepared { data } if data.sequence == sequence)
                || matches!(&event.body,
                    TopologyEventBody::TaskMerged { data } if data.sequence == sequence)),
            "halting never publishes unverified work: {kinds:?}"
        );
        assert!(
            events.iter().any(|event| matches!(&event.body,
                TopologyEventBody::MergeVerificationInterrupted { data }
                    if data.sequence == sequence && data.detail.contains("halted"))),
            "{kinds:?}"
        );
        assert_eq!(
            wide.run.discarded(),
            0,
            "the verification's result was accepted before the halt was ingested, not \
             discarded as a cancelled pipeline's: {:?}",
            wide.run.warnings()
        );
        let verification_endings: Vec<_> = wide
            .env
            .runner
            .endings()
            .into_iter()
            .filter(|(invocation, _)| matches!(invocation, InvocationId::Sequence { .. }))
            .collect();
        assert!(
            !verification_endings.is_empty()
                && verification_endings.iter().all(|(_, ending)| {
                    *ending == crate::engine::topology::scaffold::Ending::Completed
                }),
            "every process of the verification ran to its end before the halt: \
             {verification_endings:?}"
        );
        let pin = crate::engine::topology::integrate::prepared_pin_ref(&run_id, sequence);
        assert_eq!(
            manager
                .direct_ref_target(pin.as_str())
                .expect("the pin is readable"),
            None,
            "the prepared pin was deleted expected-old"
        );
        let at_end = watching.intents_when("run_finished");
        assert!(
            !at_end
                .iter()
                .any(|slot| matches!(slot, crate::workspace_manager::Slot::Staging { .. }))
                && snapshot_names(at_end).is_empty(),
            "before `run_finished` the closure had removed the staging and this sequence's \
             snapshots: {at_end:?}"
        );
        assert!(wide.run.fold().transaction().is_none());
        assert!(wide.run.invocations_balance(), "{:?}", wide.run.warnings());
        assert_eq!(wide.run.broker_duplicates(), 0);
        replay_equals_live(&wide);
    }

    #[test]
    fn a_decline_that_cancels_the_open_verification_goes_on_to_integrate_the_queued_candidate() {
        let tasks = [
            WideTask::independent("alpha"),
            WideTask::independent("beta"),
            WideTask::independent("gamma"),
            WideTask::independent("delta"),
        ];
        let armed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut wide = Wide::started_with(
            "coordinator-declined-with-queued",
            &tasks,
            3,
            WidePlans::default(),
            holding(&tasks, &[]),
        );
        wide.env.answers = std::sync::Arc::new(DecliningOnceArmed(std::sync::Arc::clone(&armed)));
        let runner = std::sync::Arc::clone(&wide.env.runner);
        let mut planted: Option<Vec<TopologyEventBody>> = None;
        let mut at_decline: Option<(SequenceId, bool)> = None;
        let mut scheduler = Scheduler::scripted(
            &runner,
            Box::new(|view: &Quiescent<'_>| {
                if count(view.run.events(), "task_merged") == 0 {
                    return released(view, |invocation| attempt_key(invocation) == Some(0));
                }
                if at_decline.is_some() {
                    return None;
                }
                let Some(open) = view.run.fold().transaction() else {
                    return released(view, |invocation| attempt_key(invocation) == Some(1));
                };
                let queued = view
                    .run
                    .fold()
                    .queue()
                    .is_some_and(|queue| queue.holds_task(TaskKey(2)));
                if !queued {
                    return released(view, |invocation| attempt_key(invocation) == Some(2));
                }
                let events = planted.get_or_insert_with(|| {
                    sibling_parked_on(view.run, TaskKey(1), open.candidate.clone())
                });
                if !events.is_empty() {
                    return Some(Release::Append(Box::new(events.remove(0))));
                }
                let gate = SequenceIdentities::new(open.sequence).gate(0, 0);
                at_decline = Some((open.sequence, view.invoking.contains(&gate)));
                armed.store(true, std::sync::atomic::Ordering::SeqCst);
                released(view, |invocation| worker(invocation) == Some(TaskKey(3)))
            }),
        );
        let progress = drive(&mut wide, Some(&mut scheduler))
            .expect("the decline fails beta's lineage and the run goes on to its end");
        drop(scheduler);
        let (first, gate_held) = at_decline.expect("the decline was armed");
        assert!(
            gate_held,
            "the verification's gate was held when the decline was armed"
        );
        assert_eq!(outcome_of(&progress), RunOutcome::Complete);
        let events = wide.env.durable_events();
        let kinds = kinds_of(&events);
        let decline = position(&kinds, "question_answered", 0);
        let queued_before = events
            .iter()
            .take(decline)
            .any(|event| matches!(&event.body,
                TopologyEventBody::TaskCandidateCreated { data } if data.candidate.key == TaskKey(2)));
        assert!(
            queued_before,
            "gamma's candidate was queued before the decline: {kinds:?}"
        );
        let after = events.get(decline..).unwrap_or_default();
        assert!(
            !after.iter().any(|event| {
                matches!(&event.body,
                    TopologyEventBody::MergeVerificationStarted { data } if data.sequence == first)
                    || matches!(&event.body,
                        TopologyEventBody::MergeVerificationUnavailable { data } if data.sequence == first)
                    || matches!(&event.body,
                        TopologyEventBody::MergeVerificationInterrupted { data } if data.sequence == first)
                    || matches!(&event.body,
                        TopologyEventBody::MergePrepared { data } if data.sequence == first)
                    || matches!(&event.body,
                        TopologyEventBody::MergeRejected { data } if data.sequence == first)
                    || matches!(&event.body,
                        TopologyEventBody::TaskMerged { data } if data.sequence == first)
            }),
            "nothing is appended for the cancelled verification after the decline: {kinds:?}"
        );
        let gamma = after
            .iter()
            .find_map(|event| match &event.body {
                TopologyEventBody::MergePrepared { data } if data.key == TaskKey(2) => {
                    Some(data.sequence)
                }
                _ => None,
            })
            .expect("gamma's queued candidate was integrated after the decline");
        assert!(
            after.iter().any(|event| matches!(&event.body,
                TopologyEventBody::TaskMerged { data } if data.sequence == gamma)),
            "{kinds:?}"
        );
        let gate = SequenceIdentities::new(first).gate(0, 0);
        assert!(
            wide.env
                .runner
                .endings()
                .contains(&(gate, crate::engine::topology::scaffold::Ending::Cancelled)),
            "the verification's gate, held when the decline was ingested, was cancelled: {:?}",
            wide.env.runner.endings()
        );
        assert!(
            wide.run
                .warnings()
                .iter()
                .any(|warning| warning.contains("no longer holds its transaction open")),
            "{:?}",
            wide.run.warnings()
        );
        assert_eq!(
            count(&events, "task_merged"),
            3,
            "alpha, gamma and delta merge; beta's lineage failed: {kinds:?}"
        );
        assert!(wide.run.invocations_balance(), "{:?}", wide.run.warnings());
        replay_equals_live(&wide);
    }

    #[derive(Default)]
    struct GrantLog(Vec<InvocationId>);

    impl Quiescence for GrantLog {
        fn granted(&mut self, invocation: &InvocationId) {
            self.0.push(invocation.clone());
        }

        fn quiescent(&mut self, _view: &Quiescent<'_>) -> Release {
            Release::Nothing
        }
    }

    struct Siblings {
        root: InvocationId,
        sibling: InvocationId,
        holder: InvocationId,
    }

    type Answer = oneshot::Receiver<Result<(), UpstrokeError>>;

    struct ClosedSiblings<T> {
        wide: Wide,
        ids: Siblings,
        granted: Vec<InvocationId>,
        root: Answer,
        sibling: Answer,
        found: T,
    }

    fn attempt_identity(key: u32) -> Identity {
        Identity::Attempt {
            key: TaskKey(key),
            generation: GenerationId(0),
            attempt: AttemptNumber(1),
        }
    }

    fn with_two_closed_siblings<T>(
        tag: &str,
        body: impl FnOnce(&mut Coordinator<'_>, &Siblings) -> T,
    ) -> ClosedSiblings<T> {
        let tasks = [
            WideTask::independent("root"),
            WideTask::independent("holder"),
        ];
        let mut wide =
            Wide::started_with(tag, &tasks, 4, WidePlans::default(), holding(&tasks, &[]));
        wide.run
            .limit_slots(SlotLimits::new(1, 1).expect("positive limits"))
            .expect("an empty broker");
        let armed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        wide.env.answers = std::sync::Arc::new(DecliningOnceArmed(std::sync::Arc::clone(&armed)));
        let mut hooks = wide.env.hooks();
        let seams = wide.env.seams();
        for key in [TaskKey(0), TaskKey(1)] {
            wide.run
                .begin_dispatch(key, GenerationId(0), false, &seams, &mut hooks)
                .expect("the root and the unrelated holder start");
        }
        let source = CandidateRef {
            key: TaskKey(0),
            generation: GenerationId(0),
            commit_sha: crate::topology::events::CommitSha(wide.env.head(&wide.run)),
            candidate_ref: crate::topology::events::GitRef(
                "refs/upstroke/planted/sibling-source".to_owned(),
            ),
        };
        let mut running = sibling_parked_on(&wide.run, TaskKey(0), source.clone());
        running.pop();
        for event in running {
            wide.run
                .emit(event, &seams, &mut hooks)
                .expect("a fold-valid running sibling");
        }
        let ids = Siblings {
            root: AttemptIdentities::new(TaskKey(0), GenerationId(0), AttemptNumber(1)).worker(),
            sibling: AttemptIdentities::new(TaskKey(2), GenerationId(0), AttemptNumber(1)).worker(),
            holder: AttemptIdentities::new(TaskKey(1), GenerationId(0), AttemptNumber(1)).worker(),
        };
        for (invocation, agent, pool, admission) in [
            (&ids.holder, "a", None, Admission::Granted),
            (&ids.root, "a", Some("p"), Admission::Pending),
            (&ids.sibling, "b", Some("p"), Admission::Pending),
        ] {
            let standing = Standing::of(wide.run.fold(), invocation);
            let pair = SlotPair {
                agent: agent.to_owned(),
                pool: pool.map(str::to_owned),
            };
            assert_eq!(
                wide.run
                    .broker_mut()
                    .register(&standing, invocation, Some(pair))
                    .expect("the identity is open"),
                admission,
                "`{invocation}`"
            );
        }
        let mut asking = sibling_parked_on(&wide.run, TaskKey(0), source);
        if let Some(TopologyEventBody::TaskSpawned { data }) = asking.first_mut() {
            data.spawn.entry.display_id = crate::ir::TaskId::from(
                crate::topology::registry::repair_display_id(2, &crate::ir::TaskId::from("root"))
                    .as_str(),
            );
            if let Some(lineage) = data.spawn.entry.lineage.as_mut() {
                lineage.index = 2;
            }
        }
        for event in asking {
            wide.run
                .emit(event, &seams, &mut hooks)
                .expect("a fold-valid embedded question");
        }
        armed.store(true, std::sync::atomic::Ordering::SeqCst);
        wide.run
            .ingest_answers(&seams, &mut hooks)
            .expect("the decline is ingested")
            .expect("an answer");
        assert!(
            !wide.run.fold().run_is_ending(),
            "the decline does not halt"
        );

        let pipelines = wide.env.pipelines();
        let (outbox, inbox) = mpsc::unbounded_channel();
        let (injector, injected) = mpsc::unbounded_channel();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .build()
            .expect("a runtime");
        let (root_reply, root) = oneshot::channel();
        let (sibling_reply, sibling) = oneshot::channel();
        let live = BTreeMap::from([
            (
                PipelineId(1),
                Live {
                    identity: attempt_identity(0),
                    cancel: Cancellation::new(),
                    cancelled: false,
                    busy: Busy::Awaiting,
                    running: None,
                    job: None,
                },
            ),
            (
                PipelineId(2),
                Live {
                    identity: attempt_identity(2),
                    cancel: Cancellation::new(),
                    cancelled: false,
                    busy: Busy::Awaiting,
                    running: None,
                    job: None,
                },
            ),
            (
                PipelineId(3),
                Live {
                    identity: attempt_identity(1),
                    cancel: Cancellation::new(),
                    cancelled: false,
                    busy: Busy::Invoking(ids.holder.clone()),
                    running: Some(ids.holder.clone()),
                    job: None,
                },
            ),
        ]);
        let replies = BTreeMap::from([
            (ids.root.clone(), (PipelineId(1), root_reply)),
            (ids.sibling.clone(), (PipelineId(2), sibling_reply)),
        ]);
        let mut log = GrantLog::default();
        let found = {
            let mut coordinator = Coordinator {
                run: &mut wide.run,
                seams: &seams,
                hooks: &mut hooks,
                pipelines: &pipelines,
                observer: Some(&mut log),
                leases: Vec::new(),
                live,
                replies,
                gate: SnapshotGate::default(),
                next: 3,
                in_verify: None,
                arrived: None,
                abandoned: None,
                interrupt: None,
                cancelled_work: closure::Cancelled::none(),
                unresolved: Vec::new(),
                buffer: Vec::new(),
                arrivals: 0,
                handles: Vec::new(),
                inbox,
                outbox,
                injected,
                injector: Injector(injector),
                runtime,
            };
            assert!(
                coordinator.live.iter().all(|(pipeline, live)| {
                    live.identity.open_in(coordinator.run) == (*pipeline == PipelineId(3))
                }),
                "the decline closed the root's and the sibling's attempts, not the holder's"
            );
            body(&mut coordinator, &ids)
        };
        ClosedSiblings {
            wide,
            ids,
            granted: log.0,
            root,
            sibling,
            found,
        }
    }

    fn answered(answer: &mut Answer) -> Option<bool> {
        answer.try_recv().ok().map(|result| result.is_ok())
    }

    #[test]
    fn stopping_two_closed_pipelines_grants_neither_the_pair_the_first_one_frees() {
        let mut closed =
            with_two_closed_siblings("coordinator-reconcile-grants", |coordinator, _| {
                coordinator.reconcile();
                (
                    coordinator
                        .live
                        .get(&PipelineId(2))
                        .map(|live| live.cancelled),
                    coordinator.run.warnings().to_vec(),
                )
            });
        let (sibling_cancelled, warnings) = closed.found;
        assert_eq!(
            answered(&mut closed.root),
            Some(false),
            "the root's pending request was refused"
        );
        assert_eq!(
            answered(&mut closed.sibling),
            Some(false),
            "the sibling's request, granted the pair the root's withdrawal freed, was refused, \
             not answered with the grant: granted {:?}",
            closed.granted
        );
        assert!(
            closed.granted.is_empty(),
            "no invocation of a closed identity was handed out as granted: {:?}",
            closed.granted
        );
        assert_eq!(sibling_cancelled, Some(true), "the sibling was stopped too");
        assert!(
            warnings.iter().any(|warning| warning.contains(&format!(
                "the broker granted `{}` to pipeline 2",
                closed.ids.sibling
            ))),
            "{warnings:?}"
        );
        let ledger = closed.wide.run.broker_mut().invocations();
        assert!(
            ledger.settled(&closed.ids.root) && ledger.settled(&closed.ids.sibling),
            "both closed requests are settled cancelled: running {:?}, pending {:?}",
            ledger.running(),
            ledger.pending()
        );
        assert!(
            !ledger.slots().holds(&closed.ids.sibling) && ledger.slots().holds(&closed.ids.holder),
            "the sibling holds no pair and the unrelated holder keeps its own"
        );
        assert_eq!(ledger.running(), vec![closed.ids.holder.render().as_str()]);
        assert!(ledger.pending().is_empty());
    }

    #[test]
    fn a_pipeline_whose_identity_closed_is_granted_nothing_before_it_is_stopped() {
        let sibling_gate =
            AttemptIdentities::new(TaskKey(2), GenerationId(0), AttemptNumber(1)).gate(0, 0);
        let mut closed =
            with_two_closed_siblings("coordinator-closed-granted-nothing", |coordinator, ids| {
                let (admit_reply, mut admitted) = oneshot::channel();
                coordinator
                    .handle(
                        Origin::Pipeline,
                        ToCoordinator::Admit {
                            pipeline: PipelineId(2),
                            invocation: sibling_gate.clone(),
                            pair: None,
                            reply: admit_reply,
                        },
                    )
                    .expect("an admission is handled");
                let (snapshot_reply, mut snapshot) = oneshot::channel();
                coordinator
                    .handle(
                        Origin::Pipeline,
                        ToCoordinator::SnapshotBegin {
                            pipeline: PipelineId(2),
                            reply: snapshot_reply,
                        },
                    )
                    .expect("a snapshot request is handled");
                let (waiting_reply, mut waited) = oneshot::channel();
                coordinator.gate.mode = GateMode::Closed;
                coordinator
                    .gate
                    .waiting
                    .push_back((PipelineId(2), waiting_reply));
                coordinator.open_gate();
                coordinator
                    .handle(
                        Origin::Pipeline,
                        ToCoordinator::Ended {
                            pipeline: PipelineId(3),
                            invocation: ids.holder.clone(),
                            end: InvocationEnd::Completed,
                        },
                    )
                    .expect("an end is handled");
                (
                    answered(&mut admitted),
                    answered(&mut snapshot),
                    answered(&mut waited),
                    coordinator.gate.live(),
                )
            });
        let (admitted, snapshot, waited, snapshots_held) = closed.found;
        let freed = (answered(&mut closed.root), answered(&mut closed.sibling));
        assert_eq!(
            (
                admitted,
                snapshot,
                waited,
                snapshots_held,
                freed,
                closed.granted.clone()
            ),
            (
                Some(false),
                Some(false),
                Some(false),
                0,
                (Some(false), Some(false)),
                Vec::new()
            ),
            "every grant a closed pipeline asked for or was queued for was refused: its gate's \
             registration, its snapshot request, its queued snapshot when the gate opened, and \
             the pair the holder's end freed (to the closed root, then to the closed sibling); \
             (admitted, snapshot, queued snapshot, snapshots held, (root, sibling), handed out)"
        );
        let ledger = closed.wide.run.broker_mut().invocations();
        assert!(
            ledger.running().is_empty() && ledger.pending().is_empty(),
            "running {:?}, pending {:?}",
            ledger.running(),
            ledger.pending()
        );
        assert!(
            !ledger.settled(&sibling_gate) && ledger.registered() == 3,
            "the closed sibling's gate was never registered"
        );
    }

    const INC_A: &str = "01KZP5CONTAINERSA000000001";

    type InventoryAt = (Vec<String>, Vec<String>, Vec<String>, Vec<String>);

    fn container_names_of(
        wide: &Wide,
        incarnation: &str,
        invocations: &[InvocationId],
    ) -> Vec<String> {
        let identity = wide.env.identity(incarnation);
        let mut names: Vec<String> = invocations
            .iter()
            .map(|invocation| {
                crate::runner::container::container_name_for(
                    &identity.repo_key,
                    &identity.run_id,
                    &identity.incarnation,
                    invocation,
                )
            })
            .collect();
        names.sort();
        names
    }

    fn intents_under(root: &std::path::Path) -> Vec<String> {
        crate::runner::container::list_intents(root)
            .expect("list the container namespace")
            .into_iter()
            .map(|found| found.name.as_str().to_owned())
            .collect()
    }

    fn views_under(root: &std::path::Path) -> Vec<String> {
        let mut views: Vec<String> = match std::fs::read_dir(
            root.join(crate::runner::container::census::VIEWS_DIR),
        ) {
            Ok(entries) => entries
                .map(|entry| {
                    entry
                        .expect("a views entry")
                        .file_name()
                        .to_string_lossy()
                        .into_owned()
                })
                .collect(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(error) => panic!("read the views directory: {error}"),
        };
        views.sort();
        views
    }

    fn running_in(host: &crate::runner::container::FakeRuntime) -> Vec<String> {
        host.container_names()
            .into_iter()
            .filter(|name| {
                host.container(name).is_some_and(|container| {
                    container.state == crate::runner::container::runtime::Liveness::Running
                })
            })
            .collect()
    }

    #[test]
    fn every_container_invocation_is_launched_and_released_on_its_own_at_width_three() {
        use crate::runner::container::runtime::RuntimeOp;
        let tasks = three();
        let mut wide = Wide::durable_contained(
            "coordinator-contained",
            &tasks,
            3,
            WidePlans::default(),
            holding(&tasks, &[]),
            INC_A,
        );
        let host = crate::engine::topology::scaffold::container_host();
        let contained = wide.env.contained(&host, INC_A);
        let double = std::sync::Arc::clone(&wide.env.runner);
        let root = wide.env.fixture.private.clone();
        let names_env = wide.env.identity(INC_A);
        let mut points: Vec<InventoryAt> = Vec::new();
        let mut scheduler = Scheduler::scripted(
            &double,
            Box::new(|view: &Quiescent<'_>| {
                let mut expected: Vec<String> = view
                    .invoking
                    .iter()
                    .map(|invocation| {
                        crate::runner::container::container_name_for(
                            &names_env.repo_key,
                            &names_env.run_id,
                            &names_env.incarnation,
                            invocation,
                        )
                    })
                    .collect();
                expected.sort();
                points.push((
                    expected,
                    running_in(&host),
                    intents_under(&root),
                    views_under(&root),
                ));
                None
            }),
        );
        let mut hooks = wide.env.hooks();
        let pipelines = wide.env.pipelines_over(contained.clone());
        let progress = wide
            .run
            .run_concurrently(
                &wide.env.seams_over(&*contained),
                &pipelines,
                &mut hooks,
                Some(&mut scheduler),
            )
            .expect("the contained run completes");
        let widest = scheduler.widest;
        drop(scheduler);
        assert_eq!(outcome_of(&progress), RunOutcome::Complete);
        assert_eq!(widest, 3, "three containers ran at once");
        for (expected, running, intents, views) in &points {
            assert_eq!(
                (running, intents, views),
                (expected, expected, expected),
                "at every quiescent point the containers running, the intents and the views are \
                 exactly those of the invocations inside the runner"
            );
        }
        assert!(
            host.container_names().is_empty()
                && intents_under(&root).is_empty()
                && views_under(&root).is_empty(),
            "every container, intent and view was released: {:?} {:?} {:?}",
            host.container_names(),
            intents_under(&root),
            views_under(&root)
        );
        let journal = host.journal();
        let created: Vec<&str> = journal
            .iter()
            .filter(|entry| entry.op == RuntimeOp::Create)
            .map(|entry| entry.target.as_str())
            .collect();
        let removed: Vec<&str> = journal
            .iter()
            .filter(|entry| entry.op == RuntimeOp::Remove)
            .map(|entry| entry.target.as_str())
            .collect();
        let ran = double.ran();
        assert_eq!(created.len(), ran.len(), "one container per process the double ran");
        let mut created_sorted = created.clone();
        created_sorted.sort_unstable();
        created_sorted.dedup();
        assert_eq!(created_sorted.len(), created.len(), "no name was created twice");
        let mut removed_sorted = removed.clone();
        removed_sorted.sort_unstable();
        assert_eq!(created_sorted, removed_sorted, "each container was removed once");
        let ran_names = container_names_of(
            &wide,
            INC_A,
            &ran.iter().map(|ran| ran.invocation.clone()).collect::<Vec<_>>(),
        );
        assert_eq!(created_sorted, ran_names, "each container is its own invocation's");
        for ran in &ran {
            let name = container_names_of(&wide, INC_A, std::slice::from_ref(&ran.invocation))
                .remove(0);
            let volumes = journal
                .iter()
                .find(|entry| entry.op == RuntimeOp::Create && entry.target == name)
                .map(|entry| entry.detail.clone())
                .unwrap_or_default();
            let expected = if crate::runner::container::env::supplies_credential_location(&ran.role)
            {
                match ran.agent.as_ref().map(crate::runner::AgentId::as_str) {
                    Some(crate::engine::topology::scaffold::AGENT) => {
                        crate::engine::topology::scaffold::WORKER_VOLUME
                    }
                    Some(crate::engine::topology::scaffold::REVIEW_AGENT) => {
                        crate::engine::topology::scaffold::REVIEWER_VOLUME
                    }
                    _ => "",
                }
            } else {
                ""
            };
            assert_eq!(
                volumes, expected,
                "{}: the credential volume of its agent, exactly when its role is given one",
                ran.invocation
            );
        }
        assert!(wide.run.invocations_balance());
        replay_equals_live(&wide);
    }

    const INC_1: &str = "01KZP5CONTAINERS1000000001";
    const INC_2: &str = "01KZP5CONTAINERS2000000002";
    const INC_3: &str = "01KZP5CONTAINERS3000000003";
    const INC_FOREIGN: &str = "01KZP5CONTAINERSF00000000F";
    const RUN_DEAD: &str = "01KZP5DEADOWNER00000000001";
    const RUN_FOREIGN: &str = "01KZP5FOREIGNRUN0000000001";
    const RUN_NOBODY: &str = "01KZP5NOSUCHRUN00000000001";
    const CONTAINER_CHILD: &str =
        "engine::topology::coordinator::tests::container_coordinator_child";

    fn child_env(key: &str) -> String {
        std::env::var(key).unwrap_or_else(|_| panic!("the parent names `{key}`"))
    }

    fn probe_request(
        workspace: &std::path::Path,
        target: crate::runner::ProbeTarget,
    ) -> crate::runner::RunnerRequest {
        let (invocation, agent) = match &target {
            crate::runner::ProbeTarget::Shell => (
                crate::engine::topology::identity::PreflightIdentities::shell(0),
                None,
            ),
            crate::runner::ProbeTarget::Agent(agent) => (
                crate::engine::topology::identity::PreflightIdentities::agent(agent.as_str(), 0),
                Some(agent.clone()),
            ),
        };
        crate::runner::RunnerRequest {
            command: crate::runner::CommandSpec::new("sh").arg("-c").arg("exit 0"),
            workspace: workspace.to_path_buf(),
            role: crate::runner::ExecutionRole::Probe(target),
            timeout: Duration::from_secs(600),
            agent,
            invocation: invocation.expect("a probe identity"),
        }
    }

    fn agent_probe(workspace: &std::path::Path) -> crate::runner::RunnerRequest {
        probe_request(
            workspace,
            crate::runner::ProbeTarget::Agent(crate::runner::AgentId::new(
                crate::engine::topology::scaffold::AGENT,
            )),
        )
    }

    fn run_held(
        parent: &crate::engine::topology::scaffold::ParentSide,
        wide: &mut Wide,
        incarnation: &str,
    ) -> ! {
        let runner: std::sync::Arc<dyn crate::runner::Runner> =
            std::sync::Arc::new(crate::engine::topology::scaffold::container_runner(
                wide.env.identity(incarnation),
                &wide.env.fixture.base,
                Box::new(parent.runtime()),
                Duration::from_millis(10),
            ));
        let pipelines = wide.env.pipelines_over(std::sync::Arc::clone(&runner));
        let mut hooks = wide.env.hooks();
        let ended =
            wide.run
                .run_concurrently(&wide.env.seams_over(&*runner), &pipelines, &mut hooks, None);
        panic!("the parent kills this coordinator while its containers run; it ended {ended:?}");
    }

    fn fresh_child(parent: &crate::engine::topology::scaffold::ParentSide) {
        let incarnation = child_env("UPSTROKE_TEST_CHILD_INCARNATION");
        let tasks = three();
        let mut wide = Wide::durable_contained(
            "coordinator-child",
            &tasks,
            3,
            WidePlans::default(),
            RecordingRunner::new(),
            &incarnation,
        );
        parent.event(&serde_json::json!({
            "root": wide.env.fixture.root.to_string_lossy(),
        }));
        run_held(parent, &mut wide, &incarnation);
    }

    fn resume_child(parent: &crate::engine::topology::scaffold::ParentSide) {
        let root = std::path::PathBuf::from(child_env("UPSTROKE_TEST_CHILD_ROOT"));
        let incarnation = child_env("UPSTROKE_TEST_CHILD_INCARNATION");
        let tasks = three();
        let env = crate::engine::topology::scaffold::WideEnv::adopted(
            root,
            &tasks,
            3,
            WidePlans::default(),
        );
        let adapters = std::sync::Arc::clone(&env.adapters);
        let base = env.fixture.base.clone();
        let probes = crate::engine::topology::scaffold::container_runner(
            env.identity(&incarnation),
            &base,
            Box::new(parent.runtime()),
            Duration::from_millis(10),
        );
        let preflight = crate::engine::topology::preflight::RunPreflight::new(
            &probes,
            &*adapters,
            crate::gates::ShellKind::Sh,
            &base,
            Vec::new(),
        );
        let runtime = parent.runtime();
        let mut hooks = env.hooks();
        let (recovered, mut wide) = env
            .resume_over(
                &incarnation,
                RecordingRunner::new(),
                crate::engine::topology::select::Ceiling::unlimited(),
                &crate::engine::topology::scaffold::ResumingOver {
                    runtime: &runtime,
                    liveness: &crate::runner::container::runtime::LockProbe,
                    preflight: &preflight,
                    awaits_release: true,
                },
                &mut hooks,
            )
            .unwrap_or_else(|error| panic!("the resuming child's recovery order: {error}"));
        parent.event(&serde_json::json!({"interrupted": recovered.interrupted}));
        run_held(parent, &mut wide, &incarnation);
    }

    fn census_child(parent: &crate::engine::topology::scaffold::ParentSide) {
        let repo = std::path::PathBuf::from(child_env("UPSTROKE_TEST_CHILD_REPO"));
        let private = std::path::PathBuf::from(child_env("UPSTROKE_TEST_CHILD_PRIVATE"));
        let incarnation = child_env("UPSTROKE_TEST_CHILD_INCARNATION");
        let refused = crate::engine::topology::recover::chain::RootDerived::derive_with(
            &repo,
            RUN_NOBODY,
            None,
            crate::topology::schema::TOPOLOGY_SCHEMA,
        )
        .is_err();
        let git_dir = repo.join(".git");
        let lock = crate::rundir::WorktreeLock::acquire_in(&repo, &git_dir)
            .expect("the worktree lock, which the refusal before it left untaken");
        let repo_key = crate::rundir::RepoKey::v1(
            &std::fs::canonicalize(&git_dir).expect("the second repository's git dir"),
        );
        let runtime = parent.runtime();
        let view = crate::runner::container::DisposableDirView::new(
            crate::runner::container::runtime::ContainerTrace::off(),
        );
        let mut hooks = crate::engine::topology::seams::NoTopologyHooks::new();
        let censused = crate::engine::topology::startup::startup_census(
            crate::engine::topology::startup::WorktreeLocked::from(lock),
            &mut hooks,
            &crate::engine::topology::startup::CensusInputs {
                repo_root: &repo,
                repo_key: &repo_key,
                authorized_root: &private,
                incarnation: &incarnation,
                runtime: &runtime,
                liveness: &crate::runner::container::runtime::LockProbe,
                view: &view,
            },
        );
        let report = match &censused {
            Ok(done) => {
                let report = done.census().containers().report();
                serde_json::json!({
                    "census": "complete",
                    "reclaimed": report
                        .reclaimed
                        .iter()
                        .map(|reclaimed| serde_json::json!({
                            "name": reclaimed.name.as_str(),
                            "ownership": reclaimed.ownership.name(),
                        }))
                        .collect::<Vec<_>>(),
                    "untouched": report
                        .untouched
                        .iter()
                        .map(|untouched| serde_json::json!({
                            "name": untouched.name.as_str(),
                            "ownership": untouched.ownership.name(),
                        }))
                        .collect::<Vec<_>>(),
                })
            }
            Err(error) => serde_json::json!({"census": "refused", "error": error.to_string()}),
        };
        parent.event(&serde_json::json!({"prelock_refused": refused, "report": report}));
        if std::env::var_os("UPSTROKE_TEST_CHILD_USES_VOLUME").is_some() && censused.is_ok() {
            let run_dir = crate::rundir::public_dir(&repo, RUN_FOREIGN);
            crate::workspace_manager::fixture::write_file(&run_dir.join("created"), b"\n");
            let run_lock = crate::rundir::RunLock::acquire(&run_dir).expect("its own run lock");
            let identity = crate::runner::container::exec::RunIdentity {
                private_root: private.clone(),
                run_id: RUN_FOREIGN.to_owned(),
                run_dir: run_dir.clone(),
                incarnation: incarnation.clone(),
                repo_key: repo_key.as_str().to_owned(),
            };
            let runner = crate::engine::topology::scaffold::container_runner(
                identity,
                &repo,
                Box::new(parent.runtime()),
                Duration::from_millis(10),
            );
            let request = agent_probe(&run_dir);
            let used = crate::runner::container::container_name_for(
                repo_key.as_str(),
                RUN_FOREIGN,
                &incarnation,
                &request.invocation,
            );
            let ran = crate::runner::Runner::run_blocking(&runner, &request);
            parent.event(&serde_json::json!({
                "used": used,
                "ran": ran.as_ref().map(|output| output.code).map_err(ToString::to_string),
            }));
            drop(run_lock);
        }
        drop(censused);
    }

    fn owner_child(parent: &crate::engine::topology::scaffold::ParentSide) {
        let run_dir = std::path::PathBuf::from(child_env("UPSTROKE_TEST_CHILD_RUN_DIR"));
        let identity = crate::runner::container::exec::RunIdentity {
            private_root: std::path::PathBuf::from(child_env("UPSTROKE_TEST_CHILD_PRIVATE")),
            run_id: child_env("UPSTROKE_TEST_CHILD_RUN"),
            run_dir: run_dir.clone(),
            incarnation: child_env("UPSTROKE_TEST_CHILD_INCARNATION"),
            repo_key: child_env("UPSTROKE_TEST_CHILD_REPO_KEY"),
        };
        let _run_lock = crate::rundir::RunLock::acquire(&run_dir).expect("the owner's run lock");
        let runner = std::sync::Arc::new(crate::engine::topology::scaffold::container_runner(
            identity,
            &run_dir,
            Box::new(parent.runtime()),
            Duration::from_millis(10),
        ));
        let requests = vec![
            probe_request(&run_dir, crate::runner::ProbeTarget::Shell),
            agent_probe(&run_dir),
        ];
        let threads: Vec<std::thread::JoinHandle<()>> = requests
            .into_iter()
            .map(|request| {
                let runner = std::sync::Arc::clone(&runner);
                std::thread::spawn(move || {
                    let ended = crate::runner::Runner::run_blocking(&*runner, &request);
                    panic!("the owner's container ended before the kill: {ended:?}");
                })
            })
            .collect();
        for thread in threads {
            let _ = thread.join();
        }
        panic!("the parent kills this owner while its containers run");
    }

    #[test]
    #[ignore = "spawned by the two-process container tests"]
    fn container_coordinator_child() {
        let parent = crate::engine::topology::scaffold::ParentSide::attach();
        match child_env("UPSTROKE_TEST_CHILD_ROLE").as_str() {
            "fresh" => fresh_child(&parent),
            "resume" => resume_child(&parent),
            "census" => census_child(&parent),
            "owner" => owner_child(&parent),
            #[cfg(unix)]
            "hosted" => hosted_child(&parent),
            other => panic!("no child role `{other}`"),
        }
    }

    fn by(
        journal: &[crate::runner::container::Journaled],
        actor: &str,
        op: crate::runner::container::runtime::RuntimeOp,
    ) -> Vec<String> {
        journal
            .iter()
            .filter(|entry| entry.actor == actor && entry.op == op)
            .map(|entry| entry.target.clone())
            .collect()
    }

    fn await_starts(host: &crate::runner::container::FakeRuntime, actor: &str, count: usize) {
        assert!(
            host.await_journal(BOUND, |journal| {
                journal
                    .iter()
                    .filter(|entry| {
                        entry.actor == actor
                            && entry.op == crate::runner::container::runtime::RuntimeOp::Start
                    })
                    .count()
                    >= count
            }),
            "`{actor}` did not start {count} container(s) within {BOUND:?}: {:?}",
            host.journal()
                .iter()
                .filter(|entry| entry.actor == actor)
                .collect::<Vec<_>>()
        );
    }

    fn holds_nothing(run_dir: &std::path::Path, repo: &std::path::Path) -> Result<(), String> {
        let started = std::time::Instant::now();
        while crate::rundir::observe_cleanup_hold(run_dir, &mut crate::rundir::NoHooks)
            && started.elapsed() < BOUND
        {
            crate::workspace_manager::fixture::rest_within(
                Duration::from_millis(20),
                BOUND.saturating_sub(started.elapsed()),
            );
        }
        if crate::rundir::is_running(run_dir) {
            return Err(format!("{} is held", run_dir.display()));
        }
        crate::rundir::WorktreeLock::acquire_in(repo, &repo.join(".git"))
            .map(drop)
            .map_err(|error| format!("the worktree lock of {}: {error}", repo.display()))
    }

    fn broker_is_empty(run: &mut TopologyRun) -> Result<(), String> {
        let (held, cancelled, peak) = (
            run.entitlements_held(),
            run.reservations_cancelled(),
            run.reservations_peak(),
        );
        let duplicates = run.broker_duplicates();
        let ledger = run.broker_mut().invocations();
        if ledger.registered() != 0
            || !ledger.running().is_empty()
            || !ledger.pending().is_empty()
            || !ledger.slots().holders().is_empty()
            || !ledger.slots().pending().is_empty()
            || held != 0
            || cancelled != 0
            || peak != 0
            || duplicates != 0
        {
            return Err(format!(
                "registered {}, running {:?}, pending {:?}, pair holders {:?}, entitlements held \
                 {held}, reservations cancelled {cancelled}, peak {peak}, duplicates {duplicates}",
                ledger.registered(),
                ledger.running(),
                ledger.pending(),
                ledger.slots().holders()
            ));
        }
        Ok(())
    }

    fn second_repository(root: &std::path::Path) -> std::path::PathBuf {
        let repo = root.join("repo-y");
        crate::workspace_manager::fixture::write_file(&repo.join("seed.txt"), b"y\n");
        crate::workspace_manager::fixture::git(&repo, &["init", "-q", "-b", "main"]);
        repo
    }

    fn census_names(report: &serde_json::Value, list: &str) -> Vec<(String, String)> {
        let mut names: Vec<(String, String)> = report["report"][list]
            .as_array()
            .unwrap_or_else(|| panic!("the census report lists `{list}`: {report}"))
            .iter()
            .map(|entry| {
                (
                    entry["name"].as_str().unwrap_or_default().to_owned(),
                    entry["ownership"].as_str().unwrap_or_default().to_owned(),
                )
            })
            .collect();
        names.sort();
        names
    }

    fn served(
        logs: &std::path::Path,
        name: &str,
        env: &[(&str, &std::ffi::OsStr)],
        runtime: crate::runner::container::FakeRuntime,
    ) -> crate::engine::topology::scaffold::Served {
        crate::engine::topology::scaffold::Served::spawn(
            CONTAINER_CHILD,
            env,
            &logs.join(format!("{name}.stderr")),
            runtime,
        )
    }

    #[test]
    fn a_foreign_census_reclaims_a_dead_coordinators_containers_and_leaves_a_live_coordinators_running()
     {
        use crate::runner::container::runtime::RuntimeOp;
        use std::ffi::OsStr;
        let tasks = three();
        let mut wide = Wide::durable_contained(
            "coordinator-census-b",
            &tasks,
            3,
            WidePlans::default(),
            holding(&tasks, &[]),
            INC_A,
        );
        let host = crate::engine::topology::scaffold::container_host();
        let root = wide.env.fixture.root.clone();
        let private = wide.env.fixture.private.clone();
        let logs = crate::engine::topology::scaffold::kill_dir("coordinator-census-b-logs");
        let repo_key = wide.env.identity(INC_A).repo_key;

        let dead_dir = crate::rundir::public_dir(&root.join("repo-b"), RUN_DEAD);
        crate::workspace_manager::fixture::write_file(&dead_dir.join("created"), b"\n");
        let owner = served(
            logs.path(),
            "owner",
            &[
                ("UPSTROKE_TEST_CHILD_ROLE", OsStr::new("owner")),
                ("UPSTROKE_TEST_CHILD_RUN_DIR", dead_dir.as_os_str()),
                ("UPSTROKE_TEST_CHILD_PRIVATE", private.as_os_str()),
                ("UPSTROKE_TEST_CHILD_RUN", OsStr::new(RUN_DEAD)),
                ("UPSTROKE_TEST_CHILD_INCARNATION", OsStr::new(INC_1)),
                ("UPSTROKE_TEST_CHILD_REPO_KEY", OsStr::new(&repo_key)),
            ],
            host.acting_as("dead-owner"),
        );
        await_starts(&host, "dead-owner", 2);
        assert!(
            crate::rundir::is_running(&dead_dir),
            "the owner holds its run lock while its containers run"
        );
        let died = owner.kill();
        drop(owner);
        assert!(!died.success(), "the owner was killed: {died:?}");
        let dead: Vec<String> = {
            let mut dead = by(&host.journal(), "dead-owner", RuntimeOp::Start);
            dead.sort();
            dead
        };
        assert_eq!(dead.len(), 2);
        assert!(
            !crate::rundir::is_running(&dead_dir),
            "the dead owner's lock hold (R17) went with its process"
        );
        assert_eq!(running_in(&host), dead, "the dead owner's containers outlive it");

        let repo_y = second_repository(&root);
        let contained = wide.env.contained(&host, INC_A);
        let double = std::sync::Arc::clone(&wide.env.runner);
        let mut census: Option<(serde_json::Value, serde_json::Value, Vec<String>, Vec<String>)> =
            None;
        let mut scheduler = Scheduler::scripted(
            &double,
            Box::new(|view: &Quiescent<'_>| {
                if census.is_none() && view.invoking.len() == 3 {
                    let live = running_in(&host);
                    let foreign = served(
                        logs.path(),
                        "census",
                        &[
                            ("UPSTROKE_TEST_CHILD_ROLE", OsStr::new("census")),
                            ("UPSTROKE_TEST_CHILD_REPO", repo_y.as_os_str()),
                            ("UPSTROKE_TEST_CHILD_PRIVATE", private.as_os_str()),
                            ("UPSTROKE_TEST_CHILD_INCARNATION", OsStr::new(INC_FOREIGN)),
                            ("UPSTROKE_TEST_CHILD_USES_VOLUME", OsStr::new("1")),
                        ],
                        host.acting_as("foreign")
                            .starting(crate::engine::topology::scaffold::exiting()),
                    );
                    let report = foreign.event("the foreign census");
                    let used = foreign.event("the foreign write command's first volume use");
                    let status = foreign.exited("the foreign write command");
                    assert!(status.success(), "{status:?}: {}", foreign.stderr());
                    census = Some((report, used, live, running_in(&host)));
                }
                None
            }),
        );
        let mut hooks = wide.env.hooks();
        let pipelines = wide.env.pipelines_over(contained.clone());
        let progress = wide
            .run
            .run_concurrently(
                &wide.env.seams_over(&*contained),
                &pipelines,
                &mut hooks,
                Some(&mut scheduler),
            )
            .expect("the live coordinator completes");
        drop(scheduler);
        assert_eq!(outcome_of(&progress), RunOutcome::Complete);
        let (report, used, before, after) = census.expect("the foreign census ran at width three");

        let mut live: Vec<String> = before
            .iter()
            .filter(|name| !dead.contains(name))
            .cloned()
            .collect();
        live.sort();
        assert_eq!(live.len(), 3, "the live coordinator had three containers running");
        assert_eq!(report["prelock_refused"], true, "{report}");
        assert_eq!(report["report"]["census"], "complete", "{report}");
        assert_eq!(
            census_names(&report, "reclaimed"),
            dead.iter()
                .map(|name| (name.clone(), "foreign-run-dead-owner".to_owned()))
                .collect::<Vec<_>>(),
            "the census reclaimed the dead owner's containers and nothing else"
        );
        assert_eq!(
            census_names(&report, "untouched"),
            live.iter()
                .map(|name| (name.clone(), "foreign-run-live-owner".to_owned()))
                .collect::<Vec<_>>(),
            "the live coordinator's containers were classified live and left alone"
        );
        assert_eq!(after, live, "the live containers ran on across the census");

        let journal = host.journal();
        let foreign_effects: Vec<&crate::runner::container::Journaled> = journal
            .iter()
            .filter(|entry| entry.actor == "foreign" && entry.op.is_effect())
            .collect();
        for entry in &foreign_effects {
            assert!(
                !live.contains(&entry.target),
                "the foreign command touched a live container: {entry:?}"
            );
        }
        let used_name = used["used"].as_str().expect("the name it used").to_owned();
        assert_eq!(used["ran"], serde_json::json!({"Ok": 0}), "{used}");
        let position = |wanted: &dyn Fn(&crate::runner::container::Journaled) -> bool| {
            journal.iter().position(wanted)
        };
        let first_use = position(&|entry| {
            entry.actor == "foreign" && entry.op == RuntimeOp::Create && entry.target == used_name
        })
        .expect("the foreign command created its container");
        assert!(
            journal[first_use]
                .detail
                .contains(crate::engine::topology::scaffold::WORKER_VOLUME),
            "its first invocation mounts the shared credential volume: {:?}",
            journal[first_use]
        );
        for name in &dead {
            let removed = position(&|entry| {
                entry.actor == "foreign" && entry.op == RuntimeOp::Remove && entry.target == *name
            })
            .unwrap_or_else(|| panic!("`{name}` was removed by the census"));
            let terminated = journal
                .iter()
                .enumerate()
                .filter(|(_, entry)| {
                    entry.actor == "foreign"
                        && entry.op == RuntimeOp::Observe
                        && entry.target == *name
                })
                .map(|(index, _)| index)
                .next()
                .unwrap_or_else(|| panic!("`{name}` was observed by the census"));
            assert!(
                terminated < first_use && removed < first_use,
                "`{name}` was observed terminated ({terminated}) and removed ({removed}) before \
                 the foreign command's first use of the credential volume ({first_use})"
            );
        }
        for name in &live {
            let removers: Vec<&str> = journal
                .iter()
                .filter(|entry| entry.target == *name && entry.op == RuntimeOp::Remove)
                .map(|entry| entry.actor.as_str())
                .collect();
            assert_eq!(removers, [INC_A], "`{name}` was released by its own coordinator only");
        }
        assert!(
            host.container_names().is_empty()
                && intents_under(&private).is_empty()
                && views_under(&private).is_empty(),
            "nothing is left under the private root: {:?} {:?} {:?}",
            host.container_names(),
            intents_under(&private),
            views_under(&private)
        );
        assert!(wide.run.invocations_balance());
        let public = wide.env.paths.public.clone();
        let base = wide.env.fixture.base.clone();
        assert!(
            crate::rundir::is_running(&public),
            "the live coordinator's run lock is held until it ends"
        );
        let Wide { run, env } = wide;
        drop(run);
        holds_nothing(&public, &base).expect("the live coordinator's holds went with it");
        holds_nothing(&crate::rundir::public_dir(&repo_y, RUN_FOREIGN), &repo_y)
            .expect("the foreign write command's holds went with it");
        drop(env);
    }

    struct LedgerWatch<'p> {
        harness: crate::runner::container::HarnessHooks,
        preflight: &'p crate::engine::topology::preflight::RunPreflight<'p>,
        seen: Vec<(crate::topology::effects::EffectSiteId, (usize, usize), usize)>,
    }

    impl crate::runner::container::ContainerHooks for LedgerWatch<'_> {
        fn phase(
            &mut self,
            site: crate::topology::effects::EffectSiteId,
            phase: crate::topology::effects::HookPhase,
        ) -> crate::topology::effects::Injection {
            self.seen.push((
                site,
                self.preflight.settlements(),
                self.preflight.running().len(),
            ));
            self.harness.phase(site, phase)
        }

        fn trace(&self) -> crate::runner::container::runtime::ContainerTrace {
            self.harness.trace()
        }
    }

    struct CensusWatch<'p> {
        inner: crate::engine::topology::seams::HarnessTopologyHooks,
        watch: LedgerWatch<'p>,
    }

    impl TopologyHooks for CensusWatch<'_> {
        fn effects(&mut self) -> &mut dyn crate::workspace_manager::EffectHooks {
            self.inner.effects()
        }

        fn rundir(&mut self) -> &mut dyn crate::rundir::RunDirHooks {
            self.inner.rundir()
        }

        fn events(&mut self) -> &mut dyn crate::events::log::EventHooks {
            self.inner.events()
        }

        fn container(&mut self) -> &mut dyn crate::runner::container::ContainerHooks {
            &mut self.watch
        }

        fn spawn(&mut self) -> &mut dyn crate::agent::proc::SpawnHooks {
            self.inner.spawn()
        }

        fn folded(&mut self, fold: &TopologyFold, events: &[TopologyEvent]) {
            self.inner.folded(fold, events);
        }
    }

    fn sorted(mut names: Vec<String>) -> Vec<String> {
        names.sort();
        names
    }

    fn killed_fresh_incarnation(
        host: &crate::runner::container::FakeRuntime,
        logs: &std::path::Path,
    ) -> (std::path::PathBuf, Vec<String>) {
        use std::ffi::OsStr;
        let first = served(
            logs,
            "first",
            &[
                ("UPSTROKE_TEST_CHILD_ROLE", OsStr::new("fresh")),
                ("UPSTROKE_TEST_CHILD_INCARNATION", OsStr::new(INC_1)),
            ],
            host.acting_as(INC_1),
        );
        let root = std::path::PathBuf::from(
            first.event("the first incarnation's fixture")["root"]
                .as_str()
                .expect("a fixture root"),
        );
        await_starts(host, INC_1, 3);
        let died = first.kill();
        assert!(!died.success(), "the first incarnation was killed: {died:?}");
        drop(first);
        let started = sorted(by(
            &host.journal(),
            INC_1,
            crate::runner::container::runtime::RuntimeOp::Start,
        ));
        (root, started)
    }

    fn foreign_census_at_width_three(
        host: &crate::runner::container::FakeRuntime,
        logs: &std::path::Path,
        repo: &std::path::Path,
        private: &std::path::Path,
    ) -> serde_json::Value {
        use std::ffi::OsStr;
        let foreign = served(
            logs,
            "foreign",
            &[
                ("UPSTROKE_TEST_CHILD_ROLE", OsStr::new("census")),
                ("UPSTROKE_TEST_CHILD_REPO", repo.as_os_str()),
                ("UPSTROKE_TEST_CHILD_PRIVATE", private.as_os_str()),
                ("UPSTROKE_TEST_CHILD_INCARNATION", OsStr::new(INC_FOREIGN)),
            ],
            host.acting_as("foreign"),
        );
        let report = foreign.event("the foreign census");
        let status = foreign.exited("the foreign write command");
        assert!(status.success(), "{status:?}: {}", foreign.stderr());
        report
    }

    #[test]
    fn a_resuming_incarnation_reclaims_its_earlier_incarnations_containers_before_its_ledgers_probes_and_admission()
     {
        use crate::runner::container::runtime::RuntimeOp;
        use std::ffi::OsStr;
        let tasks = three();
        let host = crate::engine::topology::scaffold::container_host();
        let logs = crate::engine::topology::scaffold::kill_dir("coordinator-resume-f-logs");
        let (root, first) = killed_fresh_incarnation(&host, logs.path());
        let env = crate::engine::topology::scaffold::WideEnv::adopted(
            root.clone(),
            &tasks,
            3,
            WidePlans::default(),
        );
        holds_nothing(&env.paths.public, &env.fixture.base)
            .expect("the first incarnation's holds went with its process");
        assert_eq!(running_in(&host), first, "its three containers outlive it");

        let second = served(
            logs.path(),
            "second",
            &[
                ("UPSTROKE_TEST_CHILD_ROLE", OsStr::new("resume")),
                ("UPSTROKE_TEST_CHILD_ROOT", root.as_os_str()),
                ("UPSTROKE_TEST_CHILD_INCARNATION", OsStr::new(INC_2)),
            ],
            host.acting_as(INC_2),
        );
        await_starts(&host, INC_2, 1);
        let died = second.kill();
        assert!(!died.success(), "the second incarnation was killed: {died:?}");
        drop(second);
        let journal = host.journal();
        assert_eq!(
            sorted(by(&journal, INC_2, RuntimeOp::Remove)),
            first,
            "the second incarnation's census reclaimed the first incarnation's containers"
        );
        let orphan = by(&journal, INC_2, RuntimeOp::Start);
        assert_eq!(orphan.len(), 1, "it died inside its own shell probe: {orphan:?}");
        let orphan = orphan[0].clone();
        assert_eq!(running_in(&host), std::slice::from_ref(&orphan));
        holds_nothing(&env.paths.public, &env.fixture.base)
            .expect("the second incarnation's holds went with its process");

        let base = env.fixture.base.clone();
        let private = env.fixture.private.clone();
        let adapters = std::sync::Arc::clone(&env.adapters);
        let probes = crate::engine::topology::scaffold::container_runner(
            env.identity(INC_3),
            &base,
            Box::new(
                host.acting_as("third-probes")
                    .starting(crate::engine::topology::scaffold::exiting()),
            ),
            Duration::from_millis(5),
        );
        let preflight = crate::engine::topology::preflight::RunPreflight::new(
            &probes,
            &*adapters,
            crate::gates::ShellKind::Sh,
            &base,
            Vec::new(),
        );
        let census_runtime = host.acting_as("third-census");
        let mut hooks = CensusWatch {
            inner: env.hooks(),
            watch: LedgerWatch {
                harness: crate::runner::container::HarnessHooks::new(std::sync::Arc::clone(
                    &env.harness,
                )),
                preflight: &preflight,
                seen: Vec::new(),
            },
        };
        let (recovered, mut wide) = env
            .resume_over(
                INC_3,
                holding(&tasks, &[]),
                crate::engine::topology::select::Ceiling::unlimited(),
                &crate::engine::topology::scaffold::ResumingOver {
                    runtime: &census_runtime,
                    liveness: &crate::runner::container::runtime::LockProbe,
                    preflight: &preflight,
                    awaits_release: true,
                },
                &mut hooks,
            )
            .expect("the third incarnation resumes");
        let seen = std::mem::take(&mut hooks.watch.seen);
        drop(hooks);
        assert_eq!(recovered.interrupted, 3, "the first incarnation's three attempts");
        broker_is_empty(&mut wide.run).expect("the broker starts empty");

        let journal = host.journal();
        assert_eq!(
            by(&journal, "third-census", RuntimeOp::Remove),
            std::slice::from_ref(&orphan),
            "the third incarnation's census reclaimed the second incarnation's probe"
        );
        let own = by(&journal, "third-probes", RuntimeOp::Create);
        assert_eq!(own.len(), 1, "one shell probe: {own:?}");
        let own = own[0].clone();
        let (orphan_parts, own_parts) = (
            crate::runner::container::container_name_parts(&orphan).expect("the orphan's name"),
            crate::runner::container::container_name_parts(&own).expect("its own name"),
        );
        let shell = crate::runner::container::intent::invocation_hash(
            &crate::engine::topology::identity::PreflightIdentities::shell(0)
                .expect("the shell probe's identity"),
        );
        assert_eq!(
            (orphan_parts.1.as_str(), own_parts.1.as_str()),
            (shell.as_str(), shell.as_str()),
            "one deterministic InvocationId in both incarnations"
        );
        assert_eq!(
            (orphan_parts.0.as_str(), own_parts.0.as_str()),
            (INC_2, INC_3)
        );
        assert_ne!(
            crate::runner::container::intent_path_for(&private, &orphan),
            crate::runner::container::intent_path_for(&private, &own),
            "the two incarnations' records never share a path"
        );
        let first_own = journal
            .iter()
            .position(|entry| entry.actor == "third-probes")
            .expect("its probe ran");
        let last_reclaim = journal
            .iter()
            .rposition(|entry| entry.actor == "third-census" && entry.target == orphan)
            .expect("the census acted on the orphan");
        assert!(
            last_reclaim < first_own,
            "the census finished with the orphan ({last_reclaim}) before this incarnation's first \
             container operation ({first_own})"
        );
        let reclaims: Vec<&(crate::topology::effects::EffectSiteId, (usize, usize), usize)> = seen
            .iter()
            .filter(|(site, _, _)| {
                matches!(
                    site,
                    crate::topology::effects::EffectSiteId::Container(
                        crate::topology::effects::ContainerSite::Stop
                            | crate::topology::effects::ContainerSite::Remove
                            | crate::topology::effects::ContainerSite::UnmountGitView
                            | crate::topology::effects::ContainerSite::RemoveIntent
                    )
                )
            })
            .collect();
        assert!(!reclaims.is_empty(), "the census's reclaim steps were observed: {seen:?}");
        for (site, settled, running) in &reclaims {
            assert_eq!(
                (*settled, *running),
                ((0, 0), 0),
                "{site:?}: the pre-flight's ledger held nothing while the census reclaimed"
            );
        }
        assert_eq!(preflight.settlements(), (1, 0), "then its shell probe ran and settled");

        let repo_y = second_repository(&root);
        let contained = wide.env.contained(&host, INC_3);
        let double = std::sync::Arc::clone(&wide.env.runner);
        let mut foreign: Option<(serde_json::Value, Vec<String>, Vec<String>)> = None;
        let mut scheduler = Scheduler::scripted(
            &double,
            Box::new(|view: &Quiescent<'_>| {
                if foreign.is_none() && view.invoking.len() == 3 {
                    let before = running_in(&host);
                    let report =
                        foreign_census_at_width_three(&host, logs.path(), &repo_y, &private);
                    foreign = Some((report, before, running_in(&host)));
                }
                None
            }),
        );
        let mut hooks = wide.env.hooks();
        let pipelines = wide.env.pipelines_over(contained.clone());
        let progress = wide
            .run
            .run_concurrently(
                &wide.env.seams_over(&*contained),
                &pipelines,
                &mut hooks,
                Some(&mut scheduler),
            )
            .expect("the third incarnation completes");
        drop(scheduler);
        assert_eq!(outcome_of(&progress), RunOutcome::Complete);
        let (report, before, after) = foreign.expect("a foreign census ran at width three");
        assert_eq!(before.len(), 3, "{before:?}");
        assert_eq!(report["report"]["census"], "complete", "{report}");
        assert!(census_names(&report, "reclaimed").is_empty(), "{report}");
        assert_eq!(
            census_names(&report, "untouched"),
            before
                .iter()
                .map(|name| (name.clone(), "foreign-run-live-owner".to_owned()))
                .collect::<Vec<_>>(),
            "the containers it started after its census are a live owner's"
        );
        assert_eq!(after, before, "and they ran on");
        assert!(
            host.container_names().is_empty()
                && intents_under(&private).is_empty()
                && views_under(&private).is_empty(),
            "{:?} {:?} {:?}",
            host.container_names(),
            intents_under(&private),
            views_under(&private)
        );
        let (public, base) = (wide.env.paths.public.clone(), wide.env.fixture.base.clone());
        let Wide { run, env } = wide;
        drop(run);
        holds_nothing(&public, &base).expect("the third incarnation's holds went with it");
        drop(env);
    }

    #[test]
    fn crashes_across_three_incarnations_with_pipelines_in_flight_leave_every_orphan_reclaimed_and_no_name_twice()
     {
        use crate::runner::container::runtime::RuntimeOp;
        use std::ffi::OsStr;
        let tasks = three();
        let host = crate::engine::topology::scaffold::container_host();
        let logs = crate::engine::topology::scaffold::kill_dir("coordinator-crashes-g-logs");
        let (root, first) = killed_fresh_incarnation(&host, logs.path());

        let second = served(
            logs.path(),
            "second",
            &[
                ("UPSTROKE_TEST_CHILD_ROLE", OsStr::new("resume")),
                ("UPSTROKE_TEST_CHILD_ROOT", root.as_os_str()),
                ("UPSTROKE_TEST_CHILD_INCARNATION", OsStr::new(INC_2)),
            ],
            host.acting_as(INC_2)
                .starting(crate::engine::topology::scaffold::exiting_probes()),
        );
        assert_eq!(
            second.event("the second incarnation's recovery")["interrupted"],
            3,
            "it settled the first incarnation's three attempts interrupted"
        );
        await_starts(&host, INC_2, 4);
        let died = second.kill();
        assert!(!died.success(), "the second incarnation was killed: {died:?}");
        drop(second);
        let journal = host.journal();
        let probe_hash = crate::runner::container::intent::invocation_hash(
            &crate::engine::topology::identity::PreflightIdentities::shell(0)
                .expect("the shell probe's identity"),
        );
        let is_probe = |name: &String| {
            crate::runner::container::container_name_parts(name)
                .is_some_and(|(_, invocation_hash)| invocation_hash == probe_hash)
        };
        let second_pipelines: Vec<String> = sorted(
            by(&journal, INC_2, RuntimeOp::Start)
                .into_iter()
                .filter(|name| !is_probe(name))
                .collect(),
        );
        assert_eq!(second_pipelines.len(), 3, "{second_pipelines:?}");
        assert_eq!(
            sorted(
                by(&journal, INC_2, RuntimeOp::Remove)
                    .into_iter()
                    .filter(|name| !is_probe(name))
                    .collect()
            ),
            first,
            "the second incarnation reclaimed the first incarnation's orphans"
        );
        assert_eq!(running_in(&host), second_pipelines);

        let env = crate::engine::topology::scaffold::WideEnv::adopted(
            root.clone(),
            &tasks,
            3,
            WidePlans::default(),
        );
        holds_nothing(&env.paths.public, &env.fixture.base)
            .expect("the second incarnation's holds went with its process");
        let base = env.fixture.base.clone();
        let private = env.fixture.private.clone();
        let adapters = std::sync::Arc::clone(&env.adapters);
        let probes = crate::engine::topology::scaffold::container_runner(
            env.identity(INC_3),
            &base,
            Box::new(
                host.acting_as("third-probes")
                    .starting(crate::engine::topology::scaffold::exiting()),
            ),
            Duration::from_millis(5),
        );
        let preflight = crate::engine::topology::preflight::RunPreflight::new(
            &probes,
            &*adapters,
            crate::gates::ShellKind::Sh,
            &base,
            Vec::new(),
        );
        let census_runtime = host.acting_as("third-census");
        let mut hooks = env.hooks();
        let (recovered, mut wide) = env
            .resume_over(
                INC_3,
                holding(&tasks, &[]),
                crate::engine::topology::select::Ceiling::unlimited(),
                &crate::engine::topology::scaffold::ResumingOver {
                    runtime: &census_runtime,
                    liveness: &crate::runner::container::runtime::LockProbe,
                    preflight: &preflight,
                    awaits_release: true,
                },
                &mut hooks,
            )
            .expect("the third incarnation resumes");
        assert_eq!(
            recovered.interrupted, 3,
            "the second incarnation's three attempts"
        );
        broker_is_empty(&mut wide.run).expect("the broker starts empty");
        assert_eq!(
            sorted(by(&host.journal(), "third-census", RuntimeOp::Remove)),
            second_pipelines,
            "the third incarnation reclaimed the second incarnation's orphans"
        );

        let contained = wide.env.contained(&host, INC_3);
        let double = std::sync::Arc::clone(&wide.env.runner);
        let mut scheduler = Scheduler::first(&double);
        let mut hooks = wide.env.hooks();
        let pipelines = wide.env.pipelines_over(contained.clone());
        let progress = wide
            .run
            .run_concurrently(
                &wide.env.seams_over(&*contained),
                &pipelines,
                &mut hooks,
                Some(&mut scheduler),
            )
            .expect("the third incarnation completes");
        drop(scheduler);
        assert_eq!(outcome_of(&progress), RunOutcome::Complete);

        let journal = host.journal();
        let created: Vec<String> = journal
            .iter()
            .filter(|entry| entry.op == RuntimeOp::Create)
            .map(|entry| entry.target.clone())
            .collect();
        let distinct = sorted(created.clone());
        let mut deduped = distinct.clone();
        deduped.dedup();
        assert_eq!(deduped, distinct, "no container name was created twice");
        let intents: std::collections::BTreeSet<std::path::PathBuf> = created
            .iter()
            .map(|name| {
                crate::runner::container::intent_path_for(&private, name).expect("a container name")
            })
            .collect();
        assert_eq!(intents.len(), created.len(), "and no intent path twice");
        let by_hash = |incarnation: &str| -> std::collections::BTreeMap<String, String> {
            created
                .iter()
                .filter_map(|name| {
                    let (of, invocation_hash) =
                        crate::runner::container::container_name_parts(name)
                            .expect("a container name");
                    (of == incarnation).then(|| (invocation_hash, name.clone()))
                })
                .collect()
        };
        let (one, two, three) = (by_hash(INC_1), by_hash(INC_2), by_hash(INC_3));
        let shared: Vec<&String> = two.keys().filter(|hash| three.contains_key(*hash)).collect();
        assert!(
            shared.contains(&&probe_hash),
            "the second and third incarnations ran one deterministic probe identity: {shared:?}"
        );
        for hash in one.keys().chain(two.keys()) {
            let names: Vec<&String> = [&one, &two, &three]
                .into_iter()
                .filter_map(|of| of.get(hash))
                .collect();
            let mut unique = names.clone();
            unique.sort();
            unique.dedup();
            assert_eq!(unique.len(), names.len(), "{hash}: one name per incarnation");
        }
        assert_eq!(
            count(&wide.env.durable_events(), "attempt_interrupted"),
            6,
            "each dead incarnation's in-flight attempts were settled once"
        );
        assert_eq!(count(&wide.env.durable_events(), "run_resumed"), 2);
        assert!(
            host.container_names().is_empty()
                && intents_under(&private).is_empty()
                && views_under(&private).is_empty(),
            "{:?} {:?} {:?}",
            host.container_names(),
            intents_under(&private),
            views_under(&private)
        );
    }

    #[test]
    fn a_foreign_census_and_a_resuming_incarnation_converge_on_one_dead_container_as_two_processes()
     {
        use crate::runner::container::runtime::RuntimeOp;
        use std::ffi::OsStr;
        let tasks = three();
        let wide = Wide::durable_contained(
            "coordinator-converge-h",
            &tasks,
            3,
            WidePlans::default(),
            RecordingRunner::new(),
            INC_1,
        );
        let host = crate::engine::topology::scaffold::container_host();
        let logs = crate::engine::topology::scaffold::kill_dir("coordinator-converge-h-logs");
        let root = wide.env.fixture.root.clone();
        let private = wide.env.fixture.private.clone();
        let repo_key = wide.env.identity(INC_1).repo_key;
        let dead_dir = crate::rundir::public_dir(&root.join("repo-dead"), RUN_DEAD);
        crate::workspace_manager::fixture::write_file(&dead_dir.join("created"), b"\n");
        let owner = served(
            logs.path(),
            "owner",
            &[
                ("UPSTROKE_TEST_CHILD_ROLE", OsStr::new("owner")),
                ("UPSTROKE_TEST_CHILD_RUN_DIR", dead_dir.as_os_str()),
                ("UPSTROKE_TEST_CHILD_PRIVATE", private.as_os_str()),
                ("UPSTROKE_TEST_CHILD_RUN", OsStr::new(RUN_DEAD)),
                ("UPSTROKE_TEST_CHILD_INCARNATION", OsStr::new(INC_FOREIGN)),
                ("UPSTROKE_TEST_CHILD_REPO_KEY", OsStr::new(&repo_key)),
            ],
            host.acting_as("dead-owner"),
        );
        await_starts(&host, "dead-owner", 2);
        let died = owner.kill();
        assert!(!died.success(), "the owner was killed: {died:?}");
        drop(owner);
        let dead = sorted(by(&host.journal(), "dead-owner", RuntimeOp::Start));
        let contested = dead[0].clone();
        host.pace(&contested, &[RuntimeOp::Stop, RuntimeOp::Remove], 2);

        let Wide { run, env } = wide;
        drop(run);
        holds_nothing(&env.paths.public, &env.fixture.base)
            .expect("the first incarnation ended holding nothing");
        let repo_y = second_repository(&root);
        let foreign = served(
            logs.path(),
            "foreign",
            &[
                ("UPSTROKE_TEST_CHILD_ROLE", OsStr::new("census")),
                ("UPSTROKE_TEST_CHILD_REPO", repo_y.as_os_str()),
                ("UPSTROKE_TEST_CHILD_PRIVATE", private.as_os_str()),
                ("UPSTROKE_TEST_CHILD_INCARNATION", OsStr::new(INC_FOREIGN)),
            ],
            host.acting_as("foreign"),
        );
        let census_runtime = host.acting_as("resuming");
        let mut hooks = env.hooks();
        let (recovered, resumed) = env
            .resume_over(
                INC_2,
                RecordingRunner::new(),
                crate::engine::topology::select::Ceiling::unlimited(),
                &crate::engine::topology::scaffold::ResumingOver {
                    runtime: &census_runtime,
                    liveness: &crate::runner::container::runtime::LockProbe,
                    preflight: &crate::engine::topology::scaffold::Certifying,
                    awaits_release: true,
                },
                &mut hooks,
            )
            .expect("the resuming incarnation converges and resumes");
        let report = foreign.event("the foreign census");
        let status = foreign.exited("the foreign write command");
        assert!(status.success(), "{status:?}: {}", foreign.stderr());
        drop(foreign);
        assert_eq!(recovered.interrupted, 0);
        assert_eq!(report["report"]["census"], "complete", "{report}");
        assert!(
            census_names(&report, "reclaimed")
                .iter()
                .any(|(name, ownership)| *name == contested
                    && ownership == "foreign-run-dead-owner"),
            "the foreign command reclaimed the contested container: {report}"
        );
        let journal = host.journal();
        for op in [RuntimeOp::Stop, RuntimeOp::Remove] {
            let actors: std::collections::BTreeSet<&str> = journal
                .iter()
                .filter(|entry| entry.op == op && entry.target == contested)
                .map(|entry| entry.actor.as_str())
                .collect();
            assert_eq!(
                actors,
                ["foreign", "resuming"].into_iter().collect(),
                "{op}: both reclaimers acted on the contested container"
            );
        }
        assert!(
            host.container_names().is_empty()
                && intents_under(&private).is_empty()
                && views_under(&private).is_empty(),
            "both reclaimers converged on a clean root: {:?} {:?} {:?}",
            host.container_names(),
            intents_under(&private),
            views_under(&private)
        );
        drop(resumed);
    }

    #[cfg(unix)]
    struct CarriedLeases {
        inner: std::sync::Arc<RecordingRunner>,
        carried: std::sync::Mutex<Vec<(InvocationId, Vec<std::path::PathBuf>)>>,
    }

    #[cfg(unix)]
    impl crate::runner::Runner for CarriedLeases {
        fn run<'a>(
            &'a self,
            request: &'a crate::runner::RunnerRequest,
            call: crate::runner::RunnerCall<'a>,
        ) -> crate::runner::RunFuture<'a> {
            let parts = call.into_parts();
            assert!(parts.spawn.is_none() && parts.container.is_none());
            self.carried
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push((request.invocation.clone(), parts.leases.to_vec()));
            self.inner.run(
                request,
                crate::runner::RunnerCall::new(parts.cancellation)
                    .holding_cleanup_leases(parts.leases),
            )
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_surviving_reaper_hold_refuses_the_next_coordinator_until_released_and_is_never_reset_at_width_three()
     {
        let tasks = three();
        let mut wide = Wide::durable(
            "coordinator-reaper-hold",
            &tasks,
            3,
            WidePlans::default(),
            holding(&tasks, &[]),
        );
        let double = std::sync::Arc::clone(&wide.env.runner);
        let carrying = std::sync::Arc::new(CarriedLeases {
            inner: std::sync::Arc::clone(&double),
            carried: std::sync::Mutex::new(Vec::new()),
        });
        let mut scheduler = shutdown_at_first_point(&double);
        let mut hooks = wide.env.hooks();
        let pipelines =
            wide.env
                .pipelines_over(std::sync::Arc::clone(&carrying) as std::sync::Arc<dyn crate::runner::Runner>);
        let error = wide
            .run
            .run_concurrently(
                &wide.env.seams_over(&*carrying),
                &pipelines,
                &mut hooks,
                Some(&mut scheduler),
            )
            .expect_err("a shutdown ends the command");
        drop(scheduler);
        assert!(error.to_string().contains("shut down"), "{error}");
        let public = wide.env.paths.public.clone();
        let carried = carrying
            .carried
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        assert_eq!(carried.len(), 3, "three workers were granted: {carried:?}");
        let lease = std::fs::canonicalize(&public)
            .expect("the run's public directory")
            .join("cleanup.lock");
        for (invocation, leases) in &carried {
            let leases: Vec<std::path::PathBuf> = leases
                .iter()
                .map(|path| std::fs::canonicalize(path).expect("a carried lease path"))
                .collect();
            assert_eq!(
                leases,
                std::slice::from_ref(&lease),
                "`{invocation}`, spawned from a pipeline thread, carries the run's cleanup lease \
                 to its reaper"
            );
        }

        let Wide { run, env } = wide;
        drop(run);
        let mut reaper = crate::workspace_manager::fixture::spawn_ready_helper(
            "rundir::tests::cleanup_hold_child",
            &[("UPSTROKE_TEST_CLEANUP_DIR", public.as_os_str())],
        );
        reaper
            .await_line("held", BOUND)
            .or_fail("the surviving reaper never took its hold");
        assert!(
            crate::rundir::observe_cleanup_hold(&public, &mut crate::rundir::NoHooks),
            "a surviving reaper holds R28"
        );
        let runtime = crate::runner::container::FakeRuntime::new(
            crate::runner::container::runtime::ContainerTrace::off(),
        );
        let liveness = crate::runner::container::FakeOwnerLiveness::new();
        let mut hooks = env.hooks();
        let (refused, env) = match env.try_resume_over(
            INC_2,
            holding(&tasks, &[]),
            crate::engine::topology::select::Ceiling::unlimited(),
            &crate::engine::topology::scaffold::ResumingOver {
                runtime: &runtime,
                liveness: &liveness,
                preflight: &crate::engine::topology::scaffold::Certifying,
                awaits_release: false,
            },
            &mut hooks,
        ) {
            Ok(_) => panic!("the next coordinator resumed over a surviving reaper's hold"),
            Err(refused) => *refused,
        };
        assert!(
            refused
                .to_string()
                .contains("still has a process of its own alive"),
            "the refusal names the hold it observed: {refused}"
        );
        assert!(
            crate::rundir::observe_cleanup_hold(&public, &mut crate::rundir::NoHooks),
            "the refused coordinator left the reaper's hold as it found it"
        );
        drop(reaper);
        let (recovered, mut resumed) = env
            .resume(
                INC_2,
                RecordingRunner::new().answering(wide_responder(&tasks, &[])),
                crate::engine::topology::select::Ceiling::unlimited(),
            )
            .expect("once the hold is released the next coordinator resumes");
        assert_eq!(recovered.interrupted, 3);
        broker_is_empty(&mut resumed.run).expect("its ledgers start empty");
        let progress = drive(&mut resumed, None).expect("the resumed run completes");
        assert_eq!(outcome_of(&progress), RunOutcome::Complete);
        assert!(resumed.run.invocations_balance());
    }

    #[cfg(unix)]
    struct HostHeld {
        host: crate::runner::host::HostRunner,
        pids: std::path::PathBuf,
    }

    #[cfg(unix)]
    impl crate::runner::Runner for HostHeld {
        fn run<'a>(
            &'a self,
            request: &'a crate::runner::RunnerRequest,
            call: crate::runner::RunnerCall<'a>,
        ) -> crate::runner::RunFuture<'a> {
            let pidfile = self.pids.join(format!("{}.pid", request.invocation.render()));
            let held = crate::runner::RunnerRequest {
                command: crate::runner::CommandSpec::new("sh").arg("-c").arg(format!(
                    "echo $$ > '{}'; exec sleep 600",
                    pidfile.display()
                )),
                timeout: Duration::from_secs(900),
                ..request.clone()
            };
            let parts = call.into_parts();
            Box::pin(async move {
                let call = crate::runner::RunnerCall::new(parts.cancellation)
                    .holding_cleanup_leases(parts.leases);
                self.host.run(&held, call).await
            })
        }
    }

    #[cfg(unix)]
    fn hosted_child(parent: &crate::engine::topology::scaffold::ParentSide) {
        let pids = std::path::PathBuf::from(child_env("UPSTROKE_TEST_CHILD_PIDS"));
        let tasks = three();
        let mut wide = Wide::durable(
            "coordinator-child-host",
            &tasks,
            3,
            WidePlans::default(),
            RecordingRunner::new(),
        );
        parent.event(&serde_json::json!({
            "root": wide.env.fixture.root.to_string_lossy(),
        }));
        let runner: std::sync::Arc<dyn crate::runner::Runner> = std::sync::Arc::new(HostHeld {
            host: crate::runner::host::HostRunner::new(),
            pids,
        });
        let pipelines = wide.env.pipelines_over(std::sync::Arc::clone(&runner));
        let mut hooks = wide.env.hooks();
        let ended =
            wide.run
                .run_concurrently(&wide.env.seams_over(&*runner), &pipelines, &mut hooks, None);
        panic!("the parent kills this coordinator while its processes run; it ended {ended:?}");
    }

    #[cfg(unix)]
    fn within(bound: Duration, what: &str, done: impl Fn() -> bool) {
        let started = std::time::Instant::now();
        while !done() {
            assert!(started.elapsed() < bound, "{what} within {bound:?}");
            crate::workspace_manager::fixture::rest_within(
                Duration::from_millis(20),
                bound.saturating_sub(started.elapsed()),
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_killed_coordinators_reapers_settle_its_pipelines_processes_under_r28_before_the_next_one_resumes_at_width_three()
     {
        use std::ffi::OsStr;
        let tasks = three();
        let logs = crate::engine::topology::scaffold::kill_dir("coordinator-reapers-logs");
        let pids = logs.path().join("pids");
        crate::workspace_manager::fixture::write_file(&pids.join("created"), b"\n");
        let child = served(
            logs.path(),
            "hosted",
            &[
                ("UPSTROKE_TEST_CHILD_ROLE", OsStr::new("hosted")),
                ("UPSTROKE_TEST_CHILD_PIDS", pids.as_os_str()),
            ],
            crate::engine::topology::scaffold::container_host(),
        );
        let root = std::path::PathBuf::from(
            child.event("the coordinator's fixture")["root"]
                .as_str()
                .expect("a fixture root"),
        );
        let read_pids = || -> Vec<u32> {
            std::fs::read_dir(&pids)
                .expect("the pid directory")
                .filter_map(|entry| {
                    let path = entry.expect("a pid entry").path();
                    (path.extension().and_then(OsStr::to_str) == Some("pid"))
                        .then(|| std::fs::read_to_string(&path).ok())
                        .flatten()
                        .and_then(|text| text.trim().parse().ok())
                })
                .collect()
        };
        within(BOUND, "three pipeline processes started", || read_pids().len() == 3);
        let started = read_pids();
        let public =
            crate::rundir::public_dir(&root.join("repo"), crate::workspace_manager::fixture::RUN_ID);
        assert!(
            crate::rundir::is_running(&public),
            "the coordinator holds its run lock"
        );
        assert!(
            crate::rundir::observe_cleanup_hold(&public, &mut crate::rundir::NoHooks),
            "the reapers of processes spawned from pipeline threads hold the run's cleanup lease"
        );
        for pid in &started {
            assert!(crate::workspace_manager::fixture::process_exists(*pid));
        }
        let died = child.kill();
        assert!(!died.success(), "the coordinator was killed: {died:?}");
        drop(child);
        within(BOUND, "every pipeline process settled by its reaper", || {
            started
                .iter()
                .all(|pid| !crate::workspace_manager::fixture::process_exists(*pid))
        });
        within(BOUND, "the reapers' holds released once their groups settled", || {
            !crate::rundir::observe_cleanup_hold(&public, &mut crate::rundir::NoHooks)
        });
        let env = crate::engine::topology::scaffold::WideEnv::adopted(
            root,
            &tasks,
            3,
            WidePlans::default(),
        );
        holds_nothing(&env.paths.public, &env.fixture.base)
            .expect("the dead coordinator's lock holds went with it");
        let (recovered, mut resumed) = env
            .resume(
                "inc-2",
                RecordingRunner::new().answering(wide_responder(&tasks, &[])),
                crate::engine::topology::select::Ceiling::unlimited(),
            )
            .expect("the next coordinator resumes");
        assert_eq!(recovered.interrupted, 3);
        broker_is_empty(&mut resumed.run).expect("its ledgers start empty");
        let progress = drive(&mut resumed, None).expect("the resumed run completes");
        assert_eq!(outcome_of(&progress), RunOutcome::Complete);
    }

    fn balanced_at_end(run: &mut TopologyRun) -> Result<(), String> {
        let held = run.entitlements_held();
        let balances = run.invocations_balance();
        let ledger = run.broker_mut().invocations();
        if !balances
            || held != 0
            || !ledger.running().is_empty()
            || !ledger.pending().is_empty()
            || !ledger.slots().holders().is_empty()
            || !ledger.slots().pending().is_empty()
        {
            return Err(format!(
                "balances {balances}, entitlements held {held}, running {:?}, pending {:?}, pair \
                 holders {:?}",
                ledger.running(),
                ledger.pending(),
                ledger.slots().holders()
            ));
        }
        Ok(())
    }

    fn next_start(wide: Wide, tasks: &[WideTask], shape: &str) -> Option<Wide> {
        let Wide { run, env } = wide;
        drop(run);
        holds_nothing(&env.paths.public, &env.fixture.base)
            .unwrap_or_else(|error| panic!("{shape}: {error}"));
        let runtime = crate::runner::container::FakeRuntime::new(
            crate::runner::container::runtime::ContainerTrace::off(),
        );
        let liveness = crate::runner::container::FakeOwnerLiveness::new();
        let mut hooks = env.hooks();
        match env.try_resume_over(
            "inc-2",
            RecordingRunner::new().answering(wide_responder(tasks, &[])),
            crate::engine::topology::select::Ceiling::unlimited(),
            &crate::engine::topology::scaffold::ResumingOver {
                runtime: &runtime,
                liveness: &liveness,
                preflight: &crate::engine::topology::scaffold::Certifying,
                awaits_release: true,
            },
            &mut hooks,
        ) {
            Ok((_, mut resumed)) => {
                broker_is_empty(&mut resumed.run)
                    .unwrap_or_else(|error| panic!("{shape}: the next start: {error}"));
                Some(resumed)
            }
            Err(refused) => {
                let (error, env) = *refused;
                holds_nothing(&env.paths.public, &env.fixture.base).unwrap_or_else(|held| {
                    panic!("{shape}: the refused next start ({error}) left a hold: {held}")
                });
                None
            }
        }
    }

    #[test]
    fn the_broker_ledgers_balance_at_every_end_and_start_empty_at_every_next_start_at_width_three()
     {
        use crate::topology::effects::SubEffectPoint;
        let tasks = three();
        let answering = || RecordingRunner::new().answering(wide_responder(&tasks, &[]));

        let mut complete = Wide::durable(
            "coordinator-ledgers-complete",
            &tasks,
            3,
            WidePlans::default(),
            answering(),
        );
        let progress = drive(&mut complete, None).expect("the run completes");
        assert_eq!(outcome_of(&progress), RunOutcome::Complete);
        balanced_at_end(&mut complete.run).expect("Complete");
        assert!(
            next_start(complete, &tasks, "Complete").is_none(),
            "a complete run is finalized and refused at the next start"
        );

        let mut parked = Wide::durable(
            "coordinator-ledgers-parked",
            &tasks,
            3,
            WidePlans::default(),
            RecordingRunner::new().answering(
                crate::engine::topology::scaffold::wide_responder_asking(&tasks, &[], &[0, 1, 2]),
            ),
        );
        let progress = drive(&mut parked, None).expect("the run parks");
        assert_eq!(outcome_of(&progress), RunOutcome::Parked);
        balanced_at_end(&mut parked.run).expect("Parked");
        assert!(next_start(parked, &tasks, "Parked").is_some());

        let mut halted = durable_halting_on_gamma("coordinator-ledgers-halted");
        let runner = std::sync::Arc::clone(&halted.env.runner);
        let mut scheduler = gamma_first(&runner);
        let progress = drive(&mut halted, Some(&mut scheduler)).expect("the run halts");
        drop(scheduler);
        assert_eq!(outcome_of(&progress), RunOutcome::Halted);
        balanced_at_end(&mut halted.run).expect("Halted");
        assert!(next_start(halted, &tasks, "Halted").is_none());

        let wider = four();
        let mut budget = Wide::durable_under(
            "coordinator-ledgers-budget",
            &wider,
            3,
            WidePlans::default(),
            holding(&wider, &[]),
            TIGHT,
        );
        let runner = std::sync::Arc::clone(&budget.env.runner);
        let mut scheduler = Scheduler::scripted(
            &runner,
            Box::new(|view: &Quiescent<'_>| {
                released(view, |invocation| attempt_key(invocation) == Some(0))
            }),
        );
        let progress = drive(&mut budget, Some(&mut scheduler)).expect("the budget stop ends it");
        drop(scheduler);
        assert_eq!(outcome_of(&progress), RunOutcome::BudgetExceeded);
        balanced_at_end(&mut budget.run).expect("BudgetExceeded");
        assert!(next_start(budget, &wider, "BudgetExceeded").is_some());

        let mut failed = Wide::durable(
            "coordinator-ledgers-append-error",
            &tasks,
            3,
            WidePlans::default(),
            holding(&tasks, &[]),
        );
        let runner = std::sync::Arc::clone(&failed.env.runner);
        let harness = std::sync::Arc::clone(&failed.env.harness);
        let mut scheduler = arming_on(&runner, harness, SubEffectPoint::Synced);
        drive(&mut failed, Some(&mut scheduler)).expect_err("the append error ends the command");
        drop(scheduler);
        balanced_at_end(&mut failed.run).expect("an append error");
        assert!(next_start(failed, &tasks, "an append error").is_some());

        let mut shut = Wide::durable(
            "coordinator-ledgers-shutdown",
            &tasks,
            3,
            WidePlans::default(),
            holding(&tasks, &[]),
        );
        let runner = std::sync::Arc::clone(&shut.env.runner);
        let mut scheduler = shutdown_at_first_point(&runner);
        drive(&mut shut, Some(&mut scheduler)).expect_err("a shutdown ends the command");
        drop(scheduler);
        balanced_at_end(&mut shut.run).expect("a shutdown");
        assert!(next_start(shut, &tasks, "a shutdown").is_some());
    }
}
