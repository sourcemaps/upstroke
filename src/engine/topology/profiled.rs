//! Extended notes: `docs/internals/engine/topology/profiled.md`

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::capacity::{Pool, pool_for};
use crate::runner::{CallParts, ExecutionRole, RunFuture, Runner, RunnerCall, RunnerRequest};
use crate::workspace_manager::ComposedCommonDir;

pub(super) fn profiled(inner: Arc<dyn Runner>, pools: &[Pool]) -> Arc<dyn Runner> {
    let profiles = profiles(pools);
    if profiles.is_empty() {
        return inner;
    }
    Arc::new(Profiled { inner, profiles })
}

fn profiles(pools: &[Pool]) -> BTreeMap<String, PathBuf> {
    pools
        .iter()
        .filter_map(|pool| {
            let bound = pool_for(&pool.agent, pools)?;
            let profile = bound.profile.as_ref()?;
            Some((bound.agent.clone(), PathBuf::from(profile)))
        })
        .collect()
}

struct Profiled {
    inner: Arc<dyn Runner>,
    profiles: BTreeMap<String, PathBuf>,
}

impl Profiled {
    fn stamped(&self, request: &RunnerRequest) -> Option<RunnerRequest> {
        if request.role == ExecutionRole::Implement || request.credential_profile.is_some() {
            return None;
        }
        let agent = request.agent.as_ref()?;
        let profile = self.profiles.get(agent.as_str())?;
        Some(request.clone().with_credential_profile(profile.clone()))
    }
}

