// Allowlist placement: the **funnel section** of `effects/allowlist.toml`, by
// attachment to `src/workspace_manager.rs` -- the shape
// `src/runner/container/tests.rs` and `src/agent/proc/test_support/readiness.rs`
// established for a funnel's out-of-line child. This file builds the scratch
// repositories the Worktree/Snapshot/Ref/Object suites measure against, so it
// names `fs::write`, `fs::create_dir_all` and `std::process::Command` directly.
//
// `PR6-LANEF-004`: a Rust lint level is scoped by the MODULE TREE and not by
// the file, so without an attribute here the parent's inner allow of all three
// would reach this file silently and no reviewed record would name the file
// doing the work. `clippy::disallowed_macros` is RE-DENIED rather than
// inherited -- measured at zero sites -- so a `println!` here is still a build
// error. `decisions.effect_site_inventory.mechanism` (2).
#![allow(clippy::disallowed_methods, clippy::disallowed_types)]
#![forbid(clippy::disallowed_macros)]

use super::*;

// `OsStr` came from the parent's import list until the `m4-workspace` split
// moved its last production user into a child; named here for the same reason.
use std::cell::{Cell, RefCell};
use std::ffi::OsStr;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicU32, Ordering};

// -----------------------------------------------------------------------
// Fixtures
// -----------------------------------------------------------------------

pub(crate) static SCRATCH: AtomicU32 = AtomicU32::new(0);

// -----------------------------------------------------------------------
// Observing the removal retry
// -----------------------------------------------------------------------

/// What to run after each of this thread's removal attempts.
///
/// A named alias because the raw shape trips `clippy::type_complexity`, and
/// because the two `thread_local!` slots below read better for having it.
type AttemptObserver = Box<dyn FnMut(u32)>;

// Attempts `remove_tree_once_handles_close` has made **on this thread**, and
// the observer to run after each.
//
// Thread-local rather than global, and that is the whole reason it is sound:
// the suite runs tests in parallel and several of them remove worktrees, so a
// process-wide counter would be another test's number as often as this one's.
// The primitive runs on the thread that called `remove_worktree`, so a
// thread-local counts exactly the removals the observing test drove.
//
// `PR5-R1-CFG-TEST-SHRINKS-THE-DOMAIN` is why the parent's half of this seam
// is declared at the bottom of `src/workspace_manager.rs` rather than beside
// the primitive: `effects::production_region` truncates a source at its first
// `#[cfg(test)]`, so a `#[cfg(test)]` item above the funnels would take every
// one of them out of the census that proves the Worktree group has them.
//
// `//` rather than `///`: rustdoc does not document a macro invocation, and
// `-D unused-doc-comments` says so.
thread_local! {
    static REMOVAL_ATTEMPTS: Cell<u32> = const { Cell::new(0) };
    static REMOVAL_ATTEMPT_OBSERVER: RefCell<Option<AttemptObserver>> =
        const { RefCell::new(None) };
    static MARKER_READ_ATTEMPTS: Cell<u32> = const { Cell::new(0) };
    static MARKER_READ_ATTEMPT_OBSERVER: RefCell<Option<AttemptObserver>> =
        const { RefCell::new(None) };
}

/// A live observation of this thread's removal attempts, ended by dropping it.
///
/// Held by the observing test for exactly as long as the observation is wanted;
/// its `Drop` uninstalls the observer, so a test that unwinds cannot leave a
/// closure behind for whatever runs next on this thread.
pub(crate) struct AttemptObservation {
    /// Not `Send`: the counter and the observer are this thread's.
    _not_send: PhantomData<*const ()>,
}

impl AttemptObservation {
    /// Attempts made since the observation began.
    pub(crate) fn count(&self) -> u32 {
        REMOVAL_ATTEMPTS.with(Cell::get)
    }
}

impl Drop for AttemptObservation {
    fn drop(&mut self) {
        REMOVAL_ATTEMPT_OBSERVER.with(|slot| {
            if let Ok(mut slot) = slot.try_borrow_mut() {
                *slot = None;
            }
        });
    }
}

/// Start counting this thread's removal attempts, running `observer` after each.
///
/// The observer runs **on the removing thread, after the attempt has already
/// returned**, which is the property the closing-handle control is built on: a
/// test that releases a held handle from here knows the attempt it is releasing
/// against has completed, rather than hoping it has. Nothing outside the loop
/// can establish that, which is why this seam exists at all.
pub(crate) fn observe_removal_attempts(observer: AttemptObserver) -> AttemptObservation {
    REMOVAL_ATTEMPTS.with(|count| count.set(0));
    REMOVAL_ATTEMPT_OBSERVER.with(|slot| {
        if let Ok(mut slot) = slot.try_borrow_mut() {
            *slot = Some(observer);
        }
    });
    AttemptObservation {
        _not_send: PhantomData,
    }
}

/// Record that attempt `attempt` has completed, and run the observer.
///
/// Called from `super::note_removal_attempt`, the `#[cfg(test)]` half of the
/// primitive's seam. `try_borrow_mut` rather than `borrow_mut` so that an
/// observer which somehow removes a tree of its own is a no-op here instead of
/// a panic inside production code.
pub(crate) fn note_removal_attempt(attempt: u32) {
    REMOVAL_ATTEMPTS.with(|count| count.set(attempt));
    REMOVAL_ATTEMPT_OBSERVER.with(|slot| {
        if let Ok(mut slot) = slot.try_borrow_mut() {
            if let Some(observer) = slot.as_mut() {
                observer(attempt);
            }
        }
    });
}

/// A live observation of this thread's registration-marker read attempts
/// (`read_marker_once_handles_close`), ended by dropping it: the removal
/// seam's twin for the classifier's read, kept apart from it so a control
/// that counts one is never counting the other.
pub(crate) struct MarkerReadObservation {
    /// Not `Send`: the counter and the observer are this thread's.
    _not_send: PhantomData<*const ()>,
}

impl MarkerReadObservation {
    /// Attempts made since the observation began.
    pub(crate) fn count(&self) -> u32 {
        MARKER_READ_ATTEMPTS.with(Cell::get)
    }
}

impl Drop for MarkerReadObservation {
    fn drop(&mut self) {
        MARKER_READ_ATTEMPT_OBSERVER.with(|slot| {
            if let Ok(mut slot) = slot.try_borrow_mut() {
                *slot = None;
            }
        });
    }
}

/// Start counting this thread's marker read attempts, running `observer`
/// after each — on the reading thread, after the attempt has returned, as
/// [`observe_removal_attempts`] does, and for the same reason.
pub(crate) fn observe_marker_read_attempts(observer: AttemptObserver) -> MarkerReadObservation {
    MARKER_READ_ATTEMPTS.with(|count| count.set(0));
    MARKER_READ_ATTEMPT_OBSERVER.with(|slot| {
        if let Ok(mut slot) = slot.try_borrow_mut() {
            *slot = Some(observer);
        }
    });
    MarkerReadObservation {
        _not_send: PhantomData,
    }
}

/// Record that marker read attempt `attempt` has completed, and run the
/// observer. Called from `super::note_marker_read_attempt`.
pub(crate) fn note_marker_read_attempt(attempt: u32) {
    MARKER_READ_ATTEMPTS.with(|count| count.set(attempt));
    MARKER_READ_ATTEMPT_OBSERVER.with(|slot| {
        if let Ok(mut slot) = slot.try_borrow_mut() {
            if let Some(observer) = slot.as_mut() {
                observer(attempt);
            }
        }
    });
}

/// A scratch directory unique to this process *and* to this call, because
/// the suite runs tests in parallel and two fixtures sharing a directory
/// would each measure the other's Git repository.
pub(crate) fn scratch(tag: &str) -> PathBuf {
    let ordinal = SCRATCH.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "upstroke-wm-{tag}-{}-{ordinal}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create the scratch directory");
    dir
}

pub(crate) fn git_out(dir: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .expect("run git")
}

