//! Extended notes: `docs/internals/engine/tests/legacy_resume.md`
// Allowlist placement: the funnel section of `effects/allowlist.toml`.
#![allow(clippy::disallowed_methods, clippy::disallowed_types)]
#![forbid(clippy::disallowed_macros)]

use super::*;

#[test]
fn a_resume_keeps_the_reviewers_the_run_started_with() {
    let (_tree, repo) = temp_engine_repo("resumereviewers");
    seed(
        &repo,
        "## Rotate the signing key\n\
             <!-- upstroke: id=t1 kind=implement depends= tier=frontier -->\n",
        Some(
            "[interaction]\nmode = \"never\"\n\n\
                 [routing]\nimplement = { chain = [\"frontier\"], attempts_per = 1 }\n",
        ),
    );

    let source = source(vec![Effect::NoEdit], vec![ReviewBehavior::Pass]);
    let first = run_with(&cross_vendor_opts(&repo), &source).expect("run");
    assert!(
        matches!(task(&first, "t1").status, TaskRunStatus::Parked { .. }),
        "{first:?}"
    );

    let paths = paths_of(&repo, &first.run_id);
    let recorded = {
        let mut warnings = Vec::new();
        let events = events::read_all(&paths.events(), &mut warnings).expect("log");
        events::started_of(&events, &paths.events())
            .expect("run_started")
            .reviews
            .clone()
    };
    let recorded = recorded.expect("step 9 records who reviews");
    assert_eq!(
        recorded.alternative, None,
        "there was nothing to rebind to when this run started"
    );
    assert_eq!(recorded.pass_timeout_secs, Some(5400));

    fs::write(
        repo.join("upstroke.toml"),
        "[interaction]\nmode = \"never\"\n\n\
             [routing]\nimplement = { chain = [\"frontier\"], attempts_per = 1 }\n\
             review = { timeout_secs = 60 }\n",
    )
    .expect("edit only the future review timeout");

    crate::answer::answer(
        &repo,
        &first.questions[0].question.id.to_string(),
        crate::answer::Reply::Text("put the key in src/auth/keys.rs".to_owned()),
    )
    .expect("answer");

    let later = cross_vendor(
        vec![Effect::EditFile],
        vec![ReviewBehavior::Pass],
        vec![ReviewBehavior::Pass],
    );
    let resumed =
        resume_with(&resume_options(&repo, &first.run_id), &later).expect("resume continues");

    assert!(committed(&resumed, "t1"), "{resumed:?}");
    assert_eq!(
        task(&resumed, "t1").review_models,
        ["claude-opus-5"],
        "the recorded reviewer judged the resumed attempt"
    );
    assert_eq!(
        later.copilot().reviews_run(),
        0,
        "a CLI installed since the run began must not become its judge"
    );
    let warning = resumed
        .warnings
        .iter()
        .find(|warning| warning.contains("review pass timeout"))
        .unwrap_or_else(|| panic!("no timeout-difference warning: {:?}", resumed.warnings));
    assert!(warning.contains("60s"), "{warning}");
    assert!(warning.contains("5400s"), "{warning}");
    assert!(warning.contains("Start a new run"), "{warning}");
}

#[test]
fn resume_runs_with_the_effort_policy_the_run_recorded_not_todays_config() {
    let original = "[interaction]\nmode = \"never\"\n\n\
                        [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n\n\
                        [routing.effort]\nimplementation = \"xhigh\"\nreview = \"max\"\n";
    let (_tree, repo, run_id) = parked_run_with_config("resumeeffort", original);
    fs::write(
        repo.join("upstroke.toml"),
        "[interaction]\nmode = \"never\"\n\n\
             [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n\n\
             [routing.effort]\nimplementation = \"low\"\nreview = \"high\"\n",
    )
    .expect("edit effort only");

    let resumed = resume_answering(&repo, &run_id, Effect::EditFile);
    assert_eq!(resumed.outcome(), RunOutcome::Complete, "{resumed:?}");
    let logged = events_of(&repo, &run_id);
    let resumed_worker = logged
        .iter()
        .filter_map(|event| match &event.body {
            EventBody::AttemptStarted { data, .. } => Some(data),
            _ => None,
        })
        .next_back()
        .expect("resumed worker start");
    assert_eq!(resumed_worker.effort, Some(Effort::XHigh));
    let resumed_reviews = logged
        .iter()
        .filter_map(|event| match &event.body {
            EventBody::AttemptFinished { data, .. } if !data.reviews.is_empty() => {
                Some(&data.reviews)
            }
            _ => None,
        })
        .next_back()
        .expect("resumed review records");
    assert!(
        resumed_reviews
            .iter()
            .all(|review| review.effort == Some(Effort::Max)),
        "every review pass keeps max: {resumed_reviews:?}"
    );
    let warning = resumed
        .warnings
        .iter()
        .find(|warning| warning.contains("today's effort policy"))
        .unwrap_or_else(|| panic!("no effort difference warning: {:?}", resumed.warnings));
    assert!(warning.contains("implementation small=low"), "{warning}");
    assert!(warning.contains("implementation small=xhigh"), "{warning}");
    assert!(warning.contains("review=max"), "{warning}");
    assert!(warning.contains("Start a new run"), "{warning}");
}

#[test]
fn resume_restores_the_recorded_worker_binding_before_preflight() {
    let original = "[interaction]\nmode = \"never\"\n\n\
                        [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n";
    let (_tree, repo, run_id) = parked_run_with_config("resumebinding", original);
    fs::write(
        repo.join("upstroke.toml"),
        "[interaction]\nmode = \"never\"\n\n\
             [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n\n\
             [[pins]]\ntier = \"small\"\nagent = \"copilot\"\nmodel = \"gpt-5-mini\"\n",
    )
    .expect("edit only the binding");

    let resumed = resume_answering(&repo, &run_id, Effect::EditFile);
    assert_eq!(resumed.outcome(), RunOutcome::Complete, "{resumed:?}");
    let logged = events_of(&repo, &run_id);
    let worker = logged
        .iter()
        .filter_map(|event| match &event.body {
            EventBody::AttemptStarted { data, .. } => Some(data),
            _ => None,
        })
        .next_back()
        .expect("resumed worker");
    assert_eq!(worker.agent, "claude-code");
    assert_eq!(worker.model, "claude-haiku-4-5");
    assert_eq!(
        worker.selection_origin,
        Some(events::SelectionOrigin::Auto),
        "the recorded absence of a pin is part of the snapshot too"
    );
    assert!(
        resumed
            .warnings
            .iter()
            .any(|warning| warning.contains("today's worker bindings")
                && warning.contains("gpt-5-mini")
                && warning.contains("claude-haiku-4-5")),
        "binding difference warning: {:?}",
        resumed.warnings
    );
}

#[test]
fn the_resume_that_rederives_an_old_logs_effort_records_it_for_the_next_one() {
    let original = "[interaction]\nmode = \"never\"\n\n\
                        [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n\n\
                        [routing.effort]\nimplementation = \"xhigh\"\nreview = \"max\"\n";
    let (_tree, repo, run_id) = parked_run_with_config("oldlogeffort", original);
    let paths = paths_of(&repo, &run_id);
    rewrite_run_started_as_schema_one(&paths, &["effort_policy"]);

    fs::write(
        repo.join("upstroke.toml"),
        "[interaction]\nmode = \"never\"\n\n\
             [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n\n\
             [routing.effort]\nimplementation = \"high\"\nreview = \"xhigh\"\n",
    )
    .expect("first derived policy");
    let first = resume_answering(&repo, &run_id, Effect::NoEdit);
    assert_eq!(first.outcome(), RunOutcome::Parked, "{first:?}");
    assert!(
        first
            .warnings
            .iter()
            .any(|warning| warning.contains("predates the effort-policy record")),
        "legacy warning: {:?}",
        first.warnings
    );
    let established = ResolvedEffortPolicy {
        small: Effort::High,
        mid: Effort::High,
        frontier: Effort::High,
        review: Effort::XHigh,
    };
    assert_eq!(
        events::recorded_effort_policy(&events_of(&repo, &run_id)),
        Some(established),
        "the first resume writes down what it derived"
    );
    let after_first = events_of(&repo, &run_id);
    assert!(events::recorded_chains(&after_first).is_some());
    assert_eq!(
        after_first
            .iter()
            .filter(|event| matches!(event.body, EventBody::RunSchemaUpgraded { .. }))
            .count(),
        1,
        "the first current-binary resume appends one downgrade barrier"
    );

    fs::write(
        repo.join("upstroke.toml"),
        "[interaction]\nmode = \"never\"\n\n\
             [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n\n\
             [routing.effort]\nimplementation = \"low\"\nreview = \"medium\"\n",
    )
    .expect("later policy");
    let second = resume_answering(&repo, &run_id, Effect::EditFile);
    assert_eq!(second.outcome(), RunOutcome::Complete, "{second:?}");
    let logged = events_of(&repo, &run_id);
    let worker = logged
        .iter()
        .filter_map(|event| match &event.body {
            EventBody::AttemptStarted { data, .. } => Some(data),
            _ => None,
        })
        .next_back()
        .expect("second resumed worker");
    assert_eq!(worker.effort, Some(Effort::High));
    let reviews = logged
        .iter()
        .filter_map(|event| match &event.body {
            EventBody::AttemptFinished { data, .. } if !data.reviews.is_empty() => {
                Some(&data.reviews)
            }
            _ => None,
        })
        .next_back()
        .expect("second resumed reviews");
    assert!(
        reviews
            .iter()
            .all(|review| review.effort == Some(Effort::XHigh)),
        "reviews retain the established legacy policy: {reviews:?}"
    );
    assert!(
        second
            .warnings
            .iter()
            .any(|warning| warning.contains("today's effort policy")),
        "the later edit is reported: {:?}",
        second.warnings
    );
    assert!(
        !second
            .warnings
            .iter()
            .any(|warning| warning.contains("predates the effort-policy record")),
        "the legacy absence was established once: {:?}",
        second.warnings
    );
    assert_eq!(
        events_of(&repo, &run_id)
            .iter()
            .filter(|event| matches!(event.body, EventBody::RunSchemaUpgraded { .. }))
            .count(),
        1,
        "later resumes must not append duplicate schema transitions"
    );
}

#[test]
fn the_resume_that_rederives_an_old_review_plan_records_it_for_the_next_one() {
    let original = "[interaction]\nmode = \"never\"\n\n\
                        [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n";
    let (_tree, repo, run_id) = parked_run_with_config("oldlogreviews", original);
    let paths = paths_of(&repo, &run_id);
    rewrite_run_started_as_schema_two(&paths);
    strip_run_started_field(&paths, "reviews");

    fs::write(
        repo.join("upstroke.toml"),
        "[interaction]\nmode = \"never\"\n\n\
             [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n\
             review = { timeout_secs = 60 }\n",
    )
    .expect("first derived review plan");
    let first = resume_answering(&repo, &run_id, Effect::NoEdit);
    assert_eq!(first.outcome(), RunOutcome::Parked, "{first:?}");
    assert!(
        first
            .warnings
            .iter()
            .any(|warning| warning.contains("predates the review record")),
        "legacy warning: {:?}",
        first.warnings
    );
    let established = events::recorded_reviews(&events_of(&repo, &run_id))
        .cloned()
        .expect("the first resume writes down what it derived");
    assert_eq!(established.pass_timeout_secs, Some(60));

    fs::write(
        repo.join("upstroke.toml"),
        "[interaction]\nmode = \"never\"\n\n\
             [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n\
             review = { timeout_secs = 120 }\n",
    )
    .expect("later review plan");
    let second = resume_answering(&repo, &run_id, Effect::EditFile);
    assert_eq!(second.outcome(), RunOutcome::Complete, "{second:?}");
    assert_eq!(
        events::recorded_reviews(&events_of(&repo, &run_id))
            .expect("record survives")
            .pass_timeout_secs,
        Some(60),
        "a later config edit cannot replace the established plan"
    );
    let warning = second
        .warnings
        .iter()
        .find(|warning| warning.contains("today's review pass timeout"))
        .unwrap_or_else(|| panic!("no timeout drift warning: {:?}", second.warnings));
    assert!(warning.contains("120s"), "{warning}");
    assert!(warning.contains("60s"), "{warning}");
    assert!(
        !second
            .warnings
            .iter()
            .any(|warning| warning.contains("predates the review record")),
        "the legacy absence is established exactly once: {:?}",
        second.warnings
    );
}

