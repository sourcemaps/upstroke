//! Extended notes: `docs/internals/engine/tests/verification.md`

use super::*;

fn candidate_mutation_marker(repo: &Path) -> PathBuf {
    let name = repo
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "repo".to_owned());
    repo.with_file_name(format!("{name}-candidate-mutation.txt"))
}

fn mutate_index_after_candidate_capture(
    workspace: &Workspace,
    candidate: &crate::workspace::CapturedCandidate,
) -> Result<(), UpstrokeError> {
    fs::write(
        workspace.root().join("agent-output.txt"),
        "tampered after capture\n",
    )
    .map_err(|error| UpstrokeError::Git {
        message: format!("test could not mutate the captured worktree: {error}"),
    })?;
    let add = Command::new("git")
        .arg("-C")
        .arg(workspace.root())
        .args(["add", "-A"])
        .output()
        .map_err(|error| UpstrokeError::Git {
            message: format!("test could not stage its post-capture mutation: {error}"),
        })?;
    if !add.status.success() {
        return Err(UpstrokeError::Git {
            message: format!(
                "test could not stage its post-capture mutation: {}",
                String::from_utf8_lossy(&add.stderr).trim()
            ),
        });
    }
    let tampered_tree = Command::new("git")
        .arg("-C")
        .arg(workspace.root())
        .arg("write-tree")
        .output()
        .map_err(|error| UpstrokeError::Git {
            message: format!("test could not inspect its post-capture tree: {error}"),
        })?;
    if !tampered_tree.status.success() {
        return Err(UpstrokeError::Git {
            message: format!(
                "test could not inspect its post-capture tree: {}",
                String::from_utf8_lossy(&tampered_tree.stderr).trim()
            ),
        });
    }
    let tampered_tree = String::from_utf8_lossy(&tampered_tree.stdout)
        .trim()
        .to_owned();
    if tampered_tree == candidate.tree_oid {
        return Err(UpstrokeError::Git {
            message: "test post-capture mutation did not change the staged tree".to_owned(),
        });
    }
    fs::write(
        candidate_mutation_marker(workspace.root()),
        format!(
            "{}\n{}\n{tampered_tree}\n",
            candidate.parent_oid, candidate.tree_oid
        ),
    )
    .map_err(|error| UpstrokeError::Git {
        message: format!("test could not record its capture identities: {error}"),
    })
}

#[derive(Default)]
struct FailTheThirdLegacyAppend {
    entered: u32,
}

impl crate::events::log::EventHooks for FailTheThirdLegacyAppend {
    fn point(
        &mut self,
        site: EventSite,
        point: crate::topology::effects::SubEffectPoint,
        mode: crate::topology::effects::InjectionMode,
    ) -> crate::topology::effects::Injection {
        use crate::topology::effects::{Injection, InjectionMode, SubEffectPoint};
        if site != EventSite::LegacyAppend
            || point != SubEffectPoint::Written
            || mode != InjectionMode::ErrorReturn
        {
            return Injection::Proceed;
        }
        self.entered += 1;
        if self.entered == 3 {
            Injection::Error
        } else {
            Injection::Proceed
        }
    }
}

fn fail_the_third_legacy_append() -> Box<dyn crate::events::log::EventHooks> {
    Box::<FailTheThirdLegacyAppend>::default()
}

#[test]
fn a_returned_legacy_append_error_stops_the_run() {
    let (_tree, repo) = temp_engine_repo("legacy-append-error");
    let mut opts = options(&repo);
    opts.log_hooks = Some(fail_the_third_legacy_append);
    let source = fake(Effect::EditFile);

    let error = run_with(&opts, &source)
        .expect_err("a returned append error must reach the caller, not be swallowed");
    let message = error.to_string();
    assert!(
        message.contains("Event.LegacyAppend"),
        "the error must be the append's own, naming its site: {message}"
    );
    assert!(
        message.contains("Written"),
        "…and its point, so an operator can tell which coordinate failed: {message}"
    );
}

#[test]
fn a_returned_legacy_append_error_still_leaves_the_partial_report() {
    let (_tree, repo) = temp_engine_repo("legacy-append-partial");
    let mut opts = options(&repo);
    opts.log_hooks = Some(fail_the_third_legacy_append);
    let source = fake(Effect::EditFile);

    run_with(&opts, &source).expect_err("the append error stops the run");

    let runs = opts.repo_root.join(".upstroke").join("runs");
    let public = fs::read_dir(&runs)
        .expect("the runs root")
        .flatten()
        .map(|entry| entry.path())
        .find(|path| path.join("events.jsonl").is_file())
        .expect("the failed run left its public directory and its log");

    let log = fs::read_to_string(public.join("events.jsonl")).expect("the log");
    let complete = log.lines().filter(|line| line.ends_with('}')).count();
    assert!(
        complete >= 2,
        "only {complete} complete line(s) in the log: the injected failure landed on \
         a startup append, so this test never reached drain_and_report's branch"
    );

    let report = public.join("report.json");
    assert!(
        report.is_file(),
        "no report beside {}: the legacy engine's partial report is a courtesy for \
         whoever opens the directory next, and failing to write it must not be \
         silent (PR5-CONF-011)",
        public.display()
    );
    let parsed: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&report).expect("read the partial report"))
            .expect("the partial report is JSON");
    assert!(
        parsed.get("tasks").is_some(),
        "the partial report is a report, not a stub: {parsed}"
    );
}

#[test]
fn happy_path_commits_one_commit_per_task() {
    let (_tree, repo) = temp_engine_repo("happy");
    let source = fake(Effect::EditFile);
    let report = run_with(&options(&repo), &source).expect("run succeeds");

    assert_eq!(report.outcome(), RunOutcome::Complete);
    assert_eq!(report.tasks.len(), 2);
    assert!(
        report
            .tasks
            .iter()
            .all(|t| matches!(t.status, TaskRunStatus::Committed { .. })),
        "report: {report:?}"
    );

    assert!(
        (report.total_cost_usd - 0.12).abs() < 1e-9,
        "worker and reviewer spend both counted: {}",
        report.total_cost_usd
    );

    let branch = git_in(&repo, &["rev-parse", "--abbrev-ref", "HEAD"]);
    assert!(branch.trim().starts_with("upstroke/run-"), "on run branch");
    let count = git_in(&repo, &["rev-list", "--count", "main..HEAD"]);
    assert_eq!(count.trim(), "2", "one commit per task");
    let log = git_in(&repo, &["log", "--format=%s", "main..HEAD"]);
    assert!(
        log.contains("[upstroke] t1: Implement the widget"),
        "log: {log}"
    );
    assert!(log.contains("[upstroke] t2: Document the widget"));
    assert!(
        git_in(&repo, &["status", "--porcelain"]).trim().is_empty(),
        "clean tree after run"
    );
    assert!(
        repo.join(".upstroke").join("runs").exists(),
        "run dir written"
    );
}

#[test]
fn gates_review_and_commit_use_one_frozen_candidate_tree() {
    let (_tree, repo) = temp_engine_repo("one-frozen-candidate");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 depends= -->\n",
        Some(
            "[interaction]\nmode = \"never\"\n\n\
                 [[gates]]\nname = \"frozen-candidate\"\n\
                 cmd = 'git grep -q \"edited: 0\" -- agent-output.txt'\n",
        ),
    );
    let base = git_in(&repo, &["rev-parse", "HEAD"]).trim().to_owned();
    let marker = candidate_mutation_marker(&repo);
    let _ = fs::remove_file(&marker);
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    opts.after_candidate_capture = Some(mutate_index_after_candidate_capture);
    let source = fake(Effect::FrozenCandidate);

    let report = run_with(&opts, &source).expect("the frozen candidate remains authoritative");
    assert_eq!(report.outcome(), RunOutcome::Complete, "{report:?}");
    assert_eq!(report.gates, ["frozen-candidate"]);

    let capture: Vec<_> = fs::read_to_string(&marker)
        .expect("the post-capture mutation hook ran")
        .lines()
        .map(str::to_owned)
        .collect();
    assert_eq!(capture.len(), 3, "capture marker: {capture:?}");
    assert_eq!(capture[0], base, "captured parent is the attempt parent");
    assert_ne!(
        capture[1], capture[2],
        "the mutable index really changed after capture"
    );

    let logged = events_of(&repo, &report.run_id);
    let prepared = logged
        .iter()
        .find_map(|event| match &event.body {
            EventBody::AttemptFinished {
                prepared_commit, ..
            } => prepared_commit.as_deref().cloned(),
            _ => None,
        })
        .expect("successful settlement records its prepared object");
    assert_eq!(
        prepared.branch_ref,
        format!("refs/heads/{}", report.branch),
        "the durable settlement owns the exact run ref"
    );
    assert_eq!(prepared.parent_sha, capture[0]);
    assert_eq!(prepared.tree_sha, capture[1]);
    assert_ne!(prepared.tree_sha, capture[2]);

    let review_snapshots = source.adapter.review_snapshots();
    assert_eq!(review_snapshots.len(), 1, "one reviewer snapshot");
    assert_eq!(review_snapshots[0].0, prepared.tree_sha);
    assert_eq!(
        review_snapshots[0].1.replace("\r\n", "\n"),
        "edited: 0\n",
        "review sees the captured tree, not the later staged mutation"
    );

    let committed = logged
        .iter()
        .find_map(|event| match &event.body {
            EventBody::TaskCommitted { data, .. } => Some(data),
            _ => None,
        })
        .expect("task_committed follows the prepared settlement");
    let head = git_in(&repo, &["rev-parse", "HEAD"]).trim().to_owned();
    let head_tree = git_in(&repo, &["rev-parse", "HEAD^{tree}"])
        .trim()
        .to_owned();
    assert_eq!(head, prepared.commit_sha);
    assert_eq!(committed.sha, prepared.commit_sha);
    assert_eq!(head_tree, prepared.tree_sha);
    assert_eq!(
        git_in(&repo, &["show", "HEAD:agent-output.txt"]),
        "edited: 0\n",
        "the staged post-capture mutation is never published"
    );
}

