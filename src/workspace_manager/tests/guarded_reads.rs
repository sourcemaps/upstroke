// Allowlist placement: the funnel section of `effects/allowlist.toml`.
#![allow(clippy::disallowed_methods, clippy::disallowed_types)]
#![forbid(clippy::disallowed_macros)]

use super::*;

// ---------------------------------------------------------------------------
// The external-prune follow-up (F): closure 1 at the manager — its reader,
// placement 1's error arm, answer arm and write-tree pre-check, and C1-CLS —
// and closure 3, each red at C `66e8a393` where the defect lives
// (`PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`).
// ---------------------------------------------------------------------------

/// Acts once at `phase` of `site`, then proceeds.
struct AtPhase<F: FnMut()> {
    site: EffectSiteId,
    phase: HookPhase,
    act: Option<F>,
}

impl<F: FnMut()> AtPhase<F> {
    fn new(site: EffectSiteId, phase: HookPhase, act: F) -> Self {
        Self {
            site,
            phase,
            act: Some(act),
        }
    }
}

impl<F: FnMut()> EffectHooks for AtPhase<F> {
    fn phase(&mut self, site: EffectSiteId, phase: HookPhase) -> Injection {
        if site == self.site && phase == self.phase {
            if let Some(mut act) = self.act.take() {
                act();
            }
        }
        Injection::Proceed
    }

    fn refusal_cause(&self) -> Option<String> {
        None
    }
}

/// A prune that no engine process started, deleting the whole entry of the
/// checkout at `checkout` the way #329's d9 witnesses do: its `.git` set aside,
/// `git worktree prune --expire=now` from the base, and the `.git` put back.
/// When the entry was the store's last, Git removes the store too.
fn prune_entry_of(fixture: &Fixture, checkout: &Path) {
    let pointer = checkout.join(".git");
    let aside = checkout.join(".git-set-aside");
    fs::rename(&pointer, &aside).expect("set the checkout's .git aside");
    git(&fixture.base, &["worktree", "prune", "--expire=now"]);
    fs::rename(&aside, &pointer).expect("put the checkout's .git back");
}

/// The registry refusal a case expects, naming `words`, or why not.
fn refused_naming<T: std::fmt::Debug>(
    result: Result<T, UpstrokeError>,
    words: &[&str],
) -> Result<(), String> {
    match result {
        Err(UpstrokeError::RegistryRefused { message }) => {
            match words.iter().find(|word| !message.contains(**word)) {
                None => Ok(()),
                Some(word) => Err(format!("a registry refusal not naming `{word}`: {message}")),
            }
        }
        other => Err(format!("expected a registry refusal, got {other:?}")),
    }
}

/// T-C1-READERS, the manager's reader: closure 1's predicate (#329's record
/// §8.4) over every construction `fixture::registration_constructions` makes —
/// whole; each name of the entry removed, or the first three empty; the `.git`
/// removed, a directory or no pointer; an entry outside the store; `commondir`
/// or `gitdir` naming another repository or checkout; relative pointers; a link
/// in place of `index`. The topology twin is run over the same constructions
/// beside it (`dispatch::tests`).
#[test]
fn closure_one_reader_answers_every_registration_construction_as_the_check_states() {
    let fixture = Fixture::created("f-readers-manager");
    let common = fixture.manager.common_git_dir().to_path_buf();
    let cases: Vec<(String, Result<(), String>)> =
        super::fixture::registration_constructions(&fixture)
            .into_iter()
            .map(|construction| {
                let found = super::fixture::manager_registration_found(
                    &construction.checkout,
                    &common,
                    &super::ComposedCommonDir::Unknown,
                );
                let outcome = match (construction.expected, &found) {
                    (Ok(()), Ok(())) => Ok(()),
                    (Err(words), Err(text)) if text.contains(words) => Ok(()),
                    (expected, found) => Err(format!("expected {expected:?}, found {found:?}")),
                };
                (construction.label.to_owned(), outcome)
            })
            .collect();
    assert!(cases.len() >= 15, "every construction was made");
    report_cases(&cases);
}

/// T-C1-CLS-MGR's construction, W-CLS-GIT's (the design's executed E4
/// `md_empty_head`) at the manager: the candidate changes `a.txt` on `seed`, the
/// integration head is a commit of the empty tree on `seed`, the staging slot is
/// added at the head and the proposal pick stops on a modify/delete conflict.
/// Returns the staging slot, its checkout, its registration entry and the head.
fn a_conflicted_staging_pick_on_an_empty_head(
    fixture: &Fixture,
) -> (Slot, PathBuf, PathBuf, String) {
    let base = &fixture.base;
    git(base, &["checkout", "-q", "-b", "candidate", &fixture.seed]);
    fs::write(base.join("a.txt"), "changed\n").expect("the candidate's change");
    git(
        base,
        &["commit", "-q", "-am", "the candidate changes a.txt"],
    );
    let candidate = git(base, &["rev-parse", "HEAD"]);
    git(base, &["checkout", "-q", "main"]);
    let empty = git(base, &["hash-object", "-t", "tree", "-w", "/dev/null"]);
    let head = git(
        base,
        &[
            "commit-tree",
            &empty,
            "-p",
            &fixture.seed,
            "-m",
            "the head deletes every file",
        ],
    );
    let staging = Slot::Staging { sequence: 1 };
    fixture
        .manager
        .write_intent(&mut NoHooks, &staging)
        .expect("the staging intent");
    let checkout = fixture
        .manager
        .add_worktree(&mut NoHooks, &staging, &head)
        .expect("the staging worktree at the empty head");
    fixture
        .manager
        .proposal_cherry_pick(&mut NoHooks, &staging, &candidate)
        .expect_err("the pick stops on the modify/delete conflict");
    let entry = super::fixture::registration_of(&fixture.manager, &checkout);
    (staging, checkout, entry, head)
}

/// T-C1-CLS-MGR (FUF-R1-1): with the staging entry's `index` deleted after the
/// pick, Git reads an empty index, and the modify/delete conflict on an empty
/// head reads as the empty pick. At C the classification answers
/// `Ok(ProposalState::Empty)`; under F it is a registry refusal naming the
/// staging checkout and `index`, and the classification wrote no index.
#[test]
fn a_staging_conflict_read_through_a_deleted_index_is_refused_not_classified_as_an_empty_pick() {
    let fixture = Fixture::created("f-cls-index");
    let (staging, checkout, entry, head) = a_conflicted_staging_pick_on_an_empty_head(&fixture);
    fs::remove_file(entry.join("index")).expect("a prune deletes the staging entry's index");
    let outcome = refused_naming(
        fixture.manager.proposal_state(&staging, &head),
        &[&checkout.display().to_string(), "index"],
    );
    report_cases(&[("index deleted after the pick".to_owned(), outcome)]);
    assert!(
        !entry.join("index").exists(),
        "the classification wrote no index"
    );
}

/// T-C1-CLS-MGR's control: with the registration whole, the same pick is
/// classified as the conflict on `a.txt`, at C and under F.
#[test]
fn a_staging_conflict_in_a_whole_registration_is_classified_as_the_conflict() {
    let fixture = Fixture::created("f-cls-whole");
    let (staging, _, _, head) = a_conflicted_staging_pick_on_an_empty_head(&fixture);
    match fixture.manager.proposal_state(&staging, &head) {
        Ok(ProposalState::Conflict { paths }) => {
            assert!(format!("{paths:?}").contains("a.txt"), "{paths:?}");
        }
        other => panic!("the genuine conflict: {other:?}"),
    }
}

/// T-C1-CLS-MGR, `gitdir` deleted: Git still reads the conflict genuinely
/// (E4), so at C the classification answers `Conflict`; under F the check finds
/// the registration not whole and refuses it — the liveness cost the design
/// pins (§9.2).
#[test]
fn a_staging_conflict_read_with_its_gitdir_deleted_is_refused_though_git_reads_it_genuinely() {
    let fixture = Fixture::created("f-cls-gitdir");
    let (staging, _, entry, head) = a_conflicted_staging_pick_on_an_empty_head(&fixture);
    fs::remove_file(entry.join("gitdir")).expect("a prune deletes the staging entry's gitdir");
    let outcome = refused_naming(fixture.manager.proposal_state(&staging, &head), &["gitdir"]);
    report_cases(&[("gitdir deleted after the pick".to_owned(), outcome)]);
}

/// T-C1-CLS-MGR, `HEAD` deleted: the classification's first read fails, which
/// at C is a Git error and under F a registry refusal naming `HEAD`.
#[test]
fn a_staging_classification_whose_head_is_deleted_is_a_registry_refusal_not_a_git_error() {
    let fixture = Fixture::created("f-cls-head");
    let (staging, _, entry, head) = a_conflicted_staging_pick_on_an_empty_head(&fixture);
    fs::remove_file(entry.join("HEAD")).expect("a prune deletes the staging entry's HEAD");
    let outcome = refused_naming(fixture.manager.proposal_state(&staging, &head), &["HEAD"]);
    report_cases(&[("HEAD deleted after the pick".to_owned(), outcome)]);
}

/// T-C1-P1-DIFF: `candidate_diff` in a slot whose entry was pruned whole (the
/// store kept by another registration, and the store gone with it), or that
/// lost `HEAD` or `commondir`, fails in Git; at C that is `UpstrokeError::Git`,
/// which the verification settles as an outage, and under F a registry refusal
/// naming what the check found missing.
#[test]
fn a_candidate_diff_in_a_slot_whose_registration_is_not_whole_is_a_registry_refusal() {
    let mut cases = Vec::new();
    {
        let fixture = Fixture::created("f-diff-pruned-store-kept");
        let _keeper = fixture.add_task(&mut NoHooks, "keeper", 1);
        let slot = fixture.add_task(&mut NoHooks, "alpha", 1);
        let checkout = fixture.manager.slot_path(&slot);
        prune_entry_of(&fixture, &checkout);
        assert!(
            fixture.manager.common_git_dir().join("worktrees").is_dir(),
            "the keeper keeps the store"
        );
        let outcome = refused_naming(
            fixture
                .manager
                .candidate_diff(&slot, &fixture.seed, &fixture.head),
            &["gitdir"],
        );
        cases.push(("pruned whole, the store kept".to_owned(), outcome));
    }
    {
        let fixture = Fixture::created("f-diff-pruned-store-gone");
        let slot = fixture.add_task(&mut NoHooks, "alpha", 1);
        let checkout = fixture.manager.slot_path(&slot);
        prune_entry_of(&fixture, &checkout);
        assert!(
            !fixture.manager.common_git_dir().join("worktrees").exists(),
            "the last entry's prune took the store"
        );
        let outcome = refused_naming(
            fixture
                .manager
                .candidate_diff(&slot, &fixture.seed, &fixture.head),
            &["worktrees"],
        );
        cases.push(("pruned whole, the store gone".to_owned(), outcome));
    }
    for name in ["HEAD", "commondir"] {
        let fixture = Fixture::created(&format!("f-diff-{name}"));
        let slot = fixture.add_task(&mut NoHooks, "alpha", 1);
        let checkout = fixture.manager.slot_path(&slot);
        let entry = super::fixture::registration_of(&fixture.manager, &checkout);
        fs::remove_file(entry.join(name)).expect("remove one name");
        let outcome = refused_naming(
            fixture
                .manager
                .candidate_diff(&slot, &fixture.seed, &fixture.head),
            &[name],
        );
        cases.push((format!("{name} removed"), outcome));
    }
    report_cases(&cases);
}

/// T-C1-P1-DIFF's control: in a whole registration a diff against a tree that
/// is not an object is the Git error it was, at C and under F.
#[test]
fn a_candidate_diff_failing_in_a_whole_registration_stays_a_git_error() {
    let fixture = Fixture::created("f-diff-whole");
    let slot = fixture.add_task(&mut NoHooks, "alpha", 1);
    let absent = "0123456789abcdef0123456789abcdef01234567";
    match fixture.manager.candidate_diff(&slot, &fixture.seed, absent) {
        Err(UpstrokeError::Git { .. }) => {}
        other => panic!("the Git error, unrefused: {other:?}"),
    }
}

