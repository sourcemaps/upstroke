// Allowlist placement: the funnel section of `effects/allowlist.toml`.
#![allow(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros
)]

use super::*;

/// Whether `path`, a checkout path Git or the filesystem reported, is an
/// instance of `slot` under `manager`'s execution root: its parent is the
/// slot's namespace directory, and its name is the slot's own component, or
/// that component, `_` and twelve characters of the Crockford alphabet.
///
/// Written here against the names, independently of the parser the manager
/// discovers instances with, so that a parser that stopped recognising an
/// instance cannot also blind this oracle to it.
fn is_instance_of(manager: &WorkspaceManager, slot: &Slot, path: &Path) -> bool {
    const ALPHABET: &str = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    let relative = slot.relative();
    let (Some(namespace), Some(component)) = (
        relative.parent(),
        relative.file_name().and_then(|name| name.to_str()),
    ) else {
        return false;
    };
    let Ok(directory) = canonical_prefix(&manager.execution_root().join(namespace)) else {
        return false;
    };
    let Ok(path) = canonical_prefix(path) else {
        return false;
    };
    if path.parent() != Some(directory.as_path()) {
        return false;
    }
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    name == component
        || name
            .strip_prefix(component)
            .and_then(|rest| rest.strip_prefix('_'))
            .is_some_and(|tag| tag.len() == 12 && tag.chars().all(|c| ALPHABET.contains(c)))
}

/// Every registration Git lists whose checkout is an instance of `slot`.
fn registered_instances_of(manager: &WorkspaceManager, slot: &Slot) -> Vec<PathBuf> {
    manager
        .worktree_records()
        .expect("Git enumerates the store")
        .into_iter()
        .map(WorktreeRecord::into_path)
        .filter(|path| is_instance_of(manager, slot, path))
        .collect()
}

/// Every entry under `slot`'s namespace directory that is an instance of it.
fn instance_directories_of(manager: &WorkspaceManager, slot: &Slot) -> Vec<PathBuf> {
    let relative = slot.relative();
    let namespace = relative.parent().expect("a slot path has a namespace");
    match fs::read_dir(manager.execution_root().join(namespace)) {
        Ok(entries) => entries
            .map(|entry| entry.expect("a namespace entry").path())
            .filter(|path| is_instance_of(manager, slot, path))
            .collect(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => panic!("list {}: {error}", namespace.display()),
    }
}

/// What a dead incarnation's Git writer that had not yet run does once it
/// runs: `git worktree add --detach` at `path`, which makes the leading
/// directories, the checkout and the registration and writes no intent.
fn late_add(fixture: &Fixture, path: &Path) {
    git_os(
        &fixture.base,
        &[
            OsStr::new("worktree"),
            OsStr::new("add"),
            OsStr::new("-q"),
            OsStr::new("--detach"),
            path.as_os_str(),
            OsStr::new(&fixture.head),
        ],
    );
}

/// The removal and intent removal every walk makes of a slot, in the frozen
/// walks' order (`finalize.rs`'s `scrub_slots`, `recover.rs`'s reclaims).
fn reclaim_slot(manager: &WorkspaceManager, slot: &Slot) -> Result<(), UpstrokeError> {
    manager.remove_worktree(&mut NoHooks, slot)?;
    manager.remove_intent(&mut NoHooks, slot)
}

/// T-TAG, the manager's half: one incarnation renders one instance of a slot
/// in any manager, and another incarnation renders another, in the same
/// namespace; neither renders the untagged name.
#[test]
fn one_incarnation_renders_one_instance_of_a_slot_and_another_incarnation_another() {
    let fixture = Fixture::created("instance-rendering");
    let again = of_incarnation(&fixture, "inc-1");
    let successor = of_incarnation(&fixture, "inc-2");
    let slot = fixture.task("alpha", 1);
    assert_eq!(fixture.manager.slot_path(&slot), again.slot_path(&slot));
    assert_eq!(fixture.manager.intent_path(&slot), again.intent_path(&slot));
    assert_eq!(fixture.manager.instance_tag(), again.instance_tag());
    let ours = fixture.manager.slot_path(&slot);
    let theirs = successor.slot_path(&slot);
    assert_ne!(ours, theirs, "two incarnations share no instance path");
    assert_ne!(
        fixture.manager.intent_path(&slot),
        successor.intent_path(&slot),
        "nor an intent"
    );
    assert_eq!(
        ours.parent(),
        theirs.parent(),
        "both live in the slot's namespace"
    );
    for (manager, path) in [(&fixture.manager, &ours), (&successor, &theirs)] {
        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some(format!("kalpha-g1_{}", manager.instance_tag().as_str()).as_str()),
            "an instance is the slot's component, `_` and the incarnation's tag"
        );
        assert!(is_instance_of(manager, &slot, path));
    }
    assert_ne!(
        ours,
        fixture.manager.execution_root().join(slot.relative()),
        "no incarnation renders the untagged name"
    );
}

/// The two-incarnation reclaim. The successor's walk meets the earlier
/// incarnation's instance through its intent and removes it — checkout,
/// registration and intent — and never verifies it as its own: until it adds
/// its own instance, its verification reads `NotRegistered`, which routes a
/// resume to the forced removal and a fresh add of its own.
#[test]
fn a_successors_reclaim_removes_an_earlier_incarnations_instance_and_never_verifies_it() {
    let fixture = Fixture::created("instance-two-incarnations");
    let slot = fixture.add_task(&mut NoHooks, "alpha", 1);
    let earlier = fixture.manager.slot_path(&slot);
    let earlier_admin = super::fixture::registration_of(&fixture.manager, &earlier);
    let successor = of_incarnation(&fixture, "inc-2");

    assert_eq!(
        successor
            .verify_worktree(
                &mut NoHooks,
                &slot,
                &Quiescence::AtBase(fixture.head.clone())
            )
            .expect("verify"),
        Err(VerifyFailure::NotRegistered),
        "the earlier incarnation's instance is not the successor's to verify"
    );
    assert_eq!(successor.intents().expect("intents"), vec![slot.clone()]);
    reclaim_slot(&successor, &slot).expect("the successor reclaims the slot");
    assert!(!earlier.exists(), "the earlier checkout is gone");
    assert!(!earlier_admin.exists(), "and its registration");
    assert!(
        !fixture.manager.intent_path(&slot).exists(),
        "and its intent"
    );
    assert!(successor.intents().expect("intents").is_empty());
    assert!(registered_instances_of(&successor, &slot).is_empty());

    successor
        .write_intent(&mut NoHooks, &slot)
        .expect("the successor's intent");
    let own = successor
        .add_worktree(&mut NoHooks, &slot, &fixture.head)
        .expect("the successor's own instance");
    assert_eq!(own, successor.slot_path(&slot));
    assert_eq!(
        successor
            .verify_worktree(
                &mut NoHooks,
                &slot,
                &Quiescence::AtBase(fixture.head.clone())
            )
            .expect("verify"),
        Ok(()),
        "and it verifies only its own"
    );
    assert_eq!(registered_instances_of(&successor, &slot), vec![own]);
}

/// §4.3, the late add (FUC-D3-LATEADD). An earlier incarnation's Git writer
/// that had not yet run when its slot was reclaimed recreates its instance's
/// checkout and registration, and no intent; a helper writing by absolute
/// path recreates a directory alone; and a checkout can be gone while its
/// registration stays. Every walk reaches each through `intents()`, as a
/// logical slot, and removes it with the slot, whether or not the successor
/// has an instance of that slot of its own; the successor's own stands.
#[test]
fn an_earlier_incarnations_instance_with_no_intent_is_found_by_every_walk() {
    let fixture = Fixture::created("instance-late-add");
    let earlier = of_incarnation(&fixture, "inc-0");
    let shared = fixture.task("alpha", 1);
    let own_only = fixture.task("beta", 1);
    let theirs_only = Slot::Snapshot {
        name: SnapshotName::integration(1),
    };

    // The successor (the fixture's manager) holds alpha and beta of its own.
    for slot in [&shared, &own_only] {
        fixture
            .manager
            .write_intent(&mut NoHooks, slot)
            .expect("intent");
        fixture
            .manager
            .add_worktree(&mut NoHooks, slot, &fixture.head)
            .expect("add");
    }
    let own_paths = [
        fixture.manager.slot_path(&shared),
        fixture.manager.slot_path(&own_only),
    ];

    // The late add: the earlier incarnation's alpha and its own snapshot slot,
    // each checkout and registration, no intent.
    late_add(&fixture, &earlier.slot_path(&shared));
    late_add(&fixture, &earlier.slot_path(&theirs_only));
    // A registration whose checkout is gone, and a directory with none.
    let registration_only = Slot::Staging { sequence: 4 };
    let registration_only_path = earlier.slot_path(&registration_only);
    late_add(&fixture, &registration_only_path);
    fs::remove_dir_all(&registration_only_path).expect("the checkout goes, the registration stays");
    let directory_only = fixture.task("gamma", 2);
    create_dir(&earlier.slot_path(&directory_only));
    fs::write(
        earlier.slot_path(&directory_only).join("helper-wrote-this"),
        b"by absolute path\n",
    )
    .expect("a helper's late write");

    let mut expected = vec![
        shared.clone(),
        own_only.clone(),
        directory_only.clone(),
        registration_only.clone(),
        theirs_only.clone(),
    ];
    expected.sort();
    assert_eq!(
        fixture.manager.intents().expect("intents"),
        expected,
        "every earlier instance is a logical slot every walk enumerates, each once"
    );

    for slot in &expected {
        reclaim_slot(&fixture.manager, slot).expect("the walk removes the slot");
    }
    for slot in &expected {
        assert!(
            instance_directories_of(&fixture.manager, slot).is_empty(),
            "no instance of {slot:?} is left in its namespace"
        );
        assert!(
            registered_instances_of(&fixture.manager, slot).is_empty(),
            "and none is registered"
        );
    }
    assert!(fixture.manager.intents().expect("intents").is_empty());
    for path in own_paths {
        assert!(
            !path.exists(),
            "the walk removed the successor's own with its slot"
        );
    }
}

/// §4.3's exclusion: this incarnation creates an instance only after its
/// intent, so an intentless instance at its own tag is none of its dead
/// instances. `intents()` does not report it and the final sweep leaves it,
/// so the finalizer's existing refusal of a torn registration no intent names
/// stands, and a checkout of its own keeps the root (T-FIN2).
#[test]
fn this_incarnations_own_instance_with_no_intent_is_not_discovered_and_keeps_the_root() {
    let fixture = Fixture::created("instance-own-intentless");
    let slot = fixture.task("alpha", 1);
    let path = fixture.manager.slot_path(&slot);
    late_add(&fixture, &path);
    let admin = super::fixture::registration_of(&fixture.manager, &path);
    assert!(fixture.manager.intents().expect("intents").is_empty());
    let removed = fixture
        .manager
        .remove_execution_root(&mut NoHooks)
        .expect("finalization's last step");
    assert!(
        !removed,
        "a checkout of this incarnation's own keeps the root"
    );
    assert!(path.is_dir() && admin.is_dir(), "and the sweep left it");
}

