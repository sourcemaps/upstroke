//! Extended notes: `docs/internals/engine/topology/coordinator/tests/interleaving.md`

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use super::*;
use crate::engine::topology::identity::{
    AttemptIdentities, InvocationLedger, ReservationKind, Reservations, SequenceIdentities,
    is_slotted,
};
use crate::engine::topology::scaffold::{AGENT, REVIEW_AGENT, Ran, SlotLimitsOf, exited};
use crate::engine::topology::select::Entitlements;
use crate::runner::ExecutionRole;
use crate::runner::invocation::SequenceRole;
use crate::topology::effects::{
    EffectSiteId, HookPhase, Injection, ObjectSite, RefSite, WorktreeSite,
};
use crate::topology::events::{PreparedDisposition, RejectionDisposition};

const STEPS: usize = 256;

const G6_EXPORT: &str = "UPSTROKE_G6_EXPORT";

fn export(kind: &str, value: &serde_json::Value) {
    let Ok(dir) = std::env::var(G6_EXPORT) else {
        return;
    };
    let thread = std::thread::current();
    let test = thread
        .name()
        .and_then(|name| name.rsplit("::").next())
        .unwrap_or("unnamed");
    let path = PathBuf::from(dir).join(kind).join(format!("{test}.json"));
    let json = serde_json::to_vec_pretty(value).expect("an export serializes");
    crate::workspace_manager::fixture::write_file(&path, &json);
}

fn digest(text: &str) -> String {
    use sha2::Digest;
    format!("{:x}", sha2::Sha256::digest(text.as_bytes()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Adverse {
    Seeded,
    Newest,
    Oldest,
    AgentsLast,
}

impl Adverse {
    const fn name(self) -> &'static str {
        match self {
            Self::Seeded => "seeded",
            Self::Newest => "newest-granted-first",
            Self::Oldest => "oldest-granted-first",
            Self::AgentsLast => "gates-first-agents-last",
        }
    }
}

#[derive(Debug, Clone, Default)]
struct Point {
    invoking: Vec<InvocationId>,
    provisional: u32,
    peak: usize,
    discarded: u32,
    duplicates: u32,
    injected: usize,
}

type Inject<'r> =
    Box<dyn FnMut(&Quiescent<'_>, &[InvocationId], &mut Seeded) -> Option<ToCoordinator> + 'r>;

type Watch<'r> = Box<dyn FnMut(&Quiescent<'_>) + 'r>;

struct Interleaver<'r> {
    runner: &'r RecordingRunner,
    order: Seeded,
    adverse: Adverse,
    dice: Seeded,
    inject: Option<Inject<'r>>,
    watch: Option<Watch<'r>>,
    granted: Vec<InvocationId>,
    released: Vec<InvocationId>,
    points: Vec<Point>,
    injected: usize,
}

impl<'r> Interleaver<'r> {
    fn new(runner: &'r RecordingRunner, seed: u64, adverse: Adverse) -> Self {
        runner.enter_late();
        Self {
            runner,
            order: Seeded(seed),
            adverse,
            dice: Seeded(seed ^ 0xD1CE_D1CE),
            inject: None,
            watch: None,
            granted: Vec::new(),
            released: Vec::new(),
            points: Vec::new(),
            injected: 0,
        }
    }

    fn injecting(mut self, inject: Inject<'r>) -> Self {
        self.inject = Some(inject);
        self
    }

    fn watching(mut self, watch: Watch<'r>) -> Self {
        self.watch = Some(watch);
        self
    }

    fn choose(&mut self, invoking: &[InvocationId]) -> Option<InvocationId> {
        let mut sorted = invoking.to_vec();
        sorted.sort();
        match self.adverse {
            Adverse::Seeded => {
                let index = self.order.below(sorted.len());
                sorted.get(index).cloned()
            }
            Adverse::Newest => self
                .granted
                .iter()
                .rev()
                .find(|granted| invoking.contains(granted))
                .cloned(),
            Adverse::Oldest => self
                .granted
                .iter()
                .find(|granted| invoking.contains(granted))
                .cloned(),
            Adverse::AgentsLast => sorted
                .iter()
                .find(|held| !is_slotted(held))
                .or_else(|| sorted.last())
                .cloned(),
        }
    }
}

impl Quiescence for Interleaver<'_> {
    fn granted(&mut self, invocation: &InvocationId) {
        assert!(
            self.runner.admit(invocation, BOUND),
            "`{invocation}` was granted and did not reach the runner within {BOUND:?}"
        );
        self.granted.push(invocation.clone());
    }

    fn quiescent(&mut self, view: &Quiescent<'_>) -> Release {
        for invocation in &view.invoking {
            assert!(
                self.runner.inside(invocation),
                "`{invocation}` is granted and not inside the runner at a quiescent point"
            );
        }
        self.points.push(Point {
            invoking: view.invoking.clone(),
            provisional: view.run.entitlements_held(),
            peak: view.run.reservations_peak(),
            discarded: view.run.discarded(),
            duplicates: view.run.broker_duplicates(),
            injected: self.injected,
        });
        if let Some(watch) = self.watch.as_mut() {
            watch(view);
        }
        if self.points.len() > STEPS {
            return Release::Nothing;
        }
        let roll = self.dice.below(4);
        if roll == 0 {
            if let Some(inject) = self.inject.as_mut() {
                if let Some(message) = inject(view, &self.released, &mut self.dice) {
                    if view.injector.inject(message) {
                        self.injected += 1;
                        return Release::Injected;
                    }
                }
            }
        }
        match self.choose(&view.invoking) {
            Some(invocation) => {
                self.runner
                    .release(&invocation, BOUND)
                    .unwrap_or_else(|error| panic!("releasing `{invocation}`: {error}"));
                self.released.push(invocation.clone());
                Release::Invocation(invocation)
            }
            None => Release::Nothing,
        }
    }
}

fn mixed() -> [WideTask; 4] {
    [
        WideTask::independent("alpha"),
        WideTask::independent("beta"),
        WideTask::independent("gamma"),
        WideTask::after("delta", &["alpha"]),
    ]
}

const BETA_RETRIES: [(u32, u32); 1] = [(1, 1)];

fn two_reviewers() -> WidePlans {
    WidePlans {
        reviewers: 2,
        verify_reviewers: 2,
        ..WidePlans::default()
    }
}

fn run_id_of(wide: &Wide) -> String {
    wide.run.fold().started().expect("started").run_id.clone()
}

fn agent_of(ran: &[Ran], invocation: &InvocationId) -> Option<String> {
    ran.iter()
        .rev()
        .find(|entry| entry.invocation == *invocation)
        .and_then(|entry| entry.agent.as_ref().map(|agent| agent.as_str().to_owned()))
}

fn pool_of(plans: &WidePlans, agent: &str) -> Option<String> {
    plans
        .pools
        .iter()
        .find(|(named, _)| *named == agent)
        .map_or_else(|| plans.pool.clone(), |(_, pool)| pool.clone())
}

fn held_over_limits(
    points: &[Point],
    ran: &[Ran],
    plans: &WidePlans,
    per_agent: usize,
    per_pool: usize,
) -> Vec<String> {
    let mut over = Vec::new();
    for (index, point) in points.iter().enumerate() {
        let mut agents: BTreeMap<String, usize> = BTreeMap::new();
        let mut pools: BTreeMap<String, usize> = BTreeMap::new();
        for held in point.invoking.iter().filter(|held| is_slotted(held)) {
            let agent = agent_of(ran, held).unwrap_or_default();
            if let Some(pool) = pool_of(plans, &agent) {
                *pools.entry(pool).or_default() += 1;
            }
            *agents.entry(agent).or_default() += 1;
        }
        for (agent, held) in &agents {
            if *held > per_agent {
                over.push(format!("point {index}: {held} processes of `{agent}` held"));
            }
        }
        for (pool, held) in &pools {
            if *held > per_pool {
                over.push(format!(
                    "point {index}: {held} processes in pool `{pool}` held"
                ));
            }
        }
    }
    over
}

fn most_held_of(points: &[Point], ran: &[Ran], agent: &str) -> usize {
    points
        .iter()
        .map(|point| {
            point
                .invoking
                .iter()
                .filter(|held| is_slotted(held) && agent_of(ran, held).as_deref() == Some(agent))
                .count()
        })
        .max()
        .unwrap_or(0)
}

fn both_held(points: &[Point], ran: &[Ran], first: &str, second: &str) -> bool {
    points.iter().any(|point| {
        let agents: BTreeSet<String> = point
            .invoking
            .iter()
            .filter(|held| is_slotted(held))
            .filter_map(|held| agent_of(ran, held))
            .collect();
        agents.contains(first) && agents.contains(second)
    })
}

struct Shape<'a> {
    tag: String,
    tasks: &'a [WideTask],
    plans: WidePlans,
    runner: RecordingRunner,
    limits: SlotLimitsOf,
}

impl<'a> Shape<'a> {
    fn of(tag: String, tasks: &'a [WideTask], plans: WidePlans, runner: RecordingRunner) -> Self {
        Self {
            tag,
            tasks,
            plans,
            runner,
            limits: SlotLimitsOf::Defaulted,
        }
    }

    const fn limited(mut self, limits: SlotLimitsOf) -> Self {
        self.limits = limits;
        self
    }
}

struct SeededRun {
    wide: Wide,
    released: Vec<InvocationId>,
    points: Vec<Point>,
    injected: usize,
    outcome: Result<Progress, UpstrokeError>,
}

fn seeded_run(
    shape: Shape<'_>,
    seed: u64,
    adverse: Adverse,
    inject: Option<Inject<'static>>,
) -> SeededRun {
    let Shape {
        tag,
        tasks,
        plans,
        runner,
        limits,
    } = shape;
    let mut wide = Wide::started_with(&tag, tasks, 3, plans, runner);
    let double = std::sync::Arc::clone(&wide.env.runner);
    let mut interleaver = Interleaver::new(&double, seed, adverse);
    if let Some(inject) = inject {
        interleaver = interleaver.injecting(inject);
    }
    let mut hooks = wide.env.hooks();
    let pipelines = wide.env.pipelines_limited(limits);
    let outcome = wide.run.run_concurrently(
        &wide.env.seams(),
        &pipelines,
        &mut hooks,
        Some(&mut interleaver),
    );
    let Interleaver {
        released,
        points,
        injected,
        ..
    } = interleaver;
    SeededRun {
        wide,
        released,
        points,
        injected,
        outcome,
    }
}

