---
id: PR320-R7-PRODUCTION-LEASE-PROBE-RETRIES-AN-INTERRUPTED-OPEN
severity: P3
disposition: deferred
category: liveness
pr: 320
reviewed_sha: 23021ff6a3f13c5833baffc5c6b714ff7cb9c95d
location: src/rundir.rs:2492
provenance: pre_existing
first_bad:
guard: the change that takes up production's cleanup-lease probe; carried beside `PR281-CLEANUP-LEASE-HOLD-OUTLIVED-AND-ITS-UNREADABLE-TWIN`, whose production inheritance is deferred too
---

## Failure sequence

Production's `rundir::observe_cleanup_hold` answers whether a run's cleanup lease is held through
`cleanup::is_held`, which opens the lease file with `File::options().read(true).write(true).open`,
then makes one `flock(LOCK_EX | LOCK_NB)` and one `flock(LOCK_UN)`. std's `open` makes an
interrupted `open(2)` again inside itself (`cvt_r`), so an open that answers `EINTR` every time --
a policy that refuses it, or a filesystem whose open waits interruptibly and is interrupted at
every attempt -- never returns from the probe.

The recovery tests' lease wait (`wait_for_cleanup_hold_release_observing`, reached by the three
resume trunks through `await_previous_incarnations_release`) and its rundir twin
(`lease_released_within`) call that probe once per turn, and their bound is looked at between
turns. Pull request #320 made every rest between two of those turns one attempt
(`PR320-R6-REG-001`), so a refused rest no longer holds a wait past its bound; a probe that never
returns still would, and the wait's bound cannot see inside it: it is production's one call, which
the fixture must make as production makes it, since what the wait exists to observe is
production's own answer.

## Why it is deferred

The probe is production code this pull request does not change (its `src/rundir.rs` change is
documentation only), and making its open one attempt is a production change: its callers would
have to decide what an interrupted probe means -- held, as every other open error answers today,
fail-closed, or not observed -- which is a design decision for the run lease, not a test-fixture
repair. No suite installs a policy that reaches the probe, and no witness has held a probe open;
this is the named limit of the waits' bound, recorded so that it is not only prose.

## What the change that takes this up should do

Open the lease once (`open(2)` with `O_CLOEXEC`, no retry), and answer an interrupted open as the
probe's other open errors are answered, deliberately; then hold it with a regression that refuses
the open with `EINTR` on the probing thread, as the rundir suite's `Refusal::NondirectoryOpens`
does, through one of the lease waits.
