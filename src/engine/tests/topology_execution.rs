//! Extended notes: `docs/internals/engine/tests/topology_execution.md`
// Allowlist placement: the funnel section of `effects/allowlist.toml`.
#![allow(clippy::disallowed_methods)]
#![forbid(clippy::disallowed_types, clippy::disallowed_macros)]

use super::*;

/// The stub's edit is a real append under the shell `shell_spec` picks on this
/// platform, run by the production `HostRunner`: each run adds one line, so a
/// task whose base already holds an earlier task's output still leaves a diff
/// of its own. A second task's empty diff on the Windows guest (CI run
/// 41529099549) is what an edit that left the file as it was looks like. Only
/// a Windows host runs the edit under `cmd`, so only there does this
/// discriminate between the spellings.
#[test]
fn the_topology_stubs_edit_appends_one_line_per_run_under_the_native_shell() {
    let tree = temp_engine_scratch("topology-stub-edit");
    let workspace = tree.path().to_path_buf();
    let runner = crate::runner::host::HostRunner::new();
    for runs in 1..=2 {
        let request = crate::runner::gate_request(
            shell_spec(TOPOLOGY_STUB_EDIT),
            workspace.clone(),
            Duration::from_secs(60),
            crate::runner::InvocationId::attempt(
                crate::topology::registry::TaskKey(0),
                crate::topology::events::GenerationId(0),
                crate::topology::events::AttemptNumber(1),
                crate::runner::invocation::AttemptRole::Gate(0),
                0,
            ),
        );
        let output = crate::runner::Runner::run_blocking(&runner, &request)
            .expect("the native shell runs the stub's edit");
        assert_eq!(output.code, Some(0), "run {runs}: {output:?}");
        let text = fs::read_to_string(workspace.join("agent-output.txt")).expect("the edit's file");
        assert_eq!(
            text.lines().collect::<Vec<_>>(),
            vec!["edited by the topology stub"; runs],
            "run {runs} appends exactly one line: {text:?}"
        );
    }
}

/// The stub reads a worker's end as a real adapter does: only a clean exit
/// completes, a timeout is a timeout, and any other end is an agent error, so
/// a failed edit is reported with its own exit and stderr, not as an empty
/// diff. This holds and discriminates on every platform.
#[test]
fn the_topology_stub_completes_only_a_worker_that_exited_cleanly() {
    let stub = TopologyStub::new(TOPOLOGY_STUB_EDIT, 0);
    let status = |code: Option<i32>, timed_out: bool| {
        stub.parse(&ProcessOutput {
            code,
            stdout: String::new(),
            stderr: "the edit did not run".to_owned(),
            duration: Duration::from_millis(1),
            timed_out,
            output_limited: false,
        })
        .expect("the stub reads every end")
        .status
    };
    assert_eq!(status(Some(0), false), OutcomeStatus::Completed);
    assert_eq!(status(Some(1), false), OutcomeStatus::AgentError);
    assert_eq!(status(None, false), OutcomeStatus::AgentError);
    assert_eq!(status(None, true), OutcomeStatus::Timeout);
}

#[test]
fn a_schema_4_run_is_created_driven_and_merged_through_the_public_entry_point() {
    let (_tree, repo) = temp_engine_repo("topology-fresh");
    let source = SharedTopologyStubs(std::sync::Arc::new(TopologyStub::new(
        TOPOLOGY_STUB_EDIT,
        0,
    )));
    let report = run_with(&topology_options(&repo), &source).expect("the schema-4 run completes");

    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:#?}");
    assert!(
        committed(&report, "t1") && committed(&report, "t2"),
        "{report:#?}"
    );
    let first = first_line_of_the_log(&repo, &report.run_id);
    assert_eq!(first["event"], "run_started");
    assert_eq!(first["data"]["schema"], 4, "`upstroke run` wrote schema 4");
    assert_eq!(first["data"]["runner"]["kind"], "host");
    assert_eq!(report.branch, format!("upstroke/run-{}", report.run_id));
    assert_merged_on_the_integration_branch(&repo, &report);

    until_released(&repo, &report.run_id);
    let mut resume = resume_options(&repo, &report.run_id);
    resume.private_root = None;
    let refused =
        resume_with(&resume, &source).expect_err("a finished run has nothing to continue");
    assert!(
        refused.to_string().contains("already finished"),
        "the topology recovery order refused continuation: {refused}"
    );
}

#[test]
fn a_schema_4_run_on_an_unshareable_source_is_driven_by_the_step_loop() {
    let (_tree, repo) = temp_engine_repo("topology-step");
    let stub = TopologyStub::new(TOPOLOGY_STUB_EDIT, 0);
    let report = run_with(&topology_options(&repo), &BorrowedTopologyStub(&stub))
        .expect("the schema-4 run completes at width 1");

    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:#?}");
    assert!(
        committed(&report, "t1") && committed(&report, "t2"),
        "{report:#?}"
    );
    assert_eq!(
        first_line_of_the_log(&repo, &report.run_id)["data"]["schema"],
        4
    );
    assert_merged_on_the_integration_branch(&repo, &report);
}

#[test]
fn a_schema_4_run_wider_than_one_refuses_an_unshareable_source_before_any_lock() {
    let (_tree, repo) = temp_engine_repo("topology-unshareable");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 depends= -->\nMake it.\n",
        Some("[engine]\nmax_parallel = 2\n"),
    );
    let stub = TopologyStub::new(TOPOLOGY_STUB_EDIT, 0);
    let refused = run_with(&topology_options(&repo), &BorrowedTopologyStub(&stub))
        .expect_err("width 2 needs a shareable adapter source");
    assert!(
        refused.to_string().contains("cannot be shared"),
        "{refused}"
    );
    assert!(
        !rundir::runs_root(&repo).exists()
            || fs::read_dir(rundir::runs_root(&repo))
                .expect("the runs root")
                .next()
                .is_none(),
        "the refused run created a run directory"
    );
}

/// A worker that notes, for about five seconds, whether another stub worker is
/// running beside it, then writes a file of its own and leaves. `overlap.*` in
/// `markers` is the evidence two workers ran at once. Each attempt writes a
/// different file, so two candidates never conflict at the merge.
///
/// Five seconds, not one: in V139's full suite, beside a guest VM, two
/// dispatches at `max_parallel = 2` started more than a second apart, and the
/// premise that two workers ran at once failed, while the test alone passed
/// (V140). A longer look makes the premise robust to load, and it gives the
/// bounded arms (`max_per_agent = 1`, a one-slot pool) longer in which a
/// forbidden overlap would be seen, so their zero-overlap assertions are only
/// stronger for it.
#[cfg(unix)]
fn overlap_observing_worker(markers: &Path) -> String {
    format!(
        "d='{}'; me=$$; : > \"$d/run.$me\"; i=0; while [ $i -lt 50 ]; do \
         n=$(ls \"$d\" | grep -c '^run\\.'); if [ \"$n\" -gt 1 ]; then : > \"$d/overlap.$me\"; fi; \
         i=$((i+1)); sleep 0.1; done; printf 'edited by the topology stub\\n' > \"out.$me.txt\"; \
         rm -f \"$d/run.$me\"",
        markers.display()
    )
}

#[cfg(unix)]
fn overlaps_in(markers: &Path) -> usize {
    fs::read_dir(markers)
        .expect("the marker directory")
        .filter(|entry| {
            entry
                .as_ref()
                .is_ok_and(|entry| entry.file_name().to_string_lossy().starts_with("overlap."))
        })
        .count()
}

/// A fresh run at `max_parallel = 2` over two independent tasks, under `[engine]`
/// `extra`: whether two workers overlapped.
#[cfg(unix)]
fn fresh_overlap(tag: &str, extra: &str, pools: Option<&str>) -> usize {
    let (tree, repo) = temp_engine_repo(tag);
    seed(
        &repo,
        TWO_INDEPENDENT_TASKS,
        Some(&format!("[engine]\nmax_parallel = 2\n{extra}")),
    );
    let markers = tree.path().join("markers");
    fs::create_dir(&markers).expect("the marker directory");
    let mut opts = topology_options(&repo);
    if let Some(pools) = pools {
        let path = tree.path().join("pools.toml");
        fs::write(&path, pools).expect("the pools file");
        opts.pools_path = Some(path);
    }
    let source = SharedTopologyStubs(std::sync::Arc::new(TopologyStub::new(
        &overlap_observing_worker(&markers),
        0,
    )));
    let report = run_with(&opts, &source).expect("the run completes");
    assert_eq!(report.outcome(), RunOutcome::Complete, "{tag}: {report:#?}");
    assert!(
        committed(&report, "t1") && committed(&report, "t2"),
        "{tag}: {report:#?}"
    );
    overlaps_in(&markers)
}

#[cfg(unix)]
#[test]
fn a_schema_4_run_runs_two_workers_at_once_and_the_configured_slot_limits_bound_them() {
    assert!(
        fresh_overlap("topology-overlap", "", None) > 0,
        "two independent tasks at `max_parallel = 2` never ran at once, so the bounds below \
         would prove nothing"
    );
    assert_eq!(
        fresh_overlap("topology-agent-bound", "max_per_agent = 1\n", None),
        0,
        "`max_per_agent = 1` let two invocations of one agent run at once"
    );
    assert_eq!(
        fresh_overlap(
            "topology-pool-bound",
            "max_per_pool = 1\n",
            Some("[pools.claude-max]\nkind = \"subscription-window\"\nagent = \"claude-code\"\n"),
        ),
        0,
        "`max_per_pool = 1` let two invocations in one pool run at once"
    );
}

/// A schema-4 run whose two tasks both park on a question, resumed with both
/// answers delivered under today's `[engine]` `extra`: whether the resumed
/// workers overlapped. The answers are written where the run's own answer
/// source polls for them before every selection, so both tasks are ready
/// together when the resumed loop starts.
#[cfg(unix)]
fn resumed_overlap(tag: &str, extra: &str) -> usize {
    let (tree, repo) = temp_engine_repo(tag);
    seed(
        &repo,
        TWO_INDEPENDENT_TASKS,
        Some("[engine]\nmax_parallel = 2\n"),
    );
    let markers = tree.path().join("markers");
    fs::create_dir(&markers).expect("the marker directory");
    let source = SharedTopologyStubs(std::sync::Arc::new(TopologyStub::new(
        &overlap_observing_worker(&markers),
        2,
    )));
    let parked = run_with(&topology_options(&repo), &source).expect("the run parks");
    assert_eq!(parked.outcome(), RunOutcome::Parked, "{tag}: {parked:#?}");
    for entry in fs::read_dir(&markers).expect("the marker directory") {
        fs::remove_file(entry.expect("a marker").path()).expect("a clean marker directory");
    }

    assert_eq!(
        parked.questions.len(),
        2,
        "{tag}: both tasks asked: {parked:#?}"
    );
    let answers = rundir::public_dir(&repo, &parked.run_id).join("answers");
    fs::create_dir_all(&answers).expect("the run's answers directory");
    for asked in &parked.questions {
        interaction::write_answer(
            &answers,
            &asked.question.id,
            &interaction::AnswerRecord::unattributed(Answer::Answered {
                text: "the first".to_owned(),
            }),
        )
        .expect("the answer is delivered");
    }

    let today = tree.path().join("today.toml");
    fs::write(&today, format!("[engine]\nmax_parallel = 2\n{extra}")).expect("today's config");
    until_released(&repo, &parked.run_id);
    let mut resume = resume_options(&repo, &parked.run_id);
    resume.private_root = None;
    resume.config_path = Some(today);
    let report = resume_with(&resume, &source).expect("the resume completes the run");
    assert_eq!(report.outcome(), RunOutcome::Complete, "{tag}: {report:#?}");
    assert!(
        committed(&report, "t1") && committed(&report, "t2"),
        "{tag}: {report:#?}"
    );
    overlaps_in(&markers)
}

