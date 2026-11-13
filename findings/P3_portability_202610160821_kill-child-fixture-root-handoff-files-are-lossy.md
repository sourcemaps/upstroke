---
id: G6-KILL-ROOT-HANDOFF-LOSSY
severity: P3
disposition: deferred
category: portability
pr: 332
reviewed_sha: 62bbbb4335da20fb51d4aad332a8df6f2c889639
location: src/engine/topology/coordinator.rs:8272
provenance: pre_existing
first_bad:                # undetermined per pair: the history under Provenance is pattern-level
guard: orch_pr11b routes the separate test handoff repair or sweep and assesses Gate 6 applicability; it is not grouped silently with production persisted-locator changes (G6-PRIVATE-DIR-LOSSY-LOCATOR).
---

## Failure sequence

**Kill-child handoff files carry the fixture root as lossy text.** Static, per pair. The mechanism was executed on
#332's C10 handoff, which is not on master and is excluded here.

a fixture root has a path that is not valid UTF-8 -> the kill child writes `fixture.root.to_string_lossy().as_bytes()`
into a handoff file, with U+FFFD where the invalid bytes were -> the parent reads the file with
`std::fs::read_to_string` and rebuilds the root from that text -> the parent's root names a different path from the
child's: one that does not exist, or another directory -> the parent's adoption and recovery checks run against the
wrong root, and the test fails or misreads on a valid path.

## The inventory: four writer -> reader pairs

Every line read at `62bbbb43` (`~/orch-pr11/logs/pr11_g6_filing1/sites-at-master.txt`).

| # | Child's writer (`to_string_lossy`) | Parent's reader (`read_to_string`) |
|---|---|---|
| 1 | `closure_kill_child_at_width_three`, `src/engine/topology/coordinator.rs:8272` | `kill_inside_closure_recovers_at_width_three`, `coordinator.rs:8314` (`KILL_HANDOFF`) |
| 2 | `finalization_kill_child_at_width_three`, `coordinator.rs:17309` | `finalization_kill_matrix`, `coordinator.rs:17516` (`KILL_HANDOFF`) |
| 3 | `charged_record_kill_child`, `coordinator.rs:19612` | `killed_at_the_record`, `coordinator.rs:19659` (`RECORD_HANDOFF`) |
| 4 | the scaffold's `hand_off`, `src/engine/topology/scaffold.rs:1745` | `launch_the_kill_child_and_adopt`, `scaffold.rs:2299` (`HANDOFF`) |

**Excluded:** #332's C10 pair, which is not on master; #332's INV-07 repair round made it lossless on #332's branch.

## Evidence

**Executed: the mechanism only, on #332's C10 handoff, Linux** (the INV-07 repairer's sealed slot,
`~/orch-pr11/logs/pr11_g6_inv07_fix1/slot/rows/`):

- **W4**, #332's repaired source with only the root's handoff serialization put back to `to_string_lossy` and
  `read_to_string`: `the_kill_handoff_hands_the_parent_back_a_root_no_string_can_spell` failed at
  `coordinator.rs:13078:9` on #332's branch, "the parent reads back the child's root, byte for byte", left
  `.../unspellable-<U+FFFD>`, right `.../unspellable-\xFF` (`W4/row.log`, `W4/VERDICT.txt`).
- **W1**, both halves of the repair put back: the same failure, and the full C10 under a non-UTF-8 handoff directory
  red at `:13001:9` (`W1/VERDICT.txt`).
- That round-trip test is a native-unit serialization of a constructed path through the actual handoff file. It does
  not create such a root or prove filesystem acceptance or recovery, in particular for a Windows unpaired-surrogate
  value (babysit's ruling, sha256 `e6792739…`).

**The four pairs above are static.** W1 and W4 exercise C10's mechanism, not these pairs.

**Platforms.** Linux is the executed case, and only for C10's pair. No cross-target build or native run of these four
pairs exists. Other platforms are untested here.

## Provenance

`pre_existing`. `first_bad` is undetermined per pair. Pattern-level only: the writer's
`fixture.root.to_string_lossy().as_bytes()` first appears under `src/` at `3853adc2` (2026-08-23) and the reader's
`std::fs::read_to_string(dir.join(HANDOFF))` at `3853adc2` (2026-08-23), "feat(topology): dispatch and attempt (lane F)"
(`~/orch-pr11/logs/pr11_g6_filing1/provenance-pattern-level.txt`).

## Severity

P3 `portability`: test-only, no product behaviour.

## Related

- `G6-KILL-CHILD-PATH-ENV-UNICODE`: the children of pairs 1 to 3, and the scaffold's children through
  `kill_child_environment` (pair 4), also read their handoff directory with `std::env::var` (that finding's rows 1 to 3
  and 13). A separate cause, with its own inventory.
- `G6-PRIVATE-DIR-LOSSY-LOCATOR`: production's persisted locator. Not grouped with it: the guard keeps the two apart.

## What the change that takes this up should do

Not decided here: orch_pr11b routes it. The shape #332's INV-07 repair round took for C10's pair, which is not on
master: serialize the root losslessly in its native unit, not as text, and rebuild it from that. A sweep would need, per
pair, a witness that fails on the lossy round trip, with a catching reversal.

## Disposition

`deferred` records queued separate work under the guard. It does not accept residual risk, does not promise that this
finding may stay open past Gate 6, and waives no `MUST` or witness rule.

Filed by `pr11_g6_filing1` on a findings-only branch, from the proposal of the INV-07 repairer
(`pr11_g6_inv07_fix1`, #332), under babysit's direction of 2026-10-16 (sha256 `aaeccd6a…`) and its two
clarifications (sha256 `876e1f56…` and `29131374…`). Dedup: `~/orch-pr11/logs/pr11_g6_filing1/DEDUP-RECORD.md`.
