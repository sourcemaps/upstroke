---
id: CI-FLAKE-WINDOWS-THREE-COORDINATOR-TESTS-SEVERAL-RUNS-UNDER-ONE-BOUND
severity: P3
disposition: deferred
category: liveness
pr: TBD
reviewed_sha: 50c11ded63e3d4e5ea87b07d3fc25caff0c46bd3
location: src/engine/topology/coordinator.rs:26567
provenance: undetermined
first_bad:
guard: the change that bounds each run of a_cancelled_or_unresolved_gate_run_ends_the_command_as_it_did_whatever_the_registration, a_container_gate_launched_in_a_whole_snapshot_is_judged and a_verification_refused_after_its_passes_were_charged_records_them_under_o14b on its own, as f_bounded_case does (src/engine/topology/coordinator.rs:25873), keeping every assertion and the 120 s bound, with no retry, skip or ignore, on branch fix-P3/liveness_two-gate-control-tests-ran-past-one-bound-on-hosted-windows
---

# Three coordinator tests ran several runs under one 120 s bound past it on hosted Windows

## Failure sequence

    the job's test harness runs a test with the shard's other tests
    -> the test's runs together take longer than 120 s
    -> `bounded` panics at `src\engine\topology\coordinator.rs:11345:64`
    -> the job fails

## What the tests assert

At `50c11ded`, in `src/engine/topology/coordinator.rs`:

- `a_container_gate_launched_in_a_whole_snapshot_is_judged` (line 26467) runs all four entries of `F_MATRIX` inside
  one call of `bounded`, named `f-c1-p2-launch-control` (line 26468).
- `a_cancelled_or_unresolved_gate_run_ends_the_command_as_it_did_whatever_the_registration` (line 26566) runs all
  four entries of `F_MATRIX`, each with `FLaunch::Cancelled` and `FLaunch::Unresolved`, eight runs, inside one call
  of `bounded`, named `f-c1-p2-launch-fates` (line 26567).
- `a_verification_refused_after_its_passes_were_charged_records_them_under_o14b` (line 27864) runs widths 1 and 3,
  each with `FAnswer::Unavailable` and `FAnswer::Pass`, four runs, inside one call of `bounded`, named `f-o14b`
  (line 27865).
- `bounded` (line 11330) runs its body on a thread. If the body has not finished within `BOUND` (line 2181,
  120 s), it panics at line 11345 with:

  > `<name>` did not end within 120s: the coordinator waited on its inbox for a pipeline that was itself waiting
  > on the coordinator

- `f_bounded_case` (line 25873) bounds each case of a test on its own.

## The sightings

| run | event | pull request | head | created |
|---|---|---|---|---|
| `45633405997` | `pull_request` | #428 | `4a3e2030` | 2026-11-25T14:22:49Z |
| `45642925560` | `pull_request` | #427 | `498c09a8` | 2026-11-25T15:14:33Z |

Both jobs checked out, at line 109:

    cd9a41b Merge 4a3e20303afbb844157f471ad164a3638e492b00 into 0d45280c2605c0e401bd76d63002c476c443aa58

Job `139272354717`, `test (windows-latest, 18 of 22)`, image version `20261123.321.1` (line 18), log SHA-256
`993092f4d37d4dc7ab6937a66d0c781a34cb1fae4731a66658b7822d04af1cba`:

- Line 378:

      a_cancelled_or_unresolved_gate_run_ends_the_command_as_it_did_whatever_the_registration ... FAILED <121.459s>

- Line 380:

      a_container_gate_launched_in_a_whole_snapshot_is_judged ... FAILED <120.004s>

- Lines 444 to 450: both panicked at `src\engine\topology\coordinator.rs:11345:64`; line 445 reads
  `f-c1-p2-launch-fates did not end within 120s: ...`.
- Line 458:

      test result: FAILED. 61 passed; 2 failed; 6 ignored; 0 measured; 3843 filtered out; finished in 269.56s

Job `139272354772`, `test (windows-latest, 4 of 22)`, image version `20261116.315.1` (line 18), log SHA-256
`fd793d8dd26e5884a2a6376c9843da3cf1cff9831f5314c211c85d1954a98339`:

- Line 379:

      a_verification_refused_after_its_passes_were_charged_records_them_under_o14b ... FAILED <120.022s>

- Line 483: it panicked at `src\engine\topology\coordinator.rs:11345:64`.
- Line 491:

      test result: FAILED. 88 passed; 1 failed; 4 ignored; 0 measured; 3819 filtered out; finished in 247.08s

#428's diff from its base is six files under `docs/internals/agent/`.

