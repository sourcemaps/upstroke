//! Extended notes: `docs/internals/engine/topology/run.md`

use std::collections::BTreeMap;

use crate::error::{ProcessFate, UpstrokeError};
use crate::events::RunOutcome;
use crate::ir::{Answer, Question, QuestionId};
use crate::review;
use crate::topology::events::Answer4;
use crate::topology::events::TopologyEventBody;
use crate::topology::fold::QuestionOrigin;

use crate::events::AttemptRecord;
use crate::interaction::Sleeper;
use crate::topology::events::{
    AttemptNumber, CandidateLeaseEffect, CandidateRef, CommitSha, FrozenQuestion,
    GenerationCloseReason, GenerationId, InfrastructureKind, Materialization, SequenceId,
    SessionId, TopologyEvent,
};
use crate::topology::fold::{FrozenInputs, TopologyFold};
use crate::topology::registry::TaskKey;
use crate::workspace_manager::WorkspaceManager;

use super::attempt::{
    AttemptContext, AttemptJob, AttemptPlans, AttemptSite, InputsRequest, JudgeError,
    JudgeIdentities, JudgeNames, Judged, Judgement, PlanRequest, ReviewAccount, ReviewInputPolicy,
    ReviewPasses, SnapshotDisposal, SnapshotOf, Subject, VerificationRequest, Work, attempt_body,
};
use super::candidate::{
    CandidateJournal, JudgedTree, append_candidate_created, append_candidate_prepared,
    create_candidates_ref, pin_candidate, reclaim_after_creation, write_candidate_commit,
};
use super::closure;
use super::dispatch::{
    DispatchJournal, DispatchKind, DispatchRequest, Dispatched, EventEmitter, OpenGeneration,
    dispatch_through, resume_open_no_attempt, task_slot,
};
use super::emit::{EmitFailure, EmitState, RunIdentity, emit};
use super::finalize;
use super::identity::{InvocationLedger, ReservationKind, SequenceIdentities, SlotLimits};
use super::integrate::{
    self, IntegrationJournal, IntegrationRequest, Terminal, Verification, Verified, VerifyRequest,
};
use super::permits::PermitBroker;
use super::preflight::Carried;
use super::recover::RunHandle;
use super::seams::{IdSource, TimeSource, TopologyHooks};
use super::select::{Admitted, Ceiling, Entitlements, Spend, Standing, Step, checkpoint, select};
use super::settle::{
    Deferral, FinishedAttempt, ManagedWorktrees, RetryOutcome, RetryRequest, WorktreeVerify,
    close_generation, settle_failed,
};

pub struct RunEmitter<'a> {
    pub identity: &'a RunIdentity,
    pub state: EmitState<'a>,
    pub clock: &'a dyn TimeSource,
}

impl EventEmitter for RunEmitter<'_> {
    fn emit(
        &mut self,
        body: TopologyEventBody,
        hooks: &mut dyn TopologyHooks,
    ) -> Result<(), EmitFailure> {
        emit(self.identity, &mut self.state, self.clock, body, hooks)?;
        Ok(())
    }

    fn standing(&self, invocation: &crate::runner::InvocationId) -> Standing {
        Standing::of(self.state.fold, invocation)
    }
}

struct RunJournal<'a, 'h> {
    emitter: RunEmitter<'a>,
    hooks: &'h mut dyn TopologyHooks,
    invocations: &'h mut InvocationLedger,
}

impl CandidateJournal for RunJournal<'_, '_> {
    fn emit(&mut self, body: TopologyEventBody) -> Result<(), UpstrokeError> {
        self.emitter
            .emit(body, self.hooks)
            .map_err(|failure| failure.discharging(self.invocations))
    }

    fn fold(&self) -> &TopologyFold {
        self.emitter.state.fold
    }
}

impl IntegrationJournal for RunJournal<'_, '_> {
    fn emit(&mut self, body: TopologyEventBody) -> Result<(), UpstrokeError> {
        self.emitter
            .emit(body, self.hooks)
            .map_err(|failure| failure.discharging(self.invocations))
    }

    fn fold(&self) -> &TopologyFold {
        self.emitter.state.fold
    }

    fn hooks(&mut self) -> &mut dyn TopologyHooks {
        &mut *self.hooks
    }

    fn converted(&mut self, key: TaskKey) -> Result<(), UpstrokeError> {
        self.emitter
            .state
            .reservations
            .convert(key, ReservationKind::Integration)
    }
}

struct IntegrationCx<'a, 'h> {
    emitter: RunEmitter<'a>,
    hooks: &'h mut dyn TopologyHooks,
    invocations: &'h mut InvocationLedger,
    spend: &'h mut Spend,
    seams: &'h RunSeams<'a>,
}

impl IntegrationJournal for IntegrationCx<'_, '_> {
    fn emit(&mut self, body: TopologyEventBody) -> Result<(), UpstrokeError> {
        self.emitter
            .emit(body, self.hooks)
            .map_err(|failure| failure.discharging(self.invocations))
    }

    fn fold(&self) -> &TopologyFold {
        self.emitter.state.fold
    }

    fn hooks(&mut self) -> &mut dyn TopologyHooks {
        &mut *self.hooks
    }

    fn converted(&mut self, key: TaskKey) -> Result<(), UpstrokeError> {
        self.emitter
            .state
            .reservations
            .convert(key, ReservationKind::Integration)
    }
}

pub(super) trait Operator {
    fn parts(&mut self) -> (&mut TopologyRun, &RunSeams<'_>, &mut dyn TopologyHooks);

    fn driven(&self) -> &TopologyRun;

    fn registry(&mut self) -> &mut dyn TopologyHooks;
}

pub(super) trait Driver: Operator {
    fn seams(&self) -> &RunSeams<'_>;

    fn verify(&mut self, request: &VerifyRequest<'_>) -> Result<Verified, UpstrokeError>;
}

pub(super) struct Stepping<'a, 's> {
    pub run: &'a mut TopologyRun,
    pub seams: &'a RunSeams<'s>,
    pub hooks: &'a mut dyn TopologyHooks,
}

impl Operator for Stepping<'_, '_> {
    fn parts(&mut self) -> (&mut TopologyRun, &RunSeams<'_>, &mut dyn TopologyHooks) {
        (&mut *self.run, self.seams, &mut *self.hooks)
    }

    fn driven(&self) -> &TopologyRun {
        self.run
    }

    fn registry(&mut self) -> &mut dyn TopologyHooks {
        &mut *self.hooks
    }
}

struct OperatorJournal<'o, O: ?Sized>(&'o mut O);

impl<O: Operator + ?Sized> DispatchJournal for OperatorJournal<'_, O> {
    fn emit(&mut self, body: TopologyEventBody) -> Result<(), EmitFailure> {
        let (run, seams, hooks) = self.0.parts();
        run.emit_undischarged(body, seams, hooks)
    }

    fn hooks(&mut self) -> &mut dyn TopologyHooks {
        self.0.registry()
    }
}

pub(super) fn begin_dispatch<O: Operator + ?Sized>(
    operator: &mut O,
    manager: &WorkspaceManager,
    key: TaskKey,
    generation: GenerationId,
    continuing: bool,
) -> Result<AttemptJob, UpstrokeError> {
    let dispatched = if continuing {
        continue_open(operator, manager, key, generation)?
    } else {
        dispatch_ready(operator, manager, key, generation)?
    };
    let (run, seams, hooks) = operator.parts();
    run.first_attempt(&dispatched, seams, hooks)
}

fn dispatch_ready<O: Operator + ?Sized>(
    operator: &mut O,
    manager: &WorkspaceManager,
    key: TaskKey,
    generation: GenerationId,
) -> Result<Dispatched, UpstrokeError> {
    let (kind, authorized, refname) = operator.driven().dispatch_inputs(key)?;
    let base = integrate::dispatch_head_at(
        manager,
        operator.registry().effects(),
        authorized,
        &refname,
        key,
    )?;
    let request = DispatchRequest {
        key,
        generation,
        base,
        kind,
    };
    operator.parts().0.reserve_dispatch(key)?;
    let dispatched = dispatch_through(manager, &mut OperatorJournal(&mut *operator), &request);
    operator.parts().0.dispatch_settled(key, dispatched)
}

fn continue_open<O: Operator + ?Sized>(
    operator: &mut O,
    manager: &WorkspaceManager,
    key: TaskKey,
    generation: GenerationId,
) -> Result<Dispatched, UpstrokeError> {
    let (open, kind) = operator.driven().open_to_continue(key, generation)?;
    let resumed = resume_open_no_attempt(manager, operator.registry(), &open)?;
    operator.parts().0.deferral.progressed();
    Ok(Dispatched {
        key,
        generation,
        base: open.base,
        worktree: manager.slot_path(&open.slot),
        slot: open.slot,
        kind,
        materialized: resumed.materialized,
    })
}

