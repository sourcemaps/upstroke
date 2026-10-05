---
id: INV07-L13
severity: P2
disposition: deferred
category: correctness
pr: pending
reviewed_sha: 5c222ff2162da28aa471433773bf4966a883a428
location: src/topology/fold/apply.rs:438
provenance: pre_existing
first_bad: b791cef32b6e647d8433a5d59fa5359382aaf97e
guard: a separate change, under an owner ruling on the frozen fold and recovery modules, that gives every started verification exactly one terminal when a lineage failure intervenes, live and across a resume; not deferred past G6, which cannot pass on a predicate this fails at its final range; and in any case before any writer of task_spawned lands and before activation (PR12)
---

## Failure sequence

At `5c222ff2`, a lineage-failure carrier releases a started integration verification, and no terminal is ever
appended for it:

1. `merge_verification_started(s)` opens a `VerificationStarted` transaction for the candidate of lineage member A.
   On the stale-clean path, the staging worktree for `s` and the pin `prepared/<s>` now exist.
2. Another member B of the same lineage is in flight, having been registered by `task_spawned`. B settles with an
   embedded question that is then declined (`question_answered{Declined}`), or B settles `attempt_finished{Failed}`.
3. The fold's `fail_lineage` releases the transaction without any terminal (`src/topology/fold/apply.rs:428-439`). The
   decline is admitted on purpose while the transaction is `VerificationStarted`
   (`src/topology/fold/check_end.rs:114-136`).
4. The concurrent coordinator stops the verification's pipeline, discards its late result and carries on
   (`src/engine/topology/coordinator.rs:821-849`, `:603-607`). Its warning says "nothing is appended for it".
5. The run ends `run_finished(Complete)`. After a halting decline it ends `run_finished(Halted)` instead: closure
   appends `attempt_interrupted` for the in-flight attempts and nothing for `s`, because `closure::in_flight` reads only
   the fold's transaction (`src/engine/topology/closure.rs:116-151`). The halt cancelled `s`'s running gate.

**The wrong result:** `s` has a start and no terminal, permanently.

- **The terminal can never be appended.** The fold refuses `merge_verification_interrupted(s)`,
  `merge_verification_unavailable(s)` and `merge_prepared(s)` with "the open transaction is none". INV-07's own
  recovery action, "append the missing interrupted terminal", cannot be taken.
- **Recovery skips it.** Step (f) never settles `s`, because it acts only on `fold.transaction()`
  (`src/engine/topology/recover.rs:1102-1104`). It prunes the pin and staging of `s` as stale residue instead.
  - So T-VERIFY's resume action, which begins "append merge_verification_interrupted; delete pin expected-old; reclaim
    snapshots", is never taken.
  - That holds after a shutdown, whether `s`'s pipeline had stopped or was still running when it came, and after an
    error returned at the carrier's own append once its line is synced. Each resume appends only the in-flight
    attempt's `attempt_interrupted` and `run_resumed`, and the run ends `Complete` with `s` still open.
- **The report says it is still open.** report.json and status show `transaction: null` next to an integration-ledger
  row for `s` with `terminal: "open"` and cost `? (unknown)`. That contradicts `design/26:174`: "ledger/status count
  exactly one of them".
- **Nothing is released at a terminal, because there is none.**
  - R1 says the pipeline entitlement is "released exactly once at task_candidate_created, generation Closed, settlement
    to RetainedIdle, or the transaction terminal". The fold's count of held pipeline entitlements falls from 1 to 0 at
    the decline with no terminal, and the 1 was the transaction's.
  - R2 says the merge entitlement is "released exactly once at the transaction terminal".
  - R10 says the staging worktree is "pruned after the terminal event".
  - R12 says the pin is "pruned expected-old at interrupted, Deferred, or Parked terminals".
  - None of that happens at a terminal. The carrier releases both entitlements, and the staging worktree and the pin
    persist until terminal finalization or the next process's recovery.
- **The outcome equations still hold.** R1/R2 are zero at run end and R10/R12 are pruned by finalization, so the
  resource ledger's check finds no disagreement. The log replays equal to the live fold.

**What it contradicts, and what it follows.**

- It contradicts packet v17 INV-07, `design/15:156`, `design/26:169-174` and `:397`; the packet's closure procedure
  step (2); T-VERIFY (`transaction_fault_matrix[11]`), both its resume action and "Halted cancellation appends the
  interrupted terminal"; T-APPEND's "the next resume follows the fault row of the surviving prefix"; and R1's and R2's
  release at the transaction terminal.
- It follows `design/26:292-296` ("A matching `VerificationStarted` transaction is cancelled") and `:309-311`. Those
  lines entered in the same commit as the fold change (`first_bad`), so DESIGN.md contradicts itself, and the packet
  states only the unconditional rule.

