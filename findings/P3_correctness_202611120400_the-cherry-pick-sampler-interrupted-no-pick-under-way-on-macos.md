---
id: CI-FLAKE-CHERRY-PICK-SAMPLER-NO-PICK-UNDER-WAY
severity: P3
disposition: deferred
category: correctness
pr: TBD
reviewed_sha: 47986d013001373cec29f530f75e14f26671e578
location: src/engine/topology/recover/tests.rs:11635
provenance: undetermined
first_bad:
guard: the change that makes the sampler's requirement that at least one kill lands while the pick is writing hold on a slow or unevenly scheduled machine, or replaces the timing sample with a deterministic interruption, keeping the requirement itself
---

# The cherry-pick sampler's kills all missed the pick on macOS

## Failure sequence

`engine::topology::recover::tests::sampled_cherry_pick_child_kills_every_residue_classified_and_recovered`
(`src/engine/topology/recover/tests.rs`, line 11459 at the reviewed commit) spawns a child that
makes a cherry-pick, measures how long the pick takes, and kills later children at delays aimed
inside that time. Among its assertions is that at least one kill landed while the pick was writing
— `killed_while_writing >= 1`, line 11635, added by commit `9f7aafc1` on 2026-09-10. **That is the
assertion that failed.**

**One sighting**, read from the job's log:

| run | job | platform | date | pull request | head |
|---|---|---|---|---|---|
| `43118198459` | `130851553875` | `test (macos-latest)` | 2026-11-11 | #368 | `c43e0ed6` |

The change at that head does not touch this file. The suite's result was **3645 passed, 1 failed,
168 ignored**.

The panic was at `src/engine/topology/recover/tests.rs:11634:5`, with the message beginning:

> none of the 14 kills in 16 spawns landed while the pick was writing — each found a child that
> had not begun (`None`, nothing written) or one that had published its commit (`After`) — so the
> sample interrupted no pick under way

It went on to say the probe measured **19.443125ms** and the ladder followed **2 completions**.

**What the sixteen spawns observed:**

- **thirteen** were killed before they had begun — nothing written (`None`);
- **one** was killed after publishing its commit (`After`);
- **two** completed (exit 0).

In the timeline, one run outlived its kill, which failed at 18.64ms with *"Operation not permitted
(os error 1)"*, and one run completed in 19.872667ms; the other fourteen were killed.

**Rate.** This failure appeared in **1 of the 62** `test (macos-latest)` jobs that completed among
the 86 CI runs created on 2026-11-11 UTC (50 succeeded, 12 failed, 24 were cancelled). A second
sighting, on Linux, is recorded below.

**Why this is its own row.** The nearest relative is the open finding
`findings/P2_correctness_202609131201_the-recover-sampler-cold-probe-sibling.md`, which records a
failure of the same test on the same platform. That is a different assertion — *"no sample died by
the kill"* — and that finding's own guard says a sighting after commit `f837f4ca` is a new
fingerprint. `f837f4ca` is an ancestor of the failing head `c43e0ed6`. So this is filed on its own,
with that finding named as its nearest relative and not as its parent.

**The cause is not established.** What the record shows is that every kill landed either before the
child began writing or after it had finished, on a machine where the pick took about twenty
milliseconds.

## A second sighting, on Linux, 2026-11-22

The same test failed on the same assertion, `killed_while_writing >= 1`, on a GitHub-hosted
`ubuntu-latest` runner:

| run | job | platform | date | pull request | head |
|---|---|---|---|---|---|
| `45058469607` (`merge_group`) | `137318868062` | `test (ubuntu-latest, 1 of 2)` | 2026-11-22, 19:02:40Z | #404 | `810f525f` |

- **Log.** `evidence/job-45058469607-137318868062.log`, SHA-256
  `744665897fbb5a35982d91700ad0fcfd3b653085728a115a8f36f0c400ac006c`.
