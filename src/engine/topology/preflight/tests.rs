//! Extended notes: `docs/internals/engine/topology/preflight/tests.md`

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use super::*;
use crate::agent::AgentAdapter;
use crate::capacity::PoolKind;
use crate::gates::ShellKind;
use crate::ir::Outcome;
use crate::runner::invocation::AttemptRole;
use crate::runner::{AgentId, CommandSpec, InvocationId, ProbeTarget};
use crate::topology::events::{
    AttemptNumber, GenerationId, ImageIdentity, RunnerContract, RunnerKind,
};
use crate::topology::registry::TaskKey;

const AGENT: &str = "claude-code";
const OTHER: &str = "copilot";

#[derive(Debug, Default)]
struct Recording {
    seen: Mutex<Vec<RunnerRequest>>,
    failing: Option<String>,
    refuses: bool,
}

impl Recording {
    fn failing(program: &str) -> Self {
        Self {
            failing: Some(program.to_owned()),
            ..Self::default()
        }
    }

    fn refusing() -> Self {
        Self {
            refuses: true,
            ..Self::default()
        }
    }

    fn programs(&self) -> Vec<String> {
        self.seen
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .map(|request| request.command.program.clone())
            .collect()
    }

    fn requests(&self) -> Vec<RunnerRequest> {
        self.seen
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl crate::runner::contract::tests::InlineRunner for Recording {
    fn run_inline(
        &self,
        request: &RunnerRequest,
    ) -> Result<ProcessOutput, crate::runner::RunnerError> {
        self.seen
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(request.clone());
        if self.refuses {
            return Err(crate::runner::RunnerError::never_started(
                &request.invocation,
                UpstrokeError::Refused {
                    message: "the boundary refused to start this process".to_owned(),
                },
            ));
        }
        Ok(ProcessOutput {
            code: Some(
                if self.failing.as_deref() == Some(request.command.program.as_str()) {
                    127
                } else {
                    0
                },
            ),
            stdout: "9.9.9".to_owned(),
            stderr: String::new(),
            duration: Duration::from_millis(1),
            timed_out: false,
            output_limited: false,
        })
    }
}

#[derive(Debug)]
struct Stub(&'static str);

impl AgentAdapter for Stub {
    fn id(&self) -> &'static str {
        self.0
    }

    fn probe(&self, runner: &dyn Runner) -> Result<Caps, UpstrokeError> {
        let request = crate::agent::probe_request(
            self.0,
            CommandSpec::new(self.0).arg("--version"),
            0,
            Duration::from_secs(10),
        )?;
        let output = runner.run_blocking(&request)?;
        if output.code != Some(0) {
            return Err(UpstrokeError::Agent {
                message: format!("`{} --version` exited {:?}", self.0, output.code),
            });
        }
        Ok(Caps {
            version: output.stdout.trim().to_owned(),
            json_output: true,
            session_resume: false,
            cost_reporting: false,
            read_only_mode: false,
            acp: false,
            model_list: false,
        })
    }

    fn build(&self, _run: &crate::agent::TaskRun) -> Result<CommandSpec, UpstrokeError> {
        Ok(CommandSpec::new(self.0))
    }

    fn parse(&self, _out: &ProcessOutput) -> Result<Outcome, UpstrokeError> {
        Err(UpstrokeError::Agent {
            message: "the fixture adapter runs no attempt".to_owned(),
        })
    }
}

struct Registry;

impl AdapterSource for Registry {
    fn get(&self, id: &str) -> Option<&dyn AgentAdapter> {
        match id {
            AGENT => Some(&Stub(AGENT) as &dyn AgentAdapter),
            OTHER => Some(&Stub(OTHER) as &dyn AgentAdapter),
            _ => None,
        }
    }
}

fn container_policy() -> RunnerPolicy {
    RunnerPolicy {
        kind: RunnerKind::Container,
        policy: RunnerContract::ContainerV1,
        image: Some(ImageIdentity {
            reference: "ghcr.io/example/runner:2".to_owned(),
            id: "sha256:abc".to_owned(),
            digest: None,
        }),
        credential_volumes: Some(Default::default()),
        mounts: Vec::new(),
    }
}

fn host_policy() -> RunnerPolicy {
    RunnerPolicy {
        kind: RunnerKind::Host,
        policy: RunnerContract::HostV1,
        image: None,
        credential_volumes: None,
        mounts: Vec::new(),
    }
}

fn preflight<'a>(
    runner: &'a dyn Runner,
    adapters: &'a Registry,
    agents: &[&str],
) -> RunPreflight<'a> {
    RunPreflight::new(
        runner,
        adapters,
        ShellKind::Bash,
        Path::new("."),
        agents.iter().map(|agent| (*agent).to_owned()).collect(),
    )
}

#[test]
fn a_preflight_is_the_shell_probe_then_one_probe_per_recorded_agent() {
    let runner = Recording::default();
    let adapters = Registry;
    let preflight = preflight(&runner, &adapters, &[OTHER, AGENT]);

    preflight
        .certify(&container_policy())
        .expect("every probe answers");

    assert_eq!(
        runner.programs(),
        vec!["bash".to_owned(), OTHER.to_owned(), AGENT.to_owned()],
        "the shell probe is first and the agents follow in the order the run recorded them"
    );
    let slotted: Vec<bool> = runner
        .requests()
        .iter()
        .map(|request| is_slotted(&request.invocation))
        .collect();
    assert_eq!(
        slotted,
        vec![false, true, true],
        "`permits.agent_pool_slots` excludes the shell probe by name and slots every agent probe"
    );
    let probed = preflight
        .probed()
        .expect("a successful certify records what it probed");
    assert_eq!(probed.agents(), [OTHER.to_owned(), AGENT.to_owned()]);
    assert_eq!(
        preflight.settlements(),
        (3, 0),
        "three processes ran and three completed; nothing was cancelled"
    );
    assert_eq!(
        probed
            .caps
            .iter()
            .map(|(agent, caps)| (agent.as_str(), caps.version.as_str()))
            .collect::<Vec<_>>(),
        vec![(OTHER, "9.9.9"), (AGENT, "9.9.9")],
        "and what each CLI said about itself"
    );
    assert!(preflight.ledgers_balance());
    assert!(preflight.running().is_empty());
}

#[test]
fn every_probe_carries_the_identity_the_preflight_mints_for_it() {
    let runner = Recording::default();
    let adapters = Registry;
    let preflight = preflight(&runner, &adapters, &[AGENT]);
    preflight
        .certify(&host_policy())
        .expect("both probes answer");

    let identities: Vec<InvocationId> = runner
        .requests()
        .iter()
        .map(|request| request.invocation.clone())
        .collect();
    assert_eq!(
        identities,
        vec![
            PreflightIdentities::shell(0).expect("the shell identity"),
            PreflightIdentities::agent(AGENT, 0).expect("the agent identity"),
        ]
    );
}

