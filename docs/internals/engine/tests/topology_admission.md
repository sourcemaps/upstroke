# `src/engine/tests/topology_admission.rs`

Extended notes for [`src/engine/tests/topology_admission.rs`](../../../../src/engine/tests/topology_admission.rs).

## `fn a_schema_4_resume_proves_its_owner_and_commit_records_before_it_writes_its_include() {`

PR12-CI-01 (P1, concurrency/invariants lens) and
PR12-RESUME-INCLUDE-BEFORE-RECORD-VERIFICATION (P2, regression lens): one defect, one
guard (INV-15, O18: the private root is derived read-only before any lock, and the owner
and commit records are verified before any private write). A real schema-4 run parks
through `run_with`; every resume below goes through `engine::resume_with` — the public
composition, which the recovery-order tests never reach — with no `--private-root`, so
the run's committed locator is the only root authority, and with the private half
present.

The premise holds that locator to the root the run was given, spelled as the code that
records it spells it: `run_started(4).private_dir` is the configured root through
`fs::canonicalize` (`prelock::authorized_private_root`) joined with `runs/<run-id>`
(`PreLockChecked::private_dir`), so the expected path is `fs::canonicalize` of the same
`private_root_for` joined with the same two components, compared by exact `PathBuf`
equality. The configured spelling is another spelling of that directory wherever the
temporary directory is not canonical — Windows' verbatim `\\?\` prefix, macOS's `/var`
for `/private/var` — and native CI run 41936043911 failed this premise on both before
any refusal ran, when it compared the locator with that spelling
(PR12-NATIVE11-WINDOWS-PRIVATE-PATH-ORACLE). On a canonical temporary directory, as on
the Linux runners, the two spellings are the same path.

Ten damages to the run's own records — owner record missing, not JSON, or naming
another run, repository, run directory, incarnation or runner (each owner field
`RecordsVerified::verify` compares); commit record missing, not JSON, or naming
another first line — each under both include states, refuse with the record's own
refusal. The locator retargeted, in the committed first line, to another existing root
of the conforming `<root>/runs/<run-id>` shape refuses too, both when that root holds
no records and when it holds copies of the run's own, which only the commit record's
digest of the first line tells apart, again under both include states. For every one of
those twenty-four refusals the whole persistent private root — and the foreign root for the
retargeted cases — is unchanged entry by entry, byte by byte and mode by mode, the log
is unchanged, every ref is where it was, and no probe ran.

The positive control resumes the provable run with the include absent and then changed:
the include holds its fixed bytes again, its directory is `0700` on Unix, the resume
reached its probes and appended `run_resumed`. With `resume_harness` publishing the
include before `ResumePrepared::verify` (the order before PR12-CI-01), the first refusal
fails on the private root that moved.

## `fn on_this_path(program: &str) -> bool {`

Whether a file named `program` is in a directory of this process's `PATH`; the
recorded-shell guards assert that `cmd` is not, so that "unavailable" is a fact of the
host and not an assumption.

## `fn a_fake_cmd(tree: &ScratchTree) -> PathBuf {`

A directory holding a `cmd` that exits 0, written by a process of its own
(`runner::container::write_program_in_its_own_process`) so that no thread of this test
binary holds it open for writing when it is executed.

## `fn with_shells_on_path(`

Runs [`recorded_shell_helper`] in a process of its own whose `PATH` starts with the
fake shell's directory, so the shell is available to that process alone and to no other
test.

[`RecordedShellScope`] owns that process and its process group. Its stdout and stderr
go to files in a scratch tree, not to pipes, so nothing it leaves running can hold the
wait open, and its stdin is null, as `Command::output` made it. The wait is bounded at
300 seconds, the bound
`a_legacy_run_under_an_inherited_selection_checks_its_review_input_at_the_selected_repository`
gives a whole run in a child of its own. Each caller requires a successful exit status
and `1 passed`, and prints the child's status, stdout and stderr when that does not
hold, so a child killed at the bound (status `None`), one that failed and one that
matched no test all fail with their diagnostics. The `Command::output` this replaced
waited without a bound for the child and for every holder of its pipes
(PR12-R7-RECORDED-SHELL-HELPER-UNBOUNDED, with its correlated
PR12-CI2-RECORDED-SHELL-HELPER-UNBOUNDED).

## `struct RecordedShellScope {`

The recorded-shell helper's process group, owned on every way out. An anchor
(`/bin/sleep`) starts first, leading a group of its own, and the helper joins that
group. The anchor is collected only after the group has been killed, so the group id is
this test's for the whole scope — after the helper has been collected, and even if the
anchor has exited — and the kill cannot reach a process that reused it. `settle` sends
`SIGKILL` to the group and then collects the helper and the anchor within
`ISOLATED_CHILD_COLLECTION_BOUND`. It runs when the helper ends, successfully or not,
so whatever of its descendants stayed in its group dies with the anchor; at the bound;
when polling the helper fails; and from `Drop`, so a panic between the helper's start
and its settlement settles it too. A settlement that cannot collect both within its
bound fails the test or, while unwinding, says so, and leaves the rest to this process's
exit. Descendants that left the group, as the engine's commands and their reapers each
do, are the engine's to settle, as after any run. `agent::proc::test_support::run_test_isolated`,
which this fix first used, kills its child's group only at its deadline: it collects a
child that ended by itself without settling the group, and a failed poll panics with the
child unkilled and no guard to settle it.

