---
id: CI-FLAKE-WINDOWS-GATE-CONTROLS-FOUR-RUNS-UNDER-ONE-BOUND
severity: P3
disposition: deferred
category: liveness
pr: TBD
reviewed_sha: 681f429f8f3dba7c19ed248f69711b59a1fe17b1
location: src/engine/topology/coordinator.rs:25926
provenance: undetermined
first_bad:
guard: the change that gives each of the four F_MATRIX runs in a_genuinely_failing_gate_in_a_whole_snapshot_is_judged and a_gate_reading_a_whole_index_fails_on_the_forbidden_file its own bound, as f_bounded_case gives each case of their refusal siblings, with no retry, skip or weaker assertion, on branch fix-P3/liveness_two-gate-control-tests-ran-past-one-bound-on-hosted-windows; a_gate_launch_failing_in_a_whole_snapshot_is_the_outage_it_was (95.492 s in run 45542041793, one bound at src/engine/topology/coordinator.rs:26512) is the next nearest
---

# Two gate-control tests ran past their single 120 s bound on one hosted Windows leg

## Failure sequence

    the job's test harness runs the two tests with the shard's other tests
    -> each test's four `F_MATRIX` runs take longer than 120 s together
    -> `bounded` panics at `src\engine\topology\coordinator.rs:11345:64` for each
    -> the job fails

## What the tests assert

At `681f429f`, in `src/engine/topology/coordinator.rs`:

- `a_genuinely_failing_gate_in_a_whole_snapshot_is_judged` (line 25925) and
  `a_gate_reading_a_whole_index_fails_on_the_forbidden_file` (line 25989) each run all four entries of `F_MATRIX`
  (line 25886: `Attempt` at widths 1 and 3, `Verification` at widths 1 and 3) inside one call of `bounded` (line
  25926, named `f-c1-p2-gate-control`; line 25990, named `f-c1-p2-pass-control`).
- `bounded` (line 11330) runs its body on a thread. If the body has not finished within `BOUND` (line 2181,
  `Duration::from_secs(120)`), it panics at line 11345 with:

  > `<name>` did not end within 120s: the coordinator waited on its inbox for a pipeline that was itself waiting
  > on the coordinator

- Their refusal siblings bound each case on its own through `f_bounded_case` (line 25873).

## The sighting

| run | job | event | pull request | head | created |
|---|---|---|---|---|---|
| `45542041793` | `138962631949`, `test (windows-latest, 5 of 14)` | `pull_request` | #421 | `18d9206e` | 2026-11-25T03:44:28Z |

Read from the job log as GitHub's API serves it, with ANSI escape sequences removed, `evidence/job-45542041793-138962631949.log`, SHA-256
`83670a1823679c135c0c17d2c028d6f315d8ba062f033e9c051f90cde8814db4`:

- Line 109, the checkout:

      HEAD is now at 9603467 Merge 18d9206e2839fb7867d989c673331b428c4f8856 into 6abbe1c35a62dab4fd25db7d43d45390733c1947

- Line 434:

      test engine::topology::coordinator::tests::a_gate_reading_a_whole_index_fails_on_the_forbidden_file ... FAILED <120.009s>

- Line 439:

      test engine::topology::coordinator::tests::a_genuinely_failing_gate_in_a_whole_snapshot_is_judged ... FAILED <120.013s>

- Lines 457 and 458: the first test panicked at `src\engine\topology\coordinator.rs:11345:64` with
  `f-c1-p2-pass-control did not end within 120s: ...`.
- Lines 462 and 463: the second panicked at the same place with `f-c1-p2-gate-control did not end within 120s: ...`.
- Line 433: `a_gate_launch_failing_in_a_whole_snapshot_is_the_outage_it_was ... ok <95.492s>`.
- Line 470:

      test result: FAILED. 56 passed; 2 failed; 0 ignored; 0 measured; 3854 filtered out; finished in 483.81s

#421's diff from its base (`git diff --name-only 6abbe1c3...18d9206e`) is
`design/17_design_configuration_reference.md`, `docs/internals/capacity.md`, `docs/internals/config.md`,
`docs/internals/config/read.md`, `effects/wrappers.toml`, one file under `findings/`, `src/capacity.rs`,
`src/config.rs` and `src/tracker.rs`. No path under `src/engine/`.

