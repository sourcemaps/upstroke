---
id: CI-FLAKE-WINDOWS-CENSUS-LISTING-PAST-ITS-QUESTION-BOUND
severity: P3
disposition: deferred
category: liveness
pr: TBD
reviewed_sha: 50c11ded63e3d4e5ea87b07d3fc25caff0c46bd3
location: src/runner/container/census.rs:569
provenance: undetermined
first_bad:
guard: the change that makes an unanswered listing say which of its three ends it met and after how long, then ends the tests' dependence on a hosted runner's Docker answering inside 60 s, keeping every assertion, the bound and the refusal
---

# The container census's runtime listing did not answer inside its 60 s bound on hosted Windows

## Failure sequence

    a test starts a schema-4 run on a hosted Windows runner
    -> the census asks `docker ps --all --filter label=upstroke.private_root=…`
    -> the question ends before Docker answers ("the bound it was asked under passed, its invocation was
       cancelled, or a signal ended it")
    -> the census refuses
    -> the test's run is refused and the test panics

## What the code does

At `50c11ded`:

- `discover_by_label` (`src/runner/container/census.rs:563`) asks the runtime for every container carrying the
  run's `upstroke.private_root` label (line 569, `runtime.containers_with_label(LABEL_PRIVATE_ROOT, &label)`).
- If that question fails in a way `proceeds_without` does not accept (line 573), it returns
  `UpstrokeError::Refused` (lines 574-584) with:

  > the container runtime was reached and refused `list-by-label` under `<path>`: <error>. A runtime that answers
  > and will not list cannot prove that no labeled container of a dead owner is still running, so this write
  > command refuses rather than admitting over one

- Each Docker question runs under `QUESTION_BOUND` (`src/runner/container.rs:958`, `Duration::from_secs(60)`).

## The sightings

### Sighting 1

Run `45609656177` (`pull_request`, #426 at `f8f2815b`), job `139189568820`, `test (windows-latest, 16 of 22)`,
runner image `windows-2025-vs2026` version `20261116.315.1` (lines 17-18). Log SHA-256
`8e35868eed4b338d0c747f4c1e6b8988fbe0d311aefdb35e08bc86da8df342b6`:

- Line 109, the checkout:

      HEAD is now at 78a1b27 Merge f8f2815bde34288627bbe16240ea040132eac053 into 0d45280c2605c0e401bd76d63002c476c443aa58

- Line 455: `engine::tests::host_acceptance_record` panicked at `src\engine\tests.rs:19830:53`; line 456:

      the fresh schema-4 host run completes: Refused { message: "the container runtime was reached and refused `list-by-label` … ended before it answered: the bound it was asked under passed, its invocation was cancelled, or a signal ended it. …" }

- Line 465:

      test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 3911 filtered out; finished in 63.62s

### Sighting 2

Run `45630176996` (`pull_request`, #426 at `1c6883d2`, an empty commit with `f8f2815b`'s tree), job
`139261221083`, `test (windows-latest, 11 of 22)`, image version `20261116.315.1` (line 18). Log SHA-256
`b3ae587cbed1716c6883ae6afe527cae6af47fd2b1544f967c17caafa7209ce7`:

- Line 109, the checkout:

      HEAD is now at 286ef2e Merge 1c6883d23262137d5b6afab56e759d0a3198af5e into 0d45280c2605c0e401bd76d63002c476c443aa58

- Lines 369, 370, 371 and 373: four tests in `engine::tests` `FAILED`:

  | test | time |
  |---|---|
  | `a_schema_4_answer_in_words_reaches_the_next_worker_and_its_reviewer` | 74.115 s |
  | `a_schema_4_follow_prints_one_line_per_event_and_no_raw_control_whatever_the_plan_or_the_record_holds` | 74.609 s |
  | `a_schema_4_declined_spend_approval_fails_the_task_before_the_frontier_runs` | 76.430 s |
  | `a_schema_4_frontier_escalation_over_the_threshold_waits_for_its_approval` | 69.171 s |

- Lines 426-447: each panicked (at `src\engine\tests.rs` lines 29034, 23758, 28924 and 28860) on a `Refused`
  with the same `list-by-label` message and "ended before it answered".
- Line 452:

      test result: FAILED. 47 passed; 4 failed; 0 ignored; 0 measured; 3861 filtered out; finished in 280.36s

### Sighting 3

Run `45637680259` (`pull_request`, #429 at `40f2332e`), job `139288287202`, `test (windows-latest, 3 of 22)`,
image version `20261123.321.1` (line 18). Log SHA-256
`b714fb8034562b7375afedd533f8a66d77f0a10352fe611fdd579841ddb0eaa3`:

- Line 109, the checkout:

      HEAD is now at da408f2 Merge 40f2332eab0411b5b148951fe3d42a2c2affe87d into 50c11ded63e3d4e5ea87b07d3fc25caff0c46bd3

- Line 636:
  `engine::tests::a_resume_selects_its_reader_by_the_logs_header_and_refuses_before_any_lock ... FAILED <70.285s>`;
  line 802, it panicked at `src\engine\tests.rs:21923:64`; line 803, on a `Refused` with the same `list-by-label`
  message and "ended before it answered".
- Line 809:

      test result: FAILED. 363 passed; 1 failed; 22 ignored; 0 measured; 3530 filtered out; finished in 318.81s

#429's diff changes files under `src/capacity/`, `src/engine/` and `src/util.rs`, and none under
`src/runner/container/`; its earlier head `16935754` passed this test in run `45620876733`, job `139228353555` of
the same shard (log SHA-256 `5d1c459afa9aa92f62de0a621f4af989f241e97d75e39dbec46681d5e847c047`, line 634,
`ok <46.810s>`).

## The same tree passed

Run `45607353978` (`pull_request`, #426 at `f8f2815b`) checked out the same merge commit as sighting 1
(`78a1b27`, line 109 of both its Windows logs below). Sighting 2's head has `f8f2815b`'s tree.

- Job `139181564119`, `test (windows-latest, 11 of 22)`, SHA-256
  `e3288c94af71850783ee1ae176d4d8b0caa33841c1f6b36e08241bbdac60e621`: lines 365, 366, 368 and 372 show the four
  tests of sighting 2 `ok`, in 6.043, 6.873, 8.420 and 22.496 s; line 417:

      test result: ok. 51 passed; 0 failed; … finished in 123.52s

- Job `139181564633`, `test (windows-latest, 16 of 22)`, SHA-256
  `4066f43b2d35b3f9cb82a5bf9e9a12dba4e501bffc7b9454e40c0536e2d84342`: line 890,
  `test engine::tests::host_acceptance_record ... ok`; line 892, `finished in 22.97s`.

#426's diff from its base is `docs/internals/config.md` and one deleted file under `findings/`.

## What is and is not established

**Established.** In three runs, six tests failed on the census's refusal of `list-by-label` after the question
"ended before it answered", each on a hosted Windows leg. The same tree passed all five in run `45607353978`.
#426's diff reaches no code, and #429's changes nothing under `src/runner/container/`.

**Not established:**

- which of the three ends the question met (the message does not say);
- how long Docker took;
- why.

Ruling r64 counted the hosted Windows legs: none failed in the 20 runs created 04:43Z-11:45Z, and three of the
next seven did, all on diffs with no code.

**Readings.**

- *Inferred, not established:* the runner's Docker service was slow to answer for a stretch, and every test that
  asked during it waited past 60 s.
- *Inferred, not established:* the question was ended by something other than its bound, such as a cancellation
  or a signal on the runner.

These readings are not exhaustive.

## Pointer

`src/runner/container.rs:958` is the bound named in the message's first end. No open finding names
`list-by-label`.

## What the change that takes this up should do

It makes an unanswered listing say which of its three ends it met and after how long, then ends the tests'
dependence on a hosted runner's Docker answering inside 60 s, keeping every assertion, the bound and the refusal,
with no retry, skip or weaker assertion.
