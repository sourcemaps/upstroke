//! Extended notes: `docs/internals/engine/tests/snapshot_registration.md`

use super::*;

#[test]
fn the_v1_conductor_runs_and_resumes_on_the_graph_its_own_workspace_wrote() {
    let status = crate::workspace_manager::fixture::run_replacement_witness_child(
        "engine::tests::v1_object_graph_helper",
    );
    assert!(
        status.success(),
        "the child drives `engine::run_harness` and `engine::resume_harness` over a \
         repository whose committed probe blob carries a replacement, and ended \
         {status:?}"
    );
}

#[test]
fn a_checkout_a_replacement_rewrote_is_refused_with_a_check_the_reader_can_run() {
    let status = crate::workspace_manager::fixture::run_replacement_witness_child(
        "engine::tests::replaced_head_refusal_helper",
    );
    assert!(
        status.success(),
        "the child refuses a run, and a resume off the run branch, over a checkout that \
         a replacement of HEAD rewrote, and ended {status:?}"
    );
}

fn challenge_every_scope(truthful: &Path, probe: &Path) -> Vec<(&'static str, std::ffi::OsString)> {
    vec![
        ("GIT_CONFIG_SYSTEM", truthful.as_os_str().to_owned()),
        ("GIT_CONFIG_NOSYSTEM", "0".into()),
        ("GIT_CONFIG_GLOBAL", truthful.as_os_str().to_owned()),
        ("GIT_CONFIG_COUNT", "1".into()),
        ("GIT_CONFIG_KEY_0", "core.useReplaceRefs".into()),
        ("GIT_CONFIG_VALUE_0", "true".into()),
        (
            "GIT_CONFIG_PARAMETERS",
            "'core.useReplaceRefs'='true'".into(),
        ),
        (ROLE_PROBE, probe.as_os_str().to_owned()),
    ]
}

#[test]
fn the_v1_conductor_keeps_every_role_on_the_recorded_graph_of_its_own_repository() {
    let tree = temp_engine_scratch("v1-role-graph");
    let truthful = tree.path().join("true.gitconfig");
    fs::write(&truthful, "[core]\n\tuseReplaceRefs = true\n").expect("a `true` for two scopes");
    let status = crate::workspace_manager::fixture::run_challenged_replacement_witness_child(
        "engine::tests::v1_role_graph_helper",
        &challenge_every_scope(&truthful, tree.path()),
    );
    assert!(
        status.success(),
        "the child runs and resumes a v0.1 run whose worker, gates and reviewers each read \
         a repository with blob, tree and commit replacements, under `core.useReplaceRefs = \
         true` at every scope, and ended {status:?}"
    );
}

#[test]
fn a_gate_that_replaces_objects_in_its_own_fixture_passes_under_the_v1_runner() {
    let tree = temp_engine_scratch("v1-own-fixture");
    let status = crate::workspace_manager::fixture::run_challenged_replacement_witness_child(
        "engine::tests::v1_own_fixture_helper",
        &[(ROLE_PROBE, tree.path().as_os_str().to_owned())],
    );
    assert!(
        status.success(),
        "the child runs a v0.1 task in a repository with nothing under `refs/replace/`, \
         whose worker, gate and reviewer each create a repository of their own, install a \
         replacement in it and read it back, and ended {status:?}"
    );
}

#[test]
fn a_refused_an_interrupted_and_a_resumed_v1_run_leave_git_configuration_as_they_found_it() {
    let tree = temp_engine_scratch("v1-configuration");
    let truthful = tree.path().join("true.gitconfig");
    fs::write(&truthful, "[core]\n\tuseReplaceRefs = true\n").expect("a `true` for two scopes");
    let status = crate::workspace_manager::fixture::run_challenged_replacement_witness_child(
        "engine::tests::v1_configuration_helper",
        &challenge_every_scope(&truthful, tree.path()),
    );
    assert!(
        status.success(),
        "the child refuses a run over a dirty checkout, kills one inside its attempt and \
         resumes it, comparing every Git configuration file after each, and ended {status:?}"
    );
}

#[test]
fn sibling_v1_runs_in_one_repository_share_one_include_and_keep_their_roles_recorded() {
    let tree = temp_engine_scratch("v1-siblings");
    let status = crate::workspace_manager::fixture::run_challenged_replacement_witness_child(
        "engine::tests::v1_siblings_helper",
        &[(ROLE_PROBE, tree.path().as_os_str().to_owned())],
    );
    assert!(
        status.success(),
        "the child starts two v0.1 runs at once, in two linked worktrees of one repository \
         with replacements, over one private root with no include in it yet, and ended \
         {status:?}"
    );
}

#[test]
fn a_gates_worktree_setting_is_attempted_again_only_past_another_registrations_empty_commondir() {
    let dir = Path::new("/r/store/worktrees/upstroke-gates-1-01AAAAAAAAAAAAAAAAAAAAAAAA");
    let other = "fatal: failed to read /r/.git/worktrees/upstroke-gates-2-01BBBBBBBBBBBBBBBBBBBBBBBB\
                 /commondir: Success\n";
    let own = "fatal: failed to read /r/.git/worktrees/upstroke-gates-1-01AAAAAAAAAAAAAAAAAAAAAAAA\
               /commondir: Success\n";
    let numbered = "fatal: failed to read /r/.git/worktrees/upstroke-gates-1-01AAAAAAAAAAAAAAAAAAAAAAAA1\
                    /commondir: Success\n";
    #[cfg(unix)]
    let exited = |code: i32| {
        <std::process::ExitStatus as std::os::unix::process::ExitStatusExt>::from_raw(code << 8)
    };
    #[cfg(windows)]
    let exited = |code: u32| {
        <std::process::ExitStatus as std::os::windows::process::ExitStatusExt>::from_raw(code)
    };
    #[cfg(unix)]
    let signalled =
        Some(<std::process::ExitStatus as std::os::unix::process::ExitStatusExt>::from_raw(15));
    #[cfg(not(unix))]
    let signalled: Option<std::process::ExitStatus> = None;
    let output = |status, stderr: &str| std::process::Output {
        status,
        stdout: Vec::new(),
        stderr: stderr.as_bytes().to_vec(),
    };
    let far = || std::time::Instant::now() + Duration::from_secs(60);
    let attempt = |deadline: std::time::Instant,
                   script: Vec<std::process::Output>|
     -> (Result<std::process::Output, String>, usize) {
        let mut script = script.into_iter();
        let mut probes = 0;
        let read = past_another_registrations_write(dir, deadline, &mut || {
            probes += 1;
            script
                .next()
                .ok_or_else(|| std::io::Error::other("probed past the script"))
        });
        (read.map_err(|error| error.to_string()), probes)
    };
    let torn = output(exited(128), other);
    let set = output(exited(0), "");

    assert_eq!(
        attempt(far(), vec![torn.clone(), torn.clone(), set.clone()]),
        (Ok(set.clone()), 3),
        "past two tears of another registration"
    );
    let at_once = [
        (
            output(exited(1), other),
            "the same line, with an exit that is not Git's death",
        ),
        (
            output(exited(128), other.trim_end_matches('\n')),
            "the same line, unfinished",
        ),
        (
            output(exited(128), &format!("{other}\n")),
            "the same line and an empty one after it",
        ),
        (
            output(exited(128), &format!("warning: w924\n{other}")),
            "a second line",
        ),
        (
            output(
                exited(128),
                &format!("fatal: failed to read /r/one\n{other}"),
            ),
            "two lines, each Git's death",
        ),
        (output(exited(128), own), "its own registration"),
        (
            output(exited(128), numbered),
            "its own, under the suffix Git gives a taken name",
        ),
        (
            output(
                exited(128),
                "fatal: failed to read /r/.git/worktrees/w/commondir: No such file or directory\n",
            ),
            "a removal's",
        ),
        (
            output(
                exited(128),
                "fatal: failed to read '/r/.git/worktrees/w/locked'\n",
            ),
            "a lock's",
        ),
        (
            output(
                exited(128),
                "fatal: --worktree cannot be used with multiple working trees\n",
            ),
            "another refusal",
        ),
    ]
    .into_iter()
    .chain(signalled.map(|status| {
        (
            output(status, other),
            "the same line from a Git a signal ended",
        )
    }));
    for (answer, why) in at_once {
        assert_eq!(
            attempt(far(), vec![answer.clone(), set.clone()]),
            (Ok(answer), 1),
            "{why}, at once"
        );
    }
    assert_eq!(
        attempt(std::time::Instant::now(), vec![torn.clone(), set.clone()]),
        (Ok(torn), 1),
        "a deadline already past"
    );
    assert_eq!(
        attempt(far(), vec![set.clone()]),
        (Ok(set), 1),
        "a success, at once"
    );
    assert_eq!(
        attempt(far(), Vec::new()),
        (Err("probed past the script".to_owned()), 1),
        "a probe that cannot start, at once"
    );
}

