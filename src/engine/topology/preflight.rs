//! Extended notes: `docs/internals/engine/topology/preflight.md`

use std::borrow::BorrowMut;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, PoisonError};

use crate::agent::{AdapterSource, Caps, ProcessOutput};
use crate::error::UpstrokeError;
use crate::gates::ShellKind;
use crate::runner::container::resolve::RunnerPreflight;
use crate::runner::{RunFuture, Runner, RunnerCall, RunnerError, RunnerRequest};
use crate::topology::events::RunnerPolicy;

use super::identity::{InvocationLedger, PreflightIdentities, SlotPair, is_slotted};
use super::select::Standing;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Probed {
    pub agents: Vec<String>,
    pub caps: Vec<(String, Caps)>,
}

impl Probed {
    #[must_use]
    pub fn agents(&self) -> &[String] {
        &self.agents
    }
}

pub struct RunPreflight<'a> {
    runner: &'a dyn Runner,
    adapters: &'a dyn AdapterSource,
    shell: ShellKind,
    workspace: PathBuf,
    agents: Vec<String>,
    ledger: Mutex<InvocationLedger>,
    probed: Mutex<Option<Probed>>,
}

impl std::fmt::Debug for RunPreflight<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RunPreflight")
            .field("shell", &self.shell)
            .field("workspace", &self.workspace)
            .field("agents", &self.agents)
            .finish_non_exhaustive()
    }
}

impl<'a> RunPreflight<'a> {
    #[must_use]
    pub fn new(
        runner: &'a dyn Runner,
        adapters: &'a dyn AdapterSource,
        shell: ShellKind,
        workspace: &Path,
        agents: Vec<String>,
    ) -> Self {
        Self {
            runner,
            adapters,
            shell,
            workspace: workspace.to_path_buf(),
            agents,
            ledger: Mutex::new(InvocationLedger::new()),
            probed: Mutex::new(None),
        }
    }

    #[must_use]
    pub fn probed(&self) -> Option<Probed> {
        self.probed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    #[must_use]
    pub fn ledgers_balance(&self) -> bool {
        self.ledger
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .balances()
    }

    #[must_use]
    pub fn settlements(&self) -> (usize, usize) {
        let ledger = self.ledger.lock().unwrap_or_else(PoisonError::into_inner);
        (ledger.completed(), ledger.cancelled())
    }

    #[must_use]
    pub fn running(&self) -> Vec<String> {
        self.ledger
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .running()
            .into_iter()
            .map(str::to_owned)
            .collect()
    }

    fn registering(&self) -> Registering<'_, InvocationLedger> {
        Registering::new(self.runner, &self.ledger, Slots::Probe)
    }
}

impl RunnerPreflight for RunPreflight<'_> {
    fn certify(&self, policy: &RunnerPolicy) -> Result<(), UpstrokeError> {
        let registering = self.registering();
        let shell_id = PreflightIdentities::shell(0)?;
        crate::runner::host::run_shell_probe(
            &registering,
            self.shell,
            self.workspace.clone(),
            shell_id,
        )
        .map_err(|error| refused(policy, &format!("the recorded shell: {error}")))?;

        let mut caps = Vec::with_capacity(self.agents.len());
        for agent in &self.agents {
            let adapter = self.adapters.get(agent).ok_or_else(|| {
                refused(policy, &format!("no adapter is registered for `{agent}`"))
            })?;
            let probed = adapter
                .probe(&registering)
                .map_err(|error| refused(policy, &format!("the `{agent}` CLI: {error}")))?;
            caps.push((agent.clone(), probed));
        }

        let probed = Probed {
            agents: self.agents.clone(),
            caps,
        };
        *self.probed.lock().unwrap_or_else(PoisonError::into_inner) = Some(probed);
        Ok(())
    }
}

