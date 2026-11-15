---
id: PR376-R7-WINDOWS-ARGV-BOUND
severity: P3
disposition: deferred
category: correctness
pr: 376
reviewed_sha: d5b1036f097043744ae817c16a48676c6faf0d11
location: src/effects/tests.rs:6132
provenance: introduced_by_feature
first_bad:
guard: the change that bounds the fixture compilers' argument list under the per-unit layout, by an argument file the compiler expands or by passing only the directories the chosen library needs, with a test that a build directory of several hundred unit directories still compiles a fixture
---

# A per-unit build directory can push the fixture compilers past Windows' command-line limit

## Failure sequence

Round seven's frontier review reasoned this sequence; it was not run, because the build machine
has no Windows host.

    Cargo 1.100.0's per-unit layout
    -> `crate_under_test` (`src/effects/tests.rs`, near line 6106) passes one
       `-L dependency=<build>/<package>/<hash>/out` for every unit directory in the build
       directory (`unit_out_dirs`, used near line 6132): about 87 characters each under a GitHub
       Windows runner's `D:\a\upstroke\upstroke\target`, about 100 on a typical developer path
    -> the two effects fixture compilers, `unresolved_paths` and `lint_fixture`, also pass about
       60 `--extern name=<rlib>` pairs, roughly 8,000 to 9,000 characters, however many units
       there are
    -> they cross Windows' 32,767-character command-line limit at about 280 unit directories on
       the runner's path and about 235 on a developer's; `typecheck` and
       `compile_against_this_crate` at roughly 320
    -> Cargo never prunes superseded units, and on stable every unit hash includes the full
       `rustc -vV`, so each toolchain update adds a second set: the build machine's slot 1 held
       about 95 unit `out` directories after one test build and one plain build, and about 150
       after `cargo clippy --all-targets` in the same directory, since check-mode units get their
       own `out/`
    -> one stable update without `cargo clean` puts a Windows developer's directory past the limit
    -> `Command::output()` fails with OS error 206, the `expect("clippy-driver runs; …")` in
       `unresolved_paths` and `lint_fixture` panics, and the five effects fixture tests fail

The old lookup was bounded: one `-L` directory, and `--extern` deduplicated by crate name.

CI is not exposed today. The hosted test jobs run on Linux and macOS, `lint (windows)` runs no
tests, the Windows guest is rebuilt for every job and pinned to 1.97.1, and the release workflow's
Windows test builds about 100 units. A guest image that kept a pre-warmed target directory across
a toolchain move past 1.100 would be exposed.

## What the change that takes this up should do

Bound the argument list, by one of the two remedies the review names:

- Pass the arguments in an `@argfile`. `rustc` expands these; check that `clippy-driver` forwards
  an `@argfile` to the compiler before relying on it, since `unresolved_paths` and `lint_fixture`
  run `clippy-driver`.
- Or pass only the `-L` directories the chosen library needs, rather than every unit's `out`
  directory in the build directory.

Either way, add a test that a build directory of several hundred unit directories still compiles
a fixture.
