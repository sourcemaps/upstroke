// Allowlist placement: the funnel section of `effects/allowlist.toml`.
#![allow(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros
)]

use super::*;

/// Every site the four groups this lane owns declare, derived from the
/// enums rather than written out. A group that grows a variant grows this.
fn lane_sites() -> Vec<EffectSiteId> {
    EffectSiteId::all()
        .into_iter()
        .filter(|site| {
            matches!(
                site,
                EffectSiteId::Worktree(_)
                    | EffectSiteId::Snapshot(_)
                    | EffectSiteId::Ref(_)
                    | EffectSiteId::Object(_)
            )
        })
        .collect()
}

/// `PR28-REPLAY-FAILS-OPEN`. A misspelled site name is byte-for-byte
/// indistinguishable from "this site was deliberately left to measure", so a
/// parser that only looks for the site it wants cannot tell them apart. It
/// returned `None`, the run measured fresh, and an operator who asked to
/// replay a red run got a green one that replayed nothing.
///
/// Measured before the fix: `Object.CandidateStgae=44365` produced
/// `budget=39916us` with no `replayed` marker.
#[test]
fn a_replay_spec_naming_an_unknown_site_is_refused_rather_than_ignored() {
    let stage = EffectSiteId::Object(ObjectSite::CandidateStage);
    assert_eq!(
        parse_budget_spec("Object.CandidateStgae=44365", stage),
        Err(BudgetSpecError::UnknownSite(
            "Object.CandidateStgae".to_owned()
        )),
    );
    // The control that makes the assertion mean something: the *correctly*
    // spelled name is accepted, so the refusal is about the typo and not
    // about the parser rejecting everything.
    assert_eq!(
        parse_budget_spec("Object.CandidateStage=44365", stage),
        Ok(Some(std::time::Duration::from_micros(44365))),
    );
}

/// `PR28-REPLAY-FAILS-OPEN`, second shape. The old parser took the **first**
/// entry matching the site and then tried to parse it; a malformed first
/// entry produced `None` and a later valid duplicate was never reached.
///
/// Measured before the fix: `...=abc,...=44365` produced `budget=40971us`
/// with no `replayed` marker, while `...=44365,...=abc` replayed at 44365 —
/// so which entry won depended on order, which is reason enough to refuse
/// duplicates outright.
#[test]
fn a_duplicated_site_is_refused_rather_than_resolved_by_order() {
    let stage = EffectSiteId::Object(ObjectSite::CandidateStage);
    assert_eq!(
        parse_budget_spec(
            "Object.CandidateStage=abc,Object.CandidateStage=44365",
            stage
        ),
        Err(BudgetSpecError::NotAPositiveInteger(
            "Object.CandidateStage=abc".to_owned()
        )),
    );
    assert_eq!(
        parse_budget_spec("Object.CandidateStage=1,Object.CandidateStage=2", stage),
        Err(BudgetSpecError::Duplicate(
            "Object.CandidateStage".to_owned()
        )),
    );
}

/// `PR28-REPLAY-UNBOUNDED`. `u64::MAX` parsed happily. Measured before the
/// fix: the printed ladder's first rung asked for 2_049_638_230_412_172_119
/// microseconds — about **64,949 years** — and `kill_git_child` sleeps
/// unconditionally, so the run would have ended at CI's job timeout rather
/// than at any assertion.
#[test]
fn a_replay_budget_past_the_ceiling_is_refused() {
    let stage = EffectSiteId::Object(ObjectSite::CandidateStage);
    assert_eq!(
        parse_budget_spec("Object.CandidateStage=18446744073709551615", stage),
        Err(BudgetSpecError::AboveCeiling {
            site: "Object.CandidateStage".to_owned(),
            micros: u64::MAX,
        }),
    );
    // The boundary itself is allowed, so the ceiling is a limit and not an
    // off-by-one that silently narrows what a replay may ask for.
    assert_eq!(
        parse_budget_spec(
            &format!("Object.CandidateStage={MAX_REPLAY_BUDGET_US}"),
            stage
        ),
        Ok(Some(std::time::Duration::from_micros(MAX_REPLAY_BUDGET_US))),
    );
}

/// The legitimate partial replay this must not break: pin some sites, leave
/// the rest to measure. Witnessed by hand while building the feature — two
/// sites replayed and two measured in the same run — and pinned here so the
/// validation added above cannot quietly turn "unmentioned" into an error.
#[test]
fn a_spec_that_omits_a_site_leaves_that_site_to_be_measured() {
    assert_eq!(
        parse_budget_spec(
            "Object.CandidateStage=44365",
            EffectSiteId::Object(ObjectSite::ProposalCherryPick)
        ),
        Ok(None),
    );
    assert_eq!(
        parse_budget_spec("", EffectSiteId::Object(ObjectSite::CandidateStage)),
        Ok(None),
    );
}

/// A malformed entry is refused even when it names no site this run asks
/// about, because the operator's intent was to replay and half a spec cannot
/// deliver it.
#[test]
fn a_malformed_entry_is_refused_even_for_a_site_this_run_does_not_want() {
    assert_eq!(
        parse_budget_spec(
            "Object.ProposalCherryPick",
            EffectSiteId::Object(ObjectSite::CandidateStage)
        ),
        Err(BudgetSpecError::Malformed(
            "Object.ProposalCherryPick".to_owned()
        )),
    );
}

/// The refusal `sampled_command` gives for a site it does not handle. Matched
/// rather than assumed, so a panic for any *other* reason is not silently read
/// as absence — see [`has_sampling_command`].
const NO_SAMPLING_COMMAND: &str = "is not one of the four commands the contract samples";

/// Whether `sampled_command` handles `site`, distinguishing "does not handle
/// it" from "blew up for some other reason".
///
/// Treating *every* panic as absence was a real hole: an arm added to
/// `sampled_command` but omitted from `SAMPLED_SITES` — validating its slot
/// and panicking because the test supplies a task slot — read as absent, which
/// matched its absence from the list, and the drift went undetected. Measured.
fn has_sampling_command(site: EffectSiteId, fixture: &Fixture, slot: &Slot) -> bool {
    match panic_message(|| {
        let _unused = sampled_command(site, fixture, slot);
    }) {
        None => true,
        Some(message) if message.contains(NO_SAMPLING_COMMAND) => false,
        Some(message) => panic!(
            "`{}` panicked for a reason other than being unsampled, so this test cannot \
                 tell whether it has a command: {message}",
            site.name()
        ),
    }
}

/// The partition itself, over **every** real site rather than one example.
///
/// An earlier version of this test named `Object.CandidateCommitTree` alone,
/// which asserted an instance and not the property: replacing the
/// `SAMPLED_SITES` membership check with `named == CandidateCommitTree`
/// special-cased that one site, left every other unsampled site failing open,
/// and the whole suite stayed green. Measured.
#[test]
fn every_real_site_is_either_replayable_or_refused_as_unsampled() {
    let all = EffectSiteId::all();
    assert!(
        all.len() > SAMPLED_SITES.len(),
        "there must be unsampled sites, or this test asserts nothing"
    );
    for site in all {
        let spec = format!("{}=1000", site.name());
        let got = parse_budget_spec(&spec, site);
        if SAMPLED_SITES.contains(&site) {
            assert_eq!(
                got,
                Ok(Some(std::time::Duration::from_micros(1000))),
                "{} is sampled and must be replayable",
                site.name()
            );
        } else {
            assert_eq!(
                got,
                Err(BudgetSpecError::NotSampled(site.name().to_owned())),
                "{} is a real site this sampler never drives, so naming it must \
                     refuse rather than fall through to a fresh measurement",
                site.name()
            );
        }
    }
}

/// `SAMPLED_SITES` and the command table are two statements of one fact.
/// Nothing but this test makes them agree: adding an arm to `sampled_command`
/// without adding the site here would leave the new arm unreachable through a
/// replay and unobserved by any other test.
#[test]
fn the_replayable_site_list_matches_the_commands_that_exist() {
    let fixture = Fixture::created("site-list-agreement");
    let slot = fixture.task("agree", 0);
    for site in EffectSiteId::all() {
        let has_command = has_sampling_command(site, &fixture, &slot);
        assert_eq!(
            has_command,
            SAMPLED_SITES.contains(&site),
            "{} has a sampling command but is not in SAMPLED_SITES (or the reverse); \
                 the two lists have drifted",
            site.name()
        );
    }
}

/// **Every** refusal must reach the operator through the environment edge,
/// not merely the one an earlier test happened to use.
///
/// With only `UnknownSite` covered, `Err(BudgetSpecError::NotSampled(_)) => None`
/// survived: the direct parser test still saw `NotSampled`, the edge test still
/// panicked for its typo, and a real-but-unsampled spec fell through to four
/// fresh measurements. Asserting the variant name appears also pins the
/// `{error:?}` in the message, whose removal would hide the distinction the
/// two errors exist to draw.
#[test]
fn every_refusal_reaches_the_operator_through_the_environment_edge() {
    let stage = EffectSiteId::Object(ObjectSite::CandidateStage);
    for (spec, variant) in [
        ("Object.CandidateStgae=1", "UnknownSite"),
        ("Object.CandidateCommitTree=1", "NotSampled"),
        ("Object.CandidateStage", "Malformed"),
        ("Object.CandidateStage=abc", "NotAPositiveInteger"),
        ("Object.CandidateStage=99999999", "AboveCeiling"),
        (
            "Object.CandidateStage=1,Object.CandidateStage=2",
            "Duplicate",
        ),
    ] {
        let message = panic_message(|| {
            let _unused: Option<std::time::Duration> = budget_from_var(Ok(spec.to_owned()), stage);
        })
        .unwrap_or_else(|| panic!("`{spec}` must be refused at the edge, not silently measured"));
        assert!(
            message.contains(variant),
            "the refusal for `{spec}` must name {variant} so the operator can tell \
                 the refusals apart; got: {message}"
        );
    }
}

/// The repair's own blind spot. Validating against the whole `EffectSiteId`
/// registry accepted any *real* site, including the many this sampler never
/// drives — so `Object.CandidateCommitTree` parsed, matched none of the four
/// sampled sites, and every one of them fell through to a fresh measurement
/// while the run reported success.
///
/// Measured on the unfixed repair: zero `replayed` markers, four fresh
/// budgets, `test result: ok`. No mutation was needed to expose it.
#[test]
fn a_real_site_this_sampler_never_drives_is_refused() {
    let stage = EffectSiteId::Object(ObjectSite::CandidateStage);
    assert_eq!(
        parse_budget_spec("Object.CandidateCommitTree=44365", stage),
        Err(BudgetSpecError::NotSampled(
            "Object.CandidateCommitTree".to_owned()
        )),
    );
    // Every site the sampler *does* drive is still accepted, so the new check
    // narrows the domain to exactly the replayable set and no further.
    for site in SAMPLED_SITES {
        assert_eq!(
            parse_budget_spec(&format!("{}=1000", site.name()), site),
            Ok(Some(std::time::Duration::from_micros(1000))),
            "{} is sampled and must be replayable",
            site.name()
        );
    }
}

/// `std::env::var` reports `NotPresent` and `NotUnicode` as different errors.
/// Collapsing them made a spec that was set but not valid UTF-8 look exactly
/// like no spec at all — every site measuring fresh, against the adjacent
/// promise that an unhonourable spec is refused.
///
/// This tests the environment *edge* rather than the parser. All the other
/// replay tests call `parse_budget_spec` directly, so before this existed the
/// panic arm could be replaced with `Err(_) => None` and the whole suite
/// stayed green.
#[test]
fn an_absent_variable_is_absence_and_a_present_one_is_not() {
    let stage = EffectSiteId::Object(ObjectSite::CandidateStage);
    assert_eq!(
        budget_from_var(Err(std::env::VarError::NotPresent), stage),
        None,
        "no spec means measure, which is the ordinary case"
    );
    assert_eq!(
        budget_from_var(Ok("Object.CandidateStage=44365".to_owned()), stage),
        Some(std::time::Duration::from_micros(44365)),
    );
}

/// The edge must refuse a spec the parser rejects, not just an unreadable
/// variable. Without this, replacing the parse-error arm with
/// `Err(_) => None` leaves every other replay test green: they all call
/// `parse_budget_spec` directly and never exercise what the edge does with
/// its answer.
#[test]
#[should_panic(expected = "not a spec this run can honour")]
fn a_spec_the_parser_rejects_is_refused_at_the_environment_edge() {
    let _ = budget_from_var(
        Ok("Object.CandidateStgae=44365".to_owned()),
        EffectSiteId::Object(ObjectSite::CandidateStage),
    );
}

/// The other half of the edge: a value that is present but unusable must not
/// be mistaken for absence.
#[test]
#[should_panic(expected = "not valid UTF-8")]
fn a_present_but_non_unicode_variable_is_refused_not_treated_as_unset() {
    let invalid = {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt as _;
            std::ffi::OsString::from_vec(vec![0x66, 0x6f, 0x80, 0x6f])
        }
        #[cfg(not(unix))]
        {
            use std::os::windows::ffi::OsStringExt as _;
            std::ffi::OsString::from_wide(&[0x66, 0x6f, 0xD800, 0x6f])
        }
    };
    let _ = budget_from_var(
        Err(std::env::VarError::NotUnicode(invalid)),
        EffectSiteId::Object(ObjectSite::CandidateStage),
    );
}

// -----------------------------------------------------------------------
// The Object group: rows, IdUnread, and the residue classifier
// -----------------------------------------------------------------------

