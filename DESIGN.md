# upstroke — Design Document v2.1

> **Name:** `upstroke` — the conductor's preparatory lift: the rising beat that cues the performers and sounds no note itself, which is exactly the engine's relation to the agents it dispatches. Verified free on crates.io and npm (2026-08-08, live API check). Known adjacent collision: AnthusAI/Upstroke, an alpha Lua DSL for agent orchestration (~3★) — assessed as tolerable, but the decision is deliberate: we differentiate hard rather than share ground by accident. **Action on repo creation: publish a placeholder crate immediately.**

**Status:** v2 — consolidates the original architecture, the two-phase lifecycle, the interaction model, the capacity engine, and two rounds of research whose companion reports are maintained in the strategy record outside this repository, plus the v2.1 late-binding refinement: connect your plans; tiers bind to concrete models and pools at attempt time.
**Language:** Rust · **License:** Apache-2.0, relicensed from AGPL-3.0-only on 2026-09-01 · **Form factor:** single static binary, Windows first-class

---

This file is the index. The design lives in `design/`, one numbered section per
file, and **it is the only living authority for product design.** Section
numbers are the API: code, reviews, and the standards cite `DESIGN.md §N`, so a
number is never reassigned. New sections append; a retired section keeps its
number and says so. When a decision changes the design, the section changes in
the same pull request — there is no separate record to keep in step.

| § | Section | File |
|---|---|---|
| 1 | Summary | [design/01_design_summary.md](design/01_design_summary.md) |
| 2 | The nine pillars | [design/02_design_pillars.md](design/02_design_pillars.md) |
| 3 | Goals and non-goals | [design/03_design_goals_and_non_goals.md](design/03_design_goals_and_non_goals.md) |
| 4 | Invariants | [design/04_design_invariants.md](design/04_design_invariants.md) |
| 5 | The work unit: a two-phase lifecycle (P7) | [design/05_design_work_unit_lifecycle.md](design/05_design_work_unit_lifecycle.md) |
| 6 | Architecture | [design/06_design_architecture.md](design/06_design_architecture.md) |
| 7 | Core data model | [design/07_design_core_data_model.md](design/07_design_core_data_model.md) |
| 8 | Trait surface | [design/08_design_trait_surface.md](design/08_design_trait_surface.md) |
| 9 | Plan ingestion (P1) | [design/09_design_plan_ingestion.md](design/09_design_plan_ingestion.md) |
| 10 | Routing (P3) — three sources, then capacity and affinity | [design/10_design_routing.md](design/10_design_routing.md) |
| 11 | Verification ladder (P4) | [design/11_design_verification_ladder.md](design/11_design_verification_ladder.md) |
| 12 | Interaction model (P8) | [design/12_design_interaction_model.md](design/12_design_interaction_model.md) |
| 13 | Capacity engine (P9) | [design/13_design_capacity_engine.md](design/13_design_capacity_engine.md) |
| 14 | Execution engine | [design/14_design_execution_engine.md](design/14_design_execution_engine.md) |
| 15 | Event log, resume, run layout (P6) | [design/15_design_event_log_resume_run_layout.md](design/15_design_event_log_resume_run_layout.md) |
| 16 | Agent adapters (P2) | [design/16_design_agent_adapters.md](design/16_design_agent_adapters.md) |
| 17 | Configuration reference | [design/17_design_configuration_reference.md](design/17_design_configuration_reference.md) |
| 18 | CLI surface | [design/18_design_cli_surface.md](design/18_design_cli_surface.md) |
| 19 | Failure handling | [design/19_design_failure_handling.md](design/19_design_failure_handling.md) |
| 20 | Safety and permissions | [design/20_design_safety_and_permissions.md](design/20_design_safety_and_permissions.md) |
| 21 | Versioned scope | [design/21_design_versioned_scope.md](design/21_design_versioned_scope.md) |
| 22 | Adopted from the field (with credit) | [design/22_design_adopted_from_the_field.md](design/22_design_adopted_from_the_field.md) |
| 23 | Risks and kill criteria | [design/23_design_risks.md](design/23_design_risks.md) |
| 24 | References | [design/24_design_references.md](design/24_design_references.md) |
| 25 | Export schema (`export-decisions`) | [design/25_design_export_decisions_schema.md](design/25_design_export_decisions_schema.md) |
| 26 | Merge queue and execution topology protocol | [design/26_design_merge_queue_protocol.md](design/26_design_merge_queue_protocol.md) |

