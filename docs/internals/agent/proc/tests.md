# `src/agent/proc/tests.rs`

Extended notes for [`src/agent/proc/tests.rs`](../../../../src/agent/proc/tests.rs).

[Source on GitHub](https://github.com/sourcemaps/upstroke/blob/master/src/agent/proc/tests.rs).

The code defines current behavior. These notes preserve contracts and implementation
history. Search each backticked heading fragment separately in the source.

## `#![allow(`

Allowlist placement: the **funnel section** of `effects/allowlist.toml`, by
attachment to `src/agent/proc.rs` -- the shape `src/runner/host/tests.rs` and
`src/rundir/tests.rs` established for a funnel's own test module. This suite
re-execs this test binary as a helper, builds and tears down scratch trees,
and calls `libc` directly to observe process groups, signals and reaping, so
it names those primitives itself.

`PR6-LANEF-004`: a Rust lint level is scoped by the MODULE TREE and not by
the file, so without an attribute here the Process funnel's inner allow would
reach this file silently and no reviewed record would name the file doing the
work. THE ALLOWANCE IS NOT WIDER THAN WHAT MOVED: every one of these calls was
already made by these same bodies inside `src/agent/proc.rs`, under that
file's own inner allow of the same three lints. What moved is where the level
is stated, not what it permits.
`decisions.effect_site_inventory.mechanism` (2).

## `use crate::topology::effects::Injection;`

Reached through `use super::*;` while `apply` lived in the parent; the split
moved the injection surface to `proc/hooks.rs`, so the observers below name
it themselves (`COMMON.md` rule 7e).

## `use super::ambient::join_ambient_job_with;`

`join_ambient_job_with` moved to `proc/ambient.rs` with the rest of the
ambient API; it is a private item of `proc` there as it was here, and only
the Windows arm of this suite exercises it.

## `fn a_memoised_establishment_failure_reaches_every_later_caller() {`

A memoised establishment failure is reported to **every** later caller.

`crash_reconstruction`: "if the ambient job cannot be created or joined
the write command refuses at startup with a diagnostic before any
workspace effect (**no degraded mode**; deferred)". The memo makes the
first caller's answer every caller's answer, so an arm that turned a
remembered failure back into success is a degraded mode that no later
call can escape (`PR5-CORRECTNESS-010`).

Runs on every platform, deliberately. The value is Windows-only; the
decision about it is not, and before this the only machine that could
have executed the failing arm was one where the arm was unreachable —
a process that memoised a failure never got a coordinator to observe it
with.

## `fn a_memoised_establishment_failure_reaches_every_later_caller` › `assert_eq!(memoised_outcome::<()>(&Ok(())), Ok(()));`

The success arm, so this is not a test that only ever says "Err".

## `fn a_memoised_establishment_failure_reaches_every_later_caller` › `for message in [`

The failure arm, and the diagnostic is the memo's own: the caller
renders it into the operator-facing refusal, so a fresh or empty
message would name something that did not happen.

## `fn a_memoised_establishment_failure_reaches_every_later_caller` › `let memo: Result<(), String> = Err("it could not be created".to_owned());`

And it is stable: the *second* caller gets the same answer as the
first, which is the whole of what a memo promises.

## `fn shell(script: &str) -> Command {`

Windows-first-class: exercise the supervisor through cmd.exe, which is
always present there; use sh on everything else.

## `fn excessive_output_helper() {`

Writes `UPSTROKE_EXCESSIVE_OUTPUT_HELPER` bytes to stdout, then exits.

**Bounded, and the bound is the point.** This used to be `loop { write }`,
which is harmless while the funnel bounds capture: the parent stops
reading at the allowance, the child blocks on a full pipe, and the tree
is killed long before any budget matters. But the test that exists to
catch an *unbounded* allowance —
[`crate::runner::host::tests::the_runner_bounds_output_at_the_same_allowance_the_direct_funnel_does`]
— then had no failure mode except memory exhaustion. Measured under
`PR4-CORRECTNESS-004`'s own mutation (`OUTPUT_LIMIT_BYTES` ->
`usize::MAX`): the parent captured until the OOM killer took the whole
test binary, so the witness arrived as `signal: 9` attributed to an
unrelated test, with 900-odd tests never run and no `test result:` line
at all. A witness that destroys the evidence it is producing is not a
witness.

A finite budget several times the real allowance keeps both readings.
A funnel that bounds correctly still kills a child blocked on a full
pipe well before the budget is written, so nothing about the passing
case changes; a funnel that does not bound captures a large but
survivable amount, the child exits 0, and the assertion that fails is
`output_limited`, by name.

## `fn excessive_output_helper()` › `let on_stderr = std::env::var_os("UPSTROKE_EXCESSIVE_OUTPUT_STREAM")`

Which stream, because the allowance is **per stream** and every
fixture used to fill only one of them: a check that never looked at
stderr was indistinguishable from this one.

## `fn excessive_output_helper()` › `thread::sleep(Duration::from_secs(15));`

Written the budget, and still alive.

The budget alone is not enough: 64 MiB crosses a pipe in well under
a second, so a child that exited here would often be *gone* before
the supervisor acted on the allowance, and the funnel would report
`code: Some(0)` with the limit observed during the final drain —
a real behaviour, but not the one the two callers assert. Staying
alive keeps "an output-limited tree is terminated, not exited" true
for a funnel that bounds, while a funnel that does *not* bound still
reaches this line with a bounded amount captured and then exits, so
its witness is an assertion rather than an OOM.

## `const EXCESSIVE_OUTPUT_BUDGET: usize = 64 * 1024 * 1024;`

What this module's output-limit test gives the helper: comfortably more
than the allowance under test, and small enough to hold in memory if
the allowance stops working.

`runner::host`'s test declares its own, deliberately: a budget below
the allowance it is testing makes that test's own `output_limited`
assertion fail, so each budget is checked by the test that sets it and
there is nothing for a shared constant to keep in step.

## `fn the_output_allowance_bounds_stderr_as_well_as_stdout() {`

The allowance is **per stream**, and stderr is a stream.

Every output-limit fixture in this suite filled stdout, so a check that
never looked at stderr behaved exactly like this one: an agent that
writes its diagnostics to stderr — which is where a CLI writes them —
could fill memory without ever tripping the bound.
`invariants_preserved[0]` is "output capture … unchanged", and the
bounded half of that is what this asks about.

## `fn the_output_allowance_bounds_stderr_as_well_as_stdout()` › `let small = run_with_timeout_and_limit(`

The negative control first: a small writer on the same stream is not
limited, so `output_limited` below is the size and not the stream.

## `fn the_output_allowance_bounds_stderr_as_well_as_stdout()` › `assert!(`

`output_limited` alone is **not** the property, and measuring it
alone let the first version of this test pass under the mutation it
exists for: the final drain sets that flag from `stderr_limited`
whatever the supervisor did, so a limit check that never looked at
stderr still reported the overrun — after letting the child run to
completion. The property is that the tree is *terminated* at the
allowance, which is an exit code that is not the child's and a
return that does not wait for it.

## `fn stdin_hex_helper() {`

Stdin is **bytes**, and arrives byte for byte.

`CommandSpec { … stdin: Vec<u8> }` (DESIGN.md:222) is a byte field, and
every stdin fixture in this suite is valid UTF-8 text — so a lossy
conversion on the way to the child changes nothing any of them can see,
while an agent handed binary input on stdin would silently receive
`U+FFFD` where its bytes used to be.

The child reports what it received in hex, so the comparison is against
the bytes this test wrote and not against a string round trip.

## `fn stdin_reaches_the_child_byte_for_byte()` › `let payload: Vec<u8> = vec![0x00, 0x80, 0xff, 0x0a, 0x41];`

Not valid UTF-8: a lone 0x80 continuation, a 0xff that no encoding
produces, and a NUL — every one of which `from_utf8_lossy` replaces.

## `fn timeout_transcript_helper() {`

A timed-out attempt keeps the transcript it produced.

§14 makes the partial transcript the retry's feedback, and
`invariants_preserved[0]` keeps "output capture … unchanged". The one
timing-out fixture in this suite is `sleep 30`, which writes nothing
before it is killed — so discarding the whole transcript on timeout was
a no-op on every fixture that reaches the branch.

## `fn terminate_fault_helper() {`

The child of the `Process.Terminate` fault witnesses: publishes its pid (and, on Windows, its
creation time, which `ambient::process_alive` needs to tell a reused pid from this process) and
then outlives the three-second timeout the witness supervises it under, so the funnel's
termination is what ends it.

## `struct TerminateFaultAt {`

The production adapter (`runner::HarnessHooks`) with an error return armed at one phase of
`Process.Terminate`: the harness records the phase first, as the funnel's own call does, so the
observation export names this test as an execution of the coordinate, and the armed phase then
answers `Injection::Error`.

## `fn a_fault_at_the_terminate_funnel_settles_the_child_and_reports_its_fate(`

G5's clause 2 found both phases of `Process.Terminate` observed under the production adapter and
faulted by no committed test. This drives each: a supervised child outlives its timeout, the
termination funnel returns the injected error at the phase, and what the fault leaves is read
against the residue authority rather than restated. The process does not outlive the faulted
termination on either phase (the funnel settles it on the error path as it does on the ordinary
one). The fate the failure carries — which the runner hands its caller, and on which the caller
decides whether the invocation's process may still run (`engine::topology::run`'s verification
settles an outage only on `Gone` or `NeverStarted`) — is the authority's
rows: before the primitive, R22 still accounts for the handle, so the fate is `Unresolved`; after
it, the row holds nothing, so the fate is `Gone`. The tabled action is then the next command
through the same adapter, which runs to its own exit.

The scratch directory the helper publishes its identity into is a `rundir::scratch_tree` guard, so it
is reclaimed when the witness unwinds as well as when it returns. A reclaim that fails on the return
fails the test naming the directory; one that fails while the test is already unwinding is reported
on stderr beside the failure that unwound it, never as a second panic (#292's review round 5,
finding 3: the directory was removed by a last statement whose error was discarded, and a failing
assertion skipped it).

## `struct SpawnAfterThenTerminateAfterFault {`

The production adapter with an error return at the after phase of both process sites, and every
`child_created` callback recorded with the child's identity (on Windows its creation time, which
`ambient::process_alive` needs).

## `fn a_spawn_fault_whose_cleanup_termination_faults_after_its_primitive_reports_the_child_gone() {`

