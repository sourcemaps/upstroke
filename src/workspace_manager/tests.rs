// Allowlist placement: the **funnel section** of `effects/allowlist.toml`, by
// attachment to `src/workspace_manager.rs` -- the shape
// `src/runner/container/tests.rs` established for a funnel's own test module.
// This suite drives the site-taking APIs and plants the residue they are meant
// to find, so it names `fs::write`, `fs::create_dir_all`,
// `std::process::Command` and `println!` directly.
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

#[cfg(test)]
mod containment;
#[cfg(test)]
mod guarded_reads;
#[cfg(test)]
mod incarnations;
#[cfg(test)]
mod reclaim;
#[cfg(test)]
mod refs;
#[cfg(test)]
mod registry;
#[cfg(test)]
mod residue;
#[cfg(test)]
mod snapshots;

// The `m4-workspace` split moved the path-hygiene predicates into a child, and
// `use super::*` reaches the parent's namespace only. `is_reparse_point` is the
// one item this suite drives that the parent does not call itself -- both
// platform arms are asserted here directly -- so it is named through the
// child's own `pub(super)` surface. No test is renamed, no assertion changes
// and no body moves; this line is the whole of what the extraction owes this
// file.
use super::containment::is_reparse_point;

use std::collections::BTreeSet;

// The repository fixture and the three Git helpers are `fixture`'s, not
// this module's: `src/engine/topology/**` needs them too and cannot reach
// an effect primitive of its own. See that module for why they moved.
use super::fixture::{
    Fixture, REPLACEMENT_DISABLED_EXIT, REPLACEMENT_WITNESS, ReplacementLiveness,
    ambient_replacement_controls, as_git_writes_it, assert_replacement_controls_pinned, create_dir,
    died_by_abort, died_by_kill, environment_without_ambient_replacement_controls,
    fan_out_directory, git, git_os, git_out, replacement_liveness, run_kill_child,
    run_kill_child_within, run_replacement_witness_child, scratch, tear_registration,
    without_ambient_replacement_controls, write_file, write_include_path,
};

/// `value`, which the fixture read from Git, as the [`ObjectId`] every
/// snapshot input is built from.
pub(super) fn oid(value: &str) -> ObjectId {
    ObjectId::new(value.to_owned()).expect("an id the fixture read from Git")
}

// The observing scratch tree: its drop reclaims the directory and reports a
// reclaim that failed, where `scratch` above hands back a bare path nothing
// removes. The `canonical_prefix` tests below build no repository, so this is
// the fixture they take.
use crate::rundir::scratch_tree::acquire;

// The slot-component grammar, named so the run-id restatement of it can be
// held to the same verdicts.
// Named here rather than borrowed from the parent's import list. The split
// moved the items that needed them into children -- the hook observers, the
// residue classifier, the slot vocabulary and the changed-path decoder -- so
// `src/workspace_manager.rs` no longer imports these and `use super::*` no
// longer carries them. Same names, same crate paths, no new dependency.
//
// `OsStr` is named here for the same reason as the rest -- the root pruned
// `use std::ffi::{OsStr, OsString};` to `OsString`. It carried a `#[cfg(unix)]`
// while its only call site was inside a `#[cfg(unix)]` test: on Windows the
// item was compiled out and an ungated import is an `unused_imports` error
// under the Windows legs' `-D warnings`. `hostile_replacement_environments` names it
// on both platforms, so the gate is gone -- and it was the gate that broke the
// three Windows legs of CI while ubuntu and macos were green, which is the one
// thing this box cannot measure for itself.
use std::ffi::OsStr;

use std::sync::{Arc, Mutex};

use crate::topology::effects::{
    ClassHistogram, Evidence, EvidenceLabel, HookHarness, Injection, InjectionMode, ObjectResidue,
    ResidueElement, ResourceRow, SamplingRecord, SnapshotSite, SyntheticRecord,
};

use crate::topology::paths::GitPath;

// The replacement-isolation witnesses run a **role process** over a snapshot,
// which is the half `command`'s own isolation never covered. They need the
// production runner and the production request builder, not a `Command` of
// their own: what is being asserted is that `HostEnvironment::compose` puts the
// pair in the vector `HostRunner::run` installs after `env_clear`.
use crate::agent::ProcessOutput;

use crate::runner::host::{HostEnvironment, HostRunner, KeyCase, ObjectGraph};

use crate::runner::invocation::{AttemptRole, InvocationId};

use crate::runner::{CommandSpec, Runner};

use crate::topology::events::{AttemptNumber, GenerationId};

use crate::topology::registry::TaskKey;

/// A harness that answers `Proceed` and records everything.
pub(super) fn harness() -> (HarnessEffects, Arc<Mutex<HookHarness>>) {
    let shared = Arc::new(Mutex::new(HookHarness::new()));
    (HarnessEffects::new(Arc::clone(&shared)), shared)
}

pub(super) fn refusal_of(error: &UpstrokeError) -> String {
    error.to_string()
}

/// Run `body`, returning its panic message if it panicked.
///
/// **The panic hook is deliberately left alone.** An earlier version took it,
/// installed a no-op and restored it afterwards, to keep intentional panics
/// out of the test output. The hook is process-global and these tests run in
/// parallel, so two of them interleaving —
/// `A` takes `H` installs `a`; `B` takes `a` installs `b`; `A` restores `H`;
/// `B` restores `a` — leaves the process running with a no-op hook for good,
/// and every later panic anywhere in the suite loses its message and
/// backtrace. Tidy output is not worth that: the noise below is a few lines,
/// and the alternative silently disarms the diagnostics of every other test.
pub(super) fn panic_message(body: impl FnOnce()) -> Option<String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(body))
        .err()
        .map(|payload| {
            payload
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_owned()))
                .unwrap_or_else(|| "<non-string panic payload>".to_owned())
        })
}

/// A directory reparse point at `link` naming `target`, on either platform.
///
/// A POSIX symlink and a Windows **junction** are the two shapes
/// `expected_failures_refusals[0]` names ("symlink/junction on the chain"),
/// and they are the two an operator can actually build: a Windows
/// *symbolic* link needs a privilege the guest's test user does not have,
/// while `mklink /J` needs none.
pub(super) fn plant_directory_link(target: &Path, link: &Path) {
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link).expect("plant the symlink");
    #[cfg(windows)]
    {
        let made = Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()
            .expect("run mklink");
        assert!(
            made.status.success(),
            "the fixture must really create a junction: {}",
            String::from_utf8_lossy(&made.stderr)
        );
    }
    let metadata = fs::symlink_metadata(link).expect("link metadata");
    // The premise, asserted rather than assumed — and it is exactly the
    // difference between the two calls this test exists to keep apart.
    assert!(
        is_reparse_point(&metadata),
        "the fixture must really be a reparse point"
    );
    assert!(
        fs::metadata(link).expect("target metadata").is_dir(),
        "and it must resolve to a real directory, or a check that followed \
             it would refuse for the wrong reason"
    );
}

/// A `Before` hook that exchanges `target` for a link to `victim`: a
/// directory link (a junction on Windows), or a file symlink on Unix.
pub(super) struct SubstituteAtBefore {
    site: EffectSiteId,
    target: PathBuf,
    moved: PathBuf,
    victim: PathBuf,
    file_link: bool,
}

impl EffectHooks for SubstituteAtBefore {
    fn phase(&mut self, site: EffectSiteId, phase: HookPhase) -> Injection {
        if site == self.site && phase == HookPhase::Before {
            if fs::symlink_metadata(&self.target).is_ok() {
                fs::rename(&self.target, &self.moved).expect("move the real path aside");
            } else if let Some(parent) = self.target.parent() {
                fs::create_dir_all(parent).expect("the substituted path's parent");
            }
            if self.file_link {
                #[cfg(unix)]
                std::os::unix::fs::symlink(&self.victim, &self.target)
                    .expect("plant the file link");
                #[cfg(windows)]
                panic!("file-link cases are skipped on Windows");
            } else {
                plant_directory_link(&self.victim, &self.target);
            }
        }
        Injection::Proceed
    }

    fn refusal_cause(&self) -> Option<String> {
        None
    }
}

/// A victim's state: its bytes if a file, otherwise every entry's name and
/// kind one level down and every file's bytes.
pub(super) fn snapshot(victim: &Path) -> Vec<(String, Vec<u8>)> {
    let metadata = fs::symlink_metadata(victim).expect("the victim exists");
    if metadata.is_file() {
        return vec![("<file>".to_owned(), fs::read(victim).expect("victim bytes"))];
    }
    let mut entries = Vec::new();
    for entry in fs::read_dir(victim).expect("list the victim") {
        let entry = entry.expect("entry");
        let name = entry.file_name().to_string_lossy().into_owned();
        let kind = entry.file_type().expect("type");
        let bytes = if kind.is_file() {
            fs::read(entry.path()).expect("file bytes")
        } else if kind.is_dir() {
            b"<dir>".to_vec()
        } else {
            b"<link>".to_vec()
        };
        entries.push((name, bytes));
    }
    entries.sort();
    entries
}

/// Restores a directory's mode on drop, so a fixture made unreadable or
/// unwritable for one assertion is reclaimable again whatever the assertion
/// did.
#[cfg(unix)]
pub(super) struct RestoreMode {
    path: PathBuf,
}

#[cfg(unix)]
impl Drop for RestoreMode {
    fn drop(&mut self) {
        use std::os::unix::fs::PermissionsExt as _;
        match fs::set_permissions(&self.path, fs::Permissions::from_mode(0o755)) {
            Ok(()) => {}
            // Already gone: nothing to restore, and a second panic while
            // unwinding would abort the whole test process.
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => panic!("restore the mode of {}: {error}", self.path.display()),
        }
    }
}

/// Git's enumeration dies on the registration at `admin`, whose `commondir`
/// holds no bytes.
///
/// Read by that signature — a read of no bytes, and a message naming the file
/// — and never by the word the platform renders for the errno nothing set:
/// glibc prints `Success` and macOS `Undefined error: 0` for this same failure
/// (`PR5-RD-002`), so a check on either word misses the other platform.
pub(super) fn assert_enumeration_dies_on(fixture: &Fixture, admin: &Path) {
    assert_eq!(
        fs::metadata(admin.join("commondir"))
            .expect("the torn commondir")
            .len(),
        0,
        "the read Git fails is a read of no bytes"
    );
    let message = refusal_of(
        &fixture
            .manager
            .worktree_records()
            .expect_err("Git's enumeration dies on a zero-length commondir"),
    );
    let name = admin
        .file_name()
        .expect("an administrative directory has a name")
        .to_string_lossy()
        .into_owned();
    assert!(
        message.contains(&name) && message.contains("commondir"),
        "the enumeration dies on {name}'s commondir: {message}"
    );
}

/// One entry of a [`tree_bytes`] snapshot: what the name is, and its content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum TreeEntry {
    /// A directory, recorded so a deleted empty one is a difference.
    Dir,
    /// A regular file and its bytes.
    File(Vec<u8>),
    /// A symbolic link or junction and its target, never followed.
    Link(PathBuf),
}

/// Everything under `root`, by path relative to it: every directory, every
/// regular file with its bytes, every symbolic link or junction with its target. Nothing
/// is followed, so replacing a file with a link to equal bytes is a
/// difference, and so is a deleted empty directory.
impl WorkspaceManager {
    /// The binding the plain funnel makes: `revalidate_removal_proving` under
    /// `WriterProof::Unknown`, its admin directory alone.
    fn revalidate_removal(&self, target: &Path) -> Result<Option<PathBuf>, UpstrokeError> {
        self.revalidate_removal_proving(target, WriterProof::Unknown)
            .map(|binding| binding.admin)
    }
}

