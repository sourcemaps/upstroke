// Allowlist placement: the funnel section of `effects/allowlist.toml`.
#![allow(clippy::disallowed_methods, clippy::disallowed_types)]
#![forbid(clippy::disallowed_macros)]

use super::*;

#[test]
fn snapshots_create_no_object_for_a_commit_and_never_share_a_checkout() {
    let fixture = Fixture::created("snapshot-clauses");
    let common = PathBuf::from(git(
        &fixture.base,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    ));

    // (1) The integration case: an existing commit, and no object created.
    let before = loose_objects(&common);
    let integration = fixture
        .manager
        .add_snapshot(
            &mut NoHooks,
            &SnapshotName::integration(1),
            &SnapshotInput::Commit(oid(&fixture.head)),
        )
        .expect("Snapshot.WriteIntent + Snapshot.Add");
    assert_eq!(
        integration.head().as_str(),
        fixture.head,
        "an integration snapshot checks out the commit it was given"
    );
    assert_eq!(
        integration.ephemeral(),
        None,
        "…and records no ephemeral commit, because it created none"
    );
    assert_eq!(
        loose_objects(&common),
        before,
        "an integration snapshot created an object; `workspace_candidates` says it \
             checks out the proposal or head commit and **creates no object**"
    );
    assert_eq!(
        git(integration.path(), &["rev-parse", "HEAD"]),
        fixture.head,
        "and the checkout really is at that commit"
    );

    // (2) Two names, one fixture, both alive at once — the shape no fixture
    // in the tree built, and the whole reason the name could be ignored.
    let tree = git(&fixture.base, &["rev-parse", "HEAD^{tree}"]);
    let gates = fixture
        .manager
        .add_snapshot(
            &mut NoHooks,
            &SnapshotName::gates(0, 1, 1),
            &SnapshotInput::Tree {
                tree: oid(&tree),
                parent: oid(&fixture.head),
            },
        )
        .expect("the gate snapshot");
    let reviewer = fixture
        .manager
        .add_snapshot(
            &mut NoHooks,
            &SnapshotName::review(0, 1, 1, 0),
            &SnapshotInput::Tree {
                tree: oid(&tree),
                parent: oid(&fixture.head),
            },
        )
        .expect("a reviewer's snapshot on the same judged tree");

    assert_ne!(
        gates.slot(),
        reviewer.slot(),
        "the gate set and a reviewer are different roles and must not share a slot"
    );
    assert_ne!(
        gates.path(),
        reviewer.path(),
        "…and therefore not a checkout either: {} vs {}",
        gates.path().display(),
        reviewer.path().display()
    );
    assert!(
        gates.path().is_dir() && reviewer.path().is_dir(),
        "both snapshots are live at once; that is what 'never reused across roles \
             or attempts' means and what no fixture built"
    );
    // Not merely different names for one directory: each is separately
    // registered, and the kernel agrees they are two.
    assert_ne!(
        git(gates.path(), &["rev-parse", "--absolute-git-dir"]),
        git(reviewer.path(), &["rev-parse", "--absolute-git-dir"]),
        "two registered worktrees, not one directory under two names"
    );

    // The same role at a later attempt is a third, again without reuse.
    let retry = fixture
        .manager
        .add_snapshot(
            &mut NoHooks,
            &SnapshotName::gates(0, 1, 2),
            &SnapshotInput::Tree {
                tree: oid(&tree),
                parent: oid(&fixture.head),
            },
        )
        .expect("the gate snapshot of the next attempt");
    assert_ne!(
        retry.path(),
        gates.path(),
        "attempt 2's gate snapshot must not be attempt 1's checkout"
    );

    for snapshot in [&integration, &gates, &reviewer, &retry] {
        fixture
            .manager
            .remove_snapshot(&mut NoHooks, snapshot)
            .expect("Snapshot.Remove + Snapshot.RemoveIntent");
    }
}

/// A snapshot input the repository does not resolve to itself is refused by
/// name, before the ephemeral commit and before the intent (PR #130, pass 1).
///
/// `ObjectId` is a spelling, and a ref spelt as hexadecimal of the *other*
/// object format's length is one: measured on git 2.43, in a SHA-256
/// repository `git worktree add --detach <forty hex characters>` follows a
/// branch of that name to wherever it points, and the inverse in a SHA-1
/// repository. The branch here is created at the seed and moved to the head,
/// so that a checkout that followed it would be observable as one at the head.
/// The funnel resolves each id to the object type its role requires and
/// accepts only an answer equal to the input; the refusal carries what Git
/// answered. Witnessed failing with the resolution removed from
/// `add_snapshot` (the head's lexical-only check) in both repositories.
#[test]
fn a_snapshot_input_spelt_as_hexadecimal_of_the_other_formats_length_is_refused_by_name() {
    for (fixture, other_len, own_len) in [
        (Fixture::created("resolve-sha1"), 64, 40),
        (Fixture::created_sha256("resolve-sha256"), 40, 64),
    ] {
        assert_eq!(
            fixture.head.len(),
            own_len,
            "the fixture's object format is pinned"
        );
        let name: String = "0123456789abcdef".chars().cycle().take(other_len).collect();
        git(&fixture.base, &["branch", &name, &fixture.seed]);
        git(&fixture.base, &["branch", "-f", &name, &fixture.head]);
        assert_eq!(
            git(
                &fixture.base,
                &["rev-parse", "--verify", &format!("{name}^{{commit}}")]
            ),
            fixture.head,
            "Git resolves the hexadecimal name to the head the branch was moved to"
        );
        let tree = git(&fixture.base, &["rev-parse", "HEAD^{tree}"]);
        let common = PathBuf::from(git(
            &fixture.base,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        ));
        let objects_before = loose_objects(&common);
        let worktrees_before = git(&fixture.base, &["worktree", "list", "--porcelain"]);
        let missing: String = "f".repeat(own_len);

        let cases = [
            (
                1,
                SnapshotInput::Commit(oid(&name)),
                SnapshotObject::Commit,
                Some(fixture.head.clone()),
                name.clone(),
            ),
            (
                2,
                SnapshotInput::Tree {
                    tree: oid(&name),
                    parent: oid(&fixture.head),
                },
                SnapshotObject::Tree,
                Some(tree.clone()),
                name.clone(),
            ),
            (
                3,
                SnapshotInput::Tree {
                    tree: oid(&tree),
                    parent: oid(&name),
                },
                SnapshotObject::Parent,
                Some(fixture.head.clone()),
                name.clone(),
            ),
            (
                4,
                SnapshotInput::Commit(oid(&missing)),
                SnapshotObject::Commit,
                None,
                missing.clone(),
            ),
        ];
        for (attempt, input, role, resolved, value) in cases {
            let slot_name = SnapshotName::gates(0, 1, attempt);
            let error = fixture
                .manager
                .add_snapshot(&mut NoHooks, &slot_name, &input)
                .expect_err("an input the repository does not resolve to itself is refused");
            let expected = Refusal::SnapshotInputResolvesElsewhere {
                role,
                value,
                resolved,
                found_type: None,
            };
            assert!(
                matches!(&error, UpstrokeError::Refused { message } if *message == expected.to_string()),
                "{role} at {own_len}: expected `{expected}`, got `{error}`"
            );
            let slot = Slot::Snapshot { name: slot_name };
            assert!(
                !fixture.manager.intent_path(&slot).exists(),
                "{role} at {own_len}: the refusal precedes the intent"
            );
        }
        assert_eq!(
            git(&fixture.base, &["worktree", "list", "--porcelain"]),
            worktrees_before,
            "at {own_len}: nothing was checked out, so nothing followed the branch"
        );
        assert_eq!(
            loose_objects(&common),
            objects_before,
            "at {own_len}: the refusal precedes the ephemeral commit"
        );
    }
}

/// A full id of the repository's own format is accepted, and the snapshot's
/// HEAD is what Git reports for the checkout, in a repository of each object
/// format (PR #130, pass 1). The branch spelt as the head's own id points at
/// the seed: Git resolves the full id to the object, not to the ref, so the
/// snapshot is at the head and not where the branch points.
#[test]
fn a_full_id_of_the_repositorys_own_format_is_accepted_and_the_head_is_what_git_reports() {
    for (fixture, own_len) in [
        (Fixture::created("own-format-sha1"), 40),
        (Fixture::created_sha256("own-format-sha256"), 64),
    ] {
        assert_eq!(
            fixture.head.len(),
            own_len,
            "the fixture's object format is pinned"
        );
        git(&fixture.base, &["branch", &fixture.head, &fixture.seed]);

        let integration = fixture
            .manager
            .add_snapshot(
                &mut NoHooks,
                &SnapshotName::integration(1),
                &SnapshotInput::Commit(oid(&fixture.head)),
            )
            .expect("a full id of the repository's own format is accepted");
        assert_eq!(integration.head().as_str(), fixture.head);
        assert_eq!(
            git(integration.path(), &["rev-parse", "HEAD"]),
            fixture.head,
            "at {own_len}: the checkout is at the object, not where the same-named branch points"
        );
        assert_ne!(fixture.seed, fixture.head, "the branch points elsewhere");

        let tree = git(&fixture.base, &["rev-parse", "HEAD^{tree}"]);
        let gates = fixture
            .manager
            .add_snapshot(
                &mut NoHooks,
                &SnapshotName::gates(0, 1, 1),
                &SnapshotInput::Tree {
                    tree: oid(&tree),
                    parent: oid(&fixture.head),
                },
            )
            .expect("a tree and parent of the repository's own format are accepted");
        assert_eq!(
            gates.head().as_str().len(),
            own_len,
            "the ephemeral commit is of the repository's format"
        );
        assert_eq!(
            git(gates.path(), &["rev-parse", "HEAD"]),
            gates.head().as_str(),
            "at {own_len}: the snapshot's HEAD is what Git reports for the checkout"
        );
        assert_eq!(gates.ephemeral(), Some(gates.head()));

        for snapshot in [&integration, &gates] {
            fixture
                .manager
                .remove_snapshot(&mut NoHooks, snapshot)
                .expect("Snapshot.Remove + Snapshot.RemoveIntent");
        }
    }
}

/// A snapshot of a tree with a replacement object in place materialises the
/// tree that was judged, not its replacement (PR #130, pass 2's P1).
///
/// `git replace A B` makes Git read `B` wherever `A` is named while
/// `rev-parse` still prints `A`, so `add_snapshot`'s resolve-once check
/// cannot see it: measured on git 2.43, `commit-tree A -p P` records the raw
/// tree `A` and `worktree add --detach` on that commit writes the *contents
/// of `B`*, so gates and reviewers would run over another tree entirely,
/// against `DESIGN.md` §15's exact snapshot. The manager sets
/// `GIT_NO_REPLACE_OBJECTS=1` on every command it runs, so the mechanism is
/// gone rather than detected.
///
/// The assertion is on the materialised bytes and on the recorded commit's
/// raw tree, since the head is unchanged under a tree replacement and a test
/// that read only the head could not see this. Witnessed failing with the
/// environment variable removed from `command`: the checkout holds `B`.
///
/// **The work is in a neutralised child** (PR #271, round 4), for the reason
/// [`quiescence_holds_when_the_recorded_tree_carries_a_replacement`] is: the
/// manager's own `git` children inherit this process's environment, less the
/// work-tree context they shed in a work tree the engine made, the four
/// selectors, and with a relative `GIT_COMMON_DIR` given as the absolute path it
/// names at the started checkout, so an
/// operator's `GIT_NO_REPLACE_OBJECTS=1` -- which is what this pull request
/// makes upstroke's own gates supply -- would hand this witness the very
/// protection it exists to prove the manager installs, and the checkout would
/// hold `A` with the repair removed. The child is where
/// [`assert_replacement_controls_pinned`] can be stated, and it is stated there
/// before anything is measured.
#[test]
fn a_snapshot_ignores_a_replacement_object_and_materialises_the_judged_tree() {
    let status =
        run_replacement_witness_child("workspace_manager::tests::snapshot_replacement_helper");
    assert!(
        status.success(),
        "the child witnesses an exact snapshot over a replaced tree with every \
         ambient control over `refs/replace/*` taken away from it, and ended \
         {status:?}"
    );
}

/// A **role process** inside a snapshot reads the judged tree too (PR #130,
/// pass 3's P1, `PR130-REVIEW3-REPLACEMENT-ISOLATION-STOPS-AT-THE-MANAGER`).
///
/// `a_snapshot_ignores_a_replacement_object_and_materialises_the_judged_tree`
/// pins the snapshot's *filesystem*, and that was the whole of the guarantee:
/// it asserts bytes and the raw commit and never runs a role process, so it
/// could not see that a gate or a reviewer inside the snapshot got the
/// environment its runner composes -- which clears the ambient one -- and read
/// the replacement through Git. Measured on git 2.43 before the repair: `git
/// show HEAD:replaced.txt` printed `B` and `git status --porcelain` reported
/// `M  replaced.txt` against a checkout nothing had touched, so a gate reading
/// the tree through Git judged one tree and a gate reading the filesystem
/// judged another.
///
/// The witness is the production [`HostRunner`] over the production
/// [`WorkspaceManager`] snapshot, because the composition is exactly what was
/// missing: a unit assertion on `compose` alone would not have caught a runner
/// that composed the pair and then dropped it. The base deliberately carries no
/// `GIT_NO_REPLACE_OBJECTS`, so nothing here can pass by inheritance.
///
/// Witnessed failing with the pair removed from `HostEnvironment::compose`:
/// `git show` printed `B`, and `git status --porcelain` printed `M
/// replaced.txt`.
///
/// **The work is in a neutralised child, and it states two preconditions**
/// (PR #271, round 4). Round 3's blocking finding was that this witness -- *the
/// role-process path, which is the core of the finding* -- was the one the
/// round-3 neutraliser was never given: mutating `HostRunner::run` to discard
/// the pair from the composed environment exited `101` normally and **`0`**
/// under `GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=core.useReplaceRefs
/// GIT_CONFIG_VALUE_0=false`. The child closes the enumerated mechanisms and
/// [`assert_replacement_controls_pinned`] states that by name and by
/// measurement; [`assert_a_role_process_sees_replacements`] then measures the
/// one thing a list of names cannot reach, through this witness's own runner.
#[test]
fn a_role_process_in_a_snapshot_reads_the_judged_tree_not_a_replacement() {
    let status =
        run_replacement_witness_child("workspace_manager::tests::role_process_replacement_helper");
    assert!(
        status.success(),
        "the child witnesses a role process reading the judged tree with every \
         ambient control over `refs/replace/*` taken away from it, and ended \
         {status:?}"
    );
}