`kill_tree` stores `Gone` as soon as its primitive completes, before its after phase is consulted
(#292's fate fix). The timeout witnesses above reach `kill_tree` only on Windows: on Unix a timeout
terminates through `terminate_supervised`, which stores the fate itself, so a `kill_tree` that
stored it only on Windows kept the Linux suite green (#292's round-1 fix-check lens, finding 3).
This reaches `kill_tree` on Linux and Windows through the path that calls it when the spawn's after
phase fails: the child is created (recorded once), the spawn's after phase returns the injected error,
the cleanup termination runs its before phase and its primitive, and its after phase returns the
second error. The failure is the spawn's error with the termination's beside it, the child is gone
when the funnel returns, and the fate is `Gone`. Its scratch directory is the same kind of guard as
the witness above.

Not on macOS. On every Unix that error path drops the Supervisor before it calls `kill_tree`, and
the drop's `finish` has the reaper kill the group and wait until it holds no non-zombie member, so
the leader, this process's unreaped child, is a zombie when the primitive signals its group. On
macOS that signal failed with `EPERM` (#292's CI, `test (macos-latest)`, job 104258732193:
"terminating the agent process group did not establish it gone: the group signal failed (Operation
not permitted (os error 1))"); `signal_group_kill` treats only `ESRCH` as gone, so the primitive
fails before the fate store this witness is about and the fate stays `Unresolved`. That answer is
the primitive's, not the store's: `PR292-MACOS-ZOMBIE-ONLY-GROUP-EPERM-FAILS-KILL-TREE`, deferred,
whose repair runs this witness on macOS again. The direct witness below guards the store on every
Unix.

## `fn kill_tree_stores_a_terminated_groups_fate_before_its_after_phase_errs() {`

`kill_tree` called directly on a live process group of its own, on every Unix host, with an error
answered at `Process.Terminate`'s after phase: the primitive kills the group and reaps its leader,
the error returned is the after phase's, the leader is gone, and the fate is `Gone`. The group is
live when it is signalled, so the macOS answer above does not arise. With the store made
Windows-only, the fate stays `Unresolved` and this fails.

## `fn a_child_registered_pre_exec_is_settled_when_the_parent_never_registers_it() {`

The reaper knows the group **before** the parent registers it, because
the child registered it before `exec`.

`crash_reconstruction`: "Host, Unix: private process groups plus the
per-invocation cleanup reaper **registered pre-exec inside the child**
… leave no unregistered prefix". The existing pre-exec witness asks the
kernel `getpgid(pid) == pid`, which proves `setpgid(0, 0)` ran and says
nothing about the registration beside it — so moving the registration
out of the `pre_exec` closure and into the parent's `register` left
every test passing while re-opening the window the design closes: a
coordinator SIGKILLed between `spawn` returning and parent-side
registration leaves a running group no reaper will settle.

The oracle is that window itself. The supervisor is dropped in exactly
that state — child spawned, parent registration never performed — and
the group has to be settled anyway. Everything the reaper can know here
it learned from the child.

## `fn a_child_registered_pre_exec_is_settled_when_the_parent_never_registers_it() {`

`try_wait` in the loop and `kill` + `wait` in the fallback do settle the
child on every path; the lint does not model `try_wait`.

## `fn a_child_registered_pre_exec_is_settled_when_the_parent_never_registers_it` › `drop(supervisor);`

Not registered by the parent: this is the prefix the packet says
must not exist unregistered. Dropping here is the coordinator dying
in that window, and `Drop` in the `Spawning` phase cancels the
reaper — which settles whatever the reaper knows about.

## `fn a_child_registered_pre_exec_is_settled_when_the_parent_never_registers_it` › `let _ = child.kill();`

Do not leak a 60-second sleeper into the rest of the suite when
this fails.

## `struct ReapedChild {`

The owner of a spawned child for the two group-observation tests below and
their own regression, `PR173-EARLY-ASSERTION-LEAKS-A-ZOMBIE`. Both bodies
assert several times between the spawn and their closing `wait()`, and
`std::process::Child` does not reap on drop: in the scheduling case those
bodies describe in their own messages — the child gone before the first look —
the premise assertion panics, the reap is skipped, and the exited child stays
an unreaped zombie for the rest of the suite. The supervisor of the first test
settles the child's GROUP and never reaps the direct child, measured; the
second has no supervisor at all.

§6: RAII owns child processes, and cleanup happens on early return, error and
panic unwinding — "a guard or resource-owning type beats a `start`/`finish`
pair whose second half can be skipped". The assertions stay exactly where they
are; what changes is who owns the child while they run.

`#[cfg(unix)]` because the three bodies that use it are.

## `impl ReapedChild` › `fn new(child: Child) -> Self {`

The pid is copied once, at construction, so `pid()` answers after `wait()` has
taken the `Child` out and there is no `Option` to unwrap at a call site.

## `impl ReapedChild` › `fn close_stdin(&mut self) {`

`drop(child.stdin.take())`, which is how all three bodies ask the child to
exit. Silent when the child has already been reaped: closing the stdin of a
child that is gone is not a failure to report.

## `impl ReapedChild` › `fn wait(&mut self) -> std::io::Result<std::process::ExitStatus> {`

The deliberate reap, and it takes the `Child` out on success so `Drop` has
nothing left to do. A second call is an error rather than a second `wait`:
once a pid is reaped the operating system is free to reissue it, and a
`kill`/`wait` on a reissued pid is a signal to somebody else's process.

## `impl Drop for ReapedChild` › `fn drop(&mut self) {`

The whole point of the type. `kill` then `wait` on every path the body did not
finish — an early assertion, an `expect`, a `panic!`, a `?` — and nothing at
all after a successful `wait()`, because the `Option` is empty by then. Both
results are discarded because a `Drop` has nowhere to report to and the child
is already leaving: a child that exited on its own answers `kill` with
`ESRCH`, and that is the expected case rather than a failure.

## `fn an_exited_but_unreaped_child_still_answers_for_its_own_group() {`

The regression test for `W2-MACOS-HOST-CONTAINMENT-ROLE-GROUP-FINGERPRINT`
and the experiment `PR125-CLOSE-GROUP-ORACLE-CANNOT-SEE-A-ZOMBIE-ON-DARWIN`
asked for. A child spawned through the production pre-exec path is held
on its stdin and observed running (it must lead its own group, and must
not have exited, or the first look witnesses nothing); its stdin is
closed, `await_exit_without_reaping` waits for the exit itself, and the
second look must find an exited, unreaped child that still leads its own
group. Before the Darwin fall-through in `GroupObservation::leads_own_group`
this failed on the macOS runner with `getpgid` answering `ESRCH` and the
exited record answering the child's own pid (run 34001563243); Linux
passed it throughout, because Linux answers `getpgid` for a zombie.
`register` then `finish` is the supervisor lifecycle production runs, in
production's order: `finish` before the reap, while the child is still a
zombie.
The child is held in a `ReapedChild`, so a premise assertion that fires before
that reap still leaves no zombie behind (`PR173-EARLY-ASSERTION-LEAKS-A-ZOMBIE`).

## `fn a_child_left_in_this_processs_group_never_answers_for_its_own() {`

The control for the fall-through: the same two looks at a child spawned
without the pre-exec step, so it stays in this process's group. Running,
`getpgid` names this process's group; exited and unreaped, the Darwin
record names the same group and the decision stays `false`. A
fall-through that trusted any exited record would pass the regression
test and fail here.
Its child is held in a `ReapedChild` for the same reason, and it has no
supervisor to settle anything on its behalf.

## `fn a_reaped_childs_pid_never_answers_for_its_own_group() {`

The second control the Darwin fall-through needs, and the one
`PR173-DARWIN-FALL-THROUGH-ACCEPTS-A-LIVE-RECORD` asked for: the same
child, through the production pre-exec path, witnessed leading its own
group while it is still an unreaped zombie, then settled and reaped in
production's order. After the reap `observe_child_group` must answer
`exited_before == Err(ECHILD)` — the pid is no longer this process's
child — and the decision must be `false`. The reap is the only thing
between the two looks, so a `true` here is an answer about whatever
holds the pid now rather than about the child.

## `fn only_this_processs_own_zombie_answers_for_its_group_after_an_esrch() {`

macOS only, and the decision itself rather than a process: the case the
finding names cannot be staged on demand, because it needs XNU to reuse
a freed pid for an unrelated live group leader. So the records are built
in the test and handed to `GroupObservation::leads_own_group`, which is
the call site the grid and the oracle both use. This process's own
`SZOMB` record naming its own pid must read `true`, and seven records
must read `false`: a live (`SRUN`) record for a pid that is not this
process's child, a zombie of some other parent, a record naming another
group, no record at all, a running child, this process's own unreaped
child beside a record that is not a zombie's, and a pid this process
never had. A `getpgid` failure other than `ESRCH` reads `false` beside a
perfect record.

MEASURED, because a test that cannot fail proves nothing. At `51ef2c3`
— this body without the repair, and before the review added the seventh
rejected record — `test (macos-latest)` (run 34014712713, job
101436393531, `macos-latest`) failed it on the first record it rejects:
"a live process holding the pid, which `proc_pidinfo` reaches through
`proc_find` before it ever consults the zombie list answered for its own
group: pid 424242: before the look waitid failed: No child processes (os
error 10); getpgid failed: No such process (os error 3);
proc_pidinfo(PROC_PIDT_SHORTBSDINFO, zombie) answered pgid 424242 status
2; after the look waitid failed: No child processes (os error 10)" —
status 2 is `SRUN`. On that same job
`a_reaped_childs_pid_never_answers_for_its_own_group` passed.
`test (ubuntu-latest)` compiles and ran that control, which is
`#[cfg(unix)]`, and passed; this test is `#[cfg(target_os = "macos")]`
and no leg but macOS compiles it. `test (winguest)` compiles neither.

The seven rejected records exercise each condition on its own. Six fail
on `exited_before`, on having no record to read, or on the group the
record names, so `pbsi_status` never decides them; the seventh pairs
`Ok(true)` with its own pid and an `SRUN` record, and the `SZOMB`
comparison is the only thing that rejects it. Drop any one of the three
conditions and one of the seven reads `true`: without `exited_before`
the other parent's zombie does, without `SZOMB` the seventh does, and
without the group comparison the record naming another group does.

## `fn a_premise_that_fails_before_the_reap_leaves_no_zombie() {`

The regression for `PR173-EARLY-ASSERTION-LEAKS-A-ZOMBIE`. It runs the shape of
the two bodies above — spawn, hold on stdin, `await_exit_without_reaping`, the
premise look — and then panics, inside `catch_unwind` and holding the guard,
where the scheduling case makes the premise assertion fail. The claim is about
what is left AFTER that unwind:
`observe_child_group(pid).exited_before` must be `Err(ECHILD)`, the answer for a
pid this process no longer has as a child, rather than `Ok(true)`, the answer for
an exited, unreaped zombie.

THE SETUP IS ESTABLISHED OUTSIDE THE `catch_unwind`, AND THE PANIC IT CATCHES IS
IDENTIFIED. `PR173-ZOMBIE-REGRESSION-SWALLOWS-PREMISE-FAILURE`: an earlier shape
ran the whole body inside `catch_unwind` and accepted `is_err()`, so a timed-out
`await_exit_without_reaping` or a failed zombie premise was caught, reaped by the
guard exactly as an intended unwind would be, and reported as a pass — the test
went green having established nothing. §12: a missing prerequisite fails
diagnostically rather than being read as the condition under test. So the wait
and the premise assertion run in the test's own frame, where their failure is the
test's failure, and the closure holds only the guard and one panic whose payload
is compared against the message the test itself wrote. Any other panic reaching
`catch_unwind` fails on that comparison instead of standing in for it.

`let _owned_across_the_unwind = child;` is load-bearing rather than a name for
nothing: a `move` closure captures the variables its body USES, so a closure that
only panicked would not take the guard at all, and the child would be dropped by
the enclosing frame after the observation instead of by the unwind.

MEASURED, because "the test passes" is not evidence that it detects. On the base
the same body without the guard — the two tests' own ownership shape — leaves
`exited_before: Ok(true)`. With the guard's `Drop` body emptied the test fails on
`Ok(true)`; with `wait` dropped from it and only `kill` kept it fails the same
way, since a killed child is a zombie until somebody reaps it. With the premise
assertion forced to fail, the test fails on the premise instead of passing.

`ECHILD` is also the answer if the pid were reissued to an unrelated process
between the reap and the look, so the assertion cannot be satisfied by a zombie
under any scheduling.

## `fn timeout_kills_the_process_tree_quickly()` › `let script = if cfg!(windows) {`

Through the shell, the sleeper is a grandchild — exactly the
claude.cmd shim shape this module must handle.

## `fn timeout_kills_a_background_grandchild_before_it_can_escape() {` › `let parent = std::env::temp_dir();`

The marker was `temp_dir()/upstroke-proc-tree-<pid>-<test>.marker`, opened by a discarded
`remove_file` of a name a later process under a recycled pid computes again
(`PR64-CLEANUP-003-SCRATCH-PRECLEAN`). It is now in a tree this test acquired, whose guard
reclaims it.

## `fn every_pipe_writer_is_gone(fd: libc::c_int) -> bool {`

Whether every writer of `fd`'s pipe is gone, asked of the kernel and
answered now.

A dead process holds no descriptors, so an immediate `EOF` from a
non-blocking read is exactly "nothing that inherited this pipe is still
running" — and unlike `kill(pid, 0)` it is not answered `Ok` by a
zombie waiting for its reparented reaper. `EAGAIN` is the other answer:
somebody still holds the write end.

**Bytes are not an answer, so they are drained rather than counted.**
`read` returns how many bytes it moved, and this used to compare that
against zero: one byte of anything on the child's stderr — a shell
diagnostic, a linker warning, a locale complaint, none of which this
fixture controls on every platform — then reads as "a writer is still
there" for as long as the byte sits in the pipe, which is forever. EOF
is a property of the pipe once it is empty, so emptying it first is
what makes this question the one the caller means.

## `fn every_pipe_writer_is_gone(fd: libc::c_int) -> bool` › `0 => return true,`

EOF: no descriptor for the write end exists anywhere.

## `fn every_pipe_writer_is_gone(fd: libc::c_int) -> bool` › `1.. => (),`

Somebody wrote. Not an answer either way — drain and re-ask.

## `fn every_pipe_writer_is_gone(fd: libc::c_int) -> bool` › `_ => return false,`

`EAGAIN` (a writer holds it) or `EINTR` (ask again later).

## `fn kill_tree_settles_the_whole_unix_group_before_it_returns() {`

`kill_tree` settles the child's whole **group**, and does it before it
returns.

This is the one path on Unix that reaches `kill_tree`, and no test drove
it: the explicit `kill(-pgid, SIGKILL)` could be deleted outright and
the suite stayed green, because everywhere the funnel *is* exercised the
per-invocation reaper settles the same group and either mechanism alone
satisfies every assertion. Nothing here starts a reaper, so `kill_tree`
is the only thing that can settle this group — which is what tells the
two apart.

The oracle is `kill_tree`'s own doc comment turned into a question:
"the real agent process would survive, keep running, and **keep the
pipes open**". A group member that outlived the call still holds the
inherited stderr, so the read end is not at EOF.

## `fn kill_tree_settles_the_whole_unix_group_before_it_returns` › `let mut command = shell(`

Staged and renamed, not written in place. The waiter below polls for
the path, and a path that is created and then filled is observable
before the state it stands for (CODING_STANDARDS.md §12).

## `fn kill_tree_settles_the_whole_unix_group_before_it_returns` › `let published = readiness::await_signal(&ready, &mut tree.child, Duration::from_secs(10))`

Producer-aware: the direct child is the only liveness fact a file
signal has, and a fixture whose `sh` never started would otherwise
have spent the whole bound before saying so.

## `fn kill_tree_settles_the_whole_unix_group_before_it_returns` › `let deadline = Instant::now() + Duration::from_secs(5);`

Bounded rather than instantaneous, and the bound is the kernel's:
`kill(-pgid, SIGKILL)` returns as soon as the signals are queued, so
a member can still be tearing down when this line runs. What the
bound cannot absorb is a member that was never signalled — the
fixture's survivors sleep for a minute.

## `fn a_successful_direct_exit_settles_its_group_before_the_transcript_is_collected() {`

A direct child that exits successfully does not leave its group behind.

`successful_direct_exit_still_kills_detached_group_members` plants a
detached grandchild and then sleeps 1.3 s before looking, so a
settlement that happened *after* the supervisor returned would still
pass it. This one asks inside the supervisor's own window: the
grandchild writes to the inherited stdout after a second, and the
funnel's post-exit drain grace is two, so a grandchild that outlived the
return lands in the transcript the caller is given.

## `fn every_unix_containment_point_is_measured_against_its_own_operation() {`

Every Unix containment point, measured against the operation it is named
for rather than against the other points.

The Unix half of the same gap: `containment_sub_effects` says "ST-07
evidence executes each point **on its platform**", and the suite checked
that these four exist, are declared Unix, and fire in the packet's order
relative to each other — never that the thing each one is named for had
happened. `ReaperStarted` says the per-invocation reaper is forked *and
holding R28*; `PreExecPgidAndRegister` says the child leads its own
group; `Registered` says the parent has it. Each could move to the wrong
side of its own operation and stay green.

The oracles are outside this crate wherever one exists: `getpgid` for
the group (`child_leads_its_own_group`, already the pattern for one
point and now for all of them) and `flock` for the hold — R28's own
primitive, asked from the coordinator while the reaper owns it.

## `struct Row` › `child_known: bool,`

Whether the child exists yet, from `child_created`.

## `struct Row` › `leads_own_group: Option<bool>,`

`getpgid(pid) == pid`, or `None` before there is a pid.

## `struct Row` › `registered: usize,`

How many times this child's pgid appears in parent state.

## `struct Row` › `cleanup_hold_taken: bool,`

Whether an exclusive probe of R28 is refused right now.

## `impl Observer` › `fn hold_taken(&self) -> bool {`

Whether somebody holds R28 shared, asked with R28's own
primitive from a descriptor this test opened.

## `fn every_unix_containment_point_is_measured_against_its_own_operation` › `let public = std::env::temp_dir().join(format!(`

A run directory with a live cleanup lease, so the reaper has an R28
to take. Without one `lock_cleanup_paths` is handed an empty list and
the hold this test is about does not exist.

## `fn every_unix_containment_point_is_gated_on_unix_and_not_on_one_unix() {`

Where the four Unix containment points are compiled in.

`os_matrix` states the invariant for **all** Unix — "Linux and macOS
(`cfg(unix)`): the cleanup reaper survives coordinator death, settles
the dead coordinator's process groups while holding R28" — not for
Linux. Narrowing any of these gates to `target_os = "linux"` would take
macOS out of the containment contract, and no test on this box or on the
Windows guest would notice: the emission would simply stop existing on a
platform neither of them is. CI does run `macos-latest`, so this is an
ordinary coverage gap rather than an unmeasurable one, and a census
closes it without a macOS machine.

The reaper's own `target_os` gates are a different thing and stay: the
group scanner reads `/proc` on Linux and asks `/bin/ps` on macOS, which
is two implementations of one behaviour, not one platform dropped.

## `fn every_unix_containment_point_is_gated_on_unix_and_not_on_one_unix` › `let gate = lines[..index]`

The nearest preceding attribute is the gate this emission is
compiled behind.

## `fn unix_reaper_reparent_helper() {`

A disposable coordinator that leaves a non-`exec` fork holding the
reaper's command pipe, then is hard-killed.

The fork is the whole fixture: descriptors survive `fork` whether or not
they are `CLOEXEC`, so this process's death closes no write end and the
reaper never sees EOF. What it does see is reparenting.

## `fn unix_reaper_reparent_helper()` › `std::mem::forget(supervisor);`

Unreachable in the fixture: the parent hard-kills this process.

## `fn the_reaper_settles_its_group_on_reparenting_without_waiting_for_pipe_eof() {`

The reaper settles its group on **reparenting**, without waiting for the
command pipe to close.

`os_matrix`'s Unix half is stated for macOS as much as Linux, and on
Darwin an exec-racing descendant can retain a pipe writer, so EOF is not
a trustworthy parent-liveness signal — which is why `reaper_loop` polls
`getppid()` at all. That check is invisible in every ordinary test
because the coordinator's death closes the pipe too. Here a fork that
never execs holds the write end open, so EOF never arrives and the
reparenting check is the only thing that can settle the group.

Its cleanup holds what it signals (`PR340-R2-1`). The first shape ended
with `kill(holder, SIGKILL)`, `kill(agent_pid, SIGKILL)` and
`kill(-agent_pid, SIGKILL)`, sent to saved numbers after the wait, under
a comment calling them a no-op for the passing case. On that path the
agent had been seen gone and collected by its new parent, so its pid
and group id were free to be issued again, and nothing held the
holder's pid either: under a reuse each signal would have reached
whatever took the number. Now every signal the test sends goes to its
own unreaped child or to a group one of its own children holds
(`GroupHold`), and nothing is signalled by a number the test no longer
holds.

## `fn the_reaper_settles_its_group_on_reparenting_without_waiting_for_pipe_eof()` › `.process_group(0)`

The coordinator leads a group of its own, so the fork it leaves holding
the pipe is in that group rather than in this process's, and the test
can hold the group and end the fork through it. Nothing the reaper
reads depends on the coordinator's group: it watches its command pipe
and `getppid()`.

## `fn the_reaper_settles_its_group_on_reparenting_without_waiting_for_pipe_eof()` › `let holders = GroupHold::join(coordinator_group, || {`

Two holds, joined while the coordinator still runs, before it is
killed. The coordinator's own group: the coordinator leads it and is
this test's unreaped child, so the number is that group's, and the
hold keeps it the test's once the coordinator has been reaped, with the
pipe holder inside. The agent's group: the coordinator never waits for
its agent, so while the coordinator runs the agent, running or a
zombie, keeps its pid and its group.

## `fn the_reaper_settles_its_group_on_reparenting_without_waiting_for_pipe_eof()` › `let settled = settle_agent_group(agents, Duration::from_secs(20));`

The wait for the agent, 20 s as before, then the agent hold's release
(`settle_agent_group`), which ends the agent's whole group while it is
held, on the passing path as on the failing one: an agent seen gone
says nothing of the group's other members (`PR340-R4-1`).

## `fn the_reaper_settles_its_group_on_reparenting_without_waiting_for_pipe_eof()` › `drop(holders);`

After the settle, as the holder's kill was, so the pipe stays held for
the whole wait and the test cannot pass by an EOF. The drop's release,
like every release, sends the coordinator's group SIGKILL, the pipe
holder included, while its member still holds that group's number,
whatever that member's state (`PR340-R3-2`).

## `fn a_real_ambient_join_failure_refuses_the_write_command() {`

A **real** ambient-job failure refuses the write command.

`crash_reconstruction`: "if the ambient job cannot be created or joined
the write command refuses at startup with a diagnostic before any
workspace effect (no degraded mode; deferred)". The suite's other
ambient failure is the harness injection, and that fires *before* this
step — so the branch that carries a real `join_ambient` error was
unwitnessed, and deleting it (`let _ = windows_job::join_ambient();`)
left `run` and `resume` dispatching with no ambient job while every
test stayed green.

The two failures are told apart by their wording, which is the point:
an injected failure must not be able to stand in for the real one.

## `fn a_real_ambient_join_failure_refuses_the_write_command()` › `join_ambient_job_with(&mut NoHooks, || Ok(())).expect("a successful ambient join proceeds");`

And a join that succeeds is not turned into a refusal.

## `fn the_unix_ambient_join_is_a_no_op_that_consults_no_observer() {`

On Unix the ambient join is a no-op that consults no observer.

`join_ambient_job`'s Unix contract, in its own words: "The hook is not
consulted here either — recording a Windows containment point as executed
on a Unix host would let a Linux CI cell claim Windows coverage." Every
test of the point above is `#[cfg(windows)]`, and the Unix suite asserted
only that the step returns `Ok`, so a Unix arm that consulted the observer
— and let a `HookHarness` record `Spawn.AmbientJobJoined` as reached on
Linux — passed the whole Linux suite. The observer here answers `Error` to
everything, so an arm that consults *and applies* is caught by the `Ok`,
and an arm that consults and discards the answer is caught by the record.

## (end of `fn windows_direct_exit_parent_helper()`)

Returning successfully while the child is live models a CLI shim
whose real worker outlives it.

## `fn windows_self_identity() -> String {`

`{pid} {creation_time}` for this process.

A pid alone is not an identity — Windows reuses them — so a test that
asks "is it gone" by pid could be answered by an unrelated process that
inherited the number.

## `fn windows_escape_watcher_helper() {`

A grandchild that reports the moment it outlives the process Upstroke
waits on.

It announces its own identity, then polls for the direct child's death
and writes `ESCAPED` to the **inherited stderr** only after observing it
gone three times 30 ms apart. `TerminateJobObject` ends every member of
the job at once, so a contained grandchild cannot survive that 90 ms
window; one whose parent alone was killed survives the whole drain grace
and is captured. stderr rather than stdout because the output-limit
fixture deliberately fills stdout past the point where the drain stops
retaining what it reads.

## `fn windows_escape_watcher_helper()` › `thread::sleep(Duration::from_secs(90));`

Long enough that a bounded wait for termination cannot be
satisfied by this process simply finishing.

## `fn windows_escape_parent_helper() {`

The direct child of the two Windows escape fixtures: start the watcher,
wait for it, then either fill stdout or wait to be timed out.

## `fn still_running_after(pid: u32, created: u64, bound: Duration) -> bool {`

Whether `pid` is *still* running after a bounded wait.

The supervisor drops its `ProcessTree` before it returns, so by the time
a caller can look, termination is under way by one route or another and
a process in the middle of its exit path can still answer "alive" for a
few milliseconds. The bound absorbs that and nothing else: an escaped
grandchild in these fixtures outlives it by ninety seconds.

This is the secondary witness. The primary one is the `ESCAPED` sentinel
in the captured transcript, which is exact and unbounded — a contained
grandchild never writes it at all.

## `fn kill_tree_observes_the_windows_job_empty_before_it_returns() {`

`kill_tree` settles the whole job **before it returns**, and the job it
settles is this invocation's own.

Both properties are invisible through the funnel, and for the same
reason: `ProcessTree` is dropped inside the supervisor, and
`KILL_ON_JOB_CLOSE` then terminates every descendant with no help from
any code under test. So both a cleanup that never terminated the job and
a cleanup that terminated only the direct child by pid look, from
outside, exactly like this one. Here the tree is still alive at the
assertion — the handle is open and the fail-safe has not fired — so
whatever settled the grandchild was `kill_tree` itself.

The private job's separate identity is the other half: DESIGN.md:402's
"private per-invocation jobs scope timeouts" is a claim about *which*
job, and the coordinator is a member of the ambient one. A tree that
carried the ambient handle instead would answer this query the other
way — and would terminate the coordinator on the next timeout.

## `fn kill_tree_observes_the_windows_job_empty_before_it_returns` › `if tree.job.contains(std::process::id()) != Some(false) {`

Read the answer before acting on it. If the coordinator really is a
member, *closing* this handle terminates this process — so a plain
`assert_eq!` would unwind, drop the job, and take the report with it:
the run ends with `running 1 test` and no result line, which reads
like infrastructure rather than like this assertion. Leak the handle
instead, and fail in words.

## `fn kill_tree_observes_the_windows_job_empty_before_it_returns` › `let deadline = Instant::now() + Duration::from_secs(3);`

Bounded rather than instantaneous: a job the kernel has already
emptied can still be running the last of its exit paths when this
line does. What the bound cannot absorb is a member that was never
terminated — the fixture's grandchild outlives it by a minute either
way. `tree` is still alive throughout, so KILL_ON_JOB_CLOSE has not
fired and cannot be what settled anything.

## `fn timeout_kills_a_windows_grandchild_before_it_can_escape() {`

The Windows timeout path, watched from the grandchild.

`timeout_kills_the_process_tree_quickly` reaches this branch on Windows
but only ever asks about the direct child; the test that looks for the
grandchild is `#[cfg(unix)]`. This is its Windows sibling.

## `fn the_output_limit_path_settles_a_windows_grandchild_too() {`

And the output-limit path settles the same tree the same way.

`invariants_preserved[0]` is "process supervision, timeout, output
capture … unchanged (host contract: ordinary descendants only)": the
allowance branch is not a lesser kind of termination. Its fixture fills
**stdout**, so the escape sentinel goes to stderr, which keeps its own
allowance and therefore keeps retaining.

## `fn every_windows_containment_point_is_measured_against_its_own_operation() {`

Every Windows containment point, measured against the operation it is
named for rather than against the other points.

`containment_sub_effects` says "ST-07 evidence executes each point **on
its platform**", and the three per-spawn Windows points make claims the
suite could only check by name and relative order: `CreatedSuspended`
says the child exists and is not yet in the private job,
`PrivateJobAssigned` says it is in the private job and *still
suspended*, `Resumed` says it is not. Each could be moved to the wrong
side of its own operation and stay green.

The oracles are the kernel's, following `child_leads_its_own_group`:
`SuspendThread`'s returned count for suspension, `IsProcessInJob` for
membership — the membership question asked of a handle captured through
the assignment seam, so a hook that fires before the assignment has no
handle to ask about. The child's first instruction is a third,
end-to-end witness: a suspended process cannot write it in any amount of
time, so the two pre-resume points sample it after a grace rather than
instantaneously.

The expected table is transcribed from that sentence, not read back.

## `struct Row` › `first_instruction_ran: Option<bool>,`

`None` at `Resumed`: after the resume the child is free to run,
so neither answer would mean anything.

## `fn point(&mut self, point: SubEffectPoint) -> Injection` › `thread::sleep(Duration::from_millis(250));`

Turn absence-at-an-instant into an observation: a running
child writes its first instruction in milliseconds.

## `fn every_windows_containment_point_is_measured_against_its_own_operation` › `wait_for_marker(&ready, Duration::from_secs(20));`

The positive control: the absences above were suspension, not a
helper that never runs.

## `fn a_windows_spawn_that_fails_after_creation_leaves_no_suspended_stub() {`

The two spawn steps that can fail after the child exists leave nothing
behind.

R22: "created as an ambient-job member, so a coordinator death at any
spawn sub-step **incl. the create-suspended prefix** terminates it".
Neither `AssignProcessToJobObject` nor `ResumeThread` fails on a working
machine, so both recovery branches — terminate the private job, kill the
child, wait for it — were unreachable, and either could have returned
the error while leaving a suspended stub that nothing owns.

## `fn terminal_interrupt_helper() {`

Subprocess entry point for the Unix signal-supervision tests below.
Ignored in ordinary test discovery; the parent test invokes only this
case in a fresh process because the expected outcome is SIGINT.

## `fn terminal_interrupt_helper()` › `let no_core = libc::rlimit {`

SIGQUIT normally requests a core dump. Disable it in this disposable
helper so the regression observes supervision semantics without
invoking a host crash reporter (notably ReportCrash on macOS).

## `extern "C" fn record_custom_job_control(_: libc::c_int)` › `let _ = unsafe { libc::kill(parent, libc::SIGKILL) };`

A fork-copied host callback executing in the private guard is a
test failure: terminate the disposable parent immediately so the
outer test observes it rather than relying on private atomics.

## `extern "C" fn record_custom_aux_signal(_: libc::c_int)` › `let _ = unsafe { libc::kill(parent, libc::SIGKILL) };`

Any forked helper that retained this callback turns a harmless
auxiliary signal into an observable failure in the disposable
parent instead of mutating only its private atomic copy.

## `fn drop(&mut self)` › `if let Some(pgid) = self.supervised_pgid {`

A failed assertion must never strand either the helper's guard
group or its separately isolated agent group (the macOS runner
would otherwise wait forever for a suspended descendant).

## `fn spawn_signal_helper` › `.process_group(0)`

Keep a broken child-group setup inside the disposable helper's
group. A regression must fail the test, never suspend the test
runner that is responsible for reporting and cleaning it up.

## `fn spawn_signal_helper` › `unsafe {`

Block before exec so every thread subsequently created by the
Rust test harness inherits the host policy. Blocking only in the
selected test thread would leave another harness thread able to
receive the process-directed signal.

## `fn wait_for_first_progress(marker: &std::path::Path, context: &str) {`

Wait until the supervised worker has written its marker at least once.

**Why this exists.** Every stop test sends its signal to the whole
process group immediately after spawn, and then reads the marker. If the
worker has not yet created it, the group is already stopped, the file can
never appear, and the first read fails `ENOENT` — for ever, not flakily.
`wait_for_stop` cannot cover this: it observes the *helper*, and says
nothing about whether the worker ever ran.

Measured on PR6: `agent::proc::tests::uncatchable_sigstop_covers_the_isolated_tree`
failed on `macos-latest` with *"progress before signal 17: No such file
or directory"* on a tree whose suite had grown to 1243 macOS tests. The
race is PR4-era and pre-existing; it surfaced when the runner got busier.
A test that passes because a spawn usually wins a race is not a test.

## `fn settled_progress_after_stop` › `let deadline = Instant::now() + Duration::from_secs(2);`

A process-group snapshot can report every member stopped while a
write already accepted by the kernel is still becoming visible on
disk (observed on macOS). Require more than two 50 ms worker periods
with no change before measuring the sustained stop. A genuinely
running worker keeps incrementing and either fails here or in the
longer assertion interval at the call site.

## `fn arbitrary_host_callbacks_never_run_in_private_helpers()` › `assert_eq!(unsafe { libc::kill(-parent, libc::SIGUSR1) }, 0);`

The helper parent deliberately retains and observes its host-owned
callback. The guard shares this group but must have scrubbed the
fork-copied callback before unblocking signals.

## `fn arbitrary_host_callbacks_never_run_in_private_helpers()` › `assert_eq!(unsafe { libc::kill(reaper, libc::SIGUSR1) }, 0);`

The private cleanup reaper is in its own group; target it directly so
both fork-only helper types prove the same callback boundary.

## `fn sigkill_of_upstroke_job_still_reaps_the_isolated_agent_group` › `helper.active = false;`

From here onward the test harness must not kill the agent on drop:
only the helper's external reaper is allowed to make progress stop.

## `fn sigkill_of_upstroke_job_still_reaps_the_isolated_agent_group` › `let _ = unsafe { libc::kill(-agent_pgid, libc::SIGKILL) };`

Clean up only after recording the result, so a regression cannot be
hidden while still avoiding a leaked worker after a failed test.

## `fn a_continue_racing_with_suspend_cannot_strand_the_tree()` › `assert_eq!(unsafe { libc::kill(-pid, libc::SIGTSTP) }, 0);`

Deliver the transition back-to-back, before the monitor can promise
whether it has reached its final stop instruction.

## `fn termination_racing_with_suspend_still_kills_the_tree()` › `assert_eq!(unsafe { libc::kill(-pid, libc::SIGTERM) }, 0);`

A terminal signal targets the foreground group. The guard remains
runnable and wakes a parent that SIGSTOP may already have committed.

## `fn pid_directed_termination_kills_a_suspended_tree_without_continue` › `assert_eq!(unsafe { libc::kill(pid, libc::SIGTERM) }, 0);`

Target only Upstroke, not its foreground group and therefore not the
external guard. No external SIGCONT follows: the guard's bounded
probe must expose the pending signal to Upstroke's handler, then let
the ordinary monitor/reaper path settle the whole tree.

## `fn unix_reaper_container_helper() {`

-----------------------------------------------------------------------
ST-16 (d) — the Unix reaper kills the dead coordinator's containers
-----------------------------------------------------------------------

## `fn unix_reaper_container_helper() {`

A disposable coordinator that arms the container scope, starts one
supervised agent, and then waits to be killed.

A subprocess, because the claim is about what survives a coordinator's
death and this test process must survive to assert it. The `docker` the
scope names is a **recording stub**, so the argument vectors the reaper
actually execs are readable afterwards and the assertion is on a
sequence rather than on "a container went away".

## `fn unix_reaper_container_helper()` › `drop(supervisor);`

The **live**-coordinator half: the invocation is settled the
ordinary way and this process exits without dying.

## `fn unix_reaper_kills_labeled_containers() {`

The Unix reaper kills the dead coordinator's labeled containers.

ST-16 (d), and `os_matrix`: "the cleanup reaper survives coordinator
death, settles the dead coordinator's process groups **while holding
R28**, and **additionally kills the dead coordinator's labeled
containers**, closing the orphan window".

Four claims, each separately droppable, and each asserted:

1. the selector names **both** `upstroke.private_root` and
   `upstroke.incarnation`, with two distinct values — a reaper that
   filtered on the private root alone would kill every container of every
   run under `<R>`, including a **live** coordinator's, which is exactly
   what `authoritative_state` forbids;
2. the order is `ps` → `kill` → `rm --force`, taken from the stub's own
   ordered log;
3. R28 is **still held** while the kill is in flight — the stub blocks
   inside `kill` and the reaper is observed alive there, so a reaper that
   released its hold and then reclaimed would fail;
4. the agent group is settled too, so the container half did not replace
   the process half.

**Second field held constant**: the fixture is run twice with the same
scope, the same stub and the same agent — the only thing that moves is
whether the coordinator **dies** or exits cleanly. On a clean exit the
stub is never invoked at all, which is the assertion that keeps a reaper
from killing a live coordinator's containers on the ordinary settle path.

The cells run in `reaper_container_cells`, which this test hands its
three cells and its recording stub. The regression witness
`the_container_cell_reclaims_its_tree_only_after_the_reapers_last_write`
drives the same function with a stub that holds its late writes, so the
lifetime it pins is this test's own and not a copy of it.

## `fn unix_reaper_kills_labeled_containers()` › `&[(true, true), (false, true), (false, false)],`

{program spelling} x {coordinator dies}. The **bare** cell is the
production shape — `runner::container::DOCKER_PROGRAM` is the bare
name `docker` — and here it is resolvable *only* through `PATH`: the
stub is written into a scratch directory prepended to the
coordinator's `PATH`, and nothing of that name exists in the working
directory the coordinator inherits. `execv` does not search `PATH`,
so this is the cell that dies when the resolution before the fork
goes away; the path-spelled cell is what keeps a repair that resolved
bare names from breaking the spelling that already worked.

The fourth cell, {bare} x {lives}, is deliberately absent: on the
clean-exit path the reaper execs nothing at all, so the spelling
cannot discriminate there and the cell would assert the same absent
log as the one beside it.

## `fn unix_reaper_kills_labeled_containers()` › `"#!/bin/sh\n\`

A recording `docker`. It reports one container the first time it
is listed and nothing once that container has been removed, which
is what ends the reaper's bounded round loop. `kill` blocks so the
R28 assertion has a window to observe.

The text is a template: `reaper_container_cells` replaces its
`{CONTAINER_ID}`, so the stub written into each tree is byte for byte
the one the `format!` this replaced wrote.

## `fn unix_reaper_kills_labeled_containers()` › `&[],`

No environment of the test's own for the stub, and a drain hook that
does nothing: the recording stub holds nothing, so there is no gate to
publish. Both are what the witness below uses.

## `fn reaper_container_cells(`

One scratch tree, one disposable coordinator and one reaper per cell,
`(bare, coordinator_dies)` each, with `stub_script` written into the
tree as the `docker` the reaper's scope names. It returns each cell's
reaper pid, in cell order, so that a caller can wait for what that
reaper started once the cell has ended.

`stub_env` reaches the stub through the coordinator's environment, which
the reaper and every stub it starts inherit, as `OsStr` values: a path
passed there is never text a shell parses, and never converted to
`str`. `on_drain_wait` runs at most once per dying cell, when the drain
below first finds the reaper's group with a member and is about to wait
for it; the witness publishes its gate there.

## `fn reaper_container_cells` › `coordinator.envs(stub_env.iter().copied());`

The caller's names, after the fixture's own. The coordinator and its
reaper read none of them; the stub does.

## `fn reaper_container_cells` › `let inherited = std::env::var_os("PATH").unwrap_or_default();`

Only through `PATH`: the scratch directory first, the
inherited entries after it so the stub's own `sleep` still
resolves.

## `fn reaper_container_cells` › `coordinator.wait().expect("reap the coordinator");`

The live half: the coordinator settles its invocation and
exits. Nothing may have been killed on its behalf.

## `fn reaper_container_cells` › `let hold = GroupHold::join(agent_pid, || matches!(coordinator.try_wait(), Ok(None)));`

A dying cell holds its agent's group before the coordinator is killed,
while the coordinator still runs: it never waits for its agent, so for
the whole join the agent, running or a zombie, keeps its pid and its
group (`GroupHold::join`). From here until `settle_agent_group` releases
it the group's number is the test's, and so is every probe of the
agent's pid. A failing assertion in between releases the hold as it
unwinds, ending the agent's whole group while the hold still holds it,
whatever its member's state (`PR340-R3-2`).

## `fn reaper_container_cells` › `assert!(`

(3) R28 is still held while the container kill is in flight.

## `fn reaper_container_cells` › `let deadline = Instant::now() + Duration::from_secs(30);`

(2) The order, from the stub's own ordered log.

## `fn reaper_container_cells` › `let declared = crate::runner::container::census::ReaperContainerScope::new(`

The removal the reaper **actually executed**, against the
declaration in `ReaperContainerScope::remove_argv` rather than
against a literal repeated here (`PR6-ACCT-006`). The fork side
builds its argv from `c"…"` literals that nothing can read back
at runtime, so without this comparison the declaration and the
behaviour are two self-consistent halves with nothing crossing
them — the shape `PR6E-005` measured on the view path. `argv[0]`
is dropped because the stub logs the arguments only.

`--volumes` is what makes it the same removal `DockerCli::remove`
issues: the reaper is the *only* thing that removes a dead
coordinator's containers on Unix, and an `rm` without it leaks
one anonymous volume per container into a state no later census
can discover, the container being gone and nothing else referring
to the volume.

## `fn reaper_container_cells` › `let filters: Vec<&str> = lines[0]`

(1) Both filters, two distinct values.

## `fn reaper_container_cells` › `let settled = settle_agent_group(hold, Duration::from_secs(30));`

(4) The process half still happened: the agent stops answering within
30 s, and the agent hold is released (`settle_agent_group`). Nothing is
signalled by the agent's saved numbers (`PR340-R2-1`); the first shape
sent both of them SIGKILL here after the wait, whether or not the agent
had been seen gone. The release ends the agent's whole group either way,
while the hold still holds it (`PR340-R4-1`): an agent seen gone says
nothing of the group's other members, and an agent still answering at
30 s fails this assertion once its group has been ended.

## `fn reaper_container_cells` › `let drain_bound = Duration::from_secs(30);`

The tree outlives every writer the reaper started
(`PR339-CONTAINER-REAPER-TEST-RECLAIMS-BEFORE-THE-REAPERS-LAST-WRITE`).
The three-line wait above ends while the reaper may still write into
the tree: the `rm` stub creates `removed` after its log line, and the
listing that follows a round which settled an id, the one that ends the
reaper's loop, appends a fourth line and re-creates `argv.log` if the
reclaim had already unlinked it. One of those landing between
`remove_dir_all`'s enumeration and its removal of the root fails the
reclaim with `ENOTEMPTY`, and the guard panics on the normal path. So a
dying cell does not let its guard drop until `drain_reaper_group` finds
the reaper's own process group without a member. Past 30 s, the bound
of the test's other waits, it fails naming the cell, the group and the
tree, and it signals nothing.

Why the group's end is the writers' end. Every process that can write
into the tree after the last assertion is a stub invocation the reaper
forked (`list_labeled_containers`, `spawn_docker`), and none of them
leaves the reaper's group: the reaper made that group its own before
it reported READY (`setpgid(0, 0)` in `fork_reaper`), so the group's id
is the pid the coordinator records at `UPSTROKE_TEST_REAPER_PID_PATH`;
the forked children change no group before `execv`; and the stub is a
non-interactive `/bin/sh`, which runs no job control, so the one
command it forks, `sleep`, runs in the foreground and in the same
group. An ended group is therefore an ended set of writers, observed
directly.

Why nothing is signalled. Once its coordinator is reaped the reaper is
an orphan its new parent collects, so all the test holds of the group
is its number, and a number can be reused once the group has ended.
The probe cannot end the wait early: while the reaper's group has a
member its id stays in use, and no other group can take it. A reuse
after the group ended can lengthen the wait to the bound and fail it
there, a false red, and nothing more, because no path here signals the
group. The first shape of this drain sent the group SIGKILL on that
failing path, which under such a reuse would have reached whatever
group had taken the number (`PR340-R1-1`). Holding the group by
ownership instead would need a member of the test's own inside the
reaper's group, which would itself keep the group from ending, and the
test has no portable way to enumerate a group's other members. The
agent's group is held that way (`GroupHold`) because nothing waits for
that group to end: the test waits for the agent's pid, and the reaper's
settle signals the whole group and then counts only members that have
not exited. What that gives up: a group still busy at the bound is left
to the reaper's own bounds (`reap_bounded` kills a stub that outlives
its 30 s), and the guard's reclaim while the failure unwinds may report
a tree it could not remove.

The reaper's own exit (`alive(reaper_pid)`) would also do here, by
another route: once its coordinator has died the reaper `_exit`s when
`settle_after_coordinator_death` returns, and `reclaim_labeled_containers`
reaps every stub it starts (`reap_bounded`) before it returns. The group
observes the writers without leaning on that discipline, or on nothing
killing the reaper before its stubs have ended.

The reaper is an orphan once its coordinator is reaped, so on Linux
its exit leaves a zombie member until its new parent collects it: the
same reliance on prompt orphan reaping the agent's wait in
`settle_agent_group` already makes. Only the dying cells wait. On the
clean-exit path the coordinator joins its own reaper before it exits
(`Supervisor`'s drop, `finish`, `Reaper::cleanup`, `close_and_wait`),
so that tree has no unjoined writer, and the live cell is unchanged.

What this does not settle: a writer outside the analysed fixture paths
is not excluded by the wait, and the macOS red that filed the finding
has no established cause.

## `fn drain_reaper_group(`

Waits for the process group `pgid` to have no member, probing every
10 ms, and returns how long it waited once `bound` has passed with a
member still answering. `on_wait` runs once, after the first probe that
finds a member and before the first pause: the announcement the
witness's gate is built on. The bound is read before the announcement,
so a zero bound fails without announcing. Nothing here signals; the only
call is `kill(-pgid, 0)`. It is a function of its own so that
`a_reaper_group_drain_fails_at_its_bound_without_signalling_the_group`
can drive its bounded failure against a group that test owns.

## `fn drain_reaper_group(` › `fn group_ended(pgid: i32) -> bool {`

Whether no process of the group `pgid` exists: `kill(-pgid, 0)`
answering `ESRCH`, and nothing else. A group the caller may not signal
(`EPERM`) still exists, and no other answer is read as an end.

## `fn group_member(pgid: i32) -> Child {`

A shell blocked reading its stdin, spawned into the process group
`pgid` (`process_group`; `0` makes it the leader of a group of its own),
with its stdin piped from this test, so it runs until the test closes
that pipe or a signal ends it. It is the member of a `GroupHold`, and
the agent, the other member, the leader and the unrelated group's leader
of the seven tests that drive a `GroupHold` against groups the test
owns. A spawn that cannot join the group (`setpgid` refused, as for a
group that does not exist in this session) fails the test where it is
called.

## `const MEMBER_END_BOUND: Duration = Duration::from_secs(10);`

How long `end_and_collect` and `close_and_collect` wait for a child of
the test's own to exit (`PR340-R3-1`). After SIGKILL the child's end is
the kernel's alone, since no stop, handler or mask can delay that
signal, so the bound bounds a wedged kernel, not a slow shell; after an
end of file it bounds a shell that should exit at once. It bounds a
wedge; it is not a measured margin.

## `fn end_and_collect(child: &mut Child) -> Result<std::process::ExitStatus, String> {`

Ends a child of the test's own by SIGKILL to its own pid
(`Child::kill`) and collects it within `MEMBER_END_BOUND`
(`await_exit_without_reaping`, then `wait`, which reaps at once). The
child is unreaped until that `wait`, so its pid is its and no other's
when the signal is sent; SIGKILL ends it even while a group stop has
suspended it, where an end of file would wait for it to run. Past the
bound the answer is an error naming the pid, and the child stays the
test's unreaped child, so its number is not issued again while the test
process lives. It is how a `GroupHold` ends its member, and how the
controls end and collect a process they end themselves.

## `fn close_and_collect(child: &mut Child) -> Result<std::process::ExitStatus, String> {`

Closes a child's stdin and collects it within `MEMBER_END_BOUND`. It
collects the controls' own children after the release under test has
run, so their statuses say how each ended: by the release's SIGKILL, or
by the end of file this sends. A child that has not exited by the bound
is ended by `end_and_collect` and the answer is an error saying so, so
the control fails with every child collected rather than waiting
without a bound or reading a status this function caused.

## `struct GroupHold {`

A hold on a process group the test does not lead: a member of the
test's own inside it (`PR340-R2-1`). While that member is unreaped,
running or exited and not yet waited for, the group has not ended, so
its number names that group and no other. A process group lasts until
its last member, a zombie included, has left it, and its id is not
issued again, as a pid or a group id, before then (POSIX's process ID
reuse rule); that an exited, unreaped child stays in its group is what
`an_exited_but_unreaped_child_still_answers_for_its_own_group` checks
on Linux and macOS. So every probe and every signal sent to the group's
number while the hold lasts reaches that group. Holding is what the
agent cleanup of the two reaper fixtures lacked: once its coordinator
was reaped the agent was an orphan its new parent collects, and the test
had only its pid and its group id, numbers that are free once the agent
has been collected.

Neither the member's own state nor the agent's says anything about the
rest of the group. The member can exit alone, killed by its pid or by
anything else, while the group's other members run on (`PR340-R3-2`);
the agent can end alone too, as it does when a reaper ends only the
leader or fails before its settle is complete (`PR340-R4-1`). So nothing
here decides whether the group still needs ending: every release ends
it, and nothing reaps the member except the release, after its signal
to the group. The member is the ownership basis, and it lasts until the
signal that needs it has been sent. `released` records that the release
has run, so that the drop after an explicit release does nothing more.

The hold changes nothing the reaper decides. Its settle signals the
whole group, the member included, and then waits only for members that
have not exited (`group_has_non_zombie_members`, on Linux and on
macOS), so the member's zombie is skipped as the reaper's own anchor's
is. And the member's parent is this process, in the same session and
another group, so the agent's group is not orphaned while the hold
lasts. Without the hold, a reaper that died without settling would
leave that group orphaned with its own stopped anchor in it, and the
SIGHUP and SIGCONT a kernel sends such a group could end the agent and
pass the test; with it the agent keeps running and the test fails, as
it should (reasoned, not executed).

## `impl GroupHold` › `fn join(pgid: i32, held_throughout: impl FnOnce() -> bool) -> Self {`

Spawns the member into `pgid`, then asks `held_throughout` whether the
number named the intended group for the whole join. The spawn returns
only after the member's `setpgid` has happened, so an answer taken
after it covers the join. Both fixtures pass
`matches!(coordinator.try_wait(), Ok(None))`: a coordinator still
running has not been reaped, it never waits for its agent, and it
inherits this process's default `SIGCHLD` disposition, without which
this process's own waits would fail, so its agent, running or a zombie,
kept its pid and its group the whole time. For the coordinator's own
group, the coordinator leads it and is this test's unreaped child. Any
other answer means the member may have joined a group that had taken
the number, so nothing may be sent to the number: the member, this
test's own unreaped child, is ended by SIGKILL to its own pid and
collected within `MEMBER_END_BOUND` (`end_and_collect`), and the test
fails there, its message saying how the member ended. The join's
first shape ended the member by closing its stdin and waited for it
without a bound, so a member a group stop had suspended could not read
the end of file and the failure was never reported (`PR340-R3-1`).

## `impl GroupHold` › `fn release(&mut self) -> Result<std::process::ExitStatus, String> {`

The release: once, on every path, and the same on each. The whole group
gets SIGKILL first, while the member, unreaped whether it runs or has
exited, still keeps the group's number the test's; then the member is
ended by its own pid and collected (`end_and_collect`), and after that
the test holds nothing of the group and sends nothing to its number. The
signal's answer is not read: whatever it is, it is about this group.
Nothing decides whether the group is ended, because nothing the test
can see shows that the group's other members have ended: not the
member's state, since a member that has exited proves nothing about the
others (`PR340-R3-2`), and not the agent's absence, since a group whose
leader alone has ended can still hold members (`PR340-R4-1`). The signal
reaches only members of the group the hold holds, so ending them is
containment of that group, never a signal to a number the test no
longer holds (`PR340-R2-1`).

The hold's first shape decided by the member alone: it asked
`try_wait()`, which reaped a member that had exited, and then sent
nothing, so a member killed alone left its group's other members
running. Its second shape took the decision from its caller
(`ends_group`), and `settle_agent_group` passed it only on the failing
path, so once the agent had been seen gone the release ended its own
member and nothing else: a group whose agent alone had ended kept its
other members running (`PR340-R4-1`). The answer is the member's
collected status, or an error naming the group when the member was not
collected within its bound.

## `impl Drop for GroupHold` › `fn drop(&mut self) {`

A hold not released by `settle_agent_group` is released here, by the
same release, which ends its group: a hold dropped while a failed
assertion unwinds, whose fixture never reached its settle, and the
reparenting test's hold on the coordinator's group, whose pipe holder
never ends by itself. A release that has already run is not repeated.
A member not collected within its bound fails the test here, or, while
another failure is already unwinding, is written to stderr instead,
since a second panic there would abort the whole test process.

## `fn settle_agent_group(mut hold: GroupHold, bound: Duration) -> bool {`

The agent cleanup of both reaper fixtures (`PR340-R2-1`): the wait
both already made for the agent to stop answering, a probe every 50 ms
up to the fixture's bound (30 s in the container cells, 20 s on
reparenting), then the hold's release. The agent leads its group, so
the hold's group id is the agent's pid, and the hold is in place for
every probe, so no pid issued again can answer for the agent. A probe
can still answer for the agent's zombie until its new parent collects
it, which lengthens the wait toward its bound and fails it there. It
returns whether the agent was seen gone; each fixture keeps its own
assertion on that.

The release ends the agent's whole group on both paths, while the
hold's member, still unreaped, keeps the group in being, and whatever
that member did in the meantime (`PR340-R3-2`). The wait's answer
decides only what the function returns. That the agent no longer
answers shows that the agent has gone and nothing more: a reaper that
ended the leader alone, or failed before its settle was complete,
leaves the group's other members running, and the release ends them
(`PR340-R4-1`). A member not collected within its bound fails the test.

The first shape followed the same wait with `kill(agent_pid, SIGKILL)`
and `kill(-agent_pid, SIGKILL)`, whatever the wait had found, under a
comment calling them cleanup for the failing case and a no-op for the
passing one. On the passing path the agent had been seen gone, so both
numbers were free, and under a reuse the two signals would have reached
whatever had taken them. Now nothing is signalled by a saved number:
the agent's pid is never signalled, and its group only by the release,
while the hold's member, still unreaped, keeps the group in being. The
hold's first shape decided by the member instead, ending the group only
while its member still ran, so on the failing path a member that had
exited alone left the agent and the group's other members running. Its
second shape signalled the group on the failing path alone, on the
reading that the passing path owed no signal; the agent's absence,
which is all the wait observes, does not show that, and a group whose
agent alone had ended kept its other members running (`PR340-R4-1`).

## `fn the_container_cell_reclaims_its_tree_only_after_the_reapers_last_write() {`

The regression witness for the drain, through the same
`reaper_container_cells`: one `path-dies` cell, whose stub journals, in
a directory of the witness's own, the status of each write the
reaper's stubs make after the third line: `rm` and the status of
creating `removed`, then `ps` and the status of the second listing's
append.

The order is forced by synchronization, not by a delay (`PR340-R1-2`).
After its log line the `rm` stub holds until one of two states has been
published: the drain's gate, an empty file `on_drain_wait` creates once
the drain has found the reaper's group with a member and is about to
wait for it; or the cell's tree gone, which only the guard's reclaim
does. With the drain, the gate releases the hold while the drain
waits, so both writes land and the journal reads `["rm 0", "ps 0"]`.
Without it, the old ordering, nothing publishes the gate, so the hold
ends only once the reclaim has removed the tree's root: the write comes
after the reclaim however long the cell takes to reach it, creating
`removed` fails, and the second listing's `execv` finds no stub and
journals nothing. A delay anywhere in the cell cannot change that
outcome. The first shape of this witness held the `rm` arm for a fixed
two seconds instead, which assumed the cell reached its reclaim within
them: with the drain removed and three seconds of delay added before
the guard dropped, it passed.

The hold is bounded: after 1,000 polls of `sleep 0.01`, at least ten
seconds, the stub journals `rm hold expired` and exits without writing,
so a cell that neither waits nor reclaims fails the witness rather than
passing or hanging it. The bound can only fail the witness, and each
poll tests a published state rather than waiting for time to pass.

What fails at the old ordering, exactly: a write that finds no tree,
not the native `ENOTEMPTY`. The hold moves the late write past the
whole reclaim, so it never races `remove_dir_all`; the witness pins the
lifetime ordering and says nothing about macOS.

The two paths reach the stub as `OsStr` environment values,
`UPSTROKE_WITNESS_JOURNAL` and `UPSTROKE_WITNESS_GATE`, expanded inside
double quotes (`PR340-R1-3`): no path is shell source and none is
converted to `str`, so a temporary directory whose name holds an
apostrophe, or bytes that are not UTF-8, is carried as it is. Both
live in a directory whose own name holds an apostrophe, double quotes,
`$`, backticks, a backslash and spaces, so every run carries such a
path.

`removed` is created by `printf ''` rather than `:` here: a failed
redirection on a special built-in ends a non-interactive shell, which
would take the journal line with it.

## `fn the_container_cell_reclaims_its_tree_only_after_the_reapers_last_write()` › `let decided_by = Instant::now() + Duration::from_secs(30);`

Before the journal is read, the witness waits for the reaper's group
itself, so the journal is final when it is read, at the old ordering as
at the new. The probe is written out here rather than taken from
`drain_reaper_group`, so a mutation of the drain cannot also disable
this wait. Like the drain it signals nothing: past 30 s it fails
naming the group.

## `fn the_container_cell_reclaims_its_tree_only_after_the_reapers_last_write()` › `let late: Vec<String> = match std::fs::read_to_string(&journal) {`

A journal that does not exist is a cell whose stubs wrote nothing late;
any other error reading it fails the witness rather than reading as an
empty journal.

## `fn a_reaper_group_drain_fails_at_its_bound_without_signalling_the_group() {`

The drain's bounded failure signals nothing (`PR340-R1-1`). The member
is this test's own child, a shell blocked reading its stdin, which
`process_group(0)` makes the leader of a group of its own, so the test
holds that group by ownership: the child is not reaped until the test
waits for it, and until then its number cannot name another group.
`drain_reaper_group` is given 100 ms and must fail, having announced
once. The test then closes the child's stdin, the shell's `read` meets
end of file and the shell exits, and its status must carry no signal.
A drain that signalled the group on its failing path, as the first
shape did, ends the child by that signal before its stdin is closed,
and the status says so.

## `fn an_agent_seen_gone_still_has_its_held_group_killed_even_when_its_hold_member_has_exited() {`

The catching control for `PR340-R4-1` with the hold's member exited,
and the corrected form of round 3's first control, which ran the same
sequence to the opposite end. The agent leads a group of its own, and
`other`, a second member, joins it, as the reaper's anchor is in the
fixtures' agent groups. Then the test ends part of the group by hand,
as a reaper that ended some members and not others would leave it: the
hold's member is ended by its own pid and left unreaped
(`await_exit_without_reaping`), and the agent is ended by its own pid
and collected (`end_and_collect`), as its new parent would collect it.
`settle_agent_group` then finds the agent gone and must say so, and
`other`, still running inside the group the hold holds, must end by the
release's SIGKILL, not by the end of file `close_and_collect` sends. The
settle runs under `catch_unwind`, so `other` is collected before any
assertion whatever the release does.

A release that takes the agent's absence for the group's end, as the
hold's second shape did, ends its own member and nothing else, and
`other` ends by its end of file; so does one that decides by its
member's state, as the hold's first shape did, since that member has
exited. Round 3 ran this sequence with `other` as `next_holder`, a
stand-in for a process holding the agent's group number once the agent
had gone, and required it to end unsignalled. That stand-in was a
member of the group the hold still held, so the requirement forbade the
containment that group is owed, and it is reversed here. The obligation
it was meant to guard, that nothing is signalled by a number the test
does not hold, has a control of its own, whose stand-in is outside the
held group
(`a_process_holding_the_agents_pid_outside_its_held_group_is_never_signalled`).

## `fn an_agent_seen_gone_still_has_its_held_group_killed() {`

The catching control for `PR340-R4-1`, the reviewers' sequence: the
previous test with the hold's member left running. Only the agent is
ended, by its own pid, and collected, while the hold's member and
`other` still run in the group the hold holds. The cleanup must report
the agent gone, and `other` must end by the release's SIGKILL. A release
that takes the agent's absence for the group's end ends only its own
member and leaves `other` to end by its end of file. A release that
decides by its member's state signals the group here, its member still
running, and is caught by the previous test instead.

## `fn an_agent_still_running_at_its_bound_is_killed_with_its_group() {`

The obligation the first shape's signals carried on the failing path,
kept: an agent still running when the wait's bound has passed is ended
with its whole group, while the group is held. The agent is this test's
own child leading a group of its own, `other` a second member of that
group, as the reaper's anchor is in the fixtures' agent groups, and the
hold joins while the agent runs. `settle_agent_group` is given 100 ms
and must report the agent not gone; then both stdins are closed and
both processes collected within a bound (`close_and_collect`), and each
must have ended by SIGKILL, the release's signal to the group, not by
end of file. A release that dropped the group signal leaves both to end
by their stdin, and the statuses say so.

## `fn an_agent_outliving_its_bound_is_killed_with_its_group_even_when_its_hold_member_has_exited() {`

The catching control for `PR340-R3-2`, the regression lens's sequence:
the previous test with one step added, the hold's member alone ended by
its own pid and left unreaped (`await_exit_without_reaping`) before the
settle, as anything other than a signal to the whole group might end it.
The agent and `other` still run, so the failing path still owes their
group's ending: the release must send the group SIGKILL while the
exited member keeps it in being, and both must end by that signal. A
release that reads the member's state, as the hold's first shape did,
reaps the exited member, sends nothing, and leaves both to end by their
stdin: `(None, None)` where `(Some(9), Some(9))` is required.

## `fn a_hold_dropped_while_a_failure_unwinds_kills_its_group_even_when_its_member_has_exited() {`

The same obligation on the path that has no settle: a failed assertion
between a fixture's join and its settle, which drops the hold while the
failure unwinds. The hold's member alone is ended and left unreaped, as
in the previous test; then a panic carrying the hold unwinds through
`catch_unwind`, and the drop must end the agent's whole group while the
exited member still holds it. The payload is checked to be the
deliberate panic, so the release observed is the one claimed; the agent
and `other` are collected before any assertion, and each must have
ended by SIGKILL.

## `fn a_refused_join_ends_its_stopped_member_within_its_bound_and_signals_nothing_else() {`

The catching control for `PR340-R3-1`: a refused join reports within a
bound when its member cannot run, and signals nothing but its member.
The group is the test's own: its leader is this test's child, unreaped
until the end, so every signal the test sends to the group reaches its
own children only. The join's ownership check stops the whole group, as
a group stop would suspend a newly joined member, then answers that the
group was not held. The join runs on a thread of its own, under
`catch_unwind`, and its report, the refusal's message, comes back on a
channel within 30 s. The message must be the refusal's and must say
that the member was ended by SIGKILL and collected; the leader, resumed
and ended by its stdin, must carry no signal, so the refusal sent
nothing to the group's number.

A refusal that waits for its stopped member without a bound, as the
join's first shape did, does not report in time: past 30 s the test
ends its own group by SIGKILL, which the leader still keeps the test's,
so the waiting join returns; it then collects the leader and fails,
naming the late report. A refusal that only bounds the wait leaves its
stopped member behind and says so in its message; one that signals the
group's number ends the leader by that signal. Every wait is bounded,
and the leader is collected before any assertion.

## `fn group_leaving_agent_helper() {`

The agent of
`a_process_holding_the_agents_pid_outside_its_held_group_is_never_signalled`.
Spawned as the leader of a group of its own, it waits for one line on
its stdin, moves itself into the process group `UPSTROKE_LEAVE_FOR_GROUP`
names (`setpgid(0, …)`, a group of its own session), then reads its
stdin to the end and returns. A refused move fails it, so it ends with
a non-zero status rather than staying in its group. Without the
variable it returns at once.

## `fn a_process_holding_the_agents_pid_outside_its_held_group_is_never_signalled() {`

The catching control for the obligation `PR340-R2-1` guards: the
cleanup signals the group its hold holds and nothing else, never a
number it does not hold. The defect's harm is a signal sent by the
agent's saved pid or group id once they may name another process. A
pid issued again cannot be staged on demand, and a stand-in inside the
agent's group is a member the release must end (the two tests above
that end the agent alone), so the stand-in here is the process at the
agent's pid itself, moved out of the group the hold holds. `elsewhere`
leads an unrelated group of the test's own. The agent,
`group_leaving_agent_helper`, leads a group of its own, which `other`
and the hold's member join while it leads it; then the agent moves into
`elsewhere`'s group, and the test waits, within 30 s, until `getpgid`
names that group for the agent's pid. From the cleanup's side nothing
tells this process from one the pid was issued to again: the number is
alive, and the group the hold holds does not contain it.

