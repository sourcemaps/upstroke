//! Extended notes: `docs/internals/engine/tests/kept_output.md`

use super::*;

fn move_head_and_tear_after_capture(
    workspace: &Workspace,
    candidate: &crate::workspace::CapturedCandidate,
) -> Result<(), UpstrokeError> {
    record_the_capture(workspace, candidate)?;
    let root = workspace.root();
    let moved = git_after_capture(
        root,
        &[
            "commit-tree",
            "HEAD^{tree}",
            "-p",
            "HEAD",
            "-m",
            "moved after capture",
        ],
    )?;
    git_after_capture(root, &["update-ref", &candidate.branch_ref, &moved])?;
    plant_a_torn_registration_in(root)
        .map(|_| ())
        .map_err(after_capture_failed)
}

fn bounded_child(
    what: &str,
    mut command: Command,
    log: &Path,
) -> (std::process::ExitStatus, String) {
    let out = fs::File::create(log).expect("the child's log");
    let err = out.try_clone().expect("the child's log, for its stderr");
    let mut child = command
        .stdout(out)
        .stderr(err)
        .spawn()
        .expect("spawn the child");
    let deadline = std::time::Instant::now() + ENGINE_CALL_BOUND;
    loop {
        if let Some(status) = child.try_wait().expect("poll the child") {
            return (status, fs::read_to_string(log).unwrap_or_default());
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "{what}: the child did not end within {ENGINE_CALL_BOUND:?}: {}",
                fs::read_to_string(log).unwrap_or_default()
            );
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn this_test_binary(fixture: &str) -> Command {
    let mut command = Command::new(std::env::current_exe().expect("this test binary"));
    command.args([
        "--exact",
        &format!("engine::tests::{fixture}"),
        "--ignored",
        "--test-threads",
        "1",
    ]);
    command
}

fn kept_pin_of(repo: &Path, attempt: u32) -> String {
    let run_id = rundir::latest_run(repo).expect("a run started");
    format!(
        "{}{}",
        super::coordinator::prepared_pin_ref(&run_id, 0, attempt),
        crate::workspace::KEPT_PIN_SUFFIX
    )
}

fn pin_target(repo: &Path, pin: &str) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "--verify", "--quiet", pin])
        .output()
        .expect("run git");
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

fn blob_at(repo: &Path, revision: &str, path: &str) -> Option<Vec<u8>> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["--no-replace-objects", "cat-file", "blob"])
        .arg(format!("{revision}:{path}"))
        .output()
        .expect("run git");
    out.status.success().then_some(out.stdout)
}

fn kept_copies(repo: &Path) -> Vec<PathBuf> {
    let run_id = rundir::latest_run(repo).expect("a run started");
    let mut copies: Vec<PathBuf> = fs::read_dir(paths_of(repo, &run_id).public)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| {
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.starts_with("kept-") && name.ends_with(".bundle"))
                })
                .collect()
        })
        .unwrap_or_default();
    copies.sort();
    copies
}

fn restore_the_pin_from(repo: &Path, copy: &Path, pin: &str) {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("fetch")
        .arg(copy)
        .arg(format!("{pin}:{pin}"))
        .output()
        .expect("run git fetch");
    assert!(
        out.status.success(),
        "git fetch {} {pin}:{pin}: {}",
        copy.display(),
        String::from_utf8_lossy(&out.stderr)
    );
}

fn forget_every_unreferenced_object(repo: &Path) {
    git_in(repo, &["reflog", "expire", "--expire=now", "--all"]);
    git_in(repo, &["gc", "-q", "--prune=now"]);
}

fn resume_refusal(result: &Result<RunReport, UpstrokeError>, what: &str) -> String {
    match result {
        Err(UpstrokeError::Resume { message, .. }) => message.clone(),
        Err(other) => panic!("{what}: a resume refusal, never {other:?}"),
        Ok(report) => panic!("{what}: a resume refusal, not {report:?}"),
    }
}

fn file_bytes(repo: &Path, path: &str) -> Option<Vec<u8>> {
    fs::read(repo.join(path)).ok()
}

fn shaping(shape: Shape) -> OneAdapter<ShapingWorker> {
    OneAdapter {
        adapter: ShapingWorker {
            inner: FakeAdapter::new(vec![Effect::NoEdit], vec![ReviewBehavior::Pass]),
            shape,
            then_exit: false,
        },
    }
}

fn paid_output_in_a_new_directory(root: &Path) -> std::io::Result<()> {
    write_into(root, "agent-output.txt", b"paid output\n")?;
    write_into(root, "newdir/inner.txt", b"paid, in a new directory\n")
}

fn paid_edit_to_a_tracked_file(root: &Path) -> std::io::Result<()> {
    write_into(root, "agent-output.txt", b"paid output\n")?;
    write_into(root, "sub/t.txt", b"paid edit\n")
}

fn die_inside_an_attempt(repo: &Path, shape: &str) {
    let mut command = this_test_binary("kept_shape_child_dies_inside_an_attempt");
    command
        .env("UPSTROKE_KEPT_CHILD_REPO", repo)
        .env("UPSTROKE_KEPT_CHILD_SHAPE", shape);
    let (status, log) = bounded_child(
        "the run that dies inside its attempt",
        command,
        &repo.with_file_name("dies-inside.log"),
    );
    assert_eq!(
        status.code(),
        Some(CRASH_EXIT_CODE),
        "the child dies inside the attempt, after its worker wrote: {log}"
    );
}

fn resume_and_die_inside_the_attempt(repo: &Path) {
    let mut command = this_test_binary("kept_resume_child_dies_inside_the_attempt");
    command.env("UPSTROKE_KEPT_RESUME_REPO", repo);
    let (status, log) = bounded_child(
        "the resume that dies inside its attempt",
        command,
        &repo.with_file_name("resume-dies.log"),
    );
    assert_eq!(
        status.code(),
        Some(CRASH_EXIT_CODE),
        "the resume reached its attempt, past the guarded discard: {log}"
    );
}

struct ChildEngine {
    report: PathBuf,
    log: PathBuf,
}

fn child_engine(repo: &Path, label: &str) -> ChildEngine {
    ChildEngine {
        report: repo.with_file_name(format!("{label}.report")),
        log: repo.with_file_name(format!("{label}.log")),
    }
}

impl ChildEngine {
    fn run(
        &self,
        what: &str,
        repo: &Path,
        fixture: &str,
        env: &[(&str, &std::ffi::OsStr)],
    ) -> String {
        let _ = fs::remove_file(&self.report);
        let mut command = this_test_binary(fixture);
        command
            .env("UPSTROKE_KEPT_ENGINE_REPO", repo)
            .env("UPSTROKE_KEPT_ENGINE_REPORT", &self.report);
        for (name, value) in env {
            command.env(name, value);
        }
        let (status, log) = bounded_child(what, command, &self.log);
        assert!(
            status.success(),
            "{what}: the child ended {status:?}: {log}"
        );
        fs::read_to_string(&self.report).unwrap_or_else(|_| panic!("{what}: no report: {log}"))
    }
}

#[cfg(unix)]
struct GitShim {
    dir: PathBuf,
    log: PathBuf,
}

#[cfg(unix)]
struct ShimRule<'a> {
    subcommand: &'a str,
    holding: &'a str,
    after: bool,
    action: String,
}

#[cfg(unix)]
fn real_git() -> String {
    String::from_utf8(
        Command::new("sh")
            .args(["-c", "command -v git"])
            .output()
            .expect("find git")
            .stdout,
    )
    .expect("a UTF-8 path to git")
    .trim()
    .to_owned()
}

#[cfg(unix)]
fn git_shim(beside: &Path, label: &str, rules: &[ShimRule<'_>]) -> GitShim {
    let dir = beside.with_file_name(format!("{label}-shim"));
    fs::create_dir_all(&dir).expect("the shim's directory");
    let log = dir.join("calls.log");
    let real = real_git();
    let mut script = format!(
        "#!/bin/sh\nreal='{real}'\nforeign() {{ env -u GIT_INDEX_FILE -u GIT_NO_REPLACE_OBJECTS \"$real\" \"$@\"; }}\nsub=''\nskip=''\nfor a in \"$@\"; do\n  if [ -n \"$skip\" ]; then skip=''; continue; fi\n  case \"$a\" in\n    -C|-c) skip=1 ;;\n    -*) ;;\n    *) sub=\"$a\"; break ;;\n  esac\ndone\nprintf '%s\\n' \"$*\" >> '{}'\n",
        log.display()
    );
    let mut afters = String::new();
    for (index, rule) in rules.iter().enumerate() {
        let marker = dir.join(format!("rule-{index}.fired"));
        let guard = format!(
            "if [ \"$sub\" = '{}' ] && case \" $* \" in *'{}'*) true;; *) false;; esac && [ ! -e '{}' ]; then\n  : > '{}'\n{}\nfi\n",
            rule.subcommand,
            rule.holding,
            marker.display(),
            marker.display(),
            rule.action
        );
        if rule.after {
            afters.push_str(&guard);
        } else {
            script.push_str(&guard);
        }
    }
    if afters.is_empty() {
        script.push_str("exec \"$real\" \"$@\"\n");
    } else {
        script.push_str("\"$real\" \"$@\"\nstatus=$?\n");
        script.push_str(&afters);
        script.push_str("exit $status\n");
    }
    let shim = dir.join("git");
    // Written by a process of its own, never through a descriptor of this
    // test process: a fork another test thread makes while this process held
    // the shim open for writing keeps that writer until it execs, and the
    // child engine's exec of the shim then fails with ETXTBSY. The helper and
    // its witness are `runner::container::write_program_in_its_own_process`
    // and `the_reaper_relay_writer_leaves_no_writer_in_another_threads_fork`.
    crate::runner::container::write_program_in_its_own_process(&shim, &script);
    GitShim { dir, log }
}

#[cfg(unix)]
impl GitShim {
    fn path(&self) -> std::ffi::OsString {
        let mut path = self.dir.clone().into_os_string();
        path.push(":");
        path.push(std::env::var_os("PATH").expect("a PATH"));
        path
    }

    fn fired(&self, rule: usize) -> bool {
        self.dir.join(format!("rule-{rule}.fired")).exists()
    }

    fn calls(&self) -> String {
        fs::read_to_string(&self.log).unwrap_or_default()
    }
}

#[cfg(unix)]
const EVIDENCE_HEAD: u16 = 2 * 1024;

#[cfg(unix)]
const EVIDENCE_TAIL: u16 = 6 * 1024;

#[cfg(unix)]
const EVIDENCE_TEXT: usize = 64 * 1024;

#[cfg(unix)]
#[track_caller]
fn assert_the_shim_fired(
    shim: &GitShim,
    child: &ChildEngine,
    report: &str,
    extra: &[PathBuf],
    what: std::fmt::Arguments<'_>,
) {
    assert!(
        shim.fired(0),
        "{what}{}",
        unfired_evidence(shim, child, report, extra)
    );
}

#[cfg(unix)]
fn unfired_evidence(
    shim: &GitShim,
    child: &ChildEngine,
    report: &str,
    extra: &[PathBuf],
) -> String {
    let tree = shim.dir.parent();
    let marker = shim.dir.join("rule-0.fired");
    let script = shim.dir.join("git");
    let mut lines = vec![
        observed(tree, &marker, false),
        observed(tree, &shim.dir, false),
        observed_report(report),
    ];
    for path in [&shim.log, &child.log, &script].into_iter().chain(extra) {
        lines.push(observed(tree, path, true));
    }
    let mut text = String::from("\n  observed after exists() answered false, not by it:");
    for line in &lines {
        if !push_capped(&mut text, line) {
            break;
        }
    }
    text
}

#[cfg(unix)]
fn observed(tree: Option<&Path>, path: &Path, read: bool) -> String {
    let (Ok(text) | Err(text)) = observe(tree, path, read);
    said(format_args!("\n  {path:?}: {text}"))
}

#[cfg(unix)]
fn observe(tree: Option<&Path>, path: &Path, read: bool) -> Result<String, String> {
    let (dir, name) = evidence_parent(tree, path)?;
    evidence_at(&dir, &name, path, read)
}

#[cfg(unix)]
fn evidence_parent(
    tree: Option<&Path>,
    path: &Path,
) -> Result<(fs::File, std::ffi::OsString), String> {
    use std::os::unix::fs::OpenOptionsExt;
    let Some(tree) = tree else {
        return Err("not read: no fixture tree bounds it".to_owned());
    };
    let Ok(inside) = path.strip_prefix(tree) else {
        return Err("not read: outside the fixture's tree".to_owned());
    };
    let mut names = Vec::new();
    for part in inside.components() {
        let std::path::Component::Normal(name) = part else {
            return Err("not read: not a plain path in the tree".to_owned());
        };
        names.push(name);
    }
    let Some(leaf) = names.pop() else {
        return Err("not read: the fixture tree itself".to_owned());
    };
    let mut dir = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(tree)
        .map_err(refused("the tree's open"))?;
    let mut at = tree.to_path_buf();
    for name in names {
        at.push(name);
        dir = match open_at(&dir, name, true) {
            Ok(next) => next,
            Err(error) => {
                let kind = error.kind();
                return Err(said(format_args!("not read: {at:?}: {kind:?}: {error}")));
            }
        };
    }
    Ok((dir, leaf.to_os_string()))
}

#[cfg(unix)]
fn open_at(dir: &fs::File, name: &std::ffi::OsStr, directory: bool) -> std::io::Result<fs::File> {
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::os::unix::ffi::OsStrExt;
    let name = std::ffi::CString::new(name.as_bytes())?;
    let wanted = if directory { libc::O_DIRECTORY } else { 0 };
    let flags = wanted | libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC;
    // SAFETY: `openat` reads `name` and `dir`'s descriptor only during the call. `name` is a
    // `CString` owned here until this function returns, so the pointer is to a NUL-terminated
    // string with no interior NUL that outlives the call; `dir` is borrowed for the whole call, so
    // its descriptor stays open and is the one the caller opened. `flags` is built just above from
    // `O_RDONLY`, `O_NOFOLLOW`, `O_NONBLOCK`, `O_CLOEXEC` and, for a directory, `O_DIRECTORY`, and
    // a caller can add nothing to it: it never holds `O_CREAT` or `O_TMPFILE` (which adds its own
    // bit to `O_DIRECTORY`), the flags for which `openat` reads a fourth, variadic mode argument,
    // so passing none is sound.
    let fd = unsafe { libc::openat(dir.as_raw_fd(), name.as_ptr(), flags) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: `openat` returns -1 on failure and a new open descriptor on success, so a
    // non-negative `fd` is a descriptor this call has just created. Nothing else in the process
    // owns or closes it, and it needs no cleanup but `close`, so the `OwnedFd` becomes its one
    // owner and closes it once, when the `File` holding it is dropped.
    let owned = unsafe { OwnedFd::from_raw_fd(fd) };
    Ok(fs::File::from(owned))
}

#[cfg(unix)]
fn evidence_at(
    dir: &fs::File,
    name: &std::ffi::OsStr,
    path: &Path,
    read: bool,
) -> Result<String, String> {
    use std::os::unix::fs::MetadataExt;
    let mut file = open_at(dir, name, false).map_err(refused("its open"))?;
    let opened = file.metadata().map_err(refused("its fstat"))?;
    let kind = kind_of(&opened);
    let len = opened.len();
    if !opened.is_file() {
        return Ok(said(format_args!("{kind}, not read")));
    }
    if !read {
        return Ok(said(format_args!("{kind} of {len} B, not read")));
    }
    let text = read_windows(&mut file, len)?;
    let after = file.metadata().map_err(refused("its second fstat"))?;
    let note = if changed(&opened, &after) {
        ", changed during the read"
    } else {
        ""
    };
    let later = match fs::symlink_metadata(path) {
        Ok(now) if (now.dev(), now.ino()) == (opened.dev(), opened.ino()) => String::new(),
        Ok(_) => "; a later lookup found another file".to_owned(),
        Err(error) => {
            let what = error.kind();
            said(format_args!("; a later lookup: {what:?}: {error}"))
        }
    };
    Ok(said(format_args!("{kind}{note}, {text}{later}")))
}

#[cfg(unix)]
fn read_windows(file: &mut fs::File, len: u64) -> Result<String, String> {
    let head = u64::from(EVIDENCE_HEAD);
    let tail = u64::from(EVIDENCE_TAIL);
    if len <= head + tail {
        let whole = read_window(file, 0, len)?;
        let mut text = said(format_args!("{len} B at its open: "));
        text.push_str(&shown(0, len, &whole));
        return Ok(text);
    }
    let from = len - tail;
    let first = read_window(file, 0, head)?;
    let last = read_window(file, from, tail)?;
    let mut text = said(format_args!("{len} B at its open, truncated: "));
    text.push_str(&shown(0, head, &first));
    text.push_str("; ");
    text.push_str(&shown(from, len, &last));
    Ok(text)
}

#[cfg(unix)]
fn read_window(file: &mut fs::File, from: u64, want: u64) -> Result<Vec<u8>, String> {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(size) = usize::try_from(want) else {
        return Err("not read: its window is too large".to_owned());
    };
    let start = SeekFrom::Start(from);
    file.seek(start).map_err(refused("its seek"))?;
    let mut bytes = vec![0; size];
    let mut filled = 0;
    while filled < size {
        let Some(spare) = bytes.get_mut(filled..) else {
            break;
        };
        match file.read(spare) {
            Ok(0) => break,
            Ok(count) => filled += count,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) => return Err(refused("its read")(error)),
        }
    }
    bytes.truncate(filled);
    Ok(bytes)
}

#[cfg(unix)]
fn shown(from: u64, to: u64, bytes: &[u8]) -> String {
    let got = bytes.len();
    let mut text = said(format_args!("{from}..{to} requested, {got} B read: "));
    text.push_str(&said(format_args!("\"{}\"", bytes.escape_ascii())));
    text
}

#[cfg(unix)]
fn observed_report(report: &str) -> String {
    let bytes = report.as_bytes();
    let len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    let head = usize::from(EVIDENCE_HEAD);
    let tail = usize::from(EVIDENCE_TAIL);
    let text = if bytes.len() <= head + tail {
        shown(0, len, bytes)
    } else {
        let from = bytes.len() - tail;
        let first = bytes.get(..head).unwrap_or_default();
        let last = bytes.get(from..).unwrap_or_default();
        let mut text = shown(0, u64::from(EVIDENCE_HEAD), first);
        text.push_str("; ");
        text.push_str(&shown(len - u64::from(EVIDENCE_TAIL), len, last));
        text
    };
    said(format_args!("\n  the child's report, {len} B: {text}"))
}

#[cfg(unix)]
fn changed(before: &fs::Metadata, after: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    before.len() != after.len()
        || (before.mtime(), before.mtime_nsec()) != (after.mtime(), after.mtime_nsec())
        || (before.ctime(), before.ctime_nsec()) != (after.ctime(), after.ctime_nsec())
}

#[cfg(unix)]
fn kind_of(meta: &fs::Metadata) -> &'static str {
    use std::os::unix::fs::FileTypeExt;
    let kind = meta.file_type();
    if kind.is_file() {
        "a regular file"
    } else if kind.is_dir() {
        "a directory"
    } else if kind.is_symlink() {
        "a symbolic link"
    } else if kind.is_fifo() {
        "a FIFO"
    } else if kind.is_socket() {
        "a socket"
    } else if kind.is_char_device() {
        "a character device"
    } else if kind.is_block_device() {
        "a block device"
    } else {
        "a file of another kind"
    }
}