pub(crate) fn git(dir: &Path, args: &[&str]) -> String {
    let output = git_out(dir, args);
    assert!(
        output.status.success(),
        "git {args:?} in {}: {}",
        dir.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// [`git`] with arguments passed as the bytes they are, for an argument that
/// is a path: `to_string_lossy` would hand Git a path holding U+FFFD wherever
/// the real one holds a byte no UTF-8 spells, and Git would act on that
/// other path (the round-1 fix-check lens of #308, with `TMPDIR` under a
/// directory named with byte `0xff`).
pub(crate) fn git_os(dir: &Path, args: &[&OsStr]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {args:?} in {}: {}",
        dir.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// The administrative directory of the linked worktree at `worktree`, bound
/// the way the forced removal binds it: by its `gitdir`, never by the name
/// Git generated for it.
pub(crate) fn registration_of(manager: &WorkspaceManager, worktree: &Path) -> PathBuf {
    manager
        .revalidate_removal_proving(worktree, WriterProof::Unknown)
        .expect("the store binds the worktree")
        .admin
        .expect("the worktree is registered")
}

/// Tear the registration of the linked worktree at `worktree` the way a `git
/// worktree add` killed while it writes `commondir` leaves it: `locked`
/// holding `initializing`, which the add writes first and unlinks last, and a
/// `commondir` opened and never written. Returns the registration's
/// administrative directory, found the way the removal binds it — by its
/// `gitdir` — and never by its Git-generated name.
///
/// Git's enumeration dies on the result (`fatal: failed to read
/// .git/worktrees/<name>/commondir: Success` on glibc, `Undefined error: 0`
/// on macOS), which is `PR5-RD-002`'s fixture V8 for one slot.
pub(crate) fn tear_registration(manager: &WorkspaceManager, worktree: &Path) -> PathBuf {
    let admin = registration_of(manager, worktree);
    fs::write(admin.join("locked"), "initializing\n")
        .expect("the lock the add holds until it finishes");
    fs::write(admin.join("commondir"), []).expect("the file the add opened and never wrote");
    admin
}

/// A real repository, a real private root, and a manager over both.
/// The fixture's run id: a canonical ULID, as `derive` requires
/// (`DESIGN.md` §15, "run-id = ULID"), spelt to be recognisable in a path.
pub(crate) const RUN_ID: &str = "01KZSWEEP00000000000000001";

pub(crate) struct Fixture {
    pub(crate) root: PathBuf,
    pub(crate) base: PathBuf,
    pub(crate) private: PathBuf,
    pub(crate) manager: WorkspaceManager,
    /// The first commit.
    pub(crate) seed: String,
    /// The tip of `main`.
    pub(crate) head: String,
    /// A commit on a side branch, based on `seed`, for the cherry-picks.
    pub(crate) side: String,
}

impl Fixture {
    /// A SHA-1 repository, whatever `GIT_DEFAULT_HASH` says in the
    /// environment: the object format is part of what a test asserts about
    /// (an object id's length, the null id's spelling), so the fixture pins
    /// it rather than inheriting it (§12). [`Self::with_object_format`] is
    /// the other format.
    pub(crate) fn new(tag: &str) -> Self {
        Self::with_object_format(tag, "sha1")
    }

    /// A repository of the given object format, `sha1` or `sha256`.
    pub(crate) fn with_object_format(tag: &str, object_format: &str) -> Self {
        let root = scratch(tag);
        let base = root.join("repo");
        let private = root.join("private");
        fs::create_dir_all(&base).expect("repo directory");
        fs::create_dir_all(&private).expect("private root");

        let object_format = format!("--object-format={object_format}");
        git(&base, &["init", "-q", "-b", "main", &object_format]);
        git(&base, &["config", "user.email", "tests@upstroke.local"]);
        git(&base, &["config", "user.name", "upstroke tests"]);
        // Line endings are pinned for the same reason the object format is
        // (§12, and `PR126-REVIEW2-NULL-TESTS-INHERIT-THE-HASH-FORMAT`): an
        // ambient Git setting that silently changes what a test observes.
        // With `core.autocrlf` on, as it is on the Windows guest, a blob
        // written as `A\n` is checked out as `A\r\n`, so a test comparing
        // checked-out content against what it wrote fails on that platform
        // alone while the blob is the one it asked for.
        git(&base, &["config", "core.autocrlf", "false"]);
        git(&base, &["config", "core.eol", "lf"]);
        // Whether `refs/replace/*` is honoured is pinned for the same reason
        // and against the same class of accident: an operator's
        // `core.useReplaceRefs = false`, or one an `init.templateDir` copied
        // into this repository's own config, decides what four of these
        // suites measure. See `pin_replacement_refs_in`, and
        // `without_ambient_replacement_controls` for the half a repository's
        // configuration cannot reach.
        pin_replacement_refs_in(&base);
        // `git worktree add` writes a reflog entry; keep the repository
        // self-contained so nothing depends on a global config.
        git(&base, &["config", "core.logAllRefUpdates", "true"]);
        fs::write(base.join("a.txt"), "one\n").expect("seed file");
        git(&base, &["add", "-A"]);
        git(&base, &["commit", "-q", "-m", "seed"]);
        let seed = git(&base, &["rev-parse", "HEAD"]);

        fs::write(base.join("b.txt"), "two\n").expect("second file");
        git(&base, &["add", "-A"]);
        git(&base, &["commit", "-q", "-m", "second"]);
        let head = git(&base, &["rev-parse", "HEAD"]);

        git(&base, &["checkout", "-q", "-b", "side", &seed]);
        fs::write(base.join("c.txt"), "side\n").expect("side file");
        git(&base, &["add", "-A"]);
        git(&base, &["commit", "-q", "-m", "side"]);
        let side = git(&base, &["rev-parse", "HEAD"]);
        git(&base, &["checkout", "-q", "main"]);

        // The run's public directory, where `RunLock::acquire` would have
        // created the cleanup lease a coordinator's ref writes hold
        // (`rundir::hold_cleanup_lease_for_child`). No run lock is taken over
        // this fixture, so the directory is made here for the lease file to
        // land in, exactly where the engine's own run directory puts it.
        fs::create_dir_all(crate::rundir::public_dir(&base, RUN_ID))
            .expect("the run's public directory");
        let manager =
            WorkspaceManager::derive(&base, &private, RUN_ID, "inc-1").expect("derive the manager");
        Self {
            root,
            base,
            private,
            manager,
            seed,
            head,
            side,
        }
    }

    /// Re-open a fixture a **previous process** built.
    ///
    /// A kill child dies by `std::process::abort()`, so its `Drop` never
    /// runs and its scratch tree survives it. The parent then has to speak
    /// about that tree — which repository, which private root, which
    /// commits — and re-deriving it is the only honest way: a value passed
    /// through an environment variable would be the child's belief about
    /// its own state, and the whole point of a kill test is that the
    /// child's beliefs did not survive.
    ///
    /// The manager is derived with the **same** run id and incarnation as
    /// [`Self::new`], because an intent records both and a reclaim that
    /// derived a different pair would be reclaiming another run's residue.
    pub(crate) fn adopt(root: PathBuf) -> Self {
        let base = root.join("repo");
        let private = root.join("private");
        let head = git(&base, &["rev-parse", "main"]);
        let seed = git(&base, &["rev-parse", "main~1"]);
        let side = git(&base, &["rev-parse", "side"]);
        let manager = WorkspaceManager::derive(&base, &private, RUN_ID, "inc-1")
            .expect("derive the manager over an adopted fixture");
        Self {
            root,
            base,
            private,
            manager,
            seed,
            head,
            side,
        }
    }

    pub(crate) fn created(tag: &str) -> Self {
        let fixture = Self::new(tag);
        fixture
            .manager
            .create_execution_root(&mut NoHooks)
            .expect("create the execution root");
        fixture
    }

    /// [`Self::created`] over a SHA-256 repository, for the tests that assert
    /// something about both object formats.
    pub(crate) fn created_sha256(tag: &str) -> Self {
        let fixture = Self::with_object_format(tag, "sha256");
        fixture
            .manager
            .create_execution_root(&mut NoHooks)
            .expect("create the execution root");
        fixture
    }

    pub(crate) fn task(&self, key: &str, generation: u32) -> Slot {
        Slot::Task {
            key: key.to_owned(),
            generation,
        }
    }

    /// A task worktree at `head`, intent first.
    pub(crate) fn add_task(&self, hooks: &mut dyn EffectHooks, key: &str, generation: u32) -> Slot {
        let slot = self.task(key, generation);
        self.manager
            .write_intent(hooks, &slot)
            .expect("write the intent");
        self.manager
            .add_worktree(hooks, &slot, &self.head)
            .expect("add the worktree");
        slot
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

// -----------------------------------------------------------------------
// The primitives a topology module's test cannot reach for itself
// -----------------------------------------------------------------------

/// Write `bytes` at `path`, creating the parent directories.
///
/// This is a test's *worker*: in production an agent subprocess edits files
/// and the engine never does (DESIGN.md §4). A test has no agent, so it
/// writes what the agent would have written — and it does it here, where
/// the write is inside the reviewed funnel module, rather than in the
/// topology module whose whole point is that it cannot.
pub(crate) fn write_file(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("the parent directory of a fixture file");
    }
    fs::write(path, bytes).expect("write a fixture file");
}

/// Create `path` and every missing parent.
pub(crate) fn create_dir(path: &Path) {
    fs::create_dir_all(path).expect("create a fixture directory");
}

/// A fan-out directory the object store already holds, to construct Git's
/// temporary object file in.
///
/// Git creates the fan-out directory when it is missing and writes the
/// temporary file inside it, so any two-hexadecimal-digit name would do; one
/// that already exists puts the file beside real objects, which is the shape
/// the trace in
/// [`temporary_object_files`](crate::workspace_manager::temporary_object_files)
/// records. Shared by the three places a test constructs the element —
/// `construct_element`, `plant_stage_residue` and the dispatch
/// materialization test — so that none of them plants at the object root,
/// where the arm the scan always had would see it and the fan-out arm this
/// element is evidence for would not be exercised
/// (`PR258-GRID-PLANTS-AT-THE-OBJECT-ROOT`).
pub(crate) fn fan_out_directory(objects: &Path) -> PathBuf {
    let mut names: Vec<PathBuf> = fs::read_dir(objects)
        .expect("the object directory")
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_dir()
                && path.file_name().is_some_and(|name| {
                    let name = name.to_string_lossy();
                    name.len() == 2 && name.chars().all(|character| character.is_ascii_hexdigit())
                })
        })
        .collect();
    names.sort();
    names
        .into_iter()
        .next()
        .expect("a store with objects in it has a fan-out directory")
}

/// Remove `path` if it is there. Idempotent, like every reclaim.
pub(crate) fn remove_file(path: &Path) {
    match fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => panic!("removing {}: {error}", path.display()),
    }
}

/// Remove the empty directory at `path`: a worker's file operation, standing
/// for a tool that removes a directory it has emptied.
pub(crate) fn remove_dir(path: &Path) {
    fs::remove_dir(path)
        .unwrap_or_else(|error| panic!("removing the directory {}: {error}", path.display()));
}

/// Run this test binary again, `--exact --ignored`, with `env` set, and
/// return its exit status.
///
/// The kill-test shape `src/rundir.rs` established: `Injection::Kill` is
/// `std::process::abort()`, a real process death, so the child has to be a
/// real process and the claim is what it left on disk. `env` is a list
/// rather than a map so a caller can pass the same key twice and see the
/// last win, exactly as `Command` does.
pub(crate) fn run_kill_child(test: &str, env: &[(&str, &OsStr)]) -> std::process::ExitStatus {
    let mut command = Command::new(std::env::current_exe().expect("this test binary"));
    command
        .args(["--exact", test, "--ignored", "--nocapture"])
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    for (key, value) in env {
        command.env(key, value);
    }
    command.status().expect("spawn the kill child")
}

/// [`run_kill_child`] with a deadline: `Some` with the child's exit status
/// once it ends within `bound`, and `None` when it has not — and then the
/// child has been killed and reaped before this returns, so a child that
/// wedges neither holds its caller past the bound nor outlives the call.
///
/// Spawned exactly as [`run_kill_child`] spawns it, and polled (`try_wait`,
/// then a 10 ms sleep) rather than blocked on, so a child that ends is
/// answered for within one poll of its end. The kill is the child's own
/// (`Child::kill`): a process the child started is not killed with it.
pub(crate) fn run_kill_child_within(
    test: &str,
    env: &[(&str, &OsStr)],
    bound: std::time::Duration,
) -> Option<std::process::ExitStatus> {
    let mut command = Command::new(std::env::current_exe().expect("this test binary"));
    command
        .args(["--exact", test, "--ignored", "--nocapture"])
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    for (key, value) in env {
        command.env(key, value);
    }
    let mut child = command.spawn().expect("spawn the kill child");
    let deadline = std::time::Instant::now() + bound;
    loop {
        if let Some(status) = child.try_wait().expect("poll the kill child") {
            return Some(status);
        }
        if std::time::Instant::now() >= deadline {
            child
                .kill()
                .expect("kill the kill child the deadline ended");
            child
                .wait()
                .expect("reap the kill child the deadline ended");
            return None;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// Run this test binary again, `--exact --ignored --nocapture`, with `env` set
/// and its stdout piped, adopted by a [`readiness::Producer`]: the caller waits
/// for the line the helper prints once it is ready, and the child is
/// terminated, reaped and its reader joined when the producer drops, however
/// the caller's scope ends.
///
/// For the suites under `src/engine/topology/**`, where `clippy.toml` denies
/// `std::process::Command` to tests as well as production and which therefore
/// cannot spawn a helper of their own.
///
/// Unix only, because its one caller is: the helper it spawns holds a shared
/// `flock`, which Windows has no counterpart of. An ungated definition with a
/// `cfg(unix)` caller is dead code on the Windows legs, and CI's `-D warnings`
/// makes that a build error the Linux box cannot see (`#275`, CI run
/// 34693369689).
#[cfg(unix)]
pub(crate) fn spawn_ready_helper(
    test: &str,
    env: &[(&str, &OsStr)],
) -> crate::agent::proc::test_support::readiness::Producer {
    let mut command = Command::new(std::env::current_exe().expect("this test binary"));
    command
        .args(["--exact", test, "--ignored", "--nocapture"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    for (key, value) in env {
        command.env(key, value);
    }
    crate::agent::proc::test_support::readiness::Producer::adopt(
        command.spawn().expect("spawn the ready helper"),
    )
}

// -----------------------------------------------------------------------
// A fork parked with one inherited descriptor, held until it is released
// -----------------------------------------------------------------------

/// A child of this process, forked and parked before it exits, holding the
/// one descriptor it was forked to hold.
///
/// A `fork` copies the whole descriptor table, so the child holds a copy of
/// every descriptor this process had open at that instant, and a copy of a
/// run's cleanup lease -- open in this process for the life of a ref write,
/// `rundir::hold_cleanup_lease_for_child` -- holds the shared `flock` with
/// it. That is the window every spawn in this process has between its
/// `fork` and its `exec`; this value makes one such window last as long as
/// a test needs, with the one descriptor it is meant to model and nothing
/// of any other test's.
///
/// **The protocol, as it runs.** The parent computes the ceiling of its own
/// descriptor table ([`descriptor_ceiling`]) and forks. The child, before
/// anything else, closes every inherited descriptor above stdio except the
/// one it keeps and its end of a socket pair, each close checked
/// ([`close_above_stdio_except`]); writes one report to the socket -- its
/// pid after a complete sweep, or the descriptor it could not close and
/// the errno -- and then, parked, blocks on a read of the same socket until
/// [`Self::release`] writes to it or the parent dies, on which it `_exit`s.
/// The child never `exec`s: its exit closes its copy exactly as an `exec`'s
/// `CLOEXEC` would, and there is no `Command`, no spawn whose return could
/// come early and no thread to join. The report arriving is the proof that
/// the child is alive, parked and holding its copy with nothing else, and
/// the constructor returns only once it has; nothing about the child is
/// inferred from a clock. A report of a failed sweep, or none within
/// [`READY_BOUND`], fails the constructor after the child has been
/// collected -- killed first if it is still there -- so a helper that could
/// not isolate its child never announces one. The child's copy of the
/// parent's socket end goes with the sweep, so the parent's death ends the
/// read with EOF and the child does not outlive it parked.
///
/// Every stage that can block is bounded, and each bound starts when its
/// stage does: the wait for the report by `READY_BOUND`, one deadline the
/// reads look at before every turn; the reap after a release, or after a
/// drop, by [`REAP_BOUND`] ([`Self::reap_bound`] lowers it), past which
/// the child is killed and then collected within the same bound again --
/// every observation one `waitpid` with `WNOHANG` that nothing retries,
/// every rest between two of them one `nanosleep` that nothing makes again
/// ([`rest_within`]), and the release one write that is not made again.
/// [`Self::is_alive`] asks the kernel whether the child has ended rather
/// than the signal table whether its pid exists, and collects a child that
/// has, so a zombie never reads as alive. Dropping this value releases and
/// reaps the child, while unwinding too, never panics doing it, and says on
/// stderr ([`say_on_stderr`], nothing made again) what it could not do and
/// the state the child is left in: collected with its status when only the
/// release failed, uncollected with the step that failed otherwise.
/// [`Self::kept`] names the two descriptors the child holds above stdio.
#[cfg(unix)]
pub(crate) struct ParkedFork {
    pid: libc::pid_t,
    kept: libc::c_int,
    socket: libc::c_int,
    release: Option<std::os::unix::net::UnixStream>,
    reap_bound: std::time::Duration,
    /// The status of a child already collected, by [`Self::is_alive`] or by
    /// a release, so it is never waited for twice.
    ended: Cell<Option<std::process::ExitStatus>>,
}

/// How long [`ParkedFork::holding`] waits for the child's report before it
/// collects the child and fails: a bound on a child that wedges before it
/// parks, never a measure of a healthy one, which reports in microseconds.
#[cfg(unix)]
pub(crate) const READY_BOUND: std::time::Duration = std::time::Duration::from_secs(60);

/// The tick of each read the constructor makes while it waits for the report:
/// the handshake socket's own timeout, set once, before the fork, while both
/// ends are open. The bound on the whole wait is [`READY_BOUND`], one deadline
/// from the fork that no tick, interruption or short read restarts
/// ([`read_report_within`]); the tick only bounds a single read so that the
/// deadline is looked at.
#[cfg(unix)]
pub(crate) const READY_TICK: std::time::Duration = std::time::Duration::from_millis(100);

/// How long a release, or a drop, waits for the released child to exit
/// before it kills it.
#[cfg(unix)]
pub(crate) const REAP_BOUND: std::time::Duration = std::time::Duration::from_secs(10);

/// The child's report: a tag byte and two native-endian `c_int`s -- the
/// pid and zero after a complete sweep, the descriptor and the errno after
/// a close that failed.
#[cfg(unix)]
pub(crate) const REPORT_LEN: usize = 9;
#[cfg(unix)]
const REPORT_PARKED: u8 = 0;
#[cfg(unix)]
const REPORT_CLOSE_FAILED: u8 = 1;

#[cfg(unix)]
impl ParkedFork {
    /// Park a fork that holds a copy of the cleanup lease of the run whose
    /// public directory is `public`, taken exactly as a ref write takes it.
    ///
    /// The copy is opened on a `Command` this never spawns, the fork is made
    /// while the copy is open, and the copy is then closed: what remains is
    /// the parked child's inherited descriptor, the shared `flock` with it.
    pub(crate) fn holding_the_lease_of(public: &Path) -> Self {
        use std::os::fd::AsRawFd as _;

        let mut never_spawned = Command::new("true");
        let copy = crate::rundir::hold_cleanup_lease_for_child(&mut never_spawned, public)
            .expect("the run directory exists, so its lease can be taken")
            .expect("Unix hands the child a hold");
        let parked = Self::holding(copy.as_raw_fd());
        drop(copy);
        parked
    }

    /// Park a fork that keeps `kept`, a descriptor open in this process, and
    /// nothing else of this process's above stdio. Returns once the child has
    /// reported itself parked.
    ///
    /// # Panics
    ///
    /// When the child reports a descriptor its sweep could not close, or
    /// reports nothing within [`READY_BOUND`] of the fork -- one deadline,
    /// whatever interrupts or shortens the reads that wait for the report.
    /// The child has been collected -- killed first if it was still there --
    /// before the panic, whose message names its pid and, for a failed close,
    /// the descriptor and the error.
    pub(crate) fn holding(kept: libc::c_int) -> Self {
        use std::os::fd::AsRawFd as _;

        let (release, child_end) =
            std::os::unix::net::UnixStream::pair().expect("a socket pair for the park handshake");
        release
            .set_read_timeout(Some(READY_TICK))
            .expect("bound each read of the child's report, while both ends are open");
        let socket = child_end.as_raw_fd();
        let listed = listed_descriptors();
        let ceiling = descriptor_ceiling(&listed);
        // SAFETY: `fork` takes nothing and reads nothing. Its child runs
        // `park` as its first act and nothing else: only async-signal-safe
        // syscalls, no allocation, no lock, and it never returns into this
        // process's state, which is what forking a threaded process asks of
        // the child.
        let pid = unsafe { libc::fork() };
        if pid == 0 {
            // SAFETY: this is that child, before anything else.
            unsafe { park(kept, socket, ceiling, &listed) }
        }
        assert!(pid > 0, "fork: {}", std::io::Error::last_os_error());
        let report_deadline = std::time::Instant::now() + READY_BOUND;
        // This process's copy of the child's end is closed now, so the
        // child's read can only be ended by a release or by this process's
        // death.
        drop(child_end);
        let parked = Self {
            pid,
            kept,
            socket,
            release: Some(release),
            reap_bound: REAP_BOUND,
            ended: Cell::new(None),
        };
        let mut parent_end = parked
            .release
            .as_ref()
            .expect("the parent's end is open until a release");
        let report = match read_report_within(&mut parent_end, report_deadline) {
            Ok(report) => report,
            Err(error) => {
                let collected = parked.collect(std::time::Duration::ZERO);
                panic!(
                    "the parked child {pid} reported nothing within {READY_BOUND:?}: {error}; \
                     killed and reaped: {collected:?}"
                );
            }
        };
        let (tag, first, second) = decode_report(&report);
        match tag {
            REPORT_PARKED => {
                assert_eq!(
                    first, pid,
                    "the child that reported itself parked is the one that was forked"
                );
                parked
            }
            REPORT_CLOSE_FAILED => {
                let collected = parked.collect(REAP_BOUND);
                let reason = if second == 0 {
                    String::from("the close answered success and the descriptor read open after it")
                } else {
                    std::io::Error::from_raw_os_error(second).to_string()
                };
                panic!(
                    "the parked child {pid} could not close inherited descriptor {first}: {reason}; \
                     it exited before parking and was reaped: {collected:?}"
                );
            }
            other => {
                let collected = parked.collect(std::time::Duration::ZERO);
                panic!(
                    "the parked child {pid} sent a report tagged {other}, which is neither \
                     outcome; killed and reaped: {collected:?}"
                );
            }
        }
    }

    /// The parked child's pid.
    pub(crate) fn pid(&self) -> libc::pid_t {
        self.pid
    }

    /// The two descriptors the child holds above stdio, by the numbers this
    /// process opened them under, which the child's table shares: the one it
    /// was forked to hold, then its socket end.
    pub(crate) fn kept(&self) -> [libc::c_int; 2] {
        [self.kept, self.socket]
    }

    /// This fork with `bound` in place of [`REAP_BOUND`] as the time a
    /// release or a drop gives the child to exit before killing it: for a
    /// test whose child is meant to be killed, so that it does not pay ten
    /// seconds to see the kill.
    pub(crate) fn reap_bound(mut self, bound: std::time::Duration) -> Self {
        self.reap_bound = bound;
        self
    }

    /// Whether the child has not ended, as the kernel answers it: `waitpid`
    /// with `WNOHANG` on this child's pid reports nothing while the child
    /// runs or is stopped, and collects it once it has ended. A child found
    /// ended is collected here and its status kept for the release, so a
    /// zombie never reads as alive and the pid is never waited for twice.
    /// An observation a signal interrupted observed nothing and is made
    /// again, after a rest of the observation tick ([`rest_within`]), within
    /// the reap bound of this call; one that failed otherwise, or was
    /// interrupted for the whole bound, answered neither alive nor ended,
    /// and this panics saying so rather than answer either.
    pub(crate) fn is_alive(&self) -> bool {
        if self.ended.get().is_some() {
            return false;
        }
        let started = std::time::Instant::now();
        loop {
            match self.observe() {
                Ok(Observed::NotEnded) => return true,
                Ok(Observed::Ended(status)) => {
                    self.ended.set(Some(status));
                    return false;
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {
                    assert!(
                        started.elapsed() < self.reap_bound,
                        "waitpid({}, WNOHANG) was interrupted at every observation for {:?}: {error}",
                        self.pid,
                        self.reap_bound
                    );
                }
                Err(error) => panic!("waitpid({}, WNOHANG): {error}", self.pid),
            }
            rest_within(
                OBSERVE_TICK,
                self.reap_bound.saturating_sub(started.elapsed()),
            );
        }
    }

    /// One observation of the child, `waitpid` with `WNOHANG`, as the
    /// kernel answers it: ended with its status, not ended -- running or
    /// stopped -- or the error the call answered, an interruption included.
    /// Nothing is retried here: the caller's loop, and its deadline, decide
    /// whether another observation is made (`PR320-R4-MAIN-001`,
    /// `PR320-R4-REG-002`).
    fn observe(&self) -> std::io::Result<Observed> {
        observe_child(self.pid)
    }

    /// Let the child go on to its exit, and reap it: its exit status, within
    /// the reap bound of the release or after a kill.
    ///
    /// # Panics
    ///
    /// When the release could not be written and the child then had to be
    /// killed -- the message says both, and the status the killed child was
    /// collected with -- or when the child could not be observed, killed or
    /// collected within the bounds: the message names the pid, the step and
    /// the state the child is left in, and the release's own failure when
    /// there was one too. Whatever the kernel allowed has been collected
    /// before the panic.
    pub(crate) fn release(mut self) -> std::process::ExitStatus {
        let reaped = self.release_and_reap();
        match (reaped.collected, reaped.released) {
            (Ok((status, false)), _) | (Ok((status, true)), Ok(())) => status,
            (Ok((status, true)), Err(write)) => panic!(
                "release the parked child {}: its release could not be written: {write}; it was \
                 killed at the end of {:?} and collected: {status}",
                self.pid, self.reap_bound
            ),
            (Err(collect), Ok(())) => panic!("release the parked child {}: {collect}", self.pid),
            (Err(collect), Err(write)) => panic!(
                "release the parked child {}: {collect}; its release could not be written \
                 either: {write}",
                self.pid
            ),
        }
    }

    /// The release and the reap, shared by [`Self::release`] and `Drop`: the
    /// release byte is written once ([`write_release`]) -- a write that
    /// neither waits for the child to read it, so a stopped child does not
    /// block it, nor is made again, so an interruption does not hold the
    /// owner outside its bound -- this process's end of the socket is
    /// dropped, so a child the byte did not reach reads EOF instead, and the
    /// child is then collected within the reap bound, killed first if it is
    /// still there at the end. Never panics, so `Drop` can run it while
    /// unwinding. The answer keeps the collection and the write apart
    /// ([`Reaped`]): a child killed and collected after a write that failed
    /// is collected, and the write failed, and neither is read as the other
    /// (`PR320-R5-MAIN-003`, `PR320-R5-REG-003`). A child that ended by
    /// itself, before or despite a failed write, ended the way its status
    /// says.
    fn release_and_reap(&mut self) -> Reaped {
        if let Some(status) = self.ended.get() {
            return Reaped {
                collected: Ok((status, false)),
                released: Ok(()),
            };
        }
        let released = match self.release.take() {
            // The stream is dropped at the end of this arm: the EOF that
            // releases a child the byte did not reach.
            Some(mut release) => write_release(&mut release),
            None => Ok(()),
        };
        Reaped {
            collected: self.collect(self.reap_bound),
            released,
        }
    }

    /// Collect the child within `bound`, killing it first if it is still
    /// there at the end of it: its status, and whether the kill was needed.
    /// Observed through `waitpid` with `WNOHANG` every [`OBSERVE_TICK`]
    /// rather than blocked on, so the bound is the bound whatever the child
    /// is doing and whatever the observations answer: one a signal
    /// interrupted observed nothing and costs the tick, never a retry of its
    /// own, a rest a signal or a policy cuts short costs less than the tick
    /// and is not made again ([`rest_within`], `PR320-R5-MAIN-006`), and the
    /// collection after the kill is observed the same way, within the reap
    /// bound from the kill, never blocked on (`PR320-R4-MAIN-001`,
    /// `PR320-R4-REG-002`). `bound` is how long the
    /// child is given to end by itself -- the reap bound after a release,
    /// nothing at all when the constructor failed -- and the reap bound is
    /// how long a killed child is given to be collected.
    ///
    /// # Errors
    ///
    /// Each names the pid, the step and what was observed -- the child
    /// alive at the last observation, or unobserved with the last answer --
    /// never more than was observed: a kill that was sent is a kill that
    /// was sent, and a child that read its release and ended on its own
    /// while its owner's observations were interrupted is a child that was
    /// sent a signal it never received, which the collection of it says by
    /// its status. So that a release can panic with it and a drop can print
    /// it: an observation that failed otherwise than by interruption, after
    /// which nothing is killed -- a pid that a failed observation cannot
    /// prove uncollected is not this fork's to signal; a kill the OS refused;
    /// or a kill sent and the child not collected within the reap bound of it.
    fn collect(
        &self,
        bound: std::time::Duration,
    ) -> std::io::Result<(std::process::ExitStatus, bool)> {
        if let Some(status) = self.ended.get() {
            return Ok((status, false));
        }
        let before_the_kill = match self.observed_within(bound) {
            Ok(Collected::Ended(status)) => return Ok((status, false)),
            Ok(Collected::Outlasted(last)) => last,
            Err(error) => {
                return Err(std::io::Error::new(
                    error.kind(),
                    format!(
                        "waitpid({}, WNOHANG): {error}; the child is left as it was, uncollected",
                        self.pid
                    ),
                ));
            }
        };
        // SAFETY: `kill` takes a pid and a signal by value. The pid is this
        // fork's child, which no observation above collected -- each
        // answered not ended, or nothing at all -- and so is still this
        // process's uncollected child: it cannot have been reused.
        if unsafe { libc::kill(self.pid, libc::SIGKILL) } == -1 {
            let error = std::io::Error::last_os_error();
            return Err(std::io::Error::new(
                error.kind(),
                format!(
                    "kill the parked child {} at the end of {bound:?}: {error}; the child is \
                     left uncollected, {}",
                    self.pid,
                    match before_the_kill {
                        None => String::from("alive at the last observation"),
                        Some(last) => format!("unobserved, the last observation answering: {last}"),
                    }
                ),
            ));
        }
        match self.observed_within(self.reap_bound) {
            Ok(Collected::Ended(status)) => Ok((status, true)),
            Ok(Collected::Outlasted(last)) => Err(std::io::Error::other(format!(
                "the parked child {} was sent SIGKILL at the end of {bound:?} and not collected \
                 within another {:?}; {}",
                self.pid,
                self.reap_bound,
                match last {
                    None => String::from("not ended at the last observation"),
                    Some(last) => format!("unobserved, the last observation answering: {last}"),
                }
            ))),
            Err(error) => Err(std::io::Error::new(
                error.kind(),
                format!(
                    "waitpid({}, WNOHANG) after the kill: {error}; the child was sent SIGKILL and \
                     is left uncollected",
                    self.pid
                ),
            )),
        }
    }

    /// Observe the child every [`OBSERVE_TICK`] until it has ended or
    /// `bound` runs out, from now: its status, kept for the release, or what
    /// the last observation answered at the end of the bound -- nothing, or
    /// the interruption. A `bound` of zero is one observation. Each rest
    /// between two observations is one `nanosleep` of the tick, or of what
    /// is left of `bound` when that is less, and a rest a signal or a policy
    /// cuts short is not made again ([`rest_within`]): the loop looks at the
    /// bound next, whatever the rest answered (`PR320-R5-MAIN-006`).
    ///
    /// # Errors
    ///
    /// An observation that failed otherwise than by interruption, as it was.
    fn observed_within(&self, bound: std::time::Duration) -> std::io::Result<Collected> {
        let started = std::time::Instant::now();
        // What the observation answered, set by every observation that did
        // not end the loop before it is read.
        let mut interrupted: Option<std::io::Error>;
        loop {
            match self.observe() {
                Ok(Observed::Ended(status)) => {
                    self.ended.set(Some(status));
                    return Ok(Collected::Ended(status));
                }
                Ok(Observed::NotEnded) => interrupted = None,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {
                    interrupted = Some(error);
                }
                Err(error) => return Err(error),
            }
            if started.elapsed() >= bound {
                return Ok(Collected::Outlasted(interrupted));
            }
            rest_within(OBSERVE_TICK, bound.saturating_sub(started.elapsed()));
        }
    }
}

/// What a release and reap came to ([`ParkedFork::release_and_reap`]): the
/// collection and the release write, each as it answered. A write that
/// failed and a child the kill then ended and the owner collected are both
/// facts, and neither stands in for the other (`PR320-R5-MAIN-003`,
/// `PR320-R5-REG-003`).
#[cfg(unix)]
struct Reaped {
    /// The child's status and whether the kill was needed, or the error that
    /// names the step and the state the child is left in
    /// ([`ParkedFork::collect`]).
    collected: std::io::Result<(std::process::ExitStatus, bool)>,
    /// The release write's own answer: `Ok` when the byte was written or
    /// there was nothing left to write.
    released: std::io::Result<()>,
}

#[cfg(unix)]
impl Drop for ParkedFork {
    fn drop(&mut self) {
        // Best effort by nature: a drop has no caller to answer to, the
        // status is `release`'s to return, and a panic here would abort a
        // test already unwinding through it. What a drop guarantees is what
        // `release_and_reap` does before it returns anything at all: the
        // child is released and collected, killed first past the reap bound,
        // every step bounded. What it could not do is said on stderr, the
        // one channel a drop has, each state as it is (`PR320-R4-MAIN-003`):
        // a release that could not be written, with the child killed and
        // collected after it and its status (`PR320-R5-MAIN-003`,
        // `PR320-R5-REG-003`); or a child left uncollected -- alive after a
        // kill the OS refused, killed and not collected, or not observed --
        // with the error that names the step, and the release's failure too
        // when there was one. Said through `say_on_stderr`, which writes the
        // descriptor itself and makes nothing again: an interrupted or
        // refused write ends the saying and never the drop's bound, and an
        // error is never a panic (`PR320-R5-MAIN-001`, `PR320-R5-REG-001`).
        // This file forbids the print macros (`PR6-LANEF-004`, the header).
        // A stderr that cannot be written leaves a drop no channel at all.
        let reaped = self.release_and_reap();
        let line = match (&reaped.collected, &reaped.released) {
            (Ok((_, false)), _) | (Ok((_, true)), Ok(())) => return,
            (Ok((status, true)), Err(write)) => format!(
                "[the parked child {} could not be released by its owner's drop: {write}; it was \
                 killed at the end of {:?} and collected: {status}]\n",
                self.pid, self.reap_bound
            ),
            (Err(collect), Ok(())) => format!(
                "[the parked child {} was left uncollected by its owner's drop: {collect}]\n",
                self.pid
            ),
            (Err(collect), Err(write)) => format!(
                "[the parked child {} was left uncollected by its owner's drop: {collect}; its \
                 release could not be written either: {write}]\n",
                self.pid
            ),
        };
        say_on_stderr(&line);
    }
}

/// What one observation of a child answered ([`observe_child`]).
#[cfg(unix)]
#[derive(Clone, Copy, Debug)]
enum Observed {
    /// The child has ended, with this status, and is now collected.
    Ended(std::process::ExitStatus),
    /// The child has not ended: running, or stopped.
    NotEnded,
}

/// What observing a child until a bound answered
/// ([`ParkedFork::observed_within`]).
#[cfg(unix)]
#[derive(Debug)]
enum Collected {
    /// The child ended within the bound, with this status.
    Ended(std::process::ExitStatus),
    /// The child had not ended at the end of the bound; the last observation
    /// answered nothing, or this interruption.
    Outlasted(Option<std::io::Error>),
}

/// How often a child is observed while it is given time to end: the tick of
/// [`ParkedFork::collect`], [`ParkedFork::observed_within`] and an
/// interrupted [`ParkedFork::is_alive`].
#[cfg(unix)]
const OBSERVE_TICK: std::time::Duration = std::time::Duration::from_millis(5);

/// One `waitpid` on `pid` with `WNOHANG`, as it answers: the child ended
/// with its status, not ended, or the error -- an interruption included --
/// and nothing retried.
///
/// # Errors
///
/// What `waitpid` answered, as it was.
#[cfg(unix)]
fn observe_child(pid: libc::pid_t) -> std::io::Result<Observed> {
    use std::os::unix::process::ExitStatusExt as _;

    let mut status = 0;
    // SAFETY: `waitpid` writes one int through `status`, which lives for
    // the call, and takes the pid and the flags by value; the pid is a
    // child of this process.
    let answered = unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) };
    if answered == pid {
        return Ok(Observed::Ended(std::process::ExitStatus::from_raw(status)));
    }
    if answered == 0 {
        return Ok(Observed::NotEnded);
    }
    Err(std::io::Error::last_os_error())
}

/// The release byte, written once to `writer`: one attempt, made whatever
/// it answers and never made again -- not on an interruption, not on a
/// short write. A write that failed is answered as it was; what releases
/// the child then is the EOF the caller's drop of the socket gives it, or
/// the kill at the end of the reap bound. `write_all` would make an
/// interrupted write again inside itself, outside every bound the owner
/// keeps (`PR320-R4-REG-002`); the one byte cannot block, the socket's
/// buffer being empty, so there is nothing a second attempt could wait for.
///
/// # Errors
///
/// The write's own error, or `WriteZero` for a write that answered fewer
/// bytes than the one.
#[cfg(unix)]
pub(crate) fn write_release(writer: &mut impl std::io::Write) -> std::io::Result<()> {
    match writer.write(&[1]) {
        Ok(1) => Ok(()),
        Ok(count) => Err(std::io::Error::new(
            std::io::ErrorKind::WriteZero,
            format!("the release wrote {count} of 1 byte"),
        )),
        Err(error) => Err(error),
    }
}

/// Rest for `span`, once: one `nanosleep`, whatever it answers. A signal that
/// interrupts it, or a policy that refuses it, ends the rest early, and the
/// rest is not made again: the caller's loop, and the bound it keeps, decide
/// what happens next. `std::thread::sleep` makes an interrupted sleep again
/// inside itself for what it had left, and a refused one again for all of it,
/// so an owner whose rests were refused never came back to look at its bound
/// (`PR320-R5-MAIN-006`).
#[cfg(unix)]
pub(crate) fn rest(span: std::time::Duration) {
    let request = libc::timespec {
        tv_sec: libc::time_t::try_from(span.as_secs()).unwrap_or(libc::time_t::MAX),
        tv_nsec: libc::c_long::from(span.subsec_nanos()),
    };
    // SAFETY: `nanosleep` reads the one `timespec` it is handed, which lives
    // for the call, and writes nothing: the remainder pointer is null.
    if unsafe { libc::nanosleep(&request, std::ptr::null_mut()) } == -1 {
        // Interrupted or refused: the rest ended early, which is all a rest
        // can do wrong, and the caller's bound is what ends its loop.
    }
}

/// [`rest`] on Windows: `std::thread::sleep`, which there is one wait of the
/// span -- a high-resolution waitable timer, or `Sleep` where there is none --
/// and nothing it makes again, no signal interrupting it and no thread policy
/// refusing it. For the waits shared with the Unix suites
/// (`wait_for_cleanup_hold_release_observing`, `await_signal_by`), whose rest
/// is one attempt on every platform.
#[cfg(windows)]
pub(crate) fn rest(span: std::time::Duration) {
    std::thread::sleep(span);
}

/// [`rest`] for `tick`, or for `remaining` when that is less: the rest an
/// owner's loop takes between two turns never carries it past the bound it
/// keeps, and one cut short costs less than the tick.
#[cfg(any(unix, windows))]
pub(crate) fn rest_within(tick: std::time::Duration, remaining: std::time::Duration) {
    rest(tick.min(remaining));
}

/// Refuse every rest this thread takes, and every rest of every process it
/// forks: a seccomp policy answering `EINTR` to `clock_nanosleep`, the call
/// a sleep makes here, then one real `nanosleep`, which it must refuse, so
/// that a test built on it enters the refused rest rather than assuming it.
/// For the suites that cannot reach the rundir suite's own policies
/// (`Refusal::Rests` is that module's): the recovery trunks' lease-wait
/// regressions, which run it in a process of their own, a policy being its
/// thread's for good (`PR320-R6-REG-001`).
///
/// # Errors
///
/// The refusal is not in force: the `nanosleep` was made, or answered
/// something other than `EINTR`.
///
/// # Panics
///
/// When the kernel refuses the policy itself.
#[cfg(target_os = "linux")]
pub(crate) fn refuse_rests_on_this_thread() -> Result<(), String> {
    const NONE: libc::c_long = 0;
    const NO_NEW_PRIVS: libc::c_long = 1;

    let instruction = |code: u32, jt: u8, jf: u8, k: u32| libc::sock_filter {
        code: u16::try_from(code).expect("a BPF instruction class fits the kernel's field"),
        jt,
        jf,
        k,
    };
    let word = |value: libc::c_long| {
        u32::try_from(value).expect("a syscall number or errno fits the kernel's field")
    };
    let mut program = [
        instruction(libc::BPF_LD | libc::BPF_W | libc::BPF_ABS, 0, 0, 0),
        instruction(
            libc::BPF_JMP | libc::BPF_JEQ | libc::BPF_K,
            0,
            1,
            word(libc::SYS_clock_nanosleep),
        ),
        instruction(
            libc::BPF_RET | libc::BPF_K,
            0,
            0,
            libc::SECCOMP_RET_ERRNO | word(libc::c_long::from(libc::EINTR)),
        ),
        instruction(libc::BPF_RET | libc::BPF_K, 0, 0, libc::SECCOMP_RET_ALLOW),
    ];
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
    let request = libc::timespec {
        tv_sec: 0,
        tv_nsec: 1,
    };
    // SAFETY: `nanosleep` reads the one `timespec`, which lives for the call;
    // the remainder pointer is null.
    let answered = unsafe { libc::nanosleep(&request, std::ptr::null_mut()) };
    let error = std::io::Error::last_os_error();
    match (answered, error.raw_os_error()) {
        (-1, Some(libc::EINTR)) => Ok(()),
        (-1, _) => Err(format!("nanosleep answered {error}, not the refusal")),
        _ => Err(String::from(
            "nanosleep was made: the refusal is not in force",
        )),
    }
}

/// Say `line` on this process's standard error, as far as the descriptor
/// takes it and no further: [`write_while_progressing`] on descriptor 2
/// itself ([`StandardError`]), so every write is one `write(2)`, no lock is
/// shared with the rest of the process, and the first answer that is not
/// progress -- an interruption, a refusal, an error, nothing taken -- ends
/// the saying, never a retry and never a panic. This is how an owner's drop
/// reports what it could not do: a drop that retried a refused channel
/// would not return, and one that panicked on a broken channel would abort
/// the test it was cleaning up after (`PR320-R5-MAIN-001`,
/// `PR320-R5-REG-001`, `PR320-R5-REG-002`). A channel the OS refuses
/// carries nothing, and that is the OS's answer, not the owner's failure.
#[cfg(unix)]
pub(crate) fn say_on_stderr(line: &str) {
    if write_while_progressing(&mut StandardError, line.as_bytes()).is_err() {
        // The channel refused, was interrupted or took nothing: there is no
        // other channel to say so on, and nothing to do about it here.
    }
}

/// Write `bytes` to `writer` for as long as every write makes progress: one
/// write per turn, a turn that wrote something followed by a write of the
/// rest, and the first answer that is not progress -- an error, an
/// interruption included, or nothing written -- the end, answered as it was
/// and never made again. At most `bytes.len()` writes, whatever the writer
/// answers. `write_all` makes an interrupted write again inside itself, and
/// forever when every write is interrupted (`PR320-R5-MAIN-001`,
/// `PR320-R5-REG-001`).
///
/// # Errors
///
/// The first error a write answered, or `WriteZero` for a write that took
/// nothing, saying how much had been written.
#[cfg(unix)]
pub(crate) fn write_while_progressing(
    writer: &mut impl std::io::Write,
    bytes: &[u8],
) -> std::io::Result<()> {
    let mut written = 0_usize;
    while let Some(rest) = bytes.get(written..).filter(|rest| !rest.is_empty()) {
        match writer.write(rest) {
            Ok(0) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::WriteZero,
                    format!(
                        "a write took nothing after {written} of {} bytes",
                        bytes.len()
                    ),
                ));
            }
            Ok(count) => written = written.saturating_add(count),
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

/// This process's standard error as descriptor 2 itself: each write one
/// `write(2)`, answered as it was -- no buffer, no lock shared with the rest
/// of the process, nothing made again. `std::io::stderr()` takes a lock every
/// thread shares and retries an interruption in `write_all`; the print
/// macros do the same and panic on an error ([`say_on_stderr`]).
#[cfg(unix)]
struct StandardError;

#[cfg(unix)]
impl std::io::Write for StandardError {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        // SAFETY: `write` reads `bytes` for its length and writes no memory;
        // descriptor 2 is whatever this process holds as its standard error,
        // and a closed one is answered `EBADF`.
        let written =
            unsafe { libc::write(libc::STDERR_FILENO, bytes.as_ptr().cast(), bytes.len()) };
        usize::try_from(written).map_err(|_| std::io::Error::last_os_error())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// This process's descriptor table as `/dev/fd` lists it now, sorted: the
/// numbers the child's sweep is verified against after a range close, and
/// what the ceiling below is read from. Computed by the parent before it
/// forks: listing a directory is not something the forked child of a
/// threaded process may do. The listing's own directory descriptor is among
/// the numbers and is closed again before the fork, so the child finds it
/// closed like any other.
///
/// # Panics
///
/// When `/dev/fd` cannot be listed: the domain of the child's sweep would
/// then be a guess, and a helper that guesses announces an isolation it did
/// not check. Linux and macOS, the two Unix targets CI runs, both have it.
#[cfg(unix)]
fn listed_descriptors() -> Vec<libc::c_int> {
    let mut listed: Vec<libc::c_int> = std::fs::read_dir("/dev/fd")
        .expect("list this process's descriptor table through /dev/fd")
        .filter_map(|entry| {
            entry
                .expect("an entry of /dev/fd")
                .file_name()
                .to_string_lossy()
                .parse::<libc::c_int>()
                .ok()
        })
        .collect();
    listed.sort_unstable();
    listed
}

/// One past the highest number a descriptor of this process can have at a
/// fork made now: the larger of `_SC_OPEN_MAX` -- the soft limit every open
/// made from here on stays below -- and one past the highest number this
/// process's own table holds as `/dev/fd` lists it (`listed`, from
/// [`listed_descriptors`]), which is where a descriptor opened before the
/// soft limit was lowered still sits, above what `sysconf` alone would say.
#[cfg(unix)]
fn descriptor_ceiling(listed: &[libc::c_int]) -> libc::c_int {
    // SAFETY: `sysconf` reads a process limit and touches no memory.
    let soft = unsafe { libc::sysconf(libc::_SC_OPEN_MAX) };
    // A limit `sysconf` cannot report (-1) contributes nothing; the listing
    // still covers every descriptor that exists. A limit above what a
    // `c_int` holds exists on neither supported Unix and is read the same.
    let soft = libc::c_int::try_from(soft).unwrap_or(0).max(0);
    let highest = listed.last().copied().unwrap_or(-1);
    soft.max(highest.saturating_add(1))
}

/// The child of [`ParkedFork::holding`]'s fork, from its first instruction
/// to its `_exit`: the sweep, the report, the park, the exit. `listed` is the
/// parent's table at the fork ([`listed_descriptors`]), read here and never
/// written or freed: the child `_exit`s.
///
/// # Safety
///
/// For the child of a `fork` of a threaded process, as its first act, and
/// nowhere else: it calls only async-signal-safe syscalls -- `close`,
/// `close_range` through `syscall`, `fcntl`, `getpid`, `write`, `read`,
/// `_exit` -- on descriptors the child inherited, allocates nothing and
/// takes no lock.
#[cfg(unix)]
unsafe fn park(
    kept: libc::c_int,
    socket: libc::c_int,
    ceiling: libc::c_int,
    listed: &[libc::c_int],
) -> ! {
    // SAFETY: the sweep's own contract, which is this fn's.
    let swept = unsafe { close_above_stdio_except([kept, socket], ceiling, listed) };
    let (tag, first, second) = match swept {
        // SAFETY: `getpid` takes nothing and reads nothing.
        Ok(()) => (REPORT_PARKED, unsafe { libc::getpid() }, 0),
        Err(failed) => (REPORT_CLOSE_FAILED, failed.descriptor, failed.errno),
    };
    let report = encode_report(tag, first, second);
    // SAFETY: `write` reads `report` for its length; `socket` is the child's
    // own end, which the sweep kept.
    let reported = unsafe { write_fully(socket, &report) };
    if reported.is_err() || tag != REPORT_PARKED {
        // SAFETY: `_exit` ends this child without unwinding, which is the
        // only way out of the forked child of a threaded process.
        unsafe { libc::_exit(1) }
    }
    let mut byte = [0_u8; 1];
    loop {
        // SAFETY: `read` writes at most one byte into `byte`.
        let read = unsafe { libc::read(socket, byte.as_mut_ptr().cast(), 1) };
        // One byte is the release; zero is EOF, the parent gone.
        if read >= 0 {
            break;
        }
        if errno() != libc::EINTR {
            // SAFETY: as above.
            unsafe { libc::_exit(2) }
        }
    }
    // SAFETY: as above.
    unsafe { libc::_exit(0) }
}

/// The nine bytes of a report.
#[cfg(unix)]
fn encode_report(tag: u8, first: libc::c_int, second: libc::c_int) -> [u8; REPORT_LEN] {
    let mut report = [0_u8; REPORT_LEN];
    let (head, numbers) = report.split_at_mut(1);
    let (first_bytes, second_bytes) = numbers.split_at_mut(4);
    head.copy_from_slice(&[tag]);
    first_bytes.copy_from_slice(&first.to_ne_bytes());
    second_bytes.copy_from_slice(&second.to_ne_bytes());
    report
}

/// The tag and the two numbers of a report.
#[cfg(unix)]
fn decode_report(report: &[u8; REPORT_LEN]) -> (u8, libc::c_int, libc::c_int) {
    let (head, numbers) = report.split_at(1);
    let (first_bytes, second_bytes) = numbers.split_at(4);
    let mut first = [0_u8; 4];
    let mut second = [0_u8; 4];
    first.copy_from_slice(first_bytes);
    second.copy_from_slice(second_bytes);
    (
        head.first().copied().unwrap_or(u8::MAX),
        libc::c_int::from_ne_bytes(first),
        libc::c_int::from_ne_bytes(second),
    )
}

/// Read one report from `reader` before `deadline`: the nine bytes, or the
/// error. One absolute deadline for the whole report, whatever the reads
/// answer, looked at before every turn: a read that is interrupted, would
/// block or times out -- the reader's own tick, [`READY_TICK`] on the
/// handshake socket -- is made again against the same deadline and never a
/// fresh one; the bytes a short read did deliver are kept and the next read
/// is made against the same deadline too; and a report complete only after
/// the deadline is not accepted, so progress and completion cross the one
/// deadline exactly as interruptions do (`PR320-R4-MAIN-002`,
/// `PR320-R4-REG-001`). `read_exact` would retry an interruption itself,
/// each retry a fresh socket timeout, so a signal handled more often than
/// the timeout would hold the caller for as long as the signals came
/// (`PR320-R3-MAIN-004`). EOF before the report is the child gone before it
/// reported: an error saying how much arrived.
///
/// # Errors
///
/// `TimedOut` once the deadline has passed, saying how many bytes had
/// arrived and what the last read answered; `UnexpectedEof` for a reader
/// that ended early; and any other error the reader answered, as it was.
#[cfg(unix)]
pub(crate) fn read_report_within(
    reader: &mut impl std::io::Read,
    deadline: std::time::Instant,
) -> std::io::Result<[u8; REPORT_LEN]> {
    read_report_within_by(reader, deadline, std::time::Instant::now)
}

/// [`read_report_within`] with `now` in place of `Instant::now`: the clock
/// the deadline is read against, once at the top of every turn, so that a
/// test drives what the clock answers between reads rather than arranging
/// it with a scheduler -- the seam `readiness::await_signal_by` gives its
/// wait, for the same reason.
///
/// # Errors
///
/// As [`read_report_within`].
#[cfg(unix)]
pub(crate) fn read_report_within_by(
    reader: &mut impl std::io::Read,
    deadline: std::time::Instant,
    mut now: impl FnMut() -> std::time::Instant,
) -> std::io::Result<[u8; REPORT_LEN]> {
    let mut report = [0_u8; REPORT_LEN];
    let mut filled = 0_usize;
    let mut reads = 0_u32;
    let mut last: Option<std::io::Error> = None;
    loop {
        if now() >= deadline {
            let answered = match (reads, last.as_ref()) {
                (0, _) => String::from("no read was made"),
                (_, None) => String::from("the last read delivered bytes"),
                (_, Some(error)) => format!("the last read answered: {error}"),
            };
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!(
                    "the report deadline passed with {filled} of {REPORT_LEN} bytes read; {answered}"
                ),
            ));
        }
        let Some(rest) = report.get_mut(filled..).filter(|rest| !rest.is_empty()) else {
            return Ok(report);
        };
        reads += 1;
        match reader.read(rest) {
            Ok(0) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    format!("the child's end closed after {filled} of {REPORT_LEN} report bytes"),
                ));
            }
            Ok(count) => {
                filled += count;
                last = None;
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::Interrupted
                        | std::io::ErrorKind::WouldBlock
                        | std::io::ErrorKind::TimedOut
                ) =>
            {
                last = Some(error);
            }
            Err(error) => return Err(error),
        }
    }
}

/// `write` until every byte of `bytes` is written, `EINTR` retried; the
/// errno of a write that failed otherwise.
///
/// # Safety
///
/// For the forked child only, as [`park`]: async-signal-safe because it is
/// one syscall in a loop and nothing more.
#[cfg(unix)]
unsafe fn write_fully(fd: libc::c_int, bytes: &[u8]) -> Result<(), libc::c_int> {
    let mut written = 0_usize;
    while let Some(rest) = bytes.get(written..).filter(|rest| !rest.is_empty()) {
        // SAFETY: `write` reads `rest` for its length from a live descriptor.
        let count = unsafe { libc::write(fd, rest.as_ptr().cast(), rest.len()) };
        match usize::try_from(count) {
            Ok(count) => written += count,
            Err(_) => {
                let errno = errno();
                if errno != libc::EINTR {
                    return Err(errno);
                }
            }
        }
    }
    Ok(())
}

/// The calling thread's errno, as the last OS error reports it.
#[cfg(unix)]
fn errno() -> libc::c_int {
    std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

/// A descriptor the sweep left open: which, and what its close answered --
/// the errno, or 0 for a close that answered success while the descriptor
/// read open after it, which a policy can do as easily as it answers an errno.
#[cfg(unix)]
#[derive(Clone, Copy, Debug)]
struct CloseFailed {
    descriptor: libc::c_int,
    errno: libc::c_int,
}

/// Close every descriptor of the calling process from 3 up to `ceiling`,
/// exclusive, except the two in `keep`, and say so or say which descriptor
/// is still open. Whether a descriptor is closed is read from the kernel's
/// descriptor table, `fcntl` with `F_GETFD`, and never from what a close
/// answered: a policy answers `EBADF`, `EINTR` or success without making the
/// close, and an errno says nothing about whether the operation it names was
/// made (`PR320-R2-MAIN-004`, `PR320-R3-REG-002`). On Linux, `close_range`
/// over each gap between the kept descriptors and above the last
/// ([`close_ranges_except`]); when every call answers success, each number the
/// parent listed at the fork (`listed`) except the kept two is asked and must
/// read `EBADF`, or it is reported with its close's answer, 0. When any of
/// those calls fails -- unavailable, `ENOSYS` before Linux 5.9; refused,
/// `EPERM` under a seccomp policy; or anything else -- and on every other
/// Unix, one pass by number: a number that reads `EBADF` before any close is
/// not open and is skipped, so a closed number costs what it cost before;
/// one that reads open is closed and asked again, `EBADF` then being the
/// close, and any other reading a descriptor still open, reported with the
/// close's answer -- `EIO`, `EINTR`, `EBADF` or 0 alike
/// (`rundir::tests::a_parked_fork_whose_sweep_is_denied_a_close_with_eintr_fails_before_announcing_the_child`,
/// `rundir::tests::a_parked_fork_whose_sweep_is_denied_a_close_with_ebadf_fails_before_announcing_the_child`).
/// Nothing is retried: a close that did not close is a failed sweep, and the
/// constructor collects the child and fails. The forked child is
/// single-threaded and opens nothing, so the number a close was asked about
/// is the number the readings before and after it are of. The ceiling is the
/// caller's ([`descriptor_ceiling`]), never `sysconf` alone, because a soft
/// limit lowered after a descriptor was opened leaves that descriptor above
/// `_SC_OPEN_MAX`.
///
/// # Safety
///
/// For the child of a `fork` before it does anything else, and nowhere
/// else: it closes descriptors the caller does not own, and it is
/// async-signal-safe only because it makes syscalls and nothing more.
#[cfg(unix)]
unsafe fn close_above_stdio_except(
    keep: [libc::c_int; 2],
    ceiling: libc::c_int,
    listed: &[libc::c_int],
) -> Result<(), CloseFailed> {
    #[cfg(target_os = "linux")]
    {
        // SAFETY: the range sweep's contract, which is this fn's.
        if unsafe { close_ranges_except(keep) } {
            for &descriptor in listed {
                if descriptor < 3 || keep.contains(&descriptor) {
                    continue;
                }
                // SAFETY: `fcntl` with `F_GETFD` reads one flag of this
                // process's own descriptor table and touches no memory;
                // async-signal-safe.
                if unsafe { libc::fcntl(descriptor, libc::F_GETFD) } == -1 && errno() == libc::EBADF
                {
                    continue;
                }
                return Err(CloseFailed {
                    descriptor,
                    errno: 0,
                });
            }
            return Ok(());
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = listed;
    for descriptor in 3..ceiling {
        if keep.contains(&descriptor) {
            continue;
        }
        // SAFETY: `fcntl` with `F_GETFD` reads one flag of this process's
        // own descriptor table and touches no memory; async-signal-safe.
        if unsafe { libc::fcntl(descriptor, libc::F_GETFD) } == -1 {
            let unreadable = errno();
            if unreadable == libc::EBADF {
                continue;
            }
            return Err(CloseFailed {
                descriptor,
                errno: unreadable,
            });
        }
        // SAFETY: a numeric close in the forked child; the fn's own contract.
        let answered = if unsafe { libc::close(descriptor) } == -1 {
            errno()
        } else {
            0
        };
        // SAFETY: as above, on the number the close was just asked about,
        // which nothing in this child can have reused.
        if unsafe { libc::fcntl(descriptor, libc::F_GETFD) } == -1 && errno() == libc::EBADF {
            continue;
        }
        return Err(CloseFailed {
            descriptor,
            errno: answered,
        });
    }
    Ok(())
}

/// `close_range` over every gap between the kept descriptors, from 3, and
/// above the last of them: `true` when every call succeeded, `false` at the
/// first that did not, after which the caller's loop closes everything
/// itself and the ranges already closed answer `EBADF` to it.
///
/// # Safety
///
/// As [`close_above_stdio_except`].
#[cfg(target_os = "linux")]
unsafe fn close_ranges_except(keep: [libc::c_int; 2]) -> bool {
    let mut keep = keep;
    keep.sort_unstable();
    let mut first = 3_u32;
    for kept in keep {
        let Ok(kept) = u32::try_from(kept) else {
            continue;
        };
        if kept < first {
            continue;
        }
        // SAFETY: a raw syscall over a numeric range; the fn's own contract.
        if kept > first
            && unsafe { libc::syscall(libc::SYS_close_range, first, kept - 1, 0_u32) } != 0
        {
            return false;
        }
        first = kept + 1;
    }
    // SAFETY: as above.
    unsafe { libc::syscall(libc::SYS_close_range, first, u32::MAX, 0_u32) == 0 }
}

// -----------------------------------------------------------------------
// The ambient controls over `refs/replace/*`
// -----------------------------------------------------------------------

/// Removed outright from a Git child that measures replacement behaviour:
/// each of these decides that behaviour by its **presence**, or hands the
/// child a path or a ref namespace the fixture did not write.
///
/// `GIT_NO_REPLACE_OBJECTS` and `GIT_REPLACE_REF_BASE` are `git-replace(1)`'s
/// and `git(1)`'s own; `GIT_CONFIG_PARAMETERS` is the undocumented variable
/// Git itself uses to propagate `-c` into its subprocesses, and it reaches
/// `core.useReplaceRefs` exactly as `-c` does; `GIT_CONFIG` redirects
/// `git config`'s **writes**, so a fixture that pins a key with it set would
/// pin it in the operator's file and not in its own repository.
///
/// `GIT_TEMPLATE_DIR` is the seventeenth mechanism, and the one that says why
/// the enumeration is no longer what correctness rests on: it was found by a
/// reviewer within the hour of the previous sixteen being written down.
/// Measured on git 2.43.0, `git init` copies a template's `config` **into the
/// new repository**, above everything `git init` itself writes there, so a
/// template naming `[core] useReplaceRefs = false` decides the question for a
/// repository the fixture created and never asked -- and it does it at
/// creation, which no later environment neutralisation can undo. The same
/// value arrives through `init.templateDir` in a global or system file
/// (measured), and that half is closed by the two pins below.
pub(crate) const REPLACEMENT_CONTROLS_REMOVED: &[&str] = &[
    "GIT_NO_REPLACE_OBJECTS",
    "GIT_REPLACE_REF_BASE",
    "GIT_CONFIG_PARAMETERS",
    "GIT_CONFIG",
    "GIT_TEMPLATE_DIR",
];

/// Removed by prefix: the indexed configuration pairs, every index of them.
///
/// **The claim this carried until round 4 was an artefact of the environment it
/// was measured in, and is corrected rather than left standing.** It said that
/// on git 2.43.0 a lone `GIT_CONFIG_KEY_0`/`GIT_CONFIG_VALUE_0` takes effect
/// with `GIT_CONFIG_COUNT` absent. It does not: measured 2026-09-12 with the
/// count genuinely unset, `git config --get core.useReplaceRefs` exits `1` and
/// prints nothing, and exits `0` printing `false` with the count at `1`. What
/// made the earlier reading come out the other way is that this box's build
/// wrapper exports `GIT_CONFIG_COUNT=1` and an indexed pair of its own into
/// every `cargo test` it runs, so the "uncounted" pair was counted by an
/// inherited count nobody had taken away. `git-config(1)` describes the
/// documented behaviour and git 2.43.0 has it.
///
/// What closes the vector is the `GIT_CONFIG_COUNT=0` pin below, and these
/// removals are a second, independent way: measured on git 2.43.0, emptying
/// this list leaves the grid green, so it is defence in depth and not the
/// load-bearing half. They also close the case the pin cannot -- a pair set on
/// the `Command` itself rather than inherited, which the sweep over this
/// process's environment would not see.
pub(crate) const REPLACEMENT_CONTROL_PREFIXES: &[&str] = &["GIT_CONFIG_KEY_", "GIT_CONFIG_VALUE_"];

/// Pinned to a fixed value rather than removed, because an absent value is
/// not a neutral one: absent `GIT_CONFIG_GLOBAL` means *read `$HOME`'s and
/// `$XDG_CONFIG_HOME`'s*, and absent `GIT_CONFIG_COUNT` means *read whatever
/// indexed pairs are there*.
///
/// The two file variables are pinned at a file this process wrote empty, so
/// no `[core] useReplaceRefs`, and no `include.path`/`includeIf` reaching
/// one, survives at either level -- including at `git init`, where a global
/// `init.templateDir` otherwise copies a `config` **into the new repository**
/// and lands below every later `git config` (measured). With both pinned,
/// `git config --show-scope --list` reports `command` and `local` and nothing
/// else (measured), which is the whole claim.
///
/// `GIT_CONFIG_NOSYSTEM` is redundant while `GIT_CONFIG_SYSTEM` is pinned --
/// git(1) says setting the latter means the build-time system file is not
/// read, and the scope listing above confirms it. It is pinned anyway so that
/// neither variable is alone in carrying the claim. The grid does go red when
/// it is dropped, but that is `assert_replacement_controls_pinned`'s own
/// precondition failing, not a measured vector; do not read it as evidence
/// this one is load-bearing.
pub(crate) const REPLACEMENT_CONTROLS_PINNED: &[&str] = &[
    "GIT_CONFIG_COUNT",
    "GIT_CONFIG_GLOBAL",
    "GIT_CONFIG_SYSTEM",
    "GIT_CONFIG_NOSYSTEM",
];

/// An empty Git configuration file, written once per test process.
///
/// A file this process wrote rather than a path it expects to be absent: Git
/// reads a missing file as empty today, but "nothing created this path" is a
/// claim about the rest of the run, and "this file is empty" is a fact.
/// `/dev/null` is not portable to the Windows leg of the matrix.
pub(crate) fn neutral_git_config() -> &'static Path {
    static FILE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    FILE.get_or_init(|| {
        let path =
            std::env::temp_dir().join(format!("upstroke-neutral-gitconfig-{}", std::process::id()));
        fs::write(&path, b"").unwrap_or_else(|error| {
            panic!(
                "writing the neutral Git configuration {}: {error}",
                path.display()
            )
        });
        path
    })
    .as_path()
}

/// [`REPLACEMENT_CONTROLS_PINNED`] with the values they are pinned to.
pub(crate) fn pinned_replacement_controls() -> Vec<(&'static str, OsString)> {
    let neutral = neutral_git_config().as_os_str().to_owned();
    vec![
        ("GIT_CONFIG_COUNT", OsString::from("0")),
        ("GIT_CONFIG_GLOBAL", neutral.clone()),
        ("GIT_CONFIG_SYSTEM", neutral),
        ("GIT_CONFIG_NOSYSTEM", OsString::from("1")),
    ]
}

/// Whether `key` is one of the enumerated ambient controls, under either
/// name rule: Windows matches environment keys case-insensitively, so a
/// filter that matched only the documented spelling would pass
/// `git_no_replace_objects` straight through on that leg of the matrix.
pub(crate) fn is_ambient_replacement_control(key: &OsStr) -> bool {
    let name = key.to_string_lossy().to_ascii_uppercase();
    REPLACEMENT_CONTROLS_REMOVED.contains(&name.as_str())
        || REPLACEMENT_CONTROLS_PINNED.contains(&name.as_str())
        || REPLACEMENT_CONTROL_PREFIXES
            .iter()
            .any(|prefix| name.starts_with(prefix))
}

/// Take every ambient control over `refs/replace/*` away from `command`'s
/// child, so what the child measures is what the code under test installs.
///
/// Rounds 1 and 2 of PR #271 each shipped a witness that passed without its
/// own fix, through two different controls -- `GIT_NO_REPLACE_OBJECTS` and
/// then `core.useReplaceRefs=false`. This is the enumeration both were
/// missing, applied in one place. It is not closed by construction, so the
/// witnesses that could pass silently also call
/// [`assert_replacement_refs_are_live`], which closes it by measurement.
///
/// `the_neutraliser_defeats_every_ambient_control_it_enumerates` is where
/// this function is exercised: CI exports none of these, so without that grid
/// a name could be dropped here and nothing would go red until a reviewer
/// exported it, which is how rounds 1 and 2 were found. Measured against it,
/// dropping `GIT_NO_REPLACE_OBJECTS`, `GIT_CONFIG_PARAMETERS`,
/// `GIT_REPLACE_REF_BASE`, the `GIT_CONFIG_COUNT` pin, the
/// `GIT_CONFIG_GLOBAL` pin or the `GIT_CONFIG_SYSTEM` pin each turns it red.
/// `GIT_CONFIG` is not one of that grid's rows -- it does not change what a
/// Git child reads -- and has its own witness,
/// `a_redirected_git_config_cannot_capture_a_fixtures_own_pin`, which goes red
/// when it is dropped from the list here.
pub(crate) fn without_ambient_replacement_controls(command: &mut Command) {
    for key in REPLACEMENT_CONTROLS_REMOVED {
        command.env_remove(key);
    }
    for (key, _) in std::env::vars_os() {
        let name = key.to_string_lossy().to_ascii_uppercase();
        if REPLACEMENT_CONTROL_PREFIXES
            .iter()
            .any(|prefix| name.starts_with(prefix))
        {
            command.env_remove(&key);
        }
    }
    for (key, value) in pinned_replacement_controls() {
        command.env(key, value);
    }
}

/// This process's environment as [`without_ambient_replacement_controls`]
/// would leave it, for a boundary that takes a base rather than a `Command`.
pub(crate) fn environment_without_ambient_replacement_controls() -> Vec<(OsString, OsString)> {
    let mut base: Vec<(OsString, OsString)> = std::env::vars_os()
        .filter(|(key, _)| !is_ambient_replacement_control(key))
        .collect();
    for (key, value) in pinned_replacement_controls() {
        base.push((OsString::from(key), value));
    }
    base
}

/// Every enumerated control this process still carries, with its value, for
/// a diagnostic that says what the environment actually was.
///
/// **Two of them carry their value redacted, and that is not caution.** This
/// box's build wrapper derives `GIT_CONFIG_COUNT=1`,
/// `GIT_CONFIG_KEY_0=http.https://github.com/.extraHeader` and a
/// `GIT_CONFIG_VALUE_0` holding a GitHub personal access token, and exports all
/// three into every `cargo test` it runs (measured 2026-09-12). A diagnostic
/// that prints an enumerated control's value therefore prints that token into a
/// failing test's output, and into CI's log, the first time one of these
/// assertions fires. `GIT_CONFIG_PARAMETERS` is redacted for the same reason:
/// it is how Git propagates `-c` to its own subprocesses, so it carries the
/// same pairs. The length is kept, because "set and empty" and "set to
/// something" are different diagnoses.
pub(crate) fn ambient_replacement_controls() -> Vec<String> {
    let mut seen: Vec<String> = std::env::vars_os()
        .filter(|(key, _)| is_ambient_replacement_control(key))
        .map(|(key, value)| {
            let name = key.to_string_lossy().into_owned();
            if carries_a_credential(&key) {
                format!("{name}=<redacted, {} bytes>", value.len())
            } else {
                format!("{name}={}", value.to_string_lossy())
            }
        })
        .collect();
    seen.sort();
    seen
}

/// Whether `key`'s **value** may be a credential, and so must never reach a
/// panic message. See [`ambient_replacement_controls`] for the measurement.
fn carries_a_credential(key: &OsStr) -> bool {
    let name = key.to_string_lossy().to_ascii_uppercase();
    name.starts_with("GIT_CONFIG_VALUE_") || name == "GIT_CONFIG_PARAMETERS"
}

/// Refuse to measure replacement behaviour in a process that still carries an
/// ambient control over it, and then refuse again on the evidence.
///
/// The first half is the enumeration by name: nothing in
/// [`REPLACEMENT_CONTROLS_REMOVED`] survived, `GIT_CONFIG_COUNT` is the `0`
/// that closes the indexed pairs whatever indices are present, and the two
/// configuration files are files this process can see and that are **empty**
/// -- their *paths* are per-process, so what a child checks is what they hold
/// and never which path the parent chose.
///
/// The second half is [`assert_replacement_refs_are_live`], which is the one
/// that does not depend on the list being complete.
pub(crate) fn assert_replacement_controls_pinned(tag: &str) {
    for key in REPLACEMENT_CONTROLS_REMOVED {
        assert!(
            std::env::var_os(key).is_none(),
            "`{tag}`: `{key}` reached this process, and the reads it governs \
             would answer about the environment rather than about the code \
             under test: {:?}",
            ambient_replacement_controls()
        );
    }
    assert_eq!(
        std::env::var_os("GIT_CONFIG_COUNT").as_deref(),
        Some(OsStr::new("0")),
        "`{tag}`: `GIT_CONFIG_COUNT` must be pinned at `0`; absent, a lone \
         `GIT_CONFIG_KEY_0` still takes effect (measured on git 2.43.0): {:?}",
        ambient_replacement_controls()
    );
    assert!(
        std::env::var_os("GIT_CONFIG_NOSYSTEM").is_some(),
        "`{tag}`: `GIT_CONFIG_NOSYSTEM` must be pinned: {:?}",
        ambient_replacement_controls()
    );
    for key in ["GIT_CONFIG_GLOBAL", "GIT_CONFIG_SYSTEM"] {
        let Some(path) = std::env::var_os(key) else {
            panic!(
                "`{tag}`: `{key}` must be pinned at an empty file, or the level \
                 it names -- and every `include.path` reaching one -- decides \
                 what this process measures: {:?}",
                ambient_replacement_controls()
            );
        };
        let path = PathBuf::from(path);
        let bytes = fs::read(&path).unwrap_or_else(|error| {
            panic!(
                "`{tag}`: reading the pinned `{key}` at {}: {error}",
                path.display()
            )
        });
        assert!(
            bytes.is_empty(),
            "`{tag}`: the pinned `{key}` at {} is not empty, so it is not neutral",
            path.display()
        );
    }
    assert_replacement_refs_are_live(tag);
}

/// What a probe found when it asked whether `refs/replace/*` is honoured.
///
/// A verdict rather than a panic because two of its answers are *evidence* --
/// the grid's controlled leg wants to be told that a treatment really did
/// disable replacements -- and because the third thing that can happen, a
/// `git` command failing outright, must never be counted as either. See
/// [`replacement_liveness`].
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ReplacementLiveness {
    /// `git replace` wrote under `refs/replace/` and a child read through it.
    Live,
    /// `git replace` wrote somewhere else, so this process is not speaking
    /// about the namespace the design and the finding name.
    RefsElsewhere { found: String },
    /// The refs are where they belong and a child ignored them.
    NotHonoured { read: String },
}

/// The exit status the grid's probe helper uses for
/// [`ReplacementLiveness::RefsElsewhere`] and
/// [`ReplacementLiveness::NotHonoured`].
///
/// Distinct from `0` (live) and from the `101` a panicking `git` helper gives,
/// so a caller can tell *the treatment disabled replacements* from *the setup
/// never ran*. A grid that cannot tell them apart credits a malformed
/// configuration file as evidence of replacement suppression, which is what the
/// include-path row did with a backslash in `TMPDIR` (PR #271, round 3).
pub(crate) const REPLACEMENT_DISABLED_EXIT: i32 = 3;

/// Measure -- in **this process's** environment, over a throwaway repository
/// -- whether a Git child still honours `refs/replace/*`.
///
/// The enumeration above is a list of names, and a list of names is exactly
/// what cost this pull request three rounds: a witness sensitive to one vector
/// and blind to the next passes silently. This is the closure that does not
/// depend on the list being complete, and every replacement witness in the
/// crate runs it before it measures anything, so a control nobody enumerated
/// costs a loud failure rather than a false green.
///
/// The child is `read_only_git`'s and `HostEnvironment::from_process`'s own
/// child: both inherit this process's environment, which is why measuring this
/// process is measuring theirs.
///
/// **Every `git` here runs in this process's environment, `git init`
/// included.** That is deliberate, and it is what closes `GIT_TEMPLATE_DIR`
/// honestly rather than by exclusion: a template writes its `config` into the
/// repository at creation, so a probe whose own `init` were neutralised would
/// build a clean instrument, report `Live`, and say nothing about the
/// repositories the witness then builds. The probe pins no repository-local
/// `core.useReplaceRefs` for the same reason -- a local pin would outrank the
/// operator's `~/.gitconfig` and hide a control this exists to find.
pub(crate) fn replacement_liveness(tag: &str) -> ReplacementLiveness {
    let root = scratch(&format!("replacement-live-{tag}"));
    let repo = root.join("repo");
    create_dir(&repo);
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["config", "user.email", "tests@upstroke.local"]);
    git(&repo, &["config", "user.name", "upstroke tests"]);
    git(&repo, &["config", "core.autocrlf", "false"]);
    git(&repo, &["config", "core.eol", "lf"]);

    write_file(&repo.join("probe.txt"), b"recorded\n");
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "recorded"]);
    let recorded = git(&repo, &["rev-parse", "HEAD"]);

    write_file(&repo.join("probe.txt"), b"replacing\n");
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "replacing"]);
    let replacing = git(&repo, &["rev-parse", "HEAD"]);
    assert_ne!(recorded, replacing, "two distinct commits");

    git(&repo, &["replace", &recorded, &replacing]);
    let found = git(
        &repo,
        &["for-each-ref", "--format=%(refname)", "refs/replace/"],
    );
    let verdict = if found == format!("refs/replace/{recorded}") {
        let read = git(&repo, &["show", &format!("{recorded}:probe.txt")]);
        if read == "replacing" {
            ReplacementLiveness::Live
        } else {
            ReplacementLiveness::NotHonoured { read }
        }
    } else {
        ReplacementLiveness::RefsElsewhere { found }
    };
    let _ = fs::remove_dir_all(&root);
    verdict
}

