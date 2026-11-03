//! Extended notes: `docs/internals/runner/container/resolve/tests.md`

// Allowlist placement: the funnel section of `effects/allowlist.toml`, which
// carries this module's review clause. `effect_site_inventory.mechanism` (2).
#![allow(clippy::disallowed_methods, clippy::disallowed_types)]
#![forbid(clippy::disallowed_macros)]

#[cfg(test)]
mod this_file_is_test_only {}

use crate::runner::container::runtime::QuestionBound;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::Duration;

use super::*;
use crate::agent::{AdapterSource, AgentAdapter};
use crate::config::{EngineLimits, RunnerMount, RunnerSelection};
use crate::engine::{ResumeOptions, RunOptions};
use crate::rundir::scratch_tree::ScratchTree;
use crate::runner::container::FakeRuntime;
use crate::runner::container::runtime::{ContainerTrace, Liveness};
use crate::runner::policy::{canonical_bytes, runner_policy_sha256};

const REFERENCE: &str = "upstroke/ci:3.2";
const IMAGE_ID: &str = "sha256:1111111111111111111111111111111111111111111111111111111111111111";
const OTHER_ID: &str = "sha256:2222222222222222222222222222222222222222222222222222222222222222";
const MANIFEST: &str = "sha256:3333333333333333333333333333333333333333333333333333333333333333";
const CLAUDE_VOLUME: &str = "upstroke-creds-claude-code";
const CODEX_VOLUME: &str = "upstroke-creds-codex";

fn volumes() -> BTreeMap<String, String> {
    let mut volumes = BTreeMap::new();
    volumes.insert("claude-code".to_owned(), CLAUDE_VOLUME.to_owned());
    volumes.insert("codex".to_owned(), CODEX_VOLUME.to_owned());
    volumes
}

fn selection() -> RunnerSelection {
    RunnerSelection {
        kind: RunnerKind::Container,
        image: Some(REFERENCE.to_owned()),
        credential_volumes: volumes(),
        mounts: Vec::new(),
        from_config: true,
    }
}

fn ready_runtime() -> (FakeRuntime, ContainerTrace) {
    let trace = ContainerTrace::recording();
    let runtime = FakeRuntime::new(trace.clone());
    runtime.add_image(IMAGE_ID, Some(MANIFEST));
    runtime.tag(REFERENCE, IMAGE_ID);
    runtime.add_volume(CLAUDE_VOLUME);
    runtime.add_volume(CODEX_VOLUME);
    (runtime, trace)
}

fn recorded() -> RunnerPolicy {
    RunnerPolicy {
        kind: RunnerKind::Container,
        policy: RunnerContract::ContainerV1,
        image: Some(ImageIdentity {
            reference: REFERENCE.to_owned(),
            id: IMAGE_ID.to_owned(),
            digest: Some(MANIFEST.to_owned()),
        }),
        credential_volumes: Some(volumes()),
        mounts: Vec::new(),
    }
}

struct RecordingPreflight {
    trace: ContainerTrace,
    calls: Mutex<Vec<(RunnerPolicy, Vec<String>)>>,
    refuse: Option<String>,
}

impl RecordingPreflight {
    fn accepting(trace: ContainerTrace) -> Self {
        Self {
            trace,
            calls: Mutex::new(Vec::new()),
            refuse: None,
        }
    }

    fn refusing(trace: ContainerTrace, message: &str) -> Self {
        Self {
            trace,
            calls: Mutex::new(Vec::new()),
            refuse: Some(message.to_owned()),
        }
    }

    fn calls(&self) -> Vec<(RunnerPolicy, Vec<String>)> {
        self.calls.lock().expect("preflight log").clone()
    }

    fn spawns(&self) -> usize {
        self.calls().len()
    }
}

impl RunnerPreflight for RecordingPreflight {
    fn certify(&self, policy: &RunnerPolicy) -> Result<(), UpstrokeError> {
        self.calls
            .lock()
            .expect("preflight log")
            .push((policy.clone(), self.trace.rendered()));
        match &self.refuse {
            None => Ok(()),
            Some(message) => Err(UpstrokeError::Refused {
                message: message.clone(),
            }),
        }
    }
}

#[test]
fn the_resolved_container_policy_is_the_record_inv23_describes() {
    let (runtime, _trace) = ready_runtime();
    let policy = resolve_container(&runtime, &selection(), &MountContext::unbound())
        .expect("a ready runtime resolves");

    assert_eq!(policy.kind, RunnerKind::Container);
    assert_eq!(policy.policy, RunnerContract::ContainerV1);
    let image = policy.image.as_ref().expect("a container records an image");
    assert_eq!(
        image.reference, REFERENCE,
        "the reference is the operator's, from `[runner] image`"
    );
    assert_eq!(
        image.id, IMAGE_ID,
        "the id is the runtime's immutable answer"
    );
    assert_eq!(
        image.digest.as_deref(),
        Some(MANIFEST),
        "the manifest digest, because this runtime reported one"
    );
    assert_eq!(
        policy.credential_volumes.as_ref(),
        Some(&volumes()),
        "the per-agent credential volume names, verbatim"
    );
    policy
        .completeness()
        .expect("PR3 accepts the record PR6 resolves");
}

#[test]
fn the_recorded_reference_is_the_operators_and_never_the_runtimes() {
    let (runtime, _trace) = ready_runtime();
    runtime.tag("mirror.example/upstroke:latest", IMAGE_ID);
    runtime.tag("aaa-sorts-first/upstroke:1", IMAGE_ID);

    let inspection = runtime
        .image_by_reference(REFERENCE)
        .expect("inspects")
        .expect("present");
    assert_eq!(
        inspection.references.len(),
        3,
        "the fixture does not offer the resolver a choice: {:?}",
        inspection.references
    );
    assert_ne!(
        inspection.references.first().map(String::as_str),
        Some(REFERENCE),
        "the configured reference sorts first, so `references[0]` would accidentally be right"
    );

    let policy =
        resolve_container(&runtime, &selection(), &MountContext::unbound()).expect("resolves");
    assert_eq!(
        policy.image.expect("image").reference,
        REFERENCE,
        "the record took a reference from the runtime's answer"
    );
}

#[test]
fn resolution_issues_only_read_only_operations_in_the_scopes_order() {
    let (runtime, trace) = ready_runtime();
    resolve_container(&runtime, &selection(), &MountContext::unbound()).expect("resolves");

    assert_eq!(
        trace.ops(),
        vec![
            RuntimeOp::Probe,
            RuntimeOp::InspectImageByReference,
            RuntimeOp::InspectVolume,
            RuntimeOp::InspectVolume,
        ],
        "the inspection sequence moved: {:?}",
        trace.rendered()
    );
    assert!(
        trace.ops().iter().all(|op| !op.is_effect()),
        "resolution reached an effectful operation: {:?}",
        trace.rendered()
    );
    let asked: Vec<String> = trace
        .rendered()
        .into_iter()
        .filter(|entry| entry.starts_with("rt:inspect-volume:"))
        .collect();
    assert_eq!(
        asked,
        vec![
            format!("rt:inspect-volume:{CLAUDE_VOLUME}"),
            format!("rt:inspect-volume:{CODEX_VOLUME}"),
        ]
    );
}

#[test]
fn a_runtime_reporting_no_digest_and_one_reporting_an_empty_string_both_resolve_to_none() {
    for (label, reported) in [("absent", None), ("empty string", Some(""))] {
        let trace = ContainerTrace::recording();
        let runtime = FakeRuntime::new(trace);
        runtime.add_image(IMAGE_ID, reported);
        runtime.tag(REFERENCE, IMAGE_ID);
        runtime.add_volume(CLAUDE_VOLUME);
        runtime.add_volume(CODEX_VOLUME);

        let policy =
            resolve_container(&runtime, &selection(), &MountContext::unbound()).expect("resolves");
        assert_eq!(
            policy.image.as_ref().expect("image").digest,
            None,
            "`{label}`: a runtime that reported no usable manifest digest still put one in the \
             record"
        );
        policy
            .completeness()
            .expect("a container without a digest is a complete record");
    }

    let mut absent = recorded();
    let mut empty = recorded();
    absent.image.as_mut().expect("image").digest = None;
    empty.image.as_mut().expect("image").digest = Some(String::new());
    assert_ne!(absent, empty, "the two fixtures are the same value");
    assert_ne!(
        canonical_bytes(&absent),
        canonical_bytes(&empty),
        "`digest: None` and `digest: Some(\"\")` canonicalize alike"
    );
    assert_ne!(
        runner_policy_sha256(&absent),
        runner_policy_sha256(&empty),
        "and the digest INV-23 compares exactly does not separate them either"
    );
}

