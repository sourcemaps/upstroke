---
id: PR313-GIT-DISCOVERY-REDIRECT-INSIDE-THE-STATED-BOUNDARY
severity: P2
disposition: accepted-risk
category: security-trust
pr: 313
reviewed_sha: 604d8139749c907e9d0071e4a78438d3cc178eb4
location: src/workspace_manager.rs:779
provenance: pre_existing
first_bad: 7a83e69 (the funnel ran Git through those paths before the table existed); prior finding PR120-TABLE-OMITS-GIT-DISCOVERY-PATHS
guard: the directory-handle-relative rewrite of the funnel primitives (`standards/SWEEP.md` queue row 11)
---

## Failure sequence

a writer inside the execution root renames `<base>/.git` away and plants `<base>/.git -> <victim>/.git`, or does the same at a slot checkout's `.git` -> the in-funnel walk passes, because the nine walked roles do not include either path and cannot: the base lies outside the authorized private root the walk is anchored at -> the funnel's Git child resolves the victim's repository and the effect lands there, measured for the ref funnel as `git update-ref` exiting 0, the ref present in the victim and absent from the managed repository (`the_ref_funnel_follows_a_git_discovery_link_planted_at_the_before_hook`)

## What the change that takes this up should do

Nothing, unless the trust boundary changes. `GitWorkingDirectory` states why this is
accepted rather than open: every writer that can reach these paths is a process already
executing inside the execution root as this engine's own user — an agent's file tools, a
repository-authored gate command — and `design/15` places exactly that party outside the
shipped host runner's boundary ("defence in depth, not an OS boundary"; untrusted input
belongs on "a dedicated OS account or VM"). Such a writer can already write wherever that
user can, so the redirect adds no capability. The two routes that would reach it from
*outside* that boundary are closed and were measured: a repository's own hooks do not run
(`core.hooksPath` at an empty directory, proven real, link-free and empty adjacent to every
spawn), and a repository's own content cannot plant the path (`git worktree add` of a tree
carrying a top-level `.git` fails with `error: invalid path '.git'`, exit 128).

This row is here so that deleting the P1 does not delete the residue. The durable fix is
directory-handle-relative operations — `openat`/`unlinkat` against a descriptor held from
the check — which close this redirect and the check-to-syscall window of the nine walked
roles together, and which are a slice of work rather than a finding. Whoever takes it
should expect the three generated tests to move with it:
`every_git_discovery_path_the_table_names_is_outside_the_walk` asserts that the walk does
**not** cover these paths and fails the moment it does, and
`the_ref_funnel_follows_a_git_discovery_link_planted_at_the_before_hook` asserts the
redirect as it stands.
