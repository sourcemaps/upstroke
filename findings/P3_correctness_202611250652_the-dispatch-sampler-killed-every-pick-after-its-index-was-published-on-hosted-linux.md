---
id: CI-FLAKE-DISPATCH-SAMPLER-EVERY-KILL-AFTER-PUBLISH
severity: P3
disposition: deferred
category: correctness
pr: 423
reviewed_sha: e1e39e974f631203bd4609926b6738470b54ec46
location: src/engine/topology/dispatch/tests.rs:1570
provenance: undetermined
first_bad:
guard: the change that makes the dispatch sampler's requirement that at least one kill lands before the pick's index is published hold on a fast or unevenly scheduled machine, or replaces the timing sample with a deterministic interruption, keeping the requirement, with no retry, skip or weaker assertion
---

# The dispatch sampler killed every pick after its index was published, on a hosted Linux runner

## Failure sequence

    the sampler times probe picks and aims its kills inside the median it measured (2.636521 ms)
    -> all eight kills are aimed between 292.947 µs and 2.343574 ms, and each kill returns between 2.22146 ms and 2.628139 ms
    -> every killed child is classed `After`: its index was already published
    -> the assertion that at least one kill left a class other than `After` fails the test

`engine::topology::dispatch::tests::sampled_repair_materialization_child_kills_every_residue_classified_and_recovered`
runs `git cherry-pick --no-commit` children and kills each at a delay. The assertion at
`src/engine/topology/dispatch/tests.rs:1570` (at `e1e39e97`) requires that, among the children
the kill reached, at least one was classed other than `ObjectResidue::After`
(`killed_classes.iter().any(|class| *class != ObjectResidue::After)`). **That is the assertion
that failed.** The test's next assertion, at line 1578, is `kills.iter().any(KilledSample::while_writing)`.

## The sighting

| run | job | platform | date | pull request | checkout |
|---|---|---|---|---|---|
| `45555894218` (`pull_request`) | `139007810588`, `test (ubuntu-latest, 5 of 5)` | a GitHub-hosted `ubuntu-latest` runner | 2026-11-25, 05:46Z to 05:52Z | #423 at `483ee5bd` | `5cc8c36`, GitHub's merge of `483ee5bd` into `681f429f` (log line 123) |

Read from the job's log as GitHub's API serves it, with ANSI escape sequences removed, SHA-256
`b4a7f83097b99ae6ee90331201ee995d971030e47674ad81e95091083fad1bb7`:

- Line 1458: `test engine::topology::dispatch::tests::sampled_repair_materialization_child_kills_every_residue_classified_and_recovered ... FAILED`.
- Line 3203: the panic at `src/engine/topology/dispatch/tests.rs:1570:5`. Line 3204: "every one of
  the 8 kills landed after the index was published, so the sample interrupted no materialization
  the site registers: [After, After, After, After, After, After, After, After]; the probe measured
  2.636521ms and the ladder followed 0 completion(s)". Its schedule lists samples 0 to 7, aimed at
  292.947 µs, 585.894 µs, 878.84 µs, 1.171787 ms, 1.464734 ms, 1.757681 ms, 2.050627 ms and
  2.343574 ms; each was killed, the kill returned between 2.22146 ms and 2.628139 ms, and each is
  `After`.
- Line 3210: `test result: FAILED. 2637 passed; 1 failed; 105 ignored; 0 measured; 1640 filtered
  out; finished in 230.52s`. This test was the job's only failure.
- **The same tree passed.** Run `45560655637`, job `139023287920`, the same leg, on #423's empty
  commit `75a5143a` (the tree of `483ee5bd`), checked out `75c70a6`, GitHub's merge of `75a5143a`
  into the same `681f429f` (log line 123), and line 1459 reports the test `ok`. That log's
  SHA-256 is `3b0573a756c4088b1d00be8730b5134a92e796124c3ee10d14521572fb18dbd9`.
- #423's diff changes only files under `findings/`.

## What is and is not established

- **Established:** on one hosted Linux leg, all eight kills the sampler made reached a child classed
  `After`, no pick completed before its kill, and the probe measured 2.636521 ms. The same tree,
  merged into the same `master`, passed the test 40 minutes later.
- **Not established:** when each pick published its index relative to its kill; the log records
  when each kill was aimed and when it returned, not when it was delivered.
- One reading, **inferred, not established**: each kill was delivered after its pick had already
  published its index.
- Another reading, **inferred, not established**: the picks published their index sooner than the
  probe's median, so every aim fell after it.
- These readings are not exhaustive.
- **Not established: the rate.** One sighting, its first on Linux.

`CI-FLAKE-DISPATCH-SAMPLER-NO-KILL-WHILE-WRITING`
(`findings/P3_correctness_202611220301_the-dispatch-sampler-landed-no-kill-while-writing-on-hosted-windows.md`)
records the same test failing its next assertion, at line 1578, on hosted Windows; this red is a
different assertion, and that file is not edited.

## What the change that takes this up should do

Keep the assertion at line 1570, with no retry, skip or weaker assertion, and make observable when
each pick published its index relative to its kill. The guard names the change.
