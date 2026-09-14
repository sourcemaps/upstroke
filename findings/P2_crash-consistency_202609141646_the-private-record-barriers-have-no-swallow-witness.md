---
id: PR10-PRIVATE-RECORD-BARRIERS-UNWITNESSED
severity: P2
disposition: deferred
category: crash-consistency
pr: 279
reviewed_sha: 8e215030a0a0278733d3e332d0d2775771f17221
location: src/rundir.rs:1577
provenance: introduced_by_feature
first_bad: 9355c36d
guard: deferred by owner decision 2026-09-14 (#279 merges at the reviewed head; fixed after the merge; Gate 5 judges whether it bears on a pass-rule clause): `fix-P2/crash-consistency_the-private-record-barriers-have-no-swallow-witness` adds `a_report_staging_record_failure_stops_before_directory_creation` beside `a_report_staging_directory_is_recorded_before_it_is_made`, arming `fail_file_barriers_within(&private)` and, separately, `fail_barriers_at(&private)` across two attempts and requiring the barrier diagnostic, no `DirectoryCreated` entry, no staging directory and no report, then publication and reclamation once disarmed, with the swallow recipes `staging-record-publish-swallowed` and `staging-record-stage-swallowed` executed before (the ordering test passes) and after (the new test fails); until then the two private-record barriers of `begin_report_staging` have no error-propagation witness (rule 2 of the repair rounds)
---

## Failure sequence

Found by all four round-11 lenses of PR #279 (regression, finding 1; fix-check, B2 "Rule 2
remains unmet"; crash, finding 2; record, finding 1; `~/orch-pr10/reviews/r11/`), each reasoning
from the opened test body; the survivor recipes are unexecuted hypotheses, by the lenses and by the
session that filed this. Every line number below is the tree at `8e215030`.

`begin_report_staging` (`src/rundir.rs:1561-1602`) publishes the staging directory's record in the
private half before making the directory:

```rust
// src/rundir.rs:1569-1577
let staged_record = private.join(REPORT_STAGING_RECORD_STAGED);
remove_file_if_present(&staged_record)?;
stage_json(
    &staged_record,
    &ReportStagingRecord { directory: name },
    ledger,
    None,
)?;
publish(&staged_record, &record, ledger)?;
```

`stage_json` (`:566`) syncs the staged record's file through `sync_file_recorded` (`:807`) and
`util::fsync_file_at`; `publish` (`:824`) renames it and syncs the private directory through
`sync_dir` (`:893`) and `util::fsync_dir`. Both propagate their errors with `?`. The guard round 10
wrote for the protocol, `a_report_staging_directory_is_recorded_before_it_is_made`
(`src/rundir/tests.rs:4993`), exercises successful barriers only: it asserts the record present at
the instant before `create_dir` and the order

```rust
record_published < private_synced && private_synced < made
```

read from the ledger, whose entries the helpers record whether or not the barrier held. No test
arms either barrier to fail: every caller of `fail_barriers_at` (`src/util.rs:373`) targets a
checkout parent, the execution root or the public report directory, and the sole caller of
`fail_file_barriers_within` (`:378`) targets `public` (`src/rundir/tests.rs:3196`), its doc comment
saying so — "the staging record's own barrier is in the private half and holds" (`:3181-3182`).

**Mutations the suite admits** (verbatim from the lenses; unexecuted):

- recipe `staging-record-publish-swallowed`: at `src/rundir.rs:1577`, replace
  `publish(&staged_record, &record, ledger)?;` with
  `publish(&staged_record, &record, ledger).unwrap_or(());`
- recipe `staging-record-stage-swallowed`: at `:1571-1576`, replace the `stage_json(...)` call's
  terminating `?;` with `.unwrap_or(());`

Run, for each:

```bash
env UPSTROKE_RAMTARGET=/mnt/ramtarget/iso-<agent> /home/ubuntu/bin/upstroke-build cargo test --all-targets --all-features rundir::tests::a_report_staging_directory_is_recorded_before_it_is_made -- --exact
```

Expected (the lenses' reading; not observed): exit 0 under both, the ordering test's
successful-path observations being identical.

Sequence under either mutation: the record's file sync, or the private directory's sync, fails ->
the error is discarded -> `symlink_metadata` at `:1578` still reports the record present (the
rename happened, or the staged file stands unsynced) -> `create_dir` at `:1588` makes the public
staging directory with `present: true` on its ledger entry -> a crash before the record is durable
leaves the directory, and the report staged inside it, with no record or an unreadable one -> the
next write's `reclaim_report_staging` (`:1520`) finds no record, `unrecorded_report_staging`
(`:1479`) names the directory as not the protocol's, and it is passed over permanently.

**Reachability.** The production code at the reviewed head propagates both errors; what is
reachable today is the gap, not the failure: a later change that swallows either barrier — the
shape the ordering guard exists to catch, and the shape round 9's execution-root guard was found to
admit until round 10 (the recipe `root-barrier-error-swallowed`, killed only since `09d6887a`) —
leaves the suite green. The state the missing witness guards, a staging directory whose record was
never made durable, is reached from a refused or failed sync in the private half, which
`fail_barriers_at(&private)` and `fail_file_barriers_within(&private)` produce on demand
(`util::injected_barrier_fault`, `src/util.rs:409`, matches the private directory exactly and the
staged record as a file within it).

**Severity.** The four lenses graded this P1 under the repair rounds' rule 2 ("an error-propagation
guard is proven by the swallow mutation", `~/orch-pr10/repair-279-r11.md`), the threshold those
rounds ran at; the filing brief (`~/orch-pr10/file-279-residue.md`, item 4) files it at P2, the
production code propagating the errors and the defect being the missing witness. Filed at the
brief's grade, with the lenses' recorded here so a reviewer who reclassifies does it with a
`git mv`.

## Why this is deferred

Owner decision 2026-09-14: #279 merges at the reviewed head; this is fixed after the merge, and
Gate 5 judges whether it bears on a pass-rule clause.

## What the change that takes this up should do

Branch `fix-P2/crash-consistency_the-private-record-barriers-have-no-swallow-witness`. Add
`a_report_staging_record_failure_stops_before_directory_creation` beside the ordering test in
`src/rundir/tests.rs`: arm `fail_file_barriers_within(&private)` and, separately,
`fail_barriers_at(&private)`; attempt `write_report` twice while armed; require the barrier
diagnostic ("injected barrier fault"), no `DirectoryCreated` observation in the ledger, no
staging-shaped directory under `public` and no `report.json`; disarm; require publication and
reclamation. Its `expect_err` must fail under each mutation above: execute both recipes before
(the ordering test passes) and after (the new test fails), and record each `cargo exit`. The
barriers are path-shaped: the Windows guest runs the test by name.
