---
id: PR446-CANCELLATION-RECORDER-WIRING
severity: P3
disposition: deferred
category: correctness
pr: 446
reviewed_sha: af0b64b737c48c1b1881fc498ef3b930f2a7e5e5
location: src/runner/container/tests.rs:9832
provenance: introduced_by_feature
first_bad:
guard: the test-only pull request, on its own claim branch, that makes a test fail when exec_within hands its question any stop but the recording one, changing no production line
---

# No test asks a cancelled question through `exec_within`

## Failure sequence

The sequence is the frontier reviewer's, from #446's first pair of passes, on `af0b64b7`:

    exec_within hands its question the raw cancellation predicate in place of the recording stop
    -> the funnel ends the question on the cancellation, but the record still says the stop
       never fired
    -> the cancelled question's failure names "a signal", while every test of the change passes

The mutation witness is the reviewer's. A temporary Linux test cancelled a live
`DockerCli::observe` after its `docker` stub announced its start. At `af0b64b7` the failure named
the cancellation. With only `exec_within` changed to hand the question
`cancellation.is_cancelled()` directly, both of the change's tests stayed green, and the
observation failed with "the end it met was a signal, after 53 ms".

The change's two tests call `answer_within` and `answer_recording_stop` directly
(`src/runner/container/tests.rs:9741-9875`), so no edit confined to `exec_within`
(`src/runner/container.rs:1098-1125`) changes their outcome.

#446's second pair of passes, on `d302e2c9`, found a second substitution of the same kind:
`exec_within` handing the funnel the raw predicate and asking the recording stop only after the
question returns. All three of the pull request's tests at `d302e2c9` passed under it. This is the
frontier pass's finding `PR446-LATE-STOP-SAMPLE-PASSES-OBSERVE`, reconciled into this one.

## What round one tried

Round one of #446's repairs (ruling r109) added a Unix test that asked a question through
`observe`, in an isolated child of the test binary:

- the child's search path was the stub's directory alone;
- the child wrote its stub in a process of its own;
- it cancelled its invocation when the stub's `ps` marked that it was asked;
- it required the detail to name its invocation's cancellation.

The commit is `180a7df1`, reviewed on `d302e2c9`. The reviewer's mutation failed it. The second
pair of passes found two defects in it, and ruling r109 step 8 withdrew it. With it withdrawn the
finding is open again, filed under the owner's reproduced-findings exception of
2026-11-27T08:41:42Z.

Each defect is the reviewers', marked as their reproduction:

- `PR446-STUB-PATH-CAN-SELECT-HOST-DOCKER`. The parent set the search path to the temporary `bin`
  directory itself, and a colon in that path splits it, so one entry can hold the machine's own
  `docker`. The frontier pass's pre-spawn check found three entries, one of them holding
  `/usr/bin/docker`. The regression lens, with `TMPDIR` ending in `scratch:case`, saw the test
  fail after 60.07 s with its stub never asked (reproduced by both passes).
- `PR446-MARK-TIMEOUT-CAN-PASS`. The child cancelled after its 60-second wait for the mark whether
  or not the mark had appeared. With `observe` paused after its pre-check, the cancellation came
  first and the test still passed, in 60.10 s (the frontier pass, reproduced in a copy).

## What the change that takes this up should do

It adds a test that fails when `exec_within` hands its question any stop but the recording one,
and it changes no production line.

Ruling r109 names six needs. They are those that #444's second pair of passes found any check of
this kind needs, recorded in `PR444-UNARMED-TEST-MISSES-DOCKER-INVOCATIONS`:

1. It passes the record's path to the stub as data, not interpolated into shell text.
2. A stub that cannot run fails the test, and no later `docker` on the search path stays reachable.
3. It joins no temporary path into the search path. Round one's test set its search path to a
   temporary path alone, and a colon split it (`PR446-STUB-PATH-CAN-SELECT-HOST-DOCKER`).
4. It writes the stub inside the bounded child (`PR368-WRITER-HELPER-UNBOUNDED-CALLERS`).
5. It relies on nothing another test can move.
6. It says what it shows on Windows.

Round one's test showed two more needs. The new test's cancellation is shown to follow the start
of the question's process, not assumed to (`PR446-MARK-TIMEOUT-CAN-PASS`). And an `exec_within`
that asks the recording stop only after the question returns fails it, as one that hands the raw
predicate does (`PR446-LATE-STOP-SAMPLE-PASSES-OBSERVE`).
