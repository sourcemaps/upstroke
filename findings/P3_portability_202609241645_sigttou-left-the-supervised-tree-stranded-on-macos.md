---
id: PR320-R7-MACOS-SIGTTOU-STRANDED
severity: P3
disposition: deferred
category: portability
pr: 320
reviewed_sha: ac1cdfbf55e5b42a89e64dda740e5b420ad8bebb
location: src/agent/proc/tests.rs:3005
provenance: pre_existing
first_bad:
guard: a recurrence on native macOS CI, or the next change to agent::proc's job-control supervision or `terminal_input_and_output_stops_cover_the_isolated_tree`: that change or recurrence gets the cause established before anything is repaired; nothing enforces this meanwhile
---

## Failure sequence

This was observed once, on native macOS CI, in code #320 does not touch. **The cause is not
established.**

`terminal_input_and_output_stops_cover_the_isolated_tree` (`src/agent/proc/tests.rs:3013`) runs
`assert_stop_covers_the_isolated_tree` (`:2983`) for `SIGTTIN` and then for `SIGTTOU`. Each pass
does the following:

1. It spawns the signal helper and waits for its first progress.
2. It sends the signal to the supervisor's process group and waits up to 10 s for the stop.
3. It checks that no progress is made for 350 ms.
4. It writes `finish` and sends `SIGCONT` to the group.
5. It waits up to 10 s for the helper to exit (`:3004`).

In CI run 36027570644, job 107728080673 (`test (macos-latest)`), at `ac1cdfbf`, step 5 of the
`SIGTTOU` pass ran out. The test panicked at `src/agent/proc/tests.rs:3005:28` with
`signal 22 left the supervised tree stranded`. The library suite finished
`2733 passed; 1 failed; 70 ignored` in 620.34 s. The failure means the helper had not exited within
10 s of `SIGCONT` after a `SIGTTOU` stop. Whether the tree never resumed or resumed too slowly is
not known. The log records only the timeout.

## What is known

- **#320's diff does not reach it.** The test and its helpers (`src/agent/proc/tests.rs` around
  `:2587`–`:3020`) and the job-control supervision they exercise are unchanged by #320. The PR's diff
  to that file is four hunks, all inside
  `the_bound_is_the_callers_and_it_does_not_time_a_healthy_producer` from `:3722`
  (`git diff -U0 5b16f727 ac1cdfbf -- src/agent/proc/tests.rs`). Its only other change under
  `src/agent/proc/` is `test_support/readiness.rs`, which this test does not reach. The test's code
  at `:2983`–`:3020` is the same at `ac1cdfbf`, at #320's head `9b0b35f1`, and at `master` after
  #320 merged.
- **It passed natively on macOS immediately before and after.** In `test (macos-latest)` the test
  logged `... ok` at `23021ff6` (run 35982877097, job 107578803416), at `980301ff` (run 36018639391,
  job 107697760302, where only five lifeline tests failed), and at `ac1cdfbf` again (run
  36030067532, job 107736279133, library `2734 passed; 0 failed; 70 ignored`). That last run was
  started by a body edit on the unchanged head.
- **It does not appear in 20 other macOS reds.** The author searched the last 40 failed CI runs,
  2026-09-14 to 09-24. Twenty-one had a failed macOS test job, and this test failed in none of the
  other 20 (`~/orch-pr10/repair-320-r7-evidence/ci/ac1cdfbf/history-search.log`). A re-read of those
  20 job logs when this file was written found the test logged `... ok` in 19 of them. The twentieth
  (run 35853662836) failed to compile its library test target, `function
  assert_a_parked_fork_isolates_a_sentinel is never used`, so the test never ran there.
- No finding under `findings/` named this test or its panic message before this file.

## Readings, none established

- A timing failure of a pre-existing job-control test on a loaded macOS runner. That job's library
  suite took 620.34 s. The three neighbouring macOS runs in which the test passed took 546.05 s,
  707.47 s and 292.18 s, so suite duration alone does not separate them.
