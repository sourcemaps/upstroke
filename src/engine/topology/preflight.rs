//! Extended notes: `docs/internals/engine/topology/preflight.md`

use std::borrow::BorrowMut;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, PoisonError};

use crate::agent::{AdapterSource, Caps, ProcessOutput};
use crate::error::UpstrokeError;
use crate::gates::ShellKind;
use crate::runner::container::resolve::RunnerPreflight;
use crate::runner::{
    Cancellation, InvocationId, RunFuture, Runner, RunnerCall, RunnerError, RunnerRequest,
};
use crate::topology::events::RunnerPolicy;

use super::identity::{InvocationEnd, InvocationLedger, PreflightIdentities, SlotPair, is_slotted};
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
    #[cfg(unix)]
    reaper: Option<&'a super::coordinator::IncarnationReaper>,
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
            #[cfg(unix)]
            reaper: None,
        }
    }

    #[cfg(unix)]
    #[must_use]
    pub const fn reaping(mut self, reaper: &'a super::coordinator::IncarnationReaper) -> Self {
        self.reaper = Some(reaper);
        self
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

    fn registering(&self) -> Registering<'_, Mutex<InvocationLedger>> {
        Registering::new(self.runner, &self.ledger, Slots::Probe)
    }
}

impl RunnerPreflight for RunPreflight<'_> {
    fn certify(&self, policy: &RunnerPolicy) -> Result<(), UpstrokeError> {
        #[cfg(unix)]
        let covered = match self.reaper {
            Some(reaper) => Some(reaper.cover(&[]).map_err(|error| UpstrokeError::Refused {
                message: format!(
                    "pre-flight: the run's container reaper could not be armed before its first \
                     probe ({error}); nothing was spawned for the run and no recovery event was \
                     appended, so the run is resumable"
                ),
            })?),
            None => None,
        };
        let certified = self.probe(policy);
        #[cfg(unix)]
        if let Some(covered) = covered {
            covered.close(self.ledgers_balance());
        }
        certified
    }
}

impl RunPreflight<'_> {
    fn probe(&self, policy: &RunnerPolicy) -> Result<(), UpstrokeError> {
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

pub trait Registrar: Sync {
    fn admit(
        &self,
        invocation: &InvocationId,
        slots: Option<(SlotPair, Standing)>,
    ) -> Result<(), UpstrokeError>;

    fn ended(&self, invocation: &InvocationId, end: InvocationEnd) -> Result<(), UpstrokeError>;

    fn snapshot_begin(&self) -> Result<(), UpstrokeError> {
        Ok(())
    }

    fn snapshot_end(&self) {}
}

// The synchronous registrar: the ledger's owner calls it from the one thread
// that runs every invocation, so a pair it cannot grant at once is refused
// (R-O). The lock guards the register and settle calls only, never the Runner
// call between them.
impl<L: BorrowMut<InvocationLedger> + Send> Registrar for Mutex<L> {
    fn admit(
        &self,
        invocation: &InvocationId,
        slots: Option<(SlotPair, Standing)>,
    ) -> Result<(), UpstrokeError> {
        let mut guard = self.lock().unwrap_or_else(PoisonError::into_inner);
        let ledger: &mut InvocationLedger = (*guard).borrow_mut();
        ledger.register_at_once(
            invocation,
            slots
                .as_ref()
                .map(|(pair, standing)| (pair.clone(), standing)),
        )
    }

    fn ended(&self, invocation: &InvocationId, end: InvocationEnd) -> Result<(), UpstrokeError> {
        let mut guard = self.lock().unwrap_or_else(PoisonError::into_inner);
        let ledger: &mut InvocationLedger = (*guard).borrow_mut();
        ledger.end(invocation, &end).map(drop)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Carried {
    cancellation: Cancellation,
    leases: Vec<PathBuf>,
}

impl Carried {
    #[must_use]
    pub const fn new(cancellation: Cancellation, leases: Vec<PathBuf>) -> Self {
        Self {
            cancellation,
            leases,
        }
    }

    #[must_use]
    pub fn call(&self) -> RunnerCall<'_> {
        RunnerCall::new(self.cancellation.clone()).holding_cleanup_leases(&self.leases)
    }

    fn rebuild<'a>(&'a self, call: RunnerCall<'a>) -> RunnerCall<'a> {
        let parts = call.into_parts();
        let mut rebuilt = self.call();
        if let Some(spawn) = parts.spawn {
            rebuilt = rebuilt.observed_by(spawn);
        }
        if let Some(container) = parts.container {
            rebuilt = rebuilt.observed_in_container_by(container);
        }
        rebuilt
    }
}

pub(super) struct Registering<'a, R: ?Sized> {
    inner: &'a dyn Runner,
    registrar: &'a R,
    slots: Slots,
    carried: Option<&'a Carried>,
    completed: AtomicU32,
}

impl<'a, R: ?Sized> Registering<'a, R> {
    pub(super) const fn new(inner: &'a dyn Runner, registrar: &'a R, slots: Slots) -> Self {
        Self {
            inner,
            registrar,
            slots,
            carried: None,
            completed: AtomicU32::new(0),
        }
    }

    pub(super) const fn carrying(mut self, carried: &'a Carried) -> Self {
        self.carried = Some(carried);
        self
    }

    pub(super) fn completed(&self) -> u32 {
        self.completed.load(Ordering::Relaxed)
    }
}

impl<R: Registrar + ?Sized> Runner for Registering<'_, R> {
    fn run<'a>(&'a self, request: &'a RunnerRequest, call: RunnerCall<'a>) -> RunFuture<'a> {
        Box::pin(async move {
            self.admit(request)?;
            let call = match self.carried {
                Some(carried) => carried.rebuild(call),
                None => call,
            };
            let outcome = self.inner.run(request, call).await;
            self.settle(request, outcome)
        })
    }
}

impl<R: Registrar + ?Sized> Registering<'_, R> {
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
        self.registrar
            .admit(&request.invocation, slots)
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
        self.registrar
            .ended(&request.invocation, InvocationEnd::of(&outcome))
            .map_err(settled)?;
        if outcome.is_ok() {
            self.completed.fetch_add(1, Ordering::Relaxed);
        }
        outcome
    }
}

#[cfg(test)]
mod tests;
