# `src/engine/topology/run/tests.rs`

Repository source for these notes: [`src/engine/topology/run/tests.rs`](../../../../../src/engine/topology/run/tests.rs).
[Source on GitHub](https://github.com/sourcemaps/upstroke/blob/master/src/engine/topology/run/tests.rs).
The relative link works in a checkout or on GitHub; the GitHub link also works from the published site.

The code is the authority for what it does. The explanatory prose is preserved below.
Each backticked part of a section heading is an exact source excerpt. Search for the final
excerpt within the preceding item when a heading names both an item and a line inside it.

## Module

The loop's branches, checked against the packet's list rather than against
the implementation.

## `fn the_transcribed_loop_branches_are_the_packets_seven() {`

The transcribed list is the packet's list — seven branches, these labels, in
this order.

`decisions.sequential_substrate.loop` names them in one sentence, split on
`->`. A branch dropped from [`LoopBranch::ALL`] would make every other test
in this file pass by asking for less, which is exactly how step (g) survived
two review rounds in `recover.rs`.

## `fn every_branch_states_what_this_build_does_with_it() {`

Every branch this build does not perform says which, and why, in the type.

**The point of this test is the third disposition.** `RefusedByCheckpoint`
is a decision the packet licenses; `NotYetImplemented` is debt. A build that
conflated them would be indistinguishable from one that had quietly dropped
a branch — and "quietly dropped a branch" is the defect this whole module
exists because of.

## `fn every_branch_states_what_this_build_does_with_it()` › `let owed: Vec<&str> = LoopBranch::ALL`

And the debt, named rather than implied. This assertion is expected to
shrink as branches land; it must never grow.

## `fn every_branch_states_what_this_build_does_with_it()` › `let elsewhere: Vec<&str> = LoopBranch::ALL`

**What is another slice's, cited rather than owed — and today nothing
is.** `ingest answers` carried the contract passage that assigned it to
PR9 for as long as it was PR9's; PR9 performs it, so the set is asserted
empty. A branch that returns to another slice must come back with the
passage that assigns it, not as debt and not as a checkpoint refusal,
of which the packet authorises exactly one (run end).

## `fn every_branch_states_what_this_build_does_with_it()` › `assert_eq!(`

The half-built one, and both halves in the branch's own words. A branch
that performs a durable append and reports `NotYetImplemented` would be
claiming the log is untouched when it is not; one that reported
`Performed` would be claiming an attempt ran.

## `fn every_step_belongs_to_one_branch_or_to_none_for_a_reason() {`

Every `Step` a selection can produce maps to exactly one branch, or to none
for a stated reason.

The mapping is total by construction — `LoopBranch::of` matches on `Step`
exhaustively, so a new variant does not compile until someone decides which
branch it belongs to. What this test adds is the *two `None` arms*, which a
compiler cannot check: they are the claim that neither is a branch of the
loop, and each is wrong in a different and specific way if the claim slips.

## `fn a_refusal_names_the_branch_and_says_whether_anything_happened() {`

A refusal says which branch, and — this is the part that matters — whether
anything happened.

**The two messages must not be interchangeable.** A branch that performed
nothing says so, and an operator reading it knows the log is untouched. A
branch that appended and then stopped says what it did, because an operator
told "not implemented" after a durable `task_dispatched` would go looking
for a run directory that does not match the message.

## `fn a_refusal_names_the_branch_and_says_whether_anything_happened() {` › `assert!(`

**No branch is `PartlyImplemented` today**, and that is a statement about
this build rather than about the type. `ReadyRetry` was the last one and
became `Performed` when its second half landed. The variant stays because
the next branch built in halves will need it, and this assertion is what
says so out loud the moment one appears — a half-built branch is the one
shape whose refusal has to say what it already did, because by then
`attempt_started` or `task_dispatched` is durable.

## `fn a_refusal_names_the_branch_and_says_whether_anything_happened() {` › `for branch in LoopBranch::ALL {`

Every refusal names its own branch, whatever its disposition. A message
that named the wrong one would send an operator to the wrong lane.

## `fn every_driver_append_propagates_its_error() {`

**Every append the driver makes propagates its error.**

The append-error protocol is five obligations, and all five begin with the
error *reaching* the protocol. A `let _ = self.emit(..)` reaches none of
them: the fold is not poisoned, no reservation or invocation is cancelled,
and the command reports success for a run whose log does not contain the
line it just claimed to write.

Catalogue entry `PR7-SELECT-026` did exactly that to the
`Admitted::BudgetExceeded` arm and the whole suite stayed green, because the
arms whose append failure *is* armed by a fixture are not that one.

A **census rather than a fixture per arm**, for the reason the other four
single-authority censuses exist: a per-arm test proves the arm it names and
says nothing about the arm added next week. This proves the property over
every append site the driver has, including the ones not yet written.

The region is [`crate::effects::production_code`], which blanks comments and
strings — a `let _ = self.emit(` quoted in a doc comment must not fail this,
and a truncating region would let a site below the cut through, which is
`PR4-CENSUS-COMMENT-ORACLE` and is how the barrier census scanned 4.7% of
this very file.

## `fn every_driver_append_propagates_its_error()` › `let mut depth = 0_i32;`

Walk to the matching close paren, then check what follows it.

## `fn the_loop_selects_through_one_function() {`

**The loop chooses its branch through one selector.**

`decisions.sequential_substrate.loop` gives seven branches in one order, and
`select` is where that order lives. Catalogue entry `PR7-SELECT-015` added a
**second** selector — `select_rescan`, ordered Dispatch/Retry/Integrate
instead of Integrate/Retry/Dispatch — pointed `TopologyRun::step` at it, and
left canonical `select` untouched with every one of its tests still passing.
The whole suite was green.

That is the seams category in its purest form: `select.rs` is coherent,
`run.rs` is coherent, and the branch order the packet specifies is not the
one the run takes. No per-function test can see it, because each function is
right about itself.

The fifth single-authority census this slice owns, and the cheapest: the
driver reaches its branch order through exactly one call, and `checkpoint`
guards exactly that call's result. A second selector makes this count zero,
not two — which is why the assertion is on the **canonical** name rather than
on a total.

## `fn the_loop_selects_through_one_function()` › `let calls = |needle: &str| {`

Calls, not definitions — neither is defined here, but the filter is the
one the barrier census learned to use and costs nothing.

## `const DRIVEN_TRANSITIONS: [(&str, usize, usize); 5] = [`

The five transitions both drivers run, with how many times each driver calls
each: `step` settles twice (the retry arm and the dispatch arm) and closes the
run twice (its closure arm, and its hard block falling through with the
operator it was handed); the coordinator closes the run twice (idle, and a
halt's `finish`).

## `fn transition_calls(code: &str, transition: &str) -> Vec<(String, bool)> {`

Every call of `transition` in blanked production code, as its first argument
with its whitespace collapsed — so a call rustfmt splits after its parenthesis
reads as one that it does not — and whether it was called as a method. A name
that ends a longer identifier is not a call of it, and the generic definition
(`transition<O`) is not one either.

## `fn transition_bypasses(run: &str, coordinator: &str) -> Vec<String> {`

What the census reports: a transition not defined exactly once generic over
its operator, a driver that calls it another number of times than
[`DRIVEN_TRANSITIONS`] says, or any one call whose operator is not the
driver's own — `&mut self.stepping(seams, hooks)` or the operator a generic
transition was handed in `run.rs`, `self` in `coordinator.rs`.

## `fn driver_sources() -> (String, String) {`

The two drivers' production code, blanked, with their line endings normalised.

## `fn both_drivers_run_each_transition_through_the_one_generic_function() {`

**Both drivers run each transition through the one generic function** (R-T,
follow-up B's repair round 3; every call site since repair round 6, I2-4).
`begin_dispatch`, `begin_retry`, `settle_judged`, `close_run` and `hard_block`
are each defined once, generic over the operator that runs them, and **every
call** of each, in both drivers, hands it the driver's own operator: the
width-1 `step` its [`Stepping`], the coordinator itself. Until repair round 6
the census asked only that one such call exist, so one settlement handed a
foreign operator beside a correct one passed it, and the whole suite with it
(the delta review's R9). Its domain is the drivers' production code, which
the census checks reaches the functions it must (`step`, `idle`, `finish`,
`registry_pause`).
`coordinator::tests::every_append_of_the_width_one_step_a_retrys_settlement_included_folds_through_the_callers_hooks`
is the behavioural witness, and
`coordinator::tests::the_width_one_step_runs_the_same_transitions_and_its_access_waits_by_sleeping`
the control.

## `fn the_r_t_census_reports_one_call_that_bypasses_its_driver() {`

The census's positive control (standards §12): the real sources pass it; the
reviews' two spellings of the single-call mutation — the retry arm's
settlement handed a `Stepping` with fresh `NoTopologyHooks`, written as a
literal or through `stepping` — are each reported once, under either
spelling; so is the coordinator's settlement handed a `Stepping` of its own,
and a closure called as the run's own method.

## `fn driver_sources() -> (String, String) {` › `let read = |file: &str| {`

Line endings are normalised before the source is blanked. rustfmt splits the
`step`'s call of `begin_dispatch` after its parenthesis, so that needle spells
a line break, and a checkout with Windows line endings holds `\r\n` there:
without the normalisation the census fails a source that satisfies it. It did
on `test (winguest)` at `f9c88fdb` (follow-up B's R8), while every Linux and
macOS leg passed.

## `fn the_frozen_pool_table_is_read_through_one_seam() {`

**The frozen pool table is read through one seam.**

`AttemptPlans::pool_for` exists so that the plan builder, the reviewer
profile and the driver's `RetryRequest` reach one answer. `79cd9c8` said it
gave the rule "one production implementation" and it did not: `assembly.rs`
called `crate::capacity::pool_for` from three places, two of them
character-for-character copies of the seam's body, and the seam's only caller
was `run.rs`. `reviews/FINDINGS.md` §19, claim (4).

**Who asks the seam has changed since, and the count has not.** An agent may
have several pools now. The plan builder is handed the pool its attempt binds
(`PlanRequest::pool`), the retry takes the retained attempt's from its record,
and the reviewer profile is the seam's remaining production caller.
`assembly.rs` still names `capacity::pool_for` once. Its `pools_for` lists an
agent's pools by walking the table, which is the kind of unnamed resolution
this note says, below, that the census cannot see.

**The needle is a free call to `pool_for`**, through the shared
[`crate::effects::census_domain::production_calls`]. It was the literal
`capacity::pool_for(`, which reasons about one direction only — a longer
identifier colliding with it — and not about the other: `use
crate::capacity::pool_for;` followed by a bare `pool_for(...)` is the
ordinary way to write a second implementation and that literal does not
match it. Both spellings are already live in this tree. `R5-SEAMS-002`.

**What it still cannot see, stated rather than left to be found**: a second
resolution that never names the function. `capacity::pool_for` is
`pools.iter().find(…)`, and a caller walking `self.pools` inline is a second
implementation of the rule with no `pool_for` in it. A name census cannot
reach that, so what this asserts is **one named resolution**, not one
resolution.

**The count is one and not zero.** Zero would mean the seam had been rewritten
to resolve pools some other way, which is the same defect from the other
side, so the assertion is an equality.

## `fn the_frozen_pool_table_is_read_through_one_seam()` › `use crate::effects::census_domain::{Call, production_calls};`

**Free calls to `pool_for`, not the qualified spelling.** The needle was
the literal `capacity::pool_for(`, which does not match the ordinary way
to write a second implementation — `use crate::capacity::pool_for;` and
then a bare `pool_for(...)`. Both idioms are live in this tree
(`config.rs` writes the qualified form, `capacity.rs` the bare one), so
it is not a hypothetical spelling. `R5-SEAMS-002`, `PR7-R5-ATT-002`.

`Call::Free` is what separates a second implementation from the seam's
own callers: the plan builder and the reviewer profile ask
`self.pool_for(...)`, a method call, and the trait method's definition is
filtered as a definition.

## `fn the_frozen_pool_table_is_read_through_one_seam()` › `assert_eq!(`

Controls on the needle itself, both directions, because a needle that has
stopped matching reads exactly like a clean file.

## `fn both_attempt_started_arms_take_their_pool_from_an_authority() {`

**Both arms of `attempt_started` get their pool from an authority.**

`attempt_started` is appended from two places and they reach it differently:
the dispatch arm builds its plan first and reads `plan.pool`; the retry arm
appends **before** its plan exists, because `settle::retry` produces the
event and the plan is built after. Sol's `R3-SEAMS-001` is what that
asymmetry produced — the retry passed `pool: None`, so a resumed run's ledger
recorded no pool while the plan it then built resolved one, and the two
disagreed about the same attempt.

The needle is the field's value in each production `AttemptStarted4` literal.
A hard-coded `None` fails; anything that names something does not, because
this census's claim is "not invented here", not "non-empty".

The retry arm's authority is the retained attempt's own record. `SITES` said
the driver filled `request.pool` from `AttemptPlans::pool_for`, the agent's
first pool, and that was true until a dispatch could bind any of its agent's
pools. A retry resumes a session, so it is announced on the pool that session
ran on (`TopologyRun::retry_request`), and the description says so. What the
census reads is unchanged: `settle.rs`'s literal still reads `request.pool`.
`a_retained_retry_resumes_on_the_pool_its_session_ran_on` is the behavioural
witness of the value.

### Two corrections to what this test was said to be

**It is not the only witness available, and the claim that it was is false.**
`79cd9c8`'s message argued a source census was structurally necessary because
"a retry is only reachable *within* one process … and **no driver fixture can
reach the arm**". One does: the fixture is
`recover::tests::the_retaining_incarnation_retries_in_place`, and it exists —
**named, not cited by line**. The first draft of this block quoted
`recover/tests.rs:5488` as terminal output — correct **at `c01a844`** — and the
very next commit inserted nineteen lines above it. `PR7-R6-ATT-003`, and
the rule it gives: a doc comment names an item, because a line number is a
claim about a version of a file and decays silently. The doc-comment filter
(`| grep -v '///'`) is the other half — a needle quoted here would otherwise
match its own quotation, `reviews/FINDINGS.md` §4.

It drives `TopologyRun::step` twice in one process and the second iteration
**is** the retained-generation retry. It now asserts the pool on both
`attempt_started` appends, which is the behavioural witness this census was
offered in place of. `reviews/FINDINGS.md` §19, claim (3).

**And this census does not read the file the defect was in.** The two sites
below are `attempt.rs` and `settle.rs`; the literal `None` that
`R3-SEAMS-001` found was in **`run.rs`**, which fills `settle::retry`'s
`RetryRequest`, and `settle.rs`'s own literal reads `request.pool` and was
correct throughout. Measured at `5a08f19`: restoring `pool: None` in
`run.rs` leaves this census green **and the entire suite green** — 1698 + 8
passed, 0 failed. The behavioural assertion above is what kills it. §19,
claim (2).

So this census keeps a real and narrower job: the two *literals* name an
authority rather than inventing a value. It is not a witness that the value
arriving at them is right.

**Its domain is asserted** (follow-up B's repair round 6, the delta review's
census audit). Until then it read the first `AttemptStarted4` literal of each
file, so a second literal in either file, or a third arm in another, went
unread while the test said "both arms". It now walks every production source
of the crate (whole-file test modules skipped), requires that exactly these
two files construct the event, once each, and reads every literal it finds;
the counter's own fixture shows a second literal counted and the definition
not.

## `fn attempt_started_constructions(code: &str) -> Vec<usize> {`

Where `code` constructs `AttemptStarted4`: every `AttemptStarted4 {` that is
not the struct's definition.

## `fn the_settled_notes_separate_the_successful_and_the_failed_settlement() {`

`PR160-NOTES-SUCCESS-SETTLEMENT`. The `Progress::Settled` section of
`docs/internals/engine/topology/run.md` summarised the whole ready-dispatch
branch as ending in `attempt_finished`. That is only the rejected half.
[`TopologyRun::settle`] opens with `let Some(failure) = judgement.failure ...
else`, so an attempt nothing rejected takes [`TopologyRun::promote_candidate`]
before any failure settlement is built, and that path appends
`candidate_prepared` then `task_candidate_created` and no `attempt_finished`
at all. The fold enforces the same rule — `check_attempt_finished` refuses
`SettlementTransition::Succeeded` outright — and so do
`design/15_design_event_log_resume_run_layout.md` and
`design/26_design_merge_queue_protocol.md` §26.

So a reader using that contract to reconstruct a successful attempt's durable
record was sent looking for an event that is never written. This pins the two
settlements separately, and refuses the retired sentence by name so the claim
cannot come back under a reflow.

**It is a text pin and only a text pin.** The behaviour it describes is held
elsewhere — `recover::tests::the_driver_carries_an_accepted_attempt_through_the_candidate_sequence`
for the successful durable sequence, and the fold's
`candidate_prepared_is_the_sole_successful_settlement` for the settlement
contract. This one asserts that the prose agrees with them; `src/export.rs` and
`agent/proc/tests.rs` pin the sentences they own the same way.

## `fn the_settled_notes_separate_the_successful_and_the_failed_settlement()` › `let settled = settled.split_whitespace().collect::<Vec<_>>().join(" ");`

Match on the prose, not on where its line breaks fall: a reflow must not break
the pin, only a changed claim.

## `fn the_ready_branch_notes_do_not_owe_the_attempt_the_branch_runs() {`

`PR160-NOTES-INCOMPLETE-BRANCHES`. Both ready branches' sections of
`docs/internals/engine/topology/run.md` opened with the intermediate build's
paragraph and closed with the current one, so each described the branch as
half-built and then, two sentences later, as whole. The ready-dispatch
section said the first three of its four clauses were performed and the run
was left at `OpenNoAttempt`; the ready-retry section said running and
settling the retry was "the half still owed". Neither has been true since
`0eaf6b07` and `59683dc3`: [`TopologyRun::step`] calls
[`TopologyRun::attempt`] then [`TopologyRun::settle`] before returning
`Progress::Settled`, and [`TopologyRun::retry_ready`] reaches the same two on
`RetryOutcome::Start`. Both commits added the corrected paragraph and left
the one it superseded standing, and the migration into these notes carried
both across.

A reader taking either section as the contract would look for a state the
branch does not stop in, and — the sharper cost — would conclude the driver
does not settle, which is the one thing `decisions.sequential_substrate`
requires of it.

**It is a text pin and only a text pin.** The behaviour is held elsewhere:
`recover::tests::the_driver_takes_over_from_the_recovery_order_and_steps` is
the dispatch branch running an attempt through to `Progress::Settled`, and
`recover::tests::the_retaining_incarnation_retries_in_place` is the retry
branch doing the same over two iterations of the loop. This asserts that the
prose agrees with them, and refuses each retired sentence by name so the
claim cannot come back under a reflow — the same shape as
`the_settled_notes_separate_the_successful_and_the_failed_settlement` and as
the sentence pins in `src/export.rs`.

The `PartlyImplemented` section is pinned with them because it carried the
third instance of the same class: it gave the ready-dispatch branch as a
*present* example of an honestly half-built branch, while
`a_refusal_names_the_branch_and_says_whether_anything_happened` two hundred
lines above asserts that no branch is `PartlyImplemented` at all.

Measured over the notes as this commit leaves them: six mutations, one per
pin — each retired sentence restored, each stated proposition removed — and
all six killed, against an unmutated control that passes.

## `fn the_ready_branch_notes_do_not_owe_the_attempt_the_branch_runs()` › `assert_eq!(`

**The pins are conditioned on the code, not asserted beside it.** A section
is only required to describe a whole branch while its arm reads
`Disposition::Performed`; a branch that became half-built again would need
the opposite prose, and this says so at the point where the two claims
diverge rather than leaving a stale pin to fail with a message about
Markdown.

## `mod scaffold_runner {`

The substrate's runner double, `scaffold::RecordingRunner`, is what PR11's coordinator tests drive
(`tests_acceptance.determinism`: "FakeRunner with explicit start/complete control, recorded
RunnerRequests, InvocationIds (incl. shell and agent probes), and the declared RunnerPolicy and
image id each invocation executed under … and to fail a shell or agent probe on demand"). These
tests hold the double to that sentence before anything leans on it.

## `mod scaffold_runner` › `fn the_scaffold_runner_holds_each_invocation_until_the_test_completes_it_in_any_order() {`

Three invocations on three threads all start and wait; the test delivers their results in reverse
order, and each gets its own; the endings are recorded in the order the test delivered them; every
request is recorded with its identity.

## `mod scaffold_runner` › `fn the_scaffold_runner_refuses_and_counts_a_second_completion_and_an_unknown_one() {`

A completion is delivered exactly once: a second delivery for one invocation, and a delivery for an
invocation the runner never held, are refused and both counted, and the first delivery stands.

## `mod scaffold_runner` › `fn the_scaffold_runner_ends_a_cancelled_or_dropped_invocation_exactly_once() {`

A held invocation whose call is cancelled from another thread resolves cancelled with fate `Gone`,
and a completion after that finds nothing to complete; a held invocation whose future is dropped
releases its hold and is recorded abandoned; a call already cancelled when it reaches the runner is
cancelled with fate `NeverStarted`, starts no process and is recorded cancelled before start. One
ending each.

## `mod scaffold_runner` › `fn a_late_call_waits_at_the_door_until_admitted_and_a_barred_one_until_cancelled() {`

Under late entry a call starts only once it is admitted, and until then is not inside the runner;
a barred call is inside once it waits at its door, is never started by an admission, and ends
cancelled before start when its call is cancelled. What the coordinator's scheduler and
`a_shutdown_releases_each_invocation_once_whether_pending_unstarted_running_or_finished` lean on.

## `mod scaffold_runner` › `fn the_scaffold_runner_records_the_policy_and_image_each_invocation_ran_under() {`

By default an invocation records the host policy and no image; under a declared container policy it
records that policy and its immutable image id, beside the request it carried.

## `mod scaffold_runner` › `fn the_scaffold_runner_fails_a_shell_or_agent_probe_on_demand() {`

Through a real consumer: the production shell probe (`host::run_shell_probe`) refuses when the
double was told to fail the next shell probe with an exit code, and passes on the one after, since a
failure is used once; an agent probe told to never start is a `NeverStarted` error that is not a
cancellation. Failed probes are recorded like any request.

## `fn the_closure_notes_say_what_closure_does_under_concurrency_and_what_it_still_refuses() {`

Review round 2's `R2-REG-1` (the PR11 record, §13, round R2). The `Self::Closure` section of
`docs/internals/engine/topology/run.md` still said the concurrent half of closure — in-flight
cancellation, the budget drain, promotion and publication inside closure — was refused by
`closure::refuse_unclosable` naming PR11, after phase 4 had built all of it into
[`TopologyRun::close_run`]. The pin holds what the section says now — the concurrent half done in
`close_run`, a halt's vouched identities settled, a budget stop drained, only unvouched in-flight work
refused — and that the retired sentence does not come back. It matches on the prose with its whitespace
collapsed, as the pins above do.

## `fn a_rate_limited_worker_naming_a_reset_writes_one_pool_exhausted_before_its_attempt_finished() {`

The schema-4 run writes the capacity signal the sequential engine writes, and writes it first.
The scaffold's `Wide` run settles one attempt through `TopologyRun::step`. Its Claude Code worker
is rate limited on the pool `claude-max`, and its stderr carries Claude Code's five-hour limit
message, which names a 05:30 UTC reset. The run's clock is fixed at 03:00 UTC on 11 November
2026, so the reset read against it is `2026-11-11T05:30:00Z`. The log then holds exactly one
`pool_exhausted`, for that pool and agent, with that reset. It comes before the attempt's
`attempt_finished`, so a log cut between the two still records which subscription refused the
work and when it comes back.

The clock is the run's own seam, not the host's. The reset is read against `RunSeams::clock`,
which also stamps every event, so the test can fix it and the expected reset is a constant.

## `fn on_claude_max(`

A `Wide` run of `tasks` on the pool `claude-max` at width 1, answered by `limited_runner`.
Whether an attempt is rate limited is the adapters' call, not the runner's, so
`step_answered_by` sets the adapters for each step. It reads every reset against `RunClock`,
03:00 UTC on 11 November 2026.

## `fn limited_runner(`

A runner that answers the way `wide_responder` does, except for the worker of each
`(task, generation)` named in `limited`. That worker exits 1 with the given limit message on its
stderr. `on_claude_max` starts a run with it, and the resume tests hand a fresh one to the next
process.

## `fn rate_limited_step_past_any_wait(`

`step_answered_by` with the rate-limiting adapters, repeated while the step answers
`Progress::Waiting`. A five-hour limit read at 03:00 names a reset the back-off waits out, in
sixteen slices on this fixed clock, and a test about what the limit writes is about the step
after them.

## `fn a_second_rate_limit_naming_the_same_reset_writes_no_second_pool_exhausted() {`

`TopologyRun` writes `pool_exhausted` once for each pool and reset, as the sequential engine
does. A rate limit is an outage, so the ladder defers the task: its generation closes, the
backoff wakes it, and it is dispatched again under the next generation. The backoff waits out
the 05:30 reset the first limit recorded, in the slices `rate_limited_step_past_any_wait` steps
through, before it wakes the task. The second attempt hits the same five-hour limit, and the log
keeps its single `pool_exhausted`.

## `fn a_second_rate_limit_naming_a_different_reset_writes_one_more_pool_exhausted() {`

The same deferral, waited out to 05:32 the same way, with a second limit that names a different
reset: the weekly one, at 17:00 UTC on 14 November. A pool still marked exhausted gets one more
`pool_exhausted` when the reset moves, so the log holds both, in order.

## `fn a_rate_limit_after_an_attempt_the_pool_served_writes_pool_exhausted_again() {`

`alpha` is rate limited on `claude-max`. `beta` is then served on the same pool, accepted and
merged, and the backoff wakes `alpha`, which hits the same limit again. That accepted attempt
proves the pool was serving, so the run forgets the signal (`capacity::retire_served_pools`, on
the promotion path). The second limit is therefore recorded again, even though it names the
same reset.

## `fn a_failure_that_is_not_rate_limited_writes_no_pool_exhausted() {`

Neither failure is a rate limit, so neither writes `pool_exhausted`. One is an agent error whose
stderr carries the five-hour limit message anyway. The other is a gate failure after the worker
ran on the pool.

## `fn durable_on_claude_max(`

`on_claude_max`, on the scaffold's durable run, which has the owner and commit records a resume
needs.

## `fn resumed_limiting(`

The next process. It drops the run, recovers it from its log under the incarnation `inc-2`,
and hands it a fresh `limited_runner` answering with `limited`.

## `fn a_resumed_run_writes_pool_exhausted_again_only_when_the_limit_names_another_reset() {`

The two dedup tests above, across a resume. The first process is limited once, writes
`pool_exhausted` for the 05:30 reset, and ends. The next process starts from what its log
recorded (`capacity::exhausted_pools_of`). Its resume wakes the deferred task, and the task's
next generation is limited again at once. The same five-hour limit writes nothing more. The weekly limit names another
reset, so it writes one more `pool_exhausted`. Before the resume read the log, the next process
started with no pools recorded and wrote the same signal a second time.

## `struct CutAfterPoolExhausted {`

Hooks that leave the log the way this stage's ordering can stop it. Once a `pool_exhausted` has
been folded, they arm an error return at `Event.append`'s `Written` point. The next append, the
attempt's `attempt_finished`, then fails before its line is whole. Its torn tail is no committed
line, so the log ends with the signal and no settlement after it.

## `fn a_run_whose_log_ends_with_pool_exhausted_resumes_with_that_pool_exhausted_at_that_reset() {`

The settlement's append fails after the signal's, so the first process ends with `alpha`'s
attempt in flight and its `pool_exhausted` durable. The resume settles that attempt as
interrupted, which carries no record and retires nothing. When `alpha` runs again into the same
five-hour limit, the log still holds a single `pool_exhausted`. So the resumed run held the pool
exhausted at 05:30, the reset the cut log recorded. Had it held the pool with no reset, the 05:30
limit would have counted as a new reset and been written again.

## `struct RecordingSleeper(std::cell::RefCell<Vec<std::time::Duration>>);`

A sleeper that sleeps nothing and keeps every duration it was asked for, in order, so a test reads
the whole of a wait as the slices it was slept in.

## `fn defer_waits(wide: &crate::engine::topology::scaffold::Wide) -> Vec<(u64, u32)> {`

Every `defer_wait_elapsed` the log holds, as its `waited_ms` and `round`.

## `struct Stepped {`

One step of `stepped_until_waited`: what it answered, every `defer_wait_elapsed` durable after
it, and how many slices the sleeper had slept by then.

## `fn stepped_until_waited(`

Step the run on `clock` and `sleeper` until a step answers `Progress::Waited`, keeping each step.
It reads the log after every step, so a test can say that no `defer_wait_elapsed` was appended
before the last slice and not only that one was appended in the end. `answers` replaces the
scaffold's unattended answer source when a test needs an answer to arrive. The bound of 64 steps
is four times the sixteen slices the wait from 03:00 to 05:32 takes.

## `fn deferred_on_claude_max(`

`on_claude_max` with `alpha`'s first generation limited by `limit`, stepped once at `RunClock`:
the attempt settles rate limited and the task is `Deferred`. Every reset-wait test starts here,
at 03:00 UTC on 11 November 2026, with one deferred task.

## `fn a_deferred_rate_limit_naming_a_reset_is_waited_out_to_two_minutes_past_it() {`

The witness. Claude Code's five-hour limit, read at 03:00, names a 05:30 reset, and the run waits
until two minutes past it, at 05:32. The sleeper records slices of at most ten minutes that come
to 9,120 seconds, which is 05:32:00 less 03:00:00. The log holds exactly one `defer_wait_elapsed`,
with `waited_ms` 9,120,000 and `round` 1, and no step before the last appended one.

Before `back_off` waited toward a reset, the first back-off slept the default sixty seconds and
appended a `defer_wait_elapsed` of 60,000. The task woke after a minute, met the same limit, and
spent one of its `max_defers` on each wake, so the fixture's allowance of two was gone within
minutes, hours before the window cleared.

## `fn assert_backs_off_as_before(tag: &str, steps: &[Stepped], sleeper: &RecordingSleeper) {`

The back-off when no known reset is waited for: one step, which sleeps the doubling backoff's
first minute and appends a `defer_wait_elapsed` of 60,000 in round 1.

## `fn a_limit_naming_a_reset_past_the_bound_backs_off_as_before() {`

Claude Code's weekly limit, read at 03:00 on 11 November, names 17:00 on 14 November. That is
more than `interaction::RESET_WAIT_BOUND` (five hours and a quarter) ahead, so
`reset_wait_until` waits for nothing and the run backs off as it did before.

## `const UNREAD_LIMIT: &str = "429 Too Many Requests";`

A rate-limit message that names no reset.

## `fn a_rate_limit_whose_reset_is_not_read_backs_off_as_before() {`

The pool is recorded exhausted with no reset, so there is nothing to wait for and the run backs
off as before.

The task names an agent whose reset is not read, which is any agent but Claude Code
(`capacity::pool_exhausted_to_record`). This scaffold's ladder runs Claude Code on every rung,
so the test gets the same record another way: a Claude Code limit whose message names no reset.
`back_off` reads only the `exhausted_pools` entry, and either way that entry is
`("claude-max", None)`, which the test asserts before it steps.

## `struct AfterTheReset;`

The run's clock at 06:00 UTC on 11 November 2026, half an hour after the reset `RunClock` reads
from the five-hour limit.

## `fn a_recorded_reset_already_passed_backs_off_as_before() {`

The five-hour limit is read at 03:00, so the pool is recorded exhausted until 05:30. The run then
backs off at 06:00. A reset already behind the clock is not waited for (`reset_wait_until`), so
the back-off is the plain one.

## `fn a_reset_on_a_day_its_month_lacks_is_not_read() {`

`PR390-INVALID-RESET-DRIVES-WAIT` (P2): the witness. `unix_seconds_of` checked a day only against
1 to 31, so 31 February 2026 read as 3 March, 29 February 2026 (not a leap year) as 1 March, and
31 April as 1 May. Each of the three now reads as `None`. 29 February 2028 is a leap day and still
reads, at one hour past `util::days_from_civil`'s count for that date. The five-hour limit's
`2026-11-11T05:30:00Z` reads as it always did. On the code before the repair the test fails on its
first reading: `Some(1772499600)`, 01:00 on 3 March 2026, where it expects `None`.

## `struct OnMarchThird;`

The run's clock at midnight UTC on 3 March 2026. The old parser read `2026-02-31T01:00:00Z` as an
hour after this, well inside `interaction::RESET_WAIT_BOUND`.

## `fn a_recorded_reset_on_a_day_its_month_lacks_backs_off_as_before() {`

The same defect at the level of the run. The pool's recorded reset is replaced with
`2026-02-31T01:00:00Z`, as a corrupted or hand-edited log would leave it, and the run backs off on
`OnMarchThird`. A reset that does not read is no known reset, so the back-off is the plain one.
On the code before the repair the run instead waited toward 01:02 on 3 March, in slices of
600 seconds six times and then 120, 3,720 seconds where the plain back-off sleeps 60.

The test sets `exhausted_pools` directly. It does not resume from a log that holds the reset,
because a resume wakes the deferred task and runs it again before any back-off. A resumed run
reaches `back_off` only after a second limit, and that limit records its own reset over the one
under test. Either way, `exhausted_pools` is what `back_off` reads.

## `fn a_reset_reading_outside_the_writers_shape_is_not_read() {`

`PR390-RESET-YEAR-OVERFLOW-PANIC` and `PR390-MALFORMED-RESET-SUFFIX-WAITS`: the witness. Each
reading is the five-hour limit's `2026-11-11T05:30:00Z` with one thing changed, and each is `None`:
a year of `9223372036854775807` or of `10000`, a month of `1`, seconds of `+0`, `.not-a-fraction`
or a bare `.` before the `Z`, a lowercase `t` or `z`, `+00:00` in place of the `Z`, a 60th second,
and the year in full-width digits. The empty string is `None` too. A fraction of digits still
reads, as the whole second before it, and so do the epoch and the last second of 9999, the two ends
of what four digits of year can write.

On the code before the repair the test panics on its first reading, `attempt to multiply with
overflow` in `util::days_from_civil` (`era * 146_097`). With it set aside, six of the other eleven
read:
`10000` as 253,429,536,600, the one-digit month as 11 January 2026, `+0`, `.not-a-fraction` and
the bare `.` as 05:30, and the 60th second as 05:31.

## `fn whatever_reads_is_what_the_writer_writes() {`

The parser's grammar is the writer's, from both sides. Every instant of a spread from the epoch to
the last second of 9999, and of a few edges among them (29 February 2000, the five-hour limit's
reset), reads back from what `util::rfc3339_utc` writes for it. Then every one-character
insertion, deletion and substitution in `2026-11-11T05:30:00Z` and in `2028-02-29T23:59:59.5Z`,
drawn from `0 9 + - . : T Z é ２` and a space, either does not read or reads as an instant whose
`rfc3339_utc` is the edited text less its fraction. A fraction here is `.` and one or more digits
just before the final `Z`, and anything else after a `.` stays in the text, so a reading the
parser takes too loosely cannot match. The leap day puts the calendar under the edits too: one that
moves it into `2029` or `2098` names a day those years lack, and does not read.

On the code before the repair the test fails at `02026-11-11T05:30:00Z`, a `0` inserted before the
year, which read as 1,794,375,000, written `2026-11-11T05:30:00Z`.

## `fn a_recorded_reset_outside_the_writers_shape_backs_off_as_before() {`

The two defects at the level of the run, as in
`a_recorded_reset_on_a_day_its_month_lacks_backs_off_as_before` but on `RunClock`. The pool's
recorded reset is replaced once with the five-hour limit's reset in the year
`9223372036854775807`, and once with `2026-11-11T05:30:00.not-a-fractionZ`. Neither reads, so each
is no known reset and the run backs off the plain minute. On the code before the repair the first
case panicked in `util::days_from_civil` inside `TopologyRun::awaited_reset`. The second read as
05:30 and waited toward 05:32, in fifteen slices of 600 seconds and one of 120, 9,120 seconds where
the plain back-off sleeps 60.

## `struct AnsweredAfterSlices<'a> {`

An answer source that has nothing to say until the sleeper has slept `slices` slices, and then
answers `answer`: an answer published between two slices of the wait.

## `struct WaitingLines(std::sync::Mutex<Vec<String>>);`

A notifier that keeps every waiting line it is given. It is a `static` because a run holds its
notifiers as `&'static dyn Notifier`. Only the one test below hands it to a run, so no other test
writes to it.

## `fn an_answer_published_between_two_slices_is_ingested_before_the_wait_continues() {`

`alpha` is rate limited and deferred until 05:32, and `beta` waits for an answer to a spend
approval, raised through `ask_to_approve_spend`. The answer source declines it once two slices
have been slept. The step after the second slice ingests the decline, and sleeps nothing. Every
other step sleeps one slice toward 05:32. The run selects before it sleeps, so an answer that
arrives during the wait is taken in at most one slice late.

The decline fails `beta` and leaves `alpha` deferred, so the next step backs off again. It
continues the wait it held, which ingesting an answer does not drop: the slices still come to
9,120 seconds in all, and the log holds one `defer_wait_elapsed`, at the end. The notifier is
told about the wait once, when it starts, and not again when it continues after the answer.

## `struct MovingClock<'a>(&'a RecordingSleeper);`

`RunClock`'s 03:00, moved on by every slice the sleeper has slept, read back as RFC 3339 UTC
(`util::rfc3339_utc`). The clock and the sleeper share one count, the sleeper's record, so the
clock reads 05:30 once the slices come to two and a half hours, as a real run's clock would, and
the two cannot drift apart. `RunClock` stays at 03:00 however long the run sleeps, which is why
the tests above never reached the reset and never saw a held wait lose it.

## `fn a_held_reset_wait_runs_through_its_margin_on_a_moving_clock() {`

`PR390-HELD-RESET-LOSES-MARGIN` (P1): the witness's run, on a clock that moves. Fifteen
ten-minute slices bring the clock from 03:00 to 05:30, the reset itself. `back_off` used to
choose its reset again before every slice, and at 05:30 `reset_wait_until` chose none, because
the reset was no longer ahead. The run then slept the plain backoff's minute, appended a
`defer_wait_elapsed` of 60,000 in round 1, and woke the task at 05:31, before the 05:32 it had
announced, with 9,000 seconds of completed sleep missing from the log.

A held wait is now continued before any reset is chosen, so it runs on through its two-minute
margin. The slices come to 9,120 seconds, none longer than ten minutes, the clock ends at 05:32,
and the log holds exactly one `defer_wait_elapsed`, of 9,120,000 in round 1. On the code before
the repair the test fails on its first assertion, 9,060 seconds slept against 9,120.

## `fn every_elapsed_wait_records_all_of_its_sleeps() {`

The same run on the same moving clock, stepped until `Progress::Waited`. Each
`defer_wait_elapsed` appended must record, in `waited_ms`, the sum of the slices slept since the
one before it, or since stepping began. A back-off that drops a held wait for the plain one
records only its own minute: on the code before the repair, the one `defer_wait_elapsed` said
60 seconds where 9,060 had been slept.

## `fn the_serving_pool_is_the_first_in_file_order_no_signal_holds_exhausted() {`

The rule `serving_pool` applies, case by case, over the candidates `a`, `b` and `c` in that
order. The clock is `RunClock`'s 03:00 UTC on 11 November 2026.

| Held | Answer |
|---|---|
| nothing | `a` |
| `a` | `b` |
| `a` and `b` | `c` |
| `claude-max`, which is not a candidate | `a` |
| all three, `b`'s reset at 02:58:00 | `b` |
| all three, `b`'s reset at 02:58:01 | `a` |
| all three, every reset at 05:30 | `a` |
| all three, no reset recorded | `a` |
| all three, `b`'s reset `2026-02-31T01:00:00Z` | `a` |

With no candidates the answer is `None`.

02:58:00 is `interaction::RESET_WAIT_MARGIN`, two minutes, behind the clock to the second, and
the test asserts that before it uses it. So the fifth row is the boundary of "at or before
now", and the sixth, one second later, is the first reset that has not passed. The last row is
a reset on a day its month lacks. `unix_seconds_of` reads it as no date, so it has not passed. A
parser that counted the day on into March would have put it eight months behind the clock and
answered `b`.

On the code before `serving_pool` existed the test does not compile: the function it calls is
not there.

## `fn on_two_claude_max_pools(`

`on_claude_max` with two pools for the scaffold's implementing agent, `claude-max` and then
`claude-max-2`, as `WidePlans::pools` entries naming `AGENT` twice. The primary reviewer runs on
the same agent, and a reviewer's pool is its agent's first, so reviews stay on `claude-max`.

## `fn served_steps_until_settled(`

`step_answered_by` with the serving adapters, repeated until a step settles an attempt of
`key`, keeping each step. It panics after 64 steps. The steps before the settlement are whatever
the run does first. Before a worker's limit failed over that was the sixteen slices of a wait
toward the 05:30 reset, and since then it is nothing. A test that uses this says nothing about
whether that wait happens, so it holds on a run that waits and on one that does not.

## `fn attempts_started(`

Every `attempt_started` the log holds for `key`, in order.

## `fn limited_once_on_two_pools(`

`on_two_claude_max_pools` with one task, `alpha`, whose first generation is limited by
`FIVE_HOUR_LIMIT`, stepped once with the rate-limiting adapters. The attempt settles rate
limited, and the log holds one `pool_exhausted`, for `claude-max` with the 05:30 reset. Both
witnesses below start here, with `claude-max` held exhausted and `claude-max-2` not.

## `fn a_task_dispatched_again_after_a_rate_limit_binds_the_next_pool_of_its_agent_in_a_fresh_session()`

The witness for the pool a new attempt binds. `alpha`'s first generation is rate limited on
`claude-max`. Its next generation is served, and accepted. The log holds exactly two
`attempt_started` for the task: generation 0 on `claude-max`, then generation 1 on
`claude-max-2`, each attempt 1 with no session to resume, and both under the same binding. The
agent, model and effort did not change. Only the pool did.

The steps between the two attempts are not asserted (`served_steps_until_settled`). When this
test was written the run waited out the 05:30 reset before it woke the task, though
`claude-max-2` could have served it at once. The limit now fails over, and
`a_rate_limit_fails_over_to_the_next_pool_of_the_same_agent_in_a_fresh_session` asserts that
nothing comes between the two attempts.

On the code before `prepare_attempt` chose a pool the test fails on the pools: generation 1
starts on `claude-max` again, the pool that had just refused generation 0, because the plan
builder resolved the agent's first pool for every attempt.

## `fn a_retained_retry_resumes_on_the_pool_its_session_ran_on() {`

The witness for a retry's pool. The same start, with the gate failing `alpha`'s attempt 1.
Generation 0 never reaches its gate, because its worker is rate limited. Generation 1's first
attempt runs on `claude-max-2` and fails the gate. That is an ordinary failure with a session, so
the generation is retained and its second attempt resumes the session.

Before the retry the test takes `claude-max` out of `exhausted_pools`, as a review that pool
served for another task would. `claude-max` is then first in file order and held by nothing, so
`serving_pool` would choose it. The retry still records `claude-max-2`, with a `resume_session`,
because `retry_request` takes the retained attempt's own pool. A session lives under its pool's
credential directory, and the first pool's credentials never held this one.

The scaffold reaches the retained retry through `TopologyRun::step`, so `retry_request` is not
called directly. The retry is the step after the retained settlement, and the test asserts it
is one step.

On the code before the change the test fails on the retry's pool, `claude-max` where it expects
`claude-max-2`: every attempt there, the retry included, bound the agent's first pool.

## `fn last_settlement(`

The settlement of `key`'s last `attempt_finished` in the log: `Retained`, or `Closed` with its
transition.

## `fn deferral_of(wide: &crate::engine::topology::scaffold::Wide, key: TaskKey) -> (TaskState, u32) {`

The fold's state for `key` and the deferrals it has counted for it, the two things a failover
must leave alone and a deferral must move.

## `fn a_rate_limit_fails_over_to_the_next_pool_of_the_same_agent_in_a_fresh_session() {`

The witness for the failover. `alpha`'s first generation is rate limited on `claude-max`, with
`claude-max-2` held by nothing. The step answers `Settled` with nothing accepted and nothing
spent. The task is not `Deferred` and its `defers` is still 0. Its `attempt_finished` closes
the generation with the `Retry` transition. The log still holds the `pool_exhausted` for
`claude-max`, with the 05:30 reset.

The second step runs on a `RecordingSleeper` with the serving adapters. It settles generation 1
accepted. The sleeper recorded no sleep and the log holds no `defer_wait_elapsed`, so nothing
came between the limit and the attempt that replaced it. Generation 1's `attempt_started`
names `claude-max-2`, resumes no session, and carries generation 0's binding.

On the code before `settle_failure` failed over, the test fails where it reads the task's
state: the limit deferred the task (`Deferred` where it expects any other state), and the run
went on to wait until 05:32 with `claude-max-2` idle.

## `const LIMIT_RESET_A_MINUTE_AGO: &str = "You've hit your 5-hour limit · resets 2:59am (UTC)";`

Claude Code's five-hour limit naming a reset at 02:59 UTC. Read at `RunClock`'s 03:00, the
nearest 02:59 is the one a minute earlier, so the pool is recorded exhausted with a reset
already behind the clock.

## `fn a_failover_never_returns_to_the_pool_that_refused_even_when_its_reset_reads_as_past() {`

`claude-max` refuses generation 0 with `LIMIT_RESET_A_MINUTE_AGO`. The test first asserts that
the recorded reset is `2026-11-11T02:59:00Z`. If the limit were read as any other instant, such
as 02:59 the next morning, the test would not be about a past reset, and it stops there saying
so.

A reset behind the clock could look like a pool that is free again. It is not: the pool refused
this attempt a moment ago. The limit fails over, and generation 1 binds `claude-max-2`. The
failover asks only whether a signal holds a pool (`TopologyRun::another_pool_can_serve`), and
`serving_pool` takes a pool no signal holds before any held pool whose reset has passed.

Had generation 1 gone back to `claude-max`, it would have met the same limit. With
`claude-max-2` still free, that limit would have failed over again, round after round, with no
deferral to count them.

On the code before the failover the test fails where it reads the task's state, `Deferred`.
The pools it asserts last would have passed there: the task was deferred, backed off for the
plain minute because the reset had passed, and woke onto `claude-max-2`.

## `const SEVEN_AM_LIMIT: &str = "You've hit your 5-hour limit · resets 7am (UTC)";`

The five-hour limit naming a 07:00 UTC reset, an hour and a half after `FIVE_HOUR_LIMIT`'s and
still inside `interaction::RESET_WAIT_BOUND` of 03:00.

## `fn with_every_pool_of_the_agent_exhausted_the_run_waits_for_the_earliest_reset_among_them() {`

The witness for the reset a deferral waits on. Generation 0 is limited on `claude-max` with
the 05:30 reset and fails over. Generation 1 is limited on `claude-max-2` with
`SEVEN_AM_LIMIT`. Both of the agent's pools are then held, so the second limit defers the
task: `Deferred`, with `defers` 1, and one `pool_exhausted` for each pool with its own reset.

The run then backs off on a `RecordingSleeper` until a step answers `Waited`. The slices are at
most ten minutes each and come to 9,120 seconds, from 03:00 to 05:32. That is two minutes past
the reset of `claude-max`, the pool that comes back first, and not 07:02, two minutes past the
reset of the pool that refused last. The log holds exactly one `defer_wait_elapsed`, of
9,120,000 in round 1, and the one line the notifier was given names `claude-max` and 05:30.

The notifier is a `WaitingLines` leaked for this test alone, since a run holds its notifiers as
`&'static dyn Notifier` and `WAITING_LINES` belongs to the test above that fills it.

Two earlier states of the code fail it differently. Before the failover, the second step is not
a settlement: the first limit deferred the task, so that step slept the first slice of a wait
toward 05:32 and answered `Waiting`. With the failover and `awaited_reset` as it was, reading
only the pool the last attempt ran on, the task is deferred as asserted and the wait is 14,520
seconds, to 07:02.

This is also the witness for the third case of `awaited_reset`: with every recorded reset still
ahead, the wait is the one `interaction::reset_wait_until` chooses. It passes unchanged with
the two cases for a reset already passed decided before that rule is asked.

## `fn a_held_pool_whose_reset_and_margin_have_passed_takes_the_ordinary_back_off() {`

The regression reviewer's witness for #395 (`regression-witness-b657ba7.rs`), renamed, with its
comments moved here. Generation 0 is limited on `claude-max` with a reset at 03:01, a minute
ahead of `RunClock`'s 03:00, and fails over. The run is then stepped at 03:05
(`AtThreeOhFive`), when `claude-max`'s reset and its two-minute margin have both passed. That
step tries the spare, `claude-max-2`, which refuses until 07:00, so both pools are held and the
task is deferred. The back-off that follows is the ordinary one, and the sleeper records exactly
60 seconds: `claude-max` serves again from 03:03 (`serving_pool`), so there is no reset to wait
for.

Before the failover (`b73d347a`), the first limit deferred the task, so the back-off ran at
once, found the 03:01 reset behind the clock and slept the same minute. With the failover and
`awaited_reset` asking `interaction::reset_wait_until` alone, that rule dropped the passed 03:01
reset and the run slept 14,220 seconds, to 07:02, two minutes past the spare's reset.

## `fn a_held_pool_whose_reset_passed_inside_the_margin_is_waited_for_only_to_two_minutes_past_it() {`

The frontier pass's mixed case for #395, with its pool names. `primary` refuses generation 0
with `LIMIT_RESET_A_MINUTE_AGO`, a reset at 02:59, and the task fails over to `spare`, which
refuses generation 1 with `FIVE_HOUR_LIMIT`'s 05:30. Both pools are then held, each with its
own reset, and the task is `Deferred` with `defers` 1.

At 03:00, `primary`'s reset has passed but its two-minute margin has not, so `primary` serves
again from 03:01 (`serving_pool`). The test asks `awaited_reset` for the target directly,
because a wait of a single slice answers `Waited` and never the `Progress::Waiting` that
reports it. The target is 03:01, for `primary` and its 02:59 reset. The run sleeps 60
seconds and records one `defer_wait_elapsed`, of 60,000 in round 1. The one line the notifier
gets names `primary`, 02:59 and a one-minute wait. As in the earliest-reset test, the notifier
is a leaked `WaitingLines`.

With `awaited_reset` asking `interaction::reset_wait_until` alone, that rule dropped the 02:59
reset as behind the clock and chose `spare`'s, to 05:32. The test fails at the target. The run
would have slept 9,120 seconds and left `primary` idle from 03:01.

## `fn a_rate_limit_with_no_other_pool_for_the_agent_still_defers() {`

The pools are `claude-max` for the implementing agent and `copilot-pool` for `REVIEW_AGENT`,
which no reviewer of this run uses. `copilot-pool` is held by nothing, and it is not a pool of
the worker's agent, so it is no target for a failover. The limit on `claude-max` defers the
task as it did before the failover existed: nothing spent, `Deferred` with `defers` 1, a
`Deferred` transition, one `pool_exhausted` with the 05:30 reset, and a wait of 9,120 seconds
recorded as one `defer_wait_elapsed`. Only one attempt started, on `claude-max`.

This is also the test that the pool which refused is not counted among the pools that could
take over. `exhausted_pools` does not hold `claude-max` when `settle_failure` decides, and a
failover that counted it would have retried on it for ever.

It passes on the code before the failover and after it.

## `fn a_reviewers_rate_limit_still_defers() {`

Only a worker's limit fails over. The run has two reviewers, the second on `REVIEW_AGENT`, and
two pools for each agent. The adapters are
`ScaffoldAdapters::rate_limiting_the_second_reviewer`, so the worker is served on `claude-max`
and passes its gate and its first review, and the second reviewer is rate limited on
`copilot-pool`. The settled failure is `RateLimited` with the origin `Reviewer`, and the one
`pool_exhausted` is for `copilot-pool` with no reset, since only Claude Code's limit message is
read for one.

Both `claude-max-2` and `copilot-pool-2` are held by nothing, and the task is still `Deferred`
with `defers` 1. The record's own pool is the worker's `claude-max`, which served. A failover
that did not ask whose limit it was would have found `claude-max-2` free and retried, and the
reviewer, which runs on its agent's first pool, would have been limited again on every round.

The back-off that follows is the plain one (`assert_backs_off_as_before`): a reviewer's limit
waits on the reviewer's pool alone, and `copilot-pool` has no reset recorded.

It passes on the code before the failover and after it.

## `fn escalated_log(interrupted_first: bool) -> (TopologyFold, Vec<TopologyEvent>) {`

A schema-4 log replayed into the settle fixture's fold, every event applied, so
the fold admits each one: aleph is dispatched and its attempt started, and, when
`interrupted_first`, that attempt is settled `attempt_interrupted` (with the
disposition its predicted lease requires) and aleph is dispatched again under the
next generation; the attempt then fails at $0.50 and escalates (`Next::Escalate`).

## `fn a_spend_approval_after_an_interrupted_attempt_calls_its_reported_spend_a_floor_as_the_report_does()`

K5, regular review 4, finding 5 (P2, correctness; first bad `e8ab8838`, J1's
repair). The reviewer's sequence: a worker runs and its coordinator dies, resume
records `attempt_interrupted`, and a later priced failure escalates onto the
frontier and asks for approval. The approval's context, worded from that log by
`spend_approval_context`, reports $0.5000 as "a floor, not a total" and "at least
what the run has already cost"; the run's report of the same log is a floor too.
Without the interruption it is neither, and the context says "This is what the
run has already cost". With the approval's floor read by the event scan it had
before K5, the interrupted case fails with the reviewer's own text: "Reported
spend so far: $0.5000. This is what the run has already cost, …".