Run `45642925560` (#427 at `498c09a8`), job `139306121355`, `test (windows-latest, 4 of 22)`, image version
`20261123.321.1` (line 18), log SHA-256 `08b41b91bd15c527691ac97e95e14e994e9f63d3fcf8e137e5ce75fa514be270`:

- Line 109, the checkout:

      HEAD is now at 16a781d Merge 498c09a83bac3a5cfed0bb6b345e9389508973f4 into 50c11ded63e3d4e5ea87b07d3fc25caff0c46bd3

- Line 380:

      a_verification_refused_after_its_passes_were_charged_records_them_under_o14b ... FAILED <120.885s>

- Lines 485 and 486: it panicked at `src\engine\topology\coordinator.rs:11345:64` with
  `f-o14b did not end within 120s: ...`.
- Line 493:

      test result: FAILED. 88 passed; 1 failed; 4 ignored; 0 measured; 3821 filtered out; finished in 262.90s

#427's diff from its base is `docs/internals/agent/proc.md`, `src/agent/proc.rs`, `src/workspace_manager/fixture.rs`
and `src/workspace_manager/tests.rs`; its earlier head `2113ba72` passed this test on the same shard in run
`45615743035`, job `139210548361` (log SHA-256 `1e3e850856fe38c0fba04ca91ae624e1aac5b0107fdfe50fe0d8609fe03c8e4d`,
line 380, `ok <118.213s>`; line 479, `finished in 248.93s`).

## The same shards passed

Run `45610806016` (`pull_request`, #428 at `f5a4a13c`) checked out, at line 109:

    f69c81b Merge f5a4a13c38c9cb099a2e6bfea7ad7fb82ad6d41b into 0d45280c2605c0e401bd76d63002c476c443aa58

Its tree differs from the sighting's only in four of those notes.

Job `139193492774`, `test (windows-latest, 18 of 22)`, log SHA-256
`a9353aba3e71630deb7570428529d62b7e4b9b446491abc995eda2de9a7976a7`:

- Line 377: `a_cancelled_or_unresolved_gate_run_ends_the_command_as_it_did_whatever_the_registration ... ok <97.356s>`.
- Line 392: `a_container_gate_launched_in_a_whole_snapshot_is_judged ... ok <74.517s>`.
- Line 437: `finished in 195.07s`.

Job `139193492427`, `test (windows-latest, 4 of 22)`, log SHA-256
`966ac45280b42e3fad46d4f34a465be71021c68754b2db261ae0f57f2c8b986a`:

- Line 379: `a_verification_refused_after_its_passes_were_charged_records_them_under_o14b ... ok <94.341s>`.
- Line 479: `finished in 217.52s`.

Ruling r65 read the same three tests across seven green runs from 09:47Z to 14:05Z as 85.4 to 114.4 s
(`f-c1-p2-launch-fates`), 69.1 to 88.4 s (`f-c1-p2-launch-control`) and 54.2 to 94.3 s (`f-o14b`). Those runs'
logs are not copied here.

## What is and is not established

**Established.** Three tests, each running four or eight runs under one 120 s bound, failed at that bound in two
Windows jobs of one run, and one of them,
`a_verification_refused_after_its_passes_were_charged_records_them_under_o14b`, again in a second run.
On the same shards one run earlier they passed in 74.5 to 97.4 s. #428's diff is notes only, and #427's reaches
nothing under `src/engine/`.

**Not established:**

- whether any run was still advancing when its bound passed (`bounded` reports nothing on timeout);
- the rate.

**Readings.**

- *Inferred, not established:* the legs ran slow, and each test's runs were still advancing when the one bound
  passed.
- *Inferred, not established:* one run stopped advancing before the bound passed.

These readings are not exhaustive.

## Pointers

`CI-FLAKE-WINDOWS-INTERLEAVING-TIME-BOUNDS`
(`findings/P3_liveness_202611211601_two-interleaving-tests-ran-past-their-time-bounds-on-hosted-windows.md`) and
`CI-FLAKE-WINDOWS-GATE-CONTROLS-FOUR-RUNS-UNDER-ONE-BOUND`
(`findings/P3_liveness_202611250457_two-gate-control-tests-ran-past-one-bound-on-hosted-windows.md`) record the same
panic site, `src/engine/topology/coordinator.rs:11345`, for other tests.

## What the change that takes this up should do

It bounds each run of these three tests on its own, as `f_bounded_case` does, keeping every assertion and the 120 s
bound, with no retry, skip or ignore. It is the same change the gate-controls finding's guard names, on the same
branch.
