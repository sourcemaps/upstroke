// Allowlist placement: the funnel section of `effects/allowlist.toml`.
#![allow(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros
)]

use super::*;

// ---------------------------------------------------------------------------
// #329: the tolerant registry access
// (`48f2af7a:reviews/2026-10-01-pr11-follow-up-b-record.md`, §5 to §8 and its
// implementation section). The T-numbers are the record's.
// ---------------------------------------------------------------------------

/// A failed attempt of a contract witness, numbered, so that a refusal's text
/// can be checked against the attempt it carries.
fn attempt_failed(number: u32) -> UpstrokeError {
    UpstrokeError::Git {
        message: format!("attempt {number} failed"),
    }
}

/// A key no other test's access shares: R-X and `CONTENDED_ATTEMPTS` are keyed
/// by the common git dir as the caller passes it. The guard keeps it unique.
fn contract_key(tag: &str) -> (crate::rundir::scratch_tree::ScratchTree, PathBuf) {
    let tree = scratch(tag);
    let key = tree.path().join("common");
    (tree, key)
}

/// The message of a registry refusal, or a panic naming what came instead.
fn registry_refusal<T: std::fmt::Debug>(result: Result<T, UpstrokeError>, what: &str) -> String {
    match result {
        Err(UpstrokeError::RegistryRefused { message }) => message,
        other => panic!("{what}: expected a registry refusal, got {other:?}"),
    }
}

/// The `pause_for` of a witness on `clock`: each wait its access asks for
/// moves the clock by the wait's length and sleeps nothing, inside the
/// witness's containment.
fn clock_pause(
    clock: &super::RegistryClock,
    since: std::time::Instant,
) -> impl FnMut(std::time::Duration) -> Result<(), UpstrokeError> + '_ {
    move |pause| {
        contained(since, "a wait");
        clock.advance(pause);
        Ok(())
    }
}

/// Whether the clock table holds an entry for `common_git_dir`, read from the
/// table itself. A reading through `registry_now` cannot tell: an entry left
/// behind still reads ahead of the monotonic clock once the test has moved it
/// on (B17-1, the record's §9.34).
fn clock_entry_stands(common_git_dir: &Path) -> bool {
    super::REGISTRY_CLOCKS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .contains_key(common_git_dir)
}

/// B16 (the record's §9.33): a [`super::RegistryClock`] reaches only the
/// accesses of its own repository; it stands still while the host's time
/// passes and moves by exactly what it is moved by; nested guards share it,
/// and it goes with its last guard, on an unwinding as on a return. It takes
/// only its table's lock: nothing of it waits. That it goes is read from its
/// table, its entry absent after each last guard (B17-1, the record's §9.34).
#[test]
fn a_registry_clock_reaches_only_its_own_repository_and_goes_with_its_last_guard() {
    let (_tree, held) = contract_key("registry-clock-held");
    let (_other_tree, other) = contract_key("registry-clock-other");
    let before = std::time::Instant::now();
    let clock = super::RegistryClock::hold(&held);
    let reading = clock.now();
    assert!(
        reading >= before,
        "it starts at the monotonic clock's reading"
    );
    std::thread::sleep(std::time::Duration::from_millis(2));
    assert_eq!(
        super::registry_now(&held),
        reading,
        "it stands still while the host's time passes"
    );
    assert!(
        super::registry_now(&other) >= reading + std::time::Duration::from_millis(2),
        "another repository's accesses read the monotonic clock"
    );
    clock.advance(REGISTRY_ACCESS_DEADLINE);
    assert_eq!(
        super::registry_now(&held),
        reading + REGISTRY_ACCESS_DEADLINE,
        "it moves by exactly what it is moved by"
    );
    let nested = super::RegistryClock::hold(&held);
    assert_eq!(nested.now(), clock.now(), "a nested guard shares the clock");
    drop(clock);
    assert_eq!(
        nested.now(),
        reading + REGISTRY_ACCESS_DEADLINE,
        "the clock stays while a guard lives"
    );
    drop(nested);
    assert!(
        !clock_entry_stands(&held),
        "with its last guard gone, its repository's clock entry is gone"
    );
    let after = std::time::Instant::now();
    assert!(
        super::registry_now(&held) >= after,
        "with its last guard gone, its repository's accesses read the monotonic clock again"
    );
    let unwound = std::panic::catch_unwind(|| {
        let _clock = super::RegistryClock::hold(&held);
        panic!("a test that holds a clock fails");
    });
    assert!(unwound.is_err(), "the holder unwound");
    assert!(
        !clock_entry_stands(&held),
        "a guard dropped by an unwinding takes its clock's entry with it"
    );
    let later = std::time::Instant::now();
    assert!(
        super::registry_now(&held) >= later,
        "a guard dropped by an unwinding takes its clock with it"
    );
}

/// T4 (record §5.5, §6.4): a veto that answers `Return` after the first
/// failure returns that failure exactly as the attempt returned it, after one
/// attempt, the veto asked once and nothing counted as contended.
#[test]
fn a_registry_access_returns_a_vetoed_failure_unchanged_after_one_attempt() {
    let (_tree, key) = contract_key("registry-access-veto");
    let mut asked = 0_u32;
    let mut made = 0_u32;
    let result: Result<(), _> = tolerant_registry_access(
        &key,
        RegistryHold::Unheld,
        &mut crate::workspace_manager::sleep_for,
        &mut || {
            asked += 1;
            Again::Return
        },
        &mut || {
            made += 1;
            Err(attempt_failed(made))
        },
    );
    match result {
        Err(UpstrokeError::Git { message }) => assert_eq!(message, "attempt 1 failed"),
        other => panic!("a vetoed failure is returned as the attempt returned it: {other:?}"),
    }
    assert_eq!((made, asked), (1, 1), "one attempt, the veto asked once");
    assert_eq!(contended_attempts(&key), 0, "`Return` is not an `Attempt`");
}

/// T4: an access whose every attempt fails refuses at its deadline as a
/// registry refusal, never as Git state, naming the store, the deadline, the
/// attempt count and the last failure; every failure answered `Attempt` and was
/// counted, the last one included. The final attempt is the first that begins
/// at or after the deadline (§6.4, step 7): with attempts that take no time it
/// follows the sleep the deadline cut short, and the backoff fits sixteen in
/// 500 ms.
///
/// **On its repository's test clock** (the record's §9.33, B16): the deadline
/// and `took` are that [`super::RegistryClock`]'s readings, which only the
/// waits the access asks for move, so a stall of the host before the first
/// attempt makes that attempt no later on it. Wall time is held twice: to the
/// containment, and, as before, to the deadline and 5 s; a long enough stall of
/// the host can trip either. A refusal no earlier than the deadline in wall
/// time stays a real repository's
/// (`a_list_over_a_registration_half_written_refuses_at_its_deadline_and_is_never_git_state`).
#[test]
fn a_registry_access_that_always_fails_refuses_at_its_deadline_naming_the_count_and_the_last_failure()
 {
    let (_tree, key) = contract_key("registry-access-deadline");
    let clock = super::RegistryClock::hold(&key);
    let wall = std::time::Instant::now();
    let mut made = 0_u32;
    let started = clock.now();
    let result: Result<(), _> = tolerant_registry_access(
        &key,
        RegistryHold::Unheld,
        &mut clock_pause(&clock, wall),
        &mut || Again::Attempt,
        &mut || {
            contained(wall, "an attempt");
            made += 1;
            Err(attempt_failed(made))
        },
    );
    let took = clock.now().saturating_duration_since(started);
    let ran = wall.elapsed();
    contained(wall, "the refusal");
    let message = registry_refusal(result, "an access that always fails");
    assert!(
        ran < REGISTRY_ACCESS_DEADLINE + std::time::Duration::from_secs(5),
        "and within the deadline and 5 s of wall time: {ran:?}"
    );
    assert!(
        took >= REGISTRY_ACCESS_DEADLINE,
        "it refused only at its deadline, {REGISTRY_ACCESS_DEADLINE:?}: it took {took:?}"
    );
    assert!(
        took < REGISTRY_ACCESS_DEADLINE + std::time::Duration::from_secs(5),
        "and returned by the deadline plus its last attempt and veto, which take no time here: \
         {took:?}"
    );
    assert!(
        (2..=16).contains(&made),
        "the backoff of 1 ms doubling to 50 ms makes at most sixteen attempts in 500 ms, the \
         final one included: {made}"
    );
    assert!(
        message.contains(&key.join("worktrees").display().to_string())
            && message.contains(&format!("{REGISTRY_ACCESS_DEADLINE:?}"))
            && message.contains(&format!("{made} attempt(s)"))
            && message.ends_with(&format!("git error: attempt {made} failed")),
        "the refusal names the store, the deadline, the count and the last failure: {message}"
    );
    assert_eq!(
        contended_attempts(&key),
        usize::try_from(made).expect("a count fits"),
        "every failure answered `Attempt`, and each was counted"
    );
}

/// T4: two failures and then a success return the success, after three
/// attempts, with the two failures counted. On its repository's test clock
/// (the record's §9.33, B16), which only the waits the access asks for move,
/// so its attempts begin before the deadline however the host schedules them;
/// wall time is its containment.
#[test]
fn a_registry_access_passes_two_failures_and_returns_the_success_after_them() {
    let (_tree, key) = contract_key("registry-access-success");
    let clock = super::RegistryClock::hold(&key);
    let wall = std::time::Instant::now();
    let mut made = 0_u32;
    let result = tolerant_registry_access(
        &key,
        RegistryHold::Unheld,
        &mut clock_pause(&clock, wall),
        &mut || Again::Attempt,
        &mut || {
            contained(wall, "an attempt");
            made += 1;
            if made < 3 {
                Err(attempt_failed(made))
            } else {
                Ok(30_u32)
            }
        },
    );
    contained(wall, "the success");
    assert_eq!(
        result.expect("the third attempt succeeds"),
        30,
        "its own value"
    );
    assert_eq!(made, 3);
    assert_eq!(contended_attempts(&key), 2);
}

/// T4 (FUB-D6-DABSENCE): a veto that answers `Undecidable` refuses at once,
/// after one attempt, as a registry refusal naming why and carrying the
/// failure; never Git state, never attempted past, and not counted.
///
/// **"At once" is on its repository's test clock** (the record's §9.33, B16):
/// `took` is that [`super::RegistryClock`]'s, which only the waits the access
/// asks for move; the refusal asks for none, so its `took` on the clock is
/// zero, however the host schedules the test. Wall time is its containment. An
/// undecidable refusal at once in wall time is still asserted by a real
/// repository's test
/// (`an_add_whose_checkout_cannot_be_made_refuses_after_one_attempt_and_leaves_nothing`),
/// whose wall-time bound a long enough stall of the host can trip.
#[test]
fn an_undecidable_veto_refuses_at_once_naming_why() {
    let (_tree, key) = contract_key("registry-access-undecidable");
    let clock = super::RegistryClock::hold(&key);
    let wall = std::time::Instant::now();
    let mut made = 0_u32;
    let started = clock.now();
    let result: Result<(), _> = tolerant_registry_access(
        &key,
        RegistryHold::Unheld,
        &mut clock_pause(&clock, wall),
        &mut || Again::Undecidable {
            why: "the destination now holds a file".to_owned(),
        },
        &mut || {
            contained(wall, "an attempt");
            made += 1;
            Err(attempt_failed(made))
        },
    );
    let took = clock.now().saturating_duration_since(started);
    contained(wall, "the refusal");
    let message = registry_refusal(result, "an undecidable veto");
    assert_eq!(made, 1, "no attempt after an undecidable veto");
    assert!(
        took < REGISTRY_ACCESS_DEADLINE,
        "it refused at once, not at the deadline: {took:?}"
    );
    assert_eq!(
        took,
        std::time::Duration::ZERO,
        "it asked for no wait on its clock: {took:?}"
    );
    assert!(
        message.contains("the destination now holds a file")
            && message.contains("1 attempt(s)")
            && message.ends_with("git error: attempt 1 failed"),
        "{message}"
    );
    assert_eq!(contended_attempts(&key), 0);
}