#[test]
fn a_schema_two_resume_records_the_complete_review_barrier_before_work() {
    let (_tree, repo, run_id) = parked_run("schema2reviewbarrier");
    let paths = paths_of(&repo, &run_id);
    rewrite_run_started_as_schema_two(&paths);
    fs::write(
        repo.join("upstroke.toml"),
        "[interaction]\nmode = \"never\"\n\n\
             [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n\
             review = { timeout_secs = 47 }\n",
    )
    .expect("first explicit complete-review timeout");

    let resumed = resume_answering(&repo, &run_id, Effect::NoEdit);
    assert_eq!(resumed.outcome(), RunOutcome::Parked, "{resumed:?}");

    let logged = events_of(&repo, &run_id);
    let barrier = logged
        .iter()
        .position(|event| {
            matches!(
                &event.body,
                EventBody::RunSchemaUpgraded { data }
                    if data.from == 2 && data.to == events::SCHEMA_VERSION
            )
        })
        .expect("schema 2 -> 3 downgrade barrier");
    let resumed_attempt = logged
        .iter()
        .enumerate()
        .skip(barrier + 1)
        .find(|(_, event)| matches!(event.body, EventBody::AttemptStarted { .. }))
        .map(|(index, _)| index)
        .expect("resumed attempt after the barrier");
    assert!(
        barrier < resumed_attempt,
        "the old verification contract must be fenced off before work starts"
    );
    let upgraded_reviews = events::recorded_complete_reviews(&logged)
        .expect("schema-3 resume records a complete review plan");
    assert_eq!(upgraded_reviews.pass_timeout_secs, Some(47));
    assert_eq!(upgraded_reviews.enabled, Some(true));
    assert_eq!(
        upgraded_reviews.alternative_available,
        Some(upgraded_reviews.alternative.is_some())
    );
    assert_eq!(upgraded_reviews.second_opinion.len(), 1);

    fs::write(
        repo.join("upstroke.toml"),
        "[interaction]\nmode = \"never\"\n\n\
             [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n\
             review = { timeout_secs = 83 }\n",
    )
    .expect("later configured timeout");
    let second = resume_answering(&repo, &run_id, Effect::EditFile);
    assert_eq!(second.outcome(), RunOutcome::Complete, "{second:?}");
    assert_eq!(
        events::recorded_reviews(&events_of(&repo, &run_id))
            .expect("upgraded review plan survives")
            .pass_timeout_secs,
        Some(47),
        "a later binary/config default cannot reinterpret the upgraded timeout"
    );
    assert!(
        second.warnings.iter().any(|warning| {
            warning.contains("today's review pass timeout")
                && warning.contains("83s")
                && warning.contains("47s")
        }),
        "timeout drift warning: {:?}",
        second.warnings
    );
}

#[test]
fn a_refused_ceiling_beats_both_locks_on_resume() {
    let (_tree, repo, run_id) = parked_run("resumeceilingbeforelocks");
    let paths = paths_of(&repo, &run_id);
    rewrite_run_started_as_schema_two(&paths);
    fs::write(
        repo.join("upstroke.toml"),
        format!("{PARKED_RUN_CONFIG}\n[engine]\nmax_per_agent = 0\n"),
    )
    .expect("today's config");

    let lease = worktree_lock_path(&repo);
    let run_lock = rundir::lock_file(&paths.public);
    for path in [&lease, &run_lock] {
        fs::remove_file(path).unwrap_or_else(|error| {
            panic!("the fixture run left {}: {error}", path.display());
        });
    }

    let refused = resume_err(&repo, &run_id);
    assert!(refused.contains("max_per_agent"), "{refused}");
    assert!(
        !lease.exists(),
        "the worktree lease was taken before the config was read"
    );
    assert!(
        !run_lock.exists(),
        "the run lock was taken before the config was read"
    );

    let competitor = WorktreeLock::acquire(&repo).expect("a competing holder takes the lease");
    let contended = resume_err(&repo, &run_id);
    assert!(
        contended.contains("max_per_agent"),
        "the config error must win the race it never needed to enter: {contended}"
    );
    assert!(
        !contended.contains("another upstroke process"),
        "lock contention must not be the diagnosis for a config error: {contended}"
    );
    drop(competitor);
    assert!(
        !run_lock.exists(),
        "and the run lock stays untaken either way"
    );
}

fn volatile_strings(repo: &Path, run_id: &str) -> Vec<String> {
    let mut volatile = vec![run_id.to_owned()];

    for path in [private_root_for(repo), repo.to_path_buf()] {
        let text = path.to_string_lossy().into_owned();
        volatile.push(text.replace('\\', "/"));
        volatile.push(text);
    }
    volatile
}

fn replace_exact_runs(
    text: &str,
    len: usize,
    token: &str,
    member: impl Fn(char) -> bool,
) -> String {
    let mut out = String::with_capacity(text.len());
    let mut run = String::new();
    let flush = |run: &mut String, out: &mut String| {
        if run.chars().count() == len {
            out.push_str(token);
        } else {
            out.push_str(run);
        }
        run.clear();
    };
    for ch in text.chars() {
        if member(ch) {
            run.push(ch);
            continue;
        }
        flush(&mut run, &mut out);
        out.push(ch);
    }
    flush(&mut run, &mut out);
    out
}

fn canonicalize_json(value: &mut serde_json::Value, volatile: &[String]) {
    match value {
        serde_json::Value::String(text) => {
            let mut canonical = text.clone();
            for needle in volatile {
                canonical = canonical.replace(needle.as_str(), "<volatile>");
            }
            canonical = replace_exact_runs(&canonical, 40, "<sha>", |ch| {
                ch.is_ascii_digit() || ch.is_ascii_lowercase() && ch.is_ascii_hexdigit()
            });
            *text = replace_exact_runs(&canonical, 26, "<ulid>", |ch| {
                ch.is_ascii_digit() || ch.is_ascii_uppercase()
            });
        }
        serde_json::Value::Array(items) => {
            for item in items {
                canonicalize_json(item, volatile);
            }
        }
        serde_json::Value::Object(fields) => {
            for (key, field) in fields.iter_mut() {
                if matches!(key.as_str(), "ts" | "duration_ms" | "duration") {
                    *field = serde_json::Value::String(format!("<{key}>"));
                    continue;
                }
                canonicalize_json(field, volatile);
            }
        }
        _ => {}
    }
}

fn canonical_trace(events: &[events::Event], repo: &Path, run_id: &str) -> Vec<String> {
    let volatile = volatile_strings(repo, run_id);
    events
        .iter()
        .map(|event| {
            let mut value = serde_json::to_value(event).expect("an event serializes");
            canonicalize_json(&mut value, &volatile);
            value.to_string()
        })
        .collect()
}

fn canonical_projection(report: &RunReport, repo: &Path, run_id: &str) -> String {
    let volatile = volatile_strings(repo, run_id);
    let mut value = serde_json::to_value(report).expect("a report serializes");
    value
        .as_object_mut()
        .expect("a report is an object")
        .remove("warnings")
        .expect("a report records its warnings");
    canonicalize_json(&mut value, &volatile);
    value.to_string()
}

const LEGACY_RESUME_LIMITS: &str = "\n[engine]\nmax_parallel = 2\nmax_merge_repairs = 7\n\
                                    max_per_agent = 4\nmax_per_pool = 5\n";

const LEGACY_RESUME_NO_LIMITS: &str = "\n# no [engine] ceilings in this arm\n";

#[derive(Clone, Copy)]
enum LegacyFixture {
    Parked,

    InterruptedAttempt,
}

struct LegacyArm {
    report: RunReport,

    trace: Vec<String>,

    projection: String,

    tree: String,
    events: Vec<events::Event>,
}

fn legacy_resume_pair(tag: &str, fixture: LegacyFixture) -> Vec<LegacyArm> {
    let mut observed = Vec::new();
    for (arm, extra) in [
        ("control", LEGACY_RESUME_NO_LIMITS),
        ("limits", LEGACY_RESUME_LIMITS),
    ] {
        let (_tree, repo, run_id) = parked_run(&format!("legacylimits-{tag}-{arm}"));
        let paths = paths_of(&repo, &run_id);
        rewrite_run_started_as_schema_two(&paths);
        if matches!(fixture, LegacyFixture::InterruptedAttempt) {
            truncate_log_after(&paths, "attempt_started");
        }
        fs::write(
            repo.join("upstroke.toml"),
            format!("{PARKED_RUN_CONFIG}{extra}"),
        )
        .expect("today's config");

        let report = resume_answering(&repo, &run_id, Effect::EditFile);
        let events = events_of(&repo, &run_id);
        observed.push(LegacyArm {
            trace: canonical_trace(&events, &repo, &run_id),
            projection: canonical_projection(&report, &repo, &run_id),
            tree: git_in(&repo, &["rev-parse", "HEAD^{tree}"]),
            events,
            report,
        });
    }
    observed
}

#[test]
fn a_legacy_resume_is_not_reinterpreted_by_the_new_engine_limits() {
    for (fixture, tag) in [
        (LegacyFixture::Parked, "parked"),
        (LegacyFixture::InterruptedAttempt, "interrupted"),
    ] {
        let observed = legacy_resume_pair(tag, fixture);
        let control = &observed[0];
        let limits = &observed[1];

        assert_eq!(
            control.report.outcome(),
            RunOutcome::Complete,
            "the {tag} control resume continues to the end: {:?}",
            control.report
        );
        assert_eq!(
            limits.report.outcome(),
            control.report.outcome(),
            "the new keys must not change how a legacy run ends ({tag})"
        );
        assert_eq!(
            limits.trace, control.trace,
            "nor what it records, nor with what contents, nor in what order ({tag})"
        );
        assert_eq!(
            limits.projection, control.projection,
            "nor what it reports about each task ({tag})"
        );
        assert_eq!(
            limits.tree, control.tree,
            "nor the tree it committed ({tag})"
        );
        assert!(
            !control.tree.is_empty(),
            "the fixture must actually have committed something for that to mean anything"
        );

        let mut open = 0i32;
        let mut peak = 0i32;
        for event in &limits.events {
            match &event.body {
                EventBody::AttemptStarted { .. } => open += 1,

                EventBody::AttemptFinished { .. } | EventBody::AttemptInterrupted { .. } => {
                    open -= 1;
                }
                _ => continue,
            }
            peak = peak.max(open);
        }
        assert_eq!(peak, 1, "the resume ran one attempt at a time ({tag})");
        assert_eq!(open, 0, "and settled every one of them ({tag})");

        for key in [
            "max_parallel",
            "max_merge_repairs",
            "max_per_agent",
            "max_per_pool",
        ] {
            assert!(
                limits
                    .report
                    .warnings
                    .iter()
                    .any(|warning| warning.contains(key) && warning.contains("not acted on")),
                "`{key}` must be reported as unacted-on ({tag}): {:?}",
                limits.report.warnings
            );
            assert!(
                !control
                    .report
                    .warnings
                    .iter()
                    .any(|warning| warning.contains(key)),
                "and only when it was written ({tag}): {:?}",
                control.report.warnings
            );
        }
        assert!(
            limits.report.warnings.iter().any(|warning| {
                warning.contains("max_parallel = 2") && warning.contains("this resume")
            }),
            "and the refused-for-fresh-runs ceiling says which run it is talking about: {:?}",
            limits.report.warnings
        );
    }
}

#[test]
fn schema_two_review_markers_upgrade_independently_of_timeout() {
    let (_tree, repo, run_id) = parked_run("schema2reviewmarkers");
    let paths = paths_of(&repo, &run_id);
    let recorded_timeout = events::recorded_reviews(&events_of(&repo, &run_id))
        .and_then(|plan| plan.pass_timeout_secs)
        .expect("current run records a timeout");
    rewrite_run_started_as_schema_two_missing_review_fields(
        &paths,
        &["enabled", "alternative_available"],
    );

    let first = resume_answering(&repo, &run_id, Effect::NoEdit);
    assert_eq!(first.outcome(), RunOutcome::Parked, "{first:?}");
    assert!(
        first
            .warnings
            .iter()
            .any(|warning| warning.contains("explicit reviewer-identity markers")),
        "marker-upgrade warning: {:?}",
        first.warnings
    );
    let upgraded = events::recorded_complete_reviews(&events_of(&repo, &run_id))
        .cloned()
        .expect("schema-3 resume records the complete identity");
    assert_eq!(upgraded.pass_timeout_secs, Some(recorded_timeout));
    assert_eq!(upgraded.enabled, Some(upgraded.primary.is_some()));
    assert_eq!(
        upgraded.alternative_available,
        Some(upgraded.alternative.is_some())
    );

    let second = resume_answering(&repo, &run_id, Effect::EditFile);
    assert_eq!(second.outcome(), RunOutcome::Complete, "{second:?}");
    assert_eq!(
        events::recorded_complete_reviews(&events_of(&repo, &run_id)),
        Some(&upgraded),
        "the next replay accepts and preserves the explicit markers"
    );
}

