# `src/engine/topology/prelock/tests.rs`

Extended notes for [`src/engine/topology/prelock/tests.rs`](../../../../../src/engine/topology/prelock/tests.rs).

The code is the authority for what it does; this file is the whole of its prose, moved out of
the source verbatim. Each section is headed by the line of code the comment sat above, spelled
as it is in the source, so the heading is the grep string that finds the code.

## `struct Inventory {`

A runtime that answers from a fixed inventory and records every question.

It performs no effect of its own: the four effectful methods return
canned values, which is why a module that may not *call* a
`ContainerRuntime` primitive may still implement the trait.

## `struct Ids;`

Fixed identities, so an assertion can name a literal.

## `struct Scratch {`

A scratch directory that **owns** its tree: the guard
`rundir::scratch_tree::acquire` returns, held for as long as the `Scratch` is.

The predecessor was a `fn scratch(&str) -> PathBuf`: it created the
directory and handed back a path nothing owned, so every invocation left
its root in the temp directory forever — on the ordinary exit, on an
early return, and on the unwind a failing assertion starts. On this
project's build box a directory leaked per test is inode exhaustion,
which `df -h` reports as 72% full while every write fails — and the leak
is not hypothetical: 5050 `upstroke-prelock-*` roots had accumulated in
the temp directory by 2026-08-30, and five runs of this module after the
repair added none.

This file is a `TOPOLOGY_MODULE`: `std::fs::create_dir_all` and every
`std::fs` removal are denied in it, tests included, so both ends go through
a funnel. `scratch_tree::acquire` creates the root with an exclusive,
non-recursive create that refuses an occupied name, and the `ScratchTree`
guard it returns reclaims the tree on the ordinary exit and on an unwind —
after checking that the directory at its name is still the one it created,
and reporting rather than removing when it is not.

Until #322's round 3 this type created its root through
`RunDir.CreatePrivateDir`, which is `create_dir_all` and adopts whatever
already stands at the name, and removed it in a `Drop` of its own by path
through `RunDir.RemovePublicHusk`. Adoption followed by removal by path is
what made a name collision a deletion of the occupant's bytes rather than
a refusal, which #322's round-2 delta review reproduced with the clock
frozen (its D3). Both are gone, and so is that `Drop`.

## `impl Scratch` › `fn new(tag: &str) -> Self {`

A root directly under the temp directory. Every test here takes one; the
two that plant a replacement at a guard's name nest a second inside it.

The name is `acquire`'s: `upstroke-prelock-<tag>-<ten ULID characters>`.
It was `upstroke-prelock-<tag>-<pid>-ThreadId(<n>)` until #322, which a later
process repeats (`PR7-SCRATCH-FIXTURE-LEAK`), and then a ULID's tail over the
adopting create, on the ground that a `TOPOLOGY_MODULE` had no
exclusive-create funnel. That ground was false: `acquire` is one, and
`recover/tests.rs` and `startup/tests.rs` beside this file already took
their roots through it.

## `impl Scratch` › `fn under(parent: &Path, tag: &str) -> Self {`

A root under `parent`, for the two witnesses that plant a replacement at a
guard's name. Nested inside an outer `Scratch`, the replacement goes with
the outer tree however the witness ends, so neither removes anything by
path on its way out.

A refusal panics and names the tag and the refusal: a test that asked for
a tree it owns has nothing to fall back on that would not be another
holder's.

## `impl Scratch` › `fn path(&self) -> &Path {`

The authorized private root a test hands to [`check`].

## `fn a_host_selection_resolves_host_v1_and_carries_its_digest() {`

A host run resolves `host-v1`, digests it, and mints its identities —
and the digest is the one the marker will carry.

## `fn the_pre_lock_checks_leave_no_residue() {`

The pre-lock checks leave nothing behind — not the run directory, not
the private half, not a lock file, not a container.

## `fn the_pre_lock_checks_leave_no_residue()` › `for question in runtime.asked() {`

Every runtime interaction was a read.