#[test]
fn a_failing_shell_refuses_before_any_agent_is_probed() {
    let runner = Recording::failing("bash");
    let adapters = Registry;
    let preflight = preflight(&runner, &adapters, &[AGENT]);

    let error = preflight
        .certify(&container_policy())
        .expect_err("a shell that cannot run `exit 0` refuses");
    let text = error.to_string();
    assert!(text.contains("the recorded shell"), "{text}");
    assert!(
        text.contains("ghcr.io/example/runner:2") && text.contains("sha256:abc"),
        "the refusal names the image and its id: {text}"
    );
    assert_eq!(
        runner.programs(),
        vec!["bash".to_owned()],
        "no agent is probed after the shell fails"
    );
    assert!(preflight.ledgers_balance(), "and the ledgers still balance");
    assert!(preflight.probed().is_none(), "nothing was certified");
}

#[test]
fn a_failing_agent_cli_refuses_naming_the_agent_and_releases_its_slot() {
    let runner = Recording::failing(AGENT);
    let adapters = Registry;
    let preflight = preflight(&runner, &adapters, &[AGENT]);

    let error = preflight
        .certify(&host_policy())
        .expect_err("a CLI that does not answer refuses");
    let text = error.to_string();
    assert!(text.contains(AGENT), "{text}");
    assert!(
        text.contains("this host"),
        "a host record names the host rather than an image: {text}"
    );
    assert!(
        preflight.ledgers_balance(),
        "R3 releases on cancel as well as on complete; still running: {:?}",
        preflight.running()
    );
}

#[test]
fn a_refused_spawn_cancels_its_registration_rather_than_completing_it() {
    let runner = Recording::refusing();
    let adapters = Registry;
    let preflight = preflight(&runner, &adapters, &[AGENT]);

    preflight
        .certify(&host_policy())
        .expect_err("a boundary that will not spawn refuses");
    assert!(preflight.ledgers_balance());
    assert!(
        preflight.running().is_empty(),
        "the registration was settled: {:?}",
        preflight.running()
    );
    assert_eq!(
        preflight.settlements(),
        (0, 1),
        "settled as cancelled, not completed: nothing ran, so nothing completed"
    );
}

#[test]
fn an_agent_with_no_adapter_refuses_naming_it() {
    let runner = Recording::default();
    let adapters = Registry;
    let preflight = preflight(&runner, &adapters, &["nobody-ships-this"]);

    let error = preflight
        .certify(&host_policy())
        .expect_err("an unknown agent refuses");
    assert!(error.to_string().contains("nobody-ships-this"), "{error}");
    assert!(preflight.ledgers_balance());
}

#[test]
fn a_probe_refusal_ends_saying_the_run_is_resumable_and_without_a_full_stop() {
    const PROBE_REFUSAL_ENDING: &str = "so the run is resumable";

    for policy in [host_policy(), container_policy()] {
        let error = refused(&policy, &format!("no adapter is registered for `{AGENT}`"));
        let text = error.to_string();
        assert!(
            matches!(error, UpstrokeError::Refused { .. }),
            "a refusal, not some other failure: {text}"
        );
        assert!(
            text.starts_with("pre-flight: "),
            "the refusal shape: {text}"
        );
        assert!(
            text.ends_with(PROBE_REFUSAL_ENDING),
            "a probe refusal ends saying the run is resumable: {text}"
        );
        assert!(
            !text.ends_with('.'),
            "and with no full stop, as `Display` text takes no trailing period: {text}"
        );
        assert_eq!(
            text.matches(PROBE_REFUSAL_ENDING).count(),
            1,
            "and says it once: {text}"
        );
    }
}

#[test]
fn a_slotted_invocation_without_an_agent_binding_is_refused() {
    let runner = Recording::default();
    let adapters = Registry;
    let preflight = preflight(&runner, &adapters, &[]);
    let registering = preflight.registering();

    let request = RunnerRequest {
        command: CommandSpec::new("claude"),
        workspace: PathBuf::from("."),
        role: crate::runner::ExecutionRole::Probe(ProbeTarget::Agent(AgentId::new(AGENT))),
        timeout: Duration::from_secs(1),
        agent: None,
        invocation: PreflightIdentities::agent(AGENT, 0).expect("a slotted probe identity"),
        credential_profile: None,
    };
    let error = registering
        .run_blocking(&request)
        .expect_err("a slotted invocation with no agent binding cannot be accounted");
    assert!(error.to_string().contains("no agent binding"), "{error}");
    assert!(
        preflight.running().is_empty(),
        "the registration was cancelled: {:?}",
        preflight.running()
    );
    assert!(preflight.ledgers_balance());
    assert!(runner.programs().is_empty(), "and nothing was spawned");
}

#[test]
fn one_identity_cannot_be_registered_twice() {
    let runner = Recording::default();
    let adapters = Registry;
    let preflight = preflight(&runner, &adapters, &[]);
    let registering = preflight.registering();

    let request = RunnerRequest {
        command: CommandSpec::new("bash"),
        workspace: PathBuf::from("."),
        role: crate::runner::ExecutionRole::Probe(ProbeTarget::Shell),
        timeout: Duration::from_secs(1),
        agent: None,
        invocation: PreflightIdentities::shell(0).expect("the shell identity"),
        credential_profile: None,
    };
    registering.run_blocking(&request).expect("the first runs");
    let error = registering
        .run_blocking(&request)
        .expect_err("the second shares an identity with the first");
    assert!(error.to_string().contains("already registered"), "{error}");
}

#[test]
fn a_gate_invocation_through_the_boundary_takes_no_slot() {
    let runner = Recording::default();
    let adapters = Registry;
    let preflight = preflight(&runner, &adapters, &[]);
    let registering = preflight.registering();

    let gate = InvocationId::attempt(
        TaskKey(0),
        GenerationId(0),
        AttemptNumber(1),
        AttemptRole::Gate(0),
        0,
    );
    assert!(!is_slotted(&gate), "the rule itself");
    let request = RunnerRequest {
        command: CommandSpec::new("bash"),
        workspace: PathBuf::from("."),
        role: crate::runner::ExecutionRole::Gate,
        timeout: Duration::from_secs(1),
        agent: None,
        invocation: gate,
        credential_profile: None,
    };
    registering
        .run_blocking(&request)
        .expect("a gate needs no slot pair");
    assert!(preflight.ledgers_balance());
}

struct Composing {
    inner: Recording,
    composed: ComposedCommonDir,
}

impl Runner for Composing {
    fn run<'a>(&'a self, request: &'a RunnerRequest, call: RunnerCall<'a>) -> RunFuture<'a> {
        self.inner.run(request, call)
    }

    fn composed_common_dir(&self, request: &RunnerRequest) -> ComposedCommonDir {
        let _ = request;
        self.composed.clone()
    }
}

