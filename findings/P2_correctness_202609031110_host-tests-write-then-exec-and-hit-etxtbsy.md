---
id: W2-HOST-TESTS-WRITE-THEN-EXEC-ETXTBSY
severity: P2
disposition: deferred
category: correctness
pr: 
reviewed_sha:
location: src/runner/host/tests.rs:7179
provenance: pre_existing
first_bad:
guard: a change that makes the host suite's write-then-exec shims immune to a descriptor inherited across a fork in another harness thread (the repair this file asks for), or whoever meets the failure again
---

## Failure sequence

`an_empty_path_entry_never_reaches_the_workspaces_own_copy_of_a_bare_name` (`src/runner/host/tests.rs:7179`) writes an executable through `marker_shim` (`:5329`) and immediately spawns it. In a gate run at `d8f4d13` the spawn failed with `"an empty entry before a real installation: a raw spawn: Text file busy (os error 26)"` — **ETXTBSY**, a concurrently-forking thread in the same process still holding a write descriptor at `execve`. A textbook write-then-exec race under a parallel harness, not a logic error. **Both functions are byte-identical from `1cbdccd` through `ae2a58f`** (`marker_shim` sha256 `f666ed74…`, 701 bytes; the test `098f21e8…`, 4489 bytes), so the race travelled unchanged through the M4, M5 and M6 splits

## What the change that takes this up should do

Owner, as the ledger recorded it until PR11: the slice that next changes `src/runner/host/tests.rs`, or whoever meets the failure again. Re-guarded below.

Pre-existing, not reproducible on demand, and fixing it inside a split packet would put a concurrency change in a refactor's diff. **Both prescriptions this finding has carried are refuted, which is the most useful thing in the row**: `drop` plus `sync_all` closes nothing, because the writer is `std::fs::write` and it already drops its handle; and rename-into-place does not help either, because a `fork` that inherits the descriptor inherits it whatever the path is called. A repair must demonstrate that it addresses **fd inheritance across a `fork` in another harness thread**. Misattributed by construction — the failure lands on whichever test happens to be spawning. Full derivation: **§43**

Appended to `reviews/FINDINGS.md` §2 by §43, the 2026-09-03 W1/W2 decomposition review, which recorded it as pre-existing: neither introduced nor activated by the diff in front of it. The row carried no severity label; **P2** here is this migration's judgement from the consequence described above, not the reviewer's own word.

## Re-guarded by PR11 (2026-10-01)

PR11 is a slice that changed `src/runner/host/tests.rs`: phase 1 migrated its Runner call sites to
the boxed-future surface and added the cancellation, parity and two-invocation tests, and phase 3
added the carried-lease reaper witness (the PR11 record, §6 and §7). It did not take this finding
up. The repair this file asks for is one that "demonstrate[s] that it addresses fd inheritance across
a `fork` in another harness thread" — a property of how every spawn in the host suite meets every
other thread's spawns, not of the tests PR11 added, and so a harness-wide change outside PR11's
scope; and both prescriptions this file records are refuted. No PR11 run is known to have met the
failure: no file under the build box's `~/orch-pr11/logs/` — the phases' and repair rounds'
development, gate and mutation runs, and the CI job logs they saved — carries `Text file busy`
(searched 2026-10-01; `~/orch-pr11/logs/pr11_impl_g/findings/etxtbsy-search.txt`).

A guard of "the slice that next changes the file" names whoever is passing through, and PR11 shows
what that buys: a slice with no reason to take it up, deferring it again. The guard is therefore the
change that makes the shims immune to an inherited write descriptor — for instance one that execs a
copy written and closed before any thread can fork, or that retries `ETXTBSY` with a bound, with the
measurement this file says a repair owes — or whoever meets the failure again.