pub(super) fn begin_retry<O: Operator + ?Sized>(
    operator: &mut O,
    manager: &WorkspaceManager,
    key: TaskKey,
    generation: GenerationId,
) -> Result<Retrying, UpstrokeError> {
    let request = {
        let (run, seams, _) = operator.parts();
        run.retry_request(key, generation, seams)?
    };
    let begun = {
        let run = operator.parts().0;
        super::settle::retry_begin(&run.handle.fold, run.broker.halves().0, &request)?
    };
    let verified = ManagedWorktrees::new(manager).verify(
        operator.registry().effects(),
        &request.slot,
        &crate::workspace_manager::Quiescence::HoldsTree(request.retained_tree.clone()),
    );
    let outcome = {
        let run = operator.parts().0;
        super::settle::retry_end(
            &run.handle.fold,
            run.broker.halves().0,
            &request,
            begun,
            verified,
        )?
    };
    match outcome {
        RetryOutcome::Start(started) => {
            let (run, seams, hooks) = operator.parts();
            let job = run.retry_started(generation, *started, &request.slot, seams, hooks)?;
            Ok(Retrying::Started(Box::new(job)))
        }
        RetryOutcome::Close { closed, .. } => {
            {
                let (run, seams, hooks) = operator.parts();
                run.retry_closed(key, closed, seams, hooks)?;
            }
            super::dispatch::scrub(manager, operator.registry(), &request.slot)?;
            Ok(Retrying::Closed { key })
        }
    }
}

pub(super) fn settle_judged<O: Operator + ?Sized>(
    operator: &mut O,
    manager: &WorkspaceManager,
    job: &AttemptJob,
    judged: &Judged,
) -> Result<Progress, UpstrokeError> {
    let accepted = judged.judgement.accepted();
    let spent_attempt = settle(
        operator,
        manager,
        job.site(),
        &job.plan,
        Produced {
            capture: &judged.capture,
            assessed: &judged.assessed,
            judgement: &judged.judgement,
        },
    )?;
    Ok(Progress::Settled {
        key: job.key,
        accepted,
        spent_attempt,
    })
}

fn settle<O: Operator + ?Sized>(
    operator: &mut O,
    manager: &WorkspaceManager,
    site: AttemptSite<'_>,
    plan: &super::attempt::AttemptPlan,
    produced: Produced<'_>,
) -> Result<bool, UpstrokeError> {
    let record = crate::engine::classify::attempt_record(
        plan.attempt.0,
        crate::engine::classify::AttemptFacts {
            tier: plan.binding.tier,
            model: &plan.binding.model,
            pool: plan.pool.clone(),
            resumed: plan.resume_session.is_some(),
            outcome: &produced.assessed.outcome,
            reviews: &produced.judgement.reviews,
            failure: produced.judgement.failure.as_ref(),
            feedback: crate::engine::classify::FeedbackCarrier::AttemptRecord,
        },
    );
    let Some(failure) = produced.judgement.failure.as_ref() else {
        promote_candidate(operator, manager, site, plan, produced.capture, record)?;
        return Ok(crate::ladder::spends_allowance(None));
    };
    let (closed, spent_attempt) = {
        let (run, seams, hooks) = operator.parts();
        run.settle_failure(site, plan, produced, record, failure, seams, hooks)?
    };
    if closed {
        super::dispatch::scrub(manager, operator.registry(), site.slot)?;
    }
    Ok(spent_attempt)
}

fn promote_candidate<O: Operator + ?Sized>(
    operator: &mut O,
    manager: &WorkspaceManager,
    site: AttemptSite<'_>,
    plan: &super::attempt::AttemptPlan,
    capture: &super::attempt::Capture,
    record: AttemptRecord,
) -> Result<(), UpstrokeError> {
    let actual_paths =
        manager.changed_paths_pausing(operator.registry().effects(), site.slot, &capture.parent)?;
    let (judged, run_id) =
        operator
            .driven()
            .judged_tree(site, plan, capture, &record, actual_paths)?;
    let unpinned = write_candidate_commit(manager, operator.registry(), &run_id, judged)?;
    let pinned = pin_candidate(manager, operator.registry(), unpinned)?;
    operator.parts().0.record_promoted(site.key, &record);
    let promoting = {
        let (run, seams, hooks) = operator.parts();
        run.with_journal(seams, hooks, |journal| {
            append_candidate_prepared(journal, pinned)
        })?
    };
    let referenced = create_candidates_ref(manager, operator.registry(), promoting)?;
    let created = {
        let (run, seams, hooks) = operator.parts();
        run.with_journal(seams, hooks, |journal| {
            append_candidate_created(journal, referenced)
        })?
    };
    reclaim_after_creation(manager, operator.registry(), site.slot, created)?;
    Ok(())
}

pub(super) struct DrivenJournal<'d, D: ?Sized>(pub &'d mut D);

impl<D: Driver + ?Sized> IntegrationJournal for DrivenJournal<'_, D> {
    fn emit(&mut self, body: TopologyEventBody) -> Result<(), UpstrokeError> {
        let (run, seams, hooks) = self.0.parts();
        run.emit(body, seams, hooks)
    }

    fn fold(&self) -> &TopologyFold {
        self.0.driven().fold()
    }

    fn hooks(&mut self) -> &mut dyn TopologyHooks {
        self.0.registry()
    }

    fn converted(&mut self, key: TaskKey) -> Result<(), UpstrokeError> {
        self.0.parts().0.convert_integration(key)
    }
}

impl<D: Driver + ?Sized> Verification for DrivenJournal<'_, D> {
    fn verify(&mut self, request: &VerifyRequest<'_>) -> Result<Verified, UpstrokeError> {
        self.0.verify(request)
    }

    fn ids(&self) -> &dyn super::seams::IdSource {
        self.0.seams().ids
    }
}

fn implementer_binding(
    fold: &TopologyFold,
    key: TaskKey,
) -> Result<crate::review::PassBinding, UpstrokeError> {
    let rung = fold
        .task(key)
        .ok_or_else(|| UpstrokeError::Refused {
            message: format!("task {key} is not in this run's fold"),
        })?
        .rung;
    let binding = fold
        .rung_binding(key, rung)
        .ok_or_else(|| UpstrokeError::Refused {
            message: format!(
                "task {key}'s candidate was produced at rung {rung} and neither a validated \
                 one-off binding nor a frozen rung binds it, so there is no implementer to \
                 select its reviewers against"
            ),
        })?;
    Ok(crate::review::PassBinding::new(
        &binding.agent,
        &binding.model,
    ))
}

impl Verification for IntegrationCx<'_, '_> {
    fn verify(&mut self, request: &VerifyRequest<'_>) -> Result<Verified, UpstrokeError> {
        let job = VerificationJob::of(self.emitter.state.fold, request)?;
        let mut charged = Vec::new();
        let outcome = {
            let ledger = std::sync::Mutex::new(&mut *self.invocations);
            let carried = Carried::default();
            let standing = Standing::of(
                self.emitter.state.fold,
                &SequenceIdentities::new(request.sequence).gate(0, 0),
            );
            let mut work = Work {
                manager: self.seams.manager,
                hooks: &mut *self.hooks,
                runner: self.seams.runner,
                standing,
                registrar: &ledger,
                carried: &carried,
                adapters: self.seams.adapters,
                paths: self.seams.paths,
                plans: self.seams.plans,
                reviews: self.seams.reviews,
                input_policy: self.seams.input_policy,
            };
            let mut account = SpendAccount {
                spend: Some(&mut *self.spend),
                key: request.candidate.key,
                charged: &mut charged,
            };
            verification_body(&mut work, &job, &mut account)
        };
        verified(outcome, charged, request.sequence)
    }

    fn ids(&self) -> &dyn super::seams::IdSource {
        self.seams.ids
    }
}

pub fn verified(
    outcome: Result<Judgement, JudgeError>,
    charged: Vec<crate::events::ReviewRecord>,
    sequence: SequenceId,
) -> Result<Verified, UpstrokeError> {
    match outcome {
        Ok(judgement) => Ok(Verified::Judged(judgement)),
        Err(JudgeError::Runner(error)) if error.is_cancelled() => Err(error.into()),
        Err(JudgeError::Runner(error)) => match error.fate {
            ProcessFate::NeverStarted => Ok(Verified::Unavailable {
                kind: InfrastructureKind::RunnerSpawnFailure,
                detail: error.to_string(),
                reviews: charged,
            }),
            ProcessFate::Gone => Ok(Verified::Unavailable {
                kind: InfrastructureKind::Other {
                    detail: format!(
                        "the Runner lost `{}` after its process started and has since \
                         established the process is gone; nothing was judged",
                        error.invocation
                    ),
                },
                detail: error.to_string(),
                reviews: charged,
            }),
            ProcessFate::Unresolved => Err(error.into()),
        },
        Err(JudgeError::Other(UpstrokeError::Git { message })) => Ok(Verified::Unavailable {
            kind: InfrastructureKind::Other {
                detail: format!(
                    "foreign Git state observed by the verification of sequence {}: {message}",
                    sequence.0
                ),
            },
            detail: message,
            reviews: charged,
        }),
        Err(JudgeError::Other(error)) => Err(error),
    }
}

