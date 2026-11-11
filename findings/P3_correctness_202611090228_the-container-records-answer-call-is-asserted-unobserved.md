---
id: G7-M2-REGRESSION-002-ANSWER-UNOBSERVED
severity: P3
disposition: deferred
category: correctness
pr: TBD
reviewed_sha: 7e80d921bbe98f5babd773bfb7d5236f685e770d
location: src/engine/tests.rs:20387
provenance: introduced_by_feature
first_bad: 7f70fde459b7a37df2b439959f18b3b5b7d054c8
guard: the next change to `container_acceptance_resumed` in src/engine/tests.rs
---

## Failure sequence

`container_acceptance_resumed` drops its first arming guard when `engine::run_with` returns, and
arms again only around `engine::resume_with`. `answer::answer` (src/engine/tests.rs:20387) runs
unarmed in between.

    answer::answer runs with the seam unarmed
    -> a container runtime it constructed would be a plain DockerCli that no journal sees
    -> container_acceptance_run asserts that answer::answer constructed no container runtime by
       comparing the journal's runtime count before and after it, and that passes whatever the
       answer did
    -> the record's scope.observer text says the seam was armed for each public call, which
       overstates what was observed of the answer

On this head `answer::answer` and its open path construct no container runtime, by source
inspection, so no acceptance container is missing from any record. What is missing is the
observation that would show it. Found in the regression review of 7e80d921; no reproducer was
run. Until this is taken up, the record's statement about the answer call is source-derived, and
a native verdict or pull request that cites it should say so.

## What the change that takes this up should do

Arm the same journal around `answer::answer` before its runtime counts are compared, so the
zero-runtime assertion observes the call it names. Or narrow the emitted `scope.observer` text,
and the notes in `docs/internals/engine/tests.md`, to say that the answer's absence of a runtime
rests on source inspection rather than on the journal.
