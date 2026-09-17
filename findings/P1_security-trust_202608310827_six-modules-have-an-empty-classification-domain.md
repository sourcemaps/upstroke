---
id: PR7-WRAPPERS-EMPTY-DOMAIN
severity: P1
disposition: deferred
category: security-trust
pr: 7
reviewed_sha: beb16c924dfbdb5c395ac37b5671f4168b08cce6
location: src/engine/mod.rs:5
provenance: undetermined
first_bad:
guard: project owner — the post-v0.2 pass over PR3's layer
---

## Failure sequence

As filed on 2026-08-31: `effects::externally_reachable_fns` consulted the truncating `production_region`, so for `engine/{attempt,coordinator,resume}.rs` and three siblings cut at a `#[cfg(test)] use` the **classification domain was empty**, every externally reachable production function of those modules was unclassified, and a working bypass was demonstrated: a `pub(super) fn` below the cut, called from a live topology module, passed clippy and the whole suite. **That part is closed by #306** (three rounds, `3dae6a14`, `94c21c45`, `beb16c92`; the closed list is below). What the finding is *for* — `mechanism` (3)'s guarantee that a topology module cannot reach an effect through a legacy wrapper while enforcement stays green — **still does not hold**, and the route that remains was opened by #306's own remedy:

- `src/engine/mod.rs:5` carries `#![allow(clippy::disallowed_methods)]` since `94c21c45`, a reviewed row of `effects/allowlist.toml`'s legacy section, so the v0.1 facade can call the two denied conductor entry points. A lint level inherits down the module tree, and the facade is in neither `CLASSIFIED_MODULES` (`src/effects.rs:636`) nor `effects/wrappers.toml`.
- An attribute-free **inline** module in the facade (`mod r3_inline_child { pub(super) fn r3_inline_inherited_effect(..) }` calling `std::fs::write`), referenced from the production body of `park_question` in `src/engine/topology/integrate.rs`: `clippy --all-targets --all-features -- -D warnings` **exit 0**; the effects suite plus a live topology caller **177 passed**, and the caller wrote and read back `r3 inline effect`. Round 3's guard walks only out-of-line `mod x;` declarations (`scan_module_declarations` emits nothing for `mod x { .. }`), so the inline child is never judged. Substituting `3dae6a14`'s `engine/mod.rs` and `clippy.toml`, before the allow, flips the same tree to **exit 101** at the write; so does a deny inside the inline child. The allowance activates the route.
- A `pub(crate) fn r3_unclassified_facade_effect` calling `std::fs::write` placed **directly in the facade**, referenced from the same body: clippy **exit 0**, effects suite **176 passed**. Nothing requires the facade's own functions to be classified.

Evidence: the fourth review of #306, `gpt-6-astra` at `max`, finding 1 (`~/orch-pr10/reviews/r306/r3-main-review.md`; `r3-main-evidence/`: `13-inline-child.patch`, `13-inline-child-clippy.log`, `14-inline-child-effects-and-live.log`, `inline-effect.txt`, `15-inline-child-before-facade-allow.log`, `16-inline-child-fenced.log`, `10-facade-probe.patch`, `10-facade-probe-clippy.log`, `11-facade-probe-effects.log`, `static-audit.json`). Filed as `PR306-FACADE-INLINE-ESCAPE`, whose file carries the witnesses in full; this file carries the guarantee and what #306 closed of it.

## What the change that takes this up should do

Owner, as the ledger records it: project owner — **the post-v0.2 pass over PR3's layer**.

**Closed by #306, and held by tests, so the next pass does not redo it:** the domain is derived from `production_code` rather than the truncating region (`a_configured_item_above_a_production_fn_does_not_hide_it_from_the_domain`, `every_classified_module_that_declares_a_visible_fn_has_a_domain`); every one of the 62 names the derivation added is classified against a stated module-closed rule, and every library record carries a real crate path (`only_the_binary_crate_root_leaves_its_crate_path_empty`); the five nameable effectful engine wrappers are denied by path and refused from a production body of `engine::topology::integrate` (`every_effectful_wrapper_is_on_the_disallowed_list`, `every_denied_path_this_host_can_resolve_does_resolve`); `src/engine/topology.rs` and the five out-of-line children of the facade that write no allow deny all three governed lints at file level, and `every_child_the_engine_facade_declares_re_denies_or_records_what_it_inherits` walks the facade's out-of-line declarations recursively and compiles the sibling witness open and fenced (`the_topology_root_re_denies_every_lint_the_engine_facade_allows` holds the topology root).

**Remaining, and deliberately not attempted on #306 (owner's decision, 2026-09-17):** the two routes above, both executed by the fourth review. The owner weighed a fourth round against the cascade of classifying the facade — its entry points (`run`, `run_with`, `run_harness`, `resume`, `resume_with`, `resume_harness`) would become `effectful`, needing denials that bind `src/main.rs`, whose crate path is legitimately empty — and chose to stop and file. The pass that takes this up should (a) make the boundary guard model inline modules, or make the placement scan refuse an attribute-free `mod x { .. }` under a file-level allow, **and** (b) constrain the facade's own items: classify `src/engine/mod.rs` with the `src/main.rs` consequence decided, or move the two entry-point calls into the allowed legacy modules they call so the facade needs no allow at all (the row's own `shrinks_when`), which retires both routes at once. The warning carried since 2026-08-31 still applies: the repair is shared enforcement machinery whose blast radius is every classified module, the shape that made PR5 round 7 a revert.

Carried in `reviews/FINDINGS.md` §2, "Open — carried deliberately, with an owner", confirmed still carried by the full-ledger audit of 2026-08-31 (§39); deleted on #306 with a `fixed` row that two reviews found premature (the third, `PR306-FACADE-ALLOW-ESCAPES-TO-SIBLINGS`, repaired in round 3; the fourth, `PR306-FACADE-INLINE-ESCAPE`, deferred), and restored on #306's close-out. The row carried no severity label; **P1** here is the migration's judgement from the consequence described above, not the reviewer's own word.
