---
id: PR380-RESET-LOST-BETWEEN-SETTLEMENT-AND-SIGNAL
severity: P2
disposition: deferred
category: crash-consistency
pr: 380
reviewed_sha: 265abbd2aeabc437337b91dcef7e0c5d8de69700
location: src/engine/coordinator.rs:827
provenance: introduced_by_feature
first_bad: ae2097a081dd14256691d801e9d2d3c5c2a11f16
guard: the next change to the legacy coordinator's rate-limited appends (ruling r41 step 5, 2026-11-22; the earlier guard, capacity stage three and no later than stage four, lapsed when stage four merged)
---

# A known reset is lost between its settlement and its signal

## Failure sequence

Round ten's frontier review found this by reading the code
(`/home/ubuntu/selfhost-resetat-20261113/evidence/review-frontier-265abbd.md`). It was not run, and
no test reproduces it. Line numbers are at `265abbd2`.

The rate-limited settlement and the signal that carries its reset are two appends, and the reset is
only in the second:

- `attempt_finished` is emitted at `src/engine/coordinator.rs:827` and synced. Its failure record
  (`FailureRecord`, `src/events/mod.rs:392-398`) has `kind`, `origin`, `reason` and `detail`, and no
  field for a reset. The limit text the reset is parsed from is `AttemptFailure`'s `limit_message`,
  held in memory only (`src/ladder.rs:44`).
- Next the parking question is written (`src/engine/coordinator.rs:857-868`), and an error there
  returns at once. Only after it is `pool_exhausted`, which carries `reset_at`, appended
  (`:890-892`, through `record_pool_exhausted`). Each append syncs on its own
  (`src/events/log.rs:503`), so the log can hold the first without the second. With
  `max_defers = 0` an outage asks a human at once (`src/ladder.rs:180-185`), so the settlement
  carries a parking question.
- On resume the fold marks the pool exhausted from the settlement alone, with no reset
  (`src/capacity.rs:236`, `exhausted.entry(pool.clone()).or_insert(None)`), and the coordinator
  starts from that fold (`src/engine/resume.rs:742`, `:774`).

Two sequences reach it:

1. **A crash.** Claude Code reports a reset on stderr → `attempt_finished` becomes durable → the
   process stops before `pool_exhausted` is appended → resume rebuilds the exhausted pool with
   `reset_at` null → the deferred retry succeeds and retires the pool → no event ever records the
   known reset.
2. **No crash.** With `max_defers = 0`, a recoverable failure writing the question returns before
   `pool_exhausted` is appended, with the same loss.

A later refusal on the same pool that names a reset writes the signal again
(`src/engine/coordinator.rs:1076-1092`: a recorded `None` does not equal a named reset, so it is not
`already_recorded`). So the reset is lost for good only when the pool serves first — that is, for an
outage that is already over. The run goes on correctly; what is lost is the record of when the
pool was due back.

## Whose it is, and why P2

- **The order of the two appends is the base's.** `origin/master`'s `src/engine/coordinator.rs`
  already appends the settlement, then the question, then `pool_exhausted`, and `origin/master`'s
  `docs/internals/capacity.md` already accepts a crash between them, in the paragraph beginning
  "`pool_exhausted` remains useful detail": the settlement is the durable source of the signal, so
  replay still knows which subscription refused. On the base, losing the signal lost nothing the
  settlement lacked.
- **The reset is this pull request's.** `ae2097a0` added the reset to the signal, and since
  `2e8a0e7a` (`2e8a0e7aa6bbc776f6eabf3d991fe9bd6a814aa5`) its text has been held in memory only. So
  the window the base accepted now loses something: the body and design say each outage records
  `pool_exhausted` with its reset and that resume rebuilds it, and in this window neither holds.
- **Narrow reach.** Fresh runs write schema 4, where this recorder does not run; it runs only when
  a schema-3 run resumes (ruling r6). Nothing routes on `reset_at` yet
  (`design/13_design_capacity_engine.md:15`: "Nothing yet schedules on the reset"), so the loss
  degrades `validate`, `upstroke capacity` and a resume's `capacity_snapshot`, not a scheduling
  decision. That keeps it below P1. It stays P2 because the stage that builds the retry-at-reset
  timer will rely on the reset being durable once the outage is recorded.

## What the change that takes this up should do

Either:

- make the settlement carry the parsed reset, and have the fold mark the pool with it, so the
  first durable append holds everything the signal does; or
- correct `design/13` to say a reset is durable only once its signal is.

Either way, append `pool_exhausted` before the question is written, so a failure writing the
question cannot drop the signal.

**Witness.** A run on the legacy writer, killed once the rate-limited settlement is durable (as
`kill_once_the_parking_settlement_is_durable` in `src/engine/tests.rs` does) and then resumed onto
a success, shows the reset in the log. It fails on `265abbd2`.
