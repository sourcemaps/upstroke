# `src/engine/topology/coordinator/tests/pipelines.rs`

Extended notes for [`src/engine/topology/coordinator/tests/pipelines.rs`](../../../../../../src/engine/topology/coordinator/tests/pipelines.rs).

The code is the authority for what it does; this file is the whole of its prose but for the
concurrency protocol standards §10 places at its site. Each section
here is headed by the line of code it describes, spelled as it is in the source, so the heading is
the grep string that finds the code.

## `fn a_pipeline_error_cancels_the_others_and_ends_the_command_resumably() {`

A pipeline that fails, with `a_panicking_pipeline_ends_the_command_with_a_defined_error`. Since round
R1 each asserts the ledger with `settled_but`: the gate whose process was unresolved, and the gate
the panic unwound through, keep their registrations, and every other invocation is settled once.

## `fn an_unresolved_end_keeps_its_pair_and_no_waiting_invocation_starts_on_it() {`

The early review's `R1-CONC-1` (the working record's §13, round R1). At width two with one slot for
the agent, alpha's worker holds the pair and beta's waits for it. Alpha's process ends unresolved —
the Runner could not establish that it is gone — and alpha reports that end before its error
completion. The phase-3 `Ended` carried no fate, so the coordinator released the pair at once and
granted it to beta, whose process started while alpha's might still run. Now beta's worker never
starts, the command ends with alpha's own error, and alpha's worker keeps its registration and pair.

## `fn an_unreported_end_keeps_its_pair_and_no_waiting_invocation_starts_on_it() {`

`R1-CONC-1`'s other release: the double panics on alpha's thread as alpha's worker ends
(`RecordingRunner::panic_when_released`), so alpha never reports the end and its caught panic is its
completion. `retire` used to withdraw the invocation alpha still held and grant its pair to beta.

## `fn a_fatal_verification_completion_interrupts_on_receipt_without_waiting_for_the_snapshots() {`

The early review's `R1-CONC-2`. Gamma holds a review snapshot, granted inside the verification,
when the verification's body panics (`WidePlans::panic_verifying`); the caught panic is the
verification's completion. The phase-3 coordinator kept it in `arrived` until every attempt snapshot
had drained, so gamma was not cancelled, ran its review to the end and settled first. Now the
coordinator is never asked, with that completion received, to wait for the drain; gamma's held
review is cancelled; nothing is appended after `merge_verification_started`.

## `fn a_declined_embedded_question_stops_the_verification_its_lineage_had_started() {`

The early review's `R1-CONC-3`, in the state the fold's
`declining_an_embedded_question_cancels_its_lineages_unprepared_verification` builds. Beta's
candidate is being verified, its gate held, when a sibling repair of beta parks on an embedded
question and a decline that does not halt is ingested (in the pass gamma's worker ending opens).
The fold fails the lineage and cancels the transaction. The phase-3 coordinator neither cancelled
the verification nor unwound `verify`: it discarded the late completion without setting `arrived`
and then waited for a message nothing would send — a stuck quiescent point with an observer,
`blocking_recv` for ever without one — or, with other work ready, selected that work's integration
inside `verify` and ended the command. Now the verification's gate is cancelled, nothing is appended
for its sequence, and the run goes on to merge gamma. As built in round R1 that held only because
gamma's candidate became eligible after `verify` had unwound; with it queued before the decline, the
pass went on selecting and refused it inside `verify` (round R2,
`a_decline_that_cancels_the_open_verification_goes_on_to_integrate_the_queued_candidate`).

## `fn a_declined_embedded_question_stops_a_running_sibling_attempt() {`

`R1-CONC-3`'s attempt side, in the state the fold's
`declining_an_embedded_question_closes_a_running_sibling_and_refuses_its_late_result` builds: beta's
worker is held when the decline closes beta's in-flight generation. The phase-3 coordinator let
beta's pipeline run on — its gate was registered and run for a generation the fold had closed — and
only discarded its completion. Now beta's worker is cancelled and nothing more of beta starts.

## `fn a_halt_answers_a_request_its_intake_still_buffers() {`

Round R1's class search (the working record's §13): beta reports its worker's end and asks for its gate
snapshot before the deterministic intake applies either, and the pass after the end ingests a halting
decline, so `cancel_all` meets beta with its request still buffered. The phase-3 transition marked
every cancelled pipeline `Running`, and the intake then waited on the inbox for beta, which was waiting
on it. Now the buffered request is applied, refused, and beta's attempt is settled interrupted by the
closure.