#[test]
fn the_boundary_forwards_its_runners_report_and_keeps_one_per_admitted_request() {
    let composed = ComposedCommonDir::Present(std::ffi::OsString::from("/the/runs/common"));
    let runner = Composing {
        inner: Recording::default(),
        composed: composed.clone(),
    };
    let adapters = Registry;
    let preflight = preflight(&runner, &adapters, &[]);
    let registering = preflight.registering();

    let gate = RunnerRequest {
        command: CommandSpec::new("bash"),
        workspace: PathBuf::from("."),
        role: crate::runner::ExecutionRole::Gate,
        timeout: Duration::from_secs(1),
        agent: None,
        invocation: InvocationId::attempt(
            TaskKey(0),
            GenerationId(0),
            AttemptNumber(1),
            AttemptRole::Gate(0),
            0,
        ),
        credential_profile: None,
    };
    assert_eq!(
        registering.composed_common_dir(&gate),
        composed,
        "the boundary reports what the runner it wraps composes"
    );
    registering
        .run_blocking(&gate)
        .expect("a gate needs no slot pair");
    let unbound = RunnerRequest {
        command: CommandSpec::new("claude"),
        workspace: PathBuf::from("."),
        role: crate::runner::ExecutionRole::Probe(ProbeTarget::Agent(AgentId::new(AGENT))),
        timeout: Duration::from_secs(1),
        agent: None,
        invocation: PreflightIdentities::agent(AGENT, 0).expect("a slotted probe identity"),
        credential_profile: None,
    };
    registering
        .run_blocking(&unbound)
        .expect_err("a slotted invocation with no agent binding is refused at admission");
    assert_eq!(
        registering.into_composed(),
        vec![composed],
        "one report for the admitted request, none for the one refused before its runner"
    );
    assert!(preflight.ledgers_balance());
    assert_eq!(
        runner.inner.programs(),
        vec!["bash".to_owned()],
        "only the admitted request reached the runner"
    );
}

#[test]
fn the_debug_rendering_names_the_boundary_and_not_the_ledgers() {
    let runner = Recording::default();
    let adapters = Registry;
    let preflight = preflight(&runner, &adapters, &[AGENT]);
    let rendered = format!("{preflight:?}");
    assert!(rendered.contains("RunPreflight"), "{rendered}");
    assert!(rendered.contains("Bash"), "{rendered}");
    assert!(rendered.contains(AGENT), "{rendered}");
    assert!(
        !rendered.contains("Mutex"),
        "the ledgers are not part of the rendering: {rendered}"
    );
}

const EXISTING_DIRECTORY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src");
const A_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");
const MISSING: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/no-such-credential-profile");
const TOKEN: &str = "sk-ant-oat01-fixture-token-value";

fn no_token(_key: &str) -> Option<OsString> {
    None
}

fn token_set(key: &str) -> Option<OsString> {
    (key == CLAUDE_CODE_OAUTH_TOKEN).then(|| OsString::from(TOKEN))
}

fn pool(name: &str, agent: &str, profile: Option<&str>) -> Pool {
    let mut pool = Pool::discovered(name, PoolKind::SubscriptionWindow, agent, Vec::new());
    pool.profile = profile.map(str::to_owned);
    pool
}

fn profiled_preflight<'a>(
    runner: &'a dyn Runner,
    adapters: &'a Registry,
    pools: &[Pool],
    environment: fn(&str) -> Option<OsString>,
) -> RunPreflight<'a> {
    preflight(runner, adapters, &[AGENT])
        .with_pools(pools)
        .reading_environment(environment)
}

const PROFILE_REFUSAL_CLOSING: &str = concat!(
    ". No agent CLI was started and no event was appended; ",
    "correct the pools file or the environment and run the same command again"
);

fn refused_before_anything_ran(
    runner: &Recording,
    preflight: &RunPreflight<'_>,
    policy: &RunnerPolicy,
) -> String {
    let error = preflight
        .certify(policy)
        .expect_err("the profile cannot be honoured, so the pre-flight refuses");
    let text = error.to_string();
    assert!(
        matches!(error, UpstrokeError::Refused { .. }),
        "a refusal, not some other failure: {text}"
    );
    assert!(
        text.starts_with("pre-flight: "),
        "the refusal shape: {text}"
    );
    assert!(
        text.ends_with(PROFILE_REFUSAL_CLOSING),
        "a profile refusal ends in its own closing sentence: {text}"
    );
    assert_eq!(
        text.matches(PROFILE_REFUSAL_CLOSING).count(),
        1,
        "and says it once: {text}"
    );
    assert!(
        !text.contains("resumable") && !text.contains("probed inside"),
        "and nothing of the probe refusal's closing, which is false before a run exists: {text}"
    );
    assert!(
        runner.programs().is_empty(),
        "nothing is spawned before a profile refusal, not even the shell probe: {:?}",
        runner.programs()
    );
    assert_eq!(preflight.settlements(), (0, 0), "nothing was registered");
    assert!(preflight.ledgers_balance());
    assert!(preflight.probed().is_none(), "nothing was certified");
    text
}

#[test]
fn a_profiled_pool_on_the_container_runner_refuses_before_anything_runs() {
    let runner = Recording::default();
    let adapters = Registry;
    let pools = [pool("max", AGENT, Some(EXISTING_DIRECTORY))];
    let preflight = profiled_preflight(&runner, &adapters, &pools, no_token);

    let text = refused_before_anything_ran(&runner, &preflight, &container_policy());

    assert!(text.contains("`max`"), "the refusal names the pool: {text}");
    assert!(
        text.contains("host runner only") && text.contains("container runner"),
        "the refusal says a profile needs the host runner: {text}"
    );
}

#[test]
fn a_profiled_pool_whose_agent_is_not_claude_code_refuses_naming_the_pool_and_agent() {
    let runner = Recording::default();
    let adapters = Registry;
    let pools = [
        pool("max", AGENT, Some(EXISTING_DIRECTORY)),
        pool("work", OTHER, Some(EXISTING_DIRECTORY)),
    ];
    let preflight = profiled_preflight(&runner, &adapters, &pools, no_token);

    let text = refused_before_anything_ran(&runner, &preflight, &host_policy());

    assert!(
        text.contains("`work`"),
        "the refusal names the pool: {text}"
    );
    assert!(
        text.contains(&format!("`{OTHER}`")),
        "and its agent: {text}"
    );
    assert!(
        text.contains("Claude Code's profile only"),
        "and says which agent's profile this stage supports: {text}"
    );
}

#[test]
fn a_profile_that_does_not_exist_refuses_naming_the_pool_and_the_path() {
    let runner = Recording::default();
    let adapters = Registry;
    let pools = [pool("max", AGENT, Some(MISSING))];
    let preflight = profiled_preflight(&runner, &adapters, &pools, no_token);

    let text = refused_before_anything_ran(&runner, &preflight, &host_policy());

    assert!(text.contains("`max`"), "the refusal names the pool: {text}");
    assert!(text.contains(MISSING), "and the path: {text}");
    assert!(text.contains("does not exist"), "{text}");
}

#[test]
fn a_profile_that_is_not_a_directory_refuses_naming_the_pool_and_the_path() {
    let runner = Recording::default();
    let adapters = Registry;
    let pools = [pool("max", AGENT, Some(A_FILE))];
    let preflight = profiled_preflight(&runner, &adapters, &pools, no_token);

    let text = refused_before_anything_ran(&runner, &preflight, &host_policy());

    assert!(text.contains("`max`"), "the refusal names the pool: {text}");
    assert!(text.contains(A_FILE), "and the path: {text}");
    assert!(text.contains("is not a directory"), "{text}");
}