/// Whether a row of [`hostile_replacement_environments`] has to be a control on
/// every Git this suite runs on, or only on the ones that have it.
///
/// [`Reach::SomeGits`] is not a loophole: the row's neutralised leg is asserted
/// either way, and a row that is inert on this Git cannot be a masking vector
/// on this Git. What it admits is that Git changed. The one row that needs it
/// is the uncounted indexed configuration pair, and the reason is now measured
/// rather than assumed -- see that row.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Reach {
    EveryGit,
    SomeGits,
}

/// Which of a row's three legs to run.
///
/// The two that matter are [`Leg::Controlled`] and [`Leg::Treated`], and they
/// differ **only** by the row's treatment: both start from the neutralised
/// environment, both lift exactly the pins that would mask the row, and the
/// first must be replacement-live. That is what makes the second's failure
/// attributable to the row (PR #271, round 4). Until now the treated leg took
/// this process's environment raw, so `GIT_CONFIG_NOSYSTEM=1` exported into the
/// suite made the `GIT_CONFIG_SYSTEM` row inert and the grid exited `101`
/// blaming the row; and in the masking direction, an inert treatment
/// substituted for a real one exited `101` cleanly but **`0`** under an
/// exported `GIT_NO_REPLACE_OBJECTS=1`, because the grid credited the
/// environment's own suppression to the row.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Leg {
    /// Neutralised, the row's masking pins lifted, the row's controlled values
    /// applied, and **no treatment**. Must be replacement-live.
    Controlled,
    /// [`Leg::Controlled`] plus the row's treatment. This is where the row is
    /// measured to be a control at all.
    Treated,
    /// The row's controlled values and treatment applied as an operator's
    /// environment would have them, and *then* the neutralisation. Must be
    /// replacement-live: this is the claim the whole grid exists for.
    Neutralised,
}

/// One hostile environment per mechanism the enumeration claims to close,
/// built over `root`.
///
/// Every row is a way an operator, a wrapper or a machine can decide Git's
/// replacement behaviour for a child, measured one at a time on git 2.43.0.
/// The last row sets all of them at once, because a neutraliser that closes
/// each in isolation and leaves one open in combination is the shape this
/// pull request has already shipped twice.
struct HostileEnvironment {
    name: &'static str,
    reach: Reach,
    /// Pins [`without_ambient_replacement_controls`] installs that would mask
    /// this row, lifted from the controlled and treated legs alike so the row
    /// can act at all -- and never from the neutralised leg, which is the one
    /// asserting those pins win.
    ///
    /// Measured, git 2.43.0: `GIT_CONFIG_SYSTEM` is ignored outright while
    /// `GIT_CONFIG_NOSYSTEM` is set, and `HOME`/`XDG_CONFIG_HOME` are not read
    /// at all while `GIT_CONFIG_GLOBAL` is. A row whose pin is not lifted is
    /// not a weaker row, it is no row.
    lifted: &'static [&'static str],
    /// What puts the *dimension* this row acts in into a known,
    /// replacement-live state once its pins are lifted. Applied to the
    /// controlled leg and to the treated leg.
    controlled: Vec<(String, OsString)>,
    /// What makes the row hostile: the only difference between the controlled
    /// leg and the treated one.
    treatment: Vec<(String, OsString)>,
}

fn hostile_replacement_environments(root: &Path) -> Vec<HostileEnvironment> {
    let disables = root.join("disables.cfg");
    write_file(&disables, b"[core]\n\tuseReplaceRefs = false\n");
    // Written by Git's own configuration writer rather than by `format!`
    // (PR #271, round 4). A path is not a config value: measured on git
    // 2.43.0, an unquoted `.display()` of a directory containing `#` is
    // truncated at the comment character and the row silently stops being a
    // control, and one containing a backslash is read as an escape -- `git
    // config --list` over it exits `128`, `bad config line 2`. `git config
    // --file` quotes the first and doubles the second, and both then resolve.
    let including = root.join("including.cfg");
    write_include_path(&including, &disables);
    let home = root.join("home");
    write_file(
        &home.join(".gitconfig"),
        b"[core]\n\tuseReplaceRefs = false\n",
    );
    let xdg = root.join("xdg");
    write_file(
        &xdg.join("git").join("config"),
        b"[core]\n\tuseReplaceRefs = false\n",
    );
    let empty_home = root.join("empty-home");
    create_dir(&empty_home);
    let empty_xdg = root.join("empty-xdg");
    create_dir(&empty_xdg);
    // A template directory is the seventeenth mechanism, and it acts at
    // repository *creation*: `git init` copies this `config` into the new
    // repository's own, above everything `git init` writes there (measured,
    // git 2.43.0). Nothing an environment does afterwards can undo it, which
    // is why the probe's own `git init` runs in the environment under test.
    let template = root.join("template");
    write_file(
        &template.join("config"),
        b"[core]\n\tuseReplaceRefs = false\n",
    );

    let set = |key: &str, value: &OsStr| (key.to_owned(), value.to_owned());
    let row = |name: &'static str,
               reach: Reach,
               lifted: &'static [&'static str],
               controlled: Vec<(String, OsString)>,
               treatment: Vec<(String, OsString)>| HostileEnvironment {
        name,
        reach,
        lifted,
        controlled,
        treatment,
    };
    // Both file-backed rows below read a `HOME`/`XDG_CONFIG_HOME` pair, and
    // both legs of each get the same pair: only the one variable under test
    // differs between them.
    let neutral_dirs = || {
        vec![
            set("HOME", empty_home.as_os_str()),
            set("XDG_CONFIG_HOME", empty_xdg.as_os_str()),
        ]
    };
    let rows: Vec<HostileEnvironment> = vec![
        row(
            "GIT_NO_REPLACE_OBJECTS",
            Reach::EveryGit,
            &[],
            Vec::new(),
            vec![set("GIT_NO_REPLACE_OBJECTS", OsStr::new("1"))],
        ),
        row(
            "an indexed config pair, counted",
            Reach::EveryGit,
            &[],
            Vec::new(),
            vec![
                set("GIT_CONFIG_COUNT", OsStr::new("1")),
                set("GIT_CONFIG_KEY_0", OsStr::new("core.useReplaceRefs")),
                set("GIT_CONFIG_VALUE_0", OsStr::new("false")),
            ],
        ),
        // `Reach::SomeGits`, and the measurement behind it is corrected here
        // (PR #271, round 4). The claim this row used to carry -- that a lone
        // `GIT_CONFIG_KEY_0` takes effect on git 2.43.0 with no count -- was an
        // artefact of the environment it was measured in: this box's build
        // wrapper exports `GIT_CONFIG_COUNT=1` with an indexed pair of its own
        // into every `cargo test`, so the "uncounted" pair was counted. With
        // the count genuinely lifted, `git config --get core.useReplaceRefs`
        // exits `1` and prints nothing on git 2.43.0, and exits `0` printing
        // `false` with the count at `1`. The row is inert on this Git, kept as
        // `SomeGits` because the pair is real wherever a count reaches it and
        // the neutralised leg is asserted either way.
        row(
            "an indexed config pair, uncounted",
            Reach::SomeGits,
            &["GIT_CONFIG_COUNT"],
            Vec::new(),
            vec![
                set("GIT_CONFIG_KEY_0", OsStr::new("core.useReplaceRefs")),
                set("GIT_CONFIG_VALUE_0", OsStr::new("false")),
            ],
        ),
        row(
            "GIT_CONFIG_PARAMETERS",
            Reach::EveryGit,
            &[],
            Vec::new(),
            vec![set(
                "GIT_CONFIG_PARAMETERS",
                OsStr::new("'core.usereplacerefs'='false'"),
            )],
        ),
        row(
            "GIT_CONFIG_GLOBAL",
            Reach::EveryGit,
            &[],
            Vec::new(),
            vec![set("GIT_CONFIG_GLOBAL", disables.as_os_str())],
        ),
        row(
            "GIT_CONFIG_SYSTEM",
            Reach::EveryGit,
            &["GIT_CONFIG_NOSYSTEM"],
            Vec::new(),
            vec![set("GIT_CONFIG_SYSTEM", disables.as_os_str())],
        ),
        row(
            "GIT_CONFIG_GLOBAL through include.path",
            Reach::EveryGit,
            &[],
            Vec::new(),
            vec![set("GIT_CONFIG_GLOBAL", including.as_os_str())],
        ),
        row(
            "HOME",
            Reach::EveryGit,
            &["GIT_CONFIG_GLOBAL"],
            neutral_dirs(),
            vec![set("HOME", home.as_os_str())],
        ),
        row(
            "XDG_CONFIG_HOME",
            Reach::EveryGit,
            &["GIT_CONFIG_GLOBAL"],
            neutral_dirs(),
            vec![set("XDG_CONFIG_HOME", xdg.as_os_str())],
        ),
        row(
            "GIT_REPLACE_REF_BASE",
            Reach::EveryGit,
            &[],
            Vec::new(),
            vec![set("GIT_REPLACE_REF_BASE", OsStr::new("refs/elsewhere/"))],
        ),
        row(
            "GIT_TEMPLATE_DIR",
            Reach::EveryGit,
            &[],
            Vec::new(),
            vec![set("GIT_TEMPLATE_DIR", template.as_os_str())],
        ),
    ];
    let mut everything: Vec<(String, OsString)> = rows
        .iter()
        .flat_map(|row| row.treatment.iter().cloned())
        .collect();
    // `GIT_CONFIG` is enumerated and neutralised but is not a row of its own,
    // because it is not a control over what a Git child *reads*: measured, a
    // probe under it still honours `refs/replace/*`, and git-config(1) says the
    // variable has no effect on commands other than `git config`. What it
    // captures is a fixture's own `git config` **write**, which
    // `a_redirected_git_config_cannot_capture_a_fixtures_own_pin` measures
    // directly. It rides in the combination so the neutralised leg still has to
    // survive it beside everything else.
    everything.push(set("GIT_CONFIG", disables.as_os_str()));
    let mut rows = rows;
    rows.push(row(
        "all of them at once",
        Reach::EveryGit,
        // Every pin any row lifts, because the combination is every row.
        &[
            "GIT_CONFIG_COUNT",
            "GIT_CONFIG_NOSYSTEM",
            "GIT_CONFIG_GLOBAL",
        ],
        neutral_dirs(),
        everything,
    ));
    rows
}

/// Run [`replacement_control_probe_helper`] as one leg of `row`, and return its
/// exit status.
fn run_replacement_control_probe(row: &HostileEnvironment, leg: Leg) -> std::process::ExitStatus {
    let mut command = Command::new(std::env::current_exe().expect("this test binary"));
    command
        .args([
            "--exact",
            "workspace_manager::tests::replacement_control_probe_helper",
            "--ignored",
            "--nocapture",
        ])
        .env(
            REPLACEMENT_CONTROL_PROBE,
            if leg == Leg::Neutralised {
                "pinned"
            } else {
                "live"
            },
        )
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if leg == Leg::Neutralised {
        // The operator's environment first, the neutralisation second: the
        // neutralisation is what this leg asserts wins.
        for (key, value) in row.controlled.iter().chain(&row.treatment) {
            command.env(key, value);
        }
        without_ambient_replacement_controls(&mut command);
    } else {
        // The controlled base first -- which is where this leg's baseline comes
        // from -- then the pins this row would otherwise be masked by, then the
        // row's own values.
        without_ambient_replacement_controls(&mut command);
        for key in row.lifted {
            command.env_remove(key);
        }
        for (key, value) in &row.controlled {
            command.env(key, value);
        }
        if leg == Leg::Treated {
            for (key, value) in &row.treatment {
                command.env(key, value);
            }
        }
    }
    command.status().expect("spawn the control probe")
}

/// A probe leg's exit status, said in a sentence a CI log's reader can use.
fn probe_outcome(status: std::process::ExitStatus) -> String {
    match status.code() {
        Some(0) => "replacements live".to_owned(),
        Some(code) if code == REPLACEMENT_DISABLED_EXIT => {
            "a Git child did not honour `refs/replace/*`".to_owned()
        }
        Some(code) => format!("the probe itself failed, exit {code}"),
        None => format!("the probe was killed by a signal ({status:?})"),
    }
}