Read §4 in full before touching the engine, the event log, or anything that
handles capacity or questions. §21 is the build order, and it is deliberate.

## Line citations

Comments in `src/` cite the pre-split document by line, as `DESIGN.md:<line>`.
Those lines refer to the single-file `DESIGN.md` as it stood at master
`cfec136` on 2026-09-03, the last commit before the split; the file is
unchanged in that history. This table resolves a line to its section, which is
where the sentence now lives, unchanged. New citations use `§N`.

| Lines | § | Lines | § | Lines | § |
|---|---|---|---|---|---|
| 1–9 | preamble (this file) | 268–287 | 9 | 550–567 | 18 |
| 10–17 | 1 | 288–303 | 10 | 568–584 | 19 |
| 18–33 | 2 | 304–315 | 11 | 585–591 | 20 |
| 34–51 | 3 | 316–325 | 12 | 592–629 | 21 |
| 52–61 | 4 | 326–353 | 13 | 630–637 | 22 |
| 62–76 | 5 | 354–372 | 14 | 638–688 | 23 |
| 77–123 | 6 | 373–441 | 15 | 689–693 | 24 |
| 124–201 | 7 | 442–457 | 16 | | |
| 202–267 | 8 | 458–549 | 17 | | |

## Retired records

Until 2026-09-03 the repository also carried `decisions/` (dated decision
records), `proposals/` (design proposals and their council critiques) and
`acceptance/` (the v0.1 acceptance run book and write-ups). They were retired
so that the design is the one place a rule lives; every conclusion they reached
that still binds is in the sections above. The full texts remain in the
repository history before that date and in the private companion repository.
Comments in `src/`, `effects/allowlist.toml` and `upstroke.toml` still cite
some records by path; this table says where each one's substance now lives.

