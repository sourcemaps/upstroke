---
id: PR12-R15-NOTHINGBOUND-CAUSE-NOTES
severity: P3
disposition: deferred
category: docs-contract
pr: 353
reviewed_sha: 49729333c3d87dc346cc00e42b38ccf488893951
location: docs/internals/engine/topology/create/tests.md:1704
provenance: fix_regression
first_bad: 60a04239a4677344445478043da04f519760660a
guard: documented: NotFound-only absence now enforced in ownership.rs (Group B); note wording not corrected in this candidate; history: BODY19 2110b921
---

## Failure sequence

NothingBound in creator cleanup -> the note still says only an outside actor reaches the arm -> absent binding is not proof of outside deletion (historical review at 49729333 of the repair15 wording)

Original review: `49729333c3d87dc346cc00e42b38ccf488893951 / docs/internals/engine/topology/create/tests.md:1709` at `49729333` (BODY19 `2110b921`, mapping `3047f80f`). Current-candidate reassessment location: `47fad28c858fd30b0f0d4c2b6bce7adc995dc599 / docs/internals/engine/topology/create/tests.md:1704` (ROOT mapping direction `5a631538`).

## What the change that takes this up should do

Correct the create-tests note so NothingBound describes absent binding, not proof that an outside actor deleted the private half (NotFound-only absence is already enforced in ownership.rs by Group B).

Accepted PR-fit deferral for the owner-selected repair14 selective candidate (`113582b1`); full Gate7 obligations remain owed after fit merge. Correlations: 60a04239a4677344445478043da04f519760660a.
