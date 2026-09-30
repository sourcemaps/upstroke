# `src/engine/topology/permits.rs`

Extended notes for [`src/engine/topology/permits.rs`](../../../../src/engine/topology/permits.rs).

The code is the authority for what it does; this file is the whole of its prose. Each section is
headed by the line of code it describes, spelled as it is in the source, so the heading is the grep
string that finds the code.

## Module

The PermitBroker: PR11's one authority for the four admission resources the
packet assigns to the broker — the pipeline and merge entitlements (R1, R2),
the provisional reservations (R13), the agent/pool slot pairs (R3) and the
invocation registrations (R4) — at every width.

`decisions.admission_and_leases.permits` and INV-18: "The coordinator never
blocks on an entitlement, provisional reservation, or slot; reservations are
all-or-nothing; acquisition order pipeline -> merge -> {agent, pool} is
acyclic; slot holders never wait; every agent CLI invocation incl. agent
probes acquires its atomic {agent, pool?} pair while gates and the shell
probe register without slots; every registration, grant, completion, cancel,
and provisional conversion is bound to a unique InvocationId or selection
identity". Enforced by "PermitBroker try_reserve", "atomic slot pairs granted
by the coordinator loop", "head-reservation FIFO", "InvocationId protocol",
"provisional reservation rules".

### Where each rule lives

- **R1, R2** are the fold's, and are read, not stored:
  [`crate::engine::topology::select::Entitlements`] reads the fold's own
  counts at the moment of a reservation.
- **R13** is [`crate::engine::topology::identity::Reservations`]: taken only
  when the derived count plus the outstanding reservations permits, never
  awaited, each bound to its selection, converted or cancelled once.
- **R3 and R4** are [`crate::engine::topology::identity::InvocationLedger`]
  with its [`crate::engine::topology::identity::SlotTable`]: registered once,
  granted by the FIFO with head reservation, settled once, the settlement
  releasing the pair.
- **The acquisition order** is [`crate::engine::topology::select::Standing`],
  the precondition of a slotted registration: a pipeline that holds no
  entitlement in the fold cannot request a pair.

The broker composes them and owns them. The two ledgers stay in
`identity.rs` because the frozen `recover.rs` names them through
`EmitContext`; the fold readers stay in `select.rs` because the fold is
named there already (record §3 R-J).

### Coordinator-owned, never shared

The broker is a plain value its owner holds: the width-1 run's
[`crate::engine::topology::run::TopologyRun`] today, the coordinator loop in
phase 3. No lock and no shared ownership inside it: it decides grants and
hands them back as return values, and the waiting is the pipelines' (record
§3 R-F). The one lock anywhere near it is a registering boundary's, for the
length of one call (`preflight.rs`, `Registering`).

### What it does not do

It never waits and never polls a Runner. It does not know about processes
or containers: "a granted or non-slotted running invocation is cancelled
after the Runner terminated its process or container" is its caller's
obligation, discharged by settling only after the Runner call returned. And
it has no coordinator loop to grant from yet: at width 1 every slotted
request is made synchronously and must be granted at once
([`crate::engine::topology::identity::InvocationLedger::register_at_once`]);
the pipelines that wait for grants arrive with phase 3.

## `pub struct ShutDown {`

What the broker's half of a shutdown cancelled: the outstanding provisional
reservations and the waiting slot requests.

## `pub struct PermitBroker {`

R13 and R4 (with R3 inside it), owned together.

## `impl PermitBroker` › `pub fn new(slots: SlotLimits) -> Self {`