#[derive(Debug, Clone)]
pub struct VerificationJob {
    pub candidate: CandidateRef,
    pub sequence: SequenceId,
    pub staging: crate::workspace_manager::Slot,
    pub head: CommitSha,
    pub proposed: CommitSha,
    pub already_present: bool,
    entry: crate::topology::registry::TaskEntry,
    base: Option<CommitSha>,
    implementer: crate::review::PassBinding,
}

impl VerificationJob {
    pub fn of(fold: &TopologyFold, request: &VerifyRequest<'_>) -> Result<Self, UpstrokeError> {
        let key = request.candidate.key;
        let entry = fold
            .registry()
            .and_then(|registry| registry.get(key))
            .cloned()
            .ok_or_else(|| UpstrokeError::Refused {
                message: format!("task {key} is not in this run's registry"),
            })?;
        let base = fold
            .task(key)
            .and_then(|task| {
                task.generations
                    .iter()
                    .find(|generation| generation.id == request.candidate.generation)
            })
            .and_then(|generation| generation.candidate.as_ref())
            .map(|prepared| prepared.base_sha.clone());
        Ok(Self {
            candidate: request.candidate.clone(),
            sequence: request.sequence,
            staging: request.staging.clone(),
            head: request.head.clone(),
            proposed: request.proposed.clone(),
            already_present: request.already_present,
            entry,
            base,
            implementer: implementer_binding(fold, key)?,
        })
    }
}

pub fn verification_body(
    work: &mut Work<'_>,
    job: &VerificationJob,
    account: &mut dyn ReviewAccount,
) -> Result<Judgement, JudgeError> {
    let (diff_parent, diff_tree) = if job.already_present {
        let base = job.base.as_ref().ok_or_else(|| {
            JudgeError::Other(UpstrokeError::Refused {
                message: "an already-present verification needs the candidate's recorded \
                          base to review its original patch"
                    .to_owned(),
            })
        })?;
        (base.0.clone(), job.candidate.commit_sha.0.clone())
    } else {
        (job.head.0.clone(), job.proposed.0.clone())
    };
    let diff = work
        .manager
        .candidate_diff(&job.staging, &diff_parent, &diff_tree)
        .map_err(JudgeError::Other)?;

    let plan = work
        .plans
        .verification(&VerificationRequest {
            entry: &job.entry,
            implementer: job.implementer.clone(),
        })
        .map_err(JudgeError::Other)?;

    let prior_failure =
        match crate::engine::classify::unjudgeable_diff(&diff, !plan.reviewers.is_empty()) {
            Some(failure) => Some(failure),
            None => {
                let tree = work
                    .manager
                    .commit_tree_sha(job.proposed.as_str())
                    .map_err(JudgeError::Other)?
                    .ok_or_else(|| {
                        JudgeError::Other(UpstrokeError::Git {
                            message: format!(
                                "the proposed commit {} has no tree; the review-input policy \
                                 cannot be consulted for it",
                                job.proposed
                            ),
                        })
                    })?;
                let staging = work.manager.slot_path(&job.staging);
                work.input_policy
                    .problem(&staging, &tree)
                    .map_err(JudgeError::Other)?
                    .map(crate::engine::classify::review_input_failure)
            }
        };

    let inputs = work
        .plans
        .inputs(&InputsRequest {
            entry: &job.entry,
            diff,
        })
        .map_err(JudgeError::Other)?;

    let proposed =
        crate::workspace_manager::ObjectId::new(job.proposed.0.clone()).map_err(|refusal| {
            JudgeError::Other(UpstrokeError::Refused {
                message: format!(
                    "the recorded proposal of sequence {} is not an object id: {refusal}",
                    job.sequence.0
                ),
            })
        })?;
    let identities = SequenceIdentities::new(job.sequence);
    work.judge().judge(
        &Subject {
            snapshot: SnapshotOf::Commit(proposed),
            disposal: SnapshotDisposal::AfterTheTerminal,
            names: JudgeNames::Integration {
                sequence: u64::from(job.sequence.0),
            },
            identities: JudgeIdentities::Sequence(identities),
            stem: format!("integration-s{}", job.sequence.0),
            gates: &plan.gates,
            reviewers: &plan.reviewers,
            inputs: &inputs,
            prior_failure,
            invocations: &move |pass| review::ReviewInvocations {
                pass: identities.review_pass(pass, 0),
                reask: identities.review_reask(pass, 0),
            },
        },
        account,
    )
}

pub struct SpendAccount<'a> {
    pub spend: Option<&'a mut Spend>,
    pub key: TaskKey,
    pub charged: &'a mut Vec<crate::events::ReviewRecord>,
}

impl ReviewAccount for SpendAccount<'_> {
    fn charge(&mut self, review: &crate::events::ReviewRecord) {
        if let Some(spend) = self.spend.as_deref_mut() {
            spend.record_review_cost(self.key, review.cost_usd);
        }
        self.charged.push(review.clone());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LoopBranch {
    IngestAnswers,
    Integration,
    ReadyRetry,
    ReadyDispatch,
    DeferBackoff,
    HardBlock,
    Closure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    Performed,
    RefusedByCheckpoint,
    NotYetImplemented,
    #[allow(dead_code)]
    NotThisSlice {
        slice: &'static str,
        citation: &'static str,
    },
    #[allow(dead_code)]
    PartlyImplemented {
        performs: &'static str,
        owes: &'static str,
    },
}

impl LoopBranch {
    pub const ALL: [Self; 7] = [
        Self::IngestAnswers,
        Self::Integration,
        Self::ReadyRetry,
        Self::ReadyDispatch,
        Self::DeferBackoff,
        Self::HardBlock,
        Self::Closure,
    ];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::IngestAnswers => "ingest answers",
            Self::Integration => "integration",
            Self::ReadyRetry => "ready_retry",
            Self::ReadyDispatch => "ready dispatch",
            Self::DeferBackoff => "defer backoff",
            Self::HardBlock => "hard block",
            Self::Closure => "run-end closure",
        }
    }

    #[must_use]
    pub const fn disposition(self) -> Disposition {
        match self {
            Self::Closure => Disposition::Performed,
            Self::Integration => Disposition::Performed,
            Self::DeferBackoff => Disposition::Performed,
            Self::ReadyDispatch => Disposition::Performed,
            Self::ReadyRetry => Disposition::Performed,
            Self::HardBlock => Disposition::Performed,
            Self::IngestAnswers => Disposition::Performed,
        }
    }

    #[must_use]
    pub const fn of(step: &Step) -> Option<Self> {
        match step {
            Step::Poisoned => None,
            Step::BudgetExceeded(_) => None,
            Step::Integrate { .. } => Some(Self::Integration),
            Step::Retry { .. } => Some(Self::ReadyRetry),
            Step::Dispatch { .. } | Step::RepairDispatch { .. } => Some(Self::ReadyDispatch),
            Step::Backoff => Some(Self::DeferBackoff),
            Step::HardBlock { .. } => Some(Self::HardBlock),
            Step::Closure(_) => Some(Self::Closure),
            Step::NotStarted | Step::Finished(_) => None,
        }
    }

    #[allow(dead_code)]
    pub fn owes(self, clause: &str) -> UpstrokeError {
        UpstrokeError::Refused {
            message: format!(
                "the schema-4 run loop's `{}` branch reached a case this build does not \
                 implement: {clause}. Nothing was appended for it",
                self.label()
            ),
        }
    }

    pub fn unimplemented(self) -> UpstrokeError {
        match self.disposition() {
            Disposition::PartlyImplemented { performs, owes } => UpstrokeError::Refused {
                message: format!(
                    "the schema-4 run loop's `{}` branch performed {performs}, and this build \
                     does not {owes}",
                    self.label()
                ),
            },
            Disposition::NotThisSlice { slice, citation } => UpstrokeError::Refused {
                message: format!(
                    "the schema-4 run loop's `{}` branch belongs to {slice}, not to this build: \
                     {citation}. No effect was performed and no event was appended",
                    self.label()
                ),
            },
            _ => UpstrokeError::Refused {
                message: format!(
                    "the schema-4 run loop selected its `{}` branch, which this build does not \
                     implement yet; no effect was performed and no event was appended",
                    self.label()
                ),
            },
        }
    }
}

