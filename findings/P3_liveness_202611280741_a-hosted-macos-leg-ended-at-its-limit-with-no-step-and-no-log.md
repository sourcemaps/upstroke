---
id: CI-MACOS-HOSTED-LEG-ENDED-AT-ITS-LIMIT-WITH-NO-STEP
severity: P3
disposition: deferred
category: liveness
pr: 443
reviewed_sha: d1b706c23def16fb3a37e0e19c122136d8eb99dd
location: .github/workflows/ci.yml:281
provenance: undetermined
first_bad:
guard: a recurrence on any hosted leg, or the change that prints whether a leg that did not succeed began a step
---

# A hosted macOS leg ended at its 30-minute limit with no step and no log

## Failure sequence

1. Run `46089584677` queues `test (macos-latest, 3 of 12)`, and GitHub names a runner for it.
2. The job records no step.
3. GitHub cancels it 35 min 1 s after its start, past the job's limit, `timeout-minutes: 30`
   (`.github/workflows/ci.yml:281`).
4. The aggregate `upstroke-ci` fails on `TEST_MACOS_RESULT: cancelled`.

## The loss

| run | job | platform | date | pull request | checkout |
|---|---|---|---|---|---|
| `46089584677` (`pull_request`), attempt 1 | `140807181395`, `test (macos-latest, 3 of 12)`, runner `GitHub Actions 1000008970` | a GitHub-hosted `macos-latest` runner | 2026-11-28: created 00:45:41Z, started 00:46:12Z, completed 01:21:13Z | #443 at `d1b706c2` | none recorded; this job's attempt 2 and 44 of the run's 45 other attempt-1 jobs checked out `3b3f421`, GitHub's merge of `d1b706c2` into `master` at `b7482c2c`; the 45th, the aggregate `upstroke-ci`, has no checkout step |

GitHub serves no log for this job. Asked for it (`actions/jobs/140807181395/logs`), the API answered
HTTP 404 with a `BlobNotFound` body at 01:23:04Z, 01:25:31Z, 01:31:07Z and 01:57:31Z on 2026-11-28,
each time the saved body's own, and HTTP 404 again at 06:46:36Z, that response's body not kept; the
run's attempt-1 log archive holds no log for it. In place of the log, the API's
records, each saved under `~/selfhost-findings-20261128/evidence/r99-shapes` and listed with its
SHA-256 in that directory's `SHA256SUMS`:

- The job record (`job-140807181395.json`, SHA-256
  `9fa778e2d2ee4a281221ef7c4e9dc821bf4e3b998b9f5389865aada25f059e6c`): conclusion `cancelled`;
  `steps` empty; runner `GitHub Actions 1000008970`; created 00:45:41Z, started 00:46:12Z after a
  31 s queue, completed 01:21:13Z, 35 min 1 s after its start.
- Its check run's annotations (`annotations-140807181395.json`, SHA-256
  `b06b08cd2129c58ec4f808c77343ed50b8d5dbe7272fac7dda462440a8cad60a`): a failure, "The job has
  exceeded the maximum execution time of 30m0s", and a notice, "Due to capacity constraints, jobs
  targeting macOS arm64 runners may experience longer queue times."
- The run's attempt-1 jobs (`run-46089584677-attempt-1-jobs.json`, SHA-256
  `9897407fdef8ced81f236fa07ef547f6338cce64c8aeb195fa71e6206ee95596`): 44 of the 46 jobs passed.
  The other eleven macOS legs queued 2 to 3 s and ran 3 min 52 s to 9 min 47 s, with 11 steps each.
- The aggregate `upstroke-ci`, job `140816939813`, whose log GitHub does serve
  (`~/selfhost-findings-20261128/evidence/job-46089584677-140816939813.log`, ANSI escape sequences
  removed, SHA-256 `6a82d7b243d1140d4d5d8f0956965ab165134051970454987b4e404a40521695`): line 51,
  `TEST_MACOS_RESULT: cancelled`; lines 50 and 52, the Linux and hosted Windows results, `success`.
- #443's lane made the first copies of these records at 01:23Z to 01:57Z, under
  `~/selfhost-sampler-wait-20261127/evidence/ci-46089584677` (its `SHA256SUMS`, SHA-256
  `6c4416834963ec19b84e7da6ab54182e6c605dc39813f511e022364040fbad0b`).

## The same checkout passed

