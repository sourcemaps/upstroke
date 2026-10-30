---
id: G6-ST07-PARTIAL-OBSERVATION-EXPORT
severity: P2
disposition: deferred     # open: implemented by 541b9d92 below it, not yet built or run; the guarded slot's runs and this pull request's fresh reviews decide its close
category: correctness
pr:                       # assigned when the pull request is created
reviewed_sha: adf4f91d452f83b4be7f4100b5ff632ad39b8d4c
location: src/observations.rs:177
provenance: undetermined
first_bad:                # undetermined for the observed record; the in-place write dates from 8dd82409, the same-named pipeline threads from 1e0cd0fe
guard: a_cut_short_export_leaves_the_earlier_record_whole and overlapping_exports_of_one_test_take_turns_and_keep_every_observation
---

## Failure sequence

L-ST07-1, the ST-07 row of the r2 remainder at `adf4f91d`, ran
`st07_the_sequential_range_is_a_bijection_over_the_exported_observations` over L-COPY-1's copy of
L-S2's export. It exited 101 at `src/engine/topology/coverage/tests.rs:559`, where the export failed
to load: `closure_kill_child_at_width_three.2411595.json` "is not an observation record: EOF while
parsing a value at line 1 column 0".

That record is 0 bytes in L-S2's export and in the copy, and in the export its mtime equals its
ctime. Both full suites before it passed. The first campaign's two exports at `adf4f91d` each hold
one empty `closure_kill_child_at_width_three` record as well. L-S3's export and Gate 6's two
exports at `e0932045` hold none. The exporter, the fixture, the loader and the event funnel are
unchanged between `e0932045` and `adf4f91d`.

At `adf4f91d` the export (`src/observations.rs:152-178`) reads the record, merges, and writes the
result back in place with `fixture::write_file` (`src/workspace_manager/fixture.rs:938-943`). Its
`fs::write` creates the file, emptying it, before it writes the bytes. Two source facts reach that
window:
- **The process can end inside it.** `Exported::carried` exports before handing a `Kill` back, and
  the event funnel then calls `std::process::abort()` on that thread (`src/events/log.rs:157`).
- **One record has several writers.** `run_concurrently` names its runtime's threads after the
  calling test (`src/engine/topology/coordinator.rs:222-232`). Each pipeline's
  `HarnessTopologyHooks` (`src/engine/topology/seams.rs:99-109`) carries five exporting adapters and
  drops on its blocking thread. The record is named after the thread and the process.
  `docs/internals/observations.md` said such exports were one thread's and could not interleave.

So an export cut short between creation and bytes leaves the record empty. The strict loader
(`src/engine/topology/coverage.rs:18-43`) then refuses the whole export. An export that reads the
record while another rewrites it can merge nothing and publish without the earlier observations.

It is not established which writer, thread and cut emptied the observed record. No process was
traced and nothing was reproduced at `adf4f91d`.

## What the change that takes this up should do

- Publish each record whole: write the merged record beside it and rename it over. A reader then
  meets a whole record, and a cut leaves the earlier one.
- Let the exports of one record take turns from read to rename, so overlapping exports keep every
  observation.
- Keep the strict loader and the bijection. The loader passes a temporary over by its extension.

This is implemented on this branch by `541b9d92`: `fixture::update_file`, the write seam
`fixture::at_next_write`, `observations::export_into`, two regression tests and the notes. None of
it has been built or run. The targeted tests, the full baseline, both export suites and both ST-07
rows wait for the next guarded slot.
