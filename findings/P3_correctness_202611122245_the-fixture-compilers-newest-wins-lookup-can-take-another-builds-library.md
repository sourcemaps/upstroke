---
id: PR376-R1-NEWEST-WINS
severity: P3
disposition: deferred
category: correctness
pr: 376
reviewed_sha: e62b49cce5d09020fd0019b7b8ef471546d0a8a4
location: src/effects/tests.rs:6184
provenance: pre_existing
first_bad:
guard: the change that makes `crate_under_test` choose the library unit built for the running test executable rather than the newest one, for instance by reading the unit's fingerprint or the dependency file beside the executable, with a witness of a build slot holding two builds of the library
---

# The fixture compilers' newest-wins lookup can take another build's library

## Failure sequence

`crate_under_test` in `src/effects/tests.rs` (line 6108 at the reviewed commit) finds this crate's library for the
fixture-compiling tests. Under Cargo 1.100.0's per-unit layout it takes, through `newest_upstroke_rlib` (line 6184), the
newest `libupstroke-*.rlib` among the package's unit directories, `debug/build/upstroke/<hash>/out/`.

A build directory can hold two builds of the library. On the operator's build machine, build slot 1 held two 1.100.0
units, `build/upstroke/73049cf421cb3c54/` and `build/upstroke/3d5016eed0999272/`, whose fingerprints differ in
`profile` and in a dependency, as the outputs of a `build --all-targets` and a `test --all-targets` would.

Reasoned, not run:

    checkout X runs `cargo test --all-targets` in a shared slot
    -> checkout Y then runs `cargo build --all-targets` in the same slot
    -> X runs its tests again without changing its source, so its test unit is fresh and not rebuilt
    -> the lookup returns Y's newer library
    -> X's fixtures compile against Y's code

The old layout had the same rule (newest wins); the per-unit layout makes two library builds in one directory routine,
even between checkouts with identical dependency graphs. CI builds once per job and is not affected.

A related open finding, `PR262-WARM-BUILD-SLOT-SERVES-ANOTHER-WORKTREE`, concerns the build machine's slot tool outside
this repository; this one is the repository's own lookup.

## What the change that takes this up should do

Choose the library unit built for the running test executable (its fingerprint or `.d` file names the library it
linked), or fail when more than one candidate exists. Keep the lookup working under both layouts. Add a witness over a
synthetic directory holding two library units.
