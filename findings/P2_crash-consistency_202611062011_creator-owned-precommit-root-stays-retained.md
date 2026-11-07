---
id: PR12-INV8-PRECOMMIT-EXECUTION-ROOT-UNRECLAIMED
severity: P2
disposition: deferred
category: crash-consistency
pr: 353
reviewed_sha: b4b16580b317896904c5c06983466c3f67e05276
location: src/engine/topology/startup.rs:495
provenance: fix_regression
first_bad: 60a04239; REGULAR13-R18-PRECOMMIT-CONVERGENCE
guard: documented deferral with REGULAR13-R18-PRECOMMIT-CONVERGENCE: startup.md fn another_contexts_root(; history: BODY19 2110b921
---

## Failure sequence

same killed-precommit sequence (invariants lens) -> finding the root keeps the husk -> the creator-owned root and halves stay retained, inspectable, not reclaimed

Original review: `b4b16580b317896904c5c06983466c3f67e05276 / src/engine/topology/startup.rs:495` at `b4b16580` (BODY19 `2110b921`, mapping `3047f80f`). Current-candidate reassessment location: `b4b16580b317896904c5c06983466c3f67e05276 / src/engine/topology/startup.rs:495` (ROOT mapping direction `5a631538`).

## What the change that takes this up should do

Same convergence obligation as REGULAR13-R18-PRECOMMIT-CONVERGENCE (correlated): a later write command reclaims the creator-owned root and halves once ownership and absence of commitment are proven.

Accepted PR-fit deferral for the owner-selected repair14 selective candidate (`113582b1`); full Gate7 obligations remain owed after fit merge. Correlations: 60a04239; REGULAR13-R18-PRECOMMIT-CONVERGENCE.