#[test]
fn a_relative_profile_refuses_naming_the_pool_and_the_value() {
    for (name, relative) in [
        ("current", "."),
        ("nested", "profiles/personal"),
        ("home", "~/profiles/personal"),
    ] {
        let runner = Recording::default();
        let adapters = Registry;
        let pools = [pool(name, AGENT, Some(relative))];
        let preflight = profiled_preflight(&runner, &adapters, &pools, no_token);

        let text = refused_before_anything_ran(&runner, &preflight, &host_policy());

        assert!(
            text.contains(&format!(
                "pool `{name}` names the credential profile `{relative}`, which is a relative \
                 path"
            )),
            "the refusal names its pool and the value as written: {text}"
        );
        for said in [
            "each task runs in its own directory, so a relative path would name a different \
             directory for each process",
            "`~` is not expanded",
            "give the profile as an absolute path",
        ] {
            assert!(text.contains(said), "looked for {said:?} in: {text}");
        }
    }
}

#[test]
fn a_profile_that_cannot_be_inspected_refuses_with_the_error() {
    const UNINSPECTABLE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/nul\0byte");
    let runner = Recording::default();
    let adapters = Registry;
    let pools = [pool("max", AGENT, Some(UNINSPECTABLE))];
    let preflight = profiled_preflight(&runner, &adapters, &pools, no_token);

    let text = refused_before_anything_ran(&runner, &preflight, &host_policy());

    assert!(text.contains("`max`"), "the refusal names the pool: {text}");
    assert!(text.contains("which cannot be inspected: "), "{text}");
    assert!(
        !text.contains("does not exist"),
        "an error other than a missing path is not reported as one: {text}"
    );
}

#[test]
fn an_oauth_token_in_the_coordinators_environment_refuses_without_printing_it() {
    let runner = Recording::default();
    let adapters = Registry;
    let pools = [pool("max", AGENT, Some(EXISTING_DIRECTORY))];
    let preflight = profiled_preflight(&runner, &adapters, &pools, token_set);

    let text = refused_before_anything_ran(&runner, &preflight, &host_policy());

    assert!(
        text.contains(CLAUDE_CODE_OAUTH_TOKEN),
        "the refusal names the variable: {text}"
    );
    assert!(
        text.contains("`max`") && text.contains(EXISTING_DIRECTORY),
        "and the profile the run would not use: {text}"
    );
    assert!(
        !text.contains(TOKEN),
        "a refusal must never carry the credential itself: {text}"
    );
}

#[test]
fn a_profiled_claude_code_pool_on_the_host_with_its_directory_and_no_token_certifies() {
    let runner = Recording::default();
    let adapters = Registry;
    let pools = [pool("max", AGENT, Some(EXISTING_DIRECTORY))];
    let preflight = profiled_preflight(&runner, &adapters, &pools, no_token);

    preflight
        .certify(&host_policy())
        .expect("every profile check holds and every probe answers");

    assert_eq!(
        runner.programs(),
        vec!["bash".to_owned(), AGENT.to_owned()],
        "the checks spawn nothing of their own; the probes are the ones an unprofiled run makes"
    );
    assert_eq!(
        preflight.probed().map(|probed| probed.agents),
        Some(vec![AGENT.to_owned()])
    );
    assert!(preflight.ledgers_balance());
}

#[test]
fn the_claude_code_probe_of_a_profiled_pool_runs_under_that_profile() {
    let recording = Arc::new(Recording::default());
    let pools = [pool("max", AGENT, Some(EXISTING_DIRECTORY))];
    let runner = crate::engine::topology::profiled::profiled(
        Arc::clone(&recording) as Arc<dyn Runner>,
        &pools,
    );
    let adapters = Registry;
    let preflight = profiled_preflight(&*runner, &adapters, &pools, no_token);

    preflight
        .certify(&host_policy())
        .expect("the profiled pre-flight certifies");

    let requests = recording.requests();
    let profiles: Vec<(String, Option<PathBuf>)> = requests
        .iter()
        .map(|request| {
            (
                request.command.program.clone(),
                request.credential_profile.clone(),
            )
        })
        .collect();
    assert_eq!(
        profiles,
        vec![
            ("bash".to_owned(), None),
            (AGENT.to_owned(), Some(PathBuf::from(EXISTING_DIRECTORY))),
        ],
        "the agent probe goes through the run's profiled runner, as `entry` builds it, so the \
         Claude Code CLI is probed under the profile its pool names; the shell probe has no \
         agent and takes none"
    );
}

#[test]
fn pools_that_name_no_profile_leave_the_preflight_as_it_was() {
    let runner = Recording::default();
    let adapters = Registry;
    let pools = [pool("max", AGENT, None), pool("work", OTHER, None)];
    let preflight = profiled_preflight(&runner, &adapters, &pools, token_set);

    preflight
        .certify(&container_policy())
        .expect("with no profile named, none of the profile checks applies");

    assert_eq!(
        runner.programs(),
        vec!["bash".to_owned(), AGENT.to_owned()],
        "the same probes, on the container runner and with a token set"
    );
}

const SECOND_DIRECTORY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/agent");
const THIRD_DIRECTORY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/engine");

const CLAUDE_HELP: &str = concat!(
    "-p --output-format --model --settings --setting-sources --permission-mode\n",
    "  --effort <level> (low, medium, high, xhigh, max)\n"
);

const LOGGED_IN: &str = concat!(
    r#"{"loggedIn": true, "authMethod": "claude.ai", "apiProvider": "firstParty", "#,
    r#""subscriptionType": "max"}"#
);

const LOGGED_OUT: &str = concat!(
    r#"{"loggedIn": false, "authMethod": "none", "#,
    r#""apiProvider": "firstParty"}"#
);

fn answered(code: i32, stdout: &str) -> ProcessOutput {
    ProcessOutput {
        code: Some(code),
        stdout: stdout.to_owned(),
        stderr: String::new(),
        duration: Duration::from_millis(1),
        timed_out: false,
        output_limited: false,
    }
}

fn answers_signed_in(_profile: Option<&Path>) -> ProcessOutput {
    answered(0, LOGGED_IN)
}

fn answers_signed_out(_profile: Option<&Path>) -> ProcessOutput {
    answered(1, LOGGED_OUT)
}

fn answers_signed_in_under_the_first_profile_only(profile: Option<&Path>) -> ProcessOutput {
    if profile == Some(Path::new(EXISTING_DIRECTORY)) {
        answers_signed_in(profile)
    } else {
        answers_signed_out(profile)
    }
}

fn answers_signed_in_under_a_profile_only(profile: Option<&Path>) -> ProcessOutput {
    if profile.is_some() {
        answers_signed_in(profile)
    } else {
        answers_signed_out(profile)
    }
}

struct ClaudeCli {
    seen: Mutex<Vec<RunnerRequest>>,
    auth_status: fn(Option<&Path>) -> ProcessOutput,
}

impl ClaudeCli {
    fn answering(auth_status: fn(Option<&Path>) -> ProcessOutput) -> Self {
        Self {
            seen: Mutex::new(Vec::new()),
            auth_status,
        }
    }

