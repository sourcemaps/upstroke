---
id: G5RUN4-RESIDUAL-BYPASS-OUTSIDE-THE-Q5-CARVE-OUT
severity: P2
disposition: deferred
category: security-trust
pr: 7
reviewed_sha: 9bb177ea59e1f97b8f3b3282c19519a31e1253a4
location: src/capacity.rs:1
provenance: pre_existing
first_bad:
guard: project owner — the post-v0.2 pass over PR3's layer, with `PR7-WRAPPERS-EMPTY-DOMAIN`, whose class this is
---

## Failure sequence

**Executed at `9bb177ea59e1f97b8f3b3282c19519a31e1253a4`**, in a scratch clone, by Gate 5's fourth run
(`~/tactus-artifacts/g5-evidence-9bb177e/questions/q5-route/`).

Q5 was amended v16 -> v17 on 2026-09-22 to scope the wrapper classification's completeness to *"the modules it
classifies — with the files that carry a file-level allowance of a governed lint excluded as an explicit carve-out
… and the residual bypass **inside that carve-out** carried by a filed finding"*. The carve-out is the **45** files
with a non-empty `allows` in `effects/allowlist.toml`. `effects::CLASSIFIED_MODULES` and `effects/wrappers.toml`
classify **54** modules; **30** of them are in the carve-out, so the amendment's affirmative answer is over the
remaining **24**.

**`PR7-WRAPPERS-EMPTY-DOMAIN`'s class is open in two of those 24.** `src/capacity.rs` and
`src/runner/invocation.rs` are classified modules that carry **no** file-level allowance — so they are outside the
carve-out — and **no file-level fence of a governed lint** either, so #312's `deny`->`forbid` flip does not reach
them and the three governed lints take their level from `-D warnings` alone, which an `allow` lowers.

Executed, the witness the reviews of #309 wrote, moved into those two files: a `macro_rules!` that emits
`#[$level(clippy::disallowed_methods)] pub fn $name(..) { std::fs::write(..) }`, invoked with `allow` and a
substituted name, and both functions called from production `park_question` in `src/engine/topology/integrate.rs`,
a topology module.

- **The witness**: `cargo clippy --all-targets --all-features -- -D warnings` exits **0**, and the whole effects
  governance suite passes, **190 passed, 0 failed** (`clippy-macro-witness.log`, `effects-macro-witness.log`,
  `witness-macro.patch`).
- **Control, the level**: the same macro emitting `deny` instead of `allow` makes clippy exit **101**,
  `error: use of a disallowed method`, quoting `UPSTROKE-EFFECT R21/R18`, at both writes
  (`clippy-control-deny.log`, `control-deny.patch`). So the write is genuinely denied and the emitted `allow` is
  what opens the route.
- **Control, the name**: the same function written with its name in the source text is refused —
  `effects::tests::every_externally_reachable_fn_of_a_legacy_or_shared_module_is_classified` fails at
  `src/effects/tests/classification.rs:56`, naming `g5r4_q5_probe_write_inv` unclassified
  (`effects-spaced-witness.log`). So the classification census works against a written name, and identifier
  substitution is what defeats it — the cause `PR7-WRAPPERS-EMPTY-DOMAIN` states.

**The consequence for the record, which is the point of filing this separately.** `PR7-WRAPPERS-EMPTY-DOMAIN` says
the route is open in *"the 45 files that carry an allowance of a governed lint … and … the five files that still
fence with `deny`"*. That sentence is about the **fence** sets, and it is true; read as a statement about the
**classification's** domain it under-states, because a classified module can be in neither set — neither allowing
nor fencing — and two are. Q5's v17 wording carves out only the allowance-bearing files, so **a gate answering Q5's
wrapper limb in the affirmative over the modules outside the carve-out would be asserting something this
reproduction contradicts.**

Nothing in the tree is wrong *today* by this route: no such macro exists in either file. What is filed is that the
carve-out does not bound the class.

## What the change that takes this up should do

Owner, as the ledger records it: project owner — the post-v0.2 pass over PR3's layer, the pass that owns
`PR7-WRAPPERS-EMPTY-DOMAIN`. Two things are worth separating for whoever takes it:

1. **The cheap, real part, on the model of #312.** Neither `src/capacity.rs` nor `src/runner/invocation.rs` carries
   an allowance, so `#![forbid(clippy::disallowed_methods, clippy::disallowed_types, clippy::disallowed_macros)]`
   compiles in both and closes the class there outright, the way it did in the 54 files that fence. Whether every
   classified module outside the carve-out can be fenced is a measurement, not an assumption: flip each and compile,
   as the fence change did.
2. **The rule that would keep it closed.** A classified module that neither allows nor forbids a governed lint is in
   no census: `every_fence_of_a_governed_lint_forbids_wherever_forbid_would_compile` judges the files that *have* a
   fence, and the allow-placement scan judges the files that *have* an allowance. A guard asserting that every module
   of `CLASSIFIED_MODULES` either forbids each governed lint or records an allowance of it would make this shape
   fail closed, and would have caught these two.