#[test]
fn a_gates_worktree_setting_answers_what_the_probe_answers_when_nothing_tears() {
    let (_tree, repo) = temp_engine_repo("w924-probe-answers");
    let missing = repo.join("w924-no-such-directory");
    for (dir, args) in [
        (repo.as_path(), &["rev-parse", "--is-inside-work-tree"][..]),
        (
            repo.as_path(),
            &["rev-parse", "--verify", "w924-no-such-revision"],
        ),
        (repo.as_path(), &["config", "--w924-no-such-option"]),
        (
            missing.as_path(),
            &["config", "--worktree", "core.useReplaceRefs", "true"],
        ),
    ] {
        assert_eq!(
            probe_git_past_another_registrations_write(dir, args),
            probe_git(dir, args),
            "{args:?} in {}",
            dir.display()
        );
    }
}

#[cfg(target_os = "linux")]
#[test]
fn the_gate_role_probe_stand_in_writer_leaves_no_writer_in_another_threads_fork() {
    let ended = crate::agent::proc::test_support::run_test_isolated(
        "engine::tests::gate_probe_writer_helper",
        &[],
        Duration::from_secs(180),
    );
    assert!(
        ended.status.is_some_and(|status| status.success()),
        "the isolated inherited-writer witness ended within its bound: {ended}"
    );
    assert!(
        ended.stdout.contains("1 passed"),
        "the helper must run one test: {ended}"
    );
}

#[cfg(unix)]
#[test]
fn the_gate_role_sets_its_worktree_value_again_only_past_another_registrations_empty_commondir() {
    let tree = temp_engine_scratch("w924-gate-probe");
    let ended = crate::agent::proc::test_support::run_test_isolated(
        "engine::tests::the_gate_role_probe_cases_child",
        &[(GATE_PROBE_TREE, tree.path().as_os_str())],
        Duration::from_secs(300),
    );
    assert!(
        ended.status.is_some_and(|status| status.success()) && ended.stdout.contains("1 passed"),
        "an isolated child ran the gate role's cases within its bound: {ended}"
    );
    assert!(
        tree.path().join("ran").exists(),
        "the isolated child reached the end of its cases: {ended}"
    );
}

#[test]
fn the_v1_include_names_the_managed_repository_however_its_path_is_spelled() {
    let tree = temp_engine_scratch("v1-shapes");
    let status = crate::workspace_manager::fixture::run_challenged_replacement_witness_child(
        "engine::tests::v1_shapes_helper",
        &[(ROLE_PROBE, tree.path().as_os_str().to_owned())],
    );
    assert!(
        status.success(),
        "the child runs a v0.1 task in repositories with replacements whose paths carry \
         glob characters, spaces and a quote, that are entered through a symbolic link, \
         keep their Git directory elsewhere or behind a link, are linked worktrees, or are \
         spelled in another case, and ended {status:?}"
    );
}

fn unstage_and_tear_after_capture(
    workspace: &Workspace,
    candidate: &crate::workspace::CapturedCandidate,
) -> Result<(), UpstrokeError> {
    record_the_capture(workspace, candidate)?;
    git_after_capture(
        workspace.root(),
        &["reset", "-q", "HEAD", "--", "agent-output.txt"],
    )?;
    plant_a_torn_registration_in(workspace.root())
        .map(|_| ())
        .map_err(after_capture_failed)
}

fn block_the_snapshot_store_after_capture(
    workspace: &Workspace,
    candidate: &crate::workspace::CapturedCandidate,
) -> Result<(), UpstrokeError> {
    record_the_capture(workspace, candidate)?;
    let run_id = rundir::latest_run(workspace.root())
        .ok_or_else(|| after_capture_failed("no run has started"))?;
    let store = paths_of(workspace.root(), &run_id).gate_worktrees();
    fs::create_dir_all(&store).map_err(after_capture_failed)?;
    let intents = store.join("intents");
    if intents.is_dir() {
        fs::remove_dir_all(&intents).map_err(after_capture_failed)?;
    }
    fs::write(&intents, "not a directory\n").map_err(after_capture_failed)
}

fn replace_the_new_blob_and_tear_after_capture(
    workspace: &Workspace,
    candidate: &crate::workspace::CapturedCandidate,
) -> Result<(), UpstrokeError> {
    record_the_capture(workspace, candidate)?;
    let root = workspace.root();
    let listing = git_after_capture(
        root,
        &[
            "-c",
            "core.useReplaceRefs=false",
            "ls-tree",
            &candidate.tree_oid,
            "new.txt",
        ],
    )?;
    let original = listing
        .split_whitespace()
        .nth(2)
        .ok_or_else(|| after_capture_failed("no new.txt in the captured tree"))?
        .to_owned();
    let other = root.with_file_name("replacement.txt");
    fs::write(&other, "replacement content\n").map_err(after_capture_failed)?;
    let replacement = git_after_capture(root, &["hash-object", "-w", &other.to_string_lossy()])?;
    git_after_capture(root, &["replace", &original, &replacement])?;
    plant_a_torn_registration_in(root)
        .map(|_| ())
        .map_err(after_capture_failed)
}

