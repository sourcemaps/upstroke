// Allowlist placement: the funnel section of `effects/allowlist.toml`.
#![allow(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros
)]

use super::*;

// -----------------------------------------------------------------------
// R18: repo_key and the execution root
// -----------------------------------------------------------------------

/// The digest is pinned against values computed **outside this program**,
/// from the packet's formula, so the function is never its own oracle.
///
/// `decisions.workspace_candidates.execution_root`: "repo_key v1 =
/// hex16(sha256('upstroke-repo-key-v1' NUL canonical common git dir bytes))".
///
/// Independently computed with:
/// `python3 -c "import hashlib;
///  print(hashlib.sha256(b'upstroke-repo-key-v1\x00' + P).hexdigest()[:16])"`
#[test]
fn the_repo_key_is_the_packets_digest_and_not_a_neighbouring_one() {
    assert_eq!(
        repo_key_v1(Path::new("/srv/upstroke/.git")),
        "fb0df4eb6c4a32c7",
        "the digest of the packet's own formula"
    );
    assert_eq!(
        repo_key_v1(Path::new("/srv/upstroke/.git/")),
        "8d01d05e9d96ec4b",
        "a trailing separator is different bytes and must be a different key"
    );
    assert_eq!(
        repo_key_v1(Path::new(r"C:\repos\upstroke\.git")),
        "a6a151c73796e709",
        "a Windows-shaped path hashes its own bytes on either platform"
    );

    // The two neighbouring formulas a transcription slip produces: the
    // domain prefix dropped, and the NUL separator dropped. Neither may be
    // what the function computes.
    assert_ne!(
        repo_key_v1(Path::new("/srv/upstroke/.git")),
        "85185d58540dc79c",
        "the NUL separator is part of the formula"
    );
    assert_ne!(
        repo_key_v1(Path::new("/srv/upstroke/.git")),
        "c2ed95c96b45a16d",
        "the domain prefix is `upstroke-repo-key-v1`"
    );

    // hex16 is sixteen hexadecimal characters, and it is a prefix of the
    // full digest rather than a fold of it.
    let key = repo_key_v1(Path::new("/srv/upstroke/.git"));
    assert_eq!(key.len(), 16);
    assert!(key.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert!("fb0df4eb6c4a32c75b6a8adf8ad24da6752f85b84a984a0ac8b89163527c849d".starts_with(&key));
}

#[cfg(unix)]
#[test]
fn the_repo_key_hashes_path_bytes_a_string_cannot_carry() {
    use std::os::unix::ffi::OsStringExt as _;
    let path = PathBuf::from(OsString::from_vec(b"/tmp/\xff\xfe/.git".to_vec()));
    assert_eq!(
        repo_key_v1(&path),
        "d43422329d1de7f3",
        "a repository path is bytes on Unix, and the key is over those bytes"
    );
}

/// The property no digest constant can pin: the key is taken over the
/// *common* git dir, so two linked worktrees of one repository agree and
/// two repositories differ.
#[test]
fn the_repo_key_is_the_repositorys_and_not_the_worktrees() {
    let fixture = Fixture::created("repokey-common");
    let linked = fixture.root.join("linked");
    git(
        &fixture.base,
        &[
            "worktree",
            "add",
            "-q",
            "--detach",
            &linked.to_string_lossy(),
            &fixture.head,
        ],
    );
    let from_linked = WorkspaceManager::derive(
        &linked,
        &fixture.private,
        "01KZSWEEP00000000000000002",
        "inc-1",
    )
    .expect("derive");
    assert_eq!(
        from_linked.repo_key(),
        fixture.manager.repo_key(),
        "a linked worktree of the same repository has the same common git dir"
    );

    let other = Fixture::new("repokey-other");
    assert_ne!(
        other.manager.repo_key(),
        fixture.manager.repo_key(),
        "a different repository has a different common git dir"
    );

    git(
        &fixture.base,
        &["worktree", "remove", "--force", &linked.to_string_lossy()],
    );
}

#[test]
fn the_execution_root_is_the_path_the_packet_names() {
    let fixture = Fixture::new("root-shape");
    // `strip_verbatim` on the canonical root, not the raw canonical root:
    // on Windows `fs::canonicalize` answers `\\?\C:\...`, which Git cannot
    // open, so the recorded root is the Win32 spelling of the same
    // directory. The expected value is built from the packet's own formula
    // — `<private_root>/workspaces/<repo_key>/<run_id>` — rather than from
    // the manager.
    let expected = strip_verbatim(
        fixture
            .private
            .canonicalize()
            .expect("canonical private root"),
    )
    .join("workspaces")
    .join(fixture.manager.repo_key())
    .join(super::fixture::RUN_ID);
    assert_eq!(fixture.manager.execution_root(), expected.as_path());
    assert!(
        !fixture
            .manager
            .execution_root()
            .to_string_lossy()
            .contains("\\\\?\\"),
        "the recorded root is a path Git can open"
    );
}

#[test]
fn the_execution_root_is_pruned_only_when_it_is_empty() {
    let fixture = Fixture::created("root-prune");
    let slot = fixture.add_task(&mut NoHooks, "alpha", 1);
    assert!(
        !fixture
            .manager
            .remove_execution_root(&mut NoHooks)
            .expect("attempt the removal"),
        "R18 is pruned by finalization *when empty*; a live worktree is not empty"
    );
    assert!(fixture.manager.execution_root().is_dir());

    fixture
        .manager
        .remove_worktree(&mut NoHooks, &slot)
        .expect("forced removal");
    fixture
        .manager
        .remove_intent(&mut NoHooks, &slot)
        .expect("intent removal");
    assert!(
        fixture
            .manager
            .remove_execution_root(&mut NoHooks)
            .expect("remove the root"),
        "an empty root is pruned"
    );
    assert!(!fixture.manager.execution_root().exists());
}

/// Every entry under `top`, by path relative to it: its kind, its permission
/// bits, and a file's bytes or a link's target. What "untouched" is measured by.
fn root_state(top: &Path) -> Vec<(PathBuf, String)> {
    fn mode(metadata: &fs::Metadata) -> u32 {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            metadata.permissions().mode()
        }
        #[cfg(not(unix))]
        {
            u32::from(metadata.permissions().readonly())
        }
    }
    let mut state = Vec::new();
    let mut pending = vec![top.to_path_buf()];
    while let Some(path) = pending.pop() {
        let metadata = fs::symlink_metadata(&path)
            .unwrap_or_else(|error| panic!("the state of {}: {error}", path.display()));
        let relative = path.strip_prefix(top).expect("under the top").to_path_buf();
        let kind = if metadata.file_type().is_symlink() {
            format!(
                "link to {}",
                fs::read_link(&path).expect("the link").display()
            )
        } else if metadata.is_dir() {
            for entry in fs::read_dir(&path).expect("list the directory") {
                pending.push(entry.expect("an entry").path());
            }
            format!("directory {:o}", mode(&metadata))
        } else {
            format!(
                "file {:o} {:?}",
                mode(&metadata),
                fs::read(&path).expect("the file's bytes")
            )
        };
        state.push((relative, kind));
    }
    state.sort();
    state
}

const SCAFFOLDING: [&str; 5] = ["intents", "tasks", "merge", "snapshots", "hooks-none"];

/// `FreshExecutionRoot`, PR12-INV7-PREPUBLICATION-EXECUTION-ROOT-ORPHAN: a fresh
/// run claims its root with one exclusive create of the leaf, inside the same
/// `Worktree.CreateExecutionRoot` funnel. The claim makes the root and its
/// scaffolding; a second claim of the same path is refused as unclaimed and
/// leaves it exactly as it was; and the idempotent form every other caller uses
/// still reuses it.
#[test]
fn a_fresh_execution_root_is_claimed_exclusively_and_a_claimed_one_is_never_claimed_twice() {
    let fixture = Fixture::new("fresh-root-claim");
    let root = fixture.manager.execution_root().to_path_buf();
    assert!(
        fs::symlink_metadata(&root).is_err(),
        "the premise: no root exists yet"
    );
    FreshExecutionRoot(&fixture.manager)
        .create_execution_root(&mut NoHooks)
        .expect("a fresh claim of an empty path");
    for directory in SCAFFOLDING {
        assert!(root.join(directory).is_dir(), "the claim makes {directory}");
    }

    let before = root_state(&root);
    match FreshExecutionRoot(&fixture.manager).create_execution_root(&mut NoHooks) {
        Err(FreshRootFailure::Unclaimed(UpstrokeError::Filesystem { path, source, .. })) => {
            assert_eq!(path, root, "the refusal names the root");
            assert_eq!(source.kind(), std::io::ErrorKind::AlreadyExists, "{source}");
        }
        other => panic!("a root already there is refused, not adopted: {other:?}"),
    }
    assert_eq!(
        root_state(&root),
        before,
        "the refused claim changed the root"
    );

    fixture
        .manager
        .create_execution_root(&mut NoHooks)
        .expect("the idempotent form still reuses the root");
}

/// The claim boundary: a root another actor puts at the path after every check
/// a caller can make and before the leaf is created — here at the funnel's
/// `Before` phase, the last point before the primitive — is refused as
/// unclaimed, and nothing in it changes: not the five empty scaffolding
/// directories that look exactly like a run's own, not its modes, not a file
/// beside them. And a link at the leaf is refused without being followed.
#[test]
fn a_root_that_appears_at_the_claim_boundary_is_refused_and_left_as_it_was() {
    struct PlantAtBefore {
        root: PathBuf,
        with_a_file: bool,
        planted: Option<Vec<(PathBuf, String)>>,
    }

    impl EffectHooks for PlantAtBefore {
        fn phase(&mut self, site: EffectSiteId, phase: HookPhase) -> Injection {
            if site == EffectSiteId::Worktree(WorktreeSite::CreateExecutionRoot)
                && phase == HookPhase::Before
                && self.planted.is_none()
            {
                for directory in SCAFFOLDING {
                    fs::create_dir_all(self.root.join(directory)).expect("another actor's root");
                }
                if self.with_a_file {
                    fs::write(self.root.join("theirs.txt"), b"another actor's bytes")
                        .expect("another actor's file");
                }
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt as _;
                    fs::set_permissions(&self.root, fs::Permissions::from_mode(0o750))
                        .expect("its own mode");
                }
                self.planted = Some(root_state(&self.root));
            }
            Injection::Proceed
        }

        fn refusal_cause(&self) -> Option<String> {
            None
        }
    }

    for (tag, with_a_file) in [
        ("fresh-root-foreign", false),
        ("fresh-root-foreign-file", true),
    ] {
        let fixture = Fixture::new(tag);
        let root = fixture.manager.execution_root().to_path_buf();
        let mut hooks = PlantAtBefore {
            root: root.clone(),
            with_a_file,
            planted: None,
        };
        let failure = FreshExecutionRoot(&fixture.manager)
            .create_execution_root(&mut hooks)
            .expect_err("a root already at the path is refused");
        assert!(
            matches!(failure, FreshRootFailure::Unclaimed(_)),
            "{tag}: not this caller's: {failure:?}"
        );
        assert_eq!(
            Some(root_state(&root)),
            hooks.planted,
            "{tag}: the other actor's root changed"
        );
    }

    #[cfg(unix)]
    {
        let fixture = Fixture::new("fresh-root-link");
        let root = fixture.manager.execution_root().to_path_buf();
        let target = fixture.root.join("link-target");
        fs::create_dir_all(&target).expect("the link's target");
        fs::create_dir_all(root.parent().expect("a parent")).expect("the namespace");
        std::os::unix::fs::symlink(&target, &root).expect("a link at the leaf");
        let failure = FreshExecutionRoot(&fixture.manager)
            .create_execution_root(&mut NoHooks)
            .expect_err("a link at the leaf is refused");
        assert!(
            matches!(failure, FreshRootFailure::Unclaimed(_)),
            "not this caller's: {failure:?}"
        );
        assert_eq!(
            fs::read_link(&root).expect("the link is still there"),
            target
        );
        assert_eq!(
            fs::read_dir(&target).expect("the target").count(),
            0,
            "nothing was made through the link"
        );
    }
}

/// The claim carried: a failure after the exclusive create — the after-phase
/// hook here, standing for a scaffolding directory or the durability barrier —
/// answers `Claimed`, so its caller knows the root, whole or in part, is its
/// own. A failure before it answers `Unclaimed` and leaves no root.
#[test]
fn a_failure_after_the_claim_leaves_the_root_claimed_and_one_before_it_leaves_none() {
    struct ErrorAt(HookPhase);

    impl EffectHooks for ErrorAt {
        fn phase(&mut self, site: EffectSiteId, phase: HookPhase) -> Injection {
            if site == EffectSiteId::Worktree(WorktreeSite::CreateExecutionRoot) && phase == self.0
            {
                Injection::Error
            } else {
                Injection::Proceed
            }
        }

        fn refusal_cause(&self) -> Option<String> {
            None
        }
    }

    let fixture = Fixture::new("fresh-root-after");
    let root = fixture.manager.execution_root().to_path_buf();
    let failure = FreshExecutionRoot(&fixture.manager)
        .create_execution_root(&mut ErrorAt(HookPhase::After))
        .expect_err("the after phase fails");
    assert!(
        matches!(failure, FreshRootFailure::Claimed(_)),
        "made, then failed: {failure:?}"
    );
    for directory in SCAFFOLDING {
        assert!(
            root.join(directory).is_dir(),
            "the claimed root holds {directory}"
        );
    }

    let fixture = Fixture::new("fresh-root-before");
    let root = fixture.manager.execution_root().to_path_buf();
    let failure = FreshExecutionRoot(&fixture.manager)
        .create_execution_root(&mut ErrorAt(HookPhase::Before))
        .expect_err("the before phase fails");
    assert!(
        matches!(failure, FreshRootFailure::Unclaimed(_)),
        "nothing was made: {failure:?}"
    );
    assert!(fs::symlink_metadata(&root).is_err(), "and there is no root");
}

// -----------------------------------------------------------------------
// The containment refusals — real temp repositories, one test per reason
// -----------------------------------------------------------------------

#[cfg(unix)]
#[test]
fn a_symlink_below_the_private_root_refuses_the_execution_root() {
    let fixture = Fixture::new("symlink-chain");
    let workspaces = fixture
        .private
        .canonicalize()
        .expect("canonical")
        .join("workspaces");
    let elsewhere = fixture.root.join("elsewhere");
    fs::create_dir_all(&elsewhere).expect("target directory");
    std::os::unix::fs::symlink(&elsewhere, &workspaces).expect("plant the symlink");

    let error = fixture
        .manager
        .create_execution_root(&mut NoHooks)
        .expect_err("a reparse point on the chain refuses");
    let message = refusal_of(&error);
    assert!(
        message.contains("symlink or reparse point"),
        "the refusal must name its reason, not merely fail: {message}"
    );
    assert!(
        message.contains(&workspaces.display().to_string()),
        "and name the component: {message}"
    );
    assert!(
        !fixture.manager.execution_root().exists(),
        "and perform no effect"
    );
}

/// `PR28-NOTFOUND-NOT-CONVERGENCE`. A removal whose path is already gone has
/// achieved what it was asked to achieve. Before the fix this returned
/// `Err(NotFound)`, and `remove_worktree` would then skip the Git-admin
/// cleanup that follows it for a tree that was already deleted.
///
/// On Windows the sequence needs no second actor: an attempt answers
/// `ERROR_ACCESS_DENIED` for a delete-pending name, the last handle closes,
/// and the next attempt finds nothing. Verified red before the fix by a
/// scratch test on the guest, which asserted `is_err()` and passed.
#[test]
fn removing_a_path_that_is_already_gone_is_convergence_not_failure() {
    let tree = scratch("already-gone");
    let root = tree.path();
    fs::create_dir_all(root).expect("fixture root");
    let absent = root.join("no-such-tree");
    assert!(!absent.exists(), "the fixture must not create it");
    assert!(
        remove_tree_once_handles_close(&absent).is_ok(),
        "a path that is already gone is the outcome a removal wanted"
    );
    // And the ordinary case still works, so the arm above has not been
    // widened into "every error is success".
    let present = root.join("a-tree");
    fs::create_dir_all(present.join("nested")).expect("a tree to remove");
    assert!(remove_tree_once_handles_close(&present).is_ok());
    assert!(!present.exists(), "and it is actually gone");
}