/// §4.7: a name with no tag was written before instances existed, and is
/// another incarnation's like any other: its intent, its checkout and its
/// registration are reclaimed with its slot.
#[test]
fn an_untagged_instance_from_before_instances_is_reclaimed_like_an_earlier_incarnations() {
    let fixture = Fixture::created("instance-untagged");
    let slot = fixture.task("alpha", 1);
    let untagged = fixture.manager.execution_root().join(slot.relative());
    let record = IntentRecord::new(
        &slot,
        None,
        super::fixture::RUN_ID.to_owned(),
        "old".to_owned(),
    )
    .expect("a record of the untagged name");
    let intent = fixture
        .manager
        .execution_root()
        .join("intents")
        .join(slot.intent_name());
    fs::write(&intent, serde_json::to_vec(&record).expect("serialize")).expect("the old intent");
    late_add(&fixture, &untagged);
    assert!(is_instance_of(&fixture.manager, &slot, &untagged));
    assert_eq!(
        fixture.manager.intents().expect("intents"),
        vec![slot.clone()]
    );

    reclaim_slot(&fixture.manager, &slot).expect("the walk reclaims the untagged instance");
    assert!(!untagged.exists() && !intent.exists());
    assert!(registered_instances_of(&fixture.manager, &slot).is_empty());
    assert!(fixture.manager.intents().expect("intents").is_empty());
}

/// The follow-up C record's §6.11: a registration of the repository's store
/// whose `gitdir` cannot be read — a directory stands where the file should
/// be — at `<common git dir>/worktrees/<name>`. Git's own list passes over it;
/// the read that discovers earlier incarnations' instances and the removal's
/// scan fail on it, on every attempt, until it is gone.
fn plant_an_unreadable_registration(common_git_dir: &Path, name: &str) -> PathBuf {
    let admin = common_git_dir.join("worktrees").join(name);
    create_dir(&admin.join("gitdir"));
    write_file(&admin.join("commondir"), b"../..\n");
    admin
}

/// §6.11's observer: every wait of a registry access the call makes is
/// counted, and the first that finds the unreadable registration removes it,
/// so the attempt after that wait reads the store whole. It never sleeps: a
/// wait that slept on the calling thread instead (`super::sleep_for`) is one it
/// never sees, and `slept_pauses` counts it.
struct HealsTheStoreAtAPause {
    admin: PathBuf,
    pauses: usize,
    healed_at: Option<usize>,
}

impl HealsTheStoreAtAPause {
    fn over(admin: PathBuf) -> Self {
        Self {
            admin,
            pauses: 0,
            healed_at: None,
        }
    }
}

impl EffectHooks for HealsTheStoreAtAPause {
    fn phase(&mut self, _site: EffectSiteId, _phase: HookPhase) -> Injection {
        Injection::Proceed
    }

    fn refusal_cause(&self) -> Option<String> {
        None
    }

    fn registry_pause(&mut self, _pause: std::time::Duration) -> Result<(), UpstrokeError> {
        self.pauses += 1;
        if self.healed_at.is_none() && self.admin.exists() {
            fs::remove_dir_all(&self.admin).unwrap_or_else(|error| {
                panic!("heal the store at {}: {error}", self.admin.display())
            });
            self.healed_at = Some(self.pauses);
        }
        Ok(())
    }
}

/// An earlier incarnation's instance of `slot` that its registration alone
/// names: the late add's checkout, then the checkout removed.
fn an_earlier_registration_only_instance(
    fixture: &Fixture,
    earlier: &WorkspaceManager,
    slot: &Slot,
) {
    late_add(fixture, &earlier.slot_path(slot));
    fs::remove_dir_all(earlier.slot_path(slot)).expect("the checkout goes, the registration stays");
}

/// §6.11: `intents_pausing`, the form a walk that holds hooks takes, reads
/// the registry through them. The read meets a registration it cannot read,
/// waits through the hooks — which heal the store — and the attempt after
/// the wait finds the earlier instance its registration alone names. Nothing
/// sleeps on the calling thread.
///
/// Its read waits to the production deadline (the follow-up C record's §6.15,
/// the class of the follow-up B record's §9.31), so it asks for the wait after
/// its first failed attempt only if time remains when it reads its clock after
/// that attempt's veto and count. If the deadline has passed by then, wherever
/// the time went (the attempt, its veto, the count, or its thread descheduled),
/// it asks for no wait: it makes its final attempt, or refuses if the failed
/// attempt was already its final one (the follow-up B record's §9.32 and
/// §9.34).
#[test]
fn intents_pausing_reads_the_registry_through_the_hooks_it_is_handed() {
    let fixture = Fixture::created("pausing-intents");
    let _deadline = super::RegistryDeadline::hold(
        fixture.manager.common_git_dir(),
        PRODUCTION_REGISTRY_DEADLINE,
    );
    let earlier = of_incarnation(&fixture, "inc-0");
    let slot = Slot::Staging { sequence: 4 };
    an_earlier_registration_only_instance(&fixture, &earlier, &slot);
    let mut hooks = HealsTheStoreAtAPause::over(plant_an_unreadable_registration(
        fixture.manager.common_git_dir(),
        "unreadable",
    ));
    let slept = super::fixture::slept_pauses();

    let intents = fixture
        .manager
        .intents_pausing(&mut hooks)
        .expect("the read is passed once the store is healed");

    assert_eq!(intents, vec![slot], "the slot its registration alone names");
    assert_eq!(
        hooks.healed_at,
        Some(1),
        "the read failed on the store and waited through the hooks"
    );
    assert_eq!(
        super::fixture::slept_pauses(),
        slept,
        "no wait of the read slept on the calling thread"
    );
}

/// §6.11: a reclaim's walk (`reclaim_intents`) enumerates through the hooks
/// it is handed, so its registry read waits through them, and the earlier
/// instance it finds is reclaimed.
///
/// Its walk's read waits to the production deadline (the follow-up C record's
/// §6.15, the class of the follow-up B record's §9.31), so it asks for the wait
/// after its first failed attempt only if time remains when it reads its clock
/// after that attempt's veto and count. If the deadline has passed by then,
/// wherever the time went (the attempt, its veto, the count, or its thread
/// descheduled), it asks for no wait: it makes its final attempt, or refuses if
/// the failed attempt was already its final one (the follow-up B record's §9.32
/// and §9.34).
#[test]
fn a_reclaims_walk_reads_the_registry_through_the_hooks_it_is_handed() {
    let fixture = Fixture::created("pausing-reclaim");
    let _deadline = super::RegistryDeadline::hold(
        fixture.manager.common_git_dir(),
        PRODUCTION_REGISTRY_DEADLINE,
    );
    let earlier = of_incarnation(&fixture, "inc-0");
    let slot = Slot::Staging { sequence: 4 };
    an_earlier_registration_only_instance(&fixture, &earlier, &slot);
    let mut hooks = HealsTheStoreAtAPause::over(plant_an_unreadable_registration(
        fixture.manager.common_git_dir(),
        "unreadable",
    ));
    let slept = super::fixture::slept_pauses();

    let reclaimed = fixture
        .manager
        .reclaim_intents(&mut hooks)
        .expect("the reclaim is passed once the store is healed");

    assert_eq!(reclaimed.slots, vec![slot.clone()]);
    assert_eq!(
        hooks.healed_at,
        Some(1),
        "the walk's read failed on the store and waited through the hooks"
    );
    assert_eq!(super::fixture::slept_pauses(), slept, "nothing slept");
    assert!(registered_instances_of(&fixture.manager, &slot).is_empty());
}

/// §6.11: terminal finalization's final sweep reads the registry through the
/// hooks it is handed. Its read meets a registration it cannot read, waits
/// through the hooks, and then removes the earlier instance it found.
///
/// The sweep's read waits to the production deadline (the follow-up C record's
/// §6.15, the class of the follow-up B record's §9.31), so it asks for the wait
/// after its first failed attempt only if time remains when it reads its clock
/// after that attempt's veto and count. If the deadline has passed by then,
/// wherever the time went (the attempt, its veto, the count, or its thread
/// descheduled), it asks for no wait: it makes its final attempt, or refuses if
/// the failed attempt was already its final one (the follow-up B record's §9.32
/// and §9.34).
#[test]
fn the_final_sweeps_registry_read_waits_through_the_hooks_it_is_handed() {
    let fixture = Fixture::created("pausing-sweep-read");
    let _deadline = super::RegistryDeadline::hold(
        fixture.manager.common_git_dir(),
        PRODUCTION_REGISTRY_DEADLINE,
    );
    let earlier = of_incarnation(&fixture, "inc-0");
    let slot = fixture.task("alpha", 1);
    late_add(&fixture, &earlier.slot_path(&slot));
    let mut hooks = HealsTheStoreAtAPause::over(plant_an_unreadable_registration(
        fixture.manager.common_git_dir(),
        "unreadable",
    ));
    let slept = super::fixture::slept_pauses();

    fixture
        .manager
        .remove_execution_root(&mut hooks)
        .expect("the finalization's last step is passed once the store is healed");

    assert_eq!(
        hooks.healed_at,
        Some(1),
        "the sweep's read failed on the store and waited through the hooks"
    );
    assert_eq!(super::fixture::slept_pauses(), slept, "nothing slept");
    assert!(instance_directories_of(&fixture.manager, &slot).is_empty());
    assert!(registered_instances_of(&fixture.manager, &slot).is_empty());
}

/// §6.11: each removal the final sweep makes waits through the hooks it is
/// handed. The unreadable registration is planted just before the removal's
/// own access, the third of the call (the gate's list, the sweep's read, the
/// removal), so only the removal's scan meets it.
///
/// The removal's scan waits to the production deadline (the follow-up C
/// record's §6.15, the class of the follow-up B record's §9.31), so it asks for
/// the wait after its first failed attempt only if time remains when it reads
/// its clock after that attempt's veto and count. If the deadline has passed by
/// then, wherever the time went (the attempt, its veto, the count, or its
/// thread descheduled), it asks for no wait: it makes its final attempt, or
/// refuses if the failed attempt was already its final one (the follow-up B
/// record's §9.32 and §9.34).
#[test]
fn the_final_sweeps_removal_waits_through_the_hooks_it_is_handed() {
    let fixture = Fixture::created("pausing-sweep-removal");
    let _deadline = super::RegistryDeadline::hold(
        fixture.manager.common_git_dir(),
        PRODUCTION_REGISTRY_DEADLINE,
    );
    let earlier = of_incarnation(&fixture, "inc-0");
    let slot = fixture.task("alpha", 1);
    late_add(&fixture, &earlier.slot_path(&slot));
    let admin = fixture
        .manager
        .common_git_dir()
        .join("worktrees")
        .join("unreadable");
    let _planting = super::fixture::before_registry_access(
        3,
        Box::new(|common_git_dir: &Path| {
            plant_an_unreadable_registration(common_git_dir, "unreadable");
        }),
    );
    let mut hooks = HealsTheStoreAtAPause::over(admin.clone());
    let slept = super::fixture::slept_pauses();

    fixture
        .manager
        .remove_execution_root(&mut hooks)
        .expect("the removal is passed once the store is healed");

    assert_eq!(
        hooks.healed_at,
        Some(1),
        "the removal's scan failed on the store and waited through the hooks"
    );
    assert!(
        !admin.exists(),
        "the planted registration was met and healed"
    );
    assert_eq!(super::fixture::slept_pauses(), slept, "nothing slept");
    assert!(instance_directories_of(&fixture.manager, &slot).is_empty());
    assert!(registered_instances_of(&fixture.manager, &slot).is_empty());
}

