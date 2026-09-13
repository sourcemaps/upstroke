---
id: PR281-RECOVER-SAMPLER-NO-SAMPLE-DIED-BY-THE-KILL
severity: P2
disposition: deferred
category: correctness
pr: 281
reviewed_sha: 2467df322b17ba8ee93eb345e1a19367c0b75e37
location: src/engine/topology/recover/tests.rs:10275
provenance: pre_existing
first_bad: PR80-MACOS-WORKSPACE-SAMPLER-COLD-PROBE-RECURRENCE is the same mechanism in a different sampler, not this one
guard: the round that takes up the cherry-pick recovery sampler, alongside `PR80-MACOS-WORKSPACE-SAMPLER-COLD-PROBE-RECURRENCE` and `G4R3-WM-ADD-SAMPLER-FAILS-IN-ISOLATION`. Filed as its own fingerprint rather than as a member of `CLASS-INTERMITTENT-SUBPROCESS-KILL-SETTLE-RESIDUE-FAILURES`: that row names four members and this is none of them
---

## Failure sequence

`engine::topology::recover::tests::sampled_cherry_pick_child_kills_every_residue_classified_and_recovered`
fails intermittently on macOS with *"no sample died by the kill: … a run in which every child
exited cleanly is a run in which nothing was killed — the evidence of 8 samples was of completed
picks, not of kills: [(After, ExitStatus(unix_wait_status(0))), …]"*. The sampler schedules its
kills from a probe of the pick's duration; on a slow or contended macOS runner every scheduled kill
lands after its child has already completed, so the run samples controls rather than kills and the
vacuity oracle correctly refuses it. **The oracle is doing its job; the schedule is what is wrong.**

**Five sightings between 2026-09-09 and 2026-09-10**, each read from its own job log. **Two are on
`master` itself.** The assertion moved from `tests.rs:9413` to `:10133` by successive splits and
sits at `:10275` at this head; it is one assertion throughout.

| run | job | date | branch | head |
|---|---|---|---|---|
| `34304029954` | `102316933501` | 2026-09-09T02:38:39Z | `gate/g3` | `828da6cd` |
| `34328230257` | `102390406978` | 2026-09-09T08:16:37Z | **`master`** | `9a6897ea` |
| `34353183264` | `102471596760` | 2026-09-09T12:48:29Z | `feature/pr9-repair-execution` | `fbf3e50b` |
| `34356671343` | `102482993424` | 2026-09-09T13:22:34Z | **`master`** | `74da2cbb` |
| `34433061085` | `102732429008` | 2026-09-10T03:22:11Z | `gh-readonly-queue/master/pr-258-81ee09ef` | `bfcb073a` |

A sixth failure of the same test in the window — job `101720525156`, run `34115245433`,
`feat/pr8-integration-transactions` — is **not** this fingerprint: it is *"run 4: the resume did not
converge: git error: git update-ref --no-deref -d refs/upstroke/runs/…"* at `tests.rs:8302`, a ref
deletion, and it is not counted here.

**Why this is a row and not a member of an existing one.** `PR80-MACOS-WORKSPACE-SAMPLER-COLD-PROBE-RECURRENCE`
records this exact mechanism — *"every scheduled kill lands after its child has already completed;
all 32 observations come back `Completed`"* — but names one test,
`workspace_manager::tests::sampled_git_child_kills_every_residue_classified_and_recovered`, in a
different module. This is the **recovery** sampler, and repairing PR80's sampler does not reach it:
PR80's own remedy is scoped to *"this sampler"*. `CLASS-INTERMITTENT-SUBPROCESS-KILL-SETTLE-RESIDUE-FAILURES`
limits membership to four named fingerprints and this is none of them.

## What the change that takes this up should do

Give the cherry-pick recovery sampler the discipline PR80 asks for in its sibling — warm-up probe
discarded, median-of-three, recalibration against actual duration, bounded retry — and demonstrate
on a controlled macOS repetition that a kill actually lands, without masking the vacuity oracle
that caught this. Take it together with `PR80-…` and `G4R3-WM-ADD-SAMPLER-FAILS-IN-ISOLATION`: the
three are one schedule defect in three samplers, and fixing one in isolation leaves the other two
red.
