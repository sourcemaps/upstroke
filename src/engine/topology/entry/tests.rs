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