#[test]
fn resolution_refuses_each_of_its_faults_before_any_lock_or_effect() {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Fault {
        None,
        RuntimeUnreachable,
        RuntimeFails,
        ReferenceAbsent,
        ImageUnidentified,
        VolumeAbsent,
    }

    const FAULTS: &[Fault] = &[
        Fault::None,
        Fault::RuntimeUnreachable,
        Fault::RuntimeFails,
        Fault::ReferenceAbsent,
        Fault::ImageUnidentified,
        Fault::VolumeAbsent,
    ];

    let mut refusals = BTreeSet::new();
    for fault in FAULTS {
        let (runtime, trace) = ready_runtime();
        match fault {
            Fault::None => {}
            Fault::RuntimeUnreachable => runtime.set_unreachable(RuntimeOp::Probe),
            Fault::RuntimeFails => runtime.set_failing(RuntimeOp::InspectImageByReference),
            Fault::ReferenceAbsent => runtime.tag(REFERENCE, "sha256:not-a-real-id"),
            Fault::ImageUnidentified => {
                runtime.add_image("", None);
                runtime.tag(REFERENCE, "");
            }
            Fault::VolumeAbsent => runtime.remove_volume(CODEX_VOLUME),
        }

        let outcome = resolve_container(&runtime, &selection(), &MountContext::unbound());

        assert!(
            trace.ops().iter().all(|op| !op.is_effect()),
            "{fault:?}: resolution reached an effectful operation: {:?}",
            trace.rendered()
        );

        match (fault, outcome) {
            (Fault::None, Ok(policy)) => {
                assert_eq!(policy.image.expect("image").id, IMAGE_ID);
            }
            (Fault::RuntimeUnreachable, Err(refusal)) => {
                assert_eq!(
                    refusal,
                    InspectionRefusal::RuntimeUnavailable {
                        operation: RuntimeOp::Probe,
                        detail: "the fake runtime is armed unreachable for this operation"
                            .to_owned(),
                    }
                );
                assert!(refusal.is_runtime_unavailable());
                assert_eq!(trace.ops(), vec![RuntimeOp::Probe]);
                refusals.insert("runtime unreachable");
            }
            (Fault::RuntimeFails, Err(refusal)) => {
                assert_eq!(
                    refusal.to_string(),
                    "the container runtime refused `inspect-image-by-reference` (the fake \
                     runtime is armed failing for this operation)"
                );
                assert!(
                    refusal.is_runtime_unavailable(),
                    "a runtime that answered with a failure is still a runtime that could not \
                     establish the runner"
                );
                refusals.insert("runtime failed");
            }
            (Fault::ReferenceAbsent, Err(refusal)) => {
                assert_eq!(
                    refusal,
                    InspectionRefusal::ImageReferenceAbsent {
                        reference: REFERENCE.to_owned(),
                    }
                );
                assert!(
                    refusal.to_string().contains("nothing is pulled implicitly"),
                    "the refusal does not say why it is not a fetch: {refusal}"
                );
                assert!(!trace.ops().contains(&RuntimeOp::InspectVolume));
                refusals.insert("reference absent");
            }
            (Fault::ImageUnidentified, Err(refusal)) => {
                assert_eq!(
                    refusal,
                    InspectionRefusal::ImageNotIdentified {
                        reference: REFERENCE.to_owned(),
                    }
                );
                refusals.insert("image unidentified");
            }
            (Fault::VolumeAbsent, Err(refusal)) => {
                assert_eq!(
                    refusal,
                    InspectionRefusal::CredentialVolumeAbsent {
                        agent: "codex".to_owned(),
                        volume: CODEX_VOLUME.to_owned(),
                    },
                    "the refusal must name the agent, not just the volume"
                );
                assert_eq!(
                    trace
                        .rendered()
                        .iter()
                        .filter(|entry| entry.starts_with("rt:inspect-volume:"))
                        .count(),
                    2
                );
                refusals.insert("volume absent");
            }
            (fault, outcome) => panic!("{fault:?} produced {outcome:?}"),
        }
    }
    assert_eq!(
        refusals.len(),
        5,
        "five distinct refusals were driven: {refusals:?}"
    );
}

#[test]
fn the_pre_lock_sequence_reaches_no_lock_no_marker_and_no_probe_when_resolution_refuses() {
    for (label, ready) in [("ready", true), ("no image", false)] {
        let log = SharedLog::default();
        let (fake, _trace) = ready_runtime();
        if !ready {
            fake.tag(REFERENCE, "sha256:not-a-real-id");
        }
        let runtime = LoggingRuntime {
            inner: fake,
            log: log.clone(),
        };

        let resolved = resolve_container(&runtime, &selection(), &MountContext::unbound());
        if resolved.is_ok() {
            log.push("lock:worktree");
            log.push("effect:public-dir");
            log.push("effect:marker");
            log.push("spawn:probe");
        }

        let entries = log.entries();
        let inspections = entries
            .iter()
            .filter(|entry| entry.starts_with("rt:"))
            .count();
        assert_eq!(
            inspections,
            if ready { 4 } else { 2 },
            "{label}: {entries:?}"
        );

        let after: Vec<&String> = entries
            .iter()
            .filter(|entry| !entry.starts_with("rt:"))
            .collect();
        if ready {
            assert_eq!(
                after,
                vec![
                    "lock:worktree",
                    "effect:public-dir",
                    "effect:marker",
                    "spawn:probe"
                ],
                "{label}: {entries:?}"
            );
            let first_after = entries
                .iter()
                .position(|entry| !entry.starts_with("rt:"))
                .expect("the driver ran");
            assert_eq!(
                first_after, inspections,
                "{label}: an inspection happened after the lock: {entries:?}"
            );
        } else {
            assert!(
                after.is_empty(),
                "{label}: a refused resolution still reached {after:?}"
            );
        }
    }
}

#[test]
fn resolution_and_rebuild_ask_different_questions_of_the_runtime() {
    for reference_present in [true, false] {
        for id_present in [true, false] {
            let trace = ContainerTrace::recording();
            let runtime = FakeRuntime::new(trace);
            runtime.add_volume(CLAUDE_VOLUME);
            runtime.add_volume(CODEX_VOLUME);
            if id_present {
                runtime.add_image(IMAGE_ID, Some(MANIFEST));
            }
            if reference_present {
                let target = if id_present { IMAGE_ID } else { OTHER_ID };
                runtime.add_image(target, Some(MANIFEST));
                runtime.tag(REFERENCE, target);
            }

            let cell = format!("reference={reference_present} id={id_present}");
            let resolved = resolve_container(&runtime, &selection(), &MountContext::unbound());
            let mut warnings = Vec::new();
            let rebuilt = rebuild_by_inspection(&runtime, &recorded(), &selection(), &mut warnings);

            assert_eq!(
                resolved.is_ok(),
                reference_present,
                "{cell}: resolution's answer does not track the reference"
            );
            assert_eq!(
                rebuilt.is_ok(),
                id_present,
                "{cell}: the rebuild's answer does not track the recorded id"
            );
            if !reference_present {
                assert_eq!(
                    resolved.expect_err("an absent image reference refuses inspection"),
                    InspectionRefusal::ImageReferenceAbsent {
                        reference: REFERENCE.to_owned()
                    }
                );
            }
            if !id_present {
                assert_eq!(
                    rebuilt.expect_err("an absent image id refuses inspection"),
                    InspectionRefusal::ImageIdAbsent {
                        id: IMAGE_ID.to_owned()
                    }
                );
            }
        }
    }
}

#[test]
fn the_resolved_records_digest_moves_with_the_id_the_runtime_reported() {
    let mut digests = BTreeSet::new();
    let mut ids = BTreeSet::new();
    for id in [IMAGE_ID, OTHER_ID] {
        let trace = ContainerTrace::recording();
        let runtime = FakeRuntime::new(trace);
        runtime.add_image(id, Some(MANIFEST));
        runtime.tag(REFERENCE, id);
        runtime.add_volume(CLAUDE_VOLUME);
        runtime.add_volume(CODEX_VOLUME);
        let policy =
            resolve_container(&runtime, &selection(), &MountContext::unbound()).expect("resolves");
        ids.insert(policy.image.as_ref().expect("image").id.clone());
        digests.insert(runner_policy_sha256(&policy));
    }
    assert_eq!(ids.len(), 2, "the fixture did not vary the id at all");
    assert_eq!(
        digests.len(),
        2,
        "two runners executing different images carry one runner_policy_sha256"
    );
}

#[test]
fn the_only_volume_operation_the_seam_has_is_a_read_only_presence_question() {
    let volume_ops: Vec<RuntimeOp> = RuntimeOp::ALL
        .iter()
        .copied()
        .filter(|op| op.name().contains("volume"))
        .collect();
    assert_eq!(
        volume_ops,
        vec![RuntimeOp::InspectVolume],
        "the seam grew a volume operation, so a run can now create or prune R20"
    );
    assert!(!RuntimeOp::InspectVolume.is_effect());

    let (runtime, trace) = ready_runtime();
    resolve_container(&runtime, &selection(), &MountContext::unbound()).expect("resolves");
    let mut warnings = Vec::new();
    rebuild_by_inspection(&runtime, &recorded(), &selection(), &mut warnings).expect("rebuilds");
    assert_eq!(
        trace
            .ops()
            .iter()
            .filter(|op| op.name().contains("volume"))
            .count(),
        4,
        "two volumes, twice"
    );
    assert!(trace.ops().iter().all(|op| !op.is_effect()));
    assert!(runtime.volume_present(CLAUDE_VOLUME).expect("inspects"));
    assert!(runtime.volume_present(CODEX_VOLUME).expect("inspects"));
}

#[cfg(any(unix, windows))]
struct MountWorld {
    _tree: ScratchTree,
    root: PathBuf,
    repo: PathBuf,
    private: PathBuf,
    credentials: PathBuf,
    shared: PathBuf,
    tool: PathBuf,
}

#[cfg(any(unix, windows))]
fn mount_world(tag: &str) -> MountWorld {
    let tree = scratch(tag);
    let root = fs::canonicalize(tree.path()).expect("the scratch root canonicalizes");
    let repo = root.join("repo");
    fs::create_dir_all(repo.join(".git")).expect("the repository's .git");
    let private = root.join("state").join("private");
    fs::create_dir_all(&private).expect("the private root");
    let credentials = root.join("home").join(".codex");
    fs::create_dir_all(&credentials).expect("a credential location");
    let shared = root.join("shared");
    fs::create_dir_all(&shared).expect("a shared directory");
    let tool = root.join("tool.bin");
    fs::write(&tool, "tool\n").expect("a tool file");
    MountWorld {
        _tree: tree,
        root,
        repo,
        private,
        credentials,
        shared,
        tool,
    }
}

#[cfg(any(unix, windows))]
impl MountWorld {
    fn context(&self) -> MountContext {
        MountContext::bind_with(
            &self.repo,
            fixtures::plain_checkout(&self.repo.join(".git")),
            &self.private,
            std::slice::from_ref(&self.credentials),
        )
        .expect("the world binds a mount context")
    }
}

#[cfg(any(unix, windows))]
fn configured(source: &Path, target: &str, read_only: bool) -> RunnerMount {
    RunnerMount {
        source: source.to_path_buf(),
        target: target.to_owned(),
        read_only,
    }
}

#[cfg(any(unix, windows))]
fn mounted_selection(mounts: Vec<RunnerMount>) -> RunnerSelection {
    RunnerSelection {
        mounts,
        ..selection()
    }
}

#[cfg(unix)]
fn identity_of(path: &Path) -> MountSourceIdentity {
    use std::os::unix::fs::MetadataExt as _;
    let metadata = fs::symlink_metadata(path).expect("the source exists");
    MountSourceIdentity::Unix {
        kind: if metadata.is_dir() {
            crate::topology::events::MountSourceKind::Directory
        } else {
            crate::topology::events::MountSourceKind::File
        },
        device: metadata.dev(),
        inode: metadata.ino(),
    }
}

