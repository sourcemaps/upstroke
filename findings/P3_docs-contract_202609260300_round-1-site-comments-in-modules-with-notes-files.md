---
id: PR322-ROUND-1-SITE-COMMENTS-IN-MODULES-WITH-NOTES-FILES
severity: P3
disposition: deferred
category: docs-contract
pr: 322
reviewed_sha: 1d957ae23f9c17e700087588882e5335986c70b1
location: src/rundir/tests.rs:34
provenance: introduced_by_feature   # this pull request's round 1
first_bad: the round-1 conversion commits of PR #322
guard: a docs-only change that moves the prose, with no code in its diff
---

## Failure sequence

`standards/13_standards_documentation_and_observability.md` says that where a module has a
`docs/internals/` notes file, that file *"carries the whole of the module's prose — contracts,
rationale, history, worked examples — and the source carries a single `Extended notes:` pointer in
its module header **and no other comment**"*.

**This pull request's round 1 added 351 comment lines to 25 modules that have notes files.** Round 2's
own prose went into the notes files as the standard requires; round 1's was left at the sites. So
those 25 modules now carry site prose the standard places in their notes file, and a reader of the
notes file does not find it there.

## Why it is deferred rather than fixed here, and by whose decision

**The orchestrator's ruling, recorded in this pull request's body under "Standard §13".**

`standards/01_standards_authority_and_scope.md` grades obligations: *"**MUST** and **MUST NOT** are
requirements: a deviation needs a reviewed change to the standard, not an ad hoc exception.
**SHOULD** is the default: deviate with a stated reason in the code or the pull request."*
**§13 contains no `MUST`**, so it is SHOULD, and a stated reason in the pull request is the
documented route rather than an ad hoc exception. §13 also says *"the obligation is that the contract
is written and can be found, not where it sits"*, and these contracts are written and findable.

The reason: relocating 351 comment lines across 25 modules is a large diff whose only effect is to
move prose, in a pull request that is otherwise reviewed and green, and the risk of disturbing a
converted fixture while moving prose past it exceeds the benefit of the move.

**`MAINTAINING.md` step 5's non-discretionary limb does not reach this row.** That limb covers a
`MUST` deviation in materially touched code, and a finding carrying a failing test, reproduction or
mutation witness. This is neither: §13 carries no `MUST`, and no test or witness fails on it — no gate
reads comment placement at all.

## What the change that takes this up should do

Move the 351 lines into the 25 modules' notes files, leaving each module header's single
`Extended notes:` pointer, **in a docs-only change with no code in its diff** — which is the guard on
this row. Keep §13's own carve-out: *"Reasoning another standard requires **at** a site — a
`SAFETY:` obligation (§11), a concurrency protocol (§10) — is that standard's to place, and a module
with a notes file is not excused from it."* So a comment that another standard requires at its site
stays, and the mover should say which ones those are rather than moving every line mechanically.

## Reachability, against the owner's rule of 2026-09-11

**Neither limb.** Comment placement is not reachable in ordinary use and nobody without push access
can change it.
