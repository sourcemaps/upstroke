//! Extended notes: `docs/internals/engine/tests/topology_admission.md`
// Allowlist placement: the funnel section of `effects/allowlist.toml`.
#![allow(clippy::disallowed_methods, clippy::disallowed_types)]
#![forbid(clippy::disallowed_macros)]

#[cfg(unix)]
use super::legacy_resume::strip_run_started_field;
use super::*;

/// P12-W3, the dispatch matrix on the resume path: schemas 1 to 3 are legacy, 4 is
/// the topology's, 5 refuses as newer before any lock, and a log whose header did
/// not commit is no run at all.
#[test]
fn a_resume_selects_its_reader_by_the_logs_header_and_refuses_before_any_lock() {
    use crate::engine::topology::entry::route_resume;

    let (_tree, repo) = temp_engine_repo("topology-dispatch-matrix");
    let legacy = run_with(&options(&repo), &fake(Effect::EditFile)).expect("a legacy run");
    let legacy_log = rundir::public_dir(&repo, &legacy.run_id).join("events.jsonl");
    let original = fs::read_to_string(&legacy_log).expect("the legacy log");
    assert!(
        original
            .lines()
            .next()
            .is_some_and(|line| line.contains("\"schema\":3")),
        "the legacy writer wrote schema 3"
    );
    for schema in [1, 2, 3] {
        let (first, rest) = original.split_once('\n').expect("a first line");
        let mut header: serde_json::Value = serde_json::from_str(first).expect("the header");
        header["data"]["schema"] = serde_json::json!(schema);
        fs::write(
            &legacy_log,
            format!(
                "{}\n{rest}",
                serde_json::to_string(&header).expect("a header")
            ),
        )
        .expect("a schema-{schema} header");
        assert!(
            route_resume(&resume_options(&repo, &legacy.run_id))
                .expect("a legacy header routes")
                .is_none(),
            "schema {schema} resumes on the legacy lines"
        );
    }
    fs::write(&legacy_log, &original).expect("the legacy log restored");

    let source = SharedTopologyStubs(std::sync::Arc::new(TopologyStub::new(
        TOPOLOGY_STUB_EDIT,
        0,
    )));
    let topology = run_with(&topology_options(&repo), &source).expect("a schema-4 run");
    until_released(&repo, &topology.run_id);
    let mut resume = resume_options(&repo, &topology.run_id);
    resume.private_root = None;
    assert!(
        route_resume(&resume)
            .expect("a schema-4 header routes")
            .is_some(),
        "schema 4 resumes on the topology's lines"
    );

    let log = rundir::public_dir(&repo, &topology.run_id).join("events.jsonl");
    let bytes = fs::read_to_string(&log).expect("the schema-4 log");
    let (first, rest) = bytes.split_once('\n').expect("a first line");
    let mut header: serde_json::Value = serde_json::from_str(first).expect("the header");
    header["data"]["schema"] = serde_json::json!(crate::topology::schema::MAX_READABLE_SCHEMA + 1);
    let newer = format!(
        "{}\n{rest}",
        serde_json::to_string(&header).expect("a header")
    );
    fs::write(&log, &newer).expect("a newer header");
    let held = WorktreeLock::acquire(&repo).expect("the held-lock control");
    let refused = resume_with(&resume, &source).expect_err("a newer schema refuses");
    drop(held);
    assert!(
        refused.to_string().contains("written by a newer upstroke")
            && !refused.to_string().contains("already driving worktree"),
        "with the worktree lock held, the newer-schema refusal came before any lock: {refused}"
    );
    assert_eq!(
        fs::read_to_string(&log).expect("the log"),
        newer,
        "the refused resume wrote nothing"
    );
    drop(WorktreeLock::acquire(&repo).expect("no-leak check: no worktree lock is left held"));
    fs::write(&log, &bytes).expect("the schema-4 log restored");

    let torn_id = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    let torn = rundir::public_dir(&repo, torn_id);
    fs::create_dir_all(&torn).expect("a run directory");
    fs::write(
        torn.join("events.jsonl"),
        "{\"ts\":\"2026-11-02T00:00:00Z\",\"ev",
    )
    .expect("a header that never committed");
    let held = WorktreeLock::acquire(&repo).expect("the held-lock control");
    let refused = resume_with(&resume_options(&repo, torn_id), &source)
        .expect_err("a log whose header did not commit is no run");
    drop(held);
    assert!(
        refused
            .to_string()
            .contains("never recorded a committed run_started")
            && refused.to_string().contains(torn_id)
            && !refused.to_string().contains("already driving worktree"),
        "with the worktree lock held, the uncommitted-header refusal came before any lock: \
         {refused}"
    );
    assert_eq!(
        fs::read_to_string(torn.join("events.jsonl")).expect("the torn log"),
        "{\"ts\":\"2026-11-02T00:00:00Z\",\"ev",
        "the refused resume changed the torn log"
    );
    drop(WorktreeLock::acquire(&repo).expect("no-leak check: no worktree lock is left held"));
}

/// The resume refusals `prepare_resume` owns, and its recorded-limits warning,
/// through `engine::resume_with` on a parked schema-4 run: a private half that is
/// not a directory refuses and is never created; a frozen plan whose digest no
/// longer matches the record refuses; and today's `max_parallel` that differs
/// from the record warns while the run keeps its own.
#[test]
fn a_schema_4_resume_refuses_a_missing_private_half_or_a_changed_plan_and_warns_on_limits() {
    let (tree, repo) = temp_engine_repo("topology-resume-refusals");
    let source = SharedTopologyStubs(std::sync::Arc::new(TopologyStub::new(
        TOPOLOGY_STUB_EDIT,
        1,
    )));
    let parked = run_with(&topology_options(&repo), &source).expect("the run parks");
    assert_eq!(parked.outcome(), RunOutcome::Parked, "{parked:#?}");
    until_released(&repo, &parked.run_id);
    let public = rundir::public_dir(&repo, &parked.run_id);
    let log = public.join("events.jsonl");
    let before = fs::read(&log).expect("the log");
    let mut resume = resume_options(&repo, &parked.run_id);
    resume.private_root = None;

    let private = private_dir_of(&repo, &parked.run_id);
    let away = private.with_extension("away");
    fs::rename(&private, &away).expect("the private half moved away");
    let refused = resume_with(&resume, &source).expect_err("no private half");
    assert!(
        refused
            .to_string()
            .contains("which is not a directory here"),
        "{refused}"
    );
    assert!(
        !private.exists(),
        "the resume created the missing private half"
    );
    assert_eq!(
        fs::read(&log).expect("the log"),
        before,
        "the refused resume wrote"
    );
    fs::rename(&away, &private).expect("the private half restored");

    let plan = public.join("plan.normalized.json");
    let frozen = fs::read(&plan).expect("the frozen plan");
    let mut altered = frozen.clone();
    altered.extend_from_slice(b" ");
    fs::write(&plan, &altered).expect("an altered frozen plan");
    let refused = resume_with(&resume, &source).expect_err("an altered frozen plan");
    assert!(refused.to_string().contains("digests to"), "{refused}");
    assert_eq!(
        fs::read(&log).expect("the log"),
        before,
        "the refused resume wrote"
    );
    fs::write(&plan, &frozen).expect("the frozen plan restored");

    let today = tree.path().join("today.toml");
    fs::write(&today, "[engine]\nmax_parallel = 3\n").expect("today's config");
    resume.config_path = Some(today);
    let resumed = resume_with(&resume, &source).expect("the resume runs");
    assert!(
        resumed
            .warnings
            .iter()
            .any(|warning| warning
                .contains("`max_parallel = 3` differs from the `max_parallel = 1`")),
        "{:?}",
        resumed.warnings
    );
}

/// PR12-CI-01 (P1) and, the same defect, PR12-RESUME-INCLUDE-BEFORE-RECORD-VERIFICATION
/// (INV-15, O18): through `engine::resume_with`, a parked schema-4 run whose private half is
/// present refuses before any private write when its owner or commit record is missing,
/// malformed or another's, or when its public locator names another conforming root, with the
/// include absent or changed; a provable run's resume still writes the include again.
#[test]
fn a_schema_4_resume_proves_its_owner_and_commit_records_before_it_writes_its_include() {
    let (tree, repo) = temp_engine_repo("topology-resume-include-after-proof");
    let witness = OwnerWitness {
        stub: TopologyStub::new(TOPOLOGY_STUB_EDIT, 1),
        private_root: private_root_for(&repo),
        owner_at_probe: Mutex::new(Vec::new()),
    };
    let source = BorrowedOwnerWitness(&witness);
    let probes = || {
        witness
            .owner_at_probe
            .lock()
            .expect("the probe witness")
            .len()
    };
    let parked = run_with(&topology_options(&repo), &source).expect("the run parks");
    assert_eq!(parked.outcome(), RunOutcome::Parked, "{parked:#?}");
    until_released(&repo, &parked.run_id);

    let home = private_root_for(&repo);
    let private = private_dir_of(&repo, &parked.run_id);
    let root = fs::canonicalize(&home).expect("the private root resolves");
    assert_eq!(
        private,
        root.join("runs").join(&parked.run_id),
        "the premise: the run's private half is present under the root its locator names"
    );
    let log = rundir::public_dir(&repo, &parked.run_id).join(rundir::EVENT_LOG);
    let committed = fs::read(&log).expect("the parked run's log");
    let owner_path = private.join(rundir::OWNER_RECORD);
    let commit_path = private.join(rundir::COMMIT_RECORD);
    let owner = fs::read(&owner_path).expect("owner.json");
    let commit = fs::read(&commit_path).expect("committed.json");
    let mut resume = resume_options(&repo, &parked.run_id);
    resume.private_root = None;

    let with_field = |record: &[u8], field: &str, value: &str| {
        let mut parsed: serde_json::Value = serde_json::from_slice(record).expect("a record");
        parsed[field] = serde_json::Value::from(value);
        serde_json::to_vec(&parsed).expect("the damaged record")
    };
    let mut other_runner: serde_json::Value = serde_json::from_slice(&owner).expect("a record");
    other_runner["runner"]["kind"] = serde_json::Value::from("container");
    let other_runner = serde_json::to_vec(&other_runner).expect("the damaged record");
    let damages: Vec<(&str, &Path, Option<Vec<u8>>, &str)> = vec![
        (
            "a missing owner record",
            &owner_path,
            None,
            "private half has no",
        ),
        (
            "an owner record that is not JSON",
            &owner_path,
            Some(b"{\"run_id\":".to_vec()),
            "is not the record this build understands",
        ),
        (
            "another run's owner record",
            &owner_path,
            Some(with_field(&owner, "run_id", "01FOREIGNRUN00000000000000")),
            "records run id",
        ),
        (
            "another repository's owner record",
            &owner_path,
            Some(with_field(&owner, "repo_key", "0000000000000000")),
            "records repo key",
        ),
        (
            "another run directory's owner record",
            &owner_path,
            Some(with_field(
                &owner,
                "public_dir",
                "/elsewhere/.upstroke/runs/01FOREIGN",
            )),
            "records public directory",
        ),
        (
            "another incarnation's owner record",
            &owner_path,
            Some(with_field(
                &owner,
                "incarnation",
                "01FOREIGNINC00000000000000",
            )),
            "records incarnation",
        ),
        (
            "an owner record of another runner",
            &owner_path,
            Some(other_runner),
            "records a different runner",
        ),
        (
            "a missing commit record",
            &commit_path,
            None,
            "private half has no",
        ),
        (
            "a commit record that is not JSON",
            &commit_path,
            Some(b"not json".to_vec()),
            "is not the record this build understands",
        ),
        (
            "a commit record of another first line",
            &commit_path,
            Some(with_field(
                &commit,
                "run_started_sha256",
                "sha256:0000000000000000000000000000000000000000000000000000000000000000",
            )),
            "says the committed first line digests",
        ),
    ];
    for state in [IncludeState::Absent, IncludeState::Changed] {
        for (what, path, damaged, says) in &damages {
            put_include(&home, state);
            match damaged {
                None => fs::remove_file(path).expect("the record, removed"),
                Some(bytes) => fs::write(path, bytes).expect("the record, damaged"),
            }
            let before = private_snapshot(&home);
            let refs = every_ref(&repo);
            let probed = probes();
            let refused = resume_with(&resume, &source)
                .expect_err("a run whose records do not prove it is refused");
            assert!(
                refused.to_string().contains(says),
                "{what}, include {state:?}: {refused}"
            );
            let moved = moved_entries(&before, &private_snapshot(&home));
            assert!(
                moved.is_empty(),
                "{what}, include {state:?}: the refused resume wrote under the private root: \
                 {moved:?}"
            );
            assert!(
                fs::read(&log).expect("the log") == committed,
                "{what}, include {state:?}: the refused resume wrote the log"
            );
            assert_eq!(
                every_ref(&repo),
                refs,
                "{what}, include {state:?}: the refs moved"
            );
            assert_eq!(probes(), probed, "{what}, include {state:?}: a probe ran");
            let original = if *path == owner_path.as_path() {
                &owner
            } else {
                &commit
            };
            fs::write(path, original).expect("the record, restored");
            until_released(&repo, &parked.run_id);
        }
    }

    let foreign = tree.path().join("another-home");
    let foreign_half = foreign.join("runs").join(&parked.run_id);
    let first_end = committed
        .iter()
        .position(|byte| *byte == b'\n')
        .expect("a committed first line");
    let first = std::str::from_utf8(committed.get(..first_end).expect("the first line"))
        .expect("a UTF-8 first line");
    let spelled = |path: &Path| {
        format!(
            "\"private_dir\":{}",
            serde_json::to_string(path.to_str().expect("a UTF-8 path")).expect("a JSON string")
        )
    };
    assert_eq!(
        first.matches(&spelled(&private)).count(),
        1,
        "the premise: the first line spells its locator once"
    );
    let mut retargeted = first
        .replacen(&spelled(&private), &spelled(&foreign_half), 1)
        .into_bytes();
    retargeted.extend_from_slice(committed.get(first_end..).expect("the rest of the log"));
    for (what, copies, says) in [
        (
            "a conforming root holding no records",
            false,
            "private half has no",
        ),
        (
            "a conforming root holding copies of this run's records",
            true,
            "says the committed first line digests",
        ),
    ] {
        for state in [IncludeState::Absent, IncludeState::Changed] {
            fs::create_dir_all(&foreign_half).expect("a conforming private half elsewhere");
            for (name, bytes) in [
                (rundir::OWNER_RECORD, &owner),
                (rundir::COMMIT_RECORD, &commit),
            ] {
                let copy = foreign_half.join(name);
                if copies {
                    fs::write(&copy, bytes).expect("a copied record");
                } else if copy.exists() {
                    fs::remove_file(&copy).expect("no record");
                }
            }
            put_include(&foreign, state);
            fs::write(&log, &retargeted).expect("the locator, retargeted");
            let mine = private_snapshot(&home);
            let theirs = private_snapshot(&foreign);
            let refs = every_ref(&repo);
            let probed = probes();
            let refused = resume_with(&resume, &source)
                .expect_err("a locator naming another root is not proof of it");
            assert!(
                refused.to_string().contains(says),
                "{what}, include {state:?}: {refused}"
            );
            for (root, snapshot) in [(&home, &mine), (&foreign, &theirs)] {
                let moved = moved_entries(snapshot, &private_snapshot(root));
                assert!(
                    moved.is_empty(),
                    "{what}, include {state:?}: the refused resume wrote under {}: {moved:?}",
                    root.display()
                );
            }
            assert!(
                fs::read(&log).expect("the log") == retargeted,
                "{what}, include {state:?}: the refused resume wrote the log"
            );
            assert_eq!(
                every_ref(&repo),
                refs,
                "{what}, include {state:?}: the refs moved"
            );
            assert_eq!(probes(), probed, "{what}, include {state:?}: a probe ran");
            fs::write(&log, &committed).expect("the locator, restored");
            until_released(&repo, &parked.run_id);
        }
    }

    for state in [IncludeState::Absent, IncludeState::Changed] {
        put_include(&home, state);
        let resumed_before = event_kinds(&repo, &parked.run_id)
            .iter()
            .filter(|kind| *kind == "run_resumed")
            .count();
        let probed = probes();
        resume_with(&resume, &source).expect("a run its records prove resumes");
        assert_eq!(
            fs::read(include_for(&repo)).expect("the include the resume wrote again"),
            crate::runner::host::RECORDED_OBJECTS_INCLUDE,
            "include {state:?}"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = fs::metadata(home.join("git"))
                .expect("the include's directory")
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o700, "include {state:?}: {mode:o}");
        }
        assert!(
            probes() > probed,
            "include {state:?}: the resume reached its probes after writing the include"
        );
        let resumed_after = event_kinds(&repo, &parked.run_id)
            .iter()
            .filter(|kind| *kind == "run_resumed")
            .count();
        assert_eq!(resumed_after, resumed_before + 1, "include {state:?}");
        until_released(&repo, &parked.run_id);
    }
}