#[cfg(windows)]
#[test]
fn a_windows_configured_mount_is_recorded_with_its_volume_and_full_file_id_and_rebuilt_from_it() {
    use crate::topology::events::MountSourceKind;

    let world = mount_world("mount-windows");
    let mounted = mounted_selection(vec![
        configured(&world.shared, "/data", true),
        configured(&world.tool, "/opt/tool", false),
    ]);
    let (runtime, _trace) = ready_runtime();
    let record = resolve_container(&runtime, &mounted, &world.context()).expect("resolves");
    assert_eq!(record.completeness(), Ok(()));
    let plain = |path: &Path| {
        let spelled = path.to_str().expect("utf-8");
        spelled.strip_prefix(r"\\?\").unwrap_or(spelled).to_owned()
    };
    assert_eq!(
        record
            .mounts
            .iter()
            .map(|mount| (mount.source.clone(), mount.target.as_str(), mount.read_only))
            .collect::<Vec<_>>(),
        vec![
            (plain(&world.shared), "/data", true),
            (plain(&world.tool), "/opt/tool", false),
        ],
        "the record is the drive-letter spelling, never the verbatim one"
    );
    for (mount, expected) in record
        .mounts
        .iter()
        .zip([MountSourceKind::Directory, MountSourceKind::File])
    {
        match &mount.source_identity {
            MountSourceIdentity::Windows { kind, file_id, .. } => {
                assert_eq!(*kind, expected);
                assert_eq!(file_id.len(), 32, "the full 128-bit file id: {file_id}");
            }
            other => panic!("a Windows build recorded {other:?}"),
        }
    }
    assert!(canonical_bytes(&record).starts_with(b"25:upstroke.runner-policy.v2;"));
    assert_eq!(
        resolve_container(&runtime, &mounted, &world.context()).expect("resolves"),
        record,
        "the same objects are recorded the same way twice"
    );

    let rebuild = || rebuild_by_inspection(&runtime, &record, &mounted, &mut Vec::new());
    assert_eq!(rebuild().expect("rebuilds"), record);
    let away = world.root.join("shared.away");
    let other = world.root.join("other");
    fs::create_dir_all(&other).expect("another directory");
    fs::rename(&world.shared, &away).expect("move the source away");
    match rebuild() {
        Err(InspectionRefusal::MountSourceMoved { reason, .. }) => {
            assert!(reason.contains("is gone"), "{reason}");
        }
        other => panic!("a moved source rebuilt as {other:?}"),
    }
    fs::rename(&other, &world.shared).expect("put another directory at the recorded path");
    match rebuild() {
        Err(InspectionRefusal::MountSourceMoved { reason, .. }) => {
            assert!(reason.contains("names another object"), "{reason}");
        }
        other => panic!("another directory rebuilt as {other:?}"),
    }
    fs::rename(&world.shared, &other).expect("take it back out");
    fs::rename(&away, &world.shared).expect("put the recorded source back");
    assert_eq!(
        rebuild().expect("the restored source re-establishes the record"),
        record
    );
}

#[cfg(unix)]
#[test]
fn a_configured_mount_is_recorded_and_rebuilt_from_the_record() {
    let world = mount_world("mount-recorded");
    let link = world.root.join("link-to-shared");
    std::os::unix::fs::symlink(&world.shared, &link).expect("a link to the shared directory");
    let mounted = mounted_selection(vec![
        configured(&world.tool, "/opt/tool", false),
        configured(&link, "/data", true),
    ]);

    let (runtime, _trace) = ready_runtime();
    let bare = resolve_container(&runtime, &selection(), &world.context()).expect("resolves");
    let with = resolve_container(&runtime, &mounted, &world.context()).expect("resolves");
    assert!(bare.mounts.is_empty());
    assert_eq!(
        with.mounts,
        vec![
            RecordedMount {
                source: world.shared.to_str().expect("utf-8").to_owned(),
                target: "/data".to_owned(),
                read_only: true,
                source_identity: identity_of(&world.shared),
            },
            RecordedMount {
                source: world.tool.to_str().expect("utf-8").to_owned(),
                target: "/opt/tool".to_owned(),
                read_only: false,
                source_identity: identity_of(&world.tool),
            },
        ],
        "the record is the canonical source, the target, the disposition and the identity of \
         each mount, ordered by target"
    );
    assert_eq!(with.completeness(), Ok(()));
    assert_eq!(bare.difference(&with), Some(RunnerField::Mounts));
    assert_ne!(runner_policy_sha256(&bare), runner_policy_sha256(&with));
    assert!(canonical_bytes(&bare).starts_with(b"25:upstroke.runner-policy.v1;"));
    assert!(canonical_bytes(&with).starts_with(b"25:upstroke.runner-policy.v2;"));

    let canonical_spelling = mounted_selection(vec![
        configured(&world.shared, "/data", true),
        configured(&world.tool, "/opt/tool", false),
    ]);
    for (label, today) in [
        ("the configuration it was resolved from", &mounted),
        ("the same mounts spelled canonically", &canonical_spelling),
    ] {
        let mut warnings = Vec::new();
        let rebuilt =
            rebuild_by_inspection(&runtime, &with, today, &mut warnings).expect("rebuilds");
        assert_eq!(rebuilt, with, "`{label}`");
        assert!(
            warnings.is_empty(),
            "`{label}` is the recorded configuration and warned: {warnings:?}"
        );
    }

    let changed: [(&str, RunnerSelection); 5] = [
        (
            "a target moved",
            mounted_selection(vec![
                configured(&link, "/shared", true),
                configured(&world.tool, "/opt/tool", false),
            ]),
        ),
        (
            "made writable",
            mounted_selection(vec![
                configured(&link, "/data", false),
                configured(&world.tool, "/opt/tool", false),
            ]),
        ),
        (
            "another source",
            mounted_selection(vec![
                configured(&world.private, "/data", true),
                configured(&world.tool, "/opt/tool", false),
            ]),
        ),
        (
            "a source that does not exist today",
            mounted_selection(vec![
                configured(&world.root.join("absent"), "/data", true),
                configured(&world.tool, "/opt/tool", false),
            ]),
        ),
        ("no mounts", selection()),
    ];
    for (label, today) in changed {
        let mut warnings = Vec::new();
        let rebuilt =
            rebuild_by_inspection(&runtime, &with, &today, &mut warnings).expect("rebuilds");
        assert_eq!(
            rebuilt, with,
            "`{label}`: today's config reached the rebuilt runner"
        );
        assert_eq!(
            runner_policy_sha256(&rebuilt),
            runner_policy_sha256(&with),
            "`{label}`"
        );
        assert_eq!(warnings.len(), 1, "`{label}`: {warnings:?}");
        assert!(
            warnings[0].contains("configured mount set") && warnings[0].contains("is ignored"),
            "`{label}`: the warning does not name the mount set as ignored: {}",
            warnings[0]
        );
    }
}

#[cfg(any(unix, windows))]
#[test]
fn a_mount_context_withholds_the_git_directories_it_is_given_and_refuses_relative_ones() {
    let world = mount_world("mount-given-git");
    let git_dir = world.repo.join(".git");
    let common = world.root.join("common");
    fs::create_dir_all(common.join("refs")).expect("a common directory outside the checkout");
    let mounted = mounted_selection(vec![configured(&common.join("refs"), "/data", false)]);
    let (runtime, _trace) = ready_runtime();
    resolve_container(&runtime, &mounted, &world.context())
        .expect("the control: beside the checkout's own Git the source is an ordinary directory");

    let relocated = MountContext::bind_with(
        &world.repo,
        RepositoryGit {
            git_dir: &git_dir,
            common_dir: &common,
        },
        &world.private,
        &[],
    )
    .expect("the context binds the directories it is given");
    match resolve_container(&runtime, &mounted, &relocated) {
        Err(InspectionRefusal::MountRefused { reason, .. }) => assert!(
            reason.contains(&format!(
                "lies inside `{}`",
                canonical_spelling(&common).display()
            )) && reason.contains("authoritative Git"),
            "{reason}"
        ),
        other => panic!("a source inside the given common directory resolved as {other:?}"),
    }

    for (git_dir, common_dir) in [
        (Path::new(".git"), common.as_path()),
        (git_dir.as_path(), Path::new("../common")),
    ] {
        let refused = MountContext::bind_with(
            &world.repo,
            RepositoryGit {
                git_dir,
                common_dir,
            },
            &world.private,
            &[],
        )
        .expect_err("a relative Git directory cannot be compared with a source");
        assert!(
            refused.to_string().contains("is not an absolute path"),
            "{refused}"
        );
    }
}

#[cfg(unix)]
#[test]
fn every_configured_mount_that_could_reach_a_withheld_path_is_refused_before_the_runtime_is_asked()
{
    use std::os::unix::ffi::OsStrExt as _;

    let world = mount_world("mount-refusals");
    let linked = world.root.join("linked");
    let main_git = world.root.join("main").join(".git");
    let admin = main_git.join("worktrees").join("linked");
    fs::create_dir_all(&admin).expect("a linked worktree's admin directory");
    fs::create_dir_all(main_git.join("objects")).expect("the common objects");
    fs::write(admin.join("commondir"), "../..\n").expect("commondir");
    fs::create_dir_all(&linked).expect("the linked worktree");
    fs::write(
        linked.join(".git"),
        format!("gitdir: {}\n", admin.display()),
    )
    .expect("gitfile");
    let linked_context = MountContext::bind_with(
        &linked,
        RepositoryGit {
            git_dir: &admin,
            common_dir: &main_git,
        },
        &world.private,
        std::slice::from_ref(&world.credentials),
    )
    .expect("a linked worktree binds");

    fs::create_dir_all(world.repo.join("src")).expect("a directory inside the repository");
    fs::create_dir_all(world.private.join("runs")).expect("a directory inside the private root");
    fs::write(world.credentials.join("config.toml"), "x\n").expect("a credential file");
    let socket = world.root.join("socket");
    let _listener = std::os::unix::net::UnixListener::bind(&socket).expect("a socket");
    let spelled = |name: &[u8]| {
        let path = world.root.join(std::ffi::OsStr::from_bytes(name));
        fs::create_dir_all(&path).expect("an awkwardly named directory");
        path
    };
    // A name that is not valid UTF-8 is the one cell a host may refuse to hold:
    // the macOS leg of CI run 41529099549 refused it with OS error 92, "Illegal
    // byte sequence" (`EILSEQ`). Where the name exists, the source resolves and
    // the late UTF-8 refusal is asserted exactly. Where creating it fails with
    // `EILSEQ`, no such source can exist, and the cell asserts the refusal
    // `resolve_source` gives first, from the same resolution: "does not exist"
    // for a lookup answering `NotFound`, "cannot be resolved" for one answering
    // `EILSEQ`. Any other outcome fails with its kind and code. The label says
    // the late UTF-8 guard was not reached.
    let not_utf8 = world.root.join(std::ffi::OsStr::from_bytes(b"bad\xff"));
    let (not_utf8_label, not_utf8_needle) = match fs::create_dir_all(&not_utf8) {
        Ok(()) => ("not UTF-8", "not valid UTF-8"),
        Err(error) if error.raw_os_error() == Some(libc::EILSEQ) => (
            "not UTF-8, refused by this host with EILSEQ: refused before the UTF-8 guard",
            match fs::canonicalize(&not_utf8) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => "does not exist",
                Err(error) if error.raw_os_error() == Some(libc::EILSEQ) => "cannot be resolved",
                Err(error) => panic!(
                    "the host refused the name with EILSEQ and its lookup failed otherwise: \
                     {:?}, os error {:?}: {error}",
                    error.kind(),
                    error.raw_os_error()
                ),
                Ok(found) => panic!(
                    "the host refused the name with EILSEQ and yet it resolves to {}",
                    found.display()
                ),
            },
        ),
        Err(error) => panic!("an awkwardly named directory: {error}"),
    };

    let context = world.context();
    let unbound = MountContext::unbound();
    let source_cells: Vec<(&str, PathBuf, &MountContext, &str)> = vec![
        (
            "relative",
            PathBuf::from("shared"),
            &context,
            "not an absolute host path",
        ),
        (
            "absent",
            world.root.join("absent"),
            &context,
            "does not exist",
        ),
        (
            "a socket",
            socket.clone(),
            &context,
            "neither a directory nor a regular file",
        ),
        (
            "the repository",
            world.repo.clone(),
            &context,
            "its source is `",
        ),
        (
            "inside the repository",
            world.repo.join("src"),
            &context,
            "lies inside",
        ),
        (
            "the repository's .git",
            world.repo.join(".git"),
            &context,
            "lies inside",
        ),
        ("above everything", world.root.clone(), &context, "contains"),
        (
            "the private root",
            world.private.clone(),
            &context,
            "the private root",
        ),
        (
            "inside the private root",
            world.private.join("runs"),
            &context,
            "lies inside",
        ),
        (
            "above the private root",
            world.root.join("state"),
            &context,
            "contains",
        ),
        (
            "a credential location",
            world.credentials.clone(),
            &context,
            "credential",
        ),
        (
            "above a credential location",
            world.root.join("home"),
            &context,
            "credential",
        ),
        (
            "inside a credential location",
            world.credentials.join("config.toml"),
            &context,
            "credential",
        ),
        (
            "the common Git directory",
            main_git.clone(),
            &linked_context,
            "authoritative Git",
        ),
        (
            "inside common Git",
            main_git.join("objects"),
            &linked_context,
            "authoritative Git",
        ),
        (
            "above common Git",
            world.root.join("main"),
            &linked_context,
            "authoritative Git",
        ),
        (
            "a linked admin directory",
            admin.clone(),
            &linked_context,
            "authoritative Git",
        ),
        ("a comma", spelled(b"a,b"), &context, "contains `,`"),
        ("a quote", spelled(b"q\"uote"), &context, "contains `\"`"),
        (
            "a newline",
            spelled(b"new\nline"),
            &context,
            "control character",
        ),
        (
            "a trailing space",
            spelled(b"trailing "),
            &context,
            "whitespace",
        ),
        (not_utf8_label, not_utf8.clone(), &context, not_utf8_needle),
        (
            "unbound",
            world.shared.clone(),
            &unbound,
            "no repository context",
        ),
    ];
    let target_cells: [(&str, &str); 13] = [
        ("data", "not an absolute path"),
        ("/", "the container's root"),
        ("/data/", "normal form"),
        ("/data//x", "normal form"),
        ("/./data", "normal form"),
        ("/data/../etc", "normal form"),
        ("/upstroke", "reserves"),
        ("/upstroke/cache", "reserves"),
        ("/tmp", "reserves"),
        ("/tmp/cache", "reserves"),
        ("/data,readonly=false", "contains `,`"),
        (" /data", "whitespace"),
        ("", "is empty"),
    ];

    let mut refused = 0_usize;
    let mut expect_refused =
        |label: &str, selection: RunnerSelection, context: &MountContext, needle: &str| {
            let (runtime, trace) = ready_runtime();
            match resolve_container(&runtime, &selection, context) {
                Err(InspectionRefusal::MountRefused { reason, .. }) => assert!(
                    reason.contains(needle),
                    "`{label}` was refused for another reason: {reason}"
                ),
                Err(other) => panic!("`{label}` was refused as {other}"),
                Ok(policy) => panic!("`{label}` was recorded: {:?}", policy.mounts),
            }
            assert!(
                trace.ops().is_empty(),
                "`{label}`: the runtime was asked before the mount was refused: {:?}",
                trace.rendered()
            );
            refused += 1;
        };
    for (label, source, context, needle) in source_cells.iter().cloned() {
        expect_refused(
            label,
            mounted_selection(vec![configured(&source, "/data", true)]),
            context,
            needle,
        );
    }
    for (target, needle) in target_cells {
        expect_refused(
            target,
            mounted_selection(vec![configured(&world.shared, target, true)]),
            &context,
            needle,
        );
    }
    for (label, first, second) in [
        ("a repeated target", "/data", "/data"),
        ("a nested target", "/data", "/data/inner"),
        ("an enclosing target", "/srv/data/inner", "/srv/data"),
    ] {
        expect_refused(
            label,
            mounted_selection(vec![
                configured(&world.shared, first, true),
                configured(&world.tool, second, true),
            ]),
            &context,
            "one holds the other",
        );
    }
    assert_eq!(refused, source_cells.len() + 13 + 3);

    for reserved in [
        BoundaryLayout::DEFAULT_WORKSPACE,
        BoundaryLayout::DEFAULT_CREDENTIALS,
        BoundaryLayout::DEFAULT_GIT_VIEW,
        BoundaryLayout::DEFAULT_GIT_OBJECTS,
        BoundaryLayout::DEFAULT_SETTINGS,
    ] {
        assert!(
            reserved.starts_with(&format!("{BOUNDARY_ROOT}/")),
            "`{reserved}` is a path the runner composes outside the reserved `{BOUNDARY_ROOT}`"
        );
    }

    let (runtime, _trace) = ready_runtime();
    let accepted = resolve_container(
        &runtime,
        &mounted_selection(vec![
            configured(&world.shared, "/upstroked", true),
            configured(&world.tool, "/tmpfile", false),
            configured(&world.repo.join("src"), "/database", true),
        ]),
        &linked_context,
    )
    .expect(
        "a prefix of a reserved target that is not under it, and a directory of another \
         repository's checkout, are accepted",
    );
    assert_eq!(
        accepted
            .mounts
            .iter()
            .map(|mount| mount.target.as_str())
            .collect::<Vec<_>>(),
        vec!["/database", "/tmpfile", "/upstroked"]
    );
}

