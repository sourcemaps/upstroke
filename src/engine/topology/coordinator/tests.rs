//! Extended notes: `docs/internals/engine/topology/coordinator/tests.md`

#[cfg(test)]
mod interleaving;
#[cfg(test)]
mod pipelines;
#[cfg(test)]
mod registry_waits;
#[cfg(test)]
mod runners;
#[cfg(test)]
mod scheduling;
#[cfg(test)]
mod shutdown;
#[cfg(test)]
mod snapshot_registration;
#[cfg(test)]
mod verification_charges;

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
    fn scrub(value: &mut serde_json::Value, labels: &mut BTreeMap<String, String>, run_id: &str) {
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

    fn resolve(&self, _question: &crate::ir::Question) -> Result<crate::ir::Answer, UpstrokeError> {
        Ok(crate::ir::Answer::Declined)
    }

    fn poll(&self, _question: &crate::ir::Question) -> Result<crate::ir::Answer, UpstrokeError> {
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

fn three() -> [WideTask; 3] {
    [
        WideTask::independent("alpha"),
        WideTask::independent("beta"),
        WideTask::independent("gamma"),
    ]
}

fn scheduled_here(every: std::ops::Range<u64>, on_windows: &[u64]) -> Vec<u64> {
    assert!(
        !on_windows.is_empty() && on_windows.iter().all(|seed| every.contains(seed)),
        "the Windows schedules {on_windows:?} are a subset of {every:?}"
    );
    if cfg!(windows) {
        on_windows.to_vec()
    } else {
        every.collect()
    }
}

const ATTEMPT_LEVEL: [&str; 5] = [
    "task_dispatched",
    "attempt_started",
    "attempt_finished",
    "candidate_prepared",
    "task_candidate_created",
];

const ONE_SUCCEEDING_ATTEMPT: [&str; 4] = [
    "task_dispatched",
    "attempt_started",
    "candidate_prepared",
    "task_candidate_created",
];

fn attempt_level(events: &[TopologyEvent], run_id: &str) -> BTreeMap<u64, Vec<String>> {
    let mut traces: BTreeMap<u64, Vec<TopologyEvent>> = BTreeMap::new();
    for event in events
        .iter()
        .filter(|event| ATTEMPT_LEVEL.contains(&event.body.kind()))
    {
        let key = event
            .body
            .key()
            .expect("an attempt-level event names its task");
        // §6: canonical() labels one slice of events and a key's trace is not contiguous in
        // the log, so each selected event is copied into its own key's trace.
        traces
            .entry(u64::from(key.0))
            .or_default()
            .push(event.clone());
    }
    traces
        .into_iter()
        .map(|(key, trace)| {
            let projected = canonical(&trace, run_id)
                .iter()
                .map(ToString::to_string)
                .collect();
            (key, projected)
        })
        .collect()
}

fn projected_kinds(values: &[String]) -> Vec<String> {
    values
        .iter()
        .map(|value| {
            serde_json::from_str::<serde_json::Value>(value)
                .expect("a projected event is JSON")
                .get("event")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned()
        })
        .collect()
}

fn attempt_level_problems(
    keyed: &BTreeMap<u64, Vec<String>>,
    keys: &[u64],
) -> BTreeMap<u64, String> {
    let mut problems: BTreeMap<u64, String> = keyed
        .keys()
        .filter(|key| !keys.contains(key))
        .map(|key| (*key, "projected, and not an expected key".to_owned()))
        .collect();
    for key in keys {
        let kinds = projected_kinds(keyed.get(key).map(Vec::as_slice).unwrap_or_default());
        if kinds != ONE_SUCCEEDING_ATTEMPT {
            problems.insert(
                *key,
                format!("projects {kinds:?}, not {ONE_SUCCEEDING_ATTEMPT:?}"),
            );
        }
    }
    problems
}

fn final_tree(wide: &Wide) -> String {
    let head = wide.env.head(&wide.run);
    crate::workspace_manager::fixture::git(
        &wide.env.fixture.base,
        &["rev-parse", &format!("{head}^{{tree}}")],
    )
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

fn gate_snapshot_names(
    manager: &WorkspaceManager,
    key: u32,
    generation: u32,
    attempts: &[u32],
) -> Vec<String> {
    attempts
        .iter()
        .map(|attempt| {
            manager
                .slot_path(&crate::workspace_manager::Slot::Snapshot {
                    name: crate::workspace_manager::SnapshotName::gates(key, generation, *attempt),
                })
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_owned)
                .unwrap_or_default()
        })
        .collect()
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
            .filter(|(_, ending)| *ending == crate::engine::topology::scaffold::Ending::Cancelled)
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

const BINDING: &str = "whose identity is";

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

// Protocol (standards §10): the policy owns its writer. The verification
// that plants the tear stores it once; `finish`, or the drop when a test
// unwinds first, cancels it through its channel and joins it, so nothing it
// does lands after the fixture is reclaimed. The lock (§6) guards only that
// handle, stored once and taken once: `problem` takes `&self` on a
// pipeline's thread.
struct TearsAForeignRegistration {
    admin: PathBuf,
    head: String,
    common: PathBuf,
    finishes: bool,
    watchdog: Duration,
    remaining: std::sync::atomic::AtomicUsize,
    writer: std::sync::Mutex<Option<ForeignWriter>>,
}

struct ForeignWriter {
    cancel: std::sync::mpsc::Sender<()>,
    handle: std::thread::JoinHandle<Result<(), String>>,
}

impl TearsAForeignRegistration {
    fn new(
        admin: PathBuf,
        head: String,
        common: PathBuf,
        finishes: bool,
        watchdog: Duration,
    ) -> Self {
        Self {
            admin,
            head,
            common,
            finishes,
            watchdog,
            remaining: std::sync::atomic::AtomicUsize::new(1),
            writer: std::sync::Mutex::new(None),
        }
    }

    fn finish(&self) -> Result<(), String> {
        self.settle(true)
    }

    fn settle(&self, cancel: bool) -> Result<(), String> {
        let writer = self
            .writer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        let Some(ForeignWriter {
            cancel: canceller,
            handle,
        }) = writer
        else {
            return Err(format!(
                "no writer was started for the registration at {}: no verification planted \
                     the tear, or it was planted with `finishes` false",
                self.admin.display()
            ));
        };
        if cancel {
            let _ = canceller.send(());
        }
        handle.join().unwrap_or_else(|_| {
            Err(format!(
                "the writer of the registration at {} panicked",
                self.admin.display()
            ))
        })
    }

    fn start_writer(&self) -> ForeignWriter {
        let before = crate::workspace_manager::contended_attempts(&self.common);
        let (admin, common, watchdog) = (self.admin.clone(), self.common.clone(), self.watchdog);
        let (cancel, cancelled) = std::sync::mpsc::channel::<()>();
        let handle = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + watchdog;
            loop {
                if crate::workspace_manager::contended_attempts(&common) > before {
                    crate::workspace_manager::fixture::write_file(
                        &admin.join("commondir"),
                        b"../..\n",
                    );
                    return Ok(());
                }
                let left = deadline.saturating_duration_since(std::time::Instant::now());
                if left.is_zero() {
                    return Err(format!(
                        "no registry access failed on the registration planted at {} within \
                             {watchdog:?}, so its writer wrote nothing",
                        admin.display()
                    ));
                }
                match cancelled.recv_timeout(left.min(Duration::from_millis(1))) {
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                    Ok(()) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        return Err(format!(
                            "the writer of the registration planted at {} was cancelled \
                                 before any registry access failed on it, and wrote nothing",
                            admin.display()
                        ));
                    }
                }
            }
        });
        ForeignWriter { cancel, handle }
    }
}

impl Drop for TearsAForeignRegistration {
    fn drop(&mut self) {
        let held = self
            .writer
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some();
        if !held {
            return;
        }
        if let Err(why) = self.settle(true) {
            if std::thread::panicking() {
                crate::workspace_manager::fixture::say(&why);
            } else {
                panic!("{why}");
            }
        }
    }
}

impl ReviewInputPolicy for TearsAForeignRegistration {
    fn problem(
        &self,
        worktree: &std::path::Path,
        _tree: &str,
    ) -> Result<Option<String>, UpstrokeError> {
        let verifying = worktree.parent().and_then(std::path::Path::file_name)
            == Some(std::ffi::OsStr::new("merge"));
        if verifying
            && self
                .remaining
                .compare_exchange(
                    1,
                    0,
                    std::sync::atomic::Ordering::SeqCst,
                    std::sync::atomic::Ordering::SeqCst,
                )
                .is_ok()
        {
            let checkout = self.admin.join("foreign-checkout").join(".git");
            crate::workspace_manager::fixture::write_file(
                &self.admin.join("HEAD"),
                format!("{}\n", self.head).as_bytes(),
            );
            crate::workspace_manager::fixture::write_file(
                &self.admin.join("gitdir"),
                format!(
                    "{}\n",
                    crate::workspace_manager::fixture::as_git_writes_it(&checkout)
                )
                .as_bytes(),
            );
            crate::workspace_manager::fixture::write_file(&self.admin.join("commondir"), b"");
            if self.finishes {
                let writer = self.start_writer();
                *self
                    .writer
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(writer);
            }
        }
        Ok(None)
    }
}

fn tearing(wide: &Wide, name: &str, finishes: bool) -> (Arc<TearsAForeignRegistration>, PathBuf) {
    let common = wide.env.fixture.manager.common_git_dir().to_path_buf();
    let admin = common.join("worktrees").join(name);
    let policy = Arc::new(TearsAForeignRegistration::new(
        admin.clone(),
        wide.env.fixture.head.clone(),
        common,
        finishes,
        BOUND,
    ));
    (policy, admin)
}

#[derive(Clone, Copy)]
enum TearAt {
    Fold(fn(&TopologyEventBody) -> bool),
    Access(fn(&TopologyEventBody) -> bool, usize),
}

struct Prober {
    cancel: std::sync::mpsc::Sender<()>,
    handle: std::thread::JoinHandle<Result<(), String>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Wakes {
    Entered,
    Ended,
    Contended,
    Never,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Torn {
    CommondirEmpty,
    GitdirMissing,
    /// A registration whose `gitdir` cannot be read: a directory stands
    /// where the file should be. The removal's scan and the registry read
    /// that discovers earlier incarnations' instances both fail on it,
    /// where they pass over the two shapes above (the follow-up C record,
    /// §6.11).
    GitdirUnreadable,
}

#[derive(Clone)]
struct Plant {
    admin: PathBuf,
    head: String,
    runner: Arc<RecordingRunner>,
    wakes: Wakes,
    common: PathBuf,
    torn: Torn,
}

impl Plant {
    fn finish_tear(&self) {
        if self.torn == Torn::GitdirUnreadable {
            crate::workspace_manager::fixture::remove_dir(&self.admin.join("gitdir"));
        }
        if matches!(self.torn, Torn::GitdirMissing | Torn::GitdirUnreadable) {
            self.whole();
        }
        crate::workspace_manager::fixture::write_file(&self.admin.join("commondir"), b"../..\n");
        if self.torn == Torn::GitdirMissing {
            crate::workspace_manager::fixture::remove_file(&self.admin.join("locked"));
        }
    }

    fn whole(&self) {
        let checkout = self.admin.join("foreign-checkout").join(".git");
        crate::workspace_manager::fixture::write_file(
            &self.admin.join("HEAD"),
            format!("{}\n", self.head).as_bytes(),
        );
        crate::workspace_manager::fixture::write_file(
            &self.admin.join("gitdir"),
            format!(
                "{}\n",
                crate::workspace_manager::fixture::as_git_writes_it(&checkout)
            )
            .as_bytes(),
        );
    }

    fn tear(&self) -> Prober {
        self.tear_sampling_every(Duration::from_millis(1))
    }

    fn tear_sampling_every(&self, every: Duration) -> Prober {
        match self.torn {
            Torn::CommondirEmpty => {
                self.whole();
                crate::workspace_manager::fixture::write_file(&self.admin.join("commondir"), b"");
            }
            Torn::GitdirMissing => {
                crate::workspace_manager::fixture::write_file(
                    &self.admin.join("locked"),
                    b"initializing\n",
                );
            }
            Torn::GitdirUnreadable => {
                crate::workspace_manager::fixture::write_file(
                    &self.admin.join("HEAD"),
                    format!("{}\n", self.head).as_bytes(),
                );
                crate::workspace_manager::fixture::write_file(
                    &self.admin.join("commondir"),
                    b"../..\n",
                );
                crate::workspace_manager::fixture::create_dir(&self.admin.join("gitdir"));
            }
        }
        let (cancel, cancelled) = std::sync::mpsc::channel::<()>();
        if self.wakes == Wakes::Never {
            let handle = std::thread::spawn(move || {
                let _ = cancelled.recv_timeout(BOUND);
                Ok(())
            });
            return Prober { cancel, handle };
        }
        let (wakes, common) = (self.wakes, self.common.clone());
        let progress = move |runner: &RecordingRunner| match wakes {
            Wakes::Entered => runner.ran().len(),
            Wakes::Ended => runner.endings().len(),
            Wakes::Contended => crate::workspace_manager::contended_attempts(&common),
            Wakes::Never => 0,
        };
        let before = progress(&self.runner);
        let (admin, runner, finishing) =
            (self.admin.clone(), Arc::clone(&self.runner), self.clone());
        let handle = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + BOUND;
            loop {
                let left = deadline.saturating_duration_since(std::time::Instant::now());
                let cancelled_now = !matches!(
                    cancelled.recv_timeout(left.min(every)),
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout)
                );
                if progress(&runner) > before {
                    finishing.finish_tear();
                    return Ok(());
                }
                if cancelled_now {
                    return Err(format!(
                        "{} before the prober of the tear planted at {} was cancelled, so \
                             the registration stayed torn",
                        wakes.not_seen(),
                        admin.display()
                    ));
                }
                if left.is_zero() {
                    return Err(format!(
                        "{} within {BOUND:?} of the tear planted at {}, so nothing finished \
                             it",
                        wakes.not_seen(),
                        admin.display()
                    ));
                }
            }
        });
        Prober { cancel, handle }
    }
}

