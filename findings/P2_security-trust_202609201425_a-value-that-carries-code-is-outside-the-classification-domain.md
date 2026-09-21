---
id: PR7-CLASSIFICATION-DOMAIN-READS-FNS-ONLY
severity: P2
disposition: deferred
category: security-trust
pr: 7
reviewed_sha: b1869118baee7d6a03bae1265c38d9b7792b3d9e
location: src/effects.rs:693
provenance: pre_existing
first_bad:
guard: project owner — the post-v0.2 pass over PR3's layer
---

## Failure sequence

**Reasoned from the domain's definition, not executed.** `mechanism` (3) classifies "every pubfn of a legacy or shared module", and `effects::externally_reachable_fns` (`src/effects.rs:693`) derives that domain from `fn` items: a function that declares a visibility, a method of a trait impl, a public trait's default body. A module that carries a recorded allow can hold code in something that is not a `fn` item: a `const` or `static` whose value is a closure or a function pointer -> its body is compiled under the file's allow, so a denied primitive in it is not reported -> the item is not a `fn`, so the census never asks for its classification and `clippy.toml` has no path to deny, because a call through the value resolves to the `const`, not to a denied function -> a module that can see the item calls it. In `src/engine/{attempt,coordinator,resume}.rs`, whose `pub(super)` items every module under `engine::topology` can see, that is a route from a topology module to an effect with enforcement green. No such item exists in the tree; the route needs one written.

It predates #306 and the engine facade's allowance, and it holds of every module in `CLASSIFIED_MODULES`, not of `engine` alone. Found on 2026-09-20 by reading the domain while closing `PR306-FACADE-INLINE-ESCAPE` and the last of `PR7-WRAPPERS-EMPTY-DOMAIN`: `effects::tests::every_inline_module_under_the_engine_facade_is_walked_and_answered_for` requires every allowed production file under the facade to be a classified module, and this is the limit of what being one answers for. The same reading found that `declares_visibility` missed `pub(in path)`; that one was a recogniser's omission and is fixed in the same change.

## What the change that takes this up should do

Owner, as the ledger records it: project owner — the post-v0.2 pass over PR3's layer, which is where `PR7-WRAPPERS-EMPTY-DOMAIN` sat.

This needs a decision before it needs code: what the classification says about a value that carries code. The candidates are to widen the domain to `const` and `static` items with a visibility and classify them like functions (their effectful ones cannot be denied by path, so they would be `effectful_unnameable` and the record would be the whole of the control), or to refuse, in a module that carries a recorded allow, a visible `const` or `static` whose type or initializer names a function or a closure. Either is shared enforcement machinery whose blast radius is every classified module. Execute the route before choosing: under `findings/PROCESS.md` §7 a reproduction makes it a finding that is fixed whatever its label.
