//! Extended notes: `docs/internals/engine/topology/coordinator/tests/runners.md`

use super::*;

#[test]
fn every_container_invocation_is_launched_and_released_on_its_own_at_width_three() {
    use crate::runner::container::runtime::RuntimeOp;
    let tasks = three();
    let mut wide = Wide::durable_contained(
        "coordinator-contained",
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
    let names_env = wide.env.identity(INC_A);
    let mut points: Vec<InventoryAt> = Vec::new();
    let mut scheduler = Scheduler::scripted(
        &double,
        Box::new(|view: &Quiescent<'_>| {
            let mut expected: Vec<String> = view
                .invoking
                .iter()
                .map(|invocation| {
                    crate::runner::container::container_name_for(
                        &names_env.repo_key,
                        &names_env.run_id,
                        &names_env.incarnation,
                        invocation,
                    )
                })
                .collect();
            expected.sort();
            points.push((
                expected,
                running_in(&host),
                intents_under(&root),
                views_under(&root),
            ));
            None
        }),
    );
    let mut hooks = wide.env.hooks();
    let pipelines = wide.env.pipelines_over(contained.clone());
    let progress = wide
        .run
        .run_concurrently(
            &wide.env.seams_over(&*contained),
            &pipelines,
            &mut hooks,
            Some(&mut scheduler),
        )
        .expect("the contained run completes");
    let widest = scheduler.widest;
    drop(scheduler);
    assert_eq!(outcome_of(&progress), RunOutcome::Complete);
    assert_eq!(widest, 3, "three containers ran at once");
    for (expected, running, intents, views) in &points {
        assert_eq!(
            (running, intents, views),
            (expected, expected, expected),
            "at every quiescent point the containers running, the intents and the views are \
                 exactly those of the invocations inside the runner"
        );
    }
    assert!(
        host.container_names().is_empty()
            && intents_under(&root).is_empty()
            && views_under(&root).is_empty(),
        "every container, intent and view was released: {:?} {:?} {:?}",
        host.container_names(),
        intents_under(&root),
        views_under(&root)
    );
    let journal = host.journal();
    let created: Vec<&str> = journal
        .iter()
        .filter(|entry| entry.op == RuntimeOp::Create)
        .map(|entry| entry.target.as_str())
        .collect();
    let removed: Vec<&str> = journal
        .iter()
        .filter(|entry| entry.op == RuntimeOp::Remove)
        .map(|entry| entry.target.as_str())
        .collect();
    let ran = double.ran();
    assert_eq!(
        created.len(),
        ran.len(),
        "one container per process the double ran"
    );
    let mut created_sorted = created.clone();
    created_sorted.sort_unstable();
    created_sorted.dedup();
    assert_eq!(
        created_sorted.len(),
        created.len(),
        "no name was created twice"
    );
    let mut removed_sorted = removed.clone();
    removed_sorted.sort_unstable();
    assert_eq!(
        created_sorted, removed_sorted,
        "each container was removed once"
    );
    let ran_names = container_names_of(
        &wide,
        INC_A,
        &ran.iter()
            .map(|ran| ran.invocation.clone())
            .collect::<Vec<_>>(),
    );
    assert_eq!(
        created_sorted, ran_names,
        "each container is its own invocation's"
    );
    for ran in &ran {
        let name =
            container_names_of(&wide, INC_A, std::slice::from_ref(&ran.invocation)).remove(0);
        let volumes = journal
            .iter()
            .find(|entry| entry.op == RuntimeOp::Create && entry.target == name)
            .map(|entry| entry.detail.clone())
            .unwrap_or_default();
        let expected = if crate::runner::container::env::supplies_credential_location(&ran.role) {
            match ran.agent.as_ref().map(crate::runner::AgentId::as_str) {
                Some(crate::engine::topology::scaffold::AGENT) => {
                    crate::engine::topology::scaffold::WORKER_VOLUME
                }
                Some(crate::engine::topology::scaffold::REVIEW_AGENT) => {
                    crate::engine::topology::scaffold::REVIEWER_VOLUME
                }
                _ => "",
            }
        } else {
            ""
        };
        assert_eq!(
            volumes, expected,
            "{}: the credential volume of its agent, exactly when its role is given one",
            ran.invocation
        );
    }
    assert!(wide.run.invocations_balance());
    replay_equals_live(&wide);
}

const INC_1: &str = "01KZP5CONTAINERS1000000001";
const INC_2: &str = "01KZP5CONTAINERS2000000002";
const INC_3: &str = "01KZP5CONTAINERS3000000003";
const INC_FOREIGN: &str = "01KZP5CONTAINERSF00000000F";
const RUN_DEAD: &str = "01KZP5DEADOWNER00000000001";
const CONTAINER_CHILD: &str = "engine::topology::coordinator::tests::container_coordinator_child";

fn by(
    journal: &[crate::runner::container::Journaled],
    actor: &str,
    op: crate::runner::container::runtime::RuntimeOp,
) -> Vec<String> {
    journal
        .iter()
        .filter(|entry| entry.actor == actor && entry.op == op)
        .map(|entry| entry.target.clone())
        .collect()
}

fn await_starts(host: &crate::runner::container::FakeRuntime, actor: &str, count: usize) {
    assert!(
        host.await_journal(BOUND, |journal| {
            journal
                .iter()
                .filter(|entry| {
                    entry.actor == actor
                        && entry.op == crate::runner::container::runtime::RuntimeOp::Start
                })
                .count()
                >= count
        }),
        "`{actor}` did not start {count} container(s) within {BOUND:?}: {:?}",
        host.journal()
            .iter()
            .filter(|entry| entry.actor == actor)
            .collect::<Vec<_>>()
    );
}

fn holds_nothing(run_dir: &std::path::Path, repo: &std::path::Path) -> Result<(), String> {
    let started = std::time::Instant::now();
    while crate::rundir::observe_cleanup_hold(run_dir, &mut crate::rundir::NoHooks)
        && started.elapsed() < BOUND
    {
        crate::workspace_manager::fixture::rest_within(
            Duration::from_millis(20),
            BOUND.saturating_sub(started.elapsed()),
        );
    }
    if crate::rundir::is_running(run_dir) {
        return Err(format!("{} is held", run_dir.display()));
    }
    crate::rundir::WorktreeLock::acquire_in(repo, &repo.join(".git"))
        .map(drop)
        .map_err(|error| format!("the worktree lock of {}: {error}", repo.display()))
}

fn second_repository(root: &std::path::Path) -> std::path::PathBuf {
    let repo = root.join("repo-y");
    crate::workspace_manager::fixture::write_file(&repo.join("seed.txt"), b"y\n");
    crate::workspace_manager::fixture::git(&repo, &["init", "-q", "-b", "main"]);
    repo
}

fn census_names(report: &serde_json::Value, list: &str) -> Vec<(String, String)> {
    let mut names: Vec<(String, String)> = report["report"][list]
        .as_array()
        .unwrap_or_else(|| panic!("the census report lists `{list}`: {report}"))
        .iter()
        .map(|entry| {
            (
                entry["name"].as_str().unwrap_or_default().to_owned(),
                entry["ownership"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect();
    names.sort();
    names
}

fn served(
    logs: &std::path::Path,
    name: &str,
    env: &[(&str, &std::ffi::OsStr)],
    runtime: crate::runner::container::FakeRuntime,
) -> crate::engine::topology::scaffold::Served {
    let mut environment = env.to_vec();
    environment.extend(crate::engine::topology::scaffold::child_temporary_of(logs));
    crate::engine::topology::scaffold::Served::spawn(
        CONTAINER_CHILD,
        &environment,
        &logs.join(format!("{name}.stderr")),
        runtime,
    )
}

#[test]
fn a_foreign_census_reclaims_a_dead_coordinators_containers_and_leaves_a_live_coordinators_running()
{
    use crate::runner::container::runtime::RuntimeOp;
    use std::ffi::OsStr;
    let tasks = three();
    let mut wide = Wide::durable_contained(
        "coordinator-census-b",
        &tasks,
        3,
        WidePlans::default(),
        holding(&tasks, &[]),
        INC_A,
    );
    let host = crate::engine::topology::scaffold::container_host();
    let root = wide.env.fixture.root.clone();
    let private = wide.env.fixture.private.clone();
    let logs = crate::engine::topology::scaffold::kill_dir("census-b");
    let repo_key = wide.env.identity(INC_A).repo_key;

    let dead_dir = crate::rundir::public_dir(&root.join("repo-b"), RUN_DEAD);
    crate::workspace_manager::fixture::write_file(&dead_dir.join("created"), b"\n");
    let owner = served(
        logs.path(),
        "owner",
        &[
            ("UPSTROKE_TEST_CHILD_ROLE", OsStr::new("owner")),
            ("UPSTROKE_TEST_CHILD_RUN_DIR", dead_dir.as_os_str()),
            ("UPSTROKE_TEST_CHILD_PRIVATE", private.as_os_str()),
            ("UPSTROKE_TEST_CHILD_RUN", OsStr::new(RUN_DEAD)),
            ("UPSTROKE_TEST_CHILD_INCARNATION", OsStr::new(INC_1)),
            ("UPSTROKE_TEST_CHILD_REPO_KEY", OsStr::new(&repo_key)),
        ],
        host.acting_as("dead-owner"),
    );
    await_starts(&host, "dead-owner", 2);
    assert!(
        crate::rundir::is_running(&dead_dir),
        "the owner holds its run lock while its containers run"
    );
    let died = owner.kill();
    drop(owner);
    assert!(!died.success(), "the owner was killed: {died:?}");
    let dead: Vec<String> = {
        let mut dead = by(&host.journal(), "dead-owner", RuntimeOp::Start);
        dead.sort();
        dead
    };
    assert_eq!(dead.len(), 2);
    within(
        BOUND,
        "the dead owner's lock hold (R17) went with its process",
        || !crate::rundir::is_running(&dead_dir),
    );
    assert_eq!(
        running_in(&host),
        dead,
        "the dead owner's containers outlive it: its runner armed a reaper over the fake's \
             no-op reaper program, which reclaims nothing, so the census below is what does"
    );

    let repo_y = second_repository(&root);
    let contained = wide.env.contained(&host, INC_A);
    let double = std::sync::Arc::clone(&wide.env.runner);
    let mut census: Option<(
        serde_json::Value,
        serde_json::Value,
        Vec<String>,
        Vec<String>,
    )> = None;
    let mut scheduler = Scheduler::scripted(
        &double,
        Box::new(|view: &Quiescent<'_>| {
            if census.is_none() && view.invoking.len() == 3 {
                let live = running_in(&host);
                let foreign = served(
                    logs.path(),
                    "census",
                    &[
                        ("UPSTROKE_TEST_CHILD_ROLE", OsStr::new("census")),
                        ("UPSTROKE_TEST_CHILD_REPO", repo_y.as_os_str()),
                        ("UPSTROKE_TEST_CHILD_PRIVATE", private.as_os_str()),
                        ("UPSTROKE_TEST_CHILD_INCARNATION", OsStr::new(INC_FOREIGN)),
                        ("UPSTROKE_TEST_CHILD_USES_VOLUME", OsStr::new("1")),
                    ],
                    host.acting_as("foreign")
                        .starting(crate::engine::topology::scaffold::exiting()),
                );
                let report = foreign.event("the foreign census");
                let used = foreign.event("the foreign write command's first volume use");
                let status = foreign.exited("the foreign write command");
                assert!(status.success(), "{status:?}: {}", foreign.stderr());
                census = Some((report, used, live, running_in(&host)));
            }
            None
        }),
    );
    let mut hooks = wide.env.hooks();
    let pipelines = wide.env.pipelines_over(contained.clone());
    let progress = wide
        .run
        .run_concurrently(
            &wide.env.seams_over(&*contained),
            &pipelines,
            &mut hooks,
            Some(&mut scheduler),
        )
        .expect("the live coordinator completes");
    drop(scheduler);
    assert_eq!(outcome_of(&progress), RunOutcome::Complete);
    let (report, used, before, after) = census.expect("the foreign census ran at width three");

    let mut live: Vec<String> = before
        .iter()
        .filter(|name| !dead.contains(name))
        .cloned()
        .collect();
    live.sort();
    assert_eq!(
        live.len(),
        3,
        "the live coordinator had three containers running"
    );
    assert_eq!(report["prelock_refused"], true, "{report}");
    assert_eq!(report["report"]["census"], "complete", "{report}");
    assert_eq!(
        census_names(&report, "reclaimed"),
        dead.iter()
            .map(|name| (name.clone(), "foreign-run-dead-owner".to_owned()))
            .collect::<Vec<_>>(),
        "the census reclaimed the dead owner's containers and nothing else"
    );
    assert_eq!(
        census_names(&report, "untouched"),
        live.iter()
            .map(|name| (name.clone(), "foreign-run-live-owner".to_owned()))
            .collect::<Vec<_>>(),
        "the live coordinator's containers were classified live and left alone"
    );
    assert_eq!(after, live, "the live containers ran on across the census");

    let journal = host.journal();
    let foreign_effects: Vec<&crate::runner::container::Journaled> = journal
        .iter()
        .filter(|entry| entry.actor == "foreign" && entry.op.is_effect())
        .collect();
    for entry in &foreign_effects {
        assert!(
            !live.contains(&entry.target),
            "the foreign command touched a live container: {entry:?}"
        );
    }
    let used_name = used["used"].as_str().expect("the name it used").to_owned();
    assert_eq!(used["ran"], serde_json::json!({"Ok": 0}), "{used}");
    let position = |wanted: &dyn Fn(&crate::runner::container::Journaled) -> bool| {
        journal.iter().position(wanted)
    };
    let first_use = position(&|entry| {
        entry.actor == "foreign" && entry.op == RuntimeOp::Create && entry.target == used_name
    })
    .expect("the foreign command created its container");
    assert!(
        journal[first_use]
            .detail
            .contains(crate::engine::topology::scaffold::WORKER_VOLUME),
        "its first invocation mounts the shared credential volume: {:?}",
        journal[first_use]
    );
    for name in &dead {
        let removed = position(&|entry| {
            entry.actor == "foreign" && entry.op == RuntimeOp::Remove && entry.target == *name
        })
        .unwrap_or_else(|| panic!("`{name}` was removed by the census"));
        let terminated = journal
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                entry.actor == "foreign" && entry.op == RuntimeOp::Observe && entry.target == *name
            })
            .map(|(index, _)| index)
            .next()
            .unwrap_or_else(|| panic!("`{name}` was observed by the census"));
        assert!(
            terminated < first_use && removed < first_use,
            "`{name}` was observed terminated ({terminated}) and removed ({removed}) before \
                 the foreign command's first use of the credential volume ({first_use})"
        );
    }
    for name in &live {
        let removers: Vec<&str> = journal
            .iter()
            .filter(|entry| entry.target == *name && entry.op == RuntimeOp::Remove)
            .map(|entry| entry.actor.as_str())
            .collect();
        assert_eq!(
            removers,
            [INC_A],
            "`{name}` was released by its own coordinator only"
        );
    }
    assert!(
        host.container_names().is_empty()
            && intents_under(&private).is_empty()
            && views_under(&private).is_empty(),
        "nothing is left under the private root: {:?} {:?} {:?}",
        host.container_names(),
        intents_under(&private),
        views_under(&private)
    );
    assert!(wide.run.invocations_balance());
    let public = wide.env.paths.public.clone();
    let base = wide.env.fixture.base.clone();
    assert!(
        crate::rundir::is_running(&public),
        "the live coordinator's run lock is held until it ends"
    );
    let Wide { run, env } = wide;
    drop(run);
    holds_nothing(&public, &base).expect("the live coordinator's holds went with it");
    holds_nothing(&crate::rundir::public_dir(&repo_y, RUN_FOREIGN), &repo_y)
        .expect("the foreign write command's holds went with it");
    drop(env);
}

struct LedgerWatch<'p> {
    harness: crate::runner::container::HarnessHooks,
    preflight: &'p crate::engine::topology::preflight::RunPreflight<'p>,
    seen: Vec<(
        crate::topology::effects::EffectSiteId,
        (usize, usize),
        usize,
    )>,
}