/// §4.5, the refusing variant: nothing retains a dead instance. One that
/// cannot be removed refuses the reclaim, resumably, as any removal does, and
/// the next reclaim, once it can be removed, converges. Evaluated on the Unix
/// legs, where a mode bit binds a non-root user.
#[cfg(unix)]
#[test]
fn an_earlier_instance_that_cannot_be_removed_refuses_the_reclaim_and_the_next_converges() {
    use std::os::unix::fs::PermissionsExt as _;
    let fixture = Fixture::created("instance-unremovable");
    let earlier = of_incarnation(&fixture, "inc-0");
    let slot = fixture.task("alpha", 1);
    earlier
        .write_intent(&mut NoHooks, &slot)
        .expect("the earlier intent");
    let path = earlier
        .add_worktree(&mut NoHooks, &slot, &fixture.head)
        .expect("the earlier instance");
    let held = path.join("held");
    create_dir(&held);
    fs::write(held.join("file"), b"a writer's file\n").expect("plant a file");
    let _restore = RestoreMode { path: held.clone() };
    fs::set_permissions(&held, fs::Permissions::from_mode(0o555)).expect("make it unwritable");
    let probe = held.join("probe-write");
    if fs::write(&probe, b"x").is_ok() {
        fs::remove_file(&probe).expect("remove the probe");
        panic!(
            "prerequisite not met: the mode bit did not bind (running as root or with \
             CAP_DAC_OVERRIDE); this test needs an unprivileged user"
        );
    }

    let error = fixture
        .manager
        .remove_worktree(&mut NoHooks, &slot)
        .expect_err("an earlier instance that cannot be removed refuses the reclaim");
    assert!(
        matches!(
            &error,
            UpstrokeError::Filesystem {
                operation: "remove",
                ..
            }
        ),
        "the refusal is the removal's own: {error}"
    );
    assert!(
        earlier.intent_path(&slot).exists(),
        "its intent stays for the next reclaim"
    );

    fs::set_permissions(&held, fs::Permissions::from_mode(0o755)).expect("restore");
    reclaim_slot(&fixture.manager, &slot).expect("the next reclaim converges");
    assert!(!path.exists());
    assert!(registered_instances_of(&fixture.manager, &slot).is_empty());
    assert!(fixture.manager.intents().expect("intents").is_empty());
}

/// T-FIN1 (§5.4): an earlier incarnation's writer adds its instance after
/// finalization's task scrub. The final sweep at the root's removal finds it
/// — directory and registration — removes it through its kind's removal, and
/// the root is removed.
#[test]
fn the_final_sweep_removes_an_instance_an_earlier_incarnation_added_after_the_scrub() {
    let fixture = Fixture::created("final-sweep");
    let earlier = of_incarnation(&fixture, "inc-0");
    let slot = fixture.add_task(&mut NoHooks, "alpha", 1);
    for slot in fixture.manager.intents().expect("intents") {
        fixture
            .manager
            .remove_worktree_proving(&mut NoHooks, &slot, WriterProof::NoWriterAlive)
            .expect("the task scrub");
        fixture
            .manager
            .remove_intent(&mut NoHooks, &slot)
            .expect("the task scrub's intent removal");
    }
    let late = earlier.slot_path(&slot);
    late_add(&fixture, &late);
    let late_admin = super::fixture::registration_of(&fixture.manager, &late);

    let (mut hooks, shared) = harness();
    let removed = fixture
        .manager
        .remove_execution_root(&mut hooks)
        .expect("finalization's last step");
    assert!(removed, "the sweep left the root empty, and it was removed");
    assert!(!late.exists() && !late_admin.exists());
    assert!(registered_instances_of(&fixture.manager, &slot).is_empty());
    let harness = shared.lock().expect("the harness");
    assert_eq!(
        harness.count(
            EffectSiteId::Worktree(WorktreeSite::Remove),
            HookPhase::After
        ),
        1,
        "the instance went through its kind's removal site, once"
    );
    assert_eq!(
        harness.count(
            EffectSiteId::Worktree(WorktreeSite::RemoveExecutionRoot),
            HookPhase::After
        ),
        1
    );
}

/// The switch set (§2.2, §5.2, §5.3), pinned in both builders: every Git
/// child the manager starts carries the six command-scope switches and the
/// three bindings, with the replacement pair, before its own arguments.
#[test]
fn both_builders_carry_the_engine_switch_set_and_its_bindings() {
    let fixture = Fixture::created("switch-set");
    for switch in [
        "maintenance.auto=false",
        "gc.auto=0",
        "gc.autoDetach=false",
        "maintenance.autoDetach=false",
        "rerere.enabled=false",
        "worktree.useRelativePaths=false",
    ] {
        assert!(
            ENGINE_GIT_SWITCHES
                .chunks(2)
                .any(|pair| pair == ["-c", switch]),
            "the switch set names `-c {switch}`"
        );
    }
    let builders = [
        (
            "WorkspaceManager::command",
            fixture
                .manager
                .command(&fixture.base, &[OsString::from("status")]),
        ),
        (
            "read_only_git",
            read_only_command(ReadAt::Base, &fixture.base, &["status"]),
        ),
    ];
    for (builder, command) in &builders {
        let args: Vec<&OsStr> = command.get_args().collect();
        let own = args
            .iter()
            .position(|arg| *arg == OsStr::new("status"))
            .expect("the command's own argument");
        for pair in ENGINE_GIT_SWITCHES.chunks(2) {
            let at = args
                .windows(2)
                .position(|window| {
                    window
                        .iter()
                        .zip(pair)
                        .all(|(arg, switch)| *arg == OsStr::new(switch))
                })
                .unwrap_or_else(|| panic!("{builder} does not carry {pair:?}: {args:?}"));
            assert!(at < own, "{builder}'s {pair:?} comes before the subcommand");
        }
        let envs: Vec<(&OsStr, Option<&OsStr>)> = command.get_envs().collect();
        for (key, value) in ENGINE_GIT_ENVIRONMENT
            .iter()
            .chain([&NO_REPLACEMENT_OBJECTS])
        {
            assert!(
                envs.contains(&(OsStr::new(key), Some(OsStr::new(value)))),
                "{builder} binds {key}={value}: {envs:?}"
            );
        }
    }
    assert_eq!(
        ENGINE_GIT_ENVIRONMENT,
        [
            ("GIT_NO_LAZY_FETCH", "1"),
            ("GIT_ALLOW_PROTOCOL", ""),
            ("GIT_TERMINAL_PROMPT", "0"),
        ]
    );
}

/// FUC-D5-SPLITREAD and `PR128-RESIDUE-UNREACHABLE-OBJECTS-IGNORES-THE-EXIT-STATUS`:
/// a `fsck` that fails is an error naming the command, its exit and its
/// standard error, never the empty listing it printed before it stopped, and
/// the residue classifier passes it on. A registration a killed add left torn
/// is one such failure (`fsck` reads every worktree's `HEAD` and index);
/// with the store whole again the same repository lists its unreachable
/// object.
#[test]
fn a_failed_fsck_is_an_error_naming_the_command_and_never_an_empty_listing() {
    let fixture = Fixture::created("fsck-fails-closed");
    let slot = fixture.add_task(&mut NoHooks, "alpha", 1);
    let path = fixture.manager.slot_path(&slot);
    let orphan = fixture.base.join("orphan.txt");
    fs::write(&orphan, b"an object nothing references\n").expect("orphan file");
    let object = git(
        &fixture.base,
        &["hash-object", "-w", &orphan.to_string_lossy()],
    );
    fs::remove_file(&orphan).expect("remove the file; its object stays");
    assert!(
        unreachable_objects(&fixture.base)
            .expect("fsck over a whole store")
            .contains(&object),
        "the control: the listing names the planted object"
    );

    let admin = tear_registration(&fixture.manager, &path);
    let error = unreachable_objects(&fixture.base).expect_err("a failed fsck is an error");
    let message = error.to_string();
    assert!(
        message.contains("git fsck --unreachable") && message.contains("commondir"),
        "the error names the command and Git's own complaint: {message}"
    );
    let site = EffectSiteId::Object(ObjectSite::CandidateCommitTree);
    let target = ResidueTarget::new(&fixture.base).at(&path);
    assert!(
        observed_residue_elements(site, &target).is_err(),
        "the residue classifier passes the failure on rather than certifying no residue"
    );

    fs::write(admin.join("commondir"), b"../..\n").expect("repair the torn commondir");
    assert!(
        unreachable_objects(&fixture.base)
            .expect("fsck over a whole store again")
            .contains(&object)
    );
}

/// FUC-D5-WINPATHBYTES: Git for Windows refuses a `$GIT_DIR` longer than
/// `PATH_MAX - 40`, 220, by `strlen`, so the add's budget is counted in UTF-8
/// bytes of the path as Git for Windows renders it — no verbatim prefix, `/`
/// separators — and not in characters. Pinned on every platform over the
/// rendering; the native add is the Windows test below.
#[test]
fn the_windows_git_dir_budget_counts_utf8_bytes_of_the_path_git_for_windows_is_handed() {
    use crate::runner::host::GitdirRule;
    let checkout = |ascii: usize, accented: usize| {
        PathBuf::from(format!(
            "C:{}{}",
            "a".repeat(ascii - 2),
            "\u{e9}".repeat(accented)
        ))
    };
    let refused = |path: &Path| match refuse_git_dir_over_budget(path, GitdirRule::Windows) {
        Ok(()) => None,
        Err(UpstrokeError::Refused { message }) => Some(message),
        Err(other) => panic!("not the budget's refusal: {other}"),
    };
    // `<checkout>/.git`: 215 ASCII characters and `/.git` are 220 bytes.
    assert_eq!(
        refused(&checkout(215, 0)),
        None,
        "220 bytes is within the budget"
    );
    let over = refused(&checkout(216, 0)).expect("221 bytes is over it");
    assert!(
        over.contains("221 bytes") && over.contains("220"),
        "the refusal names the length and the budget: {over}"
    );
    // The record's boundary case: 217 ASCII characters and two `é` make 219
    // characters and 221 bytes, which a character count would admit.
    let accented = checkout(212, 2);
    let git_dir = accented.join(".git");
    let text = git_dir.to_str().expect("UTF-8");
    assert_eq!(text.chars().count(), 219);
    assert_eq!(text.len(), 221);
    let message = refused(&accented).expect("219 characters, 221 bytes, is over the budget");
    assert!(message.contains("221 bytes"), "{message}");
    assert_eq!(
        refused(&checkout(211, 2)),
        None,
        "218 characters and 220 bytes is within it"
    );
    // The verbatim prefix is not Git's spelling, so it is not counted.
    let verbatim = PathBuf::from(format!(r"\\?\C:{}", "a".repeat(213)));
    assert_eq!(
        refused(&verbatim),
        None,
        "220 bytes once the prefix is dropped"
    );
    let verbatim = PathBuf::from(format!(r"\\?\C:{}", "a".repeat(214)));
    assert!(
        refused(&verbatim).is_some(),
        "221 bytes once the prefix is dropped"
    );
}