#[cfg(unix)]
#[test]
fn the_rebuild_refuses_a_missing_or_rebound_source_before_any_spawn_and_resumes_once_it_is_back() {
    let world = mount_world("mount-rebuild");
    let today = mounted_selection(vec![configured(&world.shared, "/data", false)]);
    let (runtime, _trace) = ready_runtime();
    let record = resolve_container(&runtime, &today, &world.context()).expect("resolves");
    let away = world.root.join("shared.away");
    let other = world.root.join("other");
    fs::create_dir_all(&other).expect("another directory");

    struct Attempt {
        label: &'static str,
        outcome: Result<RunnerPolicy, RebuildRefusal>,
        spawns: usize,
        asked: usize,
    }
    fn assert_moved(attempt: Attempt, needle: &str) {
        let label = attempt.label;
        let refusal = attempt.outcome.expect_err(label);
        assert!(refusal.before_any_spawn(), "`{label}`");
        assert_eq!(attempt.spawns, 0, "`{label}`: the preflight spawned");
        assert_eq!(
            attempt.asked, 0,
            "`{label}`: the runtime was asked before the source was checked"
        );
        match refusal {
            RebuildRefusal::Inspection(InspectionRefusal::MountSourceMoved { reason, .. }) => {
                assert!(reason.contains(needle), "`{label}`: {reason}");
            }
            other => panic!("`{label}` was refused as {other}"),
        }
    }
    let rebuild = |label: &'static str| {
        let (runtime, trace) = ready_runtime();
        let preflight = RecordingPreflight::accepting(trace.clone());
        let mut warnings = Vec::new();
        let outcome = rebuild_from_record(&runtime, &record, &today, &preflight, &mut warnings);
        Attempt {
            label,
            outcome,
            spawns: preflight.spawns(),
            asked: trace.ops().len(),
        }
    };

    fs::rename(&world.shared, &away).expect("move the source away");
    assert_moved(rebuild("moved away"), "is gone");
    fs::rename(&other, &world.shared).expect("put another directory at the recorded path");
    assert_moved(rebuild("another directory"), "names another object");
    fs::rename(&world.shared, &other).expect("take it back out");
    std::os::unix::fs::symlink(&away, &world.shared).expect("a link to the moved source");
    assert_moved(rebuild("a link to the source"), "rebound it");
    fs::remove_file(&world.shared).expect("remove the link");

    fs::rename(&away, &world.shared).expect("put the recorded source back");
    let restored = rebuild("restored");
    assert_eq!(
        restored
            .outcome
            .expect("the restored source re-establishes the record"),
        record
    );
    assert_eq!(restored.spawns, 1, "the restored run reaches its preflight");
}

#[test]
fn resolution_refuses_a_selection_that_does_not_ask_for_a_container() {
    let (runtime, trace) = ready_runtime();
    let host = RunnerSelection::host_default();
    assert_eq!(
        resolve_container(&runtime, &host, &MountContext::unbound())
            .expect_err("a host selection is not a container selection"),
        InspectionRefusal::NotAContainerSelection {
            kind: RunnerKind::Host
        }
    );
    let mut imageless = selection();
    imageless.image = None;
    assert_eq!(
        resolve_container(&runtime, &imageless, &MountContext::unbound())
            .expect_err("a selection without an image refuses"),
        InspectionRefusal::SelectionWithoutImage
    );
    assert!(
        trace.ops().is_empty(),
        "a selection guard asked the runtime something: {:?}",
        trace.rendered()
    );
}

