# `src/agent/mod.rs`

Extended notes for [`src/agent/mod.rs`](../../../src/agent/mod.rs).

[Source on GitHub](https://github.com/sourcemaps/upstroke/blob/master/src/agent/mod.rs).

The code defines current behavior. These notes preserve contracts and implementation
history. Search each backticked heading fragment separately in the source.

`decisions.*` and `INV-18`, where this module tree cites them, are identifiers of the retired
v0.2 design packet, which is not in this repository; they record implementation history and add
no current requirement.
[DESIGN.md](https://github.com/sourcemaps/upstroke/blob/master/DESIGN.md#retired-records)
is the living design authority.

## Module

Agent adapters (DESIGN.md §8, §16): turn a `TaskRun` into a subprocess of
an official agent CLI and parse what came back. Adapters never commit and
never speak HTTP. Besides building commands and reading process output,
each adapter's `materialize_permissions` writes a JSON file (`util::write_json`): Claude Code's is the
settings file its command references; Copilot's and Codex's is an audit-only record, their permissions
being argv flags. One file per agent.

## `#![deny(`

The three governed lints are `deny` here, and `deny` is the strongest level that compiles: this
module's children allow governed lints at file level in production code (recorded rows of
`effects/allowlist.toml`), and a `forbid` above an `allow` is `E0453` in every build. A `deny` is a
level an inner `allow` lowers, so this file holds nothing but declarations and re-exports — the
items it used to hold live in [`adapter`](adapter.md), which can `forbid` — and
`effects::tests::a_declaring_module_holds_declarations_and_re_exports_and_nothing_else` holds it to
that shape (`DECLARATION_ONLY_MODULES`): it fails on any item `items_beyond_declarations` counts
here. A file that holds no code hosts no wrapper, which is what closed
`GUARD-DECISION-SILENT-PRODUCTION-FILES-OUTSIDE-THE-ROLL-CALL` for this file.
