---
id: PR281-CLEANUP-LEASE-HOLD-OUTLIVED-AND-ITS-UNREADABLE-TWIN
severity: P2
disposition: deferred
category: correctness
pr: 281
reviewed_sha: 2467df322b17ba8ee93eb345e1a19367c0b75e37
location: src/engine/topology/recover/tests.rs:8356
provenance: pre_existing
first_bad:
guard: the round that takes up the run cleanup lease. It also blocks `PR125-CLOSE-MACOS-READY-RED-CAUSE-UNKNOWN`, whose confirmation window this sighting falls in; that row closes only when this one is classified or the owner reads the evidence as sufficient
---

## Failure sequence

`engine::topology::recover::tests::a_host_integration_reaper_holds_the_runs_cleanup_lease` fails
intermittently on macOS at `src/engine/topology/recover/tests.rs:8356:5` with *"the hold outlived
the reaper that took it"*. The test drives one gate through the production host runner, records
`rundir::observe_cleanup_hold` at each `SubEffectPoint::ReaperStarted`, and then asserts that the
hold is gone once the drive returns. It is not: something still holds the run's `cleanup.lock`
after the reaper that took it has ended.

**Sighting, read in full**: job `103674713617`, run `34738739536`, 2026-09-13T04:48:01Z, branch
`fix-P1/security-trust_a-failed-sentinel-write-manufactures-a-matching-finding`, head `339a238b`.

**Not reproducible in isolation**: 40 consecutive runs of `engine::topology::recover::tests::` at
`2467df32` on Linux, all green. The shape wants the whole suite, which is consistent with a forked
child of another test inheriting the lease descriptor.

**Under the whole suite it reproduces, on Linux, at `:8356`.** A full
`cargo test --all-targets --all-features` at `7e214b4` failed

```
thread '…a_host_integration_reaper_holds_the_runs_cleanup_lease' panicked at
src/engine/topology/recover/tests.rs:8356:5:
the hold outlived the reaper that took it
```

— the same assertion and the same location as the macOS sighting, on a platform where the Darwin
READY race this shape is being confused with **cannot occur**.

**Measured, with the merge base as a control run under the same conditions.** Two full suites run
concurrently on one box, four runs each, `46e8be8` against the merge base `231c1aad`:

| | runs | failed | `a_host_integration_reaper_holds_the_runs_cleanup_lease` |
|---|---|---|---|
| head `46e8be8` | 4 | 4 | **3** |
| merge base `231c1aad` | 4 | 3 | 0 — but `repeated_container_launch_outages_…` once, the same lease |

The diff between them is markdown under `findings/` alone
(`git diff --name-only 231c1aad 46e8be8 -- src/ Cargo.toml Cargo.lock .github/` prints nothing), so
the binaries are the same; **master fails three of four under this load too**, and which suite loses
the race for a process-global lease is arbitrary. Run alone, `46e8be8` is green
(`ALL 9 PASS`, `test result: ok. 2490 passed; 0 failed`).

So the `:8356` shape is a real full-suite intermittent that owes nothing to the reaper's startup,
which is what the CI sighting most likely was — **an attribution the reproduction strengthens and
still does not settle**, because job `103617998766`'s own assertion remains unreadable.

**The mechanism also appears in a neighbour.** A
full `cargo test --all-targets --all-features` at `685957d` on Linux failed
`engine::topology::recover::tests::repeated_container_launch_outages_before_start_consume_defers_through_the_production_runner`
at `recover/tests.rs:7939` with

```
Refused { message: "run `01KZ…001` still has a process of its own alive in worktree
/tmp/upstroke-pr7e-…/repo -- an agent-cleanup reaper, or a Git child writing one of its refs --
and that process holds the run's cleanup lease; refusing overlapping engine ownership" }
```

— the same fact, read by a different observer: the run's `cleanup.lock` is held by a process that
should have ended. The run was contended (a second full suite from a sibling worktree, load average
16), and the immediately preceding and following runs of the same head did not reproduce it, so this
is a sighting of the mechanism and not a rate.

## The second sighting cannot be classified at all, and that is the finding

