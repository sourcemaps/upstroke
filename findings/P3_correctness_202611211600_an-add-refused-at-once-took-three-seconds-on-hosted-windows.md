---
id: CI-FLAKE-WINDOWS-ADD-REFUSAL-PAST-REGISTRY-DEADLINE
severity: P3
disposition: deferred
category: correctness
pr: TBD
reviewed_sha: b73d347af70806578051ddd58a439c0db9269a7f
location: src/workspace_manager/tests.rs:17137
provenance: undetermined
first_bad:
guard: the change that makes the test's bound on the git-path add measure what refused at once means, no wait of the registry access and no attempt beyond the first, rather than Git's own wall time on a slow runner, by asserting that the add made one attempt (contended_attempts for the repository unchanged across it, as the test already asserts, and the refusal's 1 attempt(s)) and that no wait of its registry access slept (slept_pauses on the add's thread unchanged across it), and by recording the attempt's own duration beside the refusal, without changing the production REGISTRY_ACCESS_DEADLINE, keeping the assertions that the add is refused after one attempt and leaves nothing at the slot
---

# An add refused "at once" took 3.39 s on a hosted Windows runner, past the 500 ms the test allows

## Failure sequence

    an add whose checkout cannot be made (a tree holding a `.git` path) is refused after one attempt
    -> on a GitHub-hosted windows-latest runner, the add takes 3.39 s from call to refusal
    -> the test bounds that wall time by the registry access deadline, 500 ms in test builds
    -> the test fails though the add made one attempt and was refused as it should be

`workspace_manager::tests::an_add_whose_checkout_cannot_be_made_refuses_after_one_attempt_and_leaves_nothing`
(`src/workspace_manager/tests.rs`, line 17108 at the reviewed commit) pins what
`PR329-A-GENUINE-CHECKOUT-FAILURE-AFTER-THE-TAKEOVER-REFUSES`
(`findings/P2_liveness_202610021308_a-genuine-checkout-failure-after-the-takeover-refuses.md`) accepts: such an add
refuses after one attempt and leaves nothing at the slot. It runs two cases, `git-path` and `name-too-long`; only
`git-path` runs on Windows, because `name-too-long` is skipped where `cfg!(unix)` is false (line 17110).

For each case the test reads `contended_attempts` for the repository, starts a timer, calls `add_worktree`, and stops
the timer when the call returns. It then asserts, in this order:

1. `contended_attempts` is unchanged: "one attempt, never attempted again" (line 17132);
2. `took < REGISTRY_ACCESS_DEADLINE`: "refused at once" (line 17137);
3. the refusal's message says `1 attempt(s)` and `is gone` (line 17141);
4. nothing is left at the slot (line 17145).

In test builds `REGISTRY_ACCESS_DEADLINE` is **500 ms** (`src/workspace_manager.rs`, line 9502); in production it is
10 s (line 2319). The second assertion bounds the whole call's wall time, Git's `git worktree add` included, by that
500 ms.

## The one sighting

| run | job | platform | date | pull request | head |
|---|---|---|---|---|---|
| `44740932431` (`pull_request`) | `136304158953`, `test (windows-latest, 5 of 5)` | a GitHub-hosted `windows-latest` runner | 2026-11-20, 13:03:25Z to 13:21:19Z | #390 | `4ad37204` |

Read from the raw job log (`evidence/job-44740932431-136304158953.log`, SHA256
`9ba03189797e33f292101dc466ed718d0a6de42aeced1c034ea9fa04809c7af0`):

- `workspace_manager::tests::an_add_whose_checkout_cannot_be_made_refuses_after_one_attempt_and_leaves_nothing ...
  FAILED`, panicking at `src\workspace_manager\tests.rs:17137:9` with:

  > git-path: refused at once: 3.3885434s

- `test result: FAILED. 2252 passed; 1 failed; 54 ignored; 0 measured; 1351 filtered out; finished in 872.13s`. It
  was the job's only failure.

The add took **3388.5 ms, about 2.9 s past the 500 ms bound**, nearly seven times it.

Pull request #390 changes no workspace-manager code. The same test passed in the next run of the same tree, CI run
`44819913095` at `ef3c3768` (an empty commit on `4ad37204`), on 2026-11-20.

**Related, not the same:** `CI-FLAKE-WINDOWS-LIST-PAST-REGISTRY-DEADLINE`
(`findings/P3_correctness_202611120401_a-windows-registry-listing-took-longer-than-the-tests-deadline.md`) is another
test's duration past the same 500 ms deadline. It differs in three ways:

- **the operation**: there a `git worktree list`, which takes no lock and may be attempted again until its deadline,
  in `an_add_refuses_within_its_deadline_while_r_x_is_held_alone_and_a_list_does_not_wait`; here an add that is
  refused after one attempt and is never attempted again;
- **the runner**: there the self-hosted Windows guest (`test (winguest)`); here a GitHub-hosted `windows-latest`
  runner;
- **by how much**: there 528 ms, 28 ms past the bound; here 3388 ms, about 2.9 s past it.

## What is and is not established

**Established.** One add of the `git-path` case, on a hosted Windows runner, took 3.39 s from `add_worktree`'s call
to its return, and failed the test's timing assertion; the change under test did not touch the workspace manager;
the same tree passed in the next run.

**Established: the add was not attempted again.** The `contended_attempts` assertion runs before the timing
assertion and passed: no answer of the access decided to attempt again. The message and slot assertions run after
the timing assertion, so they were not reached in this job.

