//! Extended notes: `docs/internals/runner/container/exec/tests.md`

// Allowlist placement: the funnel section of `effects/allowlist.toml`, which
// carries this module's review clause. `effect_site_inventory.mechanism` (2).
#![allow(clippy::disallowed_methods)]
#![forbid(clippy::disallowed_types, clippy::disallowed_macros)]

use crate::runner::container::runtime::QuestionBound;
use std::collections::BTreeSet;
use std::sync::Arc;

use super::*;
use crate::gates::ShellKind;
use crate::rundir::RunPaths;
use crate::rundir::scratch_tree::ScratchTree;
use crate::runner::container::intent::LABEL_PRIVATE_ROOT;
use crate::runner::container::resolve::fixtures;
use crate::runner::container::runtime::{
    ContainerExecution, ContainerTrace, CreatedContainer, DiscoveredContainer, ImageInspection,
    Liveness, RuntimeOp, Settled, StopMode,
};
use crate::runner::container::view::fixtures as repo;
use crate::runner::container::{
    DOCKER_GATED_TESTS, FakeRuntime, GitView, RecordingHooks, docker_gate, list_intents,
};
use crate::runner::host::{self, HostEnvironment};
use crate::runner::invocation::AttemptRole;
use crate::runner::{
    AgentId, InvocationId, ProbeTarget, RunnerRequest, gate_request, review_request, worker_request,
};
use crate::topology::events::{AttemptNumber, GenerationId, ImageIdentity};
use crate::topology::registry::TaskKey;

const RUN_ID: &str = "01KZRN48A4ZK3AEDST3RJ8HMA4";
fn repo_key() -> &'static str {
    crate::runner::container::fake::slot_repo_key()
}

#[test]
fn a_container_name_is_scoped_to_its_build_slot() {
    use crate::runner::container::fake::scoped_repo_key;

    let slot2 = scoped_repo_key("/mnt/ramtarget/slot2");
    let slot3 = scoped_repo_key("/mnt/ramtarget/slot3");

    assert_ne!(
        slot2, slot3,
        "two build slots share a repository key, so their container names \
         collide and each run's pre-clean kills the other's container"
    );
    assert_eq!(
        slot2,
        scoped_repo_key("/mnt/ramtarget/slot2"),
        "the key is not stable for one slot, so a pre-clean cannot match the \
         residue its own previous run left — the only thing it is for"
    );
    for (scope, key) in [
        ("/mnt/ramtarget/slot2", &slot2),
        ("/mnt/ramtarget/slot3", &slot3),
        ("", &scoped_repo_key("")),
    ] {
        assert!(
            key.len() == 16 && key.chars().all(|c| c.is_ascii_hexdigit()),
            "`{scope}` derives `{key}`, which is not the sixteen hex \
             characters `workspace_manager::REPO_KEY_HEX_CHARS` requires"
        );
    }
    assert_eq!(
        repo_key(),
        scoped_repo_key(&std::env::var("CARGO_TARGET_DIR").unwrap_or_default()),
        "the cached key is not this process's own scope's"
    );
}
const INCARNATION_1: &str = "01KZTAAAAAAAAAAAAAAAAAAAAA";
const INCARNATION_2: &str = "01KZTBBBBBBBBBBBBBBBBBBBBB";
const IMAGE_ID: &str = "sha256:1111111111111111111111111111111111111111111111111111111111111111";
const OTHER_IMAGE_ID: &str =
    "sha256:2222222222222222222222222222222222222222222222222222222222222222";
const IMAGE_REFERENCE: &str = "ghcr.io/example/upstroke-runner:v1";
const MANIFEST_DIGEST: &str =
    "sha256:3333333333333333333333333333333333333333333333333333333333333333";
const VOLUMES: &[(&str, &str)] = &[
    ("claude-code", "upstroke-creds-claude"),
    ("copilot", "upstroke-creds-copilot"),
    ("codex", "upstroke-creds-codex"),
];
const EVENT_LOG_MARKER: &str = "COORDINATOR-EVENT-LOG-a5f2";
fn removal_in_progress_diagnostic(name: &str) -> String {
    format!("Error response from daemon: removal of container {name} is already in progress")
}

const IMAGE_PATH: &str = "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin";

const CWD_RELATIVE_PATH: &str = "/usr/local/bin:.:/usr/bin";

fn image_environment() -> ContainerEnvironment {
    ContainerEnvironment::from_image(
        [
            ("PATH", IMAGE_PATH),
            ("HOME", "/root"),
            ("UPSTROKE_IMAGE_MARKER", IMAGE_MARKER_VALUE),
        ]
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .collect(),
    )
}

fn container_policy() -> RunnerPolicy {
    RunnerPolicy {
        kind: RunnerKind::Container,
        policy: RunnerContract::ContainerV1,
        image: Some(ImageIdentity {
            reference: IMAGE_REFERENCE.to_owned(),
            id: IMAGE_ID.to_owned(),
            digest: Some(MANIFEST_DIGEST.to_owned()),
        }),
        credential_volumes: Some(
            VOLUMES
                .iter()
                .map(|(agent, volume)| ((*agent).to_owned(), (*volume).to_owned()))
                .collect(),
        ),
        mounts: Vec::new(),
    }
}

#[derive(Debug)]
struct Scripted {
    fake: FakeRuntime,
    exit_on_start: bool,
    execution: Mutex<Option<ContainerExecution>>,
    /// Set by [`Runtime::leaves_its_start_unresolved`].
    start_unresolved: std::sync::atomic::AtomicBool,
    observation_unresolved: std::sync::atomic::AtomicBool,
    volume_inspection_unresolved: std::sync::atomic::AtomicBool,
}

#[derive(Debug, Clone)]
struct Runtime(Arc<Scripted>);

impl Runtime {
    fn new(trace: ContainerTrace, exit_on_start: bool) -> Self {
        let fake = FakeRuntime::new(trace);
        fake.add_image(IMAGE_ID, Some(MANIFEST_DIGEST));
        fake.add_image(OTHER_IMAGE_ID, None);
        fake.tag(IMAGE_REFERENCE, IMAGE_ID);
        for (_, volume) in VOLUMES {
            fake.add_volume(volume);
        }
        Self(Arc::new(Scripted {
            fake,
            exit_on_start,
            execution: Mutex::new(None),
            start_unresolved: std::sync::atomic::AtomicBool::new(false),
            observation_unresolved: std::sync::atomic::AtomicBool::new(false),
            volume_inspection_unresolved: std::sync::atomic::AtomicBool::new(false),
        }))
    }

    fn leaves_its_observation_unresolved(&self) {
        let flag = &self.0.observation_unresolved;
        flag.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    fn leaves_its_volume_inspections_unresolved(&self) {
        let flag = &self.0.volume_inspection_unresolved;
        flag.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// Every later start runs and then reports a process it could not establish gone, as a
    /// bounded runtime question whose supervisor could not reap its process does.
    fn leaves_its_start_unresolved(&self) {
        let flag = &self.0.start_unresolved;
        flag.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    fn fake(&self) -> &FakeRuntime {
        &self.0.fake
    }

    fn scripts(&self, execution: ContainerExecution) -> Self {
        *self
            .0
            .execution
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(execution);
        self.clone()
    }
}

impl ContainerRuntime for Runtime {
    fn probe(&self) -> Result<(), RuntimeError> {
        self.0.fake.probe()
    }
    fn image_by_reference(&self, reference: &str) -> Result<Option<ImageInspection>, RuntimeError> {
        self.0.fake.image_by_reference(reference)
    }
    fn image_by_id(&self, id: &str) -> Result<Option<ImageInspection>, RuntimeError> {
        self.0.fake.image_by_id(id)
    }
    fn volume_present(&self, name: &str) -> Result<bool, RuntimeError> {
        let asked = self.0.fake.volume_present(name);
        let flag = &self.0.volume_inspection_unresolved;
        if flag.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(RuntimeError::Unresolved {
                operation: RuntimeOp::InspectVolume,
                detail: format!(
                    "a question about the credential volume `{name}` ended its supervision \
                     without establishing its process gone"
                ),
            });
        }
        asked
    }
    fn containers_with_label(
        &self,
        key: &str,
        value: &str,
    ) -> Result<Vec<DiscoveredContainer>, RuntimeError> {
        self.0.fake.containers_with_label(key, value)
    }
    fn observe(&self, name: &str, bound: QuestionBound<'_>) -> Result<Liveness, RuntimeError> {
        let flag = &self.0.observation_unresolved;
        if flag.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(RuntimeError::Unresolved {
                operation: RuntimeOp::Observe,
                detail: format!(
                    "a question about `{name}` ended its supervision without establishing its \
                     process gone"
                ),
            });
        }
        self.0.fake.observe(name, bound)
    }
    fn collect(&self, name: &str) -> Result<ContainerExecution, RuntimeError> {
        self.0.fake.collect(name)
    }
    fn create(&self, covered: CoveredCreate<'_>) -> Result<CreatedContainer, RuntimeError> {
        self.0.fake.create(covered)
    }
    fn start(&self, covered: CoveredStart<'_>) -> Result<(), RuntimeError> {
        let name = covered.name();
        self.0.fake.start(covered)?;
        let flag = &self.0.start_unresolved;
        if flag.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(RuntimeError::Unresolved {
                operation: RuntimeOp::Start,
                detail: format!(
                    "a question the start of `{name}` asked ended its supervision without \
                     establishing its process gone"
                ),
            });
        }
        if self.0.exit_on_start {
            self.0.fake.set_container_state(name, Liveness::Exited);
            if let Some(execution) = self
                .0
                .execution
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone()
            {
                self.0.fake.set_execution(name, execution);
            }
        }
        Ok(())
    }
    fn stop(&self, name: &str, mode: StopMode) -> Result<Settled, RuntimeError> {
        self.0.fake.stop(name, mode)
    }
    fn remove(&self, name: &str) -> Result<Settled, RuntimeError> {
        self.0.fake.remove(name)
    }
    fn reaper_program(&self) -> PathBuf {
        self.0.fake.reaper_program()
    }
}

struct Fixture {
    /// The guard over the root [`Fixture::new`] acquired, kept for the
    /// fixture's whole life so the tree is reclaimed when it drops -- on a
    /// panicking assertion as much as on a normal return.
    _tree: ScratchTree,
    root: PathBuf,
    repo: PathBuf,
    private_root: PathBuf,
    paths: RunPaths,
    identity: RunIdentity,
    task_a: PathBuf,
    task_b: PathBuf,
    merge: PathBuf,
    trace: ContainerTrace,
    runtime: Runtime,
}

struct Layout {
    repo: PathBuf,
    private_root: PathBuf,
    paths: RunPaths,
    task_a: PathBuf,
    task_b: PathBuf,
    merge: PathBuf,
}

impl Layout {
    fn of(root: &std::path::Path) -> Self {
        let repo = root.join("repo");
        let private_root = root.join("private");
        let paths = RunPaths::with_private_root(&repo, RUN_ID, &private_root);
        let execution_root =
            crate::workspace_manager::execution_root_of(&private_root, repo_key(), RUN_ID);
        Self {
            task_a: execution_root.join("tasks").join("kalpha-g0"),
            task_b: execution_root.join("tasks").join("kbeta-g0"),
            merge: execution_root.join("merge").join("s0"),
            repo,
            private_root,
            paths,
        }
    }

    fn build(&self) {
        let (head, _) = repo::repository(&self.repo);
        self.paths.create().expect("the run's two halves");
        std::fs::write(self.paths.events(), format!("{EVENT_LOG_MARKER}\n"))
            .expect("the public log");
        std::fs::write(
            self.paths.transcripts().join("k0-a1.md"),
            "PRIVATE-TRANSCRIPT-a5f2\n",
        )
        .expect("a private artifact");
        std::fs::create_dir_all(crate::runner::container::intent::containers_dir(
            &self.private_root,
        ))
        .expect("the container namespace");
        for at in [&self.task_a, &self.task_b, &self.merge] {
            repo::worktree(&self.repo, at, &head);
        }
        std::fs::write(self.task_b.join("sibling.txt"), "SIBLING-WORKTREE-a5f2\n")
            .expect("a sibling file");
    }
}

#[cfg(unix)]
const FIXTURE_ROOT: &str = "UPSTROKE_TEST_EXEC_FIXTURE_ROOT";

#[cfg(unix)]
const FIXTURE_CHILD_BOUND: Duration = Duration::from_secs(120);

#[cfg(unix)]
#[test]
#[ignore = "isolated builder of Fixture::built_in_a_bounded_child"]
fn fixture_tree_child() {
    let root = PathBuf::from(std::env::var_os(FIXTURE_ROOT).expect("the parent names the root"));
    Layout::of(&root).build();
}

impl Fixture {
    fn new(tag: &str, exit_on_start: bool) -> Self {
        let tree = repo::scratch(tag);
        Layout::of(tree.path()).build();
        Self::over(tree, exit_on_start)
    }

    #[cfg(unix)]
    fn built_in_a_bounded_child(tag: &str, exit_on_start: bool) -> Self {
        let tree = repo::scratch(tag);
        let ended = crate::agent::proc::test_support::run_test_isolated(
            "runner::container::exec::tests::fixture_tree_child",
            &[(FIXTURE_ROOT, tree.path().as_os_str())],
            FIXTURE_CHILD_BOUND,
        );
        assert!(
            ended.status.is_some_and(|status| status.success())
                && ended.stdout.contains("1 passed"),
            "an isolated child of this test binary built the fixture's repository and \
             worktrees under {} within {FIXTURE_CHILD_BOUND:?}: {ended}",
            tree.path().display()
        );
        Self::over(tree, exit_on_start)
    }

    fn over(tree: ScratchTree, exit_on_start: bool) -> Self {
        let root = tree.path().to_path_buf();
        let Layout {
            repo: repo_dir,
            private_root,
            paths,
            task_a,
            task_b,
            merge,
        } = Layout::of(&root);
        let trace = ContainerTrace::recording();
        Self {
            identity: RunIdentity {
                private_root: private_root.clone(),
                run_id: RUN_ID.to_owned(),
                run_dir: paths.public.clone(),
                incarnation: INCARNATION_1.to_owned(),
                repo_key: repo_key().to_owned(),
            },
            runtime: Runtime::new(trace.clone(), exit_on_start),
            trace,
            _tree: tree,
            root,
            repo: repo_dir,
            private_root,
            paths,
            task_a,
            task_b,
            merge,
        }
    }

    fn confinement(&self) -> Confinement {
        Confinement::of_run(
            &self.identity,
            &self.repo,
            fixtures::plain_checkout(&self.repo.join(".git")),
        )
    }

    fn runner(&self) -> ContainerRunner {
        self.runner_with(self.identity.clone())
    }

    #[cfg(unix)]
    fn runner_over(&self, policy: RunnerPolicy) -> ContainerRunner {
        ContainerRunner::new(
            policy,
            self.identity.clone(),
            &self.repo,
            fixtures::plain_checkout(&self.repo.join(".git")),
            image_environment(),
            Box::new(self.runtime.clone()),
        )
        .expect("a container policy")
        .with_hooks(Box::new(RecordingHooks::new(self.trace.clone())))
        .with_poll(Duration::ZERO)
    }

    #[cfg(unix)]
    fn mounted_policy(&self) -> (RunnerPolicy, PathBuf, PathBuf) {
        let shared = self.root.join("operator-shared");
        let tools = self.root.join("operator-tools");
        std::fs::create_dir_all(&shared).expect("an operator directory");
        std::fs::create_dir_all(&tools).expect("another operator directory");
        let selection = crate::config::RunnerSelection {
            kind: RunnerKind::Container,
            image: Some(IMAGE_REFERENCE.to_owned()),
            credential_volumes: recorded_volumes(&container_policy()),
            mounts: vec![
                crate::config::RunnerMount {
                    source: shared.clone(),
                    target: "/data".to_owned(),
                    read_only: true,
                },
                crate::config::RunnerMount {
                    source: tools.clone(),
                    target: "/opt/tools".to_owned(),
                    read_only: false,
                },
            ],
            from_config: true,
        };
        let context = MountContext::bind_with(
            &self.repo,
            fixtures::plain_checkout(&self.repo.join(".git")),
            &self.private_root,
            &[],
        )
        .expect("the fixture binds a mount context");
        let policy = crate::runner::container::resolve::resolve_container(
            self.runtime.fake(),
            &selection,
            &context,
        )
        .expect("the configured mounts resolve");
        (
            policy,
            std::fs::canonicalize(&shared).expect("canonical"),
            std::fs::canonicalize(&tools).expect("canonical"),
        )
    }

    fn runner_with(&self, identity: RunIdentity) -> ContainerRunner {
        ContainerRunner::new(
            container_policy(),
            identity,
            &self.repo,
            fixtures::plain_checkout(&self.repo.join(".git")),
            image_environment(),
            Box::new(self.runtime.clone()),
        )
        .expect("a container policy")
        .with_hooks(Box::new(RecordingHooks::new(self.trace.clone())))
        .with_poll(Duration::ZERO)
    }

    fn withheld(&self) -> Vec<(Withheld, PathBuf)> {
        vec![
            (Withheld::PublicLog, self.paths.public.clone()),
            (Withheld::PublicLog, self.paths.events()),
            (Withheld::PrivateArtifacts, self.paths.private.clone()),
            (Withheld::PrivateArtifacts, self.paths.transcripts()),
            (
                Withheld::PrivateArtifacts,
                crate::runner::container::intent::containers_dir(&self.private_root),
            ),
            (Withheld::SiblingWorktree, self.task_b.clone()),
            (Withheld::SiblingWorktree, self.merge.clone()),
            (Withheld::AuthoritativeGit, self.repo.join(".git")),
        ]
    }
}

#[test]
fn the_runner_reports_what_it_established_about_the_process_when_it_fails() {
    use crate::error::ProcessFate;
    use crate::runner::container::runtime::RuntimeOp;
    use crate::topology::effects::{EffectSiteId, HookPhase};

    struct Cell {
        case: &'static str,
        unreachable: &'static [RuntimeOp],
        failing: &'static [RuntimeOp],
        removal_in_progress: bool,
        hook: Option<(ContainerSite, HookPhase)>,
        mismatch: bool,
        exit_on_start: bool,
        timeout: Duration,
        expected: ProcessFate,
        start_attempted: bool,
        survivor: Option<Liveness>,
        view_and_intent_retained: bool,
    }

    let ten = Duration::from_secs(10);
    let cells = [
        Cell {
            case: "the runtime is down before create",
            unreachable: &[RuntimeOp::Create, RuntimeOp::Stop, RuntimeOp::Remove],
            failing: &[],
            removal_in_progress: false,
            hook: None,
            mismatch: false,
            exit_on_start: false,
            timeout: ten,
            expected: ProcessFate::NeverStarted,
            start_attempted: false,
            survivor: None,
            view_and_intent_retained: true,
        },
        Cell {
            case: "the runtime refuses to create",
            unreachable: &[],
            failing: &[RuntimeOp::Create],
            removal_in_progress: false,
            hook: None,
            mismatch: false,
            exit_on_start: false,
            timeout: ten,
            expected: ProcessFate::NeverStarted,
            start_attempted: false,
            survivor: None,
            view_and_intent_retained: false,
        },
        Cell {
            case: "the created container reports another image",
            unreachable: &[],
            failing: &[],
            removal_in_progress: false,
            hook: None,
            mismatch: true,
            exit_on_start: false,
            timeout: ten,
            expected: ProcessFate::NeverStarted,
            start_attempted: false,
            survivor: None,
            view_and_intent_retained: false,
        },
        Cell {
            case: "the runtime refuses to start and the cancel completes",
            unreachable: &[],
            failing: &[RuntimeOp::Start],
            removal_in_progress: false,
            hook: None,
            mismatch: false,
            exit_on_start: false,
            timeout: ten,
            expected: ProcessFate::Gone,
            start_attempted: true,
            survivor: None,
            view_and_intent_retained: false,
        },
        Cell {
            case: "the start is committed and the launch fails after it",
            unreachable: &[],
            failing: &[],
            removal_in_progress: false,
            hook: Some((ContainerSite::Start, HookPhase::After)),
            mismatch: false,
            exit_on_start: false,
            timeout: ten,
            expected: ProcessFate::Gone,
            start_attempted: true,
            survivor: None,
            view_and_intent_retained: false,
        },
        Cell {
            case: "the runtime is lost at start and the cancel establishes nothing",
            unreachable: &[RuntimeOp::Start, RuntimeOp::Stop, RuntimeOp::Remove],
            failing: &[],
            removal_in_progress: false,
            hook: None,
            mismatch: false,
            exit_on_start: false,
            timeout: ten,
            expected: ProcessFate::Unresolved,
            start_attempted: true,
            survivor: Some(Liveness::Exited),
            view_and_intent_retained: true,
        },
        Cell {
            case: "the observation is lost and the release completes",
            unreachable: &[RuntimeOp::Observe],
            failing: &[],
            removal_in_progress: false,
            hook: None,
            mismatch: false,
            exit_on_start: false,
            timeout: ten,
            expected: ProcessFate::Gone,
            start_attempted: true,
            survivor: None,
            view_and_intent_retained: false,
        },
        Cell {
            case: "the observation and the stop are lost and the forced removal completes",
            unreachable: &[RuntimeOp::Observe, RuntimeOp::Stop],
            failing: &[],
            removal_in_progress: false,
            hook: None,
            mismatch: false,
            exit_on_start: false,
            timeout: ten,
            expected: ProcessFate::Gone,
            start_attempted: true,
            survivor: None,
            view_and_intent_retained: false,
        },
        Cell {
            case: "the observation, the stop and the removal are all lost",
            unreachable: &[RuntimeOp::Observe, RuntimeOp::Stop, RuntimeOp::Remove],
            failing: &[],
            removal_in_progress: false,
            hook: None,
            mismatch: false,
            exit_on_start: false,
            timeout: ten,
            expected: ProcessFate::Unresolved,
            start_attempted: true,
            survivor: Some(Liveness::Running),
            view_and_intent_retained: true,
        },
        Cell {
            case: "the exit is observed and then the collection, the stop and the removal are lost",
            unreachable: &[RuntimeOp::Collect, RuntimeOp::Stop, RuntimeOp::Remove],
            failing: &[],
            removal_in_progress: false,
            hook: None,
            mismatch: false,
            exit_on_start: true,
            timeout: ten,
            expected: ProcessFate::Gone,
            start_attempted: true,
            survivor: Some(Liveness::Exited),
            view_and_intent_retained: false,
        },
        Cell {
            case: "the gate times out and only the view discard fails",
            unreachable: &[],
            failing: &[],
            removal_in_progress: false,
            hook: Some((ContainerSite::UnmountGitView, HookPhase::Before)),
            mismatch: false,
            exit_on_start: false,
            timeout: Duration::ZERO,
            expected: ProcessFate::Gone,
            start_attempted: true,
            survivor: None,
            view_and_intent_retained: true,
        },
        Cell {
            case: "the gate times out, the stop fails and the forced removal completes",
            unreachable: &[],
            failing: &[RuntimeOp::Stop],
            removal_in_progress: false,
            hook: None,
            mismatch: false,
            exit_on_start: false,
            timeout: Duration::ZERO,
            expected: ProcessFate::Gone,
            start_attempted: true,
            survivor: None,
            view_and_intent_retained: false,
        },
        Cell {
            case: "the gate times out and neither the stop nor the removal completes",
            unreachable: &[RuntimeOp::Stop, RuntimeOp::Remove],
            failing: &[],
            removal_in_progress: false,
            hook: None,
            mismatch: false,
            exit_on_start: false,
            timeout: Duration::ZERO,
            expected: ProcessFate::Unresolved,
            start_attempted: true,
            survivor: Some(Liveness::Running),
            view_and_intent_retained: true,
        },
        Cell {
            case: "the observation and the stop are lost and another reclaimer's removal is in \
                   progress",
            unreachable: &[RuntimeOp::Observe, RuntimeOp::Stop],
            failing: &[],
            removal_in_progress: true,
            hook: None,
            mismatch: false,
            exit_on_start: false,
            timeout: ten,
            expected: ProcessFate::Unresolved,
            start_attempted: true,
            survivor: Some(Liveness::Running),
            view_and_intent_retained: true,
        },
        Cell {
            case: "the gate times out, the stop is lost and another reclaimer's removal is in \
                   progress",
            unreachable: &[RuntimeOp::Stop],
            failing: &[],
            removal_in_progress: true,
            hook: None,
            mismatch: false,
            exit_on_start: false,
            timeout: Duration::ZERO,
            expected: ProcessFate::Unresolved,
            start_attempted: true,
            survivor: Some(Liveness::Running),
            view_and_intent_retained: true,
        },
        Cell {
            case: "the funnel refuses before `docker start` is attempted and the cancel cannot \
                   reach the runtime",
            unreachable: &[RuntimeOp::Stop, RuntimeOp::Remove],
            failing: &[],
            removal_in_progress: false,
            hook: Some((ContainerSite::Start, HookPhase::Before)),
            mismatch: false,
            exit_on_start: false,
            timeout: ten,
            expected: ProcessFate::NeverStarted,
            start_attempted: false,
            survivor: Some(Liveness::Exited),
            view_and_intent_retained: true,
        },
    ];

    let mut seen = 0_usize;
    for (index, cell) in cells.iter().enumerate() {
        let case = cell.case;
        let fixture = Fixture::new(&format!("fate-{index}"), cell.exit_on_start);
        for op in cell.unreachable {
            fixture.runtime.fake().set_unreachable(*op);
        }
        for op in cell.failing {
            fixture.runtime.fake().set_failing(*op);
        }
        let request = gate_request(
            ShellKind::Sh.spec(if cell.timeout.is_zero() {
                "sleep 600"
            } else {
                "exit 0"
            }),
            fixture.task_a.clone(),
            cell.timeout,
            gate_id(0),
        );
        let name = ContainerName::new(repo_key(), RUN_ID, INCARNATION_1, &request.invocation)
            .expect("a container name");
        if cell.removal_in_progress {
            fixture.runtime.fake().set_docker_stderr(
                RuntimeOp::Remove,
                &removal_in_progress_diagnostic(name.as_str()),
            );
        }
        if cell.mismatch {
            fixture
                .runtime
                .fake()
                .substitute_reported_image_id(name.as_str(), OTHER_IMAGE_ID);
        }
        let mut runner = fixture.runner();
        if let Some((site, phase)) = cell.hook {
            let mut hooks = RecordingHooks::new(fixture.trace.clone());
            hooks.fail_at(EffectSiteId::Container(site), phase);
            runner = runner.with_hooks(Box::new(hooks));
        }

        let error = runner.run_blocking(&request).expect_err(case);
        assert_eq!(error.fate, cell.expected, "{case}: {error}");
        assert!(
            error.to_string().contains(cell.expected.describe()),
            "{case}: the error says what was established: {error}"
        );
        assert_eq!(
            fixture.trace.ops().contains(&RuntimeOp::Start),
            cell.start_attempted,
            "{case}: whether `docker start` was attempted decides whether a spawn failure can \
             be claimed; ops were {:?}",
            fixture.trace.ops()
        );
        let names = fixture.runtime.fake().container_names();
        let survivor = names
            .first()
            .and_then(|name| fixture.runtime.fake().container(name))
            .map(|container| container.state);
        assert_eq!(
            survivor, cell.survivor,
            "{case}: the container left behind and its state; names were {names:?}"
        );
        let view_retained = view_dir(&fixture.private_root, &name).exists();
        let intent_retained = name.intent_path(&fixture.private_root).exists();
        assert_eq!(
            (view_retained, intent_retained),
            (cell.view_and_intent_retained, cell.view_and_intent_retained),
            "{case}: the R19 view and the R26 intent are retained together exactly when the \
             runtime did not establish the container gone (or the view could not be pruned)"
        );
        if cell.view_and_intent_retained {
            assert!(
                error.to_string().contains("deliberately retained"),
                "{case}: the error says the residue was kept on purpose: {error}"
            );
        }
        seen += 1;
    }
    assert_eq!(seen, 16, "every cell of the container fate matrix ran");
}

#[test]
fn a_start_that_left_a_process_unresolved_stays_unresolved_after_a_clean_cancel() {
    #[cfg(unix)]
    unresolved_process_in_an_owned_helper("start");
    #[cfg(not(unix))]
    assert_unresolved_start_fate(&Fixture::new("unresolved-start", false));
}

fn assert_unresolved_start_fate(fixture: &Fixture) {
    use crate::error::ProcessFate;

    fixture.runtime.leaves_its_start_unresolved();
    let request = gate_request(
        ShellKind::Sh.spec("exit 0"),
        fixture.task_a.clone(),
        Duration::from_secs(10),
        gate_id(0),
    );

    let error = fixture
        .runner()
        .run_blocking(&request)
        .expect_err("the start left a process unresolved");
    assert_eq!(error.fate, ProcessFate::Unresolved, "{error}");
    let message = error.to_string();
    assert!(
        message.contains(ProcessFate::Unresolved.describe()),
        "{message}"
    );
    let ops = fixture.trace.ops();
    assert!(
        ops.contains(&RuntimeOp::Start) && ops.contains(&RuntimeOp::Remove),
        "the start ran and the cancel removed the container: {ops:?}"
    );
    assert_eq!(
        fixture.runtime.fake().container_names(),
        Vec::<String>::new(),
        "the cancel completed"
    );
}

#[test]
fn a_container_the_runtime_cannot_confirm_stopped_keeps_its_mounted_git_view_and_intent() {
    use crate::error::ProcessFate;

    let fixture = Fixture::new("unresolved-mounted-view", false);
    for op in [RuntimeOp::Observe, RuntimeOp::Stop, RuntimeOp::Remove] {
        fixture.runtime.fake().set_unreachable(op);
    }
    let request = gate_request(
        ShellKind::Sh.spec("exit 0"),
        fixture.task_a.clone(),
        Duration::from_secs(10),
        gate_id(0),
    );
    let name = ContainerName::new(repo_key(), RUN_ID, INCARNATION_1, &request.invocation)
        .expect("a container name");

    let error = fixture
        .runner()
        .run_blocking(&request)
        .expect_err("the runtime lost the gate");
    assert_eq!(error.fate, ProcessFate::Unresolved, "{error}");
    assert_eq!(
        fixture
            .runtime
            .fake()
            .container(name.as_str())
            .map(|container| container.state),
        Some(Liveness::Running),
        "the runtime never confirmed the gate stopped"
    );
    let view = view_dir(&fixture.private_root, &name);
    let intent = name.intent_path(&fixture.private_root);
    assert!(
        view.exists() && intent.exists(),
        "a live container's mounted Git view and its intent record must outlive an \
         unresolved release: view={}, intent={}",
        view.exists(),
        intent.exists()
    );
    let sites: Vec<String> = fixture
        .trace
        .rendered()
        .into_iter()
        .filter(|entry| entry.starts_with("site:"))
        .collect();
    assert!(
        !sites
            .iter()
            .any(|site| site.starts_with("site:UnmountGitView:"))
            && !sites
                .iter()
                .any(|site| site.starts_with("site:RemoveIntent:")),
        "neither the view nor the intent was touched while the container may run: {sites:#?}"
    );
    let message = error.to_string();
    assert!(
        message.contains("deliberately retained") && message.contains("R26"),
        "the error names the retained anchor: {message}"
    );

    for op in [RuntimeOp::Observe, RuntimeOp::Stop, RuntimeOp::Remove] {
        fixture.runtime.fake().set_reachable(op);
    }
    let found = list_intents(&fixture.private_root).expect("scan");
    assert_eq!(
        found.len(),
        1,
        "the retained intent is what a later census discovers the survivor through"
    );
    crate::runner::container::reclaim(
        &mut RecordingHooks::new(fixture.trace.clone()),
        &fixture.runtime,
        &crate::runner::container::DisposableDirView::new(fixture.trace.clone()),
        &fixture.private_root,
        &name,
        Some(&view),
    )
    .expect("with the runtime back, the census reclaim converges on the retained record");
    assert!(fixture.runtime.fake().container_names().is_empty());
    assert!(!view.exists() && !intent.exists());
}

fn worker_id(ordinal: u32) -> InvocationId {
    InvocationId::attempt(
        TaskKey(0),
        GenerationId(0),
        AttemptNumber(1),
        AttemptRole::Worker,
        ordinal,
    )
}

fn gate_id(ordinal: u32) -> InvocationId {
    InvocationId::attempt(
        TaskKey(0),
        GenerationId(0),
        AttemptNumber(1),
        AttemptRole::Gate(ordinal),
        0,
    )
}

fn shell_probe_id() -> InvocationId {
    InvocationId::probe(ProbeTarget::Shell, 0).expect("the shell probe identity")
}

fn agent_probe_id(agent: &str) -> InvocationId {
    InvocationId::probe(ProbeTarget::Agent(AgentId::new(agent)), 0)
        .expect("an agent probe identity")
}

fn requests(workspace: &Path) -> Vec<RunnerRequest> {
    let claude = AgentId::new("claude-code");
    let spec = ShellKind::Sh.spec("exit 0");
    vec![
        crate::agent::probe_request("claude-code", spec.clone(), 0, Duration::from_secs(10))
            .expect("an agent probe request"),
        host::shell_probe_request(ShellKind::Sh, workspace.to_path_buf(), shell_probe_id()),
        worker_request(
            spec.clone(),
            workspace.to_path_buf(),
            claude.clone(),
            Duration::from_secs(10),
            worker_id(0),
        ),
        gate_request(
            spec.clone(),
            workspace.to_path_buf(),
            Duration::from_secs(10),
            gate_id(0),
        ),
        review_request(
            spec,
            workspace.to_path_buf(),
            claude,
            Duration::from_secs(10),
            InvocationId::attempt(
                TaskKey(0),
                GenerationId(0),
                AttemptNumber(1),
                AttemptRole::ReviewPass(0),
                0,
            ),
        ),
    ]
}

fn hostile_bindings(workspace: &Path) -> Vec<RunnerRequest> {
    let claude = AgentId::new("claude-code");
    vec![
        RunnerRequest {
            command: ShellKind::Sh.spec("cargo test"),
            workspace: workspace.to_path_buf(),
            role: ExecutionRole::Gate,
            timeout: Duration::from_secs(10),
            agent: Some(claude.clone()),
            invocation: gate_id(7),
        },
        RunnerRequest {
            command: ShellKind::Sh.spec("exit 0"),
            workspace: workspace.to_path_buf(),
            role: ExecutionRole::Probe(ProbeTarget::Shell),
            timeout: Duration::from_secs(10),
            agent: Some(claude),
            invocation: shell_probe_id(),
        },
    ]
}

fn sources(mounts: &[Mount]) -> Vec<PathBuf> {
    mounts
        .iter()
        .filter_map(|mount| match mount {
            Mount::Path { source, .. } => Some(source.clone()),
            Mount::Volume { .. } | Mount::Tmpfs { .. } => None,
        })
        .collect()
}

fn target_of<'a>(mounts: &'a [Mount], target: &str) -> Option<&'a Mount> {
    mounts.iter().find(|mount| mount.target() == target)
}

#[test]
fn the_view_path_the_census_prunes_is_the_one_the_invocation_mounts() {
    let fixture = Fixture::new("view-path-convention", true);
    let runner = fixture.runner();
    let mut seen = std::collections::BTreeSet::new();
    for ordinal in 0..3 {
        let request = worker_request(
            ShellKind::Sh.spec("exit 0"),
            fixture.task_a.clone(),
            AgentId::new("claude-code"),
            Duration::from_secs(10),
            worker_id(ordinal),
        );
        let plan = runner.plan(&request).expect("plans");
        let name = plan.launch.name.clone();

        let expected = fixture.private_root.join("views").join(name.as_str());
        assert_eq!(
            plan.launch.view.path, expected,
            "the invocation mounts a view the census would not look for"
        );
        assert_eq!(
            crate::runner::container::census::view_path(&fixture.private_root, &name),
            expected,
            "the census prunes a view the invocation would not have mounted"
        );
        assert!(
            plan.mounts().iter().any(|mount| matches!(
                mount,
                Mount::Path { source, .. } if source == &expected
            )),
            "{:?}",
            plan.mounts()
        );
        seen.insert(expected);
    }
    assert_eq!(
        seen.len(),
        3,
        "three invocations must give three view paths; a convention that ignored the \
         container name would give one"
    );
}

#[test]
fn the_mount_set_is_the_roles_own_and_reaches_nothing_of_the_coordinators() {
    let fixture = Fixture::new("mounts", true);
    let runner = fixture.runner();
    let request = worker_request(
        ShellKind::Sh.spec("exit 0"),
        fixture.task_a.clone(),
        AgentId::new("claude-code"),
        Duration::from_secs(10),
        worker_id(0),
    );
    let plan = runner.plan(&request).expect("plans");

    let mounts = plan.mounts();
    let targets: Vec<&str> = mounts.iter().map(Mount::target).collect();
    assert_eq!(
        targets,
        vec![
            "/upstroke/workspace",
            "/upstroke/gitview",
            "/upstroke/gitobjects",
            "/upstroke/workspace/.git",
            "/upstroke/credentials/claude-code",
            "/tmp",
        ],
        "the mount set moved"
    );
    assert_eq!(
        target_of(mounts, "/upstroke/gitobjects").map(Mount::read_only),
        Some(true),
        "the borrowed object store is read-only (DESIGN.md:612)"
    );
    assert_eq!(
        target_of(mounts, "/upstroke/workspace").map(Mount::read_only),
        Some(false),
        "an implementer writes to its worktree"
    );
    assert_eq!(
        target_of(mounts, "/tmp"),
        Some(&Mount::Tmpfs {
            target: "/tmp".to_owned()
        }),
        "the scratch surface is not a tmpfs"
    );
    assert!(
        plan.launch.spec.read_only_root,
        "the container layer is writable, so `gate write outside mount fails` does not hold"
    );

    let withheld = fixture.withheld();
    assert!(withheld.len() >= 8, "the fixture withholds {withheld:?}");
    for (category, path) in &withheld {
        assert!(
            path.exists(),
            "{path:?} does not exist, so withholding it proves nothing"
        );
        for source in sources(mounts) {
            assert!(
                !path.starts_with(&source),
                "the mount `{}` hands the container `{}` ({})",
                source.display(),
                path.display(),
                category.passage()
            );
        }
    }
    assert!(
        fixture.confinement().violations(mounts).is_empty(),
        "{:?}",
        fixture.confinement().violations(mounts)
    );

    let hostile = vec![Mount::Path {
        source: fixture.root.clone(),
        target: "/everything".to_owned(),
        read_only: false,
    }];
    let found = fixture.confinement().violations(&hostile);
    let categories: BTreeSet<&str> = Withheld::ALL
        .iter()
        .filter(|category| found.iter().any(|entry| entry.contains(category.passage())))
        .map(|category| category.passage())
        .collect();
    assert_eq!(
        categories.len(),
        Withheld::ALL.len(),
        "a mount of the whole tree did not name every withheld category: {found:#?}"
    );
    assert_eq!(Withheld::ALL.len(), 4);
}

fn review_pass_id() -> InvocationId {
    InvocationId::attempt(
        TaskKey(0),
        GenerationId(0),
        AttemptNumber(1),
        AttemptRole::ReviewPass(0),
        0,
    )
}

fn naming_its_settings(own: &Path) -> crate::runner::CommandSpec {
    crate::runner::CommandSpec::new("claude")
        .arg("-p")
        .arg("--settings")
        .arg(own.to_string_lossy().into_owned())
        .settings_file(Some(own.to_path_buf()))
}

fn materialized_settings(fixture: &Fixture) -> (PathBuf, PathBuf) {
    let settings = fixture.paths.settings();
    let worker = settings.join("t1-1.json");
    let reviewer = settings.join("t1-review.json");
    for file in [&worker, &reviewer] {
        std::fs::write(file, "{\"permissions\":{}}\n").expect("a materialized settings file");
    }
    (worker, reviewer)
}

fn settings_mounts(mounts: &[Mount]) -> Vec<&Mount> {
    mounts
        .iter()
        .filter(|mount| mount.target().starts_with(BoundaryLayout::DEFAULT_SETTINGS))
        .collect()
}

#[test]
fn an_agents_own_settings_file_is_bound_read_only_and_named_by_its_container_path() {
    let fixture = Fixture::new("settings-bind", true);
    let runner = fixture.runner();
    let settings = fixture.paths.settings();
    assert_eq!(
        settings,
        fixture
            .private_root
            .join("runs")
            .join(&fixture.identity.run_id)
            .join("settings"),
        "the runner recognizes the directory `RunPaths` materializes settings into"
    );
    let (worker_own, reviewer_own) = materialized_settings(&fixture);
    let claude = AgentId::new("claude-code");
    let worker = worker_request(
        naming_its_settings(&worker_own),
        fixture.task_a.clone(),
        claude.clone(),
        Duration::from_secs(10),
        worker_id(0),
    );
    let review = review_request(
        naming_its_settings(&reviewer_own),
        fixture.task_a.clone(),
        claude,
        Duration::from_secs(10),
        review_pass_id(),
    );
    for (request, own, inside) in [
        (&worker, &worker_own, "/upstroke/settings/t1-1.json"),
        (&review, &reviewer_own, "/upstroke/settings/t1-review.json"),
    ] {
        let plan = runner.plan(request).expect("plans");
        let mounts = plan.mounts();
        let targets: Vec<&str> = mounts.iter().map(Mount::target).collect();
        assert_eq!(
            targets,
            vec![
                "/upstroke/workspace",
                "/upstroke/gitview",
                "/upstroke/gitobjects",
                "/upstroke/workspace/.git",
                "/upstroke/credentials/claude-code",
                "/tmp",
                inside,
            ],
            "{:?}: the role's own mounts, and its one settings file",
            request.role
        );
        assert_eq!(
            settings_mounts(mounts),
            vec![&Mount::Path {
                source: own.clone(),
                target: inside.to_owned(),
                read_only: true,
            }],
            "{:?}: exactly its own file, read-only, and no other invocation's",
            request.role
        );
        assert_eq!(
            plan.launch.spec.command,
            vec!["claude", "-p", "--settings", inside],
            "{:?}: the agent is given the container's path",
            request.role
        );
        assert!(
            !plan
                .launch
                .spec
                .command
                .iter()
                .any(|arg| arg.contains(&*fixture.private_root.to_string_lossy())),
            "{:?}: no host path reaches the container's argv: {:?}",
            request.role,
            plan.launch.spec.command
        );
        assert!(
            fixture.confinement().violations(mounts).is_empty(),
            "{:?}",
            fixture.confinement().violations(mounts)
        );
        for (category, path) in &fixture.withheld() {
            for source in sources(mounts) {
                assert!(
                    !path.starts_with(&source),
                    "the mount `{}` hands the container `{}` ({})",
                    source.display(),
                    path.display(),
                    category.passage()
                );
            }
        }
        assert!(
            !sources(mounts)
                .iter()
                .any(|source| source == &settings || source == &fixture.paths.private),
            "{:?}: the settings directory and the private half stay withheld",
            request.role
        );
    }
}

#[test]
fn only_the_invocations_own_settings_file_is_bound_and_no_argument_grants_another() {
    let fixture = Fixture::new("settings-own-only", true);
    let runner = fixture.runner();
    let (worker_own, reviewer_own) = materialized_settings(&fixture);
    let claude = AgentId::new("claude-code");
    let ten = Duration::from_secs(10);
    let host = |path: &Path| path.to_string_lossy().into_owned();

    let with_another = worker_request(
        naming_its_settings(&worker_own).arg(host(&reviewer_own)),
        fixture.task_a.clone(),
        claude.clone(),
        ten,
        worker_id(0),
    );
    let plan = runner.plan(&with_another).expect("plans");
    assert_eq!(
        settings_mounts(plan.mounts()),
        vec![&Mount::Path {
            source: worker_own.clone(),
            target: "/upstroke/settings/t1-1.json".to_owned(),
            read_only: true,
        }],
        "another invocation's settings file named in the argv is given no mount"
    );
    assert_eq!(
        plan.launch.spec.command,
        vec![
            "claude".to_owned(),
            "-p".to_owned(),
            "--settings".to_owned(),
            "/upstroke/settings/t1-1.json".to_owned(),
            host(&reviewer_own),
        ],
        "only the intended argument is rewritten; the other stays the host path it was"
    );

    for (case, request) in [
        (
            "an argument naming a settings file, with no metadata",
            worker_request(
                crate::runner::CommandSpec::new("claude")
                    .arg("--settings")
                    .arg(host(&worker_own)),
                fixture.task_a.clone(),
                claude.clone(),
                ten,
                worker_id(1),
            ),
        ),
        (
            "metadata whose file no argument names",
            worker_request(
                crate::runner::CommandSpec::new("codex")
                    .arg("exec")
                    .settings_file(Some(worker_own.clone())),
                fixture.task_a.clone(),
                claude.clone(),
                ten,
                worker_id(2),
            ),
        ),
        (
            "a gate carrying metadata and the argument",
            gate_request(
                naming_its_settings(&worker_own),
                fixture.task_a.clone(),
                ten,
                gate_id(0),
            ),
        ),
        (
            "an agent probe carrying metadata and the argument",
            crate::agent::probe_request("claude-code", naming_its_settings(&worker_own), 0, ten)
                .expect("an agent probe request"),
        ),
    ] {
        let plan = runner.plan(&request).expect("plans");
        assert!(
            settings_mounts(plan.mounts()).is_empty(),
            "{case}: no settings mount: {:?}",
            plan.mounts()
        );
        let mut expected = vec![request.command.program.clone()];
        expected.extend(request.command.args.iter().cloned());
        assert_eq!(
            plan.launch.spec.command, expected,
            "{case}: the argv is the command's own"
        );
    }
}

#[test]
fn an_own_settings_file_that_is_not_a_regular_file_of_the_runs_settings_directory_is_refused() {
    let fixture = Fixture::new("settings-identity", true);
    let runner = fixture.runner();
    let (worker_own, _) = materialized_settings(&fixture);
    let settings = fixture.paths.settings();
    let a_directory = settings.join("t1-2.json");
    std::fs::create_dir(&a_directory).expect("a directory where a settings file would be");
    let elsewhere = fixture.paths.transcripts().join("k0-a1.md");
    #[cfg(unix)]
    let a_link = {
        let a_link = settings.join("t1-3.json");
        std::os::unix::fs::symlink(&elsewhere, &a_link).expect("a link to a private artifact");
        Some((
            "a link in the settings directory to a private artifact",
            a_link,
            "not a regular file",
        ))
    };
    #[cfg(not(unix))]
    let a_link = None;
    let cases = [
        (
            "a directory in the settings directory",
            a_directory,
            "not a regular file",
        ),
        (
            "a file elsewhere in the private half",
            elsewhere.clone(),
            "not a file of the run's settings directory",
        ),
    ]
    .into_iter()
    .chain(a_link);
    for (case, own, why) in cases {
        let request = worker_request(
            naming_its_settings(&own),
            fixture.task_a.clone(),
            AgentId::new("claude-code"),
            Duration::from_secs(10),
            worker_id(0),
        );
        let refused = runner
            .plan(&request)
            .expect_err("a settings file that is not the invocation's own is refused");
        assert!(refused.to_string().contains(why), "{case}: {refused}");
    }
    let control = worker_request(
        naming_its_settings(&worker_own),
        fixture.task_a.clone(),
        AgentId::new("claude-code"),
        Duration::from_secs(10),
        worker_id(0),
    );
    assert!(
        runner.plan(&control).is_ok(),
        "the control: the regular file the invocation materialized plans"
    );
}

#[cfg(target_os = "linux")]
fn effective_ids() -> ContainerUser {
    let status = std::fs::read_to_string("/proc/self/status").expect("this process's status");
    let effective = |key: &str| -> u32 {
        status
            .lines()
            .find_map(|line| line.strip_prefix(key))
            .and_then(|ids| ids.split_whitespace().nth(1))
            .and_then(|id| id.parse().ok())
            .unwrap_or_else(|| panic!("no effective id after `{key}` in {status}"))
    };
    ContainerUser {
        uid: effective("Uid:"),
        gid: effective("Gid:"),
    }
}

#[cfg(all(unix, not(target_os = "linux")))]
fn effective_ids() -> ContainerUser {
    // SAFETY: `geteuid` and `getegid` take no arguments, read no memory of
    // this process's, and always succeed (POSIX.1-2008).
    let (uid, gid) = unsafe { (libc::geteuid(), libc::getegid()) };
    ContainerUser { uid, gid }
}

#[cfg(unix)]
#[test]
fn every_container_is_created_as_the_coordinators_user() {
    let expected = effective_ids();
    let fixture = Fixture::new("container-user", true);
    let runner = fixture.runner();
    let mut roles = BTreeSet::new();
    for request in requests(&fixture.task_a) {
        let plan = runner.plan(&request).expect("plans");
        assert_eq!(
            plan.launch.spec.user,
            Some(expected),
            "{:?} is not created as the coordinator's effective uid and gid",
            request.role
        );
        let args = crate::runner::container::docker_create_arguments(&plan.launch.spec)
            .expect("the planned mounts are carried exactly");
        let at = args
            .iter()
            .position(|arg| arg == "--user")
            .unwrap_or_else(|| panic!("{:?}: no --user in {args:?}", request.role));
        assert_eq!(
            args.get(at + 1),
            Some(&format!("{}:{}", expected.uid, expected.gid)),
            "{:?}: {args:?}",
            request.role
        );
        assert!(
            at < args
                .iter()
                .position(|arg| *arg == plan.launch.spec.image_id)
                .expect("the image id"),
            "{:?}: --user is an option of `docker create`, before the image: {args:?}",
            request.role
        );
        roles.insert(request.role.label());
    }
    assert_eq!(
        roles.len(),
        5,
        "both probes, the worker, the gate and the reviewer: {roles:?}"
    );
}

#[cfg(not(unix))]
#[test]
fn every_container_keeps_the_images_user_off_unix() {
    let fixture = Fixture::new("container-user", true);
    let runner = fixture.runner();
    for request in requests(&fixture.task_a) {
        let plan = runner.plan(&request).expect("plans");
        assert_eq!(plan.launch.spec.user, None, "{:?}", request.role);
        assert!(
            !crate::runner::container::docker_create_arguments(&plan.launch.spec)
                .expect("the planned mounts are carried exactly")
                .contains(&"--user".to_owned()),
            "{:?}",
            request.role
        );
    }
}

const PROMPT_MARKER: &str = "PROMPT-MARKER-91f3";

fn prompt(tag: &str, bytes: usize) -> Vec<u8> {
    let mut payload = format!("{PROMPT_MARKER} {tag}\n").into_bytes();
    let mut state: u32 = 0x9e37_79b9;
    while payload.len() < bytes {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        payload.push(state.to_le_bytes()[0]);
    }
    payload
}

#[test]
fn a_commands_stdin_reaches_its_own_container_exactly_and_through_nothing_else() {
    let fixture = Fixture::new("stdin-delivery", true);
    let runner = fixture.runner();
    let claude = AgentId::new("claude-code");
    let ten = Duration::from_secs(10);
    let worker_prompt = prompt("worker", 200 * 1024);
    let reviewer_prompt = prompt("reviewer", 3 * 1024);
    assert_ne!(worker_prompt, reviewer_prompt);
    let worker = worker_request(
        ShellKind::Sh.spec("cat").stdin(worker_prompt.clone()),
        fixture.task_a.clone(),
        claude.clone(),
        ten,
        worker_id(0),
    );
    let review = review_request(
        ShellKind::Sh.spec("cat").stdin(reviewer_prompt.clone()),
        fixture.task_a.clone(),
        claude,
        ten,
        review_pass_id(),
    );
    for (request, payload) in [(&worker, &worker_prompt), (&review, &reviewer_prompt)] {
        let plan = runner.plan(request).expect("plans");
        assert!(
            plan.launch.spec.open_stdin,
            "{:?}: a command with input is created with stdin open",
            request.role
        );
        let spec = &plan.launch.spec;
        let carried: Vec<String> = spec
            .command
            .iter()
            .cloned()
            .chain(spec.env.iter().map(|(key, value)| format!("{key}={value}")))
            .chain(
                spec.labels
                    .iter()
                    .map(|(key, value)| format!("{key}={value}")),
            )
            .chain(spec.mounts.iter().map(|mount| format!("{mount:?}")))
            .chain(
                crate::runner::container::docker_create_arguments(spec)
                    .expect("the planned mounts are carried exactly"),
            )
            .collect();
        assert!(
            !carried.iter().any(|text| text.contains(PROMPT_MARKER)),
            "{:?}: the prompt reached argv, the environment, a label or a mount",
            request.role
        );
        runner.run_blocking(request).expect("runs");
        assert_eq!(
            fixture
                .runtime
                .fake()
                .stdin_received(plan.launch.name.as_str())
                .as_ref(),
            Some(payload),
            "{:?}: its own payload, byte for byte",
            request.role
        );
    }

    for request in requests(&fixture.task_b)
        .into_iter()
        .filter(|request| matches!(request.role, ExecutionRole::Gate | ExecutionRole::Probe(_)))
    {
        let plan = runner.plan(&request).expect("plans");
        assert!(
            !plan.launch.spec.open_stdin,
            "{:?}: a command with no input keeps the closed stdin it had",
            request.role
        );
        let _ = runner.run_blocking(&request);
        assert_eq!(
            fixture
                .runtime
                .fake()
                .stdin_received(plan.launch.name.as_str()),
            None,
            "{:?}",
            request.role
        );
    }
}

#[test]
fn a_workspace_that_contains_a_withheld_path_is_refused_before_any_effect() {
    let fixture = Fixture::new("hostile-ws", true);
    let runner = fixture.runner();
    for (tag, workspace, expected) in [
        (
            "the repository root",
            fixture.repo.clone(),
            vec![Withheld::PublicLog, Withheld::AuthoritativeGit],
        ),
        (
            "the private root",
            fixture.private_root.clone(),
            vec![Withheld::PrivateArtifacts, Withheld::SiblingWorktree],
        ),
        (
            "the filesystem root",
            fixture
                .repo
                .ancestors()
                .last()
                .expect("every path has a root")
                .to_path_buf(),
            Withheld::ALL.to_vec(),
        ),
    ] {
        let request = worker_request(
            ShellKind::Sh.spec("exit 0"),
            workspace,
            AgentId::new("claude-code"),
            Duration::from_secs(10),
            worker_id(0),
        );
        let refusal = runner
            .plan(&request)
            .expect_err("a workspace containing a withheld path is refused");
        let message = refusal.to_string();
        for category in expected {
            assert!(
                message.contains(category.passage()),
                "{tag}: the refusal does not name {category:?}: {message}"
            );
        }
    }
    assert!(
        list_intents(&fixture.private_root)
            .expect("scan")
            .is_empty()
    );
    assert!(fixture.runtime.fake().container_names().is_empty());
}

#[test]
fn only_the_reviewer_receives_a_read_only_worktree() {
    let fixture = Fixture::new("ro-review", true);
    let runner = fixture.runner();
    let mut read_only = Vec::new();
    let mut writable = Vec::new();
    let mut without = Vec::new();
    for request in requests(&fixture.task_a) {
        let plan = runner.plan(&request).expect("plans");
        match target_of(plan.mounts(), "/upstroke/workspace") {
            Some(mount) if mount.read_only() => read_only.push(request.role.label()),
            Some(_) => writable.push(request.role.label()),
            None => without.push(request.role.label()),
        }
    }
    assert_eq!(read_only, vec!["review".to_owned()], "{read_only:?}");
    assert_eq!(
        writable,
        vec!["implement".to_owned(), "gate".to_owned()],
        "{writable:?}"
    );
    assert_eq!(
        without,
        vec!["probe(claude-code)".to_owned(), "probe(shell)".to_owned()],
        "{without:?}"
    );
    assert_eq!(read_only.len() + writable.len() + without.len(), 5);
}

#[cfg(unix)]
#[test]
fn every_role_and_probe_gets_the_recorded_mounts_and_review_and_probes_get_them_read_only() {
    let fixture = Fixture::new("configured-mounts", true);
    let (policy, shared, tools) = fixture.mounted_policy();
    let mounted = fixture.runner_over(policy);
    let plain = fixture.runner();
    let mut forced = Vec::new();
    let mut as_recorded = Vec::new();
    for request in requests(&fixture.task_a) {
        let role = request.role.label();
        let plan = mounted.plan(&request).expect("plans");
        assert_eq!(
            target_of(plan.mounts(), "/data"),
            Some(&Mount::Path {
                source: shared.clone(),
                target: "/data".to_owned(),
                read_only: true,
            }),
            "{role}: the recorded read-only mount"
        );
        let tools_mount = target_of(plan.mounts(), "/opt/tools").expect("the read-write mount");
        assert_eq!(
            tools_mount,
            &Mount::Path {
                source: tools.clone(),
                target: "/opt/tools".to_owned(),
                read_only: forces_configured_mounts_read_only(&request.role),
            },
            "{role}: the recorded read-write mount"
        );
        if tools_mount.read_only() {
            forced.push(role.clone());
        } else {
            as_recorded.push(role.clone());
        }
        let own: Vec<&Mount> = plan
            .mounts()
            .iter()
            .filter(|mount| !matches!(mount.target(), "/data" | "/opt/tools"))
            .collect();
        let unmounted = plain.plan(&request).expect("plans");
        assert_eq!(
            own,
            unmounted.mounts().iter().collect::<Vec<_>>(),
            "{role}: a configured mount changed the role's own mounts"
        );
        assert!(
            fixture.confinement().violations(plan.mounts()).is_empty(),
            "{role}: {:?}",
            fixture.confinement().violations(plan.mounts())
        );
    }
    assert_eq!(
        forced,
        vec![
            "probe(claude-code)".to_owned(),
            "probe(shell)".to_owned(),
            "review".to_owned()
        ]
    );
    assert_eq!(
        as_recorded,
        vec!["implement".to_owned(), "gate".to_owned()],
        "a gate is given an explicitly read-write mount as the operator recorded it"
    );
}

#[cfg(unix)]
#[test]
fn the_docker_argument_names_exactly_the_recorded_mount() {
    let fixture = Fixture::new("mount-argument", true);
    let (policy, shared, tools) = fixture.mounted_policy();
    let runner = fixture.runner_over(policy);
    for request in requests(&fixture.task_a) {
        let plan = runner.plan(&request).expect("plans");
        let args = crate::runner::container::docker_create_arguments(&plan.launch.spec)
            .expect("every planned mount is carried exactly");
        let options: Vec<&str> = args
            .windows(2)
            .filter(|pair| pair[0] == "--mount")
            .map(|pair| pair[1].as_str())
            .collect();
        let read_only = forces_configured_mounts_read_only(&request.role);
        for (source, target, readonly) in
            [(&shared, "/data", true), (&tools, "/opt/tools", read_only)]
        {
            let expected = format!(
                "type=bind,source={},target={target}{}",
                source.to_str().expect("utf-8"),
                if readonly { ",readonly" } else { "" }
            );
            assert_eq!(
                options
                    .iter()
                    .filter(|option| option.contains(&format!("target={target}")))
                    .copied()
                    .collect::<Vec<_>>(),
                vec![expected.as_str()],
                "{}: the argument does not denote the recorded mount",
                request.role.label()
            );
        }
        assert_eq!(
            options.len(),
            plan.mounts().len(),
            "an argument for every mount and no other"
        );
    }
}

#[cfg(unix)]
#[test]
fn a_recorded_mount_that_moved_or_reaches_a_withheld_path_refuses_the_plan_before_any_effect() {
    let fixture = Fixture::new("mount-recheck", true);
    let (policy, shared, _tools) = fixture.mounted_policy();
    let runner = fixture.runner_over(policy.clone());
    let request = gate_request(
        ShellKind::Sh.spec("exit 0"),
        fixture.task_a.clone(),
        Duration::from_secs(10),
        gate_id(0),
    );
    runner.plan(&request).expect("the control plans");

    let away = fixture.root.join("operator-shared.away");
    std::fs::rename(&shared, &away).expect("move the recorded source");
    let refusal = runner
        .plan(&request)
        .expect_err("a recorded source that is gone refuses the plan");
    assert!(refusal.to_string().contains("is gone"), "{refusal}");
    let outcome = runner.run_blocking(&request);
    assert!(
        outcome.is_err(),
        "the invocation ran without its recorded mount"
    );
    std::fs::rename(&away, &shared).expect("put it back");
    runner
        .plan(&request)
        .expect("the restored source plans again");

    // Recorded as selection records a source (`resolve_source`): canonically.
    // The fixture's own spelling of the private root can run through a link,
    // macOS's `/var` (CI run 41529099549), and the plan's recheck refuses such
    // a record as rebound before any exclusion is consulted, which
    // `a_record_spelled_through_a_link_is_refused_as_rebound_before_any_exclusion`
    // holds on every Unix host.
    let mut reaching = policy;
    let (source, source_identity) = recorded_directory(&fixture.private_root);
    reaching.mounts[0].source = source;
    reaching.mounts[0].source_identity = source_identity;
    let refusal = fixture
        .runner_over(reaching)
        .plan(&request)
        .expect_err("a recorded mount of the private root refuses the plan");
    assert!(
        refusal.to_string().contains("the private root"),
        "{refusal}"
    );

    assert!(
        list_intents(&fixture.private_root)
            .expect("scan")
            .is_empty(),
        "a refused plan wrote an intent"
    );
    assert!(fixture.runtime.fake().container_names().is_empty());
}

#[cfg(unix)]
#[test]
fn a_recorded_policy_whose_targets_a_fresh_run_refuses_starts_no_container() {
    use crate::error::ProcessFate;
    use crate::runner::container::runtime::RuntimeOp;

    let fixture = Fixture::new("recorded-target-plan", true);
    let (policy, _shared, _tools) = fixture.mounted_policy();
    let request = gate_request(
        ShellKind::Sh.spec("exit 0"),
        fixture.task_a.clone(),
        Duration::from_secs(10),
        gate_id(0),
    );
    fixture
        .runner_over(fixtures::with_recorded_targets(&policy, &["/data"]))
        .plan(&request)
        .expect("the control: the same record at a target a fresh run accepts plans");

    for (targets, needle) in fixtures::REFUSED_RECORDED_TARGETS {
        let planted = fixtures::with_recorded_targets(&policy, targets);
        assert_eq!(
            planted.completeness(),
            Ok(()),
            "{targets:?}: the record is self-consistent, so only the target rule can refuse it"
        );
        let refused = fixture
            .runner_over(planted)
            .run_blocking(&request)
            .expect_err("a recorded target a fresh run refuses refuses the invocation");
        assert_eq!(
            refused.fate,
            ProcessFate::NeverStarted,
            "{targets:?}: {refused}"
        );
        let message = refused.to_string();
        assert!(
            message.contains("is refused for this container") && message.contains(needle),
            "{targets:?}: the refusal names the target rule: {message}"
        );
    }
    assert!(
        !fixture
            .runtime
            .fake()
            .journal()
            .iter()
            .any(|entry| entry.op == RuntimeOp::Create),
        "a refused record created a container: {:?}",
        fixture.runtime.fake().journal()
    );
    assert!(
        list_intents(&fixture.private_root)
            .expect("scan")
            .is_empty(),
        "a refused plan wrote an intent"
    );
    assert!(fixture.runtime.fake().container_names().is_empty());
}

/// A directory as selection records a mount source (`resolve_source`): its
/// canonical spelling, and the identity of the object that spelling names.
#[cfg(unix)]
fn recorded_directory(path: &Path) -> (String, crate::topology::events::MountSourceIdentity) {
    use std::os::unix::fs::MetadataExt as _;
    let canonical = std::fs::canonicalize(path).expect("the directory resolves");
    let metadata = std::fs::metadata(&canonical).expect("the directory");
    (
        canonical.to_str().expect("utf-8").to_owned(),
        crate::topology::events::MountSourceIdentity::Unix {
            kind: crate::topology::events::MountSourceKind::Directory,
            device: metadata.dev(),
            inode: metadata.ino(),
        },
    )
}

/// The order of the plan's two guards on a recorded mount: a record whose
/// source is spelled through a link is refused by the recheck as rebound before
/// any exclusion is consulted, and the canonical record of the same directory
/// reaches the exclusion. macOS's `/var` link gave the private-root case above
/// such a spelling; a link reproduces it on every Unix host.
#[cfg(unix)]
#[test]
fn a_record_spelled_through_a_link_is_refused_as_rebound_before_any_exclusion() {
    let fixture = Fixture::new("mount-link-order", true);
    let (policy, _shared, _tools) = fixture.mounted_policy();
    let request = gate_request(
        ShellKind::Sh.spec("exit 0"),
        fixture.task_a.clone(),
        Duration::from_secs(10),
        gate_id(0),
    );
    let (canonical, identity) = recorded_directory(&fixture.private_root);
    let link = fixture.root.join("private-link");
    std::os::unix::fs::symlink(&canonical, &link).expect("a link to the private root");
    let through_link = link.to_str().expect("utf-8").to_owned();
    for (spelling, expected) in [
        (through_link, "so a link or a move rebound it"),
        (canonical, "the private root"),
    ] {
        let mut reaching = policy.clone();
        reaching.mounts[0].source = spelling.clone();
        reaching.mounts[0].source_identity = identity.clone();
        let refusal = fixture
            .runner_over(reaching)
            .plan(&request)
            .expect_err("a recorded mount of the private root refuses the plan");
        assert!(
            refusal.to_string().contains(expected),
            "`{spelling}` is refused for another reason than `{expected}`: {refusal}"
        );
    }
    assert!(
        list_intents(&fixture.private_root)
            .expect("scan")
            .is_empty(),
        "a refused plan wrote an intent"
    );
    assert!(fixture.runtime.fake().container_names().is_empty());
}

#[cfg(unix)]
#[test]
fn a_recorded_mount_inside_the_common_directory_the_runner_is_given_refuses_the_plan() {
    let fixture = Fixture::new("mount-common-dir", true);
    let common = fixture.root.join("relocated-common");
    std::fs::create_dir_all(common.join("refs")).expect("a common directory outside the checkout");
    let (mut policy, _shared, _tools) = fixture.mounted_policy();
    let (source, source_identity) = recorded_directory(&common.join("refs"));
    let Some(first) = policy.mounts.first_mut() else {
        panic!("the mounted policy records its mounts: {policy:?}");
    };
    first.source = source;
    first.source_identity = source_identity;
    let request = gate_request(
        ShellKind::Sh.spec("exit 0"),
        fixture.task_a.clone(),
        Duration::from_secs(10),
        gate_id(0),
    );
    fixture
        .runner_over(policy.clone())
        .plan(&request)
        .expect("the control: beside the checkout's own Git the source is an ordinary directory");

    let git_dir = fixture.repo.join(".git");
    let refusal = ContainerRunner::new(
        policy,
        fixture.identity.clone(),
        &fixture.repo,
        RepositoryGit {
            git_dir: &git_dir,
            common_dir: &common,
        },
        image_environment(),
        Box::new(fixture.runtime.clone()),
    )
    .expect("a container policy")
    .plan(&request)
    .expect_err("a recorded source inside the run's common directory refuses the plan");
    let said = refusal.to_string();
    let canonical = std::fs::canonicalize(&common).expect("the common directory resolves");
    assert!(
        said.contains(&format!("lies inside `{}`", canonical.display()))
            && said.contains("authoritative Git"),
        "{said}"
    );
    assert!(
        list_intents(&fixture.private_root)
            .expect("scan")
            .is_empty(),
        "a refused plan wrote an intent"
    );
    assert!(fixture.runtime.fake().container_names().is_empty());
}

#[test]
fn a_configured_mount_that_lies_inside_a_withheld_directory_is_refused_and_a_roles_own_is_not() {
    let tree = repo::scratch("configured-confinement");
    let root = tree.path();
    let repo_dir = root.join("repo");
    let git_dir = repo_dir.join(".git");
    let relocated = root.join("relocated");
    let common = relocated.join("common");
    let identity = RunIdentity {
        private_root: root.join("private"),
        run_id: RUN_ID.to_owned(),
        run_dir: root.join("public"),
        incarnation: INCARNATION_1.to_owned(),
        repo_key: repo_key().to_owned(),
    };
    let confinement = Confinement::of_run(
        &identity,
        &repo_dir,
        RepositoryGit {
            git_dir: &git_dir,
            common_dir: &common,
        },
    );
    let mount = |source: PathBuf| Mount::Path {
        source,
        target: "/data".to_owned(),
        read_only: false,
    };
    for (relation, source, own_rule_refuses) in [
        ("is", common.clone(), true),
        ("contains", relocated.clone(), true),
        ("lies inside", common.join("refs"), false),
    ] {
        let mounted = [mount(source)];
        let found = confinement.configured_violations(&mounted);
        let [only] = found.as_slice() else {
            panic!("`{relation}`: one configured violation, of the common directory: {found:?}");
        };
        assert!(
            only.contains(&format!("{relation} `{}`", common.display()))
                && only.contains(Withheld::AuthoritativeGit.passage()),
            "`{relation}`: {only}"
        );
        assert_eq!(
            !confinement.violations(&mounted).is_empty(),
            own_rule_refuses,
            "`{relation}`: the rule for a role's own mounts is unchanged, one-directional"
        );
    }
    let beside = [mount(root.join("shared"))];
    assert!(
        confinement.configured_violations(&beside).is_empty()
            && confinement.violations(&beside).is_empty(),
        "a directory beside every withheld path is no violation"
    );
}

#[cfg(unix)]
#[test]
fn a_mount_the_runtimes_option_cannot_carry_exactly_refuses_the_plan_before_any_effect() {
    let fixture = Fixture::new("mount-spelling", true);
    let runner = fixture.runner();
    for name in ["comma,in-name", "quote\"in-name", "trailing-space "] {
        let workspace = fixture.root.join(name);
        std::fs::create_dir_all(&workspace).expect("an awkwardly named workspace");
        let request = gate_request(
            ShellKind::Sh.spec("exit 0"),
            workspace,
            Duration::from_secs(10),
            gate_id(0),
        );
        let refusal = runner
            .plan(&request)
            .expect_err("a workspace the mount option cannot spell is refused");
        assert!(
            refusal.to_string().contains("/upstroke/workspace"),
            "{name}: the refusal does not name the mount: {refusal}"
        );
    }
    assert!(
        list_intents(&fixture.private_root)
            .expect("scan")
            .is_empty()
    );
    assert!(fixture.runtime.fake().container_names().is_empty());
}

#[test]
fn the_credential_volume_is_mounted_exactly_when_its_location_is_supplied() {
    let fixture = Fixture::new("creds", true);
    let with_volumes = fixture.runner();
    let without_volumes = ContainerRunner::new(
        RunnerPolicy {
            credential_volumes: None,
            ..container_policy()
        },
        fixture.identity.clone(),
        &fixture.repo,
        fixtures::plain_checkout(&fixture.repo.join(".git")),
        image_environment(),
        Box::new(fixture.runtime.clone()),
    )
    .expect("a container policy with no volumes")
    .with_poll(Duration::ZERO);

    let mut mounted = 0_usize;
    let mut cells = 0_usize;
    let mut volume_names: BTreeSet<String> = BTreeSet::new();
    for (recorded, runner) in [(true, &with_volumes), (false, &without_volumes)] {
        for request in requests(&fixture.task_a) {
            let plan = runner.plan(&request).expect("plans");
            let volume = plan.mounts().iter().find_map(|mount| match mount {
                Mount::Volume { name, target, .. } => Some((name.clone(), target.clone())),
                Mount::Path { .. } | Mount::Tmpfs { .. } => None,
            });
            let key = request.agent.as_ref().and_then(host::credential_location);
            let in_env = key.and_then(|key| {
                plan.env()
                    .iter()
                    .find(|(name, _)| name == key)
                    .map(|(_, value)| value.clone())
                    .filter(|value| !value.is_empty())
            });
            let expected =
                recorded && supplies_credential_location(&request.role) && request.agent.is_some();
            if let Some(key) = key {
                assert!(
                    plan.env().iter().any(|(name, _)| name == key),
                    "{} (recorded: {recorded}): `{key}` is not named at all, so the recorded \
                     image's own value reaches the container",
                    request.role
                );
            }
            assert_eq!(
                volume.is_some(),
                expected,
                "{} (recorded: {recorded}) mounted {volume:?}",
                request.role
            );
            assert_eq!(
                in_env.is_some(),
                volume.is_some(),
                "{}: the mount and the location disagree — {volume:?} vs {in_env:?}",
                request.role
            );
            if let (Some((name, target)), Some(value)) = (volume, in_env) {
                assert_eq!(
                    target, value,
                    "{}: the variable points elsewhere",
                    request.role
                );
                volume_names.insert(name);
                mounted += 1;
            }
            assert_eq!(
                runner
                    .credential_volume_for(&request.role, request.agent.as_ref())
                    .is_some(),
                expected,
                "{}",
                request.role
            );
            cells += 1;
        }
    }
    assert_eq!(cells, 10, "five roles crossed with recorded/absent");
    assert_eq!(mounted, 3, "implement, review and the agent probe, once");

    let mut hostile_cells = 0_usize;
    for request in hostile_bindings(&fixture.task_a) {
        assert!(request.agent.is_some(), "the fixture bound no agent");
        assert!(
            !supplies_credential_location(&request.role),
            "{}: this role does take credentials, so the cell is not hostile",
            request.role
        );
        assert!(
            fixture
                .runtime
                .volume_present("upstroke-creds-claude")
                .expect("reachable"),
            "the volume this role must not receive does not exist"
        );
        let plan = with_volumes.plan(&request).expect("plans");
        assert!(
            !plan
                .mounts()
                .iter()
                .any(|mount| matches!(mount, Mount::Volume { .. })),
            "{}: a role that takes no credentials was handed a credential volume — \
             `host-v1` refuses this shape whatever agent the request names",
            request.role
        );
        assert_eq!(
            plan.env()
                .iter()
                .find(|(key, _)| key == "CLAUDE_CONFIG_DIR")
                .map(|(_, value)| value.as_str()),
            Some(""),
            "{}: and it was told where they live",
            request.role
        );
        assert_eq!(
            with_volumes.credential_volume_for(&request.role, request.agent.as_ref()),
            None,
            "{}",
            request.role
        );
        hostile_cells += 1;
    }
    assert_eq!(
        hostile_cells, 2,
        "a gate and a shell probe, each bound anyway"
    );
    assert_eq!(
        volume_names,
        BTreeSet::from(["upstroke-creds-claude".to_owned()]),
        "the volume is the one the record names for that agent"
    );
}

#[test]
fn every_container_is_created_from_the_recorded_id_even_after_the_reference_moves() {
    let fixture = Fixture::new("recorded-id", true);
    let runner = fixture.runner();
    let request = gate_request(
        ShellKind::Sh.spec("exit 0"),
        fixture.task_a.clone(),
        Duration::from_secs(10),
        gate_id(0),
    );

    let before = runner.plan(&request).expect("plans");
    assert_eq!(before.launch.spec.image_id, IMAGE_ID);

    fixture
        .runtime
        .fake()
        .move_tag(IMAGE_REFERENCE, OTHER_IMAGE_ID);
    assert_eq!(
        fixture
            .runtime
            .image_by_reference(IMAGE_REFERENCE)
            .expect("reachable")
            .expect("present")
            .id,
        OTHER_IMAGE_ID,
        "the fixture did not move the reference"
    );
    let after = runner.plan(&request).expect("plans");
    assert_eq!(
        after.launch.spec.image_id, IMAGE_ID,
        "a moved reference changed what executes (INV-23, DESIGN.md:610)"
    );

    fixture.trace.clear();
    runner.run_blocking(&request).expect("runs");
    assert_eq!(
        fixture
            .trace
            .ops()
            .iter()
            .filter(|op| **op == RuntimeOp::Create)
            .count(),
        1
    );
    assert_eq!(
        fixture
            .trace
            .ops()
            .iter()
            .filter(|op| **op == RuntimeOp::InspectImageByReference)
            .count(),
        0,
        "the runner resolved a reference on the way to creating a container"
    );
}

#[test]
fn a_substituted_reported_image_id_refuses_before_start_in_both_phases() {
    for (phase, expected, build) in [
        (
            "pre-flight",
            ImageIdMismatch::RefusedBeforeStart,
            (|fixture: &Fixture| {
                host::shell_probe_request(ShellKind::Sh, fixture.task_a.clone(), shell_probe_id())
            }) as fn(&Fixture) -> RunnerRequest,
        ),
        (
            "mid-run worker",
            ImageIdMismatch::SpawnFailureOutage,
            |fixture: &Fixture| {
                worker_request(
                    ShellKind::Sh.spec("exit 0"),
                    fixture.task_a.clone(),
                    AgentId::new("claude-code"),
                    Duration::from_secs(10),
                    worker_id(0),
                )
            },
        ),
        (
            "mid-run sequence gate",
            ImageIdMismatch::SpawnFailureOutage,
            |fixture: &Fixture| {
                let mut request = worker_request(
                    ShellKind::Sh.spec("exit 0"),
                    fixture.task_a.clone(),
                    AgentId::new("claude-code"),
                    Duration::from_secs(10),
                    worker_id(0),
                );
                request.invocation = InvocationId::sequence(
                    crate::topology::events::SequenceId(7),
                    crate::runner::invocation::SequenceRole::Gate(0),
                    0,
                );
                request
            },
        ),
    ] {
        let fixture = Fixture::new(&format!("mismatch-{phase}"), true);
        let runner = fixture.runner();
        let request = build(&fixture);
        let name = ContainerName::new(repo_key(), RUN_ID, INCARNATION_1, &request.invocation)
            .expect("a name");
        fixture
            .runtime
            .fake()
            .substitute_reported_image_id(name.as_str(), OTHER_IMAGE_ID);

        let refusal = runner.run_blocking(&request).expect_err("{phase}: refused");
        let message = refusal.to_string();
        assert!(message.contains(IMAGE_ID), "{phase}: {message}");
        assert!(message.contains(OTHER_IMAGE_ID), "{phase}: {message}");
        assert!(message.contains("INV-23"), "{phase}: {message}");

        assert_eq!(
            ImageIdMismatch::of(&request.invocation),
            expected,
            "{phase}: the phase was classified from the invocation wrongly"
        );
        match expected {
            ImageIdMismatch::RefusedBeforeStart => assert!(
                matches!(*refusal.source, UpstrokeError::Refused { .. }),
                "{phase}: a pre-flight mismatch must be a refusal before any spend: \
                 {refusal:?}"
            ),
            ImageIdMismatch::SpawnFailureOutage => assert!(
                matches!(*refusal.source, UpstrokeError::Agent { .. }),
                "{phase}: a mid-run mismatch reached the caller as the same generic refusal a \
                 pre-flight one does, so the RunnerSpawnFailure outage settlement the \
                 contract requires is unreachable: {refusal:?}"
            ),
        }
        assert_eq!(
            refusal.fate,
            crate::error::ProcessFate::NeverStarted,
            "{phase}: a created container that never started and was removed is no process"
        );
        match expected {
            ImageIdMismatch::RefusedBeforeStart | ImageIdMismatch::SpawnFailureOutage => {}
        }

        assert!(
            fixture.trace.position_starting("site:Start").is_none(),
            "{phase}: the container was started: {:#?}",
            fixture.trace.rendered()
        );
        assert!(!fixture.trace.ops().contains(&RuntimeOp::Start), "{phase}");

        assert!(
            fixture.runtime.fake().container_names().is_empty(),
            "{phase}"
        );
        assert!(
            list_intents(&fixture.private_root)
                .expect("scan")
                .is_empty(),
            "{phase}"
        );
        assert!(
            !fixture
                .private_root
                .join("views")
                .join(name.as_str())
                .exists()
        );
    }
}

#[test]
fn the_image_mismatch_phase_is_read_from_the_invocation() {
    let cells: [(&str, InvocationId, ImageIdMismatch); 5] = [
        (
            "the RunnerPreflight shell probe",
            shell_probe_id(),
            ImageIdMismatch::RefusedBeforeStart,
        ),
        (
            "a RunnerPreflight agent probe",
            agent_probe_id("claude-code"),
            ImageIdMismatch::RefusedBeforeStart,
        ),
        (
            "an attempt's worker",
            worker_id(0),
            ImageIdMismatch::SpawnFailureOutage,
        ),
        (
            "an attempt's gate",
            gate_id(1),
            ImageIdMismatch::SpawnFailureOutage,
        ),
        (
            "an integration sequence's gate",
            InvocationId::sequence(
                crate::topology::events::SequenceId(7),
                crate::runner::invocation::SequenceRole::Gate(0),
                0,
            ),
            ImageIdMismatch::SpawnFailureOutage,
        ),
    ];
    assert_eq!(
        cells
            .iter()
            .map(|(_, invocation, _)| invocation.render())
            .collect::<BTreeSet<_>>()
            .len(),
        cells.len(),
        "five distinct invocation identities"
    );

    let mut seen = BTreeSet::new();
    for (what, invocation, expected) in &cells {
        let got = ImageIdMismatch::of(invocation);
        assert_eq!(got, *expected, "{what}");
        seen.insert(got);
        let name =
            ContainerName::new(repo_key(), RUN_ID, INCARNATION_1, invocation).expect("a name");
        let error = got.error(&name, OTHER_IMAGE_ID, IMAGE_ID);
        assert_eq!(
            matches!(error, UpstrokeError::Refused { .. }),
            *expected == ImageIdMismatch::RefusedBeforeStart,
            "{what}: {error:?}"
        );
    }
    assert_eq!(
        seen.into_iter().collect::<Vec<_>>(),
        ImageIdMismatch::ALL.to_vec(),
        "the grid must reach both outcomes the contract names"
    );
    assert_eq!(
        ImageIdMismatch::ALL
            .iter()
            .map(|outcome| outcome.name())
            .collect::<BTreeSet<_>>()
            .len(),
        ImageIdMismatch::ALL.len()
    );
}

#[test]
fn a_policy_that_is_not_a_container_policy_is_refused_at_construction() {
    let fixture = Fixture::new("policy", true);
    let cases: Vec<(&str, RunnerPolicy)> = vec![
        ("a host policy", crate::runner::policy::host_policy()),
        (
            "a container policy with no image",
            RunnerPolicy {
                image: None,
                ..container_policy()
            },
        ),
        (
            "a container policy with an empty image id",
            RunnerPolicy {
                image: Some(ImageIdentity {
                    reference: IMAGE_REFERENCE.to_owned(),
                    id: String::new(),
                    digest: None,
                }),
                ..container_policy()
            },
        ),
        (
            "a container kind under the host contract",
            RunnerPolicy {
                policy: RunnerContract::HostV1,
                ..container_policy()
            },
        ),
    ];
    for (tag, policy) in cases {
        ContainerRunner::new(
            policy,
            fixture.identity.clone(),
            &fixture.repo,
            fixtures::plain_checkout(&fixture.repo.join(".git")),
            image_environment(),
            Box::new(fixture.runtime.clone()),
        )
        .err()
        .unwrap_or_else(|| panic!("{tag} was accepted"));
    }
    let runner = ContainerRunner::new(
        container_policy(),
        fixture.identity.clone(),
        &fixture.repo,
        fixtures::plain_checkout(&fixture.repo.join(".git")),
        image_environment(),
        Box::new(fixture.runtime.clone()),
    )
    .expect("accepted");
    assert_eq!(
        runner.policy_digest(),
        crate::runner::policy::runner_policy_sha256(&container_policy())
    );
    assert!(runner.policy_digest().starts_with("sha256:"));
}

#[test]
fn the_shell_probe_runs_through_this_runner_as_a_registered_container_invocation() {
    let fixture = Fixture::new("shell-probe", true);
    let runner = fixture.runner();
    let invocation = shell_probe_id();
    let name = ContainerName::new(repo_key(), RUN_ID, INCARNATION_1, &invocation).expect("a name");

    host::run_shell_probe(
        &runner,
        ShellKind::Sh,
        fixture.task_a.clone(),
        invocation.clone(),
    )
    .expect("the recorded shell runs inside the recorded image");

    let rendered = fixture.trace.rendered();
    let at = |needle: &str| {
        fixture
            .trace
            .position_starting(needle)
            .unwrap_or_else(|| panic!("`{needle}` is not in {rendered:#?}"))
    };
    assert!(at("durable:synced") < at("rt:create"), "{rendered:#?}");
    assert!(at("site:MountGitView") < at("rt:create"), "{rendered:#?}");
    assert!(at("site:MountGitView") < at("site:Start"), "{rendered:#?}");
    assert!(at("rt:create") < at("site:Start"), "{rendered:#?}");
    assert!(at("site:Start") < at("site:Stop"), "{rendered:#?}");

    assert!(invocation.probe_target().is_some());
    assert_eq!(invocation.render(), "p.shell.o0");

    assert!(
        list_intents(&fixture.private_root)
            .expect("scan")
            .is_empty()
    );
    assert!(fixture.runtime.fake().container_names().is_empty());
    assert!(
        !fixture
            .private_root
            .join("views")
            .join(name.as_str())
            .exists()
    );
}

#[test]
fn failing_preflight_probe_on_resume_refuses_before_recovery_event_and_reclaims_probe_containers() {
    for (tag, invocation, exit, stderr) in [
        (
            "shell",
            shell_probe_id(),
            Some(127),
            "sh: exec: not found".to_owned(),
        ),
        (
            "agent",
            agent_probe_id("claude-code"),
            Some(1),
            "claude: command not found".to_owned(),
        ),
    ] {
        let fixture = Fixture::new(&format!("failing-{tag}"), true);
        fixture.runtime.scripts(ContainerExecution {
            exit_code: exit,
            stdout: Vec::new(),
            stderr: stderr.clone().into_bytes(),
            output_limited: false,
        });
        let runner = fixture.runner();
        let name =
            ContainerName::new(repo_key(), RUN_ID, INCARNATION_1, &invocation).expect("a name");

        if tag == "shell" {
            let refusal = host::run_shell_probe(
                &runner,
                ShellKind::Sh,
                fixture.task_a.clone(),
                invocation.clone(),
            )
            .expect_err("a shell that fails inside the image refuses");
            assert!(refusal.to_string().contains("sh"), "{refusal}");
            assert!(refusal.to_string().contains("127"), "{refusal}");
        } else {
            let adapter: &dyn crate::agent::AgentAdapter = &crate::agent::claude::ClaudeCodeAdapter;
            let refusal = adapter
                .probe(&runner)
                .expect_err("an agent CLI that fails inside the image refuses");
            let message = refusal.to_string();
            assert!(
                message.contains("claude"),
                "{tag}: the refusal does not name the CLI: {message}"
            );
            assert!(
                message.contains("--version"),
                "{tag}: the refusal came from somewhere other than the CLI's own exit \
                 status: {message}"
            );
            assert!(
                message.contains(&format!("{exit:?}")),
                "{tag}: the refusal does not carry the exit status: {message}"
            );
            let request = crate::agent::probe_request(
                "claude-code",
                ShellKind::Sh.spec("claude --version"),
                0,
                Duration::from_secs(10),
            )
            .expect("an agent probe request");
            let output = runner
                .run_blocking(&request)
                .expect("the spawn itself succeeds");
            assert_eq!(output.code, exit, "the CLI failed inside the image");
            assert!(output.stderr.contains("command not found"));
        }
        assert!(
            fixture.trace.ops().contains(&RuntimeOp::Start),
            "{tag}: nothing was spawned, so nothing observed the failure"
        );

        assert!(
            fixture.runtime.fake().container_names().is_empty(),
            "{tag}: a probe container survived"
        );
        assert!(
            list_intents(&fixture.private_root)
                .expect("scan")
                .is_empty(),
            "{tag}: a probe intent survived"
        );
        assert!(
            !fixture
                .private_root
                .join("views")
                .join(name.as_str())
                .exists()
        );
        assert!(fixture.paths.events().exists());
    }
}

#[test]
fn two_incarnations_of_one_probe_are_two_container_invocations() {
    let fixture = Fixture::new("epochs", true);
    let mut names = BTreeSet::new();
    let mut intents = BTreeSet::new();
    for incarnation in [INCARNATION_1, INCARNATION_2] {
        for invocation in [shell_probe_id(), agent_probe_id("claude-code")] {
            let identity = RunIdentity {
                incarnation: incarnation.to_owned(),
                ..fixture.identity.clone()
            };
            let runner = fixture.runner_with(identity);
            let request = if invocation.render().starts_with("p.shell") {
                host::shell_probe_request(ShellKind::Sh, fixture.task_a.clone(), invocation.clone())
            } else {
                crate::agent::probe_request(
                    "claude-code",
                    ShellKind::Sh.spec("claude --version"),
                    0,
                    Duration::from_secs(10),
                )
                .expect("an agent probe request")
            };
            let plan = runner.plan(&request).expect("plans");
            names.insert(plan.launch.name.as_str().to_owned());
            intents.insert(plan.launch.name.intent_path(&fixture.private_root));
            assert_eq!(
                plan.launch.intent.incarnation, incarnation,
                "the record does not carry the incarnation it was built for"
            );
        }
    }
    assert_eq!(shell_probe_id().render(), shell_probe_id().render());
    assert_eq!(names.len(), 4, "{names:?}");
    assert_eq!(intents.len(), 4, "{intents:?}");
}

#[test]
fn probe_and_execution_compose_through_one_code_path() {
    let source = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("runner")
            .join("container")
            .join("exec.rs"),
    )
    .expect("read this module");
    let production = crate::effects::blank_comments(&crate::effects::production_region(&source));
    assert_eq!(
        production.matches("self.environment.compose(").count(),
        1,
        "the environment is composed in more than one place"
    );
    assert_eq!(
        production.matches("self.plan(").count(),
        1,
        "a request becomes a plan in more than one place"
    );
    assert_eq!(
        production.matches("self.mounts(").count(),
        1,
        "the mount set is built in more than one place"
    );

    let fixture = Fixture::new("parity-probe", true);
    let runner = fixture.runner();
    let overlay = (
        "CLAUDE_CODE_MAX_OUTPUT_TOKENS".to_owned(),
        "8000".to_owned(),
    );
    let pairs: Vec<(&str, RunnerRequest, RunnerRequest)> = vec![
        (
            "the agent probe and the worker it certifies",
            crate::agent::probe_request(
                "claude-code",
                ShellKind::Sh
                    .spec("claude --version")
                    .env(&overlay.0, &overlay.1),
                0,
                Duration::from_secs(10),
            )
            .expect("an agent probe request"),
            worker_request(
                ShellKind::Sh.spec("claude -p").env(&overlay.0, &overlay.1),
                fixture.task_a.clone(),
                AgentId::new("claude-code"),
                Duration::from_secs(10),
                worker_id(0),
            ),
        ),
        (
            "the shell probe and the gate it certifies",
            host::shell_probe_request(ShellKind::Sh, fixture.task_a.clone(), shell_probe_id()),
            gate_request(
                ShellKind::Sh.spec("cargo test"),
                fixture.task_a.clone(),
                Duration::from_secs(10),
                gate_id(0),
            ),
        ),
    ];
    for (tag, probe, execution) in pairs {
        let probed = runner.plan(&probe).expect("plans");
        let executed = runner.plan(&execution).expect("plans");
        assert_eq!(
            probed.launch.spec.image_id, executed.launch.spec.image_id,
            "{tag}: two boundaries"
        );
        let composed = |plan: &InvocationPlan| -> BTreeMap<String, String> {
            plan.env().iter().cloned().collect()
        };
        assert_eq!(
            composed(&probed),
            composed(&executed),
            "{tag}: pre-flight certifies an environment the attempt does not run in"
        );
        let probe_targets: BTreeSet<&str> = probed.mounts().iter().map(Mount::target).collect();
        let execution_targets: BTreeSet<&str> =
            executed.mounts().iter().map(Mount::target).collect();
        assert!(
            probe_targets.is_subset(&execution_targets),
            "{tag}: {probe_targets:?} vs {execution_targets:?}"
        );
        let difference: BTreeSet<&&str> = execution_targets.difference(&probe_targets).collect();
        assert_eq!(
            difference,
            BTreeSet::from([
                &"/upstroke/workspace",
                &"/upstroke/gitview",
                &"/upstroke/gitobjects",
                &"/upstroke/workspace/.git",
            ]),
            "{tag}: the probe and the execution differ by something other than the worktree"
        );
    }
}

#[test]
fn host_and_container_compose_the_same_environment_for_every_role() {
    let base: Vec<(String, String)> = [
        ("PATH", "/usr/local/bin:/usr/bin:/bin"),
        ("HOME", "/root"),
        ("LANG", "C.UTF-8"),
        ("CLAUDE_CONFIG_DIR", "/host/claude"),
        ("UPSTROKE_SHARED", "shared"),
    ]
    .into_iter()
    .map(|(key, value)| (key.to_owned(), value.to_owned()))
    .collect();
    let host_base: Vec<(std::ffi::OsString, std::ffi::OsString)> = base
        .iter()
        .map(|(key, value)| (key.into(), value.into()))
        .collect();
    let host_env = HostEnvironment::with_base(host_base, super::super::env::CONTAINER_KEY_CASE);
    let container_env =
        ContainerEnvironment::with_base(base.clone(), super::super::env::CONTAINER_KEY_CASE);
    let volumes: BTreeMap<String, String> = VOLUMES
        .iter()
        .map(|(agent, volume)| ((*agent).to_owned(), (*volume).to_owned()))
        .collect();
    let layout = BoundaryLayout::new();
    let overlay = vec![
        ("UPSTROKE_OVERLAY".to_owned(), "1".to_owned()),
        ("LANG".to_owned(), "en_GB.UTF-8".to_owned()),
    ];

    let mut supplied_locations = 0_usize;
    let mut rows = 0_usize;
    for role in ExecutionRole::all() {
        let agent = match &role {
            ExecutionRole::Probe(ProbeTarget::Agent(agent)) => Some(agent.clone()),
            ExecutionRole::Implement | ExecutionRole::Review => Some(AgentId::new("claude-code")),
            ExecutionRole::Gate | ExecutionRole::Probe(ProbeTarget::Shell) => None,
        };
        let scope = RoleScope {
            role: &role,
            agent: agent.as_ref(),
            volumes: &volumes,
            layout: &layout,
        };
        let mut host_composed: BTreeMap<String, String> = host_env
            .compose(&role, agent.as_ref(), &overlay)
            .expect("the host composes")
            .into_iter()
            .map(|(key, value)| {
                (
                    key.to_string_lossy().into_owned(),
                    value.to_string_lossy().into_owned(),
                )
            })
            .collect();
        // The one key the two boundaries compose by different mechanisms since PR12's Q2:
        // this host environment's default object graph (`Recorded`) disables replacements
        // with the process-wide variable, and the container states the same policy in the
        // role's projected view config instead, so that other repositories keep theirs.
        // Named and checked here rather than compared.
        assert_eq!(
            host_composed
                .remove(crate::workspace_manager::NO_REPLACEMENT_OBJECTS.0)
                .as_deref(),
            Some(crate::workspace_manager::NO_REPLACEMENT_OBJECTS.1),
            "{role}: the host's default object graph composes the process-wide variable"
        );
        let container_composed: BTreeMap<String, String> = container_env
            .compose(&scope, &overlay)
            .expect("the container composes")
            .into_iter()
            .collect();
        assert!(
            !container_composed.contains_key(crate::workspace_manager::NO_REPLACEMENT_OBJECTS.0),
            "{role}: the container composed a process-wide replacement setting"
        );

        let withheld: BTreeMap<String, String> = container_env
            .withheld_credential_locations(&scope)
            .into_iter()
            .collect();
        for (key, value) in &withheld {
            assert_eq!(value, "", "{role}: `{key}` is withheld with a value");
            assert!(
                !host_composed.contains_key(key),
                "{role}: the host supplies `{key}`, so the container must not withhold it"
            );
            assert_eq!(
                container_composed.get(key).map(String::as_str),
                Some(""),
                "{role}: `{key}` is withheld and is not in the composed vector, so the \
                 image's own value reaches the container"
            );
        }
        let container_supplied: Vec<&String> = container_composed
            .keys()
            .filter(|key| !withheld.contains_key(*key))
            .collect();
        assert_eq!(
            host_composed.keys().collect::<Vec<_>>(),
            container_supplied,
            "{role}: the two runners composed different key sets"
        );
        let location = agent.as_ref().and_then(host::credential_location);
        for (key, host_value) in &host_composed {
            let container_value = &container_composed[key];
            if Some(key.as_str()) == location {
                assert_eq!(host_value, "/host/claude");
                assert_eq!(
                    container_value,
                    &layout.credentials(agent.as_ref().expect("an agent")),
                    "{role}: the container named a location that is not its own"
                );
                supplied_locations += 1;
            } else {
                assert_eq!(
                    host_value, container_value,
                    "{role}: `{key}` composed differently"
                );
            }
        }
        for reserved in host::reserved_keys() {
            let bad = vec![(reserved.to_owned(), "x".to_owned())];
            host_env
                .compose(&role, agent.as_ref(), &bad)
                .expect_err("the host refuses");
            container_env
                .compose(&scope, &bad)
                .expect_err("and so does the container");
        }
        rows += 1;
    }
    assert_eq!(rows, 5, "all five roles, both probe targets");
    assert_eq!(
        supplied_locations, 3,
        "implement, review and the agent probe get a location at both boundaries"
    );
    assert!(base.iter().any(|(key, _)| key == "CLAUDE_CONFIG_DIR"));
}

#[test]
fn a_completed_invocation_releases_in_the_contracts_order_and_reports_the_result() {
    let fixture = Fixture::new("complete", true);
    fixture.runtime.scripts(ContainerExecution {
        exit_code: Some(3),
        stdout: b"the work is done\n".to_vec(),
        stderr: b"a warning\n".to_vec(),
        output_limited: false,
    });
    let runner = fixture.runner();
    let request = worker_request(
        ShellKind::Sh.spec("exit 3"),
        fixture.task_a.clone(),
        AgentId::new("claude-code"),
        Duration::from_secs(10),
        worker_id(0),
    );
    let name =
        ContainerName::new(repo_key(), RUN_ID, INCARNATION_1, &request.invocation).expect("a name");

    let output = runner
        .run_blocking(&request)
        .expect("a non-zero exit is not an error");
    assert_eq!(output.code, Some(3), "a non-zero exit is a ProcessOutput");
    assert_eq!(output.stdout, "the work is done\n");
    assert_eq!(
        output.stderr, "a warning\n",
        "the seam carries two streams and `ProcessOutput` keeps them apart"
    );
    assert!(!output.timed_out);
    assert!(!output.output_limited);

    let rendered = fixture.trace.rendered();
    let at = |needle: &str| {
        fixture
            .trace
            .position_starting(needle)
            .unwrap_or_else(|| panic!("`{needle}` is not in {rendered:#?}"))
    };
    let order = [
        "site:WriteIntent:before",
        "durable:synced",
        "durable:renamed",
        "durable:dir-synced",
        "site:MountGitView:before",
        "site:Create:before",
        "site:Start:before",
        "rt:collect",
        "site:Stop:before",
        "site:Remove:before",
        "site:UnmountGitView:before",
        "site:RemoveIntent:before",
    ];
    for pair in order.windows(2) {
        assert!(
            at(pair[0]) < at(pair[1]),
            "`{}` is not before `{}` in {rendered:#?}",
            pair[0],
            pair[1]
        );
    }
    assert!(
        at("durable:synced") < at("rt:create"),
        "intent synced before docker create"
    );
    assert!(
        at("rt:create") < at("site:Start:before"),
        "created and verified before start"
    );
    assert!(
        at("view:materialized") < at("site:Start:before"),
        "view mounted before start"
    );
    assert!(at("view:materialized") < at("rt:create"), "{rendered:#?}");
    assert!(at("rt:collect") < at("rt:remove"), "{rendered:#?}");

    assert!(fixture.runtime.fake().container_names().is_empty());
    assert!(
        list_intents(&fixture.private_root)
            .expect("scan")
            .is_empty()
    );
    assert!(
        !fixture
            .private_root
            .join("views")
            .join(name.as_str())
            .exists()
    );
}

#[test]
fn a_container_that_outlives_its_timeout_is_stopped_and_removed() {
    let fixture = Fixture::new("timeout", false);
    let runner = fixture.runner();
    let payload = prompt("timeout", 128 * 1024);
    let mut request = worker_request(
        ShellKind::Sh.spec("sleep 600").stdin(payload.clone()),
        fixture.task_a.clone(),
        AgentId::new("claude-code"),
        Duration::from_secs(10),
        worker_id(0),
    );
    request.timeout = Duration::ZERO;
    let name =
        ContainerName::new(repo_key(), RUN_ID, INCARNATION_1, &request.invocation).expect("a name");

    let output = runner
        .run_blocking(&request)
        .expect("a timeout is an output, not an error");
    assert!(output.timed_out);
    assert_eq!(
        output.code, None,
        "a container the supervisor stopped did not exit on its own"
    );
    assert_eq!(
        fixture.runtime.fake().stdin_received(name.as_str()),
        Some(payload),
        "the input went to the container at its start, and the timeout path is the same"
    );

    let rendered = fixture.trace.rendered();
    let at = |needle: &str| {
        fixture
            .trace
            .position_starting(needle)
            .unwrap_or_else(|| panic!("`{needle}` is not in {rendered:#?}"))
    };
    assert!(
        at("site:Start:before") < at("site:Stop:before"),
        "{rendered:#?}"
    );
    assert!(
        at("site:Stop:before") < at("site:Remove:before"),
        "{rendered:#?}"
    );
    assert!(
        at("site:Remove:before") < at("site:UnmountGitView:before"),
        "{rendered:#?}"
    );
    assert!(
        at("site:UnmountGitView:before") < at("site:RemoveIntent:before"),
        "{rendered:#?}"
    );
    assert!(fixture.runtime.fake().container_names().is_empty());
    assert!(
        list_intents(&fixture.private_root)
            .expect("scan")
            .is_empty()
    );
    assert!(
        !fixture
            .private_root
            .join("views")
            .join(name.as_str())
            .exists()
    );

    assert_eq!(
        fixture
            .trace
            .ops()
            .iter()
            .filter(|op| **op == RuntimeOp::Observe)
            .count(),
        1,
        "{rendered:#?}"
    );
}

#[test]
fn output_beyond_the_bound_is_truncated_and_reported_as_limited() {
    for (tag, bytes, limited) in [("under", 8_usize, false), ("over", 40_usize, true)] {
        let fixture = Fixture::new(&format!("bound-{tag}"), true);
        fixture.runtime.scripts(ContainerExecution {
            exit_code: Some(0),
            stdout: vec![b'x'; bytes],
            stderr: Vec::new(),
            output_limited: false,
        });
        let runner = fixture.runner().with_output_limit(16);
        let request = gate_request(
            ShellKind::Sh.spec("yes"),
            fixture.task_a.clone(),
            Duration::from_secs(10),
            gate_id(0),
        );
        let output = runner.run_blocking(&request).expect("runs");
        assert_eq!(output.output_limited, limited, "{tag}");
        assert_eq!(output.stdout.len(), bytes.min(16), "{tag}");
        assert_eq!(output.code, Some(0), "{tag}: the exit status is held fixed");

        let probe = host::run_shell_probe(
            &fixture.runner().with_output_limit(16),
            ShellKind::Sh,
            fixture.task_a.clone(),
            shell_probe_id(),
        );
        if limited {
            let refusal = probe.expect_err("a shell that printed too much is refused");
            assert!(
                refusal.to_string().contains("bounded output allowance"),
                "{refusal}"
            );
        } else {
            probe.expect("and an ordinary one is not");
        }
    }
}

#[test]
fn a_credential_volume_is_never_created_or_pruned_by_any_disposition() {
    let volume = "upstroke-creds-claude";
    type Disposition = (&'static str, fn(&Fixture));
    let dispositions: Vec<Disposition> = vec![
        ("complete", |fixture| {
            let request = worker_request(
                ShellKind::Sh.spec("exit 0"),
                fixture.task_a.clone(),
                AgentId::new("claude-code"),
                Duration::from_secs(10),
                worker_id(0),
            );
            fixture.runner().run_blocking(&request).expect("completes");
        }),
        ("cancelled by timeout", |fixture| {
            let mut request = worker_request(
                ShellKind::Sh.spec("sleep 600"),
                fixture.task_a.clone(),
                AgentId::new("claude-code"),
                Duration::from_secs(10),
                worker_id(1),
            );
            request.timeout = Duration::ZERO;
            fixture.runner().run_blocking(&request).expect("times out");
        }),
        ("refused for a substituted image id", |fixture| {
            let request = worker_request(
                ShellKind::Sh.spec("exit 0"),
                fixture.task_a.clone(),
                AgentId::new("claude-code"),
                Duration::from_secs(10),
                worker_id(2),
            );
            let name = ContainerName::new(repo_key(), RUN_ID, INCARNATION_1, &request.invocation)
                .expect("a name");
            fixture
                .runtime
                .fake()
                .substitute_reported_image_id(name.as_str(), OTHER_IMAGE_ID);
            fixture
                .runner()
                .run_blocking(&request)
                .expect_err("refuses");
        }),
        ("refused at a funnel phase", |fixture| {
            let mut hooks = RecordingHooks::new(fixture.trace.clone());
            hooks.fail_at(
                crate::topology::effects::EffectSiteId::Container(
                    crate::topology::effects::ContainerSite::Create,
                ),
                crate::topology::effects::HookPhase::Before,
            );
            let runner = ContainerRunner::new(
                container_policy(),
                fixture.identity.clone(),
                &fixture.repo,
                fixtures::plain_checkout(&fixture.repo.join(".git")),
                image_environment(),
                Box::new(fixture.runtime.clone()),
            )
            .expect("a container policy")
            .with_hooks(Box::new(hooks))
            .with_poll(Duration::ZERO);
            let request = worker_request(
                ShellKind::Sh.spec("exit 0"),
                fixture.task_a.clone(),
                AgentId::new("claude-code"),
                Duration::from_secs(10),
                worker_id(3),
            );
            runner
                .run_blocking(&request)
                .expect_err("the funnel was made to fail");
        }),
        ("reclaimed as an orphan", |fixture| {
            let request = worker_request(
                ShellKind::Sh.spec("exit 0"),
                fixture.task_a.clone(),
                AgentId::new("claude-code"),
                Duration::from_secs(10),
                worker_id(4),
            );
            let name = ContainerName::new(repo_key(), RUN_ID, INCARNATION_1, &request.invocation)
                .expect("a name");
            let mut hooks = RecordingHooks::new(fixture.trace.clone());
            crate::runner::container::reclaim(
                &mut hooks,
                &fixture.runtime,
                &crate::runner::container::DisposableDirView::new(fixture.trace.clone()),
                &fixture.private_root,
                &name,
                Some(&view_dir(&fixture.private_root, &name)),
            )
            .expect("reclaim converges on a container that never existed");
        }),
    ];
    assert_eq!(
        dispositions.len(),
        5,
        "one per `at_run_end` outcome: Complete, Parked, Halted, BudgetExceeded, NoRunFinished"
    );
    for (tag, drive) in dispositions {
        let fixture = Fixture::new(&format!("r20-{}", tag.replace(' ', "-")), true);
        assert!(
            fixture.runtime.volume_present(volume).expect("reachable"),
            "{tag}: the operator's volume was not there to begin with"
        );
        drive(&fixture);
        assert!(
            fixture.runtime.volume_present(volume).expect("reachable"),
            "{tag}: the run pruned an operator-owned credential volume"
        );
        for (_, other) in VOLUMES {
            assert!(
                fixture.runtime.volume_present(other).expect("reachable"),
                "{tag}: `{other}` is gone"
            );
        }
    }
}

fn container_subtree_production_regions() -> (Vec<(String, String)>, BTreeSet<String>) {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("runner");
    let funnel = std::fs::read_to_string(dir.join("container.rs")).expect("the funnel");
    let declarations = crate::effects::blank_comments(&crate::effects::production_region(&funnel));
    let mut regions = vec![("container.rs".to_owned(), declarations.clone())];
    let mut excluded = BTreeSet::new();
    let mut names: Vec<String> = std::fs::read_dir(dir.join("container"))
        .expect("the container subtree")
        .map(|entry| entry.expect("an entry").file_name())
        .filter_map(|name| {
            let name = name.to_string_lossy().into_owned();
            name.ends_with(".rs").then_some(name)
        })
        .collect();
    names.sort();
    for name in names {
        let stem = name.trim_end_matches(".rs");
        if !declarations.contains(&format!("mod {stem};")) {
            excluded.insert(name);
            continue;
        }
        let source = std::fs::read_to_string(dir.join("container").join(&name)).expect("a module");
        regions.push((
            name,
            crate::effects::blank_comments(&crate::effects::production_region(&source)),
        ));
    }
    (regions, excluded)
}

#[test]
fn a_launch_that_fails_after_a_committed_effect_still_releases_everything() {
    use crate::topology::effects::{EffectSiteId, HookPhase};

    let sites = [
        ContainerSite::WriteIntent,
        ContainerSite::MountGitView,
        ContainerSite::Create,
        ContainerSite::Start,
    ];
    let mut cells = 0_usize;
    for site in sites {
        for phase in [HookPhase::Before, HookPhase::After] {
            let fixture = Fixture::new(&format!("committed-{}-{phase}", site.name()), true);
            let mut hooks = RecordingHooks::new(fixture.trace.clone());
            hooks.fail_at(EffectSiteId::Container(site), phase);
            let runner = ContainerRunner::new(
                container_policy(),
                fixture.identity.clone(),
                &fixture.repo,
                fixtures::plain_checkout(&fixture.repo.join(".git")),
                image_environment(),
                Box::new(fixture.runtime.clone()),
            )
            .expect("a container policy")
            .with_hooks(Box::new(hooks))
            .with_poll(Duration::ZERO);

            let request = worker_request(
                ShellKind::Sh.spec("exit 0"),
                fixture.task_a.clone(),
                AgentId::new("claude-code"),
                Duration::from_secs(10),
                worker_id(0),
            );
            let name = ContainerName::new(repo_key(), RUN_ID, INCARNATION_1, &request.invocation)
                .expect("a name");

            let refusal = runner
                .run_blocking(&request)
                .expect_err("the funnel was made to fail");
            assert!(
                refusal.to_string().contains(site.name()),
                "[{site:?}/{phase}] the error does not name the site that failed: {refusal}"
            );

            if phase == HookPhase::After {
                let ran = match site {
                    ContainerSite::WriteIntent => fixture
                        .trace
                        .rendered()
                        .iter()
                        .any(|entry| entry.starts_with("durable:renamed")),
                    ContainerSite::MountGitView => fixture
                        .trace
                        .rendered()
                        .iter()
                        .any(|entry| entry.starts_with("view:materialized")),
                    ContainerSite::Create => fixture.trace.ops().contains(&RuntimeOp::Create),
                    _ => fixture.trace.ops().contains(&RuntimeOp::Start),
                };
                assert!(
                    ran,
                    "[{site:?}/{phase}] the primitive did not run, so this is not the \
                     committed-effect cell it claims to be: {:#?}",
                    fixture.trace.rendered()
                );
            }

            assert!(
                fixture.runtime.fake().container_names().is_empty(),
                "[{site:?}/{phase}] a container survived"
            );
            assert!(
                list_intents(&fixture.private_root)
                    .expect("scan")
                    .is_empty(),
                "[{site:?}/{phase}] an R26 intent record survived"
            );
            assert!(
                !crate::runner::container::intent::containers_dir(&fixture.private_root)
                    .join(format!("{}.intent.tmp", name.as_str()))
                    .exists(),
                "[{site:?}/{phase}] a staged R26 record survived"
            );
            assert!(
                !view_dir(&fixture.private_root, &name).exists(),
                "[{site:?}/{phase}] an R19 view survived"
            );
            for (_, volume) in VOLUMES {
                assert!(
                    fixture.runtime.volume_present(volume).expect("reachable"),
                    "[{site:?}/{phase}] `{volume}` is gone"
                );
            }
            cells += 1;
        }
    }
    assert_eq!(cells, 8, "four sites x {{Before, After}}");
}

#[test]
fn every_at_run_end_outcome_is_driven_through_its_mechanism_and_the_ledgers_balance() {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    enum Mechanism {
        Complete,
        Cancel,
        Shutdown,
        Census,
    }

    const OUTCOMES: &[(&str, &str, &str, &str, Mechanism)] = &[
        (
            "Complete",
            "pruned",
            "persistent_output",
            "released",
            Mechanism::Complete,
        ),
        (
            "Parked",
            "pruned",
            "persistent_output",
            "released",
            Mechanism::Cancel,
        ),
        (
            "Halted",
            "pruned",
            "persistent_output",
            "released",
            Mechanism::Shutdown,
        ),
        (
            "BudgetExceeded",
            "pruned",
            "persistent_output",
            "released",
            Mechanism::Cancel,
        ),
        (
            "NoRunFinished",
            "pruned at the next write-command start after the owning container is observed \
             terminated",
            "persistent_output",
            "reclaimed when the owner or its incarnation is dead",
            Mechanism::Census,
        ),
    ];
    assert_eq!(OUTCOMES.len(), 5, "the five `at_run_end` outcomes");
    assert_eq!(
        OUTCOMES
            .iter()
            .filter(|(_, r19, ..)| *r19 == "pruned")
            .count(),
        4,
        "R19 is `pruned` in four outcomes; the fifth is pruned by the census"
    );
    assert_eq!(
        OUTCOMES
            .iter()
            .filter(|(_, _, r20, ..)| *r20 == "persistent_output")
            .count(),
        5,
        "R20 is `persistent_output` in ALL five — the row has no cell in which a run may \
         touch it"
    );
    assert_eq!(
        OUTCOMES
            .iter()
            .filter(|(_, _, _, r26, _)| *r26 == "released")
            .count(),
        4,
        "a container surviving a park or a budget stop keeps spending while the run is \
         supposed to be quiescent"
    );
    let mechanisms: BTreeSet<Mechanism> =
        OUTCOMES.iter().map(|(.., mechanism)| *mechanism).collect();
    assert_eq!(
        mechanisms,
        BTreeSet::from([
            Mechanism::Complete,
            Mechanism::Cancel,
            Mechanism::Shutdown,
            Mechanism::Census,
        ]),
        "an outcome maps to a mechanism this slice does not drive, or a mechanism the \
         lifecycle sentence names is driven by no outcome"
    );

    let volume = "upstroke-creds-claude";
    for (outcome, r19, r20, r26, mechanism) in OUTCOMES {
        let fixture = Fixture::new(&format!("outcome-{outcome}"), true);
        assert!(
            fixture.runtime.volume_present(volume).expect("reachable"),
            "[{outcome}] the operator's volume was not there to begin with"
        );

        let request = worker_request(
            ShellKind::Sh.spec("exit 0"),
            fixture.task_a.clone(),
            AgentId::new("claude-code"),
            Duration::from_secs(10),
            worker_id(0),
        );
        let name = ContainerName::new(repo_key(), RUN_ID, INCARNATION_1, &request.invocation)
            .expect("a name");
        let view = view_dir(&fixture.private_root, &name);
        let intent = name.intent_path(&fixture.private_root);
        let staged = crate::runner::container::intent::containers_dir(&fixture.private_root)
            .join(format!("{}.intent.tmp", name.as_str()));

        match mechanism {
            Mechanism::Complete => {
                fixture.runner().run_blocking(&request).expect("completes");
            }
            Mechanism::Cancel => {
                let fixture = Fixture::new(&format!("outcome-{outcome}-live"), false);
                let mut request = request.clone();
                request.timeout = Duration::ZERO;
                let output = fixture.runner().run_blocking(&request).expect("times out");
                assert!(
                    output.timed_out,
                    "[{outcome}] the cancellation cell did not cancel anything"
                );
                assert!(
                    fixture.trace.ops().contains(&RuntimeOp::Stop),
                    "[{outcome}] nothing was stopped"
                );
                assert!(
                    fixture.runtime.fake().container_names().is_empty()
                        && list_intents(&fixture.private_root)
                            .expect("scan")
                            .is_empty()
                        && !view_dir(&fixture.private_root, &name).exists(),
                    "[{outcome}] R19/R26 residue after a cancel"
                );
                assert!(
                    fixture.runtime.volume_present(volume).expect("reachable"),
                    "[{outcome}] the run pruned an operator-owned credential volume"
                );
                continue;
            }
            Mechanism::Shutdown => {
                fixture
                    .runtime
                    .fake()
                    .substitute_reported_image_id(name.as_str(), OTHER_IMAGE_ID);
                fixture
                    .runner()
                    .run_blocking(&request)
                    .expect_err("refuses");
            }
            Mechanism::Census => {
                let mut hooks = RecordingHooks::new(fixture.trace.clone());
                let dead = fixture.runner_with(RunIdentity {
                    incarnation: INCARNATION_2.to_owned(),
                    ..fixture.identity.clone()
                });
                let plan = dead.plan(&request).expect("plans");
                crate::runner::container::launch(
                    &mut hooks,
                    &fixture.runtime,
                    &crate::runner::container::view::RoleGitView::new(fixture.trace.clone()),
                    &plan.launch,
                )
                .expect("the dead incarnation's container");
                let orphan = plan.launch.name.clone();
                assert!(
                    orphan.intent_path(&fixture.private_root).exists()
                        && view_dir(&fixture.private_root, &orphan).exists()
                        && !fixture.runtime.fake().container_names().is_empty(),
                    "[{outcome}] the orphan was not seeded"
                );
                let liveness = crate::runner::container::FakeOwnerLiveness::new();
                let view_impl =
                    crate::runner::container::view::RoleGitView::new(fixture.trace.clone());
                crate::runner::container::census::run_startup_census(
                    &mut hooks,
                    &crate::runner::container::census::Census {
                        private_root: &fixture.private_root,
                        start: &crate::runner::container::census::CensusStart::FreshRun {
                            incarnation: INCARNATION_1.to_owned(),
                        },
                        runtime: &fixture.runtime,
                        liveness: &liveness,
                        view: &view_impl,
                    },
                )
                .expect("the census completes");
                assert!(
                    fixture.runtime.fake().container_names().is_empty(),
                    "[{outcome}] R26: the container"
                );
                assert!(
                    !orphan.intent_path(&fixture.private_root).exists(),
                    "[{outcome}] R26: the intent record"
                );
                assert!(
                    !view_dir(&fixture.private_root, &orphan).exists(),
                    "[{outcome}] R19: the view"
                );
                assert!(
                    fixture.runtime.volume_present(volume).expect("reachable"),
                    "[{outcome}] the census pruned an operator-owned credential volume"
                );
                continue;
            }
        }

        assert!(
            fixture.runtime.fake().container_names().is_empty(),
            "[{outcome}] R26 `{r26}`: the container survived"
        );
        assert!(!intent.exists(), "[{outcome}] R26 `{r26}`: the record");
        assert!(!staged.exists(), "[{outcome}] R26 `{r26}`: the staged half");
        assert!(
            !view.exists(),
            "[{outcome}] R19 `{r19}`: the view directory"
        );
        for (_, other) in VOLUMES {
            assert!(
                fixture.runtime.volume_present(other).expect("reachable"),
                "[{outcome}] R20 `{r20}`: `{other}` is gone"
            );
        }
    }
}

#[test]
fn every_physical_resource_of_a_container_invocation_maps_to_exactly_one_row() {
    const RESOURCES: &[(&str, &str, &str, &str)] = &[
        (
            "the running container and its labels",
            "R26",
            "released",
            "Container.Stop + Container.Remove",
        ),
        (
            "the published `<name>.intent` record",
            "R26",
            "released",
            "Container.RemoveIntent",
        ),
        (
            "the staged `<name>.intent.tmp` half",
            "R26",
            "released",
            "Container.RemoveIntent, reached by the census's staged sweep",
        ),
        (
            "the anonymous volume an image's VOLUME declaration creates",
            "R26",
            "released",
            "Container.Remove, which issues `rm --force --volumes`",
        ),
        (
            "the disposable Git view directory",
            "R19",
            "pruned",
            "Container.UnmountGitView",
        ),
        (
            "the per-agent credential volume",
            "R20",
            "persistent_output",
            "nothing: never created or pruned by a run",
        ),
    ];

    let rows: BTreeSet<&str> = RESOURCES.iter().map(|(_, row, ..)| *row).collect();
    assert_eq!(
        rows,
        BTreeSet::from(["R19", "R20", "R26"]),
        "a container invocation's resources map outside the three rows this slice owns"
    );
    let names: BTreeSet<&str> = RESOURCES.iter().map(|(name, ..)| *name).collect();
    assert_eq!(
        names.len(),
        RESOURCES.len(),
        "two rows name the same resource, so the inventory overlaps"
    );

    let mut by_row: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for (_, row, class, _) in RESOURCES {
        by_row.entry(row).or_default().insert(class);
    }
    for (row, classes) in &by_row {
        assert_eq!(
            classes.len(),
            1,
            "`{row}` holds resources of {classes:?}, so it is not one accounting class"
        );
    }
    assert_eq!(by_row["R19"], BTreeSet::from(["pruned"]));
    assert_eq!(by_row["R20"], BTreeSet::from(["persistent_output"]));
    assert_eq!(by_row["R26"], BTreeSet::from(["released"]));
    for (_, _, class, _) in RESOURCES {
        assert!(
            [
                "released",
                "consumed",
                "persistent_output",
                "pruned",
                "resumably_open"
            ]
            .contains(class),
            "`{class}` is not one of the five accounting classes"
        );
    }

    let undisposed: Vec<&str> = RESOURCES
        .iter()
        .filter(|(_, _, _, by)| by.starts_with("nothing"))
        .map(|(name, ..)| *name)
        .collect();
    assert_eq!(
        undisposed,
        vec!["the per-agent credential volume"],
        "a resource other than R20 has nothing that releases it, or R20 acquired one"
    );

    let site_names: BTreeSet<&str> = ContainerSite::ALL.iter().map(|site| site.name()).collect();
    let mut named = 0_usize;
    for (resource, _, _, by) in RESOURCES {
        for word in by.split(|c: char| !(c.is_ascii_alphanumeric() || c == '.')) {
            let Some(site) = word.strip_prefix("Container.") else {
                continue;
            };
            assert!(
                site_names.contains(site),
                "`{resource}` is disposed of at `Container.{site}`, which is not one of the \
                 frozen eight sites"
            );
            named += 1;
        }
    }
    assert!(named >= 4, "the disposers name no sites at all");
}

#[test]
fn the_container_subtree_can_only_inspect_a_volume() {
    let (regions, excluded) = container_subtree_production_regions();

    let files: BTreeSet<&str> = regions.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(
        files,
        [
            "container.rs",
            "census.rs",
            "env.rs",
            "exec.rs",
            "intent.rs",
            "resolve.rs",
            "runtime.rs",
            "view.rs",
        ]
        .into_iter()
        .collect::<BTreeSet<_>>(),
        "the domain of the R20 vocabulary census moved. Every production module of the \
         container subtree must be in it: `PR6-ACCT-002` measured this census reading \
         `container.rs` alone and reporting on seven files, because the sources were \
         concatenated and cut at the FIRST `#[cfg(test)]`"
    );
    assert_eq!(
        excluded,
        ["fake.rs", "tests.rs"]
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>(),
        "a file of this subtree is neither declared above `container.rs`'s `#[cfg(test)]` cut \
         (production, in the domain) nor below it (test-only, excluded), so nothing decides \
         which it is"
    );
    for (name, production) in &regions {
        assert!(
            production.len() > 2_000,
            "`{name}`'s production region is {} bytes, so the census below is measuring \
             almost nothing of it — this is `PR5-R1-CFG-TEST-SHRINKS-THE-DOMAIN`",
            production.len()
        );
    }
    let joined: String = regions
        .iter()
        .map(|(_, production)| production.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    assert_eq!(
        joined.matches("\"volume\",").count(),
        1,
        "the `docker volume` census is measuring nothing"
    );
    assert!(joined.contains("\"inspect\""));
    for mutating in ["\"create\", ", "volume rm", "volume prune", "\"prune\""] {
        assert_eq!(
            joined.matches(mutating).count(),
            0,
            "the container subtree names `{mutating}`, so a run could create or prune a volume"
        );
    }

    let funnel = &regions
        .iter()
        .find(|(name, _)| name == "container.rs")
        .expect("the funnel")
        .1;
    assert!(
        funnel.contains("fn expect_mounted_volumes_present"),
        "the create-time volume guard is gone, so `docker create` can create an R20 volume \
         without this subtree naming `volume create` at all"
    );
    assert_eq!(
        funnel.matches("expect_mounted_volumes_present").count(),
        2,
        "the guard must be defined and called exactly once"
    );

    let seam = &regions
        .iter()
        .find(|(name, _)| name == "runtime.rs")
        .expect("the seam")
        .1;
    assert_eq!(seam.matches("fn volume").count(), 1, "one volume method");
    assert!(seam.contains("fn volume_present(&self, name: &str) -> Result<bool, RuntimeError>"));
}

#[test]
fn the_container_runner_is_object_safe_and_send_and_sync() {
    fn takes_dyn(_: &dyn Runner) {}
    fn is_send_sync<T: Send + Sync>() {}
    is_send_sync::<ContainerRunner>();

    let fixture = Fixture::new("object-safe", true);
    let runner = fixture.runner();
    takes_dyn(&runner);
    let boxed: Box<dyn Runner> = Box::new(fixture.runner());
    takes_dyn(boxed.as_ref());
    let view: Box<dyn GitView> = Box::new(RoleGitView::new(ContainerTrace::off()));
    fn takes_view(_: &dyn GitView) {}
    takes_view(view.as_ref());
}

type SeenIntents = Arc<Mutex<Vec<(String, Option<Vec<u8>>)>>>;

#[derive(Debug)]
struct IntentPeek {
    inner: crate::runner::container::DisposableDirView,
    private_root: PathBuf,
    seen: SeenIntents,
}

impl GitView for IntentPeek {
    fn materialize(&self, request: &GitViewRequest) -> Result<PathBuf, UpstrokeError> {
        let name = request
            .path
            .file_name()
            .expect("a view path names a container")
            .to_string_lossy()
            .into_owned();
        let record = std::fs::read(
            crate::runner::container::intent::containers_dir(&self.private_root)
                .join(format!("{name}.intent")),
        )
        .ok();
        self.seen
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((name, record));
        self.inner.materialize(request)
    }

    fn discard(&self, path: &Path) -> Result<(), UpstrokeError> {
        self.inner.discard(path)
    }
}

#[derive(Debug, Default)]
struct FailingView;

impl GitView for FailingView {
    fn materialize(&self, request: &GitViewRequest) -> Result<PathBuf, UpstrokeError> {
        Err(UpstrokeError::Refused {
            message: format!("VIEW-REFUSED for {}", request.path.display()),
        })
    }

    fn discard(&self, _path: &Path) -> Result<(), UpstrokeError> {
        Ok(())
    }
}

#[test]
fn the_public_constructor_withholds_every_category_from_every_role() {
    let fixture = Fixture::new("default-confinement", true);
    let runner = ContainerRunner::new(
        container_policy(),
        fixture.identity.clone(),
        &fixture.repo,
        fixtures::plain_checkout(&fixture.repo.join(".git")),
        image_environment(),
        Box::new(fixture.runtime.clone()),
    )
    .expect("a container policy");

    let execution_root =
        crate::workspace_manager::execution_root_of(&fixture.private_root, repo_key(), RUN_ID);
    let hostile: Vec<(Withheld, PathBuf)> = vec![
        (Withheld::PublicLog, fixture.paths.public.clone()),
        (Withheld::PrivateArtifacts, fixture.private_root.clone()),
        (Withheld::SiblingWorktree, execution_root),
        (Withheld::AuthoritativeGit, fixture.repo.clone()),
    ];

    let mut refused = 0_usize;
    let mut probe_cells = 0_usize;
    let mut named: BTreeSet<Withheld> = BTreeSet::new();
    for (category, workspace) in &hostile {
        for request in requests(workspace) {
            if receives_a_worktree(&request.role) {
                let refusal = runner
                    .plan(&request)
                    .expect_err("a workspace containing a withheld path is refused");
                let message = refusal.to_string();
                assert!(
                    message.contains(category.passage()),
                    "{} / {}: {message}",
                    request.role,
                    workspace.display()
                );
                named.insert(*category);
                refused += 1;
            } else {
                let plan = runner
                    .plan(&request)
                    .expect("a probe has no worktree to refuse");
                for source in sources(plan.mounts()) {
                    assert!(
                        !source.starts_with(workspace),
                        "{}: a probe was handed `{}`",
                        request.role,
                        source.display()
                    );
                }
                probe_cells += 1;
            }
        }
    }
    assert_eq!(
        refused,
        4 * 3,
        "four categories crossed with three worktree roles"
    );
    assert_eq!(
        probe_cells,
        4 * 2,
        "four categories crossed with two probe roles"
    );
    assert_eq!(named.len(), Withheld::ALL.len(), "{named:?}");

    let mut planned = 0_usize;
    for request in requests(&fixture.task_a) {
        runner
            .plan(&request)
            .expect("the role's own worktree plans");
        planned += 1;
    }
    assert_eq!(planned, 5);

    assert!(
        list_intents(&fixture.private_root)
            .expect("scan")
            .is_empty()
    );
    assert!(fixture.runtime.fake().container_names().is_empty());
}

#[test]
fn the_execution_root_and_its_worktree_namespaces_are_withheld_and_one_worktree_is_not() {
    let fixture = Fixture::new("exec-root", true);
    let runner = fixture.runner();
    let execution_root =
        crate::workspace_manager::execution_root_of(&fixture.private_root, repo_key(), RUN_ID);
    let namespaces: Vec<PathBuf> = vec![
        execution_root.clone(),
        execution_root.join("tasks"),
        execution_root.join("merge"),
        execution_root.join("snapshots"),
    ];

    let confinement = Confinement::of_run(
        &fixture.identity,
        &fixture.repo,
        fixtures::plain_checkout(&fixture.repo.join(".git")),
    );
    for path in &namespaces {
        assert!(
            confinement
                .entries()
                .iter()
                .any(|(category, entry)| *category == Withheld::SiblingWorktree && entry == path),
            "`of_run` does not withhold `{}`: {:?}",
            path.display(),
            confinement.entries()
        );
    }

    let mut refused = 0_usize;
    for path in &namespaces {
        for request in requests(path) {
            if !receives_a_worktree(&request.role) {
                continue;
            }
            let refusal = runner
                .plan(&request)
                .expect_err("a mount containing every worktree is refused");
            assert!(
                refusal
                    .to_string()
                    .contains(Withheld::SiblingWorktree.passage()),
                "{} / {}: {refusal}",
                request.role,
                path.display()
            );
            refused += 1;
        }
    }
    assert_eq!(
        refused,
        4 * 3,
        "four namespace directories, three worktree roles"
    );

    let mut planned = 0_usize;
    for workspace in [&fixture.task_a, &fixture.task_b, &fixture.merge] {
        for request in requests(workspace) {
            if !receives_a_worktree(&request.role) {
                continue;
            }
            runner
                .plan(&request)
                .unwrap_or_else(|error| panic!("{}: {error}", workspace.display()));
            planned += 1;
        }
    }
    assert_eq!(planned, 3 * 3, "three worktrees, three worktree roles");
}

#[test]
fn every_role_writes_and_syncs_its_own_six_field_intent_before_its_container_is_created() {
    let fixture = Fixture::new("intent-per-invocation", true);
    let seen: SeenIntents = Arc::new(Mutex::new(Vec::new()));
    let runner = ContainerRunner::new(
        container_policy(),
        fixture.identity.clone(),
        &fixture.repo,
        fixtures::plain_checkout(&fixture.repo.join(".git")),
        image_environment(),
        Box::new(fixture.runtime.clone()),
    )
    .expect("a container policy")
    .with_hooks(Box::new(RecordingHooks::new(fixture.trace.clone())))
    .with_view(Box::new(IntentPeek {
        inner: crate::runner::container::DisposableDirView::new(fixture.trace.clone()),
        private_root: fixture.private_root.clone(),
        seen: Arc::clone(&seen),
    }))
    .with_poll(Duration::ZERO);

    let all = requests(&fixture.task_a);
    assert_eq!(all.len(), 5, "all five roles");
    let mut expected: Vec<(String, String)> = Vec::new();
    for request in &all {
        let name = ContainerName::new(repo_key(), RUN_ID, INCARNATION_1, &request.invocation)
            .expect("a name");
        expected.push((name.as_str().to_owned(), request.invocation.render()));
        runner.run_blocking(request).expect("runs");
    }

    let seen = seen.lock().unwrap_or_else(PoisonError::into_inner).clone();
    assert_eq!(seen.len(), 5, "five invocations, five views");
    let mut invocations: BTreeSet<String> = BTreeSet::new();
    for ((name, bytes), (expected_name, expected_invocation)) in seen.iter().zip(&expected) {
        assert_eq!(name, expected_name, "the view names another container");
        let bytes = bytes.as_ref().unwrap_or_else(|| {
            panic!("`{name}` reached Container.Create with no intent record of its own")
        });
        let record: ContainerIntent = serde_json::from_slice(bytes).expect("the six fields parse");
        assert_eq!(record.run_id, RUN_ID);
        assert_eq!(
            crate::runner::container::intent::owner_run_dir(&record.run_dir, "intent record")
                .expect("the recorded run dir decodes"),
            fixture.paths.public
        );
        assert_eq!(record.incarnation, INCARNATION_1);
        assert_eq!(record.repo_key, repo_key());
        assert_eq!(
            &record.invocation, expected_invocation,
            "`{name}` carries another invocation's record"
        );
        assert_eq!(
            record.runner_policy_sha256,
            crate::runner::policy::runner_policy_sha256(&container_policy())
        );
        invocations.insert(record.invocation.clone());
    }
    assert_eq!(
        invocations.len(),
        5,
        "five invocations must write five distinct records: {invocations:?}"
    );

    let rendered = fixture.trace.rendered();
    assert_eq!(
        rendered
            .iter()
            .filter(|entry| entry.as_str() == "durable:dir-synced:containers")
            .count(),
        5,
        "a rename is durable because the directory entry is, and there are five: {rendered:#?}"
    );
    for (name, _) in &expected {
        let at = |needle: String| {
            fixture
                .trace
                .position(&needle)
                .unwrap_or_else(|| panic!("`{needle}` is not in {rendered:#?}"))
        };
        let synced = at(format!("durable:synced:{name}.intent.tmp"));
        let renamed = at(format!("durable:renamed:{name}.intent"));
        let created = at(format!("rt:create:{name}"));
        assert!(
            synced < renamed && renamed < created,
            "`{name}`: synced {synced}, renamed {renamed}, created {created}"
        );
    }
}

#[test]
fn a_container_is_created_and_started_only_under_its_own_intent_record() {
    let fixture = Fixture::new("intent-capability", false);
    let root = fixture.private_root.clone();
    let mine =
        ContainerName::new(repo_key(), RUN_ID, INCARNATION_1, &worker_id(0)).expect("a name");
    let other =
        ContainerName::new(repo_key(), RUN_ID, INCARNATION_1, &worker_id(1)).expect("a name");

    let refusal = crate::runner::container::intent::IntentWritten::certify(&root, &mine)
        .expect_err("there is no record, so there is no proof");
    assert!(
        matches!(
            &refusal,
            UpstrokeError::Io { source, .. } if source.kind() == std::io::ErrorKind::NotFound
        ),
        "a missing record must refuse as a missing file: {refusal}"
    );

    std::fs::create_dir_all(crate::runner::container::intent::containers_dir(&root))
        .expect("the namespace");
    std::fs::write(mine.intent_path(&root), b"{\"not\":\"an intent\"}")
        .expect("a malformed record");
    let refusal = crate::runner::container::intent::IntentWritten::certify(&root, &mine)
        .expect_err("a malformed record is not evidence");
    assert!(
        matches!(refusal, UpstrokeError::Refused { .. }),
        "{refusal}"
    );

    let mut hooks = RecordingHooks::new(fixture.trace.clone());
    let plan = fixture
        .runner()
        .plan(&worker_request(
            ShellKind::Sh.spec("exit 0"),
            fixture.task_a.clone(),
            AgentId::new("claude-code"),
            Duration::from_secs(10),
            worker_id(0),
        ))
        .expect("plans");
    let proof = crate::runner::container::write_intent(
        &mut hooks,
        ContainerSite::WriteIntent,
        &root,
        &mine,
        &plan.launch.intent,
    )
    .expect("the record publishes and certifies");
    assert_eq!(proof.name(), &mine);

    fixture.trace.clear();
    let mut spec = plan.launch.spec.clone();
    spec.name = other.as_str().to_owned();
    let refusal = crate::runner::container::create_container(
        &mut hooks,
        ContainerSite::Create,
        &fixture.runtime,
        &proof,
        CoveredCreate::without_a_reaper(&spec),
    )
    .expect_err("`other` has no record of its own");
    let message = refusal.to_string();
    assert!(message.contains(other.as_str()), "{message}");
    assert!(message.contains(mine.as_str()), "{message}");
    assert!(
        message.contains("expected_failures_refusals[6]"),
        "{message}"
    );
    assert_eq!(
        fixture.trace.rendered(),
        Vec::<String>::new(),
        "the mismatch refused and something still happened: {:#?}",
        fixture.trace.rendered()
    );
    assert!(fixture.runtime.fake().container_names().is_empty());

    crate::runner::container::create_container(
        &mut hooks,
        ContainerSite::Create,
        &fixture.runtime,
        &proof,
        CoveredCreate::without_a_reaper(&plan.launch.spec),
    )
    .expect("its own proof creates");
    assert_eq!(
        fixture.runtime.fake().container_names(),
        vec![mine.as_str().to_owned()]
    );
    crate::runner::container::start_container(
        &mut hooks,
        ContainerSite::Start,
        &fixture.runtime,
        &proof,
        CoveredStart::without_a_reaper(mine.as_str()),
    )
    .expect("and starts the container the proof names");
    assert_eq!(
        fixture
            .runtime
            .fake()
            .container(mine.as_str())
            .map(|held| held.state),
        Some(Liveness::Running)
    );
}

#[cfg(unix)]
#[test]
fn a_start_cover_starts_only_the_container_it_was_minted_for() {
    let fixture = Fixture::built_in_a_bounded_child("start-cover-binding", false);
    let mut hooks = RecordingHooks::new(fixture.trace.clone());
    let plan = fixture
        .runner()
        .plan(&worker_request(
            ShellKind::Sh.spec("exit 0"),
            fixture.task_a.clone(),
            AgentId::new("claude-code"),
            Duration::from_secs(10),
            worker_id(0),
        ))
        .expect("plans");
    let other = ContainerName::new(repo_key(), RUN_ID, INCARNATION_1, &worker_id(1))
        .expect("another worker's name");
    let written = crate::runner::container::write_intent(
        &mut hooks,
        ContainerSite::WriteIntent,
        &fixture.private_root,
        &plan.launch.name,
        &plan.launch.intent,
    )
    .expect("the record publishes and certifies");
    crate::runner::container::create_container(
        &mut hooks,
        ContainerSite::Create,
        &fixture.runtime,
        &written,
        CoveredCreate::without_a_reaper(&plan.launch.spec),
    )
    .expect("its own cover creates");

    fixture.trace.clear();
    let refused = crate::runner::container::start_container(
        &mut hooks,
        ContainerSite::Start,
        &fixture.runtime,
        &written,
        CoveredStart::without_a_reaper(other.as_str()),
    )
    .expect_err("a cover minted for `other` does not start this container");
    let message = refused.error.to_string();
    assert!(
        !refused.attempted,
        "refused before the runtime was asked: {message}"
    );
    assert!(message.contains(other.as_str()), "{message}");
    assert!(
        message.contains("expected_failures_refusals[6]"),
        "{message}"
    );
    assert_eq!(
        fixture.trace.rendered(),
        Vec::<String>::new(),
        "the mismatch refused and something still happened: {:#?}",
        fixture.trace.rendered()
    );
    assert_eq!(
        fixture
            .runtime
            .fake()
            .container(plan.launch.name.as_str())
            .map(|held| held.state),
        Some(Liveness::Exited),
        "created, and still not started"
    );

    crate::runner::container::start_container(
        &mut hooks,
        ContainerSite::Start,
        &fixture.runtime,
        &written,
        CoveredStart::without_a_reaper(plan.launch.spec.name.as_str()),
    )
    .expect("the cover minted for it starts it");
    assert_eq!(
        fixture
            .runtime
            .fake()
            .container(plan.launch.name.as_str())
            .map(|held| held.state),
        Some(Liveness::Running)
    );
}

#[test]
fn a_launch_that_fails_at_any_step_releases_everything_it_reached() {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Where {
        View,
        Create,
        Mismatch,
        Start,
    }
    let points: &[(&str, Where, &str)] = &[
        (
            "the view cannot be materialised",
            Where::View,
            "VIEW-REFUSED",
        ),
        ("`docker create` fails", Where::Create, "armed failing"),
        ("the reported image id differs", Where::Mismatch, "INV-23"),
        ("`docker start` fails", Where::Start, "armed failing"),
    ];

    let mut cells = 0_usize;
    let mut with_residue = 0_usize;
    for (tag, point, marker) in points {
        for stop_fails in [false, true] {
            let fixture = Fixture::new(
                &format!(
                    "atomic-{}-{stop_fails}",
                    tag.replace(|c: char| !c.is_ascii_alphanumeric(), "-")
                ),
                true,
            );
            let request = worker_request(
                ShellKind::Sh.spec("exit 0"),
                fixture.task_a.clone(),
                AgentId::new("claude-code"),
                Duration::from_secs(10),
                worker_id(0),
            );
            let name = ContainerName::new(repo_key(), RUN_ID, INCARNATION_1, &request.invocation)
                .expect("a name");
            let mut runner = fixture.runner();
            match point {
                Where::View => runner = runner.with_view(Box::new(FailingView)),
                Where::Create => fixture.runtime.fake().set_failing(RuntimeOp::Create),
                Where::Mismatch => fixture
                    .runtime
                    .fake()
                    .substitute_reported_image_id(name.as_str(), OTHER_IMAGE_ID),
                Where::Start => fixture.runtime.fake().set_failing(RuntimeOp::Start),
            }
            if stop_fails {
                fixture.runtime.fake().set_failing(RuntimeOp::Stop);
            }

            let refusal = runner.run_blocking(&request).expect_err("the launch fails");
            let message = refusal.to_string();
            assert!(
                message.contains(marker),
                "{tag} (stop_fails: {stop_fails}): the original cause was masked: {message}"
            );

            assert!(
                fixture.runtime.fake().container_names().is_empty(),
                "{tag} (stop_fails: {stop_fails}): a container survived"
            );
            assert!(
                list_intents(&fixture.private_root)
                    .expect("scan")
                    .is_empty(),
                "{tag} (stop_fails: {stop_fails}): an intent survived"
            );
            assert!(
                !fixture
                    .private_root
                    .join("views")
                    .join(name.as_str())
                    .exists(),
                "{tag} (stop_fails: {stop_fails}): a view survived"
            );

            if *point != Where::Start {
                assert!(
                    !fixture.trace.ops().contains(&RuntimeOp::Start),
                    "{tag}: the container was started"
                );
            }

            let attempts_stop = matches!(point, Where::Create | Where::Mismatch | Where::Start);
            let expects_residue = stop_fails && attempts_stop;
            assert_eq!(
                message.contains("could not be stopped"),
                expects_residue,
                "{tag} (stop_fails: {stop_fails}): {message}"
            );
            if expects_residue {
                let sites: Vec<String> = fixture
                    .trace
                    .rendered()
                    .into_iter()
                    .filter(|entry| entry.starts_with("site:"))
                    .collect();
                for later in [
                    "site:Remove:before",
                    "site:UnmountGitView:before",
                    "site:RemoveIntent:before",
                ] {
                    assert!(
                        sites.contains(&later.to_owned()),
                        "{tag}: `{later}` was skipped after the stop failed: {sites:#?}"
                    );
                }
                with_residue += 1;
            }
            cells += 1;
        }
    }
    assert_eq!(
        cells, 8,
        "four failure points crossed with two cleanup states"
    );
    assert_eq!(
        with_residue, 3,
        "three of the cells reach the cancel with a container that may exist"
    );
}

#[test]
fn the_working_directory_is_the_runners_own_for_every_role() {
    let fixture = Fixture::new("workdir", true);
    let runner = fixture.runner();
    let mut seen: BTreeMap<String, String> = BTreeMap::new();
    for request in requests(&fixture.task_a) {
        let plan = runner.plan(&request).expect("plans");
        let workdir =
            plan.launch.spec.workdir.clone().unwrap_or_else(|| {
                panic!("{}: the image chose the working directory", request.role)
            });
        seen.insert(request.role.label(), workdir);
    }
    assert_eq!(
        seen,
        BTreeMap::from([
            ("implement".to_owned(), "/upstroke/workspace".to_owned()),
            ("gate".to_owned(), "/upstroke/workspace".to_owned()),
            ("review".to_owned(), "/upstroke/workspace".to_owned()),
            ("probe(shell)".to_owned(), "/tmp".to_owned()),
            ("probe(claude-code)".to_owned(), "/tmp".to_owned()),
        ]),
        "the working directory rule moved"
    );
    for request in requests(&fixture.task_a) {
        let plan = runner.plan(&request).expect("plans");
        let workdir = plan.launch.spec.workdir.clone().expect("pinned");
        assert!(
            plan.mounts().iter().any(|mount| mount.target() == workdir),
            "{}: the working directory `{workdir}` is not a declared mount",
            request.role
        );
    }
}

#[test]
fn a_cwd_relative_or_absent_path_refuses_every_role_before_any_effect() {
    let fixture = Fixture::new("hostile-path", true);
    let cases: Vec<(&str, ContainerEnvironment, &str)> = vec![
        (
            "a relative component",
            ContainerEnvironment::from_image(vec![(
                "PATH".to_owned(),
                CWD_RELATIVE_PATH.to_owned(),
            )]),
            "DESIGN.md:612",
        ),
        (
            "no PATH at all — the production default",
            ContainerEnvironment::inherited(),
            "names no `PATH`",
        ),
    ];
    let mut refused = 0_usize;
    for (tag, environment, expected) in cases {
        let runner = ContainerRunner::new(
            container_policy(),
            fixture.identity.clone(),
            &fixture.repo,
            fixtures::plain_checkout(&fixture.repo.join(".git")),
            environment,
            Box::new(fixture.runtime.clone()),
        )
        .expect("a container policy");
        for request in requests(&fixture.task_a) {
            let refusal = runner
                .plan(&request)
                .expect_err("an environment that cannot certify resolution is refused");
            assert!(
                refusal.to_string().contains(expected),
                "{tag} / {}: {refusal}",
                request.role
            );
            refused += 1;
        }
    }
    assert_eq!(refused, 2 * 5, "two hostile bases crossed with five roles");

    assert!(
        list_intents(&fixture.private_root)
            .expect("scan")
            .is_empty()
    );
    assert!(fixture.runtime.fake().container_names().is_empty());

    let runner = fixture.runner();
    let mut supplied: BTreeSet<String> = BTreeSet::new();
    for request in requests(&fixture.task_a) {
        let plan = runner.plan(&request).expect("plans");
        supplied.insert(
            plan.env()
                .iter()
                .find(|(key, _)| key == "PATH")
                .map(|(_, value)| value.clone())
                .expect("the runner supplies PATH"),
        );
    }
    assert_eq!(
        supplied,
        BTreeSet::from([IMAGE_PATH.to_owned()]),
        "the five roles do not execute under one PATH"
    );
}

#[test]
fn every_role_gets_a_read_only_root_and_one_ephemeral_scratch_mount() {
    let fixture = Fixture::new("read-only-root", true);
    let runner = fixture.runner();
    let mut rows = 0_usize;
    for request in requests(&fixture.task_a) {
        let plan = runner.plan(&request).expect("plans");
        assert!(
            plan.launch.spec.read_only_root,
            "{}: the container layer is writable, so a write outside every declared \
             mount would succeed",
            request.role
        );
        let scratch: Vec<Mount> = plan
            .mounts()
            .iter()
            .filter(|mount| matches!(mount, Mount::Tmpfs { .. }))
            .cloned()
            .collect();
        assert_eq!(
            scratch,
            vec![Mount::Tmpfs {
                target: "/tmp".to_owned()
            }],
            "{}: {:?}",
            request.role,
            plan.mounts()
        );
        for mount in plan.mounts() {
            let declared = match mount {
                Mount::Path { .. } | Mount::Volume { .. } => true,
                Mount::Tmpfs { target } => target == "/tmp",
            };
            assert!(declared, "{}: {mount:?}", request.role);
        }
        rows += 1;
    }
    assert_eq!(rows, 5);
}

const PREFERRED_IMAGES: &[&str] = &[
    "upstroke-test/git:v1",
    "alpine:3.20",
    "busybox:latest",
    "debian:stable-slim",
];

const GIT_IMAGES: &[&str] = &["upstroke-test/git:v1"];

const MARKER_IMAGE: &str = "upstroke-test/git:v1";
const IMAGE_MARKER_VALUE: &str = "image-environment-v1";

const CREDENTIAL_ENV_IMAGE: &str = "upstroke-test/credenv:v1";

const CREDENTIAL_ENV_CONTROL: (&str, &str) = ("GH_CONFIG_DIR", "/image/gh");

const ROOM_FOR_A_SLOW_CONTAINER_START: Duration = Duration::from_secs(30);

fn skipped(reason: &str) {
    use crate::runner::container::fake::{absent_reason, processes_out_of_sight_reason};
    assert!(
        [absent_reason(), processes_out_of_sight_reason()].contains(&reason.to_owned()),
        "a Docker-gated test skipped for a reason the gate does not know about: {reason}"
    );
}

fn no_image(reason: &str) {
    assert!(reason.contains("never pull"), "{reason}");
    assert!(
        std::env::var_os(crate::runner::container::fake::REQUIRE_DOCKER).is_none(),
        "{} is set and a gated test found no usable image: {reason}",
        crate::runner::container::fake::REQUIRE_DOCKER
    );
}

fn discover(docker: &dyn ContainerRuntime, preferred: &[&str]) -> Result<(String, String), String> {
    for reference in preferred {
        if let Ok(Some(found)) = docker.image_by_reference(reference) {
            return Ok(((*reference).to_owned(), found.id));
        }
    }
    Err(format!(
        "the container runtime holds none of {preferred:?} and these tests never pull \
         (non_goals[1])"
    ))
}

fn real_policy(image_id: &str) -> RunnerPolicy {
    RunnerPolicy {
        kind: RunnerKind::Container,
        policy: RunnerContract::ContainerV1,
        image: Some(ImageIdentity {
            reference: "discovered-locally".to_owned(),
            id: image_id.to_owned(),
            digest: None,
        }),
        credential_volumes: None,
        mounts: Vec::new(),
    }
}

fn image_environment_of(
    docker: &crate::runner::container::DockerCli,
    image_id: &str,
) -> ContainerEnvironment {
    let raw = docker
        .raw(
            RuntimeOp::InspectImageById,
            image_id,
            &[
                "image",
                "inspect",
                image_id,
                "--format",
                "{{join .Config.Env \"\u{1f}\"}}",
            ],
        )
        .expect("the image's environment");
    let declared: Vec<(String, String)> = raw
        .trim_end_matches('\n')
        .split('\u{1f}')
        .filter_map(|entry| entry.split_once('='))
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .collect();
    assert!(
        declared.iter().any(|(key, _)| key == "PATH"),
        "the discovered image declares no PATH, so nothing below measures \
         resolution: {declared:?}"
    );
    let reserved = host::reserved_keys();
    ContainerEnvironment::from_image(
        declared
            .into_iter()
            .filter(|(key, _)| reserved.iter().any(|name| name == key))
            .collect(),
    )
}

fn real_identity(root: &Path, repo: &Path, run_id: &str) -> RunIdentity {
    let private_root = root.join("private");
    let paths = RunPaths::with_private_root(repo, run_id, &private_root);
    paths.create().expect("the run's two halves");
    std::fs::write(paths.events(), format!("{EVENT_LOG_MARKER}\n")).expect("the public log");
    std::fs::write(
        paths.transcripts().join("k0-a1.md"),
        "PRIVATE-TRANSCRIPT-a5f2\n",
    )
    .expect("a private artifact");
    RunIdentity {
        private_root,
        run_id: run_id.to_owned(),
        run_dir: paths.public,
        incarnation: INCARNATION_1.to_owned(),
        repo_key: repo_key().to_owned(),
    }
}

const GATED_RUNS: &[(&str, &str)] = &[
    ("env", "01KZGATEDA000000000000000A"),
    ("readonly", "01KZGATEDB000000000000000B"),
    ("confine", "01KZGATEDC000000000000000C"),
    ("gitview", "01KZGATEDD000000000000000D"),
    ("parity", "01KZGATEDE000000000000000E"),
    ("outside", "01KZR1GATED000000000000001"),
    ("daemonspec", "01KZR1GATED000000000000002"),
    ("shadow", "01KZR1GATED000000000000003"),
    ("credenv", "01KZR3BGATED00000000000001"),
    ("descendant", "01KZR3BGATED00000000000002"),
    ("settings", "01KZP12GATED00000000000001"),
    ("user", "01KZP12GATED00000000000002"),
    ("stdin", "01KZP12GATED00000000000003"),
    ("mountro", "01KZP12GATED00000000000004"),
    ("mountrw", "01KZP12GATED00000000000005"),
    ("mountresume", "01KZP12GATED00000000000006"),
    ("slowstart", "01KZCFGATED000000000000001"),
];

fn gated_run(tag: &str) -> &'static str {
    GATED_RUNS
        .iter()
        .find(|(name, _)| *name == tag)
        .map(|(_, run)| *run)
        .unwrap_or_else(|| panic!("`{tag}` has no gated run id"))
}

struct LeaveNoResidue {
    docker: crate::runner::container::DockerCli,
    private_root: PathBuf,
}

impl Drop for LeaveNoResidue {
    fn drop(&mut self) {
        let label = crate::runner::container::intent::private_root_label(&self.private_root);
        let Ok(found) = self
            .docker
            .containers_with_label(LABEL_PRIVATE_ROOT, &label)
        else {
            return;
        };
        for container in found {
            let Ok(name) = ContainerName::rebuild(&container.name) else {
                continue;
            };
            let mut hooks = crate::runner::container::NoHooks;
            let view = crate::runner::container::DisposableDirView::default();
            let _ = crate::runner::container::reclaim(
                &mut hooks,
                &self.docker,
                &view,
                &self.private_root,
                &name,
                Some(&view_dir(&self.private_root, &name)),
            );
        }
    }
}

#[test]
fn real_docker_runs_from_the_recorded_image_id_and_composes_over_the_image_environment() {
    let trace = ContainerTrace::recording();
    let docker = match docker_gate(
        "real_docker_runs_from_the_recorded_image_id_and_composes_over_the_image_environment",
        trace.clone(),
    ) {
        Ok(docker) => docker,
        Err(reason) => return skipped(&reason),
    };
    let (reference, image_id) = match discover(docker.as_ref(), PREFERRED_IMAGES) {
        Ok(found) => found,
        Err(reason) => return no_image(&reason),
    };

    let tree = repo::scratch("real-env");
    let root = tree.path();
    let run_id = gated_run("env");
    let repo_dir = root.join("repo");
    repo::repository(&repo_dir);
    let identity = real_identity(root, &repo_dir, run_id);
    let workspace = root.join("plain-workspace");
    std::fs::create_dir_all(&workspace).expect("a workspace");
    let _residue = LeaveNoResidue {
        docker: (*docker).clone(),
        private_root: identity.private_root.clone(),
    };

    let runner = ContainerRunner::new(
        real_policy(&image_id),
        identity.clone(),
        &repo_dir,
        fixtures::plain_checkout(&repo_dir.join(".git")),
        image_environment_of(docker.as_ref(), &image_id),
        Box::new((*docker).clone()),
    )
    .expect("a container policy")
    .with_hooks(Box::new(RecordingHooks::new(trace.clone())))
    .with_poll(Duration::from_millis(10));

    let request = gate_request(
        ShellKind::Sh
            .spec(
                "printf 'PATH=%s\\n' \"$PATH\"; \
             printf 'OVERLAY=%s\\n' \"$UPSTROKE_OVERLAY\"; \
             printf 'MARKER=%s\\n' \"$UPSTROKE_IMAGE_MARKER\"; \
             printf 'PWD=%s\\n' \"$(pwd)\"",
            )
            .env("UPSTROKE_OVERLAY", "landed"),
        workspace.clone(),
        Duration::from_secs(60),
        gate_id(0),
    );

    let plan = runner.plan(&request).expect("plans");
    let supplied_path = plan
        .env()
        .iter()
        .find(|(key, _)| key == "PATH")
        .map(|(_, value)| value.clone())
        .expect("the runner supplies PATH (DESIGN.md:260)");
    assert!(
        super::super::env::cwd_dependent_path_components(&supplied_path).is_empty(),
        "the runner supplied a working-directory-relative PATH: {supplied_path}"
    );
    assert!(
        !plan
            .env()
            .iter()
            .any(|(key, _)| key == "UPSTROKE_IMAGE_MARKER"),
        "the runner named the image's own key, so the overlay claim below is vacuous: {:?}",
        plan.env()
    );
    assert_eq!(plan.launch.spec.image_id, image_id);

    let output = runner.run_blocking(&request).expect("runs");
    assert_eq!(output.code, Some(0), "stderr: {}", output.stderr);
    let line = |key: &str| -> String {
        output
            .stdout
            .lines()
            .find_map(|line| line.strip_prefix(key))
            .unwrap_or_else(|| panic!("`{key}` is not in {:?}", output.stdout))
            .to_owned()
    };
    assert_eq!(
        line("PATH="),
        supplied_path,
        "the child ran under a PATH the runner did not supply, so pre-flight cannot \
         certify which binary an attempt resolves (DESIGN.md:612)"
    );
    assert_eq!(line("OVERLAY="), "landed", "the overlay did not land");
    assert_eq!(line("PWD="), "/upstroke/workspace");
    if reference == MARKER_IMAGE {
        assert_eq!(
            line("MARKER="),
            IMAGE_MARKER_VALUE,
            "the marker image's own environment did not survive composition"
        );
    }

    assert_eq!(
        docker
            .containers_with_label(
                LABEL_PRIVATE_ROOT,
                &crate::runner::container::intent::private_root_label(&identity.private_root)
            )
            .expect("reachable")
            .len(),
        0
    );
    assert!(
        list_intents(&identity.private_root)
            .expect("scan")
            .is_empty()
    );
}

#[test]
fn real_docker_refuses_a_reviewer_write_to_its_read_only_mount() {
    let trace = ContainerTrace::recording();
    let docker = match docker_gate(
        "real_docker_refuses_a_reviewer_write_to_its_read_only_mount",
        trace.clone(),
    ) {
        Ok(docker) => docker,
        Err(reason) => return skipped(&reason),
    };
    let (_, image_id) = match discover(docker.as_ref(), PREFERRED_IMAGES) {
        Ok(found) => found,
        Err(reason) => return no_image(&reason),
    };

    let tree = repo::scratch("real-ro");
    let root = tree.path();
    let run_id = gated_run("readonly");
    let repo_dir = root.join("repo");
    repo::repository(&repo_dir);
    let identity = real_identity(root, &repo_dir, run_id);
    let _residue = LeaveNoResidue {
        docker: (*docker).clone(),
        private_root: identity.private_root.clone(),
    };
    let runner = ContainerRunner::new(
        real_policy(&image_id),
        identity.clone(),
        &repo_dir,
        fixtures::plain_checkout(&repo_dir.join(".git")),
        image_environment_of(docker.as_ref(), &image_id),
        Box::new((*docker).clone()),
    )
    .expect("a container policy")
    .with_poll(Duration::from_millis(10));

    let mut outcomes = Vec::new();
    for (tag, role_is_review, ordinal) in [("review", true, 0_u32), ("implement", false, 1)] {
        let workspace = root.join(format!("ws-{tag}"));
        std::fs::create_dir_all(&workspace).expect("a workspace");
        let spec =
            ShellKind::Sh.spec("( echo upstroke-wrote-this > /upstroke/workspace/probe.txt ) 2>&1");
        let request = if role_is_review {
            review_request(
                spec,
                workspace.clone(),
                AgentId::new("claude-code"),
                Duration::from_secs(60),
                InvocationId::attempt(
                    TaskKey(0),
                    GenerationId(0),
                    AttemptNumber(1),
                    AttemptRole::ReviewPass(0),
                    ordinal,
                ),
            )
        } else {
            worker_request(
                spec,
                workspace.clone(),
                AgentId::new("claude-code"),
                Duration::from_secs(60),
                worker_id(ordinal),
            )
        };
        let plan = runner.plan(&request).expect("plans");
        assert_eq!(
            target_of(plan.mounts(), "/upstroke/workspace").map(Mount::read_only),
            Some(role_is_review),
            "{tag}: the mount disposition"
        );
        let output = runner.run_blocking(&request).expect("runs");
        let wrote = workspace.join("probe.txt").exists();
        outcomes.push((
            tag,
            output.code,
            wrote,
            format!("{}{}", output.stdout, output.stderr),
        ));
    }

    let (_, review_code, review_wrote, review_output) = &outcomes[0];
    assert_ne!(*review_code, Some(0), "the reviewer's write succeeded");
    assert!(
        review_output.to_ascii_lowercase().contains("read-only"),
        "the failure is not the read-only mount: {review_output}"
    );
    assert!(!review_wrote, "the reviewer wrote into the workspace");

    let (_, implement_code, implement_wrote, _) = &outcomes[1];
    assert_eq!(
        *implement_code,
        Some(0),
        "the control could not write either, so the test above proves nothing"
    );
    assert!(*implement_wrote);
    assert_eq!(
        std::fs::read_to_string(root.join("ws-implement").join("probe.txt"))
            .expect("the control's file")
            .trim(),
        "upstroke-wrote-this"
    );
}

#[test]
fn real_docker_a_containerized_agent_reads_its_own_settings_and_cannot_write_them() {
    let trace = ContainerTrace::recording();
    let docker = match docker_gate(
        "real_docker_a_containerized_agent_reads_its_own_settings_and_cannot_write_them",
        trace.clone(),
    ) {
        Ok(docker) => docker,
        Err(reason) => return skipped(&reason),
    };
    let (_, image_id) = match discover(docker.as_ref(), PREFERRED_IMAGES) {
        Ok(found) => found,
        Err(reason) => return no_image(&reason),
    };

    let tree = repo::scratch("real-settings");
    let root = tree.path();
    let run_id = gated_run("settings");
    let repo_dir = root.join("repo");
    repo::repository(&repo_dir);
    let identity = real_identity(root, &repo_dir, run_id);
    let _residue = LeaveNoResidue {
        docker: (*docker).clone(),
        private_root: identity.private_root.clone(),
    };
    let runner = ContainerRunner::new(
        real_policy(&image_id),
        identity.clone(),
        &repo_dir,
        fixtures::plain_checkout(&repo_dir.join(".git")),
        image_environment_of(docker.as_ref(), &image_id),
        Box::new((*docker).clone()),
    )
    .expect("a container policy")
    .with_poll(Duration::from_millis(10));

    let paths = RunPaths::with_private_root(&repo_dir, run_id, &identity.private_root);
    let settings = paths.settings();
    std::fs::create_dir_all(&settings).expect("the settings directory");
    let file = settings.join("k0-1.json");
    let written = "{\"permissions\":{\"allow\":[\"Edit\"],\"deny\":[\"Read(.upstroke/**)\"]}}\n";
    std::fs::write(&file, written).expect("an attempt's settings");
    std::fs::write(settings.join("k0-2.json"), "SIBLING-SETTINGS-a5f2\n")
        .expect("another attempt's settings");
    let workspace = root.join("ws-settings");
    std::fs::create_dir_all(&workspace).expect("a workspace");
    let spec = crate::runner::CommandSpec::new("sh")
        .arg("-c")
        .arg(
            "cat \"$0\"; ls -A /upstroke/settings | sed 's/^/LISTED=/'; \
             test -e /upstroke/settings/k0-2.json && echo SIBLING=yes || echo SIBLING=no; \
             test -e \"$1\" && echo PRIVATE=yes || echo PRIVATE=no; \
             if ( echo tampered >> \"$0\" ) 2>/dev/null; then echo WROTE; \
             else echo READ-ONLY; fi",
        )
        .arg(file.to_string_lossy().into_owned())
        .arg(paths.private.to_string_lossy().into_owned())
        .settings_file(Some(file.clone()));
    let request = worker_request(
        spec,
        workspace,
        AgentId::new("claude-code"),
        Duration::from_secs(60),
        worker_id(0),
    );
    let output = runner.run_blocking(&request).expect("runs");
    assert_eq!(output.code, Some(0), "{}{}", output.stdout, output.stderr);
    assert!(
        output.stdout.contains(written.trim()),
        "the agent read its settings: {}",
        output.stdout
    );
    assert!(
        output.stdout.contains("READ-ONLY") && !output.stdout.contains("WROTE"),
        "the settings are read-only to the agent: {}",
        output.stdout
    );
    let listed: Vec<&str> = output
        .stdout
        .lines()
        .filter_map(|line| line.strip_prefix("LISTED="))
        .collect();
    assert_eq!(
        listed,
        vec!["k0-1.json"],
        "the agent sees its own settings file and no sibling: {}",
        output.stdout
    );
    assert!(
        output.stdout.contains("SIBLING=no") && output.stdout.contains("PRIVATE=no"),
        "another attempt's settings or the run's private directory reached the container: {}",
        output.stdout
    );
    assert_eq!(
        std::fs::read_to_string(&file).expect("the host file"),
        written,
        "the host file is unchanged"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn real_docker_a_container_runs_as_the_coordinators_user_and_leaves_what_it_can_reclaim() {
    use std::os::unix::fs::MetadataExt as _;
    let trace = ContainerTrace::recording();
    let docker = match docker_gate(
        "real_docker_a_container_runs_as_the_coordinators_user_and_leaves_what_it_can_reclaim",
        trace.clone(),
    ) {
        Ok(docker) => docker,
        Err(reason) => return skipped(&reason),
    };
    let (_, image_id) = match discover(docker.as_ref(), GIT_IMAGES) {
        Ok(found) => found,
        Err(reason) => return no_image(&reason),
    };

    let tree = repo::scratch("real-user");
    let root = tree.path();
    let run_id = gated_run("user");
    let repo_dir = root.join("repo");
    let (head, _) = repo::repository(&repo_dir);
    let identity = real_identity(root, &repo_dir, run_id);
    let execution_root =
        crate::workspace_manager::execution_root_of(&identity.private_root, repo_key(), run_id);
    let workspace = execution_root.join("tasks").join("kalpha-g0");
    repo::worktree(&repo_dir, &workspace, &head);
    let _residue = LeaveNoResidue {
        docker: (*docker).clone(),
        private_root: identity.private_root.clone(),
    };
    let runner = ContainerRunner::new(
        real_policy(&image_id),
        identity.clone(),
        &repo_dir,
        fixtures::plain_checkout(&repo_dir.join(".git")),
        image_environment_of(docker.as_ref(), &image_id),
        Box::new((*docker).clone()),
    )
    .expect("a container policy")
    .with_poll(Duration::from_millis(10));
    let expected = effective_ids();
    let line = |output: &ProcessOutput, key: &str| -> String {
        output
            .stdout
            .lines()
            .find_map(|line| line.strip_prefix(key))
            .unwrap_or_else(|| {
                panic!(
                    "`{key}` is not in stdout {:?} / stderr {:?}",
                    output.stdout, output.stderr
                )
            })
            .trim()
            .to_owned()
    };

    let worker = worker_request(
        ShellKind::Sh.spec(
            "id -u | sed 's/^/UID=/'; id -g | sed 's/^/GID=/'; \
             mkdir -p nested/deeper && echo written > nested/deeper/file && echo MADE=yes",
        ),
        workspace.clone(),
        AgentId::new("claude-code"),
        Duration::from_secs(60),
        worker_id(0),
    );
    let output = runner.run_blocking(&worker).expect("runs");
    assert_eq!(output.code, Some(0), "{}{}", output.stdout, output.stderr);
    assert_eq!(
        (line(&output, "UID="), line(&output, "GID=")),
        (expected.uid.to_string(), expected.gid.to_string()),
        "the container's process is not the coordinator's effective uid and gid"
    );
    assert_eq!(line(&output, "MADE="), "yes");
    let made = workspace.join("nested").join("deeper").join("file");
    for path in [
        workspace.join("nested"),
        workspace.join("nested").join("deeper"),
        made,
    ] {
        let metadata = std::fs::metadata(&path).expect("what the container made");
        assert_eq!(
            (metadata.uid(), metadata.gid()),
            (expected.uid, expected.gid),
            "the host sees {} as the coordinator's",
            path.display()
        );
    }
    std::fs::remove_dir_all(workspace.join("nested"))
        .expect("the coordinator reclaims what the container made");
    assert!(!workspace.join("nested").exists());

    let gate = gate_request(
        ShellKind::Sh.spec(
            "git -C /upstroke/workspace rev-parse HEAD | sed 's/^/HEAD=/'; \
             git -C /upstroke/workspace status --porcelain | wc -l | tr -d ' ' \
             | sed 's/^/DIRTY=/'",
        ),
        workspace.clone(),
        Duration::from_secs(60),
        gate_id(0),
    );
    let output = runner.run_blocking(&gate).expect("runs");
    assert_eq!(
        output.code,
        Some(0),
        "a Git-dependent gate needs no wildcard safe.directory: {}",
        output.stderr
    );
    assert!(
        !output.stderr.contains("dubious ownership"),
        "{}",
        output.stderr
    );
    assert_eq!(line(&output, "HEAD="), head, "the exact detached HEAD");
    assert_eq!(line(&output, "DIRTY="), "0", "the worktree is clean again");
}

#[test]
fn real_docker_a_containerized_program_reads_its_exact_stdin_then_eof() {
    use sha2::{Digest as _, Sha256};
    let trace = ContainerTrace::recording();
    let docker = match docker_gate(
        "real_docker_a_containerized_program_reads_its_exact_stdin_then_eof",
        trace.clone(),
    ) {
        Ok(docker) => docker,
        Err(reason) => return skipped(&reason),
    };
    let (_, image_id) = match discover(docker.as_ref(), PREFERRED_IMAGES) {
        Ok(found) => found,
        Err(reason) => return no_image(&reason),
    };

    let tree = repo::scratch("real-stdin");
    let root = tree.path();
    let run_id = gated_run("stdin");
    let repo_dir = root.join("repo");
    repo::repository(&repo_dir);
    let identity = real_identity(root, &repo_dir, run_id);
    let workspace = root.join("ws-stdin");
    std::fs::create_dir_all(&workspace).expect("a workspace");
    let _residue = LeaveNoResidue {
        docker: (*docker).clone(),
        private_root: identity.private_root.clone(),
    };
    let runner = ContainerRunner::new(
        real_policy(&image_id),
        identity.clone(),
        &repo_dir,
        fixtures::plain_checkout(&repo_dir.join(".git")),
        image_environment_of(docker.as_ref(), &image_id),
        Box::new((*docker).clone()),
    )
    .expect("a container policy")
    .with_poll(Duration::from_millis(10));
    let claude = AgentId::new("claude-code");
    let line = |output: &ProcessOutput, key: &str| -> String {
        output
            .stdout
            .lines()
            .find_map(|line| line.strip_prefix(key))
            .unwrap_or_else(|| {
                panic!(
                    "`{key}` is not in stdout {:?} / stderr {:?}",
                    output.stdout, output.stderr
                )
            })
            .trim()
            .to_owned()
    };

    let payload = prompt("exact", 300 * 1024);
    let exact = worker_request(
        ShellKind::Sh
            .spec(
                "cat > /tmp/in; wc -c < /tmp/in | tr -d ' ' | sed 's/^/BYTES=/'; \
                 sha256sum /tmp/in | cut -d ' ' -f 1 | sed 's/^/SHA=/'",
            )
            .stdin(payload.clone()),
        workspace.clone(),
        claude.clone(),
        Duration::from_secs(120),
        worker_id(0),
    );
    let output = runner.run_blocking(&exact).expect("runs");
    assert_eq!(output.code, Some(0), "{}{}", output.stdout, output.stderr);
    assert!(!output.timed_out, "`cat` never saw the end of its input");
    assert_eq!(
        line(&output, "BYTES="),
        payload.len().to_string(),
        "every byte, beyond a pipe buffer"
    );
    let digest: String = Sha256::digest(&payload)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    assert_eq!(line(&output, "SHA="), digest, "exactly the bytes sent");

    let early = worker_request(
        ShellKind::Sh
            .spec("head -c 16 > /dev/null; exit 7")
            .stdin(prompt("early", 1024 * 1024)),
        workspace.clone(),
        claude.clone(),
        Duration::from_secs(120),
        worker_id(1),
    );
    let started = Instant::now();
    let output = runner
        .run_blocking(&early)
        .expect("a program that stops reading early reports its own exit");
    assert_eq!(output.code, Some(7), "{}{}", output.stdout, output.stderr);
    assert!(!output.timed_out);
    assert!(
        started.elapsed() < Duration::from_secs(60),
        "the undelivered rest of the input held the invocation: {:?}",
        started.elapsed()
    );

    let never = worker_request(
        ShellKind::Sh
            .spec("sleep 600")
            .stdin(prompt("never-read", 1024 * 1024)),
        workspace,
        claude,
        ROOM_FOR_A_SLOW_CONTAINER_START,
        worker_id(2),
    );
    let started = Instant::now();
    let output = runner
        .run_blocking(&never)
        .expect("a timeout is an output, not an error");
    assert!(output.timed_out, "{}{}", output.stdout, output.stderr);
    assert!(
        started.elapsed() < Duration::from_secs(120),
        "a pending input write held the timeout: {:?}",
        started.elapsed()
    );
}

#[test]
fn real_docker_confines_a_gate_to_its_mount() {
    let trace = ContainerTrace::recording();
    let docker = match docker_gate("real_docker_confines_a_gate_to_its_mount", trace.clone()) {
        Ok(docker) => docker,
        Err(reason) => return skipped(&reason),
    };
    let (_, image_id) = match discover(docker.as_ref(), PREFERRED_IMAGES) {
        Ok(found) => found,
        Err(reason) => return no_image(&reason),
    };

    let tree = repo::scratch("real-confine");
    let root = tree.path();
    let run_id = gated_run("confine");
    let repo_dir = root.join("repo");
    let (head, _) = repo::repository(&repo_dir);
    let identity = real_identity(root, &repo_dir, run_id);
    let paths = RunPaths::with_private_root(&repo_dir, run_id, &identity.private_root);
    let execution_root =
        crate::workspace_manager::execution_root_of(&identity.private_root, repo_key(), run_id);
    let mine = execution_root.join("tasks").join("kalpha-g0");
    let sibling = execution_root.join("tasks").join("kbeta-g0");
    repo::worktree(&repo_dir, &mine, &head);
    repo::worktree(&repo_dir, &sibling, &head);
    std::fs::write(sibling.join("sibling.txt"), "SIBLING-WORKTREE-a5f2\n").expect("a sibling file");
    std::fs::write(mine.join("mine.txt"), "MY-OWN-WORKTREE-a5f2\n").expect("my file");

    let withheld: Vec<(Withheld, PathBuf)> = vec![
        (Withheld::PublicLog, paths.events()),
        (
            Withheld::PrivateArtifacts,
            paths.transcripts().join("k0-a1.md"),
        ),
        (Withheld::SiblingWorktree, sibling.join("sibling.txt")),
        (
            Withheld::AuthoritativeGit,
            repo_dir.join(".git").join("HEAD"),
        ),
    ];
    let before: Vec<Vec<u8>> = withheld
        .iter()
        .map(|(_, path)| std::fs::read(path).expect("a withheld file"))
        .collect();

    let _residue = LeaveNoResidue {
        docker: (*docker).clone(),
        private_root: identity.private_root.clone(),
    };
    let runner = ContainerRunner::new(
        real_policy(&image_id),
        identity.clone(),
        &repo_dir,
        fixtures::plain_checkout(&repo_dir.join(".git")),
        image_environment_of(docker.as_ref(), &image_id),
        Box::new((*docker).clone()),
    )
    .expect("a container policy")
    .with_poll(Duration::from_millis(10));

    let mut script = String::from("cat /upstroke/workspace/mine.txt;");
    for (_, path) in &withheld {
        let path = path.to_string_lossy().replace('\\', "/");
        script.push_str(&format!(
            " printf 'READ {path}: '; cat '{path}' 2>&1 | head -1; \
             printf 'WRITE {path}: '; \
             ( mkdir -p \"$(dirname '{path}')\" && \
               echo upstroke-container-wrote-this > '{path}' ) 2>&1 && echo WROTE || echo FAILED;"
        ));
    }
    let request = gate_request(
        ShellKind::Sh.spec(&script),
        mine.clone(),
        Duration::from_secs(60),
        gate_id(0),
    );
    let output = runner.run_blocking(&request).expect("runs");

    assert!(
        output.stdout.contains("MY-OWN-WORKTREE-a5f2"),
        "the gate could not read its own workspace, so nothing here is measured: {:?}",
        output.stdout
    );
    for marker in [
        EVENT_LOG_MARKER,
        "PRIVATE-TRANSCRIPT-a5f2",
        "SIBLING-WORKTREE-a5f2",
    ] {
        assert!(
            !output.stdout.contains(marker),
            "the gate read `{marker}`: {:?}",
            output.stdout
        );
    }
    for ((category, path), original) in withheld.iter().zip(&before) {
        assert_eq!(
            &std::fs::read(path).expect("still there"),
            original,
            "a gate changed `{}` ({})",
            path.display(),
            category.passage()
        );
    }
    assert_eq!(repo::git_ok(&repo_dir, &["rev-parse", "HEAD"]), head);
}

#[test]
fn real_docker_a_gate_write_outside_every_declared_mount_fails() {
    let trace = ContainerTrace::recording();
    let docker = match docker_gate(
        "real_docker_a_gate_write_outside_every_declared_mount_fails",
        trace.clone(),
    ) {
        Ok(docker) => docker,
        Err(reason) => return skipped(&reason),
    };
    let (_, image_id) = match discover(docker.as_ref(), PREFERRED_IMAGES) {
        Ok(found) => found,
        Err(reason) => return no_image(&reason),
    };

    let tree = repo::scratch("real-outside");
    let root = tree.path();
    let run_id = gated_run("outside");
    let repo_dir = root.join("repo");
    let (head, _) = repo::repository(&repo_dir);
    let identity = real_identity(root, &repo_dir, run_id);
    let execution_root =
        crate::workspace_manager::execution_root_of(&identity.private_root, repo_key(), run_id);
    let mine = execution_root.join("tasks").join("kalpha-g0");
    repo::worktree(&repo_dir, &mine, &head);

    let _residue = LeaveNoResidue {
        docker: (*docker).clone(),
        private_root: identity.private_root.clone(),
    };
    let runner = ContainerRunner::new(
        real_policy(&image_id),
        identity.clone(),
        &repo_dir,
        fixtures::plain_checkout(&repo_dir.join(".git")),
        image_environment_of(docker.as_ref(), &image_id),
        Box::new((*docker).clone()),
    )
    .expect("a container policy")
    .with_poll(Duration::from_millis(10));

    const OUTSIDE: &[&str] = &[
        "/outside-role-mount",
        "/etc/upstroke-escaped",
        "/usr/local/bin/upstroke-escaped",
        "/upstroke/escape",
    ];
    let inside: Vec<String> = vec![
        format!("{}/written-by-the-gate", BoundaryLayout::DEFAULT_WORKSPACE),
        format!("{}/written-by-the-gate", BoundaryLayout::DEFAULT_SCRATCH),
    ];

    let mut script = String::new();
    for path in OUTSIDE
        .iter()
        .map(|p| (*p).to_owned())
        .chain(inside.clone())
    {
        script.push_str(&format!(
            "if ( printf owned > '{path}' ) 2>/dev/null; then echo 'WROTE {path}'; \
             else echo 'FAILED {path}'; fi;"
        ));
    }
    let request = gate_request(
        ShellKind::Sh.spec(&script),
        mine.clone(),
        Duration::from_secs(60),
        gate_id(0),
    );
    let plan = runner.plan(&request).expect("plans");
    let targets: Vec<&str> = plan.mounts().iter().map(Mount::target).collect();
    for path in OUTSIDE {
        assert!(
            !targets.iter().any(|target| path.starts_with(target)),
            "`{path}` is inside a declared mount, so refusing it proves nothing: {targets:?}"
        );
    }
    assert!(plan.launch.spec.read_only_root);

    let output = runner.run_blocking(&request).expect("runs");
    let said = |path: &str| -> String {
        output
            .stdout
            .lines()
            .find(|line| line.ends_with(path))
            .unwrap_or_else(|| {
                panic!(
                    "`{path}` is in neither stream — stdout {:?} / stderr {:?}",
                    output.stdout, output.stderr
                )
            })
            .to_owned()
    };
    for path in OUTSIDE {
        assert_eq!(
            said(path),
            format!("FAILED {path}"),
            "a gate wrote outside every declared mount, so \
             `expected_failures_refusals[5]` does not hold: {:?}",
            output.stdout
        );
    }
    for path in &inside {
        assert_eq!(said(path), format!("WROTE {path}"), "{:?}", output.stdout);
    }
    assert_eq!(
        std::fs::read_to_string(mine.join("written-by-the-gate"))
            .expect("the gate's own worktree write reached the host"),
        "owned"
    );
}

#[test]
fn real_docker_the_daemon_holds_exactly_the_specs_mounts_and_a_read_only_root() {
    let trace = ContainerTrace::recording();
    let docker = match docker_gate(
        "real_docker_the_daemon_holds_exactly_the_specs_mounts_and_a_read_only_root",
        trace.clone(),
    ) {
        Ok(docker) => docker,
        Err(reason) => return skipped(&reason),
    };
    let (_, image_id) = match discover(docker.as_ref(), PREFERRED_IMAGES) {
        Ok(found) => found,
        Err(reason) => return no_image(&reason),
    };

    let tree = repo::scratch("real-daemonspec");
    let root = tree.path();
    let run_id = gated_run("daemonspec");
    let repo_dir = root.join("repo");
    let (head, _) = repo::repository(&repo_dir);
    let identity = real_identity(root, &repo_dir, run_id);
    let execution_root =
        crate::workspace_manager::execution_root_of(&identity.private_root, repo_key(), run_id);
    let mine = execution_root.join("tasks").join("kalpha-g0");
    repo::worktree(&repo_dir, &mine, &head);

    let _residue = LeaveNoResidue {
        docker: (*docker).clone(),
        private_root: identity.private_root.clone(),
    };
    let runner = ContainerRunner::new(
        real_policy(&image_id),
        identity.clone(),
        &repo_dir,
        fixtures::plain_checkout(&repo_dir.join(".git")),
        image_environment_of(docker.as_ref(), &image_id),
        Box::new((*docker).clone()),
    )
    .expect("a container policy")
    .with_poll(Duration::from_millis(10));

    let request = gate_request(
        ShellKind::Sh.spec("exit 0"),
        mine.clone(),
        Duration::from_secs(60),
        gate_id(0),
    );
    let plan = runner.plan(&request).expect("plans");
    let name = plan.launch.name.clone();

    crate::runner::container::fake::preclean_names(
        docker.as_ref(),
        &RoleGitView::new(ContainerTrace::off()).for_reader(
            BoundaryLayout::DEFAULT_GIT_VIEW,
            BoundaryLayout::DEFAULT_GIT_OBJECTS,
        ),
        &identity.private_root,
        &[&name],
    );

    let mut hooks = crate::runner::container::NoHooks;
    let view = RoleGitView::new(ContainerTrace::off()).for_reader(
        BoundaryLayout::DEFAULT_GIT_VIEW,
        BoundaryLayout::DEFAULT_GIT_OBJECTS,
    );
    let written = crate::runner::container::write_intent(
        &mut hooks,
        ContainerSite::WriteIntent,
        &identity.private_root,
        &name,
        &plan.launch.intent,
    )
    .expect("the record publishes");
    crate::runner::container::mount_git_view(
        &mut hooks,
        ContainerSite::MountGitView,
        &view,
        &plan.launch.view,
    )
    .expect("the view is a bind source, so it exists before the create");
    crate::runner::container::create_container(
        &mut hooks,
        ContainerSite::Create,
        docker.as_ref(),
        &written,
        CoveredCreate::without_a_reaper(&plan.launch.spec),
    )
    .expect("created");

    let read_only = docker
        .raw(
            RuntimeOp::Observe,
            name.as_str(),
            &[
                "container",
                "inspect",
                name.as_str(),
                "--format",
                "{{.HostConfig.ReadonlyRootfs}}",
            ],
        )
        .expect("the daemon answers");
    assert_eq!(
        read_only.trim(),
        "true",
        "the daemon gave the container a writable root filesystem"
    );

    let raw = docker
        .raw(
            RuntimeOp::Observe,
            name.as_str(),
            &[
                "container",
                "inspect",
                name.as_str(),
                "--format",
                "{{range .Mounts}}{{.Type}}\u{1f}{{.Destination}}\u{1f}{{.RW}}\u{1e}{{end}}",
            ],
        )
        .expect("the daemon answers");
    let daemon: BTreeSet<(String, String, bool)> = raw
        .trim()
        .split('\u{1e}')
        .filter(|entry| !entry.trim().is_empty())
        .map(|entry| {
            let fields: Vec<&str> = entry.trim().split('\u{1f}').collect();
            (
                fields.first().copied().unwrap_or_default().to_owned(),
                fields.get(1).copied().unwrap_or_default().to_owned(),
                fields.get(2).copied().unwrap_or_default() == "true",
            )
        })
        .collect();
    let planned: BTreeSet<(String, String, bool)> = plan
        .mounts()
        .iter()
        .map(|mount| {
            let kind = match mount {
                Mount::Path { .. } => "bind",
                Mount::Volume { .. } => "volume",
                Mount::Tmpfs { .. } => "tmpfs",
            };
            (
                kind.to_owned(),
                mount.target().to_owned(),
                !mount.read_only(),
            )
        })
        .collect();
    assert_eq!(
        daemon, planned,
        "the container the daemon holds is not the one `CreateSpec` describes — a mount \
         that reaches the daemon without going through `CreateSpec.mounts` is exactly \
         `PR6-ENUM-005`'s surviving mutation"
    );
    assert!(planned.len() >= 4, "{planned:?}");
    assert!(
        planned.contains(&("tmpfs".to_owned(), "/tmp".to_owned(), true)),
        "{planned:?}"
    );

    crate::runner::container::reclaim(
        &mut hooks,
        docker.as_ref(),
        &view,
        &identity.private_root,
        &name,
        Some(&view_dir(&identity.private_root, &name)),
    )
    .expect("reclaimed");
}

#[test]
fn real_docker_a_worktree_binary_cannot_shadow_the_certified_cli() {
    let trace = ContainerTrace::recording();
    let docker = match docker_gate(
        "real_docker_a_worktree_binary_cannot_shadow_the_certified_cli",
        trace.clone(),
    ) {
        Ok(docker) => docker,
        Err(reason) => return skipped(&reason),
    };
    let (_, image_id) = match discover(docker.as_ref(), PREFERRED_IMAGES) {
        Ok(found) => found,
        Err(reason) => return no_image(&reason),
    };

    let tree = repo::scratch("real-shadow");
    let root = tree.path();
    let run_id = gated_run("shadow");
    let repo_dir = root.join("repo");
    let (head, _) = repo::repository(&repo_dir);
    let identity = real_identity(root, &repo_dir, run_id);
    let execution_root =
        crate::workspace_manager::execution_root_of(&identity.private_root, repo_key(), run_id);
    let mine = execution_root.join("tasks").join("kalpha-g0");
    repo::worktree(&repo_dir, &mine, &head);

    let shim = mine.join("claude");
    std::fs::write(&shim, "#!/bin/sh\necho SHIMMED\n").expect("the shim");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755))
            .expect("executable");
    }

    let _residue = LeaveNoResidue {
        docker: (*docker).clone(),
        private_root: identity.private_root.clone(),
    };
    let label = crate::runner::container::intent::private_root_label(&identity.private_root);

    let hostile = ContainerRunner::new(
        real_policy(&image_id),
        identity.clone(),
        &repo_dir,
        fixtures::plain_checkout(&repo_dir.join(".git")),
        ContainerEnvironment::from_image(vec![("PATH".to_owned(), format!(".:{IMAGE_PATH}"))]),
        Box::new((*docker).clone()),
    )
    .expect("a container policy")
    .with_poll(Duration::from_millis(10));
    let refusal = hostile
        .run_blocking(&gate_request(
            ShellKind::Sh.spec("claude"),
            mine.clone(),
            Duration::from_secs(60),
            gate_id(1),
        ))
        .expect_err("a cwd-relative PATH is refused");
    assert!(refusal.to_string().contains("DESIGN.md:612"), "{refusal}");
    assert_eq!(
        docker
            .containers_with_label(LABEL_PRIVATE_ROOT, &label)
            .expect("reachable")
            .len(),
        0,
        "the refusal created a container"
    );

    let runner = ContainerRunner::new(
        real_policy(&image_id),
        identity.clone(),
        &repo_dir,
        fixtures::plain_checkout(&repo_dir.join(".git")),
        image_environment_of(docker.as_ref(), &image_id),
        Box::new((*docker).clone()),
    )
    .expect("a container policy")
    .with_poll(Duration::from_millis(10));

    let script = "printf 'PWD=%s\\n' \"$(pwd)\"; \
         printf 'CLAUDE=%s\\n' \"$(command -v claude || echo NOTFOUND)\"; \
         printf 'SH=%s\\n' \"$(command -v sh || echo NOTFOUND)\"";
    let gate = gate_request(
        ShellKind::Sh.spec(&format!(
            "{script}; printf 'SHIM=%s\\n' \"$(test -x ./claude && echo PRESENT || echo ABSENT)\""
        )),
        mine.clone(),
        Duration::from_secs(60),
        gate_id(0),
    );
    let probe = crate::agent::probe_request(
        "claude-code",
        ShellKind::Sh.spec(script),
        0,
        Duration::from_secs(60),
    )
    .expect("an agent probe request");

    let mut answers: Vec<(&str, BTreeMap<String, String>)> = Vec::new();
    for (tag, request) in [("the attempt", gate), ("the probe", probe)] {
        let output = runner.run_blocking(&request).expect("runs");
        assert_eq!(output.code, Some(0), "{tag}: stderr {}", output.stderr);
        let mut read = BTreeMap::new();
        for line in output.stdout.lines() {
            if let Some((key, value)) = line.split_once('=') {
                read.insert(key.to_owned(), value.trim().to_owned());
            }
        }
        answers.push((tag, read));
    }

    assert_eq!(answers[0].1["PWD"], BoundaryLayout::DEFAULT_WORKSPACE);
    assert_eq!(answers[1].1["PWD"], BoundaryLayout::DEFAULT_SCRATCH);
    assert_ne!(answers[0].1["PWD"], answers[1].1["PWD"]);
    assert_eq!(
        answers[0].1["SHIM"], "PRESENT",
        "the shim is not in the attempt's worktree, so NOTFOUND below proves nothing"
    );
    assert_eq!(
        answers[0].1["CLAUDE"], "NOTFOUND",
        "repository content became the CLI the attempt runs"
    );
    assert_eq!(answers[1].1["CLAUDE"], "NOTFOUND");
    assert_eq!(
        answers[0].1["SH"], answers[1].1["SH"],
        "pre-flight certified a different binary from the one the attempt executes \
         (DESIGN.md:612)"
    );
    assert!(
        answers[0].1["SH"].starts_with('/'),
        "{:?}",
        answers[0].1["SH"]
    );

    assert_eq!(
        docker
            .containers_with_label(LABEL_PRIVATE_ROOT, &label)
            .expect("reachable")
            .len(),
        0
    );
}

#[test]
fn real_docker_a_git_dependent_gate_sees_only_the_role_view() {
    let trace = ContainerTrace::recording();
    let docker = match docker_gate(
        "real_docker_a_git_dependent_gate_sees_only_the_role_view",
        trace.clone(),
    ) {
        Ok(docker) => docker,
        Err(reason) => return skipped(&reason),
    };
    let (_, image_id) = match discover(docker.as_ref(), GIT_IMAGES) {
        Ok(found) => found,
        Err(reason) => return no_image(&reason),
    };

    let tree = repo::scratch("real-gitview");
    let root = tree.path();
    let run_id = gated_run("gitview");
    let repo_dir = root.join("repo");
    let (head, _) = repo::repository(&repo_dir);
    let planted = repo::engine_refs(&repo_dir, &head);
    let identity = real_identity(root, &repo_dir, run_id);
    let execution_root =
        crate::workspace_manager::execution_root_of(&identity.private_root, repo_key(), run_id);
    let workspace = execution_root.join("tasks").join("kalpha-g0");
    repo::worktree(&repo_dir, &workspace, &head);
    repo::git_ok(&repo_dir, &["pack-refs", "--all"]);

    let _residue = LeaveNoResidue {
        docker: (*docker).clone(),
        private_root: identity.private_root.clone(),
    };
    let runner = ContainerRunner::new(
        real_policy(&image_id),
        identity.clone(),
        &repo_dir,
        fixtures::plain_checkout(&repo_dir.join(".git")),
        image_environment_of(docker.as_ref(), &image_id),
        Box::new((*docker).clone()),
    )
    .expect("a container policy")
    .with_poll(Duration::from_millis(10));

    let git = "git -C /upstroke/workspace";
    let leak = &planted[0];
    let script = format!(
        "{git} rev-parse HEAD | sed 's/^/HEAD=/'; \
         {git} rev-parse --absolute-git-dir | sed 's/^/GITDIR=/'; \
         {git} for-each-ref --format='%(refname)' | wc -l | tr -d ' ' | sed 's/^/REFS=/'; \
         {git} log -1 --format=%s | sed 's/^/SUBJECT=/'; \
         {git} status --porcelain | wc -l | tr -d ' ' | sed 's/^/DIRTY=/'; \
         {git} rev-parse --verify --quiet '{leak}' >/dev/null 2>&1 && echo LEAK=yes || echo LEAK=no"
    );
    let request = gate_request(
        ShellKind::Sh.spec(&script),
        workspace.clone(),
        Duration::from_secs(60),
        gate_id(0),
    );
    let output = runner.run_blocking(&request).expect("runs");
    assert_eq!(output.code, Some(0), "stderr: {}", output.stderr);
    let line = |key: &str| -> String {
        output
            .stdout
            .lines()
            .find_map(|line| line.strip_prefix(key))
            .unwrap_or_else(|| {
                panic!(
                    "`{key}` is not in stdout {:?} / stderr {:?}",
                    output.stdout, output.stderr
                )
            })
            .trim()
            .to_owned()
    };
    assert_eq!(line("HEAD="), head, "the exact detached HEAD");
    assert_eq!(
        line("GITDIR="),
        BoundaryLayout::DEFAULT_GIT_VIEW,
        "the tool found the coordinator's Git directory, not the role view"
    );
    assert_eq!(line("REFS="), "0", "the view carries refs");
    assert_eq!(line("SUBJECT="), "second", "the objects do not resolve");
    assert_eq!(
        line("DIRTY="),
        "0",
        "the index the view carries is not exact"
    );
    assert_eq!(
        line("LEAK="),
        "no",
        "an engine ref resolved inside the view"
    );

    for name in &planted {
        assert_eq!(
            repo::git_ok(&repo_dir, &["rev-parse", "--verify", name]),
            head
        );
    }
    assert_eq!(repo::git_ok(&repo_dir, &["rev-parse", "HEAD"]), head);
    assert!(
        list_intents(&identity.private_root)
            .expect("scan")
            .is_empty()
    );
}

#[test]
fn real_docker_adapter_parsing_matches_the_host_table() {
    let trace = ContainerTrace::recording();
    let docker = match docker_gate(
        "real_docker_adapter_parsing_matches_the_host_table",
        trace.clone(),
    ) {
        Ok(docker) => docker,
        Err(reason) => return skipped(&reason),
    };
    let (_, image_id) = match discover(docker.as_ref(), PREFERRED_IMAGES) {
        Ok(found) => found,
        Err(reason) => return no_image(&reason),
    };

    let tree = repo::scratch("real-parity");
    let root = tree.path();
    let run_id = gated_run("parity");
    let repo_dir = root.join("repo");
    repo::repository(&repo_dir);
    let identity = real_identity(root, &repo_dir, run_id);
    let workspace = root.join("parity-workspace");
    std::fs::create_dir_all(&workspace).expect("a workspace");
    let _residue = LeaveNoResidue {
        docker: (*docker).clone(),
        private_root: identity.private_root.clone(),
    };
    let runner = ContainerRunner::new(
        real_policy(&image_id),
        identity.clone(),
        &repo_dir,
        fixtures::plain_checkout(&repo_dir.join(".git")),
        image_environment_of(docker.as_ref(), &image_id),
        Box::new((*docker).clone()),
    )
    .expect("a container policy")
    .with_poll(Duration::from_millis(10));

    let container_rows = crate::runner::contract::tests::adapter_parse_parity(&runner, &workspace);
    let host_rows = crate::runner::contract::tests::adapter_parse_parity(
        &crate::runner::host::HostRunner::new(),
        &workspace,
    );
    assert_eq!(
        container_rows, host_rows,
        "the container runner's adapter parsing differs from the host's"
    );
    assert_eq!(container_rows.len(), 3);
    let statuses: BTreeSet<String> = container_rows
        .iter()
        .map(|row| format!("{:?}", row.status))
        .collect();
    assert_eq!(statuses.len(), 2, "{statuses:?}");
    assert!(
        list_intents(&identity.private_root)
            .expect("scan")
            .is_empty()
    );
}

#[test]
fn real_docker_withholds_an_image_credential_variable_from_a_role_that_takes_none() {
    let trace = ContainerTrace::recording();
    let docker = match docker_gate(
        "real_docker_withholds_an_image_credential_variable_from_a_role_that_takes_none",
        trace.clone(),
    ) {
        Ok(docker) => docker,
        Err(reason) => return skipped(&reason),
    };
    let (_, image_id) = match discover(docker.as_ref(), &[CREDENTIAL_ENV_IMAGE]) {
        Ok(found) => found,
        Err(reason) => return no_image(&reason),
    };
    let declared = docker
        .raw(
            RuntimeOp::InspectImageById,
            &image_id,
            &[
                "image",
                "inspect",
                &image_id,
                "--format",
                "{{join .Config.Env \"\u{1f}\"}}",
            ],
        )
        .expect("the image environment");
    assert!(
        declared.contains("CODEX_HOME=/image/codex"),
        "`{CREDENTIAL_ENV_IMAGE}` does not set a credential location, so this measurement \
         would be vacuous: {declared}"
    );

    let tree = repo::scratch("real-credenv");
    let root = tree.path();
    let repo_dir = root.join("repo");
    repo::repository(&repo_dir);
    let identity = real_identity(root, &repo_dir, gated_run("credenv"));
    let workspace = root.join("plain-workspace");
    std::fs::create_dir_all(&workspace).expect("a workspace");
    let _residue = LeaveNoResidue {
        docker: (*docker).clone(),
        private_root: identity.private_root.clone(),
    };

    let runner = ContainerRunner::new(
        real_policy(&image_id),
        identity,
        &repo_dir,
        fixtures::plain_checkout(&repo_dir.join(".git")),
        image_environment_of(docker.as_ref(), &image_id),
        Box::new((*docker).clone()),
    )
    .expect("a container policy")
    .with_hooks(Box::new(RecordingHooks::new(trace.clone())))
    .with_poll(Duration::from_millis(10));

    let (control_key, control_value) = CREDENTIAL_ENV_CONTROL;
    let spec = ShellKind::Sh.spec(&format!(
        "printf 'CODEX=[%s]\\n' \"$CODEX_HOME\"; \
         printf 'CLAUDE=[%s]\\n' \"$CLAUDE_CONFIG_DIR\"; \
         printf 'CONTROL=[%s]\\n' \"${{{control_key}}}\""
    ));
    let request = gate_request(spec, workspace, Duration::from_secs(60), gate_id(0));
    let output = runner.run_blocking(&request).expect("runs");
    assert_eq!(output.code, Some(0), "stderr: {}", output.stderr);
    let line = |key: &str| -> String {
        output
            .stdout
            .lines()
            .find_map(|line| line.strip_prefix(key))
            .unwrap_or_else(|| panic!("`{key}` is not in {:?}", output.stdout))
            .to_owned()
    };
    assert_eq!(
        line("CODEX="),
        "[]",
        "a gate ran with the image's own `CODEX_HOME`; DESIGN.md:258-262 has each runner \
         supply ROLE-SCOPED credential locations, and a location the composed vector does \
         not name is one the image supplies"
    );
    assert_eq!(line("CLAUDE="), "[]", "stdout: {}", output.stdout);
    assert_eq!(
        line("CONTROL="),
        format!("[{control_value}]"),
        "the image environment was wiped rather than overridden, so the two assertions \
         above hold for the wrong reason"
    );
}

#[test]
#[cfg(target_os = "linux")]
fn real_docker_a_container_contains_a_daemonised_descendant() {
    use std::sync::atomic::{AtomicBool, Ordering};

    struct SetOnDrop<'flag>(&'flag AtomicBool);

    impl Drop for SetOnDrop<'_> {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    fn pids_with(marker: &str) -> Vec<String> {
        let mut found = Vec::new();
        let Ok(entries) = std::fs::read_dir("/proc") else {
            return found;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            let Ok(raw) = std::fs::read(entry.path().join("cmdline")) else {
                continue;
            };
            if raw
                .split(|byte| *byte == 0)
                .filter(|arg| !arg.is_empty())
                .any(|arg| String::from_utf8_lossy(arg) == marker)
            {
                found.push(name);
            }
        }
        found
    }

    let clock = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos());
    let pid = std::process::id();
    let namespace = std::fs::metadata(Path::new("/proc/self/ns/pid"))
        .map_or(0, |found| std::os::unix::fs::MetadataExt::ino(&found));
    let leader_marker = format!("1{pid:07}.{namespace:020}{clock:039}");
    let descendant_marker = format!("2{pid:07}.{namespace:020}{clock:039}");
    let probe_marker = format!("3{pid:07}.{namespace:020}{clock:039}");

    let trace = ContainerTrace::recording();
    let docker = match docker_gate(
        "real_docker_a_container_contains_a_daemonised_descendant",
        trace.clone(),
    ) {
        Ok(docker) => docker,
        Err(reason) => return skipped(&reason),
    };
    let (_, image_id) = match discover(docker.as_ref(), PREFERRED_IMAGES) {
        Ok(found) => found,
        Err(reason) => return no_image(&reason),
    };

    let tree = repo::scratch("real-descendant");
    let root = tree.path();
    let repo_dir = root.join("repo");
    repo::repository(&repo_dir);
    let identity = real_identity(root, &repo_dir, gated_run("descendant"));
    let workspace = root.join("plain-workspace");
    std::fs::create_dir_all(&workspace).expect("a workspace");
    let _residue = LeaveNoResidue {
        docker: (*docker).clone(),
        private_root: identity.private_root.clone(),
    };

    let runner = ContainerRunner::new(
        real_policy(&image_id),
        identity,
        &repo_dir,
        fixtures::plain_checkout(&repo_dir.join(".git")),
        image_environment_of(docker.as_ref(), &image_id),
        Box::new((*docker).clone()),
    )
    .expect("a container policy")
    .with_hooks(Box::new(RecordingHooks::new(trace.clone())))
    .with_poll(Duration::from_millis(10));

    let probe = gate_request(
        ShellKind::Sh.spec(&format!("sleep {probe_marker}")),
        workspace.clone(),
        ROOM_FOR_A_SLOW_CONTAINER_START,
        gate_id(1),
    );
    let cancellation = Cancellation::new();
    let probe_returned = AtomicBool::new(false);
    let (visible, probed) = std::thread::scope(|scope| {
        let sampler = scope.spawn(|| {
            while !probe_returned.load(Ordering::SeqCst) {
                if !pids_with(&probe_marker).is_empty() {
                    cancellation.cancel();
                    return true;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            false
        });
        let probed = {
            let _returned = SetOnDrop(&probe_returned);
            runner.run_blocking_with(&probe, RunnerCall::new(cancellation.clone()))
        };
        (
            sampler.join().expect("the probe's sampler panicked"),
            probed,
        )
    });
    match probed {
        Ok(output) => assert!(
            output.timed_out,
            "the probe's `sleep` ended before its deadline, so its absence from `/proc` says \
             nothing about this host: {output:?}"
        ),
        Err(error) => {
            assert!(error.is_cancelled(), "the probe invocation failed: {error}");
            assert_eq!(
                error.fate,
                ProcessFate::Gone,
                "the probe was cancelled and its container may outlive it: {error}"
            );
        }
    }
    if !visible {
        let reason = crate::runner::container::fake::processes_out_of_sight_reason();
        assert!(
            std::env::var_os(crate::runner::container::fake::REQUIRE_DOCKER).is_none(),
            "{} is set and this test would have skipped: {reason}",
            crate::runner::container::fake::REQUIRE_DOCKER
        );
        return skipped(&reason);
    }

    let mut request = gate_request(
        ShellKind::Sh.spec(&format!(
            "if command -v setsid >/dev/null 2>&1; then \
               setsid sleep {descendant_marker} >/dev/null 2>&1 & \
             else \
               sleep {descendant_marker} >/dev/null 2>&1 & \
             fi; \
             sleep {leader_marker}"
        )),
        workspace,
        Duration::from_secs(120),
        gate_id(0),
    );
    request.timeout = ROOM_FOR_A_SLOW_CONTAINER_START;

    let returned = AtomicBool::new(false);
    let seen = std::thread::scope(|scope| {
        let sampler = scope.spawn(|| {
            let mut leader = false;
            let mut descendant = false;
            while !(returned.load(Ordering::SeqCst) || (leader && descendant)) {
                leader |= !pids_with(&leader_marker).is_empty();
                descendant |= !pids_with(&descendant_marker).is_empty();
                std::thread::sleep(Duration::from_millis(50));
            }
            (leader, descendant)
        });
        let ran = {
            let _returned = SetOnDrop(&returned);
            runner.run_blocking(&request)
        };
        let output = ran.expect("runs");
        assert!(output.timed_out, "the fixture did not reach its timeout");
        sampler.join().expect("the sampler panicked")
    });
    assert_eq!(
        seen,
        (true, true),
        "the fixture never had a leader and a detached descendant running at once, so the \
         containment assertion below would hold vacuously"
    );

    let leader = pids_with(&leader_marker);
    let descendant = pids_with(&descendant_marker);
    assert!(
        descendant.is_empty(),
        "a `setsid`-detached descendant survived its container, so \
         `invariants_introduced[0]` (\"container contains descendants\") does not hold: \
         {descendant:?}"
    );
    assert!(
        leader.is_empty(),
        "the invocation's leader survived its container: {leader:?}"
    );
}

#[test]
fn real_docker_a_container_descendant_test_is_gated_to_linux() {
    const GUARDED: &str = "real_docker_a_container_contains_a_daemonised_descendant";
    let source = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("runner")
            .join("container")
            .join("exec")
            .join("tests.rs"),
    )
    .expect("read this module");
    let attributes = attributes_of_test(&source, GUARDED);
    assert!(
        attributes.contains(&"#[cfg(target_os = \"linux\")]"),
        "`{GUARDED}` finds its container's processes through the host's own `/proc`, which a \
         Docker Desktop or Lima host on macOS does not have, so anywhere but Linux it fails \
         after its invocation instead of observing anything; its attributes are \
         {attributes:?} (`PR366-MACOS-PROC-SAMPLER`)"
    );
}

/// The attribute lines directly above `fn {name}() {` in `source`, nearest
/// first, each with its trailing whitespace removed so that a CRLF checkout
/// (the Windows guest's) reads the same as an LF one: `str::lines` keeps the
/// `\r` of the last line, because `source[..at]` ends before its `\n`.
fn attributes_of_test<'a>(source: &'a str, name: &str) -> Vec<&'a str> {
    let signature = format!("\nfn {name}() {{");
    let at = source
        .find(&signature)
        .unwrap_or_else(|| panic!("`{name}` is not a test here"));
    source[..at]
        .lines()
        .rev()
        .map(str::trim_end)
        .take_while(|line| line.starts_with("#["))
        .collect()
}

#[test]
fn real_docker_a_container_descendant_gate_reads_lf_and_crlf_alike() {
    let lf = "}\n\n#[test]\n#[cfg(target_os = \"linux\")]\nfn guarded() {\n}\n";
    let crlf = lf.replace('\n', "\r\n");
    assert!(crlf.contains("\r\n#[cfg"), "the CRLF fragment is not CRLF");
    for source in [lf, crlf.as_str()] {
        assert_eq!(
            attributes_of_test(source, "guarded"),
            ["#[cfg(target_os = \"linux\")]", "#[test]"],
            "the attributes of {source:?}"
        );
    }
}

#[cfg(unix)]
const SLOW_START_SCRATCH: &str = "UPSTROKE_TEST_SLOW_START_SCRATCH";

#[cfg(unix)]
const SLOW_START_ORIGINAL_PATH: &str = "UPSTROKE_TEST_SLOW_START_ORIGINAL_PATH";

#[cfg(unix)]
const SLOW_START_MARK: &str = "UPSTROKE_TEST_SLOW_START_MARK";

#[cfg(unix)]
const SLOW_START_HOLD_SECONDS: u64 = 5;

#[cfg(unix)]
#[test]
fn real_docker_a_container_slow_to_start_still_reaches_its_timeout() {
    let docker = match docker_gate(
        "real_docker_a_container_slow_to_start_still_reaches_its_timeout",
        ContainerTrace::off(),
    ) {
        Ok(docker) => docker,
        Err(reason) => return skipped(&reason),
    };
    if let Err(reason) = discover(docker.as_ref(), PREFERRED_IMAGES) {
        return no_image(&reason);
    }
    let inherited = std::env::var_os("PATH").unwrap_or_default();

    let tree = repo::scratch("real-slow-start");
    let bin = tree.path().join("bin");
    std::fs::create_dir(&bin).expect("the stand-in's directory");
    let path = std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(&inherited)))
        .expect("a PATH with the stand-in's directory first");
    let mark = tree.path().join("delayed-starts");
    let ended = crate::agent::proc::test_support::run_test_isolated(
        "runner::container::exec::tests::slow_docker_start_child",
        &[
            ("PATH", path.as_os_str()),
            (SLOW_START_SCRATCH, tree.path().as_os_str()),
            (SLOW_START_ORIGINAL_PATH, inherited.as_os_str()),
            (SLOW_START_MARK, mark.as_os_str()),
        ],
        Duration::from_secs(120),
    );
    assert!(
        ended.status.is_some_and(|status| status.success()) && ended.stdout.contains("1 passed"),
        "with `docker start` held {SLOW_START_HOLD_SECONDS} seconds, the helper's never-read invocation did not end \
         in a timed-out output under {ROOM_FOR_A_SLOW_CONTAINER_START:?}: {ended}"
    );
}

#[cfg(unix)]
#[test]
#[ignore = "isolated helper of the slow container start witness"]
fn slow_docker_start_child() {
    use std::os::unix::fs::PermissionsExt as _;

    let scratch = PathBuf::from(
        std::env::var_os(SLOW_START_SCRATCH).expect("the parent names its scratch directory"),
    );
    assert!(
        std::env::var_os(SLOW_START_ORIGINAL_PATH).is_some(),
        "the parent hands this process, whose runner runs the stand-in, its own `PATH` in \
         {SLOW_START_ORIGINAL_PATH}"
    );
    let mark = PathBuf::from(
        std::env::var_os(SLOW_START_MARK).expect("the parent names the stand-in's mark"),
    );
    let stand_in = scratch
        .join("bin")
        .join(crate::runner::container::DOCKER_PROGRAM);
    std::fs::write(
        &stand_in,
        format!(
            "#!/bin/sh\n\
             fail() {{ echo \"stand-in docker: $1\" >&2; exit 1; }}\n\
             clock() {{ if [ -r /proc/uptime ]; then read -r up _ </proc/uptime && echo \"${{up%%.*}}\"; \
             else /bin/date +%s; fi; }}\n\
             if [ \"$1\" = start ]; then\n\
             \x20 began=$(clock) || fail 'no clock before the delay'\n\
             \x20 /bin/sleep {SLOW_START_HOLD_SECONDS} || fail 'the delay failed'\n\
             \x20 ended=$(clock) || fail 'no clock after the delay'\n\
             \x20 echo \"delayed $((ended - began))\" >>\"${SLOW_START_MARK}\" || fail 'no mark'\n\
             fi\n\
             PATH=\"${SLOW_START_ORIGINAL_PATH}\" exec docker \"$@\"\n"
        ),
    )
    .expect("the stand-in `docker`");
    std::fs::set_permissions(&stand_in, std::fs::Permissions::from_mode(0o755))
        .expect("the stand-in is executable");
    assert_eq!(
        crate::util::find_program(crate::runner::container::DOCKER_PROGRAM),
        Some(stand_in),
        "the stand-in is not the `docker` this process runs, so nothing below is slowed"
    );

    let docker = crate::runner::container::DockerCli::new(ContainerTrace::off());
    let (_, image_id) = discover(&docker, PREFERRED_IMAGES).expect("the parent found an image");
    let root = scratch.as_path();
    let repo_dir = root.join("repo");
    repo::repository(&repo_dir);
    let identity = real_identity(root, &repo_dir, gated_run("slowstart"));
    let workspace = root.join("ws-slow-start");
    std::fs::create_dir_all(&workspace).expect("a workspace");
    let _residue = LeaveNoResidue {
        docker: docker.clone(),
        private_root: identity.private_root.clone(),
    };
    let runner = ContainerRunner::new(
        real_policy(&image_id),
        identity,
        &repo_dir,
        fixtures::plain_checkout(&repo_dir.join(".git")),
        image_environment_of(&docker, &image_id),
        Box::new(docker.clone()),
    )
    .expect("a container policy")
    .with_poll(Duration::from_millis(10));

    let never = worker_request(
        ShellKind::Sh
            .spec("sleep 600")
            .stdin(prompt("never-read", 1024 * 1024)),
        workspace,
        AgentId::new("claude-code"),
        ROOM_FOR_A_SLOW_CONTAINER_START,
        worker_id(0),
    );
    let output = runner
        .run_blocking(&never)
        .expect("a timeout is an output, not an error");
    assert!(output.timed_out, "{}{}", output.stdout, output.stderr);

    let delayed = match std::fs::read_to_string(&mark) {
        Ok(delayed) => delayed,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => panic!(
            "the stand-in's mark {} is unreadable: {error}",
            mark.display()
        ),
    };
    assert!(
        !delayed.trim().is_empty(),
        "no `docker start` was delayed: the stand-in left no mark at {}, so the timeout above \
         was reached without the slow start this test exists to hold",
        mark.display()
    );
    for line in delayed.lines() {
        let measured = line
            .strip_prefix("delayed ")
            .and_then(|seconds| seconds.parse::<u64>().ok());
        assert!(
            measured.is_some_and(|seconds| seconds >= SLOW_START_HOLD_SECONDS),
            "a mark line that is not `delayed <seconds>` with at least the \
             {SLOW_START_HOLD_SECONDS} seconds the stand-in's `/bin/sleep \
             {SLOW_START_HOLD_SECONDS}` asked for, as its clock measured them: {line:?}"
        );
    }
}

#[cfg(unix)]
struct MountedReal {
    _tree: ScratchTree,
    root: PathBuf,
    repo: PathBuf,
    identity: RunIdentity,
    reference: String,
    image_id: String,
}

#[cfg(unix)]
impl MountedReal {
    fn new(
        docker: &crate::runner::container::DockerCli,
        tag: &str,
        run: &str,
    ) -> Result<Self, String> {
        let (reference, image_id) = discover(docker, PREFERRED_IMAGES)?;
        let tree = repo::scratch(tag);
        let root = std::fs::canonicalize(tree.path()).expect("the scratch root canonicalizes");
        let repo_dir = root.join("repo");
        repo::repository(&repo_dir);
        let identity = real_identity(&root, &repo_dir, gated_run(run));
        Ok(Self {
            _tree: tree,
            root,
            repo: repo_dir,
            identity,
            reference,
            image_id,
        })
    }

    fn selection(&self, mounts: Vec<crate::config::RunnerMount>) -> crate::config::RunnerSelection {
        crate::config::RunnerSelection {
            kind: RunnerKind::Container,
            image: Some(self.reference.clone()),
            credential_volumes: BTreeMap::new(),
            mounts,
            from_config: true,
        }
    }

    fn resolve(
        &self,
        docker: &crate::runner::container::DockerCli,
        mounts: Vec<crate::config::RunnerMount>,
    ) -> RunnerPolicy {
        let context = MountContext::bind(
            &self.repo,
            fixtures::plain_checkout(&self.repo.join(".git")),
            &self.identity.private_root,
        )
        .expect("the real fixture binds a mount context");
        crate::runner::container::resolve::resolve_container(
            docker,
            &self.selection(mounts),
            &context,
        )
        .expect("the configured mounts resolve against the real runtime")
    }

    fn runner(
        &self,
        docker: &crate::runner::container::DockerCli,
        policy: RunnerPolicy,
    ) -> ContainerRunner {
        ContainerRunner::new(
            policy,
            self.identity.clone(),
            &self.repo,
            fixtures::plain_checkout(&self.repo.join(".git")),
            image_environment_of(docker, &self.image_id),
            Box::new(docker.clone()),
        )
        .expect("a container policy")
        .with_poll(Duration::from_millis(10))
    }

    fn workspace(&self, name: &str) -> PathBuf {
        let workspace = self.root.join(name);
        std::fs::create_dir_all(&workspace).expect("a workspace");
        workspace
    }
}

#[cfg(unix)]
fn mount(source: &Path, target: &str, read_only: bool) -> crate::config::RunnerMount {
    crate::config::RunnerMount {
        source: source.to_path_buf(),
        target: target.to_owned(),
        read_only,
    }
}

#[cfg(unix)]
fn stdout_value(output: &ProcessOutput, key: &str) -> String {
    output
        .stdout
        .lines()
        .find_map(|line| line.strip_prefix(key))
        .unwrap_or_else(|| {
            panic!(
                "`{key}` is not in stdout {:?} / stderr {:?}",
                output.stdout, output.stderr
            )
        })
        .trim()
        .to_owned()
}

#[cfg(unix)]
#[test]
fn real_docker_a_read_only_configured_mount_is_readable_and_not_writable() {
    let trace = ContainerTrace::recording();
    let docker = match docker_gate(
        "real_docker_a_read_only_configured_mount_is_readable_and_not_writable",
        trace.clone(),
    ) {
        Ok(docker) => docker,
        Err(reason) => return skipped(&reason),
    };
    let world = match MountedReal::new(docker.as_ref(), "real-mount-ro", "mountro") {
        Ok(world) => world,
        Err(reason) => return no_image(&reason),
    };
    let _residue = LeaveNoResidue {
        docker: (*docker).clone(),
        private_root: world.identity.private_root.clone(),
    };
    let shared = world.root.join("operator-shared");
    std::fs::create_dir_all(&shared).expect("an operator directory");
    std::fs::write(shared.join("marker.txt"), "MOUNTED-READ-ONLY-7c1e\n").expect("a marker");
    let policy = world.resolve(
        docker.as_ref(),
        vec![mount(&shared, "/operator/shared", true)],
    );
    let runner = world.runner(docker.as_ref(), policy);

    let request = gate_request(
        ShellKind::Sh.spec(
            "printf 'READ=%s\\n' \"$(cat /operator/shared/marker.txt)\"; \
             ( echo written > /operator/shared/written.txt ) 2>&1; echo WRITE=$?",
        ),
        world.workspace("ws-gate"),
        Duration::from_secs(60),
        gate_id(0),
    );
    let plan = runner.plan(&request).expect("plans");
    assert_eq!(
        target_of(plan.mounts(), "/operator/shared").map(Mount::read_only),
        Some(true),
        "the recorded disposition"
    );
    let output = runner.run_blocking(&request).expect("runs");
    assert_eq!(output.code, Some(0), "{}{}", output.stdout, output.stderr);
    assert_eq!(stdout_value(&output, "READ="), "MOUNTED-READ-ONLY-7c1e");
    assert_ne!(
        stdout_value(&output, "WRITE="),
        "0",
        "a write through the read-only mount succeeded"
    );
    assert!(
        output.stdout.to_ascii_lowercase().contains("read-only"),
        "the write failed for another reason: {}",
        output.stdout
    );
    assert!(
        !shared.join("written.txt").exists(),
        "the container wrote into the host directory through a read-only mount"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn real_docker_a_read_write_configured_mount_is_writable_as_the_coordinators_user() {
    use std::os::unix::fs::MetadataExt as _;
    let trace = ContainerTrace::recording();
    let docker = match docker_gate(
        "real_docker_a_read_write_configured_mount_is_writable_as_the_coordinators_user",
        trace.clone(),
    ) {
        Ok(docker) => docker,
        Err(reason) => return skipped(&reason),
    };
    let world = match MountedReal::new(docker.as_ref(), "real-mount-rw", "mountrw") {
        Ok(world) => world,
        Err(reason) => return no_image(&reason),
    };
    let _residue = LeaveNoResidue {
        docker: (*docker).clone(),
        private_root: world.identity.private_root.clone(),
    };
    let cache = world.root.join("operator-cache");
    std::fs::create_dir_all(&cache).expect("an operator directory");
    let policy = world.resolve(
        docker.as_ref(),
        vec![mount(&cache, "/operator/cache", false)],
    );
    let runner = world.runner(docker.as_ref(), policy);
    let expected = effective_ids();

    let worker = worker_request(
        ShellKind::Sh.spec(
            "mkdir -p /operator/cache/made && echo MOUNTED-READ-WRITE-7c1e \
             > /operator/cache/made/file && echo MADE=yes",
        ),
        world.workspace("ws-implement"),
        AgentId::new("claude-code"),
        Duration::from_secs(60),
        worker_id(0),
    );
    let output = runner.run_blocking(&worker).expect("runs");
    assert_eq!(output.code, Some(0), "{}{}", output.stdout, output.stderr);
    assert_eq!(stdout_value(&output, "MADE="), "yes");
    let made = cache.join("made").join("file");
    assert_eq!(
        std::fs::read_to_string(&made).expect("the host sees the write"),
        "MOUNTED-READ-WRITE-7c1e\n"
    );
    for path in [cache.join("made"), made] {
        let metadata = std::fs::metadata(&path).expect("what the container made");
        assert_eq!(
            (metadata.uid(), metadata.gid()),
            (expected.uid, expected.gid),
            "the host sees {} as the coordinator's",
            path.display()
        );
    }

    let review = review_request(
        ShellKind::Sh.spec("( echo nope > /operator/cache/review.txt ) 2>&1; echo WRITE=$?"),
        world.workspace("ws-review"),
        AgentId::new("claude-code"),
        Duration::from_secs(60),
        InvocationId::attempt(
            TaskKey(0),
            GenerationId(0),
            AttemptNumber(1),
            AttemptRole::ReviewPass(0),
            0,
        ),
    );
    let output = runner.run_blocking(&review).expect("runs");
    assert_ne!(
        stdout_value(&output, "WRITE="),
        "0",
        "a reviewer wrote through a read-write configured mount"
    );
    assert!(!cache.join("review.txt").exists());
    std::fs::remove_dir_all(cache.join("made"))
        .expect("the coordinator reclaims what the container made");
}

#[cfg(unix)]
#[test]
fn real_docker_a_resume_with_changed_mount_config_keeps_the_recorded_mounts() {
    let trace = ContainerTrace::recording();
    let docker = match docker_gate(
        "real_docker_a_resume_with_changed_mount_config_keeps_the_recorded_mounts",
        trace.clone(),
    ) {
        Ok(docker) => docker,
        Err(reason) => return skipped(&reason),
    };
    let world = match MountedReal::new(docker.as_ref(), "real-mount-resume", "mountresume") {
        Ok(world) => world,
        Err(reason) => return no_image(&reason),
    };
    let _residue = LeaveNoResidue {
        docker: (*docker).clone(),
        private_root: world.identity.private_root.clone(),
    };
    let recorded_source = world.root.join("recorded-source");
    let today_source = world.root.join("today-source");
    for (dir, marker) in [
        (&recorded_source, "RECORDED-SOURCE-7c1e"),
        (&today_source, "TODAY-SOURCE-7c1e"),
    ] {
        std::fs::create_dir_all(dir).expect("an operator directory");
        std::fs::write(dir.join("marker.txt"), format!("{marker}\n")).expect("a marker");
    }
    let record = world.resolve(
        docker.as_ref(),
        vec![mount(&recorded_source, "/operator/data", true)],
    );
    let today = world.selection(vec![
        mount(&today_source, "/operator/data", false),
        mount(&today_source, "/operator/extra", false),
    ]);

    let mut warnings = Vec::new();
    let rebuilt = crate::runner::container::resolve::rebuild_by_inspection(
        docker.as_ref(),
        &record,
        &today,
        &mut warnings,
    )
    .expect("the recorded runner is re-established");
    assert_eq!(rebuilt, record, "today's config reached the rebuilt runner");
    assert!(
        warnings
            .iter()
            .any(|warning| warning.contains("configured mount set")
                && warning.contains("is ignored")),
        "{warnings:?}"
    );

    let runner = world.runner(docker.as_ref(), rebuilt);
    let request = gate_request(
        ShellKind::Sh.spec(
            "printf 'DATA=%s\\n' \"$(cat /operator/data/marker.txt)\"; \
             if [ -e /operator/extra ]; then echo EXTRA=present; else echo EXTRA=absent; fi; \
             ( echo x > /operator/data/written.txt ) 2>&1; echo WRITE=$?",
        ),
        world.workspace("ws-resumed"),
        Duration::from_secs(60),
        gate_id(0),
    );
    let output = runner.run_blocking(&request).expect("runs");
    assert_eq!(output.code, Some(0), "{}{}", output.stdout, output.stderr);
    assert_eq!(
        stdout_value(&output, "DATA="),
        "RECORDED-SOURCE-7c1e",
        "the resumed container did not see the recorded source"
    );
    assert_eq!(stdout_value(&output, "EXTRA="), "absent");
    assert_ne!(
        stdout_value(&output, "WRITE="),
        "0",
        "the recorded mount is read-only"
    );
    assert!(!recorded_source.join("written.txt").exists());
    assert!(!today_source.join("written.txt").exists());

    let away = world.root.join("recorded-source.away");
    std::fs::rename(&recorded_source, &away).expect("move the recorded source");
    let mut warnings = Vec::new();
    let refusal = crate::runner::container::resolve::rebuild_by_inspection(
        docker.as_ref(),
        &record,
        &today,
        &mut warnings,
    )
    .expect_err("a resume whose recorded source is gone refuses");
    assert!(
        matches!(
            refusal,
            crate::runner::container::resolve::InspectionRefusal::MountSourceMoved { .. }
        ),
        "{refusal}"
    );
    assert!(
        docker
            .containers_with_label(
                LABEL_PRIVATE_ROOT,
                &crate::runner::container::intent::private_root_label(&world.identity.private_root),
            )
            .expect("reachable")
            .is_empty(),
        "a container exists for the refused resume"
    );
    std::fs::rename(&away, &recorded_source).expect("put it back");
}

#[test]
fn every_gated_test_of_this_lane_is_counted() {
    const MINE: &[&str] = &[
        "real_docker_runs_from_the_recorded_image_id_and_composes_over_the_image_environment",
        "real_docker_refuses_a_reviewer_write_to_its_read_only_mount",
        "real_docker_confines_a_gate_to_its_mount",
        "real_docker_a_git_dependent_gate_sees_only_the_role_view",
        "real_docker_adapter_parsing_matches_the_host_table",
        "real_docker_a_gate_write_outside_every_declared_mount_fails",
        "real_docker_the_daemon_holds_exactly_the_specs_mounts_and_a_read_only_root",
        "real_docker_a_worktree_binary_cannot_shadow_the_certified_cli",
        "real_docker_withholds_an_image_credential_variable_from_a_role_that_takes_none",
        "real_docker_a_container_contains_a_daemonised_descendant",
        "real_docker_a_containerized_agent_reads_its_own_settings_and_cannot_write_them",
        "real_docker_a_container_runs_as_the_coordinators_user_and_leaves_what_it_can_reclaim",
        "real_docker_a_containerized_program_reads_its_exact_stdin_then_eof",
        "real_docker_a_read_only_configured_mount_is_readable_and_not_writable",
        "real_docker_a_read_write_configured_mount_is_writable_as_the_coordinators_user",
        "real_docker_a_resume_with_changed_mount_config_keeps_the_recorded_mounts",
        "real_docker_a_container_slow_to_start_still_reaches_its_timeout",
    ];
    assert_eq!(MINE.len(), 17);
    assert_eq!(GATED_RUNS.len(), MINE.len(), "one run id per gated test");
    let ids: BTreeSet<&str> = GATED_RUNS.iter().map(|(_, run)| *run).collect();
    assert_eq!(
        ids.len(),
        GATED_RUNS.len(),
        "two gated tests share a run id, so they would fight over a container name"
    );
    let source = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("runner")
            .join("container")
            .join("exec")
            .join("tests.rs"),
    )
    .expect("read this module");
    for name in MINE {
        assert!(
            DOCKER_GATED_TESTS.contains(name),
            "`{name}` is gated and nothing counts it"
        );
        assert!(
            source.contains(&format!("fn {name}(")),
            "`{name}` is counted and is not a test here"
        );
    }
}

struct CancelAfter {
    inner: RecordingHooks,
    cancellation: crate::runner::Cancellation,
    at: crate::topology::effects::EffectSiteId,
}

impl ContainerHooks for CancelAfter {
    fn phase(
        &mut self,
        site: crate::topology::effects::EffectSiteId,
        phase: crate::topology::effects::HookPhase,
    ) -> crate::topology::effects::Injection {
        if site == self.at && phase == crate::topology::effects::HookPhase::After {
            self.cancellation.cancel();
        }
        self.inner.phase(site, phase)
    }

    fn trace(&self) -> ContainerTrace {
        self.inner.trace()
    }
}

#[test]
fn a_cancelled_container_invocation_is_released_through_the_reclaim_steps_before_it_reports() {
    let fixture = Fixture::new("cancel-running", false);
    let runner = fixture.runner();
    let payload = prompt("cancelled", 128 * 1024);
    let request = worker_request(
        ShellKind::Sh.spec("sleep 600").stdin(payload.clone()),
        fixture.task_a.clone(),
        AgentId::new("claude-code"),
        Duration::from_secs(600),
        worker_id(0),
    );
    let name =
        ContainerName::new(repo_key(), RUN_ID, INCARNATION_1, &request.invocation).expect("a name");
    let cancellation = crate::runner::Cancellation::new();
    let mut observer = CancelAfter {
        inner: RecordingHooks::new(fixture.trace.clone()),
        cancellation: cancellation.clone(),
        at: crate::topology::effects::EffectSiteId::Container(ContainerSite::Start),
    };

    let error = runner
        .run_blocking_with(
            &request,
            RunnerCall::new(cancellation).observed_in_container_by(&mut observer),
        )
        .expect_err("a cancelled invocation reports an error, not an output");
    assert!(error.is_cancelled(), "{error}");
    assert_eq!(error.fate, ProcessFate::Gone, "{error}");
    assert_eq!(
        fixture.runtime.fake().stdin_received(name.as_str()),
        Some(payload),
        "the input went to the container at its start, and the cancellation path is the same"
    );

    let rendered = fixture.trace.rendered();
    let at = |needle: &str| {
        fixture
            .trace
            .position_starting(needle)
            .unwrap_or_else(|| panic!("`{needle}` is not in {rendered:#?}"))
    };
    assert!(
        at("site:Start:before") < at("site:Stop:before"),
        "{rendered:#?}"
    );
    assert!(
        at("site:Stop:before") < at("site:Remove:before"),
        "{rendered:#?}"
    );
    assert!(
        at("site:Remove:before") < at("site:UnmountGitView:before"),
        "{rendered:#?}"
    );
    assert!(
        at("site:UnmountGitView:before") < at("site:RemoveIntent:before"),
        "{rendered:#?}"
    );
    assert!(fixture.runtime.fake().container_names().is_empty());
    assert!(
        list_intents(&fixture.private_root)
            .expect("scan")
            .is_empty()
    );
    assert!(
        !fixture
            .private_root
            .join("views")
            .join(name.as_str())
            .exists()
    );
    assert!(
        !fixture.trace.ops().contains(&RuntimeOp::Collect),
        "a cancelled invocation's output is not collected: {rendered:#?}"
    );
}

#[test]
fn a_launch_that_fails_after_a_cancellation_reports_the_cancellation() {
    let fixture = Fixture::new("cancel-failed-start", false);
    fixture.runtime.fake().set_failing(RuntimeOp::Start);
    let runner = fixture.runner();
    let request = worker_request(
        ShellKind::Sh
            .spec("cat")
            .stdin(prompt("cancelled-start", 4 * 1024)),
        fixture.task_a.clone(),
        AgentId::new("claude-code"),
        Duration::from_secs(600),
        worker_id(0),
    );
    let cancellation = crate::runner::Cancellation::new();
    let mut observer = CancelAfter {
        inner: RecordingHooks::new(fixture.trace.clone()),
        cancellation: cancellation.clone(),
        at: crate::topology::effects::EffectSiteId::Container(ContainerSite::Create),
    };

    let error = runner
        .run_blocking_with(
            &request,
            RunnerCall::new(cancellation).observed_in_container_by(&mut observer),
        )
        .expect_err("a start that fails reports an error");
    assert!(
        error.is_cancelled(),
        "a launch that fails once a cancellation is requested reports the cancellation: {error}"
    );
    assert_eq!(
        error.fate,
        ProcessFate::Gone,
        "with the fate the reclaim established: {error}"
    );
    assert!(fixture.runtime.fake().container_names().is_empty());
    assert!(
        list_intents(&fixture.private_root)
            .expect("scan")
            .is_empty()
    );
}

#[test]
fn an_invocation_whose_removal_leaves_a_process_unresolved_is_never_reported_gone() {
    for cancelled in [true, false] {
        #[cfg(unix)]
        unresolved_removal_in_an_owned_helper(cancelled);
        #[cfg(not(unix))]
        assert_unresolved_removal_fate(
            &Fixture::new(&format!("unresolved-removal-{cancelled}"), !cancelled),
            cancelled,
        );
    }
}

fn assert_unresolved_removal_fate(fixture: &Fixture, cancelled: bool) {
    let runner = fixture.runner();
    let request = worker_request(
        ShellKind::Sh
            .spec("cat")
            .stdin(prompt("unresolved", 4 * 1024)),
        fixture.task_a.clone(),
        AgentId::new("claude-code"),
        Duration::from_secs(600),
        worker_id(0),
    );
    let name =
        ContainerName::new(repo_key(), RUN_ID, INCARNATION_1, &request.invocation).expect("a name");
    fixture
        .runtime
        .fake()
        .leave_a_process_unresolved_at_removal(name.as_str());
    let cancellation = crate::runner::Cancellation::new();
    let mut observer = CancelAfter {
        inner: RecordingHooks::new(fixture.trace.clone()),
        cancellation: cancellation.clone(),
        at: crate::topology::effects::EffectSiteId::Container(if cancelled {
            ContainerSite::Start
        } else {
            ContainerSite::RemoveIntent
        }),
    };
    let error = runner
        .run_blocking_with(
            &request,
            RunnerCall::new(cancellation).observed_in_container_by(&mut observer),
        )
        .expect_err("an invocation with a process left unresolved is not a success");
    assert!(
        !error.is_cancelled(),
        "cancelled {cancelled}: not a plain cancellation: {error}"
    );
    assert_eq!(
        error.fate,
        ProcessFate::Unresolved,
        "cancelled {cancelled}: never Gone while a process may still run: {error}"
    );
    assert!(
        error
            .to_string()
            .contains("could not establish every process"),
        "cancelled {cancelled}: the unresolved fact is the error: {error}"
    );
}

#[cfg(unix)]
const UNRESOLVED_REMOVAL_PATH: &str = "UPSTROKE_TEST_UNRESOLVED_REMOVAL_PATH";

#[cfg(unix)]
const UNRESOLVED_REMOVAL_ENDING: &str = "UPSTROKE_TEST_UNRESOLVED_REMOVAL_ENDING";

#[cfg(unix)]
fn unresolved_removal_in_an_owned_helper(cancelled: bool) {
    let path = if cancelled { "cancelled" } else { "completed" };
    for ending in ["return", "unwind"] {
        let ended = crate::agent::proc::test_support::run_test_isolated(
            "runner::container::exec::tests::unresolved_removal_child",
            &[
                (UNRESOLVED_REMOVAL_PATH, std::ffi::OsStr::new(path)),
                (UNRESOLVED_REMOVAL_ENDING, std::ffi::OsStr::new(ending)),
            ],
            FIXTURE_CHILD_BOUND,
        );
        assert!(
            ended.status.is_some_and(|status| status.success())
                && ended.stdout.contains("1 passed")
                && ended
                    .stderr
                    .matches("collected retained mock reaper ")
                    .count()
                    == 1
                && ended
                    .stderr
                    .matches("collected isolated job-control guard ")
                    .count()
                    == 1
                && ended
                    .stderr
                    .matches("isolated helper has no direct children (ECHILD)")
                    .count()
                    == 1,
            "{path}/{ending}: the owned helper must prove the unresolved fate and collect its \
             retained reaper before succeeding: {ended}"
        );
        crate::workspace_manager::fixture::say(&format!("removal {path}/{ending}: {ended}"));
    }
}

#[cfg(unix)]
#[test]
#[ignore = "owned helper for the unresolved-removal guard"]
fn unresolved_removal_child() {
    use crate::agent::proc::test_support::{
        IsolatedJobControl, RetainedMockReaper, assert_no_children_in_isolated_test,
    };

    let job_control = IsolatedJobControl::new();
    let path = std::env::var(UNRESOLVED_REMOVAL_PATH).expect("the parent names the removal path");
    let cancelled = match path.as_str() {
        "cancelled" => true,
        "completed" => false,
        _ => panic!("an unknown removal path: {path}"),
    };
    let ending =
        std::env::var(UNRESOLVED_REMOVAL_ENDING).expect("the parent names the helper ending");
    assert!(matches!(ending.as_str(), "return" | "unwind"), "{ending}");
    let fixture = Fixture::new(&format!("unresolved-removal-{path}-{ending}"), !cancelled);
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let job_control = job_control;
        let reaper = RetainedMockReaper::new(&fixture.private_root, INCARNATION_1);
        assert_unresolved_removal_fate(&fixture, cancelled);
        assert!(
            armed_for(&fixture.private_root, INCARNATION_1),
            "the runner's drop exercised the production retained-reaper backstop"
        );
        assert!(
            ending != "unwind",
            "deliberate unwind after retained reaper"
        );
        reaper.collect();
        job_control.collect();
    }));
    if ending == "unwind" {
        let payload = outcome.expect_err("the deliberate unwind exercised guard cleanup");
        assert_eq!(
            payload.downcast_ref::<&str>(),
            Some(&"deliberate unwind after retained reaper")
        );
    } else if let Err(payload) = outcome {
        std::panic::resume_unwind(payload);
    }
    assert_no_children_in_isolated_test();
}

#[test]
fn a_container_call_cancelled_before_it_starts_writes_no_intent() {
    let fixture = Fixture::new("cancel-before", true);
    let runner = fixture.runner();
    let request = worker_request(
        ShellKind::Sh.spec("exit 0"),
        fixture.task_a.clone(),
        AgentId::new("claude-code"),
        Duration::from_secs(60),
        worker_id(1),
    );
    let cancellation = crate::runner::Cancellation::new();
    cancellation.cancel();
    let error = runner
        .run_blocking_with(&request, RunnerCall::new(cancellation))
        .expect_err("a cancelled call reports an error");
    assert!(error.is_cancelled(), "{error}");
    assert_eq!(error.fate, ProcessFate::NeverStarted, "{error}");
    assert!(
        fixture.trace.rendered().is_empty(),
        "nothing was reached: {:#?}",
        fixture.trace.rendered()
    );
    assert!(fixture.runtime.fake().container_names().is_empty());
    assert!(
        list_intents(&fixture.private_root)
            .expect("scan")
            .is_empty()
    );
}

#[cfg(unix)]
const UNARMED_LAUNCHERS: &[&str] = &["launch", "create_container", "start_container"];

#[cfg(unix)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Naming {
    Call,
    Value,
    Import,
    Alias,
    Reexport,
    Field,
    MacroArgument,
}

#[cfg(unix)]
fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

#[cfg(unix)]
fn identifier_occurrences(code: &str, name: &str) -> Vec<usize> {
    let bytes = code.as_bytes();
    code.match_indices(name)
        .map(|(at, _)| at)
        .filter(|at| {
            let before = at.checked_sub(1).and_then(|index| bytes.get(index));
            let after = bytes.get(at + name.len());
            !before.is_some_and(|byte| is_identifier_byte(*byte))
                && !after.is_some_and(|byte| is_identifier_byte(*byte))
        })
        .collect()
}

#[cfg(unix)]
fn ends_with_word(text: &str, word: &str) -> bool {
    text.strip_suffix(word).is_some_and(|before| {
        !before
            .as_bytes()
            .last()
            .is_some_and(|byte| is_identifier_byte(*byte))
    })
}

#[cfg(unix)]
fn declaration_starts_at(code: &str, at: usize) -> bool {
    let head = code[..at].trim_end();
    head.is_empty()
        || head.ends_with([';', '{', '}', ']'])
        || ends_with_word(head, "pub")
        || head.ends_with(')')
}

#[cfg(unix)]
fn use_declaration_of(code: &str, at: usize) -> Option<usize> {
    identifier_occurrences(&code[..at], "use")
        .into_iter()
        .rev()
        .find(|start| !code[*start..at].contains(';') && declaration_starts_at(code, *start))
}

#[cfg(unix)]
fn is_public(code: &str, use_at: usize) -> bool {
    let head = code[..use_at].trim_end();
    if ends_with_word(head, "pub") {
        return true;
    }
    head.strip_suffix(')')
        .and_then(|inside| inside.rfind('(').map(|open| &inside[..open]))
        .is_some_and(|before| ends_with_word(before.trim_end(), "pub"))
}

#[cfg(unix)]
fn macro_arguments(code: &str) -> Vec<(usize, usize)> {
    let bytes = code.as_bytes();
    let mut spans = Vec::new();
    for (bang, _) in code.match_indices('!') {
        let named = bang
            .checked_sub(1)
            .and_then(|index| bytes.get(index))
            .is_some_and(|byte| is_identifier_byte(*byte));
        let rest = &code[bang + 1..];
        let open = bang + 1 + (rest.len() - rest.trim_start().len());
        if !named || !matches!(bytes.get(open), Some(b'(' | b'[' | b'{')) {
            continue;
        }
        let mut depth = 0_usize;
        for (offset, byte) in bytes.iter().enumerate().skip(open) {
            match byte {
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        spans.push((open, offset));
                        break;
                    }
                }
                _ => {}
            }
        }
    }
    spans
}

#[cfg(unix)]
fn inside_a_macro_argument(spans: &[(usize, usize)], at: usize) -> bool {
    spans.iter().any(|(open, close)| *open < at && at < *close)
}

#[cfg(unix)]
fn namings(code: &str, name: &str) -> Vec<(Naming, usize)> {
    let macros = macro_arguments(code);
    let mut found = Vec::new();
    for at in identifier_occurrences(code, name) {
        if inside_a_macro_argument(&macros, at) {
            found.push((Naming::MacroArgument, at));
            continue;
        }
        let before = code[..at].trim_end();
        let after = code[at + name.len()..].trim_start();
        if ends_with_word(before, "fn") || before.ends_with('.') {
            continue;
        }
        let kind = match use_declaration_of(code, at) {
            Some(use_at) if is_public(code, use_at) => Naming::Reexport,
            Some(_) if ends_with_word(after.split_whitespace().next().unwrap_or(""), "as") => {
                Naming::Alias
            }
            Some(_) => Naming::Import,
            None if after.starts_with('(') => Naming::Call,
            None if after.starts_with(':') && !after.starts_with("::") => Naming::Field,
            None => Naming::Value,
        };
        found.push((kind, at));
    }
    found
}

#[cfg(unix)]
fn primitive_namings(code: &str, name: &str) -> Vec<(String, usize)> {
    let macros = macro_arguments(code);
    let mut found = Vec::new();
    for at in identifier_occurrences(code, name) {
        let before = code[..at].trim_end();
        let after = code[at + name.len()..].trim_start();
        let in_a_macro = inside_a_macro_argument(&macros, at);
        if !in_a_macro && ends_with_word(before, "fn") {
            continue;
        }
        if !in_a_macro && after.starts_with("::") && !after.starts_with("::<") {
            continue;
        }
        let called = after.starts_with('(');
        let pathed = before.ends_with("::");
        let colon = after.starts_with(':') && !after.starts_with("::");
        if !(in_a_macro || colon || called || pathed || use_declaration_of(code, at).is_some()) {
            continue;
        }
        let argument = after
            .strip_prefix('(')
            .and_then(|arguments| arguments.split(')').next())
            .map(str::trim);
        if !in_a_macro && called && matches!(argument, Some("true" | "false")) {
            continue;
        }
        let separator = if before.ends_with("::") {
            "::"
        } else if before.ends_with('.') {
            "."
        } else {
            ""
        };
        let path = &before[..before.len() - separator.len()];
        let receiver = if separator.is_empty() {
            ""
        } else {
            let start = path
                .rfind(|character: char| !(character.is_alphanumeric() || character == '_'))
                .map_or(0, |end| end + 1);
            &path[start..]
        };
        let next = after.chars().next().map(String::from).unwrap_or_default();
        let spelled = format!("{receiver}{separator}{name}{next}");
        found.push((
            if in_a_macro {
                format!("{spelled} in a macro argument")
            } else {
                spelled
            },
            at,
        ));
    }
    found
}

#[cfg(unix)]
fn stated_disallowed_methods(source: &str) -> Option<bool> {
    let blanked = crate::effects::blank_comments(source);
    let mut rest = blanked.trim_start();
    let mut allowed = false;
    let mut guarded = false;
    while let Some(after) = rest.strip_prefix("#![") {
        let mut depth = 1_usize;
        let mut end = None;
        for (offset, character) in after.char_indices() {
            match character {
                '[' => depth += 1,
                ']' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(offset);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(end) = end else {
            break;
        };
        let attribute: String = after[..end].split_whitespace().collect();
        rest = after[end + 1..].trim_start();
        if !attribute.contains("clippy::disallowed_methods") {
            continue;
        }
        let (applies, level) = match attribute.strip_prefix("cfg_attr(") {
            Some(conditional) if conditional.starts_with("test,") => continue,
            Some(conditional) => match conditional.strip_prefix("not(test),") {
                Some(level) => (true, level.to_owned()),
                None => (false, conditional.to_owned()),
            },
            None => (true, attribute.clone()),
        };
        if level.starts_with("allow(") {
            allowed = true;
        } else if applies {
            guarded = true;
        }
    }
    if allowed {
        Some(false)
    } else if guarded {
        Some(true)
    } else {
        None
    }
}

#[cfg(unix)]
fn parent_module(src: &Path, module: &Path) -> Option<PathBuf> {
    let relative = module.strip_prefix(src).ok()?;
    let mut segments: Vec<String> = relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect();
    if segments.last().is_some_and(|last| last == "mod.rs") {
        segments.pop();
    } else if let Some(last) = segments.last_mut() {
        *last = last.trim_end_matches(".rs").to_owned();
    }
    if segments == ["lib"] || segments == ["main"] {
        return None;
    }
    segments.pop();
    if segments.is_empty() {
        return Some(src.join("lib.rs"));
    }
    let parent = segments.join("/");
    let found = [
        src.join(format!("{parent}.rs")),
        src.join(&parent).join("mod.rs"),
    ]
    .into_iter()
    .find(|candidate| candidate.is_file());
    assert!(
        found.is_some(),
        "the parent module of {} is neither {parent}.rs nor {parent}/mod.rs, so its lint level \
         cannot be read",
        module.display()
    );
    found
}

#[cfg(unix)]
fn clippy_refuses_the_primitives_in(src: &Path, file: &Path) -> bool {
    let mut module = Some(file.to_path_buf());
    while let Some(current) = module {
        let source = std::fs::read_to_string(&current).expect("a module source");
        if let Some(guarded) = stated_disallowed_methods(&source) {
            return guarded;
        }
        module = parent_module(src, &current);
    }
    true
}

#[cfg(unix)]
fn enclosing_fn(code: &str, at: usize) -> Option<String> {
    let bytes = code.as_bytes();
    identifier_occurrences(code, "fn")
        .into_iter()
        .filter(|start| *start < at)
        .rev()
        .find_map(|start| {
            let name = code[start + 2..].trim_start();
            let end = name
                .find(|character: char| !(character.is_alphanumeric() || character == '_'))
                .unwrap_or(name.len());
            if end == 0 {
                return None;
            }
            let open = start + code[start..].find('{')?;
            let mut depth = 0_usize;
            let mut close = None;
            for (offset, byte) in bytes.iter().enumerate().skip(open) {
                match byte {
                    b'{' => depth += 1,
                    b'}' => {
                        depth = depth.checked_sub(1)?;
                        if depth == 0 {
                            close = Some(offset);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            (open < at && close.is_some_and(|close| at < close)).then(|| name[..end].to_owned())
        })
}

#[cfg(unix)]
#[test]
fn the_naming_reader_names_an_alias_a_function_value_and_a_re_export_of_an_unarmed_launcher() {
    let snippet = "\
use super::launch as start_uncovered;
pub use super::create_container as create_uncovered;
pub(crate) use super::start_container;
use super::{mount_git_view, start_container, write_intent};
use super::*;
fn bypass(hooks: &mut dyn ContainerHooks) {
    let uncovered = super::start_container;
    uncovered(hooks);
    launch(hooks, runtime, view, plan);
    crate::runner::container::create_container(hooks);
}
fn launch(&self, plan: &InvocationPlan) {
    self.launch(hooks);
    let spec = &plan.launch.spec;
    let built = InvocationPlan { launch: spec.clone() };
}
";
    let mut read: Vec<(&str, Naming, usize)> = Vec::new();
    for name in UNARMED_LAUNCHERS {
        for (kind, at) in namings(snippet, name) {
            read.push((name, kind, snippet[..at].lines().count()));
        }
    }
    read.sort();
    let mut expected = vec![
        ("launch", Naming::Alias, 1),
        ("create_container", Naming::Reexport, 2),
        ("start_container", Naming::Reexport, 3),
        ("start_container", Naming::Import, 4),
        ("start_container", Naming::Value, 7),
        ("launch", Naming::Call, 9),
        ("create_container", Naming::Call, 10),
        ("launch", Naming::Field, 15),
    ];
    expected.sort();
    assert_eq!(
        read, expected,
        "the reader names every alias, re-export, import, function value, call and field \
         initializer of an unarmed launcher, and no definition, method call or field access"
    );
    assert_eq!(
        enclosing_fn(
            snippet,
            snippet.find("launch(hooks, runtime").expect("the call")
        ),
        Some("bypass".to_owned())
    );
}

#[cfg(unix)]
#[test]
fn the_naming_reader_names_a_launcher_inside_a_macro_argument_whatever_follows_it() {
    let snippet = "\
macro_rules! uncovered_delegate {
    ($function:ident: $($argument:expr),* $(,)?) => {
        super::$function($($argument),*)
    };
}
pub fn delta_uncovered(hooks: &mut dyn ContainerHooks, plan: &LaunchPlan) -> bool {
    uncovered_delegate!(launch: hooks, runtime, view, plan).is_ok()
}
fn braces(hooks: &mut dyn ContainerHooks) {
    uncovered_delegate! { start_container: hooks, site, runtime, intent };
    spliced![plan.launch, hooks];
    let unchanged = a != (b);
}
struct InvocationPlan {
    launch: LaunchPlan,
}
";
    let mut read: Vec<(&str, Naming, usize)> = Vec::new();
    for name in UNARMED_LAUNCHERS {
        for (kind, at) in namings(snippet, name) {
            read.push((name, kind, snippet[..at].lines().count()));
        }
    }
    read.sort();
    let mut expected = vec![
        ("launch", Naming::MacroArgument, 7),
        ("start_container", Naming::MacroArgument, 10),
        ("launch", Naming::MacroArgument, 11),
        ("launch", Naming::Field, 15),
    ];
    expected.sort();
    assert_eq!(
        read, expected,
        "a macro can splice any identifier of its argument anywhere, so the reader names every \
         occurrence of an unarmed launcher inside a macro's delimiters -- `name:`, `.name` and \
         all -- and a field declaration outside one; `!=` opens no macro"
    );
    assert_eq!(
        enclosing_fn(
            snippet,
            snippet.find("launch: hooks").expect("the macro argument")
        ),
        Some("delta_uncovered".to_owned())
    );

    let primitives = "\
fn plant(runtime: &dyn ContainerRuntime) {
    delegate!(start: runtime, \"probe\");
    let options = OpenOptions::new().create(true);
}
struct Span {
    start: usize,
}
";
    let mut spelled: Vec<String> = ["create", "start"]
        .into_iter()
        .flat_map(|name| primitive_namings(primitives, name))
        .map(|(spelled, _)| spelled)
        .collect();
    spelled.sort();
    assert_eq!(
        spelled,
        ["start:", "start: in a macro argument"],
        "the primitives' reader names a `create` or `start` inside a macro argument and a field \
         named after one, and still passes over `OpenOptions::create(true)`"
    );
}

#[cfg(unix)]
#[test]
fn the_lint_level_walk_reads_a_stated_or_inherited_allowance_and_a_production_forbid() {
    let tree = repo::scratch("lint-level-walk");
    let src = tree.path().join("src");
    let write = |relative: &str, text: &str| {
        let path = src.join(relative);
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
        std::fs::write(&path, text).expect("a module");
        path
    };
    write("lib.rs", "pub mod allowing;\npub mod silent;\n");
    write(
        "allowing.rs",
        "//! notes\n\n// a reason\n#![allow(clippy::disallowed_methods)]\n\npub mod inherits;\npub mod forbids;\n",
    );
    let inherits = write("allowing/inherits.rs", "pub fn f() {}\n");
    let forbids = write(
        "allowing/forbids.rs",
        "#![cfg_attr(not(test), forbid(clippy::disallowed_methods))]\npub fn f() {}\n",
    );
    let test_only = write(
        "silent.rs",
        "#![cfg_attr(test, allow(clippy::disallowed_methods))]\npub fn f() {}\n",
    );
    let allowing = src.join("allowing.rs");
    assert_eq!(
        [
            clippy_refuses_the_primitives_in(&src, &allowing),
            clippy_refuses_the_primitives_in(&src, &inherits),
            clippy_refuses_the_primitives_in(&src, &forbids),
            clippy_refuses_the_primitives_in(&src, &test_only),
        ],
        [false, false, true, true],
        "a stated allowance, one a silent child inherits, a production forbid beneath it, and an \
         allowance that holds in tests alone"
    );
}

#[cfg(unix)]
#[test]
fn every_container_start_in_production_is_reached_only_through_a_covered_launch() {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    let mut stack = vec![src.clone()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("src is readable") {
            let path = entry.expect("a directory entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                files.push(path);
            }
        }
    }
    files.sort();
    let test_modules = crate::effects::census_domain::whole_file_test_modules(&src, &files, 13);
    assert!(
        test_modules.contains(
            &src.join("runner")
                .join("container")
                .join("exec")
                .join("tests.rs")
        ),
        "this file is declared `#[cfg(test)] mod tests;` and the scan has to know it"
    );
    let relative = |path: &Path| {
        path.strip_prefix(env!("CARGO_MANIFEST_DIR"))
            .unwrap_or(path)
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/")
    };

    let mut scanned = 0_usize;
    let mut scanned_bytes = 0_usize;
    let mut found: Vec<(String, &str, Naming, Option<String>)> = Vec::new();
    let mut naming_start_container: BTreeSet<String> = BTreeSet::new();
    let mut primitives: Vec<(String, String, Option<String>)> = Vec::new();
    let mut operations: Vec<(String, String, Option<String>)> = Vec::new();
    let mut guarded = 0_usize;
    let mut unguarded: BTreeSet<String> = BTreeSet::new();
    let mut covered_launch = None;
    let mut funnels = None;
    let mut runtime_trait = None;
    let mut test_mints: Vec<(String, Option<String>)> = Vec::new();
    for path in &files {
        if test_modules.contains(path) {
            continue;
        }
        let source = std::fs::read_to_string(path).expect("a source file");
        let production = crate::effects::production_code(&source);
        let dense = production
            .as_bytes()
            .iter()
            .filter(|byte| !byte.is_ascii_whitespace())
            .count();
        assert!(dense > 0, "{}'s region is empty", path.display());
        scanned += 1;
        scanned_bytes += dense;
        let file = relative(path);
        for name in UNARMED_LAUNCHERS {
            for (kind, at) in namings(&production, name) {
                if *name == "start_container" {
                    naming_start_container.insert(file.clone());
                }
                found.push((file.clone(), name, kind, enclosing_fn(&production, at)));
            }
        }
        if clippy_refuses_the_primitives_in(&src, path) {
            guarded += 1;
        } else {
            unguarded.insert(file.clone());
            for primitive in ["create", "start"] {
                for (spelled, at) in primitive_namings(&production, primitive) {
                    primitives.push((file.clone(), spelled, enclosing_fn(&production, at)));
                }
            }
        }
        for operation in ["RuntimeOp::Create", "RuntimeOp::Start"] {
            for (at, _) in production.match_indices(operation) {
                operations.push((
                    file.clone(),
                    operation.to_owned(),
                    enclosing_fn(&production, at),
                ));
            }
        }
        for at in identifier_occurrences(&production, "without_a_reaper") {
            test_mints.push((file.clone(), enclosing_fn(&production, at)));
        }
        match file.as_str() {
            "src/runner/container/exec.rs" => covered_launch = Some(production),
            "src/runner/container.rs" => funnels = Some(production),
            "src/runner/container/runtime.rs" => runtime_trait = Some(production),
            _ => {}
        }
    }
    assert!(scanned > 40, "the walk found only {scanned} source files");
    assert!(
        scanned_bytes > 750_000,
        "the {scanned} regions hold {scanned_bytes} non-whitespace bytes between them"
    );

    found.sort();
    let mut expected = vec![
        (
            "src/runner/container/exec.rs".to_owned(),
            "create_container",
            Naming::Import,
            None,
        ),
        (
            "src/runner/container/exec.rs".to_owned(),
            "start_container",
            Naming::Import,
            None,
        ),
        (
            "src/runner/container/exec.rs".to_owned(),
            "create_container",
            Naming::Call,
            Some("launch".to_owned()),
        ),
        (
            "src/runner/container/exec.rs".to_owned(),
            "start_container",
            Naming::Call,
            Some("launch".to_owned()),
        ),
        (
            "src/runner/container/exec.rs".to_owned(),
            "launch",
            Naming::Field,
            None,
        ),
        (
            "src/runner/container/exec.rs".to_owned(),
            "launch",
            Naming::Field,
            Some("plan".to_owned()),
        ),
    ];
    expected.sort();
    assert_eq!(
        found, expected,
        "a production region names an unarmed launcher — the free `launch`, `create_container` or \
         `start_container` — outside the two funnels that may: by a call, an import, an alias, a \
         function value, a re-export, a field or anywhere inside a macro's argument. Every \
         container must be started through `ContainerRunner::launch`, which only `contain` calls \
         with the cover its reaper armed; the free `launch` is test-only, `mod uncovered`"
    );
    assert_eq!(
        naming_start_container,
        ["src/runner/container/exec.rs"]
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>()
    );

    primitives.sort();
    let mut expected_primitives: Vec<(String, String, Option<String>)> = [
        (
            "src/agent/codex.rs",
            "MissingOutputSchema::create(",
            "validate_effort_config_key",
        ),
        (
            "src/agent/proc.rs",
            "Drain::start(",
            "run_with_timeout_and_limit",
        ),
        (
            "src/agent/proc.rs",
            "Drain::start(",
            "run_with_timeout_and_limit",
        ),
        (
            "src/agent/proc.rs",
            "Feeder::start(",
            "run_with_timeout_and_limit",
        ),
        (
            "src/agent/proc.rs",
            "Job::create(",
            "spawn_suspended_in_job_with",
        ),
        ("src/agent/proc.rs", "Job::create,", "create_ambient"),
        ("src/agent/proc.rs", "create(", "create_with"),
        (
            "src/engine/resume.rs",
            "paths.create(",
            "resume_harness_inner_on",
        ),
        ("src/agent/proc.rs", "create:", ""),
        ("src/export.rs", "start:", ""),
        ("src/export.rs", "start:", ""),
        ("src/util.rs", "start. in a macro argument", "tail"),
        ("src/runner/container.rs", "File::create(", "write_synced"),
        (
            "src/runner/container.rs",
            "runtime.create(",
            "create_container",
        ),
        (
            "src/runner/container.rs",
            "runtime.start(",
            "start_container",
        ),
        (
            "src/runner/container/view.rs",
            "File::create(",
            "write_file",
        ),
        ("src/workspace.rs", ".create(", "create_private_dir"),
        (
            "src/workspace.rs",
            "PendingGateWorkspace::create(",
            "gate_snapshot_for_candidate_in_with_mode",
        ),
        (
            "src/workspace.rs",
            "PrivateHooksDir::create(",
            "git_output_with_input",
        ),
        (
            "src/workspace.rs",
            "PrivateHooksDir::create(",
            "run_git_with_private_hooks",
        ),
        ("src/workspace.rs", "builder.create(", "create_private_dir"),
    ]
    .into_iter()
    .map(|(file, spelled, function)| {
        (
            file.to_owned(),
            spelled.to_owned(),
            (!function.is_empty()).then(|| function.to_owned()),
        )
    })
    .collect();
    expected_primitives.sort();
    assert!(
        guarded > 100
            && unguarded.contains("src/runner/container.rs")
            && unguarded.contains("src/runner/container/view.rs")
            && !unguarded.contains("src/runner/container/exec.rs")
            && !unguarded.contains("src/runner/container/census.rs")
            && !unguarded.contains("src/engine/topology/attempt.rs"),
        "the lint-level walk read {guarded} modules where clippy refuses the runtime's primitives \
         and these where it cannot: {unguarded:?}; the funnel and the Git view allow the lint, \
         `exec.rs` and `census.rs` forbid it, and `engine/topology/attempt.rs` inherits \
         `engine/topology.rs`'s forbid"
    );
    assert_eq!(
        primitives, expected_primitives,
        "a production module where clippy cannot refuse `ContainerRuntime::create` or `::start` — \
         its effective `clippy::disallowed_methods` level is `allow`, stated or inherited — names a \
         `create` or `start` call or path this list does not hold. If it is not a container \
         runtime's, add it here; if it is, it starts a container no cover dominates"
    );
    operations.sort();
    assert_eq!(
        operations,
        vec![
            (
                "src/runner/container.rs".to_owned(),
                "RuntimeOp::Create".to_owned(),
                Some("create".to_owned())
            ),
            (
                "src/runner/container.rs".to_owned(),
                "RuntimeOp::Create".to_owned(),
                Some("create".to_owned())
            ),
            (
                "src/runner/container.rs".to_owned(),
                "RuntimeOp::Create".to_owned(),
                Some("create".to_owned())
            ),
            (
                "src/runner/container.rs".to_owned(),
                "RuntimeOp::Create".to_owned(),
                Some("create".to_owned())
            ),
            (
                "src/runner/container.rs".to_owned(),
                "RuntimeOp::Start".to_owned(),
                Some("start".to_owned())
            ),
        ],
        "the real runtime's `docker create` and `docker start` are reached only from its \
         `ContainerRuntime::create` and `::start`"
    );

    assert!(
        test_mints.is_empty(),
        "a production region names `without_a_reaper`, the test-only mint of a cover's proofs: \
         {test_mints:?}"
    );
    let signature = |code: &str, definition: &str| {
        let definitions: Vec<usize> = code.match_indices(definition).map(|(at, _)| at).collect();
        assert_eq!(definitions.len(), 1, "one `{definition}`");
        let end = definitions[0]
            + code[definitions[0]..]
                .find(['{', ';'])
                .expect("a signature ends");
        code[definitions[0]..end].to_owned()
    };
    let funnels = funnels.expect("container.rs is scanned");
    let runtime_trait = runtime_trait.expect("runtime.rs is scanned");
    for (code, definition, proof) in [
        (&funnels, "fn create_container(", "CoveredCreate<"),
        (&funnels, "fn start_container(", "CoveredStart<"),
        (&runtime_trait, "fn create(", "CoveredCreate<"),
        (&runtime_trait, "fn start(", "CoveredStart<"),
    ] {
        assert!(
            signature(code, definition).contains(proof),
            "`{definition}` no longer takes `{proof}…>`: a start primitive that takes no proof \
             can be called with no cover, whatever spelling reaches it"
        );
    }

    let exec = covered_launch.expect("exec.rs is scanned");
    let launch = signature(&exec, "fn launch(");
    assert!(
        launch.contains("CoveredCreate<") && launch.contains("CoveredStart<"),
        "`ContainerRunner::launch` takes the two proofs the cover minted"
    );
    let launches: Vec<usize> = exec
        .match_indices("self.launch(")
        .map(|(at, _)| at)
        .collect();
    assert_eq!(launches.len(), 1, "one call of the covered launch");
    assert_eq!(
        enclosing_fn(&exec, launches[0]).as_deref(),
        Some("contain"),
        "the covered launch is called from `contain`"
    );
    let covers: Vec<usize> = exec
        .match_indices(".cover(")
        .map(|(at, _)| at)
        .filter(|at| enclosing_fn(&exec, *at).as_deref() == Some("contain"))
        .collect();
    assert!(
        covers.len() == 1 && covers[0] < launches[0],
        "`contain` covers its invocation before it calls the launch"
    );
    for construction in ["Covered {", "CoveredCreate {", "CoveredStart {"] {
        let made: Vec<usize> = exec.match_indices(construction).map(|(at, _)| at).collect();
        assert!(
            made.len() == 1 && enclosing_fn(&exec, made[0]).as_deref() == Some("cover"),
            "`{construction}…}}` is built only by the cover that armed the reaper, and \
             exec.rs builds it {} time(s)",
            made.len()
        );
    }
    let mint = exec.find("mod cover {").expect("the cover's module");
    let mint_end = block_end(&exec, mint + "mod cover ".len()).expect("the module closes");
    assert!(
        identifier_occurrences(&exec[mint..mint_end], "mod").len() == 1,
        "the module that mints the proofs declares no module of its own in production: a child \
         module sees its private fields"
    );
}

#[cfg(unix)]
fn reaper_spec(private_root: &Path, incarnation: &str) -> CreateSpec {
    CreateSpec {
        name: format!("upstroke-reaper-scope-{incarnation}"),
        image_id: IMAGE_ID.to_owned(),
        labels: [
            (
                LABEL_PRIVATE_ROOT.to_owned(),
                crate::runner::container::intent::private_root_label(private_root),
            ),
            (LABEL_INCARNATION.to_owned(), incarnation.to_owned()),
        ]
        .into_iter()
        .collect(),
        mounts: Vec::new(),
        env: Vec::new(),
        command: Vec::new(),
        workdir: None,
        read_only_root: true,
        user: None,
        open_stdin: false,
    }
}

#[cfg(unix)]
fn armed_for(private_root: &Path, incarnation: &str) -> bool {
    crate::agent::proc::armed_container_reaper_selects(
        &crate::runner::container::intent::private_root_label(private_root),
        incarnation,
    )
}

#[cfg(unix)]
#[test]
fn a_scope_that_does_not_select_its_containers_labels_is_refused_with_no_reaper_armed() {
    let fixture = Fixture::built_in_a_bounded_child("reaper-scope-refused", true);
    let reaping = Reaping::default();
    let foreign = reaper_spec(&fixture.private_root, INCARNATION_2);

    let refused = reaping
        .cover(
            &fixture.runtime,
            &fixture.identity,
            &foreign,
            &[],
            Instant::now() + Duration::from_secs(60),
            &crate::runner::Cancellation::new(),
            &[],
            &crate::rundir::worktree_lock_file(&fixture.repo.join(".git")),
        )
        .err()
        .expect("a container its runner's scope does not select is refused");
    assert!(
        refused.to_string().contains("would not find it"),
        "the refusal says why: {refused}"
    );
    {
        let state = reaping.state.lock().unwrap_or_else(PoisonError::into_inner);
        assert!(
            state.armed.is_none() && state.in_flight == 0 && !state.unsettled,
            "the refused cover armed nothing and counted nothing"
        );
    }
    assert!(
        !armed_for(&fixture.private_root, INCARNATION_1)
            && !armed_for(&fixture.private_root, INCARNATION_2),
        "no reaper was forked for either scope: nothing arms until the scope is validated"
    );

    let own = reaper_spec(&fixture.private_root, INCARNATION_1);
    let (covered, _, _) = reaping
        .cover(
            &fixture.runtime,
            &fixture.identity,
            &own,
            &[],
            Instant::now() + Duration::from_secs(60),
            &crate::runner::Cancellation::new(),
            &[],
            &crate::rundir::worktree_lock_file(&fixture.repo.join(".git")),
        )
        .expect("the runner's own container is covered");
    assert!(
        armed_for(&fixture.private_root, INCARNATION_1),
        "the control: a container the scope selects arms the runner's reaper"
    );
    covered.settle(ProcessFate::NeverStarted);
    drop(reaping);
    assert!(
        !armed_for(&fixture.private_root, INCARNATION_1),
        "every cover settled with an established end, so dropping the runner disarmed it"
    );
}

#[cfg(unix)]
#[test]
fn a_container_whose_labels_differ_from_the_armed_scope_is_refused_before_its_intent() {
    let fixture = Fixture::built_in_a_bounded_child("reaper-scope-armed-foreign", true);
    let runner = fixture.runner();
    let foreign = RunIdentity {
        incarnation: INCARNATION_2.to_owned(),
        ..fixture.identity.clone()
    };
    let armed_for_another = reaper_spec(&fixture.private_root, INCARNATION_2);
    let (covered, _, _) = runner
        .reaping
        .cover(
            &fixture.runtime,
            &foreign,
            &armed_for_another,
            &[],
            Instant::now() + Duration::from_secs(60),
            &crate::runner::Cancellation::new(),
            &[],
            &crate::rundir::worktree_lock_file(&fixture.repo.join(".git")),
        )
        .expect("a reaper armed for another incarnation, as a caller-built one would have been");
    covered.settle(ProcessFate::NeverStarted);
    assert!(armed_for(&fixture.private_root, INCARNATION_2));

    let request = gate_request(
        ShellKind::Sh.spec("exit 0"),
        fixture.task_a.clone(),
        Duration::from_secs(10),
        gate_id(0),
    );
    let refused = runner
        .run_blocking(&request)
        .expect_err("the runner's container is not selected by the armed scope");
    assert_eq!(refused.fate, ProcessFate::NeverStarted, "{refused}");
    assert!(
        refused.to_string().contains("would not find it"),
        "the refusal says why: {refused}"
    );
    assert!(
        list_intents(&fixture.private_root)
            .expect("the container namespace is readable")
            .is_empty(),
        "no intent was written for a container its reaper would not find"
    );
    assert!(
        !fixture
            .runtime
            .fake()
            .journal()
            .iter()
            .any(|entry| entry.op == RuntimeOp::Create),
        "nothing was created: {:?}",
        fixture.runtime.fake().journal()
    );
    assert!(
        !armed_for(&fixture.private_root, INCARNATION_1),
        "and no second reaper was armed for the runner's own scope"
    );
}

#[cfg(unix)]
#[test]
fn a_runner_whose_container_is_unresolved_keeps_its_reaper_armed_past_its_drop() {
    let fixture = Fixture::built_in_a_bounded_child("reaper-unresolved-kept", false);
    for op in [RuntimeOp::Observe, RuntimeOp::Stop, RuntimeOp::Remove] {
        fixture.runtime.fake().set_unreachable(op);
    }
    let runner = fixture.runner();
    let request = gate_request(
        ShellKind::Sh.spec("exit 0"),
        fixture.task_a.clone(),
        Duration::from_secs(10),
        gate_id(0),
    );
    let error = runner
        .run_blocking(&request)
        .expect_err("the runtime cannot observe, stop or remove the container");
    assert_eq!(error.fate, ProcessFate::Unresolved, "{error}");
    assert!(armed_for(&fixture.private_root, INCARNATION_1));
    drop(runner);
    assert!(
        armed_for(&fixture.private_root, INCARNATION_1),
        "a runner whose container may still run leaves its reaper armed past its last handle, \
         until the process exits"
    );
}

#[cfg(unix)]
#[test]
fn a_runner_whose_containers_all_ended_disarms_its_reaper_at_its_drop() {
    let fixture = Fixture::built_in_a_bounded_child("reaper-ended-disarmed", true);
    let runner = fixture.runner();
    let completed = gate_request(
        ShellKind::Sh.spec("exit 0"),
        fixture.task_a.clone(),
        Duration::from_secs(10),
        gate_id(0),
    );
    runner
        .run_blocking(&completed)
        .expect("the container ran and was released");
    fixture.runtime.fake().set_unreachable(RuntimeOp::Observe);
    let released = gate_request(
        ShellKind::Sh.spec("exit 0"),
        fixture.task_a.clone(),
        Duration::from_secs(10),
        gate_id(1),
    );
    let error = runner
        .run_blocking(&released)
        .expect_err("the observation is lost and the release completes");
    assert_eq!(error.fate, ProcessFate::Gone, "{error}");
    assert!(armed_for(&fixture.private_root, INCARNATION_1));
    drop(runner);
    assert!(
        !armed_for(&fixture.private_root, INCARNATION_1),
        "every container the runner started is established gone, so its drop disarmed the reaper"
    );
}

#[cfg(unix)]
#[test]
fn a_panic_inside_contain_leaves_the_reaper_armed() {
    use crate::topology::effects::{EffectSiteId, HookPhase, Injection};

    struct PanicsAfterStart;

    impl ContainerHooks for PanicsAfterStart {
        fn phase(&mut self, site: EffectSiteId, phase: HookPhase) -> Injection {
            assert!(
                !(site == EffectSiteId::Container(ContainerSite::Start)
                    && phase == HookPhase::After),
                "a panic inside `contain`, after the container started"
            );
            Injection::Proceed
        }
    }

    let fixture = Fixture::built_in_a_bounded_child("reaper-panic-kept", false);
    let runner = fixture.runner().with_hooks(Box::new(PanicsAfterStart));
    let request = gate_request(
        ShellKind::Sh.spec("exit 0"),
        fixture.task_a.clone(),
        Duration::from_secs(10),
        gate_id(0),
    );
    let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        runner.run_blocking(&request)
    }));
    assert!(unwound.is_err(), "the hook panicked inside the invocation");
    drop(runner);
    assert!(
        armed_for(&fixture.private_root, INCARNATION_1),
        "a container whose invocation unwound has no established end, so the reaper stays armed"
    );
}

#[cfg(unix)]
const REAPER_TESTS: &[(&str, &[&str])] = &[
    (
        "src/agent/proc.rs",
        &[
            "run_isolated",
            "a_container_reaper_is_armed_only_at_the_terminate_site",
            "container_reaper_whose_cancellation_fails_child",
            "a_container_reapers_failed_cancellation_ends_its_caller_through_the_signal_monitor",
            "container_reaper_stopped_before_its_cancellation_child",
            "a_stopped_container_reaper_ends_its_caller_rather_than_releasing_it",
            "container_reaper_lease_child",
            "an_armed_container_reaper_holds_no_cleanup_lease",
            "container_reaper_stopped_after_its_acknowledgement_child",
            "a_container_reaper_stopped_after_acknowledging_its_cancel_does_not_wedge_its_caller",
        ],
    ),
    (
        "src/engine/topology/coordinator.rs",
        &[
            "reaper_finished",
            "reclaimed_by_its_reaper",
            "a_resume_killed_inside_its_pre_flight_probe",
            "a_resuming_incarnations_pre_flight_probe_container_is_killed_by_its_reaper_when_the_coordinator_dies_inside_it",
            "the_reapers_scope_is_the_runners_identity_and_selects_its_first_probe",
            "stranded_child",
            "a_runner_whose_containers_are_unresolved_keeps_its_reaper_armed_past_its_last_handle_until_the_process_exits",
            "a_contained_run_with_its_reaper_relayed",
            "released_by_their_own_runner_and_never_reaped",
            "a_coordinator_that_ends_disarms_its_reaper_and_kills_nothing_at_width_three",
            "a_runner_whose_containers_all_ended_disarms_its_reaper_and_the_relay_is_never_called",
            "every_container_an_incarnation_starts_is_covered_by_an_armed_reaper_with_its_scope",
        ],
    ),
    (
        "src/engine/topology/create/tests.rs",
        &[
            "fresh_p4_probe_child",
            "a_fresh_runs_p4_probe_container_is_killed_by_its_reaper_before_run_started",
            "every_container_a_fresh_runs_creation_starts_is_covered_by_an_armed_reaper_with_its_scope",
        ],
    ),
    (
        "src/runner/container/exec/tests.rs",
        &[
            "a_scope_that_does_not_select_its_containers_labels_is_refused_with_no_reaper_armed",
            "a_container_whose_labels_differ_from_the_armed_scope_is_refused_before_its_intent",
            "a_runner_whose_container_is_unresolved_keeps_its_reaper_armed_past_its_drop",
            "a_runner_whose_containers_all_ended_disarms_its_reaper_at_its_drop",
            "a_panic_inside_contain_leaves_the_reaper_armed",
        ],
    ),
];

#[cfg(unix)]
fn block_end(code: &str, open: usize) -> Option<usize> {
    let mut depth = 0_usize;
    for (offset, byte) in code.as_bytes().iter().enumerate().skip(open) {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(offset);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(unix)]
fn negated_hold_reads_outside_a_bounded_wait(body: &str) -> Vec<String> {
    let mut unbounded = Vec::new();
    for read in ["observe_cleanup_hold", "is_running"] {
        for at in identifier_occurrences(body, read) {
            if !body[at + read.len()..].trim_start().starts_with('(') {
                continue;
            }
            let path = body[..at].trim_end_matches(|character: char| {
                character.is_alphanumeric() || character == '_' || character == ':'
            });
            if !path.trim_end().ends_with('!') {
                continue;
            }
            let bounded = ["loop", "while"].iter().any(|keyword| {
                identifier_occurrences(&body[..at], keyword)
                    .into_iter()
                    .filter_map(|start| {
                        let open = start + body[start..].find('{')?;
                        let end = block_end(body, open)?;
                        (open < at && at < end).then_some(&body[start..end])
                    })
                    .any(|block| block.contains("deadline") || block.contains("elapsed()"))
            });
            if !bounded {
                let line = body[..at].lines().count();
                unbounded.push(format!("`!{read}(` at line {line} of the body"));
            }
        }
    }
    unbounded
}

#[cfg(unix)]
#[test]
fn no_reaper_test_reads_a_hold_as_released_once() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut scanned = 0_usize;
    let mut negated_reads = 0_usize;
    let mut violations = Vec::new();
    for (file, functions) in REAPER_TESTS {
        let source = crate::effects::blank_comments(
            &std::fs::read_to_string(root.join(file)).expect("a source"),
        );
        for function in *functions {
            let definitions: Vec<usize> = source
                .match_indices(&format!("fn {function}("))
                .map(|(at, _)| at)
                .collect();
            assert_eq!(
                definitions.len(),
                1,
                "`{function}` is defined once in {file}, so this census reads it"
            );
            let open = definitions[0] + source[definitions[0]..].find('{').expect("a body");
            let end = block_end(&source, open).expect("a closed body");
            let body = &source[definitions[0]..=end];
            scanned += 1;
            negated_reads += ["observe_cleanup_hold", "is_running"]
                .iter()
                .map(|read| identifier_occurrences(body, read).len())
                .sum::<usize>();
            for unbounded in negated_hold_reads_outside_a_bounded_wait(body) {
                violations.push(format!("{file}: `{function}`: {unbounded}"));
            }
        }
    }
    assert!(scanned >= 25, "the census read {scanned} functions");
    assert!(
        negated_reads >= 1,
        "the census found no hold read at all, so it is reading nothing"
    );
    assert!(
        violations.is_empty(),
        "a reaper test reads a cleanup hold as released with one read: a sibling thread's fork \
         holds an inherited lease descriptor for a moment \
         (`PR281-CLEANUP-LEASE-HOLD-OUTLIVED-AND-ITS-UNREADABLE-TWIN`), so every `not held` read \
         polls within a bound: {violations:#?}"
    );
    assert_eq!(
        negated_hold_reads_outside_a_bounded_wait(
            "fn once() { assert!(!crate::rundir::observe_cleanup_hold(&public, &mut NoHooks)); }"
        )
        .len(),
        1,
        "the reader names a one-shot read"
    );
    assert!(
        negated_hold_reads_outside_a_bounded_wait(
            "fn polled() { loop { if !observe_cleanup_hold(&public, &mut NoHooks) { return; } \
             if Instant::now() >= deadline { return; } } }"
        )
        .is_empty(),
        "and accepts a read polled within a deadline"
    );
}

#[test]
fn an_observation_that_leaves_a_process_unresolved_keeps_the_invocation_unresolved_after_a_clean_release()
 {
    #[cfg(unix)]
    unresolved_process_in_an_owned_helper("observation");
    #[cfg(not(unix))]
    assert_unresolved_observation_fate(&Fixture::new("unresolved-observation", false));
}

fn assert_unresolved_observation_fate(fixture: &Fixture) {
    use crate::error::ProcessFate;

    fixture.runtime.leaves_its_observation_unresolved();
    let request = gate_request(
        ShellKind::Sh.spec("exit 0"),
        fixture.task_a.clone(),
        Duration::from_secs(10),
        gate_id(0),
    );

    let error = fixture
        .runner()
        .run_blocking(&request)
        .expect_err("the supervision's observation left a process unresolved");
    assert_eq!(error.fate, ProcessFate::Unresolved, "{error}");
    assert!(
        error
            .to_string()
            .contains(ProcessFate::Unresolved.describe()),
        "{error}"
    );
    let ops = fixture.trace.ops();
    assert!(
        ops.contains(&RuntimeOp::Start)
            && ops.contains(&RuntimeOp::Remove)
            && !ops.contains(&RuntimeOp::Collect),
        "the container started, nothing was collected, and the release removed it: {ops:?}"
    );
    assert_eq!(
        fixture.runtime.fake().container_names(),
        Vec::<String>::new(),
        "the release completed, and the invocation is still unresolved"
    );
}

#[test]
fn a_prestart_failure_whose_cleanup_leaves_a_process_unresolved_keeps_the_invocation_unresolved() {
    #[cfg(unix)]
    unresolved_process_in_an_owned_helper("prestart-cleanup");
    #[cfg(not(unix))]
    assert_unresolved_prestart_cleanup_fate(&Fixture::new("unresolved-prestart-cleanup", false));
}

#[test]
fn an_unresolved_credential_volume_inspection_keeps_the_invocation_unresolved_and_held() {
    #[cfg(unix)]
    unresolved_process_in_an_owned_helper("volume-inspection");
    #[cfg(not(unix))]
    assert_unresolved_volume_inspection_fate(&Fixture::new("unresolved-volume-inspection", false));
}

struct ProbeEnd {
    fate: crate::error::ProcessFate,
    cancelled: bool,
    message: String,
    view_retained: bool,
    intent_retained: bool,
    registration_kept: bool,
    slot_held: bool,
    spawn_outage: bool,
}

fn agent_probe_named(ordinal: u32) -> (RunnerRequest, ContainerName) {
    let request = crate::agent::probe_request(
        "claude-code",
        ShellKind::Sh.spec("exit 0"),
        ordinal,
        Duration::from_secs(10),
    )
    .expect("an agent probe request");
    let name = ContainerName::new(repo_key(), RUN_ID, INCARNATION_1, &request.invocation)
        .expect("a container name");
    (request, name)
}

fn run_admitted(
    fixture: &Fixture,
    runner: &ContainerRunner,
    request: &RunnerRequest,
    name: &ContainerName,
) -> ProbeEnd {
    use crate::engine::topology::identity::{
        InvocationEnd, InvocationLedger, SlotLimits, SlotPair,
    };
    use crate::engine::topology::integrate::Verified;
    use crate::engine::topology::select::Standing;
    use crate::topology::events::{InfrastructureKind, SequenceId};

    let mut ledger =
        InvocationLedger::with_limits(SlotLimits::new(1, 1).expect("one slot per agent"));
    ledger
        .register_slotted(
            &request.invocation,
            SlotPair {
                agent: "claude-code".to_owned(),
                pool: None,
            },
            &Standing::preflight(),
        )
        .expect("the probe is admitted");
    assert!(
        ledger.slots().holds(&request.invocation),
        "the premise: the probe holds its slot while it runs"
    );
    let outcome = runner.run_blocking(request);
    ledger
        .end(&request.invocation, &InvocationEnd::of(&outcome))
        .expect("the ledger takes the end");
    let error = outcome.expect_err("the launch failed before any start");
    let (fate, cancelled, message) = (error.fate, error.is_cancelled(), error.to_string());
    let spawn_outage = matches!(
        crate::engine::topology::run::verified(
            Err(crate::engine::topology::attempt::JudgeError::Runner(error)),
            Vec::new(),
            SequenceId(0),
        ),
        Ok(Verified::Unavailable {
            kind: InfrastructureKind::RunnerSpawnFailure,
            ..
        })
    );
    ProbeEnd {
        fate,
        cancelled,
        message,
        view_retained: view_dir(&fixture.private_root, name).exists(),
        intent_retained: name.intent_path(&fixture.private_root).exists(),
        registration_kept: !ledger.settled(&request.invocation),
        slot_held: ledger.slots().holds(&request.invocation),
        spawn_outage,
    }
}

fn assert_settled_and_released(case: &str, end: &ProbeEnd) {
    use crate::error::ProcessFate;

    assert_eq!(
        end.fate,
        ProcessFate::NeverStarted,
        "{case}: {}",
        end.message
    );
    assert!(
        !end.view_retained && !end.intent_retained,
        "{case}: the cancel released the R19 view and the R26 intent: {}",
        end.message
    );
    assert!(
        !end.registration_kept && !end.slot_held,
        "{case}: the ledger released the registration and its slot"
    );
    assert!(
        end.spawn_outage,
        "{case}: a settled pre-start failure is the outage verification settles"
    );
}

fn assert_unresolved_and_held(case: &str, end: &ProbeEnd) {
    use crate::error::ProcessFate;

    assert_eq!(end.fate, ProcessFate::Unresolved, "{case}: {}", end.message);
    assert!(
        !end.cancelled,
        "{case}: not a plain cancellation: {}",
        end.message
    );
    assert!(
        end.view_retained && end.intent_retained,
        "{case}: the R19 view and the R26 intent are kept for the census: {}",
        end.message
    );
    assert!(
        end.registration_kept && end.slot_held,
        "{case}: the ledger keeps the registration and its slot"
    );
    assert!(
        !end.spawn_outage,
        "{case}: no RunnerSpawnFailure outage is settled over an unresolved process"
    );
    assert!(
        end.message.contains(ProcessFate::Unresolved.describe())
            && end.message.contains("deliberately retained"),
        "{case}: the error names the unresolved fate and the residue it keeps: {}",
        end.message
    );
}

fn assert_unresolved_prestart_cleanup_fate(fixture: &Fixture) {
    let runner = fixture.runner();

    let (control, control_name) = agent_probe_named(0);
    fixture
        .runtime
        .fake()
        .substitute_reported_image_id(control_name.as_str(), OTHER_IMAGE_ID);
    let settled = run_admitted(fixture, &runner, &control, &control_name);
    assert_settled_and_released("a mismatch whose cleanup is clean", &settled);

    for (ordinal, case, primary) in [
        (
            1,
            "a reported image id that differs from the record",
            "reports image id",
        ),
        (2, "a create the runtime refuses", "armed failing"),
    ] {
        let (request, name) = agent_probe_named(ordinal);
        if ordinal == 1 {
            fixture
                .runtime
                .fake()
                .substitute_reported_image_id(name.as_str(), OTHER_IMAGE_ID);
        } else {
            fixture.runtime.fake().set_failing(RuntimeOp::Create);
        }
        fixture
            .runtime
            .fake()
            .leave_a_process_unresolved_at_removal(name.as_str());
        let end = run_admitted(fixture, &runner, &request, &name);
        assert_unresolved_and_held(case, &end);
        assert!(
            end.message.contains(primary) && end.message.contains("unresolved at its removal"),
            "{case}: the primary failure and the cleanup's own fact are both reported: {}",
            end.message
        );
        assert!(
            fixture.runtime.fake().container(name.as_str()).is_none(),
            "{case}: the cleanup removed what the runtime created; only a process is unknown"
        );
    }
    assert!(
        !fixture.trace.ops().contains(&RuntimeOp::Start),
        "no invocation here reached `docker start`: {:?}",
        fixture.trace.ops()
    );
}

fn assert_unresolved_volume_inspection_fate(fixture: &Fixture) {
    let volume = "upstroke-creds-claude";
    let context = MountContext::bind_with(
        &fixture.repo,
        fixtures::plain_checkout(&fixture.repo.join(".git")),
        &fixture.private_root,
        &[],
    )
    .expect("the fixture binds a mount context");
    let selection = crate::config::RunnerSelection {
        kind: RunnerKind::Container,
        image: Some(IMAGE_REFERENCE.to_owned()),
        credential_volumes: recorded_volumes(&container_policy()),
        mounts: Vec::new(),
        from_config: true,
    };
    let resolved = crate::runner::container::resolve::resolve_container(
        &fixture.runtime,
        &selection,
        &context,
    )
    .expect("the run's preflight finds every recorded credential volume");
    assert_eq!(
        resolved.credential_volumes,
        container_policy().credential_volumes,
        "the premise: the initial resolution recorded the volumes the runner mounts"
    );
    let runner = fixture.runner();

    fixture.runtime.fake().remove_volume(volume);
    let (absent, absent_name) = agent_probe_named(0);
    let refused = run_admitted(fixture, &runner, &absent, &absent_name);
    assert_settled_and_released("an absent volume", &refused);
    assert!(
        refused
            .message
            .contains("is not present in the container runtime"),
        "{}",
        refused.message
    );
    fixture.runtime.fake().add_volume(volume);

    fixture.runtime.fake().set_failing(RuntimeOp::InspectVolume);
    let (failed, failed_name) = agent_probe_named(1);
    let failure = run_admitted(fixture, &runner, &failed, &failed_name);
    assert_settled_and_released("an inspection that failed", &failure);

    fixture.runtime.leaves_its_volume_inspections_unresolved();
    let (request, name) = agent_probe_named(2);
    let end = run_admitted(fixture, &runner, &request, &name);
    assert_unresolved_and_held("an unresolved inspection", &end);
    assert!(
        end.message
            .contains("could not be asked whether the credential volume")
            && end.message.contains(volume),
        "the contextual R20 refusal is kept beside the fate: {}",
        end.message
    );
    let ops = fixture.trace.ops();
    assert!(
        !ops.contains(&RuntimeOp::Create)
            && ops.contains(&RuntimeOp::Stop)
            && ops.contains(&RuntimeOp::Remove),
        "no container was ever created, and each cancel's stop and removal were asked: {ops:?}"
    );
    assert!(
        fixture.runtime.fake().container_names().is_empty(),
        "the runtime established the never-created container absent, which settles nothing \
         about the inspection's process"
    );
}

#[cfg(unix)]
const UNRESOLVED_PROCESS_CASE: &str = "UPSTROKE_TEST_UNRESOLVED_PROCESS_CASE";

#[cfg(unix)]
const UNRESOLVED_PROCESS_ENDING: &str = "UPSTROKE_TEST_UNRESOLVED_PROCESS_ENDING";

#[cfg(unix)]
fn unresolved_process_in_an_owned_helper(case: &str) {
    for ending in ["return", "unwind"] {
        let ended = crate::agent::proc::test_support::run_test_isolated(
            "runner::container::exec::tests::unresolved_process_child",
            &[
                (UNRESOLVED_PROCESS_CASE, std::ffi::OsStr::new(case)),
                (UNRESOLVED_PROCESS_ENDING, std::ffi::OsStr::new(ending)),
            ],
            FIXTURE_CHILD_BOUND,
        );
        assert!(
            ended.status.is_some_and(|status| status.success())
                && ended.stdout.contains("1 passed")
                && ended
                    .stderr
                    .matches("collected retained mock reaper ")
                    .count()
                    == 1
                && ended
                    .stderr
                    .matches("collected isolated job-control guard ")
                    .count()
                    == 1
                && ended
                    .stderr
                    .matches("isolated helper has no direct children (ECHILD)")
                    .count()
                    == 1,
            "{case}/{ending}: the owned helper must prove the fate and collect its retained \
             reaper before succeeding: {ended}"
        );
        crate::workspace_manager::fixture::say(&format!("{case}/{ending}: {ended}"));
    }
}

#[cfg(unix)]
#[test]
#[ignore = "owned helper for the unresolved start, observation, pre-start cleanup and volume \
            inspection guards"]
fn unresolved_process_child() {
    use crate::agent::proc::test_support::{
        IsolatedJobControl, RetainedMockReaper, assert_no_children_in_isolated_test,
    };

    let job_control = IsolatedJobControl::new();
    let case =
        std::env::var(UNRESOLVED_PROCESS_CASE).expect("the parent names the unresolved case");
    let ending =
        std::env::var(UNRESOLVED_PROCESS_ENDING).expect("the parent names the helper ending");
    assert!(
        matches!(
            case.as_str(),
            "start" | "observation" | "prestart-cleanup" | "volume-inspection"
        ),
        "{case}"
    );
    assert!(matches!(ending.as_str(), "return" | "unwind"), "{ending}");
    let fixture = Fixture::new(&format!("unresolved-{case}-{ending}"), false);
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let job_control = job_control;
        let reaper = RetainedMockReaper::new(&fixture.private_root, INCARNATION_1);
        match case.as_str() {
            "start" => assert_unresolved_start_fate(&fixture),
            "observation" => assert_unresolved_observation_fate(&fixture),
            "prestart-cleanup" => assert_unresolved_prestart_cleanup_fate(&fixture),
            "volume-inspection" => assert_unresolved_volume_inspection_fate(&fixture),
            _ => panic!("an unknown unresolved-process case: {case}"),
        }
        assert!(
            armed_for(&fixture.private_root, INCARNATION_1),
            "the runner's drop exercised the production retained-reaper backstop"
        );
        assert!(
            ending != "unwind",
            "deliberate unwind after retained reaper"
        );
        reaper.collect();
        job_control.collect();
    }));
    if ending == "unwind" {
        let payload = outcome.expect_err("the deliberate unwind exercised guard cleanup");
        assert_eq!(
            payload.downcast_ref::<&str>(),
            Some(&"deliberate unwind after retained reaper")
        );
    } else if let Err(payload) = outcome {
        std::panic::resume_unwind(payload);
    }
    assert_no_children_in_isolated_test();
}

#[cfg(unix)]
const STALLED_OBSERVATION_MODE: &str = "UPSTROKE_TEST_STALLED_OBSERVATION";

#[cfg(unix)]
const STALLED_OBSERVATION_ENDING: &str = "UPSTROKE_TEST_STALLED_OBSERVATION_ENDING";

#[cfg(unix)]
const STALLED_OBSERVATION_MARKER: &str = "UPSTROKE_TEST_OBSERVED";

#[cfg(unix)]
const STALLED_OBSERVATION_IMAGE: &str = "UPSTROKE_TEST_IMAGE";

#[cfg(unix)]
const STALLED_OBSERVATION_STUB: &str = "UPSTROKE_TEST_STALLED_OBSERVATION_STUB";

#[cfg(unix)]
const STALLED_CLEANUP: &str = "UPSTROKE_TEST_CLEANUP_STALLS";

#[cfg(unix)]
const STALLED_START_HELD: &str = "UPSTROKE_TEST_START_HELD";

#[cfg(unix)]
const STALLED_OBSERVATION_DEADLINE: Duration = Duration::from_secs(10);

#[cfg(unix)]
const STALLED_OBSERVATION_SETTLES: Duration = Duration::from_secs(20);

#[cfg(unix)]
const STALLED_OBSERVATION_DOCKER: &str = "#!/bin/sh\n\
    case \"$1\" in\n\
    ps)\n\
    \x20 case \"$*\" in\n\
    \x20 *\"filter name=\"*) : > \"$UPSTROKE_TEST_OBSERVED\"; exec sleep 60 ;;\n\
    \x20 *) exit 0 ;;\n\
    \x20 esac ;;\n\
    create) exit 0 ;;\n\
    start) if [ -n \"$UPSTROKE_TEST_START_HELD\" ]; then sleep \"$UPSTROKE_TEST_START_HELD\"; fi; exit 0 ;;\n\
    container)\n\
    \x20 case \"$*\" in\n\
    \x20 *.Image*) printf '%s\\n' \"$UPSTROKE_TEST_IMAGE\" ;;\n\
    \x20 *) printf '0\\n' ;;\n\
    \x20 esac ;;\n\
    logs) printf 'what the container wrote\\n' ;;\n\
    stop|rm) if [ -n \"$UPSTROKE_TEST_CLEANUP_STALLS\" ]; then exec sleep 60; fi; exit 0 ;;\n\
    *) exit 93 ;;\n\
    esac\n";

#[cfg(unix)]
#[test]
fn a_running_container_whose_observation_stalls_settles_at_its_deadline_or_cancellation_and_keeps_its_fate()
 {
    let tree = repo::scratch("stalled-observation");
    let bin = tree.path().join("bin");
    std::fs::create_dir(&bin).expect("the stub's directory");
    let stub = bin.join("docker");
    crate::runner::container::write_program_in_its_own_process(&stub, STALLED_OBSERVATION_DOCKER);
    let inherited = std::env::var_os("PATH").unwrap_or_default();
    let path = std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(&inherited)))
        .expect("a PATH with the stub first");
    for mode in ["deadline", "cancel", "cleanup"] {
        let (retained, disarmed) = if mode == "cleanup" { (1, 0) } else { (0, 1) };
        for ending in ["return", "unwind"] {
            let observed = tree.path().join(format!("observed-{mode}-{ending}"));
            let mut vars = vec![
                ("PATH", path.as_os_str()),
                (STALLED_OBSERVATION_MODE, std::ffi::OsStr::new(mode)),
                (STALLED_OBSERVATION_ENDING, std::ffi::OsStr::new(ending)),
                (STALLED_OBSERVATION_MARKER, observed.as_os_str()),
                (STALLED_OBSERVATION_IMAGE, std::ffi::OsStr::new(IMAGE_ID)),
                (STALLED_OBSERVATION_STUB, stub.as_os_str()),
            ];
            if mode == "cleanup" {
                vars.push((STALLED_CLEANUP, std::ffi::OsStr::new("1")));
                vars.push((STALLED_START_HELD, std::ffi::OsStr::new("2")));
            }
            let ended = crate::agent::proc::test_support::run_test_isolated(
                "runner::container::exec::tests::stalled_observation_child",
                &vars,
                Duration::from_secs(120),
            );
            assert!(
                ended.status.is_some_and(|status| status.success())
                    && ended.stdout.contains("1 passed")
                    && ended
                        .stderr
                        .matches("collected retained stub reaper ")
                        .count()
                        == retained
                    && ended
                        .stderr
                        .matches("disarmed its container reaper on drop")
                        .count()
                        == disarmed
                    && ended
                        .stderr
                        .matches("collected isolated job-control guard ")
                        .count()
                        == 1
                    && ended
                        .stderr
                        .matches("isolated helper has no direct children (ECHILD)")
                        .count()
                        == 1,
                "{mode}/{ending}: the stalled observation held the invocation, it settled with \
                 another fate, or the owned helper did not collect its retained reaper and its \
                 job-control guard before succeeding: {ended}"
            );
            crate::workspace_manager::fixture::say(&format!("{mode}/{ending}: {ended}"));
        }
    }
}

#[cfg(unix)]
#[test]
#[ignore = "owned helper of the stalled-observation witness"]
fn stalled_observation_child() {
    use crate::agent::proc::test_support::{
        IsolatedJobControl, RetainedStubReaper, assert_no_children_in_isolated_test,
    };

    let job_control = IsolatedJobControl::new();
    let mode = std::env::var(STALLED_OBSERVATION_MODE).expect("the parent names the mode");
    let ending =
        std::env::var(STALLED_OBSERVATION_ENDING).expect("the parent names the helper ending");
    assert!(
        matches!(mode.as_str(), "deadline" | "cancel" | "cleanup"),
        "{mode}"
    );
    assert!(matches!(ending.as_str(), "return" | "unwind"), "{ending}");
    let observed = PathBuf::from(
        std::env::var_os(STALLED_OBSERVATION_MARKER).expect("the parent names the marker"),
    );
    let stub = PathBuf::from(
        std::env::var_os(STALLED_OBSERVATION_STUB).expect("the parent names its stub"),
    );
    let held = std::env::var(STALLED_START_HELD).ok().map(|seconds| {
        Duration::from_secs(
            seconds
                .parse()
                .expect("the parent holds the start for whole seconds"),
        )
    });
    let fixture = Fixture::new(&format!("stalled-observation-{mode}-{ending}"), false);
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let job_control = job_control;
        let docker = crate::runner::container::DockerCli::new(ContainerTrace::off());
        let reaper = RetainedStubReaper::new(
            &fixture.private_root,
            INCARNATION_1,
            &docker.reaper_program(),
            &stub,
        );
        assert_a_stalled_observation_settles(&fixture, &mode, &observed, held, docker);
        let armed = armed_for(&fixture.private_root, INCARNATION_1);
        if mode == "cleanup" {
            assert!(
                armed,
                "the unresolved runner's drop exercised the production retained-reaper backstop"
            );
        } else {
            assert!(
                !armed,
                "{mode}: the settled runner's drop disarmed its container reaper"
            );
            crate::workspace_manager::fixture::say(&format!(
                "{mode}: the settled runner disarmed its container reaper on drop"
            ));
        }
        assert!(
            ending != "unwind",
            "deliberate unwind after the runner dropped"
        );
        if mode == "cleanup" {
            reaper.collect();
        }
        job_control.collect();
    }));
    if ending == "unwind" {
        let payload = outcome.expect_err("the deliberate unwind exercised guard cleanup");
        assert_eq!(
            payload.downcast_ref::<&str>(),
            Some(&"deliberate unwind after the runner dropped")
        );
    } else if let Err(payload) = outcome {
        std::panic::resume_unwind(payload);
    }
    assert_no_children_in_isolated_test();
}

#[cfg(unix)]
fn assert_a_stalled_observation_settles(
    fixture: &Fixture,
    mode: &str,
    observed: &Path,
    held: Option<Duration>,
    docker: crate::runner::container::DockerCli,
) {
    use crate::error::ProcessFate;
    use std::sync::atomic::{AtomicBool, Ordering};

    let docker = if mode == "cleanup" {
        docker.with_question_bound(Duration::from_secs(1))
    } else {
        docker
    };
    let runner = ContainerRunner::new(
        container_policy(),
        fixture.identity.clone(),
        &fixture.repo,
        fixtures::plain_checkout(&fixture.repo.join(".git")),
        image_environment(),
        Box::new(docker),
    )
    .expect("a container policy");
    let mut request =
        host::shell_probe_request(ShellKind::Sh, fixture.task_a.clone(), shell_probe_id());
    let settles_within = if mode == "cancel" {
        request.timeout = Duration::from_secs(600);
        STALLED_OBSERVATION_SETTLES
    } else {
        request.timeout = STALLED_OBSERVATION_DEADLINE;
        STALLED_OBSERVATION_DEADLINE + STALLED_OBSERVATION_SETTLES
    };
    let cancellation = Cancellation::new();
    let ended = AtomicBool::new(false);
    let began = Instant::now();
    let (ran, asked_after) = std::thread::scope(|scope| {
        let watcher = scope.spawn(|| {
            let until = Instant::now() + Duration::from_secs(60);
            let asked = loop {
                let finished = ended.load(Ordering::SeqCst);
                if observed.exists() {
                    break Some(began.elapsed());
                }
                if finished || Instant::now() >= until {
                    break None;
                }
                crate::workspace_manager::fixture::rest(Duration::from_millis(10));
            };
            if mode == "cancel" {
                cancellation.cancel();
            }
            asked
        });
        let ran = runner.run_blocking_with(&request, RunnerCall::new(cancellation.clone()));
        ended.store(true, Ordering::SeqCst);
        (ran, watcher.join().expect("the observation watcher"))
    });
    let took = began.elapsed();
    assert!(
        observed.exists(),
        "{mode}: the premise, the started container's observation was asked: the invocation \
         returned {ran:?} after {took:?} under a {:?} timeout without asking it",
        request.timeout
    );
    if let Some(held) = held {
        assert!(
            asked_after.is_some_and(|after| after >= held),
            "{mode}: the launch was held {held:?} at `docker start`, and its observation was \
             asked after {asked_after:?}"
        );
    }
    assert!(
        took < settles_within,
        "{mode}: the stalled `docker` held the invocation {took:?}, past {settles_within:?}"
    );
    crate::workspace_manager::fixture::say(&format!(
        "{mode}: the observation was asked after {asked_after:?} and the invocation settled \
         after {took:?}, under a {:?} timeout",
        request.timeout
    ));
    match mode {
        "deadline" => {
            let output = ran.expect("a timed-out invocation reports its output");
            assert!(output.timed_out && output.code.is_none(), "{output:?}");
        }
        "cancel" => {
            let error = ran.expect_err("a cancelled invocation reports an error");
            assert!(error.is_cancelled(), "{error}");
            assert_eq!(error.fate, ProcessFate::Gone, "{error}");
        }
        _ => {
            let error = ran.expect_err("a release whose questions stalled reports an error");
            assert_eq!(error.fate, ProcessFate::Unresolved, "{error}");
            assert!(
                !list_intents(&fixture.private_root)
                    .expect("the container namespace reads")
                    .is_empty(),
                "the intent is kept for the census that reclaims the container"
            );
        }
    }
}

#[test]
fn review_timeouts_obey_the_platform_deadline_range() {
    let tree = repo::scratch("review-deadline-helper");
    let executable = std::env::current_exe().expect("the test binary");
    let command = crate::runner::CommandSpec::new(
        executable
            .to_str()
            .expect("the test binary has a UTF-8 path"),
    )
    .arg("runner::container::exec::tests::review_deadline_child")
    .arg("--exact")
    .arg("--ignored")
    .arg("--nocapture")
    .arg("--test-threads=1");
    let request = gate_request(
        command,
        tree.path().to_path_buf(),
        Duration::from_secs(120),
        gate_id(0),
    );
    let output = host::HostRunner::new()
        .run_blocking(&request)
        .expect("supervise the review deadline helper");
    assert!(
        !output.timed_out,
        "the review deadline helper exceeded its bound: {output:?}"
    );
    assert_eq!(output.code, Some(0), "{output:?}");
    assert!(
        output.stdout.contains("1 passed"),
        "the helper ran its assertions: {output:?}"
    );
}

#[test]
#[ignore = "owned helper for the bounded review deadline guard"]
fn review_deadline_child() {
    use crate::error::ProcessFate;

    let tree = repo::scratch("review-deadline-config");
    let configuration = tree.path().join("upstroke.toml");
    std::fs::write(
        &configuration,
        "[routing]\nreview = { tier = \"frontier\", timeout_secs = 9223372036854775807 }\n",
    )
    .expect("the configuration");
    let pools = tree.path().join("pools.toml");
    std::fs::write(&pools, "# no pools\n").expect("an empty pools file");
    let mut warnings = Vec::new();
    let config = crate::config::load(
        Some(&configuration),
        tree.path(),
        Some(&pools),
        &mut warnings,
    )
    .expect("the configuration accepts the review timeout");
    let plan = crate::engine::topology::settle::tests::plan();
    let chains: Vec<_> = plan
        .tasks
        .iter()
        .map(|task| crate::route::resolve(task, &config))
        .collect();
    let timeout = crate::review::plan_for(
        &plan,
        &chains,
        &config,
        |agent| agent == "claude-code",
        &mut warnings,
    )
    .expect("the review plan resolves")
    .pass_timeout()
    .expect("and accepts the timeout");
    assert_eq!(
        timeout,
        Duration::from_secs(9_223_372_036_854_775_807),
        "the premise: both readers accept the configured timeout"
    );

    for (case, timeout, representable) in [
        (
            "configured",
            timeout,
            Instant::now().checked_add(timeout).is_some(),
        ),
        ("representable", Duration::from_secs(60), true),
        ("unrepresentable", Duration::MAX, false),
    ] {
        assert_eq!(
            Instant::now().checked_add(timeout).is_some(),
            representable,
            "{case}: the platform holds the intended deadline premise"
        );
        let fixture = Fixture::new(&format!("review-deadline-{case}"), true);
        let request = review_request(
            ShellKind::Sh.spec("exit 0"),
            fixture.task_a.clone(),
            AgentId::new("claude-code"),
            timeout,
            InvocationId::attempt(
                TaskKey(0),
                GenerationId(0),
                AttemptNumber(1),
                AttemptRole::ReviewPass(0),
                0,
            ),
        );
        let runner = fixture.runner().with_poll(Duration::from_millis(10));
        let Ok(ran) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            runner.run_blocking(&request)
        })) else {
            panic!("{case}: the runner unwound on a review timeout");
        };
        if representable {
            let output = ran.expect("a representable review deadline permits the invocation");
            assert_eq!(output.code, Some(0), "{case}: {output:?}");
            assert!(!output.timed_out, "{case}: {output:?}");
            let trace = fixture.trace.rendered();
            for operation in ["rt:start:", "rt:collect:", "rt:remove:"] {
                assert!(
                    trace.iter().any(|entry| entry.starts_with(operation)),
                    "{case}: {trace:?}"
                );
            }
        } else {
            let error =
                ran.expect_err("an invocation whose deadline no instant can hold is refused");
            assert_eq!(error.fate, ProcessFate::NeverStarted, "{case}: {error}");
            assert!(!error.is_cancelled(), "{case}: {error}");
            assert!(
                error
                    .to_string()
                    .contains(&format!("{} s", timeout.as_secs())),
                "{case}: the refusal names the timeout: {error}"
            );
            assert_eq!(
                fixture.trace.rendered(),
                Vec::<String>::new(),
                "{case}: nothing was asked of the funnel or the runtime"
            );
        }
        assert!(
            list_intents(&fixture.private_root)
                .expect("the container namespace reads")
                .is_empty(),
            "{case}: no intent remains"
        );
        assert!(
            fixture.runtime.fake().container_names().is_empty(),
            "{case}: no container remains"
        );
    }
}

// ---------------------------------------------------------------------------
// `PR12-INV9-CONTAINER-HELPER-CLEANUP-LEASE`: a killed coordinator's Docker
// helpers keep run admission refused until they are settled
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
const LEASE_KILL_CASE: &str = "UPSTROKE_TEST_LEASE_KILL_CASE";

#[cfg(target_os = "linux")]
const LEASE_KILL_ROOT: &str = "UPSTROKE_TEST_LEASE_KILL_ROOT";

#[cfg(target_os = "linux")]
const LEASE_KILL_ACK: &str = "UPSTROKE_TEST_LEASE_KILL_ACK";

/// How long each Unix reaper of the killed coordinator takes over its
/// settlement after it has ended its group (the reaper's own test-build delay,
/// `UPSTROKE_TEST_CLEANUP_DELAY_MS`): the window in which the contender must
/// find admission refused while the killed coordinator's reapers are still
/// settling.
#[cfg(target_os = "linux")]
const LEASE_KILL_SETTLEMENT_DELAY_MS: &str = "3000";

/// The `docker` on the killed coordinator's `PATH`. It pauses at an
/// acknowledged boundary of one bounded runtime question of the worker's
/// invocation -- the synchronous `docker create` (`worker-create`); the
/// attached client's `docker start --attach`, on the thread the attached start
/// spawns (`attached-start`); the credential volume's `docker volume inspect`,
/// which `create_container` asks before any create and so before any
/// container name exists (`credential-volume`); or the `docker stop` of the
/// cancellation that follows a create refused before it was asked
/// (`pre-create-cancel`, whose volume inspection the stub will not answer) --
/// by writing its pid to the acknowledgement path (a temporary name, then
/// `mv`) and becoming a long `sleep`. Otherwise `container inspect` answers the
/// recorded image id, `ps` answers `created` for the observed name and nothing
/// for a backstop's label listing, and every other verb succeeds with nothing
/// to say.
#[cfg(target_os = "linux")]
fn lease_kill_stub(case: &str) -> String {
    let pause = "printf '%s' \"$$\" > \"$UPSTROKE_TEST_LEASE_KILL_ACK.tmp\" && \
                 mv \"$UPSTROKE_TEST_LEASE_KILL_ACK.tmp\" \"$UPSTROKE_TEST_LEASE_KILL_ACK\" && \
                 exec sleep 600";
    let unanswered = "printf '%s\\n' 'Error response from daemon: the lease-kill stub does not \
                      answer this question' >&2; exit 1";
    let (volume, create, start, stop) = match case {
        "worker-create" => ("exit 0", pause, "exit 0", "exit 0"),
        "attached-start" => ("exit 0", "exit 0", pause, "exit 0"),
        "credential-volume" => (pause, "exit 0", "exit 0", "exit 0"),
        "pre-create-cancel" => (unanswered, "exit 0", "exit 0", pause),
        other => panic!("unknown lease-kill case `{other}`"),
    };
    format!(
        "#!/bin/sh\n\
         case \"$1\" in\n\
         volume) {volume} ;;\n\
         create) {create} ;;\n\
         start) {start} ;;\n\
         stop) {stop} ;;\n\
         container) printf '%s\\n' '{IMAGE_ID}' ;;\n\
         ps) for argument in \"$@\"; do case \"$argument\" in name=*) printf '%s\\037created\\n' \"${{argument#name=}}\" ;; esac; done ;;\n\
         *) exit 0 ;;\n\
         esac\n"
    )
}

/// One process of the lease-kill fixture, as it was identified when this
/// harness first found it: its pid, its start time (the clock ticks
/// `/proc/<pid>/stat` records, which a reused pid would not share), its
/// command name, and its process group.
#[cfg(target_os = "linux")]
#[derive(Debug, Clone, PartialEq, Eq)]
struct Identified {
    pid: libc::pid_t,
    start: u64,
    comm: String,
    pgrp: libc::pid_t,
}

/// `/proc/<pid>/stat` of a process this harness found through its own
/// children lists, or one the kernel named ended: its identity and its state
/// letter. A zombie keeps its record, start time included, until collected.
#[cfg(target_os = "linux")]
fn stat_of(pid: libc::pid_t) -> Result<(Identified, char), String> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .map_err(|error| format!("/proc/{pid}/stat: {error}"))?;
    let open = stat
        .find('(')
        .ok_or_else(|| format!("/proc/{pid}/stat: no `(`"))?;
    let (comm, rest) = stat
        .get(open + 1..)
        .and_then(|after| after.rsplit_once(')'))
        .ok_or_else(|| format!("/proc/{pid}/stat: no `)`"))?;
    let fields: Vec<&str> = rest.split_whitespace().collect();
    // After the command: state(0) ppid(1) pgrp(2) ... starttime(19).
    let field = |index: usize| {
        fields
            .get(index)
            .copied()
            .ok_or_else(|| format!("/proc/{pid}/stat: field {index} is missing"))
    };
    let state = field(0)?.chars().next().unwrap_or('?');
    let pgrp = field(2)?
        .parse::<libc::pid_t>()
        .map_err(|error| format!("/proc/{pid}/stat: pgrp: {error}"))?;
    let start = field(19)?
        .parse::<u64>()
        .map_err(|error| format!("/proc/{pid}/stat: starttime: {error}"))?;
    Ok((
        Identified {
            pid,
            start,
            comm: comm.to_owned(),
            pgrp,
        },
        state,
    ))
}

/// The children of `pid`, read from the children list of each of its tasks
/// (`/proc/<pid>/task/<tid>/children`). An unreadable list is an error, never
/// an answer of "no children"; and a list read empty is discovery only, never
/// evidence that no child remains.
#[cfg(target_os = "linux")]
fn children_of(pid: &str) -> Result<Vec<libc::pid_t>, String> {
    let task_dir = Path::new("/proc").join(pid).join("task");
    let gone = |error: &std::io::Error| {
        error.kind() == std::io::ErrorKind::NotFound || error.raw_os_error() == Some(libc::ESRCH)
    };
    children_in_passes(
        || {
            // A task is its id and its start time, since ids are reused. The
            // start time is the twentieth field after the command, which
            // closes at the last `)`. A task whose `stat` is gone is left out.
            let mut listed = BTreeSet::new();
            for task in std::fs::read_dir(&task_dir)? {
                let id = task?.file_name();
                let stat = task_dir.join(&id).join("stat");
                let text = match std::fs::read_to_string(&stat) {
                    Ok(text) => text,
                    Err(error) if gone(&error) => continue,
                    Err(error) => return Err(error),
                };
                let start = text
                    .rsplit_once(')')
                    .and_then(|(_, rest)| rest.split_whitespace().nth(19))
                    .ok_or_else(|| {
                        std::io::Error::other(format!("{}: no starttime field", stat.display()))
                    })?
                    .parse::<u64>()
                    .map_err(|error| {
                        std::io::Error::other(format!("{}: starttime: {error}", stat.display()))
                    })?;
                listed.insert((id, start));
            }
            Ok(listed)
        },
        |(task, _): &(std::ffi::OsString, u64)| {
            std::fs::read_to_string(task_dir.join(task).join("children"))
        },
        |(task, _): &(std::ffi::OsString, u64)| {
            // The state is the first field after the command, which closes at
            // the last `)`. A zombie (`Z`) or dead (`X`, `x`) task is ending,
            // and so is one whose `stat` is gone.
            let stat = task_dir.join(task).join("stat");
            match std::fs::read_to_string(&stat) {
                Ok(text) => {
                    let state = text
                        .rsplit_once(')')
                        .and_then(|(_, rest)| rest.split_whitespace().next())
                        .ok_or_else(|| {
                            std::io::Error::other(format!("{}: no state field", stat.display()))
                        })?;
                    Ok(!matches!(state, "Z" | "X" | "x"))
                }
                Err(error) if gone(&error) => Ok(false),
                Err(error) => Err(error),
            }
        },
        std::thread::sleep,
    )
    .map_err(|error| format!("{}: {error}", task_dir.display()))
}

/// The rule of [`children_of`], with its readers passed in; the notes describe each.
#[cfg(target_os = "linux")]
fn children_in_passes<T: Ord + std::fmt::Debug>(
    mut list_tasks: impl FnMut() -> std::io::Result<BTreeSet<T>>,
    mut read_children: impl FnMut(&T) -> std::io::Result<String>,
    mut running_normally: impl FnMut(&T) -> std::io::Result<bool>,
    mut pause: impl FnMut(std::time::Duration),
) -> Result<Vec<libc::pid_t>, String> {
    const PASSES: usize = 5;
    // The pause before the second pass, the third, the fourth and the fifth:
    // growing, so that the passes span long enough for a thread that is
    // ending, and still listed in its dead state, to be released.
    const PAUSES: [std::time::Duration; PASSES - 1] = [
        std::time::Duration::from_millis(1),
        std::time::Duration::from_millis(5),
        std::time::Duration::from_millis(20),
        std::time::Duration::from_millis(50),
    ];
    let mut last = String::new();
    for pass in 0..PASSES {
        if let Some(wait) = pass.checked_sub(1).and_then(|before| PAUSES.get(before)) {
            pause(*wait);
        }
        let before = list_tasks().map_err(|error| error.to_string())?;
        let mut children = Vec::new();
        let mut changed = Vec::new();
        for task in &before {
            match read_children(task) {
                Ok(text) => {
                    for child in text.split_whitespace() {
                        let pid = child.parse::<libc::pid_t>().map_err(|error| {
                            format!("the children list of task {task:?}: {child}: {error}")
                        })?;
                        children.push(pid);
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    changed.push(format!(
                        "the children list of task {task:?} was not found ({error})"
                    ));
                }
                Err(error) => return Err(format!("the children list of task {task:?}: {error}")),
            }
        }
        for task in &before {
            match running_normally(task) {
                Ok(true) => {}
                Ok(false) => changed.push(format!(
                    "task {task:?} was not running normally after the reads"
                )),
                Err(error) => return Err(format!("the state of task {task:?}: {error}")),
            }
        }
        let after = list_tasks().map_err(|error| error.to_string())?;
        if after != before {
            changed.push(format!(
                "the tasks listed after the reads, {after:?}, were not those listed before them, \
                 {before:?}"
            ));
        }
        if changed.is_empty() {
            children.sort_unstable();
            children.dedup();
            return Ok(children);
        }
        last = changed.join(", and ");
    }
    Err(format!(
        "none of {PASSES} passes over the tasks counted; in the last, {last}"
    ))
}

#[cfg(target_os = "linux")]
#[test]
fn an_engines_children_are_not_lost_when_a_task_ends_after_its_list_was_opened() {
    // B ends after its list was opened and hands its child to A, already read.
    let mut listings = [
        BTreeSet::from(["A", "B"]),
        BTreeSet::from(["A"]),
        BTreeSet::from(["A"]),
        BTreeSet::from(["A"]),
    ]
    .into_iter();
    let mut reads = [("A", ""), ("B", ""), ("A", "4242 ")].into_iter();
    let mut listed = 0;
    let answer = children_in_passes(
        || {
            listed += 1;
            listings
                .next()
                .ok_or_else(|| std::io::Error::other("no listing is scripted"))
        },
        |task| {
            let (expected, list) = reads.next().expect("a read the script holds");
            assert_eq!(*task, expected, "the reads follow the listed tasks");
            Ok(list.to_owned())
        },
        |_| Ok(true),
        |_| {},
    );
    assert_eq!(
        answer,
        Ok(vec![4242]),
        "the child B handed to A is found by a second pass, not lost to a first pass that read \
         B's list empty after B ended"
    );
    assert_eq!(
        listed, 4,
        "two passes, each listing the tasks before and after its reads"
    );
    assert_eq!(reads.next(), None, "every scripted read was made");
}

#[cfg(target_os = "linux")]
#[test]
fn an_engines_children_are_not_lost_when_a_task_id_is_reused_between_the_listings() {
    // B ends after its list was opened and hands its child to A, already read;
    // a new thread takes B's id, with a new start time, before the second
    // listing. Its state, read through the id, is running.
    let mut listings = [
        BTreeSet::from([("A", 1), ("B", 1)]),
        BTreeSet::from([("A", 1), ("B", 2)]),
        BTreeSet::from([("A", 1), ("B", 2)]),
        BTreeSet::from([("A", 1), ("B", 2)]),
    ]
    .into_iter();
    let mut reads = [
        (("A", 1), ""),
        (("B", 1), ""),
        (("A", 1), "4242 "),
        (("B", 2), ""),
    ]
    .into_iter();
    let mut listed = 0;
    let answer = children_in_passes(
        || {
            listed += 1;
            listings
                .next()
                .ok_or_else(|| std::io::Error::other("no listing is scripted"))
        },
        |task| {
            let (expected, list) = reads.next().expect("a read the script holds");
            assert_eq!(*task, expected, "the reads follow the listed tasks");
            Ok(list.to_owned())
        },
        |_| Ok(true),
        |_| {},
    );
    assert_eq!(
        answer,
        Ok(vec![4242]),
        "the child B handed to A is found by a second pass, not lost to a first pass whose two \
         listings held the same task ids, B's taken by a new thread"
    );
    assert_eq!(
        listed, 4,
        "two passes, each listing the tasks before and after its reads"
    );
    assert_eq!(reads.next(), None, "every scripted read was made");
}

#[cfg(target_os = "linux")]
#[test]
fn an_engines_children_are_not_lost_when_a_task_still_listed_is_ending() {
    // B is in both listings of the first pass, but begins to end after its
    // list was read empty and hands its child to A, already read.
    let events = std::cell::RefCell::new(Vec::new());
    let mut listings = [
        BTreeSet::from(["A", "B"]),
        BTreeSet::from(["A", "B"]),
        BTreeSet::from(["A"]),
        BTreeSet::from(["A"]),
    ]
    .into_iter();
    let mut reads = [("A", ""), ("B", ""), ("A", "4242 ")].into_iter();
    let mut states = [("A", true), ("B", false), ("A", true)].into_iter();
    let answer = children_in_passes(
        || {
            events.borrow_mut().push("list".to_owned());
            listings
                .next()
                .ok_or_else(|| std::io::Error::other("no listing is scripted"))
        },
        |task| {
            events.borrow_mut().push(format!("read {task}"));
            let (expected, list) = reads.next().expect("a read the script holds");
            assert_eq!(*task, expected, "the reads follow the listed tasks");
            Ok(list.to_owned())
        },
        |task| {
            events.borrow_mut().push(format!("state {task}"));
            let (expected, running) = states.next().expect("a state the script holds");
            assert_eq!(*task, expected, "the states asked follow the listed tasks");
            Ok(running)
        },
        |_| {},
    );
    assert_eq!(
        answer,
        Ok(vec![4242]),
        "the child B handed to A is found by a second pass, not lost to a first pass whose two \
         listings agreed while B was ending"
    );
    assert_eq!(
        events.into_inner(),
        [
            "list", "read A", "read B", "state A", "state B", "list", "list", "read A", "state A",
            "list",
        ],
        "two passes, each asking every listed task's state after its reads and before its second \
         listing"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn an_engines_children_are_found_after_pauses_when_a_task_stays_listed_while_ending() {
    // B has handed its child to A and stays listed, ending, for three passes;
    // on the fourth it is gone and the child is in A's list.
    let events = std::cell::RefCell::new(Vec::new());
    let mut pauses = Vec::new();
    let ending = || BTreeSet::from(["A", "B"]);
    let mut listings = [
        ending(),
        ending(),
        ending(),
        ending(),
        ending(),
        ending(),
        BTreeSet::from(["A"]),
        BTreeSet::from(["A"]),
    ]
    .into_iter();
    let mut reads = [
        ("A", ""),
        ("B", ""),
        ("A", ""),
        ("B", ""),
        ("A", ""),
        ("B", ""),
        ("A", "4242 "),
    ]
    .into_iter();
    let mut states = [
        ("A", true),
        ("B", false),
        ("A", true),
        ("B", false),
        ("A", true),
        ("B", false),
        ("A", true),
    ]
    .into_iter();
    let answer = children_in_passes(
        || {
            events.borrow_mut().push("list".to_owned());
            listings
                .next()
                .ok_or_else(|| std::io::Error::other("no listing is scripted"))
        },
        |task| {
            events.borrow_mut().push(format!("read {task}"));
            let (expected, list) = reads.next().expect("a read the script holds");
            assert_eq!(*task, expected, "the reads follow the listed tasks");
            Ok(list.to_owned())
        },
        |task| {
            events.borrow_mut().push(format!("state {task}"));
            let (expected, running) = states.next().expect("a state the script holds");
            assert_eq!(*task, expected, "the states asked follow the listed tasks");
            Ok(running)
        },
        |wait| {
            events.borrow_mut().push("pause".to_owned());
            pauses.push(wait);
        },
    );
    assert_eq!(
        pauses,
        [
            std::time::Duration::from_millis(1),
            std::time::Duration::from_millis(5),
            std::time::Duration::from_millis(20),
        ],
        "a pause after each of the three discarded passes, each longer than the one before"
    );
    assert_eq!(
        answer,
        Ok(vec![4242]),
        "the child B handed to A is found by the fourth pass, made after B, ending through three \
         passes, was released"
    );
    let pass_with_b_ending = [
        "list", "read A", "read B", "state A", "state B", "list", "pause",
    ];
    let mut expected = pass_with_b_ending.repeat(3);
    expected.extend(["list", "read A", "state A", "list"]);
    assert_eq!(
        events.into_inner(),
        expected,
        "three passes discarded for B ending, each followed by a pause, then a fourth that counts"
    );
    assert_eq!(listings.next(), None, "every scripted listing was made");
    assert_eq!(reads.next(), None, "every scripted read was made");
    assert_eq!(states.next(), None, "every scripted state was asked");
}

#[cfg(target_os = "linux")]
#[test]
fn an_engines_children_are_read_in_another_pass_when_a_list_is_not_found() {
    // The listings agree, so the not-found read alone discards the first pass.
    let mut listings = [
        BTreeSet::from(["A", "B"]),
        BTreeSet::from(["A", "B"]),
        BTreeSet::from(["A"]),
        BTreeSet::from(["A"]),
    ]
    .into_iter();
    let mut reads = [("A", Some("7 ")), ("B", None), ("A", Some("8 "))].into_iter();
    let mut listed = 0;
    let answer = children_in_passes(
        || {
            listed += 1;
            listings
                .next()
                .ok_or_else(|| std::io::Error::other("no listing is scripted"))
        },
        |task| {
            let (expected, list) = reads.next().expect("a read the script holds");
            assert_eq!(*task, expected, "the reads follow the listed tasks");
            list.map(str::to_owned)
                .ok_or_else(|| std::io::ErrorKind::NotFound.into())
        },
        |_| Ok(true),
        |_| {},
    );
    assert_eq!(
        answer,
        Ok(vec![8]),
        "a pass in which B's list was not found is discarded whole, its read of A included, and \
         the next pass's answer is returned"
    );
    assert_eq!(
        listed, 4,
        "two passes, each listing the tasks before and after its reads"
    );
    assert_eq!(reads.next(), None, "every scripted read was made");
}

#[cfg(target_os = "linux")]
#[test]
fn an_engines_tasks_that_change_under_every_pass_end_in_an_error_after_five_passes() {
    let mut listed = 0;
    let mut pauses = Vec::new();
    let answer = children_in_passes(
        || {
            listed += 1;
            if listed > 10 {
                return Err(std::io::Error::other("a listing after the fifth pass"));
            }
            Ok(BTreeSet::from([listed]))
        },
        |_| Ok("4242 ".to_owned()),
        |_| Ok(true),
        |wait| pauses.push(wait),
    );
    assert_eq!(
        answer,
        Err(
            "none of 5 passes over the tasks counted; in the last, the tasks listed after the \
             reads, {10}, were not those listed before them, {9}"
                .to_owned()
        ),
        "tasks that change under every pass end in an error that says what changed in the last \
         pass, never in the children its reads found"
    );
    assert_eq!(
        listed, 10,
        "five passes, each listing the tasks before and after its reads"
    );
    assert_eq!(
        pauses,
        [
            std::time::Duration::from_millis(1),
            std::time::Duration::from_millis(5),
            std::time::Duration::from_millis(20),
            std::time::Duration::from_millis(50),
        ],
        "a growing pause between each pass and the next, and none after the fifth"
    );
}

/// What is established about a process this harness identified. Only
/// [`Observed::Live`] is a positive premise of liveness, and only
/// [`Observed::Ended`] is evidence of an ending; [`Observed::Unknown`] is
/// neither, and as evidence that something ended it fails.
#[cfg(target_os = "linux")]
#[derive(Debug, Clone, PartialEq, Eq)]
enum Observed {
    /// Its pid answers the start time it was identified with, in a state that
    /// is neither a zombie nor dead.
    Live,
    /// Its ending is an owned fact: this harness collected it with that
    /// identity, or it has exited (a zombie answering that identity) and waits
    /// for this harness to collect it.
    Ended(String),
    /// Nothing establishes either: an observation that could not be read, or
    /// a pid now naming another process.
    Unknown(String),
}

#[cfg(target_os = "linux")]
fn observed(who: &Identified, owned: &OwnedDescendants) -> Observed {
    if let Some((_, status)) = owned
        .collected
        .iter()
        .find(|(collected, _)| collected.pid == who.pid && collected.start == who.start)
    {
        return Observed::Ended(format!("collected here, status {status:#x}"));
    }
    match stat_of(who.pid) {
        Ok((now, state)) if now.start == who.start && matches!(state, 'Z' | 'X') => {
            Observed::Ended(format!("exited (state {state}) and not yet collected"))
        }
        Ok((now, _)) if now.start == who.start => Observed::Live,
        Ok((now, state)) => Observed::Unknown(format!(
            "pid {} now names another process: {} started at {} (state {state})",
            who.pid, now.comm, now.start
        )),
        Err(error) => Observed::Unknown(error),
    }
}

/// What one `waitpid(pid, WNOHANG)` answered.
#[cfg(target_os = "linux")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Waited {
    Collected(libc::c_int),
    StillRunning,
    Interrupted,
    NotAChild,
    Failed(i32),
}

#[cfg(target_os = "linux")]
fn wait_now(pid: libc::pid_t) -> Waited {
    let mut status = 0;
    // SAFETY: `status` is writable and `WNOHANG` blocks on nothing; `pid` is a
    // child the kernel named ended, and this harness is the only waiter for its
    // descendants.
    let answered = unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) };
    if answered == pid {
        return Waited::Collected(status);
    }
    if answered == 0 {
        return Waited::StillRunning;
    }
    match std::io::Error::last_os_error().raw_os_error() {
        Some(libc::EINTR) => Waited::Interrupted,
        Some(libc::ECHILD) => Waited::NotAChild,
        Some(errno) => Waited::Failed(errno),
        None => Waited::Failed(0),
    }
}

/// What `waitid(P_ALL, WEXITED | WNOHANG | WNOWAIT)` answered about this
/// harness's children, collecting none of them: the kernel's own account.
#[cfg(target_os = "linux")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pending {
    /// This child has ended and waits to be collected.
    Ended(libc::pid_t),
    /// Children remain and none of them has ended.
    Running,
    /// No child remains (`ECHILD`).
    NoChildren,
    Interrupted,
    Failed(i32),
}

#[cfg(target_os = "linux")]
fn pending_now() -> Pending {
    // SAFETY: `siginfo_t` is plain data for which zeroes are a valid value, and
    // a zero `si_pid` is how `WNOHANG` reports that no child has ended.
    let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
    // SAFETY: `info` outlives the call; `WNOWAIT` leaves an ended child
    // waitable and `WNOHANG` blocks on nothing.
    let answered = unsafe {
        libc::waitid(
            libc::P_ALL,
            0,
            &raw mut info,
            libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
        )
    };
    if answered == 0 {
        // SAFETY: `waitid` returned zero, so `info` is the record it filled,
        // or the zeroes it left when no child has ended.
        let pid = unsafe { info.si_pid() };
        return if pid == 0 {
            Pending::Running
        } else {
            Pending::Ended(pid)
        };
    }
    match std::io::Error::last_os_error().raw_os_error() {
        Some(libc::EINTR) => Pending::Interrupted,
        Some(libc::ECHILD) => Pending::NoChildren,
        Some(errno) => Pending::Failed(errno),
        None => Pending::Failed(0),
    }
}

/// What the kernel's account established when a collection pass stopped.
#[cfg(target_os = "linux")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Accounted {
    /// `ECHILD`: this harness has no child left, ended or running.
    NoneRemain,
    /// Children remain and none of them has ended.
    SomeRemain,
    /// The account could not be completed; the reason is recorded.
    Unaccountable,
}

/// Every process the killed coordinator leaves, owned by the harness.
///
/// The harness makes itself a child subreaper (`PR_SET_CHILD_SUBREAPER`)
/// before it starts the coordinator, so each process the coordinator forked or
/// spawned -- the paused stub, every command reaper and its group anchor, the
/// container-label backstops and any `docker` child a reaper runs -- is
/// re-parented to the harness when its own parent dies. The harness forks
/// nothing of its own (its fixture is built by its parent; the coordinator is
/// started with `std::process::Command`, not the process funnel), and asserts
/// as much with the kernel before the coordinator starts, so the kernel's later
/// answer that no child remains is about the coordinator's processes alone.
/// It finds its children only through its own tasks' children lists -- for
/// identification and signalling, never as evidence that none remains --
/// records each one's identity when it first sees it, and collects only
/// children the kernel names ended (`waitid(P_ALL, ..., WNOWAIT)`), each with
/// its own `waitpid(pid, WNOHANG)`, reconciling the identity its zombie
/// answers with the one it was first listed with. It signals a process only
/// while it is listed as its own unreaped child with the start time it was
/// identified with, never a process group. It is the only waiter for its
/// descendants, and makes no wait before its coordinator has been collected
/// by the coordinator's own handle.
#[cfg(target_os = "linux")]
struct OwnedDescendants {
    /// Every child listed here and not yet collected, as first identified.
    known: BTreeMap<libc::pid_t, Identified>,
    /// Every child collected here: the identity its zombie answered, and its
    /// wait status.
    collected: Vec<(Identified, libc::c_int)>,
    errors: Vec<String>,
}

/// What [`OwnedDescendants::end_and_collect_all`] established.
#[cfg(target_os = "linux")]
struct Collection {
    complete: bool,
    collected_now: usize,
    left: Vec<String>,
}

#[cfg(target_os = "linux")]
impl OwnedDescendants {
    fn adopt() -> Self {
        // SAFETY: `prctl` takes its five arguments by value and reads through
        // no pointer for `PR_SET_CHILD_SUBREAPER`, which marks only this
        // isolated harness process.
        let set = unsafe {
            libc::syscall(
                libc::SYS_prctl,
                libc::c_long::from(libc::PR_SET_CHILD_SUBREAPER),
                1_i64,
                0_i64,
                0_i64,
                0_i64,
            )
        };
        assert_eq!(
            set,
            0,
            "PR_SET_CHILD_SUBREAPER: {}",
            std::io::Error::last_os_error()
        );
        Self {
            known: BTreeMap::new(),
            collected: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// Record `error` once.
    fn note(&mut self, error: String) {
        if !self.errors.contains(&error) {
            self.errors.push(error);
        }
    }

    /// The premise of every collection here, asked of the kernel before the
    /// coordinator starts: this harness has no child at all.
    fn has_no_child(&mut self) -> bool {
        loop {
            match pending_now() {
                Pending::NoChildren => return true,
                Pending::Interrupted => {}
                other => {
                    self.note(format!(
                        "before its coordinator started this harness already had a child of its \
                         own (waitid answered {other:?}), so the kernel's account could not tell \
                         the coordinator's processes from it"
                    ));
                    return false;
                }
            }
        }
    }

    /// This harness's own children, each identified on first sight -- for
    /// identification and signalling only; `None` (and an error recorded) when
    /// a list cannot be read.
    fn own_children(&mut self) -> Option<Vec<libc::pid_t>> {
        match children_of("self") {
            Ok(children) => {
                for pid in &children {
                    if !self.known.contains_key(pid) {
                        match stat_of(*pid) {
                            Ok((identified, _)) => {
                                self.known.insert(*pid, identified);
                            }
                            Err(error) => self.note(error),
                        }
                    }
                }
                Some(children)
            }
            Err(error) => {
                self.note(error);
                None
            }
        }
    }

    /// Collect every child of this harness that the kernel names ended, and
    /// stop at the kernel's next answer: that children remain and none has
    /// ended, that none remains, or that the account cannot be made.
    fn collect_ended(&mut self) -> Accounted {
        loop {
            match pending_now() {
                Pending::Ended(pid) => {
                    let identity = match stat_of(pid) {
                        Ok((identity, _)) => identity,
                        Err(error) => {
                            self.note(format!("the ended child {pid}: {error}"));
                            Identified {
                                pid,
                                start: 0,
                                comm: "?".to_owned(),
                                pgrp: 0,
                            }
                        }
                    };
                    if let Some(first) = self.known.remove(&pid) {
                        if first.start != identity.start {
                            self.note(format!(
                                "child {pid} was listed with start time {} and ended with {}",
                                first.start, identity.start
                            ));
                        }
                    }
                    loop {
                        match wait_now(pid) {
                            Waited::Collected(status) => {
                                self.collected.push((identity, status));
                                break;
                            }
                            Waited::Interrupted => {}
                            other => {
                                self.note(format!(
                                    "the kernel named {pid} ended and waitpid({pid}, WNOHANG) \
                                     answered {other:?}"
                                ));
                                return Accounted::Unaccountable;
                            }
                        }
                    }
                }
                Pending::Running => return Accounted::SomeRemain,
                Pending::NoChildren => return Accounted::NoneRemain,
                Pending::Interrupted => {}
                Pending::Failed(errno) => {
                    self.note(format!("waitid(P_ALL) failed with errno {errno}"));
                    return Accounted::Unaccountable;
                }
            }
        }
    }

    /// `true` when every child ever listed here was collected here.
    fn reconciled(&mut self) -> bool {
        if self.known.is_empty() {
            return true;
        }
        let never: Vec<String> = self
            .known
            .values()
            .map(|who| format!("{}:{}", who.pid, who.comm))
            .collect();
        self.note(format!(
            "listed here and never collected, though the kernel reports no child: {never:?}"
        ));
        false
    }

    /// Let every process the killed coordinator left end on its own within
    /// `bound`, sending none a signal, and collect each; `true` only when the
    /// kernel reports that no child remains, every listed child was collected
    /// with its identity, and nothing failed.
    fn settle_naturally_within(&mut self, bound: Duration) -> bool {
        let deadline = Instant::now() + bound;
        loop {
            let _ = self.own_children();
            match self.collect_ended() {
                Accounted::NoneRemain => return self.reconciled() && self.errors.is_empty(),
                Accounted::Unaccountable => return false,
                Accounted::SomeRemain => {}
            }
            if Instant::now() >= deadline {
                return false;
            }
            crate::workspace_manager::fixture::rest(Duration::from_millis(10));
        }
    }

    /// End and collect every own child within `bound`: each one still listed
    /// as this harness's unreaped child, alive and with the start time it was
    /// identified with, is sent `SIGKILL` by its pid, and every ended one is
    /// collected, again as grandchildren are re-parented here, until the
    /// kernel reports that no child remains.
    fn end_and_collect_all(&mut self, bound: Duration) -> Collection {
        let deadline = Instant::now() + bound;
        let before = self.collected.len();
        loop {
            let listed = self.own_children();
            if self.collect_ended() == Accounted::NoneRemain {
                let reconciled = self.reconciled();
                return Collection {
                    complete: reconciled && self.errors.is_empty(),
                    collected_now: self.collected.len().saturating_sub(before),
                    left: Vec::new(),
                };
            }
            if Instant::now() >= deadline {
                let left = self
                    .own_children()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|pid| {
                        stat_of(pid).map_or_else(
                            |error| format!("{pid}: {error}"),
                            |(who, state)| format!("{pid}:{}:{state}", who.comm),
                        )
                    })
                    .collect();
                return Collection {
                    complete: false,
                    collected_now: self.collected.len().saturating_sub(before),
                    left,
                };
            }
            for pid in listed.unwrap_or_default() {
                let Some(known) = self.known.get(&pid) else {
                    continue;
                };
                if stat_of(pid).is_ok_and(|(now, state)| {
                    now.start == known.start && !matches!(state, 'Z' | 'X')
                }) {
                    // SAFETY: `pid` is listed as this harness's own unreaped
                    // child and still has the start time it was identified
                    // with, so it names that process and no other: only this
                    // harness can collect it, so its pid cannot have been
                    // reused.
                    let _ = unsafe { libc::kill(pid, libc::SIGKILL) };
                }
            }
            crate::workspace_manager::fixture::rest(Duration::from_millis(10));
        }
    }
}

/// The collector, ended at its drop: on a pass, a failing assertion, a panic,
/// an internal deadline and the B1 reversal alike, it ends and collects every
/// descendant within its bound and reports what it did on the harness's
/// stderr. The fixture belongs to the parent test, which removes it only on a
/// report of a complete collection and otherwise keeps it: an incomplete
/// collection is never success.
#[cfg(target_os = "linux")]
struct OwnedCollection {
    owned: OwnedDescendants,
}

#[cfg(target_os = "linux")]
impl Drop for OwnedCollection {
    fn drop(&mut self) {
        let collection = self.owned.end_and_collect_all(Duration::from_secs(20));
        let statuses: Vec<String> = self
            .owned
            .collected
            .iter()
            .map(|(who, status)| format!("{}:{}:{status:#x}", who.pid, who.comm))
            .collect();
        crate::workspace_manager::fixture::say(&format!(
            "lease-kill collection: complete: {}; collected at the drop: {}; collected in all: {} \
             [{}]; errors: {:?}; left: {:?}",
            collection.complete,
            collection.collected_now,
            self.owned.collected.len(),
            statuses.join(" "),
            self.owned.errors,
            collection.left
        ));
    }
}

/// Where the coordinator writes the pids of the container-label backstops it
/// armed (`armed_container_reaper_pids`), once its helper has paused, for its
/// harness to identify them.
#[cfg(target_os = "linux")]
const LEASE_KILL_BACKSTOPS: &str = "UPSTROKE_TEST_LEASE_KILL_BACKSTOPS";

/// The coordinator a lease-kill harness kills. As a fresh run's or a resume's
/// admission does, it takes its physical worktree's lock; as
/// `TopologyRun::run_concurrently` does, it takes its run lock and enters its
/// cleanup scope, captures the
/// scope's paths and hands a worker thread -- inside no scope of its own -- the
/// `Carried` call a pipeline is given (`engine::topology::preflight::Carried`).
/// The worker invokes a real `ContainerRunner` over `DockerCli` and the process
/// funnel, whose stub `docker` pauses at the case's boundary, so this never
/// returns: its harness kills it there. A second thread reports, once the
/// helper has paused, the backstops the worker's invocation armed.
#[cfg(target_os = "linux")]
#[test]
#[ignore = "the coordinator a lease-kill harness kills at its acknowledged boundary"]
fn lease_kill_coordinator_child() {
    let case = std::env::var(LEASE_KILL_CASE).expect("the harness names the case");
    let root =
        PathBuf::from(std::env::var_os(LEASE_KILL_ROOT).expect("the harness names the fixture"));
    let ack = PathBuf::from(std::env::var_os(LEASE_KILL_ACK).expect("the harness names the pause"));
    let backstops = PathBuf::from(
        std::env::var_os(LEASE_KILL_BACKSTOPS).expect("the harness names the report"),
    );
    let layout = Layout::of(&root);
    let _worktree =
        crate::rundir::WorktreeLock::acquire_in(&layout.repo, &layout.repo.join(".git"))
            .expect("the coordinator's worktree lock");
    let lock =
        crate::rundir::RunLock::acquire(&layout.paths.public).expect("the coordinator's run lock");
    let _scope = lock.enter_cleanup_scope();
    let carried = crate::engine::topology::preflight::Carried::new(
        crate::runner::Cancellation::new(),
        crate::rundir::active_cleanup_lease_paths(),
    );
    let report = |path: &Path, pids: &[libc::pid_t]| {
        let text: Vec<String> = pids.iter().map(ToString::to_string).collect();
        let staged = path.with_extension("tmp");
        std::fs::write(&staged, format!("{}\n", text.join(" "))).expect("the backstop report");
        std::fs::rename(&staged, path).expect("the backstop report");
    };
    std::thread::scope(|scope| {
        scope.spawn(|| {
            let deadline = Instant::now() + Duration::from_secs(60);
            while Instant::now() < deadline {
                if ack.exists() {
                    report(
                        &backstops,
                        &crate::agent::proc::test_support::armed_container_reaper_pids(),
                    );
                    return;
                }
                crate::workspace_manager::fixture::rest(Duration::from_millis(5));
            }
        });
        scope.spawn(|| {
            let identity = RunIdentity {
                private_root: layout.private_root.clone(),
                run_id: RUN_ID.to_owned(),
                run_dir: layout.paths.public.clone(),
                incarnation: INCARNATION_1.to_owned(),
                repo_key: repo_key().to_owned(),
            };
            let runner = ContainerRunner::new(
                container_policy(),
                identity,
                &layout.repo,
                fixtures::plain_checkout(&layout.repo.join(".git")),
                image_environment(),
                Box::new(crate::runner::container::DockerCli::default()),
            )
            .expect("a container runner over DockerCli")
            .with_view(Box::new(crate::runner::container::DisposableDirView::new(
                ContainerTrace::off(),
            )))
            .with_poll(Duration::from_millis(25));
            let request = match case.as_str() {
                "worker-create" | "attached-start" => {
                    let mut request = gate_request(
                        ShellKind::Sh.spec("exit 0"),
                        layout.task_a.clone(),
                        Duration::from_secs(600),
                        gate_id(0),
                    );
                    if case == "attached-start" {
                        request.command.stdin = b"the attached start's stdin\n".to_vec();
                    }
                    request
                }
                // An implementer's invocation mounts its agent's credential
                // volume, which `create_container` asks the runtime about
                // before anything is created.
                "credential-volume" | "pre-create-cancel" => worker_request(
                    ShellKind::Sh.spec("exit 0"),
                    layout.task_a.clone(),
                    AgentId::new("claude-code"),
                    Duration::from_secs(600),
                    worker_id(0),
                ),
                other => panic!("unknown lease-kill case `{other}`"),
            };
            let ended = runner.run_blocking_with(&request, carried.call());
            panic!(
                "the paused helper was to hold this worker until its coordinator was killed: \
                 {ended:?}"
            );
        });
    });
}

/// The pids written at `path`, once it is there: possibly none.
#[cfg(target_os = "linux")]
fn pids_at(path: &Path) -> Option<Vec<libc::pid_t>> {
    let text = std::fs::read_to_string(path).ok()?;
    Some(
        text.split_whitespace()
            .filter_map(|pid| pid.parse().ok())
            .collect(),
    )
}

/// Every candidate for the command reaper that registered the paused
/// `helper`'s process group, each paired with the group anchor it forked into
/// that group: a child of the coordinator (`forks`) that is neither the helper
/// nor a member of the helper's group, and a child of that candidate's own in
/// the helper's group that is not the helper. The helper is never its own
/// candidate: until it execs its `sleep`, the stub's `mv` that publishes its
/// acknowledgement is a child of the helper in the helper's group. Each is
/// identified by pid and start time; the caller requires exactly one.
#[cfg(target_os = "linux")]
fn helper_reaper_candidates(
    helper: libc::pid_t,
    forks: &[libc::pid_t],
) -> Vec<(Identified, Identified)> {
    let mut candidates = Vec::new();
    for fork in forks.iter().copied().filter(|fork| *fork != helper) {
        let Ok((reaper, _)) = stat_of(fork) else {
            continue;
        };
        if reaper.pgrp == helper {
            continue;
        }
        candidates.extend(
            children_of(&fork.to_string())
                .unwrap_or_default()
                .into_iter()
                .filter(|anchor| *anchor != helper)
                .filter_map(|anchor| stat_of(anchor).ok())
                .filter(|(anchor, _)| anchor.pgrp == helper)
                .map(|(anchor, _)| (reaper.clone(), anchor)),
        );
    }
    candidates
}

/// The owned, bounded harness of the lease-kill guards.
///
/// It adopts every process the coordinator will leave ([`OwnedDescendants`],
/// ended at the drop of [`OwnedCollection`]) and has the kernel confirm it has
/// no child of its own; starts the coordinator over the fixture its parent
/// built, with the stub `docker` first on its `PATH`; and waits for the paused
/// helper's acknowledgement and the backstop reports. While the coordinator is
/// alive it identifies, through the coordinator's children lists, the covered
/// processes: the paused helper, the command reaper that registered the
/// helper's process group (the parent of the group's anchor), and the
/// container-label backstop, which lists its labels at its coordinator's death and,
/// finding nothing, ends having taken nothing. Then it kills the coordinator and
/// collects it, and asks for the run's admission -- the exclusive cleanup probe of
/// `RunLock::acquire` -- and for a different run's admission to the worktree --
/// `WorktreeLock`'s scan of every run's cleanup hold -- requiring both refused
/// while the helper's reaper is live, owing its settlement, both before and
/// after the ask; asks again until admitted, requiring every covered
/// process's ending established by then by an owned fact; and lets every
/// process the coordinator left end on its own and collects it, to the kernel's
/// own account that none remains. Every wait has its own deadline inside the
/// parent's isolation bound.
#[cfg(target_os = "linux")]
#[test]
#[ignore = "the isolated harness of the cleanup-lease kill guards"]
fn lease_kill_harness_child() {
    use crate::agent::proc::test_support::RunningTestChild;
    use std::ffi::OsStr;

    let case = std::env::var(LEASE_KILL_CASE).expect("the parent names the case");
    let root =
        PathBuf::from(std::env::var_os(LEASE_KILL_ROOT).expect("the parent names the fixture"));
    let Layout {
        repo, paths: run, ..
    } = Layout::of(&root);
    let public = run.public;
    let git_dir = repo.join(".git");
    let bin = root.join("bin");
    let ack = root.join("paused-helper.pid");
    let backstops = root.join("backstops.pid");
    let inherited = std::env::var_os("PATH").unwrap_or_default();
    let path =
        std::env::join_paths(std::iter::once(bin.clone()).chain(std::env::split_paths(&inherited)))
            .expect("a PATH with the stub first");
    let (out, err) = (
        root.join("coordinator.stdout"),
        root.join("coordinator.stderr"),
    );
    let mut owned = OwnedCollection {
        owned: OwnedDescendants::adopt(),
    };
    assert!(
        owned.owned.has_no_child(),
        "{case}: before its coordinator starts this harness has no child of its own: {:?}",
        owned.owned.errors
    );
    let mut coordinator = RunningTestChild::start(
        "runner::container::exec::tests::lease_kill_coordinator_child",
        &[
            ("PATH", path.as_os_str()),
            (LEASE_KILL_CASE, OsStr::new(&case)),
            (LEASE_KILL_ROOT, root.as_os_str()),
            (LEASE_KILL_ACK, ack.as_os_str()),
            (LEASE_KILL_BACKSTOPS, backstops.as_os_str()),
            (
                "UPSTROKE_TEST_CLEANUP_DELAY_MS",
                OsStr::new(LEASE_KILL_SETTLEMENT_DELAY_MS),
            ),
        ],
        &out,
        &err,
    );
    let told = || {
        format!(
            "stdout:\n{}\nstderr:\n{}",
            std::fs::read_to_string(&out).unwrap_or_default(),
            std::fs::read_to_string(&err).unwrap_or_default()
        )
    };
    let paused_by = Instant::now() + Duration::from_secs(60);
    let (helper, backstop_pids) = loop {
        if let (Some(helper), Some(armed)) = (pids_at(&ack), pids_at(&backstops)) {
            if let Some(helper) = helper.first() {
                break (*helper, armed);
            }
        }
        if let Some(status) = coordinator.exited() {
            panic!(
                "{case}: the coordinator ended ({status:?}) before its helper paused: {}",
                told()
            );
        }
        assert!(
            Instant::now() < paused_by,
            "{case}: the helper never acknowledged its pause, or the backstops were not reported: \
             {}",
            told()
        );
        crate::workspace_manager::fixture::rest(Duration::from_millis(10));
    };

    // The covered processes, identified while the coordinator still lives.
    let coordinator_pid = coordinator
        .pid()
        .expect("the coordinator's pid")
        .to_string();
    let forks = children_of(&coordinator_pid).expect("the coordinator's children lists");
    let (helper_id, _) = stat_of(helper).expect("the paused helper's identity");
    assert!(
        forks.contains(&helper),
        "{case}: the paused helper is the coordinator's child"
    );
    // The helper's reaper: the one child of the coordinator, neither the helper
    // nor in its group, whose own child anchors the helper's group, found alike
    // by a second scan. None or several is refused, never resolved by the order
    // of a scan.
    let candidates = helper_reaper_candidates(helper, &forks);
    let [(helper_reaper, anchor)] = candidates.as_slice() else {
        panic!(
            "{case}: not exactly one child of the coordinator other than the paused helper \
             {helper}, and outside its group, anchors the helper's group: {candidates:?} among \
             {forks:?}"
        );
    };
    let rescanned = helper_reaper_candidates(
        helper,
        &children_of(&coordinator_pid).expect("the coordinator's children lists"),
    );
    assert_eq!(
        rescanned, candidates,
        "{case}: a second scan did not find the same reaper and anchor of the paused helper"
    );
    crate::workspace_manager::fixture::say(&format!(
        "{case}: the paused helper {} ({}) is anchored by {} ({}), a child of its reaper {} ({})",
        helper_id.pid,
        helper_id.comm,
        anchor.pid,
        anchor.comm,
        helper_reaper.pid,
        helper_reaper.comm
    ));
    let helper_reaper = helper_reaper.clone();
    for pid in &backstop_pids {
        assert!(
            forks.contains(pid),
            "{case}: the backstop {pid} is the coordinator's child"
        );
    }
    let backstop_ids: Vec<Identified> = backstop_pids
        .iter()
        .map(|pid| stat_of(*pid).expect("the backstop's identity").0)
        .collect();
    let covered: Vec<Identified> = std::iter::once(helper_id.clone())
        .chain(std::iter::once(helper_reaper.clone()))
        .chain(backstop_ids.iter().cloned())
        .collect();

    assert!(
        coordinator.kill_and_collect().is_some(),
        "{case}: the coordinator was killed and collected"
    );
    let before = observed(&helper_reaper, &owned.owned);
    let refused = crate::rundir::RunLock::acquire(&public).err();
    let refused_worktree = crate::rundir::WorktreeLock::acquire_in(&repo, &git_dir).err();
    let after = observed(&helper_reaper, &owned.owned);
    let owing = before == Observed::Live && after == Observed::Live;
    crate::workspace_manager::fixture::say(&format!(
        "{case}: admission refused while the paused helper's reaper still owed its settlement: \
         {} (reaper {} before the ask {before:?}, after it {after:?}; the run refused {}; a \
         different run's worktree admission refused {})",
        owing && refused.is_some() && refused_worktree.is_some(),
        helper_reaper.pid,
        refused.is_some(),
        refused_worktree.is_some()
    ));
    assert!(
        owing && refused.is_some() && refused_worktree.is_some(),
        "{case}: with the killed coordinator's helper reaper still settling, the run's admission \
         and a different run's worktree admission refused (before {before:?}, after {after:?}, \
         refusals {refused:?} and {refused_worktree:?})"
    );

    let admitted_by = Instant::now() + Duration::from_secs(60);
    let admitted = loop {
        let _ = owned.owned.collect_ended();
        let admitted =
            crate::rundir::WorktreeLock::acquire_in(&repo, &git_dir).and_then(|worktree| {
                crate::rundir::RunLock::acquire(&public).map(|run| (worktree, run))
            });
        match admitted {
            Ok(locks) => break locks,
            Err(error) => {
                assert!(
                    Instant::now() < admitted_by,
                    "{case}: admission never came once the settlement ended: {error}"
                );
                crate::workspace_manager::fixture::rest(Duration::from_millis(20));
            }
        }
    };
    let at_admission: Vec<(libc::pid_t, String, Observed)> = covered
        .iter()
        .map(|who| (who.pid, who.comm.clone(), observed(who, &owned.owned)))
        .collect();
    drop(admitted);
    let not_ended: Vec<&(libc::pid_t, String, Observed)> = at_admission
        .iter()
        .filter(|(_, _, observed)| !matches!(observed, Observed::Ended(_)))
        .collect();
    crate::workspace_manager::fixture::say(&format!(
        "{case}: admitted only after every covered helper, reaper and backstop had ended: {} \
         (at admission {at_admission:?})",
        not_ended.is_empty()
    ));
    assert!(
        not_ended.is_empty(),
        "{case}: at admission a covered process was live, or its ending was not established: \
         {not_ended:?}"
    );

    let settled = owned.owned.settle_naturally_within(Duration::from_secs(30));
    let remaining: Vec<String> = owned
        .owned
        .own_children()
        .unwrap_or_default()
        .into_iter()
        .map(|pid| {
            stat_of(pid).map_or_else(
                |error| format!("{pid}: {error}"),
                |(who, state)| format!("{pid} {} {state} pgrp {}", who.comm, who.pgrp),
            )
        })
        .collect();
    crate::workspace_manager::fixture::say(&format!(
        "{case}: every process the killed coordinator left ended on its own and was collected \
         here: {settled} ({} collected; remaining {remaining:?}; errors {:?})",
        owned.owned.collected.len(),
        owned.owned.errors
    ));
    assert!(
        settled,
        "{case}: after admission every process the killed coordinator left ended on its own and \
         this harness collected it, to the kernel's own account (remaining {remaining:?})"
    );
}

#[cfg(target_os = "linux")]
fn the_lease_kill_guard(case: &str) {
    let fixture = Fixture::built_in_a_bounded_child(&format!("lease-kill-{case}"), false);
    let bin = fixture.root.join("bin");
    std::fs::create_dir(&bin).expect("the stub's directory");
    let stub = bin.join("docker");
    std::fs::write(&stub, lease_kill_stub(case)).expect("the stub");
    crate::workspace_manager::fixture::set_mode(&stub, 0o755);
    let ended = crate::agent::proc::test_support::run_test_isolated(
        "runner::container::exec::tests::lease_kill_harness_child",
        &[
            (LEASE_KILL_CASE, std::ffi::OsStr::new(case)),
            (LEASE_KILL_ROOT, fixture.root.as_os_str()),
        ],
        Duration::from_secs(300),
    );
    let said = format!("{}{}", ended.stdout, ended.stderr);
    let complete = said.contains("lease-kill collection: complete: true;")
        && !said.contains("running test child collection: not established");
    if !complete {
        crate::workspace_manager::fixture::say(&format!(
            "lease-kill collection FAILED: the fixture at {} is kept for what may still need it",
            fixture.root.display()
        ));
        std::mem::forget(fixture);
    }
    assert!(
        complete,
        "{case}: the harness ended and collected every process the killed coordinator left, \
         whatever else it found: {ended}"
    );
    assert!(
        ended.status.is_some_and(|status| status.success()) && ended.stdout.contains("1 passed"),
        "{case}: the bounded harness ended: {ended}"
    );
    for line in [
        format!(
            "{case}: admission refused while the paused helper's reaper still owed its \
             settlement: true"
        ),
        format!(
            "{case}: admitted only after every covered helper, reaper and backstop had ended: \
             true"
        ),
        format!(
            "{case}: every process the killed coordinator left ended on its own and was \
             collected here: true"
        ),
        "lease-kill collection: complete: true;".to_owned(),
    ] {
        assert!(said.contains(&line), "{case}: missing `{line}`: {ended}");
    }
}

/// A worker's invocation carries its run's cleanup lease (R28) through the
/// `ContainerRunner` and `DockerCli`'s bounded question to the process funnel:
/// with the coordinator killed while the worker's synchronous `docker create`
/// is paused, run admission refuses while that helper's reaper still owes its
/// settlement, and is admitted only once it, the helper and the
/// container-label backstop have ended; every process the coordinator left is
/// then collected by the harness that owns it.
#[cfg(target_os = "linux")]
#[test]
fn a_killed_coordinators_paused_docker_create_keeps_run_admission_refused_until_it_is_settled() {
    the_lease_kill_guard("worker-create");
}

/// The attached start's supervisor thread is inside no cleanup scope, so the
/// invocation's leases reach its `docker start --attach` by value: with the
/// coordinator killed while that client is paused, run admission refuses while
/// its reaper still owes its settlement, and is admitted only once it has
/// ended.
#[cfg(target_os = "linux")]
#[test]
fn a_killed_coordinators_paused_attached_start_keeps_run_admission_refused_until_it_is_settled() {
    the_lease_kill_guard("attached-start");
}

/// The credential volume's inspection is asked before any container is
/// created, through a runtime method that takes no lease, on the worker's own
/// thread: the invocation's cleanup scope there is what its helper's reaper
/// holds. With the coordinator killed while that `docker volume inspect` is
/// paused, run admission refuses while its reaper still owes its settlement,
/// and is admitted only once it has ended.
#[cfg(target_os = "linux")]
#[test]
fn a_killed_coordinators_paused_credential_volume_inspection_keeps_run_admission_refused_until_it_is_settled()
 {
    the_lease_kill_guard("credential-volume");
}

/// A create refused before it was asked -- here, because the runtime would not
/// answer whether the credential volume exists -- is followed by the
/// invocation's cancellation, whose `docker stop` asks about a container no
/// create ever named. With the coordinator killed while that stop is paused,
/// run admission refuses while its reaper still owes its settlement, and is
/// admitted only once it has ended.
#[cfg(target_os = "linux")]
#[test]
fn a_killed_coordinators_paused_pre_create_cancellation_keeps_run_admission_refused_until_it_is_settled()
 {
    the_lease_kill_guard("pre-create-cancel");
}

// ---------------------------------------------------------------------------
// `PR12-INV9-CONTAINER-HELPER-CLEANUP-LEASE`: the dormant label backstop's
// handoff to an engine admitted to the same physical worktree
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
const HANDOFF_CASE: &str = "UPSTROKE_TEST_HANDOFF_CASE";

#[cfg(target_os = "linux")]
const HANDOFF_STATE: &str = "UPSTROKE_TEST_HANDOFF_STATE";

/// A stateful `docker` the killed coordinator's runner, its container-label
/// backstop and the contender's real startup census all share, so that every
/// one of them sees the same containers. Each call is appended to
/// `$UPSTROKE_TEST_HANDOFF_STATE/calls` as its caller's pid (`$PPID`) and its
/// arguments, so the harness can attribute each removal to its owner. `create`
/// records the container's labels under `containers/<name>`; `ps` answers the
/// backstop's `--quiet` listing with names, an observation by `name=` with its
/// state, and the census's labelled listing with the six fields its format
/// asks for, each filtered on every `label=` it is given; `rm` removes the
/// record; every other verb succeeds with nothing to say.
#[cfg(target_os = "linux")]
fn handoff_stub() -> String {
    format!(
        "#!/bin/sh\n\
         state=\"$UPSTROKE_TEST_HANDOFF_STATE\"\n\
         printf '%s %s\\n' \"$PPID\" \"$*\" >> \"$state/calls\"\n\
         case \"$1\" in\n\
         create)\n\
         \x20 name=''; labels=''; previous=''\n\
         \x20 for argument in \"$@\"; do\n\
         \x20   case \"$previous\" in\n\
         \x20     --name) name=\"$argument\" ;;\n\
         \x20     --label) labels=\"$labels$argument\n\"\n\
         \x20   esac\n\
         \x20   previous=\"$argument\"\n\
         \x20 done\n\
         \x20 printf '%s' \"$labels\" > \"$state/containers/$name\"\n\
         \x20 printf '%s\\n' \"$name\" ;;\n\
         container)\n\
         \x20 case \"$*\" in\n\
         \x20   *'{{{{.Image}}}}'*) printf '%s\\n' '{IMAGE_ID}' ;;\n\
         \x20 esac ;;\n\
         ps)\n\
         \x20 quiet=''; named=''; labelled=''; previous=''\n\
         \x20 for argument in \"$@\"; do\n\
         \x20   [ \"$argument\" = --quiet ] && quiet=yes\n\
         \x20   if [ \"$previous\" = --filter ]; then\n\
         \x20     case \"$argument\" in\n\
         \x20       name=*) named=\"${{argument#name=}}\" ;;\n\
         \x20       label=*) labelled=\"$labelled ${{argument#label=}}\" ;;\n\
         \x20     esac\n\
         \x20   fi\n\
         \x20   previous=\"$argument\"\n\
         \x20 done\n\
         \x20 for record in \"$state\"/containers/*; do\n\
         \x20   [ -f \"$record\" ] || continue\n\
         \x20   name=$(basename \"$record\")\n\
         \x20   [ -n \"$named\" ] && [ \"$name\" != \"$named\" ] && continue\n\
         \x20   selected=yes\n\
         \x20   for label in $labelled; do grep -qxF \"$label\" \"$record\" || selected=''; done\n\
         \x20   [ -n \"$selected\" ] || continue\n\
         \x20   if [ -n \"$quiet\" ]; then printf '%s\\n' \"$name\"\n\
         \x20   elif [ -n \"$named\" ]; then printf '%s\\037created\\n' \"$name\"\n\
         \x20   else\n\
         \x20     value() {{ sed -n \"s/^$1=//p\" \"$record\" | head -n 1; }}\n\
         \x20     printf '%s\\037%s\\037%s\\037%s\\037%s\\037%s\\n' \"$name\" \
         \"$(value upstroke.private_root)\" \"$(value upstroke.run)\" \
         \"$(value upstroke.run_dir)\" \"$(value upstroke.incarnation)\" \
         \"$(value upstroke.invocation)\"\n\
         \x20   fi\n\
         \x20 done ;;\n\
         rm)\n\
         \x20 for argument in \"$@\"; do\n\
         \x20   case \"$argument\" in rm|-*) ;; *) rm -f \"$state/containers/$argument\" ;; esac\n\
         \x20 done ;;\n\
         *) ;;\n\
         esac\n\
         exit 0\n"
    )
}

/// The killed coordinator's runner pauses here: at `Container.Start`'s
/// `Before`, after the intent, the view and the container exist and every
/// helper question before the start (the create, its image inspection) has
/// returned -- so no helper question is in flight -- and before the start is
/// asked. It acknowledges with its pid and waits to be killed.
#[cfg(target_os = "linux")]
struct PausedBeforeTheStart {
    ack: PathBuf,
}

#[cfg(target_os = "linux")]
impl crate::runner::container::ContainerHooks for PausedBeforeTheStart {
    fn phase(
        &mut self,
        site: crate::topology::effects::EffectSiteId,
        phase: crate::topology::effects::HookPhase,
    ) -> crate::topology::effects::Injection {
        use crate::topology::effects::{ContainerSite, EffectSiteId, HookPhase};
        if site == EffectSiteId::Container(ContainerSite::Start) && phase == HookPhase::Before {
            let staged = self.ack.with_extension("tmp");
            std::fs::write(&staged, std::process::id().to_string()).expect("the pause's ack");
            std::fs::rename(&staged, &self.ack).expect("the pause's ack");
            let until = Instant::now() + Duration::from_secs(300);
            while Instant::now() < until {
                crate::workspace_manager::fixture::rest(Duration::from_millis(100));
            }
            panic!("the coordinator paused before its start was never killed");
        }
        crate::topology::effects::Injection::Proceed
    }
}

/// The coordinator a handoff harness kills between two helper questions. As a
/// fresh run's admission and its run do, it holds its physical worktree's lock
/// and its run lock and enters its cleanup scope; a worker thread inside no
/// scope of its own invokes a real `ContainerRunner` over `DockerCli` and the
/// process funnel with the `Carried` call a pipeline is given, so the runner
/// arms its dormant label backstop over the run's lease paths; the runner
/// pauses before its start ([`PausedBeforeTheStart`]) and a second thread
/// reports the backstop's pid. The backstop renders the test gate the harness
/// names in this process's environment.
#[cfg(target_os = "linux")]
#[test]
#[ignore = "the coordinator a handoff harness kills between two helper questions"]
fn handoff_coordinator_child() {
    let root =
        PathBuf::from(std::env::var_os(LEASE_KILL_ROOT).expect("the harness names the fixture"));
    let ack = PathBuf::from(std::env::var_os(LEASE_KILL_ACK).expect("the harness names the pause"));
    let backstops = PathBuf::from(
        std::env::var_os(LEASE_KILL_BACKSTOPS).expect("the harness names the report"),
    );
    let layout = Layout::of(&root);
    let _worktree =
        crate::rundir::WorktreeLock::acquire_in(&layout.repo, &layout.repo.join(".git"))
            .expect("the coordinator's worktree lock");
    let lock =
        crate::rundir::RunLock::acquire(&layout.paths.public).expect("the coordinator's run lock");
    let _scope = lock.enter_cleanup_scope();
    let carried = crate::engine::topology::preflight::Carried::new(
        crate::runner::Cancellation::new(),
        crate::rundir::active_cleanup_lease_paths(),
    );
    std::thread::scope(|scope| {
        scope.spawn(|| {
            let deadline = Instant::now() + Duration::from_secs(60);
            while Instant::now() < deadline {
                if ack.exists() {
                    let armed: Vec<String> =
                        crate::agent::proc::test_support::armed_container_reaper_pids()
                            .iter()
                            .map(ToString::to_string)
                            .collect();
                    let staged = backstops.with_extension("tmp");
                    std::fs::write(&staged, format!("{}\n", armed.join(" ")))
                        .expect("the backstop report");
                    std::fs::rename(&staged, &backstops).expect("the backstop report");
                    return;
                }
                crate::workspace_manager::fixture::rest(Duration::from_millis(5));
            }
        });
        scope.spawn(|| {
            let identity = RunIdentity {
                private_root: layout.private_root.clone(),
                run_id: RUN_ID.to_owned(),
                run_dir: layout.paths.public.clone(),
                incarnation: INCARNATION_1.to_owned(),
                repo_key: repo_key().to_owned(),
            };
            let runner = ContainerRunner::new(
                container_policy(),
                identity,
                &layout.repo,
                fixtures::plain_checkout(&layout.repo.join(".git")),
                image_environment(),
                Box::new(crate::runner::container::DockerCli::default()),
            )
            .expect("a container runner over DockerCli")
            .with_view(Box::new(crate::runner::container::DisposableDirView::new(
                ContainerTrace::off(),
            )))
            .with_hooks(Box::new(PausedBeforeTheStart { ack: ack.clone() }))
            .with_poll(Duration::from_millis(25));
            let request = gate_request(
                ShellKind::Sh.spec("exit 0"),
                layout.task_a.clone(),
                Duration::from_secs(600),
                gate_id(0),
            );
            let ended = runner.run_blocking_with(&request, carried.call());
            panic!(
                "the pause before the start was to hold this worker until its coordinator was \
                 killed: {ended:?}"
            );
        });
    });
}

/// The distinct fresh run a handoff harness admits to the coordinator's
/// physical worktree, in a process of its own so that the harness forks
/// nothing itself and every process it collects is the coordinator's or this
/// contender's. It takes the worktree lock -- `WorktreeLock`'s admission and
/// its scan of every run's cleanup hold -- and says so; on the harness's word
/// runs its real startup container census as a fresh run of another
/// incarnation over the stub and reports what it reclaimed; on the harness's
/// word lets the lock go and ends.
#[cfg(target_os = "linux")]
#[test]
#[ignore = "the distinct fresh run a handoff harness admits to the worktree"]
fn handoff_contender_child() {
    use crate::runner::container::census::{Census, CensusStart, run_startup_census};

    let root =
        PathBuf::from(std::env::var_os(LEASE_KILL_ROOT).expect("the harness names the fixture"));
    let words = root.join("contender");
    let layout = Layout::of(&root);
    let tell = |name: &str, text: &str| {
        let staged = words.join(format!("{name}.tmp"));
        std::fs::write(&staged, text).expect("a word to the harness");
        std::fs::rename(&staged, words.join(name)).expect("a word to the harness");
    };
    let hear = |name: &str| {
        let by = Instant::now() + Duration::from_secs(120);
        while !words.join(name).exists() {
            assert!(Instant::now() < by, "the harness never said `{name}`");
            crate::workspace_manager::fixture::rest(Duration::from_millis(5));
        }
    };
    let worktree = crate::rundir::WorktreeLock::acquire_in(&layout.repo, &layout.repo.join(".git"))
        .expect("a distinct fresh run's physical worktree admission");
    tell("admitted", &std::process::id().to_string());
    hear("census");
    let complete = run_startup_census(
        &mut crate::runner::container::NoHooks,
        &Census {
            private_root: &layout.private_root,
            start: &CensusStart::FreshRun {
                incarnation: INCARNATION_2.to_owned(),
            },
            runtime: &crate::runner::container::DockerCli::default(),
            liveness: &crate::runner::container::runtime::LockProbe,
            view: &crate::runner::container::DisposableDirView::new(ContainerTrace::off()),
        },
    )
    .expect("the contender's startup census");
    let reclaimed: Vec<String> = complete
        .report()
        .reclaimed
        .iter()
        .map(|reclaimed| format!("{} {}", reclaimed.name.as_str(), reclaimed.ownership.name()))
        .collect();
    tell("reclaimed", &reclaimed.join("\n"));
    hear("release");
    drop(worktree);
}

/// Whether the backstop has reached `point` of its ending: it writes one byte to
/// the point's arrival file, which the harness created empty to arm the point.
#[cfg(target_os = "linux")]
fn arrived_at(gate: &Path, point: &str) -> bool {
    std::fs::metadata(format!("{}.{point}.arrived", gate.display()))
        .is_ok_and(|metadata| metadata.len() > 0)
}

/// The calls the stub recorded from `pid`, each as its arguments.
#[cfg(target_os = "linux")]
fn calls_of(state: &Path, pid: libc::pid_t) -> Vec<String> {
    let prefix = format!("{pid} ");
    std::fs::read_to_string(state.join("calls"))
        .unwrap_or_default()
        .lines()
        .filter_map(|line| line.strip_prefix(&prefix).map(str::to_owned))
        .collect()
}

/// The containers the stub's state holds, by name.
#[cfg(target_os = "linux")]
fn stub_containers(state: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(state.join("containers"))
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

/// The owned, bounded harness of the handoff guards, under the same collection
/// as the lease-kill harness ([`OwnedDescendants`]).
///
/// It starts the coordinator over the fixture its parent built, with the
/// stateful stub first on its `PATH` and the backstop gate named, and waits for
/// the pause before the start: the intent, the view and the container exist,
/// and -- because every helper question's reaper holds the run's cleanup lease
/// while it lives -- no cleanup hold means no question in flight, and that the
/// dormant backstop holds nothing. It kills the coordinator and collects it.
/// Then, by case, it schedules a distinct fresh run's admission to the same
/// physical worktree around the backstop's ending -- the contender is a process
/// of its own ([`handoff_contender_child`]), so the harness forks nothing
/// itself and asks only lock questions, which start no process:
///
/// * `early` -- the backstop held before its listing; the contender takes the
///   worktree lock (admission and its scan); the backstop, released, lists,
///   cannot take the worktree, and ends having removed nothing; the contender's
///   real startup census then reclaims the container, its intent and its view.
/// * `middle` -- the backstop held after its listing, exactly where a backstop
///   that took its cleanup hold before the worktree's lock would take it; the
///   contender takes the worktree lock; the backstop, released, cannot take the
///   worktree and is held again before it ends; inside that interval, with the
///   backstop alive, the run shows no cleanup hold and the contender's census
///   classifies the dead run's container as a dead owner's and reclaims it,
///   its intent and its view; only then does the backstop end, having removed
///   nothing.
/// * `late` -- the backstop held after it has taken the worktree's lock and its
///   cleanup hold; a distinct run's worktree admission and the run's own
///   admission are both refused; the backstop, released, kills and removes the
///   container and ends; the contender is then admitted and its census reclaims
///   the intent and view the backstop never touches.
///
/// Every removal is attributed by the caller's pid in the stub's call log, so
/// exactly one owner removes the container. Every direct child has exactly one
/// waiter: the coordinator and the contender are collected by their own
/// [`RunningTestChild`] handles, which fail closed; the collector here reaps
/// nothing while a handle owns a child -- the backstop's ending is read from
/// its zombie, which [`observed`] counts as an ending -- so it collects only
/// the adopted descendants, once every handle has collected its own. Every
/// process the coordinator and the contender left ends and is collected here,
/// to the kernel's own account.
#[cfg(target_os = "linux")]
#[test]
#[ignore = "the isolated harness of the handoff guards"]
fn handoff_harness_child() {
    use crate::agent::proc::test_support::RunningTestChild;

    let case = std::env::var(HANDOFF_CASE).expect("the parent names the case");
    let root =
        PathBuf::from(std::env::var_os(LEASE_KILL_ROOT).expect("the parent names the fixture"));
    let state = PathBuf::from(std::env::var_os(HANDOFF_STATE).expect("the parent names the state"));
    let layout = Layout::of(&root);
    let public = layout.paths.public.clone();
    let git_dir = layout.repo.join(".git");
    let bin = root.join("bin");
    let ack = root.join("paused-before-the-start.pid");
    let backstops = root.join("backstops.pid");
    let gate = root.join("gate");
    let armed: &[&str] = match case.as_str() {
        "early" => &["death"],
        "middle" => &["listed", "decided"],
        "late" => &["decided"],
        other => panic!("unknown handoff case `{other}`"),
    };
    for point in armed {
        std::fs::write(format!("{}.{point}.arrived", gate.display()), b"")
            .expect("arm a gate point");
    }
    let release = |point: &str| {
        std::fs::write(format!("{}.{point}", gate.display()), b"").expect("release a gate point");
    };
    let inherited = std::env::var_os("PATH").unwrap_or_default();
    let path = std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(&inherited)))
        .expect("a PATH with the stub first");
    let (out, err) = (
        root.join("coordinator.stdout"),
        root.join("coordinator.stderr"),
    );
    let mut owned = OwnedCollection {
        owned: OwnedDescendants::adopt(),
    };
    assert!(
        owned.owned.has_no_child(),
        "{case}: before its coordinator starts this harness has no child of its own: {:?}",
        owned.owned.errors
    );
    let mut coordinator = RunningTestChild::start(
        "runner::container::exec::tests::handoff_coordinator_child",
        &[
            ("PATH", path.as_os_str()),
            (LEASE_KILL_ROOT, root.as_os_str()),
            (LEASE_KILL_ACK, ack.as_os_str()),
            (LEASE_KILL_BACKSTOPS, backstops.as_os_str()),
            (HANDOFF_STATE, state.as_os_str()),
            (crate::agent::proc::BACKSTOP_GATE, gate.as_os_str()),
        ],
        &out,
        &err,
    );
    let told = || {
        format!(
            "stdout:\n{}\nstderr:\n{}",
            std::fs::read_to_string(&out).unwrap_or_default(),
            std::fs::read_to_string(&err).unwrap_or_default()
        )
    };
    let paused_by = Instant::now() + Duration::from_secs(60);
    let reported = loop {
        if pids_at(&ack).is_some_and(|pids| !pids.is_empty()) {
            if let Some(pids) = pids_at(&backstops) {
                break pids;
            }
        }
        if let Some(status) = coordinator.exited() {
            panic!(
                "{case}: the coordinator ended ({status:?}) before its pause: {}",
                told()
            );
        }
        assert!(
            Instant::now() < paused_by,
            "{case}: the coordinator never paused before its start: {}",
            told()
        );
        crate::workspace_manager::fixture::rest(Duration::from_millis(10));
    };
    let coordinator_pid = coordinator
        .pid()
        .expect("the coordinator's pid")
        .to_string();
    let forks = children_of(&coordinator_pid).expect("the coordinator's children lists");
    let [backstop_pid] = reported.as_slice() else {
        panic!("{case}: exactly one backstop was armed: {reported:?}");
    };
    assert!(
        forks.contains(backstop_pid),
        "{case}: the backstop {backstop_pid} is the coordinator's child"
    );
    let (backstop, _) = stat_of(*backstop_pid).expect("the backstop's identity");
    let [container] = stub_containers(&state)
        .try_into()
        .unwrap_or_else(|left: Vec<String>| {
            panic!("{case}: the paused invocation created exactly one container: {left:?}")
        });
    let intents = || {
        crate::runner::container::list_intents(&layout.private_root)
            .expect("the container namespace lists")
    };
    let [intent] = intents().try_into().unwrap_or_else(|left: Vec<_>| {
        panic!("{case}: the paused invocation wrote exactly one intent: {left:?}")
    });
    assert_eq!(
        intent.name.as_str(),
        container,
        "{case}: the intent names the container"
    );
    let view = crate::runner::container::census::view_path(&layout.private_root, &intent.name);
    let quiet_before_the_kill =
        !crate::rundir::observe_cleanup_hold(&public, &mut crate::rundir::NoHooks);
    crate::workspace_manager::fixture::say(&format!(
        "{case}: between two helper questions nothing held the run's cleanup lease, the dormant \
         backstop included: {quiet_before_the_kill} (view present {})",
        view.exists()
    ));
    assert!(
        quiet_before_the_kill && view.exists(),
        "{case}: the coordinator paused between helper questions with its backstop dormant and \
         its view mounted: {}",
        told()
    );

    assert!(
        coordinator.kill_and_collect().is_some(),
        "{case}: the coordinator was killed and collected"
    );
    let wait_for = |what: &str, done: &dyn Fn() -> bool| {
        let by = Instant::now() + Duration::from_secs(60);
        while !done() {
            assert!(Instant::now() < by, "{case}: never {what}: {}", told());
            crate::workspace_manager::fixture::rest(Duration::from_millis(5));
        }
    };
    let ended = |owned: &OwnedDescendants| {
        let by = Instant::now() + Duration::from_secs(60);
        loop {
            let now = observed(&backstop, owned);
            if matches!(now, Observed::Ended(_)) || Instant::now() >= by {
                break now;
            }
            crate::workspace_manager::fixture::rest(Duration::from_millis(10));
        }
    };
    let listing = |call: &String| call.starts_with("ps ") && call.contains("--quiet");
    let words = root.join("contender");
    std::fs::create_dir(&words).expect("the contender's words");
    let contender_vars = [
        ("PATH", path.as_os_str()),
        (LEASE_KILL_ROOT, root.as_os_str()),
        (HANDOFF_STATE, state.as_os_str()),
    ];
    let contender_said =
        || std::fs::read_to_string(root.join("contender.stderr")).unwrap_or_default();
    let start_contender = || {
        let mut contender = RunningTestChild::start(
            "runner::container::exec::tests::handoff_contender_child",
            &contender_vars,
            &root.join("contender.stdout"),
            &root.join("contender.stderr"),
        );
        let by = Instant::now() + Duration::from_secs(60);
        while !words.join("admitted").exists() {
            if let Some(status) = contender.exited() {
                panic!(
                    "{case}: the contender ended ({status:?}) before its admission: {}",
                    contender_said()
                );
            }
            assert!(
                Instant::now() < by,
                "{case}: the contender was never admitted to the worktree: {}",
                std::fs::read_to_string(root.join("contender.stderr")).unwrap_or_default()
            );
            crate::workspace_manager::fixture::rest(Duration::from_millis(5));
        }
        contender
    };
    let census = |contender: &mut RunningTestChild| {
        std::fs::write(words.join("census"), b"").expect("the contender's word");
        let by = Instant::now() + Duration::from_secs(60);
        loop {
            if let Ok(text) = std::fs::read_to_string(words.join("reclaimed")) {
                break text.lines().map(str::to_owned).collect::<Vec<String>>();
            }
            if let Some(status) = contender.exited() {
                panic!(
                    "{case}: the contender ended ({status:?}) before its census reported: {}",
                    contender_said()
                );
            }
            assert!(
                Instant::now() < by,
                "{case}: the contender's census never reported: {}",
                std::fs::read_to_string(root.join("contender.stderr")).unwrap_or_default()
            );
            crate::workspace_manager::fixture::rest(Duration::from_millis(5));
        }
    };
    let finish = |mut contender: RunningTestChild| {
        std::fs::write(words.join("release"), b"").expect("the contender's word");
        let by = Instant::now() + Duration::from_secs(60);
        loop {
            if let Some(status) = contender.exited() {
                break status.success();
            }
            assert!(Instant::now() < by, "{case}: the contender never ended");
            crate::workspace_manager::fixture::rest(Duration::from_millis(10));
        }
    };
    let dead_owner = format!("{container} foreign-run-dead-owner");

    match case.as_str() {
        "early" => {
            wait_for("held at its death", &|| arrived_at(&gate, "death"));
            let mut contender = start_contender();
            release("death");
            let ending = ended(&owned.owned);
            let calls = calls_of(&state, backstop.pid);
            let stood_down = matches!(ending, Observed::Ended(_))
                && calls.len() == 1
                && calls.iter().all(listing)
                && stub_containers(&state) == [container.clone()];
            crate::workspace_manager::fixture::say(&format!(
                "{case}: the backstop stood down for the admitted engine, having listed and \
                 removed nothing: {stood_down} ({ending:?}; its calls {calls:?})"
            ));
            assert!(
                stood_down,
                "{case}: the backstop stood down: {ending:?} {calls:?}"
            );
            let reclaimed = census(&mut contender);
            let converged = reclaimed == [dead_owner.clone()]
                && stub_containers(&state).is_empty()
                && intents().is_empty()
                && !view.exists();
            crate::workspace_manager::fixture::say(&format!(
                "{case}: the admitted engine's census reclaimed the container, its intent and \
                 its view: {converged} ({reclaimed:?})"
            ));
            assert!(converged, "{case}: the census converged: {reclaimed:?}");
            assert!(finish(contender), "{case}: the contender ended");
        }
        "middle" => {
            wait_for("held after its listing", &|| arrived_at(&gate, "listed"));
            let listed = calls_of(&state, backstop.pid);
            let unheld = !crate::rundir::observe_cleanup_hold(&public, &mut crate::rundir::NoHooks);
            assert!(
                listed.len() == 1 && listed.iter().all(listing) && unheld,
                "{case}: the backstop listed its labels holding nothing: {listed:?}, unheld {unheld}"
            );
            let mut contender = start_contender();
            release("listed");
            wait_for("held after its exclusion was decided", &|| {
                arrived_at(&gate, "decided")
            });
            let alive_before = observed(&backstop, &owned.owned);
            let no_late_hold =
                !crate::rundir::observe_cleanup_hold(&public, &mut crate::rundir::NoHooks)
                    && !crate::rundir::is_running(&public);
            let reclaimed = census(&mut contender);
            let alive_after = observed(&backstop, &owned.owned);
            let in_the_window = alive_before == Observed::Live && alive_after == Observed::Live;
            let converged = reclaimed == [dead_owner.clone()]
                && stub_containers(&state).is_empty()
                && intents().is_empty()
                && !view.exists();
            crate::workspace_manager::fixture::say(&format!(
                "{case}: inside the window the run showed no hold of the backstop's and the \
                 admitted engine's census reclaimed the container, its intent and its view: {} \
                 (backstop {alive_before:?} then {alive_after:?}; no late hold {no_late_hold}; \
                 reclaimed {reclaimed:?})",
                in_the_window && no_late_hold && converged
            ));
            assert!(
                in_the_window && no_late_hold && converged,
                "{case}: the census inside the window owned the cleanup (backstop \
                 {alive_before:?}/{alive_after:?}, no late hold {no_late_hold}, reclaimed \
                 {reclaimed:?})"
            );
            release("decided");
            let ending = ended(&owned.owned);
            let calls = calls_of(&state, backstop.pid);
            let removed_nothing = matches!(ending, Observed::Ended(_))
                && calls.len() == 1
                && calls.iter().all(listing);
            crate::workspace_manager::fixture::say(&format!(
                "{case}: the backstop then ended having removed nothing: {removed_nothing} \
                 ({ending:?}; its calls {calls:?})"
            ));
            assert!(removed_nothing, "{case}: {ending:?} {calls:?}");
            assert!(finish(contender), "{case}: the contender ended");
        }
        "late" => {
            wait_for("held with its exclusion established", &|| {
                arrived_at(&gate, "decided")
            });
            let held = crate::rundir::observe_cleanup_hold(&public, &mut crate::rundir::NoHooks);
            let distinct = crate::rundir::WorktreeLock::acquire_in(&layout.repo, &git_dir).err();
            let same_run = crate::rundir::RunLock::acquire(&public).err();
            let untouched = stub_containers(&state) == [container.clone()];
            let excluded = held && distinct.is_some() && same_run.is_some() && untouched;
            crate::workspace_manager::fixture::say(&format!(
                "{case}: with the backstop holding the worktree and its cleanup lease, a distinct \
                 run's admission and the run's own were refused: {excluded} (held {held}; \
                 {distinct:?}; {same_run:?})"
            ));
            assert!(
                excluded,
                "{case}: {held} {distinct:?} {same_run:?} {untouched}"
            );
            release("decided");
            let ending = ended(&owned.owned);
            let calls = calls_of(&state, backstop.pid);
            let removal = format!("rm --force --volumes {container}");
            let acted = matches!(ending, Observed::Ended(_))
                && calls.len() == 4
                && listing(&calls[0])
                && calls[1] == format!("kill {container}")
                && calls[2] == removal
                && listing(&calls[3])
                && stub_containers(&state).is_empty();
            crate::workspace_manager::fixture::say(&format!(
                "{case}: the backstop removed the container and ended: {acted} ({ending:?}; its \
                 calls {calls:?})"
            ));
            assert!(acted, "{case}: {ending:?} {calls:?}");
            let mut contender = start_contender();
            let reclaimed = census(&mut contender);
            let converged =
                reclaimed == [dead_owner.clone()] && intents().is_empty() && !view.exists();
            crate::workspace_manager::fixture::say(&format!(
                "{case}: the admitted engine's census reclaimed the intent and the view the \
                 backstop never touches: {converged} ({reclaimed:?})"
            ));
            assert!(converged, "{case}: {reclaimed:?}");
            assert!(finish(contender), "{case}: the contender ended");
        }
        other => panic!("unknown handoff case `{other}`"),
    }

    let settled = owned.owned.settle_naturally_within(Duration::from_secs(30));
    crate::workspace_manager::fixture::say(&format!(
        "{case}: every process the killed coordinator left ended on its own and was collected \
         here: {settled} ({} collected; errors {:?})",
        owned.owned.collected.len(),
        owned.owned.errors
    ));
    assert!(
        settled,
        "{case}: every process the killed coordinator left ended on its own and this harness \
         collected it, to the kernel's own account"
    );
}

#[cfg(target_os = "linux")]
fn the_handoff_guard(case: &str, lines: &[&str]) {
    let fixture = Fixture::built_in_a_bounded_child(&format!("handoff-{case}"), false);
    let bin = fixture.root.join("bin");
    std::fs::create_dir(&bin).expect("the stub's directory");
    let stub = bin.join("docker");
    std::fs::write(&stub, handoff_stub()).expect("the stub");
    crate::workspace_manager::fixture::set_mode(&stub, 0o755);
    let state = fixture.root.join("docker-state");
    std::fs::create_dir_all(state.join("containers")).expect("the stub's state");
    let inherited = std::env::var_os("PATH").unwrap_or_default();
    let path = std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(&inherited)))
        .expect("a PATH with the stub first");
    let ended = crate::agent::proc::test_support::run_test_isolated(
        "runner::container::exec::tests::handoff_harness_child",
        &[
            (HANDOFF_CASE, std::ffi::OsStr::new(case)),
            (LEASE_KILL_ROOT, fixture.root.as_os_str()),
            (HANDOFF_STATE, state.as_os_str()),
            ("PATH", path.as_os_str()),
        ],
        Duration::from_secs(300),
    );
    let said = format!("{}{}", ended.stdout, ended.stderr);
    let complete = said.contains("lease-kill collection: complete: true;")
        && !said.contains("running test child collection: not established");
    if !complete {
        crate::workspace_manager::fixture::say(&format!(
            "handoff collection FAILED: the fixture at {} is kept for what may still need it",
            fixture.root.display()
        ));
        std::mem::forget(fixture);
    }
    assert!(
        complete,
        "{case}: the harness ended and collected every process the killed coordinator left, \
         whatever else it found: {ended}"
    );
    assert!(
        ended.status.is_some_and(|status| status.success()) && ended.stdout.contains("1 passed"),
        "{case}: the bounded harness ended: {ended}"
    );
    let required = std::iter::once(format!(
        "{case}: between two helper questions nothing held the run's cleanup lease, the dormant \
         backstop included: true"
    ))
    .chain(lines.iter().map(|line| format!("{case}: {line}: true")))
    .chain([
        format!(
            "{case}: every process the killed coordinator left ended on its own and was \
             collected here: true"
        ),
        "lease-kill collection: complete: true;".to_owned(),
    ]);
    for line in required {
        assert!(said.contains(&line), "{case}: missing `{line}`: {ended}");
    }
}

/// A distinct fresh run admitted to the worktree before the dead coordinator's
/// backstop lists anything: the backstop cannot take the worktree, stands down
/// having removed nothing, and the admitted engine's census reclaims the
/// container, its intent and its view.
#[cfg(target_os = "linux")]
#[test]
fn a_backstop_stands_down_for_an_engine_admitted_to_the_worktree_before_its_listing() {
    the_handoff_guard(
        "early",
        &[
            "the backstop stood down for the admitted engine, having listed and removed nothing",
            "the admitted engine's census reclaimed the container, its intent and its view",
        ],
    );
}

/// The middle window: a distinct fresh run passes physical worktree admission
/// after the dead coordinator's backstop has listed its container, at the point
/// where a backstop taking its cleanup hold before the worktree's lock would
/// take it; with the backstop alive and not yet ended, the run shows no hold of
/// its, so the admitted engine's own census owns the cleanup and reclaims the
/// container, its intent and its view; the backstop then ends having removed
/// nothing.
#[cfg(target_os = "linux")]
#[test]
fn an_engine_admitted_between_a_backstops_listing_and_its_ending_owns_and_converges_the_cleanup() {
    the_handoff_guard(
        "middle",
        &[
            "inside the window the run showed no hold of the backstop's and the admitted \
             engine's census reclaimed the container, its intent and its view",
            "the backstop then ended having removed nothing",
        ],
    );
}

/// The backstop first: holding the worktree's lock and its cleanup lease, it
/// refuses a distinct run's worktree admission and the run's own admission,
/// removes the container and ends; the engine admitted after it reclaims the
/// intent and the view.
#[cfg(target_os = "linux")]
#[test]
fn a_backstop_that_holds_the_worktree_refuses_every_admission_until_it_has_removed_its_containers()
{
    the_handoff_guard(
        "late",
        &[
            "with the backstop holding the worktree and its cleanup lease, a distinct run's \
             admission and the run's own were refused",
            "the backstop removed the container and ended",
            "the admitted engine's census reclaimed the intent and the view the backstop never \
             touches",
        ],
    );
}

/// A child that ends at once, for the focused ownership check of
/// [`RunningTestChild`]'s cleanup.
#[cfg(target_os = "linux")]
#[test]
#[ignore = "a child that ends at once, for the running-test-child ownership check"]
fn a_child_that_ends_at_once() {}

/// The isolated harness of that check: it starts the child through a handle,
/// plays the other waiter itself by collecting the child by its pid, then ends
/// the handle and says whether the handle credited anything.
#[cfg(target_os = "linux")]
#[test]
#[ignore = "the isolated harness of the running-test-child ownership check"]
fn running_test_child_ownership_harness_child() {
    use crate::agent::proc::test_support::RunningTestChild;

    let dir =
        PathBuf::from(std::env::var_os(LEASE_KILL_ROOT).expect("the parent names a directory"));
    let child = RunningTestChild::start(
        "runner::container::exec::tests::a_child_that_ends_at_once",
        &[],
        &dir.join("child.stdout"),
        &dir.join("child.stderr"),
    );
    let pid = child.pid().expect("the child's pid");
    let by = Instant::now() + Duration::from_secs(60);
    let mut status = 0;
    loop {
        // SAFETY: `status` is writable and `WNOHANG` blocks on nothing; `pid`
        // is this isolated process's own child, which this loop is the second
        // waiter of on purpose.
        let answered = unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) };
        if answered == pid {
            break;
        }
        assert!(
            answered == 0 && Instant::now() < by,
            "the other waiter collected the child ({answered})"
        );
        crate::workspace_manager::fixture::rest(Duration::from_millis(10));
    }
    let credited = child.kill_and_collect();
    crate::workspace_manager::fixture::say(&format!(
        "a handle whose child another waiter collected credited nothing: {}",
        credited.is_none()
    ));
}

/// [`RunningTestChild`] fails closed: with its child's status already taken by
/// another waiter, ending the handle signals nothing -- its own `try_wait`
/// answers `ECHILD`, so the group it would have signalled is not known to be
/// its own -- records that collection is not established, and answers `None`,
/// which every harness here reports as no collection.
#[cfg(target_os = "linux")]
#[test]
fn a_running_test_child_whose_status_another_waiter_took_is_neither_signalled_nor_credited() {
    let tree = repo::scratch("running-test-child-ownership");
    let ended = crate::agent::proc::test_support::run_test_isolated(
        "runner::container::exec::tests::running_test_child_ownership_harness_child",
        &[(LEASE_KILL_ROOT, tree.path().as_os_str())],
        Duration::from_secs(120),
    );
    assert!(
        ended.status.is_some_and(|status| status.success()) && ended.stdout.contains("1 passed"),
        "the isolated harness ended: {ended}"
    );
    let said = format!("{}{}", ended.stdout, ended.stderr);
    for line in [
        "running test child collection: not established for ",
        "its own wait failed before any signal",
        "a handle whose child another waiter collected credited nothing: true",
    ] {
        assert!(said.contains(line), "missing `{line}`: {ended}");
    }
}