/// A process the engine killed can still be closing its handles, and on Windows
/// that makes `remove_dir_all` answer `ERROR_SHARING_VIOLATION` for a worktree
/// that is about to become removable. `remove_worktree` retries across that
/// window rather than reporting a hard `Filesystem` failure.
///
/// This is the deterministic form of what the residue sampler hits at random:
/// the sampler kills a real `git` child at an unseeded point and *sometimes*
/// leaves a handle, so it proves the condition exists but cannot be re-run to
/// prove a fix. Here the handle is held on purpose.
///
/// **The oracle is the attempt count, not the clock and not the outcome.** Both
/// cases assert that the removal made more than one attempt, reading the count
/// from `fixture::observe_removal_attempts`; the primitive records an attempt
/// after it has returned and before its result is interpreted. Everything else
/// in this test exists to make that count mean something.
///
/// The first version had no such reading and could not get one: nothing outside
/// the loop can see an attempt, and the funnel's `Before` phase — the closest
/// seam a test otherwise has — fires *before* the primitive is entered. So the
/// closing case asserted the retry indirectly, by expecting a removal to
/// succeed against a handle it *believed* was open on the first attempt. That
/// belief is a scheduling assumption, and the frontier pass on `86fa1d8` showed
/// it is unsound in both directions:
///
/// * **The retry-deleted mutant survived.** Deschedule the remover between the
///   hook and the loop, let the holder's timed release expire first, and the
///   single remaining attempt succeeds unaided. `ATTEMPTS = 1` then passes.
/// * **And correct code could fail.** The same interleaving, unmutated, is a
///   removal that never needed the retry, which the old oracle also could not
///   tell from one that did.
///
/// Reading the count settles both, and the release is now driven from the same
/// seam. The holder does not close on a timer or on the funnel's signal; it
/// closes when it is told that **attempt 1 has already completed**, and the
/// observer then waits for the close before letting the loop continue. So
/// attempt 1 provably ran with the handle open, attempt 2 provably ran after it
/// closed, and neither fact depends on which thread the scheduler favours. The
/// 300ms hold and the five-second timer that preceded it are both gone, and
/// with them `FIND-109-CLOSING-CASE-HOLDER-STARVATION`, which existed only
/// because the release could not be ordered against an attempt.
///
/// The **held** case is the other half, and it is where the retry has to stop:
/// a handle held past the whole budget is a locked worktree, and reporting it
/// is the behaviour a retry that waits forever would destroy. That case never
/// releases until the assertions have run, so "was the handle still open?" is
/// not a question about scheduling there — the only thing that can close it is
/// `Holder`'s drop.
///
/// **Every wait is bounded, and bounded by one deadline rather than one each.**
/// `FAIL_SAFE` is taken once, before the holder starts, and every receive in
/// the test and in the holder waits only for what is left of it. The previous
/// shape gave each of the holder's two receives its own 600 seconds, so a
/// wedged removal could hold the job for twenty minutes and be killed by the
/// Windows job timeout with no verdict at all — which is the opposite of what a
/// fail-safe is for. `CODING_STANDARDS.md` §12.
#[cfg(windows)]
#[test]
fn a_worktree_whose_killed_child_is_still_closing_is_removed_not_refused() {
    use std::os::windows::fs::OpenOptionsExt as _;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc;
    use std::time::Instant;

    use windows_sys::Win32::Foundation::{ERROR_ACCESS_DENIED, ERROR_SHARING_VIOLATION};
    use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;

    /// One deadline for the whole of one case, and the only wall clock left.
    ///
    /// It converts a wedge into a verdict: if the retry ever became unbounded,
    /// removal would never return, the holder would never be released and the
    /// test would hang to the CI job timeout with nothing to read. Taken once
    /// and shared, so the *sum* of every wait is bounded by it — the shape it
    /// replaces gave two receives 600 seconds each, and nearly twenty minutes
    /// is longer than the Windows job has.
    ///
    /// **What a fire establishes is the bound it crossed, and not the reason.**
    /// An unbounded retry and a thread that stopped being scheduled read
    /// identically from here. The assertions say only the former and quote the
    /// elapsed time so a reader can tell them apart.
    ///
    /// Ten minutes is a wedge detector, not a percentile. Two minutes was tried
    /// and fired on a full-suite run of a loaded guest — `CODING_STANDARDS.md`
    /// §12's own warning, that a bound short enough to expire on a loaded
    /// runner has become the signal itself. Measured with the whole suite
    /// running, the locked case's `remove_worktree` returns in **1.0s** unpinned
    /// and **6.5–7.6s** pinned to 4 CPUs at `--test-threads=16`.
    const FAIL_SAFE: std::time::Duration = std::time::Duration::from_secs(600);

    /// Releases the holder, waits for it, joins it, and re-raises its panic.
    ///
    /// Every clause of that sentence was got wrong at least once, and the last
    /// frontier pass named the two that survived. Dropping a `JoinHandle` only
    /// detaches its thread, so a failing assertion left `Fixture::drop` — one
    /// best-effort `remove_dir_all` that ignores its result — racing a handle
    /// that had not closed. Discarding the join result makes a holder panic
    /// invisible: the file drops, the closing case can then succeed without ever
    /// exercising a retry, and §10 asks that a worker panic be a *defined*
    /// outcome. And the shape before this one **signalled too early** — the
    /// worker sent on `finished` from inside its own body, so a deschedule
    /// between that send and the thread's end left this `Drop` past its wait and
    /// inside an unbounded `join`, with the deadline no longer applying.
    ///
    /// So the readiness signal is now the channel's **disconnection** and
    /// nothing is ever sent on it: the `Sender` is moved into the worker's
    /// closure and drops when that closure does, which is after the body has
    /// returned — on a panic as well as on a normal return. `Disconnected`
    /// therefore means the body is finished and the `join` that follows waits
    /// only for thread teardown. `standards/12_standards_tests.md`'s readiness
    /// rule, applied to a thread rather than a file: the signal is published by
    /// the completion of the state it announces, never alongside it.
    ///
    /// **On expiry it still joins.** §12 wants the wait bounded and §10 forbids
    /// the detached worker, and when a worker is genuinely wedged those pull
    /// apart: the only way not to wait is to abandon the handle. This resolves
    /// it in §10's favour and pays for it by reporting *first* — the diagnostic
    /// is on stderr before the join is entered, so an operator whose job is then
    /// killed by its own timeout still learns why, which is exactly what
    /// detaching would deny them. The expiry is also close to unreachable by
    /// construction: the holder's only wait is on the channel this `Drop`
    /// disconnects one line earlier.
    ///
    /// Declared *after* the fixture, so it drops *before* it.
    struct Holder {
        close_now: Option<mpsc::Sender<()>>,
        finished: Option<mpsc::Receiver<()>>,
        handle: Option<std::thread::JoinHandle<()>>,
        deadline: Instant,
    }

    impl Drop for Holder {
        fn drop(&mut self) {
            // First, always: the holder's only wait is on this channel, so
            // joining ahead of the disconnect is the deadlock.
            drop(self.close_now.take());
            let Some(handle) = self.handle.take() else {
                return;
            };
            let mut wedged = false;
            if let Some(finished) = self.finished.take() {
                match finished.recv_timeout(self.deadline.saturating_duration_since(Instant::now()))
                {
                    // Nothing is ever sent on this channel. `Disconnected` is the
                    // worker's closure being dropped, which happens after its body
                    // has returned -- by a normal return or by a panic -- so the
                    // join below waits only for thread teardown.
                    Err(mpsc::RecvTimeoutError::Disconnected) => {}
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        wedged = true;
                        // Reported *before* the join, not after: if the worker is
                        // genuinely wedged the join will not return and the job
                        // will be killed by its own timeout, and this line is then
                        // the only thing that says why. Detaching instead would
                        // leave no worker to observe at all, which is the failure
                        // `standards/10_standards_concurrency.md` names.
                        eprintln!(
                            "the holder had not finished within {FAIL_SAFE:?} of this case \
                             starting; joining it anyway rather than detaching it, so this \
                             message may be the last output of the job"
                        );
                    }
                    // Unreachable: the sender is moved into the worker and never
                    // used. Treated as "finished" rather than asserted, because a
                    // `Drop` is the wrong place to discover a contradiction.
                    Ok(()) => {}
                }
            }
            let joined = handle.join();
            if let Err(payload) = joined {
                if std::thread::panicking() {
                    eprintln!(
                        "the holder thread panicked while the test was already unwinding; its \
                         payload cannot be re-raised without aborting"
                    );
                } else {
                    std::panic::resume_unwind(payload);
                }
            } else if wedged && !std::thread::panicking() {
                panic!(
                    "the holder did not finish within {FAIL_SAFE:?} of this case starting, and \
                     the join that followed did return -- so the wait was bounded and the worker \
                     was not detached, but something held it past the deadline"
                );
            }
        }
    }

    /// The closing window this control is sized against: a killed process whose
    /// last handle clears 300ms after the removal starts.
    ///
    /// It is the test's own number, not production's, which is what keeps the
    /// assertion below from being a self-oracle.
    const HOLD: std::time::Duration = std::time::Duration::from_millis(300);

    // The floor the retry budget has to clear, *derived* rather than restated:
    // covering `HOLD` at `STEP` per attempt takes `HOLD / STEP` of them. With
    // production's 25ms that is 12, against a budget of 40.
    //
    // Asserting only "more than one attempt" was not enough, and the frontier
    // pass proved it: cut `ATTEMPTS` from 40 to 2 and the closing case still
    // passes -- attempt 1 hands over, attempt 2 succeeds -- while the engine
    // would now refuse a real handle closing after 300ms about one 25ms interval
    // in. Deriving the floor from the two quantities that produce it means a
    // budget too small to tolerate `HOLD` fails here, and means the test still
    // says nothing about the exact value of `ATTEMPTS`, which is production's to
    // choose.
    let floor = u32::try_from(HOLD.as_millis() / STEP.as_millis())
        .expect("the derived attempt floor fits in the counter the seam reports");
    assert!(
        floor > 1,
        "the floor must be a real budget rather than the 'more than one attempt' this replaces:          HOLD {HOLD:?} over STEP {STEP:?} gives {floor}"
    );

    // `true`: a process whose last handle closes once `floor` attempts have
    // provably run, which is the window the retry exists to cross. `false`: one
    // that is still holding when the assertion runs, which is a locked worktree
    // however many attempts the loop spent reaching that conclusion.
    for (releases_after_the_floor, expected_removal) in [(true, true), (false, false)] {
        let fixture = Fixture::created("closing-handle");
        let slot = fixture.task("alpha", 1);
        fixture
            .manager
            .write_intent(&mut NoHooks, &slot)
            .expect("the intent must be durable");
        // The worktree is the instance the add made, at the path it returns:
        // this incarnation's, named with its tag. The slot's untagged spelling
        // names no checkout the manager makes (the follow-up C record, §4.2),
        // so nothing exists there and a file cannot be planted under it.
        let target = fixture
            .manager
            .add_worktree(&mut NoHooks, &slot, &fixture.head)
            .expect("the worktree the killed child was working in");

        // A file inside the worktree, held open with the sharing mode a running
        // process has. Opened on another thread so the handle outlives this
        // statement -- exactly the shape of a process that has exited while its
        // last handle is still closing.
        let held = target.join("held-by-the-dying-child");
        fs::write(&held, b"bytes the child had open").expect("plant the file");

        let case_began = Instant::now();
        let deadline = case_began + FAIL_SAFE;
        // Milliseconds into the case at which the removal's first attempt
        // returned; 0 means it never made one. The fail-safe's diagnostic reads
        // it, because "the fail-safe fired" and "the removal was slow" are two
        // different facts and the old assertion asserted the second from the
        // first.
        let first_attempt_ms = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let (opened, ready) = mpsc::channel::<()>();
        let (close_now, may_close) = mpsc::channel::<()>();
        let (closed, has_closed) = mpsc::channel::<()>();
        let (finished_here, finished) = mpsc::channel::<()>();
        let fail_safe_fired = Arc::new(AtomicBool::new(false));
        let fired = Arc::clone(&fail_safe_fired);
        let close_never_seen = Arc::new(AtomicBool::new(false));

        let held_by_the_child = held.clone();
        let handle = std::thread::spawn(move || {
            // `FILE_SHARE_READ` alone, deliberately: Rust's `File::open` asks for
            // `FILE_SHARE_DELETE` too, and a handle that shares deletion does not
            // block one -- the first version of this test removed the worktree
            // happily and proved nothing. A `git` child holds its files without
            // delete sharing, which is what makes the kernel answer
            // `ERROR_SHARING_VIOLATION`.
            let file = fs::OpenOptions::new()
                .read(true)
                .share_mode(FILE_SHARE_READ)
                .open(&held_by_the_child)
                .expect("hold the file open the way a git child does");
            opened.send(()).expect("announce the handle is held");
            // `Ok(())` is the closing case, told from inside the loop that an
            // attempt has completed. `Disconnected` is the held case, where the
            // only thing that closes this handle is `Holder`'s drop, after the
            // assertions have run -- and during an unwind too, so a failing
            // assertion does not leave the tree locked against the fixture's own
            // cleanup. `Timeout` is the fail-safe, recorded rather than acted on
            // so the assertion below can name it.
            if let Err(mpsc::RecvTimeoutError::Timeout) =
                may_close.recv_timeout(deadline.saturating_duration_since(Instant::now()))
            {
                fired.store(true, Ordering::SeqCst);
            }
            drop(file);
            // Best-effort: an error means the observer is already gone, which is
            // not this thread's business to report.
            let _ = closed.send(());
            // `finished_here` is never sent on. It is moved into this closure so
            // that its *drop* -- after this body returns, on a panic as well as
            // on a normal return -- is what tells `Holder` the body is over. See
            // that guard's doc: signalling from inside the body announced a
            // completion that had not happened.
            drop(finished_here);
        });
        // After `fixture`, so it drops before it. Nothing below may `join` this
        // thread by hand: the guard owns every half precisely so the
        // disconnect-then-wait-then-join order holds on every path out.
        // Cloned before the guard takes the original, because the closing
        // case's observer needs one of its own. The held case drops it below:
        // that channel disconnects only when the *last* sender goes, and the
        // guard's is meant to be the last.
        let observer_may_tell = close_now.clone();
        let _holder = Holder {
            close_now: Some(close_now),
            finished: Some(finished),
            handle: Some(handle),
            deadline,
        };
        // Declared after the guard, so it drops *before* it: the closing case's
        // observer owns a second sender, and the held case's channel cannot
        // disconnect until every sender is gone.
        // Both observers record when the removal actually entered the loop, so a
        // fail-safe fire can be told apart from a removal that never began.
        let began = Arc::clone(&first_attempt_ms);
        let stamp = move || {
            let ms = u64::try_from(case_began.elapsed().as_millis()).unwrap_or(u64::MAX);
            // `.max(1)` so "the first attempt landed inside the first
            // millisecond" is never confused with the 0 that means "never".
            let _ = began.compare_exchange(0, ms.max(1), Ordering::SeqCst, Ordering::SeqCst);
        };
        let observing = if releases_after_the_floor {
            let tell = observer_may_tell;
            let wait_for_the_close = has_closed;
            let unseen = Arc::clone(&close_never_seen);
            super::fixture::observe_removal_attempts(Box::new(move |attempt| {
                if attempt == 1 {
                    stamp();
                }
                if attempt != floor {
                    return;
                }
                // Ordered against an attempt rather than against a clock: every
                // attempt up to and including this one has already returned, so
                // each ran with the handle open, and waiting for the close here
                // means the next attempt runs after it. Both facts hold whatever
                // the scheduler does, and holding for `floor` attempts rather
                // than one is what makes the budget itself the subject.
                let _ = tell.send(());
                if wait_for_the_close
                    .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                    .is_err()
                {
                    unseen.store(true, Ordering::SeqCst);
                }
            }))
        } else {
            // Neither is wanted here, and both must go before the wait below:
            // the sender would keep `may_close` connected past the guard's
            // drop, which is the hang the deadline would then have to catch.
            drop(observer_may_tell);
            drop(has_closed);
            super::fixture::observe_removal_attempts(Box::new(move |attempt| {
                if attempt == 1 {
                    stamp();
                }
            }))
        };
        ready
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("the handle is held before removal is attempted");

        let started = Instant::now();
        let outcome = fixture.manager.remove_worktree(&mut NoHooks, &slot);
        let took = started.elapsed();
        let attempts = observing.count();

        // Read before the outcome is interpreted, so the diagnostic names the
        // real fault rather than whatever the freed handle then permitted.
        //
        // The fail-safe measures the case, which starts before removal does, so
        // firing does NOT by itself mean the removal was slow: this thread could
        // simply not have been scheduled to enter the funnel. The two are told
        // apart by whether an attempt was ever recorded, and the message says
        // which happened rather than asserting the one from the other -- that
        // conflation is `SOL-8D4-03`.
        if fail_safe_fired.load(Ordering::SeqCst) {
            let entered_at = first_attempt_ms.load(Ordering::SeqCst);
            assert!(
                entered_at != 0,
                "the fail-safe expired {FAIL_SAFE:?} after this case began and the removal had \
                 not completed a single attempt by then, so this thread was never scheduled to \
                 reach the retry at all. That is a scheduling failure of the test and says \
                 nothing about how long a removal took: it returned in {took:?} as {} after \
                 {attempts} attempt(s)",
                if outcome.is_ok() { "Ok" } else { "Err" }
            );
            panic!(
                "the fail-safe expired {FAIL_SAFE:?} after this case began with removal still \
                 inside the funnel -- its first attempt completed {entered_at}ms into the case, \
                 so the bound was spent on the removal and not on reaching it. Removal then \
                 returned in {took:?} as {} after {attempts} attempt(s). That establishes the \
                 bound it crossed and not the reason for it: an unbounded retry is why this \
                 bound exists, but a remover that stopped being scheduled reads the same",
                if outcome.is_ok() { "Ok" } else { "Err" }
            );
        }
        assert!(
            !close_never_seen.load(Ordering::SeqCst),
            "the holder never reported closing the handle within {FAIL_SAFE:?}, so the attempts \
             after the first ran against an unknown state and this run measured nothing: \
             {attempts} attempt(s) in {took:?}"
        );
        // The whole oracle. Both cases require a real retry, so the mutant that
        // deletes it (`ATTEMPTS = 1`) fails here by the count -- not by whether
        // a handle happened to still be open when the single attempt ran, which
        // is a race the test used to be on the wrong side of.
        assert!(
            attempts > floor,
            "the removal's budget must cover the closing window this control is sized against: \
             the loop reported {attempts} attempt(s) in {took:?}, and {HOLD:?} at {STEP:?} per \
             attempt needs more than {floor}. A count at or below the floor means the budget is \
             too small to tolerate a handle that clears in {HOLD:?} -- or, at one attempt, that \
             the retry is gone or the primitive was never entered"
        );

        if expected_removal {
            outcome.expect("removal retries across the closing handle");
            assert!(
                !target.exists(),
                "and the worktree is actually gone, not merely un-refused"
            );
        } else {
            // The wording is deliberately not the one this assertion carried
            // while the handle was held on a five-second timer. That message is
            // the recorded fingerprint of a retired flake, and a failure here now
            // means removal succeeded against a handle that was provably *still
            // open* -- a defect, not a scheduling artifact. Reusing the text
            // would invite the next reader to discount it.
            let error = outcome.expect_err(
                "a handle still held when removal returns is a locked worktree, not a \
                 closing one, and must be reported rather than waited on forever",
            );
            assert!(
                target.exists(),
                "and the worktree is still there, which is what the error says: {}",
                refusal_of(&error)
            );
            // `Filesystem`, and the operation is matched rather than assumed.
            // Master's sweep of this module split the two variants by §7's
            // operation-context rule -- a removal that fails did not fail to
            // *read* -- so `remove_worktree` now maps this failure to
            // `Filesystem { operation: "remove" }`. That split reached this
            // branch through a base merge that was textually clean, which is
            // why the destructure below went on compiling against a variant the
            // engine no longer returns: `code` and the path both fell to `None`
            // and the two assertions after them could not pass on any run.
            let (refused_path, code) = match &error {
                UpstrokeError::Filesystem {
                    operation: "remove",
                    path,
                    source,
                } => (Some(path.as_path()), source.raw_os_error()),
                _ => (None, None),
            };
            assert!(
                matches!(
                    code,
                    Some(code)
                        if code == ERROR_SHARING_VIOLATION as i32
                            || code == ERROR_ACCESS_DENIED as i32
                ),
                "the refusal must be the kernel's answer about the held handle: {}",
                refusal_of(&error)
            );
            // ...and it must have come from the removal itself. Neither the code
            // nor the variant alone says so. `revalidate_removal` runs before
            // the funnel is entered and reports its own failures -- an
            // unreadable `.git/worktrees` answers error 5 too -- leaving the
            // target on disk, so the `exists` assertion above would hold without
            // the loop ever running; the variant now excludes *that* step, since
            // those failures are reads and stay `Io`, but not the funnel's other
            // removals, which carry the same operation against the `locked` file
            // and the admin directory. Only `remove_tree_once_handles_close`'s
            // error names the contained worktree, and `same_path` compares
            // directories rather than spellings, which is what the earlier path
            // cannot match.
            assert!(
                refused_path.is_some_and(|path| crate::util::same_path(path, &target)),
                "the refusal must name the worktree the loop failed to remove, so that it \
                 is the budget being reported and not an earlier step: {}",
                refusal_of(&error)
            );
        }
    }
}