impl crate::runner::container::ContainerHooks for LedgerWatch<'_> {
    fn phase(
        &mut self,
        site: crate::topology::effects::EffectSiteId,
        phase: crate::topology::effects::HookPhase,
    ) -> crate::topology::effects::Injection {
        self.seen.push((
            site,
            self.preflight.settlements(),
            self.preflight.running().len(),
        ));
        self.harness.phase(site, phase)
    }

    fn trace(&self) -> crate::runner::container::runtime::ContainerTrace {
        self.harness.trace()
    }
}

struct CensusWatch<'p> {
    inner: crate::engine::topology::seams::HarnessTopologyHooks,
    watch: LedgerWatch<'p>,
}

impl TopologyHooks for CensusWatch<'_> {
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
        &mut self.watch
    }

    fn spawn(&mut self) -> &mut dyn crate::agent::proc::SpawnHooks {
        self.inner.spawn()
    }

    fn folded(&mut self, fold: &TopologyFold, events: &[TopologyEvent]) {
        self.inner.folded(fold, events);
    }
}

fn killed_fresh_incarnation(
    host: &crate::runner::container::FakeRuntime,
    logs: &std::path::Path,
) -> (std::path::PathBuf, Vec<String>) {
    use std::ffi::OsStr;
    let first = served(
        logs,
        "first",
        &[
            ("UPSTROKE_TEST_CHILD_ROLE", OsStr::new("fresh")),
            ("UPSTROKE_TEST_CHILD_INCARNATION", OsStr::new(INC_1)),
        ],
        host.acting_as(INC_1),
    );
    let root = std::path::PathBuf::from(
        first.event("the first incarnation's fixture")["root"]
            .as_str()
            .expect("a fixture root"),
    );
    await_starts(host, INC_1, 3);
    let died = first.kill();
    assert!(
        !died.success(),
        "the first incarnation was killed: {died:?}"
    );
    drop(first);
    let started = sorted(by(
        &host.journal(),
        INC_1,
        crate::runner::container::runtime::RuntimeOp::Start,
    ));
    (root, started)
}

fn foreign_census_at_width_three(
    host: &crate::runner::container::FakeRuntime,
    logs: &std::path::Path,
    repo: &std::path::Path,
    private: &std::path::Path,
) -> serde_json::Value {
    use std::ffi::OsStr;
    let foreign = served(
        logs,
        "foreign",
        &[
            ("UPSTROKE_TEST_CHILD_ROLE", OsStr::new("census")),
            ("UPSTROKE_TEST_CHILD_REPO", repo.as_os_str()),
            ("UPSTROKE_TEST_CHILD_PRIVATE", private.as_os_str()),
            ("UPSTROKE_TEST_CHILD_INCARNATION", OsStr::new(INC_FOREIGN)),
        ],
        host.acting_as("foreign"),
    );
    let report = foreign.event("the foreign census");
    let status = foreign.exited("the foreign write command");
    assert!(status.success(), "{status:?}: {}", foreign.stderr());
    report
}

