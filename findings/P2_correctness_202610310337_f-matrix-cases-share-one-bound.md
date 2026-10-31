---
id: G6-F-MATRIX-AGGREGATE-BOUND
severity: P2
disposition: deferred     # open: implemented by dae436e9 below it, checked only, not yet built for test, run or reviewed; the default-concurrency full suite at the repaired head and this pull request's fresh reviews decide its close
category: correctness
pr:                       # assigned when the pull request is created
reviewed_sha: 233ed5f82dcc437841d97eb12ee273952983c0a2
location: src/engine/topology/coordinator.rs:25586
provenance: introduced_by_feature
first_bad: e160ccba834a4ba2342a0b4dac7fca1b20c87813  # the gate and review matrices' single bound, from F's closures 1 to 3; the launch matrix's came with 0ece4e493afbf6e4643b0d84294d180cab210ddf (F-R1)
guard: a_gate_verdict_read_in_a_snapshot_whose_registration_is_not_whole_is_refused, a_gate_run_failing_in_a_snapshot_whose_registration_is_not_whole_is_refused and a_review_verdict_read_in_a_snapshot_whose_registration_is_not_whole_is_refused, each case under its own BOUND through f_bounded_case
---

## Failure sequence

Gate 6's Windows rows at `233ed5f8` saw this on the CI guest, in its full suite at default concurrency with the gate's build shape.
It appeared in three samples: W-S1, its one retry W-S1-R2, and the D1 diagnostic sample at the same source plus a test-only
progress recorder.

1. **One bound around a whole matrix.** Each of the three tests ran its whole `F_MATRIX` matrix inside one `bounded()` call, with
   `BOUND = 120 s` (`src/engine/topology/coordinator.rs:2177`, `:11326-11349`):
   - the gate test at `:25586`, 16 cases;
   - the launch test at `:26121`, 12 cases;
   - the review test at `:26315`, 12 cases.
   The cases are independent and run one after another.
2. **The matrices are slow but progressing.** D1 recorded every case's start and normal end, and `f_report`'s success, in all three
   matrices:
   - they completed at 187.8 s (launch), 232.6 s (gate) and 213.6 s (review);
   - the slowest single case took 39.1 s;
   - each attempt case took 5-9 s and each verification case 20-39 s.
3. **The bound fires, and the test fails.** At 120 s `bounded()` panics with its fixed text, "… did not end within 120s: the
   coordinator waited on its inbox for a pipeline that was itself waiting on the coordinator". The three tests fail: the lib closed
   `3215 passed; 3 failed; 105 ignored` in all three samples, and Cargo then stops before main and the probe.
4. **The same tests pass elsewhere:**
   - alone, in 40-50 s;
   - in a full suite with the outer harness serialized;
   - in CI on the same guest, where the observed log-delivery intervals were about 65-83 s.

**Why:**
- The bound measured how many cases a test chains and how loaded the runner was, not whether any one case was wedged.
- That is standards §12's warning: a deadline short enough to expire on a loaded runner has become the failure it exists to prevent.

**What this finding does not establish:**
- No product defect.
- No cause beyond the bound's scope.
- No timing of the two earlier runs' cases. D1 was a different binary, with the recorder added, and the parent `bounded()` timeout
  was not timestamped.

## What the change that takes this up should do

- **Bound each case, not the matrix.** Run each case's setup, run, outcome read and teardown inside its own existing `bounded()`
  call, named by the case.
- **Keep everything else:**
  - the 40 cases: `F_MATRIX`'s two subjects at widths 1 and 3, by each test's inner list;
  - every assertion and expected refusal;
  - `f_report` reading every outcome;
  - `BOUND = 120 s` and the shared `bounded()`;
  - default concurrency.
  There is no deadline increase, skip, filter or serialization.
- **A case that never ends still fails its test within `BOUND`**, now naming the case.
