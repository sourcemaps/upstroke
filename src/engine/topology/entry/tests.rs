use std::collections::BTreeMap;

use super::*;
use crate::rundir::scratch_tree::{ScratchTree, acquire};
use crate::runner::container::FakeRuntime;
use crate::runner::container::runtime::RuntimeOp;
use crate::topology::events::{ImageIdentity, RunnerContract};
use crate::workspace_manager::fixture::write_file;

const IMAGE_ID: &str = "sha256:1111222233334444555566667777888899990000aaaabbbbccccddddeeeeffff";

fn scratch(tag: &str) -> ScratchTree {
    let parent = std::env::temp_dir();
    match acquire(&parent, tag) {
        Ok(tree) => tree,
        Err(refusal) => panic!(
            "a scratch tree for `{tag}` under {}: {refusal:?}",
            parent.display()
        ),
    }
}

fn container_policy(image: Option<&str>) -> RunnerPolicy {
    RunnerPolicy {
        kind: RunnerKind::Container,
        policy: RunnerContract::ContainerV1,
        image: image.map(|id| ImageIdentity {
            reference: "ghcr.io/example/upstroke-runner:entry".to_owned(),
            id: id.to_owned(),
            digest: None,
        }),
        credential_volumes: Some(BTreeMap::new()),
        mounts: Vec::new(),
    }
}

#[test]
fn the_container_runner_reads_its_base_environment_from_the_recorded_image_id() {
    let runtime = FakeRuntime::new(ContainerTrace::recording());
    runtime.add_image(IMAGE_ID, None);
    runtime.set_image_environment(IMAGE_ID, &[("PATH", "/usr/local/bin:/usr/bin:/bin")]);
    assert_eq!(
        recorded_image_environment(&runtime, &container_policy(Some(IMAGE_ID)))
            .expect("the recorded image declares a usable environment"),
        vec![("PATH".to_owned(), "/usr/local/bin:/usr/bin:/bin".to_owned())]
    );
    assert_eq!(
        runtime.calls(),
        vec![RuntimeOp::InspectImageById],
        "one read-only inspection of the recorded id, and nothing else"
    );
}

#[test]
fn an_image_environment_that_cannot_run_a_bare_program_refuses_before_any_lock() {
    let runtime = FakeRuntime::new(ContainerTrace::off());
    runtime.add_image(IMAGE_ID, None);
    let error = recorded_image_environment(&runtime, &container_policy(Some(IMAGE_ID)))
        .expect_err("an image with no environment has no `PATH`");
    assert!(error.to_string().contains("PATH"), "{error}");

    runtime.set_image_environment(IMAGE_ID, &[("PATH", "bin:/usr/bin")]);
    let error = recorded_image_environment(&runtime, &container_policy(Some(IMAGE_ID)))
        .expect_err("a `PATH` that resolves against the working directory refuses");
    assert!(error.to_string().contains("PATH"), "{error}");
}

#[test]
fn an_absent_recorded_image_or_a_failing_runtime_refuses_and_nothing_else_is_asked() {
    let runtime = FakeRuntime::new(ContainerTrace::recording());
    let error = recorded_image_environment(&runtime, &container_policy(Some(IMAGE_ID)))
        .expect_err("the runtime holds no image with the recorded id");
    assert!(
        error
            .to_string()
            .contains("no longer holds the recorded image id"),
        "{error}"
    );

    let failing = FakeRuntime::new(ContainerTrace::off());
    failing.add_image(IMAGE_ID, None);
    failing.set_failing(RuntimeOp::InspectImageById);
    let error = recorded_image_environment(&failing, &container_policy(Some(IMAGE_ID)))
        .expect_err("a failing inspection refuses");
    assert!(error.to_string().contains("refused"), "{error}");

    let unreachable = FakeRuntime::new(ContainerTrace::off());
    unreachable.set_unreachable(RuntimeOp::InspectImageById);
    let error = recorded_image_environment(&unreachable, &container_policy(Some(IMAGE_ID)))
        .expect_err("an unreachable runtime refuses");
    assert!(error.to_string().contains("cannot be reached"), "{error}");

    let error = recorded_image_environment(&runtime, &container_policy(None))
        .expect_err("a policy without an image identity has no environment to read");
    assert!(error.to_string().contains("image identity"), "{error}");
    assert!(
        runtime
            .calls()
            .iter()
            .all(|op| *op == RuntimeOp::InspectImageById),
        "only the recorded id was inspected: {:?}",
        runtime.calls()
    );
}