/// T4 (the final attempt, FUD-D1-PROGRESS): a failure repaired by the
/// deadline is passed by the final attempt, the first made at or after the
/// deadline: after the deadline's cut-short sleep or, when the deadline passed
/// during the attempt before it, next, with no wait asked for before it (the
/// test below). Every attempt fails until the access's deadline, which is no
/// earlier than `REGISTRY_ACCESS_DEADLINE` after this test's clock was read;
/// only an attempt made at or after the deadline can succeed, and the contract
/// makes exactly one.
///
/// **On its repository's test clock** (the record's §9.33, B16, O-1): the
/// deadline, the repair and each attempt's reading are that
/// [`super::RegistryClock`]'s, which only the waits the access asks for move,
/// so a stall of the host before the first attempt makes that attempt no later
/// on it; the test build's 500 ms is the deadline on it. Wall time is its
/// containment.
#[test]
fn the_final_attempt_passes_a_failure_repaired_by_the_deadline() {
    let (_tree, key) = contract_key("registry-access-final");
    let clock = super::RegistryClock::hold(&key);
    let wall = std::time::Instant::now();
    let mut made = 0_u32;
    let repaired = clock.now() + REGISTRY_ACCESS_DEADLINE;
    let result = tolerant_registry_access(
        &key,
        RegistryHold::Unheld,
        &mut clock_pause(&clock, wall),
        &mut || Again::Attempt,
        &mut || {
            contained(wall, "an attempt");
            made += 1;
            if clock.now() >= repaired {
                Ok(made)
            } else {
                Err(attempt_failed(made))
            }
        },
    );
    contained(wall, "the final attempt");
    let last = result.expect("the final attempt, at the deadline, meets the repaired store");
    assert!(
        last >= 2,
        "the store was repaired only at the deadline, so earlier attempts failed: {last}"
    );
    assert_eq!(
        contended_attempts(&key),
        usize::try_from(last - 1).expect("a count fits"),
        "every attempt before the final one failed and answered `Attempt`"
    );
}

/// T4 (the final attempt, FUD-D1-PROGRESS): an attempt that began before the
/// deadline and failed after it is followed by the final attempt, with no wait
/// asked for before it, and a store repaired by the deadline is passed. The
/// first attempt reads the store before its repair and then lasts the
/// deadline's length and 100 ms more, so the nominal deadline passes while it
/// runs, as a slow Git command or a descheduled thread makes it pass.
///
/// **On its repository's test clock** (the record's §9.33, B16): the deadline,
/// the repair and each attempt's reading are that [`super::RegistryClock`]'s,
/// which only the first attempt's length and the waits the access asks for
/// move, so the first attempt begins before the deadline and reads the store
/// before its repair however the host schedules the test, and the deadline
/// passes during it. Wall time is its containment.
#[test]
fn the_final_attempt_follows_an_attempt_the_deadline_passed_during() {
    let (_tree, key) = contract_key("registry-access-straddle");
    let clock = super::RegistryClock::hold(&key);
    let wall = std::time::Instant::now();
    let mut made = 0_u32;
    let repaired = clock.now() + REGISTRY_ACCESS_DEADLINE;
    let result = tolerant_registry_access(
        &key,
        RegistryHold::Unheld,
        &mut clock_pause(&clock, wall),
        &mut || Again::Attempt,
        &mut || {
            contained(wall, "an attempt");
            made += 1;
            let whole = clock.now() >= repaired;
            if made == 1 {
                clock.advance(REGISTRY_ACCESS_DEADLINE + std::time::Duration::from_millis(100));
            }
            if whole {
                Ok(made)
            } else {
                Err(attempt_failed(made))
            }
        },
    );
    contained(wall, "the final attempt");
    assert_eq!(
        result.expect("the final attempt, after the deadline, meets the repaired store"),
        2,
        "the first attempt read the store before its repair; the second is the final one"
    );
    assert_eq!(
        contended_attempts(&key),
        1,
        "one failure, answered `Attempt`"
    );
}

/// T4: `contended_attempts` counts exactly the `Attempt` answers: two of them,
/// then a `Return`, count two; an `Undecidable` and a `Return` count nothing
/// (the two tests above). On its repository's test clock (the record's §9.33,
/// B16), so its attempts begin before the deadline however the host schedules
/// them; wall time is its containment.
#[test]
fn contended_attempts_counts_exactly_the_attempt_answers() {
    let (_tree, key) = contract_key("registry-access-counted");
    let clock = super::RegistryClock::hold(&key);
    let wall = std::time::Instant::now();
    let mut answers = [Again::Attempt, Again::Attempt, Again::Return].into_iter();
    let mut made = 0_u32;
    let result: Result<(), _> = tolerant_registry_access(
        &key,
        RegistryHold::Unheld,
        &mut clock_pause(&clock, wall),
        &mut || answers.next().unwrap_or(Again::Return),
        &mut || {
            contained(wall, "an attempt");
            made += 1;
            Err(attempt_failed(made))
        },
    );
    contained(wall, "the third failure");
    match result {
        Err(UpstrokeError::Git { message }) => assert_eq!(message, "attempt 3 failed"),
        other => panic!("the third failure is returned as it was: {other:?}"),
    }
    assert_eq!((made, contended_attempts(&key)), (3, 2));
}

/// T4 (FUB-D6-BOUND): a veto that lasts the deadline's length and 100 ms more,
/// after an attempt that began before the deadline, so that the nominal
/// deadline passes while it runs, is followed by the final attempt alone, with
/// no wait asked for before it; the access refuses after that attempt's veto.
/// Nothing bounds the attempts' or the vetoes' runtimes, or the scheduling
/// between them.
///
/// **On its repository's test clock** (the record's §9.33, B16): the deadline,
/// each attempt's start and `took` are that [`super::RegistryClock`]'s, which
/// only the vetoes' length and the waits the access asks for move, so the
/// first attempt begins before the deadline however the host schedules the
/// test, and the deadline passes during the first veto. Wall time is its
/// containment.
#[test]
fn a_veto_that_blocks_past_the_deadline_is_followed_by_the_final_attempt_alone() {
    let (_tree, key) = contract_key("registry-access-slow-veto");
    let clock = super::RegistryClock::hold(&key);
    let wall = std::time::Instant::now();
    let blocked = REGISTRY_ACCESS_DEADLINE + std::time::Duration::from_millis(100);
    let mut made = 0_u32;
    let mut began: Vec<std::time::Instant> = Vec::new();
    let started = clock.now();
    let result: Result<(), _> = tolerant_registry_access(
        &key,
        RegistryHold::Unheld,
        &mut clock_pause(&clock, wall),
        &mut || {
            contained(wall, "a veto");
            clock.advance(blocked);
            Again::Attempt
        },
        &mut || {
            contained(wall, "an attempt");
            made += 1;
            began.push(clock.now());
            Err(attempt_failed(made))
        },
    );
    let took = clock.now().saturating_duration_since(started);
    contained(wall, "the refusal");
    let message = registry_refusal(result, "an access whose veto blocked past its deadline");
    assert_eq!(
        made, 2,
        "the final attempt after the veto, and none after it"
    );
    assert!(
        began
            .get(1)
            .is_some_and(|second| *second >= started + REGISTRY_ACCESS_DEADLINE),
        "the final attempt began after the deadline: {began:?}"
    );
    assert!(
        took >= blocked + blocked,
        "it returned after the final attempt's veto: {took:?}"
    );
    assert!(message.contains("2 attempt(s)"), "{message}");
}

/// T4 and T9: with R-X held alone by another thread, as the torn-registration
/// plan holds it, a shared access (an add) refuses at its deadline with no
/// attempt run, and an unheld one (a list, a scan) runs at once. A shared
/// access beside another shared holder runs at once too, and an exclusive one
/// waits for the shared holder and refuses at its deadline.
///
/// **On its repository's test clock** (the record's §9.33, B16): each case's
/// `took` is that [`super::RegistryClock`]'s, which only the waits the access
/// asks for move. A case that waits for R-X reaches its deadline by those waits
/// alone, with no attempt run; a case that does not wait asks for none, and
/// its `took` on the clock is zero. A shared access beside a shared holder
/// returns `Ok(1)` while the holder's read guard is still held, which no access
/// that waited for or excluded that holder could. Wall time is its containment.
///
/// **A guarantee this test no longer makes** (the orchestrator's R-X
/// determination, B16): before B16 its cases that do not wait returned within
/// 500 ms of wall time (`took < REGISTRY_ACCESS_DEADLINE`, on the monotonic
/// clock). A real-time delay in the shared path that is not an R-X wait turn,
/// and still lets the access succeed while the holder is held, is not caught
/// here. R-X's wait refusing no earlier than its deadline, and an unheld list
/// beside R-X held alone not waiting, are still asserted in wall time by a real
/// repository's test
/// (`an_add_refuses_within_its_deadline_while_r_x_is_held_alone_and_a_list_does_not_wait`),
/// whose wall-time bounds a long enough stall of the host can trip.
#[test]
fn r_x_held_alone_refuses_a_shared_access_with_no_attempt_and_passes_an_unheld_one() {
    let (_tree, key) = contract_key("registry-access-rx");
    let clock = super::RegistryClock::hold(&key);
    let wall = std::time::Instant::now();
    let bound = std::time::Duration::from_secs(60);
    for (holder_alone, hold) in [
        (true, RegistryHold::Shared),
        (true, RegistryHold::Unheld),
        (false, RegistryHold::Shared),
        (false, RegistryHold::Exclusive),
    ] {
        let lock = registry_lock_of(&key);
        let (held, holding) = std::sync::mpsc::channel::<()>();
        let (release, released) = std::sync::mpsc::channel::<()>();
        let holder = std::thread::spawn(move || {
            if holder_alone {
                let _held = lock
                    .write()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let _ = held.send(());
                let _ = released.recv_timeout(bound);
            } else {
                let _held = lock
                    .read()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let _ = held.send(());
                let _ = released.recv_timeout(bound);
            }
        });
        holding
            .recv_timeout(bound)
            .expect("the other thread holds R-X");
        let mut made = 0_u32;
        let started = clock.now();
        let result = tolerant_registry_access(
            &key,
            hold,
            &mut clock_pause(&clock, wall),
            &mut || Again::Attempt,
            &mut || {
                contained(wall, "an attempt");
                made += 1;
                Ok(made)
            },
        );
        let took = clock.now().saturating_duration_since(started);
        let _ = release.send(());
        holder.join().expect("the holder thread");
        contained(wall, "a case");
        let waits = match hold {
            RegistryHold::Unheld => false,
            RegistryHold::Shared => holder_alone,
            RegistryHold::Exclusive => true,
        };
        if waits {
            let message = registry_refusal(
                result,
                &format!("a {hold:?} access beside R-X held (alone: {holder_alone})"),
            );
            assert_eq!(made, 0, "{hold:?}: no attempt ran");
            assert!(
                message.contains("no attempt ran")
                    && message.contains(&format!("{REGISTRY_ACCESS_DEADLINE:?}")),
                "{message}"
            );
            assert!(took >= REGISTRY_ACCESS_DEADLINE, "{hold:?}: {took:?}");
        } else {
            assert_eq!(
                took,
                std::time::Duration::ZERO,
                "{hold:?} (holder alone: {holder_alone}) asked for no wait on its clock; the \
                 access returned {result:?}"
            );
            assert_eq!(
                result.expect("the access runs beside the holder"),
                1,
                "{hold:?} (holder alone: {holder_alone})"
            );
            assert!(
                took < REGISTRY_ACCESS_DEADLINE,
                "{hold:?} (holder alone: {holder_alone}) did not wait: {took:?}"
            );
        }
    }
}

