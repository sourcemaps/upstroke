// Allowlist placement: the funnel section of `effects/allowlist.toml`.
#![allow(clippy::disallowed_methods, clippy::disallowed_macros)]
#![forbid(clippy::disallowed_types)]

use super::*;

// -----------------------------------------------------------------------
// Intents: synced before the add, and reclaimed
// -----------------------------------------------------------------------

/// **The hook-order assertion this test used to end with is gone,
/// deliberately.** It read `HookHarness::coverage()` — a *first-observation*
/// log — for the index of `Worktree.WriteIntent/After` and of
/// `Worktree.Add/Before`, and asserted the first came earlier. `write_intent` and `add_worktree` are two
/// separate calls this test makes in that order, so the log records them in
/// that order whatever either funnel does, and no edit to either could have
/// moved it: the assertion restated the test's own script. It is the
/// `PR5-WORKSPACE-022` shape, recorded on the snapshot test one screen below,
/// where a *fresh* harness fixed it because there both events happen inside
/// one `add_snapshot` call. Here nothing does, so the claim lives where it is
/// enforced rather than merely performed:
/// `an_add_without_a_durable_intent_refuses_and_leaves_nothing_registered` for
/// the ordering, and `the_intent_and_its_directory_are_synced_before_the_add_begins`
/// for what "durable" means at the moment the add begins. What stays here is
/// what this fixture can still witness: the intent is a durable file while no
/// worktree exists — the interrupted-add prefix the name is about — and
/// reclaim removes it.
#[test]
fn the_intent_is_durable_before_the_add_and_reclaim_removes_it() {
    let fixture = Fixture::created("intent-order");
    let slot = fixture.task("alpha", 1);

    fixture
        .manager
        .write_intent(&mut NoHooks, &slot)
        .expect("intent");
    assert!(
        fixture.manager.intent_path(&slot).is_file(),
        "the intent is durable before the add is issued"
    );
    assert!(
        !fixture.manager.slot_path(&slot).exists(),
        "and the worktree does not exist yet — this is the interrupted-add prefix"
    );

    // The cancellation clause, exactly: "an interrupted worktree or
    // snapshot add leaves a durable intent that reclaim removes."
    let reclaimed = fixture
        .manager
        .reclaim_intents(&mut NoHooks)
        .expect("reclaim");
    assert_eq!(reclaimed.slots, vec![slot.clone()]);
    assert!(!fixture.manager.intent_path(&slot).exists());
    assert!(fixture.manager.intents().expect("intents").is_empty());
}

/// A refusal at `Before(Worktree.Add)` refuses **before any effect**
/// (`PR5-CONF-003`).
///
/// `identity` says "the funnel itself calls hook(Before, site) -> primitive
/// -> hook(After, site)" and `scope` requires "every effect through typed
/// funnel APIs taking a site". `add_worktree`'s scaffolding
/// `fs::create_dir_all` sat *outside* the `funnel(...)` call, so the Before
/// hook was not the first thing that happened: the directory was already on
/// disk when the refusal was returned. The module doc at the top of this file
/// claims "every effect is issued inside a `funnel` call", and
/// `effects/wrappers.toml` classified `add_worktree` as `effect_free` —
/// which that file defines as "reaches no effect of its own".
///
/// The two axes this crosses are the *hook answer* and the *filesystem*. The
/// sibling below holds the hook answer at `Proceed` and reads the durability
/// ledger; every other add test proceeds too. What varies here is the
/// answer — `Injection::Error` at the add's Before — and what is held
/// constant is the state the effect would change: the scaffolding directory
/// is asserted absent before the call, so its absence afterwards is the
/// claim rather than an accident of the fixture.
#[test]
fn a_refusal_at_the_adds_before_hook_leaves_the_filesystem_untouched() {
    /// Refuses at the add's `Before`, and records that it did.
    #[derive(Default)]
    struct RefuseAtAddBefore {
        refused: bool,
    }

    impl EffectHooks for RefuseAtAddBefore {
        fn phase(&mut self, site: EffectSiteId, phase: HookPhase) -> Injection {
            if site == EffectSiteId::Worktree(WorktreeSite::Add) && phase == HookPhase::Before {
                self.refused = true;
                return Injection::Error;
            }
            Injection::Proceed
        }

        fn refusal_cause(&self) -> Option<String> {
            None
        }
    }

    let fixture = Fixture::created("add-before-refusal");
    let slot = fixture.task("alpha", 1);
    fixture
        .manager
        .write_intent(&mut NoHooks, &slot)
        .expect("the intent must be durable, or the add refuses for another reason");

    // The directory the effect would create. `slot_path` is private to the
    // manager, so it is derived the way the funnel derives it and then
    // asserted absent — a fixture that already had it would pass this test
    // for a funnel that created it far too early.
    let target = fixture.manager.execution_root().join(slot.relative());
    let scaffolding = target
        .parent()
        .expect("the slot target has a parent")
        .to_path_buf();
    let _ = fs::remove_dir_all(&scaffolding);
    assert!(
        !scaffolding.exists(),
        "the premise: the scaffolding directory must be absent before the call"
    );

    let mut hooks = RefuseAtAddBefore::default();
    let refusal = fixture
        .manager
        .add_worktree(&mut hooks, &slot, &fixture.head)
        .expect_err("the armed Before hook must refuse the add");
    assert!(
        hooks.refused,
        "the hook never fired, so nothing here is measured"
    );
    assert!(
        refusal.to_string().contains("before"),
        "the refusal must name the phase it came from: {refusal}"
    );
    assert!(
        !scaffolding.exists(),
        "the add's Before hook refused and {} exists anyway: an effect ran \
             before the funnel's first hook",
        scaffolding.display()
    );
    assert!(
        !target.exists(),
        "the worktree itself must not exist either: {}",
        target.display()
    );
}

/// The intent is **synced** — file and containing directory — before the
/// add's first hook (`PR5-WORKSPACE-015`, `PR5-WORKSPACE-016`).
///
/// `invariants_introduced[1]` is "worktree and snapshot intents **synced**
/// before add", and the test above checks that the intent *exists and
/// parses* before `Worktree.Add` fires. Those are different claims, and an
/// unsynced file satisfies the weaker one exactly as well as a synced one:
/// with both `sync_all` calls deleted from `write_synced`, every assertion
/// in this file still passed. The observer below crosses the two axes the
/// lane had separately — the hook order, and the durability ledger — by
/// reading the ledger *at* the add's `Before` hook rather than afterwards.
#[test]
fn the_intent_and_its_directory_are_synced_before_the_add_begins() {
    /// Snapshots the durability ledger at the first `Worktree.Add` Before.
    struct LedgerAtAdd {
        inner: HarnessEffects,
        at_add: Option<Vec<crate::util::DurableRecord>>,
    }

    impl EffectHooks for LedgerAtAdd {
        fn phase(&mut self, site: EffectSiteId, phase: HookPhase) -> Injection {
            if site == EffectSiteId::Worktree(WorktreeSite::Add)
                && phase == HookPhase::Before
                && self.at_add.is_none()
            {
                self.at_add = Some(self.inner.ledger().records());
            }
            self.inner.phase(site, phase)
        }

        fn durability_ledger(&self) -> DurabilityLedger {
            self.inner.durability_ledger()
        }

        fn refusal_cause(&self) -> Option<String> {
            self.inner.refusal_cause()
        }
    }

    let fixture = Fixture::created("intent-durability");
    let slot = fixture.task("alpha", 1);
    let intent = fixture.manager.intent_path(&slot);
    let intents_dir = intent
        .parent()
        .expect("the intents directory")
        .to_path_buf();
    let mut hooks = LedgerAtAdd {
        inner: HarnessEffects::new(Arc::new(Mutex::new(HookHarness::new()))).recording_durability(),
        at_add: None,
    };

    fixture
        .manager
        .write_intent(&mut hooks, &slot)
        .expect("intent");
    fixture
        .manager
        .add_worktree(&mut hooks, &slot, &fixture.head)
        .expect("add");

    let at_add = hooks
        .at_add
        .expect("the add's Before hook never fired, so nothing here is measured");
    let steps: Vec<DurableStep> = at_add.iter().map(|record| record.step).collect();
    // The staged file, its rename, and the directory entry: both halves of
    // `write_synced`'s durability contract, on every platform
    // (`PR5-CONF-013`). This used to fork on `cfg!(unix)` because a
    // directory fsync was held not to be a call this crate could make on
    // Windows; `util::fsync_dir` makes it.
    let expected = vec![
        DurableStep::SyncedFile,
        DurableStep::Renamed,
        DurableStep::SyncedDirectory,
    ];
    assert_eq!(
        steps, expected,
        "the durability sequence complete at the moment the add begins: {at_add:?}"
    );
    // The staged name is unique per call and never the published one; what
    // is pinned is where it lived and that it is gone once published.
    let staged = &at_add[0].path;
    assert!(
        staged.parent() == Some(intents_dir.as_path())
            && staged != &intent
            && staged
                .extension()
                .is_some_and(|extension| extension == "tmp")
            && !staged.exists(),
        "the sync is of the staged intent, in the intents directory, under a name that is \
         not the published one and is gone once published: {}",
        staged.display()
    );
    assert_eq!(
        at_add[0].len,
        fs::metadata(&intent).expect("the intent").len(),
        "the whole intent file was synced, not a prefix of it"
    );
    assert!(at_add[0].len > 0, "the intent has bytes at all");
    assert_eq!(
        at_add[1].path, intent,
        "the rename lands on the intent name"
    );
    // **On every platform, like the step list above.** `write_synced` ends in
    // `sync_directory(parent, ledger)` and `sync_directory` records the path
    // it was handed, so there is nothing platform-shaped left in what is
    // recorded — the `cfg` here was the last residue of the fork the step
    // list already lost (`PR5-CONF-013`), and it left the Windows leg unable
    // to see a barrier taken against the wrong path. It also needed a
    // `let _ = &intents_dir;` to keep the binding used, which is a §7 discard
    // standing in for a missing assertion.
    assert_eq!(
        at_add[2].path, intents_dir,
        "the directory sync is of the directory the rename changed"
    );
}

/// `snapshots`: "an interrupted add leaves a registered-but-unpopulated
/// worktree that the intent-based reclaim removes and prunes".
///
/// The residue is constructed the way `git worktree add` leaves it — the
/// registration plus the `initializing` lock Git itself holds for the whole
/// of the add — because measured, `git worktree prune` **skips** a locked
/// entry and `git worktree remove --force` refuses one. A reclaim that did
/// not clear the lock would leave exactly the residue `cleanup` promises
/// never blocks it.
#[test]
fn reclaim_removes_a_registered_but_unpopulated_worktree() {
    let fixture = Fixture::created("unpopulated");
    let slot = fixture.task("alpha", 1);
    fixture
        .manager
        .write_intent(&mut NoHooks, &slot)
        .expect("intent");
    let path = fixture.manager.slot_path(&slot);
    register_unpopulated(&fixture, &path);

    assert!(
        fixture
            .manager
            .worktree_records()
            .expect("records")
            .iter()
            .any(WorktreeRecord::is_initializing),
        "the fixture must really build the residue it is about"
    );
    assert_eq!(
        fixture
            .manager
            .quiescence(&path, &Quiescence::AtBase(fixture.head.clone()))
            .expect("verify"),
        Err(VerifyFailure::Unpopulated)
    );

    fixture
        .manager
        .reclaim_intents(&mut NoHooks)
        .expect("reclaim");
    assert!(
        !fixture
            .manager
            .worktree_records()
            .expect("records")
            .iter()
            .any(|record| record.path().ends_with("kalpha-g1")),
        "the registration is pruned"
    );
    assert!(!path.exists());
    assert!(!fixture.manager.intent_path(&slot).exists());
}