impl Runner for Profiled {
    fn run<'a>(&'a self, request: &'a RunnerRequest, call: RunnerCall<'a>) -> RunFuture<'a> {
        match self.stamped(request) {
            None => self.inner.run(request, call),
            Some(stamped) => Box::pin(async move {
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
                self.inner.run(&stamped, relent).await
            }),
        }
    }

    fn composed_common_dir(&self, request: &RunnerRequest) -> ComposedCommonDir {
        match self.stamped(request) {
            None => self.inner.composed_common_dir(request),
            Some(stamped) => self.inner.composed_common_dir(&stamped),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex, PoisonError};
    use std::time::Duration;

    use crate::agent::ProcessOutput;
    use crate::capacity::{Pool, PoolKind};
    use crate::runner::contract::tests::InlineRunner;
    use crate::runner::invocation::AttemptRole;
    use crate::runner::{
        AgentId, CommandSpec, InvocationId, Runner, RunnerError, RunnerRequest, gate_request,
        review_request, worker_request,
    };
    use crate::topology::events::{AttemptNumber, GenerationId};
    use crate::topology::registry::TaskKey;

    use super::profiled;

    #[derive(Debug, Default)]
    struct Recording {
        seen: Mutex<Vec<RunnerRequest>>,
    }

    impl Recording {
        fn seen(&self) -> Vec<RunnerRequest> {
            self.seen
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone()
        }
    }

    impl InlineRunner for Recording {
        fn run_inline(&self, request: &RunnerRequest) -> Result<ProcessOutput, RunnerError> {
            self.seen
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(request.clone());
            Ok(ProcessOutput {
                code: Some(0),
                stdout: String::new(),
                stderr: String::new(),
                duration: Duration::ZERO,
                timed_out: false,
                output_limited: false,
            })
        }
    }

    fn pool(name: &str, agent: &str, profile: Option<&str>) -> Pool {
        let mut pool = Pool::discovered(name, PoolKind::SubscriptionWindow, agent, Vec::new());
        pool.profile = profile.map(str::to_owned);
        pool
    }

    fn invocation(role: AttemptRole) -> InvocationId {
        InvocationId::attempt(TaskKey(0), GenerationId(0), AttemptNumber(1), role, 0)
    }

    fn worker(agent: &str) -> RunnerRequest {
        worker_request(
            CommandSpec::new(agent).arg("--implement"),
            PathBuf::from("/tmp/ws"),
            AgentId::new(agent),
            Duration::from_secs(30),
            invocation(AttemptRole::Worker),
        )
    }

    fn reviewer(agent: &str) -> RunnerRequest {
        review_request(
            CommandSpec::new(agent).arg("--review"),
            PathBuf::from("/tmp/ws"),
            AgentId::new(agent),
            Duration::from_secs(30),
            invocation(AttemptRole::ReviewPass(0)),
        )
    }

    fn through(pools: &[Pool], request: &RunnerRequest) -> RunnerRequest {
        let recording = Arc::new(Recording::default());
        let runner = profiled(Arc::clone(&recording) as Arc<dyn Runner>, pools);
        runner
            .run_blocking(request)
            .expect("the recording runner answers every request");
        let seen = recording.seen();
        assert_eq!(seen.len(), 1, "one request in, one request through");
        seen.into_iter().next().expect("one request")
    }

    fn same(left: &RunnerRequest, right: &RunnerRequest) -> bool {
        format!("{left:?}") == format!("{right:?}")
    }

    #[test]
    fn a_request_from_an_agent_with_a_profiled_pool_carries_that_profile() {
        let pools = [
            pool("max", "claude-code", Some("/profiles/personal")),
            pool("work", "codex", Some("/profiles/work")),
        ];
        let request = reviewer("claude-code");

        let seen = through(&pools, &request);

        assert_eq!(
            seen.credential_profile.as_deref(),
            Some(Path::new("/profiles/personal")),
            "the reviewer ran without the profile its agent's pool is bound to"
        );
        let unstamped = RunnerRequest {
            credential_profile: None,
            ..seen.clone()
        };
        assert!(
            same(&unstamped, &request),
            "the wrapper changed more than the credential profile: {seen:?}"
        );

        assert_eq!(
            through(&pools, &reviewer("codex"))
                .credential_profile
                .as_deref(),
            Some(Path::new("/profiles/work")),
            "a reviewer takes its own agent's profile, not the first one in the file"
        );
    }

    #[test]
    fn a_request_from_an_agent_whose_pool_has_no_profile_goes_through_untouched() {
        let pools = [
            pool("max", "claude-code", None),
            pool("work", "codex", Some("/profiles/work")),
        ];
        let request = reviewer("claude-code");

        let seen = through(&pools, &request);

        assert!(
            same(&seen, &request),
            "an agent with an unprofiled pool had its request changed: {seen:?}"
        );
        assert_eq!(seen.credential_profile, None);
    }

    #[test]
    fn the_first_pool_for_an_agent_decides_its_profile_as_everywhere_else() {
        let pools = [
            pool("max", "claude-code", None),
            pool("spare", "claude-code", Some("/profiles/spare")),
        ];
        let request = reviewer("claude-code");

        let seen = through(&pools, &request);

        assert_eq!(
            seen.credential_profile, None,
            "`capacity::pool_for` binds an agent to its first pool; a later pool's \
             profile was applied to it"
        );
    }

    #[test]
    fn a_worker_request_keeps_the_credentials_its_attempt_was_planned_with() {
        let pools = [
            pool("max", "claude-code", Some("/profiles/personal")),
            pool("spare", "claude-code", Some("/profiles/spare")),
        ];
        let unprofiled = worker("claude-code");
        let on_spare =
            worker("claude-code").with_credential_profile(PathBuf::from("/profiles/spare"));

        let seen_unprofiled = through(&pools, &unprofiled);
        let seen_on_spare = through(&pools, &on_spare);

        assert_eq!(
            (
                seen_unprofiled.credential_profile.clone(),
                seen_on_spare.credential_profile.clone(),
            ),
            (None, Some(PathBuf::from("/profiles/spare"))),
            "a worker planned with no profile, then one planned on `spare`: the wrapper \
             put its agent's first pool's profile over what the attempt's plan decided"
        );
        assert!(
            same(&seen_unprofiled, &unprofiled) && same(&seen_on_spare, &on_spare),
            "a worker request was changed on its way through: {seen_unprofiled:?}, \
             {seen_on_spare:?}"
        );
    }

    #[test]
    fn a_probe_that_already_carries_a_profile_keeps_it() {
        let pools = [
            pool("max", "claude-code", Some("/profiles/personal")),
            pool("spare", "claude-code", Some("/profiles/spare")),
        ];
        let unprofiled = crate::agent::probe_request(
            "claude-code",
            CommandSpec::new("claude")
                .arg("auth")
                .arg("status")
                .arg("--json"),
            2,
            Duration::from_secs(30),
        )
        .expect("a probe request");
        let on_spare = unprofiled
            .clone()
            .with_credential_profile(PathBuf::from("/profiles/spare"));

        let seen_unprofiled = through(&pools, &unprofiled);
        let seen_on_spare = through(&pools, &on_spare);

        assert_eq!(
            (
                seen_unprofiled.credential_profile.clone(),
                seen_on_spare.credential_profile.clone(),
            ),
            (
                Some(PathBuf::from("/profiles/personal")),
                Some(PathBuf::from("/profiles/spare")),
            ),
            "a probe with no profile takes its agent's first pool's, and one that already \
             names `spare`'s keeps it, as pre-flight's discovery under a further pool needs"
        );
        assert!(
            same(&seen_on_spare, &on_spare),
            "a probe that carried a profile was changed on its way through: {seen_on_spare:?}"
        );
    }

    #[test]
    fn a_request_with_no_agent_goes_through_untouched() {
        let pools = [pool("max", "claude-code", Some("/profiles/personal"))];
        let gate = gate_request(
            CommandSpec::new("sh").arg("-c").arg("cargo test"),
            PathBuf::from("/tmp/ws"),
            Duration::from_secs(30),
            invocation(AttemptRole::Gate(0)),
        );

        let seen = through(&pools, &gate);

        assert!(
            same(&seen, &gate),
            "a gate was given a credential profile, or otherwise changed: {seen:?}"
        );
        assert_eq!(seen.credential_profile, None);
    }

    #[test]
    fn with_no_profiled_pool_the_runner_is_not_wrapped_at_all() {
        let recording: Arc<dyn Runner> = Arc::new(Recording::default());
        for pools in [
            Vec::new(),
            vec![
                pool("max", "claude-code", None),
                pool("work", "codex", None),
            ],
        ] {
            let runner = profiled(Arc::clone(&recording), &pools);
            assert!(
                Arc::ptr_eq(&runner, &recording),
                "a run with no profiled pool got a wrapper over its runner: {pools:?}"
            );
        }
    }

    #[test]
    fn a_profile_is_the_configured_string_used_as_a_path() {
        let pools = [pool("max", "claude-code", Some("~/profiles/personal"))];

        let seen = through(&pools, &reviewer("claude-code"));

        assert_eq!(
            seen.credential_profile,
            Some(PathBuf::from("~/profiles/personal")),
            "the configuration has no `~/` expansion helper, so the profile is \
             passed as written; pre-flight is where it is checked"
        );
    }
}
