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
use crate::topology::events::{AttemptNumber, CandidateRef, GenerationId, SequenceId};
use crate::topology::registry::TaskKey;
use crate::workspace_manager::WorkspaceManager;

use super::attempt::{
    AttemptJob, AttemptPlans, JudgeError, Judged, Judgement, ReviewInputPolicy, ReviewPasses, Work,
    attempt_body,
};
use super::closure;
use super::identity::{Admission, AttemptIdentities, SequenceIdentities, SlotLimits, SlotPair};
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
        completed: bool,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Release {
    Invocation(InvocationId),
    Injected,
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
// refused (Running), an end reported (Running), its completion (Done); only
// `retire` removes it. Under an observer a grant is handed to it at once and
// nothing else is done until it returns, which it does when the invocation is
// inside the Runner, so no Invoking pipeline is still on its way there when the
// coordinator next acts. Winner and loser: a completion is settled only when its
// pipeline is live, not cancelled, bound to the identity it names, and that
// identity is open in the fold; any other message is discarded and counted and
// releases nothing twice. Cleanup: an interrupt cancels every live token,
// withdraws every pending registration and refuses every waiting reply; `finish`
// receives until no pipeline is live, and `join` awaits every handle, a panic
// being its pipeline's completion. The channels are bounded by this protocol, not
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
    in_verify: Option<PipelineId>,
    arrived: Option<(Result<Judgement, JudgeError>, Vec<ReviewRecord>)>,
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
            if ending {
                if halted && !self.live.is_empty() {
                    self.halt();
                }
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
        match settled {
            Ok(_) => Ok(true),
            Err(_) if self.interrupt.is_some() => Ok(false),
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
            if reply.send(Ok(())).is_ok() {
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
        let pipeline = self.spawn_verification(job);
        self.in_verify = Some(pipeline);
        self.arrived = None;
        self.gate.mode = GateMode::Verifying { granting: true };
        self.grant_snapshots();
        let arrived = loop {
            if let Err(error) = self.admit() {
                self.fail(error);
            }
            if self.interrupt.is_some() {
                break None;
            }
            if self.arrived.is_some() {
                self.gate.mode = GateMode::Verifying { granting: false };
                if self.gate.live() == 0 {
                    break self.arrived.take();
                }
            }
            self.receive_one();
        };
        self.in_verify = None;
        self.gate.mode = GateMode::Closed;
        match arrived {
            Some((outcome, charged)) => verified(outcome, charged, sequence),
            None => {
                self.cancelled_work.sequence(sequence);
                if let Some((Err(error), _)) = self.arrived.take() {
                    self.unresolved.extend(unresolved_judge(&error));
                }
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
                completed,
            } => {
                self.end_invocation(origin, pipeline, &invocation, completed);
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
                "`{invocation}` was granted to a pipeline that had stopped waiting, and \
                 withdrawing it failed: {error}"
            )),
        }
    }

    fn reply_granted(&mut self, granted: Vec<InvocationId>) {
        for invocation in granted {
            let Some((pipeline, reply)) = self.replies.remove(&invocation) else {
                self.run.warn(format!(
                    "the broker granted `{invocation}`, which no pipeline is waiting for"
                ));
                continue;
            };
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
        completed: bool,
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
        let settled = if completed {
            self.run.broker_mut().complete(invocation)
        } else {
            self.run.broker_mut().cancel(invocation)
        };
        match settled {
            Ok(granted) => self.reply_granted(granted),
            Err(error) => self.run.record_discard(Some(format!(
                "the end of `{invocation}` was refused by the invocation ledger: {error}"
            ))),
        }
    }

    fn begin_snapshot(&mut self, origin: Origin, pipeline: PipelineId, reply: Reply) {
        let known = origin == Origin::Pipeline
            && self
                .live
                .get(&pipeline)
                .is_some_and(|live| !live.cancelled && self.interrupt.is_none());
        if !known {
            let _ = reply.send(Err(UpstrokeError::Refused {
                message: format!(
                    "pipeline {} asked for a snapshot and is not live or is being cancelled",
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

    fn check(&mut self, origin: Origin, pipeline: PipelineId, identity: &Identity) -> Option<Live> {
        if self.run.fold().is_poisoned() {
            if origin == Origin::Pipeline {
                self.retire(pipeline);
            }
            self.run.record_discard(None);
            return None;
        }
        let Some(live) = self.live.get(&pipeline) else {
            self.run.record_discard(Some(format!(
                "a completion for {identity:?} came from pipeline {}, which is not live (stale \
                 or duplicate); it was discarded",
                pipeline.0
            )));
            return None;
        };
        if live.identity != *identity {
            let warning = format!(
                "a completion for {identity:?} came from pipeline {}, whose identity is {:?}; \
                 it was discarded",
                pipeline.0, live.identity
            );
            self.run.record_discard(Some(warning));
            return None;
        }
        if live.cancelled {
            if origin == Origin::Pipeline {
                self.retire(pipeline);
            }
            self.run.record_discard(None);
            return None;
        }
        if !identity.open_in(self.run) {
            if origin == Origin::Pipeline {
                self.retire(pipeline);
            }
            self.run.record_discard(Some(format!(
                "a completion for {identity:?} names an identity the fold does not hold open; \
                 it was discarded"
            )));
            return None;
        }
        if origin == Origin::Injected {
            self.run.record_discard(Some(format!(
                "a completion for {identity:?} was injected for pipeline {} while that \
                 pipeline is still running; it was discarded",
                pipeline.0
            )));
            return None;
        }
        self.retire(pipeline)
    }

    fn retire(&mut self, pipeline: PipelineId) -> Option<Live> {
        self.gate.release(pipeline);
        let live = self.live.remove(&pipeline)?;
        if let Some(invocation) = live.running.as_ref() {
            match self.run.broker_mut().cancel(invocation) {
                Ok(granted) => self.reply_granted(granted),
                Err(error) => self.run.warn(format!(
                    "pipeline {} ended holding `{invocation}`, and withdrawing it failed: \
                     {error}",
                    pipeline.0
                )),
            }
        }
        Some(live)
    }

    fn judged(
        &mut self,
        origin: Origin,
        pipeline: PipelineId,
        identity: &Identity,
        outcome: Result<Box<Judged>, UpstrokeError>,
    ) -> Result<(), UpstrokeError> {
        if let Err(error) = &outcome {
            self.note_cancelled_end(origin, pipeline, unresolved_runner(error));
        }
        let Some(live) = self.check(origin, pipeline, identity) else {
            return Ok(());
        };
        let judged = outcome?;
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
        if let Err(error) = &outcome {
            self.note_cancelled_end(origin, pipeline, unresolved_judge(error));
        }
        if self.in_verify != Some(pipeline) || self.arrived.is_some() {
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
        if self.check(origin, pipeline, identity).is_some() {
            if let Identity::Verification { candidate, .. } = identity {
                self.run.charge_reviews(candidate.key, &charged);
            }
            self.arrived = Some((outcome, charged));
        }
    }

    fn note_cancelled_end(
        &mut self,
        origin: Origin,
        pipeline: PipelineId,
        unresolved: Option<String>,
    ) {
        let cancelled = origin == Origin::Pipeline
            && self.live.get(&pipeline).is_some_and(|live| live.cancelled);
        if cancelled {
            self.unresolved.extend(unresolved);
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
            if live.busy != Busy::Done {
                live.busy = Busy::Running;
            }
        }
        let (_, invocations) = self.run.broker_mut().halves();
        invocations.withdraw_pending();
        for (invocation, (_, reply)) in std::mem::take(&mut self.replies) {
            let _ = reply.send(Err(cancelled(&invocation)));
        }
        for (pipeline, reply) in std::mem::take(&mut self.gate.waiting) {
            let _ = reply.send(Err(UpstrokeError::Refused {
                message: format!(
                    "pipeline {} was cancelled while it waited for a snapshot",
                    pipeline.0
                ),
            }));
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
                     ended when its pipeline was cancelled; closure appends nothing and scrubs \
                     nothing over a process that may still run, so the command ends and the run \
                     is resumable: the next process's census reclaims what is left before its \
                     recovery settles the rest",
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
                        "the Runner did not establish that the process of {} ended when its \
                         pipeline was cancelled",
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

    fn ended(&self, invocation: &InvocationId, completed: bool) -> Result<(), UpstrokeError> {
        if self.send(ToCoordinator::Ended {
            pipeline: self.pipeline,
            invocation: invocation.clone(),
            completed,
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

fn unresolved_runner(error: &UpstrokeError) -> Option<String> {
    match error {
        UpstrokeError::Runner {
            invocation, fate, ..
        } if fate.is_unresolved() => Some(invocation.clone()),
        _ => None,
    }
}

fn unresolved_judge(error: &JudgeError) -> Option<String> {
    match error {
        JudgeError::Runner(error) if error.fate.is_unresolved() => Some(error.invocation.render()),
        JudgeError::Runner(_) => None,
        JudgeError::Other(error) => unresolved_runner(error),
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
                            completed: true,
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
                                completed: true,
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
                                    completed: true,
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
        assert!(wide.run.invocations_balance(), "{:?}", wide.run.warnings());
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
        assert!(wide.run.invocations_balance(), "{:?}", wide.run.warnings());
    }
}
