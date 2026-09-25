---
id: PR311-NEW-RECOVER-PROSE-OUTSIDE-NOTES
severity: P3
disposition: deferred
category: docs-contract
pr: 311
reviewed_sha: 7e053e0f0bd3b4edd322adcc96a3c88bb46bb36d
location: src/engine/topology/recover/tests.rs:20684
provenance: introduced_by_feature
first_bad: 7583909365c26079d02096b608698df0ba0f015d
guard: documented, not enforced: `.github/scripts/test-internals-notes.sh` holds the module's `Extended notes:` pointer and the notes file it names, not the absence of other prose; the next change that touches these five sites of `src/engine/topology/recover/tests.rs`, or a §13 pass over the module, moves the prose into `docs/internals/engine/topology/recover/tests.md` and removes the duplication
---

## Failure sequence

PR #311's third repair round, commit `75839093` (`test(recover): credit rows 41 and 42 to the
promotion kills under the cited test`), adds 26 lines of prose to
`src/engine/topology/recover/tests.rs`, a module whose header is the single pointer
`//! Extended notes: `docs/internals/engine/topology/recover/tests.md`` and whose notes file
exists. Five sites. The line numbers below are those of the reviewed head `7e053e0f` **and** of
`master` at `9bb177ea`, the merge commit of #311: the merge commit's tree is the head's tree
(`30b89bc3` at both), so nothing has moved since the review.

