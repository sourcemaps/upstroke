---
id: PR439-MUTATION-DIAGNOSTIC-FIVE-LEGS
severity: P3
disposition: deferred
category: docs-contract
pr: 439
reviewed_sha: 1555832f9a32f2b55fb57d9a86657b9fb31b0718
location: src/effects/tests/workflow.rs:3526
provenance: introduced_by_feature
first_bad: 0d3018d9293b58d27d376bfdd095f527cf55884c — "named-ci-category-legs", the commit that generated `test-linux`'s matrix from the test categories as seven legs; the diagnostic, written while origin/master's matrix had five, kept its count
guard: the refusal the escape measures is unchanged and is not what is wrong; the change that takes this up rewrites the workflow oracle's mutation diagnostic for `MUT-TEST-FILTER-HARDCODED` (its `escape` text in `WORKFLOW_ESCAPES`, src/effects/tests/workflow.rs) to count the legs the generated `test-linux` matrix has
---

## Failure sequence

From #439 round two's regression review, at `1555832f`:

```text
P3 · introduced_by_feature — The workflow oracle still describes five Linux legs. The diagnostic
for MUT-TEST-FILTER-HARDCODED says all five legs run the hard-coded subset. The generated matrix
applies that mutation to all seven.
```

    the_workflow_shape_oracle_refuses_every_escape_the_ledger_names applies
    MUT-TEST-FILTER-HARDCODED to `test-linux`'s one suite step
    -> `test-linux`'s matrix at 1555832f is the generated block between
       `# BEGIN TEST_CATEGORIES ubuntu` and `# END TEST_CATEGORIES ubuntu`
       (.github/workflows/ci.yml:126-142): seven `include` entries, at lines 128, 130, 132, 134,
       136, 138 and 140 — coordinator, engine, kill-path, runner, self-checks, unit, workspace
    -> the one suite step serves every leg, so the hard-coded `engine::` reaches all seven
    -> the mutation's diagnostic says five: "All five Linux legs then run the one subset ... all
       five are green"
    -> the assertion at src/effects/tests.rs:5523 (5647 at this task's head) prints that text
       when the oracle stops refusing the mutation, so the message that explains the escape
       miscounts the legs it reaches

The diagnostic, at `src/effects/tests/workflow.rs:3525`:

```rust
        escape: "`test-linux`'s suite step names one filter, `engine::`, where it read the \
                 leg's `matrix.tests`, the table untouched. All five Linux legs then run the one \
                 subset, and none holds a count to a floor: all five are green and every test \
                 whose name does not contain `engine::` runs on no leg of this job. \
```

Where the five came from: on origin/master `test-linux`'s matrix is five legs,
`ubuntu-latest, 1 of 5` to `ubuntu-latest, 5 of 5` (`.github/workflows/ci.yml:147-155` there),
and the same sentence is at `src/effects/tests/workflow.rs:3733` there, true of that matrix.
`git log -S 'BEGIN TEST_CATEGORIES ubuntu'` from `1555832f` names `0d3018d9` as the commit that
introduced the generated block, and the block held seven entries there too. The diagnostic was
not changed with it. The text is the same at this task's head.

The sibling diagnostics that count legs in `WORKFLOW_ESCAPES` were read against the same matrix:
`MUT-TEST-SHARD-SELECTION-DROPPED` and `MUT-TEST-LEGS-NAMED-BY-RUNNER` say nine macOS legs, and
the generated `macos` block has nine.

Established by reading the oracle, the assertion and the workflow at this SHA; nothing was run,
in the review or here.

## Why it is deferred

- P3, and unwitnessed: there is no failing test, reproduction or mutation witness. The text is
  printed only on a failure path, and the head does not reach it: the refusal it explains
  (`test-job-command`) is unchanged and still holds.
- It is a descriptive string in a test's escape catalogue and carries no `MUST`.
- #439's round three, the last under its bound, files it rather than repairs it.

## What the change that takes this up should do

- Keep the escape's name, anchor, replacement and `refused_as`; only the count in its text is
  wrong.
- Make the diagnostic agree with the generated `test-linux` matrix: say seven, or say "every Linux
  leg" so the text does not go stale the next time `TEST_CATEGORIES` changes how many legs the job
  has.
- Read the internals notes that name the mutation (`docs/internals/effects/tests/workflow.md:245`
  and `docs/internals/effects/tests/ci_model.md:100`) before writing; neither states a leg count
  today, and the change should not add one there that can drift the same way.