/// A non-canonical intent name beside the canonical one is refused before
/// anything is removed (`PR118-RECLAIM-REGRESSION-PINNED-AT-PARSER-ONLY`).
///
/// `str::parse` reads `03` as 3, so before `Slot::from_intent_name` compared
/// the parsed slot with its own rendering, `tasks.kalpha-g03.intent` produced
/// the slot whose intent file is `tasks.kalpha-g3.intent`: reclaim removed the
/// legitimate worktree and the canonical intent and left the `g03` file for
/// every later start to enumerate again. The parser's own test pins the
/// verdict; this one pins the composition through `intents()` and
/// `reclaim_intents()`: the walk refuses the file it cannot account for, the
/// refusal names the file, and it comes before any removal.
#[test]
fn reclaim_refuses_a_non_canonical_intent_name_before_removing_anything() {
    let fixture = Fixture::created("non-canonical-intent");
    let slot = fixture.add_task(&mut NoHooks, "alpha", 3);
    let worktree = fixture.manager.slot_path(&slot);
    let canonical = fixture.manager.intent_path(&slot);
    assert!(
        worktree.is_dir() && canonical.is_file(),
        "the legitimate slot exists before the stray file is planted"
    );
    let bytes = fs::read(&canonical).expect("read the canonical intent");
    let stray = canonical.with_file_name("tasks.kalpha-g03.intent");
    fs::write(&stray, &bytes).expect("plant the non-canonical intent");

    let error = fixture
        .manager
        .reclaim_intents(&mut NoHooks)
        .expect_err("the walk refuses a file no slot renders");
    assert!(
        matches!(
            &error,
            UpstrokeError::Git { message }
                if message.contains("unexpected file `tasks.kalpha-g03.intent`")
        ),
        "refused for the wrong reason: {error}"
    );
    assert!(worktree.is_dir(), "the legitimate worktree is untouched");
    assert_eq!(
        fs::read(&canonical).expect("read the canonical intent back"),
        bytes,
        "the canonical intent is untouched"
    );
    assert!(
        stray.is_file(),
        "the stray file is left for the operator: nothing is deleted on inference"
    );
}

#[test]
fn reclaim_removes_the_registration_whose_commondir_is_empty() {
    let fixture = Fixture::created("empty-commondir");
    let slot = fixture.task("alpha", 1);
    fixture
        .manager
        .write_intent(&mut NoHooks, &slot)
        .expect("intent");
    let path = fixture.manager.slot_path(&slot);
    register_unpopulated(&fixture, &path);
    let admin = fixture
        .manager
        .revalidate_removal(&path)
        .expect("admin dir")
        .expect("registered");
    fs::write(admin.join("commondir"), []).expect("truncate commondir");

    fixture
        .manager
        .reclaim_intents(&mut NoHooks)
        .expect("the exact registration is recoverable without Git enumeration");
    assert!(!path.exists(), "the unpopulated checkout stays absent");
    assert!(!admin.exists(), "the proved registration is removed");
    assert!(!fixture.manager.intent_path(&slot).exists());
    fixture
        .manager
        .worktree_records()
        .expect("Git enumeration works again");
}

/// `PR5-RD-002`'s fixture V8: two intents, and the registration of the one
/// that sorts **second** is the one a killed `git worktree add` leaves —
/// `locked` holding `initializing` and a zero-length `commondir`.
struct TornBehindAnother {
    alpha: Slot,
    bravo: Slot,
    alpha_path: PathBuf,
    bravo_path: PathBuf,
    alpha_admin: PathBuf,
    bravo_admin: PathBuf,
}

impl TornBehindAnother {
    /// Build it in `fixture`, asserting the premise rather than assuming it:
    /// the torn slot sorts behind an intent a reclaim reaches first — with the
    /// torn slot first, reclaiming slot by slot converged as well — and Git's
    /// enumeration dies on it.
    fn build(fixture: &Fixture) -> Self {
        let alpha = fixture.add_task(&mut NoHooks, "alpha", 1);
        let bravo = fixture.add_task(&mut NoHooks, "bravo", 1);
        let alpha_path = fixture.manager.slot_path(&alpha);
        let bravo_path = fixture.manager.slot_path(&bravo);
        let admin_of = |path: &Path| {
            fixture
                .manager
                .revalidate_removal(path)
                .expect("admin dir")
                .expect("registered")
        };
        let alpha_admin = admin_of(&alpha_path);
        let bravo_admin = admin_of(&bravo_path);
        fs::write(bravo_admin.join("locked"), "initializing\n")
            .expect("the lock the add holds until it finishes");
        fs::write(bravo_admin.join("commondir"), [])
            .expect("the file the add opened and never wrote");
        assert_eq!(
            fixture.manager.intents().expect("intents"),
            vec![alpha.clone(), bravo.clone()],
            "the torn slot sorts second, behind an intent the reclaim reaches first"
        );
        assert_enumeration_dies_on(fixture, &bravo_admin);
        Self {
            alpha,
            bravo,
            alpha_path,
            bravo_path,
            alpha_admin,
            bravo_admin,
        }
    }

    /// Both slots reclaimed: no intent, no checkout, no registration, and Git
    /// enumerates again without either.
    fn assert_reclaimed(&self, fixture: &Fixture) {
        assert!(
            fixture.manager.intents().expect("intents").is_empty(),
            "both intents are reclaimed"
        );
        assert!(
            !self.alpha_path.exists() && !self.bravo_path.exists(),
            "both checkouts are gone"
        );
        assert!(
            !self.bravo_admin.exists(),
            "the torn registration is removed as the proved registration it is"
        );
        assert!(!self.alpha_admin.exists(), "and the healthy one is pruned");
        let records = fixture
            .manager
            .worktree_records()
            .expect("Git enumerates again");
        assert!(
            records.iter().all(|record| {
                !record.path().ends_with("kalpha-g1") && !record.path().ends_with("kbravo-g1")
            }),
            "neither slot is registered: {records:?}"
        );
    }
}

/// **One torn registration does not wedge the reclaim of the intents that
/// sort before it** (`PR5-RD-002`, fixture V8).
///
/// Every `remove_intent` revalidates through Git's enumeration, and at
/// `db523cd3` only the torn slot's own forced removal repaired what makes that
/// enumeration die. Reclaimed slot by slot, `alpha`'s intent was removed before
/// `bravo`'s worktree, so the reclaim refused there on every attempt — the
/// intents stayed `[alpha, bravo]` and `bravo` untouched — and never reached
/// the removal that would have repaired the store. The torn-registration tests
/// above build one slot each, which a reclaim reaches first in either order,
/// so none of them could see it. The intent removal's own repair
/// (`PR5-RD-002-ENGINE-RECLAIM-LOOPS`) is pinned by
/// `an_intents_removal_repairs_a_torn_registration_another_intent_names`; this
/// reclaim reaches every torn registration through its slot's own removal
/// first, as the phase order below pins.
#[test]
fn a_torn_registration_behind_another_intent_does_not_wedge_the_reclaim() {
    let fixture = Fixture::created("torn-behind-another");
    let torn = TornBehindAnother::build(&fixture);

    let reclaimed = fixture
        .manager
        .reclaim_intents(&mut NoHooks)
        .expect("one torn registration does not wedge the reclaim of the others");
    assert_eq!(
        reclaimed.slots,
        vec![torn.alpha.clone(), torn.bravo.clone()]
    );
    torn.assert_reclaimed(&fixture);
}

/// A reclaim of fixture V8 stopped at any phase of any of its funnels leaves no
/// checkout or registration without its intent, and the next reclaim converges
/// from it.
///
/// The stop is an error return at the phase, which leaves on disk what a kill
/// at that phase leaves: `Before` is consulted before the primitive runs and
/// `After` once it has returned, and nothing between the stop and the
/// reclaim's return writes. What it does not model is a power loss, which the
/// removal funnel's own barrier answers for the checkout. The phase log of the
/// reclaim that ran to the end pins the order that reaches the torn
/// registration before any enumeration: every worktree's removal, then every
/// intent's.
#[test]
fn a_torn_reclaim_stopped_at_any_phase_converges_on_the_next() {
    struct StopAt {
        stop: usize,
        seen: Vec<(EffectSiteId, HookPhase)>,
    }

    impl EffectHooks for StopAt {
        fn phase(&mut self, site: EffectSiteId, phase: HookPhase) -> Injection {
            self.seen.push((site, phase));
            if self.seen.len() == self.stop {
                Injection::Error
            } else {
                Injection::Proceed
            }
        }

        fn refusal_cause(&self) -> Option<String> {
            None
        }
    }

    let mut stop = 0;
    let completed = loop {
        stop += 1;
        assert!(
            stop <= 32,
            "a reclaim of two intents consults a bounded number of phases"
        );
        let fixture = Fixture::created(&format!("torn-reclaim-stop-{stop}"));
        let torn = TornBehindAnother::build(&fixture);
        let mut hooks = StopAt {
            stop,
            seen: Vec::new(),
        };
        let stopped = fixture.manager.reclaim_intents(&mut hooks);
        for (slot, path, admin) in [
            (&torn.alpha, &torn.alpha_path, &torn.alpha_admin),
            (&torn.bravo, &torn.bravo_path, &torn.bravo_admin),
        ] {
            if path.exists() || admin.exists() {
                assert!(
                    fixture.manager.intent_path(slot).exists(),
                    "stopped at phase {stop}: {} outlives its intent",
                    path.display()
                );
            }
        }
        fixture
            .manager
            .reclaim_intents(&mut NoHooks)
            .expect("the next reclaim converges wherever the last one stopped");
        torn.assert_reclaimed(&fixture);
        if stopped.is_ok() {
            break hooks.seen;
        }
    };
    let remove = EffectSiteId::Worktree(WorktreeSite::Remove);
    let remove_intent = EffectSiteId::Worktree(WorktreeSite::RemoveIntent);
    assert_eq!(
        completed,
        vec![
            (remove, HookPhase::Before),
            (remove, HookPhase::After),
            (remove, HookPhase::Before),
            (remove, HookPhase::After),
            (remove_intent, HookPhase::Before),
            (remove_intent, HookPhase::After),
            (remove_intent, HookPhase::Before),
            (remove_intent, HookPhase::After),
        ],
        "every worktree is removed before any intent"
    );
}