#[test]
fn the_rebuild_returns_the_recorded_runner_exactly_however_the_config_differs() {
    let mut moved_reference = selection();
    moved_reference.image = Some("someone/else:9".to_owned());
    let mut fewer_volumes = selection();
    fewer_volumes.credential_volumes.remove("codex");
    let mut host_now = RunnerSelection::host_default();
    host_now.from_config = true;

    let mut unavailable_mount = selection();
    unavailable_mount.mounts = vec![RunnerMount {
        source: PathBuf::from("/nonexistent/upstroke-resolve-test/mount"),
        target: "/data".to_owned(),
        read_only: true,
    }];
    let configs = [
        ("identical", selection()),
        ("moved reference", moved_reference),
        ("fewer volumes", fewer_volumes),
        ("host now", host_now),
        ("absent section", RunnerSelection::host_default()),
        (
            "a newly configured mount that does not exist",
            unavailable_mount,
        ),
    ];
    for (label, today) in configs {
        let (runtime, _trace) = ready_runtime();
        let mut warnings = Vec::new();
        let rebuilt = rebuild_by_inspection(&runtime, &recorded(), &today, &mut warnings)
            .expect("a ready runtime rebuilds");
        assert_eq!(
            rebuilt,
            recorded(),
            "`{label}`: today's config reached the rebuilt runner"
        );
        assert_eq!(
            runner_policy_sha256(&rebuilt),
            runner_policy_sha256(&recorded()),
            "`{label}`: the rebuilt runner carries a different runner_policy_sha256, so \
             run_resumed(4).runner would be a FoldError"
        );
    }
}

#[test]
fn a_config_that_differs_warns_naming_the_field_that_moved() {
    let mut host_now = RunnerSelection::host_default();
    host_now.from_config = true;
    let mut moved_reference = selection();
    moved_reference.image = Some("someone/else:9".to_owned());
    let mut renamed_volume = selection();
    renamed_volume
        .credential_volumes
        .insert("codex".to_owned(), "another-volume".to_owned());
    let mut extra_volume = selection();
    extra_volume
        .credential_volumes
        .insert("copilot".to_owned(), "creds-copilot".to_owned());
    let mut reference_and_volumes = selection();
    reference_and_volumes.image = Some("someone/else:9".to_owned());
    reference_and_volumes.credential_volumes.remove("codex");
    let mut only_reference = selection();
    only_reference.image = Some("someone/else:9".to_owned());
    let mut only_volumes = selection();
    only_volumes.credential_volumes.remove("codex");
    let mut mounted = selection();
    mounted.mounts = vec![RunnerMount {
        source: PathBuf::from("/opt/toolchain"),
        target: "/opt/toolchain".to_owned(),
        read_only: false,
    }];
    assert_eq!(
        configured_difference(&recorded(), &only_reference),
        Some(RunnerField::ImageReference)
    );
    assert_eq!(
        configured_difference(&recorded(), &only_volumes),
        Some(RunnerField::CredentialVolumes)
    );

    let cases: Vec<(&str, RunnerSelection, Option<RunnerField>)> = vec![
        ("identical", selection(), None),
        ("kind", host_now, Some(RunnerField::Kind)),
        (
            "image reference",
            moved_reference,
            Some(RunnerField::ImageReference),
        ),
        (
            "renamed volume",
            renamed_volume,
            Some(RunnerField::CredentialVolumes),
        ),
        (
            "extra volume",
            extra_volume,
            Some(RunnerField::CredentialVolumes),
        ),
        (
            "reference and volumes together",
            reference_and_volumes,
            Some(RunnerField::ImageReference),
        ),
        (
            "a mount configured today",
            mounted,
            Some(RunnerField::Mounts),
        ),
    ];

    let mut named = BTreeSet::new();
    for (label, today, expected) in cases {
        assert_eq!(
            configured_difference(&recorded(), &today),
            expected,
            "`{label}`: the wrong field was named"
        );
        let (runtime, _trace) = ready_runtime();
        let mut warnings = Vec::new();
        rebuild_by_inspection(&runtime, &recorded(), &today, &mut warnings).expect("rebuilds");
        match expected {
            None => assert!(
                warnings.is_empty(),
                "`{label}`: an identical config warned: {warnings:?}"
            ),
            Some(field) => {
                let warning = warnings
                    .iter()
                    .find(|warning| warning.contains("differs from the runner this run recorded"))
                    .unwrap_or_else(|| panic!("`{label}`: no difference warning in {warnings:?}"));
                assert!(
                    warning.contains(&field.to_string()),
                    "`{label}`: the warning does not name `{field}`: {warning}"
                );
                assert!(
                    warning.contains("is ignored"),
                    "`{label}`: the warning does not say the config was ignored: {warning}"
                );
                named.insert(field.to_string());
            }
        }
    }

    assert_eq!(
        named,
        [
            RunnerField::Kind,
            RunnerField::ImageReference,
            RunnerField::CredentialVolumes,
            RunnerField::Mounts,
        ]
        .into_iter()
        .map(|field| field.to_string())
        .collect::<BTreeSet<_>>(),
        "the set of fields a config difference can name has moved"
    );
    for unreachable in [
        RunnerField::Policy,
        RunnerField::ImagePresence,
        RunnerField::ImageId,
        RunnerField::ImageDigest,
    ] {
        assert!(
            !named.contains(&unreachable.to_string()),
            "`{unreachable}` is not a field `upstroke.toml` can move"
        );
    }
}

#[test]
fn an_absent_runner_section_warns_only_when_the_record_is_not_the_default() {
    let mut present_host = RunnerSelection::host_default();
    present_host.from_config = true;
    let absent = RunnerSelection::host_default();
    assert_eq!(
        RunnerSelection {
            from_config: absent.from_config,
            ..present_host.clone()
        },
        absent,
        "the two selections differ by more than `from_config`"
    );

    let host_record = RunnerPolicy {
        kind: RunnerKind::Host,
        policy: RunnerContract::HostV1,
        image: None,
        credential_volumes: None,
        mounts: Vec::new(),
    };

    let cells: Vec<(&str, RunnerPolicy, RunnerSelection, Option<RunnerField>)> = vec![
        (
            "absent section, host record",
            host_record.clone(),
            absent.clone(),
            None,
        ),
        (
            "present host section, host record",
            host_record.clone(),
            present_host.clone(),
            None,
        ),
        (
            "absent section, container record",
            recorded(),
            absent,
            Some(RunnerField::Kind),
        ),
        (
            "present host section, container record",
            recorded(),
            present_host,
            Some(RunnerField::Kind),
        ),
        (
            "present container section, container record",
            recorded(),
            selection(),
            None,
        ),
    ];

    for (label, record, today, expected) in cells {
        assert_eq!(
            configured_difference(&record, &today),
            expected,
            "`{label}`: the wrong field was named"
        );
        let (runtime, _trace) = ready_runtime();
        let mut warnings = Vec::new();
        if record.kind == RunnerKind::Container {
            rebuild_by_inspection(&runtime, &record, &today, &mut warnings).expect("rebuilds");
        }
        let differed = warnings
            .iter()
            .any(|warning| warning.contains("differs from the runner this run recorded"));
        assert_eq!(
            differed,
            expected.is_some() && record.kind == RunnerKind::Container,
            "`{label}`: {warnings:?}"
        );
    }
}

#[test]
fn a_moved_or_vanished_reference_warns_and_the_rebuild_keeps_the_recorded_id() {
    for (label, retag, expect) in [
        ("unchanged", Some(IMAGE_ID), None),
        ("moved", Some(OTHER_ID), Some("now names image")),
        ("vanished", None, Some("no longer resolves")),
    ] {
        let (runtime, trace) = ready_runtime();
        runtime.add_image(OTHER_ID, Some(MANIFEST));
        match retag {
            Some(target) => runtime.move_tag(REFERENCE, target),
            None => runtime.move_tag(REFERENCE, "sha256:nothing-here"),
        }

        let mut warnings = Vec::new();
        let rebuilt = rebuild_by_inspection(&runtime, &recorded(), &selection(), &mut warnings)
            .expect("the recorded id is still present, so the rebuild succeeds");
        assert_eq!(
            rebuilt.image.as_ref().expect("image").id,
            IMAGE_ID,
            "`{label}`: the rebuild took the runtime's current answer instead of the record"
        );
        match expect {
            None => assert!(warnings.is_empty(), "`{label}`: {warnings:?}"),
            Some(needle) => {
                assert_eq!(warnings.len(), 1, "`{label}`: {warnings:?}");
                assert!(
                    warnings[0].contains(needle) && warnings[0].contains(IMAGE_ID),
                    "`{label}`: the warning does not name the recorded id: {}",
                    warnings[0]
                );
            }
        }
        assert!(
            trace.ops().iter().all(|op| !op.is_effect()),
            "`{label}`: the rebuild reached an effectful operation"
        );
    }
}

