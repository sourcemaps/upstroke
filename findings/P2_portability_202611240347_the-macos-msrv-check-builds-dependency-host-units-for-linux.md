---
id: PR412-MACOS-DEPENDENCY-HOST-MSRV-GAP
severity: P2
disposition: deferred
category: portability
pr: 412
reviewed_sha: 7163640a8579474032fb498fa2b78cf82512902e
location: .github/workflows/ci.yml:623
provenance: introduced_by_feature
first_bad: a1106bf26158818f5b7bee50990700841886cd40
guard: the first change to Cargo.lock, to a dependency table in Cargo.toml or to the MSRV carries the remedy or follows it; fix-P2/portability_the-macos-msrv-check-builds-dependency-host-units-for-linux (ruling r49)
---

# The macOS MSRV check builds dependency host units for Linux

Found by the frontier review's round three over #412, and filed deferred under ruling r49.

## Failure sequence

    a locked build-script or procedural-macro dependency uses macOS-only host code that needs a Rust newer
      than 1.85
    -> the macOS MSRV entry runs Cargo 1.85 on `ubuntu-latest` with `--target aarch64-apple-darwin`
      (`.github/workflows/ci.yml:623-632` at the reviewed commit: the matrix entry `os: ubuntu-latest`,
      `target: aarch64-apple-darwin`, and the step
      `cargo check --locked --target ${{ matrix.target }} --all-targets --all-features`); under `--target`,
      Cargo builds build scripts, procedural macros and their dependencies for the host, so that unit is
      compiled for Linux and its macOS branch is never checked at 1.85
    -> the native macOS test legs compile it only with stable Rust
    -> every required context is green while a macOS user building the locked release with Rust 1.85 fails

## What is established

Ruling r49's findings, read and not run:

- `git diff ead85451 7163640a -- Cargo.lock Cargo.toml` is empty, and `master`'s native
  `msrv (Rust 1.85, macos-latest)` passed on those files at `ead85451` (run 45310681323). So today's lockfile
  is proven on a Mac at 1.85.0.
- The package has no host unit of its own, and a test refuses one
  (`no_repository_file_adds_a_host_unit_the_macos_cross_checks_cannot_reach` in `src/effects/tests.rs`).
- So the gap opens only at the next dependency change. The last lockfile change was `6140c4a8` on
  2026-09-30, and #408, #409 and #411 make none at their fetched heads.
- Until the remedy, CI no longer enforces `standards/02_standards_automated_baseline.md:28` ("The `--locked`
  MSRV check is what proves the floor against the dependency set a release ships.") for the next lockfile.
- Unwitnessed: the review cites a `cargo metadata` count (eleven dependency build scripts and three
  proc-macro targets at the reviewed commit) and no test.

## What the change that takes this up should do

- **Check dependency host units on a Mac at 1.85.0 again**, within five macOS jobs, with the workflow oracle
  requiring it: either a `cargo +1.85.0 check --locked --all-targets --all-features` step in one macOS test
  leg, or the native `msrv` entry back if the concurrent-job cap has risen.
- Failing both, the owner accepts the gap, and `standards/02_standards_automated_baseline.md:28`'s sentence
  and `standards/15_standards_dependencies_and_features.md:23`'s `Enforced by` line are narrowed to what CI
  proves.
- Its repair branch: `fix-P2/portability_the-macos-msrv-check-builds-dependency-host-units-for-linux`.
- Until it lands, the operator's ship gate stops any push whose diff from `master` touches `Cargo.lock` or
  `Cargo.toml` (ruling r49 step 8).
