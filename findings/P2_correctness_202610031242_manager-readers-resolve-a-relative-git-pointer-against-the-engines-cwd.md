---
id: PR329-MANAGER-READERS-RESOLVE-A-RELATIVE-GIT-POINTER-AGAINST-THE-ENGINES-WORKING-DIRECTORY
severity: P2
disposition: deferred
category: correctness
pr: 329
reviewed_sha: 8df424364809c99fe1b8098ac71918815133d8e7
location: src/workspace_manager.rs:5096
provenance: pre_existing
first_bad:
guard: the change that implements closure 1's resolution rule (#329's record §8.4, item 1: a relative `gitdir:` line resolved against the checkout), applied to every existing reader of a checkout's `.git` pointer as well
---

## Failure sequence

Reasoned; found by design review round 9's gitenv lens (FUB-D9-RELPOINTER, `~/orch-pr11/reviews/review-329-d9-triage.md`).
Lines are at `8df42436`.

    a checkout's `.git` holds a relative `gitdir:` line, as Git 2.48 and later write it under
    `worktree.useRelativePaths` (2.50, 2.55)
    -> `worktree_git_dir` (src/workspace_manager.rs:5081-5096) returns that pointer as a bare path
    -> it is resolved against the engine's working directory, not the checkout
    -> `administrative_residue_at` (called at :2785) looks for an `index.lock` in the wrong place, so
       `quiescence(AtBase)` can pass over a real one; and `clear_pick_state` (:4287-4288) addresses the
       wrong `MERGE_MSG` and `AUTO_MERGE`

**What narrows it already.** Follow-up C's `-c worktree.useRelativePaths=false` on every manager command (#330's
record §5.3) makes the engine's own adds write absolute pointers. A pointer written otherwise, for example by the
user's `git worktree repair` with the setting on, still reaches these readers.

Filed at PR #329's implementation round as a record only (the decision appendix §10.2,
`~/orch-pr11/owner-package/DECISION-APPENDIX.md`); it decides nothing, and that round changes no reader of a
checkout's `.git` pointer.

## What the change that takes this up should do

Resolve a relative `gitdir:` line against the checkout that holds it, in every reader of a checkout's `.git` pointer
(`worktree_git_dir` and its callers), as #329's record §8.4 item 1 states the rule for closure 1's check, with a
witness per reader: a checkout whose pointer is relative, an `index.lock` and pick state in its administrative
directory, and the reader finding them from an engine whose working directory is elsewhere.
