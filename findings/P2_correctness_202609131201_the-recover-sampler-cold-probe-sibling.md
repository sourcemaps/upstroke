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
guard: confirmation only: the schedule repair is already in the tree at `f837f4ca` (pull request #259) with its follow-up `62f55943`, and all five sightings precede it; the row closes when the hosted `test (macos-latest)` leg has run this sampler over a stated window of runs after `f837f4ca` with no sighting of this assertion, and a sighting after `f837f4ca` is a new fingerprint against the re-aimed ladder rather than this one. Filed as its own fingerprint rather than as a member of `CLASS-INTERMITTENT-SUBPROCESS-KILL-SETTLE-RESIDUE-FAILURES`: that row names four members and this is none of them
---

## Failure sequence

`engine::topology::recover::tests::sampled_cherry_pick_child_kills_every_residue_classified_and_recovered`
fails intermittently on macOS with *"no sample died by the kill: … a run in which every child
exited cleanly is a run in which nothing was killed — the evidence of 8 samples was of completed
picks, not of kills: [(After, ExitStatus(unix_wait_status(0))), …]"*. At the heads below the
sampler scheduled its kills from a probe of the pick's duration; on a slow or contended macOS runner
every scheduled kill landed after its child had already completed, so the run sampled controls
rather than kills and the vacuity oracle correctly refused it. **The oracle was doing its job; the
schedule was what was wrong — and the schedule is what `f837f4ca` repaired, below.**

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

**It is, however, already named by a row this tree does not hold.**
`RECOVER-CHERRY-PICK-SAMPLER-COLD-PROBE` is this fingerprint, filed on pull request #254 and not in
this repository (`reviews/2026-09-10-gate-G4.md:108`), which is why the five sightings above were
read as unfiled. It is the row `f837f4ca` was written against, and
`docs/internals/engine/topology/recover/tests.md:3425` names the same five runs under it.

## What the change that takes this up should do

**Nothing is missing. The schedule repair is already in the tree, and this row must not commission
it a second time.** `f837f4ca` — *"the two macOS residue samplers aim their kills at the picks they
sample, not at one cold probe"*, 2026-09-10, pull request #259 — with its follow-up `62f55943`,
gives this sampler all four disciplines at once: the budget is the median of three probe picks after
a discarded warm-up (`KillBudget::probed`); it then follows the samples, each next rung aimed inside
the median of the last three completions, so the first child that outruns its kill corrects an
inflated probe (`KillBudget::completed`, `KillBudget::aim`); the kill polls the child to its aim and
spins the last four milliseconds rather than sleeping (`KillableGitChild::run_until`); and the
cherry-pick sampler gained the bounded retry its `T-ATTEMPT` sibling has had since PR7 — a whole
second batch on the re-aimed ladder, the refusal counting kills over every spawn. Both commits are
ancestors of both this row's `reviewed_sha` `2467df32` and this branch's head `47138464`
(`git merge-base --is-ancestor`, exit 0 on all four pairs).

**All five sightings precede the repair.** The latest of them is 2026-09-10T03:22:11Z and
`f837f4ca` is dated 2026-09-10T03:57:26Z; they are the five runs
`docs/internals/engine/topology/recover/tests.md:3425` names as the evidence the change was written
against. None of them is a post-repair observation.

**What is outstanding is native macOS confirmation, and only that.** No sighting of this assertion
after `f837f4ca` has been recorded. What has been measured since is Linux: gate G4 ran this sampler
five times at `7e0110a1`, a head that contains the repair, for **40 spawns, 40 kills, 0 refusals**,
9 of them mid-write (`reviews/2026-09-10-gate-G4.md:1565`), and the test passes alone on the build
box at the head this row was filed at (`1 passed; 0 failed`, exit 0). **Linux passing does not
establish macOS correctness.** The failure is a scheduling margin that only the hosted
`test (macos-latest)` runner has produced; the build box's first pick in a fresh staging worktree is
1.1 ms against 0.94 ms warm, so it never sees that margin, and the repair's own macOS and Windows
arms were reasoned rather than executed. So the round that takes this up looks for a post-`f837f4ca`
sighting on the macOS leg over a stated window of runs, and closes the row on finding none. G4's
forty Linux kills are also expressly not evidence that the re-aiming works: `completed=0` in every
run there, so `KillBudget::completed` was never called.

**A hypothesis about the sibling, offered as one and not as a cause.**
`PR80-MACOS-WORKSPACE-SAMPLER-COLD-PROBE-RECURRENCE` records the same cold-probe mechanism in
`workspace_manager::tests::sampled_git_child_kills_every_residue_classified_and_recovered`, so the
discipline `f837f4ca` gave this sampler may serve that one too; worth trying, not established here.
It does **not** extend to `G4R3-WM-ADD-SAMPLER-FAILS-IN-ISOLATION`, which names PR80's test — the
same test, not a third sampler — and whose two recorded failures are a torn `worktree list` record
and an empty `gitdir`: a refusal by the classifier and a refusal by recovery, not an absence of
kills. Repairing a schedule would not establish that either of those is repaired, and this row
asserts no common cause with it.
