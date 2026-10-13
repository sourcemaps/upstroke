---
id: PR339-CONTAINER-REAPER-TEST-RECLAIMS-BEFORE-THE-REAPERS-LAST-WRITE
severity: P3
disposition: deferred
category: correctness
pr: 339
reviewed_sha: b6aa70c22d73159f73121d026db47eb0dbb022db
location: src/agent/proc/tests.rs:3444
provenance: pre_existing
first_bad: 0af403767f6855708fb0c0336f7a4b7524a1b9c5
guard: this change, branch fix-P3/correctness_the-container-reaper-test-reclaims-its-tree-before-the-reapers-last-write, which files this finding and repairs it in the same pull request
---

## Failure sequence

**The test's unjoined-writer lifetime defect.** In a dying cell (`bare-dies` or `path-dies`) of
`agent::proc::tests::unix_reaper_kills_labeled_containers`, the cell's scratch tree is reclaimed while a writer the
cell's reaper started can still write into it. Lines are at `b6aa70c2`; every blob they are read from is the same at
the diagnostic base `26762c18` and at the CI checkout `27cd1083`.

1. **The test kills the coordinator.** It SIGKILLs and reaps the disposable coordinator
   (`src/agent/proc/tests.rs:3424-3425`). The coordinator's cleanup reaper, which runs in its own process group
   (`setpgid(0, 0)`, `src/agent/proc.rs:2643`), settles the dead coordinator: it kills the agent group, then
   `reclaim_labeled_containers` (`src/agent/proc.rs:4977-5024`) runs the tree's recording `docker` stub as `ps`,
   `kill` (which sleeps one second), `rm --force --volumes`, and `ps` again. The last listing happens because the round
   settled one id; an empty listing is what ends the loop. Every stub call first appends its argv to `<tree>/argv.log`
   with `>>`, and `rm` then creates `<tree>/removed`.
2. **The test reclaims after three lines.** It polls `argv.log` every 20 ms until it holds three lines (`:3437-3448`;
   the decision is `:3444`), asserts them, finds the agent gone, and ends the iteration (`:3508`). The cell's
   `ScratchTree` guard then reclaims the tree with `fs::remove_dir_all` (`src/rundir/scratch_tree.rs:334-335`).
3. **Nothing waits for the reaper or the stubs it started.** Two writes can still land in the tree: W1, the `rm`
   stub's `removed`, the command after its log line; and W2, the second listing's `>>`, which re-creates `argv.log` if
   the reclaim already unlinked it. A write that lands between `remove_dir_all`'s enumeration and its removal of the
   root makes that removal fail with `ENOTEMPTY`; `remove_dir_all` does not retry.
4. **The guard panics on the normal path.** Its non-unwinding arm reports `scratch tree <tree> was not reclaimed:
   Directory not empty (os error N)` (`src/rundir/scratch_tree.rs:761`), after every assertion of the cell has passed,
   and the library run ends `FAILED`, so the required `upstroke-ci` context goes red on whatever head ran it.

### Evidence, by level

