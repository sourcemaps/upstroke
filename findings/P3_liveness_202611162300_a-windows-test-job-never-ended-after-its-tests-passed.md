---
id: CI-WINDOWS-HOSTED-JOB-OUTLIVED-ITS-TESTS
severity: P3
disposition: deferred
category: liveness
pr: TBD
reviewed_sha: 725280c9fb9824485a73262b034c87077939d34d
location: .github/workflows/ci.yml:386
provenance: undetermined
first_bad:
guard: the change that establishes which test leaves a process of the library's test executable alive after the harness's summary line, or a recurrence on any Windows job, whichever comes first
---

# A hosted Windows test job never ended after every one of its test harnesses had passed

## Failure sequence

The Windows suite runs Cargo with its output piped into PowerShell's `Tee-Object`, so the step ends
only when every writer of that pipe has closed it. `.github/workflows/ci.yml` does this in
`test-windows`'s step "Test, and witness that the suite ran" (line 386), and its hosted twin
`test-windows-hosted` does the same (line 504); neither step carries a `timeout-minutes` of its own.

    `cargo test --all-targets --all-features | Tee-Object -Variable log`
    -> the library harness, `upstroke-<hash>.exe`, runs its tests; some of them re-run the test
       executable itself as a helper subprocess, which inherits the harness's standard output
    -> (inferred) one such helper outlives its test, and so holds the write end of the pipe open
    -> every harness prints `test result: ok`, and Cargo has nothing left to run
    -> `Tee-Object` never sees the end of its input, so the step neither ends nor fails
    -> nothing ends the job but its time limit; the job is cancelled, not failed, and every test in
       it had passed

## The one sighting

Measurement run 43719719561, on branch `measure/windows-hosted-20261115` at `17895712`, job
`shard (2-of-4)` (id 132828860062), on a GitHub-hosted `windows-latest` runner with 4 processors.
The log is `evidence/job-shard__2-of-4__-132828860062.log` of that measurement.

- The step ran `cargo test --all-targets --all-features -- <filters>` piped into
  `Tee-Object -Variable log`, as the measurement workflow does and as `ci.yml` pipes the suite.
- The library harness `upstroke-8ddcc7d640fdf222.exe` started at 13:38:30 (log line 404) and
  printed, at 14:06:19 (line ~1200):

      test result: ok. 743 passed; 0 failed; 33 ignored; 0 measured; 2731 filtered out; finished in 1668.86s

  The two remaining harnesses, `src\main.rs` and `examples\probe.rs`, each printed
  `test result: ok` at 14:06:19.
- Nothing more was printed until `##[error]The operation was canceled.` at 15:34:57, when the job's
  120-minute limit ended it: about 88 minutes with every harness finished.
- At the job's clean-up, at 15:35:02, the runner printed `Cleaning up orphan processes` and
  `Terminate orphan process: pid (9428) (upstroke-8ddcc7d640fdf222)`: a process of the library's
  test executable was still alive after the step had ended.
- The job's CPU reading was 24.8% busy on 4 processors; the three other shards of the run read
  75–91%.
- The other three shards, and the three whole-suite jobs of the same run, ended normally.

## What is and is not established

**Established**, from the log: every harness of the shard reported a pass; the step then produced
no output for about 88 minutes and was cancelled by the job's limit; a process of the library's
test executable was alive when the runner cleaned up; and the job was mostly idle over its life.

**Inferred**, not observed: that the leftover process is a child the suite spawned from its own
executable (several tests re-run the test binary as a helper subprocess), that it outlived its test,
and that it kept open the pipe `Tee-Object` reads, so the step waited for an end of input that never
came. The low CPU reading fits a wait rather than work, and does not prove one.

**Not established**: which test spawned the process, since the log does not record it; whether the
helper was waiting on something or merely never told to exit; how often this happens, from one
sighting in seven jobs of one run; and whether the guest's job, `test (winguest)`, can hang the same
way. The guest pipes the suite identically, but it has its own 20-minute job limit and no sighting
there is recorded. No `src/` change is identified as the cause, hence `provenance: undetermined`.

## What the change that takes this up should do

1. **Establish which test leaves the process.** Once the harness has printed its summary line,
   list the live processes of the test executable with their command lines (for instance
   `Get-CimInstance Win32_Process` filtered on the executable's name, printing `ProcessId`,
   `ParentProcessId` and `CommandLine`), so that a recurrence names the helper's arguments and from
   them the test that spawned it. The suite step itself is blocked on the pipe at that point, so the
   listing has to come from outside it: a watcher started alongside the suite, or an
   `if: always()` step after a suite step cut off by its own timeout (item 3).
2. **Make that test reap its helper on every path**: kill and wait for the child when the test
   returns, fails or panics, for instance through a guard whose drop does so, so that no helper
   outlives the test that started it.
3. **Bound such a hang in CI meanwhile** with a `timeout-minutes` on the suite step itself, in
   `test-windows` and `test-windows-hosted`, so that a step whose tests have all finished fails at
   a bound of its own instead of holding the runner until the job's limit.

Do not widen any test's time bound, and do not retry the step or the job: neither tells which test
leaves the process, and a retry would hide the next sighting this finding's guard waits for.
