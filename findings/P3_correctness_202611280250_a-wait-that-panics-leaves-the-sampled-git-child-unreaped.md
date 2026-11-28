---
id: PR443-WAIT-PANIC-LEAVES-GIT-UNREAPED
severity: P3
disposition: deferred
category: correctness
pr: 443
reviewed_sha: d1b706c23def16fb3a37e0e19c122136d8eb99dd
location: src/workspace_manager/tests.rs:14943
provenance: introduced_by_feature
first_bad:
guard: the pull request that next changes kill_git_child or SampledChild (either branch of ruling r92 step 7); it kills and reaps the child when the wait unwinds
---

# A wait that panics in the sampler's kill path leaves its Git child unreaped

## Failure sequence

`kill_git_child` spawns its Git child, then waits. In
`kill_git_child_waits_out_its_rung_on_the_clock_and_sleeper_it_is_given` the fake sleeper panics
when the wait asks for a sleep with no reading of the fake clock since the last sleep, or since the
start (a wait that ignores the clock it was given). The panic unwinds past the kill and the reap,
so on Unix the exited Git child stays unreaped until the test binary ends.

Test: `workspace_manager::tests::kill_git_child_waits_out_its_rung_on_the_clock_and_sleeper_it_is_given`.

## What is established

- **Where, by line at `d1b706c2`:** `kill_git_child` spawns the child
  (`src/workspace_manager/tests.rs:14942` at `d1b706c2`), waits (`:14943`), kills (`:14949`) and
  reaps (`:14951`). A panic in the wait skips the kill and the reap.
- **The reviewers' reproduction**, as #443's frontier pass and regression lens at `d1b706c2` give
  it: on Linux each handed the unchanged helper the real elapsed clock with the fake sleeper, and
  the sleeper panicked. After catching the panic, the frontier pass reaped the exact Git pid with
  `waitpid` and the regression lens reaped the Git pid itself, and each got exit status 0.
- **When it happens.** Only on the kill-path test's failing path, when the fake sleeper refuses:
  the sampler hands `kill_git_child` `std::thread::sleep`, which does not panic. What is left is
  one exited `git --version`, held until the test binary ends.
- **Why it is filed.** Ruling r100 read line 6 of the standards' section 6 (ownership, mutation
  and resources) as guidance, carrying neither MUST nor SHOULD, and the frontier's JSON verdict
  gives this finding no `must_deviation`. It is a reproduced P3 on one test's failing path, in a
  test that is a subject, not a gate or an instrument, so the owner's reproduced-findings
  exception of 2026-11-27T08:41:42Z lets it be filed. Its repair is new process handling, a guard
  that kills and reaps on unwind, which ruling r100 kept out of #443's last push.

## What the change that takes this up should do

- Kill and reap the child when the wait unwinds, so that a panic in the wait leaves no child
  behind.
- Do it in the pull request that next changes `kill_git_child` or `SampledChild`, on either branch
  of ruling r92 step 7.
- Keep the kill floor, its message and its per-launch report as they are.
