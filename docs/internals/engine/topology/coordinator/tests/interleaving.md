# `src/engine/topology/coordinator/tests/interleaving.rs`

Extended notes for [`src/engine/topology/coordinator/tests/interleaving.rs`](../../../../../../src/engine/topology/coordinator/tests/interleaving.rs).

The code is the authority for what it does; this file is the whole of its prose but for the
concurrency protocol standards §10 places at its site. Each section
here is headed by the line of code it describes, spelled as it is in the source, so the heading is
the grep string that finds the code.

## Module

Phase 6's interleaving suite: the packet's seam rows under seeded permutations through this
coordinator at width 3, deadlock-freedom under reduced limits and adverse completion orders, the
runtime pool tests (acceptance item 3 through the coordinator on its Tokio blocking pool), and the raw
outputs G6 asks for (`required_artifacts`). A nested module of the coordinator's inline tests, so it
uses the scheduler's fixtures and the coordinator's private types without a new test file; its tests
are `engine::topology::coordinator::tests::interleaving::*`, which is also the filter the G6 gate runs.

Every run is driven by an [`Interleaver`] (below): no test sleeps, every wait is a handshake with the
double, and a seed reproduces a run (R-AD), so each assertion made for a seed is made for that exact
schedule on every machine. Seed counts are bounded by cost, not by coverage claims: eight seeds for the
per-run rows (ST-04, ST-05, ST-13, the chain), sixteen for the independent projection, three to four
per configuration for the pool tests, and five orders per limit configuration for deadlock-freedom
(the record's R-AS says why) — on Linux and macOS. On Windows each runs the fewest its assertions
compare (`scheduled_here`, the record's R-AY): one seed per per-run row and per pool configuration, two
for the independent projection, one order per limit configuration.

## `fn export(kind: &str, value: &serde_json::Value) {`

The G6 exporter. When `UPSTROKE_G6_EXPORT` names a directory, a test writes what it measured as
pretty JSON to `<dir>/<kind>/<test>.json`, `<test>` being the last segment of the calling thread's
name, which libtest sets to the test's path (the observation export keys on the same name,
`observations.md`). Kinds: `seam/<row>` (the seam-test
outputs, one file per test that claims the row), `projection/<plan>` (the permutation projection
equivalence report: per seed, each key's projection digest and the final tree), and
`ledgers/<ledger>` (`slot`, `invocation`, `provisional`, `entitlement`, `container`). Unset, nothing
is written. The record's §3 R-AU gives the gate's command.

## `struct Interleaver<'r> {`

The suite's [`Quiescence`] observer. Like the parent module's `Scheduler` it owns the double's door
(`enter_late`, `admit` in `granted`, `inside` checked at every quiescent point) and releases exactly
one held invocation per point, so the number of points is the number of processes released. It adds:
a record of every point (the invocations held, the provisional entitlements outstanding, the
reservation peak, the discard and duplicate counters, the injections made so far); four release
orders (`Adverse`: seeded, newest-granted-first, oldest-granted-first, gates first and agents last);
an optional injector, consulted on a second seeded stream at about one point in four, whose message
goes through the coordinator's injector and costs the point instead of a release — so the release
sequence of an injected run is the uninjected run's for the same seed; an optional watch run at every
point; and `STEPS`, beyond which it releases nothing, which the coordinator turns into its stuck
error: a run that did not finish within the bound fails, and never hangs.

## `fn aliases(`

ST-04's oracle over one whole run, from the durable log and the double's record of every process:
every `InvocationId` started once; every task worktree and snapshot path used by exactly one owner
(a task's generation for its worktree; an attempt's gates or a reviewer's pass and re-ask, or a
sequence's integration gate or reviewer, for a snapshot) and every owner in exactly one path; each
generation dispatched once; each attempt started once and settled once (`candidate_prepared`,
`attempt_finished` or `attempt_interrupted`); each candidate ref created once and each prepared pin
taken once; each sequence claimed by one candidate, in log order; integration in
`task_candidate_created` order and each sequence merged once.

## `fn duplicating(crossed: std::rc::Rc<std::cell::Cell<usize>>) -> Inject<'static> {`

ST-05's injector, which also carries ST-01, ST-02 and ST-06 under interleaving: at a seeded point it
offers one of a duplicate end of an invocation already settled (the ledger counts it as a duplicate
when its pipeline is live, the coordinator discards it otherwise), an end of a running invocation from
outside its pipeline, a snapshot end nobody holds, a stale completion (a retired pipeline's own
identity, remembered from earlier points, or a pipeline that never existed), a registration offered
through the injector, or a completion naming another live pipeline's identity (or a mismatched attempt
or sequence). Every one of them is refused or counted, once, and none reaches the log. `crossed`
counts the last kind, so the test can require each to be refused by the binding to its pipeline's
identity rather than by the injector's own refusal, which would refuse it anyway (round R5,
`FULL-SC-2`).

## `struct Tracing {`

The coordinator's hooks for ST-13, ST-10, ST-09's logical rows and ST-18: the harness adapters, with
the effect and run-directory families wrapped to record every consultation, in order, into one trace
beside every append (its kind, the fold-derived pipeline and merge holdings after it, which reservation
kind it converts, whether it is `merge_prepared(fast)`), every fold state for the replay check, and,
after every append, what `brokered` showed of the broker (a [`Brokered`] each). Only the
coordinator thread's effects are traced: each pipeline takes its own hooks from the factory.
`Killer` counts the consultations after `run_finished` is folded and answers `Kill` at the armed one,
through the observation export as the harness adapters do, having written the cell it fires at to a
report file first — which is how ST-18's parent learns where its child died.

## `fn provisional_problems(`

ST-13's oracle over the trace. At every append the derived holdings are within `max_parallel` and one
merge; each first append — `task_dispatched`, a retry's `attempt_started`,
`merge_verification_started`, `merge_prepared(fast)` — moves them by exactly its reservation's
entitlements in one step (the fast and stale integrations' pair from no merge to one); no staging
effect (the staging intent and worktree, the prepared pin, the proposal cherry-pick) falls between a
fast integration's selection and its `merge_prepared`, or before its `task_merged`; the fast CAS's two
hook phases see the merge held; and `task_merged` releases both holdings at once. The reservation
ledger itself is read at every quiescent point and at the end (R-AT), and after every append by
`ledger_problems`: `folded`, the append hook R-AT found carrying no ledger, still carries only the
fold, and the broker reaches the test through `brokered`.

## `struct Brokered {`

One append's observation of the broker, as `brokered` showed it on the coordinator's thread right
after the append: the append's prefix length and kind; the reservation its caller converts after it,
from its body (`converted_by`); every reservation the ledger holds, read by task key over the fold's
registry, with the outstanding count and the taken, converted and cancelled totals (R13); the slot
table's granted and released totals and the identities holding a pair and waiting for one (R3); and
the invocation ledger's registered count, its pending and running identities, and its completed and
cancelled counts (R4). Everything but `converts` is copied from the run's own ledgers; `converts` is
the expectation, derived from the event.

## `fn converted_by(body: &TopologyEventBody) -> Option<(TaskKey, ReservationKind)> {`

The appends after which their caller converts a reservation, with its task and kind: `task_dispatched`
(`dispatch_settled`), a retry's `attempt_started` (`retry_started`), and `merge_verification_started`,
`merge_prepared(fast)` and a conflict's `merge_rejected` (the frozen `integrate()`'s
`IntegrationJournal::converted`). `Tracing::folded` classifies the first four for
`provisional_problems`; a conflict's rejection converts its reservation but moves no holding, so it is
outside that oracle's one-step check.

## `fn ledger_problems(ledgers: &[Brokered]) -> Vec<String> {`

ST-09's logical rows at every append, from the process's ledgers rather than from the fold, as
in-flight conservation and identity association — never the endpoint `balances()` or an empty holder
set, which valid work in flight does not meet. At each append: R13, taken = converted + cancelled +
outstanding, and the reservations held are exactly the one this append's caller converts after it, or
none — the provisional phase, in which a first append's reservation is still held while the fold
already derives its entitlement; R3, granted = released + held, the pairs held by exactly the slotted
invocations R4 has running, and exactly R4's pending invocations waiting for one; R4, registrations
partitioned into pending, running, completed and cancelled, pending and running disjoint. Each problem
names the append, its kind and both values. The partition count is structural in `InvocationLedger`,
one state per entry; the two equations and the associations are what a ledger defect, or an
observation taken at the wrong moment, breaks.

## `fn seeded_runs_alias_no_task_generation_attempt_invocation_snapshot_ref_pin_sequence_or_queue_position()`

ST-04 over eight seeded runs of a four-task plan with a dependent, a same-generation retry and two
reviewers per attempt and per verification, with ST-15 (the retry re-gates on a fresh snapshot of its
own in every seed, and runs beside another task's process in some — in all eight, so seed 0 is the
Windows schedule).

## `fn injected_duplicates_at_seeded_points_release_nothing_twice_and_change_nothing_durable() {`

ST-05 (with ST-01, ST-02 and ST-06): each seed is run twice, without and with the injector; between
consecutive points the discard and duplicate counters move by exactly the injections made, and the
injected run releases, starts and logs byte for byte what the uninjected one did, with the same
registrations, settlements, slot grants and releases and reservation conversions. At least five
injections per seed over the run (forty over Linux's and macOS's eight; each seed makes five to twelve).
Since round R5 (`FULL-SC-2`), every completion injected for a live pipeline under another identity
must be refused by the binding to that pipeline's identity — counted by `duplicating`'s `crossed`
and matched against the binding's warnings — and at least one is injected over the seeds run; before
it, the injector's own refusal would have discarded them with the binding removed. That last is a claim
made of some seed, so on Windows (R-AY) the test runs seed 1, the lowest whose injections include one
(seed 0's nine include none).

## `fn every_provisional_reservation_converts_at_its_first_append_under_seeded_permutations() {`

ST-13 at every append of eight seeded runs (`provisional_problems`), no reservation outstanding at any
quiescent point, and at the end one conversion per first append with none cancelled or duplicated;
ST-10 on the same runs (the live fold equals a replay of its own log after every append); and ST-09's
logical rows at every append of the same runs (`ledger_problems`). The broker is observed after
exactly the appends the fold is, the appends that find a reservation held are as many as the first
appends, and over the seeds some append finds a pair held after another was released, so the slot
equation is checked with work in flight. Whether a request waits for a pair at these limits is not
asserted here; the reduced-limits test asserts that one does.

## `fn a_chain_projects_at_width_three_as_at_width_one_under_every_seed() {`

ST-08's chain half under eight seeds, each byte-identical to the width-1 loop's canonical projection.

## `fn independent_tasks_project_identically_per_key_under_every_seed_and_report_it() {`

ST-08's independent half (and ST-03) under sixteen seeds: every task dispatched before any completion
on one base, integration in `task_candidate_created` order, each key's attempt-level projection exactly
`ONE_SUCCEEDING_ATTEMPT` (`attempt_level_problems`), and each key's projection, its commit labels
numbered within its own trace, and the final tree equal to the first seed's.

## `fn reduced_limits_and_adverse_completion_orders_reach_run_finished_within_the_step_bound() {`

Deadlock-freedom: one slot per agent and per pool; two agents sharing one pool, each in its own, none
pooled, and one unpooled beside a pooled one; review re-asks taking pairs in two of the four; and each
under two seeds and three fixed adverse orders — on Windows each configuration under one of them, the
first seed, newest-granted-first, oldest-granted-first and gates-first in turn (`scheduled_here`), so
every configuration and every kind of order runs there once. Every run completes within `STEPS`, no point holds a
pair over a limit, every pair granted is released once, and a verification runs beside attempts. Every
run also carries ST-09's logical rows at every append (`ledger_problems`, its hooks a `Tracing` whose
kill is never armed), and over the runs some append finds a request waiting for its pair, so the
waiting association is checked where requests wait. Each
run executes under the parent module's `bounded` watchdog on its own (`reduced_limit_run`), so a
regression that hung instead of ending in the stuck error fails the test rather than the suite, and the
bound is per run, not per test — a test of twenty runs on a slow leg does not approach it.

## `fn a_scheduler_that_stops_releasing_ends_the_run_as_stuck_rather_than_hanging() {`

The bound's other half: a scheduler that stops releasing ends the run with the stuck error, every held
process cancelled and every registration settled, and nothing appended — under `bounded`, so a
coordinator that waited for ever instead fails the test within its bound.

## `fn runtime_pool_same_agent_and_pool_with_opposing_limits_serialize_on_the_binding_limit() {`

Acceptance item 3 at runtime, with the four `runtime_pool_*` tests after it: the processes held at
every quiescent point are what runs at once, so a limit that binds shows as a ceiling on them and one
that does not as two at once. On Windows one seed per configuration; the gate test's is seed 1, one of
the two whose schedule runs two gates at once beside a held slot, the claim it makes of some seed.

## `fn runtime_pool_agent_probes_take_and_release_their_pair_at_preflight_before_admission() {`

A width-3 run resumed through the frozen recovery order with a real `RunPreflight` over a double: the
shell probe, then one probe per recorded agent, each under its own probe-role `InvocationId`, each
settled once; the pre-flight's ledger balanced before admission; the coordinator's broker empty when it
starts, and the run completing on its pool.

## `fn seeded_contained_runs_never_reuse_a_container_name_intent_or_view() {`

ST-04's container rows (names and intent paths) with R19 and R26 under seeds: at every point the
containers running, the intents and the views are exactly the held invocations', and no name is
created twice.

## `fn a_halt_under_every_seed_interrupts_exactly_what_is_in_flight_and_ends_halted() {`

ST-17's halt under eight seeds: after the halting decline nothing is admitted, one interrupted terminal
is appended per attempt and verification in flight at it, and `run_finished(Halted)` ends the log.

## `fn a_budget_stop_under_every_seed_drains_without_cancelling_and_ends_budget_exceeded() {`

ST-17's budget stop under six seeds: after `budget_exceeded` nothing is admitted and nothing is
cancelled, what was in flight settles naturally, and `run_finished(BudgetExceeded)` ends the log.

## `enum Parity {`

Which part of a finalization kill matrix one test runs: the cells at the even indices of the list its
reference run makes (`after_the_end`), or the cells at the odd ones. `first` is the part's first index,
from which the matrix takes every second cell. `share_of` is how many cells of a list that long the
part holds — the even part holds the one more when the count is odd — so the two parts' shares add up
to the list. `name` is the part's word in the matrix's messages and in its export.

## `fn finalization_kill_matrix(halted: bool, parity: Parity) {`

ST-18 at width 3, one outcome and one parity of its cells per test, so libtest runs the four parts
side by side rather than one after another:

- `a_concurrent_complete_run_finalizes_through_the_frozen_finalization_and_converges_after_a_kill_at_every_cell_even_cells`
- `a_concurrent_complete_run_finalizes_through_the_frozen_finalization_and_converges_after_a_kill_at_every_cell_odd_cells`
- `a_concurrent_halted_run_finalizes_through_the_frozen_finalization_and_converges_after_a_kill_at_every_cell_even_cells`
- `a_concurrent_halted_run_finalizes_through_the_frozen_finalization_and_converges_after_a_kill_at_every_cell_odd_cells`

Each part makes its own reference run. A
Complete or a Halted run (seed 13 halts with two candidates queued) is finished once in this process,
which lists the effect and run-directory cells its finalization consults after `run_finished`, and
every assertion on that run — its outcome, the state it settled to, at least six cells listed — is
made in both parts of an outcome, whichever cells the part goes on to kill. Then,
for every cell of the part (`Parity`: the cells at the even indices of that list, or at the odd ones),
a child (`finalization_kill_child_at_width_three`) runs the same seed and dies by the
kill at that cell, and this process adopts its directory and resumes it twice through the frozen
recovery order. The first resume finalizes to the state the uninterrupted
finalization left (report outcome, retained candidates, refs under the run namespace, worktrees, the
execution root); the second finds the report current, removes nothing and refuses the same run; neither
appends, and planted answer files stay byte-identical. Every child's temporary directory is a guard the
cell holds (`scaffold::child_temporary_of`, tag `tmp`, short for the Windows path budget), declared before
the run it adopts, so the run is dropped first and the guard then reclaims what the child left.

A cell keeps its index in the whole list wherever it is named: in the child's environment, in the
handoff directory's tag, in the messages and in the export's `kills`. The seed fixes the list — every
child must die where the reference run consulted its cell, which is asserted — so the two parts of an
outcome list the same cells, and each ends by asserting that it killed its parity's share of them
(`Parity::share_of`): between them the two kill every cell after `run_finished`, each once. The share
is asserted and not a number, because the reference run's own assertion on the list is a floor of six;
on Linux, when the parts were split, the Complete run listed twelve cells and the Halted run six, so
the parts killed six and six, and three and three.
A part's `seam/ST-18` export carries its own kills beside the length of the whole list
(`cells_after_run_finished`) and which part it is (`part`).

Until the parts were split, one test per outcome killed every cell, under these names without their
`_even_cells` and `_odd_cells` suffixes; dated review records cite those names at the heads they
measured.