#[test]
fn schema_two_inconsistent_review_identity_is_refused_before_upgrade_and_spend() {
    let (_tree, repo, run_id) = parked_run("schema2badreviewidentity");
    let paths = paths_of(&repo, &run_id);
    let text = fs::read_to_string(paths.events()).expect("log");
    let mut rewritten = false;
    let lines: Vec<String> = text
        .lines()
        .map(|line| {
            let mut value: serde_json::Value = serde_json::from_str(line).expect("event json");
            if value.get("event").and_then(serde_json::Value::as_str) == Some("run_started") {
                let data = value
                    .get_mut("data")
                    .and_then(serde_json::Value::as_object_mut)
                    .expect("run_started data");
                data.insert("schema".to_owned(), serde_json::Value::from(2));
                let reviews = data
                    .get_mut("reviews")
                    .and_then(serde_json::Value::as_object_mut)
                    .expect("review plan");
                reviews.insert("enabled".to_owned(), serde_json::Value::Bool(true));
                reviews.insert("primary".to_owned(), serde_json::Value::Null);
                rewritten = true;
            }
            value.to_string()
        })
        .collect();
    assert!(rewritten);
    fs::write(paths.events(), format!("{}\n", lines.join("\n"))).expect("rewrite");

    let question = events_of(&repo, &run_id)
        .iter()
        .find_map(|event| match &event.body {
            EventBody::QuestionRaised { data, .. } => Some(data.question.id.to_string()),
            EventBody::AttemptFinished {
                parking: Some(parking),
                ..
            } => Some(parking.question.id.to_string()),
            _ => None,
        })
        .expect("parked question");
    crate::answer::answer(
        &repo,
        &question,
        crate::answer::Reply::Text("continue".to_owned()),
    )
    .expect("answer");
    let source = fake(Effect::EditFile);
    let error = resume_harness_inner(
        &resume_options(&repo, &run_id),
        &Harness {
            adapters: &source,
            answers: None,
            sleeper: None,
        },
    )
    .expect_err("an inconsistent inherited review identity must fail closed");
    assert!(
        error
            .to_string()
            .contains("reviews.enabled does not match the recorded primary reviewer"),
        "wrong error: {error}"
    );
    assert!(
        source.adapter.runs().is_empty(),
        "no worker may run under the malformed identity"
    );
    assert!(
        !events_of(&repo, &run_id)
            .iter()
            .any(|event| matches!(event.body, EventBody::RunSchemaUpgraded { .. })),
        "the malformed identity must not be blessed by a schema upgrade"
    );
}

#[test]
fn a_resume_whose_effort_policy_did_not_move_says_nothing_about_it() {
    let config = "[interaction]\nmode = \"never\"\n\n\
                      [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n\n\
                      [routing.effort]\nimplementation = \"xhigh\"\nreview = \"max\"\n";
    let (_tree, repo, run_id) = parked_run_with_config("effortunmoved", config);
    let resumed = resume_answering(&repo, &run_id, Effect::EditFile);
    assert_eq!(resumed.outcome(), RunOutcome::Complete, "{resumed:?}");
    assert!(
        !resumed
            .warnings
            .iter()
            .any(|warning| warning.contains("effort policy") || warning.contains("effort-policy")),
        "an unchanged policy must be silent: {:?}",
        resumed.warnings
    );
}

#[test]
fn a_log_written_before_step_9_still_gets_reviewed_on_resume() {
    let (_tree, repo) = temp_engine_repo("oldlogresume");
    seed(
        &repo,
        "## Rotate the signing key\n\
             <!-- upstroke: id=t1 kind=implement depends= tier=frontier -->\n",
        Some(
            "[interaction]\nmode = \"never\"\n\n\
                 [routing]\nimplement = { chain = [\"frontier\"], attempts_per = 1 }\n",
        ),
    );
    let first_source = source(vec![Effect::NoEdit], vec![ReviewBehavior::Pass]);
    let first = run_with(&cross_vendor_opts(&repo), &first_source).expect("run");

    let paths = paths_of(&repo, &first.run_id);
    rewrite_run_started_as_schema_two(&paths);
    strip_run_started_field(&paths, "reviews");

    crate::answer::answer(
        &repo,
        &first.questions[0].question.id.to_string(),
        crate::answer::Reply::Text("put the key in src/auth/keys.rs".to_owned()),
    )
    .expect("answer");

    let later = source(vec![Effect::EditFile], vec![ReviewBehavior::Fail]);
    let resumed =
        resume_with(&resume_options(&repo, &first.run_id), &later).expect("resume continues");
    assert!(
        !committed(&resumed, "t1"),
        "an older log must not silently switch review off: {resumed:?}"
    );
}

#[test]
fn unreported_cost_stays_unreported_rather_than_zero() {
    assert_eq!(sum_opt([None, None].into_iter()), None);
    assert_eq!(
        sum_opt([Some(0.01), None, Some(0.02)].into_iter()),
        Some(0.03)
    );
}

struct Scenario {
    name: &'static str,
    config: &'static str,

    plan: Option<&'static str>,
    effects: Vec<Effect>,
    reviews: Vec<ReviewBehavior>,

    second_opinion: Option<Vec<ReviewBehavior>>,
    answers: Vec<Answer>,
}

impl Scenario {
    fn new(name: &'static str, config: &'static str, effects: Vec<Effect>) -> Self {
        Self {
            name,
            config,
            plan: None,
            effects,
            reviews: vec![ReviewBehavior::Pass],
            second_opinion: None,
            answers: Vec::new(),
        }
    }

    fn reviewed(mut self, reviews: Vec<ReviewBehavior>) -> Self {
        self.reviews = reviews;
        self
    }

    fn cross_vendor(mut self, plan: &'static str, second: Vec<ReviewBehavior>) -> Self {
        self.plan = Some(plan);
        self.second_opinion = Some(second);
        self
    }

    fn answered(mut self, answers: Vec<Answer>) -> Self {
        self.answers = answers;
        self
    }
}

#[test]
fn live_state_equals_replayed_state_across_every_ladder_path() {
    let scenarios = vec![
        Scenario::new(
            "commit",
            "[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n",
            vec![Effect::EditFile],
        ),
        Scenario::new(
            "retry",
            "[routing]\nimplement = { chain = [\"small\"], attempts_per = 2 }\n",
            vec![Effect::NoEdit, Effect::EditFile],
        ),
        Scenario::new(
            "escalate",
            "[routing]\nimplement = { chain = [\"small\", \"mid\"], attempts_per = 1 }\n",
            vec![Effect::NoEdit, Effect::EditFile],
        ),
        Scenario::new(
            "defer",
            "[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n",
            vec![Effect::RateLimited, Effect::EditFile],
        ),
        Scenario::new(
            "park-then-answer",
            "[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n",
            vec![Effect::NoEdit, Effect::EditFile],
        )
        .answered(vec![Answer::Answered {
            text: "the widget lives in src/widget.rs".to_owned(),
        }]),
        Scenario::new(
            "decline-and-halt",
            "[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n",
            vec![Effect::NoEdit],
        )
        .answered(vec![Answer::Declined]),
        Scenario::new(
            "reviewer-asks-for-a-human",
            "[routing]\nimplement = { chain = [\"small\", \"mid\"], attempts_per = 2 }\n",
            vec![Effect::EditFile],
        )
        .reviewed(vec![ReviewBehavior::NeedsHuman]),
        Scenario::new(
            "second-opinion-passes",
            SECOND_OPINION_CONFIG,
            vec![Effect::EditFile],
        )
        .cross_vendor(FRONTIER_AUTH_PLAN, vec![ReviewBehavior::Pass]),
        Scenario::new(
            "second-opinion-rejects",
            SECOND_OPINION_CONFIG,
            vec![Effect::EditFile],
        )
        .cross_vendor(FRONTIER_AUTH_PLAN, vec![ReviewBehavior::Fail])
        .answered(vec![Answer::Declined]),
        Scenario::new(
            "self-review-rebind",
            FRONTIER_ONLY_CONFIG,
            vec![Effect::EditFile],
        )
        .cross_vendor(FRONTIER_AUTH_PLAN, vec![ReviewBehavior::Pass]),
        Scenario::new(
            "budget-stop",
            "[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n\n\
                 [budgets]\nrun_usd = 0.05\n",
            vec![Effect::EditFile],
        ),
        Scenario::new(
            "approve-spend",
            "[routing]\nimplement = { chain = [\"mid\", \"frontier\"], attempts_per = 1 }\n\n\
                 [interaction]\nask_before = { frontier_escalation_over_usd = 0.005 }\n",
            vec![Effect::NoEdit, Effect::EditFile],
        )
        .answered(vec![Answer::Answered {
            text: "approve: run the escalated attempt".to_owned(),
        }]),
    ];

    for Scenario {
        name,
        config,
        plan,
        effects,
        reviews,
        second_opinion,
        answers,
    } in scenarios
    {
        let (_tree, repo) = temp_engine_repo(&format!("replay-{name}"));
        seed(
            &repo,
            plan.unwrap_or(
                "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n\n\
                     ## Independent\n<!-- upstroke: id=t3 kind=implement depends= -->\n",
            ),
            Some(config),
        );
        let mut opts = options(&repo);
        opts.config_path = Some(repo.join("upstroke.toml"));
        let cross_vendor_scenario = second_opinion.is_some();
        let source = match second_opinion {
            Some(second) => cross_vendor(effects, reviews, second),
            None => source(effects, reviews),
        };
        let scripted = ScriptedAnswers::new(answers);
        let (report, live) = run_harness_inner(
            &opts,
            &Harness {
                adapters: &source,
                answers: Some(&scripted),
                sleeper: None,
            },
        )
        .unwrap_or_else(|e| panic!("{name}: {e}"));

        if cross_vendor_scenario {
            let judged: Vec<&str> = report
                .tasks
                .iter()
                .flat_map(|t| &t.attempts)
                .flat_map(|a| &a.reviews)
                .map(|r| r.agent.as_str())
                .collect();
            assert!(
                judged.contains(&"copilot"),
                "{name}: the second vendor never judged anything, so this scenario \
                     exercises nothing new: {judged:?}"
            );
        }
        assert_live_equals_replay(&repo, &live, &report);
    }
}

#[test]
fn an_aborting_error_still_leaves_a_replayable_log() {
    let (_tree, repo) = temp_engine_repo("abortlog");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));

    let source = source(vec![Effect::EditFile], vec![ReviewBehavior::Pass]);
    opts.attempt_timeout = Duration::from_secs(60);
    let report = run_with(&opts, &source).expect("the run itself succeeds");

    let paths = paths_of(&repo, &report.run_id);
    let text = fs::read_to_string(paths.events()).expect("log");
    let lines: Vec<&str> = text.lines().collect();
    let cut = lines
        .iter()
        .position(|line| line.contains("\"attempt_finished\""))
        .expect("the run recorded an attempt");
    fs::write(paths.events(), format!("{}\n", lines[..cut].join("\n"))).expect("truncate");

    let replayed = replay_of(&repo, &report.run_id);
    assert_eq!(
        legacy_status(&replayed).interrupted,
        1,
        "the dangling attempt is settled"
    );
    assert!(
        legacy_status(&replayed).interrupted_run(),
        "and the run reads as interrupted rather than finished"
    );
    assert_eq!(legacy_status(&replayed).state.states[0], TaskState::Pending);
}

#[test]
fn a_run_that_has_spent_nothing_totals_positive_zero() {
    let nothing: [f64; 0] = [];
    assert!(
        nothing.iter().sum::<f64>().is_sign_negative(),
        "`sum` no longer folds from -0.0, so `total_of` is obsolete"
    );

    assert!(!total_of(&[]).is_sign_negative(), "a spent-nothing total");
    assert_eq!(format!("${:.4}", total_of(&[])), "$0.0000");

    let spent = vec![
        task_report_costing(Some(0.25), Some(1.5)),
        task_report_costing(None, None),
        task_report_costing(Some(0.0), None),
    ];
    assert!((total_of(&spent) - 1.75).abs() < f64::EPSILON);
}

