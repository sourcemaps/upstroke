// Allowlist placement: the funnel section of `effects/allowlist.toml`.
#![allow(clippy::disallowed_methods)]
#![forbid(clippy::disallowed_types, clippy::disallowed_macros)]

use super::*;

// -----------------------------------------------------------------------
// INV-17: the ref primitives
// -----------------------------------------------------------------------

#[test]
fn a_symbolic_ref_is_refused_without_touching_the_victim() {
    let fixture = Fixture::created("symbolic-ref");
    git(
        &fixture.base,
        &["symbolic-ref", "refs/upstroke/sym", "refs/heads/main"],
    );
    let before = git(&fixture.base, &["rev-parse", "refs/heads/main"]);

    for attempt in [
        fixture.manager.create_ref_zero_old(
            &mut NoHooks,
            RefSite::CreateCandidates,
            "refs/upstroke/sym",
            &fixture.head,
        ),
        fixture.manager.delete_ref_expected_old(
            &mut NoHooks,
            RefSite::DeleteCandidatesRef,
            "refs/upstroke/sym",
            &fixture.head,
        ),
        // The CAS arm, which `ref_rules` names beside the other two and
        // which this loop did not drive. `--no-deref` on all three
        // invocations is unreachable defence in depth *because* this guard
        // runs first, so the guard is the thing that has to be complete —
        // and it was covering two of the three primitives it protects.
        fixture.manager.compare_and_swap_ref(
            &mut NoHooks,
            RefSite::CompareAndSwapIntegration,
            "refs/upstroke/sym",
            &fixture.head,
            &fixture.seed,
        ),
    ] {
        let message = refusal_of(&attempt.expect_err("a symbolic ref refuses"));
        assert!(
            message.contains("symbolic ref") && message.contains("INV-17"),
            "the refusal must name its reason: {message}"
        );
    }
    assert_eq!(
        git(&fixture.base, &["rev-parse", "refs/heads/main"]),
        before,
        "and the victim is untouched"
    );
    assert_eq!(
        git(&fixture.base, &["symbolic-ref", "refs/upstroke/sym"]),
        "refs/heads/main",
        "and the symbolic ref itself is untouched"
    );
}

#[test]
fn a_checked_out_ref_is_refused_before_any_publication() {
    let fixture = Fixture::created("checked-out");
    let message = refusal_of(
        &fixture
            .manager
            .assert_publishable("refs/heads/main")
            .expect_err("a checked-out ref refuses"),
    );
    assert!(
        message.contains("checked out in the worktree"),
        "the refusal must name its reason: {message}"
    );
    fixture
        .manager
        .assert_publishable("refs/heads/upstroke/run-1")
        .expect("a ref no worktree has checked out is publishable");
}

/// A compare-and-swap honours the **caller's recorded** expected-old, not
/// a fresh reading of the ref (`PR5-WORKSPACE-030`).
///
/// `invariants[16].recovery`: "symbolic or **substituted** refs refuse".
/// The suite owns a "wrong expected-old refuses" assertion but drives it
/// through `delete_ref_expected_old`, never through the CAS; and the CAS's
/// two production callers both pass the true current value, so a body that
/// replaced the caller's recorded SHA with a fresh reread produced the
/// identical argument every time it ran. The distinguishing manipulation is
/// a **third** SHA substituted between the caller recording expected-old
/// and the swap — under it, a self-oracle sees its own reading as current
/// and overwrites another writer's value.
#[test]
fn a_compare_and_swap_refuses_a_ref_substituted_since_the_caller_recorded_it() {
    let fixture = Fixture::created("cas-substituted");
    let name = "refs/upstroke/runs/run-1/integration";
    fixture
        .manager
        .create_ref_zero_old(
            &mut NoHooks,
            RefSite::CreateIntegration,
            name,
            &fixture.head,
        )
        .expect("create");
    // What the caller recorded, before anyone else touched the ref.
    let recorded = fixture.head.clone();

    // A third value, from a writer this caller never saw.
    git(&fixture.base, &["update-ref", name, &fixture.side]);
    assert_eq!(
        fixture.manager.direct_ref_target(name).expect("read"),
        Some(fixture.side.clone()),
        "the ref really was substituted"
    );
    assert_ne!(recorded, fixture.side);

    let error = fixture
        .manager
        .compare_and_swap_ref(
            &mut NoHooks,
            RefSite::CompareAndSwapIntegration,
            name,
            &recorded,
            &fixture.seed,
        )
        .expect_err("a substituted ref refuses the swap");
    assert_eq!(
        fixture.manager.direct_ref_target(name).expect("read"),
        Some(fixture.side.clone()),
        "and the other writer's value is untouched: {error}"
    );

    // The same swap against the true current value succeeds, so the
    // refusal above is about the substitution and not about the primitive.
    fixture
        .manager
        .compare_and_swap_ref(
            &mut NoHooks,
            RefSite::CompareAndSwapIntegration,
            name,
            &fixture.side,
            &fixture.seed,
        )
        .expect("expected-old matching the current value swaps");
    assert_eq!(
        fixture.manager.direct_ref_target(name).expect("read"),
        Some(fixture.seed.clone())
    );
}

/// CAS-1's observer: every wait of a registry access the call makes is
/// counted, and either mends the registration the witness tore, so that the
/// attempt after it passes, or ends the access with `ends` as its error, as the
/// topology coordinator's wait does once a message it answered has ended its
/// command. Every funnel phase the call reaches is recorded.
struct WaitsOutATear {
    admin: PathBuf,
    ends: Option<&'static str>,
    pauses: usize,
    phases: Vec<(EffectSiteId, HookPhase)>,
}

impl WaitsOutATear {
    fn mending(admin: PathBuf) -> Self {
        Self {
            admin,
            ends: None,
            pauses: 0,
            phases: Vec::new(),
        }
    }
}

impl EffectHooks for WaitsOutATear {
    fn phase(&mut self, site: EffectSiteId, phase: HookPhase) -> Injection {
        self.phases.push((site, phase));
        Injection::Proceed
    }

    fn refusal_cause(&self) -> Option<String> {
        None
    }

    fn registry_pause(&mut self, _pause: std::time::Duration) -> Result<(), UpstrokeError> {
        self.pauses += 1;
        if let Some(message) = self.ends {
            return Err(UpstrokeError::Refused {
                message: message.to_owned(),
            });
        }
        fs::write(self.admin.join("commondir"), "../..\n").expect("mend the torn commondir");
        match fs::remove_file(self.admin.join("locked")) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => panic!("unlock the torn registration: {error}"),
        }
    }
}

