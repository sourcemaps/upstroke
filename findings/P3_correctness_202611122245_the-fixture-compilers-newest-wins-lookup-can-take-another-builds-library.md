---
id: PR376-R1-NEWEST-WINS
severity: P3
disposition: deferred
category: correctness
pr: 376
reviewed_sha: e62b49cce5d09020fd0019b7b8ef471546d0a8a4
location: src/effects/tests.rs:6184
provenance: pre_existing
first_bad: 7a83e698824ee516b71d31c6728b64149f5642a0
guard: the change that makes `crate_under_test` choose the library unit whose dependency fingerprints match the running test executable's, and fail, naming the candidates, only when the fingerprints cannot tell the library units apart, with a witness over a synthetic directory holding two library units
---

# The fixture compilers' newest-wins lookup can take another build's library

## Failure sequence

`crate_under_test` in `src/effects/tests.rs` (line 6108 at the reviewed commit) finds this crate's library for the
fixture-compiling tests. Under Cargo 1.100.0's per-unit layout it takes, through `newest_upstroke_rlib` (line 6184), the
newest `libupstroke-*.rlib` among the package's unit directories, `debug/build/upstroke/<hash>/out/`.

A build directory can hold two builds of the library. On the operator's build machine, build slot 1 held two 1.100.0
units, `build/upstroke/73049cf421cb3c54/` and `build/upstroke/3d5016eed0999272/`, whose fingerprints differ in
`profile` and in a dependency. Round six's frontier review read the slot's fingerprints and found that the units differ
in dev-dependency features: the `hashbrown` unit `24b95b784ae1890b` has the `default-hasher` feature, which only the
dev-dependency path `yaml-rust2` -> `hashlink` turns on, while `3d5016eed0999272` is built on the featureless
`hashbrown` unit `ee164fb1d6d90d92` (through `indexmap` `07911642e4f6f5ed`). A build with no test target, such as a
plain `cargo build` or `cargo run`, next to a test build produces that split; `build --all-targets` and
`test --all-targets` both build test targets, so both turn dev-dependency features on and cannot.

Reasoned, not run:

    checkout X runs `cargo test --all-targets` in a shared slot
    -> checkout Y then runs `cargo build` in the same slot
    -> X runs its tests again without changing its source, so its test unit is fresh and not rebuilt
    -> the lookup returns Y's newer library
    -> X's fixtures compile against Y's code

The old layout had the same rule (newest wins); the per-unit layout makes two library builds in one directory routine,
even between checkouts with identical dependency graphs. CI builds once per job and is not affected.

A related open finding, `PR262-WARM-BUILD-SLOT-SERVES-ANOTHER-WORKTREE`, concerns the build machine's slot tool outside
this repository; this one is the repository's own lookup.

## What the change that takes this up should do

Choose the library unit whose dependency fingerprints match the running test executable's. Two library units in one
build directory are an ordinary state, so do not fail merely because more than one is present; fail, naming the
candidates, only when the fingerprints cannot tell them apart. The test executable is compiled with `--test` from the crate's source and links no
library rlib, so neither its fingerprint nor its `.d` file names one. Keep the lookup working under both layouts. Add
a witness over a synthetic directory holding two library units.
