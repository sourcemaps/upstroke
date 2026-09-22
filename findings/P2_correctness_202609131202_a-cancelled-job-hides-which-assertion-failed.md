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