- `:20684`–`:20691`, rustdoc on `fault_at_a_finalization_cell_and_converge`: what one cell of the
  error-return finalization matrix does (the fault, the next resume finalizing and refusing, the
  third resume's directory barrier) and the candidate-prepared pin's two readings under the
  planting's `surplus_candidate_pin` declaration.
- `:20860`–`:20864`, inline in `kill_after_report_before_each_cleanup_step`, at the `continue`
  that skips `Ref.DeleteCandidatePin`: why the site is not a cell of the matrix (a finished run's
  planting completes alpha's promotion, so finalization finds no pin to sweep), that the
  declared-pin test drives a fault at the sweep by name, and that the site's two cells are the two
  kills inside the promotion after the loop.
- `:20902`–`:20906`, the same test, above the two promotion-kill calls: that they are registry
  rows 41 and 42 (`fault_row: t_cand_ref`), a real kill inside `reclaim_after_creation` during a
  real `promote_candidate` resumed by the next incarnation, run under this test's name because
  `coverage.rs` and the registry cite it for both phases of the site.
- `:22614`–`:22618`, rustdoc on `kill_at_a_finalization_cell_and_converge`: one cell of the kill
  matrix, and that the pin's readings follow the planting's declaration as in the error-return
  matrix's cell.
- `:22704`–`:22706`, inline in `kill_at_every_finalization_cell`, at the same `continue`: the same
  exclusion, "for the reason the error-return matrix gives".

Round 4's regression lens on #311 (`gpt-6-astra` at `max`, in a private clone of `7e053e0f`;
`~/orch-pr10/reviews/c2-4/regression-review.md`, first paragraph):

> **P3 — reasoned — nonblocking: `PR311-NEW-RECOVER-PROSE-OUTSIDE-NOTES`.** New helper
> documentation and matrix commentary remain in `src/engine/topology/recover/tests.rs:20684`,
> `:20860`, `:20902`, `:22614` and `:22704`, although this module already has its
> `Extended notes:` pointer and corresponding notes file. These are ordinary contracts and
> explanations, not the site-required exceptions listed in `docs/internals/README.md`. They
> contradict `CODING_STANDARDS.md` §13's source/notes convention and duplicate material already
> added to `docs/internals/engine/topology/recover/tests.md`. Move the prose into those notes and
> remove the duplication, or record it as deferred debt. This has no execution effect and is
> nonblocking under the guidance/MUST distinction in standards §1 and the triage rule in
> `MAINTAINING.md` step 5. Locations are captured in
> [prose-locations.txt](regression-evidence/prose-locations.txt); the pointer gate does not
> enforce the absence of other prose.

Its `regression-evidence/prose-locations.txt` is a `grep -n` of the five sites with their
context; the matched lines, as that file holds them:

```
src/engine/topology/recover/tests.rs:20684:/// One cell of the error-return finalization matrix, driven over `planted`: the fault
src/engine/topology/recover/tests.rs:20860:                // Not a cell of this matrix. A finished run's planting completes alpha's
src/engine/topology/recover/tests.rs:20902:    // `Ref.DeleteCandidatePin` before and after, registry rows 41 and 42 (`fault_row:
src/engine/topology/recover/tests.rs:22614:/// One cell of the kill matrix, driven over `planted`: a child killed at the cell inside a
src/engine/topology/recover/tests.rs:22704:            // Not a cell of this matrix, for the reason the error-return matrix gives: the
```

**The convention contradicted.** `CODING_STANDARDS.md` §13: where a module has a notes file,
"that file carries the whole of the module's prose — contracts, rationale, history, worked
examples — and the source carries a single `Extended notes:` pointer in its module header and no
other comment." `docs/internals/README.md` names the four things that stay at their site (a
`SAFETY:` obligation, a concurrency protocol the type cannot carry, an `#[expect(...)]` reason
string, the allowlist-placement marker above a governed `#![allow]`); none of the five is one of
them. §1 makes the convention guidance rather than a requirement ("**MUST** and **MUST NOT** are
requirements ... Everything else is guidance"), and the finding carries no failing test,
reproduction or mutation witness, so neither of `MAINTAINING.md` step 5's two overriding rules
applies: it is a relevant finding that is not serious, which step 5 lets the author log as tech
debt with a `deferred` row. That is why it is `deferred` and not a fix owed before merge, and why
it must not be filed `open`.

**The duplication.** The same round's second commit, `7e053e0f` (`docs(recover): describe the
cited test's promotion kills and the declared-pin tests`), put the material where §13 places it.
`docs/internals/engine/topology/recover/tests.md` has a section for each of the five items, headed
by the source line as the notes convention requires:
`fn fault_at_a_finalization_cell_and_converge(`, `fn kill_after_report_before_each_cleanup_step() {`,
`fn kill_at_a_finalization_cell_and_converge(`,
`fn kill_at_every_finalization_cell(outcome: &RunOutcome) {`, and, for the promotion kills the
`:20902` comment describes,
`fn a_kill_at_the_candidate_pins_deletion_converges_on_the_next_resume(phase: HookPhase) {`. Each
states what its source comment states, at greater length and with the history the comment omits.
The notes file already carries every contract the five comments state, so removing them loses
nothing.

**Why no gate caught it.** `.github/scripts/test-internals-notes.sh` holds the pointer and the
notes file it names, in both directions, and nothing else; `docs/internals/README.md` says so in
terms: "The gate does not check section headings, arbitrary source prose, or whether a note
remains true. Those are review duties under §13." The prose is a review finding, and round 4's
regression lens is the review that found it.

**What this finding is not.** The module carried 193 lines of `//` and `///` prose beside its
pointer before #311 (at `2b24a377`, the merge base, and at `1c94cbfc`, round 3's parent) and
carries 219 at the head. The count is of lines whose first token is a line comment, read by a
scanner that skips string, raw-string and char literals and follows nested block comments: at
each of those revisions and at `75839093` the module has no trailing comment, no block comment
and no comment-shaped text inside a literal, so the count is the whole of its prose beside the
one `//!` pointer. The set difference between `1c94cbfc` and `75839093`, and between `2b24a377`
and the head, is 26 lines added and none removed, and the 26 are exactly the five sites above.
The 193 are pre-existing and outside this finding: `MAINTAINING.md` step 5 counts a pre-existing
finding as not relevant to the change, and #311 wrote none of them. §13 has no transitional rule
(`standards/SWEEP.md`'s activation rule scopes §6 and §7 only), so they stand under §13 in full,
for the §13 pass the guard names. This finding is the 26 lines round 3 added.

**Consequence.** None in execution. A reader of the code pays for prose the standard put in the
notes, and there are now two copies of each contract: a change to the matrices or the promotion
witness that updates one copy and not the other leaves a stale comment, which §4 calls a defect
and `docs/internals/README.md` extends to notes.

## What the change that takes this up should do

Delete the five comments (26 lines: `:20684`–`:20691`, `:20860`–`:20864`, `:20902`–`:20906`,
`:22614`–`:22618` and `:22704`–`:22706` at `9bb177ea`), and before deleting each, read its notes
section against it; where the comment carries a fact the section lacks, add the fact to the
section. The sections were written in the same round from the same understanding, so the expected
result is five deletions in the source and no addition to the notes. Nothing else changes: no test
body, no assertion, no notes heading, so the `Extended notes:` pointer, the module's tests, and the
citations of `kill_after_report_before_each_cleanup_step` in `coverage.rs` and the sequential
registry are untouched. Run `cargo fmt --check` and `cargo clippy` with the rest of the gates: a
comment can hold a lint at bay (`docs/internals/README.md` records `clippy::collapsible_if` firing
once when a comment between two `if`s was removed). None of the five sits between two `if`s: the
two rustdoc blocks sit above a `fn`, two of the three inline comments sit above a `continue`
inside an `if`, and the third sits above two calls after a loop. The gates decide, not the
reading.

It was not done in #311 because the finding came from round 4's regression lens on the head both
lenses passed, `7e053e0f`, with CI green on it; a source change would have moved the reviewed head
and cost two fresh passes and a CI run to relocate comments with no execution effect. #311 merged
carrying the ledger row (`deferred`, no code change) and not this file, and this file is the record
`MAINTAINING.md` step 5 and `findings/PROCESS.md` §7 require of a deferred finding, filed on a
`findings/` branch that changes nothing outside `findings/`.
