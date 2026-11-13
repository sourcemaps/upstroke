---
id: CI-FLAKE-CHERRY-PICK-SAMPLER-NO-PICK-UNDER-WAY
severity: P3
disposition: deferred
category: correctness
pr: TBD
reviewed_sha: 47986d013001373cec29f530f75e14f26671e578
location: src/engine/topology/recover/tests.rs:11635
provenance: undetermined
first_bad:
guard: the change that makes the sampler's requirement that at least one kill lands while the pick is writing hold on a slow or unevenly scheduled machine, or replaces the timing sample with a deterministic interruption, keeping the requirement itself
---

# The cherry-pick sampler's kills all missed the pick on macOS

## Failure sequence

`engine::topology::recover::tests::sampled_cherry_pick_child_kills_every_residue_classified_and_recovered`
(`src/engine/topology/recover/tests.rs`, line 11459 at the reviewed commit) spawns a child that
makes a cherry-pick, measures how long the pick takes, and kills later children at delays aimed
inside that time. Among its assertions is that at least one kill landed while the pick was writing
— `killed_while_writing >= 1`, line 11635, added by commit `9f7aafc1` on 2026-09-10. **That is the
assertion that failed.**

**One sighting**, read from the job's log:

| run | job | platform | date | pull request | head |
|---|---|---|---|---|---|
| `43118198459` | `130851553875` | `test (macos-latest)` | 2026-11-11 | #368 | `c43e0ed6` |

The change at that head does not touch this file. The suite's result was **3645 passed, 1 failed,
168 ignored**.

The panic was at `src/engine/topology/recover/tests.rs:11634:5`, with the message beginning:

> none of the 14 kills in 16 spawns landed while the pick was writing — each found a child that
> had not begun (`None`, nothing written) or one that had published its commit (`After`) — so the
> sample interrupted no pick under way

It went on to say the probe measured **19.443125ms** and the ladder followed **2 completions**.

**What the sixteen spawns observed:**

- **thirteen** were killed before they had begun — nothing written (`None`);
- **one** was killed after publishing its commit (`After`);
- **two** completed (exit 0).

In the timeline, one run outlived its kill, which failed at 18.64ms with *"Operation not permitted
(os error 1)"*, and one run completed in 19.872667ms; the other fourteen were killed.

**Rate.** This failure appeared in **1 of the 62** `test (macos-latest)` jobs that completed among
the 86 CI runs created on 2026-11-11 UTC (50 succeeded, 12 failed, 24 were cancelled).

**Why this is its own row.** The nearest relative is the open finding
`findings/P2_correctness_202609131201_the-recover-sampler-cold-probe-sibling.md`, which records a
failure of the same test on the same platform. That is a different assertion — *"no sample died by
the kill"* — and that finding's own guard says a sighting after commit `f837f4ca` is a new
fingerprint. `f837f4ca` is an ancestor of the failing head `c43e0ed6`. So this is filed on its own,
with that finding named as its nearest relative and not as its parent.

**The cause is not established.** What the record shows is that every kill landed either before the
child began writing or after it had finished, on a machine where the pick took about twenty
milliseconds.

## What the change that takes this up should do

- **Establish from further sightings which way the kills are missing:** whether they land early
  because the children start slowly, or late. One sighting does not settle it.
- **Make the requirement hold without depending on the machine's timing, or interrupt the pick
  deterministically.** Either is acceptable; what is not is a schedule that holds only where the
  runner is fast and evenly scheduled.
- **Keep the requirement that a pick under way is interrupted.** Do not weaken
  `killed_while_writing >= 1`, and do not retry or skip the test to get past it.
