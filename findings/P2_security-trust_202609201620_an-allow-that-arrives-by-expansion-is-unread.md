---
id: PR309-AN-ALLOW-THAT-ARRIVES-BY-EXPANSION-IS-UNREAD
severity: P2
disposition: deferred
category: security-trust
pr: 309
reviewed_sha: 409a613866518f43ded15b02264540a9d84d2afd
location: src/effects.rs:446
provenance: pre_existing
first_bad:
guard: project owner — the post-v0.2 pass over PR3's layer
---

## Failure sequence

`effects::governed_allows` (`src/effects.rs:446`) reads the attributes a source file WRITES, and every reading built on it inherits that: the allow-placement scan, the fences #306 put on the engine facade's children and on the topology root, and the guards #309 added. An allow does not have to be written in the file it takes effect in. **Executed, in the engine facade, by the review of `409a6138`** (`~/orch-pr10/reviews/rfacade-1/main-review.md`, finding 2; `main-evidence/04-*`, `06-*`, `08-*`): the facade `include!`s a `.inc` file that declares `#[allow(clippy::disallowed_methods)] mod rf_included { .. }` -> no scan reads a `.inc`; and the facade defines a macro taking `$kind:ident` and `$level:ident` that expands `#[$level(clippy::disallowed_methods)] $kind rf_generated { .. }`, invoked with `(mod, allow)` -> the text spells neither `allow(` nor `mod x {` -> in both forms a production body under `engine::topology` calls the generated function, clippy exits 0 and 180 tests pass, the live caller writing and reading back its bytes.

**Both facade forms are closed by #309's repair**: a module outside the topology that a topology module descends from may hold only its leading attributes, `mod x;` declarations and `use` re-exports (`effects::tests::the_engine_facade_allows_no_governed_lint_and_refuses_both_escape_routes`, by whitelist, so it does not depend on recognising a spelling), and no scanned source may use `include!` (`effects::tests::no_scanned_source_includes_a_file_no_scan_reads`). **What remains is reasoned, not executed**: the same substituting macro written in a fenced sibling (`src/engine/assembly.rs` and the four like it), in a topology file, or in any other module of the crate overrides that file's `deny` with an allow that `governed_allows` does not read, so the placement scan records nothing and the fence is lexically intact. A whitelist cannot reach those files, which hold code. It is the same family as `an-applied-cfg-attr-is-invisible-to-the-scan`: an allow applied by something the scan does not evaluate.

## What the change that takes this up should do

Owner, as the ledger records it: project owner — the post-v0.2 pass over PR3's layer.

The mechanism that does not depend on reading text is rustc's own: `#![forbid(..)]` refuses a later `allow` of the same lint however it is spelled, expansion included (`E0453`). It fits wherever nothing below needs an allow -- `src/engine/topology.rs`, none of whose forty children writes one, and the five fenced siblings of the facade -- and does not fit the facade itself, four of whose children carry recorded allows. Execute the sibling form before choosing; under `findings/PROCESS.md` §7 a reproduction makes it a finding that is fixed whatever its label. Converting the fences is a change to what #306 landed and reviewed three times, which is why #309's repair states this rather than doing it.
