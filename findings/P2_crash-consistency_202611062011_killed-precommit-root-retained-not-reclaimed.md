---
id: REGULAR13-R18-PRECOMMIT-CONVERGENCE
severity: P2
disposition: deferred
category: crash-consistency
pr: 353
reviewed_sha: b4b16580b317896904c5c06983466c3f67e05276
location: src/engine/topology/startup.rs:494
provenance: fix_regression
first_bad: 60a04239; PR12-INV7-PREPUBLICATION-EXECUTION-ROOT-ORPHAN
guard: documented deferral: startup.md fn another_contexts_root( (location and read-only inspection); retention guards a_proven_husk_whose_root_another_repository_context_made_keeps_both_halves_and_the_root; a_target_absent_husk_whose_root_another_repository_context_made_keeps_its_marker; convergence owed to full Gate7; history: BODY19 2110b921
---

## Failure sequence

a creator publishes its halves, claims the execution root at P3c and dies in pre-flight before committing -> every later census proves the halves, sees the root and keeps both -> safe retention, no automatic reclamation in this candidate

Original review: `b4b16580b317896904c5c06983466c3f67e05276 / src/engine/topology/startup.rs:494` at `b4b16580` (BODY19 `2110b921`, mapping `3047f80f`). Current-candidate reassessment location: `b4b16580b317896904c5c06983466c3f67e05276 / src/engine/topology/startup.rs:494` (ROOT mapping direction `5a631538`).

## What the change that takes this up should do

Converge a proven, never-committed precommit husk and its creator-owned execution root in a later census without manual removal, preserving private-before-public order, proof/token barriers and live/committed/foreign retention (full canonical Gate7 R18 convergence).

Accepted PR-fit deferral for the owner-selected repair14 selective candidate (`113582b1`); full Gate7 obligations remain owed after fit merge. Correlations: 60a04239; PR12-INV7-PREPUBLICATION-EXECUTION-ROOT-ORPHAN.