/// Whether `program` is a file in one of the directories of this process's `PATH`.
#[cfg(unix)]
fn on_this_path(program: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|path| {
        std::env::split_paths(&path).any(|directory| directory.join(program).is_file())
    })
}

/// A `cmd` that exits 0, in a directory of its own, for a process whose `PATH` it leads.
#[cfg(unix)]
fn a_fake_cmd(tree: &ScratchTree) -> PathBuf {
    let directory = tree.path().join("fake-shells");
    fs::create_dir(&directory).expect("the fake shells' directory");
    crate::runner::container::write_program_in_its_own_process(
        &directory.join("cmd"),
        "#!/bin/sh\nexit 0\n",
    );
    directory
}

/// Runs [`recorded_shell_helper`] in a process of its own whose `PATH` starts with `shells`.
#[cfg(unix)]
fn with_shells_on_path(
    shells: &Path,
    repo: &Path,
    config: &Path,
    mode: &str,
) -> crate::agent::proc::test_support::Isolated {
    let mut directories = vec![shells.to_path_buf()];
    directories.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    let tree = temp_engine_scratch("recorded-shell-child");
    let stdout = tree.path().join("stdout");
    let stderr = tree.path().join("stderr");
    let mut helper = Command::new(std::env::current_exe().expect("this test binary"));
    helper
        .args([
            "--exact",
            "engine::tests::recorded_shell_helper",
            "--ignored",
            "--nocapture",
        ])
        .env("PATH", std::env::join_paths(directories).expect("a PATH"))
        .env("UPSTROKE_RECORDED_SHELL_REPO", repo)
        .env("UPSTROKE_RECORDED_SHELL_CONFIG", config)
        .env("UPSTROKE_RECORDED_SHELL_MODE", mode)
        .stdin(std::process::Stdio::null())
        .stdout(fs::File::create(&stdout).expect("the helper's stdout file"))
        .stderr(fs::File::create(&stderr).expect("the helper's stderr file"));
    let status = RecordedShellScope::start(helper).wait(Duration::from_secs(300));
    crate::agent::proc::test_support::Isolated {
        status,
        stdout: fs::read_to_string(&stdout).unwrap_or_default(),
        stderr: fs::read_to_string(&stderr).unwrap_or_default(),
    }
}

#[cfg(unix)]
struct RecordedShellScope {
    anchor: std::process::Child,
    group: libc::pid_t,
    helper: Option<std::process::Child>,
    settled: bool,
}

#[cfg(unix)]
impl RecordedShellScope {
    fn start(mut helper: Command) -> Self {
        use std::os::unix::process::CommandExt as _;

        let mut anchor = Command::new("/bin/sleep")
            .arg("600")
            .process_group(0)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("the recorded-shell helper's group anchor");
        let Ok(group) = libc::pid_t::try_from(anchor.id()) else {
            let _ = anchor.kill();
            let _ = anchor.wait();
            panic!("the group anchor's pid {} is not a process id", anchor.id());
        };
        let mut scope = Self {
            anchor,
            group,
            helper: None,
            settled: false,
        };
        scope.helper = Some(
            helper
                .process_group(group)
                .spawn()
                .expect("the helper process"),
        );
        scope
    }

    fn wait(&mut self, bound: Duration) -> Option<std::process::ExitStatus> {
        let deadline = std::time::Instant::now() + bound;
        let ended = loop {
            match self.helper.as_mut().map(std::process::Child::try_wait) {
                Some(Ok(Some(status))) => break Some(status),
                Some(Err(error)) => {
                    let settled = self.settle();
                    panic!("poll the recorded-shell helper: {error}; its group: {settled:?}");
                }
                Some(Ok(None)) | None => {}
            }
            if std::time::Instant::now() >= deadline {
                break None;
            }
            crate::workspace_manager::fixture::rest(Duration::from_millis(10));
        };
        if let Err(error) = self.settle() {
            panic!("the recorded-shell helper's group: {error}");
        }
        ended
    }

    fn settle(&mut self) -> Result<(), String> {
        self.settled = true;
        let mut problems = Vec::new();
        // SAFETY: `group` is the pid of `anchor`, this scope's own child, which leads the group and
        // is collected only below, after this signal; until then the id is this child's and
        // cannot be reused, so the signal reaches the anchor and every process still in its group
        // (the helper, if it has not ended, and whatever of its descendants stayed in the group)
        // and nothing else.
        if unsafe { libc::kill(-self.group, libc::SIGKILL) } != 0 {
            problems.push(format!(
                "SIGKILL to group {}: {}",
                self.group,
                std::io::Error::last_os_error()
            ));
        }
        let bound = crate::agent::proc::test_support::ISOLATED_CHILD_COLLECTION_BOUND;
        let deadline = std::time::Instant::now() + bound;
        let mut helper = self.helper.is_none();
        let mut anchor = false;
        while !(helper && anchor) {
            for (what, child, done) in [
                ("helper", self.helper.as_mut(), &mut helper),
                ("group anchor", Some(&mut self.anchor), &mut anchor),
            ] {
                let Some(child) = child.filter(|_| !*done) else {
                    continue;
                };
                match child.try_wait() {
                    Ok(Some(_)) => *done = true,
                    Ok(None) => {}
                    Err(error) => {
                        problems.push(format!("collect the {what}: {error}"));
                        *done = true;
                    }
                }
            }
            if helper && anchor {
                break;
            }
            if std::time::Instant::now() >= deadline {
                problems.push(format!(
                    "helper collected {helper}, group anchor collected {anchor}, {bound:?} after \
                     the SIGKILL"
                ));
                break;
            }
            crate::workspace_manager::fixture::rest(Duration::from_millis(10));
        }
        if problems.is_empty() {
            Ok(())
        } else {
            Err(problems.join("; "))
        }
    }
}

#[cfg(unix)]
impl Drop for RecordedShellScope {
    fn drop(&mut self) {
        if self.settled {
            return;
        }
        if let Err(error) = self.settle() {
            crate::workspace_manager::fixture::say(&format!(
                "the recorded-shell helper's group, settled on unwind: {error}"
            ));
        }
    }
}

/// PR12-CI-03 (INV-23), today's config moved to a shell this host cannot run: a zero-gate
/// schema-4 run created under `sh` resumes, through `engine::resume_with`, probing `sh`, and
/// warns that today's `[engine] shell = "cmd"` is not the run's.
#[cfg(unix)]
#[test]
fn a_zero_gate_schema_4_resume_probes_its_recorded_shell_when_todays_config_names_an_unavailable_one()
 {
    assert!(!on_this_path("cmd"), "the premise: this host has no `cmd`");
    let (tree, repo) = temp_engine_repo("topology-resume-recorded-shell-kept");
    let source = SharedTopologyStubs(std::sync::Arc::new(TopologyStub::new(
        TOPOLOGY_STUB_EDIT,
        1,
    )));
    let parked = run_with(&topology_options(&repo), &source).expect("the run parks");
    assert_eq!(parked.outcome(), RunOutcome::Parked, "{parked:#?}");
    until_released(&repo, &parked.run_id);
    let first = first_line_of_the_log(&repo, &parked.run_id);
    assert_eq!(
        first["data"]["gate_cmds"],
        serde_json::json!([]),
        "the premise: no gates"
    );
    assert_eq!(
        first["data"]["shell"], "sh",
        "the run records the shell it probed"
    );

    let today = tree.path().join("today.toml");
    fs::write(&today, "[engine]\nshell = \"cmd\"\n").expect("today's config");
    let mut resume = resume_options(&repo, &parked.run_id);
    resume.config_path = Some(today);
    let resumed = resume_with(&resume, &source)
        .expect("the resume probes the recorded `sh`, not today's `cmd`");
    assert!(
        resumed.warnings.iter().any(|warning| warning
            .contains("`shell = cmd` differs from the `shell = sh` run")
            && warning.ends_with("pre-flight probes `sh`")),
        "{:?}",
        resumed.warnings
    );
    assert_eq!(
        event_kinds(&repo, &parked.run_id)
            .iter()
            .filter(|kind| *kind == "run_resumed")
            .count(),
        1,
        "the resume passed its pre-flight and recovered"
    );
}

/// PR12-CI-03 (INV-23), the recorded shell gone: a zero-gate schema-4 run created under `cmd`
/// (a fake one on its creator's `PATH`) refuses its resume at the pre-flight, naming `cmd`,
/// although today's `[engine] shell = "sh"` would run, and appends nothing; once `cmd` is back,
/// the same resume continues.
#[cfg(unix)]
#[test]
fn a_zero_gate_schema_4_resume_refuses_its_unavailable_recorded_shell_whatever_todays_config_names()
{
    assert!(!on_this_path("cmd"), "the premise: this host has no `cmd`");
    let (tree, repo) = temp_engine_repo("topology-resume-recorded-shell-gone");
    let shells = a_fake_cmd(&tree);
    fs::write(repo.join("upstroke.toml"), "[engine]\nshell = \"cmd\"\n").expect("the config");
    git_in(&repo, &["add", "upstroke.toml"]);
    git_in(&repo, &["commit", "-q", "-m", "a run under cmd"]);
    let created = with_shells_on_path(&shells, &repo, &repo.join("upstroke.toml"), "create");
    assert!(
        created.status.is_some_and(|status| status.success())
            && created.stdout.contains("1 passed"),
        "the run is created where `cmd` runs: {created}"
    );
    let run_id = rundir::latest_run(&repo).expect("the created run");
    until_released(&repo, &run_id);
    let first = first_line_of_the_log(&repo, &run_id);
    assert_eq!(
        first["data"]["gate_cmds"],
        serde_json::json!([]),
        "the premise: no gates"
    );
    assert_eq!(
        first["data"]["shell"], "cmd",
        "the run records the shell it probed"
    );

    let today = tree.path().join("today.toml");
    fs::write(&today, "[engine]\nshell = \"sh\"\n").expect("today's config");
    let mut resume = resume_options(&repo, &run_id);
    resume.config_path = Some(today.clone());
    let log = rundir::public_dir(&repo, &run_id).join(rundir::EVENT_LOG);
    let before = fs::read(&log).expect("the log");
    let source = SharedTopologyStubs(std::sync::Arc::new(TopologyStub::new(
        TOPOLOGY_STUB_EDIT,
        1,
    )));
    let refused = resume_with(&resume, &source)
        .expect_err("the recorded `cmd` cannot run here, whatever today's config names");
    assert!(
        refused.to_string().contains("the recorded shell `cmd`"),
        "{refused}"
    );
    assert!(
        fs::read(&log).expect("the log") == before,
        "the refused resume appended a recovery event"
    );
    until_released(&repo, &run_id);

    let continued = with_shells_on_path(&shells, &repo, &today, "resume");
    assert!(
        continued.status.is_some_and(|status| status.success())
            && continued.stdout.contains("1 passed"),
        "the run stayed resumable: {continued}"
    );
}

/// PR12-CI-03, the record's compatibility: `run_started(4).shell` is required, so a schema-4 log
/// whose first line predates it is refused at its committed first line, through
/// `engine::resume_with`, before any lock or private write, and is never resumed under a shell
/// it does not record.
#[test]
fn a_schema_4_log_without_a_recorded_shell_is_refused_and_never_resumed_under_another() {
    let (_tree, repo) = temp_engine_repo("topology-resume-no-recorded-shell");
    let source = SharedTopologyStubs(std::sync::Arc::new(TopologyStub::new(
        TOPOLOGY_STUB_EDIT,
        1,
    )));
    let parked = run_with(&topology_options(&repo), &source).expect("the run parks");
    assert_eq!(parked.outcome(), RunOutcome::Parked, "{parked:#?}");
    until_released(&repo, &parked.run_id);
    let log = rundir::public_dir(&repo, &parked.run_id).join(rundir::EVENT_LOG);
    let text = fs::read_to_string(&log).expect("the log");
    let (first, rest) = text.split_once('\n').expect("a committed first line");
    let mut line: serde_json::Value = serde_json::from_str(first).expect("the first line");
    assert!(
        line["data"]
            .as_object_mut()
            .expect("run_started's data")
            .remove("shell")
            .is_some(),
        "the premise: the run recorded its shell"
    );
    let predates = format!(
        "{}\n{rest}",
        serde_json::to_string(&line).expect("the older first line")
    );
    fs::write(&log, &predates).expect("a log written before the field");
    let home = private_root_for(&repo);
    let before = private_snapshot(&home);

    let refused = resume_with(&resume_options(&repo, &parked.run_id), &source)
        .expect_err("a schema-4 run_started without its shell is refused");
    let refused = refused.to_string();
    assert!(
        refused.contains("is not a topology event") && refused.contains("shell"),
        "{refused}"
    );
    assert!(
        fs::read_to_string(&log).expect("the log") == predates,
        "the refused resume wrote the log"
    );
    let moved = moved_entries(&before, &private_snapshot(&home));
    assert!(moved.is_empty(), "the refused resume wrote: {moved:?}");
}

