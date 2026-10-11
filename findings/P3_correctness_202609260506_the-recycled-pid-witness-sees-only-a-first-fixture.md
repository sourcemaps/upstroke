---
id: PR322-RECYCLED-PID-WITNESS-SEES-ONLY-A-FIRST-FIXTURE
severity: P3
disposition: deferred
category: correctness
pr: 322
reviewed_sha: c2586eac04b52daafbb2ec18b110a3d454b5585d
location: src/engine/topology/recover/tests.rs:374
provenance: introduced_by_feature   # this pull request's round 2
first_bad: 5667e39b
guard: a change that runs the witness's body as a child of the test binary under `--exact`, so that its fixture is always its process's first
---

## Failure sequence

`a_fixture_never_builds_on_a_root_a_crashed_process_under_this_pid_left` plants a predecessor at
the name the old pid-keyed helper gave a process's **first** fixture —
`upstroke-pr7e-<pid>-<tag>-0`, the ordinal a per-process counter starts at — and asserts the fixture
is not built there. The old helper computed that name only while its counter was still at 0. In a
process that has already built another fixture, the counter has moved on, the helper computes
`…-1` or later, and nothing collides with the plant.

So under a mutant that restores the pid-keyed, adopting helper, the witness is red only when its
fixture is its process's first. #322's round-2 delta review measured both sides at `c2586eac`, the
mutant env-gated in a `git archive` copy
(`review-322-delta-evidence/item3/rec-head-mut-pidkeyed-*.json` on the build box):

| run | exit |
|---|---|
| the witness alone, under the mutant | **101** |
| the witness after `a_build_that_panics_half_way_leaves_no_tree_behind` in one process, under the mutant | **0** |

and at `67d9bd41`, before the repair, the witness reads `ok` when it runs after that same test in
one process (`rec-base-wit-both.json`; the process exits 101 only on the other test's expected
failure there). In the suite CI runs — every test in one process, in parallel, in no fixed order —
the witness therefore cannot see the regression it names.

## Why it is deferred rather than fixed

**It is disclosed where it is written.** The witness's notes
(`docs/internals/engine/topology/recover/tests.md`) say it is red at `67d9bd41` "whenever its fixture
is its process's first … and always when it runs alone", and the pull request body says the same, so
its stated scope is accurate. The repair it witnesses is pinned elsewhere: structurally by
`scratch_tree::acquire`, whose exclusive create refuses an occupied name and whose name carries no
pid, and by the round-2 review's external recycled-pid runs, which reproduced the sequence at
`67d9bd41` and not at `c2586eac`. The review did not treat it as blocking, and the orchestrator's
round-3 brief directs it filed, not restructured, in that round.

**`MAINTAINING.md` step 5's non-discretionary limb.** The mutation is a witness of the *witness's*
blindness, not of a defect in the code under test: the code under test is repaired, and the mutant
that restores the defect is caught when the witness runs alone.

## What the change that takes this up should do

The review's suggestion: run the witness's body as a child of the test binary under `--exact`, so
the fixture it builds is always its process's first whatever the parent process has built before.
`prelock/tests.rs`'s `a_failed_reclamation_during_an_unwind_does_not_abort_the_process` is the shape
a `TOPOLOGY_MODULE` test uses for that — the child spawned through the host `Runner`, since
`std::process::Command` is denied there — and the parent should require the child's exit **and** its
`1 passed` line, so a filter that selects nothing cannot pass.

## Reachability, against the owner's rule of 2026-09-11

**Neither limb.** Test-only; no user input reaches it and nothing a required check runs changes.
