---
id: DIRECT-CHILDREN-INHERIT-THE-ENVIRONMENT
severity: P3
disposition: deferred
category: security-trust
pr: 441
reviewed_sha: d431d220f04a8fafd277b9bc55f4c113822167be
location: src/workspace.rs:79
provenance: pre_existing
first_bad:
guard: the owner's decision on what the engine's own Git, Docker and process-table children inherit, taken with HOST-ROLES-GET-EVERY-AMBIENT-CREDENTIAL's
---

# The engine's own Git, Docker and process-table children inherit the whole environment

## Failure sequence

1. The engine starts some children itself, as `std::process::Command`s, without clearing their
   environment: Git through `git_command` (`src/workspace.rs:78-103`), which removes and sets Git
   variables only; the manager's two Git builders (`src/workspace_manager.rs:7284-7287`,
   `:8140-8141`); the host Docker client through `docker_command`
   (`src/runner/container.rs:1701-1705`); and, off Linux, the reaper's `/bin/ps` query, which sets
   only `LC_ALL` (`src/agent/proc.rs:4817-4825`).
2. A `std::process::Command` hands its child the parent's environment unless told otherwise, so each
   of these children receives every credential the operator exported.

**How each step is known.**

- **Read:** each step, from the code at `d431d220`, not run.
- **Reported:** #441's round-four frontier pass, on #441's head `23970c2d` and not on `master`,
  exercised the Git builder at `src/workspace.rs:79` and the Docker builder, inspecting the commands
  they build and launching neither: "I exercised both exact-head command builders with an in-memory
  Command constructor seeded with a synthetic marker. Both completed command environments retained
  it. Neither client was launched."
  (`~/selfhost-capacity-planner-20261126/evidence/review-frontier-23970c2.md:28-29`, SHA-256
  `fa5aade84cfa31e139ecee26b9d5238ac722def6a8f2ac1c938bf69e4d97dc61`).
- The manager's two builders and `/bin/ps` are read only.
- *Inferred, not established:* what these children run in turn, Git's hooks and credential helpers
  among them, inherits the same.
- **Found by** the design note on the capacity reader's credential boundary of 2026-11-28
  (`~/selfhost-design-20261127/reader-credentials.md`, SHA-256
  `2e792ee4a70262a567c414c8769a5a481e89a41615dfdd7fe9f3aa9856d5cd56`), which ran nothing, and read
  again at `d431d220` for this record.

**They sit outside the runner on purpose:** the process-start census records the Docker client,
engine Git and the manager's Git as "deliberately NOT routed" (`src/runner/contract.rs:1723`,
`:1737`, `:1751-1752`), and the `/bin/ps` query as "a kernel query inside the reaper, not a CLI or a
gate" (`:1684-1687`).

## Why P3 security-trust

Graded by ruling r114, at the grade the note proposed.

## Related, not the same

- `HOST-ROLES-GET-EVERY-AMBIENT-CREDENTIAL`
  (`findings/P2_security-trust_202611281620_every-host-role-receives-every-credential-the-operator-exported.md`):
  the same values, reaching a run's roles through the runner.
- `PR128-PARENT-GIT-TRACE-MAKES-ABSENCE-AN-ERROR`
  (`findings/P2_correctness_202609040522_parent-git-trace-makes-absence-an-error.md:16`):
  `read_only_git` inherits the environment, so an inherited `GIT_TRACE=1` changes what a lookup
  reads.
- `PR326-MANAGER-GIT-CHILDREN-CARRY-THE-VARIABLE-ALONE`
  (`findings/P2_correctness_202609262021_the-managers-git-children-carry-the-variable-alone.md:19-20`):
  the manager's Git builders set `GIT_NO_REPLACE_OBJECTS=1` and nothing else about replacements.

## What the change that takes this up should do

- These children stay outside the runner, as the census has them (`src/runner/contract.rs:1723`,
  `:1737`, `:1752`).
- The design says what they inherit.
