---
id: PR326-SCHEMA4-ROLE-ENVIRONMENT-REFUSES-REPLACEMENTS-EVERYWHERE
severity: P2
disposition: deferred
category: correctness
pr: 326
reviewed_sha: 915c0646c1e502c74df596586d6232ce2df7a6fe
location: src/runner/host/environment.rs:345
provenance: pre_existing   # found in #326 round 3; reclassified P1 -> P2 by the review of record of 6e3e618f (gpt-6-astra, max, 2026-09-27)
first_bad: db8a525a8e3d2dc5dcec5ac81acd7829d537be90
guard: the change that activates the schema-4 topology conductor, which moves both runners onto the repository-scoped includes #326 built for v0.1 first
---

## Failure sequence

1. A run on the schema-4 path, which no supported configuration reaches on this head (Reachability, below), has
   a gate whose own work creates a fixture repository and uses `git replace` in it: the test suite of Git itself,
   of `git-filter-repo`, or of any Git tooling.
2. Both schema-4 runners put `GIT_NO_REPLACE_OBJECTS=1` on every role, and put it last:
   - `HostEnvironment::compose`, under the `ObjectGraph::Recorded` reading `HostRunner::new()` installs
     (`src/runner/host/environment.rs:345`);
   - `ContainerEnvironment::compose`, unconditionally (`src/runner/container/env.rs:232`).
3. The variable is process-wide. Every Git child of the gate takes it, in every repository, including the fixture
   the gate has just created.
4. The fixture's replacement has no effect, the gate's own assertion fails, and every attempt of every gated task
   fails its gate.

Measured through the v0.1 runner at `915c0646`, which composed exactly what `HostRunner::new()` composes: it was
`new()` with the same environment, reading `Recorded`.

- Through `ShellGate::check`, the own-fixture gate went from `Pass` to `Fail` on Git 2.40.0, 2.41.0, 2.42.0 and
  2.43.0.
- An engine run with that gate parked on 2.43.0.
- Neither the schema-4 topology engine nor the container runner was driven. The container's composition is read
  from the source. The review of record confirmed the cross-repository effect independently on Git 2.43.

On Git 2.41 the same variable is also not final against a configured `core.useReplaceRefs = true`. (Git 2.40
fails the same way, and is below the floor `README.md` states since #326 round 4.) The repository-scoped includes
below close that half for role processes too: they come after a `true` in every configuration file, the
worktree's own `config.worktree` among them, and after one in inherited `GIT_CONFIG_COUNT` or
`GIT_CONFIG_PARAMETERS`.

## Reachability, against the owner's rule of 2026-09-11

**Reclassified from P1 to P2** by the review of record of `6e3e618f`: `gpt-6-astra` at `max`, 2026-09-27.
`findings/README.md` reserves a severity change to a reviewer. Its reachability paragraph, verbatim:

> At `6e3e618fa0b45bdcd96ef9a00647cb32f2103df7`, this schema-4 defect is latent behind inactive production
> activation. `TOPOLOGY_ACTIVATION` is hard-coded `Inactive`, the production reader ceiling is 3, and CLI run and
> resume enter the legacy conductor. No supported flag, configuration, or recorded-schema choice activates the
> topology conductor. Therefore neither ordinary use nor input from someone without push access reaches this
> schema-4 failure on this head. Internal scaffolding can exercise the machinery; production activation requires
> source changes. The reachable v0.1 instance is a separate merge blocker.

Checked again at #326 round 4, which closes that v0.1 instance and leaves both schema-4 compositions as they were.
`TOPOLOGY_ACTIVATION` is still `Inactive` (`src/topology/schema.rs:27`), and compile-time assertions at `:39` and
`:41` pin it and the reader ceiling of 3. `src/main.rs:309` and `:323` still send run and resume to `engine::run`
and `engine::resume`.

## What the change that takes this up should do

**Reuse the v0.1 mechanism, and move both runners onto it before the topology conductor is activated.** #326
round 4 built it for the v0.1 runner: `HostRunner::for_legacy_workspace(repository)` reads
`ObjectGraph::RecordedIn(repository)`. For every role, that appends two conditional includes to
`GIT_CONFIG_PARAMETERS`:

```
includeIf.gitdir:<common dir>.path=<include>
includeIf.gitdir:<common dir>/worktrees/*.path=<include>
```

The include holds `[core] useReplaceRefs = false`, and nothing is written into the repository.
`Workspace::recorded_objects_scope` finds the common directory and writes the include. The schema-4 host runner
needs the same, from the repository its conductor manages. The container runner needs the include inside the
container, and the patterns spelled for the Git directory as the container sees it. #326 designed neither of
those two.

**The entries go at the end of `GIT_CONFIG_PARAMETERS`, after every entry the role inherits or its overlay sets.**
An earlier version of this row prescribed appending them to `GIT_CONFIG_COUNT`. That does not defeat an inherited
override. Git reads `GIT_CONFIG_PARAMETERS` after the counted pairs, so an inherited
`'core.useReplaceRefs'='true'` there still wins. The review reproduced that on Git 2.40.0, 2.41.0 and 2.43.0.

**The repository-configuration form does not work, on either count.**

- `core.useReplaceRefs = false` in the repository's configuration changes the operator's common configuration. It
  is visible from unrelated sibling worktrees, and it outlives completion, failure and snapshot removal.
- It does not win either. `config.worktree`, inherited `GIT_CONFIG_COUNT` and inherited `GIT_CONFIG_PARAMETERS`
  each outrank the repository file.

The review reproduced both on 2.40.0, 2.41.0 and 2.43.0. This row's earlier claim that the form covered every
configuration file missed `config.worktree`.

**What the v0.1 implementation had to get right, each pinned by a #326 witness:**

- **Two patterns.** `gitdir:<common dir>` for a main worktree, and `gitdir:<common dir>/worktrees/*` for every
  linked one, the snapshots included. A bare `<common dir>/` would also reach the Git directories of submodules
  under `modules/`.
- **The canonical path, glob-escaped.** Each of `[`, `]`, `*`, `?` and `\` is escaped. A quote in the path is
  closed, escaped and reopened inside the single-quoted key.
- **Git for Windows's spelling, and case as each directory matches it.** The keyword is `gitdir:` on every
  platform, never `gitdir/i:`, which folds every component and so reached a repository whose path differs from
  the managed one only in case (#326 round 4's rule on Windows and macOS, replaced in round 5). On Windows the
  path has no `\\?\`, forward slashes, and `//server/share` for a UNC path. Each component of the common
  directory, and the `worktrees` of the second pattern, is spelled with a class of both cases per ASCII letter
  where the directory holding it finds it under the other case, and exactly where it does not (`worktrees` since
  round 6, asked through the common directory's `refs`). An alias spelled in the other case defeats that
  lookup: `PR326-A-JUNCTION-MAKES-A-CASE-SENSITIVE-DIRECTORY-READ-AS-FOLDING`.
- **An include that exists whenever a role starts.** #326 writes it at
  `<private root>/git/recorded-objects.gitconfig` before a run's or resume's first role, and refuses to start a
  role while it is missing or altered.

**Limits the mechanism keeps:**

- a later explicit `git -c core.useReplaceRefs=true` inside a role still wins, and a role that clears its
  environment loses the include: configuration is not an enforcement boundary;
- Git 2.40's `git merge-tree` ignores the setting altogether. The floor is 2.41 since #326 round 4, and a v0.1 run
  refuses an older Git by name.
