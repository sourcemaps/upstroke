---
id: PR299-UNTOUCHED-ESCAPE-ROWS-SAY-SELF-HOSTED
severity: P3
disposition: deferred
category: docs-contract
pr: 299
reviewed_sha: fb136de52a48384bddbb8a4adfeee541449c7542
location: src/effects/tests/workflow.rs:1692
provenance: introduced_by_feature
first_bad:
guard: the next change that edits the `MUT-TEST-WINDOWS-*` escape table in `src/effects/tests/workflow.rs`
---

## Failure sequence

#299 routes `test-windows` to `windows-latest` on the merge-queue lane and keeps the guest on
the pull-request lane -> four escape rows it left byte-identical to the base,
`MUT-TEST-WINDOWS-COMMAND-DELETED` (`workflow.rs:1692`), `MUT-TEST-WINDOWS-DISABLED`
(`:1702`), `MUT-TEST-WINDOWS-RUN-RETARGETED` (`:1967`) and
`MUT-TEST-WINDOWS-CASEFOLD-DECLARATION-DROPPED` (`:2031`), still call the job or step "the
self-hosted job" or "the self-hosted step" -> each mutation bites on both lanes, one of which
is now hosted -> the recorded escape describes one lane of what the oracle refuses. The
oracle's refusal of all four is unchanged and measured by
`the_workflow_shape_oracle_refuses_every_escape_the_ledger_names`.

## Why it is not fixed in #299

Frontier review 9 of #299, finding 6
(https://github.com/sourcemaps/upstroke/pull/299#issuecomment-5723483851). The text is inside
a CI-contract test, an instrument: editing it after the review costs a fresh pass
(`MAINTAINING.md` step 5), and the finding does not block.

## What the change that takes this up should do

Reword the four `escape:` strings to name the job without its machine ("the `test-windows`
job", "its test step"), so each describes the mutation on either lane. Text only; no
`anchor:`, `replacement:` or expected code changes.