#[test]
fn a_resuming_incarnation_reclaims_its_earlier_incarnations_containers_before_its_ledgers_probes_and_admission()
 {
    use crate::runner::container::runtime::RuntimeOp;
    use std::ffi::OsStr;
    let tasks = three();
    let host = crate::engine::topology::scaffold::container_host();
    let logs = crate::engine::topology::scaffold::kill_dir("resume-f");
    let (root, first) = killed_fresh_incarnation(&host, logs.path());
    let env = crate::engine::topology::scaffold::WideEnv::adopted(
        root.clone(),
        &tasks,
        3,
        WidePlans::default(),
    );
    holds_nothing(&env.paths.public, &env.fixture.base)
        .expect("the first incarnation's holds went with its process");
    assert_eq!(
        running_in(&host),
        first,
        "its three containers outlive it: its runner's reaper runs the fake's no-op reaper \
             program, so the census below is what reclaims them"
    );

    let second = served(
        logs.path(),
        "second",
        &[
            ("UPSTROKE_TEST_CHILD_ROLE", OsStr::new("resume")),
            ("UPSTROKE_TEST_CHILD_ROOT", root.as_os_str()),
            ("UPSTROKE_TEST_CHILD_INCARNATION", OsStr::new(INC_2)),
        ],
        host.acting_as(INC_2),
    );
    await_starts(&host, INC_2, 1);
    let died = second.kill();
    assert!(
        !died.success(),
        "the second incarnation was killed: {died:?}"
    );
    drop(second);
    let journal = host.journal();
    assert_eq!(
        sorted(by(&journal, INC_2, RuntimeOp::Remove)),
        first,
        "the second incarnation's census reclaimed the first incarnation's containers"
    );
    let orphan = by(&journal, INC_2, RuntimeOp::Start);
    assert_eq!(
        orphan.len(),
        1,
        "it died inside its own shell probe: {orphan:?}"
    );
    let orphan = orphan[0].clone();
    assert_eq!(running_in(&host), std::slice::from_ref(&orphan));
    holds_nothing(&env.paths.public, &env.fixture.base)
        .expect("the second incarnation's holds went with its process");

    let base = env.fixture.base.clone();
    let private = env.fixture.private.clone();
    let adapters = std::sync::Arc::clone(&env.adapters);
    let probes = crate::engine::topology::scaffold::container_runner(
        env.identity(INC_3),
        &base,
        Box::new(
            host.acting_as("third-probes")
                .starting(crate::engine::topology::scaffold::exiting()),
        ),
        Duration::from_millis(5),
    );
    let preflight = crate::engine::topology::preflight::RunPreflight::new(
        &probes,
        &*adapters,
        crate::gates::ShellKind::Sh,
        &base,
        Vec::new(),
    );
    let census_runtime = host.acting_as("third-census");
    let mut hooks = CensusWatch {
        inner: env.hooks(),
        watch: LedgerWatch {
            harness: crate::runner::container::HarnessHooks::new(std::sync::Arc::clone(
                &env.harness,
            )),
            preflight: &preflight,
            seen: Vec::new(),
        },
    };
    let (recovered, mut wide) = env
        .resume_over(
            INC_3,
            holding(&tasks, &[]),
            crate::engine::topology::select::Ceiling::unlimited(),
            &crate::engine::topology::scaffold::ResumingOver {
                runtime: &census_runtime,
                liveness: &crate::runner::container::runtime::LockProbe,
                preflight: &preflight,
                awaits_release: true,
            },
            &mut hooks,
        )
        .expect("the third incarnation resumes");
    let seen = std::mem::take(&mut hooks.watch.seen);
    drop(hooks);
    assert_eq!(
        recovered.interrupted, 3,
        "the first incarnation's three attempts"
    );
    broker_is_empty(&mut wide.run).expect("the broker starts empty");

    let journal = host.journal();
    assert_eq!(
        by(&journal, "third-census", RuntimeOp::Remove),
        std::slice::from_ref(&orphan),
        "the third incarnation's census reclaimed the second incarnation's probe"
    );
    let own = by(&journal, "third-probes", RuntimeOp::Create);
    assert_eq!(own.len(), 1, "one shell probe: {own:?}");
    let own = own[0].clone();
    let (orphan_parts, own_parts) = (
        crate::runner::container::container_name_parts(&orphan).expect("the orphan's name"),
        crate::runner::container::container_name_parts(&own).expect("its own name"),
    );
    let shell = crate::runner::container::intent::invocation_hash(
        &crate::engine::topology::identity::PreflightIdentities::shell(0)
            .expect("the shell probe's identity"),
    );
    assert_eq!(
        (orphan_parts.1.as_str(), own_parts.1.as_str()),
        (shell.as_str(), shell.as_str()),
        "one deterministic InvocationId in both incarnations"
    );
    assert_eq!(
        (orphan_parts.0.as_str(), own_parts.0.as_str()),
        (INC_2, INC_3)
    );
    assert_ne!(
        crate::runner::container::intent_path_for(&private, &orphan),
        crate::runner::container::intent_path_for(&private, &own),
        "the two incarnations' records never share a path"
    );
    let first_own = journal
        .iter()
        .position(|entry| entry.actor == "third-probes")
        .expect("its probe ran");
    let last_reclaim = journal
        .iter()
        .rposition(|entry| entry.actor == "third-census" && entry.target == orphan)
        .expect("the census acted on the orphan");
    assert!(
        last_reclaim < first_own,
        "the census finished with the orphan ({last_reclaim}) before this incarnation's first \
             container operation ({first_own})"
    );
    let reclaims: Vec<&(
        crate::topology::effects::EffectSiteId,
        (usize, usize),
        usize,
    )> = seen
        .iter()
        .filter(|(site, _, _)| {
            matches!(
                site,
                crate::topology::effects::EffectSiteId::Container(
                    crate::topology::effects::ContainerSite::Stop
                        | crate::topology::effects::ContainerSite::Remove
                        | crate::topology::effects::ContainerSite::UnmountGitView
                        | crate::topology::effects::ContainerSite::RemoveIntent
                )
            )
        })
        .collect();
    assert!(
        !reclaims.is_empty(),
        "the census's reclaim steps were observed: {seen:?}"
    );
    for (site, settled, running) in &reclaims {
        assert_eq!(
            (*settled, *running),
            ((0, 0), 0),
            "{site:?}: the pre-flight's ledger held nothing while the census reclaimed"
        );
    }
    assert_eq!(
        preflight.settlements(),
        (1, 0),
        "then its shell probe ran and settled"
    );

    let repo_y = second_repository(&root);
    let contained = wide.env.contained(&host, INC_3);
    let double = std::sync::Arc::clone(&wide.env.runner);
    let mut foreign: Option<(serde_json::Value, Vec<String>, Vec<String>)> = None;
    let mut scheduler = Scheduler::scripted(
        &double,
        Box::new(|view: &Quiescent<'_>| {
            if foreign.is_none() && view.invoking.len() == 3 {
                let before = running_in(&host);
                let report = foreign_census_at_width_three(&host, logs.path(), &repo_y, &private);
                foreign = Some((report, before, running_in(&host)));
            }
            None
        }),
    );
    let mut hooks = wide.env.hooks();
    let pipelines = wide.env.pipelines_over(contained.clone());
    let progress = wide
        .run
        .run_concurrently(
            &wide.env.seams_over(&*contained),
            &pipelines,
            &mut hooks,
            Some(&mut scheduler),
        )
        .expect("the third incarnation completes");
    drop(scheduler);
    assert_eq!(outcome_of(&progress), RunOutcome::Complete);
    let (report, before, after) = foreign.expect("a foreign census ran at width three");
    assert_eq!(before.len(), 3, "{before:?}");
    assert_eq!(report["report"]["census"], "complete", "{report}");
    assert!(census_names(&report, "reclaimed").is_empty(), "{report}");
    assert_eq!(
        census_names(&report, "untouched"),
        before
            .iter()
            .map(|name| (name.clone(), "foreign-run-live-owner".to_owned()))
            .collect::<Vec<_>>(),
        "the containers it started after its census are a live owner's"
    );
    assert_eq!(after, before, "and they ran on");
    assert!(
        host.container_names().is_empty()
            && intents_under(&private).is_empty()
            && views_under(&private).is_empty(),
        "{:?} {:?} {:?}",
        host.container_names(),
        intents_under(&private),
        views_under(&private)
    );
    let (public, base) = (wide.env.paths.public.clone(), wide.env.fixture.base.clone());
    let Wide { run, env } = wide;
    drop(run);
    holds_nothing(&public, &base).expect("the third incarnation's holds went with it");
    drop(env);
}

#[test]
fn crashes_across_three_incarnations_with_pipelines_in_flight_leave_every_orphan_reclaimed_and_no_name_twice()
 {
    use crate::runner::container::runtime::RuntimeOp;
    use std::ffi::OsStr;
    let tasks = three();
    let host = crate::engine::topology::scaffold::container_host();
    let logs = crate::engine::topology::scaffold::kill_dir("crashes-g");
    let (root, first) = killed_fresh_incarnation(&host, logs.path());

    let second = served(
        logs.path(),
        "second",
        &[
            ("UPSTROKE_TEST_CHILD_ROLE", OsStr::new("resume")),
            ("UPSTROKE_TEST_CHILD_ROOT", root.as_os_str()),
            ("UPSTROKE_TEST_CHILD_INCARNATION", OsStr::new(INC_2)),
        ],
        host.acting_as(INC_2)
            .starting(crate::engine::topology::scaffold::exiting_probes()),
    );
    assert_eq!(
        second.event("the second incarnation's recovery")["interrupted"],
        3,
        "it settled the first incarnation's three attempts interrupted"
    );
    await_starts(&host, INC_2, 4);
    let died = second.kill();
    assert!(
        !died.success(),
        "the second incarnation was killed: {died:?}"
    );
    drop(second);
    let journal = host.journal();
    let probe_hash = crate::runner::container::intent::invocation_hash(
        &crate::engine::topology::identity::PreflightIdentities::shell(0)
            .expect("the shell probe's identity"),
    );
    let is_probe = |name: &String| {
        crate::runner::container::container_name_parts(name)
            .is_some_and(|(_, invocation_hash)| invocation_hash == probe_hash)
    };
    let second_pipelines: Vec<String> = sorted(
        by(&journal, INC_2, RuntimeOp::Start)
            .into_iter()
            .filter(|name| !is_probe(name))
            .collect(),
    );
    assert_eq!(second_pipelines.len(), 3, "{second_pipelines:?}");
    assert_eq!(
        sorted(
            by(&journal, INC_2, RuntimeOp::Remove)
                .into_iter()
                .filter(|name| !is_probe(name))
                .collect()
        ),
        first,
        "the second incarnation reclaimed the first incarnation's orphans"
    );
    assert_eq!(running_in(&host), second_pipelines);

    let env = crate::engine::topology::scaffold::WideEnv::adopted(
        root.clone(),
        &tasks,
        3,
        WidePlans::default(),
    );
    holds_nothing(&env.paths.public, &env.fixture.base)
        .expect("the second incarnation's holds went with its process");
    let base = env.fixture.base.clone();
    let private = env.fixture.private.clone();
    let adapters = std::sync::Arc::clone(&env.adapters);
    let probes = crate::engine::topology::scaffold::container_runner(
        env.identity(INC_3),
        &base,
        Box::new(
            host.acting_as("third-probes")
                .starting(crate::engine::topology::scaffold::exiting()),
        ),
        Duration::from_millis(5),
    );
    let preflight = crate::engine::topology::preflight::RunPreflight::new(
        &probes,
        &*adapters,
        crate::gates::ShellKind::Sh,
        &base,
        Vec::new(),
    );
    let census_runtime = host.acting_as("third-census");
    let mut hooks = env.hooks();
    let (recovered, mut wide) = env
        .resume_over(
            INC_3,
            holding(&tasks, &[]),
            crate::engine::topology::select::Ceiling::unlimited(),
            &crate::engine::topology::scaffold::ResumingOver {
                runtime: &census_runtime,
                liveness: &crate::runner::container::runtime::LockProbe,
                preflight: &preflight,
                awaits_release: true,
            },
            &mut hooks,
        )
        .expect("the third incarnation resumes");
    assert_eq!(
        recovered.interrupted, 3,
        "the second incarnation's three attempts"
    );
    broker_is_empty(&mut wide.run).expect("the broker starts empty");
    assert_eq!(
        sorted(by(&host.journal(), "third-census", RuntimeOp::Remove)),
        second_pipelines,
        "the third incarnation reclaimed the second incarnation's orphans"
    );

    let contained = wide.env.contained(&host, INC_3);
    let double = std::sync::Arc::clone(&wide.env.runner);
    let mut scheduler = Scheduler::first(&double);
    let mut hooks = wide.env.hooks();
    let pipelines = wide.env.pipelines_over(contained.clone());
    let progress = wide
        .run
        .run_concurrently(
            &wide.env.seams_over(&*contained),
            &pipelines,
            &mut hooks,
            Some(&mut scheduler),
        )
        .expect("the third incarnation completes");
    drop(scheduler);
    assert_eq!(outcome_of(&progress), RunOutcome::Complete);

    let journal = host.journal();
    let created: Vec<String> = journal
        .iter()
        .filter(|entry| entry.op == RuntimeOp::Create)
        .map(|entry| entry.target.clone())
        .collect();
    let distinct = sorted(created.clone());
    let mut deduped = distinct.clone();
    deduped.dedup();
    assert_eq!(deduped, distinct, "no container name was created twice");
    let intents: std::collections::BTreeSet<std::path::PathBuf> = created
        .iter()
        .map(|name| {
            crate::runner::container::intent_path_for(&private, name).expect("a container name")
        })
        .collect();
    assert_eq!(intents.len(), created.len(), "and no intent path twice");
    let by_hash = |incarnation: &str| -> std::collections::BTreeMap<String, String> {
        created
            .iter()
            .filter_map(|name| {
                let (of, invocation_hash) =
                    crate::runner::container::container_name_parts(name).expect("a container name");
                (of == incarnation).then(|| (invocation_hash, name.clone()))
            })
            .collect()
    };
    let (one, two, three) = (by_hash(INC_1), by_hash(INC_2), by_hash(INC_3));
    let shared: Vec<&String> = two
        .keys()
        .filter(|hash| three.contains_key(*hash))
        .collect();
    assert!(
        shared.contains(&&probe_hash),
        "the second and third incarnations ran one deterministic probe identity: {shared:?}"
    );
    for hash in one.keys().chain(two.keys()) {
        let names: Vec<&String> = [&one, &two, &three]
            .into_iter()
            .filter_map(|of| of.get(hash))
            .collect();
        let mut unique = names.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(
            unique.len(),
            names.len(),
            "{hash}: one name per incarnation"
        );
    }
    assert_eq!(
        count(&wide.env.durable_events(), "attempt_interrupted"),
        6,
        "each dead incarnation's in-flight attempts were settled once"
    );
    assert_eq!(count(&wide.env.durable_events(), "run_resumed"), 2);
    assert!(
        host.container_names().is_empty()
            && intents_under(&private).is_empty()
            && views_under(&private).is_empty(),
        "{:?} {:?} {:?}",
        host.container_names(),
        intents_under(&private),
        views_under(&private)
    );
}

