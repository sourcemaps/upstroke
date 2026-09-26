---
id: PR326-SCHEMA4-ROLE-ENVIRONMENT-REFUSES-REPLACEMENTS-EVERYWHERE
severity: P1
disposition: deferred
category: correctness
pr: 326
reviewed_sha: 915c0646c1e502c74df596586d6232ce2df7a6fe
location: src/runner/host/environment.rs:145
provenance: pre_existing   # found in #326 round 3, scoping the v0.1 instance #326's regression lens reproduced at 915c0646
first_bad: db8a525a8e3d2dc5dcec5ac81acd7829d537be90
guard: the schema-4 change that scopes the isolation to the managed repository, in the shape #326 settles for v0.1
---

## Failure sequence

1. A run on the schema-4 path, which engages only by explicit schema choice, has a gate whose own work creates a
   fixture repository and uses `git replace` in it: the test suite of Git itself, of `git-filter-repo`, or of any
   Git tooling.
2. Both schema-4 runners put `GIT_NO_REPLACE_OBJECTS=1` on every role, and put it last:
   - `HostEnvironment::compose`, under the `ObjectGraph::Recorded` reading `HostRunner::new()` installs
     (`src/runner/host/environment.rs:145`);
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
  from the source.

On Git 2.40 and 2.41 the same variable is also not final against a configured `core.useReplaceRefs = true`
(`design/15`, "What an exact snapshot is exact against"). The scoping below closes that half for role processes
too, against every configuration file.

## Reachability, against the owner's rule of 2026-09-11

**Neither limb holds, and the reason differs from #326's.**

- **On the v0.1 path** the review's instance can happen in normal use: any v0.1 user whose tests exercise replace
  refs meets it, with no unusual configuration. That is why it blocks #326.
- **Schema 4 is opt-in.** The machinery "engages only by explicit schema choice" (`CLAUDE.md`), and no `0.2.0` tag
  exists. So this cannot happen in normal use of what is released.
- **No one without push access can trigger it:** choosing schema 4 is the operator's own configuration.

## What the change that takes this up should do

**The fix shape is known:** scope the isolation to the managed repository instead of the process. #326's round 3
measured two ways of doing it through the v0.1 call path, on Git 2.40.0, 2.41.0, 2.42.0 and 2.43.0. With either,
the own-fixture gate passed, and so did the finding's original sequence: `git diff --exit-code HEAD` over an
untouched snapshot of a replaced tree.

- **`core.useReplaceRefs = false` in the repository's configuration.** Schema-4's task and merge worktrees are
  detached linked worktrees of the operator's repository (`design/26`), and they read its common `.git/config`.
  So this form writes the operator's own configuration, and the setting outlives the run.
- **The same setting through the role environment.** `GIT_CONFIG_COUNT` pairs
  `includeIf.gitdir:<common dir>.path` and `includeIf.gitdir:<common dir>/.path` name an engine-owned file
  holding `[core] useReplaceRefs = false`, and nothing is written into the repository. What the implementation
  has to get right, each measured on 2.43.0 unless it says otherwise:
  - **Both patterns.** `<common dir>/` alone misses the main worktree's own gitdir.
  - **Escaping.** The pattern is a glob, so `[`, `*`, `?` and `\` in the path must be escaped. Unescaped, a `[`
    silently fails to match and a `*` matches other repositories as well.
  - **The realpath.** The realpath matches where a symlinked spelling does not.
  - **Appending.** The pairs go after an operator's own `GIT_CONFIG_COUNT` pairs, never over them.
  - **Windows.** It needs `gitdir/i:` and forward slashes. Not measured.

**Neither is built into #326 yet.** Its choice between the two for the v0.1 path is pending with the owner's
orchestrator, and whichever it takes is the shape to reuse here. The container runner needs the pattern in the
container's own paths.

**Two limits both forms share:**

- a later command-line `core.useReplaceRefs=true` still wins;
- on Git 2.40.0, `git merge-tree` reads a replaced commit despite the setting. Git 2.41 fixed that.