impl Wakes {
    fn not_seen(self) -> &'static str {
        match self {
            Wakes::Entered => "no invocation reached the runner",
            Wakes::Ended => "no invocation left the runner",
            Wakes::Contended => "no registry access failed on the tear",
            Wakes::Never => "nothing was awaited",
        }
    }
}

type FoldAct = (fn(&TopologyEventBody) -> bool, Box<dyn FnMut()>);

const WITNESS_REGISTRY_DEADLINE: Duration = Duration::from_secs(10);

struct TearHeld {
    inner: crate::engine::topology::seams::HarnessTopologyHooks,
    at: TearAt,
    plant: Plant,
    prober: std::rc::Rc<std::cell::RefCell<Option<Prober>>>,
    planting: Option<crate::workspace_manager::fixture::AccessPlanting>,
    armed: bool,
    also: Option<FoldAct>,
    _deadline: crate::workspace_manager::RegistryDeadline,
}

impl TearHeld {
    fn new(wide: &Wide, name: &str, at: TearAt) -> Self {
        Self::waking(wide, name, at, Wakes::Entered)
    }

    fn waking(wide: &Wide, name: &str, at: TearAt, wakes: Wakes) -> Self {
        Self {
            inner: wide.env.hooks(),
            at,
            plant: Plant {
                admin: wide
                    .env
                    .fixture
                    .manager
                    .common_git_dir()
                    .join("worktrees")
                    .join(name),
                head: wide.env.fixture.head.clone(),
                runner: Arc::clone(&wide.env.runner),
                wakes,
                common: wide.env.fixture.manager.common_git_dir().to_path_buf(),
                torn: Torn::CommondirEmpty,
            },
            prober: std::rc::Rc::new(std::cell::RefCell::new(None)),
            planting: None,
            armed: false,
            also: None,
            _deadline: crate::workspace_manager::RegistryDeadline::hold(
                wide.env.fixture.manager.common_git_dir(),
                WITNESS_REGISTRY_DEADLINE,
            ),
        }
    }

    fn tearing(&mut self, torn: Torn) {
        self.plant.torn = torn;
    }

    fn planted(&self) -> impl Fn() -> bool + 'static {
        let prober = std::rc::Rc::clone(&self.prober);
        move || prober.borrow().is_some()
    }

    fn finish(&mut self) -> Result<(), String> {
        let Some(Prober { cancel, handle }) = self.prober.borrow_mut().take() else {
            return Err(format!(
                "the tear at {} was never planted",
                self.plant.admin.display()
            ));
        };
        let _ = cancel.send(());
        handle
            .join()
            .unwrap_or_else(|_| Err("the prober panicked".to_owned()))
    }
}

impl Drop for TearHeld {
    fn drop(&mut self) {
        self.planting = None;
        if self.prober.borrow().is_none() {
            return;
        }
        if let Err(why) = self.finish() {
            if std::thread::panicking() {
                crate::workspace_manager::fixture::say(&why);
            } else {
                panic!("{why}");
            }
        }
    }
}

impl crate::engine::topology::seams::TopologyHooks for TearHeld {
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
        let Some(last) = events.last() else {
            return;
        };
        if self.also.as_ref().is_some_and(|(when, _)| when(&last.body)) {
            if let Some((_, mut act)) = self.also.take() {
                act();
            }
        }
        if self.armed {
            return;
        }
        match self.at {
            TearAt::Fold(when) if when(&last.body) => {
                self.armed = true;
                *self.prober.borrow_mut() = Some(self.plant.tear());
            }
            TearAt::Access(when, nth) if when(&last.body) => {
                self.armed = true;
                let (plant, slot) = (self.plant.clone(), std::rc::Rc::clone(&self.prober));
                self.planting = Some(crate::workspace_manager::fixture::before_registry_access(
                    nth,
                    Box::new(move |_| {
                        *slot.borrow_mut() = Some(plant.tear());
                    }),
                ));
            }
            TearAt::Fold(_) | TearAt::Access(..) => {}
        }
    }
}

fn two_independent() -> [WideTask; 2] {
    [
        WideTask::independent("beta"),
        WideTask::independent("alpha"),
    ]
}

fn attempt_started_of(key: u32) -> fn(&TopologyEventBody) -> bool {
    match key {
        0 => {
            |body| matches!(body, TopologyEventBody::AttemptStarted { data } if data.key == TaskKey(0))
        }
        _ => {
            |body| matches!(body, TopologyEventBody::AttemptStarted { data } if data.key == TaskKey(1))
        }
    }
}

fn task_dispatched_of(key: u32) -> fn(&TopologyEventBody) -> bool {
    match key {
        0 => {
            |body| matches!(body, TopologyEventBody::TaskDispatched { data } if data.key == TaskKey(0))
        }
        _ => {
            |body| matches!(body, TopologyEventBody::TaskDispatched { data } if data.key == TaskKey(1))
        }
    }
}

fn beta_settles_while_alpha_holds_its_first_gate(view: &Quiescent<'_>) -> Option<Release> {
    let beta_live = view.live.iter().any(
        |(_, identity)| matches!(identity, Identity::Attempt { key, .. } if *key == TaskKey(0)),
    );
    released(view, |invocation| {
        attempt_key(invocation) == Some(1) && role_of(invocation) == Some(AttemptRole::Worker)
    })
    .or_else(|| {
        released(view, |invocation| {
            attempt_key(invocation) == Some(if beta_live { 0 } else { 1 })
        })
    })
}

fn candidate_created_of_beta(body: &TopologyEventBody) -> bool {
    matches!(body, TopologyEventBody::TaskCandidateCreated { data } if data.candidate.key == TaskKey(0))
}

fn merge_prepared_of_beta(body: &TopologyEventBody) -> bool {
    matches!(body, TopologyEventBody::MergePrepared { data } if data.key == TaskKey(0))
}

fn retained_attempt_finished_of_beta(body: &TopologyEventBody) -> bool {
    matches!(body, TopologyEventBody::AttemptFinished { data } if data.key == TaskKey(0))
}

fn alpha_waits_for_the_tear(planted: impl Fn() -> bool + 'static) -> Script<'static> {
    Box::new(move |view: &Quiescent<'_>| {
        if planted() {
            return beta_settles_while_alpha_holds_its_first_gate(view);
        }
        let beta_live = view.live.iter().any(
            |(_, identity)| matches!(identity, Identity::Attempt { key, .. } if *key == TaskKey(0)),
        );
        released(view, |invocation| {
            attempt_key(invocation) == Some(1) && role_of(invocation) == Some(AttemptRole::Worker)
        })
        .or_else(|| {
            if beta_live {
                released(view, |invocation| attempt_key(invocation) == Some(0))
            } else {
                None
            }
        })
        .or(Some(Release::Nothing))
    })
}

fn served_while_a_repair_waits(
    tag: &str,
    failing: &[(u32, u32)],
    at: TearAt,
    torn: Torn,
    residue: bool,
) {
    let (what, tag, failing) = (tag.to_owned(), tag.to_owned(), failing.to_vec());
    bounded(&what, move || {
        let tasks = two_independent();
        let plans = WidePlans {
            gates: 2,
            ..WidePlans::default()
        };
        let mut wide = Wide::started_with(&tag, &tasks, 2, plans, holding(&tasks, &failing));
        let common = wide.env.fixture.manager.common_git_dir().to_path_buf();
        let leftover = common.join("worktrees").join("residue");
        let mut hooks = TearHeld::new(&wide, "foreign-held", at);
        hooks.tearing(torn);
        if residue {
            let slot = crate::engine::topology::dispatch::task_slot(TaskKey(9), GenerationId(0));
            wide.env
                .fixture
                .manager
                .write_intent(&mut crate::workspace_manager::NoHooks, &slot)
                .expect("the residue's intent");
            let checkout = wide.env.fixture.manager.slot_path(&slot).join(".git");
            let (admin, head) = (leftover.clone(), wide.env.fixture.head.clone());
            let (TearAt::Fold(when) | TearAt::Access(when, _)) = at;
            hooks.also = Some((
                when,
                Box::new(move || {
                    crate::workspace_manager::fixture::write_file(
                        &admin.join("HEAD"),
                        format!("{head}\n").as_bytes(),
                    );
                    crate::workspace_manager::fixture::write_file(
                        &admin.join("gitdir"),
                        format!(
                            "{}\n",
                            crate::workspace_manager::fixture::as_git_writes_it(&checkout)
                        )
                        .as_bytes(),
                    );
                    crate::workspace_manager::fixture::write_file(&admin.join("commondir"), b"");
                }),
            ));
        }
        let planted = hooks.planted();
        let pipelines = wide.env.pipelines();
        let runner = Arc::clone(&wide.env.runner);
        let mut scheduler = Scheduler::scripted(&runner, alpha_waits_for_the_tear(planted));
        let slept = crate::workspace_manager::fixture::slept_pauses();
        let progress = wide
            .run
            .run_concurrently(
                &wide.env.seams(),
                &pipelines,
                &mut hooks,
                Some(&mut scheduler),
            )
            .expect("the access is passed once the coordinator served a pipeline meanwhile");
        assert_eq!(
            crate::workspace_manager::fixture::slept_pauses(),
            slept,
            "no wait of a registry access the coordinator made slept on its thread"
        );
        drop(scheduler);
        hooks
            .finish()
            .expect("an invocation reached the runner while the access waited on the tear");
        drop(hooks);
        assert_eq!(outcome_of(&progress), RunOutcome::Complete);
        assert!(
            !leftover.exists(),
            "the repair removed the dead add's residue"
        );
        assert!(wide.run.invocations_balance());
    });
}

#[test]
fn a_pipeline_is_served_while_an_intent_removals_repair_plan_waits_on_a_torn_registration() {
    served_while_a_repair_waits(
        "registry-served-intent-repair-plan",
        &[],
        TearAt::Access(candidate_created_of_beta, 3),
        Torn::GitdirMissing,
        true,
    );
}

fn alpha_waits_for_gammas_integration(view: &Quiescent<'_>) -> Option<Release> {
    let gamma_live = view.live.iter().any(
        |(_, identity)| matches!(identity, Identity::Attempt { key, .. } if *key == TaskKey(2)),
    );
    if gamma_live {
        released(view, |invocation| attempt_key(invocation) != Some(1))
    } else {
        released(view, |invocation| attempt_key(invocation) == Some(1))
    }
}

fn alpha_waits_for_gammas_merge(view: &Quiescent<'_>) -> Option<Release> {
    let merged = view.run.events().iter().any(|event| {
        matches!(&event.body, TopologyEventBody::TaskMerged { data } if data.satisfies.contains(&TaskKey(2)))
    });
    if merged {
        released(view, |invocation| attempt_key(invocation) == Some(1))
    } else {
        released(view, |invocation| attempt_key(invocation) != Some(1))
    }
}

fn candidate_created_of_gamma(body: &TopologyEventBody) -> bool {
    matches!(body, TopologyEventBody::TaskCandidateCreated { data } if data.candidate.key == TaskKey(2))
}

fn task_merged_of_gamma(body: &TopologyEventBody) -> bool {
    matches!(body, TopologyEventBody::TaskMerged { data } if data.satisfies.contains(&TaskKey(2)))
}

fn merge_rejected_of_gamma(body: &TopologyEventBody) -> bool {
    matches!(body, TopologyEventBody::MergeRejected { data } if data.candidate.key == TaskKey(2))
}

fn repair_dispatched(body: &TopologyEventBody) -> bool {
    matches!(body, TopologyEventBody::TaskDispatched { data } if data.source_candidate.is_some())
}

