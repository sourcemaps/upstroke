# `src/observations.rs`

Extended notes for [`src/observations.rs`](../../src/observations.rs).
[Source on GitHub](https://github.com/sourcemaps/upstroke/blob/master/src/observations.rs).
The relative link works in a checkout or on GitHub; the GitHub link also works from the published site.

The code is the authority for what it does. The explanatory prose is preserved below.
Each backticked part of a section heading is an exact source excerpt. Search for the final
excerpt within the preceding item when a heading names both an item and a line inside it.

## Module

The ST-07 observation export: what a test's hook harness saw, written to a file named after the
test and the process that wrote it when `UPSTROKE_HOOK_OBSERVATIONS` names a directory. Every harness adapter — the Worktree,
Snapshot, Ref and Object families' `HarnessEffects`, the run-directory `HarnessHooks`, the Event
family's `HarnessEventHooks`, the container and process families' `HarnessHooks` — holds its
harness through [`Exported`], so the record is written when the last clone of an adapter on a
harness drops and again just before a `Kill` injection is handed back, since the process that
carries it out writes nothing afterwards. Outside tests the export is a no-op and the handle is
inert.

`engine::topology::coverage` reads the records back, rebuilds one harness from the ones that
are funnel executions and holds the sequential registry to it. Since PR10's round 7 the
sequential residue sampler writes its histogram, `residue-histogram-sequential.json`, into the
same directory when an export is requested (and under the build's profile directory when none
is), so the merge check reads the run's own histogram beside the run's own records.

## `#![forbid(`

The three governed lints are `forbid` here since #318's third round: this file
stated no level for any of them and inherited none, so each took its level
from `-D warnings` alone, which an inner `allow` the placement scan does not
read -- macro-written, or spelled apart -- lowers; #318's second MAIN review
executed exactly that in `src/plan/mod.rs` and reached `std::fs::write` from
a production topology body while clippy and every governance test passed
(`GUARD-DECISION-SILENT-PRODUCTION-FILES-OUTSIDE-THE-ROLL-CALL`). A leaf with no children.
`forbid`, not `deny`, because it compiles: nothing in the file allows a governed lint, and clippy over all targets exits 0 with the fence in place. A downgrade beneath
a `forbid` is `E0453` however it is written or generated, and the enforcement
is the lint gate's -- rustc resolves no `clippy::` lint, so `cargo build` and
`cargo test` compile what clippy refuses. `effects::tests::every_unclassified_production_file_states_each_governed_lint_or_inherits_its_forbid`
names this file the day the fence is removed, and
`every_fence_of_a_governed_lint_forbids_wherever_forbid_would_compile` the
day it drops to `deny`.

## `pub struct ObservationRecord {`

One test's observations: what its harness saw executed (`observed`, which for a point means an
injection fired there), what it reached without injecting, and the fast sequences it recorded.
Records of one test merge by the larger count per coordinate, so a test that builds several
adapters, or a kill child spawned more than once, exports one record: the exporter merges within
its process's file, and `engine::topology::coverage::load_observations` merges every file whose
record names the same test. An empty record is not written.

## `pub struct Exported {`

The harness and its export in one handle, so an adapter can derive `Default` — the derived
`default` builds a fresh harness with its own export — and every clone shares one export. The
export is written by the drop of the last clone, through the `ExportOnDrop` the handle holds
for its drop alone, and before a `Kill` is handed back, since a process that dies at a hook never
reaches its drop. Outside `cfg(test)` the export is a no-op.

## `pub struct Exported` › `_on_drop: Arc<ExportOnDrop>,`

Held for its drop: the last clone's release writes the export.

## `impl Exported` › `pub fn hook(&self, site: EffectSiteId, phase: HookPhase) ->…`

One `hook` call under the harness's lock (a poisoned lock is
entered), the answer carried through [`Self::carried`].

## `impl Exported` › `pub fn hook(&self, site: EffectSiteId, phase: HookPhase) -> Injection {`

