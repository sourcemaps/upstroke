---
id: PR329-LEGACY-RUNS-IN-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY
severity: P1
disposition: deferred
category: correctness
pr: 329
reviewed_sha: 92c4ca81f9209d218df4534ee71d3445dc2906e1
location: src/workspace.rs:871
provenance: pre_existing
first_bad: predates PR11: the legacy engine's four registry Git children have run untolerant since before PR5 froze `src/workspace.rs`; measured at the Git level on master `92c4ca81`'s argv by PR #329's first design round
guard: follow-up D's implementation, on draft #331 and PROPOSED conditional on the owner's decision O8 (decision B), closes (e1) and (e2) by corrected B1′, and (e1′) and (e2′) by B-PRESERVE conditional on the kept pin's write; this file stays open until that pull request merges, and after it for what remains, R-D1 — a refused attempt whose kept pin cannot be written, where one storage fault can fail both the snapshot's registration and the pin (FUD-D4-PINSAMEFAULT) — until a change keeps the output durable without the pin, or the owner rules on it
---

## Failure sequence

A legacy (schema 1–3) run in one checkout of a repository, and another run, legacy or topology, in
another checkout of it, the main one or a linked one. Each holds its own worktree lock, because that
lock is per checkout, so both run.

    the other run writes a registration into the shared `.git/worktrees/`, one file at a
    time: an add of a gate snapshot or a slot, or a removal
    -> the legacy run's gate-snapshot add, its `git worktree list` after a removal, its
       `git worktree remove --force`, or its resume's `git switch` enumerates the store
       (`add_gate_worktree` `src/workspace.rs:871`, `worktree_is_registered` `:1602`,
       `cleanup_gate_workspace` `:1549`, `switch_branch` `:450`) and dies on the half-written
       entry: "failed to read …/commondir: Success", "failed to read '…/locked'",
       "Invalid path"
    -> the error returns through `?` (`src/engine/attempt.rs:154`, `:178`)
    -> the legacy coordinator answers any attempt error with `discard_uncommitted()`
       (`src/engine/coordinator.rs:544-548`), which is `git reset --hard HEAD` and
       `git clean -fd` (`src/workspace.rs:1230-1235`), and ends the command
    -> the worker's uncommitted edits for that attempt are gone, and the resume runs the
       attempt again, paying for it again

**The same discard follows a registration that stays torn.** A writer killed mid-registration leaves
the entry half written, and the legacy reader dies on that residue exactly as it dies on a write in
flight. The residue stays until its own run's resume repairs it or an operator removes it.

**Executed through the real legacy engine** by PR #329's design round 5
(`~/orch-pr11/logs/pr11_fub_design5/census/probe-v/witness-runs/TABLE.txt`, `preserve_run`, unpatched tree): a legacy
run with one gate, a foreign registration torn right after the candidate is captured and left torn. The run fails with
"failed to read .git/worktrees/d5-static-residue/commondir: Success" and the checkout is clean afterwards: the paid
output was discarded. The resume fails the same way until the residue is removed, and then pays for the attempt again.

**Executed** by design review round 3 of PR #329, by two lenses (the PR11 orchestrator's
`~/orch-pr11/reviews/329-d3-witnesses/`):
- **Regression lens** (`pr329-d3-reg-legacy-chbhff16/result.txt`). With one Git writer per
  checkout, A was held right after opening its registration's `commondir`. B's exact legacy
  snapshot-add argv exited 128: "failed to read …/worktrees/gate-C/commondir: Success". Running
  `discard_uncommitted()`'s reset and clean then restored B's tracked file to `base` and removed its
  new file. A then completed.
- **Concurrency lens** (`pr329-d3-conc-confirm-bygs96b1/result.json`). With A's registration held in
  its empty-`commondir` state, B's legacy snapshot add exited 128, and the discard turned "paid
  worker edits" into "base".

**Measured at the Git level.**
- Round 1 of PR #329 ran the legacy argv concurrently in two linked checkouts
  (`reviews/2026-10-01-pr11-follow-up-b-record.md` §1.2, table C and "The legacy race"). With four
  loops per checkout, 12 and 16 commands failed in 7,200. With one loop per checkout, the shape one
  legacy coordinator per checkout gives, none failed in 5,400.
- Round 3 ran the legacy argv untolerant in the main checkout beside the topology manager's
  tolerant cycle in a linked one (record §2.10). The legacy side failed 13, 10, 14, 5 and 13 times
  in 6,000 commands a run; the topology side failed none in 40,000.

**What PR #329 changes about it.** PR #329's topology accesses attempt again past any writer (record §5.3),
so a legacy writer no longer tears a topology pipeline or verification. What remains is the legacy
engine's own exposure, to legacy and topology writers alike. PR #329's design round 6 moved it, with
corrected B1′ and B-PRESERVE, to follow-up D (the PR11 orchestrator's decision on design review round 5,
`~/orch-pr11/reviews/review-329-d5-triage.md`).

## Grading

