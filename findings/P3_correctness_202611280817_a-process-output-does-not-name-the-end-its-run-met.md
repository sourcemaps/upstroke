---
id: PROCESS-OUTPUT-NAMES-NO-END
severity: P3
disposition: deferred
category: correctness
pr: TBD
reviewed_sha: 6513c22d24f8404b6877a6da578deaff19aa7224
location: src/agent/proc.rs:441
provenance: pre_existing
first_bad:
guard: the pull request that makes the funnel report the end a run met, the mechanical edit of its literals landed first
---

# A process output does not name the end its run met

## Failure sequence

Read from the code at `6513c22d`, not run.

    a caller's stop predicate ends a run of the process funnel, or a signal ends a run after
       16 MiB of output
    -> the funnel returns the run's output, which carries `timed_out` and `output_limited`
    -> nothing in the output names the stop, and the signal's exit reads like the funnel's own
       kill at its output limit

The process funnel's output (`src/agent/proc.rs:441-448`) carries `timed_out` and `output_limited`
but not the stop end. The funnel records that end in `stopped`, a local (`src/agent/proc.rs:318`) it
reads only to choose the drain grace (`:416`).

The output's `output_limited` does not say which end set it. The drain keeps reading past the output
limit (`src/agent/proc/drain.rs:218-224`), the wait loop sees an exit before it checks the limit
(`src/agent/proc.rs:321-333`), and collection sets the flag (`:439`). An exit a signal caused after
16 MiB of output therefore reads like the funnel's own kill at its limit. That reading is inferred,
not seen.

So the container funnel records the stop on the caller's side, in the stop predicate its
invocation-bound questions hand the process funnel, and names the limit as
"the output limit or a signal" (ruling r105, this pull request).

## What the change that takes this up should do

It makes the process funnel report the end a run met, after a separate change has added the field
that carries it to every `ProcessOutput` literal (35 in 20 files at `6513c22d`), and it changes no
bound, kill or classification.