macOS job `103617998766` (run `34717696893`, PR #276, head `8f0f203a`, 2026-09-12T20:38:49Z) prints

```
test engine::topology::recover::tests::a_host_integration_reaper_holds_the_runs_cleanup_lease ... FAILED
```

and is cancelled at `20:45:57Z` — **before libtest prints the assertion**. libtest holds a failing
test's panic in its captured stdout and emits it only in the `---- <test> stdout ----` block after
the run, so a cancellation between the `FAILED` line and the summary destroys the only record of
*which* assertion failed. The run-level archive is byte-identical to the job log (151991 bytes
both), so the text does not exist anywhere.

**That matters because this test fails two ways and they mean opposite things.** Executed at
`2467df32`:

| | command | result |
|---|---|---|
| control | `cargo test --lib …a_host_integration_reaper_holds_the_runs_cleanup_lease -- --exact` | `ok`, exit 0 |
| READY failure injected | the same with `UPSTROKE_TEST_HELPER_EXIT_BEFORE_READY=7` | `FAILED` at `tests.rs:8339`, *"the re-verification ran its gate through the production host runner and the gate's exit 1 rejected it: [Ok(Unavailable { … })]"*, exit 101 |

`Supervisor::begin` calls `spawn_reaper` and is evaluated before `hooks.point(ReaperStarted)`
(`src/agent/proc.rs:198-204`), so a reaper that never reaches READY fails the launch, leaves
`observed` empty, and lands on the **first** assertion — and the engine folds the launch error into
`Progress::Unavailable`, so **the string `Unix cleanup reaper did not initialize` appears nowhere in
the log**. A grep therefore cannot clear this job; only the assertion could, and it was not
retained.

**The general rule this establishes: a cancelled job is not evidence of absence.** Excluding
cancelled runs is right when the question is *"is this head RED?"*: a cancelled run still publishes
a failing check, and it is superseded rather than a verdict, so the readiness tooling sets it aside.
It is wrong when the question is *"did anything FAIL?"* — a cancelled job that printed a `FAILED`
line before it was killed is an observation, and discarding it answers a different question. Of the 314 cancelled `test (macos-latest)` jobs in the 2026-09-06/13 window, 185
retain a log, 94 of those reached the test phase, and **four print a `... FAILED` line** — three of
them the `effects::tests` censuses, which read source text and launch nothing, and this one.

## What the change that takes this up should do

Two things, and they are separable.

1. **The hold.** Establish what still holds `cleanup.lock` when the reaper that took it has ended —
   the likely shape is a forked child of another test that inherited the descriptor — and either
   close that inheritance or give the assertion a bounded wait rather than a single observation.
2. **The classification gap.** A failing test whose assertion is destroyed by cancellation is
   unclassifiable after the fact. Either print the panic where a cancellation cannot eat it
   (`--nocapture`, or `RUST_TEST_NOCAPTURE` on the macOS leg), or accept that cancelled jobs are
   holes in any "no failure of shape X occurred" claim and say so wherever such a claim is made.

## The hold's persistence, measured, and the bounded wait that replaces the single observation (2026-09-22)

The single observation at the end of `a_host_integration_reaper_holds_the_runs_cleanup_lease` is
now a bounded wait (`fix-P2/correctness_a-cancelled-job-hides-which-assertion-failed`). Nothing
above it changed: the first assertion is still the genuine reaper-startup failure this file
distinguishes from the hold, and the two after it are untouched.

**What holds the lease, and for how long.** Under this finding's own recipe — two full
`cargo test --all-targets --all-features` suites run concurrently on one Linux box, each on its
own target directory: the merge base `9bb177ea` untouched beside a copy of it whose last
observation, when it found the lock held, polled every 10 ms until it was free and read
`/proc/locks` and its descendants' descriptor tables while it waited — eight rounds, load average
30 to 35:

| round | instrumented copy: first observation | free again by the next observation, after | merge base, untouched |
|---|---|---|---|
| 1 | held | 50 ms | ok |
| 2 | free | — | ok |
| 3 | held | 68 ms | **FAILED** at the single observation |
| 4 | held | 38 ms | ok |
| 5 | free | — | ok |
| 6 | held | 24 ms | ok |
| 7 | free | — | **FAILED** at the single observation |
| 8 | free | — | ok |

The hold was present at the first observation in **4 of 8** instrumented runs, and the merge base
failed the single observation **2 of 8** times, at this same assertion — 6 of 16 suite runs, the
same phenomenon as the 3 of 4 above at a larger n. **Every hold was gone within about a
millisecond**: the wall-clock figures are the instrumentation's own cost, the second observation
found the lock free every time, a raw `flock` probe 0.2 ms after the first observation still
failed with `EWOULDBLOCK` (three of three, v1), and a read of `/proc/locks` about a millisecond
after it listed nothing for the inode (four of four, v1 and v2; the matcher verified against a live
`flock -s`). Nothing that lives long enough to be named held it.

The holder is not the reaper. `Supervisor::finish` waits for it without a bound
(`ReaperEnding::AcknowledgedExit`, `src/agent/proc.rs`), and its exit releases its shared hold
before the drive returns. What outlives the reaper is a **copy of the run's lease descriptor**:
`WorkspaceManager::update_ref` keeps the lease open in this process for the life of each
`git update-ref` child (`rundir::hold_cleanup_lease_for_child`), the drive's last ref writes are
inside its final step microseconds before the observation, a child another test thread forks in
that window inherits the open file description, and the shared `flock` lasts until that child
closes it — a reaper, guard or probe in its own `close_inherited_fds`, an `exec` at `CLOEXEC` — a
scheduler quantum under load. That is the "forked child of another test" guessed at above, and its
hold is milliseconds, not the lifetime of a process.

**The bound.** `RELEASE_BOUND` is 20 s: the bound `wait_for_cleanup_hold_release` already gives the
eighteen other observations of this lease in the same module (the ledger's post-drop observation
and the finalization matrix's resumes), so one number governs one condition throughout the file;
some ten thousand times the millisecond tail, 300 times even the 68 ms wall-clock figure; and paid
only by a failing run, since a passing one returns at its first free observation. Five seconds
would clear the tail as surely and add a second bound for the same condition; sixty would only
delay a real red. The wait is not a quarantine: with a `sleep 30` child holding the lease through
the production inheritance path, the test fails after 20.03 s with *"the hold outlived the reaper
that took it: the run's cleanup.lock was still held after the full 20s bound, every one of 401
observations over 20.031098878s finding it held"*; with the same child holding it for 2 s the test
passes after 1.9 s. A future red therefore says how long it waited of what bound, which is the
reading a single observation could not give.

**What this does not fix, and why the disposition stays `deferred`.**

- The inheritance is not closed. Every fork-without-exec in this process during a ref write still
  inherits the lease descriptor; the wait tolerates the hold, it does not remove it.
- The neighbour is untouched and was seen **9 times in these 16 suite runs** (7 in
  `repeated_container_launch_outages_before_start_consume_defers_through_the_production_runner`, 2
  in `sampled_cherry_pick_child_kills_every_residue_classified_and_recovered`; 3 on the untouched
  merge base): a production resume through `drive_as` refused with *"still has a process of its
  own alive … and that process holds the run's cleanup lease; refusing overlapping engine
  ownership"* — `RunLock::acquire`'s exclusive probe reading the same inherited hold at one
  instant. A wait inside one test cannot reach it; the change that closes the inheritance does.
- The classification gap (a cancelled job destroys the assertion) is as recorded above and untouched.

`PR125-CLOSE-MACOS-READY-RED-CAUSE-UNKNOWN`: the `:8356` shape now has a Linux mechanism and a
measured duration, so the sighting in that row's confirmation window is no longer unclassified in
kind; whether that reads as sufficient is the owner's, and the row is not edited here.

## The neighbour's holder, constructed, and the fixture's model of a restart (2026-09-23)

The production refusal the neighbour reads — `WorktreeLock::acquire_in_hooked`'s scan finding the
run's `cleanup.lock` held at its one exclusive probe, *"still has a process of its own alive … and
that process holds the run's cleanup lease; refusing overlapping engine ownership"* — is now
constructed on demand and waited out by the recovery fixture
(`fix-P2/correctness_a-cancelled-job-hides-which-assertion-failed`, second pull request). The
refusal itself is untouched: no production lock, probe or wait changed.

**Where it was seen.** A content-deduplicated census of every full library-suite log preserved on
the build box (`~/eight-logs`, the frozen evidence trees, the lane evidence roots): 42 of 782
distinct suites carry this refusal, 34 in
`repeated_container_launch_outages_before_start_consume_defers_through_the_production_runner`, 5
in `sampled_cherry_pick_child_kills_every_residue_classified_and_recovered`, 4 in
`the_cost_of_a_parked_verification_still_refuses_the_next_integration_after_a_restart` and 1 in
`kill_after_report_before_each_cleanup_step`; 7 of the 54 distinct suites since 2026-09-22 carry
it, and none of the isolated runs on record. In the four preserved natural sightings whose logs
retain their order, the failure completes at the same point of the suite each time (lines
1169–1208 of about 2,830), concurrent with the same seventeen recovery tests.

**The holder, by construction.** A copy of the run's own lease descriptor. `WorkspaceManager::
update_ref` opens it through `rundir::hold_cleanup_lease_for_child` and keeps it open until the
`git update-ref` child has exited (measured 1.9–2.3 ms per ref write, five writes in a first
incarnation's final step, the last closed 1.3 ms before the next incarnation's first probe); a
`fork` by any other thread of the same process while it is open copies it, and the copy holds the
shared `flock` until that child's `exec` closes it or the child closes it itself. Two witnesses at
the base `5b16f727`, tree restored by hash afterwards (`~/orch-pr10/repair-cleanup-lease-evidence/`):

- At the refusal site, deterministically: a sibling thread forks while a ref write's copy is open,
  the write ends, this process drops its copy — the lease is still held, `WorktreeLock::acquire_in`
  is refused with the exact message naming the run and `RunLock::acquire`'s probe is refused too,
  and both acquire once the sibling exits. A sibling forked before the copy existed, or one that
  closes its inherited descriptors as the reaper does, leaves the lease free; a real ref-writing
  child still alive is refused at once (236 µs).
- In the failing test's own shape, one process per run: no sibling, 0 of 10 refused; a sibling
  thread forking every millisecond whose children keep their inherited descriptors 30 ms, **6 of
  10 refused with this fingerprint**; the same forks closing their inherited descriptors first,
  0 of 10; ordinary fork-and-exec siblings on a quiet box, 0 of 10.

The design already states the hazard: `cleanup::take` (`src/rundir.rs`) declines to retain its
own lock in the conductor because "arbitrary forked children would inherit its open file
description", and `imp`'s notes record the same measurement for the run lock. What
`hold_cleanup_lease_for_child` retains is exactly such a descriptor, on purpose — the ref-writing
child must inherit it — and its doc said it was inherited by that child only; the doc now says
which forks inherit it and for how long. The refusal's own text still names only the reaper and
the ref-writing child as holders; the third, a sibling fork in its exec window, is not named
there.

**What the fixture now does.** The recovery tests resume a run in the process that drove it,
about a millisecond after the previous incarnation's last ref write. A later resume by the same
process — the second resume of a fixture and every one after it, through the three trunks
`resume_with`, `resume_as_certified_by` and `resume_holding_manager` — first waits, bounded by
`RELEASE_BOUND` (20 s, the same number the file's other eighteen waits use, polled every 50 ms),
until the run's cleanup lease is observed free, then makes the real resume whatever the wait
found; a refusal that follows an expired wait carries the production message with the bound and
the observations appended. A first resume never waits, so every test that plants a hold and
expects the immediate refusal is untouched and now asserts that no wait preceded it. The parked
fork the witnesses use is `workspace_manager::fixture::ParkedFork`: a child of the test process
parked, before it exits, on a socket read, reporting itself from there, holding a copy of the
lease taken exactly as a ref write takes it; the regression test
`a_lease_copy_a_sibling_fork_inherited_is_waited_out_before_the_next_incarnation_resumes` shows a
later resume whose wait observed the copy held and released the fork from inside that
observation, the resume then succeeding, and its twin
`a_lease_holder_that_outlives_the_bound_still_refuses_the_next_incarnation` shows a holder that
outlives a 500 ms bound refused by the resume itself, alive and holding after the refusal, with
nothing appended to the log. The failing test's shape under the forcing sibling: 6 of 10 refused
before, 0 of 10 after.

**The rundir twin, taken up on the same evidence.**
`rundir::tests::a_hold_whose_command_never_spawns_is_released_with_the_descriptor` dropped its own
copy of a lease and observed the lease once; under a whole suite that single observation read the
lease held in 25 of 788 preserved suites (7 since 2026-09-22, one of them this repair's own second
baseline attempt). That each of those sightings read an inherited copy of this kind is the
mechanism hypothesis this record proposes for them — constructed here, and consistent with the
design's own note — and not a per-sighting identification of the holder. The test now proves the
release is its own by identity: no descriptor of the process is open on the lease file once the
hold is dropped, read through `/dev/fd` and `fstat`, never by whether the hold's number still
reads open, a number the next open in any thread of the process reuses (the second pull request's
first review found exactly that reading, `PR320-R1-MAIN-001` and `PR320-R1-REG-001`); it then
observes the lease until it reads free or a 20 s bound runs out, failing with the bound and the
count in the message; beside it,
`a_copy_of_the_lease_a_sibling_fork_carries_outlives_this_processs_own_descriptor` constructs the
copy and shows the single observation reading held and the bounded one reading free only after the
fork's release — a release the bounded observation makes itself, from inside the observation that
read the copy held, so that the order is acknowledged and not timed;
`a_copy_that_outlasts_the_bound_still_fails_the_release_observation` shows a copy past the bound
still failing it; and `a_parked_fork_holds_the_lease_copy_and_its_socket_and_nothing_else` proves
what the parked fork holds: a sentinel socket end this process had open at the fork answers EOF
once this process's own copy is closed, another run's lease open across the fork reads free —
within the same bound, and while the control still lives — once this process's copy is dropped, and
on Linux the child's `/proc` descriptor table is exactly stdio, the socket and the lease; its control
is a parked fork told to keep the sentinel, which cannot answer EOF until released. The round's own
tests had re-created the single observation right after a drop at three places (the other run's
lease in that test, the lease after the `dup2` in the number-reuse regression, and an exact count of
observations after the sibling copy's release); a sibling raw fork whose copy lasted 300 ms failed
each with correct code, and each now observes within the bound and reports the count. The first form of the parked fork (the branch's first commit) held every
descriptor the process had open at its fork for as long as it was parked, which is the collateral
the recovery fixture exists to stop, and the first form of that control was a raw `fork` that
closed nothing and so held every concurrent test's descriptors for as long as it slept
(`PR320-R1-MAIN-005`). The form that landed is a raw `fork` whose child closes everything else
before it reports itself parked, each close checked — `close_range` on Linux, with a checked sweep
by number when that is unavailable or refused, and that sweep on every other Unix, its ceiling the
process's own table as `/dev/fd` lists it and never `sysconf` alone — and reports a close it could
not make instead of announcing itself, on which the parent collects it and fails the setup; it
never execs, its exit closing the copy as an exec would, so there is no spawn whose return could
come early and no join; and its release and its reap, on a drop and while unwinding too, are
bounded from the moment each stage starts (the second review's `PR320-R1-MAIN-002`,
`PR320-R1-MAIN-003`, `PR320-R1-MAIN-006` and `PR320-R1-REG-003`).

**What this does not fix, and why the disposition stays `deferred`.**

- The inheritance is not closed. Every fork of the coordinator during a ref write still copies
  the lease descriptor; the fixture tolerates this process's own copies, it does not remove them.
- No production behaviour changed: the next coordinator's single probe refuses on any hold, as
  the packet's R28 row requires, and this record establishes neither that a different-process
  restart can never meet such a copy nor that it can; only that a copy is a hold the probe is
  right to see for as long as it lasts, and that it lasts a child's fork-to-exec window.
- Which sibling fork held the copy in each natural sighting is not identified; the mechanism is
  established by construction and by the design's own notes, not by a per-sighting instrument.
- The classification gap (a cancelled job destroys the assertion) is as recorded above.
- The refusal's own text still names only the reaper and the ref-writing child as holders; the
  sibling fork in its exec window is a third kind it does not name.
- The constructed 0-of-10 and the natural 7-of-54 are different populations; neither is a rate
  for the suite after this change.

## The test machinery's own assumptions, taken out one by one (2026-09-23, round three)

The second round's independent reviews (`PR320-R2-MAIN-001`–`005`, `PR320-R2-REG-001`–`006`)
witnessed five defects in the test machinery the second pull request added, none in production,
and each witness was reproduced at `e5f944a6` before the repair. Each repair removes an assumption
at the site that made it, with nothing new layered over it:

- **An owned descriptor.** `descriptors_open_on` had made a `File` from every number `/dev/fd`
  listed, a number another thread may close after the listing; it now asks the kernel through
  the number itself — `fstat`, which owns nothing and has no precondition: a number closed since
  the listing answers `EBADF` and is skipped, one reused answers the new file's identity —
  against the target's `stat`, compared field by field, and it opens and closes nothing, so no
  lock this process holds on a file it visits is touched. (The round's first repair, a `stat` of
  the `/dev/fd` entry, read as sound in XNU's source and was not: the native macOS run of
  `1a1734cf` read an empty scan through it; the section below records that run.)
  `a_descriptor_closed_between_the_listing_and_its_lookup_is_skipped_by_the_identity_scan`
  constructs the window with the closing in the observation's hands.
- **A completed close.** The parked fork's sweep had read `EINTR` as a close the kernel made; a
  policy that answers `EINTR` makes none. The sweep now asks `fcntl` `F_GETFD` and takes `EBADF`
  alone as closed, reporting anything else as a failed close, on which the constructor collects the
  child and fails the setup
  (`a_parked_fork_whose_sweep_is_denied_a_close_with_eintr_fails_before_announcing_the_child`, a
  Linux seccomp witness, saying nothing about an ordinary `EINTR`).
- **A single observation.** The twin `a_lease_holder_that_outlives_the_bound_still_refuses_the_next_incarnation`
  had observed the lease once, right after the identified holder's release; it now makes a second
  parked copy beside the holder and observes within `RELEASE_BOUND`, releasing the sibling from
  inside the first observation that reads its copy held. The production refusal after the expired
  wait, the first-attempt behaviour, the unchanged log and the later real resume are as they were.
- **A global EOF read as one child's.** The sentinel proofs had read the sentinel end once, 500 ms,
  and attributed a copy still open to the fork under test; a sibling's fork carries a copy into
  its own window exactly as it does the lease. `sentinel_closed_within` observes the read within
  `LEASE_RELEASE_BOUND` with the same acknowledgement hook, the main test constructs the sibling
  and releases it from inside the first read that finds a copy, the fork under test is proved by
  its own `/proc` table on Linux, and a read past the bound says whose copy it was where the
  platform can say.
- **An unowned child.** `run_parked_fork_scenario` had spawned its scenario process before any
  guard, joined its stderr without a bound and killed only the direct child at its deadline; a
  stopped descendant holding the pipe hung it. The process is now owned from its spawn
  (`ScenarioChild`: its own process group, non-blocking stderr drained in the poll loop, its exit
  peeked before it is collected so the group can be killed while the id is still its own, a `Drop`
  that kills the group and collects), with the group killed within a bound when a descendant
  still holds the pipe (`a_scenario_that_never_ends_is_killed_at_the_bound_with_the_fork_it_left_stopped`,
  `a_scenario_that_exits_leaving_a_child_holding_its_stderr_returns_its_status_and_kills_the_child`,
  `a_panic_after_the_scenario_process_is_spawned_leaves_no_child_behind`).

The inheritance, the refusal's holder list, the classification gap and the disposition are as the
section above leaves them.

## The round's Unix test support, first run on macOS (2026-09-23, round three, continued)

The push of `1a1734cf` was the first time the round's Unix test support ran on macOS at all:
every earlier Apple check on the build box was a cross-compile (`cargo check`, no clippy), and
the body said the runtime there was CI's. CI's native run answered with two jobs red and every
Linux and Windows job green:

- `lint (macos)`: the two `let identity = sentinel_identity(...)` bindings are
  `clippy::let_unit_value` where the non-Linux arm returned `()`. The arm now returns
  `SentinelIdentityUnreadable`, the statement that the holder cannot be read there, and the
  non-Linux `sentinel_attribution` takes it; clippy for both Apple targets under `-D warnings`
  is now among the box's cross-checks.
- `test (macos-latest)`, five of the round's rundir tests: the identity scan read `[]` where the
  hold's descriptor was expected, in the three tests that prove a release or a reuse by
  identity — the `stat` of a `/dev/fd` entry answers no open file's identity on macOS — and
  `set_read_timeout` on a sentinel observer answered `EINVAL` in the sentinel test's last
  observation and in the lowered-soft-limit scenario's child — on macOS a socket whose peer is
  gone accepts no option, where Linux accepts it, so the timeout set at the observation read as
  sound on every Linux run. The lookup is now `fstat` through the number (above), and the
  observer's timeout is set by `sentinel_pair` while both ends are open, never at the
  observation. `fstat` and `stat` are classified `not_an_effect` beside `SYS_close`.

Both are platform facts the round had taken from source reading. The box has no macOS
executor, so each repair is verified here on Linux and by clippy for both Apple targets, with
its reasoning written at the site; its native evidence is CI's.

## One contract for the test machinery's observations, bounds and owners (2026-09-23, round four)

The third round's independent reviews (`PR320-R3-MAIN-001`–`007`, `PR320-R3-REG-001`–`006`)
witnessed seven classes of defect in the test machinery, again none in production, and each
witness was reproduced at `57232d65` before the repair. Three consecutive passes have now found
defects in machinery an earlier round of this pull request added, and the repeated failures are
one shape: an observation's answer -- a pipe's EOF, a kill's errno, a close's errno, a read's
error kind, an `fstat`'s nonzero -- taken as a fact about something else (the group, the process,
the descriptor, the sentinel, the file). This round states one contract and applies it to every
touched setup, error and cleanup path at once, in place of a seventh, eighth and ninth repair of
one site each: one owner holds each process or descriptor from the call that made it; an
observation that failed is reported as a failure and never read as absence, emptiness or
completion; and each stage has one absolute deadline from its start that no retry, interruption
or partial progress restarts, with a fixed small tick where a syscall needs a timeout of its own.
Nothing is added over the helpers -- no reader thread, no join, no supervisor -- and every bound
is one that already existed.

- **The group, not the pipe.** `run_parked_fork_scenario` had read the stderr pipe's EOF as the
  scenario's group being empty and collected the leader on it, leaving a member that had closed or
  redirected its stderr alive (`PR320-R3-MAIN-001`, `PR320-R3-REG-003`). Once the scenario process
  has ended (peeked with `waitid` `WNOWAIT`, its pid and group id still its own) or run past its
  bound, `ScenarioChild::end` kills the group, collects the leader within `SCENARIO_GROUP_BOUND`
  by polling `try_wait`, observes the group until no member is left -- on Linux from `/proc`, a
  zombie counting as ended, elsewhere by `kill` with signal 0 -- and drains the pipe to EOF, each
  within the same bound and each step's failure noted; the regression
  `a_scenario_that_exits_leaving_a_silent_member_in_its_group_has_the_group_ended_before_it_returns`
  reads the member's state the moment the wrapper returns.
- **A refused kill is a refusal.** `kill_group` had discarded the OS's answer, and a `Child::wait`
  followed it both on the timeout path and in `Drop` (`PR320-R3-MAIN-002`, `PR320-R3-REG-004`).
  `kill_group` now answers whether the group was killed; collection is a poll within the bound,
  never `Child::wait`; after a refused kill a leader that has not ended is left uncollected and
  the wrapper returns at once with no status and notes naming the refusal and the pid; `Drop`
  does the same bounded kill-and-collect and prints on stderr the one line it cannot return
  (`a_group_kill_the_os_refuses_leaves_the_wrapper_bounded_and_the_refusal_in_its_text`).
- **A drain turn is bounded work.** The drain had retried `Interrupted` inside its own loop and
  read for as long as output kept coming (`PR320-R3-MAIN-003`, `PR320-R3-REG-005`). `drain_turn`
  makes at most `DRAIN_TURN_READS` reads and returns on EOF, on nothing more to read, on an
  interruption or on an error, each as what it was, so the caller's deadline is checked between
  turns whatever the pipe answers; its contract is driven with controlled readers
  (`a_drain_turn_ends_on_an_interruption_and_leaves_the_deadline_to_its_caller`,
  `a_drain_turn_with_output_that_never_pauses_ends_at_its_work_budget`,
  `a_drain_turn_returns_eof_a_pause_and_a_failure_as_what_they_were`).
- **One readiness deadline.** `ParkedFork::holding` had waited for the child's report with
  `read_exact` under a per-read socket timeout of `READY_BOUND`, so a signal handled more often
  than the timeout restarted the bound forever (`PR320-R3-MAIN-004`). `read_report_within` reads
  the nine bytes under one deadline from the fork, the socket's timeout now a short tick
  (`READY_TICK`) set once before the fork while both ends are open; an interruption, a timeout or
  a short read returns to the same deadline, and the child is collected before the constructor
  fails, as before (`a_report_read_keeps_one_deadline_across_interruptions`,
  `a_report_read_assembles_short_answers_and_reports_an_early_close`).
- **An interrupted sentinel read observed nothing.** `sentinel_closed_within` had panicked on
  `Interrupted` (`PR320-R3-MAIN-005`); it now makes the read again against its absolute bound,
  neither counting the interruption as an observation nor acknowledging it through `on_held`,
  which keeps the acknowledged order, the Linux attribution and the keeper and
  missing-parent-release sensitivities exactly as they were
  (`an_interrupted_sentinel_read_is_made_again_within_the_same_bound`).
- **A failed metadata read is not absence.** `identity_of_the_descriptor` had answered `None` to
  every nonzero `fstat` (`PR320-R3-MAIN-006`, `PR320-R3-REG-001`); it now answers `Ok(None)` for
  `EBADF` alone and the error otherwise, and the scan fails the test naming the number and the
  error, so a scan never proves a release through a read that failed; the lookup is a seam
  (`descriptors_open_on_with`) so the scan's three readings are driven directly
  (`a_lookup_that_fails_on_a_listed_descriptor_fails_the_identity_scan_instead_of_reading_absence`).
- **The descriptor table, not the close's answer.** The sweep had taken a close's `EBADF` as a
  number not open (`PR320-R3-REG-002`), as the second round's had taken `EINTR`. It now asks the
  table before and after each close (`fcntl` `F_GETFD`) and reports any descriptor still open with
  whatever its close answered -- `EIO`, `EINTR`, `EBADF` or success -- and, after a Linux
  `close_range` that answered success, verifies every number the parent listed at the fork
  (`a_parked_fork_whose_sweep_is_denied_a_close_with_ebadf_fails_before_announcing_the_child`,
  `a_range_close_that_answers_success_without_closing_is_found_out_before_the_child_is_announced`).
  A closed number costs one `fcntl` where it cost one `close`; nothing is retried.

The stale rustdoc that called the lookup a `stat` of the entry is corrected
(`PR320-R3-MAIN-007`), and the body's sentences the reviews found wrong -- every restored first-bad
shape failing, where the unprobed lookup passed informatively, and two receipts of older heads
called this head's -- are corrected in the body alone (`PR320-R3-REG-006`). No instrument row is
added: the permanent regressions name only libc items already classified, and the reviewers'
witnesses that name others (`SYS_read`, `SYS_fstat`, `pthread_kill`) run as controls in a private
clone and never in the tree. The inheritance, the refusal's holder list, the classification gap
and the disposition are as the sections above leave them.

## The transitions the one contract had left, taken up at their sites (2026-09-23, round five)

The fourth round's independent reviews (`PR320-R4-MAIN-001`–`004`, `PR320-R4-REG-001`–`004`)
witnessed four classes of defect in the same machinery, again none in production, and each
witness was reproduced at `a632075a` before the repair. The fourth round's contract -- one owner,
a failed observation reported, one absolute deadline per stage that nothing restarts -- was right
and its application was incomplete at exactly the transitions the fourth reviews found: a retry
loop *inside* an observation that the deadline could not see, a deadline consulted on one kind of
answer and not on the others, and a cleanup error computed and then dropped. Each repair removes
the retry or the condition at its site and moves nothing into a new layer: no thread, no
supervisor, no new bound, no allowlist row, and two seams of the kind the readiness repair
introduced (a clock parameter, a call parameter) so that the deadline and the mapping are driven
directly.

- **An observation is one `waitpid`, and the collection after a kill is bounded.** `wait_for`
  had retried `EINTR` inside itself, so `ParkedFork::collect`'s poll could never reach its
  deadline, and the collection after the kill blocked with no deadline at all
  (`PR320-R4-MAIN-001`, `PR320-R4-REG-002`). `observe_child` is one `waitpid` with `WNOHANG`,
  answered as it was -- ended, not ended, or the error, an interruption included -- and
  `observed_within` polls it at a 5 ms tick until the child has ended or the bound runs out,
  an interrupted observation costing the tick and nothing more; `collect` gives the child the
  bound to end by itself, kills it, and then gives it the reap bound to be collected, and
  answers an error naming the pid and the step -- an observation that failed otherwise, after
  which nothing is killed; a kill the OS refused, the child left alive; a kill made and the
  child not collected within the reap bound -- for the release to panic with and the drop to
  print. `is_alive` makes an interrupted observation again within the reap bound and panics on
  any other failure, answering neither alive nor ended
  (`a_release_whose_every_observation_is_interrupted_ends_at_its_bounds_and_says_the_child_is_uncollected`,
  `dropping_a_parked_fork_whose_every_observation_is_interrupted_returns_within_its_bounds`,
  `a_liveness_observation_interrupted_for_the_whole_bound_fails_instead_of_answering`). The
  errors say what was observed and no more: a kill that was sent is "sent", and a child that
  read its release and ended by itself while every observation was interrupted is collected by
  the test with its own status.
- **The release is one write.** `release_and_reap` had written the release byte with
  `write_all`, which retries an interruption inside itself before any collection begins
  (`PR320-R4-REG-002`; a `UnixStream` write is a `sendto`, which the reviewers' policy
  interrupted). `write_release` makes one attempt and answers what it answered; the socket is
  dropped either way, so a child the byte did not reach reads EOF, and a child that reads
  neither is killed at the bound (`a_release_write_is_one_attempt_whatever_the_writer_answers`,
  the attempts counted through a writer that answers what the test says).
- **One deadline, on progress and completion too.** `read_report_within` had looked at its
  deadline only after a read that failed, so a report delivered in successful short reads was
  accepted after it (`PR320-R4-MAIN-002`, `PR320-R4-REG-001`). `read_report_within_by` looks at
  the deadline at the top of every turn -- before the completeness check and before every read
  -- through the clock it is given, and `read_report_within` gives it the wall; the error names
  how many bytes had arrived and what the last read answered
  (`a_report_read_looks_at_its_deadline_before_every_turn_a_successful_short_read_included`,
  `a_report_complete_only_after_its_deadline_is_not_accepted`, and on a real socket with the
  constructor's tick
  `a_report_arriving_slower_than_its_deadline_on_a_real_socket_is_timed_out_with_what_arrived`).
- **A failed cleanup is said, in both owners.** `ParkedFork`'s drop had discarded
  `release_and_reap`'s error, so a kill the OS refused left the child alive in silence; the
  scenario owner's drop had printed only when the leader could not be collected, so a refused
  group kill with a collectible leader printed nothing while a member of the group lived on
  (`PR320-R4-MAIN-003`, `PR320-R4-REG-003`). Each drop now prints on stderr, the one channel a
  drop has, what it could not do and the state that leaves: the parked fork's the error that
  names the step; the scenario owner's one line saying whether the group was killed or refused,
  whether the leader was collected and with what status or left uncollected, and whether the
  killed group was observed empty -- a collected leader is called collected and is not the
  group ended, and a refused kill is called refused. Both are read from a process of its own,
  where the drop's stderr is the wrapper's pipe
  (`dropping_a_parked_fork_whose_kill_the_os_refuses_says_so_and_what_is_left`,
  `dropping_a_scenario_owner_whose_group_kill_the_os_refuses_reports_it_although_its_leader_was_collected`);
  the member the refused kill could not end is ended by the test.
- **The mapping is tested at the call.** The permanent regression of the `fstat` error mapping
  had injected its failure above the mapping, so the shape the third round repaired -- every
  failed `fstat` answered as absence -- survived every permanent test and was killed only by
  the reviewers' seccomp witness (`PR320-R4-MAIN-004`, `PR320-R4-REG-004`).
  `identity_answered_by` is the one place the call's answer is read, and
  `identity_of_the_descriptor` is that with `fstat` as the call; the regression drives it with
  the real `fstat` on the held number and on a number that is never open, and with a call that
  fails with a real errno that is not `EBADF` -- a `waitpid` on this process's own pid,
  `ECHILD` -- so the mapping runs on a nonzero answer with the kernel's errno and the restored
  shape fails it
  (`a_failed_identity_call_is_read_at_the_call_ebadf_as_absence_and_any_other_errno_as_the_error`).
  No policy on `fstat` is needed for that, so no `SYS_fstat` row is added; the reviewers'
  seccomp witness against the real function runs as a control in a private clone.

The inheritance, the refusal's holder list, the classification gap and the disposition are as
the sections above leave them.

## A retry hidden inside a standard call, taken out of both owners, and the regressions' own failing path given an owner (2026-09-24, round six)

The fifth round's independent reviews (`PR320-R5-MAIN-001`–`006`, `PR320-R5-REG-001`–`005`)
witnessed five classes of defect in the same test machinery, again none in production, and every
witness was reproduced at `31e9fdd5` in a private clone before any helper changed (30 of 30
executions as the reviewers recorded them). Four are one class: an owner whose contract is "every
step one attempt, the owner's deadline decides" called a standard-library convenience that makes
an interrupted call again inside itself -- `write_all` and `eprintln!` for the drops' reports,
`File::open` for the group observation, `thread::sleep` for every rest between observations -- or
read two answers as one. The fifth is the regressions' own failing path, which had no owner for
the child it left. This is the sixth round, the looping signal of `MAINTAINING.md`; the repair
removes the class at every owner site rather than the witnessed instances, adds no layer, and
holds each class with a regression that meets the standard call's first-bad shape at the syscall,
through the actual owner. The syscall numbers that needs are five new `not_an_effect` rows in
`effects/wrappers.toml` (`SYS_write`, `SYS_openat`, `BPF_JSET`, `SYS_clock_nanosleep`,
`SYS_sendto`), an instrument change approved for this pull request by the orchestrator; the
effect stays `syscall`.

- **A report that cannot be made ends, and never panics.** Both drops now report through
  `say_on_stderr`: descriptor 2 itself, one `write` per piece of progress, no lock shared with the
  rest of the process, and the first answer that is not progress -- an interruption, an error,
  nothing taken -- ends the report (`write_while_progressing`). `write_all` had made an
  interrupted write again forever, and `eprintln!` had also panicked on a failed write, which
  aborted a caller already unwinding (`PR320-R5-MAIN-001`, `PR320-R5-REG-001`,
  `PR320-R5-REG-002`). Held by
  `a_parked_forks_drop_whose_kill_and_every_stderr_write_are_refused_returns_within_its_bounds`,
  `a_parked_forks_drop_whose_stderr_answers_errors_neither_panics_nor_aborts_an_unwinding_caller`,
  `a_scenario_owners_drop_whose_kill_and_every_stderr_write_are_refused_returns_within_its_bounds`,
  `a_scenario_owners_drop_whose_stderr_answers_an_error_does_not_panic`,
  `a_scenario_owners_drop_whose_stderr_answers_an_error_does_not_abort_an_unwinding_caller` and
  `a_progressing_write_ends_at_the_first_answer_that_is_not_progress`. An OS that refuses the
  channel gets no report; that is its answer.
- **Every open of the group observation is one `open`.** `group_ended` opens each process's
  `stat` through `open_once`, so an interrupted open is a failed observation, noted with the
  group left unaccounted for, and the caller's bound is looked at after every pass
  (`PR320-R5-MAIN-002`;
  `a_scenario_owners_drop_whose_group_observation_opens_are_interrupted_returns_and_says_the_group_is_unaccounted_for`).
- **Every rest is one `nanosleep`.** Every loop both owners keep between two turns rests through
  `rest_within`: one `nanosleep` of the tick or of what remains of the bound, a rest a signal or
  a policy cuts short not made again. `thread::sleep` had made a refused sleep again for its whole
  length, forever (`PR320-R5-MAIN-006`;
  `a_parked_forks_drop_whose_every_rest_is_refused_kills_and_collects_the_stopped_child_within_its_bounds`,
  `a_scenario_wrapper_whose_every_rest_is_refused_still_ends_its_scenario_at_its_bound`). Of
  the sites, the parked fork's pre-kill collection and the wrapper's loop are reached by those
  regressions every time; `is_alive`, the post-kill collection and the scenario owner's
  collection, group observation and drain use the same call at sites no regression reaches
  deterministically, and are not claimed as executed.
- **A collected child is called collected.** `release_and_reap` answers the collection and the
  release write apart, so a release that could not be written with the child then killed and
  collected is reported as exactly that, with the status, by the drop and by `release`'s panic;
  "left uncollected" is said only of a child that was (`PR320-R5-MAIN-003`,
  `PR320-R5-REG-003`;
  `a_parked_forks_drop_whose_release_send_is_interrupted_says_the_child_was_killed_and_collected`,
  `a_parked_forks_release_whose_send_is_interrupted_panics_saying_the_child_was_killed_and_collected`).
- **The regressions' failing path has an owner.** The three interruption regressions of round
  five had checked their bound before joining, correctly, and then left the stopped child with a
  worker nothing could unblock (`PR320-R5-MAIN-004`, `PR320-R5-REG-004`). Their bodies, names and
  assertions are unchanged and now run in a process of their own through the existing scenario
  support: on the failing path the scenario fails at its own bound and exits, and the wrapper
  kills its process group -- its pid uncollected at the kill, so no number that could have been
  handed out again is signalled -- and observes the group empty. The failing path itself is held
  without a mutation by
  `a_scenario_whose_owner_never_answers_fails_at_its_bound_and_its_stopped_child_ends_with_the_group`.

Each syscall-level regression installs its refusals on the owner's thread alone and asks each
one call it must now refuse before the owner meets it (`Refusal::in_force`); a refusal not in
force fails the regression. What stays outside every bound here, and is not claimed: a kernel
call that never returns, a stderr sink that blocks forever with no signal, the standard library's
own retry inside `Command::spawn` before an owner exists, and the forked child's own loops, which
its owner's kill ends. The body's consolidated history now records the aborted `c80f9163` control
campaign as it happened (`PR320-R5-MAIN-005`, `PR320-R5-REG-005`).