#[test]
fn an_oversized_review_diff_is_settled_once_before_the_task_parks() {
    let (_tree, repo) = temp_engine_repo("oversizedreviewsettlement");
    seed(
        &repo,
        "## Generate the large fixture\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some(
            "[interaction]\nmode = \"never\"\n\n\
                 [routing]\nimplement = { chain = [\"small\"], attempts_per = 3 }\n",
        ),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = fake(Effect::LargeEdit);
    let report = run_with(&opts, &source).expect("policy failure is a settled run outcome");

    assert_eq!(report.outcome(), RunOutcome::Parked, "{report:?}");
    let task_report = task(&report, "t1");
    assert_eq!(
        task_report.attempts.len(),
        1,
        "the policy boundary is not retried"
    );
    let attempt = &task_report.attempts[0];
    let failure = attempt.failure.as_ref().expect("settled policy failure");
    assert_eq!(failure.kind, FailureKind::ReviewInputTooLarge);
    assert_eq!(failure.origin, FailureOrigin::Reviewer);
    assert_eq!(attempt.cost_usd, Some(0.01), "worker spend is retained");
    assert_eq!(attempt.session_id.as_deref(), Some("s0"));
    assert!(attempt.usage.is_some(), "worker usage is retained");
    assert!(attempt.reviews.is_empty(), "no reviewer was dispatched");
    assert_eq!(source.adapter.reviews_run(), 0);

    let logged = events_of(&repo, &report.run_id);
    assert_eq!(
        logged
            .iter()
            .filter(|event| matches!(event.body, EventBody::AttemptFinished { .. }))
            .count(),
        1,
        "the attempt has a terminal ledger event"
    );
    let parking = logged.iter().find_map(|event| match &event.body {
        EventBody::AttemptFinished { parking, .. } => parking.as_deref(),
        _ => None,
    });
    assert!(
        parking.is_some(),
        "the settlement atomically carries its parking question"
    );
    assert!(
        !logged.iter().any(|event| matches!(
            event.body,
            EventBody::QuestionRaised { .. } | EventBody::TaskParked { .. }
        )),
        "policy parking must not reopen a crash window with follow-up events"
    );
    assert!(
        !logged
            .iter()
            .any(|event| matches!(event.body, EventBody::AttemptInterrupted { .. })),
        "replay must never invent an interruption for the settled refusal"
    );
    assert!(
        git_in(&repo, &["status", "--porcelain"]).trim().is_empty(),
        "parking cleans the unreviewed oversized diff"
    );
    let question = report.questions.first().expect("scope question");
    assert_eq!(question.question.kind, QuestionKind::Unblock);
    assert!(question.question.context.contains("smaller diff"));
    assert!(question.question.context.contains("starting a new run"));
    assert!(
        !question.question.context.contains("chain is spent")
            && !question.question.context.contains("all failed"),
        "policy parking must not pretend the escalation chain was exhausted: {}",
        question.question.context
    );

    let paths = paths_of(&repo, &report.run_id);
    truncate_log_after(&paths, "attempt_finished");
    fs::write(repo.join("crash-residue.txt"), "unreviewed\n").expect("crash residue");
    let retry = fake(Effect::EditFile);
    let resumed =
        resume_with(&resume_options(&repo, &report.run_id), &retry).expect("resume parks");
    assert_eq!(resumed.outcome(), RunOutcome::Parked, "{resumed:?}");
    assert!(
        retry.adapter.runs().is_empty(),
        "resume paid for the oversized attempt again"
    );
    assert_eq!(task(&resumed, "t1").attempts.len(), 1);
    assert_eq!(resumed.questions.len(), 1);
    assert!(
        git_in(&repo, &["status", "--porcelain"]).trim().is_empty(),
        "resume did not discard crash residue"
    );
}

#[test]
fn opaque_review_input_has_distinct_failure_and_remediation() {
    let (_tree, repo) = temp_engine_repo("opaquereviewsettlement");
    seed(
        &repo,
        "## Generate an opaque artifact\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some(
            "[interaction]\nmode = \"never\"\n\n\
                 [routing]\nimplement = { chain = [\"small\"], attempts_per = 3 }\n",
        ),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = fake(Effect::OpaqueEdit);
    let report = run_with(&opts, &source).expect("opaque evidence parks fail-closed");

    assert_eq!(report.outcome(), RunOutcome::Parked, "{report:?}");
    let attempt = &task(&report, "t1").attempts[0];
    assert_eq!(
        attempt.failure.as_ref().map(|failure| failure.kind),
        Some(FailureKind::ReviewInputOpaque)
    );
    assert_eq!(source.adapter.reviews_run(), 0);
    let context = &report.questions[0].question.context;
    assert!(context.contains("hides changed content"), "{context}");
    assert!(!context.contains("smaller diff"), "{context}");
}

#[test]
fn opaque_test_task_parks_before_test_provenance_retry() {
    let (_tree, repo) = temp_engine_repo("opaquetestprovenance");
    seed(
        &repo,
        "## Add the regression\n<!-- upstroke: id=t1 kind=test depends= -->\n",
        Some(
            "[interaction]\nmode = \"never\"\n\n\
                 [routing]\ntest = { chain = [\"small\"], attempts_per = 3 }\n",
        ),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = fake(Effect::OpaqueEdit);
    let report = run_with(&opts, &source).expect("opaque evidence parks fail-closed");

    assert_eq!(report.outcome(), RunOutcome::Parked, "{report:?}");
    let task = task(&report, "t1");
    assert_eq!(task.attempts.len(), 1, "opaque evidence is not retried");
    assert_eq!(
        task.attempts[0]
            .failure
            .as_ref()
            .map(|failure| failure.kind),
        Some(FailureKind::ReviewInputOpaque),
        "the intrinsic evidence failure wins over Test provenance"
    );
    assert_eq!(source.adapter.reviews_run(), 0);
    assert!(
        report.questions[0]
            .question
            .context
            .contains("hides changed content")
    );
}

#[test]
fn failed_parking_payload_still_settles_and_cleans_the_attempt() {
    let (_tree, repo) = temp_engine_repo("oversizedreviewquestionwrite");
    seed(
        &repo,
        "## Generate the large fixture\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some(
            "[interaction]\nmode = \"never\"\n\n\
                 [routing]\nimplement = { chain = [\"small\"], attempts_per = 3 }\n",
        ),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = fake(Effect::LargeEditQuestionWriteFailure);
    let error = run_with(&opts, &source).expect_err("question projection must fail");
    assert!(
        error.to_string().contains("questions"),
        "wrong failure surfaced: {error}"
    );

    let run_id = rundir::latest_run(&repo).expect("failed run remains resumable");
    let logged = events_of(&repo, &run_id);
    let parking = logged.iter().find_map(|event| match &event.body {
        EventBody::AttemptFinished { parking, .. } => parking.as_deref(),
        _ => None,
    });
    assert!(
        parking.is_some(),
        "the event must retain parking even when its JSON projection fails"
    );
    assert_eq!(
        logged
            .iter()
            .filter(|event| matches!(event.body, EventBody::AttemptFinished { .. }))
            .count(),
        1,
        "the paid attempt must settle exactly once"
    );
    assert!(
        git_in(&repo, &["status", "--porcelain"]).trim().is_empty(),
        "a failed question write leaked the oversized unreviewed diff"
    );

    let paths = paths_of(&repo, &run_id);
    fs::remove_file(paths.questions()).expect("remove injected blocker");
    fs::create_dir(paths.questions()).expect("restore questions directory");
    let retry = fake(Effect::EditFile);
    let resumed = resume_with(&resume_options(&repo, &run_id), &retry)
        .expect("resume repairs the projection and remains parked");
    assert_eq!(resumed.outcome(), RunOutcome::Parked, "{resumed:?}");
    assert!(
        retry.adapter.runs().is_empty(),
        "resume paid for an already-settled attempt"
    );
    assert_eq!(task(&resumed, "t1").attempts.len(), 1);
    let question = resumed.questions.first().expect("restored question");
    assert!(
        interaction::answer_path(&paths.questions(), &question.question.id).exists(),
        "resume did not rematerialize the authoritative question"
    );
}

#[test]
fn dirty_tree_is_refused() {
    let (_tree, repo) = temp_engine_repo("dirty");
    fs::write(repo.join("stray.txt"), "uncommitted\n").expect("stray");
    let source = fake(Effect::EditFile);
    let err = run_with(&options(&repo), &source).expect_err("must refuse");
    assert!(err.to_string().contains("not clean"), "got: {err}");
    let branch = git_in(&repo, &["rev-parse", "--abbrev-ref", "HEAD"]);
    assert_eq!(branch.trim(), "main", "no run branch created");
}

#[test]
fn sparse_checkout_preflight_refusal_leaves_worktree_clean() {
    let (_tree, repo) = temp_engine_repo("sparse-worker-preflight");
    git_in(&repo, &["update-index", "--skip-worktree", "README.md"]);
    let source = fake(Effect::EditFile);

    let error = run_with(&options(&repo), &source)
        .expect_err("incomplete materialization must be refused")
        .to_string();
    assert!(error.contains("sparse checkout is active"), "{error}");
    assert!(
        source.adapter.runs().is_empty(),
        "a worker was dispatched before sparse-checkout refusal"
    );
    assert_eq!(
        git_in(&repo, &["rev-parse", "--abbrev-ref", "HEAD"]).trim(),
        "main",
        "preflight refusal must not create or switch a run branch"
    );
    git_in(&repo, &["update-index", "--no-skip-worktree", "README.md"]);
    let worktree_git_dir = Workspace::open(&repo)
        .expect("open worktree")
        .worktree_git_dir()
        .expect("resolve private git dir");
    assert!(
        worktree_git_dir.join("upstroke-worktree.lock").exists(),
        "the regression must exercise acquisition of the private worktree lease"
    );
    assert!(
        !repo.join(".upstroke").exists(),
        "a refused preflight must not create working-tree coordinator state"
    );
    assert!(
        git_in(&repo, &["status", "--porcelain", "--untracked-files=all"])
            .trim()
            .is_empty(),
        "a refused preflight left coordinator state visible to Git"
    );
}

#[test]
fn passing_configured_gates_commit_and_are_reported() {
    let (_tree, repo) = temp_engine_repo("gatepass");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 depends= -->\n",
        Some("[[gates]]\nname = \"version\"\ncmd = \"git --version\"\n"),
    );

    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = fake(Effect::EditFile);
    let report = run_with(&opts, &source).expect("run");
    assert_eq!(report.outcome(), RunOutcome::Complete, "report: {report:?}");
    assert_eq!(report.gates, ["version"]);
    assert!(report.gates_from_config);
    assert!(report.render().contains("gates: version [from config]"));
}

#[test]
fn ignored_worker_input_cannot_make_a_gate_pass() {
    let (_tree, repo) = temp_engine_repo("ignored-gate-input");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 depends= -->\n",
        Some(
            "[interaction]\nmode = \"never\"\n\n\
                 [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n\n\
                 [[gates]]\nname = \"ignored-input\"\ncmd = \"git hash-object ignored.flag\"\n",
        ),
    );

    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = fake(Effect::IgnoredGateInput);
    let report = run_with(&opts, &source).expect("gate failure settles the task");

    assert!(!committed(&report, "t1"), "report: {report:?}");
    assert!(task(&report, "t1").attempts.iter().any(|attempt| {
        attempt
            .failure
            .as_ref()
            .is_some_and(|failure| failure.kind == FailureKind::GateFailed)
    }));
    assert!(
        !repo.join("ignored.flag").exists(),
        "ignored worker-only input was cleaned from the authoritative workspace"
    );
}

#[test]
fn unresolvable_gate_refuses_at_preflight() {
    let (_tree, repo) = temp_engine_repo("gateresolve");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 depends= -->\n",
        Some("[[gates]]\nname = \"ghost\"\ncmd = \"definitely-not-a-real-tool-xyz build\"\n"),
    );

    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = fake(Effect::EditFile);
    let err = run_with(&opts, &source).expect_err("must refuse");
    assert!(err.to_string().contains("not found on PATH"), "got: {err}");
    let branch = git_in(&repo, &["rev-parse", "--abbrev-ref", "HEAD"]);
    assert_eq!(branch.trim(), "main", "refused before branching");
}

