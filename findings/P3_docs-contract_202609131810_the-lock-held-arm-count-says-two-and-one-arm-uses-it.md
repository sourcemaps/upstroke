---
id: PR282-LOCK-HELD-ARM-COUNT-SAYS-TWO-AND-ONE-ARM-USES-IT
severity: P3
disposition: deferred
category: docs-contract
pr: 282
reviewed_sha: 5ca7abcd3104f92923ca76389e96f54131fa04e3
location: docs/internals/engine/topology/startup.md:647
provenance: introduced
first_bad: 5ca7abcd3104f92923ca76389e96f54131fa04e3
guard: "deferred: the sentence must say one arm, or name what the Husk arm does instead, which is to call rundir::is_running inline without binding lock_held"
---

## Failure sequence

`docs/internals/engine/topology/startup.md:647`:

> `lock_held` is read inside the **two arms** that use it and not above the `match`.

**One arm uses it.** At `5ca7abcd3104f92923ca76389e96f54131fa04e3`, `src/engine/topology/startup.rs`:

- `RunDirClass::Committed` (`:420`) binds `let lock_held = rundir::is_running(&public);` at
  `:421` and reads it at `:425` (`else if lock_held && !own`).
- `RunDirClass::Husk` (`:439`) calls `rundir::is_running(&public)` **inline** in its `if` and
  never binds or reads `lock_held`.
- `RunDirClass::Indeterminate` asks nothing further about the directory — which the same paragraph
  states correctly.

So the sentence's *point* is sound — the probe is not hoisted above the `match`, and the
`Indeterminate` arm does not pay for it — but its count is wrong. There is one binding, not two.

No behavioural surface: this is an internals note. `startup.rs` does not `include_str!` it, so
no test reads its contents.

Found by the `tencent/hy4-preview` regression lens only; neither fix-check half raised it.

## Why this is filed rather than fixed

Editing a tracked file moves the head and discards a three-lens review of record over one word.
`ORCH-P1.md` step 4: P3s are fine; **no clean-up round**. Step 5 files residue under
`findings/`, where a push confined to that directory keeps the review.