    fn requests(&self) -> Vec<RunnerRequest> {
        self.seen
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn discoveries(&self) -> Vec<(String, Option<PathBuf>)> {
        self.requests()
            .into_iter()
            .filter(|request| request.command.args == ["auth", "status", "--json"])
            .map(|request| (request.invocation.render(), request.credential_profile))
            .collect()
    }
}

impl crate::runner::contract::tests::InlineRunner for ClaudeCli {
    fn run_inline(
        &self,
        request: &RunnerRequest,
    ) -> Result<ProcessOutput, crate::runner::RunnerError> {
        self.seen
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(request.clone());
        if request.command.program != "claude" {
            return Ok(answered(0, ""));
        }
        let args: Vec<&str> = request.command.args.iter().map(String::as_str).collect();
        Ok(match args.as_slice() {
            ["--version"] => answered(0, "2.1.226 (Claude Code)\n"),
            ["--help"] => answered(0, CLAUDE_HELP),
            ["auth", "status", "--json"] => {
                (self.auth_status)(request.credential_profile.as_deref())
            }
            _ => answered(2, ""),
        })
    }
}

fn claude_code_preflight<'a>(runner: &'a dyn Runner, pools: &[Pool]) -> RunPreflight<'a> {
    RunPreflight::new(
        runner,
        &crate::agent::BuiltinAdapters,
        ShellKind::Bash,
        Path::new("."),
        vec![AGENT.to_owned()],
    )
    .with_pools(pools)
    .reading_environment(no_token)
}

fn profiled_over(cli: &Arc<ClaudeCli>, pools: &[Pool]) -> Arc<dyn Runner> {
    crate::engine::topology::profiled::profiled(Arc::clone(cli) as Arc<dyn Runner>, pools)
}

fn two_profiled_pools() -> [Pool; 2] {
    [
        pool("max", AGENT, Some(EXISTING_DIRECTORY)),
        pool("spare", AGENT, Some(SECOND_DIRECTORY)),
    ]
}

const SIGN_IN_REFUSAL_CLOSING: &str = concat!(
    ". Nothing was spawned for the run and no event was appended; ",
    "run the same command again once that account is signed in ("
);

fn a_sign_in_refusal(error: &UpstrokeError) -> String {
    let text = error.to_string();
    assert!(
        matches!(error, UpstrokeError::Refused { .. }),
        "a refusal, not some other failure: {text}"
    );
    assert!(
        text.starts_with(&format!(
            "pre-flight: agent `{AGENT}` is not signed in under "
        )),
        "the refusal shape, naming the agent: {text}"
    );
    assert_eq!(
        text.matches(SIGN_IN_REFUSAL_CLOSING).count(),
        1,
        "a sign-in refusal ends in its own closing sentence, once: {text}"
    );
    assert!(
        text.ends_with(')') && !text.ends_with('.'),
        "and ends with how to sign in, with no full stop: {text}"
    );
    assert!(
        !text.contains("resumable") && !text.contains("probed inside"),
        "with nothing of the probe refusal's closing, which is false on a fresh run: {text}"
    );
    text
}

#[test]
fn a_second_profiled_pool_that_is_signed_out_refuses_naming_that_pool_and_its_profile() {
    let cli = Arc::new(ClaudeCli::answering(
        answers_signed_in_under_the_first_profile_only,
    ));
    let pools = two_profiled_pools();
    let runner = profiled_over(&cli, &pools);
    let preflight = claude_code_preflight(&*runner, &pools);

    let error = preflight
        .certify(&host_policy())
        .expect_err("the second pool's account is signed out, so the pre-flight refuses");

    let text = a_sign_in_refusal(&error);
    assert!(
        text.contains(&format!(
            "pool `spare`'s account, the credential profile directory `{SECOND_DIRECTORY}`"
        )),
        "the refusal names the signed-out pool and its profile: {text}"
    );
    assert!(
        text.contains(&format!(
            "`claude auth login` with `CLAUDE_CONFIG_DIR` set to `{SECOND_DIRECTORY}`"
        )),
        "and says how to sign that profile in: {text}"
    );
    assert!(
        !text.contains("`max`") && !text.contains(&format!("`{EXISTING_DIRECTORY}`")),
        "the signed-in pool is not the one named: {text}"
    );
    assert_eq!(
        cli.discoveries(),
        vec![
            (
                "p.agent-claude-code.o2".to_owned(),
                Some(PathBuf::from(EXISTING_DIRECTORY))
            ),
            (
                "p.agent-claude-code.o18".to_owned(),
                Some(PathBuf::from(SECOND_DIRECTORY))
            ),
        ],
        "the first pool's discovery keeps the adapter's identity, the second's has its own, \
         and each runs under its own pool's profile"
    );
    assert!(preflight.probed().is_none(), "nothing was certified");
    assert!(preflight.ledgers_balance(), "{:?}", preflight.running());
}

#[test]
fn every_profiled_pool_signed_in_certifies_and_every_discovery_is_registered_and_settled() {
    let cli = Arc::new(ClaudeCli::answering(answers_signed_in));
    let pools = [
        pool("max", AGENT, Some(EXISTING_DIRECTORY)),
        pool("spare", AGENT, Some(SECOND_DIRECTORY)),
        pool("third", AGENT, Some(THIRD_DIRECTORY)),
    ];
    let runner = profiled_over(&cli, &pools);
    let preflight = claude_code_preflight(&*runner, &pools);

    preflight
        .certify(&host_policy())
        .expect("every account the run would use is signed in");

    let identities: Vec<String> = cli
        .requests()
        .iter()
        .map(|request| request.invocation.render())
        .collect();
    assert_eq!(
        identities,
        [
            "p.shell.o0",
            "p.agent-claude-code.o0",
            "p.agent-claude-code.o1",
            "p.agent-claude-code.o2",
            "p.agent-claude-code.o18",
            "p.agent-claude-code.o34",
        ],
        "the shell, the two capability probes, then one discovery per profiled pool, the two \
         further pools' at identities of their own"
    );
    assert_eq!(
        cli.discoveries()
            .into_iter()
            .map(|(_, profile)| profile)
            .collect::<Vec<_>>(),
        [EXISTING_DIRECTORY, SECOND_DIRECTORY, THIRD_DIRECTORY]
            .map(|profile| Some(PathBuf::from(profile)))
            .to_vec(),
        "each pool's discovery under its own profile, in file order"
    );
    assert_eq!(
        preflight.settlements(),
        (6, 0),
        "six processes registered and each completed once"
    );
    assert!(preflight.running().is_empty(), "{:?}", preflight.running());
    assert!(preflight.ledgers_balance());
    assert_eq!(
        preflight.probed().map(|probed| probed.agents),
        Some(vec![AGENT.to_owned()])
    );
}

#[test]
fn an_unprofiled_agent_whose_default_account_is_signed_out_refuses_naming_that_account() {
    let cli = ClaudeCli::answering(answers_signed_out);
    let pools = [pool("max", AGENT, None)];
    let preflight = claude_code_preflight(&cli, &pools);

    let error = preflight
        .certify(&host_policy())
        .expect_err("the default account is signed out, so the pre-flight refuses");

    let text = a_sign_in_refusal(&error);
    assert!(
        text.contains(
            "under pool `max`'s account, as `max` names no credential profile, so it runs under \
             the agent's default account"
        ),
        "the refusal names the agent's default account: {text}"
    );
    assert!(
        text.ends_with("(`claude auth login`)") && !text.contains("CLAUDE_CONFIG_DIR"),
        "and how to sign it in, with no profile: {text}"
    );
    assert_eq!(
        cli.discoveries(),
        vec![("p.agent-claude-code.o2".to_owned(), None)],
        "one discovery, under no profile"
    );
    assert!(preflight.ledgers_balance());
}