fn a_shutdown_injected_once_the_tear_stands(
    planted: impl Fn() -> bool + 'static,
    injected: std::rc::Rc<std::cell::Cell<Option<usize>>>,
    finish: Option<Plant>,
    mut order: Script<'static>,
) -> Script<'static> {
    Box::new(move |view: &Quiescent<'_>| {
        if injected.get().is_none() && planted() {
            injected.set(Some(view.run.events().len()));
            assert!(view.injector.inject(ToCoordinator::Shutdown));
            if let Some(plant) = &finish {
                plant.finish_tear();
            }
            return Some(Release::Injected);
        }
        order(view)
    })
}

fn published_as_the_log_authorizes(wide: &Wide) {
    let started = wide.run.fold().started().expect("a started run").clone();
    let authorized =
        crate::engine::topology::integrate::authorized_head(&started, wide.run.events());
    assert_eq!(
        wide.env
            .fixture
            .manager
            .direct_ref_target(started.integration_ref.as_str())
            .expect("the integration ref reads"),
        Some(authorized.head.0),
        "the integration ref is where the log authorizes it, so nothing was published after \
             the shutdown"
    );
}

fn of_key(events: &[TopologyEvent], kind: &str, key: u32) -> usize {
    events
        .iter()
        .filter(|event| {
            event.body.kind() == kind
                && serde_json::to_value(&event.body)
                    .ok()
                    .as_ref()
                    .and_then(key_of)
                    == Some(u64::from(key))
        })
        .count()
}

fn coordinator_of<'s>(
    run: &'s mut TopologyRun,
    seams: &'s RunSeams<'s>,
    hooks: &'s mut dyn TopologyHooks,
    pipelines: &'s PipelineSeams,
    live: BTreeMap<PipelineId, Live>,
) -> Coordinator<'s> {
    let (outbox, inbox) = mpsc::unbounded_channel();
    let (injector, injected) = mpsc::unbounded_channel();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .build()
        .expect("a runtime");
    Coordinator {
        run,
        seams,
        hooks,
        pipelines,
        observer: None,
        leases: Vec::new(),
        live,
        replies: BTreeMap::new(),
        gate: SnapshotGate::default(),
        next: 1,
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
        ledger: crate::util::DurabilityLedger::off(),
        refusal: None,
        handles: Vec::new(),
        timer: Timer::start(outbox.clone()).expect("the wait timer starts"),
        inbox,
        outbox,
        injected,
        injector: Injector(injector),
        runtime,
    }
}

#[cfg(unix)]
struct RequiresAFailingFilter {
    base: PathBuf,
    info: PathBuf,
    remaining: std::sync::atomic::AtomicUsize,
}

#[cfg(unix)]
impl ReviewInputPolicy for RequiresAFailingFilter {
    fn problem(
        &self,
        worktree: &std::path::Path,
        _tree: &str,
    ) -> Result<Option<String>, UpstrokeError> {
        let verifying = worktree.parent().and_then(std::path::Path::file_name)
            == Some(std::ffi::OsStr::new("merge"));
        if verifying
            && self
                .remaining
                .compare_exchange(
                    1,
                    0,
                    std::sync::atomic::Ordering::SeqCst,
                    std::sync::atomic::Ordering::SeqCst,
                )
                .is_ok()
        {
            crate::workspace_manager::fixture::write_file(
                &self.info.join("attributes"),
                b"* filter=b329fails\n",
            );
            for (key, value) in [
                ("filter.b329fails.smudge", "false"),
                ("filter.b329fails.clean", "cat"),
                ("filter.b329fails.required", "true"),
            ] {
                crate::workspace_manager::fixture::git(&self.base, &["config", key, value]);
            }
        }
        Ok(None)
    }
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
    let mut wide = durable_halting_on_gamma("closure-kill");
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
        self.effects.fail_at = Some((crate::topology::effects::EffectSiteId::Ref(site), passing));
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

struct DecliningOnceArmed(std::sync::Arc<std::sync::atomic::AtomicBool>);

impl crate::interaction::AnswerSource for DecliningOnceArmed {
    fn id(&self) -> &'static str {
        "declining-once-armed"
    }

    fn resolve(&self, question: &crate::ir::Question) -> Result<crate::ir::Answer, UpstrokeError> {
        self.poll(question)
    }

    fn poll(&self, _question: &crate::ir::Question) -> Result<crate::ir::Answer, UpstrokeError> {
        Ok(if self.0.load(std::sync::atomic::Ordering::SeqCst) {
            crate::ir::Answer::Declined
        } else {
            crate::ir::Answer::Unanswered
        })
    }
}

fn four() -> [WideTask; 4] {
    [
        WideTask::independent("alpha"),
        WideTask::independent("beta"),
        WideTask::independent("gamma"),
        WideTask::independent("delta"),
    ]
}

const TIGHT: crate::engine::topology::select::Ceiling = crate::engine::topology::select::Ceiling {
    run_usd: Some(0.4),
    task_usd: None,
};