#[test]
fn the_rebuild_refuses_each_of_its_faults_before_any_spawn() {
    let today = RunnerSelection {
        image: Some("someone/else:9".to_owned()),
        ..selection()
    };
    assert_eq!(
        configured_difference(&recorded(), &today),
        Some(RunnerField::ImageReference),
        "the fixture config does not differ, so `warnings.is_empty()` below is vacuous"
    );
    #[derive(Debug, Clone, Copy)]
    enum Fault {
        None,
        RuntimeUnavailable,
        RecordedIdAbsent,
        VolumeAbsent,
    }
    const FAULTS: &[Fault] = &[
        Fault::None,
        Fault::RuntimeUnavailable,
        Fault::RecordedIdAbsent,
        Fault::VolumeAbsent,
    ];

    let mut refusals = BTreeSet::new();
    for fault in FAULTS {
        let (runtime, trace) = ready_runtime();
        match fault {
            Fault::None => {}
            Fault::RuntimeUnavailable => runtime.set_all_unreachable(),
            Fault::RecordedIdAbsent => {
                runtime.add_image(OTHER_ID, Some(MANIFEST));
                runtime.move_tag(REFERENCE, OTHER_ID);
                let trace2 = ContainerTrace::recording();
                let replacement = FakeRuntime::new(trace2);
                replacement.add_image(OTHER_ID, Some(MANIFEST));
                replacement.tag(REFERENCE, OTHER_ID);
                replacement.add_volume(CLAUDE_VOLUME);
                replacement.add_volume(CODEX_VOLUME);
                let preflight = RecordingPreflight::accepting(ContainerTrace::off());
                let mut warnings = Vec::new();
                let outcome = rebuild_from_record(
                    &replacement,
                    &recorded(),
                    &today,
                    &preflight,
                    &mut warnings,
                );
                let refusal = outcome.expect_err("the recorded id is absent");
                assert!(refusal.before_any_spawn());
                assert_eq!(preflight.spawns(), 0);
                assert!(
                    warnings.is_empty(),
                    "a refused rebuild warned: {warnings:?}"
                );
                assert!(matches!(
                    refusal,
                    RebuildRefusal::Inspection(InspectionRefusal::ImageIdAbsent { .. })
                ));
                refusals.insert("recorded id absent");
                continue;
            }
            Fault::VolumeAbsent => runtime.remove_volume(CLAUDE_VOLUME),
        }

        let preflight = RecordingPreflight::accepting(trace.clone());
        let mut warnings = Vec::new();
        let outcome = rebuild_from_record(&runtime, &recorded(), &today, &preflight, &mut warnings);

        match (fault, outcome) {
            (Fault::None, Ok(rebuilt)) => {
                assert_eq!(rebuilt, recorded());
                assert_eq!(preflight.spawns(), 1, "the control never spawned");
                assert!(
                    warnings.iter().any(
                        |warning| warning.contains("differs from the runner this run recorded")
                    ),
                    "the differing config did not warn on the successful cell, so the empty \
                     warning lists on the refused cells prove nothing: {warnings:?}"
                );
            }
            (Fault::None, Err(error)) => panic!("the control refused: {error}"),
            (fault, Ok(_)) => panic!("{fault:?} did not refuse"),
            (fault, Err(refusal)) => {
                assert!(
                    refusal.before_any_spawn(),
                    "{fault:?} classified itself as a probe-observed refusal"
                );
                assert_eq!(
                    preflight.spawns(),
                    0,
                    "{fault:?} spawned before refusing: {:?}",
                    trace.rendered()
                );
                assert!(
                    warnings.is_empty(),
                    "{fault:?}: a refused rebuild warned: {warnings:?}"
                );
                match (fault, &refusal) {
                    (
                        Fault::RuntimeUnavailable,
                        RebuildRefusal::Inspection(InspectionRefusal::RuntimeUnavailable {
                            operation,
                            ..
                        }),
                    ) => {
                        assert_eq!(*operation, RuntimeOp::Probe);
                        refusals.insert("runtime unavailable");
                    }
                    (
                        Fault::VolumeAbsent,
                        RebuildRefusal::Inspection(InspectionRefusal::CredentialVolumeAbsent {
                            agent,
                            volume,
                        }),
                    ) => {
                        assert_eq!(agent, "claude-code");
                        assert_eq!(volume, CLAUDE_VOLUME);
                        refusals.insert("volume absent");
                    }
                    (fault, refusal) => panic!("{fault:?} produced {refusal}"),
                }
            }
        }
    }
    assert_eq!(refusals.len(), 3, "three distinct refusals: {refusals:?}");
}

#[test]
fn a_failing_preflight_probe_refuses_after_every_inspection_and_only_a_spawn_observes_it() {
    for (label, message) in [
        (
            "shell",
            "pre-flight: the recorded shell `sh` exited 127 inside the recorded image",
        ),
        (
            "agent CLI",
            "pre-flight: `claude` is not on PATH inside the recorded image",
        ),
    ] {
        let (runtime, trace) = ready_runtime();
        let preflight = RecordingPreflight::refusing(trace.clone(), message);
        let mut warnings = Vec::new();
        let refusal = rebuild_from_record(
            &runtime,
            &recorded(),
            &selection(),
            &preflight,
            &mut warnings,
        )
        .expect_err("a probe that fails inside the image refuses");

        assert!(
            !refusal.before_any_spawn(),
            "`{label}`: a probe-observed refusal claimed it happened before any spawn"
        );
        assert!(matches!(refusal, RebuildRefusal::Preflight(_)));
        assert!(refusal.to_string().contains(message), "{refusal}");

        let calls = preflight.calls();
        assert_eq!(calls.len(), 1, "`{label}`: the probe did not run");
        let (certified, prefix) = &calls[0];
        assert_eq!(
            certified,
            &recorded(),
            "`{label}`: the probe certified something other than the rebuilt record"
        );
        let ops: Vec<&String> = prefix
            .iter()
            .filter(|entry| entry.starts_with("rt:"))
            .collect();
        assert_eq!(
            ops.len(),
            5,
            "`{label}`: the probe ran before the inspections finished: {prefix:?}"
        );
        assert!(
            ops[0].starts_with("rt:probe:")
                && ops[1].starts_with("rt:inspect-image-by-id:")
                && ops[2].starts_with("rt:inspect-volume:")
                && ops[3].starts_with("rt:inspect-volume:")
                && ops[4].starts_with("rt:inspect-image-by-reference:"),
            "`{label}`: the inspection prefix is not the rebuild's: {ops:?}"
        );
        assert!(trace.ops().iter().all(|op| !op.is_effect()));
    }
}

#[test]
fn the_rebuild_refuses_an_incomplete_record_before_asking_the_runtime_anything() {
    let mut without_volumes = recorded();
    without_volumes.credential_volumes = None;
    let (runtime, trace) = ready_runtime();
    let mut warnings = Vec::new();
    assert_eq!(
        rebuild_by_inspection(&runtime, &without_volumes, &selection(), &mut warnings)
            .expect_err("a container record without credential volumes is incomplete"),
        InspectionRefusal::RecordIncomplete(RunnerRecordDefect::ContainerWithoutCredentialVolumes)
    );
    let host = crate::runner::policy::host_policy();
    assert_eq!(
        rebuild_by_inspection(&runtime, &host, &selection(), &mut warnings)
            .expect_err("a host record cannot be rebuilt as a container"),
        InspectionRefusal::NotAContainerSelection {
            kind: RunnerKind::Host
        }
    );
    assert!(
        trace.ops().is_empty(),
        "a record guard asked the runtime something: {:?}",
        trace.rendered()
    );
}

#[test]
fn legacy_container_selection_refused_before_effects() {
    const CONTAINER_TOML: &str = "container";
    const HOST_TOML: &str = "host";

    for kind in [HOST_TOML, CONTAINER_TOML] {
        let (_tree, repo) = temp_repo(&format!("legacy-{kind}"));
        let private = repo.join("private");
        fs::create_dir_all(&private).expect("private root");
        let config = if kind == HOST_TOML {
            "[runner]\nkind = \"host\"\n".to_owned()
        } else {
            format!(
                "[runner]\nkind = \"container\"\nimage = \"{REFERENCE}\"\n\
                 credential_volumes = {{ claude-code = \"{CLAUDE_VOLUME}\" }}\n"
            )
        };
        fs::write(repo.join("upstroke.toml"), &config).expect("config");
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "-q", "-m", "config"]);
        let seeded: Vec<(u32, String)> = [1_u32, 2, 3]
            .into_iter()
            .map(|schema| {
                let run_id = format!("01ARZ3NDEKTSV4RRFFQ69G5FA{schema}");
                seed_legacy_run(&repo, &run_id, &private, schema);
                (schema, run_id)
            })
            .collect();

        let run_opts = || {
            let mut opts = RunOptions::new(repo.join("plan.md"), repo.clone());
            opts.pools_path = Some(empty_pools(&repo));
            opts.private_root = Some(private.clone());
            opts.wait_on_block = Some(Duration::ZERO);
            opts.defer_backoff = Duration::ZERO;
            opts.legacy_writer = true;
            opts
        };
        let resume_opts = |run_id: &str| {
            let mut opts = ResumeOptions::new(run_id.to_owned(), repo.clone());
            opts.pools_path = Some(empty_pools(&repo));
            opts.private_root = Some(private.clone());
            opts.wait_on_block = Some(Duration::ZERO);
            opts.defer_backoff = Duration::ZERO;
            opts
        };

        {
            let _lease = crate::rundir::WorktreeLock::acquire(&repo).expect("the test's lease");
            let adapters = RecordingAdapters::default();
            let mut outcomes = vec![(
                "a run by the schema-1..3 writer".to_owned(),
                crate::engine::run_with(&run_opts(), &adapters),
            )];
            for (schema, run_id) in &seeded {
                outcomes.push((
                    format!("resume of a schema-{schema} run"),
                    crate::engine::resume_with(&resume_opts(run_id), &adapters),
                ));
            }
            for (command, outcome) in outcomes {
                let error = outcome.expect_err("the lease is held, so nothing can proceed");
                if kind == HOST_TOML {
                    assert!(
                        matches!(&error, UpstrokeError::Refused { message }
                            if message.contains("already driving worktree")),
                        "{command}: the control did not reach the worktree lock, so the \
                         container cell's failure to reach it proves nothing: {error}"
                    );
                } else {
                    let UpstrokeError::Config { message, .. } = &error else {
                        panic!(
                            "{command}: refused as {error:?} — a container selection reached the \
                             worktree lock before it was refused"
                        );
                    };
                    assert!(
                        message.contains("[runner] `kind = \"container\"` is refused"),
                        "{command}: {message}"
                    );
                    assert!(
                        message.contains("no owner run") && message.contains("run.lock"),
                        "{command}: the refusal does not say why a late refusal would be \
                         broken: {message}"
                    );
                }
            }
        }

        let adapters = RecordingAdapters::default();
        let run = crate::engine::run_with(&run_opts(), &adapters);
        assert!(run.is_err(), "the fixture has no agents");
        for (_, run_id) in &seeded {
            assert!(
                crate::engine::resume_with(&resume_opts(run_id), &adapters).is_err(),
                "the fixture has no agents"
            );
        }

        if kind == HOST_TOML {
            assert!(
                !adapters.asked().is_empty(),
                "the control never reached pre-flight, so the container cell's empty adapter \
                 log proves nothing"
            );
            continue;
        }

        assert!(
            adapters.asked().is_empty(),
            "an adapter was resolved before the refusal ({:?}), so a pre-flight probe could \
             have been spawned",
            adapters.asked()
        );
        let runs = repo.join(".upstroke").join("runs");
        let mut ids: Vec<String> = fs::read_dir(&runs)
            .expect("runs root")
            .map(|entry| entry.expect("entry").file_name().to_string_lossy().into())
            .collect();
        ids.sort();
        let mut seeded_ids: Vec<String> = seeded.iter().map(|(_, run_id)| run_id.clone()).collect();
        seeded_ids.sort();
        assert_eq!(
            ids, seeded_ids,
            "the refused fresh run created a run directory"
        );
        for (schema, run_id) in &seeded {
            assert!(
                !runs.join(run_id).join("run.lock").exists(),
                "the refused schema-{schema} resume left a run.lock behind"
            );
        }
        assert!(
            !private.join("runs").exists(),
            "the refused commands created a private run directory"
        );
        assert!(
            !private.join("containers").exists(),
            "a legacy command wrote something into the container intent namespace"
        );
        let branches = String::from_utf8(
            Command::new("git")
                .arg("-C")
                .arg(&repo)
                .args(["branch", "--list", "upstroke/*"])
                .output()
                .expect("git branch")
                .stdout,
        )
        .expect("utf-8");
        assert!(
            branches.trim().is_empty(),
            "the refused run created a branch: {branches}"
        );
    }
}