fn refused(policy: &RunnerPolicy, what: &str) -> UpstrokeError {
    let boundary = match policy.image.as_ref() {
        Some(image) => format!(
            "the recorded image `{}` (id `{}`)",
            image.reference, image.id
        ),
        None => "this host".to_owned(),
    };
    UpstrokeError::Refused {
        message: format!(
            "pre-flight: {what}. It was probed inside {boundary}, which is the only observation \
             of shell and CLI availability inside this run's boundary; nothing was spawned for \
             the run and no recovery event was appended, so the run is resumable."
        ),
    }
}

pub(super) enum Slots {
    None,
    Probe,
    Pipeline {
        standing: Standing,
        pool: Option<String>,
    },
}

pub(super) struct Registering<'a, L> {
    inner: &'a dyn Runner,
    ledger: &'a Mutex<L>,
    slots: Slots,
    completed: AtomicU32,
}

impl<'a, L> Registering<'a, L> {
    pub(super) const fn new(inner: &'a dyn Runner, ledger: &'a Mutex<L>, slots: Slots) -> Self {
        Self {
            inner,
            ledger,
            slots,
            completed: AtomicU32::new(0),
        }
    }

    pub(super) fn completed(&self) -> u32 {
        self.completed.load(Ordering::Relaxed)
    }
}

impl<L: BorrowMut<InvocationLedger> + Send> Runner for Registering<'_, L> {
    fn run<'a>(&'a self, request: &'a RunnerRequest, call: RunnerCall<'a>) -> RunFuture<'a> {
        Box::pin(async move {
            self.admit(request)?;
            let outcome = self.inner.run(request, call).await;
            self.settle(request, outcome)
        })
    }
}

impl<L: BorrowMut<InvocationLedger>> Registering<'_, L> {
    fn admit(&self, request: &RunnerRequest) -> Result<(), RunnerError> {
        let refused = |error: UpstrokeError| RunnerError::never_started(&request.invocation, error);
        let slots = if is_slotted(&request.invocation) {
            let standing = match &self.slots {
                Slots::None => {
                    return Err(refused(UpstrokeError::Refused {
                        message: format!(
                            "`{}` is a slotted invocation and this boundary holds no slots; \
                             INV-23's non-slotted probe is the recorded shell alone",
                            request.invocation
                        ),
                    }));
                }
                Slots::Probe => Standing::preflight(),
                Slots::Pipeline { standing, .. } => *standing,
            };
            let Some(agent) = request.agent.as_ref() else {
                return Err(refused(UpstrokeError::Refused {
                    message: format!(
                        "`{}` is a slotted invocation with no agent binding; the pair it would \
                         take is `{{agent, pool?}}` and there is no agent to name",
                        request.invocation
                    ),
                }));
            };
            let pool = match &self.slots {
                Slots::Pipeline { pool, .. } => pool.clone(),
                Slots::None | Slots::Probe => None,
            };
            Some((
                SlotPair {
                    agent: agent.to_string(),
                    pool,
                },
                standing,
            ))
        } else {
            None
        };
        let mut guard = self.ledger.lock().unwrap_or_else(PoisonError::into_inner);
        let ledger: &mut InvocationLedger = (*guard).borrow_mut();
        ledger
            .register_at_once(
                &request.invocation,
                slots
                    .as_ref()
                    .map(|(pair, standing)| (pair.clone(), standing)),
            )
            .map_err(refused)
    }

    fn settle(
        &self,
        request: &RunnerRequest,
        outcome: Result<ProcessOutput, RunnerError>,
    ) -> Result<ProcessOutput, RunnerError> {
        let settled = |error: UpstrokeError| match &outcome {
            Ok(_) => RunnerError::gone(&request.invocation, error),
            Err(failure) => RunnerError::new(&request.invocation, failure.fate, error),
        };
        let mut guard = self.ledger.lock().unwrap_or_else(PoisonError::into_inner);
        let ledger: &mut InvocationLedger = (*guard).borrow_mut();
        match &outcome {
            Ok(_) => {
                ledger.complete(&request.invocation).map_err(settled)?;
                self.completed.fetch_add(1, Ordering::Relaxed);
            }
            Err(_) => {
                ledger.cancel(&request.invocation).map_err(settled)?;
            }
        }
        drop(guard);
        outcome
    }
}

#[cfg(test)]
mod tests;
