---
id: HOST-RUNNER-DEBUG-PRINTS-ENVIRONMENT
severity: P3
disposition: deferred
category: security-trust
pr: 441
reviewed_sha: d431d220f04a8fafd277b9bc55f4c113822167be
location: src/runner/host.rs:153
provenance: pre_existing
first_bad:
guard: the change that makes the Debug form of HostEnvironment, and so of HostRunner, print no inherited value
---

# A host runner's Debug form prints every inherited name and value

## Failure sequence

1. A `HostRunner` built by `new` or `for_legacy_workspace` holds the whole process environment as
   its base (`src/runner/host.rs:165-187`; `src/runner/host/environment.rs:347-353`).
2. Its `Debug` form writes the field `environment` (`src/runner/host.rs:148-156`, the field at
   `:153`), and `HostEnvironment`'s derived `Debug` writes its `base`, a list of every inherited
   name and value (`src/runner/host/environment.rs:338-343`).
3. So formatting a runner with `{:?}` prints each credential the process inherited, in the clear.

**How each step is known.**

- **Read:** each step, from the code at `d431d220`, not run.
- **Reported:** #441's round-four frontier pass, on #441's head `23970c2d` and not on `master`,
  wrote "I formatted a HostRunner constructed with a supplied synthetic base and checked in memory
  that the formatted result contained the marker in cleartext."
  (`~/selfhost-capacity-planner-20261126/evidence/review-frontier-23970c2.md:45`, SHA-256
  `fa5aade84cfa31e139ecee26b9d5238ac722def6a8f2ac1c938bf69e4d97dc61`).
- **Not run:** formatting a runner on `master`.
- **Found by** the design note on the capacity reader's credential boundary of 2026-11-28
  (`~/selfhost-design-20261127/reader-credentials.md`, SHA-256
  `2e792ee4a70262a567c414c8769a5a481e89a41615dfdd7fe9f3aa9856d5cd56`), which ran nothing, and read
  again at `d431d220` for this record.

**Latent.** A search of every Rust source file under `src/` at `d431d220`
for a `Debug` format (`{:?}`, `{:#?}`, a named or positional `{x:?}`, `dbg!`, a manual `Debug`
impl's `.field("…", …)`, a tracing-style `?x`) found 107 sites (the search,
`~/selfhost-findings-20261128b/scripts/debug-search.py`, SHA-256
`bd530712c9accd35d9b416848621159d016ea5447f6178ec79f90b7986a08385`; its output,
`~/selfhost-findings-20261128b/evidence/debug-search-d431d220.txt`, SHA-256
`da3d1758f175b27dbcfe725d4513ed4e0e115e85084be30771f12ff2540fa035`). As test-only it leaves out an
item that carries the literal attribute `#[cfg(test)]`, the file that a `mod` line carrying it
declares, and every file below that file (the script's lines 27-55 and 115-156). That is not all
test-only code: among the 107 are `src/agent/proc.rs:819`, under `#[cfg(all(unix, test))]` (`:786`),
and `src/agent/proc/test_support/readiness.rs:36`, `:72`, `:173` and `:308`, in a file whose `mod`
line (`src/agent/proc.rs:10948`) carries no such attribute and stands inside the inline module that
`#[cfg(test)]` at `:10439` makes test-only. Nine name a runner, a request,
a spec, a command, an environment or a base in the statement: three are `HostRunner`'s own `Debug`
(`src/runner/host.rs:151-153`), which formats its `HostEnvironment` (`:153`), and six format a
runner kind, a runner policy, `PATH` components, an exit code or a residue element. No other site
formats a `HostRunner`, `HostEnvironment`, `RunnerRequest` or `CommandSpec` value. The search reads
names, not types: a value formatted through a containing type's derived `Debug`, or under another
name, is not covered. Production builds a `HostRunner` at seven places: at five it lends a local or
a temporary as `&dyn Runner` (`src/main.rs:293`, `src/capacity.rs:970`, `src/connect.rs:105`,
`src/engine/coordinator.rs:71`, `src/engine/resume.rs:61`), and at two, a run and its resume through
the topology engine, it owns one through an `Arc<dyn Runner>`
(`src/engine/topology/entry.rs:322-323`, `:651-652`, built at `:1055-1057` and held at `:842`).
`Runner` is a trait with no `Debug` bound (`src/runner/contract.rs:301`).

## Why P3 security-trust

Graded by ruling r114: it prints credentials, and it is latent (above).

## Related, not the same

- `COMMAND-SPEC-DEBUG-PRINTS-STDIN`
  (`findings/P3_security-trust_202611281622_a-command-specs-debug-form-prints-its-stdin-bytes.md`):
  a request's `Debug` form, which prints its stdin.
- `PR271-R4-CONFIG-KEY-CARRIES-A-CREDENTIAL`
  (`findings/P2_security-trust_202609120316_an-indexed-config-key-can-carry-a-credential-the-redaction-misses.md:18-21`):
  a fixture's diagnostic that prints a credential carried in a Git configuration key.

## What the change that takes this up should do

- The form prints no inherited value.
- A test formats a runner built from a supplied base with a marker, and finds the marker nowhere.
