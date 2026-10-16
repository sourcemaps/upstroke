---
id: G6-KILL-CHILD-PATH-ENV-UNICODE
severity: P3
disposition: deferred
category: portability
pr: 332
reviewed_sha: 62bbbb4335da20fb51d4aad332a8df6f2c889639
location: src/engine/topology/coordinator.rs:8262
provenance: pre_existing
first_bad:                # undetermined per site: the history under Provenance is pattern-level
guard: orch_pr11b routes the distinct test-portability repair or sweep and reassesses Gate 6 applicability; any full non-UTF-8 sequence keeps its dependency on the production locator, G6-PRIVATE-DIR-LOSSY-LOCATOR.
---

## Failure sequence

**Kill children read a path-valued handoff variable with `std::env::var`, which refuses a value that is not valid
UTF-8.** Static, per site. The mechanism was executed only on #332's C10, which is not on master and is excluded
here.

a test host's temporary directory has a path that is not valid UTF-8, so the fixture root or handoff directory under
it does too -> the parent passes that path to its kill child losslessly (`as_os_str()`, or `Command::env` with a
`Path`) -> the child reads it with `std::env::var(..).expect(..)` -> `env::var` returns `Err(NotUnicode)` and the
child panics before the point where the parent injects its kill -> the parent's assertion about how its child ended
fails, and the test fails on a valid path.

## The inventory: 15 children, each paired with a lossless parent setter

Every line read at `62bbbb43`: `~/orch-pr11/logs/pr11_g6_filing1/sites-at-master.txt` and
`~/orch-pr11/logs/pr11_g6_filing1/t1-pairing-at-master.txt`.

| # | Child (reads with `std::env::var`) | Parent's lossless setter |
|---|---|---|
| 1 | `closure_kill_child_at_width_three`, `src/engine/topology/coordinator.rs:8262` | `kill_inside_closure_recovers_at_width_three`, `coordinator.rs:8298` |
| 2 | `finalization_kill_child_at_width_three`, `coordinator.rs:17295` | `finalization_kill_matrix`, `coordinator.rs:17490`, naming the child through `FINALIZE_CHILD` (`:17238`) |
| 3 | `charged_record_kill_child`, `coordinator.rs:19602` | `killed_at_the_record`, `coordinator.rs:19643` |
| 4 | `event_kill_child`, `src/engine/topology/coverage/tests.rs:970` | `every_kill_point_of_the_event_funnels_kills_the_child_under_the_production_adapter`, `:998` |
| 5 | `spawn_kill_child`, `coverage/tests.rs:1122` | `every_kill_point_of_the_process_funnel_kills_the_child_on_this_host`, `:1166` |
| 6 | `create_kill_child`, `src/engine/topology/create/tests.rs:2430` | `kill_the_creation`, `:4325` |
| 7 | `two_crash_kill_child`, `src/engine/topology/recover/tests.rs:7760`, `:7763`, `:7766` | `unsynced_merge_prepared_two_crash_barrier_before_cas_then_power_loss_keeps_log_and_ref_agreeing`, `:7845`-`:7847` |
| 8 | `answer_ingest_kill_child`, `recover/tests.rs:14034`, `:14037` | `a_kill_at_the_answer_ingestion_converges_on_the_next_incarnation`, `:14181`, `:14182` |
| 9 | `candidate_sequence_kill_child`, `recover/tests.rs:14947` | `kill_the_candidate_sequence`, `:15058` |
| 10 | `process_spawn_kill_child`, `recover/tests.rs:17108` | `a_kill_in_the_workers_spawn_converges_on_the_next_resume`, `:17172` |
| 11 | `container_mount_kill_child`, `recover/tests.rs:17618` | `a_kill_at_the_gate_containers_git_view_mount_is_reclaimed_by_the_next_resume`, `:17667` |
| 12 | `staging_path_kill_child`, `recover/tests.rs:18223` | two parents: `a_kill_before_the_proposals_pin_leaves_a_picked_staging_worktree_the_next_resume_reclaims_and_the_candidate_integrates`, `:18275`, and `kill_the_staging_path`, `:18353` |
| 13 | `kill_child_environment`, `src/engine/topology/scaffold.rs:2306` | `launch_the_kill_child_and_adopt`, `:2283`; paired by the variable they share, not by a child name |
| 14 | `settlement_kill_child`, `src/engine/topology/settle/tests.rs:1479` | `spawn_kill_child`, `:1564` |
| 15 | `publication_kill_child`, `src/rundir/tests.rs:4818` | `a_kill_between_stage_and_rename_leaves_only_the_tmp`, `:4893` |

