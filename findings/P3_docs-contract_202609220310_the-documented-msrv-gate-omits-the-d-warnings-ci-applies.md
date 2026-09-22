---
id: PR313-DOCUMENTED-MSRV-GATE-OMITS-THE-D-WARNINGS-CI-APPLIES
severity: P3
disposition: deferred
category: docs-contract
pr: 313
reviewed_sha: cb1fa1f68852af11f365d61a59f9fecd947bd131
location: CLAUDE.md:54
provenance: pre_existing
first_bad:
guard: the change that next edits the gate list in `CLAUDE.md` and `AGENTS.md`, or the build wrapper that runs it
---

## Failure sequence

`.github/workflows/ci.yml` sets `RUSTFLAGS: -D warnings` for the whole workflow, so every leg
compiles with warnings denied -> the gate list in `CLAUDE.md` spells the MSRV command as
`cargo +1.85.0 check --locked --all-targets --all-features` with no `-D warnings`, and the build
box's wrapper runs it that way -> a warning the 1.85 toolchain emits and stable does not passes the
ten documented gates and fails CI's `msrv (Rust 1.85, *)` job on all three platforms -> measured on
this pull request: three `#[cfg_attr(not(test), expect(dead_code, ...))]` attributes, `dead_code`
firing for all three on stable and for one of them on 1.85, gave
`error: this lint expectation is unfulfilled` twice under `-D warnings` and `warning:` twice
without it, so the local baseline reported `ALL 9 PASS` and the three msrv jobs went red

## What the change that takes this up should do

Spell the MSRV command as CI runs it, which means `-D warnings` reaches it:
`RUSTFLAGS='-D warnings' cargo +1.85.0 check --locked --all-targets --all-features`, in the gate
list and in whatever wrapper runs the baseline. The same gap exists in principle for
`cargo test --all-targets --all-features`, whose CI leg is also `-D warnings` while the documented
command is not; on stable that one is covered in practice because
`cargo clippy --all-targets --all-features -- -D warnings` compiles the same targets with warnings
denied, and it is the *toolchain* difference — 1.85's `dead_code` analysis against stable's — that
the clippy leg cannot stand in for.

Measured control, so the fix is not taken on trust: with the `expect` spelling restored under
`RUSTFLAGS='-D warnings' cargo +1.85.0 check --locked --all-targets --all-features`, the build fails
with `could not compile upstroke (lib) due to 2 previous errors`, and with the `allow` spelling it
finishes; the same two commands without `RUSTFLAGS` both finish, which is the divergence.