/// `slice_contract.proof_tests[7]`: "after each creation primitive the
/// object is referenced by exactly the row `row()` names (index/HEAD
/// inspection; fsck for R27)".
///
/// The expected row per site comes from the frozen `ObjectSite::row()`, and
/// the *observation* comes from Git — index, HEAD, or `fsck`. The two are
/// independent: nothing here asks the site what it expects and then asks it
/// again what it found.
#[test]
fn after_each_object_primitive_the_object_is_referenced_by_the_row_row_names() {
    let fixture = Fixture::created("object-rows");
    let mut checked = Vec::new();

    // R9: Object.CandidateStage — blobs behind the task worktree index.
    let task = fixture.add_task(&mut NoHooks, "alpha", 1);
    let task_path = fixture.manager.slot_path(&task);
    fs::write(task_path.join("staged.txt"), "staged\n").expect("edit");
    let blob = git(&task_path, &["hash-object", "staged.txt"]);
    fixture
        .manager
        .candidate_stage(&mut NoHooks, &task, &[])
        .expect("stage");
    assert_eq!(ObjectSite::CandidateStage.row(), ResourceRow::R9);
    assert!(
        git(&task_path, &["ls-files", "-s"]).contains(&blob),
        "the staged blob is referenced by the task worktree index"
    );
    assert!(
        !unreachable_objects(&task_path)
            .expect("fsck")
            .contains(&blob),
        "so it is not R27"
    );
    checked.push(ObjectSite::CandidateStage);

    // R9: Object.CandidateWriteTree — trees behind that index's cache-tree.
    let tree = fixture
        .manager
        .candidate_write_tree(&mut NoHooks, &task)
        .expect("write-tree");
    assert_eq!(ObjectSite::CandidateWriteTree.row(), ResourceRow::R9);
    assert!(
        !unreachable_objects(&task_path)
            .expect("fsck")
            .contains(&tree),
        "the tree is reachable through the index's cache-tree extension: R9, not R27"
    );
    checked.push(ObjectSite::CandidateWriteTree);

    // R27: Object.SnapshotCommitTree — unreferenced until Snapshot.Add.
    let ephemeral = fixture
        .manager
        .snapshot_commit_tree(&mut NoHooks, &tree, &fixture.head)
        .expect("ephemeral commit");
    assert_eq!(ObjectSite::SnapshotCommitTree.row(), ResourceRow::R27);
    assert!(
        unreachable_objects(&fixture.base)
            .expect("fsck")
            .contains(&ephemeral),
        "the ephemeral commit is unreferenced: R27"
    );
    checked.push(ObjectSite::SnapshotCommitTree);

    // R27: Object.CandidateCommitTree — unreferenced until the pin.
    let candidate = fixture
        .manager
        .candidate_commit_tree(&mut NoHooks, &tree, &fixture.head, "candidate")
        .expect("candidate commit");
    assert_eq!(ObjectSite::CandidateCommitTree.row(), ResourceRow::R27);
    assert!(
        unreachable_objects(&fixture.base)
            .expect("fsck")
            .contains(&candidate),
        "the candidate commit is unreferenced: R27"
    );
    // …and R23 once pinned, which is the row that then accounts for it.
    fixture
        .manager
        .create_ref_zero_old(
            &mut NoHooks,
            RefSite::PinCandidatePrepared,
            "refs/upstroke/runs/run-1/candidate-prepared/kalpha/1",
            &candidate,
        )
        .expect("pin");
    assert_eq!(RefSite::PinCandidatePrepared.row(), ResourceRow::R23);
    assert!(
        !unreachable_objects(&fixture.base)
            .expect("fsck")
            .contains(&candidate),
        "the pin moves it out of R27 and into the row that references it"
    );
    checked.push(ObjectSite::CandidateCommitTree);

    // R10: Object.ProposalCherryPick — through the staging HEAD.
    let staging = Slot::Staging { sequence: 1 };
    fixture
        .manager
        .write_intent(&mut NoHooks, &staging)
        .expect("staging intent");
    let staging_path = fixture
        .manager
        .add_worktree(&mut NoHooks, &staging, &fixture.head)
        .expect("staging worktree");
    let proposal = fixture
        .manager
        .proposal_cherry_pick(&mut NoHooks, &staging, &fixture.side)
        .expect("cherry-pick");
    assert_eq!(ObjectSite::ProposalCherryPick.row(), ResourceRow::R10);
    assert_eq!(
        git(&staging_path, &["rev-parse", "HEAD"]),
        proposal,
        "the proposal commit is the staging worktree's HEAD"
    );
    assert!(
        !unreachable_objects(&staging_path)
            .expect("fsck")
            .contains(&proposal),
        "so it is not R27 while the staging worktree exists"
    );
    checked.push(ObjectSite::ProposalCherryPick);

    // R9: Object.RepairMaterialize — merge objects behind the repair index.
    let repair = fixture.add_task(&mut NoHooks, "repair", 1);
    let repair_path = fixture.manager.slot_path(&repair);
    fixture
        .manager
        .repair_materialize(&mut NoHooks, &repair, &fixture.side)
        .expect("materialize");
    assert_eq!(ObjectSite::RepairMaterialize.row(), ResourceRow::R9);
    assert!(
        index_differs_from_head(fixture.manager.base(), &repair_path).expect("index"),
        "the materialization is staged in the repair worktree's index"
    );
    let materialized = git(&repair_path, &["rev-parse", ":c.txt"]);
    assert!(
        !unreachable_objects(&repair_path)
            .expect("fsck")
            .contains(&materialized),
        "index-referenced, so R9 rather than R27"
    );
    checked.push(ObjectSite::RepairMaterialize);

    // And the domain is the enum's, not the author's memory.
    checked.sort();
    checked.dedup();
    assert_eq!(
        checked.len(),
        ObjectSite::ALL.len(),
        "every Object site the frozen enum declares has a row observation; missing: {:?}",
        ObjectSite::ALL
            .iter()
            .filter(|site| !checked.contains(site))
            .collect::<Vec<_>>()
    );

    // The scrub releases what the worktree held — and `cleanup` states the
    // disjunction the release obeys: "objects released to R27 **or
    // accounted by the candidate pin/ref**". Both halves are asserted,
    // because a test that checked only the first would be measuring
    // whichever half the fixture happened to build.
    fixture
        .manager
        .remove_worktree(&mut NoHooks, &task)
        .expect("scrub");
    assert!(
        !unreachable_objects(&fixture.base)
            .expect("fsck")
            .contains(&blob),
        "the staged blob is in the candidate commit's tree, so the candidate-prepared pin \
             (R23) still accounts for it after the scrub"
    );
    fixture
        .manager
        .delete_ref_expected_old(
            &mut NoHooks,
            RefSite::DeleteCandidatePin,
            "refs/upstroke/runs/run-1/candidate-prepared/kalpha/1",
            &candidate,
        )
        .expect("prune the pin expected-old");
    assert!(
        unreachable_objects(&fixture.base)
            .expect("fsck")
            .contains(&blob),
        "and once no pin or ref references it, it is R27"
    );
    fixture
        .manager
        .remove_worktree(&mut NoHooks, &staging)
        .expect("reclaim staging");
    assert!(
        unreachable_objects(&fixture.base)
            .expect("fsck")
            .contains(&proposal),
        "and removing the staging worktree releases the proposal objects"
    );
}

/// `slice_contract.proof_tests[7]`: "IdUnread hook tests for the
/// commit-tree primitives".
#[test]
fn the_commit_tree_primitives_consult_their_id_unread_point() {
    let fixture = Fixture::created("id-unread");
    let (mut hooks, shared) = harness();
    let tree = git(
        &fixture.base,
        &["rev-parse", &format!("{}^{{tree}}", fixture.head)],
    );
    fixture
        .manager
        .snapshot_commit_tree(&mut hooks, &tree, &fixture.head)
        .expect("ephemeral");
    fixture
        .manager
        .candidate_commit_tree(&mut hooks, &tree, &fixture.head, "candidate")
        .expect("candidate");

    let harness = shared.lock().expect("harness");
    let mut with_point = Vec::new();
    for site in lane_sites() {
        let declared = site.sub_effects().contains(&SubEffectPoint::IdUnread);
        let reached = harness.reached_point(site, SubEffectPoint::IdUnread, InjectionMode::Kill);
        assert_eq!(
            declared, reached,
            "`{site}` declares IdUnread = {declared} but the funnels reached it = {reached}"
        );
        if declared {
            with_point.push(site);
        }
    }
    assert_eq!(
        with_point.len(),
        2,
        "exactly the two commit-tree sites expose IdUnread: {with_point:?}"
    );
    assert!(
        !harness.observed(
            EffectSiteId::Object(ObjectSite::CandidateCommitTree),
            HookPhase::Point {
                point: SubEffectPoint::IdUnread,
                mode: InjectionMode::Kill,
            }
        ),
        "reaching a point is not executing its injection: nothing was armed"
    );
}

/// The durable state a kill at `IdUnread` leaves, without aborting this
/// process: the object is written and no id was recorded.
///
/// `transaction_fault_matrix[T-CAND-OBJ].resume_action` for that prefix is
/// "(a) nothing to delete: the unpinned object is left to Git (never
/// adopted)". The abort itself is exercised by
/// `a_kill_at_id_unread_aborts_before_the_id_is_recorded`, which runs in a
/// child process because `Injection::Kill` aborts by design.
#[test]
fn a_kill_at_id_unread_leaves_a_gc_owned_object_nothing_adopts() {
    let fixture = Fixture::created("id-unread-residue");
    let tree = git(
        &fixture.base,
        &["rev-parse", &format!("{}^{{tree}}", fixture.head)],
    );
    let commit = fixture
        .manager
        .candidate_commit_tree(&mut NoHooks, &tree, &fixture.head, "candidate")
        .expect("candidate commit");

    // The parent never recorded the id: exactly `IdUnread`.
    let target = ResidueTarget::new(&fixture.base);
    assert_eq!(
        classify_object_residue(
            EffectSiteId::Object(ObjectSite::CandidateCommitTree),
            &target
        )
        .expect("classify"),
        ObjectResidue::Internal
    );
    // And with the id recorded, the very same durable state is the after
    // phase. The classifier's answer is a function of the record, which is
    // what `IdUnread` is defined by the absence of.
    assert_eq!(
        classify_object_residue(
            EffectSiteId::Object(ObjectSite::CandidateCommitTree),
            &ResidueTarget::new(&fixture.base).published(&commit)
        )
        .expect("classify"),
        ObjectResidue::After
    );

    let before = unreachable_objects(&fixture.base).expect("fsck");
    assert!(before.contains(&commit));
    fixture
        .manager
        .reclaim_intents(&mut NoHooks)
        .expect("the tabled recovery");
    let after = unreachable_objects(&fixture.base).expect("fsck");
    assert!(
        after.contains(&commit),
        "fsck still lists the object unreachable and untouched: the run never deletes it"
    );
}

/// The abort half, in a child process. `Injection::Kill` calls
/// `std::process::abort` on purpose — a coordinator that died running
/// destructors would not be the thing under test — so the only way to
/// observe it is from outside.
#[test]
fn a_kill_at_id_unread_aborts_before_the_id_is_recorded() {
    let tree = scratch("id-unread-kill");
    let record = tree.path().join("record");
    let helper = Command::new(std::env::current_exe().expect("test binary"))
        .args([
            "--exact",
            "workspace_manager::tests::id_unread_kill_helper",
            "--ignored",
            "--nocapture",
        ])
        .env(ID_UNREAD_RECORD, &record)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .expect("run the helper");
    // **The oracle is the abort's own signature, not `!success()`**, which is
    // what `fixture::died_by_abort` exists to say and what this call site said
    // for itself until this sweep. The helper ends in `unreachable!`, so a
    // `Kill` that stopped killing leaves it panicking: the harness reports a
    // failed test and exits 101, `!success()` is true, and every assertion
    // below still holds — the commit-tree the helper then completes writes an
    // unreferenced object, which is `Internal` for a target with no recorded
    // id. Witnessed: with `Injection::Kill` replaced by `Injection::Proceed`
    // in `id_unread_kill_helper`, this test passed.
    assert!(
        died_by_abort(&helper.status),
        "the helper must die at the point by `std::process::abort`, not reach its own \
         `unreachable!` and panic: {:?}",
        helper.status
    );
    // **And it must be an abort by this platform's own measure, not merely
    // an exit that is neither success nor a panic** (PR #136 pass 2,
    // finding 2). Until #292, `died_by_abort` named `SIGABRT` on Unix but was
    // a negation on Windows — unsuccessful and not the panic's 101 — and a
    // negation accepts far too much: change only the Windows arm of
    // `Injection::Kill` from `abort()` to `process::exit(1)` and the helper
    // still dies at `IdUnread`, still leaves the object, and still satisfies
    // every assertion here. Its Windows arm now names the status an abort
    // exits with there; this comparison stays beside it.
    //
    // It **measures** that status rather than naming it. This runs one child
    // whose whole body is `std::process::abort()`, on this machine, moments
    // before the comparison — so the oracle is the exit status an abort
    // actually produces here, and `exit(1)` is not equal to it on either
    // platform.
    let aborted = run_kill_child(
        "workspace_manager::tests::abort_probe_helper",
        &[(ABORT_PROBE, std::ffi::OsStr::new("1"))],
    );
    assert!(
        died_by_abort(&aborted),
        "the probe must itself look like an abort, or it is measuring nothing: {aborted:?}"
    );
    assert!(
        same_end(&helper.status, &aborted),
        "the helper ended {:?}, and an abort on this platform ends {aborted:?}; those are \
         different ends, so what killed it was not `std::process::abort`",
        helper.status
    );
    let written = fs::read_to_string(&record).expect("the helper recorded its repository");
    let mut lines = written.lines();
    let repository = PathBuf::from(lines.next().expect("repository path"));
    let tree = lines.next().expect("tree").to_owned();

    // The child died at `IdUnread`: the object is in the store and no id
    // was ever recorded anywhere.
    let unreachable = unreachable_objects(&repository).expect("fsck");
    assert!(
        !unreachable.is_empty(),
        "the object the child wrote survives its death"
    );
    assert_eq!(
        classify_object_residue(
            EffectSiteId::Object(ObjectSite::CandidateCommitTree),
            &ResidueTarget::new(&repository)
        )
        .expect("classify"),
        ObjectResidue::Internal,
        "and the durable state classifies as the internal residue class"
    );
    assert!(!tree.is_empty());
    let _ = fs::remove_dir_all(repository.parent().unwrap_or(&repository));
}

/// How long [`the_abort_oracle_separates_an_abort_from_an_exit_of_one`] waits
/// for each probe. A probe ends at once when it ends as its name says, so the
/// bound bounds a probe that does not, never a healthy one.
const PROBE_BOUND: std::time::Duration = std::time::Duration::from_secs(120);

/// Launch the probe `helper` with `switch` set, and return how it ended.
///
/// Through `run_kill_child_within`, so a probe still running at
/// [`PROBE_BOUND`] is killed and reaped there, and this fails naming it rather
/// than waiting on it (#292's review round 6: the oracle's test waited on its
/// probes without a deadline).
fn probe_end(helper: &str, switch: &str) -> std::process::ExitStatus {
    run_kill_child_within(helper, &[(switch, std::ffi::OsStr::new("1"))], PROBE_BOUND)
        .unwrap_or_else(|| {
            panic!(
                "the probe `{helper}` did not end within {PROBE_BOUND:?}, and was killed and reaped"
            )
        })
}