fn replace_the_captured_tree_and_tear_after_capture(
    workspace: &Workspace,
    candidate: &crate::workspace::CapturedCandidate,
) -> Result<(), UpstrokeError> {
    record_the_capture(workspace, candidate)?;
    let root = workspace.root();
    let parent_tree = git_after_capture(
        root,
        &[
            "-c",
            "core.useReplaceRefs=false",
            "rev-parse",
            &format!("{}^{{tree}}", candidate.parent_oid),
        ],
    )?;
    git_after_capture(root, &["replace", &candidate.tree_oid, &parent_tree])?;
    plant_a_torn_registration_in(root)
        .map(|_| ())
        .map_err(after_capture_failed)
}

fn refusal_options(
    repo: &Path,
    after_capture: super::options::AfterCandidateCapture,
) -> RunOptions {
    let mut opts = options(repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    opts.after_candidate_capture = Some(after_capture);
    opts
}

fn follow_the_pick(repo: &Path, warning: &str, pin: &str) -> String {
    let commands = advertised_commands(warning, pin);
    let restore = commands
        .iter()
        .position(|command| matches!(subcommand_of(command).as_str(), "restore" | "checkout"))
        .expect("the warning advertises a restore");
    let command = commands
        .into_iter()
        .skip(restore + 1)
        .find(|command| !matches!(subcommand_of(command).as_str(), "update-ref" | "diff"))
        .expect("the warning advertises a command for a later HEAD");
    assert!(run_the_advertised(repo, &command).success(), "{command}");
    command
}

#[test]
fn a_registry_refusal_after_capture_keeps_and_pins_the_captured_candidate_across_both_resumes() {
    let (_tree, repo) = temp_engine_repo("kept-static");
    one_gated_task(&repo);
    let run = run_with(
        &refusal_options(&repo, tear_after_capture),
        &fake(Effect::EditFile),
    );
    let refusal = registry_refusal_text(&run, "the run beside a tear that stays");
    let (_, parent, tree) = captured(&repo);
    let status = status_of(&repo);
    let kept = kept_pins(&repo);
    let [(pin, commit)] = kept.as_slice() else {
        panic!("one kept pin: {kept:?}");
    };
    assert!(
        refusal.contains(pin.as_str()) && refusal.contains("is kept in this checkout"),
        "the refusal names the pin: {refusal}"
    );
    assert!(
        status.contains("agent-output.txt"),
        "the checkout still holds the candidate: {status}"
    );
    assert_eq!(
        tree_of(&repo, commit),
        tree,
        "the pin holds the captured tree"
    );
    assert_eq!(
        git_in(&repo, &["rev-parse", &format!("{commit}^")]).trim(),
        parent,
        "on the captured parent"
    );

    let run_id = rundir::latest_run(&repo).expect("the run started");
    let while_torn = resume_the_run(&repo, &run_id, &fake(Effect::EditFile));
    registry_refusal_text(&while_torn, "the resume while the residue stays");
    assert_eq!(
        status_of(&repo),
        status,
        "the refused resume discards nothing"
    );
    assert_eq!(kept_pins(&repo), kept, "and moves no pin");

    repair_the_torn_registration(&repo);
    let report = resume_the_run(&repo, &run_id, &fake(Effect::EditFile))
        .expect("the resume after the repair");
    assert!(committed(&report, "t1"), "{report:?}");
    let warning = kept_pin_warning(&report).expect("the resume names the kept pin");
    assert!(warning.contains(pin.as_str()), "{warning}");
    assert_eq!(kept_pins(&repo), kept, "no resume removes a kept pin");
}

#[test]
fn a_kept_pin_holds_the_captured_tree_when_the_index_changed_after_capture() {
    let (_tree, repo) = temp_engine_repo("kept-index");
    git_in(&repo, &["config", "core.autocrlf", "false"]);
    one_gated_task(&repo);
    let run = run_with(
        &refusal_options(&repo, unstage_and_tear_after_capture),
        &fake(Effect::EditFile),
    );
    registry_refusal_text(&run, "the run beside a tear that stays");
    let (_, _, tree) = captured(&repo);
    let index = git_in(&repo, &["write-tree"]).trim().to_owned();
    assert_ne!(index, tree, "the index changed after the capture");
    let kept = kept_pins(&repo);
    let [(pin, commit)] = kept.as_slice() else {
        panic!("one kept pin: {kept:?}");
    };
    assert_eq!(
        tree_of(&repo, commit),
        tree,
        "the pin holds the captured candidate, never the index as it stands at the refusal"
    );
    let paid = fs::read_to_string(repo.join("agent-output.txt")).expect("the paid output");

    repair_the_torn_registration(&repo);
    let run_id = rundir::latest_run(&repo).expect("the run started");
    let report =
        resume_the_run(&repo, &run_id, &fake(Effect::NoEdit)).expect("the resume after the repair");
    assert!(
        !repo.join("agent-output.txt").exists(),
        "the resume discarded the checkout's copy"
    );
    let warning = kept_pin_warning(&report).expect("the resume names the kept pin");
    follow_the_restore(&repo, warning, pin);
    assert_eq!(
        fs::read_to_string(repo.join("agent-output.txt")).expect("the restored output"),
        paid,
        "following the warning brings the paid output back"
    );
}

#[test]
fn following_the_kept_pin_warning_at_its_parent_restores_deletions_too() {
    let (_tree, repo) = shaped_repo("kept-restore");
    let run = run_with(
        &refusal_options(&repo, tear_after_capture),
        &shaped_worker(),
    );
    registry_refusal_text(&run, "the shaped run beside a tear that stays");
    let kept = kept_pins(&repo);
    let [(pin, commit)] = kept.as_slice() else {
        panic!("one kept pin: {kept:?}");
    };
    let kept_change = git_in(
        &repo,
        &["diff", "--name-status", &format!("{commit}^"), commit],
    );
    assert!(kept_change.contains("D\tdeleted.txt"), "{kept_change}");

    repair_the_torn_registration(&repo);
    let run_id = rundir::latest_run(&repo).expect("the run started");
    let report =
        resume_the_run(&repo, &run_id, &fake(Effect::NoEdit)).expect("the resume after the repair");
    let warning = kept_pin_warning(&report).expect("the resume names the kept pin");
    let command = follow_the_restore(&repo, warning, pin);
    assert_eq!(
        differences_from_the_pin(&repo, pin),
        Vec::<String>::new(),
        "`{command}` restores the kept tree exactly"
    );
    assert!(
        !repo.join("deleted.txt").exists(),
        "the deletion is restored too"
    );
}

#[test]
fn following_the_kept_pin_warning_on_a_later_head_applies_exactly_the_kept_change() {
    let (_tree, repo) = shaped_repo("kept-later");
    let run = run_with(
        &refusal_options(&repo, tear_after_capture),
        &shaped_worker(),
    );
    registry_refusal_text(&run, "the shaped run beside a tear that stays");
    let kept = kept_pins(&repo);
    let [(pin, commit)] = kept.as_slice() else {
        panic!("one kept pin: {kept:?}");
    };

    repair_the_torn_registration(&repo);
    let run_id = rundir::latest_run(&repo).expect("the run started");
    let report = resume_the_run(&repo, &run_id, &fake(Effect::EditFile))
        .expect("the resume after the repair");
    assert!(committed(&report, "t1"), "{report:?}");
    assert_ne!(
        git_in(&repo, &["rev-parse", "HEAD"]).trim(),
        git_in(&repo, &["rev-parse", &format!("{commit}^")]).trim(),
        "HEAD advanced past the pin's parent"
    );
    let warning = kept_pin_warning(&report).expect("the resume names the kept pin");
    let command = follow_the_pick(&repo, warning, pin);
    let mut staged: Vec<String> = git_in(&repo, &["diff", "--cached", "--name-status", "HEAD"])
        .lines()
        .map(str::to_owned)
        .collect();
    staged.sort_unstable();
    assert_eq!(
        staged,
        ["A\tnew.txt", "D\tdeleted.txt", "M\ttracked.txt"],
        "`{command}` stages exactly the kept change"
    );
}

fn a_torn_topology_slot(repo: &Path) -> (crate::workspace_manager::WorkspaceManager, PathBuf) {
    use crate::workspace_manager::{NoHooks, Slot, WorkspaceManager};
    let private = repo.with_file_name("topo");
    fs::create_dir_all(&private).expect("the topology run's private root");
    let manager = WorkspaceManager::derive(
        repo,
        &private,
        crate::workspace_manager::fixture::RUN_ID,
        "inc-1",
    )
    .expect("a topology manager of the same repository");
    manager
        .create_execution_root(&mut NoHooks)
        .expect("its execution root");
    let slot = Slot::Task {
        key: "alpha".to_owned(),
        generation: 1,
    };
    manager
        .write_intent(&mut NoHooks, &slot)
        .expect("its intent");
    let head = git_in(repo, &["rev-parse", "HEAD"]).trim().to_owned();
    let path = manager
        .add_worktree(&mut NoHooks, &slot, &head)
        .expect("a topology slot");
    let admin = crate::workspace_manager::fixture::tear_registration(&manager, &path);
    (manager, admin)
}

fn finish_the_topology_registration(admin: &Path) {
    fs::write(admin.join("commondir"), "../..\n").expect("its writer writes commondir");
    fs::remove_file(admin.join("locked")).expect("and unlocks");
}

#[test]
fn a_topology_slots_torn_registration_refuses_a_legacy_snapshot_and_keeps_its_output() {
    let (_tree, repo) = temp_engine_repo("kept-topo");
    one_gated_task(&repo);
    let (_manager, admin) = a_torn_topology_slot(&repo);
    let run = run_with(
        &refusal_options(&repo, record_the_capture),
        &fake(Effect::EditFile),
    );
    registry_refusal_text(&run, "the run beside a topology writer's residue");
    let (_, _, tree) = captured(&repo);
    assert!(status_of(&repo).contains("agent-output.txt"));
    let kept = kept_pins(&repo);
    let [(pin, commit)] = kept.as_slice() else {
        panic!("one kept pin: {kept:?}");
    };
    assert_eq!(
        tree_of(&repo, commit),
        tree,
        "the pin holds the captured tree"
    );

    finish_the_topology_registration(&admin);
    let run_id = rundir::latest_run(&repo).expect("the run started");
    let report = resume_the_run(&repo, &run_id, &fake(Effect::EditFile))
        .expect("the resume after the topology writer finished");
    assert!(committed(&report, "t1"), "{report:?}");
    let warning = kept_pin_warning(&report).expect("the resume names the kept pin");
    assert!(warning.contains(pin.as_str()), "{warning}");
    assert_eq!(kept_pins(&repo), kept, "no resume removes a kept pin");
}

#[test]
fn an_attempt_error_that_is_not_a_registry_refusal_keeps_the_output_pinned() {
    let (_tree, repo) = temp_engine_repo("kept-not-reg");
    one_gated_task(&repo);
    let run = run_with(
        &refusal_options(&repo, fail_after_capture),
        &fake(Effect::EditFile),
    );
    let Err(UpstrokeError::WithWarnings(warned)) = &run else {
        panic!("the attempt's own error, with the kept note beside it: {run:?}");
    };
    assert!(
        matches!(warned.error.as_ref(), UpstrokeError::Git { message } if message.contains("not the registry's")),
        "the error is the attempt's own, not a registry refusal: {run:?}"
    );
    assert!(
        warned
            .warnings
            .iter()
            .any(|warning| warning.contains("is kept in this checkout and pinned at")),
        "{run:?}"
    );
    assert!(
        status_of(&repo).contains("agent-output.txt"),
        "nothing discarded the attempt's output"
    );
    let kept = kept_pins(&repo);
    let [(_, commit)] = kept.as_slice() else {
        panic!("one kept pin: {kept:?}");
    };
    assert!(
        git_in(&repo, &["ls-tree", "-r", "--name-only", commit]).contains("agent-output.txt"),
        "the pin holds the output"
    );
}

#[test]
fn a_snapshot_failure_that_is_not_the_registrys_keeps_the_output_pinned() {
    let (_tree, repo) = temp_engine_repo("kept-store");
    one_gated_task(&repo);
    let run = run_with(
        &refusal_options(&repo, block_the_snapshot_store_after_capture),
        &fake(Effect::EditFile),
    );
    assert!(
        run.is_err() && !matches!(&run, Err(UpstrokeError::RegistryRefused { .. })),
        "a snapshot store that cannot be made is not the registry's refusal: {run:?}"
    );
    assert!(
        status_of(&repo).contains("agent-output.txt"),
        "nothing discarded the attempt's output"
    );
    assert_eq!(kept_pins(&repo).len(), 1, "the output is pinned");
}

#[test]
fn a_kept_pin_that_cannot_be_written_is_reported_and_nothing_is_discarded() {
    let (_tree, repo) = temp_engine_repo("kept-unpinned");
    one_gated_task(&repo);
    let run = run_with(
        &refusal_options(&repo, block_the_kept_pin_and_tear_after_capture),
        &fake(Effect::EditFile),
    );
    let refusal = registry_refusal_text(&run, "the run whose kept pin's name a ref blocks");
    assert!(
        refusal.contains("is kept in this checkout, and pinning it at")
            && refusal.contains("failed"),
        "the refusal says the pin failed: {refusal}"
    );
    assert!(
        status_of(&repo).contains("agent-output.txt"),
        "nothing is discarded"
    );
    assert_eq!(kept_pins(&repo), Vec::<(String, String)>::new());
}

#[test]
fn a_review_snapshot_refused_after_capture_keeps_and_pins_the_candidate() {
    let (_tree, repo) = temp_engine_repo("kept-review");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 depends= -->\n",
        Some("[interaction]\nmode = \"never\"\n"),
    );
    let run = run_with(
        &refusal_options(&repo, tear_after_capture),
        &fake(Effect::EditFile),
    );
    let refusal = registry_refusal_text(&run, "the review snapshot beside a tear that stays");
    let (_, _, tree) = captured(&repo);
    assert!(status_of(&repo).contains("agent-output.txt"));
    let kept = kept_pins(&repo);
    let [(pin, commit)] = kept.as_slice() else {
        panic!("one kept pin: {kept:?}");
    };
    assert!(refusal.contains(pin.as_str()), "{refusal}");
    assert_eq!(
        tree_of(&repo, commit),
        tree,
        "the pin holds the captured tree"
    );
}