#[test]
fn test_task_without_test_code_fails_provenance() {
    let (_tree, repo) = temp_engine_repo("provenance");
    seed(
        &repo,
        "## Test the widget\n<!-- upstroke: id=tt depends= -->\nAdd coverage.\n",
        Some("[routing]\ntest = { chain = [\"small\"], attempts_per = 1 }\n"),
    );

    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = fake(Effect::EditFile);
    let report = run_with(&opts, &source).expect("engine ok");
    let reason = &task(&report, "tt").attempts[0]
        .failure
        .as_ref()
        .expect("provenance should fail")
        .reason;
    assert!(reason.contains("provenance"), "reason: {reason}");
}

#[test]
fn test_task_adding_real_tests_passes_provenance() {
    let (_tree, repo) = temp_engine_repo("provenance-ok");
    seed(
        &repo,
        "## Test the widget\n<!-- upstroke: id=tt depends= -->\n",
        None,
    );

    let source = fake(Effect::EditTest);
    let report = run_with(&options(&repo), &source).expect("engine ok");
    assert_eq!(report.outcome(), RunOutcome::Complete, "report: {report:?}");
    assert!(committed(&report, "tt"));
}

#[test]
fn gate_residue_is_scrubbed_not_committed() {
    let (_tree, repo) = temp_engine_repo("residue");

    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 depends= -->\n",
        Some("[[gates]]\nname = \"leaky\"\ncmd = \"echo residue> residue.txt\"\n"),
    );

    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = fake(Effect::EditFile);
    let report = run_with(&opts, &source).expect("run");
    assert_eq!(report.outcome(), RunOutcome::Complete, "report: {report:?}");
    assert!(!repo.join("residue.txt").exists(), "residue scrubbed");
    assert!(
        git_in(&repo, &["status", "--porcelain"]).trim().is_empty(),
        "clean tree after run"
    );
    let log = git_in(&repo, &["log", "--name-only", "--format=", "main..HEAD"]);
    assert!(!log.contains("residue.txt"), "log: {log}");
}

#[test]
fn the_reviewer_is_read_only_and_bound_to_the_review_tier() {
    let (_tree, repo) = temp_engine_repo("reviewbinding");
    let source = fake(Effect::EditFile);
    let report = run_with(&options(&repo), &source).expect("run");
    let settings = paths_of(&repo, &report.run_id).settings();
    let allow_list = |file: &str| -> Vec<String> {
        let text = fs::read_to_string(settings.join(file)).expect("settings written");
        let value: serde_json::Value = serde_json::from_str(&text).expect("json");
        value["permissions"]["allow"]
            .as_array()
            .expect("allow list")
            .iter()
            .map(|v| v.as_str().unwrap_or_default().to_owned())
            .collect()
    };

    let reviewer = allow_list("00-t1-1-review.json");
    assert_eq!(reviewer, ["Read", "Glob", "Grep"], "read-only, no shell");

    let implementer = allow_list("00-t1-1.json");
    assert!(
        implementer.contains(&"Edit".to_owned()),
        "implementer can edit"
    );

    assert!(
        !settings.starts_with(&repo),
        "settings live outside the workspace: {}",
        settings.display()
    );
}

#[test]
fn review_can_be_switched_off_explicitly() {
    let (_tree, repo) = temp_engine_repo("noreview");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 depends= -->\n",
        Some("[routing]\nreview = { enabled = false }\n"),
    );

    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));

    let source = source(vec![Effect::EditFile], vec![ReviewBehavior::Fail]);
    let report = run_with(&opts, &source).expect("run");
    assert_eq!(report.outcome(), RunOutcome::Complete, "report: {report:?}");
    assert!(report.tasks[0].review_models.is_empty());
    assert!(report.tasks[0].review_cost_usd.is_none());
}

#[test]
fn reviewer_spend_is_attributed_separately() {
    let (_tree, repo) = temp_engine_repo("reviewcost");
    let source = fake(Effect::EditFile);
    let report = run_with(&options(&repo), &source).expect("run");
    let t1 = task(&report, "t1");
    assert_eq!(t1.cost_usd, Some(0.01), "implementer's own spend");
    assert_eq!(t1.review_cost_usd, Some(0.05), "reviewer's, kept apart");
    assert_eq!(t1.review_models, ["claude-opus-5"]);
    assert!((t1.total_cost_usd().expect("both") - 0.06).abs() < 1e-9);
    let rendered = report.render();
    assert!(rendered.contains("+ review claude-opus-5"), "{rendered}");
}

#[test]
fn a_second_opinion_runs_a_second_family_and_leaves_the_primary_alone() {
    let (_tree, repo) = temp_engine_repo("secondopinion");
    seed(&repo, FRONTIER_AUTH_PLAN, Some(SECOND_OPINION_CONFIG));
    let source = cross_vendor(
        vec![Effect::EditFile],
        vec![ReviewBehavior::Pass],
        vec![ReviewBehavior::Pass],
    );
    let report = run_with(&cross_vendor_opts(&repo), &source).expect("run");

    assert!(committed(&report, "t1"), "both passed: {report:?}");
    let t1 = task(&report, "t1");
    assert_eq!(
        t1.review_models,
        ["claude-opus-5", "gpt-5.3-codex"],
        "one pass per family, primary first"
    );
    assert_eq!(t1.model, "claude-opus-5", "written by the frontier model");
    assert_eq!(source.adapter.reviews_run(), 1);
    assert_eq!(source.copilot().reviews_run(), 1);

    assert_eq!(t1.review_cost_usd, Some(0.10), "0.05 per pass");
    assert_eq!(t1.cost_usd, Some(0.01), "implementer's own");
    let rendered = report.render();
    assert!(
        rendered.contains("+ review claude-opus-5, gpt-5.3-codex"),
        "{rendered}"
    );
}

#[test]
fn a_second_opinion_that_fails_fails_the_attempt() {
    let (_tree, repo) = temp_engine_repo("secondopinionfail");
    seed(&repo, FRONTIER_AUTH_PLAN, Some(SECOND_OPINION_CONFIG));
    let source = cross_vendor(
        vec![Effect::EditFile],
        vec![ReviewBehavior::Pass],
        vec![ReviewBehavior::Fail],
    );
    let report = run_with(&cross_vendor_opts(&repo), &source).expect("run");

    assert!(!committed(&report, "t1"), "a rejected change cannot commit");
    let t1 = task(&report, "t1");
    let last = t1.attempts.last().expect("an attempt ran");
    assert_eq!(
        last.failure.as_ref().map(|f| f.kind),
        Some(FailureKind::ReviewFailed)
    );
    assert_eq!(
        last.reviews.iter().map(|r| r.outcome).collect::<Vec<_>>(),
        [
            events::ReviewPassOutcome::Passed,
            events::ReviewPassOutcome::Failed
        ],
        "the record says which pass objected, and that it really judged"
    );
    assert_eq!(last.reviews[1].agent, "copilot");
}

#[test]
fn a_failing_first_pass_never_spends_the_second_reviewer() {
    let (_tree, repo) = temp_engine_repo("shortcircuit");
    seed(&repo, FRONTIER_AUTH_PLAN, Some(SECOND_OPINION_CONFIG));
    let source = cross_vendor(
        vec![Effect::EditFile],
        vec![ReviewBehavior::Fail],
        vec![ReviewBehavior::Pass],
    );
    let report = run_with(&cross_vendor_opts(&repo), &source).expect("run");

    assert!(!committed(&report, "t1"));
    assert_eq!(source.adapter.reviews_run(), 1);
    assert_eq!(
        source.copilot().reviews_run(),
        0,
        "the second vendor was never asked"
    );
    let last = task(&report, "t1").attempts.last().expect("attempt");
    assert_eq!(last.reviews.len(), 1, "only what actually ran is recorded");
}

#[test]
fn a_frontier_task_is_not_reviewed_by_the_model_that_wrote_it() {
    let (_tree, repo) = temp_engine_repo("selfreview");
    seed(&repo, FRONTIER_AUTH_PLAN, Some(FRONTIER_ONLY_CONFIG));

    let source = cross_vendor(
        vec![Effect::EditFile],
        vec![ReviewBehavior::Fail],
        vec![ReviewBehavior::Pass],
    );
    let report = run_with(&cross_vendor_opts(&repo), &source).expect("run");

    assert!(committed(&report, "t1"), "{report:?}");
    assert_eq!(task(&report, "t1").review_models, ["gpt-5.3-codex"]);
    assert_eq!(source.adapter.reviews_run(), 0, "never judged its own work");
    assert_eq!(source.copilot().reviews_run(), 1);
}

#[test]
fn a_lower_rung_keeps_the_frontier_reviewer() {
    let (_tree, repo) = temp_engine_repo("noneedtorebind");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"mid\"], attempts_per = 1 }\n"),
    );
    let source = cross_vendor(
        vec![Effect::EditFile],
        vec![ReviewBehavior::Pass],
        vec![ReviewBehavior::Pass],
    );
    let report = run_with(&cross_vendor_opts(&repo), &source).expect("run");

    assert_eq!(task(&report, "t1").model, "claude-sonnet-5");
    assert_eq!(task(&report, "t1").review_models, ["claude-opus-5"]);
    assert_eq!(source.copilot().reviews_run(), 0);
}

#[test]
fn a_configured_second_opinion_with_no_second_family_refuses_before_spending() {
    let (_tree, repo) = temp_engine_repo("nosecondfamily");
    seed(&repo, FRONTIER_AUTH_PLAN, Some(SECOND_OPINION_CONFIG));
    let source = source(vec![Effect::EditFile], vec![ReviewBehavior::Pass]);
    let error = run_with(&cross_vendor_opts(&repo), &source)
        .expect_err("a promised reviewer that cannot exist must stop the run");
    let message = error.to_string();
    assert!(message.contains("t1"), "names the task: {message}");
    assert!(
        message.contains("src/auth/**"),
        "names the override: {message}"
    );
    assert!(
        message.contains("second opinion"),
        "says what is missing: {message}"
    );
    assert_eq!(source.adapter.runs().len(), 0, "nothing was spent");
}

#[test]
fn without_a_second_vendor_self_review_warns_rather_than_refusing() {
    let (_tree, repo) = temp_engine_repo("selfreviewwarn");
    seed(&repo, FRONTIER_AUTH_PLAN, Some(FRONTIER_ONLY_CONFIG));
    let source = source(vec![Effect::EditFile], vec![ReviewBehavior::Pass]);
    let report = run_with(&cross_vendor_opts(&repo), &source).expect("run still works");

    assert!(committed(&report, "t1"));
    assert_eq!(task(&report, "t1").review_models, ["claude-opus-5"]);
    assert!(
        report
            .warnings
            .iter()
            .any(|w| w.contains("t1") && w.contains("also the reviewer")),
        "warnings: {:?}",
        report.warnings
    );
}

#[test]
fn an_unprobeable_cross_family_reviewer_downgrades_instead_of_halting() {
    let (_tree, repo) = temp_engine_repo("brokencopilot");
    seed(&repo, FRONTIER_AUTH_PLAN, Some(FRONTIER_ONLY_CONFIG));
    let source = FakeSource {
        adapter: FakeAdapter::new(vec![Effect::EditFile], vec![ReviewBehavior::Pass]),
        copilot: Some(FakeAdapter::copilot(vec![ReviewBehavior::Pass]).broken("not logged in")),
    };
    let report =
        run_with(&cross_vendor_opts(&repo), &source).expect("a broken upgrade is not a broken run");

    assert!(committed(&report, "t1"));
    assert_eq!(
        task(&report, "t1").review_models,
        ["claude-opus-5"],
        "fell back to same-model review"
    );
    assert!(
        report
            .warnings
            .iter()
            .any(|w| w.contains("not logged in") && w.contains("same-model review")),
        "warnings: {:?}",
        report.warnings
    );

    assert!(
        report
            .warnings
            .iter()
            .any(|w| w.contains("t1") && w.contains("also the reviewer")),
        "warnings: {:?}",
        report.warnings
    );
}

