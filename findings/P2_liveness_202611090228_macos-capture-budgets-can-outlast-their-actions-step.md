---
id: G7-M2-OPS-002
severity: P2
disposition: deferred
category: liveness
pr: TBD
reviewed_sha: 7e80d921bbe98f5babd773bfb7d5236f685e770d
location: .github/workflows/g7-platform-container-acceptance.yml:63
provenance: introduced_by_feature
first_bad: 7e80d921bbe98f5babd773bfb7d5236f685e770d
guard: the next change to the budgets of `.github/g7-platform-environment/macos-container-acceptance.sh` or to the workflow step envelopes that hold them
---

## Failure sequence

The capture step (`timeout-minutes: 30`, 1800 s, workflow line 63) runs
`macos-container-acceptance.sh capture`. That is six bounded prerequisites of at most
30 + 30 + 60 + 60 + 60 + 60 = 300 s, then the acceptance command (`G7_RUN_SECONDS`, 1500 s), then
a bounded leftover-container query (60 s), then the frame decode and the summary, which have no
bound of their own.

    a slow but valid capture: each prerequisite near its limit, the acceptance run near 1500 s
    -> 1860 s of individually bounded work is scheduled inside the 1800 s step
    -> Actions cancels the step before the leftover query or SUMMARY-capture.txt completes
    -> the job is red and its evidence incomplete, though no command passed its own bound

The build step (`timeout-minutes: 47`, workflow line 47) has the same shape. Provenance (120 s)
plus the build (`G7_BUILD_SECONDS`, 2700 s) fill all 2820 s, with no margin for the supervisor's
TERM-to-KILL grace or the step's terminal records.

A cancellation never becomes a pass: the step fails, and cleanup and upload still run
(`if: always()`). Found by source review and arithmetic at 7e80d921, in the platform operations
review of the G7 M2 container acceptance candidate. It has not been observed in a run.

## What the change that takes this up should do

Clamp each phase's command budgets to one phase deadline that keeps a reserve for the decode, the
leftover inspection and the summary, as `macos-docker-capability.sh`'s shared `DEADLINE` and
`FINAL_RESERVE` already do. Or widen the two step envelopes to cover those operations and the
supervisor's grace. Keep the outer Actions bound, and never let a cancelled step read as
complete.
