# `src/runner/mod.rs`

Extended notes for [`src/runner/mod.rs`](../../../src/runner/mod.rs).

The code is the authority for what it does; this file is the whole of its prose, moved out of
the source verbatim. Each section is headed by the line of code the comment sat above, spelled
as it is in the source, so the heading is the grep string that finds the code.

## Module

The Runner seam (DESIGN.md §8, §20; INV-18, INV-20, INV-22, INV-23).

> **Runner** — Execute probes, workers, gates, and reviewers on the host or
> in a role-scoped container; owns cwd, mounts, environment, supervision,
> and timeout, never agent semantics or Git. (DESIGN.md:118)

An adapter builds a data-only [`CommandSpec`]; a [`Runner`] decides where
it executes. That split is the whole point of the layer: "adapters never
learn about containers, and the runner learns nothing about agent semantics
beyond which per-agent credential volume to mount" (DESIGN.md:612).

PR4 ships the host half. [`host::HostRunner`] implements the trait, resolves
the `host-v1` [`policy`] for the marker, the owner record and
`run_started(4).runner`, composes the base-plus-overlay environment, and
executes the `RunnerPreflight` shell probe. The container runner is PR6 and
an explicit non-goal here, as are the async surface and the slot broker.

#### Why `run` is synchronous and still shaped like the async one

`decisions.sequential_substrate.runner`: "Runner::run(&RunnerRequest) ->
ProcessOutput synchronous until PR11 (then a boxed Send future)".
DESIGN.md:250-256 says why the shape has to survive that change: every
async trait used behind `dyn` returns a boxed `Send` future, so the trait
must already be object-safe and its request must already be a single
borrowed value. It is, and [`Runner`] is `Send + Sync` so a `&dyn Runner`
can be held across the await points PR11 introduces.

## `#![deny(`

`disallowed_methods` and `disallowed_types` are `deny` here, and `deny` is the
strongest level that compiles for them: this module's children allow both at
file level in production code (recorded rows of `effects/allowlist.toml`), and
a `forbid` above an `allow` is `E0453` in every build. `disallowed_macros` is
`forbid` in the production build since 2026-09-26, when the two production
allowances of it below this file, in `host.rs` and `container.rs`, were dropped
as unused on every CI target; the whole-file test module `host/tests.rs` still
allows it, so the `forbid` is `cfg_attr(not(test), ..)`, the form #318 gave
`src/runner/container/census.rs`, and
`effects::tests::no_deny_of_a_governed_lint_is_excused_by_test_code_alone` is
the census that asks for it. A `deny` is a level an inner
`allow` lowers, so since #318's third round this file holds **nothing but
declarations and re-exports** -- every item it used to hold lives in
[`contract`](contract.md), which can `forbid` -- and
`effects::tests::a_declaring_module_holds_declarations_and_re_exports_and_nothing_else`
holds it to that shape (`DECLARATION_ONLY_MODULES`): a function, a constant,
an inline module, `include!` or a macro written here is a red test
(`~/orch-pr10/repair-318-r3-evidence/plan-class/M4d-*`). A file that holds no
code hosts no wrapper, which is what closed
`GUARD-DECISION-SILENT-PRODUCTION-FILES-OUTSIDE-THE-ROLL-CALL` for this file.