/// T-C1-P1-OWNERR (M-C1b's witness): a materialization whose source is no
/// object, in a slot whose `gitdir` alone is removed. `read-tree` and
/// `diff-files` succeed and the pick, run through `git` rather than `git_ok`,
/// fails without unmerged entries (E5 `mc1b_own_error`), so the only Git error
/// is the one `repair_materialize` constructs itself: at C it is returned as
/// `UpstrokeError::Git`; under F the error arm, applied to the function's own
/// error as it returns, refuses it.
#[test]
fn a_materializations_own_git_error_in_a_slot_whose_gitdir_is_gone_is_a_registry_refusal() {
    let fixture = Fixture::created("f-ownerr-gitdir");
    let slot = fixture.add_task(&mut NoHooks, "alpha", 1);
    let checkout = fixture.manager.slot_path(&slot);
    let entry = super::fixture::registration_of(&fixture.manager, &checkout);
    fs::remove_file(entry.join("gitdir")).expect("a prune deletes gitdir alone");
    let outcome = refused_naming(
        fixture.manager.repair_materialize(
            &mut NoHooks,
            &slot,
            "0123456789abcdef0123456789abcdef01234567",
        ),
        &["left no unmerged entry", "gitdir"],
    );
    report_cases(&[("the pick's source is no object".to_owned(), outcome)]);
}

/// T-C1-P1-OWNERR's control: in a whole registration the function's own Git
/// error is returned unrefused, at C and under F.
#[test]
fn a_materializations_own_git_error_in_a_whole_registration_stays_a_git_error() {
    let fixture = Fixture::created("f-ownerr-whole");
    let slot = fixture.add_task(&mut NoHooks, "alpha", 1);
    match fixture.manager.repair_materialize(
        &mut NoHooks,
        &slot,
        "0123456789abcdef0123456789abcdef01234567",
    ) {
        Err(UpstrokeError::Git { message }) => {
            assert!(message.contains("left no unmerged entry"), "{message}");
        }
        other => panic!("the function's own Git error: {other:?}"),
    }
}

/// T-C1-P1-CLASSIFIED (control; M-C1c's witness, with #329's T16): a populated
/// destination at `add_snapshot` is Git state before `git worktree add` runs,
/// at C and under F: placement 1 is not applied to the adds, whose answers are
/// #329's access's.
#[test]
fn a_populated_snapshot_destination_stays_git_state_and_is_not_read_as_a_registration() {
    let fixture = Fixture::created("f-classified-snapshot");
    let name = SnapshotName::gates(0, 1, 1);
    let slot = Slot::Snapshot { name: name.clone() };
    let destination = fixture.manager.slot_path(&slot);
    fs::create_dir_all(&destination).expect("the destination");
    fs::write(destination.join("leftover"), b"x\n").expect("an entry in it");
    match fixture.manager.add_snapshot(
        &mut NoHooks,
        &name,
        &SnapshotInput::Commit(oid(&fixture.head)),
    ) {
        Err(UpstrokeError::Git { message }) => {
            assert!(
                message.contains("`git worktree add` did not run"),
                "{message}"
            );
        }
        other => panic!("Git state, as #329's access answers it: {other:?}"),
    }
    assert!(destination.join("leftover").exists(), "the entry is kept");
}

/// A commit on `parent` whose `a.txt` holds `text`, made in the base.
fn commit_changing_a(fixture: &Fixture, parent: &str, text: &str, branch: &str) -> String {
    let base = &fixture.base;
    git(base, &["checkout", "-q", "-b", branch, parent]);
    fs::write(base.join("a.txt"), text).expect("change a.txt");
    git(base, &["commit", "-q", "-am", branch]);
    let commit = git(base, &["rev-parse", "HEAD"]);
    git(base, &["checkout", "-q", "main"]);
    commit
}

/// A task slot added at `at`, with its checkout and registration entry.
fn task_slot_at(fixture: &Fixture, key: &str, at: &str) -> (Slot, PathBuf, PathBuf) {
    let slot = fixture.task(key, 1);
    fixture
        .manager
        .write_intent(&mut NoHooks, &slot)
        .expect("the intent");
    let checkout = fixture
        .manager
        .add_worktree(&mut NoHooks, &slot, at)
        .expect("the worktree");
    let entry = super::fixture::registration_of(&fixture.manager, &checkout);
    (slot, checkout, entry)
}

/// T-C1-ANS-MATERIALIZE (a): a repair's materialization that conflicts as
/// expected, in a slot whose `gitdir` alone is removed, reads the conflict
/// genuinely (E5): at C `Materialized::Conflict`; under F the observation is
/// not used, and the materialization is a registry refusal.
#[test]
fn a_materialization_observed_with_its_gitdir_deleted_is_a_registry_refusal() {
    let fixture = Fixture::created("f-materialize-gitdir");
    let ours = commit_changing_a(&fixture, &fixture.seed, "ours\n", "ours");
    let theirs = commit_changing_a(&fixture, &fixture.seed, "theirs\n", "theirs");
    let (slot, _, entry) = task_slot_at(&fixture, "alpha", &ours);
    fs::remove_file(entry.join("gitdir")).expect("a prune deletes gitdir alone");
    let outcome = refused_naming(
        fixture
            .manager
            .repair_materialize(&mut NoHooks, &slot, &theirs),
        &["materialization", "gitdir"],
    );
    report_cases(&[("a conflicting pick, gitdir deleted".to_owned(), outcome)]);
}

/// T-C1-ANS-MATERIALIZE (b) and (c): the materialization's observation is
/// returned only while the registration is whole. The engine's suite has no
/// seam between the pick and the observation inside the funnel, so the slot's
/// `index` is removed at the funnel's `After` phase, after the observation's
/// reads and before the answer is returned: at C the function returns its
/// observation (`Empty` for an already-present change, `Clean` for a change
/// that applies onto an empty head) with the registration not whole; under F it
/// is a registry refusal naming `index`. (E5 executed the misread itself: read
/// through the deleted index the two answers swap.)
#[test]
fn a_materializations_observation_returned_after_its_index_is_deleted_is_a_registry_refusal() {
    let mut cases = Vec::new();
    {
        let fixture = Fixture::created("f-materialize-present");
        let (slot, _, entry) = task_slot_at(&fixture, "alpha", &fixture.side.clone());
        let index = entry.join("index");
        let mut hooks = AtPhase::new(
            EffectSiteId::Object(ObjectSite::RepairMaterialize),
            HookPhase::After,
            || fs::remove_file(&index).expect("delete the index after the observation"),
        );
        let outcome = refused_naming(
            fixture
                .manager
                .repair_materialize(&mut hooks, &slot, &fixture.side),
            &["index"],
        );
        cases.push(("(b) the change already present".to_owned(), outcome));
    }
    {
        let fixture = Fixture::created("f-materialize-empty-head");
        let empty = git(
            &fixture.base,
            &["hash-object", "-t", "tree", "-w", "/dev/null"],
        );
        let head = git(
            &fixture.base,
            &[
                "commit-tree",
                &empty,
                "-p",
                &fixture.seed,
                "-m",
                "an empty head",
            ],
        );
        let (slot, _, entry) = task_slot_at(&fixture, "alpha", &head);
        let index = entry.join("index");
        let mut hooks = AtPhase::new(
            EffectSiteId::Object(ObjectSite::RepairMaterialize),
            HookPhase::After,
            || fs::remove_file(&index).expect("delete the index after the observation"),
        );
        let outcome = refused_naming(
            fixture
                .manager
                .repair_materialize(&mut hooks, &slot, &fixture.side),
            &["index"],
        );
        cases.push((
            "(c) a change that applies onto an empty head".to_owned(),
            outcome,
        ));
    }
    report_cases(&cases);
}

/// T-C1-ANS-MATERIALIZE's controls: with the registration whole the three
/// shapes are observed as they are, at C and under F.
#[test]
fn a_materialization_in_a_whole_registration_observes_conflict_empty_and_clean() {
    let fixture = Fixture::created("f-materialize-whole");
    let ours = commit_changing_a(&fixture, &fixture.seed, "ours\n", "ours");
    let theirs = commit_changing_a(&fixture, &fixture.seed, "theirs\n", "theirs");
    let (conflict, _, _) = task_slot_at(&fixture, "conflict", &ours);
    assert_eq!(
        fixture
            .manager
            .repair_materialize(&mut NoHooks, &conflict, &theirs)
            .expect("the conflict is a result"),
        Materialized::Conflict
    );
    let (present, _, _) = task_slot_at(&fixture, "present", &fixture.side.clone());
    assert_eq!(
        fixture
            .manager
            .repair_materialize(&mut NoHooks, &present, &fixture.side)
            .expect("an already-present change"),
        Materialized::Empty
    );
    let empty = git(
        &fixture.base,
        &["hash-object", "-t", "tree", "-w", "/dev/null"],
    );
    let head = git(
        &fixture.base,
        &[
            "commit-tree",
            &empty,
            "-p",
            &fixture.seed,
            "-m",
            "an empty head",
        ],
    );
    let (applies, _, _) = task_slot_at(&fixture, "applies", &head);
    assert_eq!(
        fixture
            .manager
            .repair_materialize(&mut NoHooks, &applies, &fixture.side)
            .expect("a change that applies"),
        Materialized::Clean
    );
}

/// A repair slot left conflicted on `a.txt` by its materialization, with its
/// checkout and registration entry.
fn a_conflicted_repair_slot(fixture: &Fixture) -> (Slot, PathBuf, PathBuf) {
    let ours = commit_changing_a(fixture, &fixture.seed, "ours\n", "ours");
    let theirs = commit_changing_a(fixture, &fixture.seed, "theirs\n", "theirs");
    let (slot, checkout, entry) = task_slot_at(fixture, "alpha", &ours);
    assert_eq!(
        fixture
            .manager
            .repair_materialize(&mut NoHooks, &slot, &theirs)
            .expect("the conflict a repair materializes through"),
        Materialized::Conflict
    );
    (slot, checkout, entry)
}

/// T-C1-ANS-CAPTURE, the unresolved read: a repair whose worker leaves the
/// conflict unresolved, the slot's `index` removed before the capture. Git
/// reads an empty index and answers no unmerged entry: at C
/// `unresolved_conflicts` answers `[]`, hiding the conflict; under F it is a
/// registry refusal.
#[test]
fn an_unresolved_conflict_read_through_a_deleted_index_is_refused_not_hidden() {
    let fixture = Fixture::created("f-capture-unresolved");
    let (slot, _, entry) = a_conflicted_repair_slot(&fixture);
    assert_eq!(
        fixture
            .manager
            .unresolved_conflicts(&slot)
            .expect("the read, whole"),
        vec!["a.txt".to_owned()],
        "the control: the conflict is read while the registration is whole"
    );
    fs::remove_file(entry.join("index")).expect("a prune deletes the index");
    let outcome = refused_naming(
        fixture.manager.unresolved_conflicts(&slot),
        &["unresolved conflicts", "index"],
    );
    report_cases(&[("index deleted before the capture".to_owned(), outcome)]);
}

/// T-C1-ANS-CAPTURE, the resolved read: a retained retry whose earlier capture
/// resolved `a.txt`, its `index` removed. The resolve-undo record is in the
/// index, so Git reads none: at C `resolved_conflicts` answers `[]` and a
/// revised declaration would go unread; under F it is a registry refusal.
#[test]
fn a_resolved_conflict_read_through_a_deleted_index_is_refused_not_forgotten() {
    let fixture = Fixture::created("f-capture-resolved");
    let (slot, checkout, entry) = a_conflicted_repair_slot(&fixture);
    fs::write(checkout.join("a.txt"), "resolved\n").expect("the worker's resolution");
    git(&checkout, &["add", "--", "a.txt"]);
    assert_eq!(
        fixture
            .manager
            .resolved_conflicts(&slot)
            .expect("the read, whole"),
        vec!["a.txt".to_owned()],
        "the control: the earlier resolution is read while the registration is whole"
    );
    fs::remove_file(entry.join("index")).expect("a prune deletes the index");
    let outcome = refused_naming(
        fixture.manager.resolved_conflicts(&slot),
        &["resolved conflicts", "index"],
    );
    report_cases(&[(
        "index deleted before the retry's capture".to_owned(),
        outcome,
    )]);
}