- A latent race in how `agent::proc` handles `SIGTTOU` followed by `SIGCONT` on macOS, met once.

One run cannot tell these apart. The author did not re-run anything to try.

## What the change that takes this up should do

Establish the cause before repairing anything. On this path the evidence is lost as the test
stands. The helper's own output goes to `helper.log` in its scratch directory, and it is read
(`helper.diagnostic()`, `:3006`) only after the helper has exited. On a timeout, the panic at `:3005`
unwinds, and `SignalHelper`'s `Drop` kills the groups and removes that directory. A first change
could therefore read the diagnostic before panicking. Then, on the next recurrence, check whether
the stopped tree received `SIGCONT` and whether the helper saw `finish`. If the test's 10 s wait is
the defect, the fix is to the test. If the supervisor leaves the group stopped, the fix is
production code, and severity and category should be reconsidered then.

## A later sighting of this test on native macOS, cause unknown (2026-10-29, PR #345)

This section is evidence only, and the cause of what it records is unknown. It records a later
`FAILED` line for this test on native macOS. It does not establish that this is a recurrence of
the failure above. It repairs nothing and closes nothing, and the guard, the disposition and the
frontmatter are unchanged.

**The run.** CI run 40806319069, attempt 1, job 123226497738 (`test (macos-latest)`), at PR #345's
head `8d1be26ac6250bac49a99455c92b0081db5ea704`. The job tested the merge
`1bb4d4383f432bc7ae9b67461553927777faea92` of that head into `master` at `478592e0`, whose tree is
the head's own. The job ran from 02:27:37Z to 02:58:04Z on 2026-10-29, 30 min 27 s. Its
annotations read "The job has exceeded the maximum execution time of 30m0s" and then "The
operation was canceled".

**What its log shows.**

- The library printed `running 3555 tests`. One progress line reads
  `test agent::proc::tests::terminal_input_and_output_stops_cover_the_isolated_tree ... FAILED`.
  At that head the test is at `src/agent/proc/tests.rs:3035`.
- **There is no failure block.** The job was cancelled at the 30-minute limit before libtest
  printed its `failures:` section, so no panic or assertion from this test was captured. The
  log's one panic line comes from another test's helper, `sigchld_target_setup_failure_helper`.
- The library printed no closing summary, and the main and probe test binaries were not reached.
- Of the required rows PR #345's body names, C1, RPT, PIN1..7, T-C7, T-C11 and T-C5 were not
  reached on macOS. T-C14, TOPO4 and TOPO5 printed `... ok`.
- In the same run, the Windows and Ubuntu test jobs and the lint and MSRV jobs succeeded. The
  aggregate `upstroke-ci` check (job 123235992865) failed because of this job.

**What is known. None of it establishes a cause.**

- PR #345 changes no path under `src/agent`.
- This test printed `... ok` on macOS in run 40797944126, at `553babfc`, and on Ubuntu in this
  run.
- This run's macOS tests ran about three times slower per test than run 40797944126's.
- The job carried the notice "Due to capacity constraints, jobs targeting macOS arm64 runners may
  experience longer queue times." The notice is about queue times. It does not establish why the
  tests ran slower.

**Kept apart from the failure above.** The failure above is the witnessed signal 22 assertion: a
panic at `src/agent/proc/tests.rs:3005:28`, at `ac1cdfbf`, reading
`signal 22 left the supervised tree stranded`. This sighting is a `FAILED` progress line with no
panic and no assertion. It does not show that assertion, which pass failed, a timeout inside the
test, or any other mechanism. It is not called a flake, and it is not called a regression.

**What was not done.** No selected-job rerun was requested, because a known HTTP 403 from that
endpoint already rules it out. No production code changed, no timeout was raised, and nothing
was re-run to try. The owner-authorized retrigger is the commit that adds this section. A later
pass is one more sample, and it closes nothing.

The raw job log's SHA256 is `03f4bebb93614da3d832f81b1a0758a0f0e0d4ed9b7e19116d24d0c6d92b582f`.