/// The classifier's read of a registration marker across a delete-pending
/// name: the deterministic form of what CI's Windows leg met at random in
/// PR10's round 6 (`Worktree.Add`, one sample of eight, `locked: Access is
/// denied (os error 5)`), where a `git worktree add` the sampler had just
/// terminated still held the marker it was unlinking. The sampler proves the
/// condition exists and cannot be re-run to prove a fix; here the name is
/// held delete-pending on purpose, the way the removal control above holds
/// its file, and released against an attempt the seam reports rather than
/// against a clock (`PR109-ORACLE-OBSERVES-TIMING-NOT-ATTEMPTS`).
///
/// Two cases. **Closing**: the holder closes once the read's first attempt
/// has provably returned against the delete-pending name, so a later attempt
/// finds the name gone — the marker its deleter meant to remove — and the
/// populated worktree classifies `After`. **Held**: the name is held through
/// the classifier's whole budget, and the classifier answers `Internal` —
/// the lock the add holds, the class the inventory names for a `locked`
/// marker that stands — rather than the inspection error CI recorded, since
/// ST-07 requires every sampled residue classified. The attempt count is
/// asserted in both, so a read that never retried (one attempt, answering
/// the held class at once) fails the closing case by its answer and by its
/// count, and a read that propagated the error after the budget fails the
/// held case by its answer.
#[cfg(windows)]
#[test]
fn a_locked_marker_a_terminated_add_still_holds_is_read_across_its_closing_and_as_held_past_it() {
    use std::os::windows::fs::OpenOptionsExt as _;
    use std::os::windows::io::AsRawHandle as _;
    use std::sync::mpsc;
    use std::time::Instant;

    use windows_sys::Win32::Foundation::ERROR_ACCESS_DENIED;
    use windows_sys::Win32::Storage::FileSystem::{
        DELETE, FILE_DISPOSITION_FLAG_DELETE, FILE_DISPOSITION_FLAG_POSIX_SEMANTICS,
        FILE_DISPOSITION_INFO_EX, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
        FileDispositionInfoEx, SetFileInformationByHandle,
    };

    /// One deadline per case, the wedge detector the removal control explains.
    const FAIL_SAFE: std::time::Duration = std::time::Duration::from_secs(600);

    let site = EffectSiteId::Worktree(WorktreeSite::Add);
    for (releases_after_the_first_attempt, expected) in [
        (true, ObjectResidue::After),
        (false, ObjectResidue::Internal),
    ] {
        let fixture = Fixture::created("held-locked-marker");
        let slot = fixture.task("alpha", 1);
        fixture
            .manager
            .write_intent(&mut NoHooks, &slot)
            .expect("the intent must be durable");
        let path = fixture
            .manager
            .add_worktree(&mut NoHooks, &slot, &fixture.head)
            .expect("a populated worktree, its add complete");
        let admin = git_dir_of(&path)
            .expect("the pointer reads")
            .expect("a populated worktree has a git dir behind its pointer");
        let locked = admin.join("locked");
        fs::write(&locked, b"initializing\n").expect("plant the add's own marker");
        let base = fixture.base.clone();
        let target = ResidueTarget::new(&base).at(&path).from_base(&fixture.head);
        assert_eq!(
            classify_object_residue(site, &target).expect("the classifier answers"),
            ObjectResidue::Internal,
            "premise: a readable `initializing` marker is the unpopulated class"
        );

        let deadline = Instant::now() + FAIL_SAFE;
        let (opened, ready) = mpsc::channel::<()>();
        let (close_now, may_close) = mpsc::channel::<()>();
        let (closed, has_closed) = mpsc::channel::<()>();
        let held = locked.clone();
        let holder = std::thread::spawn(move || {
            // The marking handle itself is what holds the name: a delete
            // disposition set through a handle that stays open leaves the
            // name in the directory, delete-pending, until that handle closes
            // -- the shape a terminated add's last unlink leaves behind. Not
            // `fs::remove_file`, which sets the disposition through a handle
            // of its own and closes it at once, so that the name is gone
            // before anything can meet it delete-pending (measured on the
            // guest: the premise below then read `NotFound`). The technique
            // is `runner::container::tests::windows_posix_delete_pending`'s.
            let file = fs::OpenOptions::new()
                .access_mode(DELETE)
                .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
                .open(&held)
                .expect("hold the marker open for deletion");
            let disposition = FILE_DISPOSITION_INFO_EX {
                Flags: FILE_DISPOSITION_FLAG_DELETE | FILE_DISPOSITION_FLAG_POSIX_SEMANTICS,
            };
            let size = u32::try_from(std::mem::size_of::<FILE_DISPOSITION_INFO_EX>())
                .expect("a small struct");
            // SAFETY: `file` is open for the duration of the call, `disposition`
            // is a fully initialised `FILE_DISPOSITION_INFO_EX` and `size` is
            // its size, which is the contract `FileDispositionInfoEx` documents.
            let set = unsafe {
                SetFileInformationByHandle(
                    file.as_raw_handle(),
                    FileDispositionInfoEx,
                    (&raw const disposition).cast(),
                    size,
                )
            };
            assert_ne!(
                set,
                0,
                "mark the held name delete-pending: {}",
                std::io::Error::last_os_error()
            );
            opened.send(()).expect("announce the delete-pending name");
            // `Ok` is the closing case, told from inside the read that its
            // first attempt has returned; `Disconnected` is the held case,
            // released after the classifier has answered; `Timeout` is the
            // fail-safe, and the join below re-raises whatever it left.
            let _ = may_close.recv_timeout(deadline.saturating_duration_since(Instant::now()));
            drop(file);
            let _ = closed.send(());
        });
        ready
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("the marker is delete-pending before the classifier runs");
        let premise = fs::read(&locked).expect_err("premise: a delete-pending name refuses a read");
        assert_eq!(
            premise.raw_os_error(),
            Some(ERROR_ACCESS_DENIED as i32),
            "premise: the kernel answers a read of a delete-pending name with error 5: {premise}"
        );

        let mut release = Some(close_now);
        let observing = if releases_after_the_first_attempt {
            let tell = release.take().expect("the closing case's release");
            super::fixture::observe_marker_read_attempts(Box::new(move |attempt| {
                if attempt != 1 {
                    return;
                }
                // Ordered against the attempt rather than a clock: attempt 1
                // has already returned against the delete-pending name, and
                // waiting for the close here means attempt 2 runs after it.
                let _ = tell.send(());
                let _ = has_closed.recv_timeout(deadline.saturating_duration_since(Instant::now()));
            }))
        } else {
            drop(has_closed);
            super::fixture::observe_marker_read_attempts(Box::new(|_| {}))
        };
        let answer = classify_object_residue(site, &target);
        let attempts = observing.count();
        drop(observing);
        // The held case's release, after the answer: the only thing that
        // closes its handle. Then the holder is joined on every path, so a
        // failing assertion never leaves the tree locked against the
        // fixture's own removal.
        drop(release);
        let joined = holder.join();
        if let Err(payload) = joined {
            std::panic::resume_unwind(payload);
        }

        let answer = answer.unwrap_or_else(|error| {
            panic!(
                "releases after the first attempt: {releases_after_the_first_attempt}: the \
                 classifier refused a marker a terminated add still holds, after {attempts} \
                 attempt(s): {error}"
            )
        });
        assert_eq!(
            answer, expected,
            "releases after the first attempt: {releases_after_the_first_attempt}: after \
             {attempts} attempt(s)"
        );
        if releases_after_the_first_attempt {
            assert!(
                attempts > 1 && attempts < ATTEMPTS,
                "the closing case is crossed by a retry, and before the budget is spent: \
                 {attempts} attempt(s) of {ATTEMPTS}"
            );
            assert!(
                !locked.exists(),
                "the delete-pending marker is gone once its holder closed"
            );
        } else {
            assert_eq!(
                attempts, ATTEMPTS,
                "the held case spends the whole budget before the marker is read as held"
            );
        }
    }
}

/// The Unix arm of the marker read makes exactly one attempt, and the seam
/// reports that one — the removal seam's Unix pin, for the classifier's
/// read: the retry and the held answer exist for the Windows control above,
/// so on every other leg this pins the seam where CI runs it every time, and
/// reads the two answers the Unix arm does give through the classifier.
#[cfg(not(windows))]
#[test]
fn a_marker_read_records_the_one_attempt_the_unix_arm_makes() {
    let site = EffectSiteId::Worktree(WorktreeSite::Add);
    let fixture = Fixture::created("marker-read-once");
    let slot = fixture.task("alpha", 1);
    fixture
        .manager
        .write_intent(&mut NoHooks, &slot)
        .expect("the intent must be durable");
    let path = fixture
        .manager
        .add_worktree(&mut NoHooks, &slot, &fixture.head)
        .expect("a populated worktree, its add complete");
    let admin = git_dir_of(&path)
        .expect("the pointer reads")
        .expect("a populated worktree has a git dir behind its pointer");
    let locked = admin.join("locked");
    let base = fixture.base.clone();
    let target = ResidueTarget::new(&base).at(&path).from_base(&fixture.head);

    fs::write(&locked, b"initializing\n").expect("plant the add's own marker");
    let observing = super::fixture::observe_marker_read_attempts(Box::new(|_| {}));
    let answer = classify_object_residue(site, &target).expect("the classifier answers");
    assert_eq!(
        (answer, observing.count()),
        (ObjectResidue::Internal, 1),
        "a readable `initializing` marker is the unpopulated class, read in one attempt"
    );
    drop(observing);

    fs::remove_file(&locked).expect("release the marker");
    let observing = super::fixture::observe_marker_read_attempts(Box::new(|_| {}));
    let answer = classify_object_residue(site, &target).expect("the classifier answers");
    assert_eq!(
        (answer, observing.count()),
        (ObjectResidue::After, 1),
        "an absent marker is the populated class, read in one attempt"
    );
}

/// The Unix arm makes exactly one attempt per removal, and the seam reports
/// each.
///
/// The seam exists for the Windows control above, which is the only place a
/// retry can happen — so on every other leg it would be code nothing exercises,
/// and a mis-wiring of it would show up only on the platform whose suite is
/// hardest to run. This pins it where CI runs it every time.
///
/// Two removals, one attempt each, not "at least one": since #329 the forced
/// removal deletes the registration it bound directly instead of pruning
/// (`48f2af7a:reviews/2026-10-01-pr11-follow-up-b-record.md`, §2.5), so
/// `remove_tree_once_handles_close` is called once for the checkout and once for
/// the Git admin directory, and each call's one attempt is numbered 1. The seam
/// reports the most attempts one removal made, so a count above one, or an
/// observer run more than twice, would mean it is counting something other
/// than what it says.
///
/// **What this deliberately does not claim.** An earlier version said it proved
/// the observer runs "once per attempt rather than once per call". It cannot: a
/// Unix removal has exactly one attempt, so one call and one attempt are the
/// same number and no assertion here can separate them. That property is only
/// observable where a retry is, and the Windows control observes it directly by
/// requiring a count above its derived floor. The claim was removed rather than
/// dressed up, which is the `SOL-8D4-04` repair.
#[cfg(not(windows))]
#[test]
fn a_removal_records_the_one_attempt_the_unix_arm_makes() {
    use std::sync::atomic::{AtomicU32, Ordering};

    let fixture = Fixture::created("attempt-seam");
    let slot = fixture.task("alpha", 1);
    fixture
        .manager
        .write_intent(&mut NoHooks, &slot)
        .expect("the intent must be durable");
    fixture
        .manager
        .add_worktree(&mut NoHooks, &slot, &fixture.head)
        .expect("the worktree to remove");

    let calls = Arc::new(AtomicU32::new(0));
    let ordinals = Arc::new(AtomicU32::new(0));
    let counted = Arc::clone(&calls);
    let seen = Arc::clone(&ordinals);
    let observing = super::fixture::observe_removal_attempts(Box::new(move |attempt| {
        counted.fetch_add(1, Ordering::SeqCst);
        seen.fetch_max(attempt, Ordering::SeqCst);
    }));

    fixture
        .manager
        .remove_worktree(&mut NoHooks, &slot)
        .expect("an unheld worktree is removed on the first attempt");

    assert_eq!(
        observing.count(),
        1,
        "each removal, the checkout's and its registration's, made one attempt, and the seam \
         must say so"
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "the observer ran once for each of those two attempts"
    );
    assert_eq!(
        ordinals.load(Ordering::SeqCst),
        1,
        "each attempt is numbered from one, which is what the retry assertions read"
    );
}

/// The observation is this thread's, and the guard's drop ends it.
///
/// Both halves matter to the control above. The suite runs at
/// `--test-threads=16` and several of its tests remove worktrees, so a
/// process-wide counter would report a neighbour's removals as this test's; and
/// a guard that left its observer installed would hand the closure to whatever
/// ran next on the same thread, which for a test that unwinds is a closure
/// holding channels nobody is reading.
///
/// **The second half is asserted through the observer rather than the counter**,
/// and that is the `SOL-8D4-04` repair. Reading `count()` after the drop proves
/// nothing about the drop: the next observation resets the counter to zero on
/// the way in, so the assertion passed whether or not the guard had uninstalled
/// anything, and deleting `AttemptObservation`'s `Drop` left the test green. A
/// counting observer that outlives its guard is a different matter — if the
/// uninstall does not happen, the removal below reaches it and the count moves.
#[cfg(not(windows))]
#[test]
fn the_attempt_observation_is_this_threads_and_ends_with_its_guard() {
    use std::sync::atomic::{AtomicU32, Ordering};

    let fixture = Fixture::created("attempt-seam-scope");
    let slot = fixture.task("alpha", 1);
    fixture
        .manager
        .write_intent(&mut NoHooks, &slot)
        .expect("the intent must be durable");
    fixture
        .manager
        .add_worktree(&mut NoHooks, &slot, &fixture.head)
        .expect("the worktree to remove");

    // Counting, not a no-op: this is the only thing that can still be observed
    // once the guard is gone, so it is what the second assertion reads.
    let reached = Arc::new(AtomicU32::new(0));
    let counted = Arc::clone(&reached);
    let observing = super::fixture::observe_removal_attempts(Box::new(move |_| {
        counted.fetch_add(1, Ordering::SeqCst);
    }));

    // A removal on ANOTHER thread must not reach this observation. The second
    // fixture is that thread's own, because two removals of one slot are a
    // different test.
    std::thread::scope(|scope| {
        scope.spawn(|| {
            let other = Fixture::created("attempt-seam-elsewhere");
            let slot = other.task("beta", 1);
            other
                .manager
                .write_intent(&mut NoHooks, &slot)
                .expect("the intent must be durable");
            other
                .manager
                .add_worktree(&mut NoHooks, &slot, &other.head)
                .expect("the worktree to remove");
            other
                .manager
                .remove_worktree(&mut NoHooks, &slot)
                .expect("removed on its own thread");
        });
    });
    assert_eq!(
        observing.count(),
        0,
        "another thread's removal is not this observation's attempt"
    );
    assert_eq!(
        reached.load(Ordering::SeqCst),
        0,
        "and it does not reach this thread's observer either"
    );

    drop(observing);
    // With the guard gone the observer must be uninstalled, so this removal --
    // on this very thread, where it would otherwise be seen -- reaches nothing.
    fixture
        .manager
        .remove_worktree(&mut NoHooks, &slot)
        .expect("an unheld worktree is removed");
    assert_eq!(
        reached.load(Ordering::SeqCst),
        0,
        "the guard's drop must uninstall the observer: a removal after it reached the closure, \
         so the observation outlived the guard that owns it"
    );
}

/// The Windows half of `expected_failures_refusals[0]`: a **junction** is a
/// reparse point that is not a symbolic link, so a refusal written against
/// `FileType::is_symlink` alone would pass every Linux test and refuse
/// nothing an operator can actually build with `mklink /J`.
#[cfg(windows)]
#[test]
fn a_junction_below_the_private_root_refuses_the_execution_root() {
    let fixture = Fixture::new("junction-chain");
    let workspaces = fixture
        .private
        .canonicalize()
        .expect("canonical")
        .join("workspaces");
    let elsewhere = fixture.root.join("elsewhere");
    fs::create_dir_all(&elsewhere).expect("target directory");
    let made = Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&workspaces)
        .arg(&elsewhere)
        .output()
        .expect("run mklink");
    assert!(
        made.status.success(),
        "the fixture must really create a junction: {}",
        String::from_utf8_lossy(&made.stderr)
    );

    // The premise this test exists to hold: a junction is *not* what
    // `FileType::is_symlink` is about on every reparse tag, so the check
    // has to read the attribute.
    let metadata = fs::symlink_metadata(&workspaces).expect("junction metadata");
    assert!(
        is_reparse_point(&metadata),
        "the detector must see a junction as a reparse point"
    );

    let error = fixture
        .manager
        .create_execution_root(&mut NoHooks)
        .expect_err("a junction on the chain refuses");
    let message = refusal_of(&error);
    assert!(
        message.contains("symlink or reparse point"),
        "the refusal must name its reason: {message}"
    );
    assert!(!fixture.manager.execution_root().exists());
}

/// The **leaf** of the managed base and of the private root is a chain
/// component too.
///
/// `execution_root` is "created only when the managed base is a real
/// directory with **no symlink/reparse point on the chain**", and
/// `refuse_unreal_directory` is the only check either leaf gets:
/// `reparse_point_below` walks the components *under* its anchor, and
/// `canonical_prefix` resolves a link rather than refusing it. So a leaf
/// that is a link to a real directory reaches every effect unless this
/// function reads the link itself.
///
/// `PR5-CORRECTNESS-003`: the existing coverage planted its link *below*
/// the private root, where `refuse_reparse_points` catches it, so
/// `fs::symlink_metadata` -> `fs::metadata` here survived the whole suite.
///
/// All three call sites, because the class is the call and not the
/// argument: `derive`'s base, `derive`'s private root, and `revalidate`'s
/// base — the last reached by replacing an already-derived base with a link
/// to itself, which is the sequence "every create/reclaim/delete
/// revalidates" exists for.
#[test]
fn a_managed_base_or_private_root_that_is_itself_a_link_refuses_before_any_effect() {
    let mut refused = Vec::new();

    // (1) derive's base.
    let fixture = Fixture::new("leaf-link-base");
    let real = fixture.base.canonicalize().expect("canonical base");
    let link = fixture.root.join("base-link");
    plant_directory_link(&real, &link);
    let error = WorkspaceManager::derive(
        &link,
        &fixture.private,
        "01KZSWEEP00000000000000009",
        "inc-1",
    )
    .expect_err("a managed base that is a link refuses");
    refused.push(("derive/base", refusal_of(&error)));

    // (2) derive's private root.
    let fixture = Fixture::new("leaf-link-private");
    let real = fixture.private.canonicalize().expect("canonical private");
    let link = fixture.root.join("private-link");
    plant_directory_link(&real, &link);
    let error =
        WorkspaceManager::derive(&fixture.base, &link, "01KZSWEEP00000000000000009", "inc-1")
            .expect_err("a private root that is a link refuses");
    refused.push(("derive/private-root", refusal_of(&error)));

    // (3) revalidate's base — the link arrives *after* derive succeeded.
    let fixture = Fixture::created("leaf-link-revalidate");
    let base = fixture.manager.base().to_path_buf();
    let moved = fixture.root.join("moved-away");
    fs::rename(&base, &moved).expect("move the real repository aside");
    plant_directory_link(&moved, &base);
    let error = fixture
        .manager
        .revalidate()
        .expect_err("a base that became a link refuses on revalidation");
    refused.push(("revalidate/base", refusal_of(&error)));
    // And the refusal reaches the primitives, not merely the private check.
    let slot = fixture.task("alpha", 1);
    fixture
        .manager
        .write_intent(&mut NoHooks, &slot)
        .expect_err("and every primitive revalidates first");

    assert_eq!(refused.len(), 3, "three call sites, each driven");
    for (site, message) in &refused {
        assert!(
            message.contains("not a real directory"),
            "{site}: the refusal must name its reason: {message}"
        );
    }
    // Distinct paths named, so one refusal cannot stand in for three.
    let named: std::collections::BTreeSet<&str> = refused.iter().map(|(site, _)| *site).collect();
    assert_eq!(named.len(), 3, "three distinct call sites: {named:?}");
}

/// The walk's own guarantee, driven directly: a root that is not plain
/// components below the anchor — `..` in the remainder, or no common prefix
/// at all — is refused rather than walked. The walk answered "no reparse
/// point" for such a root, true of a chain it never inspected, and on Linux
/// `derive` then succeeded with `<private>/workspaces/<key>/../../../escape`
/// as its execution root. Through `derive` these shapes are now refused
/// earlier, as run ids; this pins the arm behind that one.
#[test]
fn an_execution_root_with_no_plain_chain_below_the_private_root_refuses_before_any_effect() {
    let fixture = Fixture::new("root-not-below");
    let anchor = fixture.manager.private_root().to_path_buf();
    for root in [
        anchor
            .join("workspaces")
            .join("k")
            .join("..")
            .join("..")
            .join("..")
            .join("escape"),
        fixture.root.join("escape"),
    ] {
        let error = refuse_reparse_points(&anchor, &root, Leaf::Directory)
            .expect_err("a root with no plain chain below the anchor refuses");
        let message = refusal_of(&error);
        assert!(
            message.contains("does not lie below the authorized private root"),
            "{}: the refusal must name its reason: {message}",
            root.display()
        );
    }
    assert!(
        !fixture.root.join("escape").exists() && !anchor.join("workspaces").exists(),
        "and perform no effect"
    );
}

/// A run id is one plain component, refused at `derive` before any path is
/// built. `Path::join` lets an absolute id replace the whole prefix, so an
/// absolute id naming a peer run's root aliased that root: `revalidate`
/// treated the peer's worktree as this manager's slot and `remove_worktree`
/// could have deleted its checkout and Git admin entry. `.` passed the walk
/// because `components()` folds a non-leading `.` away, aliasing the repo-key
/// directory. Every shape, on every platform.
#[test]
fn a_run_id_that_is_not_one_plain_component_is_refused_before_any_path_is_built() {
    let victim = Fixture::created("alias-victim");
    let slot = victim.add_task(&mut NoHooks, "k1", 1);
    let victim_root = victim
        .manager
        .execution_root()
        .to_str()
        .expect("a UTF-8 scratch path")
        .to_owned();
    for run_id in [
        victim_root.as_str(),
        ".",
        "..",
        "",
        "../../../escape",
        "/escape",
        "a/b",
        "-run",
        "run.",
    ] {
        let error = WorkspaceManager::derive(&victim.base, &victim.private, run_id, "inc-2")
            .expect_err("a run id that is not one plain component refuses");
        let message = refusal_of(&error);
        assert!(
            message.contains("refusing the run id"),
            "{run_id:?}: the refusal must name its reason: {message}"
        );
    }
    assert!(
        victim.manager.slot_path(&slot).is_dir(),
        "and the peer's worktree is untouched"
    );
    assert!(
        !victim.root.join("escape").exists(),
        "and nothing was built"
    );
}