/// A foreign registration half written, as the design-recast lens of design
/// review round 3 built it (FUB-D3-TORNOK): `gitdir` written, `locked` holding
/// `initializing`, `HEAD` opened and empty, no `commondir` (Git 2.43's add
/// writes `locked`, `gitdir`, the checkout's `.git`, `HEAD`, then
/// `commondir`). Git exits 0 listing it and prints a record whose `HEAD` is
/// the zero id with neither `branch` nor `detached`, which the parser refuses.
fn plant_half_written_registration(fixture: &Fixture, name: &str) -> PathBuf {
    let admin = fixture
        .manager
        .common_git_dir()
        .join("worktrees")
        .join(name);
    let checkout = fixture.root.join(format!("{name}-checkout"));
    fs::create_dir_all(&admin).expect("the foreign registration's directory");
    fs::create_dir_all(&checkout).expect("its checkout");
    fs::write(admin.join("locked"), "initializing\n").expect("its locked");
    fs::write(
        admin.join("gitdir"),
        format!("{}\n", as_git_writes_it(&checkout.join(".git"))),
    )
    .expect("its gitdir");
    fs::write(
        checkout.join(".git"),
        format!("gitdir: {}\n", as_git_writes_it(&admin)),
    )
    .expect("its checkout's .git");
    fs::write(admin.join("HEAD"), b"").expect("its HEAD, opened and not written");
    admin
}

/// What a writer that finishes the registration [`plant_half_written_registration`]
/// made writes last: `HEAD` and `commondir`, and the `locked` it unlinks.
fn finish_registration(admin: &Path, head: &str) {
    fs::write(admin.join("HEAD"), format!("{head}\n")).expect("the writer writes HEAD");
    fs::write(admin.join("commondir"), "../..\n").expect("then commondir");
    fs::remove_file(admin.join("locked")).expect("then it unlocks");
}