#[test]
fn a_foreign_census_and_a_resuming_incarnation_converge_on_one_dead_container_as_two_processes() {
    use crate::runner::container::runtime::RuntimeOp;
    use std::ffi::OsStr;
    let tasks = three();
    let wide = Wide::durable_contained(
        "coordinator-converge-h",
        &tasks,
        3,
        WidePlans::default(),
        RecordingRunner::new(),
        INC_1,
    );
    let host = crate::engine::topology::scaffold::container_host();
    let logs = crate::engine::topology::scaffold::kill_dir("converge-h");
    let root = wide.env.fixture.root.clone();
    let private = wide.env.fixture.private.clone();
    let repo_key = wide.env.identity(INC_1).repo_key;
    let dead_dir = crate::rundir::public_dir(&root.join("repo-dead"), RUN_DEAD);
    crate::workspace_manager::fixture::write_file(&dead_dir.join("created"), b"\n");
    let owner = served(
        logs.path(),
        "owner",
        &[
            ("UPSTROKE_TEST_CHILD_ROLE", OsStr::new("owner")),
            ("UPSTROKE_TEST_CHILD_RUN_DIR", dead_dir.as_os_str()),
            ("UPSTROKE_TEST_CHILD_PRIVATE", private.as_os_str()),
            ("UPSTROKE_TEST_CHILD_RUN", OsStr::new(RUN_DEAD)),
            ("UPSTROKE_TEST_CHILD_INCARNATION", OsStr::new(INC_FOREIGN)),
            ("UPSTROKE_TEST_CHILD_REPO_KEY", OsStr::new(&repo_key)),
        ],
        host.acting_as("dead-owner"),
    );
    await_starts(&host, "dead-owner", 2);
    let died = owner.kill();
    assert!(!died.success(), "the owner was killed: {died:?}");
    drop(owner);
    let dead = sorted(by(&host.journal(), "dead-owner", RuntimeOp::Start));
    let contested = dead[0].clone();
    host.pace(&contested, &[RuntimeOp::Stop, RuntimeOp::Remove], 2);

    let Wide { run, env } = wide;
    drop(run);
    holds_nothing(&env.paths.public, &env.fixture.base)
        .expect("the first incarnation ended holding nothing");
    let repo_y = second_repository(&root);
    let foreign = served(
        logs.path(),
        "foreign",
        &[
            ("UPSTROKE_TEST_CHILD_ROLE", OsStr::new("census")),
            ("UPSTROKE_TEST_CHILD_REPO", repo_y.as_os_str()),
            ("UPSTROKE_TEST_CHILD_PRIVATE", private.as_os_str()),
            ("UPSTROKE_TEST_CHILD_INCARNATION", OsStr::new(INC_FOREIGN)),
        ],
        host.acting_as("foreign"),
    );
    let census_runtime = host.acting_as("resuming");
    let mut hooks = env.hooks();
    let (recovered, resumed) = env
        .resume_over(
            INC_2,
            RecordingRunner::new(),
            crate::engine::topology::select::Ceiling::unlimited(),
            &crate::engine::topology::scaffold::ResumingOver {
                runtime: &census_runtime,
                liveness: &crate::runner::container::runtime::LockProbe,
                preflight: &crate::engine::topology::scaffold::Certifying,
                awaits_release: true,
            },
            &mut hooks,
        )
        .expect("the resuming incarnation converges and resumes");
    let report = foreign.event("the foreign census");
    let status = foreign.exited("the foreign write command");
    assert!(status.success(), "{status:?}: {}", foreign.stderr());
    drop(foreign);
    assert_eq!(recovered.interrupted, 0);
    assert_eq!(report["report"]["census"], "complete", "{report}");
    assert!(
        census_names(&report, "reclaimed")
            .iter()
            .any(|(name, ownership)| *name == contested && ownership == "foreign-run-dead-owner"),
        "the foreign command reclaimed the contested container: {report}"
    );
    let journal = host.journal();
    for op in [RuntimeOp::Stop, RuntimeOp::Remove] {
        let actors: std::collections::BTreeSet<&str> = journal
            .iter()
            .filter(|entry| entry.op == op && entry.target == contested)
            .map(|entry| entry.actor.as_str())
            .collect();
        assert_eq!(
            actors,
            ["foreign", "resuming"].into_iter().collect(),
            "{op}: both reclaimers acted on the contested container"
        );
    }
    assert!(
        host.container_names().is_empty()
            && intents_under(&private).is_empty()
            && views_under(&private).is_empty(),
        "both reclaimers converged on a clean root: {:?} {:?} {:?}",
        host.container_names(),
        intents_under(&private),
        views_under(&private)
    );
    drop(resumed);
}

#[cfg(unix)]
struct CarriedLeases {
    inner: std::sync::Arc<RecordingRunner>,
    carried: std::sync::Mutex<Vec<(InvocationId, Vec<std::path::PathBuf>)>>,
}

#[cfg(unix)]
impl crate::runner::Runner for CarriedLeases {
    fn run<'a>(
        &'a self,
        request: &'a crate::runner::RunnerRequest,
        call: crate::runner::RunnerCall<'a>,
    ) -> crate::runner::RunFuture<'a> {
        let parts = call.into_parts();
        assert!(parts.spawn.is_none() && parts.container.is_none());
        self.carried
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((request.invocation.clone(), parts.leases.to_vec()));
        self.inner.run(
            request,
            crate::runner::RunnerCall::new(parts.cancellation).holding_cleanup_leases(parts.leases),
        )
    }
}

#[cfg(unix)]
#[test]
fn a_surviving_reaper_hold_refuses_the_next_coordinator_until_released_and_is_never_reset_at_width_three()
 {
    let tasks = three();
    let mut wide = Wide::durable(
        "coordinator-reaper-hold",
        &tasks,
        3,
        WidePlans::default(),
        holding(&tasks, &[]),
    );
    let double = std::sync::Arc::clone(&wide.env.runner);
    let carrying = std::sync::Arc::new(CarriedLeases {
        inner: std::sync::Arc::clone(&double),
        carried: std::sync::Mutex::new(Vec::new()),
    });
    let mut scheduler = shutdown_at_first_point(&double);
    let mut hooks = wide.env.hooks();
    let pipelines = wide.env.pipelines_over(
        std::sync::Arc::clone(&carrying) as std::sync::Arc<dyn crate::runner::Runner>
    );
    let error = wide
        .run
        .run_concurrently(
            &wide.env.seams_over(&*carrying),
            &pipelines,
            &mut hooks,
            Some(&mut scheduler),
        )
        .expect_err("a shutdown ends the command");
    drop(scheduler);
    assert!(error.to_string().contains("shut down"), "{error}");
    let public = wide.env.paths.public.clone();
    let carried = carrying
        .carried
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    assert_eq!(carried.len(), 3, "three workers were granted: {carried:?}");
    let lease = std::fs::canonicalize(&public)
        .expect("the run's public directory")
        .join("cleanup.lock");
    for (invocation, leases) in &carried {
        let leases: Vec<std::path::PathBuf> = leases
            .iter()
            .map(|path| std::fs::canonicalize(path).expect("a carried lease path"))
            .collect();
        assert_eq!(
            leases,
            std::slice::from_ref(&lease),
            "`{invocation}`, spawned from a pipeline thread, carries the run's cleanup lease \
                 to its reaper"
        );
    }

    let Wide { run, env } = wide;
    drop(run);
    let mut reaper = crate::workspace_manager::fixture::spawn_ready_helper(
        "rundir::tests::cleanup_hold_child",
        &[("UPSTROKE_TEST_CLEANUP_DIR", public.as_os_str())],
    );
    reaper
        .await_line("held", BOUND)
        .or_fail("the surviving reaper never took its hold");
    assert!(
        crate::rundir::observe_cleanup_hold(&public, &mut crate::rundir::NoHooks),
        "a surviving reaper holds R28"
    );
    let runtime = crate::runner::container::FakeRuntime::new(
        crate::runner::container::runtime::ContainerTrace::off(),
    );
    let liveness = crate::runner::container::FakeOwnerLiveness::new();
    let mut hooks = env.hooks();
    let (refused, env) = match env.try_resume_over(
        INC_2,
        holding(&tasks, &[]),
        crate::engine::topology::select::Ceiling::unlimited(),
        &crate::engine::topology::scaffold::ResumingOver {
            runtime: &runtime,
            liveness: &liveness,
            preflight: &crate::engine::topology::scaffold::Certifying,
            awaits_release: false,
        },
        &mut hooks,
    ) {
        Ok(_) => panic!("the next coordinator resumed over a surviving reaper's hold"),
        Err(refused) => *refused,
    };
    assert!(
        refused
            .to_string()
            .contains("still has a process of its own alive"),
        "the refusal names the hold it observed: {refused}"
    );
    assert!(
        crate::rundir::observe_cleanup_hold(&public, &mut crate::rundir::NoHooks),
        "the refused coordinator left the reaper's hold as it found it"
    );
    drop(reaper);
    let (recovered, mut resumed) = env
        .resume(
            INC_2,
            RecordingRunner::new().answering(wide_responder(&tasks, &[])),
            crate::engine::topology::select::Ceiling::unlimited(),
        )
        .expect("once the hold is released the next coordinator resumes");
    assert_eq!(recovered.interrupted, 3);
    broker_is_empty(&mut resumed.run).expect("its ledgers start empty");
    let progress = drive(&mut resumed, None).expect("the resumed run completes");
    assert_eq!(outcome_of(&progress), RunOutcome::Complete);
    assert!(resumed.run.invocations_balance());
}

#[cfg(unix)]
const F_HOSTED_AGENT: &str = r#"#!/bin/sh
printf '%s %s\n' "$1" "$(pwd -P)" >> "$UPSTROKE_TEST_F_ROLE_LOG"
if [ "$1" = --implement ]; then
  if [ "$2" = alpha ] && [ ! -e "$UPSTROKE_TEST_F_WORKER_MARKER" ]; then
    : > "$UPSTROKE_TEST_F_WORKER_MARKER"
    exit 0
  fi
  mkdir -p "src/$2" && printf '%s worked\n' "$2" > "src/$2/work.txt"
fi
if [ "$1" = --prompt ]; then
  printf '%s\n' "$UPSTROKE_TEST_F_VERDICT"
fi
"#;

#[cfg(unix)]
const F_HOSTED_GATE: &str = r#"#!/bin/sh
printf 'gate %s\n' "$(pwd -P)" >> "$UPSTROKE_TEST_F_ROLE_LOG"
common=$(git rev-parse --git-common-dir) || exit 2
real=$(cd "$common" && pwd -P) || exit 3
[ "$real" = "$UPSTROKE_TEST_F_RUN_COMMON" ] || exit 4
git cat-file -e 'HEAD^{tree}'
"#;