/// `execution_root`: "every create/reclaim/delete revalidates". The walk
/// pushed the first child before its first `symlink_metadata`, so the private
/// root itself was never examined: renamed away after `derive` and replaced
/// by a link to an unrelated directory, every component under it was read
/// through the link and `create_execution_root` built the hierarchy under
/// the link's target. A symlink here, a junction on Windows.
#[test]
fn a_private_root_replaced_by_a_link_after_derive_refuses_every_revalidation() {
    let fixture = Fixture::new("anchor-link");
    let private = fixture.manager.private_root().to_path_buf();
    let elsewhere = fixture.root.join("elsewhere");
    fs::create_dir_all(&elsewhere).expect("the unrelated directory");
    let moved = fixture.root.join("private-moved");
    fs::rename(&private, &moved).expect("move the real private root aside");
    plant_directory_link(&elsewhere, &private);

    let error = fixture
        .manager
        .create_execution_root(&mut NoHooks)
        .expect_err("a private root that became a link refuses");
    let message = refusal_of(&error);
    assert!(
        message.contains("not a real directory"),
        "the refusal must name its reason: {message}"
    );
    assert!(
        !elsewhere.join("workspaces").exists() && !moved.join("workspaces").exists(),
        "and nothing is built under the link's target"
    );
}

/// The same exchange one level up: the private root is still a real
/// directory at its recorded path, but an ancestor is now a link, so the
/// recorded canonical root no longer resolves to itself. The anchored walk
/// never looks above the anchor; the canonical pin does.
#[test]
fn a_link_planted_above_the_private_root_after_derive_refuses_every_revalidation() {
    let fixture = Fixture::new("anchor-ancestor");
    let holder = fixture.root.join("holder");
    fs::create_dir_all(holder.join("private")).expect("the held private root");
    let manager = WorkspaceManager::derive(
        &fixture.base,
        &holder.join("private"),
        "01KZSWEEP00000000000000007",
        "inc-1",
    )
    .expect("derive under the holder");
    let moved = fixture.root.join("holder-moved");
    fs::rename(&holder, &moved).expect("move the holder aside");
    plant_directory_link(&moved, &holder);

    let error = manager
        .create_execution_root(&mut NoHooks)
        .expect_err("a link above the private root refuses");
    let message = refusal_of(&error);
    assert!(
        message.contains("symlink or reparse point"),
        "the refusal must name its reason: {message}"
    );
    assert!(
        !moved.join("private").join("workspaces").exists(),
        "and nothing is built through the link"
    );
}

/// A regular file where a directory of the chain should be is a broken
/// chain, reported where it stands and on every platform alike: the walk
/// reads the component's type rather than waiting for the `ENOTDIR` only
/// Unix raises one component later, and never walks past it. Walking past
/// handed the failure to the next effect, or to none: `remove_execution_root`
/// asks `exists()`, which folded the file into "nothing to remove" and
/// answered `Ok(false)` with no path named.
#[test]
fn a_regular_file_on_the_chain_is_reported_where_it_stands_and_never_as_nothing_to_remove() {
    let fixture = Fixture::new("file-on-chain");
    let file = fixture.manager.private_root().join("workspaces");
    fs::write(&file, "not a directory\n").expect("plant the file");

    let error = fixture
        .manager
        .revalidate()
        .expect_err("a file on the chain is a broken chain, not absence");
    assert!(
        matches!(
            &error,
            UpstrokeError::Io { path, source }
                if path == &file && source.kind() == std::io::ErrorKind::NotADirectory
        ),
        "the walk names the file itself, as not a directory: {error}"
    );
    fixture
        .manager
        .create_execution_root(&mut NoHooks)
        .expect_err("the create revalidates and refuses");
    fixture
        .manager
        .remove_execution_root(&mut NoHooks)
        .expect_err(
            "the removal revalidates and refuses rather than answering \"nothing to remove\"",
        );
}

/// `DESIGN.md` §15: every create, reclaim and delete revalidates inside its
/// effect funnel, after the before-hook and immediately before the effect. A
/// revalidation outside the funnel left a window: a `Before` hook — the seam
/// `a_registration_rebound_after_validation_keeps_its_admin_state` already
/// drives — that renamed the private root away and planted a link in its
/// place after the check had passed, and `create_dir_all` then built the
/// hierarchy under the link's target. The hook here does exactly that at the
/// create's `Before`, so the only check that can refuse is the one inside.
#[test]
fn a_private_root_exchanged_between_the_before_hook_and_the_effect_is_still_refused() {
    struct ExchangeAtBefore {
        private: PathBuf,
        moved: PathBuf,
        elsewhere: PathBuf,
    }

    impl EffectHooks for ExchangeAtBefore {
        fn phase(&mut self, site: EffectSiteId, phase: HookPhase) -> Injection {
            if site == EffectSiteId::Worktree(WorktreeSite::CreateExecutionRoot)
                && phase == HookPhase::Before
            {
                fs::rename(&self.private, &self.moved).expect("move the real private root aside");
                plant_directory_link(&self.elsewhere, &self.private);
            }
            Injection::Proceed
        }

        fn refusal_cause(&self) -> Option<String> {
            None
        }
    }

    let fixture = Fixture::new("anchor-exchanged-in-funnel");
    let elsewhere = fixture.root.join("elsewhere");
    fs::create_dir_all(&elsewhere).expect("the unrelated directory");
    let mut hooks = ExchangeAtBefore {
        private: fixture.manager.private_root().to_path_buf(),
        moved: fixture.root.join("private-moved"),
        elsewhere: elsewhere.clone(),
    };

    let error = fixture
        .manager
        .create_execution_root(&mut hooks)
        .expect_err("the check inside the funnel sees the exchanged root");
    let message = refusal_of(&error);
    assert!(
        message.contains("not a real directory"),
        "the refusal must name its reason: {message}"
    );
    assert!(
        !elsewhere.join("workspaces").exists() && !hooks.moved.join("workspaces").exists(),
        "and nothing is built under the link's target or the moved root"
    );
}

/// An absent managed base, or an absent private root, is "not a real
/// directory" — the refusal `execution_root` names — and not an I/O failure
/// to read it. Only an actual not-found becomes absence; a leaf that cannot
/// be read for any other reason stays an error.
#[test]
fn an_absent_managed_base_or_private_root_refuses_as_not_a_real_directory() {
    let fixture = Fixture::new("absent-leaf");
    let absent = fixture.root.join("absent");
    for (site, base, private) in [
        ("derive/base", absent.as_path(), fixture.private.as_path()),
        (
            "derive/private-root",
            fixture.base.as_path(),
            absent.as_path(),
        ),
    ] {
        let error = WorkspaceManager::derive(base, private, "01KZSWEEP00000000000000009", "inc-1")
            .expect_err("an absent leaf refuses");
        let message = refusal_of(&error);
        assert!(
            message.contains("not a real directory"),
            "{site}: the refusal must name its reason: {message}"
        );
        assert!(
            message.contains(&absent.display().to_string()),
            "{site}: and the path: {message}"
        );
    }
}

/// `canonical_prefix` peels past **absence** only. A prefix the filesystem
/// refuses to resolve for any other reason is an error, not a component to
/// rejoin lexically: the peel used to discard every `canonicalize` failure,
/// so a link loop under the root produced a "canonical" path the filesystem
/// had never verified. Evaluated on the Unix legs; a loop needs a symbolic
/// link, which the Windows guest's test user cannot create.
#[cfg(unix)]
#[test]
fn canonical_prefix_propagates_a_resolution_failure_that_is_not_absence() {
    let tree = acquire(&std::env::temp_dir(), "canonical-loop").expect("acquire a scratch tree");
    let real = tree.path().canonicalize().expect("canonical scratch");
    let link = real.join("loop");
    std::os::unix::fs::symlink(&link, &link).expect("plant the loop");
    let below = link.join("child");
    let error = canonical_prefix(&below).expect_err("a link loop is not absence");
    assert!(
        matches!(
            &error,
            UpstrokeError::Io { path, source }
                if path == &below && source.kind() != std::io::ErrorKind::NotFound
        ),
        "the error names the path that could not be resolved and its reason: {error}"
    );
}

/// A `..` below a component that does not exist has no directory to refer
/// to. POSIX cannot traverse the absent directory, so the peel meets the
/// `..`, finds no plain component left to peel, and returns that failure
/// naming the prefix it stopped at — rather than the raw path, whose lexical
/// `starts_with` would have answered "inside" for a path the filesystem
/// never resolved. Evaluated on the Unix legs; Win32 resolves the same shape
/// before the filesystem sees it, and the test below pins that answer.
#[cfg(unix)]
#[test]
fn a_dot_dot_below_an_absent_component_is_refused_rather_than_compared_lexically() {
    let tree = acquire(&std::env::temp_dir(), "canonical-dotdot").expect("acquire a scratch tree");
    let real = tree.path().canonicalize().expect("canonical scratch");
    let path = real.join("missing").join("..").join("x");
    let error = canonical_prefix(&path).expect_err("no canonical form exists");
    let stopped_at = real.join("missing").join("..");
    assert!(
        matches!(
            &error,
            UpstrokeError::Io { path, source }
                if path == &stopped_at && source.kind() == std::io::ErrorKind::NotFound
        ),
        "the error names the prefix the peel stopped at: {error}"
    );
}

/// A relative path is anchored at the current directory, so `missing` and
/// `./missing` resolve alike. The peel used to stop at the empty parent and
/// report the first component as a failed prefix, while the same path spelt
/// `./missing` peeled to `.` and resolved — two answers for one path, and a
/// public caller (`quiescence`) failed as I/O on one spelling and reached its
/// ordinary verdict on the other.
#[test]
fn canonical_prefix_anchors_a_relative_path_at_the_current_directory() {
    let first = PathBuf::from(format!("upstroke-absent-{}", std::process::id()));
    let cwd = strip_verbatim(
        std::env::current_dir()
            .expect("current directory")
            .canonicalize()
            .expect("canonical current directory"),
    );
    let expected = cwd.join(&first).join("x");
    assert_eq!(
        canonical_prefix(&first.join("x")).expect("anchored at the current directory"),
        expected
    );
    assert_eq!(
        canonical_prefix(&Path::new(".").join(&first).join("x"))
            .expect("the same path spelt with `.`"),
        expected
    );
}

/// A relative path whose first component exists and whose next does not:
/// the peel finds the existing prefix under the current directory and
/// rejoins the rest, exactly as it does for an absolute path. `src/` exists
/// at the crate root, which is where the suite runs.
#[test]
fn canonical_prefix_resolves_an_existing_relative_prefix_and_rejoins_the_rest() {
    let cwd = strip_verbatim(
        std::env::current_dir()
            .expect("current directory")
            .canonicalize()
            .expect("canonical current directory"),
    );
    let absent = format!("upstroke-absent-{}", std::process::id());
    let path = Path::new("src").join(&absent).join("x");
    assert_eq!(
        canonical_prefix(&path).expect("an existing relative prefix resolves"),
        cwd.join("src").join(&absent).join("x")
    );
}

/// An anchor that names nothing is the same refusal as one that is a file
/// or a link — at the anchor's own check, and at its canonical pin should it
/// vanish between the two, which routes absence through the same predicate.
/// That race cannot be staged from a test; the arm is decided by the rule
/// this drives at the function's edge.
#[test]
fn an_absent_anchor_refuses_as_not_a_real_directory() {
    let fixture = Fixture::new("absent-anchor");
    let anchor = fixture.root.join("never-created");
    let error = refuse_reparse_points(&anchor, &anchor.join("workspaces"), Leaf::Directory)
        .expect_err("an absent anchor refuses");
    let message = refusal_of(&error);
    assert!(
        message.contains("not a real directory"),
        "the refusal must name its reason: {message}"
    );
}

/// A run id is the canonical ULID `DESIGN.md` §15 specifies, as this crate's
/// generator spells it, and nothing else by shape: a lowercase or mixed-case
/// spelling names the same root as its uppercase twin on a case-insensitive
/// filesystem, so two managers of one root cannot be derived through a case
/// variant, and the "manager's own slot" classification cannot be reached
/// through a non-canonical id at all.
#[test]
fn a_run_id_is_the_canonical_ulid_and_a_case_variant_is_refused() {
    let fixture = Fixture::new("run-id-canonical");
    let generated = crate::ulid::ulid();
    WorkspaceManager::derive(&fixture.base, &fixture.private, &generated, "inc-1")
        .expect("a generated id is the canonical spelling");
    let lowered = generated.to_ascii_lowercase();
    let mixed: String = generated
        .chars()
        .enumerate()
        .map(|(i, c)| {
            if i % 2 == 0 {
                c.to_ascii_lowercase()
            } else {
                c
            }
        })
        .collect();
    for run_id in [
        "run1",
        "RUN1",
        lowered.as_str(),
        mixed.as_str(),
        "8ZZZZZZZZZZZZZZZZZZZZZZZZZ",
    ] {
        let error = WorkspaceManager::derive(&fixture.base, &fixture.private, run_id, "inc-1")
            .expect_err("a non-canonical run id refuses");
        let message = refusal_of(&error);
        assert!(
            message.contains("refusing the run id"),
            "{run_id:?}: the refusal must name its reason: {message}"
        );
    }
    assert!(
        !fixture.private.join("workspaces").exists(),
        "and nothing was built for any of them"
    );
}

/// The Windows half of the test above: Win32 resolves `..` lexically before
/// the filesystem sees it, so `missing\..` canonicalizes to the scratch
/// directory and the peel rejoins `x` onto it. Evaluated on the Windows legs.
#[cfg(windows)]
#[test]
fn windows_resolves_a_dot_dot_below_an_absent_component_before_the_filesystem_sees_it() {
    let tree = acquire(&std::env::temp_dir(), "canonical-dotdot").expect("acquire a scratch tree");
    let real = strip_verbatim(tree.path().canonicalize().expect("canonical scratch"));
    let path = real.join("missing").join("..").join("x");
    assert_eq!(
        canonical_prefix(&path).expect("Win32 resolves the `..`"),
        real.join("x")
    );
}

/// The in-funnel check walks down to the directory the effect acts in, not
/// only to the root. A non-recursive `remove_file` on `<root>/intents/<name>`
/// follows a link in its *parent*: an `intents/` exchanged at the `Before`
/// hook for a link to a victim directory holding a file of the same name
/// deleted the victim's file with every check passed. The write direction
/// and the `tasks/` direction are the two tests below.
#[test]
fn an_intents_directory_exchanged_at_the_before_hook_refuses_the_intent_removal() {
    let fixture = Fixture::created("intents-exchanged-remove");
    let slot = fixture.task("alpha", 1);
    fixture
        .manager
        .write_intent(&mut NoHooks, &slot)
        .expect("write the intent");
    let intent = fixture.manager.intent_path(&slot);
    let name = intent
        .file_name()
        .expect("an intent file name")
        .to_os_string();
    let victim = fixture.root.join("victim");
    fs::create_dir_all(&victim).expect("victim directory");
    let victim_file = victim.join(&name);
    fs::write(&victim_file, "not yours\n").expect("victim file");
    let intents = fixture.manager.execution_root().join("intents");
    let mut hooks = ExchangeAtBefore {
        site: slot.remove_intent_site(),
        original: intents.clone(),
        moved: fixture.root.join("intents-moved"),
        victim,
    };

    let error = fixture
        .manager
        .remove_intent(&mut hooks, &slot)
        .expect_err("the check inside the funnel sees the exchanged intents directory");
    let message = refusal_of(&error);
    assert!(
        message.contains("symlink or reparse point")
            && message.contains(&intents.display().to_string()),
        "the refusal must name its reason and the substituted component: {message}"
    );
    assert!(victim_file.exists(), "the victim's file is untouched");
    assert!(
        hooks.moved.join(&name).exists(),
        "and the real intent is still where the hook moved it"
    );
}

/// The write direction: `write_intent` stages and renames inside `intents/`,
/// so an exchanged `intents/` would put the record, and its staging file, in
/// the victim directory.
#[test]
fn an_intents_directory_exchanged_at_the_before_hook_refuses_the_intent_write() {
    let fixture = Fixture::created("intents-exchanged-write");
    let slot = fixture.task("beta", 1);
    let victim = fixture.root.join("victim");
    fs::create_dir_all(&victim).expect("victim directory");
    let intents = fixture.manager.execution_root().join("intents");
    let mut hooks = ExchangeAtBefore {
        site: slot.write_intent_site(),
        original: intents.clone(),
        moved: fixture.root.join("intents-moved"),
        victim: victim.clone(),
    };

    let error = fixture
        .manager
        .write_intent(&mut hooks, &slot)
        .expect_err("the check inside the funnel sees the exchanged intents directory");
    let message = refusal_of(&error);
    assert!(
        message.contains("symlink or reparse point")
            && message.contains(&intents.display().to_string()),
        "the refusal must name its reason and the substituted component: {message}"
    );
    assert_eq!(
        fs::read_dir(&victim).expect("victim listing").count(),
        0,
        "nothing was staged or written in the victim directory"
    );
}

/// The `tasks/` direction: `add_worktree` creates the slot's parent and runs
/// `git worktree add` at the slot path, so an exchanged `tasks/` would put a
/// checkout in the victim directory.
#[test]
fn a_tasks_directory_exchanged_at_the_before_hook_refuses_the_worktree_add() {
    let fixture = Fixture::created("tasks-exchanged-add");
    let slot = fixture.task("gamma", 1);
    fixture
        .manager
        .write_intent(&mut NoHooks, &slot)
        .expect("write the intent");
    let victim = fixture.root.join("victim");
    fs::create_dir_all(&victim).expect("victim directory");
    let tasks = fixture.manager.execution_root().join("tasks");
    let mut hooks = ExchangeAtBefore {
        site: slot.add_site(),
        original: tasks.clone(),
        moved: fixture.root.join("tasks-moved"),
        victim: victim.clone(),
    };

    let error = fixture
        .manager
        .add_worktree(&mut hooks, &slot, &fixture.head)
        .expect_err("the check inside the funnel sees the exchanged tasks directory");
    let message = refusal_of(&error);
    assert!(
        message.contains("symlink or reparse point")
            && message.contains(&tasks.display().to_string()),
        "the refusal must name its reason and the substituted component: {message}"
    );
    assert_eq!(
        fs::read_dir(&victim).expect("victim listing").count(),
        0,
        "no checkout was created in the victim directory"
    );
}

/// A `Before` hook that exchanges one directory under the execution root for
/// a link to `victim`, at one site.
struct ExchangeAtBefore {
    site: EffectSiteId,
    original: PathBuf,
    moved: PathBuf,
    victim: PathBuf,
}

impl EffectHooks for ExchangeAtBefore {
    fn phase(&mut self, site: EffectSiteId, phase: HookPhase) -> Injection {
        if site == self.site && phase == HookPhase::Before {
            fs::rename(&self.original, &self.moved).expect("move the real directory aside");
            plant_directory_link(&self.victim, &self.original);
        }
        Injection::Proceed
    }

    fn refusal_cause(&self) -> Option<String> {
        None
    }
}

/// Every Git command runs with `core.hooksPath` at `<root>/hooks-none`, and
/// the in-funnel chain check walks the effect's target, not that path. A
/// `hooks-none` exchanged at the `Before` hook for a link to an outside
/// directory holding an executable `post-checkout` had `git worktree add`
/// run it. The Git runner now walks the hooks path immediately before every
/// command.
#[test]
fn a_hooks_path_exchanged_at_the_before_hook_refuses_the_worktree_add_and_runs_no_hook() {
    let fixture = Fixture::created("hooks-exchanged");
    let slot = fixture.task("delta", 1);
    fixture
        .manager
        .write_intent(&mut NoHooks, &slot)
        .expect("write the intent");
    let outside = fixture.root.join("outside-hooks");
    fs::create_dir_all(&outside).expect("outside hooks directory");
    let marker = fixture.root.join("hook-ran.marker");
    let script = outside.join("post-checkout");
    fs::write(&script, format!("#!/bin/sh\n: > '{}'\n", marker.display())).expect("hook script");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("executable hook");
    }
    let hooks_dir = fixture.manager.execution_root().join("hooks-none");
    let mut hooks = ExchangeAtBefore {
        site: slot.add_site(),
        original: hooks_dir.clone(),
        moved: fixture.root.join("hooks-moved"),
        victim: outside,
    };

    let error = fixture
        .manager
        .add_worktree(&mut hooks, &slot, &fixture.head)
        .expect_err("the Git runner sees the exchanged hooks path");
    let message = refusal_of(&error);
    assert!(
        message.contains("symlink or reparse point")
            && message.contains(&hooks_dir.display().to_string()),
        "the refusal must name its reason and the substituted component: {message}"
    );
    assert!(!marker.exists(), "the outside hook never ran");
    assert!(
        !fixture.manager.slot_path(&slot).exists(),
        "and no worktree was created"
    );
}