#[test]
fn a_live_run_reads_as_running_rather_than_halted() {
    let (_tree, repo) = temp_engine_repo("livestatus");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n\n\
             ## Waits on the widget\n<!-- upstroke: id=t2 kind=implement depends=t1 -->\n\n\
             ## Independent\n<!-- upstroke: id=t3 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let report = run_with(&opts, &fake(Effect::EditFile)).expect("run");
    let paths = paths_of(&repo, &report.run_id);

    let text = fs::read_to_string(paths.events()).expect("log");
    let lines: Vec<&str> = text.lines().collect();
    let cut = lines
        .iter()
        .position(|line| line.contains("\"attempt_finished\""))
        .expect("an attempt");
    fs::write(paths.events(), format!("{}\n", lines[..cut].join("\n"))).expect("truncate");

    let stopped = replay_of(&repo, &report.run_id);
    assert!(legacy_status(&stopped).interrupted_run());
    let out = crate::status::render(&stopped);
    assert!(out.contains("skipped (run interrupted)"), "{out}");
    assert!(out.contains("t2: blocked by `t1`"), "{out}");

    let lock = RunLock::acquire(&paths.public).expect("simulate a live engine");

    let live = replay_of(&repo, &report.run_id);
    assert!(
        legacy_status(&live).running,
        "a held lock means an engine is driving this"
    );
    assert_eq!(
        legacy_status(&live).interrupted,
        0,
        "an attempt in flight has not been interrupted"
    );
    let out = crate::status::render(&live);
    assert!(out.contains("t1: running now"), "{out}");

    assert!(out.contains("t2: queued"), "{out}");
    assert!(out.contains("t3: queued"), "{out}");
    assert!(out.contains("run in progress"), "{out}");
    for lie in [
        "small failed",
        "skipped (run halted)",
        "skipped (run interrupted)",
        "run complete",
        "run interrupted",
        "blocked by",
    ] {
        assert!(!out.contains(lie), "a live run reported `{lie}`:\n{out}");
    }
    drop(lock);
}

#[test]
fn a_truncated_run_resumes_without_spending_the_interrupted_attempt() {
    let (_tree, repo) = temp_engine_repo("resumetrunc");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = fake(Effect::EditFile);
    let report = run_with(&opts, &source).expect("run");
    let run_id = report.run_id.clone();
    let paths = paths_of(&repo, &run_id);

    let text = fs::read_to_string(paths.events()).expect("log");
    let lines: Vec<&str> = text.lines().collect();
    let cut = lines
        .iter()
        .position(|line| line.contains("\"attempt_finished\""))
        .expect("an attempt");
    fs::write(paths.events(), format!("{}\n", lines[..cut].join("\n"))).expect("truncate");
    git_in(&repo, &["reset", "-q", "--hard", "HEAD~1"]);
    fs::write(repo.join("agent-output.txt"), "half-written\n").expect("residue");

    let source = fake(Effect::EditFile);
    let (resumed, state) = resume_harness_inner(
        &resume_options(&repo, &run_id),
        &Harness {
            adapters: &source,
            answers: None,
            sleeper: None,
        },
    )
    .expect("resume");

    assert_eq!(resumed.outcome(), RunOutcome::Complete, "{resumed:?}");
    assert!(committed(&resumed, "t1"));

    let t1 = task(&resumed, "t1");
    assert_eq!(
        t1.attempts.len(),
        2,
        "the interrupted attempt is on the record beside the one that worked"
    );
    assert_eq!(
        t1.attempts[0].failure.as_ref().map(|f| f.kind),
        Some(FailureKind::Interrupted)
    );
    assert_eq!(
        t1.attempts[0].cost_usd, None,
        "unknown spend is reported as unknown, not as free"
    );
    assert_eq!(t1.attempts[1].tier, "small", "still on the same rung");
    assert!(
        !t1.attempts[1].resumed,
        "§14: the tree was discarded, so the session cannot be trusted"
    );

    assert!(
        git_in(&repo, &["status", "--porcelain"]).trim().is_empty(),
        "crash residue discarded"
    );
    assert_eq!(
        git_in(&repo, &["rev-list", "--count", "main..HEAD"]).trim(),
        "1",
        "one commit, not a duplicate of the interrupted attempt's work"
    );
    assert!(
        resumed
            .warnings
            .iter()
            .any(|w| w.contains("discarded") && w.contains("agent-output.txt")),
        "the operator is told what was thrown away: {:?}",
        resumed.warnings
    );
    assert_live_equals_replay(&repo, &state, &resumed);
}

#[test]
fn killing_a_run_mid_attempt_leaves_a_resumable_record() {
    let (_tree, repo) = temp_engine_repo("crashkill");
    seed(
        &repo,
        "## First\n<!-- upstroke: id=t1 kind=implement depends= -->\n\n\
             ## Second\n<!-- upstroke: id=t2 kind=implement depends=t1 -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );

    let exe = std::env::current_exe().expect("test binary");
    let status = Command::new(exe)
        .args([
            "--exact",
            "engine::tests::crash_child_dies_inside_an_attempt",
            "--ignored",
            "--test-threads",
            "1",
        ])
        .env("UPSTROKE_CRASH_REPO", &repo)
        .output()
        .expect("spawn the child run");
    assert_eq!(
        status.status.code(),
        Some(CRASH_EXIT_CODE),
        "the child must die inside the attempt, not finish or panic: {}",
        String::from_utf8_lossy(&status.stderr)
    );

    let run_id = rundir::latest_run(&repo).expect("the child started a run");
    let paths = paths_of(&repo, &run_id);

    assert!(
        !git_in(&repo, &["status", "--porcelain"]).trim().is_empty(),
        "the dead agent's edits are still in the tree"
    );
    let log = fs::read_to_string(paths.events()).expect("log");
    let last = log.lines().last().expect("events");
    assert!(
        last.contains("\"attempt_started\"") && last.contains("\"t2\""),
        "the log ends mid-attempt: {last}"
    );
    assert!(
        !log.contains("\"run_finished\""),
        "a killed run never records an ending"
    );

    let before = replay_of(&repo, &run_id);
    assert!(
        legacy_status(&before).interrupted_run(),
        "status calls it interrupted"
    );
    assert_eq!(legacy_status(&before).interrupted, 1);
    assert!(
        crate::status::render(&before).contains(&format!("upstroke resume {run_id}")),
        "and tells the operator how to continue it"
    );

    assert!(
        !rundir::is_running(&paths.public),
        "the OS released the lock"
    );

    let rendered = crate::status::render(&before);
    assert!(
        rendered.contains("run interrupted: 1 task(s) committed so far"),
        "{rendered}"
    );
    assert!(
        !rendered.contains("run complete"),
        "a killed run claimed it completed:\n{rendered}"
    );

    assert!(rendered.contains("skipped (run interrupted)"), "{rendered}");

    let source = fake(Effect::EditFile);
    let (resumed, state) = resume_harness_inner(
        &resume_options(&repo, &run_id),
        &Harness {
            adapters: &source,
            answers: None,
            sleeper: None,
        },
    )
    .expect("resume the killed run");

    assert_eq!(resumed.outcome(), RunOutcome::Complete, "{resumed:?}");
    assert!(committed(&resumed, "t1"), "the work it did survived");
    assert!(
        committed(&resumed, "t2"),
        "and the work it died in got done"
    );
    assert_eq!(
        git_in(&repo, &["rev-list", "--count", "main..HEAD"]).trim(),
        "2",
        "one commit per task, with nothing duplicated by the resume"
    );
    let t2 = task(&resumed, "t2");
    assert_eq!(
        t2.attempts[0].failure.as_ref().map(|f| f.kind),
        Some(FailureKind::Interrupted),
        "the attempt it died in is on the record: {t2:?}"
    );
    assert_live_equals_replay(&repo, &state, &resumed);
}

#[test]
fn resume_refuses_schema_two_failed_attempt_without_recorded_decision() {
    let (_tree, repo, run_id) = parked_run("legacyfailedprefix");
    let paths = paths_of(&repo, &run_id);
    rewrite_run_started_as_schema_two(&paths);
    strip_event_field(&paths, "attempt_finished", "parking");
    truncate_log_after(&paths, "attempt_finished");
    let before = fs::read(paths.events()).expect("legacy prefix");

    let error = resume_err(&repo, &run_id);
    assert!(error.contains("failed attempt 1"), "{error}");
    assert!(
        error.contains("without its durable ladder or parking decision"),
        "{error}"
    );
    assert_eq!(
        fs::read(paths.events()).expect("refused log"),
        before,
        "refusal must not upgrade or otherwise mutate the ambiguous prefix"
    );
}

#[test]
fn resume_refuses_when_the_branch_moved_under_it() {
    let (_tree, repo, run_id) = parked_run("headmoved");
    fs::write(repo.join("someone-else.txt"), "a hand-made commit\n").expect("file");
    git_in(&repo, &["add", "-A"]);
    git_in(&repo, &["commit", "-q", "-m", "not the engine's work"]);

    let err = resume_err(&repo, &run_id);
    assert!(err.contains("record ends at"), "got: {err}");
    assert!(
        err.contains("Move the branch back"),
        "and says what to do: {err}"
    );
}

#[test]
fn resume_refuses_when_the_frozen_plan_changed() {
    let (_tree, repo, run_id) = parked_run("planmoved");
    fs::write(
        repo.join("plan.md"),
        "## Doomed\n<!-- upstroke: id=t1 kind=implement depends= -->\nNow with a body.\n",
    )
    .expect("edit the plan");

    let err = resume_err(&repo, &run_id);
    assert!(
        err.contains("has changed since this run froze it"),
        "got: {err}"
    );
    assert!(
        err.contains("attribute work to the wrong tasks"),
        "and why it matters: {err}"
    );
}

#[test]
fn status_export_and_resume_refuse_mutated_normalized_plan_bytes() {
    let (_tree, repo, run_id) = parked_run("normalized-plan-tamper");
    let plan_path = paths_of(&repo, &run_id).plan_json();
    let mut plan: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&plan_path).expect("frozen plan"))
            .expect("valid frozen plan");
    plan["tasks"][0]["title"] =
        serde_json::Value::String("tampered but self-hash unchanged".to_owned());
    fs::write(
        &plan_path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&plan).expect("serialize plan")
        ),
    )
    .expect("replace frozen plan");

    let status_error = match crate::status::load(&repo, Some(&run_id)) {
        Ok(_) => panic!("status must authenticate the exact normalized bytes"),
        Err(error) => error.to_string(),
    };
    assert!(
        status_error.contains("normalized plan digest"),
        "{status_error}"
    );

    let export_error = match crate::export::load(&repo, &run_id) {
        Ok(_) => panic!("export must authenticate the exact normalized bytes"),
        Err(error) => error.to_string(),
    };
    assert!(
        export_error.contains("normalized plan digest"),
        "{export_error}"
    );

    let resume_error = resume_err(&repo, &run_id);
    assert!(
        resume_error.contains("exact bytes") && resume_error.contains("normalized-plan digest"),
        "{resume_error}"
    );
}

#[test]
fn resume_refuses_schema_two_spend_question_without_task_parked() {
    let (_tree, repo, run_id) = parked_run("legacyspendprefix");
    let paths = paths_of(&repo, &run_id);
    rewrite_run_started_as_schema_two(&paths);
    strip_event_field(&paths, "attempt_finished", "parking");
    truncate_log_after(&paths, "attempt_finished");
    let mut warnings = Vec::new();
    let mut log = EventLog::open(EventSite::LegacyOpenLog, &paths.events(), &mut warnings)
        .expect("legacy log");
    log.append(
        EventSite::LegacyAppend,
        EventBody::LadderEscalated {
            task: "t1".to_owned(),
            attempt: 1,
            rung: 0,
            data: events::LadderEscalated {
                to_rung: 1,
                tier: "small".to_owned(),
                summary: "escalate".to_owned(),
                detail: None,
            },
        },
    )
    .expect("legacy escalation");
    log.append(
        EventSite::LegacyAppend,
        EventBody::QuestionRaised {
            task: "t1".to_owned(),
            data: Box::new(events::QuestionRaised {
                question: Question {
                    id: QuestionId::from("q-spend-prefix"),
                    kind: QuestionKind::ApproveSpend,
                    affected_tasks: vec![TaskId::from("t1")],
                    context: "approve spend".to_owned(),
                    options: Vec::new(),
                },
            }),
        },
    )
    .expect("legacy question");
    drop(log);
    let before = fs::read(paths.events()).expect("ambiguous prefix");

    let error = resume_err(&repo, &run_id);
    assert!(error.contains("ApproveSpend"), "{error}");
    assert!(error.contains("before durably parking the task"), "{error}");
    assert_eq!(
        fs::read(paths.events()).expect("refused log"),
        before,
        "refusal never upgrades the spend-approval gap"
    );
}