## `fn the_container_inspections_run_in_order_and_the_first_failure_ends_them() {`

The four container inspections happen in `run_creation`'s order, and the
first failure ends it.

## `fn the_container_inspections_run_in_order_and_the_first_failure_ends_them() {` › `let unreachable = Inventory::default();`

An unreachable runtime never reaches the image question.

## `fn an_absent_credential_volume_refuses() {`

An absent credential volume refuses, and the digest is never computed.

## `fn a_container_selection_without_a_runtime_refuses() {`

A `Container` selection with no runtime seam refuses rather than
silently proceeding as though the inspection had passed.

## `fn a_private_root_that_is_not_a_real_directory_refuses_before_any_inspection() {`

A private root that is not there refuses read-only, before anything
else is asked.

## `fn a_private_root_that_is_not_a_real_directory_refuses_before_any_inspection() {` › `let absent = root.path().join("absent");`

The root the check is given is a child that was never created; the
guard still owns the scratch tree that child was named under.

## `fn the_authorized_private_root_is_canonical() {`

The recorded root is **canonical**, so the locator the marker carries and
the expectation the census computes are the same value.

## `fn a_scratch_root_is_reclaimed_on_every_exit_including_an_unwind() {`

Every exit reclaims the scratch tree — the ordinary one and the unwind,
which is the exit a failing assertion in any test above takes.

The panic hook is deliberately **not** silenced for the second half.
The hook is process-global and this suite runs in parallel, so a test
that takes it, installs a no-op and restores it can interleave with
another doing the same and leave the process with a no-op hook for good
— every later panic anywhere in the suite losing its message and
backtrace. The few lines this prints cost less than that.

## `fn a_scratch_root_is_reclaimed_on_every_exit_including_an_unwind() {` › `create_private_dir(&path.join("nested"), &mut NoHooks).expect("a child of the root");`

A tree rather than a bare directory: the guard reclaims what a
test left under its root as well as the root itself.

## `fn a_scratch_root_is_reclaimed_on_every_exit_including_an_unwind() {` › `let recorded = Mutex::new(None);`

The path is recorded from inside the closure rather than re-derived
here: re-deriving it would copy `Scratch::new`'s naming rule, and a
witness that agrees with a rule it restates proves nothing about it.

## `fn a_scratch_root_is_reclaimed_on_every_exit_including_an_unwind() {` › `assert!(!path.is_dir(), "a deliberate failure, mid-test");`

The shape of a real failure: an assertion about the run that does
not hold, raised with the guard still in scope.

## `fn a_scratch_root_that_cannot_be_reclaimed_is_reported_rather_than_discarded() {`

A reclamation that fails is **reported**, not discarded — and what the
guard could not reclaim, it did not delete.

`Drop` cannot return, so the alternative to reporting is silence — and
silence here is the same leak the guard exists to close, with nothing to
say it happened. The failure is a real one rather than an injected one:
the tree is removed out from under the guard through
`RunDir.RemovePublicHusk`, and a replacement holding a directory of its
own is created at its name through `RunDir.CreatePrivateDir`, so the
directory the guard finds when it drops is not the one it acquired. Its
reclaim refuses that, and the panic that carries the report is caught here
rather than failing this test. Then the replacement's content is asserted
present: a reclaim that removed whatever stood at its name — as the
path-named `Drop` this type had until #322's round 3 did — deletes it.

**Unix only.** `scratch_tree`'s reclaim closes its handle before it checks
absence because a Windows directory's deletion can complete only when its
handles close, and the guard holds that handle for its whole life, so a
replacement cannot be relied on to take the name while the guard is
alive. The refusals themselves are platform-independent code, witnessed on
every platform in `rundir::scratch_tree`'s own suite: a replaced root
refused, and a failed reclaim raised on the normal path and suppressed
while unwinding.

## `fn a_scratch_root_that_cannot_be_reclaimed_is_reported_rather_than_discarded() {` › `drop(root);`

