---
id: PR405-NON-JSON-BUILD-OUTPUT-FAILS-THE-WINDOWS-LEGS
severity: P2
disposition: deferred
category: correctness
pr: 405
reviewed_sha: 3ee13a9e10b5d1c7615aa715b70becbfcb77d359
location: .github/workflows/ci.yml:550
provenance: introduced_by_feature
first_bad:
guard: the first proc macro in the build that prints to standard output, or the next change to script line 6
---

# A build output line that is not JSON fails every Windows leg

Found by the frontier review's round three over #405 (finding 1), and filed deferred under ruling r46.

## Failure sequence

    a proc macro or build script in the build prints a line to standard output that is not JSON
    -> the suite step builds with `cargo test --all-targets --all-features --no-run --message-format=json-render-diagnostics`, and Cargo passes that line through to the step's build log
    -> line 6 of the suite script sends every stdout line of that build through `ConvertFrom-Json` (`.github/workflows/ci.yml:550` at the reviewed commit)
    -> `ConvertFrom-Json` throws on the line that is not JSON
    -> the step fails on all ten Windows legs before a harness starts, on a tree whose build succeeded

## What is established

- Cargo's documentation says that, with `--message-format=json`, a proc macro or build script may print
  output that is not JSON on standard output.
- Nothing in the build prints today:
  - the crate is not a proc macro;
  - the proc macros among its dependencies print nothing to standard output.
- Ten legs parsed the build's output cleanly in each of three runs.
- A printing macro fails red, on the pull request that brings it in: the Windows legs run on every
  pull request, so the failure does not pass unseen.

## What the change that takes this up should do

- **Parse only the lines that start with `{`**, and let any other line through unparsed; every message Cargo
  emits in this mode is one JSON object on one line.
- **Pin it with one mutation row** in the workflow oracle's ledger (`WORKFLOW_ESCAPES` in
  `src/effects/tests/workflow.rs`), so that a change dropping the filter is one the oracle refuses.
- Its repair branch: `fix-P2/correctness_a-build-output-line-that-is-not-json-fails-every-windows-leg`.