/// One frozen scrub step, `finalize.rs`'s private `scrub_slots`, through the
/// manager's public calls: for every slot `intents()` returns that the step
/// keeps, the forced removal under the finalizer's proof, then the intent.
/// `scrub_slots` is frozen, so this sequence cannot drift from it; what it
/// returns is the count the frozen step returns.
fn scrub_kind(
    manager: &WorkspaceManager,
    hooks: &mut dyn EffectHooks,
    keep: fn(&Slot) -> bool,
) -> Result<usize, UpstrokeError> {
    let mut count = 0;
    for slot in manager.intents()? {
        if !keep(&slot) {
            continue;
        }
        let passed_over =
            manager.remove_worktree_proving(hooks, &slot, WriterProof::NoWriterAlive)?;
        assert!(
            passed_over.is_empty(),
            "no registration here names nothing: {passed_over:?}"
        );
        manager.remove_intent(hooks, &slot)?;
        count += 1;
    }
    Ok(count)
}

fn tasks_only(slot: &Slot) -> bool {
    matches!(slot, Slot::Task { .. })
}

fn snapshots_only(slot: &Slot) -> bool {
    matches!(slot, Slot::Snapshot { .. })
}

/// The instance `manager` adds of `slot`, intent first, tracked.
fn add_tracked(fixture: &Fixture, manager: &WorkspaceManager, slot: &Slot) -> Tracked {
    manager
        .write_intent(&mut NoHooks, slot)
        .expect("write the intent");
    let checkout = manager
        .add_worktree(&mut NoHooks, slot, &fixture.head)
        .expect("add the worktree");
    Tracked {
        admin: super::fixture::registration_of(&fixture.manager, &checkout),
        intent: manager.intent_path(slot),
        checkout,
    }
}

/// The frozen `CrossKind` fixture (`finalize.rs:370-400`) and, with
/// `earlier`, the replacement's second fixture: alpha's task `g1`, the
/// integration snapshot of sequence 1 torn as a killed add leaves it, and an
/// earlier incarnation's instance of alpha beside the fixture's own.
struct CrossKindInstances {
    alpha: Slot,
    snapshot: Slot,
    tracked: Vec<Tracked>,
}

impl CrossKindInstances {
    fn build(fixture: &Fixture, earlier: bool) -> Self {
        let alpha = fixture.task("alpha", 1);
        let mut tracked = vec![add_tracked(fixture, &fixture.manager, &alpha)];
        if earlier {
            let earlier = of_incarnation(fixture, "inc-0");
            tracked.push(add_tracked(fixture, &earlier, &alpha));
        }
        let snapshot = Slot::Snapshot {
            name: SnapshotName::integration(1),
        };
        let mut torn = add_tracked(fixture, &fixture.manager, &snapshot);
        torn.admin = tear_registration(&fixture.manager, &torn.checkout);
        assert_enumeration_dies_on(fixture, &torn.admin);
        tracked.push(torn);
        assert_eq!(
            fixture.manager.intents().expect("intents"),
            vec![alpha.clone(), snapshot.clone()],
            "one task slot and one snapshot slot, whatever instances each has"
        );
        Self {
            alpha,
            snapshot,
            tracked,
        }
    }

    /// R-O1's assertion, which asks Git and not a suffix: no registration
    /// names any instance of either slot, every captured administrative path
    /// is absent, and every checkout and intent is gone.
    fn assert_converged(&self, fixture: &Fixture) {
        assert!(
            fixture.manager.intents().expect("intents").is_empty(),
            "both slots are reclaimed"
        );
        for tracked in &self.tracked {
            assert!(
                !tracked.checkout.exists() && !tracked.admin.exists() && !tracked.intent.exists(),
                "{} is reclaimed, its registration and intent with it",
                tracked.checkout.display()
            );
        }
        for slot in [&self.alpha, &self.snapshot] {
            assert!(
                registered_instances_of(&fixture.manager, slot).is_empty(),
                "Git lists no instance of {slot:?}"
            );
        }
    }
}

/// R-O1, the replacement for `finalize.rs:421-422`'s first history: the task
/// scrub, then the snapshot scrub, with no injection, on the frozen fixture
/// and on its second-incarnation twin.
#[test]
fn r_o1_a_cross_kind_scrub_leaves_no_instance_of_either_slot_registered() {
    for earlier in [false, true] {
        let fixture = Fixture::created(&format!("r-o1-{earlier}"));
        let instances = CrossKindInstances::build(&fixture, earlier);
        assert_eq!(
            scrub_kind(&fixture.manager, &mut NoHooks, tasks_only)
                .expect("a torn snapshot registration does not wedge the task scrub"),
            1
        );
        assert_eq!(
            scrub_kind(&fixture.manager, &mut NoHooks, snapshots_only)
                .expect("the snapshot scrub converges"),
            1
        );
        instances.assert_converged(&fixture);
    }
}

/// R-O2, the replacement for `finalize.rs:421-422` at every injected phase
/// (`a_cross_kind_scrub_stopped_at_any_phase_converges_on_the_next`): both
/// scrubs stopped at the stop-th phase, then both again without injection,
/// R-O1's assertion after every retry, and a completed run's phases exactly
/// the frozen test's ten — on the frozen fixture or, with `earlier`, on its
/// second-incarnation twin. The two tests below run one fixture each and
/// hold both to the same ten, so an earlier incarnation's instance costs no
/// phase of its own.
fn stop_a_cross_kind_scrub_at_every_phase(earlier: bool) {
    let mut stop = 0;
    let completed = loop {
        stop += 1;
        assert!(
            stop <= 32,
            "two scrubs of one slot each consult a bounded number of phases"
        );
        let fixture = Fixture::created(&format!("r-o2-{earlier}-{stop}"));
        let instances = CrossKindInstances::build(&fixture, earlier);
        let mut hooks = StopAt {
            stop,
            seen: Vec::new(),
        };
        let stopped = scrub_kind(&fixture.manager, &mut hooks, tasks_only)
            .and_then(|_| scrub_kind(&fixture.manager, &mut hooks, snapshots_only));
        for tracked in &instances.tracked {
            if tracked.checkout.exists() || tracked.admin.exists() {
                assert!(
                    tracked.intent.exists(),
                    "stopped at phase {stop}: {} outlives its intent",
                    tracked.checkout.display()
                );
            }
        }
        scrub_kind(&fixture.manager, &mut NoHooks, tasks_only)
            .and_then(|_| scrub_kind(&fixture.manager, &mut NoHooks, snapshots_only))
            .unwrap_or_else(|error| {
                panic!("stopped at phase {stop}, the next scrubs converge: {error}")
            });
        instances.assert_converged(&fixture);
        if stopped.is_ok() {
            break hooks.seen;
        }
    };
    let remove = EffectSiteId::Worktree(WorktreeSite::Remove);
    let remove_intent = EffectSiteId::Worktree(WorktreeSite::RemoveIntent);
    let snapshot_remove = EffectSiteId::Snapshot(SnapshotSite::Remove);
    let snapshot_remove_intent = EffectSiteId::Snapshot(SnapshotSite::RemoveIntent);
    assert_eq!(
        completed,
        vec![
            (remove, HookPhase::Before),
            (remove, HookPhase::After),
            (snapshot_remove, HookPhase::Before),
            (snapshot_remove, HookPhase::After),
            (remove_intent, HookPhase::Before),
            (remove_intent, HookPhase::After),
            (snapshot_remove, HookPhase::Before),
            (snapshot_remove, HookPhase::After),
            (snapshot_remove_intent, HookPhase::Before),
            (snapshot_remove_intent, HookPhase::After),
        ],
        "the frozen test's ten phases, with {} instance(s) of alpha",
        if earlier { 2 } else { 1 }
    );
}

/// R-O2 on the frozen `CrossKind` fixture: one instance of alpha, its own.
#[test]
fn r_o2_a_cross_kind_scrub_stopped_at_any_phase_converges_with_no_instance_registered_with_one_instance_of_alpha()
 {
    stop_a_cross_kind_scrub_at_every_phase(false);
}

/// R-O2 on the second fixture: an earlier incarnation's instance of alpha too.
#[test]
fn r_o2_a_cross_kind_scrub_stopped_at_any_phase_converges_with_no_instance_registered_with_an_earlier_incarnations_instance()
 {
    stop_a_cross_kind_scrub_at_every_phase(true);
}

/// R-O3, the replacement for `finalize.rs:456`
/// (`scrub_slots_converges_past_a_torn_registration_of_the_kind_it_reclaims`):
/// alpha's and bravo's task `g1`, bravo's torn and sorting second; the task
/// scrub returns 2, Git lists no instance of either, and alpha's own
/// administrative directory, captured before the scrub, is gone — which the
/// frozen test does not assert, and which its suffix predicate cannot see
/// under per-incarnation names.
#[test]
fn r_o3_a_task_scrub_past_a_torn_registration_leaves_no_instance_registered() {
    for earlier in [false, true] {
        let fixture = Fixture::created(&format!("r-o3-{earlier}"));
        let alpha = fixture.task("alpha", 1);
        let bravo = fixture.task("bravo", 1);
        let mut tracked = vec![add_tracked(&fixture, &fixture.manager, &alpha)];
        if earlier {
            let earlier = of_incarnation(&fixture, "inc-0");
            tracked.push(add_tracked(&fixture, &earlier, &alpha));
        }
        let mut torn = add_tracked(&fixture, &fixture.manager, &bravo);
        torn.admin = tear_registration(&fixture.manager, &torn.checkout);
        assert_enumeration_dies_on(&fixture, &torn.admin);
        tracked.push(torn);
        assert_eq!(
            fixture.manager.intents().expect("intents"),
            vec![alpha.clone(), bravo.clone()],
            "the torn slot sorts second, behind an intent the scrub reaches first"
        );

        assert_eq!(
            scrub_kind(&fixture.manager, &mut NoHooks, tasks_only)
                .expect("one torn registration does not wedge the scrub of the others"),
            2
        );
        assert!(fixture.manager.intents().expect("intents").is_empty());
        for tracked in &tracked {
            assert!(
                !tracked.checkout.exists() && !tracked.admin.exists() && !tracked.intent.exists(),
                "{} is reclaimed with its registration and its intent",
                tracked.checkout.display()
            );
        }
        for slot in [&alpha, &bravo] {
            assert!(
                registered_instances_of(&fixture.manager, slot).is_empty(),
                "Git lists no instance of {slot:?}"
            );
        }
    }
}