/// **The fail-closed half, pinned so that no later simplification turns the
/// repair fail-open:** a torn registration that no intent of this execution
/// root names is never removed, skipped, scanned around or read as absent, and
/// the reclaim refuses on Git's enumeration of it — the same refusal whichever
/// order the reclaim removes worktrees and intents in.
///
/// Two shapes, each torn the way a killed `git worktree add` tears it: the
/// user's own linked worktree outside the execution root, and a checkout in
/// one of this root's slot namespaces that no intent names. The forced
/// removal repairs only an administrative directory it has bound to the slot
/// being removed, and an intent's removal only one bound to a slot another
/// intent names (`PR5-RD-002-ENGINE-RECLAIM-LOOPS`), so neither is the
/// reclaim's: both are left byte for byte, with their checkouts, and the
/// reclaim's one intent outlives the refusal. The path reaches Git as the
/// bytes it is, so a scratch root whose name no UTF-8 spells tears the
/// registration of the checkout the test names and not of another.
#[test]
fn a_torn_registration_no_intent_names_still_refuses_the_reclaim() {
    for shape in ["foreign", "unintended"] {
        let fixture = Fixture::created(&format!("torn-unproved-{shape}"));
        let alpha = fixture.add_task(&mut NoHooks, "alpha", 1);
        let torn = if shape == "foreign" {
            fixture.root.join("user-worktree")
        } else {
            fixture.manager.slot_path(&fixture.task("charlie", 1))
        };
        git_os(
            &fixture.base,
            &[
                OsStr::new("worktree"),
                OsStr::new("add"),
                OsStr::new("-q"),
                OsStr::new("--detach"),
                torn.as_os_str(),
                OsStr::new(&fixture.head),
            ],
        );
        let admin = tear_registration(&fixture.manager, &torn);
        assert_enumeration_dies_on(&fixture, &admin);
        let admin_before = tree_bytes(&admin);
        let checkout_before = tree_bytes(&torn);

        let message = refusal_of(
            &fixture
                .manager
                .reclaim_intents(&mut NoHooks)
                .expect_err("a torn registration no intent names refuses the reclaim"),
        );
        assert!(
            message.contains("commondir"),
            "{shape}: the refusal is Git's enumeration dying on the torn registration: {message}"
        );
        assert_eq!(
            tree_bytes(&admin),
            admin_before,
            "{shape}: the torn registration is untouched"
        );
        assert_eq!(
            tree_bytes(&torn),
            checkout_before,
            "{shape}: and so is its checkout"
        );
        assert!(
            fixture.manager.intent_path(&alpha).exists(),
            "{shape}: the refusal comes before the reclaim removes its intent"
        );
        assert_enumeration_dies_on(&fixture, &admin);
    }
}

/// **An intent's removal repairs a torn registration another intent names**
/// (`PR5-RD-002-ENGINE-RECLAIM-LOOPS`): fixture V8 removed slot by slot, the
/// order every reclaim loop of the engine takes — `alpha`'s worktree, then
/// `alpha`'s intent, then `bravo`'s pair.
///
/// At `dfab458b` `alpha`'s intent removal refused on Git's enumeration dying
/// on `bravo`'s `commondir`, on every attempt, and nothing reached `bravo`'s
/// forced removal. Now it runs that forced removal itself and asks Git again:
/// `bravo`'s checkout and registration go, and `bravo`'s intent is left for
/// `bravo`'s own step, whose forced removal then finds nothing to remove and
/// converges — the state the repair leaves, taken up.
#[test]
fn an_intents_removal_repairs_a_torn_registration_another_intent_names() {
    let fixture = Fixture::created("torn-repaired-by-intent-removal");
    let torn = TornBehindAnother::build(&fixture);

    fixture
        .manager
        .remove_worktree(&mut NoHooks, &torn.alpha)
        .expect("alpha's worktree");
    fixture
        .manager
        .remove_intent(&mut NoHooks, &torn.alpha)
        .expect("alpha's intent goes although bravo's registration is torn");
    assert!(
        !torn.bravo_admin.exists() && !torn.bravo_path.exists(),
        "bravo's torn registration went, through bravo's forced removal, with its checkout"
    );
    assert!(
        fixture.manager.intent_path(&torn.bravo).exists(),
        "and bravo's intent stays for bravo's own step"
    );
    assert!(!fixture.manager.intent_path(&torn.alpha).exists());
    let records = fixture
        .manager
        .worktree_records()
        .expect("Git enumerates again");
    assert!(
        records
            .iter()
            .all(|record| !record.path().ends_with("kbravo-g1")),
        "bravo is no longer registered: {records:?}"
    );

    fixture
        .manager
        .remove_worktree(&mut NoHooks, &torn.bravo)
        .expect("bravo's forced removal converges with nothing left to remove");
    fixture
        .manager
        .remove_intent(&mut NoHooks, &torn.bravo)
        .expect("and bravo's intent goes");
    torn.assert_reclaimed(&fixture);
}

/// **The repair never takes the registration of the slot whose intent is
/// being removed.** That intent goes next, so a checkout whose registration
/// went with it would be residue no reclaim finds: nothing names it any more.
/// The slot's own torn registration keeps the enumeration's refusal, the
/// checkout keeps the intent that names it, and the slot's forced removal is
/// what repairs it. It holds at `dfab458b` too, where the intent removal
/// repaired nothing.
#[test]
fn an_intents_removal_leaves_its_own_slots_torn_registration_to_its_removal() {
    let fixture = Fixture::created("torn-own-slot");
    let torn = TornBehindAnother::build(&fixture);
    let admin_before = tree_bytes(&torn.bravo_admin);
    let checkout_before = tree_bytes(&torn.bravo_path);

    let message = refusal_of(
        &fixture
            .manager
            .remove_intent(&mut NoHooks, &torn.bravo)
            .expect_err("bravo's own torn registration keeps the enumeration's refusal"),
    );
    assert!(
        message.contains("commondir"),
        "the refusal is Git's enumeration dying on bravo's registration: {message}"
    );
    assert_eq!(
        tree_bytes(&torn.bravo_admin),
        admin_before,
        "bravo's registration is untouched"
    );
    assert_eq!(
        tree_bytes(&torn.bravo_path),
        checkout_before,
        "and so is its checkout"
    );
    assert!(
        fixture.manager.intent_path(&torn.bravo).exists(),
        "and the intent that names the checkout stays"
    );
    assert_enumeration_dies_on(&fixture, &torn.bravo_admin);

    fixture
        .manager
        .reclaim_intents(&mut NoHooks)
        .expect("the reclaim converges from it");
    torn.assert_reclaimed(&fixture);
}

/// **Verifying a slot whose own registration is torn repairs it, and the
/// slot reads as unregistered** (`PR5-RD-002-ENGINE-RECLAIM-LOOPS`). A resume
/// verifies an open generation's worktree before it reuses or recreates it,
/// and the add a killed conductor tore may be that generation's own.
/// At `dfab458b` the verification's revalidation refused on Git's enumeration
/// dying on it, on every attempt. Now the slot's forced removal runs first —
/// its checkout and registration go, its intent stays — the verification
/// reports [`VerifyFailure::NotRegistered`], and the forced removal and fresh
/// add its caller then makes converge on a worktree that verifies.
#[test]
fn verifying_a_slot_whose_own_registration_is_torn_repairs_it_and_reports_it_unregistered() {
    let fixture = Fixture::created("torn-verified");
    let alpha = fixture.add_task(&mut NoHooks, "alpha", 1);
    let path = fixture.manager.slot_path(&alpha);
    let admin = tear_registration(&fixture.manager, &path);
    assert_enumeration_dies_on(&fixture, &admin);
    let at_base = Quiescence::AtBase(fixture.head.clone());

    let verified = fixture
        .manager
        .verify_worktree(&mut NoHooks, &alpha, &at_base)
        .expect("the verification repairs the torn registration rather than refusing");
    assert_eq!(verified, Err(VerifyFailure::NotRegistered));
    assert!(
        !path.exists() && !admin.exists(),
        "the slot's forced removal took its checkout and its registration"
    );
    assert!(
        fixture.manager.intent_path(&alpha).exists(),
        "and left the intent that names the slot"
    );
    fixture
        .manager
        .worktree_records()
        .expect("Git enumerates again");

    fixture
        .manager
        .remove_worktree(&mut NoHooks, &alpha)
        .expect("the caller's forced removal finds nothing to remove");
    fixture
        .manager
        .add_worktree(&mut NoHooks, &alpha, &fixture.head)
        .expect("and its fresh add succeeds");
    assert_eq!(
        fixture
            .manager
            .verify_worktree(&mut NoHooks, &alpha, &at_base)
            .expect("verify"),
        Ok(()),
        "the recreated worktree verifies"
    );
}

/// **A repair the removal's gate refuses to plan removes nothing and keeps
/// the refusal.** The repair binds every other intent's registration under
/// `WriterProof::Unknown`, whatever the caller holds, because an intent's
/// removal is not told what the caller can prove about writers; that gate
/// refuses a registration whose `gitdir` is empty beside a `locked`, the
/// state an add killed inside its first two writes leaves (on disk, an add in
/// flight). Here the user's own worktree outside the root is in that state,
/// beside fixture V8. `alpha`'s forced removal passes it over under
/// `WriterProof::NoWriterAlive`, as finalization's does; `alpha`'s intent
/// removal then finds Git's enumeration dying on `bravo`, cannot plan the
/// repair, removes nothing, and returns the enumeration's own refusal — the
/// best-effort repair's failure path, observed.
#[test]
fn a_repair_the_removal_gate_cannot_plan_removes_nothing_and_keeps_the_refusal() {
    let fixture = Fixture::created("torn-unplannable");
    let foreign = fixture.root.join("user-worktree");
    git_os(
        &fixture.base,
        &[
            OsStr::new("worktree"),
            OsStr::new("add"),
            OsStr::new("-q"),
            OsStr::new("--detach"),
            foreign.as_os_str(),
            OsStr::new(&fixture.head),
        ],
    );
    let unbindable = fixture
        .manager
        .revalidate_removal(&foreign)
        .expect("admin dir")
        .expect("registered");
    let torn = TornBehindAnother::build(&fixture);
    fs::write(unbindable.join("locked"), "initializing\n")
        .expect("the lock the add holds until it finishes");
    fs::write(unbindable.join("gitdir"), []).expect("the gitdir the add opened and never wrote");
    let bravo_before = tree_bytes(&torn.bravo_admin);

    let passed_over = fixture
        .manager
        .remove_worktree_proving(&mut NoHooks, &torn.alpha, WriterProof::NoWriterAlive)
        .expect("alpha's worktree, the unbindable registration passed over");
    assert_eq!(passed_over, vec![unbindable.clone()]);
    let message = refusal_of(
        &fixture
            .manager
            .remove_intent(&mut NoHooks, &torn.alpha)
            .expect_err("a repair that cannot be planned keeps the enumeration's refusal"),
    );
    assert!(
        message.contains("kbravo-g1") && message.contains("commondir"),
        "the refusal is Git's enumeration dying on bravo's registration: {message}"
    );
    assert_eq!(
        tree_bytes(&torn.bravo_admin),
        bravo_before,
        "bravo's torn registration is untouched"
    );
    assert!(
        unbindable.join("locked").exists(),
        "and so is the unbindable one"
    );
    assert!(
        fixture.manager.intent_path(&torn.alpha).exists(),
        "and alpha's intent stays"
    );
}

/// Two intended slots: `bravo`, whose registration is torn the way a killed
/// `git worktree add` leaves it, and `charlie`, whose registration has lost
/// its `gitdir` — no `locked`, its checkout and its intent kept — so that no
/// registration binds `charlie`'s checkout and a repair's plan passes it over.
/// Git's enumeration skips that entry and `git worktree prune` deletes it.
/// `charlie`'s forced removal converges from it while `<common git
/// dir>/worktrees` exists
/// (`an_absent_registration_gitdir_is_already_gone_for_forced_cleanup`), and,
/// since closure 3 of the external-prune follow-up, once that directory is
/// absent too, its checkout's `.git` naming an entry of the absent store
/// (`a_missing_stored_worktree_directory_converges_removing_only_the_checkout`;
/// until then it refused there).
struct TornBesideUnbound {
    bravo: Slot,
    charlie: Slot,
    bravo_path: PathBuf,
    charlie_path: PathBuf,
    bravo_admin: PathBuf,
}

