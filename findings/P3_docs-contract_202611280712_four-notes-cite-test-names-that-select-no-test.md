---
id: NOTES-CITE-TEST-NAMES-THAT-SELECT-NO-TEST
severity: P3
disposition: deferred
category: docs-contract
pr:
reviewed_sha: 6513c22d24f8404b6877a6da578deaff19aa7224
location: docs/internals/runner/host/tests.md:440
provenance: pre_existing
first_bad:
guard: a docs-contract fix that repoints each of the four names to the test its sentence describes, or deletes the sentence
---

# Four notes cite test names that select no test

## Failure sequence

A reader takes a test name a note cites and asks Cargo for it, and no test is selected: at
`6513c22d`, four citations name a test path at which `master` has no test.

- `docs/internals/runner/host/tests.md:440` cites `proc::tests::sigchld_reaper_host_helper`. The
  function is `src/agent/proc.rs:5878`, inside `mod termination` and its `mod tests`, so its path is
  `agent::proc::termination::tests::sigchld_reaper_host_helper`.
- `docs/internals/runner/container/census.md:991` cites
  `proc::tests::the_unix_reaper_kills_labeled_containers_before_releasing_r28`, a name found nowhere
  else in the tree. *Inferred:* the sentence most nearly names
  `unix_reaper_kills_labeled_containers` (`src/agent/proc/tests.rs:3291`).
- `docs/internals/runner/container/resolve.md:573` cites
  `tests::a_runtime_that_reports_no_digest_and_one_that_reports_an_empty_string_both_resolve_to_none`,
  found nowhere else. *Inferred:* it most nearly names
  `a_runtime_reporting_no_digest_and_one_reporting_an_empty_string_both_resolve_to_none`
  (`src/runner/container/resolve/tests.rs:222`).
- `docs/internals/runner/container.md:1885` cites
  `tests::the_transcribed_unreachable_diagnostics_are_what_the_live_cli_prints`, found nowhere else.
  *Inferred:* it most nearly names `real_docker_prints_the_transcribed_unreachable_diagnostics`
  (`src/runner/container/tests.rs:5660`).

## What is established

- Each of the four was read on `6513c22d` with `git grep`.
- They were found by ruling R85's control on #439's head `bcb4c251`, which listed the Linux library
  and binary tests and matched every backticked test path under the notes directory against the list
  (`~/selfhost-catshards-20261125/evidence/r85-cited-names.json`, SHA-256
  `1fea1c95c81e95e7025d39107e04dd1cfa9dd0e934fe4a0228c108e71af56c47`). Its other two stale names are
  #439's split, not `master`'s, and three of its seven unresolved names are modules of the effects
  tests.
- The list is not claimed complete: the control ran on `bcb4c251`, not on `master`, and the 29 cited
  names it found defined in source but not listed on Linux were not checked further.
- `docs/internals/effects.md:1829` is left out. Its sentence is past tense about two former helpers
  ("This derivation was written twice"), and a notes file carries history as well as contracts
  (`standards/13_standards_documentation_and_observability.md:10`).
- A stale note is a defect: the rule that a stale comment is a defect also applies to notes
  (`docs/internals/README.md:136`). One defect with one fix shape is one finding, as
  `SWEEP-HOST-ENVIRONMENT-002` is.

## What the change that takes this up should do

Repoint each of the four names to the test its sentence describes, or delete the sentence.