/// The abort oracle is tested, on every platform, against the ends it must
/// not accept (PR #136 pass 2, finding 2; #292's review round 5, finding 1).
///
/// The finding was Windows-shaped: there `died_by_abort` was a *negation* —
/// unsuccessful, and not the panic's 101. A negation admits
/// `process::exit(1)`, so changing only the Windows arm of `Injection::Kill`
/// from `abort()` to `exit(1)` left `a_kill_at_id_unread_aborts_before_the_id_is_recorded`
/// green there, and #292's kill witnesses passed over children that exited
/// 134 instead of aborting. **The Unix legs cannot see that mutation** —
/// measured: with `Injection::Kill` changed to `exit(1)`, that test fails on
/// Linux at the reviewed head as well, because the Unix arm names `SIGABRT` —
/// so Linux was never going to witness the repair through that test.
///
/// This one witnesses it everywhere, by testing the oracle rather than the
/// funnel: real children, one aborting, one exiting 1 and one exiting 134, and
/// the predicates applied to each. The negation the Windows arm was accepts
/// all three; `died_by_abort`, whose Windows arm now names the status an abort
/// exits with, and a comparison against a *measured* abort accept only the
/// abort. The abort is real, so a Windows whose abort ended with another
/// status fails the first premise. Each probe is launched with a deadline
/// ([`probe_end`]): one that does not end fails the test at [`PROBE_BOUND`],
/// killed and reaped, instead of holding it.
#[test]
fn the_abort_oracle_separates_an_abort_from_an_exit_of_one() {
    let aborted = probe_end("workspace_manager::tests::abort_probe_helper", ABORT_PROBE);
    let exited = probe_end(
        "workspace_manager::tests::exit_one_probe_helper",
        EXIT_ONE_PROBE,
    );
    let exited_134 = probe_end(
        "workspace_manager::tests::exit_134_probe_helper",
        EXIT_134_PROBE,
    );

    // The premises: each child really ended the way its name says.
    assert!(
        died_by_abort(&aborted),
        "the abort probe must abort: {aborted:?}"
    );
    assert_eq!(
        exited.code(),
        Some(1),
        "the exit probe must exit 1: {exited:?}"
    );
    assert_eq!(
        exited_134.code(),
        Some(134),
        "the second exit probe must exit 134: {exited_134:?}"
    );

    // What `died_by_abort` was on Windows until #292, written out and applied
    // here so the weakness stays demonstrated rather than described:
    // unsuccessful, and not the panic's 101. It accepts EVERY child.
    let windows_form = |status: &std::process::ExitStatus| {
        const PANIC: i32 = 101;
        !status.success() && status.code() != Some(PANIC)
    };
    assert!(
        windows_form(&aborted) && windows_form(&exited) && windows_form(&exited_134),
        "the negation accepts every end, which is the finding: {aborted:?} vs {exited:?} vs \
         {exited_134:?}"
    );

    // And the oracle does not, on any platform.
    assert!(
        !died_by_abort(&exited) && !died_by_abort(&exited_134),
        "`died_by_abort` must reject an exit of 1 and an exit of 134 on every platform: \
         {exited:?}, {exited_134:?}"
    );

    // Nor does a comparison against the measured abort.
    assert!(
        !same_end(&aborted, &exited),
        "an abort and an exit of 1 must not be the same end on any platform: {aborted:?} \
         vs {exited:?}"
    );
    assert!(
        same_end(&aborted, &aborted),
        "…while an abort is the same end as itself, or the comparison rejects everything"
    );
}

/// Whether two exit statuses are the **same end**, by value.
///
/// Not "both unsuccessful": that is the negation `died_by_abort` fell back on
/// for Windows until #292, and it accepts every failing exit there is. On Unix
/// an end is a signal or a code; on Windows it is a code. Two ends are the same
/// when those agree.
fn same_end(left: &std::process::ExitStatus, right: &std::process::ExitStatus) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt as _;
        left.signal() == right.signal() && left.code() == right.code()
    }
    #[cfg(not(unix))]
    {
        left.code() == right.code()
    }
}

#[test]
fn classify_object_residue_refuses_a_site_that_registers_no_class() {
    let fixture = Fixture::created("classifier-domain");
    let target = ResidueTarget::new(&fixture.base);
    for site in lane_sites() {
        let answer = classify_object_residue(site, &target);
        assert_eq!(
            answer.is_ok(),
            !site.residue_classes().is_empty(),
            "`{site}` registers {} residue classes, so the classifier must {} it",
            site.residue_classes().len(),
            if site.residue_classes().is_empty() {
                "refuse"
            } else {
                "answer for"
            }
        );
    }
    let message = refusal_of(
        &classify_object_residue(EffectSiteId::Worktree(WorktreeSite::Verify), &target)
            .expect_err("a site with no class refuses"),
    );
    assert!(
        message.contains("registers no residue class"),
        "the refusal must name its reason: {message}"
    );
}

/// Every residue name the classifier read went through `Path::exists`, which
/// answers `false` for a permission failure, a symlink loop and a transient
/// I/O error as well as for an absent name: a git dir this process could not
/// search classified as `None`, and `administrative_residue_at` called the
/// worktree quiescent. §7: only an actual not-found is absence, and a failed
/// inspection is an error naming the name. Evaluated on the Unix legs, where
/// a mode bit binds a non-root user.
///
/// `administrative_residue_at` is driven directly: its one caller,
/// `quiescence`, answers `Missing` for a git dir `git rev-parse` cannot read
/// before it reaches the residue read (the parent's fold, deferred to the
/// parent's sweep), so the error is not observable through it today.
///
/// Witnessed failing with `name_present` reduced to
/// `Ok(dir.join(name).exists())`: every assertion below then sees an answer.
#[cfg(unix)]
#[test]
fn a_git_dir_that_cannot_be_searched_is_an_error_and_not_an_absent_residue() {
    use std::os::unix::fs::PermissionsExt as _;
    let fixture = Fixture::created("residue-unsearchable");
    let slot = fixture.add_task(&mut NoHooks, "alpha", 1);
    let path = fixture.manager.slot_path(&slot);
    let git_dir = git_dir_of(&path)
        .expect("git dir")
        .expect("linked worktree");
    let lock = git_dir.join("index.lock");
    let _restore = RestoreMode {
        path: git_dir.clone(),
    };
    fs::set_permissions(&git_dir, fs::Permissions::from_mode(0o000)).expect("make it unsearchable");
    // The injection must bite: root, or a process holding CAP_DAC_OVERRIDE,
    // reads through a mode of 000, and this test would then measure nothing.
    assert!(
        fs::symlink_metadata(&lock)
            .is_err_and(|error| error.kind() != std::io::ErrorKind::NotFound),
        "prerequisite not met: the mode bit did not bind (running as root or with \
         CAP_DAC_OVERRIDE); this test needs an unprivileged user"
    );
    let site = EffectSiteId::Object(ObjectSite::CandidateStage);
    let target = ResidueTarget::new(&fixture.base).at(&path);

    let error = observed_residue_elements(site, &target)
        .expect_err("a name that cannot be inspected is not a name that is absent");
    assert!(
        matches!(&error, UpstrokeError::Io { path, .. } if path == &lock),
        "the error names the lock it could not inspect: {error}"
    );
    let error = classify_object_residue(site, &target)
        .expect_err("the classifier does not answer for a git dir it could not search");
    assert!(
        matches!(&error, UpstrokeError::Io { path, .. } if path == &lock),
        "the error names the lock it could not inspect: {error}"
    );
    let error = administrative_residue_at(&git_dir)
        .expect_err("an unsearchable git dir is not a quiescent one");
    assert!(
        matches!(&error, UpstrokeError::Io { path, .. } if path == &lock),
        "the error names the first name it could not inspect: {error}"
    );
}

/// The three adds read "populated" from `worktree.join(".git").exists()`, so
/// a pointer file a kill left empty, or one holding anything but a `gitdir:`
/// line, was the after phase. Populated means a git dir is behind the
/// pointer as `git_dir_of` reads it, and its absence, with the worktree
/// registered, is the `RegisteredUnpopulatedWorktree` the site registers.
///
/// Both arms read `add_state`, one reading with the after phase and the
/// element as its two states. Witnessed failing with `add_state`'s git-dir
/// read restored to `worktree.join(".git").exists()` (the classifier answers
/// `After` for the rewritten pointers), with its `initializing` check dropped
/// (`After` under the lock), and with the after arm reading `Unpopulated`
/// (every populated control answers `Internal`).
#[test]
fn a_git_pointer_that_names_no_git_dir_is_a_registered_unpopulated_worktree() {
    let fixture = Fixture::created("residue-empty-pointer");
    let slot = fixture.add_task(&mut NoHooks, "alpha", 1);
    let path = fixture.manager.slot_path(&slot);
    let site = EffectSiteId::Worktree(WorktreeSite::Add);
    let target = ResidueTarget::new(&fixture.base).at(&path);
    assert_eq!(
        classify(site, &target),
        ObjectResidue::After,
        "the control: a populated worktree is the after phase"
    );
    let pointer = path.join(".git");
    let original = fs::read(&pointer).expect("the pointer");
    for content in ["", "not a pointer\n"] {
        fs::write(&pointer, content).expect("rewrite the pointer");
        assert!(
            pointer.exists(),
            "the name is still there; only what is behind it changed"
        );
        assert_eq!(
            classify(site, &target),
            ObjectResidue::Internal,
            "a pointer holding {content:?} names no git dir"
        );
        assert_eq!(
            observed_residue_elements(site, &target).expect("observe"),
            vec![ResidueElement::RegisteredUnpopulatedWorktree],
            "a pointer holding {content:?} names no git dir"
        );
    }

    // And the other unpopulated state: a populated checkout whose registration
    // still holds `git worktree add`'s `initializing` lock.
    fs::write(&pointer, original).expect("restore the pointer");
    assert_eq!(classify(site, &target), ObjectResidue::After, "restored");
    let admin = fixture
        .manager
        .revalidate_removal(&path)
        .expect("admin dir")
        .expect("the worktree is registered");
    fs::write(admin.join("locked"), "initializing\n").expect("hold the initializing lock");
    assert_eq!(
        classify(site, &target),
        ObjectResidue::Internal,
        "an initializing lock is Git's statement that the population did not finish"
    );
    assert_eq!(
        observed_residue_elements(site, &target).expect("observe"),
        vec![ResidueElement::RegisteredUnpopulatedWorktree]
    );
}

/// `name_present` reads the name's own metadata and does not follow a
/// symlink: these are names Git takes with `O_EXCL` and releases by
/// unlinking, so the name is the fact whatever it points at. The measurement
/// is executed here rather than quoted: with `index.lock` a dangling symlink,
/// `git add -A` fails with "File exists" (git 2.43). A metadata read that
/// followed the link would call that lock absent. A file link needs a
/// symlink, which the Windows guest's test user cannot create.
///
/// Witnessed failing with `symlink_metadata` replaced by `metadata` in
/// `name_present`.
#[cfg(unix)]
#[test]
fn a_dangling_index_lock_symlink_is_the_lock_git_sees() {
    let fixture = Fixture::created("residue-dangling-lock");
    let slot = fixture.add_task(&mut NoHooks, "alpha", 1);
    let path = fixture.manager.slot_path(&slot);
    let git_dir = git_dir_of(&path)
        .expect("git dir")
        .expect("linked worktree");
    let lock = git_dir.join("index.lock");
    std::os::unix::fs::symlink(git_dir.join("missing"), &lock).expect("plant the dangling link");
    assert!(fs::metadata(&lock).is_err(), "the link dangles");

    // The measurement: Git takes the name as the lock and refuses.
    fs::write(path.join("new"), "x\n").expect("something to stage");
    let output = git_out(&path, &["add", "-A"]);
    assert!(
        !output.status.success(),
        "`git add` must refuse under a surviving lock name"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("File exists"),
        "git names the lock as the reason: {stderr}"
    );

    let site = EffectSiteId::Object(ObjectSite::CandidateStage);
    let target = ResidueTarget::new(&fixture.base).at(&path);
    assert!(
        observed_residue_elements(site, &target)
            .expect("observe")
            .contains(&ResidueElement::IndexLock),
        "the dangling link is the lock"
    );
    assert_eq!(classify(site, &target), ObjectResidue::Internal);
    assert_eq!(
        administrative_residue_at(&git_dir).expect("read"),
        vec![ResidueElement::IndexLock]
    );
}

/// `command_internal_sub_effects`: "the classifier is **total** over
/// `{None, Internal, After}` for every Object site and for `Worktree.Add` /
/// `Snapshot.Add`".
///
/// Totality is proved by *producing all three at every site*, not by an
/// exhaustive `match` returning a default. The site list is
/// [`residue_classified_sites`], derived from the frozen enums — a grid over
/// the sites its author remembered is the `bounded_grid` failure this
/// project has recorded three times.
#[test]
fn the_classifier_is_total_over_three_classes_for_every_registered_site() {
    let sites = residue_classified_sites();
    assert_eq!(
        sites.len(),
        ObjectSite::ALL.len() + 3,
        "six Object sites plus Worktree.Add, Worktree.AddStaging and Snapshot.Add: {sites:?}"
    );
    for site in &sites {
        let observed = observed_three_classes(*site);
        assert_eq!(
            observed,
            [
                ObjectResidue::None,
                ObjectResidue::Internal,
                ObjectResidue::After
            ],
            "`{site}` must answer each of the three classes for the state that is that class"
        );
    }
    // And every value of the codomain was produced, which is the property
    // a per-site assertion alone would not state.
    assert_eq!(ObjectResidue::ALL.len(), 3);
}

