---
id: PR12-INV9-RECORDED-ROOT-CONTEXT-CONVERGENCE
severity: P2
disposition: deferred
category: crash-consistency
pr: 353
reviewed_sha: 5689c23c24b03be937c6c4a19387dc654c233f20
location: src/engine/topology/startup.rs:605
provenance: fix_regression
first_bad: 49729333; REGULAR13-R18-PRECOMMIT-CONVERGENCE
guard: documented deferral with REGULAR13-R18-PRECOMMIT-CONVERGENCE; a_proven_husk_whose_root_another_repository_context_made_keeps_both_halves_and_the_root; a_target_absent_husk_whose_root_another_repository_context_made_keeps_its_marker; history: BODY19 2110b921
---

## Failure sequence

a proven never-committed husk whose root another legal repository context made -> this head retains the husk, its marker and the root (no cross-context retirement) -> it does not converge automatically (historical review at 5689c23c)

Original review: `5689c23c24b03be937c6c4a19387dc654c233f20 / src/engine/topology/startup.rs:859` at `5689c23c` (BODY19 `2110b921`, mapping `3047f80f`). Current-candidate reassessment location: `47fad28c858fd30b0f0d4c2b6bce7adc995dc599 / src/engine/topology/startup.rs:605` (ROOT mapping direction `5a631538`).

## What the change that takes this up should do

Converge a proven never-committed husk whose root another legal repository context made, through an authenticated recorded-root mechanism, without restoring the creating context; correlated with the safe-retention deferral.

Accepted PR-fit deferral for the owner-selected repair14 selective candidate (`113582b1`); full Gate7 obligations remain owed after fit merge. Correlations: 49729333; REGULAR13-R18-PRECOMMIT-CONVERGENCE.