fn resumed_with(engine: &str, recorded_width: u32) -> SlotLimits {
    let tree = scratch("entry-resumed-slots");
    let dir = tree.path();
    let config_path = dir.join("upstroke.toml");
    write_file(&config_path, engine.as_bytes());
    let pools = dir.join("pools.toml");
    write_file(&pools, b"# no pools\n");
    let mut warnings = Vec::new();
    let config = config::load_limits(
        Some(&config_path),
        dir,
        Some(&pools),
        EngineLimits::TopologyResume,
        &mut warnings,
    )
    .expect("a schema-4 resume reads the file");
    resumed_slot_limits(&config, recorded_width).expect("non-zero limits")
}

#[test]
fn a_resume_acts_on_the_slot_limits_written_today_and_defaults_the_rest_to_the_recorded_width() {
    let both = resumed_with("[engine]\nmax_per_agent = 1\nmax_per_pool = 1\n", 4);
    assert_eq!(
        (both.per_agent(), both.per_pool()),
        (1, 1),
        "a written limit governs the resume"
    );

    let agent_only = resumed_with("[engine]\nmax_per_agent = 1\n", 4);
    assert_eq!(
        (agent_only.per_agent(), agent_only.per_pool()),
        (1, 4),
        "the absent pool limit is the recorded width, not today's default of 1"
    );

    let pool_only = resumed_with("[engine]\nmax_per_pool = 2\n", 4);
    assert_eq!((pool_only.per_agent(), pool_only.per_pool()), (4, 2));

    let today_narrower = resumed_with("[engine]\nmax_parallel = 1\n", 4);
    assert_eq!(
        (today_narrower.per_agent(), today_narrower.per_pool()),
        (4, 4),
        "today's `max_parallel` is not the run's width, so it is not the slots' default"
    );

    let none = resumed_with("", 3);
    assert_eq!((none.per_agent(), none.per_pool()), (3, 3));
}

/// Root's Q3, and the resume side of D7 (plan addendum, the re-observation at
/// a0): a recorded `case_fold = false` over a namespace that now folds refuses,
/// and nothing else does. A recorded `true` over a case-sensitive root continues
/// on the record and warns, and an observation that cannot answer warns, naming
/// its reason. The record is the answer in every case: nothing here re-resolves
/// it.
#[test]
fn a_recorded_case_sensitive_policy_the_namespace_contradicts_refuses_and_nothing_else_does() {
    let mut warnings = Vec::new();
    let refused = case_policy_holds(
        "01RUN",
        "/r/workspaces/k/01RUN",
        false,
        &CaseObservation::Folds,
        &mut warnings,
    )
    .expect_err("a recorded `case_fold = false` over a namespace that now folds refuses");
    let said = refused.to_string();
    assert!(
        said.contains("case_fold = false") && said.contains("01RUN"),
        "{said}"
    );
    assert!(
        warnings.is_empty(),
        "a refusal is not a warning: {warnings:?}"
    );

    let unknown = CaseObservation::Unknown("TASKS could not be looked up: denied".to_owned());
    let table: [(bool, &CaseObservation, &[&str]); 5] = [
        (false, &CaseObservation::Sensitive, &[]),
        (true, &CaseObservation::Folds, &[]),
        (
            true,
            &CaseObservation::Sensitive,
            &[
                "recorded `case_fold = true`",
                "case-sensitive now",
                "keeps its record",
            ],
        ),
        (
            false,
            &unknown,
            &[
                "could not",
                "TASKS could not be looked up: denied",
                "`case_fold = false`",
            ],
        ),
        (
            true,
            &unknown,
            &[
                "could not",
                "TASKS could not be looked up: denied",
                "`case_fold = true`",
            ],
        ),
    ];
    for (recorded_fold, observed, named) in table {
        let mut warnings = Vec::new();
        case_policy_holds("01RUN", "/r", recorded_fold, observed, &mut warnings).unwrap_or_else(
            |error| panic!("recorded {recorded_fold}, observed {observed:?} refused: {error}"),
        );
        if named.is_empty() {
            assert!(
                warnings.is_empty(),
                "recorded {recorded_fold} agrees with {observed:?}: {warnings:?}"
            );
            continue;
        }
        let [warning] = warnings.as_slice() else {
            panic!("recorded {recorded_fold}, observed {observed:?} warns once: {warnings:?}");
        };
        for part in named {
            assert!(warning.contains(part), "{part}: {warning}");
        }
        assert!(warning.contains("01RUN"), "{warning}");
    }
}