/// Drive one site through a state of each class, in the order
/// `[None, Internal, After]`.
///
/// A site with no arm here panics rather than being skipped: that is what
/// makes the domain the enum's rather than this function's.
fn observed_three_classes(site: EffectSiteId) -> [ObjectResidue; 3] {
    let tag = format!("total-{}", site.variant().to_lowercase());
    let fixture = Fixture::created(&tag);
    let base = fixture.base.clone();
    assert!(
        unreachable_objects(&base).expect("fsck").is_empty(),
        "the fixture must start with an empty R27, or `None` would be unobservable"
    );

    match site {
        EffectSiteId::Worktree(WorktreeSite::Add | WorktreeSite::AddStaging)
        | EffectSiteId::Snapshot(SnapshotSite::Add) => {
            let slot = match site {
                EffectSiteId::Worktree(WorktreeSite::Add) => fixture.task("alpha", 1),
                EffectSiteId::Worktree(WorktreeSite::AddStaging) => Slot::Staging { sequence: 1 },
                _ => Slot::Snapshot {
                    name: SnapshotName::gates(0, 1, 1),
                },
            };
            let path = fixture.manager.slot_path(&slot);
            let none = classify(site, &ResidueTarget::new(&base).at(&path));
            register_unpopulated(&fixture, &path);
            let internal = classify(site, &ResidueTarget::new(&base).at(&path));
            fixture
                .manager
                .remove_worktree(&mut NoHooks, &slot)
                .expect("clear the residue");
            // The intent is synced before the add — the add funnel refuses
            // otherwise, which is what makes an interrupted add reclaimable.
            fixture
                .manager
                .write_intent(&mut NoHooks, &slot)
                .expect("intent");
            fixture
                .manager
                .add_worktree(&mut NoHooks, &slot, &fixture.head)
                .expect("a completed add");
            let after = classify(site, &ResidueTarget::new(&base).at(&path));
            [none, internal, after]
        }
        EffectSiteId::Object(ObjectSite::CandidateStage) => {
            let slot = fixture.add_task(&mut NoHooks, "alpha", 1);
            let path = fixture.manager.slot_path(&slot);
            fs::write(path.join("a.txt"), "edited\n").expect("unstaged change");
            let none = classify(site, &ResidueTarget::new(&base).at(&path));
            let git_dir = git_dir_of(&path).expect("git dir").expect("linked");
            // The Internal state is built so that `index.lock` is the ONLY
            // thing that makes it Internal: the edit is staged first, so
            // the index already reflects the working tree and the
            // unstaged-changes half of the after-phase says `After`. A
            // classifier that dropped the lock check would answer `After`
            // here — which is a real reachable state, a second `git add`
            // killed on an already-clean worktree — and a fixture that
            // left the change unstaged would confound the two
            // discriminators and stay green. Measured: this arm with the
            // change unstaged survives deleting the lock check from
            // `after_reference_present`.
            fixture
                .manager
                .candidate_stage(&mut NoHooks, &slot, &[])
                .expect("stage, so the index already reflects the tree");
            fs::write(git_dir.join("index.lock"), "").expect("plant the lock");
            let internal = classify(site, &ResidueTarget::new(&base).at(&path));
            fs::remove_file(git_dir.join("index.lock")).expect("clear the lock");
            fs::write(path.join("a.txt"), "edited again\n").expect("a second unstaged change");
            fixture
                .manager
                .candidate_stage(&mut NoHooks, &slot, &[])
                .expect("stage");
            let after = classify(site, &ResidueTarget::new(&base).at(&path));
            [none, internal, after]
        }
        EffectSiteId::Object(ObjectSite::CandidateWriteTree) => {
            let slot = fixture.add_task(&mut NoHooks, "alpha", 1);
            let path = fixture.manager.slot_path(&slot);
            let none = classify(site, &ResidueTarget::new(&base).at(&path));
            write_orphan(&base, "an object nothing references\n");
            let internal = classify(site, &ResidueTarget::new(&base).at(&path));
            let tree = fixture
                .manager
                .candidate_write_tree(&mut NoHooks, &slot)
                .expect("write-tree");
            let after = classify(site, &ResidueTarget::new(&base).at(&path).published(&tree));
            [none, internal, after]
        }
        EffectSiteId::Object(ObjectSite::SnapshotCommitTree | ObjectSite::CandidateCommitTree) => {
            let none = classify(site, &ResidueTarget::new(&base));
            write_orphan(&base, "an object nothing references\n");
            let internal = classify(site, &ResidueTarget::new(&base));
            let tree = git(&base, &["rev-parse", &format!("{}^{{tree}}", fixture.head)]);
            let commit = if site == EffectSiteId::Object(ObjectSite::SnapshotCommitTree) {
                fixture
                    .manager
                    .snapshot_commit_tree(&mut NoHooks, &tree, &fixture.head)
                    .expect("ephemeral commit")
            } else {
                fixture
                    .manager
                    .candidate_commit_tree(&mut NoHooks, &tree, &fixture.head, "candidate")
                    .expect("candidate commit")
            };
            let after = classify(site, &ResidueTarget::new(&base).published(&commit));
            [none, internal, after]
        }
        EffectSiteId::Object(ObjectSite::ProposalCherryPick) => {
            let slot = Slot::Staging { sequence: 1 };
            fixture
                .manager
                .write_intent(&mut NoHooks, &slot)
                .expect("intent");
            let path = fixture
                .manager
                .add_worktree(&mut NoHooks, &slot, &fixture.head)
                .expect("staging worktree");
            let bare = ResidueTarget::new(&base).at(&path).from_base(&fixture.head);
            let none = classify(site, &bare);
            let git_dir = git_dir_of(&path).expect("git dir").expect("linked");
            fs::write(git_dir.join("CHERRY_PICK_HEAD"), &fixture.side).expect("plant");
            let internal = classify(site, &bare);
            fs::remove_file(git_dir.join("CHERRY_PICK_HEAD")).expect("clear");
            let proposal = fixture
                .manager
                .proposal_cherry_pick(&mut NoHooks, &slot, &fixture.side)
                .expect("cherry-pick");
            let after = classify(
                site,
                &ResidueTarget::new(&base)
                    .at(&path)
                    .from_base(&fixture.head)
                    .published(&proposal),
            );
            [none, internal, after]
        }
        EffectSiteId::Object(ObjectSite::RepairMaterialize) => {
            let slot = fixture.add_task(&mut NoHooks, "repair", 1);
            let path = fixture.manager.slot_path(&slot);
            let none = classify(site, &ResidueTarget::new(&base).at(&path));
            let git_dir = git_dir_of(&path).expect("git dir").expect("linked");
            fs::write(git_dir.join("index.lock"), "").expect("plant the lock");
            let internal = classify(site, &ResidueTarget::new(&base).at(&path));
            fs::remove_file(git_dir.join("index.lock")).expect("clear the lock");
            fixture
                .manager
                .repair_materialize(&mut NoHooks, &slot, &fixture.side)
                .expect("materialize");
            // Measured, git 2.43: `cherry-pick --no-commit` writes
            // **`MERGE_MSG`**, not `CHERRY_PICK_HEAD` — that file is only
            // set when the pick is going to commit. So the after phase of
            // this site reads the index, and a file the frozen element list
            // does register (`CHERRY_PICK_HEAD`) is one the real command
            // never leaves. It is still constructed synthetically, because
            // `ObjectSite::RepairMaterialize.residue_elements()` registers
            // it and PR3 froze that; it will simply never appear in a
            // sampled histogram. The funnel removes the `MERGE_MSG` (and
            // `AUTO_MERGE`) the pick did write, so the after phase is the
            // index and nothing `Worktree.Verify` reads as merge state.
            assert!(
                !git_dir.join("CHERRY_PICK_HEAD").exists(),
                "a successful `cherry-pick --no-commit` sets no CHERRY_PICK_HEAD"
            );
            assert!(
                !git_dir.join("MERGE_MSG").exists() && !git_dir.join("AUTO_MERGE").exists(),
                "the funnel clears the pick's state files: a completed materialization is \
                     quiescent at its base and a retained repair generation can verify"
            );
            let after = classify(site, &ResidueTarget::new(&base).at(&path));
            [none, internal, after]
        }
        other => panic!(
            "`{other}` registers a residue class and this grid has no arm for it; the domain \
                 is the frozen enums', not this function's"
        ),
    }
}

/// [`SampledChild::command`] runs the program it is given, on every
/// platform: a spawn that went back to `Command::new("git")` fails here.
#[test]
fn sampled_git_child_runs_the_program_it_is_given() {
    let program = Path::new("sampler-probe-real-git");
    let command = SampledChild::command(program, Path::new("."), &["status".to_string()]);
    assert_eq!(command.get_program(), program.as_os_str());
}

/// [`is_mingw_real_git`] accepts the real git under each of the four MinGW
/// prefixes, and refuses Git for Windows' launchers, the MSYS2 `usr\bin\git.exe`
/// and `git` by name.
#[test]
fn sampled_git_real_git_check_accepts_mingw_prefixes_and_refuses_launchers() {
    for real in [
        r"C:\Program Files\Git\mingw64\bin\git.exe",
        r"C:\Program Files\Git\ucrt64\bin\git.exe",
        r"C:\Program Files\Git\clangarm64\bin\git.exe",
        r"C:\Program Files\Git\mingw32\bin\git.exe",
        r"C:/Program Files/Git/ucrt64\bin\git.exe",
    ] {
        assert!(is_mingw_real_git(Path::new(real)), "{real} is the real git");
    }
    for launcher in [
        r"C:\Program Files\Git\cmd\git.exe",
        r"C:\Program Files\Git\bin\git.exe",
        r"C:\Program Files\Git\usr\bin\git.exe",
        "git",
    ] {
        assert!(
            !is_mingw_real_git(Path::new(launcher)),
            "{launcher} is not the real git"
        );
    }
}

/// [`may_sample_as_git`] asked about a Windows host refuses Git for Windows'
/// launchers and `git` by name and accepts the real git; asked about any
/// other host, it accepts `git` by name. It runs on every platform, so the
/// refusal is checked on hosts that never sample on Windows.
#[test]
fn sampled_git_decision_refuses_launchers_on_a_windows_host() {
    for launcher in [
        r"C:\Program Files\Git\cmd\git.exe",
        r"C:\Program Files\Git\bin\git.exe",
        "git",
    ] {
        assert!(
            !may_sample_as_git(Path::new(launcher), true),
            "a Windows host must not sample {launcher}"
        );
    }
    let real = r"C:\Program Files\Git\ucrt64\bin\git.exe";
    assert!(
        may_sample_as_git(Path::new(real), true),
        "a Windows host samples {real}"
    );
    assert!(
        may_sample_as_git(Path::new("git"), false),
        "a host other than Windows samples git by name"
    );
}

/// [`SampledChild::spawn`] runs [`SampledChild::program`], on every platform:
/// the child records the program its own `Command` spawned, and that is the
/// program `program()` answers. On Windows it must also be the real git.
///
/// What this cannot show is stated here. Off Windows, `program()` is `git` by
/// name. A spawn that ran `git` by name instead would look the same there, so
/// only a Windows run can catch that edit. Where git resolves, nothing shows
/// that `program()` consults the decision. Where it does not resolve, both
/// Windows-side tests fail on the same predicate: this one and
/// `sampled_git_child_runs_the_resolved_real_git_on_windows`. A real git
/// outside the four MinGW prefixes is refused.
#[test]
fn sampled_git_child_spawn_runs_the_sampled_program() {
    let tree = scratch("sampled-git-spawn");
    let mut child = SampledChild::spawn(tree.path(), &["--version".to_string()]);
    let status = child.wait().expect("reap the sampled git child");
    assert!(
        status.success(),
        "{:?} --version ended {status:?}",
        child.spawned_program
    );
    assert_eq!(child.spawned_program, SampledChild::program());
    if cfg!(windows) {
        assert!(
            is_mingw_real_git(&child.spawned_program),
            "the census sampler spawned {:?}, not the real git",
            child.spawned_program
        );
    }
}

/// [`SampledChild::program`] is [`super::fixture::sampled_git`]'s: on
/// Windows the real `<MinGW prefix>\bin\git.exe` (`mingw64`, or `ucrt64` on
/// newer images), and `git` by name elsewhere.
#[test]
fn sampled_git_child_runs_the_resolved_real_git_on_windows() {
    let program = SampledChild::program();
    assert_eq!(program, super::fixture::sampled_git());
    if cfg!(windows) {
        // The MinGW prefix varies by Git for Windows build (CI's hosted image used `ucrt64`);
        // `cmd\git.exe` and the Git root's `bin\git.exe` are launchers and fail this check.
        assert!(
            is_mingw_real_git(program),
            "the census sampler runs {program:?}, not the real git"
        );
    } else {
        assert_eq!(program, Path::new("git"));
    }
}

/// A sampled git's own children die with its kill, on every platform: git
/// runs an alias whose shell writes `started`, then waits on its stdin, then
/// writes `late`; once `started` exists the child is killed, its stdin is
/// then closed, and after the wait `late` does not exist. On Unix the
/// process-group kill passes it and a kill of the direct child alone
/// (`Child::kill`) fails it, because the surviving shell reads end-of-file
/// and writes `late` before the group empties, and the wait awaits that; on
/// Windows it is what shows the job's kill reaches what git started.
///
/// The alias holds no quote of its own: bare words, `;` and `>` mean the
/// same to Git for Windows' `sh` and to a Unix `sh`, and an argument with no
/// quote in it passes through Windows' command line back to git unchanged.
#[test]
fn a_sampled_childs_own_children_die_with_its_kill() {
    let tree = scratch("sampled-git-own-children");
    let repository = tree.path();
    git(repository, &["init", "-q"]);
    let started = repository.join("started");
    let late = repository.join("late");
    let mut command = SampledChild::command(
        SampledChild::program(),
        repository,
        &[
            "-c".to_owned(),
            "alias.hold=!echo started > started; read hold; echo late > late".to_owned(),
            "hold".to_owned(),
        ],
    );
    command.stdin(Stdio::piped());
    let mut child = SampledChild::spawn_command(command);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let mut seen = started.exists();
    while !seen && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(10));
        seen = started.exists();
    }
    // Killed and reaped whether or not the shell got going, so a failure
    // below leaves nothing running. Its stdin is closed between the two: a
    // shell the kill missed reads end-of-file there and writes `late`, and
    // the wait does not end until every process git started is gone.
    let killed = child.kill();
    drop(child.child.stdin.take());
    let status = child.wait().expect("reap the sampled git child");
    assert!(
        seen,
        "the alias's shell never wrote {} within 60 s; git ended {status:?}",
        started.display()
    );
    assert!(
        !late.exists(),
        "{} exists: a process the sampled git started outlived its kill (the kill answered \
         {killed:?})",
        late.display()
    );
    assert_eq!(
        launch_end(&status),
        LaunchEnd::Killed,
        "the sampled git ended {status:?}, not by the kill (the kill answered {killed:?})"
    );
}

/// No sampled funnel builds a Git argument from a literal (Fable's
/// `PR5-CONF-004`).
///
/// Sharing the lists makes the *transcription* impossible.
/// `command_internal_sub_effects` (ii) says the sampled child is "the Git
/// child of the site", and a child spawned with a different argv is a
/// different child however faithful the list is. The two axes are the *shared
/// list* and the *call site that uses it*: sharing covers the first, and only
/// reading the call sites can see the second.
///
/// # This is a tripwire, not a guarantee, and it says so
///
/// **Every claim this census used to make has been withdrawn**, over three
/// frontier passes that each walked around it a different way (PR #136,
/// findings `PR136-P3-SAMPLED-ARGV-CENSUS-IS-FAIL-OPEN`,
/// `PR136-PASS2-P1-CENSUS-EXTENT-FORGED-BY-A-LITERAL` and
/// `PR136-PASS3-P1-CENSUS-EXTENT-FORGED-BY-A-DECOY`). A census over source
/// text cannot hold a behaviour claim, and this one has now been shown not to
/// three times:
///
/// * a sibling spelling: `argv.push("--ignore-errors".into())` adds a fixed
///   Git argument and moves neither `OsString::from` count;
/// * a false boundary: a raw string holding a line of four spaces and a brace
///   ended the body early, so the declared counts were satisfied by what was
///   injected before the cut;
/// * a decoy: the scan takes the **first global** `.find` of the signature and
///   never proves the match is in `impl WorkspaceManager`, so an earlier
///   method of that name with an empty body on another type absorbs it while
///   the real one grows an argument.
///
/// The first two are closed — literals are counted, the extent comes from the
/// blanked text. **The third is not, and no third repair is being attempted**:
/// two passes produced two forgeries, and a third piece of machinery in the
/// same place is what this pull request stopped doing.
///
/// So what it asserts is exactly this and nothing more: **these four funnel
/// bodies contain no inline literal Git argument, and hold the declared number
/// of literals and `OsString::from` arguments, today.** It is a tripwire that
/// catches a careless edit. It does not establish that the sampled child runs
/// the funnel's argv, and **§12's domain-boundary requirement is unmet here**,
/// which the row records rather than this doc pretending otherwise. The
/// structural version — comparing what the funnel passes against what
/// `sampled_command` passes, as values at run time — needs a seam in the
/// parent's argv construction and is a deferred row.
#[test]
fn no_sampled_funnel_builds_its_argv_from_a_literal() {
    // (function, how many `OsString::from(<expression>)` arguments it adds
    // beyond its shared list, how many string literals its body holds at all,
    // and what those literals are).
    const SAMPLED: &[(&str, usize, usize, &str)] = &[
        (
            "pub fn add_worktree(",
            1,
            1,
            "the commit is the dynamic argument, the path is a PathBuf; the one literal is \
             the `operation: \"create\"` field of a Filesystem error, not a Git argument",
        ),
        (
            "pub fn candidate_stage(",
            2,
            0,
            "two dynamic arguments and no literal: `CANDIDATE_STAGE_MANIFEST_EXCLUSION` \
             completed by the manifest's spelling, appended to the shared list only when an \
             untracked, unignored regular file holds the manifest's name (`manifest_name`), \
             which the sampler's populated worktree never does, so its child runs the list \
             bare as the funnel does there; and `CANDIDATE_STAGE_MANIFEST_CLEAN_PATHSPEC` \
             completed the same way, the argument of the separate `clean` child that removes \
             the worker's manifest; the declared-resolution children it runs before the \
             sampled `add -A` take their argv from `DeclaredResolution::argv`, whose fixed \
             words are `RESOLUTION_ADD_ARGV` and `RESOLUTION_RM_ARGV`",
        ),
        (
            "pub fn candidate_write_tree(",
            0,
            1,
            "no dynamic argument; the one literal is the name closure 1's check before the \
             write gives the read in its refusal (`answered_whole`, the external-prune \
             follow-up), not a Git argument",
        ),
        (
            "pub fn proposal_cherry_pick(",
            1,
            2,
            "the commit to pick is the dynamic argument; the two literals are the argv of a \
             SECOND, unsampled Git child, `git rev-parse HEAD`, which this funnel runs after \
             the pick to read the proposal commit",
        ),
    ];
    // CRLF normalized first: the Windows legs check this tree out with it,
    // and `find("\n    }\n")` does not match `\r\n    }\r\n`. Measured — this
    // census passed on Linux and panicked "the function ends" on the guest.
    let source =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/workspace_manager.rs"))
            .expect("the funnel module's source")
            .replace("\r\n", "\n");
    for (signature, dynamic, literal, what) in SAMPLED {
        let counts = sampled_argv_counts(&source, signature);
        assert_eq!(
            counts.from_literal, 0,
            "`{signature}` builds {} Git argument(s) from a string literal; every fixed \
                 argument belongs in the shared list the kill sampler reads, or the sampler \
                 stops running the command the funnel runs",
            counts.from_literal
        );
        assert_eq!(
            counts.dynamic, *dynamic,
            "`{signature}` adds {} dynamic Git argument(s), not {dynamic} ({what}); if that \
                 is deliberate, `sampled_command` needs the same one",
            counts.dynamic
        );
        // The literal count, which is what the two above do not see (PR #136
        // pass 1, finding 3): `"--ignore-errors".into()` is a fixed Git
        // argument beside the shared list and moves neither of them. Every
        // literal a body holds is declared, so growing one — for an argument
        // or for anything else — is a number that stops matching rather than
        // a change nobody reads.
        assert_eq!(
            counts.literals, *literal,
            "`{signature}` holds {} string literal(s), not {literal} ({what}); a literal \
                 added here may be a Git argument the kill sampler does not run",
            counts.literals
        );
    }
}