pub struct RunSeams<'a> {
    pub manager: &'a WorkspaceManager,
    pub clock: &'a dyn TimeSource,
    pub sleeper: &'a dyn Sleeper,
    pub runner: &'a dyn crate::runner::Runner,
    pub adapters: &'a dyn crate::agent::AdapterSource,
    pub paths: &'a crate::rundir::RunPaths,
    pub plans: &'a dyn AttemptPlans,
    pub reviews: &'a dyn ReviewPasses,
    pub input_policy: &'a dyn ReviewInputPolicy,
    pub answers: &'a dyn crate::interaction::AnswerSource,
    pub ids: &'a dyn IdSource,
    pub halts_run: bool,
}

#[derive(Debug, Clone)]
struct RunAs {
    feedback: Vec<crate::events::Feedback>,
    attempt: AttemptNumber,
    rung: u32,
    resume_session: Option<SessionId>,
    announced: bool,
    materialized: Option<Materialization>,
}

#[derive(Debug, Clone)]
struct OpenAsked {
    question: Question,
    origin: QuestionOrigin,
    key: TaskKey,
    binding: Option<Vec<String>>,
}

fn chosen_index(options: &[String], text: &str) -> Option<u32> {
    options
        .iter()
        .position(|option| option == text)
        .and_then(|index| u32::try_from(index).ok())
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Brief {
    per_task: BTreeMap<TaskKey, Vec<crate::events::Feedback>>,
}

impl Brief {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn lines(&self, key: TaskKey) -> Vec<crate::events::Feedback> {
        self.per_task.get(&key).cloned().unwrap_or_default()
    }

    pub fn record(&mut self, key: TaskKey, record: &AttemptRecord) {
        let Some(failure) = record.failure.as_ref() else {
            return;
        };
        self.per_task
            .entry(key)
            .or_default()
            .push(crate::events::Feedback {
                attempt: record.attempt,
                tier: record.tier.clone(),
                summary: failure.reason.clone(),
                detail: failure.detail.clone(),
                human: false,
            });
    }

    #[must_use]
    pub fn replay(events: &[TopologyEvent]) -> Self {
        let mut brief = Self::new();
        for event in events {
            if let TopologyEventBody::AttemptFinished { data } = &event.body {
                brief.record(data.key, &data.record);
            }
        }
        brief
    }
}

#[derive(Debug, Clone)]
struct Retained {
    tree: String,
}

#[derive(Debug, Clone, Copy)]
struct Produced<'a> {
    capture: &'a super::attempt::Capture,
    assessed: &'a super::attempt::Assessment,
    judgement: &'a Judgement,
}

#[derive(Debug)]
pub enum Retrying {
    Started(Box<AttemptJob>),
    Closed { key: TaskKey },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Progress {
    Settled {
        key: TaskKey,
        accepted: bool,
        spent_attempt: bool,
    },
    GenerationClosed {
        key: TaskKey,
    },
    Integrated {
        key: TaskKey,
        sequence: SequenceId,
        merged_sha: CommitSha,
    },
    Rejected {
        key: TaskKey,
        sequence: SequenceId,
    },
    Unavailable {
        key: TaskKey,
        sequence: SequenceId,
        parked: bool,
    },
    Answered {
        key: TaskKey,
        question: QuestionId,
        declined: bool,
    },
    Finished {
        outcome: RunOutcome,
        closed: usize,
        report_written: bool,
        execution_root_removed: bool,
    },
    Waited {
        waited_ms: u64,
        round: u32,
    },
    BudgetExceeded,
}

pub struct TopologyRun {
    handle: RunHandle,
    identity: RunIdentity,
    broker: PermitBroker,
    warnings: Vec<String>,
    discarded: u32,
    ceiling: Ceiling,
    spend: Spend,
    deferral: Deferral,
    retained: BTreeMap<TaskKey, Retained>,
    brief: Brief,
}

impl TopologyRun {
    #[must_use]
    pub fn resumed(handle: RunHandle, inputs: FrozenInputs, ceiling: Ceiling) -> Self {
        let identity = RunIdentity {
            run_id: handle.started.run_id.clone(),
            inputs,
            committed_first_line_sha256: Some(handle.committed_first_line_sha256.clone()),
        };
        let spend = Spend::replay(&handle.events);
        let brief = Brief::replay(&handle.events);
        let broker = PermitBroker::for_run(&Entitlements::of(&handle.fold));
        Self {
            handle,
            identity,
            broker,
            warnings: Vec::new(),
            discarded: 0,
            ceiling,
            spend,
            deferral: Deferral::default_backoff(),
            retained: BTreeMap::new(),
            brief,
        }
    }

    pub fn commitment_digest(&self) -> Option<&str> {
        self.identity.committed_first_line_sha256.as_deref()
    }

    #[must_use]
    pub fn holds_entitlement(&mut self) -> bool {
        self.broker.halves().0.cancel_any()
    }

    pub fn invocations_balance(&self) -> bool {
        self.broker.invocations().balances()
    }

    pub const fn spend(&self) -> &Spend {
        &self.spend
    }

    #[must_use]
    pub fn fold(&self) -> &TopologyFold {
        &self.handle.fold
    }

    #[must_use]
    pub fn events(&self) -> &[TopologyEvent] {
        &self.handle.events
    }

    #[must_use]
    #[allow(dead_code)]
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    #[must_use]
    pub const fn discarded(&self) -> u32 {
        self.discarded
    }

    pub(super) fn record_discard(&mut self, warning: Option<String>) {
        self.discarded = self.discarded.saturating_add(1);
        self.warnings.extend(warning);
    }

    pub(super) fn warn(&mut self, warning: String) {
        self.warnings.push(warning);
    }

    #[must_use]
    pub fn entitlements_held(&self) -> u32 {
        self.broker.reservations().entitlements_held()
    }

    #[must_use]
    pub const fn reservations_cancelled(&self) -> u32 {
        self.broker.reservations().cancelled()
    }

    #[must_use]
    pub const fn reservations_peak(&self) -> usize {
        self.broker.reservations().peak()
    }

    #[must_use]
    pub fn broker_duplicates(&self) -> u32 {
        self.broker.duplicates()
    }

    #[must_use]
    #[allow(dead_code)]
    pub fn defer_round(&self) -> u32 {
        self.deferral.round()
    }

