---
id: PR281-MACOS-KILL-TREE-SETTLE-ONLY-THE-DIRECT-CHILD
severity: P2
disposition: deferred
category: correctness
pr: 281
reviewed_sha: 2467df322b17ba8ee93eb345e1a19367c0b75e37
location: src/agent/proc/tests.rs:879
provenance: pre_existing
first_bad:
guard: the project owner / the round that next opens `kill_tree`'s Unix group settlement. Filed as its own fingerprint rather than as a member of `CLASS-INTERMITTENT-SUBPROCESS-KILL-SETTLE-RESIDUE-FAILURES`: that row names four members and this is none of them
---

## Failure sequence

`agent::proc::tests::kill_tree_settles_the_whole_unix_group_before_it_returns` fails intermittently
on macOS at `src/agent/proc/tests.rs:879:5` with *"kill_tree returned while a member of the child's
process group was still running and still holding its pipes: only the direct child was killed"*.
`kill_tree` returned before the whole process group had settled, so a caller that treats its return
as "the tree is gone" proceeds while a grandchild still holds the pipes.

**Four sightings between 2026-09-06 and 2026-09-13**, every one confirmed by its own `... FAILED`
line and its own panic in its own job log, all four at the identical assertion and identical
location. **One is on `master` itself.**

| run | job | date | branch | head |
|---|---|---|---|---|
| `34029807779` att. 1 | `101477152397` | 2026-09-06T11:17:22Z | `codex/findings-c0ad2fe0dd55` | `14032a25` |
| `34080563562` | `101614908628` | 2026-09-07T03:42:40Z | **`master`** | `25a9e189` |
| `34609680659` | `103296736013` | 2026-09-11T14:21:57Z | `findings/reclassify-the-p4-finding` | `63d149ee` |
| `34738739536` | `103674713617` | 2026-09-13T04:48:01Z | `fix-P1/security-trust_a-failed-sentinel-write-manufactures-a-matching-finding` | `339a238b` |

Derived by reading every failed and every cancelled `test (macos-latest)` job log in the
2026-09-06/13 window — 296 distinct logs — not by grepping for a mention.

**Why this is a row and not a member of an existing one.** `CLASS-INTERMITTENT-SUBPROCESS-KILL-SETTLE-RESIDUE-FAILURES`
limits itself to four named fingerprints — `W2-MACOS-HOST-CONTAINMENT-ROLE-GROUP`,
`PR104-WINDOWS-SETTLE-PATH-HINT`, `PR107-WINDOWS-SETTLE-REPLAY-ALREADYSTARTED`,
`PR107-LINUX-WORKSPACE-RESIDUE-EMPTY-GITDIR` — and this test is none of them; the macOS member is
`runner::host::tests::every_role_reaches_the_containment_points_of_this_platform` failing *"the
child did not lead its own process group"*, a different test and a different assertion, whose cause
was measured and repaired by #173. Family resemblance is exactly what that row's own text forbids
as an attribution.