**P1.** It was filed at P2 by PR #329's design round 3. Design review round 3 graded it P1 in all
three lenses, and design round 4 re-graded it.
- **The criterion.** MAINTAINING's serious P1 includes "loss or corruption of data in a user
  repository — the engine owns git". `discard_uncommitted()` discards the worker's paid output in
  the user's checkout. Neither a consistent event log, nor a resumable command, nor a low measured
  rate restores it, and the sequence is the production error path.
- **Rate.** Low at one coordinator per checkout. A write in flight is torn for a few file writes,
  but residue stays torn until someone repairs it.

**G6.** All three lenses of design review round 4 of PR #329 agree
(`~/orch-pr11/reviews/review-329-d4-triage.md`):
- **(e1) and (e1′)**, legacy against legacy, a write in flight or residue that stays torn: they do not apply to G6,
  which claims nothing of the legacy engine frozen at PR5. Each remains a P1.
- **(e2)**, a topology writer tearing a legacy reader while it writes: it applies through Q6 across the shared registry
  and R17, and blocks G6 until follow-up D's corrected B1′ is implemented and validated.
- **(e2′)**, a topology writer's static residue (a killed writer's torn registration) or contention that outlasts the
  deadline: the legacy access refuses, and the coordinator then discards the paid output. It applies through Q6, and a
  crash producer engages Q1; no surviving writer is needed, so it is distinct from
  `PR329-A-RESUME-REBINDS-A-SLOT-ITS-DEAD-COORDINATORS-GIT-CHILD-STILL-WRITES`. It blocks G6 **even after B1′**, until
  follow-up D's B-PRESERVE is implemented and validated, unless the owner rules otherwise.
- **Filing waives neither case.**

## What the change that takes this up should do

**Follow-up D takes this up, before G6.** It is `pr11_fud_design`'s change, on branch
`fix-P1/correctness_legacy-runs-in-linked-checkouts-race-the-shared-worktree-registry`: two parts, both the owner's,
each unfreezing PR5-frozen legacy behaviour (decision B; the PR11 orchestrator's escalation item 7). It starts from PR
#329's round-5 text (its record, §4.4 and §4.5, with their exact unfreeze texts) and carries design review round 5's
findings against it: FUB-D5-INDEX (pin the captured candidate, not the mutable index, which unfreezes
`src/engine/attempt.rs` too), FUB-D5-RESTORE (a recovery command that restores deletions) and FUB-D5-UNFREEZETEXT. It
calls PR #329's `tolerant_registry_access`, whose contract is PR #329's record §5.5, so its implementation follows PR
#329's merge.

**Corrected B1′ closes (e1) and (e2): every write in flight that finishes within the deadline, from any writer.**
- `src/workspace.rs`'s three registry Git children each run as one attempt of PR #329's `tolerant_registry_access`:
  `switch_branch`'s `git switch`, `add_gate_worktree`'s `git worktree add`, and `cleanup_gate_workspace`'s
  `git worktree remove` together with the `git worktree list` that decides its success.
- The add is attempted again only while its destination is provably untouched: the caller's veto the contract
  provides. PR #329's design round 8 (its record §7.2 and §7.3, with the dated changes in §5.5) sets that veto for
  both adds:
  - **Untouched.** A destination still an empty directory the access can remove was never taken over by Git; it is
    made again and the add is attempted again.
  - **Possibly taken over.** A destination that is absent, or an empty directory the access cannot remove, answers
    `Undecidable`. The access refuses at once as a registry refusal, so a `git worktree prune` that deletes the
    registration after the takeover is refused and the candidate kept, never returned as Git state and discarded.
  - **Anything else** also answers `Undecidable`.
  - **The probe withdrawn.** Round 7's registry-free checkout probe is withdrawn (#329's FUB-D7-SPLITINDEX and
    FUB-D7-CONFIG), and D adopts no probe.
  - **What round 8 corrects.** The round-5 and round-6 veto (the destination unchanged and empty) read a prune's failure
    after the takeover as the add's own, and a takeover whose junk removal failed as untouched (#329's FUB-D6-PRUNE and
    FUB-D6-INODE).
  - **What D keeps.** A deletion that leaves the add Ok, landing before the add returns or after (#329's record
    §8.3), is `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`'s, which D refers to as R-D9.
- The removal's success decision is inside its attempt, so a removal a torn sibling fails is attempted again; an
  already-unregistered destination still counts as reclaimed.
- One private helper resolves the canonical common git dir as `recorded_objects_scope` does; nothing else in the
  module moves.
- PR #329 adds the access's `effect_free` row in `effects/wrappers.toml`; `src/workspace.rs`'s `legacy_effect` text in
  `effects/allowlist.toml` takes a second amendment.

**B-PRESERVE closes (e1′) and (e2′): residue that outlasts the deadline.** It needs B1′, whose refusal it reads.
- When an attempt ends in a registry refusal, `src/engine/coordinator.rs` does not discard the checkout. It pins the
  attempt's captured candidate, carried from `src/engine/attempt.rs` rather than read from the mutable index
  (FUB-D5-INDEX), at the attempt's prepared-pin name followed by `-kept`, and its refusal names the pin.
- `src/engine/resume.rs` discards the checkout's copy as before, so the attempt runs again from a clean tree. It finds
  the kept pin, names it in its warning with a command that restores deletions too (FUB-D5-RESTORE), and never removes
  it.
