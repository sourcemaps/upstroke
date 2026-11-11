//! Extended notes: `docs/internals/engine/topology/closure.md`

use std::collections::BTreeSet;

use crate::error::UpstrokeError;
use crate::events::RunOutcome;
use crate::topology::events::{
    AttemptInterrupted4, AttemptNumber, CommitSha, DerivedOutcome, GenerationId, GitRef,
    LeaseDisposition, MergeVerificationInterrupted, RunFinished4, SequenceId, TopologyEventBody,
    VerificationBasis,
};
use crate::topology::fold::{GenerationClass, TaskState, TopologyFold, TransactionClass};
use crate::topology::registry::TaskKey;

use super::report::{merged_and_parked, outcome_label};

pub fn ending_outcome(fold: &TopologyFold) -> Result<RunOutcome, UpstrokeError> {
    if fold.epoch().is_none() {
        return Err(refused(
            "the run has not started, so there is nothing to close and no outcome to derive",
        ));
    }
    if fold.halted_at().is_some() {
        return Ok(RunOutcome::Halted);
    }
    if fold.budget_stop().is_some() {
        return Ok(RunOutcome::BudgetExceeded);
    }
    match fold.derived_outcome() {
        DerivedOutcome::Ending(outcome) => Ok(outcome),
        DerivedOutcome::NotEnding => Err(refused(&format!(
            "the run is not ending: nothing is selectable and no halting settlement or budget \
             stop is recorded, yet the fold derives NotEnding because {}. Nothing was appended",
            blockers(fold).join("; ")
        ))),
        DerivedOutcome::FoldError => Err(refused(
            "the fold derives no outcome for this state (the arm the census asserts unreachable); \
             nothing was appended",
        )),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InFlight {
    Attempt {
        key: TaskKey,
        generation: GenerationId,
        attempt: AttemptNumber,
        lease: LeaseDisposition,
    },
    Verification {
        sequence: SequenceId,
        key: TaskKey,
        pin: Option<(GitRef, CommitSha)>,
        cancelled: bool,
    },
}

impl InFlight {
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::Attempt {
                key,
                generation,
                attempt,
                ..
            } => format!(
                "task k{} generation {} is in flight (attempt {})",
                key.0, generation.0, attempt.0
            ),
            Self::Verification { sequence, key, .. } => format!(
                "integration sequence {} of task k{} is unresolved",
                sequence.0, key.0
            ),
        }
    }

    #[must_use]
    pub fn interrupted(&self) -> TopologyEventBody {
        match self {
            Self::Attempt {
                key,
                generation,
                attempt,
                lease,
            } => TopologyEventBody::AttemptInterrupted {
                data: AttemptInterrupted4 {
                    key: *key,
                    generation: *generation,
                    attempt: *attempt,
                    lease: *lease,
                    detail: "the run halted while this attempt was in flight: the coordinator \
                             cancelled its pipeline and the Runner terminated its processes \
                             before this was appended; the spend is unknown"
                        .to_owned(),
                },
            },
            Self::Verification {
                sequence,
                cancelled,
                ..
            } => TopologyEventBody::MergeVerificationInterrupted {
                data: MergeVerificationInterrupted {
                    sequence: *sequence,
                    detail: if *cancelled {
                        "a decline, or a lineage member's failed settlement, failed this \
                         verification's lineage and cancelled it: its pipeline had ended, \
                         stopped by the coordinator or with a result the cancellation \
                         discards, and the Runner had established the end of each of its \
                         processes before this was appended; nothing was published, and its \
                         candidate, whose task failed, is not verified again"
                    } else {
                        "the run halted while this verification was in flight: its \
                         pipeline had ended, cancelled by the coordinator or with a result \
                         the halt discards unprepared, and the Runner had established the end \
                         of each of its processes before this was appended; nothing was \
                         published and the candidate stays queued"
                    }
                    .to_owned(),
                },
            },
        }
    }
}