#[test]
fn the_same_broken_reviewer_is_fatal_when_the_config_asked_for_it() {
    let (_tree, repo) = temp_engine_repo("brokenrequired");
    seed(&repo, FRONTIER_AUTH_PLAN, Some(SECOND_OPINION_CONFIG));
    let source = FakeSource {
        adapter: FakeAdapter::new(vec![Effect::EditFile], vec![ReviewBehavior::Pass]),
        copilot: Some(FakeAdapter::copilot(vec![ReviewBehavior::Pass]).broken("not logged in")),
    };
    let error = run_with(&cross_vendor_opts(&repo), &source)
        .expect_err("a required reviewer that cannot run stops the run");
    assert!(error.to_string().contains("not logged in"), "got: {error}");
}

#[test]
fn max_parallel_above_one_refuses_before_the_run_touches_the_workspace() {
    let (_tree, repo) = temp_engine_repo("maxparallelrefusal");
    seed(
        &repo,
        "## Doomed\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some(
            "[engine]\nmax_parallel = 3\n\n\
             [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n",
        ),
    );
    let head_before = git_in(&repo, &["rev-parse", "HEAD"]);
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = fake(Effect::EditFile);

    let error = run_with(&opts, &source).expect_err("a refused ceiling must not start a run");
    assert!(error.to_string().contains("max_parallel = 3"), "{error}");

    assert!(
        source.adapter.runs().is_empty(),
        "nothing may be spawned, let alone paid for"
    );
    assert!(
        rundir::list_runs(&repo).is_empty(),
        "no run directory: {:?}",
        rundir::list_runs(&repo)
    );
    assert_eq!(
        git_in(&repo, &["branch", "--list", "upstroke/run-*"]),
        "",
        "no run branch"
    );
    assert_eq!(git_in(&repo, &["rev-parse", "HEAD"]), head_before);
    assert_eq!(
        git_in(&repo, &["status", "--porcelain"]),
        "",
        "working tree untouched"
    );
}

#[test]
fn a_refused_ceiling_beats_the_lease_rather_than_racing_it() {
    let (_tree, repo) = temp_engine_repo("ceilingbeforelease");
    seed(
        &repo,
        "## Doomed\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some(
            "[engine]\nmax_parallel = 3\n\n\
             [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n",
        ),
    );
    let lease = worktree_lock_path(&repo);
    assert!(
        !lease.exists(),
        "the fixture has never taken the lease, so the file cannot exist yet"
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));

    let source = fake(Effect::EditFile);
    let refused = run_with(&opts, &source)
        .expect_err("a ceiling this engine cannot honour must not start a run")
        .to_string();
    assert!(refused.contains("max_parallel = 3"), "{refused}");
    assert!(
        !lease.exists(),
        "the worktree lock file at {} was created before the config was read",
        lease.display()
    );

    let competitor = WorktreeLock::acquire(&repo).expect("a competing holder takes the lease");
    assert!(
        lease.exists(),
        "the competing holder is what creates the file, which is how (a) means anything"
    );
    let contended = run_with(&opts, &source)
        .expect_err("a refused ceiling still refuses while somebody holds the lease")
        .to_string();
    assert!(
        contended.contains("max_parallel = 3"),
        "the config error must win the race it never needed to enter: {contended}"
    );
    assert!(
        !contended.contains("another upstroke process"),
        "lock contention must not be the diagnosis for a config error: {contended}"
    );
    drop(competitor);

    assert!(
        source.adapter.runs().is_empty(),
        "nothing may be spawned, let alone paid for"
    );
    assert!(
        rundir::list_runs(&repo).is_empty(),
        "and no run directory: {:?}",
        rundir::list_runs(&repo)
    );
}

fn config_with_repairs(repairs: u32) -> String {
    format!(
        "[engine]\nmax_merge_repairs = {repairs}\n\n\
         [routing]\nimplement = {{ chain = [\"small\"], attempts_per = 1 }}\n"
    )
}

#[test]
fn the_analysis_adopted_under_the_lease_is_the_one_its_own_bytes_were_validated_from() {
    let (_tree, repo) = temp_engine_repo("confirmunderlease");
    let config = repo.join("upstroke.toml");
    let mut opts = options(&repo);
    opts.config_path = Some(config.clone());

    fs::write(&config, config_with_repairs(5)).expect("the config before the lease");
    let validated = validate_inputs(&opts, config::EngineLimits::Fresh).expect("pre-lock check");
    fs::write(&config, config_with_repairs(7)).expect("the config at the lease");
    let analysis = validated
        .confirm_under_lease(&opts, config::EngineLimits::Fresh)
        .expect("a valid config is still valid");
    assert_eq!(
        analysis.config.max_merge_repairs, 7,
        "the adopted analysis must describe the bytes the run is holding"
    );

    fs::write(&config, config_with_repairs(5)).expect("the config before the lease");
    let validated = validate_inputs(&opts, config::EngineLimits::Fresh).expect("pre-lock check");
    fs::write(
        &config,
        "[engine]\nmax_parallel = 3\n\n\
         [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n",
    )
    .expect("a ceiling this engine cannot honour");
    let refused = validated
        .confirm_under_lease(&opts, config::EngineLimits::Fresh)
        .expect_err("an unhonourable ceiling must not be adopted because an older file was fine")
        .to_string();
    assert!(refused.contains("max_parallel = 3"), "{refused}");

    fs::write(&config, config_with_repairs(5)).expect("A");
    let validated = validate_inputs(&opts, config::EngineLimits::Fresh).expect("pre-lock check");
    fs::write(&config, config_with_repairs(9)).expect("B");
    fs::write(&config, config_with_repairs(5)).expect("A again");
    let analysis = validated
        .confirm_under_lease(&opts, config::EngineLimits::Fresh)
        .expect("A is what was captured and A is what is there");
    assert_eq!(
        analysis.config.max_merge_repairs, 5,
        "B was adopted from an excursion neither capture can see"
    );
}

#[test]
fn the_gate_derivation_is_taken_under_the_lease_not_carried_over_it() {
    let (_tree, repo) = temp_engine_repo("gatesunderlease");
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    fs::write(opts.config_path.as_ref().expect("config path"), "").expect("an empty config");

    let validated = validate_inputs(&opts, config::EngineLimits::Fresh).expect("pre-lock check");

    fs::write(repo.join("Cargo.toml"), "[package]\nname = \"x\"\n").expect("a rust repo now");
    let analysis = validated
        .confirm_under_lease(&opts, config::EngineLimits::Fresh)
        .expect("a shape change is not a refusal");
    assert_eq!(
        analysis
            .gates
            .iter()
            .map(|gate| gate.name.clone())
            .collect::<Vec<_>>(),
        vec!["check".to_owned(), "test".to_owned()],
        "the gates a run is held to must be derived from the worktree it holds"
    );
}

#[test]
fn an_unavailable_reviewer_is_recorded_as_such_not_as_a_rejection() {
    let (_tree, repo) = temp_engine_repo("outagerecord");
    seed(&repo, FRONTIER_AUTH_PLAN, Some(SECOND_OPINION_CONFIG));
    let source = cross_vendor(
        vec![Effect::EditFile],
        vec![ReviewBehavior::Pass],
        vec![ReviewBehavior::RateLimited, ReviewBehavior::Pass],
    );
    let report = run_with(&cross_vendor_opts(&repo), &source).expect("run");

    let first = &task(&report, "t1").attempts[0];
    assert_eq!(
        first.reviews.iter().map(|r| r.outcome).collect::<Vec<_>>(),
        [
            events::ReviewPassOutcome::Passed,
            events::ReviewPassOutcome::Unavailable
        ],
        "the second vendor was down, not unimpressed"
    );

    assert!(committed(&report, "t1"), "{report:?}");
}

#[test]
fn second_reviewer_spawn_failure_settles_worker_and_first_review_evidence() {
    let (_tree, repo) = temp_engine_repo("secondreviewerspawnsettlement");
    seed(&repo, FRONTIER_AUTH_PLAN, Some(SECOND_OPINION_CONFIG));
    let source = cross_vendor(
        vec![Effect::EditFile],
        vec![ReviewBehavior::Pass],
        vec![ReviewBehavior::SpawnError, ReviewBehavior::Pass],
    );
    let report = run_with(&cross_vendor_opts(&repo), &source).expect("settled run");

    assert!(
        committed(&report, "t1"),
        "the deferred retry recovers: {report:?}"
    );
    let task = task(&report, "t1");
    assert_eq!(task.attempts.len(), 2, "one settled outage, one recovery");
    let first = &task.attempts[0];
    let failure = first.failure.as_ref().expect("spawn failure is recorded");
    assert_eq!(failure.kind, FailureKind::ReviewUnavailable);
    assert_eq!(failure.origin, FailureOrigin::Reviewer);
    assert_eq!(first.cost_usd, Some(0.01), "worker spend survives");
    assert_eq!(first.session_id.as_deref(), Some("s0"));
    assert!(first.usage.is_some(), "worker usage survives");
    assert_eq!(
        first.reviews.iter().map(|r| r.outcome).collect::<Vec<_>>(),
        [
            events::ReviewPassOutcome::Passed,
            events::ReviewPassOutcome::Unavailable
        ],
        "the completed first verdict is not discarded"
    );
    assert_eq!(first.reviews[0].cost_usd, Some(0.05));
    assert_eq!(first.reviews[1].cost_usd, None);
    assert_eq!(source.copilot().review_spawn_failures(), 1);

    let logged = events_of(&repo, &report.run_id);
    assert!(logged.iter().any(|event| matches!(
        &event.body,
        EventBody::AttemptFinished {
            task,
            attempt: 1,
            ..
        } if task == "t1"
    )));
    assert!(!logged.iter().any(|event| matches!(
        &event.body,
        EventBody::AttemptInterrupted {
            task,
            attempt: 1,
            ..
        } if task == "t1"
    )));
}

#[test]
fn a_total_missing_an_unreported_reviewer_is_marked_rather_than_implied() {
    let (_tree, repo) = temp_engine_repo("partialcost");
    seed(&repo, FRONTIER_AUTH_PLAN, Some(SECOND_OPINION_CONFIG));
    let source = FakeSource {
        adapter: FakeAdapter::new(vec![Effect::EditFile], vec![ReviewBehavior::Pass]),
        copilot: Some(FakeAdapter::copilot(vec![ReviewBehavior::Pass]).unpriced()),
    };
    let report = run_with(&cross_vendor_opts(&repo), &source).expect("run");

    let t1 = task(&report, "t1");
    assert_eq!(t1.review_cost_usd, Some(0.05), "only what was reported");
    assert!(t1.review_cost_incomplete, "and it is not the whole story");
    assert!(
        report.render().contains("$0.0500?"),
        "the summary marks it: {}",
        report.render()
    );
    let ledger = report.render_ledger();
    assert!(ledger.contains("$0.0500?"), "{ledger}");
    assert!(
        ledger.contains("reports no spend"),
        "legend present: {ledger}"
    );
}

