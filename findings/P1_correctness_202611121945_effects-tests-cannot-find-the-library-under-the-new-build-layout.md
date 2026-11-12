---
id: CI-RUST-1-100-BUILD-LAYOUT-RLIB-LOOKUP
severity: P1
disposition: deferred     # open: the change below takes it up; it is deleted with a `fixed` row when that change merges
category: correctness
pr:                       # assigned when the pull request is created
reviewed_sha: 2b16cec2b9a4cd6f018bb0a285071a9e8946bcf9
location: src/effects/tests.rs:6090
provenance: pre_existing
first_bad: 7a83e698824ee516b71d31c6728b64149f5642a0  # brought in all three copies of the lookup's assumption; the failure itself came with Cargo 1.100.0 on the hosted runners, not with a commit
guard: the change on branch fix-P1/correctness_effects-tests-cannot-find-the-library-under-the-new-build-layout, which replaces the three lookups with one layout-aware crate_under_test in src/effects/tests.rs; witnessed by the six tests named below, red on Cargo 1.100.0 at 2b16cec2 and green on 1.100.0 and 1.97.1 after it
---

## Failure sequence

GitHub's hosted runners now install Rust stable 1.100.0 (released 2026-11-09; 1.99.1 before it).
CI's `test (ubuntu-latest)` and `test (macos-latest)` jobs install `stable` and run
`cargo test --all-targets --all-features`. On every pull request these tests panic with
`no libupstroke-*.rlib beside the test executable in …/target/debug/build/upstroke/<hash>/out`
(`src/effects/tests.rs:6114`, `src/events/log/tests.rs:3143`):

- `effects::tests::every_declared_effect_denial_refuses_for_the_reason_it_declares`
- `effects::tests::every_denied_path_this_host_can_resolve_does_resolve`
- `effects::tests::the_engine_facade_allows_no_governed_lint_and_refuses_both_escape_routes`
- `effects::tests::the_topology_root_re_denies_every_lint_the_engine_facade_allows`
- `events::log::tests::every_declared_build_refusal_fails_for_the_reason_it_declares`

The operator recorded this in CI runs 43282541123 (#366) and 43285998427 (#369) on 2026-11-12.
A sixth test has the same defect through a third copy of the lookup, `this_crates_rlib`
(`src/rundir/tests.rs:12176`), and panics with `this crate's rlib is beside its test binary`
(`:12191`): `rundir::tests::a_private_half_deletion_without_a_proof_does_not_compile_for_the_stated_reason`.
On the build box at `2b16cec2`, after `upstroke-build cargo +1.100.0 build --all-targets
--all-features`, `upstroke-build cargo +1.100.0 test --all-features --lib -- <the six>` reports
`0 passed; 6 failed`.

**The step that goes wrong.** Each helper reads `std::env::current_exe()`, takes its parent
directory, and assumes two things about it. First, that the newest `libupstroke-*.rlib` is in it.
Second, that it holds every dependency's artifacts, so it is the single
`-L dependency=<dir>` handed to `rustc` and `clippy-driver` when they compile fixtures against the
crate (`src/effects/tests.rs:3404`, `:5177`, `src/events/log/tests.rs:3169`,
`src/rundir/tests.rs:12217`). The effects call sites also scan that one directory for
`--extern <crate>=<rlib>` pairs (`extern_dependencies`, `src/effects/tests.rs:3428`). Both
assumptions hold for Cargo's `<profile>/deps` layout. Neither holds on 1.100.0.

**The layout on Cargo 1.100.0** (`cargo 1.100.0 (3acc79c0c 2026-11-05)`). It was established from
Cargo's own `--message-format=json` artifact records, from its verbose `rustc` command line, and
from the files on disk, under `upstroke-build` on slot1:

- **Every unit has its own directory** `<profile>/build/<package>/<unit hash>/`, with the unit's
  outputs in `out/` and its `fingerprint/` beside them. Nothing is written to `<profile>/deps`.
- **The library** (`--all-targets`):
  `debug/build/upstroke/73049cf421cb3c54/out/libupstroke-73049cf421cb3c54.rlib` and `.rmeta`.
  Cargo uplifts a copy to `debug/libupstroke.rlib`.
- **The lib test executable:**
  `debug/build/upstroke/e43c37f6c0538f1e/out/upstroke-e43c37f6c0538f1e`. That directory holds
  only the executable and its `.d`. The bin's test executable is in
  `debug/build/upstroke/7c1025d245a671fd/out/`, and the bin itself is uplifted to `debug/upstroke`.
- **Each dependency** is in its own unit directory: for example,
  `debug/build/serde_json/db986604bf93230c/out/libserde_json-db986604bf93230c.rlib` and `.rmeta`.
  Proc macros are there too, as in
  `debug/build/serde_derive/d0db13f022fd62a8/out/libserde_derive-d0db13f022fd62a8.so`.
- **Cargo's own `rustc` command for the library** passes 41
  `-L dependency=<profile>/build/<package>/<hash>/out` directories, one per transitive dependency
  unit. Each direct dependency goes by
  `--extern <name>=<profile>/build/<package>/<hash>/out/lib<name>-<hash>.rmeta`. A fixture that
  links the library needs all of those directories on its own `-L` list.
- **`cargo test --lib` alone** builds only the test unit. Its artifact records name only
  `upstroke-e43c37f6c0538f1e`, so it compiles no `libupstroke-*.rlib`. The rlib exists because
  `--all-targets` builds the library for the binaries, as CI's command does.
- **`cargo +1.100.0 -Z help` still lists `-Z build-dir-new-layout`** ("Use the new build-dir
  filesystem layout") as unstable. Stable 1.100.0 nevertheless writes the per-unit layout above.

So the test executable's directory holds no rlib, and the one `-L` directory it names holds none
of the dependencies. The lookup panics before any fixture is compiled. A lookup that found the
rlib would still fail to compile the fixtures: `rustc` could not resolve the crate's dependencies.
Cargo 1.97.1 (`stable` on the build box) keeps the old layout, and all six tests pass there.

## Severity: P1

It fails a required context, `upstroke-ci`, on every pull request, whatever the pull request
changes. While it stands nothing can merge. The tests it stops are the build-refusal and
lint-denial witnesses: until they run again, no lint fence or compile refusal is being checked.

## What the change that takes this up should do

Find the crate's rlib and the dependencies' directories from what is on disk, under both layouts,
without naming a toolchain version:

- **The `deps` layout:** an `libupstroke-*.rlib` beside the test executable means that directory
  holds everything, as before.
- **The per-unit layout:** the executable's directory is `<unit>/out` with `<unit>/fingerprint`
  beside it. The rlib is then the newest `<package>/*/out/libupstroke-*.rlib`. Every
  `<profile>/build/*/*/out` directory is a `-L dependency=` directory, and the effects call sites'
  `--extern` scan covers all of them.
- **When neither layout yields an rlib,** fail naming every place searched, and say that a
  `--lib`-only build compiles none.

Keep one helper for the three copies, and keep every test's assertions as they are. Witnesses:
the six tests above, red on 1.100.0 at `2b16cec2` and green on 1.100.0 and 1.97.1 after the
change. A mutation that restores the beside-the-executable lookup must turn them red on 1.100.0
and leave them green on 1.97.1.