`settle_agent_group` is given 100 ms. The process at the agent's pid
still answers, so the cleanup reports the agent not gone and releases
the hold. Every child is then collected (`close_and_collect`), the
settle having run under `catch_unwind`. `other` must end by the
release's SIGKILL: containment of the held group stands. The process at
the agent's pid must end at its own end of file, with status 0 and no
signal, and `elsewhere` with no signal.

A cleanup that signals the agent's saved pid, as the first shape did
after its wait, ends that process by SIGKILL, and any signal that
reaches the group the agent moved into ends `elsewhere`. A release that
sends nothing at all fails `other`'s assertion, so the test cannot pass
by giving up containment. What it cannot see: a signal to the agent's
group number sent after the release, which can reach only that group's
own members, all ended by then, or a number that is free or has been
issued again; staging the last would take a pid reuse, so that order is
held by inspection: the release's group signal comes before its
member's reap, and nothing else in the cleanup signals the group. Every
process the test signals is a child of its own, unreaped until the end.

## `const READINESS_ROLE: &str = "UPSTROKE_READINESS_ROLE";`

=======================================================================
CODING_STANDARDS.md §12 readiness protocols

The primitives live in `test_support::readiness` because several fixtures
in three modules had each re-derived them; these are their witnesses, and
each one names the subcase it covers. No claim about a bound is made from
wall-clock coincidence: what is asserted is which outcome ended a wait and
whether the producer was still alive when it did, and where an
interleaving is the subject it is *arranged* through a handshake rather
than raced for.
=======================================================================