/// A task slot at a commit on `head` that tracks the resolution manifest's
/// name, with its checkout and registration entry.
fn a_slot_whose_repository_tracks_the_manifest(fixture: &Fixture) -> (Slot, PathBuf, PathBuf) {
    let base = &fixture.base;
    fs::write(base.join(RESOLUTION_MANIFEST), "resolved a.txt\n").expect("the repository's file");
    git(base, &["add", "--", RESOLUTION_MANIFEST]);
    git(
        base,
        &["commit", "-q", "-m", "the repository tracks the name"],
    );
    let tracking = git(base, &["rev-parse", "HEAD"]);
    task_slot_at(fixture, "alpha", &tracking)
}

/// T-C1-ANS-CAPTURE, the manifest's name: a repository that tracks
/// `.upstroke-resolved`, the slot's `index` removed before
/// `resolution_manifest`. Git reads an empty index, and the repository's file
/// reads as the worker's untracked manifest: at C its bytes are read as
/// declarations; under F the read is a registry refusal.
#[test]
fn a_tracked_manifest_name_read_through_a_deleted_index_is_refused_not_read_as_declarations() {
    let fixture = Fixture::created("f-capture-manifest");
    let (slot, _, entry) = a_slot_whose_repository_tracks_the_manifest(&fixture);
    match fixture.manager.resolution_manifest(&slot) {
        Err(UpstrokeError::Refused { message }) => {
            assert!(message.contains("tracks"), "{message}");
        }
        other => panic!("the control: a tracked name is refused, not read: {other:?}"),
    }
    fs::remove_file(entry.join("index")).expect("a prune deletes the index");
    let outcome = refused_naming(
        fixture.manager.resolution_manifest(&slot),
        &["resolution manifest's name", "index"],
    );
    report_cases(&[("index deleted before the manifest read".to_owned(), outcome)]);
}

/// T-C1-ANS-CAPTURE, the stage: the same repository, the slot's `index`
/// removed before `candidate_stage`. At C the stage reads the repository's
/// file as the worker's manifest, excludes it from `add -A` and removes it, so
/// the candidate deletes it; under F the stage refuses before it writes, and
/// the file is kept.
#[test]
fn a_stage_over_a_tracked_manifest_name_and_a_deleted_index_is_refused_and_keeps_the_file() {
    let fixture = Fixture::created("f-capture-stage");
    let (slot, checkout, entry) = a_slot_whose_repository_tracks_the_manifest(&fixture);
    fs::remove_file(entry.join("index")).expect("a prune deletes the index");
    let outcome = refused_naming(
        fixture.manager.candidate_stage(&mut NoHooks, &slot, &[]),
        &["resolution manifest's name", "index"],
    );
    let kept = checkout.join(RESOLUTION_MANIFEST).is_file();
    report_cases(&[
        ("index deleted before the stage".to_owned(), outcome),
        (
            "the repository's file is kept".to_owned(),
            if kept {
                Ok(())
            } else {
                Err("the stage removed the repository's file".to_owned())
            },
        ),
    ]);
}

/// T-C1-WT: the capture's `add -A` done, then the slot's `index` removed at the
/// `Before` phase of `Object.CandidateWriteTree`. `write-tree` writes the empty
/// tree and the index again (E5), so at C the capture's tree is the empty tree;
/// under F the check before the write refuses.
#[test]
fn a_write_tree_over_a_deleted_index_is_refused_before_it_writes_the_empty_tree() {
    let fixture = Fixture::created("f-write-tree");
    let slot = fixture.add_task(&mut NoHooks, "alpha", 1);
    let checkout = fixture.manager.slot_path(&slot);
    let entry = super::fixture::registration_of(&fixture.manager, &checkout);
    fs::write(checkout.join("d.txt"), "the worker's file\n").expect("the worker's file");
    fixture
        .manager
        .candidate_stage(&mut NoHooks, &slot, &[])
        .expect("the capture's add -A");
    let index = entry.join("index");
    let mut hooks = AtPhase::new(
        EffectSiteId::Object(ObjectSite::CandidateWriteTree),
        HookPhase::Before,
        || fs::remove_file(&index).expect("a prune deletes the index"),
    );
    let outcome = refused_naming(
        fixture.manager.candidate_write_tree(&mut hooks, &slot),
        &["write-tree", "index"],
    );
    report_cases(&[("index deleted before the write".to_owned(), outcome)]);
}

/// T-C1-WT's control: with the registration whole the capture's tree is the
/// worktree's true tree, holding the worker's file, at C and under F.
#[test]
fn a_write_tree_in_a_whole_registration_writes_the_true_tree() {
    let fixture = Fixture::created("f-write-tree-whole");
    let slot = fixture.add_task(&mut NoHooks, "alpha", 1);
    let checkout = fixture.manager.slot_path(&slot);
    fs::write(checkout.join("d.txt"), "the worker's file\n").expect("the worker's file");
    fixture
        .manager
        .candidate_stage(&mut NoHooks, &slot, &[])
        .expect("the capture's add -A");
    let tree = fixture
        .manager
        .candidate_write_tree(&mut NoHooks, &slot)
        .expect("the tree");
    let listed = git(&fixture.base, &["ls-tree", "--name-only", &tree]);
    assert!(listed.lines().any(|name| name == "d.txt"), "{listed}");
}

/// T-C1-ANS-PROMO, at the manager: the promotion's path read of a slot whose
/// `index` is deleted. Git reads an empty index, and the base's paths read as
/// deletions while the worker's addition is lost: at C `changed_paths` answers
/// that set; under F it is a registry refusal. The run-level form is in
/// `coordinator::tests`.
#[test]
fn a_promotions_path_read_through_a_deleted_index_is_refused_not_recorded_as_deletions() {
    let fixture = Fixture::created("f-promotion-paths");
    let slot = fixture.add_task(&mut NoHooks, "alpha", 1);
    let checkout = fixture.manager.slot_path(&slot);
    let entry = super::fixture::registration_of(&fixture.manager, &checkout);
    fs::write(checkout.join("d.txt"), "the worker's file\n").expect("the worker's file");
    fixture
        .manager
        .candidate_stage(&mut NoHooks, &slot, &[])
        .expect("the capture's add -A");
    let whole = fixture
        .manager
        .changed_paths(&slot, &fixture.head)
        .expect("the read, whole");
    assert!(format!("{whole:?}").contains("d.txt"), "{whole:?}");
    fs::remove_file(entry.join("index")).expect("a prune deletes the index");
    let outcome = refused_naming(
        fixture.manager.changed_paths(&slot, &fixture.head),
        &["changed paths", "index"],
    );
    report_cases(&[(
        "index deleted before the promotion's read".to_owned(),
        outcome,
    )]);
}

/// T-C3-CONTROLS: with the store absent, closure 3's arm binds nothing only for
/// a checkout whose `.git` names an entry of this repository's own store. A
/// target whose `.git` names another repository's store, and a populated
/// target with no `.git`, keep the scan's I/O failure, which the tolerant
/// access refuses at its deadline; an empty directory and an absent target
/// converge as they did. The same at C and under F.
#[test]
fn a_removal_with_no_store_refuses_every_target_that_names_no_entry_of_its_own_store() {
    let fixture = Fixture::created("f-closure3-controls");
    let store = fixture.manager.common_git_dir().join("worktrees");
    let _ = fs::remove_dir(&store);
    let other = fixture.root.join("other-store");
    let shapes: [(&str, Option<String>, bool); 4] = [
        (
            "another repository's store",
            Some(format!(
                "gitdir: {}\n",
                as_git_writes_it(&other.join(".git").join("worktrees").join("kalpha-g1"))
            )),
            false,
        ),
        ("no .git", None, false),
        ("an empty directory", None, true),
        ("absent", None, true),
    ];
    let mut cases = Vec::new();
    for (generation, (label, pointer, converges)) in (1_u32..).zip(shapes) {
        let slot = fixture.task("control", generation);
        fixture
            .manager
            .write_intent(&mut NoHooks, &slot)
            .expect("the intent");
        let target = fixture.manager.slot_path(&slot);
        match (label, &pointer) {
            ("absent", _) => {}
            ("an empty directory", _) => fs::create_dir_all(&target).expect("an empty directory"),
            (_, Some(pointer)) => {
                fs::create_dir_all(&target).expect("a populated target");
                fs::write(target.join(".git"), pointer).expect("its .git");
            }
            (_, None) => {
                fs::create_dir_all(&target).expect("a populated target");
                fs::write(target.join("file.txt"), "x\n").expect("a file, no .git");
            }
        }
        let removed = fixture.manager.remove_worktree(&mut NoHooks, &slot);
        let outcome = match (removed, converges) {
            (Ok(()), true) if !target.exists() => Ok(()),
            (Err(UpstrokeError::RegistryRefused { message }), false)
                if target.exists() && message.contains("failed to read") =>
            {
                Ok(())
            }
            (other, _) => Err(format!(
                "{other:?}, the target present: {}",
                target.exists()
            )),
        };
        cases.push((label.to_owned(), outcome));
    }
    report_cases(&cases);
}

/// T-C3-SEQ, the witness `PR5-RD-003-A-PRUNE-STRANDS-A-CHECKOUT-WHOSE-GITDIR-IS-GONE`'s
/// file asks for: its own sequence, run to the state its step 3 refused in. A
/// healthy `alpha`, and a `charlie` whose registration has lost its `gitdir`.
/// Since #329 no engine removal prunes, so a prune that no engine process starts
/// deletes `charlie`'s entry, and `alpha`'s removal, the store's last
/// registration, takes the emptied store, where #308's prune took both. With
/// `charlie`'s `.git` naming its deleted entry, both of `charlie`'s removals
/// converge, through closure 3's arm and then over the absent target. At C both
/// refuse with the checkout kept, as the file's executed sequence recorded. The
/// control runs the same sequence with that `.git` naming another repository's
/// store (case 3): both removals refuse and the checkout's bytes stay as they
/// were, the same at C and under F.
#[test]
fn a_checkout_whose_gitdir_is_gone_converges_twice_once_a_prune_has_emptied_the_store() {
    let shapes = [
        ("its own store", "f-closure3-sequence", true),
        (
            "another repository's store",
            "f-closure3-sequence-control",
            false,
        ),
    ];
    let mut cases = Vec::new();
    for (label, tag, own_store) in shapes {
        let fixture = Fixture::created(tag);
        let alpha = fixture.add_task(&mut NoHooks, "alpha", 1);
        let charlie = fixture.add_task(&mut NoHooks, "charlie", 1);
        let alpha_path = fixture.manager.slot_path(&alpha);
        let charlie_path = fixture.manager.slot_path(&charlie);
        let charlie_admin = fixture
            .manager
            .revalidate_removal(&charlie_path)
            .expect("admin dir")
            .expect("registered");
        fs::remove_file(charlie_admin.join("gitdir"))
            .expect("charlie's registration loses its gitdir");
        let store = fixture.manager.common_git_dir().join("worktrees");
        git(&fixture.base, &["worktree", "prune", "--expire=now"]);
        assert!(
            !charlie_admin.exists() && store.is_dir(),
            "{label}: a prune that no engine process starts deletes charlie's entry, not alpha's"
        );
        fixture
            .manager
            .remove_worktree(&mut NoHooks, &alpha)
            .expect("alpha's removal");
        assert!(
            !alpha_path.exists() && !store.exists(),
            "{label}: alpha's removal, the store's last registration, takes the emptied store"
        );
        if !own_store {
            let name = charlie_admin.file_name().expect("an entry name");
            let other = fixture.root.join("other-store");
            let entry = other.join(".git").join("worktrees").join(name);
            fs::write(
                charlie_path.join(".git"),
                format!("gitdir: {}\n", as_git_writes_it(&entry)),
            )
            .expect("charlie's .git names another repository's store");
        }
        let before = tree_bytes(&charlie_path);
        for attempt in ["charlie's forced removal", "and its retry"] {
            let removed = fixture.manager.remove_worktree(&mut NoHooks, &charlie);
            let after = tree_bytes(&charlie_path);
            let outcome = match (removed, own_store) {
                (Ok(()), true) if after.is_empty() => Ok(()),
                (Err(UpstrokeError::RegistryRefused { message }), false)
                    if message.contains("failed to read") && after == before =>
                {
                    Ok(())
                }
                (other, _) => Err(format!(
                    "{other:?}, the checkout as it was: {}",
                    after == before
                )),
            };
            cases.push((format!("{label}: {attempt}"), outcome));
        }
        assert!(!store.exists(), "{label}: no store is made");
    }
    report_cases(&cases);
}