/// Runs `command` once for each exclusion and include state while `hold` keeps the checkout's
/// worktree lease from it. Each run must be refused with `says`, build no worker or reviewer,
/// and leave the checkout outside `.git`, the private root and every ref exactly as they were:
/// every path, kind, mode and byte. Both snapshots are taken while the hold is held, so whatever
/// taking it creates is in both.
fn assert_refused_under_a_held_lease<H>(
    repo: &Path,
    what: &str,
    says: &str,
    hold: impl Fn() -> H,
    command: impl Fn() -> Result<RunReport, UpstrokeError>,
    built: impl Fn() -> usize,
) {
    let home = private_root_for(repo);
    for exclusion in [ExclusionState::Absent, ExclusionState::Changed] {
        for include in [IncludeState::Absent, IncludeState::Changed] {
            put_exclusion(repo, exclusion);
            put_include(&home, include);
            let held = hold();
            let public = public_snapshot(repo);
            let private = private_snapshot(&home);
            let refs = every_ref(repo);
            let calls = built();
            let refused = command().expect_err("the lease is held, so the command is refused");
            let public_after = public_snapshot(repo);
            let private_after = private_snapshot(&home);
            let refs_after = every_ref(repo);
            drop(held);
            let case = format!("{what}, exclusion {exclusion:?}, include {include:?}");
            assert!(refused.to_string().contains(says), "{case}: {refused}");
            assert_eq!(built(), calls, "{case}: a worker or reviewer was built");
            let moved = moved_entries(&public, &public_after);
            assert!(
                moved.is_empty(),
                "{case}: the refused command wrote in the checkout: {moved:?}"
            );
            let moved = moved_entries(&private, &private_after);
            assert!(
                moved.is_empty(),
                "{case}: the refused command wrote under the private root: {moved:?}"
            );
            assert_eq!(refs_after, refs, "{case}: the refs moved");
        }
    }
}

/// A surviving reaper's shared hold on `public`'s cleanup lease (R28): the suite's own holder,
/// `rundir::tests::cleanup_hold_child`, in a child this test owns, which is killed and reaped
/// when the returned value drops.
#[cfg(unix)]
fn a_surviving_reaper_holding(
    public: &Path,
) -> crate::agent::proc::test_support::readiness::Producer {
    let mut reaper = crate::workspace_manager::fixture::spawn_ready_helper(
        "rundir::tests::cleanup_hold_child",
        &[("UPSTROKE_TEST_CLEANUP_DIR", public.as_os_str())],
    );
    reaper
        .await_line("held", Duration::from_secs(30))
        .or_fail("the surviving reaper never took its hold");
    assert!(
        rundir::observe_cleanup_hold(public, &mut rundir::NoHooks),
        "the premise: a surviving reaper holds the run's cleanup lease"
    );
    reaper
}

/// PR12-R9-PUBLIC-PUBLICATION-BEFORE-WORKTREE-LOCK, PR12-REG7-PRELOCK-WRITES and
/// PR12-N4-PRELOCK-PUBLICATION, one defect, the fresh half: through `run_with`, a fresh schema-4
/// run that loses the worktree lease publishes nothing first, and the same run admitted makes
/// both the exclusion and the include.
#[test]
fn a_fresh_schema_4_run_that_loses_the_worktree_lease_publishes_no_exclusion_and_no_include() {
    let (_tree, repo) = temp_engine_repo("topology-fresh-lease-first");
    let opts = topology_options(&repo);
    assert!(
        !repo.join(".upstroke").exists(),
        "the premise: the checkout has no `.upstroke` before its first run"
    );
    let stub = ObservedStub::new(0);
    let source = BorrowedObserved(&stub);
    assert_refused_under_a_held_lease(
        &repo,
        "a fresh run under another conductor's lease",
        "is already driving worktree",
        || WorktreeLock::acquire(&repo).expect("the test's lease"),
        || run_with(&opts, &source),
        || stub.built().len(),
    );
    let first = assert_admitted_and_published(&repo, || run_with(&opts, &source));
    assert_eq!(first.outcome(), RunOutcome::Complete, "{first:#?}");
    #[cfg(unix)]
    {
        let public = rundir::public_dir(&repo, &first.run_id);
        assert_refused_under_a_held_lease(
            &repo,
            "a fresh run beside a surviving reaper of the first",
            "still has a process of its own alive",
            || a_surviving_reaper_holding(&public),
            || run_with(&opts, &source),
            || stub.built().len(),
        );
        let second = assert_admitted_and_published(&repo, || run_with(&opts, &source));
        assert_eq!(second.outcome(), RunOutcome::Complete, "{second:#?}");
    }
}

/// The same defect, the resume half: through `resume_with`, a parked schema-4 run's resume that
/// loses the worktree lease restores no exclusion and writes no include, its log and refs
/// unmoved, and the same resume admitted makes both.
#[test]
fn a_schema_4_resume_that_loses_the_worktree_lease_restores_no_exclusion_and_writes_no_include() {
    let (_tree, repo) = temp_engine_repo("topology-resume-lease-first");
    let stub = ObservedStub::new(1);
    let source = BorrowedObserved(&stub);
    let parked = run_with(&topology_options(&repo), &source).expect("the run parks");
    assert_eq!(parked.outcome(), RunOutcome::Parked, "{parked:#?}");
    until_released(&repo, &parked.run_id);
    let mut resume = resume_options(&repo, &parked.run_id);
    resume.private_root = None;
    let log = rundir::public_dir(&repo, &parked.run_id).join(rundir::EVENT_LOG);
    let committed = fs::read(&log).expect("the parked run's log");

    assert_refused_under_a_held_lease(
        &repo,
        "a resume under another conductor's lease",
        "is already driving worktree",
        || WorktreeLock::acquire(&repo).expect("the test's lease"),
        || resume_with(&resume, &source),
        || stub.built().len(),
    );
    #[cfg(unix)]
    {
        let public = rundir::public_dir(&repo, &parked.run_id);
        assert_refused_under_a_held_lease(
            &repo,
            "a resume beside its run's surviving reaper",
            "still has a process of its own alive",
            || a_surviving_reaper_holding(&public),
            || resume_with(&resume, &source),
            || stub.built().len(),
        );
    }
    assert!(
        fs::read(&log).expect("the log") == committed,
        "the refused resumes wrote the log"
    );

    let resumed = || {
        event_kinds(&repo, &parked.run_id)
            .iter()
            .filter(|kind| *kind == "run_resumed")
            .count()
    };
    let before = resumed();
    let report = assert_admitted_and_published(&repo, || resume_with(&resume, &source));
    assert_eq!(report.run_id, parked.run_id);
    assert_eq!(
        resumed(),
        before + 1,
        "the admitted resume appended `run_resumed`"
    );
}

/// The same defect over the default private root, which only the include's publication creates:
/// in a child whose home is a directory of this fixture, so the default root it resolves is the
/// fixture's and nobody else's.
#[cfg(unix)]
#[test]
fn a_fresh_schema_4_run_over_the_default_root_that_loses_the_worktree_lease_creates_nothing_under_home()
 {
    let (tree, repo) = temp_engine_repo("topology-default-root-lease");
    let home = tree.path().join("child-home");
    fs::create_dir(&home).expect("the child's home");
    let ended = crate::agent::proc::test_support::run_test_isolated(
        "engine::tests::default_root_lease_child",
        &[
            ("HOME", home.as_os_str()),
            ("USERPROFILE", home.as_os_str()),
            (DEFAULT_ROOT_LEASE_REPO, repo.as_os_str()),
        ],
        Duration::from_secs(300),
    );
    assert!(
        ended.status.is_some_and(|status| status.success()) && ended.stdout.contains("1 passed"),
        "the default-root run, refused under a held lease and then admitted: {ended}"
    );
}

#[test]
fn a_schema_4_resume_whose_router_cannot_read_its_log_refuses_before_publishing_anything() {
    use crate::engine::topology::entry::route_resume;

    let (_tree, repo) = temp_engine_repo("topology-resume-unread-route");
    let stub = ObservedStub::new(1);
    let source = BorrowedObserved(&stub);
    let parked = run_with(&topology_options(&repo), &source).expect("the run parks");
    assert_eq!(parked.outcome(), RunOutcome::Parked, "{parked:#?}");
    until_released(&repo, &parked.run_id);
    let public = rundir::public_dir(&repo, &parked.run_id);
    let log = public.join(rundir::EVENT_LOG);
    let committed = fs::read(&log).expect("the parked run's log");
    let home = private_root_for(&repo);
    let resume = resume_options(&repo, &parked.run_id);
    assert_eq!(
        rundir::classify_run_dir(&public),
        rundir::RunDirClass::Committed,
        "the premise: the census classifies the run's committed header"
    );
    assert_eq!(
        rundir::resolve_run_id(&repo, &parked.run_id)
            .ok()
            .as_deref(),
        Some(parked.run_id.as_str()),
        "the premise: the run's id resolves"
    );
    assert!(
        route_resume(&resume)
            .expect("a readable schema-4 log routes")
            .is_some(),
        "the control: read, its header routes the resume to the topology"
    );

    for exclusion in [ExclusionState::Absent, ExclusionState::Changed] {
        for include in [IncludeState::Absent, IncludeState::Changed] {
            put_exclusion(&repo, exclusion);
            put_include(&home, include);
            let lease = WorktreeLock::acquire(&repo).expect("the test's lease");
            let checkout = public_snapshot(&repo);
            let private = private_snapshot(&home);
            let refs = every_ref(&repo);
            let calls = stub.built().len();
            let (refused, reads) =
                with_initial_read_failing(&log, || resume_with(&resume, &source));
            let checkout_after = public_snapshot(&repo);
            let private_after = private_snapshot(&home);
            let refs_after = every_ref(&repo);
            drop(lease);
            let case = format!("exclusion {exclusion:?}, include {include:?}");
            let moved = moved_entries(&private, &private_after);
            assert!(
                moved.is_empty(),
                "{case}: the unread resume wrote under the private root: {moved:?}"
            );
            let moved = moved_entries(&checkout, &checkout_after);
            assert!(
                moved.is_empty(),
                "{case}: the unread resume wrote in the checkout: {moved:?}"
            );
            assert_eq!(refs_after, refs, "{case}: the refs moved");
            assert_eq!(
                stub.built().len(),
                calls,
                "{case}: a worker or reviewer was built"
            );
            assert_eq!(reads, 1, "{case}: the router's initial read failed once");
            assert_the_initial_read_refused(refused.err(), &log, &case);
        }
    }

    let private_dir = private_dir_of(&repo, &parked.run_id);
    let owner_path = private_dir.join(rundir::OWNER_RECORD);
    let commit_path = private_dir.join(rundir::COMMIT_RECORD);
    let owner = fs::read(&owner_path).expect("owner.json");
    let commit = fs::read(&commit_path).expect("committed.json");
    let with_field = |record: &[u8], field: &str, value: &str| {
        let mut parsed: serde_json::Value = serde_json::from_slice(record).expect("a record");
        parsed[field] = serde_json::Value::from(value);
        serde_json::to_vec(&parsed).expect("the damaged record")
    };
    for (what, path, original, damaged, says) in [
        (
            "another run's owner record",
            &owner_path,
            &owner,
            with_field(&owner, "run_id", "01FOREIGNRUN00000000000000"),
            "records run id",
        ),
        (
            "a commit record of another first line",
            &commit_path,
            &commit,
            with_field(
                &commit,
                "run_started_sha256",
                "sha256:0000000000000000000000000000000000000000000000000000000000000000",
            ),
            "says the committed first line digests",
        ),
    ] {
        fs::write(path, &damaged).expect("the record, damaged");
        put_include(&home, IncludeState::Absent);
        let private = private_snapshot(&home);
        let (refused, reads) = with_initial_read_failing(&log, || resume_with(&resume, &source));
        let moved = moved_entries(&private, &private_snapshot(&home));
        assert!(
            moved.is_empty(),
            "{what}, its log unread: the resume wrote under the private root: {moved:?}"
        );
        assert_eq!(reads, 1, "{what}: the router's initial read failed once");
        assert_the_initial_read_refused(refused.err(), &log, what);
        let refused = resume_with(&resume, &source)
            .expect_err("read, a run whose records do not prove it is refused");
        let moved = moved_entries(&private, &private_snapshot(&home));
        assert!(
            moved.is_empty(),
            "{what}, its log read: the resume wrote under the private root: {moved:?}"
        );
        assert!(refused.to_string().contains(says), "{what}: {refused}");
        fs::write(path, original).expect("the record, restored");
    }

    put_include(&home, IncludeState::Changed);
    let lease = WorktreeLock::acquire(&repo).expect("the test's lease");
    let checkout = public_snapshot(&repo);
    let private = private_snapshot(&home);
    let refused = resume_with(&resume, &source).expect_err("read, the resume meets the lease");
    let checkout_after = public_snapshot(&repo);
    let private_after = private_snapshot(&home);
    drop(lease);
    assert!(
        refused.to_string().contains("is already driving worktree"),
        "{refused}"
    );
    let moved = moved_entries(&private, &private_after);
    assert!(
        moved.is_empty(),
        "read, the refused resume wrote under the private root: {moved:?}"
    );
    let moved = moved_entries(&checkout, &checkout_after);
    assert!(
        moved.is_empty(),
        "read, the refused resume wrote in the checkout: {moved:?}"
    );
    assert!(
        fs::read(&log).expect("the log") == committed,
        "a refused resume wrote the log"
    );
    let resumed = || {
        event_kinds(&repo, &parked.run_id)
            .iter()
            .filter(|kind| *kind == "run_resumed")
            .count()
    };
    let before = resumed();
    let report = assert_admitted_and_published(&repo, || resume_with(&resume, &source));
    assert_eq!(report.run_id, parked.run_id);
    assert_eq!(
        resumed(),
        before + 1,
        "the admitted resume appended `run_resumed`"
    );

    let (_legacy_tree, legacy_repo, legacy_run) = parked_run("topology-resume-unread-legacy");
    let legacy_resume = resume_options(&legacy_repo, &legacy_run);
    let legacy_log = rundir::public_dir(&legacy_repo, &legacy_run).join(rundir::EVENT_LOG);
    let legacy_home = private_root_for(&legacy_repo);
    assert!(
        route_resume(&legacy_resume)
            .expect("a readable legacy log routes")
            .is_none(),
        "the control: read, a legacy header routes the resume to the legacy lines"
    );
    put_include(&legacy_home, IncludeState::Absent);
    let checkout = public_snapshot(&legacy_repo);
    let private = private_snapshot(&legacy_home);
    let (refused, reads) = with_initial_read_failing(&legacy_log, || {
        resume_with(&legacy_resume, &fake(Effect::EditFile))
    });
    let moved = moved_entries(&private, &private_snapshot(&legacy_home));
    assert!(
        moved.is_empty(),
        "a legacy run's unread log: the resume wrote under the private root: {moved:?}"
    );
    let moved = moved_entries(&checkout, &public_snapshot(&legacy_repo));
    assert!(
        moved.is_empty(),
        "a legacy run's unread log: the resume wrote in the checkout: {moved:?}"
    );
    assert_eq!(reads, 1, "the legacy router's initial read failed once");
    assert_the_initial_read_refused(
        refused.err(),
        &legacy_log,
        "a legacy run whose log the router cannot read",
    );
    let report = resume_answering(&legacy_repo, &legacy_run, Effect::EditFile);
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:#?}");
    assert_eq!(
        fs::read(include_for(&legacy_repo)).expect("the include"),
        crate::runner::host::RECORDED_OBJECTS_INCLUDE,
        "read, the legacy lines published the include as before"
    );
}

