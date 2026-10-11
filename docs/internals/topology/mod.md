# `src/topology/mod.rs`

Extended notes for [`src/topology/mod.rs`](../../../src/topology/mod.rs).

## Module

The v0.2 execution topology uses event schema 4. The compatibility boundary is defined in
[DESIGN.md §15](../../../design/15_design_event_log_resume_run_layout.md); the build order and
remaining v0.2 work are in [§21](../../../design/21_design_versioned_scope.md).

This module declares nine children:

| Module | Responsibility |
|---|---|
| [census](../../../src/topology/census.rs) | Bounded exploration of checked fold transitions |
| [effects](../../../src/topology/effects.rs) | Typed effect sites, hooks and fault-injection records |
| [events](events.md) | Schema-4 event types and wire contracts |
| [fold](../../../src/topology/fold.rs) | Checked transitions shared by live execution and replay |
| [leases](../../../src/topology/leases.rs) | Generation, candidate and repair-lineage holdings |
| [paths](../../../src/topology/paths.rs) | Predicted and actual repository regions |
| [queue](../../../src/topology/queue.rs) | Candidate order and integration eligibility |
| [registry](../../../src/topology/registry.rs) | Task storage identities and frozen entries |
| [schema](schema.md) | Schema selection, header probing and reader compatibility |

Schema-4 writing machinery exists. The crate-private
[engine topology driver](../../../src/engine/topology.rs) creates and drives topology runs;
its [emit path](../../../src/engine/topology/emit.rs) writes through
[EventLog's topology append methods](../events/log.md). This build does not expose that driver
through the production CLI. The [engine facade](../../../src/engine/mod.rs) keeps it
`pub(crate)`, and [schema selection](../../../src/topology/schema.rs) keeps
`TOPOLOGY_ACTIVATION` at `Inactive`: fresh production runs write schema 3 and production readers
accept schemas 1 through 3. `WriterSelector::TopologyPreview` selects schema 4 for the topology
machinery exercised by tests. Existing runs continue through the sequential engine; there is no
in-flight upgrade from schema 3 to schema 4.

## `#![forbid(`

The three governed lints are `forbid` here since #318's third round: this file
stated no level for any of them and inherited none, so each took its level
from `-D warnings` alone, which an inner `allow` the placement scan does not
read -- macro-written, or spelled apart -- lowers; #318's second MAIN review
executed exactly that in `src/plan/mod.rs` and reached `std::fs::write` from
a production topology body while clippy and every governance test passed
(`GUARD-DECISION-SILENT-PRODUCTION-FILES-OUTSIDE-THE-ROLL-CALL`). A lint level is scoped by the module tree, so this fence reaches the 31 files under `src/topology/` -- `census.rs`, `effects.rs` and its eight children, `events.rs`, `fold.rs` and its thirteen, `leases.rs`, `paths.rs`, `queue.rs`, `registry.rs`, `schema.rs` -- every one of which stated nothing of its own; measured at the third round (`~/orch-pr10/repair-318-r3-evidence/plan-class/M1-*`; with the fence removed the guard names 81 pairs, `controls/C1-unfence-topology-root-*`; a generated allow in `paths.rs` is `E0453`, `C2-inheritance-reach-*`).
`forbid`, not `deny`, because it compiles: no topology module may carry an allowance (`the_legacy_section_never_contains_a_topology_module`) and the one funnel row under it, `src/topology/effects.rs`, records `allows = []`; the five whole-file test modules under it allow nothing either, so the `forbid` reaches them without `E0453`, and clippy over all targets exits 0. A downgrade beneath
a `forbid` is `E0453` however it is written or generated, and the enforcement
is the lint gate's -- rustc resolves no `clippy::` lint, so `cargo build` and
`cargo test` compile what clippy refuses. `effects::tests::every_unclassified_production_file_states_each_governed_lint_or_inherits_its_forbid`
names this file the day the fence is removed, and
`every_fence_of_a_governed_lint_forbids_wherever_forbid_would_compile` the
day it drops to `deny`.