- Both modules' `legacy_effect` texts take an amendment, and the regression tests are appended to the frozen
  `src/engine/tests.rs`; no existing test changes.

**The witnesses** (PR #329's record, §4.8, now follow-up D's to carry): T-L1 to T-L6 for corrected B1′ and T-P1 to T-P5
for B-PRESERVE, among them (e2′) itself with a topology slot's torn registration as the residue. T-L1 to T-L4 and T-P1
to T-P3 were executed on a scratch prototype, and `switch_branch`'s sequence at the Git level
(`~/orch-pr11/logs/pr11_fub_design5/census/probe-v/SUMMARY.txt`); every existing legacy test passed on it.

**If the owner declines.** Without B-PRESERVE, (e2′) remains an applicable P1 and blocks G6, and (e1′) remains a P1;
this file stays open, narrowed to the residue. Without either, (e2) and (e2′) both block G6 and the file stays as it
is. The owner may instead rule that the mixed residue case does not block G6, with this P1 kept filed; without that
ruling no waiver is inferred.

## The helper exists (2026-10-03, PR #329's implementation)

Follow-up D calls `tolerant_registry_access` in `src/workspace_manager.rs` (`pub(crate)`, classified `effect_free` in
`effects/wrappers.toml`), with `Again`, `RegistryHold` and the test handshake `CONTENDED_ATTEMPTS` /
`contended_attempts`, as #329's record §5.5 and §7.6 state the contract. The legacy accesses stay D's, and this file
stays open under its guard.

## Follow-up D's implementation (2026-10-03, draft #331): what it closes, and why this file stays

**Implemented, and not granted.** Follow-up D's reviewed proposal is implemented on PR #331's branch, a draft, as its
record's Implementation section gives it (`reviews/2026-10-02-pr11-follow-up-d-record.md`). Its five
`effects/allowlist.toml` texts, and `design/15`'s paragraph, are **proposed, conditional on the owner's decision O8**,
and nothing of it is in force until the owner adopts O8 and that pull request merges, after PR #329.

**What it closes once merged, with its regression tests.**
- **(e1) and (e2), a write in flight:** the three registry children each run as one attempt of PR #329's
  `tolerant_registry_access`. `a_snapshot_add_beside_a_tear_its_writer_finishes_is_attempted_past`,
  `a_snapshot_removal_beside_a_tear_its_writer_finishes_takes_the_registration`,
  `a_branch_switch_beside_a_tear_its_writer_finishes_switches`, `a_legacy_run_completes_past_a_tear_its_writer_finishes`
  (e1) and `a_legacy_run_completes_past_a_topology_slot_its_writer_finishes` (e2), each red at master and under a
  mutation that brings the Git error back.
- **(e1′) and (e2′), residue or contention past the deadline:** the refused attempt's captured candidate is kept in the
  checkout and pinned, and every resume names the pin with commands that take it back.
  `a_registry_refusal_after_capture_keeps_and_pins_the_captured_candidate_across_both_resumes` (e1′) and
  `a_topology_slots_torn_registration_refuses_a_legacy_snapshot_and_keeps_its_output` (e2′), with the recovery tests the
  record lists, each red at master and under the mutation that discards again.

**Why this file is not deleted.** The rule for this file is that it goes if, and only if, D's record says D wholly closes
it (its §1.12). It does not: its claim 2 keeps the captured output "conditional on the pin's write" (the PR11 decision
appendix's §10.3, FUD-D4-PINSAMEFAULT), and its §1.8 closes (e1′) only "conditional on the pin being written". What
remains is **R-D1**, inside this file's consequence, the discard of paid output after a registry refusal:
- **`HEAD` moved after the capture:** the pin is refused, the refusal says so and nothing is discarded
  (`a_kept_pin_that_cannot_be_written_is_reported_and_nothing_is_discarded`); a resume then refuses on the moved branch
  before its discard, by the resume's branch check (`src/engine/resume.rs:532-541`, read, not executed here). Nothing is
  lost.
- **The ref store cannot be written:** the output is only in the checkout, and the refusal says so; once the store is
  writable again, the next resume discards the checkout as at master. **One storage fault can do both:** a common Git
  directory that cannot be written fails the snapshot's registration and then the pin's write. Executed at the Git level
  by design review round 4's restore lens on Git 2.43.0, 2.50.1 and 2.55.0 — the add exits 128, the pin's
  `update-ref` exits 128, no kept ref exists, and the discard restores `base` over `paid output`
  (`/home/ubuntu/orch-pr11/reviews/331-d4-witnesses/review331-d4-restore-9tlo0lw2/*-single_store_fault.json`); the
  engine's consequence is reasoned. Through a tear, the finding's own trigger, it needs an independent ref-store fault.

**Grading and G6, unchanged by this note.** The file keeps P1; nothing here regrades it. (e2) and (e2′) block G6 until D
is merged and validated, which needs O8 and PR #329 merged (O9, O14, O11). The restore lens found R-D1's one-fault case
legacy-only, with no independent mixed blocker shown. No waiver and no acceptance is inferred.
