---
id: CI-FLAKE-MACOS-TERMINAL-INTERRUPT-LEFT-THE-AGENT-TREE-ALIVE
severity: P3
disposition: deferred
category: correctness
pr: TBD
reviewed_sha: fe2b9bc58af6c3eae0d0a9da64ac25a159db374f
location: src/agent/proc/tests.rs:2823
provenance: undetermined
first_bad:
guard: the change that makes this test's failure report what of the agent tree was alive and when the marker was written relative to the interrupt, and then either fixes the tree's kill or moves the marker's timer after the ready file, keeping the assertion
---

# A terminal interrupt left the agent tree alive, by the test's marker, on a hosted macOS runner

## Failure sequence

    the signal helper runs Upstroke over a shell in its own isolated process group
    -> the shell starts a worker that sleeps 1 s and then writes the marker, runs two `ps` calls, and writes `ready`
    -> the test sees `ready`, sends SIGINT to Upstroke's process group, and waits for Upstroke to exit
    -> Upstroke exits, the test sleeps 1.3 s, and the marker exists
    -> the assertion at `src/agent/proc/tests.rs:2823` fails: "signal 2 terminated Upstroke but left its isolated agent tree alive"

`agent::proc::tests::terminal_interrupt_kills_the_isolated_tree` (`src/agent/proc/tests.rs:2831`)
calls `assert_termination_kills_the_isolated_tree(libc::SIGINT, "interrupt")` (`:2810`). That
function does the following:

1. `spawn_signal_helper` (`:2604`) runs the test binary's `terminal_interrupt_helper` in a process
   group of its own. The helper runs Upstroke's supervision over a shell. The shell's script
   (`:2458-2462`) is:

       (sleep 1; printf leaked > "$UPSTROKE_MARKER") & worker=$!;
       shell_pgid=$(ps -o pgid= -p $$ | tr -d ' ');
       worker_pgid=$(ps -o pgid= -p $worker | tr -d ' ');
       printf '%s %s %s %s' $$ $shell_pgid $worker $worker_pgid > "$UPSTROKE_READY";
       wait

   So the worker's one-second timer starts first. The shell then runs two `ps` pipelines and only
   after them writes its `ready` file.
2. `spawn_signal_helper` polls `ready` every 20 ms until it holds four fields. It then checks that
   the shell leads its own group and that the worker is in that group (`:2712-2742`).
3. The test sends the signal to the helper's process group, `kill(-pid, signal)` (`:2815`). It
   waits up to 10 s for the supervisor to exit (`:2816`), sleeps 1.3 s (`:2820`), and asserts that
   the marker does not exist (`:2821-2826`).

The assertion takes the marker's existence to mean that the isolated tree outlived the interrupt.
The failure message says only that. It does not name what was alive, and it does not say when the
marker was written. `helper.complete()` (`:2822`) runs before the assertion, so the helper's
`Drop` does not kill the supervised group afterwards. The helper's own `helper.log` is not printed.

## The sighting

| run | job | platform | date | pull request | head |
|---|---|---|---|---|---|
| `44968824584` (`pull_request`) | `137043863243`, `test (macos-latest, 5 of 5)` | a GitHub-hosted `macos-latest` runner | 2026-11-22 | #399 | `144485c7` |

Read from the raw job log (`evidence/job-137043863243.log`, SHA256
`0af2a1e08c691b0d0bcaf3a79fdb3439eb182c0df9d50b837a4ed35e96bf0167`):

- Line 670: `test agent::proc::tests::terminal_interrupt_kills_the_isolated_tree ... FAILED`.
- Line 2534: the panic, at `src/agent/proc/tests.rs:2823:5`, reads
  `signal 2 terminated Upstroke but left its isolated agent tree alive`. Signal 2 is `SIGINT`.
- Line 2541: `test result: FAILED. 1994 passed; 1 failed; 66 ignored; 0 measured; 1944 filtered
  out; finished in 346.87s`. This test was the job's only failure.
- The `agent::proc::tests` lines start at line 608, before the failure. #399's only parts in this
  leg are the two R-O2 parts. They ended about four minutes later, at lines 2444-2445. Libtest
  starts tests in name order, and the lines it prints are completions. So the log shows the order
  in which tests finished. It does not show which tests were running at the same time as this one.