#[must_use]
pub fn in_flight(fold: &TopologyFold) -> Vec<InFlight> {
    let mut found = Vec::new();
    for key in keys(fold) {
        let Some(task) = fold.task(key) else { continue };
        for generation in &task.generations {
            if let GenerationClass::InFlight { attempt } = generation.class {
                found.push(InFlight::Attempt {
                    key,
                    generation: generation.id,
                    attempt,
                    lease: generation.lease.expected(false),
                });
            }
        }
    }
    if let Some(transaction) = fold.transaction() {
        if let TransactionClass::VerificationStarted {
            basis,
            proposed_sha,
            ..
        } = &transaction.class
        {
            found.push(InFlight::Verification {
                sequence: transaction.sequence,
                key: transaction.candidate.key,
                pin: match basis {
                    VerificationBasis::StaleClean { prepared_ref } => {
                        Some((prepared_ref.clone(), proposed_sha.clone()))
                    }
                    VerificationBasis::AlreadyPresent => None,
                },
                cancelled: cancelled_by_lineage(fold, transaction.sequence),
            });
        }
    }
    found
}

#[must_use]
pub fn cancelled_by_lineage(fold: &TopologyFold, sequence: SequenceId) -> bool {
    fold.transaction().is_some_and(|open| {
        open.sequence == sequence
            && matches!(open.class, TransactionClass::VerificationStarted { .. })
            && fold.task_state(open.candidate.key) == Some(TaskState::Failed)
    })
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cancelled {
    attempts: BTreeSet<(TaskKey, GenerationId, AttemptNumber)>,
    sequences: BTreeSet<SequenceId>,
}

impl Cancelled {
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    pub fn attempt(&mut self, key: TaskKey, generation: GenerationId, attempt: AttemptNumber) {
        self.attempts.insert((key, generation, attempt));
    }

    pub fn sequence(&mut self, sequence: SequenceId) {
        self.sequences.insert(sequence);
    }

    #[must_use]
    pub fn vouches(&self, item: &InFlight) -> bool {
        match item {
            InFlight::Attempt {
                key,
                generation,
                attempt,
                ..
            } => self.attempts.contains(&(*key, *generation, *attempt)),
            InFlight::Verification { sequence, .. } => self.sequences.contains(sequence),
        }
    }
}

pub fn settleable(
    fold: &TopologyFold,
    outcome: &RunOutcome,
    cancelled: &Cancelled,
) -> Result<Vec<InFlight>, UpstrokeError> {
    let found = in_flight(fold);
    let unvouched: Vec<String> = found
        .iter()
        .filter(|item| !cancelled.vouches(item))
        .map(InFlight::describe)
        .collect();
    if !unvouched.is_empty() {
        return Err(unvouched_refusal(&unvouched));
    }
    if !found.is_empty() && *outcome != RunOutcome::Halted {
        return Err(refused(&format!(
            "run-end closure as `{}` found in-flight work its coordinator cancelled: {}. Only a \
             halt cancels in-flight work; a budget stop drains it to its natural settlements, so \
             nothing was appended",
            outcome_label(outcome),
            found
                .iter()
                .map(InFlight::describe)
                .collect::<Vec<_>>()
                .join("; ")
        )));
    }
    Ok(found)
}

pub fn refuse_unclosable(fold: &TopologyFold) -> Result<(), UpstrokeError> {
    let unclosable = unclosable(fold);
    if unclosable.is_empty() {
        return Ok(());
    }
    Err(unvouched_refusal(&unclosable))
}

fn unvouched_refusal(found: &[String]) -> UpstrokeError {
    refused(&format!(
        "run-end closure found in-flight work no pipeline of this process was cancelled for: {}. \
         The synchronous loop never leaves an attempt or a verification in flight, a fresh \
         process's recovery settles those a dead coordinator left before the loop runs, and \
         PR11's coordinator settles interrupted only those whose pipelines it cancelled and saw \
         end; nothing was appended",
        found.join("; ")
    ))
}

#[must_use]
pub fn unclosable(fold: &TopologyFold) -> Vec<String> {
    in_flight(fold).iter().map(InFlight::describe).collect()
}

#[must_use]
pub fn promoting(fold: &TopologyFold) -> Vec<TaskKey> {
    keys(fold)
        .filter(|key| {
            fold.task(*key).is_some_and(|task| {
                task.generations
                    .iter()
                    .any(|generation| generation.class == GenerationClass::Promoting)
            })
        })
        .collect()
}

#[must_use]
pub fn closable(fold: &TopologyFold) -> Vec<TaskKey> {
    keys(fold)
        .filter(|key| {
            fold.task(*key).is_some_and(|task| {
                task.generations.iter().any(|generation| {
                    matches!(
                        generation.class,
                        GenerationClass::OpenNoAttempt | GenerationClass::RetainedIdle { .. }
                    )
                })
            })
        })
        .collect()
}

pub fn confirm_derived(fold: &TopologyFold, outcome: &RunOutcome) -> Result<(), UpstrokeError> {
    match fold.derived_outcome() {
        DerivedOutcome::Ending(derived) if derived == *outcome => Ok(()),
        DerivedOutcome::Ending(derived) => Err(refused(&format!(
            "closure was ending the run as `{}` and, with every open generation closed, the fold \
             derives `{}`; nothing was appended for the end",
            outcome_label(outcome),
            outcome_label(&derived)
        ))),
        DerivedOutcome::NotEnding => Err(refused(&format!(
            "closure closed every open generation and the run is still not ending because {}; \
             nothing was appended for the end",
            blockers(fold).join("; ")
        ))),
        DerivedOutcome::FoldError => Err(refused(
            "closure closed every open generation and the fold derives no outcome for what is \
             left (the arm the census asserts unreachable); nothing was appended for the end",
        )),
    }
}

#[must_use]
pub fn run_finished(fold: &TopologyFold, outcome: RunOutcome) -> RunFinished4 {
    let (merged, parked) = merged_and_parked(fold);
    RunFinished4 {
        outcome,
        halted_at: fold.halted_at(),
        merged,
        parked,
    }
}

#[must_use]
pub fn blockers(fold: &TopologyFold) -> Vec<String> {
    let mut found = unclosable(fold);
    for key in keys(fold) {
        let Some(task) = fold.task(key) else { continue };
        for generation in &task.generations {
            match &generation.class {
                GenerationClass::OpenNoAttempt => found.push(format!(
                    "task k{} generation {} is open with no attempt",
                    key.0, generation.id.0
                )),
                GenerationClass::RetainedIdle { incarnation, .. } => found.push(format!(
                    "task k{} generation {} is retained idle from epoch {}",
                    key.0, generation.id.0, incarnation.0
                )),
                GenerationClass::Promoting => found.push(format!(
                    "task k{} generation {} is promoting",
                    key.0, generation.id.0
                )),
                _ => {}
            }
        }
        if task.state == TaskState::Deferred {
            found.push(format!("task k{} is deferred (backoff pending)", key.0));
        }
    }
    if let Some(transaction) = fold.transaction() {
        if matches!(transaction.class, TransactionClass::Prepared { .. }) {
            found.push(format!(
                "integration sequence {} of task k{} is prepared and not yet published",
                transaction.sequence.0, transaction.candidate.key.0
            ));
        }
    }
    if let Some(queue) = fold.queue() {
        for entry in queue.entries() {
            if entry.verification_deferred {
                found.push(format!(
                    "candidate k{} g{} is verification-deferred (backoff pending)",
                    entry.candidate.key.0, entry.candidate.generation.0
                ));
            }
        }
    }
    if fold.structurally_admissible() {
        found.push("structurally admissible work exists".to_owned());
    }
    if found.is_empty() {
        found.push("of a state this module cannot name".to_owned());
    }
    found
}

fn keys(fold: &TopologyFold) -> impl Iterator<Item = TaskKey> + '_ {
    let len = fold.registry().map_or(0, |registry| {
        u32::try_from(registry.len()).unwrap_or(u32::MAX)
    });
    (0..len).map(TaskKey)
}

fn refused(message: &str) -> UpstrokeError {
    UpstrokeError::Refused {
        message: message.to_owned(),
    }
}