/// Every name in the enumeration earns its place, and the measurement catches
/// what no name list reaches (PR #271, rounds 3 and 4).
///
/// The neutralisation is only *exercised* when the control it removes is
/// actually set, and CI sets none of them -- so without this grid a name could
/// be dropped from [`without_ambient_replacement_controls`] and nothing would
/// go red until a reviewer exported it, which is exactly how rounds 1 and 2
/// were found.
///
/// **Each row is run three times, and the first run is why the other two mean
/// anything** (round 4). [`Leg::Controlled`] establishes that the row's own
/// dimension is replacement-live before the treatment goes in;
/// [`Leg::Treated`] adds the treatment and nothing else, so a failure there is
/// the row's; [`Leg::Neutralised`] sets the hostile values as an operator would
/// and requires the neutralisation to win. A treated leg that ends in anything
/// but `0` or [`REPLACEMENT_DISABLED_EXIT`] is a **setup failure** and is
/// reported as one rather than counted as suppression -- a malformed
/// configuration file makes `git` exit `128`, and a grid that reads that as
/// evidence is a grid that passes for the wrong reason.
///
/// Every row is reported rather than the first failing one, because a grid that
/// stops at row 3 tells a CI run's reader nothing about rows 4 to 12.
///
/// `HOME`, `XDG_CONFIG_HOME` and `GIT_TEMPLATE_DIR` are in the grid and in
/// **neither** name list: the first two are closed by pinning
/// `GIT_CONFIG_GLOBAL`, the third by a name of its own in
/// `REPLACEMENT_CONTROLS_REMOVED`, and their treated legs fail in the
/// measurement rather than in the name check. That is the half of the closure
/// that does not depend on the enumeration being complete, and this is where it
/// is witnessed doing the work.
#[test]
fn the_neutraliser_defeats_every_ambient_control_it_enumerates() {
    let tree = scratch("replacement-controls");
    let root = tree.path();
    let rows = hostile_replacement_environments(root);
    assert_eq!(
        rows.len(),
        12,
        "one row per mechanism, plus the combination"
    );

    let mut wrong: Vec<String> = Vec::new();
    let mut inert: Vec<&str> = Vec::new();
    for row in &rows {
        let name = row.name;
        let controlled = run_replacement_control_probe(row, Leg::Controlled);
        let attributable = controlled.code() == Some(0);
        if !attributable {
            wrong.push(format!(
                "`{name}`: the controlled baseline for this row is not \
                 replacement-live -- {} -- so nothing the treatment does below \
                 is attributable to the treatment",
                probe_outcome(controlled)
            ));
        }

        let treated = run_replacement_control_probe(row, Leg::Treated);
        match treated.code() {
            Some(0) => {
                if row.reach == Reach::EveryGit && attributable {
                    wrong.push(format!(
                        "`{name}` is not a control at all on this Git: the probe \
                         passed with it set over a controlled baseline, so the \
                         neutralised leg above it proves nothing"
                    ));
                }
                inert.push(name);
            }
            Some(code) if code == REPLACEMENT_DISABLED_EXIT => {}
            _ => wrong.push(format!(
                "`{name}`: the treated leg neither observed a live read nor a \
                 suppressed one -- {} -- and a setup that never ran is not \
                 evidence that this row suppresses anything",
                probe_outcome(treated)
            )),
        }

        let neutralised = run_replacement_control_probe(row, Leg::Neutralised);
        if !neutralised.success() {
            wrong.push(format!(
                "`{name}` survived the neutralisation: {}",
                probe_outcome(neutralised)
            ));
        }
    }
    assert!(
        wrong.is_empty(),
        "the enumeration does not hold on this Git: {wrong:#?}"
    );
    assert!(
        inert.len() < rows.len(),
        "no row is a control on the Git running this suite, so this grid \
         measured nothing at all"
    );
}

/// `GIT_CONFIG` captures a fixture's own configuration write, and the
/// neutralisation takes it away before the write happens (PR #271, round 3).
///
/// It is the one enumerated name that is not a control over what a Git child
/// *reads* -- measured, a probe under it still honours `refs/replace/*`, and
/// git-config(1) says the variable has no effect on commands other than `git
/// config`. What it does is redirect that command's **write**, so
/// [`pin_replacement_refs_in`]'s pin would succeed, land in the operator's
/// file, and leave the repository saying nothing about the question the fixture
/// thought it had answered. That is why it is removed rather than tolerated,
/// and this is the measurement rather than the argument.
#[test]
fn a_redirected_git_config_cannot_capture_a_fixtures_own_pin() {
    let tree = scratch("git-config-redirect");
    let root = tree.path();
    let repo = root.join("repo");
    create_dir(&repo);
    // Neutralised, because `GIT_TEMPLATE_DIR` writes into a repository at
    // creation and this one's local file is the whole measurement below.
    let mut init = Command::new("git");
    init.arg("-C").arg(&repo).args(["init", "-q", "-b", "main"]);
    without_ambient_replacement_controls(&mut init);
    assert!(
        init.status().expect("run git").success(),
        "the repository this test measures in was never created"
    );
    let elsewhere = root.join("elsewhere.cfg");
    write_file(&elsewhere, b"");

    let pin = |neutralised: bool| {
        let mut command = Command::new("git");
        command
            .arg("-C")
            .arg(&repo)
            .args(["config", "core.useReplaceRefs", "true"])
            .env("GIT_CONFIG", &elsewhere);
        if neutralised {
            without_ambient_replacement_controls(&mut command);
        }
        assert!(
            command.status().expect("run git").success(),
            "the pin itself failed, so neither leg below measures where it landed"
        );
        // **The read-back is neutralised too, and its exit status is asserted**
        // (PR #271, round 4). It was not, and that was the same read-back bug
        // this pull request had already fixed in `gates.rs`: with an ambient
        // `GIT_CONFIG` pointing anywhere, `git config --local --get` exits
        // `129` on `only one config file at a time`, prints nothing, and an
        // observer that reads empty stdout as "the key is absent" reports a pin
        // that succeeded as a pin that never landed. Measured: this test exited
        // `101` under `GIT_CONFIG` at an empty file and `0` with it unset. A
        // silent observer is not a weaker observer, it is a wrong one, and the
        // rule is every observer's rather than the one a reviewer named.
        let mut read_back = Command::new("git");
        read_back
            .arg("-C")
            .arg(&repo)
            .args(["config", "--local", "--get", "core.useReplaceRefs"]);
        without_ambient_replacement_controls(&mut read_back);
        let read = read_back.output().expect("run git");
        // `--get` exits `1` when the key is absent, which is an answer about
        // the repository; anything else means the read never happened.
        assert!(
            matches!(read.status.code(), Some(0 | 1)),
            "the read-back did not answer about the repository: {:?}, {}",
            read.status.code(),
            String::from_utf8_lossy(&read.stderr)
        );
        (
            String::from_utf8_lossy(&read.stdout).trim().to_owned(),
            fs::read_to_string(&elsewhere).expect("the redirected file"),
        )
    };

    let (local, redirected) = pin(false);
    assert_eq!(
        local, "",
        "the repository's own config answered, so `GIT_CONFIG` did not capture \
         the write and this test measures nothing"
    );
    assert!(
        redirected.contains("useReplaceRefs"),
        "`GIT_CONFIG` did not take the write either: {redirected}"
    );

    write_file(&elsewhere, b"");
    let (local, redirected) = pin(true);
    assert_eq!(
        local, "true",
        "the pin did not reach the repository it names with `GIT_CONFIG` removed"
    );
    assert_eq!(
        redirected, "",
        "the pin still reached the operator's file: {redirected}"
    );
}

/// Quiescence answers about the tree the worktree holds, not about whatever
/// `git replace` points that tree at (PR #130, pass 3's P1).
///
/// **The work is in a child process, and the child is why this test exists in
/// this shape** (PR #271, round 1's blocking finding). `quiescence` reaches Git
/// through the free [`read_only_git`], whose child inherits this process's
/// environment; run in-process, the body below passes with the `.env` call
/// removed from [`read_only_git`] whenever the suite itself was started under
/// `GIT_NO_REPLACE_OBJECTS=1` -- measured, exit `0` and all six
/// replacement-focused tests green against unrepaired code. That is the
/// environment this pull request makes both runners supply to gates, so the
/// witness would have false-greened in upstroke's own CI from here on. The child
/// is spawned with the variable *removed*, so its verdict is about what
/// `read_only_git` installs and never about what the machine exported.
///
/// `Quiescence::HoldsTree` is answered by `diff-index --cached --quiet <tree>`,
/// and Git resolves `<tree>` through `refs/replace/*` like any other name:
/// measured on git 2.43, that exit code moves from 0 to 1 the moment a
/// replacement of the recorded tree is installed, and an untouched worktree
/// becomes a `TreeMismatch` that routes to forced removal and a fresh add.
///
/// Witnessed failing with the pair removed from `read_only_git`, once under
/// **each** ambient control the parent neutralises, every one of them exported
/// into that parent: a clean environment; `GIT_NO_REPLACE_OBJECTS=1`;
/// `GIT_CONFIG_COUNT` with `GIT_CONFIG_KEY_0`/`VALUE_0`; those two with no
/// count; `GIT_CONFIG_PARAMETERS`; `GIT_CONFIG_GLOBAL`; `GIT_CONFIG_SYSTEM`; a
/// `GIT_CONFIG_GLOBAL` reaching the same value through `include.path`;
/// `GIT_CONFIG`; a `HOME` carrying `~/.gitconfig`; an `XDG_CONFIG_HOME`
/// carrying `git/config`; and `GIT_REPLACE_REF_BASE`. All twelve exited `101`
/// on `Err(TreeMismatch { expected: "6640fb01...", difference: "1 path(s)
/// differ: b.txt" })` over a worktree nothing had written to -- `b.txt` being
/// what the seed tree the replacement points at does not carry. The same twelve
/// exit `0` against the repaired code.
#[test]
fn quiescence_holds_when_the_recorded_tree_carries_a_replacement() {
    let status =
        run_replacement_witness_child("workspace_manager::tests::quiescence_replacement_helper");
    assert!(
        status.success(),
        "the child witnesses quiescence over a replaced tree with every ambient \
         control over `refs/replace/*` taken away from it, and ended {status:?}"
    );
}

/// A full object id of the wrong type for its role is refused as what it is:
/// the message names the role's object type and what the repository peeled the
/// value to, and never says a full object id is not one (PR #130, pass 2).
///
/// A commit id offered as the `tree` peels to that commit's tree, so the
/// resolve-once check refuses it; the diagnostic at the reviewed head said it
/// "is not a full object id of this repository", which is false of a commit
/// id. Witnessed failing with the message's `{role.object_type()}` replaced by
/// the role's own name, which spells the parent's required type `parent`.
#[test]
fn a_full_id_of_the_wrong_object_type_is_refused_naming_the_type_its_role_requires() {
    let fixture = Fixture::created("wrong-object-type");
    let tree = git(&fixture.base, &["rev-parse", "HEAD^{tree}"]);
    let commit = fixture.head.clone();

    // A commit where a tree is required: it peels to that commit's tree.
    let error = fixture
        .manager
        .add_snapshot(
            &mut NoHooks,
            &SnapshotName::gates(0, 2, 1),
            &SnapshotInput::Tree {
                tree: oid(&commit),
                parent: oid(&commit),
            },
        )
        .expect_err("a commit is not a tree");
    let expected = Refusal::SnapshotInputResolvesElsewhere {
        role: SnapshotObject::Tree,
        value: commit.clone(),
        resolved: Some(tree.clone()),
        found_type: None,
    };
    assert!(
        matches!(&error, UpstrokeError::Refused { message } if *message == expected.to_string()),
        "expected `{expected}`, got `{error}`"
    );
    let message = expected.to_string();
    assert!(
        message.contains("does not name a tree of this repository"),
        "the message names the object type the role requires: {message}"
    );
    assert!(
        !message.contains("is not a full object id"),
        "and never calls a full object id something other than one: {message}"
    );

    // A tree where a commit is required cannot be peeled at all: Git exits
    // non-zero with "expected commit type" on stderr rather than the silent
    // exit 1 the lookup reads as absence (measured, git 2.43). That is still
    // the caller offering the wrong kind of object, so it is the same refusal
    // and not a Git error, and it names the type the value does name.
    let error = fixture
        .manager
        .add_snapshot(
            &mut NoHooks,
            &SnapshotName::gates(0, 2, 2),
            &SnapshotInput::Tree {
                tree: oid(&tree),
                parent: oid(&tree),
            },
        )
        .expect_err("a tree is not a commit");
    let expected = Refusal::SnapshotInputResolvesElsewhere {
        role: SnapshotObject::Parent,
        value: tree.clone(),
        resolved: None,
        found_type: Some("tree".to_owned()),
    };
    assert!(
        matches!(&error, UpstrokeError::Refused { message } if *message == expected.to_string()),
        "an unpeelable type is this refusal, not a Git error: expected `{expected}`, got \
         `{error}`"
    );
    let message = expected.to_string();
    assert!(
        message.contains("does not name a commit of this repository")
            && message.contains("(it names a tree)"),
        "the parent role requires a commit, and the message says what the value is: {message}"
    );

    // An id of the repository's own format naming no object at all keeps the
    // refusal too, with nothing to say about its type: the repository will not
    // name one.
    let absent = "f".repeat(fixture.head.len());
    let error = fixture
        .manager
        .add_snapshot(
            &mut NoHooks,
            &SnapshotName::gates(0, 2, 3),
            &SnapshotInput::Commit(oid(&absent)),
        )
        .expect_err("no such object");
    let expected = Refusal::SnapshotInputResolvesElsewhere {
        role: SnapshotObject::Commit,
        value: absent,
        resolved: None,
        found_type: None,
    };
    assert!(
        matches!(&error, UpstrokeError::Refused { message } if *message == expected.to_string()),
        "expected `{expected}`, got `{error}`"
    );
    assert!(
        !expected.to_string().contains("it names a"),
        "and claims no type for an object the repository does not have: {expected}"
    );
}

