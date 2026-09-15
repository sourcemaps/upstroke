---
id: PR10-RECORDED-STAGING-LEFTOVER-UNCLASSIFIED
severity: P2
disposition: deferred
category: crash-consistency
pr: 279
reviewed_sha: 8e215030a0a0278733d3e332d0d2775771f17221
location: src/topology/effects/residue_authority.rs:987
provenance: introduced_by_feature
first_bad: 9355c36d
guard: deferred by owner decision 2026-09-14 (#279 merges at the reviewed head; fixed after the merge; Gate 5 judges whether it bears on a pass-rule clause): `fix-P2/crash-consistency_a-recorded-staging-leftover-has-no-residue-class` gives the recorded, unreclaimed staging directory its class in `residue_authority.rs` and the pinned `effects/sequential-registry.json` (a typed site, its cell, its rows, the mutation that the classification is asserted executed and killed) or names the existing row that covers it, and says in the record what class it is; until then Gate 5's residue clause ("residue-class evidence complete and labeled") is judged with one observed residue of the run's own making outside the registry's vocabulary
---

## Failure sequence

Found by the round-11 fix-check lens of PR #279 ("B2's recorded-residue requirement — NARROWED
beyond what this brief allowed. P2"; `~/orch-pr10/reviews/r11/fix-check.md`), by executed registry
inspection. Every line number below is the tree at `8e215030`.

What a report writer that dies between `Report.Write`'s two hook phases leaves — since round 10
(`9355c36d`): the record `<private>/report-staging.json` naming the directory, the directory
`<public>/.report-staging-<ulid>` and the staged `report.json` inside it — is the run's own residue
by the record (`write_report`'s rustdoc, `src/rundir.rs:1196-1213`), and it has no residue class:

- `ReportSite::before_state` answers `BeforeState::Absent` for `Write`
  (`src/topology/effects/residue_authority.rs:985-989`, "Nothing: the write produces the report it
  writes"), and `RunDirSite::before_state` answers `Absent` for `WriteReport` (`:836-837`). Both
  before-phase entries of `effects/sequential-registry.json` for `RunDir.WriteReport` and
  `Report.Write` carry `"rows": []` with the detail "nothing has been performed, so no row holds
  anything"; both after-phase entries carry `["r21"]` for the published report. The four entries
  name `kill_after_report_before_each_cleanup_step` as their executed evidence.
- The record (`reviews/2026-09-12-pr10-record.md:2241-2247`) says so and calls it acceptable: "The
  residue of a death *inside* the report site's primitive, between its two hook phases — the record
  in the private half and the directory it names in the public one, with the staged file inside —
  is the run's own residue by the record, and no registry row covers it: the sequential registry
  carries no coordinate between a Report site's phases (the Event sites' points are its only
  intra-primitive coordinates), so the leftover is unclassified in the registry's vocabulary and
  stated as such rather than given a class the registry does not hold; a staging-shaped directory
  no record names is not the run's residue at all. Why that is acceptable: …" — and the row at
  `:2914` repeats it.

The round-10 repair brief (`~/orch-pr10/repair-279-r10.md`, B2) required: "The residue class of a
recorded, unreclaimed staging directory (a dead writer's) is the run's residue: state it in the
registry properly (typed site, cell, mutation) or say honestly which existing row covers it; an
unrecorded directory is not the run's residue at all." It did not offer "unclassified and
acceptable" as an alternative; an earlier brief had, for round 9's shape. The three reclaim recipes
(`report-leftover-not-reclaimed-on-write`, `report-leftover-not-reclaimed-on-fresh`,
`report-leftover-not-reclaimed-unit`, each captured `101` in
`~/pr10-evidence/09d6887a0956ee4b032f75232f7b2a397f927c82/mutations/summary.tsv`) establish
eventual removal; they do not establish the classification, and the justification the record gives
for the omission rests in part on the third resume's assertion, which
`PR10-ST18-THIRD-RESUME-LEFTOVER-ASSERTION` finds vacuous for the directory.

Sequence: a writer dies between its stage and its rename (the shape
`a_report_whose_staged_file_will_not_sync_is_not_published`, `src/rundir/tests.rs:3188`, drives by
a refused barrier) -> the run directory holds a directory of the run's own making, recorded in its
private half, with a half-written report inside -> the residue authority, asked what the report
site's before phase can find durable, answers `Absent`, and the registry's two before-phase rows say
nothing has been performed -> the ST-07 census, the ledger's R21 row and Gate 5's residue clause
("every observed residue classified and recovered"; "residue-class evidence complete and labeled",
`~/tactus-artifacts/G5-DEFINITION.txt`, lines 19 and 89) have no word for the state, so a
kill-sampling record that observes it can neither classify it nor show it recovered under a named
class -> the record states the omission and calls it acceptable, on a justification the P1 above
shows unestablished.

**Reachability.** The state is reached by the same path as
`PR10-RECLAIM-RECORD-DROPPED-BEFORE-DURABLE-DELETION`'s third step: any death inside
`Report.Write`'s primitive after `begin_report_staging` returns (`src/rundir.rs:1264`) and before
`publish_report`'s rename (`:862`). ST-18 does not fault that coordinate (the crash lens's matrix
row "Inside the new private staging-record protocol": empty), and the will-not-sync test reaches
it by a refused file barrier rather than a kill.

## Why this is deferred

Owner decision 2026-09-14: #279 merges at the reviewed head; this is fixed after the merge, and
Gate 5 judges whether it bears on a pass-rule clause.

## What the change that takes this up should do

Branch `fix-P2/crash-consistency_a-recorded-staging-leftover-has-no-residue-class`. Give the
recorded leftover its class in the residue authority. The reading the cancelled round-12 session
reached and did not apply (`~/orch-pr10/handovers/repair_279_r11b.md`): a before-state class of
its own for `RunDir.WriteReport` and `Report.Write` — R21 holding a predecessor execution's staged
temporary and record, reclaimed by the record — with the oracle, the counts, the semantics arm and
the pinned registry (`effects/sequential-registry.json`, both before-phase entries) regenerated;
`PrecursorDurable` does not fit (its pairing rule and its "always durable" claim, which the round-9
fix-check lens already found false of this site, record `:2236-2241`). Or name an existing row
that covers it, with the reason. Either way: the executed mutation that the classification is
asserted (a registry entry carrying `rows: []` for the recorded leftover fails the pinned-registry
test), and the record's `:2241-2247`, `:2914` and §11 say what class it is; an *unrecorded*
staging-shaped directory is not the run's residue and is passed over — say that too. The diff
reaches `effects/`, which `CLAUDE.md` lists among the known instruments: whoever takes it up
classifies the diff under the first limb before merging, and expects the owner's merge or a
delegation written for that pull request.
