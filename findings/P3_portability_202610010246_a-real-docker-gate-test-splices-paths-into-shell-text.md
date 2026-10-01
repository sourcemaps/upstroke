---
id: PR11-R5-REAL-DOCKER-GATE-SCRIPT-SPLICES-PATHS
severity: P3
disposition: deferred
category: portability
pr: 327
reviewed_sha: a9e88039e7b789a426224fb9de3920774313fc37
location: src/runner/container/exec/tests.rs:3899
provenance: pre_existing
first_bad: e80564d8645b63c5fcbf4909d38e1cfbf0b4ce40
guard: the change that next touches the real-Docker tests of src/runner/container/exec/tests.rs, or a standards §9 sweep of test code, whichever comes first
---

## Failure sequence

On a host with Docker, under a legal temporary directory whose name carries an apostrophe:

    TMPDIR=/some/where/reviewer's-temp; run
      runner::container::exec::tests::real_docker_confines_a_gate_to_its_mount
    -> the test builds its gate's shell script by splicing each withheld path into the program text
       inside single quotes: printf 'READ {path}: '; cat '{path}' …; echo … > '{path}'
    -> the apostrophe in the path ends the quoting, the script does not parse, the gate prints nothing
    -> the test fails at "the gate could not read its own workspace, so nothing here is measured: \"\""
       before it measures the confinement it exists to measure

Standards §9: no values concatenated into shell text. The same class as the PR11 full review's `FULL-REG-1`
(the reaper test's `HostHeld`, repaired in PR11's round R5), found by that round's executed class search: PR11's
test modules run under `TMPDIR=…/reviewer's-temp` at the start head and after the repair
(`~/orch-pr11/logs/pr11_repair_r5/measure/apostrophe-tmpdir/SUMMARY.txt` on the build box). This test is not one
of PR11's helpers — the lines are `e80564d8`'s (2026-09-02), an ancestor of PR11's base — so the round filed it
rather than widening its own diff.

**Test-only, and P3.** No production path builds this script; CI's temporary directories carry no apostrophe; the
test runs only where a Docker daemon answers.

## What the change that takes this up should do

Pass each withheld path to the gate's shell as data — positional parameters (`sh -c '…"$1"…' gate <path>…`) or
environment variables the container receives — never inside the program text, and witness it under a temporary
directory whose name carries `'`.