fn f_hosted_linked_registry(sequence: &str, child: &str, completes: bool) {
    let tasks = f_hosted_tasks();
    let below = std::path::Path::new(if sequence == "C5" {
        crate::workspace_manager::fixture::EQUAL_DEPTH_BASE
    } else {
        "repo"
    });
    let Wide { run, env } = if sequence == "C5" {
        Wide::durable_based(
            "f-hosted-c5",
            below,
            &tasks,
            2,
            WidePlans::default(),
            RecordingRunner::new(),
        )
    } else {
        Wide::durable(
            "f-hosted-c4",
            &tasks,
            2,
            WidePlans::default(),
            RecordingRunner::new(),
        )
    };
    drop(run);
    let root = crate::workspace_manager::fixture::canonical(&env.fixture.root);
    let base = crate::workspace_manager::fixture::canonical(&env.fixture.base);
    let common = env.fixture.manager.common_git_dir().to_path_buf();
    assert_eq!(
        crate::workspace_manager::fixture::canonical(&base.join(".git")),
        common,
        "the run's common directory is the base's own"
    );
    assert!(
        root.parent().is_some(),
        "the scratch root is not the filesystem root"
    );
    let other = env.fixture.root.join("other");
    crate::workspace_manager::fixture::create_dir(&other);
    crate::workspace_manager::fixture::git(&other, &["init", "-q", "-b", "main"]);
    let elsewhere = other.join(".git").join("worktrees");
    crate::workspace_manager::fixture::create_dir(&elsewhere);
    crate::workspace_manager::fixture::link_directory(
        &elsewhere,
        &env.fixture.base.join(".git").join("worktrees"),
    );
    let value = if sequence == "C5" {
        let depth = base
            .strip_prefix(&root)
            .expect("the base is below the root")
            .components()
            .count();
        let named = common
            .strip_prefix(&root)
            .expect("the common directory is below the root");
        let mut relative = std::path::PathBuf::new();
        for _ in 0..depth {
            relative.push("..");
        }
        relative.push(named);
        let work_trees: Vec<std::path::PathBuf> = f_hosted_placements()
            .into_iter()
            .map(|(_, slot, _)| slot)
            .chain(std::iter::once(
                crate::engine::topology::integrate::staging_slot(F_HOSTED_VERIFIED),
            ))
            .map(|slot| env.fixture.manager.slot_path(&slot))
            .collect();
        for from in std::iter::once(&base).chain(&work_trees) {
            assert_eq!(
                from.ancestors().nth(depth).map(|top| top.join(named)),
                Some(common.clone()),
                "`{}` from {} names the run's common directory, climbing to the scratch \
                     root and no further",
                relative.display(),
                from.display()
            );
        }
        relative.into_os_string()
    } else {
        common.clone().into_os_string()
    };
    let bin = env.fixture.root.join("bin");
    #[cfg(unix)]
    for (name, script) in [
        (crate::engine::topology::scaffold::AGENT, F_HOSTED_AGENT),
        (
            crate::engine::topology::scaffold::REVIEW_AGENT,
            F_HOSTED_AGENT,
        ),
        ("gate", F_HOSTED_GATE),
    ] {
        let path = bin.join(name);
        crate::workspace_manager::fixture::write_file(&path, script.as_bytes());
        crate::workspace_manager::fixture::set_mode(&path, 0o755);
    }
    #[cfg(windows)]
    for (name, role) in [
        (crate::engine::topology::scaffold::AGENT, "%~1"),
        (crate::engine::topology::scaffold::REVIEW_AGENT, "%~1"),
        ("gate", "gate"),
    ] {
        crate::workspace_manager::fixture::write_file(
            &bin.join(format!("{name}.cmd")),
            f_hosted_shim(role).as_bytes(),
        );
    }
    let path = std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    )))
    .expect("a PATH that holds the scripts' directory");
    let marker = env.fixture.root.join("worker-wrote-nothing");
    let log = env.fixture.root.join("roles.log");
    let test = format!("engine::topology::coordinator::tests::{child}");
    let vars = [
        ("GIT_COMMON_DIR", value.as_os_str()),
        ("PATH", path.as_os_str()),
        (F_HOSTED_ROOT, env.fixture.root.as_os_str()),
        (F_HOSTED_BASE, below.as_os_str()),
        ("UPSTROKE_TEST_F_RUN_COMMON", common.as_os_str()),
        ("UPSTROKE_TEST_F_WORKER_MARKER", marker.as_os_str()),
        ("UPSTROKE_TEST_F_ROLE_LOG", log.as_os_str()),
        (
            "UPSTROKE_TEST_F_VERDICT",
            std::ffi::OsStr::new(crate::engine::topology::scaffold::PASSING_VERDICT),
        ),
    ];
    #[cfg(unix)]
    let ended =
        crate::agent::proc::test_support::run_test_isolated(&test, &vars, Duration::from_secs(300));
    #[cfg(windows)]
    let ended = f_hosted_isolated(
        &test,
        &vars,
        &env.fixture.root.join("child.stderr"),
        Duration::from_secs(300),
    );
    assert!(
        ended.status.is_some_and(|status| status.success()) && ended.stdout.contains("1 passed"),
        "{sequence}'s layout under a run over the host runner: {ended}"
    );
    let store = env.fixture.base.join(".git").join("worktrees");
    assert!(
        std::fs::symlink_metadata(&store).is_ok_and(|metadata| metadata.file_type().is_symlink()),
        "{sequence}: the run's store is still the link it started under"
    );
    assert_eq!(
        std::fs::canonicalize(&store).expect("the store resolves"),
        std::fs::canonicalize(&elsewhere).expect("the link's target"),
        "{sequence}: and the link still names the same target"
    );
    if !completes {
        assert_eq!(
            crate::workspace_manager::fixture::git(&other, &["for-each-ref"]),
            "",
            "{sequence}: the other repository's refs are untouched"
        );
        return;
    }
    let left: Vec<std::path::PathBuf> = std::fs::read_dir(&elsewhere)
        .expect("the link's target is kept")
        .map(|entry| entry.expect("an entry of the target").path())
        .collect();
    assert!(
        left.is_empty(),
        "{sequence}: finalization removed every registration the run made, below the pinned \
             target, and kept the target: {left:?}"
    );
    let base = &env.fixture.base;
    assert_eq!(
        crate::workspace_manager::fixture::git(
            base,
            &["for-each-ref", "--format=%(refname)", "refs/upstroke/runs/"]
        ),
        "",
        "{sequence}: the completed run's prepared, candidate and candidates refs were deleted \
             at finalization"
    );
    let branches = crate::workspace_manager::fixture::git(
        base,
        &[
            "for-each-ref",
            "--format=%(refname)",
            "refs/heads/upstroke/",
        ],
    );
    let mut named = branches.lines();
    let (Some(branch), None) = (named.next(), named.next()) else {
        panic!("{sequence}: the run's one integration branch: {branches:?}");
    };
    assert_eq!(
        crate::workspace_manager::fixture::git(base, &["diff", "--name-only", "main", branch]),
        "src/alpha/work.txt\nsrc/beta/work.txt",
        "{sequence}: the run's output on {branch} is both tasks' work"
    );
    for id in ["alpha", "beta"] {
        assert_eq!(
            crate::workspace_manager::fixture::git(
                base,
                &["show", &format!("{branch}:src/{id}/work.txt")]
            ),
            format!("{id} worked"),
            "{sequence}: with the bytes {id}'s worker wrote"
        );
    }
    assert_eq!(
        crate::workspace_manager::fixture::git(&other, &["for-each-ref"]),
        "",
        "{sequence}: the other repository's refs are untouched"
    );
}

#[test]
fn a_hosted_run_under_a_linked_registry_and_an_inherited_absolute_common_dir_completes() {
    f_hosted_linked_registry("C4", "hosted_linked_registry_child", true);
}

#[test]
fn a_hosted_run_under_a_linked_registry_and_an_inherited_relative_common_dir_at_equal_depth_completes()
 {
    f_hosted_linked_registry("C5", "hosted_linked_registry_child", true);
}

#[cfg(windows)]
fn f_hosted_shim(role: &str) -> String {
    let exe = std::env::current_exe().expect("this test binary");
    format!(
        "@echo off\r\n\
             set \"{F_HOSTED_ROLE}={role}\"\r\n\
             set \"{F_HOSTED_TASK}=%~2\"\r\n\
             \"{exe}\" --exact engine::topology::coordinator::tests::hosted_linked_registry_role_child --ignored --nocapture --test-threads=1\r\n\
             exit /b %ERRORLEVEL%\r\n",
        exe = exe.display()
    )
}

#[cfg(windows)]
fn f_hosted_isolated(
    test: &str,
    vars: &[(&str, &std::ffi::OsStr)],
    stderr: &std::path::Path,
    bound: Duration,
) -> FHostedEnded {
    let (child, lines) = crate::workspace_manager::fixture::LinkedChild::spawn(test, vars, stderr);
    let status = child.wait_within(bound);
    if status.is_none() {
        child.kill();
    }
    let mut stdout = String::new();
    while let Ok(line) = lines.recv_timeout(Duration::from_secs(10)) {
        stdout.push_str(&line);
        stdout.push('\n');
    }
    drop(child);
    FHostedEnded {
        status,
        stdout,
        stderr: std::fs::read_to_string(stderr).unwrap_or_default(),
    }
}

#[cfg(windows)]
struct FHostedEnded {
    status: Option<std::process::ExitStatus>,
    stdout: String,
    stderr: String,
}

#[cfg(windows)]
impl std::fmt::Display for FHostedEnded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "status {:?}\nstdout:\n{}\nstderr:\n{}",
            self.status, self.stdout, self.stderr
        )
    }
}

#[cfg(unix)]
#[test]
fn a_hosted_review_with_no_request_under_c4_is_deferred_and_the_run_completes() {
    f_hosted_linked_registry("C4", "hosted_no_request_outage_child", true);
}

#[cfg(unix)]
#[test]
fn a_hosted_review_with_no_request_under_c5_is_deferred_and_the_run_completes() {
    f_hosted_linked_registry("C5", "hosted_no_request_outage_child", true);
}

#[cfg(unix)]
#[test]
fn a_hosted_review_with_no_request_over_a_damaged_registration_is_refused() {
    f_hosted_linked_registry("C4", "hosted_no_request_damaged_child", false);
}

#[cfg(unix)]
#[test]
fn a_hosted_review_request_whose_runner_reports_nothing_is_refused() {
    f_hosted_linked_registry("C4", "hosted_no_request_opaque_child", false);
}

fn within(bound: Duration, what: &str, done: impl Fn() -> bool) {
    let started = std::time::Instant::now();
    while !done() {
        assert!(started.elapsed() < bound, "{what} within {bound:?}");
        crate::workspace_manager::fixture::rest_within(
            Duration::from_millis(20),
            bound.saturating_sub(started.elapsed()),
        );
    }
}