#[cfg(unix)]
#[test]
fn a_schema_4_resume_over_the_default_root_whose_router_cannot_read_its_log_creates_nothing_under_home()
 {
    let (tree, repo) = temp_engine_repo("topology-default-root-unread");
    let stub = ObservedStub::new(1);
    let parked =
        run_with(&topology_options(&repo), &BorrowedObserved(&stub)).expect("the run parks");
    assert_eq!(parked.outcome(), RunOutcome::Parked, "{parked:#?}");
    until_released(&repo, &parked.run_id);
    let home = tree.path().join("child-home");
    fs::create_dir(&home).expect("the child's home");
    let ended = crate::agent::proc::test_support::run_test_isolated(
        "engine::tests::default_root_unread_resume_child",
        &[
            ("HOME", home.as_os_str()),
            ("USERPROFILE", home.as_os_str()),
            (DEFAULT_ROOT_UNREAD_REPO, repo.as_os_str()),
            (
                DEFAULT_ROOT_UNREAD_RUN,
                std::ffi::OsStr::new(parked.run_id.as_str()),
            ),
        ],
        Duration::from_secs(300),
    );
    assert!(
        ended.status.is_some_and(|status| status.success())
            && ended.stdout.contains("1 passed")
            && ended.stderr.contains(DEFAULT_ROOT_UNREAD_DONE),
        "the default-root resume, refused on its unread log and then admitted: {ended}"
    );
}

const RETAINED_HUSK_PRODUCER: &str = "engine::topology::create::tests::create_kill_child";

fn retained_p5b_husk(root: &Path) -> String {
    let exe = std::env::current_exe().expect("the test binary");
    let mut command = CommandSpec::new(exe.to_string_lossy().into_owned());
    command.args = [
        "--exact",
        RETAINED_HUSK_PRODUCER,
        "--ignored",
        "--nocapture",
        "--test-threads",
        "1",
    ]
    .map(str::to_owned)
    .to_vec();
    command.env = vec![
        (
            "UPSTROKE_TEST_KILL_DIR".to_owned(),
            root.to_string_lossy().into_owned(),
        ),
        ("UPSTROKE_TEST_KILL_SITE".to_owned(), "p5b".to_owned()),
    ];
    let request = crate::runner::gate_request(
        command,
        root.to_path_buf(),
        Duration::from_secs(120),
        crate::runner::InvocationId::attempt(
            crate::topology::registry::TaskKey(0),
            crate::topology::events::GenerationId(0),
            crate::topology::events::AttemptNumber(1),
            crate::runner::invocation::AttemptRole::Gate(0),
            0,
        ),
    );
    let output =
        crate::runner::Runner::run_blocking(&crate::runner::host::HostRunner::new(), &request)
            .expect("the producer starts");
    assert!(
        !output.timed_out,
        "the producer outlived its bound: {output:?}"
    );
    assert_ne!(
        output.code,
        Some(0),
        "the producer must have died at P5b: {output:?}"
    );

    let repo = root.join("repo");
    let [husk]: [String; 1] =
        rundir::run_dir_names(&repo)
            .try_into()
            .unwrap_or_else(|names: Vec<String>| {
                panic!("the producer made one run directory: {names:?}")
            });
    let public = rundir::public_dir(&repo, &husk);
    let private = fs::canonicalize(root.join("private"))
        .expect("the creation private root")
        .join("runs")
        .join(&husk);
    assert!(
        private.join(rundir::COMMIT_RECORD).is_file(),
        "the commit record is what makes this the P5b prefix"
    );
    assert!(
        fs::read(public.join(rundir::EVENT_LOG))
            .expect("the log exists")
            .is_empty(),
        "nothing was appended"
    );
    assert_eq!(rundir::classify_run_dir(&public), rundir::RunDirClass::Husk);
    let report = rundir::husk_report(
        &repo,
        &husk,
        &rundir::RepoKey::v1(&root.join("git-dir")),
        &root.join("private"),
    );
    assert!(
        matches!(
            report.disposition,
            rundir::HuskDisposition::Retained(rundir::RetainReason::PossiblyCommitted)
        ),
        "{:?}",
        report.disposition
    );
    husk
}

#[test]
fn a_retained_schema_4_husk_is_refused_before_any_root_is_published() {
    use crate::engine::topology::entry::route_resume;

    let (_legacy_tree, legacy_repo, legacy_run) = parked_run("topology-husk-legacy");
    assert!(
        route_resume(&resume_options(&legacy_repo, &legacy_run))
            .expect("a resolved legacy run routes")
            .is_none(),
        "the control: a resolved legacy run routes to the legacy lines"
    );
    put_include(&private_root_for(&legacy_repo), IncludeState::Absent);
    let report = resume_answering(&legacy_repo, &legacy_run, Effect::EditFile);
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:#?}");
    assert_eq!(
        fs::read(include_for(&legacy_repo)).expect("the include"),
        crate::runner::host::RECORDED_OBJECTS_INCLUDE,
        "the control: the legacy lines published the include as before"
    );

    let (tree, repo) = temp_engine_repo("topology-husk-refusal");
    let husk = retained_p5b_husk(tree.path());
    let creation_root = tree.path().join("private");
    let named = private_root_for(&repo);
    fs::create_dir_all(&named).expect("the root the resume names");
    assert_ne!(
        fs::canonicalize(&creation_root).expect("the creation root"),
        fs::canonicalize(&named).expect("the named root"),
        "the premise: the resume names a root the husk was not created under"
    );
    let says = rundir::resolve_run_id(&repo, &husk)
        .expect_err("the premise: the resolver refuses the husk's exact id")
        .to_string();
    assert!(
        says.contains(&format!("`{husk}` never recorded a committed run_started")),
        "the premise: the resolver's husk sentence names this run: {says}"
    );
    let resume = resume_options(&repo, &husk);
    let stub = ObservedStub::new(0);
    let source = BorrowedObserved(&stub);
    let git_dir = repo.join(".git");
    drop(WorktreeLock::acquire(&repo).expect("the lease, taken once so that its file exists"));
    assert!(
        git_dir.join("upstroke-worktree.lock").is_file(),
        "the premise: the lease's own file exists before the first snapshot of .git, so no lease \
         taken below creates anything there"
    );

    for held in [false, true] {
        for exclusion in [ExclusionState::Absent, ExclusionState::Changed] {
            for include in [IncludeState::Absent, IncludeState::Changed] {
                put_exclusion(&repo, exclusion);
                put_include(&named, include);
                let case =
                    format!("lease held {held}, exclusion {exclusion:?}, include {include:?}");
                let git = private_snapshot_in(&git_dir, &case);
                let lease = held.then(|| WorktreeLock::acquire(&repo).expect("the test's lease"));
                assert_the_lease_is(
                    held,
                    &repo,
                    &git_dir,
                    tree.path(),
                    &format!("{case}, before the resume"),
                );
                let selected = private_snapshot_in(&named, &case);
                let created = private_snapshot_in(&creation_root, &case);
                let checkout = public_snapshot_in(&repo, &case);
                let refused = resume_with(&resume, &source);
                let selected_after = private_snapshot_in(&named, &case);
                let created_after = private_snapshot_in(&creation_root, &case);
                let checkout_after = public_snapshot_in(&repo, &case);
                if held {
                    assert_the_lease_is(
                        true,
                        &repo,
                        &git_dir,
                        tree.path(),
                        &format!("{case}, after the resume and every observation under the lease"),
                    );
                }
                drop(lease);
                let git_after = private_snapshot_in(&git_dir, &case);
                for (place, before, after) in [
                    ("under the root it names", &selected, &selected_after),
                    ("under the husk's creation root", &created, &created_after),
                    ("in the checkout", &checkout, &checkout_after),
                    ("in the checkout's Git directory", &git, &git_after),
                ] {
                    let moved = moved_entries(before, after);
                    assert!(
                        moved.is_empty(),
                        "{case}: the husk's resume wrote {place}: {moved:?}"
                    );
                }
                assert!(
                    stub.built().is_empty(),
                    "{case}: a worker or reviewer was built"
                );
                let refused = refused.expect_err("a husk has no run to open");
                assert_eq!(
                    refused.to_string(),
                    says,
                    "{case}: not the resolver's own refusal"
                );
            }
        }
    }
}

#[cfg(unix)]
#[test]
fn a_retained_schema_4_husk_resumed_over_the_default_root_creates_nothing_under_home() {
    let (tree, repo) = temp_engine_repo("topology-husk-default-root");
    let husk = retained_p5b_husk(tree.path());
    let creation_root = tree.path().join("private");
    let home = tree.path().join("child-home");
    fs::create_dir(&home).expect("the child's home");
    let ended = crate::agent::proc::test_support::run_test_isolated(
        "engine::tests::default_root_husk_resume_child",
        &[
            ("HOME", home.as_os_str()),
            ("USERPROFILE", home.as_os_str()),
            (DEFAULT_ROOT_HUSK_REPO, repo.as_os_str()),
            (DEFAULT_ROOT_HUSK_RUN, std::ffi::OsStr::new(husk.as_str())),
            (DEFAULT_ROOT_HUSK_CREATION_ROOT, creation_root.as_os_str()),
        ],
        Duration::from_secs(300),
    );
    assert!(
        ended.status.is_some_and(|status| status.success())
            && ended.stdout.contains("1 passed")
            && ended.stderr.contains(DEFAULT_ROOT_HUSK_DONE),
        "the default-root husk resume, refused before anything was published: {ended}"
    );
}

fn execution_roots_under(private_root: &Path) -> Vec<String> {
    let workspaces = private_root.join("workspaces");
    let keys = match fs::read_dir(&workspaces) {
        Ok(keys) => keys,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(error) => panic!("list {}: {error}", workspaces.display()),
    };
    let mut roots = Vec::new();
    for key in keys {
        let key = key.expect("a repository namespace").path();
        for root in
            fs::read_dir(&key).unwrap_or_else(|error| panic!("list {}: {error}", key.display()))
        {
            roots.push(
                root.expect("an execution root")
                    .file_name()
                    .to_string_lossy()
                    .into_owned(),
            );
        }
    }
    roots.sort();
    roots
}

#[cfg(unix)]
fn retain_execution_root(repo: &Path, run_id: &str) {
    let manager = manager_today(repo, run_id);
    assert_eq!(
        manager.execution_root(),
        recorded_execution_root(repo, run_id),
        "the premise: the run's own namespace derives the root it recorded"
    );
    manager
        .create_execution_root(&mut crate::workspace_manager::NoHooks)
        .expect("the retained scaffolding");
}

#[cfg(unix)]
fn copy_common_dir(from: &Path, to: &Path) {
    let mut pending = vec![(from.to_path_buf(), to.to_path_buf())];
    while let Some((source, target)) = pending.pop() {
        fs::create_dir(&target).expect("a directory of the copy");
        for entry in fs::read_dir(&source).expect("a directory of the common directory") {
            let entry = entry.expect("a directory entry");
            let name = entry.file_name();
            if source == from && (name == "worktrees" || name == "upstroke-worktree.lock") {
                continue;
            }
            let kind = entry.file_type().expect("the entry's type");
            if kind.is_dir() {
                pending.push((entry.path(), target.join(&name)));
            } else if kind.is_file() {
                fs::copy(entry.path(), target.join(&name)).expect("a file of the copy");
            } else {
                panic!(
                    "{} is neither a file nor a directory",
                    entry.path().display()
                );
            }
        }
    }
}

#[cfg(unix)]
#[test]
fn a_schema_4_resume_under_another_effective_common_directory_refuses_before_any_effect() {
    let (tree, repo) = temp_engine_repo("topology-common-dir");
    let park = || {
        let stub = ObservedStub::new(1);
        let parked =
            run_with(&topology_options(&repo), &BorrowedObserved(&stub)).expect("the run parks");
        assert_eq!(parked.outcome(), RunOutcome::Parked, "{parked:#?}");
        until_released(&repo, &parked.run_id);
        parked.run_id
    };
    let open = park();
    let open_absolute = park();
    let open_relative = park();
    let open_unchanged = park();
    let finished = {
        let stub = ObservedStub::new(0);
        let report = run_with(&topology_options(&repo), &BorrowedObserved(&stub))
            .expect("the run completes");
        assert_eq!(report.outcome(), RunOutcome::Complete, "{report:#?}");
        until_released(&repo, &report.run_id);
        report.run_id
    };
    assert!(
        fs::symlink_metadata(recorded_execution_root(&repo, &finished)).is_err(),
        "the premise: finalization pruned the finished run's root"
    );
    retain_execution_root(&repo, &finished);
    let isolated = |test: &str, common: &std::ffi::OsStr, open: &str| {
        crate::agent::proc::test_support::run_test_isolated(
            test,
            &[
                ("GIT_COMMON_DIR", common),
                (COMMON_DIR_REPO, repo.as_os_str()),
                (COMMON_DIR_OPEN_RUN, std::ffi::OsStr::new(open)),
                (
                    COMMON_DIR_FINISHED_RUN,
                    std::ffi::OsStr::new(finished.as_str()),
                ),
            ],
            Duration::from_secs(300),
        )
    };

    let checkout_common =
        fs::canonicalize(repo.join(".git")).expect("the checkout's common directory");
    for (spelling, value, run_id) in [
        ("absolute", checkout_common.as_os_str(), &open_absolute),
        ("relative", std::ffi::OsStr::new(".git"), &open_relative),
    ] {
        let admitted = isolated(
            "engine::tests::common_dir_admitted_resume_child",
            value,
            run_id,
        );
        assert!(
            admitted.status.is_some_and(|status| status.success())
                && admitted.stdout.contains("1 passed")
                && admitted.stderr.contains(COMMON_DIR_ADMITTED),
            "the control: the checkout's own common directory, spelled {spelling}: {admitted}"
        );
        retain_execution_root(&repo, &finished);
    }
    assert_admitted_at_the_recorded_roots(&repo, &open_unchanged, &finished);
    retain_execution_root(&repo, &finished);

    let other = tree.path().join("common-b");
    copy_common_dir(&repo.join(".git"), &other);
    let other = fs::canonicalize(&other).expect("the other common directory");
    let refused = isolated(
        "engine::tests::common_dir_refused_resume_child",
        other.as_os_str(),
        &open,
    );
    assert!(
        refused.status.is_some_and(|status| status.success())
            && refused.stdout.contains("1 passed")
            && refused.stderr.contains(COMMON_DIR_REFUSED),
        "under another common directory both resumes refuse before any effect: {refused}"
    );
}