/// An integration ref at the fixture's head, beside a sibling task whose
/// registration is torn the way a killed `git worktree add` leaves it, so the
/// publishability re-check's list fails until something mends it. Returns the
/// ref's name and the registration's administrative directory.
fn an_integration_ref_beside_a_torn_registration(fixture: &Fixture) -> (&'static str, PathBuf) {
    let name = "refs/upstroke/runs/run-1/integration";
    fixture
        .manager
        .create_ref_zero_old(
            &mut NoHooks,
            RefSite::CreateIntegration,
            name,
            &fixture.head,
        )
        .expect("create");
    let sibling = fixture.add_task(&mut NoHooks, "sibling", 1);
    let admin = tear_registration(&fixture.manager, &fixture.manager.slot_path(&sibling));
    (name, admin)
}

/// CAS-1 (the record's §9.21): a compare-and-swap's publishability re-check
/// waits through the call's own hooks. `integrate::publish` hands the swap the
/// topology coordinator's hooks, whose wait answers the coordinator's messages
/// (§9.13, R1). At `4253b2ca` the re-check asked the hook-less
/// `assert_publishable`, so its waits slept on the calling thread, and the
/// tear, which only a wait made through the hooks mends here, held the swap to
/// its deadline. The re-check waits to the production deadline (the record's
/// §9.31, B14), so it asks for the wait after its first failed attempt only
/// if time remains when it reads its clock after that attempt's veto and
/// count. If the deadline has passed by then, wherever the time went (the
/// attempt, its veto, the count, or its thread descheduled), it asks for no
/// wait: it makes its final attempt, or refuses if the failed attempt was
/// already its final one (the record's §9.32 and §9.34).
#[test]
fn a_swaps_publishability_recheck_waits_through_the_calls_hooks() {
    let fixture = Fixture::created("cas1-recheck-waits");
    let _deadline = super::RegistryDeadline::hold(
        fixture.manager.common_git_dir(),
        PRODUCTION_REGISTRY_DEADLINE,
    );
    let (name, admin) = an_integration_ref_beside_a_torn_registration(&fixture);
    let mut hooks = WaitsOutATear::mending(admin);
    let slept = super::fixture::slept_pauses();

    fixture
        .manager
        .compare_and_swap_ref(
            &mut hooks,
            RefSite::CompareAndSwapIntegration,
            name,
            &fixture.head,
            &fixture.seed,
        )
        .expect("the re-check passes once a wait made through the call's hooks mended the tear");

    assert!(
        hooks.pauses >= 1,
        "the re-check met the tear and waited through the call's hooks"
    );
    assert_eq!(
        super::fixture::slept_pauses(),
        slept,
        "no wait of the re-check slept on the calling thread"
    );
    assert_eq!(
        fixture.manager.direct_ref_target(name).expect("read"),
        Some(fixture.seed.clone()),
        "and the swap moved the ref"
    );
}

/// CAS-1, the stop: a wait of the re-check that ends its access ends the swap
/// there, with the wait's own error, before the swap's funnel opens, so the
/// ref is not moved. The topology coordinator's wait ends so once a shutdown
/// it answered has ended its command (§9.16, I2-1). At `4253b2ca` the
/// re-check never reached the hooks, and the swap refused at the access's
/// deadline with the registry's error instead. The re-check waits to the
/// production deadline (the record's §9.31, B14), so it reaches the wait that
/// ends it only if time remains when it reads its clock after its first failed
/// attempt's veto and count. If the deadline has passed by then, wherever the
/// time went (the attempt, its veto, the count, or its thread descheduled), no
/// wait is asked for: the final attempt is made, or was the failed one, and if
/// it fails the access refuses without the wait (the record's §9.32 and §9.34).
#[test]
fn a_wait_that_ends_a_swaps_publishability_recheck_moves_no_ref() {
    const ENDS: &str = "the command ended while the re-check waited";
    let fixture = Fixture::created("cas1-recheck-ends");
    let _deadline = super::RegistryDeadline::hold(
        fixture.manager.common_git_dir(),
        PRODUCTION_REGISTRY_DEADLINE,
    );
    let (name, admin) = an_integration_ref_beside_a_torn_registration(&fixture);
    let mut hooks = WaitsOutATear {
        ends: Some(ENDS),
        ..WaitsOutATear::mending(admin)
    };

    let error = fixture
        .manager
        .compare_and_swap_ref(
            &mut hooks,
            RefSite::CompareAndSwapIntegration,
            name,
            &fixture.head,
            &fixture.seed,
        )
        .expect_err("a wait that ends the access ends the swap");

    assert!(
        matches!(&error, UpstrokeError::Refused { message } if message == ENDS),
        "the swap ends with the wait's own error: {error:?}"
    );
    assert_eq!(hooks.pauses, 1, "the access stopped at its first wait");
    assert!(
        hooks.phases.is_empty(),
        "the swap's funnel never opened: {:?}",
        hooks.phases
    );
    assert_eq!(
        fixture.manager.direct_ref_target(name).expect("read"),
        Some(fixture.head.clone()),
        "and the ref is where it was"
    );
}