## `const READINESS_ROLE: &str = "UPSTROKE_READINESS_ROLE";`

Where [`readiness_producer_helper`] takes its role from.

## `const READINESS_SIGNAL: &str = "UPSTROKE_READINESS_SIGNAL";`

Where [`readiness_producer_helper`] publishes, when its role publishes.

## `struct Scratch(PathBuf);`

A scratch directory for one readiness fixture, removed when it ends
however it ends.

§12 asks for "unique temporary directories with RAII cleanup", and the
difference shows up on the failing path rather than the passing one: a
trailing `remove_dir_all` is the line a panicking assertion skips, and
these fixtures publish 64 KiB payloads sixteen at a time.

## `fn readiness_producer_helper() {`

The producer half of the readiness tests.

One helper and five roles, because what these tests vary is the
producer's behaviour and nothing else. §12's bound has to tell three
producers apart — one that is alive and silent, one that is already
gone, and one that is merely slow — and a helper per case would let them
drift into differing in something other than the case.

## `fn readiness_producer_helper()` › `const ALIVE: Duration = Duration::from_secs(120);`

Longer than any bound these tests set, and finite, so a helper
abandoned by a failing parent cannot outlive the suite.

## `fn readiness_producer_helper()` › `const SLOW: Duration = Duration::from_millis(200);`