#[cfg(unix)]
fn refused(what: &'static str) -> impl Fn(std::io::Error) -> String {
    move |error| {
        let kind = error.kind();
        said(format_args!("not read: {what}: {kind:?}: {error}"))
    }
}

#[cfg(unix)]
fn said(args: std::fmt::Arguments<'_>) -> String {
    use std::fmt::Write as _;
    let mut text = String::new();
    if text.write_fmt(args).is_err() {
        text.push_str("[a formatting error]");
    }
    text
}

#[cfg(unix)]
fn push_capped(text: &mut String, line: &str) -> bool {
    let room = EVIDENCE_TEXT.saturating_sub(text.len());
    if line.len() <= room {
        text.push_str(line);
        return true;
    }
    let cap = EVIDENCE_TEXT;
    let mark = said(format_args!("\n  [the evidence stops at its cap, {cap} B]"));
    let mut end = cap.saturating_sub(mark.len());
    text.push_str(line);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    text.push_str(&mark);
    false
}

#[cfg(unix)]
fn evidence_fixture(tag: &str) -> (ScratchTree, GitShim, ChildEngine) {
    let tree = temp_engine_scratch(tag);
    let dir = tree.path().join("t-shim");
    let log = dir.join("calls.log");
    let shim = GitShim { dir, log };
    let child = ChildEngine {
        report: tree.path().join("t.report"),
        log: tree.path().join("t.log"),
    };
    (tree, shim, child)
}

#[cfg(unix)]
#[track_caller]
fn assert_observed(evidence: &str, path: &Path, observation: &str) {
    let line = format!("  {path:?}: {observation}");
    assert!(
        evidence.split('\n').any(|seen| seen == line),
        "{line}\nis not a line of:{evidence}"
    );
}

#[cfg(unix)]
#[track_caller]
fn assert_observed_starting(evidence: &str, path: &Path, prefix: &str) {
    let start = format!("  {path:?}: {prefix}");
    assert!(
        evidence.split('\n').any(|seen| seen.starts_with(&start)),
        "{start}\nbegins no line of:{evidence}"
    );
}

#[cfg(unix)]
fn refused_with(result: &Result<String, String>, prefix: &str) -> bool {
    matches!(result, Err(refusal) if refusal.starts_with(prefix))
}

#[cfg(unix)]
fn is_regular(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|meta| meta.is_file())
}

#[cfg(unix)]
const FIFO_WAIT: Duration = Duration::from_secs(60);

// The protocol of a FIFO control's worker, stated at its site as §10 and §13 require. The calling
// test owns the worker's `JoinHandle` from the spawn to the join, and the descriptor that releases
// it; the worker owns what `work` moves into it, and its value or its panic reaches the caller
// through `join` alone. No early return lies between the spawn and the join, so the handle is not
// dropped unjoined. The mechanism is timed polls of `is_finished`, one every 5 ms; a join only
// once a poll has observed the worker finished; a release opened before the second poll; and an
// abort if the second poll expires. A worker observed finished within `wait` is joined and its
// value returned. One not observed finished is taken as waiting, which is what the control exists
// to catch, even if it finishes as that poll ends: with its id kept from the spawn, `fifo` is
// opened for reading and writing, which gives an open waiting on the FIFO for a writer the writer
// it waits for, and with that descriptor held the second poll gives the worker `FIFO_WAIT` more;
// one observed finished then is joined and reported as `Err`, with its id, the descriptor and its
// late value. A panic in the worker is resumed in the caller wherever it is joined. If the second
// poll expires, the worker is neither joined nor detached: the process aborts. What the polls do
// not bound: `is_finished` can answer true before the thread's teardown is complete, so a join is
// expected to return quickly but has no deadline of its own, and neither has the release open,
// which is expected not to wait on a FIFO, on Linux as fifo(7) documents and on macOS as XNU's
// source reads, though no native macOS run has shown it yet. The polls bound their own waits, not
// every wall-clock path, and no test here executes the abort.
#[cfg(unix)]
fn fifo_bounded<T: std::fmt::Debug + Send + 'static>(
    fifo: &Path,
    wait: Duration,
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, String> {
    let worker = std::thread::Builder::new()
        .name("fifo-control".to_owned())
        .spawn(work)
        .map_err(|error| format!("no worker: {error}"))?;
    let id = worker.thread().id();
    if finished_within(&worker, wait) {
        return Ok(joined(worker));
    }
    let mut options = fs::OpenOptions::new();
    let release = options.read(true).write(true).open(fifo);
    if !finished_within(&worker, FIFO_WAIT) {
        std::process::abort();
    }
    let late = joined(worker);
    let reported = format!(
        "the worker {id:?} had not returned in {wait:?}; it returned once {fifo:?} was opened \
         ({release:?}), and was joined: {late:?}"
    );
    Err(reported)
}

