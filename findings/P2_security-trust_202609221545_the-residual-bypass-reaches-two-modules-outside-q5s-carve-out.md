---
id: G5RUN4-RESIDUAL-BYPASS-OUTSIDE-THE-Q5-CARVE-OUT
severity: P2
disposition: deferred
category: security-trust
pr: 7
reviewed_sha: 9bb177ea59e1f97b8f3b3282c19519a31e1253a4
location: src/effects/tests.rs:749
provenance: pre_existing
first_bad:
guard: project owner — the post-v0.2 pass over PR3's layer, with `PR7-WRAPPERS-EMPTY-DOMAIN`, whose class the residue is; the guard that makes the roll-call shape fail closed is built (2026-09-22) and what remains is the pinned per-lint residue below
---

## Failure sequence

**Executed at `9bb177ea59e1f97b8f3b3282c19519a31e1253a4`**, in a scratch clone, by Gate 5's fourth run
(`~/tactus-artifacts/g5-evidence-9bb177e/questions/q5-route/`), and **narrowed by the fence change of
2026-09-22**, whose measurements are the second half of this section.

Q5 was amended v16 -> v17 on 2026-09-22 to scope the wrapper classification's completeness to *"the modules it
classifies — with the files that carry a file-level allowance of a governed lint excluded as an explicit carve-out
… and the residual bypass **inside that carve-out** carried by a filed finding"*. **Two scopes, and they are not the
same set.** Run 4's receipt counted the carve-out by allowlist row: **45** rows with a non-empty `allows` in
`effects/allowlist.toml`, of which **30** of the **54** modules `effects::CLASSIFIED_MODULES` and
`effects/wrappers.toml` classify, leaving **24** for the amendment's affirmative answer — the historical receipt,
kept as run 4 wrote it. v17's operative phrase is *"files that carry a file-level allowance of a governed lint"*, and
read from the files (`file_level_lint_state` over every leading inner attribute) the carve-out at `9bb177ea` is
**43** files (22 funnel rows, 21 legacy rows; `src/agent/bin.rs`'s row records an outer attribute on its inline
test module and `src/agent/proc/test_support/readiness.rs`'s six per-site `#[expect]`, neither a file-level
allowance), the classified split is **29 inside / 25 outside**, and **three** classified modules carried neither an
allowance nor a fence — `src/capacity.rs`, `src/runner/invocation.rs` and `src/agent/bin.rs` — not two. Both
of #314's fourth-round reviews derived the same figures independently at the frozen range
(`G5R4R4-RECORD-01`, `~/orch-pr10/reviews/pr-g5r4r4/`), and they are re-derived here at #318's final head with a
reader-faithful census (`~/orch-pr10/repair-318-r2-evidence/census/`): 187 scanned files, 51 rows, 45 non-empty,
43 file-level allowance files, 29/25, and **0** classified modules with neither. The corrected scope is the
affected one; the receipt is what run 4 measured.

**At `9bb177ea`, `PR7-WRAPPERS-EMPTY-DOMAIN`'s class was open in three of those 25, and run 4 executed it in two.**
`src/capacity.rs` and `src/runner/invocation.rs` were classified modules that carry **no** file-level allowance —
so they are outside the carve-out — and carried **no file-level fence of a governed lint** either, so #312's
`deny`->`forbid` flip did not reach them and the three governed lints took their level from `-D warnings` alone,
which an `allow` lowers. `src/agent/bin.rs` stood the same way and is the second half of this section.

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

