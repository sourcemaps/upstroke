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
    Charged, DrivenJournal, Driver, Operator, Progress, Retrying, RunSeams, SpendAccount,
    TopologyRun, VerificationJob, verification_body, verified,
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
                    && !closure::cancelled_by_lineage(run.fold(), *sequence)
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
    Wake {
        token: u64,
    },
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
            Self::Shutdown | Self::Wake { .. } => None,
        }
    }

    const fn completes(&self) -> bool {
        matches!(self, Self::Judged { .. } | Self::Verified { .. })
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
        let (outbox, inbox) = mpsc::unbounded_channel();
        let (injector, injected) = mpsc::unbounded_channel();
        let timer = Timer::start(outbox.clone())?;
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
        let ledger = hooks.effects().durability_ledger();
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
            charged: None,
            interrupt: None,
            cancelled_work: closure::Cancelled::none(),
            unresolved: Vec::new(),
            buffer: Vec::new(),
            arrivals: 0,
            deferred: VecDeque::new(),
            wakes: 0,
            ledger,
            refusal: None,
            handles: Vec::new(),
            timer,
            inbox,
            outbox,
            injected,
            injector: Injector(injector),
            runtime,
        };
        let outcome = coordinator.drive();
        coordinator.join();
        drop(coordinator);
        self.reopen();
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
// releases nothing twice; a fatal completion interrupts as it is received. A
// verification completion from a live pipeline of this coordinator, settled or
// discarded, has its review passes charged and kept, and they are recorded
// (`merge_verification_charged`) when the integration or the command ends with no
// terminal carrying them. An
// invocation is released only when its end established that its process is
// gone: one that ended unresolved, or was never reported ended, keeps its
// registration and pair for the rest of this process, and interrupts. Every
// admission pass first acts on a halt the fold records, before it reconciles
// anything, then stops each pipeline whose identity the fold has closed, and
// selects nothing while `verify`'s own transaction is gone. A grant, of a pair
// or a snapshot, reaches only a live, uncancelled pipeline whose identity the
// fold holds open, with no interrupt recorded and no halt in the fold; one freed
// for any other is withdrawn and its request refused. A halt interrupts every
// in-flight identity the fold shows, a verification whose result has arrived
// unprepared included; a budget stop interrupts nothing, and the pipelines it
// drains are granted as before. Cleanup: an interrupt cancels every live token,
// withdraws every pending registration and refuses every waiting reply; `finish`
// receives until no pipeline is live, and `join` awaits every handle, a panic
// being its pipeline's completion. The channels are bounded by this protocol, not
// by type: a pipeline has at most one request unanswered and at most three
// notifications (an end, a snapshot's end, its completion) sent since it, and the
// fold's entitlements bound the pipelines. The coordinator keeps the job it
// settles against; the pipeline takes its own copy.
//
// A registry access the coordinator makes waits only for messages (R-F, R-R;
// the follow-up B record's §9.13 and §9.16): the coordinator lends itself as the
// transition's hooks — a dispatch, a continuation, a retry, a settlement, an
// integration, the run-end closure and its finalization — and each wait of the
// access — a backoff, or a turn of its wait for R-X — is `registry_pause`, which
// answers messages until the wake its `Timer` sends into its own inbox. The timer
// is one thread, started with the coordinator before it acts, so no wait needs a
// thread it may not get. Inside the wait it applies grants, ends, snapshot
// requests and a shutdown, which append nothing, and defers a completion until
// the transition returns, so nothing is appended and nothing is selected inside
// another transition (R-S); under an observer it keeps the canonical buffer and
// releases an invocation at quiescence, and an observer's append inside the wait
// is refused. A stale wake is dropped. A wait that ends with an interrupt
// recorded — a shutdown it answered, or an error that ends the command — stops
// the run and returns an error: the access stops before another attempt, the
// run's emitter refuses every further append, and every further effect made
// through the coordinator's hooks is refused at its `Before`, so the transition
// publishes, appends and spawns nothing further whatever it does with the error.
// The one append made past the stop is O14(b)'s record of the review passes a
// verification charged: an integration's recording point leaves them to
// `finish`, which records them after its last receive, every transition having
// returned, with the stop lifted for that append alone. A wait that begins with
// an interrupt recorded does not wait. The run reopens when the coordinator is
// gone.
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
    charged: Option<Charged>,
    interrupt: Option<Interrupt>,
    cancelled_work: closure::Cancelled,
    unresolved: Vec<String>,
    buffer: Vec<(u64, ToCoordinator)>,
    arrivals: u64,
    deferred: VecDeque<(Origin, ToCoordinator)>,
    wakes: u64,
    ledger: crate::util::DurabilityLedger,
    refusal: Option<String>,
    handles: Vec<tokio::task::JoinHandle<()>>,
    timer: Timer,
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
            if halted && self.any_in_flight() {
                self.halt();
                return Ok(());
            }
            self.reconcile();
            if ending {
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
                    let manager = self.seams.manager;
                    if let Retrying::Started(job) =
                        super::run::begin_retry(self, manager, key, generation)?
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
                    let manager = self.seams.manager;
                    let job =
                        super::run::begin_dispatch(self, manager, key, generation, continuing)?;
                    self.spawn_attempt(job);
                }
                Admitted::ApproveSpend { key } => {
                    self.open_gate();
                    self.run.ask_to_approve_spend(key, self.seams, self.hooks)?;
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
        if self.run.stopped().is_none() {
            if let (Some(charged), Err(ended)) = (self.charged.take(), &terminal) {
                if let Err(append) = self.run.record_charged(charged, self.seams, self.hooks) {
                    self.supersede(append, Some(ended));
                }
            }
        }
        let settled = self.run.integration_settled(key, terminal);
        let abandoned = self.abandoned.take() == Some(request.sequence);
        if settled.is_err() && abandoned && self.interrupt.is_none() {
            let seams = self.seams;
            super::run::settle_cancelled_verification(self, seams, request.sequence)?;
        }
        self.open_gate();
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
                        "pipeline {} is cancelled, serves an identity the fold no longer holds \
                         open, or is ending with the command (an interrupt is recorded, or the \
                         fold records a halt), so the snapshot it waited for was not granted",
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
                let seams = self.seams;
                super::run::hard_block(self, seams, &questions)
            }
            Admitted::Closure(_) => {
                let seams = self.seams;
                super::run::close_run(self, seams, &closure::Cancelled::none())
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
                    "the verification of sequence {} of task {key} was cancelled: a decline, or \
                     a failed settlement, failed its lineage, so the fold holds its transaction \
                     only until its one terminal, `merge_verification_interrupted`, which is \
                     appended once its pipeline has ended; the pipeline was stopped and its late \
                     result discarded, and the coordinator records first the review passes it \
                     charged, if any (`merge_verification_charged`)",
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
                         pipeline was cancelled, no terminal is appended for it here, and the \
                         coordinator records the review passes it charged, if any \
                         (`merge_verification_charged`), before the command ends",
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
        if let Some(deferred) = self.deferred.pop_front() {
            return Ok(deferred);
        }
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

    fn answer_until_woken(&mut self, token: u64) -> Result<(), UpstrokeError> {
        let woken = |message: &ToCoordinator| matches!(message, ToCoordinator::Wake { token: sent } if *sent == token);
        loop {
            if self.observer.is_none() {
                let message = self.inbox.blocking_recv().ok_or_else(closed)?;
                if woken(&message) {
                    return Ok(());
                }
                self.note_receipt(&message);
                self.answer_paused(Origin::Pipeline, message)?;
                continue;
            }
            if let Ok(message) = self.injected.try_recv() {
                self.answer_paused(Origin::Injected, message)?;
                continue;
            }
            while let Ok(message) = self.inbox.try_recv() {
                if woken(&message) {
                    return Ok(());
                }
                self.buffer_paused(message);
            }
            if self.any_running() {
                let message = self.inbox.blocking_recv().ok_or_else(closed)?;
                if woken(&message) {
                    return Ok(());
                }
                self.buffer_paused(message);
                continue;
            }
            if let Some(message) = self.take_canonical_answerable() {
                self.answer_paused(Origin::Pipeline, message)?;
                continue;
            }
            if self.observe_paused()? {
                continue;
            }
            let message = self.inbox.blocking_recv().ok_or_else(closed)?;
            if woken(&message) {
                return Ok(());
            }
            self.buffer_paused(message);
        }
    }

    fn answer_paused(
        &mut self,
        origin: Origin,
        message: ToCoordinator,
    ) -> Result<(), UpstrokeError> {
        match message {
            ToCoordinator::Wake { .. } => Ok(()),
            message if message.completes() => {
                self.deferred.push_back((origin, message));
                Ok(())
            }
            message => self.handle(origin, message),
        }
    }

    fn buffer_paused(&mut self, message: ToCoordinator) {
        if !matches!(message, ToCoordinator::Wake { .. }) {
            self.buffer_message(message);
        }
    }

    fn take_canonical_answerable(&mut self) -> Option<ToCoordinator> {
        let index = self
            .buffer
            .iter()
            .enumerate()
            .filter(|(_, (_, message))| !message.completes())
            .min_by_key(|(_, (arrival, message))| (message.pipeline(), *arrival))
            .map(|(index, _)| index)?;
        Some(self.buffer.remove(index).1)
    }

    fn observe_paused(&mut self) -> Result<bool, UpstrokeError> {
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
        let Some(observer) = self.observer.as_deref_mut() else {
            return Ok(false);
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
                Ok(owner.is_some_and(|live| {
                    live.busy = Busy::Running;
                    true
                }))
            }
            Release::Injected => Ok(!self.injected.is_empty()),
            Release::Append(_) => Err(UpstrokeError::Refused {
                message: "the observer asked for an append inside a registry access's wait; \
                          nothing is appended inside another transition, so the command ends"
                    .to_owned(),
            }),
            Release::Nothing => Ok(false),
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
            ToCoordinator::Shutdown | ToCoordinator::Wake { .. } => return,
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
            ToCoordinator::Wake { .. } => Ok(()),
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
            Some(live) if live.cancelled || self.interrupted() => {
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

    fn interrupted(&self) -> bool {
        self.interrupt.is_some() || self.run.fold().halted_at().is_some()
    }

    fn receives(&self, pipeline: PipelineId) -> bool {
        !self.interrupted()
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
                    "the broker granted `{invocation}` to pipeline {}, which is cancelled, serves \
                     an identity the fold no longer holds open, or is ending with the command (an \
                     interrupt is recorded, or the fold records a halt); the grant was withdrawn \
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
                    "pipeline {} asked for a snapshot and is not live, is being cancelled, serves \
                     an identity the fold no longer holds open, or is ending with the command (an \
                     interrupt is recorded, or the fold records a halt)",
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
        let manager = self.seams.manager;
        super::run::settle_judged(self, manager, &job, &judged).map(drop)
    }

    fn verified_arrived(
        &mut self,
        origin: Origin,
        pipeline: PipelineId,
        identity: &Identity,
        outcome: Result<Judgement, JudgeError>,
        charged: Vec<ReviewRecord>,
    ) {
        let reported = match identity {
            Identity::Verification {
                sequence,
                candidate,
            } if origin == Origin::Pipeline
                && self
                    .live
                    .get(&pipeline)
                    .is_some_and(|live| live.identity == *identity) =>
            {
                Some((*sequence, candidate.key))
            }
            _ => None,
        };
        if self.in_verify.as_ref().map(|(verifying, _)| *verifying) != Some(pipeline)
            || self.arrived.is_some()
        {
            if origin == Origin::Pipeline
                && self.live.get(&pipeline).is_some_and(|live| live.cancelled)
            {
                self.keep_discarded(reported, charged);
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
            self.keep_discarded(reported, charged);
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
        self.charged = Some(Charged {
            sequence: *sequence,
            reviews: charged.clone(),
        });
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

    fn keep_discarded(
        &mut self,
        reported: Option<(SequenceId, TaskKey)>,
        charged: Vec<ReviewRecord>,
    ) {
        if let Some((sequence, key)) = reported {
            self.run.charge_reviews(key, &charged);
            self.charged = Some(Charged {
                sequence,
                reviews: charged,
            });
        }
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

    fn supersede(&mut self, append: UpstrokeError, ended: Option<&UpstrokeError>) {
        let cause = match self.interrupt.take() {
            Some(Interrupt::Failed(error)) => error.to_string(),
            Some(interrupt) => interrupt.describe().to_owned(),
            None => ended.map_or_else(|| "no recorded cause".to_owned(), ToString::to_string),
        };
        self.run.warn(super::run::superseded(&cause));
        self.interrupt = Some(Interrupt::Failed(append));
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
        if let Some(charged) = self.charged.take() {
            if let Err(append) = self
                .run
                .record_charged_past_the_stop(charged, self.seams, self.hooks)
            {
                self.supersede(append, None);
            }
        }
        let unresolved = std::mem::take(&mut self.unresolved);
        match self.interrupt.take() {
            Some(Interrupt::Halt) if unresolved.is_empty() => {
                let cancelled = std::mem::take(&mut self.cancelled_work);
                let seams = self.seams;
                super::run::close_run(self, seams, &cancelled)
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
                         {}nothing was settled, nothing was appended but the record of the review \
                         passes a verification had charged, and the run is resumable{}",
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
        if let Some(panicked) = self.timer.stop() {
            self.run.warn(panicked);
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

impl Operator for Coordinator<'_> {
    fn parts(&mut self) -> (&mut TopologyRun, &RunSeams<'_>, &mut dyn TopologyHooks) {
        (&mut *self.run, self.seams, &mut *self.hooks)
    }

    fn driven(&self) -> &TopologyRun {
        self.run
    }

    fn registry(&mut self) -> &mut dyn TopologyHooks {
        self
    }
}

impl TopologyHooks for Coordinator<'_> {
    fn effects(&mut self) -> &mut dyn crate::workspace_manager::EffectHooks {
        self
    }

    fn rundir(&mut self) -> &mut dyn crate::rundir::RunDirHooks {
        self.hooks.rundir()
    }

    fn events(&mut self) -> &mut dyn crate::events::log::EventHooks {
        self.hooks.events()
    }

    fn container(&mut self) -> &mut dyn crate::runner::container::ContainerHooks {
        self.hooks.container()
    }

    fn spawn(&mut self) -> &mut dyn crate::agent::proc::SpawnHooks {
        TopologyHooks::spawn(&mut *self.hooks)
    }
}

impl crate::workspace_manager::EffectHooks for Coordinator<'_> {
    fn phase(
        &mut self,
        site: crate::topology::effects::EffectSiteId,
        phase: crate::topology::effects::HookPhase,
    ) -> crate::topology::effects::Injection {
        if let Some(why) = self.run.stopped().map(str::to_owned) {
            if let crate::topology::effects::HookPhase::Point {
                mode: crate::topology::effects::InjectionMode::Kill,
                ..
            } = phase
            {
                return crate::topology::effects::Injection::Proceed;
            }
            self.refusal = Some(format!("nothing further is done in this process: {why}"));
            return crate::topology::effects::Injection::Error;
        }
        let effects = self.hooks.effects();
        let answer = effects.phase(site, phase);
        let cause = if answer == crate::topology::effects::Injection::Proceed {
            None
        } else {
            effects.refusal_cause()
        };
        self.refusal = cause;
        answer
    }

    fn durability_ledger(&self) -> crate::util::DurabilityLedger {
        self.ledger.clone()
    }

    fn refusal_cause(&self) -> Option<String> {
        self.refusal.clone()
    }

    fn registry_pause(&mut self, pause: std::time::Duration) -> Result<(), UpstrokeError> {
        if self.interrupt.is_none() {
            self.wakes = self.wakes.wrapping_add(1);
            let token = self.wakes;
            let waited = match self.timer.wake(token, pause) {
                Ok(()) => self.answer_until_woken(token),
                Err(refusal) => Err(refusal),
            };
            if let Err(error) = waited {
                self.fail(error);
            }
        }
        let Some(interrupt) = &self.interrupt else {
            return Ok(());
        };
        let stopped = format!(
            "a registry access's wait inside a transition met {}, so the access stopped there \
             and its transition appends, publishes and spawns nothing further",
            interrupt.describe()
        );
        self.run.stop(stopped.clone());
        Err(UpstrokeError::Refused { message: stopped })
    }
}

impl Driver for Coordinator<'_> {
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

// The coordinator's wait timer (the record's §9.16, I2-3 and I2-5). Owner: the
// coordinator, which starts it before it builds its runtime and stops and joins
// it when it ends; the thread holds a clone of the coordinator's own outbox.
// Protocol: one request at a time — a wait does not nest — carries a token and a
// length; the thread sleeps the length and then sends `Wake { token }` into the
// inbox. The sleep runs under `catch_unwind`, so a sleep that unwinds is still
// answered and the thread goes on serving: the thread's only calls are the wait
// for the next request, the sleep and the send, it ends only when the
// coordinator drops its sender, and so every request it accepted is answered
// within its length. A request it can no longer accept is the coordinator's
// typed refusal, never a sleep. The join reports a sleep that unwound, or a
// thread that did, as a warning.
struct Timer {
    requests: Option<std::sync::mpsc::Sender<(u64, std::time::Duration)>>,
    thread: Option<std::thread::JoinHandle<u32>>,
}

impl Timer {
    fn start(outbox: mpsc::UnboundedSender<ToCoordinator>) -> Result<Self, UpstrokeError> {
        Self::starting(outbox, std::thread::sleep)
    }

    fn starting(
        outbox: mpsc::UnboundedSender<ToCoordinator>,
        sleep: fn(std::time::Duration),
    ) -> Result<Self, UpstrokeError> {
        let (requests, waits) = std::sync::mpsc::channel::<(u64, std::time::Duration)>();
        let thread = std::thread::Builder::new()
            .name("upstroke-coordinator-timer".to_owned())
            .spawn(move || {
                let mut unwound: u32 = 0;
                while let Ok((token, pause)) = waits.recv() {
                    if std::panic::catch_unwind(|| sleep(pause)).is_err() {
                        unwound = unwound.saturating_add(1);
                    }
                    let _ = outbox.send(ToCoordinator::Wake { token });
                }
                unwound
            })
            .map_err(|error| UpstrokeError::Refused {
                message: format!(
                    "the coordinator's wait timer could not be started ({error}), so no registry \
                     access's wait could be answered without sleeping on the coordinator; \
                     nothing was spawned or appended, and the run is resumable"
                ),
            })?;
        Ok(Self {
            requests: Some(requests),
            thread: Some(thread),
        })
    }

    fn wake(&self, token: u64, pause: std::time::Duration) -> Result<(), UpstrokeError> {
        self.requests
            .as_ref()
            .and_then(|requests| requests.send((token, pause)).ok())
            .ok_or_else(|| UpstrokeError::Refused {
                message: "the coordinator's wait timer has stopped, so a registry access's wait \
                          could not be timed; the access stopped rather than sleep on the \
                          coordinator, and the command ends resumably"
                    .to_owned(),
            })
    }

    fn stop(&mut self) -> Option<String> {
        self.requests = None;
        let thread = self.thread.take()?;
        match thread.join() {
            Ok(0) => None,
            Ok(unwound) => Some(format!(
                "the coordinator's wait timer unwound inside {unwound} of its sleeps and answered \
                 each wait all the same"
            )),
            Err(_) => Some("the coordinator's wait timer's thread panicked".to_owned()),
        }
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        let _ = self.stop();
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
mod tests;