#[test]
fn a_discovery_that_cannot_say_whether_the_account_is_signed_in_certifies() {
    fn no_json(_profile: Option<&Path>) -> ProcessOutput {
        answered(1, "error: unknown command 'auth'")
    }
    fn timed_out(_profile: Option<&Path>) -> ProcessOutput {
        ProcessOutput {
            code: None,
            timed_out: true,
            ..answered(0, "")
        }
    }

    for (label, auth_status) in [
        ("no JSON", no_json as fn(Option<&Path>) -> ProcessOutput),
        ("a timeout", timed_out),
    ] {
        for pools in [Vec::new(), two_profiled_pools().to_vec()] {
            let cli = Arc::new(ClaudeCli::answering(auth_status));
            let runner = profiled_over(&cli, &pools);
            let preflight = claude_code_preflight(&*runner, &pools);

            preflight.certify(&host_policy()).unwrap_or_else(|error| {
                panic!("{label} is an unknown sign-in, which passes: {error}")
            });
            assert_eq!(
                cli.discoveries().len(),
                pools.len().max(1),
                "{label}: one discovery per profiled pool, or one under the default account"
            );
            assert!(preflight.ledgers_balance(), "{label}");
        }
    }
}

#[test]
fn a_fresh_runs_probes_refuse_a_signed_out_pool_in_the_resumes_words() {
    use crate::engine::topology::create::Probes as _;

    let pools = two_profiled_pools();
    let cli = Arc::new(ClaudeCli::answering(
        answers_signed_in_under_the_first_profile_only,
    ));
    let runner = profiled_over(&cli, &pools);
    let ledger = Mutex::new(InvocationLedger::new());
    let pair = crate::engine::topology::create::ProbePair::grant(&ledger);
    let probes = crate::engine::topology::create::RunnerProbes::new(
        ShellKind::Bash,
        PathBuf::from("."),
        &crate::agent::BuiltinAdapters,
        String::new(),
    )
    .with_pools(&pools);

    let on_fresh = probes
        .agent(AGENT, &pair.agent_probe(&*runner))
        .expect_err("the second pool's account is signed out, so the fresh run's probe refuses");

    let text = a_sign_in_refusal(&on_fresh);
    assert!(text.contains("pool `spare`"), "{text}");
    assert!(
        probes.probed_caps().is_empty(),
        "a refused agent's capabilities are not recorded"
    );
    assert_eq!(
        cli.discoveries()
            .into_iter()
            .map(|(identity, _)| identity)
            .collect::<Vec<_>>(),
        ["p.agent-claude-code.o2", "p.agent-claude-code.o18"]
    );
    assert!(pair.balances(), "every process the fresh probe ran settled");

    let resumed_cli = Arc::new(ClaudeCli::answering(
        answers_signed_in_under_the_first_profile_only,
    ));
    let resumed_runner = profiled_over(&resumed_cli, &pools);
    let on_resume = claude_code_preflight(&*resumed_runner, &pools)
        .certify(&host_policy())
        .expect_err("a resume refuses the same pool");
    assert_eq!(
        text,
        on_resume.to_string(),
        "one refusal, worded once, on both paths"
    );
}

#[test]
fn a_profiled_pool_on_the_container_runner_is_refused_before_any_discovery() {
    let cli = Arc::new(ClaudeCli::answering(answers_signed_in));
    let pools = two_profiled_pools();
    let runner = profiled_over(&cli, &pools);
    let preflight = claude_code_preflight(&*runner, &pools);

    let error = preflight
        .certify(&container_policy())
        .expect_err("a profile on the container runner is refused");

    let text = error.to_string();
    assert!(
        text.ends_with(PROFILE_REFUSAL_CLOSING) && text.contains("host runner only"),
        "the profile refusal, as before: {text}"
    );
    assert!(
        cli.discoveries().is_empty(),
        "no discovery ran: {:?}",
        cli.discoveries()
    );
    assert!(
        cli.requests().is_empty(),
        "nor anything else: {:?}",
        cli.requests()
    );
    assert_eq!(preflight.settlements(), (0, 0), "nothing was registered");
}

fn a_default_pool_then_a_profiled_spare() -> [Pool; 2] {
    [
        pool("max", AGENT, None),
        pool("spare", AGENT, Some(SECOND_DIRECTORY)),
    ]
}

const MAX_DEFAULT_ACCOUNT: &str = concat!(
    "pool `max`'s account, as `max` names no credential profile, ",
    "so it runs under the agent's default account"
);

#[test]
fn lists_each_account_an_agents_pools_use_once_in_file_order() {
    let pools = [
        pool("max", AGENT, None),
        pool("work", OTHER, Some(THIRD_DIRECTORY)),
        pool("spare", AGENT, Some(SECOND_DIRECTORY)),
        pool("again", AGENT, None),
        pool("twice", AGENT, Some(SECOND_DIRECTORY)),
    ];

    assert_eq!(
        Account::of_agent(&pools, AGENT),
        vec![
            Account::Default {
                pool: Some("max".to_owned())
            },
            Account::Profile {
                pool: "spare".to_owned(),
                profile: PathBuf::from(SECOND_DIRECTORY)
            },
        ],
        "the default account, named by the first pool that uses it, then the profile, named by \
         the first pool that names it; another agent's pool is not one of them"
    );
    assert_eq!(
        Account::of_agent(&pools, "nobody-pools-this"),
        vec![Account::Default { pool: None }],
        "an agent with no pool in the run uses its default account alone"
    );
}

#[test]
fn refuses_a_first_pools_signed_out_default_account_though_its_profiled_spare_is_signed_in() {
    let cli = Arc::new(ClaudeCli::answering(answers_signed_in_under_a_profile_only));
    let pools = a_default_pool_then_a_profiled_spare();
    let runner = profiled_over(&cli, &pools);
    let preflight = claude_code_preflight(&*runner, &pools);

    let error = preflight
        .certify(&host_policy())
        .expect_err("the first pool's default account is signed out, so the pre-flight refuses");

    let text = a_sign_in_refusal(&error);
    assert!(
        text.contains(&format!("is not signed in under {MAX_DEFAULT_ACCOUNT}; ")),
        "the refusal names the first pool and the agent's default account: {text}"
    );
    assert!(
        text.ends_with("(`claude auth login`)") && !text.contains("CLAUDE_CONFIG_DIR"),
        "and how to sign the default account in, with no profile: {text}"
    );
    assert!(
        !text.contains("`spare`") && !text.contains(SECOND_DIRECTORY),
        "the signed-in spare is not the one named: {text}"
    );
    assert_eq!(
        cli.discoveries(),
        vec![
            ("p.agent-claude-code.o2".to_owned(), None),
            (
                "p.agent-claude-code.o18".to_owned(),
                Some(PathBuf::from(SECOND_DIRECTORY))
            ),
        ],
        "the default account's discovery reaches the CLI under no profile, at the adapter's own \
         identity, then the spare's under its profile, at its own"
    );
    assert!(preflight.probed().is_none(), "nothing was certified");
    assert!(preflight.ledgers_balance(), "{:?}", preflight.running());
}