**The enumeration, re-derived at `9bb177ea` and at the fenced head, and corrected on 2026-09-22.** The fence change
enumerated `CLASSIFIED_MODULES` against `effects/allowlist.toml` and each file's leading inner attributes read the way
`file_level_lint_state` reads them (`enumerate.py`, and `throwaway-enumeration-test.rs`, the same question put to the
tree's own reader): 54 classified modules, 30 in the carve-out, 24 outside; classified modules carrying neither an
allowance nor any file-level fence, **2 at `9bb177ea`** (these two), 2 at `958d3aa4`, **0 at the fenced head**
(`~/orch-pr10/fence-q5-evidence/`). Both reviews of #316 re-derived the same numbers. **All of them took the allowlist
row as the allowance leg**, and the row is not what v17 words the carve-out by: *"the files that carry a file-level
allowance of a governed lint"*. Read from the file, the classified carve-out is **29**, not 30 (and the whole
carve-out 43 files, not 45 rows), and the class had a **third** instance: `src/agent/bin.rs` records `allows = ["clippy::disallowed_methods"]` for an **outer attribute on its inline
`#[cfg(test)] mod tests`** (`src/agent/bin.rs:97`; its row says so, *"the allow is an outer attribute on that test
module, so production remains governed"*), and states nothing at file level, so its production region took all three
governed lints from `-D warnings` alone — exactly where `src/capacity.rs` was — at `9bb177ea`, at the fenced head, and
at `de6d6434`. The guard below, run once at `de6d6434` before anything was fenced, named it and nothing else
(`~/orch-pr10/guard-decision-evidence/H0-guard-at-de6d6434-unfenced.log`: 1 classified module stating no governed
lint at file level, `src/agent/bin.rs`; 32 file-and-lint pairs stated at no level, the 31 of the fence change plus
`src/agent/bin.rs`/`clippy::disallowed_methods`; 29 classified modules carrying a file-level allowance against 30
rows). Not executed in `bin.rs`: the mechanism is byte for byte run 4's, and `bin.rs` differs from `capacity.rs` at
`9bb177ea` in nothing the mechanism reads.

**Fenced on 2026-09-22 with the guard, and re-fenced on 2026-09-23 after #318's first review executed a write through
the fence.** The 2026-09-22 shape was `#![deny(clippy::disallowed_methods)]` — `deny`, because the inline test module's
allow of that lint makes an unconditional `forbid` `E0453` — beside `#![forbid(clippy::disallowed_types,
clippy::disallowed_macros)]`, and the `deny` was excused by `every_fence_of_a_governed_lint_forbids_wherever_forbid_would_compile`
because the allowance sits in the file. **Executed at `e851b6768715a1869a0c67e383e28f8f58eb0090` by #318's MAIN
reviewer** (`~/orch-pr10/reviews/pr-318r1/main-evidence/boundary-*`): run 4's mechanism, a `macro_rules!` in `bin.rs`
emitting `#[allow(clippy::disallowed_methods)] pub fn ..(..) { std::fs::write(..) }`, called from production
`engine::topology::integrate::prepared_pin_ref`, wrote 58 verified bytes while `cargo clippy --all-targets
--all-features -- -D warnings` exited **0** and **193** effects tests passed, both roll-call guards included; the same
macro emitting `deny` was refused at the write. So a `deny` that only a test-only allowance excuses is the class's
third shape: it is a statement the guard counts, and it is a level an inner `allow` reopens.

**Re-fenced (#318, 2026-09-23)**: `#![cfg_attr(not(test), forbid(clippy::disallowed_methods))]` — `forbid` in every
build without `cfg(test)`, which is the lib target CI's three clippy legs check and the binary links; the lib test
target, the only build in which the inline `#[cfg(test)] mod tests` and its allow exist, carries no `forbid` and keeps
`-D warnings`' level for the lint. The inline test allowance is untouched. Measured on the repaired tree with the
reviewer's own patch (`~/orch-pr10/repair-318-r2-evidence/controls-tree/`): clippy over all targets exits **101** with
`error[E0453]: allow(clippy::disallowed_methods) incompatible with previous forbid` at the macro's attribute, the
`forbid` quoted from line 3 (`B1-boundary-allow-clippy.log`); the macro emitting `deny` is refused at the write as before
(`B2-*`); a written allow and a spaced `#[allow (..)]` on a production `fn` are `E0453` too (`controls/c08`, `c09`).
**What the lint gate refuses, `cargo test` still compiles** (`B1-boundary-allow-test-alltargets.log`, rc 0, the witness
written): rustc resolves no `clippy::` lint, so it enforces neither the lint nor `E0453` for it (`controls/c15`–`c18`),
for this fence exactly as for every `forbid` #312 and #316 wrote and every denial in `clippy.toml`; the lint gate — local
gate 2, CI's three `lint` legs — is where every effect denial in this tree is enforced, and it is where this route is
closed. The lib test target alone (`cargo test --lib`) compiles the generated allow -- measured properly in the third
round, one fully qualified test selected and passed with the witness path deleted first
(`~/orch-pr10/repair-318-r3-evidence/probes/P4-L1-*`); the round-2 receipt for it, `controls-tree/L1-*`, selected no
test and copied its `B1` run's file, and is VOID -- and that is the shape's stated limit. `file_level_lint_state` reads a
`cfg_attr(not(test), ..)` inner attribute as the production build's statement
(`the_file_level_lint_reader_answers_what_rustc_does` compiles the new spellings), so the guard counts the pair stated and
the pin below stays 29.

**Third round (#318, 2026-09-23), after the second reviews executed two defects in the instruments above.** (1) The
production-fence rule read only the literal `#[cfg(test)]` as test scope: `bin.rs` put back to `deny` beside an empty
`#[cfg(all(test, unix))]` module carrying a marked `#[allow]`, with the macro wrapper and its production caller, passed
clippy over all targets and 195 effects tests -- the rule included -- while the witness wrote (`R2-REG-01`,
`~/orch-pr10/reviews/pr-318r2/regression-evidence/probes/boundary-extra-v2-*`; reproduced at `8bca4608`,
`~/orch-pr10/repair-318-r3-evidence/probes/H2-*`). The rule now decides an allowance's whole `cfg` stack, its own
`cfg_attr` predicate and the module it sits in through `census_domain::decide_without_test`, the crate's one reading
of a predicate; the same tree is refused by `no_deny_of_a_governed_lint_is_excused_by_test_code_alone` naming
`src/agent/bin.rs` (`probes/P2-*`), each of the seven scoped spellings on its own too (`probes/P1-*`, `P2b-*`).
(2) The reader dropped a nested `cfg_attr` and kept the level before it, so
`#![cfg_attr(not(test), deny(L), cfg_attr(not(test), allow(L)))]` read as `deny` while clippy-driver applied the
`allow` (`R2-REG-02`, `probes/nested-parity.*`; reproduced, `H3-*`). The reader now expands nested `cfg_attr`s, decides
predicates as the cfg census does, enumerates every production valuation's answer and claims a level only when all agree
(`Resolution::undecided` otherwise; `probes/P3-*`, and the parity test's second table compiles the platform, feature
and malformed spellings). Neither defect changed a level in the tree: the census re-derived at the third round's head
gives the same 43 / 29-25 / 0 / 29 pairs in 20 files. The class this shape names — a file-level `deny` every allowance below which is test code, so
the production build could `forbid` — was measured tree-wide at **11** more pairs in four files outside the roll-call
(`src/runner/container/census.rs`, `exec.rs`, `resolve.rs`; `src/engine/mod.rs` for `disallowed_types` and
`disallowed_macros`), the bypass executed in each at its `deny` (`~/orch-pr10/repair-318-r2-evidence/witnesses-prefix/`),
and all four fenced the same way in #318; `no_deny_of_a_governed_lint_is_excused_by_test_code_alone` refuses the shape
tree-wide from then on (`PR318-DENY-THE-PRODUCTION-BUILD-COULD-FORBID`, fixed in #318 and recorded in its ledger).

**The guard, built.** Two tests in `src/effects/tests.rs`, one helper, one pinned constant, reading the tree at run
time with no list of files (`~/orch-pr10/guard-decision-evidence/`, indexed by its `README.md`):

- `every_classified_module_carries_a_file_level_fence_or_allowance_of_a_governed_lint`: for every entry of
  `CLASSIFIED_MODULES`, the file's prologue states at least one used governed lint at file level — `forbid`, `deny`, or
  a recorded `allow`/`expect` — as `file_level_lint_state` reads it. **The allowance leg is what the file states, not
  what the row records.** At `de6d6434` with the two fenced files put back to their `9bb177ea` bytes it fails naming
  exactly `src/capacity.rs` and `src/runner/invocation.rs` (`F1-*.log`); with an undeclared, unfenced, unallowed file
  appended to `CLASSIFIED_MODULES` it fails naming exactly that file (`F2-*.log`); at the head with `bin.rs` fenced
  it passes (`H1-*.log`).
- `the_governed_lint_pairs_classified_modules_leave_unstated_only_shrink`: the (module, lint) pairs stated at no
  level are counted and asserted equal to `UNSTATED_GOVERNED_LINT_PAIRS_IN_CLASSIFIED_MODULES`, **29** at this head,
  all inside the carve-out, in 20 files (the guard's own listing, re-derived at the third round's head:
  `~/orch-pr10/repair-318-r3-evidence/plan-class/census-at-head.json`); `bin.rs` contributes none, since all three of
  its lints are stated -- `disallowed_methods` as `cfg_attr(not(test), forbid(..))` since the second round, the other
  two as `forbid` -- and the 32 the first run of the guard counted at `de6d6434` were these 29 plus `bin.rs`'s three.
  The `forbid` of `disallowed_macros` removed from `src/runner/container/view.rs` fails at 30
  naming that pair (`F3-*.log`); the constant lowered by one fails at 29 against 28, and raised by one at 29 against
  30, each with its reading stated (`F4b-*.log`, `F5-*.log`). A count and not a list: a table of 29 pairs in an instrument is a second roll-call, and the change
  that fences a pair lowers one number.

**What the guard closes, in this file's own words.** A classified module can no longer arrive carrying neither an
allowance nor a fence, and an existing entry can no longer drop its allowance without a fence taking its place: both
are a red test at the next `cargo test`, not a gate run's discovery. `effects/allowlist.toml`'s sentence that
`src/capacity.rs` "can be dropped from this list at any time without weakening anything" is now checked rather than
promised.

**What remains, and it is the residue and not the class.** The **29 pairs**: 20 carve-out files that allow one or two
governed lints and state nothing about the rest (the count is the guard's own, from the pin's failure message —
`~/orch-pr10/reviews/pr-318r1/main-evidence/residue-prose-durable.log`, 29 pairs across 20 files, three of them under
`src/engine/`; corrected on 2026-09-23 from a hand count of 19 and four) — `src/interaction.rs` allows methods and macros and says nothing of
`disallowed_types`, `src/engine/{attempt,coordinator,resume}.rs` allow methods and state nothing for the other two —
each listed by the pin's own failure message and by `H0-*.log`. **Unstated is not lowerable, and the pin's listing
now says which is which.** Since #318 fenced `src/engine/mod.rs` (`cfg_attr(not(test), forbid(clippy::disallowed_types))`,
`forbid(clippy::disallowed_macros)`), those six pairs inherit a production `forbid`: a generated `allow` of either lint
in any of the three is `E0453` at the lint gate, six of them measured by #318's second regression review
(`~/orch-pr10/reviews/pr-318r2/regression-evidence/probes/inherited-forbid.*`). The other 23 pairs sit under ancestors
that state nothing up to `src/lib.rs` and take `-D warnings` alone, which an inner `allow` lowers. By v17's wording all
29 are inside the carve-out and `PR7-WRAPPERS-EMPTY-DOMAIN`'s, whose sentence *"open in the 45 files that carry an
allowance … for the lint each allows"* under-states by the 23. Checked by file at the third round's head
(`~/orch-pr10/repair-318-r3-evidence/plan-class/twenty-six-vs-carve-out.json`): the 23 lowerable pairs sit in 17 files,
and every one of the 17 carries a file-level allowance of a governed lint, so all 23 are inside v17's carve-out as read
from the files, and none is in a production host outside it; after the third round's `deny` at `src/agent/mod.rs`,
three of them (`disallowed_types` in `claude.rs`, `codex.rs`, `copilot.rs`) inherit that `deny` instead of taking
`-D warnings` alone -- lowerable either way, and inside either way. Whether `forbid` of the unstated lint compiles in
each is a measurement, and it is outside the remedy this file's guard was built for. **Two
things the guard does not reach, each its own finding**: a module absent from the roll-call altogether
(`W1-CLASSIFIED-MODULES-IS-A-HAND-MAINTAINED-ROLL-CALL`), and the **50 production and test files outside the roll-call
that state no governed lint at file level and inherit no statement** — 34 under `src/topology/` and `src/runner/`,
16 elsewhere — where the same silence stands unjudged
(`GUARD-DECISION-SILENT-PRODUCTION-FILES-OUTSIDE-THE-ROLL-CALL`, filed with the guard).

`location` is the pinned constant, because what remains is the number it holds.

## What the change that takes this up should do

Owner, as the ledger records it: project owner — the post-v0.2 pass over PR3's layer, the pass that owns
`PR7-WRAPPERS-EMPTY-DOMAIN`. One thing, and it is the fence work the guard now counts:

1. **The 29 pairs**: in each of the 20 files, state the unstated lint — `forbid` where it compiles,
   `cfg_attr(not(test), forbid(..))` where only test code below makes `forbid` `E0453`, `deny` where a production
   allowance below does, and let `every_fence_of_a_governed_lint_forbids_wherever_forbid_would_compile` and
   `no_deny_of_a_governed_lint_is_excused_by_test_code_alone` say which — and lower
   `UNSTATED_GOVERNED_LINT_PAIRS_IN_CLASSIFIED_MODULES` by the pairs closed, in the same change. Three of the 20 —
   `src/engine/attempt.rs`, `src/engine/coordinator.rs` and `src/engine/resume.rs` — sit under `src/engine/mod.rs`
   (`src/engine/preflight.rs` forbids all three and contributes no pair), whose own fence must stay `deny` for
   `disallowed_methods`, which those three allow; the children can still `forbid` a lint no allowance below them
   names. When the constant reaches 0, delete it and the ratchet, make
   `every_classified_module_carries_a_file_level_fence_or_allowance_of_a_governed_lint` refuse any unstated pair
   rather than a fully silent module, and delete this file: the class is then closed for the roll-call at both
   granularities, and what is left is the roll-call finding's and the silence outside it.