- **Line 1774:**

      test engine::topology::recover::tests::sampled_cherry_pick_child_kills_every_residue_classified_and_recovered ... FAILED

- **Line 1971:** the panic at `src/engine/topology/recover/tests.rs:11861:5`, the same
  `killed_while_writing >= 1` assertion (line 11861 at the tested commit).
- **Line 1972:** `none of the 6 kills in 16 spawns landed while the pick was writing`; the probe
  measured **2.205111ms**, and the ladder followed **10 completions**.
- **What the sixteen spawns observed:** **three** were killed before they had begun (`None`, exit by
  signal 9); **three** were killed after publishing their commit (`After`, signal 9); **ten**
  completed (exit 0), each recorded as having "outran the kill".
- **The kills' timeline, as the payload prints it:** kills were aimed between 245.012µs and
  2.076647ms; the kills that returned did so between 386.375µs and 3.811152ms; ten of sixteen
  children were recorded as having outrun their kill, and the parent saw them exited ("exited by")
  at times between 2.215365ms and 3.821434ms. Those times are taken after `wait()` returns, so they
  bound each exit from above and do not measure it: run 11's kill returned at 2.205358ms and its
  child had "exited by 2.215365ms" (job 137318868062 of run 45058469607).
- **Line 1978:** `test result: FAILED. 1418 passed; 1 failed; 86 ignored`. It was the job's only
  failure. #404 changes only `findings/`.
- **The same tree passed.** Merge-queue run `45061116652` tested `e215c508` and merge-queue run
  `45064602211` tested `2c516b64`, both with the tree of `810f525f` (`c5b41056`); in both,
  `test (ubuntu-latest, 1 of 2)` succeeded. `45064602211` merged #404.

**Established.** The same assertion failed on a hosted Linux runner, where the probe measured about
2.2 ms against the first sighting's 19.4 ms on macOS, and the same tree passed twice after.

The file's name and title say macOS; this sighting is on Linux.

## A third sighting, on macOS, on master's own push run, 2026-11-28

| run | job | platform | date | pull request | checkout |
|---|---|---|---|---|---|
| `46139491098` (`push` to `master`), attempt 1 | `140962038951`, `test (macos-latest, 12 of 12)` | a GitHub-hosted `macos-latest` runner, image `macos-26-arm64` version `20261123.0526.1` (log lines 17-18) | 2026-11-28, 11:21:25Z to 11:29:20Z (the job's record; 7 min 55 s, inside the leg's 30-minute limit, `.github/workflows/ci.yml:281`) | none: the push of `b6d4879b`, the merge of #445 | `b6d4879b835ada30c133b59f1fee0089cc64d2b6` (log line 111) |

Read from the job's log as GitHub's API serves it, with ANSI escape sequences removed
(`~/selfhost-findings-20261128b/evidence/job-46139491098-140962038951.log`, SHA-256
`1f581f6176f7bc5660f963145d2a18f0927aab246fbe95f52a04fdca99007f9a`):

- Line 2088:
  `test engine::topology::recover::tests::sampled_cherry_pick_child_kills_every_residue_classified_and_recovered ... FAILED`.
  It is the only test line in the log that reads `... FAILED`.
- Line 2973: the panic at `src/engine/topology/recover/tests.rs:11902:5`. Line 2974 gives the
  message, which begins:

  > none of the 7 kills in 16 spawns landed while the pick was writing — each found a child that had
  > not begun (`None`, nothing written) or one that had published its commit (`After`) — so the
  > sample interrupted no pick under way

- It goes on: the probe measured **9.96675ms**, and the ladder followed **9 completion(s)**.
- Its list of the sixteen spawns' ends: **seven** `(None, ExitStatus(unix_wait_status(9)))` and
  **nine** `(After, ExitStatus(unix_wait_status(0)))`. So seven children were killed before they had
  begun (`None`, signal 9), in the message's own reading of `None`, and nine completed (`After`,
  exit 0).
