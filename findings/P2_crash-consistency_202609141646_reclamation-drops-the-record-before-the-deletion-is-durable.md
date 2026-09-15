---
id: PR10-RECLAIM-RECORD-DROPPED-BEFORE-DURABLE-DELETION
severity: P2
disposition: deferred
category: crash-consistency
pr: 279
reviewed_sha: 8e215030a0a0278733d3e332d0d2775771f17221
location: src/rundir.rs:1538
provenance: introduced_by_feature
first_bad: 9355c36d
guard: deferred by owner decision 2026-09-14 (#279 merges at the reviewed head; fixed after the merge; Gate 5 judges whether it bears on a pass-rule clause): `fix-P2/crash-consistency_reclamation-drops-the-record-before-the-deletion-is-durable` keeps the old record in `reclaim_report_staging` until the public directory's barrier has made the recorded directory's deletion durable, on the write branch and the current-report branch (`sync_report_dir`) and in `publish_report`, with the lens's recipe `a_reclaimed_report_stage_remains_reclaimable_after_power_loss` executed before (fails at the reviewed head) and after (passes), rule 1 for the new ordering and rule 2 for the barrier; until then Gate 5's residue clause ("every observed residue classified and recovered") is judged against a leftover recovery passes over
---

## Failure sequence

Found by the round-11 crash lens of PR #279 (finding 3, P2; `~/orch-pr10/reviews/r11/crash.md`),
a reasoned crash-replay hypothesis whose recipe was not executed by the lens or by the session
that filed this. The orchestrator's decision (the combined comment at
https://github.com/sourcemaps/upstroke/pull/279#issuecomment-5666525585) placed it "on the slice's
subject", to be fixed as round 8's B4 was; the owner's decision of 2026-09-14 files it instead.
Every line number below is the tree at `8e215030`.

`reclaim_report_staging` (`src/rundir.rs:1520-1541`) removes the directory the private record
names and then the record:

```rust
// src/rundir.rs:1521-1539
if let Some(recorded) = recorded_report_staging(public, private)? {
    match fs::symlink_metadata(&recorded) {
        Ok(metadata) if metadata.file_type().is_dir() => {
            fs::remove_dir_all(&recorded).map_err(|source| UpstrokeError::Io {
                path: recorded.clone(),
                source,
            })?;
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(source) => { /* propagated */ }
    }
    remove_file_if_present(&report_staging_record(private))?;
}
```

No barrier on `public` stands between the `remove_dir_all` and the record's removal at `:1538`.
On the write branch (`write_report`, `:1263`) the next statement is `begin_report_staging`
(`:1264`, defined at `:1561`), which publishes a **new** record naming the new directory B durably
— staged, synced, renamed onto `report-staging.json`, the private directory synced (`:1571-1577`)
— and the only barrier on `public` is the one at the end of `publish_report` (`:855`; `sync_dir`
at `:879-881`), after the staged file's own sync and the rename. `publish_report` itself removes
the record (`:875`) before that barrier, and its comment (`:852-854`) considers the one restored
state — "a record the disk restores names a directory that is gone, which the next write removes
on its own" — and not the inverse. On the current-report branch (`sync_report_dir`, `:1338-1339`)
the reclaim precedes the barrier, so the record is gone before the deletion is durable there too.

Sequence: a dead writer left the durable staging directory A under `public` and the private record
naming A (the record synced by `begin_report_staging`, A's staged file by the writer's file barrier)
-> the next `write_report` removes A and then record(A) (`:1524-1538`) -> `begin_report_staging`
durably publishes record(B) (`:1577`) and creates B (`:1588`) -> the process dies before
`publish_report`'s barrier on `public` (`:879-881`): a kill inside the staged write, or the staged
file's barrier refused, the shape `a_report_whose_staged_file_will_not_sync_is_not_published`
(`src/rundir/tests.rs:3188`) drives -> power is lost; the public filesystem, whose deletion of A was
never synced, restores A, while the private filesystem holds the synced record(B) -> the next
write's reclaim reads record(B), removes B (or a record naming a directory that is gone) and
`unrecorded_report_staging` (`:1479`) lists A as staging-shaped and unrecorded -> A is passed over as
not the protocol's, never removed and never adopted: a directory of the run's own making, with its
half-written `report.json`, stands in the run directory permanently.

Public and private need not share a filesystem: the private half lives under the private root and
the public run directory under the public one (`design/15_design_event_log_resume_run_layout.md`),
so syncing the private record establishes nothing about A's deletion.

**Reachability.** Every step is a production path. A dead writer's leftover is what the protocol
exists to reclaim; the record-then-create sequence runs on every report write; the window between
record(B)'s publication and the public directory's barrier holds the staged file's write and sync,
so a death there is a kill inside `Report.Write`'s primitive — the coordinate the registry carries
no row for (`PR10-RECORDED-STAGING-LEFTOVER-UNCLASSIFIED`) and the ST-18 matrix does not fault
(the crash lens's matrix row "Inside the new private staging-record protocol": empty). The
unsynced-deletion-restored-by-power-loss step is the crash model this tree's other barriers exist
for (`publish`, `:824`; the checkout's parent barrier in `workspace_manager`).

**Recipe** (the lens's, verbatim as a specification; unexecuted). Add
`a_reclaimed_report_stage_remains_reclaimable_after_power_loss` beside
`a_dead_writers_staged_report_is_reclaimed_by_the_next_write` in `src/rundir/tests.rs` (`:4858`).
Plant A through the writer's helper (`plant_report_staging_of_a_dead_writer`, `src/rundir.rs:2693`)
and sync its file, directory and public parent. Fault the next report's file barrier using
`fail_file_barriers_within(&public)` (`src/util.rs:378`), allowing B's private record publication
to complete. Simulate loss of unsynced public changes: restore A's saved directory and file, discard
B's unsynced directory, and retain B's private record. Retry `write_report`. Assert A absent and the
passed-over list empty; current code should fail both expectations.

## Why this is deferred

Owner decision 2026-09-14: #279 merges at the reviewed head; this is fixed after the merge, and
Gate 5 judges whether it bears on a pass-rule clause.

## What the change that takes this up should do

Branch `fix-P2/crash-consistency_reclamation-drops-the-record-before-the-deletion-is-durable`.
Preserve the old ownership record until the public deletion is durable: after `remove_dir_all` of
the recorded directory, sync `public` through its typed site (`sync_dir`, so the ledger's
`SyncedDirectory` entry carries it) and only then remove the record — on the absent branch too,
since an absent name is not proof the deletion reached its barrier — and the same order in
`publish_report` (`:871-881`), whose record removal precedes its barrier, so a refused barrier
leaves the record standing for the next write. Rule 1 for the new ordering: the barrier's
observation records the recorded name absent at the sync's instant (the ledger's entry observed,
as `begin_report_staging`'s `DirectoryCreated` entry does at `:1589-1596`), and the
"remove the record before the sync" mutation is executed and killed. Rule 2 for the barrier: its
swallow mutation executed and killed by a test arming `fail_barriers_at(&public)`. Execute the
recipe above before (fails at `8e215030`) and after (passes); any new typed site or cell carries
its ST-18 cell and mutation. This is the design the cancelled round-12 session reached and did not
apply (`~/orch-pr10/handovers/repair_279_r11b.md`). The report writer is path-shaped: the Windows
guest runs its tests by name.