#[cfg(unix)]
fn finished_within<T>(worker: &std::thread::JoinHandle<T>, wait: Duration) -> bool {
    let start = std::time::Instant::now();
    while !worker.is_finished() {
        if start.elapsed() >= wait {
            return false;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    true
}

#[cfg(unix)]
fn joined<T>(worker: std::thread::JoinHandle<T>) -> T {
    match worker.join() {
        Ok(value) => value,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

#[cfg(unix)]
#[test]
fn the_unfired_evidence_reports_missing_and_unreadable_files_as_such() {
    let (_tree, shim, child) = evidence_fixture("evidence-missing");
    let missing = "NotFound: No such file or directory (os error 2)";
    let gone = format!("not read: {:?}: {missing}", shim.dir);
    let absent = format!("not read: its open: {missing}");
    let evidence = unfired_evidence(&shim, &child, "err\nno marker", &[]);
    assert_observed(&evidence, &shim.dir.join("rule-0.fired"), &gone);
    assert_observed(&evidence, &shim.dir, &absent);
    assert_observed(&evidence, &shim.log, &gone);
    assert_observed(&evidence, &child.log, &absent);
    assert_observed(&evidence, &shim.dir.join("git"), &gone);
    let report = "\n  the child's report, 13 B: 0..13 requested, 13 B read: \"err\\nno marker\"";
    assert!(evidence.contains(report), "{evidence}");

    fs::create_dir(&shim.dir).expect("the shim's directory");
    fs::write(&child.log, "words nobody may read\n").expect("the child's log");
    mode_of(&child.log, 0o000).expect("an unreadable log");
    let binds = fs::File::open(&child.log).is_err();
    assert!(
        binds,
        "prerequisite not met: the mode bit did not bind (root, or CAP_DAC_OVERRIDE)"
    );
    let evidence = unfired_evidence(&shim, &child, "err\nno marker", &[]);
    let denied = "not read: its open: PermissionDenied: Permission denied (os error 13)";
    assert_observed(&evidence, &child.log, denied);
    assert_observed(&evidence, &shim.dir, "a directory, not read");
    assert_observed(&evidence, &shim.dir.join("rule-0.fired"), &absent);
    assert!(!evidence.contains("nobody may read"), "{evidence}");
}

#[cfg(unix)]
#[test]
fn the_unfired_evidence_reads_a_large_file_only_in_its_two_windows() {
    let (_tree, shim, child) = evidence_fixture("evidence-large");
    fs::create_dir(&shim.dir).expect("the shim's directory");
    let mut large = b"FIRST".to_vec();
    large.resize((1 << 20) - 4, b'm');
    large.extend_from_slice(b"LAST");
    fs::write(&shim.log, large).expect("a large calls.log");
    let evidence = unfired_evidence(&shim, &child, "ok", &[]);
    let head = format!("FIRST{}", "m".repeat(2043));
    let tail = format!("{}LAST", "m".repeat(6140));
    let read = format!(
        "a regular file, 1048576 B at its open, truncated: 0..2048 requested, 2048 B read: \
         \"{head}\"; 1042432..1048576 requested, 6144 B read: \"{tail}\""
    );
    assert_observed(&evidence, &shim.log, &read);

    let mut file = fs::File::open(&shim.log).expect("the large calls.log");
    let first = read_window(&mut file, 0, u64::from(EVIDENCE_HEAD)).expect("its first window");
    let last = read_window(&mut file, 1042432, u64::from(EVIDENCE_TAIL)).expect("its last one");
    let (head, tail) = (usize::from(EVIDENCE_HEAD), usize::from(EVIDENCE_TAIL));
    assert_eq!((first.len(), first.capacity()), (head, head));
    assert_eq!((last.len(), last.capacity()), (tail, tail));

    let controls = "\u{1}".repeat(1 << 20);
    fs::write(&shim.log, &controls).expect("calls.log, all control bytes");
    fs::write(shim.dir.join("git"), &controls).expect("the script, all control bytes");
    fs::write(&child.log, controls).expect("the child's log, all control bytes");
    let evidence = unfired_evidence(&shim, &child, "ok", &[]);
    let mark = format!("\n  [the evidence stops at its cap, {EVIDENCE_TEXT} B]");
    let size = evidence.len();
    assert!(evidence.ends_with(&mark), "{size} B");
    assert!(size <= EVIDENCE_TEXT, "{size} B");
    assert!(size >= EVIDENCE_TEXT - 3, "{size} B");

    let mut wide = String::new();
    let line = "\u{e9}".repeat(EVIDENCE_TEXT);
    assert!(!push_capped(&mut wide, &line));
    let kept = "\u{e9}".repeat((EVIDENCE_TEXT - mark.len()) / 2);
    assert_eq!(wide, format!("{kept}{mark}"));
    let mut full = "x".repeat(EVIDENCE_TEXT - 1);
    assert!(!push_capped(&mut full, "yz"));
    let room = "x".repeat(EVIDENCE_TEXT - mark.len());
    assert_eq!(full, format!("{room}{mark}"));
}

#[cfg(unix)]
#[test]
fn the_unfired_evidence_follows_no_link_and_opens_no_special_file() {
    let (tree, shim, child) = evidence_fixture("evidence-links");
    let outside = temp_engine_scratch("evidence-outside");
    let secret = outside.path().join("outside.txt");
    fs::write(&secret, "BYTES OUTSIDE THE FIXTURE\n").expect("a file outside the fixture");
    fs::create_dir(&shim.dir).expect("the shim's directory");
    std::os::unix::fs::symlink(&secret, &shim.log).expect("a link as calls.log");
    let made = Command::new("mkfifo")
        .arg(&child.log)
        .status()
        .expect("mkfifo");
    assert!(made.success(), "prerequisite not met: mkfifo failed");
    fs::create_dir(shim.dir.join("git")).expect("a directory as the script");
    let linked = tree.path().join("linked");
    std::os::unix::fs::symlink(outside.path(), &linked).expect("a link to a directory");
    let plain = tree.path().join("plain");
    fs::write(&plain, "a file\n").expect("a file where a directory would be");
    let calls = shim.log.clone();
    let fifo = child.log.clone();
    let script = shim.dir.join("git");
    let extra = [
        linked.join("outside.txt"),
        plain.join("inner"),
        secret.clone(),
    ];
    let work = move || unfired_evidence(&shim, &child, "ok", &extra);
    let evidence = fifo_bounded(&fifo, FIFO_WAIT, work)
        .expect("the evidence returns without waiting on the FIFO");
    assert_observed_starting(&evidence, &calls, "not read: its open: FilesystemLoop: ");
    assert_observed(&evidence, &fifo, "a FIFO, not read");
    assert_observed(&evidence, &script, "a directory, not read");
    let through = format!("not read: {linked:?}: ");
    assert_observed_starting(&evidence, &linked.join("outside.txt"), &through);
    let on_a_file = format!("not read: {plain:?}: NotADirectory: ");
    assert_observed_starting(&evidence, &plain.join("inner"), &on_a_file);
    let elsewhere = "not read: outside the fixture's tree";
    assert_observed(&evidence, &secret, elsewhere);
    assert!(!evidence.contains("BYTES OUTSIDE"), "{evidence}");
}

#[cfg(unix)]
#[test]
fn the_unfired_evidence_refuses_what_is_swapped_in_after_its_check() {
    let (tree, shim, _child) = evidence_fixture("evidence-swaps");
    let outside = temp_engine_scratch("evidence-swaps-outside");
    let secret = outside.path().join("calls.log");
    fs::write(&secret, "BYTES OUTSIDE THE FIXTURE\n").expect("a file outside the fixture");
    fs::create_dir(&shim.dir).expect("the shim's directory");

    fs::write(&shim.log, "checked inside\n").expect("a regular calls.log");
    let (dir, name) = evidence_parent(Some(tree.path()), &shim.log).expect("its descent");
    assert!(is_regular(&shim.log), "the check sees a regular file");
    fs::remove_file(&shim.log).expect("the checked file, removed");
    std::os::unix::fs::symlink(secret, &shim.log).expect("a link in its place");
    let linked = evidence_at(&dir, &name, &shim.log, true);
    assert!(
        refused_with(&linked, "not read: its open: FilesystemLoop: "),
        "{linked:?}"
    );

    let script = shim.dir.join("git");
    fs::write(&script, "checked inside\n").expect("a regular script");
    let (dir, name) = evidence_parent(Some(tree.path()), &script).expect("its descent");
    assert!(is_regular(&script), "the check sees a regular file");
    fs::remove_file(&script).expect("the checked file, removed");
    let made = Command::new("mkfifo")
        .arg(&script)
        .status()
        .expect("mkfifo");
    assert!(made.success(), "prerequisite not met: mkfifo failed");
    let fifo = script.clone();
    let work = move || evidence_at(&dir, &name, &script, true);
    let opened =
        fifo_bounded(&fifo, FIFO_WAIT, work).expect("the open returns without waiting on the FIFO");
    assert_eq!(opened.as_deref(), Ok("a FIFO, not read"));

    let sub = tree.path().join("sub");
    let inner = sub.join("inner.txt");
    fs::create_dir(&sub).expect("a directory on the way");
    fs::write(&inner, "BYTES INSIDE\n").expect("a file inside it");
    let elsewhere = outside.path().join("sub");
    fs::create_dir(&elsewhere).expect("a directory outside");
    fs::write(elsewhere.join("inner.txt"), "BYTES OUTSIDE\n").expect("a file outside");
    let (dir, name) = evidence_parent(Some(tree.path()), &inner).expect("its descent");
    fs::rename(&sub, tree.path().join("sub-moved")).expect("the directory, moved away");
    std::os::unix::fs::symlink(elsewhere, sub).expect("a link in its place");
    let held = evidence_at(&dir, &name, &inner, true);
    let want = concat!(
        "a regular file, 13 B at its open: 0..13 requested, 13 B read: \"BYTES INSIDE\\n\"",
        "; a later lookup found another file"
    );
    assert_eq!(held.as_deref(), Ok(want));

    let locked = tree.path().join("locked");
    let deeper = locked.join("sub");
    fs::create_dir_all(&deeper).expect("two directories on the way");
    let hidden = deeper.join("inner.txt");
    fs::write(&hidden, "BYTES INSIDE\n").expect("a file below them");
    let (dir, name) = evidence_parent(Some(tree.path()), &hidden).expect("its descent");
    mode_of(&locked, 0o000).expect("a directory on the way, unsearchable");
    let binds = fs::symlink_metadata(&hidden).is_err();
    let looked = evidence_at(&dir, &name, &hidden, true);
    mode_of(&locked, 0o700).expect("searchable again");
    assert!(
        binds,
        "prerequisite not met: the mode bit did not bind (root, or CAP_DAC_OVERRIDE)"
    );
    let want = concat!(
        "a regular file, 13 B at its open: 0..13 requested, 13 B read: \"BYTES INSIDE\\n\"",
        "; a later lookup: PermissionDenied: Permission denied (os error 13)"
    );
    assert_eq!(looked.as_deref(), Ok(want));
}

#[cfg(unix)]
#[test]
fn a_fifo_control_releases_and_joins_a_waiting_worker_and_resumes_a_panicking_one() {
    let tree = temp_engine_scratch("evidence-workers");
    let waiting = tree.path().join("fifo");
    let made = Command::new("mkfifo")
        .arg(&waiting)
        .status()
        .expect("mkfifo");
    assert!(made.success(), "prerequisite not met: mkfifo failed");
    let (sender, receiver) = std::sync::mpsc::channel();
    let opening = waiting.clone();
    let work = move || {
        let opened = fs::File::open(&opening).is_ok();
        let id = std::thread::current().id();
        sender.send(id).map(|()| opened)
    };
    let waited = fifo_bounded(&waiting, Duration::from_millis(100), work);
    let id = receiver
        .try_recv()
        .expect("the worker returned before it was reported");
    let reported = waited.expect_err("a worker that waits on the FIFO is reported");
    let named = format!("the worker {id:?} had not returned in 100ms; ");
    assert!(reported.starts_with(&named), "{reported}");
    let released = format!("it returned once {waiting:?} was opened (Ok(File ");
    assert!(reported.contains(&released), "{reported}");
    let tail = ", and was joined: Ok(true)";
    assert!(reported.ends_with(tail), "{reported}");

    let panics = || -> bool { panic!("the worker's own panic") };
    let panicked = std::panic::catch_unwind(|| fifo_bounded(&waiting, FIFO_WAIT, panics));
    let payload = panicked.expect_err("the worker's panic reaches its caller");
    let message = payload.downcast_ref::<&str>();
    assert_eq!(message, Some(&"the worker's own panic"));
}

#[cfg(unix)]
#[test]
fn the_shim_fired_assertion_fails_as_before_and_its_message_keeps_the_evidence() {
    let (_tree, shim, child) = evidence_fixture("evidence-assert");
    fs::create_dir(&shim.dir).expect("the shim's directory");
    fs::write(&shim.log, "diff --no-textconv HEAD\n").expect("a calls.log");
    fs::write(&child.log, "words of the child\n").expect("a child log");
    let failed = std::panic::catch_unwind(|| {
        assert_the_shim_fired(
            &shim,
            &child,
            "err\nwhile capturing the candidate",
            &[],
            format_args!("ctl: the branch moved during the capture"),
        );
    });
    let payload = failed.expect_err("the assertion fails while the marker is absent");
    let Some(message) = payload.downcast_ref::<String>() else {
        panic!("the assertion's message is not a String");
    };
    assert!(
        message.starts_with("ctl: the branch moved during the capture\n"),
        "{message}"
    );
    for kept in [
        "ctl: the branch moved during the capture\n  observed after exists() answered false",
        "rule-0.fired\": not read: its open: NotFound: No such file or directory (os error 2)",
        "t-shim\": a directory, not read",
        "\n  the child's report, 33 B: 0..33 requested, 33 B read: ",
        "\"err\\nwhile capturing the candidate\"",
        "calls.log\": a regular file, 24 B at its open: 0..24 requested, 24 B read: ",
        "\"diff --no-textconv HEAD\\n\"",
        "t.log\": a regular file, 19 B at its open: 0..19 requested, 19 B read: ",
        "\"words of the child\\n\"",
    ] {
        assert!(message.contains(kept), "{kept}\nis not in:\n{message}");
    }

    fs::write(shim.dir.join("rule-0.fired"), "").expect("the marker");
    assert_the_shim_fired(
        &shim,
        &child,
        "ok",
        &[],
        format_args!("ctl: the marker exists"),
    );
}

fn move_the_branch_then_write(root: &Path) -> std::io::Result<()> {
    let moved = git_from_a_shape(
        root,
        &[
            "commit-tree",
            "HEAD^{tree}",
            "-p",
            "HEAD",
            "-m",
            "moved before capture",
        ],
    )?;
    let branch = git_from_a_shape(root, &["symbolic-ref", "HEAD"])?;
    git_from_a_shape(root, &["update-ref", &branch, &moved])?;
    paid_output(root)
}

fn the_parent_of(root: &Path, commit: &str) -> Result<String, UpstrokeError> {
    git_after_capture(root, &["rev-parse", &format!("{commit}^")])
}

fn move_back_and_tear_after_capture(
    workspace: &Workspace,
    candidate: &crate::workspace::CapturedCandidate,
) -> Result<(), UpstrokeError> {
    record_the_capture(workspace, candidate)?;
    let root = workspace.root();
    let recorded = the_parent_of(root, &candidate.parent_oid)?;
    git_after_capture(root, &["update-ref", &candidate.branch_ref, &recorded])?;
    plant_a_torn_registration_in(root)
        .map(|_| ())
        .map_err(after_capture_failed)
}

fn reset_hard_back_and_tear_after_capture(
    workspace: &Workspace,
    candidate: &crate::workspace::CapturedCandidate,
) -> Result<(), UpstrokeError> {
    record_the_capture(workspace, candidate)?;
    let root = workspace.root();
    let recorded = the_parent_of(root, &candidate.parent_oid)?;
    git_after_capture(root, &["reset", "-q", "--hard", &recorded])?;
    plant_a_torn_registration_in(root)
        .map(|_| ())
        .map_err(after_capture_failed)
}

#[cfg(unix)]
fn make_the_store_read_only_after_capture(
    workspace: &Workspace,
    candidate: &crate::workspace::CapturedCandidate,
) -> Result<(), UpstrokeError> {
    record_the_capture(workspace, candidate)?;
    let git = workspace.root().join(".git");
    mode_of(&git.join("refs"), 0o555).map_err(after_capture_failed)?;
    mode_of(&git, 0o555).map_err(after_capture_failed)
}

#[cfg(unix)]
fn make_the_refs_read_only_and_tear_after_capture(
    workspace: &Workspace,
    candidate: &crate::workspace::CapturedCandidate,
) -> Result<(), UpstrokeError> {
    tear_after_capture(workspace, candidate)?;
    mode_of(&workspace.root().join(".git").join("refs"), 0o555).map_err(after_capture_failed)
}

fn block_above_the_kept_pin_and_tear_after_capture(
    workspace: &Workspace,
    candidate: &crate::workspace::CapturedCandidate,
) -> Result<(), UpstrokeError> {
    record_the_capture(workspace, candidate)?;
    let root = workspace.root();
    let run_id =
        rundir::latest_run(root).ok_or_else(|| after_capture_failed("no run has started"))?;
    let above = format!("refs/upstroke/prepared/{run_id}");
    git_after_capture(root, &["update-ref", &above, &candidate.parent_oid])?;
    tear_after_capture(workspace, candidate)
}

fn kept_name_after_capture(workspace: &Workspace) -> Result<String, UpstrokeError> {
    let run_id = rundir::latest_run(workspace.root())
        .ok_or_else(|| after_capture_failed("no run has started"))?;
    Ok(format!(
        "{}{}",
        super::coordinator::prepared_pin_ref(&run_id, 0, 1),
        crate::workspace::KEPT_PIN_SUFFIX
    ))
}

fn make_the_kept_name_symbolic_and_tear_after_capture(
    workspace: &Workspace,
    candidate: &crate::workspace::CapturedCandidate,
) -> Result<(), UpstrokeError> {
    record_the_capture(workspace, candidate)?;
    let kept = kept_name_after_capture(workspace)?;
    git_after_capture(
        workspace.root(),
        &["symbolic-ref", &kept, &candidate.branch_ref],
    )?;
    tear_after_capture(workspace, candidate)
}

fn plant_a_foreign_ref_at_the_kept_name_and_tear_after_capture(
    workspace: &Workspace,
    candidate: &crate::workspace::CapturedCandidate,
) -> Result<(), UpstrokeError> {
    record_the_capture(workspace, candidate)?;
    let kept = kept_name_after_capture(workspace)?;
    git_after_capture(
        workspace.root(),
        &["update-ref", &kept, &candidate.parent_oid],
    )?;
    tear_after_capture(workspace, candidate)
}

fn advance_the_branch_after_capture(
    workspace: &Workspace,
    candidate: &crate::workspace::CapturedCandidate,
) -> Result<(), UpstrokeError> {
    record_the_capture(workspace, candidate)?;
    let root = workspace.root();
    let moved = git_after_capture(
        root,
        &[
            "commit-tree",
            "HEAD^{tree}",
            "-p",
            "HEAD",
            "-m",
            "moved after capture",
        ],
    )?;
    git_after_capture(root, &["update-ref", &candidate.branch_ref, &moved]).map(|_| ())
}

fn shared_indexes_of(repo: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(repo.join(".git"))
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|entry| entry.file_name().to_str().map(str::to_owned))
                .filter(|name| name.starts_with("sharedindex."))
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

fn shared_indexes_marker(repo: &Path) -> PathBuf {
    repo.with_file_name("shared-indexes-at-the-arm.txt")
}

fn record_shared_indexes_and_fail_after_capture(
    workspace: &Workspace,
    candidate: &crate::workspace::CapturedCandidate,
) -> Result<(), UpstrokeError> {
    record_the_capture(workspace, candidate)?;
    let root = workspace.root();
    fs::write(
        shared_indexes_marker(root),
        shared_indexes_of(root).join("\n"),
    )
    .map_err(after_capture_failed)?;
    Err(after_capture_failed(
        "an attempt error that is not the registry's",
    ))
}

fn the_restore_brings_the_output_back(repo: &Path, report: &RunReport, pin: &str) {
    let warning = kept_pin_warning(report).expect("the resume names the kept pin");
    assert!(warning.contains(pin), "{warning}");
    follow_the_restore(repo, warning, pin);
    let differences = differences_from_the_pin(repo, pin);
    assert!(
        differences.is_empty(),
        "the restore reproduces the pin: {differences:?}"
    );
}

#[test]
fn a_branch_moved_before_capture_and_back_keeps_the_output_through_the_first_resume() {
    for (shape, worker) in [
        ("a1", move_the_branch_then_write as Shape),
        (
            "a1t",
            move_the_branch_with_a_foreign_change_then_write as Shape,
        ),
    ] {
        let (_tree, repo) = rd1_repo(&format!("rd1-{shape}"));
        let run = bounded_run(
            shape,
            &repo,
            Some(move_back_and_tear_after_capture),
            shaping(worker),
        );
        let refusal = registry_refusal_text(&run, shape);
        assert!(refusal.contains("pinned at"), "{shape}: {refusal}");
        let (_, parent, tree) = captured(&repo);
        let kept = kept_pins(&repo);
        let [(pin, commit)] = kept.as_slice() else {
            panic!("{shape}: one kept pin: {kept:?}");
        };
        assert_eq!(
            tree_of(&repo, commit),
            tree,
            "{shape}: the pin holds the captured tree"
        );
        assert_eq!(
            git_in(&repo, &["rev-parse", &format!("{commit}^")]).trim(),
            parent,
            "{shape}: on the captured parent, the moved commit"
        );

        repair_the_torn_registration(&repo);
        let report = bounded_resume(shape, &repo, fake(Effect::EditFile))
            .unwrap_or_else(|error| panic!("{shape}: the first resume: {error:?}"));
        assert!(committed(&report, "t1"), "{shape}: {report:?}");
        assert!(
            warning_with(&report, "discarded").is_some_and(|w| w.contains("agent-output.txt")),
            "{shape}: the first resume discards the leftovers: {:?}",
            report.warnings
        );
        assert!(
            kept_pin_warning(&report)
                .is_some_and(|w| w.contains("neither HEAD nor one of its ancestors")),
            "{shape}: the warning says what a pin on a moved branch's parent restores: {:?}",
            report.warnings
        );
        the_restore_brings_the_output_back(&repo, &report, pin);
    }
}

fn move_the_branch_with_a_foreign_change_then_write(root: &Path) -> std::io::Result<()> {
    let index = root.with_file_name("foreign-index");
    let foreign = |args: &[&str]| -> std::io::Result<String> {
        let out = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .env("GIT_INDEX_FILE", &index)
            .output()?;
        if !out.status.success() {
            return Err(std::io::Error::other(format!(
                "git {args:?}: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
    };
    foreign(&["read-tree", "HEAD"])?;
    let blob_file = root.with_file_name("foreign-blob.txt");
    fs::write(&blob_file, "a foreign change\n")?;
    let blob = foreign(&["hash-object", "-w", &blob_file.to_string_lossy()])?;
    foreign(&[
        "update-index",
        "--cacheinfo",
        &format!("100644,{blob},tracked.txt"),
    ])?;
    let tree = foreign(&["write-tree"])?;
    let moved = git_from_a_shape(
        root,
        &[
            "commit-tree",
            &tree,
            "-p",
            "HEAD",
            "-m",
            "moved, with a foreign change",
        ],
    )?;
    let branch = git_from_a_shape(root, &["symbolic-ref", "HEAD"])?;
    git_from_a_shape(root, &["update-ref", &branch, &moved])?;
    paid_output(root)
}

#[test]
fn a_branch_moved_before_capture_and_reset_hard_back_keeps_the_output_in_the_pin() {
    let (_tree, repo) = rd1_repo("rd1-a1h");
    let run = bounded_run(
        "the run whose branch went and was reset back",
        &repo,
        Some(reset_hard_back_and_tear_after_capture),
        shaping(move_the_branch_then_write),
    );
    let refusal = registry_refusal_text(&run, "the run whose branch was reset back");
    assert!(refusal.contains("pinned at"), "{refusal}");
    assert_eq!(status_of(&repo), "", "the reset emptied the checkout");
    let (_, _, tree) = captured(&repo);
    let kept = kept_pins(&repo);
    let [(pin, commit)] = kept.as_slice() else {
        panic!("one kept pin, the only copy: {kept:?}");
    };
    assert_eq!(
        tree_of(&repo, commit),
        tree,
        "the pin holds the captured tree"
    );

    repair_the_torn_registration(&repo);
    let report = bounded_resume("the resume", &repo, fake(Effect::EditFile)).expect("the resume");
    assert!(committed(&report, "t1"), "{report:?}");
    the_restore_brings_the_output_back(&repo, &report, pin);
}

#[test]
fn a_branch_moved_after_capture_keeps_the_output_pinned_at_its_captured_identity() {
    let (_tree, repo) = rd1_repo("rd1-a2");
    let run = bounded_run(
        "the run whose branch moved after capture",
        &repo,
        Some(move_head_and_tear_after_capture),
        fake(Effect::EditFile),
    );
    let refusal = registry_refusal_text(&run, "the run whose branch moved after capture");
    assert!(refusal.contains("pinned at"), "{refusal}");
    let (branch_ref, parent, tree) = captured(&repo);
    let moved = git_in(&repo, &["rev-parse", &branch_ref]).trim().to_owned();
    assert_ne!(moved, parent, "the branch moved");
    let kept = kept_pins(&repo);
    let [(pin, commit)] = kept.as_slice() else {
        panic!("one kept pin: {kept:?}");
    };
    assert_eq!(
        git_in(&repo, &["rev-parse", &format!("{commit}^")]).trim(),
        parent,
        "the pin's parent is the captured parent, not HEAD's moved commit"
    );
    assert_eq!(
        tree_of(&repo, commit),
        tree,
        "and its tree the captured tree"
    );

    repair_the_torn_registration(&repo);
    let before = status_of(&repo);
    let first = bounded_resume(
        "the resume on the moved branch",
        &repo,
        fake(Effect::EditFile),
    );
    let refusal = resume_refusal(&first, "the resume on the moved branch");
    assert!(
        refusal.contains(&format!("git update-ref {branch_ref} {parent}")),
        "the refusal names the command that moves the branch back: {refusal}"
    );
    assert_eq!(
        status_of(&repo),
        before,
        "the refused resume discards nothing"
    );

    git_in(&repo, &["reset", "-q", "--hard", &parent]);
    let report = bounded_resume("the resume after the reset", &repo, fake(Effect::EditFile))
        .expect("the resume after the operator's reset");
    assert!(committed(&report, "t1"), "{report:?}");
    the_restore_brings_the_output_back(&repo, &report, pin);
}

#[cfg(unix)]
fn mode_bits_bind(directory: &Path) -> bool {
    let probe = directory.join("upstroke-mode-probe");
    match fs::write(&probe, "probe\n") {
        Ok(()) => {
            let _ = fs::remove_file(&probe);
            false
        }
        Err(_) => true,
    }
}

#[cfg(unix)]
#[test]
fn one_storage_fault_that_refuses_the_snapshot_and_the_pin_loses_nothing() {
    let (_tree, repo) = rd1_repo("rd1-b1");
    let run = bounded_run(
        "the run under one storage fault",
        &repo,
        Some(make_the_store_read_only_after_capture),
        fake(Effect::EditFile),
    );
    let git = repo.join(".git");
    let binds = mode_bits_bind(&git) && mode_bits_bind(&git.join("refs"));
    mode_of(&git, 0o755).expect("the fault healed");
    mode_of(&git.join("refs"), 0o755).expect("the fault healed");
    assert!(
        binds,
        "prerequisite not met: the mode bits did not bind (root, or CAP_DAC_OVERRIDE)"
    );
    let refusal = registry_refusal_text(&run, "the run under one storage fault");
    assert!(
        refusal.contains("pinning it at") && refusal.contains("failed"),
        "the pin failed too: {refusal}"
    );
    assert!(
        status_of(&repo).contains("agent-output.txt"),
        "nothing discarded the output"
    );
    assert_eq!(kept_pins(&repo), Vec::<(String, String)>::new());

    let report = bounded_resume(
        "the resume once the fault healed",
        &repo,
        fake(Effect::EditFile),
    )
    .expect("the resume");
    assert!(committed(&report, "t1"), "{report:?}");
    let (_, _, tree) = captured(&repo);
    let pin = kept_pin_of(&repo, 1);
    let commit = pin_target(&repo, &pin).expect("the resume wrote the kept pin");
    assert_eq!(
        tree_of(&repo, &commit),
        tree,
        "the pin holds the captured tree"
    );
    assert!(
        kept_pin_warning(&report).is_some_and(|w| w.contains(&pin)),
        "and names it: {:?}",
        report.warnings
    );
}

#[cfg(unix)]
#[test]
fn a_resume_refuses_while_the_ref_store_cannot_take_the_kept_pin() {
    let (_tree, repo) = rd1_repo("rd1-b2");
    let run = bounded_run(
        "the run whose ref store is read-only",
        &repo,
        Some(make_the_refs_read_only_and_tear_after_capture),
        fake(Effect::EditFile),
    );
    let refs = repo.join(".git").join("refs");
    let binds = mode_bits_bind(&refs);
    if !binds {
        mode_of(&refs, 0o755).expect("writable again");
    }
    assert!(
        binds,
        "prerequisite not met: the mode bit did not bind (root, or CAP_DAC_OVERRIDE)"
    );
    let refusal = registry_refusal_text(&run, "the run whose ref store is read-only");
    assert!(refusal.contains("failed"), "the pin failed: {refusal}");
    repair_the_torn_registration(&repo);
    let before = status_of(&repo);
    let first = bounded_resume(
        "the resume while the refs are read-only",
        &repo,
        fake(Effect::EditFile),
    );
    mode_of(&refs, 0o755).expect("the fault healed");
    let refusal = resume_refusal(&first, "the resume while the refs are read-only");
    assert!(
        refusal.contains("keeping them before the discard stopped"),
        "{refusal}"
    );
    assert_eq!(status_of(&repo), before, "nothing discarded");
    assert_eq!(
        kept_pins(&repo),
        Vec::<(String, String)>::new(),
        "and no pin"
    );

    let report = bounded_resume("the resume once healed", &repo, fake(Effect::EditFile))
        .expect("the resume once healed");
    assert!(committed(&report, "t1"), "{report:?}");
    assert!(
        pin_target(&repo, &kept_pin_of(&repo, 1)).is_some(),
        "the resume pinned the leftovers"
    );
}

#[test]
fn a_resume_writes_the_kept_pin_a_blocked_name_refused_and_refuses_while_it_is_blocked() {
    let shapes: [(&str, super::options::AfterCandidateCapture); 3] = [
        ("below", block_the_kept_pin_and_tear_after_capture),
        ("above", block_above_the_kept_pin_and_tear_after_capture),
        (
            "symbolic",
            make_the_kept_name_symbolic_and_tear_after_capture,
        ),
    ];
    for (shape, after_capture) in shapes {
        let (_tree, repo) = rd1_repo(&format!("rd1-c-{shape}"));
        let run = bounded_run(shape, &repo, Some(after_capture), fake(Effect::EditFile));
        let refusal = registry_refusal_text(&run, shape);
        assert!(
            refusal.contains("pinning it at") && refusal.contains("failed"),
            "{shape}: the pin failed: {refusal}"
        );
        repair_the_torn_registration(&repo);
        let before = status_of(&repo);
        let first = bounded_resume(shape, &repo, fake(Effect::EditFile));
        assert!(
            matches!(&first, Err(UpstrokeError::Resume { .. })),
            "{shape}: the resume refuses while the name is blocked: {first:?}"
        );
        assert_eq!(status_of(&repo), before, "{shape}: nothing discarded");

        let pin = kept_pin_of(&repo, 1);
        let run_id = rundir::latest_run(&repo).expect("the run");
        match shape {
            "below" => {
                git_in(&repo, &["update-ref", "-d", &format!("{pin}/blocker")]);
            }
            "above" => {
                git_in(
                    &repo,
                    &[
                        "update-ref",
                        "-d",
                        &format!("refs/upstroke/prepared/{run_id}"),
                    ],
                );
            }
            _ => {
                git_in(&repo, &["symbolic-ref", "--delete", &pin]);
            }
        }
        let report = bounded_resume(shape, &repo, fake(Effect::EditFile))
            .unwrap_or_else(|error| panic!("{shape}: the resume once unblocked: {error:?}"));
        assert!(committed(&report, "t1"), "{shape}: {report:?}");
        let (_, _, tree) = captured(&repo);
        let commit = pin_target(&repo, &pin)
            .unwrap_or_else(|| panic!("{shape}: the resume wrote the kept pin"));
        assert_eq!(tree_of(&repo, &commit), tree, "{shape}: holding the output");
    }
}

#[test]
fn a_foreign_ref_at_the_kept_name_is_neither_trusted_nor_named_as_the_output() {
    let (_tree, repo) = rd1_repo("rd1-c3");
    let run = bounded_run(
        "the run whose kept name a foreign ref holds",
        &repo,
        Some(plant_a_foreign_ref_at_the_kept_name_and_tear_after_capture),
        fake(Effect::EditFile),
    );
    let refusal = registry_refusal_text(&run, "the run whose kept name a foreign ref holds");
    assert!(refusal.contains("already exists"), "{refusal}");
    repair_the_torn_registration(&repo);
    let pin = kept_pin_of(&repo, 1);
    let foreign = pin_target(&repo, &pin).expect("the foreign ref");
    let before = status_of(&repo);
    let first = bounded_resume(
        "the resume beside the foreign ref",
        &repo,
        fake(Effect::EditFile),
    );
    let refusal = resume_refusal(&first, "the resume beside the foreign ref");
    assert!(
        refusal.contains("is not upstroke's kept commit of this attempt"),
        "{refusal}"
    );
    assert_eq!(status_of(&repo), before, "nothing discarded");

    git_in(&repo, &["update-ref", "-d", &pin]);
    let report = bounded_resume(
        "the resume once the ref is gone",
        &repo,
        fake(Effect::EditFile),
    )
    .expect("the resume");
    assert!(committed(&report, "t1"), "{report:?}");
    let commit = pin_target(&repo, &pin).expect("the resume wrote the kept pin");
    assert_ne!(commit, foreign);
    let (_, _, tree) = captured(&repo);
    assert_eq!(tree_of(&repo, &commit), tree, "the pin holds the output");
}

#[test]
fn a_run_that_dies_after_its_capture_loses_nothing_on_resume() {
    let (_tree, repo) = rd1_repo("rd1-e1");
    let mut command = this_test_binary("kept_capture_child_dies_after_its_capture");
    command.env("UPSTROKE_KEPT_CHILD_REPO", &repo);
    let (status, log) = bounded_child(
        "the run that dies after its capture",
        command,
        &repo.with_file_name("dies-after-capture.log"),
    );
    assert_eq!(status.code(), Some(CRASH_EXIT_CODE), "{log}");
    assert!(
        status_of(&repo).contains("agent-output.txt"),
        "the leftovers"
    );
    assert_eq!(
        kept_pins(&repo),
        Vec::<(String, String)>::new(),
        "no pin yet"
    );
    let leftovers = file_bytes(&repo, "agent-output.txt").expect("the leftovers' bytes");

    let report = bounded_resume("the resume", &repo, fake(Effect::EditFile)).expect("the resume");
    assert!(committed(&report, "t1"), "{report:?}");
    let pin = kept_pin_of(&repo, 1);
    let commit = pin_target(&repo, &pin).expect("attempt 1's kept pin");
    assert_eq!(
        blob_at(&repo, &commit, "agent-output.txt"),
        Some(leftovers),
        "the pin holds the leftovers"
    );
    the_restore_brings_the_output_back(&repo, &report, &pin);
}

#[cfg(unix)]
#[test]
fn a_run_that_dies_between_its_pins_commit_and_its_ref_loses_nothing_on_resume() {
    let (_tree, repo) = rd1_repo("rd1-e2");
    let shim = git_shim(
        &repo,
        "e2",
        &[ShimRule {
            subcommand: "update-ref",
            holding: "-kept",
            after: false,
            action: "  kill -KILL $PPID\n  exit 1".to_owned(),
        }],
    );
    let child = child_engine(&repo, "e2");
    let mut command = this_test_binary("kept_engine_child_runs_and_tears_after_capture");
    command
        .env("UPSTROKE_KEPT_ENGINE_REPO", &repo)
        .env("UPSTROKE_KEPT_ENGINE_REPORT", &child.report)
        .env("PATH", shim.path());
    let (status, log) = bounded_child("the run killed at its pin's ref", command, &child.log);
    assert_the_shim_fired(
        &shim,
        &child,
        "a sentinel, not the child's report: this test reads no report bytes of its own, and \
         the report's actual bytes are in the bounded observation of the report file's path below",
        std::slice::from_ref(&child.report),
        format_args!(
            "the shim killed the run at the pin's update-ref. The child ended {status:?}; its \
             stdout and stderr, {} B when it ended, are the child's log, observed below by its \
             path, not the engine's report; the report file is observed below by its path, and \
             the child's report line holds a sentinel, not report bytes",
            log.len()
        ),
    );
    assert!(
        status.code().is_none(),
        "the run died by a signal: {status:?}"
    );
    assert!(
        status_of(&repo).contains("agent-output.txt"),
        "the leftovers"
    );
    assert_eq!(
        kept_pins(&repo),
        Vec::<(String, String)>::new(),
        "a commit, no ref"
    );

    repair_the_torn_registration(&repo);
    let report = bounded_resume("the resume", &repo, fake(Effect::EditFile)).expect("the resume");
    assert!(committed(&report, "t1"), "{report:?}");
    let pin = kept_pin_of(&repo, 1);
    let (_, _, tree) = captured(&repo);
    let commit = pin_target(&repo, &pin).expect("the resume wrote the kept pin");
    assert_eq!(tree_of(&repo, &commit), tree, "holding the output");
}

#[test]
fn a_kept_pin_removed_before_the_resume_is_written_again_from_the_checkout() {
    for how in ["update-ref", "fetch-prune"] {
        let (_tree, repo) = rd1_repo(&format!("rd1-f-{how}"));
        let run = bounded_run(how, &repo, Some(tear_after_capture), fake(Effect::EditFile));
        registry_refusal_text(&run, how);
        let pin = kept_pin_of(&repo, 1);
        let first = pin_target(&repo, &pin).expect("the refusal's pin");
        if how == "update-ref" {
            git_in(&repo, &["update-ref", "-d", &pin]);
        } else {
            let remote = repo.with_file_name("remote.git");
            git_in(
                repo.parent().expect("the tree"),
                &["init", "-q", "--bare", &remote.to_string_lossy()],
            );
            let (branch_ref, _, _) = captured(&repo);
            git_in(
                &repo,
                &[
                    "push",
                    "-q",
                    &remote.to_string_lossy(),
                    &format!("{branch_ref}:{branch_ref}"),
                ],
            );
            git_in(
                &repo,
                &[
                    "-c",
                    "maintenance.auto=false",
                    "-c",
                    "gc.auto=0",
                    "fetch",
                    "-q",
                    "--prune",
                    &remote.to_string_lossy(),
                    "+refs/upstroke/*:refs/upstroke/*",
                ],
            );
        }
        assert_eq!(pin_target(&repo, &pin), None, "{how}: the pin is gone");
        repair_the_torn_registration(&repo);
        let report = bounded_resume(how, &repo, fake(Effect::EditFile))
            .unwrap_or_else(|error| panic!("{how}: the resume: {error:?}"));
        assert!(committed(&report, "t1"), "{how}: {report:?}");
        let again = pin_target(&repo, &pin).expect("the resume wrote the pin again");
        assert_eq!(
            tree_of(&repo, &again),
            tree_of(&repo, &first),
            "{how}: from the checkout, which held the output"
        );
        assert!(
            kept_pin_warning(&report).is_some_and(|w| w.contains(&pin)),
            "{how}: and names it: {:?}",
            report.warnings
        );
    }
}

#[test]
fn a_resume_refuses_while_the_checkout_holds_more_than_its_kept_pin() {
    let (_tree, repo) = rd1_repo("rd1-x3");
    let run = bounded_run(
        "the pinned refusal",
        &repo,
        Some(tear_after_capture),
        fake(Effect::EditFile),
    );
    registry_refusal_text(&run, "the pinned refusal");
    let mut text = fs::read_to_string(repo.join("agent-output.txt")).expect("the output");
    text.push_str("the operator's own line\n");
    fs::write(repo.join("agent-output.txt"), &text).expect("an edit after the pin");
    repair_the_torn_registration(&repo);
    let first = bounded_resume(
        "the resume over the operator's edit",
        &repo,
        fake(Effect::EditFile),
    );
    let refusal = resume_refusal(&first, "the resume over the operator's edit");
    assert!(
        refusal.contains("holds otherwise") && refusal.contains("agent-output.txt"),
        "{refusal}"
    );
    assert_eq!(
        fs::read_to_string(repo.join("agent-output.txt")).expect("the output"),
        text,
        "the operator's line stays"
    );
    git_in(&repo, &["reset", "-q", "--hard"]);
    git_in(&repo, &["clean", "-qfd"]);
    let report = bounded_resume(
        "the resume after the operator's clean",
        &repo,
        fake(Effect::EditFile),
    )
    .expect("the resume");
    assert!(committed(&report, "t1"), "{report:?}");
    let (_, _, tree) = captured(&repo);
    let commit = pin_target(&repo, &kept_pin_of(&repo, 1)).expect("the pin");
    assert_eq!(
        tree_of(&repo, &commit),
        tree,
        "the pin holds the captured tree"
    );
}

#[test]
fn leftovers_with_no_attempt_in_flight_are_discarded_as_before() {
    let (_tree, repo, run_id) = parked_run("rd1-x2");
    fs::write(
        repo.join("operator-file.txt"),
        "left with no attempt in flight\n",
    )
    .expect("a leftover");
    let repo_in = repo.clone();
    let run_in = run_id.clone();
    let resumed = bounded("the answered resume", move || {
        resume_answering(&repo_in, &run_in, Effect::EditFile)
    });
    assert!(
        warning_with(&resumed, "discarded").is_some_and(|w| w.contains("operator-file.txt")),
        "the discard warning names the file: {:?}",
        resumed.warnings
    );
    assert_eq!(
        kept_pins(&repo),
        Vec::<(String, String)>::new(),
        "no kept pin is written"
    );
    assert!(kept_copies(&repo).is_empty(), "and no copy");
}

fn a_pinned_refusal(repo: &Path, adapters: impl AdapterSource + Send + 'static) -> String {
    let run = bounded_run(
        "the pinned refusal",
        repo,
        Some(tear_after_capture),
        adapters,
    );
    let refusal = registry_refusal_text(&run, "the pinned refusal");
    assert!(refusal.contains("pinned at"), "{refusal}");
    repair_the_torn_registration(repo);
    kept_pin_of(repo, 1)
}

fn a_blocked_refusal(repo: &Path, adapters: impl AdapterSource + Send + 'static) -> String {
    let run = bounded_run(
        "the blocked refusal",
        repo,
        Some(block_the_kept_pin_and_tear_after_capture),
        adapters,
    );
    let refusal = registry_refusal_text(&run, "the blocked refusal");
    assert!(refusal.contains("failed"), "{refusal}");
    let pin = kept_pin_of(repo, 1);
    git_in(repo, &["update-ref", "-d", &format!("{pin}/blocker")]);
    repair_the_torn_registration(repo);
    pin
}

#[cfg(unix)]
#[test]
fn a_foreign_index_reset_during_the_resumes_capture_cannot_change_the_kept_tree() {
    let (_tree, repo) = rd1_repo("rd2-rd1-1");
    let pin = a_blocked_refusal(&repo, fake(Effect::EditFile));
    let (_, _, tree) = captured(&repo);
    let shim = git_shim(
        &repo,
        "rd1-1",
        &[ShimRule {
            subcommand: "write-tree",
            holding: "write-tree",
            after: false,
            action: format!("  foreign -C '{}' reset -q HEAD -- .", repo.display()),
        }],
    );
    let child = child_engine(&repo, "rd1-1");
    let report = child.run(
        "the resume whose capture another client's reset meets",
        &repo,
        "kept_engine_child_resumes",
        &[("PATH", &shim.path())],
    );
    assert!(
        shim.fired(0),
        "the foreign reset ran before the capture's write-tree"
    );
    assert!(report.starts_with("ok"), "{report}");
    let commit = pin_target(&repo, &pin).expect("the resume pinned the leftovers");
    assert_eq!(
        tree_of(&repo, &commit),
        tree,
        "the pin holds the captured tree, whatever the foreign reset did to the index"
    );
    assert_eq!(
        blob_at(&repo, &commit, "agent-output.txt"),
        Some(b"edited: 0\n".to_vec()),
        "and the output is restorable from it"
    );
}

#[test]
fn an_assume_unchanged_entry_holding_output_is_kept_before_the_discard() {
    let (_tree, repo) = shaped_repo("rd2-rd1-2");
    let pin = a_blocked_refusal(&repo, shaped_worker());
    git_in(&repo, &["reset", "-q", "HEAD", "--", "tracked.txt"]);
    git_in(
        &repo,
        &["update-index", "--assume-unchanged", "tracked.txt"],
    );
    let report = bounded_resume(
        "the resume over an assume-unchanged entry",
        &repo,
        fake(Effect::EditFile),
    )
    .expect("the resume");
    assert!(committed(&report, "t1"), "{report:?}");
    let commit = pin_target(&repo, &pin).expect("the resume pinned the leftovers");
    assert_eq!(
        blob_at(&repo, &commit, "tracked.txt"),
        Some(b"paid edit\n".to_vec()),
        "the pin's tracked.txt is the paid content"
    );
}

#[cfg(unix)]
#[test]
fn the_output_survives_a_pin_deleted_after_the_resumes_last_pin_check() {
    for (label, rule) in [
        (
            "after-list-heads",
            ShimRule {
                subcommand: "bundle",
                holding: ".partial",
                after: true,
                action: String::new(),
            },
        ),
        (
            "before-read-tree-m",
            ShimRule {
                subcommand: "read-tree",
                holding: " -m ",
                after: false,
                action: String::new(),
            },
        ),
    ] {
        let (_tree, repo) = rd1_repo(&format!("rd2-rd1-3-{label}"));
        let pin = a_pinned_refusal(&repo, fake(Effect::EditFile));
        let (_, _, tree) = captured(&repo);
        let holding = if label == "after-list-heads" {
            "list-heads"
        } else {
            rule.holding
        };
        let rule = ShimRule {
            holding,
            action: format!("  foreign -C '{}' update-ref -d '{pin}'", repo.display()),
            ..rule
        };
        let shim = git_shim(&repo, label, &[rule]);
        let child = child_engine(&repo, label);
        let report = child.run(
            label,
            &repo,
            "kept_engine_child_resumes",
            &[("PATH", &shim.path())],
        );
        assert!(
            shim.fired(0),
            "{label}: the pin was deleted: {}",
            shim.calls()
        );
        assert!(report.starts_with("ok"), "{label}: {report}");
        assert_eq!(pin_target(&repo, &pin), None, "{label}: the pin is gone");
        let copies = kept_copies(&repo);
        let [copy] = copies.as_slice() else {
            panic!("{label}: one copy: {copies:?}");
        };
        forget_every_unreferenced_object(&repo);
        restore_the_pin_from(&repo, copy, &pin);
        let commit = pin_target(&repo, &pin).expect("the pin, restored");
        assert_eq!(
            tree_of(&repo, &commit),
            tree,
            "{label}: the copy holds the output"
        );
    }
}

#[test]
fn an_ignore_rule_that_changes_with_the_discard_exposes_nothing_to_it() {
    for (variant, shape, kept) in [
        ("rd1-4-v1", "rd1-4-v1", "newdir/paid-output.txt"),
        ("rd1-4-v3", "rd1-4-v3", "paid-new.txt"),
    ] {
        let (_tree, repo) = rd1_repo(&format!("rd2-{variant}"));
        let before = file_bytes(&repo, kept);
        assert_eq!(before, None, "{variant}: not there before the worker");
        die_inside_an_attempt(&repo, shape);
        let written = file_bytes(&repo, kept).expect("the worker's ignored output");
        let report = bounded_resume(variant, &repo, fake(Effect::EditFile))
            .unwrap_or_else(|error| panic!("{variant}: the resume: {error:?}"));
        assert!(committed(&report, "t1"), "{variant}: {report:?}");
        assert_eq!(
            file_bytes(&repo, kept),
            Some(written),
            "{variant}: the resume leaves the file the rule covered"
        );
    }
}

#[test]
fn an_interrupted_text_write_does_not_stop_the_resume() {
    let (_tree, repo) = rd1_repo("rd2-rd1-5");
    die_inside_an_attempt(&repo, "rd1-5");
    let report = bounded_resume(
        "the resume over an interrupted text write",
        &repo,
        fake(Effect::EditFile),
    )
    .expect("the resume completes");
    assert!(committed(&report, "t1"), "{report:?}");
    assert_eq!(status_of(&repo), "", "and the checkout is clean");
    let commit = pin_target(&repo, &kept_pin_of(&repo, 1)).expect("the kept pin");
    assert_eq!(
        blob_at(&repo, &commit, "tracked.txt"),
        Some(b"paid text cut mid-character \xe2\x82".to_vec()),
        "the kept pin holds the bytes exactly"
    );
}

struct PartWay {
    #[cfg(unix)]
    directory: PathBuf,
    #[cfg(windows)]
    _held: fs::File,
}

impl PartWay {
    fn hold(repo: &Path, file: &str) -> Self {
        let path = repo.join(file);
        #[cfg(unix)]
        {
            let directory = path.parent().expect("the file's directory").to_path_buf();
            mode_of(&directory, 0o555).expect("a directory nothing can write");
            let binds = mode_bits_bind(&directory);
            if !binds {
                mode_of(&directory, 0o755).expect("writable again");
            }
            assert!(
                binds,
                "prerequisite not met: the mode bit did not bind (root, or CAP_DAC_OVERRIDE)"
            );
            Self { directory }
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            const FILE_SHARE_READ: u32 = 1;
            let held = fs::OpenOptions::new()
                .read(true)
                .share_mode(FILE_SHARE_READ)
                .open(&path)
                .expect("the file, held open");
            Self { _held: held }
        }
    }

    fn heal(self) {
        #[cfg(unix)]
        mode_of(&self.directory, 0o755).expect("writable again");
    }
}

#[cfg(any(unix, windows))]
#[test]
fn a_discard_stopped_part_way_finishes_on_the_next_resume_without_operator_cleanup() {
    for (form, shape, held) in [
        (
            "unlink",
            paid_output_in_a_new_directory as Shape,
            "newdir/inner.txt",
        ),
        ("write", paid_edit_to_a_tracked_file as Shape, "sub/t.txt"),
    ] {
        let (_tree, repo) = rd1_repo(&format!("rd2-greset-{form}"));
        let pin = a_pinned_refusal(&repo, shaping(shape));
        let (_, _, tree) = captured(&repo);
        let fault = PartWay::hold(&repo, held);
        let first = bounded_resume(form, &repo, fake(Effect::EditFile));
        fault.heal();
        assert!(
            first.is_err(),
            "{form}: the fault stops the first resume: {first:?}"
        );
        let report = bounded_resume(form, &repo, fake(Effect::EditFile))
            .unwrap_or_else(|error| panic!("{form}: the second resume: {error:?}"));
        assert!(committed(&report, "t1"), "{form}: {report:?}");
        assert_eq!(status_of(&repo), "", "{form}: the tree is clean");
        let commit = pin_target(&repo, &pin).expect("the pin");
        assert_eq!(
            tree_of(&repo, &commit),
            tree,
            "{form}: the pin holds the output"
        );
    }
}

fn latin1_output(root: &Path) -> std::io::Result<()> {
    write_into(root, "latin1.txt", b"caf\xe9 au lait\n")
}

#[test]
fn an_undecodable_diff_keeps_the_output_pinned() {
    let (_tree, repo) = rd1_repo("rd2-n1a");
    let run = bounded_run(
        "the run whose diff is not UTF-8",
        &repo,
        None,
        shaping(latin1_output),
    );
    let Err(UpstrokeError::WithWarnings(warned)) = &run else {
        panic!("the capture's own refusal, with the kept note: {run:?}");
    };
    assert!(
        warned
            .warnings
            .iter()
            .any(|warning| warning.contains("is kept in this checkout and pinned at")),
        "{run:?}"
    );
    assert_eq!(
        file_bytes(&repo, "latin1.txt"),
        Some(b"caf\xe9 au lait\n".to_vec()),
        "the checkout keeps the file"
    );
    let kept = kept_pins(&repo);
    let [(_, commit)] = kept.as_slice() else {
        panic!("one kept pin: {kept:?}");
    };
    assert_eq!(
        blob_at(&repo, commit, "latin1.txt"),
        Some(b"caf\xe9 au lait\n".to_vec()),
        "the pin holds it"
    );
}

#[cfg(unix)]
#[test]
fn a_branch_moved_during_the_capture_keeps_the_output_pinned() {
    for put_back in ["update-ref", "reset-hard"] {
        let (_tree, repo) = rd1_repo(&format!("rd2-n1c-{put_back}"));
        let recorded = git_in(&repo, &["rev-parse", "HEAD"]).trim().to_owned();
        let move_it = format!(
            "  r='{}'\n  b=$(foreign -C \"$r\" symbolic-ref HEAD)\n  m=$(foreign -C \"$r\" commit-tree HEAD^{{tree}} -p HEAD -m moved-during-capture)\n  foreign -C \"$r\" update-ref \"$b\" \"$m\"",
            repo.display()
        );
        let shim = git_shim(
            &repo,
            put_back,
            &[ShimRule {
                subcommand: "diff",
                holding: "--no-textconv",
                after: false,
                action: move_it,
            }],
        );
        let child = child_engine(&repo, put_back);
        let report = child.run(
            put_back,
            &repo,
            "kept_engine_child_runs",
            &[("PATH", &shim.path())],
        );
        assert_the_shim_fired(
            &shim,
            &child,
            &report,
            &[repo.join(".git").join("HEAD")],
            format_args!("{put_back}: the branch moved during the capture"),
        );
        assert!(
            report.contains("while capturing the candidate")
                && report.contains("is kept in this checkout and pinned at"),
            "{put_back}: the capture's refusal, with the kept note: {report}"
        );
        let pin = kept_pin_of(&repo, 1);
        let commit = pin_target(&repo, &pin).expect("the kept pin");
        assert_eq!(
            blob_at(&repo, &commit, "agent-output.txt"),
            Some(b"edited: 0\n".to_vec()),
            "{put_back}: the pin holds the output"
        );
        let branch = git_in(&repo, &["symbolic-ref", "HEAD"]).trim().to_owned();
        if put_back == "update-ref" {
            git_in(&repo, &["update-ref", &branch, &recorded]);
            assert!(
                status_of(&repo).contains("agent-output.txt"),
                "the checkout keeps it"
            );
        } else {
            git_in(&repo, &["reset", "-q", "--hard", &recorded]);
            assert_eq!(status_of(&repo), "", "the pin is the only copy");
        }
        let resumed = bounded_resume(put_back, &repo, fake(Effect::EditFile))
            .unwrap_or_else(|error| panic!("{put_back}: the resume: {error:?}"));
        assert!(committed(&resumed, "t1"), "{put_back}: {resumed:?}");
        assert!(
            kept_pin_warning(&resumed).is_some_and(|w| w.contains(&pin)),
            "{put_back}: the resume names the pin: {:?}",
            resumed.warnings
        );
        assert_eq!(
            pin_target(&repo, &pin),
            Some(commit),
            "{put_back}: and keeps it"
        );
    }
}

#[test]
fn a_reviewed_candidate_refused_on_a_moved_head_is_kept_and_pinned() {
    for put_back in ["kept", "reset-hard"] {
        let (_tree, repo) = rd1_repo(&format!("rd2-n2a-{put_back}"));
        let run = bounded_run(
            put_back,
            &repo,
            Some(advance_the_branch_after_capture),
            fake(Effect::EditFile),
        );
        let Err(UpstrokeError::WithWarnings(warned)) = &run else {
            panic!("{put_back}: the publication's refusal, with the kept note: {run:?}");
        };
        assert!(
            warned.error.to_string().contains("HEAD moved"),
            "{put_back}: {run:?}"
        );
        assert!(
            warned.warnings.iter().any(|warning| warning.contains(
                "the reviewed candidate of attempt 1 of `t1` is kept in this checkout and pinned at"
            )),
            "{put_back}: {run:?}"
        );
        let (_, parent, tree) = captured(&repo);
        let pin = kept_pin_of(&repo, 1);
        let commit = pin_target(&repo, &pin).expect("the kept pin");
        assert_eq!(
            tree_of(&repo, &commit),
            tree,
            "{put_back}: the captured tree"
        );
        if put_back == "kept" {
            assert!(
                status_of(&repo).contains("agent-output.txt"),
                "the checkout keeps it"
            );
        } else {
            git_in(&repo, &["reset", "-q", "--hard", &parent]);
            assert_eq!(status_of(&repo), "", "only the pin holds it");
            assert_eq!(
                blob_at(&repo, &commit, "agent-output.txt"),
                Some(b"edited: 0\n".to_vec())
            );
        }
    }
}

fn switch_away_then_write(root: &Path) -> std::io::Result<()> {
    git_from_a_shape(root, &["switch", "-q", "-c", "elsewhere"])?;
    paid_output(root)
}

#[test]
fn a_candidate_captured_on_another_branch_is_kept_and_pinned() {
    let (_tree, repo) = rd1_repo("rd2-n2b");
    let run = bounded_run(
        "the run whose worker switched branches",
        &repo,
        None,
        shaping(switch_away_then_write),
    );
    let Err(UpstrokeError::WithWarnings(warned)) = &run else {
        panic!("the publication's refusal, with the kept note: {run:?}");
    };
    assert!(
        warned.error.to_string().contains("refusing publication"),
        "{run:?}"
    );
    assert!(
        warned
            .warnings
            .iter()
            .any(|warning| warning.contains("is kept in this checkout and pinned at")),
        "{run:?}"
    );
    let commit = pin_target(&repo, &kept_pin_of(&repo, 1)).expect("the kept pin");
    assert_eq!(
        blob_at(&repo, &commit, "agent-output.txt"),
        Some(b"paid output\n".to_vec()),
        "the pin holds the candidate"
    );
    assert_eq!(
        file_bytes(&repo, "agent-output.txt"),
        Some(b"paid output\n".to_vec()),
        "and the checkout keeps it"
    );
}

#[derive(Default)]
struct FailTheLegacyAppendNumbered<const N: u32> {
    entered: u32,
}

impl<const N: u32> crate::events::log::EventHooks for FailTheLegacyAppendNumbered<N> {
    fn point(
        &mut self,
        site: EventSite,
        point: crate::topology::effects::SubEffectPoint,
        mode: crate::topology::effects::InjectionMode,
    ) -> crate::topology::effects::Injection {
        use crate::topology::effects::{Injection, InjectionMode, SubEffectPoint};
        if site != EventSite::LegacyAppend
            || point != SubEffectPoint::Written
            || mode != InjectionMode::ErrorReturn
        {
            return Injection::Proceed;
        }
        self.entered += 1;
        if self.entered == N {
            Injection::Error
        } else {
            Injection::Proceed
        }
    }
}

const PASSING_SETTLEMENT_APPEND: u32 = 4;

fn fail_the_passing_settlement() -> Box<dyn crate::events::log::EventHooks> {
    Box::<FailTheLegacyAppendNumbered<PASSING_SETTLEMENT_APPEND>>::default()
}

#[test]
fn a_reviewed_candidate_whose_settlement_is_not_appended_is_kept() {
    let (_tree, repo) = rd1_repo("rd2-n2c");
    let mut opts = options(&repo);
    opts.config_path = Some(repo.join("upstroke.toml"));
    opts.log_hooks = Some(fail_the_passing_settlement);
    let run = bounded("the run whose settlement fails", move || {
        run_with(&opts, &fake(Effect::EditFile))
    });
    let Err(UpstrokeError::WithWarnings(warned)) = &run else {
        panic!("the append's own error, with the kept note: {run:?}");
    };
    assert!(
        warned.error.to_string().contains("Event.LegacyAppend"),
        "{run:?}"
    );
    let run_id = rundir::latest_run(&repo).expect("the run");
    let log = fs::read_to_string(paths_of(&repo, &run_id).events()).expect("the log");
    let last = log.lines().last().expect("a line");
    assert!(
        last.contains("\"attempt_finished\"") && !last.ends_with('}'),
        "the failed append was the attempt's settlement, cut short: {last}"
    );
    let orphan = super::coordinator::prepared_pin_ref(&run_id, 0, 1);
    assert!(
        pin_target(&repo, &orphan).is_some(),
        "the prepared pin was written"
    );
    let pin = kept_pin_of(&repo, 1);
    let commit = pin_target(&repo, &pin).expect("the kept pin");
    assert!(
        status_of(&repo).contains("agent-output.txt"),
        "the checkout keeps the candidate"
    );
    let resumed = bounded_resume("the resume", &repo, fake(Effect::EditFile)).expect("the resume");
    assert!(
        warning_with(&resumed, "removed orphan prepared commit pin").is_some(),
        "{:?}",
        resumed.warnings
    );
    assert_eq!(pin_target(&repo, &orphan), None, "the orphan is removed");
    assert_eq!(pin_target(&repo, &pin), Some(commit), "the kept pin is not");
    assert!(
        kept_pin_warning(&resumed).is_some_and(|w| w.contains(&pin)),
        "and is named: {:?}",
        resumed.warnings
    );
}

#[cfg(unix)]
#[test]
fn an_unreadable_leftover_and_a_nested_repository_are_left_in_place_and_named() {
    let (_tree, repo) = rd1_repo("rd2-x10");
    die_inside_an_attempt(&repo, "x10");
    let locked = repo.join("locked.txt");
    let binds = fs::read(&locked).is_err();
    if !binds {
        mode_of(&locked, 0o644).expect("readable again");
    }
    assert!(binds, "prerequisite not met: mode 0 did not bind (root)");
    let resumed = bounded_resume("the resume", &repo, fake(Effect::EditFile));
    mode_of(&locked, 0o644).expect("readable again");
    assert!(
        matches!(&resumed, Err(error) if error.to_string().contains("git add -A failed")),
        "the resumed attempt's own capture meets what the discard left: {resumed:?}"
    );
    let run_id = rundir::latest_run(&repo).expect("the run");
    let log = fs::read_to_string(paths_of(&repo, &run_id).events()).expect("the log");
    let resumed_line = log
        .lines()
        .find(|line| line.contains("\"run_resumed\""))
        .expect("the guarded discard completed and the resume was recorded");
    assert!(
        resumed_line.contains("agent-output.txt")
            && !resumed_line.contains("locked.txt")
            && !resumed_line.contains("nested"),
        "the record names what was discarded, not what was left: {resumed_line}"
    );
    assert!(
        repo.join("nested").join(".git").is_dir(),
        "the nested repository stays"
    );
    assert_eq!(
        file_bytes(&repo, "locked.txt"),
        Some(b"paid, unreadable\n".to_vec())
    );
    let commit = pin_target(&repo, &kept_pin_of(&repo, 1)).expect("the kept pin");
    assert_eq!(
        blob_at(&repo, &commit, "agent-output.txt"),
        Some(b"paid output\n".to_vec()),
        "the pin holds the output"
    );
}

#[test]
fn an_operators_new_file_after_a_pinned_refusal_stays_in_the_checkout() {
    let (_tree, repo) = rd1_repo("rd2-x16");
    a_pinned_refusal(&repo, fake(Effect::EditFile));
    fs::write(
        repo.join("operator-notes.txt"),
        "the operator's own notes\n",
    )
    .expect("a new file");
    let report = bounded_resume("the resume", &repo, fake(Effect::EditFile)).expect("the resume");
    assert!(committed(&report, "t1"), "{report:?}");
    assert_eq!(
        file_bytes(&repo, "operator-notes.txt"),
        Some(b"the operator's own notes\n".to_vec()),
        "the file stays"
    );
    assert!(
        warning_with(&report, "remain in this checkout")
            .is_some_and(|w| w.contains("operator-notes.txt")),
        "named: {:?}",
        report.warnings
    );
}

#[test]
fn the_discard_reads_the_checkouts_per_worktree_ignore_rules() {
    let (_tree, repo) = rd1_repo("rd2-x18");
    git_in(&repo, &["config", "extensions.worktreeConfig", "true"]);
    let excludes = repo.with_file_name("worktree-excludes");
    fs::write(&excludes, "cache.bin\n").expect("the worktree's excludes");
    git_in(
        &repo,
        &[
            "config",
            "--worktree",
            "core.excludesFile",
            &excludes.to_string_lossy(),
        ],
    );
    die_inside_an_attempt(&repo, "x18");
    let report = bounded_resume("the resume", &repo, fake(Effect::EditFile)).expect("the resume");
    assert!(committed(&report, "t1"), "{report:?}");
    assert_eq!(
        file_bytes(&repo, "cache.bin"),
        Some(b"ignored by this worktree's own rules\n".to_vec()),
        "cache.bin stays"
    );
    for (pin, commit) in kept_pins(&repo) {
        assert_eq!(
            blob_at(&repo, &commit, "cache.bin"),
            None,
            "and no pin holds it: {pin}"
        );
    }
}

#[cfg(unix)]
#[test]
fn a_branch_moved_during_the_discard_refuses_with_the_output_kept() {
    let (_tree, repo) = rd1_repo("rd2-x19");
    let pin = a_pinned_refusal(&repo, fake(Effect::EditFile));
    let (branch_ref, _, tree) = captured(&repo);
    let shim = git_shim(
        &repo,
        "x19",
        &[ShimRule {
            subcommand: "read-tree",
            holding: " -m ",
            after: false,
            action: format!(
                "  r='{}'\n  m=$(foreign -C \"$r\" commit-tree HEAD^{{tree}} -p HEAD -m moved-during-discard)\n  foreign -C \"$r\" update-ref '{branch_ref}' \"$m\"",
                repo.display()
            ),
        }],
    );
    let child = child_engine(&repo, "x19");
    let report = child.run(
        "the resume",
        &repo,
        "kept_engine_child_resumes",
        &[("PATH", &shim.path())],
    );
    assert!(shim.fired(0), "the branch moved before the revert");
    assert!(
        report.starts_with("err") && report.contains("during the discard"),
        "the resume refuses naming the move: {report}"
    );
    let commit = pin_target(&repo, &pin).expect("the pin");
    assert_eq!(tree_of(&repo, &commit), tree, "the pin holds the output");
    let copies = kept_copies(&repo);
    let [copy] = copies.as_slice() else {
        panic!("one copy: {copies:?}");
    };
    git_in(&repo, &["update-ref", "-d", &pin]);
    forget_every_unreferenced_object(&repo);
    restore_the_pin_from(&repo, copy, &pin);
    assert_eq!(
        tree_of(&repo, &pin_target(&repo, &pin).expect("restored")),
        tree,
        "and so does its copy"
    );
}

#[test]
fn a_nested_repository_with_a_commit_does_not_wedge_the_resume() {
    let (_tree, repo) = rd1_repo("rd2-x20");
    die_inside_an_attempt(&repo, "x20");
    let nested_head = git_in(&repo.join("nested"), &["rev-parse", "HEAD"])
        .trim()
        .to_owned();
    let run_id = rundir::latest_run(&repo).expect("a run to resume");
    let log = paths_of(&repo, &run_id).events();
    let recorded = fs::read(&log).expect("the event log");
    let writable = fs::metadata(&log).expect("the event log").permissions();
    let mut read_only = writable.clone();
    read_only.set_readonly(true);
    fs::set_permissions(&log, read_only).expect("the event log made read-only");
    let first = bounded_resume("the first resume", &repo, fake(Effect::EditFile));
    fs::set_permissions(&log, writable).expect("the event log made writable again");
    match &first {
        Err(UpstrokeError::Resume { message, .. }) => {
            panic!("the first resume does not refuse: {message}")
        }
        Err(_) => {}
        Ok(report) => panic!(
            "prerequisite not met: the read-only event log took a write (root, or \
             CAP_DAC_OVERRIDE): {report:?}"
        ),
    }
    assert_eq!(
        fs::read(&log).expect("the event log"),
        recorded,
        "the first resume recorded nothing, so the second finds the same attempt in flight"
    );
    assert!(
        repo.join("nested").join(".git").is_dir(),
        "the repository stays"
    );
    let report = bounded_resume(
        "the second resume, over the same attempt",
        &repo,
        fake(Effect::EditFile),
    )
    .expect("the second resume does not refuse");
    assert!(
        repo.join("nested").join(".git").is_dir(),
        "the repository stays: {:?}",
        report.warnings
    );
    assert_eq!(
        git_in(&repo.join("nested"), &["rev-parse", "HEAD"]).trim(),
        nested_head,
        "and the nested repository stays as it was"
    );
}

fn sha256_of(path: &Path) -> String {
    use sha2::Digest;
    format!(
        "{:x}",
        sha2::Sha256::digest(fs::read(path).expect("a copy"))
    )
}

fn restored_warning(report: &RunReport) -> Option<&String> {
    warning_with(report, "was put back from its copy")
}

fn a_first_resume_stopped_part_way(repo: &Path) -> PathBuf {
    let fault = PartWay::hold(repo, "newdir/inner.txt");
    let first = bounded_resume("the first resume", repo, fake(Effect::EditFile));
    fault.heal();
    let refusal = resume_refusal(&first, "the first resume");
    assert!(refusal.contains("stopped part-way"), "{refusal}");
    let copies = kept_copies(repo);
    let [copy] = copies.as_slice() else {
        panic!("the first resume wrote one copy: {copies:?}");
    };
    copy.clone()
}

#[cfg(any(unix, windows))]
#[test]
fn a_retry_after_a_deleted_pin_restores_it_from_the_untouched_copy() {
    let (_tree, repo) = rd1_repo("rd3-rd2-1");
    let pin = a_pinned_refusal(&repo, shaping(paid_output_in_a_new_directory));
    let (_, _, tree) = captured(&repo);
    let original = pin_target(&repo, &pin).expect("the refusal's pin");
    let copy = a_first_resume_stopped_part_way(&repo);
    let copied = sha256_of(&copy);
    git_in(&repo, &["update-ref", "-d", &pin]);
    let report = bounded_resume("the second resume", &repo, fake(Effect::EditFile))
        .expect("the second resume completes");
    assert!(committed(&report, "t1"), "{report:?}");
    assert_eq!(sha256_of(&copy), copied, "the first copy is byte-identical");
    assert_eq!(
        pin_target(&repo, &pin),
        Some(original.clone()),
        "the kept ref is the original commit again"
    );
    assert_eq!(tree_of(&repo, &original), tree, "holding the output");
    let restored = restored_warning(&report).expect("the restored warning");
    assert!(
        restored.contains(&copy.display().to_string()),
        "it names the copy the pin came back from: {restored}"
    );
    git_in(&repo, &["update-ref", "-d", &pin]);
    forget_every_unreferenced_object(&repo);
    restore_the_pin_from(&repo, &copy, &pin);
    assert_eq!(
        tree_of(&repo, &pin_target(&repo, &pin).expect("restored")),
        tree,
        "the output still restores after every reflog expired and gc --prune=now"
    );
}

#[cfg(unix)]
#[test]
fn the_output_survives_its_pin_deleted_after_the_restore() {
    let (_tree, repo) = rd1_repo("rd3-rd2-1b");
    let pin = a_pinned_refusal(&repo, shaping(paid_output_in_a_new_directory));
    let (_, _, tree) = captured(&repo);
    a_first_resume_stopped_part_way(&repo);
    git_in(&repo, &["update-ref", "-d", &pin]);
    let shim = git_shim(
        &repo,
        "rd2-1b",
        &[ShimRule {
            subcommand: "read-tree",
            holding: " -m ",
            after: false,
            action: format!("  foreign -C '{}' update-ref -d '{pin}'", repo.display()),
        }],
    );
    let child = child_engine(&repo, "rd2-1b");
    let report = child.run(
        "the second resume",
        &repo,
        "kept_engine_child_resumes",
        &[("PATH", &shim.path())],
    );
    assert!(shim.fired(0), "the pin was deleted again after its restore");
    assert!(report.starts_with("ok"), "{report}");
    assert_eq!(pin_target(&repo, &pin), None);
    forget_every_unreferenced_object(&repo);
    for copy in kept_copies(&repo) {
        restore_the_pin_from(&repo, &copy, &pin);
        assert_eq!(
            tree_of(&repo, &pin_target(&repo, &pin).expect("restored")),
            tree,
            "the copy {} alone holds the output",
            copy.display()
        );
        git_in(&repo, &["update-ref", "-d", &pin]);
    }
}

#[cfg(any(unix, windows))]
#[test]
fn a_kept_pin_moved_from_its_copy_refuses_and_discards_nothing() {
    let (_tree, repo) = rd1_repo("rd3-rd2-1m");
    let pin = a_pinned_refusal(&repo, shaping(paid_output_in_a_new_directory));
    let (branch_ref, _, _) = captured(&repo);
    let copy = a_first_resume_stopped_part_way(&repo);
    let run_id = rundir::latest_run(&repo).expect("the run");
    let other_name = format!(
        "{}{}",
        super::coordinator::prepared_pin_ref(&run_id, 0, 9),
        crate::workspace::KEPT_PIN_SUFFIX
    );
    let head = git_in(&repo, &["rev-parse", "HEAD"]).trim().to_owned();
    let head_tree = tree_of(&repo, &head);
    let other = Workspace::open(&repo)
        .expect("the workspace")
        .prepare_commit_from_candidate(
            &branch_ref,
            &head,
            &head_tree,
            "[upstroke] kept: t1 attempt 1",
            &other_name,
        )
        .expect("another kept-looking commit")
        .commit_sha;
    git_in(&repo, &["update-ref", &pin, &other]);
    git_in(&repo, &["update-ref", "-d", &other_name]);
    let before = status_of(&repo);
    let second = bounded_resume("the second resume", &repo, fake(Effect::EditFile));
    let refusal = resume_refusal(&second, "the second resume");
    assert!(
        refusal.contains(&format!("`{pin}` names {other}, but its copy"))
            && refusal.contains(&copy.display().to_string()),
        "the refusal names the pin and the copy: {refusal}"
    );
    assert_eq!(status_of(&repo), before, "the checkout keeps what it held");
}

fn plant_a_copy(repo: &Path, pin: &str, suffix: &str) -> PathBuf {
    let run_id = rundir::latest_run(repo).expect("the run");
    let head = git_in(repo, &["rev-parse", "HEAD"]).trim().to_owned();
    let planted = paths_of(repo, &run_id)
        .public
        .join(format!("kept-0-1-{}{suffix}", crate::ulid::ulid()));
    git_in(
        repo,
        &[
            "bundle",
            "create",
            &planted.to_string_lossy(),
            pin,
            &format!("^{head}"),
        ],
    );
    planted
}

fn cut_to_half(path: &Path) {
    let bytes = fs::read(path).expect("the file");
    fs::write(path, bytes.get(..bytes.len() / 2).expect("its first half")).expect("cut short");
}

#[test]
fn a_copy_cut_short_before_its_rename_is_never_taken_for_the_copy() {
    let (_tree, repo) = rd1_repo("rd3-rd2-1c");
    let pin = a_pinned_refusal(&repo, fake(Effect::EditFile));
    let (_, _, tree) = captured(&repo);
    let partial = plant_a_copy(&repo, &pin, ".partial");
    cut_to_half(&partial);
    let report = bounded_resume("the resume", &repo, fake(Effect::EditFile)).expect("the resume");
    assert!(committed(&report, "t1"), "{report:?}");
    let copies = kept_copies(&repo);
    let [copy] = copies.as_slice() else {
        panic!("the resume wrote one copy of its own: {copies:?}");
    };
    git_in(&repo, &["update-ref", "-d", &pin]);
    forget_every_unreferenced_object(&repo);
    restore_the_pin_from(&repo, copy, &pin);
    assert_eq!(
        tree_of(&repo, &pin_target(&repo, &pin).expect("restored")),
        tree,
        "the output restores from the new copy"
    );
}

#[cfg(unix)]
#[test]
fn a_copy_cut_short_is_never_restored_from() {
    let (_tree, repo) = rd1_repo("rd4-rd2-1cr");
    let pin = a_pinned_refusal(&repo, fake(Effect::EditFile));
    let (_, _, tree) = captured(&repo);
    let crash = format!(
        "  r='{}'\n  f=''\n  take=''\n  for a in \"$@\"; do\n    if [ -n \"$take\" ]; then f=\"$a\"; break; fi\n    [ \"$a\" = create ] && take=1\n  done\n  size=$(wc -c < \"$r/$f\")\n  head -c $((size / 2)) \"$r/$f\" > \"$r/$f.cut\"\n  mv \"$r/$f.cut\" \"$r/$f\"\n  kill -KILL $PPID\n  exit 1",
        repo.display()
    );
    let shim = git_shim(
        &repo,
        "rd2-1cr",
        &[ShimRule {
            subcommand: "bundle",
            holding: " create ",
            after: true,
            action: crash,
        }],
    );
    let child = child_engine(&repo, "rd2-1cr");
    let mut command = this_test_binary("kept_engine_child_resumes");
    command
        .env("UPSTROKE_KEPT_ENGINE_REPO", &repo)
        .env("UPSTROKE_KEPT_ENGINE_REPORT", &child.report)
        .env("PATH", shim.path());
    let (status, log) = bounded_child(
        "the resume that dies after its bundle create",
        command,
        &child.log,
    );
    assert!(
        shim.fired(0),
        "the resume died right after `bundle create`: {log}"
    );
    assert!(status.code().is_none(), "by a signal: {status:?}");
    assert!(
        kept_copies(&repo).is_empty(),
        "the crash left no final name, only the copy it cut short"
    );
    git_in(&repo, &["update-ref", "-d", &pin]);
    let report = bounded_resume("the next resume", &repo, fake(Effect::EditFile))
        .expect("the next resume pins afresh and completes");
    assert!(committed(&report, "t1"), "{report:?}");
    assert!(
        restored_warning(&report).is_none(),
        "nothing was restored from the cut copy"
    );
    let commit = pin_target(&repo, &pin).expect("pinned afresh");
    assert_eq!(
        tree_of(&repo, &commit),
        tree,
        "from the checkout, which held the output"
    );
    assert_eq!(kept_copies(&repo).len(), 1, "and copied");
}

fn the_newest_copy_holds(repo: &Path, pin: &str, path: &str, bytes: &[u8]) {
    let copy = kept_copies(repo).into_iter().max().expect("a copy");
    let kept = pin_target(repo, pin).expect("the pin");
    git_in(repo, &["update-ref", "-d", pin]);
    forget_every_unreferenced_object(repo);
    restore_the_pin_from(repo, &copy, pin);
    assert_eq!(
        pin_target(repo, pin),
        Some(kept),
        "the copy names the pin's commit"
    );
    assert_eq!(
        blob_at(repo, pin, path),
        Some(bytes.to_vec()),
        "the copy holds {path}"
    );
}

#[test]
fn an_ignored_directory_where_head_has_a_file_is_pinned_before_the_discard() {
    let (_tree, repo) = rd1_repo("rd3-rd2-2");
    die_inside_an_attempt(&repo, "rd2-2");
    let report = bounded_resume("the resume", &repo, fake(Effect::EditFile)).expect("the resume");
    assert!(committed(&report, "t1"), "{report:?}");
    assert_eq!(
        file_bytes(&repo, "tracked.txt"),
        Some(b"base\n".to_vec()),
        "HEAD's file"
    );
    let pin = kept_pin_of(&repo, 1);
    assert_eq!(
        blob_at(&repo, &pin, "tracked.txt/paid.txt"),
        Some(b"paid and ignored\n".to_vec()),
        "the pin holds paid.txt"
    );
    the_newest_copy_holds(&repo, &pin, "tracked.txt/paid.txt", b"paid and ignored\n");
}

#[test]
fn an_ignored_file_where_head_has_a_directory_is_pinned_before_the_discard() {
    let (_tree, repo) = rd1_repo("rd3-rd2-2r");
    die_inside_an_attempt(&repo, "rd2-2r");
    let report = bounded_resume("the resume", &repo, fake(Effect::EditFile)).expect("the resume");
    assert!(committed(&report, "t1"), "{report:?}");
    assert_eq!(
        file_bytes(&repo, "sub/t.txt"),
        Some(b"sub base\n".to_vec()),
        "HEAD's directory"
    );
    let pin = kept_pin_of(&repo, 1);
    assert_eq!(
        blob_at(&repo, &pin, "sub"),
        Some(b"paid, a file where a directory was\n".to_vec()),
        "the pin holds the file"
    );
    the_newest_copy_holds(&repo, &pin, "sub", b"paid, a file where a directory was\n");
}

#[test]
fn an_obstruction_a_refusals_pin_lacks_is_refused_and_kept() {
    let (_tree, repo) = rd1_repo("rd3-rd2-2p");
    a_pinned_refusal(&repo, shaping(ignored_directory_where_head_has_a_file));
    let resumed = bounded_resume("the resume", &repo, fake(Effect::EditFile));
    let refusal = resume_refusal(&resumed, "the resume");
    assert!(
        refusal.contains("`tracked.txt/paid.txt`") && refusal.contains("move them out of the way"),
        "{refusal}"
    );
    assert_eq!(
        file_bytes(&repo, "tracked.txt/paid.txt"),
        Some(b"paid and ignored\n".to_vec()),
        "the checkout keeps it"
    );
}

#[cfg(unix)]
#[test]
fn an_unreadable_file_in_the_discards_way_is_refused_and_kept() {
    let (_tree, repo) = rd1_repo("rd3-rd2-2u");
    die_inside_an_attempt(&repo, "rd2-2u");
    let locked = repo.join("tracked.txt").join("locked.txt");
    let binds = fs::read(&locked).is_err();
    let first = bounded_resume("the first resume", &repo, fake(Effect::EditFile));
    let second = bounded_resume("the second resume", &repo, fake(Effect::EditFile));
    mode_of(&locked, 0o644).expect("readable again");
    assert!(binds, "prerequisite not met: mode 0 did not bind (root)");
    for (which, resumed) in [("first", &first), ("second", &second)] {
        let refusal = resume_refusal(resumed, which);
        assert!(
            refusal.contains("keeping them before the discard stopped"),
            "{which}: {refusal}"
        );
    }
    assert_eq!(
        file_bytes(&repo, "tracked.txt/paid.txt"),
        Some(b"paid and ignored\n".to_vec())
    );
    assert_eq!(
        file_bytes(&repo, "tracked.txt/locked.txt"),
        Some(b"paid, unreadable\n".to_vec())
    );
}

#[test]
fn a_nested_repository_in_the_discards_way_is_refused_and_kept() {
    let (_tree, repo) = rd1_repo("rd3-rd2-2n");
    die_inside_an_attempt(&repo, "rd2-2n");
    let nested = repo.join("tracked.txt").join("nested");
    let nested_head = git_in(&nested, &["rev-parse", "HEAD"]).trim().to_owned();
    for which in ["first", "second"] {
        let resumed = bounded_resume(which, &repo, fake(Effect::EditFile));
        let refusal = resume_refusal(&resumed, which);
        assert!(refusal.contains("stopped part-way"), "{which}: {refusal}");
    }
    let pin = kept_pin_of(&repo, 1);
    assert_eq!(
        blob_at(&repo, &pin, "tracked.txt/paid.txt"),
        Some(b"paid and ignored\n".to_vec()),
        "the pin holds the paid file"
    );
    assert_eq!(
        git_in(&nested, &["rev-parse", "HEAD"]).trim(),
        nested_head,
        "the nested repository stays"
    );
}

#[cfg(unix)]
#[test]
fn a_file_written_after_the_capture_is_never_written_over() {
    let (_tree, repo) = rd1_repo("rd3-rd2-2b");
    die_inside_an_attempt(&repo, "rd2-2b");
    let shim = git_shim(
        &repo,
        "rd2-2b",
        &[ShimRule {
            subcommand: "read-tree",
            holding: " -m ",
            after: false,
            action: format!(
                "  printf 'new uncaptured paid bytes\\n' > '{}'",
                repo.join("deleted.txt").display()
            ),
        }],
    );
    let child = child_engine(&repo, "rd2-2b");
    let report = child.run(
        "the first resume",
        &repo,
        "kept_engine_child_resumes",
        &[("PATH", &shim.path())],
    );
    assert_the_shim_fired(
        &shim,
        &child,
        &report,
        &[
            repo.join("deleted.txt"),
            repo.with_file_name("dies-inside.log"),
        ],
        format_args!("the late write ran before the revert"),
    );
    assert!(
        report.starts_with("err")
            && report.contains("stopped part-way")
            && report.contains("deleted.txt"),
        "{report}"
    );
    assert_eq!(
        file_bytes(&repo, "deleted.txt"),
        Some(b"new uncaptured paid bytes\n".to_vec()),
        "the bytes written after the capture stay"
    );
    let next = bounded_resume("the next resume", &repo, fake(Effect::EditFile));
    let refusal = resume_refusal(&next, "the next resume");
    assert!(
        refusal.contains("holds otherwise"),
        "X3's refusal: {refusal}"
    );
    assert_eq!(
        file_bytes(&repo, "deleted.txt"),
        Some(b"new uncaptured paid bytes\n".to_vec())
    );
}

fn autocrlf_through(repo: &Path, condition: &str) {
    let settings = repo.with_file_name("autocrlf.inc");
    fs::write(&settings, "[core]\n\tautocrlf = true\n").expect("the included settings");
    let settings = settings.to_string_lossy().replace('\\', "/");
    let git_dir = git_in(repo, &["rev-parse", "--absolute-git-dir"])
        .trim()
        .to_owned();
    match condition {
        "onbranch" => {
            git_in(
                repo,
                &[
                    "config",
                    "--add",
                    "includeIf.onbranch:upstroke/**.path",
                    &settings,
                ],
            );
        }
        "gitdir-exact" => {
            git_in(
                repo,
                &[
                    "config",
                    "--add",
                    &format!("includeIf.gitdir:{git_dir}.path"),
                    &settings,
                ],
            );
        }
        "gitdir-suffix" => {
            git_in(
                repo,
                &[
                    "config",
                    "--add",
                    "includeIf.gitdir:**/.git.path",
                    &settings,
                ],
            );
        }
        "worktree-relative" => {
            git_in(repo, &["config", "extensions.worktreeConfig", "true"]);
            fs::write(
                Path::new(&git_dir).join("config.worktree"),
                "[include]\n\tpath = autocrlf-wt.inc\n",
            )
            .expect("config.worktree");
            fs::write(
                Path::new(&git_dir).join("autocrlf-wt.inc"),
                "[core]\n\tautocrlf = true\n",
            )
            .expect("the relative include");
        }
        "hasconfig" => {
            git_in(
                repo,
                &["remote", "add", "origin", "https://example.invalid/r.git"],
            );
            git_in(
                repo,
                &[
                    "config",
                    "--add",
                    "includeIf.hasconfig:remote.*.url:https://example.invalid/**.path",
                    &settings,
                ],
            );
        }
        _ => {
            let checkout = git_in(repo, &["rev-parse", "--show-toplevel"])
                .trim()
                .to_owned();
            git_in(
                repo,
                &[
                    "config",
                    "--add",
                    &format!("includeIf.gitdir:{checkout}/.upstroke/.path"),
                    &settings,
                ],
            );
        }
    }
}

fn a_condition_chooses_the_configuration(condition: &str, turns_it_on: bool) {
    let (_tree, repo) = rd1_repo(&format!("rd3-rd2-3-{condition}"));
    autocrlf_through(&repo, condition);
    let pin = a_pinned_refusal(&repo, shaping(crlf_output));
    let effective = git_in(&repo, &["config", "--get", "core.autocrlf"])
        .trim()
        .to_owned();
    assert_eq!(
        effective == "true",
        turns_it_on,
        "{condition}: the checkout's autocrlf is {effective}"
    );
    let stored: &[u8] = if turns_it_on {
        b"paid output\n"
    } else {
        b"paid output\r\n"
    };
    assert_eq!(
        blob_at(&repo, &pin, "tracked.txt"),
        Some(stored.to_vec()),
        "{condition}: the coordinator's pin, as the checkout's Git stores it"
    );
    resume_and_die_inside_the_attempt(&repo);
    let report = bounded_resume(condition, &repo, fake(Effect::EditFile))
        .unwrap_or_else(|error| panic!("{condition}: the second resume: {error:?}"));
    assert!(committed(&report, "t1"), "{condition}: {report:?}");
    assert_eq!(
        blob_at(&repo, &pin, "tracked.txt"),
        Some(stored.to_vec()),
        "{condition}: the pin holds the output"
    );
    assert_eq!(status_of(&repo), "", "{condition}: the checkout is clean");
}

#[test]
fn a_branch_conditioned_configuration_is_the_captures_configuration() {
    a_condition_chooses_the_configuration("onbranch", true);
}

#[test]
fn an_exact_gitdir_conditioned_configuration_is_the_captures_configuration() {
    a_condition_chooses_the_configuration("gitdir-exact", true);
}

#[test]
fn a_suffix_gitdir_conditioned_configuration_is_the_captures_configuration() {
    a_condition_chooses_the_configuration("gitdir-suffix", true);
}

#[test]
fn a_relative_include_in_the_worktree_configuration_is_the_captures_configuration() {
    a_condition_chooses_the_configuration("worktree-relative", true);
}

#[test]
fn a_remote_conditioned_configuration_is_read_alike() {
    a_condition_chooses_the_configuration("hasconfig", true);
}

#[test]
fn a_condition_only_a_private_directory_meets_changes_no_byte() {
    let (_tree, repo) = rd1_repo("rd3-rd2-3p");
    autocrlf_through(&repo, "private-only");
    assert_ne!(
        git_in(&repo, &["config", "--get", "core.autocrlf"]).trim(),
        "true",
        "the checkout itself does not meet the condition"
    );
    die_inside_an_attempt(&repo, "crlf");
    let report = bounded_resume("the resume", &repo, fake(Effect::EditFile)).expect("the resume");
    assert!(committed(&report, "t1"), "{report:?}");
    assert_eq!(
        blob_at(&repo, &kept_pin_of(&repo, 1), "tracked.txt"),
        Some(b"paid output\r\n".to_vec()),
        "the resume's pin holds the CRLF bytes exactly"
    );
}

fn status_reads(repo: &Path) -> bool {
    let status = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["status", "--porcelain"])
        .output()
        .expect("run git status");
    let listed = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["ls-files"])
        .output()
        .expect("run git ls-files");
    status.status.success() && listed.status.success()
}

fn split_the_checkouts_index(repo: &Path, through_config: bool) {
    if through_config {
        git_in(repo, &["config", "core.splitIndex", "true"]);
    }
    git_in(repo, &["config", "splitIndex.sharedIndexExpire", "now"]);
    git_in(repo, &["update-index", "--split-index"]);
    assert!(
        !shared_indexes_of(repo).is_empty(),
        "the checkout's index is split"
    );
}

#[test]
fn a_capture_leaves_a_split_index_readable_and_pins() {
    let (_tree, repo) = rd1_repo("rd4-rd3-1");
    split_the_checkouts_index(&repo, true);
    let run = bounded_run(
        "N1's arm on a split index",
        &repo,
        Some(record_shared_indexes_and_fail_after_capture),
        fake(Effect::EditFile),
    );
    let after = shared_indexes_of(&repo).join("\n");
    let at_the_arm = fs::read_to_string(shared_indexes_marker(&repo)).expect("the arm's record");
    let Err(UpstrokeError::WithWarnings(warned)) = &run else {
        panic!("the attempt's error, with the kept note: {run:?}");
    };
    assert!(
        warned
            .warnings
            .iter()
            .any(|warning| warning.contains("is kept in this checkout and pinned at")),
        "{run:?}"
    );
    assert_eq!(
        after, at_the_arm,
        "the arm adds and removes no shared index"
    );
    assert!(
        status_reads(&repo),
        "git ls-files and git status read the checkout's index"
    );
    let commit = pin_target(&repo, &kept_pin_of(&repo, 1)).expect("the kept pin");
    assert_eq!(
        blob_at(&repo, &commit, "agent-output.txt"),
        Some(b"edited: 0\n".to_vec()),
        "the output is pinned"
    );
    let report =
        bounded_resume("the resume", &repo, fake(Effect::EditFile)).expect("a resume completes");
    assert!(committed(&report, "t1"), "{report:?}");
    assert!(status_reads(&repo));
}

#[test]
fn a_resume_capture_leaves_a_split_index_readable() {
    let (_tree, repo) = rd1_repo("rd4-rd3-1g");
    split_the_checkouts_index(&repo, true);
    a_pinned_refusal(&repo, fake(Effect::EditFile));
    let report =
        bounded_resume("the resume", &repo, fake(Effect::EditFile)).expect("the resume completes");
    assert!(committed(&report, "t1"), "{report:?}");
    assert!(status_reads(&repo), "git status reads the checkout's index");
}

#[test]
fn an_inherited_split_index_variable_expires_nothing() {
    let (_tree, repo) = rd1_repo("rd4-rd3-1e");
    split_the_checkouts_index(&repo, false);
    let forced = std::ffi::OsStr::new("1");
    let child = child_engine(&repo, "rd3-1e-run");
    let report = child.run(
        "N1's arm under an inherited GIT_TEST_SPLIT_INDEX",
        &repo,
        "kept_engine_child_runs_and_fails_after_capture",
        &[("GIT_TEST_SPLIT_INDEX", forced)],
    );
    assert!(
        report.contains("is kept in this checkout and pinned at"),
        "{report}"
    );
    assert!(status_reads(&repo), "the checkout's index stays readable");
    let pin = kept_pin_of(&repo, 1);
    assert!(
        blob_at(&repo, &pin, "agent-output.txt").is_some(),
        "and the output pinned"
    );
    let child = child_engine(&repo, "rd3-1e-resume");
    let report = child.run(
        "the resume's capture under an inherited GIT_TEST_SPLIT_INDEX",
        &repo,
        "kept_engine_child_resumes",
        &[("GIT_TEST_SPLIT_INDEX", forced)],
    );
    assert!(report.starts_with("ok"), "{report}");
    assert!(status_reads(&repo), "and after the resume's capture too");
}

#[cfg(unix)]
#[test]
fn the_discard_never_queries_the_checkouts_fsmonitor() {
    use std::os::unix::fs::PermissionsExt;
    let (_tree, repo) = rd1_repo("rd4-rd3-1f");
    let hook = repo.with_file_name("fsmonitor-hook.sh");
    let log = repo.with_file_name("fsmonitor-hook.log");
    fs::write(
        &hook,
        format!(
            "#!/bin/sh\necho \"$(pwd -P) $*\" >> '{}'\nprintf 'upstroke-%s\\0/\\0' \"$(date +%s)\"\n",
            log.display()
        ),
    )
    .expect("the hook");
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).expect("an executable hook");
    git_in(
        &repo,
        &["config", "core.fsmonitor", &hook.to_string_lossy()],
    );
    git_in(&repo, &["config", "core.fsmonitorHookVersion", "2"]);
    git_in(&repo, &["status", "--porcelain"]);
    let checkout = fs::canonicalize(&repo)
        .expect("the checkout")
        .display()
        .to_string();
    let calls_on_the_checkout = || {
        fs::read_to_string(&log)
            .unwrap_or_default()
            .lines()
            .filter(|line| line.split(' ').next() == Some(checkout.as_str()))
            .count()
    };
    let by_the_user = calls_on_the_checkout();
    assert!(by_the_user > 0, "the hook is live for the checkout");
    a_pinned_refusal(&repo, fake(Effect::EditFile));
    let report =
        bounded_resume("the resume", &repo, fake(Effect::EditFile)).expect("the resume completes");
    assert!(committed(&report, "t1"), "{report:?}");
    assert_eq!(
        calls_on_the_checkout(),
        by_the_user,
        "no engine child queried the checkout's fsmonitor"
    );
}

#[cfg(unix)]
fn listing_at_the_first_revert(repo: &Path, label: &str) -> (GitShim, PathBuf) {
    let run_id = rundir::latest_run(repo).expect("the run");
    let public = paths_of(repo, &run_id).public;
    let listing = repo.with_file_name(format!("{label}-listing.txt"));
    let shim = git_shim(
        repo,
        label,
        &[ShimRule {
            subcommand: "read-tree",
            holding: " -m ",
            after: false,
            action: format!("  ls '{}' > '{}'", public.display(), listing.display()),
        }],
    );
    (shim, listing)
}

#[cfg(unix)]
fn copies_listed(listing: &Path) -> (Vec<String>, Vec<String>) {
    let text = fs::read_to_string(listing).expect("the listing at the first read-tree -m");
    let copies = text
        .lines()
        .filter(|name| name.starts_with("kept-0-1-") && name.ends_with(".bundle"))
        .map(str::to_owned)
        .collect();
    let partials = text
        .lines()
        .filter(|name| name.ends_with(".partial"))
        .map(str::to_owned)
        .collect();
    (copies, partials)
}

#[cfg(unix)]
#[test]
fn a_resume_writes_and_syncs_its_own_copy_before_the_revert() {
    let (_tree, repo) = rd1_repo("rd4-rd3-2");
    let pin = a_pinned_refusal(&repo, fake(Effect::EditFile));
    let planted = plant_a_copy(&repo, &pin, ".bundle");
    let planted_bytes = sha256_of(&planted);
    let planted_name = planted
        .file_name()
        .and_then(|name| name.to_str())
        .expect("a name")
        .to_owned();
    let (shim, listing) = listing_at_the_first_revert(&repo, "rd3-2");
    let child = child_engine(&repo, "rd3-2");
    let report = child.run(
        "the resume",
        &repo,
        "kept_engine_child_resumes",
        &[("PATH", &shim.path())],
    );
    assert!(report.starts_with("ok"), "{report}");
    assert!(shim.fired(0), "the revert ran");
    let (copies, partials) = copies_listed(&listing);
    let [first, second] = copies.as_slice() else {
        panic!("at the first read-tree -m, the planted copy and one more: {copies:?}");
    };
    assert_eq!(first, &planted_name, "the planted copy");
    assert!(
        second > first,
        "a second copy, under a newer name: {copies:?}"
    );
    assert!(
        partials.is_empty(),
        "renamed into place before the revert: {partials:?}"
    );
    assert_eq!(
        sha256_of(&planted),
        planted_bytes,
        "the planted copy is byte-identical"
    );
    let fresh = planted.with_file_name(second);
    let heads = git_in(&repo, &["bundle", "list-heads", &fresh.to_string_lossy()]);
    assert!(
        heads.trim().ends_with(&pin),
        "the new copy names the pin: {heads}"
    );
}

#[cfg(unix)]
#[test]
fn a_pin_restored_from_a_copy_is_copied_afresh_before_the_revert() {
    let (_tree, repo) = rd1_repo("rd4-rd3-2r");
    let pin = a_pinned_refusal(&repo, fake(Effect::EditFile));
    let planted = plant_a_copy(&repo, &pin, ".bundle");
    git_in(&repo, &["update-ref", "-d", &pin]);
    let (shim, listing) = listing_at_the_first_revert(&repo, "rd3-2r");
    let child = child_engine(&repo, "rd3-2r");
    let report = child.run(
        "the resume",
        &repo,
        "kept_engine_child_resumes",
        &[("PATH", &shim.path())],
    );
    assert!(report.starts_with("ok"), "{report}");
    let (copies, _) = copies_listed(&listing);
    assert_eq!(
        copies.len(),
        2,
        "the new copy exists at the first read-tree -m: {copies:?}"
    );
    let restored = report
        .lines()
        .find(|line| line.contains("was put back from its copy"))
        .unwrap_or_else(|| panic!("the restored warning: {report}"));
    assert!(
        restored.contains(&planted.display().to_string()),
        "{restored}"
    );
    let fresh = copies.iter().max().expect("the newer copy");
    let copied = report
        .lines()
        .find(|line| line.contains("copied outside every ref"))
        .unwrap_or_else(|| panic!("the copy warning: {report}"));
    assert!(
        copied.contains(fresh.as_str()),
        "the copy warning names the new copy: {copied}"
    );
    assert!(pin_target(&repo, &pin).is_some(), "the pin is back");
}

#[cfg(unix)]
#[test]
fn a_resume_that_cannot_write_its_own_copy_refuses_and_discards_nothing() {
    let (_tree, repo) = rd1_repo("rd4-rd3-2p");
    let pin = a_pinned_refusal(&repo, fake(Effect::EditFile));
    plant_a_copy(&repo, &pin, ".bundle");
    let run_id = rundir::latest_run(&repo).expect("the run");
    let public = paths_of(&repo, &run_id).public;
    let before = status_of(&repo);
    mode_of(&public, 0o555).expect("a run directory that takes no new file");
    let binds = mode_bits_bind(&public);
    let first = if binds {
        Some(bounded_resume("the resume", &repo, fake(Effect::EditFile)))
    } else {
        None
    };
    mode_of(&public, 0o755).expect("writable again");
    let first = first.expect("prerequisite not met: the mode bit did not bind (root)");
    let refusal = resume_refusal(&first, "the resume");
    assert!(
        refusal.contains("keeping them before the discard stopped") && refusal.contains("bundle"),
        "the copy's own failure: {refusal}"
    );
    assert_eq!(status_of(&repo), before, "nothing discarded");
    let report =
        bounded_resume("the next resume", &repo, fake(Effect::EditFile)).expect("it completes");
    assert!(committed(&report, "t1"), "{report:?}");
}

struct UnparsedWorker {
    inner: FakeAdapter,
}

impl AgentAdapter for UnparsedWorker {
    fn id(&self) -> &'static str {
        self.inner.id()
    }

    fn probe(&self, runner: &dyn crate::runner::Runner) -> Result<Caps, UpstrokeError> {
        self.inner.probe(runner)
    }

    fn build(&self, run: &TaskRun) -> Result<CommandSpec, UpstrokeError> {
        if run.profile.permissions != PermissionMode::ReadOnly {
            paid_output(&run.workspace).map_err(after_capture_failed)?;
        }
        self.inner.build(run)
    }

    fn parse(&self, out: &ProcessOutput) -> Result<Outcome, UpstrokeError> {
        if out.stdout.contains(REVIEW_MARKER) {
            return self.inner.parse(out);
        }
        Err(UpstrokeError::Agent {
            message: "the worker's output could not be parsed".to_owned(),
        })
    }

    fn materialize_permissions(
        &self,
        profile: &WorkerProfile,
        gate_cmds: &[String],
        dir: &Path,
        stem: &str,
    ) -> Result<Option<PathBuf>, UpstrokeError> {
        self.inner
            .materialize_permissions(profile, gate_cmds, dir, stem)
    }
}

#[test]
fn an_error_after_the_worker_wrote_and_before_its_capture_keeps_the_output_pinned() {
    let (_tree, repo) = rd1_repo("rd2-n1d");
    let run = bounded_run(
        "the run whose worker's output cannot be parsed",
        &repo,
        None,
        OneAdapter {
            adapter: UnparsedWorker {
                inner: FakeAdapter::new(vec![Effect::NoEdit], vec![ReviewBehavior::Pass]),
            },
        },
    );
    let Err(UpstrokeError::WithWarnings(warned)) = &run else {
        panic!("the parse's own error, with the kept note: {run:?}");
    };
    assert!(
        warned.error.to_string().contains("could not be parsed"),
        "{run:?}"
    );
    assert!(
        warned
            .warnings
            .iter()
            .any(|warning| warning.contains("is kept in this checkout and pinned at")),
        "{run:?}"
    );
    assert_eq!(
        file_bytes(&repo, "agent-output.txt"),
        Some(b"paid output\n".to_vec()),
        "the checkout keeps the output"
    );
    let commit = pin_target(&repo, &kept_pin_of(&repo, 1)).expect("the kept pin");
    assert_eq!(
        blob_at(&repo, &commit, "agent-output.txt"),
        Some(b"paid output\n".to_vec()),
        "and the pin holds it"
    );
}

#[test]
fn an_earlier_attempts_retained_output_is_kept_when_the_next_worker_cannot_start() {
    let (_tree, repo) = temp_engine_repo("rd2-n1e");
    seed(
        &repo,
        "## Implement the widget\n<!-- upstroke: id=t1 kind=implement depends= -->\n",
        Some("[routing]\nimplement = { chain = [\"small\"], attempts_per = 2 }\n"),
    );
    let run = bounded_run(
        "the same-session retry whose worker cannot start",
        &repo,
        None,
        source(
            vec![Effect::EditFile, Effect::SpawnError],
            vec![ReviewBehavior::Fail],
        ),
    );
    let Err(UpstrokeError::WithWarnings(warned)) = &run else {
        panic!("the spawn's own error, with the kept note: {run:?}");
    };
    assert!(
        warned
            .warnings
            .iter()
            .any(|warning| warning
                .contains("attempt 2 of `t1` is kept in this checkout and pinned at")),
        "{run:?}"
    );
    let commit = pin_target(&repo, &kept_pin_of(&repo, 2)).expect("attempt 2's kept pin");
    assert_eq!(
        blob_at(&repo, &commit, "agent-output.txt"),
        Some(b"edited: 0\n".to_vec()),
        "it holds the output attempt 1 left for the retry"
    );
}

#[cfg(unix)]
fn modes_under(directory: &Path, mode: u32) -> std::io::Result<()> {
    mode_of(directory, mode)?;
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_dir() && !path.is_symlink() {
            modes_under(&path, mode)?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn make_the_objects_read_only_and_tear_after_capture(
    workspace: &Workspace,
    candidate: &crate::workspace::CapturedCandidate,
) -> Result<(), UpstrokeError> {
    tear_after_capture(workspace, candidate)?;
    modes_under(&workspace.root().join(".git").join("objects"), 0o555).map_err(after_capture_failed)
}

#[cfg(unix)]
fn make_the_reflogs_read_only_and_tear_after_capture(
    workspace: &Workspace,
    candidate: &crate::workspace::CapturedCandidate,
) -> Result<(), UpstrokeError> {
    tear_after_capture(workspace, candidate)?;
    git_after_capture(
        workspace.root(),
        &["config", "core.logAllRefUpdates", "always"],
    )?;
    let logs = workspace.root().join(".git").join("logs");
    fs::create_dir_all(&logs).map_err(after_capture_failed)?;
    modes_under(&logs, 0o555).map_err(after_capture_failed)
}

#[cfg(unix)]
#[test]
fn a_store_fault_at_the_pins_objects_or_its_reflog_loses_nothing() {
    let faults: [(&str, super::options::AfterCandidateCapture, &str); 2] = [
        (
            "objects",
            make_the_objects_read_only_and_tear_after_capture,
            "objects",
        ),
        (
            "reflog",
            make_the_reflogs_read_only_and_tear_after_capture,
            "logs",
        ),
    ];
    for (fault, after_capture, directory) in faults {
        let (_tree, repo) = rd1_repo(&format!("rd1-b-{fault}"));
        let run = bounded_run(fault, &repo, Some(after_capture), fake(Effect::EditFile));
        let store = repo.join(".git").join(directory);
        let binds = mode_bits_bind(&store);
        modes_under(&store, 0o755).expect("the fault healed");
        assert!(
            binds,
            "{fault}: prerequisite not met: the mode bit did not bind (root, or CAP_DAC_OVERRIDE)"
        );
        let ended = format!("{run:?}");
        assert!(
            run.is_err() && ended.contains("pinning it at") && ended.contains("failed"),
            "{fault}: the run ends with the checkout kept and the pin failed: {ended}"
        );
        assert_eq!(
            kept_pins(&repo),
            Vec::<(String, String)>::new(),
            "{fault}: no pin"
        );
        assert!(
            status_of(&repo).contains("agent-output.txt"),
            "{fault}: nothing discarded"
        );
        repair_the_torn_registration(&repo);
        let report = bounded_resume(fault, &repo, fake(Effect::EditFile))
            .unwrap_or_else(|error| panic!("{fault}: the resume once healed: {error:?}"));
        assert!(committed(&report, "t1"), "{fault}: {report:?}");
        let (_, _, tree) = captured(&repo);
        let commit = pin_target(&repo, &kept_pin_of(&repo, 1))
            .unwrap_or_else(|| panic!("{fault}: the resume wrote the kept pin"));
        assert_eq!(tree_of(&repo, &commit), tree, "{fault}: holding the output");
    }
}

#[test]
fn every_kind_of_change_is_pinned_and_an_ignored_file_survives() {
    let (_tree, repo) = rd1_repo("rd1-x4");
    die_inside_an_attempt(&repo, "x4");
    let report = bounded_resume("the resume", &repo, fake(Effect::EditFile)).expect("the resume");
    assert!(committed(&report, "t1"), "{report:?}");
    assert_eq!(
        file_bytes(&repo, "ignored.flag"),
        Some(b"ignored, never captured, never discarded\n".to_vec()),
        "the ignored file survives the discard"
    );
    let pin = kept_pin_of(&repo, 1);
    for (path, bytes) in [
        ("tracked.txt", &b"paid edit\n"[..]),
        ("new.txt", b"paid new file\n"),
        ("newdir/inner.txt", b"paid, in a new directory\n"),
    ] {
        assert_eq!(blob_at(&repo, &pin, path), Some(bytes.to_vec()), "{path}");
    }
    assert!(
        blob_at(&repo, &pin, "deleted.txt").is_none(),
        "the deletion"
    );
    assert!(
        blob_at(&repo, &pin, "ignored.flag").is_none(),
        "no ignored file"
    );
    #[cfg(unix)]
    {
        let listing = recorded(&repo, &["ls-tree", "-r", "--full-tree", &pin]);
        assert!(
            listing
                .lines()
                .any(|line| line.starts_with("100755 ") && line.ends_with("\tsub/t.txt")),
            "the executable bit: {listing}"
        );
        assert!(
            listing
                .lines()
                .any(|line| line.starts_with("120000 ") && line.ends_with("\tlink")),
            "the symbolic link: {listing}"
        );
    }
    let warning = kept_pin_warning(&report).expect("the resume names the pin");
    follow_the_restore(&repo, warning, &pin);
    let differences = recorded(&repo, &["diff", "--name-status", &pin, "--"]);
    assert_eq!(
        differences, "",
        "the restore reproduces the pin, modes included"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn a_leftover_whose_name_is_not_utf8_is_pinned_then_reverted() {
    use std::os::unix::ffi::OsStrExt;
    let (_tree, repo) = rd1_repo("rd1-x17");
    die_inside_an_attempt(&repo, "x17");
    let report = bounded_resume("the resume", &repo, fake(Effect::EditFile)).expect("the resume");
    assert!(committed(&report, "t1"), "{report:?}");
    assert!(
        !repo
            .join(std::ffi::OsStr::from_bytes(b"caf\xe9.txt"))
            .exists(),
        "reverted"
    );
    let pin = kept_pin_of(&repo, 1);
    let out = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .args(["ls-tree", "-r", "-z", "--name-only", &pin])
        .output()
        .expect("run git ls-tree");
    assert!(
        out.stdout
            .split(|byte| *byte == 0)
            .any(|name| name == b"caf\xe9.txt"),
        "the pin holds the name's own bytes"
    );
}

#[cfg(unix)]
#[test]
fn a_resume_that_dies_before_its_revert_is_finished_by_the_next() {
    let (_tree, repo) = rd1_repo("rd1-x6");
    let pin = a_pinned_refusal(&repo, fake(Effect::EditFile));
    let (_, _, tree) = captured(&repo);
    let shim = git_shim(
        &repo,
        "x6",
        &[ShimRule {
            subcommand: "read-tree",
            holding: " -m ",
            after: false,
            action: "  kill -KILL $PPID\n  exit 1".to_owned(),
        }],
    );
    let child = child_engine(&repo, "x6");
    let mut command = this_test_binary("kept_engine_child_resumes");
    command
        .env("UPSTROKE_KEPT_ENGINE_REPO", &repo)
        .env("UPSTROKE_KEPT_ENGINE_REPORT", &child.report)
        .env("PATH", shim.path());
    let (status, log) = bounded_child(
        "the resume that dies before its revert",
        command,
        &child.log,
    );
    assert!(
        shim.fired(0),
        "the resume died at its revert's first child: {log}"
    );
    assert!(status.code().is_none(), "by a signal: {status:?}");
    assert_eq!(kept_copies(&repo).len(), 1, "after its copy");
    assert!(
        status_of(&repo).contains("agent-output.txt"),
        "nothing reverted"
    );
    let report = bounded_resume("the next resume", &repo, fake(Effect::EditFile))
        .expect("the next resume completes");
    assert!(committed(&report, "t1"), "{report:?}");
    assert_eq!(
        kept_copies(&repo).len(),
        2,
        "it wrote a fresh copy of its own before its revert"
    );
    assert_eq!(
        tree_of(&repo, &pin_target(&repo, &pin).expect("the pin")),
        tree,
        "the pin holds the output"
    );
}

#[cfg(unix)]
#[test]
fn a_hostile_repository_keeps_its_index_readable_and_its_monitor_unqueried() {
    use std::os::unix::fs::PermissionsExt;
    let (_tree, repo) = rd1_repo("rd4-hostile");
    let hook = repo.with_file_name("fsmonitor-hook.sh");
    let log = repo.with_file_name("fsmonitor-hook.log");
    fs::write(
        &hook,
        format!(
            "#!/bin/sh\necho \"$(pwd -P) $*\" >> '{}'\nprintf 'upstroke-%s\\0/\\0' \"$(date +%s)\"\n",
            log.display()
        ),
    )
    .expect("the hook");
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).expect("an executable hook");
    for (key, value) in [
        ("core.untrackedCache", "true"),
        ("core.fsmonitor", &*hook.to_string_lossy()),
        ("core.fsmonitorHookVersion", "2"),
    ] {
        git_in(&repo, &["config", key, value]);
    }
    split_the_checkouts_index(&repo, true);
    git_in(&repo, &["status", "--porcelain"]);
    let checkout = fs::canonicalize(&repo)
        .expect("the checkout")
        .display()
        .to_string();
    let calls_on_the_checkout = || {
        fs::read_to_string(&log)
            .unwrap_or_default()
            .lines()
            .filter(|line| line.split(' ').next() == Some(checkout.as_str()))
            .count()
    };
    let by_the_user = calls_on_the_checkout();
    assert!(by_the_user > 0, "the hook is live for the checkout");
    let run = bounded_run(
        "N1's arm in the hostile repository",
        &repo,
        Some(record_shared_indexes_and_fail_after_capture),
        fake(Effect::EditFile),
    );
    let during_the_run = calls_on_the_checkout() - by_the_user;
    let after = shared_indexes_of(&repo).join("\n");
    let at_the_arm = fs::read_to_string(shared_indexes_marker(&repo)).expect("the arm's record");
    assert!(
        format!("{run:?}").contains("is kept in this checkout and pinned at"),
        "{run:?}"
    );
    assert_eq!(
        after, at_the_arm,
        "the pin adds and removes no shared index"
    );
    assert!(status_reads(&repo), "the index reads after the pin");
    let before_the_resume = calls_on_the_checkout();
    let report =
        bounded_resume("the resume", &repo, fake(Effect::EditFile)).expect("the resume completes");
    let during_the_resume = calls_on_the_checkout() - before_the_resume;
    assert!(committed(&report, "t1"), "{report:?}");
    assert!(status_reads(&repo), "and after the resume");
    assert_eq!(
        (during_the_run, during_the_resume),
        (0, 0),
        "no engine child queried the checkout's fsmonitor"
    );
}

fn filtered_output(root: &Path) -> std::io::Result<()> {
    write_into(root, ".gitattributes", FILTERED_ATTRIBUTE)?;
    write_into(root, "paid.dat", FILTERED_PAID_BYTES)?;
    paid_output(root)
}

const FILTERED_ATTRIBUTE: &[u8] = b"*.dat filter=lfs\n";

const FILTERED_PAID_BYTES: &[u8] = b"paid bytes \x00\x01\xfe\xff\n";

fn the_filtered_output_is_in_the_checkout(repo: &Path) {
    for (path, bytes) in [
        (".gitattributes", FILTERED_ATTRIBUTE),
        ("paid.dat", FILTERED_PAID_BYTES),
        ("agent-output.txt", b"paid output\n".as_slice()),
    ] {
        assert_eq!(
            file_bytes(repo, path),
            Some(bytes.to_vec()),
            "the checkout keeps {path} as the worker wrote it"
        );
    }
}

fn a_run_over_a_filtered_output(repo: &Path) -> Result<RunReport, UpstrokeError> {
    bounded_run(
        "the run whose worker attributes a clean/smudge filter to its output",
        repo,
        None,
        shaping(filtered_output),
    )
}

#[test]
fn a_filtered_output_the_pin_cannot_capture_is_kept_and_its_warning_says_how_to_resume() {
    let (_tree, repo) = rd1_repo("full-f1-warn");
    let run = a_run_over_a_filtered_output(&repo);
    let Err(UpstrokeError::WithWarnings(warned)) = &run else {
        panic!("the capture's own refusal, with the kept note: {run:?}");
    };
    assert!(
        matches!(
            warned.error.as_ref(),
            UpstrokeError::Git { message }
                if message.contains("uses clean/smudge filter `lfs`")
                    && message.contains("refusing before git add")
        ),
        "the error is the capture's filter refusal: {run:?}"
    );
    let pin = kept_pin_of(&repo, 1);
    assert_eq!(pin_target(&repo, &pin), None, "no pin holds the output");
    assert_eq!(kept_pins(&repo), Vec::<(String, String)>::new());
    assert!(kept_copies(&repo).is_empty(), "and no copy");
    the_filtered_output_is_in_the_checkout(&repo);
    let warning = warned
        .warnings
        .iter()
        .find(|warning| warning.contains("pinning it at"))
        .expect("the kept note");
    for needle in [
        format!("is kept in this checkout, and pinning it at `{pin}` failed"),
        "uses clean/smudge filter `lfs`".to_owned(),
        "Nothing outside this checkout keeps it".to_owned(),
        "`git clean` or `git reset --hard` there would lose it".to_owned(),
        "remove the attribute that sets it, after copying out anything to keep".to_owned(),
        "and then resume".to_owned(),
    ] {
        assert!(
            warning.contains(needle.as_str()),
            "the note says {needle:?}: {warning}"
        );
    }
}

#[test]
fn a_resume_over_a_filtered_output_refuses_saying_how_and_pins_it_once_the_attribute_is_gone() {
    let (_tree, repo) = rd1_repo("full-f1-resume");
    let run = a_run_over_a_filtered_output(&repo);
    assert!(
        run.is_err(),
        "the capture refuses the filtered path: {run:?}"
    );
    let pin = kept_pin_of(&repo, 1);
    let refused = bounded_resume(
        "the resume over the filtered output",
        &repo,
        fake(Effect::EditFile),
    );
    assert_eq!(
        pin_target(&repo, &pin),
        None,
        "the refused resume pins nothing"
    );
    assert!(kept_copies(&repo).is_empty(), "and copies nothing");
    the_filtered_output_is_in_the_checkout(&repo);
    let refusal = resume_refusal(&refused, "the resume over the filtered output");
    for needle in [
        "attempt 1 of `t1` is in flight".to_owned(),
        "uses clean/smudge filter `lfs`".to_owned(),
        "Nothing was discarded".to_owned(),
        "`git clean` or `git reset --hard` would lose any of it".to_owned(),
        "remove the attribute that sets it, after copying out anything to keep".to_owned(),
        format!("the resume discards none of it before `{pin}`"),
    ] {
        assert!(
            refusal.contains(needle.as_str()),
            "the refusal says {needle:?}: {refusal}"
        );
    }

    fs::copy(
        repo.join(".gitattributes"),
        repo.with_file_name("gitattributes-set-aside"),
    )
    .expect("the operator copies it out");
    fs::remove_file(repo.join(".gitattributes")).expect("and removes the attribute");
    let report = bounded_resume(
        "the resume once the attribute is gone",
        &repo,
        fake(Effect::EditFile),
    )
    .expect("the resume completes once the attribute is gone");
    assert!(committed(&report, "t1"), "{report:?}");
    let commit = pin_target(&repo, &pin).expect("the resume pinned the attempt's output");
    assert_eq!(
        blob_at(&repo, &commit, "paid.dat"),
        Some(FILTERED_PAID_BYTES.to_vec()),
        "the pin holds the paid bytes"
    );
    assert_eq!(
        blob_at(&repo, &commit, "agent-output.txt"),
        Some(b"paid output\n".to_vec()),
        "and the rest of the output"
    );
    assert!(
        warning_with(&report, "copied outside every ref")
            .is_some_and(|warning| warning.contains(pin.as_str())),
        "{report:?}"
    );
    let copies = kept_copies(&repo);
    let [copy] = copies.as_slice() else {
        panic!("one copy: {copies:?}");
    };
    git_in(&repo, &["update-ref", "-d", &pin]);
    restore_the_pin_from(&repo, copy, &pin);
    assert_eq!(
        pin_target(&repo, &pin),
        Some(commit),
        "the copy puts the pin back"
    );
}

#[cfg(unix)]
fn marked_filter_output(root: &Path) -> std::io::Result<()> {
    write_into(root, ".gitattributes", b"*.dat filter=paid-marker\n")?;
    write_into(root, "paid.dat", FILTERED_PAID_BYTES)?;
    paid_output(root)
}

#[cfg(unix)]
#[test]
fn no_engine_git_child_runs_the_filter_a_worker_attributes_its_output_to() {
    let (_tree, repo) = rd1_repo("full-f1-marker");
    let marker = repo.with_file_name("filter-ran.log");
    let command = format!("printf ran >> '{}'; cat", marker.display());
    for key in ["filter.paid-marker.clean", "filter.paid-marker.smudge"] {
        git_in(&repo, &["config", key, &command]);
    }
    fs::write(repo.join(".gitattributes"), "*.dat filter=paid-marker\n").expect("an attribute");
    fs::write(repo.join("probe.dat"), "probe\n").expect("a filtered path");
    git_in(&repo, &["hash-object", "probe.dat"]);
    assert!(
        marker.exists(),
        "the configured filter runs when Git evaluates the attribute"
    );
    for path in [
        marker.clone(),
        repo.join("probe.dat"),
        repo.join(".gitattributes"),
    ] {
        fs::remove_file(path).expect("the control's files go");
    }
    let run = bounded_run(
        "the run whose worker attributes a configured filter to its output",
        &repo,
        None,
        shaping(marked_filter_output),
    );
    assert!(
        run.is_err(),
        "the capture refuses the filtered path: {run:?}"
    );
    assert!(!marker.exists(), "no Git child of the run ran the filter");
    let refused = bounded_resume(
        "the resume over the filtered path",
        &repo,
        fake(Effect::EditFile),
    );
    assert!(refused.is_err(), "{refused:?}");
    assert!(!marker.exists(), "nor of the refused resume");
    let moved = repo.with_file_name("paid.dat");
    fs::rename(repo.join("paid.dat"), &moved).expect("the operator moves the path out");
    let report = bounded_resume(
        "the resume once the filtered path is out",
        &repo,
        fake(Effect::EditFile),
    )
    .expect("the resume completes once the filtered path is out");
    assert!(committed(&report, "t1"), "{report:?}");
    assert!(!marker.exists(), "nor of the resume that pinned the rest");
    assert_eq!(
        fs::read(moved).ok(),
        Some(FILTERED_PAID_BYTES.to_vec()),
        "the moved path keeps the paid bytes"
    );
    let commit = pin_target(&repo, &kept_pin_of(&repo, 1)).expect("the rest is pinned");
    assert_eq!(
        blob_at(&repo, &commit, ".gitattributes"),
        Some(b"*.dat filter=paid-marker\n".to_vec()),
        "the pin holds what stayed"
    );
}

const MARKED_ATTRIBUTE: &[u8] = b"*.dat filter=paid-marker\n";

fn marked_output_over_a_tracked_path(root: &Path) -> std::io::Result<()> {
    write_into(root, ".gitattributes", MARKED_ATTRIBUTE)?;
    write_into(root, "paid.dat", FILTERED_PAID_BYTES)?;
    paid_output(root)
}

fn the_marked_output_is_in_the_checkout(repo: &Path) {
    for (path, bytes) in [
        (".gitattributes", MARKED_ATTRIBUTE),
        ("paid.dat", FILTERED_PAID_BYTES),
        ("agent-output.txt", b"paid output\n".as_slice()),
    ] {
        assert_eq!(
            file_bytes(repo, path),
            Some(bytes.to_vec()),
            "the checkout keeps {path} as the worker wrote it"
        );
    }
}

#[cfg(unix)]
fn a_marking_filter(repo: &Path) -> Option<PathBuf> {
    let marker = repo.with_file_name("filter-ran.log");
    let command = format!("printf ran >> '{}'; cat", marker.display());
    for key in ["filter.paid-marker.clean", "filter.paid-marker.smudge"] {
        git_in(repo, &["config", key, &command]);
    }
    fs::write(repo.join(".gitattributes"), MARKED_ATTRIBUTE).expect("an attribute");
    fs::write(repo.join("probe.dat"), "probe\n").expect("a filtered path");
    git_in(repo, &["hash-object", "probe.dat"]);
    assert!(
        marker.exists(),
        "the configured filter runs when Git evaluates the attribute"
    );
    for path in [
        marker.clone(),
        repo.join("probe.dat"),
        repo.join(".gitattributes"),
    ] {
        fs::remove_file(path).expect("the control's files go");
    }
    Some(marker)
}

#[cfg(not(unix))]
fn a_marking_filter(_repo: &Path) -> Option<PathBuf> {
    None
}

fn the_marked_filter_never_ran(marker: Option<&Path>, when: &str) {
    if let Some(marker) = marker {
        assert!(!marker.exists(), "no Git child {when} ran the filter");
    }
}

#[test]
fn a_tracked_filtered_path_moved_out_stays_refused_and_is_pinned_once_its_attribute_is_gone() {
    let (_tree, repo) = rd1_repo("df1r-1-tracked");
    let marker = a_marking_filter(&repo);
    fs::write(repo.join("paid.dat"), "tracked before the run\n").expect("a tracked path");
    git_in(&repo, &["add", "paid.dat"]);
    git_in(&repo, &["commit", "-qm", "track paid.dat"]);
    let run = bounded_run(
        "the run whose worker rewrites a tracked path and attributes a filter to it",
        &repo,
        None,
        shaping(marked_output_over_a_tracked_path),
    );
    the_marked_filter_never_ran(marker.as_deref(), "of the run over the tracked path");
    let Err(UpstrokeError::WithWarnings(warned)) = &run else {
        panic!("the capture's own refusal, with the kept note: {run:?}");
    };
    let warning = warned
        .warnings
        .iter()
        .find(|warning| warning.contains("pinning it at"))
        .expect("the kept note");
    let pin = kept_pin_of(&repo, 1);
    assert_eq!(pin_target(&repo, &pin), None, "no pin holds the output");
    assert!(kept_copies(&repo).is_empty(), "and no copy");
    the_marked_output_is_in_the_checkout(&repo);
    let run_id = rundir::latest_run(&repo).expect("the run started");
    let events = paths_of(&repo, &run_id).events();
    let log = fs::read(&events).expect("the event log the run left");
    let refused = bounded_resume(
        "the resume over the tracked filtered path",
        &repo,
        fake(Effect::EditFile),
    );
    the_marked_filter_never_ran(marker.as_deref(), "of the refused resume");
    let refusal = resume_refusal(&refused, "the resume over the tracked filtered path");
    the_marked_output_is_in_the_checkout(&repo);
    let moved = repo.with_file_name("paid.dat");
    fs::rename(repo.join("paid.dat"), &moved).expect("the tracked path moved out");
    let still = bounded_resume(
        "the resume once the tracked path is moved out",
        &repo,
        fake(Effect::EditFile),
    );
    the_marked_filter_never_ran(marker.as_deref(), "of the resume after the move");
    let still_refused = resume_refusal(&still, "the resume once the tracked path is moved out");
    assert!(
        still_refused.contains("path `paid.dat` uses clean/smudge filter `paid-marker`"),
        "the index still holds the moved path, so its filter still refuses: {still_refused}"
    );
    assert_eq!(
        pin_target(&repo, &pin),
        None,
        "no refused resume pins the output"
    );
    assert!(kept_copies(&repo).is_empty(), "or copies it");
    assert_eq!(
        fs::read(events).ok(),
        Some(log),
        "the refused resumes leave the run's event log as it was"
    );
    assert_eq!(
        fs::read(&moved).ok(),
        Some(FILTERED_PAID_BYTES.to_vec()),
        "outside the checkout, only the operator's moved file keeps the paid bytes"
    );
    fs::rename(moved, repo.join("paid.dat")).expect("the operator puts the path back");
    let kept = repo.with_file_name("attributes-kept-by-the-operator");
    fs::copy(repo.join(".gitattributes"), &kept).expect("the operator copies the attribute out");
    fs::remove_file(repo.join(".gitattributes")).expect("and removes it, as the advice says");
    let report = bounded_resume(
        "the resume once the attribute is gone",
        &repo,
        fake(Effect::EditFile),
    )
    .expect("the resume completes once the attribute is gone");
    the_marked_filter_never_ran(marker.as_deref(), "of the resume that pinned the output");
    assert!(committed(&report, "t1"), "{report:?}");
    let commit = pin_target(&repo, &pin).expect("the resume pinned the attempt's output");
    assert_eq!(
        blob_at(&repo, &commit, "paid.dat"),
        Some(FILTERED_PAID_BYTES.to_vec()),
        "the pin holds the paid bytes the worker wrote over the tracked path"
    );
    assert_eq!(
        blob_at(&repo, &commit, "agent-output.txt"),
        Some(b"paid output\n".to_vec()),
        "and the rest of the output"
    );
    assert_eq!(
        blob_at(&repo, &commit, ".gitattributes"),
        None,
        "but not the attribute file the operator removed"
    );
    assert_eq!(
        fs::read(kept).ok(),
        Some(MARKED_ATTRIBUTE.to_vec()),
        "which only the operator's own copy keeps"
    );
    let copies = kept_copies(&repo);
    let [copy] = copies.as_slice() else {
        panic!("one copy: {copies:?}");
    };
    git_in(&repo, &["update-ref", "-d", &pin]);
    restore_the_pin_from(&repo, copy, &pin);
    assert_eq!(
        pin_target(&repo, &pin),
        Some(commit),
        "the copy puts the pin back"
    );
    let advice = "remove the attribute that sets it, after copying out anything to keep";
    for (what, text) in [
        ("the kept note", warning.as_str()),
        ("the refusal", refusal.as_str()),
        ("the refusal after the move", still_refused.as_str()),
    ] {
        assert!(
            !text.contains("move that path out"),
            "{what} offers no move-out for a tracked path: {text}"
        );
        assert!(text.contains(advice), "{what} says to {advice}: {text}");
    }
}