/// `write_intent` staged through a fixed name, `<intent>.tmp`, opened with
/// `File::create`, which follows a link planted there: the record went into
/// the victim file the link named, and the link was renamed into place as
/// the intent. The staging name is now unique per call and opened
/// `create_new`, so a planted name is never followed. A file link needs a
/// symlink, which the Windows guest's test user cannot create.
#[cfg(unix)]
#[test]
fn a_link_planted_at_the_old_staging_name_is_never_followed_by_the_intent_write() {
    let fixture = Fixture::created("staging-planted");
    let slot = fixture.task("epsilon", 1);
    let victim = fixture.root.join("victim.txt");
    fs::write(&victim, "victim bytes\n").expect("victim");
    let intent = fixture.manager.intent_path(&slot);
    let old_staging = intent.with_extension("tmp");
    std::os::unix::fs::symlink(&victim, &old_staging).expect("plant the link at the old name");

    fixture
        .manager
        .write_intent(&mut NoHooks, &slot)
        .expect("the write stages through its own unique name and lands");
    assert_eq!(
        fs::read(&victim).expect("victim"),
        b"victim bytes\n",
        "the victim is untouched"
    );
    let landed = fs::symlink_metadata(&intent).expect("the intent");
    assert!(
        landed.is_file() && landed.len() > 0,
        "the intent landed as a regular file with the record"
    );
    assert!(
        fs::symlink_metadata(&old_staging)
            .expect("the planted link")
            .file_type()
            .is_symlink(),
        "the planted link is still a link, and not what landed"
    );
}

/// A link planted at the intent's own name is a reparse point on a path the
/// write acts through (its rename target), and refuses like a link anywhere
/// else on the chain: the victim it named is untouched and nothing lands. A
/// file link needs a symlink, which the Windows guest's test user cannot
/// create.
#[cfg(unix)]
#[test]
fn a_link_planted_at_the_intent_name_refuses_the_intent_write() {
    let fixture = Fixture::created("intent-name-planted");
    let slot = fixture.task("eta", 1);
    let victim = fixture.root.join("victim.txt");
    fs::write(&victim, "victim bytes\n").expect("victim");
    let intent = fixture.manager.intent_path(&slot);
    std::os::unix::fs::symlink(&victim, &intent).expect("plant the link at the intent's name");

    let error = fixture
        .manager
        .write_intent(&mut NoHooks, &slot)
        .expect_err("a link at the intent's name refuses the write");
    let message = refusal_of(&error);
    assert!(
        message.contains("symlink or reparse point")
            && message.contains(&intent.display().to_string()),
        "the refusal must name its reason and the link: {message}"
    );
    assert_eq!(
        fs::read(&victim).expect("victim"),
        b"victim bytes\n",
        "the victim is untouched"
    );
    assert!(
        fs::symlink_metadata(&intent)
            .expect("the link")
            .file_type()
            .is_symlink(),
        "and the planted link is still a link: nothing landed"
    );
}

/// The durable-intent precondition sat before the funnel, so an intent
/// removed at the `Before` hook still yielded a worktree, one that
/// `reclaim_intents` could never find. The check now runs inside the funnel
/// after the hook, and afterwards `intents()` and `reclaim_intents` agree
/// that there is nothing.
#[test]
fn an_intent_removed_at_the_before_hook_refuses_the_worktree_add() {
    struct RemoveIntentAtBefore {
        site: EffectSiteId,
        intent: PathBuf,
    }

    impl EffectHooks for RemoveIntentAtBefore {
        fn phase(&mut self, site: EffectSiteId, phase: HookPhase) -> Injection {
            if site == self.site && phase == HookPhase::Before {
                fs::remove_file(&self.intent).expect("remove the intent");
            }
            Injection::Proceed
        }

        fn refusal_cause(&self) -> Option<String> {
            None
        }
    }

    let fixture = Fixture::created("intent-removed-in-hook");
    let slot = fixture.task("zeta", 1);
    fixture
        .manager
        .write_intent(&mut NoHooks, &slot)
        .expect("write the intent");
    let mut hooks = RemoveIntentAtBefore {
        site: slot.add_site(),
        intent: fixture.manager.intent_path(&slot),
    };

    let error = fixture
        .manager
        .add_worktree(&mut hooks, &slot, &fixture.head)
        .expect_err("the check inside the funnel sees the removed intent");
    let message = refusal_of(&error);
    assert!(
        message.contains("durable intent"),
        "the refusal must name its reason: {message}"
    );
    assert!(
        !fixture.manager.slot_path(&slot).exists(),
        "and no worktree was created"
    );
    let recorded = fixture.manager.intents().expect("intents").len();
    let reclaimed = fixture
        .manager
        .reclaim_intents(&mut NoHooks)
        .expect("reclaim")
        .slots
        .len();
    assert_eq!(
        (recorded, reclaimed),
        (0, 0),
        "nothing is recorded and nothing is reclaimed, so the two agree"
    );
}

/// One substitution per path the table names for a primitive: a `Before`
/// hook exchanges the path for a link to an outside victim, the primitive
/// must refuse naming the substituted component, and the victim must be
/// untouched. Generated from `every_primitive` and `acted_through_paths`, so
/// every path the table names is driven; the case count is pinned as a
/// regression pin on the table's own size — a path dropped from the table
/// is a case that stops being generated and a number that stops matching —
/// and it is not a proof that the table is complete, which it is not (see
/// `ActedThrough`). A file link needs a symlink, which the Windows guest's
/// test user cannot create, so the file-leaf cases run on the Unix legs and
/// are counted as skipped on Windows.
#[test]
fn every_path_a_primitive_acts_through_refuses_a_link_planted_at_the_before_hook() {
    let mut driven = 0_usize;
    let mut skipped = 0_usize;
    let mut driven_primitives = BTreeSet::new();
    for primitive in every_primitive() {
        let count = SubstitutionCase::new(primitive).paths().len();
        assert!(count > 0, "{primitive:?} acts through nothing?");
        for index in 0..count {
            let case = SubstitutionCase::new(primitive);
            let (_, target, leaf) = case.paths()[index].clone();
            let existing_kind = fs::symlink_metadata(&target).ok().map(|m| m.file_type());
            let file_link = leaf == Leaf::Entry && existing_kind.is_some_and(|k| k.is_file());
            if file_link && cfg!(windows) {
                skipped += 1;
                continue;
            }
            let victim = case.fixture.root.join(format!("victim-{index}"));
            let mut hooks = SubstituteAtBefore {
                site: case.site,
                target: target.clone(),
                moved: target.with_extension("moved-away"),
                victim: victim.clone(),
                file_link,
            };
            // The victim carries a sentinel, and a copy of whatever the
            // primitive would have found through the link (the intent, the
            // registration's `gitdir` and `locked`), so a walk that followed
            // the link would find a plausible tree, not an empty one.
            if file_link {
                fs::write(&victim, b"victim bytes\n").expect("victim file");
            } else {
                fs::create_dir_all(&victim).expect("victim directory");
                fs::write(victim.join("sentinel.txt"), b"sentinel\n").expect("sentinel");
                if existing_kind.is_some_and(|k| k.is_dir()) {
                    copy_files_shallow(&target, &victim);
                }
            }
            let before = snapshot(&victim);

            let error = case.run(&mut hooks).expect_err(&format!(
                "{primitive:?} through a link at {} must refuse",
                target.display()
            ));
            let message = refusal_of(&error);
            assert!(
                message.contains("symlink or reparse point")
                    && message.contains(&target.display().to_string()),
                "{primitive:?}: the refusal must name its reason and the substituted \
                 component {}: {message}",
                target.display()
            );
            assert_eq!(
                snapshot(&victim),
                before,
                "{primitive:?}: the victim behind {} is untouched",
                target.display()
            );
            driven += 1;
            driven_primitives.insert(format!("{primitive:?}"));
        }
    }
    assert_eq!(
        driven_primitives.len(),
        every_primitive().len(),
        "every primitive was driven: {driven_primitives:?}"
    );
    // The pinned count: the table's paths, resolved. Scaffolding is five
    // paths and Registration three, so the sixteen primitives resolve to
    // twelve root cases, four intent, five add, ten Git working directory,
    // five removal, two commit-tree and three ref cases.
    let expected_total = 41;
    assert_eq!(
        driven + skipped,
        expected_total,
        "the table generated {driven} driven and {skipped} skipped cases; a path added to or \
         dropped from `Primitive::acted_through` changes this number"
    );
    if cfg!(unix) {
        assert_eq!(skipped, 0, "every case runs on Unix");
    }
}

/// Every funnel primitive, listed here rather than in production (§12: a
/// test-only item mid-file cuts the effects census's production region) and
/// pinned complete by the exhaustive match below, which stops compiling when
/// a variant is added without a place in the list.
fn every_primitive() -> Vec<Primitive> {
    use Primitive as P;
    let all = vec![
        P::CreateExecutionRoot,
        P::RemoveExecutionRoot,
        P::WriteIntent,
        P::RemoveIntent,
        P::AddWorktree,
        P::VerifyWorktree,
        P::RemoveWorktree,
        P::CandidateStage,
        P::CandidateWriteTree,
        P::ProposalCherryPick,
        P::RepairMaterialize,
        P::SnapshotCommitTree,
        P::CandidateCommitTree,
        P::CreateRef,
        P::CompareAndSwapRef,
        P::DeleteRef,
    ];
    for primitive in &all {
        match primitive {
            P::CreateExecutionRoot
            | P::RemoveExecutionRoot
            | P::WriteIntent
            | P::RemoveIntent
            | P::AddWorktree
            | P::VerifyWorktree
            | P::RemoveWorktree
            | P::CandidateStage
            | P::CandidateWriteTree
            | P::ProposalCherryPick
            | P::RepairMaterialize
            | P::SnapshotCommitTree
            | P::CandidateCommitTree
            | P::CreateRef
            | P::CompareAndSwapRef
            | P::DeleteRef => {}
        }
    }
    assert_eq!(all.len(), 16, "one entry per variant");
    all
}

/// One generated case's fixture, in the state its primitive needs.
struct SubstitutionCase {
    fixture: Fixture,
    primitive: Primitive,
    slot: Option<Slot>,
    registration: Option<PathBuf>,
    site: EffectSiteId,
    refname: String,
    /// The tree of `head`, for the two commit-tree primitives: read here so
    /// that `run` spawns nothing of its own between the plant and the call.
    tree: String,
}

impl SubstitutionCase {
    fn new(primitive: Primitive) -> Self {
        use Primitive as P;
        let created = !matches!(primitive, P::CreateExecutionRoot);
        let fixture = if created {
            Fixture::created("acted-through")
        } else {
            Fixture::new("acted-through")
        };
        let slot = fixture.task("table", 1);
        let refname = "refs/upstroke/test/table".to_owned();
        let mut registration = None;
        match primitive {
            P::RemoveIntent | P::AddWorktree => {
                fixture
                    .manager
                    .write_intent(&mut NoHooks, &slot)
                    .expect("write the intent");
            }
            P::VerifyWorktree
            | P::RemoveWorktree
            | P::CandidateStage
            | P::CandidateWriteTree
            | P::ProposalCherryPick
            | P::RepairMaterialize => {
                fixture.add_task(&mut NoHooks, "table", 1);
                if primitive == P::RemoveWorktree {
                    let path = fixture.manager.slot_path(&slot);
                    registration = fixture
                        .manager
                        .revalidate_removal(&path)
                        .expect("the registration");
                    assert!(registration.is_some(), "the added worktree is registered");
                }
            }
            P::CompareAndSwapRef | P::DeleteRef => {
                fixture
                    .manager
                    .create_ref_zero_old(
                        &mut NoHooks,
                        RefSite::CreateCandidates,
                        &refname,
                        &fixture.head,
                    )
                    .expect("the ref to move or delete");
            }
            P::CreateExecutionRoot
            | P::RemoveExecutionRoot
            | P::WriteIntent
            | P::SnapshotCommitTree
            | P::CandidateCommitTree
            | P::CreateRef => {}
        }
        let site = match primitive {
            P::CreateExecutionRoot => EffectSiteId::Worktree(WorktreeSite::CreateExecutionRoot),
            P::RemoveExecutionRoot => EffectSiteId::Worktree(WorktreeSite::RemoveExecutionRoot),
            P::WriteIntent => slot.write_intent_site(),
            P::RemoveIntent => slot.remove_intent_site(),
            P::AddWorktree => slot.add_site(),
            P::VerifyWorktree => EffectSiteId::Worktree(WorktreeSite::Verify),
            P::RemoveWorktree => slot.remove_site(),
            P::CandidateStage => EffectSiteId::Object(ObjectSite::CandidateStage),
            P::CandidateWriteTree => EffectSiteId::Object(ObjectSite::CandidateWriteTree),
            P::ProposalCherryPick => EffectSiteId::Object(ObjectSite::ProposalCherryPick),
            P::RepairMaterialize => EffectSiteId::Object(ObjectSite::RepairMaterialize),
            P::SnapshotCommitTree => EffectSiteId::Object(ObjectSite::SnapshotCommitTree),
            P::CandidateCommitTree => EffectSiteId::Object(ObjectSite::CandidateCommitTree),
            P::CreateRef => EffectSiteId::Ref(RefSite::CreateCandidates),
            P::CompareAndSwapRef => EffectSiteId::Ref(RefSite::CompareAndSwapIntegration),
            P::DeleteRef => EffectSiteId::Ref(RefSite::DeleteCandidatePin),
        };
        let slot = if matches!(
            primitive,
            P::CreateExecutionRoot
                | P::RemoveExecutionRoot
                | P::SnapshotCommitTree
                | P::CandidateCommitTree
                | P::CreateRef
                | P::CompareAndSwapRef
                | P::DeleteRef
        ) {
            None
        } else {
            Some(slot)
        };
        let tree = git(&fixture.base, &["rev-parse", "HEAD^{tree}"]);
        Self {
            fixture,
            primitive,
            slot,
            registration,
            site,
            refname,
            tree,
        }
    }

    fn paths(&self) -> Vec<(PathBuf, PathBuf, Leaf)> {
        self.fixture
            .manager
            .acted_through_paths(
                self.primitive,
                self.slot.as_ref(),
                self.registration.as_deref(),
            )
            .expect("the table resolves for a case built for it")
    }

    /// The other half of the table for this case.
    fn discovery_paths(&self) -> Vec<(PathBuf, Leaf)> {
        self.fixture
            .manager
            .git_discovery_paths(self.primitive, self.slot.as_ref())
            .expect("the table resolves for a case built for it")
    }

    /// A second slot's checkout, added beside the case's own: the working
    /// directory no primitive's table names. `None` before the execution
    /// root exists, which is the one case that cannot have one.
    fn sibling_checkout(&self) -> Option<PathBuf> {
        if self.primitive == Primitive::CreateExecutionRoot {
            return None;
        }
        let sibling = self.fixture.add_task(&mut NoHooks, "sibling", 1);
        Some(self.fixture.manager.slot_path(&sibling))
    }

    /// Where `candidate` is for this case, or `None` when this case has no
    /// such directory to poison — an unbuilt execution root, or a slot whose
    /// checkout the primitive has not created yet.
    fn candidate_path(&self, candidate: Candidate, sibling: Option<&Path>) -> Option<PathBuf> {
        let manager = &self.fixture.manager;
        let present = |path: PathBuf| path.is_dir().then_some(path);
        match candidate {
            Candidate::ManagedBase => present(self.fixture.base.clone()),
            Candidate::SlotCheckout => present(manager.slot_path(self.slot.as_ref()?)),
            Candidate::SiblingCheckout => present(sibling?.to_path_buf()),
            Candidate::SlotParent => present(manager.execution_root().join("tasks")),
            Candidate::ExecutionRoot => present(manager.execution_root().to_path_buf()),
            Candidate::PrivateRoot => present(self.fixture.private.clone()),
        }
    }

    fn run(&self, hooks: &mut dyn EffectHooks) -> Result<(), UpstrokeError> {
        use Primitive as P;
        let manager = &self.fixture.manager;
        let slot = self.slot.as_ref();
        let head = &self.fixture.head;
        let side = &self.fixture.side;
        let slot_of = || slot.expect("a slot primitive has a slot");
        match self.primitive {
            P::CreateExecutionRoot => manager.create_execution_root(hooks),
            P::RemoveExecutionRoot => manager.remove_execution_root(hooks).map(drop),
            P::WriteIntent => manager.write_intent(hooks, slot_of()),
            P::RemoveIntent => manager.remove_intent(hooks, slot_of()),
            P::AddWorktree => manager.add_worktree(hooks, slot_of(), head).map(drop),
            // The verdict is folded in: a case asks whether the primitive
            // did its work, and a `VerifyFailure` is a report that it could
            // not. The walked cases are unaffected — their refusal is the
            // outer `Err` and arrives before any verdict.
            P::VerifyWorktree => manager
                .verify_worktree(hooks, slot_of(), &Quiescence::AtBase(head.clone()))
                .and_then(|verdict| {
                    verdict.map_err(|failure| UpstrokeError::Git {
                        message: format!("the worktree is not quiescent: {failure:?}"),
                    })
                }),
            P::RemoveWorktree => manager.remove_worktree(hooks, slot_of()),
            P::CandidateStage => manager.candidate_stage(hooks, slot_of(), &[]),
            P::CandidateWriteTree => manager.candidate_write_tree(hooks, slot_of()).map(drop),
            P::ProposalCherryPick => manager
                .proposal_cherry_pick(hooks, slot_of(), side)
                .map(drop),
            P::RepairMaterialize => manager.repair_materialize(hooks, slot_of(), side).map(drop),
            P::SnapshotCommitTree => manager
                .snapshot_commit_tree(hooks, &self.tree, head)
                .map(drop),
            P::CandidateCommitTree => manager
                .candidate_commit_tree(hooks, &self.tree, head, "candidate")
                .map(drop),
            P::CreateRef => {
                manager.create_ref_zero_old(hooks, RefSite::CreateCandidates, &self.refname, head)
            }
            P::CompareAndSwapRef => manager.compare_and_swap_ref(
                hooks,
                RefSite::CompareAndSwapIntegration,
                &self.refname,
                head,
                side,
            ),
            P::DeleteRef => manager.delete_ref_expected_old(
                hooks,
                RefSite::DeleteCandidatePin,
                &self.refname,
                head,
            ),
        }
    }
}

/// The regular files directly inside `from`, copied into `into`.
fn copy_files_shallow(from: &Path, into: &Path) {
    for entry in fs::read_dir(from).expect("list the moved directory") {
        let entry = entry.expect("entry");
        if entry.file_type().expect("type").is_file() {
            fs::copy(entry.path(), into.join(entry.file_name())).expect("copy into the victim");
        }
    }
}

/// The reviewer's P1 sequence, by name: the registration's admin directory
/// captured before the `Before` hook is exchanged for a link to a victim
/// holding copied `gitdir` and `locked` entries; `registration_still_names`
/// used to follow it and `remove_file(admin/locked)` deleted the victim's
/// file. The generated test above covers it too; this is the named witness.
#[test]
fn a_registration_admin_directory_exchanged_at_the_before_hook_refuses_the_worktree_removal() {
    let fixture = Fixture::created("admin-exchanged");
    let slot = fixture.add_task(&mut NoHooks, "alpha", 1);
    let path = fixture.manager.slot_path(&slot);
    let admin = fixture
        .manager
        .revalidate_removal(&path)
        .expect("registration")
        .expect("registered");
    fs::write(admin.join("locked"), b"do not remove\n").expect("the locked marker");
    let victim = fixture.root.join("victim-admin");
    fs::create_dir_all(&victim).expect("victim");
    copy_files_shallow(&admin, &victim);
    let before = snapshot(&victim);
    let mut hooks = SubstituteAtBefore {
        site: slot.remove_site(),
        target: admin.clone(),
        moved: admin.with_extension("moved-away"),
        victim: victim.clone(),
        file_link: false,
    };

    let error = fixture
        .manager
        .remove_worktree(&mut hooks, &slot)
        .expect_err("the walk sees the exchanged admin directory");
    let message = refusal_of(&error);
    assert!(
        message.contains("symlink or reparse point")
            && message.contains(&admin.display().to_string()),
        "the refusal must name its reason and the substituted component: {message}"
    );
    assert_eq!(
        snapshot(&victim),
        before,
        "the victim's `locked` and `gitdir` are untouched"
    );
    assert!(
        path.exists(),
        "and the checkout was not deleted either: every check runs first"
    );
}

