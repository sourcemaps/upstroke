//! Extended notes: `docs/internals/engine/topology/preflight.md`

use std::borrow::BorrowMut;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, PoisonError};

use crate::agent::{AdapterSource, AgentAdapter, AuthState, Caps, ProcessOutput};
use crate::capacity::Pool;
use crate::error::UpstrokeError;
use crate::gates::ShellKind;
use crate::runner::container::resolve::RunnerPreflight;
use crate::runner::{
    CallParts, Cancellation, InvocationId, ProbeTarget, RunFuture, Runner, RunnerCall, RunnerError,
    RunnerRequest,
};
use crate::topology::events::{RunnerKind, RunnerPolicy};
use crate::workspace_manager::ComposedCommonDir;

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

const CLAUDE_CODE_OAUTH_TOKEN: &str = "CLAUDE_CODE_OAUTH_TOKEN";

const SIGN_IN_CHECKED: &[&str] = &[crate::agent::claude::ADAPTER_ID];

pub(super) const MAX_ACCOUNTS_PER_AGENT: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProfiledPool {
    pool: String,
    agent: String,
    profile: PathBuf,
}

impl ProfiledPool {
    fn of(pool: &Pool) -> Option<Self> {
        let profile = pool.profile.as_ref()?;
        Some(Self {
            pool: pool.name.clone(),
            agent: pool.agent.clone(),
            profile: PathBuf::from(profile),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Account {
    Default { pool: Option<String> },
    Profile { pool: String, profile: PathBuf },
}

impl Account {
    pub(super) fn of_agent(pools: &[Pool], agent: &str) -> Result<Vec<Self>, UpstrokeError> {
        let mut accounts: Vec<Self> = Vec::new();
        for pool in pools.iter().filter(|pool| pool.agent == agent) {
            let profile = pool.profile.as_deref().map(Path::new);
            if accounts.iter().any(|listed| listed.profile() == profile) {
                continue;
            }
            if accounts.len() >= MAX_ACCOUNTS_PER_AGENT {
                return Err(too_many_accounts(agent));
            }
            accounts.push(match profile {
                Some(profile) => Self::Profile {
                    pool: pool.name.clone(),
                    profile: profile.to_path_buf(),
                },
                None => Self::Default {
                    pool: Some(pool.name.clone()),
                },
            });
        }
        if accounts.is_empty() {
            accounts.push(Self::Default { pool: None });
        }
        Ok(accounts)
    }

    fn profile(&self) -> Option<&Path> {
        match self {
            Self::Default { .. } => None,
            Self::Profile { profile, .. } => Some(profile),
        }
    }
}

pub(in crate::engine) fn coordinator_environment(key: &str) -> Option<OsString> {
    std::env::var_os(key)
}

pub struct RunPreflight<'a> {
    runner: &'a dyn Runner,
    adapters: &'a dyn AdapterSource,
    shell: ShellKind,
    workspace: PathBuf,
    agents: Vec<String>,
    pools: Vec<Pool>,
    environment: fn(&str) -> Option<OsString>,
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
            pools: Vec::new(),
            environment: coordinator_environment,
            ledger: Mutex::new(InvocationLedger::new()),
            probed: Mutex::new(None),
        }
    }

    #[must_use]
    pub fn with_pools(mut self, pools: &[Pool]) -> Self {
        self.pools = pools.to_vec();
        self
    }

    #[must_use]
    fn reading_environment(mut self, environment: fn(&str) -> Option<OsString>) -> Self {
        self.environment = environment;
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
        refuse_unusable_profiles(&self.pools, policy, self.environment)?;
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
            let accounts = Account::of_agent(&self.pools, agent)?;
            refuse_signed_out(agent, adapter, &probed, &registering, &accounts).map_err(
                |failure| match failure {
                    SignIn::SignedOut(refusal) => refusal,
                    SignIn::Undiscovered(error) => refused(
                        policy,
                        &format!("the `{agent}` CLI's sign-in discovery: {error}"),
                    ),
                },
            )?;
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

pub(super) fn refuse_unusable_profiles(
    pools: &[Pool],
    policy: &RunnerPolicy,
    environment: fn(&str) -> Option<OsString>,
) -> Result<(), UpstrokeError> {
    let profiled: Vec<ProfiledPool> = pools.iter().filter_map(ProfiledPool::of).collect();
    let Some(first) = profiled.first() else {
        return Ok(());
    };
    if policy.kind == RunnerKind::Container {
        return Err(profile_refused(&format!(
            "pool `{}` names the credential profile `{}`, and a profile is supported on the \
             host runner only; this run's runner is the container runner",
            first.pool,
            first.profile.display()
        )));
    }
    for pool in &profiled {
        if pool.agent != crate::agent::claude::ADAPTER_ID {
            return Err(profile_refused(&format!(
                "pool `{}` names the credential profile `{}` for agent `{}`, and this stage \
                 supports Claude Code's profile only (agent `{}`)",
                pool.pool,
                pool.profile.display(),
                pool.agent,
                crate::agent::claude::ADAPTER_ID
            )));
        }
        if !pool.profile.is_absolute() {
            return Err(profile_refused(&format!(
                "pool `{}` names the credential profile `{}`, which is a relative path; each \
                 task runs in its own directory, so a relative path would name a different \
                 directory for each process, and `~` is not expanded; give the profile as an \
                 absolute path",
                pool.pool,
                pool.profile.display()
            )));
        }
        let problem = match std::fs::metadata(&pool.profile) {
            Ok(metadata) if metadata.is_dir() => continue,
            Ok(_) => "which is not a directory".to_owned(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                "which does not exist".to_owned()
            }
            Err(error) => format!("which cannot be inspected: {error}"),
        };
        return Err(profile_refused(&format!(
            "pool `{}` names the credential profile directory `{}`, {problem}",
            pool.pool,
            pool.profile.display()
        )));
    }
    if environment(CLAUDE_CODE_OAUTH_TOKEN).is_some() {
        return Err(profile_refused(&format!(
            "`{CLAUDE_CODE_OAUTH_TOKEN}` is set in this coordinator's environment, and a token \
             overrides a profile's credentials, so the run would not use the profile pool `{}` \
             names (`{}`); unset it to run under the profile",
            first.pool,
            first.profile.display()
        )));
    }
    Ok(())
}

fn profile_refused(what: &str) -> UpstrokeError {
    UpstrokeError::Refused {
        message: format!(
            "pre-flight: {what}. No agent CLI was started and no event was appended; correct \
             the pools file or the environment and run the same command again"
        ),
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
             the run and no recovery event was appended, so the run is resumable"
        ),
    }
}

#[derive(Debug)]
pub(super) enum SignIn {
    SignedOut(UpstrokeError),
    Undiscovered(UpstrokeError),
}

impl SignIn {
    pub(super) fn into_error(self) -> UpstrokeError {
        match self {
            Self::SignedOut(error) | Self::Undiscovered(error) => error,
        }
    }
}

pub(super) fn refuse_signed_out(
    agent: &str,
    adapter: &dyn AgentAdapter,
    caps: &Caps,
    through: &dyn Runner,
    accounts: &[Account],
) -> Result<(), SignIn> {
    if !SIGN_IN_CHECKED.contains(&agent) {
        return Ok(());
    }
    for (further, account) in (0..).zip(accounts) {
        let discovered = if further == 0 && account.profile().is_none() {
            adapter.discover(through, caps)
        } else {
            let under = UnderPool {
                boundary: through,
                profile: account.profile(),
                further,
            };
            adapter.discover(&under, caps)
        };
        let discovery = discovered.map_err(SignIn::Undiscovered)?;
        signed_in(agent, account, discovery.auth)?;
    }
    Ok(())
}

fn too_many_accounts(agent: &str) -> UpstrokeError {
    UpstrokeError::Refused {
        message: format!(
            "pre-flight: agent `{agent}`'s pools use more than {MAX_ACCOUNTS_PER_AGENT} distinct \
             accounts (credential profiles, with the default account counting as one), and \
             pre-flight checks the sign-in of at most {MAX_ACCOUNTS_PER_AGENT} accounts per \
             agent. Nothing was spawned for the run and no event was appended; give the agent's \
             pools at most {MAX_ACCOUNTS_PER_AGENT} distinct accounts and run the same command \
             again"
        ),
    }
}

fn signed_in(agent: &str, account: &Account, auth: AuthState) -> Result<(), SignIn> {
    match auth {
        AuthState::NotAuthenticated => Err(SignIn::SignedOut(signed_out(agent, account))),
        AuthState::Authenticated | AuthState::Unknown => Ok(()),
    }
}

fn signed_out(agent: &str, account: &Account) -> UpstrokeError {
    let claude_code = agent == crate::agent::claude::ADAPTER_ID;
    let default_login = || {
        if claude_code {
            "`claude auth login`".to_owned()
        } else {
            "with the agent's own CLI".to_owned()
        }
    };
    let settings = if claude_code {
        " The check loads no user, project or local settings, as a worker loads none, so a \
         credential that only a settings file supplies does not count."
    } else {
        ""
    };
    let (account, login) = match account {
        Account::Profile { pool, profile } => (
            format!(
                "pool `{pool}`'s account, the credential profile directory `{}`",
                profile.display()
            ),
            if claude_code {
                format!(
                    "`claude auth login` with `CLAUDE_CONFIG_DIR` set to `{}`",
                    profile.display()
                )
            } else {
                format!("with the agent's own CLI, under `{}`", profile.display())
            },
        ),
        Account::Default { pool: Some(pool) } => (
            format!(
                "pool `{pool}`'s account, as `{pool}` names no credential profile, so it runs \
                 under the agent's default account"
            ),
            default_login(),
        ),
        Account::Default { pool: None } => (
            "its default account, as no pool of this run names it".to_owned(),
            default_login(),
        ),
    };
    UpstrokeError::Refused {
        message: format!(
            "pre-flight: agent `{agent}` is not signed in under {account}; its sign-in discovery \
             reported no signed-in account.{settings} Nothing was spawned for the run and no \
             event was appended; run the same command again once that account is signed in \
             ({login})"
        ),
    }
}

struct UnderPool<'a> {
    boundary: &'a dyn Runner,
    profile: Option<&'a Path>,
    further: u32,
}

impl UnderPool<'_> {
    fn under(&self, request: &RunnerRequest) -> Result<RunnerRequest, UpstrokeError> {
        let mut under = request.clone();
        if let Some(profile) = self.profile {
            under = under.with_credential_profile(profile.to_path_buf());
        }
        if self.further == 0 {
            return Ok(under);
        }
        let InvocationId::Probe {
            target: ProbeTarget::Agent(agent),
            ordinal,
        } = &request.invocation
        else {
            return Err(UpstrokeError::Refused {
                message: format!(
                    "`{}` is not an agent probe; a pool's sign-in discovery runs agent probes \
                     only, and only an agent probe has a further pool's identity to take",
                    request.invocation
                ),
            });
        };
        under.invocation =
            PreflightIdentities::further_pool(agent.as_str(), self.further, *ordinal)?;
        Ok(under)
    }
}