/// T-ENV-MGR, the integration correction's manager face (F): a `worktrees`
/// store linked into another repository, under an inherited
/// `GIT_COMMON_DIR` naming the run's common directory (R-C-REGISTRY's part 2),
/// keeps the manager's guarded reads and its kept-instance guard whole. C4's
/// layout inherits the absolute value. C5's inherits a relative one, derived
/// here from the actual paths, with the base as deep below the root as the
/// manager's work trees (PROP:2129), and this test asserts that the value names
/// the run's common directory from the base and from the slot the child adds.
/// The same C4 layout with nothing inherited is refused by the guarded read,
/// the original rule. With nothing inherited `derive` refuses a link already
/// in place (R-C-REGISTRY part 1's start refusal, which
/// [`derive_refuses_a_linked_worktrees_registry_while_no_common_dir_is_inherited`]
/// witnesses), so in that sequence the child plants the link itself, once its
/// own `derive` has passed: the link a guarded read meets after the start.
/// This process inherits nothing either, and derives the slot path before it
/// plants any link. Each sequence runs in an isolated child whose
/// environment alone carries the value; the guarded operations are
/// `candidate_stage` (the manifest's read), `candidate_write_tree` (its
/// pre-check) and `kept_instances`.
#[cfg(unix)]
#[test]
fn a_linked_registry_keeps_the_managers_guarded_reads_under_the_inherited_common_dir() {
    let mut cases: Vec<(String, Result<(), String>)> = Vec::new();
    for sequence in ["C4", "C5", "C4 with nothing inherited"] {
        let tree = scratch("f-linked-manager");
        let root = fs::canonicalize(tree.path()).expect("the canonical scratch root");
        assert!(
            root.parent().is_some(),
            "the scratch root is not the filesystem root"
        );
        let below = Path::new(if sequence == "C5" {
            super::fixture::EQUAL_DEPTH_BASE
        } else {
            "repo"
        });
        let base = root.join(below);
        create_dir(&base);
        create_dir(&root.join("private"));
        git(&base, &["init", "-q", "-b", "main"]);
        git(&base, &["config", "user.email", "tests@upstroke.local"]);
        git(&base, &["config", "user.name", "upstroke tests"]);
        write_file(&base.join("a.txt"), b"one\n");
        git(&base, &["add", "-A"]);
        git(&base, &["commit", "-q", "-m", "seed"]);
        let other = root.join("other");
        git(
            &root,
            &[
                "clone",
                "-q",
                &base.to_string_lossy(),
                &other.to_string_lossy(),
            ],
        );
        let elsewhere = other.join(".git").join("worktrees");
        create_dir(&elsewhere);
        let common = fs::canonicalize(base.join(".git")).expect("the run's common directory");
        let depth = below.components().count();
        let named = common
            .strip_prefix(&root)
            .expect("the common directory is below the root");
        let slot_path = WorkspaceManager::derive(
            &base,
            &root.join("private"),
            super::fixture::RUN_ID,
            "inc-linked",
        )
        .expect("derive the manager the child derives")
        .slot_path(&Slot::Task {
            key: "linked".to_owned(),
            generation: 1,
        });
        if sequence != "C4 with nothing inherited" {
            super::fixture::link_directory(&elsewhere, &base.join(".git").join("worktrees"));
        }
        let mut relative = PathBuf::new();
        for _ in 0..depth {
            relative.push("..");
        }
        relative.push(named);
        if sequence == "C5" {
            for from in [&base, &slot_path] {
                assert_eq!(
                    from.ancestors().nth(depth).map(|top| top.join(named)),
                    Some(common.clone()),
                    "`{}` from {} names the run's common directory, climbing to the scratch \
                     root and no further",
                    relative.display(),
                    from.display()
                );
            }
        }
        let value = if sequence == "C5" {
            relative.into_os_string()
        } else {
            common.clone().into_os_string()
        };
        let mut vars: Vec<(&str, &OsStr)> = vec![
            (F_LINKED_ROOT, root.as_os_str()),
            (F_LINKED_BASE, below.as_os_str()),
            (F_LINKED_SEQUENCE, OsStr::new(sequence)),
        ];
        if sequence != "C4 with nothing inherited" {
            vars.push(("GIT_COMMON_DIR", value.as_os_str()));
        }
        let ended = crate::agent::proc::test_support::run_test_isolated(
            "workspace_manager::tests::linked_registry_guarded_reads_child",
            &vars,
            std::time::Duration::from_secs(180),
        );
        let outcome = if ended.status.is_some_and(|status| status.success())
            && ended.stdout.contains("1 passed")
        {
            Ok(())
        } else {
            Err(ended.to_string())
        };
        cases.push((sequence.to_owned(), outcome));
        drop(tree);
    }
    report_cases(&cases);
}

// ---------------------------------------------------------------------------
// A registration store pinned at derivation (F, the integration correction)
// ---------------------------------------------------------------------------
//
// R5's start check in `derive` refuses a manager over a store linked out of the
// common directory while no `GIT_COMMON_DIR` is inherited (R-C-REGISTRY part 1),
// and fails closed over a store link it cannot resolve. Under an inherited value,
// part 2's configuration, it admits both without reading the store. Every row
// here that derives over such a link therefore derives in a pin child: a
// `LinkedChild` of this binary, on every platform, that inherits the run's
// canonical common directory as an absolute `GIT_COMMON_DIR` and makes the row's
// removals and every one of its assertions. This process makes the fixture, its
// registrations and any other repository first, inheriting nothing, plants the
// link, and owns the fixture until the child is gone. F's runs of these rows in
// one process, with nothing inherited, are its evidence at F and ENV.

/// The fixture's registration store, `<common git dir>/worktrees`, moved with
/// every registration in it to `target`, and a directory link to `target`
/// planted where it was, a symlink or on Windows a junction, so the store is a
/// link when a pin child derives its manager.
///
/// The fixture's own manager, derived over the ordinary store in this process,
/// which inherits no `GIT_COMMON_DIR`, makes the registrations before this, so
/// each is Git's own. An add through the link with no `GIT_COMMON_DIR` given
/// makes none: Git's checkout of the new work tree fails, `not a git
/// repository`, or `nonexistent object` where the link names another
/// repository's store, before any removal. After this the fixture's manager
/// names paths and does nothing else.
fn link_store_to(fixture: &Fixture, target: &Path) {
    let store = fixture.manager.common_git_dir().join("worktrees");
    fs::rename(&store, target).expect("the store moved, with its registrations, to the target");
    plant_directory_link(target, &store);
}

/// Run pin child `test` for `case` over `fixture`, whose store
/// [`link_store_to`] has linked, with `handed` beside the handoff, and say
/// whether it passed: `Ok` only when libtest reports the child's one test
/// passed, within [`super::fixture::LINK_BOUND`], and the child then exits with
/// success, so every assertion it makes held. Otherwise the error says what
/// the child reported, how it ended and what its stderr holds.
///
/// The child is a [`super::fixture::LinkedChild`] of this binary, which runs
/// on every platform. Its environment is this process's, with `GIT_COMMON_DIR`
/// set to the run's canonical common directory, an absolute path, and each
/// handed variable set to the `OsStr` itself. The child adopts no fixture:
/// this process owns `fixture`, and its tree is reclaimed here once the child
/// is gone.
fn run_pin_child(
    test: &str,
    fixture: &Fixture,
    case: &str,
    handed: &[(&str, &OsStr)],
) -> Result<(), String> {
    let common = fixture.manager.common_git_dir();
    let mut env = vec![
        (COMMON_DIR, common.as_os_str()),
        (PIN_ROOT, fixture.root.as_os_str()),
        (PIN_BASE, fixture.base.as_os_str()),
        (PIN_PRIVATE, fixture.private.as_os_str()),
        (PIN_CASE, OsStr::new(case)),
        (PIN_COMMON, common.as_os_str()),
    ];
    env.extend_from_slice(handed);
    let test = format!("workspace_manager::tests::{test}");
    let stderr = fixture.root.join("pin-child.stderr");
    let (child, lines) = super::fixture::LinkedChild::spawn(&test, &env, &stderr);
    let deadline = std::time::Instant::now() + super::fixture::LINK_BOUND;
    let mut said = Vec::new();
    let result = loop {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        match lines.recv_timeout(left) {
            Ok(line) if line.contains("test result:") => break Some(line),
            Ok(line) => said.push(line),
            Err(_) => break None,
        }
    };
    let ended = result
        .as_ref()
        .and_then(|_| child.wait_within(super::fixture::LINK_BOUND));
    match (&result, ended) {
        (Some(line), Some(status))
            if line.contains("test result: ok. 1 passed; 0 failed") && status.success() =>
        {
            Ok(())
        }
        _ => Err(format!(
            "{test} did not pass: its result {result:?}, its exit {ended:?}, its other lines \
             {said:?}; its stderr:\n{}",
            fs::read_to_string(&stderr).unwrap_or_default()
        )),
    }
}

/// T-PIN-1, the pinned store's removals: a store that was a link when the
/// manager was derived has the registrations its removals bound deleted below
/// the pinned target, a snapshot's and a task worktree's, and keeps the link,
/// the target, and every entry no removal bound: another repository's
/// registration beside them, and then the emptied target itself. The target is
/// another repository's store, the adopted layout, and then a directory of no
/// repository, since the pin is unconditional. The registrations are Git's
/// own, made in this process over the ordinary store and moved with it
/// ([`link_store_to`]), and the other repository is made here too, all before
/// anything is inherited.
///
/// **Admission.** With nothing inherited, R5's start check refuses to derive a
/// manager over either layout (R-C-REGISTRY part 1). Each layout's manager is
/// therefore derived, and its removals made, in a [`pinned_store_removals_child`]
/// that inherits the run's canonical common directory as an absolute
/// `GIT_COMMON_DIR`, which the check admits (part 2). That the removals read no
/// `GIT_COMMON_DIR` is a claim of their source, which these runs do not
/// witness. The snapshot this process's `add_snapshot` returned is handed to
/// the child by its identity, and the child removes it through
/// `remove_snapshot`, on the handle it rebuilds from that identity.
#[test]
fn a_store_linked_before_derive_removes_only_its_bound_registrations_and_keeps_link_and_target() {
    let mut cases: Vec<(String, Result<(), String>)> = Vec::new();
    for (tag, repository) in PINNED_REMOVAL_LAYOUTS {
        let fixture = Fixture::created(tag);
        fixture.add_task(&mut NoHooks, "alpha", 1);
        fixture.add_task(&mut NoHooks, "beta", 1);
        let gate = fixture
            .manager
            .add_snapshot(
                &mut NoHooks,
                &SnapshotName::gates(0, 1, 1),
                &SnapshotInput::Commit(oid(&fixture.head)),
            )
            .unwrap_or_else(|error| panic!("{tag}: the snapshot's add: {error}"));
        let Slot::Snapshot { name } = gate.slot() else {
            panic!("{tag}: a snapshot's slot is a snapshot slot: {gate:?}");
        };
        assert!(
            gate.ephemeral().is_none(),
            "{tag}: the snapshot of a commit names that existing commit: {gate:?}"
        );
        if repository {
            let other = fixture.root.join("other");
            create_dir(&other);
            git(&other, &["init", "-q", "-b", "main"]);
        }
        link_store_to(&fixture, &pinned_removal_target(&fixture.root, repository));
        let rendered = format!("{gate:?}");
        let outcome = run_pin_child(
            "pinned_store_removals_child",
            &fixture,
            tag,
            &[
                (PIN_SNAPSHOT_NAME, OsStr::new(name.as_str())),
                (PIN_SNAPSHOT_PATH, gate.path().as_os_str()),
                (PIN_SNAPSHOT_HEAD, OsStr::new(gate.head().as_str())),
                (PIN_SNAPSHOT_DEBUG, OsStr::new(&rendered)),
            ],
        );
        cases.push((tag.to_owned(), outcome));
    }
    report_cases(&cases);
}

