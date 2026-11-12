---
id: PR362-EXIT-RUNS-DESTRUCTORS-STANDS-IN-FOUR-MORE-PLACES
severity: P3
disposition: deferred
category: docs-contract
pr: 362
reviewed_sha: 4b0a628e19f19ebf94a5d3aea945885f96cb15bf
location: docs/internals/events/log/tests.md:731
provenance: pre_existing
first_bad:
guard: the change that corrects the four passages together, to the wording `docs/internals/agent/proc/hooks.md` and `docs/internals/events/log.md` now carry
---

## Failure sequence

This batch corrected one statement in `docs/internals/events/log.md`, which said that both a panic
and `std::process::exit` run destructors. That is wrong about `exit`. A panic unwinds and runs the
stack's destructors; `std::process::exit` runs **no** stack destructors, only the exit handlers
registered with the platform; `std::process::abort` runs neither.
`docs/internals/agent/proc/hooks.md` already states the three cases correctly.

The same wrong statement still stands in four places that none of the batch's findings named:

- `docs/internals/events/log/tests.md:731`: "both `panic!` and `exit` run destructors".
- `docs/internals/runner/host/tests.md:1891`: the same words.
- `src/rundir.rs:359`, in a doc comment: "both of the other two run destructors".
- `src/workspace_manager/hooks.rs:312`, in a doc comment: "both `panic!` and
  `std::process::exit` run destructors".

Each of the four explains why a fault-injection kill is an abort. The reason it gives is right in
its conclusion and wrong in that one clause, so a reader who follows it is told that an exit path
performs cleanup it does not.

## What the change that takes this up should do

Correct that clause in all four places, **in one change**, to the three cases as
`docs/internals/agent/proc/hooks.md` states them: a panic unwinds and runs the stack's destructors,
`std::process::exit` runs no stack destructors and only the registered exit handlers, and
`std::process::abort` runs neither. `docs/internals/events/log.md` now states them the same way.

Change no code. Two of the four passages are doc comments in `src/`, and the repair is to their
wording alone.