Long enough to be observed as "not yet", short enough that a healthy
producer still lands well inside a generous bound.

## `fn readiness_producer_helper()` › `"silent" => thread::sleep(ALIVE),`

Alive, and publishes nothing at all: only the bound can end a
wait on this one.

## `fn readiness_producer_helper()` › `"dead" => {}`

Gone at once, having published nothing. §12's fast path.

## `fn readiness_producer_helper()` › `"signal-after" => {`

Healthy but slow. A bound that ended either of these waits would
be timing a producer that was fine.

## `fn readiness_producer_helper()` › `"noise" => {`

Frames records as fast as it can, none of them the wanted one.
The waiter's `recv_timeout` never has to block against this, so
it is the producer that finds out whether the deadline is
checked on the noise path or only on the idle one.

## `fn readiness_producer_helper()` › `let mut out = std::io::BufWriter::with_capacity(1 << 16, std::io::stdout());`

Block-buffered, deliberately. `println!` goes through a
`LineWriter` and pays a syscall per record, which is slower
than a waiter draining the channel -- so the channel keeps
emptying and the deadline never has to be checked on the
noise arm at all. Batching the records is what makes this
producer actually outrun its reader, which is the condition
the arm exists for.

## `fn readiness_producer(role: &str, signal: Option<&Path>, stdout: Stdio) -> readiness::Producer {`