#[test]
fn legacy_status_still_refuses_a_mismatched_self_reported_plan_hash() {
    let (_tree, repo, run_id) = parked_run("legacy-status-plan-hash");
    let paths = paths_of(&repo, &run_id);
    rewrite_run_started_as_schema_two(&paths);
    let plan_path = paths.plan_json();
    let mut plan: serde_json::Value =
        serde_json::from_slice(&fs::read(&plan_path).expect("frozen plan"))
            .expect("valid frozen plan");
    plan["source"]["hash"] = serde_json::Value::String("different-plan".to_owned());
    fs::write(
        &plan_path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&plan).expect("serialize plan")
        ),
    )
    .expect("replace frozen plan");

    let error = match crate::status::load(&repo, Some(&run_id)) {
        Ok(_) => panic!("legacy status retains its source-hash boundary"),
        Err(error) => error.to_string(),
    };
    assert!(error.contains("frozen plan hash"), "{error}");
    assert!(error.contains("different-plan"), "{error}");
}

#[test]
fn legacy_upgrade_never_blesses_a_modified_normalized_snapshot() {
    let (_tree, repo, run_id) = parked_run("legacy-plan-upgrade-tamper");
    let paths = paths_of(&repo, &run_id);
    rewrite_run_started_as_schema_two(&paths);
    let plan_path = paths.plan_json();
    let mut plan: serde_json::Value =
        serde_json::from_slice(&fs::read(&plan_path).expect("frozen plan"))
            .expect("valid frozen plan");
    plan["tasks"][0]["title"] = serde_json::Value::String("modified snapshot".to_owned());
    fs::write(
        &plan_path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&plan).expect("serialize plan")
        ),
    )
    .expect("tamper legacy snapshot");
    let before = fs::read(paths.events()).expect("legacy log");

    let error = resume_err(&repo, &run_id);
    assert!(error.contains("Refusing to bless"), "{error}");
    assert_eq!(
        fs::read(paths.events()).expect("refused log"),
        before,
        "refusal happens before the schema upgrade append"
    );
}