- **The associated sighting** [OBSERVED, the raw log only]: CI run 38319402697 (`pull_request`, attempt 1), job
  115043656125 `test (macos-latest)`, image `macos-26-arm64` 20260907.0351.1, `rustc 1.99.0`, checkout
  `27cd108329e366e5a2777af7d42d9c7d4e739425`. `test agent::proc::tests::unix_reaper_kills_labeled_containers ...
  FAILED`; the panic at `src/rundir/scratch_tree.rs:761:21`, "scratch tree
  …/upstroke-path-dies-NAA7YBDVZ0 was not reclaimed: Directory not empty (os error 66)"; `FAILED. 3178 passed; 1
  failed; 105 ignored`. The raw log's sha256 is `ce892d9933fd812efb95ba8620e3f3b62f92b4d1337fd87a6afc2a10220e8368`.
  **Its exact cause is unresolved.** The log lists neither the leftover tree nor any timing, so which write landed,
  if either did, is not established. A macOS enumeration gap with no writer, and a writer outside the fixture, are not
  excluded by observation, only reasoned unlikely. A later red with the same fingerprint is a sighting; identity alone
  does not establish its cause.
- **The writer ordering at the diagnostic base** [REPRODUCED ON LINUX, as sampled ordering only]: a diagnostic probe,
  never committed (`pr11_scratch_diag1`'s control C1, package sha256 `6ae67cd6…ce50`, on the build box), replayed the
  `path-dies` cell three times at `26762c18` and sampled about every millisecond. The reaper was alive when the third
  line first appeared, and the second listing's line followed 9790-9933 µs later. `removed` was co-sampled with the
  third line, so W1's interval was not observed; a co-sampled interval cannot disprove a later writer. P-d in its third
  round was co-sampled, not a measured strict order. It did not reproduce the ENOTEMPTY and did not capture the macOS
  cleanup cause, and its samples do not identify the test's own cleanup instant, which the test's 20 ms poll decides.
  No rate is established, on any platform, and no indirect effect of #339 is excluded.
- **The attribution of steps 1-4** [REASONED]: the panic is the guard's non-unwinding arm; the error is the remover's
  `ENOTEMPTY`; within the analysed fixture paths (the test thread, the coordinator, the reaper, the agent and the stub)
  W1 and W2 are the only writers after the three-line wait.

## What the change that takes this up should do

Make each dying cell establish that the writers its reaper started have completed before the cell's tree is
reclaimed, and fail explicitly, naming what it waited for, if they have not within a bound. Change nothing else: the
test's name (`src/effects/tests/contract_mappings.rs` maps ST-16 (d) to `unix_reaper_kills_labeled_containers` by
name), the cleanup and reaper property it tests, and every substantive assertion stay as they are. The reaper's end
(`alive(reaper_pid)`, which the test already applies to the agent) and the end of the reaper's process group are both
candidates; whichever the change takes, it states the identity and lifecycle argument that makes its end imply the
writers' end. Add a regression witness that catches the old ordering deterministically, by construction rather than
by timing, and passes after the change. Not a repair: a sleep, a retry, an ignored or downgraded cleanup failure, a
skip, a raised existing timeout, a weaker assertion, suppressed output, or any edit to the production reaper,
`src/rundir/scratch_tree.rs` (whose reclaim stays a hard failure, as the fixed `PR64-CLEANUP-003-RECLAIM-SILENCE` made
it), the effects or contract mappings, CI or the runner.

## Why it is P3

The defect lives in test code; in the sighting every assertion of the cell had passed, and no product behaviour,
durable state or user repository is involved. The cost is an intermittent red of the required `upstroke-ci` context
on an unrelated head. `PR274-REAPER-EXIT-ASSERTION-RACES-THE-PARENTS-SIGKILL` and `PR172-REAPER-ROUNDS-STUB-NEVER-RAN`,
races in the same module's tests, are P3. It meets none of `MAINTAINING.md`'s serious-P1 conditions. The
classification is proposed and subject to review.

## Provenance and first bad

**`pre_existing`** [HISTORY-DERIVED]: the failure sequence's code is byte-identical at #339's base `26762c18`, at the
CI checkout `27cd1083` and at `b6aa70c2`, and has been on master since `daa09b1e` (PR #322). #339's net diff
(`.github/workflows/ci.yml`'s `merge_group` timeout and two `src/effects/tests/workflow.rs` escape strings) reaches none
of steps 1-4 [REASONED]; no indirect effect is excluded, and `pre_existing` classifies the derived defect, not the
sighting's cause.

**`first_bad` `0af403767f6855708fb0c0336f7a4b7524a1b9c5`** (2026-09-26, #322 round 1, "test: drop the raw removals of
a guard's own root before the guard reclaims it"): history-derived and conditional on the derived mechanism; no
failing run dates it, and none was executed at it. Read from history: the unjoined writer itself dates from the
test's arrival at `919a728e`, where the same three-line wait was followed by a discarded `remove_dir_all`, so a hit
leaked a directory silently; from `423f40de` to `a8c8da46` that discarded removal preceded the guard's reclaim, so a
failure needed a hit on both; from `0af40376` the guard's reclaim is the tree's only removal, so one hit fails the test.

**Related, not the same:** `PR7-SCRATCH-FIXTURE-LEAK` (closed), whose fix made this race loud;
`PR328-REAPER-DOCKER-WAIT-HAS-NO-DEADLINE-AFTER-SIGKILL`, the same reaper path's production liveness defect.

## Deduplication

Refreshed at `b6aa70c2` over the orchestrator's screened lists only: the 400 open findings (9 excluded unread), the
132 closed findings as `git show <commit>^:<path>` (6 excluded unread), and `reviews/FINDINGS.md`, with 22
case-insensitive patterns (the test and helper names, the message, the cell tags, `ENOTEMPTY`, the stub, the reaper's
functions, the guard, the run, job and tree suffix, and this id). No record of this fingerprint or this race was found;
every hit is another defect. `PR274` is a different test, site and message, and
`CLASS-INTERMITTENT-SUBPROCESS-KILL-SETTLE-RESIDUE-FAILURES` limits itself to four named fingerprints, none of them
this one. No match in these searches does not mean none exists anywhere: excluded records, pull request bodies,
review records and CI history were not searched.