The guard is built before the closure and moved into it, and its path is
read off it first, so the assertions after the closure have the root
without anything carrying it out. Dropping it here, as the closure's last
statement, is what makes the failed reclaim this test's subject: the guard
drops on the normal path inside `catch_unwind`, so the panic caught is its
report. Without this line the closure only borrows the guard, returns
normally, and `expect_err` fails the test.

## `fn scratch_unwind_with_a_failed_reclamation_child() {`

The child half of
[`a_failed_reclamation_during_an_unwind_does_not_abort_the_process`].

It drives the one corner of the guard's cross-product the two witnesses
above cannot reach: a reclamation that **fails** while a panic is
**already travelling**. `raii-reported` covers failure without an
unwind and `raii-unwind` covers an unwind without a failure; only both
at once reaches the guard's unwinding arm, where a second panic out of
`Drop` would abort the process rather than fail a test, and where the
guard reports on stderr instead of raising.

Everything the child asserts is asserted **in this process**, so the
parent needs no channel back beyond the child's exit, its result line and
its stderr.

## `fn scratch_unwind_with_a_failed_reclamation_child()` › `remove_public_husk(root.path(), &mut NoHooks).expect("the tree reclaims early");`

Removed out from under the live guard, and a replacement created at its
name, so the reclaim the guard attempts while unwinding is refused for a
real reason rather than an injected one — the directory at its name is
not the one it acquired. No fault hook, no permission trick, no timing:
`raii-reported`'s arrangement, and Unix-only for the same reason.

## `fn scratch_unwind_with_a_failed_reclamation_child()` › `let message = caught`

Reached at all only because the destructor did not panic a second
time: a panic out of `Drop` during this unwind aborts, and an
aborted process runs no assertion and prints no result line.

## `fn a_failed_reclamation_during_an_unwind_does_not_abort_the_process() {`

A reclamation that fails **while a panic is already travelling** does
not panic a second time: the process survives it, and the primary panic
is still the one that arrives.

Measured **from outside the process that makes the observation**, which
is forced. A second panic out of a destructor mid-unwind aborts, and an
abort takes the whole test binary — so an in-process witness for this
corner would have to survive its own subject. The child is the witness;
this is the frame that reads its exit.

The child is spawned **through the host Runner**, not through
`std::process::Command`: `std::process::Command` is on the effect
denylist and `src/engine/topology/**` may not reach it even in tests.
The Runner is the funnel that owns `Process.Spawn`, which is exactly the
rule — the same spawn `recover::tests::kill_during_recovery_repeats_recovery`
and `create::tests::spawn_and_wait` already use.

**The first two assertions are load-bearing together, and neither alone
is enough.** `abort()` takes the process before the harness prints
anything about the test, so an aborted child emits no `test result:` line
— but a child whose filter matched *nothing* also exits 0 and prints
`ok. 0 passed`, which a bare exit-code assertion would read as success.
Requiring the zero exit **and** `ok. 1 passed` separates the three
outcomes: aborted, selected-and-passed, and selected-nothing-at-all.

**The third reads the report.** The guard's unwinding arm writes one line,
`scratch tree <root> was not reclaimed while unwinding: <why>`, through
`scratch_tree`'s fallible reporter onto the process's real stderr, which
the child harness's capture of its panic message does not hold. A guard
that stopped reporting on that arm would lose a tree on a failing run in
silence, and this reads red. The line must name the replacement's root,
`upstroke-prelock-replaced-`, so a report about some other tree does not
satisfy it. Unix only, with its child.

## `fn a_failed_reclamation_during_an_unwind_does_not_abort_the_process() {` › `env: Vec::new(),`

Nothing to pass: the child derives its own scratch root from
the temp directory and a fresh ULID of its own, so the two
processes cannot collide and there is no state to hand over.

## `fn a_failed_reclamation_during_an_unwind_does_not_abort_the_process() {` › `assert_eq!(`

`stderr` rather than the whole `ProcessOutput`: the child's stdout
carries its backtrace, and a failure report that buries the one line
that names the cause — `panic in a destructor during cleanup` — under
fifty frames of it is a report nobody reads.