#[test]
fn certifies_a_signed_in_default_account_and_profiled_spare_settling_every_discovery() {
    let cli = Arc::new(ClaudeCli::answering(answers_signed_in));
    let pools = a_default_pool_then_a_profiled_spare();
    let runner = profiled_over(&cli, &pools);
    let preflight = claude_code_preflight(&*runner, &pools);

    preflight
        .certify(&host_policy())
        .expect("both accounts the run would use are signed in");

    let identities: Vec<String> = cli
        .requests()
        .iter()
        .map(|request| request.invocation.render())
        .collect();
    assert_eq!(
        identities,
        [
            "p.shell.o0",
            "p.agent-claude-code.o0",
            "p.agent-claude-code.o1",
            "p.agent-claude-code.o2",
            "p.agent-claude-code.o18",
        ],
        "the shell, the two capability probes, then the default account's discovery and the \
         spare's, at an identity of its own"
    );
    assert_eq!(
        cli.discoveries()
            .into_iter()
            .map(|(_, profile)| profile)
            .collect::<Vec<_>>(),
        vec![None, Some(PathBuf::from(SECOND_DIRECTORY))],
        "the default account's discovery under no profile, then the spare's under its own"
    );
    assert_eq!(
        preflight.settlements(),
        (5, 0),
        "five processes registered and each completed once"
    );
    assert!(preflight.running().is_empty(), "{:?}", preflight.running());
    assert!(preflight.ledgers_balance());
    assert_eq!(
        preflight.probed().map(|probed| probed.agents),
        Some(vec![AGENT.to_owned()])
    );
}

#[test]
fn refuses_a_signed_out_default_account_on_a_fresh_run_in_the_resumes_words() {
    use crate::engine::topology::create::Probes as _;

    let pools = a_default_pool_then_a_profiled_spare();
    let cli = Arc::new(ClaudeCli::answering(answers_signed_in_under_a_profile_only));
    let runner = profiled_over(&cli, &pools);
    let ledger = Mutex::new(InvocationLedger::new());
    let pair = crate::engine::topology::create::ProbePair::grant(&ledger);
    let probes = crate::engine::topology::create::RunnerProbes::new(
        ShellKind::Bash,
        PathBuf::from("."),
        &crate::agent::BuiltinAdapters,
        String::new(),
    )
    .with_pools(&pools)
    .reading_environment(no_token);

    let on_fresh = probes
        .agent(AGENT, &pair.agent_probe(&*runner))
        .expect_err("the first pool's default account is signed out, so the fresh probe refuses");

    let text = a_sign_in_refusal(&on_fresh);
    assert!(text.contains(MAX_DEFAULT_ACCOUNT), "{text}");
    assert!(
        probes.probed_caps().is_empty(),
        "a refused agent's capabilities are not recorded"
    );
    assert_eq!(
        cli.discoveries(),
        vec![
            ("p.agent-claude-code.o2".to_owned(), None),
            (
                "p.agent-claude-code.o18".to_owned(),
                Some(PathBuf::from(SECOND_DIRECTORY))
            ),
        ]
    );
    assert!(pair.balances(), "every process the fresh probe ran settled");

    let resumed_cli = Arc::new(ClaudeCli::answering(answers_signed_in_under_a_profile_only));
    let resumed_runner = profiled_over(&resumed_cli, &pools);
    let on_resume = claude_code_preflight(&*resumed_runner, &pools)
        .certify(&host_policy())
        .expect_err("a resume refuses the same account");
    assert_eq!(
        text,
        on_resume.to_string(),
        "one refusal, worded once, on both paths"
    );
}

#[test]
fn refuses_a_later_unprofiled_pools_signed_out_default_account_discovered_under_no_profile() {
    let cli = Arc::new(ClaudeCli::answering(answers_signed_in_under_a_profile_only));
    let pools = [
        pool("max", AGENT, Some(EXISTING_DIRECTORY)),
        pool("spare", AGENT, None),
    ];
    let runner = profiled_over(&cli, &pools);
    let preflight = claude_code_preflight(&*runner, &pools);

    let error = preflight
        .certify(&host_policy())
        .expect_err("the later pool's default account is signed out, so the pre-flight refuses");

    let text = a_sign_in_refusal(&error);
    assert!(
        text.contains(
            "is not signed in under pool `spare`'s account, as `spare` names no credential \
             profile, so it runs under the agent's default account; "
        ),
        "the refusal names the later pool and the agent's default account: {text}"
    );
    assert!(
        text.ends_with("(`claude auth login`)") && !text.contains("CLAUDE_CONFIG_DIR"),
        "and how to sign the default account in, with no profile: {text}"
    );
    assert!(
        !text.contains("`max`") && !text.contains(EXISTING_DIRECTORY),
        "the signed-in profile is not the one named: {text}"
    );
    assert_eq!(
        cli.discoveries(),
        vec![
            (
                "p.agent-claude-code.o2".to_owned(),
                Some(PathBuf::from(EXISTING_DIRECTORY))
            ),
            ("p.agent-claude-code.o18".to_owned(), None),
        ],
        "the later pool's discovery reaches the CLI with no profile, though `profiled` stamps \
         the agent's first pool's profile on a probe that carries none"
    );
    assert!(preflight.ledgers_balance(), "{:?}", preflight.running());
}

#[test]
fn discovers_each_account_once_however_many_pools_use_it() {
    for (label, pools, discovered) in [
        (
            "two unprofiled pools",
            [pool("max", AGENT, None), pool("spare", AGENT, None)],
            None,
        ),
        (
            "two pools naming one profile",
            [
                pool("max", AGENT, Some(EXISTING_DIRECTORY)),
                pool("spare", AGENT, Some(EXISTING_DIRECTORY)),
            ],
            Some(PathBuf::from(EXISTING_DIRECTORY)),
        ),
    ] {
        let cli = Arc::new(ClaudeCli::answering(answers_signed_in));
        let runner = profiled_over(&cli, &pools);
        let preflight = claude_code_preflight(&*runner, &pools);

        preflight
            .certify(&host_policy())
            .unwrap_or_else(|error| panic!("{label}: the one account is signed in: {error}"));
        assert_eq!(
            cli.discoveries(),
            vec![("p.agent-claude-code.o2".to_owned(), discovered)],
            "{label}: one account, so one discovery"
        );
        assert!(preflight.ledgers_balance(), "{label}");

        let signed_out_cli = Arc::new(ClaudeCli::answering(answers_signed_out));
        let signed_out_runner = profiled_over(&signed_out_cli, &pools);
        let error = claude_code_preflight(&*signed_out_runner, &pools)
            .certify(&host_policy())
            .expect_err("the one account is signed out");
        let text = a_sign_in_refusal(&error);
        assert!(
            text.contains("under pool `max`'s account") && !text.contains("`spare`"),
            "{label}: the first pool that uses the account names it: {text}"
        );
        assert_eq!(
            signed_out_cli.discoveries().len(),
            1,
            "{label}: one discovery, signed out or not"
        );
    }
}

const CODEX: &str = crate::agent::codex::ADAPTER_ID;

const PLACEHOLDER: &str = "placeholder-credential-value";