#[cfg(unix)]
#[test]
fn a_killed_coordinators_reapers_settle_its_pipelines_processes_under_r28_before_the_next_one_resumes_at_width_three()
 {
    use std::ffi::OsStr;
    let tasks = three();
    let logs = crate::engine::topology::scaffold::kill_dir("reapers");
    let pids = logs.path().join("the reaper's pids");
    crate::workspace_manager::fixture::write_file(&pids.join("created"), b"\n");
    let child = served(
        logs.path(),
        "hosted",
        &[
            ("UPSTROKE_TEST_CHILD_ROLE", OsStr::new("hosted")),
            ("UPSTROKE_TEST_CHILD_PIDS", pids.as_os_str()),
        ],
        crate::engine::topology::scaffold::container_host(),
    );
    let root = std::path::PathBuf::from(
        child.event("the coordinator's fixture")["root"]
            .as_str()
            .expect("a fixture root"),
    );
    let read_pids = || -> Vec<u32> {
        std::fs::read_dir(&pids)
            .expect("the pid directory")
            .filter_map(|entry| {
                let path = entry.expect("a pid entry").path();
                (path.extension().and_then(OsStr::to_str) == Some("pid"))
                    .then(|| std::fs::read_to_string(&path).ok())
                    .flatten()
                    .and_then(|text| text.trim().parse().ok())
            })
            .collect()
    };
    within(BOUND, "three pipeline processes started", || {
        read_pids().len() == 3
    });
    let started = read_pids();
    let public = crate::rundir::public_dir(
        &root.join("repo"),
        crate::workspace_manager::fixture::RUN_ID,
    );
    assert!(
        crate::rundir::is_running(&public),
        "the coordinator holds its run lock"
    );
    assert!(
        crate::rundir::observe_cleanup_hold(&public, &mut crate::rundir::NoHooks),
        "the reapers of processes spawned from pipeline threads hold the run's cleanup lease"
    );
    for pid in &started {
        assert!(crate::workspace_manager::fixture::process_exists(*pid));
    }
    let died = child.kill();
    assert!(!died.success(), "the coordinator was killed: {died:?}");
    drop(child);
    within(
        BOUND,
        "every pipeline process settled by its reaper",
        || {
            started
                .iter()
                .all(|pid| !crate::workspace_manager::fixture::process_exists(*pid))
        },
    );
    within(
        BOUND,
        "the reapers' holds released once their groups settled",
        || !crate::rundir::observe_cleanup_hold(&public, &mut crate::rundir::NoHooks),
    );
    let env =
        crate::engine::topology::scaffold::WideEnv::adopted(root, &tasks, 3, WidePlans::default());
    holds_nothing(&env.paths.public, &env.fixture.base)
        .expect("the dead coordinator's lock holds went with it");
    let (recovered, mut resumed) = env
        .resume(
            "inc-2",
            RecordingRunner::new().answering(wide_responder(&tasks, &[])),
            crate::engine::topology::select::Ceiling::unlimited(),
        )
        .expect("the next coordinator resumes");
    assert_eq!(recovered.interrupted, 3);
    broker_is_empty(&mut resumed.run).expect("its ledgers start empty");
    let progress = drive(&mut resumed, None).expect("the resumed run completes");
    assert_eq!(outcome_of(&progress), RunOutcome::Complete);
}

fn balanced_at_end(run: &mut TopologyRun) -> Result<(), String> {
    let held = run.entitlements_held();
    let balances = run.invocations_balance();
    let ledger = run.broker_mut().invocations();
    if !balances
        || held != 0
        || !ledger.running().is_empty()
        || !ledger.pending().is_empty()
        || !ledger.slots().holders().is_empty()
        || !ledger.slots().pending().is_empty()
    {
        return Err(format!(
            "balances {balances}, entitlements held {held}, running {:?}, pending {:?}, pair \
                 holders {:?}",
            ledger.running(),
            ledger.pending(),
            ledger.slots().holders()
        ));
    }
    Ok(())
}

fn next_start(wide: Wide, tasks: &[WideTask], shape: &str) -> Option<Wide> {
    let Wide { run, env } = wide;
    drop(run);
    holds_nothing(&env.paths.public, &env.fixture.base)
        .unwrap_or_else(|error| panic!("{shape}: {error}"));
    let runtime = crate::runner::container::FakeRuntime::new(
        crate::runner::container::runtime::ContainerTrace::off(),
    );
    let liveness = crate::runner::container::FakeOwnerLiveness::new();
    let mut hooks = env.hooks();
    match env.try_resume_over(
        "inc-2",
        RecordingRunner::new().answering(wide_responder(tasks, &[])),
        crate::engine::topology::select::Ceiling::unlimited(),
        &crate::engine::topology::scaffold::ResumingOver {
            runtime: &runtime,
            liveness: &liveness,
            preflight: &crate::engine::topology::scaffold::Certifying,
            awaits_release: true,
        },
        &mut hooks,
    ) {
        Ok((_, mut resumed)) => {
            broker_is_empty(&mut resumed.run)
                .unwrap_or_else(|error| panic!("{shape}: the next start: {error}"));
            Some(resumed)
        }
        Err(refused) => {
            let (error, env) = *refused;
            holds_nothing(&env.paths.public, &env.fixture.base).unwrap_or_else(|held| {
                panic!("{shape}: the refused next start ({error}) left a hold: {held}")
            });
            None
        }
    }
}

#[test]
fn the_broker_ledgers_balance_at_every_end_and_start_empty_at_every_next_start_at_width_three() {
    use crate::topology::effects::SubEffectPoint;
    let tasks = three();
    let answering = || RecordingRunner::new().answering(wide_responder(&tasks, &[]));

    let mut complete = Wide::durable(
        "coordinator-ledgers-complete",
        &tasks,
        3,
        WidePlans::default(),
        answering(),
    );
    let progress = drive(&mut complete, None).expect("the run completes");
    assert_eq!(outcome_of(&progress), RunOutcome::Complete);
    balanced_at_end(&mut complete.run).expect("Complete");
    assert!(
        next_start(complete, &tasks, "Complete").is_none(),
        "a complete run is finalized and refused at the next start"
    );

    let mut parked =
        Wide::durable(
            "coordinator-ledgers-parked",
            &tasks,
            3,
            WidePlans::default(),
            RecordingRunner::new().answering(
                crate::engine::topology::scaffold::wide_responder_asking(&tasks, &[], &[0, 1, 2]),
            ),
        );
    let progress = drive(&mut parked, None).expect("the run parks");
    assert_eq!(outcome_of(&progress), RunOutcome::Parked);
    balanced_at_end(&mut parked.run).expect("Parked");
    assert!(next_start(parked, &tasks, "Parked").is_some());

    let mut halted = durable_halting_on_gamma("coordinator-ledgers-halted");
    let runner = std::sync::Arc::clone(&halted.env.runner);
    let mut scheduler = gamma_first(&runner);
    let progress = drive(&mut halted, Some(&mut scheduler)).expect("the run halts");
    drop(scheduler);
    assert_eq!(outcome_of(&progress), RunOutcome::Halted);
    balanced_at_end(&mut halted.run).expect("Halted");
    assert!(next_start(halted, &tasks, "Halted").is_none());

    let wider = four();
    let mut budget = Wide::durable_under(
        "coordinator-ledgers-budget",
        &wider,
        3,
        WidePlans::default(),
        holding(&wider, &[]),
        TIGHT,
    );
    let runner = std::sync::Arc::clone(&budget.env.runner);
    let mut scheduler = Scheduler::scripted(
        &runner,
        Box::new(|view: &Quiescent<'_>| {
            released(view, |invocation| attempt_key(invocation) == Some(0))
        }),
    );
    let progress = drive(&mut budget, Some(&mut scheduler)).expect("the budget stop ends it");
    drop(scheduler);
    assert_eq!(outcome_of(&progress), RunOutcome::BudgetExceeded);
    balanced_at_end(&mut budget.run).expect("BudgetExceeded");
    assert!(next_start(budget, &wider, "BudgetExceeded").is_some());

    let mut failed = Wide::durable(
        "coordinator-ledgers-append-error",
        &tasks,
        3,
        WidePlans::default(),
        holding(&tasks, &[]),
    );
    let runner = std::sync::Arc::clone(&failed.env.runner);
    let harness = std::sync::Arc::clone(&failed.env.harness);
    let mut scheduler = arming_on(&runner, harness, SubEffectPoint::Synced);
    drive(&mut failed, Some(&mut scheduler)).expect_err("the append error ends the command");
    drop(scheduler);
    balanced_at_end(&mut failed.run).expect("an append error");
    assert!(next_start(failed, &tasks, "an append error").is_some());

    let mut shut = Wide::durable(
        "coordinator-ledgers-shutdown",
        &tasks,
        3,
        WidePlans::default(),
        holding(&tasks, &[]),
    );
    let runner = std::sync::Arc::clone(&shut.env.runner);
    let mut scheduler = shutdown_at_first_point(&runner);
    drive(&mut shut, Some(&mut scheduler)).expect_err("a shutdown ends the command");
    drop(scheduler);
    balanced_at_end(&mut shut.run).expect("a shutdown");
    assert!(next_start(shut, &tasks, "a shutdown").is_some());
}

#[cfg(unix)]
const REAPER_BOUND: Duration = Duration::from_secs(60);

#[cfg(unix)]
fn reaper_listing(
    program: &std::path::Path,
    private: &std::path::Path,
    incarnation: &str,
) -> Vec<String> {
    crate::runner::container::census::ReaperContainerScope::new(program, private, incarnation)
        .expect("the incarnation's container scope")
        .list_argv()
        .into_iter()
        .skip(1)
        .collect()
}

#[cfg(unix)]
fn reaper_finished(relay: &std::path::Path, reclaimed: usize) -> Vec<Vec<String>> {
    use crate::runner::container::FakeRuntime;
    let done = |calls: &[Vec<String>]| {
        calls.len() >= 2 + 2 * reclaimed
            && calls
                .last()
                .and_then(|call| call.first())
                .map(String::as_str)
                == Some("ps")
    };
    let started = std::time::Instant::now();
    while !done(&FakeRuntime::reaper_calls(relay)) && started.elapsed() < REAPER_BOUND {
        crate::workspace_manager::fixture::rest_within(
            Duration::from_millis(20),
            REAPER_BOUND.saturating_sub(started.elapsed()),
        );
    }
    FakeRuntime::reaper_calls(relay)
}

#[cfg(unix)]
struct Reclaimed<'a> {
    host: &'a crate::runner::container::FakeRuntime,
    relay: &'a std::path::Path,
    program: &'a std::path::Path,
    private: &'a std::path::Path,
    incarnation: &'a str,
    expected: &'a [String],
    censused_before_the_death: usize,
}

#[cfg(unix)]
fn reclaimed_by_its_reaper(reclaimed: &Reclaimed<'_>) {
    use crate::runner::container::runtime::RuntimeOp;
    let calls = reaper_finished(reclaimed.relay, reclaimed.expected.len());
    let listing = reaper_listing(reclaimed.program, reclaimed.private, reclaimed.incarnation);
    let of = |verb: &str| -> Vec<String> {
        sorted(
            calls
                .iter()
                .filter(|call| call.first().map(String::as_str) == Some(verb))
                .filter_map(|call| call.last().cloned())
                .collect(),
        )
    };
    assert_eq!(
        (
            calls.first(),
            calls.last(),
            of("kill"),
            of("rm"),
            calls.len()
        ),
        (
            Some(&listing),
            Some(&listing),
            reclaimed.expected.to_vec(),
            reclaimed.expected.to_vec(),
            2 + 2 * reclaimed.expected.len()
        ),
        "one reaper listed the containers labeled with the dead coordinator's private root and \
             incarnation, killed and removed each, and listed again to find none left: {calls:?}"
    );
    let delivered = reclaimed.host.deliver_reaper_calls(reclaimed.relay);
    let journal = reclaimed.host.journal();
    assert_eq!(
        (
            sorted(by(&journal, "reaper", RuntimeOp::Stop)),
            sorted(by(&journal, "reaper", RuntimeOp::Remove))
        ),
        (reclaimed.expected.to_vec(), reclaimed.expected.to_vec()),
        "each was stopped and removed at the daemon by the reaper's own calls: {delivered:?}"
    );
    assert!(
        reclaimed
            .expected
            .iter()
            .all(|name| reclaimed.host.container(name).is_none()),
        "none of the dead coordinator's containers is left: {:?}",
        reclaimed.host.container_names()
    );
    assert_eq!(
        censused(reclaimed.host),
        reclaimed.censused_before_the_death,
        "no census listed the runtime after the coordinator died: its reaper reclaimed the \
             containers before any census ran"
    );
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug)]
enum ProbeSchedule {
    Free,
    #[cfg(target_os = "linux")]
    HeldAtItsListing,
}

#[cfg(target_os = "linux")]
#[derive(Clone, Copy, PartialEq, Eq)]
enum GateState {
    Unarmed,
    Armed,
    LetGo,
}

