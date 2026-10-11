---
id: PR299-HOSTED-QUEUE-DOES-NOT-PROVE-GUEST-BEHAVIOUR
severity: P2
disposition: accepted-risk
category: compatibility
pr: 299
reviewed_sha: 870deb3e53c7cb38ac4db51db2d8fce054403060
location: .github/workflows/ci.yml:140
provenance: introduced_by_feature
first_bad:
guard: a revert of #299, or the retirement of the pull-request guest so both Windows lanes run on one machine
---

## Failure sequence

A pull request adds a Windows test whose outcome depends on the runner -- what
`RUNNER_ENVIRONMENT` reports, what the image carries, the machine's filesystem or services ->
its pull-request run on the guest lane passes, and its merge-queue entry on `windows-latest`
passes -> it lands, and the push to `master`, which takes the guest lane, is the first run to
fail. Before #299 the queue's own guest would have rejected the entry.

## Why it stays

It is the price of the move the owner authorised: the queue lane leaves the guest so that
`winguest-ci-q` can be retired, and no check closes a gap between two machines while both
lanes exist. The compiler pin and the witness's compiler questions remove the compiler a lane
selects by accident, and a compiler split between Cargo and the fixtures fails the suite's
four rlib-linked fixtures rather than passing (`WINDOWS_TEST_WITNESS` notes,
`docs/internals/effects/tests/ci_model.md`); what remains is the machines' runtime
differences. Stated in the workflow comment, the `TEST_WINDOWS_JOB` notes and #299's Risk and
rollback. Frontier review 2 of #299, finding 2
(https://github.com/sourcemaps/upstroke/pull/299#issuecomment-5691137663).

## What the change that takes this up should do

Nothing short of one machine for both lanes closes it: revert #299 (the queue routes to the
`winguest-queue` guest again), or move the pull-request lane to `windows-latest` too and
retire the guest. Until then, a red push-to-`master` run on the guest lane after a green queue
entry is this finding, and the fix is to the test, not the queue.
