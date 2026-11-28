---
id: PR446-CANCELLATION-LIMIT-PRECEDENCE-UNTESTED
severity: P3
disposition: deferred
category: correctness
pr: 446
reviewed_sha: d302e2c9546441ac2b134340c3fd53b9ebb8d073
location: src/runner/container/tests.rs:9825
provenance: introduced_by_feature
first_bad:
guard: the test-only pull request, on its own claim branch, that makes a test fail when a question whose stop fired and whose output came back limited is named anything but its invocation's cancellation, changing no production line
---

# No test holds a cancellation ahead of a late limit flag

## Failure sequence

The sequence is the frontier reviewer's, from #446's second pair of passes, on `d302e2c9`:

    the wait loop sees no limit and acts on the cancellation
    -> a drain flags buffered excess output during collection
    -> the output carries both a fired stop and `output_limited`
    -> with their priority reversed, the cancellation is named "the output limit or a signal",
       and every test of the change passes

The mutation witness is the reviewer's. On the reviewed code, a control with both conditions
passed. With `end_and_time`'s limit branch moved above its stop branch, all three of the pull
request's tests at `d302e2c9` stayed green, and the control failed with
"the output limit or a signal, after 27 ms".

No test of the change holds a fired stop together with a limited output.
`an_unanswered_docker_question_names_its_cancellation_only_when_its_stop_fired` hands
`answer_recording_stop` only outputs that are not limited
(`src/runner/container/tests.rs` line 9825). The two limited outputs of
`an_unanswered_docker_question_names_the_end_it_met_and_the_time_it_ran` are read with no stop.
`end_and_time` reads the stop before the limit flag
(`src/runner/container.rs` lines 1747 to 1759), and the note under `unfinished` says why.

## What the change that takes this up should do

It adds a test that hands `answer_recording_stop` a question whose stop fires and whose output
comes back limited, with no exit code. The test requires the detail to name its invocation's
cancellation, so the mutation above fails it. It changes no production line.

The finding is filed under the owner's reproduced-findings exception of 2026-11-27T08:41:42Z,
because after #446's second pair of passes, its ruling, r109, allowed no further code.
