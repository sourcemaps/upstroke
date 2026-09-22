---
id: G5RUN4-RESIDUAL-BYPASS-OUTSIDE-THE-Q5-CARVE-OUT
severity: P2
disposition: deferred
category: security-trust
pr: 7
reviewed_sha: 9bb177ea59e1f97b8f3b3282c19519a31e1253a4
location: src/effects.rs:666
provenance: pre_existing
first_bad:
guard: project owner — the post-v0.2 pass over PR3's layer, with `PR7-WRAPPERS-EMPTY-DOMAIN`, whose class this is; the guard that would make the shape fail closed is proposed below and not built
---

## Failure sequence

**Executed at `9bb177ea59e1f97b8f3b3282c19519a31e1253a4`**, in a scratch clone, by Gate 5's fourth run
(`~/tactus-artifacts/g5-evidence-9bb177e/questions/q5-route/`), and **narrowed by the fence change of
2026-09-22**, whose measurements are the second half of this section.

Q5 was amended v16 -> v17 on 2026-09-22 to scope the wrapper classification's completeness to *"the modules it
classifies — with the files that carry a file-level allowance of a governed lint excluded as an explicit carve-out
… and the residual bypass **inside that carve-out** carried by a filed finding"*. The carve-out is the **45** files
with a non-empty `allows` in `effects/allowlist.toml`. `effects::CLASSIFIED_MODULES` and `effects/wrappers.toml`
classify **54** modules; **30** of them are in the carve-out, so the amendment's affirmative answer is over the
remaining **24**.

**At `9bb177ea`, `PR7-WRAPPERS-EMPTY-DOMAIN`'s class was open in two of those 24.** `src/capacity.rs` and
`src/runner/invocation.rs` were classified modules that carry **no** file-level allowance — so they are outside the
carve-out — and carried **no file-level fence of a governed lint** either, so #312's `deny`->`forbid` flip did not
reach them and the three governed lints took their level from `-D warnings` alone, which an `allow` lowers.

Executed by run 4, the witness the reviews of #309 wrote, moved into those two files: a `macro_rules!` that emits
`#[$level(clippy::disallowed_methods)] pub fn $name(..) { std::fs::write(..) }`, invoked with `allow` and a
substituted name, and both functions called from production `park_question` in `src/engine/topology/integrate.rs`,
a topology module.

- **The witness**: `cargo clippy --all-targets --all-features -- -D warnings` exited **0**, and the whole effects
  governance suite passed, **190 passed, 0 failed** (`clippy-macro-witness.log`, `effects-macro-witness.log`,
  `witness-macro.patch`).
- **Control, the level**: the same macro emitting `deny` instead of `allow` made clippy exit **101**,
  `error: use of a disallowed method`, quoting `UPSTROKE-EFFECT R21/R18`, at both writes
  (`clippy-control-deny.log`, `control-deny.patch`). So the write was genuinely denied and the emitted `allow` was
  what opened the route.
- **Control, the name**: the same function written with its name in the source text was refused —
  `effects::tests::every_externally_reachable_fn_of_a_legacy_or_shared_module_is_classified` failed at
  `src/effects/tests/classification.rs:56`, naming `g5r4_q5_probe_write_inv` unclassified
  (`effects-spaced-witness.log`). So the classification census works against a written name, and identifier
  substitution is what defeats it — the cause `PR7-WRAPPERS-EMPTY-DOMAIN` states.

**Closed in those two files by the fence change of 2026-09-22, and this is exactly what it closed.** Each of
`src/capacity.rs` and `src/runner/invocation.rs` now carries `#![forbid(clippy::disallowed_methods,
clippy::disallowed_types, clippy::disallowed_macros)]` at file level, nothing else moved, no allowlist row and no
classification changed. **`forbid` in both, not `deny`, because it compiles and this was measured rather than
assumed**: `forbid` is module-scoped and poisons each file's inline `#[cfg(test)] mod tests`, so an allowance there
would have been `E0453`; clippy over all targets at the fenced tree exits **0** (`clippy-forbid-both.log` in
`~/orch-pr10/fence-q5-evidence/`), so neither test module carries one. Run 4's own patch files, unedited, re-applied
at the fenced head `eb18f22dac5356a0ea1eaa06a3135b8450fce85d`, with the tree hash restored after each (`~/orch-pr10/fence-q5-evidence/`):

- **`witness-macro.patch`, the executed bypass**: clippy exits **101**, two `error[E0453]: allow(clippy::disallowed_methods) incompatible with previous forbid` at the macro's own `#[$level(clippy::disallowed_methods)]` (`src/capacity.rs:706`, `src/runner/invocation.rs:342`, `in this macro invocation`), the `forbid` level quoted from line 4 of each file, lib and lib test both refused (`W1-*.log`).
- **`witness-spaced.patch`**, the same route by the spaced-attribute spelling `#[allow (..)]` that
  `effects::governed_allows` does not read: clippy exits **101**, two `E0453` at the spaced attributes (`src/capacity.rs:703`, `src/runner/invocation.rs:339`) (`W2-*.log`).