    pub fn step(
        &mut self,
        seams: &RunSeams<'_>,
        hooks: &mut dyn TopologyHooks,
    ) -> Result<Progress, UpstrokeError> {
        let _cleanup_scope = self.handle.cleanup_scope();
        if let Some(answered) = self.ingest_answers(seams, hooks)? {
            return Ok(answered);
        }
        match self.admitted()? {
            Admitted::BudgetExceeded(exceeded) => self.exceed_budget(*exceeded, seams, hooks),
            Admitted::Backoff => self.back_off(seams, hooks),
            Admitted::Integrate { candidate } => self.integrate(*candidate, seams, hooks),
            Admitted::Retry {
                key, generation, ..
            } => {
                let manager = seams.manager;
                match begin_retry(&mut self.stepping(seams, hooks), manager, key, generation)? {
                    Retrying::Started(job) => {
                        let judged = self.judge_inline(&job, seams, hooks)?;
                        settle_judged(&mut self.stepping(seams, hooks), manager, &job, &judged)
                    }
                    Retrying::Closed { key } => Ok(Progress::GenerationClosed { key }),
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
                let manager = seams.manager;
                let job = begin_dispatch(
                    &mut self.stepping(seams, hooks),
                    manager,
                    key,
                    generation,
                    continuing,
                )?;
                let judged = self.judge_inline(&job, seams, hooks)?;
                settle_judged(&mut self.stepping(seams, hooks), manager, &job, &judged)
            }
            Admitted::HardBlock { questions } => self.hard_block(&questions, seams, hooks),
            Admitted::Closure(_) => self.close_run(&closure::Cancelled::none(), seams, hooks),
        }
    }

    fn stepping<'a, 's>(
        &'a mut self,
        seams: &'a RunSeams<'s>,
        hooks: &'a mut dyn TopologyHooks,
    ) -> Stepping<'a, 's> {
        Stepping {
            run: self,
            seams,
            hooks,
        }
    }

    pub(super) fn admitted(&self) -> Result<Admitted, UpstrokeError> {
        checkpoint(select(&self.handle.fold, &self.ceiling, &self.spend))
    }

    pub(super) fn cleanup_scope(&self) -> crate::rundir::CleanupScope {
        self.handle.cleanup_scope()
    }

    pub(super) fn exceed_budget(
        &mut self,
        exceeded: crate::topology::events::BudgetExceeded4,
        seams: &RunSeams<'_>,
        hooks: &mut dyn TopologyHooks,
    ) -> Result<Progress, UpstrokeError> {
        self.emit(
            TopologyEventBody::BudgetExceeded { data: exceeded },
            seams,
            hooks,
        )?;
        Ok(Progress::BudgetExceeded)
    }

    pub(super) fn back_off(
        &mut self,
        seams: &RunSeams<'_>,
        hooks: &mut dyn TopologyHooks,
    ) -> Result<Progress, UpstrokeError> {
        let elapsed = self.deferral.wait(seams.sleeper);
        let (waited_ms, round) = (elapsed.waited_ms, elapsed.round);
        self.emit(
            TopologyEventBody::DeferWaitElapsed { data: elapsed },
            seams,
            hooks,
        )?;
        Ok(Progress::Waited { waited_ms, round })
    }

    fn first_attempt(
        &mut self,
        dispatched: &Dispatched,
        seams: &RunSeams<'_>,
        hooks: &mut dyn TopologyHooks,
    ) -> Result<AttemptJob, UpstrokeError> {
        let key = dispatched.key;
        let run_as = RunAs {
            attempt: Self::FIRST_ATTEMPT,
            rung: self.ladder_position(key)?.0,
            resume_session: None,
            feedback: self.brief.lines(key),
            announced: false,
            materialized: dispatched.materialized,
        };
        self.prepare_attempt(dispatched.site(), run_as, seams, hooks)
    }

    fn judge_inline(
        &mut self,
        job: &AttemptJob,
        seams: &RunSeams<'_>,
        hooks: &mut dyn TopologyHooks,
    ) -> Result<Judged, UpstrokeError> {
        let standing = Standing::of(&self.handle.fold, &job.identities().worker());
        let (_, invocations) = self.broker.halves();
        let ledger = std::sync::Mutex::new(invocations);
        let carried = Carried::default();
        let mut work = Work {
            manager: seams.manager,
            hooks,
            runner: seams.runner,
            standing,
            registrar: &ledger,
            carried: &carried,
            adapters: seams.adapters,
            paths: seams.paths,
            plans: seams.plans,
            reviews: seams.reviews,
            input_policy: seams.input_policy,
        };
        attempt_body(&mut work, job)
    }

    pub(super) fn ingest_answers(
        &mut self,
        seams: &RunSeams<'_>,
        hooks: &mut dyn TopologyHooks,
    ) -> Result<Option<Progress>, UpstrokeError> {
        if self.handle.fold.is_poisoned() || self.handle.fold.run_is_ending() {
            return Ok(None);
        }
        let open: Vec<QuestionId> = self
            .handle
            .fold
            .open_questions()
            .map(|questions| questions.keys().cloned().collect())
            .unwrap_or_default();
        for id in &open {
            let asked = self.open_question(id)?;
            let answer = match seams.answers.poll(&asked.question)? {
                Answer::Unanswered => continue,
                answer => answer,
            };
            let answer4 = self.answer_for(id, &asked, answer, seams)?;
            return self
                .ingest_answer(id, asked.key, answer4, seams, hooks)
                .map(Some);
        }
        Ok(None)
    }

    fn dispatch_inputs(
        &self,
        key: TaskKey,
    ) -> Result<(DispatchKind, integrate::AuthorizedHead, String), UpstrokeError> {
        let kind = self.dispatch_kind(key)?;
        let authorized = integrate::authorized_head(&self.handle.started, &self.handle.events);
        let refname = self.handle.started.integration_ref.as_str().to_owned();
        Ok((kind, authorized, refname))
    }

    fn reserve_dispatch(&mut self, key: TaskKey) -> Result<(), UpstrokeError> {
        self.broker.reserve(
            &Entitlements::of(&self.handle.fold),
            key,
            ReservationKind::Dispatch,
        )
    }

    fn dispatch_settled(
        &mut self,
        key: TaskKey,
        dispatched: Result<Dispatched, EmitFailure>,
    ) -> Result<Dispatched, UpstrokeError> {
        let (reservations, invocations) = self.broker.halves();
        match dispatched {
            Ok(dispatched) => {
                reservations.convert(key, ReservationKind::Dispatch)?;
                self.deferral.progressed();
                Ok(dispatched)
            }
            Err(error) => {
                let cancelled = reservations.cancel(key, ReservationKind::Dispatch);
                Err(error.discharging(invocations).with_cleanup(cancelled))
            }
        }
    }

    fn integrate(
        &mut self,
        candidate: CandidateRef,
        seams: &RunSeams<'_>,
        hooks: &mut dyn TopologyHooks,
    ) -> Result<Progress, UpstrokeError> {
        let request = self.integration_request(&candidate)?;
        let key = candidate.key;
        self.reserve_integration(key)?;

        let terminal = {
            let (reservations, invocations) = self.broker.halves();
            let mut cx = IntegrationCx {
                emitter: RunEmitter {
                    identity: &self.identity,
                    state: EmitState {
                        fold: &mut self.handle.fold,
                        log: &mut self.handle.log,
                        events: &mut self.handle.events,
                        reservations,
                        warnings: &mut self.warnings,
                    },
                    clock: seams.clock,
                },
                hooks,
                invocations,
                spend: &mut self.spend,
                seams,
            };
            integrate::integrate(&mut cx, seams.manager, &request)
        };
        self.integration_settled(key, terminal)
    }

    pub(super) fn integration_request(
        &self,
        candidate: &CandidateRef,
    ) -> Result<IntegrationRequest, UpstrokeError> {
        IntegrationRequest::from_log(&self.handle.fold, &self.handle.events, candidate)
    }

    pub(super) fn reserve_integration(&mut self, key: TaskKey) -> Result<(), UpstrokeError> {
        self.broker.reserve(
            &Entitlements::of(&self.handle.fold),
            key,
            ReservationKind::Integration,
        )
    }

    pub(super) fn convert_integration(&mut self, key: TaskKey) -> Result<(), UpstrokeError> {
        self.broker.convert(key, ReservationKind::Integration)
    }

    pub(super) fn integration_settled(
        &mut self,
        key: TaskKey,
        terminal: Result<Terminal, UpstrokeError>,
    ) -> Result<Progress, UpstrokeError> {
        match terminal {
            Ok(terminal) => {
                self.deferral.progressed();
                Ok(match terminal {
                    Terminal::Merged(published) => Progress::Integrated {
                        key: published.key,
                        sequence: published.sequence,
                        merged_sha: published.merged_sha,
                    },
                    Terminal::Rejected { sequence, key } => Progress::Rejected { key, sequence },
                    Terminal::Unavailable {
                        sequence,
                        key,
                        parked,
                    } => Progress::Unavailable {
                        key,
                        sequence,
                        parked,
                    },
                })
            }
            Err(error) => {
                if self.broker.reservations().held(key) == Some(ReservationKind::Integration) {
                    self.broker
                        .cancel_reservation(key, ReservationKind::Integration)?;
                }
                Err(error)
            }
        }
    }

    pub(super) fn verification_job(
        &self,
        request: &VerifyRequest<'_>,
    ) -> Result<VerificationJob, UpstrokeError> {
        VerificationJob::of(&self.handle.fold, request)
    }

    pub(super) fn charge_reviews(&mut self, key: TaskKey, reviews: &[crate::events::ReviewRecord]) {
        self.spend.record_reviews(key, reviews);
    }

    pub(super) fn broker_mut(&mut self) -> &mut PermitBroker {
        &mut self.broker
    }

    pub(super) fn cancel_provisional(&mut self) -> bool {
        self.broker.halves().0.cancel_any()
    }

    pub(super) fn limit_slots(&mut self, limits: SlotLimits) -> Result<(), UpstrokeError> {
        if !self.broker.balances() {
            return Err(UpstrokeError::Refused {
                message: "the run's broker holds an unsettled reservation or invocation, so its \
                          slot limits cannot be replaced; nothing was spawned"
                    .to_owned(),
            });
        }
        self.broker = PermitBroker::for_pipelines(limits);
        Ok(())
    }

    pub(super) fn hard_block(
        &mut self,
        questions: &[QuestionId],
        seams: &RunSeams<'_>,
        hooks: &mut dyn TopologyHooks,
    ) -> Result<Progress, UpstrokeError> {
        for id in questions {
            let asked = self.open_question(id)?;
            let answer = match seams.answers.resolve(&asked.question)? {
                Answer::Unanswered => continue,
                answer => answer,
            };
            let answer4 = self.answer_for(id, &asked, answer, seams)?;
            return self.ingest_answer(id, asked.key, answer4, seams, hooks);
        }
        self.close_run(&closure::Cancelled::none(), seams, hooks)
    }

    fn answer_for(
        &self,
        id: &QuestionId,
        asked: &OpenAsked,
        answer: Answer,
        seams: &RunSeams<'_>,
    ) -> Result<Answer4, UpstrokeError> {
        let text = match answer {
            Answer::Declined => {
                return Ok(Answer4::Declined {
                    decline_halts_run: seams.halts_run,
                });
            }
            Answer::Answered { text } => text,
            Answer::Unanswered => {
                return Err(UpstrokeError::Refused {
                    message: format!("question {} resolved to no answer to ingest", id.0),
                });
            }
        };
        let (Some(authorized), QuestionOrigin::Admission) = (&asked.binding, asked.origin) else {
            return Ok(Answer4::Answered {
                option_index: chosen_index(&asked.question.options, &text).unwrap_or(0),
                binding_override: None,
            });
        };
        let option_index =
            chosen_index(&asked.question.options, &text).ok_or_else(|| UpstrokeError::Refused {
                message: format!(
                    "question {} asks which agent runs task {}, and `{text}` is none of the {} \
                     it offers; a one-off binding activates only through an option the spawn \
                     froze, so nothing was appended",
                    id.0,
                    asked.key.index(),
                    asked.question.options.len()
                ),
            })?;
        let agent = authorized
            .get(usize::try_from(option_index).unwrap_or(usize::MAX))
            .ok_or_else(|| UpstrokeError::Refused {
                message: format!(
                    "question {} offers {} option(s) and authorizes {} binding(s); option \
                     {option_index} exists in one and not the other, so nothing was appended",
                    id.0,
                    asked.question.options.len(),
                    authorized.len()
                ),
            })?;
        let entry = self
            .handle
            .fold
            .registry()
            .and_then(|registry| registry.get(asked.key))
            .ok_or_else(|| UpstrokeError::Refused {
                message: format!(
                    "task {} is not in this run's frozen registry",
                    asked.key.index()
                ),
            })?;
        Ok(Answer4::Answered {
            option_index,
            binding_override: Some(super::repair::one_off_binding(
                entry,
                asked.key,
                id,
                option_index,
                agent,
            )?),
        })
    }

    fn ingest_answer(
        &mut self,
        id: &QuestionId,
        key: TaskKey,
        answer4: Answer4,
        seams: &RunSeams<'_>,
        hooks: &mut dyn TopologyHooks,
    ) -> Result<Progress, UpstrokeError> {
        let declined = matches!(answer4, Answer4::Declined { .. });
        self.emit(
            TopologyEventBody::QuestionAnswered {
                data: crate::topology::events::QuestionAnswered4 {
                    key,
                    question: id.clone(),
                    answer: answer4,
                    via: seams.answers.id().to_owned(),
                },
            },
            seams,
            hooks,
        )?;
        Ok(Progress::Answered {
            key,
            question: id.clone(),
            declined,
        })
    }

    fn open_question(&self, id: &QuestionId) -> Result<OpenAsked, UpstrokeError> {
        let open = self
            .handle
            .fold
            .open_questions()
            .and_then(|open| open.get(id))
            .ok_or_else(|| UpstrokeError::Refused {
                message: format!("question {} is not open in this run's fold", id.0),
            })?;
        let frozen = &open.question;
        Ok(OpenAsked {
            question: Question {
                id: frozen.id.clone(),
                kind: frozen.kind,
                affected_tasks: vec![crate::ir::TaskId(self.display_id(frozen.key)?)],
                context: frozen.context.clone(),
                options: frozen.options.clone(),
            },
            origin: open.origin,
            key: frozen.key,
            binding: open.binding.clone(),
        })
    }

    fn open_to_continue(
        &self,
        key: TaskKey,
        generation: GenerationId,
    ) -> Result<(OpenGeneration, DispatchKind), UpstrokeError> {
        let base = self
            .handle
            .fold
            .task(key)
            .and_then(|task| task.generations.iter().find(|held| held.id == generation))
            .map(|held| held.base_sha.clone())
            .ok_or_else(|| UpstrokeError::Refused {
                message: format!(
                    "task {} has no generation {} to continue",
                    key.index(),
                    generation.0
                ),
            })?;
        let kind = match self.dispatch_kind(key)? {
            DispatchKind::Ordinary { paths } => DispatchKind::Ordinary { paths },
            DispatchKind::Repair { root, .. } => DispatchKind::Repair {
                root,
                source: dispatched_source(&self.handle.events, key, generation)?,
            },
        };
        let open = OpenGeneration {
            key,
            generation,
            base,
            slot: task_slot(key, generation),
            source: match &kind {
                DispatchKind::Ordinary { .. } => None,
                DispatchKind::Repair { source, .. } => Some(source.clone()),
            },
        };
        Ok((open, kind))
    }

    fn retry_request(
        &self,
        key: TaskKey,
        generation: GenerationId,
        seams: &RunSeams<'_>,
    ) -> Result<RetryRequest, UpstrokeError> {
        let position = self.ladder_position(key)?;
        let held = self
            .retained
            .get(&key)
            .cloned()
            .ok_or_else(|| UpstrokeError::Refused {
                message: format!(
                    "task {} has a retained generation this process did not retain, so the tree \
                     its retry must re-gate is not known here. A fresh process closes a retained \
                     generation in recovery rather than continuing it",
                    key.index()
                ),
            })?;
        let binding = self
            .handle
            .fold
            .rung_binding(key, position.0)
            .ok_or_else(|| UpstrokeError::Refused {
                message: format!(
                    "task {} has neither a validated one-off binding nor a rung {} in its \
                     frozen ladder",
                    key.index(),
                    position.0
                ),
            })?;
        let pool = seams.plans.pool_for(&binding.agent);
        let materialization = self.retry_materialization(key);
        Ok(RetryRequest {
            key,
            slot: task_slot(key, generation),
            retained_tree: held.tree,
            binding,
            rung: position.0,
            pool,
            materialization,
        })
    }

    fn retry_started(
        &mut self,
        generation: GenerationId,
        started: crate::topology::events::AttemptStarted4,
        slot_for_run: &crate::workspace_manager::Slot,
        seams: &RunSeams<'_>,
        hooks: &mut dyn TopologyHooks,
    ) -> Result<AttemptJob, UpstrokeError> {
        let key = started.key;
        let run_as = RunAs {
            attempt: started.attempt,
            rung: started.rung,
            resume_session: started.resume_session.clone(),
            feedback: self.brief.lines(key),
            announced: true,
            materialized: started.materialization_observed,
        };
        self.emit(
            TopologyEventBody::AttemptStarted { data: started },
            seams,
            hooks,
        )?;
        self.broker.convert(key, ReservationKind::Retry)?;
        self.deferral.progressed();

        let base = self
            .handle
            .fold
            .task(key)
            .and_then(|task| task.generations.iter().find(|held| held.id == generation))
            .map(|held| held.base_sha.clone())
            .ok_or_else(|| UpstrokeError::Refused {
                message: format!(
                    "generation {} of task {} left the fold mid-retry",
                    generation.0,
                    key.index()
                ),
            })?;
        let worktree = seams.manager.slot_path(slot_for_run);
        let site = AttemptSite {
            key,
            generation,
            base: &base,
            slot: slot_for_run,
            worktree: &worktree,
        };
        self.prepare_attempt(site, run_as, seams, hooks)
    }

    fn retry_closed(
        &mut self,
        key: TaskKey,
        closed: crate::topology::events::GenerationClosed,
        seams: &RunSeams<'_>,
        hooks: &mut dyn TopologyHooks,
    ) -> Result<(), UpstrokeError> {
        self.emit(
            TopologyEventBody::GenerationClosed { data: closed },
            seams,
            hooks,
        )?;
        self.retained.remove(&key);
        Ok(())
    }

    pub(super) fn close_run(
        &mut self,
        cancelled: &closure::Cancelled,
        seams: &RunSeams<'_>,
        hooks: &mut dyn TopologyHooks,
    ) -> Result<Progress, UpstrokeError> {
        let outcome = closure::ending_outcome(&self.handle.fold)?;
        for interrupted in closure::settleable(&self.handle.fold, &outcome, cancelled)? {
            self.emit(interrupted.interrupted(), seams, hooks)?;
            self.reclaim_interrupted(&interrupted, seams, hooks)?;
        }
        self.complete_promotions(seams, hooks)?;
        self.complete_publication(seams, hooks)?;

        let reason = GenerationCloseReason::RunEnding {
            outcome: outcome.clone(),
        };
        let mut closed = 0;
        for key in closure::closable(&self.handle.fold) {
            let event = close_generation(&self.handle.fold, key, reason.clone())?;
            let slot = task_slot(key, event.generation);
            self.emit(
                TopologyEventBody::GenerationClosed { data: event },
                seams,
                hooks,
            )?;
            self.retained.remove(&key);
            super::dispatch::scrub(seams.manager, hooks, &slot)?;
            closed += 1;
        }

        if self.broker.halves().0.cancel_any() {
            self.warnings.push(
                "run-end closure found a provisional reservation still held and cancelled it"
                    .to_owned(),
            );
        }

        closure::confirm_derived(&self.handle.fold, &outcome)?;
        let finished = closure::run_finished(&self.handle.fold, outcome.clone());
        self.emit(
            TopologyEventBody::RunFinished { data: finished },
            seams,
            hooks,
        )?;

        let finalized = finalize::finalize(
            &finalize::Finalize {
                manager: seams.manager,
                public: &seams.paths.public,
                private: &seams.paths.private,
                run_id: &self.identity.run_id,
                fold: &self.handle.fold,
                events: &self.handle.events,
            },
            hooks,
        )?;
        Ok(Progress::Finished {
            outcome,
            closed,
            report_written: finalized.report_written,
            execution_root_removed: finalized.execution_root_removed,
        })
    }

    fn reclaim_interrupted(
        &mut self,
        interrupted: &closure::InFlight,
        seams: &RunSeams<'_>,
        hooks: &mut dyn TopologyHooks,
    ) -> Result<(), UpstrokeError> {
        match interrupted {
            closure::InFlight::Attempt {
                key,
                generation,
                attempt,
                ..
            } => {
                self.retained.remove(key);
                reclaim_snapshots_of(
                    seams.manager,
                    hooks,
                    JudgeNames::Attempt {
                        key: key.0,
                        generation: generation.0,
                        attempt: attempt.0,
                    },
                )?;
                super::dispatch::scrub(seams.manager, hooks, &task_slot(*key, *generation))
            }
            closure::InFlight::Verification { sequence, pin, .. } => {
                if let Some((pin, proposed)) = pin {
                    integrate::prune_pin(hooks, seams.manager, pin, proposed)?;
                }
                let staging = integrate::staging_slot(*sequence);
                seams.manager.remove_worktree(hooks.effects(), &staging)?;
                seams.manager.remove_intent(hooks.effects(), &staging)?;
                reclaim_snapshots_of(
                    seams.manager,
                    hooks,
                    JudgeNames::Integration {
                        sequence: u64::from(sequence.0),
                    },
                )
            }
        }
    }

    fn complete_promotions(
        &mut self,
        seams: &RunSeams<'_>,
        hooks: &mut dyn TopologyHooks,
    ) -> Result<(), UpstrokeError> {
        let run_id = self.identity.run_id.clone();
        for key in closure::promoting(&self.handle.fold) {
            let Some(promoting) =
                super::candidate::recovery_for(seams.manager, &run_id, &self.handle.fold, key)?
                    .promotion
            else {
                continue;
            };
            let slot = task_slot(key, promoting.candidate().generation);
            let referenced = create_candidates_ref(seams.manager, hooks, promoting)?;
            let created = self.with_journal(seams, hooks, |journal| {
                append_candidate_created(journal, referenced)
            })?;
            reclaim_after_creation(seams.manager, hooks, &slot, created)?;
        }
        Ok(())
    }

    fn complete_publication(
        &mut self,
        seams: &RunSeams<'_>,
        hooks: &mut dyn TopologyHooks,
    ) -> Result<(), UpstrokeError> {
        let Some(authorized) = integrate::Authorized::from_fold(&self.handle.fold)? else {
            return Ok(());
        };
        self.with_journal(seams, hooks, |journal| {
            integrate::publish(journal, seams.manager, authorized)
        })?;
        Ok(())
    }

    fn retry_materialization(&self, key: TaskKey) -> Option<Materialization> {
        self.handle
            .fold
            .registry()
            .and_then(|registry| registry.get(key))
            .and_then(|entry| entry.lineage)
            .map(|_| Materialization::Retained)
    }

    const FIRST_ATTEMPT: crate::topology::events::AttemptNumber =
        crate::topology::events::AttemptNumber(1);

    fn prepare_attempt(
        &mut self,
        site: AttemptSite<'_>,
        run_as: RunAs,
        seams: &RunSeams<'_>,
        hooks: &mut dyn TopologyHooks,
    ) -> Result<AttemptJob, UpstrokeError> {
        let key = site.key;
        let binding = self
            .handle
            .fold
            .rung_binding(key, run_as.rung)
            .ok_or_else(|| UpstrokeError::Refused {
                message: format!(
                    "task {} has neither a validated one-off binding nor a rung {} in its \
                     frozen ladder, so there is no binding to run it under",
                    key.index(),
                    run_as.rung
                ),
            })?;

        let entry = self
            .handle
            .fold
            .registry()
            .and_then(|registry| registry.get(key))
            .ok_or_else(|| UpstrokeError::Refused {
                message: format!("task {} is not in this run's frozen registry", key.index()),
            })?
            .clone();

        let plan = seams.plans.plan(&PlanRequest {
            key,
            entry: &entry,
            attempt: run_as.attempt,
            rung: run_as.rung,
            binding,
            workspace: site.worktree,
            resume_session: run_as.resume_session.clone(),
            feedback: run_as.feedback.clone(),
            materialization_observed: run_as.materialized,
        })?;

        if !run_as.announced {
            let (reservations, invocations) = self.broker.halves();
            let mut emitter = RunEmitter {
                identity: &self.identity,
                state: EmitState {
                    fold: &mut self.handle.fold,
                    log: &mut self.handle.log,
                    events: &mut self.handle.events,
                    reservations,
                    warnings: &mut self.warnings,
                },
                clock: seams.clock,
            };
            AttemptContext {
                manager: seams.manager,
                hooks,
                emitter: &mut emitter,
                runner: seams.runner,
                ledger: invocations,
                adapters: seams.adapters,
                paths: seams.paths,
                reviews: seams.reviews,
                input_policy: seams.input_policy,
            }
            .announce(site, &plan)?;
        }
        Ok(AttemptJob {
            key,
            generation: site.generation,
            base: site.base.clone(),
            slot: site.slot.clone(),
            worktree: site.worktree.to_path_buf(),
            plan,
            entry,
        })
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "the run's side of a failed settlement takes the attempt's site, plan, \
                  products, record and failure, and the run's seams and hooks; the closed \
                  slot's scrub stays with the caller, which holds the operator"
    )]
    fn settle_failure(
        &mut self,
        site: AttemptSite<'_>,
        plan: &super::attempt::AttemptPlan,
        produced: Produced<'_>,
        record: AttemptRecord,
        failure: &crate::ladder::AttemptFailure,
        seams: &RunSeams<'_>,
        hooks: &mut dyn TopologyHooks,
    ) -> Result<(bool, bool), UpstrokeError> {
        let Produced {
            capture, assessed, ..
        } = produced;
        let policy = self.ladder_policy(site.key)?;

        let defers = self.deferrals_recorded(site.key)?;
        let position = self.ladder_position(site.key)?;

        let attempts_on_rung =
            position
                .1
                .saturating_add(u32::from(crate::ladder::spends_allowance(Some(
                    crate::ladder::FailureShape::of(failure),
                ))));

        let next = crate::ladder::next_step(
            failure,
            &crate::ladder::LadderState {
                rung: position.0 as usize,
                attempts_on_rung,
                defers,
                resumable: plan.session_resume
                    && assessed.outcome.session_id.is_some()
                    && capture.unresolved.is_empty(),
            },
            &policy,
        );
        let question = match next {
            crate::ladder::Next::AskHuman(kind) => {
                Some(self.park_question(site.key, attempts_on_rung, kind, failure, seams.ids)?)
            }
            _ => None,
        };

        let settled = settle_failed(
            &self.handle.fold,
            &FinishedAttempt {
                key: site.key,
                generation: site.generation,
                attempt: plan.attempt,
                record,
                next,
                session: assessed.outcome.session_id.clone().map(SessionId),
                question,
                halts_run: seams.halts_run,
                defers,
                reason: failure.reason.clone(),
                rung: position.0.saturating_add(1),
            },
        )?;

        if matches!(
            settled.event.settlement,
            crate::topology::events::AttemptSettlement::Retained { .. }
        ) {
            self.retained.insert(
                site.key,
                Retained {
                    tree: capture.tree.clone(),
                },
            );
        }
        self.spend.record(site.key, &settled.event.record);
        self.brief.record(site.key, &settled.event.record);
        let closed = matches!(
            settled.event.settlement,
            crate::topology::events::AttemptSettlement::Closed { .. }
        );
        self.emit(
            TopologyEventBody::AttemptFinished {
                data: Box::new(settled.event),
            },
            seams,
            hooks,
        )?;
        Ok((closed, settled.spent_attempt))
    }