/// One function of this module's production code, as the hooks-routing census
/// reads it: whether its signature takes hooks, whether its body names a wait
/// that sleeps by default, and the names its body calls.
struct RoutingRead {
    name: String,
    takes_hooks: bool,
    names_a_sleeping_wait: bool,
    calls: BTreeSet<String>,
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Whether `code` names the identifier `word` as a whole word.
fn names_word(code: &str, word: &str) -> bool {
    code.match_indices(word).any(|(at, _)| {
        let before = code[..at].bytes().next_back();
        let after = code[at + word.len()..].bytes().next();
        !before.is_some_and(is_identifier_byte) && !after.is_some_and(is_identifier_byte)
    })
}

/// Every identifier in `body` that is called: followed, past whitespace and
/// any turbofish, by `(`.
fn called_names(body: &str) -> BTreeSet<String> {
    let bytes = body.as_bytes();
    let mut called = BTreeSet::new();
    let mut at = 0;
    while at < bytes.len() {
        if !is_identifier_byte(bytes[at]) || (at > 0 && is_identifier_byte(bytes[at - 1])) {
            at += 1;
            continue;
        }
        let start = at;
        while at < bytes.len() && is_identifier_byte(bytes[at]) {
            at += 1;
        }
        let mut next = at;
        if body[next..].starts_with("::<") {
            let mut depth = 0_usize;
            while next < bytes.len() {
                match bytes[next] {
                    b'<' => depth += 1,
                    b'>' => {
                        depth -= 1;
                        if depth == 0 {
                            next += 1;
                            break;
                        }
                    }
                    _ => {}
                }
                next += 1;
            }
        }
        while next < bytes.len() && bytes[next].is_ascii_whitespace() {
            next += 1;
        }
        if bytes.get(next) == Some(&b'(') {
            called.insert(body[start..at].to_owned());
        }
    }
    called
}

/// Every `fn` item with a body in `code`, which comments, strings and
/// test-only items have already been blanked out of. A nested `fn` is read
/// on its own, and its body is also part of the body that holds it.
fn routing_reads(code: &str) -> Vec<RoutingRead> {
    let bytes = code.as_bytes();
    let mut reads = Vec::new();
    for (start, _) in code.match_indices("fn ") {
        if start > 0 && is_identifier_byte(bytes[start - 1]) {
            continue;
        }
        let after = start + "fn ".len();
        let name: String = code[after..]
            .trim_start()
            .chars()
            .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
            .collect();
        if name.is_empty() {
            continue;
        }
        let mut depth = 0_i32;
        let mut open = None;
        for (at, byte) in bytes.iter().enumerate().skip(after) {
            match byte {
                b'(' => depth += 1,
                b')' => depth -= 1,
                b'{' if depth == 0 => {
                    open = Some(at);
                    break;
                }
                b';' if depth == 0 => break,
                _ => {}
            }
        }
        let Some(open) = open else {
            continue;
        };
        let mut braces = 0_usize;
        let mut close = None;
        for (at, byte) in bytes.iter().enumerate().skip(open) {
            match byte {
                b'{' => braces += 1,
                b'}' => {
                    braces -= 1;
                    if braces == 0 {
                        close = Some(at);
                        break;
                    }
                }
                _ => {}
            }
        }
        let close = close.unwrap_or_else(|| panic!("the body of `fn {name}` closes"));
        let (signature, body) = (&code[start..open], &code[open..=close]);
        reads.push(RoutingRead {
            name,
            takes_hooks: signature.contains("EffectHooks"),
            names_a_sleeping_wait: names_word(body, "sleep_for") || names_word(body, "NoHooks"),
            calls: called_names(body),
        });
    }
    reads
}

/// What the hooks-routing census found: the functions that take hooks and
/// reach a wait that sleeps by default, the names that reach one, the names
/// that take hooks, and how many functions it read.
struct RoutingCensus {
    violations: BTreeSet<String>,
    sleeping: BTreeSet<String>,
    hooked: BTreeSet<String>,
    read: usize,
}

/// The hooks-routing census over `sources`, copies of this module's files.
///
/// A wait that sleeps by default is `sleep_for`, the wait an access made with
/// no hooks sleeps by, or [`NoHooks`], whose wait is that default. A function
/// reaches one when its body names either or calls, by name, a function that
/// reaches one. A function that takes hooks and reaches one drops them: the
/// waits of that access sleep on the caller's thread whatever hooks the caller
/// passed, which on the topology coordinator is a sleep on its thread (CAS-1,
/// the record's §9.21). [`EffectHooks::registry_pause`] is not read as one: it
/// is the seam itself, the wait a routed access asks of the caller's hooks,
/// and its default is what a caller that is not the coordinator chooses.
///
/// **What it cannot see.** Calls are matched by name, so a call of another
/// type's method spelt like one that reaches a sleeping wait reads as one; and
/// a wait reached through a function value, a trait object, a closure handed
/// in from outside these files, or hooks of another type a function builds
/// for itself, is not seen at all. It reads this module's files and no other:
/// an engine call site that has hooks and calls a hook-less function is the
/// sibling sweep's (§9.21), not this census's.
fn hooks_routing_census(sources: &[&str]) -> RoutingCensus {
    let reads: Vec<RoutingRead> = sources
        .iter()
        .flat_map(|source| {
            routing_reads(&crate::effects::production_code(
                &source.replace("\r\n", "\n"),
            ))
        })
        .filter(|read| read.name != "registry_pause")
        .collect();
    let mut sleeping: BTreeSet<String> = std::iter::once("sleep_for".to_owned())
        .chain(
            reads
                .iter()
                .filter(|read| read.names_a_sleeping_wait)
                .map(|read| read.name.clone()),
        )
        .collect();
    loop {
        let reached: Vec<String> = reads
            .iter()
            .filter(|read| !sleeping.contains(&read.name))
            .filter(|read| read.calls.iter().any(|called| sleeping.contains(called)))
            .map(|read| read.name.clone())
            .collect();
        if reached.is_empty() {
            break;
        }
        sleeping.extend(reached);
    }
    let violations = reads
        .iter()
        .filter(|read| read.takes_hooks)
        .filter(|read| {
            read.names_a_sleeping_wait || read.calls.iter().any(|called| sleeping.contains(called))
        })
        .map(|read| read.name.clone())
        .collect();
    let hooked = reads
        .iter()
        .filter(|read| read.takes_hooks)
        .map(|read| read.name.clone())
        .collect();
    RoutingCensus {
        violations,
        sleeping,
        hooked,
        read: reads.len(),
    }
}

/// The path under `src/` of this module's child `name`, built from path
/// components: `workspace_manager/<name>.rs`.
fn child_module_path(name: &str) -> PathBuf {
    Path::new("workspace_manager")
        .join(name)
        .with_extension("rs")
}

/// This module's production files, `src/workspace_manager.rs` and every child
/// it declares outside a test-only item, as `(path under src/, text)`.
fn this_modules_sources() -> Vec<(PathBuf, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let parent_file = PathBuf::from("workspace_manager.rs");
    let parent = fs::read_to_string(root.join(&parent_file))
        .expect("this module's source")
        .replace("\r\n", "\n");
    let code = crate::effects::production_code(&parent);
    let children: BTreeSet<String> = code
        .match_indices("mod ")
        .filter(|(at, _)| {
            !code[..*at]
                .bytes()
                .next_back()
                .is_some_and(is_identifier_byte)
        })
        .filter_map(|(at, _)| {
            let rest = &code[at + "mod ".len()..];
            let name: String = rest
                .chars()
                .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
                .collect();
            (!name.is_empty() && rest[name.len()..].trim_start().starts_with(';')).then_some(name)
        })
        .collect();
    std::iter::once((parent_file, parent))
        .chain(children.into_iter().map(|child| {
            let file = child_module_path(&child);
            let text = fs::read_to_string(root.join(&file))
                .unwrap_or_else(|error| panic!("the declared child {}: {error}", file.display()));
            (file, text)
        }))
        .collect()
}

/// CAS-1's census (the record's §9.21): no function of this module that takes
/// hooks reaches a registry wait that sleeps by default. Every wait of a
/// registry access a call makes goes through the hooks the caller passed,
/// which the topology coordinator makes answer its messages (§9.13, R1). At
/// `4253b2ca` it named one function, `compare_and_swap_ref`, whose
/// publishability re-check asked the hook-less `assert_publishable`.
#[test]
fn no_function_that_takes_hooks_reaches_a_registry_wait_that_sleeps_by_default() {
    let sources = this_modules_sources();
    let files: BTreeSet<&Path> = sources.iter().map(|(file, _)| file.as_path()).collect();
    assert!(
        files.contains(Path::new("workspace_manager.rs")),
        "the census reads the parent: {files:?}"
    );
    for child in ["hooks", "worktree", "snapshot_ref", "object", "residue"] {
        assert!(
            files.contains(child_module_path(child).as_path()),
            "the census reads the declared child `{child}`: {files:?}"
        );
    }
    assert!(
        !files.contains(child_module_path("tests").as_path())
            && !files.contains(child_module_path("fixture").as_path()),
        "a test-only child is not production code: {files:?}"
    );
    let texts: Vec<&str> = sources.iter().map(|(_, text)| text.as_str()).collect();
    let census = hooks_routing_census(&texts);
    assert!(
        census.read > 200,
        "the census read this module's functions rather than nothing: {}",
        census.read
    );
    for hookless in [
        "revalidate",
        "worktree_records",
        "assert_publishable",
        "proposal_state",
        "changed_paths",
        "commit_parent",
        "commit_tree_sha",
        "quiescence",
    ] {
        assert!(
            census.sleeping.contains(hookless),
            "the census reads `{hookless}`, a hook-less entry point it exists to keep out of \
             every call that takes hooks, as reaching a sleeping wait: {:?}",
            census.sleeping
        );
    }
    for hooked in [
        "compare_and_swap_ref",
        "write_intent",
        "add_worktree",
        "remove_worktree",
        "assert_publishable_pausing",
        "funnel_lending",
    ] {
        assert!(
            census.hooked.contains(hooked),
            "the census reads `{hooked}` as taking hooks"
        );
    }
    assert!(
        census.violations.is_empty(),
        "these functions take hooks and reach a registry wait that sleeps by default, so on the \
         topology coordinator that wait sleeps on its thread: {:?}",
        census.violations
    );
}

/// The census's own positive control (§12): the violation it exists for, put
/// back into the live source, and two more shapes; and the same violation as
/// prose, which it does not count.
#[test]
fn the_hooks_routing_census_reports_a_swap_that_drops_its_hooks() {
    let sources = this_modules_sources();
    let routed = "        self.assert_publishable_pausing(hooks, refname)?;\n        \
                  refuse_new(refname, new)?;\n";
    let texts: Vec<String> = sources
        .iter()
        .map(|(file, text)| {
            if file == Path::new("workspace_manager.rs") {
                assert_eq!(
                    text.matches(routed).count(),
                    1,
                    "the swap's routed re-check is spelt once in the live source"
                );
                text.replacen(
                    routed,
                    "        self.assert_publishable(refname)?;\n        refuse_new(refname, new)?;\n",
                    1,
                )
            } else {
                text.clone()
            }
        })
        .collect();
    let dropped: Vec<&str> = texts.iter().map(String::as_str).collect();
    assert_eq!(
        hooks_routing_census(&dropped).violations,
        BTreeSet::from(["compare_and_swap_ref".to_owned()]),
        "the swap that asks the hook-less check is reported, and nothing else"
    );

    let through_a_helper = "impl M {\n    fn checked(&self) -> R {\n        self.revalidate()\n    }\n    pub fn primitive(&self, hooks: &mut dyn EffectHooks) -> R {\n        self.checked()?;\n        funnel(hooks, S, || Ok(()))\n    }\n    pub fn revalidate(&self) -> R {\n        self.revalidate_with(&mut sleep_for)\n    }\n}\n";
    assert_eq!(
        hooks_routing_census(&[through_a_helper]).violations,
        BTreeSet::from(["primitive".to_owned()]),
        "a hook-less wait reached through a helper is reported at the function that took hooks"
    );
    let handed_none = "fn primitive(&self, hooks: &mut dyn EffectHooks) -> R {\n    self.revalidate_pausing(&mut NoHooks)?;\n    funnel(hooks, S, || Ok(()))\n}\n";
    assert_eq!(
        hooks_routing_census(&[handed_none]).violations,
        BTreeSet::from(["primitive".to_owned()]),
        "a pausing twin handed no hooks is reported"
    );
    let the_seam = "fn registry_pause(&mut self, pause: Duration) -> R {\n    super::sleep_for(pause)\n}\nfn primitive(&self, hooks: &mut dyn EffectHooks) -> R {\n    self.revalidate_with(&mut |pause| hooks.registry_pause(pause))\n}\n";
    assert!(
        hooks_routing_census(&[the_seam]).violations.is_empty(),
        "a wait asked of the caller's hooks is the routing itself, whatever their default does"
    );

    for (what, prose) in [
        ("a block comment", format!("/*\n{handed_none}*/\n")),
        (
            "a raw string literal",
            format!("const S: &str = r\"{handed_none}\";\n"),
        ),
    ] {
        let census = hooks_routing_census(&[prose.as_str()]);
        assert!(
            census.violations.is_empty() && census.read == 0,
            "{what} is prose, not a function of this module: {:?}",
            census.violations
        );
    }
}

/// The direct-ref reader refuses a **symbolic ref that resolves to the
/// expected object** (`PR5-WORKSPACE-031`).
///
/// `ref_rules`: "all refs **direct** … symbolic refs refused". Every call
/// of `direct_ref_target` in this file is on a ref the fixture created with
/// `create_ref_zero_old`, so the reader was never once pointed at a
/// symbolic ref; the one test that builds a symbolic ref reads its victim
/// back through a raw `git symbolic-ref` helper instead. The case that
/// separates a non-dereferencing `show-ref --verify` from a dereferencing
/// `rev-parse --verify` is exactly the one never constructed: an indirection
/// that yields the **right** object, and so hides itself.
#[test]
fn a_symbolic_ref_that_resolves_to_the_expected_object_is_still_refused() {
    let fixture = Fixture::created("symbolic-reader");
    let direct = "refs/upstroke/runs/run-1/candidates/kalpha/1";
    let symbolic = "refs/upstroke/runs/run-1/integration";
    fixture
        .manager
        .create_ref_zero_old(
            &mut NoHooks,
            RefSite::CreateCandidates,
            direct,
            &fixture.head,
        )
        .expect("create the direct ref");
    git(&fixture.base, &["symbolic-ref", symbolic, direct]);
    assert_eq!(
        git(&fixture.base, &["rev-parse", "--verify", symbolic]),
        fixture.head,
        "dereferencing yields exactly the object a caller expects, which is what              makes this the hiding case"
    );

    let error = refusal_of(
        &fixture
            .manager
            .direct_ref_target(symbolic)
            .expect_err("a symbolic ref is not a direct one, whatever it resolves to"),
    );
    assert!(
        error.contains("symbolic"),
        "the refusal must name its reason: {error}"
    );
    // And the direct ref beside it still reads back, so the reader has not
    // simply stopped working.
    assert_eq!(
        fixture.manager.direct_ref_target(direct).expect("read"),
        Some(fixture.head.clone())
    );
}

#[test]
fn refs_are_created_zero_old_and_moved_or_deleted_only_expected_old() {
    let fixture = Fixture::created("ref-rules");
    let name = "refs/upstroke/runs/run-1/candidates/kalpha/1";
    fixture
        .manager
        .create_ref_zero_old(&mut NoHooks, RefSite::CreateCandidates, name, &fixture.head)
        .expect("zero-old creation");
    assert_eq!(
        fixture
            .manager
            .direct_ref_target(name)
            .expect("read the ref"),
        Some(fixture.head.clone())
    );

    fixture
        .manager
        .create_ref_zero_old(&mut NoHooks, RefSite::CreateCandidates, name, &fixture.seed)
        .expect_err("zero-old refuses a ref that already exists");
    assert_eq!(
        fixture.manager.direct_ref_target(name).expect("read"),
        Some(fixture.head.clone()),
        "and leaves it where it was"
    );

    fixture
        .manager
        .delete_ref_expected_old(
            &mut NoHooks,
            RefSite::DeleteCandidatesRef,
            name,
            &fixture.seed,
        )
        .expect_err("a wrong expected-old refuses");
    assert_eq!(
        fixture.manager.direct_ref_target(name).expect("read"),
        Some(fixture.head.clone()),
        "and leaves it where it was"
    );

    fixture
        .manager
        .delete_ref_expected_old(
            &mut NoHooks,
            RefSite::DeleteCandidatesRef,
            name,
            &fixture.head,
        )
        .expect("the right expected-old deletes");
    assert_eq!(fixture.manager.direct_ref_target(name).expect("read"), None);
}

/// The trap this project's guard exists for: a fix that introduces a
/// defect. `git update-ref -d <ref> 0{40}` **succeeds and deletes**,
/// because the null id means "must not exist"; a primitive that passed it
/// through would perform an unconditional delete under a name that
/// promises expected-old.
#[test]
fn the_null_object_id_is_never_an_expected_old_value() {
    let fixture = Fixture::created("null-old");
    let name = "refs/upstroke/runs/run-1/candidate-prepared/kalpha/1";
    fixture
        .manager
        .create_ref_zero_old(
            &mut NoHooks,
            RefSite::PinCandidatePrepared,
            name,
            &fixture.head,
        )
        .expect("pin");

    for null in ["0".repeat(40), "0".repeat(64)] {
        let message = refusal_of(
            &fixture
                .manager
                .delete_ref_expected_old(&mut NoHooks, RefSite::DeleteCandidatePin, name, &null)
                .expect_err("the null expected-old refuses"),
        );
        assert!(
            message.contains("null object id") && message.contains("INV-17"),
            "the refusal must name its reason: {message}"
        );
    }
    assert_eq!(
        fixture.manager.direct_ref_target(name).expect("read"),
        Some(fixture.head.clone()),
        "and the pin is still there"
    );

    // The measurement this refusal is derived from: raw Git really does
    // delete on the null id, so the refusal is guarding a live hazard and
    // not a hypothetical one.
    let raw = git_out(
        &fixture.base,
        &["update-ref", "--no-deref", "-d", name, &"0".repeat(40)],
    );
    assert!(
        raw.status.success()
            && fixture
                .manager
                .direct_ref_target(name)
                .expect("read")
                .is_none(),
        "raw `git update-ref -d <ref> 0{{40}}` deletes unconditionally; that is why the \
             primitive refuses it"
    );
}

/// The other side of the same trap (`PR126-OBJECT-NEW-SIDE-ACCEPTS-NULL-ID`):
/// a null **new** value means "must not exist afterwards", so raw Git turns a
/// compare-and-swap whose expected-old matches into a delete, and a create of
/// an absent ref into a success that creates nothing. Both primitives refuse
/// it before the mutating `update-ref`, at both hash lengths whatever the
/// repository's format (the refusal is on the value, and the symbolic-ref
/// and checked-out checks that run before it read Git without mutating), and
/// the raw measurement is executed in a repository of each format, with the
/// null id spelt at that format's length, so the refusal guards a live hazard
/// and not a hypothetical one.
#[test]
fn the_null_object_id_is_never_a_new_value_through_create_or_compare_and_swap() {
    for fixture in [
        Fixture::created("null-new-sha1"),
        Fixture::created_sha256("null-new-sha256"),
    ] {
        null_new_value_refuses_and_raw_git_would_not(&fixture);
    }
}

fn null_new_value_refuses_and_raw_git_would_not(fixture: &Fixture) {
    let existing = "refs/upstroke/runs/run-1/integration";
    let absent = "refs/upstroke/runs/run-1/candidates/kalpha/1";
    fixture
        .manager
        .create_ref_zero_old(
            &mut NoHooks,
            RefSite::CreateIntegration,
            existing,
            &fixture.head,
        )
        .expect("create");

    for null in ["0".repeat(40), "0".repeat(64)] {
        let message = refusal_of(
            &fixture
                .manager
                .compare_and_swap_ref(
                    &mut NoHooks,
                    RefSite::CompareAndSwapIntegration,
                    existing,
                    &fixture.head,
                    &null,
                )
                .expect_err("a null new value refuses the swap"),
        );
        assert!(
            message.contains("null object id") && message.contains("must not exist afterwards"),
            "the refusal must name its reason: {message}"
        );
        assert_eq!(
            fixture
                .manager
                .direct_ref_target(existing)
                .expect("read")
                .as_deref(),
            Some(fixture.head.as_str()),
            "and the ref is still there"
        );

        let message = refusal_of(
            &fixture
                .manager
                .create_ref_zero_old(&mut NoHooks, RefSite::CreateCandidates, absent, &null)
                .expect_err("a null new value refuses the create"),
        );
        assert!(
            message.contains("null object id"),
            "the refusal must name its reason: {message}"
        );
        assert_eq!(
            fixture.manager.direct_ref_target(absent).expect("read"),
            None,
            "and nothing was created"
        );
    }

    // The measurement the refusals are derived from, in this repository's
    // own format: raw Git deletes through the swap when the old value
    // matches, and creates nothing through the create when the ref is
    // absent, exiting 0 both times.
    let null = "0".repeat(fixture.head.len());
    let raw = git_out(
        &fixture.base,
        &["update-ref", "--no-deref", existing, &null, &fixture.head],
    );
    assert!(
        raw.status.success()
            && fixture
                .manager
                .direct_ref_target(existing)
                .expect("read")
                .is_none(),
        "raw `git update-ref <ref> <null> <old>` deletes when the old value matches; that is why \
         the primitive refuses it"
    );
    let raw = git_out(
        &fixture.base,
        &["update-ref", "--no-deref", absent, &null, ""],
    );
    assert!(
        raw.status.success()
            && fixture
                .manager
                .direct_ref_target(absent)
                .expect("read")
                .is_none(),
        "raw `git update-ref <ref> <null> \"\"` exits 0 and creates nothing; that is why the \
         primitive refuses it"
    );
}

#[test]
fn a_malformed_object_id_never_reaches_the_ref_command() {
    let fixture = Fixture::created("malformed-oid");
    for hostile in ["--delete", "", "refs/heads/main", "zzzz", &"a".repeat(39)] {
        let message = refusal_of(
            &fixture
                .manager
                .create_ref_zero_old(
                    &mut NoHooks,
                    RefSite::CreateIntegration,
                    "refs/heads/upstroke/run-1",
                    hostile,
                )
                .expect_err("a malformed object id refuses"),
        );
        assert!(
            message.contains("full hexadecimal object id"),
            "the refusal must name its reason for `{hostile}`: {message}"
        );
    }
    assert_eq!(
        fixture
            .manager
            .direct_ref_target("refs/heads/upstroke/run-1")
            .expect("read"),
        None
    );
}

#[test]
fn an_unexpected_ref_under_the_run_namespace_refuses() {
    let fixture = Fixture::created("unexpected-refs");
    let namespace = "refs/upstroke/runs/run-1/";
    let mine = "refs/upstroke/runs/run-1/candidates/kalpha/1".to_owned();
    fixture
        .manager
        .create_ref_zero_old(
            &mut NoHooks,
            RefSite::CreateCandidates,
            &mine,
            &fixture.head,
        )
        .expect("create");
    fixture
        .manager
        .refuse_unexpected_refs(namespace, std::slice::from_ref(&mine))
        .expect("the namespace carries only what is expected");

    git(
        &fixture.base,
        &[
            "update-ref",
            "refs/upstroke/runs/run-1/stowaway",
            &fixture.seed,
        ],
    );
    let message = refusal_of(
        &fixture
            .manager
            .refuse_unexpected_refs(namespace, std::slice::from_ref(&mine))
            .expect_err("an unexpected ref refuses"),
    );
    assert!(
        message.contains("unexpected ref") && message.contains("stowaway"),
        "the refusal must name its reason and the ref: {message}"
    );
}

/// A **packed** unexpected ref refuses, and so does a **nested** one
/// (`PR5-WORKSPACE-033`).
///
/// `expected_failures_refusals[2]` is "unexpected refs under the run
/// namespace" with no exception for how Git happens to be storing them.
/// The test above plants its stowaway with a plain `update-ref` and never
/// runs `pack-refs`, so the stowaway is a loose file and rewriting
/// `refs_under` to walk `<common git dir>/refs` and ignore `packed-refs`
/// entirely still found it. Nothing in this file called `pack-refs` at all,
/// and no fixture nested a ref deeper than the two-level
/// `candidates/kalpha/1`.
#[test]
fn a_packed_or_nested_unexpected_ref_refuses_too() {
    let fixture = Fixture::created("packed-refs");
    let namespace = "refs/upstroke/runs/run-1/";
    let mine = "refs/upstroke/runs/run-1/candidates/kalpha/1".to_owned();
    fixture
        .manager
        .create_ref_zero_old(
            &mut NoHooks,
            RefSite::CreateCandidates,
            &mine,
            &fixture.head,
        )
        .expect("create");

    let nested = "refs/upstroke/runs/run-1/candidates/kalpha/deeper/still/1";
    git(&fixture.base, &["update-ref", nested, &fixture.seed]);
    git(&fixture.base, &["pack-refs", "--all"]);
    assert!(
        fixture.base.join(".git/packed-refs").is_file(),
        "the fixture really packed the refs"
    );
    assert!(
        !fixture.base.join(".git").join(nested).exists(),
        "and the stowaway is no longer a loose file, which is the whole point"
    );

    let listed: Vec<String> = fixture
        .manager
        .refs_under(namespace)
        .expect("enumerate")
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    assert!(
        listed.contains(&nested.to_owned()),
        "a packed ref is still a ref under the namespace: {listed:?}"
    );

    let message = refusal_of(
        &fixture
            .manager
            .refuse_unexpected_refs(namespace, std::slice::from_ref(&mine))
            .expect_err("a packed, nested, unexpected ref refuses"),
    );
    assert!(
        message.contains("unexpected ref") && message.contains("deeper/still"),
        "the refusal must name the packed nested ref: {message}"
    );

    // And every ref is still exactly where it was: a refusal acts on
    // nothing.
    assert_eq!(
        fixture.manager.direct_ref_target(&mine).expect("read"),
        Some(fixture.head.clone())
    );
    assert_eq!(
        fixture.manager.direct_ref_target(nested).expect("read"),
        Some(fixture.seed.clone())
    );
}

/// The integration ref checked out in a **second linked worktree** refuses
/// (`PR5-WORKSPACE-032`).
///
/// `integration_ref`: "never checked out; `assert_publishable()` before
/// every prepare/CAS/recovery". The one test of the refusal asks about
/// `refs/heads/main`, which is checked out in the *primary* worktree — so
/// truncating the scan to the first worktree record still refused it, and
/// the negative case is a ref checked out nowhere. A linked worktree is
/// exactly the shape this manager creates for its own work, so it is the
/// one that had to be built.
#[test]
fn an_integration_ref_checked_out_in_a_second_worktree_is_refused() {
    let fixture = Fixture::created("checked-out-elsewhere");
    let refname = "refs/heads/upstroke/run-1";
    git(&fixture.base, &["branch", "upstroke/run-1", &fixture.head]);
    fixture
        .manager
        .assert_publishable(refname)
        .expect("checked out nowhere yet");

    let elsewhere = fixture.root.join("elsewhere");
    git(
        &fixture.base,
        &[
            "worktree",
            "add",
            "-q",
            &elsewhere.to_string_lossy(),
            "upstroke/run-1",
        ],
    );
    assert_eq!(
        git(&elsewhere, &["symbolic-ref", "-q", "--", "HEAD"]),
        refname,
        "the second worktree really has the integration ref checked out"
    );
    assert!(
        fixture.manager.worktree_records().expect("records").len() >= 2,
        "and it is not the first record, which a truncated scan would still see"
    );

    let message = refusal_of(
        &fixture
            .manager
            .assert_publishable(refname)
            .expect_err("a ref checked out in a linked worktree refuses"),
    );
    assert!(
        message.contains("checked out in the worktree"),
        "the refusal must name its reason: {message}"
    );
}

// =======================================================================
// A Git lock file a killed engine ref write left (PR8-CRASH-002)
// =======================================================================

/// A ref under this run's namespace, `refs/upstroke/runs/<run-id>/<name>`.
fn run_ref(name: &str) -> String {
    format!("{RUN_REF_ROOT}/{}/{name}", super::fixture::RUN_ID)
}

/// Where Git's files backend keeps the lock of `refname`, as
/// `WorkspaceManager::ref_lock_path` spells it.
fn lock_of(fixture: &Fixture, refname: &str) -> PathBuf {
    fixture
        .manager
        .common_git_dir
        .join(format!("{refname}.lock"))
}

/// The three primitives each reclaim the lock a killed write of their own
/// left: empty, as a kill between Git's `open` and its content write leaves
/// it, for a create; naming the value being written, as a kill between that
/// write and the publishing rename leaves it, for a swap; and empty for a
/// delete, whose lock Git never writes into.
#[test]
fn a_stale_lock_of_the_engines_own_write_is_reclaimed_by_the_write_that_retries() {
    let fixture = Fixture::created("ref-lock-reclaim");
    let name = run_ref("candidates/kalpha/1");
    let lock = lock_of(&fixture, &name);

    write_file(&lock, b"");
    fixture
        .manager
        .create_ref_zero_old(
            &mut NoHooks,
            RefSite::CreateCandidates,
            &name,
            &fixture.head,
        )
        .expect("the create reclaims the empty lock its killed predecessor left");
    assert!(!lock.exists(), "the reclaimed lock is gone");
    assert_eq!(
        fixture.manager.direct_ref_target(&name).expect("read"),
        Some(fixture.head.clone())
    );

    write_file(&lock, format!("{}\n", fixture.seed).as_bytes());
    fixture
        .manager
        .compare_and_swap_ref(
            &mut NoHooks,
            RefSite::CompareAndSwapIntegration,
            &name,
            &fixture.head,
            &fixture.seed,
        )
        .expect("the swap reclaims the lock naming the value it writes");
    assert!(!lock.exists());
    assert_eq!(
        fixture.manager.direct_ref_target(&name).expect("read"),
        Some(fixture.seed.clone())
    );

    write_file(&lock, b"");
    fixture
        .manager
        .delete_ref_expected_old(
            &mut NoHooks,
            RefSite::DeleteCandidatesRef,
            &name,
            &fixture.seed,
        )
        .expect("the delete reclaims the empty lock");
    assert!(!lock.exists());
    assert_eq!(
        fixture.manager.direct_ref_target(&name).expect("read"),
        None
    );
}

/// A lock naming anything but the value this write writes belongs to another
/// write: it is left where it is and the write refuses, saying so. For a
/// delete that is any content at all, since a deletion's lock is never
/// written into -- the ref's own current value included.
#[test]
fn a_lock_naming_another_value_is_left_and_the_write_refuses() {
    let fixture = Fixture::created("ref-lock-foreign");
    let name = run_ref("candidates/kalpha/1");
    fixture
        .manager
        .create_ref_zero_old(
            &mut NoHooks,
            RefSite::CreateCandidates,
            &name,
            &fixture.head,
        )
        .expect("create");
    let lock = lock_of(&fixture, &name);

    write_file(&lock, format!("{}\n", fixture.side).as_bytes());
    let error = fixture
        .manager
        .compare_and_swap_ref(
            &mut NoHooks,
            RefSite::CompareAndSwapIntegration,
            &name,
            &fixture.head,
            &fixture.seed,
        )
        .expect_err("a lock naming a third value is not this write's");
    assert!(
        error
            .to_string()
            .contains("names a value this write would not produce"),
        "{error}"
    );
    assert!(lock.exists(), "left in place");
    assert_eq!(
        fixture.manager.direct_ref_target(&name).expect("read"),
        Some(fixture.head.clone()),
        "the ref is untouched"
    );

    write_file(&lock, format!("{}\n", fixture.head).as_bytes());
    let error = fixture
        .manager
        .delete_ref_expected_old(
            &mut NoHooks,
            RefSite::DeleteCandidatesRef,
            &name,
            &fixture.head,
        )
        .expect_err(
            "a deletion writes nothing into its lock, so a lock with content is not a deletion's",
        );
    assert!(
        error
            .to_string()
            .contains("names a value this write would not produce"),
        "{error}"
    );
    assert!(lock.exists());
    assert_eq!(
        fixture.manager.direct_ref_target(&name).expect("read"),
        Some(fixture.head.clone())
    );

    // A lock longer than any object id and its newline is read only as far as
    // the bound and is then, whatever follows, not this write's.
    write_file(&lock, &vec![b'a'; 4096]);
    let error = fixture
        .manager
        .compare_and_swap_ref(
            &mut NoHooks,
            RefSite::CompareAndSwapIntegration,
            &name,
            &fixture.head,
            &fixture.seed,
        )
        .expect_err("a long lock is not this write's");
    assert!(
        error
            .to_string()
            .contains("names a value this write would not produce"),
        "{error}"
    );
    assert!(lock.exists());
}

/// A lock on a ref outside the run's namespace, offered to a namespace site,
/// is nothing this manager reasons about: it is left exactly as before, and
/// Git refuses the write on it as Git always did.
#[test]
fn a_lock_outside_the_run_namespace_is_left_for_git_to_refuse() {
    let fixture = Fixture::created("ref-lock-elsewhere");
    for name in [
        "refs/upstroke/runs/01KZSOMEOTHERRUN0000000000/candidates/kalpha/1",
        "refs/heads/elsewhere",
    ] {
        let lock = lock_of(&fixture, name);
        write_file(&lock, b"");
        let error = fixture
            .manager
            .create_ref_zero_old(&mut NoHooks, RefSite::CreateCandidates, name, &fixture.head)
            .expect_err("Git refuses on the lock, as before");
        assert!(
            matches!(&error, UpstrokeError::Git { message } if message.contains(".lock")),
            "{name}: Git's own refusal, naming the lock: {error}"
        );
        assert!(lock.exists(), "{name}: the lock was not touched");
        assert_eq!(
            fixture.manager.direct_ref_target(name).expect("read"),
            None,
            "{name}: nothing was created"
        );
    }
}

/// The two integration sites write the ref `run_started` recorded, wherever
/// it lives -- the scaffold's is `refs/heads/upstroke/run-<id>` -- so a stale
/// lock on it is reclaimed by its name being the run's integration ref rather
/// than by the namespace.
#[test]
fn the_integration_sites_reclaim_a_stale_lock_on_the_recorded_integration_ref() {
    let fixture = Fixture::created("ref-lock-integration");
    let name = format!("refs/heads/upstroke/run-{}", super::fixture::RUN_ID);
    let lock = lock_of(&fixture, &name);

    write_file(&lock, b"");
    fixture
        .manager
        .create_ref_zero_old(
            &mut NoHooks,
            RefSite::CreateIntegration,
            &name,
            &fixture.head,
        )
        .expect("the integration ref's create reclaims its stale lock");
    assert!(!lock.exists());

    write_file(&lock, fixture.seed.as_bytes());
    fixture
        .manager
        .compare_and_swap_ref(
            &mut NoHooks,
            RefSite::CompareAndSwapIntegration,
            &name,
            &fixture.head,
            &fixture.seed,
        )
        .expect("the integration ref's swap reclaims a lock naming the value it writes");
    assert!(!lock.exists());
    assert_eq!(
        fixture.manager.direct_ref_target(&name).expect("read"),
        Some(fixture.seed.clone())
    );

    // The same name offered to a namespace site is outside that site's
    // namespace, and is left for Git.
    write_file(&lock, b"");
    let error = fixture
        .manager
        .delete_ref_expected_old(
            &mut NoHooks,
            RefSite::DeleteCandidatesRef,
            &name,
            &fixture.seed,
        )
        .expect_err("a namespace site does not reclaim outside the namespace");
    assert!(
        matches!(&error, UpstrokeError::Git { .. }),
        "Git's own refusal: {error}"
    );
    assert!(lock.exists());
}

/// With the ref in `packed-refs`, a `git pack-refs --prune` may be holding
/// the lock at this instant and nothing the repository records says
/// otherwise, so the lock is left and the write refuses -- and once an
/// operator has removed it, the swap writes the loose ref over the packed
/// copy as any swap of a packed ref does.
#[test]
fn a_stale_lock_on_a_packed_ref_is_left_and_the_write_refuses() {
    let fixture = Fixture::created("ref-lock-packed");
    let name = run_ref("candidates/kalpha/1");
    fixture
        .manager
        .create_ref_zero_old(
            &mut NoHooks,
            RefSite::CreateCandidates,
            &name,
            &fixture.head,
        )
        .expect("create");
    git(&fixture.base, &["pack-refs", "--all"]);
    assert!(
        !fixture.manager.common_git_dir.join(&name).exists(),
        "pack-refs packed the ref and pruned its loose file"
    );
    assert_eq!(
        fixture
            .manager
            .packed_ref_value(&name)
            .expect("read packed-refs"),
        Some(fixture.head.clone()),
        "and the packed file names it"
    );

    let lock = lock_of(&fixture, &name);
    write_file(&lock, b"");
    let error = fixture
        .manager
        .compare_and_swap_ref(
            &mut NoHooks,
            RefSite::CompareAndSwapIntegration,
            &name,
            &fixture.head,
            &fixture.seed,
        )
        .expect_err("a lock on a packed ref may be a prune's");
    assert!(
        error.to_string().contains("the ref is in packed-refs"),
        "{error}"
    );
    assert!(lock.exists(), "left in place");
    assert_eq!(
        fixture.manager.direct_ref_target(&name).expect("read"),
        Some(fixture.head.clone()),
        "the ref is untouched"
    );

    super::fixture::remove_file(&lock);
    fixture
        .manager
        .compare_and_swap_ref(
            &mut NoHooks,
            RefSite::CompareAndSwapIntegration,
            &name,
            &fixture.head,
            &fixture.seed,
        )
        .expect("with the lock gone the swap writes the loose ref");
    assert_eq!(
        fixture.manager.direct_ref_target(&name).expect("read"),
        Some(fixture.seed.clone())
    );
}

/// `packed-refs` is read as Git writes it: the `# pack-refs with:` header and
/// the `^<peeled>` lines are skipped, every other line is `<id> <refname>`,
/// an absent file and an absent name both read as nothing.
#[test]
fn packed_refs_is_read_as_git_writes_it() {
    let fixture = Fixture::created("packed-refs-read");
    let packed = fixture.manager.common_git_dir.join("packed-refs");
    let name = run_ref("candidates/kalpha/1");
    assert_eq!(
        fixture.manager.packed_ref_value(&name).expect("no file"),
        None
    );
    write_file(
        &packed,
        format!(
            "# pack-refs with: peeled fully-peeled sorted \n{} refs/heads/main\n{} refs/tags/v1\n^{}\n{} {name}\n",
            fixture.head, fixture.side, fixture.seed, fixture.seed
        )
        .as_bytes(),
    );
    assert_eq!(
        fixture.manager.packed_ref_value(&name).expect("read"),
        Some(fixture.seed.clone())
    );
    assert_eq!(
        fixture
            .manager
            .packed_ref_value("refs/heads/main")
            .expect("read"),
        Some(fixture.head.clone())
    );
    assert_eq!(
        fixture
            .manager
            .packed_ref_value(&run_ref("candidates/kalpha/2"))
            .expect("read"),
        None,
        "a name the file does not list"
    );
    assert_eq!(
        fixture
            .manager
            .packed_ref_value(&fixture.seed)
            .expect("read"),
        None,
        "a peeled line is not a ref"
    );
}

/// The check a swap runs after it reclaimed a lock: `packed-refs` now holding
/// the ref at anything but the value just written means a `pack-refs` ran
/// during the reclaim, and its prune may remove the loose ref, so the swap is
/// refused rather than recorded. The same file holding the new value, or
/// nothing, is no such sign.
#[test]
fn a_swap_that_reclaimed_a_lock_refuses_when_packed_refs_then_holds_the_ref_elsewhere() {
    let fixture = Fixture::created("packed-refs-post-check");
    let packed = fixture.manager.common_git_dir.join("packed-refs");
    let name = run_ref("candidates/kalpha/1");

    fixture
        .manager
        .refuse_if_repacked_elsewhere(&name, &fixture.seed)
        .expect("no packed file, nothing to refuse");

    write_file(
        &packed,
        format!(
            "# pack-refs with: peeled fully-peeled sorted \n{} {name}\n",
            fixture.seed
        )
        .as_bytes(),
    );
    fixture
        .manager
        .refuse_if_repacked_elsewhere(&name, &fixture.seed)
        .expect("packed at the value just written: a pack-refs that ran after the swap");

    write_file(
        &packed,
        format!(
            "# pack-refs with: peeled fully-peeled sorted \n{} {name}\n",
            fixture.head
        )
        .as_bytes(),
    );
    let error = fixture
        .manager
        .refuse_if_repacked_elsewhere(&name, &fixture.seed)
        .expect_err("packed at the old value: a pack-refs that ran during the reclaim");
    let text = error.to_string();
    assert!(text.contains("packed-refs now holds it"), "{text}");
    assert!(
        text.contains(&fixture.head),
        "names the packed value: {text}"
    );
    assert!(text.contains(&fixture.seed), "and the one written: {text}");
}

/// Every `git update-ref` this module runs hands its child the run's cleanup
/// lease, and it is the one place the module spells that command: the
/// liveness fact `reclaim_own_ref_lock` rests on is made by the same
/// function that makes the write.
#[test]
fn the_one_update_ref_spawn_gives_its_child_the_cleanup_lease() {
    let source =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/workspace_manager.rs"))
            .expect("this module's source")
            // Normalised, so a checkout with Windows line endings reads the same
            // source as one without: the body search below ends on a newline.
            .replace("\r\n", "\n");
    let code = crate::effects::production_code(&source);
    assert_eq!(
        code.matches("hold_cleanup_lease_for_child(").count(),
        1,
        "the lease is handed to exactly one child"
    );
    let start = source
        .find("    fn update_ref(&self, args: &[&str])")
        .expect("the update-ref runner");
    let body = &source[start..];
    let end = body.find("\n    }\n").expect("the end of its body");
    let body = &body[..end];
    assert!(
        body.contains("hold_cleanup_lease_for_child("),
        "and that child is the update-ref"
    );
    assert!(
        body.contains("OsString::from(\"update-ref\")"),
        "which this function spells"
    );
    assert_eq!(
        source.matches("OsString::from(\"update-ref\")").count(),
        1,
        "and nothing else in the module does"
    );
}