#[test]
fn every_model_that_judged_a_task_is_listed_beside_the_cost_of_all_of_them() {
    let (_tree, repo) = temp_engine_repo("reviewtrail");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some(
            "[interaction]\nmode = \"never\"\n\n\
                 [routing]\nimplement = { chain = [\"mid\", \"frontier\"], attempts_per = 1 }\n",
        ),
    );

    let source = cross_vendor(
        vec![Effect::EditFile],
        vec![ReviewBehavior::Fail, ReviewBehavior::Pass],
        vec![ReviewBehavior::Pass],
    );
    let report = run_with(&cross_vendor_opts(&repo), &source).expect("run");
    let t1 = task(&report, "t1");
    assert_eq!(t1.attempts.len(), 2, "escalated: {t1:?}");
    assert_eq!(
        t1.review_models,
        ["claude-opus-5", "gpt-5.3-codex"],
        "both judges, in the order they judged"
    );
}

#[test]
fn each_pass_writes_its_own_verdict_transcript() {
    let (_tree, repo) = temp_engine_repo("passtranscripts");
    seed(&repo, FRONTIER_AUTH_PLAN, Some(SECOND_OPINION_CONFIG));
    let source = cross_vendor(
        vec![Effect::EditFile],
        vec![ReviewBehavior::Pass],
        vec![ReviewBehavior::Pass],
    );
    let report = run_with(&cross_vendor_opts(&repo), &source).expect("run");
    let reviews = paths_of(&repo, &report.run_id).reviews();
    assert!(reviews.join("00-t1-1-review.json").is_file());
    assert!(
        reviews.join("00-t1-1-second-opinion-review.json").is_file(),
        "the second verdict cannot overwrite the first"
    );
}

#[test]
fn the_run_record_survives_completion() {
    let (_tree, repo) = temp_engine_repo("record");
    let source = fake(Effect::EditFile);
    let report = run_with(&options(&repo), &source).expect("run");
    let report_path = repo
        .join(".upstroke")
        .join("runs")
        .join(&report.run_id)
        .join("report.json");
    let text = fs::read_to_string(&report_path).expect("report.json written");
    let restored: RunReport = serde_json::from_str(&text).expect("report round-trips");
    assert_eq!(restored.tasks.len(), 2);
    assert_eq!(restored.branch, report.branch);
    assert!(matches!(
        restored.tasks[0].status,
        TaskRunStatus::Committed { .. }
    ));
    assert_eq!(
        restored.tasks[0].attempts.len(),
        1,
        "the per-attempt ledger persists too"
    );
}

#[test]
fn forward_dependencies_run_in_topo_order_not_plan_order() {
    let (_tree, repo) = temp_engine_repo("topo");
    seed(
        &repo,
        "## Second by dependency\n<!-- upstroke: id=late depends=early -->\n\n\
             ## First by dependency\n<!-- upstroke: id=early depends= -->\n",
        None,
    );

    let source = fake(Effect::EditFile);
    let report = run_with(&options(&repo), &source).expect("run");
    let ids: Vec<&str> = report.tasks.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, ["early", "late"], "dependency beats document order");
}

#[test]
fn a_contradictory_pass_fails_closed() {
    let failure = review_failure(
        review::ReviewResult::Judged(crate::ir::Verdict {
            pass: true,
            reasons: vec!["looks fine".to_owned()],
            required_changes: vec!["parameterize the SQL".to_owned()],
            needs_human: false,
        }),
        false,
    )
    .expect("a pass that still demands changes cannot commit");
    assert_eq!(failure.kind, FailureKind::ReviewFailed);
    assert!(
        failure.reason.contains("parameterize the SQL"),
        "{}",
        failure.reason
    );

    assert!(
        review_failure(
            review::ReviewResult::Judged(crate::ir::Verdict {
                pass: true,
                reasons: vec!["meets the criteria".to_owned()],
                required_changes: Vec::new(),
                needs_human: false,
            }),
            false
        )
        .is_none()
    );
}

#[test]
fn an_unavailable_reviewer_is_not_a_rejection() {
    const LIMIT: &str = "You've hit your 5-hour limit · resets 5:30am (UTC)\n";
    let failure = review_failure(
        review::ReviewResult::Unavailable {
            status: OutcomeStatus::RateLimited,
            detail: "5-hour limit reached".to_owned(),
            limit_message: None,
        },
        false,
    )
    .expect("still fails the attempt");
    assert_eq!(failure.kind, FailureKind::RateLimited);
    assert_eq!(failure.origin, FailureOrigin::Reviewer);
    assert!(failure.is_outage(), "defers instead of blaming the worker");
    assert!(failure.reason.contains("reviewer unavailable"));
    assert_eq!(failure.limit_message(), None);

    let failure = review_failure(
        review::ReviewResult::Unavailable {
            status: OutcomeStatus::RateLimited,
            detail: "request failed".to_owned(),
            limit_message: Some(LIMIT.to_owned()),
        },
        false,
    )
    .expect("still fails the attempt");
    assert_eq!(failure.kind, FailureKind::RateLimited);
    assert_eq!(failure.origin, FailureOrigin::Reviewer);
    assert_eq!(failure.reason, "reviewer unavailable: request failed");
    assert_eq!(
        failure.limit_message(),
        Some(LIMIT),
        "the reviewer's stderr limit rides on its failure as a worker's does"
    );

    let failure = review_failure(
        review::ReviewResult::Unavailable {
            status: OutcomeStatus::Timeout,
            detail: String::new(),
            limit_message: None,
        },
        false,
    )
    .expect("still fails");
    assert_eq!(failure.kind, FailureKind::Timeout);
    assert_eq!(failure.origin, FailureOrigin::Reviewer);

    let failure = review_failure(
        review::ReviewResult::Unavailable {
            status: OutcomeStatus::AgentError,
            detail: "spawn failed".to_owned(),
            limit_message: None,
        },
        false,
    )
    .expect("still fails");
    assert_eq!(failure.kind, FailureKind::ReviewUnavailable);
}

#[test]
fn required_changes_reach_the_retry_as_a_clean_list() {
    let failure = review_failure(
        review::ReviewResult::Judged(crate::ir::Verdict {
            pass: false,
            reasons: vec!["incomplete".to_owned()],
            required_changes: vec![
                "handle the empty-input case".to_owned(),
                "add a round-trip test".to_owned(),
            ],
            needs_human: false,
        }),
        false,
    )
    .expect("fails");
    assert_eq!(
        failure.feedback.as_deref(),
        Some("- handle the empty-input case\n- add a round-trip test"),
        "every item bulleted, including the first"
    );
    assert_eq!(
        failure.origin,
        FailureOrigin::Worker,
        "a rejected diff is the worker's to fix"
    );
}

#[test]
fn prompt_names_the_allowed_gate_commands() {
    let task = Task {
        id: TaskId::from("t1"),
        kind: TaskKind::Implement,
        title: "Do the thing".to_owned(),
        body: String::new(),
        depends_on: Vec::new(),
        acceptance: Vec::new(),
        path_hints: Vec::new(),
        suggested_tier: None,
        min_tier: None,
        artifacts_in: Vec::new(),
        artifacts_out: Vec::new(),
    };
    let tree = temp_engine_scratch("prompt");
    let run_dir = tree.path();
    fs::create_dir_all(run_dir.join("artifacts")).expect("run dir");
    let prompt = materialize_prompt(
        crate::engine::assembly::WorkerSubject::of(&task),
        &["cargo check --all-targets".to_owned()],
        run_dir,
        None,
    );
    assert!(prompt.contains("EXACTLY these commands"));
    assert!(prompt.contains("- cargo check --all-targets"));
    assert!(
        prompt.contains(QUESTION_MARKER),
        "the worker is told how to ask (§12)"
    );
    let bare = materialize_prompt(
        crate::engine::assembly::WorkerSubject::of(&task),
        &[],
        run_dir,
        None,
    );
    assert!(!bare.contains("EXACTLY these commands"));
}

#[test]
fn prompt_wires_artifacts_to_real_files() {
    let tree = temp_engine_scratch("artifact");
    let run_dir = tree.path();
    fs::create_dir_all(run_dir.join("artifacts")).expect("run dir");
    let mut task = Task {
        id: TaskId::from("t1"),
        kind: TaskKind::Implement,
        title: "Build it".to_owned(),
        body: String::new(),
        depends_on: Vec::new(),
        acceptance: Vec::new(),
        path_hints: Vec::new(),
        suggested_tier: None,
        min_tier: None,
        artifacts_in: vec![crate::ir::ArtifactId::from("api-contract")],
        artifacts_out: vec![crate::ir::ArtifactId::from("notes")],
    };

    let prompt = materialize_prompt(
        crate::engine::assembly::WorkerSubject::of(&task),
        &[],
        run_dir,
        None,
    );
    assert!(prompt.contains("did \n     not leave one") || prompt.contains("did not leave one"));
    assert!(
        prompt.contains("write artifact `notes`"),
        "producer told where to write"
    );

    fs::write(
        artifact_path(run_dir, "api-contract"),
        "cursor = base64(offset)",
    )
    .expect("artifact");
    let prompt = materialize_prompt(
        crate::engine::assembly::WorkerSubject::of(&task),
        &[],
        run_dir,
        None,
    );
    assert!(
        prompt.contains("cursor = base64(offset)"),
        "content inlined"
    );

    task.artifacts_in.clear();
    task.artifacts_out.clear();
    let bare = materialize_prompt(
        crate::engine::assembly::WorkerSubject::of(&task),
        &[],
        run_dir,
        None,
    );
    assert!(!bare.contains("artifact"));
}

#[test]
fn a_gate_failure_recovers_on_the_same_rung_via_session_resume() {
    let (_tree, repo) = temp_engine_repo("resume");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some(
            "[routing]\nimplement = { chain = [\"small\"], attempts_per = 2 }\n\n\
                 [[gates]]\nname = \"needs-test\"\ncmd = \"git ls-files --error-unmatch \
                 widget_test.rs\"\n",
        ),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = source(
        vec![Effect::EditFile, Effect::EditTest],
        vec![ReviewBehavior::Pass],
    );
    let report = run_with(&opts, &source).expect("run");

    assert!(committed(&report, "t1"), "report: {report:?}");
    let t1 = task(&report, "t1");
    assert_eq!(t1.attempts.len(), 2, "one retry, not an escalation");
    assert_eq!(t1.attempts[0].tier, "small");
    assert_eq!(t1.attempts[1].tier, "small", "same rung");
    assert!(!t1.attempts[0].resumed);
    assert!(t1.attempts[1].resumed, "§11.4 retries in-session");

    let runs = source.adapter.runs();
    assert_eq!(runs[0].resume, None);
    assert_eq!(
        runs[1].resume.as_deref(),
        Some("s0"),
        "the retry resumed the failed attempt's session"
    );
    assert!(
        runs[1].prompt.contains("gate `needs-test` failed"),
        "the gate's own words go back: {}",
        runs[1].prompt
    );
    assert!(
        !runs[1].prompt.contains("# Task:"),
        "a resumed session already holds the task; the prompt stays terse"
    );

    let files = git_in(&repo, &["show", "--name-only", "--format=", "HEAD"]);
    assert!(files.contains("agent-output.txt"), "files: {files}");
    assert!(files.contains("widget_test.rs"), "files: {files}");
}

