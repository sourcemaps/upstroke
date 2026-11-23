---
id: PR405-HARNESSES-LACK-CARGOS-RUNTIME-ENVIRONMENT
severity: P2
disposition: deferred
category: portability
pr: 405
reviewed_sha: 1760a68c8a7712946d6c807bfce1aad3d2bc15f6
location: .github/workflows/ci.yml:546
provenance: introduced_by_feature
first_bad:
guard: the first test that needs, on a Windows leg, what Cargo sets at run time; or the next change to how the step starts a harness
---

# A harness the Windows step starts lacks Cargo's run-time environment

Found by the frontier review's round two over #405 (finding 2), and filed deferred under ruling r44.

## Failure sequence

    a test reads, at run time, a `CARGO_*` variable Cargo sets for a test it runs, a key of a Cargo config's `[env]` table, or a directory Cargo adds to `PATH`
    -> on the Linux and macOS legs, and on a user's machine, `cargo test` starts the harness with that environment, and the test passes
    -> on the ten Windows legs the suite step builds with `cargo test --no-run` and starts each harness itself with `Start-Process`, in the step's own environment (the loop, `.github/workflows/ci.yml:546` at the reviewed commit)
    -> the variable is unset, the `[env]` key absent, the directory not on `PATH`
    -> the test fails on Windows alone, on a tree that passes on every other leg

## What is established

- Today nothing reads what is missing:
  - `src/effects/tests.rs:51` reads `CARGO` at run time and falls back to `cargo` when it is unset; on the Windows
    legs that is the `cargo` on `PATH`, which the step's first two lines hold to the pinned 1.97.1.
  - `CARGO_TARGET_DIR` is not Cargo's to set: it is an input Cargo reads, not a variable it gives a test.
  - Every use of `CARGO_MANIFEST_DIR` is through `env!`, read when the test is compiled.
- Two ten-leg runs were green: 45045067738 and 45090409001.
- A later runtime read fails red, on the pull request that adds it: the Windows legs run on every pull request, so
  the read does not pass unseen.

## What the change that takes this up should do

- **Read it at compile time**, through `env!`, where Cargo sets it for the compile as well; the value is then in the
  harness, whoever starts it.
- **Or set that one variable in the step**, and pin it in the workflow oracle (`src/effects/tests/ci_model.rs`),
  so that a change to it is a change the oracle refuses until its pin moves too.
- **Never the whole environment.** Each variable the step adds is one the oracle pins by name; a copy of Cargo's
  environment would add ones nothing pins.