#[derive(Debug)]
struct SignedOutCodex;

impl AgentAdapter for SignedOutCodex {
    fn id(&self) -> &'static str {
        CODEX
    }

    fn probe(&self, runner: &dyn Runner) -> Result<Caps, UpstrokeError> {
        Stub(CODEX).probe(runner)
    }

    fn build(&self, run: &crate::agent::TaskRun) -> Result<CommandSpec, UpstrokeError> {
        Stub(CODEX).build(run)
    }

    fn parse(&self, out: &ProcessOutput) -> Result<Outcome, UpstrokeError> {
        Stub(CODEX).parse(out)
    }

    fn discover(
        &self,
        _runner: &dyn Runner,
        _caps: &Caps,
    ) -> Result<crate::agent::Discovery, UpstrokeError> {
        Ok(crate::agent::Discovery {
            auth: AuthState::NotAuthenticated,
            ..crate::agent::Discovery::unknown()
        })
    }
}

struct SignedOutCodexAdapters;

impl AdapterSource for SignedOutCodexAdapters {
    fn get(&self, id: &str) -> Option<&dyn AgentAdapter> {
        (id == CODEX).then_some(&SignedOutCodex as &dyn AgentAdapter)
    }
}

fn codex_preflight(
    runner: &dyn Runner,
    environment: fn(&str) -> Option<OsString>,
) -> RunPreflight<'_> {
    RunPreflight::new(
        runner,
        &SignedOutCodexAdapters,
        ShellKind::Bash,
        Path::new("."),
        vec![CODEX.to_owned()],
    )
    .reading_environment(environment)
}

fn codex_api_key_set(key: &str) -> Option<OsString> {
    (key == "CODEX_API_KEY").then(|| OsString::from(PLACEHOLDER))
}

fn openai_api_key_set(key: &str) -> Option<OsString> {
    (key == "OPENAI_API_KEY").then(|| OsString::from(PLACEHOLDER))
}

fn codex_access_token_set(key: &str) -> Option<OsString> {
    (key == "CODEX_ACCESS_TOKEN").then(|| OsString::from(PLACEHOLDER))
}

fn codex_variables_empty_and_others_set(key: &str) -> Option<OsString> {
    if ["CODEX_API_KEY", "OPENAI_API_KEY", "CODEX_ACCESS_TOKEN"].contains(&key) {
        Some(OsString::new())
    } else {
        Some(OsString::from(PLACEHOLDER))
    }
}

#[test]
fn certifies_a_signed_out_codex_account_while_one_of_its_environment_credentials_is_set() {
    for (variable, environment) in [
        (
            "CODEX_API_KEY",
            codex_api_key_set as fn(&str) -> Option<OsString>,
        ),
        ("OPENAI_API_KEY", openai_api_key_set),
        ("CODEX_ACCESS_TOKEN", codex_access_token_set),
    ] {
        let runner = Recording::default();
        let preflight = codex_preflight(&runner, environment);

        preflight.certify(&host_policy()).unwrap_or_else(|error| {
            panic!(
                "`{variable}` is set, so Codex authenticates from the environment though its \
                 discovery reports no stored login: {error}"
            )
        });
        assert_eq!(
            preflight.probed().map(|probed| probed.agents),
            Some(vec![CODEX.to_owned()]),
            "{variable}"
        );
        assert!(preflight.ledgers_balance(), "{variable}");
    }
}

#[test]
fn refuses_a_signed_out_codex_account_with_no_environment_credential_naming_never_valuing_them() {
    for (label, environment) in [
        ("nothing set", no_token as fn(&str) -> Option<OsString>),
        (
            "its variables set empty, others set",
            codex_variables_empty_and_others_set,
        ),
    ] {
        let runner = Recording::default();
        let preflight = codex_preflight(&runner, environment);

        let error = preflight
            .certify(&host_policy())
            .expect_err("no environment credential is set, so the signed-out account refuses");

        let text = error.to_string();
        assert!(
            matches!(error, UpstrokeError::Refused { .. }),
            "{label}: a refusal, not some other failure: {text}"
        );
        assert!(
            text.starts_with(
                "pre-flight: agent `codex` is not signed in under its default account, as no \
                 pool of this run names it; "
            ),
            "{label}: the refusal names the agent and its default account: {text}"
        );
        assert_eq!(
            text.matches(SIGN_IN_REFUSAL_CLOSING).count(),
            1,
            "{label}: the sign-in closing, once: {text}"
        );
        assert!(
            text.ends_with(
                "(with the agent's own CLI, or by setting one of `CODEX_API_KEY`, \
                 `OPENAI_API_KEY` or `CODEX_ACCESS_TOKEN` in the environment upstroke runs in)"
            ),
            "{label}: how to sign in names the three variables: {text}"
        );
        assert!(
            !text.contains(PLACEHOLDER),
            "{label}: a refusal never carries a variable's value: {text}"
        );
        assert!(
            preflight.probed().is_none(),
            "{label}: nothing was certified"
        );
        assert!(preflight.ledgers_balance(), "{label}");
    }
}

#[test]
fn refuses_a_signed_out_claude_code_account_whatever_codex_variable_is_set() {
    let cli = ClaudeCli::answering(answers_signed_out);
    let preflight = claude_code_preflight(&cli, &[]).reading_environment(codex_api_key_set);

    let error = preflight
        .certify(&host_policy())
        .expect_err("Codex's variables are Codex's, so Claude Code's signed-out account refuses");

    let text = a_sign_in_refusal(&error);
    assert!(
        text.contains("under its default account, as no pool of this run names it; "),
        "with no pool for the agent, the refusal names its default account: {text}"
    );
    assert!(
        text.ends_with("(`claude auth login`)"),
        "and how to sign it in: {text}"
    );
    assert!(
        !text.contains("CODEX_API_KEY") && !text.contains(PLACEHOLDER),
        "and nothing of another agent's variables: {text}"
    );
    assert!(preflight.ledgers_balance());
}

#[test]
fn the_refusal_notes_keep_a_probe_whose_process_is_unresolved() {
    const NOTES: &str = include_str!("../../../../docs/internals/engine/topology/preflight.md");
    const HEADING: &str = "`impl RunnerPreflight for RunPreflight<'_>` › `fn certify(&self, \
                           policy: &RunnerPolicy) -> Result<(), UpstrokeError> {`";

    let certify = NOTES
        .split("\n## ")
        .find(|section| section.starts_with(HEADING))
        .map(|section| section.split_whitespace().collect::<Vec<_>>().join(" "))
        .unwrap_or_else(|| panic!("the notes carry no {HEADING:?} heading"));
    for pin in [
        "a probe whose process the Runner could not establish as ended (`ProcessFate::Unresolved`) \
         is not settled",
        "keeps its pair until this process exits",
        "the ledger does not balance",
    ] {
        assert!(
            certify.contains(pin),
            "the refusal must state that an unresolved probe keeps its registration; looked for \
             {pin:?} in:\n{certify}"
        );
    }
    assert!(
        !certify.contains(
            "is cancelled and every slot pair released, so the ledger balances on both paths"
        ),
        "the retired promise that every refusal releases and balances must not come back — \
         `InvocationLedger::end` keeps an unresolved invocation:\n{certify}"
    );
}