fn started_log(wide: &Wide) -> Vec<(String, Vec<String>, String)> {
    wide.env
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
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Owner {
    Worktree(u32, u32),
    AttemptSnapshot(u32, u32, u32, String),
    SequenceSnapshot(u32, String),
}

fn owner_of(invocation: &InvocationId) -> Option<Owner> {
    match invocation {
        InvocationId::Attempt {
            key,
            generation,
            attempt,
            role,
            ..
        } => Some(match role {
            AttemptRole::Worker => Owner::Worktree(key.0, generation.0),
            AttemptRole::Gate(_) => {
                Owner::AttemptSnapshot(key.0, generation.0, attempt.0, "gates".to_owned())
            }
            AttemptRole::ReviewPass(pass) | AttemptRole::ReviewReask(pass) => {
                Owner::AttemptSnapshot(key.0, generation.0, attempt.0, format!("review{pass}"))
            }
        }),
        InvocationId::Sequence { sequence, role, .. } => Some(match role {
            SequenceRole::Gate(_) => Owner::SequenceSnapshot(sequence.0, "integration".to_owned()),
            SequenceRole::ReviewPass(pass) | SequenceRole::ReviewReask(pass) => {
                Owner::SequenceSnapshot(sequence.0, format!("review{pass}"))
            }
        }),
        InvocationId::Probe { .. } => None,
    }
}

fn aliases(events: &[TopologyEvent], ran: &[Ran]) -> (Vec<String>, BTreeMap<&'static str, usize>) {
    let mut found = Vec::new();
    let mut counted: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut ids = BTreeSet::new();
    for entry in ran {
        if !ids.insert(entry.invocation.render()) {
            found.push(format!("InvocationId `{}` started twice", entry.invocation));
        }
    }
    counted.insert("invocations", ids.len());

    let mut paths: BTreeMap<PathBuf, Owner> = BTreeMap::new();
    let mut owners: BTreeMap<Owner, PathBuf> = BTreeMap::new();
    for entry in ran {
        let Some(owner) = owner_of(&entry.invocation) else {
            continue;
        };
        if let Some(earlier) = paths.insert(entry.workspace.clone(), owner.clone()) {
            if earlier != owner {
                found.push(format!(
                    "{} is used by {earlier:?} and by {owner:?}",
                    entry.workspace.display()
                ));
            }
        }
        if let Some(earlier) = owners.insert(owner.clone(), entry.workspace.clone()) {
            if earlier != entry.workspace {
                found.push(format!(
                    "{owner:?} ran in {} and in {}",
                    earlier.display(),
                    entry.workspace.display()
                ));
            }
        }
    }
    counted.insert(
        "worktrees",
        owners
            .keys()
            .filter(|owner| matches!(owner, Owner::Worktree(..)))
            .count(),
    );
    counted.insert(
        "snapshots",
        owners
            .keys()
            .filter(|owner| !matches!(owner, Owner::Worktree(..)))
            .count(),
    );

    let mut generations = BTreeSet::new();
    let mut attempts = BTreeSet::new();
    let mut settled: BTreeMap<(u32, u32, u32), usize> = BTreeMap::new();
    let mut candidates: BTreeSet<String> = BTreeSet::new();
    let mut queued: Vec<u32> = Vec::new();
    let mut sequences: BTreeMap<u32, u32> = BTreeMap::new();
    let mut sequence_order: Vec<u32> = Vec::new();
    let mut pins: BTreeSet<String> = BTreeSet::new();
    let mut integrated: Vec<u32> = Vec::new();
    let mut merged: BTreeMap<u32, usize> = BTreeMap::new();
    for event in events {
        match &event.body {
            TopologyEventBody::TaskDispatched { data } => {
                if !generations.insert((data.key.0, data.generation.0)) {
                    found.push(format!(
                        "task {} generation {} was dispatched twice",
                        data.key.0, data.generation.0
                    ));
                }
            }
            TopologyEventBody::AttemptStarted { data } => {
                if !attempts.insert((data.key.0, data.generation.0, data.attempt.0)) {
                    found.push(format!(
                        "attempt {} of task {} generation {} started twice",
                        data.attempt.0, data.key.0, data.generation.0
                    ));
                }
            }
            TopologyEventBody::AttemptFinished { data } => {
                *settled
                    .entry((data.key.0, data.generation.0, data.attempt.0))
                    .or_default() += 1;
            }
            TopologyEventBody::CandidatePrepared { data } => {
                *settled
                    .entry((data.key.0, data.generation.0, data.attempt.attempt))
                    .or_default() += 1;
            }
            TopologyEventBody::AttemptInterrupted { data } => {
                *settled
                    .entry((data.key.0, data.generation.0, data.attempt.0))
                    .or_default() += 1;
            }
            TopologyEventBody::TaskCandidateCreated { data } => {
                if !candidates.insert(data.candidate.candidate_ref.0.clone()) {
                    found.push(format!(
                        "candidate ref `{}` was created twice",
                        data.candidate.candidate_ref.0
                    ));
                }
                queued.push(data.candidate.key.0);
            }
            TopologyEventBody::MergeVerificationStarted { data } => {
                if let Some(earlier) = sequences.insert(data.sequence.0, data.candidate.key.0) {
                    found.push(format!(
                        "sequence {} was claimed by tasks {earlier} and {}",
                        data.sequence.0, data.candidate.key.0
                    ));
                }
                sequence_order.push(data.sequence.0);
            }
            TopologyEventBody::MergePrepared { data } => {
                match sequences.get(&data.sequence.0) {
                    Some(key) if *key != data.key.0 => found.push(format!(
                        "sequence {} verified task {key} and prepared task {}",
                        data.sequence.0, data.key.0
                    )),
                    Some(_) => {}
                    None => {
                        sequences.insert(data.sequence.0, data.key.0);
                        sequence_order.push(data.sequence.0);
                    }
                }
                if let Some(pin) = &data.prepared_ref {
                    if !pins.insert(pin.0.clone()) {
                        found.push(format!("prepared pin `{}` was taken twice", pin.0));
                    }
                }
                integrated.push(data.key.0);
            }
            TopologyEventBody::TaskMerged { data } => {
                *merged.entry(data.sequence.0).or_default() += 1;
            }
            _ => {}
        }
    }
    for ((key, generation, attempt), times) in &settled {
        if *times != 1 {
            found.push(format!(
                "attempt {attempt} of task {key} generation {generation} was settled \
                         {times} times"
            ));
        }
    }
    if attempts.len() != settled.len() {
        found.push(format!(
            "{} attempts started and {} settled",
            attempts.len(),
            settled.len()
        ));
    }
    if sequence_order
        .windows(2)
        .any(|pair| matches!(pair, [earlier, later] if earlier >= later))
    {
        found.push(format!("sequences out of order: {sequence_order:?}"));
    }
    if integrated != queued {
        found.push(format!(
            "queue positions: integrated {integrated:?}, queued {queued:?}"
        ));
    }
    if merged.values().any(|times| *times != 1) {
        found.push(format!("a sequence merged more than once: {merged:?}"));
    }
    counted.insert("generations", generations.len());
    counted.insert("attempts", attempts.len());
    counted.insert("candidate_refs", candidates.len());
    counted.insert("sequences", sequences.len());
    counted.insert("prepared_pins", pins.len());
    counted.insert("queue_positions", queued.len());
    (found, counted)
}

#[test]
fn seeded_runs_alias_no_task_generation_attempt_invocation_snapshot_ref_pin_sequence_or_queue_position()
 {
    let tasks = mixed();
    let mut seeds = Vec::new();
    let mut retried_beside = 0_usize;
    for seed in scheduled_here(0..8, &[0]) {
        let tag = format!("interleaving-st04-{seed}");
        let run = seeded_run(
            Shape::of(tag, &tasks, two_reviewers(), holding(&tasks, &BETA_RETRIES)),
            seed,
            Adverse::Seeded,
            None,
        );
        let SeededRun {
            mut wide,
            points,
            outcome,
            ..
        } = run;
        let progress = outcome.unwrap_or_else(|error| panic!("seed {seed}: {error}"));
        assert_eq!(outcome_of(&progress), RunOutcome::Complete, "seed {seed}");
        let events = wide.env.durable_events();
        let ran = wide.env.runner.ran();
        let (found, counted) = aliases(&events, &ran);
        assert!(found.is_empty(), "seed {seed}: {found:#?}");
        let beta_gates: Vec<String> = ran
            .iter()
            .filter(|entry| {
                attempt_key(&entry.invocation) == Some(1)
                    && matches!(role_of(&entry.invocation), Some(AttemptRole::Gate(_)))
            })
            .filter_map(|entry| {
                entry
                    .workspace
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(str::to_owned)
            })
            .collect();
        assert_eq!(
            beta_gates,
            gate_snapshot_names(&wide.env.fixture.manager, 1, 0, &[1, 2]),
            "seed {seed}: ST-15 — beta's retry re-gated on a fresh snapshot of its own"
        );
        let retry_beside = points.iter().any(|point| {
            point.invoking.iter().any(|held| {
                matches!(held, InvocationId::Attempt { key: TaskKey(1), attempt, .. }
                    if attempt.0 == 2)
            }) && point
                .invoking
                .iter()
                .any(|held| attempt_key(held) != Some(1))
        });
        retried_beside += usize::from(retry_beside);
        assert_eq!(
            counted.get("attempts"),
            Some(&5),
            "seed {seed}: four first attempts and beta's retry: {counted:?}"
        );
        assert!(
            counted
                .get("sequences")
                .is_some_and(|sequences| *sequences == 4),
            "seed {seed}: one sequence per task: {counted:?}"
        );
        assert_eq!(
            wide.run.reservations_peak(),
            1,
            "seed {seed}: at most one provisional reservation was ever outstanding"
        );
        assert!(
            points.iter().map(|point| point.invoking.len()).max() >= Some(3),
            "seed {seed}: three processes were in flight at once"
        );
        let ledger = wide.run.broker_mut().invocations();
        assert_eq!(
            ledger.registered(),
            ran.len(),
            "seed {seed}: every process registered once under its own InvocationId"
        );
        assert_eq!(ledger.duplicates(), 0, "seed {seed}");
        assert!(ledger.balances(), "seed {seed}");
        seeds.push(serde_json::json!({
            "seed": seed,
            "st15_retry_process_beside_another_task": retry_beside,
            "distinct": counted,
            "points": points.len(),
            "widest": points.iter().map(|point| point.invoking.len()).max(),
        }));
    }
    assert!(
        retried_beside > 0,
        "ST-15: beta's retry ran beside another task's process in some seed"
    );
    export(
        "seam/ST-04",
        &serde_json::json!({
            "row": "ST-04",
            "plan": "alpha, beta, gamma independent; delta after alpha; beta's first gate \
                             fails, so beta retries in its generation; two reviewers per attempt \
                             and per verification",
            "width": 3,
            "seeds": seeds,
            "checked": [
                "every process's InvocationId distinct over the whole run",
                "every task worktree and snapshot path used by exactly one owner, and \
                         every owner in exactly one path",
                "each generation dispatched once, each attempt started once and settled \
                         once",
                "each candidate ref created once, each prepared pin taken once",
                "each sequence claimed by one candidate, sequences in log order",
                "queue positions: integration in task_candidate_created order, each \
                         sequence merged once",
                "at most one provisional reservation outstanding at any time"
            ],
        }),
    );
}

fn mismatched(identity: &Identity) -> Identity {
    match identity {
        Identity::Attempt {
            key,
            generation,
            attempt,
        } => Identity::Attempt {
            key: *key,
            generation: *generation,
            attempt: AttemptNumber(attempt.0 + 1),
        },
        Identity::Verification {
            sequence,
            candidate,
        } => Identity::Verification {
            sequence: SequenceId(sequence.0 + 1),
            candidate: candidate.clone(),
        },
    }
}

fn duplicating(crossed: std::rc::Rc<std::cell::Cell<usize>>) -> Inject<'static> {
    let mut seen: BTreeMap<PipelineId, Identity> = BTreeMap::new();
    Box::new(move |view, released, dice| {
        seen.extend(view.live.iter().cloned());
        let owner = |invocation: &InvocationId| {
            view.live
                .iter()
                .find(|(_, identity)| identity.owns(invocation))
                .map(|(pipeline, _)| *pipeline)
        };
        match dice.below(6) {
            0 => {
                let settled: Vec<&InvocationId> = released
                    .iter()
                    .filter(|invocation| !view.invoking.contains(invocation))
                    .collect();
                let owned: Vec<&InvocationId> = settled
                    .iter()
                    .copied()
                    .filter(|invocation| owner(invocation).is_some())
                    .collect();
                let from = if owned.is_empty() || dice.below(4) == 0 {
                    settled
                } else {
                    owned
                };
                let invocation = (*from.get(dice.below(from.len()))?).clone();
                Some(ToCoordinator::Ended {
                    pipeline: owner(&invocation).unwrap_or(PipelineId(0)),
                    invocation,
                    end: InvocationEnd::Completed,
                })
            }
            1 => {
                let invocation = view.invoking.get(dice.below(view.invoking.len()))?.clone();
                Some(ToCoordinator::Ended {
                    pipeline: owner(&invocation)?,
                    invocation,
                    end: InvocationEnd::Completed,
                })
            }
            2 => {
                let (pipeline, _) = view.live.get(dice.below(view.live.len()))?;
                Some(ToCoordinator::SnapshotEnd {
                    pipeline: *pipeline,
                })
            }
            3 => {
                let retired: Vec<(&PipelineId, &Identity)> = seen
                    .iter()
                    .filter(|(pipeline, _)| !view.live.iter().any(|(live, _)| live == *pipeline))
                    .collect();
                let (pipeline, identity) = match retired.get(dice.below(retired.len())) {
                    Some((pipeline, identity)) => (**pipeline, (*identity).clone()),
                    None => {
                        let (_, identity) = view.live.get(dice.below(view.live.len()))?;
                        (PipelineId(0), identity.clone())
                    }
                };
                Some(ToCoordinator::Judged {
                    pipeline,
                    identity,
                    outcome: Ok(forged()),
                })
            }
            4 => {
                let (pipeline, identity) = view.live.get(dice.below(view.live.len()))?;
                let invocation = match identity {
                    Identity::Attempt {
                        key,
                        generation,
                        attempt,
                    } => AttemptIdentities::new(*key, *generation, *attempt).gate(0, 99),
                    Identity::Verification { sequence, .. } => {
                        SequenceIdentities::new(*sequence).gate(0, 99)
                    }
                };
                let (reply, _) = oneshot::channel();
                Some(ToCoordinator::Admit {
                    pipeline: *pipeline,
                    invocation,
                    pair: None,
                    reply,
                })
            }
            _ => {
                let (pipeline, identity) = view.live.get(dice.below(view.live.len()))?;
                let other = view
                    .live
                    .iter()
                    .find(|(other, _)| other != pipeline)
                    .map_or_else(|| mismatched(identity), |(_, other)| other.clone());
                crossed.set(crossed.get() + 1);
                Some(ToCoordinator::Judged {
                    pipeline: *pipeline,
                    identity: other,
                    outcome: Ok(forged()),
                })
            }
        }
    })
}