#[test]
fn exhausting_a_rung_escalates_with_a_fresh_session_and_the_history() {
    let (_tree, repo) = temp_engine_repo("escalate");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\", \"mid\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = source(
        vec![Effect::NoEdit, Effect::EditFile],
        vec![ReviewBehavior::Pass],
    );
    let report = run_with(&opts, &source).expect("run");

    assert!(committed(&report, "t1"), "report: {report:?}");
    let t1 = task(&report, "t1");
    assert_eq!(t1.attempts.len(), 2);
    assert_eq!(t1.attempts[0].tier, "small");
    assert_eq!(t1.attempts[0].model, "claude-haiku-4-5");
    assert_eq!(t1.attempts[1].tier, "mid", "one rung up");
    assert_eq!(t1.attempts[1].model, "claude-sonnet-5");
    assert!(
        !t1.attempts[1].resumed,
        "§11.4: a new rung is a new session — a different model cannot \
             inherit another's conversation"
    );
    assert_eq!(t1.trail(), "small failed → mid ok");

    let runs = source.adapter.runs();

    assert_eq!(runs[0].model, "claude-haiku-4-5");
    assert_eq!(runs[1].model, "claude-sonnet-5");
    assert_eq!(runs[1].resume, None, "fresh session");
    assert!(
        runs[1].prompt.contains("# Task:"),
        "a fresh worker gets the whole task again"
    );
    assert!(
        runs[1].prompt.contains("diff is empty"),
        "and what the previous rung got wrong: {}",
        runs[1].prompt
    );
}

#[test]
fn agent_errors_and_empty_diffs_carry_feedback_the_retry_can_use() {
    let (_tree, repo) = temp_engine_repo("feedback");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 2 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = source(
        vec![Effect::Error, Effect::EditFile],
        vec![ReviewBehavior::Pass],
    );
    let report = run_with(&opts, &source).expect("run");
    assert!(committed(&report, "t1"), "report: {report:?}");
    let runs = source.adapter.runs();
    assert!(
        runs[1].prompt.contains("fake adapter error detail"),
        "the adapter's own diagnosis reaches the retry: {}",
        runs[1].prompt
    );
}

#[test]
fn an_unparseable_reviewer_fails_after_one_reask() {
    let (_tree, repo) = temp_engine_repo("reviewprose");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = source(vec![Effect::EditFile], vec![ReviewBehavior::Unparseable]);
    let report = run_with(&opts, &source).expect("engine ok");

    let failure = task(&report, "t1").attempts[0]
        .failure
        .as_ref()
        .expect("a reviewer that never answers cannot pass a task");
    assert_eq!(failure.kind, FailureKind::ReviewFailed);
    assert!(
        failure.reason.contains("re-ask"),
        "reason: {}",
        failure.reason
    );

    let reviews = paths_of(&repo, &report.run_id).reviews();
    assert!(reviews.join("00-t1-1-review.json").is_file());
    assert!(
        reviews.join("00-t1-1-review-reask.json").is_file(),
        "one re-ask before giving up (§11.2)"
    );
    assert_eq!(
        git_in(&repo, &["rev-list", "--count", "main..HEAD"]).trim(),
        "0",
        "nothing commits without a passing verdict"
    );
}

#[test]
fn gate_logs_are_named_by_the_collision_free_stem() {
    let (_tree, repo) = temp_engine_repo("gatelogs");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some(
            "[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n\n\
                 [[gates]]\nname = \"never\"\ncmd = \"git frobnicate-not-a-command\"\n",
        ),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = fake(Effect::EditFile);
    let report = run_with(&opts, &source).expect("engine ok");

    let failure = task(&report, "t1").attempts[0]
        .failure
        .as_ref()
        .expect("gate should fail");
    assert_eq!(failure.kind, FailureKind::GateFailed);
    let gates_dir = paths_of(&repo, &report.run_id).gates();
    assert!(
        gates_dir.join("00-t1-1-never.log").is_file(),
        "the log stem matches the task's other artifacts, so two ids that \
             sanitize alike cannot overwrite each other"
    );
    assert!(
        git_in(&repo, &["status", "--porcelain"]).trim().is_empty(),
        "rolled back"
    );
}

#[test]
fn the_trail_summarizes_the_ladder() {
    let report = TaskReport {
        id: "t1".to_owned(),
        title: "x".to_owned(),
        model: "m".to_owned(),
        status: TaskRunStatus::Skipped,
        duration: Duration::ZERO,
        cost_usd: None,
        review_models: Vec::new(),
        review_cost_usd: None,
        review_cost_incomplete: false,
        session_id: None,
        attempts: vec![
            attempt_record(1, "small", true),
            attempt_record(2, "small", true),
            attempt_record(3, "mid", false),
        ],
    };
    assert_eq!(report.trail(), "small×2 failed → mid ok");
}

fn attempt_record(attempt: u32, tier: &str, failed: bool) -> AttemptRecord {
    AttemptRecord {
        attempt,
        tier: tier.to_owned(),
        model: "m".to_owned(),
        pool: None,
        resumed: false,
        duration: Duration::ZERO,
        cost_usd: None,
        reviews: Vec::new(),
        session_id: None,
        usage: None,
        failure: failed.then(|| FailureRecord {
            kind: FailureKind::GateFailed,
            origin: FailureOrigin::Worker,
            reason: "no".to_owned(),
            detail: None,
        }),
    }
}

fn raw_object_after(line: &str, key: &str) -> Option<String> {
    let start = line.find(key)? + key.len();
    let bytes: Vec<char> = line[start..].chars().collect();
    if bytes.first() != Some(&'{') {
        return None;
    }
    let (mut depth, mut in_string, mut escaped) = (0usize, false, false);
    for (index, c) in bytes.iter().enumerate() {
        if escaped {
            escaped = false;
            continue;
        }
        match c {
            '\\' if in_string => escaped = true,
            '"' => in_string = !in_string,
            '{' if !in_string => depth += 1,
            '}' if !in_string => {
                depth -= 1;
                if depth == 0 {
                    return Some(bytes[..=index].iter().collect());
                }
            }
            _ => {}
        }
    }
    None
}

#[test]
fn the_legacy_wire_and_report_carry_no_feedback_on_the_attempt_record() {
    let (_tree, repo) = temp_engine_repo("legacy-no-detail");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some(
            "[routing]\nimplement = { chain = [\"small\"], attempts_per = 2 }\n\n\
                 [[gates]]\nname = \"needs-test\"\ncmd = \"git ls-files --error-unmatch \
                 widget_test.rs\"\n",
        ),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = source(
        vec![Effect::EditFile, Effect::EditTest],
        vec![ReviewBehavior::Pass],
    );
    run_with(&opts, &source).expect("run");

    let runs = opts.repo_root.join(".upstroke").join("runs");
    let public = fs::read_dir(&runs)
        .expect("the runs root")
        .flatten()
        .map(|entry| entry.path())
        .find(|path| path.join("events.jsonl").is_file())
        .expect("the run left a public directory");
    let log = fs::read_to_string(public.join("events.jsonl")).expect("the log");

    const PRE_CHANGE_FAILURE: &str = concat!(
        r#"{"kind":"gate_failed","origin":"worker","reason":"gate `needs-test` failed: "#,
        r#"error: pathspec 'widget_test.rs' did not match any file(s) known to git\n"#,
        r#"Did you forget to 'git add'?\n\nexit code: Some(1)"}"#,
    );

    let mut failures = 0;
    let mut carried_tail = false;
    for line in log.lines() {
        let event: serde_json::Value = serde_json::from_str(line).expect("a json line");
        if event.get("event").and_then(serde_json::Value::as_str) != Some("attempt_finished") {
            continue;
        }
        let Some(failure) = event.pointer("/data/failure").filter(|v| !v.is_null()) else {
            continue;
        };
        failures += 1;

        let bytes =
            raw_object_after(line, "\"failure\":").expect("the line carries a failure object");
        let stripped = bytes.replace(",\"detail\":null", "");
        assert_ne!(
            stripped, bytes,
            "the legacy failure record carries no `detail` key at all: {bytes}. This test \
             asserts the *only* difference from `610106b` is that one null, and if the key \
             is absent the assertion below is vacuous"
        );
        assert_eq!(
            bytes.matches(",\"detail\":").count(),
            1,
            "expected exactly one `detail` key in {bytes}"
        );

        assert_eq!(
            stripped, PRE_CHANGE_FAILURE,
            "the legacy failure record is not the bytes `610106b` wrote for this \
             scenario. If a newer git reworded its pathspec error, re-capture the \
             fixture at that commit rather than loosening this comparison"
        );

        let object = failure.as_object().expect("the failure is an object");
        assert_eq!(
            object.get("detail"),
            Some(&serde_json::Value::Null),
            "the legacy attempt record's `detail` is {:?}; it must be present and null — \
             a value would be §11.4's feedback duplicated onto the wire, and an absent \
             key would mean the field stopped serializing, which breaks schema 4's \
             strict door",
            object.get("detail")
        );

        if event
            .pointer("/transition/data/detail")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|d| d.contains("widget_test.rs"))
        {
            carried_tail = true;
        }
    }
    assert!(
        failures >= 1,
        "no failed attempt in the log, so this test asserted nothing"
    );
    assert!(
        carried_tail,
        "the gate tail reached no `ladder_retry` transition either, so the legacy engine \
         lost §11.4's feedback rather than keeping it where it belongs"
    );

    let report: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(public.join("report.json")).expect("report.json is written"),
    )
    .expect("report.json is json");
    let mut checked = 0;
    for task in report["tasks"].as_array().into_iter().flatten() {
        for attempt in task["attempts"].as_array().into_iter().flatten() {
            let Some(failure) = attempt.get("failure").filter(|v| !v.is_null()) else {
                continue;
            };
            checked += 1;
            let object = failure.as_object().expect("the failure is an object");
            assert_eq!(
                object.get("detail"),
                Some(&serde_json::Value::Null),
                "report.json's attempt {} carries `detail` as {:?}; it must be present \
                 and null. `is_null()` was the earlier assertion and it cannot tell an \
                 explicit null from an absent key, so it would have passed had the \
                 field silently stopped serializing",
                attempt["attempt"],
                object.get("detail")
            );
        }
    }
    assert!(
        checked >= 1,
        "no failed attempt reached report.json, so its half of this test asserted nothing"
    );

    let runs = source.adapter.runs();
    assert!(
        runs[1].prompt.contains("gate `needs-test` failed"),
        "the legacy retry lost the gate's words: {}",
        runs[1].prompt
    );
}