/// How many `OsString::from("…")` and how many `OsString::from(…)` the body of
/// `signature` holds, in that order.
///
/// **Comments blanked, string literals kept.** The needle lives *inside* a
/// literal — `OsString::from("` ends in the literal's own opening quote — so
/// the sibling blanker, which blanks a literal including its quotes, would
/// search for bytes the haystack can no longer contain; `blank_comments`' own
/// documentation says exactly that, which is why there are two of them.
/// Blanking at all is §12 and `PR4-CENSUS-COMMENT-ORACLE`: a census that
/// counts a comment counts prose, and a funnel body is where a comment about
/// `OsString::from(` is most likely to be written.
///
/// Takes the source as text so
/// [`the_sampled_argv_census_counts_code_and_never_comments`] can drive it
/// against an injected violation.
fn sampled_argv_counts(source: &str, signature: &str) -> SampledArgvCounts {
    // **The extent is found structurally, the counting is not** (PR #136
    // pass 2, finding 1). The needles live inside string literals, so the
    // counting text must keep them — and a text that keeps its literals can
    // carry a *false function boundary*: a raw string holding a line of four
    // spaces and a brace ends the body early, and the census then reads a
    // truncated function while its declared numbers are satisfied by whatever
    // was injected before the cut. The reviewer's reproduction added a Git
    // argument and a boundary together and stayed green.
    //
    // So the boundary comes from `blank_comments_and_strings`, where no
    // literal survives to forge one, and the counting from `blank_comments`
    // over the same byte range. The first blanker replaces each blanked byte
    // with a space rather than deleting it, so one range indexes both texts —
    // that is the property this depends on, and it is that function's, tested
    // there.
    let structural = crate::effects::blank_comments_and_strings(source);
    let at = structural
        .find(signature)
        .unwrap_or_else(|| panic!("`{signature}` is no longer in this file"));
    let start = at + signature.len();
    let end = start
        + structural[start..]
            .find("\n    }\n")
            .expect("the function ends");
    let body = crate::effects::blank_comments(&source[start..end]);
    let bytes = body.as_bytes();
    let mut literals = 0_usize;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            literals += 1;
            i += 1;
            while i < bytes.len() && bytes[i] != b'"' {
                i += if bytes[i] == b'\\' { 2 } else { 1 };
            }
        }
        i += 1;
    }
    SampledArgvCounts {
        from_literal: body.matches("OsString::from(\"").count(),
        dynamic: body.matches("OsString::from(").count(),
        literals,
    }
}

/// What [`sampled_argv_counts`] found in one funnel body.
#[derive(Debug, PartialEq, Eq)]
struct SampledArgvCounts {
    /// `OsString::from("…")` — a fixed Git argument spelt inline.
    from_literal: usize,
    /// `OsString::from(…)` of any kind, fixed or dynamic.
    dynamic: usize,
    /// **Every** string literal in the body, whatever it is for.
    ///
    /// The two counts above match two exact spellings, and a census over
    /// source text is satisfied by any sibling spelling — PR #136 pass 1,
    /// finding 3, with the reproduction `"--ignore-errors".into()`, which adds
    /// a fixed Git argument beside the shared list while leaving both of them
    /// unchanged. This count is not fooled by that one, because the argument
    /// is still a literal. It is fooled by a `const`, a variable or a call,
    /// which is why the claim this census carries is narrowed to what it can
    /// actually support and the structural version is a deferred row.
    literals: usize,
}

/// The tripwire's own controls: what it does catch, and one thing it does not.
///
/// The first violation is the one the census was written for — a fixed Git
/// argument spelt `OsString::from("…")` beside the shared list. The second is
/// pass 1's: `"--ignore-errors".into()` is the same defect in another
/// spelling, and it left both `OsString::from` counts where they were, which
/// is why literals are counted at all. The negative case is the same text in a
/// comment, which is what blanking is for; the third case shows the extent is
/// taken from the blanked text, so a literal cannot end the body early.
///
/// **None of this makes the census a guarantee.** A `const`, a variable, a
/// call, or the decoy function of pass 3's finding 1 all pass every assertion
/// here. The controls prove it catches what it claims to catch, and the claim
/// is small.
#[test]
fn the_sampled_argv_census_counts_code_and_never_comments() {
    let clean = "\
impl WorkspaceManager {
    pub fn sampled(&self) {
        let mut argv = fixed(&Self::SAMPLED_ARGV);
        // Never OsString::from(\"--force\") here: it belongs in the list.
        argv.push(OsString::from(commit));
    }
}
";
    let comment = "// Never OsString::from(\"--force\") here: it belongs in the list.";
    assert_eq!(
        sampled_argv_counts(clean, "pub fn sampled("),
        SampledArgvCounts {
            from_literal: 0,
            dynamic: 1,
            literals: 0,
        },
        "a comment naming either construct is prose and is not an argument"
    );

    let inline = clean.replace(comment, "argv.push(OsString::from(\"--force\"));");
    assert_eq!(
        sampled_argv_counts(&inline, "pub fn sampled("),
        SampledArgvCounts {
            from_literal: 1,
            dynamic: 2,
            literals: 1,
        },
        "an argument spelt `OsString::from(\"…\")` must move every count that can see it"
    );

    // **The reviewer's false-boundary reproduction** (PR #136 pass 2,
    // finding 1): a raw string carrying a line of four spaces and a brace,
    // added together with a Git argument. Over unblanked text the scan stops
    // inside that raw string and reads a body short of the real one, and the
    // injected argument makes the truncated counts match what was declared.
    // The structural extent is not fooled, so the counts are the true ones.
    let truncating = clean.replace(
        comment,
        "argv.push(OsString::from(flag));\n        let _boundary = r\"\n    }\n\";",
    );
    assert_eq!(
        sampled_argv_counts(&truncating, "pub fn sampled("),
        SampledArgvCounts {
            from_literal: 0,
            dynamic: 2,
            literals: 1,
        },
        "the extent comes from the blanked text, so the whole body is read: two \
         `OsString::from(` and one literal, the raw string itself — a scan that stopped \
         inside that raw string would see the second `argv.push` and neither of the \
         literals after it"
    );

    let sibling = clean.replace(comment, "argv.push(\"--ignore-errors\".into());");
    let counts = sampled_argv_counts(&sibling, "pub fn sampled(");
    assert_eq!(
        (counts.from_literal, counts.dynamic),
        (0, 1),
        "the reviewer's reproduction: `.into()` moves neither `OsString::from` count, which \
         is exactly why this census was fail-open"
    );
    assert_eq!(
        counts.literals, 1,
        "…and it is the literal count that sees it, which is the repair"
    );
}

/// [`report_sample`] names what a failed sample left, each field of it: a
/// slot built as `sample_site` builds one for `Object.ProposalCherryPick`,
/// whose `HEAD` reflog ends in a torn line naming an object no repository
/// holds, reported with a kill record and a refusal handed to it.
///
/// The reflog is written where the store binds the worktree's registration
/// (`registration_of`), not where the report looks for it, so a report that
/// read another reflog misses these bytes. The alive answer is asked twice,
/// of a child spawned as a sampled child is that waits on its stdin, and
/// then of the same child once it has ended, so neither a constant answer
/// nor a dropped one passes: on Unix it is the child's process group that
/// is asked, on Windows its job's active process count.
#[test]
fn the_census_sample_report_names_what_a_failed_sample_left() {
    let site = EffectSiteId::Object(ObjectSite::ProposalCherryPick);
    let fixture = Fixture::created("sample-report");
    let slot = sample_slot(site, &fixture, 0);
    fixture
        .manager
        .write_intent(&mut NoHooks, &slot)
        .expect("intent");
    let path = fixture.manager.slot_path(&slot);
    fixture
        .manager
        .add_worktree(&mut NoHooks, &slot, &fixture.head)
        .expect("worktree");
    populate_for_sampling(site, &path);
    let reflog_file = super::fixture::registration_of(&fixture.manager, &path)
        .join("logs")
        .join("HEAD");

    let zero = "0".repeat(40);
    let missing = "0123456789abcdef0123456789abcdef01234567";
    let head = &fixture.head;
    // The last line is torn mid-entry, without its newline.
    let reflog = format!(
        "{zero} {head} upstroke tests <tests@upstroke.local> 1700000000 +0000\treset: moving to \
         {head}\n{head} {missing} upstroke tests <tests@upstr"
    );
    write_file(&reflog_file, reflog.as_bytes());

    // A child that lives until its stdin closes, spawned as a sampled child
    // is: in a process group of its own on Unix, in a job of its own on
    // Windows.
    let mut waiting = SampledChild::command(
        SampledChild::program(),
        &path,
        &["cat-file".to_owned(), "--batch".to_owned()],
    );
    waiting.stdin(Stdio::piped());
    let live = SampledChild::spawn_command(waiting);
    let pgid = live.pgid();
    let mut kill = SampleKill {
        launch: SampledLaunch {
            argv: sampled_command(site, &fixture, &slot).0,
            after: std::time::Duration::from_millis(3),
            fired: Some(std::time::Duration::from_millis(5)),
            kill_error: Some("a kill error handed to the report".to_owned()),
            end: LaunchEnd::Failed(Some(7)),
            group_ended: std::time::Duration::from_micros(250),
        },
        child: live,
    };
    // fsck's words, naming the missing object twice, a present one once, and
    // a run of hex digits too short to be an id.
    let refusal = format!(
        "`git fsck --unreachable` failed (exit code 2): error: worktrees/s0/HEAD: invalid \
         reflog entry {missing}\nerror: worktrees/s0/HEAD: invalid reflog entry {head}\n\
         error: worktrees/s0/HEAD: invalid reflog entry {missing}\nerror: invalid reflog \
         entry abc123"
    );

    // The file is not compared by name: Git may spell the path it prints
    // otherwise than the store binds it (a resolved `/private/var` on macOS,
    // `/` for `\` on Windows). These bytes are this reflog's alone, and the
    // closing quote right after them is the torn line's end.
    let report = report_sample(&fixture, &path, &kill, Some(&refusal));
    assert!(
        report.contains(&format!("all {} bytes of ", reflog.len()))
            && report.contains(&format!(": \"{}\"", reflog.as_bytes().escape_ascii())),
        "the report holds the reflog's bytes, its torn last line exactly as it is: {report}"
    );
    assert!(
        report.contains(&format!("object {missing}: missing")),
        "the report says the id the refusal names is missing: {report}"
    );
    assert!(
        report.contains(&format!("object {head}: present")),
        "the report says an id the refusal names that the repository holds is present: {report}"
    );
    assert_eq!(
        (
            report.matches(&format!("object {missing}")).count(),
            report.matches("object ").count()
        ),
        (1, 2),
        "each 40-digit id the refusal names is reported once, and nothing shorter is: {report}"
    );
    assert!(
        report.contains("kill error: a kill error handed to the report;"),
        "the report holds the kill error it was handed: {report}"
    );
    assert!(
        report.contains(&format!("end: {:?};", LaunchEnd::Failed(Some(7)))),
        "the report holds the LaunchEnd it was handed: {report}"
    );
    assert!(
        report.contains(&format!(
            "fired {:?}, {:?} past the {:?} rung it was aimed at",
            std::time::Duration::from_millis(5),
            std::time::Duration::from_millis(2),
            std::time::Duration::from_millis(3)
        )),
        "the report holds when the kill fired against its rung: {report}"
    );
    // On Windows the count is git's to make, so only that it is a "yes" with
    // a count is asserted while the child lives.
    let alive_while_live = if cfg!(unix) {
        format!("alive: yes: process group {pgid} still has a member")
    } else {
        format!("alive: yes: the job of pid {pgid} has an active process count of ")
    };
    assert!(
        report.contains(&alive_while_live),
        "the report answers whether a process of the sample is alive: {report}"
    );

    drop(kill.child.child.stdin.take());
    // The sampler's own wait: the reap, then the group (Unix) or the job
    // (Windows) awaited empty within `fixture::GROUP_END_BOUND`.
    let status = kill.child.wait().expect("reap the waiting git child");
    assert!(status.success(), "the waiting child ended {status:?}");
    let ended = report_sample(&fixture, &path, &kill, None);
    let alive_once_ended = if cfg!(unix) {
        format!("alive: no: process group {pgid} has no member")
    } else {
        format!("alive: no: the job of pid {pgid} has an active process count of 0")
    };
    assert!(
        ended.contains(&alive_once_ended),
        "the alive answer is asked when the report is made: {ended}"
    );
    assert!(
        !ended.contains("object "),
        "without a refusal no object is named: {ended}"
    );

    // A reflog longer than the bound: its last bytes only.
    let long = [
        b"FIRST-BYTES".as_slice(),
        [b'x'; REPORTED_REFLOG_BYTES].as_slice(),
    ]
    .concat();
    write_file(&reflog_file, &long);
    let report = report_sample(&fixture, &path, &kill, None);
    assert!(
        report.contains(&format!(
            "the last {REPORTED_REFLOG_BYTES} of {} bytes of ",
            long.len()
        )) && report.contains(&format!(
            ": \"{}\"",
            long[long.len() - REPORTED_REFLOG_BYTES..].escape_ascii()
        )) && !report.contains("FIRST-BYTES"),
        "a longer reflog is reported by its last {REPORTED_REFLOG_BYTES} bytes: {report}"
    );

    let recovered = recover_sample(&fixture, &slot);
    assert!(
        matches!(recovered, Ok(true)),
        "the slot recovers: {recovered:?}"
    );
    let gone = report_sample(&fixture, &path, &kill, None);
    assert!(
        gone.contains(&format!(
            "HEAD reflog: absent: there is no slot at {}",
            path.display()
        )),
        "a slot the recovery removed is reported absent: {gone}"
    );
}