Spawn [`readiness_producer_helper`] in `role`, adopted by the RAII
producer so it is terminated, reaped and its reader joined on every path.

## `fn a_partial_record_is_refused_rather_than_read_as_a_short_one() {`

**Partial writes.** A truncated record is refused rather than read as a
short whole one, and a field the framing cannot carry is refused at the
producer.

§12: "a partial record MUST NOT be readable as a whole one … an
unterminated final record is a truncated write and MUST fail rather than
yield a short value". The first block is the positive control, and it is
what every hand-rolled reader in this crate did: `str::lines` hands the
truncated tail back as a value, and a path is exactly the payload for
which a short value still looks like a plausible one.

## `fn a_partial_record_is_refused_rather_than_read_as_a_short_one` › `readiness::publish(&signal, &["/tmp/upstroke-snapshot", "cafe"]).expect("publish");`

The same fields, framed and published, read back whole.

## `fn a_partial_record_is_refused_rather_than_read_as_a_short_one` › `let error = readiness::publish(&signal, &["two\nfields"])`

And the payload is kept inside what the framing can carry, at the
producer — the only place it can still be told apart from two fields.

## `fn a_live_but_silent_producer_ends_the_wait_at_the_bound() {`

**A live but silent producer.** The bound ends the wait, and the
producer is still running when it does.