// ---------------------------------------------------------------------------
// The other half of the table: Git's repository-discovery paths
// ---------------------------------------------------------------------------

/// Bytes that are neither a Git directory nor a `gitdir:` file. Measured on
/// git 2.43: a child whose working directory holds one of these as its `.git`
/// exits `128` with `fatal: invalid gitfile format: <path>`, so a primitive
/// that runs a child in that directory fails with it and one that runs none
/// there does not notice.
const NOT_A_REPOSITORY: &[u8] = b"not a git repository\n";

/// A second repository holding the same objects as the fixture's base, for a
/// planted discovery link to land in: a clone, so a commit or a ref written
/// through the link names objects the victim really holds.
fn victim_clone(fixture: &Fixture, name: &str) -> PathBuf {
    let victim = fixture.root.join(name);
    let base = fixture.base.to_string_lossy().into_owned();
    let into = victim.to_string_lossy().into_owned();
    git(
        &fixture.root,
        &["clone", "-q", "--no-hardlinks", &base, &into],
    );
    victim
}

/// Every object the repository at `repo` holds, packed or loose.
fn all_objects(repo: &Path) -> String {
    git(
        repo,
        &[
            "cat-file",
            "--batch-all-objects",
            "--batch-check=%(objectname)",
        ],
    )
}

/// The finding's own sequence, executed: `create_ref_zero_old` prechecks
/// against the managed base, its `Before` hook renames `<base>/.git` away and
/// plants `<base>/.git -> <victim>/.git`, the in-funnel walk passes — the base
/// is still a real directory and `hooks-none` is untouched — and
/// `git update-ref` follows the link, creates the ref in the victim and
/// returns success with the managed repository unchanged.
///
/// **This asserts the boundary, not a repair.** It is the measurement behind
/// [`GitWorkingDirectory`]'s trust-boundary statement, and it holds on the
/// code before this pull request exactly as it does after: the walk does not
/// cover `<base>/.git` and this pull request does not make it. The day a
/// change closes the redirect is the day this test fails and the statement on
/// [`GitWorkingDirectory`] stops being true; both move together.
#[test]
#[cfg(unix)]
fn the_ref_funnel_follows_a_git_discovery_link_planted_at_the_before_hook() {
    let fixture = Fixture::created("discovery-redirect");
    let victim = victim_clone(&fixture, "victim-repo");
    let refname = "refs/upstroke/test/redirect".to_owned();
    let dot_git = fixture.base.join(".git");
    let moved = fixture.base.join(".git.moved-away");
    let mut hooks = SubstituteAtBefore {
        site: EffectSiteId::Ref(RefSite::CreateCandidates),
        target: dot_git.clone(),
        moved: moved.clone(),
        victim: victim.join(".git"),
        file_link: false,
    };

    fixture
        .manager
        .create_ref_zero_old(
            &mut hooks,
            RefSite::CreateCandidates,
            &refname,
            &fixture.head,
        )
        .expect("the walk passes: the base is a real directory and hooks-none is untouched");

    // Restore the managed repository to ask it where the ref went.
    fs::remove_file(&dot_git).expect("the planted link");
    fs::rename(&moved, &dot_git).expect("the real git directory back");
    let in_base = git_out(
        &fixture.base,
        &["rev-parse", "--verify", "--quiet", &refname],
    );
    let in_victim = git_out(&victim, &["rev-parse", "--verify", "--quiet", &refname]);
    assert!(
        !in_base.status.success(),
        "the ref must be absent from the managed repository: {}",
        String::from_utf8_lossy(&in_base.stdout).trim()
    );
    assert_eq!(
        String::from_utf8_lossy(&in_victim.stdout).trim(),
        fixture.head,
        "and present in the repository the planted link named"
    );
}

/// The two commit-tree funnels had no [`Primitive`] variant, so no in-funnel
/// walk ran for them: `revalidate` proved the managed base a real directory
/// *before* the `Before` hook and nothing proved it again, and a base
/// exchanged for a link at the hook was followed — the commit landing in the
/// repository the link named. They are in the table now, and the walk they
/// inherit refuses the exchange.
///
/// Witnessed against the unrepaired code: with the `revalidate_acted_through`
/// call removed from `commit_tree`, `snapshot_commit_tree` returns `Ok` and
/// the object it names is one the victim holds and the managed repository
/// does not.
///
/// **The base is named in the spelling the manager holds, not the fixture's.**
/// `derive` stores `canonical_prefix(base)` and the walk refuses `&self.base`,
/// so the canonical spelling is the one the refusal carries, while the
/// fixture's is whatever `std::env::temp_dir()` answered. The two differ
/// wherever the temporary directory is spelled through something
/// canonicalization resolves, and there a comparison with the fixture's
/// spelling fails however right the refusal is. The hosted `windows-latest`
/// lane's `TEMP` is `C:\Users\RUNNER~1\…`, an 8.3 alias, and this test failed
/// on that lane in merge-queue run 36173430383, whose log ends before any
/// panic text; `a_base_derived_from_its_8dot3_spelling_is_refused_under_the_spelling_the_manager_holds`
/// builds that case on a lane whose `TEMP` is spelled long.
#[test]
fn the_commit_tree_funnels_refuse_a_base_exchanged_at_the_before_hook() {
    for (site, label) in [
        (ObjectSite::SnapshotCommitTree, "snapshot"),
        (ObjectSite::CandidateCommitTree, "candidate"),
    ] {
        let fixture = Fixture::created(&format!("commit-tree-base-{label}"));
        let victim = victim_clone(&fixture, "victim-repo");
        let objects_before = all_objects(&victim);

        let error = exchange_the_base_at_a_commit_tree_funnel(
            &fixture,
            &fixture.manager,
            &fixture.base,
            site,
            &victim,
        );
        assert_the_refusal_names_the_held_base(label, &fixture.manager, &error);
        assert_eq!(
            all_objects(&victim),
            objects_before,
            "{label}: no object was written into the repository the link named"
        );
    }
}

/// Drive `site`'s commit-tree funnel through `manager` with the managed base
/// exchanged at the `Before` hook for a link to `victim`, planted at the
/// spelling `base`, and return the refusal.
fn exchange_the_base_at_a_commit_tree_funnel(
    fixture: &Fixture,
    manager: &WorkspaceManager,
    base: &Path,
    site: ObjectSite,
    victim: &Path,
) -> UpstrokeError {
    let tree = git(&fixture.base, &["rev-parse", "HEAD^{tree}"]);
    let mut hooks = SubstituteAtBefore {
        site: EffectSiteId::Object(site),
        target: base.to_path_buf(),
        moved: fixture.root.join("base-moved-away"),
        victim: victim.to_path_buf(),
        file_link: false,
    };
    match site {
        ObjectSite::SnapshotCommitTree => {
            manager.snapshot_commit_tree(&mut hooks, &tree, &fixture.head)
        }
        _ => manager.candidate_commit_tree(&mut hooks, &tree, &fixture.head, "candidate"),
    }
    .expect_err("the in-funnel walk sees the exchanged base")
}

/// The refusal a commit-tree funnel owes a base exchanged at its `Before`
/// hook: its reason, and the base in the spelling `manager` holds — the one
/// the refusal is built from, whatever spelling `derive` was handed.
fn assert_the_refusal_names_the_held_base(
    label: &str,
    manager: &WorkspaceManager,
    error: &UpstrokeError,
) {
    let message = refusal_of(error);
    assert!(
        message.contains("the managed base is not a real directory")
            && message.contains(&manager.base().display().to_string()),
        "{label}: the refusal must name its reason and the base: {message}"
    );
}

/// A base handed to `derive` in its **8.3 spelling** is refused under the
/// canonical spelling the manager holds, and the assertion the test above
/// makes holds for it. On the hosted `windows-latest` lane every fixture under
/// `TEMP` reaches `derive` through an 8.3 alias, because `TEMP` there is
/// `C:\Users\RUNNER~1\…`; this builds the case on purpose, so a lane whose
/// `TEMP` is spelled long runs it too.
///
/// The fixture's base and private root are re-spelled through
/// `GetShortPathNameW`, the call that produces aliases like `RUNNER~1`, and a
/// second manager is derived from the re-spelled pair, as `Fixture` derives
/// its own on that lane. Four assertions, in the order a failure reads:
///
/// 1. **The 8.3 spelling is not the canonical one.** 8.3 name creation is a
///    per-volume setting and is often off, and for a path none of whose
///    components has an 8.3 name `GetShortPathNameW` answers the long
///    spelling. Without this assertion, where that answer is the canonical
///    spelling byte for byte, the test still fails, at assertion 3, but later
///    and with a message that does not point at the volume; and where it
///    differs from the canonical spelling only in ASCII case, assertion 3
///    passes, because its `contains` is case-sensitive, and so does every
///    assertion after it: the test passes with nothing re-spelled but case.
///    So this fails first, naming the spellings, rather than passing or
///    skipping: a skip is counted like a pass in the suite's own totals. It
///    compares ignoring ASCII case, and must: with `!=`, a case-only answer
///    passes it and the test is green without the case it exists for. The
///    comparison is with the canonical spelling and not the fixture's,
///    because where `TEMP` is already an alias, as on the hosted lane, the
///    fixture's spelling is one too and re-spelling it need not change it.
/// 2. **`derive` holds the canonical spelling**, not the 8.3 one it was
///    handed.
/// 3. **The assertion the test above made at `567c4b7f` fails here.** It is
///    the control, with the 8.3 spelling standing where the fixture's stood:
///    the refusal names its reason and does not carry that spelling.
/// 4. **The assertion it makes now passes here**, through the same helper.
#[cfg(windows)]
#[test]
fn a_base_derived_from_its_8dot3_spelling_is_refused_under_the_spelling_the_manager_holds() {
    for (site, label) in [
        (ObjectSite::SnapshotCommitTree, "snapshot"),
        (ObjectSite::CandidateCommitTree, "candidate"),
    ] {
        let fixture = Fixture::created(&format!("commit-tree-8dot3-base-{label}"));
        let canonical = strip_verbatim(fixture.base.canonicalize().expect("canonical base"));
        let base = short_spelling(&fixture.base);
        let private = short_spelling(&fixture.private);
        assert!(
            !base.as_os_str().eq_ignore_ascii_case(canonical.as_os_str()),
            "{label}: GetShortPathNameW answered {} for {}, which is its canonical spelling {} \
             but for ASCII case, so no component of it has an 8.3 alias and the case this test \
             exists for cannot be built on this volume; a pass would pin nothing \
             (`fsutil 8dot3name query <volume>` reports its setting)",
            base.display(),
            fixture.base.display(),
            canonical.display()
        );
        let manager = WorkspaceManager::derive(&base, &private, super::fixture::RUN_ID, "inc-1")
            .expect("derive the manager over the 8.3 spelling");
        assert_eq!(
            manager.base(),
            canonical.as_path(),
            "{label}: handed {}, derive must hold the canonical spelling",
            base.display()
        );

        let victim = victim_clone(&fixture, "victim-repo");
        let objects_before = all_objects(&victim);
        let error =
            exchange_the_base_at_a_commit_tree_funnel(&fixture, &manager, &base, site, &victim);
        let message = refusal_of(&error);
        assert!(
            !(message.contains("the managed base is not a real directory")
                && message.contains(&base.display().to_string())),
            "{label}: the fixture-spelling assertion holds for {}, so the refusal carries the \
             spelling derive was handed and this is not the case the test exists for: {message}",
            base.display()
        );
        assert_the_refusal_names_the_held_base(label, &manager, &error);
        assert_eq!(
            all_objects(&victim),
            objects_before,
            "{label}: no object was written into the repository the link named"
        );
    }
}

/// `path` as `GetShortPathNameW` spells it: each component that has an 8.3
/// alias by that alias, the rest as they are. A path with no alias on it comes
/// back unchanged rather than as an error, which is why the caller checks the
/// answer instead of trusting it.
#[cfg(windows)]
fn short_spelling(path: &Path) -> PathBuf {
    use std::os::windows::ffi::{OsStrExt as _, OsStringExt as _};

    use windows_sys::Win32::Storage::FileSystem::GetShortPathNameW;

    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    // SAFETY: `wide` is a live NUL-terminated UTF-16 path that outlives the
    // call, and a null buffer of length zero is the documented way to ask for
    // the length the answer needs, terminator included.
    let needed = unsafe { GetShortPathNameW(wide.as_ptr(), std::ptr::null_mut(), 0) };
    assert_ne!(
        needed,
        0,
        "GetShortPathNameW could not size the answer for {}: {}",
        path.display(),
        std::io::Error::last_os_error()
    );
    let mut short = vec![0_u16; usize::try_from(needed).expect("a u32 fits a usize")];
    // SAFETY: the same live path, and `short` holds the `needed` writable
    // UTF-16 units the length argument says it does.
    let written = unsafe { GetShortPathNameW(wide.as_ptr(), short.as_mut_ptr(), needed) };
    assert!(
        written != 0 && written < needed,
        "GetShortPathNameW wrote {written} units of a {needed}-unit buffer for {}: {}",
        path.display(),
        std::io::Error::last_os_error()
    );
    short.truncate(usize::try_from(written).expect("a u32 fits a usize"));
    PathBuf::from(std::ffi::OsString::from_wide(&short))
}

/// One substitution per Git repository-discovery path the table names for a
/// primitive, planted at the `Before` hook exactly as the walked cases are.
/// Generated from `every_primitive` and
/// [`WorkspaceManager::git_discovery_paths`], and the assertion is the
/// boundary [`GitWorkingDirectory`] states: the walk does **not** refuse
/// these paths. A path that moves under the walk fails here, and has to move
/// out of this half of the table with it.
///
/// The link is to a clone of the managed repository, so a primitive that
/// follows it fails or succeeds on a real repository rather than on rubble;
/// the assertion is about the refusal, and holds either way. `<slot>/.git` is
/// a file, and a symlink is what replaces a file, so those cases run on the
/// Unix legs and are counted as skipped on Windows.
#[test]
fn every_git_discovery_path_the_table_names_is_outside_the_walk() {
    let mut driven = 0_usize;
    let mut skipped = 0_usize;
    for primitive in every_primitive() {
        let count = SubstitutionCase::new(primitive).discovery_paths().len();
        for index in 0..count {
            let case = SubstitutionCase::new(primitive);
            let (target, _) = case.discovery_paths()[index].clone();
            let existing = fs::symlink_metadata(&target).ok().map(|m| m.file_type());
            let file_link = existing.is_some_and(|kind| kind.is_file());
            if file_link && cfg!(windows) {
                skipped += 1;
                continue;
            }
            let victim = victim_clone(&case.fixture, &format!("victim-{index}"));
            let mut hooks = SubstituteAtBefore {
                site: case.site,
                target: target.clone(),
                moved: case.fixture.root.join(format!("moved-away-{index}")),
                victim: victim.join(".git"),
                file_link,
            };

            if let Err(error) = case.run(&mut hooks) {
                let message = refusal_of(&error);
                assert!(
                    !(message.contains("symlink or reparse point")
                        && message.contains(&target.display().to_string())),
                    "{primitive:?}: the walk refused {}, which the table names as a path it \
                     does not cover: {message}",
                    target.display()
                );
            }
            driven += 1;
        }
    }
    // The pinned count: the managed base resolves to one path, the common
    // git dir, since that is what `<base>/.git` is; a slot checkout resolves
    // to two, its own `.git` and the common git dir. Four primitives name no
    // working directory, seven name the base alone (7), four a slot checkout
    // alone (8), and `VerifyWorktree` names both (2). The five `<slot>/.git`
    // paths are the file links, so they are the Windows skips.
    assert_eq!(
        (driven, skipped),
        if cfg!(windows) { (12, 5) } else { (17, 0) },
        "the table generated {driven} driven and {skipped} skipped cases; a working directory \
         added to or dropped from `Primitive::git_working_dirs` changes this number"
    );
}

/// The omission the table could not see: a primitive that acts through a Git
/// working directory the table does not name.
///
/// One cell per primitive and candidate directory. At the `Before` hook the
/// candidate's `.git` is renamed aside and replaced by [`NOT_A_REPOSITORY`],
/// so a Git child running there exits `128`; the table predicts the outcome
/// and the cell asserts it. A primitive that grows a Git child in a directory
/// `Primitive::git_working_dirs` does not name fails its "still succeeds"
/// cell, and one that stops running a child in a directory the table names
/// fails its "fails" cell.
///
/// **The managed base cannot be probed this way for a primitive that runs a
/// child in a slot checkout**, and those cells are skipped rather than
/// guessed: a linked worktree's git dir *is* `<base>/.git/worktrees/<name>`,
/// so renaming `<base>/.git` aside disables the slot's children too and the
/// probe cannot tell the two apart. Every other cell is driven.
#[test]
fn no_primitive_acts_through_a_git_working_directory_the_table_does_not_name() {
    let mut driven = 0_usize;
    let mut skipped = 0_usize;
    let mut absent = 0_usize;
    for primitive in every_primitive() {
        for candidate in Candidate::ALL {
            let case = SubstitutionCase::new(primitive);
            let sibling = case.sibling_checkout();
            let Some(target) = case.candidate_path(*candidate, sibling.as_deref()) else {
                absent += 1;
                continue;
            };
            let named = candidate.named_by(primitive);
            if *candidate == Candidate::ManagedBase
                && primitive
                    .git_working_dirs()
                    .contains(&GitWorkingDirectory::SlotCheckout)
            {
                skipped += 1;
                continue;
            }
            let mut hooks = PoisonAtBefore {
                site: case.site,
                target: target.clone(),
            };

            let outcome = case.run(&mut hooks);
            if named {
                assert!(
                    outcome.is_err(),
                    "{primitive:?}: the table names {candidate:?} as a working directory of its \
                     Git children, and a child cannot have run in {} with no repository there",
                    target.display()
                );
            } else {
                outcome.unwrap_or_else(|error| {
                    panic!(
                        "{primitive:?}: the table does not name {candidate:?}, so nothing this \
                         primitive does may depend on a repository at {}: {}",
                        target.display(),
                        refusal_of(&error)
                    )
                });
            }
            driven += 1;
        }
    }
    assert_eq!(
        (driven, skipped, absent),
        (78, 5, 13),
        "the matrix is sixteen primitives over {} candidates; a candidate or a working directory \
         added to or dropped from the table changes these numbers",
        Candidate::ALL.len()
    );
}

/// A directory a Git child of some funnel might run in: the two the table
/// names, and the ones it does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Candidate {
    /// [`GitWorkingDirectory::ManagedBase`].
    ManagedBase,
    /// [`GitWorkingDirectory::SlotCheckout`], the case's own slot.
    SlotCheckout,
    /// Another slot's checkout: named by no primitive, and the shape a new
    /// funnel is likeliest to reach for.
    SiblingCheckout,
    /// `tasks/`, the slot's parent.
    SlotParent,
    /// The execution root.
    ExecutionRoot,
    /// The authorized private root.
    PrivateRoot,
}

impl Candidate {
    const ALL: &'static [Self] = &[
        Self::ManagedBase,
        Self::SlotCheckout,
        Self::SiblingCheckout,
        Self::SlotParent,
        Self::ExecutionRoot,
        Self::PrivateRoot,
    ];

    /// Whether `primitive`'s table names this candidate as a working
    /// directory of its Git children.
    fn named_by(self, primitive: Primitive) -> bool {
        let named = primitive.git_working_dirs();
        match self {
            Self::ManagedBase => named.contains(&GitWorkingDirectory::ManagedBase),
            Self::SlotCheckout => named.contains(&GitWorkingDirectory::SlotCheckout),
            Self::SiblingCheckout | Self::SlotParent | Self::ExecutionRoot | Self::PrivateRoot => {
                false
            }
        }
    }
}