/// The program a configured reviewer's adapter asks the runner for in P12-W17.
/// Nothing installs it.
const ABSENT_REVIEWER_CLI: &str = "upstroke-p12-w17-absent-reviewer-cli";

fn probe_an_absent_cli(
    adapter: &'static str,
    program: &'static str,
    runner: &dyn crate::runner::Runner,
) -> Result<Caps, UpstrokeError> {
    let request = crate::agent::probe_request(
        adapter,
        CommandSpec::new(program).arg("--version"),
        0,
        Duration::from_secs(60),
    )?;
    let output = runner.run_blocking(&request).map_err(|cause| {
        crate::agent::bin::boundary_refused(
            program,
            "nothing installs it",
            &UpstrokeError::from(cause),
        )
    })?;
    Err(UpstrokeError::Agent {
        message: format!(
            "`{program}` answered with {:?}, so the witness's premise, an absent CLI, does not \
             hold",
            output.code
        ),
    })
}

/// A `copilot` adapter whose CLI is not installed: its probe asks the run's own
/// runner to start a program that is not on `PATH`, and refuses the way
/// `CopilotAdapter` does when its CLI is absent (`agent::bin::boundary_refused`).
struct AbsentReviewerCli;

impl AgentAdapter for AbsentReviewerCli {
    fn id(&self) -> &'static str {
        crate::agent::copilot::ADAPTER_ID
    }

    fn probe(&self, runner: &dyn crate::runner::Runner) -> Result<Caps, UpstrokeError> {
        probe_an_absent_cli(
            crate::agent::copilot::ADAPTER_ID,
            ABSENT_REVIEWER_CLI,
            runner,
        )
    }

    fn build(&self, _run: &TaskRun) -> Result<CommandSpec, UpstrokeError> {
        Err(UpstrokeError::Agent {
            message: "an absent CLI is never built".to_owned(),
        })
    }

    fn parse(&self, _out: &ProcessOutput) -> Result<Outcome, UpstrokeError> {
        Err(UpstrokeError::Agent {
            message: "an absent CLI is never parsed".to_owned(),
        })
    }
}

/// The same `copilot` reviewer with its CLI present: it probes the shell and
/// reviews as the topology stub does.
struct PresentReviewerCli<'a>(&'a TopologyStub);

impl AgentAdapter for PresentReviewerCli<'_> {
    fn id(&self) -> &'static str {
        crate::agent::copilot::ADAPTER_ID
    }

    fn probe(&self, runner: &dyn crate::runner::Runner) -> Result<Caps, UpstrokeError> {
        let request = crate::agent::probe_request(
            crate::agent::copilot::ADAPTER_ID,
            shell_spec("exit 0"),
            0,
            Duration::from_secs(60),
        )?;
        let output = runner.run_blocking(&request)?;
        if output.code != Some(0) {
            return Err(UpstrokeError::Agent {
                message: format!("the present reviewer's probe exited {:?}", output.code),
            });
        }
        Ok(Caps {
            version: "0.0.0-present-reviewer".to_owned(),
            json_output: true,
            session_resume: false,
            cost_reporting: true,
            read_only_mode: true,
            acp: false,
            model_list: false,
        })
    }

    fn build(&self, run: &TaskRun) -> Result<CommandSpec, UpstrokeError> {
        self.0.build(run)
    }

    fn parse(&self, out: &ProcessOutput) -> Result<Outcome, UpstrokeError> {
        self.0.parse(out)
    }
}

/// The topology stub for the chain, and one reviewer adapter beside it.
struct StubAndReviewer<'a> {
    stub: &'a TopologyStub,
    reviewer: &'a dyn AgentAdapter,
}

impl AdapterSource for StubAndReviewer<'_> {
    fn get(&self, id: &str) -> Option<&dyn AgentAdapter> {
        if id == TOPOLOGY_STUB {
            Some(self.stub as &dyn AgentAdapter)
        } else {
            (id == self.reviewer.id()).then_some(self.reviewer)
        }
    }
}

/// The review tier pinned to `copilot`, and the chain held to a rung the stub
/// serves, so `copilot` is a configured reviewer and nothing else.
const COPILOT_REVIEWER_CONFIG: &str = "[routing]\n\
                                       implement = { chain = [\"small\"], attempts_per = 1 }\n\
                                       review = { tier = \"frontier\" }\n\n\
                                       [[pins]]\ntier = \"frontier\"\nagent = \"copilot\"\n\
                                       model = \"gpt-5.3-codex\"\n";

#[test]
fn a_configured_reviewer_whose_cli_is_absent_fails_creation_before_p5b_and_commits_nothing() {
    let (_tree, repo) = temp_engine_repo("topology-absent-reviewer");
    seed(&repo, ONE_ATTEMPT_PLAN, Some(COPILOT_REVIEWER_CONFIG));
    let stub = TopologyStub::new(TOPOLOGY_STUB_EDIT, 0);
    let refused = run_with(
        &topology_options(&repo),
        &StubAndReviewer {
            stub: &stub,
            reviewer: &AbsentReviewerCli,
        },
    )
    .expect_err("an absent reviewer CLI refuses creation");
    let said = refused.to_string();
    assert!(
        said.contains("stopped at P3c"),
        "after the execution root's claim, before P4 certified: {said}"
    );
    assert!(said.contains(ABSENT_REVIEWER_CLI), "{said}");
    assert!(said.contains("could not be executed"), "{said}");
    assert_a_refused_creation_left_nothing(&repo, &said);

    let present = PresentReviewerCli(&stub);
    let report = run_with(
        &topology_options(&repo),
        &StubAndReviewer {
            stub: &stub,
            reviewer: &present,
        },
    )
    .expect("with its CLI present, the same configuration creates the run");
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:#?}");
    let first = first_line_of_the_log(&repo, &report.run_id);
    assert_eq!(
        first["data"]["probed_agents"],
        serde_json::json!(["claude-code", "copilot"]),
        "{first}"
    );
    until_released(&repo, &report.run_id);
}

fn assert_a_refused_creation_left_nothing(repo: &Path, said: &str) {
    assert_eq!(
        execution_roots_under(&private_root_for(repo)),
        Vec::<String>::new(),
        "the creator settled the root it claimed before it reclaimed either half: {said}"
    );
    assert_eq!(
        rundir::run_dir_names(repo),
        Vec::<String>::new(),
        "no run directory is left behind"
    );
    let private_runs = private_root_for(repo).join("runs");
    let mut committed_records = Vec::new();
    let mut stack = vec![private_runs.clone()];
    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) if dir == private_runs && error.kind() == std::io::ErrorKind::NotFound => {
                continue;
            }
            Err(error) => panic!("list {}: {error}", dir.display()),
        };
        for entry in entries {
            let path = entry
                .unwrap_or_else(|error| panic!("read an entry of {}: {error}", dir.display()))
                .path();
            let is_dir = fs::metadata(&path)
                .unwrap_or_else(|error| panic!("stat {}: {error}", path.display()))
                .is_dir();
            if is_dir {
                stack.push(path);
            } else if path
                .file_name()
                .is_some_and(|name| name == "committed.json")
            {
                committed_records.push(path);
            }
        }
    }
    assert_eq!(
        committed_records,
        Vec::<PathBuf>::new(),
        "no private half carries a commit record"
    );
    assert_eq!(
        git_in(
            repo,
            &[
                "for-each-ref",
                "--format=%(refname)",
                "refs/heads/upstroke/"
            ]
        ),
        "",
        "no integration ref"
    );
}

#[test]
#[should_panic(expected = "list ")]
fn an_absent_alternative_reviewer_cli_refusal_check_fails_on_a_runs_it_cannot_list() {
    let (_tree, repo) = temp_engine_repo("refusal-check-unlistable-runs");
    let private_root = private_root_for(&repo);
    fs::create_dir_all(&private_root).expect("create the private root");
    fs::write(private_root.join("runs"), "not a directory")
        .expect("put a file where the private runs directory would be");
    assert_a_refused_creation_left_nothing(&repo, "a fixture, not a refusal");
}

fn profiled_pool_file(tree: &ScratchTree, profile: &Path) -> PathBuf {
    let path = tree.path().join("profiled-pools.toml");
    fs::write(
        &path,
        format!(
            "[pools.claude-max]\nkind = \"subscription-window\"\nagent = \"claude-code\"\n\
             profile = {}\n",
            toml::Value::String(profile.display().to_string())
        ),
    )
    .expect("the profiled pools file");
    path
}

#[test]
fn the_profiled_pool_fixture_writes_a_profile_toml_reads_back() {
    let (tree, _repo) = temp_engine_repo("profiled-pool-fixture-encoding");
    let profile = Path::new("/tmp/O'Brien/a \"b\"/c\\d");
    let path = profiled_pool_file(&tree, profile);
    let text = fs::read_to_string(&path).expect("read the profiled pools file");
    let table: toml::Table = toml::from_str(&text)
        .unwrap_or_else(|error| panic!("the fixture's file is not TOML: {error}\n{text}"));
    let read_back = table
        .get("pools")
        .and_then(|pools| pools.get("claude-max"))
        .and_then(|pool| pool.get("profile"))
        .and_then(toml::Value::as_str);
    assert_eq!(read_back, Some("/tmp/O'Brien/a \"b\"/c\\d"), "{text}");
}

const FIXTURE_OAUTH_TOKEN: &str = "sk-ant-oat01-engine-fixture-token-value";

fn an_oauth_token_is_set(key: &str) -> Option<std::ffi::OsString> {
    (key == "CLAUDE_CODE_OAUTH_TOKEN").then(|| std::ffi::OsString::from(FIXTURE_OAUTH_TOKEN))
}

fn no_oauth_token(_key: &str) -> Option<std::ffi::OsString> {
    None
}

struct CountedStub {
    stub: TopologyStub,
    probes: std::sync::atomic::AtomicUsize,
    builds: std::sync::atomic::AtomicUsize,
}