/// [`replacement_liveness`], as the precondition a witness states before it
/// trusts anything it measures.
pub(crate) fn assert_replacement_refs_are_live(tag: &str) {
    let controls = ambient_replacement_controls();
    match replacement_liveness(tag) {
        ReplacementLiveness::Live => {}
        ReplacementLiveness::RefsElsewhere { found } => panic!(
            "`{tag}`: `git replace` wrote `{found}` rather than under \
             `refs/replace/`, so this process is not speaking about the \
             namespace the design and the finding name. `GIT_REPLACE_REF_BASE` \
             moves it, and a probe that writes and reads through the same moved \
             base would answer yes to a question it was not asked. The \
             enumerated controls this process carries are {controls:?}"
        ),
        ReplacementLiveness::NotHonoured { read } => panic!(
            "`{tag}`: this process cannot witness replacement isolation, \
             because a Git child of it does not honour `refs/replace/*` in the \
             first place -- it read `{read}` where the replacing object says \
             `replacing`. The enumerated controls it carries are {controls:?}; \
             if none of them explains this, the enumeration in \
             `REPLACEMENT_CONTROLS_REMOVED` and `REPLACEMENT_CONTROLS_PINNED` \
             is missing a mechanism, and closing it is the fix -- never \
             weakening this check"
        ),
    }
}

