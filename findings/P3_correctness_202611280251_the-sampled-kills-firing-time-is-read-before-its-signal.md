---
id: PR443-FIRING-TIMESTAMP-PRECEDES-SIGNAL
severity: P3
disposition: deferred
category: correctness
pr: 443
reviewed_sha: d1b706c23def16fb3a37e0e19c122136d8eb99dd
location: src/workspace_manager/tests.rs:14969
provenance: pre_existing
first_bad:
guard: the change that next alters what a kill-floor red reports (ruling r92 step 7's first branch); it reads the clock on both sides of the signal
---

# A sampled kill's firing time is read before its signal is sent

## Failure sequence

The wait reaches its rung, and `SampledChild::kill` reads `fired`. The parent is descheduled
before the signal call, and Git exits. The kill-floor report shows an on-time firing for a signal
sent late.

Test: `workspace_manager::tests::sampled_git_child_kills_every_residue_classified_and_recovered`.

## What is established

- **The read order, by line at `d1b706c2`:** `SampledChild::kill` reads `fired` off the clock
  (`src/workspace_manager/tests.rs:14835` at `d1b706c2`) before it sends the signal (`:14837` on
  Unix, `:14839` on Windows), and stores it after (`:14840`). It is as it is on `master`
  (`b7482c2c`), outside #443's diff.
- So each reported firing is a lower bound on when the signal was sent.
- **The reviewer's reproduction**, as #443's frontier pass at `d1b706c2` gives it: on Linux, with a
  150 ms sleep inserted immediately after the `fired` reading, the sampler exited 101 at its kill
  floor with all 32 launches `Completed`, and the reported lateness ranged from 75 to 467 ns.
- **What a later red can and cannot show.** An on-time firing with every end `Completed` does not
  show that the signal was sent on time. Ruling r92 step 7's second branch (on-time firings with
  every end `Completed`: a second pull request that gives `measure_budget` `KillBudget::probed`) is
  read with that.
- **Why it is filed.** The read order is master's. The owner, 2026-11-27T10:28:38Z: "if they're
  already on master then we won't fix them here."

## What the change that takes this up should do

- Read the clock on both sides of the signal, and report both.
- Do it in the change that next alters what a kill-floor red reports, ruling r92 step 7's first
  branch.
- Keep the kill floor's assertion as it is.