/// What a census of the execution root finds: every entry under the three
/// slot namespaces, and every checkout under the root Git lists, each as
/// `<namespace>/<name>`, and every file in its intents directory, by name.
///
/// The intents are a census half of their own because an intent is an
/// instance's durable record too: a resume that removed every earlier
/// instance's checkout and registration and kept their intents left the
/// first two halves clean (the follow-up C record, §6.10).
fn instance_census(manager: &WorkspaceManager) -> (Vec<String>, Vec<String>, Vec<String>) {
    let root = canonical_prefix(manager.execution_root()).expect("the root's canonical prefix");
    let mut entries = Vec::new();
    for namespace in ["tasks", "merge", "snapshots"] {
        if let Ok(listing) = fs::read_dir(root.join(namespace)) {
            for entry in listing {
                let entry = entry.expect("a namespace entry");
                entries.push(format!(
                    "{namespace}/{}",
                    entry.file_name().to_string_lossy()
                ));
            }
        }
    }
    let mut registered = Vec::new();
    for record in manager
        .worktree_records()
        .expect("Git enumerates the store")
    {
        let path = canonical_prefix(record.path()).expect("a registered path's prefix");
        if let Ok(relative) = path.strip_prefix(&root) {
            registered.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
    let intents = intent_names(manager);
    entries.sort();
    registered.sort();
    (entries, registered, intents)
}

/// Every file name in `manager`'s intents directory, sorted; none when the
/// directory does not exist.
fn intent_names(manager: &WorkspaceManager) -> Vec<String> {
    let directory = manager.execution_root().join("intents");
    let mut names = match fs::read_dir(&directory) {
        Ok(listing) => listing
            .map(|entry| {
                entry
                    .expect("an intents entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect::<Vec<_>>(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => panic!("list {}: {error}", directory.display()),
    };
    names.sort();
    names
}

/// The three halves of [`instance_census`] hold only `manager`'s own
/// instances: each entry and registration ends in `_<tag>`, and each intent
/// in `_<tag>.intent`.
fn assert_only_own_instances(manager: &WorkspaceManager, when: &str) {
    let suffix = format!("_{}", manager.instance_tag().as_str());
    let intent_suffix = format!("{suffix}.intent");
    let (entries, registered, intents) = instance_census(manager);
    for (name, own) in entries
        .iter()
        .chain(&registered)
        .map(|name| (name, &suffix))
        .chain(intents.iter().map(|name| (name, &intent_suffix)))
    {
        assert!(
            name.ends_with(own.as_str()),
            "{when}: `{name}` is not this incarnation's instance; census {entries:?} / \
             {registered:?} / {intents:?}"
        );
    }
}

/// P-1 (§4.6): three managers of one run, three fixed incarnations, leave
/// instances of shared and of distinct slots — with intents; intentless,
/// recreated by `git worktree add` at a dead tag; and registration-only, the
/// checkout removed. After each resume's walk the census, the intents
/// included, lists only the resuming incarnation's instances; after
/// finalization it lists none, and the execution root is removed.
///
/// Each resume recreates the open generation through the production path,
/// `T-DISPATCH`'s `verify_or_recreate`, as recovery does, and reclaims every
/// other slot as the walks do. A resume simulated by the walks' reclaim of
/// the open generation's slot hid that the production path kept every
/// earlier incarnation's intent of it, and a census without the intents
/// could not see them (the follow-up C record, §6.10).
#[test]
fn p1_after_every_walk_only_the_current_incarnations_instances_remain() {
    use crate::engine::topology::dispatch::{OpenGeneration, Reuse, task_slot, verify_or_recreate};
    use crate::engine::topology::seams::NoTopologyHooks;
    use crate::topology::events::CommitSha;

    let fixture = Fixture::created("p1-three-incarnations");
    let first = &fixture.manager;
    let second = of_incarnation(&fixture, "inc-2");
    let third = of_incarnation(&fixture, "inc-3");
    let open = OpenGeneration {
        key: TaskKey(0),
        generation: GenerationId(1),
        base: CommitSha(fixture.head.clone()),
        slot: task_slot(TaskKey(0), GenerationId(1)),
        source: None,
    };
    let alpha = open.slot.clone();
    let beta = fixture.task("beta", 1);
    let staging = Slot::Staging { sequence: 2 };
    let snapshot = Slot::Snapshot {
        name: SnapshotName::integration(3),
    };

    // The first incarnation: alpha and a snapshot with intents, beta's late
    // add with none, and a staging registration whose checkout is gone.
    add_tracked(&fixture, first, &alpha);
    add_tracked(&fixture, first, &snapshot);
    late_add(&fixture, &first.slot_path(&beta));
    late_add(&fixture, &first.slot_path(&staging));
    fs::remove_dir_all(first.slot_path(&staging))
        .expect("the checkout goes, the registration stays");

    // Each resume walks every slot `intents()` reports — every other slot
    // reclaimed, then the open generation alpha recreated as its own through
    // the production path — and the census then holds its own instances only.
    let resume = |manager: &WorkspaceManager, when: &str| {
        for slot in manager.intents().expect("intents") {
            if slot != alpha {
                reclaim_slot(manager, &slot).expect("the walk reclaims the slot");
            }
        }
        assert_eq!(
            verify_or_recreate(
                manager,
                &mut NoTopologyHooks::new(),
                &open,
                &open.quiescence()
            )
            .expect("the open generation, recreated as this incarnation's"),
            Reuse::Recreated {
                failure: VerifyFailure::NotRegistered
            },
            "{when}: a fresh incarnation has no instance of its own to reuse"
        );
        assert_only_own_instances(manager, when);
    };
    resume(&second, "after the second incarnation's walk");
    // The second leaves more for the third: an intentless late add of its own
    // snapshot slot and a registration-only staging slot.
    late_add(&fixture, &second.slot_path(&snapshot));
    late_add(&fixture, &second.slot_path(&staging));
    fs::remove_dir_all(second.slot_path(&staging))
        .expect("the checkout goes, the registration stays");
    resume(&third, "after the third incarnation's walk");

    for keep in [
        tasks_only as fn(&Slot) -> bool,
        snapshots_only,
        |slot: &Slot| matches!(slot, Slot::Staging { .. }),
    ] {
        scrub_kind(&third, &mut NoHooks, keep).expect("the finalizer's scrub");
    }
    third
        .remove_staging_leftovers(&mut NoHooks)
        .expect("the finalizer's staging leftovers");
    assert!(
        third
            .remove_execution_root(&mut NoHooks)
            .expect("finalization's last step"),
        "the execution root is removed"
    );
    let (entries, registered, intents) = instance_census(&third);
    assert!(
        entries.is_empty() && registered.is_empty() && intents.is_empty(),
        "after finalization the census lists nothing: {entries:?} / {registered:?} / \
         {intents:?}"
    );
}

/// `T-DISPATCH`'s recreate on resume, through the production path
/// (`verify_or_recreate`): the dispatching incarnation leaves its open
/// generation with its intent and its instance, and three fresh incarnations
/// resume it in turn. Each finds no instance of its own, recreates the
/// generation as its own, and leaves exactly one intent of it in the intents
/// directory, its own: every earlier incarnation's intent is reclaimed before
/// the replacement is created (erratum E-FUC-3's item 3; the record's §4.5).
/// At `83516466` the recreate removed the earlier instances' checkouts and
/// registrations and kept their intents, so the counts were 2, 3 and 4 (the
/// follow-up C record, §6.10).
#[test]
fn every_resume_that_recreates_an_open_generation_leaves_one_intent_its_own() {
    use crate::engine::topology::dispatch::{OpenGeneration, Reuse, task_slot, verify_or_recreate};
    use crate::engine::topology::seams::NoTopologyHooks;
    use crate::topology::events::CommitSha;

    let fixture = Fixture::created("resume-intents");
    let open = OpenGeneration {
        key: TaskKey(0),
        generation: GenerationId(1),
        base: CommitSha(fixture.head.clone()),
        slot: task_slot(TaskKey(0), GenerationId(1)),
        source: None,
    };
    let dispatched = add_tracked(&fixture, &fixture.manager, &open.slot);
    assert_eq!(intent_names(&fixture.manager).len(), 1, "the premise");

    let mut counts = Vec::new();
    let mut resumed = Vec::new();
    for incarnation in ["inc-2", "inc-3", "inc-4"] {
        let resumer = of_incarnation(&fixture, incarnation);
        let reuse = verify_or_recreate(
            &resumer,
            &mut NoTopologyHooks::new(),
            &open,
            &open.quiescence(),
        )
        .expect("the resume recreates the open generation");
        assert_eq!(
            reuse,
            Reuse::Recreated {
                failure: VerifyFailure::NotRegistered
            },
            "{incarnation}: a fresh incarnation has no instance of its own to reuse"
        );
        assert!(
            healthy_instance(&resumer, &open.slot, &fixture.head),
            "{incarnation}: the generation's worktree is its own, at the recorded base"
        );
        let census = instance_census(&resumer);
        counts.push(census.2.len());
        resumed.push((incarnation, resumer, census));
    }
    assert_eq!(
        counts,
        vec![1, 1, 1],
        "every resume reclaims the earlier incarnations' intents before it creates its own"
    );
    for (incarnation, resumer, (entries, registered, intents)) in &resumed {
        let tag = resumer.instance_tag().as_str();
        let own = format!("_{tag}");
        assert!(
            entries
                .iter()
                .chain(registered)
                .all(|name| name.ends_with(&own))
                && intents
                    .iter()
                    .all(|name| name.ends_with(&format!("{own}.intent"))),
            "{incarnation}: after its resume only its own instance and intent remain: \
             {entries:?} / {registered:?} / {intents:?}"
        );
    }
    assert!(
        !dispatched.checkout.exists() && !dispatched.admin.exists() && !dispatched.intent.exists(),
        "nothing of the dispatching incarnation's instance is left"
    );
}

/// Whether `manager`'s own instance of `slot` is registered with Git and
/// checked out at `base`.
fn healthy_instance(manager: &WorkspaceManager, slot: &Slot, base: &str) -> bool {
    let checkout = manager.slot_path(slot);
    manager
        .worktree_records()
        .expect("Git enumerates the store")
        .into_iter()
        .any(|record| crate::util::same_path(record.path(), &checkout))
        && git(&checkout, &["rev-parse", "HEAD"]) == base
}

/// A rerere resolution the user recorded the ordinary way (§5.2's witness,
/// as a fixture): `r.txt` of ten lines whose fifth is `main` on `main` and
/// `side` on a side commit; the user's own pick of the side commit
/// conflicted, they wrote the fifth line as `resolved`, ran `git rerere` and
/// aborted. The repository then enables rerere and its auto-update, so a
/// pick that reached rerere would replay the resolution and stage it.
struct RecordedResolution {
    main: String,
    side: String,
    rr_cache: PathBuf,
}

fn record_a_resolution(fixture: &Fixture) -> RecordedResolution {
    let base = &fixture.base;
    let lines = |fifth: &str| -> String {
        (1..=10)
            .map(|line| format!("line {line} {}\n", if line == 5 { fifth } else { "base" }))
            .collect()
    };
    write_file(&base.join("r.txt"), lines("base").as_bytes());
    git(base, &["add", "r.txt"]);
    git(base, &["commit", "-q", "-m", "rerere base"]);
    git(base, &["checkout", "-q", "-b", "rerere-side"]);
    write_file(&base.join("r.txt"), lines("side").as_bytes());
    git(base, &["commit", "-q", "-am", "rerere side"]);
    let side = git(base, &["rev-parse", "HEAD"]);
    git(base, &["checkout", "-q", "main"]);
    write_file(&base.join("r.txt"), lines("main").as_bytes());
    git(base, &["commit", "-q", "-am", "rerere main"]);
    let main = git(base, &["rev-parse", "HEAD"]);
    let pick = git_out(base, &["-c", "rerere.enabled=true", "cherry-pick", &side]);
    assert!(!pick.status.success(), "the user's own pick conflicts");
    write_file(&base.join("r.txt"), lines("resolved").as_bytes());
    git(base, &["-c", "rerere.enabled=true", "rerere"]);
    git(base, &["cherry-pick", "--abort"]);
    git(base, &["config", "rerere.enabled", "true"]);
    git(base, &["config", "rerere.autoUpdate", "true"]);
    let rr_cache = fixture.manager.common_git_dir().join("rr-cache");
    assert!(
        !tree_bytes(&rr_cache).is_empty(),
        "the premise: the user's resolution is recorded in the shared rr-cache"
    );
    RecordedResolution {
        main,
        side,
        rr_cache,
    }
}

/// The slot's own git dir (its registration), for its `MERGE_RR`.
fn slot_git_dir(fixture: &Fixture, slot: &Slot) -> PathBuf {
    super::fixture::registration_of(&fixture.manager, &fixture.manager.slot_path(slot))
}

/// T-RR1 (§5.2, FUC-D4-RERERE): with rerere enabled, a recorded resolution
/// and auto-update, the repair materialization's pick reads `Conflict` with
/// Git's markers in the file, `rr-cache` is the same names, sizes and bytes
/// before and after, and no `MERGE_RR` exists in the slot's git dir: the pick
/// neither read nor wrote the repository's shared rerere state.
#[test]
fn t_rr1_the_repair_pick_neither_replays_nor_records_a_rerere_resolution() {
    let fixture = Fixture::created("rerere-repair");
    let recorded = record_a_resolution(&fixture);
    let slot = fixture.task("alpha", 1);
    fixture
        .manager
        .write_intent(&mut NoHooks, &slot)
        .expect("intent");
    let path = fixture
        .manager
        .add_worktree(&mut NoHooks, &slot, &recorded.main)
        .expect("the repair slot at main");
    let before = tree_bytes(&recorded.rr_cache);
    let materialized = fixture
        .manager
        .repair_materialize(&mut NoHooks, &slot, &recorded.side)
        .expect("the pick runs");
    assert_eq!(
        materialized,
        Materialized::Conflict,
        "Git's own conflict, unresolved"
    );
    let content = fs::read_to_string(path.join("r.txt")).expect("the conflicted file");
    assert!(
        content.contains("<<<<<<<") && !content.contains("line 5 resolved"),
        "Git's markers, not the user's recorded resolution: {content}"
    );
    assert_eq!(
        tree_bytes(&recorded.rr_cache),
        before,
        "rr-cache is untouched"
    );
    assert!(
        !slot_git_dir(&fixture, &slot).join("MERGE_RR").exists(),
        "and the slot's git dir has no MERGE_RR"
    );
}

/// T-RR2: the same for the proposal pick and the state it leaves.
#[test]
fn t_rr2_the_proposal_pick_neither_replays_nor_records_a_rerere_resolution() {
    let fixture = Fixture::created("rerere-proposal");
    let recorded = record_a_resolution(&fixture);
    let slot = Slot::Staging { sequence: 1 };
    fixture
        .manager
        .write_intent(&mut NoHooks, &slot)
        .expect("intent");
    let path = fixture
        .manager
        .add_worktree(&mut NoHooks, &slot, &recorded.main)
        .expect("the staging slot at main");
    let before = tree_bytes(&recorded.rr_cache);
    fixture
        .manager
        .proposal_cherry_pick(&mut NoHooks, &slot, &recorded.side)
        .expect_err("the pick conflicts");
    assert!(
        matches!(
            fixture
                .manager
                .proposal_state(&slot, &recorded.main)
                .expect("the state the pick left"),
            ProposalState::Conflict { .. }
        ),
        "the proposal reads Git's conflict, not a resolution staged behind it"
    );
    let content = fs::read_to_string(path.join("r.txt")).expect("the conflicted file");
    assert!(
        content.contains("<<<<<<<") && !content.contains("line 5 resolved"),
        "{content}"
    );
    assert_eq!(
        tree_bytes(&recorded.rr_cache),
        before,
        "rr-cache is untouched"
    );
    assert!(!slot_git_dir(&fixture, &slot).join("MERGE_RR").exists());
}

/// The running Git's version as `(major, minor)`.
fn git_version() -> (u32, u32) {
    let text = git(Path::new("."), &["version"]);
    let numbers = text
        .trim_start_matches("git version ")
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|part| part.parse::<u32>().ok())
        .collect::<Vec<_>>();
    match numbers.as_slice() {
        [major, minor, ..] => (*major, *minor),
        _ => panic!("an unreadable `git version`: {text}"),
    }
}

/// T-CFG1 (§5.3): under the user's `worktree.useRelativePaths=true`, an
/// engine add leaves the common config byte for byte as it was; Git 2.48 and
/// later would otherwise set `extensions.relativeWorktrees` there, which Gits
/// older than 2.48 refuse. Run where Git is 2.48 or later; older Gits ignore
/// the key, so there it says so and asserts nothing.
#[test]
fn t_cfg1_an_engine_add_leaves_the_common_config_as_it_was_under_relative_paths() {
    let version = git_version();
    if version < (2, 48) {
        eprintln!("t_cfg1: git {version:?} predates worktree.useRelativePaths; nothing to assert");
        return;
    }
    let fixture = Fixture::created("relative-paths-config");
    git(
        &fixture.base,
        &["config", "worktree.useRelativePaths", "true"],
    );
    let config = fixture.manager.common_git_dir().join("config");
    let before = fs::read(&config).expect("the common config");
    fixture.add_task(&mut NoHooks, "alpha", 1);
    assert_eq!(
        String::from_utf8_lossy(&fs::read(&config).expect("the common config")),
        String::from_utf8_lossy(&before),
        "the engine's add changed the repository's format"
    );
}

/// How long a witness waits for anything it handshakes on, and how long a
/// held filter or helper waits for its release before it gives up: so a
/// mutation that breaks a handshake fails its test rather than wedging it.
#[cfg(unix)]
const WITNESS_BOUND: std::time::Duration = std::time::Duration::from_secs(60);

/// Wait, within [`WITNESS_BOUND`], for `marker` to exist.
#[cfg(unix)]
fn await_marker(marker: &Path, what: &str) {
    let deadline = std::time::Instant::now() + WITNESS_BOUND;
    while !marker.exists() {
        assert!(
            std::time::Instant::now() < deadline,
            "{what} did not happen within {WITNESS_BOUND:?}"
        );
        super::fixture::rest(std::time::Duration::from_millis(10));
    }
}

/// `path` as one POSIX shell word: single-quoted, with each `'` in it
/// closed, escaped and reopened (`'\''`). Git hands a filter's command to the
/// shell, and the witnesses' scripts assign their directory in shell, so a
/// temporary directory whose name holds a space or an apostrophe reaches both
/// whole (the follow-up C record, §6.10).
#[cfg(unix)]
fn sh_quoted(path: &Path) -> String {
    let text = path.to_str().expect("a fixture path is UTF-8");
    format!("'{}'", text.replace('\'', r"'\''"))
}

/// The DESC witness's repository (the record, §3.3.3): `.gitattributes`
/// sends `*.dat` through the `hold` filter, whose smudge runs `script`; C1
/// adds `b.dat`, and C2, on top of it, rewrites `a.txt` as `C2 base`. Returns
/// `(c1, c2)`.
#[cfg(unix)]
fn filtered_commits(fixture: &Fixture, script: &Path) -> (String, String) {
    let base = &fixture.base;
    git(
        base,
        &[
            "config",
            "filter.hold.smudge",
            &format!("sh {}", sh_quoted(script)),
        ],
    );
    git(base, &["config", "filter.hold.clean", "cat"]);
    write_file(&base.join(".gitattributes"), b"*.dat filter=hold\n");
    write_file(&base.join("b.dat"), b"held data\n");
    git(base, &["add", ".gitattributes", "b.dat"]);
    git(base, &["commit", "-q", "-m", "C1"]);
    let c1 = git(base, &["rev-parse", "HEAD"]);
    write_file(&base.join("a.txt"), b"C2 base\n");
    git(base, &["commit", "-q", "-am", "C2"]);
    let c2 = git(base, &["rev-parse", "HEAD"]);
    (c1, c2)
}

/// The successor's half of every route: its walk reclaims the slot — every
/// instance and intent the dead incarnation left — it adds its own instance
/// at C2, and the worker writes its paid edits there. Returns the successor's
/// checkout.
#[cfg(unix)]
fn successor_takes_the_slot(successor: &WorkspaceManager, slot: &Slot, c2: &str) -> PathBuf {
    for found in successor.intents().expect("intents") {
        reclaim_slot(successor, &found).expect("the successor's walk");
    }
    successor
        .write_intent(&mut NoHooks, slot)
        .expect("the successor's intent");
    let checkout = successor
        .add_worktree(&mut NoHooks, slot, c2)
        .expect("the successor's instance");
    write_file(&checkout.join("a.txt"), b"paid worker edits\n");
    checkout
}

/// What every route asserts at the end: the successor's checkout holds the
/// paid edits, its registration is there and names it, and its HEAD is C2.
#[cfg(unix)]
fn assert_successor_intact(successor: &WorkspaceManager, checkout: &Path, c2: &str) {
    assert_eq!(
        fs::read_to_string(checkout.join("a.txt")).ok().as_deref(),
        Some("paid worker edits\n"),
        "PAID EDITS LOST: the dead incarnation's writer reached the successor's checkout"
    );
    let admin = successor
        .revalidate_removal_proving(checkout, WriterProof::Unknown)
        .expect("the store binds")
        .admin;
    assert!(
        admin.is_some_and(|admin| admin.is_dir()),
        "REGISTRATION LOST: the successor's checkout is no longer registered"
    );
    assert_eq!(git(checkout, &["rev-parse", "HEAD"]), c2, "HEAD REWRITTEN");
}

/// The DESC witness's **filter** route through the production funnels
/// (the record, §3.3.3; at base, #329's `witness-locks-filter-base.log`): a
/// dead incarnation's add is held in its checkout's smudge filter; the
/// successor reclaims the slot, adds its own and writes the paid edits; the
/// filter is released, the dead add's checkout fails, and its junk removal
/// deletes its own paths by name. Under one name per slot that is the
/// successor's registration and checkout; under per-incarnation instances it
/// is the dead incarnation's own, already gone.
#[cfg(unix)]
#[test]
fn desc_filter_route_a_dead_adds_junk_removal_cannot_reach_the_successors_slot() {
    let fixture = Fixture::created("desc-filter-route");
    let hold = fixture.root.join("hold");
    create_dir(&hold);
    let script = hold.join("filter.sh");
    write_file(
        &script,
        format!(
            "dir={dir}\n\
             if mkdir \"$dir/claimed\" 2>/dev/null; then\n\
             : > \"$dir/held\"\n\
             n=0\n\
             while [ ! -e \"$dir/release\" ] && [ \"$n\" -lt 6000 ]; do sleep 0.01; n=$((n+1)); done\n\
             fi\n\
             exec cat\n",
            dir = sh_quoted(&hold)
        )
        .as_bytes(),
    );
    let (c1, c2) = filtered_commits(&fixture, &script);
    let slot = fixture.task("alpha", 1);
    let dead = of_incarnation(&fixture, "inc-0");
    dead.write_intent(&mut NoHooks, &slot)
        .expect("the dead incarnation's intent");
    let dead_add = {
        let (dead, slot) = (dead.clone(), slot.clone());
        std::thread::spawn(move || dead.add_worktree(&mut NoHooks, &slot, &c1))
    };
    await_marker(
        &hold.join("held"),
        "the dead add's checkout reaching its filter",
    );

    let checkout = successor_takes_the_slot(&fixture.manager, &slot, &c2);
    write_file(&hold.join("release"), b"");
    let dead_outcome = dead_add.join().expect("the dead add's thread");
    assert!(
        dead_outcome.is_err(),
        "the dead add's checkout lost its directory and failed: {dead_outcome:?}"
    );
    assert_successor_intact(&fixture.manager, &checkout, &c2);
}

/// The DESC witness's **helper** route (the record, §3.3.3; #329's
/// `pr329-d3-reg-recreated` result): the dead add completes, but its filter
/// started a helper that keeps the checkout's path and, once released, runs
/// `git -C <that path> reset --hard HEAD` after the successor has re-taken
/// the slot. Under one name per slot it resets the successor's checkout and
/// the paid edits are lost; under per-incarnation instances the path it kept
/// is the dead incarnation's own, which the successor's walk removed.
#[cfg(unix)]
#[test]
fn desc_helper_route_a_dead_filters_late_helper_cannot_reach_the_successors_slot() {
    let fixture = Fixture::created("desc-helper-route");
    let hold = fixture.root.join("hold");
    create_dir(&hold);
    let script = hold.join("filter.sh");
    write_file(
        &script,
        format!(
            "dir={dir}\n\
             if mkdir \"$dir/claimed\" 2>/dev/null; then\n\
             top=\"$(pwd)\"\n\
             ( unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_COMMON_DIR\n\
               n=0\n\
               while [ ! -e \"$dir/release\" ] && [ \"$n\" -lt 6000 ]; do sleep 0.01; n=$((n+1)); done\n\
               git -C \"$top\" reset -q --hard HEAD > \"$dir/helper.log\" 2>&1\n\
               : > \"$dir/helper-done\" ) < /dev/null > /dev/null 2>&1 &\n\
             fi\n\
             exec cat\n",
            dir = sh_quoted(&hold)
        )
        .as_bytes(),
    );
    let (c1, c2) = filtered_commits(&fixture, &script);
    let slot = fixture.task("alpha", 1);
    let dead = of_incarnation(&fixture, "inc-0");
    dead.write_intent(&mut NoHooks, &slot)
        .expect("the dead incarnation's intent");
    dead.add_worktree(&mut NoHooks, &slot, &c1)
        .expect("the dead add completes; its filter's helper lives on");
    assert!(
        hold.join("claimed").is_dir(),
        "the filter started its helper"
    );

    let checkout = successor_takes_the_slot(&fixture.manager, &slot, &c2);
    write_file(&hold.join("release"), b"");
    await_marker(&hold.join("helper-done"), "the helper's late reset");
    assert_successor_intact(&fixture.manager, &checkout, &c2);
}

/// The plan of a torn registration's repair reads every instance of every
/// slot: an earlier incarnation's add killed while it wrote `commondir` left
/// its registration torn, so Git's enumeration dies on it, and another slot's
/// intent removal — whose revalidation runs that enumeration — repairs it
/// through the torn slot's own removal and then succeeds. A plan that read
/// only this incarnation's instance of each slot found nothing to repair, and
/// the removal refused on every attempt.
#[test]
fn a_torn_registration_an_earlier_incarnation_left_is_repaired_with_its_slot() {
    let fixture = Fixture::created("instance-torn-earlier");
    let earlier = of_incarnation(&fixture, "inc-0");
    let alpha = fixture.task("alpha", 1);
    let beta = fixture.add_task(&mut NoHooks, "beta", 1);
    let torn = add_tracked(&fixture, &earlier, &alpha);
    tear_registration(&fixture.manager, &torn.checkout);
    assert_enumeration_dies_on(&fixture, &torn.admin);

    fixture
        .manager
        .remove_worktree(&mut NoHooks, &beta)
        .expect("beta's removal binds by gitdir and lists nothing");
    fixture
        .manager
        .remove_intent(&mut NoHooks, &beta)
        .expect("beta's intent removal repairs the earlier torn registration first");
    assert!(!torn.admin.exists() && !torn.checkout.exists());
    assert!(
        torn.intent.exists(),
        "the repair removes the checkout and the registration and leaves the intent, for the \
         step that owns the slot"
    );
    fixture
        .manager
        .worktree_records()
        .expect("Git enumerates again");
}

/// The torn-registration repair removes only the instance its plan proved
/// torn. The successor runs `alpha` and has written its paid edits there; a
/// dead incarnation's late add recreates its own instance of `alpha`, with no
/// intent, beside it. Then the successor retires the unrelated slot `beta`,
/// and its intent removal revalidates through Git's enumeration.
///
/// - **Whole**, the control, first: the late add's registration is whole,
///   nothing is repaired, and the retirement of `beta` touches no instance of
///   `alpha`.
/// - **Torn**: the late add's registration is left as a kill leaves it, so
///   the enumeration dies on it and the repair runs. It removes the dead
///   instance's checkout and registration and nothing else: the successor's
///   checkout, its registration, its intent and its paid edits stay, its
///   instance still verifies, and Git enumerates again. A repair that removed
///   every instance of the torn instance's slot deleted the successor's live
///   worktree (executed at `83516466`, the follow-up C record, §6.10).
#[test]
fn a_torn_earlier_instances_repair_leaves_the_successors_live_instance_of_its_slot() {
    for torn in [false, true] {
        let shape = if torn { "torn" } else { "whole" };
        let fixture = Fixture::created(&format!("instance-beside-live-{shape}"));
        let earlier = of_incarnation(&fixture, "inc-0");
        let alpha = fixture.add_task(&mut NoHooks, "alpha", 1);
        let beta = fixture.add_task(&mut NoHooks, "beta", 1);
        let live = fixture.manager.slot_path(&alpha);
        let live_admin = super::fixture::registration_of(&fixture.manager, &live);
        let paid = live.join("paid-edits.txt");
        write_file(&paid, b"the successor's paid edits\n");

        let dead = earlier.slot_path(&alpha);
        late_add(&fixture, &dead);
        let dead_admin = if torn {
            let admin = tear_registration(&fixture.manager, &dead);
            assert_enumeration_dies_on(&fixture, &admin);
            admin
        } else {
            super::fixture::registration_of(&fixture.manager, &dead)
        };
        assert!(
            !earlier.intent_path(&alpha).exists(),
            "{shape}: the late add wrote no intent"
        );

        fixture
            .manager
            .remove_worktree(&mut NoHooks, &beta)
            .expect("beta's removal binds by gitdir and lists nothing");
        fixture
            .manager
            .remove_intent(&mut NoHooks, &beta)
            .expect("beta's intent removal, past the repair when there is a tear");
        assert_eq!(
            fs::read(&paid).ok().as_deref(),
            Some(b"the successor's paid edits\n".as_slice()),
            "{shape}: PAID EDITS LOST: the successor's checkout was removed"
        );
        assert!(
            live_admin.is_dir(),
            "{shape}: REGISTRATION LOST: the successor's instance is no longer registered"
        );
        assert!(
            fixture.manager.intent_path(&alpha).exists(),
            "{shape}: the successor's intent stays"
        );
        assert_eq!(
            fixture
                .manager
                .verify_worktree(
                    &mut NoHooks,
                    &alpha,
                    &Quiescence::AtBase(fixture.head.clone())
                )
                .expect("verify"),
            Ok(()),
            "{shape}: the successor's instance still verifies as its own"
        );
        assert_eq!(
            !dead.exists() && !dead_admin.exists(),
            torn,
            "{shape}: the repair removes the torn instance, and only a repair removes one"
        );
        fixture
            .manager
            .worktree_records()
            .expect("Git enumerates again");
    }
}

/// P-2 (§4.6): child processes, each a dead incarnation of the run under its
/// own fixed incarnation id, are killed inside the add in turn — before Git
/// ran and after it finished — and the parent then resumes as a further
/// incarnation. After its walk the census lists only the parent's own
/// instances; after finalization it lists none, and the root is removed.
#[test]
fn p2_instances_of_killed_incarnations_in_other_processes_are_reclaimed_by_the_resume() {
    let fixture = Fixture::created("p2-killed-incarnations");
    let root = fixture.root.clone();
    for (incarnation, phase, slot) in [
        ("inc-a", "before", "task"),
        ("inc-b", "after", "task"),
        ("inc-c", "after", "snapshot"),
        ("inc-d", "before", "snapshot"),
    ] {
        let status = run_kill_child_within(
            "workspace_manager::tests::instance_kill_child",
            &[
                (INSTANCE_KILL_ROOT, root.as_os_str()),
                (INSTANCE_KILL_INCARNATION, OsStr::new(incarnation)),
                (INSTANCE_KILL_PHASE, OsStr::new(phase)),
                (INSTANCE_KILL_SLOT, OsStr::new(slot)),
            ],
            std::time::Duration::from_secs(60),
        )
        .unwrap_or_else(|| panic!("the {incarnation} child ended within its bound"));
        assert!(
            died_by_abort(&status),
            "the {incarnation} child died by abort at its {phase} phase: {status:?}"
        );
    }
    let alpha = fixture.task("alpha", 1);
    let snapshot = Slot::Snapshot {
        name: SnapshotName::integration(7),
    };
    for (slot, incarnation) in [(&alpha, "inc-b"), (&snapshot, "inc-c")] {
        assert!(
            of_incarnation(&fixture, incarnation)
                .slot_path(slot)
                .join(".git")
                .exists(),
            "the premise: {incarnation}'s add finished before it died"
        );
    }

    for slot in fixture.manager.intents().expect("intents") {
        reclaim_slot(&fixture.manager, &slot).expect("the resume's walk");
    }
    fixture
        .manager
        .write_intent(&mut NoHooks, &alpha)
        .expect("the open generation's intent");
    fixture
        .manager
        .add_worktree(&mut NoHooks, &alpha, &fixture.head)
        .expect("the open generation, as the resume's own");
    assert_only_own_instances(&fixture.manager, "after the resume's walk");

    scrub_kind(&fixture.manager, &mut NoHooks, tasks_only).expect("the task scrub");
    fixture
        .manager
        .remove_staging_leftovers(&mut NoHooks)
        .expect("the staging leftovers");
    assert!(
        fixture
            .manager
            .remove_execution_root(&mut NoHooks)
            .expect("finalization's last step"),
        "the root is removed"
    );
    let (entries, registered, intents) = instance_census(&fixture.manager);
    assert!(
        entries.is_empty() && registered.is_empty() && intents.is_empty(),
        "{entries:?} / {registered:?} / {intents:?}"
    );
}

/// The real `git` on this process's `PATH`, which P-3's stub hands on to.
#[cfg(unix)]
fn real_git() -> PathBuf {
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .map(|directory| directory.join("git"))
        .find(|candidate| candidate.is_file())
        .expect("a git on PATH")
}

#[cfg(unix)]
const LATE_ADD_REAL_GIT: &str = "UPSTROKE_PR330_LATE_ADD_REAL_GIT";

/// P-3 (§4.3, through the production funnels): a dead incarnation's add is
/// held before its Git runs, in a process of its own. The successor's walk
/// reclaims the slot (the dead intent; nothing else exists yet), adds its own
/// instance and writes the paid edits; the add is released and its Git
/// recreates the dead instance — checkout and registration, no intent. The
/// successor's instance is untouched; `intents()` reaches the late instance;
/// the final sweep removes it and keeps the successor's, whose checkout keeps
/// the root.
#[cfg(unix)]
#[test]
fn p3_a_late_add_released_after_the_successors_walk_is_found_and_removed() {
    let fixture = Fixture::created("p3-late-add");
    let hold = fixture.root.join("hold");
    create_dir(&hold);
    let stub_dir = fixture.root.join("stub");
    create_dir(&stub_dir);
    let stub = stub_dir.join("git");
    write_file(
        &stub,
        b"#!/bin/sh\n\
          case \" $* \" in\n\
          *\" worktree add \"*)\n\
          if mkdir \"$UPSTROKE_PR330_LATE_ADD_HOLD/claimed\" 2>/dev/null; then\n\
          : > \"$UPSTROKE_PR330_LATE_ADD_HOLD/held\"\n\
          n=0\n\
          while [ ! -e \"$UPSTROKE_PR330_LATE_ADD_HOLD/release\" ] && [ \"$n\" -lt 6000 ]; do sleep 0.01; n=$((n+1)); done\n\
          fi ;;\n\
          esac\n\
          exec \"$UPSTROKE_PR330_LATE_ADD_REAL_GIT\" \"$@\"\n",
    );
    super::fixture::set_mode(&stub, 0o755);
    let real = real_git();
    let mut path = std::ffi::OsString::from(stub_dir.as_os_str());
    path.push(":");
    path.push(std::env::var_os("PATH").unwrap_or_default());
    let child = {
        let (root, hold, real, path) = (
            fixture.root.clone(),
            hold.clone(),
            real.clone(),
            path.clone(),
        );
        std::thread::spawn(move || {
            crate::agent::proc::test_support::run_test_isolated(
                "workspace_manager::tests::late_add_child",
                &[
                    (LATE_ADD_ROOT, root.as_os_str()),
                    (LATE_ADD_HOLD, hold.as_os_str()),
                    (LATE_ADD_REAL_GIT, real.as_os_str()),
                    ("PATH", path.as_os_str()),
                ],
                WITNESS_BOUND,
            )
        })
    };
    await_marker(&hold.join("held"), "the dead add reaching its Git");

    let slot = fixture.task("alpha", 1);
    let (_, c2) = (fixture.head.clone(), fixture.head.clone());
    let checkout = successor_takes_the_slot(&fixture.manager, &slot, &c2);
    write_file(&hold.join("release"), b"");
    let ended = child.join().expect("the child's thread");
    assert!(
        ended.status.is_some_and(|status| status.success())
            && ended.stdout.contains("late add: added"),
        "the dead add ran late and recreated its instance: {ended}"
    );
    let dead = of_incarnation(&fixture, "inc-0");
    let late = dead.slot_path(&slot);
    assert!(late.join(".git").exists(), "the late add's checkout");
    let late_admin = super::fixture::registration_of(&fixture.manager, &late);
    assert!(
        !dead.intent_path(&slot).exists(),
        "and no intent: the walk removed it"
    );
    assert_successor_intact(&fixture.manager, &checkout, &c2);
    assert_eq!(
        fixture.manager.intents().expect("intents"),
        vec![slot.clone()]
    );

    assert!(
        !fixture
            .manager
            .remove_execution_root(&mut NoHooks)
            .expect("finalization's last step"),
        "the successor's own checkout keeps the root"
    );
    assert!(
        !late.exists() && !late_admin.exists(),
        "the sweep removed the late instance"
    );
    assert_successor_intact(&fixture.manager, &checkout, &c2);
}

/// §4.5 on Windows: an open handle blocks a deletion there, so an earlier
/// incarnation's instance a process still holds open refuses the reclaim
/// after the removal's whole retry budget, resumably, and nothing retains
/// it; once the handle closes, the next reclaim converges.
#[cfg(windows)]
#[test]
fn an_earlier_instance_held_open_refuses_the_reclaim_until_its_handle_closes() {
    use std::os::windows::fs::OpenOptionsExt as _;

    use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;

    let fixture = Fixture::created("instance-held-open");
    let earlier = of_incarnation(&fixture, "inc-0");
    let slot = fixture.task("alpha", 1);
    let tracked = add_tracked(&fixture, &earlier, &slot);
    let handle = fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(tracked.checkout.join("a.txt"))
        .expect("hold a file of the earlier instance open, deletion not shared");
    let error = fixture
        .manager
        .remove_worktree(&mut NoHooks, &slot)
        .expect_err("an earlier instance held open refuses the reclaim");
    assert!(
        matches!(
            &error,
            UpstrokeError::Filesystem {
                operation: "remove",
                ..
            }
        ),
        "the refusal is the removal's own: {error}"
    );
    assert!(
        tracked.intent.exists(),
        "its intent stays for the next reclaim"
    );
    drop(handle);
    reclaim_slot(&fixture.manager, &slot).expect("once the handle closes, the reclaim converges");
    assert!(!tracked.checkout.exists() && !tracked.admin.exists());
    assert!(fixture.manager.intents().expect("intents").is_empty());
}

/// FUC-D5-WINPATHBYTES, natively: under a private root whose name carries
/// two `é`, a task instance whose `$GIT_DIR` Git for Windows would be handed
/// is 221 UTF-8 bytes — 217 ASCII characters and the two `é`, 219 characters
/// — is refused at once, before any registry access: no destination is
/// made and nothing is registered. One byte less, 220, is added.
#[cfg(windows)]
#[test]
fn a_git_dir_over_the_byte_budget_is_refused_before_any_registry_access_on_windows() {
    use crate::runner::host::GitdirRule;

    let fixture = Fixture::created("winpathbytes");
    let private = fixture.root.join("p\u{e9}\u{e9}");
    create_dir(&private);
    let manager =
        WorkspaceManager::derive(&fixture.base, &private, super::fixture::RUN_ID, "inc-1")
            .expect("derive under the accented private root");
    manager
        .create_execution_root(&mut NoHooks)
        .expect("the execution root");
    let root = manager
        .execution_root()
        .to_str()
        .expect("the execution root is UTF-8")
        .len();
    // `<root>/tasks/k<key>-g1_<tag>/.git`, as Git for Windows renders it.
    let fixed = root + "/tasks/k".len() + "-g1_".len() + InstanceTag::LEN + "/.git".len();
    let task = |bytes: usize| {
        let key = "a".repeat(
            bytes
                .checked_sub(fixed)
                .filter(|length| *length > 0)
                .unwrap_or_else(|| panic!("the fixture's root ({root} bytes) leaves no key")),
        );
        fixture.task(&key, 1)
    };
    let rendered = |slot: &Slot| {
        let git_dir = manager.slot_path(slot).join(".git");
        String::from_utf8(GitdirRule::Windows.spelling(git_dir.to_str().expect("UTF-8").as_bytes()))
            .expect("UTF-8")
    };

    let over = task(221);
    let spelled = rendered(&over);
    assert_eq!(
        (spelled.len(), spelled.chars().count()),
        (221, 219),
        "{spelled}"
    );
    manager
        .write_intent(&mut NoHooks, &over)
        .expect("the intent");
    let store = manager.common_git_dir().join("worktrees");
    let registered_before = fs::read_dir(&store).map_or(0, |entries| entries.count());
    let message = match manager.add_worktree(&mut NoHooks, &over, &fixture.head) {
        Err(UpstrokeError::Refused { message }) => message,
        other => panic!("the add is refused by its budget: {other:?}"),
    };
    assert!(
        message.contains("221 bytes") && message.contains("220"),
        "{message}"
    );
    assert!(
        !manager.slot_path(&over).exists(),
        "no destination was made: the refusal came before the registry phase"
    );
    assert_eq!(
        fs::read_dir(&store).map_or(0, |entries| entries.count()),
        registered_before,
        "and nothing was registered"
    );

    let at = task(220);
    assert_eq!(rendered(&at).len(), 220);
    manager.write_intent(&mut NoHooks, &at).expect("the intent");
    let added = manager
        .add_worktree(&mut NoHooks, &at, &fixture.head)
        .expect("a 220-byte $GIT_DIR is within Git for Windows' budget");
    assert!(added.join(".git").exists());
}