**Reach at `reviewed_sha`: latent.**

- Step 2 needs a second, concurrently active member of the verifying lineage, and only `task_spawned` can register
  one.
  - No non-test code appends `task_spawned`; every construction site is under `#[cfg(test)]`.
  - No PR in the v0.2 packet's sequence schedules a writer.
  - A lineage built by `merge_rejected` alone has no member that can settle or be questioned while its verification is
    open: the fold refuses dispatch of the root (`AwaitingRepair`) and of the repair (`AwaitingMerge`), and refuses a
    bare question in the lineage.
- The topology engine is inactive at this commit: `TOPOLOGY_ACTIVATION = Inactive`, `MAX_READABLE_SCHEMA == 3`.
- The project's own tests build the precondition by planting `task_spawned`, and pin the no-terminal outcome:
  - `a_declined_embedded_question_stops_the_verification_its_lineage_had_started`;
  - `a_decline_that_cancels_the_open_verification_goes_on_to_integrate_the_queued_candidate`;
  - `a_stop_answers_a_request_its_intake_still_buffers`;
  - `declining_an_embedded_question_cancels_its_lineages_unprepared_verification`.

**Executed evidence** is in the PR11 orchestrator's INV-07 assessment, `evidence/inv07/REPORT.md`:

- the existing tests, green;
- the coordinator's non-halting, halting and shutdown-then-resume variants;
- a shutdown requested while `s`'s pipeline is still running, then a resume (E14), and an error returned at the
  carrier's own synced append, then a resume (E15): the two review lenses' witnesses, re-run unchanged;
- the fold's refusal of the missing terminal, and its release of the transaction's pipeline entitlement with no
  terminal (E16);
- the resource-ledger record;
- the rendered report row.

**Why P2 and not a serious P1.** The failure needs a speculative precondition at this head: there is no shipped
writer of `task_spawned`, and the engine is inactive. It reaches no DESIGN.md §4 invariant; replay equals live. The
recorded run stays reproducible. It is raised to P1 if a writer of `task_spawned`, or any other route to two
concurrently active members of one lineage, lands while this is open. The resume variants and R1's release point need
the same precondition, so they do not change the grade.

## G6 pass-rule determination

The orchestrator's addendum of 2026-10-03 directs this. The assessment's §6.4 gives it in full.

- **The grade clears one limb only.** As a P2, this finding does not trip G6's "no open critical/high finding" (the
  PR11 orchestrator's G6 plan, `g6/PLAN.md:160-168`). That is all the grade clears.
- **The mandatory predicates stay binding.** At the pinned commit, a run that crosses L13 fails each of these, quoted
  as the packet states them:
  1. run-end closure, "halt cancels in-flight pipelines and settles them interrupted": a halting decline ends `Halted`
     with no terminal for `s`;
  2. shutdown and resume settlement, "shutdown with in-flight pipelines: ... resume settles interrupted": the executed
     sequence is `started(s1)`, a same-lineage decline, a shutdown before `s1` ends, a resume, and `Complete`, leaving
     `s1` with no terminal;
  3. Q1's "reclaimed or repaired per the fault-injection registry", with T-VERIFY's resume action: the records of `s`
     are reclaimed as residue, and the interrupted terminal is never appended;
  4. Q2 with R1 and R2: both entitlements are released at the carrier, not at the transaction terminal. R10's and R12's
     lifecycles and `design/26:174`'s count fail too, while the outcome equations hold;
  5. Q5 at the carrier's own append boundary: at `Event.Append`'s `Synced` point, the resume does not take the tabled
     recovery action;
  6. Q6: atomic settlements and closure, for the sibling interleaving.
- **G6 cannot pass on any of these predicates while it fails.** Naming the negative answer, or this finding, does not
  clear it.
- **No deferral, no waiver, no exception.** Nothing here defers a predicate past G6 or waives a guarantee, and the
  precondition's latency bears on the severity alone. `disposition: deferred` is the ledger's form for an open finding,
  and it defers nothing past G6.
- **Applicability is resolved, and reviewed, at G6's final range.** That is G6's obligation. The evidence is at the
  pinned commit, follow-up A's merge. It feeds that resolution and does not replace it.

## What the change that takes this up should do

Take it as its own change. It is not part of the O14 accounting proposal, whose open item H4 should only cite this
finding. Keep packet INV-07 and every obligation it implies exactly as stated.