// -----------------------------------------------------------------------
// Worktree.Verify and forced removal
// -----------------------------------------------------------------------

/// Every loose object in `objects/??/`, sorted.
///
/// Loose rather than `fsck`-reachable on purpose: a tree `write-tree`
/// creates is referenced by nothing, so a reachability oracle would not see
/// it, and the thing `identity` forbids is the *write*, not the reference.
fn loose_objects(common_git_dir: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let objects = common_git_dir.join("objects");
    let Ok(fanout) = fs::read_dir(&objects) else {
        return found;
    };
    for entry in fanout.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.len() != 2 || !name.chars().all(|c| c.is_ascii_hexdigit()) {
            continue;
        }
        if let Ok(inner) = fs::read_dir(entry.path()) {
            for object in inner.flatten() {
                found.push(format!("{name}{}", object.file_name().to_string_lossy()));
            }
        }
    }
    found.sort();
    found
}

/// `Worktree.Verify` writes **nothing** — no object, and not the index
/// (`PR5-CONF-002`).
///
/// `identity` says "Worktree.Verify is a read-only quiescence observation
/// (no effect)" and `WorktreeSite::Verify::is_read_only()` is frozen at
/// `true`. The implementation ran `git write-tree`, whose own comment
/// claimed it "creates no object that is not already implied by the index it
/// reads" — and measured against git 2.43.0, an index carrying staged
/// content whose tree object was never written gains **two loose objects**,
/// with the index rewritten 104 → 165 bytes as the `TREE` cache-tree
/// extension is added. A second `git write-tree` inserted into `quiescence`
/// survived the whole suite, because nothing observed the object store or
/// the index around Verify.
///
/// The two axes this crosses are the *verdict* and the *state of the object
/// store*. Every other Verify test holds the store constant — it calls
/// `write-tree` in the fixture first, which leaves a valid cache-tree and
/// every tree already present, the one state in which `write-tree` writes
/// nothing. What varies here is the state: the reachable prefix
/// `Object.CandidateStage` leaves *before* `Object.CandidateWriteTree` runs.
/// Both verdicts are driven in it, so a repair that were read-only only on
/// the failing path would fail here.
#[test]
fn verify_writes_no_object_and_does_not_rewrite_the_index() {
    let fixture = Fixture::created("verify-readonly");
    let slot = fixture.add_task(&mut NoHooks, "alpha", 1);
    let path = fixture.manager.slot_path(&slot);
    let common = PathBuf::from(git(
        &path,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    ));
    let index = git_dir_of(&path)
        .expect("git dir")
        .expect("a linked worktree")
        .join("index");

    // ---- The mismatch verdict, in the state where write-tree writes ----
    //
    // The reachable prefix: content staged into the index, and no tree
    // object written for it. `git add` writes the blob and invalidates the
    // cache-tree; the trees are what `Object.CandidateWriteTree` would add.
    // The recorded tree is an *older* one that really is in the store —
    // which is the production shape, since it was written by an earlier
    // `Object.CandidateWriteTree`.
    fs::write(path.join("staged.txt"), "staged\n").expect("stage a file");
    fs::create_dir_all(path.join("nested")).expect("a subdirectory");
    fs::write(path.join("nested/deep.txt"), "deep\n").expect("stage a nested file");
    git(&path, &["add", "-A"]);
    let recorded = git(&path, &["rev-parse", "HEAD^{tree}"]);

    let before_objects = loose_objects(&common);
    let before_index = fs::read(&index).expect("the index");
    let mismatch = fixture
        .manager
        .verify_worktree(
            &mut NoHooks,
            &slot,
            &Quiescence::HoldsTree(recorded.clone()),
        )
        .expect("verify");
    assert!(
        matches!(mismatch, Err(VerifyFailure::TreeMismatch { .. })),
        "staged content the recorded tree does not carry is a mismatch: {mismatch:?}"
    );
    assert_eq!(
        loose_objects(&common),
        before_objects,
        "Worktree.Verify created an object; `identity` calls it a read-only \
             observation with no effect"
    );
    assert_eq!(
        fs::read(&index).expect("the index"),
        before_index,
        "Worktree.Verify rewrote {} ({} bytes before); a read-only observation \
             does not update the index's cache-tree",
        index.display(),
        before_index.len()
    );

    // The premise, proved rather than asserted: this really is a state in
    // which `write-tree` writes. Run it, and watch the store grow. Without
    // this the two assertions above would pass just as well against the one
    // state — valid cache-tree, every tree present — in which the pre-repair
    // code was already read-only by accident.
    let held = git(&path, &["write-tree"]);
    let after_control = loose_objects(&common);
    assert!(
        after_control.len() > before_objects.len(),
        "the control: `git write-tree` here must create objects, or this test \
             measures nothing ({} then, {} now)",
        before_objects.len(),
        after_control.len()
    );
    assert!(
        after_control.contains(&held),
        "and one of them is the tree the index holds"
    );

    // ---- The holds-it verdict, in the state where write-tree rewrites ----
    //
    // A different discriminator, because a different half of the effect is
    // available: the trees now all exist, so `write-tree` would create no
    // object — but the cache-tree can be invalidated without changing what
    // the index *holds*, and then `write-tree` rewrites `.git/index` to put
    // it back. Measured on git 2.43.0: same tree id, 0 new objects, index
    // bytes changed.
    fs::write(path.join("staged.txt"), "other\n").expect("change the file");
    git(&path, &["add", "staged.txt"]);
    fs::write(path.join("staged.txt"), "staged\n").expect("change it back");
    git(&path, &["add", "staged.txt"]);

    let before_objects = loose_objects(&common);
    let before_index = fs::read(&index).expect("the index");
    let held_verdict = fixture
        .manager
        .verify_worktree(&mut NoHooks, &slot, &Quiescence::HoldsTree(held.clone()))
        .expect("verify");
    assert_eq!(
        held_verdict,
        Ok(()),
        "the worktree does hold {held}, and Verify must still say so"
    );
    assert_eq!(
        loose_objects(&common),
        before_objects,
        "Worktree.Verify created an object on the quiescent path"
    );
    assert_eq!(
        fs::read(&index).expect("the index"),
        before_index,
        "Worktree.Verify rewrote {} on the quiescent path: a read-only \
             observation does not restore the index's cache-tree",
        index.display()
    );

    // The second control: `write-tree` in this state writes no object and
    // rewrites the index anyway, so the assertion that bit is the index one.
    assert_eq!(
        git(&path, &["write-tree"]),
        held,
        "the index still holds the same tree"
    );
    assert_eq!(
        loose_objects(&common),
        before_objects,
        "the control: no object was available to create here"
    );
    assert_ne!(
        fs::read(&index).expect("the index"),
        before_index,
        "the control: `git write-tree` here must rewrite the index, or the \
             assertion above measures nothing"
    );
}

#[test]
fn worktree_verify_answers_every_non_quiescence_by_name() {
    let fixture = Fixture::created("verify");
    let slot = fixture.task("alpha", 1);
    let path = fixture.manager.slot_path(&slot);
    let at_head = Quiescence::AtBase(fixture.head.clone());

    assert_eq!(
        fixture.manager.quiescence(&path, &at_head).expect("verify"),
        Err(VerifyFailure::NotRegistered)
    );

    fixture.add_task(&mut NoHooks, "alpha", 1);
    assert_eq!(
        fixture.manager.quiescence(&path, &at_head).expect("verify"),
        Ok(()),
        "a fresh detached worktree at the recorded base is quiescent"
    );

    // HEAD elsewhere.
    assert!(matches!(
        fixture
            .manager
            .quiescence(&path, &Quiescence::AtBase(fixture.seed.clone()))
            .expect("verify"),
        Err(VerifyFailure::HeadMismatch { .. })
    ));

    // The retained cumulative tree.
    let tree = git(&path, &["write-tree"]);
    assert_eq!(
        fixture
            .manager
            .quiescence(&path, &Quiescence::HoldsTree(tree))
            .expect("verify"),
        Ok(())
    );
    assert!(matches!(
        fixture
            .manager
            .quiescence(&path, &Quiescence::HoldsTree("0".repeat(40)))
            .expect("verify"),
        Err(VerifyFailure::TreeMismatch { .. })
    ));

    // Every administrative residue element, one at a time.
    let git_dir = git_dir_of(&path)
        .expect("git dir")
        .expect("linked worktree");
    for (name, element) in [
        ("index.lock", ResidueElement::IndexLock),
        ("CHERRY_PICK_HEAD", ResidueElement::CherryPickHead),
        ("MERGE_HEAD", ResidueElement::MergeHead),
        ("MERGE_MSG", ResidueElement::MergeMsg),
        // The held form of each state file: a writer killed between creating
        // its lock and renaming it into place. It is the same command's state,
        // and the form that makes the next writer of that name fail outright.
        ("CHERRY_PICK_HEAD.lock", ResidueElement::CherryPickHead),
        ("MERGE_HEAD.lock", ResidueElement::MergeHead),
        ("MERGE_MSG.lock", ResidueElement::MergeMsg),
        ("REVERT_HEAD", ResidueElement::SequencerState),
        ("REVERT_HEAD.lock", ResidueElement::SequencerState),
    ] {
        fs::write(git_dir.join(name), "x\n").expect("plant residue");
        assert_eq!(
            fixture.manager.quiescence(&path, &at_head).expect("verify"),
            Err(VerifyFailure::Residue(element)),
            "{name} must make the worktree non-quiescent"
        );
        fs::remove_file(git_dir.join(name)).expect("clear residue");
    }
    fs::create_dir_all(git_dir.join("sequencer")).expect("plant sequencer state");
    assert_eq!(
        fixture.manager.quiescence(&path, &at_head).expect("verify"),
        Err(VerifyFailure::Residue(ResidueElement::SequencerState))
    );
    fs::remove_dir_all(git_dir.join("sequencer")).expect("clear");

    // A missing checkout.
    fs::remove_dir_all(&path).expect("remove the checkout");
    assert_eq!(
        fixture.manager.quiescence(&path, &at_head).expect("verify"),
        Err(VerifyFailure::Missing)
    );
}

#[test]
fn forced_removal_clears_every_administrative_residue_and_is_idempotent() {
    let fixture = Fixture::created("forced-removal");
    let slot = fixture.add_task(&mut NoHooks, "alpha", 1);
    let path = fixture.manager.slot_path(&slot);
    let git_dir = git_dir_of(&path)
        .expect("git dir")
        .expect("linked worktree");
    // The element list is `ResidueElement::ALL` — PR3's, frozen — not an
    // array written here. The hand-written array this replaced named six
    // of the seven filesystem elements and omitted the `locked` marker of
    // a registered-but-unpopulated worktree, so the test's own name
    // ("every administrative residue") overclaimed: `git worktree prune`
    // *skips* a locked entry, which is why `remove_worktree` clears it, and
    // deleting that clearing left this test green. Measured as a surviving
    // mutation against this test alone.
    let mut planted = 0;
    for element in ResidueElement::ALL {
        match element {
            ResidueElement::IndexLock => {
                fs::write(git_dir.join("index.lock"), "x\n").expect("plant");
            }
            ResidueElement::CherryPickHead => {
                fs::write(git_dir.join("CHERRY_PICK_HEAD"), "x\n").expect("plant");
            }
            ResidueElement::MergeHead => {
                fs::write(git_dir.join("MERGE_HEAD"), "x\n").expect("plant");
            }
            ResidueElement::MergeMsg => {
                fs::write(git_dir.join("MERGE_MSG"), "x\n").expect("plant");
            }
            ResidueElement::OrigHead => {
                fs::write(git_dir.join("ORIG_HEAD"), "x\n").expect("plant");
            }
            ResidueElement::SequencerState => {
                fs::create_dir_all(git_dir.join("sequencer")).expect("plant");
            }
            ResidueElement::RegisteredUnpopulatedWorktree => {
                // Git holds this for the whole of an interrupted `add`, and
                // it is the one element that *blocks* the reclaim path.
                fs::write(git_dir.join("locked"), "initializing\n").expect("plant");
            }
            // Not administrative residue in a git dir: objects are R27 and
            // leave with Git, never with the worktree.
            ResidueElement::UnreferencedObject | ResidueElement::TemporaryObjectFile => {
                continue;
            }
        }
        planted += 1;
    }
    assert_eq!(
        planted,
        ResidueElement::ALL.len() - 2,
        "every element of the frozen enum except the two object classes is planted"
    );
    assert_eq!(planted, 7, "seven administrative elements");

    fixture
        .manager
        .remove_worktree(&mut NoHooks, &slot)
        .expect("forced removal succeeds over administrative residue");
    assert!(!path.exists());
    assert!(!git_dir.exists(), "the residue left with the worktree");
    assert!(
        !fixture
            .manager
            .worktree_records()
            .expect("records")
            .iter()
            .any(|record| record.path().ends_with("kalpha-g1"))
    );

    fixture
        .manager
        .remove_worktree(&mut NoHooks, &slot)
        .expect("and is idempotent");
}

// -----------------------------------------------------------------------
// Byte-safe changed paths
// -----------------------------------------------------------------------

/// One `-z --name-status` record: a status field and its path fields.
fn status_record(status: &[u8], paths: &[&[u8]]) -> Vec<u8> {
    let mut bytes = status.to_vec();
    bytes.push(0);
    for path in paths {
        bytes.extend_from_slice(path);
        bytes.push(0);
    }
    bytes
}