#[cfg(unix)]
struct KeptForItsBackstop {
    env: Option<crate::engine::topology::scaffold::WideEnv>,
    logs: Option<crate::rundir::scratch_tree::ScratchTree>,
    schedule: ProbeSchedule,
    #[cfg(target_os = "linux")]
    gate: std::path::PathBuf,
    #[cfg(target_os = "linux")]
    gate_state: std::cell::Cell<GateState>,
}

#[cfg(unix)]
impl KeptForItsBackstop {
    fn new(
        env: crate::engine::topology::scaffold::WideEnv,
        logs: crate::rundir::scratch_tree::ScratchTree,
        schedule: ProbeSchedule,
    ) -> Self {
        #[cfg(target_os = "linux")]
        let gate = logs.path().join("backstop-gate");
        Self {
            env: Some(env),
            logs: Some(logs),
            schedule,
            #[cfg(target_os = "linux")]
            gate,
            #[cfg(target_os = "linux")]
            gate_state: std::cell::Cell::new(GateState::Unarmed),
        }
    }

    fn env(&self) -> &crate::engine::topology::scaffold::WideEnv {
        self.env
            .as_ref()
            .expect("the real fixture is kept until its owner drops")
    }

    fn logs(&self) -> &std::path::Path {
        self.logs
            .as_ref()
            .expect("the kill directory is kept until its owner drops")
            .path()
    }

    fn child_environment(&self) -> Vec<(&'static str, &std::ffi::OsStr)> {
        let named = [
            ("UPSTROKE_TEST_CHILD_ROLE", std::ffi::OsStr::new("resume")),
            (
                "UPSTROKE_TEST_CHILD_ROOT",
                self.env().fixture.root.as_os_str(),
            ),
            (
                "UPSTROKE_TEST_CHILD_INCARNATION",
                std::ffi::OsStr::new(INC_2),
            ),
        ];
        #[cfg(target_os = "linux")]
        let gate = matches!(self.schedule, ProbeSchedule::HeldAtItsListing)
            .then_some((crate::agent::proc::BACKSTOP_GATE, self.gate.as_os_str()));
        #[cfg(not(target_os = "linux"))]
        let gate = None;
        named.into_iter().chain(gate).collect()
    }