#[test]
fn an_explicit_null_detail_survives_the_strict_door() {
    use crate::topology::events::{
        AttemptFinished4, AttemptNumber, AttemptSettlement, GenerationId, LeaseDisposition,
        SettlementTransition, TopologyEvent, TopologyEventBody,
    };

    let mut record = serde_json::to_value(attempt_record(1, "mid", true)).expect("serialize");
    let failure = record
        .get_mut("failure")
        .and_then(serde_json::Value::as_object_mut)
        .expect("a failed record carries a failure object");

    failure.insert("detail".to_owned(), serde_json::Value::Null);

    let mut value = serde_json::to_value(TopologyEvent::now(TopologyEventBody::AttemptFinished {
        data: Box::new(AttemptFinished4 {
            key: crate::topology::registry::TaskKey(0),
            generation: GenerationId(0),
            attempt: AttemptNumber(1),
            record: Box::new(attempt_record(1, "mid", true)),
            settlement: AttemptSettlement::Closed {
                transition: SettlementTransition::Retry,
                lease: LeaseDisposition::PredictedReleased,
            },
        }),
    }))
    .expect("the event serializes");
    value["data"]["record"] = record;

    let parsed: TopologyEvent = serde_json::from_value(value).expect(
        "an explicit null on a known optional field must pass the strict door; if this \
         refuses, the door is reporting a field the record does declare, and every \
         failed attempt's settlement is unreadable",
    );
    let TopologyEventBody::AttemptFinished { data } = parsed.body else {
        unreachable!("built as an attempt_finished")
    };
    assert_eq!(
        data.record
            .failure
            .as_ref()
            .expect("the failure survives")
            .detail,
        None,
        "an explicit null must read back as None"
    );
}

#[test]
fn both_feedback_sources_reach_the_durable_attempt_record() {
    use crate::gates::GateFailure;

    let tail =
        "error[E0308]: mismatched types\n  --> src/alpha.rs:12:9\n   expected `u32`, found `&str`";
    let gate = super::classify::gate_failure(&GateFailure {
        gate: "cargo test".to_owned(),
        summary: "1 failed".to_owned(),
        log_tail: tail.to_owned(),
    });
    assert_eq!(
        durable_detail(&gate).as_deref(),
        Some(tail),
        "§11.1's gate tail did not reach the record, so a resume cannot tell the \
         retry what the gate printed"
    );

    let review = super::attempt::review_failure(
        review::ReviewResult::Judged(crate::ir::Verdict {
            pass: false,
            reasons: vec!["the parser accepts a trailing comma".to_owned()],
            required_changes: vec![
                "reject a trailing comma in `parse_list`".to_owned(),
                "add a case for the empty list".to_owned(),
            ],
            needs_human: false,
        }),
        false,
    )
    .expect("a failed verdict is a failure");
    assert_eq!(
        durable_detail(&review).as_deref(),
        Some("- reject a trailing comma in `parse_list`\n- add a case for the empty list"),
        "§11.2's required_changes did not reach the record verbatim, so an \
         escalation carries the reviewer's summary instead of its instructions"
    );
}

fn durable_detail(failure: &crate::ladder::AttemptFailure) -> Option<String> {
    let outcome = crate::ir::Outcome {
        status: crate::ir::OutcomeStatus::Completed,
        diff: String::new(),
        detail: None,
        session_id: None,
        usage: None,
        cost_usd: None,
        transcript_path: PathBuf::new(),
        duration: Duration::ZERO,
    };
    super::classify::attempt_record(
        1,
        super::classify::AttemptFacts {
            tier: crate::ir::Tier::Mid,
            model: "claude-opus-5",
            pool: None,
            resumed: false,
            outcome: &outcome,
            reviews: &[],
            failure: Some(failure),

            feedback: super::classify::FeedbackCarrier::AttemptRecord,
        },
    )
    .failure
    .expect("a classified failure produces a failure record")
    .detail
}

fn program_stem(program: &str) -> String {
    Path::new(program)
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

#[test]
fn the_legacy_engine_routes_every_process_through_the_runner() {
    let (_tree, repo) = temp_engine_repo("routed");
    seed(
        &repo,
        "## One\n<!-- upstroke: id=t1 kind=implement depends= -->\n\n\
             ## Two\n<!-- upstroke: id=t2 kind=implement depends= -->\n",
        Some(
            "[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n\n\
                 [[gates]]\nname = \"first\"\ncmd = \"echo gate-one\"\n\n\
                 [[gates]]\nname = \"second\"\ncmd = \"echo gate-two\"\n",
        ),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = source(vec![Effect::EditFile], vec![ReviewBehavior::Pass]);
    let runner = RecordingRunner::new();
    let report = run_harness_on(
        &opts,
        &Harness {
            adapters: &source,
            answers: None,
            sleeper: None,
        },
        &runner,
    )
    .expect("run");

    assert!(committed(&report, "t1"), "report: {report:?}");
    assert!(committed(&report, "t2"), "report: {report:?}");
    let seen = runner.seen();

    let expected_ids = vec![
        "k0.g0.a1.worker.o0",
        "k0.g0.a1.gate0.o0",
        "k0.g0.a1.gate1.o0",
        "k0.g0.a1.review_pass0.o0",
        "k1.g0.a1.worker.o0",
        "k1.g0.a1.gate0.o0",
        "k1.g0.a1.gate1.o0",
        "k1.g0.a1.review_pass0.o0",
    ];
    let ids: Vec<&str> = seen.iter().map(|p| p.invocation.as_str()).collect();
    assert_eq!(ids, expected_ids, "recorded: {seen:#?}");
    assert_eq!(
        seen.len(),
        2 * (1 + 2 + 1),
        "two tasks x (worker + two gates + one review)"
    );
    assert_eq!(
        ids.iter().collect::<std::collections::BTreeSet<_>>().len(),
        ids.len(),
        "two processes of one run share an identity"
    );
    assert!(
        ids.iter().all(|id| id.contains(".g0.")),
        "the legacy engine assigns legacy-scoped values: generation 0"
    );

    use crate::runner::ExecutionRole;
    let by_role = |role: &ExecutionRole| seen.iter().filter(|p| &p.role == role).count();
    assert_eq!(by_role(&ExecutionRole::Implement), 2);
    assert_eq!(by_role(&ExecutionRole::Gate), 4);
    assert_eq!(by_role(&ExecutionRole::Review), 2);
    for process in &seen {
        match process.role {
            ExecutionRole::Implement | ExecutionRole::Review => {
                assert!(process.slotted, "{process:?}");
                assert_eq!(process.agent.as_deref(), Some("claude-code"), "{process:?}");
            }
            ExecutionRole::Gate => {
                assert!(!process.slotted, "{process:?}");
                assert_eq!(process.agent, None, "{process:?}");
            }
            ExecutionRole::Probe(_) => panic!("the legacy engine probes nothing: {process:?}"),
        }
    }

    let worker_stdin = &seen
        .iter()
        .find(|p| p.role == ExecutionRole::Implement)
        .expect("a worker")
        .stdin;
    assert!(
        worker_stdin.contains("## One") || worker_stdin.contains("One"),
        "the worker prompt is delivered on stdin: {worker_stdin:?}"
    );
    assert!(
        worker_stdin.contains("Acceptance") || worker_stdin.len() > 200,
        "and it is the materialized prompt, not a token: {} bytes",
        worker_stdin.len()
    );
    let review_stdin = &seen
        .iter()
        .find(|p| p.role == ExecutionRole::Review)
        .expect("a review")
        .stdin;
    assert!(
        review_stdin.contains("READ-ONLY"),
        "the review prompt is delivered on stdin: {review_stdin:?}"
    );
    assert_ne!(
        worker_stdin, review_stdin,
        "a worker and a judge are not sent the same prompt"
    );
    for gate in seen.iter().filter(|p| p.role == ExecutionRole::Gate) {
        assert!(gate.stdin.is_empty(), "a gate reads no stdin: {gate:?}");
    }

    let shell_probes = |seen: &[RoutedProcess]| {
        seen.iter()
            .filter(|p| p.role == ExecutionRole::Probe(crate::runner::ProbeTarget::Shell))
            .count()
    };
    assert_eq!(
        shell_probes(&seen),
        0,
        "the legacy engine ran a shell probe"
    );
    crate::runner::host::run_shell_probe(
        &runner,
        crate::gates::ShellKind::native(),
        repo.clone(),
        crate::runner::InvocationId::probe(crate::runner::ProbeTarget::Shell, 0)
            .expect("the shell probe identity"),
    )
    .expect("the recorded shell runs `exit 0`");
    assert_eq!(
        shell_probes(&runner.seen()),
        1,
        "the recorder cannot see a shell probe, so the zero above proved nothing"
    );

    assert!(
        seen.iter().all(|p| program_stem(&p.program) != "git"),
        "a git process went through the Runner: {seen:#?}"
    );
    assert!(
        !git_in(&repo, &["log", "--oneline", &report.branch]).is_empty(),
        "the run's branch has commits, so authoritative Git did run"
    );

    let worker = seen
        .iter()
        .find(|p| p.role == ExecutionRole::Implement)
        .expect("a worker");
    assert!(
        crate::util::same_path(&worker.workspace, &repo),
        "the worker runs in the repo root: {} is not {}",
        worker.workspace.display(),
        repo.display()
    );
    for process in seen.iter().filter(|p| p.role != ExecutionRole::Implement) {
        assert!(
            !crate::util::same_path(&process.workspace, &repo),
            "a gate or reviewer judged the live worktree: {process:?}"
        );
        assert!(process.workspace.is_absolute(), "{process:?}");
    }
}

#[test]
fn a_retried_attempt_with_two_passes_and_a_reask_assigns_every_identity_from_production() {
    let (_tree, repo) = temp_engine_repo("identities");
    seed(
        &repo,
        FRONTIER_AUTH_PLAN,
        Some(
            "[routing]\n\
             implement = { chain = [\"frontier\"], attempts_per = 2 }\n\n\
             [[routing.overrides]]\n\
             paths = [\"src/auth/**\"]\n\
             second_opinion = \"different-vendor\"\n\n\
             [[gates]]\nname = \"first\"\ncmd = \"echo gate-one\"\n\n\
             [[gates]]\nname = \"second\"\ncmd = \"echo gate-two\"\n",
        ),
    );
    let source = cross_vendor(
        vec![Effect::NoEdit, Effect::EditFile],
        vec![ReviewBehavior::Unparseable, ReviewBehavior::Pass],
        vec![ReviewBehavior::Pass],
    );
    let runner = RecordingRunner::new();
    let report = run_harness_on(
        &cross_vendor_opts(&repo),
        &Harness {
            adapters: &source,
            answers: None,
            sleeper: None,
        },
        &runner,
    )
    .expect("run");
    assert!(committed(&report, "t1"), "report: {report:?}");

    let seen = runner.seen();
    let ids: Vec<&str> = seen.iter().map(|p| p.invocation.as_str()).collect();
    assert_eq!(
        ids,
        vec![
            "k0.g0.a1.worker.o0",
            "k0.g0.a2.worker.o0",
            "k0.g0.a2.gate0.o0",
            "k0.g0.a2.gate1.o0",
            "k0.g0.a2.review_pass0.o0",
            "k0.g0.a2.review_reask0.o0",
            "k0.g0.a2.review_pass1.o0",
        ],
        "recorded: {seen:#?}"
    );

    use std::collections::BTreeSet;
    let attempts: BTreeSet<&str> = ids
        .iter()
        .map(|id| id.split('.').nth(2).expect("the attempt field"))
        .collect();
    assert_eq!(
        attempts,
        BTreeSet::from(["a1", "a2"]),
        "two attempt numbers"
    );
    let roles: BTreeSet<&str> = ids
        .iter()
        .map(|id| id.split('.').nth(3).expect("the role field"))
        .collect();
    assert_eq!(
        roles,
        BTreeSet::from([
            "worker",
            "gate0",
            "gate1",
            "review_pass0",
            "review_reask0",
            "review_pass1",
        ]),
        "six distinct role members across the run"
    );
    assert_eq!(
        ids.iter().collect::<BTreeSet<_>>().len(),
        ids.len(),
        "two processes of one run share an identity"
    );

    assert_eq!(
        source.adapter.reviews_run(),
        2,
        "the primary reviewer's verdict and its one re-ask"
    );
    assert_eq!(
        source.copilot().reviews_run(),
        1,
        "the second family answered once, and was not re-asked"
    );
}

#[test]
fn a_worker_that_cannot_be_spawned_returns_an_error_and_settles_nothing() {
    let (_tree, repo) = temp_engine_repo("workerspawn");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 2 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = source(vec![Effect::SpawnError], vec![ReviewBehavior::Pass]);
    let error = run_with(&opts, &source).expect_err(
        "a worker that cannot be spawned is an infrastructure error, not a run that finished",
    );
    let message = error.to_string();
    assert!(
        message.contains("failed to spawn"),
        "the runner's own diagnostic reaches the caller: {message}"
    );
    assert!(
        message.contains("missing-worker-executable"),
        "and it names the program: {message}"
    );

    let run_id = rundir::latest_run(&repo).expect("the run created its directory");
    let log = fs::read_to_string(paths_of(&repo, &run_id).events()).expect("events.jsonl");
    let kinds: Vec<String> = log
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str::<serde_json::Value>(line)
                .expect("an event")
                .get("event")
                .and_then(|kind| kind.as_str())
                .unwrap_or_default()
                .to_owned()
        })
        .collect();
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| *kind == "attempt_started")
            .count(),
        1,
        "exactly one attempt was dispatched: {kinds:?}"
    );
    for settled in [
        "attempt_finished",
        "task_failed",
        "task_completed",
        "run_finished",
    ] {
        assert_eq!(
            kinds.iter().filter(|kind| *kind == settled).count(),
            0,
            "a spawn failure synthesized `{settled}`: {kinds:?}"
        );
    }
    assert_eq!(source.adapter.runs().len(), 1, "the ladder bought no retry");
}

