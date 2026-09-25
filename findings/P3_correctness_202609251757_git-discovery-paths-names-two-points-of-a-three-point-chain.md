---
id: PR313-GIT-DISCOVERY-PATHS-NAMES-TWO-POINTS-OF-THREE
severity: P3
disposition: deferred
category: correctness
pr: 313
reviewed_sha: f00e342b18a13348c7bf103e92bf611af964f975
location: src/workspace_manager.rs:1818
provenance: pre_existing   # found by #313's adversarial lens (claude-opus-5-5, owner waiver 2026-09-25) at f00e342b
first_bad:
guard: the change that takes up PR313-GIT-DISCOVERY-REDIRECT-INSIDE-THE-STATED-BOUNDARY, which would pin the paths this table names
---

## Failure sequence

`git_discovery_paths` documents itself as *"Two paths per working directory, because two is what Git resolves
and what a substitution can be planted at"*, and claims *"a test that plants a link at each path this returns
drives the whole class"*. **A linked worktree's chain has three points, not two.**

1. A slot checkout's Git child resolves `<slot>/.git`.
2. That names the admin dir `<common>/worktrees/<id>`, which holds `commondir`, `index` and `HEAD`.
3. The admin dir's `commondir` names the common git dir.
4. `SlotCheckout` returns **the first and the last** of those three points.
5. So a substitution at the **admin dir**, or a rewrite of its `commondir`, re-aims the slot's children
   without touching either named path. For that point the "whole class" claim does not hold.

## Effect on the runner answers: none

- Under the **host** runner, the same writer can reach that point anyway.
- Under the **container** runner, the admin dir is not mounted; only its `HEAD` and index are copied into the
  view.
- It matters for the change that takes up `PR313-GIT-DISCOVERY-REDIRECT-INSIDE-THE-STATED-BOUNDARY`, which
  would pin the paths this table names — a table that names two of three points would pin an incomplete set.

## Reachability, against the owner's rule of 2026-09-11

**Neither limb holds.**

## What the change that takes this up should do

Either name `<common>/worktrees/<id>` for `SlotCheckout`, making it **three** paths rather than two, or narrow
the sentence to *"the two points a substitution of the whole chain can be planted at"*. The first is the better
fix if the table is going to be pinned.

## Provenance

Found by #313's **adversarial** lens at `f00e342b`, reasoned. Filed under the owner's ruling of
2026-09-25T17:51Z that #313 merges under the 09-11 merge bar and the wording corrections live in the findings.
