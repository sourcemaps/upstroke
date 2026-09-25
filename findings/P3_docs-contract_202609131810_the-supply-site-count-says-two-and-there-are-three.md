---
id: PR282-SUPPLY-SITE-COUNT-SAYS-TWO-AND-THERE-ARE-THREE
severity: P3
disposition: deferred
category: docs-contract
pr: 282
reviewed_sha: 5ca7abcd3104f92923ca76389e96f54131fa04e3
location: src/rundir/discovery.rs:150
provenance: introduced
first_bad: 5ca7abcd3104f92923ca76389e96f54131fa04e3
guard: "deferred: both sentences must say three and name the three supplies; whoever edits them should re-derive the count by grepping for the production calls rather than trusting the prose, and the module's own census test already asserts three"
---

## Failure sequence

Two doc comments in `src/rundir/discovery.rs` state that production supplies
`classify_run_dir` at **two** sites:

- `:150` — *"this is one of the **two** bodies that supply it"*
- `:162` — *"Production supplies [`classify_run_dir`] at the **two** call sites that name it"*

**There are three.** Verified by grep over the production region at `5ca7abcd3104f92923ca76389e96f54131fa04e3`
(the first `#[cfg(test)]` in this file opens at `:839`, so all three are production):

1. `:156` — `census` → `census_observed(repo_root, &mut classify_run_dir)`
2. `:428` — `resolve_run_id` → `resolve_observed(repo_root, wanted, &mut classify_run_dir)`
3. `:754` — `find_question` → `find_question_observed(repo_root, wanted, &mut classify_run_dir)`

The module's **own census test already says three** — *"the import and the three supplies"*, asserting
the mention count is 4 — so the two prose sites are stale from a draft written before
`find_question_observed` was added in this round. The test and the prose disagree, and the test is right.

No behavioural surface: nothing compiles, executes or gates on these sentences. The census test
counts `classify_run_dir` **mentions**, not supply sites, so it does not fail on this text.

Found by the `moonshotai/kimi-k3` fix-check lens; **not** raised by either `tencent/hy4-preview`
half, so this is a single-lens finding rather than a converged one.

## Why this is filed rather than fixed

Fixing it means editing a tracked source file, which moves the head and **discards a three-lens
review of record over two words**. `ORCH-P1.md` step 4 is explicit: P3s are fine, and no blocking
P1s and no goal-blocking P2s ends the loop — **no clean-up round**. Step 5 files residue under
`findings/`, where a push confined to that directory keeps the review.
