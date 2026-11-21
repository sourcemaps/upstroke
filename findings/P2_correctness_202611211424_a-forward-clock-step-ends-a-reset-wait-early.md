---
id: PR390-WALL-CLOCK-DEADLINE
severity: P2
disposition: deferred
category: correctness
pr: 390
reviewed_sha: ef3c3768125fbb50ac262eebcf39f893b166c1c7
location: src/engine/topology/settle.rs:175
provenance: introduced_by_feature
first_bad: 8ae725d52902a6e0bc1313442a787832498f3f90
guard: the change that builds design 13's retry-at-reset timer, or the next change to how Deferral::wait_toward measures elapsed time
---

## Failure sequence

This is the frontier reviewer's sequence (`review-frontier-ef3c376.md`). Every fact below was read
from the code. None of it was run.

- `Deferral::wait_toward` holds one wait toward an absolute Unix target. It takes the elapsed part
  of that wait as `max(now_unix - started_unix, slept)`, and the remaining part as the wait's length
  minus that (`src/engine/topology/settle.rs:175-178`). `now_unix` is the coordinator's wall clock,
  read again at every call of `TopologyRun::back_off` (`src/engine/topology/run.rs:1807`). The
  target is the pool's recorded reset plus the two-minute margin (`src/interaction.rs:496-503`,
  `src/engine/topology/run.rs:1859-1861`).
- A call sleeps at most `MAX_DEFER_BACKOFF`, which is ten minutes
  (`src/interaction.rs:487`, `src/engine/topology/settle.rs:179-181`). A wait of hours therefore
  takes many calls, and the clock is read before each one.
- During the wait, the host clock steps forward by some amount S, from an NTP step or a manual
  correction. At the next call, `now_unix - started_unix` exceeds the real elapsed time by S. The
  `max` takes it, so `remaining` comes out S short. When the shortened `remaining` fits in one
  slice, the call sleeps that slice, drops the held wait, adds one to `round` and answers `Elapsed`
  (`src/engine/topology/settle.rs:185-194`). `back_off` appends `defer_wait_elapsed`
  (`src/engine/topology/run.rs:1822`), and the deferred task wakes up to S early. If S is longer
  than the margin, the task wakes before its pool's limit has reset.
- The woken task runs against a pool that is still exhausted. Its attempt meets the same limit and
  settles deferred again (`src/engine/topology/settle.rs:73-76`).
- The next `back_off` finds no wait held, because `Elapsed` dropped it, so it calls
  `awaited_reset` to choose a reset. The stepped clock already reads the recorded reset as past,
  and `reset_wait_until` keeps only resets that are still ahead (`src/interaction.rs:500`). No
  reset is chosen, and `back_off` falls through to `Deferral::wait`
  (`src/engine/topology/run.rs:1813-1815`). That is the plain doubling back-off, capped at ten
  minutes (`src/interaction.rs:486-494`), which `master` runs.

**The cost.** A forward step of the host clock ends a reset wait early. That costs one deferral,
and then `master`'s back-off until the real reset passes.

**Why it is filed and not fixed.** The rule on clocks, `standards/03_standards_design_principles.md:33-35`,
has no `MUST`. It allows wall-clock time where a recorded timestamp needs wall-clock meaning, and a
reset is that kind of timestamp. Taking the clock out of the measure does not help either. Measured
by the sleeps alone, a suspended host oversleeps the reset, because the clock moves while the host
is suspended and the sleeps do not count that time
(`docs/internals/engine/topology/settle.md:342-347`). Ruling r32 files it.

## What the change that takes this up should do

- Keep the absolute target and the wall clock. The reset is a wall-clock instant, and only the wall
  clock can say it has passed.
- Never measure the wait by the sleeps alone, or by `Instant` alone. The sleeps alone oversleep a
  suspended host. An `Instant` cannot measure a wall-clock target, because the same standard says
  `Instant` and `SystemTime` are never compared or converted.
- Decide whether to announce the case where the clock runs ahead of the sleeps by more than one
  slice. A forward step and a suspended host look the same from inside the wait: both put the clock
  ahead of the sleeps. An announcement therefore cannot say which one happened. It can only say the
  wait was cut short by the clock.
- Decide whether `standards/03` should name this case: a wait toward a wall-clock target, where
  the target needs wall-clock meaning and the elapsed time is measured against it.