## The same shard on other runs

Each run's log is `evidence/job-<run>-<job>.log` of the `test (windows-latest, 5 of 14)` job, and each line number
is the harness's `test result:` line.

| run | job | event, tree | harness | `a_gate_reading…` | `a_genuinely_failing…` |
|---|---|---|---|---|---|
| `45508617577` | `138850216965` | push, `master` `1724628d` | 217.39 s (line 430) | 46.024 s | 45.291 s |
| `45524607007` | `138904707125` | #421 `5ef9901d`, checkout `a0938b6` (merge into `1724628d`, line 109) | 227.15 s (line 446) | 45.251 s | 46.631 s |
| `45527518852` | `138914427654` | push, `master` `9ba051b8` | 279.46 s (line 431) | 55.035 s | 51.829 s |
| `45530063221` | `138922849446` | merge queue, `6abbe1c3` | 253.30 s (line 430) | 48.471 s | 47.811 s |
| `45531931928` | `138929119692` | push, `master` `6abbe1c3` | 402.56 s (line 437) | 80.885 s | 80.321 s |
| `45532828357` | `138932103886` | #421 `e1e44c84`, checkout `512af95` (merge into `6abbe1c3`, line 109) | 302.70 s (line 448) | 59.664 s | 83.058 s |
| `45542041793` | `138962631949` | #421 `18d9206e`, checkout `9603467` (merge into `6abbe1c3`, line 109) | 483.81 s (line 470) | FAILED 120.009 s | FAILED 120.013 s |

Each harness line on a green run reads `ok. 58 passed; 0 failed`.

The SHA-256 values of the other six logs:

| log | SHA-256 |
|---|---|
| `45508617577-138850216965` | `e6f70ce0d897c121abe7fb139429b7e4a9445e5a2e1b74416bc213e61312bc39` |
| `45524607007-138904707125` | `122f6bcaab1af874c8fcec54c797da61e76924bd41ed32ea5480697f05fd6bf3` |
| `45527518852-138914427654` | `184d09a286665db0181ac91ce5d3e748c47e212cd2df9f02f9a2fa6e29715d68` |
| `45530063221-138922849446` | `44fa6250c57ca888a9985a63ad61503cdb39bda61d61c2863406aa29d10015b0` |
| `45531931928-138929119692` | `ff8b95c665c479ef5a3d2a109f14921fe79b3e104f7f65e311e86ecbe01b8067` |
| `45532828357-138932103886` | `99249529abff32aee07be7a96e370ab112fb905524320167ee1f0d741db30e6a` |

## The same trees

Runs `45532828357` and `45542041793` each checked out a merge into `master` `6abbe1c3`. `git diff e1e44c84
18d9206e` changes three files, none under `src/engine/`: `design/17_design_configuration_reference.md`,
`src/capacity.rs` and `src/tracker.rs`. So the two tests ran on the same `src/engine/` code in both runs, and passed
in the first.

## What is and is not established

**Established.** Both tests ran past their single 120 s bound in that one job. The same job's harness took
483.81 s, against 217.39 to 402.56 s on the same 58 tests in the six other runs. The diff of #421 reaches no path
under `src/engine/`.

**Not established:**

- whether either scenario was still advancing when its bound passed (`bounded` reports nothing on timeout);
- the rate. One sighting, on a Windows layout that `master` has since replaced: #422 re-packed the Windows legs
  to 22.

**Readings.**

- *Inferred, not established:* the leg ran slow, and both scenarios were still advancing when the bound passed.
- *Inferred, not established:* one or both scenarios stopped advancing before the bound passed.

These readings are not exhaustive.

## Pointer

`CI-FLAKE-WINDOWS-INTERLEAVING-TIME-BOUNDS`
(`findings/P3_liveness_202611211601_two-interleaving-tests-ran-past-their-time-bounds-on-hosted-windows.md`) records
the same panic site, `src/engine/topology/coordinator.rs:11345`, for two other tests (its `location:` line).

## What the change that takes this up should do

It keeps both tests' assertions and a 120 s bound, applied to each scenario, with no retry, skip or weaker assertion, and it
makes each of the four `F_MATRIX` runs observable on its own: each run gets its own bound, as `f_bounded_case` gives
each case of the refusal siblings. The guard names the change.
