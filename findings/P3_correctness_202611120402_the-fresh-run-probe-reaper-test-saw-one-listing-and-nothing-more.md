---
id: BASELINE-FLAKE-FRESH-P4-PROBE-REAPER-ONE-LISTING
severity: P3
disposition: deferred
category: correctness
pr: TBD
reviewed_sha: 47986d013001373cec29f530f75e14f26671e578
location: src/engine/topology/create/tests.rs:4922
provenance: undetermined
first_bad:
guard: the change that establishes why the creator's reaper stopped after its first listing in this test, and either repairs the reaper if it gave up wrongly or makes the test's setup immune to what stopped it, keeping the four-call assertion
---

# The fresh-run probe reaper test saw one listing and nothing more

## Failure sequence

The test is
`engine::topology::create::tests::a_fresh_runs_p4_probe_container_is_killed_by_its_reaper_before_run_started`,
at `src/engine/topology/create/tests.rs` line 4922 at the reviewed commit. It
does not use Docker: it uses the fake runtime with a shell stand-in for
`docker` that a real reaper process executes. It starts a creator child, waits
up to 120 seconds for the creator to have one running container, kills the
creator, waits up to 120 seconds for the reaper's calls, and asserts that they
were exactly four: a listing, a kill, a removal, and a second listing that
finds nothing.

It has been seen to fail once, on the operator's build machine and not in CI:
a full local run of the test suite on 2026-11-12 at about 01:25 UTC, at commit
`65f87b62` (a documentation-only change on pull request #368's branch), while
other builds and a Windows guest were using the machine. The suite's result
was 3736 passed, 1 failed, 204 ignored, in 245.73 s.

The panic was at `src/engine/topology/create/tests.rs:5032:5`, with the
message beginning:

> the creator's reaper listed its P4 probe by the run's private root and this
> incarnation, killed and removed it, and listed again to find nothing:

followed by the calls recorded. One call only was recorded: the first listing
(`ps --all --quiet --no-trunc` with the two label filters). No kill, no removal
and no second listing were recorded. The suite's log shows the test had been
running for over sixty seconds, which fits the test waiting out its bound for
calls that never came.

The same commit's suite passed when run again an hour later (3737 passed, 0
failed). This is the only failure of this test among the 273 failed local
suite runs the operator's logs hold, from September to November 2026.

Why the reaper stopped after its first listing is not established. From
reading the reaper (`reclaim_labeled_containers` in `src/agent/proc.rs`), two
paths return after the first listing without another call, and they are
candidates only:

- the listing reads empty; or
- the reaper does not establish its exclusion over the worktree, which it
  tries once without waiting.

Which one happened, or whether it was something else, the record does not
show: the test's scratch tree was gone.

## What the change that takes this up should do

- On the next sighting, capture the reaper's own diagnostics and the
  stand-in's record before the scratch tree is removed.
- Decide from that whether the reaper gave up wrongly under load, which would
  be a defect of the reaper and graver than P3, or the test's setup raced it.
- Keep the four-call assertion; do not retry or skip the test.
