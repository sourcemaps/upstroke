---
id: PR299-GUEST-LANE-DOWNLOADS-THE-SKIPPED-INSTALL
severity: P2
disposition: accepted-risk
category: liveness
pr: 299
reviewed_sha: 3c576f08f062aa8963421a51b76c402f6f5e44f0
location: .github/workflows/ci.yml:137
provenance: introduced_by_feature
first_bad:
guard: the owner, at the next change to the `test-windows` job's shape or the retirement of the pull-request guest
---

## Failure sequence

A pull request's `test-windows` job starts on the guest lane -> the runner resolves and
downloads every `uses:` archive during "Set up job", before it reads any step's `if:`, so the
`dtolnay/rust-toolchain` archive of the hosted lane's install is downloaded though the step is
skipped (the guest run at `3c576f08`, run 35044255513, shows both downloads) -> GitHub fails to
serve that archive -> `test-windows` fails before its first step, and `merge-gate` fails with it.
Before #299 the lane depended on one archive, `actions/checkout`; it now depends on two.

## Why it stays

It is an availability dependency of the kind the lane already had, and the price of routing
both lanes through one job. The alternative, a second job id for the hosted lane, needs a
job-level `if:` and an aggregate that accepts `skipped` for one of the pair, which is the
false green `merge-gate` exists to refuse. The workflow comment at the install step and the
`LANE_STEP_FIELDS` notes (`docs/internals/effects/tests/ci_model.md`) state the dependency and
its cause. Frontier review 1 of #299, finding 1
(https://github.com/sourcemaps/upstroke/pull/299#issuecomment-5690834133), observed the
download in a run log; nothing has reproduced the failure.

## What the change that takes this up should do

Remove the dependency without a second job: either the guest lane stops being a lane of this
job (the pull-request guest retired, both lanes hosted), or the hosted lane's compiler is
selected by a `run:` step (`rustup toolchain install 1.97.1 --component clippy` and
`rustup default 1.97.1`) instead of a `uses:` action, which the oracle's `LANE_STEP_FIELDS`
would then have to admit in place of the action. Either is a workflow and CI-contract change,
the owner's to merge.

## 2026-11-12: #375 makes the second-job shape

#375 is the change this row's guard names, and it makes the shape "Why it stays" rejected: a
second job id for the hosted route, `test-windows-hosted` (`test (windows-latest)`), beside the
guest's `test-windows`, each with a job-level `if:` on the answer of a new job,
`pick-windows-home`, and an aggregate that accepts `skipped` for one of the pair.

**How the exactly-one rule answers the objection.** The objection was that an aggregate
accepting `skipped` is the false green `merge-gate` exists to refuse. `merge-gate` does not
accept `skipped` for either job on its own: it collects both results and passes only on exactly
`success` beside `skipped`, either way round. Both skipped, both run, either failed or
cancelled, or the picker failed -- which skips both -- is red, so no run in which no Windows suite
ran can pass. The two conditions are pinned to one comparison of the picker's answer with its
two values, `guest` and `hosted`, so on a run whose picker succeeded exactly one job runs.
`the_aggregate_requires_exactly_one_windows_suite_job_and_every_other_gate` executes the
aggregate's script over every pair of the four results and every other gate failing; escape rows
`MUT-AGGREGATE-ACCEPTS-BOTH-WINDOWS-JOBS-SKIPPED` and `-RUN` are refused. What the rule does not
judge is whether the picker chose well: a run it sends to the twin is a real run of the suite on
another machine, which is `PR299-HOSTED-QUEUE-DOES-NOT-PROVE-GUEST-BEHAVIOUR`'s risk, not a false
green.

**What of this finding remains.** Its failure sequence has not been reachable since `d3434ace`
(2026-10-26) removed the hosted lane's install step from `test-windows`: the guest's job carries
one `uses:` archive, `actions/checkout`, the oracle refuses a toolchain install in it, and #375's
install step lives in the twin, a job the guest never sets up. The dependency of this row's kind
that #375 adds is the picker: the guest's job now `needs: pick-windows-home`, a GitHub-hosted job
that reads the Actions API, and when that job fails or is cancelled both Windows jobs are skipped
and `upstroke-ci` is red. The picker answers `guest` on any API error it sees, so only its own
failure, or a read that stalls to its 5-minute timeout (fix-check review of #375 at `873ff8c7`,
finding 4), reds a run that way -- red, never a false green. Whether this row closes, or carries
that dependency forward, is the owner's call; #375 records it here and deletes nothing.
