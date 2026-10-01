//! Extended notes: `docs/internals/runner/container/fake.md`

#![forbid(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros
)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::Duration;

use serde_json::{Value, json};

use super::runtime::{
    ContainerExecution, ContainerRuntime, ContainerTrace, CreateSpec, CreatedContainer,
    DiscoveredContainer, ImageInspection, Liveness, Mount, OwnerLiveness, RuntimeError, RuntimeOp,
    Settled, StopMode,
};
use super::{ContainerHooks, DockerCli};
use crate::runner::Cancellation;
use crate::topology::effects::{EffectSiteId, HookPhase, Injection};
use crate::workspace_manager::fixture::{LINK_BOUND, LinkedChild, ParentLink};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FakeImage {
    pub digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FakeContainer {
    pub labels: BTreeMap<String, String>,
    pub requested_image_id: String,
    pub reported_image_id: String,
    pub state: Liveness,
    pub execution: ContainerExecution,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Journaled {
    pub actor: String,
    pub op: RuntimeOp,
    pub target: String,
    pub detail: String,
}

pub(crate) struct Launch<'a> {
    pub labels: &'a BTreeMap<String, String>,
}

pub(crate) enum Start {
    Hold,
    Exit(ContainerExecution),
    Run(Box<dyn FnOnce(Cancellation) -> ContainerExecution + Send>),
}

pub(crate) type StartPolicy = Arc<dyn Fn(&Launch<'_>) -> Start + Send + Sync>;

#[derive(Clone, Default)]
struct Starting(Option<StartPolicy>);

impl std::fmt::Debug for Starting {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(if self.0.is_some() {
            "Starting(policy)"
        } else {
            "Starting(hold)"
        })
    }
}

#[derive(Debug)]
struct Process {
    kill: Cancellation,
    handle: std::thread::JoinHandle<ContainerExecution>,
}

#[derive(Debug)]
struct Pace {
    target: String,
    ops: BTreeSet<RuntimeOp>,
    parties: usize,
    arrived: BTreeMap<RuntimeOp, BTreeSet<String>>,
}

pub(crate) const PACE_BOUND: Duration = Duration::from_secs(120);

#[derive(Debug, Default)]
struct State {
    images: BTreeMap<String, FakeImage>,
    tags: BTreeMap<String, String>,
    volumes: BTreeSet<String>,
    unreachable: BTreeSet<RuntimeOp>,
    failing: BTreeSet<RuntimeOp>,
    diagnostics: BTreeMap<RuntimeOp, String>,
    containers: BTreeMap<String, FakeContainer>,
    substitutions: BTreeMap<String, String>,
    journal: Vec<Journaled>,
    processes: BTreeMap<String, Process>,
    pace: Option<Pace>,
}

impl Drop for State {
    fn drop(&mut self) {
        let processes = std::mem::take(&mut self.processes);
        for process in processes.values() {
            process.kill.cancel();
        }
        for (_, process) in processes {
            let _ = process.handle.join();
        }
    }
}

#[derive(Debug, Default, Clone)]
pub(crate) struct FakeRuntime {
    state: Arc<Mutex<State>>,
    trace: ContainerTrace,
    changed: Arc<Condvar>,
    actor: String,
    starting: Starting,
}

impl FakeRuntime {
    pub(crate) fn new(trace: ContainerTrace) -> Self {
        Self {
            state: Arc::new(Mutex::new(State::default())),
            trace,
            changed: Arc::new(Condvar::new()),
            actor: String::new(),
            starting: Starting(None),
        }
    }

    pub(crate) fn acting_as(&self, actor: &str) -> Self {
        Self {
            state: Arc::clone(&self.state),
            trace: self.trace.clone(),
            changed: Arc::clone(&self.changed),
            actor: actor.to_owned(),
            starting: Starting(None),
        }
    }

    pub(crate) fn starting(mut self, policy: StartPolicy) -> Self {
        self.starting = Starting(Some(policy));
        self
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn journal(&self) -> Vec<Journaled> {
        self.state().journal.clone()
    }

    pub(crate) fn await_journal(
        &self,
        within: Duration,
        done: impl Fn(&[Journaled]) -> bool,
    ) -> bool {
        let deadline = std::time::Instant::now() + within;
        let mut state = self.state();
        loop {
            if done(&state.journal) {
                return true;
            }
            let now = std::time::Instant::now();
            if now >= deadline {
                return false;
            }
            state = self
                .changed
                .wait_timeout(state, deadline - now)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }

    pub(crate) fn pace(&self, target: &str, ops: &[RuntimeOp], parties: usize) {
        self.state().pace = Some(Pace {
            target: target.to_owned(),
            ops: ops.iter().copied().collect(),
            parties,
            arrived: BTreeMap::new(),
        });
        self.changed.notify_all();
    }

    fn paced(&self, op: RuntimeOp, target: &str) -> Result<(), RuntimeError> {
        let deadline = std::time::Instant::now() + PACE_BOUND;
        let mut state = self.state();
        let parties = match state.pace.as_mut() {
            Some(pace) if pace.target == target && pace.ops.contains(&op) => {
                pace.arrived
                    .entry(op)
                    .or_default()
                    .insert(self.actor.clone());
                pace.parties
            }
            _ => return Ok(()),
        };
        self.changed.notify_all();
        loop {
            let arrived = state
                .pace
                .as_ref()
                .and_then(|pace| pace.arrived.get(&op))
                .map_or(0, BTreeSet::len);
            if arrived >= parties {
                return Ok(());
            }
            let now = std::time::Instant::now();
            if now >= deadline {
                return Err(RuntimeError::Failed {
                    operation: op,
                    detail: format!(
                        "`{}` was paced to meet {parties} parties at `{op}` on `{target}` and \
                         {arrived} arrived within {PACE_BOUND:?}",
                        self.actor
                    ),
                });
            }
            state = self
                .changed
                .wait_timeout(state, deadline - now)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }

    fn enter(&self, op: RuntimeOp, target: &str) -> Result<(), RuntimeError> {
        self.enter_with(op, target, String::new())
    }

    fn enter_with(&self, op: RuntimeOp, target: &str, detail: String) -> Result<(), RuntimeError> {
        self.trace.runtime(op, target);
        self.state().journal.push(Journaled {
            actor: self.actor.clone(),
            op,
            target: target.to_owned(),
            detail,
        });
        self.changed.notify_all();
        self.paced(op, target)?;
        let state = self.state();
        if let Some(detail) = state.diagnostics.get(&op) {
            return Err(super::classify_docker_failure(op, detail.clone()));
        }
        if state.unreachable.contains(&op) {
            return Err(RuntimeError::Unreachable {
                operation: op,
                detail: "the fake runtime is armed unreachable for this operation".to_owned(),
            });
        }
        if state.failing.contains(&op) {
            return Err(RuntimeError::Failed {
                operation: op,
                detail: "the fake runtime is armed failing for this operation".to_owned(),
            });
        }
        Ok(())
    }

    pub(crate) fn add_image(&self, id: &str, digest: Option<&str>) {
        self.state().images.insert(
            id.to_owned(),
            FakeImage {
                digest: digest.map(str::to_owned),
            },
        );
    }

    pub(crate) fn tag(&self, reference: &str, id: &str) {
        self.state()
            .tags
            .insert(reference.to_owned(), id.to_owned());
    }

    pub(crate) fn move_tag(&self, reference: &str, new_id: &str) {
        let mut state = self.state();
        assert!(
            state.tags.contains_key(reference),
            "`{reference}` is not tagged, so it cannot be moved; \
             a moved-reference fixture that started from nothing would be testing itself"
        );
        state.tags.insert(reference.to_owned(), new_id.to_owned());
    }

    pub(crate) fn substitute_reported_image_id(&self, name: &str, reported: &str) {
        self.state()
            .substitutions
            .insert(name.to_owned(), reported.to_owned());
    }

    pub(crate) fn add_volume(&self, name: &str) {
        self.state().volumes.insert(name.to_owned());
    }

    pub(crate) fn remove_volume(&self, name: &str) {
        self.state().volumes.remove(name);
    }

    pub(crate) fn set_unreachable(&self, op: RuntimeOp) {
        self.state().unreachable.insert(op);
    }

    pub(crate) fn set_all_unreachable(&self) {
        let mut state = self.state();
        for op in RuntimeOp::ALL {
            state.unreachable.insert(*op);
        }
    }

    pub(crate) fn set_reachable(&self, op: RuntimeOp) {
        self.state().unreachable.remove(&op);
    }

    pub(crate) fn set_docker_stderr(&self, op: RuntimeOp, detail: &str) {
        self.state().diagnostics.insert(op, detail.to_owned());
    }

    pub(crate) fn set_failing(&self, op: RuntimeOp) {
        self.state().failing.insert(op);
    }

    pub(crate) fn seed_container(
        &self,
        name: &str,
        labels: BTreeMap<String, String>,
        requested_image_id: &str,
        reported_image_id: &str,
        state: Liveness,
    ) {
        self.state().containers.insert(
            name.to_owned(),
            FakeContainer {
                labels,
                requested_image_id: requested_image_id.to_owned(),
                reported_image_id: reported_image_id.to_owned(),
                state,
                execution: ContainerExecution {
                    exit_code: Some(0),
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                },
            },
        );
    }

    pub(crate) fn set_container_state(&self, name: &str, state: Liveness) {
        if let Some(container) = self.state().containers.get_mut(name) {
            container.state = state;
        }
    }

    pub(crate) fn set_execution(&self, name: &str, execution: ContainerExecution) {
        if let Some(container) = self.state().containers.get_mut(name) {
            container.execution = execution;
        }
    }

    pub(crate) fn container(&self, name: &str) -> Option<FakeContainer> {
        self.state().containers.get(name).cloned()
    }

    pub(crate) fn container_names(&self) -> Vec<String> {
        self.state().containers.keys().cloned().collect()
    }

    pub(crate) fn calls(&self) -> Vec<RuntimeOp> {
        self.trace.ops()
    }
}

impl ContainerRuntime for FakeRuntime {
    fn probe(&self) -> Result<(), RuntimeError> {
        self.enter(RuntimeOp::Probe, "daemon")
    }

    fn image_by_reference(&self, reference: &str) -> Result<Option<ImageInspection>, RuntimeError> {
        self.enter(RuntimeOp::InspectImageByReference, reference)?;
        let state = self.state();
        let Some(id) = state.tags.get(reference) else {
            return Ok(None);
        };
        Ok(state.images.get(id).map(|image| ImageInspection {
            id: id.clone(),
            digest: image.digest.clone(),
            references: state
                .tags
                .iter()
                .filter(|(_, target)| *target == id)
                .map(|(name, _)| name.clone())
                .collect(),
        }))
    }

    fn image_by_id(&self, id: &str) -> Result<Option<ImageInspection>, RuntimeError> {
        self.enter(RuntimeOp::InspectImageById, id)?;
        let state = self.state();
        Ok(state.images.get(id).map(|image| ImageInspection {
            id: id.to_owned(),
            digest: image.digest.clone(),
            references: state
                .tags
                .iter()
                .filter(|(_, target)| target.as_str() == id)
                .map(|(name, _)| name.clone())
                .collect(),
        }))
    }

    fn volume_present(&self, name: &str) -> Result<bool, RuntimeError> {
        self.enter(RuntimeOp::InspectVolume, name)?;
        Ok(self.state().volumes.contains(name))
    }

    fn containers_with_label(
        &self,
        key: &str,
        value: &str,
    ) -> Result<Vec<DiscoveredContainer>, RuntimeError> {
        self.enter(RuntimeOp::ListByLabel, value)?;
        Ok(self
            .state()
            .containers
            .iter()
            .filter(|(_, container)| container.labels.get(key).map(String::as_str) == Some(value))
            .map(|(name, container)| DiscoveredContainer {
                name: name.clone(),
                labels: container.labels.clone(),
            })
            .collect())
    }

    fn observe(&self, name: &str) -> Result<Liveness, RuntimeError> {
        self.enter(RuntimeOp::Observe, name)?;
        self.settle_process(name);
        Ok(self
            .state()
            .containers
            .get(name)
            .map_or(Liveness::Gone, |container| container.state))
    }

    fn collect(&self, name: &str) -> Result<ContainerExecution, RuntimeError> {
        self.enter(RuntimeOp::Collect, name)?;
        self.state()
            .containers
            .get(name)
            .map(|container| container.execution.clone())
            .ok_or_else(|| RuntimeError::Failed {
                operation: RuntimeOp::Collect,
                detail: format!("`{name}` is gone"),
            })
    }

    fn create(&self, spec: &CreateSpec) -> Result<CreatedContainer, RuntimeError> {
        self.daemon_create(spec)
    }

    fn start(&self, name: &str) -> Result<(), RuntimeError> {
        self.daemon_start(name)
    }

    fn stop(&self, name: &str, mode: StopMode) -> Result<Settled, RuntimeError> {
        self.daemon_stop(name, mode)
    }

    fn remove(&self, name: &str) -> Result<Settled, RuntimeError> {
        self.daemon_remove(name)
    }
}

impl FakeRuntime {
    fn daemon_create(&self, spec: &CreateSpec) -> Result<CreatedContainer, RuntimeError> {
        let volumes: Vec<&str> = spec
            .mounts
            .iter()
            .filter_map(|mount| match mount {
                Mount::Volume { name, .. } => Some(name.as_str()),
                Mount::Path { .. } | Mount::Tmpfs { .. } => None,
            })
            .collect();
        self.enter_with(RuntimeOp::Create, &spec.name, volumes.join(","))?;
        let mut state = self.state();
        if !state.images.contains_key(&spec.image_id) {
            return Err(RuntimeError::Failed {
                operation: RuntimeOp::Create,
                detail: format!("no image with id `{}`", spec.image_id),
            });
        }
        if state.containers.contains_key(&spec.name) {
            return Err(RuntimeError::Failed {
                operation: RuntimeOp::Create,
                detail: format!("a container named `{}` already exists", spec.name),
            });
        }
        for mount in &spec.mounts {
            if let super::runtime::Mount::Volume { name, .. } = mount {
                if !state.volumes.contains(name) {
                    return Err(RuntimeError::Failed {
                        operation: RuntimeOp::Create,
                        detail: format!("no volume named `{name}`"),
                    });
                }
            }
        }
        let reported = state
            .substitutions
            .get(&spec.name)
            .cloned()
            .unwrap_or_else(|| spec.image_id.clone());
        state.containers.insert(
            spec.name.clone(),
            FakeContainer {
                labels: spec.labels.clone(),
                requested_image_id: spec.image_id.clone(),
                reported_image_id: reported.clone(),
                state: Liveness::Exited,
                execution: ContainerExecution {
                    exit_code: Some(0),
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                },
            },
        );
        Ok(CreatedContainer {
            name: spec.name.clone(),
            reported_image_id: reported,
        })
    }

    fn daemon_start(&self, name: &str) -> Result<(), RuntimeError> {
        self.enter(RuntimeOp::Start, name)?;
        let labels = {
            let mut state = self.state();
            let Some(container) = state.containers.get_mut(name) else {
                return Err(RuntimeError::Failed {
                    operation: RuntimeOp::Start,
                    detail: format!("no such container `{name}`"),
                });
            };
            container.state = Liveness::Running;
            container.labels.clone()
        };
        let Some(policy) = self.starting.0.clone() else {
            return Ok(());
        };
        match policy(&Launch { labels: &labels }) {
            Start::Hold => {}
            Start::Exit(execution) => {
                if let Some(container) = self.state().containers.get_mut(name) {
                    container.execution = execution;
                    container.state = Liveness::Exited;
                }
            }
            Start::Run(body) => {
                let kill = Cancellation::new();
                let token = kill.clone();
                let thread = std::thread::current()
                    .name()
                    .unwrap_or("fake-container")
                    .to_owned();
                let handle = std::thread::Builder::new()
                    .name(thread)
                    .spawn(move || body(token))
                    .expect("start the fake container's process");
                self.state()
                    .processes
                    .insert(name.to_owned(), Process { kill, handle });
            }
        }
        Ok(())
    }

    fn daemon_stop(&self, name: &str, _mode: StopMode) -> Result<Settled, RuntimeError> {
        let entered = self
            .enter(RuntimeOp::Stop, name)
            .map(|()| format!("{name}\n"));
        if entered.is_ok() {
            self.kill_process(name);
        }
        let settled = super::settle_stop(name, entered, |target| self.observe(target))?;
        if settled.process_gone() {
            if let Some(container) = self.state().containers.get_mut(name) {
                container.state = Liveness::Exited;
            }
        }
        Ok(settled)
    }

    fn daemon_remove(&self, name: &str) -> Result<Settled, RuntimeError> {
        let entered = self
            .enter(RuntimeOp::Remove, name)
            .map(|()| format!("{name}\n"));
        if entered.is_ok() {
            self.kill_process(name);
        }
        let settled = super::settle_remove(name, entered, |target| self.observe(target))?;
        if settled.process_gone() {
            self.state().containers.remove(name);
        }
        Ok(settled)
    }

    fn settle_process(&self, name: &str) {
        let finished = {
            let mut state = self.state();
            match state.processes.get(name) {
                Some(process) if process.handle.is_finished() => state.processes.remove(name),
                _ => None,
            }
        };
        if let Some(process) = finished {
            self.exited(name, process);
        }
    }

    fn kill_process(&self, name: &str) {
        let running = self.state().processes.remove(name);
        if let Some(process) = running {
            process.kill.cancel();
            self.exited(name, process);
        }
    }

    fn exited(&self, name: &str, process: Process) {
        let execution = process
            .handle
            .join()
            .unwrap_or_else(|_| ContainerExecution {
                exit_code: None,
                stdout: Vec::new(),
                stderr: b"the fake container's process panicked".to_vec(),
            });
        if let Some(container) = self.state().containers.get_mut(name) {
            container.execution = execution;
            container.state = Liveness::Exited;
        }
        self.changed.notify_all();
    }
}

#[cfg(unix)]
const REAPER_STUB: &str = r#"#!/bin/sh
relay=${0%/*}
printf '%s\t' "$@" >> "$relay/calls"
printf '\n' >> "$relay/calls"
case "$1" in
ps) cat "$relay/listing" ;;
rm) grep -v -x -F -e "$4" "$relay/listing" > "$relay/listing.next"
    mv "$relay/listing.next" "$relay/listing" ;;
esac
exit 0
"#;

#[cfg(unix)]
impl FakeRuntime {
    pub(crate) fn install_reaper_relay(relay: &Path) -> PathBuf {
        let program = relay.join("docker");
        crate::workspace_manager::fixture::write_executable(&program, REAPER_STUB.as_bytes());
        crate::workspace_manager::fixture::write_file(&relay.join("listing"), b"");
        crate::workspace_manager::fixture::write_file(&relay.join("calls"), b"");
        program
    }

    pub(crate) fn list_for_reaper(
        &self,
        relay: &Path,
        private_root: &Path,
        incarnation: &str,
    ) -> Vec<String> {
        let root = super::intent::private_root_label(private_root);
        let names: Vec<String> = self
            .state()
            .containers
            .iter()
            .filter(|(_, container)| {
                container.labels.get(super::intent::LABEL_PRIVATE_ROOT) == Some(&root)
                    && container
                        .labels
                        .get(super::intent::LABEL_INCARNATION)
                        .map(String::as_str)
                        == Some(incarnation)
            })
            .map(|(name, _)| name.clone())
            .collect();
        let listing: String = names.iter().map(|name| format!("{name}\n")).collect();
        crate::workspace_manager::fixture::write_file(&relay.join("listing"), listing.as_bytes());
        names
    }

    pub(crate) fn reaper_calls(relay: &Path) -> Vec<Vec<String>> {
        std::fs::read_to_string(relay.join("calls"))
            .unwrap_or_default()
            .lines()
            .map(|line| {
                line.split('\t')
                    .filter(|field| !field.is_empty())
                    .map(str::to_owned)
                    .collect()
            })
            .collect()
    }

    pub(crate) fn deliver_reaper_calls(&self, relay: &Path) -> Vec<String> {
        let daemon = self.acting_as("reaper");
        let mut delivered = Vec::new();
        for call in Self::reaper_calls(relay) {
            let (verb, settled) = match (call.first().map(String::as_str), call.last()) {
                (Some("kill"), Some(id)) => ("kill", daemon.daemon_stop(id, StopMode::Kill)),
                (Some("rm"), Some(id)) => ("rm", daemon.daemon_remove(id)),
                _ => continue,
            };
            delivered.push(match settled {
                Ok(settled) => format!(
                    "{verb} {}: {}",
                    call.last().map_or("", String::as_str),
                    settled_name(settled)
                ),
                Err(error) => format!("{verb} {}: {error}", call.last().map_or("", String::as_str)),
            });
        }
        delivered
    }
}

pub(crate) const RUNTIME_REQUEST: &str = "UPSTROKE-RUNTIME-REQUEST ";

pub(crate) const RUNTIME_REPLY: &str = "UPSTROKE-RUNTIME-REPLY ";

pub(crate) const CHILD_EVENT: &str = "UPSTROKE-CHILD-EVENT ";

impl FakeRuntime {
    pub(crate) fn serve(
        &self,
        lines: &Receiver<String>,
        link: &LinkedChild,
        events: &Sender<Value>,
    ) -> Vec<String> {
        let mut other = Vec::new();
        while let Ok(line) = lines.recv() {
            if let Some((_, request)) = line.split_once(RUNTIME_REQUEST) {
                let answer = match serde_json::from_str::<Value>(request) {
                    Ok(request) => self.answer(&request),
                    Err(error) => json!({"err": {
                        "kind": "failed",
                        "operation": RuntimeOp::Probe.name(),
                        "detail": format!("the request is not JSON: {error}"),
                    }}),
                };
                let _ = link.send(&format!("{RUNTIME_REPLY}{answer}"));
            } else if let Some((_, event)) = line.split_once(CHILD_EVENT) {
                match serde_json::from_str::<Value>(event) {
                    Ok(event) => {
                        let _ = events.send(event);
                    }
                    Err(_) => other.push(line),
                }
            } else {
                other.push(line);
            }
        }
        other
    }

    fn answer(&self, request: &Value) -> Value {
        let Some(op) = request.get("op").and_then(Value::as_str).and_then(op_named) else {
            return json!({"err": {
                "kind": "failed",
                "operation": RuntimeOp::Probe.name(),
                "detail": format!("the request names no runtime operation: {request}"),
            }});
        };
        let text = |key: &str| {
            request
                .get(key)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned()
        };
        let answered: Result<Value, RuntimeError> = match op {
            RuntimeOp::Probe => self.probe().map(|()| Value::Null),
            RuntimeOp::InspectImageByReference => self
                .image_by_reference(&text("reference"))
                .map(|found| found.as_ref().map_or(Value::Null, inspection_json)),
            RuntimeOp::InspectImageById => self
                .image_by_id(&text("id"))
                .map(|found| found.as_ref().map_or(Value::Null, inspection_json)),
            RuntimeOp::InspectVolume => self.volume_present(&text("name")).map(Value::Bool),
            RuntimeOp::ListByLabel => self
                .containers_with_label(&text("key"), &text("value"))
                .map(|found| Value::Array(found.iter().map(discovered_json).collect())),
            RuntimeOp::Observe => self
                .observe(&text("name"))
                .map(|state| Value::from(liveness_name(state))),
            RuntimeOp::Collect => self
                .collect(&text("name"))
                .map(|execution| execution_json(&execution)),
            RuntimeOp::Create => match request.get("spec").and_then(spec_of) {
                Some(spec) => self
                    .daemon_create(&spec)
                    .map(|created| json!({"name": created.name, "reported_image_id": created.reported_image_id})),
                None => Err(RuntimeError::Failed {
                    operation: op,
                    detail: "the create request carried no readable spec".to_owned(),
                }),
            },
            RuntimeOp::Start => self.daemon_start(&text("name")).map(|()| Value::Null),
            RuntimeOp::Stop => {
                let mode = if text("mode") == StopMode::Graceful.name() {
                    StopMode::Graceful
                } else {
                    StopMode::Kill
                };
                self.daemon_stop(&text("name"), mode)
                    .map(|settled| Value::from(settled_name(settled)))
            }
            RuntimeOp::Remove => self
                .daemon_remove(&text("name"))
                .map(|settled| Value::from(settled_name(settled))),
        };
        match answered {
            Ok(value) => json!({"ok": value}),
            Err(error) => json!({"err": error_json(&error)}),
        }
    }
}

pub(crate) struct LinkedRuntime {
    link: Arc<ParentLink>,
    turn: Mutex<()>,
}

impl LinkedRuntime {
    pub(crate) fn over(link: Arc<ParentLink>) -> Self {
        Self {
            link,
            turn: Mutex::new(()),
        }
    }

    fn call(&self, op: RuntimeOp, arguments: Value) -> Result<Value, RuntimeError> {
        let mut request = json!({"op": op.name()});
        if let (Some(request), Value::Object(arguments)) = (request.as_object_mut(), arguments) {
            request.extend(arguments);
        }
        let _turn = self.turn.lock().unwrap_or_else(PoisonError::into_inner);
        self.link.send(&format!("{RUNTIME_REQUEST}{request}"));
        let Some(line) = self.link.recv_within(LINK_BOUND) else {
            return Err(RuntimeError::Unreachable {
                operation: op,
                detail: format!("the parent's runtime did not answer within {LINK_BOUND:?}"),
            });
        };
        let reply = line
            .split_once(RUNTIME_REPLY)
            .map(|(_, reply)| reply)
            .and_then(|reply| serde_json::from_str::<Value>(reply).ok())
            .ok_or_else(|| RuntimeError::Failed {
                operation: op,
                detail: format!("the parent answered with something other than a reply: {line}"),
            })?;
        if let Some(error) = reply.get("err") {
            return Err(error_of(error, op));
        }
        Ok(reply.get("ok").cloned().unwrap_or(Value::Null))
    }
}

impl ContainerRuntime for LinkedRuntime {
    fn probe(&self) -> Result<(), RuntimeError> {
        self.call(RuntimeOp::Probe, json!({})).map(|_| ())
    }

    fn image_by_reference(&self, reference: &str) -> Result<Option<ImageInspection>, RuntimeError> {
        self.call(
            RuntimeOp::InspectImageByReference,
            json!({"reference": reference}),
        )
        .map(|found| inspection_of(&found))
    }

    fn image_by_id(&self, id: &str) -> Result<Option<ImageInspection>, RuntimeError> {
        self.call(RuntimeOp::InspectImageById, json!({"id": id}))
            .map(|found| inspection_of(&found))
    }

    fn volume_present(&self, name: &str) -> Result<bool, RuntimeError> {
        self.call(RuntimeOp::InspectVolume, json!({"name": name}))
            .map(|present| present.as_bool().unwrap_or(false))
    }

    fn containers_with_label(
        &self,
        key: &str,
        value: &str,
    ) -> Result<Vec<DiscoveredContainer>, RuntimeError> {
        self.call(RuntimeOp::ListByLabel, json!({"key": key, "value": value}))
            .map(|found| {
                found
                    .as_array()
                    .map(|found| found.iter().filter_map(discovered_of).collect())
                    .unwrap_or_default()
            })
    }

    fn observe(&self, name: &str) -> Result<Liveness, RuntimeError> {
        let state = self.call(RuntimeOp::Observe, json!({"name": name}))?;
        state
            .as_str()
            .and_then(liveness_of)
            .ok_or_else(|| RuntimeError::Failed {
                operation: RuntimeOp::Observe,
                detail: format!("the parent answered `{state}` for a liveness"),
            })
    }

    fn collect(&self, name: &str) -> Result<ContainerExecution, RuntimeError> {
        let execution = self.call(RuntimeOp::Collect, json!({"name": name}))?;
        Ok(execution_of(&execution))
    }

    fn create(&self, spec: &CreateSpec) -> Result<CreatedContainer, RuntimeError> {
        let created = self.call(RuntimeOp::Create, json!({"spec": spec_json(spec)}))?;
        Ok(CreatedContainer {
            name: created
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            reported_image_id: created
                .get("reported_image_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
        })
    }

    fn start(&self, name: &str) -> Result<(), RuntimeError> {
        self.call(RuntimeOp::Start, json!({"name": name}))
            .map(|_| ())
    }

    fn stop(&self, name: &str, mode: StopMode) -> Result<Settled, RuntimeError> {
        let settled = self.call(RuntimeOp::Stop, json!({"name": name, "mode": mode.name()}))?;
        settled_of(&settled, RuntimeOp::Stop)
    }

    fn remove(&self, name: &str) -> Result<Settled, RuntimeError> {
        let settled = self.call(RuntimeOp::Remove, json!({"name": name}))?;
        settled_of(&settled, RuntimeOp::Remove)
    }
}

fn op_named(name: &str) -> Option<RuntimeOp> {
    RuntimeOp::ALL.iter().copied().find(|op| op.name() == name)
}

fn error_json(error: &RuntimeError) -> Value {
    let (kind, operation, detail) = match error {
        RuntimeError::Unreachable { operation, detail } => ("unreachable", operation, detail),
        RuntimeError::Failed { operation, detail } => ("failed", operation, detail),
    };
    json!({"kind": kind, "operation": operation.name(), "detail": detail})
}

fn error_of(value: &Value, asked: RuntimeOp) -> RuntimeError {
    let operation = value
        .get("operation")
        .and_then(Value::as_str)
        .and_then(op_named)
        .unwrap_or(asked);
    let detail = value
        .get("detail")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    if value.get("kind").and_then(Value::as_str) == Some("unreachable") {
        RuntimeError::Unreachable { operation, detail }
    } else {
        RuntimeError::Failed { operation, detail }
    }
}

const fn liveness_name(state: Liveness) -> &'static str {
    match state {
        Liveness::Running => "running",
        Liveness::Exited => "exited",
        Liveness::Gone => "gone",
    }
}

fn liveness_of(name: &str) -> Option<Liveness> {
    [Liveness::Running, Liveness::Exited, Liveness::Gone]
        .into_iter()
        .find(|state| liveness_name(*state) == name)
}

const fn settled_name(settled: Settled) -> &'static str {
    match settled {
        Settled::ProcessGone => "process-gone",
        Settled::RemovalInProgress => "removal-in-progress",
    }
}

fn settled_of(value: &Value, op: RuntimeOp) -> Result<Settled, RuntimeError> {
    [Settled::ProcessGone, Settled::RemovalInProgress]
        .into_iter()
        .find(|settled| value.as_str() == Some(settled_name(*settled)))
        .ok_or_else(|| RuntimeError::Failed {
            operation: op,
            detail: format!("the parent answered `{value}` for a settlement"),
        })
}

fn inspection_json(found: &ImageInspection) -> Value {
    json!({"id": found.id, "digest": found.digest, "references": found.references})
}

fn inspection_of(value: &Value) -> Option<ImageInspection> {
    Some(ImageInspection {
        id: value.get("id")?.as_str()?.to_owned(),
        digest: value
            .get("digest")
            .and_then(Value::as_str)
            .map(str::to_owned),
        references: serde_json::from_value(value.get("references")?.clone()).ok()?,
    })
}

fn discovered_json(found: &DiscoveredContainer) -> Value {
    json!({"name": found.name, "labels": found.labels})
}

fn discovered_of(value: &Value) -> Option<DiscoveredContainer> {
    Some(DiscoveredContainer {
        name: value.get("name")?.as_str()?.to_owned(),
        labels: serde_json::from_value(value.get("labels")?.clone()).ok()?,
    })
}

fn execution_json(execution: &ContainerExecution) -> Value {
    json!({
        "exit_code": execution.exit_code,
        "stdout": execution.stdout,
        "stderr": execution.stderr,
    })
}

fn execution_of(value: &Value) -> ContainerExecution {
    let bytes = |key: &str| {
        value
            .get(key)
            .and_then(|bytes| serde_json::from_value::<Vec<u8>>(bytes.clone()).ok())
            .unwrap_or_default()
    };
    ContainerExecution {
        exit_code: value
            .get("exit_code")
            .and_then(Value::as_i64)
            .and_then(|code| i32::try_from(code).ok()),
        stdout: bytes("stdout"),
        stderr: bytes("stderr"),
    }
}

fn spec_json(spec: &CreateSpec) -> Value {
    let mounts: Vec<Value> = spec
        .mounts
        .iter()
        .map(|mount| match mount {
            Mount::Path {
                source,
                target,
                read_only,
            } => {
                json!({"path": source.to_string_lossy(), "target": target, "read_only": read_only})
            }
            Mount::Volume {
                name,
                target,
                read_only,
            } => json!({"volume": name, "target": target, "read_only": read_only}),
            Mount::Tmpfs { target } => json!({"tmpfs": target}),
        })
        .collect();
    json!({
        "name": spec.name,
        "image_id": spec.image_id,
        "labels": spec.labels,
        "mounts": mounts,
        "env": spec.env,
        "command": spec.command,
        "workdir": spec.workdir,
        "read_only_root": spec.read_only_root,
    })
}

fn spec_of(value: &Value) -> Option<CreateSpec> {
    let mounts = value
        .get("mounts")?
        .as_array()?
        .iter()
        .map(|mount| {
            let target = mount.get("target").and_then(Value::as_str);
            let read_only = mount
                .get("read_only")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if let Some(source) = mount.get("path").and_then(Value::as_str) {
                Some(Mount::Path {
                    source: PathBuf::from(source),
                    target: target?.to_owned(),
                    read_only,
                })
            } else if let Some(name) = mount.get("volume").and_then(Value::as_str) {
                Some(Mount::Volume {
                    name: name.to_owned(),
                    target: target?.to_owned(),
                    read_only,
                })
            } else {
                mount
                    .get("tmpfs")
                    .and_then(Value::as_str)
                    .map(|target| Mount::Tmpfs {
                        target: target.to_owned(),
                    })
            }
        })
        .collect::<Option<Vec<Mount>>>()?;
    Some(CreateSpec {
        name: value.get("name")?.as_str()?.to_owned(),
        image_id: value.get("image_id")?.as_str()?.to_owned(),
        labels: serde_json::from_value(value.get("labels")?.clone()).ok()?,
        mounts,
        env: serde_json::from_value(value.get("env")?.clone()).ok()?,
        command: serde_json::from_value(value.get("command")?.clone()).ok()?,
        workdir: value
            .get("workdir")
            .and_then(Value::as_str)
            .map(str::to_owned),
        read_only_root: value
            .get("read_only_root")
            .and_then(Value::as_bool)
            .unwrap_or(true),
    })
}

pub(crate) fn container_name_for(
    repo_key: &str,
    run_id: &str,
    incarnation: &str,
    invocation: &crate::runner::InvocationId,
) -> String {
    super::intent::ContainerName::new(repo_key, run_id, incarnation, invocation)
        .expect("a container name")
        .as_str()
        .to_owned()
}

pub(crate) fn container_name_parts(name: &str) -> Option<(String, String)> {
    super::intent::ContainerName::parse(name)
        .ok()
        .map(|parts| (parts.incarnation, parts.invocation_hash))
}

pub(crate) fn intent_path_for(private_root: &Path, name: &str) -> Option<PathBuf> {
    super::intent::ContainerName::rebuild(name)
        .ok()
        .map(|name| name.intent_path(private_root))
}

#[derive(Debug, Default)]
pub(crate) struct FakeOwnerLiveness {
    live: Mutex<BTreeSet<PathBuf>>,
}

impl FakeOwnerLiveness {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn set_live(&self, public_run_dir: &Path) {
        self.live
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(public_run_dir.to_path_buf());
    }

    pub(crate) fn set_dead(&self, public_run_dir: &Path) {
        self.live
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(public_run_dir);
    }
}

impl OwnerLiveness for FakeOwnerLiveness {
    fn is_running(&self, public_run_dir: &Path) -> bool {
        self.live
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .contains(public_run_dir)
    }
}

#[derive(Debug, Default)]
pub(crate) struct RecordingHooks {
    trace: ContainerTrace,
    armed: Option<(EffectSiteId, HookPhase)>,
}

impl RecordingHooks {
    pub(crate) fn new(trace: ContainerTrace) -> Self {
        Self { trace, armed: None }
    }

    pub(crate) fn fail_at(&mut self, site: EffectSiteId, phase: HookPhase) {
        self.armed = Some((site, phase));
    }
}

impl ContainerHooks for RecordingHooks {
    fn phase(&mut self, site: EffectSiteId, phase: HookPhase) -> Injection {
        if self.armed == Some((site, phase)) {
            return Injection::Error;
        }
        Injection::Proceed
    }

    fn trace(&self) -> ContainerTrace {
        self.trace.clone()
    }
}

pub(crate) const REQUIRE_DOCKER: &str = "UPSTROKE_REQUIRE_DOCKER";

pub(crate) const DOCKER_GATED_TESTS: &[&str] = &[
    "real_docker_reports_an_image_id_and_a_digest_for_a_reference_it_holds",
    "real_docker_refuses_a_reference_it_does_not_hold_without_pulling",
    "real_docker_creates_from_an_id_reports_it_and_reclaims_idempotently",
    "real_docker_kill_on_an_already_exited_container_is_tolerated",
    "real_docker_returns_both_streams_of_a_container_separately",
    "real_docker_removing_a_container_reclaims_its_anonymous_volumes",
    "real_docker_runs_from_the_recorded_image_id_and_composes_over_the_image_environment",
    "real_docker_refuses_a_reviewer_write_to_its_read_only_mount",
    "real_docker_confines_a_gate_to_its_mount",
    "real_docker_a_git_dependent_gate_sees_only_the_role_view",
    "real_docker_adapter_parsing_matches_the_host_table",
    "real_docker_census_reclaims_a_dead_owner_and_spares_a_live_one",
    "real_docker_a_gate_write_outside_every_declared_mount_fails",
    "real_docker_the_daemon_holds_exactly_the_specs_mounts_and_a_read_only_root",
    "real_docker_a_worktree_binary_cannot_shadow_the_certified_cli",
    "real_docker_renders_a_comma_bearing_label_value_whole",
    "real_docker_prints_the_transcribed_unreachable_diagnostics",
    "real_docker_creates_an_absent_named_volume_rather_than_refusing",
    "real_docker_prints_the_transcribed_removal_in_progress_diagnostic",
    "real_docker_withholds_an_image_credential_variable_from_a_role_that_takes_none",
    "real_docker_a_container_contains_a_daemonised_descendant",
    "real_docker_fails_locally_without_ever_saying_a_container_is_gone",
    "real_docker_lists_the_state_the_settlement_observation_reads",
];

pub(crate) fn absent_reason() -> String {
    format!(
        "no container runtime: `{}` is not on PATH or its daemon does not answer",
        super::DOCKER_PROGRAM
    )
}

pub(crate) fn docker_gate(test: &str, trace: ContainerTrace) -> Result<Box<DockerCli>, String> {
    assert!(
        DOCKER_GATED_TESTS.contains(&test),
        "`{test}` is Docker-gated and is not in DOCKER_GATED_TESTS, so nothing counts it"
    );
    if DockerCli::available() {
        return Ok(Box::new(DockerCli::new(trace)));
    }
    let reason = absent_reason();
    assert!(
        std::env::var_os(REQUIRE_DOCKER).is_none(),
        "{REQUIRE_DOCKER} is set and `{test}` would have skipped: {reason}"
    );
    Err(reason)
}

pub(crate) fn slot_repo_key() -> &'static str {
    static KEY: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
        scoped_repo_key(&std::env::var("CARGO_TARGET_DIR").unwrap_or_default())
    });
    &KEY
}

pub(crate) fn scoped_repo_key(scope: &str) -> String {
    if scope.is_empty() {
        return "0123456789abcdef".to_owned();
    }
    let digest = format!(
        "{:x}",
        <sha2::Sha256 as sha2::Digest>::digest(scope.as_bytes())
    );
    digest[..16].to_owned()
}

pub(crate) fn unscoped_names(names: &[&super::intent::ContainerName]) -> Vec<String> {
    let slot = slot_repo_key();
    names
        .iter()
        .filter(|name| {
            !super::intent::ContainerName::parse(name.as_str())
                .is_ok_and(|parts| parts.repo_key == slot)
        })
        .map(|name| name.as_str().to_owned())
        .collect()
}

pub(crate) fn preclean_names(
    runtime: &dyn ContainerRuntime,
    view: &dyn super::GitView,
    private_root: &Path,
    names: &[&super::intent::ContainerName],
) {
    let unscoped = unscoped_names(names);
    assert!(
        unscoped.is_empty(),
        "pre-cleaning {unscoped:?}: the repo-key component is not this build slot's `{}`, so a \
         concurrent suite in another slot asks for the same name and this kill lands on its live \
         container rather than on a previous run's residue. `PR7-R3-CONTRACT-001`",
        slot_repo_key()
    );
    for name in names {
        super::reclaim(&mut super::NoHooks, runtime, view, private_root, name, None)
            .unwrap_or_else(|error| {
                panic!("pre-clean could not reclaim a possibly-stranded `{name}`: {error}")
            });
    }
}