#399 touches no path under `src/agent/` (ruling r36). The same tree passed in the next run. CI run
`44975798003` is on `fe4bfa4e`, an empty commit on `144485c7` (`git diff fe4bfa4e 144485c7` is
empty). Its job `137065294769`, `test (macos-latest, 5 of 5)`, succeeded.

The test source was read at `fe2b9bc5`, where the assertion is at line 2823, the line and column
the panic names.

## What is and is not established

**Established.** In one job on a hosted macOS runner, Upstroke exited after a `SIGINT` to its
process group, and 1.3 s later the worker's marker existed. The test failed on that assertion, and
it was the job's only failure. #399 does not touch `src/agent/`, and the same tree passed on the
next run.

**Not established: why the marker existed.** There are two readings. Both are inferred, read from
the source, and not run. The log does not tell them apart.

1. **A live tree.** The interrupt terminated Upstroke, but the kill of the isolated tree that it
   should cause did not reach the worker. The worker outlived Upstroke, its timer ran out, and it
   wrote the marker during the test's 1.3 s sleep. That is a real defect in the supervisor's
   handling of a terminal interrupt.
2. **The worker's timer ran out first.** The worker's one-second timer starts before the shell's
   two `ps` calls and its `ready` file (`:2458-2462`). Before the interrupt can land, the shell must
   fork `ps` and `tr` twice and write `ready`. The test must then see `ready` on its 20 ms poll and
   send the signal, and Upstroke must handle it and kill the group. If all of that took more than
   one second on a loaded runner, the worker wrote the marker before any signal was sent. The tree's
   kill may then have worked, and the test failed on its own timing. This job's shard finished in
   346.87 s. Nothing in the log measures how long the shell took to become ready.

**What would establish each.** The failure would print what of the agent tree was still alive,
meaning the processes still in the supervised group (`supervised_pgid`) when the assertion ran. It
would also print when the marker was written relative to the signal. A live worker or shell, or a
marker written after the signal, is the first reading. An empty group and a marker written before
the signal is the second. The test records neither today.

**Not established: the rate.** One sighting. Other runs of this test were not counted.

## Related, not the same

- **`PR320-R7-MACOS-SIGTTOU-STRANDED`**
  (`findings/P3_portability_202609241645_sigttou-left-the-supervised-tree-stranded-on-macos.md`)
  also fails on native macOS in the same signal-helper harness. It is a different test,
  `terminal_input_and_output_stops_cover_the_isolated_tree`, covering job-control stops
  (`SIGTTIN`/`SIGTTOU`) and `SIGCONT`. Its assertion is also different: the helper did not exit
  within 10 s of `SIGCONT`, "left the supervised tree stranded". Here Upstroke exited promptly after
  a terminating signal, and what failed was the check that the tree was gone afterwards.
- **`PR281-MACOS-KILL-TREE-SETTLE-ONLY-THE-DIRECT-CHILD`**
  (`findings/P2_correctness_202609131200_the-macos-kill-tree-settle-fingerprint.md`) records
  `kill_tree` returning on macOS while a member of the child's process group was still running. It
  is a different test, `kill_tree_settles_the_whole_unix_group_before_it_returns`, with a different
  assertion, and it calls `kill_tree` directly. This test sends a terminal signal to Upstroke and
  observes the tree only through a marker file. If the first reading above holds, the two may share
  a mechanism. For this file, nobody traced whether Upstroke's interrupt path goes through the same
  group settlement, and one sighting cannot show it. It is named here as a relative, not as a
  parent.

Both ids were found as written.

## What the change that takes this up should do

- **Make the failure report what was alive and when the marker was written**, relative to the
  interrupt. Then decide the reading from the next sighting.
- **If the tree was alive**, fix the kill of the isolated tree on a terminal interrupt. The fix is in
  production code, and severity should be reconsidered then.
- **If the timer ran out first**, move the worker's timer after the `ready` file, so that the
  marker cannot be written before the signal is sent.
- **Keep the assertion** that the marker does not exist after the interrupt. Do not retry or skip
  the test, and do not weaken the assertion, to get past this.

**A proposal, not adopted and not reviewed:** the failure message names the processes still in the
supervised group when the assertion runs, and the marker's write time. The test would note the time
of its `kill(-pid, signal)` and read the marker's modification time. It would list the group's
remaining members, for example with `ps -o pid=,stat=,command= -g <supervised_pgid>`, before the
helper is completed. The message would then give both times and that list. There would be no retry,
no skip and no weaker assertion.