#[test]
fn resume_compares_canonical_source_semantics_to_the_recorded_plan_digest() {
    let (_tree, repo, run_id) = parked_run("source-semantics-digest");
    fs::write(
        repo.join("plan.md"),
        "## Changed semantics\n<!-- upstroke: id=t1 kind=implement depends= -->\nDifferent body.\n",
    )
    .expect("change source plan");
    let new_hash = crate::ir::content_hash(&fs::read(repo.join("plan.md")).expect("plan"));
    let paths = paths_of(&repo, &run_id);
    let text = fs::read_to_string(paths.events()).expect("log");
    let rewritten = text
        .lines()
        .map(|line| {
            let mut value: serde_json::Value = serde_json::from_str(line).expect("event");
            if value["event"] == "run_started" {
                value["data"]["plan_hash"] = serde_json::Value::String(new_hash.clone());
            }
            value.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(paths.events(), format!("{rewritten}\n")).expect("force legacy hash guard equal");

    let error = resume_err(&repo, &run_id);
    assert!(
        error.contains("validated source plan now normalizes to digest"),
        "{error}"
    );
}

#[test]
fn resume_refuses_when_routing_moved_under_a_recorded_rung() {
    let (_tree, repo, run_id) = parked_run("chainmoved");
    fs::write(
        repo.join("upstroke.toml"),
        "[interaction]\nmode = \"never\"\n\n\
             [routing]\nimplement = { chain = [\"mid\", \"frontier\"], attempts_per = 1 }\n",
    )
    .expect("edit config");

    let err = resume_err(&repo, &run_id);
    assert!(err.contains("routing has changed"), "got: {err}");
    assert!(err.contains("`t1` ran on [small]"), "names the task: {err}");
}

fn parked_run_with_gate(tag: &str, cmd: &str) -> (ScratchTree, PathBuf, String) {
    parked_run_with_config(tag, &gate_config(cmd))
}

fn gate_config(cmd: &str) -> String {
    format!("{PARKED_RUN_CONFIG}\n[[gates]]\nname = \"check\"\ncmd = \"{cmd}\"\n")
}

#[test]
fn resume_runs_the_gates_the_run_recorded_not_todays() {
    let (_tree, repo, run_id) = parked_run_with_gate("gaterecorded", "git --version");

    fs::write(
        repo.join("upstroke.toml"),
        gate_config("git frobnicate-not-a-command"),
    )
    .expect("edit config");

    let resumed = resume_answering(&repo, &run_id, Effect::EditFile);
    assert_eq!(resumed.outcome(), RunOutcome::Complete, "{resumed:?}");
    assert!(
        committed(&resumed, "t1"),
        "the recorded gate ran, not the one in today's config: {resumed:?}"
    );

    let warning = resumed
        .warnings
        .iter()
        .find(|w| w.contains("differ from the ones this run recorded"))
        .unwrap_or_else(|| panic!("no gate-difference warning: {:?}", resumed.warnings));
    assert!(
        warning.contains(
            "`check` runs `git --version` and today's config says `git \
                              frobnicate-not-a-command`"
        ),
        "names the edit: {warning}"
    );
    assert!(
        warning.contains("Start a new run to adopt them"),
        "and what to do about it: {warning}"
    );

    assert_eq!(resumed.gates, ["check"]);
}

#[test]
fn a_resume_with_a_gate_record_is_not_refused_over_todays_gates_section() {
    let (_tree, repo, run_id) = parked_run_with_gate("gate_record_lenient", "git --version");
    fs::write(
        repo.join("upstroke.toml"),
        format!(
            "{PARKED_RUN_CONFIG}\n[[gates]]\nname = \"check\"\ncmd = \"git --version\"\n\
             timeout_secs = 0\ntimeout_sec = 3600\n\
             [[gates]]\nname = \"Check\"\ncmd = \"git frobnicate-not-a-command\"\n"
        ),
    )
    .expect("edit config");

    let resumed = resume_answering(&repo, &run_id, Effect::EditFile);
    assert_eq!(resumed.outcome(), RunOutcome::Complete, "{resumed:?}");
    assert!(
        committed(&resumed, "t1"),
        "the recorded gate ran, not today's zero-timeout or failing one: {resumed:?}"
    );
    assert_eq!(
        resumed.gates,
        ["check"],
        "the report describes the recorded gate"
    );
    for shape in [
        "timeout_secs must be at least 1",
        "has unknown key `timeout_sec`",
        "repeats the name `Check` of entry 1",
    ] {
        assert!(
            resumed
                .warnings
                .iter()
                .any(|w| w.contains(shape) && w.contains("recorded")),
            "each shape is announced, with the record named as what runs (`{shape}`): {:?}",
            resumed.warnings
        );
    }
}

#[test]
fn a_resume_with_no_gate_record_refuses_the_gate_shapes_a_fresh_run_refuses() {
    let (_tree, repo, run_id) = parked_run_with_gate("gate_record_absent", "git --version");
    let paths = paths_of(&repo, &run_id);
    strip_event_data_field(&paths, "run_started", "gate_cmds");
    let before = fs::read_to_string(paths.events()).expect("the log before");
    assert!(
        events::recorded_gates(&events_of(&repo, &run_id)).is_none(),
        "the fixture must have no gate record left"
    );
    fs::write(
        repo.join("upstroke.toml"),
        format!(
            "{PARKED_RUN_CONFIG}\n[[gates]]\nname = \"check\"\ncmd = \"git --version\"\n\
             timeout_sec = 3600\n"
        ),
    )
    .expect("edit config");

    let source = source(vec![Effect::EditFile], vec![ReviewBehavior::Pass]);
    let error = resume_with(&resume_options(&repo, &run_id), &source)
        .expect_err("a resume whose gates come from this file refuses it");
    let message = error.to_string();
    assert!(
        message.contains("[[gates]] entry 1") && message.contains("`timeout_sec`"),
        "names the entry and the key: {message}"
    );

    let after = fs::read_to_string(paths.events()).expect("the log after");
    assert_eq!(after, before, "a refusal before the lock appends nothing");

    fs::write(repo.join("upstroke.toml"), gate_config("git --version")).expect("edit config");
    let resumed = resume_answering(&repo, &run_id, Effect::EditFile);
    assert_eq!(resumed.outcome(), RunOutcome::Complete, "{resumed:?}");
    assert!(committed(&resumed, "t1"), "{resumed:?}");
    assert!(
        events::recorded_gates(&events_of(&repo, &run_id)).is_some(),
        "the resume recorded the gates it settled on"
    );
}

#[test]
fn the_report_labels_gates_from_the_record_not_todays_config() {
    let (_tree, repo, run_id) = parked_run_with_gate("gatelabel", "git --version");

    fs::write(repo.join("upstroke.toml"), PARKED_RUN_CONFIG).expect("edit config");

    let resumed = resume_answering(&repo, &run_id, Effect::EditFile);
    assert_eq!(resumed.gates, ["check"], "the recorded gate ran");
    assert!(
        resumed.gates_from_config,
        "and is labelled as the record has it, not as today's config would"
    );

    let replayed = legacy_status(&replay_of(&repo, &run_id)).report();
    assert_eq!(replayed.gates, resumed.gates);
    assert_eq!(replayed.gates_from_config, resumed.gates_from_config);
}

#[test]
fn a_resume_whose_gates_did_not_move_says_nothing_about_them() {
    let (_tree, repo, run_id) = parked_run_with_gate("gateunmoved", "git --version");
    let resumed = resume_answering(&repo, &run_id, Effect::EditFile);
    assert_eq!(resumed.outcome(), RunOutcome::Complete, "{resumed:?}");
    assert!(
        !resumed
            .warnings
            .iter()
            .any(|w| w.contains("gates") && w.contains("recorded")),
        "an untouched config must warn about nothing: {:?}",
        resumed.warnings
    );
}

#[test]
fn a_gate_difference_is_described_without_inventing_edits() {
    let gate = |name: &str, cmd: &str| GateSummary {
        name: name.to_owned(),
        cmd: cmd.to_owned(),
        timeout: Duration::from_secs(600),
        shell: crate::gates::ShellKind::Sh,
    };
    let check = gate("check", "cargo test");
    let only_check = std::slice::from_ref(&check);

    assert_eq!(
        gates_differ(only_check, only_check),
        None,
        "identical gates are not a difference"
    );

    let added =
        gates_differ(only_check, &[check.clone(), gate("check", "true")]).expect("a difference");
    assert!(
        added.contains("`check` (`true`) is in today's config and not in the record"),
        "names the added gate: {added}"
    );
    assert!(
        !added.contains("different order"),
        "and does not invent a reorder: {added}"
    );

    let removed = gates_differ(&[check.clone(), gate("check", "cargo clippy")], only_check)
        .expect("a difference");
    assert!(
        removed.contains("`check` (`cargo clippy`) is in the record and not in today's config"),
        "names the removed gate: {removed}"
    );

    let edited = gates_differ(only_check, &[gate("check", "true")]).expect("a difference");
    assert!(
        edited.contains("`check` runs `cargo test` and today's config says `true`"),
        "{edited}"
    );

    let renamed = gates_differ(only_check, &[gate("verify", "cargo test")]).expect("a difference");
    assert!(
        renamed.contains("`check` (`cargo test`) is in the record"),
        "{renamed}"
    );
    assert!(
        renamed.contains("`verify` (`cargo test`) is in today's config"),
        "{renamed}"
    );

    let reshelled = gates_differ(
        only_check,
        &[GateSummary {
            shell: crate::gates::ShellKind::Bash,
            ..check.clone()
        }],
    )
    .expect("a difference");
    assert!(
        reshelled.contains("`check` runs under `sh` and today's config says `bash`"),
        "{reshelled}"
    );

    let other = gate("test", "cargo test");
    let reordered =
        gates_differ(&[check.clone(), other.clone()], &[other, check]).expect("a difference");
    assert!(reordered.contains("in a different order"), "{reordered}");
    assert!(
        !reordered.contains("not in the record"),
        "nothing came or went: {reordered}"
    );
}

#[test]
fn a_log_that_predates_the_gate_record_rederives_and_says_what_it_can() {
    let (_tree, repo, run_id) = parked_run_with_gate("oldgatelog", "git --version");
    strip_run_started_field(&paths_of(&repo, &run_id), "gate_cmds");

    fs::write(
        repo.join("upstroke.toml"),
        format!("{PARKED_RUN_CONFIG}\n[[gates]]\nname = \"renamed\"\ncmd = \"git --version\"\n"),
    )
    .expect("edit config");

    let resumed = resume_answering(&repo, &run_id, Effect::EditFile);
    assert_eq!(resumed.outcome(), RunOutcome::Complete, "{resumed:?}");
    let warning = resumed
        .warnings
        .iter()
        .find(|w| w.contains("predates the gate record"))
        .unwrap_or_else(|| panic!("no warning: {:?}", resumed.warnings));
    assert!(
        warning.contains("the gate names have moved"),
        "an old log still knows this much: {warning}"
    );
    assert!(
        warning.contains("recorded [check]") && warning.contains("resolves [renamed]"),
        "and says which: {warning}"
    );
}

#[test]
fn the_resume_that_rederives_an_old_logs_gates_records_them_for_the_next_one() {
    let (_tree, repo, run_id) = parked_run_with_gate("oldgateestablish", "git --version");
    strip_run_started_field(&paths_of(&repo, &run_id), "gate_cmds");

    let first = resume_answering(&repo, &run_id, Effect::NoEdit);
    assert_eq!(first.outcome(), RunOutcome::Parked, "{first:?}");
    assert!(
        first
            .warnings
            .iter()
            .any(|w| w.contains("predates the gate record")),
        "the first resume re-derived: {:?}",
        first.warnings
    );

    let paths = paths_of(&repo, &run_id);
    let mut log_warnings = Vec::new();
    let logged = events::read_all(&paths.events(), &mut log_warnings).expect("log");
    let established = events::recorded_gates(&logged).expect("the resume recorded its gates");
    assert_eq!(established.len(), 1);
    assert_eq!(established[0].cmd, "git --version");

    fs::write(
        repo.join("upstroke.toml"),
        gate_config("git frobnicate-not-a-command"),
    )
    .expect("edit config");

    let second = resume_answering(&repo, &run_id, Effect::EditFile);
    assert_eq!(second.outcome(), RunOutcome::Complete, "{second:?}");
    assert!(
        committed(&second, "t1"),
        "the established gate ran, not the weakened one: {second:?}"
    );

    assert!(
        second
            .warnings
            .iter()
            .any(|w| w.contains("differ from the ones this run recorded")),
        "{:?}",
        second.warnings
    );
    assert!(
        !second
            .warnings
            .iter()
            .any(|w| w.contains("predates the gate record")),
        "the log is no longer speechless about its gates: {:?}",
        second.warnings
    );
}

#[test]
fn an_old_gateless_log_is_not_warned_at_about_nothing() {
    let (_tree, repo, run_id) = parked_run("oldgatelessslog");
    strip_run_started_field(&paths_of(&repo, &run_id), "gate_cmds");

    let resumed = resume_answering(&repo, &run_id, Effect::EditFile);
    assert!(
        !resumed.warnings.iter().any(|w| w.contains("gate")),
        "nothing to say: {:?}",
        resumed.warnings
    );
}

#[test]
fn resume_refuses_when_the_run_branch_is_gone() {
    let (_tree, repo, run_id) = parked_run("branchgone");
    let branch = git_in(&repo, &["rev-parse", "--abbrev-ref", "HEAD"])
        .trim()
        .to_owned();
    git_in(&repo, &["switch", "-q", "main"]);
    git_in(&repo, &["branch", "-q", "-D", &branch]);

    let err = resume_err(&repo, &run_id);
    assert!(err.contains("no longer exists"), "got: {err}");
}

#[test]
fn resume_refuses_to_switch_over_uncommitted_work() {
    let (_tree, repo, run_id) = parked_run("dirtyelsewhere");
    git_in(&repo, &["switch", "-q", "main"]);
    fs::write(
        repo.join("my-own-work.txt"),
        "not the engine's to discard\n",
    )
    .expect("file");

    let err = resume_err(&repo, &run_id);
    assert!(err.contains("Commit or stash"), "got: {err}");
    assert!(
        repo.join("my-own-work.txt").exists(),
        "a refusal must not destroy the work it refused over"
    );
}

#[test]
fn resume_refuses_a_run_that_already_finished_or_halted() {
    let (_tree, repo) = temp_engine_repo("finished");
    let complete = fake(Effect::EditFile);
    let report = run_with(&options(&repo), &complete).expect("run");
    assert_eq!(report.outcome(), RunOutcome::Complete);
    let err = resume_err(&repo, &report.run_id);
    assert!(err.contains("already completed"), "got: {err}");

    let (_tree, repo) = temp_engine_repo("halted");
    seed(
        &repo,
        "## Doomed\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = source(vec![Effect::NoEdit], vec![ReviewBehavior::Pass]);
    let answers = ScriptedAnswers::new(vec![Answer::Declined]);
    let report = run_harness(
        &opts,
        &Harness {
            adapters: &source,
            answers: Some(&answers),
            sleeper: None,
        },
    )
    .expect("run");
    assert_eq!(report.outcome(), RunOutcome::Halted);
    let err = resume_err(&repo, &report.run_id);
    assert!(err.contains("halted at `t1`"), "got: {err}");
}

#[test]
fn resume_refuses_while_another_process_holds_the_run() {
    let (_tree, repo, run_id) = parked_run("locked");
    let paths = paths_of(&repo, &run_id);
    let _held = RunLock::acquire(&paths.public).expect("hold it");

    let err = resume_err(&repo, &run_id);
    assert!(err.contains("already driving run"), "got: {err}");
}

#[test]
fn an_unknown_run_id_lists_what_is_there() {
    let (_tree, repo, _) = parked_run("unknownid");
    let err = resume_err(&repo, "01NOPE");
    assert!(err.contains("known runs"), "got: {err}");
}

#[test]
fn status_reports_a_live_run_and_the_ledger_reads_from_the_log() {
    let (_tree, repo) = temp_engine_repo("statusledger");
    let source = fake(Effect::EditFile);
    let report = run_with(&options(&repo), &source).expect("run");

    let loaded = replay_of(&repo, &report.run_id);
    assert!(
        !legacy_status(&loaded).running,
        "nothing holds a finished run"
    );
    assert!(!legacy_status(&loaded).interrupted_run());

    let rendered = crate::status::render(&loaded);
    assert!(rendered.contains("ledger:"), "{rendered}");
    assert!(rendered.contains("t1"), "{rendered}");
    assert!(
        rendered.contains("per-pool drain: no pool is connected"),
        "{rendered}"
    );
    assert!(
        rendered.contains(&legacy_status(&loaded).paths.private.display().to_string()),
        "and points at where the transcripts actually are"
    );

    assert!(
        (legacy_status(&loaded).report().total_cost_usd - report.total_cost_usd).abs() < 1e-9,
        "{} vs {}",
        legacy_status(&loaded).report().total_cost_usd,
        report.total_cost_usd
    );

    let paths = paths_of(&repo, &report.run_id);
    let _held = RunLock::acquire(&paths.public).expect("claim the finished run");
    let claimed = replay_of(&repo, &report.run_id);
    assert!(
        !legacy_status(&claimed).running,
        "a finished run is not running"
    );
    assert!(legacy_status(&claimed).held, "but something does hold it");
    let rendered = crate::status::render(&claimed);
    assert!(
        rendered.contains("another process holds this run"),
        "{rendered}"
    );
    assert!(rendered.contains("run complete"), "{rendered}");
    assert!(!rendered.contains("run in progress"), "{rendered}");
}

#[test]
fn following_a_finished_run_replays_it_and_stops() {
    let (_tree, repo) = temp_engine_repo("follow");
    let source = fake(Effect::EditFile);
    let report = run_with(&options(&repo), &source).expect("run");

    let loaded = replay_of(&repo, &report.run_id);
    let sleeper = RecordingSleeper::default();
    let mut out: Vec<u8> = Vec::new();
    crate::status::follow(&loaded, &sleeper, Duration::ZERO, 2, &mut out).expect("follow");
    let text = String::from_utf8(out).expect("utf8");

    assert!(text.contains("run"), "{text}");
    assert!(text.contains("t1: committed"), "{text}");
    assert!(
        text.contains("run finished"),
        "it stops at the ending rather than idling: {text}"
    );
    assert!(
        sleeper.waits().is_empty(),
        "a finished run needs no waiting at all"
    );
    for line in text.lines() {
        assert!(!line.is_empty());
    }
}

#[test]
fn follow_ignores_a_terminal_marker_superseded_by_resume() {
    let (_tree, repo) = temp_engine_repo("followresume");
    let source = fake(Effect::EditFile);
    let report = run_with(&options(&repo), &source).expect("run");
    let paths = paths_of(&repo, &report.run_id);
    let mut warnings = Vec::new();
    let mut log = events::EventLog::open(EventSite::LegacyOpenLog, &paths.events(), &mut warnings)
        .expect("open log");
    log.append(
        EventSite::LegacyAppend,
        EventBody::RunResumed {
            data: events::RunResumed {
                head_sha: "second-epoch".to_owned(),
                interrupted_attempts: 0,
                discarded: Vec::new(),
                gates: None,
                effort_policy: None,
                reviews: None,
                chains: None,
                normalized_plan_digest: None,
            },
        },
    )
    .expect("resume marker");
    log.append(
        EventSite::LegacyAppend,
        EventBody::RunFinished {
            data: events::RunFinished {
                outcome: events::RunOutcome::Complete,
                halted_at: None,
                committed: 1,
                parked: 0,
            },
        },
    )
    .expect("second finish");
    drop(log);

    let loaded = replay_of(&repo, &report.run_id);
    let sleeper = RecordingSleeper::default();
    let mut out = Vec::new();
    crate::status::follow(&loaded, &sleeper, Duration::ZERO, 2, &mut out).expect("follow");
    let text = String::from_utf8(out).expect("utf8");
    assert!(text.contains("resumed at second-epo"), "{text}");
    assert_eq!(
        text.matches("run finished").count(),
        2,
        "the historical finish must not truncate the later epoch: {text}"
    );
}

#[test]
fn follow_waits_at_held_historical_terminal_until_resume_marker() {
    struct ResumeOnSleep {
        events: PathBuf,
        lock: Mutex<Option<RunLock>>,
    }

    impl Sleeper for ResumeOnSleep {
        fn sleep(&self, _: Duration) {
            let Ok(mut lock) = self.lock.lock() else {
                return;
            };
            if lock.is_none() {
                return;
            }
            let mut warnings = Vec::new();
            let mut log =
                events::EventLog::open(EventSite::LegacyOpenLog, &self.events, &mut warnings)
                    .expect("log");
            log.append(
                EventSite::LegacyAppend,
                EventBody::RunResumed {
                    data: events::RunResumed {
                        head_sha: "resumed-head".to_owned(),
                        interrupted_attempts: 0,
                        discarded: Vec::new(),
                        gates: None,
                        effort_policy: None,
                        reviews: None,
                        chains: None,
                        normalized_plan_digest: None,
                    },
                },
            )
            .expect("resume marker");
            log.append(
                EventSite::LegacyAppend,
                EventBody::RunFinished {
                    data: events::RunFinished {
                        outcome: events::RunOutcome::Complete,
                        halted_at: None,
                        committed: 1,
                        parked: 0,
                    },
                },
            )
            .expect("new terminal");
            drop(log);
            drop(lock.take());
        }
    }

    let (_tree, repo) = temp_engine_repo("followheldterminal");
    let report = run_with(&options(&repo), &fake(Effect::EditFile)).expect("run");
    let paths = paths_of(&repo, &report.run_id);
    let loaded = replay_of(&repo, &report.run_id);
    let sleeper = ResumeOnSleep {
        events: paths.events(),
        lock: Mutex::new(Some(
            RunLock::acquire(&paths.public).expect("resume owns lock before marker"),
        )),
    };
    let mut out = Vec::new();
    crate::status::follow(&loaded, &sleeper, Duration::ZERO, 1, &mut out).expect("follow");
    let text = String::from_utf8(out).expect("utf8");
    assert!(text.contains("resumed at resumed-he"), "{text}");
    assert_eq!(text.matches("run finished").count(), 2, "{text}");
}

#[test]
fn transcripts_live_outside_the_workspace_and_survive_a_rollback() {
    let (_tree, repo) = temp_engine_repo("private");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 2 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));

    let adapters = source(
        vec![Effect::NoEdit, Effect::EditFile],
        vec![ReviewBehavior::Pass],
    );
    let report = run_with(&opts, &adapters).expect("run");
    let paths = paths_of(&repo, &report.run_id);

    for private in [paths.transcripts(), paths.reviews(), paths.settings()] {
        assert!(
            !private.starts_with(&repo),
            "{} must be outside the workspace",
            private.display()
        );
        assert!(
            fs::read_dir(&private).into_iter().flatten().count() > 0,
            "{} kept its contents across the rollback",
            private.display()
        );
    }

    assert!(paths.events().starts_with(&repo));
    assert!(paths.questions().starts_with(&repo));

    let in_repo = repo.join(".upstroke").join("runs").join(&report.run_id);
    for leaked in ["transcripts", "reviews", "settings", "gates"] {
        assert!(
            !in_repo.join(leaked).exists(),
            "{leaked}/ must not exist inside the workspace"
        );
    }
}

fn strip_run_started_field(paths: &RunPaths, field: &str) {
    let text = fs::read_to_string(paths.events()).expect("log");
    let mut stripped = false;
    let rewritten: Vec<String> = text
        .lines()
        .map(|line| {
            let mut value: serde_json::Value =
                serde_json::from_str(line).expect("every line is an event");
            if value.get("event").and_then(|e| e.as_str()) == Some("run_started") {
                if let Some(data) = value.get_mut("data").and_then(|d| d.as_object_mut()) {
                    data.remove(field)
                        .unwrap_or_else(|| panic!("the run recorded no `{field}`"));
                    stripped = true;
                }
            }
            value.to_string()
        })
        .collect();
    assert!(
        stripped,
        "the log has no run_started to strip `{field}` from"
    );
    fs::write(paths.events(), format!("{}\n", rewritten.join("\n"))).expect("rewrite");
}

fn rewrite_run_started_as_schema_one(paths: &RunPaths, absent: &[&str]) {
    let text = fs::read_to_string(paths.events()).expect("log");
    let mut rewritten_start = false;
    let rewritten: Vec<String> = text
        .lines()
        .map(|line| {
            let mut value: serde_json::Value =
                serde_json::from_str(line).expect("every line is an event");
            if value.get("event").and_then(|event| event.as_str()) == Some("run_started") {
                let data = value
                    .get_mut("data")
                    .and_then(serde_json::Value::as_object_mut)
                    .expect("run_started data");
                data.insert("schema".to_owned(), serde_json::Value::from(1));
                data.remove("normalized_plan_digest");
                for field in absent {
                    data.remove(*field)
                        .unwrap_or_else(|| panic!("the run recorded no `{field}`"));
                }
                for chain in data
                    .get_mut("chains")
                    .and_then(serde_json::Value::as_array_mut)
                    .expect("run_started chains")
                {
                    chain
                        .as_object_mut()
                        .expect("chain object")
                        .remove("bindings")
                        .expect("a schema-2 run records chain bindings");
                }
                rewritten_start = true;
            }
            value.to_string()
        })
        .collect();
    assert!(rewritten_start, "the log has no run_started event");
    fs::write(paths.events(), format!("{}\n", rewritten.join("\n"))).expect("rewrite");
}

fn strip_event_field(paths: &RunPaths, event: &str, field: &str) {
    rewrite_event_field(paths, event, field, false);
}

fn truncate_log_before(paths: &RunPaths, event: &str) {
    let text = fs::read_to_string(paths.events()).expect("log");
    let lines: Vec<&str> = text.lines().collect();
    let cut = lines
        .iter()
        .position(|line| line.contains(&format!("\"{event}\"")))
        .unwrap_or_else(|| panic!("the run recorded no {event}"));
    fs::write(paths.events(), format!("{}\n", lines[..cut].join("\n"))).expect("truncate");
}

fn prepared_commit_of(paths: &RunPaths) -> events::PreparedCommit {
    let mut warnings = Vec::new();
    events::read_all(&paths.events(), &mut warnings)
        .expect("read prepared settlement")
        .into_iter()
        .find_map(|event| match event.body {
            EventBody::AttemptFinished {
                prepared_commit: Some(prepared),
                ..
            } => Some(*prepared),
            _ => None,
        })
        .expect("successful settlement records its prepared commit")
}

fn recreate_prepared_pin(repo: &Path, prepared: &events::PreparedCommit, target: &str) {
    let zero = "0".repeat(target.len());
    git_in(repo, &["update-ref", &prepared.pin_ref, target, &zero]);
}

#[test]
fn resume_adopts_the_commit_it_made_but_never_recorded() {
    let (_tree, repo) = temp_engine_repo("adoptcommit");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let report = run_with(&opts, &fake(Effect::EditFile)).expect("run");
    let run_id = report.run_id.clone();
    let paths = paths_of(&repo, &run_id);
    let sha = git_in(&repo, &["rev-parse", "HEAD"]).trim().to_owned();

    truncate_log_before(&paths, "task_committed");

    let source = fake(Effect::EditFile);
    let (resumed, state) = resume_harness_inner(
        &resume_options(&repo, &run_id),
        &Harness {
            adapters: &source,
            answers: None,
            sleeper: None,
        },
    )
    .expect("resume");

    assert_eq!(resumed.outcome(), RunOutcome::Complete, "{resumed:?}");
    assert!(committed(&resumed, "t1"), "adopted rather than redone");
    assert_eq!(
        git_in(&repo, &["rev-parse", "HEAD"]).trim(),
        sha,
        "and the branch was left exactly where it stood"
    );
    assert_eq!(
        git_in(&repo, &["rev-list", "--count", "main..HEAD"]).trim(),
        "1",
        "one commit, not a second one for the same work"
    );
    assert_eq!(
        task(&resumed, "t1").attempts.len(),
        1,
        "the attempt that passed was not spent again: {resumed:?}"
    );
    assert!(
        resumed
            .warnings
            .iter()
            .any(|w| w.contains("adopted commit")),
        "and the operator is told: {:?}",
        resumed.warnings
    );
    assert_live_equals_replay(&repo, &state, &resumed);
}

#[test]
fn resume_recovers_every_prepared_commit_ref_crash_prefix() {
    for (tag, reset_to_parent, recreate_pin) in [
        ("prepared-same-head", true, true),
        ("prepared-head-with-pin", false, true),
        ("prepared-head-no-pin", false, false),
    ] {
        let (_tree, repo) = temp_engine_repo(tag);
        seed(
            &repo,
            "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
            Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
        );
        let mut opts = options(&repo);
        opts.config_path = Some(repo.join("upstroke.toml"));
        let report = run_with(&opts, &fake(Effect::EditFile)).expect("run");
        let paths = paths_of(&repo, &report.run_id);
        let prepared = prepared_commit_of(&paths);
        truncate_log_before(&paths, "task_committed");
        if reset_to_parent {
            git_in(&repo, &["reset", "-q", "--soft", &prepared.parent_sha]);
        }
        if recreate_pin {
            let target = prepared.commit_sha.clone();
            recreate_prepared_pin(&repo, &prepared, &target);
        }

        let source = fake(Effect::EditFile);
        let resumed = resume_harness(
            &resume_options(&repo, &report.run_id),
            &Harness {
                adapters: &source,
                answers: None,
                sleeper: None,
            },
        )
        .expect("recover exact prepared object");
        assert_eq!(
            resumed.outcome(),
            RunOutcome::Complete,
            "{tag}: {resumed:?}"
        );
        assert_eq!(
            git_in(&repo, &["rev-parse", "HEAD"]).trim(),
            prepared.commit_sha,
            "{tag}: the exact reviewed object is published"
        );
        assert_eq!(task(&resumed, "t1").attempts.len(), 1, "{tag}");
        let workspace = Workspace::open(&repo).expect("workspace");
        assert_eq!(
            workspace
                .prepared_pin_target(&prepared.pin_ref)
                .expect("pin lookup"),
            None,
            "{tag}: recovery cleans the private pin"
        );
    }
}

#[test]
fn resume_removes_a_pin_whose_successful_settlement_never_landed() {
    let (_tree, repo) = temp_engine_repo("prepared-orphan");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 2 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let report = run_with(&opts, &fake(Effect::EditFile)).expect("run");
    let paths = paths_of(&repo, &report.run_id);
    let prepared = prepared_commit_of(&paths);
    truncate_log_before(&paths, "attempt_finished");
    git_in(&repo, &["reset", "-q", "--soft", &prepared.parent_sha]);
    let target = prepared.commit_sha.clone();
    recreate_prepared_pin(&repo, &prepared, &target);

    let source = fake(Effect::EditFile);
    let resumed = resume_harness(
        &resume_options(&repo, &report.run_id),
        &Harness {
            adapters: &source,
            answers: None,
            sleeper: None,
        },
    )
    .expect("orphan pin is not a settlement");
    assert_eq!(resumed.outcome(), RunOutcome::Complete, "{resumed:?}");
    assert_eq!(task(&resumed, "t1").attempts.len(), 2);
    assert!(
        resumed
            .warnings
            .iter()
            .any(|warning| warning.contains("removed orphan prepared commit pin")),
        "{:?}",
        resumed.warnings
    );
    assert_eq!(
        Workspace::open(&repo)
            .expect("workspace")
            .prepared_pin_target(&prepared.pin_ref)
            .expect("pin lookup"),
        None
    );
}

#[test]
fn resume_refuses_a_substituted_prepared_pin_without_deleting_it() {
    let (_tree, repo) = temp_engine_repo("prepared-pin-mismatch");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let report = run_with(&opts, &fake(Effect::EditFile)).expect("run");
    let paths = paths_of(&repo, &report.run_id);
    let prepared = prepared_commit_of(&paths);
    truncate_log_before(&paths, "task_committed");
    git_in(&repo, &["reset", "-q", "--soft", &prepared.parent_sha]);
    recreate_prepared_pin(&repo, &prepared, &prepared.parent_sha);

    let err = resume_err(&repo, &report.run_id);
    assert!(err.contains("not pinned"), "{err}");
    assert_eq!(
        Workspace::open(&repo)
            .expect("workspace")
            .prepared_pin_target(&prepared.pin_ref)
            .expect("pin lookup")
            .as_deref(),
        Some(prepared.parent_sha.as_str()),
        "refusal never deletes the substituted target"
    );
    assert_eq!(
        git_in(&repo, &["rev-parse", "HEAD"]).trim(),
        prepared.parent_sha,
        "HEAD remains at the recorded parent"
    );
}

#[test]
fn resume_refuses_symbolic_run_ref_at_already_published_prepared_prefix() {
    let (_tree, repo) = temp_engine_repo("prepared-symbolic-run-ref");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let report = run_with(&opts, &fake(Effect::EditFile)).expect("run");
    let paths = paths_of(&repo, &report.run_id);
    let prepared = prepared_commit_of(&paths);
    truncate_log_before(&paths, "task_committed");
    recreate_prepared_pin(&repo, &prepared, &prepared.commit_sha);

    git_in(&repo, &["branch", "victim", prepared.commit_sha.as_str()]);
    git_in(
        &repo,
        &[
            "symbolic-ref",
            prepared.branch_ref.as_str(),
            "refs/heads/victim",
        ],
    );
    let events_before = fs::read(paths.events()).expect("event bytes before refusal");
    let victim_before = git_in(&repo, &["rev-parse", "refs/heads/victim"]);

    let error = resume_err(&repo, &report.run_id);
    assert!(error.contains("itself symbolic"), "{error}");
    assert_eq!(
        fs::read(paths.events()).expect("event bytes after refusal"),
        events_before,
        "refusal happens before task_committed or any other repair append"
    );
    assert_eq!(
        Workspace::open(&repo)
            .expect("workspace")
            .prepared_pin_target(&prepared.pin_ref)
            .expect("pin lookup")
            .as_deref(),
        Some(prepared.commit_sha.as_str()),
        "refusal preserves the durable prepared pin"
    );
    assert_eq!(
        git_in(&repo, &["rev-parse", "refs/heads/victim"]),
        victim_before,
        "the symbolic run ref never advances or deletes its victim"
    );
    assert_eq!(
        git_in(
            &repo,
            &["symbolic-ref", "--no-recurse", prepared.branch_ref.as_str(),],
        )
        .trim(),
        "refs/heads/victim",
        "refusal preserves the substituted symbolic run ref for inspection"
    );
}

#[test]
fn recovered_prepared_commit_precedes_unrelated_answer_defect_repair() {
    let (_tree, repo) = temp_engine_repo("prepared-before-repair");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let report = run_with(&opts, &fake(Effect::EditFile)).expect("run");
    let paths = paths_of(&repo, &report.run_id);
    truncate_log_before(&paths, "task_committed");

    let question_id = QuestionId::from("q-before-success");
    let question = Question {
        id: question_id.clone(),
        kind: QuestionKind::Unblock,
        affected_tasks: vec![TaskId::from("t1")],
        context: "an earlier question".to_owned(),
        options: Vec::new(),
    };
    let inserted = [
        events::Event::now(EventBody::QuestionRaised {
            task: "t1".to_owned(),
            data: Box::new(events::QuestionRaised {
                question: question.clone(),
            }),
        }),
        events::Event::now(EventBody::TaskParked {
            task: "t1".to_owned(),
            data: events::TaskParked {
                question: question_id.to_string(),
                refund_attempt: false,
            },
        }),
        events::Event::now(EventBody::QuestionAnswered {
            data: events::QuestionAnswered {
                question: question_id,
                answer: Answer::Answered {
                    text: "continue".to_owned(),
                },
                decline_halts_run: None,
                via: "answer-file".to_owned(),
            },
        }),
    ];
    let mut lines: Vec<String> = fs::read_to_string(paths.events())
        .expect("log")
        .lines()
        .map(str::to_owned)
        .collect();
    let before_attempt = lines
        .iter()
        .position(|line| line.contains("\"attempt_started\""))
        .expect("attempt start");
    lines.splice(
        before_attempt..before_attempt,
        inserted
            .iter()
            .map(|event| serde_json::to_string(event).expect("event json")),
    );
    fs::write(paths.events(), format!("{}\n", lines.join("\n"))).expect("insert prefix");

    let source = fake(Effect::EditFile);
    let resumed = resume_harness(
        &resume_options(&repo, &report.run_id),
        &Harness {
            adapters: &source,
            answers: None,
            sleeper: None,
        },
    )
    .expect("resume closes settlement before repairing older metadata");
    assert_eq!(resumed.outcome(), RunOutcome::Complete, "{resumed:?}");
    let logged = events_of(&repo, &report.run_id);
    let settlement = logged
        .iter()
        .rposition(|event| matches!(event.body, EventBody::AttemptFinished { .. }))
        .expect("settlement");
    assert!(
        matches!(
            logged.get(settlement + 1).map(|event| &event.body),
            Some(EventBody::TaskCommitted { task, .. }) if task == "t1"
        ),
        "task_committed must immediately close the prepared settlement"
    );
    assert!(
        logged
            .iter()
            .skip(settlement + 2)
            .any(|event| matches!(event.body, EventBody::DesignDefect { .. })),
        "the older answer repair still lands after the commit"
    );
    events::replay(logged, vec!["t1".to_owned()], &paths.events())
        .expect("the repaired log remains replayable");
}

#[test]
fn resume_refuses_an_arbitrary_tree_with_the_same_parent_and_subject() {
    let (_tree, repo) = temp_engine_repo("adoptforeign");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let report = run_with(&opts, &fake(Effect::EditFile)).expect("run");
    truncate_log_before(&paths_of(&repo, &report.run_id), "task_committed");
    let subject = git_in(&repo, &["show", "-s", "--format=%s", "HEAD"]);
    fs::write(repo.join("foreign.txt"), "not reviewed\n").expect("foreign tree");
    git_in(&repo, &["add", "foreign.txt"]);
    git_in(&repo, &["commit", "-q", "--amend", "--no-edit"]);
    assert_eq!(
        git_in(&repo, &["show", "-s", "--format=%s", "HEAD"]),
        subject,
        "the substituted commit deliberately has the expected subject"
    );

    let err = resume_err(&repo, &report.run_id);
    assert!(err.contains("record ends at"), "got: {err}");
}

#[test]
fn legacy_success_without_prepared_identity_is_never_adopted_by_subject() {
    let (_tree, repo) = temp_engine_repo("legacy-subject");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let report = run_with(&opts, &fake(Effect::EditFile)).expect("run");
    let paths = paths_of(&repo, &report.run_id);
    truncate_log_before(&paths, "task_committed");
    rewrite_run_started_as_schema_two(&paths);
    strip_event_field(&paths, "attempt_finished", "prepared_commit");

    let err = resume_err(&repo, &report.run_id);
    assert!(err.contains("subject alone"), "{err}");
    assert_eq!(
        git_in(&repo, &["rev-list", "--count", "main..HEAD"]).trim(),
        "1",
        "refusal preserves the plausible legacy commit"
    );
}

#[test]
fn resume_writes_where_the_run_recorded_not_where_defaults_point() {
    let (_tree, repo) = temp_engine_repo("privatedir");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let report = run_with(&opts, &fake(Effect::EditFile)).expect("run");
    let run_id = report.run_id.clone();
    let recorded = paths_of(&repo, &run_id);

    truncate_log_before(&recorded, "attempt_finished");
    git_in(&repo, &["reset", "-q", "--hard", "HEAD~1"]);

    let mut resume = resume_options(&repo, &run_id);
    resume.private_root = None;
    let source = fake(Effect::EditFile);
    let resumed = resume_harness(
        &resume,
        &Harness {
            adapters: &source,
            answers: None,
            sleeper: None,
        },
    )
    .expect("resume");
    assert_eq!(resumed.outcome(), RunOutcome::Complete, "{resumed:?}");
    assert!(
        recorded.transcripts().join("00-t1-2.json").is_file(),
        "the resumed attempt wrote under {}",
        recorded.transcripts().display()
    );
}

#[test]
fn resume_makes_a_stale_question_payload_agree_with_the_log() {
    let (_tree, repo) = temp_engine_repo("stalepayload");
    seed(
        &repo,
        "## Doomed\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some(
            "[interaction]\nmode = \"never\"\n\n\
                 [routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n",
        ),
    );
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let doomed = source(vec![Effect::NoEdit], vec![ReviewBehavior::Pass]);
    let report = run_with(&opts, &doomed).expect("run");
    let run_id = report.run_id.clone();
    let first = report.questions[0].question.id.to_string();

    crate::answer::answer(
        &repo,
        &first,
        crate::answer::Reply::Text("try again".to_owned()),
    )
    .expect("answer");

    let retry = source(vec![Effect::NoEdit], vec![ReviewBehavior::Pass]);
    let resumed = resume_with(&resume_options(&repo, &run_id), &retry).expect("resume");
    assert_eq!(resumed.outcome(), RunOutcome::Parked, "{resumed:?}");

    let questions = rundir::public_dir(&repo, &run_id).join("questions");
    let path = questions.join(format!("{first}.json"));
    let mut record: QuestionRecord =
        serde_json::from_str(&fs::read_to_string(&path).expect("read")).expect("parse");
    record.answer = None;
    interaction::write_question(&questions, &record).expect("rewrite");
    let refused = crate::answer::answer(&repo, &first, crate::answer::Reply::Decline).expect_err(
        "a stale payload makes a second answer look acceptable, and create-once \
             publication refuses it all the same",
    );
    assert!(
        refused.to_string().contains("already answered"),
        "got: {refused}"
    );

    let source = fake(Effect::EditFile);
    resume_with(&resume_options(&repo, &run_id), &source).expect("second resume");

    let record: QuestionRecord =
        serde_json::from_str(&fs::read_to_string(&path).expect("read")).expect("parse");
    assert!(
        !record.is_open(),
        "the payload agrees with the log again, so nobody answers it twice"
    );
}

