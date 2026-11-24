---
id: PR10-RECORD-STALE-SENTENCES-AFTER-ROUND-11
severity: P3
disposition: deferred
category: docs-contract
pr: 279
reviewed_sha: 8e215030a0a0278733d3e332d0d2775771f17221
location: reviews/2026-09-12-pr10-record.md:2797
provenance: introduced_by_feature
first_bad: bbd3f2ee
guard: deferred by owner decision 2026-09-14 (#279 merges at the reviewed head; fixed after the merge; Gate 5 judges whether it bears on a pass-rule clause): `fix-P3/docs-contract_the-record-carries-stale-sentences-after-round-11` corrects the five copies in place — the round-8 closure row's "resolves 18 of the 20 locations" (`:2797`; the audit says 23 locations, 9 resolved, 14 unresolved), "the run's own tree by construction" (`:2914`; it is by the record), "The three guards now" introducing four (`:2273`), "`.report-staging/` since round 9" (`:350`; the writer makes `.report-staging-<ulid>` recorded privately) and the reaper-lease flake's first failure as 19:16Z in the record (`:1688`) and the body's Validation paragraph (the scan's population holds `86069bd` at 18:24Z) — and runs the class rule's searches for each pattern over the record, the body, `docs/internals/`, `findings/`, `effects/` and `src/`, stating them in the record's §13
---

## Failure sequence

Found by the round-11 fix-check and record lenses of PR #279 (`~/orch-pr10/reviews/r11/fix-check.md`,
"B3 — audit FIXED, but NOT FIXED in every required copy", "Three introducing four", "Ownership
wording copies"; `~/orch-pr10/reviews/r11/record.md`, findings 2 and 3), by executed inspection.
Round 10's record claims every copy of each false or superseded sentence was found and corrected
under the class rule (`reviews/2026-09-12-pr10-record.md:3040-3046`, the searches listed there
including `by construction`, `three guards`, `.report-staging/` and `18 RESOLVED`); these five
copies survived it. Every line number below is the tree at `8e215030`; the body's is
`~/pr10-evidence/r12/body-before.md`, the body as applied at the reviewed head.

1. `reviews/2026-09-12-pr10-record.md:2797`, the round-8 closure row B1: "The strict audit of
   round 9 (§8: exact line, exact diagnostic, else unresolved) resolves 18 of the 20 locations and
   leaves that control and `run-a-new-tests-unfixed.log`'s census location unresolved, by name".
   Round 10 rebuilt the matcher (`~/pr10-evidence/r10/check-controls-strict.py`: the expected
   operand compared, not only the message) and the audit now says
   "23 panic locations over every before-run log; 9 RESOLVED …; 14 UNRESOLVED"
   (`~/pr10-evidence/r10/control-provenance-strict.log:31`). Round 9's "18 RESOLVED … 2 UNRESOLVED"
   (`~/pr10-evidence/r9/control-provenance-strict.log:27`) was produced by the matcher round 10
   found false — an operand literal matched diagnostic text — and the row's assurance is not
   withdrawn in place.

2. `:2914`, the round-9 closure row for the leftover's residue class: "why that is acceptable — the
   run's own tree by construction, reclaimed inside the same site on either branch". Since round 10
   ownership is by the record, not by construction (`src/rundir.rs:1196-1213`; the record's own
   `:2243` says "by the record"); `by construction` was among round 10's search patterns (`:3045`)
   and this copy survived.

3. `:2273`: "The three guards now:" introduces four — the three named at `:2274-2282`
   (`a_checkouts_deletion_is_made_durable_before_its_intent_is_removed`,
   `a_failed_checkout_barrier_is_retried_before_its_intent_is_removed`,
   `an_absent_checkout_retries_its_parent_barrier`) and, "since round 10",
   `a_failed_execution_root_barrier_is_retried_before_the_intent_is_removed` (`:2284`);
   `three guards` was among round 10's search patterns (`:3045`) too.

4. `:350`: "(`.report-staging/` since round 9; a staging file named for the write,
   `report.json.<ulid>.tmp`, in round 8, and `report.json.tmp` until then)". The writer makes
   `.report-staging-<ulid>` (`src/rundir.rs:1566`; `REPORT_STAGING_PREFIX`,
   `src/rundir/names.rs:129`) recorded privately since round 10; `.report-staging/` was among
   round 10's search patterns (`:3044`).

5. `:1688`, and the body's Validation paragraph on the round-10 code head
   (`~/pr10-evidence/r12/body-before.md:30`): the reaper-lease flake is "46 of the 1,069 full-suite
   logs saved on the box, the first failing one of 2026-09-12 19:16Z". The scan both cite
   (`~/pr10-evidence/r10/reaper-lease-flake-scan.log`) holds at its line 35
   `FAIL 2026-09-12 18:24 /home/ubuntu/eight-logs/86069bd-failed-20260912T182521Z/03-test.log`, the
   earliest of its 46 `FAIL` lines by date; that log's line 927 is
   `test engine::topology::recover::tests::a_host_integration_reaper_holds_the_runs_cleanup_lease ... FAILED`
   and its modification time is 2026-09-12T18:24:51Z. The scan's own summary line (`:73`, "the
   first failing log 2026-09-12 19:16Z (174c7d1)") is what both sentences transcribed, and it
   disagrees with the scan's population. The body's copy stands after the filing commit as well:
   that commit edits the body only to add ledger rows and one Validation sentence.

Sequence: a reader takes the record's or the body's sentence as the fact -> reads a count, an
ownership mechanism, a guard count, a staging name or a first-failure time that the tree and the
evidence no longer bear out -> the record's §13 claim that every copy was corrected in round 10
is false of these five.

None of the five has a behavioural surface: nothing compiles, executes or gates on the record's
text (`src/export.rs` pins `README.md`, `MAINTAINING.md` and two design sections, not the record).

## Why this is deferred

Owner decision 2026-09-14: #279 merges at the reviewed head; this is fixed after the merge, and
Gate 5 judges whether it bears on a pass-rule clause.

## What the change that takes this up should do

Branch `fix-P3/docs-contract_the-record-carries-stale-sentences-after-round-11`. Correct each copy
in place, withdrawing rather than deleting (the record's convention, as at `:2799`); re-derive the
audit's counts from `control-provenance-strict.log`, the guard count from the tests named, the
staging name from `src/rundir/names.rs`, and the first-failure time from the scan's `FAIL` lines
sorted by date, rather than from the prose. Run the class rule's searches for each pattern
(`18 of the 20`, `by construction`, `three guards`, `.report-staging/`, `19:16Z`) over the record,
the body, `docs/internals/`, `findings/`, `effects/` and `src/`, and state them in §13. The body's
copy is corrected by whoever next edits the body; the orchestrator rewrites the Review evidence
section at merge time.
