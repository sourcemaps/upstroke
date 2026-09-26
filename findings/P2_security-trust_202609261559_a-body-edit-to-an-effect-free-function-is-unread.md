---
id: PR325-A-BODY-EDIT-TO-AN-EFFECT-FREE-FN-IS-UNREAD
severity: P2
disposition: deferred
category: security-trust
pr: 325
reviewed_sha: c0d597555d8dac04882c1ac3804b6312bb79c6e7
location: src/effects/tests/classification.rs:67
provenance: pre_existing
first_bad:
guard: project owner — the expansion-aware classification the 2026-09-26 ruling on PR7-WRAPPERS-EMPTY-DOMAIN declined for #325 (its option 3), or a classification that is re-derived rather than reviewed once
---

## Failure sequence

`decisions.effect_site_inventory.mechanism` (3) holds that a topology module cannot reach an effect
through a legacy wrapper because every externally reachable function of a legacy or shared module is
classified by review, and the effectful ones are denied by path in `clippy.toml`. The census that holds
the classification to the tree compares **names only**:
`effects::tests::classification::checks::classification_disagreement` derives the set of reachable
function names from each classified module's text (`effects::externally_reachable_fns`) and compares it
with the names `effects/wrappers.toml` records, class by class. No census reads a classified function's
body, and nothing re-derives a class once a name is recorded.

So, reasoned from that code:

1. Take a function `effects/wrappers.toml` records as `effect_free` in a classified module that allows a
   governed lint in the production build -- at `c0d59755` there are **414** such names in **28** such
   modules (`src/rundir.rs` 47, `src/workspace_manager.rs` 45, `src/events/mod.rs` 35, ...).
2. A pull request edits that function's body to call `std::fs::write`, or an effectful wrapper, which the
   module's own recorded allowance admits. It adds no name, no module and no allowance, and changes no
   record.
3. Clippy accepts the body under the module's allow. The name census sees the same names on both sides.
   The allowance, fence and macro-position censuses see nothing new. The function stays off the
   disallowed list because it is still recorded `effect_free`.
4. A topology module calls the function, which it may, and reaches the effect with every gate green.

Mechanism (3) says the classification is made "by review"; what this records is that it is made **once**:
the review that classifies a function is the only reading of its body the classification ever gets, and
a later edit to the body is reviewed as an ordinary change with no census pointing at the class it
invalidates.

**Reasoned, not executed.** The census code above is read, not exercised against an edited body, and
no witness was built.

## Reachability, against the owner's rule of 2026-09-11

*A P1 blocks a merge only if it can happen in normal use, or someone without push access can trigger
it.* **Neither limb holds.** Not normal use: it is a change to the source, not a run of the product.
Not someone without push access: the edit reaches the tree only through a merge, which is the owner's
act or a delegate's, and the body edit is in the diff the reviewer reads -- the census is blind to it,
the review is not. A contributor without push access can propose it and CI stays green, which is why it
is filed rather than left implicit.

**Why P2 and not P1:** the consequence is the one `PR7-WRAPPERS-EMPTY-DOMAIN` records -- an effect a
topology module reaches with enforcement green -- but that finding's routes hide what compiles from the
text a reviewer reads (a name spelt by expansion, a module an alias includes), and this one does not:
the added call is written in the body the diff shows. The owner may reclassify.

## What the change that takes this up should do

Owner, as the ledger records it: project owner. **Do not close it with another name recogniser**; a
body is not a name.

- Read the bodies the classification rests on. The stronger instrument measured on 2026-09-26 reads
  rustc's expansion (`RUSTC_BOOTSTRAP=1 rustc -Zunpretty=expanded`, about 3.5 s per profile with
  dependencies built, one expansion per CI target and profile, and every census re-keyed from file path
  to module path) and was declined for #325 because it makes a required gate depend on an unstable
  compiler flag. An effect inference over the expanded bodies, module-closed as the classification rule
  already is, would re-derive `effect_free` rather than trust it.
- Short of that, make the review re-occur: a census that fails when a classified function's body changes
  and its record does not -- a per-function digest of the production text beside each `effect_free`
  row, re-recorded by hand when the reviewer re-reads the body. That keeps the classification a review
  and turns "reviewed once" into "reviewed each time the body moves".

## Provenance

Named by #325's first round as the residual of both options the orchestrator took ("mechanism (3)'s
classification stays review-dependent for `effect_free` bodies"), and filed as its own finding in its
third round by the orchestrator's instruction, under its own label: it is not the expansion half of
`PR7-WRAPPERS-EMPTY-DOMAIN` (a body edit needs no macro), and folding it in would keep that finding open
for a route its own repairs were never about.