pub(super) fn tree_bytes(root: &Path) -> std::collections::BTreeMap<PathBuf, TreeEntry> {
    fn walk(root: &Path, dir: &Path, out: &mut std::collections::BTreeMap<PathBuf, TreeEntry>) {
        for entry in fs::read_dir(dir).expect("list a directory the test created") {
            let path = entry.expect("a directory entry").path();
            let relative = path
                .strip_prefix(root)
                .expect("an entry is under its root")
                .to_path_buf();
            let metadata = fs::symlink_metadata(&path).expect("entry metadata");
            if is_reparse_point(&metadata) {
                out.insert(
                    relative,
                    TreeEntry::Link(fs::read_link(&path).expect("read a link's target")),
                );
            } else if metadata.is_dir() {
                out.insert(relative, TreeEntry::Dir);
                walk(root, &path, out);
            } else {
                out.insert(
                    relative,
                    TreeEntry::File(fs::read(&path).expect("read an entry")),
                );
            }
        }
    }
    let mut out = std::collections::BTreeMap::new();
    // Only an actual not-found is an absent tree, which the empty map records
    // as a difference from a present one; any other failure is the test's.
    match fs::symlink_metadata(root) {
        Ok(metadata) if metadata.is_dir() && !is_reparse_point(&metadata) => {
            out.insert(PathBuf::new(), TreeEntry::Dir);
            walk(root, root, &mut out);
        }
        Ok(_) => panic!("{} is not a directory", root.display()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => panic!("{}: {error}", root.display()),
    }
    out
}

// The three `registration_gitdir_*` unit tests that stood here are gone. They
// drove `parsers::registration_checkout` — a `pub(super)` helper of a *sibling*
// child, reached from this file through `use super::*` — and every claim they
// made is made again, and more strongly, by that child's own module tests:
//
// * absolute-and-normalized, by `registration_checkout_names_the_row_a_gitdir_
//   falls_into`, which asserts *which* row each shape falls into rather than
//   only that it refused, over six shapes rather than two;
// * the Windows non-UTF-8 refusal, by `a_registration_that_is_not_utf8_is_
//   refused_with_its_offset`, which also asserts the byte offset;
// * the Unix byte-preserving decode, by `a_registration_keeps_every_path_byte_
//   on_unix`, which is the same assertion.
//
// The normalized-path one was worse than redundant: it spelt its fixture
// `Path::new("/repository/.git/worktrees/example")` and `b"/absolute/../…"`,
// and on Windows neither is absolute — `Path::is_absolute` wants a prefix — so
// the guest reached the "rooted but not absolute" arm and never the
// normalization check the test is named for. The sibling's fixtures route
// through its `absolute()` helper, which prepends `C:` on Windows, and are
// therefore about what they say on both platforms.
//
// What this file keeps is the composition: `a_relative_registration_still_binds_
// its_checkout` and the removal tests below drive the same decoder through the
// public primitives against a real repository, which is what a suite at this
// level is for.

/// Build the state a killed `git worktree add` leaves: the registration
/// exists and Git still holds its `initializing` lock.
pub(super) fn register_unpopulated(fixture: &Fixture, path: &Path) {
    git(
        &fixture.base,
        &[
            "worktree",
            "add",
            "-q",
            "--detach",
            &path.to_string_lossy(),
            &fixture.head,
        ],
    );
    let admin = fixture
        .manager
        .revalidate_removal(path)
        .expect("admin dir")
        .expect("the worktree is registered");
    fs::write(admin.join("locked"), "initializing\n").expect("hold the initializing lock");
    fs::remove_dir_all(path).expect("un-populate the checkout");
}

/// Spawned by
/// [`a_snapshot_ignores_a_replacement_object_and_materialises_the_judged_tree`].
#[test]
#[ignore = "subprocess helper"]
fn snapshot_replacement_helper() {
    if std::env::var_os(REPLACEMENT_WITNESS).is_none() {
        return;
    }
    assert_replacement_controls_pinned("snapshot");

    let fixture = Fixture::created("replace-objects");
    let file = fixture.base.join("replaced.txt");

    fs::write(&file, "A\n").expect("the judged content");
    git(&fixture.base, &["add", "replaced.txt"]);
    git(&fixture.base, &["commit", "-q", "-m", "the judged tree"]);
    let judged_commit = git(&fixture.base, &["rev-parse", "HEAD"]);
    let judged_tree = git(&fixture.base, &["rev-parse", "HEAD^{tree}"]);

    fs::write(&file, "B\n").expect("the other content");
    git(&fixture.base, &["add", "replaced.txt"]);
    git(&fixture.base, &["commit", "-q", "-m", "the other tree"]);
    let other_commit = git(&fixture.base, &["rev-parse", "HEAD"]);
    let other_tree = git(&fixture.base, &["rev-parse", "HEAD^{tree}"]);
    assert_ne!(judged_tree, other_tree, "two distinct trees");

    // Back to the judged commit, and the replacement in place.
    git(
        &fixture.base,
        &["checkout", "--detach", "--quiet", &judged_commit],
    );
    git(&fixture.base, &["replace", &judged_tree, &other_tree]);
    assert_eq!(
        git(&fixture.base, &["replace", "-l"]),
        judged_tree,
        "the replacement is in place"
    );
    assert_eq!(
        git(
            &fixture.base,
            &["rev-parse", "--verify", &format!("{judged_tree}^{{tree}}")]
        ),
        judged_tree,
        "and `rev-parse` still prints the judged tree, which is why the funnel's \
         resolve-once check cannot see the replacement"
    );

    let snapshot = fixture
        .manager
        .add_snapshot(
            &mut NoHooks,
            &SnapshotName::gates(0, 1, 1),
            &SnapshotInput::Tree {
                tree: oid(&judged_tree),
                parent: oid(&other_commit),
            },
        )
        .expect("the judged tree is a tree of this repository");

    // The fixture pins `core.autocrlf=false` and `core.eol=lf`, so the bytes
    // checked out are the bytes committed on every platform and this
    // comparison is about which blob, not about line endings.
    assert_eq!(
        fs::read_to_string(snapshot.path().join("replaced.txt")).expect("the checkout"),
        "A\n",
        "the snapshot materialises the judged tree, not the object replacing it"
    );
    let recorded = git(
        &fixture.base,
        &["cat-file", "commit", snapshot.head().as_str()],
    );
    assert!(
        recorded.contains(&format!("tree {judged_tree}")),
        "the ephemeral commit records the judged tree: {recorded}"
    );

    fixture
        .manager
        .remove_snapshot(&mut NoHooks, &snapshot)
        .expect("Snapshot.Remove + Snapshot.RemoveIntent");
}

/// A base that does not carry [`NO_REPLACEMENT_OBJECTS`] **and carries no other
/// ambient control over `refs/replace/*` either**, so a role process composed
/// from it cannot pass by inheriting one.
///
/// `HostEnvironment::from_process` is the production base and `HostRunner::run`
/// clears the ambient environment before installing what `compose` returned; a
/// suite that took the process environment unfiltered would be asserting about
/// whatever the machine running it happened to export.
///
/// **Removing one name was not enough, and that is round 3's blocking finding**
/// (PR #271). Until now this filtered `GIT_NO_REPLACE_OBJECTS` out of
/// `std::env::vars_os()` and passed everything else through, so mutating
/// `HostRunner::run` to discard the pair from the composed environment exited
/// `101` normally and **`0`** under `GIT_CONFIG_COUNT=1
/// GIT_CONFIG_KEY_0=core.useReplaceRefs GIT_CONFIG_VALUE_0=false` -- measured,
/// all seven non-ignored `replacement` tests green against unrepaired code. The
/// base is now [`environment_without_ambient_replacement_controls`] with that
/// one pair taken out of it, which is the enumeration; the measurement that
/// does not depend on the enumeration is
/// [`assert_a_role_process_sees_replacements`].
fn base_without_replacement_isolation() -> HostEnvironment {
    let case = KeyCase::current();
    let base: Vec<(OsString, OsString)> = environment_without_ambient_replacement_controls()
        .into_iter()
        .filter(|(key, _)| !case.same_key(key, std::ffi::OsStr::new(NO_REPLACEMENT_OBJECTS.0)))
        .collect();
    HostEnvironment::with_base(base, case)
}

/// Refuse to measure the role-process path through a runner whose own role
/// processes cannot see a replacement in the first place.
///
/// [`assert_replacement_controls_pinned`] answers for *this* process, and this
/// process is not where the claim lives: `HostRunner::run` calls `env_clear`
/// and installs what `compose` returned, so the only environment a role process
/// has is the composed one, less the work-tree context the runner sheds in a
/// work tree the engine made, the four selectors, and with a relative
/// `GIT_COMMON_DIR` given as the absolute path it names at the started
/// checkout. This probe is that exact runner --
/// [`base_without_replacement_isolation`], read as [`ObjectGraph::AsReplaced`],
/// which is the one setting that makes `compose` leave the pair off -- run over
/// a throwaway repository carrying a replacement. It must read the
/// **replacing** object. If it reads the recorded one, replacements are dead in
/// the composed environment and the witness below would read the judged tree
/// whatever production did.
///
/// This is the half that depends on no list of names: an eighteenth mechanism
/// costs this panic rather than a silent pass.
fn assert_a_role_process_sees_replacements(tag: &str) {
    let tree = scratch(&format!("role-replacement-live-{tag}"));
    let root = tree.path();
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
    git(&repo, &["replace", &recorded, "HEAD"]);
    assert_eq!(
        git(
            &repo,
            &["for-each-ref", "--format=%(refname)", "refs/replace/"]
        ),
        format!("refs/replace/{recorded}"),
        "`{tag}`: `git replace` did not write under `refs/replace/`, so this \
         probe would answer a question it was not asked"
    );

    let runner = HostRunner::new()
        .with_environment(base_without_replacement_isolation().reading(ObjectGraph::AsReplaced));
    let shown = gate_in(&runner, &repo, &["show", &format!("{recorded}:probe.txt")]);
    assert_eq!(
        shown.stdout,
        "replacing\n",
        "`{tag}`: a role process composed from this base does not honour \
         `refs/replace/*` at all, so the witness below would read the judged \
         tree whatever production installed. The enumerated controls this \
         process carries are {:?}; if none of them explains this, the \
         enumeration is missing a mechanism and closing it is the fix",
        ambient_replacement_controls()
    );
}

/// One `git` gate, run through the production runner in `workspace`.
fn gate_in(runner: &HostRunner, workspace: &Path, args: &[&str]) -> ProcessOutput {
    let request = crate::runner::gate_request(
        CommandSpec {
            program: "git".to_owned(),
            args: args.iter().map(|arg| (*arg).to_owned()).collect(),
            env: Vec::new(),
            stdin: Vec::new(),
            settings_file: None,
        },
        workspace.to_path_buf(),
        std::time::Duration::from_secs(120),
        InvocationId::attempt(
            TaskKey(0),
            GenerationId(0),
            AttemptNumber(1),
            AttemptRole::Gate(0),
            0,
        ),
    );
    let output = runner.run_blocking(&request).expect("the gate ran");
    assert_eq!(
        output.code,
        Some(0),
        "git {args:?} in {}: {output:?}",
        workspace.display()
    );
    output
}

/// Spawned by
/// [`a_role_process_in_a_snapshot_reads_the_judged_tree_not_a_replacement`].
#[test]
#[ignore = "subprocess helper"]
fn role_process_replacement_helper() {
    if std::env::var_os(REPLACEMENT_WITNESS).is_none() {
        return;
    }
    assert_replacement_controls_pinned("role-process");
    assert_a_role_process_sees_replacements("role-process");

    let fixture = Fixture::created("replace-role-process");
    let file = fixture.base.join("replaced.txt");

    fs::write(&file, "A\n").expect("the judged content");
    git(&fixture.base, &["add", "replaced.txt"]);
    git(&fixture.base, &["commit", "-q", "-m", "the judged tree"]);
    let judged_commit = git(&fixture.base, &["rev-parse", "HEAD"]);
    let judged_tree = git(&fixture.base, &["rev-parse", "HEAD^{tree}"]);

    fs::write(&file, "B\n").expect("the other content");
    git(&fixture.base, &["add", "replaced.txt"]);
    git(&fixture.base, &["commit", "-q", "-m", "the other tree"]);
    let other_commit = git(&fixture.base, &["rev-parse", "HEAD"]);
    let other_tree = git(&fixture.base, &["rev-parse", "HEAD^{tree}"]);
    assert_ne!(judged_tree, other_tree, "two distinct trees");

    git(
        &fixture.base,
        &["checkout", "--detach", "--quiet", &judged_commit],
    );
    git(&fixture.base, &["replace", &judged_tree, &other_tree]);
    // `git replace -l` lists under `GIT_REPLACE_REF_BASE`, so it would answer
    // `yes` about a namespace nothing reads; name `refs/replace/` itself.
    assert_eq!(
        git(
            &fixture.base,
            &["for-each-ref", "--format=%(refname)", "refs/replace/"]
        ),
        format!("refs/replace/{judged_tree}"),
        "the replacement is in place, under the namespace a reader looks in"
    );

    let snapshot = fixture
        .manager
        .add_snapshot(
            &mut NoHooks,
            &SnapshotName::gates(0, 1, 1),
            &SnapshotInput::Tree {
                tree: oid(&judged_tree),
                parent: oid(&other_commit),
            },
        )
        .expect("the judged tree is a tree of this repository");

    // The fixture pins `core.autocrlf=false` and `core.eol=lf`, so what a role
    // process reads back is the blob and not the platform's line endings.
    assert_eq!(
        fs::read_to_string(snapshot.path().join("replaced.txt")).expect("the checkout"),
        "A\n",
        "the manager's own commands already materialise the judged tree"
    );

    let runner = HostRunner::new().with_environment(base_without_replacement_isolation());
    let shown = gate_in(&runner, snapshot.path(), &["show", "HEAD:replaced.txt"]);
    assert_eq!(
        shown.stdout, "A\n",
        "a gate reading the snapshot through Git read the object `git replace` \
         points at, not the tree the snapshot was taken of"
    );
    let status = gate_in(&runner, snapshot.path(), &["status", "--porcelain"]);
    assert_eq!(
        status.stdout.trim(),
        "",
        "a gate saw the untouched snapshot as dirty, because Git compared its \
         index against the replacing tree"
    );

    fixture
        .manager
        .remove_snapshot(&mut NoHooks, &snapshot)
        .expect("Snapshot.Remove + Snapshot.RemoveIntent");
}

/// Where [`the_neutraliser_defeats_every_ambient_control_it_enumerates`] tells
/// its probe to do the work, and which of the two questions to answer;
/// `#[ignore]`-guarded for the reason [`REPLACEMENT_WITNESS`] is.
///
/// `pinned` is the neutralised leg: the names and then the measurement. `live`
/// is the controlled leg, and it asks **only** the measurement -- a controlled
/// leg that also checked the pinned names would fail for want of the pins
/// whatever the hostile value did, which would make it no evidence that the row
/// is a control at all, and so no evidence that the neutralised leg above it
/// means anything.
pub(super) const REPLACEMENT_CONTROL_PROBE: &str = "UPSTROKE_PR271_REPLACEMENT_CONTROL_PROBE";

/// Spawned by [`the_neutraliser_defeats_every_ambient_control_it_enumerates`].
///
/// The `live` arm exits [`REPLACEMENT_DISABLED_EXIT`] rather than panicking,
/// because *observing* that a treatment disabled replacements is the answer
/// that leg exists to collect, and a panic would spell it `101` -- the same
/// code a `git` that never ran gives. Keeping the two apart is half of the
/// round-4 repair to this grid: a backslash in `TMPDIR` used to make the raw
/// leg fail at `git config`'s parser, and the grid credited that as evidence
/// that the row suppressed replacement.
#[test]
#[ignore = "subprocess helper"]
fn replacement_control_probe_helper() {
    match std::env::var_os(REPLACEMENT_CONTROL_PROBE) {
        None => (),
        Some(mode) if mode == *"pinned" => assert_replacement_controls_pinned("control-probe"),
        Some(_) => {
            // Bound rather than tested inline so the verdict is one value: a
            // `RefsElsewhere` and a `NotHonoured` are the same answer to the
            // grid and a different answer to a reader.
            let verdict = replacement_liveness("control-probe");
            if verdict != ReplacementLiveness::Live {
                std::process::exit(REPLACEMENT_DISABLED_EXIT);
            }
        }
    }
}

/// Spawned by [`quiescence_holds_when_the_recorded_tree_carries_a_replacement`].
///
/// [`assert_replacement_controls_pinned`] is the round-3 repair, and it is the
/// first statement for a reason: it refuses the enumerated controls by name and
/// then measures, over a throwaway repository in *this* environment, that
/// `refs/replace/*` is honoured at all. The witness below is worth nothing
/// unless it is, and a mechanism nobody enumerated fails there rather than
/// passing here.
#[test]
#[ignore = "subprocess helper"]
fn quiescence_replacement_helper() {
    if std::env::var_os(REPLACEMENT_WITNESS).is_none() {
        return;
    }
    assert_replacement_controls_pinned("quiescence");

    let fixture = Fixture::created("replace-quiescence");
    let slot = fixture.add_task(&mut NoHooks, "q", 1);
    let path = fixture
        .manager
        .slot_target(&slot)
        .expect("the worktree path");

    let held = git(&path, &["rev-parse", "HEAD^{tree}"]);
    let other = git(
        &fixture.base,
        &["rev-parse", &format!("{}^{{tree}}", fixture.seed)],
    );
    assert_ne!(held, other, "two distinct trees");

    assert!(
        matches!(
            fixture
                .manager
                .verify_worktree(&mut NoHooks, &slot, &Quiescence::HoldsTree(held.clone()))
                .expect("verify"),
            Ok(())
        ),
        "the worktree holds its own tree before anything is installed"
    );

    git(&fixture.base, &["replace", &held, &other]);
    assert_eq!(
        git(&fixture.base, &["replace", "-l"]),
        held,
        "the replacement is in place"
    );

    let verdict = fixture
        .manager
        .verify_worktree(&mut NoHooks, &slot, &Quiescence::HoldsTree(held.clone()))
        .expect("verify");
    assert!(
        matches!(verdict, Ok(())),
        "a worktree nothing wrote to is quiescent; Git answered about \
         `refs/replace/{held}` instead: {verdict:?}"
    );
}

/// Where the helper tells its parent which repository to inspect.
pub(super) const ID_UNREAD_RECORD: &str = "UPSTROKE_PR5A_ID_UNREAD_RECORD";

/// Set to make [`abort_probe_helper`] abort.
///
/// Guarded by a variable for the same reason [`id_unread_kill_helper`] is: the
/// item is `#[ignore]`, but a run with `--include-ignored` would otherwise
/// abort the whole test process.
pub(super) const ABORT_PROBE: &str = "UPSTROKE_PR136_ABORT_PROBE";

/// Abort, so a parent can measure what this platform's
/// [`std::process::abort`] looks like from outside instead of describing it.
#[test]
#[ignore = "subprocess helper"]
fn abort_probe_helper() {
    if std::env::var_os(ABORT_PROBE).is_none() {
        return;
    }
    std::process::abort();
}

/// Set to make [`exit_one_probe_helper`] exit 1.
pub(super) const EXIT_ONE_PROBE: &str = "UPSTROKE_PR136_EXIT_ONE_PROBE";

/// Exit 1, which is what the mutation pass 2 named turns an abort into.
#[test]
#[ignore = "subprocess helper"]
fn exit_one_probe_helper() {
    if std::env::var_os(EXIT_ONE_PROBE).is_none() {
        return;
    }
    std::process::exit(1);
}

/// Set to make [`exit_134_probe_helper`] exit 134.
pub(super) const EXIT_134_PROBE: &str = "UPSTROKE_PR292_EXIT_134_PROBE";

/// Exit 134, the number a shell gives a death by `SIGABRT`, which is what
/// #292's review turned its kill children's aborts into.
#[test]
#[ignore = "subprocess helper"]
fn exit_134_probe_helper() {
    if std::env::var_os(EXIT_134_PROBE).is_none() {
        return;
    }
    std::process::exit(134);
}

/// Spawned by `a_kill_at_id_unread_aborts_before_the_id_is_recorded`.
#[test]
#[ignore = "subprocess helper"]
fn id_unread_kill_helper() {
    let Some(record) = std::env::var_os(ID_UNREAD_RECORD) else {
        return;
    };
    let fixture = Fixture::created("id-unread-helper");
    let tree = git(
        &fixture.base,
        &["rev-parse", &format!("{}^{{tree}}", fixture.head)],
    );
    fs::write(&record, format!("{}\n{tree}\n", fixture.base.display()))
        .expect("record the repository before dying");
    let manager = fixture.manager.clone();
    let head = fixture.head.clone();
    // Keep the repository: the parent inspects it after this process dies,
    // and `Fixture`'s destructor would remove it — which is also exactly
    // what an aborting process does not run.
    std::mem::forget(fixture);

    struct KillAtIdUnread;
    impl EffectHooks for KillAtIdUnread {
        fn phase(&mut self, _site: EffectSiteId, phase: HookPhase) -> Injection {
            match phase {
                HookPhase::Point {
                    point: SubEffectPoint::IdUnread,
                    ..
                } => Injection::Kill,
                _ => Injection::Proceed,
            }
        }

        fn refusal_cause(&self) -> Option<String> {
            None
        }
    }
    let _ = manager.candidate_commit_tree(&mut KillAtIdUnread, &tree, &head, "candidate");
    unreachable!("the funnel aborts at IdUnread");
}

// -----------------------------------------------------------------------
// The residue classifier: totality, elements, and kill sampling
// -----------------------------------------------------------------------

/// Write an object nothing references.
pub(super) fn write_orphan(repository: &Path, content: &str) -> String {
    let mut child = Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(["hash-object", "-w", "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn git hash-object");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(content.as_bytes())
        .expect("feed the object");
    let output = child.wait_with_output().expect("hash-object");
    assert!(output.status.success());
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

pub(super) fn classify(site: EffectSiteId, target: &ResidueTarget<'_>) -> ObjectResidue {
    classify_object_residue(site, target).expect("the classifier answers")
}

/// `command_internal_sub_effects`, synthetic half: "each residue element …
/// is constructed in a real temporary repository at the site's worktree,
/// `classify_object_residue` returns `Internal`, `Worktree.Verify` fails,
/// and the tabled recovery converges with fsck showing the objects
/// unreachable and untouched".
///
/// **The `Verify`-fails half is asserted where it holds and its negation
/// where it does not, and the partition is a count.**
/// [`element_breaks_quiescence`] carries the argument: an unreferenced
/// object and a Git temporary object file live in the shared object store,
/// are R27 — "Git's" — and are left by ordinary Git use, so a
/// `Worktree.Verify` that saw them would refuse to reuse an `OpenNoAttempt`
/// worktree in almost every real repository. Reported as a boundary, not
/// concealed as an omission.
#[test]
fn every_registered_residue_element_is_constructed_and_recovers() {
    let mut records: Vec<(EffectSiteId, SyntheticRecord)> = Vec::new();
    let mut quiescence_broken = 0usize;
    let mut object_store_only = 0usize;

    for site in residue_classified_sites() {
        for element in site.residue_elements() {
            let record = construct_and_recover(site, *element);
            assert!(record.constructed, "`{site}`/{element:?} was constructed");
            assert_eq!(
                record.classified,
                ObjectResidue::Internal,
                "`{site}`/{element:?} classifies Internal"
            );
            assert!(record.recovered, "`{site}`/{element:?} recovers");
            if element_breaks_quiescence(*element) {
                quiescence_broken += 1;
            } else {
                object_store_only += 1;
            }
            records.push((site, record));
        }
    }

    // Distinct-value counts rather than prose: the grid is 24 (site,
    // element) pairs, and the two halves of the Verify boundary are 12 and
    // 12. A site that grows an element, or an element that changes side,
    // moves one of these.
    assert_eq!(
        records.len(),
        residue_classified_sites()
            .iter()
            .map(|site| site.residue_elements().len())
            .sum::<usize>(),
        "one record per (site, element) the frozen enums register"
    );
    assert_eq!(records.len(), 24, "the frozen grid is 24 pairs");
    assert_eq!(
        quiescence_broken, 12,
        "elements that make a worktree non-quiescent"
    );
    assert_eq!(object_store_only, 12, "elements that are R27 and Git's");
    assert!(
        records
            .iter()
            .all(|(_, record)| record.classified == ObjectResidue::Internal),
        "every element of every registered class classifies into that class"
    );

    // The evidence record, in the packet's own type, per site.
    for site in residue_classified_sites() {
        let synthetic: Vec<SyntheticRecord> = records
            .iter()
            .filter(|(seen, _)| *seen == site)
            .map(|(_, record)| *record)
            .collect();
        assert_eq!(synthetic.len(), site.residue_elements().len());
        let evidence = Evidence::RecoveryProven {
            synthetic,
            sampling: SamplingRecord {
                n: SAMPLING_N,
                histogram: ClassHistogram::default(),
                unclassified: 0,
                recovered: true,
            },
        };
        assert_eq!(
            evidence.label(),
            EvidenceLabel::RecoveryProven,
            "a residue class never carries an executed-hook claim"
        );
        assert!(!evidence.claims_execution());
    }

    // The synthetic half of the residue-class evidence, the tracked file the
    // sequential registry (`engine::topology::coverage`) embeds: one record
    // per (site, element), exactly what this test asserted above.
    // Deterministic by construction -- every field is what the assertions
    // required -- so unlike the histogram it is pinned rather than
    // machine-varying, and this test holds the tracked bytes to what it
    // constructed instead of rewriting them: an ordinary test that rewrote
    // a tracked input in place left a truncation window for the ordinary
    // coverage tests that read it (PR10's round-3 regression lens, P2), and
    // rewrote it on every machine. `UPSTROKE_REGENERATE_EFFECT_ARTIFACTS=1`
    // regenerates it, as it does the residue-class declarations.
    let sites: Vec<serde_json::Value> = residue_classified_sites()
        .iter()
        .map(|site| {
            serde_json::json!({
                "site": site.name(),
                "synthetic": records
                    .iter()
                    .filter(|(seen, _)| seen == site)
                    .map(|(_, record)| *record)
                    .collect::<Vec<SyntheticRecord>>(),
            })
        })
        .collect();
    let emitted = serde_json::to_string_pretty(&serde_json::json!({
        "note": "decisions.effect_site_inventory.outputs, the synthetic-construction half of \
                 the residue-class evidence: one record per (site, element) the frozen enums \
                 register, written by \
                 workspace_manager::tests::every_registered_residue_element_is_constructed_and_recovers \
                 from what it constructed, classified and recovered, and held to on every run. \
                 Deterministic, so it is pinned (regenerate with \
                 UPSTROKE_REGENERATE_EFFECT_ARTIFACTS=1); effects/sequential-registry.json \
                 embeds it.",
        "sites": sites,
    }))
    .expect("the synthetic evidence serializes");
    let tracked =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(crate::effects::RESIDUE_SYNTHETIC_JSON);
    let generated = format!("{emitted}\n");
    if std::env::var_os(crate::effects::REGENERATE).is_some() {
        write_file(&tracked, generated.as_bytes());
    }
    let on_disk = fs::read_to_string(&tracked).unwrap_or_else(|error| {
        panic!(
            "{} is tracked; regenerate it with {}=1: {error}",
            crate::effects::RESIDUE_SYNTHETIC_JSON,
            crate::effects::REGENERATE
        )
    });
    assert_eq!(
        on_disk.replace("\r\n", "\n"),
        generated,
        "{} is stale against what this test constructed; regenerate it with {}=1",
        crate::effects::RESIDUE_SYNTHETIC_JSON,
        crate::effects::REGENERATE
    );
}

/// Construct one element at one site, classify it, check quiescence, and
/// run the tabled recovery.
fn construct_and_recover(site: EffectSiteId, element: ResidueElement) -> SyntheticRecord {
    let tag = format!("syn-{}-{element:?}", site.variant().to_lowercase());
    let fixture = Fixture::created(&tag);
    let base = fixture.base.clone();

    // The site's owning worktree, and the state in which its after-phase
    // reference is absent — which is the state the sentence is about.
    let (slot, path) = match site {
        EffectSiteId::Worktree(WorktreeSite::AddStaging)
        | EffectSiteId::Object(ObjectSite::ProposalCherryPick) => {
            let slot = Slot::Staging { sequence: 1 };
            fixture
                .manager
                .write_intent(&mut NoHooks, &slot)
                .expect("intent");
            let path = fixture.manager.slot_path(&slot);
            (Some(slot), path)
        }
        EffectSiteId::Snapshot(SnapshotSite::Add) => {
            let slot = Slot::Snapshot {
                name: SnapshotName::gates(0, 1, 1),
            };
            fixture
                .manager
                .write_intent(&mut NoHooks, &slot)
                .expect("intent");
            let path = fixture.manager.slot_path(&slot);
            (Some(slot), path)
        }
        EffectSiteId::Object(ObjectSite::SnapshotCommitTree | ObjectSite::CandidateCommitTree) => {
            (None, base.clone())
        }
        _ => {
            let slot = fixture.task("alpha", 1);
            fixture
                .manager
                .write_intent(&mut NoHooks, &slot)
                .expect("intent");
            let path = fixture.manager.slot_path(&slot);
            (Some(slot), path)
        }
    };

    // A populated worktree for every site whose residue lives inside one;
    // the three `Add` sites are about a worktree that was never populated.
    let is_add_site = matches!(
        site,
        EffectSiteId::Worktree(WorktreeSite::Add | WorktreeSite::AddStaging)
            | EffectSiteId::Snapshot(SnapshotSite::Add)
    );
    if let Some(slot) = slot.as_ref() {
        if is_add_site {
            register_unpopulated(&fixture, &path);
        } else {
            fixture
                .manager
                .add_worktree(&mut NoHooks, slot, &fixture.head)
                .expect("worktree");
            if site == EffectSiteId::Object(ObjectSite::CandidateStage) {
                // The after-phase reference of `git add -A` is an index that
                // reflects the working tree, so the interrupted prefix has
                // an unstaged change in it.
                fs::write(path.join("a.txt"), "edited\n").expect("unstaged change");
            }
        }
    }

    let object = construct_element(&fixture, &path, element);
    let target = ResidueTarget::new(&base).at(&path).from_base(&fixture.head);
    let classified = classify(site, &target);

    // The quiescence half, asserted in both directions.
    if let Some(slot) = slot.as_ref() {
        let verified = fixture
            .manager
            .verify_worktree(
                &mut NoHooks,
                slot,
                &Quiescence::AtBase(fixture.head.clone()),
            )
            .expect("verify");
        assert_eq!(
            verified.is_err(),
            element_breaks_quiescence(element),
            "`{site}`/{element:?}: Worktree.Verify must {} — see element_breaks_quiescence",
            if element_breaks_quiescence(element) {
                "fail"
            } else {
                "pass, because this element is R27 and Git's"
            }
        );
    }

    // The tabled recovery: the site's before-phase action. Forced removal
    // and a fresh add for a worktree site; nothing at all for the two
    // commit-tree sites, whose T-CAND-OBJ (a) action is "nothing to delete:
    // the unpinned object is left to Git".
    let before = unreachable_objects(&base).expect("fsck");
    let mut recovered = true;
    if let Some(slot) = slot.as_ref() {
        fixture
            .manager
            .remove_worktree(&mut NoHooks, slot)
            .expect("forced removal");
        fixture
            .manager
            .add_worktree(&mut NoHooks, slot, &fixture.head)
            .expect("recreate");
        recovered = fixture
            .manager
            .verify_worktree(
                &mut NoHooks,
                slot,
                &Quiescence::AtBase(fixture.head.clone()),
            )
            .expect("verify")
            .is_ok();
    }
    let after = unreachable_objects(&base).expect("fsck");
    if let Some(object) = object.as_deref() {
        assert!(
            before.iter().any(|id| id == object) && after.iter().any(|id| id == object),
            "fsck lists `{object}` unreachable before and after the recovery, untouched"
        );
    }
    assert!(
        before.iter().all(|id| after.contains(id)),
        "the recovery deletes no object: R27 is Git's"
    );

    SyntheticRecord {
        element,
        constructed: true,
        classified,
        recovered,
    }
}

/// Build one residue element in a real repository, returning the object id
/// when the element is one.
fn construct_element(fixture: &Fixture, path: &Path, element: ResidueElement) -> Option<String> {
    let git_dir = || {
        git_dir_of(path)
            .expect("git dir")
            .expect("the worktree has a git dir")
    };
    match element {
        ResidueElement::UnreferencedObject => Some(write_orphan(
            &fixture.base,
            "an object an interrupted command wrote\n",
        )),
        ResidueElement::TemporaryObjectFile => {
            // In a fan-out directory the store already holds, beside real
            // objects, where the common loose write leaves it. Planted at
            // the object root this exercised only the arm the scan always
            // had, and the grid the per-element evidence rests on stayed
            // green with the fan-out loop deleted
            // (`PR258-GRID-PLANTS-AT-THE-OBJECT-ROOT`).
            let objects = object_directory(&fixture.base).expect("object directory");
            fs::write(
                fan_out_directory(&objects).join("tmp_obj_synthetic"),
                b"partial",
            )
            .expect("temp object");
            None
        }
        ResidueElement::IndexLock => {
            fs::write(git_dir().join("index.lock"), "").expect("index.lock");
            None
        }
        ResidueElement::CherryPickHead => {
            fs::write(git_dir().join("CHERRY_PICK_HEAD"), &fixture.side).expect("plant");
            None
        }
        ResidueElement::MergeHead => {
            fs::write(git_dir().join("MERGE_HEAD"), &fixture.side).expect("plant");
            None
        }
        ResidueElement::MergeMsg => {
            fs::write(git_dir().join("MERGE_MSG"), "interrupted\n").expect("plant");
            None
        }
        ResidueElement::OrigHead => {
            fs::write(git_dir().join("ORIG_HEAD"), &fixture.head).expect("plant");
            None
        }
        ResidueElement::SequencerState => {
            let sequencer = git_dir().join("sequencer");
            fs::create_dir_all(&sequencer).expect("sequencer directory");
            fs::write(sequencer.join("todo"), "pick abc\n").expect("plant");
            None
        }
        // Already built by `register_unpopulated` before this is called:
        // the element *is* the state of the worktree, not a file added to
        // one.
        ResidueElement::RegisteredUnpopulatedWorktree => None,
    }
}

/// The frozen sample count, per site.
///
/// `command_internal_sub_effects`: "the Git child of the site is killed at
/// uncontrolled points through the process funnel across N runs (N frozen
/// per site in the registry …)". Eight, and the same for all four sampled
/// commands, because the claim each sample carries is per sample — "every
/// observed residue must classify into exactly one class and recover by the
/// classified action" — and is not a coverage claim about the classes. The
/// delays are a ladder across a *measured* uninterrupted run of the same
/// command in the same repository rather than a fixed duration, so the
/// sampler lands inside the command on a fast machine and on a slow one.
const SAMPLING_N: u32 = 8;

/// `slice_contract.proof_tests[7]`: "sampling harness kills the Git child
/// of `git add`, `write-tree`, `cherry-pick`, and `worktree add` across N
/// runs and every observed residue classifies into exactly one class and
/// recovers (histogram recorded; **Internal not required**)".
///
/// # The stability claim
///
/// This harness is nondeterministic by construction and the assertion is
/// chosen so that it is not: what is asserted is that **every** sample
/// classified into one of the three classes and recovered by that class's
/// tabled action, and that `unclassified == 0`. Which class a given sample
/// lands in is a race between the kill and Git, so the *counts* cannot be
/// asserted — a suite that required `Internal` would be red whenever the
/// machine was fast, and "no residue observed" is not a class.
///
/// # What the counts being unassertable does **not** excuse
///
/// It used to excuse two things, and `PR5-CONF-004` is both of them
/// (Fable's `PR5-CONF-002` is the same defect).
///
/// **The tally had no oracle.** `histogram.internal += 1` →
/// `histogram.none += 1` at the classifier's own match survived the whole
/// suite: every count moved, every assertion here was about the *total*, and
/// the total is invariant under a swap. So the observations are now kept
/// per sample and tallied a second time, here, by a different expression
/// over the same list — the two axes are the *classifier's answer* and *the
/// bucket it is counted in*, and only crossing them can see a bucket that is
/// counted under the wrong name.
///
/// **The histogram was never written down.** `outputs` requires, per site,
/// "sampling N **and observed-class histogram**", and
/// `effects/residue-classes.json` carried the N and not the histogram — its
/// own note conceded that the histogram "is printed … and is a property of
/// the machine, never asserted", which is a description of the omission
/// rather than a discharge of it. A byte-pinned artifact genuinely cannot
/// hold a machine-varying count, so the histogram goes to a **separate,
/// machine-varying evidence file**, this test writes it, and this test reads
/// it back: the record exists as a file a gate can collect, and the clause
/// is discharged by something other than stdout nobody keeps.
#[test]
fn sampled_git_child_kills_every_residue_classified_and_recovered() {
    let mut records = Vec::new();
    for site in SAMPLED_SITES {
        let run = sample_site(site);
        let record = run.record;
        println!(
            "residue sampling {site}: n={} none={} internal={} after={} unclassified={}",
            record.n,
            record.histogram.none,
            record.histogram.internal,
            record.histogram.after,
            record.unclassified
        );
        assert_eq!(record.n, SAMPLING_N);
        assert_eq!(
            run.observed.len(),
            SAMPLING_N as usize,
            "one observation per sample, or the tally below is over the wrong list"
        );

        // **The refusals first** (PR #136 pass 1, finding 2). A classifier
        // refusal becomes `None` in `observed`, so `tally` leaves it out of
        // every bucket and the histogram total falls short of `SAMPLING_N` —
        // which means the generic total assertion below fired first and the
        // words explaining *why* the classifier refused were never printed.
        // The diagnostic this sweep added was therefore unreachable on the
        // one failure it exists to diagnose. It is asserted here, before
        // anything derived from the tally, because a refusal is a different
        // failure from an unclassifiable residue and deserves to be reported
        // as itself.
        assert!(
            run.refusals.is_empty(),
            "{site}: the classifier refused {} of {SAMPLING_N} samples, which is not the \
                 same thing as a residue in no class — it is an inspection that failed, and \
                 the assertions below would have called it unclassified:\n{}",
            run.refusals.len(),
            // Displayed, not `{:?}`: each refusal carries its sample's report,
            // whose reflog bytes are already escaped, and debug formatting
            // would escape them a second time.
            run.refusals.join("\n")
        );

        // The independent tally. Not `tally()` again — a second call to the
        // code under test agrees with itself by construction — but a count
        // per class written out separately, so a bucket incremented under
        // the wrong name is a disagreement rather than an invisible swap.
        let counted = |wanted: ObjectResidue| -> u32 {
            u32::try_from(
                run.observed
                    .iter()
                    .filter(|sample| **sample == Some(wanted))
                    .count(),
            )
            .expect("a sample count fits in u32")
        };
        assert_eq!(
            (
                record.histogram.none,
                record.histogram.internal,
                record.histogram.after
            ),
            (
                counted(ObjectResidue::None),
                counted(ObjectResidue::Internal),
                counted(ObjectResidue::After)
            ),
            "{site}: the histogram does not count what the classifier answered: \
                 {:?}",
            run.observed
        );
        assert_eq!(
            record.histogram.total(),
            u64::from(SAMPLING_N),
            "every sample is accounted for by exactly one class"
        );
        assert_eq!(
            record.unclassified, 0,
            "an unclassifiable residue is durable state no tabled action recovers; a \
             classifier refusal reads the same here and is reported by its own assertion \
             above, which found none"
        );
        assert!(
            record.recovered,
            "every sample recovered by its classified action; at {site} these did not, \
                 each with its report taken before the recovery ran:\n{}",
            run.unrecovered.join("\n")
        );
        records.push((site, record, run.budget, run.replayed));
    }
    assert_eq!(
        records.len(),
        4,
        "the four commands the contract's proof_tests name"
    );

    // What was actually spawned, when the kill fired and what it did to the
    // child — counted independently of the site labels and of the
    // observation list; see `SAMPLED_LAUNCHES`. What is counted per command
    // SHAPE rather than per site record is counted so because "the Git
    // child of `git add`, `write-tree`, `cherry-pick`, and `worktree add`"
    // is a claim about four commands and a site label is not one: two sites
    // that sampled the same shape would leave four records and four labels
    // intact. The kill floor at the end is the one claim that is neither —
    // it is over the sampling as a whole, and only over kills that *landed*,
    // for the reason written there.
    {
        let log = SAMPLED_LAUNCHES.lock().expect("the launch log");
        assert_eq!(
            log.len(),
            4 * SAMPLING_N as usize,
            "every sampled site must launch exactly its frozen N children, \
                 and an observation is pushed whether or not one was"
        );
        let group_end = log
            .iter()
            .map(|launch| launch.group_ended)
            .max()
            .unwrap_or_default();
        println!(
            "residue sampling: every killed group was gone within {group_end:?} of its leader's \
             reap (the bound is {:?})",
            super::fixture::GROUP_END_BOUND
        );
        for (label, fixed) in [
            ("git add", WorkspaceManager::CANDIDATE_STAGE_ARGV[0]),
            (
                "git write-tree",
                WorkspaceManager::CANDIDATE_WRITE_TREE_ARGV[0],
            ),
            (
                "git cherry-pick",
                WorkspaceManager::PROPOSAL_CHERRY_PICK_ARGV[0],
            ),
            ("git worktree add", WorkspaceManager::WORKTREE_ADD_ARGV[0]),
        ] {
            let shape: Vec<&SampledLaunch> = log
                .iter()
                .filter(|launch| launch.argv[0] == fixed)
                .collect();
            let launched = shape.len();
            assert_eq!(
                launched, SAMPLING_N as usize,
                "{label}: the sampler launched it {launched} times, not N — the four \
                     command SHAPES are what the contract samples, not four site labels"
            );

            // The premise of every count below. A child that failed on its
            // own left the fixture's residue, not the kill's, and a reading
            // of the status loose enough to call that a kill would keep
            // counting kills after the kill was gone.
            let failed: Vec<Option<i32>> = shape
                .iter()
                .filter_map(|launch| match launch.end {
                    LaunchEnd::Failed(code) => Some(code),
                    LaunchEnd::Killed | LaunchEnd::Completed => None,
                })
                .collect();
            assert!(
                failed.is_empty(),
                "{label}: a sampled child neither died by the kill nor reached its \
                     own successful exit (codes {failed:?}) — what the classifier then \
                     saw is this fixture's failure, and no count of kills over these \
                     samples means anything"
            );

            // The rung each kill was **aimed at**.
            // `command_internal_sub_effects` (ii) says "killed at
            // **uncontrolled points** through the process funnel", and one
            // fixed delay is one point sampled N times: the ladder is that
            // clause, so it is asserted beside the kill rather than left to
            // the reader of `sample_site`.
            //
            // This is the aim and only the aim — `PR5-R5-001`. These are
            // the parameters the caller passed, so `sample_site` computing
            // a ladder is the whole of what they can witness: deleting
            // `std::thread::sleep(after)` from `kill_git_child`, which
            // fires every kill at the spawn instant and is the exact
            // negation of the clause cited above, left this list a perfect
            // ladder and the suite green on Linux and on the Windows guest.
            // The two assertions after it are over what the kills *did*.
            let delays: Vec<std::time::Duration> =
                shape.iter().map(|launch| launch.after).collect();
            assert!(
                delays.windows(2).all(|rungs| rungs[0] < rungs[1]),
                "{label}: the N kills must be aimed at N distinct, increasing points \
                     through the command, not at one point N times: {delays:?}"
            );

            // **A kill fired at every one of this command's children**
            // (`PR5-R5-002`). `slice_contract.proof_tests[8]` names four
            // commands — "the Git child of `git add`, `write-tree`,
            // `cherry-pick`, and `worktree add`" — and a floor over the
            // sampling as a whole discharges the clause for none of them:
            // guarding the kill with `if args[0] != "add"` left `git add`'s
            // eight children to reach their own exit 0, and the floor
            // below, the `Failed` arm above, the ladder and the whole suite
            // stayed green on both platforms.
            //
            // What is counted is kills **fired**, not kills that won their
            // race. Landing is a race against a command that may already
            // have finished — `git add` has measured 1 in 8 and 2 in 8, for
            // the reason written at the floor below — so a per-shape floor
            // on landings would stand on a margin of one or two samples and
            // would be red on the next machine. Firing is not a
            // race: the sampler either aimed a kill at this command's child
            // or it did not, N times, which is per command and exact.
            //
            // `fired` is written by `SampledChild::kill` itself, so an edit
            // that skips the kill skips the record with it. A count over
            // records written beside the call is what `PR5-R5-002` walked
            // past.
            let unfired = shape.iter().filter(|launch| launch.fired.is_none()).count();
            assert_eq!(
                unfired, 0,
                "{label}: {unfired} of this command's {launched} sampled children were \
                     never fired at — the contract names this command among the four whose \
                     Git child this harness kills, and a kill skipped for one of the four is \
                     invisible to every count taken over all four"
            );

            // **What the kill answered is recorded and printed, and not
            // asserted on** (PR #136 pass 3, finding 3). The assertion that
            // stood here paired a kill error with a `try_wait` taken
            // immediately before the kill, and the reviewer reproduced the
            // false green: on Unix an exited-but-unreaped child is a zombie,
            // `kill(SIGKILL)` on it returns **success**, and the wait reports
            // the child's own exit — so in the race window the error is empty,
            // this check passed, and another shape's kills satisfied the
            // global floor while this shape killed nothing. The claim in the
            // deleted comment that the race was "one-sided, a false red and
            // never a false green" was wrong.
            //
            // No third attempt is being made. Two observations taken at two
            // instants cannot decide this from inside the process, and the row
            // says what a sound version needs instead: the child watched from
            // outside it.
            let errors: Vec<&String> = shape
                .iter()
                .filter_map(|launch| launch.kill_error.as_ref())
                .collect();
            if !errors.is_empty() {
                println!("kill errors {label}: {errors:?}");
            }

            // **When each kill fired**, read off the clock by the kill
            // rather than off the delay the caller asked for
            // (`PR5-R5-001`). A kill cannot fire before the wait that
            // precedes it has elapsed, so with the ladder above this pins N
            // firings to N distinct, strictly increasing floors: the i-th
            // kill fired later than the rung the (i-1)-th was aimed at, and
            // no two of them can be the spawn instant. That is the
            // strongest statement available deterministically — a *ceiling*
            // on a firing would be an assertion about the scheduler — and
            // it is what the deleted wait destroys, since every kill then
            // fires within microseconds of its spawn.
            for launch in &shape {
                let fired = launch
                    .fired
                    .expect("every child was fired at, asserted just above");
                assert!(
                    fired >= launch.after,
                    "{label}: a kill fired {fired:?} after its child was spawned, sooner \
                         than the {:?} rung it was aimed at — a kill that does not wait its \
                         rung is the spawn instant sampled again, and the ladder above is \
                         then a ladder of nothing",
                    launch.after
                );
            }

            // How many of those N fired kills won their race with the
            // command. Machine-varying, so printed rather than asserted —
            // the same treatment, and for the same reason, as the class
            // histogram above. `git add` has measured 1/8 on Linux and on
            // the Windows guest and 2/8 on that same guest a run later; see
            // the floor below for why that number is reported, not required.
            let killed = shape
                .iter()
                .filter(|launch| launch.end == LaunchEnd::Killed)
                .count();
            println!("kill sampling {label}: killed {killed}/{launched}");
        }

        // **The kill itself** (`PR5-R4-001`), and the one assertion in this
        // test that a completed run does not also satisfy.
        //
        // Until this existed nothing here could tell a killed child from a
        // finished one. With `child.kill()` deleted the sampler still
        // spawned 4 × N children, `SAMPLED_LAUNCHES` still counted them,
        // every residue still classified into a legal class, recovery was
        // still idempotent and `effects/residue-histogram.json` was still
        // written and read back — recording *completion* residue under the
        // kill's name. Only the wait status changes, so only the wait
        // status can be the oracle.
        //
        // **Over the sampling as a whole, not per sample and not per
        // shape.** Per sample is wrong because a child that reaches its own
        // exit before its rung elapses is legal: the last rung is 8/9 of a
        // measured uninterrupted run and is meant to reach past the end of
        // a fast one. Per shape is wrong for a subtler reason — `git add`
        // measures **1** kill in 8 on Linux and on the Windows guest, and
        // **2** on that guest a run later, because `measure_budget`'s probe
        // writes the 1 200 blobs its samples then find already in the object
        // store. (The number moving between runs is the point, not a
        // correction to it: a floor set at either one is a floor on the
        // wrong side of the other on some machine.) Measured
        // outside this suite, same content in three worktrees of one
        // repository: 44 ms for the first `git add -A`, 10 ms and 9 ms for
        // the next two, with the loose-object count unmoved at 1 203. So
        // the samples run in about a fifth of the run their ladder is
        // scaled to, only the shortest rung lands inside them, and a
        // per-shape floor would be an assertion standing on a margin of one
        // sample — a red suite on the next machine rather than a stronger
        // proof. The per-shape counts are printed above so that margin
        // stays visible without being load-bearing.
        //
        // What discharges the **four-command** clause is therefore not this
        // floor — that was `PR5-R5-002` — but the per-shape firing count
        // above. The two are a pair, and each covers what the other cannot:
        // the firing count proves no command was skipped and could not tell
        // a kill from a call that recorded itself and threw its effect
        // away; this floor proves the kills are real and cannot tell which
        // of the four commands they reached.
        let killed = log
            .iter()
            .filter(|launch| launch.end == LaunchEnd::Killed)
            .count();
        // The floor is over every launch, so when it fails no shape killed
        // anything and each launch is one of a failing shape: each is
        // printed, its argv's first word naming its shape.
        assert!(
            killed > 0,
            "not one of the {} sampled Git children died by the kill — this harness \
                 sampled the residue its commands left when they FINISHED, and every \
                 other assertion in this test accepts that residue. Ends: {:?}\n\
                 Launches:\n{}",
            log.len(),
            log.iter().map(|launch| launch.end).collect::<Vec<_>>(),
            log.iter()
                .map(|launch| format!(
                    "  {}: after {:?}, fired {:?}, kill_error {:?}, end {:?}, group_ended {:?}",
                    launch.argv[0],
                    launch.after,
                    launch.fired,
                    launch.kill_error,
                    launch.end,
                    launch.group_ended
                ))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    // The evidence file `outputs` asks for, written and then read back.
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(crate::effects::RESIDUE_HISTOGRAM_JSON);
    let emitted = serde_json::to_string_pretty(&serde_json::json!({
        "note": "decisions.effect_site_inventory.outputs, the observed-class \
                     histogram half: written by \
                     workspace_manager::tests::sampled_git_child_kills_every_residue_\
                     classified_and_recovered on every run. Machine-varying by \
                     construction -- which class a sample lands in is a race between \
                     the kill and Git -- so it is emitted here rather than pinned into \
                     effects/residue-classes.json, which carries the declarations.",
        "sampling_n": SAMPLING_N,
        "sites": records
            .iter()
            .map(|(site, record, budget, replayed)| serde_json::json!({
                "site": site.name(),
                "n": record.n,
                // The timescale the kill ladder was cut from. A red run's
                // artifact carries it, and `UPSTROKE_RESIDUE_BUDGET_US` feeds it
                // back -- which is what makes this sampler reproducible, since
                // it has no seed and this duration is the only variance a
                // replay can pin. Wake times, Git's own progress, cache state
                // and scheduling still vary between runs.
                "budget_us": u64::try_from(budget.as_micros()).unwrap_or(u64::MAX),
                "budget_replayed": replayed,
                "none": record.histogram.none,
                "internal": record.histogram.internal,
                "after": record.histogram.after,
                "unclassified": record.unclassified,
                "recovered": record.recovered,
            }))
            .collect::<Vec<_>>(),
    }))
    .expect("the histogram serializes");
    fs::write(&path, emitted + "\n").expect("write the observed-class histogram");

    let back: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).expect("read the histogram back"))
            .expect("the emitted histogram parses");
    let sites = back["sites"].as_array().expect("a sites array");
    assert_eq!(sites.len(), 4, "one histogram per sampled site");
    for (entry, (site, record, budget, _)) in sites.iter().zip(&records) {
        assert_eq!(
            entry["site"],
            site.name(),
            "the sites are in sampling order"
        );
        let total = ["none", "internal", "after"]
            .iter()
            .map(|class| entry[*class].as_u64().expect("a count"))
            .sum::<u64>();
        assert_eq!(
            total,
            u64::from(SAMPLING_N),
            "{site}: the written histogram accounts for every sample"
        );
        assert_eq!(entry["unclassified"], record.unclassified);
        assert_eq!(
            entry["budget_us"].as_u64(),
            u64::try_from(budget.as_micros()).ok(),
            "{site}: the artifact must carry the timescale a replay needs, or a \
                 red CI run cannot be reproduced from it"
        );
    }
}

/// The MSYS2 environment directories Git for Windows installs its real git
/// under, any of which can be the `<prefix>` of `git --exec-path`.
pub(super) const MINGW_PREFIXES: [&str; 4] = ["mingw64", "ucrt64", "clangarm64", "mingw32"];

/// Whether `program` is Git for Windows' real git, `<MinGW prefix>\bin\git.exe`,
/// rather than a launcher. It splits on both `\` and `/` itself, not through
/// [`Path`]'s separator, so a Windows path reads the same on every platform and
/// the mixed separators `git --exec-path` leads to are accepted.
///
/// It decides on the path's encoded bytes, not on a lossy string:
/// `standards/08_standards_filesystems_persistence_paths.md` allows those for
/// diagnostics only, and this is an identity decision. Both separators are
/// ASCII, and an ASCII byte in an encoded `OsStr` is always that character, so
/// splitting the bytes at them splits the path. `git.exe` and `bin` must match
/// exactly; the prefix matches without regard to ASCII case.
pub(super) fn is_mingw_real_git(program: &Path) -> bool {
    let mut components = program
        .as_os_str()
        .as_encoded_bytes()
        .rsplit(|&byte| byte == b'\\' || byte == b'/')
        .filter(|part| !part.is_empty());
    components.next() == Some(b"git.exe".as_slice())
        && components.next() == Some(b"bin".as_slice())
        && components.next().is_some_and(|prefix| {
            MINGW_PREFIXES
                .iter()
                .any(|mingw| prefix.eq_ignore_ascii_case(mingw.as_bytes()))
        })
}

/// Whether the census sampler may run `program` as its git, on a Windows host
/// when `windows` is set: there only Git for Windows' real git
/// ([`is_mingw_real_git`]) may be sampled, because killing a launcher leaves the
/// real git running (`PR249-KILL-SAMPLER-WINDOWS-WRAPPER`). Elsewhere any
/// program may. [`SampledChild::program`] asserts this with `cfg!(windows)`;
/// taking the host as an argument lets a test ask about a Windows host on every
/// platform.
pub(super) fn may_sample_as_git(program: &Path, windows: bool) -> bool {
    !windows || is_mingw_real_git(program)
}

/// One site's sampling run: the packet's record, and the per-sample
/// observations it was tallied from.
///
/// The two are carried separately because the record alone cannot be
/// checked (`PR5-CONF-004`). `histogram.internal += 1` → `histogram.none +=
/// 1` survived the whole suite: which bucket a sample lands in is a race, so
/// no assertion on the *counts* can catch a swapped arm, and the only
/// available oracle is the classifier's own answers, tallied a second time
/// by something that is not the code under test.
struct SamplingRun {
    record: SamplingRecord,
    /// The timescale the kill ladder was cut from, and whether it was measured
    /// on this machine or replayed from `UPSTROKE_RESIDUE_BUDGET_US`. This is the
    /// only variance a replay can pin -- see [`measure_budget`] -- so it is the
    /// most a recorded run can offer. It is not *all* the variance: actual wake
    /// times, Git's progress, cache state and scheduling still differ, so a
    /// replay reproduces the nominal ladder rather than the original run.
    budget: std::time::Duration,
    replayed: bool,
    /// What `classify_object_residue` answered for each sample, in order.
    /// `None` is a sample it could not classify at all.
    observed: Vec<Option<ObjectResidue>>,
    /// What the classifier said when it *refused* a sample, one line each.
    ///
    /// A refusal and an unclassifiable residue both arrive in `observed` as
    /// `None`, and the assertion they fail is worded for the second of them.
    /// The words that tell the two apart are here, so a red run says which it
    /// met instead of naming only the one it probably did not. Each is
    /// followed by that sample's [`report_sample`].
    refusals: Vec<String>,
    /// The [`report_sample`] of each sample whose recovery ran and left its
    /// slot or registration behind (`record.recovered` false), taken before
    /// the recovery ran, one entry each.
    unrecovered: Vec<String>,
}

/// Tally per-sample observations into the packet's histogram.
///
/// The single place the mapping from class to bucket is written, so the test
/// can check it against an independent tally of the same list.
fn tally(observed: &[Option<ObjectResidue>]) -> (ClassHistogram, u32) {
    let mut histogram = ClassHistogram::default();
    let mut unclassified = 0;
    for sample in observed {
        match sample {
            Some(ObjectResidue::None) => histogram.none += 1,
            Some(ObjectResidue::Internal) => histogram.internal += 1,
            Some(ObjectResidue::After) => histogram.after += 1,
            None => unclassified += 1,
        }
    }
    (histogram, unclassified)
}

/// Kill the Git child of one site `SAMPLING_N` times and classify what is
/// left.
fn sample_site(site: EffectSiteId) -> SamplingRun {
    let tag = format!("sample-{}", site.variant().to_lowercase());
    let fixture = Fixture::created(&tag);
    let base = fixture.base.clone();
    let measured = measure_budget(site, &fixture);
    let replayed = replayed_budget(site);
    let budget = replayed.unwrap_or(measured);
    println!(
        "residue sampling {site}: budget={}us{} ladder={:?}",
        budget.as_micros(),
        if replayed.is_some() {
            format!(" (replayed; measured {}us)", measured.as_micros())
        } else {
            String::new()
        },
        (0..SAMPLING_N)
            .map(|run| budget
                .mul_f64(f64::from(run + 1) / f64::from(SAMPLING_N + 1))
                .as_micros())
            .collect::<Vec<_>>()
    );
    let mut observed: Vec<Option<ObjectResidue>> = Vec::new();
    let mut refusals: Vec<String> = Vec::new();
    let mut unrecovered: Vec<String> = Vec::new();
    let mut recovered = true;

    for run in 0..SAMPLING_N {
        let slot = sample_slot(site, &fixture, run);
        fixture
            .manager
            .write_intent(&mut NoHooks, &slot)
            .expect("intent");
        let path = fixture.manager.slot_path(&slot);
        if site != EffectSiteId::Worktree(WorktreeSite::Add) {
            fixture
                .manager
                .add_worktree(&mut NoHooks, &slot, &fixture.head)
                .expect("worktree");
            populate_for_sampling(site, &path);
        }

        let (args, cwd) = sampled_command(site, &fixture, &slot);
        let delay = budget.mul_f64(f64::from(run + 1) / f64::from(SAMPLING_N + 1));
        let kill = kill_git_child(&cwd, &args, delay);

        let target = ResidueTarget::new(&base).at(&path).from_base(&fixture.head);
        // `.ok()` is what the tally needs, and it is not what a reader of a
        // red run needs: it folds "the classifier refused, and here is why"
        // into the same `None` as "this residue is in no class", and the
        // assertion those `None`s fail is worded for the second. §7 asks that
        // a discard be best-effort with defined observability, so the error's
        // own words are kept and go into that assertion's message.
        let classified = classify_object_residue(site, &target);
        let refusal = classified.as_ref().err().map(ToString::to_string);
        // Taken here, while the slot still exists: the recovery below
        // deletes it. Printed only by a refusal or a failed recovery.
        let report = report_sample(&fixture, &path, &kill, refusal.as_deref());
        if let Some(error) = &refusal {
            refusals.push(format!("run {run}: {error}\n{report}"));
        }
        observed.push(classified.ok());
        match recover_sample(&fixture, &slot) {
            Ok(true) => {}
            Ok(false) => {
                recovered = false;
                unrecovered.push(format!("run {run}: {report}"));
            }
            Err(RecoveryRefused::Forced(error)) => {
                panic!("forced removal converges: {error:?}\n{site} run {run}: {report}")
            }
            Err(RecoveryRefused::Intent(error)) => {
                panic!("intent removal converges: {error:?}\n{site} run {run}: {report}")
            }
        }
    }

    let (histogram, unclassified) = tally(&observed);
    SamplingRun {
        budget,
        replayed: replayed.is_some(),
        record: SamplingRecord {
            n: SAMPLING_N,
            histogram,
            unclassified,
            recovered,
        },
        observed,
        refusals,
        unrecovered,
    }
}

pub(super) fn sample_slot(site: EffectSiteId, fixture: &Fixture, run: u32) -> Slot {
    match site {
        EffectSiteId::Object(ObjectSite::ProposalCherryPick) => Slot::Staging {
            sequence: u64::from(run),
        },
        _ => fixture.task("alpha", run),
    }
}

/// The exact command the site's funnel runs, and where it runs it.
pub(super) fn sampled_command(
    site: EffectSiteId,
    fixture: &Fixture,
    slot: &Slot,
) -> (Vec<String>, PathBuf) {
    let path = fixture.manager.slot_path(slot);
    // Read from the funnel's own lists rather than transcribed from them
    // (Fable's `PR5-CONF-004`): the transcription was faithful and nothing
    // kept it so, and a funnel that grew a flag would leave the sampler
    // sampling a stale command with every assertion here still green.
    let fixed = |argv: &[&str]| -> Vec<String> { argv.iter().map(|a| (*a).to_owned()).collect() };
    match site {
        EffectSiteId::Object(ObjectSite::CandidateStage) => {
            (fixed(&WorkspaceManager::CANDIDATE_STAGE_ARGV), path)
        }
        EffectSiteId::Object(ObjectSite::CandidateWriteTree) => {
            (fixed(&WorkspaceManager::CANDIDATE_WRITE_TREE_ARGV), path)
        }
        EffectSiteId::Object(ObjectSite::ProposalCherryPick) => {
            let mut argv = fixed(&WorkspaceManager::PROPOSAL_CHERRY_PICK_ARGV);
            argv.push(fixture.side.clone());
            (argv, path)
        }
        EffectSiteId::Worktree(WorktreeSite::Add) => {
            let mut argv = fixed(&WorkspaceManager::WORKTREE_ADD_ARGV);
            argv.push(path.to_string_lossy().into_owned());
            argv.push(fixture.head.clone());
            (argv, fixture.base.clone())
        }
        other => panic!("`{other}` is not one of the four commands the contract samples"),
    }
}

/// How long the same command takes when nothing kills it.
///
/// Measured in a **probe slot of its own**, which is then removed. The
/// first draft measured it in the very worktree the next sample would kill
/// in, and the probe therefore *performed* the command first: `write-tree`
/// found a valid cache-tree and every one of its eight samples classified
/// `None`, which read as a stable histogram and was an artefact of the
/// fixture. A probe that mutates the state under test is the
/// "environment assumption in a test" class this project has recorded.
fn measure_budget(site: EffectSiteId, fixture: &Fixture) -> std::time::Duration {
    let probe = match site {
        EffectSiteId::Object(ObjectSite::ProposalCherryPick) => Slot::Staging { sequence: 9_999 },
        _ => fixture.task("probe", 9_999),
    };
    fixture
        .manager
        .write_intent(&mut NoHooks, &probe)
        .expect("probe intent");
    let path = fixture.manager.slot_path(&probe);
    let elapsed = if site == EffectSiteId::Worktree(WorktreeSite::Add) {
        let (args, cwd) = sampled_command(site, fixture, &probe);
        let start = std::time::Instant::now();
        let output = git_out(&cwd, &args.iter().map(String::as_str).collect::<Vec<_>>());
        assert!(output.status.success(), "the probe must really run");
        start.elapsed()
    } else {
        fixture
            .manager
            .add_worktree(&mut NoHooks, &probe, &fixture.head)
            .expect("probe worktree");
        populate_for_sampling(site, &path);
        let (args, cwd) = sampled_command(site, fixture, &probe);
        let start = std::time::Instant::now();
        let output = git_out(&cwd, &args.iter().map(String::as_str).collect::<Vec<_>>());
        assert!(
            output.status.success(),
            "the probe must really run: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        start.elapsed()
    };
    fixture
        .manager
        .remove_worktree(&mut NoHooks, &probe)
        .expect("remove the probe");
    fixture
        .manager
        .remove_intent(&mut NoHooks, &probe)
        .expect("remove the probe intent");
    elapsed.max(std::time::Duration::from_micros(200))
}

/// A recorded timescale to replay instead of measuring one.
///
/// **There is no random number generator in this sampler and nothing to seed.**
/// The kill ladder is a fixed set of fractions — `(run + 1) / (SAMPLING_N + 1)` —
/// of a single duration, and that duration is how long one real `git` took on
/// this machine at this moment. Load, page cache and disk move it; the fractions
/// never move. So the measured budget is the only variance a replay can pin,
/// and pinning it is the nearest thing to a reproducible failure available here
/// -- the ladder becomes identical, though the wake times it produces and Git's
/// own progress against them still vary run to run.
///
/// A fixed default would be worse than the status quo: the point of re-measuring
/// on every run is that the ladder lands in different places on different
/// machines, which is how a sampler with `SAMPLING_N = 8` covers more than eight
/// kill points over its lifetime. The measurement stays; it is merely recorded,
/// and overridable when replaying one specific red run.
/// The sites this sampler drives, and therefore the only sites a replay spec
/// may name.
///
/// Shared with [`parse_budget_spec`] so the two cannot disagree. Validating
/// against the whole `EffectSiteId` registry was not enough: it accepted
/// `Object.CandidateCommitTree` — a real site nothing here samples — whereupon
/// every site fell through to a fresh measurement and the run reported success
/// having replayed nothing. That is the same fail-open the strictness was added
/// to remove, wearing a name that passes validation.
pub(super) const SAMPLED_SITES: [EffectSiteId; 4] = [
    EffectSiteId::Object(ObjectSite::CandidateStage),
    EffectSiteId::Object(ObjectSite::CandidateWriteTree),
    EffectSiteId::Object(ObjectSite::ProposalCherryPick),
    EffectSiteId::Worktree(WorktreeSite::Add),
];

/// Why a `UPSTROKE_RESIDUE_BUDGET_US` spec was refused.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum BudgetSpecError {
    /// An entry with no `=`.
    Malformed(String),
    /// A site name no `EffectSiteId` answers to. Almost always a typo — and a
    /// typo is **indistinguishable from "leave this site to measure"** unless
    /// every entry is validated, which is why this is an error rather than a
    /// miss.
    UnknownSite(String),
    /// A real site that this sampler never drives. Distinct from
    /// [`Self::UnknownSite`] because the fix is different: the name is spelled
    /// correctly and simply cannot be replayed, which the message should say
    /// rather than claiming it does not exist.
    NotSampled(String),
    /// A value that is not a positive whole number of microseconds.
    NotAPositiveInteger(String),
    /// A value past [`MAX_REPLAY_BUDGET_US`].
    AboveCeiling { site: String, micros: u64 },
    /// The same site named twice, where which one wins would depend on order.
    Duplicate(String),
}

/// The largest replay budget a spec may ask for.
///
/// Four sites each sleep a ladder summing to about four budgets, so the whole
/// sampling costs roughly `16 x budget`. At five seconds that is 80 seconds of
/// sleeping — generous, since the largest budget ever *measured* here is about
/// 45 ms, and bounded, which unbounded input was not: `u64::MAX` parsed happily
/// and asked its first rung to sleep about 64,949 years, ending only at CI's
/// job timeout.
pub(super) const MAX_REPLAY_BUDGET_US: u64 = 5_000_000;

/// Parse a replay spec, validating **every** entry, and return the budget for
/// `site` if the spec names it.
///
/// Takes the spec as an argument rather than reading the environment, so it can
/// be unit tested without mutating process-global state that parallel tests
/// share. [`replayed_budget`] does the environment read at the edge.
pub(super) fn parse_budget_spec(
    raw: &str,
    site: EffectSiteId,
) -> Result<Option<std::time::Duration>, BudgetSpecError> {
    let mut found = None;
    let mut seen: Vec<String> = Vec::new();
    for entry in raw
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
    {
        let (name, micros) = entry
            .split_once('=')
            .ok_or_else(|| BudgetSpecError::Malformed(entry.to_owned()))?;
        let (name, micros) = (name.trim(), micros.trim());
        // Validated against the real site registry, not a list of the four this
        // test samples, so a name that is merely misspelled is caught the same
        // way as one that does not exist at all.
        let named = EffectSiteId::from_name(name)
            .map_err(|_| BudgetSpecError::UnknownSite(name.to_owned()))?;
        if !SAMPLED_SITES.contains(&named) {
            return Err(BudgetSpecError::NotSampled(name.to_owned()));
        }
        if seen.iter().any(|already| already == name) {
            return Err(BudgetSpecError::Duplicate(name.to_owned()));
        }
        seen.push(name.to_owned());
        let value = micros
            .parse::<u64>()
            .ok()
            .filter(|value| *value > 0)
            .ok_or_else(|| BudgetSpecError::NotAPositiveInteger(entry.to_owned()))?;
        if value > MAX_REPLAY_BUDGET_US {
            return Err(BudgetSpecError::AboveCeiling {
                site: name.to_owned(),
                micros: value,
            });
        }
        if named == site {
            found = Some(std::time::Duration::from_micros(value));
        }
    }
    Ok(found)
}

/// Decide a replay from what the environment gave us.
///
/// Takes the [`std::env::var`] result rather than reading it, so the three
/// cases can be tested without mutating process-global state that parallel
/// tests share. The distinction matters: `var` reports **`NotPresent` and
/// `NotUnicode` as different errors**, and collapsing them made a spec that
/// was present but not valid UTF-8 look exactly like no spec at all — every
/// site measuring fresh while the adjacent promise says an unhonourable spec
/// is refused.
pub(super) fn budget_from_var(
    var: Result<String, std::env::VarError>,
    site: EffectSiteId,
) -> Option<std::time::Duration> {
    let raw = match var {
        Ok(raw) => raw,
        Err(std::env::VarError::NotPresent) => return None,
        Err(std::env::VarError::NotUnicode(raw)) => panic!(
            "UPSTROKE_RESIDUE_BUDGET_US is set but is not valid UTF-8 ({raw:?}), so it \
                 cannot be parsed. It will not be treated as unset."
        ),
    };
    match parse_budget_spec(&raw, site) {
        Ok(found) => found,
        Err(error) => panic!(
            "UPSTROKE_RESIDUE_BUDGET_US is not a spec this run can honour: {error:?}. \
                 Fix the spec or unset it; it will not be silently ignored."
        ),
    }
}

/// The environment read itself, kept to one line so nothing but the read is
/// untestable.
fn replayed_budget(site: EffectSiteId) -> Option<std::time::Duration> {
    budget_from_var(std::env::var("UPSTROKE_RESIDUE_BUDGET_US"), site)
}

/// Enough work in the worktree that the sampled command has a middle to be
/// killed in: many files across many directories, so `git add` writes many
/// blobs and `write-tree` writes many trees.
pub(super) fn populate_for_sampling(site: EffectSiteId, path: &Path) {
    if site == EffectSiteId::Object(ObjectSite::ProposalCherryPick) {
        return;
    }
    for directory in 0..60 {
        let bulk = path.join(format!("bulk{directory}"));
        fs::create_dir_all(&bulk).expect("bulk directory");
        for index in 0..20 {
            fs::write(
                bulk.join(format!("f{index}.txt")),
                format!("{directory}-{index}-{}", "x".repeat(2048)),
            )
            .expect("bulk file");
        }
    }
    if site == EffectSiteId::Object(ObjectSite::CandidateWriteTree) {
        // `write-tree` reads an index, so the bulk has to be in one.
        git(path, &["add", "-A"]);
    }
}

/// How one sampled Git child ended.
///
/// The wait status is the **only** thing in this harness that the kill
/// changes, so it is the only place the kill can be observed. Everything
/// else — the spawn, the argv, the residue, its class, the recovery, the
/// evidence file — is identical whether the child was killed or ran to its
/// own end, which is exactly why `PR5-R4-001` could delete `child.kill()`
/// and keep the suite green on both platforms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LaunchEnd {
    /// The status carries this platform's signature of [`Child::kill`].
    ///
    /// [`Child::kill`]: std::process::Child::kill
    Killed,
    /// The child reached its own **successful** exit before the kill
    /// landed. Legal — the delay ladder deliberately reaches past the end
    /// of a fast run — and the reason the kill floor is asserted over the
    /// sampling as a whole rather than per sample.
    Completed,
    /// Neither: the sampled command failed on its own. Then what the
    /// classifier saw is the *fixture's* residue, not the kill's, and no
    /// count of kills over these samples means anything.
    Failed(Option<i32>),
}

/// Read a [`LaunchEnd`] off a wait status.
///
/// The signature is a **value** per platform, not the property
/// `!status.success()`: a command that merely failed also fails to succeed,
/// and reading that as a kill is how a kill-count keeps counting after the
/// kill is gone. The third arm exists so that such a command reddens the
/// suite instead of being miscounted.
pub(super) fn launch_end(status: &std::process::ExitStatus) -> LaunchEnd {
    // The fingerprint is `fixture::died_by_kill`, not a second copy of it.
    // This file carried its own per-platform arms, byte-for-byte the ones in
    // `fixture.rs`, and `src/engine/topology/attempt/tests.rs` reads the
    // shared one: two spellings of one platform fact drift apart at whichever
    // of them a new platform is taught first, and the half that is not taught
    // goes on counting kills after the kill is gone.
    if died_by_kill(status) {
        LaunchEnd::Killed
    } else if status.success() {
        LaunchEnd::Completed
    } else {
        LaunchEnd::Failed(status.code())
    }
}

/// One Git child the sampler launched: what it ran, which rung of the delay
/// ladder its kill was **aimed at**, when a kill actually **fired** at it,
/// and how it ended.
///
/// `after` and `fired` are two different things, and `PR5-R5-001` is the
/// difference. `after` is the caller's parameter: it is recorded whatever
/// the sampler does with it, so a ladder of `after`s is a ladder of
/// *intentions* and stays a perfect one after the wait that realizes it is
/// deleted. `fired` is the clock, read inside [`SampledChild::kill`]: it
/// exists only if a kill ran at this child, and it moves when the wait
/// before the kill does.
///
/// `Clone` because [`kill_git_child`] hands the sample that launched the
/// child the same record it logs ([`SampleKill`]): the report of a failed
/// sample reads that, not the shared log by position.
#[derive(Clone)]
pub(super) struct SampledLaunch {
    argv: Vec<String>,
    after: std::time::Duration,
    fired: Option<std::time::Duration>,
    /// What [`SampledChild::kill`] answered, when it answered an error.
    ///
    /// **`fired` says a kill ran at this child; it does not say the kill
    /// worked** (PR #136 pass 1, finding 4). The result was discarded at the
    /// call site with `let _ =`, so a shape whose kills all *failed* while its
    /// children exited normally passed its per-shape checks — every child was
    /// fired at, none was `Failed` — and another shape's successful kills
    /// satisfied the one global floor. The suite could then certify killing all
    /// four command shapes with one of them never killed. §7 asks a discard to
    /// be best-effort with a defined failure path, and **the path is
    /// observability, not an assertion**: the value is recorded here and
    /// printed beside the per-shape counts. Pass 3 showed why it cannot be
    /// asserted on — see the note at that printing.
    kill_error: Option<String>,
    end: LaunchEnd,
    /// How long after the child's reap every process it started was gone
    /// ([`SampledChild::wait`]): on Unix its whole process group
    /// (`fixture::await_group_end`), on Windows its job, once the job's
    /// active process count read 0.
    group_ended: std::time::Duration,
}

/// Every Git child the sampler actually launched, in order.
///
/// The independent observer of *launches and of their kills*.
/// `command_internal_sub_effects` freezes N per site and
/// `slice_contract.proof_tests[8]` names four commands, and both are claims
/// about what was spawned — while every assertion in the sampling test is
/// over `run.observed`, the list the loop **pushes to**, and over the
/// residues that list classifies into. Nothing counted a spawn. A run that
/// skipped one kill and pushed its observation anyway satisfied the length
/// assertion, the histogram total and the serialized `sampling_n` alike;
/// and a site that spawned another site's command left every count, class
/// and evidence record identical, because any Git child that leaves a
/// classifiable residue in the slot satisfies them all. Both were measured
/// surviving the whole suite.
///
/// Counting launches is only the first half, and `PR5-R4-001` is the
/// second: both live passages say the child is **killed**, and with
/// `child.kill()` deleted the sampler still spawned 4 × N children, still
/// classified a legal residue from each, still recovered and still wrote
/// the histogram — of *completion* residue, filed under the kill's name.
/// So each entry also carries how its child ended and at which rung of the
/// ladder the kill fired.
///
/// It has to be collected **here** and not at the call site: the edit that
/// drops a kill skips the call, so an observer beside the call would still
/// run and would count a launch that never happened.
///
/// Round 5 stopped one level short of that same rule, and `PR5-R5-001` and
/// `PR5-R5-002` are what it cost. Inside this function the *launch* is
/// observed and the *kill* was not: the parameter the kill was aimed at was
/// recorded beside the call, so deleting the wait left the record intact,
/// and the record was pushed after `wait()` for every child, so skipping the
/// kill for one of the four commands left the record intact too. The kill's
/// own record therefore has to be collected inside the kill, which is what
/// [`SampledChild`] is for.
static SAMPLED_LAUNCHES: std::sync::Mutex<Vec<SampledLaunch>> = std::sync::Mutex::new(Vec::new());

/// The Git child the sampler kills, wrapped so that **the kill records
/// itself**.
///
/// [`Self::kill`] is inherent, so `child.kill()` in `kill_git_child` is this
/// method and not [`Child::kill`], and the note that a kill ran is written
/// by the statement that runs it rather than beside it. Both of round 5's
/// surviving mutations are edits that stop a kill happening — one deletes
/// the wait before it, one skips it for a single command — and both walked
/// past records that were written whether the kill happened or not.
///
/// It is deliberately **blind to the command it is killing**: no argv
/// reaches it, so the per-command firing count in the sampling test cannot
/// be defeated inside this type without first giving it one. The program it
/// records ([`Self::spawned_program`]) is not that: it is the same `git` for
/// all four commands, so it tells none of them from another.
///
/// **Its kill reaches every process git starts, on each platform.** On Unix
/// the child leads a process group of its own, which the kill ends whole
/// (`fixture::kill_process_group`) and the wait awaits empty
/// (`fixture::await_group_end`). On Windows it runs in a private job of its
/// own (`agent::proc::ChildInJob`): created suspended and assigned to the job
/// before its first instruction runs, so every process it starts is in the
/// job; the kill terminates the job and the wait awaits it empty. There the
/// kill was `Child::kill` until this, which ends the direct child alone,
/// while `git worktree add`'s `reset --hard` and the children of the other
/// commands ran on. The job is closed, with
/// `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, when this drops.
///
/// [`Child::kill`]: std::process::Child::kill
pub(super) struct SampledChild {
    /// The child. On Unix the leader of a process group of its own
    /// ([`Self::command`]); on Windows in a private job of its own, which
    /// derefs to the [`std::process::Child`] ([`Self::spawn_command`]).
    #[cfg(not(windows))]
    child: std::process::Child,
    #[cfg(windows)]
    child: crate::agent::proc::ChildInJob,
    /// Started once [`Command::spawn`] has returned, so what [`Self::kill`]
    /// reads off it is time the child was left *running* rather than the
    /// cost of starting it — and so an unwaited kill reads as the ~0 it is.
    spawned: std::time::Instant,
    /// The program the [`Command`] whose `spawn()` returned [`Self::child`]
    /// ran, read off that very `Command` with [`Command::get_program`]: the
    /// program only, never its arguments. Written only by [`Self::spawn`].
    spawned_program: PathBuf,
    /// What the clock said when a kill fired at this child, or `None` if
    /// none ever did. Written only by [`Self::kill`].
    fired: Option<std::time::Duration>,
    /// How long after the child's reap every process it started was gone —
    /// its process group on Unix, its job on Windows — or `None` until
    /// [`Self::wait`] has run.
    group_ended: Option<std::time::Duration>,
}

impl SampledChild {
    /// The program the sampler runs: [`super::fixture::sampled_git`]'s
    /// `git`, as `fixture::KillableGitChild`'s is. On Windows that is the
    /// real `<MinGW prefix>\bin\git.exe` (`mingw64`, or `ucrt64` on newer
    /// images) rather than Git for Windows' `cmd\git.exe` launcher, whose
    /// kill would leave the real git running
    /// (`PR249-KILL-SAMPLER-WINDOWS-WRAPPER`); `git` by name everywhere else.
    ///
    /// It fails closed, on [`may_sample_as_git`]: on Windows this panics
    /// unless the program is the real git. `sampled_git` falls back to `git`
    /// when `git --exec-path` does not lead to the real binary, and that `git`
    /// is the launcher; an inherited `GIT_EXEC_PATH=<root>\libexec\git-core`
    /// leads it to `<root>\bin\git.exe`, which is a launcher too.
    ///
    /// What the tests cannot show is stated here. Off Windows, `program()` is
    /// `git` by name. A spawn that ran `git` by name instead would look the
    /// same there, so only a Windows run can catch that edit. Where git
    /// resolves, nothing shows that `program()` consults the decision. Where
    /// it does not resolve, both Windows-side tests fail on the same
    /// predicate. A real git outside the four MinGW prefixes is refused.
    fn program() -> &'static Path {
        let program = super::fixture::sampled_git();
        assert!(
            may_sample_as_git(program, cfg!(windows)),
            "the census kill sampler needs Git for Windows' real <MinGW prefix>\\bin\\git.exe \
             (mingw64, ucrt64, clangarm64 or mingw32), and `fixture::sampled_git` resolved {program:?}: \
             killing a launcher would leave the real git running (PR249-KILL-SAMPLER-WINDOWS-WRAPPER)"
        );
        program
    }

    /// The command [`Self::spawn`] starts: `program -C cwd -c
    /// core.fsmonitor=false <args>`, its streams discarded, in a process
    /// group of its own on Unix (on Windows the job is
    /// [`Self::spawn_command`]'s). Taking `program` as an argument lets a
    /// test show that what runs is the program given, on every platform.
    fn command(program: &Path, cwd: &Path, args: &[String]) -> Command {
        let mut command = Command::new(program);
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
        command
    }

    /// Spawn the child in a process group of its own on Unix and in a
    /// private job of its own on Windows, so that [`Self::kill`] reaches the
    /// children git starts — `git worktree add`'s `reset --hard` — and not
    /// the leader alone. Until PR10's round 5 this sampler killed the leader
    /// alone on Unix (round 2's group kill reached
    /// `fixture::KillableGitChild`, the other samplers' child), and CI's
    /// macOS leg at `869e336a` met that child still populating the checkout
    /// — `PR136`'s first fingerprint — at the forced removal. On Windows it
    /// killed the leader alone until the job.
    fn spawn(cwd: &Path, args: &[String]) -> Self {
        Self::spawn_command(Self::command(Self::program(), cwd, args))
    }

    /// Spawn `command` as the sampled child: as it is on Unix, where
    /// [`Self::command`] gave it a process group of its own; on Windows
    /// suspended, assigned to a new private job before its first instruction
    /// runs and only then resumed (`agent::proc::ChildInJob::spawn`), so that
    /// every process it starts is in the job. Apart from [`Self::spawn`] so
    /// that a test can hand it a command whose streams it changed.
    fn spawn_command(mut command: Command) -> Self {
        // Read off the `Command` that spawns the child, before the spawn, so
        // the clock read below stays the first thing after it.
        let spawned_program = PathBuf::from(command.get_program());
        #[cfg(not(windows))]
        let child = command.spawn().expect("spawn the sampled git child");
        #[cfg(windows)]
        let child = crate::agent::proc::ChildInJob::spawn(&mut command)
            .expect("spawn the sampled git child in a private job");
        Self {
            child,
            spawned: std::time::Instant::now(),
            spawned_program,
            fired: None,
            group_ended: None,
        }
    }

    /// Kill the child and every process it started, recording when the kill
    /// fired. On Unix that is `SIGKILL` to its process group
    /// (`fixture::kill_process_group`). On Windows it is `TerminateJobObject`
    /// on its job, with exit code 1 for every process in it, the code
    /// `Child::kill` gives too and the one `fixture::died_by_kill` reads as a
    /// kill there. Neither waits: [`Self::wait`] does.
    ///
    /// The clock is read at the instant of the kill and stored *after* the
    /// kill has returned. Read at that instant, the value is the kill's
    /// actual timing rather than the timing it was asked for; stored after
    /// the call, the record cannot outlive the call's removal, because
    /// deleting the call that kills leaves `outcome` unbound and the module
    /// stops compiling. The kill's own answer is returned and recorded by the
    /// caller; nothing here decides what it means.
    ///
    /// What that still leaves reachable is a fake that keeps the call and
    /// throws its effect away. The kill floor at the end of the sampling
    /// test is what covers that: it is over wait statuses, which nothing
    /// but a real kill produces.
    fn kill(&mut self) -> std::io::Result<()> {
        let fired = self.spawned.elapsed();
        #[cfg(not(windows))]
        let outcome = super::fixture::kill_process_group(&mut self.child);
        #[cfg(windows)]
        let outcome = self.child.terminate();
        self.fired = Some(fired);
        outcome
    }

    /// Reap the child, then wait until every process it started is gone,
    /// bounded by `fixture::GROUP_END_BOUND`, and record how long that took
    /// in [`Self::group_ended`]: the proof `recover_sample` runs under is
    /// that no writer the child started is alive, and the child's reap alone
    /// is not that proof. On Unix that is its whole process group
    /// (`fixture::await_group_end`); on Windows its job, polled until the
    /// job's active process count is 0 (`Self::await_job_end`). Past the
    /// bound it panics, in the same words on both.
    fn wait(&mut self) -> std::io::Result<std::process::ExitStatus> {
        let status = self.child.wait()?;
        #[cfg(not(windows))]
        let ended = super::fixture::await_group_end(self.pgid());
        #[cfg(windows)]
        let ended = self.await_job_end();
        self.group_ended = Some(ended.unwrap_or_else(|outlived| {
            panic!("the sampled git child's group outlived its leader: {outlived}")
        }));
        Ok(status)
    }

    /// How long after the child's reap its job had no active process, polled
    /// once a millisecond as `fixture::await_group_end` polls a group, or
    /// what the job still held when `fixture::GROUP_END_BOUND` ran out. An
    /// accounting query that fails is answered at once, and never as an
    /// empty job.
    #[cfg(windows)]
    fn await_job_end(&self) -> Result<std::time::Duration, String> {
        let started = std::time::Instant::now();
        let pid = self.child.id();
        loop {
            let active = self.child.active_processes().map_err(|error| {
                format!(
                    "the job of pid {pid} could not be counted {:?} after its leader was \
                     reaped: {error}",
                    started.elapsed()
                )
            })?;
            if active == 0 {
                return Ok(started.elapsed());
            }
            let waited = started.elapsed();
            if waited >= super::fixture::GROUP_END_BOUND {
                return Err(format!(
                    "the job of pid {pid} still had {active} active processes {waited:?} after \
                     its leader was reaped (the bound is {:?})",
                    super::fixture::GROUP_END_BOUND
                ));
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    /// The child's pid, which on Unix is also its process group's id:
    /// [`Self::command`] makes it the leader of a group of its own. On
    /// Windows it is the pid alone, which the report names beside the job.
    fn pgid(&self) -> i32 {
        i32::try_from(self.child.id()).expect("a pid fits in i32")
    }
}

/// What [`kill_git_child`] recorded for the one child it launched, handed
/// back to the sample that launched it: the very record it pushed to
/// [`SAMPLED_LAUNCHES`], and the child itself, reaped.
pub(super) struct SampleKill {
    launch: SampledLaunch,
    /// The reaped child, kept so that [`report_sample`] can ask whether a
    /// process of the sample is alive ([`sample_alive`]): its process group
    /// on Unix, its job on Windows. The job stays open, and so can be
    /// counted, until this drops.
    child: SampledChild,
}

fn kill_git_child(cwd: &Path, args: &[String], after: std::time::Duration) -> SampleKill {
    let mut child = SampledChild::spawn(cwd, args);
    std::thread::sleep(after);
    // Kept, not discarded. A kill can answer an error without anything being
    // wrong -- while the Windows kill was `Child::kill`, `TerminateProcess`
    // answered `ERROR_ACCESS_DENIED` for a child that had already exited --
    // so this is not an `expect`; what it is is evidence, and the per-shape
    // assertion pairs it with how the child ended.
    let kill_error = child.kill().err().map(|error| error.to_string());
    // Reaped rather than discarded: this status is the whole observation.
    let status = child.wait().expect("reap the sampled git child");
    let launch = SampledLaunch {
        argv: args.to_vec(),
        after,
        fired: child.fired,
        kill_error,
        end: launch_end(&status),
        group_ended: child.group_ended.unwrap_or_default(),
    };
    SAMPLED_LAUNCHES
        .lock()
        .expect("the launch log")
        .push(launch.clone());
    SampleKill { launch, child }
}

/// Which of [`recover_sample`]'s two removals refused, with its error.
///
/// Returned rather than panicked on inside the recovery, so that the panic
/// `sample_site` raises can carry the report it took before the recovery
/// deleted the slot.
#[derive(Debug)]
pub(super) enum RecoveryRefused {
    Forced(UpstrokeError),
    Intent(UpstrokeError),
}

/// The tabled recovery for whatever the sample left: forced removal of the
/// worktree and its intent, which is the before-phase action every
/// `Internal` residue routes to and is idempotent for the other two.
///
/// It runs under [`WriterProof::NoWriterAlive`], which [`SampledChild::wait`]
/// established on each platform before it: on Unix the child's process group
/// awaited empty, on Windows its job, until no process in it was active.
///
/// `Ok` says whether the slot and its registration are gone; `Err` is a
/// removal that refused.
pub(super) fn recover_sample(fixture: &Fixture, slot: &Slot) -> Result<bool, RecoveryRefused> {
    // Every process the sampled child started is gone before this runs —
    // the child reaped and everything it started, `git worktree add`'s own
    // `reset --hard` among them, dead. On Unix that is its whole process
    // group, every member dead and reaped (`fixture::await_group_end`; until
    // PR10's round 5 the leader alone was killed and reaped here, and CI's
    // macOS leg at `869e336a` met the member still writing: `PR136`'s first
    // fingerprint). On Windows it is its job, terminated whole and polled
    // until its active process count was 0 (`SampledChild::wait`); until the
    // job the direct child alone was killed there, and a writer it started
    // could outlive the reap. So no writer of the execution root is alive:
    // the proof under which a registration the kill left naming nothing is
    // passed over rather than refused.
    fixture
        .manager
        .remove_worktree_proving(&mut NoHooks, slot, WriterProof::NoWriterAlive)
        .map_err(RecoveryRefused::Forced)?;
    fixture
        .manager
        .remove_intent(&mut NoHooks, slot)
        .map_err(RecoveryRefused::Intent)?;
    let path = fixture.manager.slot_path(slot);
    Ok(!path.exists()
        && !fixture
            .manager
            .worktree_records()
            .expect("records")
            .iter()
            .any(|record| canonical_prefix(record.path()).ok() == canonical_prefix(&path).ok()))
}

/// The most of a sample's `HEAD` reflog [`report_sample`] prints: all of a
/// reflog this long or shorter, and the last this many bytes of a longer one.
pub(super) const REPORTED_REFLOG_BYTES: usize = 4096;

/// What one sample left, in words a red run can be read by
/// (`CI-FLAKE-WINDOWS-SAMPLER-FSCK-TORN-REFLOG`, whose guard asks for it).
///
/// That finding holds three readings of a classifier refusal the logs could
/// not tell apart — the kill reached Git mid-append; it reached only part of
/// Git's tree, which ran on; the kill never landed — because each sample's
/// kill error and [`LaunchEnd`] sat in [`SAMPLED_LAUNCHES`], read only after
/// every site passed, and nothing recorded the reflog or whether a process of
/// the sample was alive. So the report holds:
///
/// 1. what [`kill_git_child`] recorded for this sample's own child: when the
///    kill fired against the rung it was aimed at, its error or none, its
///    [`LaunchEnd`] and how long its group outlived the leader's reap;
/// 2. whether a process of the sample is alive now ([`sample_alive`]);
/// 3. the bytes of the slot's `HEAD` reflog ([`head_reflog`]);
/// 4. for each object id `refusal` names after `invalid reflog entry `,
///    whether the fixture's base repository has it ([`object_presence`]).
///
/// `sample_site` takes it once per sample, after the classifier and before
/// the recovery, which deletes the slot; it prints it only when the
/// classifier refused or the recovery failed, so a green run prints what it
/// printed before. It reads and never writes: `git rev-parse`, a file read,
/// `kill(-pgid, 0)` on Unix or the job's accounting on Windows, and
/// `git cat-file -e`.
pub(super) fn report_sample(
    fixture: &Fixture,
    slot: &Path,
    kill: &SampleKill,
    refusal: Option<&str>,
) -> String {
    // First, so "now" is as near the classifier's run as the report gets.
    let alive = sample_alive(&kill.child);
    let launch = &kill.launch;
    let fired = match launch.fired {
        Some(fired) if fired >= launch.after => format!(
            "fired {fired:?}, {:?} past the {:?} rung it was aimed at",
            fired - launch.after,
            launch.after
        ),
        Some(fired) => format!(
            "fired {fired:?}, {:?} BEFORE the {:?} rung it was aimed at",
            launch.after - fired,
            launch.after
        ),
        None => format!("never fired (aimed at {:?})", launch.after),
    };
    let mut lines = vec![
        format!("report of the sample at {}:", slot.display()),
        format!(
            "  kill: {fired}; kill error: {}; end: {:?}; group gone {:?} after the leader's reap",
            launch.kill_error.as_deref().unwrap_or("none"),
            launch.end,
            launch.group_ended
        ),
        format!("  alive: {alive}"),
        format!("  HEAD reflog: {}", head_reflog(slot)),
    ];
    for id in refusal.map(reflog_entry_ids).unwrap_or_default() {
        lines.push(format!(
            "  object {id}: {}",
            object_presence(&fixture.base, &id)
        ));
    }
    lines.join("\n")
}

/// Whether a process of the sample whose child is `child` is alive at this
/// moment, in [`report_sample`]'s words.
///
/// On Unix the child led a group of its own, and every process Git started
/// is in it, so the question is [`super::fixture::group_has_a_member`]'s.
/// On Windows the child ran in a job of its own, and every process Git
/// started is in that, so the answer is the job's active process count; a
/// count that cannot be read is said as that, never as no.
pub(super) fn sample_alive(child: &SampledChild) -> String {
    let pgid = child.pgid();
    #[cfg(unix)]
    {
        if super::fixture::group_has_a_member(pgid) {
            format!("yes: process group {pgid} still has a member")
        } else {
            format!("no: process group {pgid} has no member")
        }
    }
    #[cfg(windows)]
    {
        match child.child.active_processes() {
            Ok(0) => format!("no: the job of pid {pgid} has an active process count of 0"),
            Ok(active) => {
                format!("yes: the job of pid {pgid} has an active process count of {active}")
            }
            Err(error) => {
                format!("not answered: the job of pid {pgid} could not be counted: {error}")
            }
        }
    }
}

/// The bytes of the `HEAD` reflog of the worktree at `slot`, located with
/// `git rev-parse --git-path logs/HEAD` run there, printed with
/// `escape_ascii` so a torn line shows exactly as it is: all of it up to
/// [`REPORTED_REFLOG_BYTES`], else its last that many. "absent" when the slot
/// or the file does not exist; any other failure is said as itself.
///
/// A slot with no `.git` of its own — a killed `worktree add` can leave one —
/// is "absent" too, before Git is asked: Git would search the directories
/// above it and could answer with another repository's reflog.
pub(super) fn head_reflog(slot: &Path) -> String {
    match slot.try_exists() {
        Ok(true) => {}
        Ok(false) => return format!("absent: there is no slot at {}", slot.display()),
        Err(error) => return format!("not read: the slot could not be inspected: {error}"),
    }
    match fs::symlink_metadata(slot.join(".git")) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return format!("absent: the slot at {} has no .git", slot.display());
        }
        Err(error) => return format!("not read: the slot's .git could not be inspected: {error}"),
    }
    let output = git_out(slot, &["rev-parse", "--git-path", "logs/HEAD"]);
    if !output.status.success() {
        return format!(
            "not located: `git rev-parse --git-path logs/HEAD` ended {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    // Git's answer stays bytes, less the run of `\r` and `\n` that ends it: a
    // path is never assumed to be UTF-8 (§8), and on Unix any bytes are one.
    let mut located = output.stdout.as_slice();
    while let [rest @ .., b'\r' | b'\n'] = located {
        located = rest;
    }
    let Ok(located) = super::parsers::decode_path(located) else {
        return "not located: `git rev-parse --git-path logs/HEAD` printed a path that is not \
                UTF-8"
            .to_owned();
    };
    // Relative to the directory it ran in when it is not absolute, and
    // `join` keeps an absolute one as it is.
    let file = slot.join(located);
    match fs::read(&file) {
        Ok(bytes) => {
            let shown = &bytes[bytes.len().saturating_sub(REPORTED_REFLOG_BYTES)..];
            let extent = if shown.len() == bytes.len() {
                format!("all {} bytes", bytes.len())
            } else {
                format!("the last {} of {} bytes", shown.len(), bytes.len())
            };
            format!(
                "{extent} of {}: \"{}\"",
                file.display(),
                shown.escape_ascii()
            )
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            format!("absent: there is no file at {}", file.display())
        }
        Err(error) => format!("not read: {}: {error}", file.display()),
    }
}

/// Every object id `refusal` names after `invalid reflog entry ` — fsck's
/// words for a reflog entry naming an object it did not find — once each, in
/// order: the hex digits that follow, when there are 40 (SHA-1) or 64
/// (SHA-256) of them.
pub(super) fn reflog_entry_ids(refusal: &str) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    for rest in refusal.split("invalid reflog entry ").skip(1) {
        let digits = rest.bytes().take_while(u8::is_ascii_hexdigit).count();
        // Hex digits are ASCII, so `digits` is a character boundary.
        let id = &rest[..digits];
        if matches!(digits, 40 | 64) && !ids.iter().any(|seen| seen == id) {
            ids.push(id.to_owned());
        }
    }
    ids
}

/// Whether `git cat-file -e <id>` finds the object in the repository at
/// `base`: "present" for its success, "missing" for its exit 1 with nothing
/// on stderr, which is how it answers a well-formed id it does not have, and
/// any other end said as itself.
pub(super) fn object_presence(base: &Path, id: &str) -> String {
    let output = git_out(base, &["cat-file", "-e", id]);
    if output.status.success() {
        return "present".to_owned();
    }
    if output.status.code() == Some(1) && output.stderr.is_empty() {
        return "missing".to_owned();
    }
    format!(
        "not answered: `git cat-file -e` ended {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr).trim()
    )
}

/// The real-time containment of a contract witness whose access runs on its
/// repository's [`super::RegistryClock`] (the record's §9.33, B16): a stated,
/// generous bound on wall time, checked at each step the witness sees and at
/// its end, and never its oracle. That clock moves only when the witness, or a
/// wait its access asks for, moves it, so an access that went round with
/// neither would never reach its deadline on it; this turns that into a
/// failure. It is wall time, so a stall of the host longer than it fails the
/// witness too.
pub(super) const CLOCK_CONTAINMENT: std::time::Duration = std::time::Duration::from_secs(60);

/// Fail a witness on a [`super::RegistryClock`] once [`CLOCK_CONTAINMENT`] of
/// wall time has passed since `since`, naming the `step` it had reached.
pub(super) fn contained(since: std::time::Instant, step: &str) {
    let ran = since.elapsed();
    assert!(
        ran < CLOCK_CONTAINMENT,
        "{step}: {ran:?} of wall time, past the containment of {CLOCK_CONTAINMENT:?}, which \
         is not this witness's oracle"
    );
}

// ---------------------------------------------------------------------------
// #329: two processes in one repository's registry (T1, T3, and the
// two-process forms of claim 1), each child a `LinkedChild` of this binary
// ---------------------------------------------------------------------------

/// The child of the two-process registry tests: one coordinator's registry
/// work in one checkout of the parent's repository. It takes that checkout's
/// worktree lock and its own run's lock, derives a manager over its own run,
/// says it is ready, waits for `GO`, runs `B329_CYCLES` snapshot add-and-remove
/// cycles through the production funnels, saying `B329_CYCLE` and the round
/// after each one ([`result_through_cycles`]), and reports how many failed and
/// each failure, on one line each. Its manager's accesses wait to the
/// production deadline (the record's §9.31, B14): they race the other
/// processes' registry writes.
#[test]
#[ignore = "linked child of the two-process registry tests"]
fn registry_cycles_child() {
    let link = super::fixture::ParentLink::attach();
    let var = |name: &str| std::env::var_os(name).unwrap_or_else(|| panic!("{name} is set"));
    let base = PathBuf::from(var("B329_BASE"));
    let private = PathBuf::from(var("B329_PRIVATE"));
    let number: u32 = var("B329_NUMBER")
        .to_string_lossy()
        .parse()
        .expect("a process number");
    let cycles: u32 = var("B329_CYCLES")
        .to_string_lossy()
        .parse()
        .expect("a cycle count");
    let run_id = format!("01KZB329{number:018}");
    let _worktree = crate::rundir::WorktreeLock::acquire(&base)
        .expect("this checkout's worktree lock: one coordinator per checkout");
    let public = crate::rundir::public_dir(&base, &run_id);
    fs::create_dir_all(&public).expect("the run's public directory");
    let _run = crate::rundir::RunLock::acquire(&public).expect("the run's lock");
    let manager = WorkspaceManager::derive(&base, &private, &run_id, &format!("inc-{number}"))
        .expect("a manager over this run, in the shared repository");
    let _deadline =
        super::RegistryDeadline::hold(manager.common_git_dir(), PRODUCTION_REGISTRY_DEADLINE);
    manager
        .create_execution_root(&mut NoHooks)
        .expect("the execution root");
    let head = oid(&git(&base, &["rev-parse", "HEAD"]));
    link.send(&format!("B329_READY {number}"));
    let go = link.recv_within(super::fixture::LINK_BOUND);
    assert_eq!(
        go.as_deref(),
        Some("GO"),
        "the parent starts every process at once"
    );
    let mut failures = Vec::new();
    for round in 1..=cycles {
        let name = SnapshotName::gates(number, 0, round);
        match manager.add_snapshot(&mut NoHooks, &name, &SnapshotInput::Commit(head.clone())) {
            Ok(snapshot) => {
                if let Err(error) = manager.remove_snapshot(&mut NoHooks, &snapshot) {
                    failures.push(format!("removing {name}: {error}"));
                }
            }
            Err(error) => failures.push(format!("adding {name}: {error}")),
        }
        link.send(&format!("B329_CYCLE {number} {round}"));
    }
    link.send(&format!(
        "B329_RESULT {number} cycles={cycles} failures={}",
        failures.len()
    ));
    for failure in &failures {
        link.send(&format!(
            "B329_FAILURE {number} {}",
            failure.replace(['\n', '\r'], " ")
        ));
    }
    link.send(&format!("B329_DONE {number}"));
}

/// The next line from a linked child that carries `prefix`, within the link's
/// bound; a panic carrying the child's stderr otherwise.
pub(super) fn line_from(
    lines: &std::sync::mpsc::Receiver<String>,
    prefix: &str,
    stderr: &Path,
) -> String {
    let deadline = std::time::Instant::now() + super::fixture::LINK_BOUND;
    loop {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        match lines.recv_timeout(left) {
            Ok(line) => {
                if let Some(at) = line.find(prefix) {
                    return line[at..].to_owned();
                }
            }
            Err(error) => panic!(
                "no `{prefix}` line from the linked child ({error}); its stderr:\n{}",
                fs::read_to_string(stderr).unwrap_or_default()
            ),
        }
    }
}

/// The child that writes one registration of the parent's repository the way
/// `git worktree add` does, file by file, and stops half way: the entry's
/// directory, `locked`, `gitdir`, the checkout's `.git`, `HEAD` written and
/// `commondir` opened and empty — the state Git leaves when it is killed while
/// it writes `commondir`. It says so and waits for its parent: told
/// `FINISH`, it writes `commondir` and unlinks `locked`, as the add does, and
/// ends; killed, it leaves the registration torn, as a dead writer does; and
/// told `WHOLE-UNLISTABLE`, it writes a whole registration Git cannot list (a
/// `HEAD` that names nothing) and ends, leaving a registration nobody writes.
#[test]
#[ignore = "linked child of the two-process registry tests"]
fn registry_writer_child() {
    let link = super::fixture::ParentLink::attach();
    let var = |name: &str| std::env::var_os(name).unwrap_or_else(|| panic!("{name} is set"));
    let admin = PathBuf::from(var("B329_ADMIN"));
    let checkout = PathBuf::from(var("B329_CHECKOUT"));
    let head = var("B329_HEAD").to_string_lossy().into_owned();
    fs::create_dir_all(&admin).expect("the entry's directory");
    fs::write(admin.join("locked"), "initializing\n").expect("its locked");
    fs::create_dir_all(&checkout).expect("its checkout");
    fs::write(
        admin.join("gitdir"),
        format!("{}\n", as_git_writes_it(&checkout.join(".git"))),
    )
    .expect("its gitdir");
    fs::write(
        checkout.join(".git"),
        format!("gitdir: {}\n", as_git_writes_it(&admin)),
    )
    .expect("the checkout's .git");
    fs::write(admin.join("HEAD"), format!("{head}\n")).expect("its HEAD");
    fs::write(admin.join("commondir"), b"").expect("its commondir, opened and not yet written");
    link.send("B329_TORN");
    match link.recv_within(super::fixture::LINK_BOUND).as_deref() {
        Some("FINISH") => {
            fs::write(admin.join("commondir"), "../..\n").expect("the writer writes commondir");
            fs::remove_file(admin.join("locked")).expect("and unlocks the entry");
            link.send("B329_FINISHED");
        }
        Some("WHOLE-UNLISTABLE") => {
            fs::write(admin.join("commondir"), "../..\n").expect("commondir");
            fs::write(admin.join("HEAD"), "garbage\n").expect("a HEAD that names nothing");
            fs::remove_file(admin.join("locked")).expect("unlocked");
            link.send("B329_WHOLE");
        }
        other => panic!("an instruction from the parent, not {other:?}"),
    }
}

/// The production registry deadline: `REGISTRY_ACCESS_DEADLINE`'s
/// `#[cfg(not(test))]` value, which a test build cannot name. A test holds it
/// for its repository ([`super::RegistryDeadline`]), as the coordinator's tear
/// witnesses hold `WITNESS_REGISTRY_DEADLINE`, when an access of its must be
/// attempted again before what the test is about can happen: its accesses race
/// real writers, or its own act ends the contention an access meets -- a
/// writer finishing the registration it tore, a remover, a restorer, a wait
/// made through its hooks. Under the test build's 500 ms one attempt the
/// platform makes slow can spend the whole deadline first, and the access
/// refuses with no second attempt (the record's §9.30.10 and §9.31). A test
/// whose tear stays, and whose oracle is the refusal at the deadline, keeps
/// the test build's.
pub(super) const PRODUCTION_REGISTRY_DEADLINE: std::time::Duration =
    std::time::Duration::from_secs(10);

// ---------------------------------------------------------------------------
// The bounded `LinkedChild` (PR328-LINKED-CHILD-KILL-WAITS-WITHOUT-A-DEADLINE)
// ---------------------------------------------------------------------------

/// The linked child the next test's kill cannot end: it ignores its link
/// closing, as a child busy elsewhere does, and ends when its parent does, or
/// after a minute.
#[cfg(target_os = "linux")]
#[test]
#[ignore = "linked child of linked_child_whose_kill_is_refused_child"]
fn linked_child_that_waits_on_its_parent_child() {
    let link = super::fixture::ParentLink::attach();
    link.send("B329_WAITING");
    // SAFETY: `getppid` takes no argument and cannot fail.
    let parent = unsafe { libc::getppid() };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    // SAFETY: as above.
    while std::time::Instant::now() < deadline && unsafe { libc::getppid() } == parent {
        super::fixture::rest(std::time::Duration::from_millis(10));
    }
}

/// The isolated caller: it starts a linked child, refuses `kill` on its own
/// thread with a seccomp policy so that `Child::kill` cannot end the child,
/// and kills it through `LinkedChild::kill`, then drops it, reporting how
/// each ended and after how long.
#[cfg(target_os = "linux")]
#[test]
#[ignore = "isolated caller of a_linked_childs_kill_fails_its_test_within_its_bound_rather_than_wait_for_a_child_its_kill_did_not_end"]
fn linked_child_whose_kill_is_refused_child() {
    let tree = scratch("linked-kill-refused");
    let stderr = tree.path().join("child.stderr");
    let (child, lines) = super::fixture::LinkedChild::spawn(
        "workspace_manager::tests::linked_child_that_waits_on_its_parent_child",
        &[],
        &stderr,
    );
    line_from(&lines, "B329_WAITING", &stderr);
    super::fixture::refuse_syscall_on_this_thread(libc::SYS_kill, libc::EPERM);
    let panic_text = |panic: Box<dyn std::any::Any + Send>| {
        panic
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| panic.downcast_ref::<&str>().map(|text| (*text).to_owned()))
            .unwrap_or_else(|| "a panic without a message".to_owned())
    };
    let started = std::time::Instant::now();
    let killed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| child.kill()));
    let took = started.elapsed();
    let how = match killed {
        Ok(status) => format!("returned {status:?}"),
        Err(panic) => format!("failed its test: {}", panic_text(panic)),
    };
    println!("\nthe kill ended after {} ms and {how}", took.as_millis());
    let started = std::time::Instant::now();
    let dropped = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(child)));
    let took = started.elapsed();
    let how = match dropped {
        Ok(()) => "returned".to_owned(),
        Err(panic) => format!("failed its test: {}", panic_text(panic)),
    };
    println!("\nthe drop ended after {} ms and {how}", took.as_millis());
}

