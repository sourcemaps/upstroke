---
id: PR329-CONCURRENT-PUBLICATIONS-RACE-THE-FINAL-RENAME-ON-WINDOWS
severity: P2
disposition: deferred
category: portability
pr: 329
reviewed_sha: 8df424364809c99fe1b8098ac71918815133d8e7
location: src/agent/proc/test_support/readiness.rs:115
provenance: pre_existing
first_bad:
guard: B's next implementation round, which measures concurrent stage-then-rename publications onto one destination natively on Windows and decides for the production publishers: a bounded rename retry, a single writer, or a stated precondition
---

## Failure sequence

Executed in saved CI, run 37027439407 (PR #329's head `8df42436`, the `test (winguest)` leg); found by design review
round 9's gitenv lens (FUB-D9-WINPUBLISH, `~/orch-pr11/reviews/review-329-d9-triage.md`).

    eight publishers finish staging and meet both barriers (src/agent/proc/tests.rs:4033 at 8df42436)
    -> their final renames race onto one destination
       (src/agent/proc/test_support/readiness.rs:115 at 8df42436)
    -> two return Windows error 5 ("Access is denied")
    -> the test's unconditional success assertions panic
       (`agent::proc::tests::concurrent_publications_do_not_share_a_staging_name`)

The barriers attribute the failure to the final publication step; the log does not establish the precise Windows
cause. The passing rerun (37033254411) does not close it.

**Why P2, not P3.** Production has the same stage-then-rename shape with no rename retry
(`src/workspace_manager.rs:6069`, `src/rundir.rs:826`, `src/runner/container.rs:911`, all at `8df42436`); the legacy
include publisher instead checks whether a competitor installed the required bytes (`src/workspace.rs:1534` at
`8df42436`). Production damage needs a reachable interleaving, which is not shown.

**Replaces** the provisional P3-only attribution of the winguest flake in the triage `review-329-d9-triage.md`.

Filed at PR #329's implementation round as a record only (the decision appendix §10.1,
`~/orch-pr11/owner-package/DECISION-APPENDIX.md`); it decides nothing, and that round neither measures nor changes a
publisher.

## What the change that takes this up should do

Measure concurrent stage-then-rename publications onto one destination natively on the Windows guest, with the
renames' error codes recorded, and decide for each production publisher named above: a bounded retry of the final
rename when Windows answers error 5 for a destination another publisher holds, a single writer per destination, or a
stated precondition that no two publishers target one destination. The test's own publisher
(`readiness::publish_between`) follows whichever the production publishers adopt.
