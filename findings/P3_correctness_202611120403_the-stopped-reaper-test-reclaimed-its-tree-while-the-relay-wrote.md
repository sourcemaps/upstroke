---
id: LOCAL-RUN-STOPPED-REAPER-SCRATCH-NOT-EMPTY-AT-RECLAIM
severity: P3
disposition: deferred
category: correctness
pr: TBD
reviewed_sha: 47986d013001373cec29f530f75e14f26671e578
location: src/agent/proc.rs:9990
provenance: undetermined
first_bad:
guard: the change that establishes what still wrote into the stopped-reaper test's scratch tree when it was reclaimed, and orders the reclaim after it, keeping the scratch guard's panic on a failed reclaim
---

# The stopped-reaper test's scratch tree was not empty when it was reclaimed

## Failure sequence

Observed once, and not reproduced. The sighting is from a full local run of the test suite on the operator's build
machine, not from CI.

1. **The test.** `agent::proc::termination::tests::a_stopped_container_reaper_ends_its_caller_rather_than_releasing_it`
   (`src/agent/proc.rs:9990` at the reviewed commit) uses the fake runtime. That runtime's reaper relay is a shell
   stand-in for `docker` that records each call in a `calls` file inside the test's scratch tree.
2. **The sighting.** In a local run on 2026-11-11 at about 15:40 UTC, at commit `6db8d300` (a notes-only change on pull
   request #368's branch), on Linux, while the machine was heavily used, the test panicked at
   `src/rundir/scratch_tree.rs:761:21`, the scratch guard's reclaim, with "scratch tree
   /tmp/upstroke-stopped-container-reaper-ZVP3QJC3ET was not reclaimed: Directory not empty (os error 39)".
3. **No rate is given.** The local runs that passed were not counted.
4. **A candidate mechanism, from reading the code, not established:**
   - the test's poll loop (near `src/agent/proc.rs:10015` to `:10034`) stops as soon as any call recorded in the relay's
     `calls` file begins with `ps`;
   - the relay stand-in (`src/runner/container/fake.rs`, near `:758` and `:759`) records a call in two appends, the
     fields first and then the newline: `printf '%s\t' "$@" >> "$relay/calls"`, then `printf '\n' >> "$relay/calls"`;
   - so the test can begin reclaiming its tree between those two appends, and the second append re-creates the file the
     reclaim is removing.

   Whether that is what happened is not established.

**Kept separate:** the same reclaim failure in another test, `unix_reaper_kills_labeled_containers`, is the open finding
`PR345-REAPER-TEST-SCRATCH-TREE-NOT-EMPTY-AT-RECLAIM`. Pull request #340 repairs that test and does not touch this one.
This one is filed on its own so that it does not go with that finding when #340 resolves it.

## What the change that takes this up should do

- **Establish the writer,** for instance by recording, on failure, what the tree still held.
- **If it is the relay's second append,** make the poll wait for a complete line, or make the relay write each call in
  one append.
- **Keep the scratch guard's panic on a failed reclaim:** a leaked tree must still fail the test.