§12: "the bound MUST bound a producer that has wedged rather than time
one that is healthy … the fast path is a producer that fails and closes
its channel; the bound is for the one that stays alive and silent."

The pipe half is the one that could not be written before these
primitives existed. Three waits in `src/rundir.rs` checked their
deadline only after a blocking `read_line` returned, so against this
producer the read blocked and the deadline was never reached at all —
the bound was unreachable in exactly the case it was written for.

## `fn a_live_but_silent_producer_ends_the_wait_at_the_bound()` › `const BOUND: Duration = Duration::from_millis(250);`

Small and caller-supplied. A producer that publishes nothing can only
ever reach its bound, so nothing here depends on machine speed.

## `fn a_flooding_producer_is_stopped_by_the_output_allowance() {`

**A flooding producer is stopped by the output allowance.**

The sibling above bounds an *idle* channel, where `recv_timeout` blocks
and its own timeout does the work. This is the other producer: one that
frames records faster than the waiter drains them, so the channel is
never empty and no timeout ever fires. What ends this wait is the byte
bound on the reader — `OUTPUT_LIMIT_BYTES`, this module's own per-stream
allowance — and the assertion names it, because a wait that ended at the
clock instead would mean the reader had gone on growing.

The bound is deliberately generous so the clock cannot be the answer:
under a reader with no byte bound this test does not fail late, it fails
*differently*, reporting `TimedOut` thirty seconds later.