/// T-PIN-2 (F-R12-PIN-LOSS-FALLBACK): a removal that bound its registration
/// while the store was the pinned link refuses, before its checkout is
/// touched, when the store is no longer that link resolving to the pinned
/// target: re-pointed to a directory holding a copy of the bound entry,
/// replaced by a plain directory holding one, or removed. Each is made at the
/// removal's `Before` hook, after its scan bound the registration, and the
/// first two before its scan as well. It never falls back to the walk from the
/// common git dir: the checkout, the pinned entry and every copy are
/// untouched, and the refusal names what the store is now. A removed link is
/// not made before the scan, which then finds no store and keeps its own
/// refusal, unchanged.
///
/// **Admission.** The registration is made in this process, which inherits
/// nothing. Each case's manager is derived, its loss made and its removal run
/// in a [`pinned_store_lost_child`] that inherits the run's canonical common
/// directory as an absolute `GIT_COMMON_DIR`: with nothing inherited R5's
/// start check refuses the derivation over this store, linked out of the
/// common directory.
#[test]
fn a_pinned_store_lost_before_its_removal_refuses_before_the_checkout_is_touched() {
    let mut cases: Vec<(String, Result<(), String>)> = Vec::new();
    for (index, (case, _)) in PIN_LOSS_CASES.into_iter().enumerate() {
        let fixture = Fixture::created(&format!("pin-lost-{index}"));
        fixture.add_task(&mut NoHooks, "alpha", 1);
        link_store_to(&fixture, &fixture.root.join("pinned"));
        let outcome = run_pin_child("pinned_store_lost_child", &fixture, case, &[]);
        cases.push((case.to_owned(), outcome));
    }
    report_cases(&cases);
}

/// T-PIN-3: below the pinned target the walk is the original one. The bound
/// entry, its `gitdir` or its `locked` exchanged at the `Before` hook for a
/// link to a victim refuses the removal, naming that component, before the
/// checkout is touched, with the victim untouched. The two file links need a
/// symlink, which the Windows guest's test user cannot make, so they run on
/// Unix.
///
/// **Admission.** The registration is made in this process, which inherits
/// nothing. Each case's manager is derived, and its removal run, in a
/// [`link_below_pinned_target_child`] that inherits the run's canonical common
/// directory as an absolute `GIT_COMMON_DIR`: with nothing inherited R5's
/// start check refuses the derivation over this store, linked out of the
/// common directory.
#[test]
fn a_link_planted_below_a_pinned_target_at_the_before_hook_refuses_the_removal() {
    let mut cases: Vec<(String, Result<(), String>)> = Vec::new();
    for (index, (case, name)) in LINK_BELOW_TARGET_CASES.into_iter().enumerate() {
        let file_link = !name.is_empty();
        if file_link && cfg!(windows) {
            continue;
        }
        let fixture = Fixture::created(&format!("pin-inside-{index}"));
        fixture.add_task(&mut NoHooks, "alpha", 1);
        link_store_to(&fixture, &fixture.root.join("pinned"));
        let outcome = run_pin_child("link_below_pinned_target_child", &fixture, case, &[]);
        cases.push((case.to_owned(), outcome));
    }
    report_cases(&cases);
}

/// T-PIN-4: the pinned target itself exchanged at the `Before` hook, renamed
/// away and a link to a victim holding a copy of the bound entry planted at
/// its path, leaves a store that no longer resolves to the pinned target. The
/// removal refuses before its checkout is touched, and the victim and the
/// original target, moved aside, are untouched.
///
/// **Admission.** The registration is made in this process, which inherits
/// nothing. The manager is derived, and the removal run, in a
/// [`pinned_target_exchanged_child`] that inherits the run's canonical common
/// directory as an absolute `GIT_COMMON_DIR`: with nothing inherited R5's
/// start check refuses the derivation over this store, linked out of the
/// common directory.
#[test]
fn a_pinned_target_exchanged_for_a_link_at_the_before_hook_refuses_the_removal() {
    let fixture = Fixture::created("pin-target-exchanged");
    fixture.add_task(&mut NoHooks, "alpha", 1);
    link_store_to(&fixture, &fixture.root.join("pinned"));
    let outcome = run_pin_child(
        "pinned_target_exchanged_child",
        &fixture,
        "pin-target-exchanged",
        &[],
    );
    report_cases(&[("pin-target-exchanged".to_owned(), outcome)]);
}

/// T-PIN-5: a store that was not a link when the manager was derived is not
/// pinned. A link planted there afterwards, the real store moved aside and a
/// link to a victim holding a copy of the bound entry put in its place,
/// refuses the removal at the link, as any link on the registration's chain
/// does, before the checkout is touched. That holds both at the removal's
/// `Before` hook and before its scan, and the victim and the moved store are
/// untouched.
#[test]
fn a_store_linked_after_derive_refuses_the_removal_at_the_link() {
    for (index, at_hook) in [true, false].into_iter().enumerate() {
        let fixture = Fixture::created(&format!("store-linked-after-{index}"));
        let slot = fixture.add_task(&mut NoHooks, "alpha", 1);
        let checkout = fixture.manager.slot_path(&slot);
        let store = fixture.manager.common_git_dir().join("worktrees");
        let name = checkout.file_name().expect("a checkout name").to_owned();
        let victim = fixture.root.join("victim");
        copy_entry(&store.join(&name), &victim.join(&name));
        let victim_before = snapshot(&victim.join(&name));
        let real_before = snapshot(&store.join(&name));
        let checkout_before = snapshot(&checkout);
        let moved = store.with_extension("moved-away");
        let result = if at_hook {
            let mut hooks = SubstituteAtBefore {
                site: slot.remove_site(),
                target: store.clone(),
                moved: moved.clone(),
                victim: victim.clone(),
                file_link: false,
            };
            fixture.manager.remove_worktree(&mut hooks, &slot)
        } else {
            fs::rename(&store, &moved).expect("move the real store aside");
            plant_directory_link(&victim, &store);
            fixture.manager.remove_worktree(&mut NoHooks, &slot)
        };
        let error = result.expect_err("a link made after derivation refuses");
        let message = refusal_of(&error);
        assert!(
            message.contains("symlink or reparse point")
                && message.contains(&store.display().to_string()),
            "at the hook {at_hook}: the refusal names the link: {message}"
        );
        assert_eq!(
            snapshot(&victim.join(&name)),
            victim_before,
            "at the hook {at_hook}: the victim is untouched"
        );
        assert_eq!(
            snapshot(&moved.join(&name)),
            real_before,
            "at the hook {at_hook}: the real store, moved aside, is untouched"
        );
        assert_eq!(
            snapshot(&checkout),
            checkout_before,
            "at the hook {at_hook}: the checkout is untouched"
        );
    }
}

/// T-PIN-6 (F-R12-REBOUND-PHASE): under a pinned store, a registration whose
/// identity is rewritten at the `Before` hook is caught where it always was,
/// at the destructive boundary. That comes after the checkout's deletion,
/// which the removal's order makes first, and it keeps `locked` and the
/// entry.
///
/// **Admission.** The registration is made in this process, which inherits
/// nothing. The manager is derived, and the removal run, in a
/// [`registration_rebound_under_pin_child`] that inherits the run's canonical
/// common directory as an absolute `GIT_COMMON_DIR`: with nothing inherited
/// R5's start check refuses the derivation over this store, linked out of the
/// common directory.
#[test]
fn a_registration_rebound_under_a_pinned_store_keeps_its_admin_state() {
    let fixture = Fixture::created("pin-rebound");
    fixture.add_task(&mut NoHooks, "alpha", 1);
    link_store_to(&fixture, &fixture.root.join("pinned"));
    let outcome = run_pin_child(
        "registration_rebound_under_pin_child",
        &fixture,
        "pin-rebound",
        &[],
    );
    report_cases(&[("pin-rebound".to_owned(), outcome)]);
}

/// T-PIN-7: a store link whose target is absent when the manager is derived is
/// not pinned, even once its target appears. The store's registration, Git's
/// own, moved with it to the link's target, is refused at the link, before the
/// checkout is touched, as any link on the registration's chain is.
///
/// **Admission.** The registration is made, the store moved and linked, and the
/// link's target taken away in this process, which inherits nothing. The
/// manager is derived, the target put back and the removal run in a
/// [`store_link_target_appears_child`] that inherits the run's canonical common
/// directory as an absolute `GIT_COMMON_DIR`: with nothing inherited R5's start
/// check fails the derivation closed on a store link it cannot resolve, and
/// under the inherited value it admits it without resolving the link.
#[test]
fn a_store_link_whose_target_appears_after_derive_is_not_pinned() {
    let fixture = Fixture::created("store-link-unpinned");
    fixture.add_task(&mut NoHooks, "alpha", 1);
    let later = fixture.root.join("later");
    let away = fixture.root.join("later-away");
    let store = fixture.manager.common_git_dir().join("worktrees");
    fs::rename(&store, &later)
        .expect("the store moved, with its registration, to the link's target");
    plant_directory_link(&later, &store);
    fs::rename(&later, &away).expect("its target gone at derivation");
    let outcome = run_pin_child(
        "store_link_target_appears_child",
        &fixture,
        "store-link-unpinned",
        &[],
    );
    report_cases(&[("store-link-unpinned".to_owned(), outcome)]);
}

/// A committed repository at `path` holding `files`, made in this process's
/// own environment, which sets none of the names under test. Automatic
/// maintenance is off, as it is for every Git child the engine runs: a commit
/// detaches it since Git 2.47, so it can still be running in the `.git` a test
/// copies next.
#[cfg(unix)]
fn context_repository(path: &Path, files: &[(&str, &str)]) {
    fs::create_dir_all(path).expect("a repository's directory");
    git(path, &["init", "-q", "-b", "main"]);
    git(path, &["config", "user.email", "tests@upstroke.local"]);
    git(path, &["config", "user.name", "upstroke tests"]);
    git(path, &["config", "maintenance.auto", "false"]);
    for (name, content) in files {
        write_file(&path.join(name), content.as_bytes());
    }
    git(path, &["add", "-A"]);
    git(path, &["commit", "-q", "-m", "initial"]);
}