#[test]
fn the_overshoot_notes_bound_the_remaining_spend_of_every_pipeline_live_at_the_stop() {
    const NOTES: &str = include_str!("../../../../docs/internals/engine/topology/coordinator.md");
    let section = NOTES
        .split("\n### ")
        .find(|section| section.starts_with("Budget overshoot"))
        .expect("the notes carry the `Budget overshoot` section")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    for (proposition, pin) in [
        (
            "the exposure is what each live pipeline has still to spend",
            "remaining spend",
        ),
        ("its later review passes included", "review passes"),
        ("and their re-asks", "re-asks"),
        (
            "and no count of invocations bounds it",
            "not a count of invocations",
        ),
    ] {
        assert!(
            section.contains(pin),
            "the `Budget overshoot` notes must state that {proposition}; looked for {pin:?} \
                 in:\n{section}"
        );
    }
    assert!(
        !section.contains("each running one invocation at a time:"),
        "the retired reading, which bounds the overshoot by one invocation per live pipeline, \
             must not come back:\n{section}"
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
fn independent_tasks_dispatch_together_and_keep_per_key_projections_under_every_seed() {
    let tasks = three();
    let mut reference: Option<(BTreeMap<u64, Vec<String>>, String)> = None;
    for seed in scheduled_here(0..8, &[0, 1]) {
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
        let keyed = attempt_level(&events, &run_id);
        let problems = attempt_level_problems(&keyed, &[0, 1, 2]);
        assert!(problems.is_empty(), "seed {seed}: {problems:#?}");
        let observed = (keyed, final_tree(&wide));
        match &reference {
            None => reference = Some(observed),
            Some(expected) => assert_eq!(
                &observed, expected,
                "seed {seed}: per-key projection or final tree differs"
            ),
        }
    }
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
    entry.display_id = crate::ir::TaskId::from(repair_display_id(1, &parent.display_id).as_str());
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

fn bounded(what: &str, body: impl FnOnce() + Send + 'static) {
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

struct DecliningAtPoll(std::sync::Arc<std::sync::atomic::AtomicUsize>);

impl crate::interaction::AnswerSource for DecliningAtPoll {
    fn id(&self) -> &'static str {
        "declining-at-poll"
    }

    fn resolve(&self, question: &crate::ir::Question) -> Result<crate::ir::Answer, UpstrokeError> {
        self.poll(question)
    }

    fn poll(&self, _question: &crate::ir::Question) -> Result<crate::ir::Answer, UpstrokeError> {
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

const LINEAGE_CANCELLED: &str =
    "failed this verification's lineage and cancelled it: its pipeline had ended";

fn sequence_named(event: &TopologyEvent) -> Option<SequenceId> {
    match &event.body {
        TopologyEventBody::MergeVerificationStarted { data } => Some(data.sequence),
        TopologyEventBody::MergeVerificationUnavailable { data } => Some(data.sequence),
        TopologyEventBody::MergeVerificationInterrupted { data } => Some(data.sequence),
        TopologyEventBody::MergePrepared { data } => Some(data.sequence),
        TopologyEventBody::MergeRejected { data } => Some(data.sequence),
        TopologyEventBody::TaskMerged { data } => Some(data.sequence),
        _ => None,
    }
}

fn cancelled_terminal_after(
    events: &[TopologyEvent],
    carrier: usize,
    sequence: SequenceId,
    detail: &str,
) -> usize {
    let kinds = kinds_of(events);
    let naming: Vec<usize> = (carrier..events.len())
        .filter(|index| events.get(*index).and_then(sequence_named) == Some(sequence))
        .collect();
    let [terminal] = naming.as_slice() else {
        panic!(
            "after the lineage failure the log holds exactly one event naming sequence {}, \
                 its terminal: {kinds:?}",
            sequence.0
        );
    };
    match events.get(*terminal).map(|event| &event.body) {
        Some(TopologyEventBody::MergeVerificationInterrupted { data }) => assert!(
            data.detail.contains(detail),
            "the cancelled verification's terminal says why it was cancelled: {}",
            data.detail
        ),
        other => panic!(
            "sequence {}'s one event after the lineage failure is its interrupted terminal, \
                 not {other:?}",
            sequence.0
        ),
    }
    let opened_later = events
        .get(carrier..*terminal)
        .unwrap_or_default()
        .iter()
        .any(|event| {
            matches!(&event.body,
                TopologyEventBody::MergeVerificationStarted { data } if data.sequence > sequence)
                || matches!(&event.body,
                    TopologyEventBody::MergePrepared { data } if data.sequence > sequence)
                || matches!(&event.body,
                    TopologyEventBody::MergeRejected { data } if data.sequence > sequence)
        });
    assert!(
        !opened_later,
        "no later integration opened before the cancelled verification's terminal: {kinds:?}"
    );
    *terminal
}

fn recorded_before_its_terminal(
    events: &[TopologyEvent],
    carrier: usize,
    sequence: SequenceId,
    detail: &str,
) -> (usize, usize) {
    let kinds = kinds_of(events);
    let records: Vec<usize> = events
        .iter()
        .enumerate()
        .filter(|(_, event)| {
            matches!(&event.body,
                TopologyEventBody::MergeVerificationCharged { data } if data.sequence == sequence)
        })
        .map(|(index, _)| index)
        .collect();
    let [record] = records.as_slice() else {
        panic!(
            "the log holds exactly one `merge_verification_charged` for sequence {}: \
                 {kinds:?}",
            sequence.0
        );
    };
    let terminal = cancelled_terminal_after(events, carrier, sequence, detail);
    assert!(
        carrier < *record && *record < terminal,
        "the record of the passes the cancelled verification charged follows the lineage \
             failure and precedes its one terminal, `merge_verification_interrupted`: {kinds:?}"
    );
    assert_eq!(
        terminals_of(events, sequence),
        vec!["merge_verification_interrupted"],
        "{kinds:?}"
    );
    (*record, terminal)
}

type Carrier = fn(&TopologyRun, TaskKey, CandidateRef) -> Vec<TopologyEventBody>;

fn cancelling<'r>(
    runner: &'r RecordingRunner,
    plant: Carrier,
    mut after_planting: impl FnMut(&Quiescent<'_>) -> Option<Release> + 'r,
) -> Scheduler<'r> {
    let mut planted: Option<Vec<TopologyEventBody>> = None;
    Scheduler::scripted(
        runner,
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
                planted = Some(plant(view.run, TaskKey(1), source));
            }
            if let Some(events) = planted.as_mut() {
                if !events.is_empty() {
                    return Some(Release::Append(Box::new(events.remove(0))));
                }
                return after_planting(view);
            }
            released(view, |invocation| attempt_key(invocation) == Some(1)).or_else(|| {
                released(view, |invocation| {
                    matches!(invocation, InvocationId::Sequence { .. })
                })
            })
        }),
    )
}

#[test]
#[ignore = "spawned by a_kill_at_the_declines_synced_append_resumes_into_the_interrupted_terminal"]
fn cancelled_verification_kill_child() {
    let dir = std::path::PathBuf::from(
        std::env::var_os("UPSTROKE_TEST_KILL_DIR").expect("the parent names the handoff"),
    );
    let tasks = three();
    let armed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut wide = Wide::durable(
        "coordinator-cancelled-kill",
        &tasks,
        3,
        WidePlans::default(),
        holding(&tasks, &[]),
    );
    wide.env.answers = std::sync::Arc::new(DecliningOnceArmed(std::sync::Arc::clone(&armed)));
    hand_the_kill_root_off(&dir, &wide.env.fixture.root);
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let harness = std::sync::Arc::clone(&wide.env.harness);
    let mut scheduler = cancelling(&runner, sibling_parked_on, move |view| {
        if !armed.swap(true, std::sync::atomic::Ordering::SeqCst) {
            harness
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .arm(
                    crate::topology::effects::EffectSiteId::Event(
                        crate::topology::effects::EventSite::Append,
                    ),
                    crate::topology::effects::SubEffectPoint::Synced,
                    crate::topology::effects::InjectionMode::Kill,
                )
                .expect("the Event append supports a kill after its sync");
        }
        released(view, |invocation| attempt_key(invocation) == Some(2))
    });
    let _ = drive(&mut wide, Some(&mut scheduler));
    panic!("the kill at the decline's synced append must have taken this process");
}

#[cfg(unix)]
fn hand_the_kill_root_off(handoff: &std::path::Path, root: &std::path::Path) {
    use std::os::unix::ffi::OsStrExt as _;
    crate::workspace_manager::fixture::write_file(
        &handoff.join(KILL_HANDOFF),
        root.as_os_str().as_bytes(),
    );
}

#[cfg(windows)]
fn hand_the_kill_root_off(handoff: &std::path::Path, root: &std::path::Path) {
    use std::os::windows::ffi::OsStrExt as _;
    let bytes: Vec<u8> = root
        .as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect();
    crate::workspace_manager::fixture::write_file(&handoff.join(KILL_HANDOFF), &bytes);
}

fn attempt_identity(key: u32) -> Identity {
    Identity::Attempt {
        key: TaskKey(key),
        generation: GenerationId(0),
        attempt: AttemptNumber(1),
    }
}

const INC_A: &str = "01KZP5CONTAINERSA000000001";

type InventoryAt = (Vec<String>, Vec<String>, Vec<String>, Vec<String>);

fn container_names_of(wide: &Wide, incarnation: &str, invocations: &[InvocationId]) -> Vec<String> {
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
    let mut views: Vec<String> =
        match std::fs::read_dir(root.join(crate::runner::container::census::VIEWS_DIR)) {
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

const RUN_FOREIGN: &str = "01KZP5FOREIGNRUN0000000001";
const RUN_NOBODY: &str = "01KZP5NOSUCHRUN00000000001";
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
        command: crate::runner::CommandSpec::new("sh")
            .arg("-c")
            .arg("exit 0"),
        workspace: workspace.to_path_buf(),
        role: crate::runner::ExecutionRole::Probe(target),
        timeout: Duration::from_secs(600),
        agent,
        invocation: invocation.expect("a probe identity"),
        credential_profile: None,
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
        "coord-child",
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
    let env =
        crate::engine::topology::scaffold::WideEnv::adopted(root, &tasks, 3, WidePlans::default());
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
        #[cfg(unix)]
        "stranded" => stranded_child(&parent),
        other => panic!("no child role `{other}`"),
    }
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

fn sorted(mut names: Vec<String>) -> Vec<String> {
    names.sort();
    names
}

const F_HOSTED_ROOT: &str = "UPSTROKE_TEST_F_HOSTED_ROOT";

const F_HOSTED_BASE: &str = "UPSTROKE_TEST_F_HOSTED_BASE";

/// The role a Windows stand-in for the scripts above plays, which its
/// shim sets from its first argument, or as `gate`; and the worker's task,
/// its second. The scripts read both from their own arguments.
#[cfg(windows)]
const F_HOSTED_ROLE: &str = "UPSTROKE_TEST_F_ROLE";

#[cfg(windows)]
const F_HOSTED_TASK: &str = "UPSTROKE_TEST_F_TASK";

/// The integration T-ENV-TOPO's run verifies: its second, whose
/// candidate's base is no longer the head once the first has merged.
const F_HOSTED_VERIFIED: SequenceId = SequenceId(1);

/// T-ENV-TOPO's two tasks, writing disjoint paths. At width two the
/// coordinator's first admission dispatches both before it reads any
/// message, so both generations take the start head as their base, and
/// whichever merges second is stale.
fn f_hosted_tasks() -> [WideTask; 2] {
    [
        WideTask::independent("alpha"),
        WideTask::independent("beta"),
    ]
}

/// Where T-ENV-TOPO's run places each role, and how many times. Alpha's
/// worker runs twice in its generation's slot, its first attempt leaving
/// the slot as it was, and beta's once. Each attempt that worked is gated
/// and reviewed in its own snapshots. The stale integration's gate and
/// review run in its integration snapshot and its review snapshot.
fn f_hosted_placements() -> [(&'static str, crate::workspace_manager::Slot, usize); 8] {
    use crate::workspace_manager::{Slot, SnapshotName};
    let task = |key| crate::engine::topology::dispatch::task_slot(TaskKey(key), GenerationId(0));
    let snapshot = |name| Slot::Snapshot { name };
    let verified = u64::from(F_HOSTED_VERIFIED.0);
    [
        ("--implement", task(0), 2),
        ("--implement", task(1), 1),
        ("gate", snapshot(SnapshotName::gates(0, 0, 2)), 1),
        ("--prompt", snapshot(SnapshotName::review(0, 0, 2, 0)), 1),
        ("gate", snapshot(SnapshotName::gates(1, 0, 1)), 1),
        ("--prompt", snapshot(SnapshotName::review(1, 0, 1, 0)), 1),
        ("gate", snapshot(SnapshotName::integration(verified)), 1),
        (
            "--prompt",
            snapshot(SnapshotName::integration_review(verified, 0)),
            1,
        ),
    ]
}

/// A slot instance's directory as `<namespace>/<component>_<tag>`.
fn f_hosted_instance(path: &std::path::Path) -> Option<std::path::PathBuf> {
    let instance = path.file_name()?;
    let namespace = path.parent()?.file_name()?;
    Some(std::path::Path::new(namespace).join(instance))
}

/// The host runner T-ENV-TOPO's and T-NOREQ's children start their roles
/// on. Under C4's absolute value it is `HostRunner::new()`. Under C5's
/// relative value it holds the run's repository, its include written at
/// `<root>/recorded-objects.gitconfig`: under option (C) a role in a work
/// tree the engine made is given the common directory its runner holds in
/// place of an inherited relative value, and a runner that holds none
/// refuses that role before it starts.
fn f_hosted_runner(
    root: &std::path::Path,
    common: &std::path::Path,
    relative: bool,
) -> crate::runner::host::HostRunner {
    if relative {
        let include = root.join("recorded-objects.gitconfig");
        crate::workspace_manager::fixture::write_file(
            &include,
            crate::runner::host::RECORDED_OBJECTS_INCLUDE,
        );
        crate::runner::host::HostRunner::new().with_environment(
            crate::runner::host::HostEnvironment::from_process().reading(
                crate::runner::host::ObjectGraph::RecordedIn(
                    crate::runner::host::ManagedRepository::new(
                        common,
                        &include,
                        crate::runner::host::GitdirRule::native(),
                    )
                    .expect("the run's repository, for the host runner to hold"),
                ),
            ),
        )
    } else {
        crate::runner::host::HostRunner::new()
    }
}

#[test]
#[ignore = "subprocess helper"]
fn hosted_linked_registry_child() {
    let Some(root) = std::env::var_os(F_HOSTED_ROOT).map(std::path::PathBuf::from) else {
        return;
    };
    let below =
        std::path::PathBuf::from(std::env::var_os(F_HOSTED_BASE).expect("the base below the root"));
    let tasks = f_hosted_tasks();
    let env = crate::engine::topology::scaffold::WideEnv::adopted_over(
        crate::workspace_manager::fixture::Fixture::adopt_based(root.clone(), &below),
        &tasks,
        2,
        WidePlans::default(),
    );
    let (_, mut resumed) = env
        .resume(
            "inc-2",
            RecordingRunner::new(),
            crate::engine::topology::select::Ceiling::unlimited(),
        )
        .expect("the run resumes under the linked registry");
    let common = crate::workspace_manager::fixture::canonical(&root.join(&below).join(".git"));
    let relative = std::env::var_os("GIT_COMMON_DIR")
        .is_some_and(|value| std::path::Path::new(&value).is_relative());
    let runner = f_hosted_runner(&root, &common, relative);
    let host: std::sync::Arc<dyn crate::runner::Runner> = std::sync::Arc::new(runner);
    let pipelines = resumed.env.pipelines_over(std::sync::Arc::clone(&host));
    let seams = resumed.env.seams_over(&*host);
    // On Windows the roles are batch shims, so the agents' adapters leave
    // a review's prompt to its stdin alone (`FHostedStdinPrompt`).
    #[cfg(windows)]
    let adapters = std::sync::Arc::new(FHostedStdinPromptAdapters::new());
    #[cfg(windows)]
    let pipelines = PipelineSeams {
        adapters: std::sync::Arc::clone(&adapters)
            as std::sync::Arc<dyn crate::agent::AdapterSource + Send + Sync>,
        ..pipelines
    };
    #[cfg(windows)]
    let seams = crate::engine::topology::run::RunSeams {
        adapters: &*adapters,
        ..seams
    };
    let mut hooks = resumed.env.hooks();
    let progress = resumed
        .run
        .run_concurrently(&seams, &pipelines, &mut hooks, None)
        .expect("the run over the host runner completes");
    assert_eq!(outcome_of(&progress), RunOutcome::Complete);
    assert!(
        matches!(
            progress,
            Progress::Finished {
                execution_root_removed: true,
                ..
            }
        ),
        "finalization removed the execution root, every worktree and registration first"
    );
    assert!(
        root.join("worker-wrote-nothing").exists(),
        "alpha's first attempt's worker left its slot as it was, so its failure was assessed"
    );
    let events = resumed.env.durable_events();
    let kinds = kinds_of(&events);
    let started: Vec<&crate::topology::events::MergeVerificationStarted> = events
        .iter()
        .filter_map(|event| match &event.body {
            TopologyEventBody::MergeVerificationStarted { data } => Some(data),
            _ => None,
        })
        .collect();
    let [verification] = started.as_slice() else {
        panic!("exactly one integration opened a verification: {kinds:?}");
    };
    assert!(
        verification.sequence == F_HOSTED_VERIFIED
            && matches!(
                verification.basis,
                crate::topology::events::VerificationBasis::StaleClean { .. }
            ),
        "the second integration's candidate was stale, and cherry-picked clean: \
             {verification:?}"
    );
    let prepared: Vec<&crate::topology::events::MergePrepared> = events
        .iter()
        .filter_map(|event| match &event.body {
            TopologyEventBody::MergePrepared { data } => Some(&**data),
            _ => None,
        })
        .collect();
    let [fast, stale] = prepared.as_slice() else {
        panic!("both tasks' merges were prepared: {kinds:?}");
    };
    assert!(
        fast.sequence == SequenceId(0)
            && fast.disposition == crate::topology::events::PreparedDisposition::Fast
            && fast.verification.is_none(),
        "the first merge was fast, and verified nothing: {fast:?}"
    );
    assert!(
        stale.sequence == F_HOSTED_VERIFIED
            && stale.key == verification.candidate.key
            && stale.disposition == crate::topology::events::PreparedDisposition::StaleClean
            && stale.verification.as_ref().is_some_and(|record| {
                record.passed() && record.gates_passed && record.reviews.len() == 1
            }),
        "the stale merge was prepared on its passing verification, one gate set and one \
             review: {stale:?}"
    );
    assert_eq!(
        std::collections::BTreeSet::from([fast.key, stale.key]),
        std::collections::BTreeSet::from([TaskKey(0), TaskKey(1)]),
        "one merge of each task: {kinds:?}"
    );
    assert_eq!(count(&events, "task_merged"), 2, "{kinds:?}");
    let canonical_root = crate::workspace_manager::fixture::canonical(&root);
    let depth = below.components().count();
    let named = common
        .strip_prefix(&canonical_root)
        .expect("the common directory is below the root")
        .to_path_buf();
    let log = String::from_utf8(std::fs::read(root.join("roles.log")).expect("the roles' log"))
        .expect("a UTF-8 log");
    let mut placed: BTreeMap<(&str, std::path::PathBuf), usize> = BTreeMap::new();
    for line in log.lines() {
        let (role, cwd) = line.split_once(' ').expect("a role and its directory");
        let instance = f_hosted_instance(std::path::Path::new(cwd))
            .unwrap_or_else(|| panic!("{role} ran in a slot instance: {cwd}"));
        *placed.entry((role, instance)).or_insert(0) += 1;
        if relative {
            assert_eq!(
                std::path::Path::new(cwd)
                    .ancestors()
                    .nth(depth)
                    .map(|top| top.join(&named)),
                Some(common.clone()),
                "the relative `GIT_COMMON_DIR` from {role}'s directory {cwd} names the run's \
                     common directory"
            );
        }
    }
    let manager = &resumed.env.fixture.manager;
    let expected: BTreeMap<(&str, std::path::PathBuf), usize> = f_hosted_placements()
        .into_iter()
        .map(|(role, slot, times)| {
            let instance =
                f_hosted_instance(&manager.slot_path(&slot)).expect("a slot instance's directory");
            ((role, instance), times)
        })
        .collect();
    assert_eq!(
        placed, expected,
        "each role ran over the host runner, in this incarnation's instance of the slot its \
             phase places it in: alpha's worker twice and beta's once, each working attempt's \
             gate and review, and the gate and review of sequence {}'s verification: {log}",
        F_HOSTED_VERIFIED.0
    );
    // The parent owns the tree, and asserts on it after this process
    // exits. The run and every other part of the environment go as usual;
    // only the adopted fixture is forgotten, whose drop would reclaim the
    // parent's root.
    let Wide { run, env } = resumed;
    drop(run);
    let crate::engine::topology::scaffold::WideEnv { fixture, .. } = env;
    std::mem::forget(fixture);
}

/// The Windows stand-in's body: what `F_HOSTED_AGENT` and `F_HOSTED_GATE`
/// do, for the role and task its shim set. A topology module's test writes
/// and spawns nothing itself, so each step goes through the fixture.
/// - **Every role** logs its role and its directory, canonical as `pwd -P`
///   gives it, one line in the shared log.
/// - **A worker** writes `src/<task>/work.txt`, except alpha's first
///   attempt, which leaves its slot as it was.
/// - **A reviewer** writes no output. The script prints the verdict, but
///   the run's adapters answer a review with their own verdict and read no
///   output, so neither changes what the run sees.
/// - **The gate** requires its Git's common directory to be the run's, and
///   `HEAD^{tree}` to be readable. A failed requirement fails this child, so
///   the shim exits nonzero, as the script does.
#[cfg(windows)]
#[test]
#[ignore = "subprocess helper"]
fn hosted_linked_registry_role_child() {
    let Some(role) = std::env::var_os(F_HOSTED_ROLE) else {
        return;
    };
    let role = role.to_string_lossy().into_owned();
    let directory = crate::workspace_manager::fixture::canonical(
        &std::env::current_dir().expect("the role's directory"),
    );
    let log = std::path::PathBuf::from(
        std::env::var_os("UPSTROKE_TEST_F_ROLE_LOG").expect("the roles' log"),
    );
    crate::workspace_manager::fixture::append_line(
        &log,
        &format!("{role} {}", directory.display()),
    );
    match role.as_str() {
        "--implement" => {
            let task = std::env::var(F_HOSTED_TASK).expect("the worker's task");
            let marker = std::path::PathBuf::from(
                std::env::var_os("UPSTROKE_TEST_F_WORKER_MARKER")
                    .expect("the first attempt's marker"),
            );
            if task == "alpha" && !marker.exists() {
                crate::workspace_manager::fixture::write_file(&marker, b"");
                return;
            }
            crate::workspace_manager::fixture::write_file(
                &directory.join("src").join(&task).join("work.txt"),
                format!("{task} worked\n").as_bytes(),
            );
        }
        "--prompt" => {}
        "gate" => {
            let named = crate::workspace_manager::fixture::git(
                &directory,
                &["rev-parse", "--git-common-dir"],
            );
            let run = std::path::PathBuf::from(
                std::env::var_os("UPSTROKE_TEST_F_RUN_COMMON").expect("the run's common directory"),
            );
            assert_eq!(
                crate::workspace_manager::fixture::canonical(&directory.join(named)),
                crate::workspace_manager::fixture::canonical(&run),
                "the gate's Git reads the run's common directory"
            );
            crate::workspace_manager::fixture::git(&directory, &["cat-file", "-e", "HEAD^{tree}"]);
        }
        other => panic!("a role no shim sets: {other}"),
    }
}

/// The scaffold's answering adapter with a review's prompt on its stdin
/// alone, for T-ENV-TOPO's child on Windows. `AnsweringAdapter::build`
/// passes the prompt as the argument after `--prompt`, and the review pass
/// sends it on stdin too (`stdin_payload`). There the roles are batch
/// shims, and std refuses to start a batch file with an argument holding a
/// newline, which a prompt holds. Dropping only that argument leaves the
/// prompt where every shipped adapter sends it. The stand-in reads neither,
/// as the script reads neither, and everything else is the answering
/// adapter's own.
#[cfg(windows)]
struct FHostedStdinPrompt(crate::engine::topology::scaffold::AnsweringAdapter);

#[cfg(windows)]
impl crate::agent::AgentAdapter for FHostedStdinPrompt {
    fn id(&self) -> &'static str {
        crate::agent::AgentAdapter::id(&self.0)
    }

    fn probe(
        &self,
        runner: &dyn crate::runner::Runner,
    ) -> Result<crate::agent::Caps, UpstrokeError> {
        crate::agent::AgentAdapter::probe(&self.0, runner)
    }

    fn build(
        &self,
        run: &crate::agent::TaskRun,
    ) -> Result<crate::runner::CommandSpec, UpstrokeError> {
        let mut spec = crate::agent::AgentAdapter::build(&self.0, run)?;
        let mut args = std::mem::take(&mut spec.args).into_iter();
        while let Some(arg) = args.next() {
            let prompt = arg == "--prompt";
            spec.args.push(arg);
            if prompt {
                // The prompt itself, which the review pass sends on stdin.
                let _on_stdin = args.next();
            }
        }
        Ok(spec)
    }

    fn parse(
        &self,
        out: &crate::agent::ProcessOutput,
    ) -> Result<crate::ir::Outcome, UpstrokeError> {
        crate::agent::AgentAdapter::parse(&self.0, out)
    }

    fn materialize_permissions(
        &self,
        profile: &crate::ir::WorkerProfile,
        gate_cmds: &[String],
        dir: &std::path::Path,
        stem: &str,
    ) -> Result<Option<PathBuf>, UpstrokeError> {
        crate::agent::AgentAdapter::materialize_permissions(&self.0, profile, gate_cmds, dir, stem)
    }
}

/// `ScaffoldAdapters::new()`, each adapter a [`FHostedStdinPrompt`].
#[cfg(windows)]
struct FHostedStdinPromptAdapters {
    primary: FHostedStdinPrompt,
    second: FHostedStdinPrompt,
}

#[cfg(windows)]
impl FHostedStdinPromptAdapters {
    const fn new() -> Self {
        Self {
            primary: FHostedStdinPrompt(
                crate::engine::topology::scaffold::AnsweringAdapter::passing(
                    crate::engine::topology::scaffold::AGENT,
                ),
            ),
            second: FHostedStdinPrompt(
                crate::engine::topology::scaffold::AnsweringAdapter::passing(
                    crate::engine::topology::scaffold::REVIEW_AGENT,
                ),
            ),
        }
    }
}

#[cfg(windows)]
impl crate::agent::AdapterSource for FHostedStdinPromptAdapters {
    fn get(&self, id: &str) -> Option<&dyn crate::agent::AgentAdapter> {
        if id == crate::engine::topology::scaffold::AGENT {
            Some(&self.primary)
        } else if id == crate::engine::topology::scaffold::REVIEW_AGENT {
            Some(&self.second)
        } else {
            None
        }
    }
}

#[cfg(unix)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FNoRequest {
    Outage,
    Damaged,
    Opaque,
}

#[cfg(unix)]
struct FPermitted {
    answering: crate::engine::topology::scaffold::AnsweringAdapter,
    production: &'static dyn crate::agent::AgentAdapter,
}

#[cfg(unix)]
impl crate::agent::AgentAdapter for FPermitted {
    fn id(&self) -> &'static str {
        crate::agent::AgentAdapter::id(&self.answering)
    }

    fn probe(
        &self,
        runner: &dyn crate::runner::Runner,
    ) -> Result<crate::agent::Caps, UpstrokeError> {
        crate::agent::AgentAdapter::probe(&self.answering, runner)
    }

    fn build(
        &self,
        run: &crate::agent::TaskRun,
    ) -> Result<crate::runner::CommandSpec, UpstrokeError> {
        crate::agent::AgentAdapter::build(&self.answering, run)
    }

    fn parse(
        &self,
        out: &crate::agent::ProcessOutput,
    ) -> Result<crate::ir::Outcome, UpstrokeError> {
        crate::agent::AgentAdapter::parse(&self.answering, out)
    }

    fn materialize_permissions(
        &self,
        profile: &crate::ir::WorkerProfile,
        gate_cmds: &[String],
        dir: &std::path::Path,
        stem: &str,
    ) -> Result<Option<PathBuf>, UpstrokeError> {
        crate::agent::AgentAdapter::materialize_permissions(
            self.production,
            profile,
            gate_cmds,
            dir,
            stem,
        )
    }
}