**Inferred, not established: the 3.39 s was spent in the one `git worktree add` attempt.** That attempt is Git taking
the destination over, failing the checkout, and removing its junk, on a runner that may have been loaded; the
`contended_attempts` assertion's passing supports that there was one attempt and no second. The log does not say
where in the call the time went: it records neither the attempt's own duration nor whether any wait of the access
slept. Time spent outside the Git process (in the veto that decides the destination `is gone`, in the add's other
work, or with the test's thread not scheduled) is not ruled out. **What would establish it:** the attempt's own
duration, from just before Git is started to just after it exits, recorded beside the refusal.

**Not established: the rate.** One sighting; the jobs of other runs on 2026-11-20 were not counted.

## A second sighting, 2026-11-27

| run | job | platform | date | pull request | checkout |
|---|---|---|---|---|---|
| `46020861627` (`pull_request`), attempt 1 | `140579864108`, `test (windows-latest, workspace 2 of 2 (guarded_reads, manager, reclaim, refs, registry, snapshots))` | a GitHub-hosted `windows-latest` runner, image `windows-2025-vs2026` version `20261116.315.1` (log lines 17-18) | 2026-11-27, 14:59:44Z to 15:08:27Z (8 min 43 s, inside the leg's 40-minute limit) | #439 at `449fcfb1` | `c91f148`, GitHub's merge of `449fcfb1` into `master` at `ca058512` (log line 109) |

Read from the job's log as GitHub's API serves it, with ANSI escape sequences removed
(`~/selfhost-findings-20261128/evidence/job-46020861627-140579864108.log`, SHA256
`c7c69b75be18ecd8d57caaa347f9f67f2b953de3192e2e63abdb501ea888ddb0`):

- Line 703:
  `workspace_manager::tests::registry::an_add_whose_checkout_cannot_be_made_refuses_after_one_attempt_and_leaves_nothing
  ... FAILED <6.163s>`; line 785: it panicked at `src\workspace_manager\tests\registry.rs:1128:9` with (line 786):

  > git-path: refused at once: 532.4028ms

- Line 792:
  `test result: FAILED. 210 passed; 1 failed; 18 ignored; 0 measured; 3696 filtered out; finished in 294.83s`. It
  was the job's only failure.

The add took **532.4 ms, 32.4 ms past the 500 ms bound**.

**What the test asserts, by line at `449fcfb1`** (`c91f148` has its tree). #439 moved this test to
`src/workspace_manager/tests/registry.rs` (line 1099); its lines there, 1047 to 1141, are `master` `ca058512`'s
`src/workspace_manager/tests.rs:17971-18065` line for line (ruling r86 step 2's comparison, run again for this
record: `~/selfhost-findings-20261128/evidence/test-text-449fcfb1.txt`, SHA256
`0889db60fdd82e769744ec0301060812732a9323f1769e9195c1e18f97d2d13f`). Its assertions are the four this file numbers
above, in the same order: `contended_attempts` unchanged, "one attempt, never attempted again" (line 1123);
`took < REGISTRY_ACCESS_DEADLINE`, "refused at once" (line 1128), the one that failed; the message (line 1132); and
the slot (line 1136). `REGISTRY_ACCESS_DEADLINE` is 500 ms in test builds (`src/workspace_manager.rs`, line 9524).

**The same checkout passed.** Attempt 2 of run `46020861627`, under ruling r86, re-ran this job on the same
checkout, `c91f148` (line 109 of its log): job `140607675103`, 15:54:27Z to 15:59:36Z, on image version
`20261123.321.1` (line 18). Line 702: `... ok <1.445s>`. Line 779:
`test result: ok. 211 passed; 0 failed; 18 ignored; 0 measured; 3696 filtered out; finished in 154.50s`. Line 791:
the shard reported 217 passing tests against its floor of 94. Its log:
`~/selfhost-findings-20261128/evidence/job-46020861627-140607675103.log`, SHA256
`4bae152bb84a904c9843568f07819ee735d1a28092073059d34337d79356fd88`.

**What is and is not established for this sighting.**

- **Established:** one add of the `git-path` case, on a hosted Windows runner, took 532.4 ms from `add_worktree`'s
  call to its return and failed the timing assertion; the `contended_attempts` assertion before it passed, so the
  add was not attempted again; the same checkout passed the test on attempt 2, in 1.445 s.
- The log does not show where in the call the time went: it records neither the attempt's own duration nor whether
  any wait of the access slept.
- **Inferred, not established:** the 532.4 ms was spent in the one `git worktree add` attempt.
- **Inferred, not established:** the time was spent outside the Git process, in the add's other work or with the
  test's thread not scheduled.
- These readings are not exhaustive.
- **Not established: the rate.**

## What the change that takes this up should do

- **Make the bound measure what "at once" means**: no wait for the registry deadline and no attempt beyond the first,
  rather than Git's own wall time on a slow runner. For instance:
  - keep the `contended_attempts` assertion (no answer to attempt again) and the message's `1 attempt(s)`;
  - assert that no wait of the add's registry access slept, by reading `slept_pauses`
    (`src/workspace_manager/fixture.rs`, line 331, counted by `sleep_for` in `src/workspace_manager.rs`, line 2417)
    before and after the add, on the thread the add's access runs on. A wait for R-X until the deadline and a back-off
    before another attempt both sleep through `sleep_for` when the add runs with `NoHooks`; the change should confirm
    that the add's accesses run on the test's own thread before relying on this thread-local count;
  - in place of `took < REGISTRY_ACCESS_DEADLINE`, or beside a wall-time bound wide enough not to measure Git, record
    the attempt's own duration and report it with the refusal, so a later sighting says where the time went.
- **Do not change the production deadline** (`REGISTRY_ACCESS_DEADLINE`, 10 s), nor the 500 ms the test build uses
  for the accesses other tests drive to their deadline.
- **Keep the test's other assertions as they are**: one attempt, the refusal's message, nothing left at the slot.
- **Do not retry or skip the test** to get past this.
