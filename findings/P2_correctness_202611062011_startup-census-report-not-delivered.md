---
id: PR12-INV8-STARTUP-CENSUS-REPORT-DROPPED
severity: P2
disposition: deferred
category: correctness
pr: 353
reviewed_sha: b4b16580b317896904c5c06983466c3f67e05276
location: src/engine/topology/entry.rs:349
provenance: introduced_by_feature
first_bad: fffa73fa
guard: documented deferral: census entry describe() names root-retained and locator-unreadable; a_husk_whose_private_locator_cannot_be_inspected_is_retained_and_reported_beside_a_missing_one; broad delivery deferred; history: BODY19 2110b921
---

## Failure sequence

a retained or unreclaimable husk is in the startup census -> a public fresh run or resume does not deliver the census report -> retained state remains inspectable through the documented status/read-only marker, owner-record and root inspection; automatic operator notification is deferred

Original review: `b4b16580b317896904c5c06983466c3f67e05276 / src/engine/topology/entry.rs:349` at `b4b16580` (BODY19 `2110b921`, mapping `3047f80f`). Current-candidate reassessment location: `b4b16580b317896904c5c06983466c3f67e05276 / src/engine/topology/entry.rs:349` (ROOT mapping direction `5a631538`).

## What the change that takes this up should do

Deliver the startup census report (retained/unreclaimable husks with locator and reason) through the public fresh-run and resume paths, beyond the census entry describe() and status inspection kept by this candidate.

Accepted PR-fit deferral for the owner-selected repair14 selective candidate (`113582b1`); full Gate7 obligations remain owed after fit merge. Correlations: fffa73fa.