#[cfg(unix)]
struct FPermittedAdapters {
    primary: FPermitted,
    second: FPermitted,
}

#[cfg(unix)]
impl FPermittedAdapters {
    fn new() -> Self {
        Self {
            primary: FPermitted {
                answering: crate::engine::topology::scaffold::AnsweringAdapter::passing(
                    crate::engine::topology::scaffold::AGENT,
                ),
                production: &crate::agent::claude::ClaudeCodeAdapter,
            },
            second: FPermitted {
                answering: crate::engine::topology::scaffold::AnsweringAdapter::passing(
                    crate::engine::topology::scaffold::REVIEW_AGENT,
                ),
                production: &crate::agent::copilot::CopilotAdapter,
            },
        }
    }
}

#[cfg(unix)]
impl crate::agent::AdapterSource for FPermittedAdapters {
    fn get(&self, id: &str) -> Option<&dyn crate::agent::AgentAdapter> {
        if id == crate::engine::topology::scaffold::AGENT {
            Some(&self.primary)
        } else if id == crate::engine::topology::scaffold::REVIEW_AGENT {
            Some(&self.second)
        } else {
            None
        }
    }
}

#[cfg(unix)]
struct FDamagingReviews;

#[cfg(unix)]
impl crate::engine::topology::attempt::ReviewPasses for FDamagingReviews {
    fn run(
        &self,
        cx: &crate::review::ReviewCx<'_>,
        runner: &dyn crate::runner::Runner,
        invocations: &crate::review::ReviewInvocations,
    ) -> Result<crate::review::ReviewOutcome, UpstrokeError> {
        if matches!(
            &invocations.pass,
            InvocationId::Sequence {
                sequence,
                role: crate::runner::invocation::SequenceRole::ReviewPass(0),
                ..
            } if *sequence == F_HOSTED_VERIFIED
        ) {
            crate::workspace_manager::fixture::remove_file(&f_entry_of(cx.workspace).join("HEAD"));
        }
        crate::engine::topology::attempt::ReviewPasses::run(
            &crate::engine::attempt::LegacyReviewPasses,
            cx,
            runner,
            invocations,
        )
    }
}

#[cfg(unix)]
struct FOpaqueReviewer {
    host: crate::runner::host::HostRunner,
}

#[cfg(unix)]
impl crate::runner::Runner for FOpaqueReviewer {
    fn run<'a>(
        &'a self,
        request: &'a crate::runner::RunnerRequest,
        call: crate::runner::RunnerCall<'a>,
    ) -> crate::runner::RunFuture<'a> {
        if !matches!(request.role, crate::runner::ExecutionRole::Review) {
            return self.host.run(request, call);
        }
        let never: Result<crate::agent::ProcessOutput, crate::runner::RunnerError> =
            Err(crate::runner::RunnerError::never_started(
                &request.invocation,
                UpstrokeError::Agent {
                    message: "the opaque reviewer starts no process".to_owned(),
                },
            ));
        Box::pin(async move { never })
    }

    fn composed_common_dir(
        &self,
        request: &crate::runner::RunnerRequest,
    ) -> crate::workspace_manager::ComposedCommonDir {
        if matches!(request.role, crate::runner::ExecutionRole::Review) {
            crate::workspace_manager::ComposedCommonDir::Unknown
        } else {
            self.host.composed_common_dir(request)
        }
    }
}