/// [`head_reflog`] reads a reflog whose path holds a byte no UTF-8 spells:
/// it keeps `git rev-parse --git-path logs/HEAD`'s answer as bytes and
/// decodes it with `parsers::decode_path`, so the report holds the reflog's
/// bytes where reading the answer as UTF-8 said "not UTF-8" and lost them
/// (§8: a path is never assumed to be UTF-8).
///
/// The worktree asked is a linked one at an ASCII path, not the byte-named
/// repository's main worktree: there Git answers the relative
/// `.git/logs/HEAD`, which reaches the file with no byte of the name in it,
/// while a linked worktree's answer is the absolute path of its reflog inside
/// the byte-named repository's `.git/worktrees/<name>/`. Unix, where such a
/// path exists at all, and skipped where the filesystem refuses the name
/// with `EILSEQ`, as
/// `an_absent_add_target_in_a_byte_named_repository_still_classifies` is.
#[cfg(unix)]
#[test]
fn the_report_reads_a_reflog_whose_path_is_not_utf8() {
    use std::os::unix::ffi::OsStringExt as _;
    let tree = scratch("byte-named-reflog");
    let root = tree.path();
    let repo = root.join(std::ffi::OsString::from_vec(b"repo-\xff".to_vec()));
    match fs::create_dir_all(&repo) {
        Ok(()) => {}
        Err(error) if error.raw_os_error() == Some(libc::EILSEQ) => {
            eprintln!(
                "skipped: this filesystem refuses a directory name that is not UTF-8 — {error} \
                 (errno {:?}, EILSEQ) — so the byte-named repository whose reflog is read cannot \
                 exist here, and neither can the path this test reads it by",
                error.raw_os_error()
            );
            return;
        }
        Err(error) => panic!("a repository directory Git can name and UTF-8 cannot: {error}"),
    }
    for args in [
        &["init", "-q", "-b", "main"][..],
        &["config", "user.email", "tests@upstroke.local"],
        &["config", "user.name", "upstroke tests"],
    ] {
        let output = git_out(&repo, args);
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    fs::write(repo.join("a.txt"), "one\n").expect("seed file");
    for args in [&["add", "-A"][..], &["commit", "-q", "-m", "seed"]] {
        let output = git_out(&repo, args);
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let worktree = root.join("linked");
    git_os(
        &repo,
        &[
            OsStr::new("worktree"),
            OsStr::new("add"),
            OsStr::new("-q"),
            worktree.as_os_str(),
            OsStr::new("-b"),
            OsStr::new("linked"),
        ],
    );

    // The worktree's administrative directory, the one entry Git made under
    // the repository's `.git/worktrees/`.
    let admins = fs::read_dir(repo.join(".git").join("worktrees"))
        .expect("the repository's .git/worktrees")
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()
        .expect("the entries of .git/worktrees");
    let [admin] = admins.as_slice() else {
        panic!("one linked worktree is registered: {admins:?}");
    };
    let head = git(&worktree, &["rev-parse", "HEAD"]);
    let zero = "0".repeat(40);
    // The last line is torn mid-entry, without its newline.
    let bytes = format!(
        "{zero} {head} upstroke tests <tests@upstroke.local> 1700000000 +0000\tbranch: Created \
         from HEAD\n{head} {zero} upstroke tests <tests@upstr"
    )
    .into_bytes();
    // `write_file` makes `logs/` when it is absent.
    write_file(&admin.join("logs").join("HEAD"), &bytes);

    let answered = head_reflog(&worktree);
    assert!(
        answered.contains(&format!("all {} bytes of ", bytes.len()))
            && answered.contains(&format!(": \"{}\"", bytes.escape_ascii())),
        "the reflog of a worktree whose repository's path is not UTF-8 is read, its torn last \
         line exactly as it is: {answered}"
    );
}

// -----------------------------------------------------------------------
// ST-07 for this lane: every site, both phases
// -----------------------------------------------------------------------

/// `fault_injection_registry.completeness_rule`: "every site x hook phase …
/// is observed executed at least once by the suite … an unobserved site,
/// phase, point, or mode fails".
///
/// Restricted to the four groups this lane owns, and derived from their
/// `ALL` slices, so a group that gains a variant fails this until a funnel
/// executes it.
#[test]
fn every_site_this_lane_owns_executes_both_hook_phases() {
    let fixture = Fixture::created("grand-tour");
    let (mut hooks, shared) = harness();
    let manager = &fixture.manager;
    let integration = "refs/heads/upstroke/run-1";
    let candidates = "refs/upstroke/runs/run-1/candidates/kalpha/1";
    let pin = "refs/upstroke/runs/run-1/candidate-prepared/kalpha/1";
    let prepared = "refs/upstroke/runs/run-1/prepared/1";

    // The execution root already exists (Fixture::created), so run the site
    // again: it is idempotent and this is the tour's first observation.
    manager
        .create_execution_root(&mut hooks)
        .expect("Worktree.CreateExecutionRoot");
    manager
        .create_ref_zero_old(
            &mut hooks,
            RefSite::CreateIntegration,
            integration,
            &fixture.head,
        )
        .expect("Ref.CreateIntegration");

    // A task worktree, its capture, and its snapshot.
    let task = fixture.task("alpha", 1);
    manager
        .write_intent(&mut hooks, &task)
        .expect("Worktree.WriteIntent");
    let task_path = manager
        .add_worktree(&mut hooks, &task, &fixture.head)
        .expect("Worktree.Add");
    manager
        .verify_worktree(&mut hooks, &task, &Quiescence::AtBase(fixture.head.clone()))
        .expect("Worktree.Verify")
        .expect("quiescent");
    fs::write(task_path.join("worker.txt"), "worker\n").expect("worker edit");
    manager
        .candidate_stage(&mut hooks, &task, &[])
        .expect("Object.CandidateStage");
    let tree = manager
        .candidate_write_tree(&mut hooks, &task)
        .expect("Object.CandidateWriteTree");

    let snapshot = manager
        .add_snapshot(
            &mut hooks,
            &SnapshotName::gates(0, 1, 1),
            &SnapshotInput::Tree {
                tree: oid(&tree),
                parent: oid(&fixture.head),
            },
        )
        .expect("Object.SnapshotCommitTree + Snapshot.WriteIntent + Snapshot.Add");
    manager
        .remove_snapshot(&mut hooks, &snapshot)
        .expect("Snapshot.Remove + Snapshot.RemoveIntent");

    let candidate = manager
        .candidate_commit_tree(&mut hooks, &tree, &fixture.head, "candidate")
        .expect("Object.CandidateCommitTree");
    manager
        .create_ref_zero_old(&mut hooks, RefSite::PinCandidatePrepared, pin, &candidate)
        .expect("Ref.PinCandidatePrepared");
    manager
        .create_ref_zero_old(
            &mut hooks,
            RefSite::CreateCandidates,
            candidates,
            &candidate,
        )
        .expect("Ref.CreateCandidates");
    manager
        .delete_ref_expected_old(&mut hooks, RefSite::DeleteCandidatePin, pin, &candidate)
        .expect("Ref.DeleteCandidatePin");
    manager
        .remove_worktree(&mut hooks, &task)
        .expect("Worktree.Remove");
    manager
        .remove_intent(&mut hooks, &task)
        .expect("Worktree.RemoveIntent");

    // A repair worktree, for the last Object site.
    let repair = fixture.task("repair", 1);
    manager.write_intent(&mut hooks, &repair).expect("intent");
    manager
        .add_worktree(&mut hooks, &repair, &fixture.head)
        .expect("worktree");
    manager
        .repair_materialize(&mut hooks, &repair, &fixture.side)
        .expect("Object.RepairMaterialize");
    manager
        .remove_worktree(&mut hooks, &repair)
        .expect("remove");
    manager.remove_intent(&mut hooks, &repair).expect("intent");

    // The stale integration transaction: staging, cherry-pick, pin, CAS.
    let staging = Slot::Staging { sequence: 1 };
    manager
        .write_intent(&mut hooks, &staging)
        .expect("Worktree.WriteStagingIntent");
    manager
        .add_worktree(&mut hooks, &staging, &fixture.head)
        .expect("Worktree.AddStaging");
    let proposal = manager
        .proposal_cherry_pick(&mut hooks, &staging, &fixture.side)
        .expect("Object.ProposalCherryPick");
    manager
        .create_ref_zero_old(&mut hooks, RefSite::PinPrepared, prepared, &proposal)
        .expect("Ref.PinPrepared");
    manager
        .compare_and_swap_ref(
            &mut hooks,
            RefSite::CompareAndSwapIntegration,
            integration,
            &fixture.head,
            &proposal,
        )
        .expect("Ref.CompareAndSwapIntegration");
    manager
        .delete_ref_expected_old(&mut hooks, RefSite::DeletePreparedPin, prepared, &proposal)
        .expect("Ref.DeletePreparedPin");
    manager
        .remove_worktree(&mut hooks, &staging)
        .expect("Worktree.RemoveStaging");
    manager
        .remove_intent(&mut hooks, &staging)
        .expect("Worktree.RemoveStagingIntent");

    // The exact-base fast sequence: it creates no staging worktree,
    // cherry-picks nothing, and takes no prepared pin. The absence is
    // proved *inside* a sequence that demonstrably happened.
    shared
        .lock()
        .expect("harness")
        .begin_fast_sequence("exact-base-fast");
    let fast_task = fixture.task("fast", 1);
    manager
        .write_intent(&mut hooks, &fast_task)
        .expect("intent");
    let fast_path = manager
        .add_worktree(&mut hooks, &fast_task, &proposal)
        .expect("worktree");
    fs::write(fast_path.join("fast.txt"), "fast\n").expect("edit");
    manager
        .candidate_stage(&mut hooks, &fast_task, &[])
        .expect("stage");
    let fast_tree = manager
        .candidate_write_tree(&mut hooks, &fast_task)
        .expect("write-tree");
    let fast_commit = manager
        .candidate_commit_tree(&mut hooks, &fast_tree, &proposal, "fast candidate")
        .expect("commit-tree");
    manager
        .compare_and_swap_ref(
            &mut hooks,
            RefSite::CompareAndSwapIntegration,
            integration,
            &proposal,
            &fast_commit,
        )
        .expect("the fast publication is a CAS of the candidate commit itself");
    manager
        .remove_worktree(&mut hooks, &fast_task)
        .expect("remove");
    manager
        .remove_intent(&mut hooks, &fast_task)
        .expect("intent");
    shared.lock().expect("harness").end_fast_sequence();

    manager
        .delete_ref_expected_old(
            &mut hooks,
            RefSite::DeleteCandidatesRef,
            candidates,
            &candidate,
        )
        .expect("Ref.DeleteCandidatesRef");
    manager
        .remove_execution_root(&mut hooks)
        .expect("Worktree.RemoveExecutionRoot");

    // The bijection, over the enums rather than over a list.
    let harness = shared.lock().expect("harness");
    let sites = lane_sites();
    assert_eq!(
        sites.len(),
        WorktreeSite::ALL.len()
            + SnapshotSite::ALL.len()
            + RefSite::ALL.len()
            + ObjectSite::ALL.len(),
        "the lane's site count comes from the frozen enums"
    );
    assert_eq!(sites.len(), 29, "eleven + four + eight + six");
    let mut missing: Vec<String> = Vec::new();
    for site in &sites {
        for phase in HookPhase::PHASES {
            if !harness.observed(*site, *phase) {
                missing.push(format!("{site} {phase}"));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "every site of this lane executes both hook phases; unobserved: {missing:?}"
    );

    // The no-execution record, per sequence and not per process.
    let sequence = harness
        .fast_sequence("exact-base-fast")
        .expect("the suite exercised a fast sequence");
    for absent in [
        EffectSiteId::Worktree(WorktreeSite::AddStaging),
        EffectSiteId::Object(ObjectSite::ProposalCherryPick),
        EffectSiteId::Ref(RefSite::PinPrepared),
    ] {
        assert!(
            !sequence.ran(absent),
            "`{absent}` must not execute for a fast sequence"
        );
    }
    assert!(
        !sequence.touched().is_empty(),
        "and the absence has to be proved inside a sequence that really ran"
    );
    assert!(
        harness.touched(EffectSiteId::Object(ObjectSite::ProposalCherryPick)),
        "…while the suite as a whole did exercise the stale path, so the absence is a \
             statement about the trace and not about the process"
    );
}

/// The index a worktree-inspecting **read** leaves behind, so a test can say
/// what a classification wrote.
fn worktree_index(path: &Path) -> Vec<u8> {
    let index = git_dir_of(path)
        .expect("inspect the git directory")
        .expect("a linked worktree has one")
        .join("index");
    fs::read(&index).expect("the index file")
}

/// Make the index stat-dirty and nothing else: one tracked file's timestamp
/// moves to `seconds` after the epoch while its content stays byte for byte
/// what the index records.
///
/// This is the whole input. A refresh is then the only thing that would write
/// the index, so an index whose bytes move across the call under test moved
/// because that call refreshed it.
///
/// **Into the past, and a different second per call.** Git will not cache a
/// stat that is not older than the index it is writing — the racily-clean
/// rule, at one-second granularity where the build has no nanosecond support —
/// so a file rewritten *now* leaves a refresh with nothing it is allowed to
/// write back, and the measurement reads clean for a reason that has nothing to
/// do with the repair. Measured on git 2.43 in a linked worktree: rewriting the
/// file byte for byte moved no index at all, the same file aged to 2001 moved
/// it, and aging it twice to the same second moved it only once.
fn age_tracked_file(path: &Path, seconds: u64) {
    let file = path.join("a.txt");
    let handle = fs::OpenOptions::new()
        .write(true)
        .open(&file)
        .expect("open a tracked file the fixture seeded");
    handle
        .set_times(
            fs::FileTimes::new()
                .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(seconds)),
        )
        .expect("age the file the index recorded a stat for");
}

/// `proposal_state` classifies a failed pick and writes **nothing** — not the
/// index (`decisions.effect_site_inventory.{mechanism,identity,claim_scope}`).
///
/// It takes no hooks and names no effect site, on the stated ground that it
/// "creates no object, moves no ref and touches no index". Its unmerged-entry
/// query was the porcelain `git diff`, which silently runs `update-index
/// --refresh` against the working tree first (`diff.autoRefreshIndex`, default
/// true) — and that refresh writes: measured on git 2.43,
/// `open(index.lock, O_RDWR|O_CREAT|O_EXCL)` and `rename(index.lock, index)`,
/// moving the index's hash with the 209 bytes unchanged. So the one function
/// in the manager that claims to be a pure read was writing a resource outside
/// every effect hook, which is exactly the observation the effect-site
/// inventory exists to make impossible.
///
/// Both diffs the classifier runs are inside the hashed window, so this covers
/// the `--cached` one too. The classification itself is pinned elsewhere —
/// `integrate::tests::a_conflicting_candidate_is_rejected_with_an_atomic_repair_before_any_repair_effect`
/// drives the conflict arm through this same function, and the empty arm is
/// asserted here — so a repair that bought silence by answering differently
/// would fail one of the two.
#[test]
fn the_proposal_classifier_writes_no_index_while_reading_an_empty_pick() {
    let fixture = Fixture::created("proposal-state-readonly");
    let staging = Slot::Staging { sequence: 1 };
    fixture
        .manager
        .write_intent(&mut NoHooks, &staging)
        .expect("the staging intent");
    let path = fixture
        .manager
        .add_worktree(&mut NoHooks, &staging, &fixture.head)
        .expect("the staging worktree");
    fixture
        .manager
        .proposal_cherry_pick(&mut NoHooks, &staging, &fixture.side)
        .expect("the first pick applies the side change");
    let head = git(&path, &["rev-parse", "HEAD"]);
    fixture
        .manager
        .proposal_cherry_pick(&mut NoHooks, &staging, &fixture.side)
        .expect_err("the same change a second time is an empty pick");

    age_tracked_file(&path, 1_000_000_100);
    let before = worktree_index(&path);
    let state = fixture
        .manager
        .proposal_state(&staging, &head)
        .expect("the classifier reads the worktree");
    let after = worktree_index(&path);

    assert!(
        matches!(state, ProposalState::Empty),
        "an already-present change with no unmerged entry, HEAD where it was and \
         CHERRY_PICK_HEAD left behind is the empty pick: {state:?}"
    );
    assert_eq!(
        before.len(),
        after.len(),
        "the index is the same size across the read"
    );
    assert!(
        before == after,
        "a read that names no effect site rewrote the {}-byte index it was only asked to \
         classify, and answered {state:?} either way, so nothing about the answer needed the \
         write",
        before.len()
    );
}

/// The manager's worktree-inspecting reads write no index either
/// (`PR5-CONF-002`, swept).
///
/// That finding named the mechanism — Git's porcelain takes the index lock
/// opportunistically to write back a refreshed stat cache — and
/// `--no-optional-locks` was applied at the one call site it was found at.
/// Every other read through `read_only_git` kept the behaviour, and one of them
/// is reached from the residue classifier: `git status --porcelain` moves the
/// index's hash after nothing but a timestamp did, measured on git 2.43. That
/// makes the classifier's own optional lock a thing a later classification can
/// read as evidence — an `index.lock` left by a crash inside it is read as
/// proof that the interrupted `git add` never published.
#[test]
fn a_worktree_inspecting_read_writes_no_index() {
    let fixture = Fixture::created("read-only-reads");
    let slot = fixture.add_task(&mut NoHooks, "alpha", 1);
    let path = fixture.manager.slot_path(&slot);

    age_tracked_file(&path, 1_000_000_100);
    let before = worktree_index(&path);
    let unstaged = worktree_has_unstaged_changes(fixture.manager.base(), &path)
        .expect("inspect the working tree");
    let midpoint = worktree_index(&path);
    age_tracked_file(&path, 1_000_000_200);
    let staged = index_differs_from_head(fixture.manager.base(), &path).expect("inspect the index");
    let after = worktree_index(&path);
    assert!(
        before == midpoint,
        "`git status` rewrote the {}-byte index it was only asked to read",
        before.len()
    );

    assert!(
        !unstaged,
        "the fixture worktree holds exactly what its index records"
    );
    assert!(!staged, "and its index holds exactly HEAD");
    assert!(
        before == after,
        "a read that classifies residue rewrote the {}-byte index it was classifying",
        before.len()
    );
}

/// How a name is planted for the table below.
#[derive(Clone, Copy)]
enum Planted {
    File,
    Directory,
}

/// The paths this git's own `prune -n` names as stale temporaries — "Removing
/// stale temporary file …" and "… directory …" — with `\` read as `/` so a
/// Windows spelling of the store compares the same.
fn prune_names(repository: &Path) -> Vec<String> {
    let output = git_out(repository, &["prune", "-n"]);
    assert!(
        output.status.success(),
        "git prune -n: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            line.strip_prefix("Removing stale temporary file ")
                .or_else(|| line.strip_prefix("Removing stale temporary directory "))
        })
        .map(|path| path.replace('\\', "/"))
        .collect()
}

/// [`temporary_object_files`] answers for the thirteen names planted below
/// as `git prune -n` answers for them — the same `git prune -n` is read after
/// each planting, so the table is Git's and not this test's.
///
/// `resource_accounting[R27]` says "Git prunes temporary object files itself",
/// so `git prune` is the authority on which files these are. Measured on
/// git 2.43.0 by planting one candidate name in each plausible place and
/// reading `git prune -n`
/// (`~/tactus-artifacts/tmpobj-evidence/03-git-prune-own-set.log`,
/// `21-r5-git-prune-producers-outside-the-sample.log`): every row below
/// marked `true` was named "Removing stale temporary file" — or "directory",
/// for the one directory — and the rows marked `false` were not:
/// `objects/ab/tmp_other_fanout` is reported as a *bad sha1 file* and counted
/// in `garbage`, `objects/info/tmp_info` is left alone, and `repack`'s
/// `.tmp-<pid>-pack-*` is not named. The
/// same `git prune -n` is run here after each planting: a row the predicate
/// and this git disagree on fails, so for the thirteen names planted below
/// the table is this git's and not this test's
/// (`PR258-PRUNE-SET-IS-A-SELF-ORACLE`). That is the whole of what the
/// cross-check establishes. It plants fixed names and discovers none from a
/// producer, so a git that adopts a new temporary spelling while `prune`
/// keeps naming the old ones passes every row here with the scan blind to
/// the new name (`PR258-PRUNE-ORACLE-FUTURE-COVERAGE-OVERSTATED`).
///
/// The fan-out rows are the ones that were missed. A loose object's
/// temporary file is written in the fan-out directory the object's final
/// name will live in by `hash-object -w` and `write-tree`
/// (`02-strace-where-git-writes.log`), so a scan of the root and `pack`
/// alone never saw the file a killed write leaves: a real `SIGKILL`
/// requested at half of a separately measured 3.878 s write left
/// `objects/b7/tmp_obj_ybqfZf` and this function answered `false`
/// (`G4-TEMP-OBJECT-FANOUT-UNSCANNED`, `01-real-kill-frozen.log`). The root
/// rows are real too: a streamed object above `core.bigFileThreshold` is
/// written at the root, its fan-out unknown until the stream ends
/// (`20-r5-strace-git-writes-in-three-places.log`). `00` and `ff` are the
/// two ends of the range the scan probes, and a range short by one at either
/// end answers `false` for exactly one of them
/// (`PR258-FANOUT-RANGE-BOUNDS-UNWITNESSED`).
///
/// Each name is planted **alone** and removed again, so each row is this
/// function's answer to that name and nothing else.
#[test]
fn temporary_object_files_answers_for_the_files_git_prunes_as_its_own() {
    let fixture = Fixture::new("temp-object-scan");
    let objects = object_directory(&fixture.base).expect("object directory");
    // `pack` and `info` are created empty by `git init` itself (git 2.43.0,
    // `32-r6-git-init-directories.log`). The planting places are made to
    // exist here whatever the fixture's history, and `create_dir_all` is a
    // no-op for the ones already there.
    for directory in ["pack", "info", "00", "ab", "ff"] {
        fs::create_dir_all(objects.join(directory)).expect("a store directory");
    }

    assert!(
        !temporary_object_files(&fixture.base).expect("scan"),
        "a store Git has not been killed inside holds none"
    );
    assert_eq!(
        prune_names(&fixture.base),
        Vec::<String>::new(),
        "and this git's `prune -n` names no stale temporary in it"
    );

    for (relative, planted_as, git_calls_it_its_own) in [
        // The object root: any `tmp_` name ...
        ("tmp_obj_root", Planted::File, true),
        ("tmp_other_root", Planted::File, true),
        // ... including receive-pack's quarantine directory, which is there
        // for the length of a push into the repository and which prune
        // removes as "a stale temporary directory".
        ("tmp_objdir-incoming-AbCdEf", Planted::Directory, true),
        // The pack directory: any `tmp_` name, which is the pack Git was
        // writing, the index it writes beside it, and the reverse index.
        ("pack/tmp_pack_p", Planted::File, true),
        ("pack/tmp_idx_p", Planted::File, true),
        ("pack/tmp_rev_p", Planted::File, true),
        // A fan-out directory: `tmp_obj_` is Git's temporary loose object —
        // at both ends of the range the scan probes, and in the middle.
        ("00/tmp_obj_first", Planted::File, true),
        ("ab/tmp_obj_fanout", Planted::File, true),
        ("ff/tmp_obj_last", Planted::File, true),
        // A fan-out name that is not `tmp_obj_`: `git prune -n` reported the
        // planted `ab/tmp_other_fanout` as a bad sha1 file, counted in
        // `garbage`, and did not name it, so it is not this element.
        ("ab/tmp_other_fanout", Planted::File, false),
        // `git prune -n` left the planted `info/tmp_info` alone.
        ("info/tmp_info", Planted::File, false),
        // `repack` names its in-flight pack `.tmp-<pid>-pack-*`; `git prune -n`
        // did not name the two planted here, so R27's "Git prunes temporary
        // object files itself" does not cover them and neither does this.
        (".tmp-1-pack-x.pack", Planted::File, false),
        ("pack/.tmp-1-pack-y.pack", Planted::File, false),
    ] {
        let planted = objects.join(relative);
        match planted_as {
            Planted::File => fs::write(&planted, b"half an object\n").expect("plant"),
            Planted::Directory => {
                fs::create_dir(&planted).expect("plant");
                fs::write(planted.join("held"), b"an object in quarantine\n").expect("plant");
            }
        }
        assert_eq!(
            temporary_object_files(&fixture.base).expect("scan"),
            git_calls_it_its_own,
            "{relative}: `git prune` {} call it a stale temporary",
            if git_calls_it_its_own {
                "does"
            } else {
                "does not"
            }
        );
        let named = prune_names(&fixture.base);
        assert_eq!(
            named
                .iter()
                .any(|name| name.ends_with(&format!("objects/{relative}"))),
            git_calls_it_its_own,
            "{relative}: this git's own `prune -n` {} name it, and the table says it {}: {named:?}",
            if git_calls_it_its_own {
                "does not"
            } else {
                "does"
            },
            if git_calls_it_its_own {
                "does"
            } else {
                "does not"
            }
        );
        match planted_as {
            Planted::File => fs::remove_file(&planted).expect("unplant"),
            Planted::Directory => fs::remove_dir_all(&planted).expect("unplant"),
        }
        assert!(
            !temporary_object_files(&fixture.base).expect("scan"),
            "{relative}: and removing it leaves the store with none"
        );
    }
}

/// The scan reads a store's real shape without walking what is not a fan-out.
///
/// A loose object's own name is 38 hexadecimal characters in a two-character
/// directory, and neither is a temporary file; `pack` holds packs and their
/// indexes; `info` holds `alternates` and `packs`. None of them is an answer.
/// The scan resolves Git's 256 canonical two-digit names by construction, so
/// `objects/abc` is never a candidate against the shipped design; the decoy
/// assertion below cannot fail against it and is kept as the guard against a
/// revert to name filtering with a loose prefix match
/// (`PR258-DECOY-DOC-OVERSTATES-ITS-GUARD`). A regular file at a two-digit
/// name resolves but is not a directory, so it is not a fan-out and is not
/// read as one — without that guard `read_dir` on it would be an inspection
/// error, `NotADirectory`, for a store nothing is wrong with
/// (`PR258-IS-DIR-GUARD-UNWITNESSED`).
#[test]
fn the_temporary_object_scan_answers_no_for_a_store_of_ordinary_objects() {
    let fixture = Fixture::new("temp-object-scan-negative");
    let objects = object_directory(&fixture.base).expect("object directory");
    assert!(
        !temporary_object_files(&fixture.base).expect("scan"),
        "the fixture's own objects are not temporary files"
    );

    // A directory that begins with two hex digits but is not a fan-out, and a
    // `tmp_obj_` name inside it: not Git's, not descended into.
    let decoy = objects.join("abc");
    fs::create_dir_all(&decoy).expect("a decoy directory");
    fs::write(decoy.join("tmp_obj_decoy"), b"not in a fan-out\n").expect("plant");
    assert!(
        !temporary_object_files(&fixture.base).expect("scan"),
        "`objects/abc` is not a fan-out directory, so nothing in it is one of Git's"
    );

    // And a fan-out directory that holds only real loose objects.
    let fan_out = objects.join("cd");
    fs::create_dir_all(&fan_out).expect("a fan-out directory");
    fs::write(
        fan_out.join("ef0123456789abcdef0123456789abcdef0123"),
        b"a loose object's name is not a temporary file\n",
    )
    .expect("plant");
    assert!(
        !temporary_object_files(&fixture.base).expect("scan"),
        "a fan-out holding only objects holds no temporary object file"
    );

    // And a regular file at a fan-out name: resolved, not a directory, not
    // read.
    let (file_at_a_fan_out_name, _) = unused_alphabetic_fan_out_pair(&objects);
    fs::write(
        &file_at_a_fan_out_name,
        b"a file where a fan-out could be\n",
    )
    .expect("plant");
    assert!(
        !temporary_object_files(&fixture.base)
            .expect("a regular file at a fan-out name is not an inspection failure"),
        "a regular file at a two-digit name is not a fan-out"
    );
}

/// Reserve an absent pair, independently of the fixture's timestamp-dependent
/// object names. Checking both spellings also works on case-sensitive stores.
fn unused_alphabetic_fan_out_pair(objects: &Path) -> (PathBuf, PathBuf) {
    for prefix in 0_u8..=255 {
        let name = format!("{prefix:02x}");
        let upper_name = name.to_ascii_uppercase();
        if name == upper_name {
            continue;
        }
        let lower = objects.join(name);
        let upper = objects.join(upper_name);
        let absent = [&lower, &upper]
            .into_iter()
            .all(|path| match fs::symlink_metadata(path) {
                Ok(_) => false,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
                Err(error) => panic!("inspect unused fan-out {}: {error}", path.display()),
            });
        if absent {
            return (lower, upper);
        }
    }
    panic!("the small fixture must leave an unused alphabetic fan-out pair");
}

/// The declaration under which
/// `the_temporary_object_scan_resolves_case_aliases_as_the_filesystem_does`
/// *requires* its native casefold branch rather than observing it. Set to
/// `1`, it says that the temporary directory this suite runs under folds
/// case: of the steps that run the full suite, `ci.yml` sets it on the
/// `macos-latest` test step and on the suite step of `test-windows-hosted`,
/// which runs the Windows suite on `windows-latest`; the capture steps after
/// each carry the same map. Unset, empty, or anything but `1`, the test
/// asserts the scan against whatever the filesystem does.
const TEMP_FOLDS_CASE: &str = "UPSTROKE_TEST_TEMP_FOLDS_CASE";

/// Every platform constructs the upper-case witness and asserts the scan
/// against its native lookup result. An occupied `ab` cannot suppress it.
/// Unix additionally supplies a lower-case symlink alias when the filesystem
/// distinguishes the spellings, so ordinary case-sensitive CI also executes
/// positive alias detection.
///
/// **Which branch guards what.** The native branch — Git's lower-case path
/// resolving to the stored upper-case fan-out — is the one that guards
/// `PR258-CASEFOLD-FANOUT-INVISIBLE`, and it is *required* only where the
/// leg declares it: `ci.yml` sets `TEMP_FOLDS_CASE` to `1` on the
/// `macos-latest` test step and on the suite step of `test-windows-hosted`,
/// whose temporary directories fold case, so a green there records that this
/// branch ran (both legs passed the round-5 assertion of that branch at
/// `f4a351be`, where the Windows leg was the `winguest` step of the
/// self-hosted guest, retired on 2026-11-20).
/// Anywhere the variable is not `1` the branch is observed, not required:
/// on a case-sensitive volume — the ubuntu leg, the build box the ten gates
/// run on, a developer whose `TMPDIR` is a case-sensitive APFS volume — the
/// native assertion is `false == false`, which any implementation that
/// answers `false` there satisfies, the round-2 name filter included, and
/// the symlink half then guards the link-following repair and nothing about
/// case. Round 5 keyed the requirement on the target OS instead,
/// `cfg!(any(target_os = "macos", windows))`, which fails this test on a
/// supported case-sensitive volume before the scan is checked; the round-5
/// regression lens executed the equivalent `cfg!(unix)` branch on Linux
/// (`PR258-CASEFOLD-EXPECTATION-KEYED-ON-TARGET-OS`). So the casefold P1 is
/// guarded by a gate on the legs that declare the variable -- macOS, and
/// `test-windows-hosted` -- and by no gate on the ubuntu leg or on
/// this box
/// (`PR258-CASEFOLD-GUARD-PLATFORM-SHAPED`).
#[test]
fn the_temporary_object_scan_resolves_case_aliases_as_the_filesystem_does() {
    let fixture = Fixture::new("temp-object-case-alias");
    let objects = object_directory(&fixture.base).expect("object directory");
    fs::create_dir_all(objects.join("ab")).expect("an occupied ordinary fan-out");
    let (lower, upper) = unused_alphabetic_fan_out_pair(&objects);
    fs::create_dir(&upper).expect("the upper-case fan-out");
    fs::write(upper.join("tmp_obj_case_alias"), b"half an object\n").expect("plant");
    let native_alias = match fs::metadata(&lower) {
        Ok(metadata) => {
            assert!(
                metadata.is_dir(),
                "the native alias resolves to a directory"
            );
            true
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => panic!("inspect the lower-case lookup {}: {error}", lower.display()),
    };
    // Required only where the leg declares it: `ci.yml` sets
    // `UPSTROKE_TEST_TEMP_FOLDS_CASE=1` on the macOS test step and the suite
    // step of `test-windows-hosted`, and a green there records that the
    // native branch of this guard ran.
    // Everywhere else the branch is observed, so a case-sensitive volume —
    // a developer's `TMPDIR` on case-sensitive APFS — takes the negative
    // branch below instead of failing here.
    if std::env::var_os(TEMP_FOLDS_CASE).is_some_and(|value| value == "1") {
        assert!(
            native_alias,
            "{TEMP_FOLDS_CASE}=1 declares that the temporary directory {} folds case, but Git's \
             lower-case path {} did not resolve to the stored upper-case fan-out: the leg's \
             declaration and its filesystem disagree",
            std::env::temp_dir().display(),
            lower.display()
        );
    }
    assert_eq!(
        temporary_object_files(&fixture.base).expect("scan stored upper-case fan-out"),
        native_alias,
        "Git reaches the stored upper-case directory exactly when its lower-case path resolves"
    );

    #[cfg(unix)]
    {
        if !native_alias {
            std::os::unix::fs::symlink(&upper, &lower).expect("provide the lower-case alias");
        }
        assert!(
            temporary_object_files(&fixture.base).expect("scan through the lower-case alias"),
            "the lower-case alias reaches the temporary object on every Unix test filesystem"
        );
    }

    fs::remove_file(upper.join("tmp_obj_case_alias")).expect("unplant");
    assert!(
        !temporary_object_files(&fixture.base).expect("scan after removing residue"),
        "removing the temporary object leaves neither spelling with residue"
    );
}

/// Execute with TMPDIR (Unix) or the system temporary directory (Windows) on a
/// case-insensitive filesystem. This fails its prerequisite instead of silently
/// passing on a case-sensitive volume. It never runs in CI — nothing passes
/// `--ignored` — and on a case-sensitive volume the default alias test above
/// takes its negative branch, which a `read_dir` filter that discards stored
/// upper-case names before resolving Git's lower-case path also satisfies; so
/// this witness is what rejects that filter on Linux, and it was executed on
/// ext4 casefold (`09-r3-targeted.log`, `13-r3-mutations-M9-native-casefold.log`)
/// rather than by a gate.
#[test]
#[ignore = "requires a case-insensitive temporary filesystem; run explicitly with --ignored"]
fn a_native_case_insensitive_fan_out_alias_is_detected() {
    let fixture = Fixture::new("temp-object-native-case-alias");
    let objects = object_directory(&fixture.base).expect("object directory");
    let (lower, upper) = unused_alphabetic_fan_out_pair(&objects);
    fs::create_dir(&upper).expect("the stored upper-case fan-out");
    assert!(
        fs::metadata(&lower)
            .expect("this integration test requires native case-insensitive path lookup")
            .is_dir(),
        "Git's lower-case path resolves to the stored upper-case directory"
    );
    assert!(
        !temporary_object_files(&fixture.base).expect("scan the empty alias"),
        "an empty alias is not temporary residue"
    );
    fs::write(upper.join("tmp_obj_native_alias"), b"half an object\n").expect("plant");
    assert!(
        temporary_object_files(&fixture.base).expect("scan the native case alias"),
        "a temporary object reachable through Git's canonical path must be detected"
    );
    fs::remove_file(upper.join("tmp_obj_native_alias")).expect("unplant");
    assert!(
        !temporary_object_files(&fixture.base).expect("scan after removing residue"),
        "removing the temporary object leaves the native alias empty"
    );
}

/// A symlinked fan-out directory is one Git writes into and prunes from, so
/// the scan follows it.
///
/// `DirEntry::file_type` reports the **link**, not its target, so a scan that
/// read it walked straight past `objects/93 -> elsewhere` and answered `false`
/// for a file Git had just left inside. Found by #258's class review with a
/// real `SIGKILL` inside a `hash-object -w`: the kill left
/// `objects/93/tmp_obj_HuHQtZ`, `git prune -n` named it a stale temporary
/// file, `unreachable_objects` was empty, and the classifier answered `None`
/// with no element observed — this scan's original defect surviving its own
/// repair. [`fan_out_directories`] resolves the target with `fs::metadata`.
///
/// Unix only, because creating a directory symlink on Windows needs a
/// privilege the CI runner does not hold. The resolution itself is
/// `fs::metadata` on every platform.
#[cfg(unix)]
#[test]
fn a_symlinked_fan_out_directory_is_followed_as_git_follows_it() {
    let fixture = Fixture::new("temp-object-symlink");
    let objects = object_directory(&fixture.base).expect("object directory");
    let elsewhere = fixture.root.join("fan-out-elsewhere");
    fs::create_dir_all(&elsewhere).expect("the directory the fan-out points at");
    let (fan_out, _) = unused_alphabetic_fan_out_pair(&objects);
    std::os::unix::fs::symlink(&elsewhere, &fan_out).expect("the unused fan-out symlink");

    assert!(
        !temporary_object_files(&fixture.base).expect("scan"),
        "a symlinked fan-out holding nothing holds no temporary object file"
    );

    fs::write(elsewhere.join("tmp_obj_HuHQtZ"), b"half an object\n").expect("plant");
    assert!(
        temporary_object_files(&fixture.base).expect("scan"),
        "Git writes tmp_obj_* through the fan-out link and prunes it through the link, \
         so the scan reads through the link"
    );

    fs::remove_file(elsewhere.join("tmp_obj_HuHQtZ")).expect("unplant");
    assert!(
        !temporary_object_files(&fixture.base).expect("scan"),
        "and removing it leaves the store with none"
    );

    // A dangling link is a name that is gone, which holds nothing and is not
    // an inspection failure.
    fs::remove_dir_all(&elsewhere).expect("drop the target");
    assert!(
        !temporary_object_files(&fixture.base).expect("a dangling fan-out link is not an error"),
        "a fan-out link with no target holds no temporary object file"
    );
}

/// An inspection failure at a later canonical name cannot discard an answer an
/// earlier fan-out already gave; once nothing earlier answers, the failure is
/// the caller's to see rather than a silent `false`.
///
/// Round 4's `fan_out_directories` collected all 256 probes before the caller
/// read any, so a symlink loop at a later name turned a store whose
/// `objects/00` held residue from `Ok(true)` into `Err(FilesystemLoop)`
/// (`PR258-EAGER-FANOUT-PROBE-DISCARDS-A-KNOWN-TRUE`); the probe is streamed
/// into the read now. The loop is also the witness for the `fs::metadata`
/// error arm: replaced by a skip, the scan reads `Ok(false)` for a store it
/// could not inspect (`PR258-ERROR-ARMS-SILENCEABLE`).
///
/// Unix only, because the loop is a symlink.
#[cfg(unix)]
#[test]
fn a_fan_out_link_that_loops_is_an_inspection_error_once_no_earlier_fan_out_answers() {
    let fixture = Fixture::new("temp-object-symlink-loop");
    let objects = object_directory(&fixture.base).expect("object directory");
    let first = objects.join("00");
    fs::create_dir_all(&first).expect("the first fan-out the scan probes");
    fs::write(first.join("tmp_obj_early"), b"half an object\n").expect("plant");
    // Every alphabetic pair sorts after `00`, so the loop is probed later.
    let (looped, _) = unused_alphabetic_fan_out_pair(&objects);
    std::os::unix::fs::symlink(&looped, &looped).expect("a fan-out link that resolves to itself");
    // `ELOOP`, whose `ErrorKind` (`FilesystemLoop`) is not yet stable to name
    // and whose number differs between Linux and Darwin; the scan's error is
    // compared with the one the filesystem gives here.
    let loop_errno = match fs::metadata(&looped) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            error.raw_os_error().expect("an error the OS reported")
        }
        other => panic!("premise: resolving the link is a loop, not an absence: {other:?}"),
    };

    assert!(
        temporary_object_files(&fixture.base).expect("the answer `00` gave stands"),
        "`00` answers before the loop is probed, and its answer is not discarded"
    );

    fs::remove_file(first.join("tmp_obj_early")).expect("unplant");
    let error = temporary_object_files(&fixture.base)
        .expect_err("a fan-out that cannot be resolved is not a fan-out that holds nothing");
    assert!(
        matches!(
            &error,
            UpstrokeError::Io { path, source }
                if path == &looped && source.raw_os_error() == Some(loop_errno)
        ),
        "the error names the looping fan-out, with the OS's own errno: {error}"
    );
}

/// A fan-out this process cannot read is an inspection failure the caller
/// sees, not an absence: `PermissionDenied` is not `NotFound` (§7).
///
/// This is the one production difference the change discloses — an
/// unreadable two-digit directory makes `verify_object` return `Io` where the
/// old scan reached `Refusal::ObjectMissing` — and the `read_dir` arm that
/// carries it was replaceable by `Ok(false)` with every test green
/// (`PR258-ERROR-ARMS-SILENCEABLE`). Evaluated on the Unix legs, where a mode
/// bit binds a non-root user.
#[cfg(unix)]
#[test]
fn a_fan_out_this_process_cannot_read_is_an_inspection_error_not_an_absence() {
    use std::os::unix::fs::PermissionsExt as _;
    let fixture = Fixture::new("temp-object-unreadable-fan-out");
    let objects = object_directory(&fixture.base).expect("object directory");
    let (fan_out, _) = unused_alphabetic_fan_out_pair(&objects);
    fs::create_dir(&fan_out).expect("the fan-out");
    fs::write(fan_out.join("tmp_obj_hidden"), b"half an object\n").expect("plant");
    let _restore = RestoreMode {
        path: fan_out.clone(),
    };
    fs::set_permissions(&fan_out, fs::Permissions::from_mode(0o000)).expect("make it unreadable");
    // The injection must bite: root, or a process holding CAP_DAC_READ_SEARCH,
    // lists through a mode of 000, and this test would then measure nothing.
    assert!(
        fs::read_dir(&fan_out).is_err(),
        "prerequisite not met: the mode bit did not bind (running as root or with \
         CAP_DAC_READ_SEARCH); this test needs an unprivileged user"
    );

    let error = temporary_object_files(&fixture.base)
        .expect_err("a fan-out that cannot be read is not a fan-out that holds nothing");
    assert!(
        matches!(
            &error,
            UpstrokeError::Io { path, source }
                if path == &fan_out && source.kind() == std::io::ErrorKind::PermissionDenied
        ),
        "the error names the fan-out it could not read: {error}"
    );
}

/// A listing that fails part-way through is an inspection failure, not the
/// end of the listing.
///
/// `fs::read_dir` yields its entries one at a time and can fail between two
/// of them; `holds_name_prefixed` reads that failure as an answer nobody has
/// rather than as "no more names" (§7). No filesystem the suite runs on fails
/// a `readdir` to order, so the listing is constructed
/// (`PR258-ERROR-ARMS-SILENCEABLE`).
#[test]
fn a_listing_that_fails_part_way_through_is_an_inspection_error_not_the_end_of_it() {
    use std::ffi::OsString;
    let name = |name: &str| -> std::io::Result<OsString> { Ok(OsString::from(name)) };
    let failure = || -> std::io::Result<OsString> {
        Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "the listing failed after its first entry",
        ))
    };

    assert!(
        matches!(
            holds_name_prefixed([name("ef0123"), name("tmp_obj_x")].into_iter(), "tmp_"),
            Ok(true)
        ),
        "a name with the prefix is found"
    );
    assert!(
        matches!(
            holds_name_prefixed([name("ef0123")].into_iter(), "tmp_"),
            Ok(false)
        ),
        "a listing without the prefix holds none"
    );
    let error = holds_name_prefixed(
        [name("ef0123"), failure(), name("tmp_obj_x")].into_iter(),
        "tmp_",
    )
    .expect_err("a failure before the name is not the end of the listing");
    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    assert!(
        matches!(
            holds_name_prefixed([name("tmp_obj_x"), failure()].into_iter(), "tmp_"),
            Ok(true)
        ),
        "a name found before the failure is an answer, and the listing stops there"
    );
}