- **Controls, so a refusal can be told from a compile that was never going to work.** The same two functions with
  no attribute at all, same caller: clippy exits **101**, two `error: use of a disallowed method \`std::fs::write\``, each with `the lint level is defined here` at the new fence, `src/capacity.rs:4` and `src/runner/invocation.rs:4` (`C2-*.log`) — the fence is what refuses
  the write. Run 4's `control-deny.patch` (the macro emits `deny`): exit **101**, `use of a disallowed method` at both writes with the level quoted from the macro's own `#[deny(..)]`, run 4's control reproduced verbatim (rustc accepts a `deny` under a `forbid`; only a downgrade is `E0453`) (`C1-*.log`). The bypass patch with the
  two fences dropped to `deny`: clippy exits **0** and the effects suite fails on exactly one test, `every_fence_of_a_governed_lint_forbids_wherever_forbid_would_compile`, naming exactly these two files, 189 passed (`C3-*.log`) — so the
  keyword is what closes it, `deny` would not have, and the pin from #312 refuses the drop. The spaced witness at
  `deny`: clippy exit 0, and the effects suite fails on exactly two tests, 188 passed: `every_externally_reachable_fn_of_a_legacy_or_shared_module_is_classified` naming `g5r4_q5_probe_write` and `g5r4_q5_probe_write_inv` unclassified, and the fence pin (`C4-*.log`) — run 4's literal-name control, reproduced. The two files put back to their
  `9bb177ea` bytes with the bypass patch applied: clippy exits **0** (`B1-*.log`) — run 4's result,
  reproduced by this lane.

**The enumeration, re-derived at `9bb177ea` and at the fenced head** (`enumerate.py`, over `CLASSIFIED_MODULES`,
`effects/allowlist.toml` and each file's leading inner attributes read the way `file_level_lint_state` reads them;
and `throwaway-enumeration-test.rs`, the same question put to the tree's own reader): 54 classified modules, 30 in
the carve-out, 24 outside it; classified modules carrying neither an allowance nor any file-level fence of a
governed lint: **2 at `9bb177ea`** (these two), **2 at `958d3aa4`**, `origin/master` after #315, whose only change is a findings file, and **0 at the fenced head**, confirmed by the tree's own reader through a throwaway test appended to `src/effects/tests.rs` for one run and discarded: 54, 30, 24, no file with neither, and 31 file-and-lint pairs, the same 31 the script lists (`T1-reader-enumeration-at-eb18f22d.log`). Every other classified
module outside the carve-out forbids all three governed lints at file level, none with `deny`.

`location` moved from `src/capacity.rs:1` to `src/effects.rs:666`, the `CLASSIFIED_MODULES` roll-call, because
what remains is a property of the roll-call and not of either file.

**What the fence change does not close — the class, in this file's own words.** A classified module can still
arrive carrying neither an allowance nor a fence, and nothing will catch it.
`every_fence_of_a_governed_lint_forbids_wherever_forbid_would_compile` judges the files that *have* a fence;
`every_allow_of_a_governed_lint_is_module_level_and_in_the_allowlist` judges the files that *have* an allowance;
`CLASSIFIED_MODULES` is a hand-maintained roll-call (`PR5-…`, filed as
`P2_correctness_202609031102_classified-modules-is-a-hand-maintained-roll-call.md`), and no test asks of each entry
whether it is in one of those two sets. Two ways the shape comes back, neither executed here: a module added to the
roll-call with `allows = []` and no fence, or an existing entry whose allowance is dropped without a fence taking its
place — `effects/allowlist.toml` says of `src/capacity.rs` that it "can be dropped from this list at any time
without weakening anything", and dropping an allowance today obliges nobody to add a fence. Run 4 offered three
remedies — fence the two, widen the carve-out, or add the guard that makes the shape fail closed — and the owner
chose the first; the guard is under "What the change that takes this up should do".

**One level down, measured by enumeration and not executed: the same shape per lint, inside the carve-out.** Over
the 30 classified modules that carry an allowance, **31 file-and-lint pairs in 21 files** have a governed lint that
is neither allowed by the file's row nor forbidden by its prologue (`enumerate-at-head.txt`): `src/interaction.rs`,
for one, allows `disallowed_methods` and `disallowed_macros`, fences nothing, and so takes its level of
`disallowed_types` from `-D warnings` exactly as these two files took all three. By v17's wording those files are
inside the carve-out, so the pairs are `PR7-WRAPPERS-EMPTY-DOMAIN`'s; but that finding's sentence *"open in the 45
files that carry an allowance … for the lint each allows"* under-states by these 31, which are open for a lint the
file does not allow. Not fenced by the 2026-09-22 change: whether `forbid` of the unallowed lint compiles in each is
a measurement (four of the 21 sit under `src/engine/mod.rs`, whose fence must stay `deny`), and it is outside the
remedy the owner authorised.

Nothing in the tree is wrong *today* by this route: no such macro exists in any classified module. What is filed is
that the carve-out does not bound the class, and that after the two fences nothing bounds it either.

## What the change that takes this up should do

Owner, as the ledger records it: project owner — the post-v0.2 pass over PR3's layer, the pass that owns
`PR7-WRAPPERS-EMPTY-DOMAIN`. Two things, separable:

1. **The guard, which is the cheap part and is not built.** A test asserting, for every entry of
   `CLASSIFIED_MODULES` and every lint of `USED_GOVERNED_LINTS`, that the file either forbids the lint at file
   level (`file_level_lint_state == Some("forbid")`) or records an allowance of it in `effects/allowlist.toml`.
   At the fenced head its file-level form — no classified module with neither — holds with zero exceptions and no
   list, and would have named `src/capacity.rs` and `src/runner/invocation.rs` at `9bb177ea`. Its per-lint form
   needs the 31 pairs above fenced first, or excused by name until they are. Either form makes the next unfenced,
   unallowed module a red test rather than a gate run's discovery. It is a CI-contract test under `src/effects/`,
   an instrument, so it is the owner's to authorise. A module missing from the roll-call altogether is the
   hand-maintained-roll-call finding's, not this one's.
2. **The 31 pairs**: flip each unallowed lint to `forbid` in its file and compile, as the fence change did for 59
   files and this one for two; keep `deny` where `forbid` does not compile and let
   `every_fence_of_a_governed_lint_forbids_wherever_forbid_would_compile` say which.