#[test]
fn the_case_observation_answers_only_what_one_directory_shows() {
    let tree = scratch("entry-case-observation");
    let dir = tree.path();
    let absent = observe_case_at(dir, "tasks");
    assert!(
        matches!(&absent, CaseObservation::Unknown(reason) if reason.contains("tasks")),
        "an entry that is not there says nothing, and says why: {absent:?}"
    );
    let unlettered = observe_case_at(dir, "0123");
    assert!(
        matches!(&unlettered, CaseObservation::Unknown(reason) if reason.contains("`0123`")),
        "a name without an ASCII letter has no other case to look up: {unlettered:?}"
    );
    write_file(&dir.join("tasks"), b"an entry\n");
    let observed = observe_case_at(dir, "tasks").folds();
    let upper = dir.join("TASKS");
    let expected = match std::fs::symlink_metadata(&upper) {
        Ok(_) => Some(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Some(false),
        Err(_) => None,
    };
    assert_eq!(
        observed, expected,
        "the observation agrees with a lookup of the other case in the same directory"
    );
}

/// P12-W9 (D7). The whole table from observations to `case_fold`, with injected
/// observations and `core.ignorecase` records: `case_fold = folds OR unknown OR
/// core.ignorecase == true`. An unknown observation folds and warns, naming the
/// directory and the reason; a known-sensitive namespace with `core.ignorecase`
/// false or unset never folds; `core.ignorecase = true` joins the union; a record
/// that could not be read folds and warns only where it decides the answer.
#[test]
fn every_case_observation_and_ignorecase_record_resolves_to_the_planned_case_fold() {
    let root = Path::new("/r/workspaces/k/01RUN");
    let unknown = CaseObservation::Unknown("TASKS could not be looked up: denied".to_owned());
    let unreadable = IgnoreCase::Unreadable("/r/.git/config: denied".to_owned());
    let table: [(&CaseObservation, &IgnoreCase, bool, &[&str]); 12] = [
        (&CaseObservation::Folds, &IgnoreCase::Set(false), true, &[]),
        (&CaseObservation::Folds, &IgnoreCase::Absent, true, &[]),
        (&CaseObservation::Folds, &IgnoreCase::Set(true), true, &[]),
        (&CaseObservation::Folds, &unreadable, true, &[]),
        (
            &CaseObservation::Sensitive,
            &IgnoreCase::Set(false),
            false,
            &[],
        ),
        (&CaseObservation::Sensitive, &IgnoreCase::Absent, false, &[]),
        (
            &CaseObservation::Sensitive,
            &IgnoreCase::Set(true),
            true,
            &[],
        ),
        (
            &CaseObservation::Sensitive,
            &unreadable,
            true,
            &[
                "`core.ignorecase` could not be read",
                "/r/.git/config: denied",
            ],
        ),
        (
            &unknown,
            &IgnoreCase::Set(false),
            true,
            &[
                "/r/workspaces/k/01RUN",
                "TASKS could not be looked up: denied",
            ],
        ),
        (
            &unknown,
            &IgnoreCase::Absent,
            true,
            &[
                "/r/workspaces/k/01RUN",
                "TASKS could not be looked up: denied",
            ],
        ),
        (
            &unknown,
            &IgnoreCase::Set(true),
            true,
            &[
                "/r/workspaces/k/01RUN",
                "TASKS could not be looked up: denied",
            ],
        ),
        (
            &unknown,
            &unreadable,
            true,
            &[
                "/r/workspaces/k/01RUN",
                "TASKS could not be looked up: denied",
            ],
        ),
    ];
    for (observed, ignorecase, expected, named) in table {
        let mut warnings = Vec::new();
        let resolved = resolve_case_fold(root, observed, ignorecase, &mut warnings);
        assert_eq!(
            resolved, expected,
            "{observed:?} with {ignorecase:?} resolves to `case_fold = {expected}`"
        );
        if named.is_empty() {
            assert!(
                warnings.is_empty(),
                "{observed:?} with {ignorecase:?} is a known answer, and says nothing: \
                 {warnings:?}"
            );
            continue;
        }
        let [warning] = warnings.as_slice() else {
            panic!("{observed:?} with {ignorecase:?} warns exactly once: {warnings:?}");
        };
        for part in named {
            assert!(warning.contains(part), "{part}: {warning}");
        }
        assert!(warning.contains("`case_fold = true`"), "{warning}");
    }
}

/// J10 (regression review 2, finding 1): the common directory the read-only halves withhold
/// from configured mounts is the one Git takes. With no record the Git directory is its own;
/// a `commondir` record names it, relative to the Git directory or absolute, without its line
/// ending; an inherited `GIT_COMMON_DIR` wins over the record and names its path at the
/// checkout. A record that cannot be read is an error, never an absence.
#[test]
fn the_common_directory_is_the_inherited_one_at_the_checkout_else_the_git_directorys_record() {
    use std::ffi::OsStr;

    let tree = scratch("effective-common-dir");
    let root = tree.path();
    let checkout = root.join("checkout");
    let git_dir = checkout.join(".git");
    crate::workspace_manager::fixture::create_dir(&git_dir);
    let elsewhere = root.join("common");
    let record = git_dir.join("commondir");
    let common = |inherited: Option<&OsStr>| {
        effective_common_dir(&checkout, &git_dir, inherited)
            .unwrap_or_else(|error| panic!("{inherited:?}: {error}"))
    };

    assert_eq!(
        common(None),
        git_dir,
        "with no record, the Git directory is its own common directory"
    );
    write_file(&record, b"../..\n");
    assert_eq!(
        common(None),
        git_dir.join("../.."),
        "a relative record names a path from the Git directory"
    );
    write_file(&record, format!("{}\r\n", elsewhere.display()).as_bytes());
    assert_eq!(
        common(None),
        elsewhere,
        "an absolute record is taken as written, without its line ending"
    );
    for inherited in [elsewhere.as_os_str(), OsStr::new("../common")] {
        assert_eq!(
            common(Some(inherited)),
            checkout.join(inherited),
            "an inherited value wins over the record, and names its path at the checkout"
        );
    }

    crate::workspace_manager::fixture::remove_file(&record);
    crate::workspace_manager::fixture::create_dir(&record);
    match effective_common_dir(&checkout, &git_dir, None) {
        Err(UpstrokeError::Io { path, .. }) => assert_eq!(path, record),
        other => panic!("a record that cannot be read is an error, not an absence: {other:?}"),
    }
}
