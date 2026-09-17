---
id: PR306-FACADE-INLINE-ESCAPE
severity: P1
disposition: deferred
category: security-trust
pr: 306
reviewed_sha: beb16c924dfbdb5c395ac37b5671f4168b08cce6
location: src/engine/mod.rs:5
provenance: fix_regression
first_bad: 94c21c45fe6b9fc477f10866fe122c09d1507ddf
guard: project owner — decided on #306 (2026-09-17) to file rather than run a fourth round; the post-v0.2 pass over PR3's layer
---

## Failure sequence

`src/engine/mod.rs:5` carries `#![allow(clippy::disallowed_methods)]` since `94c21c45`, a reviewed row of `effects/allowlist.toml`'s legacy section, so the v0.1 facade can call `coordinator::run_harness_inner_on` and `resume::resume_harness_inner_on`, both denied by path. A lint level inherits down the module tree; the facade is in neither `CLASSIFIED_MODULES` (`src/effects.rs:636`) nor `effects/wrappers.toml`; and round 3's boundary guard, `every_child_the_engine_facade_declares_re_denies_or_records_what_it_inherits`, reads `scan_module_declarations`, which emits a declaration only for `mod x;` — its `mod x { .. }` branch pushes a scope and emits nothing (`src/effects.rs:1498`). Two routes from `engine::topology` to a raw effect therefore stay green under enforcement:

1. **An inline child.** Add an attribute-free `mod r3_inline_child { pub(super) fn r3_inline_inherited_effect(path) -> io::Result<()> { std::fs::write(path, b"r3 inline effect") } }` to the facade and reference it from the production body of `park_question` in `src/engine/topology/integrate.rs` -> the inline module inherits the allow, is walked by no guard and recorded in no list -> `clippy --all-targets --all-features -- -D warnings` exits **0**; the effects suite plus a live topology caller passes **177 tests**, and the caller writes and reads back the bytes (`inline-effect.txt`). Controls: with `3dae6a14`'s `engine/mod.rs` and `clippy.toml`, before the allow, the same tree exits **101** at `src/engine/mod.rs:103` for `std::fs::write`; a `#![deny(..)]` inside the inline child exits **101** too. The allowance activates the route.
2. **The facade itself.** Add `pub(crate) fn r3_unclassified_facade_effect(path)` calling `std::fs::write` directly in `src/engine/mod.rs` and reference it from the same body -> nothing requires the facade's own functions to be classified, and the allow covers them -> clippy exits **0**, the effects suite passes **176 tests**.

Evidence: the fourth review of #306, `gpt-6-astra` at `max`, finding 1 (`~/orch-pr10/reviews/r306/r3-main-review.md`; evidence under `r3-main-evidence/`: `13-inline-child.patch`, `13-inline-child-clippy.log`, `14-inline-child-effects-and-live.log`, `inline-effect.txt`, `15-inline-child-before-facade-allow.log`, `16-inline-child-fenced.log`, `10-facade-probe.patch`, `10-facade-probe-clippy.log`, `11-facade-probe-effects.log`, and `static-audit.json` with `facade_in_classified_modules: false`, `facade_in_wrappers: false`). Every mutation was restored and the reviewed tree matches `beb16c92`.

## What the change that takes this up should do

Owner, as the ledger records it: project owner. Filed rather than fixed on #306 by the owner's decision of 2026-09-17: a fourth round was weighed against the cascade of classifying the facade — its six entry points would become `effectful`, needing denials that bind `src/main.rs`, whose crate path is legitimately empty and which `only_the_binary_crate_root_leaves_its_crate_path_empty` exists to allow — and the owner chose to stop. Under `findings/PROCESS.md` §7 a finding carrying a reproduction is fixed whatever its label; this file records that the owner overrode that for this pull request, and why, so nobody reads the `deferred` row as an oversight.

What closes it, as the review states it: model or refuse inline children at the inheritance boundary (the guard walking `mod x { .. }` bodies, or the placement scan refusing an attribute-free inline module under a file-level allow), **and** constrain the facade's own effectful items through classification or an equivalent enforced boundary — fixing inline traversal alone leaves the direct-facade witness green. The cheapest whole answer is the facade row's own `shrinks_when`: move the two entry-point calls into the allowed legacy modules they call, so the facade carries no allow, both routes close at once, and the row leaves the frozen legacy section. This is the surviving half of `PR7-WRAPPERS-EMPTY-DOMAIN` seen from the remedy that opened it: that file carries the guarantee and what #306 closed of it, this one carries the concrete witnesses.