/// A `Before` hook that leaves `target` a directory with no repository in it:
/// `.git` renamed aside where there was one, and [`NOT_A_REPOSITORY`] written
/// in its place.
struct PoisonAtBefore {
    site: EffectSiteId,
    target: PathBuf,
}

impl EffectHooks for PoisonAtBefore {
    fn phase(&mut self, site: EffectSiteId, phase: HookPhase) -> Injection {
        if site == self.site && phase == HookPhase::Before {
            let dot_git = self.target.join(".git");
            if fs::symlink_metadata(&dot_git).is_ok() {
                fs::rename(&dot_git, self.target.join(".git.moved-away"))
                    .expect("move the real git directory aside");
            }
            fs::write(&dot_git, NOT_A_REPOSITORY).expect("plant a non-repository");
        }
        Injection::Proceed
    }

    fn refusal_cause(&self) -> Option<String> {
        None
    }
}

/// The reviewer's P2 sequence for the add, by name: `intents/` exchanged for
/// a link to a victim holding a same-named intent file, through which the
/// add's metadata read used to authorise a worktree whose intent lived
/// outside the root.
#[test]
fn an_intents_directory_exchanged_at_the_before_hook_refuses_the_worktree_add() {
    let fixture = Fixture::created("intents-exchanged-add");
    let slot = fixture.task("beta", 1);
    fixture
        .manager
        .write_intent(&mut NoHooks, &slot)
        .expect("write the intent");
    let intents = fixture.manager.execution_root().join("intents");
    let victim = fixture.root.join("victim-intents");
    fs::create_dir_all(&victim).expect("victim");
    copy_files_shallow(&intents, &victim);
    let before = snapshot(&victim);
    let mut hooks = SubstituteAtBefore {
        site: slot.add_site(),
        target: intents.clone(),
        moved: intents.with_extension("moved-away"),
        victim: victim.clone(),
        file_link: false,
    };

    let error = fixture
        .manager
        .add_worktree(&mut hooks, &slot, &fixture.head)
        .expect_err("the walk sees the exchanged intents directory");
    let message = refusal_of(&error);
    assert!(
        message.contains("symlink or reparse point")
            && message.contains(&intents.display().to_string()),
        "the refusal must name its reason and the substituted component: {message}"
    );
    assert_eq!(snapshot(&victim), before, "the victim is untouched");
    assert!(
        !fixture.manager.slot_path(&slot).exists(),
        "and no worktree was created"
    );
}

/// The staging name carries no part of the record's name, so a slot at the
/// old maximum still lands: a 207-byte task key gives a 224-byte intent name,
/// and the previous `.<intent>.<ULID>.tmp` staging name was 256 bytes, one
/// over `NAME_MAX`.
#[test]
fn a_slot_name_at_the_old_maximum_still_lands_its_intent() {
    let fixture = Fixture::created("long-key");
    let key = "a".repeat(207);
    let slot = fixture.task(&key, 1);
    assert_eq!(
        slot.intent_name().len(),
        224,
        "the premise: the intent name is at the old maximum"
    );
    fixture
        .manager
        .write_intent(&mut NoHooks, &slot)
        .expect("a valid slot name at the old maximum lands");
    assert!(fixture.manager.intent_path(&slot).is_file());
    assert_eq!(
        fixture.manager.intents().expect("intents"),
        vec![slot],
        "and the record is listed"
    );
}

/// The §8 staging protocol's recovery rule: a staging file is never an
/// intent, and no filename proves who wrote a file. An orphan a crash left
/// behind used to fail `intents()` forever, which blocked every reclaim;
/// `intents()` ignores it, `reclaim_intents` reports it on its outcome and
/// leaves it in place, and a retry of the write lands beside it. The orphan
/// has the exact shape `write_intent` produces, a real ULID included.
#[test]
fn a_staging_orphan_is_ignored_by_intents_reported_by_reclaim_and_never_removed() {
    let fixture = Fixture::created("staging-orphan");
    let slot = fixture.task("alpha", 1);
    fixture
        .manager
        .write_intent(&mut NoHooks, &slot)
        .expect("a real intent");
    let intents = fixture.manager.execution_root().join("intents");
    let orphan = intents.join(format!(".stage-task-{}.tmp", crate::ulid::ulid()));
    fs::write(&orphan, b"{\"half\":").expect("plant the orphan");

    assert_eq!(
        fixture
            .manager
            .intents()
            .expect("intents ignore the orphan"),
        vec![slot.clone()],
        "the real intent is listed and the orphan is not"
    );
    let reclaimed = fixture
        .manager
        .reclaim_intents(&mut NoHooks)
        .expect("reclaim reports the orphan and removes the rest");
    assert_eq!(reclaimed.slots, vec![slot.clone()]);
    assert_eq!(
        reclaimed.staging_leftovers,
        vec![orphan.clone()],
        "the orphan is reported on the outcome"
    );
    assert!(
        orphan.exists(),
        "and left where it was: no filename proves who wrote it"
    );
    fixture
        .manager
        .write_intent(&mut NoHooks, &slot)
        .expect("a retry of the write lands beside it");
    assert!(fixture.manager.intent_path(&slot).is_file());
    assert!(orphan.exists(), "the retry staged under its own fresh name");
}

/// `hooks-none` must be empty, not only a real link-free directory: a hook
/// written into it runs under every Git command. A `Before` hook writes an
/// executable `post-checkout` into the existing directory; the add refuses
/// naming the entry, and the hook never runs.
#[test]
fn a_hook_written_into_hooks_none_at_the_before_hook_refuses_the_worktree_add_and_never_runs() {
    struct WriteHookAtBefore {
        site: EffectSiteId,
        script: PathBuf,
        marker: PathBuf,
    }

    impl EffectHooks for WriteHookAtBefore {
        fn phase(&mut self, site: EffectSiteId, phase: HookPhase) -> Injection {
            if site == self.site && phase == HookPhase::Before {
                fs::write(
                    &self.script,
                    format!("#!/bin/sh\n: > '{}'\n", self.marker.display()),
                )
                .expect("write the hook");
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt as _;
                    fs::set_permissions(&self.script, fs::Permissions::from_mode(0o755))
                        .expect("executable hook");
                }
            }
            Injection::Proceed
        }

        fn refusal_cause(&self) -> Option<String> {
            None
        }
    }

    let fixture = Fixture::created("hooks-none-written");
    let slot = fixture.task("theta", 1);
    fixture
        .manager
        .write_intent(&mut NoHooks, &slot)
        .expect("write the intent");
    let hooks_dir = fixture.manager.execution_root().join("hooks-none");
    let marker = fixture.root.join("hook-ran.marker");
    let mut hooks = WriteHookAtBefore {
        site: slot.add_site(),
        script: hooks_dir.join("post-checkout"),
        marker: marker.clone(),
    };

    let error = fixture
        .manager
        .add_worktree(&mut hooks, &slot, &fixture.head)
        .expect_err("a hooks path that holds a hook refuses");
    let message = refusal_of(&error);
    assert!(
        message.contains("hook-free") && message.contains("post-checkout"),
        "the refusal must name its reason and the entry: {message}"
    );
    assert!(!marker.exists(), "the hook never ran");
    assert!(
        !fixture.manager.slot_path(&slot).exists(),
        "and no worktree was created"
    );
}

/// `NotFound` from `canonicalize` does not say a component is absent: a link
/// that exists with an absent target answers the same, and the peel used to
/// walk past it and reconstruct the path through a link that is there. The
/// link is read without following, and refuses as a reparse point on the
/// chain. A dangling link needs a symlink, which the Windows guest's test
/// user cannot create.
#[cfg(unix)]
#[test]
fn canonical_prefix_refuses_a_dangling_link_rather_than_peeling_past_it() {
    let tree =
        acquire(&std::env::temp_dir(), "canonical-dangling").expect("acquire a scratch tree");
    let real = strip_verbatim(tree.path().canonicalize().expect("canonical scratch"));
    let link = real.join("link");
    std::os::unix::fs::symlink(real.join("missing"), &link).expect("plant the dangling link");
    let error = canonical_prefix(&link.join("child"))
        .expect_err("a dangling link on the chain is a reparse point, not absence");
    let message = refusal_of(&error);
    assert!(
        message.contains("symlink or reparse point")
            && message.contains(&link.display().to_string()),
        "the refusal must name its reason and the link: {message}"
    );
}

/// Only the exact shape `write_intent` produces is a staging file. A name
/// that merely resembles one is nobody's to hide or delete: `intents()`
/// reports it as the malformed file it is, and `reclaim_intents` leaves it
/// where it is.
#[test]
fn a_name_that_merely_resembles_a_staging_file_is_neither_hidden_nor_removed() {
    let fixture = Fixture::created("staging-lookalike");
    let intents = fixture.manager.execution_root().join("intents");
    let lookalike = intents.join(".stage-report.tmp");
    fs::write(&lookalike, b"someone's report\n").expect("plant the lookalike");

    let listed = fixture
        .manager
        .intents()
        .expect_err("a malformed name in the intent directory is reported");
    assert!(
        refusal_of(&listed).contains("unexpected file"),
        "reported as the malformed file it is: {listed}"
    );
    fixture
        .manager
        .reclaim_intents(&mut NoHooks)
        .expect_err("reclaim stops at the malformed name too");
    assert_eq!(
        fs::read(&lookalike).expect("the lookalike"),
        b"someone's report\n",
        "and it is untouched"
    );
}

/// An orphan of each kind is reported and left in place, whatever its
/// spelling within the generator's range, and no removal site fires for it:
/// deleting it would be cleanup that infers ownership from a filename (§8).
#[test]
fn a_staging_orphan_of_each_kind_is_reported_and_no_removal_site_fires() {
    let fixture = Fixture::created("staging-orphan-kinds");
    let intents = fixture.manager.execution_root().join("intents");
    let mut orphans = Vec::new();
    for kind in ["task", "staging", "snapshot"] {
        let orphan = intents.join(format!(".stage-{kind}-{}.tmp", crate::ulid::ulid()));
        fs::write(&orphan, b"{").expect("plant the orphan");
        orphans.push(orphan);
    }
    orphans.sort();
    let (mut hooks, shared) = harness();

    let reclaimed = fixture
        .manager
        .reclaim_intents(&mut hooks)
        .expect("reclaim reports every orphan");
    assert!(
        reclaimed.slots.is_empty(),
        "no intent was recorded, so none is reclaimed"
    );
    assert_eq!(
        reclaimed.staging_leftovers, orphans,
        "every orphan is reported, in order"
    );
    for orphan in &orphans {
        assert!(orphan.exists(), "{} is still there", orphan.display());
    }
    let observed = shared.lock().expect("harness").coverage().to_vec();
    for site in [
        EffectSiteId::Worktree(WorktreeSite::RemoveIntent),
        EffectSiteId::Worktree(WorktreeSite::RemoveStagingIntent),
        EffectSiteId::Snapshot(SnapshotSite::RemoveIntent),
    ] {
        assert!(
            !observed.iter().any(|seen| seen.site == site),
            "no removal ran under {site} for a leftover nobody can prove they own: {observed:?}"
        );
    }
}

/// `commit_parent` and `commit_tree_sha` used to fold every failure of the
/// command into `None` through `git_ok(..).ok()`, a containment refusal
/// included: an entry appearing in `hooks-none` between the gate's own
/// `worktree list` and the lookup's `rev-parse` made the per-command check
/// refuse, and `.ok()` turned that refusal into "not a commit". The gate
/// reads the hooks path through the same runner, so from the public methods
/// it refuses first; the lookup itself is driven here directly, as the
/// second command in that sequence. Only Git's own quiet "no such object"
/// (exit status 1, nothing on stderr) is `None`; a refusal and a Git failure
/// that speaks both propagate.
#[test]
fn a_hook_planted_in_hooks_none_makes_the_object_lookups_refuse_rather_than_answer_none() {
    let fixture = Fixture::created("lookup-refuses");
    let lookup = |spec: String| {
        [
            OsString::from("rev-parse"),
            OsString::from("--verify"),
            OsString::from("--quiet"),
            OsString::from(spec),
        ]
    };

    // A Git failure that speaks is an error, not absence: peeling a tree to
    // a commit exits 1 with a message.
    let spoken = fixture
        .manager
        .quiet_object_lookup(&lookup(format!("{}^{{tree}}^{{commit}}", fixture.head)))
        .expect_err("a Git failure that speaks propagates");
    assert!(
        matches!(&spoken, UpstrokeError::Git { message } if message.contains("rev-parse")),
        "the error names the command: {spoken}"
    );

    fs::write(
        fixture
            .manager
            .execution_root()
            .join("hooks-none")
            .join("post-checkout"),
        b"#!/bin/sh\nexit 0\n",
    )
    .expect("plant a hook");
    let refused = fixture
        .manager
        .quiet_object_lookup(&lookup(format!("{}^{{commit}}^", fixture.head)))
        .expect_err("the lookup propagates the runner's refusal rather than answering None");
    let message = refusal_of(&refused);
    assert!(
        message.contains("hook-free") && message.contains("post-checkout"),
        "the refusal must name its reason: {message}"
    );
    for (name, result) in [
        (
            "commit_parent",
            fixture.manager.commit_parent(&fixture.head),
        ),
        (
            "commit_tree_sha",
            fixture.manager.commit_tree_sha(&fixture.head),
        ),
    ] {
        let error = result.expect_err(&format!("{name} must refuse, not answer None"));
        let message = refusal_of(&error);
        assert!(
            message.contains("hook-free") && message.contains("post-checkout"),
            "{name}: the refusal must name its reason: {message}"
        );
    }
}

/// A prefix that resolves to a regular file cannot carry components below
/// it, and the filesystem said so; rejoining them lexically would hand back
/// exactly the unverified path `canonical_prefix` exists not to. Reported as
/// `NotADirectory` at the file, on every platform.
#[test]
fn canonical_prefix_refuses_a_prefix_that_is_a_regular_file_with_components_below_it() {
    let tree =
        acquire(&std::env::temp_dir(), "canonical-file-prefix").expect("acquire a scratch tree");
    let real = strip_verbatim(tree.path().canonicalize().expect("canonical scratch"));
    let file = real.join("file");
    fs::write(&file, "not a directory\n").expect("plant the file");
    let error = canonical_prefix(&file.join("child"))
        .expect_err("a regular file cannot carry components below it");
    assert!(
        matches!(
            &error,
            UpstrokeError::Io { path, source }
                if path == &file && source.kind() == std::io::ErrorKind::NotADirectory
        ),
        "the error names the file and says it is not a directory: {error}"
    );
}

/// `add_worktree` read its intent with `is_file()`, which folds every
/// metadata failure into `false`: an `intents/` the process cannot search
/// answered `AddWithoutIntent`, "its durable intent does not exist", for an
/// intent that was there. A read failure is an error naming the intent.
/// Evaluated on the Unix legs, where a mode bit binds a non-root user.
#[cfg(unix)]
#[test]
fn an_intent_that_cannot_be_read_is_an_error_and_not_an_absent_intent() {
    use std::os::unix::fs::PermissionsExt as _;
    let fixture = Fixture::created("intent-unreadable");
    let slot = fixture.task("alpha", 1);
    fixture
        .manager
        .write_intent(&mut NoHooks, &slot)
        .expect("write the intent");
    let intents = fixture.manager.execution_root().join("intents");
    let _restore = RestoreMode {
        path: intents.clone(),
    };
    fs::set_permissions(&intents, fs::Permissions::from_mode(0o000)).expect("make it unsearchable");
    // The injection must bite: root, or a process holding CAP_DAC_OVERRIDE,
    // reads through a mode of 000, and this test would then measure nothing.
    let intent = fixture.manager.intent_path(&slot);
    assert!(
        fs::symlink_metadata(&intent).is_err(),
        "prerequisite not met: the mode bit did not bind (running as root or with \
         CAP_DAC_OVERRIDE); this test needs an unprivileged user"
    );

    let error = fixture
        .manager
        .add_worktree(&mut NoHooks, &slot, &fixture.head)
        .expect_err("an intent that cannot be read is not an intent that is absent");
    assert!(
        matches!(&error, UpstrokeError::Io { path, .. } if path == &intent),
        "the error names the intent it could not read: {error}"
    );
}

/// `remove_execution_root` discarded the result of removing an empty
/// scaffolding directory with `let _ =`, so a scaffolding directory nothing
/// could remove kept the root and the caller learnt only `Ok(false)`. The
/// failure now names the directory. Evaluated on the Unix legs, where a
/// mode bit binds a non-root user.
#[cfg(unix)]
#[test]
fn a_scaffolding_directory_that_cannot_be_removed_is_reported_not_swallowed() {
    use std::os::unix::fs::PermissionsExt as _;
    let fixture = Fixture::created("scaffolding-unremovable");
    let root = fixture.manager.execution_root().to_path_buf();
    let _restore = RestoreMode { path: root.clone() };
    fs::set_permissions(&root, fs::Permissions::from_mode(0o555))
        .expect("make the root unwritable");
    // The injection must bite: root, or a process holding CAP_DAC_OVERRIDE,
    // writes through a mode of 555, the removal would then succeed and the
    // fixture's root would be gone before the assertion.
    let probe = root.join("probe-write");
    if fs::create_dir(&probe).is_ok() {
        fs::remove_dir(&probe).expect("remove the probe");
        panic!(
            "prerequisite not met: the mode bit did not bind (running as root or with \
             CAP_DAC_OVERRIDE); this test needs an unprivileged user"
        );
    }

    let error = fixture
        .manager
        .remove_execution_root(&mut NoHooks)
        .expect_err("a scaffolding directory that cannot be removed is reported");
    assert!(
        matches!(
            &error,
            UpstrokeError::Filesystem { operation: "remove", path, .. }
                if path.starts_with(&root) && path != &root
        ),
        "the error names the removal and the scaffolding directory it could not remove: {error}"
    );
}

/// A **deletion** revalidates the chain, and refuses before it acts
/// (`PR5-WORKSPACE-009`).
///
/// `execution_root`: "every create/reclaim/**delete** revalidates."
/// `remove_execution_root` had three test callers and all three ran against
/// an intact chain, so deleting its `self.revalidate()?;` line changed
/// nothing observable — the create and reclaim thirds of the sentence were
/// covered and the delete third was not. That third is the one where the
/// consequence is a deletion outside the managed tree, so the sentinel here
/// is *outside* the fixture and its bytes are compared after.
#[test]
fn a_deletion_revalidates_the_chain_and_refuses_before_it_deletes() {
    let fixture = Fixture::created("delete-revalidates");
    let sentinel_dir = fixture.root.join("outside");
    fs::create_dir_all(&sentinel_dir).expect("sentinel directory");
    let sentinel = sentinel_dir.join("keepme.txt");
    let sentinel_bytes = b"a file the managed tree has no business deleting";
    fs::write(&sentinel, sentinel_bytes).expect("sentinel");

    // A validated component of the chain becomes a link to the sentinel's
    // directory, after derive already succeeded.
    let base = fixture.manager.base().to_path_buf();
    let moved = fixture.root.join("moved-away");
    fs::rename(&base, &moved).expect("move the real repository aside");
    plant_directory_link(&moved, &base);

    let message = refusal_of(
        &fixture
            .manager
            .remove_execution_root(&mut NoHooks)
            .expect_err("a deletion on a chain that changed refuses"),
    );
    assert!(
        message.contains("not a real directory"),
        "the refusal must name its reason: {message}"
    );
    assert!(
        fixture.manager.execution_root().exists(),
        "and it refused BEFORE acting: the execution root is still there"
    );
    assert_eq!(
        fs::read(&sentinel).expect("sentinel"),
        sentinel_bytes,
        "the sentinel outside the managed tree is byte-identical"
    );
}

