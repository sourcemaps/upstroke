---
id: PR366-CFG-SCAN-CODE-DECOY
severity: P3
disposition: deferred
category: correctness
pr: 366
reviewed_sha: 2f36c0fb1a8d0be9f48afa8935b98b9db84376a6
location: src/runner/container/exec/tests.rs:6489
provenance: introduced_by_feature
first_bad:
guard: the change that states in the notes that the scan reads the first code signature with the test's name, so an earlier cfg-disabled function of that name would satisfy it, without narrowing the scan again
---

# The Linux-gate scanner accepts an earlier disabled code decoy

## Failure sequence

From the review:

> an earlier cfg-disabled same-named function carries #[cfg(target_os = "linux")] ->
> attributes_of_test selects that first code signature -> the real descendant test is changed
> back to #[cfg(unix)] -> the source regression remains green while the real test is again
> enabled off Linux

`attributes_of_test` blanks comments and literals, then takes the first `\nfn <name>() {` it
finds and reads the attribute lines above it. Blanking stops a decoy in a comment or a raw
string. It does not stop a decoy in code. Say a function with the same name sits earlier in the
module, carries `#[cfg(target_os = "linux")]`, and is disabled by another `cfg` so the two never
collide. The scan stops at that one. The real
`real_docker_a_container_contains_a_daemonised_descendant` can then go back to `#[cfg(unix)]`.
`real_docker_a_container_descendant_test_is_gated_to_linux` still passes, and the real test runs
again on macOS, where it fails.

## How it was found

By reading the code, in a Codex frontier review of pull request #366. It was not reproduced.

## Why it is deferred

The owner ruled on 2026-11-14 that #366's repair rounds end after round ten, and this was found
after that. It is also unwitnessed. Nobody has added such a decoy, by accident or otherwise.

## What the change that takes this up should do

State the limit in the notes: the scan reads the first code signature with the test's name, so an
earlier cfg-disabled function of the same name would satisfy it.

Do not narrow the scan again. The last narrowing (`PR366-CFG-SCAN-LITERAL-SPOOF`) closed the
comment and literal decoys and left this one open, and a text scan will always have a next one. If
a compiler-checked gate is wanted, it replaces the scan instead of adding to it.
