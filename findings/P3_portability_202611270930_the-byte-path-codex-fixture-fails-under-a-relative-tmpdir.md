---
id: PR437-ROSTER-FIXTURE-RELATIVE-TMPDIR
severity: P3
disposition: deferred
category: portability
pr: 437
reviewed_sha: bc645322053e851c026ae273e0bbe8d4ef48d907
location: src/agent/codex.rs:1056
provenance: introduced_by_feature
first_bad: 884dce5af0c78e46d3fa2aff58dae440840393f8, which moved the fixture's ordinary execution onto the worker request
guard: the next change to configured_fixture_preserves_directory_bytes_through_the_runner, or to the Codex fixture's directory handling
---

## Failure sequence

From #437's final general review on `bc645322` (`gpt-6.2-astra` at `max`, 2026-11-26), reproduced
by the reviewer:

    on Linux, TMPDIR names an existing relative directory
    -> run configured_fixture_preserves_directory_bytes_through_the_runner
    -> probe and discovery succeed
    -> the worker enters the fixture directory and resolves the relative cli.sh path again from there
    -> sh exits 2 before the verdict is produced

The test takes its scratch tree from `std::env::temp_dir()` (`src/agent/codex.rs:1032`), and so
from `TMPDIR`. `ConfiguredCodex::command` hands the fixture directory to `sh`, which runs
`sh "$CODEX_FIXTURE/cli.sh"` (`src/agent/codex.rs:965`). The worker request is given that same
directory as its working directory (`src/agent/codex.rs:1056`), so a relative directory is resolved
once to enter it and a second time, from inside it, to find `cli.sh`.

The reviewer's reproduction: the exact fixture test run with `TMPDIR=scratch` passed on `8976caec`
and failed on `bc645322` with libtest exit 101, worker exit 2 and "cannot open scratch/.../cli.sh".
The absolute spelling of the same directory passed on `bc645322`.

## Scope

- The defect is confined to this test's fixture. No production path is involved.
- The test passes whenever the temporary directory is absolute, as it is on the build box and the
  hosted runners.

## Why it is left open

It is left open under the owner's exception of 2026-11-27 for reproduced findings: a P3 confined
to test code, inside a feature change's allowance. It is not repaired in #437.

## What the change that takes this up should do

- Make the fixture directory absolute before the worker request is built.
- Add a relative-directory case, so a relative `TMPDIR` is exercised rather than assumed away.