#[test]
fn changed_paths_decode_byte_wise_and_one_undecodable_path_is_repo_wide() {
    // Hostile, and hostile in independent directions: order, case,
    // separators inside a name, a multi-byte name, a name that is longer
    // than any plausible buffer, and a leading-dot name. The status letters
    // vary independently of the paths, so a decoder that ignored the status
    // field and one that mis-read it are different observations.
    let hostile: &[(&[u8], &[u8])] = &[
        (b"M", b"src/Zebra/UBER.rs"),
        (b"A", b"a b/c\td.rs"),
        (b"D", b".hidden"),
        (b"T", "docs/\u{fc}nicode.md".as_bytes()),
        (
            b"M",
            b"a/very/deep/directory/chain/that/keeps/going/well/past/any/plausible/buffer/size/f.rs",
        ),
        (b"A", b"build.rs"),
    ];
    let mut bytes = Vec::new();
    for (status, path) in hostile {
        bytes.extend_from_slice(&status_record(status, &[path]));
    }
    let decoded = decode_changed_paths(&bytes);
    let paths = decoded.prefixes().expect("every path decoded").to_vec();
    assert_eq!(
        paths.len(),
        hostile.len(),
        "one entry per path, and the count is what says so"
    );
    assert_eq!(
        paths.iter().map(GitPath::as_str).collect::<Vec<_>>().len(),
        paths
            .iter()
            .map(GitPath::as_str)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        "and they are distinct"
    );
    for (_, path) in hostile {
        let expected = std::str::from_utf8(path).expect("fixture is UTF-8");
        assert!(
            paths.iter().any(|seen| seen.as_str() == expected),
            "`{expected}` survived the round trip"
        );
    }

    // One undecodable path makes the whole answer repo-wide, not a
    // silently shorter list.
    let mut poisoned = bytes.clone();
    poisoned.extend_from_slice(&status_record(b"M", &[b"bad/\xff\xfe.rs"]));
    assert!(
        decode_changed_paths(&poisoned).is_repo_wide(),
        "an undecodable path is never dropped: the region becomes repo-wide"
    );
    assert!(
        decode_changed_paths(b"")
            .prefixes()
            .expect("empty")
            .is_empty()
    );
}

/// **Both** endpoints of a detected rename reach the region.
///
/// `path_policy.actual` is "`--name-status` … both rename endpoints", and
/// the old endpoint is the one another owner may hold a lease on: an answer
/// that carries only the destination lets two overlapping edits be admitted
/// at once (`PR5-CORRECTNESS-005`). Copies carry two endpoints for the same
/// reason and are decoded the same way.
///
/// The expected paths are written here, not derived from the record, and
/// the record is written to the grammar in Git's own documentation rather
/// than produced by this decoder's inverse.
#[test]
fn both_endpoints_of_a_rename_or_copy_record_reach_the_region() {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&status_record(
        b"R100",
        &[b"src/auth.rs", b"archive/auth.rs"],
    ));
    bytes.extend_from_slice(&status_record(b"C75", &[b"src/lib.rs", b"src/copy.rs"]));
    bytes.extend_from_slice(&status_record(b"A", &[b"src/added.rs"]));
    bytes.extend_from_slice(&status_record(b"D", &[b"src/gone.rs"]));

    let decoded = decode_changed_paths(&bytes);
    let paths: Vec<&str> = decoded
        .prefixes()
        .expect("decoded")
        .iter()
        .map(GitPath::as_str)
        .collect();
    assert_eq!(
        paths,
        vec![
            "archive/auth.rs",
            "src/added.rs",
            "src/auth.rs",
            "src/copy.rs",
            "src/gone.rs",
            "src/lib.rs",
        ],
        "six endpoints from four records: a rename and a copy carry two each"
    );
}

/// A status field this grammar does not recognise makes the region
/// repo-wide rather than shorter.
///
/// `prediction` classifies "unsafe or unparsable forms" as repo-wide, and
/// repo-wide overlaps everything — so the unparsable direction refuses
/// rather than admits. The most important cell is the first: it is exactly
/// what this decoder sees if the invocation ever reverts to `--name-only`,
/// so that regression cannot produce a plausible short answer.
#[test]
fn an_unparsable_status_record_is_repo_wide_and_never_shorter() {
    let cases: &[(&str, Vec<u8>)] = &[
        ("--name-only output, where a path arrives as a status", {
            let mut bytes = Vec::new();
            for path in [b"archive/auth.rs".as_slice(), b"src/added.rs".as_slice()] {
                bytes.extend_from_slice(path);
                bytes.push(0);
            }
            bytes
        }),
        (
            "a rename record with only one endpoint",
            status_record(b"R100", &[b"src/auth.rs"]),
        ),
        (
            "a status letter that is not one of Git's",
            status_record(b"Z", &[b"src/auth.rs"]),
        ),
        (
            "a single-endpoint letter carrying a score",
            status_record(b"M50", &[b"src/auth.rs"]),
        ),
        (
            "a rename letter carrying no score",
            status_record(b"R", &[b"src/auth.rs", b"archive/auth.rs"]),
        ),
        (
            "a rename score that is not a number",
            status_record(b"Rxx", &[b"src/auth.rs", b"archive/auth.rs"]),
        ),
        (
            "a status field that does not decode",
            status_record(b"\xff", &[b"src/auth.rs"]),
        ),
    ];
    for (name, bytes) in cases {
        assert!(
            decode_changed_paths(bytes).is_repo_wide(),
            "{name}: an unparsable record must be repo-wide, not a shorter list"
        );
    }
    assert_eq!(cases.len(), 7, "seven independent unparsable shapes");
}

/// A Git child that **fails** inside a funnel body records `Before` only
/// (`PR5-WORKSPACE-047`).
///
/// `effect_site_inventory.identity`: "each Object site has exactly the
/// parent-executed hook phases `Before` (no object) and `After` (object
/// present and referenced as `row()` states…)". Every failure path the
/// suite drove refused *before* the funnel was entered — slot-name
/// refusals, `AddWithoutIntent`, symbolic-ref, malformed-oid, containment —
/// so no hooks fired at all, and the harness's own `Injection::Error` is
/// applied at a phase rather than to the primitive. The state the sentence
/// is about was therefore never built: `Before` recorded, the primitive
/// failed, `After` not claimed. A funnel that claimed `After` from an
/// unconditional cleanup guard would say an object is present and
/// referenced when the child that would have written it exited non-zero.
///
/// Both funnel shapes are driven: the shared `funnel()` helper, and the
/// hand-rolled `commit_tree` sequence that also carries `IdUnread`.
#[test]
fn a_git_child_that_fails_inside_a_funnel_records_before_and_never_claims_after() {
    /// One failing drive: it runs a Git child that exits non-zero inside a
    /// funnel body and answers whether the call really failed.
    type FailingDrive = Box<dyn Fn(&mut dyn EffectHooks) -> bool>;
    let cases: Vec<(&str, EffectSiteId, FailingDrive)> = vec![
        (
            "the shared funnel (Object.ProposalCherryPick)",
            EffectSiteId::Object(ObjectSite::ProposalCherryPick),
            Box::new(|hooks: &mut dyn EffectHooks| {
                let fixture = Fixture::created("funnel-failure-cherry");
                let slot = fixture.add_task(&mut NoHooks, "alpha", 1);
                fixture
                    .manager
                    .proposal_cherry_pick(hooks, &slot, ABSENT_COMMIT)
                    .is_err()
            }),
        ),
        (
            "the commit-tree sequence (Object.CandidateCommitTree)",
            EffectSiteId::Object(ObjectSite::CandidateCommitTree),
            Box::new(|hooks: &mut dyn EffectHooks| {
                let fixture = Fixture::created("funnel-failure-commit");
                fixture
                    .manager
                    .candidate_commit_tree(
                        hooks,
                        ABSENT_COMMIT,
                        &fixture.head,
                        "upstroke: candidate",
                    )
                    .is_err()
            }),
        ),
    ];

    for (what, site, drive) in cases {
        let (mut hooks, shared) = harness();
        assert!(
            drive(&mut hooks),
            "{what}: the child was supposed to fail and did not"
        );
        let harness = shared.lock().expect("harness");
        assert_eq!(
            harness.count(site, HookPhase::Before),
            1,
            "{what}: the funnel was entered, so Before fired once"
        );
        assert_eq!(
            harness.count(site, HookPhase::After),
            0,
            "{what}: the primitive failed, so there is no object present and referenced                  for After to be claiming"
        );
    }
}

/// A well-formed SHA-1 object id of an object no fixture has: it passes every
/// argument check and then makes the Git child itself fail, which is the only
/// way into a funnel body's failure path.
const ABSENT_COMMIT: &str = "0000000000000000000000000000000000000001";

/// Two generations of one task key are two different worktrees
/// (`PR5-WORKSPACE-010`).
///
/// `manager`: "detached linked worktrees with durable synced intents
/// (`tasks/k<key>-g<gen>`, `merge/s<seq>`)". Every Task slot in this file
/// is built at a single generation, so the two paths that would collide
/// were never both constructed and dropping `-g<generation>` from
/// `relative()` was invisible. `intent_name` still carried the generation,
/// and it is `intent_name` the round-trip tests exercise — so the
/// injectivity they prove is the file name's, not the worktree path's.
#[test]
fn two_generations_of_one_task_key_are_two_worktrees() {
    let fixture = Fixture::created("generations");
    let first = fixture.task("alpha", 0);
    let second = fixture.task("alpha", 1);

    assert_ne!(
        fixture.manager.slot_path(&first),
        fixture.manager.slot_path(&second),
        "one key at two generations must not name one directory"
    );
    assert!(
        first.relative().ends_with("tasks/kalpha-g0"),
        "the packet spells it tasks/k<key>-g<gen>: {}",
        first.relative().display()
    );
    assert!(
        second.relative().ends_with("tasks/kalpha-g1"),
        "{}",
        second.relative().display()
    );

    // And both really exist at once, which is the state a collision
    // destroys: the second add would land in the first's checkout.
    fixture.add_task(&mut NoHooks, "alpha", 0);
    fixture.add_task(&mut NoHooks, "alpha", 1);
    for slot in [&first, &second] {
        assert_eq!(
            fixture
                .manager
                .quiescence(
                    &fixture.manager.slot_path(slot),
                    &Quiescence::AtBase(fixture.head.clone())
                )
                .expect("verify"),
            Ok(()),
            "{slot:?} is its own quiescent worktree"
        );
    }
    assert_eq!(
        fixture
            .manager
            .worktree_records()
            .expect("records")
            .iter()
            .filter(|record| record.path().starts_with(fixture.manager.execution_root()))
            .count(),
        2,
        "two registrations, not one directory registered twice"
    );
}

/// A task worktree is **detached** even when the commit-ish is a branch
/// name (`PR5-WORKSPACE-012`).
///
/// `git worktree add <path> <sha>` detaches HEAD with or without
/// `--detach`, and every fixture in this file passes a raw 40-hex id — so
/// the flag was behaviour-neutral on everything the suite built, and
/// nothing ever read HEAD's attachment state after an add or enumerated the
/// refs an add created. A branch name is the one commit-ish where the flag
/// decides, and without it `git worktree add` checks the branch out **and
/// locks it to that worktree**, which is the state `integration_ref`'s
/// "never checked out" forbids for the run namespace.
#[test]
fn a_task_worktree_is_detached_even_when_the_base_names_a_branch() {
    let fixture = Fixture::created("detached");
    let slot = fixture.task("alpha", 1);
    let before: BTreeSet<String> = fixture
        .manager
        .refs_under("refs/heads")
        .expect("refs")
        .into_iter()
        .map(|(name, _)| name)
        .collect();

    fixture
        .manager
        .write_intent(&mut NoHooks, &slot)
        .expect("intent");
    fixture
        .manager
        .add_worktree(&mut NoHooks, &slot, "side")
        .expect("add at a branch NAME, not a sha");

    let path = fixture.manager.slot_path(&slot);
    assert_eq!(
        git(&path, &["rev-parse", "HEAD"]),
        fixture.side,
        "the worktree is at the branch's commit"
    );
    assert_eq!(
        git(&path, &["rev-parse", "--symbolic-full-name", "HEAD"]),
        "HEAD",
        "and its HEAD is detached rather than pointing at refs/heads/side"
    );
    let after: BTreeSet<String> = fixture
        .manager
        .refs_under("refs/heads")
        .expect("refs")
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    assert_eq!(after, before, "the add created and moved no branch ref");
}