Attempt 2, a re-run of this one job by its id under ruling r99, issued at 01:58:43Z, is job
`140826609413` (`job-140826609413.json` in the same directory, SHA-256
`19f0d157baf082e15de21e177172313215b34b7b4796642e33a54d119088a72b`): runner
`GitHub Actions 1000008999`, 01:58:48Z to 02:06:32Z, 11 steps, all `success`. From its log as
GitHub's API serves it, with ANSI escape sequences removed
(`~/selfhost-findings-20261128/evidence/job-46089584677-140826609413.log`, SHA-256
`b64e49b94655d3525e63a65d175ecec662e471e512c807fd6b735bc1fb4ea64d`):

- Line 107: `HEAD is now at 3b3f421 Merge d1b706c23def16fb3a37e0e19c122136d8eb99dd into b7482c2c11ec38f132d3bd204eaf08ca043e2b99`.
- Line 394: `running 87 tests`.
- Line 484: `test result: ok. 81 passed; 0 failed; 6 ignored; 0 measured; 4178 filtered out; finished in 141.14s`.
- Lines 959 and 1032: the leg's two captures, `engine::tests::host_acceptance_record` and
  `runner::container::tests::container_capability_record`, each `ok`.

Run `46089584677`'s attempt 2 concluded `success`.

## Two of this leg's tests read a file #443 changes

`effects::tests::every_pr6_refusal_st16_variant_and_invariant_clause_names_a_test_or_an_owner` and
`engine::topology::coverage::tests::every_claim_names_a_test_defined_exactly_once_under_src` run in
this leg (attempt 2's log, lines 449 and 482, each `ok`) and read `src/workspace_manager/tests.rs`,
which #443 changes, as source text. #443's round-two frontier pass ran each of them under `strace`
on Linux at `d1b706c2`: "The PR6 mapping test opened the changed file 31 times and
every_claim_names_a_test_defined_exactly_once_under_src opened it 63 times. Both passed. This
establishes reachability only."
(`~/selfhost-sampler-wait-20261127/evidence/review-frontier-d1b706c.md`, SHA-256
`de73ee1e087c70d2a6773a675d1998710521c7e2d3a85f50dd2695126455a8f4`.)

## Three shapes, read again from the API

How a lost hosted macOS leg has looked here, read from the job records and check-run annotations on
2026-11-28 at 06:49Z (saved beside the job's above):

- **A suite at its limit:** jobs `123314750013` (run `40834103792`) and `123327276076`
  (run `40838006947`), `test (macos-latest)`, in `merge_group` runs on 2026-10-29. Nine steps each,
  the step `Run cargo test --all-targets --all-features` `cancelled`; the annotations "The job has
  exceeded the maximum execution time of 30m0s" and "The operation was canceled."; 30 min 10 s and
  30 min 12 s from start to end.
- **A runner-less leg:** job `134272139961` (run `44149091955`), `test (macos-latest, 1 of 3)`, on
  2026-11-17. No runner and no step; the annotation "The job was not acquired by Runner of type
  hosted even after multiple attempts"; 15 min 1 s from start to end.
- **This job:** a runner named, no step and no log; the time-limit annotation without "The
  operation was canceled." and with no "not acquired" annotation; no Node.js 20 warning from the
  checkout action, which legs 1 and 2 of the same run carry (jobs `140807181631` and
  `140807181634`); 35 min 1 s from start to end, 5 min 1 s past the limit.

## What is and is not established

- **Established:** GitHub cancelled the leg 35 min 1 s after its start, past its 30-minute limit,
  with no step recorded and no log served, and the aggregate failed as its dependent. Attempt 2 of
  the same job passed on the same checkout in 7 min 44 s.
- **Not established:** that no test ran.
- The API's records do not show what the runner did between the job's start and its end.
- **Inferred, not established:** the named runner never began the job.
- These readings are not exhaustive.
- **Not established: the rate.**
- `CI-WINDOWS-HOSTED-JOB-OUTLIVED-ITS-TESTS`
  (`findings/P3_liveness_202611162300_a-windows-test-job-never-ended-after-its-tests-passed.md:60-61`)
  records a hosted Windows job that its time limit cancelled after every harness of its shard had
  reported a pass, read from that job's log.
- `PR320-R7-MACOS-SIGTTOU-STRANDED`
  (`findings/P3_portability_202609241645_sigttou-left-the-supervised-tree-stranded-on-macos.md:86-95`)
  records a `test (macos-latest)` job, `123226497738` on 2026-10-29, whose annotations read "The job
  has exceeded the maximum execution time of 30m0s" and then "The operation was canceled", and whose
  log shows the library printing `running 3555 tests`.

## What the change that takes this up should do

Keep the leg's 30-minute limit and every test the leg runs, with no retry, skip or weaker assertion.
Make observable, for a leg that did not succeed, whether it began a step.