#[cfg(unix)]
#[test]
fn a_resumed_schema_4_run_acts_on_todays_slot_limits() {
    assert!(
        resumed_overlap("topology-resume-overlap", "") > 0,
        "the resumed workers never ran at once at the recorded `max_parallel = 2`, so the bound \
         below would prove nothing"
    );
    assert_eq!(
        resumed_overlap("topology-resume-agent-bound", "max_per_agent = 1\n"),
        0,
        "the resume dropped today's `max_per_agent = 1`"
    );
}

/// P12-W1's rest and P12-W5. Through `engine::run_with`: the owner record was
/// published before the first probe (P3b before P4); its runner is the one
/// `run_started(4)` records, byte for byte; the commit record names the digest of
/// the committed first line; the log begins with `run_started` and ends with
/// `run_finished`; the integration ref holds the last merge; and `report.json` is
/// the projection of the committed log.
#[test]
fn a_schema_4_run_publishes_its_records_in_order_and_records_one_runner() {
    let (_tree, repo) = temp_engine_repo("topology-records");
    let witness = OwnerWitness {
        stub: TopologyStub::new(TOPOLOGY_STUB_EDIT, 0),
        private_root: private_root_for(&repo),
        owner_at_probe: Mutex::new(Vec::new()),
    };
    let report = run_with(&topology_options(&repo), &BorrowedOwnerWitness(&witness))
        .expect("the run completes");
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:#?}");
    let seen = witness
        .owner_at_probe
        .lock()
        .expect("owner witness")
        .clone();
    assert!(
        !seen.is_empty() && seen.iter().all(|published| *published),
        "every probe, the first included, ran after owner.json was published: {seen:?}"
    );

    let private = private_dir_of(&repo, &report.run_id);
    let owner: serde_json::Value =
        serde_json::from_slice(&fs::read(private.join("owner.json")).expect("owner.json"))
            .expect("owner.json parses");
    let first = first_line_of_the_log(&repo, &report.run_id);
    assert_eq!(
        serde_json::to_vec(&owner["runner"]).expect("the owner's runner"),
        serde_json::to_vec(&first["data"]["runner"]).expect("run_started's runner"),
        "owner.json records the runner run_started(4) records"
    );
    assert_eq!(owner["run_id"], first["data"]["run_id"]);

    let public = rundir::public_dir(&repo, &report.run_id);
    let log = fs::read(public.join("events.jsonl")).expect("the log");
    let first_line = &log[..log
        .iter()
        .position(|byte| *byte == b'\n')
        .expect("a first line")];
    let digest = format!("{:x}", <sha2::Sha256 as sha2::Digest>::digest(first_line));
    let committed: serde_json::Value =
        serde_json::from_slice(&fs::read(private.join("committed.json")).expect("committed.json"))
            .expect("committed.json parses");
    let recorded = committed["run_started_sha256"]
        .as_str()
        .expect("committed.json names the first line's digest");
    assert!(
        recorded == digest || recorded == format!("sha256:{digest}"),
        "committed.json names {recorded}, and the first line digests {digest}"
    );

    let kinds = event_kinds(&repo, &report.run_id);
    assert_eq!(kinds.first().map(String::as_str), Some("run_started"));
    assert_eq!(kinds.last().map(String::as_str), Some("run_finished"));
    let last_merge = fs::read_to_string(public.join("events.jsonl"))
        .expect("the log")
        .lines()
        .filter_map(|line| {
            let event: serde_json::Value = serde_json::from_str(line).expect("an event");
            (event["event"] == "task_merged").then(|| {
                event["data"]["merged_sha"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned()
            })
        })
        .next_back()
        .expect("a task merged");
    assert_eq!(
        git_in(
            &repo,
            &[
                "rev-parse",
                &format!("refs/heads/upstroke/run-{}", report.run_id)
            ]
        )
        .trim(),
        last_merge,
        "the integration ref holds the last merge the log records"
    );
    let projected = crate::engine::topology::observe::observe(&repo, &report.run_id)
        .expect("observes")
        .expect("a schema-4 log")
        .report()
        .expect("projects");
    assert!(
        projected.is_fresh_against(&fs::read(public.join("report.json")).expect("report.json")),
        "report.json is the projection of the committed log"
    );
}

/// P12-W14 (D4). At width 1 the production composition through the step loop
/// (an adapter source that cannot be shared) and through `run_concurrently` (one
/// that can) give the same canonical projection of the same plan.
#[test]
fn at_width_one_the_step_loop_and_the_concurrent_driver_project_alike() {
    let projection = |repo: &Path, run_id: &str| {
        let report = crate::engine::topology::observe::observe(repo, run_id)
            .expect("observes")
            .expect("a schema-4 log")
            .report()
            .expect("projects");
        let tasks: Vec<(String, String, u32, u32, u32, usize)> = report
            .tasks
            .iter()
            .map(|task| {
                (
                    task.display_id.clone(),
                    task.state.clone(),
                    task.rung,
                    task.attempts_on_rung,
                    task.defers,
                    task.generations.len(),
                )
            })
            .collect();
        let ledger: Vec<(String, String)> = report
            .integration_ledger
            .iter()
            .map(|row| (row.basis.clone(), row.terminal.clone()))
            .collect();
        let mut kinds = event_kinds(repo, run_id);
        kinds.sort();
        (report.outcome, tasks, ledger, kinds)
    };

    let (_step_tree, step_repo) = temp_engine_repo("topology-width-one-step");
    let stub = TopologyStub::new(TOPOLOGY_STUB_EDIT, 0);
    let stepped = run_with(&topology_options(&step_repo), &BorrowedTopologyStub(&stub))
        .expect("the step loop completes");

    let (_shared_tree, shared_repo) = temp_engine_repo("topology-width-one-shared");
    let source = SharedTopologyStubs(std::sync::Arc::new(TopologyStub::new(
        TOPOLOGY_STUB_EDIT,
        0,
    )));
    let concurrent = run_with(&topology_options(&shared_repo), &source)
        .expect("the concurrent driver completes");

    assert_eq!(stepped.outcome(), RunOutcome::Complete, "{stepped:#?}");
    assert_eq!(
        projection(&step_repo, &stepped.run_id),
        projection(&shared_repo, &concurrent.run_id),
        "the two drivers' canonical projections differ"
    );
}

/// P12-W2 and P12-W16 (D6): a schema-4 run whose process dies by SIGKILL,
/// SIGTERM or SIGINT inside an attempt leaves a log that ends mid-attempt; the
/// resume settles that attempt as interrupted, appends `run_resumed`, and
/// continues to the end. No graceful drain is claimed: the signal ends the
/// command.
#[cfg(unix)]
#[test]
fn a_schema_4_run_ended_by_a_signal_mid_attempt_resumes_and_settles_the_attempt() {
    for signal in ["KILL", "TERM", "INT"] {
        let (_tree, repo) = temp_engine_repo(&format!("topology-signal-{signal}"));
        let helper = std::sync::atomic::AtomicU32::new(0);
        signal_mid_attempt(&repo, signal, SignalFault::None, &helper);
    }
}

#[cfg(unix)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SignalFault {
    None,
    Readiness,
    Signal,
    DeliveryNeverExits,
}

/// The pids the signal helper's workers wrote, one a line. Each leads a process group of its
/// own: the process funnel puts every agent it starts in one.
#[cfg(unix)]
fn worker_pids(repo: &Path) -> Vec<u32> {
    fs::read_to_string(signalled_workers(repo))
        .unwrap_or_default()
        .lines()
        .filter_map(|line| line.trim().parse().ok())
        .collect()
}

#[cfg(unix)]
fn stalled_delivery_pid(repo: &Path) -> PathBuf {
    repo.with_file_name("stalled-delivery.pid")
}

#[cfg(unix)]
fn delivery_program(repo: &Path, fault: SignalFault) -> PathBuf {
    if fault != SignalFault::DeliveryNeverExits {
        return PathBuf::from("kill");
    }
    let stalled = repo.with_file_name("kill-that-never-exits");
    crate::runner::container::write_program_in_its_own_process(
        &stalled,
        &format!(
            "#!/bin/sh\necho $$ > '{0}.tmp' && mv '{0}.tmp' '{0}'\nexec sleep 600\n",
            stalled_delivery_pid(repo).display()
        ),
    );
    stalled
}

#[cfg(unix)]
fn deliver_within_its_bound(program: &Path, signal: &str, pid: &str, repo: &Path) -> bool {
    use crate::workspace_manager::fixture::{COLLECT_BOUND, LinkedChild};
    use std::ffi::OsStr;

    let (delivery, _said) = LinkedChild::spawn_program(
        program.as_os_str(),
        &[OsStr::new("-s"), OsStr::new(signal), OsStr::new(pid)],
        &repo.with_file_name("signal-delivery.stderr"),
    );
    delivery
        .wait_within(COLLECT_BOUND)
        .unwrap_or_else(|| {
            panic!(
                "`{} -s {signal} {pid}` had not exited {COLLECT_BOUND:?} after it was started",
                program.display()
            )
        })
        .success()
}