    fn judged_tree(
        &self,
        site: AttemptSite<'_>,
        plan: &super::attempt::AttemptPlan,
        capture: &super::attempt::Capture,
        record: &AttemptRecord,
        actual_paths: crate::topology::paths::PathSet,
    ) -> Result<(JudgedTree, String), UpstrokeError> {
        let key = site.key;
        let judged = JudgedTree {
            key,
            generation: site.generation,
            attempt: Box::new(record.clone()),
            base_sha: site.base.clone(),
            tree_sha: CommitSha(capture.tree.clone()),
            message: format!(
                "upstroke: {} attempt {}",
                self.display_id(key)?,
                plan.attempt.0
            ),
            actual_paths: actual_paths.clone(),
            lease_effect: match self.lineage_root(key) {
                Some(root) => CandidateLeaseEffect::WidensLineage {
                    root,
                    paths: actual_paths,
                },
                None => CandidateLeaseEffect::ReplacesPredicted {
                    paths: actual_paths,
                },
            },
        };
        Ok((judged, self.identity.run_id.clone()))
    }

    fn record_promoted(&mut self, key: TaskKey, record: &AttemptRecord) {
        self.spend.record(key, record);
        self.brief.record(key, record);
    }

    fn with_journal<T>(
        &mut self,
        seams: &RunSeams<'_>,
        hooks: &mut dyn TopologyHooks,
        run: impl FnOnce(&mut RunJournal<'_, '_>) -> Result<T, UpstrokeError>,
    ) -> Result<T, UpstrokeError> {
        let (reservations, invocations) = self.broker.halves();
        let mut journal = RunJournal {
            emitter: RunEmitter {
                identity: &self.identity,
                state: EmitState {
                    fold: &mut self.handle.fold,
                    log: &mut self.handle.log,
                    events: &mut self.handle.events,
                    reservations,
                    warnings: &mut self.warnings,
                },
                clock: seams.clock,
            },
            hooks,
            invocations,
        };
        run(&mut journal)
    }