**Excluded.** The label variables (`SHAPE`, `SITE`, `CELL`, `OUTCOME`, `PHASE`, `COORDINATE`), whose values are fixed
ASCII words the parent writes. Children that already read with `std::env::var_os`
(`src/engine/topology/recover/tests.rs:23118`, `:23121`, `:23178`, `:23196`; `src/workspace_manager/tests.rs:20365`).
The five conversions whose parent itself converts the path lossily into the production runner's `String`-typed
`CommandSpec` (the same `create_kill_child` reached through `spawn_and_wait`, `worktree_lock_child`,
`container_probe_kill_child`, `recovery_kill_child` and `closure_kill_child`): there the child receives another path
rather than a refusal, and that intake is recorded outside this ledger, ungraded, under orch_pr11b. And #332's C10,
which is not on master.

## Evidence

**Executed: the mechanism only, on #332's C10, Linux** (the INV-07 repairer's sealed slot,
`~/orch-pr11/logs/pr11_g6_inv07_fix1/slot/rows/`):

- **W5**, #332's reviewed head `6a0f73f683ad91fd3fbb4e442c2b994318eaaa9d`, `TMPDIR` named with byte 0xff: the parent's
  assertion "the child died at the decline's synced append" failed at `coordinator.rs:13004:9` on #332's branch, with
  `ExitStatus(unix_wait_status(25856))`, the child's exit code 101 (`W5/VERDICT.txt`).
- **W3**, #332's repaired source with only the child's environment read put back from `var_os` to `var`:
  `a_kill_at_the_declines_synced_append_resumes_through_a_non_utf8_handoff_directory`, C10's sequence with a handoff
  directory that is not valid UTF-8, failed the same assertion at `:13001:9` with the same exit status
  (`W3/row.log`, `W3/VERDICT.txt`).
- The child's panic text is not in either saved output. That `env::var` returned `NotUnicode` is read from the source,
  not observed.

**The 15 sites above are static, by pairing.** None was run under a non-UTF-8 path.

**Platforms.** Linux is the executed case, and only for C10. No cross-target build or native run of these 15 sites
under a non-UTF-8 path exists. Other platforms are untested here.

## Provenance

`pre_existing`. `first_bad` is undetermined per site. Pattern-level only: `std::env::var("UPSTROKE_TEST_KILL_DIR")`
first appears under `src/` at `16a80361` (2026-08-23), "rename: tactus -> upstroke on the integration branch", so the
pattern is older under an earlier name (`~/orch-pr11/logs/pr11_g6_filing1/provenance-pattern-level.txt`).

## Severity

P3 `portability`: test-only, no product behaviour.

## Related

- `G6-KILL-ROOT-HANDOFF-LOSSY`: rows 1, 2 and 3, and the scaffold pair of row 13, also carry that finding's lossy
  root handoff. A separate cause, with its own inventory.
- `G6-PRIVATE-DIR-LOSSY-LOCATOR`: under a non-UTF-8 `TMPDIR`, several of these tests also put their run's private
  directory there. With this read repaired they would meet that finding next, so a full non-UTF-8 sequence depends on
  its disposition.
- `PR329-THE-GIT-FLOOR-TEST-HELPER-SPLICES-PATHS-INTO-SHELL-TEXT`: the same class, a test helper reading a path-valued
  variable with `env::var`, at a different site.
- `PR334-RECORD-KILL-CHILD-GITDIR-OVER-THE-WINDOWS-BUDGET`, closed: row 3's child, for a different cause (a Windows
  `$GIT_DIR` length budget).

## What the change that takes this up should do

Not decided here: orch_pr11b routes it. The shape #332's INV-07 repair round took for C10, which is not on master:
read with `std::env::var_os` and build the path from the `OsString`. A sweep would need a witness under a non-UTF-8
`TMPDIR` on Linux that fails on the `var` read, with a catching reversal, for each child class it repairs.

## Disposition

`deferred` records queued separate work under the guard. It does not accept residual risk, does not promise that this
finding may stay open past Gate 6, and waives no `MUST` or witness rule.

Filed by `pr11_g6_filing1` on a findings-only branch, from the proposal of the INV-07 repairer
(`pr11_g6_inv07_fix1`, #332), under babysit's direction of 2026-10-16 (sha256 `aaeccd6a…`) and its two
clarifications (sha256 `876e1f56…` and `29131374…`). Dedup: `~/orch-pr11/logs/pr11_g6_filing1/DEDUP-RECORD.md`.
