// Allowlist placement: the **funnel section** of `effects/allowlist.toml`, by
// attachment to `src/rundir.rs` -- the shape `src/runner/container/tests.rs`
// established for a funnel's own test module. This suite plants husks and
// residue with raw `fs` calls, forks and locks real descriptors, and re-execs
// this test binary, so it names those primitives directly.
//
// `PR6-LANEF-004`: a Rust lint level is scoped by the MODULE TREE and not by
// the file, so without an attribute here the parent's inner allow would reach
// this file silently and no reviewed record would name the file doing the work.
// All three are needed and all three are measured; none is inherited.
// `decisions.effect_site_inventory.mechanism` (2).
#![allow(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros
)]

use super::*;
// The split moved the classification probe into a child. `use super::*` reaches the
// parent's namespace only, so the five items this suite drives directly --
// `FIRST_LINE_WINDOW`, `SCAN_CHUNK`, `first_line`, `first_line_within` and
// `RunStartedHeader` -- are reached through the child's own `pub(super)` surface, and
// the three `std::io` traits it drove them with are named here rather than borrowed
// from the parent's import list, which no longer needs them. No test is renamed, no
// assertion changes and no body moves; these two lines are the whole of what the
// extraction owes this file.
use super::classify::*;

use std::io::{Read, Seek, SeekFrom};
use std::time::{Duration, Instant};

use crate::agent::proc::test_support::readiness;
#[cfg(unix)]
use crate::workspace_manager::fixture::{rest_within, say_on_stderr};

/// A scratch tree for one test.
///
/// `pub(crate)` for the sibling suite in `discovery.rs`: the fixtures live here
/// because this file carries the funnel allowance that `std::fs::create_dir_all`
/// and `std::fs::write` need, and `discovery.rs` denies all three governed
/// lints and takes no allowlist row. What crosses the boundary is a built
/// directory, never a primitive.
pub(crate) fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("upstroke-rundir-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

/// Make `<repo>/.upstroke/runs/<run_id>` a husk: a directory whose log holds no
/// committed `run_started`. `pub(crate)` for the reason [`scratch`] is.
pub(crate) fn husk_run(repo: &Path, run_id: &str) -> PathBuf {
    let public = public_dir(repo, run_id);
    fs::create_dir_all(&public).expect("husk dir");
    fs::write(public.join(EVENT_LOG), b"{\"event\":\"other\"}\n").expect("an uncommitted log");
    public
}

/// Put a question in a committed run's `questions` directory. `pub(crate)` for
/// the reason [`scratch`] is.
pub(crate) fn question_in(repo: &Path, run_id: &str, question_id: &str) {
    let dir = commit_run(repo, run_id).join("questions");
    fs::create_dir_all(&dir).expect("questions dir");
    fs::write(dir.join(format!("{question_id}.json")), "{}").expect("question");
}

fn paths_in(root: &Path, run_id: &str) -> RunPaths {
    RunPaths::with_private_root(&root.join("repo"), run_id, &root.join("home"))
}

/// The exact bytes of a committed first line, written by hand.
///
/// Not `serde_json::to_string(&Event::now(…))`: the classifier is judged
/// against the **wire**, and a fixture that serialized through the same
/// types the classifier reads would agree with any symmetric change to
/// both (`PR3-WIRE-PINNING`). Every field here is one the packet names —
/// the `event` tag, and `schema` and `run_id` inside `data`, which is what
/// recovery step (a0) means by "probe the header of the committed first
/// line … select the engine by schema".
fn committed_line(run_id: &str, schema: u32) -> String {
    format!(
        "{{\"ts\":\"2026-08-20T00:00:00Z\",\"event\":\"run_started\",\
             \"data\":{{\"schema\":{schema},\"run_id\":\"{run_id}\",\
             \"branch\":\"upstroke/run-{run_id}\"}}}}"
    )
}

/// Make `<repo>/.upstroke/runs/<run_id>` a committed run. `pub(crate)` for the
/// reason [`scratch`] is.
pub(crate) fn commit_run(repo: &Path, run_id: &str) -> PathBuf {
    let public = public_dir(repo, run_id);
    fs::create_dir_all(&public).expect("run dir");
    fs::write(
        public.join(EVENT_LOG),
        format!("{}\n", committed_line(run_id, 3)),
    )
    .expect("committed first line");
    public
}

#[test]
fn fresh_runs_with_equal_ids_cannot_share_private_artifacts() {
    let root = scratch_tree::acquire(&std::env::temp_dir(), "equal-run-ids")
        .expect("a root owned by this regression test");
    let private_root = root.path().join("home");
    let run_id = "01M191Y2PSVX78RNEP31D23K02";
    let first = RunPaths::with_private_root(&root.path().join("repo-a"), run_id, &private_root);
    let second = RunPaths::with_private_root(&root.path().join("repo-b"), run_id, &private_root);
    assert_ne!(first.public, second.public);
    assert_eq!(first.private, second.private);
    first
        .create_fresh()
        .expect("the first run allocates both halves");
    let transcript = first.transcripts().join("task-1.json");
    fs::write(&transcript, b"first run transcript").expect("first run transcript is present");

    let error = second
        .create_fresh()
        .expect_err("the occupied private half must refuse a fresh run");
    assert!(error.to_string().contains("private"), "{error}");
    assert_eq!(
        fs::read(&transcript).expect("the first run's transcript survives"),
        b"first run transcript"
    );
    assert!(
        scratch_tree::proves_absent(&second.public),
        "the refused run leaves no public husk"
    );
}

#[test]
fn an_occupied_public_root_is_preserved_and_its_private_reservation_is_removed() {
    let root = scratch_tree::acquire(&std::env::temp_dir(), "occupied-public-run")
        .expect("a root owned by this regression test");
    let paths = paths_in(root.path(), "OCCUPIED");
    fs::create_dir_all(&paths.public).expect("an older public run exists");
    fs::write(paths.events(), b"older event log").expect("the older run has content");

    let error = paths
        .create_fresh()
        .expect_err("fresh creation refuses the old public root");
    assert!(error.to_string().contains("public"), "{error}");
    assert_eq!(
        fs::read(paths.events()).expect("the old log survives"),
        b"older event log"
    );
    assert!(
        scratch_tree::proves_absent(&paths.private),
        "the unused private reservation is removed"
    );
}

#[test]
fn ensuring_existing_run_directories_preserves_resume_contents() {
    let root = scratch_tree::acquire(&std::env::temp_dir(), "resume-run-dirs")
        .expect("a root owned by this regression test");
    let paths = paths_in(root.path(), "RESUME");
    paths.create_fresh().expect("fresh creation succeeds");
    fs::write(paths.events(), b"existing event log").expect("existing log");
    paths
        .create()
        .expect("resume may ensure existing skeleton directories");
    assert_eq!(
        fs::read(paths.events()).expect("read resumed log"),
        b"existing event log"
    );
}

#[test]
fn agent_authored_files_land_outside_the_workspace() {
    // The whole point of the split: a reviewer with read access to the
    // repo has no path to the implementer's transcript.
    let root = scratch("split");
    let paths = paths_in(&root, "RUN1");
    paths.create().expect("create");

    let repo = root.join("repo");
    for private in [
        paths.transcripts(),
        paths.reviews(),
        paths.settings(),
        paths.gates(),
        paths.gate_worktrees(),
    ] {
        assert!(private.is_dir(), "{} should exist", private.display());
        assert!(
            !private.starts_with(&repo),
            "{} must not be inside the workspace",
            private.display()
        );
    }
    for public in [paths.questions(), paths.answers(), paths.artifacts()] {
        assert!(
            public.starts_with(&repo),
            "ops surface stays beside the repo"
        );
    }
    assert_eq!(
        paths.events(),
        repo.join(".upstroke/runs/RUN1/events.jsonl")
    );
}

#[test]
fn the_private_fallback_is_never_the_workspace() {
    // No HOME is a bad day, not a reason to quietly put transcripts back
    // where an agent can read them.
    let root = default_private_root();
    assert!(
        root.ends_with(".upstroke") || root.ends_with("upstroke"),
        "{root:?}"
    );
    assert!(root.is_absolute(), "{root:?}");
}

#[test]
fn runs_list_chronologically_and_resolve_by_prefix() {
    let root = scratch("discover");
    let repo = root.join("repo");
    for id in ["01AAA", "01BBB", "01BCC"] {
        commit_run(&repo, id);
    }
    assert_eq!(list_runs(&repo), ["01AAA", "01BBB", "01BCC"]);
    assert_eq!(latest_run(&repo).as_deref(), Some("01BCC"));

    assert_eq!(resolve_run_id(&repo, "01AAA").expect("exact"), "01AAA");
    assert_eq!(resolve_run_id(&repo, "01A").expect("prefix"), "01AAA");
    assert_eq!(
        resolve_run_id(&repo, "01bcc").expect("case-insensitive"),
        "01BCC"
    );

    let err = resolve_run_id(&repo, "01B").expect_err("ambiguous");
    assert!(err.to_string().contains("matches 2 runs"), "got: {err}");
    let err = resolve_run_id(&repo, "02").expect_err("no match");
    assert!(err.to_string().contains("known runs"), "got: {err}");
}

#[test]
fn an_empty_repo_names_where_it_looked() {
    let root = scratch("norun");
    let err = resolve_run_id(&root.join("repo"), "01A").expect_err("nothing to resume");
    assert!(err.to_string().contains("no runs found"), "got: {err}");
}

#[test]
fn questions_resolve_to_their_run_by_prefix() {
    let root = scratch("questions");
    let repo = root.join("repo");
    for (run, question) in [
        ("01AAA", "q-ONE"),
        ("01BBB", "q-TWO"),
        ("01BBB", "q-TWENTY"),
    ] {
        let dir = commit_run(&repo, run).join("questions");
        fs::create_dir_all(&dir).expect("questions dir");
        fs::write(dir.join(format!("{question}.json")), "{}").expect("question");
    }

    let found = find_question(&repo, "q-ONE").expect("exact");
    assert_eq!(found.run_id, "01AAA");
    assert_eq!(found.question_id, "q-ONE");
    assert_eq!(found.public, public_dir(&repo, "01AAA"));

    // A full id wins even though `q-TWO` is also a prefix of `q-TWENTY`.
    let found = find_question(&repo, "q-TWO").expect("exact beats prefix");
    assert_eq!(found.question_id, "q-TWO");
    assert_eq!(found.run_id, "01BBB");

    let err = find_question(&repo, "q-TW").expect_err("ambiguous");
    assert!(
        err.to_string().contains("matches 2 questions"),
        "got: {err}"
    );
    let err = find_question(&repo, "q-NONE").expect_err("no match");
    assert!(err.to_string().contains("no question"), "got: {err}");
}

#[test]
fn a_run_can_only_be_held_once_at_a_time() {
    let root = scratch("lock");
    let paths = paths_in(&root, "RUN1");
    paths.create().expect("create");

    assert!(
        !is_running(&paths.public),
        "nothing holds a run that never started"
    );
    let held = RunLock::acquire(&paths.public).expect("first acquire");
    assert!(is_running(&paths.public), "status can see the run is live");

    // This one is `claims`, not the OS: `fcntl` locks belong to the process,
    // so both of these would succeed if the file were the only guard.
    // Cross-process exclusion — the property that actually matters — is
    // `a_second_process_is_refused_the_run_lock` below.
    let err = RunLock::acquire(&paths.public).expect_err("a second engine is refused");
    assert!(
        err.to_string().contains("already driving run"),
        "got: {err}"
    );

    // A refusal that failed still leaves the run exactly as claimed as it
    // was — a bookkeeping slip here would either free a live run or strand
    // a dead one.
    assert!(
        is_running(&paths.public),
        "the failed acquire changed nothing"
    );

    // Dropping releases it — which is also what a crash does, so resume
    // never has to clear a stale marker by hand.
    drop(held);
    assert!(!is_running(&paths.public));
    RunLock::acquire(&paths.public).expect("re-acquire after release");
}

#[cfg(unix)]
#[test]
fn same_process_handoff_closes_old_descriptor_before_publishing_claim_free() {
    let root = scratch("orderedhandoff");
    let paths = paths_in(&root, "RUN1");
    paths.create().expect("create");
    let mut held = RunLock::acquire(&paths.public).expect("first acquire");

    held.release_file_then(|| {
        let file = File::open(lock_file(&paths.public)).expect("inspect released lock");
        assert!(
            matches!(imp::holder(&file), Holder::Nobody),
            "the old descriptor must already be closed"
        );
        let error = RunLock::acquire(&paths.public)
            .expect_err("the in-process claim stays published until after close");
        assert!(error.to_string().contains("already driving run"), "{error}");
    });

    let replacement = RunLock::acquire(&paths.public).expect("handoff after ordered release");
    drop(replacement);
}

#[cfg(unix)]
#[test]
fn cleanup_lease_failure_closes_primary_before_releasing_claim() {
    let root = scratch("cleanupfailurehandoff");
    let paths = paths_in(&root, "RUN1");
    paths.create().expect("create");
    let mut held = RunLock::acquire(&paths.public).expect("primary acquired");
    let file = held._file.take();
    let claim = held.claim.clone();

    // This is the exact rollback primitive used when cleanup::take fails.
    // The callback is a deterministic observation point between closing
    // the POSIX descriptor and publishing the same-process claim as free.
    release_claim_after_file(file, &claim, || {
        let file = File::open(lock_file(&paths.public)).expect("inspect primary lock");
        assert!(matches!(imp::holder(&file), Holder::Nobody));
        RunLock::acquire(&paths.public)
            .expect_err("claim cannot be reused until the old descriptor is closed");
    });

    let replacement = RunLock::acquire(&paths.public).expect("clean rollback handoff");
    drop(replacement);
    drop(held);
}

#[test]
fn a_run_lock_remains_send_even_though_its_cleanup_scope_is_thread_local() {
    fn assert_send<T: Send>() {}
    assert_send::<RunLock>();
}

#[test]
fn the_lock_answers_at_once_rather_than_waiting_to_be_sure() {
    // There was a 500ms contention grace here, and it was paid in full
    // exactly when the answer was yes: a live engine never lets go, so the
    // retry loop always ran to the deadline. Every `upstroke status` and
    // `upstroke answer` against a working run paid it, and `--follow` paid it
    // once per idle poll until it was given a cheaper question to ask.
    //
    // The grace existed to disbelieve a `fork` window. The primitive now
    // rules that out outright, so there is nothing left to wait for.
    let root = scratch("prompt");
    let paths = paths_in(&root, "RUN1");
    paths.create().expect("create");
    let _held = RunLock::acquire(&paths.public).expect("acquire");

    let started = Instant::now();
    for _ in 0..20 {
        assert!(is_running(&paths.public));
    }
    let waited = started.elapsed();
    assert!(
        waited < Duration::from_millis(100),
        "twenty probes of a live run took {waited:?} — something is waiting again"
    );
}

/// A `fork` that has not reached its `exec` yet, held open on purpose.
///
/// The child does nothing but sleep and `_exit`, both of which are safe in
/// the child of a threaded process — no allocation, no locks, no
/// destructors.
#[cfg(unix)]
fn fork_a_sleeper(ms: u64) -> libc::pid_t {
    let pid = unsafe { libc::fork() };
    if pid == 0 {
        std::thread::sleep(Duration::from_millis(ms));
        unsafe { libc::_exit(0) };
    }
    assert!(pid > 0, "fork failed");
    pid
}

#[cfg(unix)]
#[test]
fn a_fork_cannot_keep_a_released_run_locked() {
    // The bug the whole design turns on, deterministically.
    //
    // `flock` belongs to the open file description, and `fork` duplicates
    // every descriptor — so a child holds the run's lock until it execs,
    // and an engine that has finished and let go still reads as live for
    // that whole window. It was measured at 50 false positives in 3000
    // probes under a suite that spawns subprocesses, and each one made a
    // run refuse to start against an engine that did not exist, or a
    // finished run report itself as running.
    //
    // Against `flock` this test fails outright: the probe below sees the
    // lock held by the sleeping child. `fcntl` locks are not inherited, so
    // releasing really releases.
    let root = scratch("forkwindow");
    let paths = paths_in(&root, "RUN1");
    paths.create().expect("create");

    let held = RunLock::acquire(&paths.public).expect("acquire");
    let sleeper = fork_a_sleeper(400);
    // The engine finishes while that child is still between fork and exec.
    drop(held);

    assert!(
        !is_running(&paths.public),
        "a forked child was still holding the run's lock"
    );
    RunLock::acquire(&paths.public).expect("and a second engine can start");

    let mut status = 0;
    unsafe { libc::waitpid(sleeper, &mut status, 0) };
}

/// The child half of `a_second_process_is_refused_the_run_lock`: takes the
/// lock, says so, and holds it until it is killed.
///
/// An `#[ignore]`d test re-invoked as a subprocess, which is how
/// `killing_a_run_mid_attempt_leaves_a_resumable_record` gets a real second
/// process too.
#[test]
#[ignore = "spawned as a subprocess by a_second_process_is_refused_the_run_lock"]
fn lock_child_holds_the_run() {
    let public = PathBuf::from(std::env::var("UPSTROKE_TEST_LOCK_DIR").expect("run dir"));
    let _held = RunLock::acquire(&public).expect("the child takes the lock");
    println!("held");
    std::io::Write::flush(&mut std::io::stdout()).expect("flush");
    std::thread::sleep(Duration::from_secs(30));
}

#[test]
#[ignore = "spawned as a subprocess by two_run_ids_cannot_drive_one_worktree_concurrently"]
fn worktree_lock_child_holds_run_a() {
    let repo = PathBuf::from(std::env::var("UPSTROKE_TEST_WORKTREE_DIR").expect("repo"));
    let git_dir = PathBuf::from(std::env::var("UPSTROKE_TEST_WORKTREE_GIT_DIR").expect("git dir"));
    let public = PathBuf::from(std::env::var("UPSTROKE_TEST_LOCK_DIR").expect("run dir"));
    let _worktree = WorktreeLock::acquire_in(&repo, &git_dir).expect("child takes worktree lease");
    let _run = RunLock::acquire(&public).expect("child takes run A lock");
    println!("held");
    std::io::Write::flush(&mut std::io::stdout()).expect("flush");
    std::thread::sleep(Duration::from_secs(30));
}

#[test]
fn two_run_ids_cannot_drive_one_worktree_concurrently() {
    let root = scratch("two-runs-one-worktree");
    let repo = root.join("repo");
    let git_dir = root.join("git-dir");
    fs::create_dir_all(&git_dir).expect("worktree git dir");
    let run_a = paths_in(&repo, "RUNA");
    let run_b = paths_in(&repo, "RUNB");
    run_a.create().expect("run A dirs");
    run_b.create().expect("run B dirs");

    let exe = std::env::current_exe().expect("test binary");
    // Adopted, so the child is terminated, reaped and its reader joined
    // when this scope ends however it ends -- including a panicking
    // assertion between here and the teardown below.
    let mut producer = readiness::Producer::adopt(
        std::process::Command::new(exe)
            .args([
                "--exact",
                "rundir::tests::worktree_lock_child_holds_run_a",
                "--ignored",
                "--nocapture",
            ])
            .env("UPSTROKE_TEST_WORKTREE_DIR", &repo)
            .env("UPSTROKE_TEST_WORKTREE_GIT_DIR", &git_dir)
            .env("UPSTROKE_TEST_LOCK_DIR", &run_a.public)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("spawn run A engine"),
    );

    // Producer-aware and *effectively* bounded, at the bound this test
    // already used. The loop it replaces checked its deadline only after
    // `read_line` returned, so against a producer that stayed alive and
    // silent -- the one case CODING_STANDARDS.md §12 says the bound exists
    // for -- the read blocked and the deadline was never reached at all.
    producer
        .await_line("held", Duration::from_secs(30))
        .or_fail("run A child never took its leases");

    // The per-run lock alone would allow this: the identifiers and files
    // differ. The outer lease is what owns shared HEAD/index/worktree state.
    let run_b_only = RunLock::acquire(&run_b.public).expect("run B lock is independent");
    drop(run_b_only);
    let error =
        WorktreeLock::acquire_in(&repo, &git_dir).expect_err("run B must lose the worktree lease");
    assert!(
        error.to_string().contains("already driving worktree"),
        "{error}"
    );

    drop(producer);
}

/// The probe of the worktree-lease fault witnesses: from a process of its
/// own, answers `absent` when the persistent lock file is not there (and
/// creates nothing), `free` when it takes the lease (and gives it back as it
/// exits), and `refused` when another process holds it.
#[test]
#[ignore = "spawned as a subprocess by the worktree-lease fault witnesses"]
fn worktree_lease_probe_child() {
    let repo = PathBuf::from(std::env::var("UPSTROKE_TEST_WORKTREE_DIR").expect("repo"));
    let git_dir = PathBuf::from(std::env::var("UPSTROKE_TEST_WORKTREE_GIT_DIR").expect("git dir"));
    let answer = if !worktree_lock_file(&git_dir).exists() {
        "absent"
    } else {
        match WorktreeLock::acquire_in(&repo, &git_dir) {
            Ok(_lease) => "free",
            Err(error) if error.to_string().contains("already driving worktree") => "refused",
            Err(error) => panic!("the probe's acquisition failed for another reason: {error}"),
        }
    };
    println!("{answer}");
    std::io::Write::flush(&mut std::io::stdout()).expect("flush");
}

/// Ask [`worktree_lease_probe_child`] about the lease and require `expected`.
fn probe_worktree_lease(repo: &Path, git_dir: &Path, expected: &str, context: &str) {
    let mut producer = readiness::Producer::adopt(
        std::process::Command::new(std::env::current_exe().expect("test binary"))
            .args([
                "--exact",
                "rundir::tests::worktree_lease_probe_child",
                "--ignored",
                "--nocapture",
            ])
            .env("UPSTROKE_TEST_WORKTREE_DIR", repo)
            .env("UPSTROKE_TEST_WORKTREE_GIT_DIR", git_dir)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("spawn the lease probe"),
    );
    producer
        .await_line(expected, Duration::from_secs(30))
        .or_fail(&format!("{context}: the probe did not answer `{expected}`"));
}

/// The production adapter with an error return armed at one `(site, phase)`:
/// the harness records the phase first, as the funnel's own hook call does,
/// and the armed coordinate answers [`Injection::Error`].
struct FailingAt {
    inner: HarnessHooks,
    at: (EffectSiteId, HookPhase),
}

impl RunDirHooks for FailingAt {
    fn hook(&mut self, site: EffectSiteId, phase: HookPhase) -> Injection {
        let answered = self.inner.hook(site, phase);
        if (site, phase) == self.at {
            Injection::Error
        } else {
            answered
        }
    }

    fn durability_ledger(&self) -> DurabilityLedger {
        self.inner.durability_ledger()
    }
}

/// A fault at one phase of the worktree lease's two sites, on a repository
/// whose persistent lock file (R25, "never removed by a run") an earlier
/// write command left and something has since removed — the planted
/// absence — and then the tabled action, which for a lease is the next write
/// command's acquisition.
///
/// The residue is read against the authority, not restated: the create's
/// rows name R25 exactly when the lock file is left, and the acquisition's
/// before phase names no row, since no hold is taken before it. No hold
/// survives an acquisition that returned an error, which a second process
/// taking the lease at once proves (an in-process probe cannot: `fcntl` locks
/// do not conflict with their own process). The action is the authority's
/// too: before a phase nothing was performed, so the next acquisition
/// performs it; after `CreateWorktreeLockFile` the file is adopted, so a byte
/// written into it survives the next acquisition.
fn a_fault_at_the_worktree_lease_converges_on_the_next_acquisition(
    site: LockSite,
    phase: HookPhase,
    tag: &str,
) {
    use crate::topology::effects::{EntryPhase, ResourceRow, ResumeAction};

    // Held until this witness returns or unwinds, which reclaims the tree:
    // `scratch_tree`'s guard, whose failed reclaim fails the test naming the
    // root, or on an unwind is reported without a second panic (#292's review
    // round 6: the root was the pid-named `scratch`, which nothing removed).
    let root = scratch_tree::acquire(&std::env::temp_dir(), tag).expect("a scratch tree");
    let repo = root.path().join("repo");
    let git_dir = root.path().join("git-dir");
    fs::create_dir_all(&repo).expect("repository");
    fs::create_dir_all(&git_dir).expect("worktree git dir");
    let lock_file = worktree_lock_file(&git_dir);
    fs::write(&lock_file, b"").expect("the lock file an earlier write command left");
    fs::remove_file(&lock_file).expect("the planted absence");

    let site = EffectSiteId::Lock(site);
    let entry = match phase {
        HookPhase::Before => EntryPhase::Before,
        HookPhase::After => EntryPhase::After,
        HookPhase::Point { .. } => panic!("a lease site's coordinates are its two hook phases"),
    };
    let semantics = site.semantics(entry);

    let harness = Arc::new(Mutex::new(HookHarness::new()));
    let mut faulted = FailingAt {
        inner: HarnessHooks::new(Arc::clone(&harness)),
        at: (site, phase),
    };
    let error = WorktreeLock::acquire_in_hooked(&repo, &git_dir, &mut faulted)
        .expect_err("the armed fault ends the acquisition");
    drop(faulted);
    assert!(
        error
            .to_string()
            .contains(&format!("was made to fail at `{site}` ({phase})")),
        "{tag}: the injected error is the one returned: {error}"
    );
    assert!(
        harness
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .observed(site, phase),
        "{tag}: the armed coordinate was reached through the production adapter"
    );

    let create = EffectSiteId::Lock(LockSite::CreateWorktreeLockFile);
    let create_performed = (site, phase) != (create, HookPhase::Before);
    assert_eq!(
        lock_file.exists(),
        create_performed,
        "{tag}: the lock file is left exactly when the create was performed"
    );
    if site == create {
        assert_eq!(
            semantics.rows,
            if create_performed {
                vec![ResourceRow::R25]
            } else {
                Vec::new()
            },
            "{tag}: and the authority's rows say the same"
        );
    } else {
        assert!(
            semantics.rows.is_empty(),
            "{tag}: before the hold is taken R17 holds nothing ({:?})",
            semantics.rows
        );
    }
    if create_performed {
        probe_worktree_lease(&repo, &git_dir, "free", &format!("{tag}: after the fault"));
        fs::write(&lock_file, b"adopted").expect("mark the file the fault left");
    } else {
        probe_worktree_lease(
            &repo,
            &git_dir,
            "absent",
            &format!("{tag}: after the fault"),
        );
    }

    let recovery = Arc::new(Mutex::new(HookHarness::new()));
    let mut hooks = HarnessHooks::new(Arc::clone(&recovery));
    let lease = WorktreeLock::acquire_in_hooked(&repo, &git_dir, &mut hooks)
        .unwrap_or_else(|error| panic!("{tag}: the next acquisition takes the lease: {error}"));
    drop(hooks);
    for (converged, phase) in [
        (LockSite::CreateWorktreeLockFile, HookPhase::Before),
        (LockSite::CreateWorktreeLockFile, HookPhase::After),
        (LockSite::AcquireWorktree, HookPhase::Before),
        (LockSite::AcquireWorktree, HookPhase::After),
    ] {
        assert!(
            recovery
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .observed(EffectSiteId::Lock(converged), phase),
            "{tag}: the next acquisition runs `Lock.{}` ({phase})",
            converged.name()
        );
    }
    assert!(lock_file.is_file(), "{tag}: R25's file stands");
    assert_eq!(
        semantics.action,
        match phase {
            HookPhase::Before => ResumeAction::ResumeUnperformed,
            _ => ResumeAction::AdoptPerformed,
        },
        "{tag}: the action the authority tables for `{site}` ({phase})"
    );
    probe_worktree_lease(&repo, &git_dir, "refused", &format!("{tag}: while held"));
    drop(lease);
    probe_worktree_lease(
        &repo,
        &git_dir,
        "free",
        &format!("{tag}: after the release"),
    );
    // Read only now: closing any descriptor of the file drops this process's
    // `fcntl` lock on it, so a read while the lease was held would release it.
    if create_performed {
        assert_eq!(
            fs::read(&lock_file).expect("the lock file reads"),
            b"adopted",
            "{tag}: the file the create performed was adopted, not replaced"
        );
    }
}

#[test]
fn a_fault_before_the_worktree_lock_file_is_created_leaves_nothing_and_the_next_acquisition_creates_it()
 {
    a_fault_at_the_worktree_lease_converges_on_the_next_acquisition(
        LockSite::CreateWorktreeLockFile,
        HookPhase::Before,
        "lease-fault-create-before",
    );
}

#[test]
fn a_fault_after_the_worktree_lock_file_is_created_leaves_it_unheld_and_the_next_acquisition_adopts_it()
 {
    a_fault_at_the_worktree_lease_converges_on_the_next_acquisition(
        LockSite::CreateWorktreeLockFile,
        HookPhase::After,
        "lease-fault-create-after",
    );
}

#[test]
fn a_fault_before_the_worktree_lease_is_taken_leaves_no_hold_and_the_next_acquisition_takes_it() {
    a_fault_at_the_worktree_lease_converges_on_the_next_acquisition(
        LockSite::AcquireWorktree,
        HookPhase::Before,
        "lease-fault-acquire-before",
    );
}

#[test]
fn a_second_process_is_refused_the_run_lock() {
    // The property `claims` cannot provide and the file lock exists for.
    // Two engines are two processes, and `fcntl` locks are per-process —
    // which is exactly why this has to be tested across a real process
    // boundary rather than against a second `acquire` here.
    let root = scratch("twoprocs");
    let paths = paths_in(&root, "RUN1");
    paths.create().expect("create");

    let exe = std::env::current_exe().expect("test binary");
    // Adopted, so the child is terminated, reaped and its reader joined
    // when this scope ends however it ends -- including a panicking
    // assertion between here and the teardown below.
    let mut producer = readiness::Producer::adopt(
        std::process::Command::new(exe)
            .args([
                "--exact",
                "rundir::tests::lock_child_holds_the_run",
                "--ignored",
                "--nocapture",
            ])
            .env("UPSTROKE_TEST_LOCK_DIR", &paths.public)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("spawn the second engine"),
    );

    // Wait for it to say it has the lock, rather than sleeping and hoping.
    // Producer-aware and effectively bounded, at the bound this test
    // already used; see `two_run_ids_cannot_drive_one_worktree_concurrently`
    // for what the loop this replaces could not do.
    producer
        .await_line("held", Duration::from_secs(30))
        .or_fail("the child never took the lock");

    let err = RunLock::acquire(&paths.public).expect_err("a second engine must be refused");
    assert!(
        err.to_string().contains("already driving run"),
        "got: {err}"
    );
    assert!(is_running(&paths.public), "and status agrees it is live");

    // `F_GETLK` names the holder, so the refusal can say who instead of
    // leaving the operator to find it. Asserted here rather than against a
    // second `acquire` in this process, because that one is refused by
    // `claims`, which knows this pid without asking the OS anything — it
    // would pass whatever the lock did.
    #[cfg(unix)]
    assert!(
        err.to_string()
            .contains(&format!("pid {}", producer.child().id())),
        "the refusal should name the process actually holding it: {err}"
    );

    drop(producer);
}

#[cfg(unix)]
#[test]
fn a_holder_never_opens_its_own_lock_file() {
    // `fcntl`'s sharpest edge: closing *any* descriptor for a file releases
    // every lock this process holds on it. So a holder that does what
    // `is_running` does — open the lock file, look, drop it — hands the run
    // away silently, and the next `acquire` anywhere succeeds against a
    // live engine.
    //
    // `is_running` answers from `claims` before it would open anything,
    // which is what makes that unreachable. This test is here because the
    // rule is invisible in the code that depends on it.
    let root = scratch("selfclose");
    let paths = paths_in(&root, "RUN1");
    paths.create().expect("create");
    let _held = RunLock::acquire(&paths.public).expect("acquire");

    // The call a holder is most likely to make.
    assert!(is_running(&paths.public));

    // If that had gone to the file, the lock would be gone by now — ask
    // from a process that has no claim of its own to answer from.
    let pid = unsafe { libc::fork() };
    if pid == 0 {
        let file = File::open(lock_file(&paths.public)).expect("open");
        let free = matches!(imp::holder(&file), Holder::Nobody);
        unsafe { libc::_exit(i32::from(free)) };
    }
    let mut status = 0;
    unsafe { libc::waitpid(pid, &mut status, 0) };
    assert_eq!(
        libc::WEXITSTATUS(status),
        0,
        "the holder released its own lock by looking at it"
    );
}

#[test]
fn a_lock_the_os_will_not_report_on_is_not_a_free_lock() {
    // No filesystem CI runs on returns `ENOLCK`, so the decision is checked
    // where it is made. A lock the OS declines to report on must not come
    // back as "nobody is running", because that is the reading that tells
    // an operator to resume a run that is still in flight.
    let unknown = Holder::Unknown(io::Error::from_raw_os_error(ENOLCK_LIKE));
    assert!(
        !matches!(unknown, Holder::Nobody),
        "an error is not an answer"
    );
}

/// Any errno at all; the value is not what is under test.
const ENOLCK_LIKE: i32 = 37;

#[test]
fn an_exact_match_resolves_to_the_name_on_disk() {
    // The comparison is case-insensitive, so the answer has to be the
    // directory that actually exists: on a case-sensitive filesystem the
    // uppercased input names nothing, and every caller joins this id onto
    // a path.
    let root = scratch("ondisk");
    let repo = root.join("repo");
    commit_run(&repo, "01AbCd");

    assert_eq!(resolve_run_id(&repo, "01abcd").expect("exact"), "01AbCd");
    assert_eq!(resolve_run_id(&repo, "01AB").expect("prefix"), "01AbCd");
}

// =======================================================================
// Classification
// =======================================================================

/// One directory shape, its construction, and the class the packet gives
/// it. The expected value is transcribed from the packet's own rule and
/// never computed by the function under test.
struct DirShape {
    name: &'static str,
    build: fn(&Path),
    expected: RunDirClass,
}

fn write(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent");
    }
    fs::write(path, bytes).expect("write");
}

/// A marker that **names the private half sitting beside it**.
///
/// `any_marker_bytes` records `/nowhere/runs/01SHAPE`, which is what let
/// `PR5-RUNDIR-005` and `PR5-RUNDIR-006` survive the thirteen-shape grid: a
/// classifier that *follows the locator* looked in `/nowhere`, found
/// nothing, fell through to the first-line probe and answered `Husk` for
/// the wrong reason. The grid proved the classifier ignores a private half
/// sitting beside it; it never proved the classifier ignores the private
/// half the marker actually names — and "`Committed` by a valid
/// newline-terminated first-line `run_started`, else `Husk`" is a claim
/// about every private half, named or not.
fn marker_bytes_locating(private: &Path) -> Vec<u8> {
    serde_json::to_vec(&CreatingMarker {
        run_id: "01SHAPE".to_owned(),
        repo_key: "0123456789abcdef".to_owned(),
        private_dir: private.to_string_lossy().into_owned(),
        incarnation: "01INC".to_owned(),
        pid: 4242,
        runner_policy_sha256: "sha256:00".to_owned(),
    })
    .expect("marker json")
}

/// An owner record with every field populated, so a classifier that parses
/// what it finds is caught as surely as one that only stats it.
fn plausible_owner_bytes() -> Vec<u8> {
    serde_json::to_vec(&OwnerRecord {
        run_id: "01SHAPE".to_owned(),
        repo_key: "0123456789abcdef".to_owned(),
        public_dir: "/nowhere/public".to_owned(),
        incarnation: "01INC".to_owned(),
        runner: crate::runner::policy::host_policy(),
    })
    .expect("owner json")
}

/// A marker whose fields do not matter to the classifier, which is the
/// point: `startup_census` classifies "whether or not a marker is present".
fn any_marker_bytes() -> Vec<u8> {
    serde_json::to_vec(&CreatingMarker {
        run_id: "01SHAPE".to_owned(),
        repo_key: "0123456789abcdef".to_owned(),
        private_dir: "/nowhere/runs/01SHAPE".to_owned(),
        incarnation: "01INC".to_owned(),
        pid: 4242,
        runner_policy_sha256: "sha256:00".to_owned(),
    })
    .expect("marker json")
}

/// The publication prefixes P0–P8, as `classify_run_dir`'s proof test
/// names them.
///
/// The contract's list — "bare, staged-marker, marker-only, marker+lock,
/// marker+private (with and without owner record; with and without commit
/// record), log-without-committed-first-line, torn-first-line,
/// committed-with-marker, malformed-marker, and committed" — reads as a
/// crossing on the `marker+private` entry, so the maximal reading is
/// thirteen shapes and the collapsed one is ten. This table carries the
/// maximal reading plus the shapes `startup_census` names that the
/// contract's phrase does not spell out separately, because covering
/// thirteen covers twelve whichever way the sentence is read.
fn shapes() -> Vec<DirShape> {
    vec![
        DirShape {
            name: "bare",
            build: |public| fs::create_dir_all(public).expect("bare"),
            expected: RunDirClass::Husk,
        },
        DirShape {
            name: "staged-marker",
            build: |public| write(&public.join(MARKER_STAGED), &any_marker_bytes()),
            expected: RunDirClass::Husk,
        },
        DirShape {
            name: "marker-only",
            build: |public| write(&public.join(MARKER), &any_marker_bytes()),
            expected: RunDirClass::Husk,
        },
        DirShape {
            name: "marker+lock",
            build: |public| {
                write(&public.join(MARKER), &any_marker_bytes());
                write(&lock_file(public), b"");
            },
            expected: RunDirClass::Husk,
        },
        DirShape {
            name: "marker+private-with-owner-record",
            build: |public| {
                write(&public.join(MARKER), &any_marker_bytes());
                write(&public.join("private/owner.json"), b"{}");
            },
            expected: RunDirClass::Husk,
        },
        DirShape {
            name: "marker+private-without-owner-record",
            build: |public| {
                write(&public.join(MARKER), &any_marker_bytes());
                fs::create_dir_all(public.join("private")).expect("private");
            },
            expected: RunDirClass::Husk,
        },
        DirShape {
            name: "marker+private-with-commit-record",
            build: |public| {
                write(&public.join(MARKER), &any_marker_bytes());
                write(&public.join("private/owner.json"), b"{}");
                write(&public.join("private/committed.json"), b"{}");
            },
            expected: RunDirClass::Husk,
        },
        DirShape {
            name: "marker+private-without-commit-record",
            build: |public| {
                write(&public.join(MARKER), &any_marker_bytes());
                write(&public.join("private/owner.json"), b"{}");
                write(&public.join(PLAN), b"{}");
            },
            expected: RunDirClass::Husk,
        },
        // The two shapes the grid was missing: the marker names the
        // private half that is really there. A classifier that follows the
        // locator answers `Committed` for both, and only these two shapes
        // can tell it from one that does not.
        DirShape {
            name: "marker-bound-private-with-owner-record",
            build: |public| {
                let private = public.join("private");
                write(&private.join(OWNER_RECORD), &plausible_owner_bytes());
                write(&public.join(MARKER), &marker_bytes_locating(&private));
            },
            expected: RunDirClass::Husk,
        },
        DirShape {
            name: "marker-bound-private-with-commit-record",
            build: |public| {
                let private = public.join("private");
                write(&private.join(OWNER_RECORD), &plausible_owner_bytes());
                write(
                    &private.join(COMMIT_RECORD),
                    b"{\"run_started_sha256\":\"sha256:00\"}",
                );
                write(&public.join(MARKER), &marker_bytes_locating(&private));
            },
            expected: RunDirClass::Husk,
        },
        DirShape {
            name: "log-without-committed-first-line",
            build: |public| {
                write(
                    &public.join(EVENT_LOG),
                    b"{\"ts\":\"t\",\"event\":\"attempt_started\",\"data\":{}}\n",
                );
            },
            expected: RunDirClass::Husk,
        },
        DirShape {
            name: "torn-first-line",
            build: |public| {
                // The newline is the commit marker, so a first line
                // without one is not an event and never was.
                let torn = committed_line("01TORN", 3);
                write(&public.join(EVENT_LOG), &torn.as_bytes()[..torn.len() - 8]);
            },
            expected: RunDirClass::Husk,
        },
        DirShape {
            // The shape above truncates the JSON as well as the newline, so
            // it refuses on the parse and stays green if the *terminator*
            // requirement is dropped — measured: a `first_committed_line`
            // that treats end-of-file as end-of-line survived the whole
            // grid. This shape isolates the terminator: a complete, valid,
            // parseable `run_started` whose only defect is that it was
            // never terminated. `startup_census` says "first
            // **newline-terminated** line", and the newline is the only
            // evidence that the writer finished writing it.
            name: "complete-first-line-with-no-newline",
            build: |public| {
                write(
                    &public.join(EVENT_LOG),
                    committed_line("01SHAPE", 3).as_bytes(),
                );
            },
            expected: RunDirClass::Husk,
        },
        DirShape {
            name: "malformed-marker",
            build: |public| {
                write(&public.join(MARKER), b"{ not json");
                write(&public.join(PLAN), b"{}");
            },
            expected: RunDirClass::Husk,
        },
        DirShape {
            name: "committed",
            build: |public| {
                write(
                    &public.join(EVENT_LOG),
                    format!("{}\n", committed_line("01SHAPE", 3)).as_bytes(),
                );
            },
            expected: RunDirClass::Committed,
        },
        DirShape {
            name: "committed-with-marker",
            build: |public| {
                write(
                    &public.join(EVENT_LOG),
                    format!("{}\n", committed_line("01SHAPE", 3)).as_bytes(),
                );
                write(&public.join(MARKER), &any_marker_bytes());
            },
            expected: RunDirClass::Committed,
        },
        // Beyond the contract's list, from `startup_census`'s own
        // enumeration and from the rule's own edges.
        DirShape {
            name: "committed-with-staged-marker",
            build: |public| {
                write(
                    &public.join(EVENT_LOG),
                    format!("{}\n", committed_line("01SHAPE", 3)).as_bytes(),
                );
                write(&public.join(MARKER_STAGED), &any_marker_bytes());
            },
            expected: RunDirClass::Committed,
        },
        DirShape {
            name: "empty-log",
            build: |public| write(&public.join(EVENT_LOG), b""),
            expected: RunDirClass::Husk,
        },
        DirShape {
            name: "blank-first-line-then-run-started",
            build: |public| {
                write(
                    &public.join(EVENT_LOG),
                    format!("\n{}\n", committed_line("01SHAPE", 3)).as_bytes(),
                );
            },
            expected: RunDirClass::Husk,
        },
        DirShape {
            name: "first-line-is-not-json",
            build: |public| write(&public.join(EVENT_LOG), b"not json at all\n"),
            expected: RunDirClass::Husk,
        },
        DirShape {
            name: "first-line-has-no-schema-to-select-by",
            build: |public| {
                write(
                    &public.join(EVENT_LOG),
                    b"{\"event\":\"run_started\",\"data\":{\"run_id\":\"01SHAPE\"}}\n",
                );
            },
            expected: RunDirClass::Husk,
        },
        DirShape {
            name: "committed-first-line-with-a-torn-tail",
            build: |public| {
                // A torn *tail* is truncated by the next open and was
                // never an event; it says nothing about the first line.
                write(
                    &public.join(EVENT_LOG),
                    format!(
                        "{}\n{{\"ts\":\"t\",\"event\":\"attempt_star",
                        committed_line("01SHAPE", 3)
                    )
                    .as_bytes(),
                );
            },
            expected: RunDirClass::Committed,
        },
        DirShape {
            name: "committed-schema-4",
            build: |public| {
                write(
                    &public.join(EVENT_LOG),
                    format!("{}\n", committed_line("01SHAPE", 4)).as_bytes(),
                );
            },
            expected: RunDirClass::Committed,
        },
    ]
}

#[test]
fn every_publication_prefix_classifies_as_the_packet_names_it() {
    let root = scratch("shapes");
    let mut committed = 0usize;
    let mut husks = 0usize;
    let mut indeterminate = 0usize;
    for shape in shapes() {
        let public = root.join(shape.name);
        fs::create_dir_all(&public).expect("shape dir");
        (shape.build)(&public);
        let actual = classify_run_dir(&public);
        assert_eq!(
            actual, shape.expected,
            "shape `{}` classified {actual:?}",
            shape.name
        );
        match actual {
            RunDirClass::Committed => committed += 1,
            RunDirClass::Husk => husks += 1,
            RunDirClass::Indeterminate => indeterminate += 1,
        }
    }
    // Distinct-value counts rather than prose: a grid that had drifted to
    // one class would still pass every assertion above.
    assert_eq!(committed, 5, "committed shapes");
    assert_eq!(husks, 18, "husk shapes");
    // Every shape here is an ordinary regular file read with no signal
    // arranged, so every read delivers bytes or ends. This is the count that
    // says `RunDirClass::Indeterminate` did not leak into ordinary
    // classification when it was added (`SWEEP-CLASSIFY-001`), and it counts
    // what the probe *answered* rather than what the shape declared -- the
    // arms above moved from `shape.expected` to `actual` for exactly that
    // reason, since counting the table would report the table.
    assert_eq!(
        indeterminate, 0,
        "no ordinary publication prefix is unclassifiable"
    );
    // The two marker-bound shapes are the ones a locator-following
    // classifier gets wrong, so their presence is asserted rather than
    // left to the count above.
    let names: Vec<&str> = shapes().iter().map(|shape| shape.name).collect();
    for bound in [
        "marker-bound-private-with-owner-record",
        "marker-bound-private-with-commit-record",
    ] {
        assert!(names.contains(&bound), "the grid lost `{bound}`");
    }
    assert!(
        committed + husks >= 13,
        "the contract's list reads as thirteen shapes at its widest"
    );
}

/// A symlinked `events.jsonl` is a `Husk` **whatever it points at**, and the
/// guard that makes it one is `symlink_metadata` rather than `metadata`.
///
/// That difference is the whole of what narrows the check-to-open race to a swap
/// of a directory entry the census owns, and nothing measured it: the other test
/// that plants a link points it at `/dev/zero`, which `metadata` refuses too
/// because a character device is not a regular file, so both spellings pass it.
/// This link points at a **valid committed log**, which is the one target the
/// two spellings disagree about -- following it answers `Committed` and refusing
/// it answers `Husk`.
///
/// Unix only: a symbolic *file* link on Windows needs a privilege the guest does
/// not grant an ordinary test process, and the suite's other link is gated the
/// same way.
#[cfg(unix)]
#[test]
fn a_symlinked_event_log_is_a_husk_however_valid_its_target() {
    let root = scratch("symlinked-log");
    let bytes = format!("{}\n", committed_line("01LINK", 3));

    // The premise, stated rather than assumed: these exact bytes under this
    // exact name are a committed run when the name is the file.
    let direct = root.join("direct");
    write(&direct.join(EVENT_LOG), bytes.as_bytes());
    assert_eq!(
        classify_run_dir(&direct),
        RunDirClass::Committed,
        "the target's own bytes are a committed run, or this measures nothing"
    );

    let target = root.join("a-real-log.jsonl");
    write(&target, bytes.as_bytes());
    let public = root.join("linked");
    fs::create_dir_all(&public).expect("public");
    std::os::unix::fs::symlink(&target, public.join(EVENT_LOG)).expect("symlink");
    assert!(
        fs::symlink_metadata(public.join(EVENT_LOG))
            .expect("stat the link")
            .file_type()
            .is_symlink(),
        "the planted entry must really be a symlink"
    );
    assert_eq!(
        classify_run_dir(&public),
        RunDirClass::Husk,
        "a symlinked events.jsonl is a husk whatever it points at"
    );
}

#[test]
fn a_missing_directory_and_a_missing_log_are_both_husks() {
    let root = scratch("absent");
    assert_eq!(classify_run_dir(&root.join("nothing")), RunDirClass::Husk);
    let bare = root.join("bare");
    fs::create_dir_all(&bare).expect("bare");
    assert_eq!(classify_run_dir(&bare), RunDirClass::Husk);
}

/// A valid `run_started` line, terminated, whose total length is exactly
/// `total` bytes.
///
/// The padding is a field *inside* the object, so the line stays a valid
/// `run_started` at every length — a fixture that padded outside the JSON
/// would refuse on the parse and could never distinguish a length bound
/// from a parse failure. That confound is the `bounded_grid` shape recorded
/// four times in `reviews/FINDINGS.md`, and `PR5B-CLASSIFIER-TERMINATOR-
/// UNTESTED` is the same file's most recent instance.
fn committed_line_of_exactly(run_id: &str, total: usize) -> Vec<u8> {
    let line = committed_line(run_id, 3);
    let head = &line[..line.len() - 1];
    let overhead = head.len() + ",\"pad\":\"".len() + "\"}".len() + "\n".len();
    assert!(
        total >= overhead,
        "a {total}-byte line cannot hold a run_started at all"
    );
    let padded = format!("{head},\"pad\":\"{}\"}}\n", "x".repeat(total - overhead));
    assert_eq!(padded.len(), total, "the padding arithmetic is off");
    padded.into_bytes()
}

/// `FIRST_LINE_WINDOW` decides how many syscalls the probe makes, and
/// nothing about what a directory *is*.
///
/// `startup_census` defines `Committed` as "`events.jsonl` exists and its
/// first **newline-terminated** line is a valid `run_started`" and states no
/// size exception, so every length classifies the same way. Six lengths
/// straddling the window in both directions, including a line four times
/// the window — which is `PR5-CORRECTNESS-002`'s failure sequence at
/// `FIRST_LINE_WINDOW + 1` and three orders of magnitude past it.
///
/// The lengths are written relative to the constant on purpose: the claim
/// is *independence*, so shrinking the constant must leave this test
/// passing. What would fail is any re-introduction of a length bound —
/// which is the mutation that matters here, and it is witnessed in
/// `reviews/FINDINGS.md`.
#[test]
fn classification_does_not_depend_on_the_probe_window() {
    let root = scratch("window");
    let window = usize::try_from(FIRST_LINE_WINDOW).expect("the window fits a usize");
    let mut lengths = std::collections::BTreeSet::new();
    for (label, total) in [
        ("tiny", 512),
        ("just under a chunk", SCAN_CHUNK - 1),
        ("exactly a chunk", SCAN_CHUNK),
        ("just under the window", window - 1),
        ("exactly the window", window),
        ("one past the window", window + 1),
        ("four windows", window * 4),
    ] {
        lengths.insert(total);
        let public = root.join(label.replace(' ', "-"));
        write(
            &public.join(EVENT_LOG),
            &committed_line_of_exactly("01WINDOW", total),
        );
        assert_eq!(
            classify_run_dir(&public),
            RunDirClass::Committed,
            "a {total}-byte valid run_started line ({label}) is committed at every length"
        );
    }
    assert_eq!(lengths.len(), 7, "seven distinct lengths: {lengths:?}");
    assert!(
        lengths.iter().filter(|len| **len > window).count() >= 2,
        "at least two lengths past the window, or the claim is untested: {lengths:?}"
    );
}

/// The terminator is still the whole of the difference, at every length.
///
/// `PR5B-CLASSIFIER-TERMINATOR-UNTESTED` added the un-terminated shape at
/// one small length; the fall-back path this slice added is a *second*
/// implementation of "is there a newline", so it gets the same question.
/// The two files differ in exactly one byte's presence.
#[test]
fn a_complete_first_line_with_no_terminator_is_a_husk_at_every_length() {
    let root = scratch("unterminated");
    let window = usize::try_from(FIRST_LINE_WINDOW).expect("the window fits a usize");
    for (label, total) in [
        ("inside the window", 4096),
        ("past the window", window + 4096),
    ] {
        let terminated = committed_line_of_exactly("01TERM", total);
        let unterminated = &terminated[..terminated.len() - 1];
        assert_eq!(
            terminated.last(),
            Some(&b'\n'),
            "{label}: the fixture must be terminated"
        );
        assert!(
            !unterminated.contains(&b'\n'),
            "{label}: dropping the terminator must leave no newline at all"
        );

        let committed = root.join(format!("{}-terminated", label.replace(' ', "-")));
        write(&committed.join(EVENT_LOG), &terminated);
        assert_eq!(
            classify_run_dir(&committed),
            RunDirClass::Committed,
            "{label}: the terminated fixture"
        );

        let husk = root.join(format!("{}-torn", label.replace(' ', "-")));
        write(&husk.join(EVENT_LOG), unterminated);
        assert_eq!(
            classify_run_dir(&husk),
            RunDirClass::Husk,
            "{label}: the same bytes without the terminator"
        );
    }
}

/// The line the probe hands to the parser is the line, exactly.
///
/// An off-by-one in the fall-back's newline offset is the defect the new
/// code could carry: one byte short truncates the closing brace and one
/// byte long splices the newline into the JSON, and *both* refuse on the
/// parse — so `Husk` would look like a correct answer for the wrong reason.
/// This asserts the bytes rather than the verdict, on both paths.
#[test]
fn the_probe_returns_the_lines_exact_bytes_on_both_paths() {
    let root = scratch("exact");
    let window = usize::try_from(FIRST_LINE_WINDOW).expect("the window fits a usize");
    for (label, total) in [("window path", 4096), ("scan path", window + 7)] {
        let line = committed_line_of_exactly("01EXACT", total);
        let mut bytes = line.clone();
        // A second event after it, so "read to end of file" and "read to
        // the first newline" are different answers.
        bytes.extend_from_slice(b"{\"ts\":\"2026-08-20T00:00:01Z\",\"event\":\"noise\"}\n");
        let path = root.join(label.replace(' ', "-")).join(EVENT_LOG);
        write(&path, &bytes);

        let mut file = File::open(&path).expect("open");
        let Observed::Found(read) = first_line(&mut file) else {
            panic!("{label}: a newline-terminated first line");
        };
        assert_eq!(
            read,
            line[..line.len() - 1].to_vec(),
            "{label}: the probe returned {} bytes for a {}-byte line",
            read.len(),
            line.len() - 1
        );
    }
}

/// A `Read + Seek` that serves one buffer until its first absolute seek and a
/// different one afterwards -- the source that changed between the probe's two
/// reads, which no single-threaded test can build out of a real file.
struct Rewritten {
    before: Vec<u8>,
    after: Vec<u8>,
    at: usize,
    rewound: bool,
}

impl Read for Rewritten {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let at = self.at;
        let bytes: &[u8] = if self.rewound {
            &self.after
        } else {
            &self.before
        };
        let take = buf.len().min(bytes.len().saturating_sub(at));
        buf[..take].copy_from_slice(&bytes[at..at + take]);
        self.at = at + take;
        Ok(take)
    }
}

impl Seek for Rewritten {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let SeekFrom::Start(at) = to else {
            return Err(io::Error::other(
                "only the probe's absolute rewind is scripted",
            ));
        };
        self.rewound = true;
        self.at = usize::try_from(at).map_err(io::Error::other)?;
        Ok(at)
    }
}

/// Every property of the returned line is re-established on the **re-read**,
/// and none of them carried over from the scan.
///
/// `first_line_within` reads twice: a constant-memory scan that finds the
/// newline's offset, then a re-read of that many bytes from the start. What the
/// scan proved is about bytes that may be gone by the time the second read
/// happens, so a source that changed in between can hand the re-read something
/// that is not a first line at all. Before this the only guard was the *length*,
/// which catches a source that shrank and nothing else: a rewrite that kept the
/// length returned bytes with a newline inside them, or bytes with none at the
/// end, as "the first newline-terminated line".
///
/// One case per guard, so no guard is witnessed by another's, plus the
/// unchanged-source control -- without it three refusals would also be what a
/// probe that refused everything answers.
#[test]
fn a_source_rewritten_between_the_scan_and_the_reread_has_no_first_line() {
    let window = usize::try_from(FIRST_LINE_WINDOW).expect("the window fits a usize");
    // Past the window, so the seek happens at all.
    let length = window + 100;
    let mut before = vec![b'x'; length];
    before.push(b'\n');
    let bound = before.len() as u64;

    // The newline moved earlier: the length and the terminator both still agree,
    // so only "no newline inside the line" refuses this.
    let mut earlier = vec![b'y'; length];
    earlier[10] = b'\n';
    earlier.push(b'\n');
    // The newline moved later: the length still agrees, so only the terminator
    // refuses this.
    let mut later = vec![b'y'; length + 100];
    later.push(b'\n');
    // Shorter, and still newline-terminated, so only the length refuses this.
    let mut shorter = vec![b'y'; length - 50];
    shorter.push(b'\n');

    for (label, after) in [
        ("a newline moved earlier", earlier),
        ("a newline moved later", later),
        ("a source that shrank", shorter),
    ] {
        let mut source = Rewritten {
            before: before.clone(),
            after,
            at: 0,
            rewound: false,
        };
        assert_eq!(
            first_line_within(&mut source, bound),
            Observed::Absent,
            "{label}: a first line the re-read cannot vouch for is a husk — and an \
             *absence*, which is a completed observation, not the unfinished one"
        );
        assert!(
            source.rewound,
            "{label}: the scan-and-re-read path was never reached, so nothing here is measured"
        );
    }

    let mut steady = Rewritten {
        before: before.clone(),
        after: before.clone(),
        at: 0,
        rewound: false,
    };
    assert_eq!(
        first_line_within(&mut steady, bound),
        Observed::Found(before[..length].to_vec()),
        "a source that did not change still has its first line"
    );
}

/// A source that never ends: every read hands back non-newline bytes and
/// it is never at end of file. `/dev/zero`, on a host that has one and on a
/// host that does not.
///
/// It refuses rather than looping once it is asked for more than the budget
/// the probe was given, so an unbounded probe **fails this test in
/// milliseconds** instead of hanging the suite or eating the machine's
/// memory. That is deliberate: the defect this guards (`PR5-RD-001`) is
/// non-termination, and a guard against non-termination that itself does
/// not terminate is no guard.
#[derive(Default)]
struct Endless {
    handed: u64,
    ceiling: u64,
}

impl Read for Endless {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.handed + buf.len() as u64 > self.ceiling {
            return Err(io::Error::other(format!(
                "the probe read past its budget: {} bytes handed out, ceiling {}",
                self.handed, self.ceiling
            )));
        }
        buf.fill(b'x');
        self.handed += buf.len() as u64;
        Ok(buf.len())
    }
}

impl Seek for Endless {
    fn seek(&mut self, _to: SeekFrom) -> io::Result<u64> {
        Ok(0)
    }
}

/// The probe **terminates** on a source with no end, and spends exactly the
/// budget it was given — not one byte more (`PR5-RD-001`).
///
/// The byte count is the assertion, not the verdict. `Husk` is what a probe
/// that read the first byte and gave up answers too, so a test that checked
/// only the class would pass for a probe that had stopped being able to see
/// a committed run at all. And the previous test of this shape asserted
/// `Husk` over one finite regular file, which every implementation of this
/// function — including the one that never returned — satisfies.
#[test]
fn the_first_line_probe_spends_its_budget_and_stops() {
    let budget = FIRST_LINE_WINDOW * 4 + 1234;
    let mut endless = Endless {
        handed: 0,
        // Generous, so what fails is the count below rather than the read:
        // an over-reading probe is caught by an assertion that names the
        // number, not by a mysterious io error.
        ceiling: budget + FIRST_LINE_WINDOW,
    };
    assert_eq!(
        first_line_within(&mut endless, budget),
        Observed::Absent,
        "a source with no newline in it has no first line, and spending the budget is a \
         completed observation rather than an unfinished one"
    );
    assert_eq!(
        endless.handed, budget,
        "the probe is bounded by the length the file declares, and by nothing else"
    );

    // A device, a fifo or a socket declares no length, so the budget is
    // zero and the probe reads nothing at all. This is the shape a symlink
    // to /dev/zero presents to `first_line`.
    let mut device = Endless {
        handed: 0,
        ceiling: 1,
    };
    assert_eq!(first_line_within(&mut device, 0), Observed::Absent);
    assert_eq!(device.handed, 0, "a source with no length is not read");
}

// =======================================================================
// SWEEP-CLASSIFY-001: a source that answers `Interrupted`
// =======================================================================

/// When an otherwise ordinary source starts answering `Interrupted`, which is
/// the same thing as which of the probe's three reads it stops.
///
/// [`first_line_within`] reads three times and the finding's warning is about
/// exactly that: `SWEEP-CLASSIFY-001` had "two independent unbounded doors" and
/// "a successor who closes only (i) will believe the probe terminates". One
/// variant per read, one test per variant, and each test asserts *which* read
/// it stopped in rather than trusting the construction.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum InterruptAfter {
    /// Nothing: the first window read never completes. Door (i) — on master
    /// this is `Take` + `read_to_end`, and the retry is `std::io`'s.
    Nothing,
    /// This many bytes: the window read completes and the scan does not. Door
    /// (ii) — on master the retry is `newline_offset_from`'s own `continue`.
    Delivered(u64),
    /// The seek back to the start: the window read and the scan both complete
    /// and the re-read does not. The third read, which the finding does not
    /// number because on master it is door (i)'s `read_to_end` again.
    TheSeek,
}

/// A source that serves `bytes` until [`InterruptAfter`] says to stop, and
/// answers `Interrupted` for ever afterwards.
///
/// **Past `ceiling` it answers a *different* error instead**, which is what
/// lets an unbounded probe fail these tests in milliseconds rather than hang
/// the suite — the trade [`Endless`] makes, for the reason its comment gives: a
/// guard against non-termination that does not itself terminate is no guard.
///
/// The ceiling is [`INTERRUPT_CEILING`], a literal, and is deliberately **not**
/// derived from the allowance it measures. A threshold computed from the
/// constant under test grows with every mutation of that constant and so
/// catches none of them.
struct Interrupting {
    bytes: Vec<u8>,
    at: usize,
    when: InterruptAfter,
    ceiling: u64,
    /// Observed, not configured: which stage the probe actually reached.
    delivered: u64,
    rewound: bool,
    interruptions: u64,
}

impl Interrupting {
    fn new(bytes: Vec<u8>, when: InterruptAfter, ceiling: u64) -> Self {
        Self {
            bytes,
            at: 0,
            when,
            ceiling,
            delivered: 0,
            rewound: false,
            interruptions: 0,
        }
    }

    fn interrupting(&self) -> bool {
        match self.when {
            InterruptAfter::Nothing => true,
            InterruptAfter::Delivered(bytes) => self.delivered >= bytes,
            InterruptAfter::TheSeek => self.rewound,
        }
    }
}

impl Read for Interrupting {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.interrupting() {
            self.interruptions += 1;
            if self.interruptions > self.ceiling {
                return Err(io::Error::other(format!(
                    "the probe made {} interrupted reads and had not stopped, so it is \
                     unbounded on a source that answers Interrupted (SWEEP-CLASSIFY-001)",
                    self.interruptions
                )));
            }
            return Err(io::Error::from(io::ErrorKind::Interrupted));
        }
        let Some(rest) = self.bytes.get(self.at..) else {
            return Ok(0);
        };
        let take = rest.len().min(buf.len());
        let (Some(from), Some(into)) = (rest.get(..take), buf.get_mut(..take)) else {
            return Ok(0);
        };
        into.copy_from_slice(from);
        self.at += take;
        self.delivered += take as u64;
        Ok(take)
    }
}

impl Seek for Interrupting {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        self.rewound = true;
        if let SeekFrom::Start(at) = to {
            self.at = usize::try_from(at).unwrap_or(usize::MAX);
        }
        Ok(self.at as u64)
    }
}

/// Every interrupted-source test's bound, as literals.
///
/// `CEILING` is what the fixture refuses past, and `DEADLINE` is what the
/// caller refuses past; both are here rather than at the call sites so that
/// "the same bound, for all three reads" is a fact about one pair of numbers.
/// Neither is computed from `INTERRUPTED_ALLOWANCE`, which is private to
/// `classify` for that reason.
const INTERRUPT_CEILING: u64 = 4_000_000;
const INTERRUPT_DEADLINE: Duration = Duration::from_secs(20);

/// Drive `source` through the probe and return what it answered, refusing to
/// take longer than [`INTERRUPT_DEADLINE`].
///
/// The elapsed check is an assertion rather than a watchdog: nothing here can
/// stop a probe that does not return, and that is what `INTERRUPT_CEILING`
/// inside the fixture is for. This is the second signal, and it is the one the
/// finding asks for in seconds.
fn probe_under_interruption(source: &mut Interrupting, bound: u64) -> Observed<Vec<u8>> {
    let started = Instant::now();
    let answer = first_line_within(source, bound);
    let elapsed = started.elapsed();
    assert!(
        elapsed < INTERRUPT_DEADLINE,
        "the probe took {elapsed:?} against an interrupting source; the census holds the \
         physical worktree lock across it (SWEEP-CLASSIFY-001)"
    );
    answer
}

/// Door (i): the **window read**, which on master is `Take` + `read_to_end`.
///
/// `std::io` retries `Interrupted` inside `read_to_end` without limit and an
/// interrupted read spends none of a `Take`'s byte budget, so master's probe
/// never returns here. Measured on master's own copy of these two functions,
/// lifted verbatim into a standalone binary: no return within 25 seconds at
/// rustc 1.85.0 and at 1.97.1.
///
/// **The answer is the assertion, not merely the return.** A probe that
/// terminated by calling an interrupted source a husk would pass a test that
/// only checked it came back — and that is precisely the repair PR #137 wrote
/// and withdrew, because `Husk` is the reclaiming classification. So this
/// asserts `Incomplete` and asserts it is not `Absent`.
#[test]
fn the_window_read_stops_on_an_interrupting_source_and_does_not_call_it_absent() {
    let mut source = Interrupting::new(Vec::new(), InterruptAfter::Nothing, INTERRUPT_CEILING);
    let answer = probe_under_interruption(&mut source, FIRST_LINE_WINDOW * 4);

    assert_eq!(
        source.delivered, 0,
        "this door is the window read: the source handed out no bytes at all"
    );
    assert!(
        !source.rewound,
        "the scan and the re-read were never reached"
    );
    assert!(
        source.interruptions > 0,
        "the fixture never interrupted, so nothing here is measured"
    );
    assert!(
        source.interruptions <= INTERRUPT_CEILING,
        "{} interrupted reads and the probe had not stopped: it is unbounded on a source \
         that answers Interrupted, which is SWEEP-CLASSIFY-001",
        source.interruptions
    );
    assert_eq!(
        answer,
        Observed::Incomplete,
        "an observation that could not be completed is not an absence"
    );
    assert_ne!(
        answer,
        Observed::Absent,
        "SWEEP-CLASSIFY-001: `Absent` is `Husk`, which is the reclaiming answer"
    );
}

/// Door (ii): the **scan**, which is this crate's own `Interrupted` arm.
///
/// The finding's warning is this test's reason for existing: "a successor who
/// closes only (i) will believe the probe terminates". The source satisfies the
/// whole first window with newline-free bytes, so `read_to_end` returns and the
/// probe is inside `newline_offset_from` when the interruptions start — which
/// the `delivered` assertion below states rather than assumes.
#[test]
fn the_scan_stops_on_an_interrupting_source_and_does_not_call_it_absent() {
    let window = FIRST_LINE_WINDOW;
    let bytes = vec![b'x'; usize::try_from(window).expect("the window fits a usize")];
    let mut source = Interrupting::new(bytes, InterruptAfter::Delivered(window), INTERRUPT_CEILING);
    let answer = probe_under_interruption(&mut source, window * 4);

    assert_eq!(
        source.delivered, window,
        "this door is the scan: the window read must have completed first, or this test \
         is measuring door (i) again"
    );
    assert!(
        !source.rewound,
        "the probe stopped in the scan, before the re-read's seek"
    );
    assert!(
        source.interruptions > 0,
        "the fixture never interrupted, so nothing here is measured"
    );
    assert!(
        source.interruptions <= INTERRUPT_CEILING,
        "{} interrupted reads and the probe had not stopped: it is unbounded on a source \
         that answers Interrupted, which is SWEEP-CLASSIFY-001",
        source.interruptions
    );
    assert_eq!(answer, Observed::Incomplete);
    assert_ne!(
        answer,
        Observed::Absent,
        "SWEEP-CLASSIFY-001: `Absent` is `Husk`, which is the reclaiming answer"
    );
}

/// The **re-read**, the third read, reached only after the seek.
///
/// The finding numbers two doors because on master this read is door (i)'s
/// `read_to_end` again. It is a third *site* all the same, and a repair that
/// bounded the first two would leave it: the fixture serves a newline-free
/// window, then a newline the scan finds, and only starts interrupting once the
/// probe has sought back to the start — which `rewound` states.
#[test]
fn the_reread_stops_on_an_interrupting_source_and_does_not_call_it_absent() {
    let window = usize::try_from(FIRST_LINE_WINDOW).expect("the window fits a usize");
    // Newline-free for the whole window, so the scan is reached; terminated
    // just past it, so the scan finds an offset and the re-read is reached.
    let mut bytes = vec![b'x'; window + 32];
    bytes.push(b'\n');
    let bound = bytes.len() as u64;
    let mut source = Interrupting::new(bytes, InterruptAfter::TheSeek, INTERRUPT_CEILING);
    let answer = probe_under_interruption(&mut source, bound);

    assert!(
        source.rewound,
        "the re-read was never reached, so this test is measuring one of the other two"
    );
    assert!(
        source.delivered > FIRST_LINE_WINDOW,
        "the window read and the scan both completed first: {} bytes",
        source.delivered
    );
    assert!(
        source.interruptions > 0,
        "the fixture never interrupted, so nothing here is measured"
    );
    assert!(
        source.interruptions <= INTERRUPT_CEILING,
        "{} interrupted reads and the probe had not stopped: it is unbounded on a source \
         that answers Interrupted, which is SWEEP-CLASSIFY-001",
        source.interruptions
    );
    assert_eq!(answer, Observed::Incomplete);
    assert_ne!(
        answer,
        Observed::Absent,
        "SWEEP-CLASSIFY-001: `Absent` is `Husk`, which is the reclaiming answer"
    );
}

/// A source that interrupts and then **recovers** still has its first line.
///
/// The control the three tests above cannot be: each of them shows the probe
/// stops, and a probe that answered `Incomplete` the moment it saw one
/// `Interrupted` would satisfy all three while classifying every interrupted
/// read as unreadable. A burst well inside the allowance must be absorbed and
/// the line returned, which is also the property a retry exists for at all.
#[test]
fn a_burst_of_interruptions_inside_the_allowance_is_absorbed() {
    let line = committed_line_of_exactly("01BURST", 4096);
    let bound = line.len() as u64;
    let mut source = Bursty {
        bytes: line.clone(),
        at: 0,
        left: 512,
        interruptions: 0,
    };
    assert_eq!(
        first_line_within(&mut source, bound),
        Observed::Found(line[..line.len() - 1].to_vec()),
        "a source that interrupts and then delivers has its first line"
    );
    assert_eq!(
        source.interruptions, 512,
        "the fixture must actually have interrupted, or this measures nothing"
    );
}

/// Answers `Interrupted` `left` times and then behaves like an ordinary file.
struct Bursty {
    bytes: Vec<u8>,
    at: usize,
    left: u64,
    interruptions: u64,
}

impl Read for Bursty {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.left > 0 {
            self.left -= 1;
            self.interruptions += 1;
            return Err(io::Error::from(io::ErrorKind::Interrupted));
        }
        let Some(rest) = self.bytes.get(self.at..) else {
            return Ok(0);
        };
        let take = rest.len().min(buf.len());
        let (Some(from), Some(into)) = (rest.get(..take), buf.get_mut(..take)) else {
            return Ok(0);
        };
        into.copy_from_slice(from);
        self.at += take;
        Ok(take)
    }
}

impl Seek for Bursty {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        if let SeekFrom::Start(at) = to {
            self.at = usize::try_from(at).unwrap_or(usize::MAX);
        }
        Ok(self.at as u64)
    }
}

/// The probe retries `Interrupted` in **one** place, and reaches no retry it
/// does not own.
///
/// Three drivers above measure the three reads. This measures the shape that
/// makes three drivers enough: `SWEEP-CLASSIFY-001`'s two doors were two
/// independent retry sites — one in this crate's code and one inside
/// `std::io`'s `read_to_end` — and the finding's warning is that closing either
/// alone looks finished. So the count is the assertion, not the reads.
///
/// Comments and string literals are blanked first, because the prose in that
/// file says "Interrupted" many times; the test region is cut off first,
/// because this is a statement about the probe and not about its tests. Both
/// derivations are `crate::effects`'s own, which is what the classification
/// census uses.
///
/// **What this does not prove**: that the one site is bounded — the three tests
/// above are that — or that a read added later goes through it. It does catch
/// the two shapes that would reintroduce the defect, which are a second
/// `ErrorKind::Interrupted` arm and any return of `read_to_end`.
#[test]
fn the_probe_has_exactly_one_interrupted_retry_site() {
    let source = include_str!("classify.rs");
    let code =
        crate::effects::blank_comments_and_strings(&crate::effects::production_region(source));

    // The positive control: a zero below is only evidence if this search can
    // see the file's code at all. `read_step` is the retry site's own name and
    // appears at its declaration and at both of its callers.
    assert!(
        code.matches("read_step").count() >= 3,
        "the blanked production region does not contain the probe's own code, so the \
         counts below measure nothing: {} bytes",
        code.len()
    );
    assert_eq!(
        code.matches("ErrorKind::Interrupted").count(),
        1,
        "SWEEP-CLASSIFY-001: every read in this module retries Interrupted through one \
         bounded site, and a second arm is the second door the finding warns about"
    );
    assert_eq!(
        code.matches("read_to_end").count(),
        0,
        "SWEEP-CLASSIFY-001 door (i): `read_to_end` retries Interrupted inside `std::io` \
         without limit, and an interrupted read spends none of a `Take`'s byte budget"
    );
}

/// The probe grows the line it is going to return through **one** fallible
/// reservation, and reaches no infallible growth beside it.
///
/// The sibling of `the_probe_has_exactly_one_interrupted_retry_site`, and for
/// a defect of the same shape one adverse condition over.
/// `by_ref().take(want).read_to_end(&mut into)` grew through `Vec::try_reserve`
/// inside `std::io` and reported a refusal as an `io::Error`; the hand-written
/// loop that replaced it grew through `Vec::extend_from_slice`, which aborts
/// the process. So the census that closed the door replaced an answer with a
/// death: on a 64 MiB first line under a 48 MiB address-space limit, `231c1aad`
/// answered `Husk` at exit 0 and that round answered nothing at exit 134.
///
/// `a_reservation_the_probe_cannot_make_answers_incomplete_rather_than_aborting`
/// drives the reservation itself and shows it answers. What it cannot show is
/// that the loop goes through it, because the only reservation the probe makes
/// on any host this crate builds for is at most one `SCAN_CHUNK` and succeeds.
/// This is that half: one `try_reserve` and one `extend_from_slice` in the
/// whole production region, which `reserve` and `append` are. A second
/// `extend_from_slice` is a growth that can abort, and a missing `try_reserve`
/// is the same defect with the helper deleted.
///
/// Comments and string literals are blanked and the test region is cut off
/// first, both through `crate::effects`' own derivations — the same two the
/// retry-site census uses.
#[test]
fn the_probe_grows_the_line_it_returns_through_a_fallible_reservation() {
    let source = include_str!("classify.rs");
    let code =
        crate::effects::blank_comments_and_strings(&crate::effects::production_region(source));

    // The positive control, for the reason the retry-site census has one: a
    // count of one is only evidence if this search can see the code at all.
    assert!(
        code.matches("read_up_to").count() >= 3,
        "the blanked production region does not contain the probe's own code, so the counts \
         below measure nothing: {} bytes",
        code.len()
    );
    assert_eq!(
        code.matches("try_reserve").count(),
        1,
        "SWEEP-CLASSIFY-001, one condition over: the probe's answer buffer grows through one \
         fallible reservation, and without it an allocation the host refuses aborts the \
         command mid-census instead of classifying"
    );
    assert_eq!(
        code.matches("extend_from_slice").count(),
        1,
        "the one append is `append`'s, past a reservation that was granted; a second is a \
         growth that aborts rather than answers"
    );
}

/// Every production site that *dispatches* on a `RunDirClass` is an
/// exhaustive `match`, so a fourth classification is a compile error rather
/// than a silent default.
///
/// **This is the claim this pull request's body makes about why the signature
/// change is the safe shape, and round 1 made it while it was false.** The
/// census's own dispatch was `if class == RunDirClass::Indeterminate`, an
/// equality guard: adding a variant to one compiles, takes the other branch and
/// says nothing. Two independent review lenses reached that conclusion by
/// different routes, and the repair was to make the claim true rather than to
/// strike it — `scan_classified` and `discovery`'s three reader predicates are
/// exhaustive `match`es now, and this keeps them that way.
///
/// The four spellings refused are the four that compile past a new variant:
/// `==`, `!=`, `matches!` and `if let`. A `_ =>` arm would too, and is not
/// refused here — `RunDirClass` has no field to fall through on, so a
/// catch-all in one of these files would be visible in review as the thing it
/// is; what a census is for is the shape that reads as ordinary code.
///
/// Line-oriented on the blanked production region, which preserves newlines:
/// each of these forms is one line of `rustfmt` output at these widths.
#[test]
fn no_production_dispatch_on_a_classification_is_an_equality_guard() {
    let sources = [
        ("src/rundir/discovery.rs", include_str!("discovery.rs")),
        ("src/rundir/classify.rs", include_str!("classify.rs")),
        (
            "src/engine/topology/startup.rs",
            include_str!("../engine/topology/startup.rs"),
        ),
    ];
    let mut dispatches = 0usize;
    let mut mentions = 0usize;
    for (path, source) in sources {
        let code =
            crate::effects::blank_comments_and_strings(&crate::effects::production_region(source));
        // The positive control: a file whose code this search cannot see
        // reports no guard for the same reason it reports no `match` arm.
        let named = code.matches("RunDirClass").count();
        assert!(
            named >= 3,
            "{path}: the blanked production region names RunDirClass {named} times, so the \
             search below measures nothing"
        );
        mentions += named;
        for (number, line) in code.lines().enumerate() {
            if !line.contains("RunDirClass") {
                continue;
            }
            for guard in ["==", "!=", "matches!", "if let"] {
                assert!(
                    !line.contains(guard),
                    "{path}:{}: `{guard}` on a classification compiles unchanged when a \
                     fourth one is added, and silently takes the other branch. The body of \
                     this pull request claims a missed arm is a compile error; only an \
                     exhaustive `match` makes that true.\n    {}",
                    number + 1,
                    line.trim()
                );
            }
            if line.trim_start().starts_with("RunDirClass::") && line.contains("=>") {
                dispatches += 1;
            }
        }
    }
    assert!(
        dispatches >= 12,
        "only {dispatches} exhaustive match arms over a classification were found across the \
         three files; the three reader predicates and `scan_classified` are four sites of \
         three arms each, so the search has stopped seeing them"
    );
    assert!(mentions > 20, "{mentions} mentions is not this tree");
}

/// `RetainReason::PROOF_KINDS` is `KINDS` without the classifier's one, and
/// nothing else.
///
/// Two censuses measure `RetainReason` and this is what stops them drifting:
/// the proof grid asserts it exercises every `PROOF_KINDS` entry, and
/// `every_retain_reason_kind_deletes_nothing` asserts the census's retain arm
/// covers every `KINDS` entry. Without this, a variant added to `KINDS` alone
/// would be exercised by the second and invisible to the first, and a variant
/// added to `PROOF_KINDS` alone would name a refusal that is not a reason.
#[test]
fn the_proof_kinds_are_the_retain_kinds_the_classifier_does_not_add() {
    let classifier_only = ["classification-incomplete"];
    let expected: Vec<&str> = RetainReason::PROOF_KINDS
        .iter()
        .copied()
        .chain(classifier_only)
        .collect();
    assert_eq!(
        RetainReason::KINDS.to_vec(),
        expected,
        "KINDS is PROOF_KINDS followed by the kinds the classifier produces"
    );
    assert_eq!(
        RetainReason::ClassificationIncomplete.kind(),
        classifier_only[0],
        "the classifier's reason is the one named above"
    );
    assert!(
        !RetainReason::PROOF_KINDS.contains(&RetainReason::ClassificationIncomplete.kind()),
        "the ownership proof never answers the classifier's reason"
    );
}

/// The budget really is *the file's own length*, and a line that runs past
/// the window is still found through it.
///
/// The pair matters: the first half is what makes the probe terminate, the
/// second is what stops that bound from becoming a classification cap — the
/// exact trade `FIRST_LINE_CAP` got wrong and a bound-shaped repair could
/// reintroduce.
#[test]
fn the_budget_is_the_files_length_and_a_line_past_the_window_is_still_read() {
    let root = scratch("budget");
    let window = usize::try_from(FIRST_LINE_WINDOW).expect("the window fits a usize");
    let line = committed_line_of_exactly("01BUDGET", window + 4096);
    let path = root.join("long").join(EVENT_LOG);
    write(&path, &line);

    let mut file = File::open(&path).expect("open");
    assert_eq!(
        file.metadata().expect("stat").len(),
        line.len() as u64,
        "the bound the probe takes is this number"
    );
    assert_eq!(
        first_line(&mut file),
        Observed::Found(line[..line.len() - 1].to_vec()),
        "a line past the window is still a line"
    );
    assert_eq!(
        classify_run_dir(root.join("long").as_path()),
        RunDirClass::Committed,
        "a committed run over the window is never excluded by a read bound"
    );
}

/// A file with no newline anywhere is a husk, and is answered without
/// materialising it.
///
/// This is what the window was introduced for and the property the repair
/// had to keep: `newline_offset_from` scans a fixed `SCAN_CHUNK` buffer, so
/// the cost of "there is no newline" is independent of the file's size.
/// Sixteen windows of it, which the pre-repair probe would have read one
/// megabyte of and this one reads all of in 64 KiB at a time.
///
/// It does **not** establish termination and no longer claims to
/// (`PR5-RD-001`): one finite regular file reaches end of file under every
/// implementation of this function, including the one that never returned
/// for a source that has no end. `the_first_line_probe_spends_its_budget_
/// and_stops` and `a_run_directory_whose_log_never_ends_is_still_classified`
/// carry that.
///
/// `Husk` is the safe direction here, but **not for the reason this comment
/// used to give**. It said a husk is never deleted on shape alone because
/// deletion requires the ownership proof and the proof requires
/// `committed.json` to be absent. That is true of
/// `PrivateHalfOwnership::Proven` and false of `NothingBound`, which reclaims
/// the public half with no commit-record check at all. What retains this
/// directory is that `unbound_shape` reclaims only a bare directory or one
/// holding the staging file alone, and this one holds an `events.jsonl`.
/// `first_committed_line` carries the whole argument and the residual
/// (`SWEEP-CLASSIFY-009`).
#[test]
fn a_log_with_no_newline_at_all_is_a_husk_however_long_it_is() {
    let root = scratch("no-newline");
    let window = usize::try_from(FIRST_LINE_WINDOW).expect("the window fits a usize");
    // Valid JSON, so the answer cannot come from the parse.
    let head = committed_line_of_exactly("01NONL", 4096);
    let mut bytes = head[..head.len() - 1].to_vec();
    bytes.extend(std::iter::repeat_n(b'x', window * 16));
    assert!(!bytes.contains(&b'\n'));
    let public = root.join("long");
    write(&public.join(EVENT_LOG), &bytes);
    assert_eq!(classify_run_dir(&public), RunDirClass::Husk);

    let mut file = File::open(public.join(EVENT_LOG)).expect("open");
    assert_eq!(
        first_line(&mut file),
        Observed::Absent,
        "no newline is no first line, not an empty one"
    );
}

/// Where [`endless_log_classification_helper`] is pointed.
const ENDLESS_LOG_DIR: &str = "UPSTROKE_ENDLESS_LOG_DIR";

/// Set when the helper may also *open* the log itself and measure
/// [`first_line`]'s bound over it — true of a device, false of a fifo.
const ENDLESS_LOG_PROBE: &str = "UPSTROKE_ENDLESS_LOG_PROBE";

/// The child half of
/// [`a_run_directory_whose_log_never_ends_is_still_classified`].
///
/// A subprocess rather than a thread, and the reason is the failure mode
/// rather than the success one: a probe that does not terminate cannot be
/// stopped from inside the process it is running in, and the mutation this
/// guards against (an unconditional `read_to_end`) also grows memory
/// without bound while it fails to return. A child can be killed at a
/// deadline; a thread would take the whole suite, and the machine, with it.
#[test]
#[ignore = "subprocess helper"]
fn endless_log_classification_helper() {
    let Ok(dir) = std::env::var(ENDLESS_LOG_DIR) else {
        return;
    };
    assert_eq!(
        classify_run_dir(Path::new(&dir)),
        RunDirClass::Husk,
        "a log with no end holds no newline-terminated run_started"
    );
    // The second axis, and it is only measurable where the source can be
    // opened at all. `classify_run_dir` answering `Husk` is satisfied by a
    // guard that refuses the *name* and by a bound that reads the *bytes*,
    // so on its own it cannot say which one answered — and once
    // `first_committed_line` refuses to open a non-regular file, the
    // endless-device witness would silently stop reaching the bound it was
    // built for (`PR5-RD-001`). Here the child holds the guard's verdict
    // constant and varies the handle: it opens the device itself and asserts
    // the bounded read *also* terminates on it.
    if std::env::var_os(ENDLESS_LOG_PROBE).is_some() {
        let mut device = File::open(Path::new(&dir).join(EVENT_LOG)).expect("the log opens");
        assert_eq!(
            first_line(&mut device),
            Observed::Absent,
            "the bounded read must terminate on the device too, not only the guard"
        );
    }
    std::process::exit(0);
}

/// Run [`endless_log_classification_helper`] against `public` in a child,
/// and fail with `never_returned` if it has not answered within 20 seconds.
///
/// A subprocess, not a thread, for the reason the helper's own comment
/// gives: a probe that does not terminate cannot be stopped from inside its
/// own process, and both shapes this drives — an unbounded `read_to_end`
/// and a blocked `open(2)` — are exactly that.
///
/// Unix-gated because both callers are: `/dev/zero` and `mkfifo` are the two
/// ways to get hold of a non-terminating source without privilege and
/// neither exists on Windows, so on the guest this would be dead code — and
/// the guest's `-D warnings` says so, which is how this gate was found.
#[cfg(unix)]
fn classification_must_answer(public: &Path, probe: bool, never_returned: &str) {
    let helper = format!(
        "{}::endless_log_classification_helper",
        module_path!()
            .split_once("::")
            .expect("this module is not the crate root")
            .1
    );
    let mut command =
        std::process::Command::new(std::env::current_exe().expect("the test executable"));
    command
        .args([helper.as_str(), "--ignored", "--exact"])
        .env(ENDLESS_LOG_DIR, public)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    if probe {
        command.env(ENDLESS_LOG_PROBE, "1");
    }
    let mut child = command.spawn().expect("spawn the classification helper");

    let deadline = Instant::now() + Duration::from_secs(20);
    let outcome = loop {
        match child.try_wait().expect("wait on the helper") {
            Some(status) => break Some(status),
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            None => std::thread::sleep(Duration::from_millis(20)),
        }
    };
    let status = outcome.unwrap_or_else(|| panic!("{never_returned}"));
    assert!(
        status.success(),
        "the helper reached a verdict but it was the wrong one, or it died: {status:?}"
    );
}

/// A public run directory whose `events.jsonl` is a **fifo with no writer**
/// is classified, and classified promptly (`PR5-CONF-001`).
///
/// The sibling below plants an endless *device*, whose `open` returns and
/// whose `read` never ends; this plants a source whose `open` itself never
/// returns, which no bound on the read can defend against because the bound
/// is taken by `fstat` on a handle that is never produced. `startup_census`
/// requires every entry to classify before a write command proceeds and the
/// command holds the physical worktree lock across the census, so the
/// consequence is the same one `PR5-RD-001` was repaired for: a lock held
/// for ever by a process that will never make progress.
///
/// The two axes this crosses are the *file type* and the *syscall that
/// meets it*. Held constant: the directory shape, which is a perfectly
/// ordinary public run directory — the only thing that varies from a
/// `Committed` one is the type of the `events.jsonl` entry.
///
/// Unix only, because a fifo is where a blocking `open` can be got hold of
/// without privilege; `mkfifo` has no Windows counterpart at all.
#[cfg(unix)]
#[test]
fn a_run_directory_whose_log_blocks_on_open_is_still_classified() {
    use std::os::unix::fs::FileTypeExt as _;

    let root = scratch("fifo");
    let public = root.join("run");
    fs::create_dir_all(&public).expect("public");
    let log = public.join(EVENT_LOG);
    let name = std::ffi::CString::new(log.as_os_str().as_encoded_bytes())
        .expect("a scratch path holds no interior NUL");
    // SAFETY: `name` is a live NUL-terminated path in a directory this test
    // just created; `mkfifo` borrows it for the duration of the call.
    let made = unsafe { libc::mkfifo(name.as_ptr(), 0o600) };
    assert_eq!(
        made,
        0,
        "could not plant the fifo: {}",
        std::io::Error::last_os_error()
    );
    assert!(
        fs::symlink_metadata(&log)
            .expect("stat the fifo")
            .file_type()
            .is_fifo(),
        "the planted entry must really be a fifo, or nothing here is measured"
    );
    // The premise, stated rather than assumed: `stat` answers about this
    // entry immediately, so a guard that consults the type before opening
    // can terminate — and that is the only reason one is possible.
    assert_eq!(
        fs::symlink_metadata(&log).expect("stat the fifo").len(),
        0,
        "a fifo declares no length"
    );

    classification_must_answer(
        &public,
        false,
        "classify_run_dir did not return within 20s for an events.jsonl that is a \
             writer-less fifo. `File::open` blocks in the kernel before any bound on the \
             read applies, the startup census would never classify this entry, and the \
             write command would hold the worktree lock for ever (PR5-CONF-001)",
    );
}

/// A public run directory whose `events.jsonl` is a real endless device is
/// classified, and classified *quickly* (`PR5-RD-001`).
///
/// `startup_census` requires **every** run-directory entry to be classified
/// before a write command proceeds, and the write command holds the physical
/// worktree lock while it does that. An entry that never classifies is
/// therefore not a slow census: it is a lock held for ever by a process that
/// will never make progress, and no later command in that worktree can run.
/// (The packet's two answers are `Committed` and `Husk`; the probe also has
/// `RunDirClass::Indeterminate` for a read it could not finish, and this
/// device is not that shape -- it delivers bytes and never ends, so the
/// bound that stops it here is the file's own declared length.)
///
/// Unix only because `/dev/zero` is where a source with no end can be got
/// hold of without privilege. The platform-free half of the same claim —
/// that the probe spends a finite budget and stops — is
/// `the_first_line_probe_spends_its_budget_and_stops`, which runs on the
/// Windows guest too.
///
/// **The child probes the device as well as the directory**, and that is not
/// decoration (`PR5-CONF-001`). Once `first_committed_line` refuses to open
/// anything that is not a regular file, this planted symlink is answered by
/// the *guard*, so the classification alone would no longer reach the bound
/// this test exists for — a green `Husk` would mean the name was refused and
/// say nothing about the read. The child therefore holds the class constant
/// and varies the handle: it asserts `Husk`, then opens the real device and
/// asserts the bounded read terminates on it too. Both assertions run inside
/// the same 20-second deadline, so either one failing to *return* fails
/// here rather than hanging the suite.
#[cfg(unix)]
#[test]
fn a_run_directory_whose_log_never_ends_is_still_classified() {
    let root = scratch("endless");
    let public = root.join("run");
    fs::create_dir_all(&public).expect("public");
    assert!(
        Path::new("/dev/zero").exists(),
        "this host has no endless device, so nothing here is measured"
    );
    std::os::unix::fs::symlink("/dev/zero", public.join(EVENT_LOG)).expect("symlink");
    // The device is what the probe will actually meet: a handle that opens,
    // declares no length, and never reaches end of file.
    let device = File::open(public.join(EVENT_LOG)).expect("the log opens");
    assert_eq!(
        device.metadata().expect("stat").len(),
        0,
        "a character device declares no length, which is the probe's budget"
    );
    drop(device);

    classification_must_answer(
        &public,
        true,
        "classify_run_dir or first_line did not return within 20s for an events.jsonl \
             that never ends. The startup census would never classify this entry and the \
             write command would hold the worktree lock for ever (PR5-RD-001)",
    );
}

// =======================================================================
// Readers by commitment
// =======================================================================

/// A repository holding one committed run, one husk older than it and one
/// husk newer than it — so a reader that returned husks would be caught
/// whichever end of the sort it went wrong at.
fn repo_with_a_committed_run_between_two_husks(tag: &str) -> PathBuf {
    let repo = scratch(tag).join("repo");
    fs::create_dir_all(runs_root(&repo).join("01AAAHUSK")).expect("older husk");
    write(
        &public_dir(&repo, "01AAAHUSK").join(PLAN),
        b"{\"tasks\":[]}",
    );
    commit_run(&repo, "01BBBRUN");
    fs::create_dir_all(runs_root(&repo).join("01ZZZHUSK")).expect("newer husk");
    write(
        &public_dir(&repo, "01ZZZHUSK").join(MARKER),
        &any_marker_bytes(),
    );
    repo
}

/// Every reader **in this module**, crossed with every husk **shape** —
/// the second axis, and the one this fixture used to be too narrow on.
///
/// `startup_census` names five readers — `list_runs`, `latest_run`,
/// `resolve_run_id`, `find_question`, `status` — and four of them live
/// here. The fifth is the `status` command, which reaches run directories
/// through `resolve_run_id` and `husk_report`, and its husk behaviour is
/// pinned in its own module by
/// `status_asked_for_a_husk_id_names_which_husk_it_is`.
///
/// Four readers against two shapes caught any reader that simply stopped
/// filtering: `find_question` scanning `run_dir_names`, and `latest_run`
/// taking the newest directory, both die here. What it could not see was a
/// shape it did not build. Its husks are a markerless directory carrying
/// content and one with a **well-formed** marker, and
/// `a_committed_run_is_never_excluded_because_of_a_marker` uses well-formed
/// markers too — so a filter that admitted exactly the *malformed-marker*
/// husk changed no measured answer, and the readers' behaviour over that
/// shape was unpinned in both directions. Measured surviving the whole
/// suite on Linux and on the Windows guest.
///
/// `01ZZZMALFORMED` is therefore built to win every reader it could: it
/// sorts lexically last, so `latest_run` would take it, and it carries the
/// question id being searched for, so `find_question` would return it.
#[test]
fn every_reader_returns_committed_directories_only() {
    let repo = repo_with_a_committed_run_between_two_husks("readers");
    // The third shape: a marker that is present and unparseable. Not a
    // fifth reader — the four this module owns are all here already, and
    // the fifth, `status`, is pinned in `status.rs` — a third *shape*.
    let malformed = public_dir(&repo, "01ZZZMALFORMED");
    fs::create_dir_all(&malformed).expect("malformed-marker husk");
    write(&malformed.join(MARKER), b"{ not json at all");
    for husk in ["01AAAHUSK", "01ZZZHUSK", "01ZZZMALFORMED"] {
        let questions = public_dir(&repo, husk).join("questions");
        fs::create_dir_all(&questions).expect("questions");
        fs::write(questions.join("q-HUSK.json"), "{}").expect("question");
    }
    let questions = public_dir(&repo, "01BBBRUN").join("questions");
    fs::create_dir_all(&questions).expect("questions");
    fs::write(questions.join("q-REAL.json"), "{}").expect("question");

    assert_eq!(list_runs(&repo), ["01BBBRUN"], "list_runs");
    assert_eq!(latest_run(&repo).as_deref(), Some("01BBBRUN"), "latest_run");
    assert_eq!(
        resolve_run_id(&repo, "01BBBRUN").expect("the committed run resolves"),
        "01BBBRUN"
    );
    for husk in ["01AAAHUSK", "01ZZZHUSK", "01ZZZMALFORMED"] {
        let error = resolve_run_id(&repo, husk).expect_err("a husk is not a run");
        assert!(
            error.to_string().contains("never recorded a committed"),
            "resolve_run_id must say why: {error}"
        );
    }
    assert_eq!(
        find_question(&repo, "q-REAL")
            .expect("the committed run's question")
            .run_id,
        "01BBBRUN"
    );
    let error = find_question(&repo, "q-HUSK").expect_err("a husk's question is not findable");
    assert!(error.to_string().contains("no question"), "{error}");

    // And the husks are still there: a reader observes, it never reclaims.
    assert_eq!(
        list_husks(&repo),
        ["01AAAHUSK", "01ZZZHUSK", "01ZZZMALFORMED"]
    );
    assert_eq!(run_dir_names(&repo).len(), 4);
}

#[test]
fn a_committed_run_is_never_excluded_because_of_a_marker() {
    // The other half of the behaviour change, and the half a plausible
    // suite forgets: `run_creation` says readers "never return a directory
    // without a committed run_started **and never hide one because of a
    // marker**". Both marker shapes, and with a newer husk present so the
    // committed run has to win `latest_run` on its merits.
    let repo = scratch("markedcommitted").join("repo");
    commit_run(&repo, "01AAAMARKED");
    commit_run(&repo, "01BBBSTAGED");
    write(
        &public_dir(&repo, "01AAAMARKED").join(MARKER),
        &any_marker_bytes(),
    );
    write(
        &public_dir(&repo, "01BBBSTAGED").join(MARKER_STAGED),
        &any_marker_bytes(),
    );
    fs::create_dir_all(runs_root(&repo).join("01ZZZHUSK")).expect("newer husk");

    assert_eq!(list_runs(&repo), ["01AAAMARKED", "01BBBSTAGED"]);
    assert_eq!(
        latest_run(&repo).as_deref(),
        Some("01BBBSTAGED"),
        "a committed-but-marked run is the latest run, and a husk newer \
             than it does not become one"
    );
    for id in ["01AAAMARKED", "01BBBSTAGED"] {
        assert_eq!(resolve_run_id(&repo, id).expect("resolves"), id);
        assert_eq!(
            classify_run_dir(&public_dir(&repo, id)),
            RunDirClass::Committed
        );
    }
}

#[test]
fn latest_run_skips_a_husk_that_would_otherwise_shadow_it() {
    // The named change: "legacy husks that today shadow latest_run are no
    // longer listed". Asserted from the shadowing direction, because that
    // is the operator-visible symptom.
    let repo = repo_with_a_committed_run_between_two_husks("shadow");
    assert_eq!(latest_run(&repo).as_deref(), Some("01BBBRUN"));
    assert!(
        run_dir_names(&repo)
            .last()
            .is_some_and(|last| last == "01ZZZHUSK"),
        "the husk really is the newest directory, so the skip is doing work"
    );
}

// =======================================================================
// The private half's ownership
// =======================================================================

const BOUND_RUN: &str = "01BOUNDHUSK000000000000000";
const BOUND_INCARNATION: &str = "01INCARNATION00000000000000";

/// A husk at P3b–P5: the marker published, the private half created, the
/// owner record published, and no commit record. The one shape the proof
/// is supposed to accept.
struct BoundHusk {
    root: PathBuf,
    repo: PathBuf,
    private_root: PathBuf,
    repo_key: RepoKey,
    /// Where the private half's bytes are written.
    private: PathBuf,
    marker: CreatingMarker,
    owner: OwnerRecord,
}

impl BoundHusk {
    fn new(tag: &str) -> Self {
        Self::at(scratch(tag))
    }

    /// The same husk, under a root the caller already owns.
    ///
    /// Extracted from [`Self::new`] so that a fixture can be built inside
    /// an acquired `scratch_tree` root rather than beside one: the
    /// committed-record witness needs both — a husk the ownership proof
    /// refuses, and a root the scratch token authorises reclaiming.
    fn at(root: PathBuf) -> Self {
        let repo = root.join("repo");
        let private_root = root.join("private");
        let public = public_dir(&repo, BOUND_RUN);
        fs::create_dir_all(&public).expect("public");
        fs::create_dir_all(private_root.join("runs")).expect("runs root");
        let private = fs::canonicalize(private_root.join("runs"))
            .expect("canonical runs root")
            .join(BOUND_RUN);
        let repo_key = RepoKey::v1(&root.join("git-dir"));
        let policy = crate::runner::policy::host_policy();
        let marker = CreatingMarker {
            run_id: BOUND_RUN.to_owned(),
            repo_key: repo_key.as_str().to_owned(),
            private_dir: private.to_string_lossy().into_owned(),
            incarnation: BOUND_INCARNATION.to_owned(),
            pid: std::process::id(),
            runner_policy_sha256: runner_policy_sha256(&policy),
        };
        let owner = OwnerRecord {
            run_id: BOUND_RUN.to_owned(),
            repo_key: repo_key.as_str().to_owned(),
            public_dir: fs::canonicalize(&public)
                .expect("canonical public")
                .to_string_lossy()
                .into_owned(),
            incarnation: BOUND_INCARNATION.to_owned(),
            runner: policy,
        };
        Self {
            root,
            repo,
            private_root,
            repo_key,
            private,
            marker,
            owner,
        }
    }

    fn public(&self) -> PathBuf {
        public_dir(&self.repo, BOUND_RUN)
    }

    /// Publish both halves through the funnels, in the packet's order.
    fn publish(&self) {
        let hooks = &mut NoHooks;
        let public = self.public();
        create_public_dir(&public, hooks).expect("P0");
        stage_marker(&public, &self.marker, hooks).expect("P1a");
        publish_marker(&public, hooks).expect("P1b");
        create_private_dir(&self.private, hooks).expect("P3");
        stage_owner_record(&self.private, &self.owner, hooks).expect("P3a");
        publish_owner_record(&self.private, hooks).expect("P3b");
    }

    fn prove(&self) -> PrivateHalfOwnership {
        prove_private_half_ownership(&self.public(), &self.repo_key, &self.private_root)
    }
}

/// Every file below `root`, by relative path, so "byte-identical
/// afterwards" is an assertion rather than a hope.
fn snapshot_tree(root: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
    let mut out = std::collections::BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if let Ok(bytes) = fs::read(&path) {
                out.insert(
                    path.strip_prefix(root).unwrap_or(&path).to_path_buf(),
                    bytes,
                );
            }
        }
    }
    out
}

/// A directory link: a POSIX symlink, or on Windows a **junction**.
///
/// `mklink /J` rather than `/D` because a junction needs no privilege and
/// is exactly the reparse point `expected_failures_refusals[0]` names
/// beside a symlink. A refusal that only fired on POSIX symlinks would
/// pass every Linux test and refuse nothing on the platform the word
/// "junction" is about.
fn link_dir(link: &Path, target: &Path) {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).expect("symlink");
    }
    #[cfg(windows)]
    {
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .status()
            .expect("mklink runs");
        assert!(
            status.success(),
            "creating a junction must succeed; an unmakeable junction is a \
                 failure of this test, never a skip"
        );
    }
    assert!(
        fs::symlink_metadata(link).is_ok(),
        "the link must exist afterwards"
    );
}

/// What a case expects the proof to answer.
#[derive(Debug, PartialEq, Eq)]
enum Expect {
    Proven,
    Nothing(UnboundShape),
    /// The kind, and — when the owner record is what disagreed — the field.
    Retained(&'static str, Option<OwnerField>),
}

struct ProofCase {
    name: &'static str,
    /// Applied to the records before publication.
    before: fn(&mut BoundHusk),
    /// Applied to the bytes on disk after publication.
    after: fn(&BoundHusk),
    expect: Expect,
}

fn nothing(_: &mut BoundHusk) {}
fn nothing_after(_: &BoundHusk) {}

/// One case per conjunct, because every conjunct is separately droppable
/// and a suite testing the happy path plus one negative passes with any
/// single one removed.
fn proof_cases() -> Vec<ProofCase> {
    vec![
        ProofCase {
            name: "a bound husk without a commit record yields a token",
            before: nothing,
            after: nothing_after,
            expect: Expect::Proven,
        },
        ProofCase {
            name: "malformed marker",
            before: nothing,
            after: |husk| write(&husk.public().join(MARKER), b"{ not json at all"),
            expect: Expect::Retained("marker-unparseable", None),
        },
        ProofCase {
            name: "forged marker naming a foreign run",
            before: |husk| husk.marker.run_id = "01FOREIGNRUN00000000000000".to_owned(),
            after: nothing_after,
            expect: Expect::Retained("marker-run-id-mismatch", None),
        },
        ProofCase {
            name: "copied husk from another repository",
            before: |husk| {
                husk.marker.repo_key = RepoKey::v1(&husk.root.join("another-git-dir"))
                    .as_str()
                    .to_owned();
            },
            after: nothing_after,
            expect: Expect::Retained("marker-repo-key-mismatch", None),
        },
        ProofCase {
            name: "locator outside the authorized private root",
            before: |husk| {
                let foreign = husk.root.join("foreign-root").join("runs");
                fs::create_dir_all(&foreign).expect("foreign root");
                husk.private = fs::canonicalize(&foreign)
                    .expect("canonical foreign root")
                    .join(BOUND_RUN);
                husk.marker.private_dir = husk.private.to_string_lossy().into_owned();
            },
            after: nothing_after,
            expect: Expect::Retained("locator-outside-authorized-root", None),
        },
        ProofCase {
            name: "locator through a reparse point",
            before: |husk| {
                let real = husk.private_root.join("elsewhere");
                fs::create_dir_all(&real).expect("real private half");
                let link = husk.private_root.join("runs").join(BOUND_RUN);
                link_dir(&link, &real);
                // The marker records the *link*, which is what a census
                // has to follow and what the chain check has to refuse.
                husk.private = link.clone();
                husk.marker.private_dir = link.to_string_lossy().into_owned();
            },
            after: nothing_after,
            expect: Expect::Retained("locator-through-reparse-point", None),
        },
        ProofCase {
            name: "private target without an owner record",
            before: nothing,
            after: |husk| {
                fs::remove_file(husk.private.join(OWNER_RECORD)).expect("remove owner record");
            },
            expect: Expect::Retained("owner-record-missing", None),
        },
        ProofCase {
            name: "owner record that cannot be read",
            before: nothing,
            after: |husk| write(&husk.private.join(OWNER_RECORD), b"{ not json"),
            expect: Expect::Retained("owner-record-unparseable", None),
        },
        ProofCase {
            name: "owner record disagreeing on run id",
            before: |husk| husk.owner.run_id = "01OTHERRUN0000000000000000".to_owned(),
            after: nothing_after,
            expect: Expect::Retained("owner-record-disagrees", Some(OwnerField::RunId)),
        },
        ProofCase {
            name: "owner record disagreeing on repo key",
            before: |husk| {
                husk.owner.repo_key = RepoKey::v1(&husk.root.join("third-git-dir"))
                    .as_str()
                    .to_owned();
            },
            after: nothing_after,
            expect: Expect::Retained("owner-record-disagrees", Some(OwnerField::RepoKey)),
        },
        ProofCase {
            name: "owner record disagreeing on public path",
            before: |husk| {
                husk.owner.public_dir = husk
                    .root
                    .join("some-other-run-directory")
                    .to_string_lossy()
                    .into_owned();
            },
            after: nothing_after,
            expect: Expect::Retained("owner-record-disagrees", Some(OwnerField::PublicDir)),
        },
        ProofCase {
            name: "owner record disagreeing on incarnation",
            before: |husk| husk.owner.incarnation = "01ANOTHERINCARNATION000000".to_owned(),
            after: nothing_after,
            expect: Expect::Retained("owner-record-disagrees", Some(OwnerField::Incarnation)),
        },
        ProofCase {
            name: "owner record naming another runner boundary",
            before: |husk| husk.owner.runner = another_policy(),
            after: nothing_after,
            expect: Expect::Retained("owner-record-disagrees", Some(OwnerField::RunnerDigest)),
        },
        ProofCase {
            name: "marker-less husk carrying run-scoped content",
            before: nothing,
            after: |husk| {
                fs::remove_file(husk.public().join(MARKER)).expect("remove marker");
                write(&lock_file(&husk.public()), b"");
            },
            expect: Expect::Retained("markerless-with-content", None),
        },
        ProofCase {
            name: "private half carrying a commit record",
            before: nothing,
            after: |husk| write(&husk.private.join(COMMIT_RECORD), b"{}"),
            expect: Expect::Retained("possibly-committed", None),
        },
        ProofCase {
            name: "bare public directory",
            before: nothing,
            after: |husk| {
                fs::remove_file(husk.public().join(MARKER)).expect("remove marker");
            },
            expect: Expect::Nothing(UnboundShape::Bare),
        },
        ProofCase {
            name: "staged marker only",
            before: nothing,
            after: |husk| {
                fs::rename(
                    husk.public().join(MARKER),
                    husk.public().join(MARKER_STAGED),
                )
                .expect("unpublish the marker");
            },
            expect: Expect::Nothing(UnboundShape::StagedMarkerOnly),
        },
        ProofCase {
            name: "marker whose recorded target is gone",
            before: nothing,
            after: |husk| {
                fs::remove_dir_all(&husk.private).expect("remove the private half");
            },
            expect: Expect::Nothing(UnboundShape::TargetAbsent),
        },
    ]
}

/// A second host policy, distinguishable from `host_policy()` by its
/// canonical bytes and therefore by its digest.
fn another_policy() -> RunnerPolicy {
    let mut policy = crate::runner::policy::host_policy();
    policy.credential_volumes = Some(std::collections::BTreeMap::from([(
        "claude-code".to_owned(),
        "upstroke-creds".to_owned(),
    )]));
    policy
}

/// Conjunct 5 binds the locator to **this run's basename**, not merely to
/// the authorized `runs` directory (`PR5-RUNDIR-022`).
///
/// `scope` is "locator chain without reparse points canonicalizing to
/// `<authorized private root>/runs/<basename>`" — an equality. The grid's
/// own conjunct-5 case points the locator at a *foreign root*, which a
/// `starts_with` prefix test rejects exactly as an equality does, so the
/// conjunct was proven to reject another root and never asked the question
/// the sentence is about. These are the two shapes a prefix test admits: a
/// **sibling** run's private half, and a path **nested** inside this run's
/// own. The first is the one that matters — under it a proof for run A
/// authorizes deleting run B's private half, and
/// `tests_acceptance.seam_tests[3]` says "no census can bind another run's
/// private half to a husk".
///
/// A separate test rather than two more `proof_cases` entries: that grid
/// asserts one *distinct* `RetainReason` per case, so two more cases
/// refusing for the same reason would fail it, and the property it is
/// asserting — every conjunct separately covered — is worth keeping.
#[test]
fn a_locator_beside_or_below_this_runs_private_half_cannot_authorize_deletion() {
    type Build = fn(&mut BoundHusk) -> PathBuf;
    let cases: Vec<(&str, Build)> = vec![
        (
            "a sibling run under the authorized runs directory",
            |husk| {
                let sibling = husk
                    .private_root
                    .join("runs")
                    .join("01SIBLINGRUN0000000000000");
                fs::create_dir_all(&sibling).expect("the sibling private half");
                write(&sibling.join("evidence"), b"another run's private half");
                fs::canonicalize(&sibling).expect("canonical sibling")
            },
        ),
        ("a path nested below this run's private half", |husk| {
            let nested = husk
                .private_root
                .join("runs")
                .join(BOUND_RUN)
                .join("transcripts");
            fs::create_dir_all(&nested).expect("the nested directory");
            fs::canonicalize(&nested).expect("canonical nested")
        }),
    ];
    for (index, (name, build)) in cases.into_iter().enumerate() {
        let mut husk = BoundHusk::new(&format!("locator-prefix{index}"));
        let target = build(&mut husk);
        husk.private = target.clone();
        husk.marker.private_dir = target.to_string_lossy().into_owned();
        husk.publish();
        let before = snapshot_tree(&target);

        match husk.prove() {
            PrivateHalfOwnership::Retained(reason) => assert_eq!(
                reason.kind(),
                "locator-outside-authorized-root",
                "{name}: {reason}"
            ),
            other => panic!(
                "{name}: a locator that is not <authorized>/runs/<basename> handed out \
                     {other:?}"
            ),
        }
        assert_eq!(
            snapshot_tree(&target),
            before,
            "{name}: and the refusal touched nothing"
        );
    }
}

#[test]
fn every_conjunct_of_the_ownership_proof_refuses_on_its_own() {
    let mut kinds: Vec<(&'static str, Option<OwnerField>)> = Vec::new();
    let mut shapes: Vec<UnboundShape> = Vec::new();
    let mut proven = 0usize;

    for (index, case) in proof_cases().into_iter().enumerate() {
        let mut husk = BoundHusk::new(&format!("proof{index}"));
        (case.before)(&mut husk);
        husk.publish();
        (case.after)(&husk);
        let before_bytes = snapshot_tree(&husk.private);

        let answer = husk.prove();
        match (&case.expect, &answer) {
            (Expect::Proven, PrivateHalfOwnership::Proven(token)) => {
                assert_eq!(token.run_id(), BOUND_RUN, "{}", case.name);
                assert_eq!(
                    fs::canonicalize(token.target()).expect("canonical target"),
                    fs::canonicalize(&husk.private).expect("canonical private"),
                    "{}",
                    case.name
                );
                proven += 1;
            }
            (Expect::Nothing(expected), PrivateHalfOwnership::NothingBound(shape)) => {
                assert_eq!(shape, expected, "{}", case.name);
                shapes.push(*shape);
            }
            (Expect::Retained(kind, field), PrivateHalfOwnership::Retained(reason)) => {
                assert_eq!(&reason.kind(), kind, "{}: {reason}", case.name);
                assert_eq!(&reason.owner_field(), field, "{}: {reason}", case.name);
                kinds.push((reason.kind(), reason.owner_field()));
            }
            (expected, actual) => {
                panic!("{}: expected {expected:?}, got {actual:?}", case.name)
            }
        }

        // "each yield a RetainReason and leave the target byte-identical".
        assert_eq!(
            snapshot_tree(&husk.private),
            before_bytes,
            "{}: the proof is read-only",
            case.name
        );
    }

    assert_eq!(proven, 1, "exactly one case is the happy path");

    // The counts are what makes a dropped conjunct fail. A suite that
    // asserted only "some negative refuses" passes with any single
    // conjunct deleted; a suite that asserts every *kind* appears exactly
    // once does not.
    let mut distinct = kinds.clone();
    distinct.sort_unstable();
    distinct.dedup();
    assert_eq!(
        distinct.len(),
        kinds.len(),
        "two cases produced the same reason, so one conjunct is untested: {kinds:?}"
    );

    let mut covered: Vec<&str> = kinds.iter().map(|(kind, _)| *kind).collect();
    covered.sort_unstable();
    covered.dedup();
    // `PROOF_KINDS`, not `KINDS`: this grid measures the *proof*, and one
    // `RetainReason` is produced by the classifier before the proof is
    // consulted at all (`SWEEP-CLASSIFY-001`). Pointing it at `KINDS` would
    // require a case for a refusal `prove_private_half_ownership` cannot make.
    // `the_proof_kinds_are_the_retain_kinds_the_classifier_does_not_add` is
    // what stops the two lists drifting apart, and the census's retain arm
    // still covers `KINDS` in full.
    let mut expected: Vec<&str> = RetainReason::PROOF_KINDS.to_vec();
    expected.sort_unstable();
    assert_eq!(
        covered, expected,
        "every RetainReason variant the proof answers is a conjunct this grid must exercise"
    );

    let mut fields: Vec<OwnerField> = kinds.iter().filter_map(|(_, field)| *field).collect();
    fields.sort_unstable();
    assert_eq!(
        fields,
        OwnerField::ALL.to_vec(),
        "every field the owner record is checked on has its own case"
    );

    shapes.sort_unstable_by_key(|shape| format!("{shape:?}"));
    let mut expected_shapes = UnboundShape::ALL.to_vec();
    expected_shapes.sort_unstable_by_key(|shape| format!("{shape:?}"));
    assert_eq!(shapes, expected_shapes, "every unbound shape has a case");
}

#[test]
fn a_marker_digest_naming_another_boundary_is_the_same_refusal() {
    // The mismatch the packet calls `runner_digest_mismatch_retained` can
    // be written from either side; both are one comparison and both must
    // refuse. The grid mutates the record's policy, so this mutates the
    // marker's digest.
    let mut husk = BoundHusk::new("markerdigest");
    husk.marker.runner_policy_sha256 = runner_policy_sha256(&another_policy());
    husk.publish();
    match husk.prove() {
        PrivateHalfOwnership::Retained(reason) => {
            assert_eq!(reason.kind(), "owner-record-disagrees");
            assert_eq!(reason.owner_field(), Some(OwnerField::RunnerDigest));
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// `owner.json` **absent** and `committed.json` **present**: the fourth
/// cell of two axes the grid covers only one at a time.
///
/// The grid has both singles — "private target without an owner record"
/// answers `owner-record-missing`, "private half carrying a commit record"
/// answers `possibly-committed` — and neither is the crossing. Measured:
/// an arm answering `Proven` for exactly this cell survived the whole
/// suite, and `Proven` here is a `PrivateHalfProof`, the deletion token and
/// the only key to `remove_private_husk`, handed out for a half that may
/// have crossed P5b.
///
/// **Standalone rather than a `ProofCase` row, and it has to be.** The grid
/// asserts that no two cases produce the same `RetainReason` ("two cases
/// produced the same reason, so one conjunct is untested"), and this cell
/// answers `owner-record-missing`, which the single-axis case already
/// claims — so the crossing cannot be written as a row at all. The grid's
/// own shape is part of why the crossing was missing.
///
/// The conjunct **order** is what decides which reason comes out: the owner
/// check precedes the commit check, so this cell reports the missing record
/// rather than the possible commit. Both are safe — neither yields a token
/// — but they are different things to tell an operator, and nothing else
/// pins which one it is.
#[test]
fn probe_a_commit_record_without_an_owner_record_yields_no_token() {
    let husk = BoundHusk::new("probe-commit-no-owner");
    husk.publish();
    fs::remove_file(husk.private.join(OWNER_RECORD)).expect("remove the owner record");
    write(&husk.private.join(COMMIT_RECORD), b"{}");
    let before = snapshot_tree(&husk.private);

    match husk.prove() {
        PrivateHalfOwnership::Retained(reason) => {
            assert_eq!(reason.kind(), "owner-record-missing");
        }
        other => {
            panic!("a private half that may have crossed P5b must never be proven: {other:?}")
        }
    }
    assert_eq!(
        snapshot_tree(&husk.private),
        before,
        "the private half is byte-identical after the proof"
    );
}

/// `owner.json.tmp` present and `owner.json` absent — what an interrupted
/// P3b leaves — is not a record, and yields no token.
///
/// Neither axis alone can see the difference. `PR5-RUNDIR-045`'s fixture
/// leaves the staging file where the published record is also present, and
/// `PR5-RUNDIR-024`'s has neither file, so a proof that fell back from
/// `owner.json` to `owner.json.tmp` changed no measured answer: an
/// interrupted publication read as a completed one, and a record that was
/// never durable became proof of ownership. That fallback survived the
/// whole suite.
///
/// The fixture is built by **unpublishing** — renaming the published record
/// back to its staging name — so the half on disk is exactly the state P3a
/// leaves and P3b has not yet finished. Both halves are compared byte for
/// byte afterwards, the staging file included, so an implementation that
/// consumed or tidied it up cannot pass either.
#[test]
fn probe_an_owner_staging_file_is_not_an_owner_record() {
    let husk = BoundHusk::new("probe-owner-staged-only");
    husk.publish();
    fs::rename(
        husk.private.join(OWNER_RECORD),
        husk.private.join(OWNER_RECORD_STAGED),
    )
    .expect("unpublish the owner record");
    let before_private = snapshot_tree(&husk.private);
    let before_public = snapshot_tree(&husk.public());

    match husk.prove() {
        PrivateHalfOwnership::Retained(reason) => {
            assert_eq!(reason.kind(), "owner-record-missing");
        }
        other => panic!("an interrupted publication is not a proof of ownership: {other:?}"),
    }
    assert!(
        husk.private.join(OWNER_RECORD_STAGED).is_file(),
        "the staging file is still where the interruption left it"
    );
    assert_eq!(
        snapshot_tree(&husk.private),
        before_private,
        "the private half is byte-identical after the proof"
    );
    assert_eq!(
        snapshot_tree(&husk.public()),
        before_public,
        "and so is the public half"
    );
}

#[test]
fn the_names_on_disk_are_the_names_the_packet_writes() {
    // The funnels and the proof share the path constants, so a rename of
    // one constant would move both together and every other test in this
    // module would still pass. These are literals, written out of
    // `run_creation` and `resource_accounting`.
    let husk = BoundHusk::new("names");
    let public = husk.public();
    stage_marker(&public, &husk.marker, &mut NoHooks).expect("stage");
    assert!(public.join(".creating.tmp").is_file(), "staged marker");
    publish_marker(&public, &mut NoHooks).expect("publish");
    assert!(public.join(".creating").is_file(), "published marker");
    assert!(!public.join(".creating.tmp").exists(), "staging is spent");

    create_private_dir(&husk.private, &mut NoHooks).expect("private");
    stage_owner_record(&husk.private, &husk.owner, &mut NoHooks).expect("stage owner");
    assert!(husk.private.join("owner.json.tmp").is_file());
    publish_owner_record(&husk.private, &mut NoHooks).expect("publish owner");
    assert!(husk.private.join("owner.json").is_file());
    assert!(!husk.private.join("owner.json.tmp").exists());

    let record = CommitRecord {
        run_id: BOUND_RUN.to_owned(),
        repo_key: husk.repo_key.as_str().to_owned(),
        public_dir: husk.owner.public_dir.clone(),
        incarnation: BOUND_INCARNATION.to_owned(),
        run_started_sha256: run_started_sha256(committed_line(BOUND_RUN, 4).as_bytes()),
    };
    stage_commit_record(&husk.private, &record, &mut NoHooks).expect("stage commit");
    assert!(husk.private.join("committed.json.tmp").is_file());
    publish_commit_record(&husk.private, &mut NoHooks).expect("publish commit");
    assert!(husk.private.join("committed.json").is_file());
    assert!(!husk.private.join("committed.json.tmp").exists());

    assert_eq!(
        public.join(EVENT_LOG).file_name().expect("name"),
        "events.jsonl"
    );
    assert_eq!(
        public.join(PLAN).file_name().expect("name"),
        "plan.normalized.json"
    );
    assert_eq!(lock_file(&public).file_name().expect("name"), "run.lock");
    assert_eq!(
        worktree_lock_file(Path::new("g"))
            .file_name()
            .expect("name"),
        "upstroke-worktree.lock"
    );
    assert_eq!(
        (
            REPORT_STAGING_PREFIX,
            REPORT_STAGING_RECORD,
            REPORT_STAGING_RECORD_STAGED
        ),
        (
            ".report-staging-",
            "report-staging.json",
            "report-staging.json.tmp"
        ),
        "the report's staging directory's prefix under the public half, and its record and \
         the record's staging name in the private half"
    );
}

/// `RunPaths::events` and `RunPaths::plan_json` return the paths [`EVENT_LOG`]
/// and [`PLAN`] name (`SWEEP-NAMES-001`).
///
/// **That is the whole sentence, and it is deliberately weaker than "the
/// accessors use the constants".** Restore either accessor to its equal literal
/// and this test still passes, because the path it returns is still the path
/// the constant names. No assertion over values can do better: substituting a
/// constant for a literal of equal value is behaviour-neutral by construction,
/// and a test that could tell them apart would have to assert over source text,
/// which a sibling helper satisfies. Pass 3 was right that the previous name
/// claimed more than the body proves.
///
/// **What it does catch is an accessor drift** — an accessor spelling something
/// other than what its constant names. `RunPaths::events` changed to
/// `"events.json"` fails this test while
/// `the_names_on_disk_are_the_names_the_packet_writes` passes, so the two are
/// not redundant. A change to the *constant* does not fail it here: both sides
/// of the assertion move together.
///
/// The repair this test accompanies is witnessed by a measurement rather than
/// by an assertion. That measurement is stated once, in the pull request body's
/// Validation section, and is not restated here.
#[test]
fn the_event_log_and_plan_accessors_return_the_paths_their_constants_name() {
    // Lexical: `RunPaths` joins, it does not touch the filesystem, so this
    // needs no scratch tree and leaves none.
    let paths = paths_in(Path::new("names-through-consts"), BOUND_RUN);

    assert_eq!(paths.events(), paths.public.join(EVENT_LOG));
    assert_eq!(paths.plan_json(), paths.public.join(PLAN));
}

#[test]
fn a_committed_private_half_is_never_provable_however_bound_it_is() {
    // The commit-record condition is the last conjunct and the one whose
    // absence is invisible in the happy path: every other field agrees, so
    // a proof that had dropped it would hand out a token for a private
    // half that may have crossed P5b.
    let husk = BoundHusk::new("committedhalf");
    husk.publish();
    assert!(
        matches!(husk.prove(), PrivateHalfOwnership::Proven(_)),
        "the same husk without a commit record is provable"
    );
    write(&husk.private.join(COMMIT_RECORD), b"{}");
    match husk.prove() {
        PrivateHalfOwnership::Retained(RetainReason::PossiblyCommitted) => {}
        other => panic!("a commit record must refuse the token: {other:?}"),
    }
}

/// Conjunct 12 is fail-closed: only `NotFound` proves the record absent.
///
/// The conjunct was `fs::symlink_metadata(..).is_ok()`, so every stat error
/// that is *not* `NotFound` — `EACCES` on a directory that became
/// unreadable between the owner-record read and this stat, `EIO`, a Windows
/// sharing violation — read as "absent" and fell through to `Proven`,
/// minting the one token `remove_private_husk` accepts for a private half
/// whose `committed.json` could not be ruled out.
/// `commit_record_after_error` answers the same question the other way
/// (`Unknown`, which `permits_deletion()` refuses), so the two paths into
/// the one deletion boundary disagreed and this was the open one.
///
/// The classification is asserted directly because it is not
/// deterministically reachable through the filesystem from one thread: a
/// private directory made unreadable refuses at conjunct 6's owner-record
/// read, long before this stat. The two shapes that *are* reachable —
/// present and absent — are asserted through the whole proof by
/// `a_committed_private_half_is_never_provable_however_bound_it_is` and by
/// the wiring half below.
#[test]
fn a_commit_record_stat_that_is_not_not_found_is_not_proof_of_absence() {
    use std::io::{Error, ErrorKind};

    let husk = BoundHusk::new("conjunct12");
    husk.publish();

    // (1) The classification, over every shape the stat can produce.
    assert!(
        ownership::commit_record_proves_absence(&Err(Error::from(ErrorKind::NotFound))),
        "`NotFound` is the one answer that proves the record is not there"
    );
    for kind in [
        ErrorKind::PermissionDenied,
        ErrorKind::Other,
        ErrorKind::InvalidInput,
        ErrorKind::TimedOut,
    ] {
        assert!(
            !ownership::commit_record_proves_absence(&Err(Error::from(kind))),
            "`{kind:?}` is a stat the filesystem declined to answer, not an absence"
        );
    }
    assert!(
        !ownership::commit_record_proves_absence(&fs::symlink_metadata(&husk.private)),
        "a successful stat is a record that is present"
    );

    // (2) The wiring: the predicate is what conjunct 12 consults, so the
    // reachable shapes go through the real proof.
    assert!(
        matches!(husk.prove(), PrivateHalfOwnership::Proven(_)),
        "an absent record (a real `NotFound`) still proves"
    );
    write(&husk.private.join(COMMIT_RECORD), b"{}");
    assert!(
        matches!(
            husk.prove(),
            PrivateHalfOwnership::Retained(RetainReason::PossiblyCommitted)
        ),
        "a present record retains"
    );

    // (3) And the two paths into the boundary now agree on the third shape.
    assert!(
        !CommitRecordPresence::Unknown("io".to_owned()).permits_deletion(),
        "the creator's stat refuses an unanswerable filesystem"
    );
}

#[test]
fn the_proof_token_names_the_half_it_authorises_and_nothing_else() {
    let husk = BoundHusk::new("tokentarget");
    husk.publish();
    let PrivateHalfOwnership::Proven(token) = husk.prove() else {
        panic!("the bound husk proves");
    };
    assert_eq!(token.public_dir(), husk.public());
    assert_eq!(token.run_id(), BOUND_RUN);
    assert!(token.target().ends_with(BOUND_RUN));

    // And spending it removes exactly that half, leaving the public one.
    remove_private_husk(token, &mut NoHooks).expect("the token authorises this deletion");
    assert!(!husk.private.exists(), "the private half is gone");
    assert!(husk.public().is_dir(), "the public half is a separate step");
}

#[test]
fn the_public_husk_is_removed_with_its_marker_last() {
    // `startup_census`: "the public directory is removed with the marker
    // last … so a kill mid-census leaves a husk the next census
    // completes". A marker removed first would leave a marker-less husk
    // with content, which the next census retains rather than finishes.
    let husk = BoundHusk::new("publiclast");
    husk.publish();
    write(&lock_file(&husk.public()), b"");
    write(&husk.public().join(PLAN), b"{}");

    struct MarkerWatcher {
        public: PathBuf,
        marker_present_at_after: bool,
        others_gone_at_after: bool,
    }
    impl RunDirHooks for MarkerWatcher {
        fn hook(&mut self, site: EffectSiteId, phase: HookPhase) -> Injection {
            if site == EffectSiteId::RunDir(RunDirSite::RemovePublicHusk)
                && phase == HookPhase::After
            {
                self.marker_present_at_after = self.public.join(MARKER).exists();
                self.others_gone_at_after = !self.public.join(PLAN).exists();
            }
            Injection::Proceed
        }
    }

    // The `After` hook runs once the directory is gone, so the ordering is
    // observed by killing the removal partway instead: a kill before the
    // marker's own unlink must leave the marker there.
    let mut watcher = MarkerWatcher {
        public: husk.public(),
        marker_present_at_after: false,
        others_gone_at_after: false,
    };
    remove_public_husk(&husk.public(), &mut watcher).expect("remove");
    assert!(!husk.public().exists(), "the public half is gone");
    assert!(
        !watcher.marker_present_at_after && watcher.others_gone_at_after,
        "the whole directory is gone by the after phase"
    );
}

/// The marker really is removed **last**, observed by interrupting the
/// removal (`PR5-RUNDIR-065`).
///
/// `startup_census`: "the public directory is removed with the marker last
/// (`RunDir.RemovePublicHusk`), **so a kill mid-census leaves a husk the
/// next census completes**". The clause after the comma is the whole point
/// of the ordering, and the test above cannot see it — its `After` hook
/// runs once the directory is already gone, so both observations are the
/// same under either order. Its own comment says what would work ("a kill
/// before the marker's own unlink must leave the marker there") and it does
/// not do it.
///
/// The interruption is a **real** failed removal rather than an injection,
/// because there is no injectable coordinate inside the loop and inventing
/// one would mean a new point in a frozen enum. `zz-blocked` sorts after
/// `plan.json`, so the loop provably got partway: an earlier entry is gone
/// and a later one failed.
///
/// Unix only. The fixture needs a removal that fails, and file permissions
/// are how one is built without privilege; a process running as root would
/// defeat them, which is why the precondition is asserted rather than
/// assumed — this fails loudly there rather than passing vacuously.
#[cfg(unix)]
#[test]
fn a_public_husk_removal_that_fails_partway_leaves_the_marker_that_locates_it() {
    use std::os::unix::fs::PermissionsExt as _;

    let husk = BoundHusk::new("publiclast-interrupted");
    husk.publish();
    let public = husk.public();
    write(&public.join(PLAN), b"{}");
    let blocked = public.join("zz-blocked");
    write(
        &blocked.join("inside.txt"),
        b"content the removal cannot reach",
    );
    fs::set_permissions(&blocked, fs::Permissions::from_mode(0o500)).expect("seal it");
    assert!(
        fs::remove_dir_all(&blocked).is_err(),
        "this fixture needs a removal that fails, and here one does not — a process with              the privilege to ignore the permission bits cannot measure this"
    );
    assert!(public.join(MARKER).is_file(), "the husk has its marker");

    let error = remove_public_husk(&public, &mut NoHooks)
        .expect_err("the removal cannot finish, so it returns the failure");

    assert!(
        public.join(MARKER).is_file(),
        "the marker survived the failure and still locates this husk for the next              census: {error}"
    );
    assert!(
        !public.join(PLAN).exists(),
        "and the loop really got partway — an earlier entry was removed"
    );
    assert!(
        public.exists(),
        "the public directory itself is still there"
    );

    fs::set_permissions(&blocked, fs::Permissions::from_mode(0o700)).expect("unseal");
    // And once the obstruction is gone the same call finishes the job,
    // which is what "the next census completes" means.
    remove_public_husk(&public, &mut NoHooks).expect("the next census completes it");
    assert!(!public.exists(), "including the marker and the directory");
}

/// A public half whose only content is `.creating.tmp` is **removed**, not
/// retained.
///
/// The shape and the removal are each exercised and never composed. The
/// grid's "staged marker only" case asserts `NothingBound(StagedMarkerOnly)`
/// and stops at the classification; every fixture that reaches
/// `remove_public_husk` drives a husk carrying a **published** marker plus
/// other content. So a removal that skipped the staging file the way it
/// skips the published marker left the directory non-empty and its final
/// `remove_dir` failing, with nothing in the suite to observe it — measured
/// surviving on Linux and on the Windows guest.
///
/// `startup_census` (i) reclaims "a bare directory or one holding only a
/// staged `.creating.tmp`", and the census reaching it is the whole
/// obligation: retained-as-markerless-content is the outcome this shape
/// must never get.
///
/// The retry half **reconstructs** the state an interrupted first pass
/// leaves — the other content gone, the staging file and the directory
/// still there — rather than interrupting a real one. Building a genuine
/// mid-loop failure needs a removal that fails, which is a permission
/// fixture and therefore Unix-only
/// (`a_public_husk_removal_that_fails_partway_leaves_the_marker_that_locates_it`
/// is exactly that and is `#[cfg(unix)]`). What convergence needs is that
/// the *state* is reached and finished, and this reaches it on both
/// platforms.
#[test]
fn probe_a_staged_marker_only_public_husk_is_removed() {
    let root = scratch("probe-stagedonly");

    let public = root.join("runs").join("01STAGEDONLY");
    fs::create_dir_all(&public).expect("public directory");
    write(&public.join(MARKER_STAGED), b"{}");
    remove_public_husk(&public, &mut NoHooks).expect("the census removes a staged-marker husk");
    assert!(
        !public.exists(),
        "the public directory itself is gone, staging file and all"
    );

    // And it converges across an interrupted first pass.
    let retried = root.join("runs").join("01RETRY");
    fs::create_dir_all(&retried).expect("public directory");
    write(&retried.join(MARKER_STAGED), b"{}");
    write(&retried.join(PLAN), b"{}");
    fs::remove_file(retried.join(PLAN)).expect("the interrupted pass got this far");
    remove_public_husk(&retried, &mut NoHooks).expect("the retry converges");
    assert!(!retried.exists(), "the next census finishes the job");
}

/// P0 creates the **public** run directory and nothing else
/// (`PR5-RUNDIR-036`).
///
/// `run_creation` orders "P0 create the public run directory
/// (`RunDir.CreatePublicDir`)" before "P3 create the private half at the
/// recorded locator", and the private half exists so that no agent-authored
/// byte is reachable from the workspace. Implementing P0 by calling the
/// legacy `RunPaths::create()` — which builds both halves and both
/// skeletons — satisfied every site-coverage assertion in this file,
/// because none of them ever looked at what was on disk at a phase.
#[test]
fn p0_creates_the_public_directory_and_nothing_private() {
    let root = scratch("p0-only");
    let paths = paths_in(&root, "01P0ONLY");
    let public = paths.public.clone();
    let private = paths.private.clone();

    create_public_dir(&public, &mut NoHooks).expect("P0");

    assert!(public.is_dir(), "P0 created the public run directory");
    assert_eq!(
        read_dir_names(&public),
        Vec::<String>::new(),
        "and it is bare: no skeleton, no marker, no private half beneath it"
    );
    assert!(
        !private.exists(),
        "the private half is P3's, at the recorded locator, and does not exist yet"
    );
}

/// The owner record is the **first content** of a private half
/// (`PR5-RUNDIR-044`).
///
/// `side_effect_vs_event_ordering` says exactly that, and until now it was
/// asserted by nothing: no test read the private half's directory listing
/// at any point in the publication sequence, so moving the five skeleton
/// directories into `create_private_dir`'s own funnel body — where they
/// exist before `owner.json` is even staged — changed nothing observable.
#[test]
fn the_owner_record_is_the_first_content_of_a_private_half() {
    let root = scratch("owner-first");
    let private = root.join("private").join("runs").join("01OWNERFIRST");
    let owner = OwnerRecord {
        run_id: "01OWNERFIRST".to_owned(),
        repo_key: "0123456789abcdef".to_owned(),
        public_dir: root.join("public").to_string_lossy().into_owned(),
        incarnation: "01INC".to_owned(),
        runner: crate::runner::policy::host_policy(),
    };

    create_private_dir(&private, &mut NoHooks).expect("P3");
    assert_eq!(
        read_dir_names(&private),
        Vec::<String>::new(),
        "immediately after P3 the private half is empty"
    );

    stage_owner_record(&private, &owner, &mut NoHooks).expect("P3a");
    assert_eq!(
        read_dir_names(&private),
        vec![OWNER_RECORD_STAGED.to_owned()],
        "the staged owner record is the only thing in it"
    );

    publish_owner_record(&private, &mut NoHooks).expect("P3b");
    assert_eq!(
        read_dir_names(&private),
        vec![OWNER_RECORD.to_owned()],
        "and after publication the owner record is the only content there has ever been"
    );
}

// =======================================================================
// The funnel
// =======================================================================

/// Records what the funnels reached, and answers with whatever was armed.
#[derive(Debug, Default)]
struct Observer {
    reached: Vec<(String, HookPhase)>,
    armed: Vec<(EffectSiteId, HookPhase, Injection)>,
}

impl Observer {
    fn arm(&mut self, site: EffectSiteId, phase: HookPhase, injection: Injection) {
        self.armed.push((site, phase, injection));
    }

    fn sites(&self) -> Vec<String> {
        let mut sites: Vec<String> = self.reached.iter().map(|(site, _)| site.clone()).collect();
        sites.sort_unstable();
        sites.dedup();
        sites
    }

    fn phases_of(&self, site: EffectSiteId) -> Vec<HookPhase> {
        let name = site.to_string();
        let mut phases: Vec<HookPhase> = self
            .reached
            .iter()
            .filter(|(seen, _)| *seen == name)
            .map(|(_, phase)| *phase)
            .collect();
        phases.dedup();
        phases
    }
}

impl RunDirHooks for Observer {
    fn hook(&mut self, site: EffectSiteId, phase: HookPhase) -> Injection {
        self.reached.push((site.to_string(), phase));
        self.armed
            .iter()
            .find(|(armed, at, _)| *armed == site && *at == phase)
            .map_or(Injection::Proceed, |(_, _, injection)| *injection)
    }
}

/// Every site of the three groups this module funnels, from the frozen
/// inventory's own `ALL` slices.
fn sites_this_module_owns() -> Vec<String> {
    let mut names: Vec<String> = RunDirSite::ALL
        .iter()
        .map(|site| EffectSiteId::RunDir(*site).to_string())
        .chain(
            AnswerSite::ALL
                .iter()
                .map(|site| EffectSiteId::Answer(*site).to_string()),
        )
        .chain(
            LockSite::ALL
                .iter()
                .map(|site| EffectSiteId::Lock(*site).to_string()),
        )
        .chain(
            crate::topology::effects::ReportSite::ALL
                .iter()
                .map(|site| EffectSiteId::Report(*site).to_string()),
        )
        .collect();
    names.sort_unstable();
    names
}

fn commit_record_of(husk: &BoundHusk) -> CommitRecord {
    CommitRecord {
        run_id: BOUND_RUN.to_owned(),
        repo_key: husk.repo_key.as_str().to_owned(),
        public_dir: husk.owner.public_dir.clone(),
        incarnation: BOUND_INCARNATION.to_owned(),
        run_started_sha256: run_started_sha256(committed_line(BOUND_RUN, 4).as_bytes()),
    }
}

/// Every atomic publication's **durability sequence**, read out of the
/// funnel's own ledger (`PR5-RUNDIR-057`).
///
/// `run_creation` spells each of the three the same way — "write
/// `<name>.tmp`, **fsync**, rename, **fsync the directory**" — and until
/// this lane had a ledger, two of those four steps were not observables at
/// all. Deleting `stage_json`'s `file.sync_all()`, which is the staging
/// half of the marker, the owner record *and* the commit record, left the
/// entire suite green: every consumer checks the *outcome* of a publication
/// (the staged name is gone, the published name holds the right JSON, the
/// census parses it) and an unsynced file is byte-for-byte a synced one on
/// a machine that does not lose power.
///
/// The ledger's length is the filesystem's own answer rather than a number
/// the funnel carried along, so a sync that reported a length while the
/// file held something else would fail here rather than agree with itself.
#[test]
fn every_atomic_publication_syncs_the_staged_file_then_renames_then_syncs_its_directory() {
    let root = scratch("durability");
    let public = root.join("public");
    let private = root.join("private");
    create_dir(&public).expect("public");
    create_dir(&private).expect("private");
    let policy = crate::runner::policy::host_policy();
    let marker = CreatingMarker {
        run_id: "01LEDGER".to_owned(),
        repo_key: "0123456789abcdef".to_owned(),
        private_dir: private.to_string_lossy().into_owned(),
        incarnation: "01INC".to_owned(),
        pid: std::process::id(),
        runner_policy_sha256: runner_policy_sha256(&policy),
    };
    let owner = OwnerRecord {
        run_id: "01LEDGER".to_owned(),
        repo_key: "0123456789abcdef".to_owned(),
        public_dir: public.to_string_lossy().into_owned(),
        incarnation: "01INC".to_owned(),
        runner: policy,
    };
    let commit = CommitRecord {
        run_id: "01LEDGER".to_owned(),
        repo_key: "0123456789abcdef".to_owned(),
        public_dir: public.to_string_lossy().into_owned(),
        incarnation: "01INC".to_owned(),
        run_started_sha256: run_started_sha256(b"{}\n"),
    };

    let mut hooks = HarnessHooks::default().recording_durability();
    let ledger = hooks.ledger();
    // The ledger is written *beside* the syscall by the same function, so on
    // its own it certifies itself (`PR5-CONF-012`): with `sync_all` replaced
    // by `Ok(())`, every assertion below still passed. `barriers_performed`
    // counts entries into `util::fsync_file`/`fsync_dir`, so the ledger's
    // claim can be checked against something that is not the ledger.
    //
    // The two axes are the *record* and the *call*. Every assertion below
    // holds the call constant — it is assumed to have happened — and reads
    // the record; this reads the call and holds the record constant. The
    // counter is process-wide and the suite is threaded, so the assertion is
    // a **lower bound on the delta**, which is the strongest thing a shared
    // counter can support and is still zero if the barrier is never entered.
    let barriers_before = util::barriers_performed();
    let publications: Vec<(&str, PathBuf, &str, &str)> = vec![
        ("marker", public.clone(), MARKER_STAGED, MARKER),
        (
            "owner record",
            private.clone(),
            OWNER_RECORD_STAGED,
            OWNER_RECORD,
        ),
        (
            "commit record",
            private.clone(),
            COMMIT_RECORD_STAGED,
            COMMIT_RECORD,
        ),
        // The report joined the atomic publications in PR10's round 3 and
        // this sequence in round 4 (the crash lens, P1): the two witnesses
        // that read the barriers across `write_report` count them and check
        // the staged name is gone, which a writer that never stages —
        // `report.json` created, synced and its directory synced, no rename —
        // satisfies. The staged-path and step-sequence assertions below do
        // not.
        ("report", public.clone(), "", REPORT),
    ];
    for (which, dir, staged_name, published_name) in publications {
        ledger.clear();
        match which {
            "marker" => {
                stage_marker(&public, &marker, &mut hooks).expect("P1a");
                publish_marker(&public, &mut hooks).expect("P1b");
            }
            "owner record" => {
                stage_owner_record(&private, &owner, &mut hooks).expect("P3a");
                publish_owner_record(&private, &mut hooks).expect("P3b");
            }
            "commit record" => {
                stage_commit_record(&private, &commit, &mut hooks).expect("P5a");
                publish_commit_record(&private, &mut hooks).expect("P5b");
            }
            _ => {
                let report = serde_json::json!({"run_id": "01LEDGER", "outcome": "complete"});
                write_report(&public, &private, &report, &mut hooks).expect("the report");
            }
        }

        let records = ledger.records();
        // The report is staged inside a directory the write makes for itself
        // under a name unique to the write, recorded in the private half
        // before the directory exists (PR10's round 10; a fixed directory in
        // round 9, a name unique to the write in round 8): the staged path
        // is read from the ledger's own record of the directory's creation,
        // which carries the record observed standing in the statement before
        // `create_dir`; the three run-creation records keep their fixed
        // names.
        let staged_path = if which == "report" {
            let made = records
                .iter()
                .find(|record| record.step == DurableStep::DirectoryCreated)
                .expect("the staging directory's creation is recorded");
            assert!(
                made.path.parent() == Some(dir.as_path())
                    && made
                        .path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.starts_with(REPORT_STAGING_PREFIX)),
                "{which}: the staging directory is made under the report's own directory, under \
                 the write's own name: {}",
                made.path.display()
            );
            assert_eq!(
                made.entry
                    .as_ref()
                    .map(|entry| (entry.path.clone(), entry.present)),
                Some((report_staging_record(&private), true)),
                "{which}: and its record in the private half stood at the instant before \
                 `create_dir`"
            );
            made.path.join(REPORT)
        } else {
            dir.join(staged_name)
        };
        // One expectation for every platform (`PR5-CONF-013`). This used to
        // fork on `cfg!(unix)` because `sync_dir` was a documented no-op on
        // Windows; `run_creation`'s "fsync the directory" carries no
        // platform exception, and now neither does this. The `Staged` entry
        // is the staged file's creation, taken before its first byte (PR10's
        // round 5): the mode it carries is what the mode test below reads.
        let publication: Vec<DurableStep> = vec![
            DurableStep::Staged,
            DurableStep::SyncedFile,
            DurableStep::Renamed,
            DurableStep::SyncedDirectory,
        ];
        // The report's sequence is the record's publication — the same four
        // steps, in the private half — then the directory's creation, then
        // its own publication; the record's steps are held to the private
        // half here and the report's own are read below as the other three
        // records' are.
        let (expected, records): (Vec<DurableStep>, Vec<util::DurableRecord>) = if which == "report"
        {
            let mut expected = publication.clone();
            expected.push(DurableStep::DirectoryCreated);
            expected.extend(publication.iter().copied());
            let record = report_staging_record(&private);
            assert_eq!(
                (
                    records[0].path.as_path(),
                    records[1].path.as_path(),
                    records[2].path.as_path(),
                    records[3].path.as_path()
                ),
                (
                    private.join(REPORT_STAGING_RECORD_STAGED).as_path(),
                    private.join(REPORT_STAGING_RECORD_STAGED).as_path(),
                    record.as_path(),
                    private.as_path()
                ),
                "{which}: the record is staged, synced, renamed and its directory synced — in \
                 the private half, before the staging directory exists"
            );
            (expected, records[5..].to_vec())
        } else {
            (publication, records)
        };
        assert_eq!(
            ledger.steps(),
            expected,
            "{which}: the durability sequence run_creation names, in order"
        );
        assert_eq!(
            (records[0].path.as_path(), records[0].len),
            (staged_path.as_path(), 0),
            "{which}: the staged file is recorded at its creation, empty"
        );
        assert_eq!(
            records[1].path, staged_path,
            "{which}: the sync is of the STAGED file, before it has its published name"
        );
        let published_len = fs::metadata(dir.join(published_name))
            .expect("the published record")
            .len();
        assert!(published_len > 0, "{which}: the record has bytes at all");
        assert_eq!(
            records[1].len, published_len,
            "{which}: the whole staged file was synced, not a prefix of it"
        );
        assert_eq!(
            records[2].path,
            dir.join(published_name),
            "{which}: the rename lands on the published name"
        );
        assert_eq!(
            records[3].path, dir,
            "{which}: the directory sync is of the directory the rename changed"
        );
    }

    // Four publications, each recording one file sync and one directory
    // sync (and one creation, which is no barrier), and the report's staging
    // record published the same way before it: ten ledger entries that each
    // claim a barrier was performed.
    let claimed = 10;
    let performed = util::barriers_performed().saturating_sub(barriers_before);
    assert!(
        performed >= claimed,
        "the ledger recorded {claimed} durability barriers and only {performed} \
             were entered; a ledger that certifies the function it is written by \
             cannot tell the two apart (PR5-CONF-012)"
    );
}

/// `let _ = stage_json(...)` in `write_report` — the staged file's sync
/// failing and the failure discarded — survived every test until PR10's
/// round 4 (the fix-check lens): nothing made a sync fail. This does: the
/// file half of the barrier is refused for every file within the public run
/// directory (`util::fail_file_barriers_within`) — the staged report's alone,
/// inside the directory this write makes for itself under a name the test
/// cannot know in advance; the staging record's own barrier is in the
/// private half and holds — the publication stops there, no rename, no
/// report under its name, and says so; what it leaves is exactly a dead
/// writer's shape, the record, the directory it names and the staged file
/// inside, and with the barrier holding again the same call reclaims them
/// by the record and publishes.
#[test]
fn a_report_whose_staged_file_will_not_sync_is_not_published() {
    let root = scratch("report-stage-sync-fault");
    let public = root.join("public");
    create_dir(&public).expect("public");
    let private = root.join("private");
    create_dir(&private).expect("private");
    let payload = serde_json::json!({"run_id": "01FAULT", "outcome": "complete"});
    {
        let _fault = util::fail_file_barriers_within(&public);
        let error = write_report(&public, &private, &payload, &mut NoHooks)
            .expect_err("a staged report whose barrier fails is not published");
        assert!(
            error.to_string().contains("injected barrier fault"),
            "the failure is the barrier's, by name: {error}"
        );
        assert!(
            !public.join(REPORT).exists(),
            "no report under its name: the rename never ran"
        );
        let left = report_staging_leftovers(&public, &private).expect("listed");
        let recorded = recorded_report_staging(&public, &private)
            .expect("the record reads")
            .expect("the record stands: it was published before the directory was made");
        assert_eq!(
            left,
            vec![
                report_staging_record(&private),
                recorded.clone(),
                recorded.join(REPORT)
            ],
            "what the failed write leaves is a dead writer's shape: the record, the directory it \
             names, the staged file inside"
        );
    }
    write_report(&public, &private, &payload, &mut NoHooks)
        .expect("with the barrier holding, published");
    assert!(
        public.join(REPORT).is_file()
            && report_staging_leftovers(&public, &private)
                .expect("listed")
                .is_empty()
            && unrecorded_report_staging(&public, &private)
                .expect("listed")
                .is_empty(),
        "the leftover reclaimed by its record and this write's own directory and record gone \
         with the rename"
    );
}

/// The staged report carries the existing report's mode **from its
/// creation**, not from a `chmod` after its bytes are written (the round-5
/// regression lens, P2): a staged report created at the umask's mode and
/// narrowed afterwards is readable by whoever opens it in between, and a
/// death in between leaves it so. The ledger's `Staged` entry is taken
/// before the first byte and carries the mode the file had then, so the
/// window is an observable rather than a reading of the source order. Two
/// modes: the lens's `0600`, which this box's umask of `077` gives every
/// fresh file anyway, so on its own it is a witness under `umask 022` only;
/// and `0400`, narrower than any umask's default, which is one under either.
#[cfg(unix)]
#[test]
fn a_private_report_is_staged_at_its_mode_before_any_byte_is_written() {
    use std::os::unix::fs::PermissionsExt as _;
    let root = scratch("report-staged-mode");
    let public = root.join("public");
    create_dir(&public).expect("public");
    let private = root.join("private");
    create_dir(&private).expect("private");
    let payload = serde_json::json!({"run_id": "01MODE", "outcome": "parked"});
    let path = public.join(REPORT);
    write_report(&public, &private, &payload, &mut NoHooks).expect("the first report");
    for mode in [0o600, 0o400] {
        fs::set_permissions(&path, fs::Permissions::from_mode(mode)).expect("make it private");
        let mut hooks = HarnessHooks::default().recording_durability();
        let ledger = hooks.ledger();
        write_report(&public, &private, &payload, &mut hooks).expect("the report, written again");
        let staged: Vec<_> = ledger
            .records()
            .into_iter()
            .filter(|record| {
                record.step == DurableStep::Staged && is_staged_report(&record.path, &public)
            })
            .collect();
        assert_eq!(
            staged.len(),
            1,
            "{mode:o}: the staged file was recorded once, at its creation"
        );
        assert_eq!(
            (staged[0].mode, staged[0].len),
            (Some(mode), 0),
            "{mode:o}: at the existing report's mode before any byte reached it — a file \
             created at the umask's mode and narrowed after the write is readable in between"
        );
        assert_eq!(
            fs::metadata(&path)
                .expect("the published report")
                .permissions()
                .mode()
                & 0o777,
            mode,
            "{mode:o}: and published at that mode"
        );
        assert!(
            report_staging_leftovers(&public, &private)
                .expect("listed")
                .is_empty(),
            "{mode:o}: the staged file was renamed onto its name"
        );
    }
}

#[test]
fn every_site_this_module_owns_is_reached_through_a_funnel_in_both_phases() {
    // Enumerated from `RunDirSite::ALL`, `AnswerSite::ALL` and
    // `LockSite::ALL` rather than from a list of what this file happens to
    // call, so a site the frozen inventory declares and no funnel names
    // fails here rather than being quietly absent from `effect_sites.json`.
    let husk = BoundHusk::new("sitecoverage");
    let mut hooks = Observer::default();
    let public = husk.public();

    create_public_dir(&public, &mut hooks).expect("P0");
    stage_marker(&public, &husk.marker, &mut hooks).expect("P1a");
    publish_marker(&public, &mut hooks).expect("P1b");
    create_private_dir(&husk.private, &mut hooks).expect("P3");
    stage_owner_record(&husk.private, &husk.owner, &mut hooks).expect("P3a");
    publish_owner_record(&husk.private, &mut hooks).expect("P3b");
    write_plan(&public, b"{\"tasks\":[]}", &mut hooks).expect("P5");
    let barriers_before = util::barriers_on_this_thread();
    write_report(
        &public,
        &husk.private,
        &serde_json::json!({"outcome": "parked"}),
        &mut hooks,
    )
    .expect("report");
    let barriers_after = util::barriers_on_this_thread();
    // Two publications, not one: the staging record's (its file synced, the
    // private directory synced) and then the report's own (the staged file
    // synced, the run directory synced). A report written in place with no
    // barrier of its own leaves the record's one of each, which is why the
    // count is held to two — under a single barrier the record's publication
    // alone satisfied this assertion and `report-write-unsynced` survived
    // (PR10's round 10).
    assert!(
        barriers_after.file >= barriers_before.file + 2
            && barriers_after.directory >= barriers_before.directory + 2,
        "the report write is a durable publication twice over — the staging record's file synced \
         and its private directory synced, then the report's staged file synced and the run \
         directory synced — and the barriers this thread entered say so, two of each at least: \
         {barriers_before:?} -> {barriers_after:?}"
    );
    assert!(
        report_staging_leftovers(&public, &husk.private)
            .expect("listed")
            .is_empty()
            && public.join(REPORT).is_file(),
        "the staged report was renamed onto its name"
    );
    let questions = public.join("questions");
    fs::create_dir_all(&questions).expect("questions");
    write_question_payload(&questions, "q-1", &serde_json::json!({}), &mut hooks)
        .expect("question payload");
    let answers = public.join("answers");
    fs::create_dir_all(&answers).expect("answers");
    stage_answer(&answers, "q-1", &serde_json::json!({}), &mut hooks).expect("stage answer");
    publish_answer(&answers, "q-1", &mut hooks).expect("publish answer");
    ingest_answer(&answers, "q-1", &mut hooks).expect("ingest answer");

    // The commit record goes to a private half of its own, so publishing
    // it does not make the husk below unprovable.
    let committed_half = husk
        .root
        .join("private")
        .join("runs")
        .join("01COMMITTEDHALF");
    create_private_dir(&committed_half, &mut hooks).expect("second private half");
    stage_commit_record(&committed_half, &commit_record_of(&husk), &mut hooks).expect("P5a");
    publish_commit_record(&committed_half, &mut hooks).expect("P5b");

    let git_dir = husk.root.join("git-dir");
    fs::create_dir_all(&git_dir).expect("git dir");
    let lease = WorktreeLock::acquire_in_hooked(&husk.repo, &git_dir, &mut hooks)
        .expect("the worktree lease");
    let run_lock = RunLock::acquire_hooked(&public, &mut hooks).expect("the run lock");
    run_lock.release(&mut hooks);
    drop(lease);

    remove_marker(&public, &mut hooks).expect("P7");
    // The marker is gone, so re-publish it for the proof, then spend the
    // token on the half it names.
    stage_marker(&public, &husk.marker, &mut hooks).expect("re-stage");
    publish_marker(&public, &mut hooks).expect("re-publish");
    let PrivateHalfOwnership::Proven(token) = husk.prove() else {
        panic!("the bound husk proves");
    };
    remove_private_husk(token, &mut hooks).expect("private half");
    remove_public_husk(&public, &mut hooks).expect("public half");

    assert_eq!(
        hooks.sites(),
        sites_this_module_owns(),
        "every declared site, and no site this module does not own"
    );
    for name in sites_this_module_owns() {
        let site: EffectSiteId = name.clone().try_into().expect("a declared site");
        assert_eq!(
            hooks.phases_of(site).first(),
            Some(&HookPhase::Before),
            "`{name}` must hook Before its primitive"
        );
        assert!(
            hooks.phases_of(site).contains(&HookPhase::After),
            "`{name}` must hook After it"
        );
    }
}

#[test]
fn the_post_error_stat_helper_stats_rather_than_reading_the_error() {
    // The two cases `run_creation` separates — "a P5b error after which
    // the record is absent" and "a P5b error after which the record is
    // present" — return the *same* error value, because the error-return
    // mode returns `Err` after performing the primitive. A helper that
    // inferred absence from an error would delete a private half that had
    // already crossed the deletion boundary.
    let husk = BoundHusk::new("posterror");
    husk.publish();
    let record = commit_record_of(&husk);
    let site = EffectSiteId::RunDir(RunDirSite::PublishCommitRecord);

    // (1) the rename happened, then the funnel returned Err.
    stage_commit_record(&husk.private, &record, &mut NoHooks).expect("stage");
    let mut after = Observer::default();
    after.arm(site, HookPhase::After, Injection::Error);
    let error = publish_commit_record(&husk.private, &mut after).expect_err("injected");
    assert!(
        error.to_string().contains("RunDir.PublishCommitRecord"),
        "the error names the point reached: {error}"
    );
    assert!(
        husk.private.join(COMMIT_RECORD).is_file(),
        "the record is there"
    );
    assert_eq!(
        commit_record_after_error(&husk.private),
        CommitRecordPresence::Present
    );
    assert!(
        !commit_record_after_error(&husk.private).permits_deletion(),
        "from the moment committed.json exists the creator deletes nothing"
    );
    // And the census agrees with the creator about the same bytes.
    assert!(matches!(
        husk.prove(),
        PrivateHalfOwnership::Retained(RetainReason::PossiblyCommitted)
    ));

    // (2) the same error, returned before the rename.
    fs::remove_file(husk.private.join(COMMIT_RECORD)).expect("reset");
    stage_commit_record(&husk.private, &record, &mut NoHooks).expect("stage again");
    let mut before = Observer::default();
    before.arm(site, HookPhase::Before, Injection::Error);
    publish_commit_record(&husk.private, &mut before).expect_err("injected");
    assert_eq!(
        commit_record_after_error(&husk.private),
        CommitRecordPresence::Absent
    );
    assert!(
        commit_record_after_error(&husk.private).permits_deletion(),
        "the creator knows the run never committed and may remove both halves"
    );
    assert!(
        husk.private.join(COMMIT_RECORD_STAGED).is_file(),
        "committed.json.tmp leaves with the private half"
    );
    assert!(
        matches!(husk.prove(), PrivateHalfOwnership::Proven(_)),
        "a staged-only commit record is not a commit record"
    );

    // (3) an unreadable answer is not "absent".
    assert!(!CommitRecordPresence::Unknown("io".to_owned()).permits_deletion());
}

/// The child of [`a_kill_between_stage_and_rename_leaves_only_the_tmp`]:
/// stages one record and dies at the publication site's `Before` phase.
#[test]
#[ignore = "spawned as a subprocess by a_kill_between_stage_and_rename_leaves_only_the_tmp"]
fn publication_kill_child() {
    let dir = PathBuf::from(std::env::var("UPSTROKE_TEST_KILL_DIR").expect("dir"));
    let which = std::env::var("UPSTROKE_TEST_KILL_SITE").expect("site");
    fs::create_dir_all(&dir).expect("dir");
    let policy = crate::runner::policy::host_policy();
    let mut hooks = Observer::default();
    let (site, publish): (RunDirSite, fn(&Path, &mut dyn RunDirHooks) -> _) = match which.as_str() {
        "marker" => {
            let marker = CreatingMarker {
                run_id: "01KILL".to_owned(),
                repo_key: "0123456789abcdef".to_owned(),
                private_dir: dir.to_string_lossy().into_owned(),
                incarnation: "01INC".to_owned(),
                pid: std::process::id(),
                runner_policy_sha256: runner_policy_sha256(&policy),
            };
            stage_marker(&dir, &marker, &mut hooks).expect("stage marker");
            (RunDirSite::PublishMarker, publish_marker)
        }
        "owner" => {
            let owner = OwnerRecord {
                run_id: "01KILL".to_owned(),
                repo_key: "0123456789abcdef".to_owned(),
                public_dir: dir.to_string_lossy().into_owned(),
                incarnation: "01INC".to_owned(),
                runner: policy,
            };
            stage_owner_record(&dir, &owner, &mut hooks).expect("stage owner");
            (RunDirSite::PublishOwnerRecord, publish_owner_record)
        }
        "commit" => {
            let record = CommitRecord {
                run_id: "01KILL".to_owned(),
                repo_key: "0123456789abcdef".to_owned(),
                public_dir: dir.to_string_lossy().into_owned(),
                incarnation: "01INC".to_owned(),
                run_started_sha256: "sha256:00".to_owned(),
            };
            stage_commit_record(&dir, &record, &mut hooks).expect("stage commit");
            (RunDirSite::StageCommitRecord, publish_commit_record)
        }
        other => panic!("unknown site `{other}`"),
    };
    let site = match which.as_str() {
        "commit" => RunDirSite::PublishCommitRecord,
        _ => site,
    };
    hooks.arm(
        EffectSiteId::RunDir(site),
        HookPhase::Before,
        Injection::Kill,
    );
    let _ = publish(&dir, &mut hooks);
    unreachable!("the kill must have taken this process");
}

#[test]
fn a_kill_between_stage_and_rename_leaves_only_the_tmp() {
    // A real process death, not an early return: the claim is what a
    // coordinator that runs *no* cleanup leaves on disk, and the funnel's
    // kill aborts rather than unwinding for exactly that reason.
    let root = scratch("killpublish");
    for (which, staged, published) in [
        ("marker", MARKER_STAGED, MARKER),
        ("owner", OWNER_RECORD_STAGED, OWNER_RECORD),
        ("commit", COMMIT_RECORD_STAGED, COMMIT_RECORD),
    ] {
        let dir = root.join(which);
        let status = std::process::Command::new(std::env::current_exe().expect("test binary"))
            .args([
                "--exact",
                "rundir::tests::publication_kill_child",
                "--ignored",
                "--nocapture",
            ])
            .env("UPSTROKE_TEST_KILL_DIR", &dir)
            .env("UPSTROKE_TEST_KILL_SITE", which)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .expect("spawn the publishing child");
        assert!(!status.success(), "`{which}`: the child must have died");
        assert!(
            dir.join(staged).is_file(),
            "`{which}`: the staged file survives the kill"
        );
        assert!(
            !dir.join(published).exists(),
            "`{which}`: nothing was published"
        );
    }
}

/// Publication re-points the *name*; it never writes through it.
///
/// The kill test above cannot see this. It kills at `Before`, where neither
/// a rename nor a copy has done anything yet, so it stays green against a
/// `publish` rewritten as copy-then-delete — measured, and the reason this
/// test exists. Copy-then-delete is not atomic: it truncates the
/// destination and then fills it, so a death inside it leaves a *partial*
/// published record where `T-RUNSTART` requires either the old one or the
/// new one. `RunDirSite::sub_effects()` is empty for every site in the
/// frozen inventory, so there is no coordinate to place a fault at inside
/// the primitive, and the discriminator has to be an observable the two
/// implementations differ on *after* a successful publication.
///
/// A hard link is that observable, on both platforms. Point a second name
/// at the destination before publishing: `fs::rename` replaces the
/// directory entry and leaves the linked file's bytes alone, while
/// `fs::copy` opens that same file through the link and overwrites it. So
/// the sentinel's bytes answer "rename or copy?" directly, with no reliance
/// on `st_ino` — which Windows does not expose on stable Rust
/// (`MetadataExt::file_index` is behind `windows_by_handle`).
#[test]
fn publication_replaces_the_name_rather_than_writing_through_it() {
    let root = scratch("publishrename");
    for (which, staged_name, published_name) in [
        ("marker", MARKER_STAGED, MARKER),
        ("owner", OWNER_RECORD_STAGED, OWNER_RECORD),
        ("commit", COMMIT_RECORD_STAGED, COMMIT_RECORD),
    ] {
        let dir = root.join(which);
        fs::create_dir_all(&dir).expect("dir");

        // The bytes that must survive: an unrelated file that happens to
        // share an inode with the publication's destination.
        let sentinel = dir.join("sentinel");
        let sentinel_bytes = b"the linked file is not the publication's business";
        fs::write(&sentinel, sentinel_bytes).expect("sentinel");
        fs::hard_link(&sentinel, dir.join(published_name)).expect("hard link");

        let staged_bytes = b"{\"published\":true}";
        fs::write(dir.join(staged_name), staged_bytes).expect("staged");
        publish(
            &dir.join(staged_name),
            &dir.join(published_name),
            &DurabilityLedger::off(),
        )
        .expect("publish");

        assert_eq!(
            fs::read(dir.join(published_name)).expect("published"),
            staged_bytes,
            "`{which}`: the published name carries the staged bytes"
        );
        assert!(
            !dir.join(staged_name).exists(),
            "`{which}`: the staged name is gone"
        );
        assert_eq!(
            fs::read(&sentinel).expect("sentinel after"),
            sentinel_bytes,
            "`{which}`: publication wrote *through* the destination name \
                 instead of replacing it, so it is a copy rather than a rename \
                 and a death inside it can leave a partial record"
        );
    }
}

// =======================================================================
// R28: a surviving reaper's shared cleanup hold
// =======================================================================

/// A reaper that outlives its coordinator: takes the shared cleanup hold
/// and keeps it until it is killed.
#[cfg(unix)]
#[test]
#[ignore = "spawned as a subprocess by a_surviving_reaper_hold_refuses_the_next_coordinator_until_released"]
fn cleanup_hold_child() {
    use std::os::fd::AsRawFd as _;
    let public = PathBuf::from(std::env::var("UPSTROKE_TEST_CLEANUP_DIR").expect("run dir"));
    let path = cleanup_lock_file(&public);
    let file = File::options()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&path)
        .expect("open the cleanup lock");
    // SHARED, which is what R28 is: "a surviving Unix cleanup reaper's
    // **shared** cleanup.lock hold (one per reaper)".
    assert_eq!(
        unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_SH | libc::LOCK_NB) },
        0,
        "the reaper takes its shared hold"
    );
    println!("held");
    std::io::Write::flush(&mut std::io::stdout()).expect("flush");
    std::thread::sleep(Duration::from_secs(30));
}

#[cfg(unix)]
#[test]
fn a_surviving_reaper_hold_refuses_the_next_coordinator_until_released() {
    // `PR4-R28-NEXT-COORDINATOR-UNWITNESSED`: two withheld mutations
    // survived the whole suite because no test started a coordinator while
    // a surviving reaper actually held R28. `PR4-WIN-073` turns the
    // would-block branch into continuation; `PR4-WIN-074` replaces the
    // immediate refusal with a loop that waits for the hold and then
    // continues. Both are killed here, and by different assertions.
    //
    // The run is a **husk** on purpose. The run whose reaper is still
    // settling groups is the one that died before its log committed, and
    // `list_runs` no longer returns it — so a lease that scanned the
    // readers' view would leave exactly this hold unobserved.
    let root = scratch("r28witness");
    let repo = root.join("repo");
    let git_dir = root.join("git-dir");
    fs::create_dir_all(&git_dir).expect("git dir");
    let husk_id = "01REAPERHUSK00000000000000";
    let husk = public_dir(&repo, husk_id);
    fs::create_dir_all(&husk).expect("husk");
    assert_eq!(classify_run_dir(&husk), RunDirClass::Husk);
    assert!(
        list_runs(&repo).is_empty(),
        "the reader does not return it, which is the point"
    );

    // Adopted, so the child is terminated, reaped and its reader joined
    // when this scope ends however it ends -- including a panicking
    // assertion between here and the teardown below.
    let mut producer = readiness::Producer::adopt(
        std::process::Command::new(std::env::current_exe().expect("test binary"))
            .args([
                "--exact",
                "rundir::tests::cleanup_hold_child",
                "--ignored",
                "--nocapture",
            ])
            .env("UPSTROKE_TEST_CLEANUP_DIR", &husk)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("spawn the surviving reaper"),
    );
    // Producer-aware and effectively bounded, at the bound this test
    // already used; see `two_run_ids_cannot_drive_one_worktree_concurrently`
    // for what the loop this replaces could not do.
    producer
        .await_line("held", Duration::from_secs(30))
        .or_fail("the reaper never took its hold");

    assert!(
        observe_cleanup_hold(&husk, &mut NoHooks),
        "R28 is held by a live reaper"
    );

    let started = Instant::now();
    let error = WorktreeLock::acquire_in(&repo, &git_dir)
        .expect_err("a coordinator must not overlap a live reaper");
    let waited = started.elapsed();
    assert!(
        error
            .to_string()
            .contains("still has a process of its own alive"),
        "{error}"
    );
    assert!(
        error.to_string().contains(husk_id),
        "names the run: {error}"
    );
    // Kills the polling-loop mutation: a lease that waited for the hold
    // to release would only have returned after it was gone.
    assert!(
        observe_cleanup_hold(&husk, &mut NoHooks),
        "the refusal returned while the hold was still held"
    );
    assert!(
        waited < Duration::from_secs(5),
        "refused at once rather than waiting the reaper out: {waited:?}"
    );

    // The other observation point: the exclusive probe at run-lock
    // acquisition, which `resource_accounting` names beside the first.
    let error = RunLock::acquire(&husk).expect_err("the exclusive side is refused");
    assert!(error.to_string().contains("already driving run"), "{error}");

    drop(producer);

    // Released with the reaper, by the OS, without anybody resetting it.
    assert!(
        !observe_cleanup_hold(&husk, &mut NoHooks),
        "the hold is gone"
    );
    let lease = WorktreeLock::acquire_in(&repo, &git_dir).expect("and now the lease is free");
    drop(lease);
    let run = RunLock::acquire(&husk).expect("and so is the run lock");
    drop(run);
}

// =======================================================================
// R28, held by a ref write's Git child
// =======================================================================

/// The child a ref write spawns, standing in for `git update-ref`: it does
/// nothing but hold the inherited lease until its stdin closes.
#[cfg(unix)]
#[test]
#[ignore = "spawned as a subprocess by a_ref_writers_child_holds_the_cleanup_lease_until_it_exits"]
fn inherited_hold_child() {
    let mut sink = Vec::new();
    let _ = std::io::stdin().read_to_end(&mut sink);
}

/// `hold_cleanup_lease_for_child`: the lease is the child's, not this
/// process's. Held before the spawn by the descriptor this process keeps,
/// held by the child alone once that descriptor is closed, and released by
/// the kernel at the child's exit with nobody resetting anything -- the fact
/// `WorkspaceManager::reclaim_own_ref_lock` rests on for "no writer of this
/// run is alive".
#[cfg(unix)]
#[test]
fn a_ref_writers_child_holds_the_cleanup_lease_until_it_exits() {
    let root = scratch("childlease");
    let public = public_dir(&root.join("repo"), "01CHILDLEASE000000000000000");
    fs::create_dir_all(&public).expect("the run's public directory");
    assert!(
        !observe_cleanup_hold(&public, &mut NoHooks),
        "nothing holds a lease that does not exist yet"
    );

    let mut command = std::process::Command::new(std::env::current_exe().expect("test binary"));
    command
        .args([
            "--exact",
            "rundir::tests::inherited_hold_child",
            "--ignored",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    let hold = hold_cleanup_lease_for_child(&mut command, &public)
        .expect("take the shared hold")
        .expect("Unix hands the child a hold");
    assert!(
        observe_cleanup_hold(&public, &mut NoHooks),
        "held by this process's descriptor before the spawn"
    );

    let mut child = command.spawn().expect("spawn the child");
    // This process's copy is gone; the child's inherited descriptor is the
    // only reference left to the open file description the lock lives on.
    drop(hold);
    assert!(
        observe_cleanup_hold(&public, &mut NoHooks),
        "the child alone keeps the lease"
    );

    // EOF on the child's stdin ends it.
    drop(child.stdin.take());
    let status = child.wait().expect("reap the child");
    assert!(status.success(), "the child exited cleanly: {status:?}");
    assert!(
        !observe_cleanup_hold(&public, &mut NoHooks),
        "released by the kernel at the child's exit"
    );
}

/// How long a lease observed after its holder's release -- or a sentinel end
/// observed after this process's copy of it is closed -- may keep reading
/// held before the observation fails: a sibling's fork-to-exec window is
/// milliseconds, so this is ten thousand times the hold it tolerates, and it
/// is paid only by a failing run.
#[cfg(unix)]
const LEASE_RELEASE_BOUND: Duration = Duration::from_secs(20);

/// What [`lease_released_within`] reports when the lease still read held at
/// the end of its bound: the bound, the time actually waited and how many
/// observations found it held, which is what tells a hold that never cleared
/// from one observed at a single instant.
#[cfg(unix)]
#[derive(Clone, Copy, Debug)]
struct LeaseHeldPastBound {
    bound: Duration,
    waited: Duration,
    observations: u32,
}

#[cfg(unix)]
impl std::fmt::Display for LeaseHeldPastBound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "the run's cleanup.lock was still held after the full {:?} bound, every one of {} \
             observations over {:?} finding it held",
            self.bound, self.observations, self.waited
        )
    }
}

/// Observe the run's cleanup lease every 50 ms until it reads free or
/// `bound` runs out: how long that took and how many observations it was, or
/// the report of a hold that outlasted the bound. The observation is
/// `observe_cleanup_hold`, production's own read. `on_held` runs after each
/// observation that found the lease held, with that observation's number
/// and before the bound is checked: it is how a test releases a holder only
/// once this observation has seen it, so that the order -- held, released,
/// free -- is acknowledged by the observation rather than timed.
#[cfg(unix)]
fn lease_released_within(
    public: &Path,
    bound: Duration,
    on_held: &mut dyn FnMut(u32),
) -> Result<(Duration, u32), LeaseHeldPastBound> {
    let started = Instant::now();
    let mut observations = 0_u32;
    loop {
        observations += 1;
        if !observe_cleanup_hold(public, &mut NoHooks) {
            return Ok((started.elapsed(), observations));
        }
        on_held(observations);
        let waited = started.elapsed();
        if waited >= bound {
            return Err(LeaseHeldPastBound {
                bound,
                waited,
                observations,
            });
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Every descriptor of this process open on the file at `path`, by the
/// identity of what it is open on -- device and inode -- and never by number.
/// The listing is `/dev/fd`, and the lookup asks the kernel through each
/// listed number itself, `fstat`, for the identity of what that number is
/// open on ([`identity_of_the_descriptor`]), against the target's from a
/// `stat` of its path ([`identity_at`]), the two compared field by field.
/// The lookup owns nothing and has no precondition: a number closed between
/// the listing and its lookup answers `EBADF` and is skipped
/// (`a_descriptor_closed_between_the_listing_and_its_lookup_is_skipped_by_the_identity_scan`
/// constructs that window), any other answer than an identity fails the scan
/// with the number and the error -- a metadata read that failed is neither an
/// absent descriptor nor a present one, and a scan that read it as absent
/// would prove a release that never happened
/// (`a_lookup_that_fails_on_a_listed_descriptor_fails_the_identity_scan_instead_of_reading_absence`)
/// -- and the scan opens and closes nothing, so no lock this process holds
/// on a file it visits is touched. Not a `stat` of
/// the `/dev/fd` entry, which was this fn's lookup before: on macOS that
/// answers no open file's identity -- the native run of `1a1734cf` read an
/// empty scan through it while the hold was open -- and on Linux it is a
/// path operation on an entry another thread's close removes. A number
/// closed by a drop is the next number any
/// thread of this process opens, so a number that reads open is no evidence
/// that the file it used to name is still held, and a number that reads
/// closed is no evidence that nothing else of this process holds that file
/// (`a_lease_descriptors_number_reused_by_an_unrelated_file_is_not_read_as_the_lease_still_open`);
/// one reused for another file has that file's identity and is not counted.
/// Sorted.
#[cfg(unix)]
fn descriptors_open_on(path: &Path) -> Vec<libc::c_int> {
    descriptors_open_on_observing(path, &mut |_| {})
}

/// [`descriptors_open_on`], with `on_listed` run for each number after it is
/// listed and before it is looked up: the seam through which a test closes a
/// listed number inside that window.
#[cfg(unix)]
fn descriptors_open_on_observing(
    path: &Path,
    on_listed: &mut dyn FnMut(libc::c_int),
) -> Vec<libc::c_int> {
    descriptors_open_on_with(path, on_listed, &identity_of_the_descriptor)
}

/// [`descriptors_open_on_observing`] with `lookup` in place of
/// [`identity_of_the_descriptor`]: the seam through which the scan's reading
/// of a lookup's three answers -- an identity, a number not open, a read
/// that failed -- is driven with answers a test chooses.
#[cfg(unix)]
fn descriptors_open_on_with(
    path: &Path,
    on_listed: &mut dyn FnMut(libc::c_int),
    lookup: &dyn Fn(libc::c_int) -> std::io::Result<Option<libc::stat>>,
) -> Vec<libc::c_int> {
    let target = identity_at(path).expect("the file whose descriptors are counted exists");
    let mut open = Vec::new();
    for entry in
        fs::read_dir("/dev/fd").expect("list this process's descriptor table through /dev/fd")
    {
        let entry = entry.expect("an entry of /dev/fd");
        let Ok(number) = entry.file_name().to_string_lossy().parse::<libc::c_int>() else {
            continue;
        };
        on_listed(number);
        match lookup(number) {
            Ok(Some(identity)) => {
                if identity.st_dev == target.st_dev && identity.st_ino == target.st_ino {
                    open.push(number);
                }
            }
            // Closed between the listing and the lookup: nothing is open there.
            Ok(None) => {}
            Err(error) => panic!(
                "the identity of descriptor {number} could not be read: {error}; a metadata read \
                 that failed is neither an absent descriptor nor a present one, and the scan \
                 does not answer through it"
            ),
        }
    }
    open.sort_unstable();
    open
}

/// The identity of the file at `path`, its `stat`, or the error: what
/// [`descriptors_open_on`] compares each descriptor's identity against, read
/// into the same struct so that the comparison is field by field.
#[cfg(unix)]
fn identity_at(path: &Path) -> std::io::Result<libc::stat> {
    use std::os::unix::ffi::OsStrExt as _;

    let path = std::ffi::CString::new(path.as_os_str().as_bytes())
        .expect("a path this module made carries no interior NUL");
    let mut identity = std::mem::MaybeUninit::<libc::stat>::uninit();
    // SAFETY: `stat` reads the NUL-terminated path and writes one `stat`
    // through the pointer; both live for the call.
    if unsafe { libc::stat(path.as_ptr(), identity.as_mut_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: `stat` answered 0, so it wrote the whole struct.
    Ok(unsafe { identity.assume_init() })
}

/// The identity of what `number` is open on, its `fstat`, asked of the
/// kernel through the number itself: `Ok(None)` for a number that is not
/// open (`EBADF`, the one answer that means absence), and the error for any
/// other failed read, which means neither absence nor identity and is the
/// caller's to fail on -- an I/O or permission failure of the read is not a
/// closed descriptor, and no retry is made. The call takes no ownership of
/// the number and has no precondition on it: a number closed since it was
/// listed is answered, not read through, and one closed and reused answers
/// the new file's identity. Nothing is opened or closed, so no lock this
/// process holds is touched -- which a lookup that opened or duplicated the
/// entry and then closed what it opened could not say, closing any
/// descriptor of a file releasing every record lock the process holds on it.
/// The reading of the call's answer is [`identity_answered_by`]'s, with
/// `fstat` as the call.
#[cfg(unix)]
fn identity_of_the_descriptor(number: libc::c_int) -> std::io::Result<Option<libc::stat>> {
    // SAFETY: the call is `fstat` itself, which keeps the seam's contract:
    // it takes the number by value and writes one `stat` through the
    // pointer, which the seam keeps alive for the call, when it answers 0,
    // and nothing that is read otherwise. The closure is inside this block,
    // so the call is made under it.
    unsafe { identity_answered_by(number, |number, identity| libc::fstat(number, identity)) }
}

/// [`identity_of_the_descriptor`] with `call` in place of `fstat`: the one
/// place the call's answer is read -- zero, and the struct it wrote, is the
/// identity; nonzero, and the errno it left, is `EBADF` for a number not
/// open and the error for anything else -- so that a test drives the
/// reading with a call that fails as it chooses, which no policy can make
/// the real call do in the suite's shared process
/// (`a_failed_identity_call_is_read_at_the_call_ebadf_as_absence_and_any_other_errno_as_the_error`;
/// `PR320-R4-MAIN-004`, `PR320-R4-REG-004`).
///
/// # Safety
///
/// `call` answers as `fstat` answers: 0 only after writing the whole
/// `stat` through the pointer, and nonzero with errno set, having written
/// nothing that is read.
///
/// # Errors
///
/// The call's, for any errno but `EBADF`.
#[cfg(unix)]
unsafe fn identity_answered_by(
    number: libc::c_int,
    mut call: impl FnMut(libc::c_int, *mut libc::stat) -> libc::c_int,
) -> std::io::Result<Option<libc::stat>> {
    let mut identity = std::mem::MaybeUninit::<libc::stat>::uninit();
    if call(number, identity.as_mut_ptr()) != 0 {
        let error = std::io::Error::last_os_error();
        return if error.raw_os_error() == Some(libc::EBADF) {
            Ok(None)
        } else {
            Err(error)
        };
    }
    // SAFETY: the call answered 0, so it wrote the whole struct, which is
    // what the caller promised of it.
    Ok(Some(unsafe { identity.assume_init() }))
}

/// The pre-spawn half without a spawn: the descriptor returned is a live
/// shared hold, and dropping it releases the lease. A caller whose spawn
/// fails therefore leaves nothing held.
///
/// The release is this process's own, and it is proved as its own by
/// identity: before the drop the hold's descriptor is the one descriptor
/// this process has open on the lease file, and after it there is none --
/// read through `descriptors_open_on`, which compares what each descriptor
/// is open on and never asks whether the hold's number is still open,
/// because that number is the next one any thread of this process opens.
/// The lease reading free is then observed until it does or
/// `LEASE_RELEASE_BOUND` runs out, because under a whole parallel suite a
/// `fork` another thread makes while the hold is open carries a copy of it
/// to that child's `exec`, and one observation made right after the drop
/// can land inside that window and read the copy
/// (`a_copy_of_the_lease_a_sibling_fork_carries_outlives_this_processs_own_descriptor`,
/// which constructs it). A lease still held at the end of the bound fails
/// this test with the bound and the observations in the message
/// (`a_copy_that_outlasts_the_bound_still_fails_the_release_observation`).
#[cfg(unix)]
#[test]
fn a_hold_whose_command_never_spawns_is_released_with_the_descriptor() {
    use std::os::fd::AsRawFd as _;

    let root = scratch("childlease-unspawned");
    let public = public_dir(&root.join("repo"), "01CHILDLEASE000000000000001");
    fs::create_dir_all(&public).expect("the run's public directory");
    let mut command = std::process::Command::new("git");
    let hold = hold_cleanup_lease_for_child(&mut command, &public)
        .expect("take the shared hold")
        .expect("Unix hands the child a hold");
    assert!(observe_cleanup_hold(&public, &mut NoHooks), "held");
    let lease = cleanup_lock_file(&public);
    let descriptor = hold.as_raw_fd();
    assert_eq!(
        descriptors_open_on(&lease),
        vec![descriptor],
        "before the drop, the hold's descriptor is the one this process has open on the lease"
    );
    drop(hold);
    assert!(
        descriptors_open_on(&lease).is_empty(),
        "this process holds no descriptor on the lease once the hold is dropped"
    );
    if let Err(held) = lease_released_within(&public, LEASE_RELEASE_BOUND, &mut |_| {}) {
        panic!("released with the descriptor: {held}");
    }
}

/// Why the release above is proved by identity and not by number. A
/// descriptor number is free the instant its file is closed and is the next
/// number any thread of this process opens, so under a parallel suite the
/// hold's number can name an unrelated file before anything reads it.
/// Constructed here in this thread's own hands, with no window for another
/// thread to take part: `dup2` closes the hold's descriptor and installs a
/// copy of `/dev/null` under the same number in one call, which is what a
/// drop followed by another thread's open leaves behind. The lease reads
/// free -- within the bound the release test observes under, because a
/// sibling's fork made while the hold was open carries a copy into its
/// `exec` window here too -- this process has no descriptor open on it, and
/// the number is open: exactly the reading a check by number would mistake
/// for the lease still held.
#[cfg(unix)]
#[test]
fn a_lease_descriptors_number_reused_by_an_unrelated_file_is_not_read_as_the_lease_still_open() {
    use std::os::fd::AsRawFd as _;

    let root = scratch("childlease-number-reused");
    let public = public_dir(&root.join("repo"), "01CHILDLEASE000000000000006");
    fs::create_dir_all(&public).expect("the run's public directory");
    let mut command = std::process::Command::new("git");
    let hold = hold_cleanup_lease_for_child(&mut command, &public)
        .expect("take the shared hold")
        .expect("Unix hands the child a hold");
    let lease = cleanup_lock_file(&public);
    let descriptor = hold.as_raw_fd();
    assert_eq!(
        descriptors_open_on(&lease),
        vec![descriptor],
        "the hold's descriptor is the one open on the lease"
    );
    let unrelated = File::open("/dev/null").expect("an unrelated file");
    // SAFETY: `dup2` takes two descriptors by value: it closes `descriptor`,
    // which `hold` owns and this thread alone uses, and makes it a copy of
    // `unrelated` in the same call, so no other thread's descriptor can be
    // the one it replaces. `hold` goes on owning the number and closes the
    // copy when it drops.
    assert_eq!(
        unsafe { libc::dup2(unrelated.as_raw_fd(), descriptor) },
        descriptor,
        "the hold's number now names /dev/null: {}",
        std::io::Error::last_os_error()
    );
    if let Err(held) = lease_released_within(&public, LEASE_RELEASE_BOUND, &mut |_| {}) {
        panic!("the lease reads free once the dup2 has closed its only descriptor here: {held}");
    }
    assert!(
        descriptors_open_on(&lease).is_empty(),
        "and this process has no descriptor open on it, whatever its old number names now"
    );
    // SAFETY: `fcntl` with `F_GETFD` reads one flag of this process's own
    // descriptor table and touches no memory.
    assert_ne!(
        unsafe { libc::fcntl(descriptor, libc::F_GETFD) },
        -1,
        "the number is open, for /dev/null: a check by number would read the lease as still \
         held here"
    );
    drop(hold);
    drop(unrelated);
}

/// Why the lookup is a raw `fstat` through the number, a call with no
/// precondition, and not a `File` made from it. A number listed by `/dev/fd`
/// is another thread's to close at any moment after the listing, and a
/// `File` made from a number that is no longer open is a broken contract
/// whatever its `fstat` then answers; the raw call owns nothing and answers
/// `EBADF` for such a number, which the scan reads as nothing open there and
/// nothing else. Constructed with the closing in the observation's hands:
/// another thread owns a file, the scan reaches its number, and only then is
/// that file closed and the close acknowledged; the lookup of that number
/// finds nothing there, and the scan goes on to report this thread's own
/// descriptor on the target and nothing else.
#[cfg(unix)]
#[test]
fn a_descriptor_closed_between_the_listing_and_its_lookup_is_skipped_by_the_identity_scan() {
    use std::os::fd::AsRawFd as _;
    use std::sync::mpsc;

    let root = scratch("descriptor-scan-closed-in-window");
    let target = root.join("target");
    let held = File::create(&target).expect("the target, held open by this thread");
    let (number_sender, number_receiver) = mpsc::channel();
    let (close_sender, close_receiver) = mpsc::channel::<()>();
    let (closed_sender, closed_receiver) = mpsc::channel();
    let owner = std::thread::spawn(move || {
        let unrelated = File::open("/dev/null").expect("a file another thread owns");
        number_sender
            .send(unrelated.as_raw_fd())
            .expect("the scanning thread waits for the number");
        if close_receiver.recv().is_ok() {
            drop(unrelated);
            closed_sender
                .send(())
                .expect("the scanning thread waits for the acknowledgement");
        }
    });
    let unrelated_number = number_receiver.recv().expect("the owner opened its file");
    let mut closed_in_window = false;
    let open = descriptors_open_on_observing(&target, &mut |listed| {
        if listed == unrelated_number && !closed_in_window {
            close_sender
                .send(())
                .expect("the owner waits to close its file");
            closed_receiver
                .recv()
                .expect("the owner closed its file and said so");
            closed_in_window = true;
        }
    });
    drop(close_sender);
    owner.join().expect("the owner thread ends");
    assert!(
        closed_in_window,
        "the listing reached the owner's number {unrelated_number} while its file was open"
    );
    assert_eq!(
        open,
        vec![held.as_raw_fd()],
        "the scan reports this thread's descriptor on the target and nothing for the number \
         closed between its listing and its lookup"
    );
    drop(held);
}

/// Why the observation above is bounded. A copy of this process's descriptor
/// carried by a sibling's fork keeps the lease after the descriptor itself is
/// closed: the fork is `ParkedFork::holding`, a child of this process parked
/// with that one descriptor and its socket, whose report comes from inside
/// that window, so it is known alive and holding. The single observation the
/// test above used to make reads held. The bounded one is then made with the
/// release in its own hands: its first observation reads the copy held and,
/// from inside that observation, releases the fork; a later observation
/// reads free. Held, released, free is therefore an order the observation
/// acknowledged, not one a clock was trusted to produce, and a legal
/// schedule that delays either side changes nothing: the release waits for
/// the observation, however long the observation takes to arrive, and a
/// second sibling's copy in its own window after the release costs
/// observations that are reported, not asserted.
#[cfg(unix)]
#[test]
fn a_copy_of_the_lease_a_sibling_fork_carries_outlives_this_processs_own_descriptor() {
    use crate::workspace_manager::fixture::ParkedFork;
    use std::os::fd::AsRawFd as _;

    let root = scratch("childlease-sibling-copy");
    let public = public_dir(&root.join("repo"), "01CHILDLEASE000000000000003");
    fs::create_dir_all(&public).expect("the run's public directory");
    let mut command = std::process::Command::new("git");
    let hold = hold_cleanup_lease_for_child(&mut command, &public)
        .expect("take the shared hold")
        .expect("Unix hands the child a hold");
    let lease = cleanup_lock_file(&public);
    let descriptor = hold.as_raw_fd();
    let parked = ParkedFork::holding(descriptor);
    let holder = parked.pid();
    assert!(parked.is_alive(), "the parked fork {holder} is alive");
    let [lease_number, _] = parked.kept();
    assert_eq!(lease_number, descriptor, "and it kept the lease copy");
    drop(hold);
    assert!(
        descriptors_open_on(&lease).is_empty(),
        "this process holds no descriptor on the lease once the hold is dropped"
    );
    assert!(
        observe_cleanup_hold(&public, &mut NoHooks),
        "one observation right after the drop reads the lease held: the copy in fork {holder}'s \
         window"
    );

    let mut parked = Some(parked);
    let mut released = None;
    let observed = lease_released_within(&public, LEASE_RELEASE_BOUND, &mut |observation| {
        if let Some(parked) = parked.take() {
            released = Some((observation, parked.release()));
        }
    });
    let (released_at, status) = released
        .expect("the bounded observation read the copy held, and only then released the fork");
    let (waited, observations) = observed.expect("the lease reads free once the fork is released");
    assert_eq!(
        released_at, 1,
        "the first observation read the copy held, so the release came after it"
    );
    assert!(
        observations > released_at,
        "and the observation that read free came after the one that released the fork: \
         {observations} observations over {waited:?}"
    );
    assert!(
        status.success(),
        "the released fork exited cleanly: {status:?}"
    );
}

/// The bound is a bound: a copy that outlasts it fails the observation with
/// the bound and the count in the message, and the observation never turns a
/// missing release into success. The count is reported, not bounded below:
/// how many observations fit inside the bound is the scheduler's to decide,
/// and no test may assert it.
#[cfg(unix)]
#[test]
fn a_copy_that_outlasts_the_bound_still_fails_the_release_observation() {
    use crate::workspace_manager::fixture::ParkedFork;
    use std::os::fd::AsRawFd as _;

    let root = scratch("childlease-past-bound");
    let public = public_dir(&root.join("repo"), "01CHILDLEASE000000000000005");
    fs::create_dir_all(&public).expect("the run's public directory");
    let mut command = std::process::Command::new("git");
    let hold = hold_cleanup_lease_for_child(&mut command, &public)
        .expect("take the shared hold")
        .expect("Unix hands the child a hold");
    let lease = cleanup_lock_file(&public);
    let descriptor = hold.as_raw_fd();
    let parked = ParkedFork::holding(descriptor);
    let holder = parked.pid();
    drop(hold);
    assert!(
        descriptors_open_on(&lease).is_empty(),
        "this process holds no descriptor on the lease once the hold is dropped"
    );

    let held = lease_released_within(&public, Duration::from_millis(300), &mut |_| {})
        .expect_err("a copy alive past the bound is reported, not waited into success");
    assert!(
        held.waited >= held.bound,
        "the observation ran its whole bound: {held}"
    );
    assert!(
        held.to_string()
            .contains("still held after the full 300ms bound"),
        "{held}"
    );
    assert!(
        parked.is_alive() && observe_cleanup_hold(&public, &mut NoHooks),
        "the holder {holder} is alive and holding after the report"
    );
    let status = parked.release();
    assert!(
        status.success(),
        "the released fork exited cleanly: {status:?}"
    );
    lease_released_within(&public, LEASE_RELEASE_BOUND, &mut |_| {})
        .expect("and the lease reads free once the fork has exited");
}

/// What [`sentinel_closed_within`] reports when a copy of the sentinel end
/// was still open at the end of its bound: the bound, the time actually
/// waited and how many reads found a copy open, which is what tells a copy
/// that never closed from one observed at a single instant.
#[cfg(unix)]
#[derive(Clone, Copy, Debug)]
struct SentinelHeldPastBound {
    bound: Duration,
    waited: Duration,
    observations: u32,
}

#[cfg(unix)]
impl std::fmt::Display for SentinelHeldPastBound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "a copy of the sentinel was still open after the full {:?} bound, every one of {} \
             reads over {:?} finding one",
            self.bound, self.observations, self.waited
        )
    }
}

/// A sentinel socket pair: the end a fork inherits and keeps or sweeps, and
/// this process's observer end, whose 50 ms read timeout is set here, while
/// both ends are open, and never at an observation. On macOS a socket whose
/// peer is gone accepts no option -- `setsockopt` answers `EINVAL` once every
/// copy of the other end has closed, which the native run of `1a1734cf` hit
/// at the observation that follows a keeper's release -- where Linux accepts
/// it, so a timeout set at the observation reads as sound on Linux and is
/// not.
#[cfg(unix)]
fn sentinel_pair() -> (
    std::os::unix::net::UnixStream,
    std::os::unix::net::UnixStream,
) {
    let (sentinel, observer) =
        std::os::unix::net::UnixStream::pair().expect("a sentinel socket pair");
    observer
        .set_read_timeout(Some(Duration::from_millis(50)))
        .expect("bound each read of the sentinel while its peer is open");
    (sentinel, observer)
}

/// Read `observer`, this process's end of a [`sentinel_pair`], every 50 ms --
/// its read timeout, set when the pair was made -- until it answers EOF or
/// `bound` runs out: how long that took and how many reads it was, or the
/// report of a copy that outlasted the bound. EOF is the
/// kernel's answer that no copy of the other end is open in any process, so
/// it proves that the fork under test closed its inherited one only once
/// every other copy is accounted for: a sibling's fork made while that end
/// was open carries a copy into its own window exactly as it does the lease,
/// which is why the read is observed within the bound the lease observations
/// use and never at one instant. `on_held` runs after each read that found a
/// copy open, with that read's number and before the bound is checked: how a
/// test releases a holder only once this observation has seen its copy, so
/// that held, released, EOF is an order the observation acknowledged rather
/// than one a timer arranged. A read a handled signal ended before its
/// timeout (`Interrupted`) observed nothing -- neither a copy open nor EOF --
/// so it is neither counted nor acknowledged; the bound, absolute from the
/// start, decides whether the read is made again, and no interruption
/// restarts it (`PR320-R3-MAIN-005`). On Linux a report is paired by the
/// caller with the fork under test's own table ([`sentinel_attribution`]),
/// which says whose copy it was. Any reader, so that the answers a socket
/// gives -- EOF, a timeout, an interruption -- are driven in an order a test
/// chooses (`an_interrupted_sentinel_read_is_made_again_within_the_same_bound`).
#[cfg(unix)]
fn sentinel_closed_within(
    observer: &mut impl std::io::Read,
    bound: Duration,
    on_held: &mut dyn FnMut(u32),
) -> Result<(Duration, u32), SentinelHeldPastBound> {
    let started = Instant::now();
    let mut observations = 0_u32;
    let mut byte = [0_u8; 1];
    loop {
        match observer.read(&mut byte) {
            Ok(0) => {
                observations += 1;
                return Ok((started.elapsed(), observations));
            }
            Ok(_) => panic!("nothing writes to a sentinel, yet a read answered bytes"),
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {
                let waited = started.elapsed();
                if waited >= bound {
                    return Err(SentinelHeldPastBound {
                        bound,
                        waited,
                        observations,
                    });
                }
                continue;
            }
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock
                    || error.kind() == std::io::ErrorKind::TimedOut =>
            {
                observations += 1;
            }
            Err(error) => panic!("read the sentinel: {error}"),
        }
        on_held(observations);
        let waited = started.elapsed();
        if waited >= bound {
            return Err(SentinelHeldPastBound {
                bound,
                waited,
                observations,
            });
        }
    }
}

/// The identity a fork's table is searched for when a sentinel read has not
/// answered EOF within its bound: on Linux the `/proc` link of this process's
/// descriptor, `socket:[<inode>]` for a socket end, read while the descriptor
/// is still open.
#[cfg(target_os = "linux")]
fn sentinel_identity(number: libc::c_int) -> std::ffi::OsString {
    std::fs::read_link(format!("/proc/self/fd/{number}"))
        .expect("this process's own descriptor has a /proc entry")
        .into_os_string()
}

/// On the Unixes without `/proc`, nothing here can name whose copy it is:
/// the identity is that statement, carried as a value so that a test binds
/// and passes it exactly as it does the Linux one.
#[cfg(all(unix, not(target_os = "linux")))]
struct SentinelIdentityUnreadable;

/// See [`SentinelIdentityUnreadable`].
#[cfg(all(unix, not(target_os = "linux")))]
fn sentinel_identity(_number: libc::c_int) -> SentinelIdentityUnreadable {
    SentinelIdentityUnreadable
}

/// Whose copy a sentinel read that has not answered EOF is reading, as far as
/// the platform can say: on Linux the fork under test's `/proc` table either
/// holds a descriptor with the sentinel's identity or it does not, and the
/// answer attributes the copy to that fork or explicitly to another process
/// of this test binary -- a sibling's fork window -- rather than to whichever
/// child the test happened to make.
#[cfg(target_os = "linux")]
fn sentinel_attribution(holder: libc::pid_t, sentinel: &std::ffi::OsStr) -> String {
    let table = match std::fs::read_dir(format!("/proc/{holder}/fd")) {
        Ok(entries) => entries,
        Err(error) => return format!("fork {holder}'s table could not be read: {error}"),
    };
    let mut holds = false;
    for entry in table {
        let Ok(entry) = entry else {
            return format!("fork {holder}'s table could not be listed to its end");
        };
        if std::fs::read_link(entry.path()).is_ok_and(|link| link.as_os_str() == sentinel) {
            holds = true;
        }
    }
    if holds {
        format!("fork {holder}'s table holds a copy of the sentinel {sentinel:?}")
    } else {
        format!(
            "fork {holder}'s table holds no copy of the sentinel {sentinel:?}: the copy still open \
             belongs to another process of this test binary, a fork made while the sentinel was \
             open"
        )
    }
}

/// On the Unixes without `/proc`, nothing here can name whose copy it is.
#[cfg(all(unix, not(target_os = "linux")))]
fn sentinel_attribution(holder: libc::pid_t, _sentinel: &SentinelIdentityUnreadable) -> String {
    format!("whether fork {holder} holds a copy cannot be read on this platform")
}

/// On Linux, the fork under test's own descriptor table, read from `/proc`:
/// exactly stdio, its lease copy and its socket end, the lease entry naming
/// `cleanup.lock`. The child-specific proof of what the fork holds, which no
/// read of a sentinel from this process can be: EOF is a fact about every
/// copy of an end at once.
#[cfg(target_os = "linux")]
fn assert_the_forks_table_is_stdio_the_lease_and_its_socket(
    holder: libc::pid_t,
    lease_number: libc::c_int,
    socket_number: libc::c_int,
    under: &str,
) {
    let mut entries: Vec<(libc::c_int, PathBuf)> = std::fs::read_dir(format!("/proc/{holder}/fd"))
        .expect("the child's descriptor table")
        .map(|entry| {
            let entry = entry.expect("an entry of the table");
            let number = entry
                .file_name()
                .to_string_lossy()
                .parse()
                .expect("a descriptor number");
            (number, std::fs::read_link(entry.path()).unwrap_or_default())
        })
        .collect();
    entries.sort();
    let numbers: Vec<libc::c_int> = entries.iter().map(|(number, _)| *number).collect();
    let mut expected = vec![0, 1, 2, lease_number, socket_number];
    expected.sort_unstable();
    assert_eq!(
        numbers, expected,
        "{under}: the child's table is stdio, the lease copy and the socket: {entries:?}"
    );
    let lease_target = &entries
        .iter()
        .find(|(number, _)| *number == lease_number)
        .expect("the lease entry")
        .1;
    assert!(
        lease_target.ends_with("cleanup.lock"),
        "{under}: the kept descriptor is the run's lease: {lease_target:?}"
    );
}

/// What the parked fork holds, and what it does not: the lease copy and its
/// socket, and not a sentinel this process had open at the fork, nor another
/// run's lease. The sentinel is one end of a socket pair. With this process's
/// copy of that end closed, a read on the other end answers EOF only when
/// nobody holds a copy, so EOF is the proof that no copy survives -- in the
/// fork under test and in every other process alike, which is the point:
/// under a whole parallel suite a sibling's fork made while the end was open
/// carries a copy into its own window, exactly as it does the lease, and a
/// single read made right after the drop can land inside that window and
/// read the sibling's copy. So the read is observed within the bound the
/// lease observations use, and the test constructs the sibling itself: a
/// second parked fork told to keep only the sentinel, released from inside
/// the first read that found a copy open, so that held, released, EOF is an
/// order the observation acknowledged (the shape of
/// `a_copy_of_the_lease_a_sibling_fork_carries_outlives_this_processs_own_descriptor`),
/// while the fork under test is proved on its own account: on Linux its
/// descriptor table, read from `/proc`, is exactly stdio, the socket and the
/// lease, the lease entry naming `cleanup.lock`, and a read that has not
/// answered EOF at the end of the bound says whether that fork holds a copy.
/// The control is a parked fork told to keep a second sentinel: the read
/// cannot answer EOF while that child lives -- on Linux its table shows the
/// copy -- and answers it within the bound once the child is released; and
/// another run's lease, open across that control's fork and this process's
/// copy then dropped, reads free within the bound while the control lives,
/// because the control closed its copy -- where a raw `fork` that closed
/// nothing would hold every descriptor of every concurrent test for as long
/// as it slept.
#[cfg(unix)]
#[test]
fn a_parked_fork_holds_the_lease_copy_and_its_socket_and_nothing_else() {
    use crate::workspace_manager::fixture::ParkedFork;
    use std::os::fd::AsRawFd as _;

    let root = scratch("childlease-sentinel");
    let public = public_dir(&root.join("repo"), "01CHILDLEASE000000000000004");
    fs::create_dir_all(&public).expect("the run's public directory");
    let (sentinel, mut observer) = sentinel_pair();
    let sentinel_number = sentinel.as_raw_fd();
    let identity = sentinel_identity(sentinel_number);
    let sibling = ParkedFork::holding(sentinel_number);
    let sibling_pid = sibling.pid();

    let parked = ParkedFork::holding_the_lease_of(&public);
    let holder = parked.pid();
    let [lease_number, socket_number] = parked.kept();
    assert!(
        parked.is_alive() && observe_cleanup_hold(&public, &mut NoHooks),
        "the parked fork {holder} holds the lease"
    );
    assert!(
        sentinel_number != lease_number && sentinel_number != socket_number,
        "the sentinel {sentinel_number} is neither kept descriptor"
    );
    #[cfg(target_os = "linux")]
    assert_the_forks_table_is_stdio_the_lease_and_its_socket(
        holder,
        lease_number,
        socket_number,
        "the fork under test",
    );
    drop(sentinel);
    let mut sibling = Some(sibling);
    let mut released = None;
    let observed = sentinel_closed_within(&mut observer, LEASE_RELEASE_BOUND, &mut |read| {
        if let Some(sibling) = sibling.take() {
            released = Some((read, sibling.release()));
        }
    });
    let (released_at, sibling_status) = released
        .expect("the first read found the sibling's copy open, and only then released the sibling");
    assert_eq!(
        released_at, 1,
        "the sibling {sibling_pid} kept its copy until the first read had seen it"
    );
    assert!(
        sibling_status.success(),
        "the released sibling exited cleanly: {sibling_status:?}"
    );
    match observed {
        Ok((waited, reads)) => assert!(
            reads > released_at,
            "the read that answered EOF came after the one that released the sibling: {reads} \
             reads over {waited:?}"
        ),
        Err(held) => panic!(
            "with this process's copy of the sentinel closed and the sibling released, the read \
             answers EOF, so no copy of it survives in fork {holder}: {held}; {}",
            sentinel_attribution(holder, &identity)
        ),
    }
    let status = parked.release();
    assert!(
        status.success(),
        "the released fork exited cleanly: {status:?}"
    );

    let (sentinel, mut observer) = sentinel_pair();
    let identity = sentinel_identity(sentinel.as_raw_fd());
    let unrelated_public = public_dir(&root.join("repo"), "01CHILDLEASE000000000000007");
    fs::create_dir_all(&unrelated_public).expect("another run's public directory");
    let mut unrelated_command = std::process::Command::new("git");
    let unrelated_hold = hold_cleanup_lease_for_child(&mut unrelated_command, &unrelated_public)
        .expect("take the other run's hold")
        .expect("Unix hands the child a hold");
    let keeper = ParkedFork::holding(sentinel.as_raw_fd());
    let keeper_pid = keeper.pid();
    drop(unrelated_hold);
    match lease_released_within(&unrelated_public, LEASE_RELEASE_BOUND, &mut |_| {}) {
        Ok((waited, observations)) => assert!(
            keeper.is_alive(),
            "the control {keeper_pid} is still parked when the other run's lease reads free \
             ({observations} observations over {waited:?}), so the copy that cleared was a \
             sibling's window and not the control's, which closed its own before it reported"
        ),
        Err(held) => panic!(
            "the other run's lease, open across the control's fork, reads free once this \
             process's copy is dropped, because the control {keeper_pid} closed its copy: {held}"
        ),
    }
    drop(sentinel);
    let held = sentinel_closed_within(&mut observer, Duration::from_millis(150), &mut |_| {})
        .expect_err(
            "a parked fork that kept the sentinel holds a copy of it, so the read cannot answer \
             EOF while that child lives",
        );
    assert!(
        held.waited >= held.bound && keeper.is_alive(),
        "the control {keeper_pid} lived through the whole bound: {held}"
    );
    #[cfg(target_os = "linux")]
    {
        let attribution = sentinel_attribution(keeper_pid, &identity);
        assert!(
            attribution.contains("holds a copy of the sentinel"),
            "and its table says so, which is the reading a report of the fork under test would \
             give: {attribution}"
        );
    }
    let status = keeper.release();
    assert!(
        status.success(),
        "the released control exited cleanly: {status:?}"
    );
    if let Err(held) = sentinel_closed_within(&mut observer, LEASE_RELEASE_BOUND, &mut |_| {}) {
        panic!(
            "and answers EOF once that fork has exited: {held}; {}",
            sentinel_attribution(keeper_pid, &identity)
        );
    }
}

/// Whether `pid` is no child of this process any more: `waitpid` with
/// `WNOHANG` answers `ECHILD`, so it has been collected. Asked right after
/// the collection it checks, before the kernel could hand the number to
/// another child of this process.
#[cfg(unix)]
fn is_reaped(pid: libc::pid_t) -> bool {
    let mut status = 0;
    // SAFETY: `waitpid` writes one int through `status`, which lives for the
    // call, and takes the pid and the flags by value.
    let answered = unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) };
    answered == -1 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ECHILD)
}

/// Collect `pid`, a child of this process, within `bound`, `waitpid` with
/// `WNOHANG` every 5 ms: its status, or `None` for a child not collected by
/// the end of the bound. For a test that collects a child its owner could
/// not, from a thread no policy binds; never `waitpid` without `WNOHANG`,
/// which would hold the test for as long as the kernel took.
#[cfg(target_os = "linux")]
fn collect_child_within(pid: libc::pid_t, bound: Duration) -> Option<std::process::ExitStatus> {
    use std::os::unix::process::ExitStatusExt as _;

    let started = Instant::now();
    loop {
        let mut status = 0;
        // SAFETY: as `is_reaped`.
        let answered = unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) };
        if answered == pid {
            return Some(std::process::ExitStatus::from_raw(status));
        }
        assert_eq!(
            answered,
            0,
            "waitpid({pid}, WNOHANG): {}",
            std::io::Error::last_os_error()
        );
        if started.elapsed() >= bound {
            return None;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Stop the parked child `pid` and observe that it stopped, so that it
/// cannot read its release: `SIGSTOP`, then `waitpid` with `WUNTRACED`,
/// which reports the stop and collects nothing.
#[cfg(unix)]
fn stop_parked_child(pid: libc::pid_t) {
    // SAFETY: `kill` takes a pid and a signal by value; the pid is a parked
    // child this test owns.
    assert_eq!(
        unsafe { libc::kill(pid, libc::SIGSTOP) },
        0,
        "stop the parked child {pid}: {}",
        std::io::Error::last_os_error()
    );
    let mut status = 0;
    // SAFETY: as `is_reaped`; `WUNTRACED` reports the stop and collects
    // nothing.
    assert_eq!(
        unsafe { libc::waitpid(pid, &mut status, libc::WUNTRACED) },
        pid,
        "observe the stop of {pid}: {}",
        std::io::Error::last_os_error()
    );
    assert!(
        libc::WIFSTOPPED(status),
        "the child {pid} is stopped: {status:#x}"
    );
}

/// Run `act` on a thread of its own and wait for its answer within `bound`:
/// the test's failure bound around a step that must itself be bounded, so a
/// step that is not fails the test instead of hanging it. The thread's
/// handle comes back with the answer, or with the timeout, for the caller to
/// unblock and join; the thread's send is best effort by design, because a
/// caller whose bound ran out reports that bound and not the late answer.
#[cfg(unix)]
fn on_a_thread_within<T: Send + 'static>(
    bound: Duration,
    act: impl FnOnce() -> T + Send + 'static,
) -> (
    Result<T, std::sync::mpsc::RecvTimeoutError>,
    std::thread::JoinHandle<()>,
) {
    let (sender, receiver) = std::sync::mpsc::channel();
    let handle = std::thread::spawn(move || {
        if sender.send(act()).is_err() {
            // The caller stopped waiting: its failure bound expired and it
            // reports that, which is the outcome this late answer confirms.
        }
    });
    (receiver.recv_timeout(bound), handle)
}

/// A parked child that never reads its release -- stopped, here, so that it
/// cannot -- is killed at the end of the reap bound and collected, and the
/// release returns its status saying so. The release runs on a thread of
/// its own so that a release which blocked past every bound would fail this
/// test rather than hang it: the outer bound is the test's failure bound,
/// thirty times the reap bound, and on its expiry the child is continued so
/// that the release can finish before the test fails.
#[cfg(unix)]
#[test]
fn a_parked_child_that_never_reads_its_release_is_killed_and_reaped_within_the_bound() {
    use crate::workspace_manager::fixture::ParkedFork;
    use std::os::fd::AsRawFd as _;
    use std::os::unix::process::ExitStatusExt as _;

    const BOUND: Duration = Duration::from_secs(1);
    let anchor = File::open("/dev/null").expect("a descriptor for the fork to keep");
    let parked = ParkedFork::holding(anchor.as_raw_fd()).reap_bound(BOUND);
    let pid = parked.pid();
    stop_parked_child(pid);
    assert!(parked.is_alive(), "a stopped child has not ended");
    let (released, releasing) = on_a_thread_within(BOUND * 30, move || {
        let started = Instant::now();
        (parked.release(), started.elapsed())
    });
    if released.is_err() {
        // SAFETY: `kill` takes a pid and a signal by value; the child is this
        // test's own, stopped above, and continuing it lets a blocked release
        // finish so the test can fail rather than hang.
        assert_eq!(
            unsafe { libc::kill(pid, libc::SIGCONT) },
            0,
            "continue the stopped child {pid}: {}",
            std::io::Error::last_os_error()
        );
    }
    releasing.join().expect("the releasing thread ends");
    let (status, took) = released.expect(
        "the release returned within thirty times its reap bound; a release that blocks past its \
         bound is the defect this test holds, and the stopped child was continued so that it \
         could finish",
    );
    assert_eq!(
        status.signal(),
        Some(libc::SIGKILL),
        "the child was killed at the end of the reap bound: {status:?}"
    );
    assert!(
        took >= BOUND,
        "the reap bound was waited out before the kill: {took:?}"
    );
    assert!(is_reaped(pid), "and the killed child {pid} was collected");
}

/// Dropping a parked fork releases and reaps its child on the same bound: a
/// child that cannot read the release because it is stopped is killed at the
/// end of the reap bound and collected before the drop returns.
#[cfg(unix)]
#[test]
fn dropping_a_parked_fork_reaps_its_child_even_when_the_child_is_stopped() {
    use crate::workspace_manager::fixture::ParkedFork;
    use std::os::fd::AsRawFd as _;

    const BOUND: Duration = Duration::from_millis(500);
    let anchor = File::open("/dev/null").expect("a descriptor for the fork to keep");
    let parked = ParkedFork::holding(anchor.as_raw_fd()).reap_bound(BOUND);
    let pid = parked.pid();
    stop_parked_child(pid);
    let (dropped, dropping) = on_a_thread_within(BOUND * 30, move || {
        let started = Instant::now();
        drop(parked);
        started.elapsed()
    });
    if dropped.is_err() {
        // SAFETY: as in the test above: this test's own stopped child,
        // continued so that a blocked drop can finish before the test fails.
        assert_eq!(
            unsafe { libc::kill(pid, libc::SIGCONT) },
            0,
            "continue the stopped child {pid}: {}",
            std::io::Error::last_os_error()
        );
    }
    dropping.join().expect("the dropping thread ends");
    let took = dropped.expect(
        "the drop returned within thirty times its reap bound; a drop that blocks past its bound \
         is the defect this test holds",
    );
    assert!(
        took >= BOUND,
        "the reap bound was waited out before the kill: {took:?}"
    );
    assert!(
        is_reaped(pid),
        "the drop killed the stopped child {pid} at the end of its reap bound and collected it"
    );
}

/// A release whose every observation a policy interrupts -- `wait4`
/// answering `EINTR` on the owner's thread alone, installed after the child
/// is acknowledged -- ends at its bounds instead of inside a retry of its
/// own: the owner sends the kill at the end of the reap bound and, the child
/// not collectable within another, the release panics naming the pid, the
/// signal sent and the interrupted observation (`PR320-R4-MAIN-001`,
/// `PR320-R4-REG-002`); the unwinding drop spends the same two bounds again
/// and prints. The child is live, as in REG's probe: it reads its release
/// byte and ends by itself, which its owner cannot see, so the signal
/// reaches a zombie, and the child, collected here from a thread the policy
/// does not bind, has the status of its own exit -- the owner said what it
/// observed and no more. The release runs on a thread of its own because
/// the policy is the thread's for good. Linux, for the policy.
///
/// The body runs in a process of its own
/// (`release-a-parked-fork-whose-every-observation-is-interrupted`,
/// [`release_a_parked_fork_whose_every_observation_is_interrupted`]), in a
/// group of its own, and this test holds it to having run to its end
/// ([`assert_the_scenario_ran_to_its_end`]). On the failing path the scenario
/// fails at its own bound and exits, and the wrapper's kill of its group ends
/// the child the stuck owner still holds -- the scenario's pid uncollected at
/// the kill, so no number is signalled that could have been handed out again --
/// before the group is observed empty: nothing in this process waits on, or is
/// left behind with, a thread spinning under a policy (`PR320-R5-MAIN-004`,
/// `PR320-R5-REG-004`).
#[cfg(target_os = "linux")]
#[test]
fn a_release_whose_every_observation_is_interrupted_ends_at_its_bounds_and_says_the_child_is_uncollected()
 {
    assert_the_scenario_ran_to_its_end(
        "release-a-parked-fork-whose-every-observation-is-interrupted",
    );
}

/// Dropping a parked fork whose every observation a policy interrupts
/// returns within its bounds too -- the kill sent at the end of the reap
/// bound, the collection given the reap bound again and then given up --
/// and never panics; what the drop could not do is on stderr, which
/// `dropping_a_parked_fork_whose_kill_the_os_refuses_says_so_and_what_is_left`
/// reads from a process of its own. The child is stopped, as in MAIN's
/// probe, so it cannot read its release and the kill is what ends it: its
/// status, collected here from a thread the policy does not bind, says so.
/// Linux, for the policy.
///
/// The body runs in a process of its own
/// (`drop-a-stopped-parked-fork-whose-every-observation-is-interrupted`,
/// [`drop_a_stopped_parked_fork_whose_every_observation_is_interrupted`]), in a
/// group of its own, and this test holds it to having run to its end
/// ([`assert_the_scenario_ran_to_its_end`]). On the failing path the scenario
/// fails at its own bound and exits, and the wrapper's kill of its group ends
/// the child the stuck owner still holds -- the scenario's pid uncollected at
/// the kill, so no number is signalled that could have been handed out again --
/// before the group is observed empty: nothing in this process waits on, or is
/// left behind with, a thread spinning under a policy (`PR320-R5-MAIN-004`,
/// `PR320-R5-REG-004`).
#[cfg(target_os = "linux")]
#[test]
fn dropping_a_parked_fork_whose_every_observation_is_interrupted_returns_within_its_bounds() {
    assert_the_scenario_ran_to_its_end(
        "drop-a-stopped-parked-fork-whose-every-observation-is-interrupted",
    );
}

/// A liveness observation a policy interrupts for the whole reap bound
/// fails instead of answering: `is_alive` makes an interrupted observation
/// again at the tick within the bound, and at its end panics naming the
/// pid, the bound and the interruption -- neither alive nor ended
/// (`PR320-R4-MAIN-001`, `PR320-R4-REG-002`). The fork is then dropped on
/// the same thread, bounded as the drop is, and the child, live and
/// released by that drop, is collected here from a thread the policy does
/// not bind. Linux, for the policy.
///
/// The body runs in a process of its own
/// (`observe-a-parked-fork-whose-every-observation-is-interrupted`,
/// [`observe_a_parked_fork_whose_every_observation_is_interrupted`]), in a
/// group of its own, and this test holds it to having run to its end
/// ([`assert_the_scenario_ran_to_its_end`]). On the failing path the scenario
/// fails at its own bound and exits, and the wrapper's kill of its group ends
/// the child the stuck owner still holds -- the scenario's pid uncollected at
/// the kill, so no number is signalled that could have been handed out again --
/// before the group is observed empty: nothing in this process waits on, or is
/// left behind with, a thread spinning under a policy (`PR320-R5-MAIN-004`,
/// `PR320-R5-REG-004`).
#[cfg(target_os = "linux")]
#[test]
fn a_liveness_observation_interrupted_for_the_whole_bound_fails_instead_of_answering() {
    assert_the_scenario_ran_to_its_end(
        "observe-a-parked-fork-whose-every-observation-is-interrupted",
    );
}

/// Run `scenario` in a process of its own and hold it to having run to its
/// end: exit 0, which the scenario's assertions give only when all of them
/// held, and the wrapper's notes saying that the scenario's group was killed
/// or found empty and then observed empty, so that whatever the scenario
/// left -- a child a refused kill left stopped, or one a stuck owner still
/// held when the scenario failed -- has ended with the group. The scenario's
/// text, the wrapper's notes with it, is in every message, and is returned
/// for the caller's own assertions.
#[cfg(target_os = "linux")]
fn assert_the_scenario_ran_to_its_end(scenario: &str) -> String {
    let (status, text) = run_parked_fork_scenario(scenario);
    assert!(
        status.is_some_and(|status| status.success()),
        "the scenario `{scenario}`, in a process of its own, ran to its end: {status:?}\n{text}"
    );
    assert!(
        (text.contains("the scenario's process group was killed")
            || text.contains("the scenario's process group was already empty"))
            && text.contains("the scenario's group was empty"),
        "and the wrapper ended its group and observed it empty: {text}"
    );
    text
}

/// A scenario whose owner never answers fails at its own bound, and the
/// stopped child that owner still holds ends with the scenario's group: in a
/// process of its own
/// (`fail-at-the-bound-holding-a-stopped-child-on-a-worker-that-never-answers`),
/// a worker holds a parked fork whose child is stopped and never answers,
/// as the owner's worker under a first-bad mutation spins, and the scenario
/// fails at its bound and exits with the child held. The wrapper then kills
/// the scenario's group -- the scenario uncollected at the kill, so the
/// group id is still its own -- and the child, which could read neither a
/// release nor an EOF, has ended when the wrapper returns. This is the
/// failing path of the three interruption regressions above, held without a
/// mutation: a bounded failure that leaves no child (`PR320-R5-MAIN-004`,
/// `PR320-R5-REG-004`). Linux, for the reading of the child's state.
#[cfg(target_os = "linux")]
#[test]
fn a_scenario_whose_owner_never_answers_fails_at_its_bound_and_its_stopped_child_ends_with_the_group()
 {
    let (status, text) = run_parked_fork_scenario(
        "fail-at-the-bound-holding-a-stopped-child-on-a-worker-that-never-answers",
    );
    let child = descendants_named_in(&text, 1)
        .first()
        .copied()
        .expect("the scenario named its child");
    // Read before any assertion, so that a child the wrapper left is ended by
    // this test whichever assertion fails.
    let ended = has_ended_by_proc(child);
    if !ended {
        assert_ended_within(
            child,
            Duration::from_secs(5),
            "the stopped child the wrapper was to end with the scenario's group",
        );
    }
    assert_eq!(
        status.and_then(|status| status.code()),
        Some(101),
        "the scenario failed at its own bound, as the failing path it stands for does: \
         {status:?}\n{text}"
    );
    assert!(
        text.contains("the worker answered within its bound"),
        "with its own bounded failure: {text}"
    );
    assert!(
        text.contains("the scenario's process group was killed")
            && text.contains("the scenario's group was empty"),
        "the wrapper killed the scenario's group and observed it empty: {text}"
    );
    assert!(
        ended,
        "the stopped child the stuck worker held had ended when the wrapper returned: {text}"
    );
}

/// A parked fork's drop whose kill and every write to stderr a policy
/// refuses -- the kill `EPERM`, the writes `EINTR`, installed on the
/// dropping thread after the child is stopped and acknowledged, and each
/// seen in force before the drop -- returns within its bounds: it spends the
/// reap bound, meets the refused kill, tries its line once and gives up on
/// it, never making the interrupted write again (`PR320-R5-MAIN-001`,
/// `PR320-R5-REG-001`). The line reaches no one, and the child is left
/// stopped, as a refused kill leaves it; the wrapper's kill of the
/// scenario's group ends it. In a process of its own
/// (`drop-a-parked-fork-whose-kill-and-stderr-writes-are-refused`): a drop
/// that retried would spin on a thread nothing can unblock. Linux, for the
/// policy.
#[cfg(target_os = "linux")]
#[test]
fn a_parked_forks_drop_whose_kill_and_every_stderr_write_are_refused_returns_within_its_bounds() {
    let text = assert_the_scenario_ran_to_its_end(
        "drop-a-parked-fork-whose-kill-and-stderr-writes-are-refused",
    );
    let pid = descendants_named_in(&text, 1)
        .first()
        .copied()
        .expect("the scenario named its fork");
    assert!(
        text.lines().any(|line| line.ends_with("left=T")),
        "the refused kill left the child stopped after the drop: {text}"
    );
    assert!(
        !text.contains(&format!("[the parked child {pid} ")),
        "the drop's line met the refused writes and reached no one: {text}"
    );
    assert_ended_within(
        pid,
        Duration::from_secs(5),
        "the stopped child the refused kill left, which the wrapper's kill of the scenario's \
         group ends",
    );
}

/// A parked fork's drop whose kill a policy refuses and whose every write to
/// stderr answers `EIO` neither panics nor aborts: dropped plainly it
/// returns within its bounds, and dropped by a caller already unwinding from
/// a failure of its own it lets that unwinding reach the caller's catch, a
/// second panic being an abort (`PR320-R5-MAIN-001`). In a process of its
/// own (`drop-parked-forks-whose-kill-is-refused-and-stderr-answers-eio`),
/// whose abort this test would read as a signal; both children are left
/// stopped by the refused kills and end with the scenario's group. Linux,
/// for the policy.
#[cfg(target_os = "linux")]
#[test]
fn a_parked_forks_drop_whose_stderr_answers_errors_neither_panics_nor_aborts_an_unwinding_caller() {
    let text = assert_the_scenario_ran_to_its_end(
        "drop-parked-forks-whose-kill-is-refused-and-stderr-answers-eio",
    );
    assert!(
        text.lines()
            .any(|line| line.contains("unwound and caught") && line.ends_with("left=T T")),
        "both drops returned, the second through the caught unwinding, and left their children \
         stopped: {text}"
    );
    for pid in descendants_named_in(&text, 2) {
        assert_ended_within(
            pid,
            Duration::from_secs(5),
            "a stopped child a refused kill left, which the wrapper's kill of the scenario's \
             group ends",
        );
    }
}

/// A scenario owner's drop whose kill and every write to stderr a policy
/// refuses (`EPERM`, `EINTR`) returns within its bounds: the leader, which
/// exited by itself, is collected, the refusal noted, and the line the drop
/// tries once reaches no one; the print macro it replaces retried the
/// interrupted write forever (`PR320-R5-MAIN-001`, `PR320-R5-REG-001`). In a
/// process of its own
/// (`drop-a-scenario-owner-whose-kill-and-stderr-writes-are-refused`), the
/// member of the owned group ended first so that nothing is left whatever
/// the drop does. Linux, for the policy.
#[cfg(target_os = "linux")]
#[test]
fn a_scenario_owners_drop_whose_kill_and_every_stderr_write_are_refused_returns_within_its_bounds()
{
    let text = assert_the_scenario_ran_to_its_end(
        "drop-a-scenario-owner-whose-kill-and-stderr-writes-are-refused",
    );
    let leader = text
        .lines()
        .find_map(|line| line.strip_prefix("leader="))
        .and_then(|pid| pid.trim().parse::<libc::pid_t>().ok())
        .expect("the scenario named the leader its owner held");
    assert!(
        text.contains(&format!("dropped the owner of {leader} after")),
        "the drop returned: {text}"
    );
    assert!(
        !text.contains(&format!("[the scenario process {leader} was dropped")),
        "the drop's line met the refused writes and reached no one: {text}"
    );
}

/// A scenario owner's drop whose kill a policy refuses and whose every write
/// to stderr answers `EIO` does not panic: the print macro it replaces
/// panicked on the failed write (`PR320-R5-REG-002`, `PR320-R5-MAIN-001`).
/// In a process of its own
/// (`drop-a-scenario-owner-whose-kill-is-refused-and-stderr-answers-eio`).
/// Linux, for the policy.
#[cfg(target_os = "linux")]
#[test]
fn a_scenario_owners_drop_whose_stderr_answers_an_error_does_not_panic() {
    let text = assert_the_scenario_ran_to_its_end(
        "drop-a-scenario-owner-whose-kill-is-refused-and-stderr-answers-eio",
    );
    assert!(
        text.contains("dropped the owner of"),
        "the drop returned: {text}"
    );
}

/// A scenario owner's drop run by the unwinding of a caller's own failure,
/// its kill refused by a policy and every write to stderr answering `EIO`,
/// lets that unwinding reach the caller's catch: the print macro it
/// replaces panicked on the failed write, and a panic while unwinding
/// aborted the process (`PR320-R5-REG-002`, `PR320-R5-MAIN-001`). In a
/// process of its own
/// (`unwind-through-a-scenario-owner-whose-kill-is-refused-and-stderr-answers-eio`),
/// whose abort this test reads as a signal. Linux, for the policy.
#[cfg(target_os = "linux")]
#[test]
fn a_scenario_owners_drop_whose_stderr_answers_an_error_does_not_abort_an_unwinding_caller() {
    let text = assert_the_scenario_ran_to_its_end(
        "unwind-through-a-scenario-owner-whose-kill-is-refused-and-stderr-answers-eio",
    );
    assert!(
        text.contains("unwound through the owner of") && text.contains("and caught it"),
        "the unwinding was caught: {text}"
    );
}

/// A scenario owner's drop whose group observation a policy interrupts at
/// every open of what it lists -- every open but a directory's answering
/// `EINTR` on the dropping thread, the refusal seen in force first --
/// returns within its bounds and says what it could and could not do: the
/// group killed, the leader collected with its status 23, and the group
/// neither observed empty nor read as empty, its members not accounted for.
/// `File::open` made the interrupted open again inside itself, and the drop
/// never came back to its bound (`PR320-R5-MAIN-002`). The drop's own kill
/// is made, so the member has ended when the drop returns. In a process of
/// its own (`drop-a-scenario-owner-whose-group-observation-opens-are-interrupted`).
/// Linux, for the policy and `/proc`.
#[cfg(target_os = "linux")]
#[test]
fn a_scenario_owners_drop_whose_group_observation_opens_are_interrupted_returns_and_says_the_group_is_unaccounted_for()
 {
    let text = assert_the_scenario_ran_to_its_end(
        "drop-a-scenario-owner-whose-group-observation-opens-are-interrupted",
    );
    let leader = text
        .lines()
        .find_map(|line| line.strip_prefix("leader="))
        .and_then(|pid| pid.trim().parse::<libc::pid_t>().ok())
        .expect("the scenario named the leader its owner held");
    assert!(
        text.contains(&format!(
            "[the scenario process {leader} was dropped by its owner with its cleanup \
             incomplete: the group was killed; the process was collected (exit status: 23); \
             the group was not observed empty within the bound:"
        )),
        "the drop said the kill, the collection and a group it did not observe empty: {text}"
    );
    assert!(
        text.contains(
            "[the scenario's group could not be observed: Interrupted system call (os error \
             4); its members are not accounted for]"
        ),
        "with the interrupted observation as it was: {text}"
    );
    for member in descendants_named_in(&text, 1) {
        assert_ended_within(
            member,
            Duration::from_secs(5),
            "the member the drop's own kill reached",
        );
    }
}

/// A parked fork's drop whose every rest a policy refuses --
/// `clock_nanosleep` answering `EINTR` on the dropping thread, the refusal
/// seen in force first -- still comes back to its bound between
/// observations: it spends the reap bound, kills the stopped child and
/// collects it, and returns. `std::thread::sleep` made the refused rest
/// again inside itself and the drop never reached the kill
/// (`PR320-R5-MAIN-006`). In a process of its own
/// (`drop-a-stopped-parked-fork-whose-every-rest-is-refused`), a drop that
/// never came back being a thread nothing can unblock. Linux, for the
/// policy.
#[cfg(target_os = "linux")]
#[test]
fn a_parked_forks_drop_whose_every_rest_is_refused_kills_and_collects_the_stopped_child_within_its_bounds()
 {
    let text = assert_the_scenario_ran_to_its_end(
        "drop-a-stopped-parked-fork-whose-every-rest-is-refused",
    );
    assert!(
        text.lines()
            .any(|line| line.starts_with("dropped after") && line.ends_with("the child collected")),
        "the drop killed and collected the child within its bounds: {text}"
    );
}

/// The scenario wrapper whose every rest a policy refuses still ends its
/// scenario at its bound: its loop comes back to the bound after every
/// refused rest, kills the group and accounts for it, and returns
/// (`PR320-R5-MAIN-006`, the rests of the scenario owner's own loops). In a
/// process of its own (`run-a-scenario-wrapper-whose-every-rest-is-refused`),
/// the wrapper's scenario spawned before the policy so that it is not the
/// policy's. Linux, for the policy.
#[cfg(target_os = "linux")]
#[test]
fn a_scenario_wrapper_whose_every_rest_is_refused_still_ends_its_scenario_at_its_bound() {
    let text =
        assert_the_scenario_ran_to_its_end("run-a-scenario-wrapper-whose-every-rest-is-refused");
    assert!(
        text.contains("the wrapper ended its scenario after"),
        "the wrapper returned: {text}"
    );
}

/// A parked fork's drop whose release a policy refuses -- the send answering
/// `EINTR` on the dropping thread, the refusal seen in force first -- with
/// the child stopped, so that no EOF can release it instead, kills the child
/// at the end of the reap bound and collects it, and says so: the release
/// could not be written, and the child was killed and collected, with its
/// status. It says nothing of a child left uncollected, which it is not; a
/// drop that read the write's failure as the collection's said so
/// (`PR320-R5-MAIN-003`, `PR320-R5-REG-003`). In a process of its own
/// (`drop-a-stopped-parked-fork-whose-release-send-is-interrupted`), whose
/// stderr this test reads. Linux, for the policy.
#[cfg(target_os = "linux")]
#[test]
fn a_parked_forks_drop_whose_release_send_is_interrupted_says_the_child_was_killed_and_collected() {
    let text = assert_the_scenario_ran_to_its_end(
        "drop-a-stopped-parked-fork-whose-release-send-is-interrupted",
    );
    let pid = descendants_named_in(&text, 1)
        .first()
        .copied()
        .expect("the scenario named its fork");
    assert!(
        text.contains(&format!(
            "[the parked child {pid} could not be released by its owner's drop: Interrupted \
             system call (os error 4); it was killed at the end of 100ms and collected: signal: \
             9 (SIGKILL)]"
        )),
        "the drop said the release failed and the child was killed and collected: {text}"
    );
    assert!(
        !text.contains("left uncollected"),
        "and never that the collected child was left uncollected: {text}"
    );
}

/// A parked fork's release whose send a policy refuses (`EINTR`), the child
/// stopped so that no EOF releases it, panics saying both what failed and
/// what was done: the release could not be written, and the child was
/// killed at the end of the reap bound and collected, with its status
/// (`PR320-R5-MAIN-003`, `PR320-R5-REG-003`). In a process of its own
/// (`release-a-stopped-parked-fork-whose-release-send-is-interrupted`).
/// Linux, for the policy.
#[cfg(target_os = "linux")]
#[test]
fn a_parked_forks_release_whose_send_is_interrupted_panics_saying_the_child_was_killed_and_collected()
 {
    let text = assert_the_scenario_ran_to_its_end(
        "release-a-stopped-parked-fork-whose-release-send-is-interrupted",
    );
    assert!(
        text.contains("released: release the parked child"),
        "the release's panic was caught and read: {text}"
    );
}

/// A progressing write ends at the first answer that is not progress:
/// `write_while_progressing` asks a writer that answers an interruption
/// once and hands the interruption back, asks one that answers an error
/// once, stops at a write that takes nothing, and writes on only while each
/// write takes something -- so an interrupted or refused channel costs one
/// write and is never asked again (`PR320-R5-MAIN-001`, `PR320-R5-REG-001`).
/// Each writer answers otherwise when asked a second time, so a retry shows
/// as a second attempt, never as a hang.
#[cfg(unix)]
#[test]
fn a_progressing_write_ends_at_the_first_answer_that_is_not_progress() {
    use crate::workspace_manager::fixture::write_while_progressing;

    let line = b"[a line an owner's drop says]\n";
    let mut attempts = 0_usize;
    let mut interrupted = Writing(|bytes: &[u8]| {
        attempts += 1;
        if attempts == 1 {
            Err(std::io::Error::from(std::io::ErrorKind::Interrupted))
        } else {
            Ok(bytes.len())
        }
    });
    assert_eq!(
        write_while_progressing(&mut interrupted, line).map_err(|error| error.kind()),
        Err(std::io::ErrorKind::Interrupted),
        "the interruption is the answer"
    );
    assert_eq!(attempts, 1, "the interrupted write was not made again");

    let mut attempts = 0_usize;
    let mut failing = Writing(|bytes: &[u8]| {
        attempts += 1;
        if attempts == 1 {
            Err(std::io::Error::from_raw_os_error(libc::EIO))
        } else {
            Ok(bytes.len())
        }
    });
    assert_eq!(
        write_while_progressing(&mut failing, line).map_err(|error| error.raw_os_error()),
        Err(Some(libc::EIO)),
        "the error is the answer"
    );
    assert_eq!(attempts, 1, "the failed write was not made again");

    let mut attempts = 0_usize;
    let mut nothing = Writing(|bytes: &[u8]| {
        attempts += 1;
        if attempts == 1 {
            Ok(0)
        } else {
            Ok(bytes.len())
        }
    });
    assert_eq!(
        write_while_progressing(&mut nothing, line).map_err(|error| error.kind()),
        Err(std::io::ErrorKind::WriteZero),
        "a write that took nothing is the end"
    );
    assert_eq!(attempts, 1, "and is not made again");

    let mut attempts = 0_usize;
    let mut taken = Vec::new();
    let mut short = Writing(|bytes: &[u8]| {
        attempts += 1;
        let piece = bytes.get(..3).unwrap_or(bytes);
        taken.extend_from_slice(piece);
        Ok(piece.len())
    });
    write_while_progressing(&mut short, line).expect("every write took something");
    assert_eq!(taken, line, "the whole line, in order");
    assert_eq!(
        attempts,
        line.len().div_ceil(3),
        "one write per piece of progress and no more"
    );
}

/// The release is one write of its byte and never a second: a writer that
/// answers `Interrupted` is asked once and the interruption is the answer,
/// one that answers the byte written is asked once, and one that answers
/// nothing written is a `WriteZero`. `write_all` would ask again on the
/// interruption inside itself, outside the reap bound the owner keeps
/// (`PR320-R4-REG-002`). The actual owner meets a refused send in processes
/// of their own
/// (`a_parked_forks_drop_whose_release_send_is_interrupted_says_the_child_was_killed_and_collected`,
/// `a_parked_forks_release_whose_send_is_interrupted_panics_saying_the_child_was_killed_and_collected`);
/// here the count of attempts is driven through the writer, the interrupted
/// case on a thread of its own so that a writer asked forever fails this
/// test rather than hangs it.
#[cfg(unix)]
#[test]
fn a_release_write_is_one_attempt_whatever_the_writer_answers() {
    use crate::workspace_manager::fixture::write_release;

    let (answered, writing) = on_a_thread_within(Duration::from_secs(10), || {
        let mut attempts = 0_u32;
        let mut interrupted = Writing(|_: &[u8]| {
            attempts += 1;
            Err(std::io::Error::from(std::io::ErrorKind::Interrupted))
        });
        let answer = write_release(&mut interrupted).map_err(|error| error.kind());
        (answer, attempts)
    });
    let (answer, attempts) = answered.expect(
        "the release write returned; one that asks an interrupted writer again forever is the \
         defect this test holds",
    );
    writing.join().expect("the writing thread ends");
    assert_eq!(answer, Err(std::io::ErrorKind::Interrupted));
    assert_eq!(attempts, 1, "the interrupted write was not made again");
    let mut attempts = 0_u32;
    let mut written = Writing(|bytes: &[u8]| {
        attempts += 1;
        Ok(bytes.len())
    });
    write_release(&mut written).expect("the byte written is the release");
    assert_eq!(attempts, 1, "one write");
    let mut nothing = Writing(|_: &[u8]| Ok(0));
    assert_eq!(
        write_release(&mut nothing)
            .expect_err("nothing written is not a release")
            .kind(),
        std::io::ErrorKind::WriteZero
    );
}

/// A test that unwinds with a parked fork in scope leaves no child behind:
/// the unwinding drops the fork, which releases and collects its child.
#[cfg(unix)]
#[test]
fn a_test_that_unwinds_past_a_parked_fork_leaves_no_child_behind() {
    use crate::workspace_manager::fixture::ParkedFork;
    use std::os::fd::AsRawFd as _;

    let anchor = File::open("/dev/null").expect("a descriptor for the fork to keep");
    let pid = std::cell::Cell::new(0);
    let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let parked = ParkedFork::holding(anchor.as_raw_fd());
        pid.set(parked.pid());
        assert!(parked.is_alive(), "the parked fork is alive");
        panic!("an assertion made with the fork in scope fails");
    }));
    assert!(unwound.is_err(), "the closure unwound");
    assert_ne!(pid.get(), 0, "the fork was made before the unwinding");
    assert!(
        is_reaped(pid.get()),
        "the unwinding dropped the fork, which released and collected its child {}",
        pid.get()
    );
}

/// One BPF instruction of a seccomp policy.
#[cfg(target_os = "linux")]
fn seccomp_instruction(code: u32, jt: u8, jf: u8, k: u32) -> libc::sock_filter {
    libc::sock_filter {
        code: u16::try_from(code).expect("a BPF instruction class fits the kernel's field"),
        jt,
        jf,
        k,
    }
}

/// Load the word at `offset` of the `seccomp_data`: 0 is the syscall
/// number, 16 the low word of the first argument on a little-endian machine.
#[cfg(target_os = "linux")]
fn seccomp_load(offset: u32) -> libc::sock_filter {
    seccomp_instruction(libc::BPF_LD | libc::BPF_W | libc::BPF_ABS, 0, 0, offset)
}

/// Jump `jt` instructions when the loaded word equals `k`, `jf` otherwise.
#[cfg(target_os = "linux")]
fn seccomp_jump_if_equal(k: u32, jt: u8, jf: u8) -> libc::sock_filter {
    seccomp_instruction(libc::BPF_JMP | libc::BPF_JEQ | libc::BPF_K, jt, jf, k)
}

/// Answer the syscall with `k`: `SECCOMP_RET_ALLOW`, or `SECCOMP_RET_ERRNO`
/// carrying the errno.
#[cfg(target_os = "linux")]
fn seccomp_return(k: u32) -> libc::sock_filter {
    seccomp_instruction(libc::BPF_RET | libc::BPF_K, 0, 0, k)
}

/// A syscall number, descriptor or errno as the word a policy compares.
#[cfg(target_os = "linux")]
fn seccomp_word(value: libc::c_long) -> u32 {
    u32::try_from(value).expect("a syscall number, descriptor or errno fits the kernel's field")
}

/// Install `program` as this thread's seccomp policy, the way
/// `agent::proc`'s termination tests install theirs: `PR_SET_NO_NEW_PRIVS`,
/// then `PR_SET_SECCOMP` in filter mode. A policy is the thread's and every
/// process it forks, and it cannot be removed; libtest runs each test on a
/// thread of its own, so it ends with the test.
#[cfg(target_os = "linux")]
fn install_seccomp_policy(program: &mut [libc::sock_filter]) {
    const NONE: libc::c_long = 0;
    const NO_NEW_PRIVS: libc::c_long = 1;

    let filter = libc::sock_fprog {
        len: u16::try_from(program.len()).expect("the program fits the count"),
        filter: program.as_mut_ptr(),
    };
    // SAFETY: `prctl` takes its five arguments by value and reads through no
    // pointer for `PR_SET_NO_NEW_PRIVS`, which the kernel refuses unless the
    // remaining four are 1, 0, 0 and 0.
    let allowed = unsafe {
        libc::syscall(
            libc::SYS_prctl,
            libc::c_long::from(libc::PR_SET_NO_NEW_PRIVS),
            NO_NEW_PRIVS,
            NONE,
            NONE,
            NONE,
        )
    };
    assert_eq!(
        allowed,
        0,
        "PR_SET_NO_NEW_PRIVS: {}",
        std::io::Error::last_os_error()
    );
    // SAFETY: `PR_SET_SECCOMP` reads the `sock_fprog` behind the third
    // argument and the instructions behind that program's own pointer; both
    // live for the call and the kernel copies them.
    let installed = unsafe {
        libc::syscall(
            libc::SYS_prctl,
            libc::c_long::from(libc::PR_SET_SECCOMP),
            libc::c_long::from(libc::SECCOMP_MODE_FILTER),
            std::ptr::from_ref(&filter),
            NONE,
            NONE,
        )
    };
    assert_eq!(
        installed,
        0,
        "PR_SET_SECCOMP: {}",
        std::io::Error::last_os_error()
    );
}

/// Refuse the syscall `number` for this thread and every process it forks,
/// answering `errno` instead: four instructions.
#[cfg(target_os = "linux")]
fn refuse_syscall_on_this_thread(number: libc::c_long, errno: libc::c_int) {
    let mut program = [
        seccomp_load(0),
        seccomp_jump_if_equal(seccomp_word(number), 0, 1),
        seccomp_return(libc::SECCOMP_RET_ERRNO | seccomp_word(libc::c_long::from(errno))),
        seccomp_return(libc::SECCOMP_RET_ALLOW),
    ];
    install_seccomp_policy(&mut program);
}

/// Refuse the `close` of `descriptor` for this thread and every process it
/// forks, answering `errno`, and refuse `close_range` with `ENOSYS` so that
/// a sweep reaches that close: eight instructions, the descriptor read from
/// the low word of the first argument.
#[cfg(target_os = "linux")]
fn refuse_closing_on_this_thread(descriptor: libc::c_int, errno: libc::c_int) {
    // `seccomp_data`: the number at 0, the architecture and the instruction
    // pointer, then the six 64-bit arguments from 16; the low word of the
    // first argument is at 16 on a little-endian machine and at 20 otherwise.
    let first_argument_low = if cfg!(target_endian = "little") {
        16
    } else {
        20
    };
    let mut program = [
        seccomp_load(0),
        seccomp_jump_if_equal(seccomp_word(libc::SYS_close_range), 0, 1),
        seccomp_return(libc::SECCOMP_RET_ERRNO | seccomp_word(libc::c_long::from(libc::ENOSYS))),
        seccomp_jump_if_equal(seccomp_word(libc::SYS_close), 0, 3),
        seccomp_load(first_argument_low),
        seccomp_jump_if_equal(seccomp_word(libc::c_long::from(descriptor)), 0, 1),
        seccomp_return(libc::SECCOMP_RET_ERRNO | seccomp_word(libc::c_long::from(errno))),
        seccomp_return(libc::SECCOMP_RET_ALLOW),
    ];
    install_seccomp_policy(&mut program);
}

/// A control that a policy is in force: `close_range` with a first
/// descriptor above its last is `EINVAL` from the kernel, so the errno it
/// answers here is only ever the policy's.
#[cfg(target_os = "linux")]
fn assert_close_range_answers(errno: libc::c_int) {
    // SAFETY: a raw syscall over a numeric range the kernel refuses as
    // empty; it closes nothing.
    let answered = unsafe { libc::syscall(libc::SYS_close_range, 3_u32, 2_u32, 0_u32) };
    let error = std::io::Error::last_os_error();
    assert!(
        answered == -1 && error.raw_os_error() == Some(errno),
        "close_range answers {}: {answered} / {error}",
        std::io::Error::from_raw_os_error(errno)
    );
}

/// Refuse this thread's writes to descriptor 2 with `errno`, and every
/// process it forks the same; a write to any other descriptor is made. Six
/// instructions, the descriptor read from the low word of the first
/// argument.
#[cfg(target_os = "linux")]
fn refuse_stderr_writes_on_this_thread(errno: libc::c_int) {
    let first_argument_low = if cfg!(target_endian = "little") {
        16
    } else {
        20
    };
    let mut program = [
        seccomp_load(0),
        seccomp_jump_if_equal(seccomp_word(libc::SYS_write), 0, 3),
        seccomp_load(first_argument_low),
        seccomp_jump_if_equal(seccomp_word(libc::c_long::from(libc::STDERR_FILENO)), 0, 1),
        seccomp_return(libc::SECCOMP_RET_ERRNO | seccomp_word(libc::c_long::from(errno))),
        seccomp_return(libc::SECCOMP_RET_ALLOW),
    ];
    install_seccomp_policy(&mut program);
}

/// Refuse this thread's opens of anything but a directory with `errno`, and
/// every process it forks the same: an `openat` whose flags lack
/// `O_DIRECTORY`. A listing's `opendir` is made, so a pass over `/proc`
/// meets the refusal at the opens of what it lists, which is where a retry
/// hid (`PR320-R5-MAIN-002`). Six instructions, the flags read from the low
/// word of the third argument.
#[cfg(target_os = "linux")]
fn refuse_nondirectory_opens_on_this_thread(errno: libc::c_int) {
    // `seccomp_data`: the number at 0, the architecture and the instruction
    // pointer, then the six 64-bit arguments from 16, so the third at 32; its
    // low word is at 32 on a little-endian machine and at 36 otherwise.
    let third_argument_low = if cfg!(target_endian = "little") {
        32
    } else {
        36
    };
    let mut program = [
        seccomp_load(0),
        seccomp_jump_if_equal(seccomp_word(libc::SYS_openat), 0, 3),
        seccomp_load(third_argument_low),
        seccomp_instruction(
            libc::BPF_JMP | libc::BPF_JSET | libc::BPF_K,
            1,
            0,
            seccomp_word(libc::c_long::from(libc::O_DIRECTORY)),
        ),
        seccomp_return(libc::SECCOMP_RET_ERRNO | seccomp_word(libc::c_long::from(errno))),
        seccomp_return(libc::SECCOMP_RET_ALLOW),
    ];
    install_seccomp_policy(&mut program);
}

/// A call an owner's thread is refused before the owner meets it, installed
/// on that thread alone and asked one call it must now refuse
/// ([`Refusal::in_force`]): a regression built on these enters the denial it
/// is about, and one not in force fails the regression rather than passing
/// it. Each is a seccomp policy of the thread and of every process it forks.
#[cfg(target_os = "linux")]
#[derive(Clone, Copy, Debug)]
enum Refusal {
    /// `kill` answers `EPERM`: the OS refuses every signal the thread sends.
    Kill,
    /// A write to descriptor 2 answers this errno; other writes are made.
    StderrWrites(libc::c_int),
    /// An `openat` without `O_DIRECTORY` answers `EINTR`; directories open.
    NondirectoryOpens,
    /// `clock_nanosleep` answers `EINTR`: every rest the thread takes is
    /// refused, before it has rested at all.
    Rests,
    /// `sendto` answers `EINTR`: a write to a Unix socket is refused.
    Sends,
}

#[cfg(target_os = "linux")]
impl Refusal {
    /// Install this refusal on this thread.
    fn install(self) {
        match self {
            Self::Kill => refuse_syscall_on_this_thread(libc::SYS_kill, libc::EPERM),
            Self::StderrWrites(errno) => refuse_stderr_writes_on_this_thread(errno),
            Self::NondirectoryOpens => refuse_nondirectory_opens_on_this_thread(libc::EINTR),
            Self::Rests => refuse_syscall_on_this_thread(libc::SYS_clock_nanosleep, libc::EINTR),
            Self::Sends => refuse_syscall_on_this_thread(libc::SYS_sendto, libc::EINTR),
        }
    }

    /// One call this refusal must now answer, made on this thread: `Ok` when
    /// it answered the refusal, and what it answered otherwise. Nothing here
    /// panics or prints: a thread whose stderr is refused has no channel for
    /// either.
    fn in_force(self) -> Result<(), String> {
        use std::io::Write as _;
        use std::os::fd::FromRawFd as _;

        match self {
            Self::Kill => {
                // SAFETY: signal 0 delivers nothing; the call only asks
                // whether this process may be signalled.
                let answered = unsafe { libc::kill(libc::getpid(), 0) };
                refused_with(answered == -1, libc::EPERM, "kill(getpid(), 0)")
            }
            Self::StderrWrites(errno) => {
                // SAFETY: a write of no bytes reads nothing from the buffer;
                // descriptor 2 is this process's standard error.
                let answered = unsafe { libc::write(libc::STDERR_FILENO, b"".as_ptr().cast(), 0) };
                refused_with(answered == -1, errno, "write(2, \"\", 0)")
            }
            Self::NondirectoryOpens => {
                // SAFETY: `open` reads the NUL-terminated name, a literal, and
                // takes the flags by value; nothing is created.
                let answered = unsafe {
                    libc::open(
                        c"/proc/self/stat".as_ptr(),
                        libc::O_RDONLY | libc::O_CLOEXEC,
                    )
                };
                let error = std::io::Error::last_os_error();
                if answered >= 0 {
                    // SAFETY: opened just now by this call and owned by
                    // nothing else; the `File` closes it on its drop.
                    drop(unsafe { File::from_raw_fd(answered) });
                    return Err(String::from(
                        "open(/proc/self/stat) was made: the refusal is not in force",
                    ));
                }
                if error.raw_os_error() != Some(libc::EINTR) {
                    return Err(format!(
                        "open(/proc/self/stat) answered {error}, not the refusal"
                    ));
                }
                fs::read_dir("/proc")
                    .map(drop)
                    .map_err(|error| format!("the listing's own open was refused too: {error}"))
            }
            Self::Rests => {
                let request = libc::timespec {
                    tv_sec: 0,
                    tv_nsec: 1,
                };
                // SAFETY: `nanosleep` reads the one `timespec`, which lives
                // for the call; the remainder pointer is null.
                let answered = unsafe { libc::nanosleep(&request, std::ptr::null_mut()) };
                refused_with(answered == -1, libc::EINTR, "nanosleep")
            }
            Self::Sends => {
                let (mut sender, _receiver) = std::os::unix::net::UnixStream::pair()
                    .map_err(|error| format!("a socket pair for the send: {error}"))?;
                match sender.write(&[1]) {
                    Err(error) if error.raw_os_error() == Some(libc::EINTR) => Ok(()),
                    other => Err(format!(
                        "a send on a Unix socket answered {other:?}, not the refusal"
                    )),
                }
            }
        }
    }
}

/// Whether a call that answered `failed` answered `errno`, read at once: `Ok`
/// for the refusal, and what the call answered otherwise.
#[cfg(target_os = "linux")]
fn refused_with(failed: bool, errno: libc::c_int, call: &str) -> Result<(), String> {
    let error = std::io::Error::last_os_error();
    match (failed, error.raw_os_error()) {
        (true, Some(answered)) if answered == errno => Ok(()),
        (true, _) => Err(format!("{call} answered {error}, not the refusal")),
        (false, _) => Err(format!("{call} was made: the refusal is not in force")),
    }
}

/// Install every refusal of `refusals` on this thread -- a refused stderr
/// last, so that nothing installed after it can panic into a channel it has
/// closed -- and ask each one call it must now refuse: the denial under test
/// is entered, not assumed.
#[cfg(target_os = "linux")]
fn refuse_on_this_thread(refusals: &[Refusal]) -> Result<(), String> {
    let (stderr, others): (Vec<Refusal>, Vec<Refusal>) = refusals
        .iter()
        .copied()
        .partition(|refusal| matches!(refusal, Refusal::StderrWrites(_)));
    for refusal in others.iter().chain(&stderr) {
        refusal.install();
    }
    for refusal in refusals {
        refusal
            .in_force()
            .map_err(|error| format!("{refusal:?}: {error}"))?;
    }
    Ok(())
}

/// Drop `owner` on a thread of its own under `refusals`, waiting for the drop
/// within `bound`: how long it took. The refusals bind the dropping thread
/// alone, the calling thread keeping every call, and each is asked one call
/// it must refuse before the drop, so the drop meets the denial under test.
/// Nothing on the dropping thread panics or prints -- a refused stderr would
/// make either a hang or a silence -- and the caller asserts instead.
///
/// # Panics
///
/// When a refusal was not in force, when the drop panicked, or when it did
/// not return within `bound`: the last is what a drop that makes a refused
/// call again inside itself does, and its thread is then left to end with
/// this process, whose group the scenario's wrapper kills once it has
/// exited.
#[cfg(target_os = "linux")]
fn drop_under<T: Send + 'static>(
    owner: T,
    refusals: &'static [Refusal],
    bound: Duration,
) -> Duration {
    let (answered, dropping) = on_a_thread_within(bound, move || {
        refuse_on_this_thread(refusals)?;
        let started = Instant::now();
        drop(owner);
        Ok::<Duration, String>(started.elapsed())
    });
    let took = settled_within(answered, bound, "the drop");
    dropping.join().expect("the dropping thread ends");
    took
}

/// Unwind through `owner` on a thread of its own under `refusals`: a failure
/// of that thread's own, raised with `owner` in scope, so that its drop runs
/// in the unwinding, and the unwinding caught -- whether it was, within
/// `bound`. A drop that panicked while unwinding would abort the process,
/// which the wrapper reads as a signal, never as a caught failure.
///
/// # Panics
///
/// As [`drop_under`].
#[cfg(target_os = "linux")]
fn unwind_through_under<T: Send + 'static>(
    owner: T,
    refusals: &'static [Refusal],
    bound: Duration,
) -> bool {
    let (answered, unwinding) = on_a_thread_within(bound, move || {
        refuse_on_this_thread(refusals)?;
        let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let _owner = owner;
            panic!("the failure an owner's drop runs in the unwinding of");
        }));
        Ok::<bool, String>(unwound.is_err())
    });
    let caught = settled_within(answered, bound, "the unwinding through the owner");
    unwinding.join().expect("the unwinding thread ends");
    caught
}

/// The answer of a thread [`on_a_thread_within`] ran under refusals, or the
/// panic that says why there is none: a refusal not in force, a thread that
/// ended without answering -- it panicked -- or `what` not done within
/// `bound`.
#[cfg(target_os = "linux")]
fn settled_within<T>(
    answered: Result<Result<T, String>, std::sync::mpsc::RecvTimeoutError>,
    bound: Duration,
    what: &str,
) -> T {
    match answered {
        Ok(Ok(answer)) => answer,
        Ok(Err(not_in_force)) => {
            panic!("a refusal {what} was to meet was not in force: {not_in_force}")
        }
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            panic!("{what} ended its thread without an answer: it panicked, which it must not")
        }
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => panic!(
            "{what} returned within {bound:?}; one that makes a refused or interrupted call again \
             inside itself is the defect this holds"
        ),
    }
}

/// The sentinel proof under a named condition: with this process's copy of
/// a socket end closed, the read on the other end answers EOF within the
/// bound, so the parked fork holds no copy of it, and its `/proc` table is
/// exactly stdio, the lease and its socket; the fork holds the lease, and is
/// released. Linux only, as its two callers are: the conditions are seccomp
/// policies, and a Unix-wide helper with Linux-only callers is dead code on
/// macOS, where `-D warnings` makes that a failed build.
#[cfg(target_os = "linux")]
fn assert_a_parked_fork_isolates_a_sentinel(tag: &str, under: &str) {
    use crate::workspace_manager::fixture::ParkedFork;
    use std::os::fd::AsRawFd as _;

    let root = scratch(tag);
    let public = public_dir(&root.join("repo"), "01CHILDLEASE000000000000008");
    fs::create_dir_all(&public).expect("the run's public directory");
    let (sentinel, mut observer) = sentinel_pair();
    let identity = sentinel_identity(sentinel.as_raw_fd());
    let parked = ParkedFork::holding_the_lease_of(&public);
    let holder = parked.pid();
    let [lease_number, socket_number] = parked.kept();
    assert!(
        parked.is_alive() && observe_cleanup_hold(&public, &mut NoHooks),
        "{under}: the parked fork {holder} holds the lease"
    );
    assert_the_forks_table_is_stdio_the_lease_and_its_socket(
        holder,
        lease_number,
        socket_number,
        under,
    );
    drop(sentinel);
    if let Err(held) = sentinel_closed_within(&mut observer, LEASE_RELEASE_BOUND, &mut |_| {}) {
        panic!(
            "{under}: with this process's copy of the sentinel closed the read answers EOF, so no \
             copy of it survives in fork {holder}: {held}; {}",
            sentinel_attribution(holder, &identity)
        );
    }
    let status = parked.release();
    assert!(
        status.success(),
        "{under}: the released fork exited cleanly: {status:?}"
    );
}

/// `close_range` unavailable -- `ENOSYS`, Linux before 5.9 -- makes the
/// sweep close by number instead, and the child is isolated all the same:
/// the sentinel proof under a policy that answers `ENOSYS` to `close_range`
/// for this thread and the child it forks.
#[cfg(target_os = "linux")]
#[test]
fn a_parked_fork_isolates_its_child_when_close_range_is_unavailable() {
    refuse_syscall_on_this_thread(libc::SYS_close_range, libc::ENOSYS);
    assert_close_range_answers(libc::ENOSYS);
    assert_a_parked_fork_isolates_a_sentinel(
        "childlease-close-range-enosys",
        "close_range unavailable",
    );
}

/// `close_range` refused -- `EPERM`, a policy's answer -- makes the sweep
/// close by number instead, and the child is isolated all the same.
#[cfg(target_os = "linux")]
#[test]
fn a_parked_fork_isolates_its_child_when_close_range_is_refused() {
    refuse_syscall_on_this_thread(libc::SYS_close_range, libc::EPERM);
    assert_close_range_answers(libc::EPERM);
    assert_a_parked_fork_isolates_a_sentinel("childlease-close-range-eperm", "close_range refused");
}

/// How long [`run_parked_fork_scenario`] gives the scenario process before
/// it kills it: a bound on a scenario that wedges, never a measure of one
/// that runs, which ends in well under a second.
#[cfg(unix)]
const SCENARIO_BOUND: Duration = Duration::from_secs(120);

/// How long each step of ending the scenario's group may take, once the
/// scenario process has ended or its bound has passed: the collection of the
/// killed leader, the observation of the group until no member is left, and
/// the drain of the stderr to EOF. A scenario that ends leaves no process of
/// its own behind, so each step ends at once; the bound is for a member the
/// scenario left stopped or parked, or a copy of the pipe held outside the
/// group.
#[cfg(unix)]
const SCENARIO_GROUP_BOUND: Duration = Duration::from_secs(2);

/// How many reads one turn of the stderr drain makes before it returns to
/// its caller with the pipe still readable, 4 KiB each: a bound on the work
/// a turn can do, so that a scenario writing without pause cannot keep the
/// owner inside the drain past the deadlines its caller checks between
/// turns ([`drain_turn`]).
#[cfg(unix)]
const DRAIN_TURN_READS: usize = 64;

/// Run `parked_fork_isolation_kill_child` in a process of its own with the
/// scenario named, bounded: its exit status and its stderr, or `None` and the
/// stderr so far once it has been killed at the end of [`SCENARIO_BOUND`].
/// Its stdin is `/dev/null`; it owns nothing of this process but the pipe.
/// The process is owned from its spawn ([`ScenarioChild`]): it leads a
/// process group of its own, so the processes it forks are the group's and a
/// kill reaches them; its stderr is read as it arrives, a bounded turn at a
/// time, and never joined; its exit is peeked before it is collected, so
/// that the group can be killed while the group id is still its own; and
/// once it has ended or run past its bound the group is killed, the leader
/// collected, the group observed until no member is left and the pipe
/// drained to EOF, each within [`SCENARIO_GROUP_BOUND`], the pipe's EOF being
/// read as what it is -- every copy closed -- and never as the group empty.
/// What the wrapper had to do, and what it could not, is appended to the
/// text it returns.
#[cfg(unix)]
fn run_parked_fork_scenario(scenario: &str) -> (Option<std::process::ExitStatus>, String) {
    run_parked_fork_scenario_within(scenario, SCENARIO_BOUND, |_| {})
}

/// [`run_parked_fork_scenario`] with `bound` in place of [`SCENARIO_BOUND`]
/// and `once_owned` run with the scenario process's pid the moment it is
/// owned: the seams through which the wrapper's own tests shorten its bound
/// and unwind through it.
#[cfg(unix)]
fn run_parked_fork_scenario_within(
    scenario: &str,
    bound: Duration,
    once_owned: impl FnOnce(libc::pid_t),
) -> (Option<std::process::ExitStatus>, String) {
    let mut owned = ScenarioChild::spawn(scenario);
    once_owned(owned.pid());
    let deadline = Instant::now() + bound;
    let ended = loop {
        owned.drain();
        if owned.has_ended() {
            break true;
        }
        if Instant::now() >= deadline {
            break false;
        }
        rest_within(
            Duration::from_millis(10),
            deadline.saturating_duration_since(Instant::now()),
        );
    };
    if !ended {
        owned.note(&format!("the scenario did not end within {bound:?}"));
    }
    // Ended and not yet collected, or past its bound: either way the
    // process's pid, and its group's id, is still its own, and the group is
    // ended and accounted for before the leader is collected.
    let status = owned.end(ended);
    (status, owned.text())
}

/// The scenario process and every process it forks, owned from the spawn.
/// The spawn puts it in a process group of its own, so a kill of the group
/// reaches what it forked -- a parked fork it left stopped, which cannot read
/// the EOF that would otherwise end it; its stderr is a pipe read without
/// blocking, a bounded turn at a time, so what it wrote so far is always in
/// hand and nothing is ever joined or retried; once the process has ended,
/// or run past its bound, its group is killed while the id is still its own,
/// the leader collected, the group observed until no member is left and the
/// pipe drained, each step within [`SCENARIO_GROUP_BOUND`] and each step's
/// failure -- a kill the OS refused, a leader not collected, a member or a
/// pipe copy that outlived the bound -- noted and never read as done; and
/// dropping this value with the process uncollected kills the group and
/// collects the process within the same bound, never panicking and never
/// waiting past it, so an unwinding caller leaves no child of this process
/// behind and no descendant of the scenario stopped or parked, and says on
/// stderr whatever of that it could not do, each state as it is. The kill
/// always precedes the collection: a collected pid is a number the kernel
/// may hand out again, and a group id with it. Every step is one attempt
/// whatever the kernel answers: every rest between two turns one
/// `nanosleep` a signal or a policy may cut short and nothing makes again
/// (`rest_within`, `PR320-R5-MAIN-006`), every open of the group's
/// observation one `open` (`open_once`, `PR320-R5-MAIN-002`), and the drop's
/// report said through `say_on_stderr`, which neither retries a refused
/// write nor panics on a failed one (`PR320-R5-MAIN-001`,
/// `PR320-R5-REG-001`, `PR320-R5-REG-002`).
#[cfg(unix)]
struct ScenarioChild {
    child: std::process::Child,
    stderr: std::process::ChildStderr,
    text: Vec<u8>,
    eof: bool,
    notes: String,
    collected: Option<std::process::ExitStatus>,
}

#[cfg(unix)]
impl ScenarioChild {
    /// Spawn the test binary on `parked_fork_isolation_kill_child` with
    /// `scenario` named, in a group of its own, its stderr piped and made
    /// non-blocking once the process is owned.
    fn spawn(scenario: &str) -> Self {
        use std::os::fd::AsRawFd as _;
        use std::os::unix::process::CommandExt as _;

        let exe = std::env::current_exe().expect("test binary");
        let mut child = std::process::Command::new(exe)
            .args([
                "--exact",
                "rundir::tests::parked_fork_isolation_kill_child",
                "--ignored",
                "--nocapture",
            ])
            .env("UPSTROKE_TEST_PARKED_FORK_SCENARIO", scenario)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .process_group(0)
            .spawn()
            .expect("spawn the scenario child");
        let stderr = child.stderr.take().expect("the child's stderr is piped");
        let owned = Self {
            child,
            stderr,
            text: Vec::new(),
            eof: false,
            notes: String::new(),
            collected: None,
        };
        // SAFETY: `fcntl` with `F_GETFL` reads the flags of this process's
        // own descriptor, the pipe's read end, and touches no memory. Made
        // once the process is owned, so a failure here unwinds through the
        // owner's drop.
        let flags = unsafe { libc::fcntl(owned.stderr.as_raw_fd(), libc::F_GETFL) };
        assert_ne!(
            flags,
            -1,
            "read the pipe's flags: {}",
            std::io::Error::last_os_error()
        );
        // SAFETY: `fcntl` with `F_SETFL` sets those flags and touches no
        // memory; the descriptor is this value's own.
        let set = unsafe {
            libc::fcntl(
                owned.stderr.as_raw_fd(),
                libc::F_SETFL,
                flags | libc::O_NONBLOCK,
            )
        };
        assert_ne!(
            set,
            -1,
            "make the pipe non-blocking: {}",
            std::io::Error::last_os_error()
        );
        owned
    }

    /// The scenario process's pid, which is also its group's id.
    fn pid(&self) -> libc::pid_t {
        libc::pid_t::try_from(self.child.id()).expect("a pid fits its type")
    }

    /// One turn of the stderr drain ([`drain_turn`]), the text kept and a
    /// failed read noted: `true` once the pipe has answered EOF or failed.
    /// Never retries: the caller's loop, and its deadline, decide whether
    /// another turn is made.
    fn drain(&mut self) -> bool {
        if self.eof {
            return true;
        }
        match drain_turn(&mut self.stderr, &mut self.text) {
            DrainTurn::Eof => self.eof = true,
            DrainTurn::Failed(error) => {
                self.note(&format!("the scenario's stderr could not be read: {error}"));
                self.eof = true;
            }
            DrainTurn::Drained | DrainTurn::Interrupted | DrainTurn::Budget => {}
        }
        self.eof
    }

    /// Drain until the pipe answers EOF or `bound` runs out: whether it did.
    fn drain_until_eof(&mut self, bound: Duration) -> bool {
        let deadline = Instant::now() + bound;
        loop {
            if self.drain() {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            rest_within(
                Duration::from_millis(10),
                deadline.saturating_duration_since(Instant::now()),
            );
        }
    }

    /// [`Self::drain_until_eof`], noting a pipe still open at the end of the
    /// bound: every process of the group is dead by then, so a copy still
    /// open is held outside it, which nothing here owns.
    fn drain_until_eof_or_note(&mut self, bound: Duration) {
        if !self.drain_until_eof(bound) {
            self.note(&format!(
                "the scenario's stderr did not answer EOF within {bound:?} of the group's kill: a \
                 process outside the group holds a copy"
            ));
        }
    }

    /// Whether the scenario process has ended, asked without collecting it
    /// (`waitid` with `WNOWAIT`), so that the answer leaves its pid, and its
    /// group's id, reserved for the kill that may follow.
    fn has_ended(&self) -> bool {
        // SAFETY: `zeroed` is a valid `siginfo_t`, and a zero `si_pid` after
        // the call is `WNOHANG`'s "nothing yet". `waitid` writes one
        // `siginfo_t` through the pointer, which lives for the call, and
        // takes the id and the flags by value; the pid is this value's own
        // child, uncollected.
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        let asked = unsafe {
            libc::waitid(
                libc::P_PID,
                libc::id_t::try_from(self.child.id()).expect("a pid fits an id"),
                &mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        assert_eq!(
            asked,
            0,
            "waitid(P_PID, {}, WEXITED | WNOHANG | WNOWAIT): {}",
            self.child.id(),
            std::io::Error::last_os_error()
        );
        // SAFETY: `si_pid` reads the union field a child report fills, and
        // the zeroing above makes it readable when nothing was reported.
        unsafe { info.si_pid() != 0 }
    }

    /// `SIGKILL` to the group, before any collection: `true` when the group
    /// is now dead or was already empty (`ESRCH`); a refusal by the OS is
    /// noted and answered `false`, and nothing here reads it as a kill.
    fn kill_group(&mut self) -> bool {
        match crate::workspace_manager::fixture::kill_process_group(&mut self.child) {
            Ok(()) => {
                self.note("the scenario's process group was killed");
                true
            }
            Err(error) if error.raw_os_error() == Some(libc::ESRCH) => {
                self.note("the scenario's process group was already empty");
                true
            }
            Err(error) => {
                self.note(&format!(
                    "the scenario's process group could not be killed: {error}"
                ));
                false
            }
        }
    }

    /// Collect the scenario process within `bound`, `try_wait` every 5 ms:
    /// its status, or `None` for a process still there at the end of the
    /// bound or one that could not be polled, either noted with the pid.
    /// Never `Child::wait`, which would hold the owner for as long as the
    /// process lives, a kill the OS refused included (`PR320-R3-MAIN-002`).
    fn collect_within(&mut self, bound: Duration) -> Option<std::process::ExitStatus> {
        if let Some(status) = self.collected {
            return Some(status);
        }
        let deadline = Instant::now() + bound;
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => {
                    self.collected = Some(status);
                    return Some(status);
                }
                Ok(None) => {}
                Err(error) => {
                    self.note(&format!(
                        "the scenario process {} could not be polled: {error}",
                        self.pid()
                    ));
                    return None;
                }
            }
            if Instant::now() >= deadline {
                self.note(&format!(
                    "the scenario process {} was not collected within {bound:?}",
                    self.pid()
                ));
                return None;
            }
            rest_within(
                Duration::from_millis(5),
                deadline.saturating_duration_since(Instant::now()),
            );
        }
    }

    /// End the scenario's group and account for it, the process having ended
    /// (`ended`) or run past its bound: the group is killed while the
    /// leader's id is still its own, the leader is collected, the group is
    /// observed until no member of it is left ([`group_ended`]) and the
    /// stderr is drained to EOF, each within [`SCENARIO_GROUP_BOUND`], and
    /// whatever a step could not do is noted. The status is the scenario's
    /// own, for a process that ended; a process killed at its bound has
    /// none, whatever its collection read. A kill the OS refused leaves a
    /// leader that has not ended uncollected and the pipe undrained, and
    /// says so, rather than waiting on either: the status is then `None` and
    /// the notes name the refusal and the pid. The pipe's EOF is never what
    /// says the group is empty: it says every copy of the pipe is closed,
    /// which a member that redirected or closed its stderr leaves true while
    /// it lives (`PR320-R3-MAIN-001`).
    fn end(&mut self, ended: bool) -> Option<std::process::ExitStatus> {
        let killed = self.kill_group();
        let status = if killed || ended {
            self.collect_within(SCENARIO_GROUP_BOUND).filter(|_| ended)
        } else {
            self.note(&format!(
                "the scenario process {} is left uncollected: alive, in a group that could not \
                 be killed",
                self.pid()
            ));
            None
        };
        if killed {
            self.await_group_end_or_note(SCENARIO_GROUP_BOUND);
            self.drain_until_eof_or_note(SCENARIO_GROUP_BOUND);
        } else {
            self.drain();
            if !self.eof {
                self.note("the scenario's stderr is left open: its holders could not be killed");
            }
        }
        status
    }

    /// Observe the scenario's group, killed a moment ago, every 10 ms until
    /// no member of it is left or `bound` runs out, and note which: whether
    /// it was found empty. A group that could not be observed is noted as
    /// not accounted for, never as empty.
    fn await_group_end_or_note(&mut self, bound: Duration) -> bool {
        let started = Instant::now();
        loop {
            match group_ended(self.pid()) {
                Ok(true) => {
                    self.note(&format!(
                        "the scenario's group was empty {:?} after the kill",
                        started.elapsed()
                    ));
                    return true;
                }
                Ok(false) => {}
                Err(error) => {
                    self.note(&format!(
                        "the scenario's group could not be observed: {error}; its members are \
                         not accounted for"
                    ));
                    return false;
                }
            }
            if started.elapsed() >= bound {
                self.note(&format!(
                    "a member of the scenario's group was still running {bound:?} after the kill"
                ));
                return false;
            }
            rest_within(
                Duration::from_millis(10),
                bound.saturating_sub(started.elapsed()),
            );
        }
    }

    fn note(&mut self, note: &str) {
        self.notes.push_str("\n[");
        self.notes.push_str(note);
        self.notes.push(']');
    }

    /// The stderr read so far and the notes, as text.
    fn text(&self) -> String {
        let mut text = String::from_utf8_lossy(&self.text).into_owned();
        text.push_str(&self.notes);
        text
    }
}

#[cfg(unix)]
impl Drop for ScenarioChild {
    fn drop(&mut self) {
        // Best effort by nature, as `ParkedFork`'s drop is: a caller that is
        // unwinding has nothing to answer to, and a panic here would abort
        // it. A process already collected has had its group killed, or the
        // kill refused and noted, before the collection (`end`); one not yet
        // collected is killed with its group, before the collection,
        // collected within the bound and its group observed until no member
        // is left -- or, after a kill the OS refused, polled once and left.
        // Whichever of those steps could not be done is said on stderr, the
        // one channel a drop has, each state as it is: a refused kill is a
        // refusal whether or not the leader could be collected, and a
        // collected leader is not the group ended (`PR320-R4-MAIN-003`,
        // `PR320-R4-REG-003`). Said through `say_on_stderr`, not a print
        // macro: the macro retries an interrupted write forever and panics on
        // a failed one, and a panic here while the caller unwinds is an abort
        // (`PR320-R5-MAIN-001`, `PR320-R5-REG-001`, `PR320-R5-REG-002`).
        if self.collected.is_some() {
            return;
        }
        let killed = self.kill_group();
        let bound = if killed {
            SCENARIO_GROUP_BOUND
        } else {
            Duration::ZERO
        };
        let status = self.collect_within(bound);
        let group_ended = killed && self.await_group_end_or_note(SCENARIO_GROUP_BOUND);
        if killed && status.is_some() && group_ended {
            return;
        }
        say_on_stderr(&format!(
            "[the scenario process {} was dropped by its owner with its cleanup incomplete: the \
             group {}; the process {}; the group {}:{}]\n",
            self.pid(),
            if killed {
                "was killed"
            } else {
                "could not be killed"
            },
            match status {
                Some(status) => format!("was collected ({status})"),
                None => String::from("is left uncollected"),
            },
            if !killed {
                "is not accounted for"
            } else if group_ended {
                "was empty"
            } else {
                "was not observed empty within the bound"
            },
            self.notes
        ));
    }
}

/// What ended one turn of a drain ([`drain_turn`]).
#[cfg(unix)]
#[derive(Debug)]
enum DrainTurn {
    /// The pipe answered EOF: every copy of its write end is closed.
    Eof,
    /// Nothing more to read now (`WouldBlock`).
    Drained,
    /// A handled signal ended a read (`Interrupted`): the turn ends, and the
    /// caller's deadline decides whether another is made.
    Interrupted,
    /// The turn's work budget ran out with the pipe still readable.
    Budget,
    /// A read failed otherwise, with this error.
    Failed(std::io::Error),
}

/// One turn of a non-blocking drain of `reader` into `text`: at most
/// [`DRAIN_TURN_READS`] reads of 4 KiB, ended early by EOF, by nothing more
/// to read, by an interruption or by an error, each returned to the caller
/// as what it was. Nothing is retried here: the caller's loop checks its own
/// deadline between turns, so no answer the pipe gives -- an interruption
/// repeated, or output that never pauses -- can hold the owner inside the
/// drain past that deadline (`PR320-R3-MAIN-003`). Any reader, so that the
/// answers are driven in an order a test chooses.
#[cfg(unix)]
fn drain_turn(reader: &mut impl std::io::Read, text: &mut Vec<u8>) -> DrainTurn {
    let mut buffer = [0_u8; 4096];
    for _ in 0..DRAIN_TURN_READS {
        match reader.read(&mut buffer) {
            Ok(0) => return DrainTurn::Eof,
            Ok(count) => text.extend_from_slice(buffer.get(..count).unwrap_or_default()),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                return DrainTurn::Drained;
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {
                return DrainTurn::Interrupted;
            }
            Err(error) => return DrainTurn::Failed(error),
        }
    }
    DrainTurn::Budget
}

/// Whether no process of the group `pgid` is left running, the group's
/// leader collected already: on Linux, read from `/proc` -- no process whose
/// group is `pgid` in a state other than dead-and-uncollected (`Z`), which is
/// ended for every purpose here, since a zombie holds no descriptor and runs
/// nothing, and a subreaper up the tree may keep the group's zombies until
/// it collects them. Every step of the pass is one attempt, never retried:
/// the listing's `opendir` and each `readdir`, each `stat`'s open
/// ([`open_once`] -- `File::open` makes an interrupted open again inside
/// itself, forever when every open is interrupted, `PR320-R5-MAIN-002`) and
/// its one `read`; so an observation a policy or a signal keeps interrupting
/// is a failed observation and not a wait, and the caller's bound is looked
/// at after every pass. A process gone between the listing and its read is
/// skipped.
///
/// # Errors
///
/// A `/proc` entry that could not be opened or read for a reason other than
/// its process having gone, an interruption included: the observation
/// failed, and the caller says so rather than reading the group as empty.
#[cfg(target_os = "linux")]
fn group_ended(pgid: libc::pid_t) -> std::io::Result<bool> {
    for entry in fs::read_dir("/proc")? {
        let entry = entry?;
        if !entry
            .file_name()
            .to_string_lossy()
            .bytes()
            .all(|byte| byte.is_ascii_digit())
        {
            continue;
        }
        let mut stat = match open_once(&entry.path().join("stat")) {
            Ok(stat) => stat,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) if error.raw_os_error() == Some(libc::ESRCH) => continue,
            Err(error) => return Err(error),
        };
        let mut buffer = [0_u8; 4096];
        let count = match stat.read(&mut buffer) {
            Ok(count) => count,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) if error.raw_os_error() == Some(libc::ESRCH) => continue,
            Err(error) => return Err(error),
        };
        let text = String::from_utf8_lossy(buffer.get(..count).unwrap_or_default());
        // `pid (comm) state ppid pgrp ...`: the comm may hold spaces and
        // parentheses, so the fields are read after the last `)`.
        let Some((_, after_comm)) = text.rsplit_once(')') else {
            continue;
        };
        let mut fields = after_comm.split_whitespace();
        let state = fields.next().unwrap_or("");
        let group = fields
            .nth(1)
            .and_then(|group| group.parse::<libc::pid_t>().ok());
        if group == Some(pgid) && state != "Z" && state != "X" {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Open `path` read-only, once: one `open(2)`, whatever it answers, and the
/// descriptor owned by the `File` it returns. `File::open` makes an
/// interrupted open again inside itself, so an observation whose opens a
/// policy or a signal keeps interrupting never came back to the bound its
/// caller keeps (`PR320-R5-MAIN-002`); this answers the interruption.
///
/// # Errors
///
/// What the open answered, an interruption included, as it was; a path
/// holding a NUL byte is `InvalidInput`, and no open is made.
#[cfg(target_os = "linux")]
fn open_once(path: &Path) -> std::io::Result<File> {
    use std::os::fd::FromRawFd as _;
    use std::os::unix::ffi::OsStrExt as _;

    let name = std::ffi::CString::new(path.as_os_str().as_bytes())
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?;
    // SAFETY: `open` reads the NUL-terminated name, which lives for the call,
    // and takes the flags by value; it creates nothing without `O_CREAT`.
    let descriptor = unsafe { libc::open(name.as_ptr(), libc::O_RDONLY | libc::O_CLOEXEC) };
    if descriptor < 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: `descriptor` was opened just now by this call and is owned by
    // nothing else; the `File` takes it and closes it once, on its drop.
    Ok(unsafe { File::from_raw_fd(descriptor) })
}

/// Whether no process of the group `pgid` is left, on the Unixes without
/// `/proc`: `kill` with signal 0 to the group, which delivers nothing and
/// answers `ESRCH` once no process of the group exists; a group the caller
/// may not signal (`EPERM`) exists. Orphans there go to `launchd`, which
/// collects them, so a killed member does not linger as a zombie of the
/// group.
///
/// # Errors
///
/// Any other answer than `0`, `ESRCH` or `EPERM`.
#[cfg(all(unix, not(target_os = "linux")))]
fn group_ended(pgid: libc::pid_t) -> std::io::Result<bool> {
    // SAFETY: signal 0 delivers nothing; the call only asks whether a
    // process of the group exists, and hands over no memory.
    if unsafe { libc::kill(-pgid, 0) } == 0 {
        return Ok(false);
    }
    let error = std::io::Error::last_os_error();
    match error.raw_os_error() {
        Some(libc::ESRCH) => Ok(true),
        Some(libc::EPERM) => Ok(false),
        _ => Err(error),
    }
}

/// Every pid a scenario wrote to its stderr as `descendant=<pid>`, at least
/// `expected` of them.
#[cfg(unix)]
fn descendants_named_in(text: &str, expected: usize) -> Vec<libc::pid_t> {
    let named: Vec<libc::pid_t> = text
        .lines()
        .filter_map(|line| line.strip_prefix("descendant="))
        .filter_map(|pid| pid.trim().parse().ok())
        .collect();
    assert!(
        named.len() >= expected,
        "the scenario named {expected} descendants before it was killed or exited: {text}"
    );
    named
}

/// Whether `pid` has ended, read from `/proc`: no entry, or one whose state
/// is `Z` -- dead, and waiting for the process it was reparented to, which is
/// not this one, to collect it.
#[cfg(target_os = "linux")]
fn has_ended_by_proc(pid: libc::pid_t) -> bool {
    match fs::read_to_string(format!("/proc/{pid}/stat")) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
        Err(error) => panic!("read /proc/{pid}/stat: {error}"),
        Ok(stat) => {
            stat.rsplit(')')
                .next()
                .and_then(|after_comm| after_comm.split_whitespace().next())
                == Some("Z")
        }
    }
}

/// The state letter of `pid` as `/proc` reads it -- `T` for a stopped
/// process, `S` or `R` for one that runs, `Z` for one ended and uncollected
/// -- or `gone` for a pid with no entry: what a scenario writes after its
/// drop, for the parent to read.
#[cfg(target_os = "linux")]
fn state_by_proc(pid: libc::pid_t) -> String {
    match fs::read_to_string(format!("/proc/{pid}/stat")) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::from("gone"),
        Err(error) => panic!("read /proc/{pid}/stat: {error}"),
        Ok(stat) => stat
            .rsplit(')')
            .next()
            .and_then(|after_comm| after_comm.split_whitespace().next())
            .unwrap_or("")
            .to_string(),
    }
}

/// Wait, bounded, for `pid` to have ended by [`has_ended_by_proc`]. A
/// process still there at the end of the bound is the wrapper's failure, and
/// it is killed before the test fails on it: it is not this process's child
/// to collect, but it is not to be left running on the machine either.
#[cfg(target_os = "linux")]
fn assert_ended_within(pid: libc::pid_t, bound: Duration, what: &str) {
    let started = Instant::now();
    while !has_ended_by_proc(pid) {
        if started.elapsed() >= bound {
            // SAFETY: `kill` takes a pid and a signal by value; the pid was
            // read from `/proc` as a live process a moment ago, the process
            // the scenario named.
            let killed = unsafe { libc::kill(pid, libc::SIGKILL) };
            panic!(
                "{what} {pid} was still running {bound:?} after the group was killed; killed by \
                 the test now (kill answered {killed})"
            );
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// A scenario that never ends is killed at the end of the wrapper's bound
/// with every process of its group: the bound shortened to a second, the
/// scenario parks a fork and stops it, spawns a child that holds its stderr,
/// and waits to be killed. The wrapper returns `None` and the stderr the
/// scenario wrote before the kill, within its own bounds and without joining
/// anything; both processes the scenario left behind are dead -- the stopped
/// fork, which could read neither a release nor an EOF, and the child that
/// ran on holding the pipe; the scenario process is collected. The wrapper
/// runs on a thread of its own with the test's failure bound around it, so a
/// wrapper that blocked would fail this test rather than hang it, the test
/// killing the group itself in that case so that the thread can end.
#[cfg(unix)]
#[test]
fn a_scenario_that_never_ends_is_killed_at_the_bound_with_the_fork_it_left_stopped() {
    let (pid_sender, pid_receiver) = std::sync::mpsc::channel();
    let (answered, running) = on_a_thread_within(Duration::from_secs(60), move || {
        let started = Instant::now();
        let answer = run_parked_fork_scenario_within(
            "park-a-stopped-fork-and-never-end",
            Duration::from_secs(1),
            |pid| pid_sender.send(pid).expect("the test waits for the pid"),
        );
        (answer, started.elapsed())
    });
    let scenario_pid = pid_receiver
        .recv_timeout(Duration::from_secs(60))
        .expect("the wrapper owned the scenario process and said which");
    if answered.is_err() {
        // SAFETY: `kill` takes a pid and a signal by value; the negative pid
        // is the scenario's own group, which the wrapper made. Killed by the
        // test only because the wrapper blocked past the test's bound, which
        // is the defect this test holds, so that the wrapper can finish and
        // the test fail rather than hang.
        assert_eq!(
            unsafe { libc::kill(-scenario_pid, libc::SIGKILL) },
            0,
            "kill the scenario's group {scenario_pid}: {}",
            std::io::Error::last_os_error()
        );
    }
    running.join().expect("the wrapper's thread ends");
    let ((status, text), took) = answered.expect(
        "the wrapper returned within the test's bound; a wrapper that blocks past its own bounds \
         is the defect this test holds",
    );
    assert!(
        status.is_none(),
        "a scenario killed at the bound has no status: {status:?}\n{text}"
    );
    assert!(
        took < Duration::from_secs(20),
        "and the wrapper returned within its own bounds: {took:?}\n{text}"
    );
    assert!(
        text.contains("the scenario did not end within 1s")
            && text.contains("the scenario's process group was killed"),
        "the wrapper says what it did: {text}"
    );
    let descendants = descendants_named_in(&text, 2);
    assert!(
        is_reaped(scenario_pid),
        "the scenario process {scenario_pid} was collected"
    );
    #[cfg(target_os = "linux")]
    for descendant in &descendants {
        assert_ended_within(
            *descendant,
            Duration::from_secs(5),
            "the process the scenario left behind",
        );
    }
    #[cfg(not(target_os = "linux"))]
    let _ = descendants;
}

/// A scenario whose process exits abnormally -- 23, its own cleanup never
/// run -- leaving a child that holds its stderr and runs on: the wrapper
/// returns that status and the stderr written before it, within its bounds,
/// and the group is killed once the scenario has ended, the pipe answering
/// EOF once its holder is dead, so the wrapper never waits on a process that
/// will not let go.
#[cfg(unix)]
#[test]
fn a_scenario_that_exits_leaving_a_child_holding_its_stderr_returns_its_status_and_kills_the_child()
{
    let (pid_sender, pid_receiver) = std::sync::mpsc::channel();
    let (answered, running) = on_a_thread_within(Duration::from_secs(60), move || {
        let started = Instant::now();
        let answer = run_parked_fork_scenario_within(
            "exit-23-leaving-a-child-holding-stderr",
            SCENARIO_BOUND,
            |pid| {
                pid_sender.send(pid).expect("the test waits for the pid");
            },
        );
        (answer, started.elapsed())
    });
    let scenario_pid = pid_receiver
        .recv_timeout(Duration::from_secs(60))
        .expect("the wrapper owned the scenario process and said which");
    if answered.is_err() {
        // SAFETY: as in the test above: the scenario's own group, killed by
        // the test only because the wrapper blocked past the test's bound.
        assert_eq!(
            unsafe { libc::kill(-scenario_pid, libc::SIGKILL) },
            0,
            "kill the scenario's group {scenario_pid}: {}",
            std::io::Error::last_os_error()
        );
    }
    running.join().expect("the wrapper's thread ends");
    let ((status, text), took) = answered.expect(
        "the wrapper returned within the test's bound; a wrapper that blocks past its own bounds \
         is the defect this test holds",
    );
    assert_eq!(
        status.and_then(|status| status.code()),
        Some(23),
        "the scenario's own exit status is returned: {status:?}\n{text}"
    );
    assert!(
        took < Duration::from_secs(20),
        "and the wrapper returned within its own bounds: {took:?}\n{text}"
    );
    assert!(
        text.contains("the scenario's process group was killed")
            && text.contains("the scenario's group was empty")
            && !text.contains("did not answer EOF"),
        "the wrapper says what it did: {text}"
    );
    let descendants = descendants_named_in(&text, 1);
    assert!(
        is_reaped(scenario_pid),
        "the scenario process {scenario_pid} was collected"
    );
    #[cfg(target_os = "linux")]
    for descendant in &descendants {
        assert_ended_within(
            *descendant,
            Duration::from_secs(5),
            "the child holding stderr",
        );
    }
    #[cfg(not(target_os = "linux"))]
    let _ = descendants;
}

/// A caller that unwinds after the scenario process is spawned leaves no
/// child behind: the process is owned from its spawn, and the unwinding
/// drops the owner, which kills the scenario's group and collects the
/// process.
#[cfg(unix)]
#[test]
fn a_panic_after_the_scenario_process_is_spawned_leaves_no_child_behind() {
    let pid = std::cell::Cell::new(0);
    let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_parked_fork_scenario_within(
            "park-a-stopped-fork-and-never-end",
            SCENARIO_BOUND,
            |owned| {
                pid.set(owned);
                panic!("a setup step after the spawn fails");
            },
        )
    }));
    assert!(unwound.is_err(), "the closure unwound");
    let pid = pid.get();
    assert_ne!(
        pid, 0,
        "the scenario process was spawned before the unwinding"
    );
    if !is_reaped(pid) {
        // Not this process's to leave behind: killed with its group and
        // collected before the test fails.
        // SAFETY: `kill` takes a pid and a signal by value; the negative pid
        // is the scenario's own group. `waitpid` writes one int through
        // `status`, which lives for the call.
        assert_eq!(
            unsafe { libc::kill(-pid, libc::SIGKILL) },
            0,
            "kill the scenario's group {pid}: {}",
            std::io::Error::last_os_error()
        );
        let mut status = 0;
        assert_eq!(
            unsafe { libc::waitpid(pid, &mut status, 0) },
            pid,
            "collect the scenario process {pid}: {}",
            std::io::Error::last_os_error()
        );
        panic!(
            "the unwinding dropped the scenario's owner, which kills and collects the scenario \
             process: {pid} was still this process's child"
        );
    }
}

/// A scenario whose process exits leaving a member of its group that holds
/// nothing of the scenario's -- its stderr sent to `/dev/null` -- so that the
/// pipe answers EOF the moment the scenario exits while the member runs on:
/// the wrapper returns the scenario's status only once the group has been
/// killed and observed empty, so the member has ended by the time the
/// wrapper returns, without any wait by this test. A pipe's EOF says every
/// copy of it is closed and nothing about the group
/// (`PR320-R3-MAIN-001`, `PR320-R3-REG-003`). Linux, for the `/proc` reading
/// of the member's state.
#[cfg(target_os = "linux")]
#[test]
fn a_scenario_that_exits_leaving_a_silent_member_in_its_group_has_the_group_ended_before_it_returns()
 {
    let (pid_sender, pid_receiver) = std::sync::mpsc::channel();
    let (answered, running) = on_a_thread_within(Duration::from_secs(60), move || {
        let started = Instant::now();
        let answer = run_parked_fork_scenario_within(
            "exit-23-leaving-a-silent-member-in-the-group",
            SCENARIO_BOUND,
            |pid| {
                pid_sender.send(pid).expect("the test waits for the pid");
            },
        );
        (answer, started.elapsed())
    });
    let scenario_pid = pid_receiver
        .recv_timeout(Duration::from_secs(60))
        .expect("the wrapper owned the scenario process and said which");
    if answered.is_err() {
        // SAFETY: as in the tests above: the scenario's own group, killed by
        // the test only because the wrapper blocked past the test's bound.
        assert_eq!(
            unsafe { libc::kill(-scenario_pid, libc::SIGKILL) },
            0,
            "kill the scenario's group {scenario_pid}: {}",
            std::io::Error::last_os_error()
        );
    }
    running.join().expect("the wrapper's thread ends");
    let ((status, text), took) = answered.expect(
        "the wrapper returned within the test's bound; a wrapper that blocks past its own bounds \
         is the defect this test holds",
    );
    let descendants = descendants_named_in(&text, 1);
    // The reading is made now, before any assertion, so that a member the
    // wrapper left running is ended by this test whichever assertion fails.
    let still_running: Vec<libc::pid_t> = descendants
        .iter()
        .copied()
        .filter(|pid| !has_ended_by_proc(*pid))
        .collect();
    for pid in &still_running {
        // SAFETY: `kill` takes a pid and a signal by value; the pid was read
        // from `/proc` as a live process a moment ago, the member the
        // scenario named, which the wrapper was to have ended.
        let killed = unsafe { libc::kill(*pid, libc::SIGKILL) };
        assert_ended_within(
            *pid,
            Duration::from_secs(5),
            &format!("the silent member, killed by the test (kill answered {killed})"),
        );
    }
    assert_eq!(
        status.and_then(|status| status.code()),
        Some(23),
        "the scenario's own exit status is returned: {status:?}\n{text}"
    );
    assert!(
        took < Duration::from_secs(20),
        "and the wrapper returned within its own bounds: {took:?}\n{text}"
    );
    assert!(
        text.contains("the scenario's process group was killed")
            && text.contains("the scenario's group was empty"),
        "the wrapper killed the group and observed it empty: {text}"
    );
    assert!(
        is_reaped(scenario_pid),
        "the scenario process {scenario_pid} was collected"
    );
    assert!(
        still_running.is_empty(),
        "the member the scenario left in its group had ended when the wrapper returned, its \
         stderr EOF notwithstanding; still running: {still_running:?}\n{text}"
    );
}

/// A group kill the OS refuses -- a policy answering `EPERM` to `kill`,
/// installed on the wrapper's own thread the moment it owns the scenario --
/// leaves the wrapper bounded: it returns within its own bounds with no
/// status, saying that the group could not be killed and that the process
/// is left alive and uncollected, and never waits on the process it could
/// not kill (`PR320-R3-MAIN-002`, `PR320-R3-REG-004`). The wrapper runs on a
/// thread of its own, which the policy binds, so this test's thread keeps
/// its `kill` and ends the group itself afterwards. Linux, for the policy.
#[cfg(target_os = "linux")]
#[test]
fn a_group_kill_the_os_refuses_leaves_the_wrapper_bounded_and_the_refusal_in_its_text() {
    let (pid_sender, pid_receiver) = std::sync::mpsc::channel();
    let (answered, running) = on_a_thread_within(Duration::from_secs(30), move || {
        let started = Instant::now();
        let answer = run_parked_fork_scenario_within(
            "hold-stderr-and-never-end",
            Duration::from_millis(250),
            |pid| {
                pid_sender.send(pid).expect("the test waits for the pid");
                refuse_syscall_on_this_thread(libc::SYS_kill, libc::EPERM);
            },
        );
        (answer, started.elapsed())
    });
    let scenario_pid = pid_receiver
        .recv_timeout(Duration::from_secs(60))
        .expect("the wrapper owned the scenario process and said which");
    let answer = answered;
    // Whatever the wrapper did, the group is this test's to end: this thread
    // has no policy. Killed before the join, so that a wrapper blocked in a
    // wait on the process it could not kill -- the defect this test holds --
    // is released and the test fails rather than hangs.
    // SAFETY: `kill` takes a pid and a signal by value; the negative pid is
    // the scenario's own group, which the wrapper made and could not kill.
    assert_eq!(
        unsafe { libc::kill(-scenario_pid, libc::SIGKILL) },
        0,
        "kill the scenario's group {scenario_pid}: {}",
        std::io::Error::last_os_error()
    );
    running.join().expect("the wrapper's thread ends");
    let mut status_word = 0;
    // SAFETY: `waitpid` writes one int through `status_word`, which lives
    // for the call; the pid is this process's child, which the wrapper
    // could not kill and so left uncollected.
    let collected = unsafe { libc::waitpid(scenario_pid, &mut status_word, 0) };
    let ((status, text), took) = answer.expect(
        "the wrapper returned within the test's bound although its kill was refused; a wrapper \
         that waits on a process it could not kill is the defect this test holds",
    );
    assert_eq!(
        collected,
        scenario_pid,
        "the scenario process was this test's to collect once its group was killed: {}",
        std::io::Error::last_os_error()
    );
    assert!(
        status.is_none(),
        "a scenario whose group could not be killed has no status: {status:?}\n{text}"
    );
    assert!(
        took < Duration::from_secs(5),
        "and the wrapper returned within its own bounds: {took:?}\n{text}"
    );
    assert!(
        text.contains("the scenario did not end within 250ms")
            && text.contains("the scenario's process group could not be killed")
            && text.contains("is left uncollected"),
        "the wrapper says what it could not do: {text}"
    );
}

/// Dropping a parked fork whose kill the OS refuses says so, and what is
/// left: in a process of its own (`drop-a-parked-fork-whose-kill-is-refused`),
/// the drop returns within its bounds and prints on stderr that the child
/// was left uncollected, with the refused kill, its errno and the child
/// alive at the last observation, and the stopped child is still there after
/// it -- which the wrapper's kill of the scenario's group then ends
/// (`PR320-R4-MAIN-003`). Linux, for the policy.
#[cfg(target_os = "linux")]
#[test]
fn dropping_a_parked_fork_whose_kill_the_os_refuses_says_so_and_what_is_left() {
    let (status, text) = run_parked_fork_scenario("drop-a-parked-fork-whose-kill-is-refused");
    let pid = descendants_named_in(&text, 1)
        .first()
        .copied()
        .expect("the scenario named its fork");
    assert!(
        status.is_some_and(|status| status.success()),
        "the scenario ran to its end: {status:?}\n{text}"
    );
    assert!(
        text.contains(&format!(
            "[the parked child {pid} was left uncollected by its owner's drop: kill the parked \
             child {pid} at the end of 100ms: Operation not permitted (os error 1); the child \
             is left uncollected, alive at the last observation]"
        )),
        "the drop said what it could not do and what is left: {text}"
    );
    assert!(
        text.lines().any(|line| line.ends_with("left=T")),
        "the stopped child was still there after the drop: {text}"
    );
    assert!(
        text.contains("the scenario's process group was killed")
            && text.contains("the scenario's group was empty"),
        "the wrapper ended the group the scenario left: {text}"
    );
}

/// Dropping a scenario owner whose group kill the OS refuses reports the
/// refusal although its leader was collected: in a process of its own
/// (`drop-a-scenario-owner-whose-group-kill-is-refused-after-its-leader-exited`),
/// the owner's leader has exited 23 leaving a silent member in its group,
/// the kill is refused, the leader is collected, and the drop prints on
/// stderr the refusal with its errno, the collection with its status and the
/// group not accounted for; the member is alive after the drop, and it is
/// this test that ends it -- it is in the dropped leader's group, which the
/// wrapper's kill of the scenario's group does not reach
/// (`PR320-R4-MAIN-003`, `PR320-R4-REG-003`). Linux, for the policy.
#[cfg(target_os = "linux")]
#[test]
fn dropping_a_scenario_owner_whose_group_kill_the_os_refuses_reports_it_although_its_leader_was_collected()
 {
    let (status, text) = run_parked_fork_scenario(
        "drop-a-scenario-owner-whose-group-kill-is-refused-after-its-leader-exited",
    );
    let members = descendants_named_in(&text, 1);
    // The reading is made now, before any assertion, so that the member the
    // refused kill left is ended by this test whichever assertion fails.
    let still_running: Vec<libc::pid_t> = members
        .iter()
        .copied()
        .filter(|pid| !has_ended_by_proc(*pid))
        .collect();
    for pid in &still_running {
        // SAFETY: `kill` takes a pid and a signal by value; the pid was read
        // from `/proc` as a live process a moment ago, the member the
        // scenario named, which the refused kill could not end.
        let killed = unsafe { libc::kill(*pid, libc::SIGKILL) };
        assert_ended_within(
            *pid,
            Duration::from_secs(5),
            &format!("the silent member, killed by the test (kill answered {killed})"),
        );
    }
    let leader: libc::pid_t = text
        .lines()
        .find_map(|line| line.strip_prefix("leader="))
        .and_then(|pid| pid.trim().parse().ok())
        .expect("the scenario named the leader it dropped");
    assert!(
        status.is_some_and(|status| status.success()),
        "the scenario ran to its end: {status:?}\n{text}"
    );
    assert!(
        text.contains(&format!(
            "[the scenario process {leader} was dropped by its owner with its cleanup \
             incomplete: the group could not be killed; the process was collected (exit \
             status: 23); the group is not accounted for:"
        )),
        "the drop said the refusal, the collection and the group's standing: {text}"
    );
    assert!(
        text.contains(
            "the scenario's process group could not be killed: Operation not permitted (os \
             error 1)"
        ),
        "with the refusal's errno: {text}"
    );
    assert!(
        text.lines().any(|line| {
            line.ends_with("left=S") || line.ends_with("left=R") || line.ends_with("left=D")
        }),
        "the member was alive after the drop: {text}"
    );
    assert_eq!(
        still_running.len(),
        1,
        "the member the refused kill could not end was still running when the wrapper \
         returned, and this test ended it: {still_running:?}\n{text}"
    );
}

/// A reader whose every read answers what `answer` says: the seam through
/// which the drain, the report read and the sentinel observation are driven
/// with the answers a pipe or a socket gives -- bytes, EOF, `WouldBlock`,
/// `TimedOut`, `Interrupted` -- in an order a test chooses, without a signal
/// or a policy to arrange them.
#[cfg(unix)]
struct Answering<F: FnMut(&mut [u8]) -> std::io::Result<usize>>(F);

#[cfg(unix)]
impl<F: FnMut(&mut [u8]) -> std::io::Result<usize>> std::io::Read for Answering<F> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        (self.0)(buffer)
    }
}

/// A writer whose every write answers what `answer` says: the seam through
/// which the release write is driven with the answers a socket gives -- the
/// byte taken, nothing taken, an interruption -- and its attempts counted.
#[cfg(unix)]
struct Writing<F: FnMut(&[u8]) -> std::io::Result<usize>>(F);

#[cfg(unix)]
impl<F: FnMut(&[u8]) -> std::io::Result<usize>> std::io::Write for Writing<F> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        (self.0)(bytes)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// A drain turn ends on the first interruption and hands the question of
/// another turn back to its caller: a pipe that answers `Interrupted` to
/// every read costs one read per turn, never a retry inside the drain
/// (`PR320-R3-MAIN-003`, `PR320-R3-REG-005`).
#[cfg(unix)]
#[test]
fn a_drain_turn_ends_on_an_interruption_and_leaves_the_deadline_to_its_caller() {
    let mut reads = 0_u32;
    let mut interrupted = Answering(|_: &mut [u8]| {
        reads += 1;
        Err(std::io::Error::from(std::io::ErrorKind::Interrupted))
    });
    let mut text = Vec::new();
    let turn = drain_turn(&mut interrupted, &mut text);
    assert!(
        matches!(turn, DrainTurn::Interrupted),
        "an interrupted read ends the turn as what it was: {turn:?}"
    );
    assert_eq!(reads, 1, "and the drain made no second read of its own");
    assert!(text.is_empty());
}

/// A drain turn has a work budget: a pipe that never pauses ends the turn
/// after `DRAIN_TURN_READS` reads with what they read kept, so the caller's
/// deadline is checked between turns however much the scenario writes.
#[cfg(unix)]
#[test]
fn a_drain_turn_with_output_that_never_pauses_ends_at_its_work_budget() {
    let mut reads = 0_usize;
    let mut endless = Answering(|buffer: &mut [u8]| {
        reads += 1;
        buffer.fill(b'x');
        Ok(buffer.len())
    });
    let mut text = Vec::new();
    let turn = drain_turn(&mut endless, &mut text);
    assert!(
        matches!(turn, DrainTurn::Budget),
        "the turn ended at its budget: {turn:?}"
    );
    assert_eq!(reads, DRAIN_TURN_READS);
    assert_eq!(
        text.len(),
        DRAIN_TURN_READS * 4096,
        "everything read was kept"
    );
    let mut ending = Answering(|buffer: &mut [u8]| {
        buffer.fill(b'y');
        Ok(2)
    });
    let mut text = Vec::new();
    let mut turns = 0;
    let turn = loop {
        turns += 1;
        match drain_turn(&mut ending, &mut text) {
            DrainTurn::Budget if turns < 3 => {}
            other => break other,
        }
    };
    assert!(matches!(turn, DrainTurn::Budget), "{turn:?}");
    assert_eq!(
        text.len(),
        3 * DRAIN_TURN_READS * 2,
        "three turns, each its budget of short reads"
    );
}

/// A drain turn ends on EOF, on nothing more to read, and on a failed read,
/// each returned as what it was with the bytes before it kept.
#[cfg(unix)]
#[test]
fn a_drain_turn_returns_eof_a_pause_and_a_failure_as_what_they_were() {
    let mut answers = vec![
        Err(std::io::Error::other("the pipe's read failed")),
        Err(std::io::Error::from(std::io::ErrorKind::WouldBlock)),
        Ok(0),
        Ok(3),
    ];
    let mut scripted = Answering(move |buffer: &mut [u8]| match answers.pop() {
        Some(Ok(count)) => {
            buffer
                .get_mut(..count)
                .expect("the answer fits the buffer")
                .fill(b'a');
            Ok(count)
        }
        Some(Err(error)) => Err(error),
        None => Err(std::io::Error::from(std::io::ErrorKind::WouldBlock)),
    });
    let mut text = Vec::new();
    assert!(matches!(
        drain_turn(&mut scripted, &mut text),
        DrainTurn::Eof
    ));
    assert_eq!(text, b"aaa", "the bytes before EOF were kept");
    assert!(matches!(
        drain_turn(&mut scripted, &mut text),
        DrainTurn::Drained
    ));
    match drain_turn(&mut scripted, &mut text) {
        DrainTurn::Failed(error) => assert_eq!(error.to_string(), "the pipe's read failed"),
        other => panic!("a failed read ends the turn with its error: {other:?}"),
    }
}

/// The report read keeps one deadline across interruptions: a socket that
/// answers `Interrupted` to every read is asked again until the deadline,
/// which no answer restarts, and the read then fails with `TimedOut`
/// (`PR320-R3-MAIN-004`). `read_exact` would retry inside itself, each
/// retry a fresh socket timeout.
#[cfg(unix)]
#[test]
fn a_report_read_keeps_one_deadline_across_interruptions() {
    use crate::workspace_manager::fixture::read_report_within;

    let mut reads = 0_u32;
    let mut interrupted = Answering(|_: &mut [u8]| {
        reads += 1;
        Err(std::io::Error::from(std::io::ErrorKind::Interrupted))
    });
    let bound = Duration::from_millis(100);
    let started = Instant::now();
    let error = read_report_within(&mut interrupted, started + bound)
        .expect_err("a report that never arrives is a timeout, not a wait");
    let took = started.elapsed();
    assert_eq!(error.kind(), std::io::ErrorKind::TimedOut, "{error}");
    assert!(
        took >= bound && took < bound * 20,
        "the deadline was kept, once: {took:?} against {bound:?} after {reads} interrupted reads"
    );
    assert!(reads > 1, "the interrupted read was made again: {reads}");
    assert!(
        error.to_string().contains("0 of 9 bytes read"),
        "the error says how much arrived: {error}"
    );
}

/// The report read keeps what a short read delivered: a socket answering one
/// byte at a time, with timeouts and interruptions between, assembles the
/// nine bytes; one that closes early fails with how much had arrived.
#[cfg(unix)]
#[test]
fn a_report_read_assembles_short_answers_and_reports_an_early_close() {
    use crate::workspace_manager::fixture::{REPORT_LEN, read_report_within};

    let report: Vec<u8> = (1..=REPORT_LEN).map(|byte| byte as u8).collect();
    let mut next = 0_usize;
    let mut interruptions = 0_u32;
    let mut byte_at_a_time = Answering(|buffer: &mut [u8]| {
        if interruptions < 2 * next as u32 + 1 {
            interruptions += 1;
            return Err(std::io::Error::from(if next % 2 == 0 {
                std::io::ErrorKind::Interrupted
            } else {
                std::io::ErrorKind::TimedOut
            }));
        }
        let byte = *report
            .get(next)
            .expect("the script ends at the report's length");
        *buffer.first_mut().expect("a buffer with room") = byte;
        next += 1;
        Ok(1)
    });
    let read = read_report_within(&mut byte_at_a_time, Instant::now() + Duration::from_secs(5))
        .expect("the report assembles from short reads");
    assert_eq!(read.to_vec(), report);
    assert_eq!(next, REPORT_LEN);
    let mut remaining = 3_usize;
    let mut early_close = Answering(|buffer: &mut [u8]| {
        if remaining == 0 {
            return Ok(0);
        }
        remaining -= 1;
        *buffer.first_mut().expect("a buffer with room") = 7;
        Ok(1)
    });
    let error = read_report_within(&mut early_close, Instant::now() + Duration::from_secs(5))
        .expect_err("a reader that closes before the report is an error");
    assert_eq!(error.kind(), std::io::ErrorKind::UnexpectedEof);
    assert!(
        error.to_string().contains("after 3 of 9 report bytes"),
        "{error}"
    );
}

/// The report read looks at its deadline before every turn, a successful
/// short read included: a reader answering one byte per read, under a clock
/// that advances thirty milliseconds a reading against a fifty millisecond
/// deadline, is asked twice and the third turn is the timeout, naming the
/// two bytes; a deadline consulted only on a read that failed would accept
/// all nine bytes at 270 ms (`PR320-R4-MAIN-002`, `PR320-R4-REG-001`).
#[cfg(unix)]
#[test]
fn a_report_read_looks_at_its_deadline_before_every_turn_a_successful_short_read_included() {
    use crate::workspace_manager::fixture::read_report_within_by;

    let origin = Instant::now();
    let mut readings = 0_u32;
    let mut reads = 0_u32;
    let mut one_byte = Answering(|buffer: &mut [u8]| {
        reads += 1;
        *buffer.first_mut().expect("a buffer with room") = 7;
        Ok(1)
    });
    let error = read_report_within_by(&mut one_byte, origin + Duration::from_millis(50), || {
        let now = origin + Duration::from_millis(30) * readings;
        readings += 1;
        now
    })
    .expect_err("a report still arriving when the deadline passes is a timeout");
    assert_eq!(error.kind(), std::io::ErrorKind::TimedOut, "{error}");
    assert_eq!(
        reads, 2,
        "two reads were made before the deadline was found passed"
    );
    assert_eq!(
        readings, 3,
        "the clock was read at the top of each of the three turns"
    );
    assert!(
        error
            .to_string()
            .contains("2 of 9 bytes read; the last read delivered bytes"),
        "the error says how much arrived and what the last read answered: {error}"
    );
}

/// A report complete only after its deadline is not accepted: a reader that
/// answers the nine bytes in one read, under a clock that reads past the
/// deadline on the completion turn, is a timeout naming nine of nine bytes,
/// and the same reader under a clock that has not passed it is the report.
/// One deadline, on completion as on progress.
#[cfg(unix)]
#[test]
fn a_report_complete_only_after_its_deadline_is_not_accepted() {
    use crate::workspace_manager::fixture::{REPORT_LEN, read_report_within_by};

    let origin = Instant::now();
    let deadline = origin + Duration::from_millis(50);
    let whole = |buffer: &mut [u8]| {
        assert_eq!(
            buffer.len(),
            REPORT_LEN,
            "the first read is offered the whole report"
        );
        buffer.fill(7);
        Ok(REPORT_LEN)
    };
    let mut readings = 0_u32;
    let error = read_report_within_by(&mut Answering(whole), deadline, || {
        readings += 1;
        // Before the deadline at the first turn, before the read; past it at
        // the second, the completion turn.
        origin + Duration::from_millis(60) * (readings - 1)
    })
    .expect_err("a report complete only after the deadline is not accepted");
    assert_eq!(error.kind(), std::io::ErrorKind::TimedOut, "{error}");
    assert_eq!(readings, 2, "the clock was read at both turns");
    assert!(
        error.to_string().contains("9 of 9 bytes read"),
        "the error says the whole report had arrived, late: {error}"
    );
    let report = read_report_within_by(&mut Answering(whole), deadline, || origin)
        .expect("a report complete before the deadline is accepted");
    assert_eq!(report, [7; REPORT_LEN]);
}

/// The same deadline on a real socket with the constructor's own tick: a
/// peer writing one report byte every sixty milliseconds against a two
/// hundred millisecond deadline is timed out with the bytes that had
/// arrived, never accepted at the 540 ms the nine take; the peer is joined
/// before any assertion. How late the timeout is found is the box's, not the
/// read's, and is bounded here only by twenty times the deadline.
#[cfg(unix)]
#[test]
fn a_report_arriving_slower_than_its_deadline_on_a_real_socket_is_timed_out_with_what_arrived() {
    use crate::workspace_manager::fixture::{READY_TICK, REPORT_LEN, read_report_within};

    let (mut reader, mut writer) =
        std::os::unix::net::UnixStream::pair().expect("a socket pair for the report");
    reader
        .set_read_timeout(Some(READY_TICK))
        .expect("the constructor's tick, set while both ends are open");
    let sending = std::thread::spawn(move || {
        for byte in 1..=REPORT_LEN {
            std::thread::sleep(Duration::from_millis(60));
            if writer.write_all(&[byte as u8]).is_err() {
                break;
            }
        }
    });
    let bound = Duration::from_millis(200);
    let started = Instant::now();
    let answer = read_report_within(&mut reader, started + bound);
    let took = started.elapsed();
    drop(reader);
    sending
        .join()
        .expect("the sending thread ends once its peer is gone");
    let error =
        answer.expect_err("nine bytes sixty milliseconds apart do not arrive within two hundred");
    assert_eq!(error.kind(), std::io::ErrorKind::TimedOut, "{error}");
    assert!(
        took >= bound && took < bound * 20,
        "the deadline was kept, once: {took:?} against {bound:?}"
    );
    let arrived: usize = error
        .to_string()
        .split(" of 9 bytes read")
        .next()
        .and_then(|head| head.rsplit(' ').next())
        .and_then(|count| count.parse().ok())
        .expect("the error says how many bytes arrived");
    assert!(
        arrived < REPORT_LEN,
        "fewer than the nine had arrived: {arrived}, {error}"
    );
}

/// An interrupted sentinel read is made again within the same bound: it
/// observed nothing, so it is neither counted nor acknowledged, and a socket
/// that answers `Interrupted` to every read ends in the bound's report with
/// zero observations, never a panic and never past the bound
/// (`PR320-R3-MAIN-005`); interruptions between timeouts and the EOF change
/// neither the count nor the acknowledgements.
#[cfg(unix)]
#[test]
fn an_interrupted_sentinel_read_is_made_again_within_the_same_bound() {
    let mut reads = 0_u32;
    let mut interrupted = Answering(|_: &mut [u8]| {
        reads += 1;
        Err(std::io::Error::from(std::io::ErrorKind::Interrupted))
    });
    let bound = Duration::from_millis(100);
    let mut acknowledged = Vec::new();
    let started = Instant::now();
    let held = sentinel_closed_within(&mut interrupted, bound, &mut |read| acknowledged.push(read))
        .expect_err("a sentinel that is never read as closed is reported at the bound");
    let took = started.elapsed();
    assert_eq!(
        held.observations, 0,
        "an interrupted read observed nothing: {held}"
    );
    assert!(
        held.waited >= bound && took < bound * 20,
        "the bound was kept, once, across {reads} interruptions: {took:?}"
    );
    assert!(reads > 1, "the interrupted read was made again: {reads}");
    assert!(
        acknowledged.is_empty(),
        "and nothing was acknowledged: {acknowledged:?}"
    );

    let mut answers = vec![
        Ok(0),
        Err(std::io::Error::from(std::io::ErrorKind::Interrupted)),
        Err(std::io::Error::from(std::io::ErrorKind::TimedOut)),
        Err(std::io::Error::from(std::io::ErrorKind::Interrupted)),
        Err(std::io::Error::from(std::io::ErrorKind::Interrupted)),
        Err(std::io::Error::from(std::io::ErrorKind::WouldBlock)),
    ];
    let mut scripted = Answering(move |_: &mut [u8]| {
        answers
            .pop()
            .expect("the script ends at EOF, which ends the observation")
    });
    let mut acknowledged = Vec::new();
    let (_, observations) =
        sentinel_closed_within(&mut scripted, Duration::from_secs(5), &mut |read| {
            acknowledged.push(read);
        })
        .expect("EOF ends the observation");
    assert_eq!(
        observations, 3,
        "two reads found a copy open and the third read EOF; the interruptions counted for nothing"
    );
    assert_eq!(
        acknowledged,
        vec![1, 2],
        "each read that found a copy open was acknowledged, once"
    );
}

/// A lookup that fails on a listed descriptor fails the identity scan with
/// the number and the error, in place of a scan that reads absence through
/// it: a failed metadata read is neither an absent descriptor nor a present
/// one (`PR320-R3-MAIN-006`, `PR320-R3-REG-001`). The lookup's own answer
/// for a number not open, `Ok(None)`, is what the scan skips.
#[cfg(unix)]
#[test]
fn a_lookup_that_fails_on_a_listed_descriptor_fails_the_identity_scan_instead_of_reading_absence() {
    use std::os::fd::AsRawFd as _;

    let root = scratch("descriptor-scan-lookup-fails");
    let target = root.join("target");
    let held = File::create(&target).expect("the target, held open by this thread");
    let number = held.as_raw_fd();
    assert_eq!(
        descriptors_open_on(&target),
        vec![number],
        "the real lookup reports the held descriptor"
    );
    let failing = |listed: libc::c_int| {
        if listed == number {
            Err(std::io::Error::from_raw_os_error(libc::EIO))
        } else {
            identity_of_the_descriptor(listed)
        }
    };
    let payload =
        std::panic::catch_unwind(|| descriptors_open_on_with(&target, &mut |_| {}, &failing))
            .expect_err("a lookup that failed fails the scan rather than answering through it");
    let message = payload
        .downcast_ref::<String>()
        .cloned()
        .expect("the panic carries a message");
    assert!(
        message.contains(&format!(
            "the identity of descriptor {number} could not be read"
        )) && message.contains("os error 5"),
        "the scan names the descriptor and the error: {message}"
    );
    let absent = |listed: libc::c_int| {
        if listed == number {
            Ok(None)
        } else {
            identity_of_the_descriptor(listed)
        }
    };
    assert!(
        descriptors_open_on_with(&target, &mut |_| {}, &absent).is_empty(),
        "a number the lookup answers as not open is skipped, which is the closed-in-window case"
    );
    drop(held);
}

/// The reading of a failed identity call is driven at the call itself, not
/// above it: through `identity_answered_by`, the real `fstat` on the held
/// number answers the target's identity, the real `fstat` on a number that
/// is never open (-1) answers absence, `EBADF`, and a call that fails with
/// any other errno -- a `waitpid` on this process's own pid, of which no
/// process is the child, so `ECHILD`, an errno the kernel set -- answers the
/// error and never absence. The shape the fourth reviews restored as a
/// mutation, every failed call answered as absence, fails here, where the
/// scan's own test above, driving the scan's three readings through an
/// injected lookup, could not see it (`PR320-R4-MAIN-004`,
/// `PR320-R4-REG-004`). No policy on `fstat` is needed and none is named:
/// the errno is real, and the reading is the one the real call's answer
/// goes through.
#[cfg(unix)]
#[test]
fn a_failed_identity_call_is_read_at_the_call_ebadf_as_absence_and_any_other_errno_as_the_error() {
    use std::os::fd::AsRawFd as _;

    let root = scratch("descriptor-identity-call-fails");
    let target = root.join("target");
    let held = File::create(&target).expect("the target, held open by this thread");
    let number = held.as_raw_fd();
    let expected = identity_at(&target).expect("the target's identity");
    let real = |number: libc::c_int, identity: *mut libc::stat| {
        // SAFETY: `fstat` takes the number by value and writes one `stat`
        // through the pointer, which the seam keeps alive for the call.
        unsafe { libc::fstat(number, identity) }
    };
    // SAFETY: the call is `fstat` itself, which keeps the seam's contract.
    let identity = unsafe { identity_answered_by(number, real) }
        .expect("the real fstat on a held number answers")
        .expect("with an identity");
    assert!(
        identity.st_dev == expected.st_dev && identity.st_ino == expected.st_ino,
        "the identity is the target's"
    );
    // SAFETY: as above; -1 is open in no process, so the call answers EBADF.
    assert!(
        unsafe { identity_answered_by(-1, real) }
            .expect("a number that is not open is answered")
            .is_none(),
        "and the answer is absence"
    );
    let own = libc::pid_t::try_from(std::process::id()).expect("a pid fits its type");
    let mut calls = 0_u32;
    // SAFETY: the call writes nothing through the pointer and answers -1:
    // `waitpid` writes one int through `status`, which lives for the call,
    // and this process is no child of itself, so it answers -1 with ECHILD.
    // The closure is inside this block, so the call is made under it.
    let error = unsafe {
        identity_answered_by(number, |_, _| {
            calls += 1;
            let mut status = 0;
            libc::waitpid(own, &mut status, libc::WNOHANG)
        })
    }
    .expect_err("a call that failed otherwise than by EBADF is the error, never absence");
    assert_eq!(calls, 1, "the call was made once");
    assert_eq!(
        error.raw_os_error(),
        Some(libc::ECHILD),
        "the error is the call's own: {error}"
    );
    drop(held);
}

/// A descriptor above the soft limit -- opened, then the limit lowered
/// beneath it, as a test that narrows `RLIMIT_NOFILE` leaves one -- is
/// still closed by the parked fork's sweep, whose ceiling comes from the
/// table itself and not from `sysconf` alone. In a process of its own,
/// because the limit is the process's.
#[cfg(unix)]
#[test]
fn a_parked_fork_closes_a_descriptor_above_a_lowered_soft_limit() {
    let (status, stderr) = run_parked_fork_scenario("a-descriptor-above-a-lowered-soft-limit");
    assert!(
        status.is_some_and(|status| status.success()),
        "the scenario held in its own process: {status:?}\n{stderr}"
    );
}

/// A close the sweep cannot make fails the parked fork's constructor,
/// naming the descriptor and the error, after the child has been
/// collected: the helper never announces a child it could not isolate. In
/// a process of its own, because the policy that refuses the close is the
/// thread's for good and would refuse this process's own close of the
/// sentinel.
#[cfg(target_os = "linux")]
#[test]
fn a_parked_fork_whose_sweep_cannot_close_a_descriptor_fails_before_announcing_the_child() {
    let (status, stderr) = run_parked_fork_scenario("a-close-that-fails");
    assert!(
        status.is_some_and(|status| status.success()),
        "the scenario held in its own process: {status:?}\n{stderr}"
    );
}

/// The child half of the isolation witnesses that change state the whole
/// process shares -- its soft descriptor limit, and a policy on `close`
/// itself -- which no test may do in the suite's shared process, and of the
/// scenario wrapper's own witnesses, which leave a process behind or never
/// end. Re-invoked as a subprocess with the scenario named in
/// `UPSTROKE_TEST_PARKED_FORK_SCENARIO`; a witness exits 0 when its
/// assertions hold, and its stderr carries the reason when they do not.
#[cfg(unix)]
#[test]
#[ignore = "spawned as a subprocess by the parked-fork isolation witnesses"]
fn parked_fork_isolation_kill_child() {
    let scenario =
        std::env::var("UPSTROKE_TEST_PARKED_FORK_SCENARIO").expect("the parent names the scenario");
    match scenario.as_str() {
        "exit-23-leaving-a-silent-member-in-the-group" => {
            exit_leaving_a_silent_member_in_the_group();
        }
        #[cfg(target_os = "linux")]
        "drop-a-parked-fork-whose-kill-is-refused" => drop_a_parked_fork_whose_kill_is_refused(),
        #[cfg(target_os = "linux")]
        "drop-a-scenario-owner-whose-group-kill-is-refused-after-its-leader-exited" => {
            drop_a_scenario_owner_whose_group_kill_is_refused_after_its_leader_exited();
        }
        #[cfg(target_os = "linux")]
        "a-close-denied-with-ebadf" => {
            a_close_the_policy_refuses_fails_the_setup_after_the_child_is_reaped(libc::EBADF);
        }
        "a-descriptor-above-a-lowered-soft-limit" => {
            a_descriptor_above_a_lowered_soft_limit_is_closed();
        }
        #[cfg(target_os = "linux")]
        "a-close-that-fails" => {
            a_close_the_policy_refuses_fails_the_setup_after_the_child_is_reaped(libc::EIO);
        }
        #[cfg(target_os = "linux")]
        "a-close-denied-with-eintr" => {
            a_close_the_policy_refuses_fails_the_setup_after_the_child_is_reaped(libc::EINTR);
        }
        "park-a-stopped-fork-and-never-end" => park_a_stopped_fork_and_never_end(),
        "exit-23-leaving-a-child-holding-stderr" => exit_leaving_a_child_holding_stderr(),
        "hold-stderr-and-never-end" => loop {
            std::thread::sleep(Duration::from_secs(1));
        },
        "exit-0-after-200ms" => {
            std::thread::sleep(Duration::from_millis(200));
            std::process::exit(0)
        }
        #[cfg(target_os = "linux")]
        "release-a-parked-fork-whose-every-observation-is-interrupted" => {
            release_a_parked_fork_whose_every_observation_is_interrupted();
        }
        #[cfg(target_os = "linux")]
        "drop-a-stopped-parked-fork-whose-every-observation-is-interrupted" => {
            drop_a_stopped_parked_fork_whose_every_observation_is_interrupted();
        }
        #[cfg(target_os = "linux")]
        "observe-a-parked-fork-whose-every-observation-is-interrupted" => {
            observe_a_parked_fork_whose_every_observation_is_interrupted();
        }
        #[cfg(target_os = "linux")]
        "fail-at-the-bound-holding-a-stopped-child-on-a-worker-that-never-answers" => {
            fail_at_the_bound_holding_a_stopped_child_on_a_worker_that_never_answers();
        }
        #[cfg(target_os = "linux")]
        "drop-a-parked-fork-whose-kill-and-stderr-writes-are-refused" => {
            drop_a_parked_fork_whose_kill_and_stderr_writes_are_refused();
        }
        #[cfg(target_os = "linux")]
        "drop-parked-forks-whose-kill-is-refused-and-stderr-answers-eio" => {
            drop_parked_forks_whose_kill_is_refused_and_stderr_answers_eio();
        }
        #[cfg(target_os = "linux")]
        "drop-a-scenario-owner-whose-kill-and-stderr-writes-are-refused" => {
            drop_a_scenario_owner_whose_kill_and_stderr_writes_are_refused();
        }
        #[cfg(target_os = "linux")]
        "drop-a-scenario-owner-whose-kill-is-refused-and-stderr-answers-eio" => {
            drop_a_scenario_owner_whose_kill_is_refused_and_stderr_answers_eio();
        }
        #[cfg(target_os = "linux")]
        "unwind-through-a-scenario-owner-whose-kill-is-refused-and-stderr-answers-eio" => {
            unwind_through_a_scenario_owner_whose_kill_is_refused_and_stderr_answers_eio();
        }
        #[cfg(target_os = "linux")]
        "drop-a-scenario-owner-whose-group-observation-opens-are-interrupted" => {
            drop_a_scenario_owner_whose_group_observation_opens_are_interrupted();
        }
        #[cfg(target_os = "linux")]
        "drop-a-stopped-parked-fork-whose-every-rest-is-refused" => {
            drop_a_stopped_parked_fork_whose_every_rest_is_refused();
        }
        #[cfg(target_os = "linux")]
        "run-a-scenario-wrapper-whose-every-rest-is-refused" => {
            run_a_scenario_wrapper_whose_every_rest_is_refused();
        }
        #[cfg(target_os = "linux")]
        "drop-a-stopped-parked-fork-whose-release-send-is-interrupted" => {
            drop_a_stopped_parked_fork_whose_release_send_is_interrupted();
        }
        #[cfg(target_os = "linux")]
        "release-a-stopped-parked-fork-whose-release-send-is-interrupted" => {
            release_a_stopped_parked_fork_whose_release_send_is_interrupted();
        }
        other => panic!("no such scenario: {other}"),
    }
}

/// A scenario that never ends, in a process of its own, with two processes
/// of its own behind it: a fork parked and then stopped, so that it can read
/// neither its release nor an EOF, and a child that runs on holding the
/// scenario's stderr ([`spawn_a_child_holding_stderr`]); each pid written to
/// stderr for the parent to read. The scenario then waits to be killed. Both
/// kinds, because the kernel ends a stopped member of a process group its
/// leader's death orphans (`SIGHUP`, then `SIGCONT`), so the stopped fork
/// alone would not tell a kill of the group from a kill of the scenario
/// process; the running child is ended by nothing but a kill that reaches
/// the group.
#[cfg(unix)]
fn park_a_stopped_fork_and_never_end() -> ! {
    use crate::workspace_manager::fixture::ParkedFork;
    use std::os::fd::AsRawFd as _;

    let anchor = File::open("/dev/null").expect("a descriptor for the fork to keep");
    let parked = ParkedFork::holding(anchor.as_raw_fd());
    stop_parked_child(parked.pid());
    eprintln!("descendant={}", parked.pid());
    let mut child = spawn_a_child_holding_stderr();
    eprintln!("descendant={}", child.id());
    loop {
        std::thread::sleep(Duration::from_secs(1));
        child
            .try_wait()
            .expect("poll the child holding stderr, which lives until the group is killed");
    }
}

/// This test binary on the `hold-stderr-and-never-end` scenario, its stderr
/// inherited from the scenario process, in that process's group: a process
/// that holds the scenario's stderr for as long as it lives, and lives until
/// it is killed.
#[cfg(unix)]
fn spawn_a_child_holding_stderr() -> std::process::Child {
    let exe = std::env::current_exe().expect("test binary");
    std::process::Command::new(exe)
        .args([
            "--exact",
            "rundir::tests::parked_fork_isolation_kill_child",
            "--ignored",
            "--nocapture",
        ])
        .env(
            "UPSTROKE_TEST_PARKED_FORK_SCENARIO",
            "hold-stderr-and-never-end",
        )
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .spawn()
        .expect("spawn the child that holds stderr")
}

/// A scenario whose process exits abnormally -- 23, its own cleanup never
/// run -- leaving behind a member of its group that holds nothing of the
/// scenario's: the same child as [`spawn_a_child_holding_stderr`] with its
/// stderr sent to `/dev/null`, so that the scenario's pipe answers EOF the
/// moment the scenario exits while the member runs on. The pid is written
/// to stderr for the parent to read. The shape under test is a group whose
/// emptiness the pipe cannot speak for.
#[cfg(unix)]
fn exit_leaving_a_silent_member_in_the_group() -> ! {
    let exe = std::env::current_exe().expect("test binary");
    let child = std::process::Command::new(exe)
        .args([
            "--exact",
            "rundir::tests::parked_fork_isolation_kill_child",
            "--ignored",
            "--nocapture",
        ])
        .env(
            "UPSTROKE_TEST_PARKED_FORK_SCENARIO",
            "hold-stderr-and-never-end",
        )
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("spawn the silent member of the group");
    eprintln!("descendant={}", child.id());
    std::process::exit(23)
}

/// A scenario, in a process of its own, that drops a parked fork whose kill
/// the OS refuses: the fork is made and acknowledged, its child stopped so
/// that no release can end it, a policy answering `EPERM` to `kill` is
/// installed on this thread, and the owner is dropped with a reap bound of
/// 100 ms. The drop's own line on stderr is what the parent reads; after it
/// this writes the child's state from `/proc`, and the parent's kill of the
/// group ends the child. The pid is written first, `descendant=`, for the
/// parent to end whatever this could not.
#[cfg(target_os = "linux")]
fn drop_a_parked_fork_whose_kill_is_refused() {
    use crate::workspace_manager::fixture::ParkedFork;
    use std::os::fd::AsRawFd as _;

    let anchor = File::open("/dev/null").expect("a descriptor for the fork to keep");
    let parked = ParkedFork::holding(anchor.as_raw_fd()).reap_bound(Duration::from_millis(100));
    let pid = parked.pid();
    stop_parked_child(pid);
    eprintln!("descendant={pid}");
    refuse_syscall_on_this_thread(libc::SYS_kill, libc::EPERM);
    let started = Instant::now();
    drop(parked);
    eprintln!(
        "dropped after {:?}; left={}",
        started.elapsed(),
        state_by_proc(pid)
    );
}

/// A scenario, in a process of its own, that drops a scenario owner whose
/// group kill the OS refuses after its leader exited: the owner spawns the
/// silent-member scenario, waits for its leader's natural exit -- peeked,
/// not collected -- writes the owner's text so far, which carries the
/// member's pid as `descendant=`, and the leader's pid as `leader=`,
/// installs a policy answering `EPERM` to `kill` on this thread, and drops
/// the owner: the leader is collectible and the group is not killable. The
/// drop's own line is what the parent reads; after it this writes the
/// member's state from `/proc`. The member is in the leader's group, not
/// this process's, so the parent ends it by pid.
#[cfg(target_os = "linux")]
fn drop_a_scenario_owner_whose_group_kill_is_refused_after_its_leader_exited() {
    let mut owned = ScenarioChild::spawn("exit-23-leaving-a-silent-member-in-the-group");
    let leader = owned.pid();
    let deadline = Instant::now() + SCENARIO_BOUND;
    while !owned.has_ended() {
        assert!(
            Instant::now() < deadline,
            "the silent-member scenario exits by itself"
        );
        owned.drain();
        std::thread::sleep(Duration::from_millis(10));
    }
    owned.drain();
    let text = owned.text();
    let member = descendants_named_in(&text, 1)
        .first()
        .copied()
        .expect("the leader named its member");
    eprintln!("{}", text.trim_end());
    eprintln!("leader={leader}");
    refuse_syscall_on_this_thread(libc::SYS_kill, libc::EPERM);
    let started = Instant::now();
    drop(owned);
    eprintln!(
        "dropped after {:?}; left={}",
        started.elapsed(),
        state_by_proc(member)
    );
}

/// The body of
/// [`a_release_whose_every_observation_is_interrupted_ends_at_its_bounds_and_says_the_child_is_uncollected`],
/// in a process of its own
/// (`release-a-parked-fork-whose-every-observation-is-interrupted`): its
/// assertions are that test's, and a failed one fails this process, which the
/// test reads.
#[cfg(target_os = "linux")]
fn release_a_parked_fork_whose_every_observation_is_interrupted() {
    use crate::workspace_manager::fixture::ParkedFork;
    use std::os::fd::AsRawFd as _;

    const BOUND: Duration = Duration::from_millis(100);
    let (pid_sender, pid_receiver) = std::sync::mpsc::channel();
    let (answered, releasing) = on_a_thread_within(BOUND * 60, move || {
        let anchor = File::open("/dev/null").expect("a descriptor for the fork to keep");
        let parked = ParkedFork::holding(anchor.as_raw_fd()).reap_bound(BOUND);
        pid_sender
            .send(parked.pid())
            .expect("the test waits for the pid");
        refuse_syscall_on_this_thread(libc::SYS_wait4, libc::EINTR);
        let started = Instant::now();
        let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| parked.release()));
        (
            unwound.map_err(|payload| {
                payload
                    .downcast_ref::<String>()
                    .cloned()
                    .unwrap_or_default()
            }),
            started.elapsed(),
        )
    });
    let pid = pid_receiver
        .recv_timeout(Duration::from_secs(60))
        .expect("the fork was made and said which pid");
    // The answer is checked before the join: a thread spinning under a policy
    // that is its own for good cannot be unblocked, so a bound that ran out is
    // this scenario's failure, and the thread is left to end with this
    // process. The child does not end with it by itself -- a stopped child
    // cannot read the EOF its parent's death gives it -- and it is the
    // wrapper's kill of this process's group, made once this process has
    // exited and before its collection, that ends it (`PR320-R5-MAIN-004`,
    // `PR320-R5-REG-004`).
    let (released, took) = answered.expect(
        "the release returned within sixty times its reap bound; a release that makes an \
         interrupted observation again inside itself is the defect this test holds",
    );
    releasing.join().expect("the releasing thread ends");
    let message =
        released.expect_err("a release that could not observe its child is a panic, not a status");
    assert!(
        message.contains(&format!(
            "release the parked child {pid}: the parked child {pid} was sent SIGKILL at the end \
             of {BOUND:?} and not collected within another {BOUND:?}; unobserved, the last \
             observation answering: "
        )) && message.contains("(os error 4)"),
        "the panic names the pid, the signal sent, the second bound and the interruption: \
         {message}"
    );
    assert!(
        took >= BOUND * 2 && took < BOUND * 60,
        "the release spent the reap bound, then the reap bound again after the kill, and \
         returned: {took:?}"
    );
    let status = collect_child_within(pid, Duration::from_secs(5))
        .expect("the child is this test's to collect once its owner gave up");
    assert_eq!(
        status.code(),
        Some(0),
        "the child read its release and ended by itself, unseen by its owner: {status:?}"
    );
    assert!(is_reaped(pid), "and is collected now");
}

/// The body of
/// [`dropping_a_parked_fork_whose_every_observation_is_interrupted_returns_within_its_bounds`],
/// in a process of its own
/// (`drop-a-stopped-parked-fork-whose-every-observation-is-interrupted`): its
/// assertions are that test's, and a failed one fails this process, which the
/// test reads.
#[cfg(target_os = "linux")]
fn drop_a_stopped_parked_fork_whose_every_observation_is_interrupted() {
    use crate::workspace_manager::fixture::ParkedFork;
    use std::os::fd::AsRawFd as _;
    use std::os::unix::process::ExitStatusExt as _;

    const BOUND: Duration = Duration::from_millis(100);
    let (pid_sender, pid_receiver) = std::sync::mpsc::channel();
    let (dropped, dropping) = on_a_thread_within(BOUND * 60, move || {
        let anchor = File::open("/dev/null").expect("a descriptor for the fork to keep");
        let parked = ParkedFork::holding(anchor.as_raw_fd()).reap_bound(BOUND);
        stop_parked_child(parked.pid());
        pid_sender
            .send(parked.pid())
            .expect("the test waits for the pid");
        refuse_syscall_on_this_thread(libc::SYS_wait4, libc::EINTR);
        let started = Instant::now();
        drop(parked);
        started.elapsed()
    });
    let pid = pid_receiver
        .recv_timeout(Duration::from_secs(60))
        .expect("the fork was made and said which pid");
    // The answer is checked before the join: a thread spinning under a policy
    // that is its own for good cannot be unblocked, so a bound that ran out is
    // this scenario's failure, and the thread is left to end with this
    // process. The child does not end with it by itself -- a stopped child
    // cannot read the EOF its parent's death gives it -- and it is the
    // wrapper's kill of this process's group, made once this process has
    // exited and before its collection, that ends it (`PR320-R5-MAIN-004`,
    // `PR320-R5-REG-004`).
    let took = dropped.expect(
        "the drop returned within sixty times its reap bound; a drop that makes an interrupted \
         observation again inside itself is the defect this test holds",
    );
    dropping.join().expect("the dropping thread ends");
    assert!(
        took >= BOUND * 2 && took < BOUND * 60,
        "the drop spent the reap bound, then the reap bound again after the kill, and returned: \
         {took:?}"
    );
    let status = collect_child_within(pid, Duration::from_secs(5))
        .expect("the killed child is this test's to collect once its owner gave up");
    assert_eq!(
        status.signal(),
        Some(libc::SIGKILL),
        "the child was killed at the end of the reap bound: {status:?}"
    );
    assert!(is_reaped(pid), "and is collected now");
}

/// The body of
/// [`a_liveness_observation_interrupted_for_the_whole_bound_fails_instead_of_answering`],
/// in a process of its own
/// (`observe-a-parked-fork-whose-every-observation-is-interrupted`): its
/// assertions are that test's, and a failed one fails this process, which the
/// test reads.
#[cfg(target_os = "linux")]
fn observe_a_parked_fork_whose_every_observation_is_interrupted() {
    use crate::workspace_manager::fixture::ParkedFork;
    use std::os::fd::AsRawFd as _;

    const BOUND: Duration = Duration::from_millis(100);
    let (pid_sender, pid_receiver) = std::sync::mpsc::channel();
    let (answered, observing) = on_a_thread_within(BOUND * 60, move || {
        let anchor = File::open("/dev/null").expect("a descriptor for the fork to keep");
        let parked = ParkedFork::holding(anchor.as_raw_fd()).reap_bound(BOUND);
        pid_sender
            .send(parked.pid())
            .expect("the test waits for the pid");
        refuse_syscall_on_this_thread(libc::SYS_wait4, libc::EINTR);
        let started = Instant::now();
        let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| parked.is_alive()));
        let took = started.elapsed();
        drop(parked);
        (
            unwound.map_err(|payload| {
                payload
                    .downcast_ref::<String>()
                    .cloned()
                    .unwrap_or_default()
            }),
            took,
        )
    });
    let pid = pid_receiver
        .recv_timeout(Duration::from_secs(60))
        .expect("the fork was made and said which pid");
    // The answer is checked before the join: a thread spinning under a policy
    // that is its own for good cannot be unblocked, so a bound that ran out is
    // this scenario's failure, and the thread is left to end with this
    // process. The child does not end with it by itself -- a stopped child
    // cannot read the EOF its parent's death gives it -- and it is the
    // wrapper's kill of this process's group, made once this process has
    // exited and before its collection, that ends it (`PR320-R5-MAIN-004`,
    // `PR320-R5-REG-004`).
    let (answer, took) = answered.expect(
        "the observation returned within sixty times its reap bound; one that makes an \
         interrupted observation again forever is the defect this test holds",
    );
    observing.join().expect("the observing thread ends");
    let message = answer
        .expect_err("an observation interrupted for the whole bound is a failure, not an answer");
    assert!(
        message.contains(&format!(
            "waitpid({pid}, WNOHANG) was interrupted at every observation for {BOUND:?}"
        )) && message.contains("(os error 4)"),
        "the panic names the pid, the bound and the interruption: {message}"
    );
    assert!(
        took >= BOUND && took < BOUND * 60,
        "the observation spent its bound and no more: {took:?}"
    );
    let status = collect_child_within(pid, Duration::from_secs(5))
        .expect("the child is this test's to collect once its owner gave up");
    assert_eq!(
        status.code(),
        Some(0),
        "the child read the release the drop wrote and ended by itself: {status:?}"
    );
    assert!(is_reaped(pid), "and is collected now");
}

/// In a process of its own: a parked fork, stopped and acknowledged, handed
/// to a worker that holds it and never answers -- as the owner's worker
/// under a first-bad mutation spins, its policy the worker's for good -- and
/// waited for within a second. The scenario fails at that bound, exactly as
/// the interruption regressions fail on their failing path, and exits with
/// the worker still holding the owner and the owner the stopped child:
/// nothing in this process releases, kills or collects it. That is left to
/// the wrapper's kill of this process's group
/// ([`a_scenario_whose_owner_never_answers_fails_at_its_bound_and_its_stopped_child_ends_with_the_group`]).
/// The pid is written first, `descendant=`.
#[cfg(target_os = "linux")]
fn fail_at_the_bound_holding_a_stopped_child_on_a_worker_that_never_answers() {
    use crate::workspace_manager::fixture::ParkedFork;
    use std::os::fd::AsRawFd as _;

    let anchor = File::open("/dev/null").expect("a descriptor for the fork to keep");
    let parked = ParkedFork::holding(anchor.as_raw_fd());
    stop_parked_child(parked.pid());
    eprintln!("descendant={}", parked.pid());
    let (never_sent, never) = std::sync::mpsc::channel::<()>();
    // Forgotten, so that no unwinding of this thread can end the worker's
    // wait: the worker holds the owner for as long as this process lives.
    std::mem::forget(never_sent);
    let (answered, _holding) = on_a_thread_within(Duration::from_secs(1), move || {
        let _held = parked;
        if never.recv().is_ok() {
            // Nothing is ever sent.
        }
    });
    answered.expect(
        "the worker answered within its bound; one that never answers is the failing path this \
         scenario stands for",
    );
}

/// In a process of its own: a parked fork, stopped and acknowledged, dropped
/// with a reap bound of 100 ms on a thread whose kill and writes to stderr a
/// policy refuses (`EPERM`, `EINTR`), each refusal seen in force first. The
/// drop's time is written after it, with the child's state from `/proc`: a
/// refused kill leaves the child stopped, and the wrapper's kill of this
/// process's group ends it. The pid is written first, `descendant=`.
#[cfg(target_os = "linux")]
fn drop_a_parked_fork_whose_kill_and_stderr_writes_are_refused() {
    use crate::workspace_manager::fixture::ParkedFork;
    use std::os::fd::AsRawFd as _;

    const BOUND: Duration = Duration::from_millis(100);
    let anchor = File::open("/dev/null").expect("a descriptor for the fork to keep");
    let parked = ParkedFork::holding(anchor.as_raw_fd()).reap_bound(BOUND);
    let pid = parked.pid();
    stop_parked_child(pid);
    eprintln!("descendant={pid}");
    let took = drop_under(
        parked,
        &[Refusal::Kill, Refusal::StderrWrites(libc::EINTR)],
        BOUND * 60,
    );
    assert!(
        took >= BOUND && took < BOUND * 60,
        "the drop spent the reap bound, met the refused kill and its refused line, and \
         returned: {took:?}"
    );
    eprintln!("dropped after {took:?}; left={}", state_by_proc(pid));
}

/// In a process of its own: two parked forks, stopped and acknowledged, each
/// under a policy that refuses its kill (`EPERM`) and answers `EIO` to every
/// write to stderr, each refusal seen in force first; the first dropped
/// plainly, the second dropped by the unwinding of a failure its thread
/// raised with it in scope, the unwinding caught. The pids are written
/// first, `descendant=`; both children are left stopped and the wrapper's
/// kill of this process's group ends them.
#[cfg(target_os = "linux")]
fn drop_parked_forks_whose_kill_is_refused_and_stderr_answers_eio() {
    use crate::workspace_manager::fixture::ParkedFork;
    use std::os::fd::AsRawFd as _;

    const BOUND: Duration = Duration::from_millis(100);
    let anchor = File::open("/dev/null").expect("a descriptor for the forks to keep");
    let dropped = ParkedFork::holding(anchor.as_raw_fd()).reap_bound(BOUND);
    let unwound = ParkedFork::holding(anchor.as_raw_fd()).reap_bound(BOUND);
    let (first, second) = (dropped.pid(), unwound.pid());
    stop_parked_child(first);
    stop_parked_child(second);
    eprintln!("descendant={first}");
    eprintln!("descendant={second}");
    let took = drop_under(
        dropped,
        &[Refusal::Kill, Refusal::StderrWrites(libc::EIO)],
        BOUND * 60,
    );
    assert!(
        took >= BOUND && took < BOUND * 60,
        "the drop spent the reap bound, met the refused kill and the failed line, and \
         returned: {took:?}"
    );
    let caught = unwind_through_under(
        unwound,
        &[Refusal::Kill, Refusal::StderrWrites(libc::EIO)],
        BOUND * 60,
    );
    assert!(
        caught,
        "the unwinding reached the caller's catch: the drop it ran did not panic"
    );
    eprintln!(
        "dropped after {took:?}; unwound and caught; left={} {}",
        state_by_proc(first),
        state_by_proc(second)
    );
}

/// A scenario owner whose leader has exited by itself, 23, leaving a silent
/// member in its group, and the leader's pid: the owner holds the leader
/// uncollected, peeked and not waited for. The leader is written to stderr
/// as `leader=`, the member as `descendant=`. With `end_the_member` the
/// member is killed first, by this thread, through the leader's group -- the
/// leader uncollected, so the id is still that group's -- and seen ended, so
/// that a drop whose own kill a policy refuses leaves nothing running,
/// whatever becomes of the drop: the drop's kill is refused whatever the
/// group holds, which is the state under test.
#[cfg(target_os = "linux")]
fn an_owner_whose_leader_exited(end_the_member: bool) -> (ScenarioChild, libc::pid_t) {
    let mut owned = ScenarioChild::spawn("exit-23-leaving-a-silent-member-in-the-group");
    let leader = owned.pid();
    let deadline = Instant::now() + SCENARIO_BOUND;
    while !owned.has_ended() {
        assert!(
            Instant::now() < deadline,
            "the silent-member scenario exits by itself"
        );
        owned.drain();
        std::thread::sleep(Duration::from_millis(10));
    }
    owned.drain();
    let member = descendants_named_in(&owned.text(), 1)
        .first()
        .copied()
        .expect("the leader named its member");
    eprintln!("leader={leader}");
    eprintln!("descendant={member}");
    if end_the_member {
        // SAFETY: `kill` takes a pid and a signal by value; the negative pid
        // is the group of the leader this owner holds uncollected, so the id
        // is still that group's.
        assert_eq!(
            unsafe { libc::kill(-leader, libc::SIGKILL) },
            0,
            "kill the leader's group {leader}: {}",
            std::io::Error::last_os_error()
        );
        assert_ended_within(
            member,
            Duration::from_secs(5),
            "the member of the leader's group, killed by this scenario",
        );
    }
    (owned, leader)
}

/// In a process of its own: a scenario owner whose leader exited 23, dropped
/// on a thread whose kill and writes to stderr a policy refuses (`EPERM`,
/// `EINTR`), each refusal seen in force first. The drop's time is written
/// after it.
#[cfg(target_os = "linux")]
fn drop_a_scenario_owner_whose_kill_and_stderr_writes_are_refused() {
    let (owned, leader) = an_owner_whose_leader_exited(true);
    let took = drop_under(
        owned,
        &[Refusal::Kill, Refusal::StderrWrites(libc::EINTR)],
        SCENARIO_GROUP_BOUND * 3,
    );
    assert!(
        is_reaped(leader),
        "the drop collected the leader although its kill was refused"
    );
    eprintln!("dropped the owner of {leader} after {took:?}");
}

/// In a process of its own: a scenario owner whose leader exited 23, dropped
/// on a thread that a policy refuses its kill (`EPERM`) and answers `EIO` to
/// every write to stderr, each refusal seen in force first. The drop's time
/// is written after it.
#[cfg(target_os = "linux")]
fn drop_a_scenario_owner_whose_kill_is_refused_and_stderr_answers_eio() {
    let (owned, leader) = an_owner_whose_leader_exited(true);
    let took = drop_under(
        owned,
        &[Refusal::Kill, Refusal::StderrWrites(libc::EIO)],
        SCENARIO_GROUP_BOUND * 3,
    );
    assert!(
        is_reaped(leader),
        "the drop collected the leader although its kill was refused"
    );
    eprintln!("dropped the owner of {leader} after {took:?}");
}

/// In a process of its own: a scenario owner whose leader exited 23, its
/// drop run by the unwinding of a failure raised with it in scope, on a
/// thread that a policy refuses its kill (`EPERM`) and answers `EIO` to
/// every write to stderr, each refusal seen in force first; the unwinding is
/// caught. What happened is written after it.
#[cfg(target_os = "linux")]
fn unwind_through_a_scenario_owner_whose_kill_is_refused_and_stderr_answers_eio() {
    let (owned, leader) = an_owner_whose_leader_exited(true);
    let caught = unwind_through_under(
        owned,
        &[Refusal::Kill, Refusal::StderrWrites(libc::EIO)],
        SCENARIO_GROUP_BOUND * 3,
    );
    assert!(
        caught,
        "the unwinding reached the caller's catch: the drop it ran did not panic"
    );
    assert!(
        is_reaped(leader),
        "the drop the unwinding ran collected the leader although its kill was refused"
    );
    eprintln!("unwound through the owner of {leader} and caught it");
}

/// In a process of its own: a scenario owner whose leader exited 23 with its
/// member alive, dropped on a thread whose opens of anything but a directory
/// a policy answers `EINTR`, the refusal seen in force first. The drop's own
/// kill is made; its observation of the group lists `/proc` and meets the
/// refusal at the first process it opens. The drop's time is written after
/// it; the drop's own line, on stderr, says what it could observe.
#[cfg(target_os = "linux")]
fn drop_a_scenario_owner_whose_group_observation_opens_are_interrupted() {
    let (owned, leader) = an_owner_whose_leader_exited(false);
    let took = drop_under(
        owned,
        &[Refusal::NondirectoryOpens],
        SCENARIO_GROUP_BOUND * 3,
    );
    assert!(
        is_reaped(leader),
        "the drop killed the group and collected the leader before it observed the group"
    );
    eprintln!("dropped the owner of {leader} after {took:?}");
}

/// In a process of its own: a parked fork, stopped and acknowledged, dropped
/// with a reap bound of 100 ms on a thread whose every rest a policy refuses
/// (`clock_nanosleep` answering `EINTR`), the refusal seen in force first.
/// The drop spends its bound, kills the child and collects it. The pid is
/// written first, `descendant=`.
#[cfg(target_os = "linux")]
fn drop_a_stopped_parked_fork_whose_every_rest_is_refused() {
    use crate::workspace_manager::fixture::ParkedFork;
    use std::os::fd::AsRawFd as _;

    const BOUND: Duration = Duration::from_millis(100);
    let anchor = File::open("/dev/null").expect("a descriptor for the fork to keep");
    let parked = ParkedFork::holding(anchor.as_raw_fd()).reap_bound(BOUND);
    let pid = parked.pid();
    stop_parked_child(pid);
    eprintln!("descendant={pid}");
    let took = drop_under(parked, &[Refusal::Rests], BOUND * 60);
    assert!(
        took >= BOUND && took < BOUND * 60,
        "the drop spent the reap bound with every rest refused, then killed the child: {took:?}"
    );
    assert!(
        is_reaped(pid),
        "the drop killed the stopped child and collected it, its rests refused"
    );
    eprintln!("dropped after {took:?}; the child collected");
}

/// In a process of its own: the scenario wrapper run on a thread of its own,
/// its every rest refused by a policy installed the moment it owns its
/// scenario -- so the scenario, `exit-0-after-200ms`, is not the policy's --
/// with a bound of 100 ms: the scenario outlives the bound, so the wrapper's
/// loop meets refused rests from its first turn to its bound, and then kills
/// the group and ends it. A wrapper that made a refused rest again inside
/// itself would never come back to its bound; the scenario then ends by
/// itself at 200 ms, and this fails at its own bound.
#[cfg(target_os = "linux")]
fn run_a_scenario_wrapper_whose_every_rest_is_refused() {
    let (answered, running) = on_a_thread_within(SCENARIO_GROUP_BOUND * 3, || {
        let mut in_force = Err(String::from("the wrapper never owned its scenario"));
        let started = Instant::now();
        let answer = run_parked_fork_scenario_within(
            "exit-0-after-200ms",
            Duration::from_millis(100),
            |_| {
                in_force = refuse_on_this_thread(&[Refusal::Rests]);
            },
        );
        in_force.map(|()| (answer, started.elapsed()))
    });
    let ((status, text), took) = settled_within(answered, SCENARIO_GROUP_BOUND * 3, "the wrapper");
    running.join().expect("the wrapper's thread ends");
    assert!(
        status.is_none(),
        "a scenario killed at the bound has no status: {status:?}\n{text}"
    );
    assert!(
        text.contains("the scenario did not end within 100ms")
            && text.contains("the scenario's process group was killed")
            && text.contains("the scenario's group was empty"),
        "the wrapper ended the scenario at its bound with every rest refused: {text}"
    );
    eprintln!("the wrapper ended its scenario after {took:?}, its rests refused");
}

/// In a process of its own: a parked fork, stopped and acknowledged, dropped
/// with a reap bound of 100 ms on a thread whose sends a policy answers
/// `EINTR`, the refusal seen in force first: the release byte is not
/// written, the child cannot read the EOF that would release it instead, and
/// the drop kills it at the end of the bound and collects it. The drop's own
/// line, on stderr, says both. The pid is written first, `descendant=`.
#[cfg(target_os = "linux")]
fn drop_a_stopped_parked_fork_whose_release_send_is_interrupted() {
    use crate::workspace_manager::fixture::ParkedFork;
    use std::os::fd::AsRawFd as _;

    const BOUND: Duration = Duration::from_millis(100);
    let anchor = File::open("/dev/null").expect("a descriptor for the fork to keep");
    let parked = ParkedFork::holding(anchor.as_raw_fd()).reap_bound(BOUND);
    let pid = parked.pid();
    stop_parked_child(pid);
    eprintln!("descendant={pid}");
    let took = drop_under(parked, &[Refusal::Sends], BOUND * 60);
    assert!(
        took >= BOUND && took < BOUND * 60,
        "the drop spent the reap bound, then killed the child: {took:?}"
    );
    assert!(
        is_reaped(pid),
        "the drop killed the child its release could not reach and collected it"
    );
    eprintln!("dropped after {took:?}; the child collected");
}

/// In a process of its own: a parked fork, stopped and acknowledged, released
/// with a reap bound of 100 ms on a thread whose sends a policy answers
/// `EINTR`, the refusal seen in force first: the release panics, saying the
/// release could not be written and that the child was killed and collected,
/// with its status -- not that it was left uncollected. The pid is written
/// first, `descendant=`.
#[cfg(target_os = "linux")]
fn release_a_stopped_parked_fork_whose_release_send_is_interrupted() {
    use crate::workspace_manager::fixture::ParkedFork;
    use std::os::fd::AsRawFd as _;

    const BOUND: Duration = Duration::from_millis(100);
    let anchor = File::open("/dev/null").expect("a descriptor for the fork to keep");
    let parked = ParkedFork::holding(anchor.as_raw_fd()).reap_bound(BOUND);
    let pid = parked.pid();
    stop_parked_child(pid);
    eprintln!("descendant={pid}");
    let (answered, releasing) = on_a_thread_within(BOUND * 60, move || {
        refuse_on_this_thread(&[Refusal::Sends])?;
        let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| parked.release()));
        Ok::<_, String>(unwound.map_err(|payload| {
            payload
                .downcast_ref::<String>()
                .cloned()
                .unwrap_or_default()
        }))
    });
    let released = settled_within(answered, BOUND * 60, "the release");
    releasing.join().expect("the releasing thread ends");
    let message = released.expect_err(
        "a release whose child had to be killed after its release could not be written is a \
         panic, not a status",
    );
    assert!(
        message.contains(&format!(
            "release the parked child {pid}: its release could not be written: Interrupted \
             system call (os error 4); it was killed at the end of {BOUND:?} and collected: \
             signal: 9 (SIGKILL)"
        )),
        "the panic says the release failed and the child was killed and collected, with its \
         status: {message}"
    );
    assert!(is_reaped(pid), "and the child is collected");
    eprintln!("released: {message}");
}

/// A scenario whose process exits abnormally -- 23, its own cleanup never
/// run -- leaving behind a child that holds its stderr and runs on
/// ([`spawn_a_child_holding_stderr`]), its pid written to stderr for the
/// parent to read. A running child, not a stopped fork, because a stopped
/// member of a process group that its leader's exit orphans is sent `SIGHUP`
/// by the kernel and ends on its own; a child that runs on holding the pipe
/// is ended by nothing but the wrapper. The child is abandoned on purpose:
/// a scenario that exits without collecting what it spawned is the shape
/// under test.
#[cfg(unix)]
fn exit_leaving_a_child_holding_stderr() -> ! {
    let child = spawn_a_child_holding_stderr();
    eprintln!("descendant={}", child.id());
    std::process::exit(23)
}

/// In a process of its own: a sentinel copied to descriptor 128 by `dup2`,
/// then the soft descriptor limit lowered to 64, so that the sentinel sits
/// above what `sysconf(_SC_OPEN_MAX)` reports; on Linux `close_range` is
/// refused as well, so that the sweep by number, whose ceiling is in
/// question, is what runs there too. The parked fork's sweep closes the
/// sentinel: the read answers EOF within the bound.
#[cfg(unix)]
fn a_descriptor_above_a_lowered_soft_limit_is_closed() {
    use crate::workspace_manager::fixture::ParkedFork;
    use std::os::fd::{AsRawFd as _, FromRawFd as _};

    const HIGH: libc::c_int = 128;
    let root = scratch("childlease-high-descriptor");
    let public = public_dir(&root.join("repo"), "01CHILDLEASE000000000000009");
    fs::create_dir_all(&public).expect("the run's public directory");
    let (sentinel, mut observer) = sentinel_pair();
    let mut limits = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: `getrlimit` writes one `rlimit` through the pointer, which
    // lives for the call.
    assert_eq!(
        unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut limits) },
        0,
        "read RLIMIT_NOFILE: {}",
        std::io::Error::last_os_error()
    );
    assert!(
        limits.rlim_cur > 128,
        "this witness places its sentinel at descriptor {HIGH}, which needs a soft limit above \
         it; the soft limit is {}",
        limits.rlim_cur
    );
    // SAFETY: `dup2` takes two descriptors by value; this process is the
    // witness's own and fresh, with nothing open at `HIGH`.
    assert_eq!(
        unsafe { libc::dup2(sentinel.as_raw_fd(), HIGH) },
        HIGH,
        "copy the sentinel to descriptor {HIGH}: {}",
        std::io::Error::last_os_error()
    );
    // SAFETY: `HIGH` is the copy `dup2` just made and nothing else owns; the
    // stream owns it from here and closes it when dropped.
    let high = unsafe { std::os::unix::net::UnixStream::from_raw_fd(HIGH) };
    drop(sentinel);
    let lowered = libc::rlimit {
        rlim_cur: 64,
        rlim_max: limits.rlim_max,
    };
    // SAFETY: `setrlimit` reads one `rlimit` through the pointer, which
    // lives for the call; the limit lowered is this witness process's own.
    assert_eq!(
        unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &lowered) },
        0,
        "lower the soft descriptor limit to 64: {}",
        std::io::Error::last_os_error()
    );
    let mut now = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: as above.
    assert_eq!(
        unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut now) },
        0,
        "read RLIMIT_NOFILE back: {}",
        std::io::Error::last_os_error()
    );
    assert_eq!(
        now.rlim_cur, 64,
        "the soft limit is below the sentinel's number while the sentinel stays open"
    );
    #[cfg(target_os = "linux")]
    {
        refuse_syscall_on_this_thread(libc::SYS_close_range, libc::ENOSYS);
        assert_close_range_answers(libc::ENOSYS);
    }
    let parked = ParkedFork::holding_the_lease_of(&public);
    let holder = parked.pid();
    drop(high);
    if let Err(held) = sentinel_closed_within(&mut observer, LEASE_RELEASE_BOUND, &mut |_| {}) {
        panic!(
            "the sentinel at descriptor {HIGH}, above the lowered soft limit of 64, was closed by \
             fork {holder}'s sweep: {held}"
        );
    }
    let status = parked.release();
    assert!(
        status.success(),
        "the released fork exited cleanly: {status:?}"
    );
}

/// A close the policy answers `EINTR` to, without making it, is verified and
/// found not made: the parked fork's constructor fails, naming the descriptor
/// and the error, after the child has been collected -- the helper never
/// announces a child whose sentinel copy the sweep left open. In a process
/// of its own, as the `EIO` witness is, because the policy is the thread's
/// for good. A native Linux witness of a denied operation: it says nothing
/// about an `EINTR` an ordinary Linux or macOS `close` answers, which the
/// same verification reads as the close it was.
#[cfg(target_os = "linux")]
#[test]
fn a_parked_fork_whose_sweep_is_denied_a_close_with_eintr_fails_before_announcing_the_child() {
    let (status, stderr) = run_parked_fork_scenario("a-close-denied-with-eintr");
    assert!(
        status.is_some_and(|status| status.success()),
        "the scenario held in its own process: {status:?}\n{stderr}"
    );
}

/// A close the policy answers `EBADF` to, without making it, is read from
/// the descriptor table and found open: the parked fork's constructor fails,
/// naming the descriptor and the error, after the child has been collected.
/// `EBADF` is the answer the sweep takes as a number not open when the table
/// says so, and a policy's `EBADF` says nothing about the table
/// (`PR320-R3-REG-002`); in a process of its own, as the `EIO` and `EINTR`
/// witnesses are.
#[cfg(target_os = "linux")]
#[test]
fn a_parked_fork_whose_sweep_is_denied_a_close_with_ebadf_fails_before_announcing_the_child() {
    let (status, stderr) = run_parked_fork_scenario("a-close-denied-with-ebadf");
    assert!(
        status.is_some_and(|status| status.success()),
        "the scenario held in its own process: {status:?}\n{stderr}"
    );
}

/// A `close_range` that answers success without closing -- a policy's
/// answer, `SECCOMP_RET_ERRNO` carrying 0 -- is found out by the reading of
/// the numbers the parent listed at the fork: the constructor fails naming
/// the first listed descriptor still open and that its close answered
/// success, after the child has been collected. The policy is this thread's
/// and its forks', and this thread makes no `close_range` of its own, so no
/// fresh process is needed.
#[cfg(target_os = "linux")]
#[test]
fn a_range_close_that_answers_success_without_closing_is_found_out_before_the_child_is_announced() {
    use crate::workspace_manager::fixture::ParkedFork;

    let root = scratch("childlease-range-close-lies");
    let public = public_dir(&root.join("repo"), "01CHILDLEASE00000000000000B");
    fs::create_dir_all(&public).expect("the run's public directory");
    let (_sentinel, _observer) = sentinel_pair();
    refuse_syscall_on_this_thread(libc::SYS_close_range, 0);
    // SAFETY: a raw syscall over a numeric range the kernel refuses as
    // empty (`EINVAL`), so an answer of 0 is the policy's alone; it closes
    // nothing.
    assert_eq!(
        unsafe { libc::syscall(libc::SYS_close_range, 3_u32, 2_u32, 0_u32) },
        0,
        "the policy answers success to close_range: {}",
        std::io::Error::last_os_error()
    );
    let unwound = std::panic::catch_unwind(|| ParkedFork::holding_the_lease_of(&public));
    let payload = unwound
        .err()
        .expect("a range close that closed nothing fails the constructor");
    let message = payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| {
            payload
                .downcast_ref::<&str>()
                .map(|text| (*text).to_owned())
        })
        .expect("the panic carries a message");
    assert!(
        message.contains("could not close inherited descriptor")
            && message.contains("the close answered success and the descriptor read open after it"),
        "the constructor names a descriptor left open and the success its close answered: {message}"
    );
    let pid: libc::pid_t = message
        .strip_prefix("the parked child ")
        .and_then(|rest| rest.split(' ').next())
        .and_then(|pid| pid.parse().ok())
        .expect("the message names the child's pid");
    assert!(
        is_reaped(pid),
        "the child {pid} was collected before the constructor failed"
    );
}

/// In a process of its own: a policy that answers `errno` to the `close` of
/// one descriptor, the sentinel's, and `ENOSYS` to `close_range` so that the
/// sweep reaches it. The parked fork's constructor fails, naming the
/// descriptor and the error, and the child it forked has been collected
/// before it fails: no child is announced that could not be isolated, and
/// none is left behind. `EIO` is a close that failed; `EINTR` is the answer
/// a policy gives without making the close, which the sweep verifies rather
/// than trusts.
#[cfg(target_os = "linux")]
fn a_close_the_policy_refuses_fails_the_setup_after_the_child_is_reaped(errno: libc::c_int) {
    use crate::workspace_manager::fixture::ParkedFork;
    use std::os::fd::AsRawFd as _;

    let root = scratch("childlease-close-fails");
    let public = public_dir(&root.join("repo"), "01CHILDLEASE00000000000000A");
    fs::create_dir_all(&public).expect("the run's public directory");
    let (sentinel, _observer) =
        std::os::unix::net::UnixStream::pair().expect("a sentinel socket pair");
    let number = sentinel.as_raw_fd();
    refuse_closing_on_this_thread(number, errno);
    assert_close_range_answers(libc::ENOSYS);
    let unwound = std::panic::catch_unwind(|| ParkedFork::holding_the_lease_of(&public));
    let payload = unwound
        .err()
        .expect("a sweep that could not close the sentinel fails the constructor");
    let message = payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| {
            payload
                .downcast_ref::<&str>()
                .map(|text| (*text).to_owned())
        })
        .expect("the panic carries a message");
    let expected = format!(
        "could not close inherited descriptor {number}: {}",
        std::io::Error::from_raw_os_error(errno)
    );
    assert!(
        message.contains(&expected),
        "the constructor names the descriptor and the error: {message}"
    );
    let pid: libc::pid_t = message
        .strip_prefix("the parked child ")
        .and_then(|rest| rest.split(' ').next())
        .and_then(|pid| pid.parse().ok())
        .expect("the message names the child's pid");
    assert!(
        is_reaped(pid),
        "the child {pid} was collected before the constructor failed"
    );
    // The policy refuses this thread's own close of the sentinel too, so
    // the stream is forgotten rather than dropped into an error a drop
    // cannot report; the process ends with the scenario.
    std::mem::forget(sentinel);
}

/// A run directory that does not exist is an error, not a silent write with
/// no lease: every ref write a coordinator makes happens inside a run whose
/// directory `RunLock::acquire` already created the lease file in.
#[cfg(unix)]
#[test]
fn a_hold_over_an_absent_run_directory_is_an_io_error_naming_the_lease() {
    let root = scratch("childlease-absent");
    let public = public_dir(&root.join("repo"), "01CHILDLEASE000000000000002");
    let mut command = std::process::Command::new("git");
    let error = hold_cleanup_lease_for_child(&mut command, &public)
        .expect_err("no directory, no lease, no silent success");
    assert!(
        matches!(&error, UpstrokeError::Io { path, .. } if *path == cleanup_lock_file(&public)),
        "{error}"
    );
}

// =======================================================================
// The test build's scratch trees
// =======================================================================

/// Witness 4 — a reclaim removes the token root and nothing outside it.
///
/// The removal is recursive and it is aimed by the token, so what has to be
/// shown is the *extent*: everything under the root goes, and every byte
/// beside it stays. Both halves are asserted against a snapshot of the
/// whole parent tree rather than against a handful of named paths, so a
/// reclaim that reached one directory too far fails here rather than
/// somewhere downstream.
#[test]
fn a_reclaim_removes_the_token_root_and_nothing_outside_it() {
    let parent = scratch_tree::acquire(&std::env::temp_dir(), "extent").expect("the parent tree");

    // Outside the root under test, at two depths: a sibling directory with
    // content, and a file directly beside the root.
    let sibling = parent.path().join("sibling");
    fs::create_dir(&sibling).expect("sibling");
    fs::write(sibling.join("keep.txt"), b"outside the root").expect("sibling content");
    fs::write(parent.path().join("beside.txt"), b"beside the root").expect("parent content");

    let tree = scratch_tree::acquire(parent.path(), "target").expect("the tree under test");
    let root = tree.path().to_path_buf();
    let name = PathBuf::from(root.file_name().expect("the root has a name"));
    let deep = root.join("a").join("b").join("c");
    fs::create_dir_all(&deep).expect("nested directories");
    fs::write(deep.join("inside.txt"), b"inside the root").expect("nested content");
    fs::write(root.join("inside.txt"), b"inside the root").expect("root content");

    let before = snapshot_tree(parent.path());
    let expected: std::collections::BTreeMap<PathBuf, Vec<u8>> = before
        .iter()
        .filter(|(path, _)| !path.starts_with(&name))
        .map(|(path, bytes)| (path.clone(), bytes.clone()))
        .collect();
    assert!(
        expected.len() < before.len(),
        "the token root held no files, so this witness would pass on a reclaim that \
             removed nothing"
    );
    assert!(
        !expected.is_empty(),
        "nothing was outside the root to preserve"
    );

    scratch_tree::remove_scratch_tree(tree.disarm()).expect("the tree is reclaimed");

    assert!(
        scratch_tree::proves_absent(&root),
        "the token root is gone, proved rather than assumed"
    );
    assert_eq!(
        snapshot_tree(parent.path()),
        expected,
        "the reclaim removed something outside its token root, or left something inside it"
    );
}

/// Witness 7 — a scratch tree carrying a published `committed.json` is
/// reclaimed through the scratch funnel, while the ownership proof over the
/// same bytes refuses.
///
/// This is the whole reason there are two tokens. The same directory is
/// looked at by both authorities and they answer differently, correctly:
///
/// * `prove_private_half_ownership` answers
///   [`RetainReason::PossiblyCommitted`]. Conjunct 12 is unmoved and
///   fail-closed, so no run-lifecycle path — census or creator — deletes
///   that private half, ever.
/// * `scratch_tree::remove_scratch_tree` reclaims the tree the fixture was
///   built in, because its authority is not about the contents at all: the
///   root did not exist before `acquire` created it, so the `committed.json`
///   inside it is a record this test published seconds ago rather than a
///   run's deletion boundary.
///
/// Routing the fixture's cleanup through `PrivateHalfProof` instead would
/// require either forging that token or weakening conjunct 12 — and the
/// conjunct-12 tests are precisely the ones that need a fixture in this
/// shape.
#[test]
fn a_scratch_tree_holding_a_committed_record_is_reclaimed_while_the_proof_refuses_it() {
    let tree = scratch_tree::acquire(&std::env::temp_dir(), "committed")
        .expect("the scratch tree the fixture is built in");
    let husk = BoundHusk::at(tree.path().to_path_buf());
    husk.publish();

    let hooks = &mut NoHooks;
    stage_commit_record(&husk.private, &commit_record_of(&husk), hooks).expect("P5a");
    publish_commit_record(&husk.private, hooks).expect("P5b");
    let record = husk.private.join(COMMIT_RECORD);
    assert!(record.is_file(), "the fixture published a commit record");

    // The run-lifecycle authority refuses, and refuses for the boundary's
    // own reason rather than for some incidental defect in the fixture.
    match husk.prove() {
        PrivateHalfOwnership::Retained(RetainReason::PossiblyCommitted) => {}
        other => {
            panic!("past P5b the ownership proof mints no token for this private half: {other:?}")
        }
    }
    assert_eq!(
        commit_record_after_error(&husk.private),
        CommitRecordPresence::Present
    );
    assert!(
        !commit_record_after_error(&husk.private).permits_deletion(),
        "the creator's half of the boundary agrees with the census's"
    );

    // And the scratch authority reclaims the same bytes.
    let root = tree.path().to_path_buf();
    assert!(husk.private.starts_with(&root) || husk.root.starts_with(&root));
    scratch_tree::remove_scratch_tree(tree.disarm())
        .expect("a tree the token minted is reclaimed whatever a fixture published in it");
    assert!(scratch_tree::proves_absent(&root), "the token root is gone");
    assert!(
        scratch_tree::proves_absent(&record),
        "and so is the record the fixture published in it"
    );
}

// =======================================================================
// The refusal that is a build failure
// =======================================================================

/// The fixture that must compile, so a refusal below is a refusal rather
/// than a broken rustc invocation.
const CONTROL: &str = r#"
        extern crate upstroke;
        use std::path::Path;
        pub fn control(public: &Path, hooks: &mut upstroke::rundir::NoHooks) {
            let _ = upstroke::rundir::classify_run_dir(public);
            let _ = upstroke::rundir::remove_public_husk(public, hooks);
        }
"#;

struct BuildRefusal {
    name: &'static str,
    source: &'static str,
    /// rustc's own error code. A fixture that only asserted "this does not
    /// compile" is green when it fails for a typo.
    codes: &'static [&'static str],
    names: &'static str,
}

fn build_refusals() -> Vec<BuildRefusal> {
    vec![
        BuildRefusal {
            name: "no-proof",
            source: r#"
        extern crate upstroke;
        pub fn delete(hooks: &mut upstroke::rundir::NoHooks) {
            let _ = upstroke::rundir::remove_private_husk(hooks);
        }
"#,
            codes: &["E0061"],
            names: "remove_private_husk",
        },
        BuildRefusal {
            name: "wrong-token",
            source: r#"
        extern crate upstroke;
        use std::path::PathBuf;
        pub fn delete(hooks: &mut upstroke::rundir::NoHooks) {
            let _ = upstroke::rundir::remove_private_husk(PathBuf::from("/tmp/x"), hooks);
        }
"#,
            codes: &["E0308"],
            names: "PrivateHalfProof",
        },
        BuildRefusal {
            name: "forged-token",
            source: r#"
        extern crate upstroke;
        use std::path::PathBuf;
        pub fn forge() -> upstroke::rundir::PrivateHalfProof {
            upstroke::rundir::PrivateHalfProof {
                target: PathBuf::new(),
                public: PathBuf::new(),
                run_id: String::new(),
            }
        }
"#,
            codes: &["E0451", "E0603", "E0063"],
            names: "PrivateHalfProof",
        },
        BuildRefusal {
            name: "cloned-token",
            source: r#"
        extern crate upstroke;
        pub fn twice(proof: upstroke::rundir::PrivateHalfProof) -> upstroke::rundir::PrivateHalfProof {
            let copy = proof.clone();
            copy
        }
"#,
            codes: &["E0599"],
            names: "PrivateHalfProof",
        },
        BuildRefusal {
            name: "defaulted-token",
            source: r#"
        extern crate upstroke;
        pub fn out_of_nothing() -> upstroke::rundir::PrivateHalfProof {
            upstroke::rundir::PrivateHalfProof::default()
        }
"#,
            codes: &["E0599"],
            names: "PrivateHalfProof",
        },
        BuildRefusal {
            name: "spent-token",
            source: r#"
        extern crate upstroke;
        pub fn twice(proof: upstroke::rundir::PrivateHalfProof, hooks: &mut upstroke::rundir::NoHooks) {
            let _ = upstroke::rundir::remove_private_husk(proof, hooks);
            let _ = upstroke::rundir::remove_private_husk(proof, hooks);
        }
"#,
            codes: &["E0382"],
            names: "proof",
        },
    ]
}

/// This crate's rlib, beside the test binary that is running.
fn this_crates_rlib(deps: &Path) -> PathBuf {
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in fs::read_dir(deps).expect("the deps directory").flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with("libupstroke-") || !name.ends_with(".rlib") {
            continue;
        }
        let when = entry
            .metadata()
            .and_then(|meta| meta.modified())
            .expect("mtime");
        if best.as_ref().is_none_or(|(seen, _)| when > *seen) {
            best = Some((when, entry.path()));
        }
    }
    best.expect("this crate's rlib is beside its test binary").1
}

fn compile_against_this_crate(tag: &str, source: &str) -> (bool, Vec<String>, String) {
    let dir = scratch(&format!("compile-{tag}"));
    let file = dir.join("fixture.rs");
    fs::write(&file, source).expect("fixture source");
    let deps = std::env::current_exe()
        .expect("test binary")
        .parent()
        .expect("deps directory")
        .to_path_buf();
    let rlib = this_crates_rlib(&deps);
    let out = std::process::Command::new("rustc")
        .args([
            "--edition",
            "2024",
            "--crate-type",
            "lib",
            "--emit",
            "metadata",
        ])
        .arg("--extern")
        .arg(format!("upstroke={}", rlib.display()))
        .arg("-L")
        .arg(format!("dependency={}", deps.display()))
        .args(["--error-format", "json"])
        .arg("--out-dir")
        .arg(&dir)
        .arg(&file)
        .output()
        .expect("rustc runs; a missing rustc is a failure of this test, never a skip");
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    let mut codes = Vec::new();
    for line in stderr.lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if value["level"] != "error" {
            continue;
        }
        if let Some(code) = value["code"]["code"].as_str() {
            codes.push(code.to_owned());
        }
    }
    (out.status.success(), codes, stderr)
}

#[test]
fn a_private_half_deletion_without_a_proof_does_not_compile_for_the_stated_reason() {
    // `resource_accounting.completeness_rule`: "a private-half deletion
    // outside the proof-token funnel fails to compile". A fixture that
    // only asserted the failure would be green for a typo, so every case
    // pins rustc's own error code and the identifier its message must
    // name — and the control proves the harness compiles anything at all.
    let (ok, codes, rendered) = compile_against_this_crate("control", CONTROL);
    assert!(
        ok && codes.is_empty(),
        "the control fixture must compile, or every refusal below is meaningless:\n{rendered}"
    );

    for case in build_refusals() {
        let (ok, codes, rendered) = compile_against_this_crate(case.name, case.source);
        assert!(!ok, "`{}` must not compile", case.name);
        assert!(
            codes.iter().any(|code| case.codes.contains(&code.as_str())),
            "`{}`: expected one of {:?}, got {codes:?}\n{rendered}",
            case.name,
            case.codes
        );
        assert!(
            rendered.contains(case.names),
            "`{}`: the message must name `{}`:\n{rendered}",
            case.name,
            case.names
        );
    }
}

// =======================================================================
// The repository key
// =======================================================================

#[test]
fn the_repo_key_is_the_construction_the_packet_states() {
    // `workspace_candidates.execution_root`: "repo_key v1 =
    // hex16(sha256('upstroke-repo-key-v1' NUL canonical common git dir
    // bytes))". The expected value is computed from that sentence here,
    // and for a fixed path it is a literal computed outside this program
    // entirely — a function may not be its own oracle.
    let dir = scratch("repokey").join("git-dir");
    fs::create_dir_all(&dir).expect("git dir");
    let canonical = fs::canonicalize(&dir).expect("canonical");
    let mut bytes = b"upstroke-repo-key-v1".to_vec();
    bytes.push(0);
    bytes.extend_from_slice(canonical.as_os_str().as_encoded_bytes());
    let expected: String = format!("{:x}", Sha256::digest(&bytes))
        .chars()
        .take(16)
        .collect();
    assert_eq!(RepoKey::v1(&canonical).as_str(), expected);
    assert_eq!(expected.len(), 16, "hex16 is sixteen hex characters");

    #[cfg(unix)]
    assert_eq!(
        RepoKey::v1(Path::new("/srv/repo/.git")).as_str(),
        "e43114efb48428eb",
        "sha256(b'upstroke-repo-key-v1\\x00/srv/repo/.git')[:16], computed elsewhere"
    );

    // Distinguishing, which is the whole job: two repositories, two keys.
    assert_ne!(
        RepoKey::v1(Path::new("/srv/a/.git")),
        RepoKey::v1(Path::new("/srv/b/.git"))
    );
}

fn git(cwd: &Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?} failed in {}", cwd.display());
}

#[test]
fn every_worktree_of_one_repository_has_one_repo_key() {
    // A run created in the main checkout and a census run from a linked
    // worktree must not call each other foreign, so the key is taken over
    // the **common** git dir. A linked worktree's own git dir is
    // `<common>/worktrees/<name>`, and this proves the derivation against
    // a real one rather than against the rule that produced it.
    let root = scratch("worktreekey");
    let main = root.join("main");
    fs::create_dir_all(&main).expect("main");
    git(&main, &["init", "-q", "-b", "main"]);
    git(&main, &["config", "user.email", "t@example.invalid"]);
    git(&main, &["config", "user.name", "t"]);
    fs::write(main.join("f"), "x").expect("file");
    git(&main, &["add", "f"]);
    git(&main, &["commit", "-q", "-m", "one"]);
    let linked = root.join("linked");
    git(
        &main,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "linked",
            linked.to_str().expect("utf-8"),
        ],
    );

    let main_git_dir = Workspace::open(&main)
        .expect("main workspace")
        .worktree_git_dir()
        .expect("main git dir");
    let linked_git_dir = Workspace::open(&linked)
        .expect("linked workspace")
        .worktree_git_dir()
        .expect("linked git dir");
    assert_ne!(
        main_git_dir, linked_git_dir,
        "the two worktrees really do have different git dirs, so the \
             common-dir derivation is doing work"
    );
    assert!(
        linked_git_dir.parent().and_then(Path::file_name)
            == Some(std::ffi::OsStr::new("worktrees")),
        "the layout this derivation reads: {}",
        linked_git_dir.display()
    );
    assert_eq!(
        RepoKey::for_repo(&main).expect("main key"),
        RepoKey::for_repo(&linked).expect("linked key"),
        "one repository, one key"
    );
}

// =======================================================================
// The wire the marker and the records are read back off
// =======================================================================

/// Every field the packet names for each record, written by hand.
fn marker_json() -> serde_json::Value {
    serde_json::json!({
        "run_id": "01RUN",
        "repo_key": "0123456789abcdef",
        "private_dir": "/private/runs/01RUN",
        "incarnation": "01INC",
        "pid": 4242,
        "runner_policy_sha256": "sha256:aa"
    })
}

fn owner_json() -> serde_json::Value {
    serde_json::json!({
        "run_id": "01RUN",
        "repo_key": "0123456789abcdef",
        "public_dir": "/repo/.upstroke/runs/01RUN",
        "incarnation": "01INC",
        "runner": {
            "kind": "host",
            "policy": "host-v1",
            "image": null,
            "credential_volumes": null
        }
    })
}

fn commit_json() -> serde_json::Value {
    serde_json::json!({
        "run_id": "01RUN",
        "repo_key": "0123456789abcdef",
        "public_dir": "/repo/.upstroke/runs/01RUN",
        "incarnation": "01INC",
        "run_started_sha256": "sha256:bb"
    })
}

#[test]
fn each_record_carries_exactly_the_fields_the_packet_names() {
    // Mutation witnessing cannot detect a field that was never written, so
    // this is a transcription check rather than a round trip: the payloads
    // are written out of `run_creation` and `resource_accounting` by hand,
    // every named field is asserted required, and an unknown one is
    // refused because a marker is what a census decides a deletion from.
    for (what, payload, fields) in [
        (
            "marker",
            marker_json(),
            vec![
                "run_id",
                "repo_key",
                "private_dir",
                "incarnation",
                "pid",
                "runner_policy_sha256",
            ],
        ),
        (
            "owner record",
            owner_json(),
            vec!["run_id", "repo_key", "public_dir", "incarnation", "runner"],
        ),
        (
            "commit record",
            commit_json(),
            vec![
                "run_id",
                "repo_key",
                "public_dir",
                "incarnation",
                "run_started_sha256",
            ],
        ),
    ] {
        let parses = match what {
            "marker" => serde_json::from_value::<CreatingMarker>(payload.clone()).is_ok(),
            "owner record" => serde_json::from_value::<OwnerRecord>(payload.clone()).is_ok(),
            _ => serde_json::from_value::<CommitRecord>(payload.clone()).is_ok(),
        };
        assert!(parses, "{what}: the packet's own payload must parse");

        assert_eq!(
            payload.as_object().expect("object").len(),
            fields.len(),
            "{what}: the packet names {} fields",
            fields.len()
        );

        for missing in &fields {
            let mut short = payload.clone();
            short.as_object_mut().expect("object").remove(*missing);
            let refused = match what {
                "marker" => serde_json::from_value::<CreatingMarker>(short).is_err(),
                "owner record" => serde_json::from_value::<OwnerRecord>(short).is_err(),
                _ => serde_json::from_value::<CommitRecord>(short).is_err(),
            };
            assert!(refused, "{what}: `{missing}` must be required");
        }

        let mut extra = payload.clone();
        extra
            .as_object_mut()
            .expect("object")
            .insert("unknown".to_owned(), serde_json::json!(1));
        let refused = match what {
            "marker" => serde_json::from_value::<CreatingMarker>(extra).is_err(),
            "owner record" => serde_json::from_value::<OwnerRecord>(extra).is_err(),
            _ => serde_json::from_value::<CommitRecord>(extra).is_err(),
        };
        assert!(refused, "{what}: an unknown field is refused");
    }
}

#[test]
fn what_the_funnels_write_is_what_the_packet_says_they_write() {
    // The other direction: the bytes on disk, compared against the
    // independently written payloads above rather than against whatever
    // this build happens to serialize.
    let root = scratch("wire");
    let dir = root.join("half");
    fs::create_dir_all(&dir).expect("dir");
    let marker: CreatingMarker =
        serde_json::from_value(marker_json()).expect("the packet's marker");
    stage_marker(&dir, &marker, &mut NoHooks).expect("stage");
    publish_marker(&dir, &mut NoHooks).expect("publish");
    let written: serde_json::Value =
        serde_json::from_slice(&fs::read(dir.join(MARKER)).expect("read")).expect("json");
    assert_eq!(written, marker_json());

    let owner: OwnerRecord = serde_json::from_value(owner_json()).expect("the packet's owner");
    stage_owner_record(&dir, &owner, &mut NoHooks).expect("stage");
    publish_owner_record(&dir, &mut NoHooks).expect("publish");
    let written: serde_json::Value =
        serde_json::from_slice(&fs::read(dir.join(OWNER_RECORD)).expect("read")).expect("json");
    assert_eq!(written, owner_json());

    let record: CommitRecord =
        serde_json::from_value(commit_json()).expect("the packet's commit record");
    stage_commit_record(&dir, &record, &mut NoHooks).expect("stage");
    publish_commit_record(&dir, &mut NoHooks).expect("publish");
    let written: serde_json::Value =
        serde_json::from_slice(&fs::read(dir.join(COMMIT_RECORD)).expect("read")).expect("json");
    assert_eq!(written, commit_json());
}

/// `run_started_sha256` and `events::log::first_line_digest` are two spellings
/// of one number, and recovery compares a value produced by each.
///
/// `create.rs` writes `committed.json`'s `run_started_sha256` from
/// `first_line_digest` at P5b; recovery step (a) recomputes it with
/// `rundir::run_started_sha256` over the line it replayed and refuses the run if
/// the two disagree. Nothing asserted that they agree, and every test on either
/// side computes its oracle with the same function it is testing -- so either
/// `format!` could change alone and every schema-4 recovery would start refusing
/// with two digests that each looked correct where they were computed.
///
/// The newline is the whole of the difference between the two signatures:
/// `first_line_digest` is given a log and finds the line, `run_started_sha256`
/// is given the line. This asserts the pairing production actually makes.
#[test]
fn the_two_spellings_of_the_first_line_digest_agree() {
    let line = committed_line("01DIGEST", 4);
    let mut log = line.clone().into_bytes();
    log.push(b'\n');
    // A second event after it, so a digest over the whole file and a digest over
    // the first line are different numbers.
    log.extend_from_slice(b"{\"ts\":\"2026-08-20T00:00:01Z\",\"event\":\"noise\"}\n");
    assert_eq!(
        crate::events::log::first_line_digest(&log),
        Some(run_started_sha256(line.as_bytes())),
        "the digest the commit record is written from and the one recovery recomputes are one \
         number"
    );
    assert_ne!(
        crate::events::log::first_line_digest(&log),
        Some(run_started_sha256(&log)),
        "a digest over the whole log would satisfy the assertion above for the wrong reason"
    );
}

#[test]
fn the_commit_records_digest_is_over_the_exact_line_bytes() {
    // `run_creation`: "run_started_sha256 = the digest of the exact
    // run_started line bytes about to be appended". Pinned against a
    // digest computed outside this program.
    assert_eq!(
        run_started_sha256(b"abc"),
        "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        "the FIPS-180-2 example digest of `abc`"
    );
    // The newline is part of the line and therefore part of the digest.
    assert_ne!(run_started_sha256(b"abc"), run_started_sha256(b"abc\n"));

    // **A real `run_started` line, spelled noncanonically**
    // (`PR5-RUNDIR-053`). Neither input above is JSON at all, so a digest
    // computed over a *reserialized* event value falls straight back to the
    // exact bytes for both and every assertion above still holds. The only
    // input that separates the two rules is a valid line whose whitespace
    // and key order are not what a serializer would emit, and the digest it
    // must have is computed outside this program:
    //
    //   python3 -c "import hashlib,sys;
    //     print(hashlib.sha256(open(sys.argv[1],'rb').read()).hexdigest())"
    // Built by concatenation rather than as one wrapped literal: a `\` line
    // continuation eats the indentation that follows it and rustfmt then joins
    // the line, so the bytes such a literal produces are not the bytes it looks
    // like — and which exact bytes are digested is the whole of this fixture.
    let mut noncanonical: Vec<u8> = Vec::new();
    noncanonical
        .extend_from_slice(b"{\"ts\":\"2026-08-20T00:00:00Z\" ,  \"event\":\"run_started\",");
    noncanonical.extend_from_slice(b" \"data\" : {\"run_id\":\"01NONCANON\", \"schema\":3}}\n");
    let noncanonical: &[u8] = &noncanonical;
    assert_eq!(
        serde_json::from_slice::<RunStartedHeader>(&noncanonical[..noncanonical.len() - 1])
            .expect("the fixture really is a parseable run_started")
            .event,
        "run_started",
        "a fixture a reserializing digest could not be applied to would prove nothing"
    );
    assert_eq!(
        run_started_sha256(noncanonical),
        "sha256:e0d7e8c55c48fb6c62fd452e4fa95b0a2ceebd60d0375120d35dde1fcd1fb8d9",
        "the digest of these exact bytes, including the terminating newline"
    );
    // And the two rules really do differ here, so the assertion above is
    // not passing for want of a distinction: the canonical reserialization
    // of the same value has a different digest.
    let canonical = serde_json::to_vec(
        &serde_json::from_slice::<serde_json::Value>(&noncanonical[..noncanonical.len() - 1])
            .expect("valid json"),
    )
    .expect("reserialize");
    assert_ne!(
        run_started_sha256(&canonical),
        run_started_sha256(noncanonical),
        "the fixture does not separate exact-bytes from reserialized"
    );
}

/// A `.partial` is writer-owned staging residue that **no reader ingests**
/// and no ingestion consumes (`PR5-RUNDIR-060`, `PR5-RUNDIR-061`).
///
/// `transaction_fault_matrix[17].resume_action` says both halves: "a
/// `.partial` file is writer-owned staging residue: **ignored by every
/// reader** and never pruned by the coordinator", and "the file itself is
/// **persistent run-directory content (R21) in every case**". Neither had a
/// fixture. The only driver of `ingest_answer` staged, published and
/// ingested back to back, so the shape this test builds — a valid partial
/// and *no* published answer — never existed, and nothing ever read a
/// published answer twice or looked for it afterwards. A reader that fell
/// back to the partial, and a reader that consumed its input, both looked
/// exactly like a correct one.
#[test]
fn a_staged_partial_is_never_ingested_and_a_published_answer_survives_ingestion() {
    let root = scratch("answer-residue");
    let answers = root.join("answers");
    create_dir(&answers).expect("answers");

    // (a) A valid partial, and nothing published.
    stage_answer(
        &answers,
        "q-1",
        &serde_json::json!({"text": "staged"}),
        &mut NoHooks,
    )
    .expect("stage");
    let partial = answers.join("q-1.json.partial");
    let staged_bytes = fs::read(&partial).expect("the partial exists");
    assert!(
        serde_json::from_slice::<serde_json::Value>(&staged_bytes).is_ok(),
        "the partial is valid JSON, so a fallback reader would happily return it"
    );
    assert!(
        !answers.join("q-1.json").exists(),
        "and nothing is published, which is the state the entry is about"
    );

    assert_eq!(
        ingest_answer(&answers, "q-1", &mut NoHooks).expect("ingest"),
        None,
        "a reader that fell back to the partial would answer with staging residue"
    );
    assert_eq!(
        fs::read(&partial).expect("the partial"),
        staged_bytes,
        "and the read-only ingestion left the partial byte-identical"
    );

    // (b) Published, ingested — and still there afterwards.
    publish_answer(&answers, "q-1", &mut NoHooks).expect("publish");
    let published = answers.join("q-1.json");
    let published_bytes = fs::read(&published).expect("the published answer");
    let first = ingest_answer(&answers, "q-1", &mut NoHooks).expect("ingest");
    assert!(
        first.is_some(),
        "the published answer is what a reader gets"
    );
    assert!(
        published.is_file(),
        "R21 is persistent run-directory content: ingestion is a read, not a take"
    );
    assert_eq!(
        fs::read(&published).expect("the published answer"),
        published_bytes,
        "with its original bytes"
    );
    assert_eq!(
        ingest_answer(&answers, "q-1", &mut NoHooks).expect("ingest again"),
        first,
        "so a second reader gets the same answer as the first"
    );
}

/// Whatever an operator left at the old fixed staging name `report.json.tmp`
/// — a hard link to a note outside the run directory, a symbolic link, a
/// directory — is left exactly as found, and the report is published beside
/// it: since PR10's round 9 the report is staged inside a directory the
/// write makes for itself (under a name unique to the write and recorded in
/// the private half since round 10, `REPORT_STAGING_PREFIX`; a fixed
/// directory in round 9; a name unique to the write in round 8), so no name
/// that is somebody else's is ever opened, truncated, written through or
/// removed (the round-6 regression lens, P2-1, whose alias round 6 refused
/// by link count; the round-8 one, P2, whose single-link draft round 6
/// removed as the writer's own).
#[test]
fn report_staging_leaves_whatever_is_at_the_old_fixed_name_as_found() {
    let root = scratch("report-stage-old-name");
    let public = root.join("public");
    create_dir(&public).expect("public directory");
    let private = root.join("private");
    create_dir(&private).expect("private directory");
    let note = root.join("operator-note.txt");
    fs::write(&note, b"keep me\n").expect("operator note");
    let old_name = public.join("report.json.tmp");
    let payload = serde_json::json!({"outcome": "parked"});

    fs::hard_link(&note, &old_name).expect("an alias at the old fixed name");
    write_report(&public, &private, &payload, &mut NoHooks)
        .expect("the report is published beside it");
    assert_eq!(
        fs::read(&note).expect("operator note"),
        b"keep me\n".to_vec(),
        "the operator's file is byte-identical: nothing staged through the alias"
    );
    assert!(
        old_name.is_file() && public.join(REPORT).is_file(),
        "the alias stands and the report was published"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        assert_eq!(
            fs::metadata(&old_name).expect("the alias").nlink(),
            2,
            "still one of the operator's two names"
        );
    }
    fs::remove_file(&old_name).expect("the operator's alias removed for the next case");

    create_dir(&old_name).expect("a directory at the old fixed name");
    write_report(&public, &private, &payload, &mut NoHooks)
        .expect("the report is published beside it");
    assert!(old_name.is_dir(), "the directory stands");
    fs::remove_dir(&old_name).expect("removed for the next case");

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&note, &old_name).expect("a symlink at the old fixed name");
        write_report(&public, &private, &payload, &mut NoHooks)
            .expect("the report is published beside it");
        assert!(
            fs::symlink_metadata(&old_name)
                .expect("the link")
                .file_type()
                .is_symlink(),
            "the link stands, unfollowed"
        );
        assert_eq!(
            fs::read(&note).expect("operator note"),
            b"keep me\n".to_vec()
        );
    }
    assert!(
        report_staging_leftovers(&public, &private)
            .expect("listed")
            .is_empty(),
        "and nothing of the report's own protocol is left staged"
    );
}

/// The round-8 and round-9 regression lenses' recipe: a single-link draft an
/// operator left under a schema-3 run survives the next report write byte for
/// byte, whatever its name — at the old fixed name `report.json.tmp`, which
/// round 6's `clear_stale_staging` read as the writer's own and unlinked; at
/// `report.json.ZZZZZZZZZZZZZZZZZZZZZZZZZZ.tmp`, a name the ULID producer
/// cannot emit and round 8's recogniser accepted and removed; and at a name
/// the producer could have emitted, which no recogniser can tell from the
/// writer's own. Nothing at any name beside the report is this writer's to
/// reason about (`standards/08`: cleanup removes only what the operation can
/// prove it owns, never inferred from a shared filename): the write stages
/// inside a directory it makes for itself and touches nothing else.
#[test]
fn legacy_report_preserves_unowned_staging_name() {
    let root = scratch("report-unowned-staging-name");
    let public = root.join("public");
    create_dir(&public).expect("public");
    let private = root.join("private");
    create_dir(&private).expect("private");
    let payload = serde_json::json!({"outcome": "parked"});
    for name in [
        "report.json.tmp",
        "report.json.ZZZZZZZZZZZZZZZZZZZZZZZZZZ.tmp",
        "report.json.01M191Y2PSP8400DBF5QSFFJT3.tmp",
    ] {
        let draft = public.join(name);
        fs::write(&draft, b"operator draft\n").expect("an operator draft at the name");
        write_report(&public, &private, &payload, &mut NoHooks)
            .expect("the report is published beside the draft");
        assert_eq!(
            fs::read(&draft).expect("the operator's draft is still there"),
            b"operator draft\n".to_vec(),
            "an operator's file at `{name}` is not this writer's to remove"
        );
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(
                &fs::read(public.join(REPORT)).expect("report")
            )
            .expect("json"),
            payload,
            "{name}: and the report was published"
        );
    }
}

/// The round-10 regression and record lenses' recipe: what an operator left
/// under a schema-3 run directory in the report's staging shape survives the
/// next report write byte for byte, whether it is a whole `.report-staging/`
/// (round 9's fixed name, which round 9 removed by its name and type before
/// the write's own exclusive creation — proof of ownership of the
/// replacement, and of nothing that was deleted) or a directory wearing this
/// round's shape, `.report-staging-<ulid>`, that no record of this run's
/// names. Neither is opened, removed or adopted: the write records its own
/// fresh name in the private half, makes its own directory, publishes
/// beside them, and hands the passed-over shapes back by name
/// (`standards/08`: cleanup removes only what the operation can prove it
/// owns, and the proof here is the record, never the name).
#[test]
fn legacy_report_preserves_unowned_staging_directory() {
    let root = scratch("report-unowned-staging-directory");
    let public = root.join("public");
    create_dir(&public).expect("public");
    let private = root.join("private");
    create_dir(&private).expect("private");
    let payload = serde_json::json!({"outcome": "parked"});

    let fixed = public.join(".report-staging");
    create_dir(&fixed).expect("an operator's directory at round 9's fixed name");
    let fixed_note = fixed.join("operator-note.txt");
    fs::write(&fixed_note, b"operator draft\n").expect("the operator's note inside it");
    let shaped = public.join("report-staging-shaped");
    let shaped = shaped.with_file_name(".report-staging-01M191Y2PSP8400DBF5QSFFJT3");
    create_dir(&shaped).expect("a directory of this round's shape, with no record naming it");
    let shaped_note = shaped.join(REPORT);
    fs::write(&shaped_note, b"{\"operator\":").expect("whatever it holds");
    assert_eq!(
        unrecorded_report_staging(&public, &private).expect("listed"),
        vec![shaped.clone()],
        "the shaped directory is listed as unrecorded; the fixed-name directory is not of this \
         round's shape at all"
    );

    let passed_over = write_report(&public, &private, &payload, &mut NoHooks)
        .expect("the report is published beside both");
    assert_eq!(
        passed_over,
        vec![shaped.clone()],
        "the write names the shaped directory it passed over, and removes nothing it did not \
         record"
    );
    assert_eq!(
        fs::read(&fixed_note).expect("the operator's note is still there"),
        b"operator draft\n".to_vec(),
        "a directory at round 9's fixed name is not this writer's to remove"
    );
    assert_eq!(
        fs::read(&shaped_note).expect("the shaped directory's file is still there"),
        b"{\"operator\":".to_vec(),
        "a directory of this round's shape that no record names is not this writer's to remove"
    );
    assert!(
        fixed.is_dir() && shaped.is_dir(),
        "both directories stand, as found"
    );
    assert!(
        report_staging_leftovers(&public, &private)
            .expect("listed")
            .is_empty(),
        "nothing of the protocol's own is left: the write's record and directory went with the \
         rename"
    );
    assert_eq!(
        unrecorded_report_staging(&public, &private).expect("listed"),
        vec![shaped.clone()],
        "and the shaped directory is still there to be passed over next time"
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(
            &fs::read(public.join(REPORT)).expect("report")
        )
        .expect("json"),
        payload,
        "and the report was published"
    );

    #[cfg(unix)]
    {
        let note = root.join("operator-note.txt");
        fs::write(&note, b"keep me\n").expect("operator note");
        let link = public.join(".report-staging-01M191Y2PSP8400DBF5QSFFJT4");
        std::os::unix::fs::symlink(&note, &link).expect("a link wearing the shape");
        let passed_over = write_report(&public, &private, &payload, &mut NoHooks)
            .expect("the report is published beside it");
        assert_eq!(
            passed_over,
            vec![shaped.clone(), link.clone()],
            "a link wearing the shape is passed over by name too, never followed"
        );
        assert!(
            fs::symlink_metadata(&link)
                .expect("the link")
                .file_type()
                .is_symlink()
                && fs::read(&note).expect("operator note") == b"keep me\n".to_vec(),
            "the link and what it names are as found"
        );
    }
}

/// What a report writer that died between its stage and its rename leaves —
/// the record of its staging directory's name in the private half, the
/// directory under the public run directory, and the half-written report
/// inside — is reclaimed by the next write **by the record**: the directory
/// the record names is removed as the run's own tree, then the record, before
/// that write makes its own, under the run lock every writer holds. The
/// leftover is planted through the writer's own record-then-create helper
/// (`plant_report_staging_of_a_dead_writer`), never by hand — a directory
/// planted by hand carries no record and is not the protocol's, which is the
/// contrast the second half of this test draws: a hand-planted directory of
/// the same shape survives the same write byte for byte. A record whose
/// directory is already gone is removed on its own; something that is not a
/// directory standing at a recorded name is left as found and the record
/// removed. Until PR10's round 10 the fixture made the directory by hand at a
/// fixed name and the writer removed whatever directory stood there (the
/// round-10 fix-check and crash lenses, P2).
#[test]
fn a_dead_writers_staged_report_is_reclaimed_by_the_next_write() {
    let root = scratch("report-dead-writer");
    let public = root.join("public");
    create_dir(&public).expect("public");
    let private = root.join("private");
    create_dir(&private).expect("private");
    let payload = serde_json::json!({"outcome": "parked"});

    let leftover = plant_report_staging_of_a_dead_writer(&public, &private);
    let staging = leftover
        .parent()
        .expect("the staged file is inside the staging directory")
        .to_path_buf();
    let record = report_staging_record(&private);
    assert_eq!(
        report_staging_leftovers(&public, &private).expect("listed"),
        vec![record.clone(), staging.clone(), leftover.clone()],
        "the dead writer's record, the directory it names, and what the directory holds"
    );
    assert_eq!(
        recorded_report_staging(&public, &private).expect("read"),
        Some(staging.clone()),
        "the record names the directory the helper made"
    );
    let by_hand = public.join(".report-staging-01M191Y2PSP8400DBF5QSFFJT3");
    create_dir(&by_hand).expect("a directory of the same shape, planted by hand: no record");
    let by_hand_file = by_hand.join(REPORT);
    fs::write(&by_hand_file, b"{\"operator\":").expect("whatever it holds");
    assert_eq!(
        unrecorded_report_staging(&public, &private).expect("listed"),
        vec![by_hand.clone()],
        "the hand-planted directory is the unrecorded one; the recorded one is not listed here"
    );

    let passed_over =
        write_report(&public, &private, &payload, &mut NoHooks).expect("the report is published");
    assert!(
        !leftover.exists() && !staging.exists() && !record.exists(),
        "the dead writer's directory is reclaimed as the run's own tree, by its record, and the \
         record with it; this write's own record and directory are gone with the rename"
    );
    assert_eq!(
        passed_over,
        vec![by_hand.clone()],
        "the hand-planted directory is passed over by name"
    );
    assert_eq!(
        fs::read(&by_hand_file).expect("the hand-planted directory's file is still there"),
        b"{\"operator\":".to_vec(),
        "a directory no record names survives the write byte for byte"
    );
    assert!(
        report_staging_leftovers(&public, &private)
            .expect("listed")
            .is_empty(),
        "nothing of the protocol is left staged"
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(
            &fs::read(public.join(REPORT)).expect("report")
        )
        .expect("json"),
        payload,
        "and the published report is the payload, not the stale bytes"
    );
    fs::remove_dir_all(&by_hand).expect("the hand-planted directory removed for the next case");

    // A record whose directory is already gone: removed on its own.
    let leftover = plant_report_staging_of_a_dead_writer(&public, &private);
    let staging = leftover
        .parent()
        .expect("the staged file is inside the staging directory")
        .to_path_buf();
    fs::remove_dir_all(&staging).expect("the recorded directory gone before the next write");
    assert_eq!(
        report_staging_leftovers(&public, &private).expect("listed"),
        vec![record.clone()],
        "the record alone is what the protocol left"
    );
    let passed_over =
        write_report(&public, &private, &payload, &mut NoHooks).expect("the report is published");
    assert!(
        passed_over.is_empty() && !record.exists(),
        "a record naming a directory that is gone is removed on its own, and nothing is passed over"
    );

    // Something that is not a directory standing at a recorded name: not
    // what this writer made, left as found, the record removed, and named.
    let leftover = plant_report_staging_of_a_dead_writer(&public, &private);
    let staging = leftover
        .parent()
        .expect("the staged file is inside the staging directory")
        .to_path_buf();
    fs::remove_dir_all(&staging).expect("the recorded directory gone");
    fs::write(&staging, b"an operator's file at the recorded name\n").expect("a file at the name");
    let passed_over = write_report(&public, &private, &payload, &mut NoHooks)
        .expect("the report is published beside it");
    assert_eq!(
        (
            passed_over,
            fs::read(&staging).expect("the file stands"),
            record.exists()
        ),
        (
            vec![staging.clone()],
            b"an operator's file at the recorded name\n".to_vec(),
            false
        ),
        "the file is left as found and named, and the record that named the directory is gone"
    );
    fs::remove_file(&staging).expect("removed for the next case");

    write_report(&public, &private, &payload, &mut NoHooks)
        .expect("with nothing left, the write publishes");
    assert!(
        report_staging_leftovers(&public, &private)
            .expect("listed")
            .is_empty()
            && unrecorded_report_staging(&public, &private)
                .expect("listed")
                .is_empty()
            && public.join(REPORT).is_file()
    );
}

/// `PR10-RECLAIM-RECORD-DROPPED-BEFORE-DURABLE-DELETION`, the round-11 crash
/// lens's recipe, committed from the witness Gate 5 ran at `caf6bed0`, where it
/// failed: a dead writer left directory A and the record naming it; the next
/// write reclaims A, publishes the record of its own directory B in the private
/// half, makes B and dies at the staged file's barrier, before the publication's
/// barrier on the public directory; then a power loss undoes every change to the
/// public directory that no barrier made durable, while B's record, synced in
/// the private half, survives; then the retry. Until the fix the reclaim removed
/// A's record before any barrier on the public directory, so the loss restored A
/// with the record naming it already replaced, and the retry passed A over as
/// unrecorded for good. What the loss undoes is read off the faulted write's
/// own ledger rather than assumed. A's deletion survives only if a
/// `SyncedDirectory` record of the public directory carries A observed absent,
/// which is a barrier that held after the deletion (`sync_dir_observing`). B,
/// whose creation no later barrier made durable, is discarded. The gate's copy
/// restored A unconditionally, which after the fix models a loss undoing a
/// synced deletion and fails there as it failed before (the Gate 5 report's
/// review, round 2). This test's first model took any `SyncedDirectory` of the
/// public directory as proof, which a barrier moved ahead of the deletion writes
/// just the same. It passed under that mutation, the weakness that review's
/// round 3 found in the gate's corrected witness.
#[test]
fn a_reclaimed_report_stage_remains_reclaimable_after_power_loss() {
    let root = scratch("report-power-loss-reclaim");
    let public = root.join("public");
    create_dir(&public).expect("public");
    let private = root.join("private");
    create_dir(&private).expect("private");
    let payload = serde_json::json!({"outcome": "parked"});

    let leftover_a = plant_report_staging_of_a_dead_writer(&public, &private);
    let staging_a = leftover_a
        .parent()
        .expect("the staged file is inside the staging directory")
        .to_path_buf();
    let record = report_staging_record(&private);
    let a_bytes = fs::read(&leftover_a).expect("A's staged file");
    assert_eq!(
        recorded_report_staging(&public, &private).expect("read"),
        Some(staging_a.clone()),
        "premise: the record names A"
    );

    let mut hooks = HarnessHooks::default().recording_durability();
    let ledger = hooks.ledger();
    let fault = util::fail_file_barriers_within(&public);
    write_report(&public, &private, &payload, &mut hooks)
        .expect_err("the staged report's file barrier is refused");
    drop(fault);
    let staging_b = recorded_report_staging(&public, &private)
        .expect("read")
        .expect("record(B) was published before the fault");
    assert_ne!(staging_b, staging_a, "B is a fresh name");
    assert!(
        !staging_a.exists(),
        "A was removed before record(B) was published"
    );

    let records = ledger.records();
    let a_deletion_durable = records.iter().any(|entry| {
        entry.step == DurableStep::SyncedDirectory
            && entry.path == public
            && entry
                .entry
                .as_ref()
                .is_some_and(|seen| seen.path == staging_a && !seen.present)
    });
    let b_created = records
        .iter()
        .position(|entry| entry.step == DurableStep::DirectoryCreated && entry.path == staging_b)
        .expect("B's creation is recorded");
    assert!(
        records
            .iter()
            .skip(b_created)
            .all(|entry| !(entry.step == DurableStep::SyncedDirectory && entry.path == public)),
        "premise: no barrier of the public directory followed B's creation: {:?}",
        ledger.steps()
    );
    if !a_deletion_durable {
        fs::create_dir(&staging_a).expect("A restored");
        fs::write(&leftover_a, &a_bytes).expect("A's file restored");
    }
    if staging_b.exists() {
        fs::remove_dir_all(&staging_b).expect("B discarded");
    }
    assert!(record.is_file(), "record(B) survives in the private half");

    let passed_over =
        write_report(&public, &private, &payload, &mut NoHooks).expect("the retry publishes");
    assert!(
        !staging_a.exists(),
        "the old directory A, restored by the loss, is reclaimed by the retry"
    );
    assert!(
        passed_over.is_empty(),
        "nothing of the run's own making is passed over"
    );
}

/// The record outlives the public deletion it names when the public
/// directory's barrier is refused (`util::fail_barriers_at`), on each arm of the
/// reclaim and on the publication.
///
/// - A reclaim that removed a dead writer's directory A stops at its barrier
///   with the record still naming A.
/// - So does the next write, which finds A already gone and still takes the
///   barrier before the record goes, since an absent name is not proof that its
///   deletion reached one (#289's fix-check lens, round 1: with the barrier
///   inside the directory-present arm alone, the first attempt stopped and the
///   second dropped the record).
/// - So does a reclaim that finds something other than a directory at the
///   recorded name, which it leaves as found.
/// - A publication that renamed its report up and removed its emptied
///   directory B stops at its barrier with the record still naming B.
/// - The fresh branch of terminal finalization (`sync_report_dir`), the
///   reclaim's other caller, stops the same way on the directory it removed, on
///   the name found gone and on a file at the name. The Gate 5 report's review,
///   round 4, moved the reclaim's barrier after `begin_report_staging` in
///   `write_report` alone. That left this caller retiring the record before any
///   barrier, and no test that drives this caller failed.
///
/// Every deletion a loss undoes is therefore still recorded, and with the
/// barrier holding the next write reclaims by that record. Until the fix the
/// reclaim removed the record before any barrier and the publication before its
/// own, so a refused barrier left no record at all
/// (`PR10-RECLAIM-RECORD-DROPPED-BEFORE-DURABLE-DELETION`). The order removal,
/// barrier, record is what this holds: each of these fails here — the record
/// removed ahead of the barrier, the barrier's error discarded, the barrier
/// taken ahead of the removal, or the barrier confined to one arm.
#[test]
fn a_refused_public_barrier_leaves_the_record_of_the_staging_directory_it_removed() {
    let root = scratch("report-refused-public-barrier");
    let public = root.join("public");
    create_dir(&public).expect("public");
    let private = root.join("private");
    create_dir(&private).expect("private");
    let payload = serde_json::json!({"outcome": "parked"});
    let record = report_staging_record(&private);

    let leftover_a = plant_report_staging_of_a_dead_writer(&public, &private);
    let staging_a = leftover_a
        .parent()
        .expect("the staged file is inside the staging directory")
        .to_path_buf();
    let a_bytes = fs::read(&leftover_a).expect("A's staged file");
    {
        let _fault = util::fail_barriers_at(&public);
        for attempt in [
            "the reclaim that removed A",
            "the next write, which finds A already gone",
        ] {
            let error = write_report(&public, &private, &payload, &mut NoHooks)
                .expect_err("the reclaim's barrier is refused");
            assert!(
                error.to_string().contains("injected barrier fault"),
                "{attempt}: the failure is the barrier's, by name: {error}"
            );
            assert_eq!(
                (
                    staging_a.exists(),
                    recorded_report_staging(&public, &private).expect("read"),
                    public.join(REPORT).exists()
                ),
                (false, Some(staging_a.clone()), false),
                "{attempt}: stopped at the barrier with the record still naming A, before any \
                 directory of its own or any report"
            );
        }
    }
    fs::create_dir(&staging_a).expect("a loss undoes A's unsynced deletion");
    fs::write(&leftover_a, &a_bytes).expect("and restores its file");
    let passed_over = write_report(&public, &private, &payload, &mut NoHooks)
        .expect("with the barrier holding, published");
    assert!(
        passed_over.is_empty() && !staging_a.exists() && !record.exists(),
        "A is reclaimed by the record that outlived its deletion, and nothing is passed over"
    );

    {
        let _fault = util::fail_barriers_at(&public);
        let error = write_report(&public, &private, &payload, &mut NoHooks)
            .expect_err("the publication's barrier is refused");
        assert!(
            error.to_string().contains("injected barrier fault"),
            "the failure is the barrier's, by name: {error}"
        );
    }
    let staging_b = recorded_report_staging(&public, &private)
        .expect("read")
        .expect("the record still names the publication's own staging directory");
    assert!(
        !staging_b.exists() && public.join(REPORT).is_file(),
        "which the publication emptied by the rename and removed before its refused barrier"
    );
    let passed_over = write_report(&public, &private, &payload, &mut NoHooks)
        .expect("with the barrier holding, published");
    assert!(
        passed_over.is_empty()
            && !record.exists()
            && report_staging_leftovers(&public, &private)
                .expect("listed")
                .is_empty(),
        "the record of the removed directory is reclaimed, and nothing is passed over"
    );

    let leftover_d = plant_report_staging_of_a_dead_writer(&public, &private);
    let staging_d = leftover_d
        .parent()
        .expect("the staged file is inside the staging directory")
        .to_path_buf();
    {
        let _fault = util::fail_barriers_at(&public);
        for attempt in [
            "the fresh branch's reclaim that removed D",
            "the fresh branch again, which finds D already gone",
        ] {
            let error = sync_report_dir(&public, &private, &mut NoHooks)
                .expect_err("the fresh branch's reclaim barrier is refused");
            assert!(
                error.to_string().contains("injected barrier fault"),
                "{attempt}: the failure is the barrier's, by name: {error}"
            );
            assert_eq!(
                (
                    staging_d.exists(),
                    recorded_report_staging(&public, &private).expect("read")
                ),
                (false, Some(staging_d.clone())),
                "{attempt}: stopped at the barrier with the record still naming D"
            );
        }
    }
    let passed_over = sync_report_dir(&public, &private, &mut NoHooks)
        .expect("with the barrier holding, the fresh branch reclaims");
    assert!(
        passed_over.is_empty() && !record.exists(),
        "the fresh branch reclaims by the record that outlived D's deletion, and passes nothing over"
    );

    let leftover_e = plant_report_staging_of_a_dead_writer(&public, &private);
    let staging_e = leftover_e
        .parent()
        .expect("the staged file is inside the staging directory")
        .to_path_buf();
    fs::remove_dir_all(&staging_e).expect("the recorded directory gone");
    fs::write(&staging_e, b"an operator's file at the recorded name\n")
        .expect("a file at the recorded name");
    {
        let _fault = util::fail_barriers_at(&public);
        let error = sync_report_dir(&public, &private, &mut NoHooks)
            .expect_err("the fresh branch's barrier ahead of the record's removal is refused");
        assert!(
            error.to_string().contains("injected barrier fault"),
            "the failure is the barrier's, by name: {error}"
        );
    }
    assert_eq!(
        (
            recorded_report_staging(&public, &private).expect("read"),
            fs::read(&staging_e).expect("the file stands")
        ),
        (
            Some(staging_e.clone()),
            b"an operator's file at the recorded name\n".to_vec()
        ),
        "on the fresh branch too, with something other than a directory at the recorded name, the \
         record still names it after the refused barrier, and the file is as found"
    );
    let passed_over = sync_report_dir(&public, &private, &mut NoHooks)
        .expect("with the barrier holding, the fresh branch reclaims");
    assert_eq!(
        (passed_over, record.exists()),
        (vec![staging_e.clone()], false),
        "the fresh branch retires the record once the barrier holds, and passes the file over by name"
    );
    fs::remove_file(&staging_e).expect("removed for the next case");

    let leftover_c = plant_report_staging_of_a_dead_writer(&public, &private);
    let staging_c = leftover_c
        .parent()
        .expect("the staged file is inside the staging directory")
        .to_path_buf();
    fs::remove_dir_all(&staging_c).expect("the recorded directory gone");
    fs::write(&staging_c, b"an operator's file at the recorded name\n")
        .expect("a file at the recorded name");
    {
        let _fault = util::fail_barriers_at(&public);
        let error = write_report(&public, &private, &payload, &mut NoHooks)
            .expect_err("the barrier ahead of the record's removal is refused");
        assert!(
            error.to_string().contains("injected barrier fault"),
            "the failure is the barrier's, by name: {error}"
        );
    }
    assert_eq!(
        (
            recorded_report_staging(&public, &private).expect("read"),
            fs::read(&staging_c).expect("the file stands")
        ),
        (
            Some(staging_c.clone()),
            b"an operator's file at the recorded name\n".to_vec()
        ),
        "with something other than a directory at the recorded name, the record still names it \
         after the refused barrier, and the file is as found"
    );
    let passed_over = write_report(&public, &private, &payload, &mut NoHooks)
        .expect("with the barrier holding, published");
    assert_eq!(
        (passed_over, record.exists()),
        (vec![staging_c.clone()], false),
        "the record goes once the barrier holds, and the file at its name is passed over by name"
    );
}

/// Rule 1 for the record: the staging directory's name is recorded durably
/// in the private half **before** the directory is made, and the ledger's
/// entry for the creation carries the record observed standing in the
/// statement immediately before `create_dir` — so the recipe
/// `staging-created-before-its-record` (the creation moved ahead of the
/// record's publication) shows on the record as `present: false` and in the
/// sequence as the creation before the record's rename. The record's own
/// publication is the four steps every atomic publication takes, in the
/// private half, and its directory barrier precedes the creation.
#[test]
fn a_report_staging_directory_is_recorded_before_it_is_made() {
    let root = scratch("report-staging-recorded-first");
    let public = root.join("public");
    create_dir(&public).expect("public");
    let private = root.join("private");
    create_dir(&private).expect("private");
    let payload = serde_json::json!({"run_id": "01RECORD", "outcome": "parked"});
    let mut hooks = HarnessHooks::default().recording_durability();
    let ledger = hooks.ledger();
    write_report(&public, &private, &payload, &mut hooks).expect("the report");
    let records = ledger.records();
    let record = report_staging_record(&private);
    let made = records
        .iter()
        .position(|entry| entry.step == DurableStep::DirectoryCreated)
        .expect("the staging directory's creation is recorded");
    assert_eq!(
        records[made]
            .entry
            .as_ref()
            .map(|entry| (entry.path.clone(), entry.present)),
        Some((record.clone(), true)),
        "at the instant before `create_dir`, read by `symlink_metadata` in the statement \
         immediately before it, the record of the directory's name stood in the private half"
    );
    let record_published = records
        .iter()
        .position(|entry| entry.step == DurableStep::Renamed && entry.path == record)
        .expect("the record was renamed onto its name");
    let private_synced = records
        .iter()
        .position(|entry| entry.step == DurableStep::SyncedDirectory && entry.path == private)
        .expect("the private directory was synced");
    assert!(
        record_published < private_synced && private_synced < made,
        "the record is renamed onto its name and its directory synced before the staging \
         directory is made: {:?}",
        ledger.steps()
    );
    assert!(
        is_staged_report(&records[made].path.join(REPORT), &public),
        "and the directory made is this write's own, under the public run directory: {}",
        records[made].path.display()
    );
    assert!(
        report_staging_leftovers(&public, &private)
            .expect("listed")
            .is_empty()
            && !record.exists()
            && public.join(REPORT).is_file(),
        "after the rename the record and the directory are gone and the report stands"
    );
}

/// Rule 2 for the record's two barriers in the private half, which
/// `a_report_staging_directory_is_recorded_before_it_is_made` reads only when
/// they hold: the staged record's file barrier
/// (`util::fail_file_barriers_within`) and the private directory's barrier
/// after the rename (`util::fail_barriers_at`), each refused on its own across
/// two attempts, stop the write with the barrier's diagnostic before the
/// staging directory is made — no `DirectoryCreated` entry in the ledger and
/// nothing under the public run directory, no staging directory and no report
/// — and with the barrier holding again the next write publishes and reclaims
/// what the refused attempts left. A discarded error at either barrier, the
/// recipes `r11-staging-record-stage-swallowed` and
/// `r11-staging-record-publish-swallowed`, publishes a report through a
/// directory whose record was never made durable, and survived the ordering
/// test (`PR10-PRIVATE-RECORD-BARRIERS-UNWITNESSED`).
#[test]
fn a_report_staging_record_failure_stops_before_directory_creation() {
    let payload = serde_json::json!({"run_id": "01RECORDFAULT", "outcome": "parked"});
    for (tag, which) in [
        ("file", "the staged record's file barrier"),
        ("directory", "the private directory's barrier"),
    ] {
        let root = scratch(&format!("report-staging-record-{tag}-fault"));
        let public = root.join("public");
        create_dir(&public).expect("public");
        let private = root.join("private");
        create_dir(&private).expect("private");
        let mut hooks = HarnessHooks::default().recording_durability();
        let ledger = hooks.ledger();
        {
            let _fault = if tag == "file" {
                util::fail_file_barriers_within(&private)
            } else {
                util::fail_barriers_at(&private)
            };
            for attempt in 1..=2 {
                let error = write_report(&public, &private, &payload, &mut hooks)
                    .expect_err("a record whose barrier is refused stops the write");
                assert!(
                    error.to_string().contains("injected barrier fault"),
                    "{which}, attempt {attempt}: the failure is the barrier's, by name: {error}"
                );
            }
        }
        assert!(
            ledger
                .records()
                .iter()
                .all(|entry| entry.step != DurableStep::DirectoryCreated),
            "{which}: no staging directory was made: {:?}",
            ledger.steps()
        );
        assert_eq!(
            fs::read_dir(&public).expect("listed").count(),
            0,
            "{which}: nothing under the public run directory, no staging directory and no report"
        );
        write_report(&public, &private, &payload, &mut NoHooks)
            .expect("with the barrier holding, published");
        assert!(
            public.join(REPORT).is_file()
                && report_staging_leftovers(&public, &private)
                    .expect("listed")
                    .is_empty()
                && unrecorded_report_staging(&public, &private)
                    .expect("listed")
                    .is_empty(),
            "{which}: published, and what the refused attempts left reclaimed"
        );
    }
}

/// Whether `path` is a staged report of this protocol's: `report.json`
/// inside a staging directory of this round's shape directly under `public`.
fn is_staged_report(path: &Path, public: &Path) -> bool {
    path.file_name().and_then(|name| name.to_str()) == Some(REPORT)
        && path.parent().is_some_and(|staging| {
            staging.parent() == Some(public)
                && staging
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(REPORT_STAGING_PREFIX))
        })
}

/// The moved payload writers keep the **legacy byte shape**
/// (`PR5-RUNDIR-058`).
///
/// `production_effect`: "shared primitives move behind funnels
/// behavior-neutrally". Every consumer of these three parses the JSON back,
/// so indentation and the final newline were unobserved and switching
/// `report.json` and a question payload from the moved pretty writer to a
/// compact `serde_json::to_vec` changed nothing any test could see. The
/// expected bytes are written out here rather than produced by calling the
/// writer, so this is a golden file rather than a round trip — a round trip
/// is satisfied by any serializer at all.
#[test]
fn the_payload_writers_keep_their_exact_legacy_bytes() {
    let root = scratch("golden-bytes");
    let public = root.join("public");
    let questions = public.join("questions");
    create_dir(&questions).expect("questions");
    let private = root.join("private");
    create_dir(&private).expect("private");
    let payload = serde_json::json!({"kind": "choice", "options": ["a", "b"]});
    let expected = "{\n  \"kind\": \"choice\",\n  \"options\": [\n    \"a\",\n    \"b\"\n  ]\n}\n";

    write_report(&public, &private, &payload, &mut NoHooks).expect("report");
    assert_eq!(
        fs::read_to_string(public.join("report.json")).expect("report.json"),
        expected,
        "report.json is pretty-printed with two-space indentation and ends in a newline"
    );
    // The v0.1 coordinator publishes through this writer (`drain_and_report`,
    // schema 3), and until PR10's round 3 it was a plain `fs::write`, which
    // keeps the mode of the file it truncates. The staged publication creates
    // a fresh file at the umask default, so a report an operator had made
    // private was reopened as `0644` on the next write (the round-4
    // regression lens, P2-1): the existing report's permissions are copied
    // onto the staged file before it is synced and renamed.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let path = public.join("report.json");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).expect("make it private");
        write_report(&public, &private, &payload, &mut NoHooks).expect("report again");
        assert_eq!(
            fs::metadata(&path)
                .expect("report metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600,
            "a rewritten report keeps the mode the existing one had, as `fs::write` did"
        );
        assert_eq!(
            fs::read_to_string(&path).expect("report.json"),
            expected,
            "with the same bytes"
        );
    }

    write_question_payload(&questions, "q-1", &payload, &mut NoHooks).expect("question");
    assert_eq!(
        fs::read_to_string(questions.join("q-1.json")).expect("q-1.json"),
        expected,
        "and a question payload is written the same way"
    );

    // The plan is a byte pass-through — it is handed bytes that are already
    // serialized and normalized — so its golden property is that nothing
    // touches them at all, trailing newline included.
    let normalized = b"{\"tasks\":[]}";
    write_plan(&public, normalized, &mut NoHooks).expect("plan");
    assert_eq!(
        fs::read(public.join(PLAN)).expect("plan.json"),
        normalized,
        "the plan's exact bytes reach disk unaltered"
    );
}

/// The report's group survives its rewrite, as it did under `fs::write`.
///
/// A schema-3 run parks; the operator gives `report.json` a group and the
/// mode `0640` so that group can read it; the run resumes and the report is
/// written again. Until PR10's round 7 the staged publication replaced it
/// with a file in the writer's primary group (the round-7 regression lens,
/// P2): the group the operator chose lost the report and the writer's group
/// gained it. The group is read back with `MetadataExt::gid` before and
/// after, and the mode with it. The group is a supplementary group of this
/// process — a user in two groups is what the case needs, so where the
/// process has no second group the test says so and stops; on CI's runners
/// and on the build box the user has several. The `EPERM` arm — a group the
/// process is not in — cannot be set up without privilege and is reasoned in
/// `keep_group`'s doc, not driven here. What this observes, and no more:
/// the `Staged` ledger entry's mode at the file's creation (`0600`, the
/// group bits withheld); the `GroupGiven` entry's two modes, read by `fstat`
/// on the descriptor in the statement immediately before the `fchown` and in
/// the statement immediately after it, both without group bits — so a
/// widening anywhere before the first read is in the first, a widening
/// between the two is in the second, and the recipe "a `chmod` to the full
/// mode immediately before the `fchown`" has no line to land on that the
/// record does not show (the round-8 fix-check lens, P1: until round 8 the
/// test read the creation and the publication and nothing between; the
/// round-9 one, P1: round 8's record was a `stat` of the path in the
/// statement before the `fchown`, and a `chmod` inserted after the record
/// and before the syscall passed it) — and the published file's group and
/// mode. What the two reads bracket is the syscall itself; nothing a program
/// observes reaches inside it, and no other instant is claimed.
#[cfg(unix)]
#[test]
fn rewriting_report_preserves_its_group() {
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
    let root = scratch("report-group");
    let public = root.join("public");
    create_dir(&public).expect("public");
    let private = root.join("private");
    create_dir(&private).expect("private");
    let payload = serde_json::json!({"run_id": "01GROUP", "outcome": "parked"});
    let path = public.join(REPORT);
    write_report(&public, &private, &payload, &mut NoHooks).expect("the first report");
    let primary = fs::metadata(&path).expect("report metadata").gid();

    // SAFETY: `getgroups` with a null list and a count of zero answers how
    // many supplementary groups the process has and writes nothing; the
    // second call is handed a buffer of exactly that many entries, which it
    // fills up to the count it returns.
    let count = unsafe { libc::getgroups(0, std::ptr::null_mut()) };
    assert!(count >= 0, "getgroups answers the group count");
    let mut groups = vec![0 as libc::gid_t; usize::try_from(count).expect("a small count")];
    let filled = unsafe { libc::getgroups(count, groups.as_mut_ptr()) };
    assert!(filled >= 0, "getgroups fills the list");
    groups.truncate(usize::try_from(filled).expect("a small count"));
    let Some(other) = groups.iter().copied().find(|gid| *gid != primary) else {
        println!(
            "skipped: this process belongs to no group but {primary}, and the case needs a \
             second one ({groups:?})"
        );
        return;
    };

    std::os::unix::fs::chown(&path, None, Some(other)).expect("give the report the other group");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).expect("group-readable");
    let before = fs::metadata(&path).expect("report metadata");
    assert_eq!(
        before.gid(),
        other,
        "premise: the report belongs to the other group"
    );

    let mut hooks = HarnessHooks::default().recording_durability();
    let ledger = hooks.ledger();
    write_report(&public, &private, &payload, &mut hooks).expect("the report, written again");
    let after = fs::metadata(&path).expect("report metadata");
    assert_eq!(
        after.gid(),
        before.gid(),
        "a rewritten report keeps the group the existing one had, as `fs::write` did"
    );
    assert_eq!(
        after.permissions().mode() & 0o777,
        0o640,
        "and the mode that lets that group read it"
    );
    let staged: Vec<_> = ledger
        .records()
        .into_iter()
        .filter(|record| {
            record.step == DurableStep::Staged && is_staged_report(&record.path, &public)
        })
        .collect();
    assert_eq!(
        staged.iter().map(|record| record.mode).collect::<Vec<_>>(),
        vec![Some(0o600)],
        "the staged file was created without the group bits"
    );
    let given: Vec<_> = ledger
        .records()
        .into_iter()
        .filter(|record| record.step == DurableStep::GroupGiven)
        .collect();
    assert_eq!(
        given
            .iter()
            .map(|record| record.path.as_path())
            .collect::<Vec<_>>(),
        vec![staged[0].path.as_path()],
        "the group was given once, at the staged file"
    );
    assert_eq!(
        given[0].mode.map(|mode| mode & 0o070),
        Some(0),
        "at the instant before the `fchown`, read by `fstat` on the descriptor in the statement \
         before it, the file carried no group bits: {:o}",
        given[0].mode.unwrap_or(0)
    );
    assert_eq!(
        given[0].mode_after.map(|mode| mode & 0o070),
        Some(0),
        "and at the instant after it, read the same way in the statement after it, still none — \
         a widening between the two reads is a widening the record shows: {:o}",
        given[0].mode_after.unwrap_or(0)
    );
    assert_eq!(
        ledger
            .records_for(&staged[0].path)
            .into_iter()
            .map(|record| record.step)
            .collect::<Vec<_>>(),
        vec![
            DurableStep::Staged,
            DurableStep::GroupGiven,
            DurableStep::SyncedFile
        ],
        "created without the group bits, given its group, then synced — the group bits arrive \
         with `settle_mode`, between the second and the third"
    );
    assert!(
        report_staging_leftovers(&public, &private)
            .expect("listed")
            .is_empty(),
        "the staged file was renamed onto its name"
    );
}

// =======================================================================
// What `status` says about a husk id
// =======================================================================

#[test]
fn a_husk_id_reports_as_one_of_the_three_things_it_can_be() {
    // `startup_census`: status "asked explicitly for a husk id, reports an
    // unstarted husk that the next write command reclaims, a retained husk
    // with its reason and locator, or a possibly committed run whose
    // public log has no valid committed first line".
    let unstarted = BoundHusk::new("statusunstarted");
    fs::create_dir_all(unstarted.public()).expect("public");
    let report = husk_report(
        &unstarted.repo,
        BOUND_RUN,
        &unstarted.repo_key,
        &unstarted.private_root,
    );
    assert!(
        matches!(report.disposition, HuskDisposition::Unstarted(_)),
        "{:?}",
        report.disposition
    );
    assert!(report.disposition.describe().contains("unstarted"));
    assert!(report.locator.is_none(), "a bare husk records no locator");

    let retained = BoundHusk::new("statusretained");
    retained.publish();
    write(&retained.private.join(OWNER_RECORD), b"{ not json");
    let report = husk_report(
        &retained.repo,
        BOUND_RUN,
        &retained.repo_key,
        &retained.private_root,
    );
    assert!(report.disposition.describe().starts_with("a retained husk"));
    assert_eq!(report.locator.as_deref(), Some(retained.private.as_path()));

    let committed = BoundHusk::new("statuspossibly");
    committed.publish();
    write(&committed.private.join(COMMIT_RECORD), b"{}");
    let report = husk_report(
        &committed.repo,
        BOUND_RUN,
        &committed.repo_key,
        &committed.private_root,
    );
    assert!(
        report.disposition.describe().contains("possibly committed"),
        "{}",
        report.disposition.describe()
    );
    assert!(
        report.disposition.describe().contains("nothing is deleted"),
        "and says so"
    );

    // The three sentences are three sentences.
    let mut said: Vec<String> = [&unstarted, &retained, &committed]
        .iter()
        .map(|husk| {
            husk_report(&husk.repo, BOUND_RUN, &husk.repo_key, &husk.private_root)
                .disposition
                .describe()
        })
        .collect();
    said.sort();
    said.dedup();
    assert_eq!(said.len(), 3, "each of the three reads differently");
}

#[test]
fn an_ambiguous_husk_prefix_is_not_reported_as_one_husk() {
    let repo = scratch("ambiguoushusk").join("repo");
    for husk in ["01HUSKA", "01HUSKB"] {
        fs::create_dir_all(runs_root(&repo).join(husk)).expect("husk");
    }
    let error = resolve_run_id(&repo, "01HUSK").expect_err("no committed run");
    assert!(
        !error.to_string().contains("never recorded a committed"),
        "two husks match, so naming one of them would be a guess: {error}"
    );
}
