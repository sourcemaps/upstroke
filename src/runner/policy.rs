//! Extended notes: `docs/internals/runner/policy.md`

#![forbid(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros
)]

use sha2::{Digest, Sha256};

use crate::error::UpstrokeError;
use crate::topology::events::{
    MountSourceIdentity, MountSourceKind, RunnerContract, RunnerKind, RunnerPolicy,
};

pub const CANONICAL_VERSION: &str = "upstroke.runner-policy.v1";

pub const CANONICAL_VERSION_WITH_MOUNTS: &str = "upstroke.runner-policy.v2";

#[must_use]
pub fn host_policy() -> RunnerPolicy {
    RunnerPolicy {
        kind: RunnerKind::Host,
        policy: RunnerContract::HostV1,
        image: None,
        credential_volumes: None,
        mounts: Vec::new(),
    }
}

pub fn resolve_host() -> Result<RunnerPolicy, UpstrokeError> {
    let policy = host_policy();
    policy
        .completeness()
        .map_err(|defect| UpstrokeError::Refused {
            message: format!("the host runner resolved an unusable RunnerPolicy: {defect}"),
        })?;
    Ok(policy)
}

#[must_use]
pub fn canonical_bytes(policy: &RunnerPolicy) -> Vec<u8> {
    let mut out = Vec::new();
    field(
        &mut out,
        if policy.mounts.is_empty() {
            CANONICAL_VERSION
        } else {
            CANONICAL_VERSION_WITH_MOUNTS
        },
    );
    field(&mut out, kind_tag(policy.kind));
    field(&mut out, contract_tag(policy.policy));
    match &policy.image {
        Some(image) => {
            flag(&mut out, true);
            field(&mut out, &image.reference);
            field(&mut out, &image.id);
            match &image.digest {
                Some(digest) => {
                    flag(&mut out, true);
                    field(&mut out, digest);
                }
                None => flag(&mut out, false),
            }
        }
        None => flag(&mut out, false),
    }
    match &policy.credential_volumes {
        Some(volumes) => {
            flag(&mut out, true);
            field(&mut out, &volumes.len().to_string());
            for (agent, volume) in volumes {
                field(&mut out, agent);
                field(&mut out, volume);
            }
        }
        None => flag(&mut out, false),
    }
    if !policy.mounts.is_empty() {
        field(&mut out, &policy.mounts.len().to_string());
        for mount in &policy.mounts {
            field(&mut out, &mount.source);
            field(&mut out, &mount.target);
            flag(&mut out, mount.read_only);
            match &mount.source_identity {
                MountSourceIdentity::Unix {
                    kind,
                    device,
                    inode,
                } => {
                    field(&mut out, "unix");
                    field(&mut out, source_kind_tag(*kind));
                    field(&mut out, &device.to_string());
                    field(&mut out, &inode.to_string());
                }
                MountSourceIdentity::Windows {
                    kind,
                    volume_serial,
                    file_id,
                } => {
                    field(&mut out, "windows");
                    field(&mut out, source_kind_tag(*kind));
                    field(&mut out, &volume_serial.to_string());
                    field(&mut out, file_id);
                }
            }
        }
    }
    out
}

#[must_use]
pub fn runner_policy_sha256(policy: &RunnerPolicy) -> String {
    format!("sha256:{:x}", Sha256::digest(canonical_bytes(policy)))
}

const fn kind_tag(kind: RunnerKind) -> &'static str {
    match kind {
        RunnerKind::Host => "host",
        RunnerKind::Container => "container",
    }
}

const fn source_kind_tag(kind: MountSourceKind) -> &'static str {
    match kind {
        MountSourceKind::Directory => "directory",
        MountSourceKind::File => "file",
    }
}

const fn contract_tag(contract: RunnerContract) -> &'static str {
    match contract {
        RunnerContract::HostV1 => "host-v1",
        RunnerContract::ContainerV1 => "container-v1",
    }
}

fn field(out: &mut Vec<u8>, value: &str) {
    out.extend_from_slice(value.len().to_string().as_bytes());
    out.push(b':');
    out.extend_from_slice(value.as_bytes());
    out.push(b';');
}

