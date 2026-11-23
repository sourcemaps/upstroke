---
id: PR406-UNIX-CONTAINMENT-STARTUP-MISSTATED
severity: P3
disposition: accepted-risk
category: docs-contract
pr: 406
reviewed_sha: 62234ed2cc65e05de139cdb416cbf2b898e8f5f7
location: docs/internals/runner/host/tests.md:1569
provenance: pre_existing
first_bad: predates #406: the claim is in origin/master's version of docs/internals/runner/host/tests.md (cffdb983); #406 kept it while cutting the note
guard: the next change to the a_containment_proof_exists_only_where_containment_was_established note, or a later docs PR 4 batch that edits docs/internals/runner/host/tests.md
---

## Failure sequence

From #406 round three's review:

    the note under `a_containment_proof_exists_only_where_containment_was_established` says
    that on Unix the startup step "is the reaper and the isolated process group"
    -> but Unix `join_ambient_job` does nothing and returns `Ok(())`
    -> the reaper and the process group are established later, per spawn
    -> so `contain_write_command` called before any spawn mints `Contained` while no reaper or group
       exists yet, and the note describes startup work that does not happen

The note, at `docs/internals/runner/host/tests.md:1569`:

```text
A refused establishment mints nothing. Only Windows can refuse; on Unix the step is the reaper and
the isolated process group, and nothing can fail.
```

The Unix step, at `src/agent/proc/ambient.rs:46`:

```rust
#[cfg(not(windows))]
pub fn join_ambient_job(_hooks: &mut dyn SpawnHooks) -> Result<(), UpstrokeError> {
    Ok(())
}
```

The other note already says it right, at `docs/internals/runner/host.md:574`:

```text
ambient job before any spawn". On Unix there is nothing to join: containment there is the
per-invocation reaper and the isolated process group, and this returns `Ok` having done nothing.
```

The same claim is in origin/master's version of the note (cffdb983, line 2147 there); #406 kept it
while cutting the note. Nothing has run this: the finding is about wording, and no test or
reproduction witnesses it.

## Why it is accepted

- Unwitnessed, and P3: no failing test, reproduction or mutation witness.
- It is a note's wording and carries no `MUST`.
- The code is right, and so is the other note (`docs/internals/runner/host.md:574`); only this
  sentence misplaces the Unix work.
- #406 is a docs batch bounded at round three.

## What a change that takes this up should do

- Say that on Unix the startup step does nothing and cannot fail.
- Say that the reaper and the isolated process group are established per spawn, not at startup,
  as `docs/internals/runner/host.md:574` does.