#[cfg(unix)]
fn f_hosted_no_request_child(case: FNoRequest) {
    use crate::topology::events::{
        InfrastructureKind, MergePrepared, MergeVerificationStarted, MergeVerificationUnavailable,
        PreparedDisposition, UnavailableCause, UnavailableOutcome, VerificationBasis,
    };
    use crate::workspace_manager::{Slot, SnapshotName};

    let Some(root) = std::env::var_os(F_HOSTED_ROOT).map(std::path::PathBuf::from) else {
        return;
    };
    let below =
        std::path::PathBuf::from(std::env::var_os(F_HOSTED_BASE).expect("the base below the root"));
    let tasks = f_hosted_tasks();
    let env = crate::engine::topology::scaffold::WideEnv::adopted_over(
        crate::workspace_manager::fixture::Fixture::adopt_based(root.clone(), &below),
        &tasks,
        2,
        WidePlans::default(),
    );
    let (_, mut resumed) = env
        .resume(
            "inc-2",
            RecordingRunner::new(),
            crate::engine::topology::select::Ceiling::unlimited(),
        )
        .expect("the run resumes under the linked registry");
    let obstructed = resumed
        .env
        .paths
        .settings()
        .join(format!("integration-s{}-review.json", F_HOSTED_VERIFIED.0));
    crate::workspace_manager::fixture::create_dir(&obstructed);
    let common =
        std::fs::canonicalize(root.join(&below).join(".git")).expect("the run's common directory");
    let relative = std::env::var_os("GIT_COMMON_DIR")
        .is_some_and(|value| std::path::Path::new(&value).is_relative());
    let host = f_hosted_runner(&root, &common, relative);
    let runner: Arc<dyn crate::runner::Runner> = if case == FNoRequest::Opaque {
        Arc::new(FOpaqueReviewer { host })
    } else {
        Arc::new(host)
    };
    let adapters = Arc::new(FPermittedAdapters::new());
    let damaging = case == FNoRequest::Damaged;
    let mut pipelines = resumed.env.pipelines_over(Arc::clone(&runner));
    pipelines.adapters =
        Arc::clone(&adapters) as Arc<dyn crate::agent::AdapterSource + Send + Sync>;
    if damaging {
        pipelines.reviews = Arc::new(FDamagingReviews);
    }
    let seams = crate::engine::topology::run::RunSeams {
        adapters: &*adapters,
        reviews: if damaging {
            &FDamagingReviews
        } else {
            &crate::engine::attempt::LegacyReviewPasses
        },
        ..resumed.env.seams_over(&*runner)
    };
    let mut hooks = resumed.env.hooks();
    let result = resumed
        .run
        .run_concurrently(&seams, &pipelines, &mut hooks, None);
    drop(hooks);
    assert!(
        obstructed.is_dir(),
        "the permission file's path is still the directory planted there"
    );
    let events = resumed.env.durable_events();
    let kinds = kinds_of(&events);
    let canonical_root = std::fs::canonicalize(&root).expect("the canonical scratch root");
    let depth = below.components().count();
    let named = common
        .strip_prefix(&canonical_root)
        .expect("the common directory is below the root")
        .to_path_buf();
    let log = String::from_utf8(std::fs::read(root.join("roles.log")).expect("the roles' log"))
        .expect("a UTF-8 log");
    let mut placed: BTreeMap<(&str, std::path::PathBuf), usize> = BTreeMap::new();
    for line in log.lines() {
        let (role, cwd) = line.split_once(' ').expect("a role and its directory");
        let instance = f_hosted_instance(std::path::Path::new(cwd))
            .unwrap_or_else(|| panic!("{role} ran in a slot instance: {cwd}"));
        *placed.entry((role, instance)).or_insert(0) += 1;
        if relative {
            assert_eq!(
                std::path::Path::new(cwd)
                    .ancestors()
                    .nth(depth)
                    .map(|top| top.join(&named)),
                Some(common.clone()),
                "the relative `GIT_COMMON_DIR` from {role}'s directory {cwd} names the run's \
                     common directory"
            );
        }
    }
    let manager = &resumed.env.fixture.manager;
    let verified = u64::from(F_HOSTED_VERIFIED.0);
    let snapshot = |name| Slot::Snapshot { name };
    let instance_of = |slot: &Slot| {
        f_hosted_instance(&manager.slot_path(slot)).expect("a slot instance's directory")
    };
    let unheard = instance_of(&snapshot(SnapshotName::integration_review(verified, 0)));
    match case {
        FNoRequest::Outage => {
            let progress = result.expect("the run over the host runner completes");
            assert_eq!(outcome_of(&progress), RunOutcome::Complete);
            assert!(
                matches!(
                    progress,
                    Progress::Finished {
                        execution_root_removed: true,
                        ..
                    }
                ),
                "finalization removed every worktree and registration, then the execution root"
            );
            let unavailable: Vec<&MergeVerificationUnavailable> = events
                .iter()
                .filter_map(|event| match &event.body {
                    TopologyEventBody::MergeVerificationUnavailable { data } => Some(data),
                    _ => None,
                })
                .collect();
            let [deferred] = unavailable.as_slice() else {
                panic!("exactly one verification was unavailable: {kinds:?}");
            };
            assert!(
                deferred.sequence == F_HOSTED_VERIFIED
                    && matches!(
                        deferred.cause,
                        UnavailableCause::Infrastructure {
                            kind: InfrastructureKind::ReviewUnavailable,
                        }
                    )
                    && matches!(deferred.outcome, UnavailableOutcome::Deferred { defers: 1 }),
                "sequence 1's reviewer outage took the ordinary unavailable route, deferred \
                     once: {deferred:?}"
            );
            let [charged] = deferred.reviews.as_slice() else {
                panic!("the outage's one review pass was charged: {deferred:?}");
            };
            assert!(
                matches!(
                    charged.outcome,
                    crate::events::ReviewPassOutcome::Unavailable
                ) && charged.cost_usd.is_none(),
                "charged as unavailable, with no cost: {charged:?}"
            );
            let started: Vec<&MergeVerificationStarted> = events
                .iter()
                .filter_map(|event| match &event.body {
                    TopologyEventBody::MergeVerificationStarted { data } => Some(data),
                    _ => None,
                })
                .collect();
            let [first, again] = started.as_slice() else {
                panic!("the deferred integration was verified again: {kinds:?}");
            };
            assert!(
                first.sequence == F_HOSTED_VERIFIED
                    && again.sequence != F_HOSTED_VERIFIED
                    && again.candidate.key == first.candidate.key
                    && matches!(first.basis, VerificationBasis::StaleClean { .. })
                    && matches!(again.basis, VerificationBasis::StaleClean { .. }),
                "one candidate's stale integration, verified at sequence 1 and again: \
                     {started:?}"
            );
            let outage = kinds
                .iter()
                .position(|seen| *seen == "merge_verification_unavailable");
            let waited = kinds.iter().position(|seen| *seen == "defer_wait_elapsed");
            let verified_again = kinds
                .iter()
                .rposition(|seen| *seen == "merge_verification_started");
            assert!(
                outage.is_some() && outage < waited && waited < verified_again,
                "the outage, then the backoff's wait, then the verification again: {kinds:?}"
            );
            let prepared: Vec<&MergePrepared> = events
                .iter()
                .filter_map(|event| match &event.body {
                    TopologyEventBody::MergePrepared { data } => Some(&**data),
                    _ => None,
                })
                .collect();
            let [fast, stale] = prepared.as_slice() else {
                panic!("both tasks' merges were prepared: {kinds:?}");
            };
            assert!(
                fast.sequence == SequenceId(0)
                    && fast.disposition == PreparedDisposition::Fast
                    && stale.sequence == again.sequence
                    && stale.key == first.candidate.key
                    && stale.disposition == PreparedDisposition::StaleClean
                    && stale.verification.as_ref().is_some_and(|record| {
                        record.passed() && record.gates_passed && record.reviews.len() == 1
                    }),
                "the first merge was fast, and the deferred candidate's was prepared on its \
                     passing verification when verified again: {prepared:?}"
            );
            assert_eq!(count(&events, "task_merged"), 2, "{kinds:?}");
            assert!(
                resumed.run.invocations_balance(),
                "every request the run admitted was ended"
            );
            let retried = u64::from(again.sequence.0);
            let expected: BTreeMap<(&str, std::path::PathBuf), usize> = f_hosted_placements()
                .into_iter()
                .filter(|(_, slot, _)| instance_of(slot) != unheard)
                .chain([
                    ("gate", snapshot(SnapshotName::integration(retried)), 1),
                    (
                        "--prompt",
                        snapshot(SnapshotName::integration_review(retried, 0)),
                        1,
                    ),
                ])
                .map(|(role, slot, times)| ((role, instance_of(&slot)), times))
                .collect();
            assert_eq!(
                placed, expected,
                "each role ran over the host runner in its phase's slot, sequence 1's review \
                     pass ran no process, and the verification again ran its gate and review: {log}"
            );
        }
        FNoRequest::Damaged => {
            let message = match result {
                Err(UpstrokeError::RegistryRefused { message }) => message,
                other => panic!("the damaged registration is refused: {other:?}: {kinds:?}"),
            };
            assert!(
                message.contains("the verdict of review pass 0")
                    && message.contains("HEAD cannot be read"),
                "the refusal names the pass and the damage: {message}"
            );
            assert_eq!(
                count(&events, "merge_verification_unavailable"),
                0,
                "{kinds:?}"
            );
            assert!(
                events.iter().any(|event| matches!(
                    &event.body,
                    TopologyEventBody::MergeVerificationStarted { data }
                        if data.sequence == F_HOSTED_VERIFIED
                )),
                "sequence 1's verification began: {kinds:?}"
            );
            let gated = instance_of(&snapshot(SnapshotName::integration(verified)));
            assert_eq!(
                placed.get(&("gate", gated)),
                Some(&1),
                "the verification's gate ran: {log}"
            );
            assert!(
                !placed.keys().any(|(_, instance)| *instance == unheard),
                "the review pass handed its runner no request: {log}"
            );
        }
        FNoRequest::Opaque => {
            let message = match result {
                Err(UpstrokeError::RegistryRefused { message }) => message,
                other => panic!("the opaque reviewer's pass is refused: {other:?}: {kinds:?}"),
            };
            assert!(
                message.contains("the verdict of review pass 0")
                    && message.contains("which is not this repository's common git dir"),
                "an admitted request whose runner reported nothing keeps the original rule: \
                     {message}"
            );
            assert!(
                !placed.keys().any(|(role, _)| *role == "--prompt"),
                "no review process started: {log}"
            );
        }
    }
    let Wide { run, env } = resumed;
    drop(run);
    let crate::engine::topology::scaffold::WideEnv { fixture, .. } = env;
    std::mem::forget(fixture);
}

#[cfg(unix)]
#[test]
#[ignore = "subprocess helper"]
fn hosted_no_request_outage_child() {
    f_hosted_no_request_child(FNoRequest::Outage);
}

#[cfg(unix)]
#[test]
#[ignore = "subprocess helper"]
fn hosted_no_request_damaged_child() {
    f_hosted_no_request_child(FNoRequest::Damaged);
}

