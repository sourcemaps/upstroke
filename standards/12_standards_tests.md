## 12. Tests

Tests are executable evidence of a contract. New behaviour and bug fixes come with tests at the
lowest layer that can observe the real failure, plus a higher-level test when composition is the
risk. A regression test fails for the reported defect before the fix and passes after it; prefer a
minimal reproducer naming the violated contract over a broad snapshot that happens to change.

Cover, as applicable: the success path; invalid, missing, malformed and boundary input; each error
category that changes caller behaviour; interrupted writes, partial external output, failed cleanup,
retry and resume; platform semantics; concurrent winner/loser and cancellation paths; persisted
formats and public API compatibility. Property and state-machine tests are worth considering for
parsers, replay, routing, serialization and concurrent protocols.

Tests are deterministic and hermetic:

- unique temporary directories with RAII cleanup;
- injected or controlled clocks, randomness, environment, capacity observations and process
  responses;
- no dependence on network, ambient credentials, user configuration, PATH contents, execution order
  or an installed vendor CLI unless the test is explicitly an integration test;
- no sleeping as synchronization: signal the state the test needs to observe;
- no silent success when a prerequisite is absent: provide it, classify the test out of the default
  suite, or fail with a diagnostic;
- `#[ignore = "reason"]`, never a bare `#[ignore]`;
- assertions on externally meaningful state, not on the implementation copied into the test as its
  own oracle.

**Readiness.** A readiness signal is published after the state it announces is complete and
observable, never alongside it. A file's existence is a signal only if the file is published
atomically: an empty marker created after the state, or a file staged elsewhere and renamed into
place — never a path created in place and written afterwards. A partial record is never readable as
a whole one: a newline-delimited record is complete only when its terminator arrives, and an
unterminated tail fails rather than yielding a short value. Keep the payload inside what the framing
carries; a path is not safely a line. Every wait is bounded, and the bound bounds a wedged producer
rather than timing a healthy one: a deadline short enough to expire on a loaded runner has become
the failure it exists to prevent.

**Flakes.** An intermittent failure is a fact about the repository, handled by measurement rather
than adjective:

- measure a rate — failures over observed runs, with platform, head and the failing assertion —
  before calling anything a flake;
- establish provenance: a byte-identical tree producing both outcomes proves nondeterminism, not
  its origin. Reproduce on a head that predates the change, or argue causally from the diff; with
  neither, a new intermittent failure is a candidate regression, and "it passed on re-run" never
  merges a change;
- fingerprint by platform and by the failing assertion or error code, not by test name;
- name an owner and the consequence: a red matching the fingerprint is this flake until shown
  otherwise, and a red that does not match is a regression;
- an identified mechanism on a supported platform is a defect at any rate, fixed by repairing it;
- keep per-run output so each occurrence's outcome and diagnostic survive together.

Carried flakes live in the finding ledger (`findings/`, one file per finding; older
ones in `48f2af7a:reviews/FINDINGS.md`) with their rate, owner and consequence.

**Classifying a test for CI.** The ordered `TEST_CATEGORIES` table in
`src/effects/tests/ci_model.rs` is the registry for all three platforms. The first row whose
`filters` contain a substring of the full libtest name owns that registration, including an
ignored test. These are case-sensitive substrings, not prefixes, regular expressions or exact
names. Put a specific topic before its broader residual row. The kill-path rows precede the
coordinator and engine rows; `unit/model` precedes the broad `self-checks/effects` match so that
`topology::effects::` stays in `unit`. A later match is permitted only when the generated selectors
still select the name in exactly its first row's leg. No leg executes an uncategorised remainder.

Each row declares a `category`, `subcategory`, `filters`, a `macos` leg number and `windows`
partitions of `(leg, passing-test floor contribution, filters)`. Linux uses leg 1 of each category.
An empty Windows partition filter list inherits the row's filters; otherwise exactly one
partition must match the name by substring. It is not a fallback partition after other matches.
The categories are `kill-path`, `coordinator`, `engine`, `workspace`, `runner`, `unit` and
`self-checks`; their subcategories and platform assignments live in the table.

After adding or renaming a test, run from the repository root:

```bash
python3 .github/scripts/test-categories.py generate
python3 .github/scripts/test-categories.py generate --check
bash .github/scripts/test-categories.sh --explain
```

The generator changes only the three `BEGIN TEST_CATEGORIES` / `END TEST_CATEGORIES` matrix
blocks in `.github/workflows/ci.yml`. Commit the table and its generated matrices together; do
not hand-edit the generated selectors. `generate --check` refuses stale blocks. The local
check builds and lists the native all-target, all-feature inventory, including ignored tests,
without executing test bodies. It checks one correct leg per name and lists again with all
category filters skipped: that remainder must be empty. It requires at least one registration
owned by `unit/cli`. `--explain` reports subcategory counts and later matches. The script uses
`upstroke-build cargo` when that wrapper is available; it does not assign a target directory.
Workstations and CI otherwise use Cargo directly. `check` refuses before starting any subprocess
when `RUSTC_BOOTSTRAP` is present in its environment, even if empty. Inside a running test,
`prove --harness` lists the already built current executable without starting Cargo.
`prove --require-cli --harness` also applies the standalone check's CLI requirement; a library
harness alone does not contain the binary's CLI registrations.

For `uncategorised: <name>`, inspect the test's full module path and purpose. Move it into the
appropriate already classified module, or add a narrowly justified filter or row to
`TEST_CATEGORIES`. For `uncategorised partition`, update the owning row's Windows partitions so
exactly one matches. For `category selection`, inspect the reported first owner and selected
legs for substring overlap; adjust the row order or filters. Regenerate and repeat the native
check after the correction. Do not add an empty selector, a catch-all runner, or an ignore to
silence the failure: ignored registrations are classified too. A local listing covers only its
host's `cfg` registrations; the native checks on the other operating systems remain required.

Leg names, floors and the OS-specific CI checks are described in `MAINTAINING.md` step 3.
These listing checks supplement the full local baseline in §2; they do not execute the suite
or replace release verification.

**Instruments and censuses.** Source-based enforcement is code and is tested like code. A census of
Rust structure blanks comments and string, character, byte-string and raw-string literals before
counting, preserves positions and length, and proves the blanker on a fixture with removable
regions. Every census asserts the size and boundaries of the domain it claims and carries a positive
control that injects one violation and sees the expected failure; a control inside a truncated
domain proves nothing about the whole. Test-only items follow every production item in a file,
because a mid-file `#[cfg(test)]` truncates any instrument that treats the first test item as the
end of production. After inserting an item, check that neighbouring doc comments and attributes
still attach where intended. Fixtures derive their field lists from the production type and vary
each field independently. Name each test as the sentence it proves.

Test code may trade abstraction for clarity but does not weaken production invariants by reaching
through private state when the public behaviour can be exercised.

Enforced by: `cargo test --all-targets --all-features` on three platforms; the native category
inventory checks and effects oracles; each instrument's own positive control; review for
sufficiency, readiness and flake records.