impl CountedStub {
    fn new() -> Self {
        Self {
            stub: TopologyStub::new(TOPOLOGY_STUB_EDIT, 0),
            probes: std::sync::atomic::AtomicUsize::new(0),
            builds: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    fn asked(&self) -> (usize, usize) {
        (
            self.probes.load(std::sync::atomic::Ordering::SeqCst),
            self.builds.load(std::sync::atomic::Ordering::SeqCst),
        )
    }
}

impl AgentAdapter for CountedStub {
    fn id(&self) -> &'static str {
        self.stub.id()
    }

    fn probe(&self, runner: &dyn crate::runner::Runner) -> Result<Caps, UpstrokeError> {
        self.probes
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.stub.probe(runner)
    }

    fn build(&self, run: &TaskRun) -> Result<CommandSpec, UpstrokeError> {
        self.builds
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.stub.build(run)
    }

    fn parse(&self, out: &ProcessOutput) -> Result<Outcome, UpstrokeError> {
        self.stub.parse(out)
    }
}

impl AdapterSource for CountedStub {
    fn get(&self, id: &str) -> Option<&dyn AgentAdapter> {
        (id == TOPOLOGY_STUB).then_some(self as &dyn AgentAdapter)
    }
}

fn assert_a_profile_refusal_created_nothing(
    refused: &UpstrokeError,
    repo: &Path,
    stub: &CountedStub,
) -> String {
    let said = refused.to_string();
    assert!(
        matches!(refused, UpstrokeError::Refused { .. }),
        "a refusal, not some other failure: {said}"
    );
    assert!(
        said.starts_with("pre-flight: "),
        "the pre-flight's refusal, in the shape a resume's takes: {said}"
    );
    assert!(
        said.ends_with(
            ". No agent CLI was started and no event was appended; correct the pools file or \
             the environment and run the same command again"
        ),
        "a profile refusal ends in its own closing sentence, true of a fresh run: {said}"
    );
    assert!(
        !said.contains("stopped at"),
        "refused before the run's creation began, not by a creation that then reclaimed what \
         it made: {said}"
    );
    assert_eq!(
        stub.asked(),
        (0, 0),
        "nothing was probed or built for the run: {said}"
    );
    assert_a_refused_creation_left_nothing(repo, &said);
    let private_runs = private_root_for(repo).join("runs");
    let private_halves: Vec<PathBuf> = match fs::read_dir(&private_runs) {
        Ok(entries) => entries
            .map(|entry| {
                entry
                    .unwrap_or_else(|error| {
                        panic!("read an entry of {}: {error}", private_runs.display())
                    })
                    .path()
            })
            .collect(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => panic!("list {}: {error}", private_runs.display()),
    };
    assert_eq!(
        private_halves,
        Vec::<PathBuf>::new(),
        "no private half, committed or not: {said}"
    );
    let absent = |path: &Path| {
        matches!(
            fs::symlink_metadata(path),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound
        )
    };
    let namespace = repo.join(".upstroke");
    assert!(
        absent(&namespace),
        "no run exclusion and no public namespace: {} exists: {said}",
        namespace.display()
    );
    let home = private_root_for(repo);
    assert!(
        fs::symlink_metadata(&home).is_ok_and(|metadata| metadata.is_dir()),
        "the private root is still a directory: {}: {said}",
        home.display()
    );
    let held: Vec<PathBuf> = fs::read_dir(&home)
        .unwrap_or_else(|error| panic!("list {}: {error}", home.display()))
        .map(|entry| {
            entry
                .unwrap_or_else(|error| panic!("read an entry of {}: {error}", home.display()))
                .path()
        })
        .collect();
    assert_eq!(
        held,
        Vec::<PathBuf>::new(),
        "the private root is an empty directory: no include was published under it: {said}"
    );
    let lease = worktree_lock_path(repo);
    assert!(
        absent(&lease),
        "no worktree lease was taken: {} exists: {said}",
        lease.display()
    );
    said
}

fn host_policy() -> crate::topology::events::RunnerPolicy {
    crate::topology::events::RunnerPolicy {
        kind: crate::topology::events::RunnerKind::Host,
        policy: crate::topology::events::RunnerContract::HostV1,
        image: None,
        credential_volumes: None,
        mounts: Vec::new(),
    }
}

#[test]
fn a_fresh_schema_4_run_whose_pool_names_a_missing_profile_is_refused_before_anything_is_created() {
    let (tree, repo) = temp_engine_repo("topology-profile-missing");
    let missing = tree.path().join("no-such-profile");
    let mut opts = topology_options(&repo);
    opts.pools_path = Some(profiled_pool_file(&tree, &missing));
    opts.coordinator_environment = Some(no_oauth_token);
    let stub = CountedStub::new();

    let refused = run_with(&opts, &stub)
        .expect_err("a profile whose directory does not exist refuses the fresh run");
    let said = assert_a_profile_refusal_created_nothing(&refused, &repo, &stub);
    assert!(
        said.contains(&format!(
            "pool `claude-max` names the credential profile directory `{}`, which does not exist",
            missing.display()
        )),
        "the refusal names the pool and the path: {said}"
    );

    let mut pool = capacity::Pool::discovered(
        "claude-max",
        capacity::PoolKind::SubscriptionWindow,
        TOPOLOGY_STUB,
        Vec::new(),
    );
    pool.profile = Some(missing.display().to_string());
    let runner = crate::runner::host::HostRunner::new();
    let resumed = super::topology::preflight::RunPreflight::new(
        &runner,
        &stub,
        crate::gates::ShellKind::native(),
        &repo,
        vec![TOPOLOGY_STUB.to_owned()],
    )
    .with_pools(&[pool]);
    let on_resume =
        crate::runner::container::resolve::RunnerPreflight::certify(&resumed, &host_policy())
            .expect_err("a resume's pre-flight refuses the same profile");
    assert_eq!(
        said,
        on_resume.to_string(),
        "one refusal, worded once, on both paths"
    );
}

#[test]
fn a_fresh_schema_4_run_whose_pool_names_a_relative_profile_is_refused_before_anything_is_created()
{
    let (tree, repo) = temp_engine_repo("topology-profile-relative");
    let mut opts = topology_options(&repo);
    opts.pools_path = Some(profiled_pool_file(&tree, Path::new(".")));
    opts.coordinator_environment = Some(no_oauth_token);
    let stub = CountedStub::new();

    let refused = run_with(&opts, &stub)
        .expect_err("a relative profile refuses the fresh run, though `.` is a directory");
    let said = assert_a_profile_refusal_created_nothing(&refused, &repo, &stub);
    assert!(
        said.contains(
            "pool `claude-max` names the credential profile `.`, which is a relative path"
        ),
        "the refusal names the pool and the value: {said}"
    );
    assert!(
        !said.contains("resumable"),
        "the refusal precedes `create_run`, so there is no run to resume: {said}"
    );

    let mut pool = capacity::Pool::discovered(
        "claude-max",
        capacity::PoolKind::SubscriptionWindow,
        TOPOLOGY_STUB,
        Vec::new(),
    );
    pool.profile = Some(".".to_owned());
    let runner = crate::runner::host::HostRunner::new();
    let resumed = super::topology::preflight::RunPreflight::new(
        &runner,
        &stub,
        crate::gates::ShellKind::native(),
        &repo,
        vec![TOPOLOGY_STUB.to_owned()],
    )
    .with_pools(&[pool]);
    let on_resume =
        crate::runner::container::resolve::RunnerPreflight::certify(&resumed, &host_policy())
            .expect_err("a resume's pre-flight refuses the same profile");
    assert_eq!(
        said,
        on_resume.to_string(),
        "one refusal, worded once, on both paths"
    );
}

#[test]
fn a_fresh_schema_4_run_whose_profile_an_oauth_token_would_override_is_refused_before_anything_is_created()
 {
    let (tree, repo) = temp_engine_repo("topology-profile-token");
    let profile = tree.path().join("profile");
    fs::create_dir(&profile).expect("the profile directory, outside the checkout");
    let mut opts = topology_options(&repo);
    opts.pools_path = Some(profiled_pool_file(&tree, &profile));
    opts.coordinator_environment = Some(an_oauth_token_is_set);
    let stub = CountedStub::new();

    let refused = run_with(&opts, &stub)
        .expect_err("a token that would override the profile refuses the fresh run");
    let said = assert_a_profile_refusal_created_nothing(&refused, &repo, &stub);
    assert!(
        said.starts_with(
            "pre-flight: `CLAUDE_CODE_OAUTH_TOKEN` is set in this coordinator's environment"
        ),
        "the refusal names the variable: {said}"
    );
    assert!(
        said.contains("`claude-max`") && said.contains(&profile.display().to_string()),
        "and the pool and profile the run would not use: {said}"
    );
    assert!(
        !said.contains(FIXTURE_OAUTH_TOKEN),
        "a refusal never carries the credential itself: {said}"
    );

    opts.coordinator_environment = Some(no_oauth_token);
    let report = run_with(&opts, &stub)
        .expect("with no token set, the same profiled configuration creates the run");
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:#?}");
    until_released(&repo, &report.run_id);

    opts.pools_path = Some(stub_pool_file(&tree));
    opts.coordinator_environment = Some(an_oauth_token_is_set);
    let report = run_with(&opts, &stub)
        .expect("a pool that names no profile is not checked, so the token refuses nothing");
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:#?}");
    until_released(&repo, &report.run_id);
}

const ABSENT_ALTERNATIVE_CLI: &str = "upstroke-absent-codex-alternative-cli";

#[derive(Default)]
struct AbsentAlternativeCli {
    probes: std::sync::atomic::AtomicUsize,
    builds: std::sync::atomic::AtomicUsize,
}

impl AbsentAlternativeCli {
    fn asked(&self) -> (usize, usize) {
        (
            self.probes.load(std::sync::atomic::Ordering::SeqCst),
            self.builds.load(std::sync::atomic::Ordering::SeqCst),
        )
    }
}

impl AgentAdapter for AbsentAlternativeCli {
    fn id(&self) -> &'static str {
        crate::agent::codex::ADAPTER_ID
    }

    fn probe(&self, runner: &dyn crate::runner::Runner) -> Result<Caps, UpstrokeError> {
        self.probes
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        probe_an_absent_cli(
            crate::agent::codex::ADAPTER_ID,
            ABSENT_ALTERNATIVE_CLI,
            runner,
        )
    }

    fn build(&self, _run: &TaskRun) -> Result<CommandSpec, UpstrokeError> {
        self.builds
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Err(UpstrokeError::Agent {
            message: "an absent CLI is never built".to_owned(),
        })
    }

    fn parse(&self, _out: &ProcessOutput) -> Result<Outcome, UpstrokeError> {
        Err(UpstrokeError::Agent {
            message: "an absent CLI is never parsed".to_owned(),
        })
    }
}

const FRONTIER_IMPLEMENTS_CONFIG: &str =
    "[routing]\nimplement = { chain = [\"frontier\"], attempts_per = 1 }\n";

#[cfg(unix)]
struct RosterAdapters<'a> {
    worker: &'a dyn AgentAdapter,
    codex: &'a crate::agent::codex::fixture::ConfiguredCodex,
}

#[cfg(unix)]
impl AdapterSource for RosterAdapters<'_> {
    fn get(&self, id: &str) -> Option<&dyn AgentAdapter> {
        if id == self.worker.id() {
            Some(self.worker)
        } else {
            self.codex.get(id)
        }
    }
}