#[test]
fn a_private_run_collision_refuses_before_early_error_cleanup() {
    let root =
        rundir::scratch_tree::acquire(&std::env::temp_dir(), "coordinator-private-collision")
            .expect("owned regression fixture");
    let repo = root.path().join("second-repo");
    fs::create_dir(&repo).expect("create the second repository");
    git_in(&repo, &["init", "-q", "-b", "main"]);
    git_in(&repo, &["config", "user.email", "test@upstroke.local"]);
    git_in(&repo, &["config", "user.name", "upstroke tests"]);
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );
    // If the coordinator mistakenly adopts the occupied private root, this
    // branch conflict sends it through the destructive early-error arm.
    git_in(&repo, &["branch", "upstroke"]);

    const RUN_ID: &str = "01M191Y2PSVX78RNEP31D23K02";
    let private_root = root.path().join("shared-home");
    let first =
        rundir::RunPaths::with_private_root(&root.path().join("first-repo"), RUN_ID, &private_root);
    first
        .create_fresh()
        .expect("the first run reserves its private half");
    let transcript = first.transcripts().join("t1-1.json");
    fs::write(&transcript, b"first run evidence").expect("first run transcript");
    let mut opts = options(&repo);
    opts.private_root = Some(private_root);
    let source = fake(Effect::EditFile);
    let harness = Harness::new(&source);
    let contained = crate::runner::host::contain_write_command(&mut crate::agent::proc::NoHooks)
        .expect("the test coordinator is contained");
    let error = coordinator::run_harness_inner_with_id(
        &opts,
        &harness,
        &crate::runner::host::HostRunner::new(),
        &contained,
        || RUN_ID.to_owned(),
    )
    .expect_err("the second run refuses the occupied private half");
    assert_eq!(
        fs::read(&transcript).expect("early-error cleanup did not delete the first run"),
        b"first run evidence"
    );
    assert!(
        error
            .to_string()
            .contains("reserve fresh private run directory"),
        "the allocation boundary must refuse before branch creation: {error}"
    );
    assert!(
        rundir::scratch_tree::proves_absent(&opts.paths(RUN_ID).public),
        "the refused coordinator leaves no public run directory"
    );
}

