---
id: PR10-ST18-THIRD-RESUME-LEFTOVER-ASSERTION
severity: P1
disposition: deferred
category: crash-consistency
pr: 279
reviewed_sha: 8e215030a0a0278733d3e332d0d2775771f17221
location: src/engine/topology/recover/tests.rs:15637
provenance: introduced_by_feature
first_bad: 9355c36d
guard: deferred by owner decision 2026-09-14 (#279 merges at the reviewed head; fixed after the merge; Gate 5 judges whether it bears on a pass-rule clause): `fix-P1/crash-consistency_the-st18-third-resume-asserts-on-the-first-leftover` keeps the path the third resume's `plant_report_leftover` returns in `kill_after_report_before_each_cleanup_step` and asserts that file, its parent directory and `report_staging_record` gone after the third resume, executing the recipe `third-resume-leftover-record-only` before (the test passes at the reviewed head) and after (it fails); until then the fresh branch's removal of a recorded directory (`sync_report_dir`, the reclaim at `src/rundir.rs:1338`) is observed by no test, and Gate 5's residue clause ("residue-class evidence complete and labeled") and ST-18's convergence clause judge whether the record's sentence at `reviews/2026-09-12-pr10-record.md:2250-2251` rests on it
---

## Failure sequence

Found by the round-11 crash lens of PR #279 (finding 1, P1; `~/orch-pr10/reviews/r11/crash.md`;
the combined comment at https://github.com/sourcemaps/upstroke/pull/279#issuecomment-5666525585).
The lens reasoned from the opened test body and captured no cargo exit; the session that filed
this executed nothing either. Every line number below is the tree at `8e215030`.

The ST-18 matrix `kill_after_report_before_each_cleanup_step`
(`src/engine/topology/recover/tests.rs:15548`) drives, for each of its 26 Complete and 24 Halted
cells, a faulted resume, a second resume that finalizes, and a third resume that finds the run
finalized and takes the fresh branch of terminal finalization (`rundir::sync_report_dir`, called
from `src/engine/topology/finalize.rs:108` and from nowhere else). Before the third resume it
plants a second dead writer's staged report:

```rust
// src/engine/topology/recover/tests.rs:15635-15639
let report_bytes = std::fs::read(fixture.public().join("report.json"))
    .expect("the report the second resume left current");
plant_report_leftover(fixture);
let third = harness();
let (result, _) = resume(fixture, &third, &given);
```

`plant_report_leftover` (`:2007`) returns the planted file's path and line 15637 discards it. What
the third resume is then held to (`:15661-15668`) is:

```rust
assert!(
    !planted.report_leftover.exists()
        && rundir::report_staging_leftovers(&fixture.public(), &fixture.private())
            .expect("listed")
            .is_empty(),
    "{tag}: nothing was staged, and the staged report a dead writer left before this \
     resume is reclaimed on the fresh branch too"
);
```

`planted.report_leftover` is the **first** leftover, planted by `plant_finished_run_with`
(`:1989`) and already gone: `assert_finalized` (`:15263`) asserted it absent at `:15322` after the
second resume, before the third ran. The first conjunct is therefore true whatever the third resume
does. The second conjunct is the oracle `rundir::report_staging_leftovers` (`src/rundir.rs:1426`),
which lists the directory **through** the record: when `recorded_report_staging` finds no record it
returns at `:1435-1437` with nothing but a staged record, if one stands. Removing the record alone
empties its answer. Nothing in the third-resume block reads the second leftover's path, its parent
directory or the private record.

Sequence: a cell finalized by the second resume -> the third resume's block plants a second
recorded staging directory under the public run directory and forgets its path -> a fresh-branch
reclamation that removes only the private record and leaves the directory and the half-written
`report.json` inside it -> the test still sees the first path absent, an empty record-derived
inventory, byte-identical report bytes and the expected hooks, and passes -> the directory and its
partial report stand in the run directory permanently, unrecorded, which `unrecorded_report_staging`
(`src/rundir.rs:1479`) then names as not the protocol's and never removes.

**Mutation the current assertions admit** (the crash lens's, verbatim; recipe
`third-resume-leftover-record-only`; unexecuted). At `src/rundir.rs:1338` in `sync_report_dir`,
replace

```rust
let passed_over = reclaim_report_staging(public, private)?;
```

with

```rust
remove_file_if_present(&report_staging_record(private))?;
let passed_over = unrecorded_report_staging(public, private)?;
```

and run

```bash
env UPSTROKE_RAMTARGET=/mnt/ramtarget/iso-<agent> /home/ubuntu/bin/upstroke-build cargo test --all-targets --all-features engine::topology::recover::tests::kill_after_report_before_each_cleanup_step -- --exact
```

Expected: the test passes under the mutation (the lens's reading; not executed). The saved recipe
`report-leftover-not-reclaimed-on-fresh`
(`~/pr10-evidence/09d6887a0956ee4b032f75232f7b2a397f927c82/mutations/`, captured `101`, killed by
this test at `:15661`) replaces the reclaim with `let passed_over = Vec::new();` — it removes
neither the directory nor the record, so the record stands, the oracle lists it, and the second
conjunct fails. That captured `101` does not cover this survivor: what kills the saved recipe is
the record standing, and this recipe removes the record.

**Reachability.** The subject is a test, so what is reachable at the reviewed head is the gap it
leaves. On the fresh branch, `sync_report_dir`'s reclaim at `src/rundir.rs:1338` is reached only
through terminal finalization (`finalize.rs:108`), and the third resume of this matrix is the only
place a recorded leftover stands when it runs (`assert_finalized` holds the second resume to the
first leftover, but that one is reclaimed on the write branch, by the first or the second resume,
in every cell); the directory's removal there is observed by no test — only the record's. The
state the missing assertion guards is a real one: a writer killed between its stage and its rename
leaves the record and the directory (the shape
`rundir::tests::a_report_whose_staged_file_will_not_sync_is_not_published`, `src/rundir/tests.rs:3188`,
drives by a refused barrier), and a resume of a finalized run is the ordinary next step.

**Consequence for the record.** `reviews/2026-09-12-pr10-record.md:2250-2251` justifies leaving
the recorded leftover without a registry class in part by: "the ST-18 matrix's cells around the
report's two sites fault the reclaim on the write branch and its third resume the fresh branch's,
and the three reclaim recipes are killed there (§8)"; the row at `:2914` repeats "faulted by the
matrix's cells around the site and its third resume". The third resume establishes nothing about
the fresh branch's removal of the directory as written, so that justification — one Gate 5's
residue clause would rest on — is not established by the test. (The cancelled round's brief
paraphrased the sentence as "the third resume proves its reclamation"; the verbatim sentence is the
one quoted.) The classification gap itself is
`PR10-RECORDED-STAGING-LEFTOVER-UNCLASSIFIED`.

## Why this is deferred

Owner decision 2026-09-14: #279 merges at the reviewed head; this is fixed after the merge, and
Gate 5 judges whether it bears on a pass-rule clause.

## What the change that takes this up should do

Branch `fix-P1/crash-consistency_the-st18-third-resume-asserts-on-the-first-leftover`. In the
third-resume block keep the path `plant_report_leftover` returns and, after the resume, assert
that file absent, its parent directory absent and `rundir::report_staging_record(&fixture.private())`
absent, beside the existing `report_staging_leftovers` conjunct. Execute the recipe above before the
change (the test passes) and after (it fails at the new assertion), and re-run the saved
`report-leftover-not-reclaimed-on-fresh` and `report-leftover-not-reclaimed-on-write` recipes at
the new head; capture every `cargo exit`. Then make the record's sentence at `:2250-2251` and the
row at `:2914` say what the test observes. The fixture is path-shaped: the Windows guest runs the
test by name.