fn flag(out: &mut Vec<u8>, value: bool) {
    field(out, if value { "1" } else { "0" });
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::topology::events::ImageIdentity;

    #[test]
    fn the_host_policy_is_inv23s_host_record() {
        let policy = resolve_host().expect("host policy resolves");
        assert_eq!(policy.kind, RunnerKind::Host);
        assert_eq!(policy.policy, RunnerContract::HostV1);
        assert_eq!(policy.image, None, "a host runner has no image");
        assert_eq!(
            policy.credential_volumes, None,
            "a host runner has no credential volumes"
        );
        policy
            .completeness()
            .expect("PR3 accepts the record PR4 resolves");
    }

    const HOST_CANONICAL: &[u8] = b"25:upstroke.runner-policy.v1;4:host;7:host-v1;1:0;1:0;";

    const CONTAINER_CANONICAL: &[u8] = b"25:upstroke.runner-policy.v1;9:container;12:container-v1;\
                                         1:1;15:upstroke/ci:3.2;8:sha256:a;1:1;8:sha256:b;1:1;1:2;\
                                         11:claude-code;8:creds-cc;5:codex;8:creds-cx;";

    fn container_fixture() -> RunnerPolicy {
        let mut volumes = BTreeMap::new();
        volumes.insert("claude-code".to_owned(), "creds-cc".to_owned());
        volumes.insert("codex".to_owned(), "creds-cx".to_owned());
        RunnerPolicy {
            kind: RunnerKind::Container,
            policy: RunnerContract::ContainerV1,
            image: Some(ImageIdentity {
                reference: "upstroke/ci:3.2".to_owned(),
                id: "sha256:a".to_owned(),
                digest: Some("sha256:b".to_owned()),
            }),
            credential_volumes: Some(volumes),
            mounts: Vec::new(),
        }
    }

    #[test]
    fn the_canonical_encoding_separates_records_the_type_separates() {
        let absent = host_policy();
        let mut empty = host_policy();
        empty.credential_volumes = Some(BTreeMap::new());
        assert_ne!(
            absent.credential_volumes, empty.credential_volumes,
            "the two fixtures are the same value, so this test proves nothing"
        );
        assert_ne!(
            canonical_bytes(&absent),
            canonical_bytes(&empty),
            "an absent volume set and an empty one canonicalize alike, so they \
             carry one runner_policy_sha256 between them"
        );
        assert_ne!(
            runner_policy_sha256(&absent),
            runner_policy_sha256(&empty),
            "and the digest INV-23 compares does not separate them either"
        );
        assert_eq!(
            canonical_bytes(&empty),
            b"25:upstroke.runner-policy.v1;4:host;7:host-v1;1:0;1:1;1:0;",
            "the empty-map encoding is not the one the field list describes"
        );

        let mut mislabelled = host_policy();
        mislabelled.image = Some(ImageIdentity {
            reference: "upstroke/ci:3.2".to_owned(),
            id: "sha256:a".to_owned(),
            digest: None,
        });
        assert_ne!(
            canonical_bytes(&host_policy()),
            canonical_bytes(&mislabelled),
            "a host record carrying container-only fields canonicalizes as a \
             clean host record"
        );
        mislabelled
            .completeness()
            .expect_err("a host record carrying an image is not a complete record");
    }

    #[test]
    fn canonical_bytes_match_the_hand_written_payload() {
        assert_eq!(
            canonical_bytes(&host_policy()),
            HOST_CANONICAL,
            "host encoding drifted from the documented field list"
        );
        assert_eq!(
            canonical_bytes(&container_fixture()),
            CONTAINER_CANONICAL,
            "container encoding drifted from the documented field list"
        );
    }

    fn mount(
        source: &str,
        target: &str,
        read_only: bool,
        source_identity: MountSourceIdentity,
    ) -> crate::topology::events::RecordedMount {
        crate::topology::events::RecordedMount {
            source: source.to_owned(),
            target: target.to_owned(),
            read_only,
            source_identity,
        }
    }

    fn mounted_fixture() -> RunnerPolicy {
        let mut policy = container_fixture();
        policy.mounts = vec![
            mount(
                "/srv/cache",
                "/cache",
                true,
                MountSourceIdentity::Unix {
                    kind: MountSourceKind::Directory,
                    device: 2049,
                    inode: 131_074,
                },
            ),
            mount(
                "/opt/cc",
                "/opt/cc",
                false,
                MountSourceIdentity::Unix {
                    kind: MountSourceKind::File,
                    device: 2049,
                    inode: 7,
                },
            ),
        ];
        policy
    }

    const FILE_ID: &str = "00112233445566778899aabbccddeeff";

    fn windows_fixture() -> RunnerPolicy {
        let mut policy = container_fixture();
        policy.mounts = vec![mount(
            r"C:\srv\cache",
            "/cache",
            true,
            MountSourceIdentity::Windows {
                kind: MountSourceKind::Directory,
                volume_serial: 4660,
                file_id: FILE_ID.to_owned(),
            },
        )];
        policy
    }

    const MOUNTED_CANONICAL: &[u8] = b"25:upstroke.runner-policy.v2;9:container;12:container-v1;\
                                       1:1;15:upstroke/ci:3.2;8:sha256:a;1:1;8:sha256:b;1:1;1:2;\
                                       11:claude-code;8:creds-cc;5:codex;8:creds-cx;1:2;\
                                       10:/srv/cache;6:/cache;1:1;4:unix;9:directory;4:2049;\
                                       6:131074;7:/opt/cc;7:/opt/cc;1:0;4:unix;4:file;4:2049;1:7;";

    const WINDOWS_CANONICAL: &[u8] = b"25:upstroke.runner-policy.v2;9:container;12:container-v1;\
                                       1:1;15:upstroke/ci:3.2;8:sha256:a;1:1;8:sha256:b;1:1;1:2;\
                                       11:claude-code;8:creds-cc;5:codex;8:creds-cx;1:1;\
                                       12:C:\\srv\\cache;6:/cache;1:1;7:windows;9:directory;\
                                       4:4660;32:00112233445566778899aabbccddeeff;";

    #[test]
    fn an_empty_mount_list_keeps_the_v1_bytes_and_a_mounted_record_hashes_in_v2() {
        let mut unmounted = mounted_fixture();
        unmounted.mounts.clear();
        assert_eq!(
            canonical_bytes(&unmounted),
            CONTAINER_CANONICAL,
            "a record without mounts left the v1 encoding"
        );
        assert_eq!(canonical_bytes(&host_policy()), HOST_CANONICAL);
        for (name, policy, golden) in [
            ("Unix", mounted_fixture(), MOUNTED_CANONICAL),
            ("Windows", windows_fixture(), WINDOWS_CANONICAL),
        ] {
            assert_eq!(
                canonical_bytes(&policy),
                golden,
                "the {name} v2 encoding drifted from the documented field list"
            );
            assert_eq!(
                runner_policy_sha256(&policy),
                format!("sha256:{:x}", Sha256::digest(golden))
            );
            assert!(
                golden.starts_with(b"25:upstroke.runner-policy.v2;"),
                "a mounted {name} record is not in its own hash domain"
            );
            assert_eq!(
                &golden[b"25:upstroke.runner-policy.v2;".len()..CONTAINER_CANONICAL.len()],
                &CONTAINER_CANONICAL[b"25:upstroke.runner-policy.v1;".len()..],
                "the {name} v2 encoding does not carry the v1 fields unchanged before its mounts"
            );
        }
    }

    #[test]
    fn the_digest_separates_every_field_of_every_mount() {
        type Mover = fn(&mut RunnerPolicy);
        fn unix_fields(p: &mut RunnerPolicy, index: usize) -> (&mut MountSourceKind, &mut u64, &mut u64) {
            match &mut p.mounts[index].source_identity {
                MountSourceIdentity::Unix {
                    kind,
                    device,
                    inode,
                } => (kind, device, inode),
                MountSourceIdentity::Windows { .. } => {
                    panic!("the Unix fixture holds a Windows identity")
                }
            }
        }
        fn windows_fields(p: &mut RunnerPolicy) -> (&mut MountSourceKind, &mut u64, &mut String) {
            match &mut p.mounts[0].source_identity {
                MountSourceIdentity::Windows {
                    kind,
                    volume_serial,
                    file_id,
                } => (kind, volume_serial, file_id),
                MountSourceIdentity::Unix { .. } => {
                    panic!("the Windows fixture holds a Unix identity")
                }
            }
        }
        let unix_movers: [(&str, Mover); 12] = [
            ("a source byte", |p| p.mounts[0].source.push('x')),
            ("a target byte", |p| p.mounts[1].target.push('x')),
            ("read-only", |p| p.mounts[0].read_only = false),
            ("read-write", |p| p.mounts[1].read_only = true),
            ("the kind", |p| *unix_fields(p, 0).0 = MountSourceKind::File),
            ("the device", |p| *unix_fields(p, 1).1 += 1),
            ("the inode", |p| *unix_fields(p, 0).2 += 1),
            ("the order", |p| p.mounts.swap(0, 1)),
            ("one mount fewer", |p| {
                p.mounts.pop();
            }),
            ("a boundary moved between source and target", |p| {
                p.mounts[0].source = "/srv/cache/".to_owned();
                p.mounts[0].target = "cache".to_owned();
            }),
            ("a boundary moved between device and inode", |p| {
                let (_, device, inode) = unix_fields(p, 0);
                *device = 20_491;
                *inode = 31_074;
            }),
            ("the same numbers under the other platform", |p| {
                p.mounts[0].source_identity = MountSourceIdentity::Windows {
                    kind: MountSourceKind::Directory,
                    volume_serial: 2049,
                    file_id: "131074".to_owned(),
                };
            }),
        ];
        let windows_movers: [(&str, Mover); 5] = [
            ("the spelling's case", |p| {
                p.mounts[0].source = p.mounts[0].source.to_ascii_lowercase();
            }),
            ("the kind", |p| *windows_fields(p).0 = MountSourceKind::File),
            ("the volume", |p| *windows_fields(p).1 += 1),
            ("one byte of the file id", |p| {
                windows_fields(p).2.replace_range(30.., "ee");
            }),
            ("a boundary moved between volume and file id", |p| {
                let (_, volume, file_id) = windows_fields(p);
                *volume = 46_600;
                file_id.remove(0);
            }),
        ];
        let mut seen = std::collections::BTreeSet::new();
        let mut unmounted = mounted_fixture();
        unmounted.mounts.clear();
        seen.insert(runner_policy_sha256(&unmounted));
        let mut moved = 0_usize;
        for (base, movers) in [
            (mounted_fixture(), &unix_movers[..]),
            (windows_fixture(), &windows_movers[..]),
        ] {
            assert!(
                seen.insert(runner_policy_sha256(&base)),
                "two fixtures share a digest"
            );
            for (name, mover) in movers {
                let mut other = base.clone();
                mover(&mut other);
                assert_ne!(other, base, "moving {name} moved nothing");
                assert!(
                    seen.insert(runner_policy_sha256(&other)),
                    "moving {name} gave a digest another record already has"
                );
                moved += 1;
            }
        }
        assert_eq!(moved, 17);
        assert_eq!(seen.len(), 1 + 2 + 17);
    }

    #[test]
    fn host_runner_declares_host_v1_policy_with_stable_digest() {
        let policy = resolve_host().expect("host policy resolves");
        let expected = format!("sha256:{:x}", Sha256::digest(HOST_CANONICAL));
        assert_eq!(runner_policy_sha256(&policy), expected);
        assert_eq!(
            runner_policy_sha256(&policy).len(),
            "sha256:".len() + 64,
            "the digest shape the marker and every container intent carry"
        );
        assert_eq!(
            runner_policy_sha256(&resolve_host().expect("resolve again")),
            expected
        );
    }

    #[test]
    fn container_digest_is_pinned_too() {
        assert_eq!(
            runner_policy_sha256(&container_fixture()),
            format!("sha256:{:x}", Sha256::digest(CONTAINER_CANONICAL))
        );
    }

    #[test]
    fn the_digest_separates_every_field_of_the_record() {
        let base = container_fixture();
        let mut moved_reference = base.clone();
        let mut moved_id = base.clone();
        let mut moved_digest = base.clone();
        let mut dropped_digest = base.clone();
        let mut renamed_volume = base.clone();
        let mut extra_volume = base.clone();
        let mut dropped_volumes = base.clone();
        let mut swapped_volume_values = base.clone();

        moved_reference.image.as_mut().expect("image").reference = "upstroke/ci:3.3".to_owned();
        moved_id.image.as_mut().expect("image").id = "sha256:z".to_owned();
        moved_digest.image.as_mut().expect("image").digest = Some("sha256:c".to_owned());
        dropped_digest.image.as_mut().expect("image").digest = None;
        renamed_volume
            .credential_volumes
            .as_mut()
            .expect("volumes")
            .insert("codex".to_owned(), "creds-other".to_owned());
        extra_volume
            .credential_volumes
            .as_mut()
            .expect("volumes")
            .insert("copilot".to_owned(), "creds-cp".to_owned());
        dropped_volumes.credential_volumes = Some(BTreeMap::new());
        {
            let volumes = swapped_volume_values
                .credential_volumes
                .as_mut()
                .expect("volumes");
            volumes.insert("claude-code".to_owned(), "creds-cx".to_owned());
            volumes.insert("codex".to_owned(), "creds-cc".to_owned());
        }

        let fixtures = vec![
            host_policy(),
            base.clone(),
            moved_reference,
            moved_id,
            moved_digest,
            dropped_digest,
            renamed_volume,
            extra_volume,
            dropped_volumes,
            swapped_volume_values,
        ];

        let kinds: std::collections::BTreeSet<_> =
            fixtures.iter().map(|p| kind_tag(p.kind)).collect();
        let contracts: std::collections::BTreeSet<_> =
            fixtures.iter().map(|p| contract_tag(p.policy)).collect();
        let references: std::collections::BTreeSet<_> = fixtures
            .iter()
            .map(|p| p.image.as_ref().map(|i| i.reference.clone()))
            .collect();
        let ids: std::collections::BTreeSet<_> = fixtures
            .iter()
            .map(|p| p.image.as_ref().map(|i| i.id.clone()))
            .collect();
        let digests: std::collections::BTreeSet<_> = fixtures
            .iter()
            .map(|p| p.image.as_ref().and_then(|i| i.digest.clone()))
            .collect();
        let volumes: std::collections::BTreeSet<_> = fixtures
            .iter()
            .map(|p| p.credential_volumes.clone())
            .collect();
        assert_eq!(kinds.len(), 2, "kind takes both values");
        assert_eq!(contracts.len(), 2, "policy version takes both values");
        assert_eq!(
            references.len(),
            3,
            "absent, upstroke/ci:3.2, upstroke/ci:3.3"
        );
        assert_eq!(ids.len(), 3, "absent, sha256:a, sha256:z");
        assert_eq!(digests.len(), 3, "absent, sha256:b, sha256:c");
        assert_eq!(
            volumes.len(),
            6,
            "absent, the pair, renamed, extended, empty, swapped"
        );

        let seen: std::collections::BTreeSet<String> =
            fixtures.iter().map(runner_policy_sha256).collect();
        assert_eq!(
            seen.len(),
            fixtures.len(),
            "two distinguishable runner records share a digest"
        );

        let mut rebuilt = BTreeMap::new();
        rebuilt.insert("codex".to_owned(), "creds-cx".to_owned());
        rebuilt.insert("claude-code".to_owned(), "creds-cc".to_owned());
        let reordered = RunnerPolicy {
            credential_volumes: Some(rebuilt),
            ..base.clone()
        };
        assert_eq!(
            runner_policy_sha256(&reordered),
            runner_policy_sha256(&base),
            "insertion order moved the digest"
        );
    }

    #[test]
    fn ascii_case_is_significant_in_every_string_field_of_the_record() {
        let base = container_fixture();
        let mut upper_reference = base.clone();
        let mut upper_id = base.clone();
        let mut upper_digest = base.clone();
        let mut upper_volume_value = base.clone();
        let mut upper_volume_agent = base.clone();

        upper_reference.image.as_mut().expect("image").reference = "Upstroke/CI:3.2".to_owned();
        upper_id.image.as_mut().expect("image").id = "SHA256:A".to_owned();
        upper_digest.image.as_mut().expect("image").digest = Some("SHA256:B".to_owned());
        upper_volume_value
            .credential_volumes
            .as_mut()
            .expect("volumes")
            .insert("codex".to_owned(), "CREDS-CX".to_owned());
        {
            let volumes = upper_volume_agent
                .credential_volumes
                .as_mut()
                .expect("volumes");
            volumes.remove("codex");
            volumes.insert("Codex".to_owned(), "creds-cx".to_owned());
        }

        let fixtures = [
            ("lowercase", base.clone()),
            ("reference", upper_reference),
            ("image id", upper_id),
            ("image digest", upper_digest),
            ("volume value", upper_volume_value),
            ("volume agent", upper_volume_agent),
        ];
        for (name, policy) in &fixtures[1..] {
            assert_ne!(
                policy, &base,
                "the `{name}` twin is not distinguishable from the base at all"
            );
            let lower = |value: &str| value.to_ascii_lowercase();
            assert_eq!(
                policy
                    .image
                    .as_ref()
                    .map(|image| (lower(&image.reference), lower(&image.id))),
                base.image
                    .as_ref()
                    .map(|image| (lower(&image.reference), lower(&image.id))),
                "the `{name}` twin differs by more than ASCII case"
            );
        }

        let digests: std::collections::BTreeSet<String> = fixtures
            .iter()
            .map(|(_, policy)| runner_policy_sha256(policy))
            .collect();
        assert_eq!(
            digests.len(),
            fixtures.len(),
            "two records differing only in ASCII case share a digest, so the canonical \
             encoding is not injective over the values PR3 compares exactly"
        );

        let upper_host = RunnerPolicy {
            kind: RunnerKind::Container,
            policy: RunnerContract::ContainerV1,
            image: Some(ImageIdentity {
                reference: "R".to_owned(),
                id: "ID".to_owned(),
                digest: None,
            }),
            credential_volumes: None,
            mounts: Vec::new(),
        };
        assert_eq!(
            canonical_bytes(&upper_host),
            b"25:upstroke.runner-policy.v1;9:container;12:container-v1;1:1;1:R;2:ID;1:0;1:0;",
            "an uppercase field did not survive canonicalisation byte for byte"
        );
    }

    #[test]
    fn field_writes_its_values_bytes_and_transforms_nothing() {
        let hostile = [
            "",
            " ",
            "creds",
            "Creds",
            "CREDS",
            "creds_a",
            "creds-a",
            "creds a",
            "creds  a",
            "creds\ta",
            " creds",
            "creds ",
            "creds\n",
            "creds\r\n",
            "creds:1;",
            "creds/../etc",
            "cafe\u{301}",
            "caf\u{e9}",
            "\u{1f600}",
            "creds\0a",
        ];
        let mut encodings = std::collections::BTreeSet::new();
        for value in hostile {
            let mut out = Vec::new();
            field(&mut out, value);
            let mut expected = value.len().to_string().into_bytes();
            expected.push(b':');
            expected.extend_from_slice(value.as_bytes());
            expected.push(b';');
            assert_eq!(
                out,
                expected,
                "`{}` was not written verbatim: `{}`",
                value.escape_debug(),
                String::from_utf8_lossy(&out).escape_debug()
            );
            encodings.insert(out);
        }
        assert_eq!(
            encodings.len(),
            hostile.len(),
            "two of the hostile values encode alike, so the set is not the \
             witness it claims to be"
        );
        let mut wide = Vec::new();
        field(&mut wide, "caf\u{e9}");
        assert_eq!(wide, b"5:caf\xc3\xa9;".to_vec());
    }

    #[test]
    fn a_normalisable_difference_in_any_string_position_moves_the_digest() {
        const PAIRS: &[(&str, &str, &str)] = &[
            ("underscore/hyphen", "creds_a", "creds-a"),
            ("trailing space", "creds", "creds  "),
            ("leading space", "creds", "  creds"),
            ("interior collapse", "creds a", "creds  a"),
            ("tab for space", "creds\ta", "creds a"),
            ("trailing newline", "creds", "creds\n"),
            ("carriage return", "creds", "creds\r"),
            ("ascii case", "creds", "Creds"),
            ("unicode composition", "cafe\u{301}", "caf\u{e9}"),
            ("empty against blank", "", " "),
            ("dotted", "creds", "creds."),
        ];

        #[allow(clippy::type_complexity)]
        let positions: &[(&str, fn(&mut RunnerPolicy, &str))] = &[
            ("image reference", |policy, value| {
                policy.image.as_mut().expect("image").reference = value.to_owned();
            }),
            ("image id", |policy, value| {
                policy.image.as_mut().expect("image").id = value.to_owned();
            }),
            ("image digest", |policy, value| {
                policy.image.as_mut().expect("image").digest = Some(value.to_owned());
            }),
            ("credential volume key", |policy, value| {
                let volumes = policy.credential_volumes.as_mut().expect("volumes");
                volumes.remove("codex");
                volumes.insert(value.to_owned(), "creds-cx".to_owned());
            }),
            ("credential volume value", |policy, value| {
                policy
                    .credential_volumes
                    .as_mut()
                    .expect("volumes")
                    .insert("codex".to_owned(), value.to_owned());
            }),
        ];

        let mut checked = 0_usize;
        for (position, set) in positions {
            for (pair, left, right) in PAIRS {
                assert_ne!(left, right, "{pair}: the fixture pair is one value");
                let mut a = container_fixture();
                let mut b = container_fixture();
                set(&mut a, left);
                set(&mut b, right);
                assert_ne!(
                    a, b,
                    "{position}/{pair}: the two records are equal, so the digest \
                     is not being asked anything"
                );
                assert_ne!(
                    canonical_bytes(&a),
                    canonical_bytes(&b),
                    "{position}/{pair}: `{}` and `{}` canonicalize alike",
                    left.escape_debug(),
                    right.escape_debug()
                );
                assert_ne!(
                    runner_policy_sha256(&a),
                    runner_policy_sha256(&b),
                    "{position}/{pair}: two execution identities share one \
                     runner_policy_sha256, so INV-23's exact equality cannot see \
                     the difference"
                );
                checked += 1;
            }
        }
        assert_eq!(
            positions.len(),
            5,
            "five string positions: reference, id, digest, volume key, volume value"
        );
        assert_eq!(PAIRS.len(), 11, "eleven normalisations");
        assert_eq!(
            checked, 55,
            "every position crossed with every normalisation"
        );
    }

    #[test]
    fn the_container_fields_option_and_sequence_boundaries_are_injective() {
        let mut absent = container_fixture();
        let mut empty = container_fixture();
        absent.image.as_mut().expect("image").digest = None;
        empty.image.as_mut().expect("image").digest = Some(String::new());
        assert_ne!(absent, empty, "the two fixtures are the same value");
        assert_eq!(
            absent.difference(&empty),
            Some(crate::topology::events::RunnerField::ImageDigest),
            "PR3 does not distinguish them, so there is nothing for the digest to preserve"
        );
        const DIGEST_ABSENT: &[u8] = b"25:upstroke.runner-policy.v1;9:container;12:container-v1;\
                                       1:1;15:upstroke/ci:3.2;8:sha256:a;1:0;1:1;1:2;\
                                       11:claude-code;8:creds-cc;5:codex;8:creds-cx;";
        const DIGEST_EMPTY: &[u8] = b"25:upstroke.runner-policy.v1;9:container;12:container-v1;\
                                      1:1;15:upstroke/ci:3.2;8:sha256:a;1:1;0:;1:1;1:2;\
                                      11:claude-code;8:creds-cc;5:codex;8:creds-cx;";
        assert_eq!(
            canonical_bytes(&absent),
            DIGEST_ABSENT,
            "an absent digest is not encoded as the field list describes"
        );
        assert_eq!(
            canonical_bytes(&empty),
            DIGEST_EMPTY,
            "an empty digest is not encoded as the field list describes"
        );
        assert_ne!(DIGEST_ABSENT, DIGEST_EMPTY);
        assert_ne!(runner_policy_sha256(&absent), runner_policy_sha256(&empty));

        let mut no_volumes = container_fixture();
        let mut empty_volumes = container_fixture();
        no_volumes.credential_volumes = None;
        empty_volumes.credential_volumes = Some(BTreeMap::new());
        assert_eq!(
            no_volumes.completeness(),
            Err(crate::topology::events::RunnerRecordDefect::ContainerWithoutCredentialVolumes),
            "PR3 refuses the absent one, which is why the two must not encode alike"
        );
        empty_volumes
            .completeness()
            .expect("an empty set is a real answer");
        assert_ne!(
            canonical_bytes(&no_volumes),
            canonical_bytes(&empty_volumes)
        );
        assert_ne!(
            runner_policy_sha256(&no_volumes),
            runner_policy_sha256(&empty_volumes)
        );

        let volume_pair = |key: &str, value: &str| {
            let mut volumes = BTreeMap::new();
            volumes.insert(key.to_owned(), value.to_owned());
            RunnerPolicy {
                credential_volumes: Some(volumes),
                ..container_fixture()
            }
        };
        let image_pair = |reference: &str, id: &str| RunnerPolicy {
            image: Some(ImageIdentity {
                reference: reference.to_owned(),
                id: id.to_owned(),
                digest: None,
            }),
            ..container_fixture()
        };
        let mut two_entries = BTreeMap::new();
        two_entries.insert("a".to_owned(), "b".to_owned());
        two_entries.insert("c".to_owned(), "d".to_owned());
        let mut one_entry = BTreeMap::new();
        one_entry.insert("ab".to_owned(), "cd".to_owned());

        let colliding: Vec<(&str, RunnerPolicy, RunnerPolicy)> = vec![
            (
                "volume key/value boundary",
                volume_pair("a", "bc"),
                volume_pair("ab", "c"),
            ),
            (
                "image reference/id boundary",
                image_pair("ab", "c"),
                image_pair("a", "bc"),
            ),
            (
                "entry count",
                RunnerPolicy {
                    credential_volumes: Some(two_entries),
                    ..container_fixture()
                },
                RunnerPolicy {
                    credential_volumes: Some(one_entry),
                    ..container_fixture()
                },
            ),
        ];
        for (name, left, right) in &colliding {
            let flatten = |policy: &RunnerPolicy| {
                let image = policy.image.as_ref().expect("image");
                let volumes = policy
                    .credential_volumes
                    .as_ref()
                    .expect("volumes")
                    .iter()
                    .map(|(key, value)| format!("{key}{value}"))
                    .collect::<String>();
                format!("{}{}{volumes}", image.reference, image.id)
            };
            assert_eq!(
                flatten(left),
                flatten(right),
                "{name}: the pair does not actually collide under concatenation, so it is not \
                 the fixture this test claims"
            );
            assert_ne!(
                canonical_bytes(left),
                canonical_bytes(right),
                "{name}: two records forged one field boundary between them"
            );
            assert_ne!(
                runner_policy_sha256(left),
                runner_policy_sha256(right),
                "{name}: two execution identities share one runner_policy_sha256"
            );
        }
        assert_eq!(colliding.len(), 3, "three boundaries, each crossed");
    }

    #[test]
    fn completeness_covers_one_direction_of_the_host_container_field_split() {
        use crate::topology::events::RunnerRecordDefect;

        let image = || {
            Some(ImageIdentity {
                reference: "upstroke/ci:3.2".to_owned(),
                id: "sha256:a".to_owned(),
                digest: None,
            })
        };
        let mut outcomes = std::collections::BTreeMap::new();
        for (kind, contract) in [
            (RunnerKind::Host, RunnerContract::HostV1),
            (RunnerKind::Container, RunnerContract::ContainerV1),
        ] {
            for has_image in [false, true] {
                for has_volumes in [false, true] {
                    let policy = RunnerPolicy {
                        kind,
                        policy: contract,
                        image: has_image.then(image).flatten(),
                        credential_volumes: has_volumes.then(BTreeMap::new),
                        mounts: Vec::new(),
                    };
                    outcomes.insert(
                        format!("{kind:?}/image={has_image}/volumes={has_volumes}"),
                        policy.completeness().err(),
                    );
                }
            }
        }
        assert_eq!(outcomes.len(), 8, "eight cells");
        for has_image in [false, true] {
            for has_volumes in [false, true] {
                let expected = (has_image || has_volumes)
                    .then_some(RunnerRecordDefect::HostWithContainerFields);
                assert_eq!(
                    outcomes[&format!("Host/image={has_image}/volumes={has_volumes}")],
                    expected,
                    "the host direction of the split moved"
                );
            }
        }
        assert_eq!(
            outcomes["Container/image=false/volumes=false"],
            Some(RunnerRecordDefect::ContainerWithoutImage)
        );
        assert_eq!(
            outcomes["Container/image=false/volumes=true"],
            Some(RunnerRecordDefect::ContainerWithoutImage)
        );
        assert_eq!(
            outcomes["Container/image=true/volumes=false"],
            Some(RunnerRecordDefect::ContainerWithoutCredentialVolumes)
        );
        assert_eq!(
            outcomes["Container/image=true/volumes=true"], None,
            "an empty credential-volume map is a complete container record"
        );
        for digest in [None, Some(String::new()), Some("sha256:b".to_owned())] {
            let mut policy = container_fixture();
            policy.image.as_mut().expect("image").digest = digest.clone();
            assert_eq!(
                policy.completeness(),
                Ok(()),
                "a container record with digest {digest:?} is refused by shape"
            );
        }
    }

    #[test]
    fn field_values_carrying_the_delimiters_do_not_collide() {
        let sneaky = RunnerPolicy {
            kind: RunnerKind::Container,
            policy: RunnerContract::ContainerV1,
            image: Some(ImageIdentity {
                reference: "a;1:b".to_owned(),
                id: "c".to_owned(),
                digest: None,
            }),
            credential_volumes: Some(BTreeMap::new()),
            mounts: Vec::new(),
        };
        let plain = RunnerPolicy {
            image: Some(ImageIdentity {
                reference: "a".to_owned(),
                id: "1:b;c".to_owned(),
                digest: None,
            }),
            ..sneaky.clone()
        };
        assert_ne!(
            runner_policy_sha256(&sneaky),
            runner_policy_sha256(&plain),
            "a value carrying the delimiter forged another field boundary"
        );
    }
}