- Its ladder: runs 0 to 2 and 8 to 11 read "killed, the kill returned at", at times from 1.094917ms
  to 4.32975ms, for aims from 1.077903ms to 4.311611ms; runs 3 to 7 and 12 to 15 read "completed
  in", at times from 8.799625ms to 10.968083ms.
- Line 2980:
  `test result: FAILED. 2427 passed; 1 failed; 107 ignored; 0 measured; 1732 filtered out; finished in 266.91s`.

Read from the run's attempt-1 job list as GitHub's API serves it, not from the log
(`~/selfhost-findings-20261128b/evidence/job-records/run-46139491098-attempt-1-jobs.json`, SHA-256
`a0a2b90e869a2eb6cda881990a51fd4c420f4bae1e124c27e255d76d4fa4492e`): 44 of the run's 46 jobs
succeeded, and the two failures were this job and the aggregate `upstroke-ci` (job `140964183595`).
The run's record, fetched at 15:40Z on 2026-11-28, gives `run_attempt` 1.

**What the test asserts**, by line at `b6d4879b`. The test starts at
`src/engine/topology/recover/tests.rs:11727`. It samples eight spawns, and eight more when no kill
in the first eight landed while the pick was writing (`:11730-11734`, `:11875-11877`). A kill counts
as landing while the pick was writing when the child died by the kill and its residue class is
`Internal` (`:11842-11843`). Two assertions end the test: `killed_while_running >= 1`, "no sample
died by the kill" (`:11890-11901`), and `killed_while_writing >= 1` (`:11902-11911`), the one that
failed; the first runs before the second, so it passed. `src/engine/topology/recover/` is the same
at `b6d4879b` and at `master` `d431d220`.

**What the pushed commit changed.** `b6d4879b` is the merge of #445, whose diff from `master`
`6513c22d` changes only files under `findings/` (`git diff --name-only 6513c22d b6d4879b`).

**The same commit passed.** Run `46138654089`, the merge-queue run that tested #445 (`merge_group`),
checked out `b6d4879b835ada30c133b59f1fee0089cc64d2b6` itself (line 111 of its job's log): job
`140959498813`, `test (macos-latest, 12 of 12)`, 11:10:59Z to 11:20:42Z, on image version
`20261123.0526.1` (line 18). Line 1972 logs the test `... ok`, and line 2972 reads
`test result: ok. 2428 passed; 0 failed; 107 ignored; 0 measured; 1732 filtered out; finished in 343.81s`.
Its log: `~/selfhost-findings-20261128b/evidence/job-46138654089-140959498813.log`, SHA-256
`0b9f2771e300927650cab7f60b541f1730b6da235289a1d1faa3316909647993`.

**What is and is not established for this sighting.**

- **Established:** on a hosted macOS runner, at `master`'s `b6d4879b`, each of the test's seven
  kills ended its child by signal 9 with its residue `None`, and the `killed_while_writing >= 1`
  assertion failed; the same commit passed the test in the merge-queue run `46138654089`.
- The log does not show when each child began writing, or when each killed child died relative to
  its kill's return.
- **Not established:** the cause.
- **Not established: the rate.**
- `PR281-RECOVER-SAMPLER-NO-SAMPLE-DIED-BY-THE-KILL`
  (`findings/P2_correctness_202609131201_the-recover-sampler-cold-probe-sibling.md:16-19`) records
  this test failing the other assertion, "no sample died by the kill".

## What the change that takes this up should do

- **Establish from further sightings which way the kills are missing:** whether they land early
  because the children start slowly, or late. One sighting does not settle it.
- **Make the requirement hold without depending on the machine's timing, or interrupt the pick
  deterministically.** Either is acceptable; what is not is a schedule that holds only where the
  runner is fast and evenly scheduled.
- **Keep the requirement that a pick under way is interrupted.** Do not weaken
  `killed_while_writing >= 1`, and do not retry or skip the test to get past it.
