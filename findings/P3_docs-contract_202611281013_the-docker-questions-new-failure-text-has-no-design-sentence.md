---
id: PR446-CLI-DIAGNOSTIC-DESIGN-OMISSION
severity: P3
disposition: accepted-risk
category: docs-contract
pr: 446
reviewed_sha: af0b64b737c48c1b1881fc498ef3b930f2a7e5e5
location: src/runner/container/census.rs:576
provenance: introduced_by_feature
first_bad:
guard: the change, if the owner asks for one, that states the container runner's failure text in the design
---

# The Docker question's new failure text has no design sentence

## Failure sequence

The frontier reviewer's sequence, from #446's review:

    a census listing ends before answering
    -> the census's refusal then includes the new end and elapsed time on the command line's
       standard error
    -> the pull request documents this output only in its internal notes, and leaves the design
       without the update standards section 13 speaks of

## What the change that takes this up should do

Ruling r109 accepts the finding, for these reasons:

- Lines 24 to 26 of standards section 13
  (`standards/13_standards_documentation_and_observability.md`) carry no `MUST` or `SHOULD`, so
  they are guidance (`standards/01_standards_authority_and_scope.md` lines 8 to 10).
- The design states no failure text. `design/26_design_merge_queue_protocol.md` lines 672 to 678
  is about fault documents.
- Section 13's lines 8 to 12 put a module's contract in its notes file, which is where #446
  wrote it.
- No behaviour changes.
- The observation is true and not acted on, which is what an accepted risk records
  (`MAINTAINING.md` lines 136 to 139).

If the owner asks for one, the change that takes this up states the container runner's failure
text in the design.
