---
id: ST07-ADAPTER-EXCLUSION-STALE
severity: P3
disposition: deferred
category: correctness
pr: 321
reviewed_sha: e0f9a1cd1b816c3de3e113baa8e3c925cf766260
location: src/engine/topology/coverage.rs:167
provenance: fix_regression
first_bad: 8f1c133a25e14d9f99b8a83c1c8e06791baa6606
guard: the next change to `ADAPTER_UNIT_TEST_MODULES` in `src/engine/topology/coverage.rs`, or a fix-P3/ branch taking this up: name `runner::contract::tests::` and hold the list to the modules that really hold adapter unit tests; measured harmless at `d724fb16` on both hosts (Gate 5 run 6's F59), so nothing is wrong today
---

## Failure sequence

The ST-07 merge checker decides which observation records count as funnel executions with
`is_funnel_execution` (`src/engine/topology/coverage.rs`). That function excludes every record whose
test lies under one of the modules in `ADAPTER_UNIT_TEST_MODULES` (`:167`):

- `workspace_manager::hooks::tests::`
- `engine::topology::seams::tests::`
- `runner::tests::`

Here is how the list went stale:

1. **#318's `8f1c133a`** (fix(effects): empty the two deny roots into children that forbid, and hold
   them to declarations) moved the adapter unit test `harness_hooks_consult_every_mode_a_point_declares`.
   It was at `src/runner/mod.rs:2198` at `9bb177ea`; it is at `src/runner/contract.rs:2200` at
   `d724fb16`, in module `runner::contract::tests::`.
2. `ADAPTER_UNIT_TEST_MODULES` still names `runner::tests::`, so the moved test's record no longer
   matches any excluded module.
3. The checker therefore counts that adapter record as a funnel execution. The test calls
   `HarnessHooks::point` directly on two of the Windows-only points, recording three point-and-mode
   coordinates, and executes no containment primitive.
4. **The latent failure.** If a Windows-only point were ever reached *only* by that adapter record, the
   bijection check would pass on a record that is not a production-funnel execution.

At `d724fb16` the failure does not occur, on either host.

## Evidence

This is Gate 5 run 6's figure F59, recorded in `reviews/2026-09-25-gate-G5.md` §8.2 and §13, with
receipts under `~/tactus-artifacts/g5-evidence-d724fb1/st07/adapter-exclusion/`. The unchanged checker
was run over a raw copy of each observation export and over a copy without the moved module's records,
which is the one record per export:

| export | raw: records / funnel / witnessed | without the adapter record |
|---|---|---|
| Linux export 1 | 434 / 429 / 163 | 433 / 428 / 163 |
| Linux export 2 | 434 / 429 / 163 | 433 / 428 / 163 |
| guest, native | 428 / 423 / 164 | 427 / 422 / 164 |

- All six runs exited 0, each with one test selected, failures 0 and every copy unchanged.
- The witnessed claims do not move, so no credited witness rested on the record.
- The Windows-only points are reached by genuine funnel executions on the guest.

REGRESSION's round-1 review of #321 found it as R6-REG-02 (P2, executed), and #321 fixed the report's
figures. This product-code row was deferred, because a gate report certifying `d724fb16` cannot change
the range's product bytes. The final review of `832e6ff0` confirmed the deferral in all three lenses:
https://github.com/sourcemaps/upstroke/pull/321#issuecomment-5834621057. The verbatim REGRESSION
section, "Item 2 — R6-REG-02, and the deferred row", reads the source at
`src/runner/contract.rs:2200` and `coverage.rs:167`.

This file also discharges `R1F-ST07-FINDING-FILE`, #321's row saying that this deferral had no findings
file. See the pull request that filed this.

## What the change that takes this up should do

Replace `runner::tests::` with `runner::contract::tests::`, or derive the exclusion from where adapter
unit tests actually live, so that a move cannot silently re-include one. Keep a check that fails when
an entry of `ADAPTER_UNIT_TEST_MODULES` names a module holding no test: that is the shape of this
regression. Re-run F59's comparison: with the list fixed, the raw and filtered counts should agree
(433 / 428 / 163 on Linux, 427 / 422 / 164 on the guest), and every ST-07 check should stay green.