/// The repository configuration a fixture that measures replacement
/// behaviour pins for itself, for the reason `Fixture::new` pins
/// `core.autocrlf` and `core.eol` (§12).
///
/// Repository-local configuration outranks the system and global files and
/// everything they include, so this is what survives an operator's
/// `core.useReplaceRefs = false` in `~/.gitconfig`. It does **not** outrank
/// `-c`, `GIT_CONFIG_COUNT` or `GIT_NO_REPLACE_OBJECTS` (measured, all
/// three), which is why it is the second half of the closure and never the
/// whole of it.
///
/// The write itself goes through [`without_ambient_replacement_controls`],
/// because `GIT_CONFIG` sends `git config`'s writes to the file it names: a
/// pin made under it would land in the operator's file, succeed, and leave
/// this repository saying nothing.
pub(crate) fn pin_replacement_refs_in(repo: &Path) {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(repo)
        .args(["config", "core.useReplaceRefs", "true"]);
    without_ambient_replacement_controls(&mut command);
    let out = command.output().expect("run git");
    assert!(
        out.status.success(),
        "pinning core.useReplaceRefs in {}: {}",
        repo.display(),
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Write `include.path = <included>` into the Git configuration file `file`,
/// **using Git's own configuration writer**.
///
/// A path is not a configuration value, and spelling one with `format!` was a
/// measured defect (PR #271, round 3): on git 2.43.0 an unquoted path
/// containing `#` is truncated at the comment character, so the include
/// silently stops including anything, and one containing a backslash is read as
/// an escape -- `git config --list` over the result exits `128` with `bad
/// config line 2`. `git config --file` quotes the first and doubles the second,
/// and both then resolve. It creates the file if it is not there.
///
/// The write goes through [`without_ambient_replacement_controls`] for the
/// reason [`pin_replacement_refs_in`]'s does: `GIT_CONFIG` redirects `git
/// config`'s writes.
pub(crate) fn write_include_path(file: &Path, included: &Path) {
    let mut command = Command::new("git");
    command
        .args(["config", "--file"])
        .arg(file)
        .arg("include.path")
        .arg(included);
    without_ambient_replacement_controls(&mut command);
    let out = command.output().expect("run git");
    assert!(
        out.status.success(),
        "writing `include.path` into {}: {}",
        file.display(),
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Where a replacement witness tells its helper to do the work.
///
/// Guarded by a variable because each helper is `#[ignore]`, and a run with
/// `--include-ignored` would otherwise execute its body in a process that still
/// carries whatever the machine exported -- which is the one environment a
/// replacement witness must never be measured in.
pub(crate) const REPLACEMENT_WITNESS: &str = "UPSTROKE_PR271_REPLACEMENT_WITNESS";

/// Run this test binary again at `test`, `--exact --ignored`, with **every**
/// ambient control over `refs/replace/*` taken away from the child, and return
/// its exit status.
///
/// [`run_kill_child`] sets variables; this one takes away the ones that would
/// make a witness pass for the wrong reason. `Command` inherits the parent's
/// environment, so a suite run under an exported `GIT_NO_REPLACE_OBJECTS=1` --
/// which is what this pull request makes upstroke's own gates supply -- would
/// hand the child the protection the child exists to prove the code installs.
/// Round 2 removed that one variable and the reviewer reached the same silent
/// pass through `core.useReplaceRefs=false` instead, so what is removed here is
/// the enumeration [`without_ambient_replacement_controls`] carries, and the
/// child measures what is left before it trusts it.
///
/// **Every replacement witness in the crate goes through here** (PR #271,
/// round 4), in `src/workspace_manager/tests.rs`, `src/gates.rs` and
/// `src/engine/tests.rs` alike. Rounds 1, 2 and 3 each shipped one witness that
/// could pass without its fix, and each repair reached only the witness a
/// reviewer had named; there is now one door and no witness beside it.
pub(crate) fn run_replacement_witness_child(test: &str) -> std::process::ExitStatus {
    let mut command = Command::new(std::env::current_exe().expect("this test binary"));
    command
        .args(["--exact", test, "--ignored", "--nocapture"])
        .env(REPLACEMENT_WITNESS, "1");
    without_ambient_replacement_controls(&mut command);
    command.status().expect("spawn the witness child")
}

/// A `git` child a test can kill at a chosen moment.
///
/// The residue sampler's child, and deliberately **blind to what it is
/// running**: no argv reaches [`Self::kill`], so a per-command count taken
/// over these cannot be defeated inside this type. It is the same shape
/// `mod tests`'s `SampledChild` uses, which stays there because the
/// four-command sampler stays there; this one exists because the
/// two-command sampler of `T-ATTEMPT` lives in a module that cannot name
/// [`Command`].
pub(crate) struct KillableGitChild {
    child: std::process::Child,
    /// The origin of every clock below, read before `Command::spawn` is
    /// called, so the child's run — the exec the parent never sees, the
    /// pick, the exit — begins after it and ends before any reading below
    /// that is taken once its exit was established; [`time_git`]'s probe is
    /// timed from the same place. Until 2026-09-10 it was read once the
    /// spawn had returned (the ultra review of `d1fef26d`, finding 1): a
    /// parent paused 20 ms between the two let a 1.02 ms pick complete
    /// before the origin existed, the poll found it gone at 4.5 µs, the
    /// samplers fed that back, and every later rung was aimed at 22–178 µs,
    /// under the pick's first write.
    origin: std::time::Instant,
    /// The clock once `Command::spawn` had returned: the spawn's own
    /// latency, which every clock below includes, and during which the
    /// child may already be running, or done. The two samplers of this
    /// fixture aim from the origin and subtract nothing; the `T-ATTEMPT`
    /// sampler sets its rungs as delays after the spawn's return and reads
    /// its clocks from there ([`Self::spawned`]).
    spawned: std::time::Duration,
    /// The clock once a kill attempt at this child had returned, or `None`
    /// if none was ever made. Written only by [`Self::kill`], read after
    /// [`std::process::Child::kill`] returned, whatever it returned. It
    /// orders the attempt against the child's clock and nothing more: it
    /// does not say the signal was sent ([`Self::kill_error`] does), and it
    /// does not say the child had exited ([`Self::reaped`] does).
    fired: Option<std::time::Duration>,
    /// What [`std::process::Child::kill`] returned when it did not return
    /// `Ok`: nothing was sent, and the child ran on.
    kill_error: Option<std::io::Error>,
    /// The clock once [`Self::wait`] had returned the child's status, or
    /// `None` until it has.
    reaped: Option<std::time::Duration>,
    /// How long after the leader's reap its whole process group was gone
    /// ([`await_group_end`]), or `None` until [`Self::wait`] has run.
    group_ended: Option<std::time::Duration>,
}

/// The `git` a sampled child runs, so that a kill of the child is a kill of
/// git.
///
/// On Unix that is `git` on the `PATH`. On Windows `git` on the `PATH` is Git
/// for Windows' `cmd\git.exe`, a 46 KB launcher that starts the real
/// `mingw64\bin\git.exe` as its own child and waits for it, so a
/// `Child::kill` there ends the launcher and the pick runs on to completion:
/// the winguest lane at `56ea88c9` recorded 30 kills, ten of which found no
/// write and twenty a finished pick, none an interruption
/// (`PR249-KILL-SAMPLER-WINDOWS-WRAPPER`). So the sampled child is the real
/// binary, found through `git --exec-path` (`<prefix>/mingw64/libexec/git-core`)
/// as `<prefix>/mingw64/bin/git.exe`, whose DLLs sit beside it. Resolved
/// once per process; `git` when that layout is absent.
pub(crate) fn sampled_git() -> &'static Path {
    static RESOLVED: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    RESOLVED.get_or_init(|| {
        let fallback = PathBuf::from("git");
        if !cfg!(windows) {
            return fallback;
        }
        let Ok(output) = Command::new("git")
            .arg("--exec-path")
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
        else {
            return fallback;
        };
        if !output.status.success() {
            return fallback;
        }
        let exec_path = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
        match exec_path
            .parent()
            .and_then(Path::parent)
            .map(|prefix| prefix.join("bin").join("git.exe"))
        {
            Some(real) if real.is_file() => real,
            _ => fallback,
        }
    })
}

impl KillableGitChild {
    /// Spawn `git -C cwd <args>` with its streams discarded; [`sampled_git`]
    /// says which `git`.
    pub(crate) fn spawn(cwd: &Path, args: &[String]) -> Self {
        let origin = std::time::Instant::now();
        let mut command = Command::new(sampled_git());
        command
            .arg("-C")
            .arg(cwd)
            .args(["-c", "core.fsmonitor=false"])
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt as _;
            command.process_group(0);
        }
        let child = command.spawn().expect("spawn the sampled git child");
        let spawned = origin.elapsed();
        Self {
            child,
            origin,
            spawned,
            fired: None,
            kill_error: None,
            reaped: None,
            group_ended: None,
        }
    }

    /// Send the kill, recording the clock once the attempt has returned and
    /// what it returned.
    ///
    /// Neither says the child has exited. `Child::kill` returns `Ok` once
    /// the signal is sent, and also for a child that has already exited
    /// (std documents that; on Windows `TerminateProcess` answers
    /// `ERROR_ACCESS_DENIED` for a process that has ended or is ending, and
    /// std reports that as `Ok`), and it returns `Err` when nothing was
    /// sent at all and the child runs on. So the clock recorded here
    /// orders the attempt and bounds nothing: a child that was still
    /// running at a failed attempt, or that exited between the poll that
    /// found it running and the system call, ends with a completion's
    /// status, and what the samplers feed back for it is [`Self::reaped`],
    /// read once [`Self::wait`] has returned that status.
    /// Until 2026-09-10 the samplers fed such a child back as this clock:
    /// first as the clock read *before* the call (the ultra review of
    /// `2d3fa9d1`, finding 1: a 20 ms pause between the read and the call
    /// fed a completed 1.09 ms pick back as 121 µs, below the pick and
    /// clamped to [`KillBudget::FLOOR`], and seven kills of children that
    /// had not begun were the evidence of a batch), then as the clock read
    /// after it (the ultra review of `8441c5fe`, finding 1: an attempt made
    /// to fail on the first kill fed a child still running at 117 µs back
    /// as 117 µs, and the batch collapsed the same way).
    pub(crate) fn kill(&mut self) {
        let outcome = self.kill_group();
        self.fired = Some(self.origin.elapsed());
        self.kill_error = outcome.err();
    }

    /// The kill itself: [`kill_process_group`] — the child's whole process
    /// group on Unix, the direct child elsewhere.
    fn kill_group(&mut self) -> std::io::Result<()> {
        kill_process_group(&mut self.child)
    }

    /// Whether the child has exited on its own, and when the parent saw it.
    ///
    /// The duration is the clock at the poll that found the child gone, from
    /// the origin read before its spawn: the parent's observation of the
    /// exit, not the child's own time, which no platform hands a parent —
    /// `wait4` carries a child's CPU times and no wall-clock exit, and
    /// Windows' `GetProcessTimes`, which does, is not bound here. The poll
    /// is the next one when the parent holds a core — at once while
    /// [`Self::run_until`] spins, after a sleep of a millisecond before then
    /// — and the one after the scheduler's wake-up when it does not, and
    /// [`KillBudget`] follows the number as the poll read it.
    ///
    /// `None` while it is still running.
    pub(crate) fn exited(&mut self) -> Option<std::time::Duration> {
        match self.child.try_wait() {
            Ok(Some(_)) => Some(self.origin.elapsed()),
            _ => None,
        }
    }

    /// Reap it, recording the clock once its status is in. The wait status
    /// is the only thing a kill changes.
    pub(crate) fn wait(&mut self) -> std::process::ExitStatus {
        let status = self.child.wait().expect("reap the sampled git child");
        self.reaped = Some(self.origin.elapsed());
        let pgid = i32::try_from(self.child.id()).expect("a pid fits in i32");
        self.group_ended = Some(await_group_end(pgid).unwrap_or_else(|outlived| {
            panic!("the sampled git child's group outlived its leader: {outlived}")
        }));
        status
    }

    /// Leave the child running until `aim` after its spawn or until it
    /// exits on its own, and say which: `Some` with how long it ran, as the
    /// parent saw it, if it exited first; `None` if it was still running at
    /// the aim — and then **the kill has fired**, sent by the poll that found
    /// it running there, and [`Self::fired`] says when.
    ///
    /// Polled, never slept through. `sleep(aim)` then `kill` reports nothing
    /// about the child and wakes when the scheduler pleases, so on a loaded
    /// host the kill is late by the wake-up and the sampler cannot tell a
    /// child it missed from one it never aimed inside. Here [`Self::exited`]
    /// is asked once a millisecond while the aim is far and continuously
    /// once it is within [`Self::SPIN_WITHIN`], and the poll that reaches
    /// the aim with the child still running sends the kill itself, with no
    /// return to the caller between the two. What remains is the
    /// platform's: between that poll's `try_wait` and the kill's own system
    /// call the parent can be descheduled, and a child that exits in that
    /// window is missed by the kill and ends on its own terms — its status
    /// is a completion, not the kill's signature, and the samplers feed it
    /// back like any completion, as [`Self::reaped`], the clock once
    /// [`Self::wait`] has returned that status.
    pub(crate) fn run_until(&mut self, aim: std::time::Duration) -> Option<std::time::Duration> {
        let deadline = self.origin + aim;
        loop {
            if let Some(ran) = self.exited() {
                return Some(ran);
            }
            let now = std::time::Instant::now();
            if now >= deadline {
                self.kill();
                return None;
            }
            if deadline - now > Self::SPIN_WITHIN {
                std::thread::sleep(std::time::Duration::from_millis(1));
            } else {
                std::thread::yield_now();
            }
        }
    }

    /// How close to its aim [`Self::run_until`] stops sleeping and polls
    /// continuously: wider than a loaded host's wake-up from a
    /// one-millisecond sleep.
    const SPIN_WITHIN: std::time::Duration = std::time::Duration::from_millis(4);

    /// The clock once `Command::spawn` had returned, from the origin read
    /// before it: the spawn's own latency, which [`Self::fired`],
    /// [`Self::exited`] and [`Self::reaped`] all include. The two samplers
    /// of this fixture aim from the origin, feed back from it and subtract
    /// nothing. The `T-ATTEMPT` sampler (`engine::topology::attempt::tests`)
    /// sets each rung as a delay after the spawn's return and asserts that
    /// its kill fired no sooner than the rung, so it subtracts this from
    /// `fired`, and from the clock `exited` read, to bring both to the
    /// reference its rungs and its retry budget have. Read from the origin,
    /// its `fired` counted the spawn's latency toward the rung: the ultra
    /// review of `ec87d6ed` (finding 1) paused the spawn one second after
    /// the origin and removed that sampler's deadline loop, and every kill,
    /// fired the instant the spawn returned, read 1.0002 s against rungs of
    /// 0.66–7.5 ms and passed.
    pub(crate) fn spawned(&self) -> std::time::Duration {
        self.spawned
    }

    /// The clock once a kill attempt at this child had returned, if one was
    /// ever made — whatever it returned. It orders the attempt against the
    /// child's clock and says nothing else: not that the signal was sent
    /// ([`Self::kill_error`]), not that the child had exited
    /// ([`Self::reaped`]).
    pub(crate) fn fired(&self) -> Option<std::time::Duration> {
        self.fired
    }

    /// What the kill attempt returned when it was not `Ok`: nothing was
    /// sent, and the child ran on to an end of its own.
    pub(crate) fn kill_error(&self) -> Option<&std::io::Error> {
        self.kill_error.as_ref()
    }

    /// The clock once [`Self::wait`] had returned the child's status, if it
    /// has, from the origin read before its spawn — whichever way the child
    /// ended: killed, missed by the kill, or never reached by a failed
    /// attempt. For a child that ended with a completion's status this is
    /// what the samplers feed back.
    pub(crate) fn reaped(&self) -> Option<std::time::Duration> {
        self.reaped
    }

    /// How long after the leader's reap its whole process group was gone
    /// ([`await_group_end`]), once [`Self::wait`] has run.
    pub(crate) fn group_ended(&self) -> Option<std::time::Duration> {
        self.group_ended
    }
}

/// How long a killed process group is given to be gone after its leader's
/// reap before [`await_group_end`] gives up.
pub(crate) const GROUP_END_BOUND: std::time::Duration = std::time::Duration::from_secs(10);

/// A killed process group that still had a member when [`GROUP_END_BOUND`]
/// ran out.
#[derive(Debug)]
pub(crate) struct GroupOutlivedItsLeader {
    pgid: i32,
    waited: std::time::Duration,
}

impl std::fmt::Display for GroupOutlivedItsLeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "process group {} still had a member {:?} after its leader was reaped (the bound is {:?})",
            self.pgid, self.waited, GROUP_END_BOUND
        )
    }
}