fn schema_4_run_options(repo: &Path, private: &Path) -> RunOptions {
    let mut opts = RunOptions::new(repo.join("plan.md"), repo.to_path_buf());
    opts.pools_path = Some(empty_pools(repo));
    opts.private_root = Some(private.to_path_buf());
    opts.wait_on_block = Some(Duration::ZERO);
    opts.defer_backoff = Duration::ZERO;
    opts.legacy_writer = false;
    opts
}

#[test]
fn a_schema_4_host_run_under_a_held_lease_is_refused_by_the_worktree_lock() {
    let (_tree, repo) = temp_repo("schema4-host-control");
    let private = repo.join("private");
    fs::create_dir_all(&private).expect("private root");
    fs::write(repo.join("upstroke.toml"), "[runner]\nkind = \"host\"\n").expect("config");
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "config"]);
    let opts = schema_4_run_options(&repo, &private);
    let adapters = RecordingAdapters::default();
    {
        let _lease = crate::rundir::WorktreeLock::acquire(&repo).expect("the test's lease");
        let error = crate::engine::run_with(&opts, &adapters)
            .expect_err("the lease is held, so nothing can proceed");
        assert!(
            matches!(&error, UpstrokeError::Refused { message }
                if message.contains("already driving worktree")),
            "the schema-4 control did not reach the worktree lock: {error}"
        );
    }
    assert!(
        crate::engine::run_with(&opts, &adapters).is_err(),
        "the fixture has no agents"
    );
    assert!(
        !adapters.asked().is_empty(),
        "the control never reached pre-flight, so the container cell's empty adapter log \
         proves nothing"
    );
}

#[test]
fn real_docker_a_schema_4_run_refuses_an_absent_image_before_the_worktree_lock() {
    let trace = ContainerTrace::recording();
    let docker = match crate::runner::container::fake::docker_gate(
        "real_docker_a_schema_4_run_refuses_an_absent_image_before_the_worktree_lock",
        trace,
    ) {
        Ok(docker) => docker,
        Err(reason) => {
            assert_eq!(
                reason,
                crate::runner::container::fake::absent_reason(),
                "a Docker-gated test skipped for a reason the gate does not know about"
            );
            return;
        }
    };
    let nonce = format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("the clock is after the epoch")
            .as_nanos()
    );
    let reference = format!("upstroke-pr12-absent/r{nonce}:never");
    let volume = format!("upstroke-pr12-absent-{nonce}");
    assert!(
        docker
            .image_by_reference(&reference)
            .expect("a read-only inspection of the controlled reference")
            .is_none(),
        "the fixture's premise: the runtime does not hold `{reference}`"
    );

    let (_tree, repo) = temp_repo("schema4-absent-image");
    let private = repo.join("private");
    fs::create_dir_all(&private).expect("private root");
    fs::write(
        repo.join("upstroke.toml"),
        format!(
            "[runner]\nkind = \"container\"\nimage = \"{reference}\"\n\
             credential_volumes = {{ claude-code = \"{volume}\" }}\n"
        ),
    )
    .expect("config");
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "config"]);
    let opts = schema_4_run_options(&repo, &private);
    let adapters = RecordingAdapters::default();
    {
        let _lease = crate::rundir::WorktreeLock::acquire(&repo).expect("the test's lease");
        let error =
            crate::engine::run_with(&opts, &adapters).expect_err("an absent image refuses the run");
        let UpstrokeError::Refused { message } = &error else {
            panic!("a schema-4 run refused as {error:?}, not by its pre-lock inspection");
        };
        assert!(
            !message.contains("already driving worktree"),
            "a schema-4 run reached the worktree lock before resolving its runner: {message}"
        );
        assert!(
            message.contains(&reference) && message.contains("does not hold the image reference"),
            "the refusal names the absent reference: {message}"
        );
    }
    assert!(
        adapters.asked().is_empty(),
        "an adapter was resolved before the refusal: {:?}",
        adapters.asked()
    );
    assert!(
        docker
            .image_by_reference(&reference)
            .expect("a read-only inspection of the controlled reference")
            .is_none(),
        "the refused run left `{reference}` held by the runtime"
    );
    let runs = repo.join(".upstroke").join("runs");
    assert!(
        !runs.exists() || fs::read_dir(&runs).expect("runs root").next().is_none(),
        "the refused run created a run directory"
    );
    assert!(
        !private.join("runs").exists(),
        "the refused run created a private run directory"
    );
}

#[test]
fn every_engine_limits_reading_is_classified_for_a_container_selection() {
    let all = [
        EngineLimits::Fresh,
        EngineLimits::Topology,
        EngineLimits::TopologyResume,
        EngineLimits::SequentialResume,
        EngineLimits::SequentialResumeWithRecordedGates,
    ];
    let selects = |limits: EngineLimits| match limits {
        EngineLimits::Topology | EngineLimits::TopologyResume => true,
        EngineLimits::Fresh
        | EngineLimits::SequentialResume
        | EngineLimits::SequentialResumeWithRecordedGates => false,
    };

    let tree = scratch("engine-limits");
    let dir = tree.path();
    let mut refused = 0;
    let mut selected = 0;
    for limits in all {
        fs::write(
            dir.join("upstroke.toml"),
            format!("[runner]\nkind = \"container\"\nimage = \"{REFERENCE}\"\n"),
        )
        .expect("config");
        let mut warnings = Vec::new();
        let loaded = crate::config::load_limits(
            Some(&dir.join("upstroke.toml")),
            dir,
            Some(&empty_pools(dir)),
            limits,
            &mut warnings,
        );
        if selects(limits) {
            let config = loaded.expect("a schema-4 reading selects the container boundary");
            assert_eq!(config.runner.kind, RunnerKind::Container, "{limits:?}");
            selected += 1;
        } else {
            let error = loaded.expect_err("a sequential reading refuses a container selection");
            assert!(
                matches!(&error, UpstrokeError::Config { message, .. }
                    if message.contains("[runner] `kind = \"container\"` is refused")),
                "{limits:?}: {error}"
            );
            refused += 1;
        }

        fs::write(dir.join("upstroke.toml"), "[runner]\nkind = \"host\"\n").expect("config");
        let config = crate::config::load_limits(
            Some(&dir.join("upstroke.toml")),
            dir,
            Some(&empty_pools(dir)),
            limits,
            &mut warnings,
        )
        .expect("a host selection loads");
        assert_eq!(config.runner.kind, RunnerKind::Host);
        assert!(config.runner.from_config);
    }
    assert_eq!(refused, 3, "every sequential reading was driven");
    assert_eq!(selected, 2, "both schema-4 readings were driven");
}

#[test]
fn no_module_outside_the_container_runner_writes_a_container_intent() {
    const WRITERS: &[&str] = &[
        "ContainerIntent",
        "ContainerName",
        "containers_dir",
        "CONTAINERS_DIR",
        "container::write_intent",
    ];
    const ALLOWED: &[(&str, &str)] = &[
        ("src/runner/container.rs", "the funnel that writes them"),
        ("src/runner/container/intent.rs", "the record itself"),
        (
            "src/runner/container/census.rs",
            "the census that reclaims them",
        ),
        (
            "src/runner/container/exec.rs",
            "the ContainerRunner that owns an invocation",
        ),
        (
            "src/runner/container/exec/tests.rs",
            "that runner's own suite, out of line since W1. It is ALLOWED and \
             not EXCLUDED because this census scans the whole file: the text \
             was inside `exec.rs` and was scanned there, and it is the only \
             place `container::write_intent` matches, so excluding it would \
             leave needle control (b) matching nothing",
        ),
    ];
    const EXCLUDED: &[&str] = &[
        "src/effects/tests.rs",
        "src/engine/topology/coverage/tests.rs",
        "src/engine/topology/create/tests.rs",
        "src/engine/topology/recover/tests.rs",
        "src/runner/container/census/tests.rs",
        "src/runner/container/fake.rs",
        "src/runner/container/resolve/tests.rs",
        "src/runner/container/tests.rs",
    ];

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let allowed: BTreeSet<&str> = ALLOWED.iter().map(|(path, _)| *path).collect();
    for excluded in EXCLUDED {
        assert!(
            root.join(excluded).is_file(),
            "`{excluded}` is excluded from this census and names no file; an exclusion that \
             matches nothing today is one that matches whatever is created at that path \
             tomorrow"
        );
    }
    let mut sorted = EXCLUDED.to_vec();
    sorted.sort_unstable();
    assert_eq!(
        EXCLUDED,
        &sorted[..],
        "`EXCLUDED` is read by eye, so it is written sorted"
    );
    let mut offenders = Vec::new();
    let mut found = BTreeSet::new();
    let mut matched_needles: BTreeSet<&str> = BTreeSet::new();
    let mut scanned = 0;
    for path in rust_sources(&root.join("src")) {
        let relative = path
            .strip_prefix(&root)
            .expect("under the manifest")
            .to_string_lossy()
            .replace('\\', "/");
        if EXCLUDED.contains(&relative.as_str()) {
            continue;
        }
        let source = fs::read_to_string(&path).expect("read source");
        let scanned_text = crate::effects::blank_comments_and_strings(&source);
        scanned += 1;
        for writer in WRITERS {
            if scanned_text.contains(writer) {
                if allowed.contains(relative.as_str()) {
                    found.insert(relative.clone());
                    matched_needles.insert(writer);
                } else {
                    offenders.push(format!("{relative} names `{writer}`"));
                }
            }
        }
        if relative == "src/engine/coordinator.rs" {
            assert!(
                scanned_text.contains("fn run_harness_inner_on"),
                "the census does not reach the legacy coordinator's body, so it cannot hold a \
                 claim about what a legacy process writes"
            );
        }
    }
    assert!(scanned > 20, "the walk found the tree: {scanned}");
    assert!(
        offenders.is_empty(),
        "a module outside the container runner can write a container intent, so a legacy \
         process could own a container with no run identity behind it: {offenders:#?}"
    );
    assert_eq!(
        found.len(),
        ALLOWED.len(),
        "the census found {found:?} of {ALLOWED:?}, so it is not looking at what it claims to"
    );
    assert_eq!(
        matched_needles,
        WRITERS.iter().copied().collect::<BTreeSet<_>>(),
        "a `WRITERS` needle matches nothing in the allowed files, so the prohibition it \
         encodes is not being enforced on anything"
    );
}