1. **Leave exactly one terminal.** When a lineage failure (`question_answered{Declined}`, or a lineage member's
   `attempt_finished{Failed}`) meets an open `VerificationStarted` transaction, leave exactly one of the four terminal
   records for it, and meet what the contract already requires of that terminal:
   - the closure procedure step (2) and `transaction_fault_matrix[11]`: the interrupted terminal, the pin deleted
     expected-old, the snapshots and staging reclaimed;
   - the R1/R2/R10/R12 lifecycles, R1's and R2's release at the transaction terminal included;
   - INV-13's and INV-14's accounting;
   - `design/26:174`'s ledger count;
   - the same across a resume: after a shutdown, or an error returned at the carrier's own append, the resume follows
     T-VERIFY's resume action.
2. **Make DESIGN.md state one rule.** Bring `design/26:292-296` and `:309-311`, the only texts that say otherwise,
   into line with INV-07 in the same change.
3. **Get the owner's ruling on the freeze first.**
   - The change edits the frozen fold (`src/topology/fold/`), probably the coordinator's abandon path, and possibly
     recovery and closure.
   - G6 claims the fold and recovery modules are byte-identical to G5. Landing it before G6's range is fixed needs that
     ruling, together with a ruling on G6's module-diff proof.
   - There is no route after G6: G6 cannot pass on a predicate this still fails at its final range.
4. **Change the pinning tests with it.** The four tests named above pin today's no-terminal outcome. Add a regression
   test that drives steps 1–5, and fails on today's shape. Include the halting variant and both resume variants: a
   shutdown while `s`'s pipeline is still running, and an error returned at the carrier's synced append.
5. **Have G6 resolve this finding at its final range.** Its report names the finding. At that range, it resolves and
   reviews whether each predicate of the G6 section above still fails for a run that crosses L13: the run-end closure
   clause, the shutdown clause, Q1, Q2, Q5 and Q6. G6 cannot pass on a predicate while it fails, and naming the finding
   does not clear it.
6. **Deadline:** before G6 can pass on any predicate this still fails at G6's final range; and in any case before any
   writer of `task_spawned`, or any other route to two concurrently active members of one lineage, lands, and before
   activation (PR12).

Nothing here proposes amending INV-07 or any obligation. Doing so would be the owner's own act.

## Implementation status, 2026-10-05

**Open.** This file stays present, and the finding stays open, at the head of branch
`fix-P2/correctness_a-lineage-failure-releases-a-started-verification-with-no-terminal`. That branch carries the change
that takes it up only as a conditional draft, for a draft pull request: the change has had no implementation review,
and it needs the owner's resolution of T-VERIFY and the owner's freeze and G6 rulings before it can land.
`disposition: deferred` keeps the ledger's meaning above, open pending that review and those adoptions. It defers
nothing past G6, past any writer of `task_spawned`, or past activation.

**The demonstrated conditional remedy, distinct from formal closure.** The branch implements the reviewed remedy,
owner package `INV07-REMEDY-PROPOSAL.md` (sha256
`bf80b5ff82b303e064ff4b60262d3f32b3a64cfe9b8c7eee9fa8944ac0b6e2dc`), as written: RULING L-1 (`d29df05e`), the
non-frozen engine change (`ec77cf90`), RULING L-3, severable (`bc099d46`), RULING L-2 (`9ede05d4`, `98c8b8e7`), the
coordinator tests (`aea697c6`), the `design/26` errata (`3fd67835`) and the internals notes (`c6d28728`, `fba5c8dc`).
At its tested head, `98c8b8e7`, the four tests named above, as the change rewrites them, and sixteen new ones were red
at `reviewed_sha` with only the tests applied and green at the head; each of the remedy's fourteen mutations turned
some of them red; and the ten gates passed. That demonstrates the conditional remedy. It does not close this finding.

**Nothing above is waived.** The obligations in the sections above, the severity and the G6 pass-rule determination
stand as written. T-VERIFY's "candidate retained; reverify under a new sequence" stays binding and unresolved for a
verification a lineage failure cancels: the remedy leaves that candidate failed and never re-verified, and its
conformance for that case rests on the owner's explicit choice of (a) erratum E-L13-1, (b) a ruling that retention
binds, under which this remedy is not the change, or (c) the owner's own confirmation. None is chosen. RULINGs L-1 to
L-3, O12(a) and O14 are unadopted.

**Step 3 above is historical.** "Get the owner's ruling on the freeze first" was written before the change was
drafted, and it stands as the account of that moment. It does not negate the authority granted since, a conditional
draft implementation after the design review, with the owner's adoption gating only the merge; the draft was built
under that authority. The rulings are still required before the change lands, and no rule is waived.

**`pr` is pending.** The pull request does not exist yet, so no number is recorded. The field is filled at this file's
next necessary record touch before merge.

**Closure.** Once the finding is actually resolved, this file is deleted under the standing rule, in a later
necessary commit.