#[cfg(unix)]
#[test]
#[ignore = "subprocess helper"]
fn hosted_no_request_opaque_child() {
    f_hosted_no_request_child(FNoRequest::Opaque);
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
        let pidfile = self
            .pids
            .join(format!("{}.pid", request.invocation.render()));
        let held = crate::runner::RunnerRequest {
            command: crate::runner::CommandSpec::new("sh")
                .arg("-c")
                .arg("echo $$ > \"$1\"; exec sleep 600")
                .arg("held")
                .arg(pidfile.to_string_lossy()),
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
fn censused(host: &crate::runner::container::FakeRuntime) -> usize {
    host.journal()
        .iter()
        .filter(|entry| entry.op == crate::runner::container::runtime::RuntimeOp::ListByLabel)
        .count()
}

#[cfg(unix)]
fn stranded_child(parent: &crate::engine::topology::scaffold::ParentSide) {
    let incarnation = child_env("UPSTROKE_TEST_CHILD_INCARNATION");
    let word = std::path::PathBuf::from(child_env("UPSTROKE_TEST_CHILD_WORD"));
    let tasks = three();
    let mut wide = Wide::durable_contained(
        "coordinator-child-stranded",
        &tasks,
        3,
        WidePlans::default(),
        RecordingRunner::new(),
        &incarnation,
    );
    parent.event(&serde_json::json!({
        "root": wide.env.fixture.root.to_string_lossy(),
        "private": wide.env.fixture.private.to_string_lossy(),
    }));
    let runner: std::sync::Arc<dyn crate::runner::Runner> =
        std::sync::Arc::new(crate::engine::topology::scaffold::container_runner(
            wide.env.identity(&incarnation),
            &wide.env.fixture.base,
            Box::new(parent.runtime()),
            Duration::from_millis(10),
        ));
    let pipelines = wide.env.pipelines_over(std::sync::Arc::clone(&runner));
    let mut hooks = wide.env.hooks();
    let ended =
        wide.run
            .run_concurrently(&wide.env.seams_over(&*runner), &pipelines, &mut hooks, None);
    parent.event(&serde_json::json!({
        "returned": match &ended {
            Ok(progress) => format!("{progress:?}"),
            Err(error) => error.to_string(),
        },
        "balanced": wide.run.invocations_balance(),
    }));
    drop(hooks);
    drop(pipelines);
    drop(runner);
    parent.event(&serde_json::json!({"dropped": true}));
    let started = std::time::Instant::now();
    while !word.exists() && started.elapsed() < BOUND {
        crate::workspace_manager::fixture::rest_within(
            Duration::from_millis(20),
            BOUND.saturating_sub(started.elapsed()),
        );
    }
    std::process::exit(0);
}

const PASS_COST: f64 = 0.25;

const DUPLICATE_OFFER: &str = "offered for a second merge_verification_charged";

const RECORD_HANDOFF: &str = "o14-fixture-root";

const RECORD_KILL_TAG: &str = "record-kill";

fn reviewing(verify_reviewers: usize) -> WidePlans {
    WidePlans {
        verify_reviewers,
        ..WidePlans::default()
    }
}

fn two() -> [WideTask; 2] {
    [
        WideTask::independent("alpha"),
        WideTask::independent("beta"),
    ]
}

fn unlimited() -> crate::engine::topology::select::Ceiling {
    crate::engine::topology::select::Ceiling::unlimited()
}

fn answering_two() -> RecordingRunner {
    RecordingRunner::new().answering(wide_responder(&two(), &[]))
}

fn charged_records(events: &[TopologyEvent]) -> Vec<(SequenceId, Vec<ReviewRecord>)> {
    events
        .iter()
        .filter(|event| event.body.kind() == "merge_verification_charged")
        .map(|event| {
            let value = serde_json::to_value(event).expect("an event serializes");
            let sequence = value["data"]["sequence"]
                .as_u64()
                .and_then(|sequence| u32::try_from(sequence).ok())
                .map(SequenceId)
                .expect("a record names its sequence");
            let reviews = serde_json::from_value(value["data"]["reviews"].clone())
                .expect("a record carries review records");
            (sequence, reviews)
        })
        .collect()
}

fn started_key(events: &[TopologyEvent], sequence: SequenceId) -> TaskKey {
    events
        .iter()
        .find_map(|event| match &event.body {
            TopologyEventBody::MergeVerificationStarted { data } if data.sequence == sequence => {
                Some(data.candidate.key)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("sequence {} has a start", sequence.0))
}

fn unrecorded(events: &[TopologyEvent]) -> Vec<TopologyEvent> {
    events
        .iter()
        .filter(|event| event.body.kind() != "merge_verification_charged")
        .cloned()
        .collect()
}

fn terminals_of(events: &[TopologyEvent], sequence: SequenceId) -> Vec<&'static str> {
    events
        .iter()
        .filter(|event| {
            event.body.sequence() == Some(sequence)
                && matches!(
                    event.body,
                    TopologyEventBody::MergeVerificationUnavailable { .. }
                        | TopologyEventBody::MergeVerificationInterrupted { .. }
                        | TopologyEventBody::MergePrepared { .. }
                        | TopologyEventBody::MergeRejected { .. }
                )
        })
        .map(|event| event.body.kind())
        .collect()
}

fn two_passes_recorded(wide: &Wide, sequence: SequenceId) {
    let events = wide.env.durable_events();
    let kinds = kinds_of(&events);
    let records = charged_records(&events);
    assert_eq!(
        records
            .iter()
            .map(|(recorded, _)| *recorded)
            .collect::<Vec<_>>(),
        vec![sequence],
        "one merge_verification_charged, for sequence {}: {kinds:?}",
        sequence.0
    );
    let passes: Vec<(String, Option<f64>)> = records
        .iter()
        .flat_map(|(_, reviews)| reviews.iter())
        .map(|review| (review.model.clone(), review.cost_usd))
        .collect();
    assert_eq!(
        passes,
        vec![
            ("scaffold-model".to_owned(), Some(PASS_COST)),
            ("scaffold-second-model".to_owned(), Some(PASS_COST))
        ],
        "the record holds both passes, in the order they were charged"
    );
    let key = started_key(&events, sequence);
    let replayed = crate::engine::topology::select::Spend::replay(&events);
    let without = crate::engine::topology::select::Spend::replay(&unrecorded(&events));
    let live = wide.run.spend();
    for (what, replayed, without, live) in [
        ("run", replayed.run_usd(), without.run_usd(), live.run_usd()),
        (
            "task",
            replayed.task_usd(key),
            without.task_usd(key),
            live.task_usd(key),
        ),
    ] {
        assert!(
            (replayed - live).abs() < 1e-9,
            "the {what}'s replayed total {replayed} is its live total {live}"
        );
        assert!(
            (replayed - without - 2.0 * PASS_COST).abs() < 1e-9,
            "the record carries the {what}'s two passes, $0.50: {without} -> {replayed}"
        );
    }
}

fn no_second_offer(wide: &Wide) {
    assert!(
        !wide
            .run
            .warnings()
            .iter()
            .any(|warning| warning.contains(DUPLICATE_OFFER)),
        "no second record was offered: {:?}",
        wide.run.warnings()
    );
}

fn records_both_passes(wide: &Wide, sequence: SequenceId) {
    two_passes_recorded(wide, sequence);
    no_second_offer(wide);
}

fn stale_at_width_one(
    tag: &str,
    verify_reviewers: usize,
    ceiling: crate::engine::topology::select::Ceiling,
) -> Wide {
    let tasks = two();
    let mut wide = Wide::durable(
        tag,
        &tasks,
        3,
        reviewing(verify_reviewers),
        holding(&tasks, &[]),
    );
    let runner = std::sync::Arc::clone(&wide.env.runner);
    let mut shut = false;
    let mut scheduler = Scheduler::scripted(
        &runner,
        Box::new(move |view: &Quiescent<'_>| {
            if shut {
                return None;
            }
            if view
                .live
                .iter()
                .any(|(_, identity)| matches!(identity, Identity::Verification { .. }))
            {
                shut = true;
                assert!(view.injector.inject(ToCoordinator::Shutdown));
                return Some(Release::Injected);
            }
            released(view, |invocation| attempt_key(invocation) == Some(0))
        }),
    );
    let error = drive(&mut wide, Some(&mut scheduler)).expect_err("the shutdown ends the command");
    drop(scheduler);
    assert!(error.to_string().contains("shut down"), "{error}");
    let events = wide.env.durable_events();
    assert_eq!(count(&events, "task_merged"), 1, "{:?}", kinds_of(&events));
    assert_eq!(
        kinds_of(&events).last(),
        Some(&"merge_verification_started"),
        "the first verification was shut down at its gate, before any pass: {:?}",
        kinds_of(&events)
    );
    let (_, resumed) = wide
        .resume("inc-2", answering_two(), ceiling)
        .expect("the next process resumes the shut-down run");
    assert_eq!(
        terminals_of(&resumed.env.durable_events(), SequenceId(1)),
        vec!["merge_verification_interrupted"],
        "the resume settles the shut-down verification interrupted: {:?}",
        kinds_of_log(&resumed)
    );
    resumed
}

fn snapshot_add() -> crate::topology::effects::EffectSiteId {
    crate::topology::effects::EffectSiteId::Snapshot(crate::topology::effects::SnapshotSite::Add)
}

fn failing_the_snapshot_after(wide: &Wide, passing: u32) -> CasFailing {
    let mut hooks = CasFailing::over(wide);
    hooks.effects.fail_at = Some((snapshot_add(), passing));
    hooks
}

fn verifying(fold: &TopologyFold, sequence: SequenceId) -> bool {
    fold.transaction().is_some_and(|open| {
        open.sequence == sequence
            && matches!(
                open.class,
                crate::topology::fold::TransactionClass::VerificationStarted { .. }
            )
    })
}

fn review_pass(invocation: &InvocationId, pass: u32) -> bool {
    matches!(
        invocation,
        InvocationId::Sequence {
            role: crate::runner::invocation::SequenceRole::ReviewPass(index),
            ..
        } if *index == pass
    )
}

fn sequence_invocation(invocation: &InvocationId) -> bool {
    matches!(invocation, InvocationId::Sequence { .. })
}

fn verification_live(view: &Quiescent<'_>) -> bool {
    view.live
        .iter()
        .any(|(_, identity)| matches!(identity, Identity::Verification { .. }))
}

struct ArmingOnTheFault {
    inner: crate::engine::topology::seams::HarnessTopologyHooks,
    effects: FailingOnce,
    harness: std::sync::Arc<std::sync::Mutex<crate::topology::effects::HookHarness>>,
    arm: Option<(
        crate::topology::effects::SubEffectPoint,
        crate::topology::effects::InjectionMode,
    )>,
    abort_at_the_next_append: bool,
}

impl ArmingOnTheFault {
    fn over(
        wide: &Wide,
        passing: u32,
        arm: Option<(
            crate::topology::effects::SubEffectPoint,
            crate::topology::effects::InjectionMode,
        )>,
    ) -> Self {
        Self {
            inner: wide.env.hooks(),
            effects: FailingOnce {
                effects: crate::workspace_manager::HarnessEffects::new(std::sync::Arc::clone(
                    &wide.env.harness,
                )),
                fail_at: Some((snapshot_add(), passing)),
            },
            harness: std::sync::Arc::clone(&wide.env.harness),
            arm,
            abort_at_the_next_append: false,
        }
    }

    fn fired(&self) -> bool {
        self.effects.fail_at.is_none()
    }
}

impl crate::workspace_manager::EffectHooks for ArmingOnTheFault {
    fn phase(
        &mut self,
        site: crate::topology::effects::EffectSiteId,
        phase: crate::topology::effects::HookPhase,
    ) -> crate::topology::effects::Injection {
        let answer = self.effects.phase(site, phase);
        if matches!(answer, crate::topology::effects::Injection::Error) {
            if let Some((point, mode)) = self.arm.take() {
                self.harness
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .arm(
                        crate::topology::effects::EffectSiteId::Event(
                            crate::topology::effects::EventSite::Append,
                        ),
                        point,
                        mode,
                    )
                    .expect("the Event append supports the mode at the point");
            }
        }
        answer
    }

    fn durability_ledger(&self) -> crate::util::DurabilityLedger {
        self.effects.durability_ledger()
    }

    fn refusal_cause(&self) -> Option<String> {
        self.effects.refusal_cause()
    }
}

impl crate::events::log::EventHooks for ArmingOnTheFault {
    fn phase(
        &mut self,
        site: crate::topology::effects::EventSite,
        phase: crate::topology::effects::HookPhase,
    ) {
        if self.abort_at_the_next_append
            && self.fired()
            && site == crate::topology::effects::EventSite::Append
            && phase == crate::topology::effects::HookPhase::Before
        {
            std::process::abort();
        }
        self.inner.events().phase(site, phase);
    }

    fn point(
        &mut self,
        site: crate::topology::effects::EventSite,
        point: crate::topology::effects::SubEffectPoint,
        mode: crate::topology::effects::InjectionMode,
    ) -> crate::topology::effects::Injection {
        self.inner.events().point(site, point, mode)
    }

    fn written_kill_shape(
        &mut self,
        site: crate::topology::effects::EventSite,
    ) -> crate::events::log::WrittenShape {
        self.inner.events().written_kill_shape(site)
    }

    fn durability_ledger(&self) -> crate::util::DurabilityLedger {
        crate::events::log::EventHooks::durability_ledger(self.inner.event_observer())
    }

    fn synced(&mut self, record: &crate::events::log::SyncRecord) {
        self.inner.events().synced(record);
    }
}

impl TopologyHooks for ArmingOnTheFault {
    fn effects(&mut self) -> &mut dyn crate::workspace_manager::EffectHooks {
        self
    }

    fn rundir(&mut self) -> &mut dyn crate::rundir::RunDirHooks {
        self.inner.rundir()
    }

    fn events(&mut self) -> &mut dyn crate::events::log::EventHooks {
        self
    }

    fn container(&mut self) -> &mut dyn crate::runner::container::ContainerHooks {
        self.inner.container()
    }

    fn spawn(&mut self) -> &mut dyn crate::agent::proc::SpawnHooks {
        self.inner.spawn()
    }

    fn folded(&mut self, fold: &TopologyFold, events: &[TopologyEvent]) {
        self.inner.folded(fold, events);
    }
}

#[test]
fn a_second_recording_of_a_sequence_is_refused_before_its_line_and_warned() {
    let mut wide = stale_at_width_one("coordinator-o14-r19", 3, unlimited());
    let mut hooks = failing_the_snapshot_after(&wide, 3);
    let seams = wide.env.seams();
    wide.run
        .step(&seams, &mut hooks)
        .expect_err("the third reviewer's snapshot add fails");
    let sequence = SequenceId(2);
    let (_, reviews) = charged_records(&wide.env.durable_events())
        .into_iter()
        .next()
        .expect("the record");
    let lines = wide.env.durable_events().len();
    wide.run
        .record_charged(
            super::super::run::Charged { sequence, reviews },
            &seams,
            &mut hooks,
        )
        .expect("a refused second offer is a warning, not an error");
    assert_eq!(
        wide.env.durable_events().len(),
        lines,
        "nothing was appended for the second offer"
    );
    assert!(
        wide.run
            .warnings()
            .iter()
            .any(|warning| warning.contains(DUPLICATE_OFFER)),
        "the refused second offer is warned of: {:?}",
        wide.run.warnings()
    );
    two_passes_recorded(&wide, sequence);
}

#[test]
#[ignore = "spawned by the charged record's kill tests"]
fn charged_record_kill_child() {
    let dir = std::path::PathBuf::from(
        std::env::var("UPSTROKE_TEST_KILL_DIR").expect("the parent names the handoff"),
    );
    let torn = match std::env::var("UPSTROKE_TEST_KILL_SHAPE").as_deref() {
        Ok("before") => false,
        Ok("torn") => true,
        other => panic!("the parent names the kill shape: {other:?}"),
    };
    let mut wide = stale_at_width_one(RECORD_KILL_TAG, 3, unlimited());
    crate::workspace_manager::fixture::write_file(
        &dir.join(RECORD_HANDOFF),
        wide.env.fixture.root.to_string_lossy().as_bytes(),
    );
    let mut hooks = if torn {
        let mut hooks = ArmingOnTheFault::over(
            &wide,
            3,
            Some((
                crate::topology::effects::SubEffectPoint::Written,
                crate::topology::effects::InjectionMode::Kill,
            )),
        );
        hooks.inner = wide
            .env
            .hooks()
            .with_written_kill_shape(crate::events::log::WrittenShape::Torn);
        hooks
    } else {
        let mut hooks = ArmingOnTheFault::over(&wide, 3, None);
        hooks.abort_at_the_next_append = true;
        hooks
    };
    let seams = wide.env.seams();
    let _ = wide.run.step(&seams, &mut hooks);
    panic!("the kill at the record's append must have taken this process");
}

struct PlantingAtTheSnapshot {
    effects: crate::workspace_manager::HarnessEffects,
    planter: Arc<dyn ReviewInputPolicy + Send + Sync>,
    passing: Option<u32>,
}

impl crate::workspace_manager::EffectHooks for PlantingAtTheSnapshot {
    fn phase(
        &mut self,
        site: crate::topology::effects::EffectSiteId,
        phase: crate::topology::effects::HookPhase,
    ) -> crate::topology::effects::Injection {
        if site == snapshot_add() && phase == crate::topology::effects::HookPhase::Before {
            match self.passing {
                Some(0) => {
                    self.passing = None;
                    self.planter
                        .problem(&PathBuf::from("merge").join("third-review"), "")
                        .expect("the fault is planted before the third reviewer's snapshot");
                }
                Some(left) => self.passing = Some(left - 1),
                None => {}
            }
        }
        self.effects.phase(site, phase)
    }

    fn durability_ledger(&self) -> crate::util::DurabilityLedger {
        self.effects.durability_ledger()
    }

    fn refusal_cause(&self) -> Option<String> {
        self.effects.refusal_cause()
    }
}

struct PlantingHooks {
    inner: crate::engine::topology::seams::HarnessTopologyHooks,
    effects: PlantingAtTheSnapshot,
}

impl PlantingHooks {
    fn new(
        harness: &Arc<std::sync::Mutex<crate::topology::effects::HookHarness>>,
        planter: &Arc<dyn ReviewInputPolicy + Send + Sync>,
        passing: u32,
    ) -> Self {
        Self {
            inner: crate::engine::topology::seams::HarnessTopologyHooks::new(Arc::clone(harness)),
            effects: PlantingAtTheSnapshot {
                effects: crate::workspace_manager::HarnessEffects::new(Arc::clone(harness)),
                planter: Arc::clone(planter),
                passing: Some(passing),
            },
        }
    }
}

impl TopologyHooks for PlantingHooks {
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

    fn folded(&mut self, fold: &TopologyFold, events: &[TopologyEvent]) {
        self.inner.folded(fold, events);
    }
}

fn refused_by_the_tear(error: &UpstrokeError) {
    assert!(
        matches!(error, UpstrokeError::RegistryRefused { message } if message.contains("commondir")),
        "the third reviewer's registry access refuses on the torn registration: {error:?}"
    );
}

fn untear(admin: &std::path::Path) {
    for file in ["HEAD", "gitdir", "commondir"] {
        crate::workspace_manager::fixture::remove_file(&admin.join(file));
    }
    crate::workspace_manager::fixture::remove_dir(admin);
}

fn resumed_spend_is_the_live_one(
    resumed: &Wide,
    key: TaskKey,
    live: &crate::engine::topology::select::Spend,
    sequence: SequenceId,
) {
    let events = resumed.env.durable_events();
    let kinds = kinds_of(&events);
    assert_eq!(
        terminals_of(&events, sequence),
        vec!["merge_verification_interrupted"],
        "recovery settles the refused sequence interrupted: {kinds:?}"
    );
    let of_sequence = |kind: &str| {
        events
            .iter()
            .position(|event| event.body.kind() == kind && event.body.sequence() == Some(sequence))
            .unwrap_or_else(|| panic!("no {kind} for sequence {}: {kinds:?}", sequence.0))
    };
    assert!(
        of_sequence("merge_verification_charged") < of_sequence("merge_verification_interrupted"),
        "recovery's interrupted terminal follows the record: {kinds:?}"
    );
    let replayed = resumed.run.spend();
    for (what, replayed, live) in [
        ("run", replayed.run_usd(), live.run_usd()),
        ("task", replayed.task_usd(key), live.task_usd(key)),
    ] {
        assert!(
            (replayed - live).abs() < 1e-9,
            "the next process replays the {what}'s total its predecessor charged live, both \
                 passes included: {live} -> {replayed}"
        );
    }
}

#[test]
fn a_registry_refusal_after_two_paid_passes_is_recorded_with_both_at_width_one() {
    let mut wide = stale_at_width_one("coordinator-o14-registry-refusal-w1", 3, unlimited());
    let common = wide.env.fixture.manager.common_git_dir().to_path_buf();
    let _deadline =
        crate::workspace_manager::RegistryDeadline::hold(&common, WITNESS_REGISTRY_DEADLINE);
    let (tear, admin) = tearing(&wide, "foreign-at-the-third-review", false);
    let tear: Arc<dyn ReviewInputPolicy + Send + Sync> = tear;
    let mut hooks = PlantingHooks::new(&wide.env.harness, &tear, 3);
    let seams = wide.env.seams();
    let error = wide
        .run
        .step(&seams, &mut hooks)
        .expect_err("the third reviewer's snapshot refuses");
    drop(hooks);
    refused_by_the_tear(&error);
    let sequence = SequenceId(2);
    let events = wide.env.durable_events();
    assert_eq!(
        kinds_of(&events).last(),
        Some(&"merge_verification_charged"),
        "the record is the step's last append: {:?}",
        kinds_of(&events)
    );
    assert!(
        verifying(wide.run.fold(), sequence),
        "the fold still holds the refused verification open: {:?}",
        wide.run.fold().transaction()
    );
    records_both_passes(&wide, sequence);
    let key = started_key(&events, sequence);
    let live = wide.run.spend().clone();
    untear(&admin);
    let (_, mut resumed) = wide
        .resume("inc-3", answering_two(), unlimited())
        .expect("the next process resumes once the tear is removed");
    resumed_spend_is_the_live_one(&resumed, key, &live, sequence);
    let mut hooks = resumed.env.hooks();
    let seams = resumed.env.seams();
    let progress = resumed
        .run
        .step(&seams, &mut hooks)
        .expect("the candidate verifies again");
    drop(hooks);
    assert!(
        matches!(progress, Progress::Integrated { sequence: again, .. } if again == SequenceId(3)),
        "the candidate verifies again under the next sequence: {progress:?}"
    );
    records_both_passes(&resumed, sequence);
}

fn stopped_inside_a_retrys_wait(
    wide: &mut Wide,
    at: TearAt,
    order: Script<'static>,
) -> (UpstrokeError, usize) {
    let common = wide.env.fixture.manager.common_git_dir().to_path_buf();
    let contended = crate::workspace_manager::contended_attempts(&common);
    let mut hooks = TearHeld::waking(wide, "foreign-held", at, Wakes::Never);
    hooks.tearing(Torn::CommondirEmpty);
    let injected = std::rc::Rc::new(std::cell::Cell::new(None));
    let script = a_shutdown_injected_once_the_tear_stands(
        hooks.planted(),
        std::rc::Rc::clone(&injected),
        Some(hooks.plant.clone()),
        order,
    );
    let pipelines = wide.env.pipelines();
    let runner = Arc::clone(&wide.env.runner);
    let mut scheduler = Scheduler::scripted(&runner, script);
    let slept = crate::workspace_manager::fixture::slept_pauses();
    let error = wide
        .run
        .run_concurrently(
            &wide.env.seams(),
            &pipelines,
            &mut hooks,
            Some(&mut scheduler),
        )
        .expect_err("a shutdown answered inside the retry's wait ends the command");
    drop(scheduler);
    hooks.finish().expect("the tear was planted");
    drop(hooks);
    let Some(at_shutdown) = injected.get() else {
        panic!(
            "the shutdown was injected while the tear stood: it was not, and the command \
                 ended on {error}"
        );
    };
    assert!(
        crate::workspace_manager::contended_attempts(&common) > contended,
        "the retry's access failed on the tear first, so the shutdown came inside its wait"
    );
    assert_eq!(
        crate::workspace_manager::fixture::slept_pauses(),
        slept,
        "no wait of a registry access the coordinator made slept on its thread"
    );
    (error, at_shutdown)
}

#[test]
fn the_record_past_a_stop_leaves_the_run_stopped_for_what_follows() {
    let tasks = two_independent();
    let mut wide = Wide::started_with(
        "stopped-record-then-dispatch",
        &tasks,
        2,
        WidePlans::default(),
        holding(&tasks, &[]),
    );
    let mut hooks = wide.env.hooks();
    let seams = wide.env.seams();
    let pipelines = wide.env.pipelines();
    let before = wide.run.events().len();
    let (stopped, recorded, still, dispatched) = {
        let mut coordinator = coordinator_of(
            &mut wide.run,
            &seams,
            &mut hooks,
            &pipelines,
            BTreeMap::new(),
        );
        coordinator
            .outbox
            .send(ToCoordinator::Shutdown)
            .expect("shutdown reaches inbox");
        let stopped = crate::workspace_manager::EffectHooks::registry_pause(
            &mut coordinator,
            Duration::from_millis(5),
        );
        let recorded = coordinator.run.record_charged_past_the_stop(
            super::super::run::Charged {
                sequence: SequenceId(1),
                reviews: Vec::new(),
            },
            coordinator.seams,
            coordinator.hooks,
        );
        let still = coordinator.run.stopped().map(str::to_owned);
        let dispatched = crate::engine::topology::run::begin_dispatch(
            &mut coordinator,
            seams.manager,
            TaskKey(0),
            GenerationId(0),
            false,
        );
        (stopped, recorded, still, dispatched)
    };
    let stopped = stopped.expect_err("a wait that answered a shutdown ends in an error");
    assert!(stopped.to_string().contains("met a shutdown"), "{stopped}");
    recorded.expect("a record of no pass offers nothing and fails nothing");
    assert!(
        still
            .as_deref()
            .is_some_and(|why| why.contains("met a shutdown")),
        "the stop is lifted for the record's offer alone and holds after it: {still:?}"
    );
    let refused = dispatched.expect_err("the run is still stopped for what follows the record");
    assert!(
        refused
            .to_string()
            .contains("nothing further is appended in this process"),
        "the run's emitter refused the dispatch's first append: {refused}"
    );
    assert!(
        refused.to_string().contains(
            "but the record of the review passes a verification had charged: a registry \
                 access's wait inside a transition met a shutdown"
        ),
        "the refusal names the one append made past the stop, then the stop: {refused}"
    );
    assert!(
        wide.run.events()[before..].is_empty(),
        "{:?}",
        kinds_of(&wide.run.events()[before..])
    );
    assert!(wide.run.stopped().is_some());
    wide.run.reopen();
}

fn f_entry_of(checkout: &std::path::Path) -> PathBuf {
    let pointer =
        std::fs::read_to_string(checkout.join(".git")).expect("the checkout's .git pointer");
    PathBuf::from(
        pointer
            .trim()
            .strip_prefix("gitdir:")
            .expect("a gitdir pointer")
            .trim(),
    )
}

#[cfg(unix)]
const BOUNDARY_ROOT: &str = "UPSTROKE_TEST_R5_BOUNDARY_ROOT";

#[cfg(unix)]
#[test]
#[ignore = "subprocess helper"]
fn relative_name_derive_boundary_child() {
    let Some(root) = std::env::var_os(BOUNDARY_ROOT).map(std::path::PathBuf::from) else {
        return;
    };
    let refused = crate::workspace_manager::WorkspaceManager::derive(
        &root.join("repo"),
        &root.join("private"),
        crate::workspace_manager::fixture::RUN_ID,
        "inc-2",
    )
    .expect_err("`derive` with the resume's arguments refuses the relative value");
    let message = refused.to_string();
    assert!(
        message.contains("the inherited `GIT_OBJECT_DIRECTORY` is `../objects`")
            && message.contains("export `GIT_OBJECT_DIRECTORY` as an absolute path"),
        "{message}"
    );
}