/// A clean worktree of a **different repository** at the recorded path
/// fails verification (`PR5-WORKSPACE-019`).
///
/// `generation`: "`Worktree.Verify`: the recorded path is a linked worktree
/// of **this** repository". `worktree_verify_answers_every_non_quiescence_
/// by_name` drives every other failure by name and never this one — no
/// fixture built a second repository — so the identity half of the sentence
/// was unobserved and deleting the common-git-dir comparison changed
/// nothing. The foreign worktree holds the **same commit object**, so a
/// verifier that only compared HEAD would still pass it.
#[test]
fn a_worktree_of_another_repository_at_the_recorded_path_is_not_this_ones() {
    let fixture = Fixture::created("foreign-repo");
    let slot = fixture.add_task(&mut NoHooks, "alpha", 1);
    let path = fixture.manager.slot_path(&slot);

    // A second repository holding the very same commit object, so identity
    // is the only thing that separates its checkout from the real one.
    let foreign = fixture.root.join("foreign");
    fs::create_dir_all(&foreign).expect("foreign repo");
    git(&foreign, &["init", "-q", "-b", "main"]);
    git(&foreign, &["config", "user.email", "tests@upstroke.local"]);
    git(&foreign, &["config", "user.name", "upstroke tests"]);
    git(
        &foreign,
        &["fetch", "-q", &fixture.base.to_string_lossy(), "main"],
    );
    let fetched = git(&foreign, &["rev-parse", "FETCH_HEAD"]);
    assert_eq!(
        fetched, fixture.head,
        "the foreign repository holds the identical commit object"
    );

    // The recorded path stays registered in **this** repository — a
    // verifier that stopped at "is it registered here" must still be
    // reached — while the checkout sitting there belongs to the other one.
    let theirs = fixture.root.join("theirs");
    git(
        &foreign,
        &[
            "worktree",
            "add",
            "-q",
            "--detach",
            &theirs.to_string_lossy(),
            &fetched,
        ],
    );
    let foreign_gitfile = fs::read(theirs.join(".git")).expect("their .git file");
    fs::write(path.join(".git"), &foreign_gitfile).expect("point the checkout at their repo");

    assert!(
        fixture
            .manager
            .quiescence(&path, &Quiescence::AtBase(fixture.head.clone()))
            .expect("verify")
            != Err(VerifyFailure::NotRegistered),
        "the path is still registered here, so this is not the registration check"
    );
    assert_eq!(
        git(&path, &["rev-parse", "HEAD"]),
        fixture.head,
        "and a HEAD-only verifier would see exactly what it expects"
    );
    assert_eq!(
        fixture
            .manager
            .quiescence(&path, &Quiescence::AtBase(fixture.head.clone()))
            .expect("verify"),
        Err(VerifyFailure::ForeignRepository),
        "but it is another repository's worktree at this repository's recorded path"
    );
}

/// The root-level name the resolution manifest reserves, in every state a
/// task worktree can hold it in, and what the two readers of that state do.
///
/// `resolution_manifest` is the read a conflict repair makes, and
/// `candidate_stage` decides from the same reading whether to exclude the
/// name from its `add -A`, whether to stage what the index held under it
/// first, and whether to consume it afterwards; the index and the disk
/// afterwards say what it decided. The attempt-level tests drive each state
/// through a capture; this one pins the classification itself, the symbolic
/// link, the tracked and the untracked case variants, and the ignored
/// manifest standing where a tracked directory was included — and that the
/// worker's manifest, by whichever spelling the checkout lists it, never
/// outlives a staging.
#[test]
fn the_manifests_name_is_read_by_what_holds_it_and_excluded_only_when_it_is_the_workers() {
    let fixture = Fixture::created("manifest-name");
    let slot = fixture.add_task(&mut NoHooks, "alpha", 1);
    let path = fixture.manager.slot_path(&slot);
    let manifest = path.join(RESOLUTION_MANIFEST);
    let staged = || {
        git(
            &path,
            &[
                "ls-files",
                "--",
                &format!(":(top,literal,icase){RESOLUTION_MANIFEST}"),
            ],
        )
        .lines()
        .map(str::to_owned)
        .collect::<Vec<_>>()
    };
    let stage = || {
        fixture
            .manager
            .candidate_stage(&mut NoHooks, &slot, &[])
            .expect("stage")
    };
    let declared = |path: &str| {
        ResolutionManifest::Declared(vec![Declaration {
            path: path.to_owned(),
            kind: ResolutionKind::Resolved,
        }])
    };

    // Absent.
    assert_eq!(
        fixture.manager.resolution_manifest(&slot).expect("read"),
        ResolutionManifest::Absent
    );
    stage();
    assert!(staged().is_empty());

    // The worker's manifest: read; kept out of the index by the exclusion;
    // and removed by the staging, which does not know — and does not ask —
    // whether the capture read it. Twice, because the second write is the
    // shape PR #249's fifth-round reviews found kept: a manifest a capture
    // had no use for, standing until a later capture had.
    for _ in 0..2 {
        fs::write(&manifest, "resolved c.txt\n").expect("write");
        assert_eq!(
            fixture.manager.manifest_name(&path).expect("classify"),
            ManifestName::Manifest {
                spelling: RESOLUTION_MANIFEST.to_owned(),
                ignored: false,
                displaces_directory: false,
            }
        );
        assert_eq!(
            fixture.manager.resolution_manifest(&slot).expect("read"),
            declared("c.txt")
        );
        stage();
        assert!(
            staged().is_empty(),
            "an untracked manifest is excluded from the `add -A`"
        );
        assert!(
            !manifest.exists(),
            "and no staging leaves the worker's manifest behind: a declaration is applied once, \
             by the capture of the attempt that wrote it, and a manifest nothing governed is \
             removed unread"
        );
    }

    // The same, ignored: read all the same, kept out by the ignore rules, and
    // removed the same way (`clean -x`).
    fs::write(&manifest, "resolved c.txt\n").expect("write");
    fs::write(path.join(".gitignore"), format!("{RESOLUTION_MANIFEST}\n")).expect("ignore");
    assert_eq!(
        fixture.manager.manifest_name(&path).expect("classify"),
        ManifestName::Manifest {
            spelling: RESOLUTION_MANIFEST.to_owned(),
            ignored: true,
            displaces_directory: false,
        }
    );
    assert!(matches!(
        fixture.manager.resolution_manifest(&slot).expect("read"),
        ResolutionManifest::Declared(_)
    ));
    stage();
    assert!(staged().is_empty());
    assert_eq!(
        git(&path, &["ls-files", "--", ".gitignore"]),
        ".gitignore",
        "the `add -A` ran: the ignore file itself is staged"
    );
    assert!(!manifest.exists(), "`clean -x` reaches an ignored manifest");
    fs::remove_file(path.join(".gitignore")).expect("unignore");
    git(&path, &["rm", "--quiet", "--cached", "--", ".gitignore"]);

    // A symbolic link: not a regular file, so not the worker's manifest;
    // refused by the read, staged as the link it is by the `add -A`, and
    // left where it is — the removal is of the worker's manifest only.
    #[cfg(unix)]
    {
        fs::write(path.join("elsewhere.txt"), "resolved c.txt\n").expect("target");
        std::os::unix::fs::symlink("elsewhere.txt", &manifest).expect("link");
        let refused = fixture
            .manager
            .resolution_manifest(&slot)
            .expect_err("a link is not the manifest");
        assert!(
            matches!(&refused, UpstrokeError::Refused { message }
                if message.contains("is a symbolic link")),
            "{refused}"
        );
        stage();
        assert_eq!(staged(), vec![RESOLUTION_MANIFEST.to_owned()]);
        assert!(manifest.symlink_metadata().is_ok(), "the link stays");
        git(
            &path,
            &["rm", "--quiet", "--cached", "--", RESOLUTION_MANIFEST],
        );
        fs::remove_file(&manifest).expect("unlink");
        fs::remove_file(path.join("elsewhere.txt")).expect("target removed");
        git(&path, &["rm", "--quiet", "--cached", "--", "elsewhere.txt"]);
    }

    // A directory: the same refusal, its contents ordinary paths, and the
    // directory left where it is.
    fs::create_dir(&manifest).expect("directory");
    fs::write(manifest.join("data.txt"), "data\n").expect("data");
    let refused = fixture
        .manager
        .resolution_manifest(&slot)
        .expect_err("a directory is not the manifest");
    assert!(
        matches!(&refused, UpstrokeError::Refused { message } if message.contains("is a directory")),
        "{refused}"
    );
    stage();
    assert_eq!(
        staged(),
        vec![format!("{RESOLUTION_MANIFEST}/data.txt")],
        "a directory of the name holds ordinary paths, and they are staged"
    );
    assert!(manifest.is_dir());

    // A regular file standing where that directory was: the index still holds
    // the directory's file, the working tree holds the worker's manifest. The
    // read is the worker's; the staging stages the directory's deletion —
    // which the exclusion, a directory prefix too, would have kept out — and
    // the worker's file stays out and goes.
    fs::remove_dir_all(&manifest).expect("the worker removes the directory");
    fs::write(&manifest, "resolved c.txt\n").expect("and writes its manifest at the name");
    assert_eq!(
        fixture.manager.manifest_name(&path).expect("classify"),
        ManifestName::Manifest {
            spelling: RESOLUTION_MANIFEST.to_owned(),
            ignored: false,
            displaces_directory: true,
        }
    );
    assert!(
        matches!(
            fixture.manager.resolution_manifest(&slot).expect("read"),
            ResolutionManifest::Declared(_)
        ),
        "a regular file at the name is the worker's, whatever the index held under the name"
    );
    stage();
    assert!(
        staged().is_empty(),
        "the directory's file is gone from the index and the worker's file is not in it: {:?}",
        staged()
    );
    assert!(!manifest.exists());

    // The same transition with the manifest ignored (PR #249's fifth-round
    // regression review): the fourth round's `add -A` of what the index held
    // under the name collected the ignored file at the name as a path its
    // pathspec named, and exited 1 after staging the deletion, which aborted
    // the capture. `add -u` walks the index alone.
    fs::create_dir(&manifest).expect("the directory again");
    fs::write(manifest.join("data.txt"), "data\n").expect("data");
    git(
        &path,
        &["add", "--", &format!("{RESOLUTION_MANIFEST}/data.txt")],
    );
    fs::remove_dir_all(&manifest).expect("the worker removes the directory");
    fs::write(&manifest, "resolved c.txt\n").expect("and writes its manifest at the name");
    fs::write(path.join(".gitignore"), format!("{RESOLUTION_MANIFEST}\n")).expect("ignore");
    assert_eq!(
        fixture.manager.manifest_name(&path).expect("classify"),
        ManifestName::Manifest {
            spelling: RESOLUTION_MANIFEST.to_owned(),
            ignored: true,
            displaces_directory: true,
        }
    );
    fixture
        .manager
        .candidate_stage(&mut NoHooks, &slot, &[])
        .expect("an ignored manifest standing where a tracked directory was stages");
    assert!(
        staged().is_empty(),
        "the directory's deletion is staged and the ignored file is not: {:?}",
        staged()
    );
    assert_eq!(git(&path, &["ls-files", "--", ".gitignore"]), ".gitignore");
    assert!(!manifest.exists(), "and the ignored manifest is removed");
    fs::remove_file(path.join(".gitignore")).expect("unignore");
    git(&path, &["rm", "--quiet", "--cached", "--", ".gitignore"]);

    // Tracked, as spelt: the repository's file, whatever the working tree
    // holds there; refused by the read, its edit staged by the `add -A`, and
    // the file left where it is.
    fs::write(&manifest, "application data\n").expect("write");
    git(&path, &["add", "--", RESOLUTION_MANIFEST]);
    fs::write(&manifest, "resolved c.txt\n").expect("the worker overwrites it");
    assert_eq!(
        fixture.manager.manifest_name(&path).expect("classify"),
        ManifestName::Tracked {
            spelling: RESOLUTION_MANIFEST.to_owned(),
        }
    );
    let refused = fixture
        .manager
        .resolution_manifest(&slot)
        .expect_err("a tracked file is not the manifest");
    assert!(
        matches!(&refused, UpstrokeError::Refused { message }
            if message.contains("tracks `.upstroke-resolved`")),
        "{refused}"
    );
    stage();
    assert_eq!(
        git(&path, &["show", &format!(":{RESOLUTION_MANIFEST}")]),
        "resolved c.txt",
        "the edit to the repository's file is staged like any other"
    );
    assert!(manifest.is_file(), "the repository's file is not removed");
    git(
        &path,
        &["rm", "--quiet", "--cached", "--", RESOLUTION_MANIFEST],
    );
    fs::remove_file(&manifest).expect("remove");

    // Tracked in another case: the repository's on every platform, refused
    // naming the index's spelling. On a checkout that folds case the
    // lowercase name *is* that file (PR #249's fourth-round manifest-contract
    // review, natively on the Windows guest, where the exact-spelling reads
    // answered nothing and the repository's file was read as the manifest);
    // on one that does not, a file the worker writes there is a second file,
    // staged as one, because a name the index holds in any case is the
    // repository's and no exclusion is made for it.
    let upper = path.join(".UPSTROKE-RESOLVED");
    fs::write(&upper, "application data\n").expect("write");
    git(&path, &["add", "--", ".UPSTROKE-RESOLVED"]);
    let refused = fixture
        .manager
        .resolution_manifest(&slot)
        .expect_err("a tracked case variant is not the manifest");
    assert!(
        matches!(&refused, UpstrokeError::Refused { message }
            if message.contains("tracks `.UPSTROKE-RESOLVED`")
                && message.contains("by case alone")),
        "{refused}"
    );
    fs::write(&manifest, "resolved c.txt\n").expect("the worker writes the lowercase name");
    let refused = fixture
        .manager
        .resolution_manifest(&slot)
        .expect_err("still not the manifest");
    assert!(
        matches!(&refused, UpstrokeError::Refused { message }
            if message.contains("tracks `.UPSTROKE-RESOLVED`")),
        "{refused}"
    );
    stage();
    let folds_case = fs::read(&upper).expect("the tracked file") == b"resolved c.txt\n";
    assert_eq!(
        folds_case,
        cfg!(any(windows, target_os = "macos")),
        "whether the lowercase write landed in the tracked file is the checkout's answer, \
         recorded for the platform the suite runs on"
    );
    if folds_case {
        assert_eq!(
            git(&path, &["show", ":.UPSTROKE-RESOLVED"]),
            "resolved c.txt",
            "the worker's write was to the tracked file, staged like any other edit"
        );
        git(
            &path,
            &["rm", "--quiet", "--cached", "--", ".UPSTROKE-RESOLVED"],
        );
    } else {
        assert_eq!(
            staged(),
            vec![
                ".UPSTROKE-RESOLVED".to_owned(),
                RESOLUTION_MANIFEST.to_owned()
            ],
            "a second file, staged as one"
        );
        git(
            &path,
            &["rm", "--quiet", "--cached", "--", ".UPSTROKE-RESOLVED"],
        );
        git(
            &path,
            &["rm", "--quiet", "--cached", "--", RESOLUTION_MANIFEST],
        );
        fs::remove_file(&manifest).expect("remove the second file");
    }
    fs::remove_file(&upper).expect("remove the tracked variant");
    assert!(staged().is_empty() && !manifest.exists() && !upper.exists());

    // Untracked in another case (PR #249's fifth-round manifest-contract
    // review, natively on the Windows guest): the directory already holds an
    // untracked `.Upstroke-Resolved` when the worker writes its declaration
    // through the lowercase name. On a checkout that folds case the two names
    // are one file, listed by the directory's spelling, which the
    // exact-spelling reads at `6448262e` did not find: the worker's manifest
    // was classified ignored, staged into the candidate under that spelling
    // by the bare `add -A`, and left on disk. The spelling the checkout lists
    // is now the spelling the exclusion and the removal use; on a checkout
    // that does not fold case the variant is a second file, staged as one,
    // and the file spelt as written is the manifest.
    let variant = path.join(".Upstroke-Resolved");
    fs::write(&variant, "resolved c.txt\n").expect("the variant first");
    fs::write(&manifest, "resolved d.txt\n").expect("then the lowercase name");
    let folds_case = fs::read(&variant).expect("the variant") == b"resolved d.txt\n";
    assert_eq!(folds_case, cfg!(any(windows, target_os = "macos")));
    let spelling = if folds_case {
        ".Upstroke-Resolved"
    } else {
        RESOLUTION_MANIFEST
    };
    assert_eq!(
        fixture.manager.manifest_name(&path).expect("classify"),
        ManifestName::Manifest {
            spelling: spelling.to_owned(),
            ignored: false,
            displaces_directory: false,
        },
        "the worker's manifest is the file the checkout lists, by its spelling"
    );
    assert_eq!(
        fixture.manager.resolution_manifest(&slot).expect("read"),
        declared("d.txt"),
        "read by that spelling: the lowercase write's bytes"
    );
    stage();
    if folds_case {
        assert!(
            staged().is_empty(),
            "one file, excluded under the spelling the checkout lists: {:?}",
            staged()
        );
        assert!(
            !manifest.exists() && !variant.exists(),
            "and removed under that spelling"
        );
    } else {
        assert_eq!(
            staged(),
            vec![".Upstroke-Resolved".to_owned()],
            "the variant is a second file, staged as one; the manifest is excluded"
        );
        assert!(
            !manifest.exists() && variant.is_file(),
            "the manifest is removed and the second file is not"
        );
        git(
            &path,
            &["rm", "--quiet", "--cached", "--", ".Upstroke-Resolved"],
        );
        fs::remove_file(&variant).expect("remove the second file");
    }
    assert!(staged().is_empty());

    // The same variant, ignored: the ignore rules keep it out, no exclusion is
    // appended for it, and the removal reaches it under its spelling.
    fs::write(path.join(".gitignore"), format!("{RESOLUTION_MANIFEST}\n")).expect("ignore");
    fs::write(&variant, "resolved c.txt\n").expect("the variant first");
    fs::write(&manifest, "resolved d.txt\n").expect("then the lowercase name");
    // `check-ignore` exits 1 for a path the rules do not reach, so not the
    // panicking helper: whether `.upstroke-resolved` in `.gitignore` reaches
    // `.Upstroke-Resolved` is the checkout's answer (`core.ignorecase`).
    let ignored_variant = git_out(
        &path,
        &[
            "check-ignore",
            "--no-index",
            "-q",
            "--",
            ".Upstroke-Resolved",
        ],
    )
    .status
    .success();
    assert_eq!(
        fixture.manager.manifest_name(&path).expect("classify"),
        ManifestName::Manifest {
            spelling: spelling.to_owned(),
            ignored: if folds_case { ignored_variant } else { true },
            displaces_directory: false,
        },
        "whether the ignore rules reach the variant's spelling is the checkout's answer"
    );
    stage();
    assert!(
        staged().is_empty() || staged() == vec![".Upstroke-Resolved".to_owned()],
        "{:?}",
        staged()
    );
    assert!(
        !manifest.exists(),
        "removed under its spelling, `-x` reaching an ignored file"
    );
    if !folds_case {
        assert!(variant.is_file());
        if staged() == vec![".Upstroke-Resolved".to_owned()] {
            git(
                &path,
                &["rm", "--quiet", "--cached", "--", ".Upstroke-Resolved"],
            );
        }
        fs::remove_file(&variant).expect("remove the second file");
    }
    fs::remove_file(path.join(".gitignore")).expect("unignore");
    git(&path, &["rm", "--quiet", "--cached", "--", ".gitignore"]);
}

