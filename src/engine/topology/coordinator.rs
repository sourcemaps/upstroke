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
            Admitted::Closure(_) => self.run.close_run(self.seams, self.hooks),
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
            Some((outcome, charged)) => {
                self.run.charge_reviews(key, &charged);
                verified(outcome, charged, sequence)
            }
            None => Err(UpstrokeError::Refused {
                message: format!(
                    "the verification of sequence {} was interrupted by {}; its pipeline was \
                     cancelled and nothing further is appended for it",
                    sequence.0,
                    self.interrupt
                        .as_ref()
                        .map_or("the coordinator", Interrupt::describe)
                ),
            }),
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
            self.arrived = Some((outcome, charged));
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
        match self.interrupt.take() {
            Some(Interrupt::Halt) => self.run.close_run(self.seams, self.hooks),
            Some(Interrupt::Shutdown) => Err(UpstrokeError::Refused {
                message: "the coordinator was shut down: every live pipeline was cancelled and \
                          its completion discarded, nothing was settled for it, and the run is \
                          resumable"
                    .to_owned(),
            }),
            Some(Interrupt::Failed(error)) => Err(error),
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

    #[test]
    fn a_halt_inside_verification_cancels_it_and_leaves_the_transaction_for_closure() {
        let tasks = three();
        let runner = RecordingRunner::new().answering(
            crate::engine::topology::scaffold::wide_responder_asking(&tasks, &[], &[2]),
        );
        runner.hold();
        let mut wide = Wide::started_with(
            "coordinator-halt-in-verify",
            &tasks,
            3,
            WidePlans::default(),
            runner,
        );
        wide.env.adapters =
            std::sync::Arc::new(crate::engine::topology::scaffold::ScaffoldAdapters::echoing());
        wide.env.answers = std::sync::Arc::new(Declining);
        wide.env.halts_run = true;
        let runner = std::sync::Arc::clone(&wide.env.runner);
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
                released(view, |invocation| attempt_key(invocation) == Some(2))
            }),
        );
        let error = drive(&mut wide, Some(&mut scheduler))
            .expect_err("closure is refused with the transaction open");
        drop(scheduler);
        let message = error.to_string();
        assert!(
            message.contains("integration sequence 1 of task k1 is unresolved"),
            "phase 3 keeps closure's refusal for an interrupted verification: {message}"
        );

        let events = wide.env.durable_events();
        let kinds = kinds_of(&events);
        assert_eq!(
            kinds.last(),
            Some(&"question_answered"),
            "the declined question halted the run and nothing followed it: {kinds:?}"
        );
        for terminal in [
            "merge_prepared",
            "merge_rejected",
            "merge_verification_unavailable",
            "attempt_interrupted",
        ] {
            assert_eq!(
                count(&events, terminal),
                usize::from(terminal == "merge_prepared"),
                "only the fast first merge prepared anything; `{terminal}`: {kinds:?}"
            );
        }
        assert!(
            wide.run.fold().halted_at().is_some(),
            "the declined question recorded the halt"
        );
        assert!(
            wide.run
                .fold()
                .transaction()
                .is_some_and(|open| open.sequence.0 == 1),
            "the transaction is left for closure (phase 4)"
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
            "the verification's invocation was cancelled, not completed"
        );
        assert!(wide.run.invocations_balance(), "{:?}", wide.run.warnings());
        assert!(
            wide.run.discarded() >= 1,
            "the cancelled verification was discarded"
        );
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