#[cfg(unix)]
#[test]
fn a_roster_upgrade_preserves_sequential_review_identity_until_a_compatible_retry() {
    use crate::agent::codex::fixture::{ConfiguredCodex, Roster};

    for (schema, recorded) in [(3, true), (2, true), (2, false)] {
        for mismatch in [Roster::Earlier, Roster::AddedEffortMissing] {
            let (tree, repo) = temp_engine_repo("roster-resume");
            seed(
                &repo,
                "## Work\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
                Some(&format!(
                    "{FRONTIER_IMPLEMENTS_CONFIG}\n[interaction]\nmode = \"never\"\n"
                )),
            );
            let codex = ConfiguredCodex::new(&tree.path().join("codex"));
            let initial_worker = TopologyStub::new(TOPOLOGY_STUB_EDIT, 1);
            let initial = run_with(
                &options(&repo),
                &RosterAdapters {
                    worker: &initial_worker,
                    codex: &codex,
                },
            )
            .expect("a compatible run parks with an independent reviewer");
            assert_eq!(initial.outcome(), RunOutcome::Parked);
            let paths = paths_of(&repo, &initial.run_id);
            let original = events::recorded_reviews(&events_of(&repo, &initial.run_id))
                .cloned()
                .expect("review plan recorded");
            assert_eq!(
                original
                    .alternative
                    .as_ref()
                    .map(|binding| binding.model.as_str()),
                Some("gpt-6-astra")
            );
            if schema == 2 {
                rewrite_run_started_as_schema_two_missing_review_fields(
                    &paths,
                    &["pass_timeout_secs", "enabled", "alternative_available"],
                );
                if !recorded {
                    strip_run_started_field(&paths, "reviews");
                }
            }
            let before = fs::read(paths.events()).expect("persisted run");
            let worker = CountedStub::new();
            codex.roster(mismatch);
            let error = resume_with(
                &resume_options(&repo, &initial.run_id),
                &RosterAdapters {
                    worker: &worker,
                    codex: &codex,
                },
            )
            .expect_err("the added requirement cannot erase an independent reviewer")
            .to_string();
            assert!(error.contains("codex roster upgrade"), "{error}");
            assert!(error.contains("Restore a compatible Codex CLI"), "{error}");
            assert!(error.contains("retry the original command"), "{error}");
            assert_eq!(worker.asked().1, 0, "no worker or primary review built");
            assert_eq!(fs::read(paths.events()).expect("unchanged log"), before);
            assert!(!codex.calls().lines().any(|line| line.contains("--model")));

            codex.roster(Roster::Full);
            let retry_worker = TopologyStub::new(TOPOLOGY_STUB_EDIT, 1);
            let answer = || {
                ScriptedAnswers::new(vec![Answer::Answered {
                    text: "continue with the original plan".to_owned(),
                }])
            };
            let retry = resume_harness(
                &resume_options(&repo, &initial.run_id),
                &Harness {
                    adapters: &RosterAdapters {
                        worker: &retry_worker,
                        codex: &codex,
                    },
                    answers: Some(&answer()),
                    sleeper: None,
                },
            )
            .expect("restoring the full roster permits the original plan");
            assert_eq!(retry.outcome(), RunOutcome::Parked);
            let established = events::recorded_complete_reviews(&events_of(&repo, &initial.run_id))
                .cloned()
                .expect("the restored retry records complete review identity");
            assert_eq!(established, original);
            let completed = resume_harness(
                &resume_options(&repo, &initial.run_id),
                &Harness {
                    adapters: &RosterAdapters {
                        worker: &worker,
                        codex: &codex,
                    },
                    answers: Some(&answer()),
                    sleeper: None,
                },
            )
            .expect("subsequent four-model resume replays the preserved plan");
            assert_eq!(completed.outcome(), RunOutcome::Complete);
            assert_eq!(task(&completed, "t1").review_models, ["gpt-6-astra"]);
            assert!(codex.calls().contains("--model gpt-6-astra"));
            assert_eq!(
                events::recorded_complete_reviews(&events_of(&repo, &initial.run_id)),
                Some(&original)
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn unrelated_codex_unavailability_keeps_the_sequential_optional_alternative_fallback() {
    use crate::agent::codex::fixture::{ConfiguredCodex, Roster};

    for unavailability in [
        Roster::EarlierModelMissing,
        Roster::EarlierEffortMissing,
        Roster::UnrelatedFailure,
        Roster::Malformed,
        Roster::Absent,
    ] {
        let (tree, repo) = temp_engine_repo("roster-optional");
        seed(
            &repo,
            "## Work\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
            Some(&format!(
                "{FRONTIER_IMPLEMENTS_CONFIG}\n[interaction]\nmode = \"never\"\n"
            )),
        );
        let codex = ConfiguredCodex::new(&tree.path().join("codex"));
        let initial_worker = TopologyStub::new(TOPOLOGY_STUB_EDIT, 1);
        let initial = run_with(
            &options(&repo),
            &RosterAdapters {
                worker: &initial_worker,
                codex: &codex,
            },
        )
        .expect("compatible initial run");
        assert_eq!(initial.outcome(), RunOutcome::Parked);
        codex.roster(unavailability);
        let worker = TopologyStub::new(TOPOLOGY_STUB_EDIT, 0);
        let answers = ScriptedAnswers::new(vec![Answer::Answered {
            text: "continue".to_owned(),
        }]);
        let resumed = resume_harness(
            &resume_options(&repo, &initial.run_id),
            &Harness {
                adapters: &RosterAdapters {
                    worker: &worker,
                    codex: &codex,
                },
                answers: Some(&answers),
                sleeper: None,
            },
        )
        .expect("pre-existing unavailability still permits optional fallback");
        assert_eq!(resumed.outcome(), RunOutcome::Complete);
        assert_eq!(task(&resumed, "t1").review_models, ["claude-opus-5"]);
        assert!(
            resumed
                .warnings
                .iter()
                .any(|warning| warning.contains("same-model review"))
        );
    }
}

#[cfg(unix)]
#[test]
fn schema_four_replays_its_codex_reviewer_after_roster_restoration() {
    use crate::agent::codex::fixture::{ConfiguredCodex, Roster};

    let (tree, repo) = temp_engine_repo("roster-schema4");
    seed(
        &repo,
        "## Work\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some(FRONTIER_IMPLEMENTS_CONFIG),
    );
    let codex = ConfiguredCodex::new(&tree.path().join("codex"));
    let initial_worker = TopologyStub::new(TOPOLOGY_STUB_EDIT, 1);
    let initial = run_with(
        &topology_options(&repo),
        &RosterAdapters {
            worker: &initial_worker,
            codex: &codex,
        },
    )
    .expect("schema-4 run with the full roster");
    assert_eq!(initial.outcome(), RunOutcome::Parked);
    let first = first_line_of_the_log(&repo, &initial.run_id);
    assert_eq!(first["data"]["schema"], 4);
    assert_eq!(
        first["data"]["probed_agents"],
        serde_json::json!(["claude-code", "codex"])
    );
    codex.roster(Roster::Earlier);
    let worker = CountedStub::new();
    let error = resume_with(
        &resume_options(&repo, &initial.run_id),
        &RosterAdapters {
            worker: &worker,
            codex: &codex,
        },
    )
    .expect_err("schema 4 still requires its recorded probe set")
    .to_string();
    assert!(error.contains("codex roster upgrade"), "{error}");
    assert_eq!(worker.asked().1, 0);
    codex.roster(Roster::Full);
    let answers = ScriptedAnswers::new(vec![Answer::Answered {
        text: "continue".to_owned(),
    }]);
    let resumed = resume_harness(
        &resume_options(&repo, &initial.run_id),
        &Harness {
            adapters: &RosterAdapters {
                worker: &worker,
                codex: &codex,
            },
            answers: Some(&answers),
            sleeper: None,
        },
    )
    .expect("schema-4 retry after restoring the roster");
    assert_eq!(resumed.outcome(), RunOutcome::Complete);
    assert_eq!(task(&resumed, "t1").review_models, ["gpt-6-astra"]);
    assert_eq!(first_line_of_the_log(&repo, &initial.run_id), first);
    until_released(&repo, &initial.run_id);
}

const FRONTIER_IMPLEMENTS_CROSS_FAMILY_OFF_CONFIG: &str = "[routing]\n\
     implement = { chain = [\"frontier\"], attempts_per = 1 }\n\
     review = { cross_family = false }\n";

#[test]
fn an_absent_alternative_reviewer_cli_refuses_creation_while_cross_family_is_unset() {
    let (_tree, repo) = temp_engine_repo("topology-absent-alternative-unset");
    seed(&repo, ONE_ATTEMPT_PLAN, Some(FRONTIER_IMPLEMENTS_CONFIG));
    let stub = TopologyStub::new(TOPOLOGY_STUB_EDIT, 0);
    let codex = AbsentAlternativeCli::default();
    let refused = run_with(
        &topology_options(&repo),
        &StubAndReviewer {
            stub: &stub,
            reviewer: &codex,
        },
    )
    .expect_err("with cross_family unset, the alternative's absent CLI refuses creation");
    let said = refused.to_string();
    assert!(
        said.contains("stopped at P3c"),
        "after the execution root's claim, before P4 certified: {said}"
    );
    assert!(said.contains(ABSENT_ALTERNATIVE_CLI), "{said}");
    assert!(said.contains("could not be executed"), "{said}");
    assert_eq!(
        codex.asked(),
        (1, 0),
        "the alternative was probed once and never built: {said}"
    );
    assert_a_refused_creation_left_nothing(&repo, &said);
}

#[test]
fn an_absent_alternative_reviewer_cli_is_never_probed_once_cross_family_is_false() {
    let (_tree, repo) = temp_engine_repo("topology-absent-alternative-off");
    seed(
        &repo,
        ONE_ATTEMPT_PLAN,
        Some(FRONTIER_IMPLEMENTS_CROSS_FAMILY_OFF_CONFIG),
    );
    let stub = TopologyStub::new(TOPOLOGY_STUB_EDIT, 0);
    let codex = AbsentAlternativeCli::default();
    let report = run_with(
        &topology_options(&repo),
        &StubAndReviewer {
            stub: &stub,
            reviewer: &codex,
        },
    )
    .expect("with cross_family = false, the alternative's absent CLI does not refuse creation");
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:#?}");
    assert!(
        committed(&report, "t1") && committed(&report, "t2"),
        "{report:#?}"
    );
    let first = first_line_of_the_log(&repo, &report.run_id);
    assert_eq!(
        first["data"]["probed_agents"],
        serde_json::json!(["claude-code"]),
        "{first}"
    );
    assert_eq!(
        codex.asked(),
        (0, 0),
        "the alternative's CLI was neither probed nor run"
    );
    for id in ["t1", "t2"] {
        assert_eq!(
            task(&report, id).review_models,
            ["claude-opus-5"],
            "the primary reviewed `{id}`, the work its own model implemented"
        );
    }
    let switched: Vec<&String> = report
        .warnings
        .iter()
        .filter(|warning| warning.contains("cross_family = false"))
        .collect();
    assert_eq!(
        switched.len(),
        1,
        "exactly one cross_family = false warning: {:?}",
        report.warnings
    );
    assert!(
        switched[0].contains("claude-code/claude-opus-5"),
        "it names the primary that reviews its own work: {switched:?}"
    );
    until_released(&repo, &report.run_id);
}

#[test]
fn a_fresh_run_on_an_unborn_branch_is_refused_before_it_creates_anything_and_a_born_one_runs() {
    let tree = temp_engine_scratch("topology-unborn");
    let repo = tree.path().join("repo");
    fs::create_dir(&repo).expect("the repository, a child of the guarded tree");
    git_in(&repo, &["init", "-q", "-b", "main"]);
    git_in(&repo, &["config", "user.email", "test@upstroke.local"]);
    git_in(&repo, &["config", "user.name", "upstroke tests"]);
    git_in(&repo, &["config", "maintenance.auto", "false"]);
    assert!(
        !repo
            .join(".git")
            .join("refs")
            .join("heads")
            .join("main")
            .exists()
            && !repo.join(".git").join("packed-refs").exists(),
        "the premise: the branch is unborn"
    );
    let plan = "## Implement the widget\n<!-- upstroke: id=t1 depends= -->\nMake it.\n";
    let outside = tree.path().join("plan.md");
    fs::write(&outside, plan).expect("the plan, outside the worktree, which stays clean");
    let mut opts = topology_options(&repo);
    opts.plan_path = outside;
    let home = private_root_for(&repo);

    let foreign_id = crate::ulid::ulid();
    let foreign = crate::workspace_manager::WorkspaceManager::derive(
        &repo,
        &home,
        &foreign_id,
        &crate::ulid::ulid(),
    )
    .expect("another run's manager");
    foreign
        .create_execution_root(&mut crate::workspace_manager::NoHooks)
        .expect("another run's root, in the same namespace");
    fs::write(
        foreign.execution_root().join("theirs.txt"),
        b"another run's bytes",
    )
    .expect("another run's content");
    let foreign_before = private_snapshot(foreign.execution_root());

    let stub = ObservedStub::new(0);
    let refused = run_with(&opts, &BorrowedObserved(&stub))
        .expect_err("an unborn branch has no base commit to record");
    let said = refused.to_string();
    assert!(
        said.contains("rev-parse") && said.contains("HEAD"),
        "the refusal is the base commit's: {said}"
    );
    assert_eq!(
        execution_roots_under(&home),
        vec![foreign_id.clone()],
        "the refused run left an execution root behind: {said}"
    );
    assert!(
        rundir::run_dir_names(&repo).is_empty(),
        "no run directory was published"
    );
    assert!(!home.join("runs").exists(), "no private half was made");
    assert!(stub.built().is_empty(), "nothing was built");
    assert_eq!(
        private_snapshot(foreign.execution_root()),
        foreign_before,
        "another run's root changed"
    );
    drop(WorktreeLock::acquire(&repo).expect("the lease is free after the refusal"));

    fs::write(repo.join("README.md"), "seed\n").expect("seed");
    fs::write(repo.join("plan.md"), plan).expect("the plan, now in the repository");
    git_in(&repo, &["add", "-A"]);
    git_in(&repo, &["commit", "-q", "-m", "seed"]);
    let source = SharedTopologyStubs(std::sync::Arc::new(TopologyStub::new(
        TOPOLOGY_STUB_EDIT,
        0,
    )));
    let report =
        run_with(&topology_options(&repo), &source).expect("a fresh run on the born branch");
    until_released(&repo, &report.run_id);
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:#?}");
    assert_eq!(
        recorded_execution_root(&repo, &report.run_id)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned()),
        Some(report.run_id.clone()),
        "the run created and recorded its own root"
    );
    assert_eq!(
        private_snapshot(foreign.execution_root()),
        foreign_before,
        "another run's root changed"
    );
}

/// A directory name no string spells: a byte UTF-8 never produces on Unix, an unpaired
/// surrogate on Windows.
#[cfg(unix)]
fn a_root_name_no_string_can_spell() -> std::ffi::OsString {
    use std::os::unix::ffi::OsStringExt as _;
    std::ffi::OsString::from_vec(b"private-\xff".to_vec())
}

#[cfg(windows)]
fn a_root_name_no_string_can_spell() -> std::ffi::OsString {
    use std::os::windows::ffi::OsStringExt as _;
    let mut units: Vec<u16> = "private-".encode_utf16().collect();
    units.push(0xD800);
    std::ffi::OsString::from_wide(&units)
}

/// J4, `PR12-NONUTF8-PRIVATE-ROOT-LOSSY-IDENTITY`. A schema-4 run records its private half and
/// execution root as text. At the reviewed head a private root holding a byte UTF-8 never
/// produces was recorded lossily, and the resume then looked for another directory. The root
/// is now refused before any lock, and afterwards nothing of the run exists: no run directory
/// in the repository and nothing under the root. A filesystem that refuses such a name
/// (`EILSEQ`, APFS) cannot hold the root at all. There the run is refused earlier, where the
/// root is first read, and the test asserts that refusal and that nothing of the run exists,
/// under a label naming the assertions such a host cannot reach, as the `EILSEQ` cell of
/// `runner/container/resolve/tests.rs` does.
#[test]
fn a_schema_4_run_refuses_a_private_root_no_string_can_spell_before_any_state() {
    let (tree, repo) = temp_engine_repo("topology-unspellable-root");
    let private = tree.path().join(a_root_name_no_string_can_spell());
    // Where the name exists, the run reaches the UTF-8 guard, and its refusal and the empty root
    // are asserted. Where creating it fails with `EILSEQ`, no such root can exist: the run is
    // refused where the root is first read (`prelock::authorized_private_root`), before the
    // UTF-8 guard, and the root is asserted to be still absent. Any other outcome fails.
    let (label, needle, created) = match fs::create_dir(&private) {
        Ok(()) => ("not UTF-8", "is not valid UTF-8".to_owned(), true),
        #[cfg(unix)]
        Err(error) if error.raw_os_error() == Some(libc::EILSEQ) => (
            "not UTF-8, refused by this host with EILSEQ: refused where the root is read, before \
             the UTF-8 guard, so the UTF-8 refusal and the empty-root check are not reached",
            format!("failed to read {}", private.display()),
            false,
        ),
        Err(error) => panic!("a private root UTF-8 cannot spell: {error}"),
    };
    let mut opts = topology_options(&repo);
    opts.private_root = Some(private.clone());
    let source = SharedTopologyStubs(std::sync::Arc::new(TopologyStub::new(
        TOPOLOGY_STUB_EDIT,
        0,
    )));
    let refused = run_with(&opts, &source).expect_err("an unspellable private root is refused");
    assert!(
        refused.to_string().contains(&needle),
        "{label}: the refusal says why: {refused}"
    );
    assert!(
        rundir::run_dir_names(&repo).is_empty(),
        "{label}: no run directory exists after the refusal"
    );
    if created {
        assert!(
            fs::read_dir(&private)
                .expect("the refused root is still readable")
                .next()
                .is_none(),
            "{label}: nothing was created under the refused root"
        );
    } else {
        assert!(
            fs::symlink_metadata(&private).is_err(),
            "{label}: the root this host refused was not created by the run"
        );
    }
}

/// J4's other side: a private root whose name holds U+FFFD is valid UTF-8, so it is not
/// refused, and the run records both paths that hang from it exactly. The resume finds the
/// run's private half through the recorded locator alone, and refuses only because the run
/// has finished. Each recorded path is spelled as the code that derives it spells it: the
/// private half from the root as `fs::canonicalize` resolves it (`prelock`), and the execution
/// root as the workspace manager spells the paths it derives, without Windows' verbatim prefix
/// (`fixture::canonical`). Native Windows CI run 41579577562 failed when this test compared the
/// execution root with the first spelling.
#[test]
fn a_private_root_named_with_the_replacement_character_is_recorded_exactly() {
    let (tree, repo) = temp_engine_repo("topology-replacement-root");
    let private = tree.path().join("private-\u{FFFD}");
    fs::create_dir(&private).expect("a private root whose name is valid UTF-8");
    let mut opts = topology_options(&repo);
    opts.private_root = Some(private.clone());
    let source = SharedTopologyStubs(std::sync::Arc::new(TopologyStub::new(
        TOPOLOGY_STUB_EDIT,
        0,
    )));
    let report = run_with(&opts, &source).expect("the run completes");
    until_released(&repo, &report.run_id);
    let canonical = fs::canonicalize(&private).expect("the private root canonicalizes");
    let recorded = first_line_of_the_log(&repo, &report.run_id);
    assert_eq!(
        recorded["data"]["private_dir"].as_str(),
        canonical.join("runs").join(&report.run_id).to_str(),
        "the private half is recorded exactly"
    );
    // The root as the workspace manager spells it: `fs::canonicalize` above, through
    // `strip_verbatim` on Windows and unchanged elsewhere.
    let managed_root = crate::workspace_manager::fixture::canonical(&private);
    let execution_root = recorded["data"]["execution_root"]
        .as_str()
        .expect("the execution root is recorded");
    assert!(
        Path::new(execution_root).starts_with(managed_root.join("workspaces")),
        "the execution root is recorded exactly under the root: {execution_root}"
    );
    let mut resume = resume_options(&repo, &report.run_id);
    resume.private_root = None;
    let refused =
        resume_with(&resume, &source).expect_err("a finished run has nothing to continue");
    assert!(
        refused.to_string().contains("already finished"),
        "the recorded locator names the run's private half: {refused}"
    );
}

#[cfg(unix)]
#[derive(Debug, Default, PartialEq, Eq)]
struct TreeSnapshot {
    entries: Vec<PathBuf>,
    unread: Vec<PathBuf>,
}

#[cfg(unix)]
fn every_path_under(root: &Path) -> TreeSnapshot {
    let not_found = |error: &std::io::Error| error.kind() == std::io::ErrorKind::NotFound;
    let relative = |path: &Path| {
        path.strip_prefix(root)
            .expect("an entry under the root")
            .to_path_buf()
    };
    let mut snapshot = TreeSnapshot::default();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let listed = match fs::read_dir(&directory) {
            Ok(listed) => listed,
            Err(error) if not_found(&error) => {
                snapshot.unread.push(relative(&directory));
                continue;
            }
            Err(error) => panic!("list {}: {error}", directory.display()),
        };
        for entry in listed {
            let entry = entry
                .unwrap_or_else(|error| panic!("an entry of {}: {error}", directory.display()));
            let path = entry.path();
            match entry.file_type() {
                Ok(kind) => {
                    if kind.is_dir() {
                        pending.push(path.clone());
                    }
                    snapshot.entries.push(relative(&path));
                }
                Err(error) if not_found(&error) => snapshot.unread.push(relative(&path)),
                Err(error) => panic!("the type of {}: {error}", path.display()),
            }
        }
    }
    snapshot.entries.sort();
    snapshot.unread.sort();
    snapshot
}

#[cfg(unix)]
#[test]
fn the_tree_snapshot_lists_by_the_platforms_own_names_and_types_and_notes_what_it_cannot_read() {
    let tree = temp_engine_scratch("tree-snapshot");
    let directory = tree.path().join("d");
    fs::create_dir(&directory).expect("a directory");
    fs::write(directory.join("f"), "a file\n").expect("a file");
    std::os::unix::fs::symlink(tree.path().join("absent"), tree.path().join("dangling"))
        .expect("a link to nothing");
    std::os::unix::fs::symlink(&directory, tree.path().join("linked"))
        .expect("a link to the directory");
    assert_eq!(
        every_path_under(tree.path()),
        TreeSnapshot {
            entries: ["d", "d/f", "dangling", "linked"]
                .into_iter()
                .map(PathBuf::from)
                .collect(),
            unread: Vec::new(),
        },
        "every entry by its listed name, links not followed"
    );
    assert_eq!(
        every_path_under(&tree.path().join("absent")),
        TreeSnapshot {
            entries: Vec::new(),
            unread: vec![PathBuf::new()],
        },
        "a root that cannot be read is noted, not a panic"
    );
}

#[cfg(unix)]
#[test]
fn a_schema_4_run_refuses_an_unspellable_default_private_root_before_it_creates_anything() {
    for home_exists in [true, false] {
        let (tree, repo) = temp_engine_repo(&format!(
            "topology-unspellable-home-{}",
            u8::from(home_exists)
        ));
        let home = tree.path().join(a_root_name_no_string_can_spell());
        if home_exists {
            match fs::create_dir(&home) {
                Ok(()) => {}
                Err(error) if error.raw_os_error() == Some(libc::EILSEQ) => continue,
                Err(error) => panic!("a home directory UTF-8 cannot spell: {error}"),
            }
        }
        let snapshot = || (every_path_under(tree.path()), every_path_under(&home));
        let before = snapshot();
        let ended = crate::agent::proc::test_support::run_test_isolated(
            "engine::tests::unspellable_default_root_child",
            &[
                ("HOME", home.as_os_str()),
                ("USERPROFILE", home.as_os_str()),
                (UNSPELLABLE_HOME_REPO, repo.as_os_str()),
            ],
            Duration::from_secs(300),
        );
        let after = snapshot();
        assert_eq!(
            (&after.0.entries, &after.1.entries),
            (&before.0.entries, &before.1.entries),
            "home exists: {home_exists}: the refusal created nothing, the recorded-objects \
             include included. Listed and not read, under the tree and the home: before {:?}, \
             after {:?}: {ended}",
            (&before.0.unread, &before.1.unread),
            (&after.0.unread, &after.1.unread)
        );
        assert!(
            ended.status.is_some_and(|status| status.success())
                && ended.stdout.contains("1 passed"),
            "home exists: {home_exists}: the run is refused for its default root: {ended}"
        );
        assert!(
            fs::symlink_metadata(home.join(".upstroke")).is_err(),
            "home exists: {home_exists}: no default root under the home"
        );
    }
}

#[cfg(unix)]
#[test]
fn the_default_root_check_judges_the_path_the_root_will_resolve_to() {
    use std::os::unix::ffi::OsStrExt as _;

    let check = crate::engine::topology::prelock::refuse_unspellable_default_root;
    let tree = temp_engine_scratch("default-root-spelling");
    let home = tree.path().join("home");
    fs::create_dir(&home).expect("a home text spells");
    check(&home.join(".upstroke")).expect("a missing root under a home text spells");
    check(&home).expect("an existing root text spells");

    let unspellable = tree.path().join(std::ffi::OsStr::from_bytes(b"home-\xff"));
    let refused = check(&unspellable.join(".upstroke"))
        .expect_err("a missing root under a missing home no string spells");
    assert!(
        refused.to_string().contains("is not valid UTF-8")
            && refused
                .to_string()
                .contains("Nothing was locked or created"),
        "{refused}"
    );
    let linked = match std::os::unix::fs::symlink(&home, &unspellable) {
        Ok(()) => {
            check(&unspellable.join(".upstroke"))
                .expect("a link no string spells that resolves to a home text spells");
            true
        }
        Err(error) if error.raw_os_error() == Some(libc::EILSEQ) => false,
        Err(error) => panic!("a link whose name no string spells: {error}"),
    };
    let mut expected = vec![PathBuf::from("home")];
    if linked {
        expected.push(PathBuf::from(std::ffi::OsStr::from_bytes(b"home-\xff")));
    }
    expected.sort();
    assert_eq!(
        every_path_under(tree.path()),
        TreeSnapshot {
            entries: expected,
            unread: Vec::new(),
        },
        "the checks created nothing"
    );
}

#[cfg(unix)]
const INHERITED_COMMON_IMAGE: &str = "upstroke-tests/inherited-common:1";

#[cfg(unix)]
fn run_directory_names(repo: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(rundir::runs_root(repo))
        .expect("the runs root")
        .map(|entry| {
            entry
                .expect("an entry of the runs root")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

#[cfg(unix)]
fn record_a_container_runner_mounting(repo: &Path, run_id: &str, mounts: &[(&Path, &str)]) {
    use crate::topology::events::{
        ImageIdentity, MountSourceIdentity, MountSourceKind, RecordedMount, RunnerContract,
        RunnerKind, RunnerPolicy, TopologyEvent, TopologyEventBody,
    };
    use std::os::unix::fs::MetadataExt as _;

    let runner = RunnerPolicy {
        kind: RunnerKind::Container,
        policy: RunnerContract::ContainerV1,
        image: Some(ImageIdentity {
            reference: INHERITED_COMMON_IMAGE.to_owned(),
            id: format!("sha256:{}", "a".repeat(64)),
            digest: None,
        }),
        credential_volumes: Some(std::collections::BTreeMap::new()),
        mounts: mounts
            .iter()
            .map(|(source, target)| {
                let canonical = fs::canonicalize(source).expect("the mounted directory resolves");
                let metadata = fs::metadata(&canonical).expect("the mounted directory");
                RecordedMount {
                    source: canonical.to_str().expect("a UTF-8 source").to_owned(),
                    target: (*target).to_owned(),
                    read_only: false,
                    source_identity: MountSourceIdentity::Unix {
                        kind: MountSourceKind::Directory,
                        device: metadata.dev(),
                        inode: metadata.ino(),
                    },
                }
            })
            .collect(),
    };
    let log = rundir::public_dir(repo, run_id).join("events.jsonl");
    let bytes = fs::read(&log).expect("the parked run's log");
    let end = bytes
        .iter()
        .position(|byte| *byte == b'\n')
        .expect("a committed first line");
    let (first, rest) = bytes.split_at(end);
    let first = std::str::from_utf8(first).expect("a UTF-8 first line");
    let event: TopologyEvent = serde_json::from_str(first).expect("the first line is an event");
    let TopologyEventBody::RunStarted { data } = event.body else {
        panic!("the first line is not `run_started`: {first}");
    };
    let recorded = serde_json::to_string(&data.runner).expect("the recorded runner serializes");
    assert_eq!(
        first.matches(&recorded).count(),
        1,
        "the first line spells its runner `{recorded}` exactly once: {first}"
    );
    let mut planted = first
        .replacen(
            &recorded,
            &serde_json::to_string(&runner).expect("the container runner serializes"),
            1,
        )
        .into_bytes();
    planted.extend_from_slice(rest);
    fs::write(&log, planted).expect("the container runner recorded");
}

#[cfg(unix)]
#[test]
fn a_mount_inside_an_inherited_common_directory_is_refused_by_run_and_resume_before_the_runtime_is_asked()
 {
    let (tree, repo) = temp_engine_repo("inherited-common-mount");
    git_in(&repo, &["config", "maintenance.auto", "false"]);
    let source = SharedTopologyStubs(std::sync::Arc::new(TopologyStub::new(
        TOPOLOGY_STUB_EDIT,
        1,
    )));
    let parked = run_with(&topology_options(&repo), &source).expect("the run parks");
    assert_eq!(parked.outcome(), RunOutcome::Parked, "{parked:#?}");
    until_released(&repo, &parked.run_id);

    let common = repo.with_file_name("common");
    copy_tree_for_a_layout(&repo.join(".git"), &common);
    let refs = common.join("refs");
    fs::write(
        repo.with_file_name("container.toml"),
        format!(
            "[runner]\nkind = \"container\"\nimage = \"{INHERITED_COMMON_IMAGE}\"\n\
             mounts = [{{ source = \"{}\", target = \"/data\", read_only = false }}]\n",
            refs.display()
        ),
    )
    .expect("a container configuration mounting the common directory's refs");
    record_a_container_runner_mounting(&repo, &parked.run_id, &[(refs.as_path(), "/data")]);

    let bin = tree.path().join("bin");
    fs::create_dir(&bin).expect("the stub runtime's directory");
    let asked = tree.path().join("runtime-asked");
    crate::runner::container::write_program_in_its_own_process(
        &bin.join(crate::runner::container::DOCKER_PROGRAM),
        &format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nexit 93\n",
            asked.display()
        ),
    );
    let inherited = std::env::var_os("PATH").unwrap_or_default();
    let path = std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(&inherited)))
        .expect("a PATH with the stub runtime first");

    let log = rundir::public_dir(&repo, &parked.run_id).join("events.jsonl");
    let before = fs::read(&log).expect("the recorded log");
    let runs = run_directory_names(&repo);
    for (spelling, value) in [
        ("absolute", common.as_os_str()),
        ("relative", std::ffi::OsStr::new("../common")),
    ] {
        let refusals = tree.path().join(format!("refusals-{spelling}"));
        let ended = crate::agent::proc::test_support::run_test_isolated(
            "engine::tests::inherited_common_dir_mount_child",
            &[
                ("GIT_COMMON_DIR", value),
                ("PATH", path.as_os_str()),
                (INHERITED_RUN_ROOT, repo.as_os_str()),
                (INHERITED_RUN_ID, std::ffi::OsStr::new(&parked.run_id)),
                (INHERITED_COMMON_REFUSALS, refusals.as_os_str()),
            ],
            Duration::from_secs(300),
        );
        assert!(
            ended.status.is_some_and(|status| status.success())
                && ended.stdout.contains("1 passed"),
            "under the {spelling} GIT_COMMON_DIR the run and the resume refused the mount: {ended}"
        );
        let said = fs::read_to_string(&refusals).unwrap_or_else(|error| {
            panic!("the {spelling} cell's child recorded no refusals ({error}): {ended}")
        });
        let canonical = fs::canonicalize(&common).expect("the relocated common directory");
        assert_eq!(
            said.matches(&format!("lies inside `{}`", canonical.display()))
                .count(),
            2,
            "the {spelling} GIT_COMMON_DIR: both refusals name the common directory:\n{said}"
        );
    }
    assert!(
        !asked.exists(),
        "the container runtime was asked: {}",
        fs::read_to_string(&asked).unwrap_or_default()
    );
    assert_eq!(
        fs::read(&log).expect("the recorded log"),
        before,
        "a refused command wrote the recorded log"
    );
    assert_eq!(
        run_directory_names(&repo),
        runs,
        "a refused run created a run directory"
    );
}

#[cfg(unix)]
#[test]
fn a_recorded_mount_whose_target_a_fresh_run_refuses_is_refused_by_the_resume_before_the_runtime_is_asked()
 {
    use crate::runner::container::resolve::fixtures::REFUSED_RECORDED_TARGETS;

    let (tree, repo) = temp_engine_repo("recorded-target-resume");
    git_in(&repo, &["config", "maintenance.auto", "false"]);
    let source = SharedTopologyStubs(std::sync::Arc::new(TopologyStub::new(
        TOPOLOGY_STUB_EDIT,
        1,
    )));
    let parked = run_with(&topology_options(&repo), &source).expect("the run parks");
    assert_eq!(parked.outcome(), RunOutcome::Parked, "{parked:#?}");
    until_released(&repo, &parked.run_id);

    let shared = tree.path().join("shared");
    fs::create_dir(&shared).expect("a directory the record mounts");
    let bin = tree.path().join("bin");
    fs::create_dir(&bin).expect("the stub runtime's directory");
    let asked = tree.path().join("runtime-asked");
    crate::runner::container::write_program_in_its_own_process(
        &bin.join(crate::runner::container::DOCKER_PROGRAM),
        &format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nexit 93\n",
            asked.display()
        ),
    );
    let inherited = std::env::var_os("PATH").unwrap_or_default();
    let path = std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(&inherited)))
        .expect("a PATH with the stub runtime first");

    let log = rundir::public_dir(&repo, &parked.run_id).join("events.jsonl");
    let runs = run_directory_names(&repo);
    for (index, (targets, needle)) in REFUSED_RECORDED_TARGETS.iter().enumerate() {
        let mounts: Vec<(&Path, &str)> = targets
            .iter()
            .map(|target| (shared.as_path(), *target))
            .collect();
        record_a_container_runner_mounting(&repo, &parked.run_id, &mounts);
        let before = fs::read(&log).expect("the log recording the container runner");
        let refusal = tree.path().join(format!("refusal-{index}"));
        let ended = crate::agent::proc::test_support::run_test_isolated(
            "engine::tests::recorded_target_resume_child",
            &[
                ("PATH", path.as_os_str()),
                (INHERITED_RUN_ROOT, repo.as_os_str()),
                (INHERITED_RUN_ID, std::ffi::OsStr::new(&parked.run_id)),
                (RECORDED_TARGET_REFUSAL, refusal.as_os_str()),
            ],
            Duration::from_secs(300),
        );
        assert!(
            ended.status.is_some_and(|status| status.success())
                && ended.stdout.contains("1 passed"),
            "{targets:?}: an isolated child resumed the run under the stub runtime: {ended}"
        );
        let said = fs::read_to_string(&refusal).unwrap_or_else(|error| {
            panic!("{targets:?}: the child recorded no refusal ({error}): {ended}")
        });
        assert!(
            said.contains(&format!("run `{}` records the mount", parked.run_id))
                && said.contains(needle)
                && said.contains("A fresh run refuses that target")
                && said.contains("nothing was published, locked or written"),
            "{targets:?}: the resume refuses the recorded target by the rule a fresh run \
             applies: {said}"
        );
        assert_eq!(
            fs::read(&log).expect("the recorded log"),
            before,
            "{targets:?}: a refused resume wrote the recorded log"
        );
    }
    assert!(
        !asked.exists(),
        "the container runtime was asked: {}",
        fs::read_to_string(&asked).unwrap_or_default()
    );
    assert_eq!(
        run_directory_names(&repo),
        runs,
        "a refused resume created a run directory"
    );
}
