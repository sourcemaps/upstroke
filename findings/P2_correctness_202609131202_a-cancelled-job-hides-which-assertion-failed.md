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
