# Verification of the open findings the records carry (docs PR 5, 2026-11-24, on `48f2af7a`)

Each id below is recorded under `reviews/` with an open-type disposition (`deferred`, `carried`,
`open`, `accepted-risk`) and no file under `findings/` names it in its `id:`. The first 34 rows
are the candidate list; rows marked "found by the scan" come from one pass over every file under
`reviews/` for ids on a line carrying an open-type word, less those any `findings/` file names
(an id whose disposition sits on a different line from it would be missed). Records are cited as
`48f2af7a:reviews/<file>.md:<line>`.

| id | verdict | evidence | filed as |
|---|---|---|---|
| `PR8-R2-SPEND-REPLAY` | fixed | `src/topology/events.rs:1058` `MergeVerificationUnavailable` carries a required `reviews` list (closed by `7da321c0`, `48f2af7a:reviews/2026-09-16-gate-G5-run3.md:758`) | — |
| `PR8-CRASH-002` | fixed | test `an_empty_ref_lock_left_by_a_killed_compare_and_swap_is_reclaimed_and_the_publication_completes` (`src/engine/topology/recover/tests.rs:11945`), closed by `1c309d49` (`48f2af7a:reviews/2026-09-16-gate-G5-run3.md:759`); residues filed as `PR8-CRASH-002-PACKED-REFS-LOCK` and `PR8-CRASH-002-PACKED-RUN-REF` | — |
| `PR7-R4-LOOP-004` | fixed | test `a_budget_stopped_run_with_a_retained_generation_is_closed_run_ending_and_ends` (`src/engine/topology/recover/tests.rs:19569`); `48f2af7a:reviews/2026-09-12-pr10-record.md:794` | — |
| `PR10-ST14-BOUNDED-CENSUS-DOES-NOT-CLOSE` | fixed | `48f2af7a:reviews/2026-09-12-pr10-record.md:2681` (packet amendment of 2026-09-13, option 3); test `the_prefix_reports_its_truncation_and_every_seeded_census_closes` (`src/topology/census.rs:2047`) | — |
| `PR5-RD-002` | fixed | `src/workspace_manager.rs:4166` `reclaim_intents` removes every worktree before any intent (closed by `dfab458b`, `48f2af7a:reviews/2026-09-25-gate-G5.md:805`); residues filed under `PR5-RD-002-…` ids | — |
| `PR120-TABLE-OMITS-GIT-DISCOVERY-PATHS` | already filed | `findings/P2_security-trust_202609220220_git-discovery-redirect-inside-the-stated-boundary.md` (the residue kept when the P1 closed at `2fad869b`) | — |
| `PR4-SPAWN-SITE-PROBE-CONTEXT` | still exists | `src/topology/effects/sites.rs:1413` `Spawn` is still `After(AttemptStarted)`; `src/runner/contract.rs:1295` census still counts 2 probe roles outside the site's context; `48f2af7a:reviews/FINDINGS.md:3741` | `P2_crash-consistency_202611241010_probe-spawns-are-filed-under-the-attempt-spawn-site.md` |
| `PR3-STRICTNESS-RECURSION` | fixed | `48f2af7a:reviews/FINDINGS.md:822` (§5 Fixed); test `an_unknown_field_is_refused_at_every_object_boundary_of_every_transaction` (`src/topology/events.rs:2479`) | — |
| `PR3-FOLD-001` | fixed | `48f2af7a:reviews/FINDINGS.md:827` (§5 Fixed); test `a_committed_line_that_is_not_an_event_is_a_rewritten_log` (`src/topology/fold/tests.rs:8541`) | — |
| `PR5-SEAMS-004` | fixed | `48f2af7a:reviews/FINDINGS.md:933` (repaired survivors); test `a_legacy_open_that_fails_fails_the_way_the_pre_move_writer_did` (`src/events/log/tests.rs:872`) | — |
| `PR3-ST14-006` | fixed | test `every_deferred_state_has_a_legal_next_transition` asserts a non-empty successor set at the ceiling (`src/topology/census.rs:2396`) | — |
| `PR5-WORKSPACE-048` | already filed | `findings/P3_correctness_202608310826_the-object-group-apis-take-no-site.md` (id `PR5-R2-OBJECT-GROUP-TAKES-NO-SITE`, whose failure sequence opens with this id) | — |
| `PR7-R5-ATT-001` | already filed | `findings/P2_correctness_202609031104_census-domain-cannot-decide-test-reachability.md:22` (an attestation key for that finding, `48f2af7a:reviews/FINDINGS.md:4717`) | — |
| `PR7-WIN-READ-RACING-BOUND-TOO-SHORT` | fixed | `src/runner/container.rs:764` `read_racing` uses the shared `racing_pause` schedule; test `the_racing_pause_is_sixteen_yields_then_forty_seven_sleeps_and_nothing_after_the_last` (`src/runner/container/tests.rs:210`) | — |
| `PR7-WIN-READ-RACING-BOUND-TOO-SHORT-TERMINOLOGY` | obsolete | a terminology correction whose disposition follows the base id (`48f2af7a:reviews/FINDINGS.md:4003`, `:4169`); the base id is fixed | — |
| `PR119-SWEEP-QUEUE-STALE-AT-HEAD` | fixed | `standards/SWEEP.md:43` queues #106, #107, #108, #111 (repair `637b8ae7`, PR #122, which the row names) | — |
| `PR118-R2-SWEEP-QUEUE-STALE` | fixed | `standards/SWEEP.md:68` queues the #107 `src/rundir/*` family (repair `637b8ae7`, PR #122) | — |
| `BRIDGE-CI-SHAPE-TEST-IS-A-SUBSTRING-ORACLE` | fixed | `48f2af7a:reviews/FINDINGS.md:3459` (fixed at `a5f3706`); test `effects::tests::every_platform_this_crate_configures_for_has_a_clippy_gate_the_aggregate_requires` (`src/effects/tests.rs:6911`) | — |
| `FIND-109-CLOSING-CASE-HOLDER-STARVATION` | fixed | `48f2af7a:reviews/FINDINGS.md:6138` ("closed, not carried"); seam `note_removal_attempt` at `src/workspace_manager.rs:2227` | — |
| `FUC-D5-ACCOUNT` | fixed | test T-ACC-1 at `src/engine/topology/ledger/tests.rs:996`; its file was deleted at `b8d14931` (A-12 met its guard) | — |
| `FULL-SC-1` | fixed | same defect as `PR11-REAPER-CONTAINER-SCOPE-UNREGISTERED` (`48f2af7a:reviews/2026-09-30-pr11-record.md:5707`), fixed at `48f2af7a:reviews/2026-10-01-pr11-follow-up-a-record.md:36`; `src/runner/container/exec.rs:409` | — |
| `FULL-SC-4` | already filed | `findings/P3_docs-contract_202610010033_the-budget-overshoot-bound-is-stated-in-no-output.md:46` | — |
| `GUARD-DECISION-SILENT-PRODUCTION-FILES-OUTSIDE-THE-ROLL-CALL` | fixed | `48f2af7a:reviews/2026-09-25-gate-G5.md:798` (#318 filed and closed in range); test `every_unclassified_production_file_states_each_governed_lint_or_inherits_its_forbid` (`src/effects/tests.rs:2219`) | — |
| `PR11-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY` | fixed | `48f2af7a:reviews/2026-10-01-pr11-follow-up-b-record.md:6617` (cases a, b, d fixed at `59d206b3`; c and e already filed); `src/workspace_manager.rs:2535` `tolerant_registry_access` | — |
| `PR11-REAPER-CONTAINER-SCOPE-UNREGISTERED` | fixed | `48f2af7a:reviews/2026-10-01-pr11-follow-up-a-record.md:36` (fixed at `a8ef3fb0`); `src/runner/container/exec.rs:409` `Armed::cover_dormant` | — |
| `R7-CONC-1` | fixed | same defect as the linked-checkouts id (`48f2af7a:reviews/2026-09-30-pr11-record.md:5779`), closed at `48f2af7a:reviews/2026-10-01-pr11-follow-up-b-record.md:6617` | — |
| `R6-REG-03` | obsolete | an error in the record's own text, corrected there (`48f2af7a:reviews/2026-09-25-gate-G5.md:1097`, `:1089` "none is left open") | — |
| `PR262-UNREACHABLE-PATH-ENTRY-ABORTS-PROGRAM-RESOLUTION` | fixed | test `every_path_entry_this_runner_searches_names_a_location_on_its_own` (`src/runner/host/tests.rs:6534`) uses a NUL-bearing name; file deleted at `8456f9c2` | — |
| `PR274-DOCKER-TERMINATION-POLL-COUNTS-YIELDS-NOT-TIME` | fixed | `src/runner/container/tests.rs:4637` `wait_until_terminated_within` waits on an `Instant` budget; file deleted at `19c24bad` | — |
| `PR328-LINKED-CHILD-KILL-WAITS-WITHOUT-A-DEADLINE` | fixed | `src/workspace_manager/fixture.rs:4081` `LinkedChild::kill_within` bounded by `COLLECT_BOUND`; `48f2af7a:reviews/2026-10-01-pr11-follow-up-b-record.md:6620` | — |
| `PR329-A-CREATION-PREFIX-RESUME-REFUSED-ON-A-HELD-CLEANUP-LEASE` | fixed | test `a_resume_over_a_creation_prefix_waits_out_a_lease_copy_a_sibling_fork_kept_from_its_ref_write` (`src/engine/topology/recover/tests.rs:18446`); `48f2af7a:reviews/2026-10-02-pr11-follow-up-c-record.md:5214` | — |
| `PR329-A-DROPPED-RESUMES-RUN-STILL-READ-AS-RUNNING` | fixed | test `a_lease_copy_a_sibling_fork_kept_from_the_first_resume_is_waited_out_before_its_death_is_read` (`src/engine/topology/recover/tests.rs:18465`); `48f2af7a:reviews/2026-10-02-pr11-follow-up-c-record.md:5369` | — |
| `TESTS-CONTAINER-WAIT-HAS-NO-TIME-BOUND` | fixed | `src/runner/container/tests.rs:4637` `wait_until_terminated_within` has a 30 s `Instant` budget; file deleted at `cd3a87ab` | — |
| `FUC-D3-PGIDLAYER` | obsolete | an error in the record's own text, corrected in its §4.9 (`48f2af7a:reviews/2026-10-02-pr11-follow-up-c-record.md:2867`, `:2754`); the open part is `FUC-D2-PGIDREUSE` in `findings/P1_correctness_202610040118_a-delayed-git-child-joins-a-reused-writer-group-under-q.md` | — |
| `R6-C1` (found by the scan) | fixed | deferred at `48f2af7a:reviews/2026-09-30-pr11-record.md:5707`, repaired at `48f2af7a:reviews/2026-10-01-pr11-follow-up-a-record.md:466`; test `a_runner_whose_containers_are_unresolved_keeps_its_reaper_armed_past_its_last_handle_until_the_process_exits` (`src/engine/topology/coordinator.rs:21552`) | — |
| `R6-C2` (found by the scan) | fixed | repaired at `48f2af7a:reviews/2026-10-01-pr11-follow-up-a-record.md:463`; test `a_resuming_incarnations_pre_flight_probe_container_is_killed_by_its_reaper_when_the_coordinator_dies_inside_it` (`src/engine/topology/coordinator.rs:21409`) | — |
| `R6-D1` (found by the scan) | fixed | repaired at `48f2af7a:reviews/2026-10-01-pr11-follow-up-a-record.md:467`; test `a_container_reapers_failed_cancellation_ends_its_caller_through_the_signal_monitor` (`src/agent/proc.rs:9942`) | — |
| `R6-D2` (found by the scan) | fixed | repaired at `48f2af7a:reviews/2026-10-01-pr11-follow-up-a-record.md:468`; test `a_coordinator_that_ends_disarms_its_reaper_and_kills_nothing_at_width_three` (`src/engine/topology/coordinator.rs:21711`) | — |
| `R6-D3` (found by the scan) | fixed | repaired at `48f2af7a:reviews/2026-10-01-pr11-follow-up-a-record.md:469`; test `the_reaper_relay_writer_leaves_no_writer_in_another_threads_fork` (`src/runner/host/tests.rs:5116`) | — |
| `R7-D1` (found by the scan) | fixed | repaired at `48f2af7a:reviews/2026-10-01-pr11-follow-up-a-record.md:464`; test `a_fresh_runs_p4_probe_container_is_killed_by_its_reaper_before_run_started` (`src/engine/topology/create/tests.rs:4927`) | — |
| `R7-D2` (found by the scan) | fixed | repaired at `48f2af7a:reviews/2026-10-01-pr11-follow-up-a-record.md:465`; test `the_reapers_scope_is_the_runners_identity_and_selects_its_first_probe` (`src/engine/topology/coordinator.rs:21430`) | — |
| `APX-R14SPEND` (found by the scan) | fixed | carried at `48f2af7a:reviews/2026-10-01-pr11-follow-up-b-record.md:10521`, file deleted at `9aa4a2f6`; test `a_registry_refusal_after_two_paid_passes_is_recorded_with_both_at_width_three` (`src/engine/topology/coordinator.rs:23788`) | — |
| `PR272-R2-DESIGN-26-OVERPROMISES-UNAVAILABLE` (found by the scan) | fixed | "stays as filed" at `48f2af7a:reviews/2026-10-01-pr11-follow-up-b-record.md:10261`; its file was deleted as closed at `19a6bea0` | — |
| `B-W924` (found by the scan) | fixed | open at `48f2af7a:reviews/2026-10-01-pr11-follow-up-b-record.md:10517`, "FIXED for its demonstrated mechanism" at `48f2af7a:reviews/2026-10-02-pr11-follow-up-c-record.md:5326` | — |
| `FUD-D1-PROGRESS` (found by the scan) | fixed | "Fixed (design)" at `48f2af7a:reviews/2026-10-02-pr11-follow-up-d-record.md:1707`; test `the_final_attempt_passes_a_failure_repaired_by_the_deadline` (`src/workspace_manager/tests.rs:16345`) | — |
| `FUB-D4-B1PREDICATE` (found by the scan) | fixed | regression test and mutation recorded at `48f2af7a:reviews/2026-10-02-pr11-follow-up-d-record.md:3147`, landed with #331 (`62bbbb43`) | — |
| `FUB-D4-B1REMOVE` (found by the scan) | fixed | mutation `m-removal-alone` killed at `48f2af7a:reviews/2026-10-02-pr11-follow-up-d-record.md:3002`, landed with #331 (`62bbbb43`) | — |
| `FUB-D4-RESUME` (found by the scan) | fixed | mutation `m-removepin` killed at `48f2af7a:reviews/2026-10-02-pr11-follow-up-d-record.md:3014`, landed with #331 (`62bbbb43`) | — |
| `FUC-D4-RREF` (found by the scan) | already filed | routed to decision D5 at `48f2af7a:reviews/2026-10-02-pr11-follow-up-c-record.md:3108`; carried by `findings/P2_crash-consistency_202610021249_a-dead-coordinators-windows-ref-write-can-land-after-its-resume-reclaimed-the-lock.md:74` | — |
| `PR128-RESIDUE-UNREACHABLE-OBJECTS-IGNORES-THE-EXIT-STATUS` (found by the scan) | fixed | "Deleted as fixed" at `48f2af7a:reviews/2026-10-02-pr11-follow-up-c-record.md:4013` | — |
| `PR119-SWEEP-QUEUE-STALE-RECURRED` (found by the scan) | fixed | deferred at `48f2af7a:reviews/FINDINGS.md:5624`; `standards/SWEEP.md:44` and `:75` queue the merged #106, #108, #111 families (repair `637b8ae7`, PR #122) | — |
| `PR137-CLASSIFY-INTERRUPTED-RETRY-NEVER-REACHED` (found by the scan) | fixed | handed to `SWEEP-CLASSIFY-001` at `48f2af7a:reviews/FINDINGS.md:6578`; `src/rundir/classify.rs:232` bounds each probe read by `INTERRUPTED_ALLOWANCE`; test `the_scan_stops_on_an_interrupting_source_and_does_not_call_it_absent` (`src/rundir/tests.rs:2327`) | — |
| `PR136-P4-A-KILL-AT-AN-ALREADY-REAPED-CHILD-IS-INDISTINGUISHABLE` (found by the scan) | obsolete | superseded at `48f2af7a:reviews/FINDINGS.md:6376` (pass 3 withdrew the `try_wait` pairing it was a residual of) | — |
| `G4-O1-REJECTION-KEEPS-CANDIDATES-REF-UNWITNESSED` (found by the scan) | fixed | carried at `48f2af7a:reviews/2026-09-10-gate-G4.md:1618`, closed at `48f2af7a:reviews/2026-09-12-pr10-record.md:803` (test `terminal_shape_coverage_table_drives_every_shape_and_each_converges_on_replay`) | — |
| `G4-O2-OVERRIDE-SINGLE-RUNG-EXHAUSTION-UNWITNESSED` (found by the scan) | fixed | carried at `48f2af7a:reviews/2026-09-16-gate-G5.md:888`; its file was deleted as resolved at `0c2084e2` | — |
| `G4B-O3-LIVE-WORKTREE-MISSING-CLOSE-KEEPS-THE-INTENT` (found by the scan) | fixed | closed at `48f2af7a:reviews/2026-09-12-pr10-record.md:799` (test `a_live_worktree_missing_close_reclaims_the_generations_worktree_and_intent`) | — |
| `G4B-O9-EVENT-LOG-ANSWER-READER-NEVER-PRUNES-UNWITNESSED` (found by the scan) | fixed | witness committed per `48f2af7a:reviews/2026-09-16-gate-G5.md:900`; its file was deleted at `90c03dc0` and `414bebbf` | — |
| `SWEEP-CLASSIFY-001` (found by the scan) | fixed | carried at `48f2af7a:reviews/FINDINGS.md:6578`; fixed and its file deleted at `7cff3e42` (`src/rundir/classify.rs:232`) | — |
| `SWEEP-NAMES-005` (found by the scan) | fixed | carried at `48f2af7a:reviews/2026-09-16-gate-G5.md:878`; its file was deleted as fixed at `305a191c` and `51b87893` | — |
| `SWEEP-WORKTREE-012` (found by the scan) | fixed | named at `48f2af7a:reviews/2026-09-12-pr10-record.md:877`; its file was deleted at `85b3d7eb` (the residue sampler's torn-registration closures) | — |
| `PR5-R1-CFG-TEST-SHRINKS-THE-DOMAIN` (found by the scan) | fixed | open for its third part at `48f2af7a:reviews/2026-09-16-gate-G5.md:882`; its file was deleted at `3dae6a14` (classification domain derived from `production_code`) | — |

Totals: 61 ids (34 listed, 27 found by the scan): 1 still exists and filed, 51 fixed, 4 obsolete, 5 already filed, 0 undecidable.