| Record | Substance now in |
|---|---|
| 2026-08-11 design council | §21 (learned routing parked; the council is manual, ≤3 family seats) |
| 2026-08-11 self-hosting v0.2 | §21 (v0.2 development runs through upstroke) |
| 2026-08-11 gate config across a resume | §15 (gates are taken from the record, warning on drift) |
| 2026-08-11 Codex reasoning effort | §10, §16, §21 (effort is a routing axis, stated on every attempt) |
| 2026-08-11 export schema | §25 |
| 2026-08-12 merge queue and execution topology | §26 (the verdict and the durable protocol, verbatim), summarised in §7, §14, §15 |
| 2026-08-17 review effort and fan-out | `MAINTAINING.md` (one frontier pass at `max`; `ultra` is delegation, not depth) |
| 2026-08-20 automated review gate; 2026-08-20 review invalidation scope; 2026-08-21 stacked slice PRs; 2026-08-23 retire App attestation; 2026-08-25 checkpoint merges; 2026-08-31 panel seats; 2026-09-01 review effort re-scoped; 2026-09-01 clean base merge keeps review | `MAINTAINING.md` (the whole review and merge lifecycle, restated in its current form; the workflow trigger contract under Repository rules) |
| 2026-08-22 strategy record private; 2026-08-27 proposals private; 2026-09-01 proposals relocated; 2026-09-01 infra private | This section (strategy, proposals and operator tooling live in the private companion repository) |
| 2026-08-24 PR3-layer freeze charter; 2026-08-31 G2 checkpoint promotion; 2026-08-31 inertness premise behavioural | §21 (the G2 pass ran; the schema-4 machinery reached master inert by default, and PR12 activates it) |
| 2026-08-25 `CommandSpec.program` stays `String` | §8 (the field carries a bare CLI name; `CODING_STANDARDS.md` §8 governs it the moment a path-valued input exists) |
| 2026-08-26 durable retry feedback | §15 (`FailureRecord.detail` carries the retry brief onto the durable record) |
| 2026-08-30 readiness lint placement | `CODING_STANDARDS.md` §2 (lints) and §12 (a per-site `#[expect]` recorded in `effects/allowlist.toml` may stand where a module-level allow did) |
| 2026-08-30 test scratch-tree ownership | §15 (run-directory deletion authority is token-carried: `rundir::PrivateHalfProof` and the `cfg(test)`-only scratch-tree token) |
| 2026-09-01 self-hosted Windows test leg | `MAINTAINING.md` step 3 and `ci.yml` (`test-windows-hosted` runs the Windows suite on `windows-latest` in ten shards, `test (windows-latest, k of 10)`, for a pull request, a push and a merge-queue entry alike, each shard installing and asserting the pinned compiler, `1.97.1`, running the suite step with its own test-name filter and holding a share of the Windows suite's passing-test floor of 1700, the shares summing to it and every test running in exactly one shard, and `upstroke-ci` requires the job, which passes only when all ten shards do; from 2026-09-01 the suite ran on an ephemeral self-hosted guest unless the run was routed to `windows-latest`, from 2026-11-12 by a picker job, and the self-hosted leg was retired on 2026-11-20 and the guests deleted; measured 2026-11-15, run 43719719561, the whole suite takes 59 to 76 minutes on `windows-latest` and three of its shards took 14m42s, 17m17s and 22m53s (the fourth shard of that run never ended, and was divided in two, which made five), against the guest's 13 for the whole; measured 2026-11-19, run 44614925593, the five shards' test harnesses took 1201, 508, 976, 564 and 860 seconds and the slowest job took 24.3 minutes, about 3.6 minutes of every leg being fixed; the suite was divided into ten shards by that measured time that day, their slowest leg predicted at about 13 to 16 minutes; run 44667503206 measured the ten for the first time, and with them the Linux and macOS legs of the `test` matrix, divided by measured time as well: the slowest Windows leg took 18.6 minutes against the 13 to 16 predicted, the slowest of the five macOS legs 15.8 against 14 to 15, and the slower of the two Linux legs 8.2 against 8 to 9, and the Linux split stands; the Windows and macOS suites were divided again on 2026-11-20, from that run, each test's sequential Linux duration scaled so that the leg it ran in sums to that leg's measured harness, blended half and half with a uniform per-test model, and the filters chosen by a search for the slowest leg; the ten shards are now `engine::topology::coordinator::tests::a_p`, `engine::topology::coordinator::tests::a_r`, the rest of `engine::topology::coordinator::tests::a_`, the rest of `engine::topology::coordinator::`, `engine::topology::recover::`, `engine::tests::a`, `runner::`, `workspace_manager::`, the rest of `engine::tests::`, and everything else, and the five macOS legs `engine::topology::coordinator::tests::a_`, the rest of `engine::topology::`, the rest of `engine::`, `runner::`, and everything else; the slowest shard was predicted at about 14.4 minutes and the slowest macOS leg at about 11.8; run 44736631711 measured both re-splits for the first time, the slowest Windows leg taking 18.1 minutes and the slowest macOS leg 16.0; single tests set the floor under the Windows prediction, a leg taking no less than its longest test and the fixed minutes, and two tests are each estimated at about 9 minutes of their leg's harness, neither changed by the re-split, `engine::topology::recover::tests::a_kill_at_every_cell_of_a_complete_finalization_converges_on_the_next_resume` and `workspace_manager::tests::r_o2_a_cross_kind_scrub_stopped_at_any_phase_converges_with_no_instance_registered`; each shard runs under a 40-minute limit, and in the 2026-11-15 run one whole-suite job failed a kill-timing test and one shard did not end after its tests had passed, neither following a change; the Windows Clippy, build-witness and MSRV legs stay on GitHub's runner) |
| 2026-09-01 relicense to Apache-2.0 | `LICENSE`, `NOTICE`, `CONTRIBUTING.md` (the CLA), `MAINTAINING.md` (the archive-contents release gate) |
| The thirteen proposals | Ten were already stubs; the G2 pass plan is discharged (§21), and the v0.5 portfolio proposal and its critique are v0.5 material, out of the engine's contract |
| The acceptance run book and write-ups | §21 (the v0.1 definition of done and the date it was met) |
