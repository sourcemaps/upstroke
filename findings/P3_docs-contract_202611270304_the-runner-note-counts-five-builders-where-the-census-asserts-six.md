---
id: PR435-RUNNER-NOTE-BUILDER-COUNT
severity: P3
disposition: deferred
category: docs-contract
pr: 435
reviewed_sha: 5ca9531f56e6eb0483bb43f6fb01e3672a0dab44
location: docs/internals/runner/contract.md:1118
provenance: introduced_by_feature
first_bad: introduced on this branch (PR #435): at origin/master (552c0395) the note's five matched the census, which asserted five; adding the `Decide` role moved the census to six and left the note at five
guard: the next change to `every_production_runner_request_is_built_by_its_roles_builder` or its note, or to the set of execution roles
---

## Failure sequence

From PR #435's round-two regression review, group 8, filed under ruling R79 at P3.

    the runner contract note describes the request-builder census
    -> it says there are five roles and five builders
    -> PR #435 added `ExecutionRole::Decide` and its builder, `runner::decide_request`
    -> the census now asserts six roles and six construction points
    -> so the note understates by one the count the test it describes holds

The note, at 5ca9531f `docs/internals/runner/contract.md:1117`-`:1118`, and at the repaired head
`:1120`-`:1121` (the round-two repair added lines above it and did not change it):

```text
Every `RunnerRequest` production builds is built by the builder for its
role, and there are five roles and five builders.
```

The census, at 5ca9531f and at the repaired head `src/runner/contract.rs:1776`-`:1780`:

```rust
        assert_eq!(
            expected.values().map(|(roles, _)| roles).sum::<usize>(),
            ExecutionRole::all().len(),
            "six roles, six construction points, and this is the count"
        );
```

At origin/master (552c0395) the same assertion reads "five roles, five construction points"
(`src/runner/contract.rs:1645` there), and the note's sentence is at its line 1073.

Nothing executes the note; the mismatch is between the sentence and the assertion, read here.

## What the change that takes this up should do

- Give the count the census asserts, six roles and six builders, or say the count without a
  number and point at the assertion that holds it, so the next role does not stale it again.
