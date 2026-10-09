//! Extended notes: `docs/internals/engine/topology/select.md`

use std::collections::BTreeMap;

use crate::error::UpstrokeError;
use crate::events::RunOutcome;
use crate::events::{AttemptRecord, BudgetKind};
use crate::ir::QuestionId;
use crate::runner::InvocationId;
use crate::topology::events::{
    AttemptNumber, BudgetExceeded4, CandidateRef, DerivedOutcome, Epoch, GenerationId, SequenceId,
    TopologyEvent, TopologyEventBody,
};
use crate::topology::fold::{GenerationClass, TopologyFold};
use crate::topology::registry::TaskKey;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Spend {
    run: f64,
    per_task: BTreeMap<TaskKey, f64>,
}

impl Spend {
    pub const fn run_total(&self) -> f64 {
        self.run
    }

    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, key: TaskKey, record: &AttemptRecord) {
        let attempt = record.cost_usd.unwrap_or(0.0) + record.review_cost_usd().unwrap_or(0.0);
        self.run += attempt;
        *self.per_task.entry(key).or_insert(0.0) += attempt;
    }

    pub fn record_reviews(&mut self, key: TaskKey, reviews: &[crate::events::ReviewRecord]) {
        for review in reviews {
            self.record_review_cost(key, review.cost_usd);
        }
    }

    pub fn record_review_cost(&mut self, key: TaskKey, cost_usd: Option<f64>) {
        let cost = cost_usd.unwrap_or(0.0);
        self.run += cost;
        *self.per_task.entry(key).or_insert(0.0) += cost;
    }

    fn record_unattributed_reviews(&mut self, reviews: &[crate::events::ReviewRecord]) {
        for review in reviews {
            self.run += review.cost_usd.unwrap_or(0.0);
        }
    }

    #[must_use]
    pub fn replay(events: &[TopologyEvent]) -> Self {
        let mut spend = Self::new();
        let mut verifying: Option<(SequenceId, TaskKey)> = None;
        for event in events {
            match &event.body {
                TopologyEventBody::AttemptFinished { data } => spend.record(data.key, &data.record),
                TopologyEventBody::CandidatePrepared { data } => {
                    spend.record(data.key, &data.attempt);
                }
                TopologyEventBody::MergeVerificationStarted { data } => {
                    verifying = Some((data.sequence, data.candidate.key));
                }
                TopologyEventBody::MergeVerificationUnavailable { data } => {
                    match verifying
                        .take()
                        .filter(|(sequence, _)| *sequence == data.sequence)
                    {
                        Some((_, key)) => spend.record_reviews(key, &data.reviews),
                        None => spend.record_unattributed_reviews(&data.reviews),
                    }
                }
                TopologyEventBody::MergeVerificationCharged { data } => {
                    match verifying
                        .as_ref()
                        .filter(|(sequence, _)| *sequence == data.sequence)
                    {
                        Some((_, key)) => spend.record_reviews(*key, &data.reviews),
                        None => spend.record_unattributed_reviews(&data.reviews),
                    }
                }
                TopologyEventBody::MergePrepared { data } => {
                    if let Some(verification) = &data.verification {
                        spend.record_reviews(data.key, &verification.reviews);
                    }
                }
                TopologyEventBody::MergeRejected { data } => {
                    if let crate::topology::events::RejectionDisposition::CodeRejected {
                        verification,
                    } = &data.disposition
                    {
                        spend.record_reviews(data.candidate.key, &verification.reviews);
                    }
                }
                _ => {}
            }
        }
        spend
    }

    #[must_use]
    pub fn run_usd(&self) -> f64 {
        self.run
    }

    #[must_use]
    pub fn task_usd(&self, key: TaskKey) -> f64 {
        self.per_task.get(&key).copied().unwrap_or(0.0)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Ceiling {
    pub run_usd: Option<f64>,
    pub task_usd: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Breach {
    pub budget: BudgetKind,
    pub limit_usd: f64,
    pub spent_usd: f64,
}

impl Ceiling {
    #[must_use]
    pub const fn unlimited() -> Self {
        Self {
            run_usd: None,
            task_usd: None,
        }
    }

    #[must_use]
    pub fn breach(&self, spend: &Spend, key: TaskKey) -> Option<Breach> {
        self.run_breach(spend)
            .or_else(|| self.task_breach(spend, key))
    }

    fn run_breach(&self, spend: &Spend) -> Option<Breach> {
        let limit = self.run_usd?;
        let spent = spend.run_usd();
        (spent >= limit).then_some(Breach {
            budget: BudgetKind::Run,
            limit_usd: limit,
            spent_usd: spent,
        })
    }

    fn task_breach(&self, spend: &Spend, key: TaskKey) -> Option<Breach> {
        let limit = self.task_usd?;
        let spent = spend.task_usd(key);
        (spent >= limit).then_some(Breach {
            budget: BudgetKind::Task,
            limit_usd: limit,
            spent_usd: spent,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    Poisoned,
    BudgetExceeded(Box<BudgetExceeded4>),
    Integrate {
        candidate: Box<CandidateRef>,
    },
    Retry {
        key: TaskKey,
        generation: GenerationId,
        attempt: AttemptNumber,
    },
    Dispatch {
        key: TaskKey,
        generation: GenerationId,
        continuing: bool,
    },
    RepairDispatch {
        key: TaskKey,
        generation: GenerationId,
        continuing: bool,
    },
    Backoff,
    HardBlock {
        questions: Vec<QuestionId>,
    },
    Closure(DerivedOutcome),
    NotStarted,
    Finished(RunOutcome),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Admitted {
    BudgetExceeded(Box<BudgetExceeded4>),
    Integrate {
        candidate: Box<CandidateRef>,
    },
    Retry {
        key: TaskKey,
        generation: GenerationId,
        attempt: AttemptNumber,
    },
    Dispatch {
        key: TaskKey,
        generation: GenerationId,
        continuing: bool,
    },
    RepairDispatch {
        key: TaskKey,
        generation: GenerationId,
        continuing: bool,
    },
    Backoff,
    HardBlock {
        questions: Vec<QuestionId>,
    },
    Closure(DerivedOutcome),
}

#[must_use]
pub fn select(fold: &TopologyFold, ceiling: &Ceiling, spend: &Spend) -> Step {
    if fold.is_poisoned() {
        return Step::Poisoned;
    }
    if let Some(outcome) = fold.finished() {
        return Step::Finished(outcome.clone());
    }
    let Some(epoch) = fold.epoch() else {
        return Step::NotStarted;
    };

    if fold.run_is_ending() {
        return Step::Closure(fold.derived_outcome());
    }

    if let Some(candidate) = eligible_integration(fold) {
        return match ceiling.run_breach(spend) {
            Some(breach) => budget_exceeded(epoch, breach, None),
            None => Step::Integrate {
                candidate: Box::new(candidate),
            },
        };
    }
    if let Some((key, generation, attempt)) = first_ready_retry(fold) {
        return ceiling_or(ceiling, spend, epoch, key, || Step::Retry {
            key,
            generation,
            attempt,
        });
    }
    if let Some((key, generation, continuing)) = first_ready(fold) {
        if is_repair(fold, key) {
            return ceiling_or(ceiling, spend, epoch, key, || Step::RepairDispatch {
                key,
                generation,
                continuing,
            });
        }
        return ceiling_or(ceiling, spend, epoch, key, || Step::Dispatch {
            key,
            continuing,
            generation,
        });
    }
    if backoff_pending(fold) {
        return Step::Backoff;
    }
    if fold.questions_open() {
        return Step::HardBlock {
            questions: open_questions(fold),
        };
    }
    Step::Closure(fold.derived_outcome())
}

pub fn checkpoint(step: Step) -> Result<Admitted, UpstrokeError> {
    match step {
        Step::BudgetExceeded(exceeded) => Ok(Admitted::BudgetExceeded(exceeded)),
        Step::Retry {
            key,
            generation,
            attempt,
        } => Ok(Admitted::Retry {
            key,
            generation,
            attempt,
        }),
        Step::Dispatch {
            key,
            generation,
            continuing,
        } => Ok(Admitted::Dispatch {
            key,
            generation,
            continuing,
        }),
        Step::Integrate { candidate } => Ok(Admitted::Integrate { candidate }),
        Step::Backoff => Ok(Admitted::Backoff),
        Step::HardBlock { questions } => Ok(Admitted::HardBlock { questions }),
        Step::RepairDispatch {
            key,
            generation,
            continuing,
        } => Ok(Admitted::RepairDispatch {
            key,
            generation,
            continuing,
        }),
        Step::Closure(outcome) => Ok(Admitted::Closure(outcome)),
        Step::NotStarted => Err(UpstrokeError::Refused {
            message: "the run has not started: nothing is admitted from a fold with no \
                      `run_started`, and nothing was appended"
                .to_owned(),
        }),
        Step::Finished(outcome) => Err(UpstrokeError::Refused {
            message: format!(
                "this run already finished as `{}`, and continuation of a finished run is \
                 refused after finalization; nothing was appended",
                super::report::outcome_label(&outcome)
            ),
        }),
        Step::Poisoned => Err(UpstrokeError::Refused {
            message: "an append returned an error and this process's fold is poisoned: nothing \
                      further is selected, and no report, cleanup or question payload is derived \
                      from it"
                .to_owned(),
        }),
    }
}

fn eligible_integration(fold: &TopologyFold) -> Option<CandidateRef> {
    fold.eligible_integration_candidate().cloned()
}

fn first_ready_retry(fold: &TopologyFold) -> Option<(TaskKey, GenerationId, AttemptNumber)> {
    keys(fold).find_map(|key| {
        if !fold.ready_retry(key) {
            return None;
        }
        let generation = open_generation(fold, key)?;
        let task = fold.task(key)?;
        let open = task.generations.iter().find(|held| held.id == generation)?;
        matches!(open.class, GenerationClass::RetainedIdle { .. }).then(|| {
            (
                key,
                generation,
                AttemptNumber(open.attempts.saturating_add(1)),
            )
        })
    })
}

fn first_ready(fold: &TopologyFold) -> Option<(TaskKey, GenerationId, bool)> {
    keys(fold).find_map(|key| {
        if let Some(generation) = fold.eligible_continuation(key) {
            return Some((key, generation, true));
        }
        if !fold.ready(key) {
            return None;
        }
        let task = fold.task(key)?;
        let Ok(generation) = u32::try_from(task.generations.len()) else {
            return None;
        };
        Some((key, GenerationId(generation), false))
    })
}

fn is_repair(fold: &TopologyFold, key: TaskKey) -> bool {
    fold.registry()
        .and_then(|registry| registry.get(key))
        .is_some_and(|entry| entry.origin == crate::topology::registry::Origin::MergeRepair)
}

fn open_generation(fold: &TopologyFold, key: TaskKey) -> Option<GenerationId> {
    fold.task(key)?
        .generations
        .iter()
        .find(|generation| generation.class != GenerationClass::Closed)
        .map(|generation| generation.id)
}

fn keys(fold: &TopologyFold) -> impl Iterator<Item = TaskKey> + '_ {
    let len = fold.registry().map_or(0, |registry| {
        u32::try_from(registry.len()).unwrap_or(u32::MAX)
    });
    (0..len).map(TaskKey)
}

fn backoff_pending(fold: &TopologyFold) -> bool {
    !fold.run_is_ending() && fold.backoff_pending()
}

fn open_questions(fold: &TopologyFold) -> Vec<QuestionId> {
    fold.open_questions()
        .map(|questions| questions.keys().cloned().collect())
        .unwrap_or_default()
}

fn ceiling_or(
    ceiling: &Ceiling,
    spend: &Spend,
    epoch: Epoch,
    key: TaskKey,
    admitted: impl FnOnce() -> Step,
) -> Step {
    match ceiling.breach(spend, key) {
        Some(breach) => budget_exceeded(epoch, breach, Some(key)),
        None => admitted(),
    }
}

fn budget_exceeded(epoch: Epoch, breach: Breach, key: Option<TaskKey>) -> Step {
    Step::BudgetExceeded(Box::new(BudgetExceeded4 {
        epoch,
        budget: breach.budget,
        limit_usd: breach.limit_usd,
        spent_usd: breach.spent_usd,
        key,
    }))
}

pub const MERGE_ENTITLEMENTS: usize = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entitlements {
    pipeline_held: usize,
    merge_held: usize,
    max_parallel: u32,
    poisoned: bool,
}

impl Entitlements {
    #[must_use]
    pub fn of(fold: &TopologyFold) -> Self {
        Self {
            pipeline_held: fold.pipeline_held(),
            merge_held: usize::from(fold.transaction().is_some()),
            max_parallel: fold
                .started()
                .map_or(0, |started| started.limits.max_parallel),
            poisoned: fold.is_poisoned(),
        }
    }

    #[must_use]
    pub const fn pipeline_held(&self) -> usize {
        self.pipeline_held
    }

    #[must_use]
    pub const fn merge_held(&self) -> usize {
        self.merge_held
    }

    #[must_use]
    pub const fn max_parallel(&self) -> u32 {
        self.max_parallel
    }

    #[must_use]
    pub const fn poisoned(&self) -> bool {
        self.poisoned
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Holds {
    Preflight,
    Pipeline,
    PipelineAndMerge,
    Nothing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pipeline {
    Attempt {
        key: TaskKey,
        generation: GenerationId,
        attempt: AttemptNumber,
    },
    Sequence(SequenceId),
    Preflight,
}

impl Pipeline {
    fn of(invocation: &InvocationId) -> Self {
        match invocation {
            InvocationId::Attempt {
                key,
                generation,
                attempt,
                ..
            } => Self::Attempt {
                key: *key,
                generation: *generation,
                attempt: *attempt,
            },
            InvocationId::Sequence { sequence, .. } => Self::Sequence(*sequence),
            InvocationId::Probe { .. } => Self::Preflight,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Standing {
    pipeline: Pipeline,
    holds: Holds,
}

impl Standing {
    #[must_use]
    pub fn of(fold: &TopologyFold, invocation: &InvocationId) -> Self {
        let pipeline = Pipeline::of(invocation);
        let holds = if fold.is_poisoned() {
            Holds::Nothing
        } else {
            match pipeline {
                Pipeline::Preflight => Holds::Preflight,
                Pipeline::Attempt {
                    key,
                    generation,
                    attempt,
                } => {
                    let in_flight = fold
                        .task(key)
                        .and_then(|task| task.generations.iter().find(|held| held.id == generation))
                        .is_some_and(|held| held.class == GenerationClass::InFlight { attempt });
                    if in_flight {
                        Holds::Pipeline
                    } else {
                        Holds::Nothing
                    }
                }
                Pipeline::Sequence(sequence) => {
                    let verifying = fold.transaction().is_some_and(|open| {
                        open.sequence == sequence
                            && matches!(
                                open.class,
                                crate::topology::fold::TransactionClass::VerificationStarted { .. }
                            )
                    });
                    if verifying {
                        Holds::PipelineAndMerge
                    } else {
                        Holds::Nothing
                    }
                }
            }
        };
        Self { pipeline, holds }
    }

    #[must_use]
    pub const fn preflight() -> Self {
        Self {
            pipeline: Pipeline::Preflight,
            holds: Holds::Preflight,
        }
    }

    #[must_use]
    pub const fn holds(&self) -> Holds {
        self.holds
    }

    pub(super) fn admits(&self, invocation: &InvocationId) -> Result<(), UpstrokeError> {
        if Pipeline::of(invocation) != self.pipeline {
            return Err(UpstrokeError::Refused {
                message: format!(
                    "`{invocation}` presented the standing of another pipeline: a standing is \
                     read from the fold for the attempt, the verification or the pre-flight an \
                     invocation belongs to, and admits that one's invocations only"
                ),
            });
        }
        match self.holds {
            Holds::Preflight | Holds::Pipeline | Holds::PipelineAndMerge => Ok(()),
            Holds::Nothing => Err(UpstrokeError::Refused {
                message: format!(
                    "`{invocation}` asked for its slot pair and the fold shows its pipeline \
                     holding no entitlement: `permits.deadlock_freedom` orders acquisition \
                     pipeline -> merge -> {{agent, pool}}, so a pair is requested only by an \
                     attempt the fold has in flight, by the integration verification the fold \
                     has started, or by a pre-flight probe"
                ),
            }),
        }
    }
}

#[cfg(test)]
mod tests;
