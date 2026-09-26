---
id: PR326-MANAGER-GIT-CHILDREN-CARRY-THE-VARIABLE-ALONE
severity: P2
disposition: deferred
category: correctness
pr: 326
reviewed_sha: 915c0646c1e502c74df596586d6232ce2df7a6fe
location: src/workspace_manager.rs:4915
provenance: pre_existing   # found in #326 round 3, measuring the same gap in src/workspace.rs's git_command
first_bad: 8a2b0bde66c87623874ba4a0a58364e8245828b8
guard: pass -c core.useReplaceRefs=false beside the pair in WorkspaceManager::command and read_only_git, and pin it in the manager's census
---

## Failure sequence

1. The machine runs Git 2.40 or 2.41, inside the "Git 2.40+" `README.md` requires. An explicit
   `core.useReplaceRefs = true` is set in a system, global or repository configuration file, or in command-line
   configuration the process inherits.
2. `WorkspaceManager::command` (`src/workspace_manager.rs:4915`) and `read_only_git` (`:5358`) set
   `GIT_NO_REPLACE_OBJECTS=1`, and nothing else about replacements.
3. Git 2.40 and 2.41 read `core.useReplaceRefs` in their default configuration as
   `read_replace_refs = git_config_bool(...)`, which overwrites what the variable set. Git 2.42 made the variable
   final.
4. So the manager's funnel primitives and reads read the replaced graph. Its builder's own comment lists them:
   `worktree add`, `add`, `write-tree`, `cherry-pick`, `commit-tree`, `update-ref`, `rev-parse` and `diff` among
   them. An exact snapshot is then materialised and judged against the objects `git replace` points at, which
   `design/15`'s "What an exact snapshot is exact against" rules out.

**Measured with Git itself** on 2.40.0 and 2.41.0, with a `true` at each of those places:

- a child carrying the variable alone read the replaced graph;
- with `-c core.useReplaceRefs=false` as well, it read the recorded one, on 2.40.0, 2.41.0, 2.42.0 and 2.43.0.

This was not driven through the manager's code. The same Git behaviour made `src/workspace.rs`'s own replacement
witnesses fail on 2.40.0 and 2.41.0, until #326 added the setting to `git_command`. Those witnesses' fixtures pin
`core.useReplaceRefs = true` (`pin_replacement_refs_in`). The manager's fixture pins the same value in every base
repository it builds (`Fixture::new`, `src/workspace_manager/fixture.rs:325`). So on those two versions the
manager's witnesses would read the replaced graph on the producer side. I did not run them there.

## Reachability, against the owner's rule of 2026-09-11

**Neither limb holds, and more narrowly than `PR326-SCHEMA4-ROLE-ENVIRONMENT-REFUSES-REPLACEMENTS-EVERYWHERE`.**

- **The module's own header** says "Nothing here is a production caller", because its primitives were built ahead
  of the schema-4 coordinator.
- **That coordinator has since arrived.** `engine::topology::run` and `recover` construct a `WorkspaceManager`,
  and that path engages only by explicit schema choice.
- **So reaching this takes three things together:** an opt-in path, a Git older than 2.42, and a
  `core.useReplaceRefs = true` that someone configured. Git's default leaves it unset.

## What the change that takes this up should do

Pass `-c core.useReplaceRefs=false` beside the pair in `WorkspaceManager::command` and in `read_only_git`, as
`git_command` in `src/workspace.rs` does since #326 (`REPLACE_REFS_REFUSED`). Then pin it in the manager's own
census, the way `every_git_child_of_this_module_is_built_where_replacements_are_refused` pins `git_command`.

This works because a `-c` applies after every configuration file and after inherited command-line
configuration, and Git hands it to the children it starts. Measured: `worktree add`'s own `reset --hard`.

The runners' role processes carry the variable alone too. For them, the scoping
`PR326-SCHEMA4-ROLE-ENVIRONMENT-REFUSES-REPLACEMENTS-EVERYWHERE` describes closes this half as well.