## `fn a_stop_answers_a_request_its_intake_still_buffers() {`

The same position through `stop`: the verification reports its gate's end and asks for its review
pair before either is applied, and the pass after the end ingests the decline that fails its lineage.
Without the reconcile the run is stuck; with the reconcile and the phase-3 transition it waits for
ever; as repaired the review request is refused and never starts, and the run completes.

## `fn a_halt_after_the_verifications_result_arrived_interrupts_it_and_publishes_nothing() {`

Review round 2's `R2-CONC4-1`. Gamma is parked on a question and alpha has merged; beta's
verification is the only live pipeline, and its last process, a review, is released with the decline
armed for the pass after the verification's completion. That completion is accepted — nothing is
discarded — and its pipeline retired, so the halt is ingested with no pipeline live and the
transaction still `VerificationStarted`. The phase-4 guard read the live table and did not act:
`verify` returned the result and `integrate()` appended `merge_prepared` and `task_merged` after
`question_answered`. Now the halt interrupts `verify`, and the closure appends
`merge_verification_interrupted`, deletes the pin and reclaims the staging and the sequence's
snapshots before `run_finished`.

## `fn a_decline_that_cancels_the_open_verification_goes_on_to_integrate_the_queued_candidate() {`

Review round 2's `R2-DECLINE-QUEUED`: round R1's verification witness with gamma's candidate queued
before the decline. Alpha merges, which lets delta dispatch; beta's candidate is under verification
with its gate held; gamma runs to its candidate inside `verify`; a sibling repair of beta parks; and
delta's worker ending opens the pass that ingests the decline. The phase-4 coordinator stopped the
verification and went on selecting in the same pass, found gamma's integration eligible with the
transaction gone, and refused it inside `verify` — the command ended. Now the pass ends, `verify`
waits for its cancelled pipeline and unwinds, and gamma's candidate is integrated next.

## `fn with_two_closed_siblings<T>(`

The state review round 2's `R2-RECONCILE-GRANT` names, built on a coordinator the test assembles
itself, since the live engine never runs two members of one lineage at once. The root and an unrelated
holder are dispatched and started; a sibling repair of the root is planted running; the broker, at one
slot per agent and per pool, holds the holder's worker on agent `a`, the root's worker waiting for
`{a, p}` at the head of the queue — reserving pool `p` — and the sibling's waiting for `{b, p}` behind
that reservation; a second sibling parks on an embedded question, and its decline, which does not
halt, closes the root's and the first sibling's attempts. Three live pipelines stand for the three
attempts and the two requests wait on their replies; `body` acts on the coordinator before its next
admission pass, and what each reply was answered with and what the observer was handed as granted come
back.

## `fn stopping_two_closed_pipelines_grants_neither_the_pair_the_first_one_frees() {`

`R2-RECONCILE-GRANT`: `reconcile` stops the root first, and withdrawing its request frees the pool the
sibling waits for. The phase-4 coordinator sent the sibling the grant before it stopped the sibling,
whose process could then start for a closed identity. Now the grant is withdrawn undelivered, both
requests are refused and settled cancelled, and the holder keeps its pair.

## `fn a_pipeline_whose_identity_closed_is_granted_nothing_before_it_is_stopped() {`