#[test]
fn the_resolution_module_names_no_lock_no_write_and_no_spawn() {
    const FORBIDDEN: &[&str] = &[
        "WorktreeLock",
        "RunLock",
        "File::",
        "Command::",
        ".spawn(",
        "runtime.create(",
        "runtime.start(",
        "runtime.stop(",
        "runtime.remove(",
        "create_dir",
        "write(",
        "GENERIC_WRITE",
        "GENERIC_ALL",
        "FILE_GENERIC_WRITE",
        "FILE_WRITE_",
        "FILE_APPEND_DATA",
        "CREATE_NEW",
        "CREATE_ALWAYS",
        "OPEN_ALWAYS",
        "TRUNCATE_EXISTING",
        "DELETE_ON_CLOSE",
        "WRITE_DAC",
        "WRITE_OWNER",
    ];
    let source = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/runner/container/resolve.rs"),
    )
    .expect("this module");
    let production =
        crate::effects::blank_comments_and_strings(&crate::effects::production_region(&source));
    let named: Vec<&str> = FORBIDDEN
        .iter()
        .copied()
        .filter(|needle| production.contains(needle))
        .collect();
    assert!(
        named.is_empty(),
        "resolution reaches {named:?}, so it is no longer read-only by construction"
    );
    const READ_ONLY_FS: &[&str] = &[
        "fs::canonicalize(",
        "fs::symlink_metadata(",
        "fs::MetadataExt",
    ];
    let observations: usize = READ_ONLY_FS
        .iter()
        .map(|call| production.matches(call).count())
        .sum();
    assert_eq!(
        production.matches("fs::").count(),
        observations,
        "resolution names an `fs::` item other than the read-only observations \
         {READ_ONLY_FS:?} that mount resolution and its recheck need"
    );
    assert!(
        observations > 0,
        "the read-only observations are gone, so the allowance above names nothing"
    );
    for call in [
        "CreateFileW(",
        "GetFileInformationByHandle(",
        "GetFileInformationByHandleEx(",
    ] {
        assert_eq!(
            production.matches(call).count(),
            1,
            "the Windows mount identity is one read-attributes open and two reads; `{call}` \
             is not named exactly once"
        );
    }
    for flag in [
        "FILE_READ_ATTRIBUTES",
        "OPEN_EXISTING",
        "FILE_FLAG_OPEN_REPARSE_POINT",
    ] {
        assert!(
            production.contains(flag),
            "the Windows open does not name `{flag}`, so it is not the read-only, \
             existing-object, link-refusing open this census admits"
        );
    }
    assert!(
        production.contains("fn resolve_container"),
        "the production region is empty, so the census above proves nothing"
    );
}

#[derive(Debug, Clone, Default)]
struct SharedLog(std::sync::Arc<Mutex<Vec<String>>>);

impl SharedLog {
    fn push(&self, entry: &str) {
        self.0.lock().expect("log").push(entry.to_owned());
    }

    fn entries(&self) -> Vec<String> {
        self.0.lock().expect("log").clone()
    }
}

struct LoggingRuntime {
    inner: FakeRuntime,
    log: SharedLog,
}

macro_rules! logged {
    ($self:ident, $op:expr) => {{
        $self.log.push(&format!("rt:{}", $op.name()));
    }};
}

impl ContainerRuntime for LoggingRuntime {
    fn probe(&self) -> Result<(), RuntimeError> {
        logged!(self, RuntimeOp::Probe);
        self.inner.probe()
    }

    fn image_by_reference(&self, reference: &str) -> Result<Option<ImageInspection>, RuntimeError> {
        logged!(self, RuntimeOp::InspectImageByReference);
        self.inner.image_by_reference(reference)
    }

    fn image_by_id(&self, id: &str) -> Result<Option<ImageInspection>, RuntimeError> {
        logged!(self, RuntimeOp::InspectImageById);
        self.inner.image_by_id(id)
    }

    fn volume_present(&self, name: &str) -> Result<bool, RuntimeError> {
        logged!(self, RuntimeOp::InspectVolume);
        self.inner.volume_present(name)
    }

    fn containers_with_label(
        &self,
        key: &str,
        value: &str,
    ) -> Result<Vec<crate::runner::container::runtime::DiscoveredContainer>, RuntimeError> {
        logged!(self, RuntimeOp::ListByLabel);
        self.inner.containers_with_label(key, value)
    }

    fn observe(&self, name: &str, bound: QuestionBound<'_>) -> Result<Liveness, RuntimeError> {
        logged!(self, RuntimeOp::Observe);
        self.inner.observe(name, bound)
    }

    fn collect(
        &self,
        name: &str,
    ) -> Result<crate::runner::container::runtime::ContainerExecution, RuntimeError> {
        logged!(self, RuntimeOp::Collect);
        self.inner.collect(name)
    }

    fn create(
        &self,
        covered: crate::runner::container::CoveredCreate<'_>,
    ) -> Result<crate::runner::container::runtime::CreatedContainer, RuntimeError> {
        logged!(self, RuntimeOp::Create);
        self.inner.create(covered)
    }

    fn start(
        &self,
        covered: crate::runner::container::CoveredStart<'_>,
    ) -> Result<(), RuntimeError> {
        logged!(self, RuntimeOp::Start);
        self.inner.start(covered)
    }

    fn stop(
        &self,
        name: &str,
        mode: crate::runner::container::runtime::StopMode,
    ) -> Result<crate::runner::container::runtime::Settled, RuntimeError> {
        logged!(self, RuntimeOp::Stop);
        self.inner.stop(name, mode)
    }

    fn remove(
        &self,
        name: &str,
    ) -> Result<crate::runner::container::runtime::Settled, RuntimeError> {
        logged!(self, RuntimeOp::Remove);
        self.inner.remove(name)
    }
}

#[derive(Default)]
struct RecordingAdapters {
    asked: Mutex<Vec<String>>,
}

impl RecordingAdapters {
    fn asked(&self) -> Vec<String> {
        self.asked.lock().expect("adapter log").clone()
    }
}

impl AdapterSource for RecordingAdapters {
    fn get(&self, id: &str) -> Option<&dyn AgentAdapter> {
        self.asked.lock().expect("adapter log").push(id.to_owned());
        None
    }
}

/// A scratch tree for one test, guarded by the token that authorises its
/// deletion.
///
/// The helper here built `temp_dir()/upstroke-resolve-<tag>-<pid>-<thread>` and pre-cleaned it with a
/// discarded `remove_dir_all` before it had any claim on the name, then
/// returned a root nothing reclaimed (`PR64-CLEANUP-003-SCRATCH-PRECLEAN`,
/// `PR7-SCRATCH-FIXTURE-LEAK`). A thread id distinguishes two fixtures inside
/// one process and nothing else: a second process, or a second run of this one
/// under a pid Windows recycled, reaches the same name. `acquire` refuses an
/// occupied root rather than emptying it, keys on a ULID no recycled pid can
/// supply, and reclaims on drop.
///
/// **Bind the guard to a live local**: `let _ = scratch("x")` drops it at the
/// end of that statement and deletes the fixture.
fn scratch(tag: &str) -> ScratchTree {
    let parent = std::env::temp_dir();
    match crate::rundir::scratch_tree::acquire(&parent, tag) {
        Ok(tree) => tree,
        Err(refusal) => panic!(
            "a scratch tree for `{tag}` under {}: {refusal:?}",
            parent.display()
        ),
    }
}

fn git(repo: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// The guard comes back with the path because it owns the root the path is
/// under: dropping it here would reclaim the repository before the caller's
/// first assertion.
fn temp_repo(tag: &str) -> (ScratchTree, PathBuf) {
    let tree = scratch(tag);
    let dir = tree.path().to_path_buf();
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["config", "user.email", "test@upstroke.local"]);
    git(&dir, &["config", "user.name", "upstroke tests"]);
    fs::write(dir.join("README.md"), "seed\n").expect("seed");
    fs::write(
        dir.join("plan.md"),
        "## Implement the widget\n<!-- upstroke: id=t1 depends= -->\nMake it.\n",
    )
    .expect("plan");
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "seed"]);
    (tree, dir)
}

fn empty_pools(dir: &Path) -> PathBuf {
    let path = dir.join("pools.toml");
    if !path.exists() {
        fs::write(&path, "# no pools\n").expect("pools");
    }
    path
}

fn seed_legacy_run(repo: &Path, run_id: &str, private: &Path, schema: u32) {
    let public = crate::rundir::public_dir(repo, run_id);
    fs::create_dir_all(&public).expect("public dir");
    let head = String::from_utf8(
        Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["rev-parse", "HEAD"])
            .output()
            .expect("git rev-parse")
            .stdout,
    )
    .expect("utf-8");
    let started = crate::events::RunStarted {
        schema,
        upstroke_version: env!("CARGO_PKG_VERSION").to_owned(),
        run_id: run_id.to_owned(),
        branch: format!("upstroke/run-{run_id}"),
        base_sha: head.trim().to_owned(),
        plan_path: "plan.md".to_owned(),
        config_path: Some("upstroke.toml".to_owned()),
        plan_hash: "unused-by-the-refusal".to_owned(),
        normalized_plan_digest: Some(format!("sha256:{}", "0".repeat(64))),
        private_dir: private.join("runs").join(run_id).display().to_string(),
        gates: Vec::new(),
        gates_from_config: false,
        interaction_mode: "off".to_owned(),
        chains: Vec::new(),
        effort_policy: None,
        gate_cmds: None,
        reviews: Some(crate::review::ReviewPlan::default()),
    };
    let event = crate::events::Event::now(crate::events::EventBody::RunStarted {
        data: Box::new(started),
    });
    let line = serde_json::to_string(&event).expect("serialize run_started");
    fs::write(public.join("events.jsonl"), format!("{line}\n")).expect("events.jsonl");
}

fn rust_sources(dir: &Path) -> Vec<PathBuf> {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .expect("read dir")
        .map(|entry| entry.expect("entry").path())
        .collect();
    entries.sort();
    let mut out = Vec::new();
    for entry in entries {
        if entry.is_dir() {
            out.extend(rust_sources(&entry));
        } else if entry.extension().is_some_and(|ext| ext == "rs") {
            out.push(entry);
        }
    }
    out
}
