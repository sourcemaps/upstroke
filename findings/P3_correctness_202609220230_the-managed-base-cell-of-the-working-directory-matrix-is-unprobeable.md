---
id: PR313-WORKING-DIRECTORY-MATRIX-CANNOT-PROBE-THE-MANAGED-BASE
severity: P3
disposition: deferred
category: correctness
pr: 313
reviewed_sha: 604d8139749c907e9d0071e4a78438d3cc178eb4
location: src/workspace_manager/tests.rs:3230
provenance: introduced_by_feature
first_bad: 604d8139749c907e9d0071e4a78438d3cc178eb4
guard: the same sweep as `PR313-GIT-DISCOVERY-REDIRECT-INSIDE-THE-STATED-BOUNDARY`
---

## Failure sequence

`no_primitive_acts_through_a_git_working_directory_the_table_does_not_name` probes a candidate working directory by leaving it without a repository -> a linked worktree's git dir **is** `<base>/.git/worktrees/<name>`, so renaming `<base>/.git` aside disables the slot checkouts' Git children too -> the managed-base cell is skipped for the five primitives whose table names a slot checkout, and a Git child added in the managed base by `candidate_stage`, `candidate_write_tree`, `proposal_cherry_pick`, `repair_materialize` or `verify_worktree` is an omission the matrix cannot see; the five skips are counted and pinned, so the gap is visible in the test's own numbers rather than silent

## What the change that takes this up should do

Find a probe that separates "a child ran in the managed base" from "a child ran in a
checkout of the managed base". Two shapes were considered and neither works as written:
making `<base>` itself unreadable breaks the slot's git dir through the same ancestor, and
replacing `<base>/.git` with a clone's leaves the slots' absolute `gitdir:` pointers naming
a path the clone does not have. A cwd-sensitive probe — something a Git child reads because
of *where* it runs rather than which repository it resolves — is the shape to look for; a
recorded argv per Git child, which the funnel does not have today, would settle it outright.