fn canonical_common_git_dir_of(repo: &Path) -> PathBuf {
    let common = git_in(
        repo,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    );
    fs::canonicalize(common.trim()).expect("the canonical common git dir")
}

struct OnceContended {
    done: std::sync::Arc<std::sync::atomic::AtomicBool>,
    writer: std::thread::JoinHandle<Option<usize>>,
}

impl OnceContended {
    fn spawn(repo: &Path, finish: impl FnOnce() + Send + 'static) -> Self {
        let common = canonical_common_git_dir_of(repo);
        let before = crate::workspace_manager::contended_attempts(&common);
        let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stop = std::sync::Arc::clone(&done);
        let writer = std::thread::spawn(move || {
            let watchdog = std::time::Instant::now() + Duration::from_secs(60);
            while std::time::Instant::now() < watchdog
                && !stop.load(std::sync::atomic::Ordering::SeqCst)
            {
                let now = crate::workspace_manager::contended_attempts(&common);
                if now > before {
                    finish();
                    return Some(now - before);
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            None
        });
        Self { done, writer }
    }

    fn join(self) -> Option<usize> {
        self.done.store(true, std::sync::atomic::Ordering::SeqCst);
        self.writer.join().expect("the writer thread")
    }
}

#[test]
fn a_legacy_run_completes_past_a_tear_its_writer_finishes() {
    let (_tree, repo) = temp_engine_repo("tear-legacy");
    one_gated_task(&repo);
    let writer = OnceContended::spawn(&repo, {
        let repo = repo.clone();
        move || repair_the_torn_registration(&repo)
    });
    let run = run_with(
        &refusal_options(&repo, tear_after_capture),
        &fake(Effect::EditFile),
    );
    let contended = writer.join();
    let report = run.expect("the run is attempted past a tear its writer finished");
    assert!(committed(&report, "t1"), "{report:?}");
    assert!(
        contended.is_some(),
        "the snapshot add failed on the tear at least once"
    );
    assert_eq!(
        kept_pins(&repo),
        Vec::<(String, String)>::new(),
        "nothing is kept"
    );
    assert_eq!(status_of(&repo), "", "and nothing is left behind");
}

#[test]
fn a_legacy_run_completes_past_a_topology_slot_its_writer_finishes() {
    let (_tree, repo) = temp_engine_repo("tear-topo");
    one_gated_task(&repo);
    let (_manager, admin) = a_torn_topology_slot(&repo);
    let writer = OnceContended::spawn(&repo, move || finish_the_topology_registration(&admin));
    let run = run_with(
        &refusal_options(&repo, record_the_capture),
        &fake(Effect::EditFile),
    );
    let contended = writer.join();
    let report = run.expect("the run is attempted past the topology writer");
    assert!(committed(&report, "t1"), "{report:?}");
    assert!(
        contended.is_some(),
        "the snapshot add failed on the topology slot's registration at least once"
    );
    assert_eq!(
        kept_pins(&repo),
        Vec::<(String, String)>::new(),
        "nothing is kept"
    );
}

fn restore_at_the_parent_under(tag: &str, after_capture: super::options::AfterCandidateCapture) {
    let (_tree, repo) = shaped_repo(tag);
    let run = run_with(&refusal_options(&repo, after_capture), &shaped_worker());
    registry_refusal_text(&run, tag);
    let (_, _, tree) = captured(&repo);
    let kept = kept_pins(&repo);
    let [(pin, _)] = kept.as_slice() else {
        panic!("{tag}: one kept pin: {kept:?}");
    };
    assert_eq!(
        tree_of(&repo, pin),
        tree,
        "{tag}: the pin holds the captured tree"
    );

    repair_the_torn_registration(&repo);
    let run_id = rundir::latest_run(&repo).expect("the run started");
    let report =
        resume_the_run(&repo, &run_id, &fake(Effect::NoEdit)).expect("the resume after the repair");
    assert!(
        !repo.join("new.txt").exists(),
        "{tag}: the resume discarded the checkout's copy"
    );
    let warning = kept_pin_warning(&report).expect("the resume names the kept pin");
    let command = follow_the_restore(&repo, warning, pin);
    assert_eq!(
        differences_from_the_pin(&repo, pin),
        Vec::<String>::new(),
        "{tag}: `{command}` restores the pin as the repository records it"
    );
}

#[test]
fn the_kept_pin_restore_writes_the_recorded_blob_under_a_blob_replacement() {
    restore_at_the_parent_under("kept-blob", replace_the_new_blob_and_tear_after_capture);
}

#[test]
fn the_kept_pin_restore_writes_the_recorded_tree_under_a_tree_replacement() {
    restore_at_the_parent_under(
        "kept-tree",
        replace_the_captured_tree_and_tear_after_capture,
    );
}

fn pick_on_a_later_head_under(
    tag: &str,
    after_capture: super::options::AfterCandidateCapture,
    graft: bool,
) {
    let (_tree, repo) = shaped_repo(tag);
    let run = run_with(&refusal_options(&repo, after_capture), &shaped_worker());
    registry_refusal_text(&run, tag);
    let kept = kept_pins(&repo);
    let [(pin, commit)] = kept.as_slice() else {
        panic!("{tag}: one kept pin: {kept:?}");
    };

    repair_the_torn_registration(&repo);
    let run_id = rundir::latest_run(&repo).expect("the run started");
    let report = resume_the_run(&repo, &run_id, &fake(Effect::EditFile))
        .expect("the resume after the repair");
    assert!(committed(&report, "t1"), "{tag}: {report:?}");
    if graft {
        let root = git_in(&repo, &["rev-list", "--max-parents=0", "HEAD"])
            .trim()
            .to_owned();
        git_in(&repo, &["replace", "--graft", commit, &root]);
    }
    let warning = kept_pin_warning(&report).expect("the resume names the kept pin");
    let command = follow_the_pick(&repo, warning, pin);
    let mut staged: Vec<String> = recorded(&repo, &["diff", "--cached", "--name-status", "HEAD"])
        .lines()
        .map(str::to_owned)
        .collect();
    staged.sort_unstable();
    assert_eq!(
        staged,
        ["A\tnew.txt", "D\tdeleted.txt", "M\ttracked.txt"],
        "{tag}: `{command}` stages exactly the kept change"
    );
    for path in ["tracked.txt", "new.txt"] {
        assert_eq!(
            fs::read_to_string(repo.join(path)).expect("a picked file"),
            recorded(&repo, &["cat-file", "blob", &format!("{commit}:{path}")]),
            "{tag}: {path} holds what the pin records"
        );
    }
    assert!(
        !repo.join("deleted.txt").exists(),
        "{tag}: the deletion applies"
    );
    assert_eq!(
        fs::read_to_string(repo.join("agent-output.txt")).expect("the later commit's file"),
        recorded(&repo, &["cat-file", "blob", "HEAD:agent-output.txt"]),
        "{tag}: the later commit's file stays"
    );
}

#[test]
fn the_kept_pin_pick_applies_the_recorded_change_under_a_tree_replacement() {
    pick_on_a_later_head_under(
        "pick-tree",
        replace_the_captured_tree_and_tear_after_capture,
        false,
    );
}

#[test]
fn the_kept_pin_pick_takes_the_recorded_parent_under_a_graft() {
    pick_on_a_later_head_under("pick-graft", tear_after_capture, true);
}

struct TearingWorker {
    inner: FakeAdapter,
}

impl AgentAdapter for TearingWorker {
    fn id(&self) -> &'static str {
        self.inner.id()
    }

    fn probe(&self, runner: &dyn crate::runner::Runner) -> Result<Caps, UpstrokeError> {
        self.inner.probe(runner)
    }

    fn build(&self, run: &TaskRun) -> Result<CommandSpec, UpstrokeError> {
        if run.profile.permissions != PermissionMode::ReadOnly {
            plant_a_torn_registration_in(&run.workspace).map_err(after_capture_failed)?;
        }
        self.inner.build(run)
    }

    fn parse(&self, out: &ProcessOutput) -> Result<Outcome, UpstrokeError> {
        self.inner.parse(out)
    }

    fn materialize_permissions(
        &self,
        profile: &WorkerProfile,
        gate_cmds: &[String],
        dir: &Path,
        stem: &str,
    ) -> Result<Option<PathBuf>, UpstrokeError> {
        self.inner
            .materialize_permissions(profile, gate_cmds, dir, stem)
    }
}

#[test]
fn every_kept_pin_is_named_after_a_second_refusal_on_the_next_resume() {
    let (_tree, repo) = temp_engine_repo("kept-twice");
    one_gated_task(&repo);
    let first = run_with(
        &refusal_options(&repo, tear_after_capture),
        &fake(Effect::EditFile),
    );
    registry_refusal_text(&first, "the first refusal");
    assert_eq!(kept_pins(&repo).len(), 1);

    repair_the_torn_registration(&repo);
    let run_id = rundir::latest_run(&repo).expect("the run started");
    let tearing = OneAdapter {
        adapter: TearingWorker {
            inner: FakeAdapter::new(vec![Effect::EditFile], vec![ReviewBehavior::Pass]),
        },
    };
    let second = resume_the_run(&repo, &run_id, &tearing);
    registry_refusal_text(&second, "the resumed attempt's own refusal");
    let both = kept_pins(&repo);
    assert_eq!(
        both.len(),
        2,
        "each refused attempt keeps its own pin: {both:?}"
    );

    repair_the_torn_registration(&repo);
    let report = resume_the_run(&repo, &run_id, &fake(Effect::EditFile))
        .expect("the resume after the second repair");
    assert!(committed(&report, "t1"), "{report:?}");
    let warning = kept_pin_warning(&report).expect("the resume names the kept pins");
    for (pin, _) in &both {
        assert!(
            warning.contains(pin.as_str()),
            "every surviving pin is named; {pin} is not: {warning}"
        );
    }
    assert_eq!(kept_pins(&repo), both, "no resume removes a kept pin");
}

fn named_after_a_failed_resume(tag: &str, retire: bool) {
    let (_tree, repo) = temp_engine_repo(tag);
    one_gated_task(&repo);
    let run = run_with(
        &refusal_options(&repo, tear_after_capture),
        &fake(Effect::EditFile),
    );
    registry_refusal_text(&run, tag);
    let kept = kept_pins(&repo);
    let [(pin, _)] = kept.as_slice() else {
        panic!("{tag}: one kept pin: {kept:?}");
    };

    repair_the_torn_registration(&repo);
    let run_id = rundir::latest_run(&repo).expect("the run started");
    let failed = resume_the_run(&repo, &run_id, &fake(Effect::SpawnError));
    assert!(
        failed.is_err(),
        "{tag}: the resume fails after settling the interrupted attempt"
    );
    let mut warnings = Vec::new();
    let events = events::read_all(
        &rundir::public_dir(&repo, &run_id).join("events.jsonl"),
        &mut warnings,
    )
    .expect("the run's log");
    let mut state = RunState::new(vec!["t1".to_owned()]);
    for event in &events {
        state.apply(event);
    }
    assert!(
        state
            .interrupted_attempts()
            .iter()
            .all(|interrupted| interrupted.flight.attempt != 1),
        "{tag}: attempt 1 is no longer in flight after the failed resume"
    );
    if retire {
        git_in(&repo, &["update-ref", "-d", pin]);
    }

    let report = resume_the_run(&repo, &run_id, &fake(Effect::EditFile))
        .expect("the resume after the failed one");
    let named = report
        .warnings
        .iter()
        .any(|warning| warning.contains(pin.as_str()));
    if retire {
        assert_eq!(kept_pins(&repo), Vec::<(String, String)>::new());
        assert!(
            !named,
            "{tag}: a removed pin is named no more: {:?}",
            report.warnings
        );
    } else {
        assert_eq!(
            kept_pins(&repo),
            kept,
            "{tag}: the pin survives both resumes"
        );
        assert!(
            named,
            "{tag}: the pin is named again: {:?}",
            report.warnings
        );
    }
}

#[test]
fn a_kept_pin_is_named_after_a_resume_that_failed() {
    named_after_a_failed_resume("kept-failed", false);
}

#[test]
fn a_removed_kept_pin_is_named_no_more() {
    named_after_a_failed_resume("kept-retired", true);
}
#[cfg(unix)]
const ONE_GATE_PLAN: &str = "## Implement the widget\n<!-- upstroke: id=t1 depends= -->\n";
#[cfg(unix)]
const ONE_GATE_CONFIG: &str = "[[gates]]\nname = \"version\"\ncmd = \"git --version\"\n";

#[cfg(unix)]
fn layout_bytes(at: &Path, into: &mut Vec<(PathBuf, Vec<u8>)>) {
    let Ok(metadata) = fs::symlink_metadata(at) else {
        return;
    };
    if metadata.is_dir() {
        into.push((at.to_path_buf(), Vec::new()));
        let mut entries: Vec<PathBuf> = fs::read_dir(at)
            .expect("read a watched directory")
            .map(|entry| entry.expect("a directory entry").path())
            .collect();
        entries.sort();
        for entry in entries {
            layout_bytes(&entry, into);
        }
    } else {
        into.push((at.to_path_buf(), fs::read(at).expect("read a watched file")));
    }
}

#[cfg(unix)]
fn other_repository_bytes(other: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut bytes = Vec::new();
    for watched in ["objects", "refs", "packed-refs"] {
        layout_bytes(&other.join(".git").join(watched), &mut bytes);
    }
    bytes
}

#[cfg(unix)]
fn link_the_registry_into(registry: &Path, repository: &Path, other: &Path) {
    copy_tree_for_a_layout(repository, other);
    fs::create_dir(other.join(".git").join("worktrees")).expect("the other registry");
    std::os::unix::fs::symlink(other.join(".git").join("worktrees"), registry)
        .expect("the run's registry, a link into the other repository");
}

#[cfg(unix)]
#[test]
fn a_legacy_run_under_an_inherited_selection_checks_its_review_input_at_the_selected_repository() {
    let tree = temp_engine_scratch("inherited-selection-run");
    let enclosing = tree.path().join("A");
    let work = enclosing.join("work");
    fs::create_dir_all(&work).expect("the started directory");
    git_in(&enclosing, &["init", "-q", "-b", "main"]);
    git_in(&enclosing, &["config", "user.email", "test@upstroke.local"]);
    git_in(&enclosing, &["config", "user.name", "upstroke tests"]);
    fs::write(enclosing.join("README.md"), "A\n").expect("A's readme");
    fs::write(work.join("shared.txt"), "base\n").expect("the shared file");
    git_in(&enclosing, &["add", "-A"]);
    git_in(&enclosing, &["commit", "-q", "-m", "A"]);
    fs::write(work.join("shared.txt"), "user-staged\n").expect("the user's version");
    git_in(&enclosing, &["add", "work/shared.txt"]);
    fs::write(work.join("shared.txt"), "base\n").expect("the file as committed");
    let selected = tree.path().join("B.git");
    for args in [
        ["init", "-q", "-b", "main"].as_slice(),
        ["add", "shared.txt"].as_slice(),
        [
            "-c",
            "user.name=upstroke tests",
            "-c",
            "user.email=test@upstroke.local",
            "commit",
            "-q",
            "-m",
            "B: initial",
        ]
        .as_slice(),
    ] {
        let output = Command::new("git")
            .current_dir(&work)
            .env("GIT_DIR", &selected)
            .env("GIT_WORK_TREE", &work)
            .args(args)
            .output()
            .expect("run git");
        assert!(output.status.success(), "{args:?}: {output:?}");
    }
    let index = fs::read(enclosing.join(".git").join("index")).expect("A's index");
    let head = fs::read(enclosing.join(".git").join("HEAD")).expect("A's HEAD");
    let ended = crate::agent::proc::test_support::run_test_isolated(
        "engine::tests::inherited_selection_run_helper",
        &[
            ("GIT_DIR", selected.as_os_str()),
            ("GIT_WORK_TREE", work.as_os_str()),
            ("GIT_AUTHOR_NAME", std::ffi::OsStr::new("upstroke tests")),
            (
                "GIT_AUTHOR_EMAIL",
                std::ffi::OsStr::new("test@upstroke.local"),
            ),
            ("GIT_COMMITTER_NAME", std::ffi::OsStr::new("upstroke tests")),
            (
                "GIT_COMMITTER_EMAIL",
                std::ffi::OsStr::new("test@upstroke.local"),
            ),
            (INHERITED_RUN_ROOT, work.as_os_str()),
        ],
        Duration::from_secs(300),
    );
    assert!(
        ended.status.is_some_and(|status| status.success()) && ended.stdout.contains("1 passed"),
        "the run under the inherited selection completed and committed into B.git: {ended}"
    );
    assert_eq!(
        fs::read(enclosing.join(".git").join("index")).expect("A's index"),
        index,
        "the enclosing repository's index is byte-identical"
    );
    assert_eq!(
        fs::read(enclosing.join(".git").join("HEAD")).expect("A's HEAD"),
        head,
        "the enclosing repository's HEAD is byte-identical"
    );
}

#[cfg(unix)]
#[test]
fn a_gated_legacy_run_under_an_absolute_common_dir_keeps_its_snapshot_in_the_run_repository() {
    let (tree, repo) = temp_engine_repo("absolute-common-registry");
    // The seed's commit starts no maintenance (detached since Git 2.47) in the copied repository.
    git_in(&repo, &["config", "maintenance.auto", "false"]);
    seed(&repo, ONE_GATE_PLAN, Some(ONE_GATE_CONFIG));
    let other = tree.path().join("other");
    link_the_registry_into(&repo.join(".git").join("worktrees"), &repo, &other);
    let before = other_repository_bytes(&other);
    let ended = crate::agent::proc::test_support::run_test_isolated(
        "engine::tests::absolute_common_dir_run_helper",
        &[
            ("GIT_COMMON_DIR", repo.join(".git").as_os_str()),
            (INHERITED_RUN_ROOT, repo.as_os_str()),
            (INHERITED_RUN_PRIVATE, private_root_for(&repo).as_os_str()),
        ],
        Duration::from_secs(300),
    );
    assert!(
        ended.status.is_some_and(|status| status.success()) && ended.stdout.contains("1 passed"),
        "the gated run under an absolute GIT_COMMON_DIR completed: {ended}"
    );
    assert!(
        repo.join("agent-output.txt").is_file(),
        "the worker's file is still in the checkout"
    );
    assert!(
        other_repository_bytes(&other) == before,
        "the other repository's objects and refs are byte-identical"
    );
    drop(tree);
}

#[cfg(unix)]
#[test]
fn a_gated_legacy_run_under_a_relative_common_dir_through_a_linked_registry_keeps_its_snapshot_in_the_run_repository()
 {
    let tree = temp_engine_scratch("relative-common-registry");
    let root = tree.path();
    let repo = root.join("source/a/b/c/d/repo");
    fs::create_dir_all(&repo).expect("the repository, six levels below the scratch root");
    git_in(&repo, &["init", "-q", "-b", "main"]);
    git_in(&repo, &["config", "user.email", "test@upstroke.local"]);
    git_in(&repo, &["config", "user.name", "upstroke tests"]);
    // No commit here starts maintenance (detached since Git 2.47) in the `.git` copied below.
    git_in(&repo, &["config", "maintenance.auto", "false"]);
    fs::write(repo.join("README.md"), "seed\n").expect("seed");
    git_in(&repo, &["add", "-A"]);
    git_in(&repo, &["commit", "-q", "-m", "seed"]);
    seed(&repo, ONE_GATE_PLAN, Some(ONE_GATE_CONFIG));
    let common = root.join("common");
    copy_tree_for_a_layout(&repo.join(".git"), &common);
    let other = root.join("other");
    link_the_registry_into(&common.join("worktrees"), &repo, &other);
    let private = root.join("private");
    fs::create_dir(&private).expect("the private root, one level below the scratch root");
    let before = other_repository_bytes(&other);
    let ended = crate::agent::proc::test_support::run_test_isolated(
        "engine::tests::relative_common_dir_run_helper",
        &[
            (
                "GIT_COMMON_DIR",
                std::ffi::OsStr::new("../../../../../../common"),
            ),
            (INHERITED_RUN_ROOT, repo.as_os_str()),
            (INHERITED_RUN_PRIVATE, private.as_os_str()),
        ],
        Duration::from_secs(300),
    );
    assert!(
        ended.status.is_some_and(|status| status.success()) && ended.stdout.contains("1 passed"),
        "the gated run under a relative GIT_COMMON_DIR completed: {ended}"
    );
    assert!(
        repo.join("agent-output.txt").is_file(),
        "the worker's file is still in the checkout"
    );
    assert!(
        other_repository_bytes(&other) == before,
        "the other repository's objects and refs are byte-identical"
    );
}

/// P12-W6 (with P12-W19's composition half): both public paths run their roles
/// on the managed repository's recorded graph and leave every other repository's
/// replacements alone. The managed repository pins `core.useReplaceRefs = true`
/// and replaces the committed probe blob; one gate reads the snapshot's bytes,
/// one reads through the object graph, and one builds an unrelated repository
/// whose replacement must hold, which a process-wide setting would defeat.
#[cfg(unix)]
#[test]
fn a_schema_4_run_and_resume_read_the_recorded_graph_and_leave_other_repositories_theirs() {
    let status = crate::workspace_manager::fixture::run_replacement_witness_child(
        "engine::tests::topology_object_graph_helper",
    );
    assert!(
        status.success(),
        "the child drives a schema-4 `engine::run_with` and `engine::resume_with` over a \
         repository whose committed probe blob carries a replacement, and ended {status:?}"
    );
}

/// P12-W7: a schema-4 run in a checkout whose environment carries a relative
/// `GIT_COMMON_DIR`, through a linked registry, completes, and its gate sees the
/// run's common directory as the absolute path it names.
#[cfg(unix)]
#[test]
fn a_schema_4_run_under_a_relative_common_dir_completes_and_its_roles_see_the_absolute_one() {
    let tree = temp_engine_scratch("topology-relative-common");
    let root = tree.path();
    let repo = root.join("source/a/b/c/d/repo");
    fs::create_dir_all(&repo).expect("the repository, six levels below the scratch root");
    git_in(&repo, &["init", "-q", "-b", "main"]);
    git_in(&repo, &["config", "user.email", "test@upstroke.local"]);
    git_in(&repo, &["config", "user.name", "upstroke tests"]);
    git_in(&repo, &["config", "maintenance.auto", "false"]);
    fs::write(repo.join("README.md"), "seed\n").expect("seed");
    git_in(&repo, &["add", "-A"]);
    git_in(&repo, &["commit", "-q", "-m", "seed"]);
    let common = root.join("common");
    let expected = fs::canonicalize(root)
        .expect("the scratch root canonicalizes")
        .join("common");
    seed(
        &repo,
        WORKER_ONLY_PLAN,
        Some(&format!(
            "[[gates]]\nname = \"common-dir\"\n\
             cmd = 'case \"$GIT_COMMON_DIR\" in /*) ;; *) exit 1 ;; esac; \
             test \"$(cd \"$GIT_COMMON_DIR\" && pwd -P)\" = \"{}\"'\n",
            expected.display()
        )),
    );
    copy_tree_for_a_layout(&repo.join(".git"), &common);
    let other = root.join("other");
    link_the_registry_into(&common.join("worktrees"), &repo, &other);
    let private = root.join("private");
    fs::create_dir(&private).expect("the private root, one level below the scratch root");
    let before = other_repository_bytes(&other);
    let ended = crate::agent::proc::test_support::run_test_isolated(
        "engine::tests::relative_common_dir_topology_helper",
        &[
            (
                "GIT_COMMON_DIR",
                std::ffi::OsStr::new("../../../../../../common"),
            ),
            (INHERITED_RUN_ROOT, repo.as_os_str()),
            (INHERITED_RUN_PRIVATE, private.as_os_str()),
        ],
        Duration::from_secs(300),
    );
    assert!(
        ended.status.is_some_and(|status| status.success()) && ended.stdout.contains("1 passed"),
        "the schema-4 run under a relative GIT_COMMON_DIR completed: {ended}"
    );
    assert!(
        other_repository_bytes(&other) == before,
        "the other repository's objects and refs are byte-identical"
    );
}

/// P12-W8: a relative `GIT_OBJECT_DIRECTORY` refuses a schema-4 run and the
/// resume of a schema-4 run before any lock. The parent holds the worktree lock
/// across the child, so a command that reached the lock would refuse as the
/// lock's, not as the relative name's.
#[cfg(unix)]
#[test]
fn a_relative_object_directory_refuses_a_schema_4_run_and_its_resume_before_any_lock() {
    let (_tree, repo) = temp_engine_repo("topology-relative-objects");
    let source = SharedTopologyStubs(std::sync::Arc::new(TopologyStub::new(
        TOPOLOGY_STUB_EDIT,
        1,
    )));
    let parked = run_with(&topology_options(&repo), &source).expect("the run parks");
    until_released(&repo, &parked.run_id);
    let held = WorktreeLock::acquire(&repo).expect("the held-lock control");
    let ended = crate::agent::proc::test_support::run_test_isolated(
        "engine::tests::relative_object_directory_topology_helper",
        &[
            (
                "GIT_OBJECT_DIRECTORY",
                std::ffi::OsStr::new("relative/objects"),
            ),
            (INHERITED_RUN_ROOT, repo.as_os_str()),
            (INHERITED_RUN_ID, std::ffi::OsStr::new(&parked.run_id)),
        ],
        Duration::from_secs(300),
    );
    drop(held);
    assert!(
        ended.status.is_some_and(|status| status.success()) && ended.stdout.contains("1 passed"),
        "both public paths refused the relative GIT_OBJECT_DIRECTORY before the lock: {ended}"
    );
    drop(WorktreeLock::acquire(&repo).expect("no-leak check: no worktree lock is left held"));
}
