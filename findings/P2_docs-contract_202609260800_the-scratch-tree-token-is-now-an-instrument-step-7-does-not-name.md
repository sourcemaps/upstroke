---
id: PR322-SCRATCH-TREE-TOKEN-IS-NOW-AN-INSTRUMENT-STEP-7-DOES-NOT-NAME
severity: P2
disposition: deferred
category: docs-contract
pr: 322
reviewed_sha: 1dabe096bd9ab6a834a2c47ca1170fda94eadea4
location: src/rundir/scratch_tree.rs:416
provenance: introduced_by_feature
first_bad: this pull request
guard: the project owner — `MAINTAINING.md` step 7 is a rule file, so only a reviewed change to it can record this
---

## Failure sequence

`MAINTAINING.md` step 7's first limb asks of each changed path: *"were this file written to deceive, could a
required check report success without having done its work?"* An **instrument** is code or text that decides
whether *other* changes are permitted. Step 7 names **"the CI-contract tests under `src/effects/`"** among the
paths known to be on the wrong side of that line, and warns that the list **"is not closed"**, having *"twice
been written down as though it were closed and twice been broken by a review, the second time by two paths nobody
had listed."*

**This pull request makes `src/rundir/scratch_tree.rs` such a path, and it is not on the list.**

Before this change, the nine CI-contract tests in `src/effects/tests.rs` built their fixtures through a local
`scratch_dir` helper that computed a path directly. After it, those nine tests obtain their fixture roots from
**`scratch_tree::acquire`**, so that function now **decides where the instrument tests compile the fixtures they
judge.**

**Executed by #322's limb-1 review** (`~/findings-sweep/orch-p1/review-322-limb1.md`): an edit **confined to
`acquire`**, leaving every assertion in `src/effects/tests.rs` byte-identical, produces a **false green 10 times
out of 10** — `every_denied_path_this_host_can_resolve_does_resolve` passes while `clippy.toml` carries a denial
that enforces nothing. The honest head catches that same denial at exit 101. The equivalent 21-line edit confined
to the `scratch_dir` helper in `src/effects/tests.rs` does the same, 5 of 5.

**This is a statement about what the file now governs, not a defect in this pull request.** The same review
established, executed, that **nothing #322 wrote produces a false green**: the borrow checker refuses a guard drop
while the lent path is in use at all nine sites, every tool call returns before the guard can drop, removing any
of the nine fixtures at the moment its tool runs turns every test red, the eight clippy tests fail on the same
assertion as at the base (24 of 24 pairs), and every refusal of `acquire` is red.

## Why it matters beyond this pull request

Every future change to `src/rundir/scratch_tree.rs` now carries the instrument property, so step 7's limb 1 is
answered **yes** for it — which sends such a pull request to the owner rather than to the standing delegation.
A reader working from step 7's list alone would not know that, and would answer the limb wrongly. That is the
third time the list has been found incomplete, and step 7 anticipates it: *"There is no third list; there is the
question above."*

## What the change that takes this up should do

Record it where step 7's readers will find it. Two candidates, and the choice is the owner's:

- add `src/rundir/scratch_tree.rs` to step 7's examples, with the reason — that it decides where the
  `src/effects/` CI-contract tests compile their fixtures; or
- state the property at `scratch_tree.rs`'s own module header, so a change there meets it before reaching step 7.

**Neither is in this pull request's gift.** `MAINTAINING.md` is a rule file, so amending it is a reviewed change
to a rule and the owner's alone — which is also why this row is `deferred` rather than fixed here.

## Reachability, against the owner's rule of 2026-09-11

**Neither limb, so it blocks no merge on reachability.** It is not reachable in ordinary use — the shipped binary
runs none of this — and nobody without push access can edit `acquire`. Its consequence is to **who may merge**,
not to what the product does. It is filed P2 rather than P3 because answering step 7's limb wrongly would place a
pull request that touches an instrument under a delegation that does not reach it.