One `hook` call under the harness's lock, the lock released before the answer is carried: the
export takes the lock itself, and `std::sync::Mutex` is not reentrant.

## `impl Exported` › `pub fn carried(&self, injection: Injection) -> Injection {`

Every adapter's answer passes through here: a `Kill` is exported before it is returned, because
the funnel aborts the process right after and a kill-mode observation that reached only the
in-memory harness would be lost with it — which is exactly the observation the merge check needs
for a kill-mode point. Call it with the harness's lock released: the export takes the lock itself.

## `mod export {`

Compiled two ways, cut at a module so the effects census's production region ends where every
other module's does. Under `cfg(test)` the export reads the variable, names the record after the
current thread — which `cargo test` names after the test — and its file after the test and this
process, and hands the file to the fixture's `update_file`, the one publication a module outside
the funnels may make in a test build: in this process's turn for the file it merges with the
record an earlier export of the same test in this process published, writes the result beside the
record and renames it over. Outside tests it does nothing.

The process is in the file name because a file several processes share loses records. Until
#292's first repair round every export of a test read `<test>.json`, merged and wrote it back
unlocked, and two kill children of one test, spawned by two parents running at once, could
interleave: a child that read the file before the other's writes and wrote after them replaced
them. #292's round-1 regression lens found it, and bounded barriers placed around that read and
write reproduced it: `candidate_sequence_kill_child`'s record lost `Ref.CreateCandidates/after`
and the merge check refused. The loader merges the processes' records by the larger count per
coordinate, the union a serial run's single file held.

Within one process, too, the exports named after a test are not one thread's. `run_concurrently`
names its runtime's threads after the thread that called it (`1e0cd0fe`), so that a pipeline's
records are its test's (`engine/topology/coordinator.md`). Each pipeline's `HarnessTopologyHooks`,
made for it and dropped on its thread, carries five exporting adapters, and the coordinator's
thread exports before a `Kill` it then dies of. Until `G6-ST07-PARTIAL-OBSERVATION-EXPORT` these
notes said the exports of one process were one thread's, one after another, and could not
interleave, and the export rewrote the record in place through `write_file`, which creates the
file, emptying it, before it writes the bytes. An export cut short in that window, by a `Kill`
on another thread or by a panic, left the record empty or partial, and an export that read the
record while another rewrote it could find it so and publish without the observations it should
have merged. L-ST07-1, the ST-07 row of the r2 remainder at `adf4f91d`, found the record
`closure_kill_child_at_width_three` exported from process 2411595 empty in a full run's export,
and the loader, which refuses what it cannot parse, refused the export: "EOF while parsing a value
at line 1 column 0". Which writer, thread and cut left it empty was not established.

`update_file` closes both windows. It publishes as `CODING_STANDARDS.md` §12's readiness rule
asks, staged elsewhere and renamed into place, never created in place and written afterwards; and
the exports of one record take turns from read to rename, so none merges over a record another is
replacing. Nothing is written into the record itself, so a cut leaves the record as the last whole
export published it, and its part under `<record>.<pid>.tmp`, which the loader passes over by its
extension.
`a_cut_short_export_leaves_the_earlier_record_whole` and
`overlapping_exports_of_one_test_take_turns_and_keep_every_observation`, in
`engine::topology::coverage::tests`, hold the two.

## `mod export` › `pub(super) fn export(harness: &Arc<Mutex<HookHarness>>) {`

Merge what `harness` observed into the record named after the current
thread (the test) under the directory `UPSTROKE_HOOK_OBSERVATIONS`
names; nothing when the variable is unset or nothing was observed.
`export_into` does the work.

## `mod export` › `pub(crate) fn export_into(dir: &Path, harness: &Arc<Mutex<HookHarness>>) {`

The export into `dir` rather than the directory the variable names, so a test can export into a
scratch directory of its own: the variable is the process's, and setting it would send every
running test's export there too. Re-exported to the crate under `cfg(test)` for those tests.