#[test]
fn injected_duplicates_at_seeded_points_release_nothing_twice_and_change_nothing_durable() {
    let tasks = mixed();
    let mut seeds = Vec::new();
    let mut total = 0_usize;
    let mut total_crossed = 0_usize;
    for seed in scheduled_here(0..8, &[1]) {
        let reference = seeded_run(
            Shape::of(
                format!("interleaving-st05-reference-{seed}"),
                &tasks,
                two_reviewers(),
                holding(&tasks, &BETA_RETRIES),
            ),
            seed,
            Adverse::Seeded,
            None,
        );
        let crossed = std::rc::Rc::new(std::cell::Cell::new(0_usize));
        let injected = seeded_run(
            Shape::of(
                format!("interleaving-st05-injected-{seed}"),
                &tasks,
                two_reviewers(),
                holding(&tasks, &BETA_RETRIES),
            ),
            seed,
            Adverse::Seeded,
            Some(duplicating(std::rc::Rc::clone(&crossed))),
        );
        let SeededRun {
            wide: mut plain,
            released: plain_released,
            outcome: plain_outcome,
            ..
        } = reference;
        let SeededRun {
            wide: mut duplicated,
            released,
            points,
            injected: injections,
            outcome,
        } = injected;
        let plain_progress = plain_outcome.unwrap_or_else(|error| panic!("seed {seed}: {error}"));
        let progress = outcome.unwrap_or_else(|error| panic!("seed {seed}: {error}"));
        assert_eq!(
            outcome_of(&plain_progress),
            RunOutcome::Complete,
            "seed {seed}"
        );
        assert_eq!(outcome_of(&progress), RunOutcome::Complete, "seed {seed}");
        assert!(injections >= 3, "seed {seed}: only {injections} injections");
        total += injections;
        for pair in points.windows(2) {
            let [before, after] = pair else {
                continue;
            };
            let counted =
                (after.discarded + after.duplicates) - (before.discarded + before.duplicates);
            assert_eq!(
                usize::try_from(counted).expect("a small count"),
                after.injected - before.injected,
                "seed {seed}: each injected duplicate is discarded or counted as a \
                         duplicate, once, and nothing else moves the counts"
            );
        }
        assert_eq!(
            released, plain_released,
            "seed {seed}: the injections released nothing and reordered nothing"
        );
        assert_eq!(
            started_log(&duplicated),
            started_log(&plain),
            "seed {seed}: the same processes started, in the same order, on the same \
                     workspaces, seeing the same log"
        );
        let run_id = run_id_of(&plain);
        let plain_log = serde_json::to_string(&canonical(&plain.env.durable_events(), &run_id))
            .expect("serializes");
        let run_id = run_id_of(&duplicated);
        let duplicated_log =
            serde_json::to_string(&canonical(&duplicated.env.durable_events(), &run_id))
                .expect("serializes");
        assert_eq!(
            duplicated_log, plain_log,
            "seed {seed}: no injected settlement reached the log"
        );
        let counts = |wide: &mut Wide| {
            let discarded = wide.run.discarded();
            let reservations = wide.run.broker_mut().reservations();
            let provisional = (
                reservations.taken(),
                reservations.converted(),
                reservations.cancelled(),
                reservations.duplicates(),
                reservations.balances(),
            );
            let ledger = wide.run.broker_mut().invocations();
            (
                (
                    ledger.registered(),
                    ledger.completed(),
                    ledger.cancelled(),
                    ledger.slots().granted(),
                    ledger.slots().released(),
                    ledger.balances(),
                ),
                provisional,
                (discarded, ledger.duplicates()),
            )
        };
        let (plain_ledger, plain_provisional, (plain_discarded, plain_duplicates)) =
            counts(&mut plain);
        let (ledger, provisional, (discarded, duplicates)) = counts(&mut duplicated);
        assert_eq!(
            ledger, plain_ledger,
            "seed {seed}: registrations, settlements and slot grants and releases are \
                     the uninjected run's, each once"
        );
        assert_eq!(ledger.3, ledger.4, "seed {seed}: every pair released once");
        assert!(ledger.5, "seed {seed}: the invocation ledger balances");
        assert_eq!(
            provisional, plain_provisional,
            "seed {seed}: no provisional reservation was converted, cancelled or \
                     released twice"
        );
        assert_eq!(provisional.3, 0, "seed {seed}");
        assert!(provisional.4, "seed {seed}");
        assert_eq!(
            usize::try_from((discarded + duplicates) - (plain_discarded + plain_duplicates))
                .expect("a small count"),
            injections,
            "seed {seed}: every injection is accounted for exactly once"
        );
        let refused_by_the_binding = duplicated
            .run
            .warnings()
            .iter()
            .filter(|warning| warning.starts_with("a completion for") && warning.contains(BINDING))
            .count();
        assert_eq!(
            refused_by_the_binding,
            crossed.get(),
            "seed {seed}: every completion injected for a live pipeline under another \
                     identity was refused by the binding to that pipeline's identity, which an \
                     injected completion meets before it is refused as injected: {:?}",
            duplicated.run.warnings()
        );
        total_crossed += crossed.get();
        seeds.push(serde_json::json!({
            "seed": seed,
            "injected": injections,
            "counted_as_ledger_duplicates": duplicates - plain_duplicates,
            "discarded": discarded - plain_discarded,
            "invocation_ledger": {
                "registered": ledger.0,
                "completed": ledger.1,
                "cancelled": ledger.2,
                "pairs_granted": ledger.3,
                "pairs_released": ledger.4,
            },
            "provisional": {
                "taken": provisional.0,
                "converted": provisional.1,
                "cancelled": provisional.2,
                "duplicates": provisional.3,
            },
            "canonical_log_sha256": digest(&duplicated_log),
        }));
    }
    assert!(
        total >= 5 * seeds.len(),
        "{total} injections over {} seeds",
        seeds.len()
    );
    assert!(
        total_crossed > 0,
        "a completion under another identity was injected in some seed"
    );
    export(
        "ledgers/invocation",
        &serde_json::json!({
            "ledger": "R4, the invocation ledger (every Runner process, gates included)",
            "from": "injected_duplicates_at_seeded_points_release_nothing_twice_and_change_nothing_durable",
            "seeds": seeds
                .iter()
                .map(|row| serde_json::json!({
                    "seed": row["seed"],
                    "registered": row["invocation_ledger"]["registered"],
                    "completed": row["invocation_ledger"]["completed"],
                    "cancelled": row["invocation_ledger"]["cancelled"],
                    "duplicates_counted": row["counted_as_ledger_duplicates"],
                    "balanced": true,
                }))
                .collect::<Vec<_>>(),
        }),
    );
    export(
        "ledgers/slot",
        &serde_json::json!({
            "ledger": "R3, the agent/pool slot table",
            "from": "injected_duplicates_at_seeded_points_release_nothing_twice_and_change_nothing_durable",
            "seeds": seeds
                .iter()
                .map(|row| serde_json::json!({
                    "seed": row["seed"],
                    "pairs_granted": row["invocation_ledger"]["pairs_granted"],
                    "pairs_released": row["invocation_ledger"]["pairs_released"],
                    "holders_at_end": 0,
                }))
                .collect::<Vec<_>>(),
        }),
    );
    export(
        "seam/ST-05",
        &serde_json::json!({
            "row": "ST-05",
            "plan": "alpha, beta, gamma independent; delta after alpha; beta retries; two \
                             reviewers per attempt and per verification",
            "width": 3,
            "rows_also_exercised": ["ST-01", "ST-02", "ST-06"],
            "injected": [
                "ST-02/ST-05: a duplicate end of an invocation already settled (reaches \
                         the ledger when its pipeline is live, else discarded)",
                "ST-05: an end of a running invocation from outside its pipeline",
                "ST-05: a snapshot end the pipeline does not hold",
                "ST-01: a stale completion, from a pipeline already retired with its own \
                         identity, or from one that never existed",
                "ST-02: a registration offered through the injector",
                "ST-06: a completion naming another live pipeline's identity, or a \
                         mismatched attempt or sequence"
            ],
            "seeds": seeds,
            "total_injected": total,
        }),
    );
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Trace {
    Effect(EffectSiteId, HookPhase),
    Appended {
        kind: &'static str,
        pipeline: usize,
        merge: usize,
        converts: Option<&'static str>,
        fast: bool,
    },
}

#[derive(Clone, Default)]
struct Traced(std::sync::Arc<std::sync::Mutex<Vec<Trace>>>);

impl Traced {
    fn push(&self, entry: Trace) {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(entry);
    }

    fn entries(&self) -> Vec<Trace> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

#[derive(Debug, Default)]
struct Arming {
    finished: bool,
    consulted: usize,
    at: Option<usize>,
    report: Option<PathBuf>,
}

#[derive(Clone, Default)]
struct Killer(std::sync::Arc<std::sync::Mutex<Arming>>);

impl Killer {
    fn at(cell: usize) -> Self {
        Self(std::sync::Arc::new(std::sync::Mutex::new(Arming {
            at: Some(cell),
            ..Arming::default()
        })))
    }

    fn reporting(self, path: PathBuf) -> Self {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .report = Some(path);
        self
    }

    fn finished(&self) {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .finished = true;
    }

    fn fires(&self, site: EffectSiteId, phase: HookPhase) -> bool {
        let mut arming = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !arming.finished {
            return false;
        }
        let cell = arming.consulted;
        arming.consulted += 1;
        if arming.at != Some(cell) {
            return false;
        }
        if let Some(path) = &arming.report {
            crate::workspace_manager::fixture::write_file(
                path,
                serde_json::to_string(&(site, phase))
                    .expect("a cell serializes")
                    .as_bytes(),
            );
        }
        true
    }
}

struct TracingEffects {
    inner: crate::workspace_manager::HarnessEffects,
    traced: Traced,
    killer: Killer,
}

impl crate::workspace_manager::EffectHooks for TracingEffects {
    fn phase(&mut self, site: EffectSiteId, phase: HookPhase) -> Injection {
        self.traced.push(Trace::Effect(site, phase));
        let shared = self.inner.phase(site, phase);
        if self.killer.fires(site, phase) {
            return crate::observations::Exported::new(std::sync::Arc::clone(self.inner.harness()))
                .carried(Injection::Kill);
        }
        shared
    }

    fn durability_ledger(&self) -> crate::util::DurabilityLedger {
        self.inner.durability_ledger()
    }

    fn refusal_cause(&self) -> Option<String> {
        self.inner.refusal_cause()
    }
}

struct TracingRunDir {
    inner: crate::rundir::HarnessHooks,
    harness: std::sync::Arc<std::sync::Mutex<crate::topology::effects::HookHarness>>,
    traced: Traced,
    killer: Killer,
}

impl crate::rundir::RunDirHooks for TracingRunDir {
    fn hook(&mut self, site: EffectSiteId, phase: HookPhase) -> Injection {
        self.traced.push(Trace::Effect(site, phase));
        let shared = self.inner.hook(site, phase);
        if self.killer.fires(site, phase) {
            return crate::observations::Exported::new(std::sync::Arc::clone(&self.harness))
                .carried(Injection::Kill);
        }
        shared
    }

    fn durability_ledger(&self) -> crate::util::DurabilityLedger {
        self.inner.durability_ledger()
    }
}

struct Tracing {
    effects: TracingEffects,
    rundir: TracingRunDir,
    events: crate::events::log::HarnessEventHooks,
    container: crate::runner::container::HarnessHooks,
    spawn: crate::runner::HarnessHooks,
    traced: Traced,
    killer: Killer,
    states: Vec<(usize, Option<RunState>)>,
    ledgers: Vec<Brokered>,
}

impl Tracing {
    fn over(env: &crate::engine::topology::scaffold::WideEnv, killer: Killer) -> Self {
        let harness = &env.harness;
        let traced = Traced::default();
        Self {
            effects: TracingEffects {
                inner: crate::workspace_manager::HarnessEffects::new(std::sync::Arc::clone(
                    harness,
                )),
                traced: traced.clone(),
                killer: killer.clone(),
            },
            rundir: TracingRunDir {
                inner: crate::rundir::HarnessHooks::new(std::sync::Arc::clone(harness)),
                harness: std::sync::Arc::clone(harness),
                traced: traced.clone(),
                killer: killer.clone(),
            },
            events: crate::events::log::HarnessEventHooks::new(std::sync::Arc::clone(harness)),
            container: crate::runner::container::HarnessHooks::new(std::sync::Arc::clone(harness)),
            spawn: crate::runner::HarnessHooks::new(std::sync::Arc::clone(harness)),
            traced,
            killer,
            states: Vec::new(),
            ledgers: Vec::new(),
        }
    }
}

impl TopologyHooks for Tracing {
    fn effects(&mut self) -> &mut dyn crate::workspace_manager::EffectHooks {
        &mut self.effects
    }

    fn rundir(&mut self) -> &mut dyn crate::rundir::RunDirHooks {
        &mut self.rundir
    }

    fn events(&mut self) -> &mut dyn crate::events::log::EventHooks {
        &mut self.events
    }

    fn container(&mut self) -> &mut dyn crate::runner::container::ContainerHooks {
        &mut self.container
    }

    fn spawn(&mut self) -> &mut dyn crate::agent::proc::SpawnHooks {
        &mut self.spawn
    }

    fn folded(&mut self, fold: &TopologyFold, events: &[TopologyEvent]) {
        let held = Entitlements::of(fold);
        let body = events.last().map(|event| &event.body);
        let (converts, fast) = match body {
            Some(TopologyEventBody::TaskDispatched { .. }) => (Some("dispatch"), false),
            Some(TopologyEventBody::AttemptStarted { data }) if data.attempt.0 > 1 => {
                (Some("retry"), false)
            }
            Some(TopologyEventBody::MergeVerificationStarted { .. }) => {
                (Some("integration"), false)
            }
            Some(TopologyEventBody::MergePrepared { data })
                if data.disposition == PreparedDisposition::Fast =>
            {
                (Some("integration"), true)
            }
            _ => (None, false),
        };
        self.traced.push(Trace::Appended {
            kind: body.map_or("", TopologyEventBody::kind),
            pipeline: held.pipeline_held(),
            merge: held.merge_held(),
            converts,
            fast,
        });
        self.states.push((events.len(), fold.state().cloned()));
        if fold.finished().is_some() {
            self.killer.finished();
        }
    }

    fn brokered(
        &mut self,
        fold: &TopologyFold,
        events: &[TopologyEvent],
        reservations: &Reservations,
        invocations: &InvocationLedger,
    ) {
        let body = events.last().map(|event| &event.body);
        let slots = invocations.slots();
        self.ledgers.push(Brokered {
            appended: events.len(),
            kind: body.map_or("", TopologyEventBody::kind),
            converts: body.and_then(converted_by),
            held: fold
                .registry()
                .map(|registry| {
                    registry
                        .entries()
                        .iter()
                        .filter_map(|entry| {
                            reservations.held(entry.key).map(|kind| (entry.key, kind))
                        })
                        .collect()
                })
                .unwrap_or_default(),
            outstanding: reservations.outstanding(),
            taken: reservations.taken(),
            converted: reservations.converted(),
            cancelled: reservations.cancelled(),
            granted: slots.granted(),
            released: slots.released(),
            holders: slots
                .holders()
                .into_iter()
                .map(InvocationId::render)
                .collect(),
            queued: slots
                .pending()
                .into_iter()
                .map(InvocationId::render)
                .collect(),
            registered: invocations.registered(),
            pending: invocations
                .pending()
                .into_iter()
                .map(str::to_owned)
                .collect(),
            running: invocations
                .running()
                .into_iter()
                .map(str::to_owned)
                .collect(),
            completed: invocations.completed(),
            ended_cancelled: invocations.cancelled(),
        });
    }
}

struct Brokered {
    appended: usize,
    kind: &'static str,
    converts: Option<(TaskKey, ReservationKind)>,
    held: Vec<(TaskKey, ReservationKind)>,
    outstanding: usize,
    taken: u32,
    converted: u32,
    cancelled: u32,
    granted: u32,
    released: u32,
    holders: Vec<String>,
    queued: Vec<String>,
    registered: usize,
    pending: Vec<String>,
    running: Vec<String>,
    completed: usize,
    ended_cancelled: usize,
}

fn converted_by(body: &TopologyEventBody) -> Option<(TaskKey, ReservationKind)> {
    match body {
        TopologyEventBody::TaskDispatched { data } => Some((data.key, ReservationKind::Dispatch)),
        TopologyEventBody::AttemptStarted { data } if data.attempt.0 > 1 => {
            Some((data.key, ReservationKind::Retry))
        }
        TopologyEventBody::MergeVerificationStarted { data } => {
            Some((data.candidate.key, ReservationKind::Integration))
        }
        TopologyEventBody::MergePrepared { data }
            if data.disposition == PreparedDisposition::Fast =>
        {
            Some((data.key, ReservationKind::Integration))
        }
        TopologyEventBody::MergeRejected { data }
            if matches!(data.disposition, RejectionDisposition::Conflict { .. }) =>
        {
            Some((data.candidate.key, ReservationKind::Integration))
        }
        _ => None,
    }
}

fn ledger_problems(ledgers: &[Brokered]) -> Vec<String> {
    let mut problems = Vec::new();
    for at in ledgers {
        let append = format!("append {} (`{}`)", at.appended, at.kind);
        let outstanding = u32::try_from(at.outstanding).expect("a small count");
        if at.taken != at.converted + at.cancelled + outstanding {
            problems.push(format!(
                "{append}: R13 took {} reservation(s), not {} converted + {} cancelled + \
                         {outstanding} outstanding",
                at.taken, at.converted, at.cancelled
            ));
        }
        let converts: Vec<(TaskKey, ReservationKind)> = at.converts.into_iter().collect();
        if at.held != converts || at.held.len() != at.outstanding {
            problems.push(format!(
                "{append}: R13 holds {:?} of {} outstanding, and the reservation this \
                         append's caller converts after it is {:?}",
                at.held, at.outstanding, at.converts
            ));
        }
        let held = u32::try_from(at.holders.len()).expect("a small count");
        if at.granted != at.released + held {
            problems.push(format!(
                "{append}: R3 granted {} pair(s), not {} released + {held} held by {:?}",
                at.granted, at.released, at.holders
            ));
        }
        let mut slotted = BTreeSet::new();
        for running in &at.running {
            match InvocationId::parse(running) {
                Ok(invocation) if is_slotted(&invocation) => {
                    slotted.insert(running.as_str());
                }
                Ok(_) => {}
                Err(error) => problems.push(format!(
                    "{append}: R4 has `{running}` running, which is not an invocation \
                             identity: {error}"
                )),
            }
        }
        let holders: BTreeSet<&str> = at.holders.iter().map(String::as_str).collect();
        if holders != slotted || holders.len() != at.holders.len() {
            problems.push(format!(
                "{append}: R3 pairs are held by {:?}, and the slotted invocations R4 has \
                         running are {slotted:?}",
                at.holders
            ));
        }
        let queued: BTreeSet<&str> = at.queued.iter().map(String::as_str).collect();
        let pending: BTreeSet<&str> = at.pending.iter().map(String::as_str).collect();
        if queued != pending || queued.len() != at.queued.len() {
            problems.push(format!(
                "{append}: R3 has {:?} waiting for a pair, and R4 has {:?} pending",
                at.queued, at.pending
            ));
        }
        let running: BTreeSet<&str> = at.running.iter().map(String::as_str).collect();
        let partitioned = at.pending.len() + at.running.len() + at.completed + at.ended_cancelled;
        if !pending.is_disjoint(&running) || at.registered != partitioned {
            problems.push(format!(
                "{append}: R4 registers {}, not {} pending + {} running + {} completed + \
                         {} cancelled",
                at.registered,
                at.pending.len(),
                at.running.len(),
                at.completed,
                at.ended_cancelled
            ));
        }
    }
    problems
}

const STAGING: [EffectSiteId; 4] = [
    EffectSiteId::Worktree(WorktreeSite::WriteStagingIntent),
    EffectSiteId::Worktree(WorktreeSite::AddStaging),
    EffectSiteId::Ref(RefSite::PinPrepared),
    EffectSiteId::Object(ObjectSite::ProposalCherryPick),
];

fn provisional_problems(
    trace: &[Trace],
    width: usize,
) -> (Vec<String>, BTreeMap<&'static str, usize>) {
    let mut problems = Vec::new();
    let mut converted: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut last = (0_usize, 0_usize);
    let mut fast_open = false;
    let mut since_append: Vec<EffectSiteId> = Vec::new();
    for (index, entry) in trace.iter().enumerate() {
        match entry {
            Trace::Effect(site, phase) => {
                since_append.push(*site);
                if fast_open && STAGING.contains(site) {
                    problems.push(format!("{index}: {site}/{phase} inside a fast sequence"));
                }
                if fast_open
                    && *site == EffectSiteId::Ref(RefSite::CompareAndSwapIntegration)
                    && last.1 != 1
                {
                    problems.push(format!(
                        "{index}: the fast CAS {phase} without the merge holding: {last:?}"
                    ));
                }
            }
            Trace::Appended {
                kind,
                pipeline,
                merge,
                converts,
                fast,
            } => {
                let now = (*pipeline, *merge);
                if *pipeline > width || *merge > 1 {
                    problems.push(format!("{index}: `{kind}` leaves {now:?} held"));
                }
                if let Some(reservation) = converts {
                    *converted.entry(reservation).or_default() += 1;
                    let expected = if *reservation == "integration" {
                        (last.0 + 1, 1)
                    } else {
                        (last.0 + 1, last.1)
                    };
                    if now != expected || (*reservation == "integration" && last.1 != 0) {
                        problems.push(format!(
                            "{index}: `{kind}` converts a {reservation} reservation from \
                                     {last:?} to {now:?}, not in one step to {expected:?}"
                        ));
                    }
                }
                if *fast {
                    if let Some(site) = since_append.iter().find(|site| STAGING.contains(site)) {
                        problems.push(format!(
                            "{index}: {site} between the fast selection and its \
                                     merge_prepared"
                        ));
                    }
                    fast_open = true;
                }
                if *kind == "task_merged" {
                    if now != (last.0.saturating_sub(1), 0) || last.1 != 1 {
                        problems.push(format!(
                            "{index}: task_merged moves {last:?} to {now:?}, not both \
                                     holdings released at once"
                        ));
                    }
                    fast_open = false;
                }
                last = now;
                since_append.clear();
            }
        }
    }
    (problems, converted)
}

#[test]
fn every_provisional_reservation_converts_at_its_first_append_under_seeded_permutations() {
    let tasks = mixed();
    let mut seeds = Vec::new();
    let mut provisional_rows = Vec::new();
    let mut entitlement_rows = Vec::new();
    let mut in_flight = 0_usize;
    for seed in scheduled_here(0..8, &[0]) {
        let mut wide = Wide::started_with(
            &format!("interleaving-st13-{seed}"),
            &tasks,
            3,
            two_reviewers(),
            holding(&tasks, &BETA_RETRIES),
        );
        {
            let reservations = wide.run.broker_mut().reservations();
            assert!(
                reservations.is_empty() && reservations.taken() == 0,
                "seed {seed}: the provisional ledger is empty at process start"
            );
        }
        let double = std::sync::Arc::clone(&wide.env.runner);
        let mut interleaver = Interleaver::new(&double, seed, Adverse::Seeded);
        let mut hooks = Tracing::over(&wide.env, Killer::default());
        let traced = hooks.traced.clone();
        let pipelines = wide.env.pipelines();
        let progress = wide
            .run
            .run_concurrently(
                &wide.env.seams(),
                &pipelines,
                &mut hooks,
                Some(&mut interleaver),
            )
            .unwrap_or_else(|error| panic!("seed {seed}: {error}"));
        assert_eq!(outcome_of(&progress), RunOutcome::Complete, "seed {seed}");
        let points = std::mem::take(&mut interleaver.points);
        drop(interleaver);
        let durable = wide.env.durable_events();
        for (length, live) in &hooks.states {
            let prefix = durable.get(..*length).expect("a prefix of the durable log");
            let replayed = TopologyFold::replay(wide.env.inputs.clone(), prefix)
                .expect("every prefix replays");
            assert_eq!(
                replayed.state(),
                live.as_ref(),
                "seed {seed}: ST-10 — after append {length} the live fold and the replay of \
                         the prefix differ"
            );
        }
        for (index, point) in points.iter().enumerate() {
            assert_eq!(
                (point.provisional, point.peak <= 1),
                (0, true),
                "seed {seed}, point {index}: a reservation is converted or cancelled \
                         within the coordinator step that took it"
            );
        }
        let trace = traced.entries();
        let (problems, converted) = provisional_problems(&trace, 3);
        assert!(problems.is_empty(), "seed {seed}: {problems:#?}");
        let events = wide.env.durable_events();
        let fast = events
            .iter()
            .filter(|event| {
                matches!(&event.body, TopologyEventBody::MergePrepared { data }
                    if data.disposition == PreparedDisposition::Fast)
            })
            .count();
        assert!(fast >= 1, "seed {seed}: a fast integration ran");
        assert!(
            count(&events, "merge_verification_started") >= 1,
            "seed {seed}: a stale integration ran"
        );
        let appended: usize = converted.values().sum();
        let ledgers = std::mem::take(&mut hooks.ledgers);
        assert_eq!(
            ledgers.iter().map(|at| at.appended).collect::<Vec<_>>(),
            hooks
                .states
                .iter()
                .map(|(length, _)| *length)
                .collect::<Vec<_>>(),
            "seed {seed}: the broker's ledgers are read after every append the fold is"
        );
        let accounting = ledger_problems(&ledgers);
        assert!(accounting.is_empty(), "seed {seed}: {accounting:#?}");
        assert_eq!(
            ledgers.iter().filter(|at| at.outstanding > 0).count(),
            appended,
            "seed {seed}: each first append finds its own reservation still held, and no \
                     other append finds one"
        );
        in_flight += ledgers
            .iter()
            .filter(|at| at.released > 0 && !at.holders.is_empty())
            .count();
        let reservations = wide.run.broker_mut().reservations();
        assert_eq!(
            (
                usize::try_from(reservations.converted()).expect("a small count"),
                reservations.cancelled(),
                reservations.duplicates(),
            ),
            (appended, 0, 0),
            "seed {seed}: one conversion per first append ({converted:?}), none \
                     cancelled or duplicated"
        );
        assert_eq!(
            reservations.taken(),
            reservations.converted() + reservations.cancelled(),
            "seed {seed}"
        );
        assert!(
            reservations.balances() && reservations.is_empty(),
            "seed {seed}: the provisional ledger balances at process end"
        );
        assert_eq!(reservations.peak(), 1, "seed {seed}");
        provisional_rows.push(serde_json::json!({
            "seed": seed,
            "taken": reservations.taken(),
            "converted": reservations.converted(),
            "cancelled": reservations.cancelled(),
            "duplicates": reservations.duplicates(),
            "peak_outstanding": reservations.peak(),
            "outstanding_at_end": reservations.outstanding(),
            "converted_by_kind": converted,
            "outstanding_at_every_quiescent_point": points
                .iter()
                .map(|point| point.provisional)
                .max(),
        }));
        entitlement_rows.push(serde_json::json!({
            "seed": seed,
            "max_parallel": 3,
            "per_append": trace
                .iter()
                .filter_map(|entry| match entry {
                    Trace::Appended {
                        kind,
                        pipeline,
                        merge,
                        ..
                    } => Some(serde_json::json!([kind, pipeline, merge])),
                    Trace::Effect(..) => None,
                })
                .collect::<Vec<_>>(),
        }));
        seeds.push(serde_json::json!({
            "seed": seed,
            "live_fold_equals_replay_after_every_append": hooks.states.len(),
            "appends": trace.iter().filter(|entry| matches!(entry, Trace::Appended { .. })).count(),
            "converted_at_first_append": converted,
            "fast_integrations": fast,
            "quiescent_points": points.len(),
            "peak_outstanding": 1,
        }));
    }
    assert!(
        in_flight > 0,
        "some append found a pair held after another had been released"
    );
    export(
        "ledgers/provisional",
        &serde_json::json!({
            "ledger": "R13, the provisional-reservation ledger",
            "from": "every_provisional_reservation_converts_at_its_first_append_under_seeded_permutations",
            "empty_at_process_start": true,
            "seeds": provisional_rows,
        }),
    );
    export(
        "ledgers/entitlement",
        &serde_json::json!({
            "ledger": "R1/R2, the fold-derived pipeline and merge entitlements, read after \
                               every append as [kind, pipeline held, merge held]",
            "from": "every_provisional_reservation_converts_at_its_first_append_under_seeded_permutations",
            "seeds": entitlement_rows,
        }),
    );
    export(
        "seam/ST-13",
        &serde_json::json!({
            "row": "ST-13",
            "plan": "alpha, beta, gamma independent; delta after alpha; beta retries; two \
                             reviewers per attempt and per verification",
            "width": 3,
            "checked": [
                "at every append: the fold-derived holdings within max_parallel and one \
                         merge; each first append (task_dispatched, a retry's attempt_started, \
                         merge_verification_started, merge_prepared(fast)) moves the derived \
                         holdings by exactly its reservation's entitlements in one step",
                "the fast path: no staging effect between the selection and \
                         merge_prepared(fast); the merge held at the CAS's PreCAS and PostCAS; \
                         both holdings released at once at task_merged",
                "at every quiescent point: no provisional reservation outstanding",
                "at process start: the ledger empty; at process end: taken == converted, \
                         converted == first appends, none cancelled or duplicated, balanced"
            ],
            "seeds": seeds,
        }),
    );
}

#[test]
fn a_chain_projects_at_width_three_as_at_width_one_under_every_seed() {
    let tasks = [
        WideTask::independent("alpha"),
        WideTask::after("beta", &["alpha"]),
        WideTask::after("gamma", &["beta"]),
    ];
    let mut narrow = Wide::started("interleaving-chain-width-one", &tasks, 1);
    let mut hooks = narrow.env.hooks();
    let seams = narrow.env.seams();
    let mut steps = 0_u32;
    loop {
        steps += 1;
        assert!(steps < 200, "the width-1 loop did not finish");
        if let Progress::Finished { outcome, .. } =
            narrow.run.step(&seams, &mut hooks).expect("a step")
        {
            assert_eq!(outcome, RunOutcome::Complete);
            break;
        }
    }
    let run_id = run_id_of(&narrow);
    let one = serde_json::to_string(&canonical(&narrow.env.durable_events(), &run_id))
        .expect("serializes");
    let one_tree = final_tree(&narrow);
    let mut seeds = Vec::new();
    for seed in scheduled_here(0..8, &[0]) {
        let run = seeded_run(
            Shape::of(
                format!("interleaving-chain-{seed}"),
                &tasks,
                WidePlans::default(),
                holding(&tasks, &[]),
            ),
            seed,
            Adverse::Seeded,
            None,
        );
        let progress = run
            .outcome
            .unwrap_or_else(|error| panic!("seed {seed}: {error}"));
        assert_eq!(outcome_of(&progress), RunOutcome::Complete, "seed {seed}");
        let run_id = run_id_of(&run.wide);
        let three = serde_json::to_string(&canonical(&run.wide.env.durable_events(), &run_id))
            .expect("serializes");
        assert_eq!(
            three, one,
            "seed {seed}: the chain's canonical projection at width 3 is byte-identical \
                     to width 1's"
        );
        let tree = final_tree(&run.wide);
        assert_eq!(tree, one_tree, "seed {seed}");
        seeds.push(serde_json::json!({
            "seed": seed,
            "canonical_projection_sha256": digest(&three),
            "final_tree": tree,
        }));
    }
    export(
        "projection/chain",
        &serde_json::json!({
            "plan": "alpha; beta after alpha; gamma after beta",
            "rule": "ST-08: for a chain-dependency plan the canonical projections at \
                             max_parallel = 3 and 1 are byte-identical",
            "width_one": {
                "canonical_projection_sha256": digest(&one),
                "final_tree": one_tree,
            },
            "width_three_seeds": seeds,
            "equal": true,
        }),
    );
}

#[test]
fn independent_tasks_project_identically_per_key_under_every_seed_and_report_it() {
    let tasks = three();
    let mut reference: Option<(BTreeMap<u64, Vec<String>>, String)> = None;
    let mut seeds = Vec::new();
    for seed in scheduled_here(0..16, &[0, 1]) {
        let run = seeded_run(
            Shape::of(
                format!("interleaving-st08-{seed}"),
                &tasks,
                two_reviewers(),
                holding(&tasks, &[]),
            ),
            seed ^ 0x0008_5EED,
            Adverse::Seeded,
            None,
        );
        let progress = run
            .outcome
            .unwrap_or_else(|error| panic!("seed {seed}: {error}"));
        assert_eq!(outcome_of(&progress), RunOutcome::Complete, "seed {seed}");
        let events = run.wide.env.durable_events();
        let kinds = kinds_of(&events);
        assert!(
            position(&kinds, "task_dispatched", 2) < position(&kinds, "candidate_prepared", 0),
            "seed {seed}: every independent task dispatched before any completion: \
                     {kinds:?}"
        );
        let bases: BTreeSet<String> = dispatched_bases(&events).into_values().collect();
        assert_eq!(
            bases.len(),
            1,
            "seed {seed}: one base for the independent tasks"
        );
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
            "seed {seed}: ST-03 — the queue integrated in task_candidate_created order"
        );
        let run_id = run_id_of(&run.wide);
        let keyed = attempt_level(&events, &run_id);
        let problems = attempt_level_problems(&keyed, &[0, 1, 2]);
        assert!(problems.is_empty(), "seed {seed}: {problems:#?}");
        let tree = final_tree(&run.wide);
        let per_key: BTreeMap<String, String> = keyed
            .iter()
            .map(|(key, values)| (key.to_string(), digest(&values.join("\n"))))
            .collect();
        seeds.push(serde_json::json!({
            "seed": seed ^ 0x0008_5EED,
            "per_key_projection_sha256": per_key,
            "final_tree": tree,
            "integration_order": integrated,
        }));
        let observed = (keyed, tree);
        match &reference {
            None => reference = Some(observed),
            Some(expected) => assert_eq!(
                &observed, expected,
                "seed {seed}: a key's projection or the final tree differs from the first \
                         seed's"
            ),
        }
    }
    export(
        "projection/independent",
        &serde_json::json!({
            "plan": "alpha, beta, gamma independent; two reviewers per attempt and per \
                             verification",
            "rule": "ST-08: every independent task dispatched before any completion \
                             (equal bases); per key, the already-defined projection fields (the \
                             attempt-level events task_dispatched, attempt_started, \
                             attempt_finished, candidate_prepared, task_candidate_created) and the \
                             final integration tree OID equal under every seeded permutation",
            "seeds": seeds,
            "equal": true,
        }),
    );
}

fn reasking(tasks: &[WideTask], failing: &[(u32, u32)]) -> RecordingRunner {
    let base = wide_responder(tasks, failing);
    let runner = RecordingRunner::new().answering(Box::new(
        move |request: &crate::runner::RunnerRequest| {
            let first_pass = matches!(
                request.invocation,
                InvocationId::Attempt {
                    role: AttemptRole::ReviewPass(_),
                    ..
                } | InvocationId::Sequence {
                    role: SequenceRole::ReviewPass(_),
                    ..
                }
            );
            if matches!(request.role, ExecutionRole::Review) && first_pass {
                return Ok(exited(0, "the reviewer answered in prose\n".to_owned()));
            }
            base(request)
        },
    ));
    runner.hold();
    runner
}

struct Limited {
    name: &'static str,
    pool: Option<String>,
    pools: Vec<(&'static str, Option<String>)>,
    reasks: bool,
}

fn reduced() -> Vec<Limited> {
    vec![
        Limited {
            name: "two agents sharing one pool",
            pool: Some("shared-pool".to_owned()),
            pools: Vec::new(),
            reasks: true,
        },
        Limited {
            name: "each agent in its own pool",
            pool: None,
            pools: vec![
                (AGENT, Some("pool-claude".to_owned())),
                (REVIEW_AGENT, Some("pool-copilot".to_owned())),
            ],
            reasks: true,
        },
        Limited {
            name: "agents without pools",
            pool: None,
            pools: Vec::new(),
            reasks: false,
        },
        Limited {
            name: "an agent without a pool beside a pooled one",
            pool: None,
            pools: vec![
                (AGENT, None),
                (REVIEW_AGENT, Some("pool-copilot".to_owned())),
            ],
            reasks: false,
        },
    ]
}

#[test]
fn reduced_limits_and_adverse_completion_orders_reach_run_finished_within_the_step_bound() {
    let orders = [
        (Adverse::Seeded, 0xDEAD_0001_u64),
        (Adverse::Seeded, 0xDEAD_0002),
        (Adverse::Newest, 0),
        (Adverse::Oldest, 0),
        (Adverse::AgentsLast, 0),
    ];
    let configurations = reduced();
    let on_windows: [u64; 4] = [0, 2, 3, 4];
    assert_eq!(
        configurations.len(),
        on_windows.len(),
        "one order per configuration on Windows"
    );
    let mut runs = Vec::new();
    let mut beside = 0_usize;
    let mut waited = 0_usize;
    for (limited, windows) in configurations.into_iter().zip(on_windows) {
        let positions = scheduled_here(0..5, &[windows]);
        for (position, (order, seed)) in (0_u64..).zip(orders) {
            if !positions.contains(&position) {
                continue;
            }
            let index = runs.len();
            let limits = Limited {
                name: limited.name,
                pool: limited.pool.clone(),
                pools: limited.pools.clone(),
                reasks: limited.reasks,
            };
            let (sent, received) = std::sync::mpsc::channel();
            bounded("a reduced-limit run", move || {
                let _ = sent.send(reduced_limit_run(index, &limits, order, seed));
            });
            let (row, verifying_beside_attempts, waited_at_an_append) =
                received.recv().expect("the bounded run reported its row");
            beside += usize::from(verifying_beside_attempts);
            waited += usize::from(waited_at_an_append);
            runs.push(row);
        }
    }
    assert!(
        beside > 0,
        "a verification ran beside attempts in at least one run"
    );
    assert!(
        waited > 0,
        "a request waited for its pair at some append in at least one run"
    );
    export(
        "seam/deadlock-freedom",
        &serde_json::json!({
            "rule": "deadlock-freedom under adverse completion orders and reduced limits: \
                             every run reaches run_finished within the step bound; no pair over a \
                             limit at any quiescent point",
            "plan": "alpha, beta, gamma independent; delta after alpha; beta retries; two \
                             reviewers per attempt and per verification",
            "width": 3,
            "step_bound": STEPS,
            "runs": runs,
        }),
    );
}

fn reduced_limit_run(
    index: usize,
    limited: &Limited,
    order: Adverse,
    seed: u64,
) -> (serde_json::Value, bool, bool) {
    let tasks = mixed();
    let tag = format!("interleaving-deadlock-{index}");
    let plans = WidePlans {
        pool: limited.pool.clone(),
        pools: limited.pools.clone(),
        ..two_reviewers()
    };
    let judge = WidePlans {
        pool: limited.pool.clone(),
        pools: limited.pools.clone(),
        ..two_reviewers()
    };
    let runner = if limited.reasks {
        reasking(&tasks, &BETA_RETRIES)
    } else {
        holding(&tasks, &BETA_RETRIES)
    };
    let mut wide = Wide::started_with(&tag, &tasks, 3, plans, runner);
    if limited.reasks {
        wide.env.adapters =
            std::sync::Arc::new(crate::engine::topology::scaffold::ScaffoldAdapters::echoing());
    }
    let double = std::sync::Arc::clone(&wide.env.runner);
    let mut interleaver = Interleaver::new(&double, seed, order);
    let mut hooks = Tracing::over(&wide.env, Killer::default());
    let pipelines = wide.env.pipelines_limited(SlotLimitsOf::Exactly(1, 1));
    let outcome = wide.run.run_concurrently(
        &wide.env.seams(),
        &pipelines,
        &mut hooks,
        Some(&mut interleaver),
    );
    let what = format!("{} / {} {seed:#x}", limited.name, order.name());
    let progress = outcome.unwrap_or_else(|error| panic!("{what}: {error}"));
    assert_eq!(outcome_of(&progress), RunOutcome::Complete, "{what}");
    let ledgers = std::mem::take(&mut hooks.ledgers);
    assert_eq!(
        ledgers.iter().map(|at| at.appended).collect::<Vec<_>>(),
        hooks
            .states
            .iter()
            .map(|(length, _)| *length)
            .collect::<Vec<_>>(),
        "{what}: the broker's ledgers are read after every append the fold is"
    );
    let accounting = ledger_problems(&ledgers);
    assert!(accounting.is_empty(), "{what}: {accounting:#?}");
    let waited = ledgers.iter().any(|at| !at.queued.is_empty());
    let points = std::mem::take(&mut interleaver.points);
    drop(interleaver);
    assert!(
        points.len() <= STEPS,
        "{what}: {} scheduler steps against a bound of {STEPS}",
        points.len()
    );
    let ran = wide.env.runner.ran();
    let over = held_over_limits(&points, &ran, &judge, 1, 1);
    assert!(over.is_empty(), "{what}: {over:#?}");
    let reasks = ran
        .iter()
        .filter(|entry| {
            matches!(
                entry.invocation,
                InvocationId::Attempt {
                    role: AttemptRole::ReviewReask(_),
                    ..
                } | InvocationId::Sequence {
                    role: SequenceRole::ReviewReask(_),
                    ..
                }
            )
        })
        .count();
    assert_eq!(reasks > 0, limited.reasks, "{what}: {reasks} re-asks");
    let verifying_beside_attempts = points.iter().any(|point| {
        point
            .invoking
            .iter()
            .any(|held| matches!(held, InvocationId::Sequence { .. }))
            && point
                .invoking
                .iter()
                .any(|held| matches!(held, InvocationId::Attempt { .. }))
    });
    let slotted = ran
        .iter()
        .filter(|entry| is_slotted(&entry.invocation))
        .count();
    let ledger = wide.run.broker_mut().invocations();
    assert!(ledger.balances(), "{what}");
    assert_eq!(
        (
            usize::try_from(ledger.slots().granted()).expect("a small count"),
            usize::try_from(ledger.slots().released()).expect("a small count"),
        ),
        (slotted, slotted),
        "{what}: every pair granted was released once"
    );
    (
        serde_json::json!({
            "limits": limited.name,
            "per_agent": 1,
            "per_pool": 1,
            "reasks": reasks,
            "order": order.name(),
            "seed": seed,
            "steps": points.len(),
            "processes": ran.len(),
            "pairs_granted_and_released": slotted,
            "a_verification_beside_attempts": verifying_beside_attempts,
            "outcome": "Complete",
        }),
        verifying_beside_attempts,
        waited,
    )
}

#[test]
fn a_scheduler_that_stops_releasing_ends_the_run_as_stuck_rather_than_hanging() {
    bounded("the run whose scheduler stops releasing", stuck_run);
}

fn stuck_run() {
    let tasks = three();
    let mut wide = Wide::started_with(
        "interleaving-stuck",
        &tasks,
        3,
        WidePlans::default(),
        holding(&tasks, &[]),
    );
    let double = std::sync::Arc::clone(&wide.env.runner);
    let mut releases = 0_u32;
    let mut scheduler = Scheduler::scripted(
        &double,
        Box::new(|view: &Quiescent<'_>| {
            if releases < 3 {
                releases += 1;
                released(view, |_| true)
            } else {
                Some(Release::Nothing)
            }
        }),
    );
    let error = drive(&mut wide, Some(&mut scheduler))
        .expect_err("a run nothing is released in ends with an error");
    drop(scheduler);
    let message = error.to_string();
    assert!(
        message.contains("released nothing") && message.contains("resumable"),
        "{message}"
    );
    let endings = wide.env.runner.endings();
    assert!(
        endings
            .iter()
            .any(|(_, ending)| *ending == crate::engine::topology::scaffold::Ending::Cancelled),
        "the invocations held when the run stuck were cancelled: {endings:?}"
    );
    let kinds = kinds_of(&wide.env.durable_events());
    assert!(!kinds.contains(&"run_finished"), "{kinds:?}");
    assert!(!wide.run.fold().is_poisoned());
    let ledger = wide.run.broker_mut().invocations();
    assert!(
        ledger.balances() && ledger.running().is_empty() && ledger.pending().is_empty(),
        "every registration settled: running {:?}, pending {:?}",
        ledger.running(),
        ledger.pending()
    );
    export(
        "seam/deadlock-freedom",
        &serde_json::json!({
            "rule": "a stuck run is an error, never a hang",
            "released_before_stopping": 3,
            "error": message,
            "run_finished_appended": false,
            "ledger_balanced": true,
        }),
    );
}

#[test]
fn runtime_pool_same_agent_and_pool_with_opposing_limits_serialize_on_the_binding_limit() {
    let tasks = three();
    let mut rows = Vec::new();
    for (per_agent, per_pool, binding, widest) in
        [(1, 2, "agent", 1), (2, 1, "pool", 1), (2, 2, "neither", 2)]
    {
        for seed in scheduled_here(0..3, &[0]) {
            let plans = WidePlans {
                pool: Some("pool-p".to_owned()),
                ..WidePlans::default()
            };
            let judge = WidePlans {
                pool: Some("pool-p".to_owned()),
                ..WidePlans::default()
            };
            let run = seeded_run(
                Shape::of(
                    format!("interleaving-pool-opposing-{binding}-{seed}"),
                    &tasks,
                    plans,
                    holding(&tasks, &[]),
                )
                .limited(SlotLimitsOf::Exactly(per_agent, per_pool)),
                seed,
                Adverse::Seeded,
                None,
            );
            let what = format!("({per_agent}, {per_pool}) seed {seed}");
            let progress = run
                .outcome
                .unwrap_or_else(|error| panic!("{what}: {error}"));
            assert_eq!(outcome_of(&progress), RunOutcome::Complete, "{what}");
            let ran = run.wide.env.runner.ran();
            let over = held_over_limits(
                &run.points,
                &ran,
                &judge,
                usize::try_from(per_agent).expect("small"),
                usize::try_from(per_pool).expect("small"),
            );
            assert!(over.is_empty(), "{what}: {over:#?}");
            let most = most_held_of(&run.points, &ran, AGENT);
            assert_eq!(
                most, widest,
                "{what}: the {binding} limit binds: at most {widest} process(es) of \
                         `{AGENT}` in `pool-p` ran at once"
            );
            rows.push(serde_json::json!({
                "per_agent": per_agent,
                "per_pool": per_pool,
                "binding": binding,
                "seed": seed,
                "most_held_at_once": most,
            }));
        }
    }
    export(
        "seam/runtime-pool",
        &serde_json::json!({
            "test": "same agent and pool with opposing limits serialize on the binding limit",
            "through": "the coordinator on its Tokio blocking pool at width 3, the \
                                scheduler observing the processes held at every quiescent point",
            "rows": rows,
        }),
    );
}

#[test]
fn runtime_pool_two_agents_with_their_own_pools_run_in_parallel() {
    let tasks = three();
    let own = || WidePlans {
        pools: vec![
            (AGENT, Some("pool-claude".to_owned())),
            (REVIEW_AGENT, Some("pool-copilot".to_owned())),
        ],
        ..two_reviewers()
    };
    let mut rows = Vec::new();
    for seed in scheduled_here(0..4, &[0]) {
        let run = seeded_run(
            Shape::of(
                format!("interleaving-pool-own-{seed}"),
                &tasks,
                own(),
                holding(&tasks, &[]),
            )
            .limited(SlotLimitsOf::Exactly(1, 1)),
            seed,
            Adverse::Seeded,
            None,
        );
        let progress = run
            .outcome
            .unwrap_or_else(|error| panic!("seed {seed}: {error}"));
        assert_eq!(outcome_of(&progress), RunOutcome::Complete, "seed {seed}");
        let ran = run.wide.env.runner.ran();
        let over = held_over_limits(&run.points, &ran, &own(), 1, 1);
        assert!(over.is_empty(), "seed {seed}: {over:#?}");
        assert!(
            both_held(&run.points, &ran, AGENT, REVIEW_AGENT),
            "seed {seed}: a `{AGENT}` process and a `{REVIEW_AGENT}` process ran at once, \
                     each in its own pool at one slot per agent and per pool"
        );
        rows.push(serde_json::json!({
            "seed": seed,
            "both_agents_held_at_once": true,
            "most_held": {
                AGENT: most_held_of(&run.points, &ran, AGENT),
                REVIEW_AGENT: most_held_of(&run.points, &ran, REVIEW_AGENT),
            },
        }));
    }
    export(
        "seam/runtime-pool",
        &serde_json::json!({
            "test": "two agents with their own pools run in parallel",
            "limits": {"per_agent": 1, "per_pool": 1},
            "rows": rows,
        }),
    );
}

#[test]
fn runtime_pool_an_agent_without_a_pool_takes_its_agent_slot_only() {
    let tasks = three();
    let mut rows = Vec::new();
    for (claude_pool, widest) in [(None, 2), (Some("pool-claude"), 1)] {
        let plans = || WidePlans {
            pools: vec![
                (AGENT, claude_pool.map(str::to_owned)),
                (REVIEW_AGENT, Some("pool-copilot".to_owned())),
            ],
            ..two_reviewers()
        };
        for seed in scheduled_here(0..3, &[0]) {
            let run = seeded_run(
                Shape::of(
                    format!(
                        "interleaving-pool-unpooled-{}-{seed}",
                        claude_pool.unwrap_or("none")
                    ),
                    &tasks,
                    plans(),
                    holding(&tasks, &[]),
                )
                .limited(SlotLimitsOf::Exactly(2, 1)),
                seed,
                Adverse::Seeded,
                None,
            );
            let what = format!("`{AGENT}` pool {claude_pool:?} seed {seed}");
            let progress = run
                .outcome
                .unwrap_or_else(|error| panic!("{what}: {error}"));
            assert_eq!(outcome_of(&progress), RunOutcome::Complete, "{what}");
            let ran = run.wide.env.runner.ran();
            let over = held_over_limits(&run.points, &ran, &plans(), 2, 1);
            assert!(over.is_empty(), "{what}: {over:#?}");
            assert_eq!(
                most_held_of(&run.points, &ran, AGENT),
                widest,
                "{what}: at two slots per agent and one per pool, `{AGENT}` ran {widest} \
                         at once"
            );
            assert!(
                most_held_of(&run.points, &ran, REVIEW_AGENT) <= 1,
                "{what}: the pooled agent never exceeded its pool's one slot"
            );
            let recorded: BTreeSet<Option<String>> = run
                .wide
                .env
                .durable_events()
                .iter()
                .filter_map(|event| match &event.body {
                    TopologyEventBody::AttemptStarted { data } => Some(data.pool.clone()),
                    _ => None,
                })
                .collect();
            assert_eq!(
                recorded,
                BTreeSet::from([claude_pool.map(str::to_owned)]),
                "{what}: every attempt_started records the worker's pool"
            );
            rows.push(serde_json::json!({
                "agent_pool": claude_pool,
                "seed": seed,
                "most_held_of_the_agent": widest,
            }));
        }
    }
    export(
        "seam/runtime-pool",
        &serde_json::json!({
            "test": "an agent without a pool takes its agent slot only",
            "limits": {"per_agent": 2, "per_pool": 1},
            "rows": rows,
        }),
    );
}

#[test]
fn runtime_pool_gate_invocations_register_without_slots_while_every_slot_is_held() {
    let tasks = three();
    let mut several = false;
    let mut rows = Vec::new();
    for seed in scheduled_here(0..4, &[1]) {
        let plans = WidePlans {
            pool: Some("pool-p".to_owned()),
            ..WidePlans::default()
        };
        let mut run = seeded_run(
            Shape::of(
                format!("interleaving-pool-gates-{seed}"),
                &tasks,
                plans,
                holding(&tasks, &[]),
            )
            .limited(SlotLimitsOf::Exactly(1, 1)),
            seed,
            Adverse::Seeded,
            None,
        );
        let progress = run
            .outcome
            .as_ref()
            .unwrap_or_else(|error| panic!("seed {seed}: {error}"));
        assert_eq!(outcome_of(progress), RunOutcome::Complete, "seed {seed}");
        let gates_beside_the_slot: Vec<usize> = run
            .points
            .iter()
            .filter(|point| point.invoking.iter().any(is_slotted))
            .map(|point| {
                point
                    .invoking
                    .iter()
                    .filter(|held| !is_slotted(held))
                    .count()
            })
            .collect();
        assert!(
            gates_beside_the_slot.iter().any(|gates| *gates >= 1),
            "seed {seed}: a gate ran while the one agent slot was held"
        );
        several |= gates_beside_the_slot.iter().any(|gates| *gates >= 2);
        let ran = run.wide.env.runner.ran();
        let slotted = ran
            .iter()
            .filter(|entry| is_slotted(&entry.invocation))
            .count();
        let gates = ran.len() - slotted;
        let ledger = run.wide.run.broker_mut().invocations();
        assert_eq!(
            (
                ledger.registered(),
                usize::try_from(ledger.slots().granted()).expect("a small count"),
                usize::try_from(ledger.slots().released()).expect("a small count"),
            ),
            (ran.len(), slotted, slotted),
            "seed {seed}: every process registered; only the agent processes took and \
                     released a pair, the {gates} gates none"
        );
        assert!(ledger.balances(), "seed {seed}");
        rows.push(serde_json::json!({
            "seed": seed,
            "registered": ran.len(),
            "gates": gates,
            "pairs": slotted,
            "most_gates_beside_a_held_slot": gates_beside_the_slot.iter().max(),
        }));
    }
    assert!(
        several,
        "two gates ran at once beside a held slot in some seed"
    );
    export(
        "seam/runtime-pool",
        &serde_json::json!({
            "test": "gate invocations register without slots while every slot is held",
            "limits": {"per_agent": 1, "per_pool": 1},
            "rows": rows,
        }),
    );
}

#[test]
fn runtime_pool_agent_probes_take_and_release_their_pair_at_preflight_before_admission() {
    let tasks = three();
    let wide = Wide::durable(
        "interleaving-pool-probes",
        &tasks,
        3,
        WidePlans::default(),
        holding(&tasks, &[]),
    );
    let agents = wide
        .run
        .fold()
        .started()
        .expect("started")
        .probed_agents
        .clone();
    let Wide { run, env } = wide;
    drop(run);
    let probes = RecordingRunner::new();
    let adapters = crate::engine::topology::scaffold::ScaffoldAdapters::probing();
    let preflight = crate::engine::topology::preflight::RunPreflight::new(
        &probes,
        &adapters,
        crate::gates::ShellKind::native(),
        &env.fixture.base,
        agents.clone(),
    );
    let runtime = crate::runner::container::FakeRuntime::new(
        crate::runner::container::runtime::ContainerTrace::default(),
    );
    let liveness = crate::runner::container::FakeOwnerLiveness::new();
    let mut hooks = env.hooks();
    let (_, mut resumed) = env
        .try_resume_over(
            "inc-2",
            holding(&tasks, &[]),
            crate::engine::topology::select::Ceiling::unlimited(),
            &crate::engine::topology::scaffold::ResumingOver {
                runtime: &runtime,
                liveness: &liveness,
                preflight: &preflight,
                awaits_release: true,
            },
            &mut hooks,
        )
        .unwrap_or_else(|failed| panic!("the resume certifies: {}", failed.0));
    let probed: Vec<(InvocationId, Option<String>)> = probes
        .ran()
        .into_iter()
        .map(|ran| {
            (
                ran.invocation,
                ran.agent.map(|agent| agent.as_str().to_owned()),
            )
        })
        .collect();
    let mut expected = vec![(
        crate::engine::topology::identity::PreflightIdentities::shell(0).expect("the shell probe"),
        None,
    )];
    for agent in &agents {
        expected.push((
            crate::engine::topology::identity::PreflightIdentities::agent(agent, 0)
                .expect("an agent probe"),
            Some(agent.clone()),
        ));
    }
    assert_eq!(
        probed, expected,
        "the pre-flight ran the shell probe, then one probe per recorded agent, each \
                 under its own probe-role InvocationId"
    );
    assert!(
        probes
            .endings()
            .iter()
            .all(|(_, ending)| *ending == crate::engine::topology::scaffold::Ending::Completed),
        "every probe ended"
    );
    assert_eq!(
        preflight.settlements(),
        (expected.len(), 0),
        "each probe's registration was settled once, as completed, releasing the agent \
                 probes' pairs"
    );
    assert!(
        preflight.ledgers_balance() && preflight.running().is_empty(),
        "the pre-flight's ledger balances before admission"
    );
    broker_is_empty(&mut resumed.run).expect("the coordinator's broker starts empty");
    let double = std::sync::Arc::clone(&resumed.env.runner);
    let mut interleaver = Interleaver::new(&double, 5, Adverse::Seeded);
    let mut hooks = resumed.env.hooks();
    let pipelines = resumed.env.pipelines_limited(SlotLimitsOf::Exactly(1, 1));
    let progress = resumed
        .run
        .run_concurrently(
            &resumed.env.seams(),
            &pipelines,
            &mut hooks,
            Some(&mut interleaver),
        )
        .expect("the resumed width-3 run completes");
    drop(interleaver);
    assert_eq!(outcome_of(&progress), RunOutcome::Complete);
    assert!(resumed.run.invocations_balance());
    export(
        "seam/runtime-pool",
        &serde_json::json!({
            "test": "agent probes take and release their pair at pre-flight; the shell \
                             probe registers without one",
            "through": "the frozen recovery order's pre-flight (RunPreflight over the \
                                test double), then the coordinator at width 3 on its Tokio pool",
            "probes": probed
                .iter()
                .map(|(invocation, agent)| serde_json::json!({
                    "invocation": invocation.render(),
                    "agent": agent,
                    "slotted": is_slotted(invocation),
                }))
                .collect::<Vec<_>>(),
            "preflight_settlements": {"completed": expected.len(), "cancelled": 0},
            "broker_empty_at_coordinator_start": true,
        }),
    );
}

#[test]
fn seeded_contained_runs_never_reuse_a_container_name_intent_or_view() {
    let tasks = three();
    let mut seeds = Vec::new();
    for seed in scheduled_here(0..3, &[0]) {
        let mut wide = Wide::durable_contained(
            &format!("interleaving-contained-{seed}"),
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
        let identity = wide.env.identity(INC_A);
        let mut inventory: Vec<InventoryAt> = Vec::new();
        let mut interleaver = Interleaver::new(&double, seed, Adverse::Seeded).watching(Box::new(
            |view: &Quiescent<'_>| {
                let mut expected: Vec<String> = view
                    .invoking
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
                expected.sort();
                inventory.push((
                    expected,
                    running_in(&host),
                    intents_under(&root),
                    views_under(&root),
                ));
            },
        ));
        let mut hooks = wide.env.hooks();
        let pipelines = wide.env.pipelines_over(contained.clone());
        let progress = wide
            .run
            .run_concurrently(
                &wide.env.seams_over(&*contained),
                &pipelines,
                &mut hooks,
                Some(&mut interleaver),
            )
            .unwrap_or_else(|error| panic!("seed {seed}: {error}"));
        drop(interleaver);
        assert_eq!(outcome_of(&progress), RunOutcome::Complete, "seed {seed}");
        for (expected, running, intents, views) in &inventory {
            assert_eq!(
                (running, intents, views),
                (expected, expected, expected),
                "seed {seed}: at every quiescent point the containers running, the \
                         intents and the views are exactly those of the invocations inside the \
                         runner, one each"
            );
        }
        let journal = host.journal();
        let mut created: Vec<&str> = journal
            .iter()
            .filter(|entry| entry.op == crate::runner::container::runtime::RuntimeOp::Create)
            .map(|entry| entry.target.as_str())
            .collect();
        let mut removed: Vec<&str> = journal
            .iter()
            .filter(|entry| entry.op == crate::runner::container::runtime::RuntimeOp::Remove)
            .map(|entry| entry.target.as_str())
            .collect();
        let total = created.len();
        created.sort_unstable();
        created.dedup();
        removed.sort_unstable();
        assert_eq!(
            created.len(),
            total,
            "seed {seed}: no container name created twice"
        );
        assert_eq!(created, removed, "seed {seed}: each container removed once");
        let ran: Vec<InvocationId> = double
            .ran()
            .iter()
            .map(|ran| ran.invocation.clone())
            .collect();
        assert_eq!(
            created,
            container_names_of(&wide, INC_A, &ran),
            "seed {seed}: each container is its own invocation's"
        );
        assert!(
            host.container_names().is_empty()
                && intents_under(&root).is_empty()
                && views_under(&root).is_empty(),
            "seed {seed}: nothing left"
        );
        assert!(wide.run.invocations_balance(), "seed {seed}");
        seeds.push(serde_json::json!({
            "seed": seed,
            "containers": total,
            "quiescent_points": inventory.len(),
            "widest": inventory.iter().map(|(expected, ..)| expected.len()).max(),
        }));
    }
    export(
        "ledgers/container",
        &serde_json::json!({
            "rows": ["ST-04 (container names, intent paths)", "R19", "R26"],
            "width": 3,
            "seeds": seeds,
            "checked": [
                "at every quiescent point: running containers == intents == views == the \
                         invocations inside the runner, by name",
                "no container name created twice; each removed once; each its own \
                         invocation's; nothing left at the end"
            ],
        }),
    );
}

fn in_flight_at(events: &[TopologyEvent], at: usize) -> (usize, usize) {
    let mut attempts: BTreeSet<(u32, u32, u32)> = BTreeSet::new();
    let mut sequences: BTreeSet<u32> = BTreeSet::new();
    for event in events.iter().take(at) {
        match &event.body {
            TopologyEventBody::AttemptStarted { data } => {
                attempts.insert((data.key.0, data.generation.0, data.attempt.0));
            }
            TopologyEventBody::AttemptFinished { data } => {
                attempts.remove(&(data.key.0, data.generation.0, data.attempt.0));
            }
            TopologyEventBody::CandidatePrepared { data } => {
                attempts.remove(&(data.key.0, data.generation.0, data.attempt.attempt));
            }
            TopologyEventBody::AttemptInterrupted { data } => {
                attempts.remove(&(data.key.0, data.generation.0, data.attempt.0));
            }
            TopologyEventBody::MergeVerificationStarted { data } => {
                sequences.insert(data.sequence.0);
            }
            TopologyEventBody::MergePrepared { data } => {
                sequences.remove(&data.sequence.0);
            }
            TopologyEventBody::MergeRejected { data } => {
                sequences.remove(&data.sequence.0);
            }
            TopologyEventBody::MergeVerificationUnavailable { data } => {
                sequences.remove(&data.sequence.0);
            }
            TopologyEventBody::MergeVerificationInterrupted { data } => {
                sequences.remove(&data.sequence.0);
            }
            _ => {}
        }
    }
    (attempts.len(), sequences.len())
}

const ADMISSIONS: [&str; 3] = [
    "task_dispatched",
    "attempt_started",
    "merge_verification_started",
];

#[test]
fn a_halt_under_every_seed_interrupts_exactly_what_is_in_flight_and_ends_halted() {
    let mut seeds = Vec::new();
    let mut interrupted_total = 0_usize;
    for seed in scheduled_here(0..8, &[0]) {
        let mut wide = durable_halting_on_gamma(&format!("interleaving-st17-halt-{seed}"));
        let double = std::sync::Arc::clone(&wide.env.runner);
        let mut interleaver = Interleaver::new(&double, seed, Adverse::Seeded);
        let mut hooks = wide.env.hooks();
        let pipelines = wide.env.pipelines();
        let progress = wide
            .run
            .run_concurrently(
                &wide.env.seams(),
                &pipelines,
                &mut hooks,
                Some(&mut interleaver),
            )
            .unwrap_or_else(|error| panic!("seed {seed}: {error}"));
        drop(interleaver);
        assert_eq!(outcome_of(&progress), RunOutcome::Halted, "seed {seed}");
        let events = wide.env.durable_events();
        let kinds = kinds_of(&events);
        let halt = position(&kinds, "question_answered", 0);
        let (attempts, verifications) = in_flight_at(&events, halt + 1);
        let after: Vec<&str> = kinds.iter().skip(halt + 1).copied().collect();
        assert!(
            !after.iter().any(|kind| ADMISSIONS.contains(kind)),
            "seed {seed}: nothing admitted after the halting settlement: {after:?}"
        );
        assert_eq!(
            (
                after
                    .iter()
                    .filter(|kind| **kind == "attempt_interrupted")
                    .count(),
                after
                    .iter()
                    .filter(|kind| **kind == "merge_verification_interrupted")
                    .count(),
            ),
            (attempts, verifications),
            "seed {seed}: one interrupted terminal per attempt and verification in flight \
                     at the halt: {kinds:?}"
        );
        assert_eq!(kinds.last(), Some(&"run_finished"), "seed {seed}");
        let (found, _) = aliases(&events, &wide.env.runner.ran());
        assert!(found.is_empty(), "seed {seed}: {found:#?}");
        assert!(
            wide.env.runner.endings().iter().all(|(_, ending)| matches!(
                ending,
                crate::engine::topology::scaffold::Ending::Completed
                    | crate::engine::topology::scaffold::Ending::Cancelled
            )),
            "seed {seed}: every process completed or was cancelled after it started"
        );
        let reservations = wide.run.broker_mut().reservations();
        assert!(
            reservations.balances() && reservations.is_empty(),
            "seed {seed}"
        );
        assert!(wide.run.invocations_balance(), "seed {seed}");
        replay_equals_live(&wide);
        interrupted_total += attempts + verifications;
        seeds.push(serde_json::json!({
            "seed": seed,
            "in_flight_at_the_halt": {"attempts": attempts, "verifications": verifications},
            "events": kinds.len(),
        }));
    }
    assert!(
        interrupted_total > 0,
        "some seed halted with work in flight"
    );
    export(
        "seam/ST-17",
        &serde_json::json!({
            "row": "ST-17 (halt)",
            "plan": "alpha, beta, gamma independent; gamma's worker asks and the answer \
                             declines, which halts the run",
            "width": 3,
            "seeds": seeds,
        }),
    );
}

#[test]
fn a_budget_stop_under_every_seed_drains_without_cancelling_and_ends_budget_exceeded() {
    let tasks = four();
    let mut seeds = Vec::new();
    let mut drained_total = 0_usize;
    for seed in scheduled_here(0..6, &[0]) {
        let mut wide = Wide::started_under(
            &format!("interleaving-st17-budget-{seed}"),
            &tasks,
            3,
            WidePlans::default(),
            holding(&tasks, &[]),
            TIGHT,
        );
        let double = std::sync::Arc::clone(&wide.env.runner);
        let mut interleaver = Interleaver::new(&double, seed, Adverse::Seeded);
        let mut hooks = wide.env.hooks();
        let pipelines = wide.env.pipelines();
        let progress = wide
            .run
            .run_concurrently(
                &wide.env.seams(),
                &pipelines,
                &mut hooks,
                Some(&mut interleaver),
            )
            .unwrap_or_else(|error| panic!("seed {seed}: {error}"));
        drop(interleaver);
        assert_eq!(
            outcome_of(&progress),
            RunOutcome::BudgetExceeded,
            "seed {seed}"
        );
        let events = wide.env.durable_events();
        let kinds = kinds_of(&events);
        let stop = position(&kinds, "budget_exceeded", 0);
        assert_eq!(kinds.last(), Some(&"run_finished"), "seed {seed}");
        let after: Vec<&str> = kinds.iter().skip(stop + 1).copied().collect();
        assert!(
            !after.iter().any(|kind| ADMISSIONS.contains(kind)),
            "seed {seed}: nothing admitted after budget_exceeded: {after:?}"
        );
        assert!(
            !after.iter().any(|kind| {
                *kind == "attempt_interrupted" || *kind == "merge_verification_interrupted"
            }),
            "seed {seed}: a budget stop cancels nothing: {after:?}"
        );
        let (attempts, verifications) = in_flight_at(&events, stop + 1);
        let (left, left_verifying) = in_flight_at(&events, events.len());
        assert_eq!(
            (left, left_verifying),
            (0, 0),
            "seed {seed}: the drain settled the {attempts} attempt(s) and {verifications} \
                     verification(s) in flight at the stop, naturally"
        );
        assert!(
            wide.env
                .runner
                .endings()
                .iter()
                .all(|(_, ending)| *ending == crate::engine::topology::scaffold::Ending::Completed),
            "seed {seed}: no process was cancelled"
        );
        let reservations = wide.run.broker_mut().reservations();
        assert!(
            reservations.balances() && reservations.is_empty(),
            "seed {seed}"
        );
        assert!(wide.run.invocations_balance(), "seed {seed}");
        replay_equals_live(&wide);
        drained_total += attempts + verifications;
        seeds.push(serde_json::json!({
            "seed": seed,
            "budget_exceeded_at": stop,
            "run_finished_at": kinds.len() - 1,
            "drained": {"attempts": attempts, "verifications": verifications},
        }));
    }
    assert!(
        drained_total > 0,
        "some seed drained work in flight at the stop"
    );
    export(
        "seam/ST-17",
        &serde_json::json!({
            "row": "ST-17 (budget stop)",
            "plan": "alpha, beta, gamma, delta independent under a run ceiling of $0.40",
            "width": 3,
            "seeds": seeds,
        }),
    );
}

const FINALIZE_CHILD: &str =
    "engine::topology::coordinator::tests::interleaving::finalization_kill_child_at_width_three";

const FINALIZE_SEED: u64 = 13;

fn finishing(halted: bool, tag: &str) -> Wide {
    if halted {
        durable_halting_on_gamma(tag)
    } else {
        let tasks = three();
        Wide::durable(tag, &tasks, 3, WidePlans::default(), holding(&tasks, &[]))
    }
}

fn finish_traced(wide: &mut Wide, killer: Killer) -> (Result<Progress, UpstrokeError>, Vec<Trace>) {
    let double = std::sync::Arc::clone(&wide.env.runner);
    let mut interleaver = Interleaver::new(&double, FINALIZE_SEED, Adverse::Seeded);
    let mut hooks = Tracing::over(&wide.env, killer);
    let traced = hooks.traced.clone();
    let pipelines = wide.env.pipelines();
    let outcome = wide.run.run_concurrently(
        &wide.env.seams(),
        &pipelines,
        &mut hooks,
        Some(&mut interleaver),
    );
    drop(interleaver);
    (outcome, traced.entries())
}

fn after_the_end(trace: &[Trace]) -> Vec<(EffectSiteId, HookPhase)> {
    trace
        .iter()
        .skip_while(|entry| {
            !matches!(
                entry,
                Trace::Appended {
                    kind: "run_finished",
                    ..
                }
            )
        })
        .filter_map(|entry| match entry {
            Trace::Effect(site, phase) => Some((*site, *phase)),
            Trace::Appended { .. } => None,
        })
        .collect()
}

const FIRED: &str = "fired-at";

#[test]
#[ignore = "spawned by the four parts of the width-3 finalization kill matrices (ST-18)"]
fn finalization_kill_child_at_width_three() {
    let dir = PathBuf::from(
        std::env::var("UPSTROKE_TEST_KILL_DIR").expect("the parent names the handoff"),
    );
    let halted = match std::env::var("UPSTROKE_TEST_KILL_OUTCOME").as_deref() {
        Ok("halted") => true,
        Ok("complete") => false,
        other => panic!("the parent names the outcome: {other:?}"),
    };
    let cell: usize = std::env::var("UPSTROKE_TEST_KILL_CELL")
        .expect("the parent names the cell")
        .parse()
        .expect("a cell index");
    let mut wide = finishing(halted, "finalize-kill");
    crate::workspace_manager::fixture::write_file(
        &dir.join(KILL_HANDOFF),
        wide.env.fixture.root.to_string_lossy().as_bytes(),
    );
    let _ = finish_traced(&mut wide, Killer::at(cell).reporting(dir.join(FIRED)));
    panic!("the kill at finalization cell {cell} must have taken this process");
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Settled {
    outcome: serde_json::Value,
    retained_candidates: usize,
    open_questions: usize,
    refs: Vec<String>,
    worktrees: usize,
    execution_root: bool,
}

fn settled_state(env: &crate::engine::topology::scaffold::WideEnv) -> Settled {
    let report: serde_json::Value = serde_json::from_slice(
        &std::fs::read(env.paths.public.join("report.json")).expect("the report exists"),
    )
    .expect("the report parses");
    let namespace = crate::engine::topology::candidate::run_namespace(
        crate::workspace_manager::fixture::RUN_ID,
    );
    let refs = crate::workspace_manager::fixture::git(
        &env.fixture.base,
        &["for-each-ref", "--format=%(refname)", &namespace],
    );
    let listed = crate::workspace_manager::fixture::git(
        &env.fixture.base,
        &["worktree", "list", "--porcelain"],
    );
    Settled {
        outcome: report["outcome"].clone(),
        retained_candidates: report["retained_candidates"].as_array().map_or(0, Vec::len),
        open_questions: report["open_questions"].as_array().map_or(0, Vec::len),
        refs: refs.lines().map(str::to_owned).collect(),
        worktrees: listed
            .lines()
            .filter(|line| line.starts_with("worktree "))
            .count(),
        execution_root: std::fs::symlink_metadata(env.fixture.manager.execution_root()).is_ok(),
    }
}

fn refused_resume(
    env: crate::engine::topology::scaffold::WideEnv,
    tag: &str,
) -> (String, crate::engine::topology::scaffold::WideEnv) {
    let runtime = crate::runner::container::FakeRuntime::new(
        crate::runner::container::runtime::ContainerTrace::default(),
    );
    let liveness = crate::runner::container::FakeOwnerLiveness::new();
    let mut hooks = env.hooks();
    match env.try_resume_over(
        "inc-2",
        RecordingRunner::new(),
        crate::engine::topology::select::Ceiling::unlimited(),
        &crate::engine::topology::scaffold::ResumingOver {
            runtime: &runtime,
            liveness: &liveness,
            preflight: &crate::engine::topology::scaffold::Certifying,
            awaits_release: true,
        },
        &mut hooks,
    ) {
        Ok(_) => panic!("{tag}: a finished run was resumed instead of finalized"),
        Err(failed) => {
            let (error, env) = *failed;
            (error.to_string(), env)
        }
    }
}

struct Planted {
    published: PathBuf,
    partial: PathBuf,
    published_bytes: Vec<u8>,
    partial_bytes: Vec<u8>,
}

impl Planted {
    fn beside(env: &crate::engine::topology::scaffold::WideEnv) -> Self {
        let answers = env.paths.public.join("answers");
        let question = env
            .durable_events()
            .iter()
            .find_map(|event| match &event.body {
                TopologyEventBody::QuestionRaised { data } => Some(data.question.id.clone()),
                _ => None,
            })
            .unwrap_or_else(|| crate::ir::QuestionId("q-after-the-end".to_owned()));
        let published = crate::interaction::answer_path(&answers, &question);
        let partial = answers.join(format!(
            "q-another.json.{}-{}.partial",
            std::process::id(),
            crate::ulid::ulid()
        ));
        crate::workspace_manager::fixture::write_file(
            &published,
            b"{\"answer\":\"answered\",\"text\":\"go ahead\"}",
        );
        crate::workspace_manager::fixture::write_file(
            &partial,
            b"{\"answer\":\"answered\",\"text\":\"half",
        );
        Self {
            published_bytes: std::fs::read(&published).expect("published"),
            partial_bytes: std::fs::read(&partial).expect("staged"),
            published,
            partial,
        }
    }

    #[track_caller]
    fn untouched(&self, tag: &str) {
        assert_eq!(
            std::fs::read(&self.published).expect("still published"),
            self.published_bytes,
            "{tag}: the published answer is byte-identical"
        );
        assert_eq!(
            std::fs::read(&self.partial).expect("still staged"),
            self.partial_bytes,
            "{tag}: the writer-owned residue is byte-identical"
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Parity {
    Even,
    Odd,
}

impl Parity {
    const fn name(self) -> &'static str {
        match self {
            Self::Even => "even",
            Self::Odd => "odd",
        }
    }

    const fn first(self) -> usize {
        match self {
            Self::Even => 0,
            Self::Odd => 1,
        }
    }

    const fn share_of(self, cells: usize) -> usize {
        match self {
            Self::Even => cells.div_ceil(2),
            Self::Odd => cells / 2,
        }
    }
}

#[test]
fn a_concurrent_complete_run_finalizes_through_the_frozen_finalization_and_converges_after_a_kill_at_every_cell_even_cells()
 {
    finalization_kill_matrix(false, Parity::Even);
}

#[test]
fn a_concurrent_complete_run_finalizes_through_the_frozen_finalization_and_converges_after_a_kill_at_every_cell_odd_cells()
 {
    finalization_kill_matrix(false, Parity::Odd);
}

#[test]
fn a_concurrent_halted_run_finalizes_through_the_frozen_finalization_and_converges_after_a_kill_at_every_cell_even_cells()
 {
    finalization_kill_matrix(true, Parity::Even);
}

#[test]
fn a_concurrent_halted_run_finalizes_through_the_frozen_finalization_and_converges_after_a_kill_at_every_cell_odd_cells()
 {
    finalization_kill_matrix(true, Parity::Odd);
}

fn finalization_kill_matrix(halted: bool, parity: Parity) {
    let label = if halted { "halted" } else { "complete" };
    let mut reference = finishing(halted, &format!("interleaving-finalize-{label}"));
    let (outcome, trace) = finish_traced(&mut reference, Killer::default());
    let progress = outcome.unwrap_or_else(|error| panic!("{label}: {error}"));
    assert_eq!(
        outcome_of(&progress),
        if halted {
            RunOutcome::Halted
        } else {
            RunOutcome::Complete
        },
        "{label}"
    );
    let cells = after_the_end(&trace);
    let settled = settled_state(&reference.env);
    assert_eq!(
        (settled.worktrees, settled.execution_root),
        (1, false),
        "{label}: the uninterrupted finalization left the user checkout alone"
    );
    assert_eq!(
        (settled.refs.len(), settled.retained_candidates),
        if halted { (2, 2) } else { (0, 0) },
        "{label}: Complete deletes the candidates refs, Halted retains and lists \
                 them: {settled:?}"
    );
    assert!(
        cells.len() >= 6,
        "{label}: the report's four cells and the execution root's two at least: \
                 {cells:?}"
    );
    let mut killed = Vec::new();
    for (cell, expected) in cells.iter().enumerate().skip(parity.first()).step_by(2) {
        let tag = format!("{label}, cell {cell} ({} {})", expected.0, expected.1);
        let handoff = crate::engine::topology::scaffold::kill_dir(&format!(
            "interleaving-finalize-{label}-{cell}"
        ));
        let index = cell.to_string();
        let temporary = crate::engine::topology::scaffold::kill_dir("tmp");
        let mut environment = vec![
            ("UPSTROKE_TEST_KILL_DIR", handoff.path().as_os_str()),
            ("UPSTROKE_TEST_KILL_OUTCOME", std::ffi::OsStr::new(label)),
            ("UPSTROKE_TEST_KILL_CELL", std::ffi::OsStr::new(&index)),
        ];
        environment.extend(crate::engine::topology::scaffold::child_temporary_of(
            temporary.path(),
        ));
        let status = crate::workspace_manager::fixture::run_kill_child_within(
            FINALIZE_CHILD,
            &environment,
            crate::engine::topology::scaffold::KILL_CHILD_BOUND,
        )
        .unwrap_or_else(|| panic!("{tag}: the kill child did not end in time"));
        assert!(
            crate::workspace_manager::fixture::died_by_abort(&status),
            "{tag}: the child died by the kill: {status:?}"
        );
        let fired: (EffectSiteId, HookPhase) = serde_json::from_str(
            &std::fs::read_to_string(handoff.path().join(FIRED))
                .unwrap_or_else(|error| panic!("{tag}: no kill was reported: {error}")),
        )
        .expect("the reported cell parses");
        assert_eq!(
            &fired, expected,
            "{tag}: the child died where the reference run consulted that cell"
        );
        let root = std::fs::read_to_string(handoff.path().join(KILL_HANDOFF))
            .unwrap_or_else(|error| panic!("{tag}: no handoff: {error}"));
        let mut env = crate::engine::topology::scaffold::WideEnv::adopted(
            PathBuf::from(root),
            &three(),
            3,
            WidePlans::default(),
        );
        if halted {
            halting(&mut env);
        }
        let log = std::fs::read(&env.log).expect("the killed run's log");
        let kinds = kinds_of(&env.durable_events());
        assert_eq!(
            kinds.last(),
            Some(&"run_finished"),
            "{tag}: the child ended the run before it died"
        );
        let planted = Planted::beside(&env);
        let (first, env) = refused_resume(env, &tag);
        let converged = settled_state(&env);
        assert_eq!(
            converged, settled,
            "{tag}: the resume finalized to what the uninterrupted finalization left"
        );
        let report = std::fs::read(env.paths.public.join("report.json")).expect("the report");
        let (second, env) = refused_resume(env, &tag);
        assert!(
            second.contains("the report was already current"),
            "{tag}: repeated finalization finds the report current: {second}"
        );
        for step in crate::engine::topology::finalize::CleanupStep::ORDER {
            let label = step.label();
            assert!(
                !second.contains(label) || second.contains(&format!(" 0 {label}")),
                "{tag}: repeated finalization removes no {label}: {second}"
            );
        }
        assert_eq!(
            settled_state(&env),
            converged,
            "{tag}: and removes nothing more"
        );
        assert_eq!(
            std::fs::read(env.paths.public.join("report.json")).expect("the report"),
            report,
            "{tag}: a fresh report is not rewritten"
        );
        assert_eq!(
            std::fs::read(&env.log).expect("the log"),
            log,
            "{tag}: neither resume appended anything, and no answer was ingested"
        );
        planted.untouched(&tag);
        killed.push(serde_json::json!({
            "cell": cell,
            "site": fired.0.to_string(),
            "phase": fired.1.to_string(),
            "first_resume": first,
            "second_resume": second,
            "converged": true,
            "idempotent": true,
            "answer_files_untouched": true,
        }));
    }
    assert_eq!(
        killed.len(),
        parity.share_of(cells.len()),
        "{label}: the {} cells of the {} after run_finished were killed, each once, and \
                 the other part's share is the rest: {cells:?}",
        parity.name(),
        cells.len()
    );
    export(
        "seam/ST-18",
        &serde_json::json!({
            "row": "ST-18",
            "width": 3,
            "through": "the coordinator's closure, whose finalization is the frozen \
                                finalize.rs; each kill in a child process at one cell after \
                                run_finished, then two resumes through the frozen recovery order",
            "outcome": label,
            "seed": FINALIZE_SEED,
            "cells_after_run_finished": cells.len(),
            "part": format!("{} cells", parity.name()),
            "settled": {
                "outcome": settled.outcome,
                "retained_candidates": settled.retained_candidates,
                "open_questions": settled.open_questions,
                "refs_under_the_run_namespace": settled.refs,
                "worktrees_listed": settled.worktrees,
                "execution_root_present": settled.execution_root,
            },
            "kills": killed,
        }),
    );
}
