---
id: MANAGED-INCLUDE-JOINS-THE-FIRST-DUPLICATE-GIT-CONFIG-PARAMETERS
severity: P3
disposition: deferred
category: correctness
pr: 441
reviewed_sha: d431d220f04a8fafd277b9bc55f4c113822167be
location: src/runner/host/environment.rs:445
provenance: pre_existing
first_bad:
guard: the next change to how compose reads or writes a name its base holds more than once
---

# The managed include joins the first of two `GIT_CONFIG_PARAMETERS` entries

## Failure sequence

1. A host runner that reads `ObjectGraph::RecordedIn`, the runner of a run on the managed repository
   (`src/runner/host.rs:179-187`), composes a role's environment from a base that holds
   `GIT_CONFIG_PARAMETERS` twice.
2. `compose` finds the first entry of that name (`src/runner/host/environment.rs:445-449`), appends
   the managed includes to its value (`:450-453`), and writes the result back with `upsert`
   (`:454-459`), which changes the first entry it matches (`:493-499`). The second entry is left as
   it was.
3. The runner hands the child the composed list (`src/runner/host.rs:337-338`).

The design says the runner appends the includes to `GIT_CONFIG_PARAMETERS` "after every entry the
role inherits or its overlay sets" (`design/15_design_event_log_resume_run_layout.md:471-474`). The
runner's own `composed_common_dir` reads the last entry of a name (`src/runner/host.rs:278`).

**How each step is known.**

- **Read:** each step, from the code at `d431d220`, not run.
- **Reported:** #441's round-four regression pass gave a managed gate two `GIT_CONFIG_PARAMETERS`
  entries, the same option first `old` and then `active`, and wrote "The merge base delivered
  `active`." (`~/selfhost-capacity-planner-20261126/evidence/review-regression-23970c2.md:11`,
  SHA-256 `4c83c0710446c78bac75e83f8d1282ba11875784826ebdb5c93c2de6ebce08de`). That is a measurement
  of #441's merge base `552c0395`, whose `src/runner/host.rs` and `src/runner/host/environment.rs`
  are `master`'s byte for byte. The frontier pass, on #441's head `23970c2d` and not on `master`,
  wrote "Command::envs previously installed the later entry" and "The unmodified base selected the
  second value." (`~/selfhost-capacity-planner-20261126/evidence/review-frontier-23970c2.md:60-61`,
  SHA-256 `fa5aade84cfa31e139ecee26b9d5238ac722def6a8f2ac1c938bf69e4d97dc61`).
- **Not reported by either pass, and not run:** that the value the child receives lacks the include.
- **Not established:** that any launcher hands upstroke a name twice; both passes supplied the base.
- **Found by** the design note on the capacity reader's credential boundary of 2026-11-28
  (`~/selfhost-design-20261127/reader-credentials.md`, SHA-256
  `2e792ee4a70262a567c414c8769a5a481e89a41615dfdd7fe9f3aa9856d5cd56`), which ran nothing, and read
  again at `d431d220` for this record.

## Why P3 correctness

Graded by ruling r114, at the grade the note proposed: not a credential matter.

## Related, not the same

- `PR326-SCHEMA4-ROLE-ENVIRONMENT-REFUSES-REPLACEMENTS-EVERYWHERE`
  (`findings/P2_correctness_202609262021_the-schema-4-role-environment-refuses-replacements-everywhere.md:78-81`):
  the managed entries go at the end of `GIT_CONFIG_PARAMETERS`, after every entry the role inherits,
  because Git reads it after the counted pairs.

## What the change that takes this up should do

- The include reaches the entry the child receives.
- Every other inherited entry keeps its effective value, which #441's repair did not (both of #441's
  round-four passes, on its head `23970c2d`).