// -----------------------------------------------------------------------
// PR11 follow-up C: a slot instance per coordinator incarnation (U)
// -----------------------------------------------------------------------
//
// The record is `48f2af7a:reviews/2026-10-02-pr11-follow-up-c-record.md`: §4.2's tag,
// §4.3's discovery, §4.5's refusing variant, §4.6's replacements for the two
// frozen oracles that U makes vacuous, §4.7's untagged names, §5.2 and §5.3's
// switches and §5.4's final sweep. Its "Implementation" section names each
// test here with the mutation that turns it red.

/// A manager of the fixture's run under `incarnation`: an earlier or a later
/// coordinator of the same run, over the same repository and private root.
pub(super) fn of_incarnation(fixture: &Fixture, incarnation: &str) -> WorkspaceManager {
    WorkspaceManager::derive(
        &fixture.base,
        &fixture.private,
        super::fixture::RUN_ID,
        incarnation,
    )
    .expect("derive a manager of the fixture's run under another incarnation")
}

/// One instance an oracle tracks: its checkout, its registration's
/// administrative directory, captured by `gitdir` before anything ran, and
/// its intent.
pub(super) struct Tracked {
    checkout: PathBuf,
    admin: PathBuf,
    intent: PathBuf,
}

/// Answers `Error` at the `stop`-th phase it sees, and records every phase.
pub(super) struct StopAt {
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

/// P-2's helper: a dead incarnation in a process of its own. It adopts the
/// parent's fixture, derives its manager under the incarnation the parent
/// names, writes the slot's intent and dies inside the add at the phase the
/// parent names, by `std::process::abort` (`Injection::Kill`), as the frozen
/// recovery tests' children die at theirs.
#[test]
#[ignore = "subprocess helper"]
fn instance_kill_child() {
    let (Some(root), Some(incarnation), Some(phase), Some(kind)) = (
        std::env::var_os(INSTANCE_KILL_ROOT),
        std::env::var(INSTANCE_KILL_INCARNATION).ok(),
        std::env::var(INSTANCE_KILL_PHASE).ok(),
        std::env::var(INSTANCE_KILL_SLOT).ok(),
    ) else {
        return;
    };
    let fixture = Fixture::adopt(PathBuf::from(root));
    let manager = of_incarnation(&fixture, &incarnation);
    let slot = match kind.as_str() {
        "task" => fixture.task("alpha", 1),
        _ => Slot::Snapshot {
            name: SnapshotName::integration(7),
        },
    };
    let head = fixture.head.clone();
    // The parent owns the tree; this process dies without its destructor.
    std::mem::forget(fixture);

    struct KillAt(HookPhase);
    impl EffectHooks for KillAt {
        fn phase(&mut self, site: EffectSiteId, phase: HookPhase) -> Injection {
            if matches!(
                site,
                EffectSiteId::Worktree(WorktreeSite::Add)
                    | EffectSiteId::Snapshot(SnapshotSite::Add)
            ) && phase == self.0
            {
                Injection::Kill
            } else {
                Injection::Proceed
            }
        }

        fn refusal_cause(&self) -> Option<String> {
            None
        }
    }
    let at = if phase == "before" {
        HookPhase::Before
    } else {
        HookPhase::After
    };
    manager
        .write_intent(&mut NoHooks, &slot)
        .expect("the dead incarnation's intent");
    let _ = manager.add_worktree(&mut KillAt(at), &slot, &head);
    panic!("the add's funnel returned past the kill armed at its {phase} phase");
}

pub(super) const INSTANCE_KILL_ROOT: &str = "UPSTROKE_PR330_INSTANCE_KILL_ROOT";

pub(super) const INSTANCE_KILL_INCARNATION: &str = "UPSTROKE_PR330_INSTANCE_KILL_INCARNATION";

pub(super) const INSTANCE_KILL_PHASE: &str = "UPSTROKE_PR330_INSTANCE_KILL_PHASE";

pub(super) const INSTANCE_KILL_SLOT: &str = "UPSTROKE_PR330_INSTANCE_KILL_SLOT";

#[cfg(unix)]
pub(super) const LATE_ADD_ROOT: &str = "UPSTROKE_PR330_LATE_ADD_ROOT";

#[cfg(unix)]
pub(super) const LATE_ADD_HOLD: &str = "UPSTROKE_PR330_LATE_ADD_HOLD";

/// P-3's helper: the dead incarnation, in a process whose `PATH` starts with
/// a `git` stub that holds the first `worktree add` before the real Git runs,
/// until the parent releases it. The incarnation writes its intent and calls
/// the production add; the add's Git runs late, after the successor's walk.
#[cfg(unix)]
#[test]
#[ignore = "subprocess helper"]
fn late_add_child() {
    let (Some(root), Some(_hold)) = (
        std::env::var_os(LATE_ADD_ROOT),
        std::env::var_os(LATE_ADD_HOLD),
    ) else {
        return;
    };
    let fixture = Fixture::adopt(PathBuf::from(root));
    let dead = of_incarnation(&fixture, "inc-0");
    let slot = fixture.task("alpha", 1);
    let head = fixture.head.clone();
    std::mem::forget(fixture);
    dead.write_intent(&mut NoHooks, &slot)
        .expect("the dead incarnation's intent");
    match dead.add_worktree(&mut NoHooks, &slot, &head) {
        Ok(path) => println!("late add: added {}", path.display()),
        Err(error) => println!("late add: refused: {error}"),
    }
}

/// Collect each case's outcome before asserting, so that a red run reports
/// every case's answer and not only the first.
pub(super) fn report_cases(cases: &[(String, Result<(), String>)]) {
    let failed: Vec<String> = cases
        .iter()
        .filter_map(|(label, outcome)| outcome.as_ref().err().map(|why| format!("{label}: {why}")))
        .collect();
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

/// The scratch root of a [`linked_registry_guarded_reads_child`], given to it
/// alone.
#[cfg(unix)]
pub(super) const F_LINKED_ROOT: &str = "UPSTROKE_TEST_F_LINKED_ROOT";

/// Where the managed base sits below that root.
#[cfg(unix)]
pub(super) const F_LINKED_BASE: &str = "UPSTROKE_TEST_F_LINKED_BASE";

/// Which sequence the child runs: `C4`, `C5`, or `C4 with nothing inherited`.
#[cfg(unix)]
pub(super) const F_LINKED_SEQUENCE: &str = "UPSTROKE_TEST_F_LINKED_SEQUENCE";

/// The child of
/// [`a_linked_registry_keeps_the_managers_guarded_reads_under_the_inherited_common_dir`]:
/// one sequence, under the names its environment inherited, through the
/// manager's own API. In the sequence with nothing inherited it plants the
/// registry's link itself, after `derive` and before the slot is added.
#[cfg(unix)]
#[test]
#[ignore = "subprocess helper"]
fn linked_registry_guarded_reads_child() {
    let Some(sequence) = std::env::var_os(F_LINKED_SEQUENCE) else {
        return;
    };
    let sequence = sequence.to_string_lossy().into_owned();
    let root = PathBuf::from(std::env::var_os(F_LINKED_ROOT).expect("the scratch root"));
    let below = PathBuf::from(std::env::var_os(F_LINKED_BASE).expect("the base below the root"));
    let base = root.join(&below);
    let head = git(&base, &["rev-parse", "HEAD"]);
    let manager = WorkspaceManager::derive(
        &base,
        &root.join("private"),
        super::fixture::RUN_ID,
        "inc-linked",
    )
    .unwrap_or_else(|error| panic!("{sequence}: derive: {error}"));
    if sequence == "C4 with nothing inherited" {
        super::fixture::link_directory(
            &root.join("other").join(".git").join("worktrees"),
            &base.join(".git").join("worktrees"),
        );
    }
    assert_eq!(
        manager.common_git_dir(),
        fs::canonicalize(base.join(".git"))
            .expect("the run's common directory")
            .as_path(),
        "{sequence}: the run's common directory is the base's own"
    );
    manager
        .create_execution_root(&mut NoHooks)
        .expect("the execution root");
    let slot = Slot::Task {
        key: "linked".to_owned(),
        generation: 1,
    };
    manager
        .write_intent(&mut NoHooks, &slot)
        .expect("the slot's intent");
    let path = manager
        .add_worktree(&mut NoHooks, &slot, &head)
        .unwrap_or_else(|error| panic!("{sequence}: the slot add: {error}"));
    let pointer = fs::read_to_string(path.join(".git")).expect("the slot's .git");
    let entry = fs::canonicalize(
        pointer
            .trim()
            .strip_prefix("gitdir:")
            .expect("a gitdir pointer")
            .trim(),
    )
    .expect("the slot's registration entry");
    assert!(
        entry.starts_with(
            fs::canonicalize(root.join("other").join(".git").join("worktrees"))
                .expect("the other repository's store")
        ),
        "{sequence}: the slot's registration is in the linked store: {}",
        entry.display()
    );
    write_file(&path.join("worker.txt"), b"a file the worker made\n");
    let staged = manager.candidate_stage(&mut NoHooks, &slot, &[]);
    if sequence == "C4 with nothing inherited" {
        match staged {
            Err(UpstrokeError::RegistryRefused { message })
                if message.contains("which is not this repository's common git dir") => {}
            other => panic!(
                "{sequence}: with nothing inherited the guarded read refuses by the original \
                 rule: {other:?}"
            ),
        }
        return;
    }
    staged.unwrap_or_else(|error| panic!("{sequence}: the capture stages: {error}"));
    let captured = manager
        .candidate_write_tree(&mut NoHooks, &slot)
        .unwrap_or_else(|error| panic!("{sequence}: the capture writes its tree: {error}"));
    assert_eq!(
        git(&base, &["ls-tree", "--name-only", &captured]),
        "a.txt\nworker.txt",
        "{sequence}: the capture holds the slot's work"
    );
    let kept = manager
        .kept_instances(&mut NoHooks, &slot)
        .unwrap_or_else(|error| panic!("{sequence}: the kept-instance read: {error}"));
    assert!(
        kept.is_empty(),
        "{sequence}: the populated slot's registration reads whole: {kept:?}"
    );
}

/// `target`, which [`link_store_to`] moved the store to, as a manager derived
/// over the link pins it: canonical, with no verbatim prefix.
fn pinned_target(target: &Path) -> PathBuf {
    strip_verbatim(fs::canonicalize(target).expect("the canonical target"))
}

/// The variables a pin child reads, each set by its parent to the `OsStr` it
/// holds: the scratch root of the parent's fixture, its managed base and
/// private root, the case the child runs, and the run's canonical common
/// directory, which the child's `GIT_COMMON_DIR` names.
pub(super) const PIN_ROOT: &str = "UPSTROKE_TEST_PIN_ROOT";

pub(super) const PIN_BASE: &str = "UPSTROKE_TEST_PIN_BASE";

pub(super) const PIN_PRIVATE: &str = "UPSTROKE_TEST_PIN_PRIVATE";

pub(super) const PIN_CASE: &str = "UPSTROKE_TEST_PIN_CASE";

pub(super) const PIN_COMMON: &str = "UPSTROKE_TEST_PIN_COMMON";

/// The identity of the snapshot T-PIN-1's parent added, handed to its child:
/// its name, its checkout and the commit its HEAD names, as the handle's own
/// getters return them, and the handle as `Debug` renders it.
pub(super) const PIN_SNAPSHOT_NAME: &str = "UPSTROKE_TEST_PIN_SNAPSHOT_NAME";

pub(super) const PIN_SNAPSHOT_PATH: &str = "UPSTROKE_TEST_PIN_SNAPSHOT_PATH";

pub(super) const PIN_SNAPSHOT_HEAD: &str = "UPSTROKE_TEST_PIN_SNAPSHOT_HEAD";

pub(super) const PIN_SNAPSHOT_DEBUG: &str = "UPSTROKE_TEST_PIN_SNAPSHOT_DEBUG";

/// What a pin child is handed ([`PIN_ROOT`] and the rest), each value read as
/// the `OsString` its parent set, none converted. Every one is required: a pin
/// child runs only under its parent, and one started without them fails rather
/// than passing with nothing asserted.
struct PinChild {
    root: PathBuf,
    base: PathBuf,
    private: PathBuf,
    case: OsString,
    common: PathBuf,
}

impl PinChild {
    /// The handoff, once this process's `GIT_COMMON_DIR` is found to name the
    /// handed common directory exactly, as an absolute path: the configuration
    /// R5's start check admits for a store linked out of it (R-C-REGISTRY part
    /// 2).
    fn handed() -> Self {
        let child = Self {
            root: PathBuf::from(pin_variable(PIN_ROOT)),
            base: PathBuf::from(pin_variable(PIN_BASE)),
            private: PathBuf::from(pin_variable(PIN_PRIVATE)),
            case: pin_variable(PIN_CASE),
            common: PathBuf::from(pin_variable(PIN_COMMON)),
        };
        assert!(
            child.common.is_absolute(),
            "the run's common directory is handed as an absolute path: {}",
            child.common.display()
        );
        assert_eq!(
            std::env::var_os(COMMON_DIR).as_deref(),
            Some(child.common.as_os_str()),
            "the child inherits the run's canonical common directory as its `GIT_COMMON_DIR`"
        );
        child
    }

    /// The entry of `cases` whose label is the case the parent named, matched
    /// exactly against the `OsString` it set.
    fn case_of<T: Copy, const N: usize>(&self, cases: [(&'static str, T); N]) -> (&'static str, T) {
        cases
            .into_iter()
            .find(|(label, _)| self.case.as_os_str() == OsStr::new(label))
            .unwrap_or_else(|| panic!("a case its parent runs, not {:?}", self.case))
    }

    /// The manager the parent's fixture holds, run `RUN_ID` and incarnation
    /// `inc-1` over its base and private root, derived here, where the store is
    /// the link the parent planted. `what` is why it must derive, and its common
    /// git dir must be the handed one.
    fn derive(&self, what: &str) -> WorkspaceManager {
        let manager =
            WorkspaceManager::derive(&self.base, &self.private, super::fixture::RUN_ID, "inc-1")
                .expect(what);
        assert_eq!(
            manager.common_git_dir(),
            self.common.as_path(),
            "the manager is derived over the run's canonical common directory"
        );
        manager
    }

    /// The snapshot T-PIN-1's parent added, rebuilt here from the identity it
    /// handed over ([`PIN_SNAPSHOT_NAME`] and the rest) through the funnel's own
    /// `Snapshot::new` and `SnapshotHead::Existing`, and checked against that
    /// snapshot: `name` is built by the constructor the parent's add was given,
    /// and the rebuilt handle's getters return the name, checkout and HEAD the
    /// parent's handle returned, its HEAD the existing commit the add was given.
    /// The rebuilt handle is also the parent's field for field, as `Debug`
    /// renders both in this one binary.
    fn snapshot(&self, name: SnapshotName, tag: &str) -> Snapshot {
        let path = PathBuf::from(pin_variable(PIN_SNAPSHOT_PATH));
        let head = pin_variable(PIN_SNAPSHOT_HEAD)
            .into_string()
            .unwrap_or_else(|head| panic!("{tag}: an object id is ASCII: {head:?}"));
        let rebuilt = Snapshot::new(name, path.clone(), SnapshotHead::Existing(oid(&head)));
        let Slot::Snapshot { name } = rebuilt.slot() else {
            panic!("{tag}: a snapshot's slot is a snapshot slot: {rebuilt:?}");
        };
        assert!(
            pin_variable(PIN_SNAPSHOT_NAME).as_os_str() == OsStr::new(name.as_str())
                && rebuilt.path() == path
                && rebuilt.head().as_str() == head
                && rebuilt.ephemeral().is_none(),
            "{tag}: the rebuilt handle names the snapshot the parent's add returned, its checkout \
             and its HEAD, the existing commit the add was given: {rebuilt:?}"
        );
        let rendered = pin_variable(PIN_SNAPSHOT_DEBUG)
            .into_string()
            .unwrap_or_else(|rendered| panic!("{tag}: a rendered handle is UTF-8: {rendered:?}"));
        assert_eq!(
            format!("{rebuilt:?}"),
            rendered,
            "{tag}: the rebuilt handle is the parent's, field for field"
        );
        rebuilt
    }
}

/// Pin variable `name`, as its parent set it.
fn pin_variable(name: &str) -> OsString {
    std::env::var_os(name).unwrap_or_else(|| panic!("{name} is set by the pin child's parent"))
}

/// The store is still a link, and still resolves to the pinned `target`.
fn assert_pinned_link(store: &Path, target: &Path, tag: &str) {
    let metadata = fs::symlink_metadata(store)
        .unwrap_or_else(|error| panic!("{tag}: the store's link is kept: {error}"));
    assert!(
        is_reparse_point(&metadata),
        "{tag}: the store is still a link"
    );
    assert_eq!(
        strip_verbatim(fs::canonicalize(store).expect("the store resolves")),
        target,
        "{tag}: and it still resolves to the pinned target"
    );
}

/// A copy of the registration entry `from` at `into`: its files byte for byte
/// and its subdirectories by name, which is everything [`snapshot`] compares.
pub(super) fn copy_entry(from: &Path, into: &Path) {
    fs::create_dir_all(into).expect("the copy's directory");
    for entry in fs::read_dir(from).expect("list the entry") {
        let entry = entry.expect("an entry");
        let kind = entry.file_type().expect("its type");
        if kind.is_file() {
            fs::copy(entry.path(), into.join(entry.file_name())).expect("copy a file");
        } else if kind.is_dir() {
            fs::create_dir_all(into.join(entry.file_name())).expect("a subdirectory");
        }
    }
}

/// What a test makes of a pinned store.
#[derive(Debug, Clone, Copy)]
pub(super) enum StoreLoss {
    /// The link removed.
    Removed,
    /// The link replaced by a plain directory holding a copy of the bound
    /// entry.
    PlainDirectory,
    /// The link re-pointed to `substitute`, holding a copy of the bound entry.
    Repointed,
}

/// `loss` made of `store` now. A plain directory, or the substitute a
/// re-pointed link names, holds a copy of `entry`, the bound registration,
/// under its own name.
fn lose_store(store: &Path, entry: &Path, loss: StoreLoss, substitute: &Path) {
    let name = entry.file_name().expect("an entry name");
    super::fixture::unlink_directory(store);
    match loss {
        StoreLoss::Removed => {}
        StoreLoss::PlainDirectory => copy_entry(entry, &store.join(name)),
        StoreLoss::Repointed => {
            copy_entry(entry, &substitute.join(name));
            plant_directory_link(substitute, store);
        }
    }
}

/// [`lose_store`] at the removal's `Before` hook: after its scan bound the
/// registration through the pinned link, before anything is removed.
struct LoseStoreAtBefore {
    site: EffectSiteId,
    store: PathBuf,
    entry: PathBuf,
    loss: StoreLoss,
    substitute: PathBuf,
}

impl EffectHooks for LoseStoreAtBefore {
    fn phase(&mut self, site: EffectSiteId, phase: HookPhase) -> Injection {
        if site == self.site && phase == HookPhase::Before {
            lose_store(&self.store, &self.entry, self.loss, &self.substitute);
        }
        Injection::Proceed
    }

    fn refusal_cause(&self) -> Option<String> {
        None
    }
}

/// T-PIN-1's two layouts, each with whether its target is another
/// repository's store.
pub(super) const PINNED_REMOVAL_LAYOUTS: [(&str, bool); 2] =
    [("pinned-repository", true), ("pinned-directory", false)];

/// Where T-PIN-1 moves the store: the store of the repository at
/// `<root>/other`, or `<root>/any-directory`.
pub(super) fn pinned_removal_target(root: &Path, repository: bool) -> PathBuf {
    if repository {
        root.join("other").join(".git").join("worktrees")
    } else {
        root.join("any-directory")
    }
}

/// The child of
/// [`a_store_linked_before_derive_removes_only_its_bound_registrations_and_keeps_link_and_target`]:
/// the layout its parent names, over the store its parent linked. It derives
/// the manager under the inherited value, then makes everything T-PIN-1 made
/// after its derivation, in the same order, and every one of its assertions.
/// The snapshot is the one its parent's `add_snapshot` returned, rebuilt
/// from the identity the parent handed over and checked against it
/// ([`PinChild::snapshot`]), and removed through `remove_snapshot`.
#[test]
#[ignore = "linked child of the pinned-store tests"]
fn pinned_store_removals_child() {
    let handed = PinChild::handed();
    let (tag, repository) = handed.case_of(PINNED_REMOVAL_LAYOUTS);
    let manager = handed.derive("derive the manager over the linked store");
    let store = manager.common_git_dir().join("worktrees");
    let target = pinned_target(&pinned_removal_target(&handed.root, repository));
    let gate = handed.snapshot(SnapshotName::gates(0, 1, 1), tag);
    assert_eq!(
        gate.path(),
        manager.slot_path(gate.slot()).as_path(),
        "{tag}: this manager places the snapshot's slot at the snapshot's checkout"
    );
    let alpha = Slot::Task {
        key: "alpha".to_owned(),
        generation: 1,
    };
    let beta = Slot::Task {
        key: "beta".to_owned(),
        generation: 1,
    };
    let alpha_checkout = manager.slot_path(&alpha);
    let beta_checkout = manager.slot_path(&beta);
    let entry_of = |checkout: &Path| target.join(checkout.file_name().expect("a checkout name"));
    let alpha_entry = entry_of(alpha_checkout.as_path());
    let gate_entry = entry_of(gate.path());
    let beta_entry = entry_of(beta_checkout.as_path());
    for entry in [&alpha_entry, &gate_entry, &beta_entry] {
        assert!(
            entry.join("gitdir").is_file(),
            "{tag}: the registration moved with its store, below the target: {}",
            entry.display()
        );
    }
    let foreign = target.join("foreign");
    fs::create_dir_all(&foreign).expect("a registration no removal binds");
    let elsewhere = handed.root.join("elsewhere").join(".git");
    fs::write(foreign.join("gitdir"), format!("{}\n", elsewhere.display()))
        .expect("its gitdir, an absolute path on every platform");
    fs::write(foreign.join("locked"), b"another repository's\n").expect("its locked");
    let foreign_before = snapshot(&foreign);

    manager
        .remove_snapshot(&mut NoHooks, &gate)
        .unwrap_or_else(|error| panic!("{tag}: the snapshot's removal: {error}"));
    manager
        .remove_worktree(&mut NoHooks, &alpha)
        .unwrap_or_else(|error| panic!("{tag}: alpha's removal: {error}"));
    for (checkout, entry) in [
        (gate.path(), gate_entry.as_path()),
        (alpha_checkout.as_path(), alpha_entry.as_path()),
    ] {
        assert!(
            !checkout.exists() && !entry.exists(),
            "{tag}: {} and its registration are removed",
            checkout.display()
        );
    }
    assert_eq!(
        snapshot(&foreign),
        foreign_before,
        "{tag}: the registration no removal bound is untouched"
    );
    assert_pinned_link(&store, &target, tag);

    fs::remove_dir_all(&foreign).expect("the foreign registration goes by hand");
    manager
        .remove_worktree(&mut NoHooks, &beta)
        .unwrap_or_else(|error| panic!("{tag}: beta's removal: {error}"));
    assert!(!beta_checkout.exists(), "{tag}: beta is removed");
    assert!(
        fs::read_dir(&target)
            .unwrap_or_else(|error| panic!("{tag}: the emptied target is kept: {error}"))
            .next()
            .is_none(),
        "{tag}: the last removal emptied the target and left it"
    );
    assert_pinned_link(&store, &target, tag);
}

/// T-PIN-2's cases: what is made of the pinned store, whether at the removal's
/// `Before` hook rather than before its scan, and what the refusal must say the
/// store is now.
pub(super) const PIN_LOSS_CASES: [(&str, (StoreLoss, bool, &str)); 5] = [
    (
        "re-pointed at the hook",
        (StoreLoss::Repointed, true, "now resolves to"),
    ),
    (
        "a plain directory at the hook",
        (
            StoreLoss::PlainDirectory,
            true,
            "is now a plain directory, not a link",
        ),
    ),
    ("removed at the hook", (StoreLoss::Removed, true, "is gone")),
    (
        "re-pointed before the scan",
        (StoreLoss::Repointed, false, "now resolves to"),
    ),
    (
        "a plain directory before the scan",
        (
            StoreLoss::PlainDirectory,
            false,
            "is now a plain directory, not a link",
        ),
    ),
];

/// The child of
/// [`a_pinned_store_lost_before_its_removal_refuses_before_the_checkout_is_touched`]:
/// the case its parent names, over the store its parent linked. It derives the
/// manager under the inherited value, then makes everything T-PIN-2 made after
/// its derivation, in the same order, the loss and the removal among them, and
/// every one of its assertions.
#[test]
#[ignore = "linked child of the pinned-store tests"]
fn pinned_store_lost_child() {
    let handed = PinChild::handed();
    let (case, (loss, at_hook, found)) = handed.case_of(PIN_LOSS_CASES);
    let manager = handed.derive("derive the manager over the linked store");
    let slot = Slot::Task {
        key: "alpha".to_owned(),
        generation: 1,
    };
    let checkout = manager.slot_path(&slot);
    let store = manager.common_git_dir().join("worktrees");
    let target = pinned_target(&handed.root.join("pinned"));
    write_file(&checkout.join("work.txt"), b"the attempt's work\n");
    let entry = target.join(checkout.file_name().expect("a checkout name"));
    fs::write(entry.join("locked"), b"held\n").expect("a locked marker");
    let entry_before = snapshot(&entry);
    let checkout_before = snapshot(&checkout);
    let substitute = handed.root.join("substitute");
    let result = if at_hook {
        let mut hooks = LoseStoreAtBefore {
            site: slot.remove_site(),
            store: store.clone(),
            entry: entry.clone(),
            loss,
            substitute: substitute.clone(),
        };
        manager.remove_worktree(&mut hooks, &slot)
    } else {
        lose_store(&store, &entry, loss, &substitute);
        manager.remove_worktree(&mut NoHooks, &slot)
    };
    let error = result.expect_err(&format!("{case}: the removal refuses"));
    let message = refusal_of(&error);
    assert!(
        message.contains("when this run's manager was derived") && message.contains(found),
        "{case}: the refusal names the pinned store's loss as `{found}`: {message}"
    );
    assert_eq!(
        snapshot(&checkout),
        checkout_before,
        "{case}: the checkout is untouched"
    );
    assert_eq!(
        snapshot(&entry),
        entry_before,
        "{case}: the pinned entry is untouched"
    );
    let name = entry.file_name().expect("an entry name");
    let copy = match loss {
        StoreLoss::Removed => None,
        StoreLoss::PlainDirectory => Some(store.join(name)),
        StoreLoss::Repointed => Some(substitute.join(name)),
    };
    if let Some(copy) = copy {
        assert_eq!(
            snapshot(&copy),
            entry_before,
            "{case}: the copy of the entry is untouched"
        );
    }
}

/// T-PIN-3's cases: what of the bound entry is exchanged for a link, named for
/// the record. The empty name is the entry itself.
pub(super) const LINK_BELOW_TARGET_CASES: [(&str, &str); 3] = [
    ("the entry", ""),
    ("its gitdir", "gitdir"),
    ("its locked", "locked"),
];

/// The child of
/// [`a_link_planted_below_a_pinned_target_at_the_before_hook_refuses_the_removal`]:
/// the case its parent names, over the store its parent linked. It derives the
/// manager under the inherited value, then makes everything T-PIN-3 made after
/// its derivation, in the same order, and every one of its assertions.
#[test]
#[ignore = "linked child of the pinned-store tests"]
fn link_below_pinned_target_child() {
    let handed = PinChild::handed();
    let (_, name) = handed.case_of(LINK_BELOW_TARGET_CASES);
    let file_link = !name.is_empty();
    let manager = handed.derive("derive the manager over the linked store");
    let slot = Slot::Task {
        key: "alpha".to_owned(),
        generation: 1,
    };
    let checkout = manager.slot_path(&slot);
    let target = pinned_target(&handed.root.join("pinned"));
    let entry = target.join(checkout.file_name().expect("a checkout name"));
    fs::write(entry.join("locked"), b"held\n").expect("a locked marker");
    let substituted = if file_link {
        entry.join(name)
    } else {
        entry.clone()
    };
    let victim = handed.root.join("victim");
    if file_link {
        fs::write(&victim, b"victim bytes\n").expect("the victim file");
    } else {
        copy_entry(&entry, &victim);
    }
    let victim_before = snapshot(&victim);
    let checkout_before = snapshot(&checkout);
    let mut hooks = SubstituteAtBefore {
        site: slot.remove_site(),
        target: substituted.clone(),
        moved: substituted.with_extension("moved-away"),
        victim: victim.clone(),
        file_link,
    };
    let error = manager
        .remove_worktree(&mut hooks, &slot)
        .expect_err("a link below the pinned target refuses");
    let message = refusal_of(&error);
    assert!(
        message.contains("symlink or reparse point")
            && message.contains(&substituted.display().to_string()),
        "{}: the refusal names the link: {message}",
        substituted.display()
    );
    assert_eq!(
        snapshot(&victim),
        victim_before,
        "{}: the victim is untouched",
        substituted.display()
    );
    assert_eq!(
        snapshot(&checkout),
        checkout_before,
        "{}: the checkout is untouched",
        substituted.display()
    );
}

/// The child of
/// [`a_pinned_target_exchanged_for_a_link_at_the_before_hook_refuses_the_removal`],
/// over the store its parent linked. It derives the manager under the
/// inherited value, then makes everything T-PIN-4 made after its derivation, in
/// the same order, and every one of its assertions.
#[test]
#[ignore = "linked child of the pinned-store tests"]
fn pinned_target_exchanged_child() {
    let handed = PinChild::handed();
    handed.case_of([("pin-target-exchanged", ())]);
    let manager = handed.derive("derive the manager over the linked store");
    let slot = Slot::Task {
        key: "alpha".to_owned(),
        generation: 1,
    };
    let checkout = manager.slot_path(&slot);
    let target = pinned_target(&handed.root.join("pinned"));
    let name = checkout.file_name().expect("a checkout name").to_owned();
    let victim = handed.root.join("victim");
    copy_entry(&target.join(&name), &victim.join(&name));
    let victim_before = snapshot(&victim.join(&name));
    let original_before = snapshot(&target.join(&name));
    let checkout_before = snapshot(&checkout);
    let moved = target.with_extension("moved-away");
    let mut hooks = SubstituteAtBefore {
        site: slot.remove_site(),
        target: target.clone(),
        moved: moved.clone(),
        victim: victim.clone(),
        file_link: false,
    };
    let error = manager
        .remove_worktree(&mut hooks, &slot)
        .expect_err("an exchanged target refuses");
    let message = refusal_of(&error);
    assert!(
        message.contains("when this run's manager was derived")
            && message.contains("now resolves to"),
        "the refusal names the store's new resolution: {message}"
    );
    assert_eq!(
        snapshot(&victim.join(&name)),
        victim_before,
        "the victim is untouched"
    );
    assert_eq!(
        snapshot(&moved.join(&name)),
        original_before,
        "the original entry, moved aside with its target, is untouched"
    );
    assert_eq!(
        snapshot(&checkout),
        checkout_before,
        "the checkout is untouched"
    );
}

/// The child of [`a_registration_rebound_under_a_pinned_store_keeps_its_admin_state`],
/// over the store its parent linked. It derives the manager under the
/// inherited value, then makes everything T-PIN-6 made after its derivation, in
/// the same order, and every one of its assertions.
#[test]
#[ignore = "linked child of the pinned-store tests"]
fn registration_rebound_under_pin_child() {
    struct RewriteGitdirAtBefore {
        site: EffectSiteId,
        gitdir: PathBuf,
        replacement: PathBuf,
    }

    impl EffectHooks for RewriteGitdirAtBefore {
        fn phase(&mut self, site: EffectSiteId, phase: HookPhase) -> Injection {
            if site == self.site && phase == HookPhase::Before {
                fs::write(&self.gitdir, format!("{}\n", self.replacement.display()))
                    .expect("rewrite the registration's identity");
            }
            Injection::Proceed
        }

        fn refusal_cause(&self) -> Option<String> {
            None
        }
    }

    let handed = PinChild::handed();
    handed.case_of([("pin-rebound", ())]);
    let manager = handed.derive("derive the manager over the linked store");
    let slot = Slot::Task {
        key: "alpha".to_owned(),
        generation: 1,
    };
    let checkout = manager.slot_path(&slot);
    let target = pinned_target(&handed.root.join("pinned"));
    let entry = target.join(checkout.file_name().expect("a checkout name"));
    let locked = entry.join("locked");
    fs::write(&locked, b"do not remove\n").expect("a locked marker");
    let mut hooks = RewriteGitdirAtBefore {
        site: slot.remove_site(),
        gitdir: entry.join("gitdir"),
        replacement: handed.root.join("other").join(".git"),
    };
    let error = manager
        .remove_worktree(&mut hooks, &slot)
        .expect_err("a changed identity refuses");
    let message = refusal_of(&error);
    assert!(
        message.contains("changed identity before removal"),
        "the identity re-read refuses: {message}"
    );
    assert!(
        !checkout.exists(),
        "the checkout went first, as the removal's order has it"
    );
    assert!(
        locked.is_file() && entry.is_dir(),
        "`locked` and the entry are kept"
    );
}

/// The child of [`a_store_link_whose_target_appears_after_derive_is_not_pinned`],
/// over the store link its parent planted and whose target it took away. It
/// derives the manager under the inherited value, then makes everything T-PIN-7
/// made after its derivation, in the same order, and every one of its
/// assertions.
#[test]
#[ignore = "linked child of the pinned-store tests"]
fn store_link_target_appears_child() {
    let handed = PinChild::handed();
    handed.case_of([("store-link-unpinned", ())]);
    let manager = handed.derive("derive over a store link whose target is gone");
    let slot = Slot::Task {
        key: "alpha".to_owned(),
        generation: 1,
    };
    let checkout = manager.slot_path(&slot);
    let store = manager.common_git_dir().join("worktrees");
    fs::rename(handed.root.join("later-away"), handed.root.join("later")).expect("the target back");
    let checkout_before = snapshot(&checkout);
    let error = manager
        .remove_worktree(&mut NoHooks, &slot)
        .expect_err("an unpinned link refuses");
    let message = refusal_of(&error);
    assert!(
        message.contains("symlink or reparse point")
            && message.contains(&store.display().to_string()),
        "the refusal names the link: {message}"
    );
    assert_eq!(
        snapshot(&checkout),
        checkout_before,
        "the checkout is untouched"
    );
}

/// The variables an isolated work-tree-context child reads: the sequence it
/// runs, the scratch root it runs it in, and the canonical common directory the
/// sequence's `derive` must read.
#[cfg(unix)]
pub(super) const CONTEXT_SEQUENCE: &str = "UPSTROKE_WORK_TREE_CONTEXT_SEQUENCE";

#[cfg(unix)]
pub(super) const CONTEXT_ROOT: &str = "UPSTROKE_WORK_TREE_CONTEXT_ROOT";

#[cfg(unix)]
pub(super) const CONTEXT_COMMON: &str = "UPSTROKE_WORK_TREE_CONTEXT_COMMON";

/// `from` copied to `to`, file by file, links as links.
#[cfg(unix)]
pub(super) fn context_copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("a copy's directory");
    for entry in fs::read_dir(from).expect("read a directory to copy") {
        let entry = entry.expect("a directory entry");
        let target = to.join(entry.file_name());
        let kind = entry.file_type().expect("an entry's type");
        if kind.is_dir() {
            context_copy_tree(&entry.path(), &target);
        } else if kind.is_symlink() {
            let link = fs::read_link(entry.path()).expect("a link's target");
            std::os::unix::fs::symlink(link, &target).expect("copy a link");
        } else if let Err(error) = fs::copy(entry.path(), &target) {
            panic!(
                "copy a file: {} to {}, listed as {kind:?}, now {:?}: {error}",
                entry.path().display(),
                target.display(),
                fs::symlink_metadata(entry.path()).map(|metadata| metadata.file_type())
            );
        }
    }
}

/// Every path under `at` with its bytes, sorted: two equal results mean
/// nothing under `at` was written between them. An absent `at` is empty.
#[cfg(unix)]
pub(super) fn context_tree_bytes(at: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn walk(at: &Path, into: &mut Vec<(PathBuf, Vec<u8>)>) {
        let Ok(metadata) = fs::symlink_metadata(at) else {
            return;
        };
        if metadata.is_dir() {
            into.push((at.to_path_buf(), Vec::new()));
            let mut entries: Vec<PathBuf> = fs::read_dir(at)
                .expect("read a watched directory")
                .map(|entry| entry.expect("a directory entry").path())
                .collect();
            entries.sort();
            for entry in entries {
                walk(&entry, into);
            }
        } else {
            into.push((at.to_path_buf(), fs::read(at).expect("read a watched file")));
        }
    }
    let mut bytes = Vec::new();
    walk(at, &mut bytes);
    bytes
}

/// A topology worker in `slot`: `git <args>` as an `Implement` role through
/// `runner`, which composes the role's environment the way a production
/// runner would.
#[cfg(unix)]
fn context_worker(
    runner: &crate::runner::host::HostRunner,
    slot: &Path,
    args: &[&str],
) -> crate::agent::ProcessOutput {
    use crate::runner::Runner as _;

    let command = args
        .iter()
        .fold(crate::runner::CommandSpec::new("git"), |spec, arg| {
            spec.arg(*arg)
        });
    let request = crate::runner::worker_request(
        command,
        slot.to_path_buf(),
        crate::runner::AgentId::new(crate::agent::claude::ADAPTER_ID),
        std::time::Duration::from_secs(60),
        crate::runner::invocation::InvocationId::attempt(
            crate::topology::registry::TaskKey(0),
            crate::topology::events::GenerationId(0),
            crate::topology::events::AttemptNumber(1),
            crate::runner::invocation::AttemptRole::Worker,
            0,
        ),
    );
    runner
        .run_blocking(&request)
        .unwrap_or_else(|error| panic!("the worker in {}: {args:?}: {error}", slot.display()))
}

/// The index of the linked work tree at `slot` that `git worktree add` gave
/// it, read through its own `.git` file and nothing inherited.
#[cfg(unix)]
fn context_own_index_lists(slot: &Path) -> String {
    use std::os::unix::ffi::OsStrExt;

    let gitfile = fs::read(slot.join(".git")).expect("the slot's .git file");
    let admin = gitfile
        .strip_prefix(b"gitdir: ")
        .and_then(|rest| rest.strip_suffix(b"\n"))
        .expect("a gitfile");
    let admin = slot.join(std::ffi::OsStr::from_bytes(admin));
    let mut command = Command::new("git");
    for name in WORK_TREE_SELECTORS.iter().chain([&COMMON_DIR]) {
        command.env_remove(name);
    }
    let output = command
        .arg("--git-dir")
        .arg(&admin)
        .args(["ls-files", "--cached"])
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "the slot's own index: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// The child of
/// [`the_managers_slot_add_and_reads_shed_the_work_tree_context_and_its_base_keeps_it`]:
/// one sequence, under the names its environment inherited, through the
/// manager's own API and, for the worker, the host runner.
#[cfg(unix)]
#[test]
#[ignore = "subprocess helper"]
fn work_tree_context_sequence_helper() {
    let Some(sequence) = std::env::var_os(CONTEXT_SEQUENCE) else {
        return;
    };
    let sequence = sequence.to_string_lossy().into_owned();
    let root = PathBuf::from(std::env::var_os(CONTEXT_ROOT).expect("the scratch root"));
    let private = root.join("private");
    let base = match sequence.as_str() {
        "D1" => root.join("A/work"),
        "newline" => root.join("repo\nname"),
        "C4" => root.join("base"),
        "C5" => root.join("checkouts/user/x/y/z/base"),
        _ => root.join("managed"),
    };
    let head = git(&base, &["rev-parse", "HEAD"]);
    let manager = WorkspaceManager::derive(&base, &private, super::fixture::RUN_ID, "inc-context")
        .unwrap_or_else(|error| panic!("{sequence}: derive: {error}"));
    if matches!(sequence.as_str(), "D1" | "C4" | "C5") {
        assert_eq!(
            manager.common_git_dir(),
            Path::new(&std::env::var_os(CONTEXT_COMMON).expect("the common directory")),
            "{sequence}: derive reads the common directory the inherited names select"
        );
    }
    manager
        .create_execution_root(&mut NoHooks)
        .expect("the execution root");
    let slot = Slot::Task {
        key: "context".to_owned(),
        generation: 1,
    };
    manager
        .write_intent(&mut NoHooks, &slot)
        .expect("the slot's intent");
    let path = manager
        .add_worktree(&mut NoHooks, &slot, &head)
        .unwrap_or_else(|error| panic!("{sequence}: the slot add: {error}"));
    match sequence.as_str() {
        "FUB-D9-ENV" => {
            assert!(
                context_own_index_lists(&path).contains("tracked.txt"),
                "{sequence}: the slot has an index of its own"
            );
        }
        "CR-1" => {
            let earlier =
                WorkspaceManager::derive(&base, &private, super::fixture::RUN_ID, "inc-earlier")
                    .expect("the earlier incarnation");
            let early = Slot::Task {
                key: "earlier".to_owned(),
                generation: 1,
            };
            earlier
                .write_intent(&mut NoHooks, &early)
                .expect("the earlier slot's intent");
            earlier
                .add_worktree(&mut NoHooks, &early, &head)
                .expect("the earlier slot");
            write_file(&path.join("paid.txt"), b"paid output\n");
            manager
                .candidate_stage(&mut NoHooks, &slot, &[])
                .expect("the successor stages its paid output");
            let side = git(&base, &["rev-parse", "side"]);
            earlier
                .repair_materialize(&mut NoHooks, &early, &side)
                .expect("the earlier incarnation's materialization");
            let captured = manager
                .candidate_write_tree(&mut NoHooks, &slot)
                .expect("the successor's capture");
            assert_eq!(
                git(&base, &["ls-tree", "--name-only", &captured]),
                "README.md\npaid.txt\ntracked.txt",
                "{sequence}: the successor's capture holds its paid output"
            );
        }
        "quiescence" => {
            let tree = git(&base, &["rev-parse", "HEAD^{tree}"]);
            write_file(&base.join("tracked.txt"), b"user staged after the add\n");
            git(&base, &["add", "tracked.txt"]);
            let verified = manager
                .quiescence(&path, &Quiescence::HoldsTree(tree))
                .expect("the quiescence read");
            assert!(
                verified.is_ok(),
                "{sequence}: the slot's own index holds the tree it was made at: {verified:?}"
            );
        }
        "newline" => {
            write_file(&path.join("worker-new.txt"), b"a file the worker made\n");
            let added = context_worker(
                &crate::runner::host::HostRunner::new(),
                &path,
                &["add", "worker-new.txt"],
            );
            assert_eq!(added.code, Some(0), "{sequence}: {added:?}");
            assert!(
                context_own_index_lists(&path).contains("worker-new.txt"),
                "{sequence}: the worker's add lands in the slot's own index"
            );
        }
        _ => {
            let include = private.join("git/recorded-objects.gitconfig");
            write_file(&include, crate::runner::host::RECORDED_OBJECTS_INCLUDE);
            let runner = crate::runner::host::HostRunner::new().with_environment(
                crate::runner::host::HostEnvironment::from_process().reading(
                    crate::runner::host::ObjectGraph::RecordedIn(
                        crate::runner::host::ManagedRepository::new(
                            manager.common_git_dir(),
                            &include,
                            crate::runner::host::GitdirRule::native(),
                        )
                        .expect("the run's repository"),
                    ),
                ),
            );
            write_file(&path.join("worker-new.txt"), b"a file the worker made\n");
            let added = context_worker(&runner, &path, &["add", "worker-new.txt"]);
            assert_eq!(added.code, Some(0), "{sequence}: {added:?}");
            manager
                .candidate_stage(&mut NoHooks, &slot, &[])
                .expect("the capture stages");
            let captured = manager
                .candidate_write_tree(&mut NoHooks, &slot)
                .expect("the capture writes its tree");
            let verified = manager
                .quiescence(&path, &Quiescence::HoldsTree(captured.clone()))
                .expect("the verification");
            assert!(
                verified.is_ok(),
                "{sequence}: the slot's verification reads the run's common directory: {verified:?}"
            );
            let commit = manager
                .candidate_commit_tree(&mut NoHooks, &captured, &head, "candidate")
                .unwrap_or_else(|error| panic!("{sequence}: the commit at the base: {error}"));
            assert!(
                manager
                    .object_exists(&commit)
                    .expect("the base reads its objects"),
                "{sequence}: the candidate commit is in the run's repository"
            );
            assert!(
                git(&base, &["ls-tree", "-r", "--name-only", &captured])
                    .lines()
                    .any(|line| line == "worker-new.txt"),
                "{sequence}: the capture holds the worker's file"
            );
        }
    }
}

/// The child of [`a_relative_common_dir_resolves_to_the_runs_common_dir_in_a_slot`].
#[cfg(unix)]
#[test]
#[ignore = "subprocess helper"]
fn relative_common_dir_slot_helper() {
    let Some(root) = std::env::var_os(CONTEXT_ROOT).map(PathBuf::from) else {
        return;
    };
    let base = root.join("managed");
    let head = git(&base, &["rev-parse", "HEAD"]);
    let manager = WorkspaceManager::derive(
        &base,
        &root.join("private"),
        super::fixture::RUN_ID,
        "inc-relative",
    )
    .expect("derive");
    assert_eq!(
        manager.common_git_dir(),
        Path::new(&std::env::var_os(CONTEXT_COMMON).expect("the common directory")),
        "derive reads the common directory the relative value names at the base"
    );
    manager
        .create_execution_root(&mut NoHooks)
        .expect("the execution root");
    let slot = Slot::Task {
        key: "relative".to_owned(),
        generation: 1,
    };
    let beside = manager
        .slot_path(&slot)
        .parent()
        .expect("the slot's namespace")
        .join("common");
    context_copy_tree(&root.join("common"), &beside);
    manager
        .write_intent(&mut NoHooks, &slot)
        .expect("the slot's intent");
    let path = manager
        .add_worktree(&mut NoHooks, &slot, &head)
        .expect("the slot add");
    let before = context_tree_bytes(&beside);
    write_file(&path.join("staged.txt"), b"staged in the slot\n");
    manager
        .candidate_stage(&mut NoHooks, &slot, &[])
        .expect("the capture stages");
    let captured = manager
        .candidate_write_tree(&mut NoHooks, &slot)
        .expect("the capture writes its tree");
    let verified = manager
        .quiescence(&path, &Quiescence::HoldsTree(captured.clone()))
        .expect("the verification");
    assert!(
        verified.is_ok(),
        "the common directory the verification reads in the slot is the base's: {verified:?}"
    );
    let commit = manager
        .candidate_commit_tree(&mut NoHooks, &captured, &head, "candidate")
        .expect("the commit at the base");
    assert_eq!(
        git(&base, &["rev-parse", &format!("{commit}^{{tree}}")]),
        captured,
        "the commit at the base holds the captured tree"
    );
    assert!(
        context_tree_bytes(&beside) == before,
        "the common directory beside the slot was written"
    );
}

/// A slot run under whatever this process inherited, through the manager's own
/// API: the slot add, a file the worker writes, the capture, and the commit at
/// the base. Its errors are returned rather than raised: the witnesses that run
/// it measure what it wrote before it failed closed, which under a relative
/// object directory is at the commit at the base, and under a linked registry
/// at the capture's guarded read.
#[cfg(unix)]
pub(super) fn slot_run_writing_a_file(
    manager: &WorkspaceManager,
    base: &Path,
    key: &str,
) -> Result<(), UpstrokeError> {
    let head = git(base, &["rev-parse", "HEAD"]);
    let slot = Slot::Task {
        key: key.to_owned(),
        generation: 1,
    };
    manager.create_execution_root(&mut NoHooks)?;
    manager.write_intent(&mut NoHooks, &slot)?;
    let path = manager.add_worktree(&mut NoHooks, &slot, &head)?;
    write_file(&path.join("worker-new.txt"), b"a file the worker made\n");
    manager.candidate_stage(&mut NoHooks, &slot, &[])?;
    let captured = manager.candidate_write_tree(&mut NoHooks, &slot)?;
    manager.candidate_commit_tree(&mut NoHooks, &captured, &head, "candidate")?;
    Ok(())
}

/// What a relative-name child prints, before the error, when `derive` returns
/// one.
#[cfg(unix)]
pub(super) const DERIVE_ERROR: &str = "UPSTROKE_R5_DERIVE_ERROR: ";

/// What a relative-name child prints when `derive` returns a manager.
#[cfg(unix)]
pub(super) const DERIVE_DERIVED: &str = "UPSTROKE_R5_DERIVE_DERIVED";

/// The child of the three relative-name witnesses above: `derive` over the
/// managed checkout the parent made, under the names its environment
/// inherited, and, when the parent asked for it and nothing refused, the slot
/// run.
#[cfg(unix)]
#[test]
#[ignore = "subprocess helper"]
fn relative_name_derive_helper() {
    let Some(root) = std::env::var_os(CONTEXT_ROOT).map(PathBuf::from) else {
        return;
    };
    let base = root.join("managed");
    match WorkspaceManager::derive(
        &base,
        &root.join("private"),
        super::fixture::RUN_ID,
        "inc-relative",
    ) {
        Err(error) => println!("{DERIVE_ERROR}{error}"),
        Ok(manager) => {
            println!("{DERIVE_DERIVED}");
            if std::env::var_os(CONTEXT_SEQUENCE).is_some() {
                let _ = slot_run_writing_a_file(&manager, &base, "relative");
            }
        }
    }
}

/// The scope a `core_ignorecase_helper` child measures, and where it records
/// that it ran.
pub(super) const IGNORECASE_SCOPE: &str = "UPSTROKE_D7_IGNORECASE_SCOPE";

pub(super) const IGNORECASE_MARKER: &str = "UPSTROKE_D7_IGNORECASE_MARKER";

#[test]
#[ignore = "subprocess helper"]
fn core_ignorecase_helper() {
    if std::env::var_os(REPLACEMENT_WITNESS).is_none() {
        return;
    }
    let scope = std::env::var(IGNORECASE_SCOPE).expect("the scope this child measures");
    let marker = PathBuf::from(std::env::var_os(IGNORECASE_MARKER).expect("the marker"));
    write_file(&marker, b"ran\n");
    let read = |configure: &dyn Fn(&Fixture)| {
        let fixture = Fixture::new("d7-ignorecase");
        // The fixture-local unset baseline (root's
        // PR12-D7-UNSET-FIXTURE-PRECONDITION). `git init` writes
        // `core.ignorecase = true` into a repository it creates on a folding
        // filesystem, and neither the neutral environment nor the pinned
        // global file reaches that. So the key is taken out of this fixture's
        // own file, and its absence there is checked, before the case applies
        // its configuration. Only the new fixture's file is written, and the
        // child's global or environment challenge is left as it was.
        let unset = git_out(
            &fixture.base,
            &["config", "--local", "--unset-all", "core.ignorecase"],
        );
        assert!(
            matches!(unset.status.code(), Some(0 | 5)),
            "clearing the fixture's own core.ignorecase: {unset:?}"
        );
        let left = git_out(
            &fixture.base,
            &["config", "--local", "--get-all", "core.ignorecase"],
        );
        assert_eq!(
            left.status.code(),
            Some(1),
            "the baseline: the fixture's own file sets no core.ignorecase: {left:?}"
        );
        configure(&fixture);
        fixture.manager.core_ignorecase()
    };
    let local = |value: &'static str| {
        move |fixture: &Fixture| {
            git(&fixture.base, &["config", "core.ignorecase", value]);
        }
    };
    let included = |value: Option<&'static str>| {
        move |fixture: &Fixture| {
            let file = fixture.root.join("included.gitconfig");
            let text = value.map_or_else(String::new, |value| {
                format!("[core]\n\tignorecase = {value}\n")
            });
            write_file(&file, text.as_bytes());
            write_include_path(&fixture.base.join(".git").join("config"), &file);
        }
    };
    match scope.as_str() {
        "neutral" => {
            assert_eq!(read(&|_| {}).expect("an unset key reads"), None);
            assert_eq!(read(&local("true")).expect("reads"), Some(true));
            assert_eq!(read(&local("false")).expect("reads"), Some(false));
            assert_eq!(
                read(&local("yes")).expect("reads"),
                Some(true),
                "Git's own boolean spellings"
            );
            assert_eq!(
                read(&included(Some("true"))).expect("reads"),
                Some(true),
                "a file the repository's config includes"
            );
            assert_eq!(
                read(&included(None)).expect("reads"),
                None,
                "the control: the same include, without the key"
            );
            let malformed = read(&local("sometimes")).expect_err("a value that is not a boolean");
            assert!(
                malformed.to_string().contains("core.ignorecase"),
                "{malformed}"
            );
            let unparseable = read(&|fixture: &Fixture| {
                let config = fixture.base.join(".git").join("config");
                let mut text = fs::read_to_string(&config).expect("the repository's config");
                text.push_str("[core\n");
                write_file(&config, text.as_bytes());
            })
            .expect_err("a configuration file Git cannot parse");
            assert!(
                unparseable.to_string().contains("ended Some("),
                "{unparseable}"
            );
        }
        "global" => {
            assert_eq!(
                read(&|_| {}).expect("reads"),
                Some(true),
                "the fixture's global file sets it, and nothing outranks it"
            );
            assert_eq!(
                read(&local("false")).expect("reads"),
                Some(false),
                "the repository's own file outranks the global one"
            );
        }
        "environment" => {
            assert_eq!(read(&|_| {}).expect("reads"), Some(true));
            assert_eq!(
                read(&local("false")).expect("reads"),
                Some(true),
                "the environment's pair outranks the repository's own file"
            );
        }
        other => panic!("no scope `{other}`"),
    }
}