#[test]
fn every_public_write_coordinator_entry_point_establishes_containment() {
    let (_tree, repo) = temp_engine_repo("containment-facade");
    let mut run_opts = options(&repo);
    run_opts.plan_path = repo.join("absent-plan.md");
    let (_legacy_tree, legacy_repo, legacy_run) = parked_run("containment-facade-resume");
    fs::remove_file(legacy_repo.join("plan.md")).expect("the recorded run's plan, removed");
    let resume_opts = resume_options(&legacy_repo, &legacy_run);
    let adapters = BuiltinAdapters;

    type Call<'a> = Box<dyn Fn() -> Result<RunReport, UpstrokeError> + 'a>;
    let entry_points: Vec<(&str, Call<'_>)> = vec![
        ("run", Box::new(|| run(&run_opts))),
        ("run_with", Box::new(|| run_with(&run_opts, &adapters))),
        (
            "run_harness",
            Box::new(|| run_harness(&run_opts, &Harness::new(&adapters))),
        ),
        ("resume", Box::new(|| resume(&resume_opts))),
        (
            "resume_with",
            Box::new(|| resume_with(&resume_opts, &adapters)),
        ),
        (
            "resume_harness",
            Box::new(|| resume_harness(&resume_opts, &Harness::new(&adapters))),
        ),
    ];

    let mut driven: Vec<&str> = entry_points.iter().map(|(name, _)| *name).collect();
    driven.sort_unstable();
    assert_eq!(
        driven,
        public_facade_entry_points(),
        "a public engine entry point is not driven here; every one of them makes its caller a \
         write coordinator"
    );
    assert_eq!(driven.len(), 6, "six entry points, and this is the count");

    for (name, call) in &entry_points {
        let before = crate::runner::host::containment_establishments();
        let outcome = call();
        assert!(
            outcome.is_err(),
            "{name}: the fixture relies on this refusing on its own input"
        );
        assert_eq!(
            crate::runner::host::containment_establishments(),
            before + 1,
            "`engine::{name}` entered the write coordinator without establishing containment \
             (INV-18); a kill after CreateProcessW and before private-job assignment would leave \
             a suspended stub alive"
        );

        #[cfg(windows)]
        assert!(
            crate::agent::proc::ambient_job_established(),
            "`engine::{name}` returned without this process joining its ambient Job Object"
        );
    }

    let mut unknown = ResumeOptions::new("01ABSENTRUN".to_owned(), repo.clone());
    unknown.pools_path = Some(no_pools());
    unknown.private_root = Some(private_root_for(&repo));
    let says = rundir::resolve_run_id(&repo, "01ABSENTRUN")
        .expect_err("no run has that id")
        .to_string();
    let refused_by_the_router: Vec<(&str, Call<'_>)> = vec![
        ("resume", Box::new(|| resume(&unknown))),
        ("resume_with", Box::new(|| resume_with(&unknown, &adapters))),
        (
            "resume_harness",
            Box::new(|| resume_harness(&unknown, &Harness::new(&adapters))),
        ),
    ];
    for (name, call) in &refused_by_the_router {
        let before = crate::runner::host::containment_establishments();
        let refused = call().expect_err("an id the router refuses");
        assert_eq!(
            crate::runner::host::containment_establishments(),
            before,
            "`engine::{name}`: an id the router refuses is refused read-only, before any \
             containment, so the caller never becomes a write coordinator"
        );
        assert_eq!(
            refused.to_string(),
            says,
            "`engine::{name}`: the resolver's own refusal"
        );
    }
}

#[test]
fn no_read_only_public_entry_point_establishes_containment() {
    let (_tree, repo) = temp_engine_repo("containment-readonly");
    let scratch = private_root_for(&repo);
    fs::create_dir_all(&scratch).expect("scratch");
    let absent = repo.join("absent-plan.md");

    type Call<'a> = Box<dyn Fn() + 'a>;
    let read_only: Vec<(&str, Call<'_>)> = vec![
        (
            "validate::run",
            Box::new(|| {
                let _ = crate::validate::run(&crate::validate::ValidateOptions {
                    plan_path: absent.clone(),
                    config_path: None,
                    config_root: repo.clone(),
                    pools_path: Some(no_pools()),
                    engine_limits: config::EngineLimits::Fresh,
                });
            }),
        ),
        (
            "status::load",
            Box::new(|| {
                let _ = crate::status::load(&repo, None);
            }),
        ),
        (
            "export::load",
            Box::new(|| {
                let _ = crate::export::load(&repo, "01ABSENTRUN");
            }),
        ),
        (
            "answer::answer",
            Box::new(|| {
                let _ = crate::answer::answer(&repo, "q1", crate::answer::Reply::Decline);
            }),
        ),
        (
            "capacity::report",
            Box::new(|| {
                let _ = capacity::report(
                    &capacity::CapacityOptions {
                        config_path: Some(absent.clone()),
                        pools_path: Some(no_pools()),
                        repo_root: repo.clone(),
                        ..capacity::CapacityOptions::default()
                    },
                    &BuiltinAdapters,
                );
            }),
        ),
        (
            "connect::run_with",
            Box::new(|| {
                let _ = crate::connect::run_with(
                    &crate::connect::ConnectOptions {
                        pools_path: Some(scratch.join("pools.toml")),
                        force: true,
                    },
                    &BuiltinAdapters,
                    std::iter::empty(),
                );
            }),
        ),
    ];
    assert_eq!(
        read_only.len(),
        6,
        "one library entry point per read-only subcommand — the same six \
         `src/main.rs` counts on the dispatch side"
    );

    for (name, call) in &read_only {
        let before = crate::runner::host::containment_establishments();
        call();
        assert_eq!(
            crate::runner::host::containment_establishments(),
            before,
            "`{name}` is not a write coordinator and established containment anyway"
        );
    }
}

#[test]
fn a_facade_run_refuses_before_any_effect_when_containment_fails() {
    let (_tree, repo) = temp_engine_repo("containment-order");
    let mut opts = options(&repo);
    opts.plan_path = repo.join("absent-plan.md");
    let source = source(vec![Effect::EditFile], vec![ReviewBehavior::Pass]);
    let harness = Harness::new(&source);
    let runner = RecordingRunner::new();

    let refused = run_contained(&opts, &harness, &runner, || {
        Err(UpstrokeError::Refused {
            message: "the ambient Job Object could not be established (simulated failure)"
                .to_owned(),
        })
    })
    .expect_err("a run whose ambient job cannot be established must refuse");
    let refused = refused.to_string();
    assert!(
        refused.contains("ambient Job Object"),
        "the refusal must diagnose the ambient job: {refused}"
    );
    assert!(
        !refused.contains("absent-plan"),
        "the coordinator ran before containment: {refused}"
    );
    assert!(
        runner.seen().is_empty(),
        "a run refused at startup spawned a process: {:?}",
        runner.seen()
    );

    let reached = run_contained(&opts, &harness, &runner, || {
        crate::runner::host::contain_write_command(&mut crate::agent::proc::NoHooks)
    })
    .expect_err("the coordinator then fails on its own, on the plan");
    let reached = reached.to_string();
    assert!(
        reached.contains("absent-plan"),
        "with containment established the coordinator must run: {reached}"
    );
    assert!(
        !reached.contains("ambient Job Object"),
        "a successful establishment must not be reported as a refusal: {reached}"
    );
}

#[test]
fn a_facade_resume_refuses_before_any_effect_when_containment_fails() {
    let (_tree, repo) = temp_engine_repo("containment-order-resume");
    let mut opts = ResumeOptions::new("01ABSENTRUN".to_owned(), repo.clone());
    opts.pools_path = Some(no_pools());
    opts.private_root = Some(private_root_for(&repo));
    let source = source(vec![Effect::EditFile], vec![ReviewBehavior::Pass]);
    let harness = Harness::new(&source);
    let runner = RecordingRunner::new();

    let refused = resume_contained(&opts, &harness, &runner, || {
        Err(UpstrokeError::Refused {
            message: "the ambient Job Object could not be established (simulated failure)"
                .to_owned(),
        })
    })
    .expect_err("a resume whose ambient job cannot be established must refuse");

    let looked_in = repo.display().to_string();
    let refused = refused.to_string();
    assert!(
        refused.contains("ambient Job Object"),
        "the refusal must diagnose the ambient job: {refused}"
    );
    assert!(
        !refused.contains(&looked_in),
        "the coordinator ran before containment: {refused}"
    );
    assert!(
        runner.seen().is_empty(),
        "a resume refused at startup spawned a process: {:?}",
        runner.seen()
    );

    let reached = resume_contained(&opts, &harness, &runner, || {
        crate::runner::host::contain_write_command(&mut crate::agent::proc::NoHooks)
    })
    .expect_err("the coordinator then fails on its own, on the run it cannot find");
    let reached = reached.to_string();
    assert!(
        reached.contains(&looked_in),
        "with containment established the coordinator must run: {reached}"
    );
    assert!(
        !reached.contains("ambient Job Object"),
        "a successful establishment must not be reported as a refusal: {reached}"
    );
}