/// `SIGKILL` to the child's whole process group on Unix — `git worktree add`
/// runs its `reset --hard` as a child of its own, and a kill of the leader
/// alone leaves that child populating the worktree after the sample is
/// taken — and to the direct child elsewhere. Both samplers' children are
/// killed through here since PR10's round 5; until then only
/// [`KillableGitChild`]'s were (round 2), and PR5's own sampler killed its
/// leader alone (`PR136`'s first fingerprint, met on CI's macOS leg at
/// `869e336a`).
#[cfg(unix)]
pub(crate) fn kill_process_group(child: &mut std::process::Child) -> std::io::Result<()> {
    let pid =
        i32::try_from(child.id()).map_err(|_| std::io::Error::from_raw_os_error(libc::EINVAL))?;
    // SAFETY: the spawn put the child in a new process group whose id is
    // its pid; a negative pid targets that group only, and the call hands
    // over no memory.
    if unsafe { libc::kill(-pid, libc::SIGKILL) } == 0 {
        return Ok(());
    }
    Err(std::io::Error::last_os_error())
}

#[cfg(not(unix))]
pub(crate) fn kill_process_group(child: &mut std::process::Child) -> std::io::Result<()> {
    child.kill()
}

/// Wait until no process of the group `pgid` exists, and say how long that
/// took after the leader's reap: the proof that no writer the sampled child
/// started is alive, which the leader's reap alone is not. A member
/// `SIGKILL` reached is dead, but its last system call can complete after
/// the leader's reap and after `remove_dir_all` listed a directory —
/// `DirectoryNotEmpty` at the forced removal, seen on CI's macOS leg at
/// `869e336a` — and a member reparented to this process stays a zombie of
/// the group until reaped, so each poll reaps what it can
/// (`waitpid(-pgid, WNOHANG)`) and then asks the group (`kill(-pgid, 0)`,
/// `ESRCH` once it is empty). Bounded by [`GROUP_END_BOUND`]. `cfg(unix)`:
/// only there is the sampled child a group; elsewhere the kill is the direct
/// child's and its reap the whole wait, as before.
#[cfg(unix)]
pub(crate) fn await_group_end(pgid: i32) -> Result<std::time::Duration, GroupOutlivedItsLeader> {
    let started = std::time::Instant::now();
    loop {
        // SAFETY: a negative pid waits for any child of this process in that
        // group, `WNOHANG` never blocks, and a null status pointer is
        // permitted; nothing else is handed over.
        while unsafe { libc::waitpid(-pgid, std::ptr::null_mut(), libc::WNOHANG) } > 0 {}
        // SAFETY: signal 0 delivers nothing; the call only asks whether a
        // process of the group exists, and hands over no memory.
        if unsafe { libc::kill(-pgid, 0) } == -1
            && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
        {
            return Ok(started.elapsed());
        }
        let waited = started.elapsed();
        if waited >= GROUP_END_BOUND {
            return Err(GroupOutlivedItsLeader { pgid, waited });
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

#[cfg(not(unix))]
pub(crate) fn await_group_end(_pgid: i32) -> Result<std::time::Duration, GroupOutlivedItsLeader> {
    Ok(std::time::Duration::ZERO)
}

/// Whether `status` is the death `std::process::abort()` produces.
///
/// **Not `!status.success()`.** A kill child that reaches its own
/// `unreachable!` panics, and a panic also fails to succeed — so a parent
/// that accepted any unsuccessful exit would read "the injection stopped
/// killing" as "the injection killed", and would then go on to inspect a
/// directory the panicking child's `Drop` had already deleted. Measured:
/// exactly that, on a kill armed at a site the child never reached.
///
/// **A value per platform, as [`died_by_kill`] is.** On Unix it is `SIGABRT`,
/// which no Rust panic raises and no exit carries. On Windows it is the exit
/// status `std::process::abort()` ends a process with there, `0xC0000409`
/// (`STATUS_STACK_BUFFER_OVERRUN`, the status of a fast fail). Measured on the
/// Windows guest, through this test binary's own abort, beside exits of 1, 2
/// and 134 and a panic, each of which ended with its own code (#292's review
/// round 5). Until then this arm was the negation of the panic's 101, which
/// read every one of those exits as an abort.
/// `the_abort_oracle_separates_an_abort_from_an_exit_of_one` applies this
/// predicate to a real abort on every run, so a Windows whose abort ended with
/// another status would fail there.
pub(crate) fn died_by_abort(status: &std::process::ExitStatus) -> bool {
    #[cfg(unix)]
    {
        std::os::unix::process::ExitStatusExt::signal(status) == Some(libc::SIGABRT)
    }
    #[cfg(windows)]
    {
        /// `STATUS_STACK_BUFFER_OVERRUN`, the status a fast-failed process
        /// exits with, as `ExitStatus::code` reads it.
        const FAST_FAIL: i32 = 0xC000_0409_u32 as i32;
        status.code() == Some(FAST_FAIL)
    }
}

/// Whether `status` carries this platform's signature of a
/// [`std::process::Child::kill`].
///
/// A **value** per platform, not `!status.success()`: a command that merely
/// failed also fails to succeed, and reading that as a kill is how a
/// kill-count keeps counting after the kill is gone.
pub(crate) fn died_by_kill(status: &std::process::ExitStatus) -> bool {
    // `Child::kill` sends `SIGKILL`, and no exit a child reaches on its own
    // carries a signal at all.
    #[cfg(unix)]
    {
        std::os::unix::process::ExitStatusExt::signal(status) == Some(libc::SIGKILL)
    }
    // `Child::kill` is `TerminateProcess(handle, 1)`; the sampler's probe
    // asserts the same command exits 0 when nothing kills it, so 1 is not
    // an end these commands reach by themselves.
    #[cfg(windows)]
    {
        status.code() == Some(1)
    }
}

/// Time one uninterrupted run of `git -C cwd <args>`.
///
/// The kill ladder is fractions of this duration, which is the only
/// variance a replay can pin — see `mod tests`'s `measure_budget` for the
/// argument, and for why the measurement runs in a **probe slot of its
/// own** rather than in the worktree the samples will kill in.
pub(crate) fn time_git(cwd: &Path, args: &[String]) -> std::time::Duration {
    let start = std::time::Instant::now();
    let output = git_out(cwd, &args.iter().map(String::as_str).collect::<Vec<_>>());
    let elapsed = start.elapsed();
    assert!(
        output.status.success(),
        "the probe must really run: git {args:?} in {}: {}",
        cwd.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    elapsed
}

/// A kill sampler's budget: how long an uninterrupted run of the sampled
/// command takes on this machine, now.
///
/// A sampler aims its kills at fixed fractions of one duration, and every
/// sampler in this tree that took that duration from a single measured run
/// has been red on a hosted macOS runner with every kill landing after its
/// child had already finished (`PR7-SAMPLER-SCHEDULES-FROM-A-COLD-PROBE`,
/// `PR80-MACOS-WORKSPACE-SAMPLER-COLD-PROBE-RECURRENCE`,
/// `RECOVER-CHERRY-PICK-SAMPLER-COLD-PROBE`,
/// `G4B-O10-REPAIR-MATERIALIZE-SAMPLER-MACOS-KILL-FLOOR`). So the budget is
/// never one number. It starts as the median of a probe's runs after a
/// discarded warm-up, and it follows the sampled children themselves: a
/// child that finished before its kill has measured the command under the
/// sampler's own conditions, at that moment, and the next kill is aimed
/// inside what it took. An inflated probe is corrected by the first child
/// that outruns it; a host that drifts is tracked rung by rung.
///
/// It follows in both directions and is not capped at the probe: a host
/// that slows after the probe needs rungs past it to reach the pick's
/// writes, and a completion the parent saw late — woken after the exit, or
/// a child the kill did not stop, fed back as the clock once its `wait` had
/// returned — is followed as it was read, never as a ceiling. What the
/// ladder follows is the median of the last [`KillBudget::RECENT`] such
/// completions, over however many exist: the first sets the budget alone,
/// of two the longer is taken, and from three on one late observation
/// among three is outvoted by the other two while two are not — so a late
/// first observation holds until two shorter completions follow it, and
/// three late ones hold until two do ([`KillBudget::current`]).
pub(crate) struct KillBudget {
    probe: std::time::Duration,
    /// The clock at which the parent established the exit of each child
    /// that ended with a completion's status, from the origin read before
    /// its spawn, oldest first.
    completed: Vec<std::time::Duration>,
}

impl KillBudget {
    /// No budget below this: a measurement that small is the clock's, not
    /// the command's.
    pub(crate) const FLOOR: std::time::Duration = std::time::Duration::from_micros(200);

    /// How many of the most recent completions the budget follows: three,
    /// so that once three exist one late observation among them is outvoted
    /// by the other two, and so that the first completion replaces the probe
    /// at once. Until three exist nothing is outvoted: one completion sets
    /// the ladder alone, and of two the longer is taken.
    pub(crate) const RECENT: usize = 3;

    /// From a probe's uninterrupted runs, in order. The first is the
    /// warm-up and is discarded — the first run in a fresh worktree pays
    /// for cold caches and, on Windows, an antivirus pass — and the budget
    /// is the median of the rest, because the failure mode of one
    /// measurement is one outlier, which a median discards and a mean
    /// keeps.
    pub(crate) fn probed(runs: &[std::time::Duration]) -> Self {
        assert!(
            runs.len() > 1,
            "a budget needs a warm-up run and at least one after it: {runs:?}"
        );
        let after_warm_up = runs.get(1..).unwrap_or_default();
        Self {
            probe: median(after_warm_up)
                .unwrap_or(Self::FLOOR)
                .max(Self::FLOOR),
            completed: Vec::new(),
        }
    }

    /// What the probe measured, for the record a red run carries.
    pub(crate) fn probe(&self) -> std::time::Duration {
        self.probe
    }

    /// The budget now: the median of the last [`Self::RECENT`] children
    /// that finished before their kill, over however many have — one alone,
    /// the longer of two, the middle of three — or the probe's while none
    /// has. [`median`] takes the upper of an even count, so a late first
    /// completion is not displaced by one shorter one; two are needed.
    pub(crate) fn current(&self) -> std::time::Duration {
        let recent: Vec<std::time::Duration> = self
            .completed
            .iter()
            .rev()
            .take(Self::RECENT)
            .copied()
            .collect();
        median(&recent).unwrap_or(self.probe).max(Self::FLOOR)
    }

    /// Where the kill of rung `rung` (from zero) of `rungs` is aimed: the
    /// ladder's fraction `(rung + 1) / (rungs + 1)` of the current budget,
    /// so the rungs stay spread through the command and never reach its
    /// measured end.
    pub(crate) fn aim(&self, rung: u32, rungs: u32) -> std::time::Duration {
        self.current()
            .mul_f64(f64::from(rung + 1) / f64::from(rungs + 1))
    }

    /// A child ended with a completion's status: `clock` is the clock at
    /// which the parent established its exit, from the origin read before
    /// its spawn — the poll that found it gone, or, when the kill attempt
    /// did not stop it, the return of the `wait` that reaped it. The two
    /// differ in what the parent did, not in where the exit fell against
    /// the aim, which the parent cannot see: on the first, no poll reached
    /// the aim with the child running and no kill was sent — a parent
    /// descheduled across the aim and the exit alike finds the child gone
    /// at its next poll, whichever of the two came first; on the second, a
    /// poll reached the aim with the child still running, and the kill sent
    /// from there missed it, or was not sent at all
    /// ([`KillableGitChild::kill_error`]). The budget follows what the child
    /// took either way, as the parent read it.
    pub(crate) fn completed(&mut self, clock: std::time::Duration) {
        self.completed.push(clock);
    }

    /// How many children have moved the budget.
    pub(crate) fn completions(&self) -> usize {
        self.completed.len()
    }
}

/// The median of `durations`; `None` of none.
pub(crate) fn median(durations: &[std::time::Duration]) -> Option<std::time::Duration> {
    let mut sorted = durations.to_vec();
    sorted.sort_unstable();
    sorted.get(sorted.len() / 2).copied()
}
