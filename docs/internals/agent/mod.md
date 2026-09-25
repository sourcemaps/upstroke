# `src/agent/mod.rs`

Extended notes for [`src/agent/mod.rs`](../../../src/agent/mod.rs).

[Source on GitHub](https://github.com/sourcemaps/upstroke/blob/master/src/agent/mod.rs).

The code defines current behavior. These notes preserve contracts and implementation
history. Search each backticked heading fragment separately in the source.

References below to `decisions.*` and `INV-18` use retired v0.2 planning identifiers.
They record implementation history and do not add current requirements.
[DESIGN.md](https://github.com/sourcemaps/upstroke/blob/master/DESIGN.md#retired-records)
is the living design authority.

## Module

Agent adapters (DESIGN.md §8, §16): turn a `TaskRun` into a subprocess of
an official agent CLI and parse what came back. Adapters never edit files,
never commit, and never speak HTTP — they only build commands and read
process output. One file per agent.

## `#![deny(`

The three governed lints are `deny` here, and `deny` is the strongest level
that compiles: this module's children allow governed lints at file level in
production code (recorded rows of `effects/allowlist.toml`), and a `forbid`
above an `allow` is `E0453` in every build. A `deny` is a level an inner
`allow` lowers, so since #318's third round this file holds **nothing but
declarations and re-exports** -- every item it used to hold lives in
[`adapter`](adapter.md), which can `forbid` -- and
`effects::tests::a_declaring_module_holds_declarations_and_re_exports_and_nothing_else`
holds it to that shape (`DECLARATION_ONLY_MODULES`): a function, a constant,
an inline module, `include!` or a macro written here is a red test
(`~/orch-pr10/repair-318-r3-evidence/plan-class/M4d-*`). A file that holds no
code hosts no wrapper, which is what closed
`GUARD-DECISION-SILENT-PRODUCTION-FILES-OUTSIDE-THE-ROLL-CALL` for this file.