impl Runner for UnderPool<'_> {
    fn run<'a>(&'a self, request: &'a RunnerRequest, call: RunnerCall<'a>) -> RunFuture<'a> {
        let under = match self.under(request) {
            Ok(under) => under,
            Err(error) => {
                return Box::pin(std::future::ready(Err(RunnerError::never_started(
                    &request.invocation,
                    error,
                ))));
            }
        };
        Box::pin(async move {
            let CallParts {
                cancellation,
                leases,
                spawn,
                container,
            } = call.into_parts();
            let mut relent = RunnerCall::new(cancellation).holding_cleanup_leases(leases);
            if let Some(spawn) = spawn {
                relent = relent.observed_by(spawn);
            }
            if let Some(container) = container {
                relent = relent.observed_in_container_by(container);
            }
            self.boundary.run(&under, relent).await
        })
    }

    fn composed_common_dir(&self, request: &RunnerRequest) -> ComposedCommonDir {
        match self.under(request) {
            Ok(under) => self.boundary.composed_common_dir(&under),
            Err(_) => self.boundary.composed_common_dir(request),
        }
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
    // What the inner runner composes for the `GIT_COMMON_DIR` of each request
    // this boundary admitted, in admission order. `run` pushes one report per
    // admitted request before it hands that request to the inner runner, and
    // holds the lock for that push alone, never across the inner run. The
    // owner reads the reports once, after the last run has returned, by
    // consuming the boundary (`into_composed`). A poisoned lock keeps the
    // reports pushed before the panic.
    composed: Mutex<Vec<ComposedCommonDir>>,
}

