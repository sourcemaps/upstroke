---
id: PR434-NOTHING-HOLDS-ONE-CASE-PER-F-BOUND
severity: P3
disposition: deferred
category: liveness
pr: 434
reviewed_sha: 008c3541be4a2c52bc13650e576e9c9900ddc03b
location: src/engine/topology/coordinator.rs:25883
provenance: pre_existing
first_bad:
guard: the next change that restructures the F tests' bounds, on branch fix-P3/liveness_nothing-holds-one-case-per-f-bound
---

# Nothing holds one case per F bound

Found by #434's review at the reviewed commit, which made two mutations and graded each P2. Ruling r71 classified
what remains explicitly as P3 residual liveness, and it is filed deferred at that grade.

## Failure sequence

    an F test of `src/engine/topology/coordinator.rs` is edited so that its cases no longer each run under a `BOUND` of their own
    -> either several cases run under one `bounded` call, or a case that called `bounded` directly runs with no bound at all
    -> no test fails: nothing in the suite checks that each F case runs under a bound of its own
    -> under a shared bound, a slow runner fails the test while every case is still progressing, and a wedged case is
       reported only as the whole test's timeout, not by its case
    -> with no bound, a wedged case is not reported by its test at all; it runs until the harness or the job stops it

## What the code does

At the reviewed commit, in `src/engine/topology/coordinator.rs`:

- `bounded` (line 11330) runs its body on a thread of its own and panics if it has not ended within `BOUND`, 120 s
  (line 2181). One call is one bound, whatever the body runs.
- `f_bounded_case` (line 25883) runs one case under `bounded`, named by the case, and returns its outcome for
  `f_report`. Of the 21 split tests (the F tests whose cases once shared one `BOUND`), the nine that collect outcomes
  call it once per case.
- The other twelve split tests — the nine `-control` tests, `f-c1-p2-launch-outage`, `f-c2-retained-residue` and
  `f-o14b` — call `bounded` once per case directly and assert inside it. F tests outside the 21 are not in this list;
  among them, `the_launch_premise_names_the_deleted_head_in_either_separator_spelling_and_nothing_else` (F-R1's path
  spelling) collects outcomes and calls `f_report` without `f_bounded_case`, and the single-case `f-residual-i` calls
  `bounded` directly.

One case per bound is a convention those call sites follow, and the notes for `f_bounded_case`
(`docs/internals/engine/topology/coordinator.md`) say why: one `BOUND` around a 12- or 16-case matrix measured how
many cases a test chains and how loaded the runner was (`G6-F-MATRIX-AGGREGATE-BOUND`).

## The two reviewed mutations

- **Shared bound.** Several cases of one test run under one `bounded` call, as a loop over `F_MATRIX` inside it or
  through a helper that runs every case. This is the shape the per-case split removed. Graded P2 by the review.
- **Omitted direct bounds.** A test that calls `bounded` once per case directly drops that call and runs its case
  unbounded. A check that reads only the `bounded` calls present cannot see a call that is absent. Graded P2 by the
  review.

Nothing kills either mutation. The source scanner that read this file's own text for a loop inside an F bound
(`no_f_test_runs_a_loop_of_cases_under_one_bound`, with `f_bounded_calls`) is removed in the change that files this, and its note with it.
It never saw the second mutation, and it saw the first only through markers in the bound's own arguments: it caught
a loop behind a helper call it recognized, such as `f_report(&f_policy_cases(3))` in its `f-control-report` control,
and could miss one behind a helper call it did not recognize.

## Grading

- **The review: P2 for each mutation.** Each one removes the per-case bound and no test fails.
- **Ruling r71: P3, residual liveness, explicitly.** The cases' assertions are unchanged under either mutation. What
  is lost is how soon, and under which name, a slow or wedged case fails its test. This file records the classification
  and does not argue with it.

## What the change that takes this up should do

The next change that restructures these tests' bounds also adds a guard for one case per bound. **It does not parse
source.** One shape it could take:

- **The bound counts its cases at run time.** Every F case enters through one entry point, such as `f_bounded_case`
  for every F test, the direct `-control` callers included. That entry point marks the case on the bound it runs
  under. A second case under one bound panics, naming both cases. A case that finds no bound panics, naming itself.
- **Each mutation becomes a witness.** The witnesses are a shared bound around two cases, and a case run with no
  bound. Each must fail the guard (`findings/PROCESS.md` §5, rule 3).
- **It keeps** every assertion, the 120 s `BOUND`, the cases' order and `f_report`'s read of every outcome. It adds no
  retry, skip or ignore.

When this is fixed, the note for `f_bounded_case` in `docs/internals/engine/topology/coordinator.md` should name the
guard instead of this finding.
