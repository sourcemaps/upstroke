---
id: DOCKER-QUESTION-UNFINISHED-NAMES-NO-END-AND-NO-TIME
severity: P3
disposition: deferred
category: correctness
pr: TBD
reviewed_sha: b7482c2c11ec38f132d3bd204eaf08ca043e2b99
location: src/runner/container.rs:1693
provenance: pre_existing
first_bad:
guard: the instrument pull request, on its own claim branch, that makes an unanswered docker question name the end it met and the time it ran, changing no bound, refusal or classification
---

# An unanswered docker question names no end and no time

## Failure sequence

Read from the code at `b7482c2c`, not run.

    a question to the container runtime ends with no answer: the bound it was asked under passed,
       its invocation was cancelled, or a signal ended it
    -> its caller returns `unfinished(op, args)`
    -> the error lists all three ends, whichever one the question met, and gives no time

`unfinished` (`src/runner/container.rs:1693-1702`) words every unanswered question to the runtime
the same way. Both its calls, at `:1009` in `exec_streams` and at `:1135` in `answer_within`, give
the one detail:

> `docker <args>` ended before it answered: the bound it was asked under passed, its invocation
> was cancelled, or a signal ended it

It gives no time. Each caller reads the question's process output, which carries `timed_out` and
`duration` (`ProcessOutput`, `src/agent/proc.rs:36-43`), and passes `unfinished` only the operation
and the arguments. A stopped question is not recorded in that output. The funnel sets `stopped`
(`src/agent/proc.rs:318`) when a question's stop check ends it, which for `exec_within` is its
invocation's cancellation, and reads it only to choose the drain grace (`:416`).

An output-limited kill is reported as "ended before it answered" too. The kill leaves no exit code
(`src/agent/proc.rs:342` and `:393`), and in `answer_within` the check at
`src/runner/container.rs:1134`, `output.timed_out || output.code.is_none()`, comes before the
output-limit check at `:1137`. `exec_streams` excludes a limited answer from that check (`:1008`).

## What the change that takes this up should do

It makes an unanswered docker question name the end it met and the time it ran, and it changes no
bound, refusal or classification.