    fn display_id(&self, key: TaskKey) -> Result<String, UpstrokeError> {
        Ok(self
            .handle
            .fold
            .registry()
            .and_then(|registry| registry.get(key))
            .ok_or_else(|| UpstrokeError::Refused {
                message: format!("task {} is not in this run's frozen registry", key.index()),
            })?
            .display_id
            .as_str()
            .to_owned())
    }

    fn park_question(
        &self,
        key: TaskKey,
        attempts_on_rung: u32,
        kind: crate::ir::QuestionKind,
        failure: &crate::ladder::AttemptFailure,
        ids: &dyn IdSource,
    ) -> Result<FrozenQuestion, UpstrokeError> {
        let entry = self
            .handle
            .fold
            .registry()
            .and_then(|registry| registry.get(key))
            .ok_or_else(|| UpstrokeError::Refused {
                message: format!("task {} is not in this run's frozen registry", key.index()),
            })?;
        let task = self
            .handle
            .fold
            .task(key)
            .ok_or_else(|| UpstrokeError::Refused {
                message: format!("task {} is not in this run's fold", key.index()),
            })?;
        let total_attempts: u32 = task
            .generations
            .iter()
            .map(|generation| generation.attempts)
            .sum();
        let rungs_spent = (task.rung as usize).saturating_add(1).max(1);
        let _ = attempts_on_rung;
        Ok(FrozenQuestion {
            id: ids.question_id(),
            key,
            kind,
            context: crate::engine::coordinator::question_context(
                crate::engine::coordinator::ParkSubject {
                    display_id: entry.display_id.as_str(),
                    title: &entry.spec.title,
                    acceptance: &entry.spec.acceptance,
                    attempts: total_attempts,
                    rungs_spent,
                },
                kind,
                failure,
            ),
            options: crate::engine::coordinator::topology_question_options(kind),
        })
    }

