---
id: G5RUN2-DOCKER-DIAGNOSTIC-TEST-DEPENDS-ON-A-SHORT-TMPDIR
severity: P3
disposition: deferred
category: correctness
pr: 
reviewed_sha: a2fe09c2414996485a95c0b916f057ece8201ed7
location: src/runner/container/tests.rs:4114
provenance: pre_existing
first_bad:
guard: the change that next takes up `real_docker_prints_the_transcribed_unreachable_diagnostics` or the fixture's denied-socket root; the test should build its unusable socket at a path short enough for `sun_path` whatever `TMPDIR` holds, or assert on the classifier's answer for the diagnostic it actually provoked
---

## Failure sequence

`runner::container::tests::real_docker_prints_the_transcribed_unreachable_diagnostics` builds a
socket the process may not use under `std::env::temp_dir()` and asserts that the live Docker CLI
prints a diagnostic the classifier calls *unreachable* rather than *an answered failure*. The path
it builds is `<TMPDIR>/upstroke-r2-denied-<pid>/docker.sock`. A Unix socket path is limited to 107
bytes (`sun_path`), so when `TMPDIR` is long the CLI does not fail the way the test intends: it
prints `Failed to initialize: unix socket path "…" is too long`, which the classifier reads as an
**answered failure**, and the assertion fires.

Observed at `a2fe09c2` on 2026-09-15 during Gate 5 run 2's first full-suite export run, whose
`TMPDIR` was this session's scratchpad — **102 characters**, so the socket path the test builds under it is
**141 characters**, against `sun_path`'s 108-byte array (107 usable):

```
thread 'runner::container::tests::real_docker_prints_the_transcribed_unreachable_diagnostics'
panicked at src/runner/container/tests.rs:4114:9:
[a socket this process may not use] the live CLI printed a diagnostic this classifier calls an
answered failure, so a census with no container evidence would refuse: "Failed to initialize: unix
socket path \"/tmp/claude-1000/…/tmpdir-export1/upstroke-r2-denied-1455436/docker.sock\" is too long"
```

**Both directions executed** (`~/tactus-artifacts/g5-evidence-a2fe09c/docker/controls.log`), the
test alone with `UPSTROKE_REQUIRE_DOCKER=1` at the same head:

- `TMPDIR` = a **105**-character directory (socket path 144): `FAILED`, `cargo exit=101`, the message above;
- `TMPDIR` = a short directory under `/home/ubuntu`: `ok`, `cargo exit=0`. The control log records that run
  and its status, not the path, so no length is quoted for it here.

It is not a defect of the engine: nothing in `src/runner/container.rs` reads `TMPDIR` in production,
and the suite passes under the ordinary `/tmp`. It is a test whose subject changes with the
environment it is run in, which costs a red and an investigation for anyone who runs the suite with
`TMPDIR` pointed somewhere long — which the residue-measurement rule this programme uses asks for.

## What the change that takes this up should do

Give the denied socket a root the fixture controls (a short path, or a directory created under the
repository's own scratch root) rather than `std::env::temp_dir()`, or widen the assertion to accept
the `sun_path` diagnostic as *unreachable* — whichever the classifier's contract says a socket that
cannot be addressed at all should be. Then run the test with a long `TMPDIR` as the regression: it
must stay green.
