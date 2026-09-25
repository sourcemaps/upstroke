---
id: CONTAINER-DESCENDANT-TEST-SCANS-THE-WHOLE-HOST
severity: P3
disposition: deferred
category: correctness
pr:
reviewed_sha: d724fb162f76d86b88077cf51011013f1cfbefb8
location: src/runner/container/exec/tests.rs:4607
provenance: pre_existing
first_bad: d646791be3102d4844ba263e8cb4eaa3e1d32861
guard: the next change to `real_docker_a_container_contains_a_daemonised_descendant`, or a fix-P3/ branch taking this up: make the two markers the test's own, or scope the scan to the test's own container, and correct the notes' "nothing else on the machine uses" claim; until then two runs of the suite overlapping on one host can fail it, or meet its vacuity guard with each other's processes
---

## Failure sequence

`real_docker_a_container_contains_a_daemonised_descendant` (`src/runner/container/exec/tests.rs:4603`)
runs a container whose leader is `sleep 903222` (`LEADER_MARKER`, `:4604`). The leader starts a
`setsid`-detached `sleep 903111` (`DESCENDANT_MARKER`, `:4605`). After the runner's 3 s timeout has
stopped and removed the container, `pids_with` (`:4607`) reads **every** `/proc/<pid>/cmdline` on the
host. The assertion at `:4706` fails if any process anywhere has `903111` in its argv. Both markers
are constants, so every run of the suite on the machine uses the same two. The module's notes say
"each `sleep` carries a marker argument nothing else on the machine uses"
(`docs/internals/runner/container/exec/tests.md`, under the test's heading). That holds only while
one run of the suite is on the machine.

What happened on the build box, 2026-09-25, from `docker events` for the test's run id
`01KZR3BGATED00000000000002`. The two runs' container names carry different repository keys,
`163b871fce7e3348` and `d370502e4a9eb490`, so no *name* collided:

1. **Run A, the findings_gate5_deferred gate run.** It was the full suite, at `c5071243`, whose
   `src/` tree is `d724fb16`'s. Its container is stopped at its timeout (`kill` 04:14:06.596Z) and is
   gone at 04:14:16.705Z (`die`).
2. **Run B, Gate 5 Run 6's `run-each.py`.** It runs one test per process from a `d724fb16` probe
   binary. The same test started at 04:14:13.786Z, as PID 4103339. Its container was created at
   04:14:14.288Z, started at 04:14:14.377Z, and ran until 04:14:27.347Z (`die`), with its own
   `sleep 903111` inside.
3. Run A's test scans `/proc` just after 04:14:16.705Z. It finds Run B's live descendant, PID
   4104602, allocated about 1,260 PIDs after Run B's test process.
4. Run A fails: "a `setsid`-detached descendant survived its container, so
   `invariants_introduced[0]` ("container contains descendants") does not hold: ["4104602"]". Run B's
   copy passed in 13.63 s.

The same constant weakens the other side. The vacuity guard requires the leader and the descendant
to be **seen running at once** (`seen == (true, true)`). Its sampler scans the whole host as well, so
another run's processes can satisfy it. From 04:14:13.8Z to 04:14:16.7Z, Run A's two `sleep`s were
on the host while Run B's sampler was looking.

So when two runs of the suite overlap on one host, the test can go red with nothing wrong, and its
control can be met by another run. Containment itself is not in question: each run's own container
went away, and PID 4104602 had exited by 04:16Z.

## Evidence

On the build box, in `~/orch-pr10/findings-gate5-deferred-evidence/gates/`:

- `attempt-1-descendant-containers.txt`, and `attempt-1-docker-events-raw.txt` covering
  04:13:20–04:15:45Z.
- `attempt-1-run6-descendant-run.txt`, Run B's run-dir time and trace head, and
  `attempt-1-run6-probe-results.jsonl`, Run B's per-test results, index 1773.
- `eight-logs-c507124-attempt-1/03-test.log`, Run A's failure.

The overlap is recorded as it happened, in `~/orch-pr10/questions/findings_gate5_deferred-2.md`, and
Run A was not re-run.

It is not `R5-SEAMS-006`. That one concerns the fixed container *name* key when `CARGO_TARGET_DIR`
is unset, and the pre-clean killing a sibling run's container
(`docs/internals/runner/container/fake.md`). Here the names differed, and what collided is the
`/proc` marker. No file under `findings/` or `reviews/` named this test, its markers or its scan.

## What the change that takes this up should do

Pick one of these:

- **Make the markers the test's own.** They are `sleep` durations, so they must stay numbers (the
  notes explain why). Derive them per run, for example from the test process's pid and a nonce.
  Keep them far apart and implausible as durations.
- **Scope the scan to the test's own container.** For example, match `/proc/<pid>/cgroup` against
  the container id the runner started, or read the container's pid namespace. Two runs then cannot
  see each other's processes, whatever markers they use.

Either way, apply the same scoping to the sampler that meets the vacuity guard, and correct the
notes' sentence. The regression is two runs of the test overlapping on one host, as above: both must
pass, and each one's guard must be met by its own processes.