impl TornBesideUnbound {
    fn build(fixture: &Fixture) -> Self {
        let bravo = fixture.add_task(&mut NoHooks, "bravo", 1);
        let charlie = fixture.add_task(&mut NoHooks, "charlie", 1);
        let bravo_path = fixture.manager.slot_path(&bravo);
        let charlie_path = fixture.manager.slot_path(&charlie);
        let charlie_admin = fixture
            .manager
            .revalidate_removal(&charlie_path)
            .expect("admin dir")
            .expect("registered");
        fs::remove_file(charlie_admin.join("gitdir"))
            .expect("charlie's registration loses its gitdir");
        assert_eq!(
            fixture
                .manager
                .revalidate_removal(&charlie_path)
                .expect("the gate reads the store"),
            None,
            "no registration binds charlie's checkout"
        );
        let bravo_admin = tear_registration(&fixture.manager, &bravo_path);
        assert_enumeration_dies_on(fixture, &bravo_admin);
        Self {
            bravo,
            charlie,
            bravo_path,
            charlie_path,
            bravo_admin,
        }
    }

    /// `charlie`'s forced removal and its retry both converge, and its
    /// checkout is gone.
    fn assert_charlie_reclaimable(&self, fixture: &Fixture) {
        for attempt in ["charlie's forced removal", "and its retry"] {
            fixture
                .manager
                .remove_worktree(&mut NoHooks, &self.charlie)
                .unwrap_or_else(|error| panic!("{attempt} converges: {error}"));
        }
        assert!(
            !self.charlie_path.exists(),
            "charlie's checkout is reclaimed"
        );
    }
}

/// **An intent's removal leaves every other intended checkout reclaimable**
/// (the round-2 regression lens of #308, P2, executed). `alpha`'s worktree has
/// gone, and its intent's removal repairs `bravo`'s torn registration and
/// passes over `charlie`, which no registration binds. At `db67a82b` that
/// repair, `bravo`'s forced removal, ended in `git worktree prune`, which
/// deleted `charlie`'s registration and then the emptied `<common git
/// dir>/worktrees`, and `charlie`'s forced removal refused its checkout on
/// every attempt. Now the empty-`commondir` branch stops at the registration
/// it removed, so the prune that deletes `charlie`'s registration is
/// `charlie`'s own, after its checkout has gone.
#[test]
fn an_intents_removal_repair_leaves_another_intended_checkout_reclaimable() {
    let fixture = Fixture::created("torn-repair-keeps-store");
    let alpha = fixture.add_task(&mut NoHooks, "alpha", 1);
    fixture
        .manager
        .remove_worktree(&mut NoHooks, &alpha)
        .expect("alpha's worktree");
    let slots = TornBesideUnbound::build(&fixture);

    let repaired = fixture.manager.remove_intent(&mut NoHooks, &alpha);
    slots.assert_charlie_reclaimable(&fixture);
    repaired.expect("alpha's intent goes although bravo's registration is torn");
    assert!(
        !slots.bravo_admin.exists() && !slots.bravo_path.exists(),
        "the repair took bravo's checkout and registration"
    );
    fixture
        .manager
        .reclaim_intents(&mut NoHooks)
        .expect("bravo's and charlie's intents go");
    assert!(fixture.manager.intents().expect("intents").is_empty());
    fixture
        .manager
        .worktree_records()
        .expect("Git enumerates again");
}

/// **A torn registration's own forced removal leaves every other intended
/// checkout reclaimable**: the same wedge without the repair, through the
/// empty-`commondir` branch, which at `db523cd3` also ended in `git worktree
/// prune`. There `bravo`'s removal deleted its torn registration, and the
/// prune then deleted `charlie`'s and the emptied store, so `charlie`'s
/// removal refused its checkout on every attempt.
#[test]
fn a_torn_registrations_removal_leaves_another_intended_checkout_reclaimable() {
    let fixture = Fixture::created("torn-removal-keeps-store");
    let slots = TornBesideUnbound::build(&fixture);

    fixture
        .manager
        .remove_worktree(&mut NoHooks, &slots.bravo)
        .expect("bravo's forced removal repairs its torn registration");
    assert!(
        !slots.bravo_admin.exists() && !slots.bravo_path.exists(),
        "bravo's checkout and registration are gone"
    );
    slots.assert_charlie_reclaimable(&fixture);
    fixture
        .manager
        .reclaim_intents(&mut NoHooks)
        .expect("bravo's and charlie's intents go");
    assert!(fixture.manager.intents().expect("intents").is_empty());
}

/// `target` relative to `from`, both canonicalised: `..` up to the common
/// ancestor, then down. What Git 2.48's `worktree.useRelativePaths` writes.
fn relative_from(from: &Path, target: &Path) -> PathBuf {
    let from = fs::canonicalize(from).expect("from exists");
    let target = fs::canonicalize(target).expect("target exists");
    let mut from = from.components().peekable();
    let mut target = target.components().peekable();
    while from.peek().is_some() && from.peek() == target.peek() {
        from.next();
        target.next();
    }
    let mut relative = PathBuf::new();
    for _ in from {
        relative.push("..");
    }
    for component in target {
        relative.push(component);
    }
    relative
}

/// A relative registration, Git 2.48's `worktree.useRelativePaths` form, is
/// joined to its registration directory and canonicalised, so the slot's own
/// registration rewritten relative still binds the slot; one that resolves to a
/// foreign directory inside the root refuses by containment, as an absolute
/// one would.
#[test]
fn a_relative_registration_still_binds_its_checkout() {
    let fixture = Fixture::created("relative-registration");
    let slot = fixture.add_task(&mut NoHooks, "alpha", 1);
    let path = fixture.manager.slot_path(&slot);
    let admin = fixture
        .manager
        .revalidate_removal(&path)
        .expect("admin dir")
        .expect("registered");
    let relative = relative_from(&admin, &path.join(".git"));
    assert!(
        relative.starts_with(".."),
        "the fixture wrote a relative path"
    );
    fs::write(admin.join("gitdir"), format!("{}\n", relative.display())).expect("relative gitdir");
    let bound = fixture
        .manager
        .revalidate_removal(&path)
        .expect("a relative registration resolves")
        .expect("and still binds the slot");
    assert_eq!(bound, admin);

    let foreign = fixture.manager.execution_root().join("foreign");
    fs::create_dir_all(&foreign).expect("a foreign directory inside the root");
    let relative = relative_from(&admin, &foreign);
    fs::write(
        admin.join("gitdir"),
        format!("{}/.git\n", relative.display()),
    )
    .expect("relative gitdir naming the foreign directory");
    fixture
        .manager
        .revalidate_removal(&path)
        .expect_err("a registration resolving to a foreign directory inside the root refuses");

    // A relative registration whose checkout does not exist and which climbs
    // above the filesystem root refuses by name at the decoder, where before
    // the join was handed to canonicalisation with its `..` intact and the
    // absent checkout was read as "not this registration".
    let depth = fs::canonicalize(&admin)
        .expect("admin exists")
        .components()
        .filter(|component| matches!(component, std::path::Component::Normal(_)))
        .count();
    let mut climb = PathBuf::new();
    for _ in 0..=depth {
        climb.push("..");
    }
    climb.push("absent");
    climb.push(".git");
    fs::write(admin.join("gitdir"), format!("{}\n", climb.display())).expect("climbing gitdir");
    let refused = fixture
        .manager
        .revalidate_removal(&path)
        .expect_err("a relative registration that climbs above the root refuses by name");
    assert!(
        refused
            .to_string()
            .contains("climbs above the filesystem root"),
        "{refused}"
    );
}

/// The registration `git worktree add` leaves when it is killed after it has
/// written `gitdir` but before `commondir` or `HEAD` holds its bytes — the
/// files it writes one at a time, each opened and truncated before it is
/// written, so a kill inside either leaves it empty (measured by PR10's ST-07
/// sampler, `~/pr10-evidence/r3/sampler-prefix-instrumented.log`, runs 20 and
/// 22 of thirty). Git's own enumeration cannot report it: an empty
/// `commondir` makes `git worktree list` die before any record, and an empty
/// `HEAD` prints a record with neither a branch nor a detached mark, which the
/// parser refuses (`PR172-SAMPLER-REFUSED-A-TORN-WORKTREE-LIST-RECORD`). So a
/// classifier that asked Git answered an error where the frozen enums
/// register a class (`SWEEP-WORKTREE-012`). It reads the registration itself,
/// byte-safe, as the removal binds it, and answers the class the state is:
/// registered, locked by the add that never finished, unpopulated. Forced
/// removal converges on both, and Git enumerates again afterwards.
#[test]
fn a_registration_git_cannot_enumerate_classifies_as_unpopulated_and_converges() {
    for torn in ["commondir", "HEAD"] {
        let fixture = Fixture::created(&format!("torn-{}", torn.to_lowercase()));
        let slot = fixture.add_task(&mut NoHooks, "alpha", 1);
        let path = fixture.manager.slot_path(&slot);
        let admin = fixture
            .manager
            .revalidate_removal(&path)
            .expect("admin dir")
            .expect("registered");
        fs::write(admin.join("locked"), "initializing\n")
            .expect("the lock the add holds until it finishes");
        fs::write(admin.join(torn), []).expect("the file the add opened and never wrote");
        fixture.manager.worktree_records().expect_err(&format!(
            "{torn}: Git's enumeration cannot report this registration"
        ));

        let site = EffectSiteId::Worktree(WorktreeSite::Add);
        let target = ResidueTarget::new(&fixture.base).at(&path);
        assert_eq!(
            classify(site, &target),
            ObjectResidue::Internal,
            "{torn}: registered, locked by the add that never finished, unpopulated"
        );
        assert_eq!(
            observed_residue_elements(site, &target).expect("observe"),
            vec![ResidueElement::RegisteredUnpopulatedWorktree],
            "{torn}"
        );

        fixture
            .manager
            .remove_worktree(&mut NoHooks, &slot)
            .expect("forced removal converges on a registration that names its checkout");
        assert!(
            !path.exists() && !admin.exists(),
            "{torn}: the checkout and the registration are gone"
        );
        assert!(
            fixture
                .manager
                .worktree_records()
                .expect("Git enumerates again")
                .iter()
                .all(|record| !record.path().ends_with("kalpha-g1")),
            "{torn}"
        );
    }
}