    fn arm(&self) {
        #[cfg(target_os = "linux")]
        if matches!(self.schedule, ProbeSchedule::HeldAtItsListing) {
            self.gate_state.set(GateState::Armed);
            for point in ["decided", "decided.arrived", "listed.arrived"] {
                crate::workspace_manager::fixture::write_file(&self.gate_point(point), b"");
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn gate_point(&self, point: &str) -> std::path::PathBuf {
        let mut name = self.gate.clone().into_os_string();
        name.push(".");
        name.push(point);
        std::path::PathBuf::from(name)
    }

    fn let_the_backstop_go(&self) -> Result<(), String> {
        #[cfg(target_os = "linux")]
        if self.gate_state.get() == GateState::Armed {
            let release = self.gate_point("listed");
            let shown = release.display().to_string();
            std::thread::Builder::new()
                .name("backstop-gate-release".to_owned())
                .spawn(move || crate::workspace_manager::fixture::write_file(&release, b""))
                .map_err(|error| {
                    format!("no thread could release the backstop's gate at {shown}: {error}")
                })?
                .join()
                .map_err(|_| format!("the backstop's gate at {shown} could not be released"))?;
            self.gate_state.set(GateState::LetGo);
            let passed = self.gate_point("decided.arrived");
            let started = std::time::Instant::now();
            while !the_backstop_reached(&passed) {
                if started.elapsed() >= REAPER_BOUND {
                    return Err(format!(
                        "the backstop let go at its listing was not past its gate ({}) \
                             within {REAPER_BOUND:?}",
                        passed.display()
                    ));
                }
                crate::workspace_manager::fixture::rest_within(
                    Duration::from_millis(20),
                    REAPER_BOUND.saturating_sub(started.elapsed()),
                );
            }
        }
        Ok(())
    }
}

#[cfg(unix)]
impl Drop for KeptForItsBackstop {
    fn drop(&mut self) {
        let released = self.let_the_backstop_go();
        let unheld = match &self.env {
            Some(env) => no_cleanup_hold_is_left(&env.paths.public),
            None => Ok(()),
        };
        drop(self.env.take());
        drop(self.logs.take());
        for why in [released, unheld].into_iter().filter_map(Result::err) {
            if std::thread::panicking() {
                crate::workspace_manager::fixture::say(&why);
            } else {
                panic!("{why}");
            }
        }
    }
}

#[cfg(unix)]
fn no_cleanup_hold_is_left(public: &std::path::Path) -> Result<(), String> {
    let started = std::time::Instant::now();
    while crate::rundir::observe_cleanup_hold(public, &mut crate::rundir::NoHooks) {
        if started.elapsed() >= REAPER_BOUND {
            return Err(format!(
                "the cleanup lease of {} was still held after {REAPER_BOUND:?}, so the real \
                     fixture was let go under a holder",
                public.display()
            ));
        }
        crate::workspace_manager::fixture::rest_within(
            Duration::from_millis(20),
            REAPER_BOUND.saturating_sub(started.elapsed()),
        );
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn the_backstop_reached(arrival: &std::path::Path) -> bool {
    std::fs::metadata(arrival).is_ok_and(|metadata| metadata.len() > 0)
}

#[cfg(unix)]
struct DiedInItsProbe {
    kept: KeptForItsBackstop,
    host: crate::runner::container::FakeRuntime,
    relay: std::path::PathBuf,
    program: std::path::PathBuf,
    private: std::path::PathBuf,
    probe: String,
    censused_before_the_death: usize,
}

#[cfg(unix)]
impl DiedInItsProbe {
    fn its_lawful_paths_are_present(&self) {
        let env = self.kept.env();
        let lawful = [
            crate::rundir::worktree_lock_file(&env.fixture.base.join(".git")),
            crate::rundir::lock_file(&env.paths.public),
            env.paths.public.join("cleanup.lock"),
        ];
        let missing: Vec<&std::path::PathBuf> =
            lawful.iter().filter(|path| !path.is_file()).collect();
        assert!(
            missing.is_empty(),
            "after its helper returned ({:?}), the dead coordinator's real fixture still has \
                 the worktree lock, the run lock and the cleanup lease its backstop opens after \
                 its listing; missing: {missing:?}",
            self.kept.schedule
        );
    }

    #[cfg(target_os = "linux")]
    fn held_at_its_listing(&self) {
        let arrived = self.kept.gate_point("listed.arrived");
        within(
            REAPER_BOUND,
            "the dead coordinator's backstop held at its listing after its helper returned",
            || the_backstop_reached(&arrived),
        );
        assert_eq!(
            crate::runner::container::FakeRuntime::reaper_calls(&self.relay),
            vec![reaper_listing(&self.program, &self.private, INC_2)],
            "held at its listing, the backstop has listed the probe once and has killed and \
                 removed nothing"
        );
    }

    #[cfg(target_os = "linux")]
    fn let_its_backstop_go(&self) {
        if let Err(why) = self.kept.let_the_backstop_go() {
            panic!("{why}");
        }
    }
}

#[cfg(unix)]
fn a_resume_killed_inside_its_pre_flight_probe(
    tag: &str,
    schedule: ProbeSchedule,
    beside: impl FnOnce(&crate::runner::container::FakeRuntime, &std::path::Path),
) -> DiedInItsProbe {
    let tasks = three();
    let Wide { run, env } = Wide::durable_contained(
        tag,
        &tasks,
        3,
        WidePlans::default(),
        RecordingRunner::new(),
        INC_1,
    );
    drop(run);
    holds_nothing(&env.paths.public, &env.fixture.base).expect("no earlier holds");
    let host = crate::engine::topology::scaffold::container_host();
    let logs = crate::engine::topology::scaffold::kill_dir(tag);
    let relay = logs.path().join("relay");
    let program = host.install_reaper_relay(&relay);
    let kept = KeptForItsBackstop::new(env, logs, schedule);
    let child = served(
        kept.logs(),
        "resume",
        &kept.child_environment(),
        host.acting_as(INC_2),
    );
    await_starts(&host, INC_2, 1);
    within(
        BOUND,
        "the resuming incarnation's pre-flight probe running",
        || running_in(&host).len() == 1,
    );
    let running = running_in(&host);
    let probe = running[0].clone();
    assert!(
        crate::runner::container::FakeRuntime::reaper_calls(&relay).is_empty(),
        "the reaper acts on nothing while its coordinator lives"
    );
    beside(&host, &kept.env().fixture.private);
    host.publish_for_reaper(&relay);
    let censused_before_the_death = censused(&host);
    kept.arm();
    let died = child.kill();
    assert!(
        !died.success(),
        "the resuming coordinator was killed: {died:?}"
    );
    drop(child);
    let private = kept.env().fixture.private.clone();
    DiedInItsProbe {
        kept,
        host,
        relay,
        program,
        private,
        probe,
        censused_before_the_death,
    }
}

#[cfg(unix)]
#[test]
fn a_resuming_incarnations_pre_flight_probe_container_is_killed_by_its_reaper_when_the_coordinator_dies_inside_it()
 {
    let died = a_resume_killed_inside_its_pre_flight_probe(
        "reaper-pre-flight",
        ProbeSchedule::Free,
        |_, _| {},
    );
    died.its_lawful_paths_are_present();
    reclaimed_by_its_reaper(&Reclaimed {
        host: &died.host,
        relay: &died.relay,
        program: &died.program,
        private: &died.private,
        incarnation: INC_2,
        expected: std::slice::from_ref(&died.probe),
        censused_before_the_death: died.censused_before_the_death,
    });
}

#[cfg(unix)]
#[test]
fn the_reapers_scope_is_the_runners_identity_and_selects_its_first_probe() {
    use crate::runner::container::FakeRuntime;
    use crate::runner::container::intent::{LABEL_INCARNATION, LABEL_PRIVATE_ROOT};
    let foreign = "upstroke-foreign-incarnation-beside-the-probe".to_owned();
    #[cfg(target_os = "linux")]
    let schedule = ProbeSchedule::HeldAtItsListing;
    #[cfg(not(target_os = "linux"))]
    let schedule = ProbeSchedule::Free;
    let died = a_resume_killed_inside_its_pre_flight_probe(
        "reaper-pre-flight-scope",
        schedule,
        |host, private| {
            host.seed_container(
                &foreign,
                [
                    (
                        LABEL_PRIVATE_ROOT.to_owned(),
                        crate::runner::container::intent::private_root_label(private),
                    ),
                    (LABEL_INCARNATION.to_owned(), INC_FOREIGN.to_owned()),
                ]
                .into_iter()
                .collect(),
                crate::engine::topology::scaffold::CONTAINER_IMAGE_ID,
                crate::engine::topology::scaffold::CONTAINER_IMAGE_ID,
                crate::runner::container::runtime::Liveness::Running,
            );
        },
    );
    #[cfg(target_os = "linux")]
    died.held_at_its_listing();
    died.its_lawful_paths_are_present();
    #[cfg(target_os = "linux")]
    died.let_its_backstop_go();
    let labels = died
        .host
        .container(&died.probe)
        .expect("the probe, as the daemon recorded it")
        .labels;
    let calls = reaper_finished(&died.relay, 1);
    let first = calls.first().cloned().unwrap_or_default();
    for key in [LABEL_PRIVATE_ROOT, LABEL_INCARNATION] {
        let filter = format!(
            "label={key}={}",
            labels.get(key).cloned().unwrap_or_default()
        );
        assert!(
            first.contains(&filter),
            "the reaper's first listing selects the probe's own `{key}` label as the daemon \
                 recorded it, `{filter}`: {calls:?}"
        );
    }
    let delivered = died.host.deliver_reaper_calls(&died.relay);
    assert!(
        died.host.container(&died.probe).is_none(),
        "the probe was reclaimed: {delivered:?}"
    );
    assert!(
        died.host.container(&foreign).is_some_and(
            |container| container.state == crate::runner::container::runtime::Liveness::Running
        ) && !FakeRuntime::reaper_calls(&died.relay)
            .iter()
            .any(|call| call.last() == Some(&foreign)),
        "another incarnation's container under the same private root is not the dead \
             coordinator's, and its reaper's scope did not select it: {calls:?}"
    );
}

#[cfg(unix)]
#[test]
fn a_runner_whose_containers_are_unresolved_keeps_its_reaper_armed_past_its_last_handle_until_the_process_exits()
 {
    use crate::runner::container::FakeRuntime;
    use crate::runner::container::runtime::RuntimeOp;
    use std::ffi::OsStr;
    let host = crate::engine::topology::scaffold::container_host();
    let logs = crate::engine::topology::scaffold::kill_dir("reaper-stranded");
    let relay = logs.path().join("relay");
    let program = host.install_reaper_relay(&relay);
    let word = logs.path().join("exit");
    let child = served(
        logs.path(),
        "stranded",
        &[
            ("UPSTROKE_TEST_CHILD_ROLE", OsStr::new("stranded")),
            ("UPSTROKE_TEST_CHILD_INCARNATION", OsStr::new(INC_1)),
            ("UPSTROKE_TEST_CHILD_WORD", word.as_os_str()),
        ],
        host.acting_as(INC_1),
    );
    let fixture = child.event("the coordinator's fixture");
    let private = std::path::PathBuf::from(fixture["private"].as_str().expect("its private root"));
    await_starts(&host, INC_1, 3);
    within(BOUND, "the coordinator's three containers running", || {
        running_in(&host).len() == 3
    });
    let running = sorted(running_in(&host));
    let unreachable = [RuntimeOp::Observe, RuntimeOp::Stop, RuntimeOp::Remove];
    for op in unreachable {
        host.set_unreachable(op);
    }
    let returned = child.event("the coordinator returned its error");
    assert_eq!(
        returned["balanced"].as_bool(),
        Some(false),
        "the Runners could not establish their containers gone, so the coordinator returned \
             with their registrations held: {returned}"
    );
    let dropped = child.event("the coordinator dropped every handle on its runner");
    assert_eq!(dropped["dropped"].as_bool(), Some(true));
    assert_eq!(
        sorted(running_in(&host)),
        running,
        "the three containers outlived the error return and the last handle"
    );
    assert!(
        FakeRuntime::reaper_calls(&relay).is_empty(),
        "the reaper acts on nothing while its coordinator lives, past its last handle: {:?}",
        FakeRuntime::reaper_calls(&relay)
    );
    for op in unreachable {
        host.set_reachable(op);
    }
    host.publish_for_reaper(&relay);
    let censused_before_the_death = censused(&host);
    crate::workspace_manager::fixture::write_file(&word, b"exit\n");
    let exited = child.exited("the coordinator's process exits on its own");
    assert!(exited.success(), "{exited:?}: {}", child.stderr());
    drop(child);
    reclaimed_by_its_reaper(&Reclaimed {
        host: &host,
        relay: &relay,
        program: &program,
        private: &private,
        incarnation: INC_1,
        expected: &running,
        censused_before_the_death,
    });
}

#[cfg(unix)]
fn a_contained_run_with_its_reaper_relayed(
    tag: &str,
    unreachable: &[crate::runner::container::runtime::RuntimeOp],
) -> (
    Result<Progress, UpstrokeError>,
    crate::runner::container::FakeRuntime,
    crate::rundir::scratch_tree::ScratchTree,
    std::path::PathBuf,
    bool,
) {
    use crate::runner::container::FakeRuntime;
    let tasks = three();
    let mut wide = Wide::durable_contained(
        tag,
        &tasks,
        3,
        WidePlans::default(),
        RecordingRunner::new().answering(wide_responder(&tasks, &[])),
        INC_A,
    );
    let host = crate::engine::topology::scaffold::container_host();
    let logs = crate::engine::topology::scaffold::kill_dir(tag);
    let relay = logs.path().join("relay");
    host.install_reaper_relay(&relay);
    let checked =
        FakeRuntime::run_reaper_relay(&relay, &["ps", "--filter", "label=upstroke.relay=bound"]);
    assert_eq!(
        (checked, FakeRuntime::reaper_calls(&relay)),
        (
            Some(0),
            vec![vec![
                "ps".to_owned(),
                "--filter".to_owned(),
                "label=upstroke.relay=bound".to_owned()
            ]]
        ),
        "the relay is bound: its program, run by its path in a bounded child of this test \
             binary, records its call where this test reads"
    );
    for op in unreachable {
        host.set_unreachable(*op);
    }
    let contained = wide.env.contained(&host, INC_A);
    let mut hooks = wide.env.hooks();
    let pipelines = wide.env.pipelines_over(contained.clone());
    let ended = wide.run.run_concurrently(
        &wide.env.seams_over(&*contained),
        &pipelines,
        &mut hooks,
        None,
    );
    drop(hooks);
    drop(pipelines);
    drop(contained);
    let balanced = wide.run.invocations_balance();
    (ended, host, logs, relay, balanced)
}

#[cfg(unix)]
fn released_by_their_own_runner_and_never_reaped(
    host: &crate::runner::container::FakeRuntime,
    relay: &std::path::Path,
) {
    use crate::runner::container::FakeRuntime;
    use crate::runner::container::runtime::RuntimeOp;
    let calls = FakeRuntime::reaper_calls(relay);
    assert_eq!(
        calls.len(),
        1,
        "the reaper was cancelled when its runner was dropped and listed, killed and removed \
             nothing; the one call is the relay's own self-check: {calls:?}"
    );
    let journal = host.journal();
    assert!(
        by(&journal, "reaper", RuntimeOp::Stop).is_empty()
            && by(&journal, "reaper", RuntimeOp::Remove).is_empty()
            && by(&journal, INC_A, RuntimeOp::Remove).len() >= 3
            && host.container_names().is_empty(),
        "every container was released by its own runner, none by a reaper: {:?}",
        host.container_names()
    );
}

#[cfg(unix)]
#[test]
fn a_coordinator_that_ends_disarms_its_reaper_and_kills_nothing_at_width_three() {
    let (ended, host, _logs, relay, balanced) =
        a_contained_run_with_its_reaper_relayed("reaper-disarmed", &[]);
    let progress = ended.expect("the contained run completes");
    assert_eq!(outcome_of(&progress), RunOutcome::Complete);
    assert!(balanced);
    released_by_their_own_runner_and_never_reaped(&host, &relay);
}

#[cfg(unix)]
#[test]
fn a_runner_whose_containers_all_ended_disarms_its_reaper_and_the_relay_is_never_called() {
    use crate::runner::container::runtime::RuntimeOp;
    let (ended, host, _logs, relay, balanced) =
        a_contained_run_with_its_reaper_relayed("reaper-ended-error", &[RuntimeOp::Observe]);
    assert!(
        ended.is_err(),
        "the lost observation fails the pipelines: {ended:?}"
    );
    assert!(
        balanced,
        "every Runner established its container gone, so the ledgers balance"
    );
    released_by_their_own_runner_and_never_reaped(&host, &relay);
}

#[cfg(unix)]
#[test]
fn every_container_an_incarnation_starts_is_covered_by_an_armed_reaper_with_its_scope() {
    use crate::runner::container::runtime::RuntimeOp;
    let tasks = three();
    let host = crate::engine::topology::scaffold::container_host();
    host.observing_covers();

    let Wide { run, env } = Wide::durable_contained(
        "reaper-covers-resume",
        &tasks,
        3,
        WidePlans::default(),
        RecordingRunner::new(),
        INC_1,
    );
    drop(run);
    let base = env.fixture.base.clone();
    let adapters = std::sync::Arc::clone(&env.adapters);
    let probes = crate::engine::topology::scaffold::container_runner(
        env.identity(INC_2),
        &base,
        Box::new(
            host.acting_as("probes")
                .starting(crate::engine::topology::scaffold::exiting()),
        ),
        Duration::from_millis(2),
    );
    let preflight = crate::engine::topology::preflight::RunPreflight::new(
        &probes,
        &*adapters,
        crate::gates::ShellKind::Sh,
        &base,
        Vec::new(),
    );
    let census = host.acting_as("census");
    let mut hooks = env.hooks();
    let (_, mut wide) = env
        .resume_over(
            INC_2,
            RecordingRunner::new().answering(wide_responder(&tasks, &[])),
            crate::engine::topology::select::Ceiling::unlimited(),
            &crate::engine::topology::scaffold::ResumingOver {
                runtime: &census,
                liveness: &crate::runner::container::runtime::LockProbe,
                preflight: &preflight,
                awaits_release: true,
            },
            &mut hooks,
        )
        .expect("the resuming incarnation's pre-flight certifies over its probe containers");
    drop(hooks);
    drop(preflight);
    drop(probes);
    let after_the_pre_flight = host.starts_observed().len();
    assert!(
        after_the_pre_flight >= 1,
        "the pre-flight started its shell probe in a container: {:?}",
        host.starts_observed()
    );

    let contained = wide.env.contained(&host, INC_2);
    let mut hooks = wide.env.hooks();
    let pipelines = wide.env.pipelines_over(contained.clone());
    let progress = wide
        .run
        .run_concurrently(
            &wide.env.seams_over(&*contained),
            &pipelines,
            &mut hooks,
            None,
        )
        .expect("the resumed contained run completes at width three");
    drop(hooks);
    drop(pipelines);
    drop(contained);
    assert_eq!(outcome_of(&progress), RunOutcome::Complete);
    let after_the_coordinator = host.starts_observed().len();
    assert!(after_the_coordinator > after_the_pre_flight + 3);

    let chain = [
        WideTask::independent("alpha"),
        WideTask::after("beta", &["alpha"]),
    ];
    let mut narrow = Wide::durable_contained(
        "reaper-covers-step",
        &chain,
        1,
        WidePlans::default(),
        RecordingRunner::new().answering(wide_responder(&chain, &[])),
        INC_3,
    );
    let stepped = narrow.env.contained(&host, INC_3);
    let seams = narrow.env.seams_over(&*stepped);
    let mut hooks = narrow.env.hooks();
    let mut steps = 0_u32;
    loop {
        steps += 1;
        assert!(steps < 200, "the width-1 loop did not finish");
        match narrow.run.step(&seams, &mut hooks).expect("a step") {
            Progress::Finished { outcome, .. } => {
                assert_eq!(outcome, RunOutcome::Complete);
                break;
            }
            _ => continue,
        }
    }
    drop(hooks);
    drop(stepped);

    let observed = host.starts_observed();
    let uncovered: Vec<&(String, bool)> = observed.iter().filter(|(_, covered)| !covered).collect();
    assert!(
        uncovered.is_empty(),
        "every container start was covered by an armed reaper whose scope selects its labels: \
             uncovered {uncovered:?}"
    );
    let created = host
        .journal()
        .iter()
        .filter(|entry| entry.op == RuntimeOp::Create)
        .count();
    assert_eq!(
        observed.len(),
        created,
        "every container created was started under observation: {observed:?}"
    );
    assert!(
        observed.len() > after_the_coordinator,
        "the width-1 `step` started containers too: {observed:?}"
    );
}