/// The seven sequences of
/// [`the_managers_slot_add_and_reads_shed_the_work_tree_context_and_its_base_keeps_it`],
/// each run in an isolated child whose environment alone carries the names
/// it inherits. The parent builds each layout in its own environment, keeps
/// the bytes the child must leave alone, and compares them afterwards.
///
/// FUB-D9-ENV's and CR-1's sequences inherit a `GIT_INDEX_FILE` naming the
/// user's index, which holds a change of the user's own: the slot add, and two
/// incarnations' slots, must leave it as it is. D1's layout inherits `GIT_DIR`
/// and `GIT_WORK_TREE` selecting `B.git` for `A/work` inside `A`: `derive`
/// and `object_exists` read `B.git` at the base, and the slot's work lands in
/// `B.git`. The quiescence read happens after the user stages a change in the
/// index an inherited `GIT_INDEX_FILE` names, and must read the slot's own.
/// The newline layout's worker add lands in the slot's own index. C4's layout
/// inherits an absolute `GIT_COMMON_DIR` naming a repository whose `worktrees`
/// directory is a link into another repository, and C5's the same link under
/// a relative `GIT_COMMON_DIR` that names the run's common directory from the
/// base and from the manager's slot, six levels below the scratch root as a
/// production gate snapshot is: in both, the verification finds no foreign
/// repository, the commit at the base succeeds, and the other repository's
/// objects and refs are not written.
#[cfg(unix)]
#[test]
fn the_managers_slot_add_and_reads_shed_the_work_tree_context_and_its_base_keeps_it() {
    let mut failures: Vec<String> = Vec::new();
    for sequence in [
        "FUB-D9-ENV",
        "CR-1",
        "D1",
        "quiescence",
        "newline",
        "C4",
        "C5",
    ] {
        let tree = scratch("work-tree-context");
        let root = tree.path();
        fs::create_dir(root.join("private")).expect("the private root");
        let mut names: Vec<(&str, OsString)> = Vec::new();
        let mut watched: Vec<PathBuf> = Vec::new();
        let mut common = None;
        match sequence {
            "FUB-D9-ENV" | "CR-1" | "quiescence" => {
                let managed = root.join("managed");
                context_repository(
                    &managed,
                    &[("README.md", "base\n"), ("tracked.txt", "base\n")],
                );
                if sequence == "CR-1" {
                    git(&managed, &["checkout", "-q", "-b", "side"]);
                    write_file(&managed.join("side.txt"), b"side\n");
                    git(&managed, &["add", "side.txt"]);
                    git(&managed, &["commit", "-q", "-m", "side"]);
                    git(&managed, &["checkout", "-q", "main"]);
                }
                if sequence == "FUB-D9-ENV" {
                    write_file(&managed.join("tracked.txt"), b"user staged\n");
                    git(&managed, &["add", "tracked.txt"]);
                }
                names.push((
                    "GIT_INDEX_FILE",
                    managed.join(".git/index").into_os_string(),
                ));
                if sequence != "quiescence" {
                    watched.push(managed.join(".git/index"));
                }
            }
            "D1" => {
                let enclosing = root.join("A");
                context_repository(
                    &enclosing,
                    &[("README.md", "A\n"), ("work/shared.txt", "base\n")],
                );
                let work = enclosing.join("work");
                write_file(&work.join("shared.txt"), b"user-staged\n");
                git(&enclosing, &["add", "work/shared.txt"]);
                write_file(&work.join("shared.txt"), b"base\n");
                let selected = root.join("B.git");
                let selection = [
                    ("GIT_DIR", selected.as_os_str()),
                    ("GIT_WORK_TREE", work.as_os_str()),
                ];
                for args in [
                    ["init", "-q", "-b", "main"].as_slice(),
                    ["add", "shared.txt"].as_slice(),
                    [
                        "-c",
                        "user.name=upstroke tests",
                        "-c",
                        "user.email=tests@upstroke.local",
                        "commit",
                        "-q",
                        "-m",
                        "B: initial",
                    ]
                    .as_slice(),
                ] {
                    let output = Command::new("git")
                        .current_dir(&work)
                        .envs(selection)
                        .args(args)
                        .output()
                        .expect("run git");
                    assert!(output.status.success(), "{args:?}: {output:?}");
                }
                names = selection
                    .iter()
                    .map(|(name, value)| (*name, value.to_os_string()))
                    .collect();
                watched.extend([enclosing.join(".git/index"), enclosing.join(".git/HEAD")]);
                common = Some(fs::canonicalize(&selected).expect("B.git"));
            }
            "newline" => {
                let repository = root.join("repo\nname");
                context_repository(&repository, &[("README.md", "readme\n")]);
                names.push(("GIT_DIR", repository.join(".git").into_os_string()));
            }
            _ => {
                let base = if sequence == "C4" {
                    root.join("base")
                } else {
                    root.join("checkouts/user/x/y/z/base")
                };
                context_repository(&base, &[("README.md", "base\n"), ("shared.txt", "seed\n")]);
                let other = root.join("other");
                context_copy_tree(&base, &other);
                fs::create_dir(other.join(".git/worktrees")).expect("the other registry");
                std::os::unix::fs::symlink(
                    other.join(".git/worktrees"),
                    base.join(".git/worktrees"),
                )
                .expect("the run's registry, a link into the other repository");
                let value = if sequence == "C4" {
                    base.join(".git").into_os_string()
                } else {
                    OsString::from("../../../../../../checkouts/user/x/y/z/base/.git")
                };
                names.push((COMMON_DIR, value));
                watched.extend([other.join(".git/objects"), other.join(".git/refs")]);
                common =
                    Some(fs::canonicalize(base.join(".git")).expect("the run's common directory"));
            }
        }
        let before: Vec<_> = watched
            .iter()
            .map(|path| context_tree_bytes(path))
            .collect();
        let common = common.unwrap_or_default();
        let mut vars: Vec<(&str, &std::ffi::OsStr)> = names
            .iter()
            .map(|(name, value)| (*name, value.as_os_str()))
            .collect();
        vars.push((CONTEXT_SEQUENCE, std::ffi::OsStr::new(sequence)));
        vars.push((CONTEXT_ROOT, root.as_os_str()));
        vars.push((CONTEXT_COMMON, common.as_os_str()));
        let ended = crate::agent::proc::test_support::run_test_isolated(
            "workspace_manager::tests::work_tree_context_sequence_helper",
            &vars,
            std::time::Duration::from_secs(180),
        );
        if !(ended.status.is_some_and(|status| status.success())
            && ended.stdout.contains("1 passed"))
        {
            failures.push(format!("{sequence}: {ended}"));
            continue;
        }
        for (path, earlier) in watched.iter().zip(&before) {
            if context_tree_bytes(path) != *earlier {
                failures.push(format!("{sequence}: {} was written", path.display()));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} of the seven sequences failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// A relative `GIT_COMMON_DIR` inherited right at the base, `../common`, with
/// a copy of the run's common directory beside the manager's slot, the draft's
/// design lens's construction. The slot add, a file staged in the slot, the
/// manager's capture and the commit at the base all address the run's common
/// directory: the verification in the slot finds the base's, the commit holds
/// the captured tree, and the copy beside the slot is not written. The copy is
/// made before the add, because Git's own checkout child of `git worktree add`
/// reads objects through the relative value from the new work tree on Git 2.43
/// and 2.50.
#[cfg(unix)]
#[test]
fn a_relative_common_dir_resolves_to_the_runs_common_dir_in_a_slot() {
    let tree = scratch("relative-common-dir");
    let root = tree.path();
    let managed = root.join("managed");
    context_repository(
        &managed,
        &[("README.md", "readme\n"), ("shared.txt", "base\n")],
    );
    context_copy_tree(&managed.join(".git"), &root.join("common"));
    fs::create_dir(root.join("private")).expect("the private root");
    let common = fs::canonicalize(root.join("common")).expect("the run's common directory");
    let ended = crate::agent::proc::test_support::run_test_isolated(
        "workspace_manager::tests::relative_common_dir_slot_helper",
        &[
            (COMMON_DIR, std::ffi::OsStr::new("../common")),
            (CONTEXT_ROOT, root.as_os_str()),
            (CONTEXT_COMMON, common.as_os_str()),
        ],
        std::time::Duration::from_secs(180),
    );
    assert!(
        ended.status.is_some_and(|status| status.success()) && ended.stdout.contains("1 passed"),
        "{ended}"
    );
}

// -----------------------------------------------------------------------
// R5: the two schema-4 start refusals (R-C-RELSTORE; R-C-REGISTRY part 1)
// -----------------------------------------------------------------------

/// What [`refuse_relative_repository_wide_values`] is given when the
/// coordinator inherits `name` alone of [`REPOSITORY_WIDE_PATHS`], holding
/// `value`.
fn only(name: &str, value: &OsStr) -> [(&'static str, Option<OsString>); 6] {
    REPOSITORY_WIDE_PATHS.map(|each| (each, (each == name).then(|| value.to_os_string())))
}

/// `rest` below a root the platform's rule calls absolute, spelt with no
/// backslash so that C-style quoting carries it unchanged: `/rest` on Unix,
/// `C:/rest` on Windows.
fn rooted(rest: &str) -> String {
    if cfg!(windows) {
        format!("C:/{rest}")
    } else {
        format!("/{rest}")
    }
}

/// R-C-RELSTORE's rule over each scalar name, at the check itself. This is
/// this head's function, so it is no red/green row: the rows that run red at
/// the bound master are the `derive` and pre-lock witnesses. A relative value
/// and an empty one refuse, naming the variable and saying to export it as an
/// absolute path; an absolute one does not; nothing inherited refuses nothing.
#[test]
fn a_relative_or_empty_repository_wide_path_refuses_naming_itself_and_an_absolute_one_does_not() {
    let absolute = std::env::temp_dir().join("r5-absolute");
    assert!(
        absolute.is_absolute(),
        "the fixture's absolute value: {}",
        absolute.display()
    );
    for name in REPOSITORY_WIDE_PATHS {
        for relative in ["../objects", "objects", "./config", ""] {
            let refusal =
                refuse_relative_repository_wide_values(only(name, OsStr::new(relative)), None)
                    .expect_err("a value that is not an absolute path refuses");
            assert_eq!(
                refusal,
                Refusal::RelativeRepositoryWideName {
                    name,
                    why: NotAbsolute::Value(OsString::from(relative)),
                },
                "`{name}={relative}`"
            );
            let message = refusal.to_string();
            assert!(
                message.contains(&format!("the inherited `{name}`"))
                    && message.contains(&format!("export `{name}` as an absolute path")),
                "the refusal names the variable and says to export it as an absolute path: \
                 {message}"
            );
        }
        assert_eq!(
            refuse_relative_repository_wide_values(only(name, absolute.as_os_str()), None),
            Ok(()),
            "`{name}` holding an absolute path is not refused"
        );
    }
    assert_eq!(
        refuse_relative_repository_wide_values(
            REPOSITORY_WIDE_PATHS.map(|name| (name, None)),
            None,
        ),
        Ok(()),
        "nothing inherited, nothing refused"
    );
}

/// The check reads the seven names the owner adopted, and [`COMMON_DIR`] is
/// not among them: option (C) gives a relative `GIT_COMMON_DIR` its meaning at
/// the started checkout, so it is not refused.
#[test]
fn the_relative_name_check_reads_the_seven_adopted_names_and_not_the_common_directory() {
    let mut read = REPOSITORY_WIDE_PATHS.to_vec();
    read.push(ALTERNATE_OBJECT_DIRECTORIES);
    assert_eq!(
        read,
        [
            "GIT_OBJECT_DIRECTORY",
            "GIT_SHALLOW_FILE",
            "GIT_GRAFT_FILE",
            "GIT_CONFIG",
            "GIT_CONFIG_GLOBAL",
            "GIT_CONFIG_SYSTEM",
            "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        ],
        "R-C-RELSTORE's seven names, as adopted"
    );
    assert!(
        !read.contains(&COMMON_DIR),
        "`GIT_COMMON_DIR` is not refused"
    );
}

/// How a case of [`the_alternates_list_is_split_and_unquoted_exactly_as_git_does`]
/// ends.
#[derive(Debug)]
enum Classified {
    /// No entry refuses the list.
    Accepted,
    /// This decoded entry, which is not an absolute path, refuses it.
    Relative(String),
    /// An entry the check cannot classify exactly refuses it.
    Unclassified,
}

/// R-C-RELSTORE's list rule at the check itself (this head's function, as
/// above), against Git's own parser as `alternate_entries` cites it: the
/// platform's separator, comments, empty entries, C-style quoting across a
/// separator, the byte Git steps past after a closing quote, broken quoting
/// read as written, the escapes, and a decoded NUL that cannot be classified
/// exactly.
#[test]
fn the_alternates_list_is_split_and_unquoted_exactly_as_git_does() {
    let sep = char::from(ALTERNATES_SEPARATOR);
    let other = if sep == ':' { ';' } else { ':' };
    let (a, b) = (rooted("a"), rooted("b"));
    let spanning = rooted(&format!("x{sep}y"));
    let relative = |entry: &str| Classified::Relative(entry.to_owned());
    for (list, entries, expected) in [
        (String::new(), Vec::new(), Classified::Accepted),
        (a.clone(), vec![a.clone()], Classified::Accepted),
        (
            format!("{a}{sep}{b}"),
            vec![a.clone(), b.clone()],
            Classified::Accepted,
        ),
        (
            "../pool".to_owned(),
            vec!["../pool".to_owned()],
            relative("../pool"),
        ),
        (
            format!("{a}{sep}../pool"),
            vec![a.clone(), "../pool".to_owned()],
            relative("../pool"),
        ),
        (
            format!("\"{spanning}\""),
            vec![spanning.clone()],
            Classified::Accepted,
        ),
        (
            spanning.clone(),
            vec![rooted("x"), "y".to_owned()],
            relative("y"),
        ),
        (
            format!("{a}{other}y"),
            vec![format!("{a}{other}y")],
            Classified::Accepted,
        ),
        (
            format!("#../not-a-path{sep}{a}"),
            vec![a.clone()],
            Classified::Accepted,
        ),
        (
            format!("{sep}{sep}{a}{sep}{sep}"),
            vec![a.clone()],
            Classified::Accepted,
        ),
        (
            format!("\"\"{sep}{a}"),
            vec![a.clone()],
            Classified::Accepted,
        ),
        (
            format!("\"{a}\"/b"),
            vec![a.clone(), "b".to_owned()],
            relative("b"),
        ),
        (
            format!("\"{a}\"x{sep}{b}"),
            vec![a.clone(), b.clone()],
            Classified::Accepted,
        ),
        (
            format!("\"{a}{sep}{b}"),
            vec![format!("\"{a}"), b.clone()],
            relative(&format!("\"{a}")),
        ),
        (
            format!("\"\\q{a}\""),
            vec![format!("\"\\q{a}\"")],
            relative(&format!("\"\\q{a}\"")),
        ),
        (
            format!("\"\\400{a}\""),
            vec![format!("\"\\400{a}\"")],
            relative(&format!("\"\\400{a}\"")),
        ),
        (
            format!("\"{a}\\t\\\\\\\"\""),
            vec![format!("{a}\t\\\"")],
            Classified::Accepted,
        ),
        (
            format!("\"{a}\\101\""),
            vec![format!("{a}A")],
            Classified::Accepted,
        ),
        (
            format!("\"{a}\\000b\""),
            vec![format!("{a}\0b")],
            Classified::Unclassified,
        ),
    ] {
        let wanted: Vec<Vec<u8>> = entries
            .iter()
            .map(|entry| entry.as_bytes().to_vec())
            .collect();
        assert_eq!(
            alternate_entries(list.as_bytes(), ALTERNATES_SEPARATOR),
            wanted,
            "`{list}` decodes into the entries Git takes"
        );
        let outcome = not_absolute_alternates(OsStr::new(&list));
        match expected {
            Classified::Accepted => assert_eq!(outcome, None, "`{list}`"),
            Classified::Relative(entry) => assert_eq!(
                outcome,
                Some(NotAbsolute::Entry(OsString::from(entry))),
                "`{list}`"
            ),
            Classified::Unclassified => assert!(
                matches!(outcome, Some(NotAbsolute::Unclassified(_))),
                "`{list}`: {outcome:?}"
            ),
        }
    }
    let refusal = refuse_relative_repository_wide_values(
        REPOSITORY_WIDE_PATHS.map(|name| (name, None)),
        Some(OsStr::new("../pool")),
    )
    .expect_err("a relative entry refuses the list");
    let message = refusal.to_string();
    let list = ALTERNATE_OBJECT_DIRECTORIES;
    assert!(
        message.contains(&format!("the inherited `{list}` holds the entry `../pool`"))
            && message.contains(&format!("export `{list}` as an absolute path")),
        "the refusal names the variable and the entry: {message}"
    );
}

/// On Unix an entry is the bytes Git decodes, whatever they are: `\057` is the
/// `/` that makes an entry absolute, and an octal escape that decodes to a
/// byte no UTF-8 spells reaches the platform's rule unchanged.
#[cfg(unix)]
#[test]
fn on_unix_an_alternates_entry_is_classified_over_the_exact_bytes_git_decodes() {
    use std::os::unix::ffi::OsStringExt as _;

    assert_eq!(
        not_absolute_alternates(OsStr::new("\"\\057a\"")),
        None,
        "`\\057` decodes to `/`"
    );
    assert_eq!(
        not_absolute_alternates(OsStr::new("\"\\377x\"")),
        Some(NotAbsolute::Entry(OsString::from_vec(vec![0xff, b'x']))),
        "an entry that is not UTF-8 is still the bytes Git resolves"
    );
}

/// B-3, the Windows forms, executed on the Windows legs: a drive-relative
/// `C:x`, a bare `C:` and a root-relative `\x` or `/x` refuse, as a value and
/// as an entry of the `;`-separated list, while `C:\x`, `C:/x` and a UNC path
/// do not. Inside C-style quoting a lone backslash is an escape, so `"C:\a"`
/// decodes to `C:` and a BEL, which is not absolute, and `"C:\\a"` to `C:\a`;
/// an entry that is not Unicode once unquoted cannot be classified exactly.
#[cfg(windows)]
#[test]
fn on_windows_a_drive_or_root_relative_value_refuses_and_a_drive_or_unc_path_does_not() {
    for relative in ["C:x", "C:", "\\x", "/x"] {
        assert_eq!(
            refuse_relative_repository_wide_values(
                only("GIT_OBJECT_DIRECTORY", OsStr::new(relative)),
                None,
            ),
            Err(Refusal::RelativeRepositoryWideName {
                name: "GIT_OBJECT_DIRECTORY",
                why: NotAbsolute::Value(OsString::from(relative)),
            }),
            "`{relative}` as a value"
        );
        assert_eq!(
            not_absolute_alternates(OsStr::new(&format!("C:\\a;{relative}"))),
            Some(NotAbsolute::Entry(OsString::from(relative))),
            "`{relative}` as an entry"
        );
    }
    for absolute in ["C:\\x", "C:/x", "\\\\server\\share\\x"] {
        assert_eq!(
            refuse_relative_repository_wide_values(
                only("GIT_OBJECT_DIRECTORY", OsStr::new(absolute)),
                None,
            ),
            Ok(()),
            "`{absolute}` as a value"
        );
        assert_eq!(
            not_absolute_alternates(OsStr::new(&format!("C:\\a;{absolute}"))),
            None,
            "`{absolute}` as an entry"
        );
    }
    assert_eq!(not_absolute_alternates(OsStr::new("\"C:\\\\a\"")), None);
    assert_eq!(
        not_absolute_alternates(OsStr::new("\"C:\\a\"")),
        Some(NotAbsolute::Entry(OsString::from("C:\u{7}")))
    );
    assert!(matches!(
        not_absolute_alternates(OsStr::new("\"\\377x\"")),
        Some(NotAbsolute::Unclassified(_))
    ));
}

/// R-C-REGISTRY part 1's rule at the check itself (this head's function; the
/// red/green rows are the `derive` witnesses below), over real directories,
/// each layout under a common directory of its own. A link is a symbolic link
/// on Unix and a junction on Windows ([`plant_directory_link`]), so the
/// Windows legs execute the junction rows.
#[test]
fn a_worktrees_registry_refuses_exactly_when_its_canonical_parent_is_not_the_common_directory() {
    let tree = scratch("linked-registry-check");
    let root = strip_verbatim(fs::canonicalize(tree.path()).expect("the canonical scratch root"));
    let other = root.join("other").join("worktrees");
    create_dir(&other);
    let layout = |name: &str| {
        let common = root.join(name);
        create_dir(&common);
        let entry = common.join("worktrees");
        (common, entry)
    };

    let (common, _) = layout("absent");
    assert!(
        refuse_linked_registry(&common, false).is_ok(),
        "an absent registry, which Git makes a directory"
    );

    let (common, entry) = layout("plain");
    create_dir(&entry);
    assert!(
        refuse_linked_registry(&common, false).is_ok(),
        "a plain directory"
    );

    let (common, entry) = layout("beside");
    create_dir(&common.join("registry"));
    plant_directory_link(&common.join("registry"), &entry);
    assert!(
        refuse_linked_registry(&common, false).is_ok(),
        "a link to a direct child of the common directory, whose record still names it"
    );

    let (common, entry) = layout("linked");
    plant_directory_link(&other, &entry);
    let refused =
        refuse_linked_registry(&common, false).expect_err("a link into another directory refuses");
    assert_names_entry_and_target(&refused, &entry, &other);
    assert!(
        refuse_linked_registry(&common, true).is_ok(),
        "with a `GIT_COMMON_DIR` inherited it is part 2's layout, accepted and not refused"
    );

    let (common, entry) = layout("chained");
    let hop = root.join("hop");
    plant_directory_link(&other, &hop);
    plant_directory_link(&hop, &entry);
    let refused = refuse_linked_registry(&common, false).expect_err("a chain of links refuses");
    assert_names_entry_and_target(&refused, &entry, &other);

    let (common, entry) = layout("nested");
    let nested = common.join("nested").join("worktrees");
    create_dir(&nested);
    plant_directory_link(&nested, &entry);
    let refused = refuse_linked_registry(&common, false)
        .expect_err("a link inside the common directory whose parent is elsewhere refuses");
    assert_names_entry_and_target(&refused, &entry, &nested);

    let (common, entry) = layout("dangling");
    let gone = root.join("gone");
    create_dir(&gone);
    plant_directory_link(&gone, &entry);
    fs::remove_dir(&gone).expect("remove the link's target");
    let error = refuse_linked_registry(&common, false)
        .expect_err("a link whose target is gone is not passed as unlinked");
    assert!(matches!(error, UpstrokeError::Io { .. }), "{error}");
}

/// The names in `dir`, sorted; an absent `dir` has none.
#[cfg(unix)]
fn entry_names(dir: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// Whether `error` names `entry` and the `target` it resolves to.
fn assert_names_entry_and_target(error: &UpstrokeError, entry: &Path, target: &Path) {
    let message = refusal_of(error);
    assert!(
        message.contains(&entry.display().to_string())
            && message.contains(&format!("resolves to {}", target.display())),
        "the refusal names the entry and its target: {message}"
    );
}

/// R-C-REGISTRY part 1 at `derive`, red at the bound master and green here: a
/// `worktrees` registry linked into another repository, linked through a chain
/// of links, or linked inside the common directory to a parent elsewhere
/// refuses the manager before its registry access, naming the entry and its
/// target, while this process inherits no `GIT_COMMON_DIR`. Each link is a
/// junction on Windows, so B-3's junction row executes there.
#[test]
fn derive_refuses_a_linked_worktrees_registry_while_no_common_dir_is_inherited() {
    assert!(
        std::env::var_os(COMMON_DIR).is_none(),
        "the condition under test is that no `GIT_COMMON_DIR` is inherited, and this process \
         inherits one"
    );
    for layout in [
        "into another repository",
        "through a chain of links",
        "inside the common directory",
    ] {
        let fixture = Fixture::new("linked-registry-derive");
        let common = strip_verbatim(
            fs::canonicalize(fixture.base.join(".git")).expect("the run's common directory"),
        );
        let entry = common.join("worktrees");
        let other = fixture.root.join("other");
        create_dir(&other);
        git(&other, &["init", "-q", "-b", "main"]);
        let registry = other.join(".git").join("worktrees");
        create_dir(&registry);
        let target = match layout {
            "into another repository" => {
                plant_directory_link(&registry, &entry);
                registry
            }
            "through a chain of links" => {
                let hop = fixture.root.join("hop");
                plant_directory_link(&registry, &hop);
                plant_directory_link(&hop, &entry);
                registry
            }
            _ => {
                let nested = common.join("nested").join("worktrees");
                create_dir(&nested);
                plant_directory_link(&nested, &entry);
                nested
            }
        };
        let target =
            strip_verbatim(fs::canonicalize(&target).expect("the link's canonical target"));
        let error = WorkspaceManager::derive(
            &fixture.base,
            &fixture.private,
            super::fixture::RUN_ID,
            "inc-linked",
        )
        .expect_err("a linked registry refuses while no `GIT_COMMON_DIR` is inherited");
        assert_names_entry_and_target(&error, &entry, &target);
        assert!(
            !fixture.manager.execution_root().exists(),
            "{layout}: the refusal made nothing"
        );
    }
}

/// RG-1 and RG-4, the "symlinked registry, nothing inherited" slot run: green
/// here, where `derive` refuses before any slot and the linked repository's
/// registry, objects and refs stay as they were. Without the check -- at a
/// master that lacks it, or with it dropped from `derive`, the mutation -- the
/// slot add registers the run's work tree in the linked repository's
/// `worktrees`, and the test is red there, at the registry. The capture after
/// it is refused by the manager's guarded read, by the original registration
/// rule, before anything is staged, so the objects and refs watched beside the
/// registry are not where it turns red.
#[cfg(unix)]
#[test]
fn a_registry_linked_into_another_repository_is_refused_before_a_slot_run_writes_there() {
    assert!(
        std::env::var_os(COMMON_DIR).is_none(),
        "the condition under test is that no `GIT_COMMON_DIR` is inherited, and this process \
         inherits one"
    );
    let tree = scratch("linked-registry-slot");
    let root = tree.path();
    let base = root.join("base");
    context_repository(&base, &[("README.md", "base\n"), ("shared.txt", "seed\n")]);
    let other = root.join("other");
    context_copy_tree(&base, &other);
    fs::create_dir(other.join(".git/worktrees")).expect("the other repository's registry");
    std::os::unix::fs::symlink(other.join(".git/worktrees"), base.join(".git/worktrees"))
        .expect("the run's registry, a link into the other repository");
    fs::create_dir(root.join("private")).expect("the private root");
    let watched = [
        other.join(".git/worktrees"),
        other.join(".git/objects"),
        other.join(".git/refs"),
    ];
    let before: Vec<_> = watched
        .iter()
        .map(|path| context_tree_bytes(path))
        .collect();
    let derived = WorkspaceManager::derive(
        &base,
        &root.join("private"),
        super::fixture::RUN_ID,
        "inc-linked",
    );
    if let Ok(manager) = &derived {
        let _ = slot_run_writing_a_file(manager, &base, "linked");
    }
    for (path, earlier) in watched.iter().zip(&before) {
        assert!(
            context_tree_bytes(path) == *earlier,
            "{} was written: the slot run reached the linked repository",
            path.display()
        );
    }
    let error = derived.expect_err("the linked registry refuses before any slot");
    let common = fs::canonicalize(base.join(".git")).expect("the run's common directory");
    let target = fs::canonicalize(other.join(".git/worktrees")).expect("the link's target");
    assert_names_entry_and_target(&error, &common.join("worktrees"), &target);
}

/// The managed checkout, private root and clean execution root of the
/// relative-name `derive` witnesses, made in this process's own environment,
/// which sets none of the names.
#[cfg(unix)]
struct RelativeNameLayout {
    /// The scratch tree that holds them, whose path is the children's root.
    tree: crate::rundir::scratch_tree::ScratchTree,
    /// The manager derived over them, whose execution root must stay empty.
    manager: WorkspaceManager,
    /// The run's object store, canonical: an absolute value that names it.
    store: PathBuf,
}

/// [`RelativeNameLayout`] under a scratch tree tagged `tag`.
#[cfg(unix)]
fn relative_name_layout(tag: &str) -> RelativeNameLayout {
    let tree = scratch(tag);
    let root = tree.path();
    let managed = root.join("managed");
    context_repository(&managed, &[("README.md", "readme\n")]);
    fs::create_dir(root.join("private")).expect("the private root");
    let manager = WorkspaceManager::derive(
        &managed,
        &root.join("private"),
        super::fixture::RUN_ID,
        "inc-clean",
    )
    .expect("derive in this process's environment, which sets none of the names");
    manager
        .create_execution_root(&mut NoHooks)
        .expect("the execution root");
    let store = fs::canonicalize(managed.join(".git/objects")).expect("the run's store");
    RelativeNameLayout {
        tree,
        manager,
        store,
    }
}

/// One relative-name case: `derive` over the layout's managed checkout in an
/// isolated child whose environment carries `name` holding `value`. The child's
/// report, once it has run and the execution root still holds no intent and no
/// slot.
#[cfg(unix)]
fn derive_in_a_child(layout: &RelativeNameLayout, name: &str, value: &OsStr) -> String {
    let label = format!("`{name}={}`", Path::new(value).display());
    let ended = crate::agent::proc::test_support::run_test_isolated(
        "workspace_manager::tests::relative_name_derive_helper",
        &[
            (name, value),
            (CONTEXT_ROOT, layout.tree.path().as_os_str()),
        ],
        std::time::Duration::from_secs(180),
    );
    assert!(
        ended.status.is_some_and(|status| status.success()) && ended.stdout.contains("1 passed"),
        "{label}: {ended}"
    );
    let root = layout.manager.execution_root();
    assert!(
        entry_names(&root.join("intents")).is_empty()
            && entry_names(&root.join("tasks")).is_empty(),
        "{label}: no intent and no slot"
    );
    ended.stdout
}

/// R-C-RELSTORE at the `derive` boundary, red at the bound master and green
/// here, each case in an isolated child whose environment alone carries the
/// name: each of the six scalar names relative and empty, and an alternates
/// list with a relative entry, with the bytes a quoted absolute entry leaves
/// after its closing quote, or with broken quoting. Every case refuses, naming
/// the variable and saying to export it as an absolute path, and leaves no
/// intent and no slot in the execution root, which was made clean.
#[cfg(unix)]
#[test]
fn derive_refuses_each_relative_repository_wide_name_before_any_slot() {
    let layout = relative_name_layout("relative-names-refused");
    let listed = layout.store.display().to_string();
    let mut cases: Vec<(&str, String)> = Vec::new();
    for name in [
        "GIT_OBJECT_DIRECTORY",
        "GIT_SHALLOW_FILE",
        "GIT_GRAFT_FILE",
        "GIT_CONFIG",
        "GIT_CONFIG_GLOBAL",
        "GIT_CONFIG_SYSTEM",
    ] {
        cases.push((name, "../elsewhere".to_owned()));
        cases.push((name, String::new()));
    }
    let alternates = "GIT_ALTERNATE_OBJECT_DIRECTORIES";
    cases.extend([
        (alternates, "../pool".to_owned()),
        (alternates, format!("{listed}:../pool")),
        (alternates, format!("\"{listed}\"/pool")),
        (alternates, format!("\"{listed}:{listed}")),
    ]);
    for (name, value) in cases {
        let report = derive_in_a_child(&layout, name, OsStr::new(&value));
        assert!(
            report.contains(DERIVE_ERROR)
                && report.contains(&format!("the inherited `{name}`"))
                && report.contains(&format!("export `{name}` as an absolute path")),
            "`{name}={value}` refuses `derive`, naming the variable: {report}"
        );
    }
}

/// RS-2, and the accepting halves of RS-3 and RS-4, at the `derive` boundary:
/// green at the bound master and here, each case in an isolated child whose
/// environment alone carries the name. Each of the six scalar names holding an
/// absolute path, and an alternates list that is empty, one quoted absolute
/// entry, or a comment before an absolute entry, is not refused, and leaves no
/// intent and no slot in the execution root.
#[cfg(unix)]
#[test]
fn derive_accepts_each_absolute_repository_wide_name() {
    let layout = relative_name_layout("relative-names-accepted");
    let listed = layout.store.display().to_string();
    let store = layout.store.clone().into_os_string();
    let absent = layout.tree.path().join("absent");
    let neutral = super::fixture::neutral_git_config().as_os_str().to_owned();
    let quoted = format!("\"{listed}\"");
    let commented = format!("#../not-a-path:{listed}");
    let alternates = "GIT_ALTERNATE_OBJECT_DIRECTORIES";
    for (name, value) in [
        ("GIT_OBJECT_DIRECTORY", store),
        ("GIT_SHALLOW_FILE", absent.join("shallow").into_os_string()),
        ("GIT_GRAFT_FILE", absent.join("grafts").into_os_string()),
        ("GIT_CONFIG", absent.join("config").into_os_string()),
        ("GIT_CONFIG_GLOBAL", neutral.clone()),
        ("GIT_CONFIG_SYSTEM", neutral),
        (alternates, OsString::new()),
        (alternates, OsString::from(quoted)),
        (alternates, OsString::from(commented)),
    ] {
        let report = derive_in_a_child(&layout, name, &value);
        assert!(
            report.contains(DERIVE_DERIVED),
            "`{name}={}` is not refused: {report}",
            Path::new(&value).display()
        );
    }
}

/// RS-6 and RS-7, the "REL objects, sibling" slot run: the coordinator inherits
/// `GIT_OBJECT_DIRECTORY=../objects`, which at the base names the run's store,
/// and a store copied beside the slot is where the same value lands from the
/// slot. Red at the bound master, where the slot run's capture writes its
/// objects into the store beside the slot; green here, where `derive`
/// refuses, naming the variable and saying to export it as an
/// absolute path, and no slot or intent is made and the store beside the slot
/// stays as it was. Removing the check from `derive`, the mutation, makes it
/// red again.
#[cfg(unix)]
#[test]
fn a_relative_object_directory_is_refused_before_the_store_beside_the_slot_is_written() {
    let tree = scratch("relative-object-directory");
    let root = tree.path();
    let managed = root.join("managed");
    context_repository(
        &managed,
        &[("README.md", "readme\n"), ("shared.txt", "base\n")],
    );
    context_copy_tree(&managed.join(".git/objects"), &root.join("objects"));
    fs::create_dir(root.join("private")).expect("the private root");
    let manager = WorkspaceManager::derive(
        &managed,
        &root.join("private"),
        super::fixture::RUN_ID,
        "inc-clean",
    )
    .expect("derive in this process's environment, which sets none of the names");
    manager
        .create_execution_root(&mut NoHooks)
        .expect("the execution root");
    let tasks = manager.execution_root().join("tasks");
    let beside = tasks.join("objects");
    context_copy_tree(&root.join("objects"), &beside);
    let before = context_tree_bytes(&beside);
    let ended = crate::agent::proc::test_support::run_test_isolated(
        "workspace_manager::tests::relative_name_derive_helper",
        &[
            ("GIT_OBJECT_DIRECTORY", OsStr::new("../objects")),
            (CONTEXT_ROOT, root.as_os_str()),
            (CONTEXT_SEQUENCE, OsStr::new("slot")),
        ],
        std::time::Duration::from_secs(180),
    );
    assert!(
        ended.status.is_some_and(|status| status.success()) && ended.stdout.contains("1 passed"),
        "{ended}"
    );
    assert!(
        context_tree_bytes(&beside) == before,
        "the store beside the slot was written: under the inherited relative object directory \
         the slot run's objects landed there\n{ended}"
    );
    assert!(
        ended.stdout.contains(DERIVE_ERROR)
            && ended
                .stdout
                .contains("the inherited `GIT_OBJECT_DIRECTORY` is `../objects`")
            && ended
                .stdout
                .contains("export `GIT_OBJECT_DIRECTORY` as an absolute path"),
        "`derive` refuses, naming the variable: {ended}"
    );
    assert_eq!(
        entry_names(&tasks),
        ["objects"],
        "no slot was made beside the store"
    );
    assert!(
        entry_names(&manager.execution_root().join("intents")).is_empty(),
        "no intent was written"
    );
}

/// D7_CONFIG_REPAIR (root's checkpoint-3 disposition). `core_ignorecase` is
/// Git's effective answer, through the manager's builder, at every scope Git
/// reads, and a result Git cannot give cleanly is an error, never a guess.
///
/// Each child runs with every ambient Git control taken away and only its own
/// configuration added (`run_challenged_replacement_witness_child`), so no
/// user's repository or global file is read or written. In the `global`
/// child, the global file is a fixture file named by `GIT_CONFIG_GLOBAL` for
/// that child only. In the `environment` child, the pair is
/// `GIT_CONFIG_COUNT`/`KEY_0`/`VALUE_0`, again for that child only. Each child
/// writes a marker before it asserts, so a child that ran no test cannot pass
/// for one that did.
///
/// The controls are across children: the `neutral` child reads an untouched
/// repository as unset, and its repository-level `false` as false. That is
/// what makes the `global` child's `true` the global file's, and the
/// `environment` child's `true` over a repository-level `false` the
/// environment's.
#[test]
fn core_ignorecase_is_gits_effective_answer_at_every_scope() {
    let tree = scratch("d7-ignorecase-scopes");
    let global = tree.path().join("global.gitconfig");
    write_file(&global, b"[core]\n\tignorecase = true\n");
    let challenges: [(&str, Vec<(&str, OsString)>); 3] = [
        ("neutral", Vec::new()),
        (
            "global",
            vec![("GIT_CONFIG_GLOBAL", global.clone().into_os_string())],
        ),
        (
            "environment",
            vec![
                ("GIT_CONFIG_COUNT", OsString::from("1")),
                ("GIT_CONFIG_KEY_0", OsString::from("core.ignorecase")),
                ("GIT_CONFIG_VALUE_0", OsString::from("true")),
            ],
        ),
    ];
    for (scope, mut challenge) in challenges {
        let marker = tree.path().join(format!("ran-{scope}"));
        challenge.push((IGNORECASE_SCOPE, OsString::from(scope)));
        challenge.push((IGNORECASE_MARKER, marker.clone().into_os_string()));
        let status = super::fixture::run_challenged_replacement_witness_child(
            "workspace_manager::tests::core_ignorecase_helper",
            &challenge,
        );
        assert!(status.success(), "the `{scope}` child ended {status:?}");
        assert!(
            marker.exists(),
            "the `{scope}` child never reached the helper, so it measured nothing"
        );
    }
}