/// Wait, with a watchdog, until an access over `common_git_dir` has answered
/// `Attempt` more times than `before`: the handshake a test uses to change the
/// store only after an access has failed on it at least once.
fn await_contended(common_git_dir: &Path, before: usize) -> usize {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        let now = contended_attempts(common_git_dir);
        if now > before {
            return now;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "no access over {} answered `Attempt` within 60 s",
            common_git_dir.display()
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

/// Runs `act` once, at the `Before` phase of `site`: after the add's gate
/// (its `revalidate`, whose list would meet a tear first) and before the
/// destination step and Git's add — where another process's registry write in
/// flight meets the add itself.
struct AtBefore<F: FnMut()> {
    site: EffectSiteId,
    act: Option<F>,
}

impl<F: FnMut()> AtBefore<F> {
    fn new(site: EffectSiteId, act: F) -> Self {
        Self {
            site,
            act: Some(act),
        }
    }
}

impl<F: FnMut()> EffectHooks for AtBefore<F> {
    fn phase(&mut self, site: EffectSiteId, phase: HookPhase) -> Injection {
        if site == self.site && phase == HookPhase::Before {
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

/// T11 (FUB-D3-TORNOK), static: a list over a registration half written that
/// nobody finishes is attempted again until its deadline and then refuses,
/// carrying the parser's refusal; never Git state.
#[test]
fn a_list_over_a_registration_half_written_refuses_at_its_deadline_and_is_never_git_state() {
    let fixture = Fixture::created("registry-tornok-static");
    let _alpha = fixture.add_task(&mut NoHooks, "alpha", 1);
    let admin = plant_half_written_registration(&fixture, "foreign-torn");
    let started = std::time::Instant::now();
    let result = fixture.manager.worktree_records();
    let took = started.elapsed();
    let message = registry_refusal(result, "a list over a half-written registration");
    assert!(took >= REGISTRY_ACCESS_DEADLINE, "{took:?}");
    assert!(
        message.contains("worktree list record"),
        "it carries the parser's refusal of the record Git printed: {message}"
    );
    fs::remove_dir_all(&admin).expect("the operator removes the registration");
    fixture
        .manager
        .worktree_records()
        .expect("the list passes once the registration is gone");
}

/// T11, transient (and T18's second shape, FUB-D5-OPTFILE's effect): the same
/// registration, finished by its writer only after the list has failed on it
/// once (the `CONTENDED_ATTEMPTS` handshake), is passed by a later attempt.
///
/// **Its list waits to the production deadline** (the record's §9.31, B14).
/// On the persistent Windows guest, under the suite's load, the list's first
/// attempt ended past the test build's 500 ms, and the access refused with no
/// attempt left for the registration its writer then finished.
#[test]
fn a_list_over_a_registration_half_written_passes_once_its_writer_finishes() {
    let fixture = Fixture::created("registry-tornok-transient");
    let _deadline = super::RegistryDeadline::hold(
        fixture.manager.common_git_dir(),
        PRODUCTION_REGISTRY_DEADLINE,
    );
    let _alpha = fixture.add_task(&mut NoHooks, "alpha", 1);
    let admin = plant_half_written_registration(&fixture, "foreign-transient");
    let common = fixture.manager.common_git_dir().to_path_buf();
    let before = contended_attempts(&common);
    let writer = {
        let (admin, head, common) = (admin.clone(), fixture.head.clone(), common.clone());
        std::thread::spawn(move || {
            let seen = await_contended(&common, before);
            finish_registration(&admin, &head);
            seen
        })
    };
    let records = fixture
        .manager
        .worktree_records()
        .expect("the list passes once the writer finishes");
    let seen = writer.join().expect("the writer");
    assert!(seen > before, "the list failed on the tear at least once");
    assert!(
        records
            .iter()
            .any(|record| record.path().ends_with("foreign-transient-checkout")),
        "and lists the finished registration: {records:?}"
    );
}

/// T11's control (§5.3: now a refusal): a whole registration nobody is
/// writing, whose `HEAD` names nothing. Git prints the record TORNOK's prefix
/// prints and the parser refuses it; nothing distinguishes it from a write in
/// flight, so it refuses at the deadline, resumably, where master returned it
/// as Git state at once.
#[test]
fn a_list_over_a_whole_registration_git_cannot_list_refuses_at_its_deadline() {
    let fixture = Fixture::created("registry-quiet-parse");
    let _alpha = fixture.add_task(&mut NoHooks, "alpha", 1);
    let admin = plant_half_written_registration(&fixture, "foreign-garbage");
    fs::remove_file(admin.join("locked")).expect("no lock");
    fs::write(admin.join("HEAD"), "garbage\n").expect("a HEAD that names nothing");
    fs::write(admin.join("commondir"), "../..\n").expect("its commondir");
    let started = std::time::Instant::now();
    let message = registry_refusal(
        fixture.manager.worktree_records(),
        "a list over a registration Git cannot list",
    );
    assert!(started.elapsed() >= REGISTRY_ACCESS_DEADLINE);
    assert!(message.contains("the last failed with"), "{message}");
}

/// The store's own registrations, for a before-and-after comparison: each
/// entry's name and the bytes of every file in it.
fn store_listing(fixture: &Fixture) -> std::collections::BTreeMap<String, Vec<u8>> {
    let store = fixture.manager.common_git_dir().join("worktrees");
    let mut listing = std::collections::BTreeMap::new();
    let Ok(entries) = fs::read_dir(&store) else {
        return listing;
    };
    for entry in entries {
        let entry = entry.expect("a store entry");
        for file in fs::read_dir(entry.path())
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
        {
            let name = format!(
                "{}/{}",
                entry.file_name().to_string_lossy(),
                file.file_name().to_string_lossy()
            );
            listing.insert(name, fs::read(file.path()).unwrap_or_default());
        }
    }
    listing
}

/// A store a mode bit makes unwritable for one test, restored when the guard
/// drops. Panics when the mode bit does not bind (root, or `CAP_DAC_OVERRIDE`).
#[cfg(unix)]
fn unwritable(path: &Path) -> RestoreMode {
    use std::os::unix::fs::PermissionsExt as _;
    let restore = RestoreMode {
        path: path.to_path_buf(),
    };
    fs::set_permissions(path, fs::Permissions::from_mode(0o555)).expect("make it unwritable");
    let probe = path.join("b329-probe-write");
    if fs::create_dir(&probe).is_ok() {
        fs::remove_dir(&probe).expect("remove the probe");
        panic!(
            "prerequisite not met: the mode bit did not bind (running as root or with \
             CAP_DAC_OVERRIDE); this test needs an unprivileged user"
        );
    }
    restore
}

/// T12 (FUB-D3-PERM): an add into a store nothing can write fails in Git's
/// registry phase, before Git takes its destination over, on every attempt; it
/// refuses at its deadline carrying Git's "Permission denied", and leaves
/// nothing at the slot. Once the store is writable the same add succeeds.
#[cfg(unix)]
#[test]
fn an_add_into_a_store_nothing_can_write_refuses_at_its_deadline_and_leaves_nothing_at_the_slot() {
    let fixture = Fixture::created("registry-perm");
    let _alpha = fixture.add_task(&mut NoHooks, "alpha", 1);
    let beta = fixture.task("beta", 1);
    fixture
        .manager
        .write_intent(&mut NoHooks, &beta)
        .expect("the intent");
    let store = fixture.manager.common_git_dir().join("worktrees");
    let common = fixture.manager.common_git_dir().to_path_buf();
    let before = contended_attempts(&common);
    let restore = unwritable(&store);
    let started = std::time::Instant::now();
    let result = fixture
        .manager
        .add_worktree(&mut NoHooks, &beta, &fixture.head);
    let took = started.elapsed();
    drop(restore);
    let message = registry_refusal(result, "an add into a store nothing can write");
    assert!(took >= REGISTRY_ACCESS_DEADLINE, "{took:?}");
    assert!(message.contains("Permission denied"), "{message}");
    assert!(
        contended_attempts(&common) > before,
        "the add was attempted again"
    );
    assert!(
        !fixture.manager.slot_path(&beta).exists(),
        "a refused add leaves no destination it made"
    );
    fixture
        .manager
        .add_worktree(&mut NoHooks, &beta, &fixture.head)
        .expect("the same add succeeds once the store is writable");
}

/// T6 at the Rust level: an add whose own registry phase fails once, naming
/// the add's own entry, before Git takes its destination over, and then can
/// pass. Git's prune of an add's entry before its `locked` exists is that
/// failure ("could not open '.git/worktrees/<name>/locked' for writing"),
/// executed at the Git level by design rounds 6 and 7 (`prune-own`); it needs a
/// pause inside Git, so the suite reproduces the class with the store refusing
/// the add's entry once ("could not create directory of
/// '.git/worktrees/<name>'"). The destination is untouched, so the add is
/// attempted again, and the store is made writable again only after the add
/// has failed once (the handshake). The add waits to the production deadline
/// (the record's §9.31, B14).
#[cfg(unix)]
#[test]
fn an_add_whose_own_entry_cannot_be_made_once_succeeds_on_a_later_attempt() {
    let fixture = Fixture::created("registry-own-entry-once");
    let _deadline = super::RegistryDeadline::hold(
        fixture.manager.common_git_dir(),
        PRODUCTION_REGISTRY_DEADLINE,
    );
    let _alpha = fixture.add_task(&mut NoHooks, "alpha", 1);
    let beta = fixture.task("beta", 1);
    fixture
        .manager
        .write_intent(&mut NoHooks, &beta)
        .expect("the intent");
    let store = fixture.manager.common_git_dir().join("worktrees");
    let common = fixture.manager.common_git_dir().to_path_buf();
    let before = contended_attempts(&common);
    let restore = unwritable(&store);
    let restorer = {
        let common = common.clone();
        std::thread::spawn(move || {
            let seen = await_contended(&common, before);
            drop(restore);
            seen
        })
    };
    let path = fixture
        .manager
        .add_worktree(&mut NoHooks, &beta, &fixture.head)
        .expect("the add passes once its own entry can be made");
    let seen = restorer.join().expect("the restorer");
    assert!(seen > before, "the add failed once before it passed");
    assert!(path.join(".git").is_file(), "and checked the slot out");
}

/// T14 (FUB-D4-OWNENTRY) through the production add: a registration of
/// another process carrying the add's own administrative name, half written
/// (`gitdir`, and a `commondir` opened and empty), on which the add's sibling
/// scan dies. Removed once the add has failed on it twice, the add passes on a
/// later attempt; kept, the add refuses at its deadline and leaves nothing at
/// the slot. Nothing tells this from the add's own failure but another
/// attempt, which the untouched destination licenses. The add whose entry is
/// removed waits to the production deadline (the record's §9.31, B14); the one
/// whose entry is kept refuses at the test build's.
#[test]
fn an_add_whose_sibling_scan_meets_a_torn_entry_of_its_own_name_is_attempted_past_it() {
    for (windows, label) in [(Some(2_usize), "two"), (None, "every")] {
        let fixture = Fixture::created(&format!("registry-own-entry-{label}"));
        let _deadline = windows.map(|_| {
            super::RegistryDeadline::hold(
                fixture.manager.common_git_dir(),
                PRODUCTION_REGISTRY_DEADLINE,
            )
        });
        let _alpha = fixture.add_task(&mut NoHooks, "alpha", 1);
        let beta = fixture.task("beta", 1);
        fixture
            .manager
            .write_intent(&mut NoHooks, &beta)
            .expect("the intent");
        let own = fixture
            .manager
            .slot_path(&beta)
            .file_name()
            .expect("a slot has a name")
            .to_os_string();
        let admin = fixture
            .manager
            .common_git_dir()
            .join("worktrees")
            .join(&own);
        let gitdir = fixture.root.join("foreign-same-name").join(".git");
        let mut hooks = AtBefore::new(beta.add_site(), || {
            fs::create_dir_all(&admin).expect("the foreign registration");
            fs::write(
                admin.join("gitdir"),
                format!("{}\n", as_git_writes_it(&gitdir)),
            )
            .expect("its gitdir");
            fs::write(admin.join("commondir"), b"").expect("its commondir, opened and not written");
        });
        let common = fixture.manager.common_git_dir().to_path_buf();
        let before = contended_attempts(&common);
        let remover = windows.map(|windows| {
            let (admin, common) = (admin.clone(), common.clone());
            std::thread::spawn(move || {
                let mut seen = before;
                while seen < before + windows {
                    seen = await_contended(&common, seen);
                }
                fs::remove_dir_all(&admin).expect("the foreign process removes its registration");
                seen
            })
        });
        let started = std::time::Instant::now();
        let result = fixture
            .manager
            .add_worktree(&mut hooks, &beta, &fixture.head);
        let took = started.elapsed();
        match remover {
            Some(remover) => {
                let seen = remover.join().expect("the remover");
                result.expect("the add passes once the torn entry is gone");
                assert!(seen >= before + 2, "{label}: it failed on the entry twice");
            }
            None => {
                let message = registry_refusal(result, "an add whose own name stays torn");
                assert!(took >= REGISTRY_ACCESS_DEADLINE, "{took:?}");
                assert!(message.contains("commondir"), "{message}");
                assert!(
                    !fixture.manager.slot_path(&beta).exists(),
                    "a refused add leaves no destination it made"
                );
                assert!(
                    admin.exists(),
                    "and the other process's entry is not its to touch"
                );
            }
        }
    }
}

/// `git` with `input` on its stdin, for the plumbing that builds a tree no
/// porcelain command will.
fn git_with_input(repo: &Path, args: &[&str], input: &[u8]) -> String {
    use std::io::Write as _;
    let mut child = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("run git");
    child
        .stdin
        .take()
        .expect("git's stdin")
        .write_all(input)
        .expect("write git's stdin");
    let output = child.wait_with_output().expect("git's output");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// FUB-D5-GENUINE's tree: a tree holding `.git/worktrees/fake-entry/file.txt`,
/// which no checkout may make. Git passes its registry phase, takes the
/// destination over, refuses the path in the checkout, and removes its junk.
fn tree_with_a_git_path(fixture: &Fixture) -> String {
    let blob = git_with_input(&fixture.base, &["hash-object", "-w", "--stdin"], b"x\n");
    let file = git_with_input(
        &fixture.base,
        &["mktree"],
        format!("100644 blob {blob}\tfile.txt\n").as_bytes(),
    );
    let entry = git_with_input(
        &fixture.base,
        &["mktree"],
        format!("040000 tree {file}\tfake-entry\n").as_bytes(),
    );
    let store = git_with_input(
        &fixture.base,
        &["mktree"],
        format!("040000 tree {entry}\tworktrees\n").as_bytes(),
    );
    git_with_input(
        &fixture.base,
        &["mktree"],
        format!("040000 tree {store}\t.git\n100644 blob {blob}\tREADME.md\n").as_bytes(),
    )
}

/// A tree holding one file whose name is 300 bytes, past every supported
/// filesystem's limit on one name: the checkout cannot make it.
fn tree_with_a_name_too_long(fixture: &Fixture) -> String {
    let blob = git_with_input(&fixture.base, &["hash-object", "-w", "--stdin"], b"x\n");
    let long = "n".repeat(300);
    git_with_input(
        &fixture.base,
        &["mktree"],
        format!("100644 blob {blob}\t{long}\n").as_bytes(),
    )
}

/// A commit of `tree` on `fixture.head`, for an add of it.
fn commit_of(fixture: &Fixture, tree: &str) -> String {
    git(
        &fixture.base,
        &["commit-tree", tree, "-p", &fixture.head, "-m", "b329"],
    )
}

/// T15 (record §7.8 and §8.10): an add whose checkout cannot be made fails
/// after Git took its destination over, and Git's junk removal took the
/// destination. Nothing outside Git tells that from another process's prune
/// deleting the add's entry after the takeover (§6.2), so it refuses at once,
/// after one attempt, as a registry refusal carrying Git's message, never as
/// Git state and never attempted again, with nothing left at the slot. That is
/// R14's narrowing (`PR329-A-GENUINE-CHECKOUT-FAILURE-AFTER-THE-TAKEOVER-REFUSES`).
#[test]
fn an_add_whose_checkout_cannot_be_made_refuses_after_one_attempt_and_leaves_nothing() {
    for (label, kind) in [("git-path", 0_u8), ("name-too-long", 1)] {
        if kind == 1 && !cfg!(unix) {
            continue;
        }
        let fixture = Fixture::created(&format!("registry-genuine-{label}"));
        let _alpha = fixture.add_task(&mut NoHooks, "alpha", 1);
        let tree = if kind == 0 {
            tree_with_a_git_path(&fixture)
        } else {
            tree_with_a_name_too_long(&fixture)
        };
        let commit = commit_of(&fixture, &tree);
        let beta = fixture.task("beta", 1);
        fixture
            .manager
            .write_intent(&mut NoHooks, &beta)
            .expect("the intent");
        let common = fixture.manager.common_git_dir().to_path_buf();
        let before = contended_attempts(&common);
        let started = std::time::Instant::now();
        let result = fixture.manager.add_worktree(&mut NoHooks, &beta, &commit);
        let took = started.elapsed();
        let message = registry_refusal(result, label);
        assert_eq!(
            contended_attempts(&common),
            before,
            "{label}: one attempt, never attempted again"
        );
        assert!(
            took < REGISTRY_ACCESS_DEADLINE,
            "{label}: refused at once: {took:?}"
        );
        assert!(
            message.contains("1 attempt(s)") && message.contains("is gone"),
            "{label}: {message}"
        );
        assert!(
            !fixture.manager.slot_path(&beta).exists(),
            "{label}: Git's junk removal took the destination"
        );
    }
}

/// T15's FUB-D6-INODE shapes (Unix): the destination's parent cannot be
/// written, so neither Git's junk removal nor this access can remove the
/// destination, and a failure after the takeover leaves it there, empty.
/// `inode-parent`: an existing empty destination, FUB-D5-GENUINE's tree.
/// `inode-both`: the destination itself cannot be written either, and a valid
/// commit. Each refuses after one attempt and the slot holds the empty
/// directory; with the parent writable again, the slot's forced removal takes
/// it, binding nothing.
#[cfg(unix)]
#[test]
fn an_add_whose_destination_cannot_be_removed_refuses_after_one_attempt_and_keeps_it() {
    use std::os::unix::fs::PermissionsExt as _;
    for (label, both) in [("inode-parent", false), ("inode-both", true)] {
        let fixture = Fixture::created(&format!("registry-{label}"));
        let _alpha = fixture.add_task(&mut NoHooks, "alpha", 1);
        let commit = if both {
            fixture.head.clone()
        } else {
            commit_of(&fixture, &tree_with_a_git_path(&fixture))
        };
        let beta = fixture.task("beta", 1);
        fixture
            .manager
            .write_intent(&mut NoHooks, &beta)
            .expect("the intent");
        let slot = fixture.manager.slot_path(&beta);
        fs::create_dir_all(&slot).expect("an existing empty destination");
        let restore_slot = RestoreMode { path: slot.clone() };
        if both {
            fs::set_permissions(&slot, fs::Permissions::from_mode(0o555))
                .expect("the destination unwritable");
        }
        let parent = slot.parent().expect("the slot's parent").to_path_buf();
        let restore = unwritable(&parent);
        let common = fixture.manager.common_git_dir().to_path_buf();
        let before = contended_attempts(&common);
        let result = fixture.manager.add_worktree(&mut NoHooks, &beta, &commit);
        let left = AtDestination::read(&slot);
        drop(restore);
        drop(restore_slot);
        let message = registry_refusal(result, label);
        assert_eq!(
            contended_attempts(&common),
            before,
            "{label}: one attempt, never attempted again"
        );
        assert!(
            message.contains("cannot remove"),
            "{label}: the removal proof failed: {message}"
        );
        assert!(
            matches!(left, AtDestination::EmptyDirectory),
            "{label}: the slot holds the empty directory"
        );
        fixture
            .manager
            .remove_worktree(&mut NoHooks, &beta)
            .expect("the forced removal takes an empty directory that binds nothing");
        assert!(!slot.exists(), "{label}");
    }
}

/// T20 (`torn-parent`, R′): a torn foreign registration fails the add's
/// sibling scan before Git takes the destination over, and the destination's
/// parent cannot be written, so the access cannot prove the destination
/// untouched by removing it. It refuses after one attempt, never Git; under
/// round 6's `Return` (option (i)) the scan's registry error would have come
/// back as Git state.
#[cfg(unix)]
#[test]
fn an_add_beside_a_torn_entry_whose_destination_cannot_be_removed_refuses_and_is_never_git() {
    let fixture = Fixture::created("registry-torn-parent");
    let _alpha = fixture.add_task(&mut NoHooks, "alpha", 1);
    let foreign = fixture.add_task(&mut NoHooks, "foreign", 1);
    let foreign_path = fixture.manager.slot_path(&foreign);
    let beta = fixture.task("beta", 1);
    fixture
        .manager
        .write_intent(&mut NoHooks, &beta)
        .expect("the intent");
    let slot = fixture.manager.slot_path(&beta);
    fs::create_dir_all(&slot).expect("an existing empty destination");
    let restore = unwritable(slot.parent().expect("the slot's parent"));
    let common = fixture.manager.common_git_dir().to_path_buf();
    let before = contended_attempts(&common);
    let mut hooks = AtBefore::new(beta.add_site(), || {
        tear_registration(&fixture.manager, &foreign_path);
    });
    let result = fixture
        .manager
        .add_worktree(&mut hooks, &beta, &fixture.head);
    drop(restore);
    let message = registry_refusal(result, "an add beside a torn entry");
    assert_eq!(contended_attempts(&common), before, "after one attempt");
    assert!(message.contains("commondir"), "{message}");
}

/// T16 (§8.2, FUB-D8-POPULATED): after the add's gate has passed, a
/// destination that is not an empty directory when the access begins is Git
/// state at once, before `git worktree add` runs. The reviewers' construction:
/// beside it, a sibling torn — after the gate's list, at the add's `Before`
/// hook — so that Git's sibling scan, which runs before Git checks the
/// destination, would die on it. The answer names the destination, never the
/// sibling's `commondir`; the store and what was at the destination are
/// untouched. A sibling already torn when the gate lists the registry is met
/// by the gate first
/// (`a_populated_destination_beside_a_registration_already_torn_meets_the_gate_first`).
#[test]
fn an_add_whose_destination_is_not_an_empty_directory_is_git_state_before_git_worktree_add() {
    for (label, file) in [("directory-holding-entries", false), ("file", true)] {
        let fixture = Fixture::created(&format!("registry-populated-{label}"));
        let foreign = fixture.add_task(&mut NoHooks, "foreign", 1);
        let foreign_path = fixture.manager.slot_path(&foreign);
        let beta = fixture.task("beta", 1);
        fixture
            .manager
            .write_intent(&mut NoHooks, &beta)
            .expect("the intent");
        let slot = fixture.manager.slot_path(&beta);
        if file {
            fs::write(&slot, b"a file at the slot\n").expect("a file at the destination");
        } else {
            fs::create_dir_all(&slot).expect("the destination");
            fs::write(slot.join("leftover"), b"x\n").expect("an entry in it");
        }
        let common = fixture.manager.common_git_dir().to_path_buf();
        let before = contended_attempts(&common);
        let store = std::cell::RefCell::new(None);
        let mut hooks = AtBefore::new(beta.add_site(), || {
            tear_registration(&fixture.manager, &foreign_path);
            *store.borrow_mut() = Some(store_listing(&fixture));
        });
        let started = std::time::Instant::now();
        let error = fixture
            .manager
            .add_worktree(&mut hooks, &beta, &fixture.head)
            .expect_err("a populated destination is refused");
        let took = started.elapsed();
        let store = store
            .into_inner()
            .expect("the sibling was torn at the add's Before hook");
        match &error {
            UpstrokeError::Git { message } => {
                assert!(
                    message.contains(&slot.display().to_string())
                        && message.contains("`git worktree add` did not run")
                        && !message.contains("commondir"),
                    "{label}: names the destination, not the sibling: {message}"
                );
            }
            other => panic!("{label}: Git state, not {other:?}"),
        }
        assert!(
            took < REGISTRY_ACCESS_DEADLINE,
            "{label}: at once: {took:?}"
        );
        assert_eq!(contended_attempts(&common), before, "{label}: no attempt");
        assert_eq!(
            store_listing(&fixture),
            store,
            "{label}: the store is untouched"
        );
        if file {
            assert_eq!(fs::read(&slot).expect("the file"), b"a file at the slot\n");
        } else {
            assert!(slot.join("leftover").exists(), "{label}: the entry is kept");
        }
    }
}

/// T16, the order (the record's §9.13, R4): the add's gate lists the registry
/// before the destination is read. So a destination already populated beside
/// a sibling already torn meets the sibling first: the gate's list is attempted
/// again to its deadline and refuses as the registry's, naming the sibling's
/// `commondir`, never Git state naming the destination, and what is at the
/// destination stays. The populated answer of
/// `an_add_whose_destination_is_not_an_empty_directory_is_git_state_before_git_worktree_add`
/// holds after a gate that passed.
#[test]
fn a_populated_destination_beside_a_registration_already_torn_meets_the_gate_first() {
    let fixture = Fixture::created("registry-populated-gate-first");
    let foreign = fixture.add_task(&mut NoHooks, "foreign", 1);
    let beta = fixture.task("beta", 1);
    fixture
        .manager
        .write_intent(&mut NoHooks, &beta)
        .expect("the intent");
    let slot = fixture.manager.slot_path(&beta);
    fs::create_dir_all(&slot).expect("the destination");
    fs::write(slot.join("paid-output"), b"keep\n").expect("an entry in it");
    tear_registration(&fixture.manager, &fixture.manager.slot_path(&foreign));
    let common = fixture.manager.common_git_dir().to_path_buf();
    let before = contended_attempts(&common);
    let result = fixture
        .manager
        .add_worktree(&mut NoHooks, &beta, &fixture.head);
    let message = registry_refusal(result, "a populated destination beside a torn sibling");
    assert!(
        message.contains("commondir") && !message.contains(&slot.display().to_string()),
        "the gate's list names the sibling, never the destination: {message}"
    );
    assert!(
        contended_attempts(&common) > before,
        "the gate's list was attempted again"
    );
    assert_eq!(
        fs::read(slot.join("paid-output")).expect("the destination's entry"),
        b"keep\n",
        "what was at the destination stays"
    );
}

/// T16, the link case (§8.2): Git's own check follows a link to an empty
/// directory and checks out through it, and the destination step refuses it
/// before `git worktree add` runs. Through the funnel a link at the slot is
/// refused first by the
/// acted-through walk (`SlotCheckoutEntry`), so the destination step is
/// witnessed directly; the walk's refusal is witnessed beside it.
#[cfg(unix)]
#[test]
fn a_destination_that_is_a_link_to_an_empty_directory_is_refused_before_git() {
    let fixture = Fixture::created("registry-destination-link");
    let beta = fixture.task("beta", 1);
    fixture
        .manager
        .write_intent(&mut NoHooks, &beta)
        .expect("the intent");
    let slot = fixture.manager.slot_path(&beta);
    let target = fixture.root.join("link-target");
    fs::create_dir_all(&target).expect("an empty directory");
    fs::create_dir_all(slot.parent().expect("the slot's parent")).expect("tasks/");
    std::os::unix::fs::symlink(&target, &slot).expect("a link at the slot");
    match Destination::prepare(&slot) {
        Err(UpstrokeError::Git { message }) => assert!(
            message.contains("a link or reparse point")
                && message.contains("`git worktree add` did not run"),
            "{message}"
        ),
        Err(other) => panic!("Git state, not {other:?}"),
        Ok(_) => panic!("a link to an empty directory is not a destination this access made"),
    }
    let error = fixture
        .manager
        .add_worktree(&mut NoHooks, &beta, &fixture.head)
        .expect_err("the funnel refuses a link at the slot");
    assert!(
        refusal_of(&error).contains("symlink or reparse point"),
        "the acted-through walk refuses it first: {}",
        refusal_of(&error)
    );
    assert!(
        fs::read_dir(&target)
            .expect("the link's target")
            .next()
            .is_none(),
        "nothing was written through the link"
    );
}

/// T16: a destination that cannot be made is Git state at once, naming the
/// path and the OS error, before `git worktree add` runs: where Git's own add
/// would have failed to make it, so a verification defers on it as before.
#[cfg(unix)]
#[test]
fn an_add_whose_destination_cannot_be_made_is_git_state_at_once() {
    let fixture = Fixture::created("registry-destination-unmakeable");
    let _alpha = fixture.add_task(&mut NoHooks, "alpha", 1);
    let beta = fixture.task("beta", 1);
    fixture
        .manager
        .write_intent(&mut NoHooks, &beta)
        .expect("the intent");
    let slot = fixture.manager.slot_path(&beta);
    let restore = unwritable(slot.parent().expect("the slot's parent"));
    let common = fixture.manager.common_git_dir().to_path_buf();
    let before = contended_attempts(&common);
    let result = fixture
        .manager
        .add_worktree(&mut NoHooks, &beta, &fixture.head);
    drop(restore);
    match result {
        Err(UpstrokeError::Git { message }) => assert!(
            message.contains(&slot.display().to_string())
                && message.contains("could not be made")
                && message.contains("`git worktree add` did not run"),
            "{message}"
        ),
        other => panic!("Git state at once, not {other:?}"),
    }
    assert_eq!(contended_attempts(&common), before, "no attempt");
}

/// T16: after an untouched failure the destination is a new empty directory,
/// removed and made again by the removal proof. The destination is made by
/// hand first with a mode no `mkdir` here gives, a torn sibling fails the
/// first attempt, and once the access has answered `Attempt` the destination
/// is read: an empty directory with the ordinary mode, which only its removal
/// and making again explain. The access is held at that answer until the
/// destination has been read ([`hold_next_contended`]), so no second attempt
/// and veto can remove it under the read. Then the sibling is repaired, the
/// access released, and the add passes. The add waits to the production
/// deadline (the record's §9.31, B14).
#[cfg(unix)]
#[test]
fn after_an_untouched_failure_the_destination_is_removed_and_made_again() {
    use std::os::unix::fs::PermissionsExt as _;
    let fixture = Fixture::created("registry-destination-again");
    let _deadline = super::RegistryDeadline::hold(
        fixture.manager.common_git_dir(),
        PRODUCTION_REGISTRY_DEADLINE,
    );
    let foreign = fixture.add_task(&mut NoHooks, "foreign", 1);
    let foreign_path = fixture.manager.slot_path(&foreign);
    let foreign_admin = super::fixture::registration_of(&fixture.manager, &foreign_path);
    let beta = fixture.task("beta", 1);
    fixture
        .manager
        .write_intent(&mut NoHooks, &beta)
        .expect("the intent");
    let slot = fixture.manager.slot_path(&beta);
    fs::create_dir_all(&slot).expect("an existing empty destination");
    fs::set_permissions(&slot, fs::Permissions::from_mode(0o711)).expect("a mark on it");
    let common = fixture.manager.common_git_dir().to_path_buf();
    let before = contended_attempts(&common);
    let release = hold_next_contended(&common);
    let observer = {
        let (common, slot, foreign_admin) = (common.clone(), slot.clone(), foreign_admin.clone());
        std::thread::spawn(move || {
            await_contended(&common, before);
            let mode = fs::symlink_metadata(&slot)
                .map(|metadata| metadata.permissions().mode() & 0o777)
                .ok();
            let empty = matches!(AtDestination::read(&slot), AtDestination::EmptyDirectory);
            fs::write(foreign_admin.join("commondir"), "../..\n").expect("repair the sibling");
            fs::remove_file(foreign_admin.join("locked")).expect("and unlock it");
            drop(release);
            (mode, empty)
        })
    };
    let mut hooks = AtBefore::new(beta.add_site(), || {
        tear_registration(&fixture.manager, &foreign_path);
    });
    fixture
        .manager
        .add_worktree(&mut hooks, &beta, &fixture.head)
        .expect("the add passes once the sibling is whole");
    let (mode, empty) = observer.join().expect("the observer");
    assert!(
        empty,
        "after the untouched failure the destination is an empty directory"
    );
    assert_ne!(
        mode,
        Some(0o711),
        "and a new one: the removal proof removed the marked directory and made it again"
    );
}

/// T17 (§5.3, round 6's form): an intent, an empty directory at its slot and
/// no registration store at all — what a coordinator killed after making an
/// add's destination leaves when the repository has no other linked checkout.
/// The forced removal binds nothing for the empty directory and takes it,
/// where master's scan answered the store's absence as an I/O error on every
/// attempt. A populated slot whose `.git` names no entry of this
/// repository's store still refuses with no store; one whose `.git` names an
/// entry of it converges (closure 3 of the external-prune follow-up,
/// `a_missing_stored_worktree_directory_converges_removing_only_the_checkout`).
#[test]
fn a_removal_with_no_store_takes_an_empty_destination_and_still_refuses_a_checkout() {
    let fixture = Fixture::created("registry-store-absent");
    let beta = fixture.task("beta", 1);
    fixture
        .manager
        .write_intent(&mut NoHooks, &beta)
        .expect("the intent");
    let slot = fixture.manager.slot_path(&beta);
    fs::create_dir_all(&slot).expect("the empty destination");
    let store = fixture.manager.common_git_dir().join("worktrees");
    let _ = fs::remove_dir(&store);
    assert!(!store.exists(), "the repository has no registration store");
    fixture
        .manager
        .remove_worktree(&mut NoHooks, &beta)
        .expect("the forced removal takes the empty destination");
    assert!(!slot.exists(), "and the slot is gone");

    fs::create_dir_all(&slot).expect("a populated slot");
    fs::write(
        slot.join(".git"),
        b"gitdir: /nowhere/.git/worktrees/kbeta-g1\n",
    )
    .expect("a checkout's .git file");
    let message = registry_refusal(
        fixture.manager.remove_worktree(&mut NoHooks, &beta),
        "a populated slot with no store",
    );
    assert!(
        message.contains("failed to read"),
        "the scan's I/O refusal: {message}"
    );
    assert!(slot.join(".git").exists(), "nothing was removed");
}

/// T22 (§6.5, FUB-D6-STATICRESUME): `WorkspaceManager::derive` over a
/// registration Git's list dies on — an add killed while it wrote `commondir`
/// — refuses as a registry refusal after its deadline, and writes nothing; the
/// operator's remedy (remove the registration directory and the checkout it
/// names) lets it derive again. `PR5-RD-002-RESUME-DERIVES-THROUGH-A-TORN-ENUMERATION`
/// is retained: this changes only the refusal's variant and its delay.
#[test]
fn derive_over_a_registration_the_list_dies_on_refuses_and_the_operators_remedy_clears_it() {
    let fixture = Fixture::created("registry-static-derive");
    let alpha = fixture.add_task(&mut NoHooks, "alpha", 1);
    let checkout = fixture.manager.slot_path(&alpha);
    let admin = tear_registration(&fixture.manager, &checkout);
    let before = tree_bytes(&fixture.root);
    let started = std::time::Instant::now();
    let message = registry_refusal(
        WorkspaceManager::derive(
            &fixture.base,
            &fixture.private,
            super::fixture::RUN_ID,
            "inc-2",
        ),
        "derive over a torn registration",
    );
    assert!(started.elapsed() >= REGISTRY_ACCESS_DEADLINE);
    assert!(message.contains("commondir"), "{message}");
    assert_eq!(tree_bytes(&fixture.root), before, "derive wrote nothing");
    fs::remove_dir_all(&admin).expect("the operator removes the registration directory");
    fs::remove_dir_all(&checkout).expect("and the checkout it names");
    WorkspaceManager::derive(
        &fixture.base,
        &fixture.private,
        super::fixture::RUN_ID,
        "inc-2",
    )
    .expect("derive passes once the registration is gone");
}

/// Write `script` as an executable file at `path`.
#[cfg(unix)]
fn executable(path: &Path, script: &str) {
    use std::os::unix::fs::PermissionsExt as _;
    fs::write(path, script).expect("the script");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("executable");
}

/// Configure a required smudge filter named `driver` for every path of the
/// repository, through `info/attributes`, so that any checkout of any tree
/// runs it: `smudge` is the command.
#[cfg(unix)]
fn require_smudge_filter(fixture: &Fixture, driver: &str, smudge: &Path) {
    let info = fixture.manager.common_git_dir().join("info");
    fs::create_dir_all(&info).expect("info/");
    fs::write(info.join("attributes"), format!("* filter={driver}\n")).expect("info/attributes");
    git(
        &fixture.base,
        &[
            "config",
            &format!("filter.{driver}.smudge"),
            &smudge.to_string_lossy(),
        ],
    );
    git(
        &fixture.base,
        &["config", &format!("filter.{driver}.clean"), "cat"],
    );
    git(
        &fixture.base,
        &["config", &format!("filter.{driver}.required"), "true"],
    );
}

/// T23 (§7.8): a failure after Git took the destination over is refused, not
/// returned and not attempted again. A prune of the add's entry after the
/// takeover needs a pause inside Git and is executed at the Git level (§6.2,
/// §7.3); the suite reproduces its end state with a required smudge filter that
/// fails its first run only. The add refuses after one attempt, carrying the
/// filter's failure; under option (i) it would be Git state after one attempt,
/// and under option (ii′) Ok after two.
#[cfg(unix)]
#[test]
fn a_failure_after_the_takeover_is_refused_not_returned_and_not_attempted_again() {
    let fixture = Fixture::created("registry-after-takeover");
    let _alpha = fixture.add_task(&mut NoHooks, "alpha", 1);
    let marker = fixture.root.join("b329-filter-ran");
    let script = fixture.root.join("b329-fails-once.sh");
    executable(
        &script,
        &format!(
            "#!/bin/sh\nif [ -e '{}' ]; then exec cat; fi\n: > '{}'\necho 'b329: the smudge \
             filter refuses its first run' >&2\nexit 1\n",
            marker.display(),
            marker.display()
        ),
    );
    require_smudge_filter(&fixture, "b329once", &script);
    let beta = fixture.task("beta", 1);
    fixture
        .manager
        .write_intent(&mut NoHooks, &beta)
        .expect("the intent");
    let common = fixture.manager.common_git_dir().to_path_buf();
    let before = contended_attempts(&common);
    let result = fixture
        .manager
        .add_worktree(&mut NoHooks, &beta, &fixture.head);
    let message = registry_refusal(result, "an add whose checkout's filter failed once");
    assert!(marker.exists(), "the filter ran, inside the add's checkout");
    assert_eq!(
        contended_attempts(&common),
        before,
        "one attempt, never attempted again"
    );
    assert!(
        message.contains("1 attempt(s)") && message.contains("b329once"),
        "it carries the filter's failure: {message}"
    );
}

/// T13 (R-X shared): every add's checkout is held past the test deadline by a
/// slow smudge filter, and four threads add and remove snapshots at once. Adds
/// hold R-X shared and never wait for each other, so every cycle completes; a
/// mutex taken by every access under a bounded wait refused cycles like these
/// (record §3.4: 3 of 120 of PR11's concurrency test).
#[cfg(unix)]
#[test]
fn adds_whose_checkouts_outlast_the_deadline_do_not_wait_for_each_other() {
    const THREADS: u32 = 4;
    const ROUNDS: u32 = 2;
    let fixture = Fixture::created("registry-rx-shared");
    let script = fixture.root.join("b329-slow.sh");
    executable(&script, "#!/bin/sh\nsleep 0.7\nexec cat\n");
    require_smudge_filter(&fixture, "b329slow", &script);
    let failures: Vec<String> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..THREADS)
            .map(|thread| {
                let manager = fixture.manager.clone();
                let head = fixture.head.clone();
                scope.spawn(move || {
                    let mut failures = Vec::new();
                    for round in 1..=ROUNDS {
                        let name = SnapshotName::gates(thread, 0, round);
                        let started = std::time::Instant::now();
                        match manager.add_snapshot(
                            &mut NoHooks,
                            &name,
                            &SnapshotInput::Commit(oid(&head)),
                        ) {
                            Ok(snapshot) => {
                                if started.elapsed() < REGISTRY_ACCESS_DEADLINE {
                                    failures.push(format!(
                                        "adding {name} took {:?}, inside the deadline: the \
                                         filter did not hold it",
                                        started.elapsed()
                                    ));
                                }
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
    assert!(failures.is_empty(), "{failures:#?}");
}

/// T9 at the manager (record §3.4): while R-X is held alone by another thread,
/// as the torn-registration plan holds it, an add refuses within its deadline
/// plus one backoff, with no Git child run and nothing left at its slot, and a
/// list runs at once. The same add succeeds once R-X is released.
#[test]
fn an_add_refuses_within_its_deadline_while_r_x_is_held_alone_and_a_list_does_not_wait() {
    let fixture = Fixture::created("registry-rx-held");
    let beta = fixture.task("beta", 1);
    fixture
        .manager
        .write_intent(&mut NoHooks, &beta)
        .expect("the intent");
    let bound = std::time::Duration::from_secs(60);
    let lock = registry_lock_of(fixture.manager.common_git_dir());
    let (held, holding) = std::sync::mpsc::channel::<()>();
    let (release, released) = std::sync::mpsc::channel::<()>();
    let holder = std::thread::spawn(move || {
        let _held = lock
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let _ = held.send(());
        let _ = released.recv_timeout(bound);
    });
    holding.recv_timeout(bound).expect("R-X held alone");
    let started = std::time::Instant::now();
    let added = fixture
        .manager
        .add_worktree(&mut NoHooks, &beta, &fixture.head);
    let add_took = started.elapsed();
    let started = std::time::Instant::now();
    let listed = fixture.manager.worktree_records();
    let list_took = started.elapsed();
    let _ = release.send(());
    holder.join().expect("the holder");
    let message = registry_refusal(added, "an add while R-X is held alone");
    assert!(message.contains("no attempt ran"), "{message}");
    assert!(
        add_took >= REGISTRY_ACCESS_DEADLINE
            && add_took < REGISTRY_ACCESS_DEADLINE + std::time::Duration::from_secs(5),
        "the add refused at its deadline: {add_took:?}"
    );
    assert!(
        !fixture.manager.slot_path(&beta).exists(),
        "and left no destination it made"
    );
    listed.expect("the list runs beside R-X");
    assert!(
        list_took < REGISTRY_ACCESS_DEADLINE,
        "the list takes no R-X and does not wait: {list_took:?}"
    );
    fixture
        .manager
        .add_worktree(&mut NoHooks, &beta, &fixture.head)
        .expect("the add passes once R-X is released");
}

/// T5 (§2.5, §3.5): no engine removal prunes. A registration of another
/// process caught between its `mkdir` and its `locked` — an entry with neither
/// `locked` nor `gitdir`, which `git worktree prune` deletes whatever its age —
/// survives a removal of an engine slot, which removes its own registration
/// directly. The store goes only with its last registration: a removal that
/// leaves another entry keeps it, and the last one's removal takes it, as
/// `git worktree prune` did.
#[test]
fn no_removal_prunes_another_processs_registration_and_the_store_goes_only_when_empty() {
    let fixture = Fixture::created("registry-no-prune");
    let alpha = fixture.add_task(&mut NoHooks, "alpha", 1);
    let beta = fixture.add_task(&mut NoHooks, "beta", 1);
    let store = fixture.manager.common_git_dir().join("worktrees");
    let in_flight = store.join("foreign-add-in-flight");
    fs::create_dir_all(&in_flight).expect("another process's add, before its locked");
    fixture
        .manager
        .remove_worktree(&mut NoHooks, &alpha)
        .expect("remove alpha");
    assert!(
        in_flight.is_dir(),
        "the other process's entry in flight survives the removal"
    );
    fs::remove_dir(&in_flight).expect("the other process's add fails and removes its junk");
    // Git names a registration after its checkout's basename, which is
    // beta's instance under this incarnation's tag.
    let beta_admin = store.join(
        fixture
            .manager
            .slot_path(&beta)
            .file_name()
            .expect("an instance path has a final component"),
    );
    assert!(beta_admin.is_dir(), "beta is still registered");
    fixture
        .manager
        .remove_worktree(&mut NoHooks, &beta)
        .expect("remove beta");
    assert!(
        !store.exists(),
        "the last registration's removal took the emptied store"
    );
}

/// T5's census: no production argv of this module names `prune`. A tripwire
/// over the production region's code lines; the witness above is the
/// behaviour.
#[test]
fn no_production_argv_of_the_manager_names_prune() {
    let source =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/workspace_manager.rs"))
            .expect("this module's source")
            .replace("\r\n", "\n");
    let region = crate::effects::production_region(&source);
    let named: Vec<&str> = region
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .filter(|line| line.contains("\"prune\""))
        .collect();
    assert!(named.is_empty(), "{named:#?}");
}

/// The `B329_RESULT` line of linked child `number`, read through the
/// `B329_CYCLE` line it says after each of its `cycles` cycles, each within
/// `bound` of the cycle line before it ([`super::fixture::LINK_BOUND`] in T1
/// and T3); a panic carrying the child's stderr otherwise, or when a round
/// arrives out of order or the result before the last round.
///
/// **Why the bound restarts at each cycle** (the record's §9.30, B13). The
/// link's bound only turns a silent peer into a failure, and a child that is
/// still cycling is not silent. Read with [`line_from`], the result's one
/// deadline timed the whole run instead, which is the runner's speed and not
/// the registry's: merge-queue run 37834058719 failed T1 on hosted
/// `windows-latest` at that deadline, with no result line and an empty
/// stderr, and on the persistent Windows guest under load every child was
/// still cycling when the same deadline ended T1 and T3. A child that stops is
/// still caught, `bound` after the last cycle it reported, and the message
/// says how many that was.
fn result_through_cycles(
    lines: &std::sync::mpsc::Receiver<String>,
    number: u32,
    cycles: u32,
    stderr: &Path,
    bound: std::time::Duration,
) -> String {
    let mut reported = 0_u32;
    let mut deadline = std::time::Instant::now() + bound;
    loop {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        let line = match lines.recv_timeout(left) {
            Ok(line) => line,
            Err(error) => panic!(
                "no `B329_RESULT` line from the linked child ({error}): process {number} \
                 reported {reported} of its {cycles} cycles and then nothing for {bound:?}; \
                 its stderr:\n{}",
                fs::read_to_string(stderr).unwrap_or_default()
            ),
        };
        if let Some(result) = line.find("B329_RESULT").and_then(|at| line.get(at..)) {
            assert_eq!(
                reported, cycles,
                "process {number} reported its result after {reported} of {cycles} cycles: {result}"
            );
            return result.to_owned();
        }
        if let Some(cycle) = line.find("B329_CYCLE ").and_then(|at| line.get(at..)) {
            reported += 1;
            assert!(
                reported <= cycles && cycle == format!("B329_CYCLE {number} {reported}"),
                "process {number} said `{cycle}` where cycle {reported} of {cycles} was due"
            );
            deadline = std::time::Instant::now() + bound;
        }
    }
}

/// T1 and T3: `processes` coordinators of one repository, each in its own
/// checkout — the first in the main checkout, the others in linked checkouts
/// added with plain Git — each holding its own worktree lock and run lock,
/// run their registry work at once. Each one's R-X is its own; tolerance is
/// what keeps every cycle from failing on another's write in flight. Each
/// child's result is read through its cycle lines
/// ([`result_through_cycles`]), so the link's bound ends a child that stops
/// and never one still cycling, however long its cycles take on the runner.
fn registry_cycles_across(tag: &str, processes: u32, cycles: u32) {
    let fixture = Fixture::created(tag);
    let mut children = Vec::new();
    for number in 0..processes {
        let base = if number == 0 {
            fixture.base.clone()
        } else {
            let linked = fixture.root.join(format!("linked-{number}"));
            git_os(
                &fixture.base,
                &[
                    OsStr::new("worktree"),
                    OsStr::new("add"),
                    OsStr::new("-q"),
                    OsStr::new("--detach"),
                    linked.as_os_str(),
                    OsStr::new(&fixture.head),
                ],
            );
            linked
        };
        let stderr = fixture.root.join(format!("child-{number}.stderr"));
        let number_text = number.to_string();
        let cycles_text = cycles.to_string();
        let (child, lines) = super::fixture::LinkedChild::spawn(
            "workspace_manager::tests::registry_cycles_child",
            &[
                ("B329_BASE", base.as_os_str()),
                ("B329_PRIVATE", fixture.private.as_os_str()),
                ("B329_NUMBER", OsStr::new(&number_text)),
                ("B329_CYCLES", OsStr::new(&cycles_text)),
            ],
            &stderr,
        );
        line_from(&lines, "B329_READY", &stderr);
        children.push((number, child, lines, stderr));
    }
    for (_, child, _, _) in &children {
        assert!(child.send("GO"), "every process is told to start");
    }
    let mut failed = Vec::new();
    for (number, child, lines, stderr) in &children {
        let result =
            result_through_cycles(lines, *number, cycles, stderr, super::fixture::LINK_BOUND);
        loop {
            let line = line_from(lines, "B329_", stderr);
            if line.starts_with("B329_DONE") {
                break;
            }
            failed.push(line);
        }
        let status = child
            .wait_within(super::fixture::LINK_BOUND)
            .unwrap_or_else(|| {
                panic!(
                    "process {number} did not end; its stderr:\n{}",
                    fs::read_to_string(stderr).unwrap_or_default()
                )
            });
        assert!(
            status.success() && result.ends_with("failures=0"),
            "process {number}: {result}, {status:?}; its failures: {failed:#?}"
        );
    }
    assert!(failed.is_empty(), "{failed:#?}");
    assert!(
        processes * cycles >= if cfg!(windows) { 300 } else { 1_000 },
        "the test runs the cycles it claims"
    );
}

/// T1 (PR11 review round 7's witness, `R7-CONC-1`): two processes, one in
/// the main checkout and one in a linked checkout, 500 add-and-remove cycles
/// each through the production funnels, and not one fails. Unpatched, round
/// 3 measured it red 10 of 10, 68 failed operations in 20,000 (the record's
/// §2.4). Fewer cycles on Windows, where each Git command costs more.
#[test]
fn two_coordinators_in_two_checkouts_of_one_repository_never_fail_on_each_others_registry_writes() {
    registry_cycles_across(
        "registry-two-processes",
        2,
        if cfg!(windows) { 150 } else { 500 },
    );
}

/// T3: T1's shape with three processes, one in the main checkout and two in
/// linked checkouts.
#[test]
fn three_coordinators_in_three_checkouts_of_one_repository_never_fail_on_each_others_registry_writes()
 {
    registry_cycles_across(
        "registry-three-processes",
        3,
        if cfg!(windows) { 100 } else { 340 },
    );
}

/// B13 (the record's §9.30): T1's and T3's wait restarts its bound at each
/// cycle line, so a child whose cycles together outlast the bound, none of
/// them near it, is read to its result. Eight cycle lines 400 ms apart, then
/// the result, take at least 3.2 s against a 2 s bound: the one deadline
/// [`line_from`] keeps from its call refuses that whatever the scheduling,
/// and this refuses it only if a line comes 2 s after the one before it, five
/// times its interval.
///
/// **The speaker starts only once the test's clock is read** (the record's
/// §9.33, B16-2): it waits for a start signal the test sends after `started`,
/// so none of its eight intervals falls before the measurement begins, however
/// long the test is descheduled after spawning it. That wait is bounded by
/// [`super::fixture::LINK_BOUND`], a containment and not the oracle.
#[test]
fn a_childs_result_is_read_through_cycles_whose_sum_outlasts_the_bound() {
    let (say, lines) = std::sync::mpsc::channel::<String>();
    let (go, told) = std::sync::mpsc::channel::<()>();
    let speaker = std::thread::spawn(move || {
        if told.recv_timeout(super::fixture::LINK_BOUND).is_err() {
            return;
        }
        for round in 1..=8 {
            std::thread::sleep(std::time::Duration::from_millis(400));
            if say.send(format!("B329_CYCLE 0 {round}")).is_err() {
                return;
            }
        }
        let _ = say.send("B329_RESULT 0 cycles=8 failures=0".to_owned());
    });
    let started = std::time::Instant::now();
    go.send(())
        .expect("the speaker is told to start once the clock is read");
    let result = result_through_cycles(
        &lines,
        0,
        8,
        Path::new(""),
        std::time::Duration::from_secs(2),
    );
    let took = started.elapsed();
    speaker.join().expect("the speaker");
    assert_eq!(result, "B329_RESULT 0 cycles=8 failures=0");
    assert!(
        took >= std::time::Duration::from_millis(3_200),
        "the cycles together outlasted the bound: {took:?}"
    );
}

/// B13 (the record's §9.30): a child that stops cycling is still caught, the
/// bound after the last cycle it reported, and its failure says how many that
/// was. Three cycle lines wait in the channel and its sender stays open, so
/// only the bound ends the wait, as it does for a child that hangs.
#[test]
#[should_panic(
    expected = "(timed out waiting on channel): process 0 reported 3 of its 10 cycles and then nothing for 100ms"
)]
fn a_child_that_stops_cycling_fails_naming_the_cycles_it_reported() {
    let (say, lines) = std::sync::mpsc::channel::<String>();
    for round in 1..=3 {
        say.send(format!("B329_CYCLE 0 {round}"))
            .expect("a cycle line");
    }
    let _result = result_through_cycles(
        &lines,
        0,
        10,
        Path::new(""),
        std::time::Duration::from_millis(100),
    );
    drop(say);
}

/// B13 (the record's §9.30): a result before the last cycle fails at once, so
/// a child is held to the cycles its test claims.
#[test]
#[should_panic(expected = "process 1 reported its result after 2 of 3 cycles")]
fn a_result_before_the_last_cycle_fails() {
    let (say, lines) = std::sync::mpsc::channel::<String>();
    for line in [
        "B329_CYCLE 1 1",
        "B329_CYCLE 1 2",
        "B329_RESULT 1 cycles=3 failures=0",
    ] {
        say.send(line.to_owned()).expect("a line");
    }
    let _result = result_through_cycles(
        &lines,
        1,
        3,
        Path::new(""),
        std::time::Duration::from_secs(2),
    );
}

/// B13 (the record's §9.30): a cycle line out of order fails at once, so the
/// wait reads at most one line per cycle.
#[test]
#[should_panic(expected = "process 1 said `B329_CYCLE 1 3` where cycle 2 of 10 was due")]
fn a_cycle_line_out_of_order_fails() {
    let (say, lines) = std::sync::mpsc::channel::<String>();
    for line in ["B329_CYCLE 1 1", "B329_CYCLE 1 3"] {
        say.send(line.to_owned()).expect("a line");
    }
    let _result = result_through_cycles(
        &lines,
        1,
        10,
        Path::new(""),
        std::time::Duration::from_secs(2),
    );
}

/// A [`registry_writer_child`] for `fixture`'s repository, its entry named
/// `name`, once it has said its registration is torn.
fn torn_by_another_process(
    fixture: &Fixture,
    name: &str,
) -> (
    std::sync::Arc<super::fixture::LinkedChild>,
    std::sync::mpsc::Receiver<String>,
    PathBuf,
    PathBuf,
) {
    let admin = fixture
        .manager
        .common_git_dir()
        .join("worktrees")
        .join(name);
    let checkout = fixture.root.join(format!("{name}-checkout"));
    let stderr = fixture.root.join(format!("{name}.stderr"));
    let (child, lines) = super::fixture::LinkedChild::spawn(
        "workspace_manager::tests::registry_writer_child",
        &[
            ("B329_ADMIN", admin.as_os_str()),
            ("B329_CHECKOUT", checkout.as_os_str()),
            ("B329_HEAD", OsStr::new(&fixture.head)),
        ],
        &stderr,
    );
    line_from(&lines, "B329_TORN", &stderr);
    (child, lines, admin, stderr)
}

/// Claim 1, another process's write in flight, across two processes: a list
/// meets a registration another process is half way through writing, fails on
/// it, and — the writer told to finish only once the list has failed on it
/// (the `CONTENDED_ATTEMPTS` handshake) — passes on a later attempt. The list
/// waits to the production deadline (the record's §9.31, B14).
#[test]
fn a_list_passes_a_registration_another_process_finishes_writing() {
    let fixture = Fixture::created("registry-writer-in-flight");
    let _deadline = super::RegistryDeadline::hold(
        fixture.manager.common_git_dir(),
        PRODUCTION_REGISTRY_DEADLINE,
    );
    let _alpha = fixture.add_task(&mut NoHooks, "alpha", 1);
    let (child, lines, _admin, stderr) = torn_by_another_process(&fixture, "other-process");
    let common = fixture.manager.common_git_dir().to_path_buf();
    let before = contended_attempts(&common);
    let teller = {
        let (child, common) = (std::sync::Arc::clone(&child), common.clone());
        std::thread::spawn(move || {
            let seen = await_contended(&common, before);
            assert!(child.send("FINISH"), "the writer is told to finish");
            seen
        })
    };
    let records = fixture
        .manager
        .worktree_records()
        .expect("the list passes once the other process finishes");
    let seen = teller.join().expect("the teller");
    line_from(&lines, "B329_FINISHED", &stderr);
    assert!(
        seen > before,
        "the list failed on the write in flight first"
    );
    assert!(
        records
            .iter()
            .any(|record| record.path().ends_with("other-process-checkout")),
        "{records:?}"
    );
    child
        .wait_within(super::fixture::LINK_BOUND)
        .expect("the writer ends");
}

/// Claim 1, a write a dead process left torn, across two processes: the writer
/// is killed (the bounded `LinkedChild::kill`) inside its registration write.
/// Every access over the store then fails on the torn entry on every attempt:
/// a list and an add each refuse at their deadline, resumably, never as Git
/// state, and the add leaves nothing at its slot. The operator's remedy —
/// remove the registration and the checkout it names, once no process writes
/// it — clears it.
#[test]
fn accesses_over_a_registration_a_dead_process_left_torn_refuse_and_are_never_git_state() {
    let fixture = Fixture::created("registry-dead-writer");
    let _alpha = fixture.add_task(&mut NoHooks, "alpha", 1);
    let beta = fixture.task("beta", 1);
    fixture
        .manager
        .write_intent(&mut NoHooks, &beta)
        .expect("the intent");
    let (child, _lines, admin, _stderr) = torn_by_another_process(&fixture, "dead-writer");
    let _status = child.kill();
    assert!(
        admin.join("locked").exists(),
        "the dead writer left its entry torn"
    );
    let message = registry_refusal(
        fixture.manager.worktree_records(),
        "a list over a dead writer's torn entry",
    );
    assert!(message.contains("commondir"), "{message}");
    let message = registry_refusal(
        fixture
            .manager
            .add_worktree(&mut NoHooks, &beta, &fixture.head),
        "an add over a dead writer's torn entry",
    );
    assert!(message.contains("commondir"), "{message}");
    assert!(!fixture.manager.slot_path(&beta).exists());
    fs::remove_dir_all(&admin).expect("the operator removes the registration");
    fs::remove_dir_all(fixture.root.join("dead-writer-checkout")).expect("and its checkout");
    fixture
        .manager
        .add_worktree(&mut NoHooks, &beta, &fixture.head)
        .expect("the add passes once the torn entry is gone");
}

/// Claim 1, a registration nobody is writing, across two processes: the other
/// process leaves a whole registration Git cannot list and ends. A list over
/// it refuses at its deadline, never as Git state.
#[test]
fn a_list_over_a_registration_another_process_left_whole_and_unlistable_refuses() {
    let fixture = Fixture::created("registry-nobody-writes");
    let _alpha = fixture.add_task(&mut NoHooks, "alpha", 1);
    let (child, lines, admin, stderr) = torn_by_another_process(&fixture, "left-unlistable");
    assert!(child.send("WHOLE-UNLISTABLE"));
    line_from(&lines, "B329_WHOLE", &stderr);
    child
        .wait_within(super::fixture::LINK_BOUND)
        .expect("the other process ends");
    assert!(!admin.join("locked").exists(), "nobody is writing it");
    let message = registry_refusal(
        fixture.manager.worktree_records(),
        "a list over a whole registration Git cannot list",
    );
    assert!(message.contains("the last failed with"), "{message}");
}

/// T10 (the legacy writer beside the manager): the legacy engine's gate
/// snapshots, added and dropped in a linked checkout by four threads, beside
/// the manager's snapshot cycles in the main checkout on four threads. The
/// legacy accesses are follow-up D's and are counted, not asserted; the
/// manager has no failure.
///
/// **Its accesses wait to the production deadline** (the record's §9.30.10,
/// B13). Under the test build's 500 ms, one attempt the platform makes slow
/// can spend the whole deadline: on the persistent Windows guest, under the
/// suite's load, an add's one attempt failed on another add's registration in
/// flight after the deadline had passed, and the access refused with no second
/// attempt, which is the retry this test exists to exercise.
#[test]
fn the_manager_never_fails_beside_the_legacy_engines_gate_snapshots() {
    const THREADS: u32 = 4;
    const ROUNDS: u32 = 40;
    let fixture = Fixture::created("registry-legacy-beside");
    let _deadline = super::RegistryDeadline::hold(
        fixture.manager.common_git_dir(),
        PRODUCTION_REGISTRY_DEADLINE,
    );
    let linked = fixture.root.join("legacy-checkout");
    git_os(
        &fixture.base,
        &[
            OsStr::new("worktree"),
            OsStr::new("add"),
            OsStr::new("-q"),
            OsStr::new("--detach"),
            linked.as_os_str(),
            OsStr::new(&fixture.head),
        ],
    );
    let legacy = crate::workspace::Workspace::open(&linked).expect("the legacy workspace");
    let tree = git(&linked, &["rev-parse", "HEAD^{tree}"]);
    let parent = git(&linked, &["rev-parse", "HEAD"]);
    let legacy_failures = std::sync::atomic::AtomicUsize::new(0);
    let failures: Vec<String> = std::thread::scope(|scope| {
        for thread in 0..THREADS {
            let (legacy, tree, parent, counted) = (&legacy, &tree, &parent, &legacy_failures);
            let store = fixture.root.join(format!("legacy-store-{thread}"));
            scope.spawn(move || {
                for _ in 0..ROUNDS {
                    match legacy.gate_snapshot_for_candidate_in_store(parent, tree, &store) {
                        Ok(snapshot) => drop(snapshot),
                        Err(_) => {
                            counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        }
                    }
                }
            });
        }
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
            .flat_map(|worker| worker.join().expect("a manager thread"))
            .collect()
    });
    eprintln!(
        "b329 T10: legacy gate snapshot failures beside the manager: {} of {}",
        legacy_failures.load(std::sync::atomic::Ordering::SeqCst),
        THREADS * ROUNDS
    );
    assert!(failures.is_empty(), "{failures:#?}");
}

/// T18 (FUB-D5-SPELLING): T14's interleaving on a repository whose `.git` is a
/// link to a directory of another name. Nothing reads Git's text, so no
/// spelling of the store can be missed: the add is attempted past its own
/// name's torn entry. It waits to the production deadline (the record's
/// §9.31, B14).
#[cfg(unix)]
#[test]
fn an_add_in_a_repository_whose_git_dir_is_a_link_is_attempted_past_a_torn_entry_of_its_name() {
    let tree = scratch("registry-spelling");
    let root = tree.path();
    let base = root.join("repo");
    let private = root.join("private");
    fs::create_dir_all(&base).expect("repo");
    fs::create_dir_all(&private).expect("private root");
    git(&base, &["init", "-q", "-b", "main", "--object-format=sha1"]);
    git(&base, &["config", "user.email", "tests@upstroke.local"]);
    git(&base, &["config", "user.name", "upstroke tests"]);
    git(&base, &["config", "core.autocrlf", "false"]);
    fs::write(base.join("a.txt"), "one\n").expect("a file");
    git(&base, &["add", "-A"]);
    git(&base, &["commit", "-q", "-m", "seed"]);
    let head = git(&base, &["rev-parse", "HEAD"]);
    fs::rename(base.join(".git"), root.join("elsewhere.gitdir")).expect("move the git dir");
    std::os::unix::fs::symlink(root.join("elsewhere.gitdir"), base.join(".git"))
        .expect("link .git to a directory of another name");
    fs::create_dir_all(crate::rundir::public_dir(&base, super::fixture::RUN_ID))
        .expect("public dir");
    let manager = WorkspaceManager::derive(&base, &private, super::fixture::RUN_ID, "inc-1")
        .expect("a manager over it");
    let _deadline =
        super::RegistryDeadline::hold(manager.common_git_dir(), PRODUCTION_REGISTRY_DEADLINE);
    manager
        .create_execution_root(&mut NoHooks)
        .expect("the execution root");
    let beta = Slot::Task {
        key: "beta".to_owned(),
        generation: 1,
    };
    manager
        .write_intent(&mut NoHooks, &beta)
        .expect("the intent");
    let own = manager
        .slot_path(&beta)
        .file_name()
        .expect("a slot has a name")
        .to_os_string();
    let admin = manager.common_git_dir().join("worktrees").join(&own);
    let gitdir = root.join("foreign-same-name").join(".git");
    let mut hooks = AtBefore::new(beta.add_site(), || {
        fs::create_dir_all(&admin).expect("the foreign registration");
        fs::write(
            admin.join("gitdir"),
            format!("{}\n", as_git_writes_it(&gitdir)),
        )
        .expect("its gitdir");
        fs::write(admin.join("commondir"), b"").expect("its commondir, opened and not written");
    });
    let common = manager.common_git_dir().to_path_buf();
    let before = contended_attempts(&common);
    let remover = {
        let (admin, common) = (admin.clone(), common.clone());
        std::thread::spawn(move || {
            let seen = await_contended(&common, before);
            fs::remove_dir_all(&admin).expect("the foreign process removes its registration");
            seen
        })
    };
    manager
        .add_worktree(&mut hooks, &beta, &head)
        .expect("the add passes once the torn entry is gone");
    assert!(remover.join().expect("the remover") > before);
}

/// `PR328-LINKED-CHILD-KILL-WAITS-WITHOUT-A-DEADLINE`: a `LinkedChild` whose
/// kill did not end the child — here `kill` itself refused by a seccomp policy,
/// standing for a child a tracer holds at its exit stop or one in
/// uninterruptible sleep — fails its test within its collection bound, saying
/// the child is still not collectable, instead of waiting for the child; and
/// its drop does the same rather than hang. The isolated caller runs under a
/// bound of its own, so a mutation that removes either bound fails here rather
/// than wedging the suite.
#[cfg(target_os = "linux")]
#[test]
fn a_linked_childs_kill_fails_its_test_within_its_bound_rather_than_wait_for_a_child_its_kill_did_not_end()
 {
    let ended = crate::agent::proc::test_support::run_test_isolated(
        "workspace_manager::tests::linked_child_whose_kill_is_refused_child",
        &[],
        std::time::Duration::from_secs(90),
    );
    let after = |what: &str| {
        ended
            .stdout
            .split(&format!("the {what} ended after "))
            .nth(1)
            .and_then(|rest| rest.split(" ms").next())
            .and_then(|ms| ms.parse::<u64>().ok())
            .map(std::time::Duration::from_millis)
    };
    let (kill, drop) = (after("kill"), after("drop"));
    assert!(
        ended.status.is_some_and(|status| status.success()) && kill.is_some() && drop.is_some(),
        "the isolated caller ended and said when its kill and its drop did: {ended}"
    );
    let bound = super::fixture::COLLECT_BOUND * 3;
    assert!(
        kill.is_some_and(|took| took < bound)
            && drop.is_some_and(|took| took < bound)
            && ended.stdout.matches("still not collectable").count() == 2,
        "a kill that did not end its child must fail its test within its collection bound, and \
         so must the drop: {ended}"
    );
}
