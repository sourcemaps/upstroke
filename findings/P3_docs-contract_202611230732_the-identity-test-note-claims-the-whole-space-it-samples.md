---
id: PR406-IDENTITY-TEST-WHOLE-SPACE-OVERSTATED
severity: P3
disposition: accepted-risk
category: docs-contract
pr: 406
reviewed_sha: 62234ed2cc65e05de139cdb416cbf2b898e8f5f7
location: docs/internals/runner/host/tests.md:1295
provenance: pre_existing
first_bad: predates #406: the claim is in origin/master's version of docs/internals/runner/host/tests.md (cffdb983); #406 kept it while cutting the note
guard: the next change to every_production_invocation_identity_reaches_the_containment_points or its note, or a later docs PR 4 batch that edits docs/internals/runner/host/tests.md
---

## Failure sequence

From #406 round three's review:

    the note says the test covers the identity dimension "over the whole space production builds"
    -> the test samples one ordinal per identity class: `ReviewReask(0)`, `ReviewPass(2)`,
       `Gate(3)` and agent probe ordinal 2
    -> gate indices come from a configured gate list of any length, so production builds `Gate(n)`
       for every position in it
    -> a suppression keyed to `Gate(4)` passes the role grid (`Gate(0)`) and this test (`Gate(3)`)

The note, at `docs/internals/runner/host/tests.md:1295`:

```text
The **identity** dimension, over the whole space production builds rather than the five the role
grid carries (`worker`, `gate0`, `review_pass0`, and the two probes at ordinal 0). Every identity
```

The samples, at `src/runner/host/tests.rs:3627`, `:3637` and `:3646`, and the probe ordinal `2`
passed to `crate::agent::probe_request` a few lines below:

```rust
attempt(AttemptRole::ReviewReask(0)),
...
attempt(AttemptRole::ReviewPass(2)),
...
attempt(AttemptRole::Gate(3)),
```

Gate indices, at `src/gates.rs:206`:

```rust
for (index, gate) in gates.iter().enumerate() {
    let result = gate.check(
        runner,
        invocation(u32::try_from(index).unwrap_or(u32::MAX)),
        ws,
    )?;
```

The `Gate(4)` case is the reviewer's reasoning, not run; nothing here has run it either.

The same claim is in origin/master's version of the note (cffdb983, line 1736 there); #406 kept it
while cutting the note.

## Why it is accepted

- Unwitnessed, and P3: no failing test, reproduction or mutation witness.
- It is a note's wording and carries no `MUST`.
- #406 is a docs batch bounded at round three.

## What a change that takes this up should do

- Describe the test as covering each identity class with representative ordinals (a re-ask, a
  non-zero review pass, a non-zero gate index, a non-zero probe ordinal), not every identity
  production can build.