    fn ladder_position(&self, key: TaskKey) -> Result<(u32, u32), UpstrokeError> {
        let task = self
            .handle
            .fold
            .task(key)
            .ok_or_else(|| UpstrokeError::Refused {
                message: format!("task {} is not in this run's fold", key.index()),
            })?;
        Ok((task.rung, task.attempts_on_rung))
    }

    fn deferrals_recorded(&self, key: TaskKey) -> Result<u32, UpstrokeError> {
        Ok(self
            .handle
            .fold
            .task(key)
            .ok_or_else(|| UpstrokeError::Refused {
                message: format!("task {} is not in this run's fold", key.index()),
            })?
            .defers)
    }

    fn ladder_policy(&self, key: TaskKey) -> Result<crate::ladder::LadderPolicy, UpstrokeError> {
        let entry = self
            .handle
            .fold
            .registry()
            .and_then(|registry| registry.get(key))
            .ok_or_else(|| UpstrokeError::Refused {
                message: format!("task {} is not in this run's frozen registry", key.index()),
            })?;
        let limits = self
            .handle
            .fold
            .started()
            .ok_or_else(|| UpstrokeError::Refused {
                message: "the run has not started".to_owned(),
            })?
            .limits;
        Ok(crate::ladder::LadderPolicy {
            attempts_per: entry.ladder.attempts_per,
            rungs: if self.handle.fold.binding_override(key).is_some() {
                1
            } else {
                entry.ladder.rungs.len()
            },
            max_defers: limits.max_defers,
        })
    }

    fn lineage_root(&self, key: TaskKey) -> Option<TaskKey> {
        self.handle
            .fold
            .registry()
            .and_then(|registry| registry.get(key))
            .and_then(|entry| entry.lineage)
            .map(|lineage| lineage.root)
    }

    fn dispatch_kind(&self, key: TaskKey) -> Result<DispatchKind, UpstrokeError> {
        let entry = self
            .handle
            .fold
            .registry()
            .and_then(|registry| registry.get(key))
            .ok_or_else(|| UpstrokeError::Refused {
                message: format!(
                    "the fold selected task {} for dispatch and the frozen registry has no such \
                     entry; the two disagree and nothing is dispatched",
                    key.0
                ),
            })?;
        let Some(lineage) = entry.lineage else {
            let paths =
                self.handle
                    .fold
                    .predicted_region(key)
                    .ok_or_else(|| UpstrokeError::Refused {
                        message: format!("task {} has no predicted region", key.index()),
                    })?;
            return Ok(DispatchKind::Ordinary { paths });
        };
        Ok(DispatchKind::Repair {
            root: lineage.root,
            source: rejected_source(&self.handle.events, key)?,
        })
    }

    pub(super) fn emit(
        &mut self,
        body: TopologyEventBody,
        seams: &RunSeams<'_>,
        hooks: &mut dyn TopologyHooks,
    ) -> Result<(), UpstrokeError> {
        self.emit_undischarged(body, seams, hooks)
            .map_err(|failure| failure.discharging(self.broker.halves().1))
    }

    fn emit_undischarged(
        &mut self,
        body: TopologyEventBody,
        seams: &RunSeams<'_>,
        hooks: &mut dyn TopologyHooks,
    ) -> Result<(), EmitFailure> {
        let mut emitter = RunEmitter {
            identity: &self.identity,
            state: EmitState {
                fold: &mut self.handle.fold,
                log: &mut self.handle.log,
                events: &mut self.handle.events,
                reservations: self.broker.halves().0,
                warnings: &mut self.warnings,
            },
            clock: seams.clock,
        };
        emitter.emit(body, hooks)
    }
}

fn reclaim_snapshots_of(
    manager: &WorkspaceManager,
    hooks: &mut dyn TopologyHooks,
    names: JudgeNames,
) -> Result<(), UpstrokeError> {
    for slot in manager.intents()? {
        let crate::workspace_manager::Slot::Snapshot { name } = &slot else {
            continue;
        };
        if names.owns(name) {
            manager.remove_worktree(hooks.effects(), &slot)?;
            manager.remove_intent(hooks.effects(), &slot)?;
        }
    }
    Ok(())
}

fn rejected_source(events: &[TopologyEvent], key: TaskKey) -> Result<CandidateRef, UpstrokeError> {
    events
        .iter()
        .rev()
        .find_map(|event| match &event.body {
            TopologyEventBody::MergeRejected { data } if data.repair.key == key => {
                Some(data.candidate.clone())
            }
            _ => None,
        })
        .ok_or_else(|| UpstrokeError::Refused {
            message: format!(
                "task {} is a repair and no `merge_rejected` in this log registered it, so the \
                 candidate it is materialized from is not known; nothing is dispatched",
                key.index()
            ),
        })
}

pub(super) fn dispatched_source(
    events: &[TopologyEvent],
    key: TaskKey,
    generation: GenerationId,
) -> Result<CandidateRef, UpstrokeError> {
    events
        .iter()
        .rev()
        .find_map(|event| match &event.body {
            TopologyEventBody::TaskDispatched { data }
                if data.key == key && data.generation == generation =>
            {
                Some(data.source_candidate.clone())
            }
            _ => None,
        })
        .ok_or_else(|| UpstrokeError::Refused {
            message: format!(
                "task {} generation {} has no `task_dispatched` in this log, so there is no \
                 record of what it was materialized from; nothing is continued",
                key.index(),
                generation.0
            ),
        })?
        .ok_or_else(|| UpstrokeError::Refused {
            message: format!(
                "task {} generation {} is a repair whose `task_dispatched` names no source \
                 candidate; the log disagrees with the registry and nothing is continued",
                key.index(),
                generation.0
            ),
        })
}

#[cfg(test)]
mod tests;
