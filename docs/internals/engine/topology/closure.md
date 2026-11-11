# `src/engine/topology/closure.rs`

Extended notes for [`src/engine/topology/closure.rs`](../../../../src/engine/topology/closure.rs).
[Source on GitHub](https://github.com/sourcemaps/upstroke/blob/master/src/engine/topology/closure.rs).
The relative link works in a checkout or on GitHub; the GitHub link also works from the published site.

The code is the authority for what it does. The explanatory prose is preserved below.
Each backticked part of a section heading is an exact source excerpt. Search for the final
excerpt within the preceding item when a heading names both an item and a line inside it.

## Module

Run-end closure: the pure half of `run_end_policy`'s closure procedure. `TopologyRun::close_run`
(`run.md`) is the acting half; everything here reads a fold and answers a question about it,
appends nothing and touches no file.

`decisions.run_end_policy.closure_procedure`, in order: (1) the ending outcome is derived —
halt, then budget, then the fold's own `derived_outcome`; (2) in-flight attempts and
verifications are settled interrupted at Halted (a budget stop drains them instead); (3)
promoting generations are promoted; (4) authorized publications are completed; (5) every open
generation is closed `RunEnding { outcome }` with its worktree scrubbed; (5b) deferred items
stay as they are (void at Halted, resumably open at BudgetExceeded); (6)–(8) provisional
reservations are cancelled, the derived outcome is confirmed against the closed fold and
`run_finished` is appended.

**Step (2) settles only what its caller vouches for** (the working record's R-AF, PR11 phase 4).
The coordinator hands the closure the in-flight identities whose pipelines it cancelled and saw
end ([`Cancelled`]); anything else in flight is refused, appending nothing. The synchronous loop
and the coordinator's idle closure vouch for nothing, which keeps PR10's refusal for them: the
synchronous loop never leaves an attempt or a verification in flight when it reaches closure, and a
fresh process's recovery steps (d) and (f) settle a dead coordinator's before the loop runs.
**Steps (3) and (4) are completed wherever the closure finds them** (R-AG): no coordinator schedule
reaches either at a live end, because a promotion and a publication each run from their first
append to their last on the coordinator's thread, and recovery step (f) completes one an error left.

## `pub fn ending_outcome(fold: &TopologyFold) -> Result<RunOutcome, UpstrokeError> {`

Step (1), read **before** any generation is closed (record §3 R3). A halting settlement
outranks a budget stop and both outrank the fold's derivation, exactly as `derived_outcome`
orders them; the difference is that the fold derives `NotEnding` while a retained or open
generation blocks `common`, and a budget-stopped run with a retained generation is precisely
the shape closure exists to end (`PR7-R4-LOOP-004`). Halted and BudgetExceeded are therefore
read from `halted_at` and `budget_stop` directly, and only Parked and Complete come from the
derivation — which for those two already requires every generation closed.

`NotEnding` here is refused with [`blockers`]'s diagnostic, naming the retained generation,
the deferred task or the admissible work rather than the derivation's arm; an unstarted run is
refused before anything is derived.

## `pub fn refuse_unclosable(fold: &TopologyFold) -> Result<(), UpstrokeError> {`

Step (2) for a caller that vouches for nothing: every in-flight attempt and started verification
is refused, before any append. [`settleable`] with an empty [`Cancelled`] refuses the same work
with the same sentence, which keeps the words the frozen
`closure_refuses_an_in_flight_generation_and_an_unresolved_transaction_before_any_append` holds
(`recover/tests.rs`: "is in flight", "is unresolved", "PR11", "nothing was appended").

## `pub fn unclosable(fold: &TopologyFold) -> Vec<String> {`

The in-flight work [`in_flight`] finds, one line each, so the refusal and the diagnostic read the
same list. A promoting generation and a prepared transaction are no longer here: the closure
completes them (steps (3) and (4)), and [`blockers`] still names them.

## `pub enum InFlight {`

Step (2)'s work: an attempt in flight (its key, generation, attempt number and the lease its
interruption records, by kind, as recovery records it), or a started verification (its sequence,
its candidate's task, and — for a stale-clean basis — the pin and the proposal it pins). A
verification also records whether a lineage failure cancelled it (`cancelled`).

## `impl InFlight` › `pub fn describe(&self) -> String {`

The line a refusal or a diagnostic names it by.

## `impl InFlight` › `pub fn interrupted(&self) -> TopologyEventBody {`

The terminal a halt appends for it: `attempt_interrupted` or `merge_verification_interrupted`, each
saying the run halted. An attempt's says the coordinator cancelled its pipeline and the Runner
terminated its processes before the terminal was appended. A verification's says its pipeline had
ended — cancelled by the coordinator, or with a result the halt discards unprepared, since a halt
recorded after the result arrived and before `integrate()` prepared it still interrupts it (the
working record's round R2) — and that the Runner had established the end of each of its processes.
Either is what vouching for it means.

A verification a lineage failure cancelled gets its own detail, because "the candidate stays
queued" would be false for it: a decline, or a lineage member's failed settlement, failed its
lineage and cancelled it; its pipeline had ended, stopped by the coordinator or with a result the
cancellation discards, and the Runner had established the end of each of its processes; nothing was
published, and its candidate, whose task failed, is not verified again. The coordinator's own
settlement of a cancelled verification and a halt's closure append that same terminal.

## `pub fn in_flight(fold: &TopologyFold) -> Vec<InFlight> {`

Every in-flight attempt in key order, then the started verification if one is open — recovery's
order, step (d) before step (f). The verification's `cancelled` is [`cancelled_by_lineage`].

## `pub fn cancelled_by_lineage(fold: &TopologyFold, sequence: SequenceId) -> bool {`

Whether the open transaction is `sequence`'s, is `VerificationStarted`, and its candidate's task is
`Failed`: the whole of what the fold holds for a verification a lineage failure cancelled, since
`fail_lineage` keeps that transaction open until its one terminal. The coordinator's `open_in`
reads it to treat the verification as closed, and [`in_flight`] to choose its terminal's detail.

## `pub struct Cancelled {`

The in-flight identities a coordinator vouches for: attempts by key, generation and attempt, and
verifications by sequence. The coordinator records each identity when it cancels its pipeline, and a
verification's sequence when an interrupt ends `verify`, whether or not its result had arrived; it
hands the set over only after every pipeline has ended with its termination established
(`coordinator.md`, `finish`).

## `impl Cancelled` › `pub fn vouches(&self, item: &InFlight) -> bool {`

Whether `item` is one of them.

## `pub fn settleable(`

Step (2)'s gate. Every in-flight item must be vouched for, or the closure is refused naming the
ones that are not, before any append. Vouched in-flight work outside a Halted ending is refused
too: a budget stop drains its pipelines to their natural settlements and never cancels one, so a
cancelled identity at a BudgetExceeded end is a coordinator defect, not work to settle.

## `fn unvouched_refusal(found: &[String]) -> UpstrokeError {`

The one sentence both refusals use.

## `pub fn promoting(fold: &TopologyFold) -> Vec<TaskKey> {`

Step (3)'s selection: every task holding a `Promoting` generation, in key order.

## `pub fn closable(fold: &TopologyFold) -> Vec<TaskKey> {`

Step (5)'s selection: every task holding an `OpenNoAttempt` or `RetainedIdle` generation, in
key order. The loop closes each with `close_generation` (`dispatch.rs`), appends the close, and
scrubs the slot after the append — the order `G4B-O3` fixed for the live `Close` arm.

## `pub fn confirm_derived(fold: &TopologyFold, outcome: &RunOutcome) -> Result<(), UpstrokeError> {`

Step (6)'s check: with every closable generation closed, the fold must now derive exactly the
outcome step (1) read. A disagreement is refused rather than appended, because
`check_run_finished` would refuse the append anyway and a refusal here names both outcomes.

## `pub fn run_finished(fold: &TopologyFold, outcome: RunOutcome) -> RunFinished4 {`

The event, with `merged` and `parked` derived from the fold the way the report derives them
(`report::merged_and_parked`). Neither count is validated by the fold on replay (record §3
R5): the fold checks the outcome and `halted_at`, and the counts are a projection.

## `pub fn blockers(fold: &TopologyFold) -> Vec<String> {`

Why a fold is not ending, for an operator: the in-flight work first, then every open, retained or
promoting generation, every deferred task, a prepared transaction not yet published, every
verification-deferred candidate, and structurally admissible work. The last line, "of a state this module cannot name", is the
`DerivedOutcome::FoldError` arm's; the totality census asserts that arm is never reached in the
fold's derivation over every explored state (`the_derived_outcome_is_total_over_every_explored_state`),
and says nothing about this function's text.