`R2-RECONCILE-GRANT`'s class: every other place a grant is delivered, reached in the step between an
append that closes an identity and the admission pass that stops its pipeline — the order the
production intake keeps and an observer's `Release::Append` does not. The closed sibling's gate
registration, its snapshot request, its queued snapshot when the gate opens, and the pair the holder's
end frees (granted to the closed root, then, withdrawn, to the closed sibling) are all refused; the
phase-4 coordinator granted every one of them.

## `fn with_a_waiter_behind_a_decline<T>(`

The state review round 3's `R3-HALT-GRANT` names, on a coordinator the test assembles itself, as
`with_two_closed_siblings` does. Four unrelated attempts — the root, a holder, a waiter and an asker —
are dispatched and started; the broker, at one slot per agent and per pool, holds the holder's worker
on agent `a`, the root's worker waiting for `{a, p}` at the head of the queue — reserving pool `p` —
and the waiter's for `{b, p}` behind that reservation. A sibling repair of the root parks on an
embedded question, and its decline, ingested through `ingest_answers`, fails the root's lineage: it
closes the root's attempt and none of the other three. `Decline::Halting` has the decline halt the run;
`Decline::UnderABudgetStop` has it not halt and then plants a fold-valid `budget_exceeded`, since the
fold refuses a decline while a budget stop is current. Four live pipelines stand for the four attempts
— the root and the waiter awaiting their replies, the holder inside its invocation, the asker running —
and `body` acts on the coordinator before its next admission pass. The sibling is planted, as in
round R2's witnesses: in this build's live engine a root whose repair exists awaits that repair and is
never in flight beside it.

## `fn a_recorded_halt_withdraws_every_waiting_request_before_reconciliation_frees_a_pair() {`

`R3-HALT-GRANT` through the production path: the admission pass that follows the halting decline. It
acts on the halt before it reconciles, so every pipeline is cancelled, both waiting requests are
withdrawn while pending and refused, the broker grants nothing (its grant count is the holder's
alone before and after the pass) and nothing is handed out; no warning names a withdrawn grant, since
none was computed; the holder's, the waiter's and the asker's attempts, in flight, are vouched for the
closure; and only the holder's invocation runs, to be released by its own end. The round R2
coordinator reconciled first: withdrawing the root's request freed pool `p`, and the waiter — open,
uncancelled, with no interrupt yet recorded — was sent the grant, and could start its process after
the halt was durable.

## `fn a_budget_stop_drains_so_the_pair_reconciliation_frees_reaches_an_open_waiter() {`

Its control, under a budget stop: the same pass records no interrupt, stops only the root, whose
identity the decline closed, and grants the waiter the pair the root's withdrawal frees — the drain of
a budget stop goes on granting live, open pipelines.

## `fn grant_sites(`

Every other place a grant is delivered, reached in the step before the next admission pass — the step
an observer's `Release::Append` opens: the asker registers its worker for `{c}` (free, and clear of
the head's reservation), asks for a snapshot, waits for one while the gate is closed and is queued
when it opens, and the holder's end frees agent `a`, which the broker grants to the root at the head
of the queue and, that grant withdrawn from the closed root, to the waiter. It returns what each of
the asker's three requests was answered with and how many snapshots the gate holds.

## `fn no_grant_reaches_an_open_pipeline_between_a_recorded_halt_and_the_pass_that_acts_on_it() {`

`R3-HALT-GRANT`'s class: with the halt recorded and not yet acted on, every one of those grants is
refused although every pipeline asking is open — the asker's registration (never registered), its
snapshot request, its queued snapshot (no snapshot held), and the pair the holder's end frees, to the
closed root and then to the open waiter; nothing is handed out, and nothing is pending or running. The
round R2 coordinator granted all of them but the root's.

## `fn under_a_budget_stop_every_grant_site_still_grants_an_open_pipeline() {`

Its control: under a budget stop every one of those grants is delivered — the asker's registration,
its snapshot and its queued snapshot, and the freed pair to the waiter — and only the closed root is
refused.
