---
id: BOX-FLAKE-PRE-FLIGHT-PROBE-REAPER-KILLED-NOTHING
severity: P3
disposition: deferred
category: correctness
pr: 421
reviewed_sha: e1e39e974f631203bd4609926b6738470b54ec46
location: src/engine/topology/coordinator.rs:21070
provenance: undetermined
first_bad:
guard: the change that next edits reclaimed_by_its_reaper (src/engine/topology/coordinator.rs:21057) or a_resume_killed_inside_its_pre_flight_probe, keeping the assertion, with no retry, skip or weaker assertion
---

# The pre-flight probe's reaper killed nothing in three build-box baselines

## Failure sequence

    a resuming incarnation's coordinator dies inside its pre-flight probe, leaving one probe container labeled with its private root and incarnation
    -> one reaper lists the containers carrying those labels
    -> the reaper records that one listing and no `kill` and no `rm`
    -> the assertion that the reaper listed, killed and removed the probe container and listed again fails the test

`engine::topology::coordinator::tests::a_resuming_incarnations_pre_flight_probe_container_is_killed_by_its_reaper_when_the_coordinator_dies_inside_it`
(`src/engine/topology/coordinator.rs:21409` at `e1e39e97`) passes the dead coordinator's probe
container, as `expected`, to `reclaimed_by_its_reaper` (line 21057). Its assertion at line 21070
compares `(calls.first(), calls.last(), of("kill"), of("rm"), calls.len())` with
`(Some(&listing), Some(&listing), expected, expected, 2 + 2 * expected.len())`: with one expected
container, four calls.

## The sightings

All three are the build machine's own pre-push baseline, `cargo test --all-targets --all-features`
through its wrapper. Each failure was its run's only failure, and the next baseline at the same head
passed this test.

| date | head (pull request) | failed log, SHA-256 | lines | passing re-run, SHA-256 |
|---|---|---|---|---|
| 2026-11-18, about 16:37Z | `2e8b8308` (#389) | `50b841480a18b2f5861dcdcda6c357ddb83f5ed2e798479757874b9758b4bb4a` | 3032 (running over 60 s), 3038 FAILED, 4583 panic at `coordinator.rs:21014:9`, 4592 `3853 passed; 1 failed; 206 ignored` | `9279db0e8383e8c5da2d503e2780aab9ee910f321ff9323d329783105f91b992`, line 1288 `ok` |
| 2026-11-19, about 04:12Z | `54ce702b` (#389) | `27cf768f67d3fdf0a6d7c6c42321a4791dcc57d0ccd779945afa5f8810c477f1` | 2978, 2979 FAILED, 4532 panic at `:21014:9`, 4541 `3854 passed; 1 failed; 206 ignored` | `1cfee05a44be45bc506b3b7a1eb056ac963c708290426da8dcc04236098a3bca`, line 1291 `ok` |
| 2026-11-25, about 05:40Z | `51776334` (#421) | `24f2e5931426cdffed14bfa0e45ea94a8c3cb53ea0913d7841ea4a7fbf411dbe` | 3251, 3252 FAILED, 4859 panic at `:21070:9`, 4868 `4183 passed; 1 failed; 207 ignored` | `9af9905000124295b7e24e1c33f8cea5c995e7b140381b26deedcf76f01133d5`, line 1454 `ok` |

The assertion moved from line 21014 to line 21070 between those heads; its text is the same.

In each failure the assertion's left side is `(Some(listing), Some(listing), [], [], 1)` and its
right side `(Some(listing), Some(listing), [<the probe container>], [<the probe container>], 4)`,
where `listing` is `ps --all --quiet --no-trunc` filtered on the labels
`upstroke.private_root=/tmp/upstroke-reaper-pre-flight-<random>/private` and
`upstroke.incarnation=01KZP5CONTAINERS2000000002`.

Neither #389 nor #421 changes anything under `src/engine/topology/`. The box's load average was
recorded once, about 6 on 2026-11-25; it was not recorded for the other two.

## What is and is not established

- **Established:** in three box baselines over eight days, the reaper recorded one listing and no
  `kill` or `rm` call for the dead coordinator's probe container; each time the next baseline at
  the same head passed.
- **Not established:** what the listing returned, and whether the probe container carried its labels
  when the reaper listed.
- One reading, **inferred, not established**: the reaper's listing ran before the probe container
  was visible to it.
- These readings are not exhaustive.
- **Not established: the rate.** Three sightings, all on the build machine; none is known in CI.

## What the change that takes this up should do

Not proposed here (ruling r39 step 7); the guard names the change that takes it up.