#[cfg(unix)]
fn signal_mid_attempt(
    repo: &Path,
    signal: &str,
    fault: SignalFault,
    helper: &std::sync::atomic::AtomicU32,
) {
    use crate::workspace_manager::fixture::{COLLECT_BOUND, LinkedChild, await_group_end};
    use std::sync::atomic::Ordering;

    let (child, _lines) = LinkedChild::spawn_with(
        "engine::tests::topology_signalled_child",
        &[("UPSTROKE_TOPOLOGY_SIGNAL_REPO", repo.as_os_str())],
        &repo.with_file_name("signalled-child.stderr"),
        |pid| {
            helper.store(pid, Ordering::SeqCst);
            std::thread::Builder::new()
        },
    );
    let mut run_id = None;
    for _ in 0..600 {
        if let Some(id) = rundir::latest_run(repo) {
            let log = rundir::public_dir(repo, &id).join("events.jsonl");
            let started =
                fs::read_to_string(&log).is_ok_and(|text| text.contains("\"attempt_started\""));
            let working = worker_pids(repo)
                .into_iter()
                .any(crate::workspace_manager::fixture::process_exists);
            if started && working {
                run_id = Some(id);
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let run_id = run_id
        .filter(|_| fault != SignalFault::Readiness)
        .expect("the child reached an attempt within a minute");
    let pid = helper.load(Ordering::SeqCst).to_string();
    let delivered = fault != SignalFault::Signal
        && deliver_within_its_bound(&delivery_program(repo, fault), signal, &pid, repo);
    assert!(delivered, "kill -s {signal}");
    let ended = child
        .wait_within(COLLECT_BOUND)
        .unwrap_or_else(|| panic!("the child outlived SIG{signal} by {COLLECT_BOUND:?}"));
    assert!(!ended.success(), "SIG{signal} ended the child: {ended:?}");

    let log = rundir::public_dir(repo, &run_id).join("events.jsonl");
    let killed = fs::read_to_string(&log).expect("the log");
    assert!(
        !killed.contains("\"run_finished\"") && !killed.contains("\"attempt_finished\""),
        "SIG{signal}: the log ends inside the attempt:\n{killed}"
    );
    until_released(repo, &run_id);
    for worker in worker_pids(repo) {
        let group = i32::try_from(worker).expect("a pid");
        await_group_end(group).unwrap_or_else(|outlived| panic!("SIG{signal}: {outlived}"));
    }
    let source = SharedTopologyStubs(std::sync::Arc::new(TopologyStub::new(
        TOPOLOGY_STUB_EDIT,
        0,
    )));
    let mut resume = resume_options(repo, &run_id);
    resume.private_root = None;
    let report = resume_with(&resume, &source).expect("the resume continues the run");
    assert_eq!(
        report.outcome(),
        RunOutcome::Complete,
        "SIG{signal}: {report:#?}"
    );
    let resumed = fs::read_to_string(&log).expect("the log");
    assert!(
        resumed.starts_with(&killed),
        "SIG{signal}: the resume rewrote the log it adopted"
    );
    let appended: Vec<String> = resumed[killed.len()..]
        .lines()
        .map(|line| {
            serde_json::from_str::<serde_json::Value>(line).expect("an event")["event"]
                .as_str()
                .unwrap_or_default()
                .to_owned()
        })
        .collect();
    let at = |kind: &str| appended.iter().position(|seen| seen == kind);
    let (Some(resumed_at), Some(interrupted_at), Some(finished_at)) = (
        at("run_resumed"),
        at("attempt_interrupted"),
        at("run_finished"),
    ) else {
        panic!("SIG{signal}: the resume appended {appended:?}");
    };
    assert!(
        interrupted_at < resumed_at && resumed_at < finished_at,
        "SIG{signal}: the interrupted attempt settled (recovery's step C), then \
         run_resumed (its step G), then the end: {appended:?}"
    );
    assert!(
        report.total_is_floor(),
        "SIG{signal}: the interrupted attempt's unknown spend makes the total a floor: \
         {report:#?}"
    );
    let interrupted: Vec<&events::AttemptRecord> = report
        .tasks
        .iter()
        .flat_map(|task| task.attempts.iter())
        .filter(|record| {
            record
                .failure
                .as_ref()
                .is_some_and(|failure| failure.kind == FailureKind::Interrupted)
        })
        .collect();
    let [kept] = interrupted.as_slice() else {
        panic!("SIG{signal}: the report keeps the one interrupted attempt: {report:#?}");
    };
    let logged: Vec<serde_json::Value> = resumed
        .lines()
        .map(|line| serde_json::from_str(line).expect("an event"))
        .collect();
    let identity = |event: &serde_json::Value| {
        ["key", "generation", "attempt"].map(|field| event["data"][field].as_u64())
    };
    let settled = logged
        .iter()
        .find(|event| event["event"] == "attempt_interrupted")
        .expect("the interrupted settlement");
    let started = logged
        .iter()
        .find(|event| event["event"] == "attempt_started" && identity(event) == identity(settled))
        .expect("the start of the attempt the resume settled interrupted");
    assert_eq!(
        (kept.cost_usd, kept.model.as_str()),
        (
            None,
            started["data"]["binding"]["model"]
                .as_str()
                .unwrap_or_default()
        ),
        "SIG{signal}: its spend is unknown and its model is the one its start recorded: \
         {kept:#?}"
    );
}

/// J8, PR12 regression review #5: the signal test owns its helper from the spawn, so neither a
/// readiness that fails nor a signal that cannot be delivered leaves the helper, or the worker
/// it started, running. The reviewer's witness was a `kill` that exited 1: the parent failed
/// at its assertion, and seven of the helper's processes outlived it. Each fault is injected
/// into the driver once the helper's worker runs. The driver fails as the test would, and
/// then the helper has been reaped, the worker's group is gone, and nothing holds the run,
/// each within its bound.
#[cfg(unix)]
#[test]
fn the_signal_tests_helper_is_settled_when_its_readiness_or_its_signal_fails() {
    use crate::workspace_manager::fixture::{await_group_end, process_exists};

    for (fault, says) in [
        (
            SignalFault::Readiness,
            "the child reached an attempt within a minute",
        ),
        (SignalFault::Signal, "kill -s TERM"),
    ] {
        let (_tree, repo) = temp_engine_repo(&format!("topology-signal-{fault:?}"));
        let helper = std::sync::atomic::AtomicU32::new(0);
        let payload = std::panic::catch_unwind(|| {
            signal_mid_attempt(&repo, "TERM", fault, &helper);
        })
        .expect_err("the injected fault fails the driver");
        let Some(message) = payload.downcast_ref::<String>() else {
            panic!("{fault:?}: the driver's message is not a String");
        };
        assert!(message.contains(says), "{fault:?}: {message}");

        let child = helper.load(std::sync::atomic::Ordering::SeqCst);
        assert_ne!(child, 0, "{fault:?}: the driver spawned its helper");
        assert!(
            !process_exists(child),
            "{fault:?}: the helper {child} was reaped"
        );
        let workers = worker_pids(&repo);
        assert!(!workers.is_empty(), "{fault:?}: the premise, a worker ran");
        for worker in workers {
            let group = i32::try_from(worker).expect("a pid");
            await_group_end(group).unwrap_or_else(|outlived| panic!("{fault:?}: {outlived}"));
        }
        let run_id = rundir::latest_run(&repo).expect("the helper's run");
        until_released(&repo, &run_id);
    }
}

#[cfg(unix)]
#[test]
fn the_signal_tests_helper_is_settled_when_the_command_delivering_its_signal_never_exits() {
    use crate::workspace_manager::fixture::{await_group_end, process_exists};

    let (_tree, repo) = temp_engine_repo("topology-signal-stalled-delivery");
    let helper = std::sync::atomic::AtomicU32::new(0);
    let payload = std::panic::catch_unwind(|| {
        signal_mid_attempt(&repo, "TERM", SignalFault::DeliveryNeverExits, &helper);
    })
    .expect_err("a delivery that never exits fails the driver");
    let Some(message) = payload.downcast_ref::<String>() else {
        panic!("the driver's message is not a String");
    };
    assert!(message.contains("had not exited"), "{message}");

    let stalled: u32 = fs::read_to_string(stalled_delivery_pid(&repo))
        .expect("the stalled delivery recorded its pid")
        .trim()
        .parse()
        .expect("a pid");
    assert!(
        !process_exists(stalled),
        "the stalled delivery {stalled} was killed and reaped"
    );
    let child = helper.load(std::sync::atomic::Ordering::SeqCst);
    assert_ne!(child, 0, "the driver spawned its helper");
    assert!(!process_exists(child), "the helper {child} was reaped");
    let workers = worker_pids(&repo);
    assert!(!workers.is_empty(), "the premise, a worker ran");
    for worker in workers {
        let group = i32::try_from(worker).expect("a pid");
        await_group_end(group).unwrap_or_else(|outlived| panic!("{outlived}"));
    }
    let run_id = rundir::latest_run(&repo).expect("the helper's run");
    until_released(&repo, &run_id);
}

#[cfg(unix)]
const ANNOUNCE_PLAN: &str = "## Lay the first stone\n\
                             <!-- upstroke: id=first depends= paths=first/** -->\n\
                             Lay it.\n\n## Ask about the widget\n\
                             <!-- upstroke: id=asks depends=first paths=asks/** -->\n\
                             Ask which of the two formats the spec means.\n\n\
                             ## Wait for the release\n\
                             <!-- upstroke: id=waits depends=first paths=waits/** -->\n\
                             Wait until the test releases this task.\n";

#[cfg(unix)]
#[test]
fn a_schema_4_question_is_announced_before_its_sibling_is_released() {
    use crate::workspace_manager::fixture::LinkedChild;
    use std::ffi::OsStr;

    for (resumed, step) in [(false, true), (false, false), (true, true), (true, false)] {
        let cell = format!(
            "{} run, {} driver",
            if resumed { "resumed" } else { "fresh" },
            if step { "step" } else { "concurrent" }
        );
        let (tree, repo) = temp_engine_repo(&format!(
            "topology-announce-{}{}",
            u8::from(resumed),
            u8::from(step)
        ));
        seed(
            &repo,
            ANNOUNCE_PLAN,
            Some(&format!(
                "[engine]\nmax_parallel = {}\n\n[interaction]\nmode = \"never\"\n\
                 notify = [\"cli\"]\n",
                if step { 1 } else { 2 }
            )),
        );
        let gate = tree.path().join("gate");
        fs::create_dir(&gate).expect("the gate");
        let run_id = if resumed {
            let stub = TopologyStub::new(&announcing_worker(&gate), 0);
            let mut opts = topology_options(&repo);
            opts.config_path = Some(repo.join("upstroke.toml"));
            opts.budget_usd = Some(0.015);
            let stopped = if step {
                run_with(&opts, &BorrowedTopologyStub(&stub))
            } else {
                run_with(&opts, &SharedTopologyStubs(std::sync::Arc::new(stub)))
            }
            .expect("the first run ends");
            assert_eq!(
                stopped.outcome(),
                RunOutcome::BudgetExceeded,
                "{cell}: {stopped:#?}"
            );
            assert!(
                !gate.join("waits-started").exists(),
                "{cell}: the premise, the budget stopped the run before the sibling ran"
            );
            until_released(&repo, &stopped.run_id);
            stopped.run_id
        } else {
            String::new()
        };
        let stderr = tree.path().join("announcing-child.stderr");
        let (child, _lines) = LinkedChild::spawn(
            "engine::tests::topology_announcing_child",
            &[
                ("UPSTROKE_ANNOUNCE_REPO", repo.as_os_str()),
                ("UPSTROKE_ANNOUNCE_GATE", gate.as_os_str()),
                (
                    "UPSTROKE_ANNOUNCE_DRIVER",
                    OsStr::new(if step { "step" } else { "concurrent" }),
                ),
                ("UPSTROKE_ANNOUNCE_RUN", OsStr::new(&run_id)),
            ],
            &stderr,
        );
        let announced = |said: &str| {
            said.lines().any(|line| {
                line.starts_with("question ") && line.ends_with("— parking asks; the run continues")
            })
        };
        let deadline = std::time::Instant::now() + Duration::from_secs(90);
        loop {
            let said = fs::read_to_string(&stderr).unwrap_or_default();
            if announced(&said) && gate.join("waits-started").exists() {
                break;
            }
            if let Some(ended) = child.wait_within(Duration::ZERO) {
                panic!(
                    "{cell}: the run ended ({ended:?}) before it announced the question while \
                     the sibling was held:\n{said}"
                );
            }
            assert!(
                std::time::Instant::now() < deadline,
                "{cell}: no announcement of the question while the sibling was held, within \
                 90 s:\n{said}"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
        fs::write(gate.join("release"), b"").expect("the sibling's release");
        let ended = child
            .wait_within(Duration::from_secs(300))
            .unwrap_or_else(|| panic!("{cell}: the run outlived its release by 300 s"));
        assert!(
            ended.success(),
            "{cell}: {ended:?}\n{}",
            fs::read_to_string(&stderr).unwrap_or_default()
        );
    }
}

// ---------------------------------------------------------------------------
// PR12 S10: the read commands over a schema-4 log (D1, D3, D10)
// ---------------------------------------------------------------------------

/// P12-W12. `status` on a schema-4 run reads the committed prefix only: its
/// projection equals the barrier's replay and the run's own `report.json`; with
/// a writer holding the run and a torn tail at the end of the log, it reports
/// both, names the runner identity, and leaves the log's bytes and mtime as it
/// found them; `--follow` prints the run's events and stops once it finished.
#[test]
fn a_schema_4_status_reads_the_committed_prefix_and_leaves_the_log_as_it_found_it() {
    let (_tree, repo) = temp_engine_repo("topology-status");
    let source = SharedTopologyStubs(std::sync::Arc::new(TopologyStub::new(
        TOPOLOGY_STUB_EDIT,
        0,
    )));
    let report = run_with(&topology_options(&repo), &source).expect("the schema-4 run completes");
    until_released(&repo, &report.run_id);
    let public = rundir::public_dir(&repo, &report.run_id);
    let log = public.join("events.jsonl");

    let projected = crate::engine::topology::observe::observe(&repo, &report.run_id)
        .expect("the observer reads the run")
        .expect("a schema-4 log")
        .report()
        .expect("the projection derives");
    let written: crate::engine::topology::report::TopologyReport =
        serde_json::from_slice(&fs::read(public.join("report.json")).expect("report.json"))
            .expect("report.json is a topology report");
    assert_eq!(
        projected, written,
        "status's projection is the one the live run wrote at its finish"
    );
    let plan_bytes = fs::read(public.join("plan.normalized.json")).expect("the frozen plan");
    let inputs = crate::topology::fold::FrozenInputs {
        plan: serde_json::from_slice(&plan_bytes).expect("the frozen plan parses"),
        normalized_plan_digest: events::normalized_plan_digest(&plan_bytes),
    };
    let mut warnings = Vec::new();
    let barrier = crate::events::log::establish_stable_prefix(
        &log,
        inputs,
        None,
        &mut warnings,
        &mut crate::events::log::NoEventHooks,
    )
    .expect("the barrier proves the finished run's prefix");
    assert_eq!(
        crate::engine::topology::report::topology_status(&report.run_id, &barrier)
            .expect("the barrier's replay projects"),
        projected,
        "and the one the barrier's replay derives"
    );
    drop(barrier);

    let mut torn = fs::read(&log).expect("the log");
    torn.extend_from_slice(b"{\"ts\":\"2026-11-02T09:00:00Z\",\"event\":\"capacity_snap");
    fs::write(&log, &torn).expect("a writer interrupted mid-line");
    let mtime = fs::metadata(&log)
        .and_then(|metadata| metadata.modified())
        .expect("the log's mtime");
    let held = RunLock::acquire(&public).expect("a live writer's lock");
    let status = crate::status::load(&repo, Some(&report.run_id)).expect("status reads a held run");
    let rendered = crate::status::render(&status);
    drop(held);
    for expected in [
        format!("run {}: complete", report.run_id),
        "runner: host (host-v1)".to_owned(),
        "log: schema 4".to_owned(),
        "state: running now".to_owned(),
        "byte(s) after its last commit marker".to_owned(),
    ] {
        assert!(rendered.contains(&expected), "`{expected}` in:\n{rendered}");
    }
    assert_eq!(
        fs::read(&log).expect("the log"),
        torn,
        "status changed the log's bytes"
    );
    assert_eq!(
        fs::metadata(&log)
            .and_then(|metadata| metadata.modified())
            .expect("the log's mtime"),
        mtime,
        "status changed the log's mtime"
    );
    assert_eq!(
        crate::engine::topology::observe::observe(&repo, &report.run_id)
            .expect("observes")
            .expect("a schema-4 log")
            .report()
            .expect("projects"),
        projected,
        "a torn tail is not part of the projection"
    );

    let sleeper = RecordingSleeper::default();
    let mut out: Vec<u8> = Vec::new();
    crate::status::follow(&status, &sleeper, Duration::ZERO, 2, &mut out).expect("follow");
    let followed = String::from_utf8(out).expect("utf8");
    let first = followed.lines().next().unwrap_or_default();
    assert!(first.contains("run_started"), "{followed}");
    assert!(
        followed
            .lines()
            .last()
            .is_some_and(|last| last.contains("run_finished complete")),
        "{followed}"
    );
}

#[test]
fn a_schema_4_follow_prints_one_line_per_event_and_no_raw_control_whatever_the_plan_or_the_record_holds()
 {
    let (_tree, repo) = temp_engine_repo("topology-follow-controls");
    seed(
        &repo,
        "## One\n<!-- upstroke: id=t1\u{1b}[2J kind=implement depends= -->\nMake it.\n",
        None,
    );
    let source = SharedTopologyStubs(std::sync::Arc::new(TopologyStub::new(
        TOPOLOGY_STUB_EDIT,
        0,
    )));
    let report = run_with(&topology_options(&repo), &source).expect("the run completes");
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:#?}");
    until_released(&repo, &report.run_id);
    let committed = log_events(&repo, &report.run_id);

    let loaded = crate::status::load(&repo, Some(&report.run_id)).expect("status reads the run");
    let mut out: Vec<u8> = Vec::new();
    crate::status::follow(
        &loaded,
        &RecordingSleeper::default(),
        Duration::ZERO,
        2,
        &mut out,
    )
    .expect("follow");
    let followed = String::from_utf8(out).expect("utf8");
    assert_eq!(
        followed.lines().count(),
        committed.len(),
        "one physical line per committed event:\n{followed}"
    );
    assert!(
        !followed.chars().any(|c| c.is_control() && c != '\n'),
        "no raw control reaches the terminal: {followed:?}"
    );
    for kind in ["attempt_started", "task_merged"] {
        assert!(
            followed
                .lines()
                .any(|line| line.contains(kind) && line.contains("t1\\u{1b}[2J")),
            "the `{kind}` line names the task with its control made visible:\n{followed}"
        );
    }

    let observed = crate::engine::topology::observe::observe(&repo, &report.run_id)
        .expect("observes")
        .expect("a schema-4 log");
    let mut hostile = observed
        .events()
        .iter()
        .find(|event| event.body.kind() == "attempt_started")
        .expect("the run started an attempt")
        .clone();
    hostile.ts = "2026-11-04T00:00:00Z\r\u{1b}[2K".to_owned();
    if let crate::topology::events::TopologyEventBody::AttemptStarted { data } = &mut hostile.body {
        data.binding.agent = "agent\n\u{1b}[31m".to_owned();
        data.binding.model = "model\t\u{7}".to_owned();
    }
    let line = observed.describe(&hostile);
    assert!(
        !line.chars().any(char::is_control),
        "a recorded field's controls never reach the line: {line:?}"
    );
    assert!(
        line.contains("agent \\u{1b}[31m") && line.contains("model \\u{7}"),
        "{line:?}"
    );
}

#[test]
fn a_fresh_schema_4_run_refuses_a_staged_an_unstaged_or_an_untracked_change_before_any_worker_or_log()
 {
    for dirt in ["staged", "unstaged", "untracked"] {
        let (_tree, repo) = temp_engine_repo(&format!("topology-dirty-{dirt}"));
        match dirt {
            "staged" => {
                fs::write(repo.join("README.md"), "a staged edit\n").expect("an edit");
                git_in(&repo, &["add", "README.md"]);
            }
            "unstaged" => fs::write(repo.join("README.md"), "an unstaged edit\n").expect("an edit"),
            _ => fs::write(repo.join("notes.txt"), "an untracked file\n").expect("a new file"),
        }
        let stub = ObservedStub::new(0);
        let refused = run_with(&topology_options(&repo), &BorrowedObserved(&stub))
            .expect_err("a dirty tree is refused");
        assert!(
            refused.to_string().contains("working tree is not clean"),
            "{dirt}: {refused}"
        );
        assert!(
            stub.built().is_empty(),
            "{dirt}: no worker or reviewer was built"
        );
        assert!(
            rundir::run_dir_names(&repo).is_empty(),
            "{dirt}: no run directory, so no run log, exists"
        );
        let kept = match dirt {
            "staged" => git_in(&repo, &["diff", "--cached", "--name-only"]).contains("README.md"),
            "unstaged" => {
                fs::read_to_string(repo.join("README.md")).expect("README.md")
                    == "an unstaged edit\n"
            }
            _ => repo.join("notes.txt").exists(),
        };
        assert!(
            kept,
            "{dirt}: the change is the operator's and is untouched"
        );
    }

    let (_tree, repo) = temp_engine_repo("topology-clean-twice");
    for run in ["first", "second"] {
        let stub = ObservedStub::new(0);
        let report = run_with(&topology_options(&repo), &BorrowedObserved(&stub))
            .unwrap_or_else(|error| panic!("the {run} run of a clean tree is admitted: {error}"));
        assert_eq!(report.outcome(), RunOutcome::Complete, "{run}: {report:#?}");
        assert!(!stub.workers().is_empty(), "{run}: its workers ran");
        until_released(&repo, &report.run_id);
    }
}

/// Cuts a run's log after the first committed line whose event is `kind` and that `matches`,
/// as a coordinator that died just after appending it leaves the log: a committed prefix.
fn cut_log_after(
    repo: &Path,
    run_id: &str,
    kind: &str,
    matches: impl Fn(&serde_json::Value) -> bool,
) {
    let log = rundir::public_dir(repo, run_id).join("events.jsonl");
    let text = fs::read_to_string(&log).expect("the run's event log");
    let mut kept = String::new();
    for line in text.split_inclusive('\n') {
        kept.push_str(line);
        let event: serde_json::Value =
            serde_json::from_str(line.trim_end()).expect("a committed event");
        if event["event"] == kind && matches(&event) {
            fs::write(&log, kept).expect("the log, cut where its coordinator died");
            return;
        }
    }
    panic!("the premise: the log commits a matching `{kind}`:\n{text}");
}

/// The `status` a reader gets for `run_id`, through the public load and render.
fn upstroke_status(repo: &Path, run_id: &str) -> String {
    crate::status::render(&crate::status::load(repo, Some(run_id)).expect("status reads the run"))
}

/// The ledger row `status` prints for task `id`, split into its cells.
fn ledger_cells(status: &str, id: &str) -> Vec<String> {
    let ledger = status
        .split_once("\nledger:\n")
        .map(|(_, ledger)| ledger)
        .unwrap_or_else(|| panic!("status prints a ledger:\n{status}"));
    ledger
        .lines()
        .find(|line| line.split_whitespace().next() == Some(id))
        .unwrap_or_else(|| panic!("the ledger has a row for `{id}`:\n{status}"))
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

/// L3 (regression review 3, finding 3; DESIGN §18, "the report and its ledger (§13)"). A
/// schema-4 `status` prints the accounting projection `run` reports: the task ledger, the
/// known total, its floors and the per-pool drain. A completed run of two tasks on one pool
/// prints, byte for byte, the ledger the run itself returned, and its two fast merges say
/// they spent nothing on review. The same log cut just after the second task's
/// `attempt_started`, as a coordinator that died there leaves it, prints the first task's
/// known spend and marks everything the cut-off attempt reaches as a floor: its row, the
/// total and the pool, with the legend that says what `?` means.
#[test]
fn a_schema_4_status_prints_the_ledger_run_reports_for_a_completed_and_an_interrupted_run() {
    let (tree, repo) = temp_engine_repo("topology-status-ledger");
    let mut opts = topology_options(&repo);
    opts.pools_path = Some(stub_pool_file(&tree));
    let source = SharedTopologyStubs(std::sync::Arc::new(TopologyStub::new(
        TOPOLOGY_STUB_EDIT,
        0,
    )));
    let report = run_with(&opts, &source).expect("the schema-4 run completes");
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:#?}");
    until_released(&repo, &report.run_id);
    let ledger = report.render_ledger();
    assert!(
        !report.total_is_floor() && report.total_cost_usd > 0.0 && ledger.contains("claude-max"),
        "the premise: every amount is known and the pool paid for it:\n{ledger}"
    );

    let status = upstroke_status(&repo, &report.run_id);
    assert!(
        status.contains(&ledger),
        "status prints the ledger the run reported:\n{status}\n--- the run's ledger:\n{ledger}"
    );
    let fast: Vec<&str> = status
        .lines()
        .filter(|line| line.contains(" fast expected "))
        .collect();
    assert_eq!(
        fast.len(),
        2,
        "the premise: both merges were fast:\n{status}"
    );
    assert!(
        fast.iter()
            .all(|line| line.ends_with("(0 review(s), cost none)")),
        "a fast merge runs no review, so its spend is known to be none:\n{status}"
    );

    let first = task(&report, "t1")
        .total_cost_usd()
        .expect("the first task's spend is known");
    let second = crate::engine::topology::observe::observe(&repo, &report.run_id)
        .expect("observes")
        .expect("a schema-4 log")
        .registry()
        .and_then(|registry| {
            registry
                .entries()
                .iter()
                .find(|entry| entry.display_id.as_str() == "t2")
        })
        .map(|entry| entry.key.0)
        .expect("t2 is registered");
    cut_log_after(&repo, &report.run_id, "attempt_started", |event| {
        event["data"]["key"] == second
    });
    let status = upstroke_status(&repo, &report.run_id);
    assert!(status.contains("state: interrupted"), "{status}");
    let known = ledger_cells(&status, "t1");
    assert_eq!(
        known.last().map(String::as_str),
        Some(format!("${first:.4}").as_str()),
        "the first task's spend is known and exact: {known:?}\n{status}"
    );
    let cut = ledger_cells(&status, "t2");
    assert!(
        cut.get(1).map(String::as_str) == Some("1")
            && cut.len() > 3
            && cut.iter().rev().take(3).all(|cell| cell == "—"),
        "the cut-off attempt is counted, and nothing it spent is known: {cut:?}\n{status}"
    );
    for floor in [
        format!("  total ${first:.4}? (api-equivalent"),
        "  `?` marks a figure missing an attempt".to_owned(),
        format!("    claude-max: 3 attempt(s), ${first:.4}?"),
    ] {
        assert!(
            status.contains(&floor),
            "the known spend is a floor, not a total: no `{floor}` in\n{status}"
        );
    }
}

/// L3, the integration reviews. Two independent tasks at width 2 make the second merge a stale
/// candidate's re-verification, whose review pass is paid for by the verified task. A completed
/// run's `status` prints the ledger the run reported, that pass's cost on the integration row
/// and "none" on the fast one. The same log cut just after `merge_verification_started` leaves
/// the verification in flight: the verified task's review is a floor, and so is the total.
#[cfg(unix)]
#[test]
fn a_schema_4_status_prints_the_integration_review_spend_run_reports_and_its_floor_once_cut_off() {
    let (tree, repo) = temp_engine_repo("topology-status-integration");
    seed(
        &repo,
        TWO_INDEPENDENT_TASKS,
        Some("[engine]\nmax_parallel = 2\n"),
    );
    let mut opts = topology_options(&repo);
    opts.pools_path = Some(stub_pool_file(&tree));
    let source = SharedTopologyStubs(std::sync::Arc::new(TopologyStub::new(
        "printf 'edited by the topology stub\\n' > \"out.$$.txt\"",
        0,
    )));
    let report = run_with(&opts, &source).expect("the schema-4 run completes");
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:#?}");
    until_released(&repo, &report.run_id);
    let observed = crate::engine::topology::observe::observe(&repo, &report.run_id)
        .expect("observes")
        .expect("a schema-4 log");
    let projected = observed.report().expect("projects");
    let stale = projected
        .integration_ledger
        .iter()
        .find(|row| row.basis == "stale_clean")
        .unwrap_or_else(|| {
            panic!(
                "the premise: one merge was a stale candidate's re-verification: {:#?}",
                projected.integration_ledger
            )
        });
    let paid = match (stale.reviews, stale.review_cost_usd) {
        (1.., Some(cost)) if !stale.review_cost_incomplete => cost,
        _ => panic!("the premise: the re-verification ran a priced review pass: {stale:#?}"),
    };
    let verified = observed.display_id(stale.candidate.key);

    let ledger = report.render_ledger();
    let status = upstroke_status(&repo, &report.run_id);
    assert!(
        status.contains(&ledger),
        "status prints the ledger the run reported:\n{status}\n--- the run's ledger:\n{ledger}"
    );
    for (basis, cost) in [
        (
            "stale_clean",
            format!("({} review(s), cost ${paid:.4})", stale.reviews),
        ),
        ("fast", "(0 review(s), cost none)".to_owned()),
    ] {
        assert!(
            status
                .lines()
                .any(|line| line.contains(&format!(" {basis} expected ")) && line.ends_with(&cost)),
            "the {basis} integration row ends `{cost}`:\n{status}"
        );
    }

    cut_log_after(&repo, &report.run_id, "merge_verification_started", |_| {
        true
    });
    let status = upstroke_status(&repo, &report.run_id);
    assert!(status.contains("state: interrupted"), "{status}");
    let cells = ledger_cells(&status, &verified);
    let review = cells
        .get(cells.len().saturating_sub(2))
        .map_or("", String::as_str);
    assert!(
        review.starts_with('$') && review.ends_with('?'),
        "the verified task's review is what its attempt's pass cost, and a floor: {cells:?}\n{status}"
    );
    assert!(
        status
            .lines()
            .any(|line| line.trim_start().starts_with("total $")
                && line.contains("? (api-equivalent")),
        "the run's total is a floor:\n{status}"
    );
    assert!(
        status
            .lines()
            .any(|line| line.contains(" -> open ")
                && line.ends_with("(0 review(s), cost ? (unknown))")),
        "the verification in flight is unknown on its integration row:\n{status}"
    );
}

/// L4 (regression review 3, finding 4). A completed production run in a repository with no
/// `.upstroke` exclusion leaves its generated public files (its log, frozen plan, report and
/// locks) where Git ignores them: `git status` lists nothing, and an ordinary `git add -A`
/// stages nothing.
#[test]
fn a_completed_schema_4_run_leaves_no_generated_file_for_an_ordinary_git_add_to_take() {
    let (_tree, repo) = temp_engine_repo("topology-run-exclusion");
    assert!(
        !repo.join(".upstroke").exists(),
        "the premise: the repository has no exclusion before its first run"
    );
    let source = SharedTopologyStubs(std::sync::Arc::new(TopologyStub::new(
        TOPOLOGY_STUB_EDIT,
        0,
    )));
    let report = run_with(&topology_options(&repo), &source).expect("the schema-4 run completes");
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:#?}");
    until_released(&repo, &report.run_id);
    let generated: Vec<String> = fs::read_dir(rundir::public_dir(&repo, &report.run_id))
        .expect("the run's public directory")
        .map(|entry| {
            entry
                .expect("a public entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert!(
        ["events.jsonl", "report.json"]
            .iter()
            .all(|name| generated.iter().any(|file| file == name)),
        "the premise: the run generated its public files: {generated:?}"
    );
    let listed = status_of(&repo);
    assert!(
        listed.trim().is_empty(),
        "`git status` lists none of the run's files: {listed}"
    );
    git_in(&repo, &["add", "-A"]);
    let staged = git_in(&repo, &["diff", "--cached", "--name-only"]);
    assert!(
        staged.trim().is_empty(),
        "an ordinary `git add -A` stages none of them: {staged}"
    );
}

/// L4, the resume. A schema-4 resume makes the run exclusion, as the sequential resume does, so
/// a run whose `.upstroke/.gitignore` went missing while it was parked is ignored by Git again
/// once it is resumed.
#[test]
fn a_schema_4_resume_restores_a_missing_run_exclusion() {
    let (_tree, repo) = temp_engine_repo("topology-resume-exclusion");
    let source = SharedTopologyStubs(std::sync::Arc::new(TopologyStub::new(
        TOPOLOGY_STUB_EDIT,
        1,
    )));
    let parked = run_with(&topology_options(&repo), &source).expect("the run parks");
    assert_eq!(parked.outcome(), RunOutcome::Parked, "{parked:#?}");
    until_released(&repo, &parked.run_id);
    let [asked] = parked.questions.as_slice() else {
        panic!("one question: {parked:#?}");
    };
    crate::answer::answer(
        &repo,
        &asked.question.id.to_string(),
        crate::answer::Reply::Text("the first".to_owned()),
    )
    .expect("the answer is recorded");
    let exclusion = repo.join(".upstroke").join(".gitignore");
    fs::remove_file(&exclusion).expect("the fresh run made the exclusion");
    let exposed = status_of(&repo);
    assert!(
        exposed.contains(".upstroke/"),
        "the premise: without the exclusion the run's files are exposed: {exposed}"
    );

    let mut resume = resume_options(&repo, &parked.run_id);
    resume.private_root = None;
    let report = resume_with(&resume, &source).expect("the resume completes the run");
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:#?}");
    until_released(&repo, &parked.run_id);
    assert!(exclusion.exists(), "the resume made the exclusion again");
    let listed = status_of(&repo);
    assert!(
        listed.trim().is_empty(),
        "`git status` lists none of the run's files after the resume: {listed}"
    );
}

#[test]
fn a_budget_stopped_schema_4_resume_recreates_its_pruned_execution_root_where_it_recorded_it() {
    let (_tree, repo) = temp_engine_repo("topology-pruned-root");
    seed(&repo, ONE_ATTEMPT_PLAN, Some(ONE_ATTEMPT_ROUTING));
    let source = SharedTopologyStubs(std::sync::Arc::new(TopologyStub::new(
        TOPOLOGY_STUB_EDIT,
        0,
    )));
    let mut opts = topology_options(&repo);
    opts.budget_usd = Some(0.015);
    let stopped = run_with(&opts, &source).expect("the run ends");
    assert_eq!(
        stopped.outcome(),
        RunOutcome::BudgetExceeded,
        "{stopped:#?}"
    );
    until_released(&repo, &stopped.run_id);
    let recorded = recorded_execution_root(&repo, &stopped.run_id);
    assert!(
        fs::symlink_metadata(&recorded).is_err(),
        "the premise: finalization pruned the stopped run's emptied root"
    );
    let namespaces = execution_namespaces(&repo);

    let mut resume = resume_options(&repo, &stopped.run_id);
    resume.private_root = None;
    resume.budget_usd = Some(1.0);
    let resumed = resume_with(&resume, &source).expect("a budget-stopped run resumes");
    until_released(&repo, &stopped.run_id);
    assert!(
        resumed.warnings.iter().any(|warning| warning.contains(
            "the execution root was pruned when the run last ended and has been recreated"
        )),
        "the resume recreated the pruned root: {:?}",
        resumed.warnings
    );
    assert_eq!(
        execution_namespaces(&repo),
        namespaces,
        "the resume made no other namespace"
    );
}

const ONE_ATTEMPT_ROUTING: &str =
    "[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n";

fn settled_attempts(events: &[serde_json::Value]) -> usize {
    events
        .iter()
        .filter(|event| {
            event["event"] == "attempt_finished"
                || event["event"] == "candidate_prepared"
                || event["event"] == "attempt_interrupted"
        })
        .count()
}

/// The display id finalize's projection gives a task key.
fn display_of(projected: &crate::engine::topology::report::TopologyReport, key: u64) -> String {
    projected
        .tasks
        .iter()
        .find(|task| u64::from(task.key) == key)
        .map(|task| task.display_id.clone())
        .unwrap_or_else(|| panic!("no task k{key} in the projection"))
}

/// S9, a halted run. A task whose gate fails on its one attempt reaches the end of
/// its ladder, and the run asks before failing it: it parks on the task's
/// question (V109 found this, against the first draft's premise of a run that
/// halts at once). The operator gives the task up through the public `answer
/// --decline`, and under the default `on_task_failure = halt` the resume halts.
/// The `RunReport` that `engine::resume_with` returns projects what the fold
/// says: `outcome()` is `Halted`, `halted_at` names the task the log's
/// `run_finished` names, the task behind it is skipped, and nothing claims a
/// budget stop. A fresh observation of the log and finalize's `report.json` say
/// the same. `main.rs` turns this outcome into its exit status; the command-line
/// scenario's case E runs that.
#[cfg(unix)]
#[test]
fn a_halted_schema_4_run_projects_where_it_halted_and_its_outcome() {
    let (_tree, repo) = temp_engine_repo("topology-halted");
    seed(
        &repo,
        ONE_ATTEMPT_PLAN,
        Some(
            format!(
                "{ONE_ATTEMPT_ROUTING}\n[[gates]]\nname = \"refuses-the-edit\"\n\
                 cmd = \"test ! -e agent-output.txt\"\n"
            )
            .as_str(),
        ),
    );
    let source = SharedTopologyStubs(std::sync::Arc::new(TopologyStub::new(
        TOPOLOGY_STUB_EDIT,
        0,
    )));
    let parked = run_with(&topology_options(&repo), &source).expect("the run ends");
    assert_eq!(
        parked.outcome(),
        RunOutcome::Parked,
        "the end of the ladder asks before the task fails: {parked:#?}"
    );
    until_released(&repo, &parked.run_id);
    let [asked] = parked.questions.as_slice() else {
        panic!("one question: {parked:#?}");
    };
    assert_eq!(
        asked.question.affected_tasks,
        vec![TaskId::from("t1")],
        "{asked:#?}"
    );
    crate::answer::answer(
        &repo,
        &asked.question.id.to_string(),
        crate::answer::Reply::Decline,
    )
    .expect("the task is given up");
    let mut resume = resume_options(&repo, &parked.run_id);
    resume.private_root = None;
    let report = resume_with(&resume, &source).expect("the resume ends");
    until_released(&repo, &report.run_id);

    let events = log_events(&repo, &report.run_id);
    let finished = events.last().expect("a log");
    assert_eq!(finished["event"], "run_finished", "{finished}");
    assert_eq!(finished["data"]["outcome"], "halted", "{finished}");
    let halted_key = finished["data"]["halted_at"]
        .as_u64()
        .expect("a halted run names the task it halted at");
    let projected = crate::engine::topology::observe::observe(&repo, &report.run_id)
        .expect("observes")
        .expect("a schema-4 log")
        .report()
        .expect("projects");
    let halted_task = display_of(&projected, halted_key);
    assert_eq!(
        halted_task, "t1",
        "the fixture's premise: the first task halts the run"
    );

    assert_eq!(report.outcome(), RunOutcome::Halted, "{report:#?}");
    assert_eq!(report.halted_at.as_deref(), Some(halted_task.as_str()));
    assert!(report.budget_stop.is_none(), "{report:#?}");
    let status = |id: &str| {
        &report
            .tasks
            .iter()
            .find(|task| task.id == id)
            .unwrap_or_else(|| panic!("task {id} in {report:#?}"))
            .status
    };
    assert!(
        matches!(
            status("t1"),
            TaskRunStatus::Failed {
                kind: FailureKind::GateFailed,
                ..
            }
        ),
        "{report:#?}"
    );
    assert!(
        matches!(status("t2"), TaskRunStatus::Skipped),
        "{report:#?}"
    );
    assert_eq!(
        report
            .tasks
            .iter()
            .map(|task| task.attempts.len())
            .sum::<usize>(),
        settled_attempts(&events),
        "every attempt the log settles is in the report: {report:#?}"
    );

    assert_eq!(projected.outcome, Some(crate::events::RunOutcome::Halted));
    assert_eq!(projected.halted_at.map(u64::from), Some(halted_key));
    assert!(projected.budget_stop.is_none());
    let finalized: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(rundir::public_dir(&repo, &report.run_id).join("report.json"))
            .expect("finalize's report"),
    )
    .expect("json");
    assert_eq!(finalized["outcome"], "halted", "{finalized}");
    assert_eq!(finalized["halted_at"], halted_key, "{finalized}");
}

fn blocked_closure_plan() -> String {
    format!(
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n\
         Its workers below the frontier end without an edit: {ESCALATES}.\n\n\
         ## Implement the gadget\n<!-- upstroke: id=t2 kind=implement depends=t1 -->\n\
         Make it on top of the widget.\n\n\
         ## Implement the gizmo\n<!-- upstroke: id=t3 kind=implement depends=t2 -->\n\
         Make it on top of the gadget.\n\n\
         ## Implement the sprocket\n<!-- upstroke: id=t4 kind=implement depends= -->\n\
         Make it on its own.\n"
    )
}

fn dependent_rows(rendered: &str) -> Vec<&str> {
    rendered
        .lines()
        .filter(|line| line.starts_with("  t2:") || line.starts_with("  t3:"))
        .collect()
}

fn declined_failure_run(repo: &Path, stub: &ObservedStub) -> RunReport {
    let answers = ScriptedAnswers::new(vec![Answer::Declined]);
    let report = run_harness(
        &topology_options(repo),
        &Harness {
            adapters: &BorrowedObserved(stub),
            answers: Some(&answers),
            sleeper: None,
        },
    )
    .expect("the run ends");
    until_released(repo, &report.run_id);
    report
}

#[test]
fn a_completed_schema_4_run_reports_its_failed_tasks_dependents_blocked_live_and_replayed() {
    use crate::topology::fold::{FrozenInputs, TaskState as Structural, TopologyFold};

    let (_tree, repo) = temp_engine_repo("topology-blocked-closure");
    seed(
        &repo,
        &blocked_closure_plan(),
        Some(&format!(
            "[engine]\non_task_failure = \"continue\"\n\n{ONE_ATTEMPT_ROUTING}"
        )),
    );
    let stub = ObservedStub::new(0);
    let report = declined_failure_run(&repo, &stub);
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:#?}");
    assert!(
        matches!(task(&report, "t1").status, TaskRunStatus::Failed { .. }),
        "the premise: t1 failed: {report:#?}"
    );
    assert!(
        committed(&report, "t4"),
        "the premise: the independent task merged: {report:#?}"
    );
    let rendered = report.render();
    assert_eq!(
        dependent_rows(&rendered),
        ["  t2: blocked by `t1`", "  t3: blocked by `t2`"],
        "the failed t1's direct and transitive dependents, as the completed run's final summary \
         prints them:\n{rendered}"
    );
    for (id, by) in [("t2", "t1"), ("t3", "t2")] {
        assert!(
            matches!(&task(&report, id).status, TaskRunStatus::Blocked { by: blocker } if blocker == by),
            "{id} is blocked by {by}: {report:#?}"
        );
    }
    assert!(
        rendered.contains(&format!(
            "run complete: 1 task(s) committed on {}",
            report.branch
        )),
        "{rendered}"
    );

    let log = fs::read(rundir::public_dir(&repo, &report.run_id).join(rundir::EVENT_LOG))
        .expect("the run's log");
    let events = TopologyFold::parse_log(&log).expect("the committed log parses");
    let frozen = fs::read(paths_of(&repo, &report.run_id).plan_json()).expect("the frozen plan");
    let inputs = || FrozenInputs {
        plan: serde_json::from_slice(&frozen).expect("the frozen plan parses"),
        normalized_plan_digest: crate::events::normalized_plan_digest(&frozen),
    };
    let replay = |end: usize| {
        TopologyFold::replay(inputs(), events.get(..end).unwrap_or_default()).unwrap_or_else(
            |error| panic!("the log's first {end} event(s) replay through the fold: {error}"),
        )
    };
    let replayed = replay(events.len());
    assert_eq!(
        replayed.finished(),
        Some(&crate::events::RunOutcome::Complete)
    );
    let key = |id: &str| {
        replayed
            .registry()
            .and_then(|registry| registry.key_of(id))
            .unwrap_or_else(|| panic!("task {id} in the registry"))
    };
    assert_eq!(
        ["t1", "t2", "t3", "t4"].map(|id| replayed.task_state(key(id))),
        [
            Some(Structural::Failed),
            Some(Structural::Pending),
            Some(Structural::Pending),
            Some(Structural::Merged),
        ],
        "the durable states the projection reads: t2 and t3 stay Pending"
    );
    let projected = RunReport::from_topology(&replayed, &events, Vec::new()).render();
    assert_eq!(
        dependent_rows(&projected),
        dependent_rows(&rendered),
        "the replayed log's projection:\n{projected}"
    );
    let finalized: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(rundir::public_dir(&repo, &report.run_id).join("report.json"))
            .expect("finalize's report"),
    )
    .expect("json");
    for id in ["t2", "t3"] {
        let state = finalized["tasks"]
            .as_array()
            .and_then(|tasks| tasks.iter().find(|task| task["display_id"] == id))
            .map(|task| task["state"].clone());
        assert_eq!(
            state,
            Some(serde_json::Value::from("pending")),
            "the run's own report keeps {id}'s durable state: {finalized}"
        );
    }

    let fails_t1 = (1..=events.len())
        .find(|&end| replay(end).task_state(key("t1")) == Some(Structural::Failed))
        .expect("an event of the log fails t1");
    let before = RunReport::from_topology(
        &replay(fails_t1 - 1),
        events.get(..fails_t1 - 1).unwrap_or_default(),
        Vec::new(),
    )
    .render();
    assert_eq!(
        dependent_rows(&before),
        ["  t2: queued", "  t3: queued"],
        "before the event that fails t1, its dependents wait on a task that has not failed:\n{before}"
    );
    let at = RunReport::from_topology(
        &replay(fails_t1),
        events.get(..fails_t1).unwrap_or_default(),
        Vec::new(),
    )
    .render();
    assert_eq!(
        dependent_rows(&at),
        ["  t2: blocked by `t1`", "  t3: blocked by `t2`"],
        "the event that fails t1 is the one that blocks its dependents:\n{at}"
    );
}

#[test]
fn a_schema_4_report_keeps_queued_behind_a_parked_task_and_skipped_behind_a_halting_failure() {
    let (_parked_tree, parked_repo) = temp_engine_repo("topology-queued-parked");
    let asking = ObservedStub::new(1);
    let parked = run_with(&topology_options(&parked_repo), &BorrowedObserved(&asking))
        .expect("the run parks");
    until_released(&parked_repo, &parked.run_id);
    assert_eq!(parked.outcome(), RunOutcome::Parked, "{parked:#?}");
    assert!(
        matches!(task(&parked, "t1").status, TaskRunStatus::Parked { .. }),
        "the premise: t1 is parked on its question: {parked:#?}"
    );
    let rendered = parked.render();
    assert_eq!(
        dependent_rows(&rendered),
        ["  t2: queued"],
        "t2 waits on a question, not behind a failure:\n{rendered}"
    );

    let (_halted_tree, halted_repo) = temp_engine_repo("topology-skipped-halted");
    seed(
        &halted_repo,
        &blocked_closure_plan(),
        Some(ONE_ATTEMPT_ROUTING),
    );
    let stub = ObservedStub::new(0);
    let halted = declined_failure_run(&halted_repo, &stub);
    assert_eq!(halted.outcome(), RunOutcome::Halted, "{halted:#?}");
    assert_eq!(halted.halted_at.as_deref(), Some("t1"), "{halted:#?}");
    let rendered = halted.render();
    assert_eq!(
        dependent_rows(&rendered),
        ["  t2: skipped (run halted)", "  t3: skipped (run halted)"],
        "a halted run skips the failed task's dependents, as it always has:\n{rendered}"
    );
}

/// S9, a budget stop. A run budget the first task's spend crosses stops the run
/// before the next attempt. The `RunReport` projects the stop the log records:
/// `outcome()` is `BudgetExceeded`, and `budget_stop` carries the log's
/// `budget_exceeded` budget, limit and spend, and the task it stopped at, by its
/// display id. Nothing claims a halt, and a fresh observation of the log and
/// finalize's `report.json` say the same. `main.rs`'s exit status 3 is the
/// command-line scenario's case F.
#[test]
fn a_schema_4_run_stopped_at_its_budget_projects_the_stop_and_its_outcome() {
    let (_tree, repo) = temp_engine_repo("topology-budget");
    seed(&repo, ONE_ATTEMPT_PLAN, Some(ONE_ATTEMPT_ROUTING));
    let source = SharedTopologyStubs(std::sync::Arc::new(TopologyStub::new(
        TOPOLOGY_STUB_EDIT,
        0,
    )));
    let mut opts = topology_options(&repo);
    opts.budget_usd = Some(0.015);
    let report = run_with(&opts, &source).expect("the run ends");
    until_released(&repo, &report.run_id);

    let events = log_events(&repo, &report.run_id);
    let finished = events.last().expect("a log");
    assert_eq!(finished["event"], "run_finished", "{finished}");
    assert_eq!(finished["data"]["outcome"], "budget_exceeded", "{finished}");
    let stops: Vec<&serde_json::Value> = events
        .iter()
        .filter(|event| event["event"] == "budget_exceeded")
        .collect();
    let [exceeded] = stops.as_slice() else {
        panic!("one budget_exceeded event: {events:#?}");
    };
    let data = &exceeded["data"];
    assert_eq!(data["budget"], "run", "{data}");
    let projected = crate::engine::topology::observe::observe(&repo, &report.run_id)
        .expect("observes")
        .expect("a schema-4 log")
        .report()
        .expect("projects");
    let stopped_at = data["key"]
        .as_u64()
        .map(|key| display_of(&projected, key))
        .unwrap_or_default();

    assert_eq!(report.outcome(), RunOutcome::BudgetExceeded, "{report:#?}");
    assert_eq!(
        report.budget_stop,
        Some(crate::events::BudgetExceeded {
            budget: crate::events::BudgetKind::Run,
            limit_usd: data["limit_usd"].as_f64().expect("a limit"),
            spent_usd: data["spent_usd"].as_f64().expect("a spend"),
            task: stopped_at,
        }),
        "{data}"
    );
    assert!(
        (data["limit_usd"].as_f64().expect("a limit") - 0.015).abs() < 1e-9,
        "the limit is the one the run was given: {data}"
    );
    assert!(
        data["spent_usd"].as_f64().expect("a spend") > 0.0,
        "the fixture's premise: the first task spent before the stop: {data}"
    );
    assert!(report.halted_at.is_none(), "{report:#?}");
    assert_eq!(
        report
            .tasks
            .iter()
            .map(|task| task.attempts.len())
            .sum::<usize>(),
        settled_attempts(&events),
        "every attempt the log settles is in the report, successful ones included: \
         {report:#?}"
    );
    assert!(
        (report.total_cost_usd - data["spent_usd"].as_f64().expect("a spend")).abs() < 1e-9,
        "the report's total is the spend the stop records: {} against {data}",
        report.total_cost_usd
    );
    for task in report
        .tasks
        .iter()
        .filter(|task| matches!(task.status, TaskRunStatus::Committed { .. }))
    {
        assert!(
            !task.model.is_empty() && task.cost_usd.is_some(),
            "a committed task carries its successful attempt: {task:#?}"
        );
    }
    assert_eq!(
        projected.outcome,
        Some(crate::events::RunOutcome::BudgetExceeded)
    );
    assert_eq!(
        projected.budget_stop.map(|stop| stop.budget),
        Some(crate::events::BudgetKind::Run)
    );
    assert!(projected.halted_at.is_none());
    let finalized: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(rundir::public_dir(&repo, &report.run_id).join("report.json"))
            .expect("finalize's report"),
    )
    .expect("json");
    assert_eq!(finalized["outcome"], "budget_exceeded", "{finalized}");
    assert_eq!(finalized["budget_stop"]["budget"], "run", "{finalized}");
}

/// S9, a task-budget stop. `[budgets] task_usd` refuses a task's retry once the
/// task's own spend reaches it, and the log's `budget_exceeded` names that
/// task's key. The `RunReport` projects the stop to the task's display id. The
/// run-wide stop above, refused at integration, names no task and projects to
/// an empty one; both semantics stand side by side.
///
/// The gate fails the first candidate that carries the stub's output, once,
/// so the task's first attempt is spent and its retry is what the task budget
/// refuses.
#[cfg(unix)]
#[test]
fn a_task_budget_stop_projects_the_task_it_names() {
    let (tree, repo) = temp_engine_repo("topology-task-budget");
    let failed_once = tree.path().join("gate-failed-once");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\nMake it.\n",
        Some(
            format!(
                "[routing]\nimplement = {{ chain = [\"small\"], attempts_per = 2 }}\n\n\
                 [budgets]\ntask_usd = 0.01\n\n\
                 [[gates]]\nname = \"fails-once\"\n\
                 cmd = \"if [ -e agent-output.txt ] && [ ! -e '{}' ]; then : > '{}'; exit 1; fi\"\n",
                failed_once.display(),
                failed_once.display()
            )
            .as_str(),
        ),
    );
    let source = SharedTopologyStubs(std::sync::Arc::new(TopologyStub::new(
        TOPOLOGY_STUB_EDIT,
        0,
    )));
    let report = run_with(&topology_options(&repo), &source).expect("the run ends");
    until_released(&repo, &report.run_id);
    assert!(
        failed_once.exists(),
        "the fixture's premise: the gate failed the first attempt"
    );

    let events = log_events(&repo, &report.run_id);
    let stops: Vec<&serde_json::Value> = events
        .iter()
        .filter(|event| event["event"] == "budget_exceeded")
        .collect();
    let [exceeded] = stops.as_slice() else {
        panic!("one budget_exceeded event: {events:#?}");
    };
    let data = &exceeded["data"];
    assert_eq!(data["budget"], "task", "{data}");
    let key = data["key"]
        .as_u64()
        .expect("a task-budget stop names the task it refused");
    let projected = crate::engine::topology::observe::observe(&repo, &report.run_id)
        .expect("observes")
        .expect("a schema-4 log")
        .report()
        .expect("projects");
    let task = display_of(&projected, key);
    assert_eq!(
        task, "t1",
        "the fixture's premise: the one task's retry was refused"
    );

    assert_eq!(report.outcome(), RunOutcome::BudgetExceeded, "{report:#?}");
    assert_eq!(
        report.budget_stop,
        Some(crate::events::BudgetExceeded {
            budget: crate::events::BudgetKind::Task,
            limit_usd: data["limit_usd"].as_f64().expect("a limit"),
            spent_usd: data["spent_usd"].as_f64().expect("a spend"),
            task: task.clone(),
        }),
        "{data}"
    );
    assert!(
        report.render_ledger().contains(&format!("before `{task}`")),
        "the ledger names the task the stop was for: {}",
        report.render_ledger()
    );
    assert_eq!(
        projected.budget_stop.map(|stop| stop.budget),
        Some(crate::events::BudgetKind::Task)
    );
    assert_eq!(
        report
            .tasks
            .iter()
            .map(|task| task.attempts.len())
            .sum::<usize>(),
        settled_attempts(&events),
        "every attempt the log settles is in the report: {report:#?}"
    );
}

fn recorded_case_fold(repo: &Path, run_id: &str) -> (String, bool) {
    let line = fs::read_to_string(rundir::public_dir(repo, run_id).join("events.jsonl"))
        .expect("the run's event log")
        .lines()
        .next()
        .expect("a committed first line")
        .to_owned();
    let first: serde_json::Value = serde_json::from_str(&line).expect("an event");
    let fold = first["data"]["path_policy"]["case_fold"]
        .as_bool()
        .expect("run_started(4) records a case policy");
    (line, fold)
}

/// P12-W11 (D7). A public fresh run records the case-fold resolver's answer in
/// `run_started(4)`'s `path_policy`, and a resume never changes it, even where
/// the resolver would now answer differently. The managed repository's
/// `core.ignorecase` changes between each run's creation and its resume, and a
/// fresh run made after each change shows what the resolver answers then.
///
/// The namespace is observed the way the resolver observes it (one name, looked
/// up in the other case, beside the private root). Where it folds, every answer
/// is `true`, so only "a resume never changes it" can be shown there.
///
/// On a case-sensitive namespace, the run that recorded `true` resumes over a
/// root that does not fold. The resume continues on the record and warns (D7's
/// resume side), while the run that recorded `false` resumes without a word.
#[test]
fn a_fresh_run_records_the_case_policy_and_a_resume_never_changes_it() {
    let (tree, repo) = temp_engine_repo("topology-case-record");
    let probe = tree.path().join("case-probe");
    fs::write(&probe, b"lower case\n").expect("the case probe");
    let folds_here = tree.path().join("CASE-PROBE").exists();
    let answer = |run: &RunReport| {
        let [asked] = run.questions.as_slice() else {
            panic!("one question: {run:#?}");
        };
        crate::answer::answer(
            &repo,
            &asked.question.id.to_string(),
            crate::answer::Reply::Text("the first".to_owned()),
        )
        .expect("the answer is recorded");
    };
    let resumed = |run: &RunReport, stub: &TopologyStub| {
        let mut resume = resume_options(&repo, &run.run_id);
        resume.private_root = None;
        let report = resume_with(&resume, &BorrowedTopologyStub(stub)).expect("the resume");
        assert_eq!(report.outcome(), RunOutcome::Complete, "{report:#?}");
        until_released(&repo, &run.run_id);
        report
    };
    let warns_of_a_sensitive_root = |report: &RunReport| {
        report.warnings.iter().any(|warning| {
            warning.contains("recorded `case_fold = true`")
                && warning.contains("case-sensitive now")
        })
    };
    let parked = |stub: &TopologyStub| {
        let run =
            run_with(&topology_options(&repo), &BorrowedTopologyStub(stub)).expect("the run parks");
        assert_eq!(run.outcome(), RunOutcome::Parked, "{run:#?}");
        until_released(&repo, &run.run_id);
        run
    };

    git_in(&repo, &["config", "core.ignorecase", "false"]);
    let sensitive_stub = TopologyStub::new(TOPOLOGY_STUB_EDIT, 1);
    let recorded_off = parked(&sensitive_stub);
    let (off_line, off_fold) = recorded_case_fold(&repo, &recorded_off.run_id);
    assert_eq!(
        off_fold, folds_here,
        "with `core.ignorecase = false` the record is the namespace's own answer"
    );

    git_in(&repo, &["config", "core.ignorecase", "true"]);
    let union_stub = TopologyStub::new(TOPOLOGY_STUB_EDIT, 1);
    let recorded_on = parked(&union_stub);
    let (on_line, on_fold) = recorded_case_fold(&repo, &recorded_on.run_id);
    assert!(on_fold, "`core.ignorecase = true` joins the union");
    if !folds_here {
        assert_ne!(
            off_fold, on_fold,
            "the premise on a case-sensitive namespace: the resolver now answers differently"
        );
    }

    answer(&recorded_off);
    let resumed_off = resumed(&recorded_off, &sensitive_stub);
    assert!(
        !warns_of_a_sensitive_root(&resumed_off),
        "a record of `false` that the root agrees with says nothing: {:?}",
        resumed_off.warnings
    );
    assert_eq!(
        recorded_case_fold(&repo, &recorded_off.run_id).0,
        off_line,
        "the resume under `core.ignorecase = true` left the record byte for byte"
    );
    let observed = crate::engine::topology::observe::observe(&repo, &recorded_off.run_id)
        .expect("observes")
        .expect("a schema-4 log");
    assert_eq!(
        observed
            .started()
            .expect("a started run")
            .path_policy
            .case_fold,
        off_fold,
        "the resumed run's fold reads the recorded policy"
    );

    git_in(&repo, &["config", "core.ignorecase", "false"]);
    let today_stub = TopologyStub::new(TOPOLOGY_STUB_EDIT, 0);
    let today = run_with(&topology_options(&repo), &BorrowedTopologyStub(&today_stub))
        .expect("a fresh run today");
    until_released(&repo, &today.run_id);
    assert_eq!(
        recorded_case_fold(&repo, &today.run_id).1,
        folds_here,
        "with `core.ignorecase = false` again, a fresh run records the namespace's answer"
    );
    answer(&recorded_on);
    let resumed_on = resumed(&recorded_on, &union_stub);
    assert_eq!(
        warns_of_a_sensitive_root(&resumed_on),
        !folds_here,
        "a record of `true` over a root that does not fold continues and warns: {:?}",
        resumed_on.warnings
    );
    assert_eq!(
        recorded_case_fold(&repo, &recorded_on.run_id).0,
        on_line,
        "the resume under `core.ignorecase = false` left the record byte for byte"
    );
}

/// FINDINGS2_3, the committer-identity parity check (root's checkpoint-3
/// disposition). At `b9d15bdb` the legacy engine asked the operator for no
/// identity: every commit it put on a run's branch was `commit-tree` with
/// `upstroke`'s (`Workspace::commit_tree_with_upstroke_identity`). The schema-4
/// integration's proposal cherry-pick took the committer from the operator's
/// configuration, so a repository without one stopped there (the CLI
/// scenario's case C at `ad513bdf`). That was an activation regression, and the
/// cherry-pick now carries the engine's committer identity.
///
/// The child runs with every ambient Git control taken away. In it, the
/// repository's own identity is removed and `user.useConfigOnly` is set, so Git
/// can find no committer identity at all, which is checked first. Two
/// independent tasks at width 2 make the second merge a stale candidate's
/// cherry-pick, which is checked too.
#[cfg(unix)]
#[test]
fn a_schema_4_run_integrates_where_git_has_no_committer_identity() {
    let tree = temp_engine_scratch("no-committer-identity");
    let marker = tree.path().join("ran");
    let status = crate::workspace_manager::fixture::run_challenged_replacement_witness_child(
        "engine::tests::no_committer_identity_helper",
        &[(NO_IDENTITY_MARKER, marker.clone().into_os_string())],
    );
    assert!(
        status.success(),
        "a schema-4 run in a repository and environment without a committer identity, \
         ended {status:?}"
    );
    assert!(
        marker.exists(),
        "the child never reached the helper, so it measured nothing"
    );
}

/// Three tasks in flight from one base. The two in slots `k0` and `k1` both
/// create `shared.txt` with different contents, and the one in `k2` writes a file
/// of its own: whichever merges first, a later candidate is stale and clean (a
/// merge verification) and another conflicts (a merge rejection, which spawns a
/// merge-repair task).
#[cfg(unix)]
const THREE_INDEPENDENT_TASKS: &str = "## Implement the widget\n\
                                       <!-- upstroke: id=t1 depends= paths=widget/** -->\n\
                                       Make it.\n\n## Implement the gadget\n\
                                       <!-- upstroke: id=t2 depends= paths=gadget/** -->\n\
                                       Make that.\n\n## Implement the gizmo\n\
                                       <!-- upstroke: id=t3 depends= paths=gizmo/** -->\n\
                                       Make the third.\n";

#[cfg(unix)]
const CONFLICTING_WORKER: &str = "case \"$(basename \"$PWD\")\" in k2-*) f=own.txt ;; *) f=shared.txt ;; \
                                  esac; sleep 1; printf 'written by %s\\n' \"$$\" > \"$f\"";

/// P12-W18. `export-decisions` over a schema-4 run with a merge-repair task:
/// one row per attempt, in today's columns, the repair task's attempts joined to
/// the frozen registry by its display id, and no verification or merge event as
/// a row. Each row is settled as the log settles its attempt: one that
/// succeeded, by `candidate_prepared`, is `passed`, with its finish and cost.
#[cfg(unix)]
#[test]
fn a_schema_4_export_keeps_todays_rows_and_joins_a_repair_task_to_the_registry() {
    let (_tree, repo) = temp_engine_repo("topology-export");
    seed(
        &repo,
        THREE_INDEPENDENT_TASKS,
        Some("[engine]\nmax_parallel = 3\nmax_merge_repairs = 2\n"),
    );
    let source = SharedTopologyStubs(std::sync::Arc::new(TopologyStub::new(
        CONFLICTING_WORKER,
        0,
    )));
    let report = run_with(&topology_options(&repo), &source).expect("the run ends");
    until_released(&repo, &report.run_id);
    let log = fs::read_to_string(rundir::public_dir(&repo, &report.run_id).join("events.jsonl"))
        .expect("the log");
    let kinds: Vec<String> = log
        .lines()
        .map(|line| {
            serde_json::from_str::<serde_json::Value>(line).expect("an event")["event"]
                .as_str()
                .unwrap_or_default()
                .to_owned()
        })
        .collect();
    assert!(
        kinds.iter().any(|kind| kind == "merge_rejected"),
        "the fixture's premise: a candidate's merge was rejected ({kinds:?})"
    );
    assert!(
        kinds
            .iter()
            .any(|kind| kind == "merge_verification_started"),
        "and verifications ran ({kinds:?})"
    );
    let starts = kinds
        .iter()
        .filter(|kind| *kind == "attempt_started")
        .count();

    let loaded = crate::export::load(&repo, &report.run_id).expect("the schema-4 export loads");
    assert_eq!(
        loaded.rows.len(),
        starts,
        "one row per attempt, and nothing else becomes a row"
    );
    let mut jsonl = Vec::new();
    crate::export::write(&loaded.rows, crate::export::Format::Jsonl, &mut jsonl).expect("jsonl");
    let rows: Vec<serde_json::Value> = String::from_utf8(jsonl)
        .expect("utf8")
        .lines()
        .map(|line| serde_json::from_str(line).expect("a row"))
        .collect();
    let task_ids: Vec<&str> = rows
        .iter()
        .map(|row| row["task_id"].as_str().unwrap_or_default())
        .collect();
    assert!(
        task_ids
            .iter()
            .any(|id| *id != "t1" && *id != "t2" && *id != "t3"),
        "a repair task's attempts are rows, joined by its display id: {task_ids:?}"
    );
    let passed = kinds
        .iter()
        .filter(|kind| *kind == "candidate_prepared")
        .count();
    let settled_as = |outcome: &str| {
        rows.iter()
            .filter(|row| row["outcome"] == outcome)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        settled_as("passed").len(),
        passed,
        "every attempt the log settles by `candidate_prepared` is exported as passed: {rows:#?}"
    );
    assert!(
        settled_as("interrupted").is_empty(),
        "the log interrupts no attempt, and a finished run leaves none dangling: {rows:#?}"
    );
    for row in settled_as("passed") {
        assert!(
            row["attempt_finished_at"].is_string() && row["cost_usd"].is_number(),
            "a passed row carries its settlement's finish and cost: {row}"
        );
    }
    let keys: Vec<String> = rows[0]
        .as_object()
        .expect("a row is an object")
        .keys()
        .cloned()
        .collect();
    for row in &rows {
        let these: Vec<String> = row
            .as_object()
            .expect("a row is an object")
            .keys()
            .cloned()
            .collect();
        assert_eq!(these, keys, "every row has today's columns");
    }
    let mut csv = Vec::new();
    crate::export::write(&loaded.rows, crate::export::Format::Csv, &mut csv).expect("csv");
    let csv = String::from_utf8(csv).expect("utf8");
    assert!(
        csv.starts_with("schema_version,run_id,upstroke_version,run_started_at,"),
        "{csv}"
    );
    assert_eq!(
        csv.lines().count(),
        starts + 1,
        "a header and one line per attempt"
    );
}

/// J3, `PR12-REPORT-OMITS-INTEGRATION-REVIEW-SPEND` (both reviews). Three tasks in flight from
/// one base: a later candidate is stale and clean, so its integration verification runs a
/// review the run's spend ledger charges. At the reviewed head the CLI report collected
/// attempt records only and its total missed that charge ($0.10 against $0.11 recorded; the
/// second witness, $0.0400 against $0.0500). The report now carries those passes on the
/// candidate's task, once, and its total, and the tasks' own totals, are the ledger's, with no
/// mismatch warning.
#[cfg(unix)]
#[test]
fn a_schema_4_report_counts_every_integration_review_once() {
    let (_tree, repo) = temp_engine_repo("topology-integration-spend");
    seed(
        &repo,
        THREE_INDEPENDENT_TASKS,
        Some("[engine]\nmax_parallel = 3\nmax_merge_repairs = 2\n"),
    );
    let source = SharedTopologyStubs(std::sync::Arc::new(TopologyStub::new(
        CONFLICTING_WORKER,
        0,
    )));
    let report = run_with(&topology_options(&repo), &source).expect("the run ends");
    until_released(&repo, &report.run_id);
    let log = fs::read(rundir::public_dir(&repo, &report.run_id).join("events.jsonl"))
        .expect("the run's log");
    let events = crate::topology::fold::TopologyFold::parse_log(&log).expect("the log parses");
    let ledger = crate::engine::topology::select::Spend::replay(&events).run_total();
    let integration = crate::engine::topology::select::IntegrationReviews::replay(&events);
    let integration_cost: f64 = integration.all().filter_map(|review| review.cost_usd).sum();
    assert!(
        integration_cost > 0.0,
        "the premise: an integration verification charged a review"
    );
    assert!(
        (report.total_cost_usd - ledger).abs() < 1e-9,
        "the report's total {} is the ledger's {ledger}",
        report.total_cost_usd
    );
    let tasks: f64 = report
        .tasks
        .iter()
        .filter_map(TaskReport::total_cost_usd)
        .sum();
    assert!(
        (tasks - ledger).abs() < 1e-9,
        "the tasks' own totals {tasks} add up to the ledger's {ledger}"
    );
    assert!(
        !report
            .warnings
            .iter()
            .any(|warning| warning.contains("differs from")),
        "no cost mismatch is reported: {:?}",
        report.warnings
    );
}

/// J7, PR12 regression review #4: a schema-4 resume runs the gates its run recorded and says
/// how today's config differs, as `resume_runs_the_gates_the_run_recorded_not_todays` pins for
/// the sequential engine (DESIGN §17). At the reviewed head the resume restored the recorded
/// gates without comparing them, so an edited gate passed unremarked. Today's config here edits
/// one gate's command and another's timeout, renames a third, and changes the shell: the
/// warning names every edit, and the recorded gates, not today's failing command, decide the
/// run.
#[test]
fn a_schema_4_resume_runs_the_recorded_gates_and_names_how_todays_differ() {
    let config = |shell: &str, check: &str, lint_secs: u64, third: &str| {
        format!(
            "[interaction]\nmode = \"never\"\n\n{shell}\
             [[gates]]\nname = \"check\"\ncmd = \"{check}\"\ntimeout_secs = 600\n\n\
             [[gates]]\nname = \"lint\"\ncmd = \"git --version\"\ntimeout_secs = {lint_secs}\n\n\
             [[gates]]\nname = \"{third}\"\ncmd = \"git --version\"\ntimeout_secs = 600\n"
        )
    };
    let (tree, repo) = temp_engine_repo("topology-gates-recorded");
    seed(
        &repo,
        WORKER_ONLY_PLAN,
        Some(&config("", "git --version", 600, "old")),
    );
    let source = SharedTopologyStubs(std::sync::Arc::new(TopologyStub::new(
        TOPOLOGY_STUB_EDIT,
        1,
    )));
    let parked = run_with(&topology_options(&repo), &source).expect("the run parks");
    assert_eq!(parked.outcome(), RunOutcome::Parked, "{parked:#?}");
    until_released(&repo, &parked.run_id);
    let [asked] = parked.questions.as_slice() else {
        panic!("one question: {parked:#?}");
    };
    crate::answer::answer(
        &repo,
        &asked.question.id.to_string(),
        crate::answer::Reply::Text("the first".to_owned()),
    )
    .expect("the answer is recorded");

    let today = tree.path().join("today.toml");
    fs::write(
        &today,
        config(
            "[engine]\nshell = \"pwsh\"\n\n",
            "git frobnicate-not-a-command",
            900,
            "new",
        ),
    )
    .expect("today's config");
    let mut resume = resume_options(&repo, &parked.run_id);
    resume.private_root = None;
    resume.config_path = Some(today);
    let resumed = resume_with(&resume, &source).expect("the resume completes the run");

    assert_eq!(resumed.outcome(), RunOutcome::Complete, "{resumed:#?}");
    assert!(
        committed(&resumed, "t1"),
        "the recorded `check` ran, not today's failing command: {resumed:#?}"
    );
    assert_eq!(resumed.gates, ["check", "lint", "old"]);
    let warning = resumed
        .warnings
        .iter()
        .find(|warning| warning.contains("the gates in today's config differ"))
        .unwrap_or_else(|| panic!("no gate-difference warning: {:?}", resumed.warnings));
    let native = crate::gates::ShellKind::native().program();
    for edit in [
        format!(
            "`check` runs `git --version` and today's config says `git frobnicate-not-a-command`, \
             and runs under `{native}` and today's config says `pwsh`"
        ),
        format!(
            "`lint` runs under `{native}` and today's config says `pwsh`, and has 600s to finish \
             and today's config allows 900s"
        ),
        "`old` (`git --version`) is in the record and not in today's config".to_owned(),
        "`new` (`git --version`) is in today's config and not in the record".to_owned(),
        "Start a new run to adopt them".to_owned(),
    ] {
        assert!(warning.contains(&edit), "names `{edit}`: {warning}");
    }
}