/// The one rule that picks the manifest's entry out of the index's records
/// and out of the directory's ([`name_spelling`]): the exact spelling when it
/// is listed, else the one that differs by ASCII case alone, else nothing —
/// and never an entry under the name. Pinned on every platform, because only a
/// checkout that folds case exercises the variant arm through a capture.
#[test]
fn the_manifests_spelling_is_the_exact_one_when_listed_and_the_case_variant_otherwise() {
    let name = RESOLUTION_MANIFEST.as_bytes();
    let exact: &[u8] = b".upstroke-resolved";
    let variant: &[u8] = b".Upstroke-Resolved";
    let upper: &[u8] = b".UPSTROKE-RESOLVED";
    let under: &[u8] = b".upstroke-resolved/data.txt";
    let under_variant: &[u8] = b".UPSTROKE-RESOLVED/data.txt";
    assert_eq!(name_spelling(&[], name), None);
    assert_eq!(name_spelling(&[under, under_variant], name), None);
    assert_eq!(name_spelling(&[variant], name), Some(variant));
    assert_eq!(name_spelling(&[under_variant, upper], name), Some(upper));
    assert_eq!(
        name_spelling(&[variant, exact], name),
        Some(exact),
        "exact first, whatever the order"
    );
    assert_eq!(name_spelling(&[exact, variant], name), Some(exact));
    assert_eq!(
        name_spelling(&[b".upstroke-resolved.bak", b".upstroke-resolve"], name),
        None
    );
}

/// PR #249's fourth-round regression review, finding 2: the read of the
/// paths the index still holds passed every recorded path as one argument,
/// and a valid index of 13,000 resolved conflicts of 180 characters each had
/// `Argument list too long (os error 7)` abort the capture before any gate,
/// while `add -A`, `write-tree` and the unfiltered reads all succeeded. The
/// index is built with `update-index --index-info` over stdin — fixture
/// setup, not a staging privilege the worker has — and the read is the
/// production one. The files are not checked out: the read consults the
/// index, 13,000 paths of that length are what Linux's argument limit
/// needs, and a name of that length exceeds Windows' path limit on disk
/// (measured on the guest: `checkout-index` failed "Filename too long").
#[test]
fn the_resolved_conflict_read_passes_no_recorded_path_as_an_argument() {
    use std::io::Write as _;

    let fixture = Fixture::created("resolved-read-argument-limit");
    let slot = fixture.add_task(&mut NoHooks, "alpha", 1);
    let path = fixture.manager.slot_path(&slot);
    let with_input = |args: &[&str], bytes: &[u8]| -> Vec<u8> {
        let mut child = std::process::Command::new("git")
            .arg("-C")
            .arg(&path)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("fixture git starts");
        child
            .stdin
            .take()
            .expect("stdin")
            .write_all(bytes)
            .expect("fixture input");
        let output = child.wait_with_output().expect("fixture git ends");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        output.stdout
    };
    let blobs: Vec<String> = ["ancestor\n", "ours\n", "theirs\n"]
        .iter()
        .map(|content| {
            String::from_utf8(with_input(
                &["hash-object", "-w", "--stdin"],
                content.as_bytes(),
            ))
            .expect("an object id")
            .trim()
            .to_owned()
        })
        .collect();
    let resolved_blob = blobs.get(1).expect("three blobs").clone();
    const COUNT: usize = 13_000;
    let paths: Vec<String> = (0..COUNT)
        .map(|i| format!("conflicts/{i:05}_{}.txt", "x".repeat(160)))
        .collect();
    let mut records = Vec::new();
    for name in &paths {
        for (stage, blob) in blobs.iter().enumerate() {
            records.extend_from_slice(format!("100644 {blob} {}\t{name}\0", stage + 1).as_bytes());
        }
    }
    with_input(&["update-index", "-z", "--index-info"], &records);
    records.clear();
    for name in &paths {
        records.extend_from_slice(format!("100644 {resolved_blob} 0\t{name}\0").as_bytes());
    }
    with_input(&["update-index", "-z", "--index-info"], &records);
    fixture
        .manager
        .candidate_write_tree(&mut NoHooks, &slot)
        .expect("`write-tree` handles this index");
    assert!(
        fixture
            .manager
            .unresolved_conflicts(&slot)
            .expect("the unmerged read")
            .is_empty()
    );
    let recorded = git_out(&path, &["ls-files", "--resolve-undo", "-z"]);
    assert!(recorded.status.success());
    assert_eq!(
        parsers::stage_record_paths(&recorded.stdout).len(),
        COUNT * 3
    );
    let read = fixture.manager.resolved_conflicts(&slot).expect(
        "the read of what the index resolved and still holds passes no path as an argument",
    );
    assert_eq!(read.len(), COUNT);
    assert_eq!(read.first(), paths.first());
}

/// The recorded base is honoured **after the worktree's HEAD has moved off
/// it** (`PR5-WORKSPACE-038`).
///
/// `path_policy.actual` specifies `git diff-tree -r -z -M --name-status
/// base tree` — a diff between two *recorded* values. Every other fixture
/// in this file leaves the worktree checked out at exactly the base it then
/// passes, so `diff --cached <base>` and a bare `diff --cached` name the
/// same diff and a primitive that had quietly stopped honouring its
/// argument was indistinguishable from one that honoured it. Nothing here
/// asserts a spelling; it moves the one variable the two readings disagree
/// about and checks the answer.
#[test]
fn changed_paths_honour_the_recorded_base_after_head_has_moved_off_it() {
    let fixture = Fixture::created("changed-paths-moved-head");
    let slot = fixture.add_task(&mut NoHooks, "alpha", 1);
    let path = fixture.manager.slot_path(&slot);
    fs::write(path.join("staged.rs"), "fn main() {}\n").expect("add");
    fixture
        .manager
        .candidate_stage(&mut NoHooks, &slot, &[])
        .expect("stage");

    // Move the worktree's HEAD to the seed, keeping the index. `head` is
    // still the recorded base; HEAD is now `seed`, one commit behind it.
    git(&path, &["reset", "-q", "--soft", &fixture.seed]);
    assert_eq!(
        git(&path, &["rev-parse", "HEAD"]),
        fixture.seed,
        "the worktree's HEAD really moved off the recorded base"
    );
    assert_ne!(fixture.seed, fixture.head, "the two must differ at all");

    let against_base: Vec<String> = fixture
        .manager
        .changed_paths(&slot, &fixture.head)
        .expect("capture")
        .prefixes()
        .expect("decoded")
        .iter()
        .map(|path| path.as_str().to_owned())
        .collect();
    assert_eq!(
        against_base,
        vec!["staged.rs".to_owned()],
        "the diff is against the recorded base, not against wherever HEAD is now"
    );

    // And the two readings really are different here, so the assertion
    // above is not passing for want of a distinction: `b.txt` arrived
    // between the seed and the base, so a HEAD-relative diff carries it.
    let head_relative = git(&path, &["diff", "--cached", "--name-only"]);
    let head_relative: Vec<&str> = head_relative.lines().collect();
    assert!(
        head_relative.contains(&"b.txt"),
        "the fixture does not separate the two readings: {head_relative:?}"
    );
}

/// Each commit-tree site commits onto the **recorded** parent after HEAD
/// has moved (`PR5-WORKSPACE-023`, `PR5-WORKSPACE-042`).
///
/// `snapshots` says "the snapshot funnel first creates an ephemeral commit
/// of that tree on **the recorded parent**", and `candidate` says
/// `parent_sha == base_sha`. Both were asserted against a base the
/// repository's HEAD already equalled, so `commit-tree <tree> -p <recorded>`
/// and a body that had re-read the world produced the same commit. The
/// manipulation is one line — move HEAD — and it is the only one that
/// separates a primitive that honours its argument from one that does not.
#[test]
fn the_commit_tree_sites_use_the_recorded_parent_and_not_current_head() {
    let fixture = Fixture::created("recorded-parent");
    let tree = git(&fixture.base, &["rev-parse", "HEAD^{tree}"]);
    let recorded = fixture.head.clone();

    git(
        &fixture.base,
        &["checkout", "-q", "--detach", &fixture.side],
    );
    assert_eq!(
        git(&fixture.base, &["rev-parse", "HEAD"]),
        fixture.side,
        "HEAD moved off the recorded parent"
    );
    assert_ne!(fixture.side, recorded);

    for (what, commit) in [
        (
            "snapshot",
            fixture
                .manager
                .snapshot_commit_tree(&mut NoHooks, &tree, &recorded)
                .expect("the ephemeral snapshot commit"),
        ),
        (
            "candidate",
            fixture
                .manager
                .candidate_commit_tree(&mut NoHooks, &tree, &recorded, "upstroke: candidate")
                .expect("the candidate commit"),
        ),
    ] {
        let parents = git(
            &fixture.base,
            &["rev-list", "--parents", "-n", "1", &commit],
        );
        let parents: Vec<&str> = parents.split_whitespace().skip(1).collect();
        assert_eq!(
            parents,
            vec![recorded.as_str()],
            "{what}: the sole parent is the recorded one, not current HEAD ({})",
            fixture.side
        );
        assert_eq!(
            git(&fixture.base, &["rev-parse", &format!("{commit}^{{tree}}")]),
            tree,
            "{what}: the tree is the supplied one"
        );
    }
}