## `fn a_dead_producer_ends_the_wait_without_spending_the_bound() {`

**A dead producer.** The wait ends on the producer's death rather than
on the clock, and says so.

§12's fast path. The bound is set far past anything this suite could
spend, so the claim is that the wait does not wait it out: a waiter that
only watched its signal would report "nothing published in five
minutes", which is the clock talking rather than the death.

## `fn a_producer_whose_reader_cannot_be_started_is_killed_and_reaped() {`

**A reader that cannot be started.** Review round 4's `R4-REG-1` (the PR11
record, §13), in the constructor that had the same order as the one the
finding named: `Producer::adopt` started the reader of the child's stdout
before anything owned the child, so a thread the system refused unwound past
a bare `Child`, whose drop neither kills nor reaps, and the child outlived
its adoption.

The refusal is the OS's, not a simulation: `readiness::unstartable_reader`
asks for a stack no address space can map, so `pthread_create` answers
`EAGAIN`. The adoption must panic — it cannot hand back a producer with no
reader for a piped stdout — and must unwind through the producer's drop,
leaving nothing under the child's pid, not even a zombie
(`fixture::process_exists` counts one). The builder is asked for with the
child's pid, which the test checks, so a refusal that came from anywhere
else is not read as this one. The child is told to run a test no test has
the name of, so it exits at once of its own accord and leaves nothing
behind itself; at the start head it was left unreaped. Unix, because a pid
is what answers for the child; the adoption is the same code on Windows.

## `fn the_bound_is_the_callers_and_it_does_not_time_a_healthy_producer() {`

**Effective deadlines.** The bound is the caller's, it bounds the wait
rather than the producer, and it does not time a producer that is merely
slow.

§12: "a deadline short enough to expire on a loaded runner has become
the signal itself, which is the failure this rule exists to prevent."
Two claims, and the second is the one that keeps the first honest — a
wait that always returned at its bound would satisfy the timing half and
still be useless.

## `fn the_bound_is_the_callers_and_it_does_not_time_a_healthy_producer` › `let silent = scratch.join("never");`

One bound against one silent producer, on the wall clock: the wait
reports the bound its caller passed and does not end before it -- a
lower bound, which a loaded runner cannot fail. Then two bounds, 120 ms
and 480 ms, through `await_signal_by` on a clock the test drives, sixty
milliseconds a reading: the wait reads the clock once for its deadline
and once per poll, so the first ends at its third reading and the second
at its ninth, which is the deadline being the caller's bound and nothing
else -- a bound of the wait's own, whatever its value, would end both at
the same reading. The oracle before this one compared the two waits'
wall-clock durations and required the longer bound to have spent longer,
which a stall of the first wait past their difference of 360 ms fails: CI's
Windows guest measured the 120 ms wait at 558.8 ms against the 480 ms
wait's 485.5 ms (`PR320-R4-WAIT-ORDER-READ-FROM-THE-WALL`). A reading count
has no stall in it.

## `fn the_bound_is_the_callers_and_it_does_not_time_a_healthy_producer` › `const GENEROUS: Duration = Duration::from_secs(30);`

And a producer that is slow but fine is not timed out, at the bound
`src/rundir.rs`'s waits already use.

## `fn a_signal_is_visible_only_after_the_state_it_announces() {`

**Publication before notification, decided rather than raced.**

§12: "a readiness signal MUST be published only after the state it
announces is complete and observable by the waiter", and "a file's
existence is a readiness signal only if the file is published
atomically."

Both halves are observed at one *arranged* instant — the point at which
the record's bytes are entirely written and the publication has not yet
been committed. A producer that reaches that point hands the observer a
turn and waits for it back, so which of the two runs first is decided by
the handshake and not by the scheduler. Nothing here sleeps, spins or
polls, and the test would fail identically on a machine with one core or
a hundred.

The unsound form is run through the same observer first and MUST be
caught, so an observer looking in the wrong place cannot pass the sound
half by seeing nothing.

## `fn a_signal_is_visible_only_after_the_state_it_announces()` › `let payload = "x".repeat(64 * 1024);`

Large enough that a reader catching it half-written sees that it did.

## `fn a_signal_is_visible_only_after_the_state_it_announces()` › `let unsound = scratch.join("in-place");`

The unsound form: creation and content are separate events, so at the
arranged instant the name exists and does not yet carry the payload.

## `fn a_signal_is_visible_only_after_the_state_it_announces()` › `let sound = scratch.join("published");`

The sound form, at the same arranged instant: the record is entirely
written and the name does not exist. That *is* atomic publication,
and it is asserted rather than sampled.

## `fn a_signal_is_visible_only_after_the_state_it_announces()` › `assert_eq!(`

And once committed it carries the record whole.

## `struct Observation {`

What the observer saw at the arranged instant.

## `struct Handshake {`

A one-shot handshake: the producer hands the observer a turn and blocks
until it is given back.

## `static HANDSHAKE: std::cell::RefCell<Option<Handshake>> =`

The handshake the producer closure running on this thread should use.

## `fn handshake() {`

Use this thread's handshake at the point the producer has reached.

## `fn at_the_uncommitted_instant(`

Run `produce` on another thread and observe `signal` at the exact
instant the producer has written its record and not yet committed it.

The producer blocks there until the observation is taken, so the
interleaving is decided by this function rather than by the scheduler.

## `fn a_publication_that_fails_after_staging_removes_what_it_made() {`

**Cleanup, including the branch that has something to clean.** A
publication leaves no staging residue; a refused one leaves no claim;
and a publication that fails *after* staging removes the file it made.

The last is the branch a refusal cannot reach — the framing check runs
before anything is created, so it exercises "there was nothing to clean
up" rather than the cleanup. Failing the rename after the record is
staged is what reaches the real one, and the staging name being unique
to the call is what makes removing it safe to do unconditionally.

## `fn a_publication_that_fails_after_staging_removes_what_it_made` › `let refused = scratch.join("refused");`

A refused publish creates nothing at all: the framing check runs
before the staging write.

## `fn a_publication_that_fails_after_staging_removes_what_it_made` › `let blocked = scratch.join("blocked");`

THE POST-STAGING FAILURE. At the seam the record is fully staged;
putting a directory in the signal's place makes the rename that
follows fail on every platform this ships on, which is the only
reachable way to arrive at the cleanup with a file to remove.

## `fn a_publication_that_fails_after_staging_removes_what_it_made` › `let marker = scratch.join("marker");`

The marker form: an empty published file reads as no fields rather
than as a truncated record, and it is unambiguous because `publish`
renames — a partial record is never given this name.

## `fn a_publication_that_fails_after_staging_removes_what_it_made` › `readiness::publish(&signal, &["second"]).expect("republish");`

Republishing replaces the record whole, and stages under a name of
its own rather than a shared one.

## `fn staging_residue(dir: &Path) -> usize {`

How many staging files are sitting in `dir`.

## `fn concurrent_publications_do_not_share_a_staging_name() {`

**Ownership-safe staging.** Concurrent publications of one signal do not
share a staging name, so neither can consume or delete the other's.

A fixed `<signal>.publishing` made this a real collision rather than a
theoretical one: two publishers interleave in one file, and the failure
path of either removes whatever is there — by then possibly the other's
staged record.

The overlap is *arranged*, not hoped for. Every publisher stops at the
seam and waits on a barrier for all the others, so at the instant each
one looks, all eight records are provably staged at once. A machine that
never ran two of these threads together would deadlock the barrier
rather than pass the test vacuously.

## `fn concurrent_publications_do_not_share_a_staging_name()` › `all_staged.wait();`

Every publisher is here, with its record written
and its publication uncommitted.

## `fn concurrent_publications_do_not_share_a_staging_name()` › `all_staged.wait();`

And nobody commits until everybody has looked. A
publisher that renamed first would empty its
staging name out from under a slower one's
listing, which is a race in the *observation*
rather than in what is being observed.

## `fn concurrent_publications_do_not_share_a_staging_name()` › `for (which, staged_now) in seen.iter().enumerate() {`

Eight publications staged at once, under eight names.

## `fn concurrent_publications_do_not_share_a_staging_name()` › `let published = readiness::read_published(&signal).expect("a whole record survives");`

One of them is the published record, whole, and it is a value
somebody actually sent rather than a splice of two.

## `fn staging_names(dir: &Path) -> Vec<String> {`

The staging names currently sitting in `dir`.