/// The registration reader the classifier gained in PR10's round 3
/// (`registration_for`) enters `common_git_dir`, which read Git's answer as
/// UTF-8: in a repository whose path holds a byte no UTF-8 spells, an absent
/// add target — no registration, no checkout, the shape every first `add`
/// starts from — errored where the merge base's reader, which decoded Git's
/// NUL-delimited enumeration byte for byte, answered `ObjectResidue::None`
/// (the round-4 regression lens, P2-2; the decoder defect for a populated
/// worktree is `PR128-REVIEW2-PATHS-READ-AS-UTF8`'s and stays filed). A raw
/// repository, not `Fixture::created`: the fixture's own manager derivation
/// reaches the same reader. Unix, where such a path exists at all — and not
/// on every Unix filesystem: APFS refuses the name with `EILSEQ` ("Illegal
/// byte sequence", errno 92 on macOS; CI's `test (macos-latest)` job at
/// `96bf1944`), so the directory is probed first and the test is skipped,
/// saying why and with the errno it saw, where the name cannot be made — on
/// such a filesystem the shape the test guards against cannot arise either.
/// Linux makes the name and measures the decoder.
#[cfg(unix)]
#[test]
fn an_absent_add_target_in_a_byte_named_repository_still_classifies() {
    use std::os::unix::ffi::OsStringExt as _;
    let tree = scratch("byte-named-repository");
    let root = tree.path();
    let repo = root.join(std::ffi::OsString::from_vec(b"repo-\xff".to_vec()));
    match fs::create_dir_all(&repo) {
        Ok(()) => {}
        Err(error) if error.raw_os_error() == Some(libc::EILSEQ) => {
            eprintln!(
                "skipped: this filesystem refuses a directory name that is not UTF-8 — {error} \
                 (errno {:?}, EILSEQ) — so the byte-named repository the decoder is measured on \
                 cannot exist here, and neither can the shape this test guards against",
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
    let target = root.join("not-added");
    assert!(
        !target.exists(),
        "the add target is absent: nothing was ever added there"
    );

    assert_eq!(
        classify_object_residue(
            EffectSiteId::Worktree(WorktreeSite::Add),
            &ResidueTarget::new(&repo).at(&target),
        )
        .expect("valid Unix repository path"),
        ObjectResidue::None,
        "an absent target in a byte-named repository is unregistered, as at the merge base"
    );
}

#[test]
fn malformed_gitdir_refuses_before_removal() {
    for (case, bytes) in [("empty", &b""[..]), ("partial", &b"not-a-dot-git-path"[..])] {
        let fixture = Fixture::created(&format!("bad-gitdir-{case}"));
        let slot = fixture.add_task(&mut NoHooks, "alpha", 1);
        let path = fixture.manager.slot_path(&slot);
        let admin = fixture
            .manager
            .revalidate_removal(&path)
            .expect("admin dir")
            .expect("registered");
        fs::write(admin.join("gitdir"), bytes).expect("replace gitdir");

        fixture
            .manager
            .remove_worktree(&mut NoHooks, &slot)
            .expect_err("unbound registration refuses");
        assert!(path.exists(), "{case}: refusal precedes checkout deletion");
        assert!(
            admin.exists(),
            "{case}: refusal precedes registration deletion"
        );
    }
}

#[cfg(unix)]
#[test]
fn tree_bytes_detects_a_file_replaced_by_a_link_to_equal_contents() {
    let scratch = acquire(&std::env::temp_dir(), "tree-bytes-file-link").expect("scratch tree");
    let root = scratch.path().join("observed");
    fs::create_dir(&root).expect("observed directory");
    let file = root.join("file");
    let target = scratch.path().join("same-bytes");
    fs::write(&file, b"unchanged bytes\n").expect("original file");
    fs::write(&target, b"unchanged bytes\n").expect("equal-content target");
    let before = tree_bytes(&root);

    fs::remove_file(&file).expect("remove original file");
    std::os::unix::fs::symlink(&target, &file).expect("substitute the link");
    assert_eq!(
        fs::read(&file).expect("read through the link"),
        b"unchanged bytes\n",
        "a snapshot that follows the link would see the original bytes"
    );
    assert_ne!(
        tree_bytes(&root),
        before,
        "file-to-link substitution must change the refusal snapshot"
    );
}

#[test]
fn tree_bytes_detects_a_directory_replaced_by_a_link_to_equal_contents() {
    let scratch =
        acquire(&std::env::temp_dir(), "tree-bytes-directory-link").expect("scratch tree");
    let root = scratch.path().join("observed");
    let directory = root.join("directory");
    let target = scratch.path().join("same-tree");
    fs::create_dir_all(&directory).expect("original directory");
    fs::create_dir(&target).expect("target directory");
    fs::write(directory.join("file"), b"unchanged bytes\n").expect("original contents");
    fs::write(target.join("file"), b"unchanged bytes\n").expect("equal-content target");
    let before = tree_bytes(&root);

    fs::remove_file(directory.join("file")).expect("remove original contents");
    fs::remove_dir(&directory).expect("remove original directory");
    plant_directory_link(&target, &directory);
    assert_eq!(
        fs::read(directory.join("file")).expect("read through the directory link"),
        b"unchanged bytes\n",
        "a snapshot that follows the link would see the original subtree"
    );
    assert_ne!(
        tree_bytes(&root),
        before,
        "directory-to-link substitution must change the refusal snapshot"
    );
}

/// What `git worktree add` leaves when it is killed before it writes `gitdir`,
/// and what `remove_worktree` does with it. Measured under strace on git
/// 2.43.0, the add writes in this order: the admin directory, `locked`
/// (`initializing`), the checkout directory, `gitdir`, the checkout's `.git`
/// pointer, `HEAD`, `commondir`, then the checkout through child processes; on
/// the run measured, `locked` was unlinked as the last syscall. A kill between
/// opening `gitdir` -- which creates it, zero-length -- and writing it leaves
/// exactly an admin directory holding `locked` and a zero-length `gitdir`, and
/// an empty checkout directory with no pointer.
///
/// Three facts, each asserted because each was believed otherwise once.
///
/// The classifier answers `None` for it, and that answer is a **hole**, not a
/// verdict: `None` is defined as "nothing was written", and here an intent,
/// an admin directory, `locked`, `gitdir` and a checkout directory were
/// written. The classifier reads registration through `git worktree list`,
/// which skips a registration it reads zero bytes from, so `record_for`
/// answers `None`, the element list is empty, and the state is invisible to
/// the site's residue contract even though it is durable. The sampler runs
/// the tabled recovery for every sample and requires every one to recover, so
/// a sample landing in this window is the sampler's `forced removal
/// converges` red.
///
/// `remove_worktree` **refuses** it, with a diagnostic naming this admin
/// directory and nothing else, and the residue blocks every removal through
/// this funnel in the repository, not only this slot's. That is pinned as what
/// the public `WorkspaceManager` library API does today. Since #329 the
/// removal's scan is a tolerant registry access, so on disk alone this state
/// is a write in flight and is attempted past until the access's deadline;
/// the refusal then arrives as `UpstrokeError::RegistryRefused`, resumable and
/// never Git state, carrying that diagnostic as its last failure
/// (`48f2af7a:reviews/2026-10-01-pr11-follow-up-b-record.md`, §3.6). The shipped CLI's
/// `upstroke resume` uses the separate `Workspace::reclaim_gate_workspaces`
/// path, which never decodes a registration's `gitdir`; this test does not
/// reproduce a refusal in that path. `DESIGN.md` §15's reclaim sentence is
/// about those gate/review worktrees. The engine's task-slot callers are in
/// its crate-private schema-4 topology, and `reclaim_intents` has no non-test
/// caller within this repository, but external library callers can use the
/// public manager directly.
/// The design is silent about a task registration this funnel cannot bind;
/// `RESIDUE-UNBINDABLE-TASK-REGISTRATION-HAS-NO-DESIGN-SENTENCE` records that
/// gap and the measurements a sentence needs. On disk this state is
/// indistinguishable from an add in flight in the same window -- `locked` is
/// present in both -- and a skip here was measured to delete the checkout
/// beneath a live writer's registration (PR #151 pass 1), so the refusal is
/// not to be relaxed on disk state alone. What relaxes it is a proof about
/// writers, `WriterProof::NoWriterAlive`, and the sibling
/// `a_torn_registration_is_passed_over_and_the_slot_converges_when_no_writer_is_alive`
/// holds that form to what it may and may not touch.
///
/// What each refusal is checked against is a snapshot -- relative path to
/// directory, bytes or link target -- of four trees, taken before and compared
/// whole after: the unrelated slot's populated checkout, this slot's empty
/// checkout directory, the whole `.git/worktrees/` store (the stale entry and
/// the unrelated slot's administrative directory alike), and the intents
/// directory. The same four trees at both refusals; the unrelated slot exists
/// before the first. Checking named files would let a removal that deleted
/// the rest pass, and checking the checkout without its administrative
/// directory would let a removal that deleted the unrelated slot's `index`
/// pass.
#[test]
fn an_add_killed_before_it_wrote_gitdir_is_unlisted_and_refuses_forced_cleanup() {
    let fixture = Fixture::created("add-killed-before-gitdir");
    let slot = fixture.task("alpha", 1);
    fixture
        .manager
        .write_intent(&mut NoHooks, &slot)
        .expect("intent");
    let path = fixture.manager.slot_path(&slot);
    let name = path
        .file_name()
        .expect("a slot path has a final component")
        .to_os_string();
    let worktrees = fixture.manager.common_git_dir.join("worktrees");
    let stale = worktrees.join(&name);
    fs::create_dir_all(&stale).expect("the admin directory the add makes first");
    fs::write(stale.join("locked"), "initializing\n").expect("the lock the add writes next");
    fs::write(stale.join("gitdir"), []).expect("the gitdir the add opened and never wrote");
    fs::create_dir_all(&path).expect("the checkout directory the add made, still empty");
    let intents = fixture
        .manager
        .intent_path(&slot)
        .parent()
        .expect("an intent lives in a directory")
        .to_path_buf();
    let expected = format!(
        "worktree registration {} has an empty gitdir",
        stale.display()
    );

    // An unrelated, populated slot, present for both refusals: its checkout
    // and its administrative directory are in every comparison below.
    let other = fixture.add_task(&mut NoHooks, "beta", 1);
    let other_path = fixture.manager.slot_path(&other);
    fs::write(other_path.join("witness.txt"), "unrelated slot\n").expect("plant a file");
    let snapshot = || {
        (
            tree_bytes(&other_path),
            tree_bytes(&path),
            tree_bytes(&worktrees),
            tree_bytes(&intents),
        )
    };

    // The classifier's hole: durable state it reads as nothing, because Git
    // does not list it.
    let site = EffectSiteId::Worktree(WorktreeSite::Add);
    let target = ResidueTarget::new(&fixture.base).at(&path);
    assert!(
        !fixture
            .manager
            .worktree_records()
            .expect("Git enumerates")
            .iter()
            .any(|record| canonical_prefix(record.path()).ok() == canonical_prefix(&path).ok()),
        "`git worktree list` skips a zero-length gitdir"
    );
    assert_eq!(classify(site, &target), ObjectResidue::None);
    assert_eq!(
        observed_residue_elements(site, &target).expect("observe"),
        Vec::<ResidueElement>::new(),
        "no registered element sees an admin directory Git does not enumerate"
    );

    // The refusal, for this slot: the diagnostic names this admin directory
    // and nothing else, and none of the four trees changed. It is the scan's
    // last failure, after the access's deadline.
    let refused = |error: &UpstrokeError| match error {
        UpstrokeError::RegistryRefused { message } => {
            message.ends_with(&format!("the last failed with: git error: {expected}"))
        }
        _ => false,
    };
    let before = snapshot();
    assert!(
        before.0.len() > 3 && before.2.len() > 4,
        "the unrelated checkout and the worktree store hold more than a named check would \
         cover: {:?} / {:?}",
        before.0.keys().collect::<Vec<_>>(),
        before.2.keys().collect::<Vec<_>>()
    );
    let error = fixture
        .manager
        .remove_worktree(&mut NoHooks, &slot)
        .expect_err("a registration that names nothing refuses");
    assert!(refused(&error), "expected `{expected}`, got: {error}");
    assert_eq!(
        snapshot(),
        before,
        "the refusal changed nothing in either checkout, the worktree store or the intents"
    );

    // ... and for every other slot through this funnel, which is what makes
    // the residue a finding rather than a stuck sample.
    let before = snapshot();
    let error = fixture
        .manager
        .remove_worktree(&mut NoHooks, &other)
        .expect_err("identification enumerates every registration");
    assert!(
        refused(&error),
        "the same diagnostic, naming the stale directory, for an unrelated slot: {error}"
    );
    assert_eq!(
        snapshot(),
        before,
        "the refusal changed nothing in either checkout, the worktree store or the intents"
    );
    assert!(
        fixture
            .manager
            .worktree_records()
            .expect("records")
            .iter()
            .any(|record| record.path() == other_path),
        "the unrelated slot is still registered"
    );
}

/// The same two torn registrations under the proof the plain funnel lacks —
/// no writer of the execution root is alive, which terminal finalization
/// holds (a durable `run_finished` proves every add of the run passed) and
/// the kill samplers hold once their child is reaped
/// (`WriterProof::NoWriterAlive`), and nothing that resumes: each entry that names no checkout
/// is passed over and reported, never bound by its Git-generated name and
/// never touched, and the slot's contained checkout and intent converge.
/// The two states are the ones an add leaves when killed inside its first
/// two writes: `locked` alone (opened, never written), and `locked` beside
/// an empty `gitdir` — PR10's ST-07 sampler measured both
/// (`~/pr10-evidence/r3/sampler-prefix-instrumented.log`, runs 17 and 22).
/// What is checked is the whole `.git/worktrees/` store byte for byte, an
/// unrelated populated slot's checkout, and Git's enumeration; and that the
/// plain funnel still refuses over the same store.
#[test]
fn a_torn_registration_is_passed_over_and_the_slot_converges_when_no_writer_is_alive() {
    let fixture = Fixture::created("torn-passed-over");
    let slot = fixture.task("alpha", 1);
    fixture
        .manager
        .write_intent(&mut NoHooks, &slot)
        .expect("intent");
    let path = fixture.manager.slot_path(&slot);
    let name = path
        .file_name()
        .expect("a slot path has a final component")
        .to_os_string();
    let worktrees = fixture.manager.common_git_dir.join("worktrees");
    let stale = worktrees.join(&name);
    fs::create_dir_all(&stale).expect("the admin directory the add makes first");
    fs::write(stale.join("locked"), "initializing\n").expect("the lock the add writes next");
    fs::write(stale.join("gitdir"), []).expect("the gitdir the add opened and never wrote");
    fs::create_dir_all(&path).expect("the checkout directory the add made, still empty");
    let earlier = worktrees.join("kalpha-g0");
    fs::create_dir_all(&earlier).expect("the admin directory of an earlier torn add");
    fs::write(earlier.join("locked"), []).expect("its lock, opened and never written");

    let other = fixture.add_task(&mut NoHooks, "beta", 1);
    let other_path = fixture.manager.slot_path(&other);
    fs::write(other_path.join("witness.txt"), "unrelated slot\n").expect("plant a file");
    let store_before = tree_bytes(&worktrees);
    let other_before = tree_bytes(&other_path);
    assert!(
        store_before.len() > 6,
        "the store holds the two torn entries and the unrelated slot's registration: {:?}",
        store_before.keys().collect::<Vec<_>>()
    );

    let passed_over = fixture
        .manager
        .remove_worktree_proving(&mut NoHooks, &slot, WriterProof::NoWriterAlive)
        .expect("a registration that names nothing is passed over, not refused");
    assert_eq!(
        passed_over,
        vec![earlier.clone(), stale.clone()],
        "both torn entries are reported, sorted, whichever slot left them"
    );
    assert!(!path.exists(), "the contained checkout is reclaimed");
    assert_eq!(
        tree_bytes(&worktrees),
        store_before,
        "neither torn entry nor the unrelated slot's registration was touched"
    );
    assert_eq!(tree_bytes(&other_path), other_before);
    fixture
        .manager
        .remove_intent(&mut NoHooks, &slot)
        .expect("intent removal converges");
    assert_eq!(
        fixture.manager.intents().expect("intents"),
        vec![other.clone()],
        "the torn slot's intent is gone and the unrelated slot's stands"
    );
    assert!(
        fixture
            .manager
            .worktree_records()
            .expect("Git enumerates: it lists neither torn entry")
            .iter()
            .all(|record| !record.path().ends_with("kalpha-g1")),
        "Git registers nothing for the slot"
    );

    let again = fixture
        .manager
        .remove_worktree_proving(&mut NoHooks, &slot, WriterProof::NoWriterAlive)
        .expect("idempotent");
    assert_eq!(again, passed_over, "and still reported");
    assert_eq!(tree_bytes(&worktrees), store_before);

    let error = fixture
        .manager
        .remove_worktree(&mut NoHooks, &other)
        .expect_err("without the proof the plain funnel still refuses over this store");
    let text = error.to_string();
    assert!(
        text.contains("has an empty gitdir") || text.contains("is locked and has no gitdir"),
        "the refusal names whichever torn entry the scan met first: {text}"
    );
    assert_eq!(tree_bytes(&other_path), other_before, "and changed nothing");
    let unrelated = fixture
        .manager
        .remove_worktree_proving(&mut NoHooks, &other, WriterProof::NoWriterAlive)
        .expect("the unrelated slot converges under the proof");
    assert_eq!(
        unrelated, passed_over,
        "reporting the same two torn entries"
    );
    assert!(!other_path.exists());
    assert!(
        stale.join("gitdir").exists() && earlier.join("locked").exists(),
        "the torn entries outlive every removal: Git's, or an operator's, to remove"
    );

    // The same store with both torn entries in the first shape — `locked`,
    // no `gitdir` — which the plain funnel skipped instead of refusing until
    // PR10's round 4, while the rustdoc promised one boundary for both (the
    // round-4 record lens, P2-2): the refusal names the missing file, the
    // proof passes both over, and neither is touched.
    fs::remove_file(stale.join("gitdir")).expect("the second torn entry now lacks gitdir too");
    let store_before = tree_bytes(&worktrees);
    let error = fixture
        .manager
        .remove_worktree(&mut NoHooks, &other)
        .expect_err("without the proof the plain funnel refuses a locked entry without gitdir");
    assert!(
        error.to_string().contains("is locked and has no gitdir"),
        "the refusal names the missing gitdir: {error}"
    );
    assert_eq!(tree_bytes(&worktrees), store_before, "and changed nothing");
    let both = fixture
        .manager
        .remove_worktree_proving(&mut NoHooks, &other, WriterProof::NoWriterAlive)
        .expect("under the proof both entries are passed over");
    assert_eq!(both, passed_over, "the same two, reported");
    assert_eq!(
        tree_bytes(&worktrees),
        store_before,
        "and neither is touched"
    );
}

#[test]
fn an_absent_registration_gitdir_is_already_gone_for_forced_cleanup() {
    let fixture = Fixture::created("absent-gitdir-converges");
    let slot = fixture.add_task(&mut NoHooks, "alpha", 1);
    let path = fixture.manager.slot_path(&slot);
    let admin = fixture
        .manager
        .revalidate_removal(&path)
        .expect("admin dir")
        .expect("registered");
    fs::remove_file(admin.join("gitdir")).expect("interrupt before gitdir survives");

    fixture
        .manager
        .remove_worktree(&mut NoHooks, &slot)
        .expect("missing identity metadata is forced-cleanup convergence");
    assert!(!path.exists(), "the exact contained checkout is reclaimed");
}

/// T-C3-1, #329's pinned refusal changed by closure 3: the store moved aside,
/// the target populated with a `.git` naming the absent store's entry. At C the
/// removal refuses with the checkout kept, and every resume's finalization
/// with it; under F the scan binds nothing for it, the checkout is removed and
/// nothing of the store is touched.
#[test]
fn a_missing_stored_worktree_directory_converges_removing_only_the_checkout() {
    let fixture = Fixture::created("missing-worktrees-store");
    let slot = fixture.add_task(&mut NoHooks, "alpha", 1);
    let path = fixture.manager.slot_path(&slot);
    let admin = fixture
        .manager
        .revalidate_removal(&path)
        .expect("admin dir")
        .expect("registered");
    let worktrees = admin.parent().expect("worktrees directory").to_path_buf();
    let moved = fixture.root.join("stored-worktrees-moved");
    fs::rename(&worktrees, &moved).expect("move the stored metadata");
    let store_before = tree_bytes(&moved);

    let removed = fixture.manager.remove_worktree(&mut NoHooks, &slot);
    let outcome = match removed {
        Ok(()) if !path.exists() => Ok(()),
        Ok(()) => Err("the removal answered Ok and left the checkout".to_owned()),
        Err(error) => Err(format!("the removal refused, the checkout kept: {error}")),
    };
    report_cases(&[("the store absent".to_owned(), outcome)]);
    assert!(!worktrees.exists(), "no store was made");
    assert_eq!(
        tree_bytes(&moved),
        store_before,
        "nothing of the store is touched"
    );
}

#[test]
fn a_registration_rebound_after_validation_keeps_its_admin_state() {
    struct RebindAtBefore {
        gitdir: PathBuf,
        replacement: PathBuf,
    }

    impl EffectHooks for RebindAtBefore {
        fn phase(&mut self, site: EffectSiteId, phase: HookPhase) -> Injection {
            if site == EffectSiteId::Worktree(WorktreeSite::Remove) && phase == HookPhase::Before {
                fs::write(&self.gitdir, format!("{}\n", self.replacement.display()))
                    .expect("replace the registration identity");
            }
            Injection::Proceed
        }

        fn refusal_cause(&self) -> Option<String> {
            None
        }
    }

    let fixture = Fixture::created("admin-rebound");
    let slot = fixture.add_task(&mut NoHooks, "alpha", 1);
    let path = fixture.manager.slot_path(&slot);
    let admin = fixture
        .manager
        .revalidate_removal(&path)
        .expect("admin dir")
        .expect("registered");
    let locked = admin.join("locked");
    fs::write(&locked, "do not remove\n").expect("replacement state");
    let other = fixture.root.join("other").join(".git");
    let mut hooks = RebindAtBefore {
        gitdir: admin.join("gitdir"),
        replacement: other,
    };

    fixture
        .manager
        .remove_worktree(&mut hooks, &slot)
        .expect_err("changed identity refuses");
    assert!(locked.exists(), "identity is rechecked before lock removal");
    assert!(admin.exists(), "identity is rechecked before admin removal");
}

/// The other half of the cancellation clause: "an ephemeral snapshot commit
/// created *before* the intent is left to Git".
#[test]
fn an_ephemeral_snapshot_commit_created_before_the_intent_is_left_to_git() {
    let fixture = Fixture::created("ephemeral-before-intent");
    let (mut hooks, _shared) = harness();
    let tree = git(
        &fixture.base,
        &["rev-parse", &format!("{}^{{tree}}", fixture.head)],
    );

    let commit = fixture
        .manager
        .snapshot_commit_tree(&mut hooks, &tree, &fixture.head)
        .expect("ephemeral commit");
    let slot = Slot::Snapshot {
        name: SnapshotName::gates(0, 1, 1),
    };
    assert!(
        !fixture.manager.intent_path(&slot).exists(),
        "the object exists and nothing durable claims it yet"
    );
    assert!(
        unreachable_objects(&fixture.base)
            .expect("fsck")
            .contains(&commit),
        "so it is unreferenced R27 residue: Git's"
    );

    // Nothing reclaims it and nothing may: the resume action is to leave it.
    fixture
        .manager
        .reclaim_intents(&mut hooks)
        .expect("reclaim finds no intent");
    assert!(
        unreachable_objects(&fixture.base)
            .expect("fsck")
            .contains(&commit),
        "reclaim leaves the object exactly where Git put it"
    );

    // And the full sequence puts the commit-tree before the intent.
    //
    // **On a harness of its own** (`PR5-WORKSPACE-022`). `HookHarness::
    // coverage()` is a *first-observation* log — one entry per `(site,
    // phase)` however many times it fires — and this test has already
    // driven `snapshot_commit_tree` standalone above, through `hooks`. So
    // the entry a `position()` first-match found for
    // `SnapshotCommitTree/After` was that earlier, unrelated invocation,
    // which precedes every event `add_snapshot` emits whatever order
    // `add_snapshot` uses internally: the assertion below passed with the
    // intent written first, which is exactly what it exists to forbid, and
    // the ordering it names was never measured at all. A fresh harness
    // starts empty, so every index below is this call's own. Taking a
    // *mark* into the old log would not have worked either — the second
    // occurrence is not recorded, so there is nothing after the mark.
    let (mut measured, ordering) = harness();
    let snapshot = fixture
        .manager
        .add_snapshot(
            &mut measured,
            &SnapshotName::gates(0, 2, 1),
            &SnapshotInput::Tree {
                tree: oid(&tree),
                parent: oid(&fixture.head),
            },
        )
        .expect("snapshot");
    let observed = ordering.lock().expect("harness").coverage().to_vec();
    let index = |site: EffectSiteId, phase: HookPhase| {
        observed
            .iter()
            .position(|seen| seen.site == site && seen.phase == phase)
            .unwrap_or_else(|| panic!("{site} {phase} was never observed inside this add_snapshot"))
    };
    assert!(
        index(
            EffectSiteId::Object(ObjectSite::SnapshotCommitTree),
            HookPhase::After
        ) < index(
            EffectSiteId::Snapshot(SnapshotSite::WriteIntent),
            HookPhase::Before
        ),
        "the ephemeral commit is created before the intent"
    );
    // The fresh harness is load-bearing, so it is checked rather than
    // trusted: this log holds this add's own commit-tree and nothing
    // earlier could have supplied it.
    assert_eq!(
        ordering.lock().expect("harness").count(
            EffectSiteId::Object(ObjectSite::SnapshotCommitTree),
            HookPhase::After
        ),
        1,
        "this add's commit-tree fired exactly once, on a log that began empty"
    );
    assert_eq!(snapshot.ephemeral(), Some(snapshot.head()));
    assert!(
        !unreachable_objects(&fixture.base)
            .expect("fsck")
            .contains(&snapshot.head().to_string()),
        "and the add makes it the snapshot HEAD: R24, no longer R27"
    );
}

/// An integration snapshot **creates no object**, and two snapshot names in
/// one repository are two live checkouts (`PR5-CONF-007`, `PR5-CONF-008`).
///
/// One function, two clauses of `workspace_candidates`, and neither had a
/// witness — both mutations survived the whole suite:
///
/// * make the `SnapshotInput::Commit` arm fabricate an ephemeral commit and
///   return it as the head, against "integration snapshots check out the
///   proposal or head commit and **create no object**";
/// * ignore the supplied `SnapshotName`, derive one slot from the judged
///   tree and hand back the existing checkout on later calls, against "one
///   snapshot for the gate set and one fresh snapshot per reviewer, **never
///   reused across roles or attempts**".
///
/// The measured cause was the same for both: `SnapshotInput::Commit` and
/// `SnapshotName::review` were **constructed nowhere in the crate**, and
/// `add_snapshot`'s two callers each used a separate fixture, so no fixture
/// ever held two snapshots alive at once. The recorded carry justification
/// said a second live request needed orchestration "PR5's scope stops
/// before"; it is two calls in one fixture, and here they are.
/// `review-common`'s standing ruling is the general form: *"'No production
/// caller' has a shelf life of one slice."*
///
/// The two axes are the *input variant* and the *name*, and this test is one
/// test rather than two because the surviving pair is one function: every
/// other `add_snapshot` call in the tree holds both constant at
/// `Tree`/`gates(0, 1, 1)`.
/// PR11's registry lock (the working record's R-X). Threads add and remove
/// snapshots through their own clones of one manager, as the coordinator's
/// pipelines do beside its task and staging worktrees: every removal prunes the
/// repository's registry and every funnel enumerates it, and Git 2.43's registry
/// is not safe against itself (`git worktree add` raced by `prune` or `list`
/// fails, measured in the record's §8), so the lock is what keeps every cycle
/// here from failing.
///
/// **Since #329 its accesses wait to the production deadline** (#329's record,
/// §9.31, B14). Under the tolerant registry access they race each other's
/// registry writes and are attempted again, and one attempt the platform
/// makes slow must not spend the test build's 500 ms first.
#[test]
fn concurrent_snapshot_adds_and_removals_on_one_repository_never_fail() {
    const THREADS: u32 = 4;
    const ROUNDS: u32 = 30;

    let fixture = Fixture::created("registry-lock");
    let _deadline = super::RegistryDeadline::hold(
        fixture.manager.common_git_dir(),
        PRODUCTION_REGISTRY_DEADLINE,
    );
    let failures: Vec<String> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..THREADS)
            .map(|thread| {
                let manager = fixture.manager.clone();
                let head = fixture.head.clone();
                scope.spawn(move || {
                    let mut failures = Vec::new();
                    for round in 1..=ROUNDS {
                        let name = SnapshotName::gates(thread, 0, round);
                        match manager.add_snapshot(
                            &mut NoHooks,
                            &name,
                            &SnapshotInput::Commit(oid(&head)),
                        ) {
                            Ok(snapshot) => {
                                if let Err(error) = manager.remove_snapshot(&mut NoHooks, &snapshot)
                                {
                                    failures.push(format!("removing {name}: {error}"));
                                }
                            }
                            Err(error) => failures.push(format!("adding {name}: {error}")),
                        }
                    }
                    failures
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|worker| worker.join().expect("a worker thread"))
            .collect()
    });
    assert!(
        failures.is_empty(),
        "{} of {} add/remove cycles failed: {failures:#?}",
        failures.len(),
        THREADS * ROUNDS
    );
    assert!(
        fixture.manager.intents().expect("intents").is_empty(),
        "every snapshot went with its intent"
    );
    assert_eq!(
        fixture
            .manager
            .worktree_records()
            .expect("the registry enumerates")
            .len(),
        1,
        "only the base checkout is still registered"
    );
}

/// R7 (#329's record, §9.14): a sibling snapshot whose checkout cannot be read,
/// as a Windows checkout whose deletion is pending cannot. On `test (winguest)`
/// the test above failed once, "adding k3-g0-a16-gates: failed to read
/// …\snapshots\k1-g0-a16-gates: Access is denied. (os error 5)": the add's
/// gate resolved every path the worktree list named after the list's access
/// had returned, and Windows answers `ERROR_ACCESS_DENIED` to an open of a
/// directory another removal has deleted while a handle on it is still open.
/// Here a real snapshot is added and its checkout exchanged for a link to
/// itself, so its registration still names the path and resolving the path
/// fails with `ELOOP` where Windows answers `ERROR_ACCESS_DENIED`: the same
/// call, `canonical_prefix(record.path())`, failing the same way, with another
/// error. Unix, where such a link needs no privilege. Returns the sibling's
/// checkout path.
#[cfg(unix)]
fn plant_a_sibling_whose_checkout_cannot_be_read(fixture: &Fixture, key: u32) -> PathBuf {
    let path = a_sibling_snapshot(fixture, key);
    fs::remove_dir_all(&path).expect("take the sibling's checkout away");
    leave_a_link_to_itself(&path);
    // The plant must bite: the registry still names the sibling, and its path
    // fails to resolve with something other than absence.
    let name = path.file_name().expect("a checkout name").to_owned();
    assert!(
        fixture
            .manager
            .worktree_records()
            .expect("the registry lists")
            .iter()
            .any(|record| record.path().ends_with(&name)),
        "prerequisite not met: the registry no longer names the sibling"
    );
    assert!(
        matches!(
            canonical_prefix(&path),
            Err(UpstrokeError::Io { ref source, .. })
                if source.kind() != std::io::ErrorKind::NotFound
        ),
        "prerequisite not met: the sibling's path resolves, so nothing here fails to read it"
    );
    path
}

/// A snapshot of task `key` beside the one a witness adds, through the
/// production funnels: its checkout path.
#[cfg(unix)]
fn a_sibling_snapshot(fixture: &Fixture, key: u32) -> PathBuf {
    fixture
        .manager
        .add_snapshot(
            &mut NoHooks,
            &SnapshotName::gates(key, 0, 1),
            &SnapshotInput::Commit(oid(&fixture.head)),
        )
        .expect("the sibling snapshot")
        .path()
        .to_path_buf()
}

/// A link at `path` naming `path` itself: resolving it fails with `ELOOP`.
#[cfg(unix)]
fn leave_a_link_to_itself(path: &Path) {
    std::os::unix::fs::symlink(path, path).expect("leave a link to itself");
}

/// R7's observer: every wait of a registry access the call makes ends the
/// sibling's removal the witness left in flight, as the last handle on a
/// Windows checkout whose deletion is pending closing ends it — the link
/// standing in for that checkout is removed, and the path is absent from then
/// on — and is counted. With `plant_at`, the link is planted at that site's
/// `Before` hook rather than before the call.
#[cfg(unix)]
struct EndsTheSiblingsRemovalAtAPause {
    link: PathBuf,
    plant_at: Option<EffectSiteId>,
    pauses: usize,
}

#[cfg(unix)]
impl EffectHooks for EndsTheSiblingsRemovalAtAPause {
    fn phase(&mut self, site: EffectSiteId, phase: HookPhase) -> Injection {
        if phase == HookPhase::Before && self.plant_at == Some(site) {
            fs::remove_dir_all(&self.link).expect("take the sibling's checkout away");
            leave_a_link_to_itself(&self.link);
        }
        Injection::Proceed
    }

    fn refusal_cause(&self) -> Option<String> {
        None
    }

    fn registry_pause(&mut self, _pause: std::time::Duration) -> Result<(), UpstrokeError> {
        self.pauses += 1;
        match fs::remove_file(&self.link) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => panic!(
                "end the sibling's removal at {}: {error}",
                self.link.display()
            ),
        }
    }
}

/// R7, the add: a sibling whose checkout cannot be read while its removal is
/// in flight does not fail an add. The gate's list and the resolution of every
/// path it names are one registry access, so the read that failed is attempted
/// again, after a wait made through the call's own hooks — R1's guarantee that
/// the coordinator answers its messages for the length of every wait — and the
/// attempt after it reads the sibling as absent. At `f9c88fdb` the gate
/// returned the failed read at once, as `UpstrokeError::Io`, and no wait ran.
/// The gate's access waits to the production deadline (the record's §9.31,
/// B14), so it asks for the wait after its first failed attempt only if time
/// remains when it reads its clock after that attempt's veto and count. If
/// the deadline has passed by then, wherever the time went (the attempt, its
/// veto, the count, or its thread descheduled), it asks for no wait: it makes
/// its final attempt, or refuses if the failed attempt was already its final
/// one (the record's §9.32 and §9.34).
#[cfg(unix)]
#[test]
fn a_sibling_whose_checkout_cannot_be_read_while_its_removal_is_in_flight_does_not_fail_an_add() {
    let fixture = Fixture::created("r7-sibling-read-add");
    let _deadline = super::RegistryDeadline::hold(
        fixture.manager.common_git_dir(),
        PRODUCTION_REGISTRY_DEADLINE,
    );
    let sibling = plant_a_sibling_whose_checkout_cannot_be_read(&fixture, 1);
    let attempted_again = contended_attempts(fixture.manager.common_git_dir());
    let mut hooks = EndsTheSiblingsRemovalAtAPause {
        link: sibling,
        plant_at: None,
        pauses: 0,
    };

    let snapshot = fixture
        .manager
        .add_snapshot(
            &mut hooks,
            &SnapshotName::gates(3, 0, 1),
            &SnapshotInput::Commit(oid(&fixture.head)),
        )
        .expect("a sibling's removal in flight does not fail the add");

    assert!(
        hooks.pauses >= 1,
        "the read that failed waited through the call's hooks before it was attempted again"
    );
    assert!(
        contended_attempts(fixture.manager.common_git_dir()) > attempted_again,
        "the registry access answered that it would attempt again"
    );
    assert!(
        fs::symlink_metadata(&hooks.link).is_err(),
        "the sibling's removal ended at the wait"
    );
    assert!(
        snapshot.path().join(".git").is_file(),
        "and the add made its checkout"
    );
}

/// R7, the bound: a sibling whose checkout stays unreadable refuses the add
/// resumably, at the registry access's deadline, as
/// `UpstrokeError::RegistryRefused` carrying the read's own error — never as
/// the raw I/O error, which `f9c88fdb` returned at once. The gate refuses
/// before the add's intent is written, so nothing of the add is left.
#[cfg(unix)]
#[test]
fn a_sibling_whose_checkout_stays_unreadable_refuses_the_add_resumably_and_never_as_io() {
    let fixture = Fixture::created("r7-sibling-read-refused");
    let sibling = plant_a_sibling_whose_checkout_cannot_be_read(&fixture, 1);
    let sibling = sibling
        .file_name()
        .expect("a checkout name")
        .to_string_lossy();
    let name = SnapshotName::gates(3, 0, 1);

    let error = fixture
        .manager
        .add_snapshot(
            &mut NoHooks,
            &name,
            &SnapshotInput::Commit(oid(&fixture.head)),
        )
        .expect_err("a sibling that stays unreadable refuses the add");

    let UpstrokeError::RegistryRefused { message } = &error else {
        panic!("the refusal is the registry's, typed and resumable, not {error:?}");
    };
    assert!(
        message.contains(&format!("{sibling}: ")) && message.contains("failed to read"),
        "it carries the read that failed: {message}"
    );
    let slot = Slot::Snapshot { name };
    assert!(
        !fixture.manager.intent_path(&slot).exists() && !fixture.manager.slot_path(&slot).exists(),
        "the gate refused before the add wrote its intent or made its destination"
    );
}

/// R7, the verification's lookup: the same read in `worktree_record`, where a
/// verification looks its slot up among the paths the list names. The sibling's
/// checkout is made unreadable at the verification's `Before` hook, after its
/// gate has passed, and its removal ends at the lookup's first wait: the lookup
/// attempts again and answers for the slot, which is not registered. At
/// `f9c88fdb` the lookup returned the failed read at once, as
/// `UpstrokeError::Io`. The lookup waits to the production deadline (the
/// record's §9.31, B14).
#[cfg(unix)]
#[test]
fn a_sibling_whose_checkout_cannot_be_read_while_its_removal_is_in_flight_does_not_fail_a_verification()
 {
    let fixture = Fixture::created("r7-sibling-read-verify");
    let _deadline = super::RegistryDeadline::hold(
        fixture.manager.common_git_dir(),
        PRODUCTION_REGISTRY_DEADLINE,
    );
    let sibling = a_sibling_snapshot(&fixture, 1);
    let slot = fixture.task("alpha", 1);
    let mut hooks = EndsTheSiblingsRemovalAtAPause {
        link: sibling,
        plant_at: Some(EffectSiteId::Worktree(WorktreeSite::Verify)),
        pauses: 0,
    };

    let verified = fixture
        .manager
        .verify_worktree(&mut hooks, &slot, &Quiescence::AtBase(fixture.head.clone()))
        .expect("a sibling's removal in flight does not fail the verification");

    assert_eq!(verified, Err(VerifyFailure::NotRegistered));
    assert!(
        hooks.pauses >= 1,
        "the lookup's read that failed waited through the call's hooks"
    );
    assert!(
        fs::symlink_metadata(&hooks.link).is_err(),
        "the sibling's removal ended at the wait"
    );
}

/// R7, what the resolution reads and refuses: a sibling whose checkout is a
/// link to nothing is not a path that failed to read. Resolving it reads the
/// link, and the refusal it makes is the gate's answer at once, as it was
/// before the resolution joined the registry access: no attempt is made again
/// and no deadline is waited out, so the refusal is never retyped as the
/// registry's.
#[cfg(unix)]
#[test]
fn a_sibling_whose_checkout_is_a_link_to_nothing_refuses_the_add_at_once() {
    let fixture = Fixture::created("r7-sibling-link-refused");
    let sibling = a_sibling_snapshot(&fixture, 1);
    fs::remove_dir_all(&sibling).expect("take the sibling's checkout away");
    std::os::unix::fs::symlink(fixture.root.join("nothing-here"), &sibling)
        .expect("leave a link to nothing in its place");
    let attempted_again = contended_attempts(fixture.manager.common_git_dir());

    let error = fixture
        .manager
        .add_snapshot(
            &mut NoHooks,
            &SnapshotName::gates(3, 0, 1),
            &SnapshotInput::Commit(oid(&fixture.head)),
        )
        .expect_err("a link on a listed path refuses the add");

    assert!(
        matches!(&error, UpstrokeError::Refused { message }
            if message.contains("is a symlink or reparse point")),
        "the refusal is the link's, as the resolution made it: {error:?}"
    );
    assert_eq!(
        contended_attempts(fixture.manager.common_git_dir()),
        attempted_again,
        "and the registry access never answered that it would attempt again"
    );
}

/// The early review's `R1-REG-1` (the PR11 record, §13, round R1): the removal
/// held the registry lock across the whole funnel, `Before` and `After` hooks
/// included, and `worktree_records` takes the same lock, so an observer that
/// lists the worktrees from either hook waited for ever on the thread that
/// called it. The removal runs on a thread of its own and is waited for a
/// bounded time, so the regression fails here instead of hanging the suite; the
/// thread it would leave blocked holds only this fixture's repository's lock.
#[test]
fn an_observer_lists_the_worktrees_from_both_hooks_of_a_removal() {
    const BOUND: std::time::Duration = std::time::Duration::from_secs(60);

    type Seen = Vec<(HookPhase, Result<usize, String>)>;

    struct Listing {
        manager: WorkspaceManager,
        site: EffectSiteId,
        seen: Seen,
    }

    impl EffectHooks for Listing {
        fn phase(&mut self, site: EffectSiteId, phase: HookPhase) -> Injection {
            if site == self.site {
                let listed = self
                    .manager
                    .worktree_records()
                    .map(|records| records.len())
                    .map_err(|error| error.to_string());
                self.seen.push((phase, listed));
            }
            Injection::Proceed
        }

        fn refusal_cause(&self) -> Option<String> {
            None
        }
    }

    let fixture = Fixture::created("list-from-removal-hooks");
    let slot = fixture.add_task(&mut NoHooks, "alpha", 1);
    let mut hooks = Listing {
        manager: fixture.manager.clone(),
        site: slot.remove_site(),
        seen: Vec::new(),
    };
    let manager = fixture.manager.clone();
    let (done, finished) = std::sync::mpsc::channel::<(Result<(), String>, Seen)>();
    std::thread::spawn(move || {
        let removed = manager
            .remove_worktree(&mut hooks, &slot)
            .map_err(|error| error.to_string());
        let _ = done.send((removed, hooks.seen));
    });
    let (removed, seen) = finished.recv_timeout(BOUND).unwrap_or_else(|_| {
        panic!(
            "the removal did not return within {BOUND:?}: an observer listing the worktrees from \
             its hook waits on the registry lock its own caller holds"
        )
    });
    removed.expect("the removal completes");
    assert_eq!(
        seen,
        vec![(HookPhase::Before, Ok(2)), (HookPhase::After, Ok(1))],
        "the `Before` hook lists the base checkout and the slot, the `After` hook the base \
         checkout alone"
    );
    assert_eq!(
        fixture.manager.intents().expect("intents").len(),
        1,
        "the removal leaves the slot's intent for its caller to remove"
    );
}

/// Review round 4's `R4-REG-1` (the PR11 record, §13): `LinkedChild::spawn`
/// started the reader of the child's stdout before anything owned the child, so
/// a reader thread the system could not create unwound past a bare
/// `std::process::Child`, whose drop neither kills nor reaps, and the child
/// outlived its failed constructor. The reader is refused for real here -- its
/// builder asks for a stack no address space can map, so the thread's creation
/// fails in the OS (`pthread_create` answers `EAGAIN`) -- and the constructor must
/// unwind through the owner's drop, leaving nothing under the child's pid, not
/// even a zombie. The child is given a name no test has, so it runs nothing and
/// leaves nothing of its own. Unix, because what answers for a pid is
/// `fixture::process_exists`; the construction is the same code on Windows.
#[cfg(unix)]
#[test]
fn a_linked_child_whose_reader_cannot_be_started_is_killed_and_reaped() {
    const NO_SUCH_TEST: &str = "workspace_manager::tests::no_test_is_named_this";
    let scratch = scratch("linked-reader-refused");
    let stderr = scratch.path().join("stderr.log");
    let asked = std::cell::Cell::new(None);
    let refusal = panic_message(|| {
        drop(super::fixture::LinkedChild::spawn_with(
            NO_SUCH_TEST,
            &[],
            &stderr,
            |pid| {
                asked.set(Some(pid));
                crate::agent::proc::test_support::readiness::unstartable_reader()
            },
        ));
    });
    let pid = asked
        .get()
        .expect("the constructor spawned its child and asked for the child's reader");
    let refusal = refusal.unwrap_or_else(|| {
        panic!("the linked child {pid} was constructed although its reader could not be started")
    });
    let state = std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()
        .and_then(|stat| {
            stat.rsplit_once(") ")
                .and_then(|(_, rest)| rest.split(' ').next().map(str::to_owned))
        })
        .map_or_else(String::new, |state| format!(" (its state: {state})"));
    assert!(
        !super::fixture::process_exists(pid),
        "the linked child {pid} outlived its failed constructor, neither killed nor reaped{state}; \
         the constructor failed with: {refusal}"
    );
}