/// A **reclaim** revalidates the chain, and refuses before it removes
/// anything.
///
/// `execution_root`: "every create/**reclaim**/delete revalidates." The
/// create third dies at
/// `a_symlink_below_the_private_root_refuses_the_execution_root` and the
/// delete third at the test above. The reclaim third had no fixture that
/// could see it: deleting `reclaim_intents`' own `self.revalidate()?;`
/// left the whole suite green on Linux and on the Windows guest.
///
/// **The shape is what makes it visible, and it is not the obvious one.**
/// `remove_worktree` and `remove_intent` each revalidate on their own
/// before they act, so the outer check is masked the moment there is
/// anything to remove — and every other fixture reaching `reclaim_intents`
/// does so with at least one intent written. The one shape where the outer
/// check is the sole guard is a reclaim over an execution root with **no
/// intents**, where an unguarded version answers `Ok([])` instead of the
/// containment refusal. The otherwise identical fixture that writes one
/// intent first was measured against that same edit and does **not**
/// distinguish it, so the emptiness below is the premise and is asserted
/// rather than assumed.
///
/// The sentinel is *outside* the fixture and its bytes are compared after,
/// for the reason the deletion test gives: what a reclaim through an
/// exchanged ancestor reaches is a removal outside the managed tree.
#[test]
fn a_reclaim_revalidates_the_chain_and_refuses_before_it_removes() {
    let fixture = Fixture::created("reclaim-revalidates");
    let sentinel_dir = fixture.root.join("outside");
    fs::create_dir_all(&sentinel_dir).expect("sentinel directory");
    let sentinel = sentinel_dir.join("keepme.txt");
    let sentinel_bytes = b"a file the managed tree has no business deleting";
    fs::write(&sentinel, sentinel_bytes).expect("sentinel");

    // The premise: nothing to remove, so no inner revalidation can stand in
    // for the outer one. A fixture that grew an intent here would still
    // pass and would stop measuring anything.
    assert!(
        fixture
            .manager
            .intents()
            .expect("the intents of a freshly created root")
            .is_empty(),
        "this fixture is the no-intent reclaim, and only that shape is \
             guarded by `reclaim_intents`' own revalidation"
    );

    // A validated component of the chain becomes a link to the sentinel's
    // directory, after derive already succeeded.
    let base = fixture.manager.base().to_path_buf();
    let moved = fixture.root.join("moved-away");
    fs::rename(&base, &moved).expect("move the real repository aside");
    plant_directory_link(&moved, &base);

    let message = refusal_of(
        &fixture
            .manager
            .reclaim_intents(&mut NoHooks)
            .expect_err("a reclaim on a chain that changed refuses"),
    );
    assert!(
        message.contains("not a real directory"),
        "the refusal must name its reason: {message}"
    );
    assert_eq!(
        fs::read(&sentinel).expect("sentinel"),
        sentinel_bytes,
        "the sentinel outside the managed tree is byte-identical"
    );
}

#[test]
fn a_root_inside_a_repository_worktree_refuses() {
    let fixture = Fixture::new("root-inside");
    // The private root *is* the repository checkout: the execution root
    // would then live inside a worktree of the repository it manages.
    let error = WorkspaceManager::derive(
        &fixture.base,
        &fixture.base,
        "01KZSWEEP00000000000000003",
        "inc-1",
    )
    .expect_err("a root inside a repository worktree refuses");
    let message = refusal_of(&error);
    assert!(
        message.contains("inside the repository worktree"),
        "the refusal must name its reason: {message}"
    );
}

#[test]
fn a_foreign_repository_worktree_inside_the_root_refuses() {
    let fixture = Fixture::created("worktree-inside");
    let intruder = fixture.manager.execution_root().join("intruder");
    git(
        &fixture.base,
        &[
            "worktree",
            "add",
            "-q",
            "--detach",
            &intruder.to_string_lossy(),
            &fixture.head,
        ],
    );
    let error = fixture
        .manager
        .revalidate()
        .expect_err("a foreign worktree inside the root refuses");
    let message = refusal_of(&error);
    assert!(
        message.contains("is inside it"),
        "the refusal must name its reason: {message}"
    );

    // And the manager's own slots are not foreign, which is the half a
    // literal reading of the sentence would get wrong.
    git(
        &fixture.base,
        &["worktree", "remove", "--force", &intruder.to_string_lossy()],
    );
    let slot = fixture.add_task(&mut NoHooks, "alpha", 1);
    fixture
        .manager
        .revalidate()
        .expect("the manager's own worktree is not a foreign one");
    fixture
        .manager
        .remove_worktree(&mut NoHooks, &slot)
        .expect("remove");
}

#[test]
fn nothing_outside_the_execution_root_is_ever_deleted() {
    let fixture = Fixture::created("containment");
    let outside = fixture.root.join("precious");
    fs::create_dir_all(&outside).expect("outside directory");
    fs::write(outside.join("keep.txt"), "keep\n").expect("outside file");

    let error = fixture
        .manager
        .contained(&outside)
        .expect_err("a path outside the root refuses");
    let message = refusal_of(&error);
    assert!(
        message.contains("outside the execution root"),
        "the refusal must name its reason: {message}"
    );
    assert!(outside.join("keep.txt").exists(), "and delete nothing");

    // The root itself is not inside itself: a removal that accepted it
    // would delete the whole root through a per-slot primitive.
    assert!(
        fixture
            .manager
            .contained(fixture.manager.execution_root())
            .is_err(),
        "the root is not a contained target of a slot removal"
    );
    fixture
        .manager
        .contained(&fixture.manager.execution_root().join("tasks").join("kx-g1"))
        .expect("a slot path is contained");
}

// `a_slot_name_that_could_escape_the_root_refuses` stood here and is gone: its
// six hostile names are a strict subset of `HOSTILE_SLOT_NAMES` below and it
// drove one primitive of the eleven, which is the `bounded_grid` shape the
// grid test was written to replace. What it carried that the grid did not is
// the *legal* name — and that half is not redundant at all, so it has moved
// into the grid and now runs for every primitive rather than for
// `write_intent` alone.

/// The hostile slot names, one per **mechanism** by which a name escapes
/// containment or changes a command's meaning.
///
/// Kept as a table with its mechanism named so that hostility is a
/// distinct-value count rather than a claim in prose: two entries that
/// escape the same way are one test, and the count below is asserted.
const HOSTILE_SLOT_NAMES: &[(&str, &str)] = &[
    ("..", "the parent directory itself"),
    ("../escape", "traversal through a separator"),
    ("a/b", "a POSIX separator, so the name is two components"),
    ("a\\b", "a Windows separator, which POSIX-only checks miss"),
    (
        "-force",
        "a leading dash the Git commands would read as an option",
    ),
    ("", "empty, which collapses the path component away"),
    ("naïve", "non-ASCII, whose NFC/NFD forms are two names"),
    (
        ".",
        "the current directory, which aliases the namespace root",
    ),
];

/// Every public primitive **this scan can see** that turns a `&Slot` into a
/// path refuses a hostile name.
///
/// `a_slot_name_that_could_escape_the_root_refuses` exercises exactly one
/// primitive, `write_intent`. That is the `bounded_grid` failure this
/// project has recorded three times: the grid varies the hostile name and
/// holds the primitive fixed, so it stays green while
/// `candidate_stage`, `candidate_write_tree`, `proposal_cherry_pick`,
/// `repair_materialize` and `changed_paths` run `git add -A`,
/// `git write-tree`, `git cherry-pick` and `git diff` with a working
/// directory the name placed outside the execution root. `Slot`'s fields
/// are `pub`, so the name is caller data at every one of those entry
/// points.
///
/// **What the scan is, and what it cannot see** (PR #136 pass 3, finding 2;
/// noticed by this sweep at its start and not acted on, which is the honest
/// provenance). The set comes from matching two exact spellings in this
/// module's source — `slot: &Slot` and `-> Result<` — so it is a tripwire and
/// not a derivation. A primitive spelt `target: &Slot`, or one returning
/// `std::result::Result<…>`, is **invisible to it**: the derived set, this
/// grid and the scan's own positive control all stay green while that
/// primitive turns caller data into a path with nothing refusing for it. The
/// `seen_fns > 30` check below is a floor that says the scan read something,
/// not a boundary that says it read everything, and §12 asks for the second.
///
/// So the claim is: *adding a primitive spelt the way the existing eleven are
/// spelt, without an arm here, fails this test by name.* Adding one spelt
/// otherwise does not, and no third census is being written to chase it — two
/// text censuses in this file have now been walked around three times between
/// them. The row records both invisible spellings.
#[test]
fn every_slot_taking_primitive_refuses_a_hostile_slot_name() {
    let fixture = Fixture::created("slot-grid");
    let manager = &fixture.manager;
    let head = fixture.head.clone();

    type Call<'a> = Box<dyn Fn(&Slot) -> Result<(), UpstrokeError> + 'a>;
    let primitives: Vec<(&str, Call<'_>)> = vec![
        (
            "write_intent",
            Box::new(|slot| manager.write_intent(&mut NoHooks, slot)),
        ),
        (
            "remove_intent",
            Box::new(|slot| manager.remove_intent(&mut NoHooks, slot)),
        ),
        (
            "add_worktree",
            Box::new(|slot| manager.add_worktree(&mut NoHooks, slot, &head).map(drop)),
        ),
        (
            "verify_worktree",
            Box::new(|slot| {
                manager
                    .verify_worktree(&mut NoHooks, slot, &Quiescence::AtBase(head.clone()))
                    .map(drop)
            }),
        ),
        (
            "remove_worktree",
            Box::new(|slot| manager.remove_worktree(&mut NoHooks, slot)),
        ),
        (
            "remove_worktree_proving",
            Box::new(|slot| {
                manager
                    .remove_worktree_proving(&mut NoHooks, slot, WriterProof::NoWriterAlive)
                    .map(drop)
            }),
        ),
        (
            "candidate_stage",
            Box::new(|slot| manager.candidate_stage(&mut NoHooks, slot, &[])),
        ),
        (
            "candidate_write_tree",
            Box::new(|slot| manager.candidate_write_tree(&mut NoHooks, slot).map(drop)),
        ),
        (
            "proposal_cherry_pick",
            Box::new(|slot| {
                manager
                    .proposal_cherry_pick(&mut NoHooks, slot, &fixture.side)
                    .map(drop)
            }),
        ),
        (
            "repair_materialize",
            Box::new(|slot| {
                manager
                    .repair_materialize(&mut NoHooks, slot, &fixture.side)
                    .map(drop)
            }),
        ),
        (
            "changed_paths",
            Box::new(|slot| manager.changed_paths(slot, &head).map(drop)),
        ),
        (
            "changed_paths_pausing",
            Box::new(|slot| {
                manager
                    .changed_paths_pausing(&mut NoHooks, slot, &head)
                    .map(drop)
            }),
        ),
        (
            "candidate_diff",
            Box::new(|slot| manager.candidate_diff(slot, &head, &head).map(drop)),
        ),
        (
            "proposal_state",
            Box::new(|slot| manager.proposal_state(slot, &head).map(drop)),
        ),
        (
            "proposal_state_pausing",
            Box::new(|slot| {
                manager
                    .proposal_state_pausing(&mut NoHooks, slot, &head)
                    .map(drop)
            }),
        ),
        (
            "unresolved_conflicts",
            Box::new(|slot| manager.unresolved_conflicts(slot).map(drop)),
        ),
        (
            "resolution_manifest",
            Box::new(|slot| manager.resolution_manifest(slot).map(drop)),
        ),
        (
            "resolved_conflicts",
            Box::new(|slot| manager.resolved_conflicts(slot).map(drop)),
        ),
        (
            "kept_instances",
            Box::new(|slot| manager.kept_instances(&mut NoHooks, slot).map(drop)),
        ),
    ];

    let covered: BTreeSet<String> = primitives
        .iter()
        .map(|(name, _)| (*name).to_owned())
        .collect();
    assert_eq!(
        covered.len(),
        primitives.len(),
        "each primitive appears once"
    );
    assert_eq!(
        covered,
        slot_taking_fallible_primitives(),
        "the grid must be this module's slot-taking fallible `pub fn`s, derived from its \
             signatures — a primitive with no arm here is one nothing refuses for"
    );

    let mechanisms: BTreeSet<&str> = HOSTILE_SLOT_NAMES.iter().map(|(_, why)| *why).collect();
    assert_eq!(
        mechanisms.len(),
        HOSTILE_SLOT_NAMES.len(),
        "every hostile name is a distinct escape mechanism, not a restatement"
    );
    assert_eq!(mechanisms.len(), 8, "eight distinct mechanisms");

    // Something outside the root that a successful escape would reach, so
    // "it refused" is not the only thing asserted.
    let outside = fixture.root.join("precious");
    fs::create_dir_all(&outside).expect("outside directory");
    fs::write(outside.join("keep.txt"), "keep\n").expect("outside file");

    // The legal name, driven through every primitive **before** the hostile
    // ones, because a primitive that refuses every slot name satisfies every
    // assertion below and this is the only thing that says otherwise. The
    // grid as it stood asserted refusals and nothing else, and the one test
    // that carried a legal-name control drove `write_intent` alone. Measured:
    // with `candidate_diff` returning a slot-name refusal for every call,
    // **this module's whole suite passed** — the primitive has no other
    // success-path driver here, and what caught it was a driver test two
    // modules away, which is not where a claim about this API belongs (§12,
    // the lowest layer that can observe the failure).
    //
    // A primitive may still fail here for its own reasons — `add_worktree`
    // has no durable intent, `proposal_cherry_pick` has no worktree — so what
    // is asserted is the *reason*: whatever else goes wrong, a legal name is
    // never refused as a slot name.
    let legal = Slot::Task {
        key: "ok_key-1".to_owned(),
        generation: 1,
    };
    for (name, call) in &primitives {
        if let Err(error) = call(&legal) {
            let message = refusal_of(&error);
            assert!(
                !message.contains("slot name"),
                "`{name}` refused the legal slot name `ok_key-1` as a slot name, so its \
                 refusals below say nothing about hostility: {message}"
            );
        }
    }

    for (name, call) in &primitives {
        for (hostile, why) in HOSTILE_SLOT_NAMES {
            let slot = Slot::Task {
                key: (*hostile).to_owned(),
                generation: 1,
            };
            let Err(error) = call(&slot) else {
                panic!("`{name}` accepted the slot name `{hostile}` ({why})")
            };
            let message = refusal_of(&error);
            assert!(
                message.contains("slot name"),
                "`{name}` must refuse `{hostile}` by naming the slot name: {message}"
            );
        }
    }

    assert_eq!(
        fs::read_to_string(outside.join("keep.txt")).expect("still there"),
        "keep\n",
        "and nothing outside the execution root was touched"
    );
}

/// The names of this module's `pub fn`s **spelt** `slot: &Slot` and
/// `-> Result<` — matched in the source rather than listed, and matched by
/// spelling rather than derived, which is the limit
/// `every_slot_taking_primitive_refuses_a_hostile_slot_name` states in full.
///
/// `slot_path` and `intent_path` are deliberately not in this set: they
/// return a `PathBuf` infallibly and are path arithmetic, which is why the
/// predicate is "returns a `Result`" rather than "mentions a `Slot`".
fn slot_taking_fallible_primitives() -> BTreeSet<String> {
    let source = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/workspace_manager.rs"),
    )
    .expect("read this module's source");
    let (names, seen_fns) = slot_taking_fallible_primitives_in(&source);
    // The domain, asserted rather than assumed: the scan read this module's
    // signatures and not nothing.
    assert!(
        seen_fns > 30,
        "the scan read this module's signatures rather than nothing: {seen_fns}"
    );
    names
}

/// The scan itself, over text rather than over a path, so
/// [`the_slot_primitive_scan_reads_code_and_never_prose`] can inject a
/// violation into it. Returns the names and how many `pub fn`s were examined.
fn slot_taking_fallible_primitives_in(source: &str) -> (BTreeSet<String>, usize) {
    // **Comments and string literals blanked first** (§12: "a census of Rust
    // structure blanks comments and string … literals before counting"; and
    // `PR4-CENSUS-COMMENT-ORACLE`, the standing row for a census that counted
    // a doc comment). Every needle here is code — `pub fn`, `slot: &Slot`,
    // `-> Result<` — so a signature written inside a block comment or a raw
    // string is prose and not a method of this module. The blanker is
    // `crate::effects`', the one in the tree, which carries its own proof;
    // this census does not write a second (`PR5D-VISIBILITY-CHECK-DUPLICATED`
    // is the standing row for a parser written twice).
    let blanked = crate::effects::blank_comments_and_strings(source);
    let mut names = BTreeSet::new();
    let mut seen_fns = 0_usize;
    for chunk in blanked.split("\n    pub fn ").skip(1) {
        seen_fns += 1;
        let Some((signature, _)) = chunk.split_once('{') else {
            continue;
        };
        let Some(name) = signature.split(['(', '<']).next() else {
            continue;
        };
        if signature.contains("slot: &Slot") && signature.contains("-> Result<") {
            names.insert(name.to_owned());
        }
    }
    (names, seen_fns)
}

/// The census's own positive control (§12: "every census … carries a positive
/// control that injects one violation and sees the expected failure").
///
/// The violation is the thing the grid above exists to catch: a slot-taking
/// fallible `pub fn` that no arm names. The two negative cases are the two
/// shapes that reach the split needle without being code — a block comment
/// and a raw string literal, both of which can carry a real newline followed
/// by four spaces — and they are why the scan blanks before counting.
#[test]
fn the_slot_primitive_scan_reads_code_and_never_prose() {
    let code = "\n    pub fn newly_added(&self, slot: &Slot) -> Result<(), UpstrokeError> {\n        Ok(())\n    }\n";

    let (found, seen) = slot_taking_fallible_primitives_in(code);
    assert_eq!(seen, 1, "the scan examined the injected signature");
    assert!(
        found.contains("newly_added"),
        "the injected violation must be reported, or this census cannot fail: {found:?}"
    );

    for (what, prose) in [
        ("a block comment", format!("/*{code}*/\n")),
        (
            "a raw string literal",
            format!("const S: &str = r\"{code}\";\n"),
        ),
    ] {
        let (found, seen) = slot_taking_fallible_primitives_in(&prose);
        assert_eq!(
            (found.len(), seen),
            (0, 0),
            "{what} is prose, not a method of this module: {found:?}"
        );
    }
}

/// `slice_contract.invariants_introduced[1]`: "worktree and snapshot
/// intents **synced before** the add".
///
/// `WriteIntent` and `Add` are separate sites — each carries its own hooks
/// and the cancellation clause is stated per clause — so no single funnel
/// body can order them. That makes the ordering a *caller's* obligation,
/// and an unchecked obligation is one the first schema-4 caller in PR7–PR10
/// can drop silently: the add succeeds, and the worktree it created is
/// invisible to `reclaim_intents`, which walks intents. The second half of
/// this test is the consequence the refusal exists to prevent.
#[test]
fn an_add_without_a_durable_intent_refuses_and_leaves_nothing_registered() {
    let fixture = Fixture::created("add-without-intent");
    let slots = [
        fixture.task("alpha", 1),
        Slot::Staging { sequence: 7 },
        Slot::Snapshot {
            name: SnapshotName::gates(0, 1, 1),
        },
    ];
    assert_eq!(
        slots
            .iter()
            .map(Slot::add_site)
            .collect::<BTreeSet<_>>()
            .len(),
        3,
        "one slot per add site: Worktree.Add, Worktree.AddStaging, Snapshot.Add"
    );

    for slot in &slots {
        let path = fixture.manager.slot_path(slot);
        let error = fixture
            .manager
            .add_worktree(&mut NoHooks, slot, &fixture.head)
            .expect_err("an add with no durable intent refuses");
        let message = refusal_of(&error);
        assert!(
            message.contains("durable intent"),
            "the refusal must name its reason: {message}"
        );
        assert!(!path.exists(), "and no worktree directory was created");
        assert!(
            fixture
                .manager
                .worktree_records()
                .expect("records")
                .iter()
                .all(|record| record.path() != path),
            "and nothing was registered with Git either"
        );

        // The same add, with the intent durable, succeeds — so the refusal
        // is about the ordering and not about the slot.
        fixture
            .manager
            .write_intent(&mut NoHooks, slot)
            .expect("intent");
        fixture
            .manager
            .add_worktree(&mut NoHooks, slot, &fixture.head)
            .expect("the same add once the intent is durable");
        assert!(path.exists());
    }

    // And the reason: every worktree the manager created is reachable from
    // an intent, so reclaim finds all three.
    let reclaimed = fixture
        .manager
        .reclaim_intents(&mut NoHooks)
        .expect("reclaim");
    assert_eq!(
        reclaimed.slots.iter().collect::<BTreeSet<_>>(),
        slots.iter().collect::<BTreeSet<_>>(),
        "reclaim walks intents, so an add without one would leave a worktree it never sees"
    );
    for slot in &slots {
        assert!(!fixture.manager.slot_path(slot).exists());
    }
}
