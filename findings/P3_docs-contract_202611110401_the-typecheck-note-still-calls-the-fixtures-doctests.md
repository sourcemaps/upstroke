---
id: PR362-TYPECHECK-NOTE-STILL-CALLS-THE-FIXTURES-DOCTESTS
severity: P3
disposition: deferred
category: docs-contract
pr: 362
reviewed_sha: 4b0a628e19f19ebf94a5d3aea945885f96cb15bf
location: docs/internals/events/log/tests.md:1351
provenance: pre_existing
first_bad:
guard: the next change to the build-refusal notes in `docs/internals/events/log/tests.md`
---

## Failure sequence

This batch corrected the notes around `declared_build_refusals` in
`docs/internals/events/log/tests.md`: the build-refusal fixtures are fenced blocks read from
`docs/internals/events/log.md`, not doctests lifted from source comments.

It did not touch one note. Under the heading that quotes the line of `typecheck` which writes the
fixture wrapped in a `fn main`, line 1351 still reads:

> Doctests without a `fn main` are wrapped in one, so the fixtures are
> written that way and this wraps them the same.

The wrapping is real and the reason is stale. The fixtures are no longer doctests, so the note
explains today's code by a mechanism that is gone.

## What the change that takes this up should do

Say why the wrapper is there today: the fixtures are bodies without a `fn main`, and `typecheck`
supplies one. If the doctest origin is kept, label it as history.