/// An undecodable byte in a rename **source** makes the region repo-wide
/// (`PR5-WORKSPACE-036`).
///
/// `path_policy.actual`: "both rename endpoints; NUL-delimited bytes;
/// GitPath byte-safe; **undecodable -> repo-wide**". The lane had solid
/// coverage on each axis separately and never their intersection: every
/// rename fixture's four endpoints are valid UTF-8, and every undecodable
/// fixture plants its bad byte in a single-endpoint record. So the one
/// field a source-dropping decoder treats differently was never hostile,
/// and "both endpoints or repo-wide" could not be told from "the
/// destination, plus the source when it happens to decode" — which loses a
/// path another owner may hold a lease on, silently.
#[test]
fn an_undecodable_rename_source_makes_the_region_repo_wide() {
    // A rename whose DESTINATION is perfectly ordinary, so a decoder that
    // returns what it could read returns something plausible.
    let source_bad = status_record(b"R100", &[b"src/\xff\xfe.rs", b"archive/auth.rs"]);
    assert!(
        decode_changed_paths(&source_bad).is_repo_wide(),
        "an undecodable rename source is not a path that may be quietly dropped"
    );

    // The other endpoint, and a copy record, so this is the field rather
    // than the record kind.
    let destination_bad = status_record(b"R100", &[b"src/auth.rs", b"archive/\xff.rs"]);
    assert!(decode_changed_paths(&destination_bad).is_repo_wide());
    let copy_source_bad = status_record(b"C75", &[b"src/\xff.rs", b"copy/auth.rs"]);
    assert!(decode_changed_paths(&copy_source_bad).is_repo_wide());

    // And the same record with both endpoints decodable is NOT repo-wide,
    // so the assertions above are about the undecodable byte rather than
    // about rename records in general.
    let both_fine = status_record(b"R100", &[b"src/auth.rs", b"archive/auth.rs"]);
    let decoded = decode_changed_paths(&both_fine);
    assert!(!decoded.is_repo_wide());
    let paths: Vec<&str> = decoded
        .prefixes()
        .expect("decoded")
        .iter()
        .map(GitPath::as_str)
        .collect();
    assert_eq!(paths, vec!["archive/auth.rs", "src/auth.rs"]);
}

#[test]
fn changed_paths_come_from_the_index_of_the_recorded_worktree() {
    let fixture = Fixture::created("changed-paths");
    let slot = fixture.add_task(&mut NoHooks, "alpha", 1);
    let path = fixture.manager.slot_path(&slot);
    fs::write(path.join("a.txt"), "changed\n").expect("edit");
    fs::create_dir_all(path.join("nested")).expect("nested");
    fs::write(path.join("nested/new.rs"), "fn main() {}\n").expect("add");
    fixture
        .manager
        .candidate_stage(&mut NoHooks, &slot, &[])
        .expect("stage");

    let captured = fixture
        .manager
        .changed_paths(&slot, &fixture.head)
        .expect("capture");
    let paths: Vec<&str> = captured
        .prefixes()
        .expect("decoded")
        .iter()
        .map(GitPath::as_str)
        .collect();
    assert_eq!(paths, vec!["a.txt", "nested/new.rs"]);
}

/// The same claim against **real Git**, over the change kinds the previous
/// test does not contain.
///
/// `PR5-CORRECTNESS-005`: the shipped invocation was `--name-only`, and
/// rename detection is Git's default — so a staged rename produced the
/// destination alone and the source, which another owner may hold a lease
/// on, silently left the region. That coverage held "one modification and
/// one addition", the two kinds where every invocation agrees.
///
/// Four kinds here, and the expected list is written out rather than
/// derived: a rename (two endpoints), a deletion, an addition, and a
/// modification. The rename is made by moving the file on disk and staging
/// through the production funnel, so detection is Git's decision at diff
/// time and not something the fixture asserted into being — which is also
/// why the record is checked to really be an `R`.
#[test]
fn every_change_kind_reaches_the_region_including_both_rename_endpoints() {
    let fixture = Fixture::created("changed-paths-kinds");
    let slot = fixture.add_task(&mut NoHooks, "alpha", 1);
    let path = fixture.manager.slot_path(&slot);

    // A base inside the worktree holding one file of each kind's
    // pre-state, so all four kinds can be produced against one commit.
    fs::write(path.join("kept.txt"), "before\n").expect("kept");
    fs::write(path.join("doomed.txt"), "doomed\n").expect("doomed");
    fs::write(path.join("moved.txt"), "moved\n").expect("moved");
    git(&path, &["add", "-A"]);
    git(&path, &["commit", "-q", "-m", "the base for this diff"]);
    let base = git(&path, &["rev-parse", "HEAD"]);

    // moved.txt -> archive/moved.txt, byte-identical: 100% similarity.
    fs::create_dir_all(path.join("archive")).expect("archive dir");
    fs::rename(path.join("moved.txt"), path.join("archive/moved.txt")).expect("move");
    fs::remove_file(path.join("doomed.txt")).expect("delete");
    fs::write(path.join("added.rs"), "fn main() {}\n").expect("add");
    fs::write(path.join("kept.txt"), "after\n").expect("modify");

    fixture
        .manager
        .candidate_stage(&mut NoHooks, &slot, &[])
        .expect("stage");

    // Git really did detect a rename here, rather than reporting a delete
    // and an add — otherwise this fixture would pass under `--name-only`
    // too and would be witnessing nothing.
    let records = git(&path, &["diff", "--cached", "--name-status", "-M", &base]);
    assert!(
        records.contains("R100\tmoved.txt\tarchive/moved.txt"),
        "the fixture must contain a *detected* rename, or it tests nothing: {records}"
    );

    let captured = fixture
        .manager
        .changed_paths(&slot, &base)
        .expect("capture");
    let paths: Vec<&str> = captured
        .prefixes()
        .expect("decoded")
        .iter()
        .map(GitPath::as_str)
        .collect();
    assert_eq!(
        paths,
        vec![
            "added.rs",
            "archive/moved.txt",
            "doomed.txt",
            "kept.txt",
            "moved.txt",
        ],
        "both rename endpoints, the deletion, the addition and the modification"
    );
}

/// `candidate_diff` produces the text a reviewer judges, and until this sweep
/// **nothing in this suite asserted anything about it**: its one appearance in
/// this file is the hostile-slot-name grid above, where every call is expected
/// to fail, so a body returning `Err` for every slot passed this module's
/// whole suite. What caught that mutation crate-wide was
/// `engine::topology::recover`'s driver tests, fourteen of which went red —
/// two modules away, and for reasons that say nothing about the diff. §12 asks
/// for the lowest layer that can observe the failure, and the three claims
/// below are not observable there at all.
///
/// Three claims, one fixture, each with the mutation it is witnessed against:
///
/// * **The recorded objects, not the world.** The worktree's HEAD is moved off
///   the recorded parent first, exactly as
///   `changed_paths_honour_the_recorded_base_after_head_has_moved_off_it` moves
///   it, and the two readings are proved to differ before the answer is read.
///   Witnessed against a body that diffs from current HEAD: `b.txt` arrived
///   between the seed and the base, so that diff carries it and this one
///   must not.
/// * **`--binary`.** Witnessed against dropping it from
///   [`crate::workspace::REVIEW_DIFF_FLAGS`]: Git then says "Binary files …
///   differ", which is not a patch a reviewer or `classify::diff_failure` can
///   read.
/// * **Colour suppression.** The repository is configured `color.ui=always`,
///   which is exactly the operator config the flag list exists to defeat, and
///   `WorkspaceManager::command` sets no colour option of its own. The witness
///   is dropping **both** `-c color.ui=false` and `--no-color`, and it is
///   stated as a pair on purpose: dropping either alone leaves the other
///   suppressing colour, and a claim measured against a mutation that does not
///   bite is not a measurement. Witnessed against dropping the pair: the diff
///   comes back carrying escape codes.
///
/// `--no-ext-diff` and `--no-textconv` need an external program or a
/// `.gitattributes` filter to observe; they are a deferred row rather than
/// half a test.
///
/// Each mutation above was run against **master's whole crate suite**, which
/// passed all 1,903 tests under every one of them.
#[test]
fn the_candidate_diff_is_of_the_recorded_objects_and_survives_operator_diff_config() {
    let fixture = Fixture::created("candidate-diff");
    let slot = fixture.add_task(&mut NoHooks, "alpha", 1);
    let path = fixture.manager.slot_path(&slot);

    // The operator config the shared flag list is there to defeat.
    git(&fixture.base, &["config", "color.ui", "always"]);

    fs::write(path.join("a.txt"), "changed\n").expect("a text edit");
    // A NUL in the first bytes is what makes Git call a file binary.
    fs::write(path.join("bin.dat"), [0_u8, 1, 2, 0xff]).expect("a binary file");
    fixture
        .manager
        .candidate_stage(&mut NoHooks, &slot, &[])
        .expect("stage");
    let tree = fixture
        .manager
        .candidate_write_tree(&mut NoHooks, &slot)
        .expect("write-tree");

    // **Both** recorded arguments are moved off the world, because each has
    // its own way of being ignored.
    //
    // HEAD moves off the recorded parent: that separates a body honouring
    // `parent` from one re-reading HEAD.
    git(&path, &["reset", "-q", "--soft", &fixture.seed]);
    assert_eq!(
        git(&path, &["rev-parse", "HEAD"]),
        fixture.seed,
        "the worktree's HEAD really moved off the recorded base"
    );
    // And the **index** moves off the recorded tree (PR #136 pass 1,
    // finding 1). `reset --soft` deliberately leaves the index equal to the
    // tree just captured, so with only the step above a body that ignored
    // `tree` and ran `git diff --cached <parent> --` produced byte-identical
    // output and passed every assertion here. Staging one more file after the
    // capture is what makes the argument the only route to the expected
    // answer. The harm it stands for is `DESIGN.md` §14's exact-tree
    // guarantee: the index moves on after a capture, and a reviewer shown the
    // index is shown a tree the candidate does not commit.
    fs::write(path.join("after-capture.txt"), "staged after the capture\n")
        .expect("a file staged after the tree was captured");
    git(&path, &["add", "after-capture.txt"]);

    // The premises, asserted before the answer is read, so neither
    // discriminator can quietly stop discriminating.
    assert_ne!(
        git(&path, &["write-tree"]),
        tree,
        "the index must no longer hold the recorded tree, or `tree` is not the only \
         way to the expected diff"
    );
    let from_index = git(&path, &["diff", "--cached", "--name-only", &fixture.head]);
    assert!(
        from_index.contains("after-capture.txt"),
        "a body reading the index rather than the recorded tree would carry \
         after-capture.txt: {from_index}"
    );
    assert!(
        git(&path, &["diff", "--name-only", &fixture.seed, &tree]).contains("b.txt"),
        "and a body reading current HEAD rather than the recorded parent would carry b.txt"
    );

    let diff = fixture
        .manager
        .candidate_diff(&slot, &fixture.head, &tree)
        .expect("the diff of the recorded objects");

    assert!(
        !diff.contains("after-capture.txt"),
        "the diff is of the recorded tree, not of whatever the index holds now: {diff}"
    );
    assert!(
        !diff.contains("b.txt"),
        "the diff is against the recorded parent, not against wherever HEAD is now: {diff}"
    );
    assert!(
        diff.contains("a/a.txt") && diff.contains("+changed"),
        "and it carries the edit it was asked for: {diff}"
    );
    assert!(
        diff.contains("GIT binary patch"),
        "a binary file reaches the reviewer as a patch, not as \"Binary files … differ\": \
         {diff}"
    );
    assert!(
        !diff.contains('\u{1b}'),
        "a configured `color.ui=always` must not reach the text a gate reads: {diff:?}"
    );
}

#[cfg(unix)]
#[test]
fn a_repository_path_a_string_cannot_carry_makes_the_region_repo_wide() {
    use std::os::unix::ffi::OsStrExt as _;
    let fixture = Fixture::created("nonutf8-paths");
    let slot = fixture.add_task(&mut NoHooks, "alpha", 1);
    let path = fixture.manager.slot_path(&slot);
    let hostile = path.join(OsStr::from_bytes(b"bad-\xff\xfe.txt"));
    if fs::write(&hostile, "bytes\n").is_err() {
        // A filesystem that refuses the name cannot host the fixture; the
        // pure-byte case above still covers the decision.
        return;
    }
    fixture
        .manager
        .candidate_stage(&mut NoHooks, &slot, &[])
        .expect("stage");
    assert!(
        fixture
            .manager
            .changed_paths(&slot, &fixture.head)
            .expect("capture")
            .is_repo_wide(),
        "a path no string can carry makes the region repo-wide rather than shorter"
    );
}
