---
id: GUARD-DECISION-SILENT-PRODUCTION-FILES-OUTSIDE-THE-ROLL-CALL
severity: P2
disposition: deferred
category: security-trust
pr: 
reviewed_sha: de6d64348b28fb1ba460ffb198584e132c8076c3
location: src/topology/mod.rs:1
provenance: pre_existing
first_bad:
guard: project owner — the post-v0.2 pass over PR3's layer, with `PR7-WRAPPERS-EMPTY-DOMAIN`, whose mechanism this is; the tree-wide form of the classified-module guard is proposed below and not built
---

## Failure sequence

**Measured at `de6d64348b28fb1ba460ffb198584e132c8076c3` by enumeration, not executed.** The mechanism is the one
Gate 5's fourth run executed in `src/capacity.rs` and `src/runner/invocation.rs`
(`G5RUN4-RESIDUAL-BYPASS-OUTSIDE-THE-Q5-CARVE-OUT`): a file whose prologue states no level for a governed lint takes
that lint's level from `-D warnings` alone, and an inner `allow` the placement scan does not read — written by a
macro, or with its tokens spelled apart — lowers it, so a `std::fs::write` wrapper hosted there and called from a
production body under `src/engine/topology/` passes clippy and every governance test. The fence change of 2026-09-21
(#312) closed that in the 54 files that `forbid`, #316 in two classified modules, and the guard filed with this finding
in every entry of `CLASSIFIED_MODULES`. **None of them looks at a file the roll-call does not name, and 50 such
files state nothing.**

The tree's own reader, `effects::lint_levels::file_level_lint_state`, over every file `scanned_sources()` walks
(`~/orch-pr10/guard-decision-evidence/H0-guard-at-de6d6434-unfenced.log`, a throwaway test run once and discarded):
**187** files; files whose prologue states none of the three used governed lints at file level: **1** classified
(`src/agent/bin.rs`, fenced with the guard), **73** under a `TOPOLOGY_MODULES` prefix and not classified, **17**
neither. A lint level is scoped by the module tree, so a silent file under a fencing parent is covered by
inheritance: resolving each silent file up its module chain (`survey-inherit.py`, `survey-inherit-at-de6d6434.txt`;
`#[path]` is not modelled and the tree has none the walk admits), **39** of the 73 sit under
`src/engine/topology.rs`'s file-level `forbid` and **1** of the 17 under `src/engine/report.rs`'s. **The other 50
inherit nothing**: every ancestor up to `src/lib.rs` is silent too, and `src/lib.rs` states no governed lint. `Cargo.toml`'s
`[lints]` names none of the three either, so in each of these files the level is the command line's `-D warnings`
and nothing more.

- **34 under a topology prefix**, every one a file `the_legacy_section_never_contains_a_topology_module` forbids from
  ever carrying an allowance: `src/topology/mod.rs` and the 31 files beneath it (`census.rs`, `effects.rs` and its
  eight children including `effects/tests.rs`, `events.rs`, `fold.rs` and its thirteen children including the three
  under `fold/tests/`, `leases.rs`, `paths.rs`, `queue.rs`, `registry.rs`, `schema.rs`), plus `src/runner/mod.rs`
  and `src/runner/policy.rs`.
- **16 elsewhere**: `src/lib.rs`, `src/effects.rs` (a funnel row with `allows = []`, "carries no attribute"),
  `src/agent/mod.rs`, `src/plan/mod.rs`, `src/plan/markdown.rs` and its five children (`annotation.rs`,
  `assemble.rs`, `drafts.rs`, `hints.rs`, `sections.rs`), `src/catalog.rs`, `src/error.rs`, `src/ir.rs`,
  `src/ladder.rs`, `src/observations.rs`, `src/ulid.rs`.

Three of the 16 are declaring modules that hold no code
(`a_declaring_module_holds_declarations_and_re_exports_and_nothing_else` covers `src/engine/mod.rs`; whether it covers
`src/lib.rs`, `src/agent/mod.rs`, `src/plan/mod.rs` and `src/topology/mod.rs` is that test's domain to state), and a
module that holds no code hosts no wrapper. The rest hold production code, and a production `fn` anywhere in the crate
is a name a topology body can reference. Where the file is also outside `CLASSIFIED_MODULES` — every one of the 50 —
the classification census does not read it either, so unlike `src/capacity.rs` at `9bb177ea` a wrapper there needs no
macro to escape a literal-name census: nothing asks for its name at all. That half is
`W1-CLASSIFIED-MODULES-IS-A-HAND-MAINTAINED-ROLL-CALL`'s and is not re-filed here; what this file records is the lint
level.

`PR7-WRAPPERS-EMPTY-DOMAIN`'s sentence that the route is open *"in the 45 files that carry an allowance of a governed
lint, for the lint each allows, and in the five files that still fence with `deny`"* under-states by these 50 files,
in each of which it is open for all three lints. Nothing in the tree is wrong *today* by this route: no such wrapper
exists. What is filed is that after the roll-call guard the class is bounded for the 54 files the roll-call names and
for no other, and that the boundary is the roll-call, which is hand-maintained.

`location` is `src/topology/mod.rs:1`, the one prologue whose statement would reach 32 of the 50.

## What the change that takes this up should do

Owner, as the ledger records it: project owner — the post-v0.2 pass over PR3's layer, the pass that owns
`PR7-WRAPPERS-EMPTY-DOMAIN`. Two things, in this order:

1. **Root fences, measured by compiling.** `#![forbid(clippy::disallowed_methods, clippy::disallowed_types,
   clippy::disallowed_macros)]` in `src/topology/mod.rs` reaches all 32 files under it; no allowance exists below it
   (the legacy section may not hold a topology module and the funnel section holds `src/topology/effects.rs` with
   `allows = []`), so `E0453` cannot arise, and a lint hit would already fail `-D warnings` — reasoned, so compile it.
   The same in `src/plan/mod.rs` reaches its seven; `src/runner/policy.rs`, `src/catalog.rs`, `src/error.rs`,
   `src/ir.rs`, `src/ladder.rs`, `src/observations.rs` and `src/ulid.rs` each take their own. `src/lib.rs`,
   `src/agent/mod.rs`, `src/runner/mod.rs` and `src/effects.rs` have allowances beneath them (`src/agent/claude.rs`,
   `src/runner/host.rs`, `src/effects/tests.rs`, and through `lib.rs` every allowing file), so `forbid` is `E0453`
   there and `deny` is what they can state; three of the four hold no code, and `src/effects.rs` then joins the
   `deny`-excused set of `PR7-WRAPPERS-EMPTY-DOMAIN`.
2. **The tree-wide form of the guard.** Once the 50 state a level,
   `every_classified_module_carries_a_file_level_fence_or_allowance_of_a_governed_lint` should read every file
   `scanned_sources()` walks rather than `CLASSIFIED_MODULES`, with an inherited `forbid` counting for a child — the
   module walk `census_domain` already has — so that the boundary is the tree and not the roll-call. That subsumes
   the roll-call guard; delete it then, and delete this file.