## `fn a_zero_gate_schema_4_resume_probes_its_recorded_shell_when_todays_config_names_an_unavailable_one()`

PR12-CI-03 (INV-23), one direction of today's drift. A zero-gate run is created under
`sh` (the premises: `gate_cmds` is empty, `run_started(4).shell` is `sh`, and this host
has no `cmd`). Its resume through `resume_with`, with today's config naming
`shell = "cmd"`, passes its pre-flight, recovers (one `run_resumed`) and warns that
today's `cmd` is not the run's and that the pre-flight probes `sh`. With the resume's
shell taken from `gate_cmds` else today's config again, it fails: the pre-flight probes
`cmd` and refuses.

## `fn a_zero_gate_schema_4_resume_refuses_its_unavailable_recorded_shell_whatever_todays_config_names()`

PR12-CI-03 (INV-23), the other direction, and the unavailable recorded shell. A
zero-gate run is created under `cmd` by a child whose `PATH` holds a fake `cmd` (the
premises: no gates, `run_started(4).shell` is `cmd`, and this host has no `cmd`). Its
resume in this process, where `cmd` is not on `PATH`, with today's config naming the
available `sh`, refuses at the pre-flight naming the recorded shell `cmd`, and the log
is unchanged, so no recovery event was appended. The same resume in a child whose
`PATH` holds the fake `cmd` again continues and appends `run_resumed`: the run stayed
resumable. With the resume's shell taken from `gate_cmds` else today's config again,
the first resume probes `sh`, recovers and fails the refusal it expects.

## `fn a_schema_4_log_without_a_recorded_shell_is_refused_and_never_resumed_under_another() {`

PR12-CI-03, the record's compatibility (`run_started(4).shell` is required, as every
schema-4 transaction-payload field is). A parked run's committed first line, rewritten
without `shell`, stands for a log written before the field. Its resume through
`resume_with` refuses at the committed first line, naming `shell`, writes nothing to
the log and nothing under the private root, and never resumes under a shell the record
does not hold.

## `struct TreeSnapshot {`

What `every_path_under` saw: `entries`, every entry it listed, as a path
relative to the root, sorted; and `unread`, the entries it listed and could not
read, a directory it could not open by its listed name or an entry whose type
could not be read because it was gone. The J14 tests compare the entries and
carry the unread ones in their messages (K7).

## `fn every_path_under(root: &Path) -> TreeSnapshot {`

Every file, directory and link under `root`, by the names the platform lists
and the types its listing reports (`DirEntry::file_type`, which reads the
directory entry's own type and looks nothing up by name where the platform
reports one); a link is listed and not followed. A directory is descended by
its listed name; one that cannot be opened by it, or an entry gone before its
type was read, is noted in `unread` rather than ending the walk. A root that
cannot be read is noted the same way, as the empty path.