**Its one prior appearance is recorded under a row that disclaims it.** `48f2af7a:reviews/FINDINGS.md`
§`W1-MACOS-PROC-LATE-REAPER-SELF-SIGTERM` lists this test twice (#97 run `33674393240` att. 1,
#104 run `33741105025` att. 1) and marks both *"an assertion flake, harness alive"* — explicitly
**not** that row's self-kill mechanism, which is what that row was about and what its repair
addressed. That row is closed. So these sightings have had no home since.

## Two more sightings, 2026-11-22 and 2026-11-24

| run | job | date | branch | head |
|---|---|---|---|---|
| `45061116652` | `137327046665` | 2026-11-22T19:30:20Z | `gh-readonly-queue/master/pr-404-…` | `e215c508` |
| `45309476256` | `138161315576` | 2026-11-24T02:02:51Z | `ci/macos-checks-cross-compile-on-linux` | `7110364a` (checkout `89ed1a42`) |

Both ran on GitHub-hosted `macos-latest` runners, in `test (macos-latest, 5 of 5)`. Each is the same
test with the same message as the four recorded above, now at `src/agent/proc/tests.rs:1263:5`: the
assertion moved from line 879 as the file grew, and it is the same `assert!(settled, …)`. In each, it
is the job's only failure.

- **Run `45061116652`** (`merge_group`, #404), job `137327046665`, 2026-11-22T19:30:20Z, branch
  `gh-readonly-queue/master/pr-404-…`, head `e215c508`. Log
  `evidence/job-45061116652-137327046665.log`, ANSI-stripped SHA-256
  `7c645be14209832ac1a5f7bc02e8c9c65b098cf7fde7ccd2ae70f90d2594401f`. Line 667: `... FAILED`.
  Line 2532: the panic at `src/agent/proc/tests.rs:1263:5`, *"kill_tree returned while a member of
  the child's process group was still running and still holding its pipes: only the direct child
  was killed"*. Line 2539: `1992 passed; 1 failed; 66 ignored`. #404 changes only `findings/`.
  **Same tree's pass:** merge-queue run `45064602211` tested `2c516b64`, whose tree is `e215c508`'s
  (`c5b41056`), and passed; it merged #404.
- **Run `45309476256`** (`pull_request`, #412 at `7110364a`), job `138161315576`,
  2026-11-24T02:02:51Z, branch `ci/macos-checks-cross-compile-on-linux`, checkout `89ed1a42`
  (GitHub's merge of `7110364a` into `master` `4c9c566b`). Log
  `evidence/job-45309476256-138161315576.log`, ANSI-stripped SHA-256
  `3da9a1c111993fee7f993cad18a775bda4c7e76025a40fd22f695bb90ac3ae60`. Line 668: `... FAILED`.
  Line 2551: the same panic. Line 2558: `2011 passed; 1 failed; 66 ignored`. #412 changes no file
  under `src/agent/`. The test passed in the next run, `45316622352`, on `7163640a` (an empty commit
  on `7110364a`), which checked out `678f8f6f`, a merge into a later `master` (`ead85451`, which had
  merged a docs pull request). That is **not the same tree**.
- **Established:** the same assertion failed twice more on hosted macOS runners, on changes that do
  not reach `kill_tree`, and the test passed in the next run each time.

## One more sighting, 2026-11-24

| run | job | date | branch | head |
|---|---|---|---|---|
| `45491842626` | `138791929214` | 2026-11-24T21:30:48Z | `findings/three-ci-flakes-sighted-2026-11-22-to-24` | `fd7c7324` (checkout `631a052`) |

- **Run `45491842626`** (`pull_request`, #415 at `fd7c7324`), job `138791929214`,
  `test (macos-latest, 5 of 5)`, on a GitHub-hosted `macos-latest` runner. It was started by an edit
  of #415's body after #415 entered the merge queue (21:13:44Z). #415 merged from the queue at
  21:33:21Z, so this run gated nothing, and its red was not seen until 2026-11-25.
- **The log.** `evidence/job-45491842626-138791929214.log`, ANSI-stripped SHA-256
  `a7946e756c2f88eb98a75e522e5437b3409c27e2cdcf6c0da74671914d34f479`. Line 107:
  `HEAD is now at 631a052 Merge fd7c7324936aefb16a35cc943608b04e98c67853 into 81bb1ab2fcb49350fc39939ecd0d72971cf892ac`,
  GitHub's merge of `fd7c7324` into `master` `81bb1ab2`. Line 671:
  `test agent::proc::tests::kill_tree_settles_the_whole_unix_group_before_it_returns ... FAILED`.
  Lines 3013-3014: the panic at `src/agent/proc/tests.rs:1263:5`, *"kill_tree returned while a
  member of the child's process group was still running and still holding its pipes: only the
  direct child was killed"*. Line 3020: `test result: FAILED. 2425 passed; 1 failed; 115 ignored`.
  Apart from the aggregate `upstroke-ci`, this was the run's only failed job, and this test was the
  job's only failure.
- **What #415 changed:** only files under `findings/`, five of them.
- **Same tree's pass.** Run `45486440086` (20:59:19Z, `pull_request`, the same head `fd7c7324`),
  job `138772988997`, checked out the same merge commit `631a052` (line 107 of
  `evidence/job-45486440086-138772988997.log`, SHA-256
  `1dd4b47b8b837d945ef965df3cab30c7742dfbe6e4b146afcf71e6e755284f12`), and the test passed
  (line 653). The merge-queue run `45488991284`, job `138781911396`, also passed it (line 653 of
  `evidence/job-45488991284-138781911396.log`, SHA-256
  `28308b01e6885327b2c96f4536642c109949498f2572216751adfe7cc8533312`).
- **Established:** the same assertion failed once more on hosted macOS, on a change confined to
  `findings/`, and passed on the same merge commit 31 minutes earlier.

## A sighting on #439, 2026-11-26

| run | job | date | branch | head |
|---|---|---|---|---|
| `45814313794` | `139890070568` | 2026-11-26T11:51:46Z | `ci/test-legs-named-by-category` | `f8d610f3` (checkout `88f6876`) |

- **Run `45814313794`** (`pull_request`, #439 at `f8d610f3`), attempt 1, job `139890070568`,
  `test (macos-latest, unit (agent-adapters, agent-proc, cli, config, model, plan, run-state, util))`,
  on a GitHub-hosted `macos-latest` runner, image `macos-26-arm64` version `20261117.0516.1`
  (log lines 17-18), 11:51:46Z to 11:57:41Z (the job's record; inside the leg's 30-minute limit,
  `.github/workflows/ci.yml:266` at `88f6876`).
- **The log.** `~/selfhost-findings-20261128b/evidence/job-45814313794-139890070568.log`,
  ANSI-stripped SHA-256 `44375bbdb10d2f6016e5ef8d56cb1f31cd98d5c43e795712d5e76cfd6ba0589f`,
  the bytes of the copy saved on 2026-11-26
  (`~/selfhost-catshards-20261125/evidence/r76-ci-45814313794/139890070568.plain.log`; `cmp`).
  Line 107:
  `HEAD is now at 88f6876 Merge f8d610f378c3ab2f0f85fa575a36f74cd33cb46b into 552c039596a4a8a4ef3d5b4f514c46d206038311`,
  GitHub's merge of `f8d610f3` into `master` `552c0395`. Line 692:
  `test agent::proc::tests::kill_tree_settles_the_whole_unix_group_before_it_returns ... FAILED`,
  the only test line in the log that reads `... FAILED`. Lines 2098-2099: the panic at
  `src/agent/proc/tests.rs:1263:5`, *"kill_tree returned while a member of the child's process group
  was still running and still holding its pipes: only the direct child was killed"*. Line 2105:
  `test result: FAILED. 1629 passed; 1 failed; 31 ignored; 0 measured; 2602 filtered out; finished in 55.03s`.
- **From the run's attempt-1 job list**, not the log
  (`~/selfhost-findings-20261128b/evidence/job-records/run-45814313794-attempt-1-jobs.json`, SHA-256
  `995771197d67613b3d5819140c0cd8933fa33e7ca2d6dc490889af5913986519`): 41 of the run's 43 jobs
  succeeded, and the two failures were this job and the aggregate `upstroke-ci`
  (job `139896770625`).
- **What the test asserts, at `88f6876`:** `assert!(settled, …)` at `src/agent/proc/tests.rs:1263`,
  in the test at line 1192, the assertion this file records. `src/agent/` at `88f6876` is
  `552c0395`'s, and `git diff --name-only 552c0395 88f6876 -- src/agent/ src/topology/` prints
  nothing.
- **Same tree's pass.** Run `45822510547` (`pull_request`, the same branch at `1555832f`, an empty
  commit on `f8d610f3` with its tree), attempt 1, job `139918209456`, the same leg, 12:44:55Z to
  12:50:51Z, on image version `20261117.0516.1` (line 18), checked out `adf1ed8` (line 107:
  `HEAD is now at adf1ed8 Merge 1555832f9a32f2b55fb57d9a86657b9fb31b0718 into 552c039596a4a8a4ef3d5b4f514c46d206038311`),
  whose tree is `88f6876`'s (`db662546`). Line 677 logs the test `... ok`, and line 2094 reads
  `test result: ok. 1630 passed; 0 failed; 31 ignored; 0 measured; 2602 filtered out; finished in 49.07s`.
  Its log: `~/selfhost-findings-20261128b/evidence/job-45822510547-139918209456.log`, SHA-256
  `683fc902b11a689455c9edbe39ab7fcc978631f8db48636d45f8473d3553fab6`, fetched for this record.
  That run's attempt-1 job list
  (`~/selfhost-findings-20261128b/evidence/job-records/run-45822510547-attempt-1-jobs.json`, SHA-256
  `3c33a72d117550e350a6eec6913e7bbcce7551c9c9c8d26c2f263a0089084bde`) has two other failures,
  `test (windows-latest, self-checks (census, effects))` (job `139918209376`) and `upstroke-ci`
  (job `139925756965`), which its attempt 2 re-ran under ruling r80.
- **Established:** the same assertion failed once more on hosted macOS, on a change with no file
  under `src/agent/` or `src/topology/`, and the same tree passed it in run `45822510547`, on an
  empty commit.
- **Not established:** which process held the pipes' writer, or the errno of the last read
  (ruling r78 step 6).
- **Not established: the rate.**
- `CI-FLAKE-MACOS-TERMINAL-INTERRUPT-LEFT-THE-AGENT-TREE-ALIVE`
  (`findings/P3_correctness_202611220300_a-terminal-interrupt-left-the-agent-tree-alive-on-hosted-macos.md:123-127`)
  names this test, `kill_tree_settles_the_whole_unix_group_before_it_returns`, as a different test
  from its own, with a different assertion.
- `PR292-MACOS-ZOMBIE-ONLY-GROUP-EPERM-FAILS-KILL-TREE`
  (`findings/P3_portability_202609150527_macos-eperm-on-a-zombie-only-group-fails-the-kill.md:17`,
  `:56-58`) records a group of zombies refused a signal, `EPERM`, and says it is not this
  fingerprint, which it describes as a live grandchild still holding the pipes after `kill_tree`
  returns.

## What the change that takes this up should do

Read the settle predicate at `src/agent/proc/tests.rs:879` against what `kill_tree` actually
guarantees on Darwin: whether the test's witness (a member still holding its pipes) can be observed
after a `kill_tree` that did everything correctly, or whether `kill_tree` genuinely returns before
the group is reaped. Decide which, then either bound the witness or fix the settlement — and do not
close this row by repairing a different member of the kill/settle family.