impl<'a, R: ?Sized> Registering<'a, R> {
    pub(super) const fn new(inner: &'a dyn Runner, registrar: &'a R, slots: Slots) -> Self {
        Self {
            inner,
            registrar,
            slots,
            carried: None,
            completed: AtomicU32::new(0),
            composed: Mutex::new(Vec::new()),
        }
    }

    pub(super) const fn carrying(mut self, carried: &'a Carried) -> Self {
        self.carried = Some(carried);
        self
    }

    pub(super) fn completed(&self) -> u32 {
        self.completed.load(Ordering::Relaxed)
    }

    pub(super) fn into_composed(self) -> Vec<ComposedCommonDir> {
        self.composed
            .into_inner()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

impl<R: Registrar + ?Sized> Runner for Registering<'_, R> {
    fn run<'a>(&'a self, request: &'a RunnerRequest, call: RunnerCall<'a>) -> RunFuture<'a> {
        Box::pin(async move {
            self.admit(request)?;
            let composed = self.inner.composed_common_dir(request);
            self.composed
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(composed);
            let call = match self.carried {
                Some(carried) => carried.rebuild(call),
                None => call,
            };
            let outcome = self.inner.run(request, call).await;
            self.settle(request, outcome)
        })
    }

    fn composed_common_dir(&self, request: &RunnerRequest) -> ComposedCommonDir {
        self.inner.composed_common_dir(request)
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