Until K7 each listed entry was `lstat`ed by its listed path, with `.expect`.
On CI's macOS leg (run 41724856814, job 126229500908, macOS 26 arm64, APFS, git
2.56.0) the J14 test panicked inside this walk with `an entry's own metadata:
NotFound`: an entry the listing had just named could not be found by its path.
The job log does not say which entry, or which of the test's walks: the panic
names neither. Two mechanisms fit it, and the repair holds against both:
- a name the platform lists but cannot look up by the bytes it listed (the
  coordinator's reading of a home named `private-\xff` on APFS). The walk no
  longer looks a listed entry up to learn its type, notes a directory it cannot
  open, and the J14 test also walks the home by the path it holds, so whatever
  the run might create under it, the include included, is still compared;
- an entry that existed when the directory was listed and was gone a moment
  later: git 2.47 and later detach `git maintenance run --auto` after a commit,
  and it holds `.git/objects/maintenance.lock` while it decides, so a walk
  begun just after the fixture's commit could list that lock and miss it. The
  fixture's repository now turns automatic maintenance off before its commit
  (`temp_engine_repo`), and an entry gone before its type is read is noted
  rather than fatal.

## `fn a_schema_4_run_refuses_an_unspellable_default_private_root_before_it_creates_anything() {`

J14 (regular review 3, finding 3; regression review 2, finding 5): a fresh
schema-4 run with no explicit private root and a home no string spells is
refused before anything is created, the recorded-objects include included.
`HOME` is process-wide, so the run is a child of this test binary
(`unspellable_default_root_child`, through `run_test_isolated`, whose output
files live in a scratch tree of their own), with `HOME` and `USERPROFILE` set to
`<tree>/private-\xff`. Two cells: that home exists and is empty, and it does not
exist. A host that refuses the name (`EILSEQ`) cannot hold the first, which is
then skipped; the second needs no such name on disk. Every path under the
scratch tree, the repository and the home included, is the same after the
child as before it, and there is no `.upstroke` under the home. The comparison
comes first, so a regression shows what it left. With the read-only check
removed from `prepare_fresh`, the first cell fails on exactly the reviewers'
witness: `private-\xff/.upstroke`, its `git` directory and
`recorded-objects.gitconfig` appear, and the run is refused later, by
`authorize`.

Since K7 (CI's macOS leg) each snapshot is two walks: the scratch tree, by the
names the platform lists, and the home, by the path the test holds, which is
the path the child's `HOME` names. What is compared is every entry listed; the
entries a walk listed and could not read are carried in the message, before and
after, as the record of what the platform would not show. On a host that can
name the home, the home's own walk is the one that sees inside it however the
tree's listing spells it; on one that refuses the name, the first cell is
skipped as before.

## `fn the_tree_snapshot_lists_by_the_platforms_own_names_and_types_and_notes_what_it_cannot_read() {`

K7: the snapshot lists a directory, a file in it, a link to nothing and a link
to the directory, by their names, without following either link, and notes a
root it cannot read as the empty path rather than panicking. The macOS case
itself (a listed name or entry that cannot then be found) is not reproducible
on this box; this pins the walk's own rules.

## `fn the_default_root_check_judges_the_path_the_root_will_resolve_to() {`

`prelock::refuse_unspellable_default_root` judges the path the default root
will resolve to, not its spelling: a missing root under a home text spells and
an existing root text spells pass; a missing root under a missing home no
string spells is refused, saying nothing was locked or created; a link no
string spells that resolves to the home text spells passes (where the host can
hold the link's name). The checks create nothing: the tree holds the home and
the link only.

## `const INHERITED_COMMON_IMAGE: &str = "upstroke-tests/inherited-common:1";`

The image reference J10's guard configures and records. No runtime holds it,
and none is asked: the guard's stub runtime fails every command it is given.

## `fn run_directory_names(repo: &Path) -> Vec<String> {`

The names under the repository's runs root, sorted: what J10's guard compares
before and after its children, so a refused run that left a run directory
fails it.

## `fn record_a_container_runner_mounting(repo: &Path, run_id: &str, mounts: &[(&Path, &str)]) {`

Makes the parked run's committed `run_started` record a container runner whose
mounts are `mounts`, each source writable at its target, with the canonical
spelling and the identity a resolution would record, in the order given. The
runner's JSON is replaced in the first
line, which must spell it exactly once, and every other byte of the log is
kept, so a second call replaces the first one's runner. It stands for the
record a container run would have written: the public
fresh path cannot create one here without a container runtime, and a resume
reads nothing of the run but `run_started`, its frozen plan and its private
half before it refuses. J10 records one mount at `/data`; K1 records the
targets a fresh run refuses, which no resolution would write.

## `fn a_mount_inside_an_inherited_common_directory_is_refused_by_run_and_resume_before_the_runtime_is_asked()`

J10 (regression review 2, finding 1). A parked schema-4 run, then the
repository's `.git` copied to `common`, a sibling of the checkout, as the
reviewer's layout places it. A configuration selects the container runner with
a writable mount of `common/refs`, and the parked run's record is made to name
the same mount. A stub `docker`, first on the children's `PATH`, appends what
it is asked to a file and exits 93, so no real runtime can be reached.

For `GIT_COMMON_DIR` set absolute and set relative (`../common`, from the
checkout), a child of this test binary whose environment alone carries it
(`inherited_common_dir_mount_child`, under `run_test_isolated`) runs a fresh
run and a resume and records both refusals; the parent requires the child's
success and libtest's `1 passed`, and that both recorded refusals name the
canonical common directory as the source's container. Afterwards the stub
was never asked anything, so no image was inspected, no runner was built and
no launch plan was made; the recorded log is byte-identical; and the runs
root holds what it held.

With the fix removed (`MountContext` bound from the checkout's `.git`, the
pre-J10 entry), the absolute cell fails: the fresh run accepts the mount, and
its first runtime question, the probe, reaches the stub ("the container runtime
refused `probe`"). Even then only the stub could be asked.

## `fn a_recorded_mount_whose_target_a_fresh_run_refuses_is_refused_by_the_resume_before_the_runtime_is_asked()`

K1 (regular review 4: recorded mounts bypass target validation), through the
public resume, `engine::resume_with`. A parked schema-4 run; a stub `docker`,
first on the children's `PATH`, appends what it is asked to a file and exits
93, as in J10's guard, so no real runtime can be reached. For each set of
`resolve::fixtures::REFUSED_RECORDED_TARGETS` (the container's root,
`/upstroke/settings`, `/cache/../upstroke/settings`, and `/cache` with
`/cache/inner`), the parked run's `run_started` is made to record a
container runner mounting a real directory at those targets
(`record_a_container_runner_mounting`), a complete record that a resume
reads back from the log. A child of this test binary
(`recorded_target_resume_child`, under `run_test_isolated`) resumes it and
records the refusal; the parent requires the child's success and libtest's
`1 passed`, a refusal that names the run, the mount, the target rule's words
and that nothing was published, locked or written, and a log byte-identical
to the one the child was given. Afterwards the stub was never asked
anything, so the image's environment was not read, no runner was built and
nothing was recovered; and the runs root holds what it held.

The public resume asks the runtime before recovery does: `prepare_resume`
reads the recorded image's environment. With
`resolve::refuse_recorded_targets` bypassed, the child's resume reaches
the stub ("the container runtime refused `inspect-image-by-id`") and the
test fails on the root cell (measured before the commit). Recovery's own
check, which a resume reaches only after that question, is the subject of
`engine::topology::recover::tests`' K1 guard.

## `fn assert_refused_under_a_held_lease<H>(`

The held-lease controls of PR12-R9-PUBLIC-PUBLICATION-BEFORE-WORKTREE-LOCK,
PR12-REG7-PRELOCK-WRITES and PR12-N4-PRELOCK-PUBLICATION (one defect), shared by the fresh and
resume guards below. For each of the four exclusion and include states (absent or changed, each)
it puts the states, takes the hold, snapshots the checkout outside `.git`, the private root and
every ref, runs the public command, snapshots again, and only then releases the hold: so whatever
taking the hold itself creates (a lease file, a reaper's open of `cleanup.lock`) is in both
snapshots and nothing is attributed to the command that it did not do. Each run must be refused
with the holder's own words, build no worker or reviewer, and leave every path, kind, mode and
byte in both roots, and every ref, as it found them.

## `fn a_surviving_reaper_holding(`

A surviving reaper's shared hold on one run's cleanup lease (R28), by the suite's own holder,
`rundir::tests::cleanup_hold_child`, through `workspace_manager::fixture::spawn_ready_helper`: a
child of this test, bounded by its 30-second `await_line` and by its own sleep, and killed and
reaped when the returned `Producer` drops. The premise — that the hold is observed — is asserted
before it is returned. Unix only, as the helper and the hold are.

## `fn a_fresh_schema_4_run_that_loses_the_worktree_lease_publishes_no_exclusion_and_no_include() {`

The fresh half of the one defect, through `run_with` with the topology active. A pristine
repository (the premise: no `.upstroke`) and its explicit private root; the run is refused under
the lease this test takes in-process (the conductor's case, "is already driving worktree") in all
four states, with nothing published in either root and no ref moved. Released, the same run is
admitted and completes, and makes the exclusion and the include from absent. On Unix, beside a
surviving reaper of that first run (R28's case, "still has a process of its own alive"), the four
refusals again publish nothing; released, a second run is admitted and makes both again.

Measured before commit as negative control A1 (the old ordering restored in `run_harness`, the
exclusion, the dirty-tree refusal and the include before `FreshPrepared::lease`): it fails at the
first case on "created .upstroke", "created .upstroke/.gitignore".

## `fn a_schema_4_resume_that_loses_the_worktree_lease_restores_no_exclusion_and_writes_no_include() {`

The resume half of the one defect, through `resume_with` with no `--private-root`, so the committed
locator is the only root authority. A run parks on its one question; its log's bytes are kept.
Under the lease this test holds, and on Unix beside a surviving reaper of the parked run, each of
the four states' resume is refused with nothing written in either root, no ref moved and no worker
or reviewer built; the log is the parked run's, byte for byte. Released, the resume is admitted,
appends `run_resumed`, and makes the exclusion and the include from absent. The owner and commit
proof before the include stays `a_schema_4_resume_proves_its_owner_and_commit_records_before_it_writes_its_include`'s.

Measured before commit as negative control A1 (`resume_harness` making the exclusion before
`ResumePrepared::verify`, byte for byte the file at `eceed606`): it fails at the first case on
"created .upstroke/.gitignore".

## `fn a_fresh_schema_4_run_over_the_default_root_that_loses_the_worktree_lease_creates_nothing_under_home()`

The same defect over the default private root (Unix, as `run_test_isolated` is). The default root
is `$HOME/.upstroke`, which no test may touch in the operator's home, so the parent makes a
directory of its own fixture tree the child's `HOME` and `USERPROFILE`, as
`a_schema_4_run_refuses_an_unspellable_default_private_root_before_it_creates_anything` does, runs
`default_root_lease_child` under `run_test_isolated` with a 300-second bound, and requires its
success and `1 passed`. The default root is the one branch whose root the include's publication
creates, and whose authorization (`RunnerResolved::authorize`) therefore follows the lease; this
is its witness that losing the lease creates no root, and that winning it creates and authorizes
one.

## `fn a_schema_4_resume_whose_router_cannot_read_its_log_refuses_before_publishing_anything() {`

PR12-INV5-RESUME-READ-ERROR-LEGACY-PUBLICATION and PR12-REG8-RESUME-READ-ERROR-LEGACY-PUBLICATION
(one defect), through `resume_with` with the explicit private root the run recorded. A run parks
on its one question, and the premises are read first: the census classifies its committed header,
its id resolves, and read normally its header routes the resume to the topology. Then, for each
of the four exclusion and include states (absent or changed, each), under the worktree lease this
test holds and with the router's initial read of the run's log failing, the resume is refused with
that read's own error after exactly one attempt at the read, and nothing was published: every
path, kind, mode and byte of the checkout outside `.git` and of the private root, and every ref,
as they were, and no worker or reviewer built. The no-publication comparison is asserted before
the refusal's identity, so it is the assertion the old fold fails.

The controls follow. With the run's owner record naming another run, and then with its commit
record digesting another first line, the unread resume is refused with the read's error and writes
nothing, and the same resume read normally is refused by the record (`records run id`, `says the
committed first line digests`) and writes nothing either: the read's refusal comes first, and the
proof still stands behind it. Read normally under the held lease, the resume is refused as already
driven and publishes nothing, and the log is the parked run's, byte for byte. Released, the resume
is admitted, appends `run_resumed`, and makes the exclusion and the include from absent
(`assert_admitted_and_published`). A genuine legacy run (`parked_run`) routes to the legacy lines
when read; with its log unreadable to the router it is refused with the read's error and nothing is
published under its root either; read again, `resume_answering` takes it to completion and the
legacy lines publish its include as before.

Measured before commit as negative control B1 (`route_resume` folding the failed read into
`Ok(None)` again, as at `8b7b22d4`, over the same read): it fails at the first case, exclusion and
include absent, on "created git", "created git/recorded-objects.gitconfig" under the private root.

## `fn a_schema_4_resume_over_the_default_root_whose_router_cannot_read_its_log_creates_nothing_under_home()`

The same defect with no `--private-root` (Unix, as `run_test_isolated` is). The legacy lines'
include goes under today's default root, `$HOME/.upstroke`, which no test may touch in the
operator's home; so, as
`a_fresh_schema_4_run_over_the_default_root_that_loses_the_worktree_lease_creates_nothing_under_home`
does, the parent parks a run under its explicit fixture root, makes a directory of its own fixture
tree the child's `HOME` and `USERPROFILE`, runs `default_root_unread_resume_child` under
`run_test_isolated` with a 300-second bound, and requires its success, `1 passed` and
`DEFAULT_ROOT_UNREAD_DONE`.

## `const RETAINED_HUSK_PRODUCER: &str = "engine::topology::create::tests::create_kill_child";`

The producer of the retained schema-4 husk the guards below resume: `create`'s own kill child,
whose `p5b` site is the one `kill_between_commit_record_and_run_started_retained_as_possibly_committed`
measures.

## `fn retained_p5b_husk(root: &Path) -> String {`

The retained husk P5b leaves, made by the actual producer rather than written by hand:
`create_run`, driven by `create`'s tests' driver, killed after `RunDir.PublishCommitRecord` and
before its first append, in the child [`RETAINED_HUSK_PRODUCER`] names, under the production
`HostRunner` with a 120-second bound (`create`'s own `spawn_and_wait` shape). The child lays out
`create`'s fixture under `root`: `root/repo`, which here is the Git checkout `temp_engine_repo`
already made (`create_private_dir` is `create_dir_all`), so the legacy lines' prelude would find a
repository there, and `root/private`, the husk's creation private root. It then asserts the shape
the producer's own test asserts: one run directory, `committed.json` in the private half, an empty
log, the census's `Husk`, and the disposition `Retained(PossiblyCommitted)` under that fixture's
own repository key. Returns the husk's id. It produces the P5b shape only. A failed first append
that `create`'s append-error protocol answers `RetainedPossiblyCommittedHusk` leaves, as that
protocol reads, the same two halves (its torn line truncated by `Event.OpenLog`); that producer is
not run here.

## `fn a_retained_schema_4_husk_is_refused_before_any_root_is_published() {`

PR12-REG9-HUSK-REFUSAL-LEGACY-PUBLICATION through `resume_with` naming an explicit root
(`private_root_for`, the fixture's `home`) that is not the husk's creation root. The positive
control runs first: a genuine legacy run (`parked_run`) still routes to the legacy lines and, with
its include absent, `resume_answering` takes it to completion and the legacy lines publish the
include as before. Then the husk is produced, the premises are read (the two roots differ; the
resolver refuses the exact id with its husk sentence naming the run), and for each of the eight
cases — the worktree lease free and held by this test, each exclusion and include state — the
resume changes nothing under the root it names, under the creation root, in the checkout outside
`.git` or in `.git` (every path, kind, mode and byte), builds nothing, and is refused with the
resolver's own refusal, byte for byte. The no-write comparisons precede the refusal's identity, so
they are what the old fold fails.

**The held lease is held, physically, throughout.** PR12-R12-WINDOWS-HUSK-SNAPSHOT-LOCK,
PR12-REG10-HUSK-SNAPSHOT-LOCK and PR12-INV7-HELD-LEASE-SNAPSHOT-WINDOWS (one defect, first in
`fbdd3e69`): the guard took the lease and then snapshotted `.git`, which reads
`.git/upstroke-worktree.lock` through a handle of its own. On Windows that read meets the
holder's `LockFileEx` range lock — Native16's `test (winguest)` failed in the snapshot's `fs::read`
with `os error 33`, though the log names no file, case or holder — and on Unix closing that
second descriptor released this process's `F_SETLK` lock (`rundir`'s lock notes: a holder must
never open its own lock file again), so the resume ran against a lease the operating system no
longer held, with only the in-process claim left. Now `.git` is snapshotted only outside the held
interval — before the lease is taken and after it drops — so nothing in this process opens the
lease file while it holds it; the lease file is created once before the loop and asserted present
(R25: created on first acquisition, never removed), so no acquisition below creates anything in
`.git`; and in every case [`assert_the_lease_is`] asks another process: before the resume (held:
refused, naming this process; free: taken and given back) and, held, again after the resume and
every observation made under the lease. Nothing is skipped on any platform, no production lock
changes, and every assertion the guard made it still makes, failures naming their case and path.

Measured before commit as negative control A13 (the router's three lines folding the resolver's
refusal into `Ok(None)` again, exactly as at `328ee6dd`): the legacy control passed, and the first
case failed on "created git", "created git/recorded-objects.gitconfig" under the named root. A
compiled and executed failure on the unwanted write, not a compile error or a timeout.

## `fn a_retained_schema_4_husk_resumed_over_the_default_root_creates_nothing_under_home() {`

The same defect with no `--private-root` (Unix, as `run_test_isolated` is): the case the finding
names, where today's default root is not the husk's creation root. The parent produces the husk,
makes a directory of its own fixture tree the child's `HOME` and `USERPROFILE`, runs
`default_root_husk_resume_child` under `run_test_isolated` with a 300-second bound, and requires its
success, `1 passed` and [`DEFAULT_ROOT_HUSK_DONE`]. No variable of this process changes.

## `fn retain_execution_root(repo: &Path, run_id: &str) {`

Leaves a finished run's execution root as a run keeps it when it ends before finalization's last
step: the scaffolding `WorkspaceManager::create_execution_root` makes, through [`manager_today`],
after asserting that this manager derives the root the run recorded.

## `fn copy_common_dir(from: &Path, to: &Path) {`

A second common directory this test owns: the checkout's Git directory copied file by file
(objects, refs, packed refs, logs and configuration) without its linked-worktree registry and
without the worktree lease's file. With `GIT_COMMON_DIR` naming it, Git keeps the checkout's own
`--absolute-git-dir` and answers this directory for `--git-common-dir`, which is the split the
finding needs, and nothing outside the fixture is copied or touched. A plain-Git fact the test
relies on, not a reproduction of anything by Git alone.

## `fn a_schema_4_resume_under_another_effective_common_directory_refuses_before_any_effect() {`

PR12-INV6-RESUME-COMMON-DIR-REBIND through `resume_with` on the host runner (Unix, as
`run_test_isolated` is; `GIT_COMMON_DIR` changes only in the children's environments, never in this
process's). In one checkout with no `GIT_COMMON_DIR`, four runs park on their one question (open,
resumable prefixes) and one completes, its pruned root then retained ([`retain_execution_root`]),
a valid terminal prefix with execution-root residue. Their owner, commit and log bytes are the
engine's own. The positive controls run first: a child with `GIT_COMMON_DIR` naming the checkout's
own common directory absolute, then one naming it relative (`.git`, given its meaning at the
checkout), then this process with none, each admitting its own open run and finalizing the finished
run at the root it recorded ([`assert_admitted_at_the_recorded_roots`]; the residue retained again
after each). Then [`copy_common_dir`] makes the other common directory, and the refusing child runs
with `GIT_COMMON_DIR` naming it. Each child has a 300-second bound and a completion sentinel.

Measured before commit as negative control B13 (`ResumePrepared::verify` discarding the binding's
answer with `let _ =`, so today's manager is adopted as at `328ee6dd`): the three positive controls
passed, and the refusing child failed at its first case on the wrong-root admission's writes — the
exclusion and the include published, the open run's log and report changed, and a namespace
`workspaces/<key>` of the other common directory created under the private root.

## `fn execution_roots_under(private_root: &Path) -> Vec<String> {`

The run ids of every execution root under a private root's `workspaces/`, by name, so a root
spelled through a canonicalized private root compares equal.

## `fn a_fresh_run_on_an_unborn_branch_is_refused_before_it_creates_anything_and_a_born_one_runs() {`

PR12-INV7-PREPUBLICATION-EXECUTION-ROOT-ORPHAN through the public entry, on the route the finding
names: a clean repository on an unborn branch, its plan outside the worktree. The run is refused by
`git rev-parse HEAD` — the base commit it cannot record — and leaves no execution root of its own,
no run directory, no private half and no build, with the lease free; another run's root in the same
namespace, content included, is untouched. Before the repair the root was made before that read,
and stayed with no record naming it. Then the branch is born, and a fresh run on it completes and
records the root it created; the other run's root is still untouched.

## `fn probe_an_absent_cli(`

A probe, for `adapter`, that asks the run's own runner to start `program`, which is not on `PATH`,
and refuses the way a real adapter does when its CLI is absent (`agent::bin::boundary_refused`).

## `fn a_configured_reviewer_whose_cli_is_absent_fails_creation_before_p5b_and_commits_nothing() {`

P12-W17 (D9). A configured reviewer whose CLI is absent fails creation before P5b, and nothing is
committed. Every recorded reviewer is probed at P4, so `run_with` refuses there, naming the
reviewer's program, after P3c claimed the run's execution root. The creator's cleanup leaves no run
directory, no private half holding `committed.json`, no execution root and no integration ref, and
no run was made without the reviewer. The control: the same configuration, with the reviewer's CLI
present, creates the run, records both agents as probed, and completes it.

This prose sat above the test in the source until the execution root moved into the create protocol
at P3c; the refusal used to be reported after P3b, and the root was made before creation began.

## `fn assert_a_refused_creation_left_nothing(repo: &Path, said: &str) {`

What a creation refused at P3c leaves in `repo`, whose refusal said `said`: no execution root, no
run directory, no private half carrying a commit record, and no integration ref.

The walk of the private `runs` directory reads every directory and entry or panics naming the path:
one it cannot read is not certified as holding no commit record. Only the `runs` directory itself
being absent counts as empty, since then there is nothing under it to hold one.

## `fn an_absent_alternative_reviewer_cli_refusal_check_fails_on_a_runs_it_cannot_list() {`

The refusal check the absent-alternative tests lean on does not certify a private `runs` it cannot
list: here a file stands where the directory would, so listing it fails with an error other than
its absence, and the check fails rather than reading it as holding no commit record.

## `fn profiled_pool_file(tree: &ScratchTree, profile: &Path) -> PathBuf {`

[`stub_pool_file`]'s pool, `claude-max` for the topology stub's agent (`claude-code`), naming
`profile` as its credential profile. The file sits in the scratch tree, outside the checkout, under
a name of its own, so the run's tree stays clean and a test can switch between this file and
[`stub_pool_file`]'s. The `toml` crate encodes the path, as a `toml::Value::String`, so a
backslash, a quotation mark or an apostrophe in it is read back as written.

## `fn the_profiled_pool_fixture_writes_a_profile_toml_reads_back() {`

[`profiled_pool_file`] writes a profile whose path holds an apostrophe, quotation marks and a
backslash (`/tmp/O'Brien/a "b"/c\d`), and the file it writes, parsed by the `toml` crate, gives
`pools.claude-max.profile` back as that text.

Measured before the repair, when the fixture wrote the path between apostrophes as a TOML literal
string: the file did not parse. The `toml` crate stopped at line 4, column 19, the apostrophe in
`O'Brien` that closed the literal early, expecting a newline or `#`.

## `const FIXTURE_OAUTH_TOKEN: &str = "sk-ant-oat01-engine-fixture-token-value";`

A recognisable token value, so a refusal can be asserted not to carry it.

## `fn an_oauth_token_is_set(key: &str) -> Option<std::ffi::OsString> {`

The coordinator environments a fresh run's profile check reads, through `RunOptions`'s
test-only `coordinator_environment`, in place of the process environment, which every test in this
binary shares. This one has `CLAUDE_CODE_OAUTH_TOKEN` and nothing else. `no_oauth_token` below has
nothing at all.

## `struct CountedStub {`

The topology stub, counting the probes and builds a run asks of it. It is its own adapter source,
which cannot be shared, so the run steps at width 1. A probe is the first process a fresh run spawns
for an agent (P4), so `(0, 0)` means the run spawned nothing for its agents and built no worker or
reviewer.

## `fn assert_a_profile_refusal_created_nothing(`

What every fresh-run profile refusal shares, asserted once:
- it is a `Refused` in the pre-flight's own shape (`pre-flight: `);
- it ends in the one closing sentence every profile refusal takes, on this path and a resume's
  alike: no agent CLI was started and no event was appended, so correct the pools file or the
  environment and run the same command again;
- it is not a creation's refusal (`stopped at`), so `create_run` never began;
- the stub was neither probed nor built;
- [`assert_a_refused_creation_left_nothing`] holds;
- the private `runs` directory holds no entry at all, so there is no private half, committed or not;
- `<repo>/.upstroke` does not exist, so no run exclusion was made;
- the private root is still a directory and an empty one, so no include was published under it;
- the worktree Git directory holds no `upstroke-worktree.lock` ([`worktree_lock_path`]), so no
  lease was taken.

The last three are what make "before the lease" observable: a fresh run refuses a profile in
`entry`'s `prepare_fresh`, before the lease, with nothing published, locked or created (the Git
children of its checks ran, read-only). The fixture's checkout has no `.upstroke`, its private root
is created empty ([`topology_options`]) and no earlier command took its lease, so each absence is
this run's. A resume refuses the same profile first in `certify`, under its locks and after its
exclusion and include, before any probe or recovery event, which is why the closing claims no more
than the two paths share.

Measured before the move, with these three assertions added and the call still in
`FreshLeased::run` after the lease, the exclusion and the include (and the closing still the old
one): all three tests that use this helper failed here and no other test failed. Each panicked at
the first of the three, naming its checkout's `.upstroke` as existing, so the private root and lease
assertions were not reached on that source; the refusal message each carried was the one its own
test pins.

Returns the message for the test's own pins.

## `fn host_policy() -> crate::topology::events::RunnerPolicy {`

The host runner's policy, which is the one a fresh run with no `[runner]` section records. The
pre-flight's refusal names only its boundary (`this host`), so it stands in for the policy a
resume's pre-flight would be handed for such a run.

## `fn a_fresh_schema_4_run_whose_pool_names_a_missing_profile_is_refused_before_anything_is_created() {`

A fresh schema-4 run whose one pool names a profile directory that does not exist is refused
through `run_with` before anything of the run is created
([`assert_a_profile_refusal_created_nothing`]). The refusal names the pool and the path. The run's
environment has no token, so the directory is the only thing wrong. Then a resume's pre-flight, a
`RunPreflight` over the same pool certifying the host policy, refuses the same profile, and the two
messages are equal: one refusal, worded once, on both paths. The resume's refusal comes before its
token check, so the process environment it reads by default is never consulted.

Measured before the repair, on the source with only this test, its partner and their test-only
`coordinator_environment` field added: the `expect_err` panicked. The run had been created and had
completed, `t1` and `t2` committed with four attempts on `claude-max`, all under a profile directory
that did not exist.

Measured again before the refusal moved into `prepare_fresh`, with the helper's three absence
assertions added: the test panicked in the helper, "no run exclusion and no public namespace:
…/repo/.upstroke exists", after a refusal that named the pool and the missing directory. The run
had been refused after its lease, its exclusion and its include.

## `fn a_fresh_schema_4_run_whose_pool_names_a_relative_profile_is_refused_before_anything_is_created()`

A fresh schema-4 run whose one pool's profile is `.` is refused through `run_with` before anything of
the run is created ([`assert_a_profile_refusal_created_nothing`]). `.` exists and is a directory
from the coordinator's working directory, and the run's environment has no token, so its being
relative is the only thing wrong. The refusal names the pool and the value, and its text does not
contain "resumable": the refusal comes before `create_run`, so there is no run to resume
(`PR386-FRESH-REFUSAL-NOT-RESUMABLE`). Then a resume's pre-flight over the same pool refuses in
the same words, as for the missing profile above.

`PR386-RELATIVE-PROFILE-CWD`: before the repair the pre-flight read a relative profile from the
coordinator's working directory, while the host runner starts each agent process in its own task
directory, so `.` passed the check and named a different directory for every process.

Before the refusal moved into `prepare_fresh`, this test failed in the helper's first absence
assertion as its partner did: `…/repo/.upstroke` existed beside the relative-profile refusal.

## `fn a_fresh_schema_4_run_whose_profile_an_oauth_token_would_override_is_refused_before_anything_is_created()`

A fresh schema-4 run whose pool names an existing profile directory, with
`CLAUDE_CODE_OAUTH_TOKEN` set in the environment the check reads ([`an_oauth_token_is_set`]), is
refused before anything of the run is created. The refusal names the variable, the pool and the
profile, and never the token's value. Two controls follow in the same checkout:
- the same configuration with no token creates the run and completes it, so the token was the only
  thing wrong;
- a pool that names no profile ([`stub_pool_file`]), with the token set again, is never checked,
  and that run completes too. This is an unprofiled fresh run's behaviour, unchanged.

Measured before the repair, as its partner was: the `expect_err` panicked. The run had been created
and had completed although the environment it was given held the token: the fresh path read no
environment and made none of the profile checks.

Before the refusal moved into `prepare_fresh`, this test failed in the helper's first absence
assertion too, before its controls ran: `…/repo/.upstroke` existed beside the token refusal. The
controls create runs, so they follow the helper; the helper's absences are those of the refused run
alone.

## `const ABSENT_ALTERNATIVE_CLI: &str = "upstroke-absent-codex-alternative-cli";`

The program the alternative reviewer's adapter, `codex`, asks the runner for. Nothing installs it.

## `struct AbsentAlternativeCli {`

A `codex` adapter whose CLI is not installed, as [`AbsentReviewerCli`] is a `copilot` one, counting
what the run asks of it: every probe and every build.

## `const FRONTIER_IMPLEMENTS_CONFIG: &str =`

Implementation held to the frontier rung, which the topology stub serves as Claude Code's frontier
model, one attempt per task. The review tier is the frontier too, so the primary reviewer is the
model that implemented each task, and with `cross_family` unset the run names another family's
reviewer to take its place: on a harness that drives `codex`, Codex's.

## `const FRONTIER_IMPLEMENTS_CROSS_FAMILY_OFF_CONFIG: &str = "[routing]\`

The same, with the switch off: no other family's reviewer is named.

## `fn an_absent_alternative_reviewer_cli_refuses_creation_while_cross_family_is_unset() {`

CAP-CROSS-FAMILY-REVIEWER-CANNOT-BE-DROPPED at run creation, the half that refuses; its partner is
the next test. The tasks are implemented on the frontier rung, which the topology stub serves as
Claude Code's frontier model, so the primary reviewer, `claude-code/claude-opus-5`, is the model
that wrote the work, and with `cross_family` unset the run names another family's reviewer to take
its place. The harness drives `codex` and not `copilot`, so that reviewer is Codex's, and its CLI is
absent. Run creation probes every agent in the probe set it derives from `review_plan.agents()`, so
`run_with` refuses at P3c, before the commit record, naming the alternative's program as one that
could not be executed. The `codex` adapter was probed once and never built, and the creation left
nothing behind, by the same assertions as the configured-reviewer test above: no execution root,
no run directory, no private half holding `committed.json`, no integration ref.

## `fn an_absent_alternative_reviewer_cli_is_never_probed_once_cross_family_is_false() {`

The partner: the same plan, stub and absent Codex CLI, with `[routing] review = { cross_family =
false }`. The run is created and completes with both tasks committed. Its first logged line records
`claude-code` alone as probed, the `codex` adapter was neither probed nor built, and the primary
reviewed each task its own model implemented. The run's warnings hold exactly one that mentions
`cross_family = false`, and it names the primary.

The pair was measured on 2026-11-14 with the gate `cargo test --all-features --lib
engine::tests::an_absent_alternative_reviewer_cli`, against two changes to the probe set in
`engine::topology::entry::prepare_fresh`. Each change was applied alone and reverted before the
next, and on the unchanged source both tests pass. With `codex` pushed onto the probe set whenever
the harness has an adapter of that id, whatever `cross_family` says, this test fails. Its `expect`
panics with "with cross_family = false, the alternative's absent CLI does not refuse creation",
and the refusal it carries reads "creating run `…` stopped at P3c: agent error:
`upstroke-absent-codex-alternative-cli` could not be executed by the runner this run uses". Its
partner, `an_absent_alternative_reviewer_cli_refuses_creation_while_cross_family_is_unset`, still
passes. With the probe set taken from `review_plan.required_agents()`, which leaves the
alternative out, the partner fails because creation no longer refuses. Its `expect_err` panics
with "with cross_family unset, the alternative's absent CLI refuses creation" and carries the
report of a run that went ahead: `t1` parked after four attempts, each failing with "reviewer
unavailable: review command setup failed: agent error: an absent CLI is never built", and `t2`
stayed queued. This test still passes.