An empty broker, which is what a process start requires
(`crash_reconstruction`: "provisional reservations, slot table, invocation
ledger … are empty at process start").

## `impl PermitBroker` › `pub fn for_run(derived: &Entitlements) -> Self {`

An empty broker whose slot limits default to the run's recorded
`max_parallel` (`permits.agent_pool_slots`: "max_per_agent, default
max_parallel … max_per_pool, default max_parallel").

## `impl PermitBroker` › `pub fn reserve(`

Take a provisional reservation for `key`, or refuse; never wait. `derived`
is the fold's entitlements now.

### Errors

As [`crate::engine::topology::identity::Reservations::reserve`].

## `impl PermitBroker` › `pub fn convert(&mut self, key: TaskKey, kind: ReservationKind) -> Result<(), UpstrokeError> {`

Convert `key`'s reservation at its first append.

## `impl PermitBroker` › `pub fn cancel_reservation(`

Cancel `key`'s reservation on a pre-append failure.

## `impl PermitBroker` › `pub fn register(`

`permits.protocol`'s `register(invocation_id, slots: None | Some{agent,
pool?})`: without slots, runnable at once; with a pair, granted or queued,
under the pipeline's `standing`.

## `impl PermitBroker` › `pub fn complete(`

Complete an invocation, releasing its pair; returns the waiting requests
that release granted, in grant order, for the coordinator to hand on.

## `impl PermitBroker` › `pub fn cancel(`

Cancel an invocation — withdrawing a waiting request, or releasing a running
one's pair after the Runner terminated it — and return what that granted.

## `impl PermitBroker` › `pub fn end(`

Settle an invocation by how its Runner call ended
(`InvocationLedger::end`): completed, cancelled, or — its process fate
unresolved — kept running with its pair, granting nothing. The coordinator
settles every pipeline's end through it.

## `impl PermitBroker` › `pub fn shut_down(&mut self) -> ShutDown {`

The broker's half of a shutdown: every provisional reservation cancelled and
every waiting request withdrawn, nothing granted on the way. Running
invocations are left registered: `permits.protocol` cancels them "after the
Runner terminated its process or container", which is the caller's to
observe.

## `impl PermitBroker` › `pub fn halves(&mut self) -> (&mut Reservations, &mut InvocationLedger) {`

The two ledgers, borrowed apart, for the shapes that carry them separately:
`EmitState` takes the reservations, the append-error discharge the
invocation ledger, and the frozen `EmitContext` both.

## `impl PermitBroker` › `pub fn duplicates(&self) -> u32 {`

Every duplicate or stale operation both ledgers ignored and counted (ST-02).

## `impl PermitBroker` › `pub fn is_empty(&self) -> bool {`

Nothing reserved, nothing ever registered, nothing held or waiting: the
state a process starts in.

## `impl PermitBroker` › `pub fn balances(&self) -> bool {`

The process-end condition: every reservation converted or cancelled once,
every registration settled once, every pair released once.

## `mod tests` › `fn pipeline_entitlements_are_read_from_the_fold_and_never_stored() {`

--- R1, R2 --------------------------------------------------------------

Every test below is **broker-only**: it drives the broker and its ledgers
directly, on folds built from events, with no Runner and no coordinator.
Phase 6 re-runs the slot tests of acceptance item 3 and the seeded orders as
runtime pool tests on a real multi-thread Tokio runtime; record §7 names
which.

## `mod tests` › `fn pipeline_entitlements_are_read_from_the_fold_and_never_stored() {`

Each generation class counts as `permits.pipeline` says, the broker's reading
is the fold's own count, and a broker built at process start is empty and
takes its slot limits from the recorded `max_parallel`.

## `mod tests` › `fn an_unresolved_transaction_holds_both_entitlements_and_a_queued_candidate_neither() {`

A queued candidate holds nothing, a started verification holds
`{pipeline, merge}`, and its terminal releases both. Built on prefixes of the
census's deferred-verification trace, so the transaction is the fold's own.

## `mod tests` › `fn a_reservation_is_taken_only_when_the_derived_count_plus_the_outstanding_permits() {`

--- R13 ---------------------------------------------------------------

Several reservations outstanding at width 2, a third refused without
waiting, one per task, and the count moving from provisional to derived at a
conversion without the total moving.

## `mod tests` › `fn the_merge_entitlement_admits_one_integration_reservation() {`

The one merge entitlement, held by an outstanding integration reservation or
by the fold's open transaction, refuses a second integration reservation
while pipeline entitlements remain. At width 3, so the refusal is the
merge's and not the pipeline's.

## `mod tests` › `fn each_kind_converts_at_its_first_append_and_the_total_never_moves() {`

ST-13's ledger half for the three kinds: dispatch converted at
`task_dispatched`, the retry reserved by `settle::retry` itself and converted
at `attempt_started(retry)`, and the integration pair converted in one
operation at `merge_verification_started`.

## `mod tests` › `fn the_fast_path_converts_its_pair_at_merge_prepared_and_releases_both_at_task_merged() {`

ST-13's fast path through the frozen merge module: the pair reserved through
the broker's check, converted once by `IntegrationJournal::converted` at
`merge_prepared(fast)`, both derived holdings present before the CAS, and
both released at `task_merged`.

## `mod tests` › `fn reservations_are_cancelled_on_a_pre_append_failure_run_end_shutdown_or_a_poisoned_fold() {`

The four cancellations ST-13 names, and a poisoned fold refusing the next
reservation.

## `mod tests` › `fn a_slot_request_from_a_pipeline_holding_no_entitlement_is_refused() {`

--- the acquisition order ---------------------------------------------

`permits.deadlock_freedom`'s order as an API rule: no generation, an open
generation with no attempt started, a stale attempt, a sequence whose
verification has not started or is another's, and a poisoned fold are all
refused a pair; an attempt in flight, a started verification and a probe are
admitted; a standing read for one pipeline admits no other's invocation.

## `mod tests` › `fn same_agent_and_pool_with_opposing_limits_serialize_on_the_binding_limit() {`

--- acceptance item 3: the slot tests ---------------------------------

Two requests of one agent in one pool, under `(max_per_agent, max_per_pool)`
of `(1, 2)` and then `(2, 1)`: each time the smaller limit binds and the
second waits for the first. The control at `(2, 2)` runs both.

## `mod tests` › `fn two_agents_with_their_own_pools_run_in_parallel() {`

Two agents, each in its own pool, both granted at limits of one.

## `mod tests` › `fn an_agent_without_a_pool_takes_its_agent_slot_only() {`

An agent with no pool is granted while a pool is full, and takes no pool
slot.

## `mod tests` › `fn agent_probes_acquire_and_release_their_pair() {`

A probe is an agent CLI invocation and takes its pair like one: a second
probe of the same agent waits until the first releases.

## `mod tests` › `fn gate_and_shell_probe_invocations_register_without_slots() {`

A gate and the shell probe are runnable at once while every slot is held,
take no pair, and a gate offered one is refused, as is a worker offered
none.

## `mod tests` › `fn broker_only_two_agents_sharing_one_pool_serialize_on_the_pool() {`

**Broker-only, labelled as the packet asks.** Two agents in one pool cannot
be reached end to end under the one-agent-per-pool model
(`alternatives[21]`: "unreachable under the current one-agent-per-pool
model; broker-only tests labeled"), so the pool's limit is shown binding on
the broker alone.

## `mod tests` › `fn broker_only_one_agent_in_two_pools_serializes_on_the_agent() {`

**Broker-only**, for the same reason: an agent is in one pool, so the agent
limit binding across two pools is shown on the broker alone.

## `mod tests` › `fn a_reviewer_and_a_worker_of_one_agent_never_hold_its_slot_at_once() {`

The half of `PR7-R3-ATTEMPT-002`'s witness the broker carries: at
`max_per_agent = 1` a reviewer of the agent a worker holds waits until the
worker releases. The other half — that a review pass is registered and holds
its pair while its process runs — is
`attempt::tests::a_review_pass_registers_and_holds_its_pair_while_its_process_runs`.

## `mod tests` › `fn duplicate_operations_are_ignored_and_counted_and_nothing_is_released_twice() {`

--- ST-02, ST-05, and seeded orders -----------------------------------

ST-02: duplicate completions and cancellations of an invocation, and a
duplicate conversion and a stale cancellation of a reservation, are ignored
and counted; the invocation granted after the first release keeps its pair;
a reservation never taken is refused rather than counted.

## `mod tests` › `struct Seeded(u64);`

A seeded SplitMix64, so an adversarial order is reproducible from its seed
and needs no ambient randomness.

## `mod tests` › `struct Trial {`

A slot table driven by a seeded adversary, with the bookkeeping the
invariants need: every request's pair and arrival, when each grant happened,
and when the current head became the head.

## `impl Trial` › `fn record(&mut self, granted: &[InvocationId]) {`

A settlement's grants are the heads first, then backfills against the new
head. So a grant that arrived before the head that remains was granted
before it became the head, and one that arrived after was a backfill past it.

## `impl Trial` › `fn check(&self, seed: u64) {`

After every operation: no agent or pool over its limit; the grant and
release counts agree with the holders; the head is waiting because its pair
does not fit, never while it fits; and every holder of a slot the head is
blocked on was granted **before** it became the head — so no later request
took a slot the head reserved.

## `mod tests` › `fn seeded_adversarial_orders_grant_every_request_in_order_and_exceed_no_limit() {`

Deadlock-freedom and FIFO fairness under 256 seeded adversarial orders of
requests, completions and cancellations, at random limits: the invariants
above hold after every step, and when the adversary finally completes
everything it holds, nothing is left waiting and every request that was not
withdrawn was granted exactly once. Waiting is only ever for a running
holder, and holders wait for nothing, so draining the holders drains the
queue.

## `mod tests` › `fn injected_duplicate_settlements_release_nothing_twice_and_no_count_goes_negative() {`

ST-05 under seeded injection: duplicate completions and cancellations mixed
into adversarial orders move nothing and are counted exactly; every pair is
released once; and a provisional ledger hit with stale cancellations never
holds more than `max_parallel`, refuses exactly when the width is full or the
task already holds one, and accounts every reservation it took.
