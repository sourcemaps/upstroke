---
id: PR446-NOTES-MISS-A-LIMIT-FLAG-SET-AFTER-THE-END
severity: P3
disposition: deferred
category: docs-contract
pr: 446
reviewed_sha: d302e2c9546441ac2b134340c3fd53b9ebb8d073
location: docs/internals/agent/proc.md:217
provenance: pre_existing
first_bad:
guard: the next change to the process funnel's notes or to the container funnel's note on QUESTION_BOUND
---

# Two notes miss a limit flag set after a question's end

## Failure sequence

This is the regression lens's third finding from #446's second pair of passes, on `d302e2c9`,
and the claim it names in #446's body is corrected there. The sequence is the lens's:

    a writer crosses the capture limit after the funnel has chosen the end of its run, its bound
       or its stop
    -> the funnel returns the output with `output_limited` set: with `timed_out` after the bound,
       and after a cancellation too
    -> two notes on master say otherwise

The lens reproduced both with a termination hook that let a writer cross the capture limit after
the funnel chose its end.

The two sentences are master's, each unchanged by #446:

- `docs/internals/agent/proc.md` lines 217 to 218, the process funnel's note on its stop: a
  stopped run "returns the output with neither `timed_out` nor `output_limited` set". The lens's
  probe returned a limited output after a cancellation.
- `docs/internals/runner/container.md` lines 1346 to 1348, the note on `QUESTION_BOUND`:
  "A question that reaches the bound is killed and reaped by the funnel and fails like any
  question that ended before it answered." But `exec_streams`, which asks `docker logs`, fails an
  answer that ended before it answered only when the answer is not limited, and otherwise returns
  it with the flag (`src/runner/container.rs` lines 1007 to 1018). So a `docker logs` whose bound
  ended it after its capture was cut comes back limited, not failed; the lens's probe returned
  such an output, with `timed_out` and `output_limited` both set.

The note under `unfinished` says the limit flag can also be set after the wait loop, when the
output is collected.

## What the change that takes this up should do

It corrects both sentences from the code, and changes no code. For the flag, the code is the
process funnel's: `src/agent/proc.rs` line 439, where collection sets it, and
`src/agent/proc/drain.rs` lines 218 to 224, where the drain reads past the limit. For
`docker logs` it is `exec_streams`.

The finding is recorded here and not corrected in #446, which surfaced it: by the owner's
direction of 2026-11-27T10:28:38Z, a finding already on master is recorded and not repaired in the
pull request that surfaced it.
