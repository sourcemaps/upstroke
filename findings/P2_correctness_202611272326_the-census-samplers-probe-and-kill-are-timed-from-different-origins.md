---
id: PR443-PROBE-AND-KILL-CLOCK-ORIGINS
severity: P2
disposition: deferred
category: correctness
pr: 443
reviewed_sha: 4a12f13759e3b848be0515c8ee4b95d5900210ea
location: src/workspace_manager/tests.rs:14327
provenance: pre_existing
first_bad:
guard: the pull request that next changes this sampler's probe (measure_budget), the second branch of ruling r92 step 7; it times the probe from the origin the kill is timed from
---

# The census sampler's probe and its kills are timed from different origins

## Failure sequence

A probe and its samples pay the same launch delay. `measure_budget` includes that delay in the
budget the kill ladder is cut from, while each sample's clock starts after the spawn. So even the
first rung can exceed what is left of the child's life, and kills timed accurately miss all four
commands.

Test: `workspace_manager::tests::sampled_git_child_kills_every_residue_classified_and_recovered`.

## What is established

- **The two origins, by line at `4a12f137`:**
  - `measure_budget` reads its clock before the probe's command is launched
    (`src/workspace_manager/tests.rs:14327` for `worktree add`, and `:14338` for the other three
    sites), so the probe's duration includes the launch.
  - `SampledChild::spawned` is read once `Command::spawn` has returned (`:14799`), and
    `SampledChild::kill` reads each firing from it (`:14826`). Each rung is therefore aimed at time
    measured after the launch.
  - Both are as they are on `master` (`b7482c2c`).
- **The reviewer's reproduction**, as #443's frontier review at `4a12f137` gives it: on Linux, with
  900 ms inserted immediately before each probe and sample launch, the sampler failed at its kill
  floor. All 32 firings were 0.092 to 0.697 µs past their rungs, every end was `Completed`, and it
  exited 101.
- **The sibling's record.** `fixture::KillableGitChild` reads its `origin` before `Command::spawn`,
  and `time_git`'s probe is timed from the same place (`src/workspace_manager/fixture.rs:3289-3299`
  at `4a12f137`). Until 2026-09-10 that origin was read once the spawn had returned (the ultra
  review of `d1fef26d`, finding 1).
- Not established: that any hosted red had this cause.

## What the change that takes this up should do

- Time this sampler's probe from the origin its kills are timed from, as the sibling does.
- Do it in the pull request that next changes the probe (`measure_budget`), the second branch of
  ruling r92 step 7.
- Keep the kill floor, its message and its per-launch report as they are.
