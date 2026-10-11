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
