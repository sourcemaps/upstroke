---
id: CI-FLAKE-REAL-DOCKER-START-DEADLINE
severity: P3
disposition: deferred
category: correctness
pr: TBD
reviewed_sha: 7d96f365761e1187805bf3dba2f80043f8fa487f
location: src/runner/container/exec/tests.rs:5449
provenance: undetermined
first_bad:
guard: the change that gives the two timed-out invocations a deadline a slow Docker daemon can create and start a container inside
---

## Failure sequence

Seen on the build machine on 2026-11-10 and 2026-11-11, in full-suite baseline runs at heads that
changed only documentation or tests elsewhere.

Two tests in `src/runner/container/exec/tests.rs` run an invocation that is meant to time out,
under a deadline of three seconds, and each expects a timed-out output:

- `real_docker_a_containerized_program_reads_its_exact_stdin_then_eof`, whose last invocation
  sleeps six hundred seconds. Its deadline is at line 5449.
- `real_docker_a_container_contains_a_daemonised_descendant`, whose request's timeout is set at
  line 6337.

1. **The three seconds also have to cover Docker creating and starting the container.**
2. **When that alone takes longer, the runner refuses the invocation, correctly.** A refusal is an
   error and not a timed-out output.
3. **The test panics:** it expects a timed-out output, and gets an error.

Two refusals have been seen:

- **The container was still `created` when the invocation's deadline passed, so it never
  started.** That is the first test, panicking at line 5455.
- **The start was not asked, because the invocation's deadline had passed.** That is the second
  test, panicking at line 6351.

The first test failed four times, each while another checkout was compiling or a CI job was using
the Windows guest on the same machine. The second failed twice, once under such load and once on
an otherwise idle machine. The first test passed alone three times out of three, and four times out
of four under forty busy loops with nothing else using Docker, so processor load alone does not
produce it. Container start measured 150 to 160 milliseconds on the idle machine.

**The runner's refusals are correct behaviour and are not what this finding is about.** The defect
is that each test's deadline, for an invocation meant to time out, is shorter than a slow daemon's
container creation and start.

## What the change that takes this up should do

- **Give those two invocations a deadline with room for a slow start,** still far below the time
  their programs sleep.
- **Keep each test's assertions.**

No mutation witness is available: the failure has not been produced on demand.
