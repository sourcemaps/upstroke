# `src/catalog.rs`

Extended notes for [`src/catalog.rs`](../../src/catalog.rs).

[Source on GitHub](https://github.com/sourcemaps/upstroke/blob/master/src/catalog.rs).

The code defines current behavior. These notes preserve contracts and implementation
history. Search each backticked heading fragment separately in the source.

## Module

Static capability catalog (DESIGN.md §13): model → tier classification.

This is point-in-time data shipped with the binary — the no-HTTP invariant
holds, so it can only be updated by releasing. Unknown models are never
auto-selected; a pin naming one is a hard error.

## `#![forbid(`

The three governed lints are `forbid` here since #318's third round: this file
stated no level for any of them and inherited none, so each took its level
from `-D warnings` alone, which an inner `allow` the placement scan does not
read -- macro-written, or spelled apart -- lowers; #318's second MAIN review
executed exactly that in `src/plan/mod.rs` and reached `std::fs::write` from
a production topology body while clippy and every governance test passed
(`GUARD-DECISION-SILENT-PRODUCTION-FILES-OUTSIDE-THE-ROLL-CALL`). A leaf with no children (`controls/C1-unfence-catalog-single-*`).
`forbid`, not `deny`, because it compiles: nothing in the file allows a governed lint, and clippy over all targets exits 0 with the fence in place. A downgrade beneath
a `forbid` is `E0453` however it is written or generated, and the enforcement
is the lint gate's -- rustc resolves no `clippy::` lint, so `cargo build` and
`cargo test` compile what clippy refuses. `effects::tests::every_unclassified_production_file_states_each_governed_lint_or_inherits_its_forbid`
names this file the day the fence is removed, and
`every_fence_of_a_governed_lint_forbids_wherever_forbid_would_compile` the
day it drops to `deny`.

## `pub enum Family {`

Which lab trained the model, independent of which CLI serves it.

§11.3's cross-vendor second opinion turns on this and **not** on the agent
id: Copilot serves Anthropic models too, so `claude-code/claude-opus-5` and
`copilot/claude-opus-5` are a harness switch that shares every blind spot.
"Different families share fewer blind spots" is the whole claim, so family
is what the binder compares.

Assigned explicitly per entry rather than inferred from the model name. This
is hand-maintained data, and a prefix heuristic that silently misclassifies
one rename would quietly pair a reviewer with its own family — a verification
property failing without a symptom.

## `const EXAMPLE_SMALL: CatalogEntry = entry(`

Preview default per tier when no pin applies (also listed in [`CATALOG`]).

## `pub const CATALOG: &[CatalogEntry] = &[`

Snapshot of the Claude Code, Copilot and Codex CLI rosters as of Aug 2026,
plus the two exact frontier slugs added in Nov 2026 for the self-host
binding (`claude-opus-5-5`, `gpt-6-astra`).

**Table order is preference order.** [`different_family_at`] returns the
first match, so moving an entry up promotes it for cross-family review.
Other readers follow the same order: the topology repair ladder's
`catalogued_model` takes an agent's first entry at its floor, and the
Codex probe checks the Codex entries in this order.

The Copilot half was checked against GitHub's supported-models reference on
2026-08-09; plain `gpt-5` and `gemini-2.5-pro` had already left the roster
and were replaced (see the entries). Slugs churn faster than releases do, so
treat every name here as point-in-time: `upstroke connect` cross-checks the
roster against what the installed CLI advertises wherever that CLI can
actually enumerate models. Claude Code and Copilot cannot today; Codex's
local `debug models` catalog is validated during its probe.

## `EXAMPLE_FRONTIER,`

Keep the default frontier model ahead of retained older slugs: this
order is the binding preference used by `different_family_at`, not just
display order. Exact model names keep frozen runs reproducible; the
moving `opus` alias must not decide which model a run meant after the
fact.

## `"claude-opus-5-5",`

Registered for explicit pins. The self-host binding needs nothing more,
because its worker is pinned. Two separate things keep every default where
it was:

- **The constant.** `EXAMPLE_FRONTIER` is untouched. `example_binding(Frontier)`
  returns it directly, so the unpinned preview, the unpinned worker and the
  unpinned review primary are still `claude-opus-5`. Table order has no say
  in that.
- **The row's place.** This row sits behind Opus 5, so the picks that follow
  table order are unchanged as well. `different_family_at(Frontier, OpenAI, _)`
  is the Anthropic partner of every OpenAI-family review primary, including
  this repository's own `codex/gpt-5.6-sol` pin. That pick and the repair
  ladder's `claude-code` frontier pick both still resolve to `claude-opus-5`.

Moving this row ahead of Opus 5 would move those two picks and leave the
preview where it is. Changing the default means changing `EXAMPLE_FRONTIER`.

## `entry("copilot", "gemini-3.1-pro", Tier::Mid, Family::Google),`

Slug pattern-derived, not verbatim: GitHub's reference lists the model as
"Gemini 3.1 Pro" and writes dots inside versions elsewhere
(`claude-sonnet-4.6`), but no example spells this one out. Lower
confidence than the entries around it — the `connect` cross-check is what
is meant to catch it once a CLI can enumerate models.

## `entry("codex", "gpt-6-astra", Tier::Frontier, Family::OpenAI),`

The exact slug the self-host review binding names. It was registered on
request and not read from a CLI by the change that added it. The Codex probe
refuses a CLI whose local `debug models` catalog lacks it, or lacks any of
the five efforts for it, before anything is spent.

It sits ahead of `copilot/gpt-5.3-codex` because that is the only position
that makes it the partner. Every public caller of `different_family_at`
(`validate`, `run` and the sequential pre-flight) passes the build's
adapter registry, which ships all three agents. No public caller passes
the pools file or the CLIs that are installed. Below Copilot, an Opus
frontier primary would still be partnered with `copilot/gpt-5.3-codex`.

What moved, on a shipped build:

- **Partner pick.** The frontier partner of every Anthropic-family review
  primary changed from `copilot/gpt-5.3-codex` to `codex/gpt-6-astra`. That
  covers the default `claude-opus-5` and the self-review alternative and
  second opinion of every Opus pin.
- **Pre-flight probe.** A schema-4 run now probes `codex` for that
  alternative instead of `copilot`, and it refuses the run when the probe
  fails. That is unchanged behaviour, now about a different CLI.
- **Codex entry order.** It now leads the Codex entries, so the repair
  ladder's `codex` pick at frontier is this slug, not `gpt-5.6-sol`.

What did not move: a build without the Codex adapter still pairs with
`copilot/gpt-5.3-codex`, OpenAI-family primaries still pair with
`claude-opus-5`, and every pick at mid and small is unchanged. Review plans
already recorded in a run's log replay as they were recorded.

## `entry("copilot", "gpt-5.3-codex", Tier::Frontier, Family::OpenAI),`

Verbatim from GitHub's CLI programmatic reference, where it appears as a
`--model` example — the highest exact-slug confidence available without
enumeration. It is load-bearing wherever the build has no Codex adapter:
`different_family_at` picks it for every cross-vendor second opinion at
frontier there (and did on every build until `gpt-6-astra` was placed
ahead of it), so a wrong name here fails §11.3 reviews at runtime on
exactly the paths blast radius protects.

## `entry("codex", "gpt-5.6-sol", Tier::Frontier, Family::OpenAI),`

Read from a live session's own rollout on 2026-08-11 rather than from a
docs page — the highest confidence any entry in this table has, since it
is the string the CLI recorded for a turn it actually ran. Frontier
because that is what it is and what it is for: the whole point of this
pool is implementing at the top rung without the reviewer's window
paying for it (§23.2, as scoped there).

## `pub fn example_binding(tier: Tier) -> CatalogEntry {`

Catalog-derived example binding shown by the binder preview.

## `pub fn different_family_at(`

The model §11.3's cross-vendor second opinion binds to: same tier, a
different training family, and an agent this build can actually drive.

First match in [`CATALOG`] order wins, so the choice is deterministic and
the table is where the preference is expressed. `has_adapter` is injected
rather than read from the registry so the engine can ask about the adapters
its own harness holds — which, under test, is not the built-in set. In the
shipped binary it is the built-in set: neither the pools file nor which
CLIs are installed narrows the choice. An uninstalled partner surfaces later,
as a probe failure.

`None` means this build cannot second-opinion that tier at all. Callers
decide what that costs: a configured `second_opinion` refuses the run, while
the implicit anti-self-review rebind settles for a warning.

## `pub fn missing_from(agent: &str, advertised: &[String]) -> Vec<&'static str> {`

Catalog entries for `agent` that the installed CLI does not advertise.

The guard the step-9 review asked for, and the reason `Discovery.models`
exists: this table is hand-maintained point-in-time data, and two of its
entries had already gone stale by Aug 2026 — including the one
[`different_family_at`] then picked for every frontier second opinion, where a
wrong slug fails §11.3 review at runtime on exactly the blast-radius paths
the second opinion exists to protect.

**`advertised` is empty on both adapters today** (neither CLI enumerates
models non-interactively), and an empty list means *the CLI said nothing*,
not *the CLI has no models* — so it returns nothing rather than condemning
the whole roster. The check fires when a future CLI grows enumeration;
`Caps::model_list` is what gates asking at all.

## `fn missing_from` › `let overlap = CATALOG`

Zero overlap is a format mismatch, not a stale catalog. GitHub's own
reference writes display names ("GPT-5.3-Codex", "Gemini 3.1 Pro") beside
the slugs `--model` takes, so a listing command that prints the former
would make every entry look missing — and a guard that names the entire
roster on its first real firing, advising an upgrade that cannot help, is
worse than no guard. One match is enough to trust the comparison.

## `fn normalize(model: &str) -> String {`

Case and separators folded away, so `GPT-5.3-Codex`, `gpt-5.3-codex` and
`GPT 5.3 Codex` all compare equal. Slugs are the contract, but a listing
meant for humans is not obliged to use them.

## `fn a_cli_that_enumerates_models_is_cross_checked_against_the_catalog` › `assert!(missing_from("copilot", &[]).is_empty());`

Silence is not disagreement: a CLI that advertises nothing must not
read as one that has repudiated the whole roster.

## `fn a_cli_that_enumerates_models_is_cross_checked_against_the_catalog` › `assert!(!missing.contains(&"claude-haiku-4-5"));`

Scoped to the agent asked about — Claude Code's roster is not
evidence about Copilot's.

## `fn a_second_opinion_crosses_families_not_merely_vendors()` › `let picked = different_family_at(Tier::Frontier, Family::Anthropic, everything)`

The trap this exists to avoid: `copilot/claude-opus-5` is a different
AGENT at the same tier, so an agent-id comparison would happily pick
it — and the "different families share fewer blind spots" claim
(§11.3) would be silently untrue.

## `fn a_second_opinion_crosses_families_not_merely_vendors()` › `let without_codex = |agent: &str| agent != "codex";`

The pick every build made before `gpt-6-astra` was registered, and the one
a build without the Codex adapter still makes: the promotion moved the
partner only where Codex can be driven.

## `fn a_second_opinion_crosses_families_not_merely_vendors()` › `let picked = different_family_at(Tier::Frontier, Family::OpenAI, everything)`

Asked from the other side, it comes back to Anthropic.

## `fn the_requested_frontier_pair_is_registered_exactly_and_nothing_near_it_is()` › `for (agent, model) in [`

Exact slugs only. No dotted spelling, no moving alias, and no other agent
claims either name. A pin that misses by one character must stay a hard
error, never a near match.

## `fn the_requested_frontier_pair_is_registered_exactly_and_nothing_near_it_is()` › `let claude_and_codex = |agent: &str| matches!(agent, "claude-code" | "codex");`

The restricted build is a control. The `everything` assertion after it is
the claim the shipped binary depends on, since its registry also carries
Copilot.

## `fn a_tier_with_no_reachable_other_family_resolves_to_nothing` › `let claude_only = |agent: &str| agent == "claude-code";`

The single-vendor install: only claude-code has an adapter, and every
claude-code entry is Anthropic. Callers must handle this rather than
quietly pairing a model with itself.

## `fn every_entry_declares_a_family_consistent_with_its_name()` › `for e in CATALOG {`

Families are assigned by hand, so this is the guard against a typo in
the table rather than a naming rule the code relies on anywhere.
