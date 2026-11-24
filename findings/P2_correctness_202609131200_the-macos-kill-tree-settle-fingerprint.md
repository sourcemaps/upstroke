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

## What the change that takes this up should do

Read the settle predicate at `src/agent/proc/tests.rs:879` against what `kill_tree` actually
guarantees on Darwin: whether the test's witness (a member still holding its pipes) can be observed
after a `kill_tree` that did everything correctly, or whether `kill_tree` genuinely returns before
the group is reaped. Decide which, then either bound the witness or fix the settlement — and do not
close this row by repairing a different member of the kill/settle family.