#[test]
fn a_run_that_never_started_leaves_no_directory_behind() {
    let (_tree, repo) = temp_engine_repo("husk");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 1 }\n"),
    );

    git_in(&repo, &["branch", "upstroke"]);

    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    let source = fake(Effect::EditFile);
    run_with(&opts, &source).expect_err("the run branch cannot be created");

    assert_eq!(
        rundir::latest_run(&repo),
        None,
        "no husk left behind to shadow the next run"
    );
}

struct LockReleasingSleeper {
    waits: Mutex<u32>,
    release_after: u32,
    lock: Mutex<Option<RunLock>>,
}

impl LockReleasingSleeper {
    fn new(lock: RunLock, release_after: u32) -> Self {
        Self {
            waits: Mutex::new(0),
            release_after,
            lock: Mutex::new(Some(lock)),
        }
    }

    fn waits(&self) -> u32 {
        self.waits.lock().map(|w| *w).unwrap_or(0)
    }
}

impl Sleeper for LockReleasingSleeper {
    fn sleep(&self, _: Duration) {
        let Ok(mut waits) = self.waits.lock() else {
            return;
        };
        *waits += 1;
        if *waits == self.release_after {
            if let Ok(mut lock) = self.lock.lock() {
                drop(lock.take());
            }
        }
    }
}

#[test]
fn following_waits_out_a_silent_live_run_and_stops_once_it_dies() {
    let (_tree, repo) = temp_engine_repo("followlive");
    let source = fake(Effect::EditFile);
    let report = run_with(&options(&repo), &source).expect("run");
    let paths = paths_of(&repo, &report.run_id);

    let text = fs::read_to_string(paths.events()).expect("log");
    let kept: Vec<&str> = text
        .lines()
        .filter(|line| !line.contains("\"run_finished\""))
        .collect();
    fs::write(paths.events(), format!("{}\n", kept.join("\n"))).expect("truncate");

    let loaded = replay_of(&repo, &report.run_id);
    let held = RunLock::acquire(&paths.public).expect("simulate a live engine");
    let sleeper = LockReleasingSleeper::new(held, 5);
    let mut out: Vec<u8> = Vec::new();

    crate::status::follow(&loaded, &sleeper, Duration::ZERO, 1, &mut out).expect("follow");

    assert!(
        sleeper.waits() > 5,
        "watched the live run past its idle budget and stopped once the lock went, \
             instead of timing out its silence: {} sleeps",
        sleeper.waits()
    );
}
